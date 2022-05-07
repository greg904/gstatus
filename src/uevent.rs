use std::convert::TryFrom;
use std::ptr;
use std::{collections::HashMap, io, mem, os::unix::io::AsRawFd, os::unix::io::RawFd};

#[derive(Debug)]
pub(crate) enum ParseVarsError {
    InvalidUtf8,
    MissingEqualSign,
    MissingDelimiter,
}

pub(crate) fn parse_vars(
    s: &[u8],
    delimiter: u8,
) -> Result<HashMap<String, String>, ParseVarsError> {
    let mut r = HashMap::new();
    let mut i = 0;
    while i < s.len() {
        let remaining = &s[i..];
        let end = remaining
            .iter()
            .position(|b| *b == delimiter)
            .ok_or(ParseVarsError::MissingDelimiter)?;
        let part = &remaining[..end];
        let eq_sign = part
            .iter()
            .position(|b| *b == b'=')
            .ok_or(ParseVarsError::MissingEqualSign)?;
        let k = std::str::from_utf8(&part[..eq_sign]).map_err(|_| ParseVarsError::InvalidUtf8)?;
        let v =
            std::str::from_utf8(&part[(eq_sign + 1)..]).map_err(|_| ParseVarsError::InvalidUtf8)?;
        r.insert(k.to_owned(), v.to_owned());
        i += end + 1;
    }
    Ok(r)
}

pub(crate) struct Socket {
    fd: libc::c_int,
}

pub(crate) struct Event {
    pub header: String,
    pub vars: HashMap<String, String>,
}

impl Socket {
    /// Tries to open and bind a uevent socket. An error is returned if the socket could not be
    /// opened.
    pub(crate) fn open_and_bind(non_blocking: bool, close_on_exec: bool) -> io::Result<Self> {
        let mut flags = libc::SOCK_RAW;
        if non_blocking {
            flags |= libc::SOCK_NONBLOCK;
        }
        if close_on_exec {
            flags |= libc::SOCK_CLOEXEC;
        }
        let fd = unsafe { libc::socket(libc::AF_NETLINK, flags, libc::NETLINK_KOBJECT_UEVENT) };
        if fd == -1 {
            return Err(io::Error::last_os_error());
        }
        // Construct the socket as early as possible to get RAII to automatically close it if there
        // is an error.
        let socket = Self { fd };

        let mut addr: libc::sockaddr_nl = unsafe { mem::zeroed() };
        addr.nl_family = libc::AF_NETLINK as u16;
        addr.nl_groups = 1;
        let ret = unsafe {
            libc::bind(
                fd,
                &addr as *const libc::sockaddr_nl as *const libc::sockaddr,
                mem::size_of_val(&addr) as libc::c_uint,
            )
        };
        if ret == -1 {
            return Err(io::Error::last_os_error());
        }

        Ok(socket)
    }

    pub(crate) fn recv_event(&self) -> io::Result<Event> {
        let mut buf = vec![0u8; 4096];

        let mut iov = libc::iovec {
            iov_base: buf.as_mut_ptr() as *mut libc::c_void,
            iov_len: buf.len(),
        };

        let mut hdr = libc::msghdr {
            msg_name: ptr::null_mut(),
            msg_namelen: 0,
            msg_iov: &mut iov as *mut libc::iovec,
            msg_iovlen: 1,
            msg_control: ptr::null_mut(),
            msg_controllen: 0,
            msg_flags: 0,
        };

        let ret = unsafe { libc::recvmsg(self.fd, &mut hdr as *mut libc::msghdr, 0) };
        if ret == -1 {
            return Err(io::Error::last_os_error());
        }
        if (hdr.msg_flags & libc::MSG_TRUNC) != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "uevent datagram is too big",
            ));
        }
        let read = usize::try_from(ret).unwrap();
        if read == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "empty uevent datagram",
            ));
        }

        let header_end = match buf[..read].iter().position(|b| *b == b'\0') {
            Some(p) => p,
            None => {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "missing header",
                ));
            }
        };

        let header = std::str::from_utf8(&buf[..header_end])
            .map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "missing or invalid uevent header",
                )
            })?
            .to_owned();
        let vars = parse_vars(&buf[(header_end + 1)..read], b'\0')
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid uevent vars"))?;
        Ok(Event { header, vars })
    }
}

impl AsRawFd for Socket {
    fn as_raw_fd(&self) -> RawFd {
        self.fd
    }
}

impl Drop for Socket {
    fn drop(&mut self) {
        let ret = unsafe { libc::close(self.fd) };
        if ret == -1 {
            eprintln!(
                "failed to close uevent socket: {}",
                io::Error::last_os_error()
            );
        }
    }
}
