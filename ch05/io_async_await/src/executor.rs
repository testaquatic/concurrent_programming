use std::{
    pin::Pin,
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, SyncSender},
    },
    task::{Context, Wake, Waker},
};

struct Task {
    /// 실행하는 코루틴
    future: Mutex<Pin<Box<dyn Future<Output = ()> + Send>>>,
    /// Executor에 스케줄링 하기위한 채널
    sender: SyncSender<Arc<Task>>,
}

impl Wake for Task {
    fn wake(self: Arc<Self>) {
        let self0 = self.clone();
        self.sender.send(self0).unwrap();
    }
}

pub struct Executor {
    // 실행 큐
    sender: SyncSender<Arc<Task>>,
    receiver: Receiver<Arc<Task>>,
}

impl Executor {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::sync_channel(1024);
        Self { sender, receiver }
    }

    /// 새롭게 Task를 생성하기 위한 Spawner
    pub fn get_spawner(&self) -> Spawner {
        Spawner {
            sender: self.sender.clone(),
        }
    }

    pub fn run(&self) {
        // 채널에서 Task를 수신하고 순서대로 실행
        while let Ok(task) = self.receiver.recv() {
            let mut future = task.future.lock().unwrap();
            let waker = Waker::from(task.clone());
            let mut ctx = Context::from_waker(&waker);
            let _ = future.as_mut().poll(&mut ctx);
        }
    }
}

pub struct Spawner {
    sender: SyncSender<Arc<Task>>,
}

impl Spawner {
    pub fn spawn<F>(&self, future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let future = Box::pin(future);
        let task = Arc::new(Task {
            future: Mutex::new(future),
            sender: self.sender.clone(),
        });

        self.sender.send(task).unwrap();
    }
}
