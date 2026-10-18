use std::{
    collections::LinkedList,
    hint::spin_loop,
    sync::{Arc, Condvar, Mutex, atomic::AtomicUsize},
};

#[derive(Clone)]
pub struct Sender<T> {
    cnt: Arc<AtomicUsize>,
    max: usize,
    buf: Arc<Mutex<LinkedList<T>>>,
    cond: Arc<Condvar>,
}

impl<T: Send> Sender<T> {
    pub fn send(&self, data: T) {
        while self
            .cnt
            .try_update(
                std::sync::atomic::Ordering::Acquire,
                std::sync::atomic::Ordering::Relaxed,
                |cnt| if cnt >= self.max { None } else { Some(cnt + 1) },
            )
            .is_ok()
        {
            spin_loop();
        }

        let mut buf = self.buf.lock().unwrap();
        buf.push_back(data);
        self.cond.notify_one();
    }
}

pub struct Reciever<T> {
    cnt: Arc<AtomicUsize>,
    buf: Arc<Mutex<LinkedList<T>>>,
    cond: Arc<Condvar>,
}

impl<T: Send> Reciever<T> {
    pub fn recv(&self) -> T {
        let mut buf = self.buf.lock().unwrap();
        loop {
            if let Some(data) = buf.pop_front() {
                self.cnt.fetch_sub(1, std::sync::atomic::Ordering::Release);
                return data;
            }
            buf = self.cond.wait(buf).unwrap();
        }
    }
}

pub fn channel<T>(max: usize) -> (Sender<T>, Reciever<T>) {
    assert_ne!(max, 0);

    let cnt = Arc::new(AtomicUsize::new(0));
    let buf = Arc::new(Mutex::new(LinkedList::new()));
    let cond = Arc::new(Condvar::new());

    let tx = Sender {
        cnt: cnt.clone(),
        max,
        buf: buf.clone(),
        cond: cond.clone(),
    };

    let rx = Reciever { cnt, buf, cond };

    (tx, rx)
}
