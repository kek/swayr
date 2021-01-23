use crate::ipc;
use crate::util;
use std::cmp;
use std::collections::HashMap;
use std::fmt;
use std::os::unix::net::UnixStream;

pub fn get_tree() -> ipc::Node {
    let output = util::swaymsg(&["-t", "get_tree"]);
    let result = serde_json::from_str(output.as_str());

    match result {
        Ok(node) => node,
        Err(e) => {
            eprintln!("Error: {}", e);
            panic!()
        }
    }
}

#[test]
fn test_get_tree() {
    let tree = get_tree();

    println!("Those IDs are in get_tree():");
    for n in tree.iter() {
        println!("  id: {}, type: {:?}", n.id, n.r#type);
    }
}

#[derive(Debug)]
pub struct Window<'a> {
    node: &'a ipc::Node,
    workspace: &'a ipc::Node,
    con_props: Option<ipc::ConProps>,
}

impl Window<'_> {
    pub fn get_id(&self) -> &ipc::Id {
        &self.node.id
    }

    pub fn get_app_name(&self) -> &str {
        if let Some(app_id) = &self.node.app_id {
            app_id
        } else if let Some(wp_class) = self
            .node
            .window_properties
            .as_ref()
            .and_then(|wp| wp.class.as_ref())
        {
            wp_class
        } else {
            "<Unknown>"
        }
    }

    pub fn get_title(&self) -> &str {
        self.node.name.as_ref().unwrap()
    }
}

impl PartialEq for Window<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.get_id() == other.get_id()
    }
}

impl Eq for Window<'_> {}

impl Ord for Window<'_> {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        if self == other {
            cmp::Ordering::Equal
        } else if self.node.urgent && !other.node.urgent
            || !self.node.focused && other.node.focused
        {
            cmp::Ordering::Less
        } else if !self.node.urgent && other.node.urgent
            || self.node.focused && !other.node.focused
        {
            std::cmp::Ordering::Greater
        } else {
            let lru_a =
                self.con_props.as_ref().map_or(0, |wp| wp.last_focus_time);
            let lru_b =
                other.con_props.as_ref().map_or(0, |wp| wp.last_focus_time);
            lru_a.cmp(&lru_b).reverse()
        }
    }
}

impl PartialOrd for Window<'_> {
    fn partial_cmp(&self, other: &Window) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<'a> fmt::Display for Window<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(
            f,
            "<span font_weight=\"bold\" {}>“{}”</span>   \
             <i>{}</i>   \
             on workspace <b>{}</b>   \
             <span alpha=\"20000\">id {}</span>", // Almost hide ID!
            if self.node.urgent {
                " background=\"darkred\" foreground=\"white\""
            } else {
                ""
            },
            self.get_title(),
            self.get_app_name(),
            self.workspace.name.as_ref().unwrap(),
            self.get_id()
        )
    }
}

fn build_windows(
    root: &ipc::Node,
    mut con_props: HashMap<ipc::Id, ipc::ConProps>,
) -> Vec<Window> {
    let mut v = vec![];
    for workspace in root.workspaces() {
        for n in workspace.windows() {
            v.push(Window {
                node: &n,
                con_props: con_props.remove(&n.id),
                workspace: &workspace,
            })
        }
    }
    v
}

fn build_workspaces(
    root: &ipc::Node,
    mut con_props: HashMap<ipc::Id, ipc::ConProps>,
    include_scratchpad: bool,
) -> Vec<Workspace> {
    let mut v = vec![];
    for workspace in root.workspaces() {
        if !include_scratchpad
            && workspace.name.as_ref().unwrap().eq("__i3_scratch")
        {
            continue;
        }
        v.push(Workspace {
            node: &workspace,
            con_props: con_props.remove(&workspace.id),
            windows: workspace
                .windows()
                .iter()
                .map(|w| Window {
                    node: &w,
                    con_props: con_props.remove(&w.id),
                    workspace: &workspace,
                })
                .collect(),
        })
    }
    v
}

fn get_con_props() -> Result<HashMap<ipc::Id, ipc::ConProps>, serde_json::Error>
{
    if let Ok(sock) = UnixStream::connect(util::get_swayr_socket_path()) {
        serde_json::from_reader(sock)
    } else {
        panic!("Could not connect to socket!")
    }
}

/// Gets all application windows of the tree.
pub fn get_windows(root: &ipc::Node) -> Vec<Window> {
    let con_props = match get_con_props() {
        Ok(con_props) => Some(con_props),
        Err(e) => {
            eprintln!("Got no con_props: {:?}", e);
            None
        }
    };

    build_windows(root, con_props.unwrap_or_default())
}

/// Gets all application windows of the tree.
pub fn get_workspaces(
    root: &ipc::Node,
    include_scratchpad: bool,
) -> Vec<Workspace> {
    let con_props = match get_con_props() {
        Ok(con_props) => Some(con_props),
        Err(e) => {
            eprintln!("Got no con_props: {:?}", e);
            None
        }
    };

    build_workspaces(root, con_props.unwrap_or_default(), include_scratchpad)
}

#[test]
fn test_get_windows() {
    let root = get_tree();
    let cons = get_windows(&root);

    println!("There are {} cons.", cons.len());

    for c in cons {
        println!("  {}", c);
    }
}

pub fn select_window<'a>(
    prompt: &'a str,
    windows: &'a [Window],
) -> Option<&'a Window<'a>> {
    util::wofi_select(prompt, windows)
}

pub fn select_workspace<'a>(
    prompt: &'a str,
    workspaces: &'a [Workspace],
) -> Option<&'a Workspace<'a>> {
    util::wofi_select(prompt, workspaces)
}

pub struct Workspace<'a> {
    node: &'a ipc::Node,
    con_props: Option<ipc::ConProps>,
    windows: Vec<Window<'a>>,
}

impl Workspace<'_> {
    pub fn get_name(&self) -> &str {
        self.node.name.as_ref().unwrap()
    }

    pub fn get_id(&self) -> &ipc::Id {
        &self.node.id
    }
}

impl PartialEq for Workspace<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.get_id() == other.get_id()
    }
}

impl Eq for Workspace<'_> {}

impl Ord for Workspace<'_> {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        if self == other {
            cmp::Ordering::Equal
        } else {
            let lru_a =
                self.con_props.as_ref().map_or(0, |wp| wp.last_focus_time);
            let lru_b =
                other.con_props.as_ref().map_or(0, |wp| wp.last_focus_time);
            lru_a.cmp(&lru_b).reverse()
        }
    }
}

impl PartialOrd for Workspace<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<'a> fmt::Display for Workspace<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(
            f,
            "<span font_weight=\"bold\">“Workspace {}”</span>   \
             <span alpha=\"20000\">id {}</span>", // Almost hide ID!
            self.get_name(),
            self.get_id()
        )
    }
}