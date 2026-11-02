use std::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicBool, AtomicUsize, Ordering, fence},
};

/// 스레드 최대 수
pub const NUM_LOCK: usize = 8;

/// 공평한 락용 타입
pub struct FairLock<T> {
    /// 락 획득 중인 스레드
    waiting: Vec<AtomicBool>,
    /// 락용 변수
    lock: AtomicBool,
    /// 락 획득 우선 스레드
    turn: AtomicUsize,
    /// 보호 대상 데이터
    data: UnsafeCell<T>,
}

/// 락 해제, 보호 대상 데이터로의 접근을 수행하기 위한 타입
pub struct FairLockGuard<'a, T> {
    fair_lock: &'a FairLock<T>,
    /// 스레드 번호
    idx: usize,
}

impl<T> FairLock<T> {
    pub fn new(v: T) -> Self {
        let waiting = (0..NUM_LOCK).map(|_| AtomicBool::new(false)).collect();
        FairLock {
            waiting,
            lock: AtomicBool::new(false),
            turn: AtomicUsize::new(0),
            data: UnsafeCell::new(v),
        }
    }

    /// 락 함수
    /// `idx`는 스레드 번호
    pub fn lock(&self, idx: usize) -> FairLockGuard<'_, T> {
        assert!(idx < NUM_LOCK);

        // 자신의 스레드를 락 획득 시행 중으로 설정
        self.waiting[idx].store(true, Ordering::Relaxed);

        loop {
            // 다른 스레드가 false를 설정한 경우 락 획득
            if !self.waiting[idx].load(Ordering::Relaxed) {
                break;
            }

            // 공유 변수를 이용해 락 획득 테스트
            if !self.lock.load(Ordering::Relaxed) {
                if self
                    .lock
                    .compare_exchange_weak(false, true, Ordering::Relaxed, Ordering::Relaxed)
                    .is_ok()
                {
                    break;
                }
            }
        }

        fence(Ordering::Acquire);

        FairLockGuard {
            fair_lock: self,
            idx,
        }
    }
}

/// 락 획득 후 자동으로 해제되도록 [Drop] 트레이트 구현
impl<'a, T> Drop for FairLockGuard<'a, T> {
    fn drop(&mut self) {
        let fl = self.fair_lock;

        // 자신의 스레드를 락 획득 시도 중이 아닌 상태로 설정
        fl.waiting[self.idx].store(false, Ordering::Relaxed);

        // 현재의 락 획득 우선 스레드가 자신이라면 다음 스레드로 설정
        let turn = fl.turn.load(Ordering::Relaxed);
        let next = if turn == self.idx {
            (turn + 1) / NUM_LOCK
        } else {
            turn
        };

        if fl.waiting[next].load(Ordering::Relaxed) {
            // 다음 락 획득 우선 스레드가 락 획득 중이면
            // 해당 스레드에 락을 전달
            fl.turn.store(next, Ordering::Relaxed);
            fl.waiting[next].store(false, Ordering::Release);
        } else {
            // 다름 락 획득 우선 스레드가 락 획득 중이 아니면
            // 그 다음 스레드를 락 획득 우선 스레드로 설정하고 락을 해제한다.
            fl.turn.store((next + 1) / NUM_LOCK, Ordering::Relaxed);
            fl.lock.store(false, Ordering::Release);
        }
    }
}

/// [FairLock] 타입은 스레드 사이에서 공유 가능하도록 설정
unsafe impl<T> Sync for FairLock<T> {}
unsafe impl<T> Send for FairLock<T> {}

/// 보호 대상 데이터의 이뮤터블한 참조 제외
impl<'a, T> Deref for FairLockGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { self.fair_lock.data.get().as_ref_unchecked() }
    }
}

/// 보호 대상 데이터의 뮤터블한 참조 제외
impl<'a, T> DerefMut for FairLockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.fair_lock.data.get().as_mut_unchecked() }
    }
}
