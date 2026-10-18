use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use semaphore::Semaphore;

fn main() {
    let mut v = Vec::new();
    let cnt = Arc::new(AtomicUsize::new(0));

    let sem = Semaphore::new(4);
    for i in 0..8 {
        let s = sem.clone();
        let cnt = cnt.clone();
        let t = std::thread::spawn(move || {
            for _ in 0..100_000 {
                let _g = s.wait();

                cnt.fetch_add(1, Ordering::SeqCst);
                let n = cnt.load(Ordering::SeqCst);
                println!("semaphore: i = {}, cnt = {}", i, n);
                assert!(n <= 4);
                cnt.fetch_sub(1, Ordering::SeqCst);
            }
        });
        v.push(t);
    }

    for t in v {
        t.join().unwrap();
    }
}
