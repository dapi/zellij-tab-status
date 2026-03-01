use std::collections::{BTreeMap, HashMap};
use zellij_tile::prelude::*;

use zellij_tab_status::pipe_handler::{self, PipeEffect, StatusPayload};

/// Probing marker prefix: APL star diaeresis with numeric suffix.
/// Candidate-specific markers prevent delayed TabUpdate events from being
/// mis-attributed to the wrong candidate index.
const PROBE_MARKER_PREFIX: &str = "\u{235F}";

#[derive(Debug, Default)]
enum Phase {
    Probing(ProbingState),
    #[default]
    Ready,
}

#[derive(Debug)]
struct ProbingState {
    /// Tab names saved before probing started
    original_names: BTreeMap<usize, String>,
    /// Current candidate index being probed
    candidate: u32,
    /// Found mappings: (tab_position, persistent_index)
    found: Vec<(usize, u32)>,
    /// How many tabs still need to be found
    remaining: usize,
    /// true = waiting for name restoration after marker was found
    restoring: bool,
}

#[derive(Default)]
struct State {
    /// Maps pane_id -> tab_position (0-indexed). NEVER stores tab names.
    pane_to_tab: BTreeMap<u32, usize>,

    /// Current tabs info
    tabs: Vec<TabInfo>,

    /// Current panes info
    panes: PaneManifest,

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

    /// Deferred mutating commands keyed by pane_id.
    /// Keeps only the latest command per pane while plugin is not ready.
    queued_mutations: BTreeMap<u32, String>,
}

register_plugin!(State);

