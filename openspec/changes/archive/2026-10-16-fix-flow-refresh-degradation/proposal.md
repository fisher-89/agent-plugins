# 提案: fix-flow-refresh-degradation

> **变更**: fix-flow-refresh-degradation
> **日期**: 2026-10-16
> **状态**: draft
> **探索**: `openspec/changes/fix-flow-refresh-degradation/explore.md`（2026-10-15 定稿：真机取证三因素分解 + 层 1–4 修复方案与取消语义 / 陈旧容忍措辞定夺在案）

---

## 问题

桌面端发起 run 后，流程图偶发「整页塌掉」：节点清零、徽章消失、无停止按钮，页面呈「未找到该 change」（run 明明在飞），到下一条通知才自愈；期间步迁移的更新被整轮吃掉。真机取证（CDP 驱动真实 UI 发起 run + 并行直接 invoke 后端对照）将观感问题分解为三因素：

1. **长工具步静默是设计使然**（非缺陷）：test-execution（1765 测试 ≈5min）、mutation-test、agent 会话期间零通知，节点仅在步 starting 时上图一次。发起后 ~1s 首刷新链路证实是活的。
2. **缺陷（复现一次）**：步迁移写入突发（`append_step×2` + record upsert）与通知触发的重查同刻竞争，该轮 `get_change_detail` 失败 → 前端 `loadUnifiedView` 走 `apply({ detail: null, activeRun: null, artifacts: [] })` 整页塌兜底，迁移间的更新被吃掉。
3. **吞错面放大**：`crates/core/workflow/src/queries/detail.rs` 的 `store.get_change(id).ok().flatten()?` 把读 `Err` 降级成 `None` → 前端呈「未找到该 change」（运行中！）；且 `activeRun: null` 连锁把 `activeRunPresent` 翻 false → `change_flow_watch` 订阅被 effect cleanup 拆除，恢复只剩「下一条通知」一条腿。

用户现场（库取证）：09:24 发起 run-1792026966457，test-execution attempt4 通过、mutation-test 通过、phaselog 落账、停在相位间确认；8 分钟静默观感后用户点停止。另外前端 `change-detail-view.tsx` 对 `error !== null` 一律整页 `DetailFallback`——刷新轮失败（detail 在手）同样塌页，是第 2 因素的前端半边放大面。

---

## 提案

沿 explore 定稿的分层方案（层 4 已定拆出，不在本 change）：

1. **层 1：失败轮降级契约（核心，前端）**——通知（或显式刷新）触发的统一视图重查失败轮，在 detail 在手时**保 last-good**：不清 `detail` / `activeRun` / `artifacts`、不塌整页（无 nodes=0 空图、无「未找到」兜底页）；以**内联「刷新失败」提示**如实标示（非整页 fallback；呈现位置与 data-testid 挂钩 design 定夺）。首载失败（无可保）维持既有兜底页不变。**有界重试**：3 次、退避 1s / 2s / 4s（常量单点），穷尽后静默回落「靠下一条通知兜底」——理由是终态竞态洞：walker 写序 `run_finished`（整包落库）→ `guard.finish`（发通知），`Finished` 是 run 最后一条通知，300ms 去抖后的终态重查若失败且只靠通知恢复 = 永无下一条，必须自带重试。**取消语义**：重试计时器与去抖同生命周期——任何 tick 变化（新通知、手动刷新、换 root / id、卸载）即作废挂起的重试并重置退避序列；重试是「自动补发的刷新意图」，与通知 / 手动刷新共用同一条取消通道，任一时刻最多一个待生效的刷新意图，不产生交错重复查询。保 last-good 期间面板动作（停止 / 确认 / 应答）照常——动作 invoke 不依赖前端态。
2. **层 2：后端错误语义分流（观测性）**——`get_change_detail` 出线区分：**None = 真「未找到」**（记录缺席）→ 兜底页语义不变；**Err = 读失败**（store 错误串）→ 前端走层 1 保 last-good + 内联提示。消掉 `change_detail` 内 `get_change` 的 `.ok()` 吞错：core 签名 `Option<ChangeDetail>` → `Result<Option<ChangeDetail>, StoreFault>`（仅 `get_change` 读 Err 传播；其余子表读的 `unwrap_or_default` 空面降级哲学不变）；命令面 `Result<Option<ChangeDetailUnified>, String>`（blank root 仍 `Ok(None)`）。效果：同类竞态下次发生时错误串自现，根因（redb 读事务 vs 写并发的瞬时错？）不阻塞本方案、由观测面浮出。
3. **层 3：订阅粘性（随层 1 自然解决）**——`activeRunPresent` 挂在单轮查询结果上；层 1 保 last-good 后 `activeRun` 不清、`activeRunPresent` 自然粘住不闪，`change_flow_watch` 订阅不被拆。仅补测试验证，不做「mount 即订阅」等独立改造。
4. **层 4（已定拆出，不在本 change）**——长步活性反馈（节点携 started_at + 前端渲染「已运行 XmYs」计时）动 `ChangeStepState` 活面时间戳 + snapshot 投影 + DTO（change 域 golden 显式重写）+ 节点渲染，纯 UX 增强，独立 change 承载。

