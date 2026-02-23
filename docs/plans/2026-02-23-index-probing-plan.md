# Index Probing Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Определить реальные persistent tab indices при загрузке плагина через sequential probing (rename_tab + наблюдение в TabUpdate).

**Architecture:** FSM с двумя фазами (`Probing`/`Ready`). При первом TabUpdate плагин последовательно пробует `rename_tab(candidate, "⍟")`, наблюдает какой таб получил маркер, запоминает `(position, index)`, восстанавливает имя, переходит к следующему candidate. Pipe-команды заблокированы во время probing.

**Tech Stack:** Rust, zellij-tile 0.43.1, WASM

---

### Task 1: Интеграционный тест — probing при старте

**Files:**
- Modify: `scripts/integration-test.sh` (добавить Test 20 в конец, перед Summary)

**Step 1: Написать тест**

Добавить в `scripts/integration-test.sh` перед строкой `# --- Summary ---`:

```bash
# --- Test 20: get_debug shows correct tab_indices ---
echo "--- 20. get_debug tab_indices ---"
close_extra_tabs
PANE_ID=$(discover_pane_id)

pipe_cmd "{\"pane_id\":\"$PANE_ID\",\"action\":\"set_name\",\"name\":\"DbgTab\"}"
pipe_cmd "{\"pane_id\":\"$PANE_ID\",\"action\":\"clear_status\"}"
wait_for_name "$PANE_ID" "DbgTab" "tab named DbgTab"

# Create second tab
zellij action new-tab
wait_for_tab_count 2
PANE_DBG2=$(discover_pane_id)
pipe_cmd "{\"pane_id\":\"$PANE_DBG2\",\"action\":\"set_name\",\"name\":\"DbgTab2\"}"
wait_for_name "$PANE_DBG2" "DbgTab2" "tab2 named DbgTab2"

# get_debug should return JSON with tab_indices
debug_result=$(pipe_cmd "{\"action\":\"get_debug\"}")
echo "  Debug output: $debug_result"
assert_contains "$debug_result" "tab_indices" "get_debug returns tab_indices"
assert_contains "$debug_result" "next_tab_index" "get_debug returns next_tab_index"
assert_contains "$debug_result" "pane_tab_index" "get_debug returns pane_tab_index"
```

**Step 2: Прогнать тесты — убедиться что Test 20 проходит (get_debug уже реализован)**

Run: `make test-integration`
Expected: Test 20 PASS (get_debug уже работает с прошлого PR)

**Step 3: Commit**

```bash
git add scripts/integration-test.sh
git commit -m "test: add get_debug integration test (Test 20)"
```

---

### Task 2: Phase enum и ProbingState struct

**Files:**
- Modify: `src/main.rs` (добавить enum Phase, struct ProbingState, поле phase в State)

**Step 1: Добавить Phase enum и ProbingState**

В `src/main.rs` после строки `use zellij_tab_status::pipe_handler::{self, PaneTabMap, PipeEffect, StatusPayload};` добавить:

```rust
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
```

В struct `State` добавить поле:

```rust
    /// Current plugin phase: Probing (detecting tab indices) or Ready
    phase: Phase,
```

**Step 2: Убедиться что компилируется**

Run: `cargo build --target wasm32-wasip1`
Expected: компиляция успешна (phase не используется, но компилируется)

**Step 3: Прогнать unit-тесты**

Run: `cargo test --lib`
Expected: все тесты проходят

**Step 4: Commit**

```bash
git add src/main.rs
git commit -m "feat: add Phase enum and ProbingState struct"
```

---

### Task 3: Запуск probing при первом TabUpdate

**Files:**
- Modify: `src/main.rs` — изменить ветку `tab_indices.is_empty()` в `update_tab_indices()`

**Step 1: Заменить начальную инициализацию на запуск probing**

В `update_tab_indices()`, заменить блок `if self.tab_indices.is_empty()`:

```rust
    fn update_tab_indices(&mut self, new_tabs: &[TabInfo]) {
        let new_count = new_tabs.len();

        if self.tab_indices.is_empty() {
            // First TabUpdate: start probing to discover real persistent indices
            let original_names: Vec<String> = new_tabs.iter().map(|t| t.name.clone()).collect();
            eprintln!(
                "[tab-status] Starting index probing for {} tabs, names: {:?}",
                new_count, original_names
            );

            // Temporary indices for pipe blocking (will be overwritten after probing)
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

            // Send first probe
            rename_tab(1, PROBE_MARKER);
            return;
        }

        // ... rest unchanged
```

