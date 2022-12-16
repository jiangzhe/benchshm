use anyhow::Result;
use std::{net::TcpListener, time::Instant};
use std::os::unix::net::UnixListener;
use benchshm::{ConnKind, server_conn, parse_conn_kind, Cabin, CabinGuard, CabinState};
use shared_memory::ShmemConf;
use crossbeam_utils::Backoff;
use std::sync::atomic::Ordering;

fn main() -> Result<()> {
    let args = SvrArgs::parse_from_env()?;
    
    println!("Listening at ({:?})({})", args.addr.0, args.addr.1);

    match args.addr.0 {
        ConnKind::Tcp => {
            let listener = TcpListener::bind(&args.addr.1)?;
            while let Ok((conn, remote_addr)) = listener.accept() {
                // use current thread to handle connection
                let inst = Instant::now();
                let sum = server_conn(conn)?;
                let dur = inst.elapsed();
                println!("disconnected from remote addr {:?}, sum is {}, duration is {:?}", remote_addr, sum, dur);
            }
        }
        ConnKind::Unix => {
            let listener = UnixListener::bind(&args.addr.1)?;
            while let Ok((conn, remote_addr)) = listener.accept() {
                // use current thread to handle connection
                let inst = Instant::now();
                let sum = server_conn(conn)?;
                let dur = inst.elapsed();
                println!("disconnected from remote addr {:?}, sum is {}, duration is {:?}", remote_addr, sum, dur);
            }
        }
        ConnKind::Shm => {
            unsafe {
                let shmem = ShmemConf::new().size(4096).flink(&args.addr.1).create()?;
                let raw_ptr = shmem.as_ptr();
                let guard = CabinGuard;
                let cabin: &Cabin<u64, u64> = Cabin::new(raw_ptr, &guard)?;
                let mut client_id = 0;
                let mut sum = 0;
                let mut resp_written = false;
                let mut inst = Instant::now();
                let mut yield_num = 0usize;
                loop {
                    match cabin.load_state(Ordering::Acquire) {
                        CabinState::AcceptingSpin => {
                            let backoff = Backoff::new();
                            backoff.snooze();
                            while cabin.load_state(Ordering::Acquire) == CabinState::AcceptingSpin {
                                if backoff.is_completed() {
                                    // try yield current thread
                                    _ = cabin.cas_state(CabinState::AcceptingSpin, CabinState::AcceptingYield);
                                    break
                                } else {
                                    backoff.snooze();
                                }
                            }
                        }
                        CabinState::AcceptingYield => {
                            yield_num += 1;
                            // blocking wait
                            let lg = cabin.lock().unwrap();
                            while cabin.load_state(Ordering::Acquire) == CabinState::AcceptingYield {
                                lg.wait()?;
                            }
                        }
                        CabinState::Connecting => {
                            client_id = cabin.id();
                            inst = Instant::now();
                            // transfer state to WAIT_REQ
                            _ = cabin.cas_state(CabinState::Connecting, CabinState::WaitReqSpin);
                        }
                        CabinState::WaitReqSpin => {
                            let backoff = Backoff::new();
                            backoff.snooze();
                            while cabin.load_state(Ordering::Acquire) == CabinState::WaitReqSpin {
                                if !args.spin_only && backoff.is_completed() {
                                    // try yield current thread
                                    _ = cabin.cas_state(CabinState::WaitReqSpin, CabinState::WaitReqYield);
                                    break
                                } else {
                                    backoff.snooze();
                                }
                            }
                        }
                        CabinState::WaitReqYield => {
                            yield_num += 1;
                            // blocking wait
                            let lg = cabin.lock().unwrap();
                            while cabin.load_state(Ordering::Acquire) == CabinState::WaitReqYield {
                                lg.wait()?;
                            }
                        }
                        CabinState::WaitRespSpin => {
                            let req = cabin.req();
                            sum += req;
                            cabin.set_resp(sum);
                            resp_written = true;
                            // transfer state to WAIT_REQ
                            if cabin.cas_state(CabinState::WaitRespSpin, CabinState::WaitReqSpin).is_ok() {
                                resp_written = false; // reset the flag so next time write new response
                            }
                        }
                        CabinState::WaitRespYield => {
                            if !resp_written {
                                let req = cabin.req();
                                sum += req;
                                cabin.set_resp(sum);
                            }
                            let lg = cabin.lock().unwrap();
                            assert!(cabin.cas_state(CabinState::WaitRespYield, CabinState::WaitReqSpin).is_ok());
                            resp_written = false;
                            lg.signal().unwrap();
                        }
                        CabinState::Disconnected => {
                            let dur = inst.elapsed();
                            println!("disconnected from client {}, sum is {}, duration is {:?}, yields is {}", client_id, sum, dur, yield_num);
                            client_id = 0;
                            sum = 0;
                            yield_num = 0;
                            inst = Instant::now();
                            // transfer state to ACCEPTING
                            _ = cabin.cas_state(CabinState::Disconnected, CabinState::AcceptingSpin);
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

#[derive(Debug)]
pub struct SvrArgs {
    pub addr: (ConnKind, String),
    pub spin_only: bool,
}

impl SvrArgs {
    #[inline]
    pub fn parse_from_env() -> Result<SvrArgs> {
        use lexopt::prelude::*;
        let mut parser = lexopt::Parser::from_env();
        let mut addr = None;
        let mut spin_only = false;
        while let Some(arg) = parser.next()? {
            match arg {
                Short('a') | Long("addr") => {
                    addr = Some(parse_conn_kind(&parser.value()?.to_string_lossy())?);
                }
                Short('s') | Long("spin-only") => {
                    spin_only = parser.value()?.parse()?
                }
                _ => return Err(arg.unexpected().into())
            }
        }
        Ok(SvrArgs{addr: addr.unwrap_or_else(|| parse_conn_kind("tcp:127.0.0.1:9001").unwrap()), spin_only})
    }
}