波及面收敛说明：`Result<Option<T>, String>` 经 bindings 出线仍为 `Promise<T | null>`（Err 走 reject 通道），TS 类型面零漂移；DTO 线面形状零演进 → golden 零重写。orchestration 侧唯一调用点 `snapshot.rs`（`WorkflowSnapshotPort::detail`）机械适配：`Err` 由 store 错误串透传（取代误报「change 不存在」），`None` 仍按「change 不存在」——port 契约与 walker「快照读取失败: {message}」文案格式不变。

---

## 能力

### 新增能力

- 无（两关注点均落既有能力：错误语义分流 = 查询层读面语义；失败降级 = 流程视图取数模型）。

### 修改的能力

- **desktop-change-queries** — 新增「详情查询错误语义分流（None / Err）」requirement（`get_change` 读 Err 传播 / 未知 id 仍 `Ok(None)` / 子表降级哲学不变 / 命令面 Result 化与错误串透传 / bindings 零漂移）；MODIFIED「change 详情聚合」——「详情标示『运行中』MUST NOT 假象」条款**保持原强度**，仅追加一行指向 desktop-change-flow-view 有界降级例外的引用（豁免的是有界、带失败提示、收敛保证的过渡态，非无界静默假态）。
- **desktop-change-flow-view** — 新增「通知重查失败降级与有界重试」requirement：「通知仅失效信号、查询结果权威」维持；失败轮保 last-good + 内联提示（信号界限主条款）+ 陈旧窗口有界（时间界限辅条款：≤3 次退避自愈或下一条通知恢复）+ 有界重试与取消语义（新意图取代旧重试）+ 终态重试兜底（Finished 竞态洞）+ 订阅粘性（随保 last-good 自然达成，仅补测试）。

### 引用沿用（零 delta）

- desktop-change-orchestration — walker 写序（`run_finished` → `guard.finish`，已核验）与 `RunNotice` 六 kind 通知面零改动；`snapshot.rs` 为机械适配（错误串保真度提升，port 契约不变）。
- desktop-ipc-type-bindings — bindings 再生走既有管线（`Result<Option<T>, String>` → `Promise<T | null>`，预期零 diff），一致性守卫不变。
- desktop-app-shell — 命令面零新增（`get_change_detail` 为签名随动非新命令）；`list_changes` / `read_artifact` / `read_exec_output` 的 Option 空结果语义不动。

---

## 变更范围

### 实现文件

- `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs` — `change_detail` 签名 `Option<ChangeDetail>` → `Result<Option<ChangeDetail>, StoreFault>`：`get_change` 读 `Err` 传播（消 `.ok()` 吞错）；`list_phase_records` / `list_runs` / `list_run_nodes` / `list_run_edges` 的 `unwrap_or_default` 哲学不动
- `packages/desktop/src-tauri/src/commands/changes/mod.rs` — `get_change_detail` 返回 `Result<Option<ChangeDetailUnified>, String>`：blank root 仍 `Ok(None)`；`StoreFault` → 错误串映射收命令层单点（形态 design 定稿）
- `packages/desktop/src-tauri/crates/core/orchestration/src/snapshot.rs` — 机械适配：`Err` 透传 store 错误串，`None` 仍「change 不存在: {id}」（port 契约与 walker 消费面不变）
- `packages/desktop/src/types/generated/bindings.ts` — 再生成（预期零 diff：Err 走 reject 通道，类型面不变）
- `packages/desktop/src/views/changes/hooks/use-change-detail.ts` — 失败轮分流（reject 且 detail 在手 → 保 last-good + 内联失败态 + 退避重试调度）；重试计时器与 tick / 去抖同生命周期（新意图取代旧重试、卸载清计时器）；退避常量（3 次 / 1s / 2s / 4s）单点
- `packages/desktop/src/views/changes/change-detail-view.tsx` — 内联「刷新失败」提示呈现（data-testid 挂钩）；`error` 分支收敛为首载语义（detail 在手的刷新失败不再整页 `DetailFallback`）；首载错误 / 真 `None` 兜底页分支不变
- `packages/desktop/package.json` — `version` 0.5.10 → 0.5.11（`src-tauri/Cargo.toml` 不随动）

