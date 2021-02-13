//! Functions and data structures of the swayr client.

use crate::con;
use crate::util;

use clap::Clap;
use std::fmt;

use swayipc as s;
use swayipc::reply as r;

#[derive(Clap, Debug)]
pub enum SwayrCommand {
    /// Switch to next urgent window (if any) or to last recently used window.
    SwitchToUrgentOrLRUWindow,
    /// Focus the selected window
    SwitchWindow,
    /// Focus the next window.
    NextWindow,
    /// Focus the previous window.
    PrevWindow,
    /// Quit the selected window
    QuitWindow,
    /// Switch to the selected workspace
    SwitchWorkspace,
    /// Switch to the selected workspace or focus the selected window
    SwitchWorkspaceOrWindow,
    /// Quit all windows of selected workspace or the selected window
    QuitWorkspaceOrWindow,
    /// Select and execute a swaymsg command
    ExecuteSwaymsgCommand,
    /// Select and execute a swayr command
    ExecuteSwayrCommand,
}

impl fmt::Display for SwayrCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "<b>{:?}</b>", self)
    }
}

pub fn exec_swayr_cmd(cmd: &SwayrCommand) {
    match cmd {
        SwayrCommand::SwitchToUrgentOrLRUWindow => {
            switch_to_urgent_or_lru_window()
        }
        SwayrCommand::SwitchWindow => switch_window(),
        SwayrCommand::NextWindow => {
            focus_next_window_in_direction(Direction::Forward)
        }
        SwayrCommand::PrevWindow => {
            focus_next_window_in_direction(Direction::Backward)
        }
        SwayrCommand::QuitWindow => quit_window(),
        SwayrCommand::SwitchWorkspace => switch_workspace(),
        SwayrCommand::SwitchWorkspaceOrWindow => switch_workspace_or_window(),
        SwayrCommand::QuitWorkspaceOrWindow => quit_workspace_or_window(),
        SwayrCommand::ExecuteSwaymsgCommand => exec_swaymsg_command(),
        SwayrCommand::ExecuteSwayrCommand => {
            if let Some(c) = util::wofi_select(
                "Select swayr command",
                &[
                    SwayrCommand::ExecuteSwaymsgCommand,
                    SwayrCommand::QuitWindow,
                    SwayrCommand::QuitWorkspaceOrWindow,
                    SwayrCommand::SwitchWindow,
                    SwayrCommand::SwitchWorkspace,
                    SwayrCommand::SwitchWorkspaceOrWindow,
                    SwayrCommand::SwitchToUrgentOrLRUWindow,
                    SwayrCommand::NextWindow,
                    SwayrCommand::PrevWindow,
                ],
            ) {
                exec_swayr_cmd(c);
            }
        }
    }
}

fn focus_window_by_id(id: i64) {
    util::swaymsg(&[format!("[con_id={}]", id).as_str(), "focus"]);
}

fn quit_window_by_id(id: i64) {
    util::swaymsg(&[format!("[con_id={}]", id).as_str(), "kill"]);
}

fn get_tree() -> r::Node {
    match s::Connection::new() {
        Ok(mut con) => con.get_tree().expect("Got no root node"),
        Err(err) => panic!(err),
    }
}

pub fn switch_to_urgent_or_lru_window() {
    let root = get_tree();
    let windows = con::get_windows(&root, true);
    if let Some(win) = windows
        .iter()
        .find(|w| w.is_urgent())
        .or_else(|| windows.get(0))
    {
        println!("Switching to {}", win);
        focus_window_by_id(win.get_id())
    } else {
        println!("No window to switch to.")
    }
}

pub fn switch_window() {
    let root = get_tree();
    let windows = con::get_windows(&root, true);

    if let Some(window) = con::select_window("Switch to window", &windows) {
        focus_window_by_id(window.get_id())
    }
}

pub enum Direction {
    Backward,
    Forward,
}

pub fn focus_next_window_in_direction(dir: Direction) {
    let root = get_tree();
    let windows = con::get_windows(&root, false);

    if windows.len() < 2 {
        return;
    }

    let pred: Box<dyn Fn(&con::Window) -> bool> =
        if windows.iter().find(|w| w.is_focused()).is_none() {
            let last_focused_win_id =
                con::get_windows(&root, true).get(0).unwrap().get_id();
            Box::new(move |w| w.get_id() == last_focused_win_id)
        } else {
            Box::new(|w: &con::Window| w.is_focused())
        };

    let mut iter: Box<dyn Iterator<Item = &con::Window>> = match dir {
        Direction::Forward => Box::new(windows.iter().rev().cycle()),
        Direction::Backward => Box::new(windows.iter().cycle()),
    };

    loop {
        let win = iter.next().unwrap();
        if pred(win) {
            let win = iter.next().unwrap();
            focus_window_by_id(win.get_id());
            return;
        }
    }
}

