use std::thread;

use bankers::banker::Banker;

const NUM_LOOP: usize = 100_000;

fn main() {
    let banker = Banker::new([1, 1], [[1, 1], [1, 1]]);
    let banker0 = banker.clone();

    let philosopher0 = thread::spawn(move || {
        for _ in 0..NUM_LOOP {
            while !banker0.take(0, 0) {}
            while !banker0.take(0, 1) {}

            println!("philosopher0 is eating");

            banker0.release(0, 1);
            banker0.release(0, 0);
        }
    });

    let philosopher1 = thread::spawn(move || {
        for _ in 0..NUM_LOOP {
            while !banker.take(1, 0) {}
            while !banker.take(1, 1) {}

            println!("philosopher1 is eating");

            banker.release(1, 1);
            banker.release(1, 0);
        }
    });

    philosopher0.join().unwrap();
    philosopher1.join().unwrap();
}