### 测试文件

- `crates/core/workflow/src/queries/detail_test.rs` — `get_change` Err → `Err` 透传断言（fault 错误串逐字）；未知 id 仍 `Ok(None)` 回归；子表读失败不升级整份 `Err`（单区块空面哲学保持）
- `packages/desktop/src-tauri/src/commands/changes/mod_test.rs` — 命令面错误透传回归（Err 面错误串、非 `None` 应答）；blank root `Ok(None)` 语义不变；`snapshot` 机械适配回归（`snapshot_test.rs` 随动）
- `packages/desktop/src/views/changes/hooks/use-change-detail.test.ts` — jsdom 注入「第 N 轮 invoke 失败 / 返回空」序列：保 last-good（detail / activeRun / artifacts 不清）、重试节奏（fake timers 断言 1s / 2s / 4s 与 ≤3 次）、穷尽后静默（无第 4 次自动查询）、新意图取消（通知 / 手动刷新作废挂起重试且退避重置）、卸载清计时器、首载失败整页 error 兜底
- `packages/desktop/src/views/changes/change-detail-view.test.tsx` — 内联「刷新失败」提示呈现与消失；降级窗口停止 / 确认 / 应答入口在案；真 `None` 清面兜底页回归
- `packages/desktop/src/views/changes/hooks/use-change-flow-run.test.ts` — 订阅粘性：失败轮 `activeRun` 保持 → `activeRunPresent` 不翻 false → 订阅不拆、下一通知照常抵达

### 删除文件

- 无。

### 不要修改

