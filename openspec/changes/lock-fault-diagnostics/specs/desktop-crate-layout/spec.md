# desktop-crate-layout Specification（delta: lock-fault-diagnostics）

## ADDED Requirements

### Requirement: 平台条件依赖的落位例外（infra 的 Windows 系统 API 绑定）

infra crate 声明平台 API 绑定依赖时 SHALL 落 `[target.'<cfg>'.dependencies]` 条件段，MUST NOT 落无条件的 `[dependencies]`——使非目标平台（含 CI 的 Linux / macOS 构建）零新增编译依赖。首个消费者为 `crates/infra/exec` 的锁占用诊断面：

- `crates/infra/exec` SHALL 以 `[target.'cfg(windows)'.dependencies] windows-sys = { version = "0.61", features = ["Win32_System_RestartManager"] }` 引入 Restart Manager 绑定（占用者点名）；feature 面 MUST 为**按需最小集**（MUST NOT 引 `Win32` 全量 / MUST NOT 引与用途无关的 feature 族），且每个 feature MUST 在 `Cargo.toml` 注释其用途。
- 版本 SHALL 与 workspace 锁内既有版本一致（`0.61.2`），MUST NOT 为同一 crate 追加第二个直接依赖版本行。
- 精确锁探针的「DELETE 访问权 + 全共享打开」SHALL 用 std `std::os::windows::fs::OpenOptionsExt`（`access_mode` / `share_mode`）表达，MUST NOT 为访问权 / 共享模式常量单独引入 `Win32_Storage_FileSystem` / `Win32_Security` feature。
- 本例外 MUST NOT 改变既有分层与依赖方向结论：core 各 crate 仍零平台依赖、零 Tauri；`infra/exec` 的 workspace 内依赖仍仅 `foundation`；非 Windows 目标的依赖面与编译产物 MUST 与本变更前一致。

#### Scenario: 条件段落位审查

- **WHEN** 审查 `crates/infra/exec/Cargo.toml`
- **THEN** `windows-sys` 仅出现在 `[target.'cfg(windows)'.dependencies]` 段，feature 仅含 `Win32_System_RestartManager`，且该行有用途注释；无条件 `[dependencies]` 段无平台绑定 crate

#### Scenario: 非 Windows 零新增

- **WHEN** 以非 Windows 目标解析依赖（如 Linux CI）
- **THEN** 依赖图与编译输入不含 `windows-sys` 新增直接依赖，`crates/infra/exec` 的 crate 依赖仍为 `foundation` + tokio（特性面不变）

#### Scenario: 分层结论不变

- **WHEN** 复核 `desktop-crate-layout` 既有「依赖方向符合分层」与「core 零 Tauri」断言
- **THEN** core 各 crate 依赖面零变化、零 Tauri；`infra/exec` 不新增 workspace 内依赖边；平台绑定依赖不被任何 core crate 引入

## Module Contract

| 模块 | 定位 | 关键契约 |
|------|------|----------|
| `crates/infra/exec/Cargo.toml` | 平台条件依赖 | `[target.'cfg(windows)'.dependencies] windows-sys = { version = "0.61", features = ["Win32_System_RestartManager"] }`（按需最小 feature + 用途注释）；非目标平台零新增 |
| `crates/infra/exec/src/lockprobe.rs` | 平台 API 消费点 | 访问权 / 共享模式常量经 std `OpenOptionsExt`；Restart Manager 经 `cfg(windows)` 模块；非 Windows 臂零 windows-sys 引用 |
| `crates/core/**` | 分层边界 | 零平台绑定依赖、零 Tauri（结论不变） |
