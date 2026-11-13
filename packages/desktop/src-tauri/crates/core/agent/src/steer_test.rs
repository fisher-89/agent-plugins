//! `steer.rs`（补充说明注入注册表）的单元测试：空表 miss / 登记往返 / 一条一
//! ack（不合并） / 同键覆盖 / 除名幂等 / `send` 失败同臂 / 键与正文边界 /
//! ack 三形态 / 并发取用原子性。
//!
//! 零 mock：被测面是进程内内存表 + 既有 `tokio::sync::{mpsc, oneshot}`（无 IO、
//! 无配置、无子进程）。「miss 与 `send` 失败同臂」「逐条独立 ack」「ack 未消费
//! 即 drop」三条判据只能由真实通道的 `Disconnected` / `RecvError` 观测；`Mutex`
//! / `HashMap` 亦取真实容器（并发行需要真实锁语义，替身化会让原子性断言空转）。

use std::sync::{Arc, Barrier};
use std::thread;

use tokio::sync::{mpsc, oneshot};

use crate::steer::{SteerMessage, SteerRegistry};

/// 登记辅助：登记并交出接收入端（命中 / miss 二值观测半边）。
fn register_receiver(
    registry: &SteerRegistry,
    session_id: &str,
) -> mpsc::UnboundedReceiver<SteerMessage> {
    let (tx, rx) = mpsc::unbounded_channel::<SteerMessage>();
    registry.register(session_id, tx);
    rx
}

/// 载荷辅助：原样正文 + 该条自带的 ack 两端（一条一 ack 的类型载体）。
fn steer_msg(text: &str) -> (SteerMessage, oneshot::Receiver<()>) {
    let (ack, ack_rx) = oneshot::channel::<()>();
    (
        SteerMessage {
            text: text.to_owned(),
            ack,
        },
        ack_rx,
    )
}

// ---------------------------------------------------------------------------
// 正向：空表形态与 new / default 等价
// ---------------------------------------------------------------------------

#[test]
fn 空表即请求err且文案为会话已收口补充说明不可送达() {
    let registry = SteerRegistry::new();
    let (msg, _ack_rx) = steer_msg("先跑 lint");
    let err = registry.request("s1", msg).expect_err("空表请求应 Err");
    assert_eq!(err, "会话已收口，补充说明不可送达", "miss 文案逐字");
}

#[test]
fn new与default行为等价_均可登记可请求() {
    let via_new = SteerRegistry::new();
    let via_default = SteerRegistry::default();

    for (label, registry) in [("new", &via_new), ("default", &via_default)] {
        let mut rx = register_receiver(registry, "s1");
        let (msg, mut ack_rx) = steer_msg(&format!("{label}-补充说明"));
        registry
            .request("s1", msg)
            .unwrap_or_else(|error| panic!("{label} 形态可请求: {error}"));
        let delivered = rx.try_recv().expect("命中即投递");
        assert_eq!(
            delivered.text,
            format!("{label}-补充说明"),
            "{label} 形态正文逐字"
        );
        delivered.ack.send(()).expect("ack 可回");
        assert!(ack_rx.try_recv().is_ok(), "{label} 形态等待侧收 Ok");
    }
}

// ---------------------------------------------------------------------------
// 正向：命中往返与一条一 ack（不合并）
// ---------------------------------------------------------------------------

#[test]
fn 登记后请求ok且接收端得原文与原位ack() {
    let registry = SteerRegistry::new();
    let mut rx = register_receiver(&registry, "s1");
    let (msg, mut ack_rx) = steer_msg("先跑 lint");

    registry.request("s1", msg).expect("登记后请求 Ok");
    let delivered = rx.try_recv().expect("接收端收到该条");
    assert_eq!(delivered.text, "先跑 lint", "正文逐字");
    assert!(
        matches!(ack_rx.try_recv(), Err(oneshot::error::TryRecvError::Empty)),
        "ack 仍在等待侧在场（未被提前消费）"
    );
    delivered.ack.send(()).expect("ack 回投");
    assert_eq!(ack_rx.try_recv().expect("等待侧收到"), ());
}