- `crates/core/orchestration/src/walker.rs` 写序与 `RunNotice` 通知面（已核验：`run_finished` → `guard.finish`；六 kind-only 不动）
- `queries` 其余读面哲学（`list_phase_records` / `list_runs` / `list_run_nodes` / `list_run_edges` 的 `unwrap_or_default`）与 `list_changes` / `read_artifact` / `read_exec_output` IPC 签名（产物单读失败已有 Fallback 信封降级，不扩面）
- 通知去抖常量 D8（统一视图 300ms / 转录 150ms）与 `use-change-flow-run.ts` 订阅生命周期实现（层 3 仅补测试）
- 转换层纯函数（`flow/graph.ts` / `attachments.ts` / `run-state.ts`）与既有 9 列布局 / 边表读取规则
- golden 语料与 `DESKTOP_GOLDEN_REWRITE` 流程（DTO 线面形状零演进，零重写）
- 层 4 长步活性反馈（started_at + 「已运行 XmYs」）——独立 change 承载
- `plugins/dev-team` 全部（MCP 工具、hooks、版本与三类交付产物）
- `openspec/specs/**` 既有基线 spec（本变更只写 `openspec/changes/<name>/specs/**` delta，归档时合并）

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | 失败轮保 last-good | jsdom 注入第 2 轮起 `get_change_detail` reject（detail 已在手）：`detail` / `activeRun` / `artifacts` 维持上一成功轮值（nodes 不清零、徽章与停止入口在案），内联「刷新失败」提示呈现（data-testid 可查），MUST NOT 出现「未找到该 change」兜底页或 nodes=0 空图 |
| AC-2 | 有界重试节奏 | 失败后 fake timers 断言重试间隔 1s / 2s / 4s 且至多 3 次自动补发；某次重试成功则提示消失、图面更新到最新（迁移间更新不被吃掉）；3 次穷尽后静默（无第 4 次自动查询），等待下一条通知兜底 |
| AC-3 | 重试取消语义 | 重试计时器挂起期间新通知到达（或手动刷新 / 换 root / id / 卸载）：旧重试作废、退避序列重置、不产生交错重复查询（invoke 计数可断言） |
| AC-4 | 终态重试兜底 | 注入「Finished 通知 → 终态重查失败 → 重试成功」序列：终局自愈不悬（不依赖「下一条通知」——Finished 已是最后一条）；全程 last-good 不塌 |
| AC-5 | 后端 None / Err 分流 | mock store `get_change` 返回 `Err(StoreFault)`：core `change_detail` 出 `Err`（错误串透传，MUST NOT 降级 `None` / 空面）；未知 id 仍 `Ok(None)`；命令面以错误串 reject（非 `null` 应答）；子表读失败仍单区块空面（不升级整份 `Err`） |
| AC-6 | 首载与真 None 兜底不变 | 首次加载即失败 → 整页错误兜底页（既有语义）；重查成功返回 `None`（真未找到）→ 清面「未找到该 change」兜底页（既有语义）；blank root 命令面 `Ok(None)` |
| AC-7 | 订阅粘性 | 失败轮保 last-good 期间 `activeRunPresent` 保持真值，`change_flow_watch` 订阅不被 effect cleanup 拆除，下一通知照常触发重查（测试断言零重订 / 零丢通知） |
| AC-8 | 降级窗口动作可用 | 保 last-good 窗口内停止 / phase 间确认 / ask 应答动作照常可发（独立 invoke 不受降级态影响），控制入口状态与 last-good `activeRun` 一致 |
| AC-9 | 零漂移与观测性 | bindings 再生成零 diff（`getChangeDetail` 类型面不变）、一致性守卫绿、golden 零重写；注入 store fault 时错误串自现于前端内联提示（观测面成立——根因钉死不在本轮） |
| AC-10 | 真机走查留痕 | 修复后以 CDP 手法做一次真机走查（发起 → 长步 → 迁移 → 收口全程图面不塌、终态不悬）并留痕；真机竞态本身不作为复现 AC（不可稳定复现） |
| AC-11 | 版本交付 | `packages/desktop` version 0.5.11（`src-tauri/Cargo.toml` 保持 0.1.0）；`vp check` / `vp test` / knip 全绿，无新增豁免 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| R1 store 读错根因未钉死（redb 读事务 vs 写并发瞬时错？） | 层 2 只分流不根治；若错误为持续性，有界重试穷尽后仍靠通知兜底，降级窗口拉长 | 中 | 错误串经层 2 自现（AC-9 观测面），根因另立 change 钉死；窗口内有内联提示 + 显式刷新可用 |
| R2 计时面复杂度：`useChangeDetail` 已有双去抖计时器，再加重试计时器 | 状态翻倍、`max-lines-per-function: 50` 压力、交错 bug | 中 | 重试与去抖共用同一条取消通道（单点）；调度落位（ref 计时器 vs 拆 scheduler）design 定稿；fake timers 全节奏单测（AC-2 / AC-3） |
| R3 终态重试全失败的停留态：3 次退避（共 ~7s）内未自愈且无下一条通知 | 页面停留 last-good 运行中态（有内联提示） | 低 | 有界 + 有提示 + 显式刷新可用的过渡态（立法辨析：非无界静默假态）；文案引导；AC-4 验证常态自愈 |
| R4 mid-session 真 `None` 与失败轮分流误判：后端残留降级路径（如 blank root）把错误伪装成 None | 误清面（把降级当未找到） | 低 | 命令面 Err 全链只增不改；blank root 时前端本就早退（`root === null` 不发查询）；AC-5 / AC-6 双向断言 |
| R5 `snapshot.rs` 波及：错误文案由「change 不存在」变为 store 错误串 | walker 终态「快照读取失败: {message}」内容变化 | 低 | 机械适配 + `snapshot_test.rs` 回归；语义提升（不再误报不存在），消费面格式不变 |
| R6 并行 change（worktree-cleanup-and-active-cap 未归档）同样 MODIFIED「change 详情聚合」 | 归档合并时两 delta 需逐段并集（词汇 十九→二十 由彼 change 承载，指针行由本 change 叠加） | 低 | 本 delta 基于 `openspec/specs/` 当前基线逐字拷贝、仅追加指针行；归档时人工核对合并 |
| R7 真机竞态不可稳定复现 | 修复效果难在 CI 验证 | 中 | 单测层以注入失败序稳定复现降级契约（AC-1–AC-4）；AC-10 真机走查留痕兜底 |

---

## 待决问题（design 定稿）

- 内联「刷新失败」提示的呈现位置（header 区 / 图面横幅）与文案、data-testid 命名。
- 重试调度落位：`useChangeDetail` 内 ref 计时器 vs 拆独立 scheduler hook；退避常量命名与单点位置。
- 保 last-good 期间 `loading` 面表现（是否呈轻量加载指示，默认不闪烁整页 loading）。
- `StoreFault` → String 映射形态（命令层单点 `map_err` 格式：是否含 fault kind 前缀）。
- store 读错根因：层 2 观测面跑出错误串后另立 change 钉死（explore 遗留开放问题 1）。
