use std::sync::mpsc::{self, Sender};

fn main() {
    let mut v = Vec::new();

    // 채널 작성
    let (tx, rx) = mpsc::channel::<Sender<()>>();

    // 배리어 동기용 스레드
    let barrier = move || {
        let x = rx.recv().unwrap();
        let y = rx.recv().unwrap();
        let z = rx.recv().unwrap();

        println!("send!");

        x.send(()).unwrap();
        y.send(()).unwrap();
        z.send(()).unwrap();
    };

    let t = std::thread::spawn(barrier);
    v.push(t);

    // 클라이언트 스레드
    for _ in 0..3 {
        let tx_c = tx.clone();
        let node = move || {
            // 배리어 동기
            let (tx0, rx0) = mpsc::channel();
            tx_c.send(tx0).unwrap();
            rx0.recv().unwrap();
            println!("received!");
        };
        let t = std::thread::spawn(node);
        v.push(t);
    }

    v.into_iter().try_for_each(|t| t.join()).unwrap();
}
