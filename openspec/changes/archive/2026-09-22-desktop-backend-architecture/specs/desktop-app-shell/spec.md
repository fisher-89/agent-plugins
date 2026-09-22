# desktop-app-shell Specification

## ADDED Requirements

### Requirement: command body 纪律与 app 层微形态

desktop-app SHALL NOT 抽独立 app 层 crate：command 即应用服务，维持既有双轨（queries / workspaces）加 exec 空轨道的组织不变。作为补偿纪律，任何 Tauri command body SHALL 只允许三件事：

1. **参数转换**（IPC 入参 → 领域/store 入参）；
2. **调用**（core 函数或 store 操作）；
3. **错误映射**（领域/Store 错误 → `Err(String)`）。

command 层 MUST NOT 实现领域解释（属领域解释的编排 SHALL 下推 core）或跨边界协调（属跨边界协调的编排 SHALL 触发 app crate 决策，见下一 requirement），MUST NOT 以"先塞进命令里"的方式消化编排增长。

`commands/workspaces` 的 `*_inner(&Store)` 纯函数模式 SHALL 视为 app 层微形态（IPC 适配与纯逻辑的函数级分层）予以保留：将来抽 app crate 时 SHALL 将 inner 函数平移复用（函数边界升 crate 边界），MUST NOT 重写。

#### Scenario: 新命令符合三件事

- **WHEN** 新增任一 Tauri command
- **THEN** body 为参数转换 + 调用 + 错误映射三段，无编排逻辑、无领域解释、无跨 store / fs 协调

#### Scenario: 编排增长被下推而非上塞

- **WHEN** 某命令需要新增一段编排逻辑（解析、校验、多步写等）
- **THEN** 领域解释部分落入 core crate，命令层仅保留三件事；跨边界协调则触发 app crate 决策，而非继续膨胀命令体

#### Scenario: inner 函数缝保留

- **WHEN** 审查 `commands/workspaces` 实现
- **THEN** IPC 命令为薄包装并经 `*_inner(&Store)` 纯函数调用 store，错误映射在命令层完成；该模式未被"内联回命令体"的重构破坏

### Requirement: app crate 翻转信号

出现下列任一信号时 SHALL 重新决策是否抽独立 app 层 crate（决策触发，不等架构回顾）：

1. Rust 侧引入 phase lifecycle 类命令（parse + validate + mutate + write 的真编排；TS 端 phase_start / phase_log 已落地，Rust 对齐时即触发）；
2. exec 轨道第一条真实命令落地（写 repo：保护检查 + 审计 + 执行 + touch 协调）；
3. 命令出现跨 store + fs 协调（如 add workspace 后自动 rescan）；
4. CLI / headless 复用桌面后端逻辑的需求出现；
5. 机械判据兜底：单命令非 IPC 样板逻辑超过一屏（约 30 行），或 `commands/` 非样板逻辑合计持续增长。

翻转信号清单 SHALL 保持可机械判定（命令类型可枚举、行数可数、复用需求可证），SHALL 随本能力 spec 存档，供代码评审与后续变更 proposal 引用。

#### Scenario: 信号可机械判定

- **WHEN** 审查翻转信号清单
- **THEN** 每条信号可由代码状态直接判定（特定命令是否出现 / 命令体行数 / 复用需求是否提出），无需主观架构评价

#### Scenario: 信号触发即重议

- **WHEN** 任一翻转信号出现（如 exec 第一条真实命令进入 design）
- **THEN** 对应变更的 proposal 显式回应"是否抽 app crate"的重新决策，而不是默认沿袭"不建"

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `desktop-app::commands::*`（全体命令） | 应用服务（command 即应用服务） | body 三件事：参数转换 / 调用 / 错误映射；无独立 app crate；编排下推 core 或触发翻转 |
| `commands::workspaces::{list,add,remove,touch}_workspace_inner` | app 层微形态 | 纯函数 + `&Store` 入参；将来抽 crate 时平移复用，不重写 |
| app crate 翻转信号 | 决策触发器 | 五条信号（phase lifecycle 命令 / exec 首命令 / 跨 store+fs 协调 / CLI 复用 / 行数机械判据），任一出现即重议 |
