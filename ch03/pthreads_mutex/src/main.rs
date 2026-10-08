use std::time::Duration;

static mut MUTEX_T: libc::pthread_mutex_t = libc::PTHREAD_MUTEX_INITIALIZER;

extern "C" fn some_func(_: *mut std::ffi::c_void) -> *mut std::ffi::c_void {
    unsafe {
        if libc::pthread_mutex_lock(&raw mut MUTEX_T) != 0 {
            panic!("pthread_mutex_lock");
        }
    }
    println!("작업 시작");
    std::thread::sleep(Duration::from_secs(2));
    println!("작업 끝");

    unsafe {
        if libc::pthread_mutex_unlock(&raw mut MUTEX_T) != 0 {
            panic!("pthread_mutex_unlock");
        }
    }

    std::ptr::null_mut()
}

fn main() {
    let (mut th1, mut th2) = (libc::pthread_t::default(), libc::pthread_t::default());
    unsafe {
        if libc::pthread_create(&mut th1, std::ptr::null(), some_func, std::ptr::null_mut()) != 0 {
            panic!("pthread_create");
        }

        if libc::pthread_create(&mut th2, std::ptr::null(), some_func, std::ptr::null_mut()) != 0 {
            panic!("pthread_create");
        }

        if libc::pthread_join(th1, std::ptr::null_mut()) != 0 {
            panic!("pthread_join");
        }

        if libc::pthread_join(th2, std::ptr::null_mut()) != 0 {
            panic!("pthread_join");
        }

        if libc::pthread_mutex_destroy(&raw mut MUTEX_T) != 0 {
            panic!("pthread_mutex_destroy");
        }
    }
}
