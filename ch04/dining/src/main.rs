use std::{
    sync::{Arc, Mutex},
    thread,
};

fn main() {
    // 포크
    let c0 = Arc::new(Mutex::new(()));
    let c1 = Arc::new(Mutex::new(()));

    let c0_p0 = Arc::clone(&c0);
    let c1_p0 = Arc::clone(&c1);

    // 철학자 1
    let p0 = thread::spawn(move || {
        for _ in 0..100_000 {
            let _n1 = c0_p0.lock().unwrap();
            let _n2 = c1_p0.lock().unwrap();
            println!("0: eating");
        }
    });

    // 철학자 2
    let p1 = thread::spawn(move || {
        for _ in 0..100_000 {
            let _n1 = c1.lock().unwrap();
            let _n2 = c0.lock().unwrap();
            println!("1: eating");
        }
    });

    p0.join().unwrap();
    p1.join().unwrap();
}
