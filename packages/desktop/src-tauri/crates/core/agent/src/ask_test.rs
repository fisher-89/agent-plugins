//! `ask.rs`（ask 应答注册表）的单元测试：take 语义 / 同键覆盖 / 除名幂等 /
//! 键与正文边界 / 并发取用原子性。
//!
//! 零 mock：被测面是进程内内存表 + 既有 `tokio::sync::oneshot`（无 IO、无配置、
//! 无子进程）。take 语义与「同键覆盖即 drop 旧 Sender」两条判据只能由真实通道的
//! `RecvError` / `await` 观测；`Mutex` / `HashMap` 亦取真实容器（并发行需要真实
//! 锁语义，替身化会让原子性断言空转）。

use std::sync::{Arc, Barrier};
use std::thread;

use tokio::sync::oneshot;

use crate::ask::AskRegistry;

/// 注册辅助：登记并交出接收方（单次 take 语义的观测半边）。
fn register_waiter(registry: &AskRegistry, session_id: &str) -> oneshot::Receiver<String> {
    let (tx, rx) = oneshot::channel::<String>();
    registry.register(session_id, tx);
    rx
}

// ---------------------------------------------------------------------------
// 正向：空表形态与 new / default 等价
// ---------------------------------------------------------------------------

#[test]
fn 空表即时应答err且文案为无等待中的ask() {
    let registry = AskRegistry::new();
    let err = registry
        .answer("s1", "x".to_owned())
        .expect_err("空表应答应 Err");
    assert!(
        err.contains("当前无等待中的 ask"),
        "miss 文案逐字可读，实际: {err}"
    );
}

#[test]
fn new与default行为等价_均可构造可登记可应答() {
    let via_new = AskRegistry::new();
    let via_default = AskRegistry::default();

    for (label, registry) in [("new", &via_new), ("default", &via_default)] {
        let mut rx = register_waiter(registry, "s1");
        assert!(
            registry.answer("s1", format!("{label}-应答")).is_ok(),
            "{label} 形态可应答"
        );
        assert_eq!(
            rx.try_recv().expect("应答已投递"),
            format!("{label}-应答"),
            "{label} 形态投递原文"
        );
        assert!(
            registry.answer("s1", "第二次".to_owned()).is_err(),
            "{label} 形态 take 后 miss"
        );
    }
}

// ---------------------------------------------------------------------------
// 正向：首登往返与 take 语义
// ---------------------------------------------------------------------------

#[test]
fn 首登返回true且应答原文逐字送达() {
    let registry = AskRegistry::new();
    let (tx, mut rx) = oneshot::channel::<String>();

    assert!(registry.register("s1", tx), "首登时该键为空 → true");
    assert!(
        registry.answer("s1", "选项A".to_owned()).is_ok(),
        "登记后应答 Ok"
    );
    assert_eq!(
        rx.try_recv().expect("接收方收到应答"),
        "选项A",
        "原文逐字送达（零加工）"
    );
}

#[test]
fn take语义_二次应答err且接收方只收一次() {
    let registry = AskRegistry::new();
    let (tx, mut rx) = oneshot::channel::<String>();
    registry.register("s1", tx);

    assert!(registry.answer("s1", "第一次".to_owned()).is_ok());
    let err = registry
        .answer("s1", "第二次".to_owned())
        .expect_err("槽已 take → Err");
    assert_eq!(err, "当前无等待中的 ask", "重复应答文案逐字一致");

    assert_eq!(
        rx.try_recv().expect("接收方收到唯一一次应答"),
        "第一次",
        "只收一次原文（无第二次回灌）"
    );
    assert!(rx.try_recv().is_err(), "通道已消费完毕，无第二条消息");
}

#[test]
fn 同键覆盖返回false且旧等待方收recv_error() {
    let registry = AskRegistry::new();
    let (tx1, mut rx1) = oneshot::channel::<String>();
    let (tx2, mut rx2) = oneshot::channel::<String>();

    assert!(registry.register("s1", tx1), "首登 true");
    assert!(!registry.register("s1", tx2), "同键覆盖 → false（诊断值）");
    assert!(
        matches!(rx1.try_recv(), Err(oneshot::error::TryRecvError::Closed)),
        "旧等待方的 Sender 随覆盖 drop（顶替即放弃上一问）"
    );

    assert!(
        registry.answer("s1", "新应答".to_owned()).is_ok(),
        "应答命中的是第二个等待方"
    );
    assert_eq!(
        rx2.try_recv().expect("新等待方收到应答"),
        "新应答",
        "覆盖后的接收方拿到原文"
    );
    assert!(rx1.try_recv().is_err(), "被顶替的接收方零投递");
}

