# 提案: desktop-change-flow-auto-next-phase

> **变更**: desktop-change-flow-auto-next-phase
> **日期**: 2026-10-03
> **状态**: 草案

---

## 问题

desktop change flow run 的停等节奏是「phase 内自动、phase 间停等确认」（`orchestration/src/walker.rs` drive() 主循环⑥：相位落账 pass 后 `guard.emit(RunUpdate::ConfirmWait)` + `guard.wait_confirm().await` 挂起，至 `change_flow_confirm` 应答才推进）。该节奏对所有 run 一刀切：

1. **逐相位手动确认打断推进**：requirement 全管线 9 相位，每个相位收口都要人工点一次「继续」；对内容可预期、想一口气跑完的 change，确认本身不产生判断价值（相位已 pass 落账），纯节奏税。
2. **无人值守不可用**：挂机跑 run 的场景下，run 每相位停等即永久挂起在 `waitingConfirm`，无人点继续则 run 停在半途。

需要一种**可选**的「自动确认」发起模式：phase 间停等跳过、run 自动推进至终态；默认节奏（手动确认）保持不变。停等节奏是 run 级属性（walker 头注释「拍板停等节奏」），run 生命周期由后端注册表承载、不依赖前端在位——生效层必须在后端。

---

## 提案

**发起参数定格 + walker 分支生效**（explore 已收敛，否决前端自动应答 / 后端 watcher 两条路）：

1. **walker 分支**：`RunRequest` 增加 `auto_confirm: bool` 字段；drive() 确认点 `if !request.auto_confirm` 才 `emit(RunUpdate::ConfirmWait)` + `wait_confirm()`。auto 模式 run 根本不进 `WaitingConfirm` 态、不产生 `ConfirmWait` 事件——`RunUpdate` / `ChangeRunSnapshot` DTO 与 `control.rs` 零改动，重挂恢复路径天然兼容。
2. **命令面透传**：`change_flow_start` / `change_flow_start_with` 增加 `auto_confirm: bool` 参数，透传 `RunRequest`；bindings 重导（`changeFlowStart` + `autoConfirm`）。
3. **前端开关**：`RunControlPanel` 发起区增加「自动确认」开关（默认关），`useChangeFlowRun.start(autoConfirm)` 传参；开关为发起参数、发起时定格，运行中不可切（手动改自动可停止重发自 `active_phase` 续走，成本低；auto 改手动同路径）。开关旁一行说明文案轻提示，不做阻断式确认。
4. **语义边界一——ask 中断不自动应答**：`waitingAsk` 来自决策 agent 重试预算耗尽后的真实提问，恰恰需要人的判断；auto 模式下 ask 到来照常停等应答。自动确认只作用于 phase 间停等。
5. **语义边界二——终态不自动归档**：全相位 pass 照常以 `completed` 收口（"Ready for archiving"）停给用户，归档保持手动（与 no-auto-archive 偏好一致，现有代码即如此）。
6. **版本**：用户可见变更（新发起模式）→ 归档时 desktop 版本 bump（0.4.2 → 0.4.3）。

---

## 能力

### 新增能力

- 无（本变更不新建能力，均为既有能力的增量修改）。

### 修改的能力

- **desktop-change-orchestration** — 薄图 walker 停等节奏增加 `auto_confirm` 发起参数分支（auto 模式跳过 phase 间停等、不进 `WaitingConfirm`；ask / 终态语义边界定格）；运行控制命令面发起入口增加 `auto_confirm` 参数透传。
- **desktop-change-flow-view** — 运行控制面板发起区增加「自动确认」开关（默认关、运行期定格、轻提示文案）；auto 模式 run 不呈现确认卡片，ask 卡片照常。

---

## 变更范围

### 实现文件

- `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` — `RunRequest` + `auto_confirm` 字段；drive() 确认点分支（`if !request.auto_confirm` 才 emit + wait）；头注释停等节奏口径补 auto 模式。
- `packages/desktop/src-tauri/src/commands/change_flow/mod.rs` — `change_flow_start` / `change_flow_start_with` 签名 + `auto_confirm: bool`，透传 `RunRequest`。
- `packages/desktop/src/types/generated/bindings.ts`（生成物重导，`pnpm run bindings:export`）— `changeFlowStart` + `autoConfirm` 参数。
- `packages/desktop/src/views/changes/hooks/use-change-flow-run.ts` — `start(autoConfirm)` 传参。
- `packages/desktop/src/views/changes/flow/run-control-panel.tsx` — 发起区「自动确认」开关（默认关）+ 轻提示文案；运行期开关定格。
- `packages/desktop/package.json` — 归档时 version 0.4.2 → 0.4.3（不在实现任务列表，随归档提交执行）。

### 测试文件

- `packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs` — 新增 auto_confirm=true 多相位 run 直通用例（更新流零 `ConfirmWait`、直通 `completed`）；既有用例 `RunRequest` 构造点补 `auto_confirm: false`，手动模式语义回归不变。
- `packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs` — start 透传 auto_confirm 用例（auto 参数下达后事件/快照面无 `ConfirmWait`）；blank 守卫模板沿用。
- `packages/desktop/src/views/changes/flow/run-control-panel.test.tsx` — 开关默认关、开启后 start 携 `autoConfirm=true`、运行期不可切、auto run 不出确认卡片、ask 卡片照常；data-testid 挂钩。

