//! desktop-app 壳层库目标：`main` 薄壳与导出工具（crates/tools/
//! export-bindings）共享同一命令面（Tauri 2 上游模板形态）。`commands` 为 IPC 命令轨，`bindings` 为 specta builder
//! 组装与 TS bindings 幂等导出入口；`env` 为构建模式与数据根单点（判别源 /
//! 三分优先级 / 校验 / 双写与留痕），`selfcheck` 为 `--self-check` 自检契约。
//! 两者均为壳层模块（core / infra 仍禁 Tauri，判别源不越层）。

pub mod bindings;
pub mod commands;
pub mod env;
pub mod selfcheck;
