use std::{
    alloc::Layout,
    collections::{HashMap, HashSet, LinkedList},
    ptr::NonNull,
};

use nix::sys::mman::ProtFlags;

/// 레지스터 값을 저장하는 구조체
#[repr(C)]
struct Registers {
    rbx: u64,
    rbp: u64,
    r12: u64,
    r13: u64,
    r14: u64,
    r15: u64,
    rsp: u64,
    rdx: u64,
}

impl Registers {
    /// 스택 포인터를 가리키는 주소를 인수 rsp로 받아
    /// `Registers`타입을 초기화해서 반환한다.
    fn new(rsp: u64) -> Self {
        Self {
            rbx: 0,
            rbp: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
            rsp,
            // 스레드 개시 엔트리 포인트가 되는 함수의 주소를 보존
            rdx: entry_point as *const () as u64,
        }
    }
}

unsafe extern "C" {
    fn set_context(ctx: *mut Registers) -> u64;
    fn switch_context(ctx: *const Registers) -> !;
}

/// 스레드 개시 시 실행하는 함수 타입
type Entry = fn();

/// 페이지 크기
/// 리눅스에서는 4KB
const PAGE_SIZE: usize = 4 * 1024;

/// 컨텍스트
struct Context {
    /// 레지스터
    regs: Registers,
    /// 스택
    stack: *mut u8,
    /// 스택 레이아웃
    stack_layout: Layout,
    /// 엔트리 포인트
    entry: Entry,
    /// 스레드 ID
    id: u64,
}

impl Context {
    /// 레지스터 정보로 포인터 가져오기
    fn get_regs_mut(&mut self) -> *mut Registers {
        &mut self.regs
    }

    fn get_regs(&self) -> *const Registers {
        &self.regs
    }

    fn new(func: Entry, stack_size: usize, id: u64) -> Self {
        // 스택 영역 확보
        let layout = Layout::from_size_align(stack_size, PAGE_SIZE).unwrap();
        let stack = unsafe { std::alloc::alloc(layout) };

        // 가드 페이지 설정
        unsafe {
            nix::sys::mman::mprotect(
                NonNull::from_mut(&mut *(stack as *mut std::ffi::c_void)),
                PAGE_SIZE,
                ProtFlags::PROT_NONE,
            )
            .unwrap();
        }

        // 레지스터 초기화
        let regs = Registers::new(stack as u64 + stack_size as u64);

        // 컨텍스트 초기화
        Self {
            regs,
            stack,
            stack_layout: layout,
            entry: func,
            id,
        }
    }
}

/// 모든 스레드 종료 시 돌아올 위치
static mut CTX_MAIN: Option<Box<Registers>> = None;

/// 필요하지 않은 스택 영역
static mut UNUSED_STACK: (*mut u8, Layout) = (std::ptr::null_mut(), Layout::new::<u8>());

/// 스레드 실행 큐
static mut CONTEXTS: LinkedList<Box<Context>> = LinkedList::new();

/// 스레드 ID 집합
static mut ID: *mut HashSet<u64> = std::ptr::null_mut();

/// 고유한 스레드 ID를 생성한다.
fn get_id() -> u64 {
    loop {
        let rnd = rand::random::<u64>();
        unsafe {
            if !(*ID).contains(&rnd) {
                (*ID).insert(rnd);

                return rnd;
            }
        }
    }
}

/// 스레드 생성을 수행한다.
pub fn spawn(func: Entry, stack_size: usize) -> u64 {
    unsafe {
        let id = get_id();
        (&raw mut CONTEXTS)
            .as_mut_unchecked()
            .push_back(Box::new(Context::new(func, stack_size, id)));
        schdule();

        id
    }
}

/// 컨텍스트 스위칭을 수행한다.
pub fn schdule() {
    unsafe {
        // 실행 가능한 프로세스가 자신뿐이다.
        if (&raw const CONTEXTS).as_ref_unchecked().len() == 1 {
            return;
        }

        // 자신의 컨텍스트를 실행큐의 맨 끝으로 이동한다.
        let mut ctx = (&raw mut CONTEXTS).as_mut_unchecked().pop_front().unwrap();
        // 레지스터 보존 영역을 가리키는 포인터를 얻는다.
        let regs = ctx.get_regs_mut();
        (&raw mut CONTEXTS).as_mut_unchecked().push_back(ctx);

        // 레지스터 보존
        if set_context(regs) == 0 {
            // 다음 스레드로 컨텍스트 스위칭
            let next = (&raw const CONTEXTS).as_ref_unchecked().front().unwrap();
            switch_context((**next).get_regs());
        }

        // 불필요한 스택 영역 삭제
        rm_unused_stack();
    }
}

/// 스레드가 가장 먼저 호출될 때의 엔트리 포인트
extern "C" fn entry_point() {
    unsafe {
        // 저장한 엔트리 함수 실행
        let ctx = (&raw const CONTEXTS).as_ref_unchecked().front().unwrap();
        (ctx.entry)();

        // 스레드 종료 후

        // 스레드 ID 삭제
        (*ID).remove(&ctx.id);

        // 불필요한 스택 영역으로 보존
        UNUSED_STACK = (ctx.stack, ctx.stack_layout);

        match (&raw const CONTEXTS).as_ref_unchecked().front() {
            Some(c) => {
                // 다음 스레드로 컨텍스트 스위칭
                switch_context((**c).get_regs());
            }
            None => {
                // 모든 스레드가 종료하면 main함수의 스레드로 돌아감
                if let Some(c) = (&raw const CTX_MAIN).as_ref_unchecked() {
                    switch_context(&**c as *const Registers);
                }
            }
        }

        panic!("entry_point");
    }
}

