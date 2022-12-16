use std::io::{self, Read, Write};
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicU8, Ordering};
use std::mem::{self, align_of, MaybeUninit};
use libc::{
    pthread_mutex_init,
    pthread_mutex_lock,
    pthread_mutex_t,
    pthread_mutex_unlock,
    pthread_mutexattr_init,
    pthread_mutexattr_setpshared,
    pthread_mutexattr_t,
    pthread_condattr_init,
    pthread_condattr_setpshared,
    pthread_condattr_t,
    pthread_cond_init,
    pthread_cond_signal,
    pthread_cond_wait,
    pthread_cond_t,
    PTHREAD_PROCESS_SHARED,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("unknown protocol")]
    UnknownProtocol,
    #[error("unknown state")]
    UnknownState,
    #[error("fail to initialize pthread_mutexattr_t")]
    FailInitPthreadMutexAttr,
    #[error("fail to setup pthread_mutexattr_t")]
    FailSetupPthreadMutexAttr,
    #[error("fail to initialize pthread_mutex_t")]
    FailInitPthreadMutex,
    #[error("fail to initialize pthread_condattr_t")]
    FailInitPthreadCondAttr,
    #[error("fail to setup pthread_condattr_t")]
    FailSetupPthreadCondAttr,
    #[error("fail to initialize pthread_cond_t")]
    FailInitPthreadCond,
    #[error("fail to lock pthread_mutex_t with code {0}")]
    FailPthreadLock(i32),
    #[error("fail to unlock pthread_mutex_t with code {0}")]
    FailPthreadUnlock(i32),
    #[error("fail to wait pthread_cond_t with code {0}")]
    FailPthreadWait(i32),
    #[error("fail to signal pthread_cond_t with code {0}")]
    FailPthreadSignal(i32),
}

pub type Result<T> = std::result::Result<T, Error>;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CabinState {
    AcceptingSpin = 0,
    AcceptingYield = 1,
    Connecting = 2,
    WaitReqSpin = 3,
    WaitReqYield = 4,
    WaitRespSpin = 5,
    WaitRespYield = 6,
    Disconnected = 7,
}

impl From<u8> for CabinState {
    #[inline]
    fn from(src: u8) -> Self {
        use CabinState::*;
        match src {
            0 => AcceptingSpin,
            1 => AcceptingYield,
            2 => Connecting,
            3 => WaitReqSpin,
            4 => WaitReqYield,
            5 => WaitRespSpin,
            6 => WaitRespYield,
            7 | _ => Disconnected,
        }
    }
}

pub struct CabinGuard;

pub struct LockGuard<'a, T, U> {
    cabin: &'a Cabin<T, U>,
}

impl<'a, T, U> LockGuard<'a, T, U> {
    pub fn wait(&self) -> Result<()> {
        self.cabin.wait()
    }

    pub fn signal(&self) -> Result<()> {
        self.cabin.signal()
    }
}

impl<'a, T, U> Drop for LockGuard<'a, T, U> {
    fn drop(&mut self) {
        self.cabin.unlock().unwrap();
    }
}

#[repr(C)]
pub struct Cabin<T, U> {
    mutex: UnsafeCell<pthread_mutex_t>,
    cond: UnsafeCell<pthread_cond_t>,
    state: AtomicU8,
    id: UnsafeCell<u32>,
    req: UnsafeCell<T>,
    resp: UnsafeCell<U>,
}

impl<T, U> Cabin<T, U> {

