use std::collections::{HashMap, HashSet};

pub const DEFAULT_BLINK_DELAY_MS: u64 = 500;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlinkCommand {
    pub tab_index: u32,
    pub tab_position: usize,
    pub name: String,
}

#[derive(Debug)]
struct BlinkTabState {
    frames: Vec<String>,
    base_name: String,
    current_frame: usize,
    delay_ms: u64,
    next_tick_ms: u64,
    paused_since_ms: Option<u64>,
}

#[derive(Debug, Default)]
pub struct BlinkRuntime {
    tabs: HashMap<u32, BlinkTabState>,
    paused_at_ms: Option<u64>,
}

pub fn normalize_delay_ms(delay_ms: Option<u64>) -> u64 {
    match delay_ms {
        Some(0) | None => DEFAULT_BLINK_DELAY_MS,
        Some(value) => value,
    }
}

impl BlinkRuntime {
    pub fn start(
        &mut self,
        tab_index: u32,
        base_name: String,
        frames: Vec<String>,
        delay_ms: u64,
        now_ms: u64,
    ) {
        if frames.len() < 2 {
            self.tabs.remove(&tab_index);
            return;
        }

        let delay_ms = delay_ms.max(1);
        self.tabs.insert(
            tab_index,
            BlinkTabState {
                frames,
                base_name,
                current_frame: 0,
                delay_ms,
                next_tick_ms: now_ms.saturating_add(delay_ms),
                paused_since_ms: self.paused_at_ms.map(|_| now_ms),
            },
        );
    }

    pub fn stop(&mut self, tab_index: u32) {
        self.tabs.remove(&tab_index);
    }

    pub fn update_base_name(&mut self, tab_index: u32, base_name: String) {
        if let Some(state) = self.tabs.get_mut(&tab_index) {
            state.base_name = base_name;
        }
    }

    pub fn pause(&mut self, now_ms: u64) {
        if self.paused_at_ms.is_some() {
            return;
        }

        self.paused_at_ms = Some(now_ms);
        for state in self.tabs.values_mut() {
            if state.paused_since_ms.is_none() {
                state.paused_since_ms = Some(now_ms);
            }
        }
    }

    pub fn resume(&mut self, now_ms: u64) {
        if self.paused_at_ms.take().is_none() {
            return;
        }

        for state in self.tabs.values_mut() {
            if let Some(paused_since_ms) = state.paused_since_ms.take() {
                state.next_tick_ms = state
                    .next_tick_ms
                    .saturating_add(now_ms.saturating_sub(paused_since_ms));
            }
        }
    }

    pub fn tick(&mut self, now_ms: u64, tab_positions: &HashMap<u32, usize>) -> Vec<BlinkCommand> {
        if self.paused_at_ms.is_some() {
            return Vec::new();
        }

        let mut commands = Vec::new();
        let mut stale_tabs = Vec::new();

        for (&tab_index, state) in &mut self.tabs {
            let Some(&tab_position) = tab_positions.get(&tab_index) else {
                stale_tabs.push(tab_index);
                continue;
            };

            if now_ms < state.next_tick_ms {
                continue;
            }

            let steps = ((now_ms - state.next_tick_ms) / state.delay_ms) + 1;
            state.current_frame = (state.current_frame + steps as usize) % state.frames.len();
            state.next_tick_ms = state
                .next_tick_ms
                .saturating_add(state.delay_ms.saturating_mul(steps));

            let frame = &state.frames[state.current_frame];
            commands.push(BlinkCommand {
                tab_index,
                tab_position,
                name: format!("{} {}", frame, state.base_name),
            });
        }

        for tab_index in stale_tabs {
            self.tabs.remove(&tab_index);
        }

        commands.sort_by_key(|cmd| cmd.tab_index);
        commands
    }

    pub fn next_delay_ms(&self, now_ms: u64) -> Option<u64> {
        if self.paused_at_ms.is_some() {
            return None;
        }

        self.tabs
            .values()
            .map(|state| {
                if state.next_tick_ms <= now_ms {
                    1
                } else {
                    state.next_tick_ms - now_ms
                }
            })
            .min()
    }

    pub fn retain_active_tab_indices(&mut self, active_tab_indices: &[u32]) {
        let active: HashSet<u32> = active_tab_indices.iter().copied().collect();
        self.tabs.retain(|tab_index, _| active.contains(tab_index));
    }