#[test]
fn 多键隔离_应答只唤醒本键() {
    for order in ["先s1后s2", "先s2后s1"] {
        let registry = AskRegistry::new();
        let (tx1, mut rx1) = oneshot::channel::<String>();
        let (tx2, mut rx2) = oneshot::channel::<String>();
        registry.register("s1", tx1);
        registry.register("s2", tx2);

        assert!(registry.answer("s1", "答给s1".to_owned()).is_ok());
        assert_eq!(rx1.try_recv().expect("s1 收到"), "答给s1");
        assert!(
            matches!(rx2.try_recv(), Err(oneshot::error::TryRecvError::Empty)),
            "{order}：s2 接收方仍 pending（未被 s1 的应答唤醒）"
        );

        assert!(registry.answer("s2", "答给s2".to_owned()).is_ok());
        assert_eq!(rx2.try_recv().expect("s2 收到"), "答给s2");
    }
}

// ---------------------------------------------------------------------------
// 正向：remove 除名与幂等
// ---------------------------------------------------------------------------

#[test]
fn remove除名后应答err且miss连调幂等不波及其它键() {
    let registry = AskRegistry::new();
    let (tx, mut rx) = oneshot::channel::<String>();
    registry.register("s1", tx);
    let (tx_other, mut rx_other) = oneshot::channel::<String>();
    registry.register("s2", tx_other);

    // 形态一：已登记键 → 除名
    registry.remove("s1");
    assert!(
        registry.answer("s1", "迟到应答".to_owned()).is_err(),
        "除名后应答 Err（轮收敛兜底的机械判据）"
    );
    assert!(
        matches!(rx.try_recv(), Err(oneshot::error::TryRecvError::Closed)),
        "除名即 drop Sender（等待方随之收 Closed）"
    );

    // 形态二：miss 键连调两次 → 不 panic、不波及其它键
    registry.remove("s-404");
    registry.remove("s-404");
    assert!(
        registry.answer("s2", "仍可答".to_owned()).is_ok(),
        "miss 除名不波及其它键"
    );
    assert_eq!(rx_other.try_recv().expect("s2 收到"), "仍可答");
}

// ---------------------------------------------------------------------------
// 异常：三形态 miss 文案逐字一致且不改写表项
// ---------------------------------------------------------------------------

#[test]
fn 三形态miss文案逐字一致且不改写表项() {
    // 形态 ①：从未登记
    let never = AskRegistry::new();
    let err_never = never
        .answer("s1", "x".to_owned())
        .expect_err("从未登记 → Err");

    // 形态 ②：remove 后迟滞应答
    let removed = AskRegistry::new();
    let (tx, _rx) = oneshot::channel::<String>();
    removed.register("s1", tx);
    removed.remove("s1");
    let err_removed = removed
        .answer("s1", "x".to_owned())
        .expect_err("除名后 → Err");

    // 形态 ③：已被 take 后重复应答
    let taken = AskRegistry::new();
    let (tx, _rx) = oneshot::channel::<String>();
    taken.register("s1", tx);
    taken.answer("s1", "首次".to_owned()).expect("首次受理");
    let err_taken = taken
        .answer("s1", "二次".to_owned())
        .expect_err("take 后 → Err");

    assert_eq!(err_never, err_removed, "①② 文案逐字相同");
    assert_eq!(err_removed, err_taken, "②③ 文案逐字相同");
    assert_eq!(err_never, "当前无等待中的 ask");

    // 表项未被改写：其后同键 register 仍为新登记（返回 true）
    for (label, registry) in [("never", &never), ("removed", &removed), ("taken", &taken)] {
        let (tx, mut rx) = oneshot::channel::<String>();
        assert!(
            registry.register("s1", tx),
            "{label}：miss 不改写表项，其后同键登记仍为「键原为空」"
        );
        assert!(registry.answer("s1", "再来".to_owned()).is_ok());
        assert_eq!(rx.try_recv().expect("收到"), "再来");
    }
}

#[test]
fn 接收方先行drop后应答不panic且槽仍被take() {
    let registry = AskRegistry::new();
    let (tx, rx) = oneshot::channel::<String>();
    registry.register("s1", tx);
    drop(rx);

    // 只断言「槽已被 take」这一机械事实，不裁决首次返回值（send 失败不改受理结论）
    let _ = registry.answer("s1", "对端已消失".to_owned());
    assert_eq!(
        registry
            .answer("s1", "第二次".to_owned())
            .expect_err("槽已空 → Err"),
        "当前无等待中的 ask",
        "对端消失不影响「槽已 take」的幂等语义"
    );
}

