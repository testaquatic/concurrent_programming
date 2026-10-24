use std::{
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Waker},
};

struct Hello {
    state: StateHello,
}

enum StateHello {
    Hello,
    World,
    End,
}

impl Hello {
    fn new() -> Self {
        Self {
            state: StateHello::Hello,
        }
    }
}

impl Future for Hello {
    type Output = ();

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        match self.state {
            StateHello::Hello => {
                print!("Hello, ");
                self.state = StateHello::World;
                std::task::Poll::Pending
            }
            StateHello::World => {
                println!("World!");
                self.state = StateHello::End;
                std::task::Poll::Pending
            }
            StateHello::End => std::task::Poll::Ready(()),
        }
    }
}

struct Task {
    hello: Mutex<Pin<Box<dyn Future<Output = ()> + Send>>>,
}

impl Task {
    fn new() -> Self {
        Self {
            hello: Mutex::new(Box::pin(Hello::new())),
        }
    }
}

fn main() {
    let task = Arc::new(Task::new());
    let mut ctx = Context::from_waker(Waker::noop());
    let mut hello = task.hello.lock().unwrap();
    let _ = hello.as_mut().poll(&mut ctx);
    let _ = hello.as_mut().poll(&mut ctx);
    let _ = hello.as_mut().poll(&mut ctx);
}
