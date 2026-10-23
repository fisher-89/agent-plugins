//! 取消原语测试（std-only，零 mock）：`CancelFlag` / `CancelHandle` 的零值、
//! 置位传播、clone 共享原子、身份相等语义、`Debug` 渲染与跨线程可见性。
//!
//! 覆盖 AC-4 的「`CancelFlag::never()` 恒 false」与 D7 的「`PartialEq` 是身份比较
//! 而非状态比较」；依赖面（零 tokio / 零 workspace 内依赖）属 crate 图审查面，
//! 不在用例内断言。

use std::sync::{Arc, Barrier};
use std::thread;

use super::cancel::CancelFlag;

// ---------------------------------------------------------------------------
// 正向：零值与置位传播
// ---------------------------------------------------------------------------

/// 正向：`never()` 恒 false——连续多次读不翻转，两次独立铸造互不影响。
#[test]
fn never恒false且多次读不翻转() {
    let flag = CancelFlag::never();
    assert!(!flag.is_set(), "never() 零值必须为 false");
    for round in 0..8 {
        assert!(!flag.is_set(), "第 {round} 次读仍为 false（无自翻转）");
    }

    // 两个独立 never() 互不影响
    let other = CancelFlag::never();
    flag.handle().set();
    assert!(flag.is_set(), "置位后自身为 true");
    assert!(!other.is_set(), "另一独立 never() 不受影响");
}

/// 正向：`handle().set()` → `is_set()` 为 true（写端与读端同一原子）。
#[test]
fn handle置位后读端可见() {
    let flag = CancelFlag::never();
    let handle = flag.handle();
    assert!(!flag.is_set(), "置位前 false");
    handle.set();
    assert!(flag.is_set(), "同一原子的写端置位后读端可见");
}

/// 正向：`CancelFlag::clone` 共享同一原子——双向传播（clone 置位原值可见、
/// 原值置位 clone 可见）。
#[test]
fn clone共享同一原子且双向传播() {
    // 方向一：clone 置位 → 原值可见
    let flag = CancelFlag::never();
    let copy = flag.clone();
    copy.handle().set();
    assert!(flag.is_set(), "clone 置位后原值为 true");
    assert!(copy.is_set(), "clone 自身为 true");

    // 方向二：原值置位 → clone 可见
    let flag = CancelFlag::never();
    let copy = flag.clone();
    flag.handle().set();
    assert!(copy.is_set(), "原值置位后 clone 为 true");
}

/// 正向：`CancelHandle` clone 后任一份 `set()` 均置位同一标志。
#[test]
fn handle_clone后任一set均置位() {
    let flag = CancelFlag::never();
    let handle = flag.handle();
    let handle2 = handle.clone();
    handle2.set();
    assert!(flag.is_set(), "handle clone 置位后标志为 true");

    // 另一支：clone handle 置位 → 原 handle 读端可见；重复 set 幂等
    let other = CancelFlag::never();
    let first = other.handle();
    let second = first.clone();
    second.set();
    assert!(other.is_set(), "clone handle 置位后标志为 true");
    first.set();
    assert!(other.is_set(), "原 handle 重复置位后标志仍为 true（幂等）");
}

// ---------------------------------------------------------------------------
// 边界：幂等 / 身份相等 / Debug / 不串台
// ---------------------------------------------------------------------------

/// 边界（参数化 1 / 3 / N 次）：重复 `set()` 幂等——恒 true、无 pod panic、
/// 无副作用。
#[test]
fn set幂等_一次三次多次无副作用() {
    for times in [1usize, 3, 64] {
        let flag = CancelFlag::never();
        let handle = flag.handle();
        for round in 0..times {
            handle.set();
            assert!(flag.is_set(), "第 {} 次 set 后为 true", round + 1);
        }
        assert!(flag.is_set(), "set {times} 次后仍为 true（幂等）");
        // 幂等不改变身份：多次取写端后读端仍与 clone 相等
        assert_eq!(flag.clone(), flag, "重复 set / handle 不改身份");
    }
}

/// 边界（表驱动 2 形态）：相等语义是 **identity**——同一原子相等（含反射性），
/// 独立铸造永不相等（状态值比较不存在）。
#[test]
fn 相等语义为identity而非状态比较() {
    // 形态一：同一原子（clone）
    let a = CancelFlag::never();
    assert_eq!(a.clone(), a, "clone 共享同一原子 → 相等");
    assert!(a == a, "反射性成立");

    // 形态二：独立铸造
    assert_ne!(
        CancelFlag::never(),
        CancelFlag::never(),
        "两个独立 never() 不相等（身份比较非状态比较）"
    );
    let b = CancelFlag::never();
    assert_ne!(a, b, "独立铸造的标志互不相等");
}

/// 边界：置位后仍相等——clone 与原值共享原子，状态差异不存在；独立标志置位
/// 不影响彼此相等关系。
#[test]
fn 置位后相等关系不变() {
    let a = CancelFlag::never();
    let copy = a.clone();
    copy.handle().set();
    assert_eq!(a, copy, "置位后 clone 与原值仍相等（共享原子）");
    assert_eq!(
        a.is_set(),
        copy.is_set(),
        "共享原子 → 两读点状态一致（不存在一 true 一 false）"
    );

    // 独立标志置位不改变不相等语义
    let b = CancelFlag::never();
    b.handle().set();
    assert_ne!(a, b, "独立置位后仍不相等（身份比较）");
}

/// 边界（参数化 2 形态）：`Debug` 为手写渲染 `CancelFlag(set = {bool})`，
/// 非字段展开。
#[test]
fn debug渲染_置位前后两形态() {
    let flag = CancelFlag::never();
    assert_eq!(format!("{flag:?}"), "CancelFlag(set = false)", "未置位渲染");

    flag.handle().set();
    assert_eq!(format!("{flag:?}"), "CancelFlag(set = true)", "置位后渲染");
}

/// 边界（表驱动 2 形态）：不串台——两独立标志其一置位另一个仍 false；同一标志
/// 的两份 clone 状态恒一致。
#[test]
fn 不串台_独立标志与同一标志两形态() {
    // 形态一：独立标志
    let first = CancelFlag::never();
    let second = CancelFlag::never();
    first.handle().set();
    assert!(first.is_set(), "被置位者 true");
    assert!(!second.is_set(), "另一独立标志仍 false");

    // 形态二：同一标志两个 clone
    let shared = CancelFlag::never();
    let left = shared.clone();
    let right = shared.clone();
    left.handle().set();
    assert_eq!(
        (left.is_set(), right.is_set(), shared.is_set()),
        (true, true, true),
        "同一标志的 clone 状态一致"
    );
}

// ---------------------------------------------------------------------------
// 正向：跨线程可见（纯 std，零 tokio）
// ---------------------------------------------------------------------------

/// 正向：`std::thread::spawn` 内 `handle.set()` + `join()` → 主线程可见
/// （`Relaxed` 语义下 `join` 提供 happens-before）。
#[test]
fn 跨线程置位后主线程可见() {
    let flag = CancelFlag::never();
    let writer = flag.handle();
    let reader = flag.clone();
    let barrier = Arc::new(Barrier::new(2));

    let thread_barrier = Arc::clone(&barrier);
    let spawned = thread::spawn(move || {
        assert!(!reader.is_set(), "线程开始前未置位");
        writer.set();
        assert!(reader.is_set(), "同线程内置位立即可见");
        thread_barrier.wait();
    });

    barrier.wait();
    spawned.join().expect("写线程正常退出");
    assert!(flag.is_set(), "join 后主线程观测到置位（happens-before）");
}
