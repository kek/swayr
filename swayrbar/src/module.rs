// Copyright (C) 2022  Tassilo Horn <tsdh@gnu.org>
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

use std::collections::HashMap;

use crate::config;
use swaybar_types as s;

pub mod battery;
pub mod date;
pub mod pactl;
pub mod sysinfo;
pub mod window;

pub type NameAndInstance = (String, String);

fn should_refresh(m: &dyn BarModuleFn, nai: &Option<NameAndInstance>) -> bool {
    let cfg = m.get_config();
    match nai {
        None => true,
        Some((n, i)) => n == &cfg.name && i == &cfg.instance,
    }
}

pub trait BarModuleFn: Sync + Send {
    fn create(config: config::ModuleConfig) -> Box<dyn BarModuleFn>
    where
        Self: Sized;
    fn default_config(instance: String) -> config::ModuleConfig
    where
        Self: Sized;
    fn get_config(&self) -> &config::ModuleConfig;
    fn get_on_click_map(
        &self,
        name: &str,
        instance: &str,
    ) -> Option<&HashMap<String, Vec<String>>> {
        let cfg = self.get_config();
        if name == cfg.name && instance == cfg.instance {
            cfg.on_click.as_ref()
        } else {
            None
        }
    }
    fn build(&self, nai: &Option<NameAndInstance>) -> s::Block;
    fn subst_args<'a>(&'a self, _cmd: &'a [String]) -> Option<Vec<String>>;
}
