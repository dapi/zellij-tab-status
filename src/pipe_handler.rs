use std::collections::BTreeMap;

use serde::Deserialize;

use crate::status_utils::{extract_base_name, extract_status};

/// Side effects returned by pure handlers, executed by main.rs via Zellij API calls
#[derive(Debug, PartialEq)]
pub enum PipeEffect {
    RenameTab { tab_position: usize, name: String },
    PipeOutput { output: String },
}

#[derive(Debug, Deserialize)]
pub struct StatusPayload {
    #[serde(default)]
    pub pane_id: String,
    pub action: String,
    #[serde(default)]
    pub emoji: String,
    #[serde(default)]
    pub name: String,
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

pub fn handle_status(pane_to_tab: &mut PaneTabMap, payload: &Option<String>) -> Vec<PipeEffect> {
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

    // Actions that don't require pane context
    if status.action == "get_version" {
        let version = env!("CARGO_PKG_VERSION");
        eprintln!("[tab-status] get_version: '{}'", version);
        return vec![PipeEffect::PipeOutput {
            output: version.to_string(),
        }];
    }

    let Some(pane_id) = parse_pane_id(&status.pane_id, "tab-status") else {
        return vec![];
    };

    let Some((tab_position, current_name)) = get_tab_info(pane_to_tab, pane_id, "tab-status")
    else {
        return vec![];
    };
    let current_name = current_name.clone();

    let base_name = extract_base_name(&current_name);

    match status.action.as_str() {
        "set_status" => {
            if status.emoji.is_empty() {
                eprintln!("[tab-status] ERROR: emoji is required for 'set_status' action");
                return vec![];
            }
            let new_name = format!("{} {}", status.emoji, base_name);
            eprintln!(
                "[tab-status] set_status on tab position {}: '{}' -> '{}'",
                tab_position, current_name, new_name
            );
            let effects = vec![PipeEffect::RenameTab {
                tab_position,
                name: new_name.clone(),
            }];
            update_cached_name(pane_to_tab, pane_id, new_name);
            effects
        }
        "clear_status" => {
            let new_name = base_name.to_string();
            eprintln!(
                "[tab-status] clear_status on tab position {}: '{}' -> '{}'",
                tab_position, current_name, new_name
            );
            let effects = vec![PipeEffect::RenameTab {
                tab_position,
                name: new_name.clone(),
            }];
            update_cached_name(pane_to_tab, pane_id, new_name);
            effects
        }
        "get_status" => {
            let emoji = extract_status(&current_name);
            eprintln!("[tab-status] get_status: '{}'", emoji);
            vec![PipeEffect::PipeOutput {
                output: emoji.to_string(),
            }]
        }
        "get_name" => {
            eprintln!("[tab-status] get_name: '{}'", base_name);
            vec![PipeEffect::PipeOutput {
                output: base_name.to_string(),
            }]
        }
        "set_name" => {
            if status.name.is_empty() {
                eprintln!("[tab-status] ERROR: name is required for 'set_name' action");
                return vec![];
            }
            let current_status = extract_status(&current_name);
            let new_name = if current_status.is_empty() {
                status.name.clone()
            } else {
                format!("{} {}", current_status, status.name)
            };
            eprintln!(
                "[tab-status] set_name on tab position {}: '{}' -> '{}'",
                tab_position, current_name, new_name
            );
            let effects = vec![PipeEffect::RenameTab {
                tab_position,
                name: new_name.clone(),
            }];
            update_cached_name(pane_to_tab, pane_id, new_name);
            effects
        }
        _ => {
            eprintln!(
                "[tab-status] ERROR: unknown action '{}'. Use 'set_status', 'clear_status', 'get_status', 'get_name', 'set_name', 'get_version', or 'get_debug'",
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
        );
        assert_eq!(
            effects,
            vec![PipeEffect::RenameTab {
                tab_position: 0,
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
        );
        assert_eq!(map.get(&1).unwrap().1, "✅ Work");
    }

    #[test]
    fn set_status_replaces_existing_status() {
        let mut map = make_map(&[(1, 0, "🤖 Work")]);
        let effects = handle_status(
            &mut map,
            &payload(r#"{"pane_id":"1","action":"set_status","emoji":"✅"}"#),
        );
        assert_eq!(
            effects,
            vec![PipeEffect::RenameTab {
                tab_position: 0,
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
        );
        assert_eq!(
            effects,
            vec![PipeEffect::RenameTab {
                tab_position: 0,
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
        );
        assert_eq!(map.get(&1).unwrap().1, "Work");
    }

    #[test]
    fn clear_status_on_plain_name_still_renames() {
        let mut map = make_map(&[(1, 0, "Work")]);
        let effects = handle_status(
            &mut map,
            &payload(r#"{"pane_id":"1","action":"clear_status"}"#),
        );
        assert_eq!(
            effects,
            vec![PipeEffect::RenameTab {
                tab_position: 0,
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
        );
        assert_eq!(
            effects,
            vec![PipeEffect::PipeOutput {
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
        );
        assert_eq!(effects, vec![PipeEffect::PipeOutput { output: "".into() }]);
    }

    // ==================== handle_status: get_name ====================

    #[test]
    fn get_name_returns_base_name() {
        let mut map = make_map(&[(1, 0, "🤖 Work")]);
        let effects = handle_status(&mut map, &payload(r#"{"pane_id":"1","action":"get_name"}"#));
        assert_eq!(
            effects,
            vec![PipeEffect::PipeOutput {
                output: "Work".into()
            }]
        );
    }

    // ==================== handle_status: get_version ====================

    #[test]
    fn get_version_returns_cargo_pkg_version() {
        let mut map = make_map(&[(1, 0, "Work")]);
        let effects = handle_status(
            &mut map,
            &payload(r#"{"pane_id":"1","action":"get_version"}"#),
        );
        assert_eq!(
            effects,
            vec![PipeEffect::PipeOutput {
                output: env!("CARGO_PKG_VERSION").into()
            }]
        );
    }

    // ==================== handle_status: set_name ====================

    #[test]
    fn set_name_preserves_emoji_status() {
        let mut map = make_map(&[(1, 0, "🤖 Work")]);
        let effects = handle_status(
            &mut map,
            &payload(r#"{"pane_id":"1","action":"set_name","name":"Code"}"#),
        );
        assert_eq!(
            effects,
            vec![PipeEffect::RenameTab {
                tab_position: 0,
                name: "🤖 Code".into()
            }]
        );
    }

    #[test]
    fn set_name_without_status_sets_plain_name() {
        let mut map = make_map(&[(1, 0, "Work")]);
        let effects = handle_status(
            &mut map,
            &payload(r#"{"pane_id":"1","action":"set_name","name":"Code"}"#),
        );
        assert_eq!(
            effects,
            vec![PipeEffect::RenameTab {
                tab_position: 0,
                name: "Code".into()
            }]
        );
    }

    #[test]
    fn set_name_updates_cache() {
        let mut map = make_map(&[(1, 0, "🤖 Work")]);
        handle_status(
            &mut map,
            &payload(r#"{"pane_id":"1","action":"set_name","name":"Code"}"#),
        );
        assert_eq!(map.get(&1).unwrap().1, "🤖 Code");
    }

    #[test]
    fn set_name_empty_name_returns_no_effects() {
        let mut map = make_map(&[(1, 0, "Work")]);
        let effects = handle_status(
            &mut map,
            &payload(r#"{"pane_id":"1","action":"set_name","name":""}"#),
        );
        assert_eq!(effects, vec![]);
    }

    #[test]
    fn set_name_missing_name_field_returns_no_effects() {
        let mut map = make_map(&[(1, 0, "Work")]);
        let effects = handle_status(&mut map, &payload(r#"{"pane_id":"1","action":"set_name"}"#));
        assert_eq!(effects, vec![]);
    }

    // ==================== handle_status: error paths ====================

    #[test]
    fn missing_payload_returns_no_effects() {
        let mut map = make_map(&[(1, 0, "Work")]);
        let effects = handle_status(&mut map, &None);
        assert_eq!(effects, vec![]);
    }

    #[test]
    fn invalid_json_returns_no_effects() {
        let mut map = make_map(&[(1, 0, "Work")]);
        let effects = handle_status(&mut map, &payload("not json"));
        assert_eq!(effects, vec![]);
    }

    #[test]
    fn unknown_pane_returns_no_effects() {
        let mut map = make_map(&[(1, 0, "Work")]);
        let effects = handle_status(
            &mut map,
            &payload(r#"{"pane_id":"999","action":"set_status","emoji":"🤖"}"#),
        );
        assert_eq!(effects, vec![]);
    }

    #[test]
    fn invalid_pane_id_returns_no_effects() {
        let mut map = make_map(&[(1, 0, "Work")]);
        let effects = handle_status(
            &mut map,
            &payload(r#"{"pane_id":"abc","action":"set_status","emoji":"🤖"}"#),
        );
        assert_eq!(effects, vec![]);
    }

    #[test]
    fn unknown_action_returns_no_effects() {
        let mut map = make_map(&[(1, 0, "Work")]);
        let effects = handle_status(&mut map, &payload(r#"{"pane_id":"1","action":"destroy"}"#));
        assert_eq!(effects, vec![]);
    }

    // ==================== tab_position passthrough ====================

    #[test]
    fn tab_position_passed_through() {
        let mut map = make_map(&[(5, 2, "Tab3")]);
        let effects = handle_status(
            &mut map,
            &payload(r#"{"pane_id":"5","action":"set_status","emoji":"🔥"}"#),
        );
        assert_eq!(
            effects,
            vec![PipeEffect::RenameTab {
                tab_position: 2,
                name: "🔥 Tab3".into()
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
        );
        assert_eq!(map.get(&1).unwrap().1, "🤖 Work");
    }

    #[test]
    fn get_name_does_not_mutate_cache() {
        let mut map = make_map(&[(1, 0, "🤖 Work")]);
        handle_status(&mut map, &payload(r#"{"pane_id":"1","action":"get_name"}"#));
        assert_eq!(map.get(&1).unwrap().1, "🤖 Work");
    }

    #[test]
    fn error_paths_do_not_mutate_cache() {
        let mut map = make_map(&[(1, 0, "Work")]);
        let original = map.clone();

        handle_status(
            &mut map,
            &payload(r#"{"pane_id":"abc","action":"set_status","emoji":"🤖"}"#),
        );
        assert_eq!(map, original, "cache must not change on invalid pane_id");

        handle_status(&mut map, &payload(r#"{"pane_id":"1","action":"destroy"}"#));
        assert_eq!(map, original, "cache must not change on unknown action");
    }

    // ==================== additional edge cases ====================

    #[test]
    fn set_status_missing_emoji_field_returns_no_effects() {
        let mut map = make_map(&[(1, 0, "Work")]);
        let effects = handle_status(
            &mut map,
            &payload(r#"{"pane_id":"1","action":"set_status"}"#),
        );
        assert_eq!(effects, vec![]);
    }

    #[test]
    fn get_name_returns_full_name_when_no_status() {
        let mut map = make_map(&[(1, 0, "Work")]);
        let effects = handle_status(&mut map, &payload(r#"{"pane_id":"1","action":"get_name"}"#));
        assert_eq!(
            effects,
            vec![PipeEffect::PipeOutput {
                output: "Work".into()
            }]
        );
    }
}
