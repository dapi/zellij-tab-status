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

        let effects = match pipe_message.name.as_str() {
            "tab-status" => {
                pipe_handler::handle_status(&mut self.pane_to_tab, &pipe_message.payload)
            }
            _ => {
                eprintln!(
                    "[tab-status] WARNING: unknown pipe name '{}', ignoring",
                    pipe_message.name
                );
                vec![]
            }
        };

        if let Some(ref pipe_id) = cli_pipe_id {
            for effect in effects {
                match effect {
                    // Workaround for zellij 0.43.x bug: rename_tab() plugin API uses
                    // BTreeMap key lookup instead of position-based lookup (screen.rs:5070),
                    // causing "Failed to find tab with index" when tabs have been closed.
                    // Instead of calling rename_tab(), return the computed name to the CLI
                    // caller which will do `zellij action rename-tab` (works correctly).
                    PipeEffect::RenameTab { name, .. } => {
                        cli_pipe_output(pipe_id, &name);
                    }
                    PipeEffect::PipeOutput { output, .. } => {
                        cli_pipe_output(pipe_id, &output);
                    }
                }
            }
            // Unblock using CLI pipe ID so the response reaches the correct client
            unblock_cli_pipe_input(pipe_id);
        } else {
            // Non-CLI source: fall back to rename_tab API (may fail on 0.43.x)
            for effect in effects {
                match effect {
                    PipeEffect::RenameTab { tab_id, name } => {
                        rename_tab(tab_id, name);
                    }
                    PipeEffect::PipeOutput { .. } => {
                        eprintln!("[tab-status] WARNING: PipeOutput ignored (non-CLI source)");
                    }
                }
            }
        }

        false
    }

    fn render(&mut self, _rows: usize, _cols: usize) {}
}

impl State {
    fn rebuild_mapping(&mut self) {
        self.pane_to_tab.clear();

        for tab in self.tabs.iter() {
            if let Some(pane_list) = self.panes.panes.get(&tab.position) {
                for pane in pane_list {
                    // Skip plugin panes
                    if pane.is_plugin {
                        continue;
                    }

                    // Store tab.position (0-indexed) for pane-to-tab mapping
                    self.pane_to_tab
                        .insert(pane.id, (tab.position, tab.name.clone()));
                }
            }
        }

        eprintln!("[tab-status] Total mappings: {}", self.pane_to_tab.len());
    }
}
