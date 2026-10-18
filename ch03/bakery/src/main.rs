use std::{
    ptr::{read_volatile, write_volatile},
    sync::atomic::fence,
    thread,
};

/// 쓰레드수
const NUM_THREADS: usize = 4;
/// 각 쓰레드에서 루프 수
const NUM_LOOP: usize = 100_000;

struct BakeryLock {
    /// i번째 쓰레드가 티켓을 획득중이면 entering[i]는 true
    entering: [bool; NUM_THREADS],
    /// i번째 쓰레드의 티켓은 ticket[i]
    tickets: [Option<usize>; NUM_THREADS],
}

impl BakeryLock {
    /// 락 함수
    /// idx는 쓰레드 번호
    fn lock(&mut self, idx: usize) -> LockGuard {
        // 티켓 취득 처리
        fence(std::sync::atomic::Ordering::SeqCst);
        unsafe {
            write_volatile(&mut self.entering[idx], true);
        }
        fence(std::sync::atomic::Ordering::SeqCst);

        // 현재 배포되어 있는 티켓의 최댓값
        let max = self.tickets.iter().fold(0, |acc, ticket| unsafe {
            match read_volatile(ticket) {
                Some(t) => acc.max(t),
                None => acc,
            }
        });

        // 티켓 발급
        let ticket = max + 1;

        unsafe {
            write_volatile(&mut self.tickets[idx], Some(ticket));
        }

        fence(std::sync::atomic::Ordering::SeqCst);
        unsafe {
            write_volatile(&mut self.entering[idx], false);
        }
        fence(std::sync::atomic::Ordering::SeqCst);

        // 대기처리
        for i in 0..NUM_THREADS {
            if i == idx {
                continue;
            }

            // 스레드 i가 티켓 취득 중이면 대기
            while unsafe { read_volatile(&self.entering[i]) } {
                std::hint::spin_loop();
            }

            // 스레드 i와 자신의 순서를 비교해 자신의 순위가 높거나 스레드 i가 처리 중이 아니면 대기 종료
            while let Some(t) = unsafe { read_volatile(&self.tickets[i]) } {
                if ticket < t || (ticket == t && idx < i) {
                    break;
                }
                std::hint::spin_loop();
            }
            std::hint::spin_loop();
        }

        fence(std::sync::atomic::Ordering::SeqCst);

        LockGuard { idx }
    }
}

/// 락 관리용 타입
struct LockGuard {
    idx: usize,
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        fence(std::sync::atomic::Ordering::SeqCst);
        unsafe {
            write_volatile(&mut LOCK.tickets[self.idx], None);
        }
    }
}

static mut LOCK: BakeryLock = BakeryLock {
    entering: [false; NUM_THREADS],
    tickets: [None; NUM_THREADS],
};

static mut COUNT: u64 = 0;

fn main() {
    let mut v = Vec::new();

    for i in 0..NUM_THREADS {
        let th = thread::spawn(move || {
            for _ in 0..NUM_LOOP {
                let _lock = unsafe { (&raw mut LOCK).as_mut().unwrap().lock(i) };
                unsafe {
                    let c = read_volatile(&raw const COUNT);
                    write_volatile(&raw mut COUNT, c + 1);
                }
            }
        });

        v.push(th);
    }

    v.into_iter().for_each(|th| th.join().unwrap());

    println!(
        "count: {} (expected = {})",
        unsafe { COUNT },
        NUM_LOOP * NUM_THREADS
    );
}