**Step 2: Убедиться что компилируется**

Run: `cargo build --target wasm32-wasip1`
Expected: компиляция успешна

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat: start probing on first TabUpdate instead of assuming [1..N]"
```

---

### Task 4: Probing FSM в TabUpdate handler

**Files:**
- Modify: `src/main.rs` — добавить обработку Phase::Probing в `update()` метод

**Step 1: Добавить probing FSM в Event::TabUpdate**

В методе `update()`, заменить ветку `Event::TabUpdate(tabs)`:

```rust
            Event::TabUpdate(tabs) => {
                eprintln!("[tab-status] TabUpdate: {} tabs", tabs.len());

                // Handle probing FSM before normal processing
                if let Phase::Probing(ref mut state) = self.phase {
                    let handled = Self::handle_probing(&tabs, state);
                    match handled {
                        ProbingResult::Continue => {
                            // Update tabs for rebuild_mapping but don't touch indices
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
                            self.next_tab_index = self.tab_indices.iter()
                                .max().copied().unwrap_or(0) + 1;
                            self.phase = Phase::Ready;
                            self.tabs = tabs;
                            self.sync_pane_tab_index();
                            self.rebuild_mapping();
                            return false;
                        }
                        ProbingResult::NotProbing => {
                            // Fall through to normal processing
                        }
                    }
                }

                // Normal TabUpdate processing (Phase::Ready)
                self.update_tab_indices(&tabs);
                // ... rest unchanged (pending_renames, self.tabs = tabs, rebuild_mapping)
```

**Step 2: Добавить ProbingResult enum и handle_probing**

```rust
enum ProbingResult {
    Continue,
    Complete(Vec<u32>),
    NotProbing,
}

impl State {
    /// Handle one step of the probing FSM.
    /// Returns ProbingResult indicating what the caller should do.
    fn handle_probing(tabs: &[TabInfo], state: &mut ProbingState) -> ProbingResult {
        if state.restoring {
            // Phase: waiting for name restoration
            // Check that marker is gone (name restored)
            let marker_gone = !tabs.iter().any(|t| t.name == PROBE_MARKER);
            if !marker_gone {
                eprintln!("[tab-status] Probing: still waiting for restore");
                return ProbingResult::Continue;
            }

            eprintln!("[tab-status] Probing: restore confirmed, candidate was {}", state.candidate);
            state.restoring = false;
            state.candidate += 1;

            if state.remaining == 0 {
                // All tabs found — build tab_indices sorted by position
                state.found.sort_by_key(|(pos, _)| *pos);
                let tab_indices: Vec<u32> = state.found.iter().map(|(_, idx)| *idx).collect();
                return ProbingResult::Complete(tab_indices);
            }

            // Probe next candidate
            rename_tab(state.candidate, PROBE_MARKER);
            eprintln!("[tab-status] Probing: sent probe candidate={}", state.candidate);
            return ProbingResult::Continue;
        }

        // Phase: looking for marker
        let marker_pos = tabs.iter().position(|t| t.name == PROBE_MARKER);

        match marker_pos {
            Some(pos) => {
                // Found! Record mapping and restore original name
                eprintln!(
                    "[tab-status] Probing: found candidate={} at position={}",
                    state.candidate, pos
                );
                state.found.push((pos, state.candidate));
                state.remaining -= 1;

                // Restore original name
                let original = &state.original_names[pos];
                eprintln!("[tab-status] Probing: restoring name '{}' at index={}", original, state.candidate);
                rename_tab(state.candidate, original);
                state.restoring = true;

                ProbingResult::Continue
            }
            None => {
                // Not found — this index doesn't exist (was deleted)
                eprintln!(
                    "[tab-status] Probing: candidate={} is a gap (deleted index)",
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
                    return ProbingResult::Complete(fallback);
                }

                // Probe next candidate
                rename_tab(state.candidate, PROBE_MARKER);
                eprintln!("[tab-status] Probing: sent probe candidate={}", state.candidate);

                ProbingResult::Continue
            }
        }
    }
}
```

**Step 3: Убедиться что компилируется**

Run: `cargo build --target wasm32-wasip1`
Expected: компиляция успешна

**Step 4: Прогнать unit-тесты**

Run: `cargo test --lib`
Expected: все тесты проходят

**Step 5: Commit**

```bash
git add src/main.rs
git commit -m "feat: implement probing FSM in TabUpdate handler"
```

---

### Task 5: Блокировка pipe-команд во время probing

**Files:**
- Modify: `src/main.rs` — добавить проверку Phase в `pipe()` метод

**Step 1: Добавить блокировку**

В начало метода `pipe()`, после извлечения `cli_pipe_id`, добавить:

```rust
        // Allow get_version and get_debug during probing, block everything else
        let is_probing = matches!(self.phase, Phase::Probing(_));
        if is_probing {
            let is_allowed = pipe_message.payload.as_ref().map_or(false, |p| {
                serde_json::from_str::<StatusPayload>(p)
                    .map(|s| s.action == "get_version" || s.action == "get_debug")
                    .unwrap_or(false)
            });
            if !is_allowed {
                eprintln!("[tab-status] Probing in progress, blocking pipe command");
                if let Some(ref pipe_id) = cli_pipe_id {
                    cli_pipe_output(pipe_id, "");
                    unblock_cli_pipe_input(pipe_id);
                }
                return false;
            }
        }
```

**Step 2: Убедиться что компилируется**

Run: `cargo build --target wasm32-wasip1`
Expected: компиляция успешна

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat: block pipe commands during probing phase"
```

---

### Task 6: Команда `probe_indices`

**Files:**
- Modify: `src/main.rs` — добавить обработку action `probe_indices` в `pipe()`

**Step 1: Добавить обработку probe_indices**

В `pipe()`, после блока обработки `get_debug` (перед вызовом `pipe_handler::handle_status`), добавить:

```rust
                if status.action == "probe_indices" {
                    eprintln!("[tab-status] probe_indices: starting re-probe");
                    let original_names: Vec<String> =
                        self.tabs.iter().map(|t| t.name.clone()).collect();
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
                    rename_tab(1, PROBE_MARKER);
                    if let Some(ref pipe_id) = cli_pipe_id {
                        cli_pipe_output(pipe_id, "probing started");
                        unblock_cli_pipe_input(pipe_id);
                    }
                    return false;
                }
```

**Step 2: Убедиться что компилируется**

Run: `cargo build --target wasm32-wasip1`
Expected: компиляция успешна

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat: add probe_indices command for on-demand re-probing"
```

---

### Task 7: Интеграционный тест — probe_indices command

**Files:**
- Modify: `scripts/integration-test.sh` (добавить Test 21 перед Summary)

**Step 1: Написать тест**

```bash
# --- Test 21: probe_indices command ---
echo "--- 21. probe_indices re-probing ---"
close_extra_tabs
PANE_ID=$(discover_pane_id)

pipe_cmd "{\"pane_id\":\"$PANE_ID\",\"action\":\"set_name\",\"name\":\"ProbeTab\"}"
pipe_cmd "{\"pane_id\":\"$PANE_ID\",\"action\":\"clear_status\"}"
wait_for_name "$PANE_ID" "ProbeTab" "tab named ProbeTab"

# Create second tab
zellij action new-tab
wait_for_tab_count 2
PANE_PROBE2=$(discover_pane_id)
pipe_cmd "{\"pane_id\":\"$PANE_PROBE2\",\"action\":\"set_name\",\"name\":\"ProbeTab2\"}"
wait_for_name "$PANE_PROBE2" "ProbeTab2" "tab2 named ProbeTab2"

# Trigger probe_indices
result=$(pipe_cmd "{\"action\":\"probe_indices\"}")
assert_eq "$result" "probing started" "probe_indices returns 'probing started'"

# Wait for probing to complete — names should be restored
sleep 3
wait_for_name "$PANE_ID" "ProbeTab" "tab1 name restored after probing"
wait_for_name "$PANE_PROBE2" "ProbeTab2" "tab2 name restored after probing"

# Verify get_debug shows correct indices after probing
debug_result=$(pipe_cmd "{\"action\":\"get_debug\"}")
echo "  Debug after probe: $debug_result"
assert_contains "$debug_result" "tab_indices" "get_debug works after probing"

# Verify plugin still works normally after probing
pipe_cmd "{\"pane_id\":\"$PANE_ID\",\"action\":\"set_status\",\"emoji\":\"✅\"}"
wait_for_tab_contains "✅ ProbeTab" "set_status works after probing"
```

**Step 2: Прогнать интеграционные тесты**

Run: `make test-integration`
Expected: все 21 тест проходят

**Step 3: Commit**

```bash
git add scripts/integration-test.sh
git commit -m "test: add probe_indices integration test (Test 21)"
```

---

### Task 8: Интеграционный тест — probing с gap (удалённый таб)

**Files:**
- Modify: `scripts/integration-test.sh` (добавить Test 22 перед Summary)

**Step 1: Написать тест**

Этот тест проверяет, что `probe_indices` корректно обрабатывает gap'ы (удалённые табы, чьи persistent indices были пропущены).

```bash
# --- Test 22: probe_indices after tab deletion (gap detection) ---
echo "--- 22. probe_indices with gap ---"
close_extra_tabs
PANE_ID=$(discover_pane_id)

# Create 3 tabs: G1, G2, G3
pipe_cmd "{\"pane_id\":\"$PANE_ID\",\"action\":\"set_name\",\"name\":\"G1\"}"
pipe_cmd "{\"pane_id\":\"$PANE_ID\",\"action\":\"clear_status\"}"
wait_for_name "$PANE_ID" "G1" "tab1 named G1"

zellij action new-tab
wait_for_tab_count 2
PANE_G2=$(discover_pane_id)
pipe_cmd "{\"pane_id\":\"$PANE_G2\",\"action\":\"set_name\",\"name\":\"G2\"}"
wait_for_name "$PANE_G2" "G2" "tab2 named G2"

zellij action new-tab
wait_for_tab_count 3
PANE_G3=$(discover_pane_id)
pipe_cmd "{\"pane_id\":\"$PANE_G3\",\"action\":\"set_name\",\"name\":\"G3\"}"
wait_for_name "$PANE_G3" "G3" "tab3 named G3"

# Delete G2 (middle) — creates a gap in persistent indices
zellij action go-to-tab 2
sleep 0.3
zellij action close-tab
wait_for_tab_count 2

# Now indices should be [1, 3] (gap at 2)
# Re-probe to discover real indices
result=$(pipe_cmd "{\"action\":\"probe_indices\"}")
assert_eq "$result" "probing started" "probe_indices started"

# Wait for probing to complete
sleep 5

# Names should be restored
wait_for_name "$PANE_ID" "G1" "G1 name restored after gap probing"
wait_for_name "$PANE_G3" "G3" "G3 name restored after gap probing"

# Verify set_status works on correct tabs after probing
pipe_cmd "{\"pane_id\":\"$PANE_G3\",\"action\":\"set_status\",\"emoji\":\"🎯\"}"
wait_for_tab_contains "🎯 G3" "G3 has 🎯 after gap probing"

tab_names=$(zellij action query-tab-names)
assert_not_contains "$tab_names" "🎯 G1" "G1 does not have 🎯"
```

**Step 2: Прогнать интеграционные тесты**

Run: `make test-integration`
Expected: все 22 теста проходят

**Step 3: Commit**

```bash
git add scripts/integration-test.sh
git commit -m "test: add probe_indices gap detection test (Test 22)"
```

---

### Task 9: Финальная проверка и обновление документации

**Files:**
- Modify: `CLAUDE.md` — обновить секцию про probing
- Run: все тесты

**Step 1: Прогнать все тесты**

```bash
make test              # unit-тесты
make test-integration  # все 22 теста
```

Expected: все тесты проходят

**Step 2: Обновить CLAUDE.md**

В секции State Management добавить:
- `phase: Phase` — текущая фаза плагина (Probing или Ready)
- Описание probing: при первом TabUpdate плагин пробует `rename_tab(candidate, "⍟")` для определения persistent indices
- `probe_indices` command в списке Pipe Commands

В секции Testing обновить количество тестов.

**Step 3: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: update CLAUDE.md with probing phase documentation"
```
