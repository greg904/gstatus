use std::{
    iter,
    os::unix::io::RawFd,
    time::{Duration, Instant},
};

use time::OffsetDateTime;

use crate::module::{Block, Module};

pub(crate) struct Clock {
    timeout: Instant,
    hour: u8,
    minute: u8,
}

impl Clock {
    pub(crate) fn new() -> Self {
        let now = Clock::read();
        Self {
            timeout: Instant::now() + Clock::time_until_next_minute(now),
            hour: now.hour(),
            minute: now.minute(),
        }
    }

    fn read() -> OffsetDateTime {
        OffsetDateTime::try_now_local().unwrap_or_else(|err| {
            eprintln!("failed to retreive local timezone: {:?}", err);
            OffsetDateTime::now_utc()
        })
    }

    fn time_until_next_minute(time: OffsetDateTime) -> Duration {
        const MARGIN_SECS: u8 = 1;
        Duration::new(
            // Round up to the next minute, adding a small margin to account for
            // error.
            (59 - (time.second() % 60) + MARGIN_SECS).into(),
            1_000_000_000 - (time.nanosecond() % 1_000_000_000),
        )
    }
}

impl Module for Clock {
    fn render<'a>(&'a self) -> Box<dyn Iterator<Item = Block> + 'a> {
        let block = Block {
            text: format!("{:02}:{:02}", self.hour, self.minute),
            is_warning: false,
        };
        Box::new(iter::once(block))
    }

    fn update(&mut self) -> bool {
        let now = Clock::read();
        let mut dirty = false;
        if self.hour != now.hour() {
            self.hour = now.hour();
            dirty = true;
        }
        if self.minute != now.minute() {
            self.minute = now.minute();
            dirty = true;
        }
        self.timeout = Instant::now() + Clock::time_until_next_minute(now);
        dirty
    }

    fn pollable_fd(&self) -> Option<RawFd> {
        None
    }

    fn timeout(&self) -> Option<Instant> {
        Some(self.timeout)
    }
}