// ---------------------------------------------------------------------------
// 边界：键面
// ---------------------------------------------------------------------------

#[test]
fn 键边界_原串可被同串命中且近邻键互不命中() {
    let long_key = "字".repeat(1200);
    let keys: [&str; 4] = ["", "   ", "a\0b🚀", &long_key];
    for key in keys {
        let registry = AskRegistry::new();
        let mut rx = register_waiter(&registry, key);
        assert!(
            registry.answer(key, "命中".to_owned()).is_ok(),
            "键 {key:?} 不 trim 不截断：同串应答可命中"
        );
        assert_eq!(rx.try_recv().expect("收到"), "命中");
    }

    // 近邻键：`""` 与 `" "` 是两个不同键
    let registry = AskRegistry::new();
    let mut empty_rx = register_waiter(&registry, "");
    assert!(
        registry.answer(" ", "空格键应答".to_owned()).is_err(),
        "近邻键不误命中"
    );
    assert!(
        matches!(
            empty_rx.try_recv(),
            Err(oneshot::error::TryRecvError::Empty)
        ),
        "空串键的等待方仍 pending"
    );
    assert!(registry.answer("", "空串键应答".to_owned()).is_ok());
    assert_eq!(empty_rx.try_recv().expect("收到"), "空串键应答");
}

// ---------------------------------------------------------------------------
// 边界：正文面（零加工）
// ---------------------------------------------------------------------------

#[test]
fn 正文边界_逐字收到同串且零trim零截断() {
    let texts = [
        "".to_owned(),
        "   ".to_owned(),
        "a\nb".to_owned(),
        "a\0b🚀".to_owned(),
        "字".repeat(1200),
    ];
    for text in texts {
        let registry = AskRegistry::new();
        let mut rx = register_waiter(&registry, "s1");
        assert!(registry.answer("s1", text.clone()).is_ok(), "受理");
        assert_eq!(
            rx.try_recv().expect("收到"),
            text,
            "正文 {text:?} 逐字保真（零空白校验）"
        );
    }
}

// ---------------------------------------------------------------------------
// 边界：并发取用原子性 / 并发读写不 panic
// ---------------------------------------------------------------------------

#[test]
fn 并发取用原子性_恰一ok恰一err且接收方收一次() {
    for _ in 0..16 {
        let registry = Arc::new(AskRegistry::new());
        let mut rx = register_waiter(&registry, "s1");
        let barrier = Arc::new(Barrier::new(2));

        let mut workers = Vec::new();
        for text in ["线程甲", "线程乙"] {
            let registry = Arc::clone(&registry);
            let barrier = Arc::clone(&barrier);
            workers.push(thread::spawn(move || {
                barrier.wait();
                registry.answer("s1", text.to_owned()).is_ok()
            }));
        }
        let results: Vec<bool> = workers
            .into_iter()
            .map(|w| w.join().expect("线程正常结束"))
            .collect();

        assert_eq!(
            results.iter().filter(|ok| **ok).count(),
            1,
            "恰一次受理，实际: {results:?}"
        );
        assert_eq!(
            results.iter().filter(|ok| !**ok).count(),
            1,
            "恰一次落空（take 语义的竞态面）"
        );
        assert!(rx.try_recv().is_ok(), "接收方恰好收到一次应答");
        assert!(rx.try_recv().is_err(), "无第二条投递");
    }
}

#[test]
fn 并发混读写不panic且结束态可查() {
    let registry = Arc::new(AskRegistry::new());
    let mut workers = Vec::new();
    for _ in 0..8 {
        let registry = Arc::clone(&registry);
        workers.push(thread::spawn(move || {
            for _ in 0..32 {
                let (tx_a, _rx_a) = oneshot::channel::<String>();
                registry.register("s1", tx_a);
                registry.remove("s1");
                let (tx_b, _rx_b) = oneshot::channel::<String>();
                registry.register("s1", tx_b);
                let _ = registry.answer("s1", "并发应答".to_owned());
                registry.remove("s1");
            }
        }));
    }
    for worker in workers {
        worker.join().expect("并发混读写无 panic / 无死锁");
    }

    // 结束态可查：末次应答结果二值之一（表内无残留键之外的不变量）
    let last = registry.answer("s1", "末次".to_owned());
    assert!(
        last.is_ok() || last.as_ref().err() == Some(&"当前无等待中的 ask".to_owned()),
        "末次应答结果二值之一，实际: {last:?}"
    );
}
