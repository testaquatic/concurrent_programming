use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

const NUM_TASKS: usize = 8;

// 락만 하는 태스크
async fn lock_only(v: Arc<Mutex<u64>>) {
    let mut n = v.lock().unwrap();
    *n += 1;
}

// 락 상태에서 await 를 수행하는 태스크
async fn lock_sleep(v: Arc<Mutex<u64>>) {
    let mut n = v.lock().unwrap();
    tokio::time::sleep(Duration::from_secs(10)).await;
    *n += 1;
}

#[tokio::main]
async fn main() {
    let val = Arc::new(Mutex::new(0));
    let mut v = Vec::new();

    let t = tokio::spawn(lock_sleep(Arc::clone(&val)));
    t.await.unwrap();
}
