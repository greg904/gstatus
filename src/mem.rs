use std::convert::TryInto;
use std::fs::File;
use std::io;
use std::io::{BufRead, BufReader, ErrorKind, Seek, SeekFrom};

fn parse_u64_with_io_error(s: &str) -> io::Result<u64> {
    return s
        .parse()
        .map_err(|err| io::Error::new(ErrorKind::InvalidData, err));
}

pub(crate) struct Meminfo {
    reader: BufReader<File>,
}

impl Meminfo {
    pub(crate) fn open() -> io::Result<Self> {
        let file = File::open("/proc/meminfo")?;
        Ok(Meminfo {
            reader: BufReader::new(file),
        })
    }

    pub(crate) fn read_usage_percentage(&mut self) -> io::Result<u8> {
        // If we're reading it again, make sure to seek to the start. Note that
        // this also discards the BufReader's buffer which is important as the
        // data in this file has probably changed since the last read.
        self.reader.seek(SeekFrom::Start(0))?;

        let mut mem_avail: Option<u64> = None;
        let mut mem_total: Option<u64> = None;

        let mut line = String::new();
        loop {
            let len = self.reader.read_line(&mut line)?;
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
                "MemAvailable:" => mem_avail = Some(parse_u64_with_io_error(&val)?),
                "MemTotal:" => mem_total = Some(parse_u64_with_io_error(&val)?),
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
