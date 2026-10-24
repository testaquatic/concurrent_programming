use std::{
    collections::HashMap,
    io::{BufRead, BufReader, BufWriter, Write},
    net::TcpListener,
    os::fd::AsRawFd,
};

use libc::epoll_wait;

fn main() {
    // epoll 플래그
    let epoll_in = libc::EPOLLIN;
    let epoll_add = libc::EPOLL_CTL_ADD;
    let epoll_del = libc::EPOLL_CTL_DEL;

    let listener = TcpListener::bind("127.0.0.1:10000").unwrap();

    unsafe {
        // epoll용 객체 생성
        let epfd = libc::epoll_create1(libc::O_CLOEXEC);

        // 소켓을 감시 대상에 추가
        let listen_fd = listener.as_raw_fd();
        let mut ev = libc::epoll_event {
            events: epoll_in as u32,
            u64: listen_fd as u64,
        };
        libc::epoll_ctl(epfd, epoll_add, listen_fd, &mut ev);

        let mut fd2buf = HashMap::new();
        let mut events = vec![libc::epoll_event { events: 0, u64: 0 }; 1024];

        // 이벤트 발생 감시
        loop {
            let nfds = epoll_wait(
                epfd,
                events.as_mut_ptr() as *mut libc::epoll_event,
                events.len() as i32,
                -1,
            );

            if nfds < 0 {
                break;
            }

            for n in 0..nfds {
                let fd = events[n as usize].u64 as i32;
                if fd == listen_fd {
                    // 리슨 소켓에 이벤트
                    if let Ok((stream, _)) = listener.accept() {
                        let fd = stream.as_raw_fd();
                        let stream0 = stream.try_clone().unwrap();
                        let reader = BufReader::new(stream0);
                        let writer = BufWriter::new(stream);

                        fd2buf.insert(fd, (reader, writer));

                        println!("accept: fd = {}", fd);

                        let mut ev = libc::epoll_event {
                            events: epoll_in as u32,
                            u64: fd as u64,
                        };
                        libc::epoll_ctl(epfd, epoll_add, fd, &mut ev);
                    }
                } else {
                    // 클라이언트에서 데이터 도착
                    let fd = events[n as usize].u64 as i32;
                    let (reader, writer) = fd2buf.get_mut(&fd).unwrap();

                    // 커넥션을 클로즈 한 경우 감시대상에서 제외
                    let mut buf = String::new();
                    let n = reader.read_line(&mut buf).unwrap();
                    if n == 0 {
                        let mut ev = libc::epoll_event {
                            events: epoll_in as u32,
                            u64: fd as u64,
                        };

                        libc::epoll_ctl(epfd, epoll_del, fd, &mut ev);
                        fd2buf.remove(&fd);
                        println!("close: fd = {}", fd);
                        continue;
                    }

                    print!("read: fd = {}, buf = {}", fd, buf);

                    writer.write_all(buf.as_bytes()).unwrap();
                    writer.flush().unwrap();
                }
            }
        }
    }
}
