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
            "[tab-status] Pipe: name={}, payload={:?}",
            pipe_message.name, pipe_message.payload
        );

        let effects = match pipe_message.name.as_str() {
            "tab-status" => pipe_handler::handle_status(
                &mut self.pane_to_tab,
                &pipe_message.payload,
                &pipe_message.name,
            ),
            _ => {
                eprintln!(
                    "[tab-status] WARNING: unknown pipe name '{}', ignoring",
                    pipe_message.name
                );
                vec![]
            }
        };

        for effect in effects {
            match effect {
                PipeEffect::RenameTab { tab_id, name } => rename_tab(tab_id, name),
                PipeEffect::PipeOutput { pipe_name, output } => {
                    cli_pipe_output(&pipe_name, &output);
                }
            }
        }

        // Always unblock CLI pipe to prevent `zellij pipe` from hanging
        unblock_cli_pipe_input(&pipe_message.name);

        false
    }

    fn render(&mut self, _rows: usize, _cols: usize) {}
}

impl State {
    fn rebuild_mapping(&mut self) {
        self.pane_to_tab.clear();

        for (display_index, tab) in self.tabs.iter().enumerate() {
            if let Some(pane_list) = self.panes.panes.get(&tab.position) {
                for pane in pane_list {
                    // Skip plugin panes
                    if pane.is_plugin {
                        continue;
                    }

                    // Store tab.position (0-indexed); pipe_handler adds +1 for rename_tab API
                    self.pane_to_tab
                        .insert(pane.id, (tab.position, tab.name.clone()));

                    eprintln!(
                        "[tab-status] Mapped pane {} -> tab position {} (display {}) '{}'",
                        pane.id, tab.position, display_index, tab.name
                    );
                }
            }
        }

        eprintln!("[tab-status] Total mappings: {}", self.pane_to_tab.len());
    }
}