#[test]
fn 同键连续三次请求三次ok且接收端收三条独立消息() {
    let registry = SteerRegistry::new();
    let mut rx = register_receiver(&registry, "s1");
    let texts = ["第一条", "第二条", "第三条"];
    let mut ack_receivers = Vec::new();

    for text in texts {
        let (msg, ack_rx) = steer_msg(text);
        registry.request("s1", msg).expect("每一条各自 Ok");
        ack_receivers.push(ack_rx);
    }

    // 按序三条独立消息：无合并、无覆盖
    for (index, text) in texts.iter().enumerate() {
        let delivered = rx.try_recv().expect("逐条在场");
        assert_eq!(delivered.text, *text, "第 {index} 条正文逐字");
        delivered.ack.send(()).expect("该条 ack 独立可回");
    }
    assert!(
        matches!(rx.try_recv(), Err(mpsc::error::TryRecvError::Empty)),
        "三条取净，第四条空缺"
    );
    for (index, mut ack_rx) in ack_receivers.into_iter().enumerate() {
        assert!(ack_rx.try_recv().is_ok(), "第 {index} 个 ack 均被回");
    }
}

// ---------------------------------------------------------------------------
// 正向 / 异常：除名幂等、send 失败同臂、同键覆盖
// ---------------------------------------------------------------------------

#[test]
fn remove除名后请求err且miss连调幂等不波及其它键() {
    // 形态一：已登记键 → 除名 → miss
    let registry = SteerRegistry::new();
    let mut rx = register_receiver(&registry, "s1");
    let mut other_rx = register_receiver(&registry, "s2");

    registry.remove("s1");
    let (msg, _ack_rx) = steer_msg("迟到补充说明");
    assert_eq!(
        registry.request("s1", msg).expect_err("除名后请求 Err"),
        "会话已收口，补充说明不可送达"
    );
    assert!(
        matches!(rx.try_recv(), Err(mpsc::error::TryRecvError::Disconnected)),
        "除名即 drop 接收入端（旧轮已收口）"
    );

    // 形态二：从未登记的键连调两次 → 不 panic、不波及其它键
    registry.remove("s-404");
    registry.remove("s-404");
    let (msg, mut ack_rx) = steer_msg("仍可送");
    registry.request("s2", msg).expect("miss 除名不波及其它键");
    let delivered = other_rx.try_recv().expect("s2 收到");
    delivered.ack.send(()).expect("ack 可回");
    assert!(ack_rx.try_recv().is_ok());
}

#[test]
fn 接收入端已drop时请求err且文案与miss臂逐字相同() {
    let registry = SteerRegistry::new();
    let rx = register_receiver(&registry, "s1");
    // 接收入端 drop = 该轮已被抛弃（send 失败面与 miss 同一事实）
    drop(rx);
    let (msg, _ack_rx) = steer_msg("无处可去");
    let send_failed = registry.request("s1", msg).expect_err("send 失败应 Err");

    let (miss_msg, _miss_ack) = steer_msg("无处可去");
    let miss = registry
        .request("s-never", miss_msg)
        .expect_err("miss 应 Err");

    assert_eq!(send_failed, "会话已收口，补充说明不可送达");
    assert_eq!(send_failed, miss, "两成因同一文案（同臂）");
}

#[test]
fn 同键覆盖后请求只落新接收端且旧接收端不再收到项() {
    let registry = SteerRegistry::new();
    let mut old_rx = register_receiver(&registry, "s1");
    let mut new_rx = register_receiver(&registry, "s1");

    let (msg, mut ack_rx) = steer_msg("只给新轮");
    registry.request("s1", msg).expect("新轮在场即 Ok");

    let delivered = new_rx.try_recv().expect("新接收入端收到");
    assert_eq!(delivered.text, "只给新轮");
    delivered.ack.send(()).expect("ack 可回");
    assert!(ack_rx.try_recv().is_ok());
    assert!(
        matches!(
            old_rx.try_recv(),
            Err(mpsc::error::TryRecvError::Disconnected)
        ),
        "旧接收入端随覆盖 drop（上一轮已收口）"
    );
}

