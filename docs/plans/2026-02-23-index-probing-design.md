# Design: Index Probing — определение persistent tab indices при загрузке

**Дата:** 2026-02-23
**Статус:** Утверждён

## Проблема

При первом `TabUpdate` плагин предполагает `tab_indices = [1, 2, ..., N]`. Это неверно если до загрузки плагина были удалены табы — persistent indices не переиспользуются (Zellij bug #3535).

`TabInfo` не содержит persistent index. Единственный способ его определить — вызвать `rename_tab(index, marker)` и посмотреть какой таб изменил имя.

## Решение: Sequential probing

### Маркер

Символ `⍟` (U+235F, APL star diaeresis) — монохромный, незаметный, не используется как обычный статус.

### Фазы плагина

```rust
enum Phase {
    Probing(ProbingState),
    Ready,
}

struct ProbingState {
    original_names: Vec<String>,  // имена табов до probing
    candidate: u32,               // текущий пробуемый индекс
    found: Vec<(usize, u32)>,     // (position, index) — найденные
    remaining: usize,             // сколько ещё не найдено
    restoring: bool,              // true = ждём восстановления имени
}
```

### Алгоритм

При первом `TabUpdate` (N табов):
1. Сохранить `original_names` из `TabInfo.name`
2. Перейти в `Phase::Probing { candidate: 1, remaining: N, restoring: false }`
3. Отправить `rename_tab(1, "⍟")`

На каждый `TabUpdate` в фазе Probing:

**Если `restoring == false`** (ищем маркер):
- Сканируем табы — ищем `name == "⍟"`
- **Нашли** на position P:
  - Записываем `found.push((P, candidate))`, `remaining -= 1`
  - Восстанавливаем: `rename_tab(candidate, original_names[P])`
  - `restoring = true`
- **Не нашли** → index `candidate` не существует (удалён):
  - `candidate += 1`
  - Отправляем `rename_tab(candidate, "⍟")`

**Если `restoring == true`** (ждём восстановления имени):
- Проверяем что маркер `⍟` исчез (имя восстановлено)
- `restoring = false`, `candidate += 1`
- Если `remaining > 0`: отправляем `rename_tab(candidate, "⍟")`
- Если `remaining == 0`: probing завершён → собрать `tab_indices` из `found`, перейти в `Phase::Ready`

### Защита от зацикливания

Если `candidate > N * 3` и `remaining > 0` — fallback на `[1..N]` (как сейчас), лог предупреждения.

### Блокировка pipe-команд

Во время `Phase::Probing` все pipe-команды (кроме `get_version` и `get_debug`) возвращают пустую строку + лог `"[tab-status] probing in progress, try again later"`. `unblock_cli_pipe_input` вызывается чтобы CLI не зависал.

### Команда `probe_indices`

Новый action `"probe_indices"` (без `pane_id`, как `get_version`):
- Переводит плагин обратно в `Phase::Probing` с текущими табами
- Полезно для диагностики и после подозрения на рассинхронизацию
- Возвращает `"probing started"` через PipeOutput

### Восстановление имён

`original_names` берётся из `TabInfo.name` на момент входа в probing. После обнаружения маркера на position P — `rename_tab(candidate, original_names[P])` восстанавливает оригинальное имя через тот же persistent index.

## Roundtrip-оценка

- Найденный индекс: 2 TabUpdate (probe + restore)
- Gap (удалённый индекс): 1 TabUpdate (probe, ничего не нашли)
- N табов, G gaps: `2*N + G` roundtrip'ов
- Типично (5 табов, 0 gaps): ~10 roundtrip'ов, ~200-400мс

## Ограничение

Если табы удалялись ДО первой загрузки плагина в сессии, probing корректно определит индексы. Но если плагин перезагружается (`probe_indices`) после длительной работы с большим количеством удалений, верхняя граница `N*3` может быть недостаточной. На практике это крайне маловероятно.

## Файлы

| Файл | Изменения |
|-|-|
| `src/main.rs` | +`Phase` enum, +`ProbingState`, переписать `update_tab_indices` init branch, probing FSM в `update()`, блокировка pipe в `pipe()`, handle `probe_indices` |
| `scripts/integration-test.sh` | +тест: probing при старте (проверить что tab_indices корректны через `get_debug`) |
