use channel::channel;

fn main() {
    let (tx, rx) = channel(4);
    let mut v = Vec::new();

    let t = std::thread::spawn(move || {
        let mut cnt = 0;
        while cnt < 4 * 100_000 {
            let n = rx.recv();
            println!("recv: n = {:?}", n);
            cnt += 1;
        }
    });

    v.push(t);

    for i in 0..4 {
        let tx0 = tx.clone();
        let t = std::thread::spawn(move || {
            for j in 0..100_000 {
                tx0.send((i, j));
            }
        });

        v.push(t);
    }

    for i in v {
        i.join().unwrap();
    }
}
