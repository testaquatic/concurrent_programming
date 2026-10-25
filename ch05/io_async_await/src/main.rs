use std::io::Write;

use io_async_await::{
    executor::Executor,
    io_selector::{AsyncListener, IOSelector},
};

fn main() {
    let executor = Executor::new();
    let selector = IOSelector::new();
    let spawner = executor.get_spawner();

    let server = async move {
        let listener = AsyncListener::listen("127.0.0.1:10000", selector.clone());

        loop {
            let (mut reader, mut writer, addr) = listener.accept().await;
            println!("{} is connected.", addr);

            spawner.spawn(async move {
                while let Some(buf) = reader.read_line().await {
                    print!("read: {}, {}", addr, buf);
                    writer.write_all(buf.as_bytes()).unwrap();
                    writer.flush().unwrap();
                }
                println!("close: {}", addr);
            });
        }
    };

    executor.get_spawner().spawn(server);
    executor.run();
}
