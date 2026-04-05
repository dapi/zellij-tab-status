use std::process::Command;

/// Returns the zellij binary path: `$ZELLIJ_PATH` if set, otherwise `"zellij"`.
fn zellij_bin() -> String {
    std::env::var("ZELLIJ_PATH").unwrap_or_else(|_| "zellij".to_string())
}

#[derive(serde::Deserialize)]
struct PaneEntry {
    id: u32,
    tab_id: u32,
    #[serde(default)]
    is_plugin: bool,
}

#[derive(serde::Deserialize)]
struct TabEntry {
    tab_id: u32,
    name: String,
}

/// Resolve pane_id to tab_id via `zellij action list-panes --json`
pub fn resolve_tab_id(pane_id: u32) -> Result<u32, String> {
    let bin = zellij_bin();
    let output = Command::new(&bin)
        .args(["action", "list-panes", "--json"])
        .output()
        .map_err(|e| format!("Failed to run 'zellij action list-panes --json': {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "'zellij action list-panes --json' failed (exit {}): {}",
            output.status, stderr
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let panes: Vec<PaneEntry> =
        serde_json::from_str(&stdout).map_err(|e| format!("Failed to parse panes JSON: {}", e))?;

    // Prefer non-plugin panes (plugin pane IDs can overlap with terminal pane IDs)
    panes
        .iter()
        .find(|p| p.id == pane_id && !p.is_plugin)
        .or_else(|| panes.iter().find(|p| p.id == pane_id))
        .map(|p| p.tab_id)
        .ok_or_else(|| format!("Pane ID {} not found in list-panes output", pane_id))
}

/// Get tab name by tab_id via `zellij action list-tabs --json`
pub fn get_tab_name(tab_id: u32) -> Result<String, String> {
    let bin = zellij_bin();
    let output = Command::new(&bin)
        .args(["action", "list-tabs", "--json"])
        .output()
        .map_err(|e| format!("Failed to run 'zellij action list-tabs --json': {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "'zellij action list-tabs --json' failed (exit {}): {}",
            output.status, stderr
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let tabs: Vec<TabEntry> =
        serde_json::from_str(&stdout).map_err(|e| format!("Failed to parse tabs JSON: {}", e))?;

    tabs.iter()
        .find(|t| t.tab_id == tab_id)
        .map(|t| t.name.clone())
        .ok_or_else(|| format!("Tab ID {} not found in list-tabs output", tab_id))
}

/// Rename tab by id via `zellij action rename-tab-by-id <id> <name>`
pub fn rename_tab(tab_id: u32, new_name: &str) -> Result<(), String> {
    let bin = zellij_bin();
    let output = Command::new(&bin)
        .args(["action", "rename-tab-by-id", &tab_id.to_string(), new_name])
        .output()
        .map_err(|e| format!("Failed to run 'zellij action rename-tab-by-id': {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "'zellij action rename-tab-by-id {} \"{}\"' failed (exit {}): {}",
            tab_id, new_name, output.status, stderr
        ));
    }

    Ok(())
}