impl ZellijPlugin for State {
    fn load(&mut self, _configuration: BTreeMap<String, String>) {
        eprintln!("[tab-status] Plugin loaded v{}", env!("CARGO_PKG_VERSION"));

        // When launched on-demand via `zellij pipe --plugin`, hide this plugin pane
        // so it does not appear as an empty floating panel in the UI.
        hide_self();

        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
            PermissionType::ReadCliPipes,
        ]);
        subscribe(&[
            EventType::TabUpdate,
            EventType::PaneUpdate,
            EventType::Timer,
        ]);
    }

    fn update(&mut self, event: Event) -> bool {
        match event {
            Event::TabUpdate(tabs) => {
                eprintln!("[tab-status] TabUpdate: {} tabs", tabs.len());

                // Handle probing FSM before normal processing
                if let Phase::Probing(ref mut state) = self.phase {
                    let result = Self::handle_probing(&tabs, state);
                    match result {
                        ProbingResult::Continue => {
                            self.tabs = tabs;
                            self.rebuild_mapping();
                            return false;
                        }
                        ProbingResult::Complete(tab_indices) => {
                            eprintln!(
                                "[tab-status] Probing complete! tab_indices={:?}",
                                tab_indices
                            );
                            self.tab_indices = tab_indices;
                            self.next_tab_index =
                                self.tab_indices.iter().max().copied().unwrap_or(0) + 1;
                            self.phase = Phase::Ready;
                            self.tabs = tabs;
                            self.sync_pane_tab_index();
                            self.rebuild_mapping();
                            self.flush_queued_mutations();
                            eprintln!(
                                "[tab-status] Tab indices after probing: {:?} (next={})",
                                self.tab_indices, self.next_tab_index
                            );
                            return false;
                        }
                    }
                }

                // Normal TabUpdate processing (Phase::Ready)
                self.update_tab_indices(&tabs);

                // Safety net: if a delayed probe rename arrives after probing
                // completion, restore the previous tab name immediately.
                let leaked_markers: Vec<(usize, u32)> = tabs
                    .iter()
                    .filter_map(|tab| {
                        Self::parse_probe_marker(&tab.name)
                            .map(|candidate| (tab.position, candidate))
                    })
                    .collect();
                for (position, candidate) in leaked_markers {
                    let restore_name = self
                        .tabs
                        .iter()
                        .find(|tab| tab.position == position)
                        .map(|tab| tab.name.clone())
                        .filter(|name| Self::parse_probe_marker(name).is_none());
                    if let Some(name) = restore_name {
                        eprintln!(
                            "[tab-status] Ready: restoring leaked probe marker candidate={} position={} -> '{}'",
                            candidate, position, name
                        );
                        let actual_index = self.get_tab_index(position);
                        rename_tab(actual_index, &name);
                    } else {
                        eprintln!(
                            "[tab-status] WARNING: ready-phase probe marker leaked at position={} candidate={}, but no fallback name",
                            position, candidate
                        );
                    }
                }

                self.tabs = tabs;
                self.rebuild_mapping();
                self.flush_queued_mutations();
            }
            Event::PaneUpdate(panes) => {
                eprintln!("[tab-status] PaneUpdate: {} tab entries", panes.panes.len());
                self.panes = panes;
                // Note: sync_pane_tab_index is NOT called here because PaneUpdate
                // can arrive before TabUpdate during tab deletion, when pane positions
                // have shifted but tab_indices is stale. Sync only runs inside
                // update_tab_indices() where tab_indices are always correct.
                self.rebuild_mapping();
                self.flush_queued_mutations();
            }
            Event::Timer(_) => {
                // Timer fires during probing to detect gaps (non-existent indices).
                // If rename_tab targets a deleted index, Zellij silently ignores it
                // and sends no TabUpdate. The timer catches this.
                if let Phase::Probing(ref mut state) = self.phase {
                    if state.restoring {
                        // Recovery path: if restore rename was lost, retry it on timer.
                        if let Some((position, _)) = state
                            .found
                            .iter()
                            .find(|(_, candidate)| *candidate == state.candidate)
                        {
                            eprintln!(
                                "[tab-status] Probing: timer fired while restoring candidate={}, retry restore",
                                state.candidate
                            );
                            Self::restore_probe_marker(state, *position, state.candidate);
                            set_timeout(1.0);
                        } else {
                            eprintln!(
                                "[tab-status] WARNING: restoring candidate={} has no recorded position",
                                state.candidate
                            );
                        }
                    } else {
                        if state.remaining == 0 {
                            return false;
                        }
                        eprintln!(
                            "[tab-status] Probing: timer fired, candidate={} is a gap (no TabUpdate received)",
                            state.candidate
                        );
                        state.candidate += 1;

                        // Safety: prevent infinite loop
                        let max_candidate = state.original_names.len() as u32 * 3;
                        if state.candidate > max_candidate && state.remaining > 0 {
                            eprintln!(
                                "[tab-status] WARNING: probing exceeded limit (candidate={}), falling back to [1..N]",
                                state.candidate
                            );
                            let n = state.original_names.len();
                            let fallback: Vec<u32> = (1..=n as u32).collect();
                            self.tab_indices = fallback;
                            self.next_tab_index =
                                self.tab_indices.iter().max().copied().unwrap_or(0) + 1;
                            self.phase = Phase::Ready;
                            self.sync_pane_tab_index();
                            self.rebuild_mapping();
                            self.flush_queued_mutations();
                            return false;
                        }

                        Self::send_probe(state.candidate, "after gap");
                    }
                }
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

        let parsed_status = pipe_message
            .payload
            .as_ref()
            .and_then(|p| serde_json::from_str::<StatusPayload>(p).ok());

        // Queue mutating commands until pane->tab mapping is ready.
        if let (Some(status), Some(raw_payload)) =
            (parsed_status.as_ref(), pipe_message.payload.as_ref())
        {
            if self.should_queue_mutation(status) {
                self.enqueue_mutation(status, raw_payload);
                if let Some(ref pipe_id) = cli_pipe_id {
                    cli_pipe_output(pipe_id, pipe_handler::NOT_READY_OUTPUT);
                    unblock_cli_pipe_input(pipe_id);
                }
                return false;
            }
        }

        // Allow get_version and get_debug during probing, block everything else
        let is_probing = matches!(self.phase, Phase::Probing(_));
        if is_probing {
            let is_allowed = parsed_status.as_ref().is_some_and(|s| {
                s.action == "get_version" || s.action == "get_debug" || s.action == "probe_indices"
            });
            if !is_allowed {
                eprintln!("[tab-status] Probing in progress, signaling NOT_READY");
                if let Some(ref pipe_id) = cli_pipe_id {
                    cli_pipe_output(pipe_id, pipe_handler::NOT_READY_OUTPUT);
                    unblock_cli_pipe_input(pipe_id);
                }
                return false;
            }
        }

        // Handle get_debug before pipe_handler (needs access to State)
        if let Some(ref payload) = pipe_message.payload {
            if let Ok(status) = serde_json::from_str::<StatusPayload>(payload) {
                if status.action == "get_debug" {
                    let phase_str = match &self.phase {
                        Phase::Probing(_) => "probing",
                        Phase::Ready => "ready",
                    };
                    let debug_info = serde_json::json!({
                        "phase": phase_str,
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
                if status.action == "probe_indices" {
                    eprintln!("[tab-status] probe_indices: starting re-probe");
                    let original_names: BTreeMap<usize, String> = self
                        .tabs
                        .iter()
                        .map(|t| (t.position, t.name.clone()))
                        .collect();
                    let tab_count = original_names.len();
                    if tab_count == 0 {
                        eprintln!("[tab-status] probe_indices: no tabs, nothing to probe");
                        if let Some(ref pipe_id) = cli_pipe_id {
                            cli_pipe_output(pipe_id, "no tabs");
                            unblock_cli_pipe_input(pipe_id);
                        }
                        return false;
                    }
                    self.phase = Phase::Probing(ProbingState {
                        original_names,
                        candidate: 1,
                        found: Vec::new(),
                        remaining: tab_count,
                        restoring: false,
                    });
                    Self::send_probe(1, "manual re-probe start");
                    if let Some(ref pipe_id) = cli_pipe_id {
                        cli_pipe_output(pipe_id, "probing started");
                        unblock_cli_pipe_input(pipe_id);
                    }
                    return false;
                }
            }
        }

        let tab_names = self.tab_names();
        let effects =
            pipe_handler::handle_status(&self.pane_to_tab, &tab_names, &pipe_message.payload);

        self.apply_pipe_effects(&effects, cli_pipe_id.as_ref());

        if let Some(ref pipe_id) = cli_pipe_id {
            unblock_cli_pipe_input(pipe_id);
        }

        false
    }

    fn render(&mut self, _rows: usize, _cols: usize) {}
}

enum ProbingResult {
    Continue,
    Complete(Vec<u32>),
}

impl State {
    fn probe_marker(candidate: u32) -> String {
        format!("{}{}", PROBE_MARKER_PREFIX, candidate)
    }

    fn parse_probe_marker(name: &str) -> Option<u32> {
        name.strip_prefix(PROBE_MARKER_PREFIX)?.parse().ok()
    }

    fn send_probe(candidate: u32, context: &str) {
        let marker = Self::probe_marker(candidate);
        rename_tab(candidate, &marker);
        set_timeout(1.0);
        eprintln!(
            "[tab-status] Probing: sent probe candidate={} ({})",
            candidate, context
        );
    }

    fn probe_marker_hits(tabs: &[TabInfo]) -> Vec<(usize, u32)> {
        tabs.iter()
            .filter_map(|tab| {
                Self::parse_probe_marker(&tab.name).map(|candidate| (tab.position, candidate))
            })
            .collect()
    }

    fn record_found_candidate(state: &mut ProbingState, position: usize, candidate: u32) -> bool {
        if state.found.iter().any(|&(_, idx)| idx == candidate) {
            return false;
        }
        state.found.push((position, candidate));
        state.remaining = state.remaining.saturating_sub(1);
        true
    }

    fn restore_probe_marker(state: &ProbingState, position: usize, candidate: u32) {
        match state.original_names.get(&position) {
            Some(original) => {
                eprintln!(
                    "[tab-status] Probing: restoring name '{}' at index={}",
                    original, candidate
                );
                rename_tab(candidate, original);
            }
            None => {
                eprintln!(
                    "[tab-status] WARNING: missing original name for position={} while restoring candidate={}",
                    position, candidate
                );
            }
        }
    }

    fn finalize_probe(state: &mut ProbingState) -> ProbingResult {
        state.found.sort_by_key(|(pos, _)| *pos);
        let tab_indices: Vec<u32> = state.found.iter().map(|(_, idx)| *idx).collect();
        ProbingResult::Complete(tab_indices)
    }

    fn is_mutating_action(action: &str) -> bool {
        matches!(action, "set_status" | "clear_status" | "set_name")
    }

    fn should_queue_mutation(&self, status: &StatusPayload) -> bool {
        if !Self::is_mutating_action(status.action.as_str()) {
            return false;
        }
        if matches!(self.phase, Phase::Probing(_)) {
            return true;
        }
        match status.pane_id.parse::<u32>() {
            Ok(pane_id) => !self.pane_to_tab.contains_key(&pane_id),
            Err(_) => false,
        }
    }

    fn enqueue_mutation(&mut self, status: &StatusPayload, raw_payload: &str) {
        let pane_id = match status.pane_id.parse::<u32>() {
            Ok(pane_id) => pane_id,
            Err(e) => {
                eprintln!(
                    "[tab-status] enqueue_mutation: invalid pane_id='{}' action='{}': {}",
                    status.pane_id, status.action, e
                );
                return;
            }
        };
        self.queued_mutations
            .insert(pane_id, raw_payload.to_string());
        eprintln!(
            "[tab-status] queued mutation action='{}' pane_id={} queue_size={}",
            status.action,
            pane_id,
            self.queued_mutations.len()
        );
    }

    fn apply_pipe_effects(&self, effects: &[PipeEffect], cli_pipe_id: Option<&String>) {
        for effect in effects {
            match effect {
                PipeEffect::RenameTab { tab_position, name } => {
                    // Use persistent tab index (workaround for Zellij #3535)
                    let actual_index = self.get_tab_index(*tab_position);
                    rename_tab(actual_index, name);
                }
                PipeEffect::PipeOutput { output } => {
                    if let Some(pipe_id) = cli_pipe_id {
                        cli_pipe_output(pipe_id, output);
                    } else {
                        eprintln!("[tab-status] WARNING: PipeOutput ignored (non-CLI source)");
                    }
                }
            }
        }
    }

    fn flush_queued_mutations(&mut self) {
        if self.queued_mutations.is_empty() {
            return;
        }
        if matches!(self.phase, Phase::Probing(_)) || self.pane_to_tab.is_empty() {
            return;
        }

        let queued = std::mem::take(&mut self.queued_mutations);
        let mut remaining = BTreeMap::new();

        for (pane_id, payload) in queued {
            if !self.pane_to_tab.contains_key(&pane_id) {
                remaining.insert(pane_id, payload);
                continue;
            }

            let tab_names = self.tab_names();
            let effects =
                pipe_handler::handle_status(&self.pane_to_tab, &tab_names, &Some(payload));
            self.apply_pipe_effects(&effects, None);
        }

        if !remaining.is_empty() {
            eprintln!(
                "[tab-status] queued mutations still pending: {}",
                remaining.len()
            );
            self.queued_mutations = remaining;
        } else {
            eprintln!("[tab-status] flushed queued mutations");
        }
    }

    /// Track persistent tab indices across TabUpdate events.
    /// Uses pane IDs as stable anchors instead of name-based diff.
    /// Zellij bug #3535: rename_tab uses persistent 1-indexed tab index,
    /// not position. Indices are never reused after deletion.
    fn update_tab_indices(&mut self, new_tabs: &[TabInfo]) {
        let new_count = new_tabs.len();

        if self.tab_indices.is_empty() {
            // First TabUpdate: start probing to discover real persistent indices
            let original_names: BTreeMap<usize, String> = new_tabs
                .iter()
                .map(|t| (t.position, t.name.clone()))
                .collect();
            eprintln!(
                "[tab-status] Starting index probing for {} tabs, names: {:?}",
                new_count, original_names
            );

            // Temporary indices for pipe commands (will be overwritten after probing)
            self.tab_indices = (1..=new_count as u32).collect();
            self.next_tab_index = new_count as u32 + 1;
            self.sync_pane_tab_index();

            self.phase = Phase::Probing(ProbingState {
                original_names,
                candidate: 1,
                found: Vec::new(),
                remaining: new_count,
                restoring: false,
            });

            // Send first probe (timer detects gap if index 1 doesn't exist)
            Self::send_probe(1, "startup");
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
                panes.iter().filter(|p| !p.is_plugin).find_map(|p| {
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

    /// Handle one step of the probing FSM.
    fn handle_probing(tabs: &[TabInfo], state: &mut ProbingState) -> ProbingResult {
        let current_candidate = state.candidate;
        let marker_hits = Self::probe_marker_hits(tabs);

        if state.restoring {
            // While waiting for the current marker to disappear, we may still receive
            // delayed marker updates from older candidates. Handle those as late hits.
            let mut current_marker_present = false;
            for (position, candidate) in marker_hits {
                if candidate == current_candidate {
                    current_marker_present = true;
                    continue;
                }

                let is_new = Self::record_found_candidate(state, position, candidate);
                if is_new {
                    eprintln!(
                        "[tab-status] Probing: late marker candidate={} at position={}",
                        candidate, position
                    );
                } else {
                    eprintln!(
                        "[tab-status] Probing: duplicate late marker candidate={} at position={}",
                        candidate, position
                    );
                }
                Self::restore_probe_marker(state, position, candidate);
            }

            if current_marker_present {
                eprintln!("[tab-status] Probing: still waiting for restore");
                return ProbingResult::Continue;
            }

            eprintln!(
                "[tab-status] Probing: restore confirmed, candidate was {}",
                current_candidate
            );
            state.restoring = false;

            if state.remaining == 0 {
                // All tabs found
                return Self::finalize_probe(state);
            }

            state.candidate += 1;

            // Probe next candidate (timer detects gap if index doesn't exist)
            Self::send_probe(state.candidate, "after restore");
            return ProbingResult::Continue;
        }

        // Looking for probe markers in TabUpdate. Candidate-specific markers allow
        // out-of-order / delayed TabUpdate events to be processed safely.
        let mut found_current = false;
        for (position, candidate) in marker_hits {
            let is_new = Self::record_found_candidate(state, position, candidate);
            if is_new {
                eprintln!(
                    "[tab-status] Probing: found candidate={} at position={}",
                    candidate, position
                );
            } else {
                eprintln!(
                    "[tab-status] Probing: duplicate marker candidate={} at position={}",
                    candidate, position
                );
            }
            Self::restore_probe_marker(state, position, candidate);
            if candidate == current_candidate {
                found_current = true;
            } else {
                eprintln!(
                    "[tab-status] Probing: candidate={} arrived while waiting for candidate={}",
                    candidate, current_candidate
                );
            }
        }

        if found_current {
            state.restoring = true;
            return ProbingResult::Continue;
        }

        // If all indices were discovered out-of-order, complete as soon as no marker
        // remains visible in the current TabUpdate snapshot.
        if state.remaining == 0 {
            let marker_still_visible = tabs
                .iter()
                .any(|tab| Self::parse_probe_marker(&tab.name).is_some());
            if !marker_still_visible {
                return Self::finalize_probe(state);
            }
        }

        // Current marker not found yet — Zellij may not have processed rename_tab.
        // Gap detection is handled by Timer event (no TabUpdate = gap).
        ProbingResult::Continue
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

    /// Build tab names from current tabs (always fresh, never cached).
    fn tab_names(&self) -> Vec<String> {
        self.tabs.iter().map(|t| t.name.clone()).collect()
    }

    fn rebuild_mapping(&mut self) {
        self.pane_to_tab.clear();

        for tab in self.tabs.iter() {
            if let Some(pane_list) = self.panes.panes.get(&tab.position) {
                for pane in pane_list {
                    if pane.is_plugin {
                        continue;
                    }
                    self.pane_to_tab.insert(pane.id, tab.position);
                }
            }
        }

        eprintln!("[tab-status] Total mappings: {}", self.pane_to_tab.len());
    }
}
