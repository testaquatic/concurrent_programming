use std::sync::Arc;

const NUM_LOOP: usize = 100_000;
const NUM_THREAD: usize = 4;

fn main() {
    let n = Arc::new(mcs_lock::MCSLock::new(0));
    let mut v = Vec::new();

    for _ in 0..NUM_THREAD {
        let n0 = Arc::clone(&n);
        let t = std::thread::spawn(move || {
            // 노드를 작성하고 락
            let mut node = mcs_lock::MCSNode::new();
            for _ in 0..NUM_LOOP {
                let mut r = n0.lock(&mut node);
                *r += 1;
            }
        });

        v.push(t);
    }

    v.into_iter().try_for_each(|t| t.join()).unwrap();

    // 노드를 작성하고 락
    let mut node = mcs_lock::MCSNode::new();
    let r = n.lock(&mut node);
    println!("COUNT = {} (expected = {})", *r, NUM_LOOP * NUM_THREAD);
}
