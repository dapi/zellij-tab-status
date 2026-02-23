use std::collections::{BTreeMap, HashMap};
use zellij_tile::prelude::*;

use zellij_tab_status::pipe_handler::{self, PaneTabMap, PipeEffect, StatusPayload};

/// Probing marker: APL star diaeresis (monochrome, not used as regular status)
const PROBE_MARKER: &str = "\u{235F}";

#[derive(Debug)]
enum Phase {
    Probing(ProbingState),
    Ready,
}

#[derive(Debug)]
struct ProbingState {
    /// Tab names saved before probing started
    original_names: Vec<String>,
    /// Current candidate index being probed
    candidate: u32,
    /// Found mappings: (tab_position, persistent_index)
    found: Vec<(usize, u32)>,
    /// How many tabs still need to be found
    remaining: usize,
    /// true = waiting for name restoration after marker was found
    restoring: bool,
}

impl Default for Phase {
    fn default() -> Self {
        Phase::Ready
    }
}

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

    /// pane_id -> persistent tab_index. Pane IDs are stable anchors
    /// for identifying tabs even when positions shift after deletions.
    pane_tab_index: HashMap<u32, u32>,

    /// Current plugin phase: Probing (detecting tab indices) or Ready
    phase: Phase,
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
                // Note: sync_pane_tab_index is NOT called here because PaneUpdate
                // can arrive before TabUpdate during tab deletion, when pane positions
                // have shifted but tab_indices is stale. Sync only runs inside
                // update_tab_indices() where tab_indices are always correct.
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

        // Handle get_debug before pipe_handler (needs access to State)
        if let Some(ref payload) = pipe_message.payload {
            if let Ok(status) = serde_json::from_str::<StatusPayload>(payload) {
                if status.action == "get_debug" {
                    let debug_info = serde_json::json!({
                        "tab_indices": self.tab_indices,
                        "next_tab_index": self.next_tab_index,
                        "pane_tab_index": self.pane_tab_index,
                        "pane_to_tab_count": self.pane_to_tab.len(),
                    });
                    let output = debug_info.to_string();
                    eprintln!("[tab-status] get_debug: {}", output);
                    if let Some(ref pipe_id) = cli_pipe_id {
                        cli_pipe_output(pipe_id, &output);
                        unblock_cli_pipe_input(pipe_id);
                    }
                    return false;
                }
            }
        }

        let effects =
            pipe_handler::handle_status(&mut self.pane_to_tab, &pipe_message.payload);

        for effect in &effects {
            match effect {
                PipeEffect::RenameTab { tab_position, name } => {
                    // Use persistent tab index (workaround for Zellij #3535)
                    let actual_index = self.get_tab_index(*tab_position);
                    self.pending_renames.insert(*tab_position, name.clone());
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
    /// Uses pane IDs as stable anchors instead of name-based diff.
    /// Zellij bug #3535: rename_tab uses persistent 1-indexed tab index,
    /// not position. Indices are never reused after deletion.
    fn update_tab_indices(&mut self, new_tabs: &[TabInfo]) {
        let new_count = new_tabs.len();

        if self.tab_indices.is_empty() {
            // First TabUpdate: assume indices are 1-indexed sequential
            self.tab_indices = (1..=new_count as u32).collect();
            self.next_tab_index = new_count as u32 + 1;
            self.sync_pane_tab_index();
            eprintln!(
                "[tab-status] Tab indices initialized: {:?} (next={})",
                self.tab_indices, self.next_tab_index
            );
            return;
        }

        if new_count == self.tab_indices.len() {
            // No structural change (just renames), sync pane mapping
            self.sync_pane_tab_index();
            return;
        }

        // Structural change: find surviving tabs by pane_id lookup.
        // Only trust pane_tab_index entries whose index exists in current tab_indices
        // (stale entries from deleted tabs would cause wrong index assignment).
        let mut new_indices = Vec::with_capacity(new_count);
        for pos in 0..new_count {
            let known = self.panes.panes.get(&pos).and_then(|panes| {
                panes
                    .iter()
                    .filter(|p| !p.is_plugin)
                    .find_map(|p| {
                        self.pane_tab_index
                            .get(&p.id)
                            .copied()
                            .filter(|idx| self.tab_indices.contains(idx))
                    })
            });
            match known {
                Some(idx) => new_indices.push(idx),
                None => {
                    new_indices.push(self.next_tab_index);
                    self.next_tab_index += 1;
                }
            }
        }

        self.tab_indices = new_indices;
        self.next_tab_index = self.tab_indices.iter().max().copied().unwrap_or(0) + 1;
        self.sync_pane_tab_index();
        eprintln!(
            "[tab-status] Tab indices updated: {:?} (next={})",
            self.tab_indices, self.next_tab_index
        );
    }

    /// Rebuild pane_id -> persistent tab_index mapping from current tab_indices + PaneManifest.
    /// Clears stale entries to prevent reused pane IDs from mapping to deleted tab indices.
    fn sync_pane_tab_index(&mut self) {
        self.pane_tab_index.clear();
        for (pos, &tab_idx) in self.tab_indices.iter().enumerate() {
            if let Some(panes) = self.panes.panes.get(&pos) {
                for pane in panes {
                    if !pane.is_plugin {
                        self.pane_tab_index.insert(pane.id, tab_idx);
                    }
                }
            }
        }
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
