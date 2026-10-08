use std::{ffi::c_void, mem::MaybeUninit, time::Duration};

use libc::{pthread_attr_destroy, pthread_create, pthread_t};

// 스레드용 함수
extern "C" fn thread_func(_: *mut c_void) -> *mut c_void {
    (0..5).into_iter().for_each(|i| {
        println!("i = {}", i);
        std::thread::sleep(Duration::from_secs(1));
    });

    std::ptr::null_mut()
}

fn main() {
    let mut attr = MaybeUninit::uninit();

    unsafe {
        // 어트리뷰트 초기화
        if libc::pthread_attr_init(attr.as_mut_ptr()) != 0 {
            eprintln!("pthread_attr_init");

            std::process::exit(-1);
        }
        let mut attr = attr.assume_init();

        // 디태치 스레드로 설정
        if libc::pthread_attr_setdetachstate(&mut attr, libc::PTHREAD_CREATE_DETACHED) != 0 {
            eprintln!("pthread_attr_setdetachstate");
            std::process::exit(-1);
        }

        // 스레드 생성
        let mut th = pthread_t::default();
        if pthread_create(&mut th, &attr, thread_func, std::ptr::null_mut()) != 0 {
            eprintln!("pthread_create");
            std::process::exit(-1);
        }

        if pthread_attr_destroy(&mut attr) != 0 {
            eprintln!("pthread_attr_destroy");
            std::process::exit(-1);
        }
    }

    std::thread::sleep(Duration::from_secs(7));
}