### 删除文件

- 无。

### 不要修改

- `crates/core/orchestration/src/control.rs` — `ChangeFlowControl` / `RunGuard` / confirm 应答通道零改动（auto 分支在 walker 侧，不触碰控制面）。
- `crates/core/orchestration/src/state.rs` — `RunUpdate` / `ChangeRunSnapshot` / `ChangeRunStatus` DTO 零改动（auto 模式不产生新事件、不进 `WaitingConfirm`）。
- `crates/core/workflow/` 全域 — golden wire 契约在 workflow.json 解析层，不涉 change_flow 命令签名；`tests/golden` 不可重写。
- 前端 `views/changes/flow/run-state.ts` — 纯 reducer 零改动（无新事件词汇）。
- 决策协议 ask 面与 `change_flow_answer` — ask 不自动应答。
- `plugins/dev-team` — 插件零触点。

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | walker `RunRequest.auto_confirm` 字段 + drive() 确认点分支 | auto_confirm=true 的多相位 run 更新流零 `ConfirmWait` 信封、快照面全程不经 `waitingConfirm`，逐相位推进直通 `completed`（reason = "All phases have passed evaluation. Ready for archiving."）；walker_test 直测 |
| AC-2 | 手动模式分支保持 | auto_confirm=false（默认）既有停等语义不变：`ConfirmWait` 逐相位流出、挂起至 `change_flow_confirm`，proceed=false / 等待期取消 → 受控 `stopped`（既有用例全绿） |
| AC-3 | `change_flow_start` + auto_confirm 透传 | 命令签名 + `auto_confirm: bool`，bindings 重导后 `changeFlowStart` 带 `autoConfirm`；mod_test 证明 auto 参数达 walker（事件/快照面无 `ConfirmWait`） |
| AC-4 | 前端发起开关 | 发起按钮旁开关默认关；开启后 start 携 `autoConfirm=true`；运行期开关定格不可切；测试以 data-testid 查询 |
| AC-5 | ask / 终态语义边界 | auto 模式下决策 ask 照常进入 `waitingAsk` 停等应答（不自动选择）；全相位 pass 照常 `completed` 收口停给用户，不触发归档 |
| AC-6 | 版本 | 归档时 `packages/desktop/package.json` version 0.4.2 → 0.4.3（`tauri.conf.json` 经 `../package.json` 跟随，`Cargo.toml` 不随动） |
| AC-7 | 全管线 | `cargo test --workspace` / `vp test` 全绿；`bindings:check` 无 diff；knip 无新增豁免条目 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| auto 模式无人监督连续推进 9 相位，token 消耗失控 | 成本失控 / 失败相位反复重试无人拦 | 中 | 开关旁一行说明文案轻提示；既有封顶仍在（fail 重试预算 ≤5、static-check 反馈边 ≤5）；ask 中断与失败终态仍停给人 |
| auto 模式被误用为「免监督全自动交付」 | 产出质量预期错配 | 低 | 说明文案明示语义边界（仅跳过 phase 间确认，ask / 失败 / 终态仍停）；终态不自动归档 |
| 重挂恢复错过停等 | 前端重挂丢状态 | 低 | auto 模式无 `ConfirmWait` 事件、快照面无新状态，DTO 零改动重挂天然兼容（explore 已验证） |
| 命令层参数与 walker 字段漂移 | 运行期静默失配 | 低 | specta 绑定编译期校验 + tsc 拦截 + walker_test / mod_test 直测 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|------|------|------|----------|
| 生效层放哪 | walker 确认点分支（`RunRequest.auto_confirm`） | 前端自动应答致命伤：推进依赖前端挂载，切视图 / 窗口事件断流即卡死在 `waitingConfirm`，违背 run 生命周期后端承载原则；后端 watcher 自动放行则引入隐藏控制流 | 前端定时自动 confirm（否决）；注册表 watcher 自动放行（否决） |
| 开关粒度 | 发起参数、发起时定格（运行中不可切） | 停等节奏是 run 级属性（walker 头注释「拍板停等节奏」）；运行中切换语义不对称且复杂——手动改自动可停止重发自 `active_phase` 续走，成本低，无需新控制面 | 全局持久化设置（否决：run 级属性不该全局化）；运行中切换命令（否决：新增控制面复杂度） |
| ask 是否自动应答 | 不自动应答 | `waitingAsk` 来自决策 agent 重试预算耗尽后的真实提问，恰恰需要人的判断 | ask 自动选第一项（否决：臆测用户决策） |
| 终态是否自动归档 | 不自动归档 | 与用户 no-auto-archive 偏好一致，现有代码即此行为 | completed 后自动 archive（否决） |
| auto 模式 UI 提示形态 | 开关旁一行说明文案，不做阻断式确认 | 轻量可发现；阻断式确认与「省去确认」的目的自相矛盾（explore 开放问题按此收敛） | 首次开启弹确认（否决） |

### 待决问题

- 无重大遗留。轻提示的具体文案措辞由 design / 实现定夺；开关运行期的呈现形态（禁用 vs 隐藏）由 design 定夺。

---
