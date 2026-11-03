use std::{
    cell::UnsafeCell,
    hint,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicUsize, Ordering, fence},
};

/// 티켓락용 타입
pub struct TicketLock<T> {
    /// 티켓
    ticket: AtomicUsize,
    /// 실행 가능한 티켓
    turn: AtomicUsize,
    data: UnsafeCell<T>,
}

/// 락 해제, 보호 대상 데이터로의 접근을 수행하기 위한 타입
pub struct TicketLockGuard<'a, T> {
    ticket_lock: &'a TicketLock<T>,
}

impl<T> TicketLock<T> {
    pub fn new(v: T) -> Self {
        Self {
            ticket: AtomicUsize::new(0),
            turn: AtomicUsize::new(0),
            data: UnsafeCell::new(v),
        }
    }

    /// 락용 함수
    pub fn lock(&self) -> TicketLockGuard<'_, T> {
        // 티켓 취득
        let t = self.ticket.fetch_add(1, Ordering::Relaxed);
        // 소유한 티켓의 순서가 될 때까지 스핀
        while self.turn.load(Ordering::Relaxed) != t {
            hint::spin_loop();
        }
        fence(Ordering::Acquire);

        TicketLockGuard { ticket_lock: self }
    }
}

/// 락 획득 후 자동으로 해제되도록 [Drop] 트레이트 구현
impl<'a, T> Drop for TicketLockGuard<'a, T> {
    fn drop(&mut self) {
        // 다음 티켓을 실행 가능하도록 설정
        self.ticket_lock.turn.fetch_add(1, Ordering::Release);
    }
}

/// 참조 해제
impl<'a, T> Deref for TicketLockGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { self.ticket_lock.data.get().as_ref_unchecked() }
    }
}

/// 가변 참조 해제
impl<'a, T> DerefMut for TicketLockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.ticket_lock.data.get().as_mut_unchecked() }
    }
}
