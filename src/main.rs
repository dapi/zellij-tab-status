use std::env;
use std::process;

use zellij_tab_status::tab_name;
use zellij_tab_status::zellij_api;

const HELP: &str = "\
zellij-tab-status - Manage status emoji in zellij tab name

Usage:
  zellij-tab-status                 Get current status (same as --get)
  zellij-tab-status <emoji>        Set status emoji
  zellij-tab-status --clear, -c    Remove status emoji
  zellij-tab-status --get, -g      Get current status emoji
  zellij-tab-status --get-status   Get current status emoji (alias)
  zellij-tab-status --name, -n     Get base name (without status)
  zellij-tab-status --set-name, -s <name>  Set tab name (preserving status)
  zellij-tab-status --version, -v  Show version
  zellij-tab-status --help, -h     Show this help

Options:
  --pane-id <id>    Use specific pane ID instead of $ZELLIJ_PANE_ID
  --tab-id <id>     Use specific tab ID directly (skip pane resolution)";

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    let mut pane_id_arg: Option<u32> = None;
    let mut tab_id_arg: Option<u32> = None;
    let mut command: Option<String> = None;
    let mut command_value: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => {
                println!("{}", HELP);
                process::exit(0);
            }
            "--version" | "-v" => {
                println!("{}", env!("CARGO_PKG_VERSION"));
                process::exit(0);
            }
            "--pane-id" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("Error: --pane-id requires a value");
                    process::exit(2);
                }
                pane_id_arg = Some(args[i].parse::<u32>().unwrap_or_else(|_| {
                    eprintln!("Error: --pane-id must be a non-negative integer");
                    process::exit(2);
                }));
            }
            "--tab-id" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("Error: --tab-id requires a value");
                    process::exit(2);
                }
                tab_id_arg = Some(args[i].parse::<u32>().unwrap_or_else(|_| {
                    eprintln!("Error: --tab-id must be a non-negative integer");
                    process::exit(2);
                }));
            }
            "--get" | "-g" | "--get-status" => {
                command = Some("get_status".to_string());
            }
            "--clear" | "-c" => {
                command = Some("clear_status".to_string());
            }
            "--name" | "-n" => {
                command = Some("get_name".to_string());
            }
            "--set-name" | "-s" => {
                command = Some("set_name".to_string());
                i += 1;
                if i >= args.len() {
                    eprintln!("Error: --set-name requires a value");
                    process::exit(2);
                }
                command_value = Some(args[i].clone());
            }
            other => {
                if other.starts_with('-') {
                    eprintln!("Error: unknown option '{}'", other);
                    process::exit(2);
                }
                // Positional argument = set_status with emoji
                command = Some("set_status".to_string());
                command_value = Some(other.to_string());
            }
        }
        i += 1;
    }

    // Default command: get_status
    let command = command.unwrap_or_else(|| "get_status".to_string());

    // Validate mutually exclusive options
    if pane_id_arg.is_some() && tab_id_arg.is_some() {
        eprintln!("Error: --pane-id and --tab-id are mutually exclusive");
        process::exit(2);
    }

    // Resolve tab_id
    let tab_id = resolve_tab_id(pane_id_arg, tab_id_arg);

    // Execute command
    match command.as_str() {
        "get_status" => {
            let name = get_current_tab_name(tab_id);
            let status = tab_name::get_status(&name);
            println!("{}", status);
        }
        "get_name" => {
            let name = get_current_tab_name(tab_id);
            let base = tab_name::get_name(&name);
            println!("{}", base);
        }
        "set_status" => {
            let emoji = command_value.expect("set_status requires emoji value");
            let name = get_current_tab_name(tab_id);
            let new_name = tab_name::set_status(&name, &emoji);
            if new_name != name {
                rename_tab(tab_id, &new_name);
            }
        }
        "clear_status" => {
            let name = get_current_tab_name(tab_id);
            let new_name = tab_name::clear_status(&name);
            if new_name != name {
                rename_tab(tab_id, &new_name);
            }
        }
        "set_name" => {
            let new_base = command_value.expect("set_name requires name value");
            let name = get_current_tab_name(tab_id);
            let new_name = tab_name::set_name(&name, &new_base);
            if new_name != name {
                rename_tab(tab_id, &new_name);
            }
        }
        _ => unreachable!(),
    }
}

fn resolve_tab_id(pane_id_arg: Option<u32>, tab_id_arg: Option<u32>) -> u32 {
    if let Some(tab_id) = tab_id_arg {
        return tab_id;
    }

    let pane_id = if let Some(id) = pane_id_arg {
        id
    } else {
        match env::var("ZELLIJ_PANE_ID") {
            Ok(val) => val.parse::<u32>().unwrap_or_else(|_| {
                eprintln!("Error: $ZELLIJ_PANE_ID is not a valid integer: '{}'", val);
                process::exit(2);
            }),
            Err(_) => {
                eprintln!("Error: $ZELLIJ_PANE_ID not set (not running inside Zellij?)");
                process::exit(2);
            }
        }
    };

    zellij_api::resolve_tab_id(pane_id).unwrap_or_else(|e| {
        eprintln!("Error: {}", e);
        process::exit(1);
    })
}

fn get_current_tab_name(tab_id: u32) -> String {
    zellij_api::get_tab_name(tab_id).unwrap_or_else(|e| {
        eprintln!("Error: {}", e);
        process::exit(1);
    })
}

fn rename_tab(tab_id: u32, new_name: &str) {
    zellij_api::rename_tab(tab_id, new_name).unwrap_or_else(|e| {
        eprintln!("Error: {}", e);
        process::exit(1);
    });
}
