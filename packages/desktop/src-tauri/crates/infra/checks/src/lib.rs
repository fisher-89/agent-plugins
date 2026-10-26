mod environment;
mod static_check;
mod testexec;

pub mod mutation;

pub use mutation::ProcessMutation;
pub use static_check::ProcessStaticCheck;
pub use testexec::ProcessTestExecution;

#[cfg(test)]
mod static_check_test;

#[cfg(test)]
pub(crate) static TEST_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// 取 crate 级环境锁（**中毒自愈单点**，MUST NOT 在调用点各写一份取锁式）。
///
/// 本锁只做「进程全局 env / PATH 串行化」，持锁窗口的 `Drop`（各 `*Window` /
/// `PathWindowGuard`）负责恢复 PATH 与摘除 shim 键——`PoisonError` 里没有任何
/// 需要守护的数据，一律剥除后继续。
///
/// 反过来，任一处 `.expect("…不可中毒")` 都会把「一例 flake」放大成「所有触
/// spawn / PATH 的用例集体 panic」：panic 级联使整个 suite 非零退出，`cargo
/// llvm-cov` 随即不写覆盖 JSON（聚合面多出一条假的「覆盖率采集缺失」归因），
/// 真因被埋在几十条 `PoisonError` 之下。
#[cfg(test)]
pub(crate) fn lock_test_env() -> std::sync::MutexGuard<'static, ()> {
    TEST_ENV_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
