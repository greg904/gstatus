use std::collections::HashMap;
use std::os::unix::io::{AsRawFd, RawFd};
use std::time::Instant;
use std::{fs, io};

use crate::module::{Block, Module};
use crate::uevent;

struct Battery {
    energy_full: u64,
    energy_now: u64,
    status: String,
}

#[derive(Debug)]
struct MissingOrInvalidProperty;

impl Battery {
    fn from_vars(vars: &HashMap<String, String>) -> Result<Self, MissingOrInvalidProperty> {
        let energy_full = vars
            .get("POWER_SUPPLY_ENERGY_FULL")
            .and_then(|s| s.parse().ok())
            .ok_or(MissingOrInvalidProperty)?;
        let energy_now = vars
            .get("POWER_SUPPLY_ENERGY_NOW")
            .and_then(|s| s.parse().ok())
            .ok_or(MissingOrInvalidProperty)?;
        let status = vars
            .get("POWER_SUPPLY_STATUS")
            .ok_or(MissingOrInvalidProperty)?;
        Ok(Self {
            energy_full,
            energy_now,
            status: status.to_string(),
        })
    }
}

pub(crate) struct Batteries {
    uevent_socket: uevent::Socket,
    map: HashMap<String, Battery>,
}

impl Batteries {
    pub(crate) fn new() -> io::Result<Batteries> {
        let map = Batteries::scan_batteries()?;
        let uevent_socket = uevent::Socket::open_and_bind(true, true)?;
        Ok(Batteries { uevent_socket, map })
    }

    fn scan_batteries() -> io::Result<HashMap<String, Battery>> {
        let mut map = HashMap::new();
        for entry in fs::read_dir("/sys/class/power_supply")? {
            let entry = entry?;
            let type_ = match entry.file_type() {
                Ok(t) => t,
                Err(err) => {
                    eprintln!("failed to read file type in /sys/class/power_supply: {err}");
                    continue;
                }
            };
            if !type_.is_symlink() {
                continue;
            }
            let target = match fs::read_link(entry.path()) {
                Ok(t) => t,
                Err(err) => {
                    eprintln!("failed to read link target in /sys/class/power_supply: {err}");
                    continue;
                }
            };
            let target_str = target.to_string_lossy();
            let devpath = match target_str.strip_prefix("../..") {
                Some(d) => d,
                None => {
                    eprintln!("invalid battery device link: {}", target_str);
                    continue;
                }
            };
            let ue_path = entry.path().join("uevent");
            let ue = match fs::read(ue_path) {
                Ok(u) => u,
                Err(err) => {
                    eprintln!("failed to read uevent from battery device: {err}");
                    continue;
                }
            };
            let vars = match uevent::parse_vars(&ue, b'\n') {
                Ok(v) => v,
                Err(err) => {
                    eprintln!("failed to read vars from battery device: {:?}", err);
                    continue;
                }
            };
            // This will fail for power supply devices that aren't batteries, so no error should be
            // logged.
            if let Ok(info) = Battery::from_vars(&vars) {
                map.insert(devpath.to_owned(), info);
            }
        }
        Ok(map)
    }
}

impl Module for Batteries {
    fn render<'a>(&'a self) -> Box<dyn Iterator<Item = Block> + 'a> {
        Box::new(self.map.values().map(|bat| {
            let percentage = bat.energy_now * 100 / bat.energy_full;
            Block {
                text: format!("Battery: {}% ({})", percentage, bat.status),
                is_warning: percentage <= 15,
            }
        }))
    }

    fn update(&mut self) -> bool {
        let mut dirty = false;
        loop {
            let event = match self.uevent_socket.recv_event() {
                Ok(e) => e,
                Err(err) => {
                    if err.kind() != io::ErrorKind::WouldBlock {
                        eprintln!("failed to read uevent: {err}");
                    }
                    break;
                }
            };
            let action = match event.vars.get("ACTION") {
                Some(a) => a,
                None => continue,
            };
            let devpath = match event.vars.get("DEVPATH") {
                Some(d) => d,
                None => continue,
            };
            if action == "add" || action == "change" {
                // This will fail for devices that aren't batteries, so no error should be logged.
                if let Ok(info) = Battery::from_vars(&event.vars) {
                    self.map.insert(devpath.to_owned(), info);
                    dirty = true;
                }
            } else if action == "remove" {
                dirty |= self.map.remove(devpath).is_some();
            }
        }
        dirty
    }

    fn pollable_fd(&self) -> Option<RawFd> {
        Some(self.uevent_socket.as_raw_fd())
    }

    fn timeout(&self) -> Option<Instant> {
        None
    }
}