pub fn switch_workspace() {
    let root = get_tree();
    let workspaces = con::get_workspaces(&root, false);

    if let Some(workspace) =
        con::select_workspace("Switch to workspace", &workspaces)
    {
        util::swaymsg(&["workspace", "number", workspace.get_name()]);
    }
}

pub fn switch_workspace_or_window() {
    let root = get_tree();
    let workspaces = con::get_workspaces(&root, false);
    let ws_or_wins = con::WsOrWin::from_workspaces(&workspaces);
    if let Some(ws_or_win) = con::select_workspace_or_window(
        "Select workspace or window",
        &ws_or_wins,
    ) {
        match ws_or_win {
            con::WsOrWin::Ws { ws } => {
                util::swaymsg(&["workspace", "number", ws.get_name()]);
            }
            con::WsOrWin::Win { win } => focus_window_by_id(win.get_id()),
        }
    }
}

pub fn quit_window() {
    let root = get_tree();
    let windows = con::get_windows(&root, true);

    if let Some(window) = con::select_window("Quit window", &windows) {
        quit_window_by_id(window.get_id())
    }
}

pub fn quit_workspace_or_window() {
    let root = get_tree();
    let workspaces = con::get_workspaces(&root, false);
    let ws_or_wins = con::WsOrWin::from_workspaces(&workspaces);
    if let Some(ws_or_win) =
        con::select_workspace_or_window("Quit workspace or window", &ws_or_wins)
    {
        match ws_or_win {
            con::WsOrWin::Ws { ws } => {
                for win in &ws.windows {
                    quit_window_by_id(win.get_id())
                }
            }
            con::WsOrWin::Win { win } => quit_window_by_id(win.get_id()),
        }
    }
}

fn get_swaymsg_commands<'a>() -> Vec<SwaymsgCmd<'a>> {
    let mut cmds = vec![];
    cmds.push(vec!["exit"]);
    cmds.push(vec!["floating", "toggle"]);
    cmds.push(vec!["focus", "child"]);
    cmds.push(vec!["focus", "parent"]);

    for b in &["none", "normal", "csd", "pixel"] {
        cmds.push(vec!["border", b]);
    }

    cmds.push(vec!["focus", "tiling"]);
    cmds.push(vec!["focus", "floating"]);
    cmds.push(vec!["focus", "mode_toggle"]);

    cmds.push(vec!["fullscreen", "toggle"]);

    for x in &["focus", "fullscreen", "open", "none", "visible"] {
        cmds.push(vec!["inhibit_idle", x])
    }

    for l in &["default", "splith", "splitv", "stacking", "tiling"] {
        cmds.push(vec!["layout", l])
    }

    cmds.push(vec!["reload"]);

    for e in &["enable", "disable"] {
        cmds.push(vec!["shortcuts", "inhibitor", e])
    }

    cmds.push(vec!["sticky", "toggle"]);

    for x in &["yes", "no", "always"] {
        cmds.push(vec!["focus_follows_mouse", x])
    }

    for x in &["smart", "urgent", "focus", "none"] {
        cmds.push(vec!["focus_on_window_activation", x])
    }

    for x in &["yes", "no", "force", "workspace"] {
        cmds.push(vec!["focus_wrapping", x])
    }

    for x in &[
        "none",
        "vertical",
        "horizontal",
        "both",
        "smart",
        "smart_no_gaps",
    ] {
        cmds.push(vec!["hide_edge_borders", x])
    }

    cmds.push(vec!["kill"]);

    for x in &["on", "no_gaps", "off"] {
        cmds.push(vec!["smart_borders", x])
    }

    for x in &["on", "off"] {
        cmds.push(vec!["smart_gaps", x])
    }

    for x in &["output", "container", "none"] {
        cmds.push(vec!["mouse_warping", x])
    }

    for x in &["smart", "ignore", "leave_fullscreen"] {
        cmds.push(vec!["popup_during_fullscreen", x])
    }

    for x in &["yes", "no"] {
        cmds.push(vec!["show_marks", x]);
        cmds.push(vec!["workspace_auto_back_and_forth", x]);
    }

    cmds.push(vec!["tiling_drag", "toggle"]);

    for x in &["left", "center", "right"] {
        cmds.push(vec!["title_align", x]);
    }

    for x in &["enable", "disable", "allow", "deny"] {
        cmds.push(vec!["urgent", x])
    }

    cmds.sort();

    cmds.iter()
        .map(|v| SwaymsgCmd { cmd: v.to_vec() })
        .collect()
}

struct SwaymsgCmd<'a> {
    cmd: Vec<&'a str>,
}

impl<'a> fmt::Display for SwaymsgCmd<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "<b>{}</b>", self.cmd.join(" "))
    }
}

pub fn exec_swaymsg_command() {
    let cmds = get_swaymsg_commands();
    let cmd = util::wofi_select("Execute swaymsg command", &cmds);
    if let Some(cmd) = cmd {
        util::swaymsg(&cmd.cmd);
    }
}
