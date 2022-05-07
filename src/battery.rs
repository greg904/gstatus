use std::convert::TryInto;
use std::fs::File;
use std::io;
use std::io::{ErrorKind, Read, Seek, SeekFrom};
use std::os::unix::prelude::{AsRawFd, RawFd};

fn read_sysfs_u64(file: &mut File) -> io::Result<u64> {
    let mut buf = String::new();
    file.read_to_string(&mut buf)?;
    // Ignore the last LF.
    buf[..buf.len() - 1]
        .parse()
        .map_err(|err| io::Error::new(ErrorKind::InvalidData, err))
}

pub(crate) struct Battery {
    energy_full: u64,
    energy_now: File,
    status: File,
    udev_socket: udev::MonitorSocket,
}

pub(crate) struct BatteryState {
    pub percentage: u8,
    pub status: String,
}

impl Battery {
    pub(crate) fn open_bat0() -> io::Result<Battery> {
        let mut energy_full = File::open("/sys/class/power_supply/BAT0/energy_full")?;
        let energy_now = File::open("/sys/class/power_supply/BAT0/energy_now")?;
        let status = File::open("/sys/class/power_supply/BAT0/status")?;
        let udev_socket = udev::MonitorBuilder::new()?
            .match_subsystem("power_supply")?
            .listen()?;
        Ok(Battery {
            energy_full: read_sysfs_u64(&mut energy_full)?,
            energy_now,
            status,
            udev_socket,
        })
    }

    fn read_percentage(&mut self) -> io::Result<u8> {
        self.energy_now.seek(SeekFrom::Start(0))?;
        let val = read_sysfs_u64(&mut self.energy_now)?;
        (val * 100 / self.energy_full)
            .try_into()
            .map_err(|err| io::Error::new(ErrorKind::InvalidData, err))
    }

    fn read_status(&mut self) -> io::Result<String> {
        self.status.seek(SeekFrom::Start(0))?;
        let mut buf = String::new();
        self.status.read_to_string(&mut buf)?;
        // Remove the last LF.
        buf.pop();
        Ok(buf)
    }

    pub(crate) fn pollable_fd(&self) -> RawFd {
        self.udev_socket.as_raw_fd()
    }

    /// The function is called when the FD returned by `pollable_fd` is ready
    /// to be read from. It returns whether the state must be reread.
    pub(crate) fn on_fd_ready(&mut self) -> bool {
        let mut dirty = false;
        while let Some(_event) = self.udev_socket.next() {
            dirty = true;
        }
        dirty
    }

    pub(crate) fn read_state(&mut self) -> io::Result<BatteryState> {
        let state = BatteryState {
            percentage: self.read_percentage()?,
            status: self.read_status()?,
        };
        Ok(state)
    }
}
