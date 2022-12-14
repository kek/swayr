// Copyright (C) 2021-2022  Tassilo Horn <tsdh@gnu.org>
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the Free
// Software Foundation, either version 3 of the License, or (at your option)
// any later version.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
// FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License for
// more details.
//
// You should have received a copy of the GNU General Public License along with
// this program.  If not, see <https://www.gnu.org/licenses/>.

//! Convenience data structures built from the IPC structs.

use crate::config;
use crate::focus::FocusData;
use crate::shared::fmt::subst_placeholders;
use crate::shared::ipc;
use crate::shared::ipc::NodeMethods;
use crate::util;
use crate::util::DisplayFormat;
use once_cell::sync::Lazy;
use regex::Regex;
use std::cell::RefCell;
use std::cmp;
use std::collections::HashMap;
use std::rc::Rc;
use swayipc as s;

pub struct Tree<'a> {
    root: &'a s::Node,
    id_node: HashMap<i64, &'a s::Node>,
    id_parent: HashMap<i64, i64>,
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum IndentLevel {
    Fixed(usize),
    WorkspacesZeroWindowsOne,
    TreeDepth(usize),
}

pub struct DisplayNode<'a> {
    pub node: &'a s::Node,
    pub tree: &'a Tree<'a>,
    indent_level: IndentLevel,
}

impl<'a> Tree<'a> {
    fn get_node_by_id(&self, id: i64) -> &&s::Node {
        self.id_node
            .get(&id)
            .unwrap_or_else(|| panic!("No node with id {}", id))
    }

    fn get_parent_node(&self, id: i64) -> Option<&&s::Node> {
        self.id_parent.get(&id).map(|pid| self.get_node_by_id(*pid))
    }

    pub fn get_parent_node_of_type(
        &self,
        id: i64,
        t: ipc::Type,
    ) -> Option<&&s::Node> {
        let n = self.get_node_by_id(id);
        if n.get_type() == t {
            Some(n)
        } else if let Some(pid) = self.id_parent.get(&id) {
            self.get_parent_node_of_type(*pid, t)
        } else {
            None
        }
    }

    fn sorted_nodes_of_type_1(
        &self,
        node: &'a s::Node,
        t: ipc::Type,
        fdata: &FocusData,
    ) -> Vec<&s::Node> {
        let mut v: Vec<&s::Node> = node.nodes_of_type(t);
        self.sort_by_urgency_and_lru_time_1(&mut v, fdata);
        v
    }

    fn sorted_nodes_of_type(
        &self,
        t: ipc::Type,
        fdata: &FocusData,
    ) -> Vec<&s::Node> {
        self.sorted_nodes_of_type_1(self.root, t, fdata)
    }

