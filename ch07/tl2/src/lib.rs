use std::{
    cell::UnsafeCell,
    collections::{HashMap, HashSet},
    sync::atomic::{AtomicU64, Ordering, fence},
};

/// 스트라이프의 크기
/// [u64], 8바이트
const STRIPE_SIZE: usize = 8;

/// 메모리 합계 크기
/// 512 바이트
const MEM_SIZE: usize = 512;

/// 메모리 타입
struct Memory {
    /// 메모리
    mem: Vec<u8>,
    /// lock & version
    /// 최상위 비트는 락
    /// 나머지 63비트는 버전
    lock_ver: Vec<AtomicU64>,
    /// global version-clock
    global_clock: AtomicU64,
    /// 주소에서 스트라이프 번호로 변환하기 위한 이동량
    shift_size: u32,
}

impl Memory {
    pub fn new() -> Self {
        // 메모리 영역 생성
        let mem = vec![0; MEM_SIZE];

        // 주소에서 스트라이프 번호로 변환하기 위한 시프트량 게산
        // 스트라이프 크기는 2^n에 맞춰야 함
        assert_eq!(
            STRIPE_SIZE.count_ones(),
            1,
            "스트라이프 크기는 2^n이어야 합니다."
        );
        let shift = STRIPE_SIZE.trailing_zeros();

        // lock&version을 초기화
        let lock_ver = (0..MEM_SIZE >> shift).map(|_| AtomicU64::new(0)).collect();

        Self {
            mem,
            lock_ver,
            global_clock: AtomicU64::new(0),
            shift_size: shift,
        }
    }

    /// [Self::global_clock]을 증가
    fn inc_global_clock(&mut self) -> u64 {
        self.global_clock.fetch_add(1, Ordering::AcqRel)
    }

    /// 대상 주소의 버전 취득
    fn get_addr_ver(&self, addr: usize) -> u64 {
        let idx = addr >> self.shift_size;
        let n = self.lock_ver[idx].load(Ordering::Relaxed);

        n & !(1 << 63)
    }

    /// 대상 주소의 버전이 rv 이하로 락되어 있지 않은지 확인
    fn test_not_modify(&self, addr: usize, rv: u64) -> bool {
        let idx = addr >> self.shift_size;
        let n = self.lock_ver[idx].load(Ordering::Relaxed);

        n <= rv
    }

    /// 대상 주소의 락 획득
    fn lock_addr(&mut self, addr: usize) -> bool {
        let idx = addr >> self.shift_size;
        if self.lock_ver[idx]
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |val| {
                // 최상위 비트값 확인 및 설정
                let n = val & (1 << 63);
                if n == 0 { Some(val | (1 << 63)) } else { None }
            })
            .is_ok()
        {
            true
        } else {
            false
        }
    }

    /// 대상 주소의 락 해제
    fn unlock(&mut self, addr: usize) {
        let idx = addr >> self.shift_size;
        self.lock_ver[idx].fetch_and(!(1 << 63), Ordering::Relaxed);
    }
}

/// 읽기 트랜잭션
pub struct ReadTrans<'a> {
    /// read-version
    read_ver: u64,
    /// 경합을 감지하면 `true`
    is_abort: bool,
    /// [Memory] 타입 참조
    mem: &'a Memory,
}

impl<'a> ReadTrans<'a> {
    fn new(mem: &'a Memory) -> Self {
        ReadTrans {
            // global version-clock 읽기
            read_ver: mem.global_clock.load(Ordering::Acquire),
            is_abort: false,
            mem,
        }
    }

