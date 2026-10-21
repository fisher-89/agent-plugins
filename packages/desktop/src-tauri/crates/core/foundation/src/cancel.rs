//! 取消原语（std-only）：[`CancelFlag`]（读端，`Clone` 共享同一原子标志）+
//! [`CancelHandle`]（写端，构造面收口在 [`CancelFlag::handle`]）。
//!
//! **为何落 `foundation`**：本 crate 的「第二个消费者到场才下沉」纪律在此正触发
//! ——两消费者为编排侧 `RunGuard`（写：`request_stop` 置位）与执行体
//! `exec::run_shell`（读：200 ms 轮询观测）。单消费者时代各自持有私有 `AtomicBool`
//! 即可，两消费者共用同一标志才需要中立载体。
//!
//! **为何是轮询而非异步通知**：`foundation` MUST NOT 依赖 tokio（本 crate 依赖面
//! 仅有 `sha2`），故不能引 `tokio::sync::Notify` / `watch`。执行体的取消观测因此
//! 为定间隔轮询（间隔常量驻 `exec::run`），异步等待面（停等 / 应答）仍走编排侧既有
//! `watch` 通道——两道并行、互不替代。
//!
//! **`PartialEq` 是身份比较而非状态比较**：两个 `CancelFlag` 相等 ⟺ 共享同一底层
//! 原子（`Arc::ptr_eq`），与「是否已置位」无关——两个独立铸造的恒 false 标志**不**
//! 相等。此语义供承载 `CancelFlag` 的请求载荷（`port::ToolStepRequest`）保住既有
//! `derive(PartialEq, Eq)`：同一请求与自身克隆比较恒等，跨铸造比较恒不等（无消费
//! 者做状态值比较）。

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// 取消标志读端：克隆共享同一底层原子（非值拷贝），只读不可置位。
#[derive(Clone)]
pub struct CancelFlag(Arc<AtomicBool>);

impl CancelFlag {
    /// 恒 false 零值（缺省信号 / 测试默认 / run 已收口回落）。
    pub fn never() -> CancelFlag {
        Self(Arc::new(AtomicBool::new(false)))
    }

    /// 读标志（`Relaxed`：仅「置位后可见」语义，无跨数据同步需求）。
    pub fn is_set(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    /// 取写端（共享同一原子；同一原子的写端可取多次，重复 `set` 幂等）。
    pub fn handle(&self) -> CancelHandle {
        CancelHandle(Arc::clone(&self.0))
    }
}

impl fmt::Debug for CancelFlag {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "CancelFlag(set = {})", self.is_set())
    }
}

impl PartialEq for CancelFlag {
    /// 身份比较（`Arc::ptr_eq`）——同一底层原子才相等，见模块 doc。
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for CancelFlag {}

/// 取消标志写端：只能由 [`CancelFlag::handle`] 铸造（构造面收口在读端）。
#[derive(Clone, Debug)]
pub struct CancelHandle(Arc<AtomicBool>);

impl CancelHandle {
    /// 置位（`Relaxed`；幂等——重复置位无副作用）。
    pub fn set(&self) {
        self.0.store(true, Ordering::Relaxed);
    }
}
