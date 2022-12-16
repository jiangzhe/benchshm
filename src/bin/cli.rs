use anyhow::Result;
use std::net::TcpStream;
use std::os::unix::net::UnixStream;
use std::time::Instant;
use benchshm::{client_conn, parse_conn_kind, ConnKind, Cabin, CabinState, CabinGuard};
use shared_memory::ShmemConf;
use crossbeam_utils::Backoff;
use std::sync::atomic::Ordering;

fn main() -> anyhow::Result<()> {
    let args = CliArgs::parse_from_env()?;

    println!("connecting ({:?})({})", args.addr.0, args.addr.1);

    let (sum, dur, yield_num) = match args.addr.0 {
        ConnKind::Tcp => {
            let conn = TcpStream::connect(&args.addr.1)?;
            let inst = Instant::now();
            let sum = client_conn(conn, args.value, args.num)?;
            let dur = inst.elapsed();
            (sum, dur, 0)
        }
        ConnKind::Unix => {
            let conn = UnixStream::connect(&args.addr.1)?;
            let inst = Instant::now();
            let sum = client_conn(conn, args.value, args.num)?;
            let dur = inst.elapsed();
            (sum, dur, 0)
        }
        ConnKind::Shm => {
            unsafe {
                let shmem = ShmemConf::new().size(4096).flink(&args.addr.1).open()?;
                let raw_ptr = shmem.as_ptr();
                let guard = CabinGuard;
                let cabin: &Cabin<u64, u64> = Cabin::from_existing(raw_ptr, &guard);
                let client_id: u32 = rand::random();
                let mut sum = 0;
                let mut value = 0;
                let mut id_written = false;
                let mut req_written = false;
                let mut yield_num = 0usize;
                let mut inst = Instant::now();
                loop {
                    match cabin.load_state(Ordering::Acquire) {
                        CabinState::AcceptingSpin => {
                            cabin.set_id(client_id);
                            id_written = true;
                            inst = Instant::now();
                            _ = cabin.cas_state(CabinState::AcceptingSpin, CabinState::Connecting);
                        }
                        CabinState::AcceptingYield => {
                            if !id_written {
                                cabin.set_id(client_id);
                                inst = Instant::now();
                                id_written = true;
                            }
                            let lg = cabin.lock().unwrap();
                            assert!(cabin.cas_state(CabinState::AcceptingYield, CabinState::Connecting).is_ok());
                            lg.signal().unwrap();
                        }
                        CabinState::Connecting => {
                            let backoff = Backoff::new();
                            backoff.snooze();
                            while cabin.load_state(Ordering::Acquire) == CabinState::Connecting {
                                backoff.snooze(); // todo: yield if spin limit reached
                            }
                        }
                        CabinState::WaitReqSpin => {
                            let resp = cabin.resp();
                            debug_assert_eq!(sum, resp);
                            if value >= args.num as u64 {
                                _ = cabin.cas_state(CabinState::WaitReqSpin, CabinState::Disconnected);
                            } else {
                                cabin.set_req(value);
                                sum += value;
                                value += 1;
                                req_written = true;
                                if cabin.cas_state(CabinState::WaitReqSpin, CabinState::WaitRespSpin).is_ok() {
                                    req_written = false;
                                }
                            }
                        }
                        CabinState::WaitReqYield => {
                            let resp = cabin.resp();
                            debug_assert_eq!(sum, resp);
                            if value >= args.num as u64 {
                                _ = cabin.cas_state(CabinState::WaitReqYield, CabinState::Disconnected);
                                break
                            } else {
                                if !req_written {
                                    cabin.set_req(value);
                                    sum += value;
                                    value += 1;
                                }
                                let lg = cabin.lock().unwrap();
                                assert!(cabin.cas_state(CabinState::WaitReqYield, CabinState::WaitRespSpin).is_ok());
                                req_written = false;
                                lg.signal().unwrap();
                            }
                        }
                        CabinState::WaitRespSpin => {
                            let backoff = Backoff::new();
                            backoff.snooze();
                            while cabin.load_state(Ordering::Acquire) == CabinState::WaitRespSpin {
                                if !args.spin_only && backoff.is_completed() {
                                    // try yield current thread
                                    _ = cabin.cas_state(CabinState::WaitRespSpin, CabinState::WaitRespYield);
                                    break
                                } else {
                                    backoff.snooze();
                                }
                            }
                        }
                        CabinState::WaitRespYield => {
                            yield_num += 1;
                            // blocking wait
                            let lg = cabin.lock().unwrap();
                            while cabin.load_state(Ordering::Acquire) == CabinState::WaitRespYield {
                                lg.wait()?;
                            }
                        }
                        CabinState::Disconnected => break,
                    }
                }
                let dur = inst.elapsed();
                (sum, dur, yield_num)
            }
        }
    };
    
    println!("disconnected: num is {}, sum is {}, duration is {:?}, avg latency is {:?}, yields is {}", args.num, sum, dur, dur / args.num, yield_num);
    Ok(())
}

#[derive(Debug)]
pub struct CliArgs {
    pub addr: (ConnKind, String),
    pub num: u32,
    pub value: Option<u64>,
    pub spin_only: bool
}

impl CliArgs {
    #[inline]
    pub fn parse_from_env() -> Result<CliArgs> {
        use lexopt::prelude::*;
        let mut parser = lexopt::Parser::from_env();
        let mut addr = None;
        let mut num = 1024; // by default 1024
        let mut value = None;
        let mut spin_only = false;
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
                Short('s') | Long("spin-only") => {
                    spin_only = parser.value()?.parse()?
                }
                _ => return Err(arg.unexpected().into())
            }
        }
        Ok(CliArgs{addr: addr.unwrap_or_else(|| parse_conn_kind("tcp:127.0.0.1:9001").unwrap()), num, value, spin_only})
    }
}
