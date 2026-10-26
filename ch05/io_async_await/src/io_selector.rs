use std::{
    collections::{HashMap, VecDeque},
    io::{BufRead, BufReader, BufWriter},
    net::{SocketAddr, TcpListener, TcpStream},
    os::fd::{AsFd, AsRawFd, BorrowedFd, RawFd},
    sync::{Arc, Mutex},
    task::Waker,
};

use nix::{
    poll::PollTimeout,
    sys::{
        epoll::{Epoll, EpollCreateFlags, EpollEvent, EpollFlags},
        eventfd::{EfdFlags, EventFd},
    },
    unistd::write,
};

fn write_eventfd(fd: RawFd, n: usize) {
    let ptr = &n as *const usize as *const u8;
    let val = unsafe { std::slice::from_raw_parts(ptr, std::mem::size_of_val(&n)) };

    unsafe { write(BorrowedFd::borrow_raw(fd), val).unwrap() };
}

enum IOOps {
    Add(EpollFlags, RawFd, Waker),
    Remove(RawFd),
}

pub struct IOSelector {
    wakers: Mutex<HashMap<RawFd, Waker>>,
    queue: Mutex<VecDeque<IOOps>>,
    epfd: Epoll,
    event: EventFd,
}

impl IOSelector {
    pub fn new() -> Arc<Self> {
        let s = Self {
            wakers: Mutex::new(HashMap::new()),
            queue: Mutex::new(VecDeque::new()),
            epfd: Epoll::new(EpollCreateFlags::empty()).unwrap(),
            event: EventFd::from_value_and_flags(0, EfdFlags::empty()).unwrap(),
        };

        let result = Arc::new(s);
        let s = result.clone();
        std::thread::spawn(move || s.select());

        result
    }

    fn add_event(
        &self,
        flag: EpollFlags,
        fd: RawFd,
        waker: Waker,
        wakers: &mut HashMap<RawFd, Waker>,
    ) {
        let mut ev = EpollEvent::new(flag | EpollFlags::EPOLLONESHOT, fd as u64);
        unsafe {
            if let Err(err) = self.epfd.add(BorrowedFd::borrow_raw(fd), ev) {
                match err {
                    nix::Error::EEXIST => {
                        self.epfd
                            .modify(BorrowedFd::borrow_raw(fd), &mut ev)
                            .unwrap();
                    }
                    _ => panic!("epoll_ctl: {}", err),
                }
            }
        }
        assert!(!wakers.contains_key(&fd));
        wakers.insert(fd, waker);
    }

    fn rm_event(&self, fd: RawFd, wakers: &mut HashMap<RawFd, Waker>) {
        unsafe {
            self.epfd.delete(BorrowedFd::borrow_raw(fd)).ok();
        }
        wakers.remove(&fd);
    }

    fn select(&self) {
        let ev = EpollEvent::new(EpollFlags::EPOLLIN, self.event.as_raw_fd() as u64);
        self.epfd.add(self.event.as_fd(), ev).unwrap();

        let mut events = vec![EpollEvent::empty(); 1024];
        while let Ok(nfds) = self.epfd.wait(&mut events, PollTimeout::NONE) {
            let mut t = self.wakers.lock().unwrap();
            for item in events.iter().take(nfds) {
                if item.data() == self.event.as_raw_fd() as u64 {
                    let mut q = self.queue.lock().unwrap();
                    while let Some(op) = q.pop_front() {
                        match op {
                            IOOps::Add(flag, fd, waker) => {
                                self.add_event(flag, fd, waker, &mut t);
                            }
                            IOOps::Remove(fd) => {
                                self.rm_event(fd, &mut t);
                            }
                        }
                    }
                } else {
                    let data = item.data() as RawFd;
                    let waker = t.remove(&data).unwrap();
                    waker.wake_by_ref();
                }
            }
        }
    }

    fn register(&self, flag: EpollFlags, fd: RawFd, waker: Waker) {
        let mut q = self.queue.lock().unwrap();
        q.push_back(IOOps::Add(flag, fd, waker));
        write_eventfd(self.event.as_raw_fd(), 1);
    }

    fn unregister(&self, fd: RawFd) {
        let mut q = self.queue.lock().unwrap();
        q.push_back(IOOps::Remove(fd));
        write_eventfd(self.event.as_raw_fd(), 1);
    }
}

pub struct AsyncListener {
    listener: TcpListener,
    selector: Arc<IOSelector>,
}

impl AsyncListener {
    pub fn listen(addr: &str, selector: Arc<IOSelector>) -> Self {
        let listener = TcpListener::bind(addr).unwrap();
        listener.set_nonblocking(true).unwrap();

        AsyncListener { listener, selector }
    }

    pub fn accept(&self) -> Accept<'_> {
        Accept { listener: self }
    }
}

impl Drop for AsyncListener {
    fn drop(&mut self) {
        self.selector.unregister(self.listener.as_raw_fd());
    }
}

pub struct Accept<'a> {
    listener: &'a AsyncListener,
}

impl<'a> Future for Accept<'a> {
    type Output = (AsyncReader, BufWriter<TcpStream>, SocketAddr);

    fn poll(
        self: std::pin::Pin<&mut Self>,
        ctx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        match self.listener.listener.accept() {
            Ok((stream, addr)) => {
                let stream0 = stream.try_clone().unwrap();
                std::task::Poll::Ready((
                    AsyncReader::new(stream0, self.listener.selector.clone()),
                    BufWriter::new(stream),
                    addr,
                ))
            }
            Err(ref err) => {
                if err.kind() == std::io::ErrorKind::WouldBlock {
                    self.listener.selector.register(
                        EpollFlags::EPOLLIN,
                        self.listener.listener.as_raw_fd(),
                        ctx.waker().clone(),
                    );
                    std::task::Poll::Pending
                } else {
                    panic!("accept: {}", err);
                }
            }
        }
    }
}

pub struct AsyncReader {
    fd: RawFd,
    reader: BufReader<TcpStream>,
    selector: Arc<IOSelector>,
}

impl AsyncReader {
    fn new(stream: TcpStream, selector: Arc<IOSelector>) -> AsyncReader {
        stream.set_nonblocking(true).unwrap();
        Self {
            fd: stream.as_raw_fd(),
            reader: BufReader::new(stream),
            selector,
        }
    }

    pub fn read_line(&mut self) -> ReadLine<'_> {
        ReadLine { reader: self }
    }
}

impl Drop for AsyncReader {
    fn drop(&mut self) {
        self.selector.unregister(self.fd);
    }
}

pub struct ReadLine<'a> {
    reader: &'a mut AsyncReader,
}

impl<'a> Future for ReadLine<'a> {
    type Output = Option<String>;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let mut line = String::new();
        match self.reader.reader.read_line(&mut line) {
            Ok(0) => std::task::Poll::Ready(None),
            Ok(_) => std::task::Poll::Ready(Some(line)),
            Err(ref err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                self.reader.selector.register(
                    EpollFlags::EPOLLIN,
                    self.reader.fd,
                    cx.waker().clone(),
                );
                std::task::Poll::Pending
            }
            Err(_) => std::task::Poll::Ready(None),
        }
    }
}
