use std::{
    ptr,
    sync::atomic::{AtomicPtr, Ordering},
};

/// 스택의 노드
/// 리스트 구조로 관리
struct Node<T> {
    next: AtomicPtr<Node<T>>,
    data: T,
}

/// 스택의 선두
pub struct StackBad<T> {
    head: AtomicPtr<Node<T>>,
}

impl<T> StackBad<T> {
    pub fn new() -> Self {
        Self {
            head: AtomicPtr::new(ptr::null_mut()),
        }
    }

    pub fn push(&self, v: T) {
        // 추가할 노드 작성
        let node = Box::new(Node {
            next: AtomicPtr::new(ptr::null_mut()),
            data: v,
        });

        // Box 타입의 값으로부터 포인터를 꺼낸다
        let ptr = Box::into_raw(node);

        unsafe {
            loop {
                // 아토믹하게 헤드 업데이트
                let head = self.head.load(Ordering::Relaxed);

                // 추가할 노드의 next를 head로 설정
                (*ptr).next.store(head, Ordering::Relaxed);

                // head 값이 업데이트 되지 않으면 추가할 노드에 업데이트
                if self
                    .head
                    .compare_exchange_weak(head, ptr, Ordering::Release, Ordering::Relaxed)
                    .is_ok()
                {
                    break;
                }
            }
        }
    }

    pub fn pop(&self) -> Option<T> {
        unsafe {
            loop {
                // 아토믹하게 헤드 업데이트
                let head = self.head.load(Ordering::Relaxed);

                // head가 null이면 None
                if head.is_null() {
                    return None;
                }

                let next = (*head).next.load(Ordering::Relaxed);

                // head 값이 업데이트 되지 않으면
                // head.next를 새로운 header로 업데이트
                if self
                    .head
                    .compare_exchange_weak(head, next, Ordering::Acquire, Ordering::Relaxed)
                    .is_ok()
                {
                    // 포인터를 Box로 되돌려 안의 값을 반환
                    let h = Box::from_raw(head);

                    return Some(h.data);
                }
            }
        }
    }
}

impl<T> Drop for StackBad<T> {
    fn drop(&mut self) {
        // 데이터 삭제
        let mut node = self.head.load(Ordering::Relaxed);
        while !node.is_null() {
            // 포인터를 Box로 되돌리는 조작을 반복
            let n = unsafe { Box::from_raw(node) };
            node = n.next.load(Ordering::Relaxed);
        }
    }
}