/// 최초의 스레드 생성을 수행한다.
pub fn spawn_from_main(func: Entry, stack_size: usize) {
    unsafe {
        // 이미 초기화를 했다면 에러
        if (&raw const CTX_MAIN).as_ref_unchecked().is_some() {
            panic!("spawn_from_main is called twice");
        }

        // main 함수 용 컨텍스트 생성
        CTX_MAIN = Some(Box::new(Registers::new(0)));
        if let Some(ctx) = (&raw mut CTX_MAIN).as_mut_unchecked() {
            // 글로벌 변수 초기화
            let mut msgs = MappedList::new();
            MESSAGES = &mut msgs;

            let mut waiting = HashMap::new();
            WAITING = &mut waiting;

            let mut ids = HashSet::new();
            ID = &mut ids;

            // 모든 스레드 종료 시 돌아갈 위치 저장
            if set_context(&mut **ctx) == 0 {
                // 최초 실행하는 스레드의 컨텍스트 생성
                (&raw mut CONTEXTS)
                    .as_mut_unchecked()
                    .push_back(Box::new(Context::new(func, stack_size, get_id())));
                let first = (&raw const CONTEXTS).as_ref_unchecked().front().unwrap();
                switch_context(first.get_regs());
            }

            // 불필요한 스택 해제
            rm_unused_stack();

            // 글로벌 변수 클리어
            CTX_MAIN = None;
            (&raw mut CONTEXTS).as_mut_unchecked().clear();
            MESSAGES = std::ptr::null_mut();
            WAITING = std::ptr::null_mut();
            ID = std::ptr::null_mut();

            msgs.clear();
            waiting.clear();
            ids.clear();
        }
    }
}

/// 스택을 삭제한다.
unsafe fn rm_unused_stack() {
    unsafe {
        if !UNUSED_STACK.0.is_null() {
            // 스택 보호 영역 삭제
            nix::sys::mman::mprotect(
                NonNull::new_unchecked(UNUSED_STACK.0).cast(),
                PAGE_SIZE,
                ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
            )
            .unwrap();
            // 스택 영역 삭제
            std::alloc::dealloc(UNUSED_STACK.0, UNUSED_STACK.1);
            UNUSED_STACK = (std::ptr::null_mut(), Layout::new::<u8>());
        }
    }
}

struct MappedList<T> {
    map: HashMap<u64, LinkedList<T>>,
}

impl<T> MappedList<T> {
    fn new() -> Self {
        MappedList {
            map: HashMap::new(),
        }
    }

    /// Key에 대응하는 리스트의 가장 마지막에 추가
    fn push_back(&mut self, key: u64, val: T) {
        if let Some(list) = self.map.get_mut(&key) {
            // 대응하는 리스트가 존재하면 추가
            list.push_back(val);
        } else {
            // 존재하지 않는 경우 새롭게 리스트를 작성하고 추가
            let mut list = LinkedList::new();
            list.push_back(val);
            self.map.insert(key, list);
        }
    }

    /// Key에 대응하는 리스트의 가장 처음부터 값을 꺼낸다
    fn pop_front(&mut self, key: u64) -> Option<T> {
        if let Some(list) = self.map.get_mut(&key) {
            let val = list.pop_front();
            if list.is_empty() {
                self.map.remove(&key);
            }

            val
        } else {
            None
        }
    }

    fn clear(&mut self) {
        self.map.clear();
    }
}

/// 메시지 큐
static mut MESSAGES: *mut MappedList<u64> = std::ptr::null_mut();

/// 대기 스레드 집합
static mut WAITING: *mut HashMap<u64, Box<Context>> = std::ptr::null_mut();

/// 메시지 송신을 수행한다.
pub fn send(key: u64, msg: u64) {
    unsafe {
        // 메시지큐의 맨 끝에 추가
        (*MESSAGES).push_back(key, msg);

        // 스레드가 수신 대기 상태이면 실행 큐로 이동시킴
        if let Some(ctx) = (*WAITING).remove(&key) {
            (&raw mut CONTEXTS).as_mut_unchecked().push_back(ctx);
        }

        schdule();
    }
}

/// 메시지를 수신한다
pub fn recv() -> Option<u64> {
    unsafe {
        // 스레드 ID 취득
        let key = (&raw const CONTEXTS).as_ref_unchecked().front().unwrap().id;

        // 메시지가 이미 큐에 있으면 즉시 반환
        if let Some(msg) = (*MESSAGES).pop_front(key) {
            return Some(msg);
        }

        // 실행 가능한 스레드가 없으면 데드락
        if (&raw const CONTEXTS).as_ref_unchecked().len() == 1 {
            panic!("deadlock");
        }

        // 실행 중 스레드를 수신 대기 상태로 이동
        let mut ctx = (&raw mut CONTEXTS).as_mut_unchecked().pop_front().unwrap();
        let regs = ctx.get_regs_mut();
        (*WAITING).insert(key, ctx);

        // 다음 실행 가능한 스레드를 컨텍스트 스위칭
        if set_context(regs) == 0 {
            let next = (&raw const CONTEXTS).as_ref_unchecked().front().unwrap();
            switch_context((**next).get_regs());
        }

        // 불필요한 스택 삭제
        rm_unused_stack();

        // 수신한 메시지 취득
        (*MESSAGES).pop_front(key)
    }
}