    #[cfg(test)]
    fn contains_tab(&self, tab_index: u32) -> bool {
        self.tabs.contains_key(&tab_index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn positions(entries: &[(u32, usize)]) -> HashMap<u32, usize> {
        entries.iter().copied().collect()
    }

    #[test]
    fn normalize_delay_defaults_to_500ms() {
        assert_eq!(normalize_delay_ms(None), DEFAULT_BLINK_DELAY_MS);
        assert_eq!(normalize_delay_ms(Some(0)), DEFAULT_BLINK_DELAY_MS);
    }

    #[test]
    fn tick_rotates_frames_over_time() {
        let mut runtime = BlinkRuntime::default();
        runtime.start(
            7,
            "Build".to_string(),
            vec!["🔴".to_string(), "🟡".to_string(), "🟢".to_string()],
            500,
            0,
        );

        let no_updates = runtime.tick(499, &positions(&[(7, 0)]));
        assert!(no_updates.is_empty());

        let first = runtime.tick(500, &positions(&[(7, 0)]));
        assert_eq!(
            first,
            vec![BlinkCommand {
                tab_index: 7,
                tab_position: 0,
                name: "🟡 Build".to_string(),
            }]
        );

        let second = runtime.tick(1000, &positions(&[(7, 0)]));
        assert_eq!(
            second,
            vec![BlinkCommand {
                tab_index: 7,
                tab_position: 0,
                name: "🟢 Build".to_string(),
            }]
        );
    }

    #[test]
    fn delay_override_is_respected() {
        let mut runtime = BlinkRuntime::default();
        runtime.start(
            4,
            "Deploy".to_string(),
            vec!["🔴".to_string(), "🟡".to_string()],
            350,
            0,
        );

        assert!(runtime.tick(349, &positions(&[(4, 1)])).is_empty());
        let updates = runtime.tick(350, &positions(&[(4, 1)]));
        assert_eq!(
            updates,
            vec![BlinkCommand {
                tab_index: 4,
                tab_position: 1,
                name: "🟡 Deploy".to_string(),
            }]
        );
    }

    #[test]
    fn stop_clears_blinking_state() {
        let mut runtime = BlinkRuntime::default();
        runtime.start(
            3,
            "Tests".to_string(),
            vec!["🟥".to_string(), "🟩".to_string()],
            200,
            0,
        );
        runtime.stop(3);

        assert!(!runtime.contains_tab(3));
        assert!(runtime.tick(1000, &positions(&[(3, 0)])).is_empty());
    }

    #[test]
    fn retain_active_tab_indices_drops_deleted_tabs() {
        let mut runtime = BlinkRuntime::default();
        runtime.start(
            1,
            "One".to_string(),
            vec!["🔴".to_string(), "🟡".to_string()],
            500,
            0,
        );
        runtime.start(
            2,
            "Two".to_string(),
            vec!["🟥".to_string(), "🟨".to_string()],
            500,
            0,
        );

        runtime.retain_active_tab_indices(&[2]);

        assert!(!runtime.contains_tab(1));
        assert!(runtime.contains_tab(2));
    }

    #[test]
    fn pause_resume_delays_existing_state_without_losing_frame_order() {
        let mut runtime = BlinkRuntime::default();
        runtime.start(
            9,
            "Pause".to_string(),
            vec!["🔴".to_string(), "🟡".to_string()],
            500,
            0,
        );

        runtime.pause(100);
        runtime.resume(1_000);

        assert!(runtime.tick(1_399, &positions(&[(9, 0)])).is_empty());
        let updates = runtime.tick(1_400, &positions(&[(9, 0)]));
        assert_eq!(
            updates,
            vec![BlinkCommand {
                tab_index: 9,
                tab_position: 0,
                name: "🟡 Pause".to_string(),
            }]
        );
    }

    #[test]
    fn state_started_during_pause_resumes_with_single_delay() {
        let mut runtime = BlinkRuntime::default();
        runtime.pause(1_000);

        runtime.start(
            11,
            "Queued".to_string(),
            vec!["🟥".to_string(), "🟨".to_string()],
            500,
            1_500,
        );

        assert!(runtime.tick(10_000, &positions(&[(11, 0)])).is_empty());

        runtime.resume(2_000);
        assert!(runtime.tick(2_499, &positions(&[(11, 0)])).is_empty());
        let updates = runtime.tick(2_500, &positions(&[(11, 0)]));
        assert_eq!(
            updates,
            vec![BlinkCommand {
                tab_index: 11,
                tab_position: 0,
                name: "🟨 Queued".to_string(),
            }]
        );
    }
}
