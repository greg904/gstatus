mod batteries;
mod clock;
mod mem;
mod module;

use std::convert::TryFrom;
use std::convert::TryInto;
use std::io;
use std::io::Write;
use std::time::Instant;

use crate::module::Module;

use self::batteries::*;
use self::clock::*;
use self::mem::*;

fn escape_json_string(s: &str) -> String {
    s.replace('"', "\"")
}

fn main() {
    // i3 protocol start.
    print!("{{\"version\":1}}\n[");

    // Create modules.
    let mut modules: Vec<Box<dyn Module>> = Vec::new();
    match Mem::open() {
        Ok(val) => modules.push(Box::new(val)),
        Err(err) => eprintln!("failed to open meminfo: {:?}", err),
    };
    match Batteries::new() {
        Ok(val) => modules.push(Box::new(val)),
        Err(err) => eprintln!("failed to create the batteries module: {:?}", err),
    };
    modules.push(Box::new(Clock::new()));

    loop {
        // Render all modules.
        print!("[");
        let mut first_block = true;
        for module in modules.iter() {
            for block in module.render() {
                if first_block {
                    first_block = false;
                } else {
                    print!(",");
                }
                let text = escape_json_string(&block.text);
                if block.is_warning {
                    print!("{{\"full_text\":\"{}\",\"color\":\"#ff0000\"}}", text);
                } else {
                    print!("{{\"full_text\":\"{}\"}}", text);
                }
            }
        }
        print!("],");

        // Flush standard output as we don't have a LF and we're going to sleep now.
        if let Err(err) = io::stdout().flush() {
            eprintln!("failed to flush stdout: {:?}", err);
        }

        // Now, wait until something changes...
        'dirty: loop {
            // Find the first module to timeout.
            let now = Instant::now();
            let mut min_diff_ms = -1;
            let mut min_diff_i = 0;
            for (i, module) in modules.iter_mut().enumerate() {
                let timeout = match module.timeout() {
                    Some(val) => val,
                    None => continue,
                };
                let diff_ms = timeout
                    .checked_duration_since(now)
                    .map(|diff| libc::c_int::try_from(diff.as_millis()).unwrap_or(libc::c_int::MAX))
                    .unwrap_or(0);
                if diff_ms == 0 {
                    if module.update() {
                        break 'dirty;
                    }
                } else if min_diff_ms == -1 || diff_ms < min_diff_ms {
                    min_diff_ms = diff_ms;
                    min_diff_i = i;
                }
            }

            // Collect module pollable FDs.
            let mut fds = Vec::new();
            let mut indices = Vec::new();
            for (i, module) in modules.iter().enumerate() {
                let fd = match module.pollable_fd() {
                    Some(val) => val,
                    None => continue,
                };
                fds.push(libc::pollfd {
                    fd,
                    events: libc::EPOLLIN as i16,
                    revents: 0,
                });
                indices.push(i);
            }
            let fds_ptr = fds.as_mut_ptr();

            let ret = unsafe { libc::poll(fds_ptr, fds.len().try_into().unwrap(), min_diff_ms) };
            if ret == -1 {
                eprintln!("poll() failed: {:?}", io::Error::last_os_error());
            } else if ret == 0 {
                if modules[min_diff_i].update() {
                    break 'dirty;
                }
            } else {
                let mut dirty = false;
                for (i, fd) in fds.iter().enumerate() {
                    if i32::from(fd.revents) == libc::EPOLLIN {
                        let module = &mut modules[indices[i]];
                        dirty |= module.update();
                    }
                }
                if dirty {
                    break 'dirty;
                }
            }
        }
    }
}