    fn as_display_nodes(
        &self,
        v: &[&'a s::Node],
        indent_level: IndentLevel,
    ) -> Vec<DisplayNode> {
        v.iter()
            .map(|node| DisplayNode {
                node,
                tree: self,
                indent_level,
            })
            .collect()
    }

    pub fn get_current_workspace(&self) -> &s::Node {
        self.root
            .iter()
            .find(|n| n.get_type() == ipc::Type::Workspace && n.is_current())
            .expect("No current Workspace")
    }

    pub fn get_outputs(&self) -> Vec<DisplayNode> {
        let outputs: Vec<&s::Node> = self
            .root
            .iter()
            .filter(|n| n.get_type() == ipc::Type::Output && !n.is_scratchpad())
            .collect();
        self.as_display_nodes(&outputs, IndentLevel::Fixed(0))
    }

    pub fn get_workspaces(&self, fdata: &FocusData) -> Vec<DisplayNode> {
        let mut v = self.sorted_nodes_of_type(ipc::Type::Workspace, fdata);
        if !v.is_empty() {
            v.rotate_left(1);
        }
        self.as_display_nodes(&v, IndentLevel::Fixed(0))
    }

    pub fn get_windows(&self, fdata: &FocusData) -> Vec<DisplayNode> {
        let mut v = self.sorted_nodes_of_type(ipc::Type::Window, fdata);
        // Rotate, but only non-urgent windows.  Those should stay at the front
        // as they are the most likely switch candidates.
        let mut x;
        if !v.is_empty() {
            x = vec![];
            loop {
                if !v.is_empty() && v[0].urgent {
                    x.push(v.remove(0));
                } else {
                    break;
                }
            }
            if !v.is_empty() {
                v.rotate_left(1);
                x.append(&mut v);
            }
        } else {
            x = v;
        }
        self.as_display_nodes(&x, IndentLevel::Fixed(0))
    }

    pub fn get_workspaces_and_windows(
        &self,
        fdata: &FocusData,
    ) -> Vec<DisplayNode> {
        let workspaces = self.sorted_nodes_of_type(ipc::Type::Workspace, fdata);
        let mut first = true;
        let mut v = vec![];
        for ws in workspaces {
            v.push(ws);
            let mut wins =
                self.sorted_nodes_of_type_1(ws, ipc::Type::Window, fdata);
            if first && !wins.is_empty() {
                wins.rotate_left(1);
                first = false;
            }
            v.append(&mut wins);
        }

        self.as_display_nodes(&v, IndentLevel::WorkspacesZeroWindowsOne)
    }

    fn sort_by_urgency_and_lru_time_1(
        &self,
        v: &mut [&s::Node],
        fdata: &FocusData,
    ) {
        v.sort_by(|a, b| {
            if a.urgent && !b.urgent {
                cmp::Ordering::Less
            } else if !a.urgent && b.urgent {
                cmp::Ordering::Greater
            } else {
                let lru_a = fdata.last_focus_tick(a.id);
                let lru_b = fdata.last_focus_tick(b.id);
                lru_a.cmp(&lru_b).reverse()
            }
        });
    }

    fn push_subtree_sorted(
        &self,
        n: &'a s::Node,
        v: Rc<RefCell<Vec<&'a s::Node>>>,
        fdata: &FocusData,
    ) {
        v.borrow_mut().push(n);

        let mut children: Vec<&s::Node> = n.nodes.iter().collect();
        children.append(&mut n.floating_nodes.iter().collect());
        self.sort_by_urgency_and_lru_time_1(&mut children, fdata);

        for c in children {
            self.push_subtree_sorted(c, Rc::clone(&v), fdata);
        }
    }

    pub fn get_outputs_workspaces_containers_and_windows(
        &self,
        fdata: &FocusData,
    ) -> Vec<DisplayNode> {
        let outputs = self.sorted_nodes_of_type(ipc::Type::Output, fdata);
        let v: Rc<RefCell<Vec<&s::Node>>> = Rc::new(RefCell::new(vec![]));
        for o in outputs {
            self.push_subtree_sorted(o, Rc::clone(&v), fdata);
        }

        let x = self.as_display_nodes(&*v.borrow(), IndentLevel::TreeDepth(1));
        x
    }

    pub fn get_workspaces_containers_and_windows(
        &self,
        fdata: &FocusData,
    ) -> Vec<DisplayNode> {
        let workspaces = self.sorted_nodes_of_type(ipc::Type::Workspace, fdata);
        let v: Rc<RefCell<Vec<&s::Node>>> = Rc::new(RefCell::new(vec![]));
        for ws in workspaces {
            self.push_subtree_sorted(ws, Rc::clone(&v), fdata);
        }

        let x = self.as_display_nodes(&*v.borrow(), IndentLevel::TreeDepth(2));
        x
    }

    pub fn is_child_of_tiled_container(&self, id: i64) -> bool {
        match self.get_parent_node(id) {
            Some(n) => {
                n.layout == s::NodeLayout::SplitH
                    || n.layout == s::NodeLayout::SplitV
            }
            None => false,
        }
    }

    pub fn is_child_of_tabbed_or_stacked_container(&self, id: i64) -> bool {
        match self.get_parent_node(id) {
            Some(n) => {
                n.layout == s::NodeLayout::Tabbed
                    || n.layout == s::NodeLayout::Stacked
            }
            None => false,
        }
    }
}

fn init_id_parent<'a>(
    n: &'a s::Node,
    parent: Option<&'a s::Node>,
    id_node: &mut HashMap<i64, &'a s::Node>,
    id_parent: &mut HashMap<i64, i64>,
) {
    id_node.insert(n.id, n);

    if let Some(p) = parent {
        id_parent.insert(n.id, p.id);
    }

    for c in &n.nodes {
        init_id_parent(c, Some(n), id_node, id_parent);
    }
    for c in &n.floating_nodes {
        init_id_parent(c, Some(n), id_node, id_parent);
    }
}

