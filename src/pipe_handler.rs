use std::collections::BTreeMap;

use serde::Deserialize;

use crate::status_utils::{extract_base_name, extract_status};

/// Side effects returned by pure handlers, executed by main.rs via Zellij API calls
#[derive(Debug, PartialEq)]
pub enum PipeEffect {
    RenameTab { tab_id: u32, name: String },
    PipeOutput { pipe_name: String, output: String },
}

#[derive(Debug, Deserialize)]
pub struct RenamePayload {
    pub pane_id: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct StatusPayload {
    pub pane_id: String,
    pub action: String,
    #[serde(default)]
    pub emoji: String,
}

/// Maps pane_id -> (tab_position, tab_name)
pub type PaneTabMap = BTreeMap<u32, (usize, String)>;

fn parse_pane_id(pane_id_str: &str, context: &str) -> Option<u32> {
    match pane_id_str.parse() {
        Ok(id) => Some(id),
        Err(e) => {
            eprintln!(
                "[{}] ERROR: pane_id must be a number, got '{}': {}",
                context, pane_id_str, e
            );
            None
        }
    }
}

fn get_tab_info<'a>(
    pane_to_tab: &'a PaneTabMap,
    pane_id: u32,
    context: &str,
) -> Option<(usize, &'a String)> {
    match pane_to_tab.get(&pane_id) {
        Some(&(tab_position, ref name)) => Some((tab_position, name)),
        None => {
            eprintln!(
                "[{}] ERROR: pane {} not found. Known panes: {:?}",
                context,
                pane_id,
                pane_to_tab.keys().collect::<Vec<_>>()
            );
            None
        }
    }
}

fn update_cached_name(pane_to_tab: &mut PaneTabMap, pane_id: u32, new_name: String) {
    if let Some((_, ref mut cached_name)) = pane_to_tab.get_mut(&pane_id) {
        *cached_name = new_name;
    }
}

pub fn handle_rename(pane_to_tab: &mut PaneTabMap, payload: &Option<String>) -> Vec<PipeEffect> {
    let Some(payload) = payload else {
        eprintln!("[tab-status] ERROR: missing payload");
        return vec![];
    };

    let rename: RenamePayload = match serde_json::from_str(payload) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[tab-status] ERROR: invalid JSON: {}", e);
            return vec![];
        }
    };

    let Some(pane_id) = parse_pane_id(&rename.pane_id, "tab-rename") else {
        return vec![];
    };

    eprintln!(
        "[tab-status] Looking for pane_id={} in {} mappings",
        pane_id,
        pane_to_tab.len()
    );

    let Some((tab_position, _)) = get_tab_info(pane_to_tab, pane_id, "tab-rename") else {
        return vec![];
    };

    let tab_id = (tab_position + 1) as u32;

    eprintln!(
        "[tab-status] Renaming tab {} (position {}) to '{}'",
        tab_id, tab_position, rename.name
    );

    let effects = vec![PipeEffect::RenameTab {
        tab_id,
        name: rename.name.clone(),
    }];
    update_cached_name(pane_to_tab, pane_id, rename.name);

    effects
}

