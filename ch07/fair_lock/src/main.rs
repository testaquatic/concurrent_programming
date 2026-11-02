use std::sync::Arc;

use fair_lock::fairlock::{FairLock, NUM_LOCK};

const NUM_LOOP: usize = 100_000;
const NUM_THREADS: usize = 4;

fn main() {
    let lock = Arc::new(FairLock::new(0));
    let mut v = Vec::with_capacity(NUM_LOCK);

    for i in 0..NUM_THREADS {
        let lock0 = Arc::clone(&lock);
        let t = std::thread::spawn(move || {
            for _ in 0..NUM_LOOP {
                let mut data = lock0.lock(i);
                *data += 1;
            }
        });

        v.push(t);
    }

    v.into_iter().try_for_each(|t| t.join()).unwrap();

    println!(
        "COUNT = {} (expected = {})",
        *lock.lock(0),
        NUM_LOOP * NUM_THREADS
    );
}
