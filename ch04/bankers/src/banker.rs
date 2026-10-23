use std::sync::{Arc, Mutex};

struct Resource<const NRES: usize, const NTH: usize> {
    /// 이용 가능한 리소스
    available: [usize; NRES],
    /// 스레드 i가 확보중인 리소스
    allocation: [[usize; NRES]; NTH],
    /// 스레드 i가 필요로 하는 리소스의 최댓값
    max: [[usize; NRES]; NTH],
}

impl<const NRES: usize, const NTH: usize> Resource<NRES, NTH> {
    fn new(available: [usize; NRES], max: [[usize; NRES]; NTH]) -> Self {
        Self {
            available,
            allocation: [[0; NRES]; NTH],
            max,
        }
    }

    /// 현재 상태가 데드락을 발생시키지 않는지 확인
    fn is_safe(&self) -> bool {
        // 리소스 획득과 반환에 성공한 스레드 i
        let mut finish = [false; NTH];

        // 이용 가능한 리소스의 시뮬레이션 값
        let mut work = self.available;

        loop {
            let mut found = false;
            let mut num_true = 0;
            for (i, alc) in self.allocation.iter().enumerate() {
                if finish[i] {
                    num_true += 1;
                    continue;
                }

                let need = self.max[i].iter().zip(alc).map(|(m, a)| m - a);
                let is_avail = work.iter().zip(need).all(|(w, n)| *w >= n);

                if is_avail {
                    found = true;
                    finish[i] = true;
                    for (w, a) in work.iter_mut().zip(alc) {
                        *w += a;
                    }
                    break;
                }
            }

            if num_true == NTH {
                // 모든 스레드가 확보 가능하면 안전함
                return true;
            }

            if !found {
                // 스레드가 리소스를 확보할 수 없음
                break;
            }
        }

        false
    }

    /// id번째 스레드가 resource 하나 획득
    fn take(&mut self, id: usize, resource: usize) -> bool {
        if id >= NTH || resource >= NRES || self.available[resource] == 0 {
            return false;
        }

        self.allocation[id][resource] += 1;
        self.available[resource] -= 1;

        if self.is_safe() {
            // 리소스 확보 성공
            true
        } else {
            // 롤백
            self.allocation[id][resource] -= 1;
            self.available[resource] += 1;

            false
        }
    }

    /// id번째 스레드가 resource 하나 반환
    fn release(&mut self, id: usize, resource: usize) {
        if id >= NTH || resource >= NRES || self.allocation[id][resource] == 0 {
            return;
        }

        self.allocation[id][resource] -= 1;
        self.available[resource] += 1;
    }
}

#[derive(Clone)]
pub struct Banker<const NRES: usize, const NTH: usize> {
    resource: Arc<Mutex<Resource<NRES, NTH>>>,
}

impl<const NRES: usize, const NTH: usize> Banker<NRES, NTH> {
    pub fn new(available: [usize; NRES], max: [[usize; NRES]; NTH]) -> Self {
        Self {
            resource: Arc::new(Mutex::new(Resource::new(available, max))),
        }
    }

    pub fn take(&self, id: usize, resource: usize) -> bool {
        let mut r = self.resource.lock().unwrap();
        r.take(id, resource)
    }

    pub fn release(&self, id: usize, resource: usize) {
        let mut r = self.resource.lock().unwrap();
        r.release(id, resource);
    }
}
