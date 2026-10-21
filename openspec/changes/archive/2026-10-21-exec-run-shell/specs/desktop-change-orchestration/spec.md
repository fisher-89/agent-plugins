# desktop-change-orchestration Specification（delta: exec-run-shell）

## ADDED Requirements

### Requirement: 停止贯通到进程执行（cancel 传播）

run 的停止信号 SHALL 由「相位 / 步边界观测」贯通到**运行中的外部进程**：

- **注册表侧**：`RunEntry` SHALL 持取消标志（`foundation::cancel::CancelFlag`，与既有取消 `watch` 同道），`begin_run` 时铸造；`request_stop()` SHALL **双写**——既有 `watch::send_replace(true)` 保留（停等 / 应答等待的异步观测面不变）**且** `cancel_flag.set()`（无订阅者也不丢位）；重复停止幂等。
- **控制柄侧**：`RunGuard` SHALL 暴露 `cancel_flag() -> CancelFlag`（克隆共享同一原子，供同步读取）。
- **请求侧**：`ToolStepRequest` SHALL 增 `cancel: CancelFlag` 字段；三个检查域 runner port 签名 SHALL 各加一参——`StaticCheckRunner::run(root, name, slot, cancel)`、`TestExecutionRunner::run(root, name, slot, cancel)`、`MutationRunner::run(root, name, files, patch, cancel)`。
- **透传侧**：`LocalToolSteps` SHALL 把 `step.cancel` 透传给三 runner（`steps.rs`）；walker 的 `run_tool` SHALL 在**单点**以 `guard.cancel_flag()` 填充该字段（MUST NOT 逐调用点各写一遍取值逻辑）。
- **终态语义（过渡形态，显式留痕）**：取消使 runner 返回 `Err("执行已取消")`，walker 按既有映射把该步收敛为 failed、run 收敛为 failed——MUST NOT 在本变更内改判 run stopped（正式收敛 = `fault = Stopped` → run stopped 属后续变更；本要求只保证「进程确实被杀掉、不残留、run 秒级收敛」）。

#### Scenario: 停止运行中的检查步秒级收敛

- **WHEN** user 对正在执行 static-check / test-execution / mutation 步的 run 发停止命令
- **THEN** 取消标志置位 → 执行体终止进程树（不残留 rustc 类子孙）→ runner 返回 `Err("执行已取消")` → 该步 failed、run 在秒级收敛（MUST NOT 等命令自然结束，如等 340–393 s 的 suite 跑完）

#### Scenario: 双写幂等且无订阅者不丢

- **WHEN** 对同一运行中 run 连续两次调停止命令，或该 run 当前无任何 watch 订阅者
- **THEN** 两次调用均不报错（幂等），且第二次后 `RunEntry` 的取消标志仍为已置位（`CancelFlag::is_set()` 为真）——置位不因无订阅者丢失

#### Scenario: 终态过渡语义显式

- **WHEN** 停止导致检查步取消
- **THEN** 该步节点收敛为 failed、其失败文本含「执行已取消」；run 终态为 failed（非 stopped），且该过渡口径在 spec 中留痕（后续变更改判 stopped 时须显式修订本条）

#### Scenario: 缺省取消信号零行为变化

- **WHEN** 既有测试 / 非取消路径以 `CancelFlag::never()` 构造 `ToolStepRequest` 与 runner 调用
- **THEN** 行为与本变更前逐字等价（执行体走正常完成臂），既有 walker / steps / port 测试除签名外零改动

#### Scenario: 透传单点

- **WHEN** 审查 walker 中 `ToolStepRequest` 的构造点
- **THEN** `cancel` 仅在 `run_tool` 一处从 `guard.cancel_flag()` 取值填充，各步函数不各自取值；`steps.rs` 仅做字段透传不做决策

## Module Contract

| 模块 | 定位 | 关键契约（Function / API） |
|------|------|------|
| `crates/core/orchestration/src/control.rs` | run 控制注册表 | `RunEntry.cancel_flag: CancelFlag`；`request_stop(root, change_id) -> bool` 双写（`watch::send_replace(true)` + `cancel_flag.set()`）；`RunGuard::cancel_flag(&self) -> CancelFlag` |
| `crates/core/orchestration/src/port.rs` | port 契约 | `ToolStepRequest { root, command, cancel: CancelFlag }`；`StaticCheckRunner::run(root, name, slot, cancel)`、`TestExecutionRunner::run(root, name, slot, cancel)`、`MutationRunner::run(root, name, files, patch, cancel)` |
| `crates/core/orchestration/src/steps.rs` | 组合根直调 | `LocalToolSteps::execute` 把 `step.cancel` 透传给三 runner（零决策） |
| `crates/core/orchestration/src/walker.rs` | 透传单点 | `run_tool(tools, root, cancel, command, label)`：`cancel` 由 `ctx.guard.cancel_flag()` 取；取消步收敛 failed（过渡） |
| `crates/infra/checks` | runner 实现 | 三 runner 消费 `cancel` 并经 `exec::run_shell` 观测（见 desktop-checks-domain / desktop-mutation-testing） |
| `crates/infra/agent` / `crates/infra/vcs` | 不涉及 | 假件签名随 port 更新（`lib_test.rs`），生产实现零改动 |

**API 接口**：无（`RunUpdate` / 节点词汇 / 快照 DTO 零扩展，无新 IPC 命令，bindings 零 diff；停止命令的请求-应答形态与幂等语义不变）。
**CLI 命令**：无。
**前端组件**：无（前端零改动：停止按钮与流程视图契约不变，收敛终态由后端下发）。
