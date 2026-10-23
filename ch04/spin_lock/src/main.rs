use std::{ops::Deref, sync::Arc};

use spin_lock::SpinLock;

const NUM_THREAD: usize = 4;
const NUM_LOOP: usize = 100_000;
fn main() {
    let lock = Arc::new(SpinLock::new(0));
    let mut v = Vec::new();

    for _ in 0..NUM_THREAD {
        let lock0 = Arc::clone(&lock);

        let t = std::thread::spawn(move || {
            for _ in 0..NUM_LOOP {
                let mut data = lock0.lock();
                *data += 1;
            }
        });

        v.push(t);
    }

    v.into_iter().try_for_each(|t| t.join()).unwrap();

    println!(
        "COUNT = {} (expected = {})",
        lock.lock().deref(),
        NUM_THREAD * NUM_LOOP
    );
}
