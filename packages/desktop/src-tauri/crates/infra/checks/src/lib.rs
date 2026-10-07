mod static_check;
mod testexec;

pub use static_check::ProcessStaticCheck;
pub use testexec::ProcessTestExecution;

#[cfg(test)]
mod static_check_test;

#[cfg(test)]
pub(crate) static TEST_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