// ---------------------------------------------------------------------------
// 正向 / 边界：多键隔离与键面
// ---------------------------------------------------------------------------

#[test]
fn 多键隔离_请求只落本键且两序等价() {
    for order in ["先s1后s2", "先s2后s1"] {
        let registry = SteerRegistry::new();
        let mut rx1 = register_receiver(&registry, "s1");
        let mut rx2 = register_receiver(&registry, "s2");

        let (msg, mut ack_rx) = steer_msg("给 s1");
        registry.request("s1", msg).expect("s1 命中");
        let delivered = rx1.try_recv().expect("s1 收到");
        delivered.ack.send(()).expect("ack 可回");
        assert!(ack_rx.try_recv().is_ok());
        assert!(
            matches!(rx2.try_recv(), Err(mpsc::error::TryRecvError::Empty)),
            "{order}：s2 未收到 s1 的项"
        );

        let (msg, mut ack_rx) = steer_msg("给 s2");
        registry.request("s2", msg).expect("s2 命中");
        let delivered = rx2.try_recv().expect("s2 收到");
        delivered.ack.send(()).expect("ack 可回");
        assert!(ack_rx.try_recv().is_ok());
    }
}

#[test]
fn 键边界_原串可被同串命中且近邻键互不命中() {
    let long_key = "字".repeat(1200);
    let keys: [&str; 4] = ["", "   ", "a\0b🚀", &long_key];
    for key in keys {
        let registry = SteerRegistry::new();
        let mut rx = register_receiver(&registry, key);
        let (msg, mut ack_rx) = steer_msg("命中");
        registry
            .request(key, msg)
            .unwrap_or_else(|error| panic!("键 {key:?} 不 trim 不截断：同串应命中 {error}"));
        let delivered = rx.try_recv().expect("收到");
        delivered.ack.send(()).expect("ack 可回");
        assert!(ack_rx.try_recv().is_ok());
    }

    // 近邻键：`""` 与 `" "` 是两个不同键
    let registry = SteerRegistry::new();
    let mut empty_rx = register_receiver(&registry, "");
    let (msg, _ack_rx) = steer_msg("空格键的补充说明");
    assert!(
        registry.request(" ", msg).is_err(),
        "近邻键不误命中（表零归一）"
    );
    assert!(
        matches!(empty_rx.try_recv(), Err(mpsc::error::TryRecvError::Empty)),
        "空串键的接收入端仍空"
    );
    let (msg, mut ack_rx) = steer_msg("空串键的补充说明");
    registry.request("", msg).expect("原串命中");
    let delivered = empty_rx.try_recv().expect("收到");
    delivered.ack.send(()).expect("ack 可回");
    assert!(ack_rx.try_recv().is_ok());
}

// ---------------------------------------------------------------------------
// 边界：正文零加工与 ack 三形态
// ---------------------------------------------------------------------------

#[test]
fn 正文边界_逐字送达且表零校验零trim零截断() {
    let texts = [
        "".to_owned(),
        "   ".to_owned(),
        "a\nb".to_owned(),
        "a\0b🚀".to_owned(),
        "字".repeat(1200),
    ];
    for text in texts {
        let registry = SteerRegistry::new();
        let mut rx = register_receiver(&registry, "s1");
        let (msg, mut ack_rx) = steer_msg(&text);
        registry.request("s1", msg).expect("表面零空白校验");
        let delivered = rx.try_recv().expect("收到");
        assert_eq!(delivered.text, text, "正文 {text:?} 逐字保真");
        delivered.ack.send(()).expect("ack 可回");
        assert!(ack_rx.try_recv().is_ok());
    }
}

