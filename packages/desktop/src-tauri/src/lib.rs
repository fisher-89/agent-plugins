//! desktop-app 壳层库目标：`main` 薄壳与导出 bin 共享同一命令面（Tauri 2
//! 上游模板形态）。`commands` 为 IPC 命令五轨，`bindings` 为 specta builder
//! 组装与 TS bindings 幂等导出入口。

pub mod bindings;
pub mod commands;

#[cfg(test)]
mod bindings_test;
