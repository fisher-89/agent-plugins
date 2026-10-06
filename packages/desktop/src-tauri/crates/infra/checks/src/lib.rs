mod static_check;
mod testexec;

pub use static_check::ProcessStaticCheck;
pub use testexec::ProcessTestExecution;

#[cfg(test)]
mod static_check_test;

/// PATH 进程全局窗口的 crate 级串行化锁
#[cfg(all(test, not(target_os = "windows")))]
pub(crate) static TEST_PATH_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// 进程全局测试环境窗口（PATH / SHIM_* 环境变量改写）的全 crate 串行化锁
#[cfg(test)]
pub(crate) static TEST_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
