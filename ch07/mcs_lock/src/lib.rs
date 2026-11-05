use std::{
    cell::UnsafeCell,
    hint,
    ops::{Deref, DerefMut},
    ptr::{self, null_mut},
    sync::atomic::{AtomicBool, AtomicPtr, Ordering, fence},
};

pub struct MCSLock<T> {
    /// 큐의 맨 마지막
    last: AtomicPtr<MCSNode<T>>,
    /// 보호 대상 데이터
    data: UnsafeCell<T>,
}

pub struct MCSNode<T> {
    /// 다음 노드
    next: AtomicPtr<MCSNode<T>>,
    /// `true`면 락 획득 중
    locked: AtomicBool,
}

pub struct MCSLockGuard<'a, T> {
    /// 자신의 스레드 노드
    node: &'a mut MCSNode<T>,
    /// 큐의 가장 마지막과 보호 대상 데이터 참조
    mcs_lock: &'a MCSLock<T>,
}

/// 스레드끼리의 데이터 공유 및 채널을 이용한 송수신 기능 설정
unsafe impl<T> Sync for MCSLock<T> {}
unsafe impl<T> Send for MCSLock<T> {}

impl<T> MCSNode<T> {
    pub fn new() -> Self {
        Self {
            next: AtomicPtr::new(null_mut()),
            locked: AtomicBool::new(false),
        }
    }
}

impl<T> Default for MCSNode<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// 보호 대상 데이터의 불변 참조 제외
impl<'a, T> Deref for MCSLockGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { self.mcs_lock.data.get().as_ref_unchecked() }
    }
}

/// 보호 대상 데이터의 가변 참조 제외
impl<'a, T> DerefMut for MCSLockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.mcs_lock.data.get().as_mut_unchecked() }
    }
}

impl<T> MCSLock<T> {
    pub fn new(v: T) -> Self {
        Self {
            last: AtomicPtr::new(ptr::null_mut()),
            data: UnsafeCell::new(v),
        }
    }

    pub fn lock<'a>(&'a self, node: &'a mut MCSNode<T>) -> MCSLockGuard<'a, T> {
        // 자기 스레드용 노드를 초기화
        node.next = AtomicPtr::new(null_mut());
        node.locked = AtomicBool::new(false);

        let guard = MCSLockGuard {
            node,
            mcs_lock: self,
        };

        // 자신을 큐의 맨 마지막으로 한다
        let ptr = guard.node as *mut MCSNode<T>;
        let prev = self.last.swap(ptr, Ordering::Relaxed);

        // 맨 마지막이 `null`이면 아무도 락을 획득하지 않는 것이므로 락을 획득
        // null이 아닌 경우에는 자신을 큐의 맨 끝에 추가
        if !prev.is_null() {
            // 락 획득중이라고 설정
            guard.node.locked.store(true, Ordering::Relaxed);

            // 자신을 큐의 맨 끝에 추가
            let prev = unsafe { prev.as_ref_unchecked() };
            prev.next.store(ptr, Ordering::Relaxed);

            // 다른 스레드에서 `locked`변수가 `false`로 설정될 때까지 스핀
            while guard.node.locked.load(Ordering::Relaxed) {
                hint::spin_loop();
            }
        }

        fence(Ordering::Acquire);

        guard
    }
}

impl<'a, T> Drop for MCSLockGuard<'a, T> {
    fn drop(&mut self) {
        // 자신의 다음 노드가 `null`이고 자신이 맨 끝 노드라면 맨 끝을 `null`로 설정한다.
        if self.node.next.load(Ordering::Relaxed).is_null() {
            let ptr = self.node as *mut MCSNode<T>;
            if self
                .mcs_lock
                .last
                .compare_exchange(ptr, null_mut(), Ordering::Release, Ordering::Relaxed)
                .is_ok()
            {
                return;
            }
        }

        // 자신의 다음 스레드가 [MCSLock::lock]함수를 실행중이므로 종료할 때까지 대기한다.
        while self.node.next.load(Ordering::Relaxed).is_null() {
            hint::spin_loop();
        }

        // 자신의 다음 스레드를 실행 가능하게 설정한다.
        let next = unsafe { self.node.next.load(Ordering::Relaxed).as_mut_unchecked() };
        next.locked.store(false, Ordering::Release);
    }
}
