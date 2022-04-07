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

//! The date `swayrbar` module.

use crate::bar::config;
use crate::bar::module::BarModuleFn;
use crate::fmt_replace::fmt_replace;
use std::sync::Mutex;
use std::sync::Once;
use swaybar_types as s;
use sysinfo as si;
use sysinfo::ProcessorExt;
use sysinfo::SystemExt;

const NAME: &str = "sysinfo";

pub struct BarModuleSysInfo {
    config: config::ModuleConfig,
    system: Mutex<si::System>,
}

struct OnceRefresher {
    cpu: Once,
    memory: Once,
}

impl OnceRefresher {
    fn new() -> OnceRefresher {
        OnceRefresher {
            cpu: Once::new(),
            memory: Once::new(),
        }
    }

    fn refresh_cpu(&self, sys: &mut si::System) {
        self.cpu.call_once(|| sys.refresh_cpu());
    }

    fn refresh_memory(&self, sys: &mut si::System) {
        self.memory.call_once(|| sys.refresh_memory());
    }
}

fn get_cpu_usage(sys: &mut si::System, upd: &OnceRefresher) -> f32 {
    upd.refresh_cpu(sys);
    sys.global_processor_info().cpu_usage()
}

fn get_memory_usage(sys: &mut si::System, upd: &OnceRefresher) -> f64 {
    upd.refresh_memory(sys);
    sys.used_memory() as f64 * 100_f64 / sys.total_memory() as f64
}

#[derive(Debug)]
enum LoadAvg {
    One,
    Five,
    Fifteen,
}

fn get_load_average(
    sys: &mut si::System,
    avg: LoadAvg,
    upd: &OnceRefresher,
) -> f64 {
    upd.refresh_cpu(sys);
    let load_avg = sys.load_average();
    match avg {
        LoadAvg::One => load_avg.one,
        LoadAvg::Five => load_avg.five,
        LoadAvg::Fifteen => load_avg.fifteen,
    }
}

impl BarModuleFn for BarModuleSysInfo {
    fn create(config: config::ModuleConfig) -> Box<dyn BarModuleFn> {
        Box::new(BarModuleSysInfo {
            config,
            system: Mutex::new(si::System::new_all()),
        })
    }

    fn name() -> &'static str {
        NAME
    }

    fn matches(&self, name: &str, instance: &str) -> bool {
        NAME == name && self.config.instance == instance
    }

    fn build(&self) -> s::Block {
        let updater = OnceRefresher::new();
        s::Block {
            name: Some(Self::name().to_owned()),
            instance: Some(self.config.instance.clone()),
            full_text: {
                let mut sys = self.system.lock().unwrap();
                fmt_replace!(&self.config.format, self.config.html_escape, {
                    "cpu_usage" => get_cpu_usage(&mut sys, &updater),
                    "mem_usage" => get_memory_usage(&mut sys, &updater),
                    "load_avg_1" => get_load_average(&mut sys,
                                                     LoadAvg::One, &updater),
                    "load_avg_5" => get_load_average(&mut sys,
                                                     LoadAvg::Five, &updater),
                    "load_avg_15" => get_load_average(&mut sys,
                                                      LoadAvg::Fifteen, &updater),
                })
            },
            align: Some(s::Align::Left),
            markup: Some(s::Markup::Pango),
            short_text: None,
            color: None,
            background: None,
            border: None,
            border_top: None,
            border_bottom: None,
            border_left: None,
            border_right: None,
            min_width: None,
            urgent: None,
            separator: Some(true),
            separator_block_width: None,
        }
    }

    fn default_config(instance: String) -> config::ModuleConfig {
        config::ModuleConfig {
            module_type: "sysinfo".to_owned(),
            instance,
            format: "💻 CPU: {cpu_usage:{:4.1}}% Mem: {mem_usage:{:4.1}}% Load: {load_avg_1:{:4.2}} / {load_avg_5:{:4.2}} / {load_avg_15:{:4.2}}".to_owned(),
            html_escape: true }
    }
}