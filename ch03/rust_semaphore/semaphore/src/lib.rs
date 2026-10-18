use std::sync::{Arc, Condvar, Mutex};

struct _Inner {
    cnt: Mutex<usize>,
    cond: Condvar,
    max: usize,
}

#[derive(Clone)]
pub struct Semaphore(Arc<_Inner>);

pub struct SemGuard<'a>(&'a Semaphore);

impl<'a> Drop for SemGuard<'a> {
    fn drop(&mut self) {
        self.0.post();
    }
}

impl Semaphore {
    pub fn new(max: usize) -> Self {
        Self(Arc::new(_Inner {
            cnt: Mutex::new(0),
            max,
            cond: Condvar::new(),
        }))
    }

    pub fn wait<'a>(&'a self) -> Result<SemGuard<'a>, String> {
        let mut cnt = self
            .0
            .cond
            .wait_while(self.0.cnt.lock().unwrap(), |cnt| *cnt >= self.0.max)
            .unwrap();
        *cnt += 1;

        Ok(SemGuard(self))
    }

    fn post(&self) {
        let mut cnt = self.0.cnt.lock().unwrap();
        *cnt -= 1;

        if *cnt <= self.0.max {
            self.0.cond.notify_one();
        }
    }
}