    /// 메모리 읽기 함수
    pub fn load(&mut self, addr: usize) -> Option<[u8; STRIPE_SIZE]> {
        // 경합을 감지하면 종료
        if self.is_abort {
            return None;
        }

        // 주소가 스트라이프의 자릿수와 맞는지 확인
        assert_eq!(
            addr & (STRIPE_SIZE - 1),
            0,
            "주소가 스트라이프 자릿수와 맞지 않습니다."
        );

        // 읽기 메모리가 락되어 있지 않고 read_ver 이하인지 확인
        if !self.mem.test_not_modify(addr, self.read_ver) {
            self.is_abort = true;
            return None;
        }

        fence(Ordering::Acquire);

        // 메모리 읽기, 단순한 복사임
        let mut mem = [0; STRIPE_SIZE];
        mem.clone_from_slice(&self.mem.mem[addr..addr + STRIPE_SIZE]);

        fence(Ordering::SeqCst);

        // 읽기 메모리가 락 되어 있지 않고 read-version 이하인지 확인
        if !self.mem.test_not_modify(addr, self.read_ver) {
            self.is_abort = true;
            return None;
        }

        Some(mem)
    }
}

pub struct WriteTrans<'a> {
    /// read-version
    read_ver: u64,
    /// read-set
    read_set: HashSet<usize>,
    /// write-set
    write_set: HashMap<usize, [u8; STRIPE_SIZE]>,
    /// 락 완료 주소
    locked: Vec<usize>,
    /// 경합을 감지하면 `true`
    is_abort: bool,
    /// [Memory] 타입 참조
    mem: &'a mut Memory,
}

impl<'a> WriteTrans<'a> {
    fn new(mem: &'a mut Memory) -> Self {
        Self {
            // global version-clock 읽기
            read_ver: mem.global_clock.load(Ordering::Acquire),
            read_set: HashSet::new(),
            write_set: HashMap::new(),
            locked: Vec::new(),
            is_abort: false,
            mem,
        }
    }

    /// 메모리 쓰기 함수
    pub fn store(&mut self, addr: usize, val: [u8; STRIPE_SIZE]) {
        // 주소가 스트라이프의 자릿수와 맞는지 확인
        assert_eq!(
            addr & (STRIPE_SIZE - 1),
            0,
            "주소가 스트라이프의 자릿수와 맞지 않습니다."
        );

        self.write_set.insert(addr, val);
    }

    /// 메모리 읽기 함수
    pub fn load(&mut self, addr: usize) -> Option<[u8; STRIPE_SIZE]> {
        // 경합을 감지한 경우 종료
        if self.is_abort {
            return None;
        }

        // 주소가 스트라이프의 자릿수와 맞는지 확인
        assert_eq!(
            addr & (STRIPE_SIZE - 1),
            0,
            "주소가 스트라이프의 자릿수와 맞지 않습니다."
        );

        // 읽기 주소 저장
        self.read_set.insert(addr);

        // write-set에 있다면 이를 읽음
        if let Some(m) = self.write_set.get(&addr) {
            return Some(*m);
        }

        // 읽기 메모리가 락 되어 있지 않고 read-version 이하인지 판정
        if !self.mem.test_not_modify(addr, self.read_ver) {
            self.is_abort = true;
            return None;
        }

        fence(Ordering::Acquire);

        // 메모리 읽기 단순 복사함
        let mut mem = [0; STRIPE_SIZE];
        mem.clone_from_slice(&self.mem.mem[addr..addr + STRIPE_SIZE]);

        fence(Ordering::SeqCst);

        // 읽기 메모리가 락되어 있지 않고 read-version 이하인지 판정
        if !self.mem.test_not_modify(addr, self.read_ver) {
            self.is_abort = true;
            return None;
        }

        Some(mem)
    }

    /// write-set 안의 주소를 락
    /// 모든 주소의 락을 획득할 수 있는 경우 true를 반환
    fn lock_write_set(&mut self) -> bool {
        for addr in self.write_set.keys() {
            if self.mem.lock_addr(*addr) {
                // 락 획득 가능한 경우에는 locked에 추가
                self.locked.push(*addr);
            } else {
                // 가능하지 않은 경우에는 false를 반환하고 종료
                return false;
            }
        }

        true
    }

