use std::collections::HashMap;
use std::convert::TryFrom;
use std::ffi::OsStr;
use std::io;
use std::os::unix::io::{AsRawFd, RawFd};

use crate::module::{Block, Module};

struct Battery {
    energy_full: u64,
    energy_now: u64,
    status: String,
}

struct MissingOrInvalidProperty;

impl TryFrom<&udev::Device> for Battery {
    type Error = MissingOrInvalidProperty;

    fn try_from(device: &udev::Device) -> Result<Self, Self::Error> {
        let energy_full = device.property_value("POWER_SUPPLY_ENERGY_FULL")
            .and_then(OsStr::to_str)
            .and_then(|s| s.parse().ok())
            .ok_or(MissingOrInvalidProperty)?;
        let energy_now = device.property_value("POWER_SUPPLY_ENERGY_NOW")
            .and_then(OsStr::to_str)
            .and_then(|s| s.parse().ok())
            .ok_or(MissingOrInvalidProperty)?;
        let status = device.property_value("POWER_SUPPLY_STATUS")
            .and_then(OsStr::to_str)
            .ok_or(MissingOrInvalidProperty)?;
        Ok(Self {
            energy_full,
            energy_now,
            status: status.to_string(),
        })
    }
}

pub(crate) struct Batteries {
    udev_socket: udev::MonitorSocket,
    map: HashMap<String, Battery>,
}

impl Batteries {
    pub(crate) fn new() -> io::Result<Batteries> {
        let map = Batteries::scan_batteries()?;
        let udev_socket = udev::MonitorBuilder::new()?
            .match_subsystem("power_supply")?
            .listen()?;
        Ok(Batteries {
            udev_socket,
            map,
        })
    }

    fn scan_batteries() -> io::Result<HashMap<String, Battery>> {
        let mut enumerator = udev::Enumerator::new()?;
        enumerator.match_subsystem("power_supply")?;
        let mut map = HashMap::new();
        for device in enumerator.scan_devices()? {
            if let Some(devpath) = device.devpath().to_str() {
                if let Ok(info) = Battery::try_from(&device) {
                    map.insert(devpath.to_string(), info);
                }
            }
        }
        Ok(map)
    }
}

impl Module for Batteries {
    fn render(&self) -> Vec<Block> {
        self.map.values()
            .map(|bat| {
                let percentage = bat.energy_now * 100 / bat.energy_full;
                Block {
                    text: format!("Battery: {}% ({})", percentage, bat.status),
                    is_warning: percentage <= 15,
                }
            })
            .collect()
    }

    fn update(&mut self) -> bool {
        let mut dirty = false;
        while let Some(event) = self.udev_socket.next() {
            let device = event.device();
            if let Some(devpath) = device.devpath().to_str() {
                let action = event.action()
                    .and_then(|cstr| cstr.to_str());
                match action {
                    Some("add" | "change") => {
                        if let Ok(info) = Battery::try_from(&device) {
                            self.map.insert(devpath.to_string(), info);
                            dirty = true;
                        }
                    },
                    Some("remove") => dirty |= self.map.remove(devpath).is_some(),
                    _ => {},
                }
            }
        }
        dirty
    }

    fn pollable_fd(&self) -> Option<RawFd> {
        Some(self.udev_socket.as_raw_fd())
    }

    fn timeout(&self) -> Option<std::time::Duration> {
        None
    }
}
