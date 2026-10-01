pub(crate) mod config;
// `loop` 为 Rust 关键字：文件名按 design 定为 `loop.rs`，模块名以 raw
// identifier 挂载（`r#loop`）
pub(crate) mod r#loop;
pub(crate) mod normalize;
pub(crate) mod policy;
pub(crate) mod resume;
pub(crate) mod runner;
pub(crate) mod sandbox;
pub(crate) mod tools;

#[cfg(test)]
mod config_test;
#[cfg(test)]
mod loop_test;
#[cfg(test)]
mod normalize_test;
#[cfg(test)]
mod policy_test;
#[cfg(test)]
mod resume_test;
#[cfg(test)]
mod runner_test;
#[cfg(test)]
mod sandbox_test;
#[cfg(test)]
mod tools_test;