pub fn handle_status(
    pane_to_tab: &mut PaneTabMap,
    payload: &Option<String>,
    pipe_name: &str,
) -> Vec<PipeEffect> {
    let Some(payload) = payload else {
        eprintln!("[tab-status] ERROR: missing payload");
        return vec![];
    };

    let status: StatusPayload = match serde_json::from_str(payload) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[tab-status] ERROR: invalid JSON: {}", e);
            return vec![];
        }
    };

    let Some(pane_id) = parse_pane_id(&status.pane_id, "tab-status") else {
        return vec![];
    };

    let Some((tab_position, current_name)) = get_tab_info(pane_to_tab, pane_id, "tab-status")
    else {
        return vec![];
    };
    let current_name = current_name.clone();

    let base_name = extract_base_name(&current_name);
    let tab_id = (tab_position + 1) as u32;

    match status.action.as_str() {
        "set_status" => {
            if status.emoji.is_empty() {
                eprintln!("[tab-status] ERROR: emoji is required for 'set_status' action");
                return vec![];
            }
            let new_name = format!("{} {}", status.emoji, base_name);
            eprintln!(
                "[tab-status] set_status on tab {} (position {}): '{}' -> '{}'",
                tab_id, tab_position, current_name, new_name
            );
            let effects = vec![PipeEffect::RenameTab {
                tab_id,
                name: new_name.clone(),
            }];
            update_cached_name(pane_to_tab, pane_id, new_name);
            effects
        }
        "clear_status" => {
            let new_name = base_name.to_string();
            eprintln!(
                "[tab-status] clear_status on tab {} (position {}): '{}' -> '{}'",
                tab_id, tab_position, current_name, new_name
            );
            let effects = vec![PipeEffect::RenameTab {
                tab_id,
                name: new_name.clone(),
            }];
            update_cached_name(pane_to_tab, pane_id, new_name);
            effects
        }
        "get_status" => {
            let emoji = extract_status(&current_name);
            eprintln!("[tab-status] get_status: '{}'", emoji);
            vec![PipeEffect::PipeOutput {
                pipe_name: pipe_name.to_string(),
                output: emoji.to_string(),
            }]
        }
        "get_name" => {
            eprintln!("[tab-status] get_name: '{}'", base_name);
            vec![PipeEffect::PipeOutput {
                pipe_name: pipe_name.to_string(),
                output: base_name.to_string(),
            }]
        }
        _ => {
            eprintln!(
                "[tab-status] ERROR: unknown action '{}'. Use 'set_status', 'clear_status', 'get_status', or 'get_name'",
                status.action
            );
            vec![]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_map(entries: &[(u32, usize, &str)]) -> PaneTabMap {
        entries
            .iter()
            .map(|&(pane_id, tab_pos, name)| (pane_id, (tab_pos, name.to_string())))
            .collect()
    }

    fn payload(json: &str) -> Option<String> {
        Some(json.to_string())
    }

    // ==================== handle_status: set_status ====================

    #[test]
    fn set_status_renames_tab_with_emoji_prefix() {
        let mut map = make_map(&[(1, 0, "Work")]);
        let effects = handle_status(
            &mut map,
            &payload(r#"{"pane_id":"1","action":"set_status","emoji":"🤖"}"#),
            "tab-status",
        );
        assert_eq!(
            effects,
            vec![PipeEffect::RenameTab {
                tab_id: 1,
                name: "🤖 Work".into()
            }]
        );
    }

    #[test]
    fn set_status_updates_cache() {
        let mut map = make_map(&[(1, 0, "Work")]);
        handle_status(
            &mut map,
            &payload(r#"{"pane_id":"1","action":"set_status","emoji":"✅"}"#),
            "tab-status",
        );
        assert_eq!(map.get(&1).unwrap().1, "✅ Work");
    }

    #[test]
    fn set_status_replaces_existing_status() {
        let mut map = make_map(&[(1, 0, "🤖 Work")]);
        let effects = handle_status(
            &mut map,
            &payload(r#"{"pane_id":"1","action":"set_status","emoji":"✅"}"#),
            "tab-status",
        );
        assert_eq!(
            effects,
            vec![PipeEffect::RenameTab {
                tab_id: 1,
                name: "✅ Work".into()
            }]
        );
    }

    #[test]
    fn set_status_empty_emoji_returns_no_effects() {
        let mut map = make_map(&[(1, 0, "Work")]);
        let effects = handle_status(
            &mut map,
            &payload(r#"{"pane_id":"1","action":"set_status","emoji":""}"#),
            "tab-status",
        );
        assert_eq!(effects, vec![]);
    }

    // ==================== handle_status: clear_status ====================

    #[test]
    fn clear_status_removes_emoji_prefix() {
        let mut map = make_map(&[(1, 0, "🤖 Work")]);
        let effects = handle_status(
            &mut map,
            &payload(r#"{"pane_id":"1","action":"clear_status"}"#),
            "tab-status",
        );
        assert_eq!(
            effects,
            vec![PipeEffect::RenameTab {
                tab_id: 1,
                name: "Work".into()
            }]
        );
    }

    #[test]
    fn clear_status_updates_cache() {
        let mut map = make_map(&[(1, 0, "🤖 Work")]);
        handle_status(
            &mut map,
            &payload(r#"{"pane_id":"1","action":"clear_status"}"#),
            "tab-status",
        );
        assert_eq!(map.get(&1).unwrap().1, "Work");
    }

    #[test]
    fn clear_status_on_plain_name_still_renames() {
        let mut map = make_map(&[(1, 0, "Work")]);
        let effects = handle_status(
            &mut map,
            &payload(r#"{"pane_id":"1","action":"clear_status"}"#),
            "tab-status",
        );
        assert_eq!(
            effects,
            vec![PipeEffect::RenameTab {
                tab_id: 1,
                name: "Work".into()
            }]
        );
    }

    // ==================== handle_status: get_status ====================

    #[test]
    fn get_status_returns_pipe_output_with_emoji() {
        let mut map = make_map(&[(1, 0, "🤖 Work")]);
        let effects = handle_status(
            &mut map,
            &payload(r#"{"pane_id":"1","action":"get_status"}"#),
            "tab-status",
        );
        assert_eq!(
            effects,
            vec![PipeEffect::PipeOutput {
                pipe_name: "tab-status".into(),
                output: "🤖".into()
            }]
        );
    }

    #[test]
    fn get_status_returns_empty_when_no_status() {
        let mut map = make_map(&[(1, 0, "Work")]);
        let effects = handle_status(
            &mut map,
            &payload(r#"{"pane_id":"1","action":"get_status"}"#),
            "tab-status",
        );
        assert_eq!(
            effects,
            vec![PipeEffect::PipeOutput {
                pipe_name: "tab-status".into(),
                output: "".into()
            }]
        );
    }

    // ==================== handle_status: get_name ====================

    #[test]
    fn get_name_returns_base_name() {
        let mut map = make_map(&[(1, 0, "🤖 Work")]);
        let effects = handle_status(
            &mut map,
            &payload(r#"{"pane_id":"1","action":"get_name"}"#),
            "tab-status",
        );
        assert_eq!(
            effects,
            vec![PipeEffect::PipeOutput {
                pipe_name: "tab-status".into(),
                output: "Work".into()
            }]
        );
    }

    // ==================== handle_status: error paths ====================

    #[test]
    fn missing_payload_returns_no_effects() {
        let mut map = make_map(&[(1, 0, "Work")]);
        let effects = handle_status(&mut map, &None, "tab-status");
        assert_eq!(effects, vec![]);
    }

    #[test]
    fn invalid_json_returns_no_effects() {
        let mut map = make_map(&[(1, 0, "Work")]);
        let effects = handle_status(&mut map, &payload("not json"), "tab-status");
        assert_eq!(effects, vec![]);
    }

    #[test]
    fn unknown_pane_returns_no_effects() {
        let mut map = make_map(&[(1, 0, "Work")]);
        let effects = handle_status(
            &mut map,
            &payload(r#"{"pane_id":"999","action":"set_status","emoji":"🤖"}"#),
            "tab-status",
        );
        assert_eq!(effects, vec![]);
    }

    #[test]
    fn invalid_pane_id_returns_no_effects() {
        let mut map = make_map(&[(1, 0, "Work")]);
        let effects = handle_status(
            &mut map,
            &payload(r#"{"pane_id":"abc","action":"set_status","emoji":"🤖"}"#),
            "tab-status",
        );
        assert_eq!(effects, vec![]);
    }

    #[test]
    fn unknown_action_returns_no_effects() {
        let mut map = make_map(&[(1, 0, "Work")]);
        let effects = handle_status(
            &mut map,
            &payload(r#"{"pane_id":"1","action":"destroy"}"#),
            "tab-status",
        );
        assert_eq!(effects, vec![]);
    }

    // ==================== handle_rename ====================

    #[test]
    fn rename_returns_rename_effect() {
        let mut map = make_map(&[(1, 0, "Work")]);
        let effects = handle_rename(&mut map, &payload(r#"{"pane_id":"1","name":"New Name"}"#));
        assert_eq!(
            effects,
            vec![PipeEffect::RenameTab {
                tab_id: 1,
                name: "New Name".into()
            }]
        );
    }

    #[test]
    fn rename_updates_cache() {
        let mut map = make_map(&[(1, 0, "Work")]);
        handle_rename(&mut map, &payload(r#"{"pane_id":"1","name":"New Name"}"#));
        assert_eq!(map.get(&1).unwrap().1, "New Name");
    }

    #[test]
    fn rename_missing_payload_returns_no_effects() {
        let mut map = make_map(&[(1, 0, "Work")]);
        let effects = handle_rename(&mut map, &None);
        assert_eq!(effects, vec![]);
    }

    #[test]
    fn rename_invalid_json_returns_no_effects() {
        let mut map = make_map(&[(1, 0, "Work")]);
        let effects = handle_rename(&mut map, &payload("{bad}"));
        assert_eq!(effects, vec![]);
    }

    #[test]
    fn rename_unknown_pane_returns_no_effects() {
        let mut map = make_map(&[(1, 0, "Work")]);
        let effects = handle_rename(&mut map, &payload(r#"{"pane_id":"999","name":"New"}"#));
        assert_eq!(effects, vec![]);
    }

    #[test]
    fn rename_invalid_pane_id_returns_no_effects() {
        let mut map = make_map(&[(1, 0, "Work")]);
        let effects = handle_rename(&mut map, &payload(r#"{"pane_id":"abc","name":"New"}"#));
        assert_eq!(effects, vec![]);
    }

    // ==================== tab_id calculation ====================

    #[test]
    fn tab_id_is_one_indexed() {
        let mut map = make_map(&[(5, 2, "Tab3")]);
        let effects = handle_status(
            &mut map,
            &payload(r#"{"pane_id":"5","action":"set_status","emoji":"🔥"}"#),
            "tab-status",
        );
        assert_eq!(
            effects,
            vec![PipeEffect::RenameTab {
                tab_id: 3, // position 2 + 1
                name: "🔥 Tab3".into()
            }]
        );
    }

    #[test]
    fn rename_tab_id_is_one_indexed() {
        let mut map = make_map(&[(5, 3, "Tab4")]);
        let effects = handle_rename(&mut map, &payload(r#"{"pane_id":"5","name":"Renamed"}"#));
        assert_eq!(
            effects,
            vec![PipeEffect::RenameTab {
                tab_id: 4, // position 3 + 1
                name: "Renamed".into()
            }]
        );
    }

    // ==================== pipe_name passthrough ====================

    #[test]
    fn pipe_output_uses_provided_pipe_name() {
        let mut map = make_map(&[(1, 0, "🤖 Work")]);
        let effects = handle_status(
            &mut map,
            &payload(r#"{"pane_id":"1","action":"get_status"}"#),
            "custom-pipe-name",
        );
        assert_eq!(
            effects,
            vec![PipeEffect::PipeOutput {
                pipe_name: "custom-pipe-name".into(),
                output: "🤖".into()
            }]
        );
    }

    // ==================== cache immutability ====================

    #[test]
    fn get_status_does_not_mutate_cache() {
        let mut map = make_map(&[(1, 0, "🤖 Work")]);
        handle_status(
            &mut map,
            &payload(r#"{"pane_id":"1","action":"get_status"}"#),
            "tab-status",
        );
        assert_eq!(map.get(&1).unwrap().1, "🤖 Work");
    }

    #[test]
    fn get_name_does_not_mutate_cache() {
        let mut map = make_map(&[(1, 0, "🤖 Work")]);
        handle_status(
            &mut map,
            &payload(r#"{"pane_id":"1","action":"get_name"}"#),
            "tab-status",
        );
        assert_eq!(map.get(&1).unwrap().1, "🤖 Work");
    }

    #[test]
    fn error_paths_do_not_mutate_cache() {
        let mut map = make_map(&[(1, 0, "Work")]);
        let original = map.clone();

        handle_status(
            &mut map,
            &payload(r#"{"pane_id":"abc","action":"set_status","emoji":"🤖"}"#),
            "tab-status",
        );
        assert_eq!(map, original, "cache must not change on invalid pane_id");

        handle_status(
            &mut map,
            &payload(r#"{"pane_id":"1","action":"destroy"}"#),
            "tab-status",
        );
        assert_eq!(map, original, "cache must not change on unknown action");
    }

    // ==================== additional edge cases ====================

    #[test]
    fn set_status_missing_emoji_field_returns_no_effects() {
        let mut map = make_map(&[(1, 0, "Work")]);
        let effects = handle_status(
            &mut map,
            &payload(r#"{"pane_id":"1","action":"set_status"}"#),
            "tab-status",
        );
        assert_eq!(effects, vec![]);
    }

    #[test]
    fn get_name_returns_full_name_when_no_status() {
        let mut map = make_map(&[(1, 0, "Work")]);
        let effects = handle_status(
            &mut map,
            &payload(r#"{"pane_id":"1","action":"get_name"}"#),
            "tab-status",
        );
        assert_eq!(
            effects,
            vec![PipeEffect::PipeOutput {
                pipe_name: "tab-status".into(),
                output: "Work".into()
            }]
        );
    }
}
