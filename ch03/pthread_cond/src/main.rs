use std::{
    io::{BufRead, Write},
    ptr::{read_volatile, write_volatile},
};

static mut BUF: String = String::new();
static mut MUTEX_LOCK: libc::pthread_mutex_t = libc::PTHREAD_MUTEX_INITIALIZER;
static mut COND: libc::pthread_cond_t = libc::PTHREAD_COND_INITIALIZER;
static mut READY: bool = false;

unsafe fn producer() {
    print!("producer: ");
    std::io::stdout().flush().unwrap();

    unsafe {
        (&mut *(&raw mut BUF)).clear();
        std::io::stdin()
            .lock()
            .read_line(&mut *(&raw mut BUF))
            .unwrap();

        libc::pthread_mutex_lock(&raw mut MUTEX_LOCK);
        write_volatile(&raw mut READY, true);

        if libc::pthread_cond_broadcast(&raw mut COND) != 0 {
            panic!("pthread_cond_broadcast");
        }

        libc::pthread_mutex_unlock(&raw mut MUTEX_LOCK);
    }
}

unsafe fn consumer() {
    unsafe {
        libc::pthread_mutex_lock(&raw mut MUTEX_LOCK);
        while !read_volatile(&raw mut READY) {
            if libc::pthread_cond_wait(&raw mut COND, &raw mut MUTEX_LOCK) != 0 {
                panic!("pthread_cond_wait");
            }
        }

        libc::pthread_mutex_unlock(&raw mut MUTEX_LOCK);
        println!("consumer: {}", *(&raw const BUF));
    }
}

fn main() {
    unsafe {
        let pr = std::thread::spawn(|| producer());

        let cn = std::thread::spawn(move || consumer());

        pr.join().unwrap();
        cn.join().unwrap();

        libc::pthread_mutex_destroy(&raw mut MUTEX_LOCK);

        if (libc::pthread_cond_destroy(&raw mut COND)) != 0 {
            panic!("pthread_cond_destroy");
        }
    }
}
