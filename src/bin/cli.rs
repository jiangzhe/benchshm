use anyhow::Result;
use std::net::TcpStream;
use std::os::unix::net::UnixStream;
use std::time::Instant;
use benchshm::{client_conn, parse_conn_kind, ConnKind};
use benchshm::state::*;
use shared_memory::ShmemConf;
use crossbeam_utils::Backoff;
use std::io::{self, Read, Write};

fn main() -> anyhow::Result<()> {
    let args = CliArgs::parse_from_env()?;

    println!("connecting ({:?})({})", args.addr.0, args.addr.1);

    let (sum, dur) = match args.addr.0 {
        ConnKind::Tcp => {
            let conn = TcpStream::connect(&args.addr.1)?;
            let inst = Instant::now();
            let sum = client_conn(conn, args.value, args.num)?;
            let dur = inst.elapsed();
            (sum, dur)
        }
        ConnKind::Unix => {
            let conn = UnixStream::connect(&args.addr.1)?;
            let inst = Instant::now();
            let sum = client_conn(conn, args.value, args.num)?;
            let dur = inst.elapsed();
            (sum, dur)
        }
        ConnKind::Shm => {
            let shmem = ShmemConf::new().size(4096).flink(&args.addr.1).open()?;
            let raw_ptr = shmem.as_ptr();
            unsafe {
                let state_ptr = raw_ptr;
                let id_ptr = raw_ptr.add(4) as *mut u32;
                let req_ptr = raw_ptr.add(8) as *mut u64;
                let resp_ptr = raw_ptr.add(16) as *mut u64;
                let client_id: u32 = rand::random();
                let mut sum = 0u64;
                let mut value = 0;
                let mut inst = Instant::now();
                let mut state = state_ptr.read_volatile();
                loop {
                    match state {
                        ACCEPTING => {
                            id_ptr.write_volatile(client_id);
                            inst = Instant::now();
                            state = set_state(state_ptr, CONNECTING);
                        }
                        CONNECTING => {
                            let backoff = Backoff::new();
                            loop {
                                backoff.snooze();
                                state = state_ptr.read_volatile();
                                if state != CONNECTING {
                                    break
                                }
                            }
                        }
                        WAIT_CLI_REQ => {
                            let resp = resp_ptr.read_volatile();
                            debug_assert_eq!(sum, resp);
                            if value >= args.num as u64 {
                                state_ptr.write_volatile(DISCONNECTED);
                                break
                            } else {
                                req_ptr.write_volatile(value);
                                sum += value;
                                value += 1;
                                state = set_state(state_ptr, WAIT_SVR_RESP);
                            }
                        }
                        WAIT_SVR_RESP => {
                            let backoff = Backoff::new();
                            loop {
                                backoff.snooze();
                                state = state_ptr.read_volatile();
                                if state != WAIT_SVR_RESP {
                                    break
                                }
                            }
                        }
                        _ => unreachable!(),
                    }
                }
                let dur = inst.elapsed();
                (sum, dur)
            }
        }
    };
    
    println!("disconnected: num is {}, sum is {}, duration is {:?}, avg latency is {:?}", args.num, sum, dur, dur / args.num);
    Ok(())
}

#[derive(Debug)]
pub struct CliArgs {
    pub addr: (ConnKind, String),
    pub num: u32,
    pub value: Option<u64>,
}

impl CliArgs {
    #[inline]
    pub fn parse_from_env() -> Result<CliArgs> {
        use lexopt::prelude::*;
        let mut parser = lexopt::Parser::from_env();
        let mut addr = None;
        let mut num = 1024; // by default 1024
        let mut value = None;
        while let Some(arg) = parser.next()? {
            match arg {
                Short('a') | Long("addr") => {
                    addr = Some(parse_conn_kind(&parser.value()?.to_string_lossy())?);
                }
                Short('n') | Long("num") => {
                    num = parser.value()?.parse()?
                }
                Short('v') | Long("value") => {
                    value = Some(parser.value()?.parse()?)
                }
                _ => return Err(arg.unexpected().into())
            }
        }
        Ok(CliArgs{addr: addr.unwrap_or_else(|| parse_conn_kind("tcp:127.0.0.1:9001").unwrap()), num, value})
    }
}
