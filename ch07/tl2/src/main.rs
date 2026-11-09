use std::sync::Arc;

const NUM_PHILOSOPHERS: usize = 8;

/// `n`은 철학자 번호이다.
fn philosopher(stm: Arc<tl2::STM>, n: usize) {
    // 왼쪽과 오른쪽 포크용 메모리
    let left = 8 * n;
    let right = 8 * ((n + 1) % NUM_PHILOSOPHERS);

    for _ in 0..500_000 {
        // 포크를 든다
        while !stm
            .write_transaction(|tr| {
                // 왼쪽 포크
                let Some(mut f1) = tr.load(left) else {
                    return tl2::STMResult::Retry;
                };
                // 오른쪽 포크
                let Some(mut f2) = tr.load(right) else {
                    return tl2::STMResult::Retry;
                };
                if f1[0] == 0 && f2[0] == 0 {
                    // 양쪽이 모두 비어 있으면 1로 설정
                    f1[0] = 1;
                    f2[0] = 1;
                    tr.store(left, f1);
                    tr.store(right, f2);

                    tl2::STMResult::Ok(true)
                } else {
                    // 양쪽을 모두 들 수 없으면 취득 실패
                    tl2::STMResult::Ok(false)
                }
            })
            .unwrap()
        {
            std::hint::spin_loop();
        }

        // 포크를 내려놓는다
        stm.write_transaction(|tr| {
            let Some(mut f1) = tr.load(left) else {
                return tl2::STMResult::Retry;
            };
            let Some(mut f2) = tr.load(right) else {
                return tl2::STMResult::Retry;
            };
            f1[0] = 0;
            f2[0] = 0;
            tr.store(left, f1);
            tr.store(right, f2);

            tl2::STMResult::Ok(())
        });
    }
}

/// 관측자
fn observer(stm: Arc<tl2::STM>) {
    for _ in 0..10000 {
        // 포크의 현재 상태를 얻는다.
        let chopsticks = stm
            .read_transaction(|tr| {
                let mut v = [0; NUM_PHILOSOPHERS];
                for (i, v_item) in v.iter_mut().enumerate() {
                    let Some(mem) = tr.load(8 * i) else {
                        return tl2::STMResult::Retry;
                    };
                    *v_item = mem[0]
                }

                tl2::STMResult::Ok(v)
            })
            .unwrap();

        println!("{:?}", chopsticks);

        // 들고 있는 포크 수가 홀수면 올바르지 않음
        let n = chopsticks.iter().sum::<u8>();

        if !n.is_multiple_of(2) {
            panic!("inconsistent");
        }

        // 슬립
        let us = std::time::Duration::from_micros(100);
        std::thread::sleep(us);
    }
}

fn main() {
    let stm = Arc::new(tl2::STM::new());

    // 철학자 스레드 생성
    let v = (0..NUM_PHILOSOPHERS)
        .map(|i| {
            let s = Arc::clone(&stm);
            std::thread::spawn(move || philosopher(s, i))
        })
        .collect::<Vec<_>>();

    // 관측자 스레드 생성
    let obs = std::thread::spawn(move || observer(stm));

    v.into_iter().try_for_each(|th| th.join()).unwrap();
    obs.join().unwrap();
}