pub fn get_tree(root: &s::Node) -> Tree {
    let mut id_node: HashMap<i64, &s::Node> = HashMap::new();
    let mut id_parent: HashMap<i64, i64> = HashMap::new();
    init_id_parent(root, None, &mut id_node, &mut id_parent);

    Tree {
        root,
        id_node,
        id_parent,
    }
}

static APP_NAME_AND_VERSION_RX: Lazy<Regex> =
    Lazy::new(|| Regex::new("(.+)(-[0-9.]+)").unwrap());

fn format_marks(marks: &[String]) -> String {
    if marks.is_empty() {
        "".to_string()
    } else {
        format!("[{}]", marks.join(", "))
    }
}

impl DisplayFormat for DisplayNode<'_> {
    fn format_for_display(&self, cfg: &config::Config) -> String {
        let indent = cfg.get_format_indent();
        let html_escape = cfg.get_format_html_escape();
        let urgency_start = cfg.get_format_urgency_start();
        let urgency_end = cfg.get_format_urgency_end();
        let icon_dirs = cfg.get_format_icon_dirs();
        // fallback_icon has no default value.
        let fallback_icon: Option<Box<std::path::Path>> = cfg
            .get_format_fallback_icon()
            .as_ref()
            .map(|i| std::path::Path::new(i).to_owned().into_boxed_path());

        let app_name_no_version =
            APP_NAME_AND_VERSION_RX.replace(self.node.get_app_name(), "$1");

        let fmt = match self.node.get_type() {
            ipc::Type::Root => String::from("Cannot format Root"),
            ipc::Type::Output => cfg.get_format_output_format(),
            ipc::Type::Workspace => cfg.get_format_workspace_format(),
            ipc::Type::Container => cfg.get_format_container_format(),
            ipc::Type::Window => cfg.get_format_window_format(),
        };
        let fmt = fmt
            .replace(
                "{indent}",
                indent.repeat(self.get_indent_level()).as_str(),
            )
            .replace(
                "{urgency_start}",
                if self.node.urgent {
                    urgency_start.as_str()
                } else {
                    ""
                },
            )
            .replace(
                "{urgency_end}",
                if self.node.urgent {
                    urgency_end.as_str()
                } else {
                    ""
                },
            )
            .replace(
                "{app_icon}",
                util::get_icon(self.node.get_app_name(), &icon_dirs)
                    .or_else(|| {
                        util::get_icon(&app_name_no_version, &icon_dirs)
                    })
                    .or_else(|| {
                        util::get_icon(
                            &app_name_no_version.to_lowercase(),
                            &icon_dirs,
                        )
                    })
                    .or(fallback_icon)
                    .map(|i| i.to_string_lossy().into_owned())
                    .unwrap_or_else(String::new)
                    .as_str(),
            );

        subst_placeholders!(&fmt, html_escape, {
            "id" => self.node.id,
            "app_name" => self.node.get_app_name(),
            "layout" => format!("{:?}", self.node.layout),
            "name" | "title" => self.node.get_name(),
            "output_name" => self
                .tree
                .get_parent_node_of_type(self.node.id, ipc::Type::Output)
                .map_or("<no_output>", |w| w.get_name()),
            "workspace_name" => self
                .tree
                .get_parent_node_of_type(self.node.id, ipc::Type::Workspace)
                .map_or("<no_workspace>", |w| w.get_name()),
            "marks" => format_marks(&self.node.marks),
        })
    }

    fn get_indent_level(&self) -> usize {
        match self.indent_level {
            IndentLevel::Fixed(level) => level as usize,
            IndentLevel::WorkspacesZeroWindowsOne => {
                match self.node.get_type(){
                    ipc::Type::Workspace => 0,
                    ipc::Type::Window => 1,
                    _ => panic!("Only Workspaces and Windows expected. File a bug report!")
                }
            }
            IndentLevel::TreeDepth(offset) => {
                let mut depth: usize = 0;
                let mut node = self.node;
                while let Some(p) = self.tree.get_parent_node(node.id) {
                    depth += 1;
                    node = p;
                }
                if offset > depth {
                    0
                } else {
                    depth - offset as usize
                }
            }
        }
    }
}
