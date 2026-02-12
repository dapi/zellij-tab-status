# План переноса zellij-tab-rename → zellij-tab-status

**Дата:** 2026-02-12
**Статус:** В работе

## Контекст

Перенос Rust WASM плагина для Zellij из marketplace (`claude-code-marketplace/zellij-tab-rename/`) в отдельный репозиторий (`zellij-tab-status`).

## Решения

- **Переименование:** `zellij-tab-rename` → `zellij-tab-status`
- **История:** Начинаем с чистого коммита (без переноса истории)
- **Marketplace:** Полное удаление директории `zellij-tab-rename/`

## Чеклист выполнения

### 1. Новый репозиторий (`/home/danil/code/zellij-tab-status`)

- [ ] Скопировать файлы из `zellij-tab-rename/`
- [ ] Переименовать пакет в `Cargo.toml` → `name = "zellij-tab-status"`
- [ ] Обновить `Makefile` → `PLUGIN_NAME = zellij-tab-status`
- [ ] Обновить логи в `main.rs`: `[tab-rename]` → `[tab-status]`
- [ ] Написать расширенный `README.md` с примерами
- [ ] Создать `CLAUDE.md` для проекта
- [ ] `git add . && git commit -m "Initial commit" && git push -u origin master`

### 2. Проверка сборки

- [ ] `make build` — собирается без ошибок
- [ ] `make install` — устанавливается в `~/.config/zellij/plugins/`

### 3. Marketplace (`/home/danil/code/claude-code-marketplace`)

- [ ] `git rm -r zellij-tab-rename/`
- [ ] Обновить `zellij-tab-claude-status/README.md` — добавить Requirements с ссылкой
- [ ] Коммит: `Remove zellij-tab-rename: moved to github.com/dapi/zellij-tab-status`

### 4. Финальная проверка

- [ ] Перезапустить Zellij
- [ ] Проверить работу Claude Code плагина с новым `zellij-tab-status`

## Файлы для переноса

```
zellij-tab-rename/
├── Cargo.toml          → переименовать пакет
├── Cargo.lock          → скопировать
├── Makefile            → обновить PLUGIN_NAME
├── README.md           → переписать с примерами
├── src/main.rs         → обновить логи
└── test-plugin.sh      → скопировать
```

## Структура README

1. Features
2. Installation (build, make install, config.kdl)
3. Usage Examples
   - Basic status management (🤖 ⏳ ❌ ✅)
   - Scripting examples
   - Claude Code integration
4. API Reference (tab-rename, tab-status pipes)
5. Troubleshooting