    #[inline]
    pub unsafe fn new<'a>(mem: *mut u8, _guard: &'a CabinGuard) -> Result<&'a Self> {
        let padding = mem.align_offset(align_of::<Self>());
        let ptr = mem.add(padding);
        let cabin = mem::transmute::<*mut u8, &mut Self>(ptr);
        // initialize pthread mutex
        let mut lock_attr: pthread_mutexattr_t = MaybeUninit::zeroed().assume_init();
        if pthread_mutexattr_init(&mut lock_attr) != 0 {
            return Err(Error::FailInitPthreadMutexAttr)
        }
        if pthread_mutexattr_setpshared(&mut lock_attr, PTHREAD_PROCESS_SHARED) != 0 {
            return Err(Error::FailSetupPthreadMutexAttr)
        }
        if pthread_mutex_init(cabin.mutex.get(), &lock_attr) != 0 {
            return Err(Error::FailInitPthreadMutex)
        }
        // initialize pthread cond
        let mut cond_attr: pthread_condattr_t = MaybeUninit::zeroed().assume_init();
        if pthread_condattr_init(&mut cond_attr) != 0 {
            return Err(Error::FailInitPthreadCondAttr)
        }
        if pthread_condattr_setpshared(&mut cond_attr, PTHREAD_PROCESS_SHARED) != 0 {
            return Err(Error::FailSetupPthreadCondAttr)
        }
        if pthread_cond_init(cabin.cond.get(), &cond_attr) != 0 {
            return Err(Error::FailInitPthreadCond)
        }
        Ok(cabin)
    }

    #[inline]
    pub unsafe fn from_existing<'a>(mem: *mut u8, _guard: &'a CabinGuard) -> &'a Self {
        let padding = mem.align_offset(align_of::<Self>());
        let ptr = mem.add(padding);
        mem::transmute::<*mut u8, &Self>(ptr)
    }

    pub fn id(&self) -> u32 {
        unsafe { self.id.get().read_volatile() }
    }

    pub fn set_id(&self, id: u32) {
        unsafe { self.id.get().write_volatile(id) }
    }

    pub fn req(&self) -> T where T: Copy {
        unsafe { *self.req.get() }
    }

    pub fn set_req(&self, req: T) {
        unsafe { self.req.get().write_volatile(req) }
    }

    pub fn resp(&self) -> U where U: Copy {
        unsafe { *self.resp.get() }
    }

    pub fn set_resp(&self, resp: U) {
        unsafe { self.resp.get().write_volatile(resp) }
    }

    #[inline]
    pub fn load_state(&self, order: Ordering) -> CabinState {
        self.state.load(order).into()
    }

    #[inline]
    pub fn cas_state(&self, current: CabinState, new: CabinState) -> std::result::Result<CabinState, CabinState> {
        self.state.compare_exchange_weak(current as u8, new as u8, Ordering::SeqCst, Ordering::SeqCst)
            .map(|s| s.into())
            .map_err(|s| s.into())
    }

    #[inline]
    pub fn lock(&self) -> Result<LockGuard<'_, T, U>> {
        let res = unsafe { pthread_mutex_lock(self.mutex.get()) };
        if res != 0 {
            return Err(Error::FailPthreadLock(res))
        }
        Ok(LockGuard{cabin: self})
    }

    #[inline]
    fn unlock(&self) -> Result<()> {
        let res = unsafe { pthread_mutex_unlock(self.mutex.get()) };
        if res != 0 {
            return Err(Error::FailPthreadUnlock(res))
        }
        Ok(())
    }

    #[inline]
    fn wait(&self) -> Result<()> {
        let res = unsafe { pthread_cond_wait(self.cond.get(), self.mutex.get()) };
        if res != 0 {
            return Err(Error::FailPthreadWait(res))
        }
        Ok(())
    }

    #[inline]
    fn signal(&self) -> Result<()> {
        let res = unsafe { pthread_cond_signal(self.cond.get()) };
        if res != 0 {
            return Err(Error::FailPthreadSignal(res))
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum ConnKind {
    Tcp,
    Unix,
    Shm,
}

#[inline]
pub fn parse_conn_kind(s: &str) -> Result<(ConnKind, String)> {
    for (proto, kind) in [
        ("tcp", ConnKind::Tcp), 
        ("unix", ConnKind::Unix), 
        ("shm", ConnKind::Shm),
    ] {
        if s.starts_with(proto) {
            return Ok((kind, s[proto.len()+1..].to_string()))
        }
    }
    Err(Error::UnknownProtocol)
}

#[inline]
pub fn client_conn<T>(mut conn: T, value: Option<u64>, num: u32) -> anyhow::Result<u64> 
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
pub fn server_conn<T>(mut conn: T) -> anyhow::Result<u64> 
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
