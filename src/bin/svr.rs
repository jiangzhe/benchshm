use anyhow::Result;
use std::{net::TcpListener, time::Instant};
use std::os::unix::net::UnixListener;
use benchshm::{ConnKind, server_conn, parse_conn_kind};
use benchshm::state::*;
use shared_memory::ShmemConf;
use crossbeam_utils::Backoff;
use std::io::{self, Read, Write};
use std::fs::File;

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
            let shmem = ShmemConf::new().size(4096).flink(&args.addr.1).create()?;
            let raw_ptr = shmem.as_ptr();
            // memory layout:
            // | _ _ _ _ | _ _ _ _ | _ _ _ _ _ _ _ _ _ | _ _ _ _ _ _ _ _ |
            // | state   | id      | request data      | _ _ _ _ _ _ _ _ |
            // state: WaitCliReq=0, WaitSvrResp=1, Disconnected=2
            unsafe {
                let state_ptr = raw_ptr;
                let id_ptr = raw_ptr.add(4) as *mut u32;
                let req_ptr = raw_ptr.add(8) as *mut u64;
                let resp_ptr = raw_ptr.add(16) as *mut u64;

                let mut client_id = 0u32;
                let mut sum = 0u64;
                let mut inst = Instant::now();
                let mut state = raw_ptr.read_volatile();
                loop {
                    match state {
                        ACCEPTING => {
                            let backoff = Backoff::new();
                            loop {
                                backoff.snooze();
                                state = state_ptr.read_volatile();
                                if state != ACCEPTING {
                                    break
                                }
                            }
                        }
                        CONNECTING => {
                            // read client identifier
                            client_id = id_ptr.read_volatile();
                            inst = Instant::now();
                            // transfer state to WAIT_CLI_REQ
                            state = set_state(state_ptr, WAIT_CLI_REQ);
                        }
                        WAIT_CLI_REQ => {
                            let backoff = Backoff::new();
                            loop {
                                backoff.snooze();
                                state = state_ptr.read_volatile();
                                if state != WAIT_CLI_REQ {
                                    break
                                }
                            }
                        }
                        WAIT_SVR_RESP => {
                            let req = req_ptr.read_volatile();
                            sum += req;
                            resp_ptr.write_volatile(sum);
                            // transfer state to WAIT_CLI_REQ
                            state = set_state(state_ptr, WAIT_CLI_REQ);
                        }
                        _ => {
                            let dur = inst.elapsed();
                            println!("disconnected from client {}, sum is {}, duration is {:?}", client_id, sum, dur);
                            client_id = 0;
                            sum = 0;
                            inst = Instant::now();
                            // transfer state to ACCEPTING
                            state = set_state(state_ptr, ACCEPTING);
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
}

impl SvrArgs {
    #[inline]
    pub fn parse_from_env() -> Result<SvrArgs> {
        use lexopt::prelude::*;
        let mut parser = lexopt::Parser::from_env();
        let mut addr = None;
        while let Some(arg) = parser.next()? {
            match arg {
                Short('a') | Long("addr") => {
                    addr = Some(parse_conn_kind(&parser.value()?.to_string_lossy())?);
                }
                _ => return Err(arg.unexpected().into())
            }
        }
        Ok(SvrArgs{addr: addr.unwrap_or_else(|| parse_conn_kind("tcp:127.0.0.1:9001").unwrap())})
    }
}
