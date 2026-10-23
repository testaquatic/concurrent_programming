use std::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    sync::atomic::AtomicBool,
};

/// 스핀락용 타입
pub struct SpinLock<T> {
    /// 락용 공용 변수
    lock: AtomicBool,
    /// 보호 대상 데이터
    data: UnsafeCell<T>,
}

/// 락 해제 및 락 중에 보호 대상 데이터를 조작하기 위한 타입
pub struct SpinLockGuard<'a, T> {
    spin_lock: &'a SpinLock<T>,
}

impl<T> SpinLock<T> {
    pub fn new(v: T) -> Self {
        Self {
            lock: AtomicBool::new(false),
            data: UnsafeCell::new(v),
        }
    }

    /// 락 함수
    pub fn lock(&self) -> SpinLockGuard<'_, T> {
        loop {
            // 락용 공유 변수가 false가 될 때까지 대기
            while self.lock.load(std::sync::atomic::Ordering::Relaxed) {
                std::hint::spin_loop();
            }

            // 락용 공유 변수를 아토믹하게 씀
            if self
                .lock
                .compare_exchange_weak(
                    false,
                    true,
                    std::sync::atomic::Ordering::Acquire,
                    std::sync::atomic::Ordering::Relaxed,
                )
                .is_ok()
            {
                break;
            }
        }

        SpinLockGuard { spin_lock: self }
    }
}

unsafe impl<T> Sync for SpinLock<T> {}

unsafe impl<T> Send for SpinLock<T> {}

/// 락 회득후 자동으로 해제
impl<T> Drop for SpinLockGuard<'_, T> {
    fn drop(&mut self) {
        self.spin_lock
            .lock
            .store(false, std::sync::atomic::Ordering::Release);
    }
}

/// 보호 대상 데이터의 이뮤터블한 참조 제외
impl<'a, T> Deref for SpinLockGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.spin_lock.data.get() }
    }
}

/// 보호 대상 데이터의 뮤터블한 참조 제외
impl<'a, T> DerefMut for SpinLockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.spin_lock.data.get() }
    }
}
