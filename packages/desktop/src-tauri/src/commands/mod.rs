//! Tauri command 三轨：queries（change 域查询 + workspace 注册）、exec（执行；
//! 承载 agent 命令）与 db（db 查看轨道，只读）。
//!
//! Command body 纪律（决策出处：desktop-app-shell 能力 spec，
//! `specs/desktop-app-shell/spec.md`，路径相对域根）：每条命令的 body 仅允许三件事——
//! 参数转换（IPC 入参 → 领域/store 入参）、调用（core 函数或 store 操作）、
//! 错误映射（领域/Store 错误 → `Err(String)`）。两条禁令：领域解释 SHALL 下推 core；
//! 跨边界协调 SHALL 触发 app crate 决策（五条翻转信号见 spec），
//! MUST NOT 以「先塞进命令里」的方式消化编排增长。
//! `commands/workspaces` 的 `*_inner(&Store)` 纯函数模式与 `commands/exec` 的
//! `run_agent()` 编排函数同为 app 层微形态，将来抽 app crate 时平移复用
//! （函数边界升 crate 边界）、不重写、不内联回命令体。

pub mod db;
pub mod exec;
pub mod queries;
pub mod workspaces;
