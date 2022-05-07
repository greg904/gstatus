use std::convert::TryInto;
use std::fs::File;
use std::io::{BufRead, BufReader, ErrorKind, Seek, SeekFrom};
use std::os::unix::io::RawFd;
use std::time::{Duration, Instant};
use std::{io, iter};

use crate::module::{Block, Module};

fn parse_u64_with_io_error(s: &str) -> io::Result<u64> {
    s.parse()
        .map_err(|err| io::Error::new(ErrorKind::InvalidData, err))
}

pub(crate) struct Mem {
    reader: BufReader<File>,
    timeout: Instant,
    percentage: u8,
}

impl Mem {
    const TIMEOUT: Duration = Duration::from_secs(60);

    pub(crate) fn open() -> io::Result<Self> {
        let file = File::open("/proc/meminfo")?;
        let mut reader = BufReader::new(file);
        let percentage = Mem::read_usage_percentage(&mut reader)?;
        Ok(Mem {
            reader,
            timeout: Instant::now() + Mem::TIMEOUT,
            percentage,
        })
    }

    fn read_usage_percentage(reader: &mut BufReader<File>) -> io::Result<u8> {
        // If we're reading it again, make sure to seek to the start. Note that
        // this also discards the BufReader's buffer which is important as the
        // data in this file has probably changed since the last read.
        reader.seek(SeekFrom::Start(0))?;

        let mut mem_avail: Option<u64> = None;
        let mut mem_total: Option<u64> = None;

        let mut line = String::new();
        loop {
            let len = reader.read_line(&mut line)?;
            if len == 0 {
                // Handle EOF.
                return Err(io::Error::new(
                    ErrorKind::UnexpectedEof,
                    "failed to find MemAvailable and MemTotal in meminfo",
                ));
            }

            if line.is_empty() {
                continue;
            }

            let mut parts = line.split_whitespace();
            let key = parts
                .next()
                .ok_or_else(|| io::Error::new(ErrorKind::InvalidData, "missing key in line"))?;
            let val = parts
                .next()
                .ok_or_else(|| io::Error::new(ErrorKind::InvalidData, "missing value in line"))?;
            match key {
                "MemAvailable:" => mem_avail = Some(parse_u64_with_io_error(val)?),
                "MemTotal:" => mem_total = Some(parse_u64_with_io_error(val)?),
                _ => {}
            }
            if let Some(mem_avail) = mem_avail {
                if let Some(mem_total) = mem_total {
                    return ((mem_total - mem_avail) * 100 / mem_total)
                        .try_into()
                        .map_err(|err| io::Error::new(ErrorKind::InvalidData, err));
                }
            }

            line.clear();
        }
    }
}

impl Module for Mem {
    fn render<'a>(&'a self) -> Box<dyn Iterator<Item = Block> + 'a> {
        let block = Block {
            text: format!("Mem: {}%", self.percentage),
            is_warning: self.percentage >= 70,
        };
        Box::new(iter::once(block))
    }

    fn update(&mut self) -> bool {
        let mut dirty = false;
        match Mem::read_usage_percentage(&mut self.reader) {
            Ok(percentage) => {
                if self.percentage != percentage {
                    self.percentage = percentage;
                    dirty = true;
                }
            }
            Err(err) => eprintln!("failed to read memory usage: {:?}", err),
        }
        self.timeout = Instant::now() + Mem::TIMEOUT;
        dirty
    }

    fn pollable_fd(&self) -> Option<RawFd> {
        None
    }

    fn timeout(&self) -> Option<Instant> {
        Some(self.timeout)
    }
}
