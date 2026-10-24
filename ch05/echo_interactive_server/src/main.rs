use std::{
    io::{BufRead, BufReader, BufWriter, Write},
    net::TcpListener,
};

fn main() {
    let listner = TcpListener::bind("127.0.0.1:10000").unwrap();

    while let Ok((stream, _)) = listner.accept() {
        let stream0 = stream.try_clone().unwrap();
        let reader = BufReader::new(stream0);
        let mut writer = BufWriter::new(stream);

        for line in reader.lines() {
            writer.write_all(line.unwrap().as_bytes()).unwrap();
            writer.flush().unwrap();
        }
    }
}
