use anyhow::{anyhow, Result};
use std::io::{self, Read, Write};
// use libc::{mkfifo, mode_t, EACCES, EEXIST, ENOENT};
// use std::ffi::CString;
// use std::path::Path;
// use std::fs::{File, OpenOptions};


pub mod state {
    pub const ACCEPTING: u8 = 0;
    pub const CONNECTING: u8 = 1;
    pub const WAIT_CLI_REQ: u8 = 2;
    pub const WAIT_SVR_RESP: u8 = 3;
    pub const DISCONNECTED: u8 = 4;

    #[inline]
    pub unsafe fn set_state(state_ptr: *mut u8, new_state: u8) -> u8 {
        state_ptr.write_volatile(new_state);
        new_state
    }
}

#[derive(Debug)]
pub enum ConnKind {
    Tcp,
    Unix,
    Shm,
    // Pipe,
}

#[inline]
pub fn parse_conn_kind(s: &str) -> Result<(ConnKind, String)> {
    for (proto, kind) in [
        ("tcp", ConnKind::Tcp), 
        ("unix", ConnKind::Unix), 
        ("shm", ConnKind::Shm),
        // ("pipe", ConnKind::Pipe),
    ] {
        if s.starts_with(proto) {
            return Ok((kind, s[proto.len()+1..].to_string()))
        }
    }
    Err(anyhow!("unexpected protocol type: {}", s))
}

#[inline]
pub fn client_conn<T>(mut conn: T, value: Option<u64>, num: u32) -> Result<u64> 
where
    T: Read + Write,
{
    let mut sum = 0u64;
    if let Some(value) = value {
        if value & 1 == 1 {
            // read response only if value is odd
            for _ in 0..num {
                let mut buf = value.to_le_bytes();
                // send request
                conn.write_all(&buf)?;
                conn.flush()?;
                sum += value;
                conn.read_exact(&mut buf)?;
            }
        } else {
            for _ in 0..num {
                let buf = value.to_le_bytes();
                // send request
                conn.write_all(&buf)?;
                conn.flush()?;
                sum += value;
            }
        }
    } else {
        for value in 0..num as u64 {
            let mut buf = value.to_le_bytes();
            // send request
            conn.write_all(&buf)?;
            conn.flush()?;
            sum += value;
            if value & 1 == 1 {
                // only read response if value is odd
                conn.read_exact(&mut buf)?;
                // debug_assert_eq!(sum, u64::from_le_bytes(buf));
            }
        }
    }
    Ok(sum)
}


#[inline]
pub fn server_conn<T>(mut conn: T) -> Result<u64> 
where
    T: Read + Write,
{
    // read 8 bytes as little-endian integer and sum.
    let mut sum = 0u64;
    let mut buf = [0u8; 8];
    // read request
    while conn.read_exact(&mut buf).is_ok() {
        let value = u64::from_le_bytes(buf);
        sum += value;
        
        if value & 1 == 1 {
            // only send response if value is odd
            conn.write_all(&sum.to_le_bytes())?;
            conn.flush()?;
        }
    }
    Ok(sum)
}

// #[inline]
// pub fn create_fifo<P: AsRef<Path>>(path: P, mode: Option<u32>) -> io::Result<()> {
//     let path = CString::new(path.as_ref().to_str().unwrap())?;
//     let mode = mode.unwrap_or(0o644);
//     let res = unsafe { mkfifo(path.as_ptr(), mode as mode_t) };
//     if res == 0 {
//         return Ok(())
//     }
//     let error = errno::errno();
//     match error.0 {
//         EACCES => Err(io::Error::new(
//             io::ErrorKind::PermissionDenied,
//             format!("could not open {:?}: {}", path, error),
//         )),
//         EEXIST => Err(io::Error::new(
//             io::ErrorKind::AlreadyExists,
//             format!("could not open {:?}: {}", path, error),
//         )),
//         ENOENT => Err(io::Error::new(
//             io::ErrorKind::NotFound,
//             format!("could not open {:?}: {}", path, error),
//         )),
//         _ => Err(io::Error::new(
//             io::ErrorKind::Other,
//             format!("could not open {:?}: {}", path, error),
//         ))
//     }
// }

// pub fn open_read<P: AsRef<Path>>(path: P) -> io::Result<File> {
//     OpenOptions::new()
//         .read(true)
//         .open(path)
// }

// pub fn open_write<P: AsRef<Path>>(path: P) -> io::Result<File> {
//     OpenOptions::new()
//         .write(true)
//         .append(true)
//         .open(path)
// }