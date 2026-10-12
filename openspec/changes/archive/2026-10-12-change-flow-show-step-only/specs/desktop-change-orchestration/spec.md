# desktop-change-orchestration Specification (Delta)

## MODIFIED Requirements

### Requirement: static-check spawn 步门禁

implement / test-gen 相位的 executor 收口后，walker SHALL 必经 static-check 步（checks 边界 `crates/infra/checks` spawn 执行，承接原 SubagentStop hook 的门禁职责；该步由相位 step 定义声明，非 `STATIC_CHECK_PHASES` 常量注入）。static-check 步 SHALL 经 id → 记录 → name 解析（`store.get_change(id).map(name)`，与 test-execution 分支同式）把 change name 传入 `StaticCheckRunner::run(root, name)`——name 供 infra 侧 output 落盘（recorded，`output_ref` 回填，`<report_dir>/exec-outputs/<exec_id>.log`）；未建档（无 `ChangeRecord`）显式 `Err`。static-check 失败 SHALL 走**定向反馈边**：将捕获的诊断反馈注入同一 executor 会话（Continue）修复，独立计数上限 5 次（沿原 hook `loop_limit` 语义）；每次修复重跑为 `instance_no` 递增的新节点、图上以 `feedback` 边表达。超限 SHALL 升格为相位 fail（walker 以桌面代写的 fail checklist 经写面 `phase_log` 落账，不跑 evaluator，进入重试 / 决策路径并消耗相位 retry 预算）。MUST NOT 将反馈边重试直接计入相位 retry 预算。非 implement / test-gen 相位 MUST NOT 触发 static-check（其 step 序列不含 static-check 步）。

#### Scenario: static-check 步 name 解析与 recorded

- **WHEN** walker 走到某已建档 change 的 static-check 步（假 store 供给 name）
- **THEN** `StaticCheckRunner::run(root, name)` 收到该 change 的裸名，`StaticCheckOutcome.output_ref` 回填（passed 与 failed 两分支均携 `output_ref`），节点可经 stdout 面板回查；change 未建档则显式 `Err`

#### Scenario: 修复反馈边

- **WHEN** executor 收口后 static-check 报 lint / fmt 错误
- **THEN** walker 将诊断文本注入同一 executor 会话（Continue）重试修复，反馈边计数 +1，新修复节点 `instance_no` 递增；修复后 static-check 通过则相位继续 evaluator 步

#### Scenario: 反馈边超限升格

- **WHEN** 同一相位内定向反馈边已达 5 次仍不通过
- **THEN** walker 以代写 fail checklist 经写面 `phase_log` 落账（不跑 evaluator），相位按 fail 收场进入重试 / 决策路径，walker 不再注入反馈边

#### Scenario: 门禁相位限定

- **WHEN** proposal / dev-design 等非 implement / test-gen 相位 executor 收口
- **THEN** walker 不发起 static-check spawn（该相位 step 序列不含 static-check 步），直接进入 evaluator 步

### Requirement: 版本交付

本变更 SHALL 将 `packages/desktop/package.json` 的 `version` 由 `0.4.37` 升级为 `0.4.38`（`src-tauri/tauri.conf.json` 经 `../package.json` 自动跟随，`src-tauri/Cargo.toml` 版本不随动）。本变更 SHALL NOT 变更 `plugins/dev-team`（版本保持 `2.10.44` 与三类交付产物）。

#### Scenario: 版本号升级与插件零改动

- **WHEN** 本变更实现完成（归档时执行版本升级）
- **THEN** `packages/desktop/package.json` 的 version 为 0.4.38；`plugins/dev-team` 版本与交付产物零改动

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/orchestration/src/port.rs` | static-check port 契约 | `StaticCheckRunner::run(root, name)`（对齐 `TestExecutionRunner::run(root, name)`）；`StaticCheckOutcome.output_ref` 语义不变 |
| `crates/core/orchestration/src/steps.rs` | static-check 消费点 name 解析 | `ToolCommand::StaticCheck` 分支经 `store.get_change(id).map(name)` 解析 change name（抄 TestExecution 分支）；未建档显式 `Err` |
| `src/commands/change_flow/mod.rs` | 组合根装配 | `ProcessStaticCheck::new()` 构造不变（去 `recorded()` 装配分支，recorded 默认化后注释随动）；`run(root, name)` 由 steps 消费点传 name |