#[test]
fn ack三形态_消费一次成功_接收侧已drop返err_未消费即drop得closed() {
    // 形态①：正常消费一次 → 等待侧得 Ok(())，且只此一次
    let (ack_tx, mut ack_rx) = oneshot::channel::<()>();
    let message = SteerMessage {
        text: "正常".to_owned(),
        ack: ack_tx,
    };
    message.ack.send(()).expect("ack 送达");
    assert_eq!(ack_rx.try_recv().expect("等待侧收到"), ());
    assert!(
        ack_rx.try_recv().is_err(),
        "只投一次（`send(self, …)` 消费性已排除二次发送）"
    );

    // 形态②：等待侧已 drop → `send` 返 Err（ack 的失败面）
    let (ack_tx, ack_rx) = oneshot::channel::<()>();
    drop(ack_rx);
    let message = SteerMessage {
        text: "对端已消失".to_owned(),
        ack: ack_tx,
    };
    assert!(message.ack.send(()).is_err(), "接收侧 drop 即 send Err");

    // 形态③：消息被丢弃（ack 未被消费）→ 等待侧 Closed（零超时机制）
    let registry = SteerRegistry::new();
    let mut rx = register_receiver(&registry, "s1");
    let (msg, mut ack_rx) = steer_msg("在途即收口");
    registry.request("s1", msg).expect("投递成功");
    let delivered = rx.try_recv().expect("收到");
    drop(delivered);
    assert!(
        matches!(ack_rx.try_recv(), Err(oneshot::error::TryRecvError::Closed)),
        "未消费即 drop → 等待侧 Closed（「生效前结束」的机制面）"
    );
}

// ---------------------------------------------------------------------------
// 边界：并发取用 / 并发读写
// ---------------------------------------------------------------------------

#[test]
fn 并发取用原子性_两线程同键请求均ok且接收端收两条() {
    for _ in 0..16 {
        let registry = Arc::new(SteerRegistry::new());
        let mut rx = register_receiver(&registry, "s1");
        let barrier = Arc::new(Barrier::new(2));

        let mut workers = Vec::new();
        for text in ["线程甲", "线程乙"] {
            let registry = Arc::clone(&registry);
            let barrier = Arc::clone(&barrier);
            workers.push(thread::spawn(move || {
                barrier.wait();
                let (ack, _ack_rx) = oneshot::channel::<()>();
                registry
                    .request(
                        "s1",
                        SteerMessage {
                            text: text.to_owned(),
                            ack,
                        },
                    )
                    .is_ok()
            }));
        }
        let results: Vec<bool> = workers
            .into_iter()
            .map(|worker| worker.join().expect("线程正常结束"))
            .collect();

        assert_eq!(
            results.iter().filter(|ok| **ok).count(),
            2,
            "并发两条均投递成功（无丢失、无 panic），实际: {results:?}"
        );
        let first = rx.try_recv().expect("第一条");
        let second = rx.try_recv().expect("第二条");
        let mut texts = vec![first.text, second.text];
        texts.sort();
        assert_eq!(texts, vec!["线程乙".to_owned(), "线程甲".to_owned()]);
        assert!(
            matches!(rx.try_recv(), Err(mpsc::error::TryRecvError::Empty)),
            "恰两条"
        );
    }
}

#[test]
fn 并发混读写不panic且结束态结果二值可考() {
    let registry = Arc::new(SteerRegistry::new());
    let mut workers = Vec::new();
    for _ in 0..8 {
        let registry = Arc::clone(&registry);
        workers.push(thread::spawn(move || {
            for _ in 0..32 {
                let (tx, _rx) = mpsc::unbounded_channel::<SteerMessage>();
                registry.register("s1", tx);
                registry.remove("s1");
                let (tx, _rx) = mpsc::unbounded_channel::<SteerMessage>();
                registry.register("s1", tx);
                let (ack, _ack_rx) = oneshot::channel::<()>();
                let _ = registry.request(
                    "s1",
                    SteerMessage {
                        text: "并发补充说明".to_owned(),
                        ack,
                    },
                );
                registry.remove("s1");
            }
        }));
    }
    for worker in workers {
        worker.join().expect("并发混读写无 panic / 无死锁");
    }

    // 结束态可查：末次请求结果二值之一（表内无残留键之外的不变量）
    let (ack, _ack_rx) = oneshot::channel::<()>();
    let last = registry.request(
        "s1",
        SteerMessage {
            text: "末次".to_owned(),
            ack,
        },
    );
    assert!(
        last.is_ok() || last.as_ref().err() == Some(&"会话已收口，补充说明不可送达".to_owned()),
        "末次请求结果二值之一，实际: {last:?}"
    );
}