    /// read-set 검증
    fn validate_read_set(&self) -> bool {
        for addr in self.read_set.iter() {
            // write-set 안에 있는 주소인 경우에는
            // 자기 스레드가 락을 획득한 상태임
            if self.write_set.contains_key(addr) {
                // 버전만 검사
                let ver = self.mem.get_addr_ver(*addr);
                if ver > self.read_ver {
                    return false;
                }
            } else {
                // 다른 스레드가 락을 하지 않았는지와 버전 검사
                if !self.mem.test_not_modify(*addr, self.read_ver) {
                    return false;
                }
            }
        }

        true
    }

    /// 커밋
    fn commit(&mut self, ver: u64) {
        // 모든 주소에 대해 쓰기, 단순한 메모리 복사임
        for (&addr, val) in self.write_set.iter() {
            self.mem.mem[addr..addr + STRIPE_SIZE].clone_from_slice(val);
        }

        fence(Ordering::Release);

        // 모든 주소의 락 해제 및 버전 업데이트
        self.write_set.iter().for_each(|(addr, _)| {
            let idx = addr >> self.mem.shift_size;
            self.mem.lock_ver[idx].store(ver, Ordering::Relaxed);
        });

        // 락 완료 주소 집합 초기화
        self.locked.clear();
    }
}

impl<'a> Drop for WriteTrans<'a> {
    fn drop(&mut self) {
        // 락 완료 주소의 락을 해제
        self.locked.iter().for_each(|addr| self.mem.unlock(*addr));
    }
}

/// 트랜잭션을 실행하는 클로저의 반환값 타입
pub enum STMResult<T> {
    Ok(T),
    /// 트랜잭션 재시도
    Retry,
    /// 트랜잭션 중단
    Abort,
}

/// 트랜잭션을 실행한다.
pub struct STM {
    /// 실제 메모리
    mem: UnsafeCell<Memory>,
}

impl STM {
    pub fn new() -> Self {
        Self {
            mem: UnsafeCell::new(Memory::new()),
        }
    }

    /// 읽기 트랜잭션
    pub fn read_transaction<F, R>(&self, f: F) -> Option<R>
    where
        F: Fn(&mut ReadTrans) -> STMResult<R>,
    {
        loop {
            // global version-clock 읽기
            let mut tr = ReadTrans::new(unsafe { self.mem.get().as_ref_unchecked() });

            // 투기적 실행
            match f(&mut tr) {
                // 중단
                STMResult::Abort => return None,
                STMResult::Retry => {
                    if tr.is_abort {
                        // 재시도
                        continue;
                    }
                    // 중단
                    return None;
                }
                STMResult::Ok(val) => {
                    if tr.is_abort {
                        // 재시도
                        continue;
                    } else {
                        return Some(val);
                    }
                }
            }
        }
    }

    /// 쓰기 트랜잭션
    pub fn write_transaction<F, R>(&self, f: F) -> Option<R>
    where
        F: Fn(&mut WriteTrans) -> STMResult<R>,
    {
        loop {
            // global version-lock 읽기
            let mut tr = WriteTrans::new(unsafe { self.mem.get().as_mut_unchecked() });

            // 투기적 실행
            let result = match f(&mut tr) {
                STMResult::Abort => return None,
                STMResult::Retry => {
                    if tr.is_abort {
                        continue;
                    }
                    return None;
                }
                STMResult::Ok(val) => {
                    if tr.is_abort {
                        continue;
                    }

                    val
                }
            };

            // write-set 락
            if !tr.lock_write_set() {
                continue;
            }

            // global version-clock 증가
            let ver = tr.mem.inc_global_clock() + 1;

            // read-set 검증
            if tr.read_ver + 1 != ver && !tr.validate_read_set() {
                continue;
            }

            // 커밋과 릴리즈
            tr.commit(ver);

            return Some(result);
        }
    }
}

/// 스레드 사이에서 공유 가능하도록 설정, 채널에서 송수신 가능하도록 설정
unsafe impl Sync for STM {}
unsafe impl Send for STM {}

impl Default for STM {
    fn default() -> Self {
        Self::new()
    }
}
