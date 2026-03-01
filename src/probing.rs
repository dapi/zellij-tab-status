use std::collections::BTreeMap;

/// Maximum number of timer retries while waiting for restore confirmation.
/// After this many retries (5 seconds total), force advance to next candidate.
pub const MAX_RESTORE_RETRIES: u32 = 5;

#[derive(Debug)]
pub struct ProbingState {
    /// Tab names saved before probing started
    pub original_names: BTreeMap<usize, String>,
    /// Current candidate index being probed
    pub candidate: u32,
    /// Found mappings: (tab_position, persistent_index)
    pub found: Vec<(usize, u32)>,
    /// How many tabs still need to be found
    pub remaining: usize,
    /// true = waiting for name restoration after marker was found
    pub restoring: bool,
    /// Counts consecutive timer firings while restoring (reset on success)
    pub restore_retries: u32,
}

impl ProbingState {
    pub fn new(original_names: BTreeMap<usize, String>) -> Self {
        let remaining = original_names.len();
        Self {
            original_names,
            candidate: 1,
            found: Vec::new(),
            remaining,
            restoring: false,
            restore_retries: 0,
        }
    }
}

/// Result of handling a timer event during probing.
#[derive(Debug, PartialEq)]
pub enum TimerResult {
    /// Set another timeout and wait (restore retry)
    Retry,
    /// Advance to next candidate: send probe at given index
    AdvanceProbe(u32),
    /// Exceeded max candidates, fall back to [1..N]
    Fallback,
    /// Not in a state that needs timer handling (remaining=0, not restoring)
    Ignore,
}

/// Pure function: determine what to do when a timer fires during probing.
/// Extracted from update() for unit testability.
pub fn handle_probe_timer(state: &mut ProbingState) -> TimerResult {
    if state.restoring {
        state.restore_retries += 1;
        if state.restore_retries >= MAX_RESTORE_RETRIES {
            eprintln!(
                "[tab-status] WARNING: restore stuck for candidate={} after {} retries, forcing advance",
                state.candidate, state.restore_retries
            );
            state.restoring = false;
            state.restore_retries = 0;
            state.candidate += 1;

            let max_candidate = state.original_names.len() as u32 * 3;
            if state.candidate > max_candidate && state.remaining > 0 {
                return TimerResult::Fallback;
            }
            return TimerResult::AdvanceProbe(state.candidate);
        }
        eprintln!(
            "[tab-status] Probing: timer fired while restoring candidate={}, retry {}/{}",
            state.candidate, state.restore_retries, MAX_RESTORE_RETRIES
        );
        return TimerResult::Retry;
    }

    if state.remaining == 0 {
        return TimerResult::Ignore;
    }

    eprintln!(
        "[tab-status] Probing: timer fired, candidate={} is a gap (no TabUpdate received)",
        state.candidate
    );
    state.candidate += 1;

    let max_candidate = state.original_names.len() as u32 * 3;
    if state.candidate > max_candidate && state.remaining > 0 {
        eprintln!(
            "[tab-status] WARNING: probing exceeded limit (candidate={}), falling back to [1..N]",
            state.candidate
        );
        return TimerResult::Fallback;
    }

    TimerResult::AdvanceProbe(state.candidate)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_state(
        restoring: bool,
        restore_retries: u32,
        candidate: u32,
        remaining: usize,
        num_tabs: usize,
    ) -> ProbingState {
        let original_names: BTreeMap<usize, String> = (0..num_tabs)
            .map(|i| (i, format!("Tab {}", i + 1)))
            .collect();
        ProbingState {
            original_names,
            candidate,
            found: Vec::new(),
            remaining,
            restoring,
            restore_retries,
        }
    }

    #[test]
    fn gap_detection_normal() {
        let mut state = make_state(false, 0, 3, 2, 3);
        let result = handle_probe_timer(&mut state);
        assert_eq!(result, TimerResult::AdvanceProbe(4));
        assert_eq!(state.candidate, 4);
    }

    #[test]
    fn restore_retry_first() {
        let mut state = make_state(true, 0, 5, 1, 3);
        let result = handle_probe_timer(&mut state);
        assert_eq!(result, TimerResult::Retry);
        assert_eq!(state.restore_retries, 1);
        assert!(state.restoring);
    }

    #[test]
    fn restore_retry_mid() {
        let mut state = make_state(true, 3, 5, 1, 3);
        let result = handle_probe_timer(&mut state);
        assert_eq!(result, TimerResult::Retry);
        assert_eq!(state.restore_retries, 4);
        assert!(state.restoring);
    }

    #[test]
    fn restore_stuck_force_advance() {
        let mut state = make_state(true, 4, 5, 1, 3);
        let result = handle_probe_timer(&mut state);
        assert_eq!(result, TimerResult::AdvanceProbe(6));
        assert!(!state.restoring);
        assert_eq!(state.restore_retries, 0);
        assert_eq!(state.candidate, 6);
    }

    #[test]
    fn gap_max_candidate_fallback() {
        // 3 tabs, max_candidate = 9, candidate starts at 9 → advances to 10 > 9
        let mut state = make_state(false, 0, 9, 1, 3);
        let result = handle_probe_timer(&mut state);
        assert_eq!(result, TimerResult::Fallback);
    }

    #[test]
    fn stuck_restore_max_candidate_fallback() {
        // restoring stuck at retries=4, candidate=9, 3 tabs → max=9
        // force advance → candidate=10 > 9 → Fallback
        let mut state = make_state(true, 4, 9, 1, 3);
        let result = handle_probe_timer(&mut state);
        assert_eq!(result, TimerResult::Fallback);
    }

    #[test]
    fn ignore_when_remaining_zero() {
        let mut state = make_state(false, 0, 5, 0, 3);
        let result = handle_probe_timer(&mut state);
        assert_eq!(result, TimerResult::Ignore);
    }

    #[test]
    fn init_state_retries_zero() {
        let names: BTreeMap<usize, String> = [(0, "Tab 1".into())].into();
        let state = ProbingState::new(names);
        assert_eq!(state.restore_retries, 0);
        assert!(!state.restoring);
        assert_eq!(state.candidate, 1);
        assert_eq!(state.remaining, 1);
    }
}
