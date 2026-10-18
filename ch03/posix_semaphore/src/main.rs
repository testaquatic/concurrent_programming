use std::{ffi::CStr, sync::atomic::AtomicUsize, time::Duration};

const NUM_THREADS: usize = 10;
const NUM_LOOP: usize = 10;

const SEM_NAME: &CStr = c"/mysemaphore7";

static COUNT: AtomicUsize = AtomicUsize::new(0);

fn th() {
    unsafe {
        // 이름 있는 세마포어를 연다.
        let s = libc::sem_open(SEM_NAME.as_ptr(), 0);
        if s == libc::SEM_FAILED {
            panic!("sem_open");
        }

        for _ in 0..NUM_LOOP {
            // 대기
            if libc::sem_wait(s) == -1 {
                panic!("sem_wait");
            }

            // 카운터를 아토믹하게 증가
            COUNT.fetch_add(1, std::sync::atomic::Ordering::Acquire);
            println!(
                "count = {}",
                COUNT.load(std::sync::atomic::Ordering::Relaxed)
            );

            // 10ms 슬립
            std::thread::sleep(Duration::from_millis(100));

            // 카운터를 아토믹하게 감소
            COUNT.fetch_sub(1, std::sync::atomic::Ordering::Release);

            // 세마포어 값을 증가시킨다.
            if libc::sem_post(s) == -1 {
                panic!("sem_post");
            }
        }

        // 세마포어를 닫는다.
        if libc::sem_close(s) == -1 {
            panic!("sem_close");
        }
    }
}

fn main() {
    let s = unsafe {
        // 이름이 붙은 세마포어를 연다.
        let s = libc::sem_open(SEM_NAME.as_ptr(), libc::O_CREAT, 0o0660, 3);
        if s == libc::SEM_FAILED {
            panic!("sem_open");
        }
        s
    };
    // 스레드 생성
    let v = (0..NUM_THREADS)
        .map(|_idx| std::thread::spawn(th))
        .collect::<Vec<_>>();

    // 조인
    v.into_iter().try_for_each(|h| h.join()).expect("조인 실패");

    unsafe {
        // 세마포어를 닫는다
        if libc::sem_close(s) == -1 {
            panic!("sem_close");
        }

        // 세마포어 파기
        if libc::sem_unlink(SEM_NAME.as_ptr()) == -1 {
            panic!("sem_unlink");
        }
    }
}
