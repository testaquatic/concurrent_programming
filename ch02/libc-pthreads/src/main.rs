use std::{
    ffi::{CStr, c_void},
    mem::MaybeUninit,
    time::Duration,
};

const NUM_THREADS: usize = 10;

extern "C" fn thread_func(arg: *mut c_void) -> *mut c_void {
    let id = arg as usize;
    for i in 0..5 {
        println!("id = {}, i = {}", id, i);
        std::thread::sleep(Duration::from_secs(1));
    }

    c"finished!".as_ptr() as *mut c_void
}

fn main() {
    let mut v = [libc::pthread_t::default(); NUM_THREADS];

    unsafe {
        for (i, v_item) in v.iter_mut().enumerate() {
            if libc::pthread_create(v_item, std::ptr::null(), thread_func, i as *mut c_void) != 0 {
                eprintln!("pthread_create");
                std::process::exit(-1);
            }
        }
    }

    unsafe {
        for v_item in v.iter() {
            let mut value = MaybeUninit::<*mut c_void>::uninit();
            if libc::pthread_join(*v_item, value.as_mut_ptr()) == 0 {
                let c_str = CStr::from_ptr(value.assume_init() as *mut i8);
                println!("msg = {}", c_str.to_str().unwrap());
            } else {
                eprintln!("pthread_join");
                std::process::exit(-1);
            }
        }
    }
}
