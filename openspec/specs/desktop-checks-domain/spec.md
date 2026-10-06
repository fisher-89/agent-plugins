# desktop-checks-domain Specification

## Purpose

Desktop 后端检查域（checks 边界，边界分类学第六类）：外部进程 + pass/fail 结论门禁（static-check / test-execution）的领域组织——纯层落位 `crates/core/checks`（报告 schema 模型 / 解析器 / 聚合判定 / 诊断树 / 复用门，零 spawn 零 Tauri），进程执行落位 `crates/infra/checks`（crate 裸名 `checks-runtime`）；runner port 归属消费者留在 `core/orchestration::port`，spawn 不进 core。

## Requirements

### Requirement: checks 边界定义与分流法则

Desktop 后端 SHALL 以 checks 边界（边界分类学第六类）承接检查域。walker 直接调用的执行步按执行性质分流：外部进程 + pass/fail 结论门禁（static-check、test-execution）SHALL 落 checks 边界——契约与纯层落位 `crates/core/checks`，进程执行落位 `crates/infra/checks`（crate 裸名 `checks-runtime`）；外部进程 + agent 会话（executor / evaluator / decision）SHALL 留 agent 边界；外部进程 + 纯上下文文本（git diff）SHALL 按消费者归属——`git_diff.rs` 暂留 infra/agent（executor / 决策供料链路），待纯上下文工具成族再立域，MUST NOT 为单文件预建边界。

#### Scenario: 检查步归 checks 边界

- **WHEN** 评审 static-check 与 test-execution 执行步的落位
- **THEN** 二者声明 checks 边界（core/checks 纯层 + infra/checks 执行），infra/agent 内无检查域成员寄居

#### Scenario: 会话步留 agent 边界

- **WHEN** 评审 executor / evaluator / decision 会话步的落位
- **THEN** 留 agent 边界（compose_turn 会话租户），checks 边界不承载任何 agent 会话

#### Scenario: 纯上下文工具挂账不收编

- **WHEN** 评审 `git_diff.rs` 的落位
- **THEN** 暂留 infra/agent（消费者为会话供料链路），checks 边界不预收编，裁决可考无需重辩

### Requirement: core/checks 纯层零进程边界

`crates/core/checks` SHALL 承载检查域纯层：报告 schema 模型（对齐 CLI zod 权威 schema）、coverage 解析器（istanbul / llvm-cov / coverage 文本）、阈值与 override 聚合判定（conclusion 判定）、完整性校验与诊断树确定性分支、复用门判定（mtime 新鲜度比较纯函数）。core/checks MUST NOT 进程 spawn、MUST NOT 依赖 tokio process、MUST NOT 依赖 Tauri；解析与聚合 SHALL 为纯函数，以 fixtures 直驱可测（corpus 黄金语料先行，文件系统扫描等 IO 侧落 infra）。

#### Scenario: 纯函数 fixtures 直驱

- **WHEN** 以真实 summary.json / report.json fixtures 驱动 core/checks 的解析器与聚合判定
- **THEN** 无需进程、无需文件系统即可得出结论与诊断分支，测试全程进程内完成

#### Scenario: 零 spawn 审查

- **WHEN** 审查 core/checks 源码的依赖声明与进程创建调用
- **THEN** 无 tokio process 依赖、无进程 spawn、无 Tauri 依赖

### Requirement: infra/checks 进程执行层与 static_check 平移

`crates/infra/checks` SHALL 承载检查域进程执行：`static_check.rs`（ProcessStaticCheck）自 infra/agent 平移、零逻辑改动（config 读命令 → shell 语义 spawn → 诊断捕获 + 退出码映射；无配置 / 空命令直过；配置在位而程序不可达显式 `Err`）；`testexec/`（test-execution runner：框架探测、命令模板展开、spawn / 超时 / 输出捕获、报告写盘编排）。Windows shell shim（cmd /C）与程序解析前置检查（PATH / cwd 探测、可执行后缀候选——防 shell 吞命令缺失烧反馈边预算）SHALL 复用 static_check 既有经验。

#### Scenario: 平移零逻辑改动

- **WHEN** ProcessStaticCheck 及其测试自 infra/agent 迁入 infra/checks
- **THEN** 执行语义逐行等价（无配置直过 / 程序不可达显式 Err / 退出码映射不变），平移后测试全绿，infra/agent 无 static_check 残留

#### Scenario: Windows .cmd shim 可达

- **WHEN** runner 在 Windows 上执行 jest / vitest / npx 类 `.cmd` shim 命令
- **THEN** spawn 前完成程序解析前置检查（含可执行后缀候选），程序缺失显式 `Err` 停给用户而非落 passed=false 反馈边空转

### Requirement: checks 依赖规则与 port 归属

依赖方向 SHALL 为：`checks`（core）→ `config` + `foundation`（core→core 消费 typed tests[] 模型，先例：config 唯一出口纪律）；`checks-runtime`（infra）→ `orchestration`（port 类型消费，agent-runtime 同款先例）+ `checks` + `config` + `foundation`。runner port（`TestExecutionRunner` / `StaticCheckRunner` / `ToolCommand` / `ToolStepOutput`）SHALL 留 `orchestration::port` 原地（port 属于消费者；移 port 牵动 ToolStepOutput 封闭集引用链）；`ToolStepOutput::TestExecution` 最小载荷类型定义于 orchestration::port，core/orchestration MUST NOT 依赖 core/checks（组合根装配时由 checks 域产出映射为 port 载荷）。spawn 不进 core 红线延续：core/checks 零 spawn，进程执行全部落 infra/checks。

#### Scenario: Cargo.toml 依赖审查

- **WHEN** 检查 checks 两 crate 的 `Cargo.toml`
- **THEN** `checks` 的 workspace 内依赖仅 `config` + `foundation` 且零 Tauri；`checks-runtime` 依赖 `orchestration` + `checks` + `config` + `foundation` 且零 Tauri

#### Scenario: orchestration 零依赖 checks

- **WHEN** 检查 core/orchestration 的依赖声明
- **THEN** 无 `checks` 边（最小载荷类型定义于 orchestration::port 本地）；port.rs 三件套（TestExecution 命令 / 产出 / Runner port）原地扩展

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/checks`（新，裸名 `checks`） | 检查域纯层 | model / parser / aggregate / diagnose / reuse 纯函数；零 spawn 零 tokio process 零 Tauri；依赖 config + foundation；fixtures 直驱 + corpus 黄金可测 |
| `crates/infra/checks`（新，裸名 `checks-runtime`） | 检查域进程执行 | static_check.rs 平移（零逻辑改动）+ testexec/（detect / runner / report）；依赖 orchestration + checks + config + foundation + tokio process；零 Tauri |
| `orchestration::port`（原地不动） | runner port 归属 | `TestExecutionRunner` 与 `StaticCheckRunner` 并列；`ToolCommand::TestExecution` / `ToolStepOutput::TestExecution`（最小载荷）入封闭集；core/orchestration 零依赖 checks |
| `crates/infra/agent`（裸名 `agent-runtime`，收窄） | 会话租户身份回归 | cli / sdk / worker / compose / store_port / git_diff；static_check 移出；git_diff 暂留挂账（纯上下文工具成族再立域） |
