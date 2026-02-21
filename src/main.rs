use std::collections::BTreeMap;
use zellij_tile::prelude::*;

use zellij_tab_status::pipe_handler::{self, PaneTabMap, PipeEffect};

#[derive(Default)]
struct State {
    /// Maps pane_id -> (tab_position, tab_name)
    pane_to_tab: PaneTabMap,

    /// Current tabs info
    tabs: Vec<TabInfo>,

    /// Current panes info
    panes: PaneManifest,

    /// Pending renames: tab_position -> expected name.
    /// Protects against rebuild_mapping overwriting inline cache updates
    /// with stale tab names before TabUpdate confirms the rename.
    pending_renames: BTreeMap<usize, String>,

    /// Persistent Zellij tab indices for each position.
    /// Workaround for Zellij bug #3535: rename_tab() uses persistent internal
    /// tab index (1-indexed, never reused after deletion), NOT position.
    /// See: https://github.com/zellij-org/zellij/issues/3535
    tab_indices: Vec<u32>,

    /// Counter for the next tab index to assign to newly detected tabs.
    next_tab_index: u32,
}

register_plugin!(State);

impl ZellijPlugin for State {
    fn load(&mut self, _configuration: BTreeMap<String, String>) {
        eprintln!("[tab-status] Plugin loaded v{}", env!("CARGO_PKG_VERSION"));

        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
            PermissionType::ReadCliPipes,
        ]);
        subscribe(&[EventType::TabUpdate, EventType::PaneUpdate]);
    }

    fn update(&mut self, event: Event) -> bool {
        match event {
            Event::TabUpdate(tabs) => {
                eprintln!("[tab-status] TabUpdate: {} tabs", tabs.len());
                // Track persistent tab indices (workaround for Zellij #3535)
                self.update_tab_indices(&tabs);
                // Confirm pending renames that Zellij has applied
                self.pending_renames.retain(|pos, pending_name| {
                    match tabs.iter().find(|tab| tab.position == *pos) {
                        Some(tab) => tab.name != *pending_name,
                        None => false, // tab position gone (deleted)
                    }
                });
                if !self.pending_renames.is_empty() {
                    eprintln!(
                        "[tab-status] Pending renames still active: {:?}",
                        self.pending_renames
                    );
                }
                self.tabs = tabs;
                self.rebuild_mapping();
            }
            Event::PaneUpdate(panes) => {
                eprintln!("[tab-status] PaneUpdate: {} tab entries", panes.panes.len());
                self.panes = panes;
                self.rebuild_mapping();
            }
            _ => {}
        }
        false
    }

    fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
        eprintln!(
            "[tab-status] Pipe: name={}, source={:?}, payload={:?}",
            pipe_message.name, pipe_message.source, pipe_message.payload
        );

        // Extract CLI pipe ID for response routing
        let cli_pipe_id = match &pipe_message.source {
            PipeSource::Cli(pipe_id) => Some(pipe_id.clone()),
            _ => None,
        };

        let effects =
            pipe_handler::handle_status(&mut self.pane_to_tab, &pipe_message.payload);

        for effect in &effects {
            match effect {
                PipeEffect::RenameTab { tab_id, name } => {
                    let tab_position = (*tab_id - 1) as usize;
                    // Use persistent tab index (workaround for Zellij #3535)
                    let actual_index = self.get_tab_index(tab_position);
                    self.pending_renames.insert(tab_position, name.clone());
                    rename_tab(actual_index, name);
                }
                PipeEffect::PipeOutput { output } => {
                    if let Some(ref pipe_id) = cli_pipe_id {
                        cli_pipe_output(pipe_id, output);
                    } else {
                        eprintln!("[tab-status] WARNING: PipeOutput ignored (non-CLI source)");
                    }
                }
            }
        }

        if let Some(ref pipe_id) = cli_pipe_id {
            unblock_cli_pipe_input(pipe_id);
        }

        false
    }

    fn render(&mut self, _rows: usize, _cols: usize) {}
}

impl State {
    /// Track persistent tab indices across TabUpdate events.
    /// Zellij bug #3535: rename_tab uses persistent 1-indexed tab index,
    /// not position. Indices are never reused after deletion.
    fn update_tab_indices(&mut self, new_tabs: &[TabInfo]) {
        let new_count = new_tabs.len();

        if self.tab_indices.is_empty() {
            // First TabUpdate: indices are 1-indexed sequential
            self.tab_indices = (1..=new_count as u32).collect();
            self.next_tab_index = new_count as u32 + 1;
            eprintln!(
                "[tab-status] Tab indices initialized: {:?} (next={})",
                self.tab_indices, self.next_tab_index
            );
            return;
        }

        let old_count = self.tab_indices.len();

        if new_count == old_count {
            // No structural change (just renames), indices stay the same
            return;
        }

        // Build old names from self.tabs (the previous TabUpdate state)
        let old_names: Vec<&str> = self.tabs.iter().map(|t| t.name.as_str()).collect();
        let new_names: Vec<&str> = new_tabs.iter().map(|t| t.name.as_str()).collect();

        // Diff old vs new to track which indices survive
        let mut new_indices = Vec::with_capacity(new_count);
        let mut oi = 0usize; // old index
        let mut ni = 0usize; // new index

        while ni < new_count {
            if oi < old_count && old_names[oi] == new_names[ni] {
                // Matched: same tab at this position
                new_indices.push(self.tab_indices[oi]);
                oi += 1;
                ni += 1;
            } else if new_count < old_count && oi < old_count {
                // Tab deleted at old position oi — skip it
                oi += 1;
            } else {
                // New tab at new position ni
                new_indices.push(self.next_tab_index);
                self.next_tab_index += 1;
                ni += 1;
            }
        }

        self.tab_indices = new_indices;
        // Recalculate next index to match Zellij's get_new_tab_index()
        // which uses `last_key + 1`, NOT a monotonically increasing counter.
        self.next_tab_index = self.tab_indices.iter().max().copied().unwrap_or(0) + 1;
        eprintln!(
            "[tab-status] Tab indices updated: {:?} (next={})",
            self.tab_indices, self.next_tab_index
        );
    }

    /// Get the persistent Zellij tab index for a given position.
    /// Falls back to position + 1 if tracking is not yet initialized.
    fn get_tab_index(&self, position: usize) -> u32 {
        self.tab_indices
            .get(position)
            .copied()
            .unwrap_or((position as u32) + 1)
    }

    fn rebuild_mapping(&mut self) {
        self.pane_to_tab.clear();

        for tab in self.tabs.iter() {
            // Use pending rename if available (protects against stale self.tabs)
            let tab_name = self
                .pending_renames
                .get(&tab.position)
                .cloned()
                .unwrap_or_else(|| tab.name.clone());

            if let Some(pane_list) = self.panes.panes.get(&tab.position) {
                for pane in pane_list {
                    // Skip plugin panes
                    if pane.is_plugin {
                        continue;
                    }

                    // Store tab.position (0-indexed) for pane-to-tab mapping
                    self.pane_to_tab
                        .insert(pane.id, (tab.position, tab_name.clone()));
                }
            }
        }

        eprintln!("[tab-status] Total mappings: {}", self.pane_to_tab.len());
    }
}
