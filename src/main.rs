mod battery;
mod mem;

use std::io;
use std::io::ErrorKind;
use std::io::Write;
use std::thread;
use std::time::Duration;

use time::OffsetDateTime;

use self::battery::*;
use self::mem::*;

fn sleep_until_next_minute(now: &OffsetDateTime) {
    const MARGIN_SECS: u8 = 1;
    thread::sleep(Duration::new(
        // Round up to the next minute, adding a small margin to account for
        // error.
        (59 - (now.second() % 60) + MARGIN_SECS).into(),
        1_000_000_000 - (now.nanosecond() % 1_000_000_000),
    ));
}

fn main() {
    // i3 protocol start.
    print!("{{\"version\":1}}\n[");

    let mut bat0 = match Battery::open_bat0() {
        Ok(val) => Some(val),
        Err(err) => {
            if err.kind() != ErrorKind::NotFound {
                eprintln!("Failed to read energy_full for BAT0: {:?}", err);
            }
            None
        }
    };

    let mut meminfo = match Meminfo::open() {
        Ok(val) => Some(val),
        Err(err) => {
            eprintln!("Failed to open meminfo: {:?}", err);
            None
        }
    };

    let mut tz_err_shown = false;

    loop {
        // Start of the new status.
        print!("[");

        if let Some(ref mut meminfo) = meminfo {
            // Print memory usage.
            let usage = meminfo.read_usage_percentage().unwrap();
            if usage >= 70 {
                print!("{{\"full_text\":\"Mem: {}%\",\"color\":\"#ff0000\"}},", usage);
            } else {
                print!("{{\"full_text\":\"Mem: {}%\"}},", usage);
            }
        }

        if let Some(ref mut bat0) = bat0 {
            // Print battery state.
            if let Ok(state) = bat0.read_state() {
                if state.percentage <= 15 {
                    print!(
                        "{{\"full_text\":\"Battery: {}% ({})\",\"color\":\"#ff0000\"}},",
                        state.percentage, state.status
                    );
                } else {
                    print!(
                        "{{\"full_text\":\"Battery: {}% ({})\"}},",
                        state.percentage, state.status
                    );
                }
            }
        }

        let now = OffsetDateTime::try_now_local()
            .unwrap_or_else(|err| {
                if !tz_err_shown {
                    eprintln!("Failed to retreive local timezone: {:?}", err);
                    tz_err_shown = true;
                }
                OffsetDateTime::now_utc()
            });

        // Print clock.
        print!("{{\"full_text\":\"{}:{:02}\"}}],", now.hour(), now.minute());

        // Flush standard output as we don't have a LF and we're going to sleep now.
        if let Err(err) = io::stdout().flush() {
            eprintln!("Failed to flush stdout: {:?}", err);
        }

        sleep_until_next_minute(&now);
    }
}
