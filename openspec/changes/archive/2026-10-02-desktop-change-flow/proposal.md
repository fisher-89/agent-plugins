# 提案: desktop-change-flow

> **变更**: desktop-change-flow
> **日期**: 2026-10-01（回溯重收敛：实现中途架构转向，见「过程 - 决策」翻转行）
> **状态**: draft

---

## 问题

dev-team 插件的 change 流程（PGE 相位工作流）目前只能在 Claude Code 会话内由 skill 编排驱动：subagent 步骤（executor / evaluator）依赖插件 agent 定义，编排真相藏在 skill 会话里，workflow.json 的操作权也在 agent 手中。用户无法在 desktop 内发起并观测一次 change 的全流程执行——`packages/desktop` 已有 change 列表 / 详情 / 流程图（派生视图）与会话域 agent 内核（`compose_turn` 新会话、转录可观测、可停止），但两者之间没有编排桥梁。

**战略前提（用户拍板，2026-10-01 实现中途转向）**：插件将逐步下线，desktop walker 是插件 change 流程的承接者——承接者不应依赖被承接物的 CLI。core/workflow 已有 workflow.json 读面，本变更在此基础上**补全写面**，使 core/workflow 成为 workflow.json 的 Rust 单一权威（读 + 写）。原「Rust 直写否决」理由（两份实现永久同步漂移）在「TS 版冻结待下线、Rust 版是唯一未来」前提下失效——这是迁移，不是双实现。插件**零新增**：本次实现中途追加的 walker-* CLI 子命令面全部回退，插件现有 MCP / hooks 冻结，后续独立下线。

把 change 流程搬进 desktop 有三处与插件原形态的结构性差异：

1. **编排宿主变更**：原 skill 会话编排改为桌面薄图运行时；executor / evaluator 不再是 subagent，而是桌面发起的独立 agent 会话（转录可观测、可停止）。
2. **file_log 归账链路断**：`record-files` hook 依赖会话内 MCP `phase_next`/`phase_start` 调用事件建立 `session_id → change` 绑定；phase-start 移到桌面后 executor 会话永无绑定，编辑不入 file_log。**处置拍板：不修补、绕开**——desktop run 不记录 file_log，prompt 组装降级为 **git diff** 提供变更文件上下文；file_log 的既有用途（让 agent 识别变更文件）由 git diff 等价承接。
3. **evaluator verdict 回传协议变**：原 evaluator 直调 MCP `phase_log`；桌面模式下若沿用，agent 仍持有 workflow.json 操作权，与「操作权收归桌面（用户拍板）」的目标冲突。

另有两条原插件机制在桌面模式静默失效，需显式承接：static-check 门禁挂在 SubagentStop 上（桌面无 subagent 即永不触发）；sweep-phase 中断标定挂在 UserPromptSubmit 绑定上（桌面未绑定 no-op）。

同时桌面侧 `buildFlowGraph` 的边由执行史时间序推导、不含路由语义，路由真相原唯一存在于插件 `phase_next`——本变更把相位路由状态机移植进 core/workflow（桌面路径自持权威），但「图不持有转移规则」红线保持：walker 每步过渡仍问 core/workflow 的 `phase_next`，不自持规则。

---

## 提案

在 `packages/desktop` 后端新增**薄图 walker 编排运行时**，整合 change 流程。原则：**deterministic core, LLM at the edges**——常态下没有编排 agent 在场；图骨架确定性（桌面 walker），智能只注入 WorkerAgent 节点。

### 写面落点：core/workflow 成为 workflow.json 单一权威（读 + 写）

- **core/workflow**（sync、无 tokio）= workflow.json 域权威：既有读面 + 新增写面（`phase_next` 路由状态机 / `phase_start` / `phase_log` / `backtrack`）+ requirement 相位表（含 executor / evaluator prompt 模板）单源收敛 + workflow.json 持久化。schema 形状原样不动（零新字段）。
- 写面为命令层可达的独立能力（不经 walker run 可直调），为后续 UI「手动 backtrack / 重试」留口。
- **已知缺陷顺手修**：backtrack 后 `phase_log` 丢重评条目（插件侧已知 bug）在 Rust 写面修复；插件侧冻结不修（长期无两套共存）。
- **static-check 执行（spawn）留 infra 层**——「spawn 不进 core」红线保持。
- **core/orchestration**（async、tokio）= 运行时域：walker 循环、port 缝、agent 会话、control / state（IPC 类型）、prompt 组装与解析器。依赖方向 orchestration → workflow（snapshot.rs 先例）。
- **不合并的理由**（crate 处置拍板 = 方案 A：不合并、职责重划）：写面 = 相位状态机与 workflow.json schema 血亲，schema + 门控同屋檐才是单一权威（TS 侧 `phase-next.ts` 住插件同理）；合并会把 tokio 拽进纯读叶库，读路径消费方陪跑运行时重编译；拆开放否则 workflow.json 知识跨两 crate（最差形态）。合并重估信号：写面仅剩 walker 一个消费者且 orchestration 只剩 walker 一个模块——当前看不到。

### V1 执行引擎与边界

- 执行引擎恒为 **claude code CLI 引擎**（会话执行面不变：用户级安装、插件 dev-team 天然加载，protect-files hook 护栏过渡期成立）；SDK 引擎不承载 change 执行。
- **插件零新增**：不迁移也不增补 CLI / MCP 工具；既有 MCP 工具与四挂点 hooks 冻结（不迁移、不改造、不关闭），随插件独立下线。walker 对 workflow.json 的全部状态变更走 core/workflow 写面进程内直调，**零 CLI 子进程写通道**。
- V1 范围：requirement 工作流全链；bug-fix / test-only 相位表后续独立 change。

### 每相位循环（walker 硬编码走这一张图）

```
① core/workflow phase-next（进程内）→ next_phase + 已插值 executor/evaluator prompt
   + allowed_backtrack_phases 白名单
② core/workflow phase-start（开相位、attempt 计时）
③ spawn executor agent（compose_turn 新会话）
④ [implement/test-gen 后] static-check（infra spawn 步，补 SubagentStop 门禁；
   失败走定向反馈边注入同一 executor 会话，≤5 次）
⑤ spawn evaluator agent（新会话，输出 checklist JSON，不落账）
⑥ 解析 verdict → core/workflow phase-log（桌面代写）
⑦ core/workflow phase-next → pass 推进 / fail 重试(≤5) / 重试上限唤决策 agent 分叉
```

（原 ⑤「转录提取 Write/Edit 路径 → change-files 补录」随 file_log 出局整体删除。）

### 三类节点统一在 walker

- **WorkerAgent 节点**（executor / evaluator / decision）：compose_turn 开新会话；
- **ToolStep 节点**（phase-start / phase-log / backtrack 相位机步进程内直调写面 + static-check spawn 步落 infra）：无智能；
- **Gate 节点**（verdict 解析 / retry 计数 / 白名单校验）：纯 Rust 分支。

ToolStep 做成节点而非命令式内联，报酬是可观测性均一：static-check 挂了 = 图上红节点，与 executor 挂同等可见。

**红线：图不持有转移规则**（语义升级版）。路由权威随写面进 core/workflow 后与 walker 同进程，walker 每步过渡仍须问写面 `phase_next`；`PIPELINE_PHASES` 与 `detail.rs` 相位列保持纯布局身份，不上位为路由权威；TS 侧相位表冻结，不再是桌面依赖。

**执行图 = 展示图（同源）**：节点状态 = workflow.json 状态（active_phase / eval）× 按 provenance 反查 session 集的派生视图；不建 flow_runs 表，workflow.json schema 原样不动。UI 流程图升级为实时执行视图（节点亮状态，点击看会话转录）。

### 决策协议（决议权与 plugin/dev-team 先例一致）

插件已有同构先例（`agents/test-execution-evaluator.md`：无法判断时不调 phase_log，返回结构化诊断 + 建议 backtrack 选项，由主会话问用户）。桌面把该协议推广为统一决策协议：

```
决策 agent（phase-next 返回重试上限时唤起；retry 预算内不唤，walker 自走）
  输入（有界，来自 workflow.json / 写面响应）:
    - fail checklist（fail 项 + evidence）
    - allowed_backtrack_phases（phase-next 白名单）
    - 候选相位最近一次 eval report
  输出（封闭集）:
    { action: "backtrack", backtrack_to ∈ 白名单, reason ≤500 }  ← 自治，经写面 backtrack 执行
    { action: "retry" }
    { action: "stop", reason }
    { action: "ask", question, options[] }                        ← 无法裁决 → UI 中断提问
  兜底: 写面 backtrack 二次校验白名单——坏决议损坏不了状态
```

### 推进节奏与 hooks 处置

推进节奏：**phase 内自动**（executor → static-check → evaluator → verdict → 推进），**phase 间停等用户确认**（贴合 no-auto-archive 习惯，用户拍板点与 archive 边界对齐）。

hooks 不迁移、不改造、不关闭——随插件继续挂在桌面 spawn 的每个会话上（继承用户级配置），四个挂点的处置：

| Hook | 桌面模式下 | 职责归属 |
|---|---|---|
| protect-files | 原样完整工作（全局 glob + 项目级 write_protection，不依赖绑定；spawn cwd 正确即生效；walker 不在工具循环内，只能节点间设卡，此 hook 是唯一步前实时护栏） | 保留 hook 层（**过渡期**）；插件真下线前需独立 change 补桌面原生护栏（内核 permission 层方向） |
| record-files | 未绑定 → fail-open 静默 no-op（刻意设计） | **不再接管**：desktop run 不记录 file_log（决策拍板，git diff 供变更上下文），原方案 A 补录通道整体出局 |
| static-check | 永不触发（无 subagent） | infra spawn 步承接；原语义为 loop_limit=5 的带反馈修复循环，以**定向反馈边**等价承接（诊断反馈注入同一 executor 会话，≤5 次，超限升格相位 fail） |
| sweep-phase | 未绑定 → no-op | walker 重入自标定：`sessionAnchors` 随进程内写面复活，mid-phase interruption 分支可达，中断相位由 walker 重入检查标定（原「插件显式出口」缺口消失） |

### 已拍板

1. 决策 agent 决议权与 plugin/dev-team 一致：白名单内自主决策 backtrack；无法裁决时中断提问（ask 出口）；写面 backtrack 二次校验兜底。
2. workflow.json 结构保持不动，图状态纯派生，不建 flow_runs 表。
3. **（翻转）** workflow.json 写通道 = core/workflow Rust 写面进程内直调（CLI 子进程方案随「插件零新增」拍板整体回退）；写面以**对照功能验收**为准（不建差分 oracle），长期单实现，接受微小行为漂移。
4. **（翻转）** desktop run 不记录 file_log：prompt 组装以 git diff 提供变更文件上下文；walker-change-files / 转录路径提取 / 补录通道全部出局；视图对既有（skill 路径）file_log 数据的显示不动。
5. executor / evaluator prompt 模板单源入 core/workflow 相位表；orchestration prompt 层做运行时插值（git diff 上下文等）。
6. crate 处置 = 方案 A：core/workflow（域权威，sync）与 core/orchestration（运行时域，tokio）不合并，依赖方向 orchestration → workflow；infra/devteam crate 整体删除。
7. schema 权威移交（zod → serde）为过渡期既定事实：serde 写出 MUST 兼容插件 zod schema 读取；移交时点在 design 显式声明。
8. desktop 版本 0.3.15 → **0.4.0**（特性级 minor bump，实现已就位）；dev-team 插件**零 bump**（保持 2.10.44）。

---

## 能力

### 新增能力

- **desktop-workflow-write-face** — core/workflow 补全 workflow.json 写面（Rust 进程内唯一写通道）：`phase_next` 路由状态机（含 `sessionAnchors` 进程内锚点与重试上限判定）、`phase_start` / `phase_log` / `backtrack`（stale 标记 + 传播 + 白名单二次校验 + reason ≤500）、requirement 相位表与 executor / evaluator prompt 模板单源、workflow.json 持久化（schema 形状不变、serde 写出兼容插件 zod 读取）、backtrack 后 phase_log 丢重评条目缺陷顺手修复；写面独立可达（不经 walker 可直调）。
- **desktop-change-orchestration** — core/orchestration 薄图 walker 编排运行时：相位循环（进程内写面直调、零 CLI 子进程写通道）、三类节点（WorkerAgent / ToolStep / Gate）、会话 provenance 挂靠与节点状态派生、变更文件上下文 git diff、static-check 门禁（spawn 留 infra）、verdict 解析与 phase-log 代写、决策协议、运行控制命令面（发起 / 停止 / ask 应答 / phase 间确认 / 状态流）。

### 修改的能力

- **desktop-change-flow-view** — 流程图从「执行史派生视图」升级为「实时执行视图」：运行态节点呈现、节点 ↔ 会话转录联动、运行期间经运行状态流实时刷新（解除「仅显式刷新」在运行期间的适用）、运行控制入口（发起 / 停止 / phase 间确认 / ask 应答）。既有三分类节点模型、9 列布局、时间序边推导、抽屉交互与降级规则不变；ToolStep 节点集随写面转向更新（change-files 步出局）。

---

## 变更范围

### 实现文件

**desktop 后端 — 写面（新增）**

- `packages/desktop/src-tauri/crates/core/workflow/src/` 新增写面模块：`phase_next` 路由状态机（含 `sessionAnchors` 进程内锚点、重试上限判定、白名单与 prompt 插值）、`phase_start` / `phase_log` / `backtrack`（stale 标记 + 传播 + 白名单二次校验 + reason ≤500）、requirement 相位表与 prompt 模板单源、workflow.json 持久化（serde 写出）；模块划分与命名 design 定稿（保持 sync 无 tokio）

**desktop 后端 — 编排（语义换血，主体保留）**

- `packages/desktop/src-tauri/crates/core/orchestration/src/port.rs` — ToolStepPort 语义换血：「CLI 工具步」→「相位机 + 工具步」进程内缝；ToolCommand 封闭集收缩（ChangeFiles 出局）
- `packages/desktop/src-tauri/crates/core/orchestration/src/` — walker / control / state / prompt / verdict / decision 适配进程内写面；`transcript.rs` 砍半（`extract_write_paths` 出局，`final_assistant_text` 保留）；prompt 组装接入 git diff 变更文件上下文；`snapshot.rs` 缩水或并入新缝（去留 design 定稿）
- `packages/desktop/src-tauri/crates/infra/agent/src/worker.rs` — WorkerAgentPort 实现（既有工作树内文件继续）；static-check spawn 缝落点 design 定稿（spawn MUST NOT 进 core）

**desktop 后端 — 命令面与交付**

- `packages/desktop/src-tauri/src/commands/change_flow/` — 运行控制命令组适配（发起前置校验去除 CLI 可发现项）
- `packages/desktop/src-tauri/src/bindings/` — 经 export-bindings 再生成，非手改
- `packages/desktop/package.json` — version 0.3.15 → 0.4.0（已就位，保持；`tauri.conf.json` 经 `../package.json` 自动跟随，`src-tauri/Cargo.toml` 不随动）

**desktop 前端**

- `packages/desktop/src/views/changes/flow/` — 运行态节点与实时执行视图（`graph.ts` 转换层扩展节点运行态，时间序边推导规则不变）；run-control-panel / run-step-node / session-transcript-panel / run-state 等工作树内新增文件继续
- `packages/desktop/src/views/changes/change-detail-view.tsx` — 运行控制入口（发起 / 停止 / phase 间确认 / ask 应答卡片）
- `packages/desktop/src/views/changes/hooks/` — use-change-flow-run / use-session-transcript（既有工作树内文件继续）
- 会话转录联动复用既有会话基建（`src/lib/agent-transport.ts` / `src/hooks/use-agent-chat.ts` / `AgentTimeline`）

**dev-team 插件（零新增——回退本次工作树内未发布增量）**

- 删除 `plugins/dev-team/bin/src/commands/walker-*.ts` 七文件（六命令 + walker-io 及其测试）
- `plugins/dev-team/bin/src/cli.ts` / `cli.test.ts` — 回退 walker-* 注册
- `plugins/dev-team/package.json` — version 2.10.45 回退为 2.10.44
- `dist/claude-plugins/dev-team/` / `dist/cursor-plugins/dev-team/` / `dist/cursor-home-image/dev-team/` — 回退后 rebuild 刷新（CLAUDE.md 项目规则）

### 测试文件

- core/workflow 写面测试：相位状态机路由（pass 推进 / fail 重试 / 上限分叉）、backtrack（stale 标记 + 传播 + 白名单越权拒绝 + reason 超长拒绝）、phase_log（**backtrack 后重评条目齐全回归**）、workflow.json serde 持久化往返（与插件 zod 兼容 fixture 对照）
- orchestration walker 测试：假引擎（预录 AgentEvent 序列）+ 假写面（fake port）驱动相位循环 / pass 推进 / fail 重试 / backtrack 分叉 / ask 中断 / 停止收敛
- verdict 与决策 JSON 解析器密集测试：合法 / 结构漂移 / 越权 backtrack / reason 超长
- 前端 flow 视图测试：运行态节点 / 转录联动 / 控制入口（data-testid 查询挂钩）；desktop-change-flow-view 既有场景回归（graph.ts 转换层直测保持绿）
- 随回退删除：CLI 假件（fake CLI 缝）、Write/Edit 路径提取器测试、插件 walker-* 命令测试

### 删除文件

- `packages/desktop/src-tauri/crates/infra/devteam/` — 整体删除（discover / runner 及测试）
- `packages/desktop/src-tauri/crates/core/orchestration/src/views.rs` + `views_test.rs` — CLI JSON 封闭视图删除
- `packages/desktop/src-tauri/crates/core/orchestration/src/transcript.rs` 的 `extract_write_paths` 半边
- `plugins/dev-team/bin/src/commands/walker-*.ts` 七文件（含测试）
- `snapshot.rs` 视 design 定稿缩水或并入（可能在删除之列）

### 不要修改

- `plugins/dev-team/bin/src/lib/workflow.ts` 相位表与 `commands/phase-next.ts` 路由语义（TS 侧冻结待下线；Rust 写面为移植非双修）
- 插件既有 MCP 工具注册面与四挂点 hooks（`protect-files` / `record-files` / `static-check` / `sweep-phase`）：冻结，不迁移、不改造、不关闭
- `workflow.json` schema 形状（零新字段；schema 权威移交仅指校验/写出实现易手，不改结构）与 `openspec/specs/**` 既有能力基线
- `crates/core/agent` 内核契约（AgentEvent 词汇、AgentRunner trait 面、纯度原则）与 `crates/infra/store` schema（不建 flow_runs 表、不加新列）
- `crates/core/workflow` 既有读面契约（`ChangeDetail` DTO、`PIPELINE_PHASES` 布局身份、desktop-change-queries 只读解析）
- golden wire contract（`tests/golden`）

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | walker 相位循环 | 以假引擎 + 假写面（fake port）驱动一次 requirement 工作流 run：walker 依写面 phase-next 返回走完 ①→⑦ 全循环（executor → static-check → evaluator → verdict → phase-log → phase-next），pass 相位自动推进、fail 重试 ≤5（与插件 `MAX_RETRY_TIMES` 一致），workflow.json 的 active_phase / eval 演进与插件直跑形态对照一致 |
| AC-2 | 决策协议 | 重试上限时决策 agent 被唤起且输入有界（fail checklist + 白名单 + 候选 eval report）；白名单内 backtrack 被执行、越权 backtrack 被写面二次校验拒绝；`ask` 动作令运行中断并在 UI 呈现问题与选项；retry 预算内不唤决策 agent（walker 自走） |
| AC-3 | 变更文件上下文（重写：file_log 出局） | desktop run 全程不产生 file_log 条目（workflow.json `file_log` 零新增）；executor / evaluator prompt 组装含 git diff 变更文件上下文；既有 skill 路径 file_log 数据的流程图挂载与图外面板显示不动 |
| AC-4 | static-check 门禁 | implement / test-gen 相位 executor 收口后 static-check spawn 步必经执行；失败诊断注入同一 executor 会话修复（≤5 次），超限升格相位 fail（代写 fail checklist 落账后进重试 / 决策分叉）；static-check 节点状态上图可观测 |
| AC-5 | 执行图 = 展示图 | 运行中流程图节点实时呈现运行态（WorkerAgent / ToolStep / Gate 各自可辨），点击 WorkerAgent 节点打开对应会话转录（运行中实时流、结束后重放一致）；运行结束后图与既有派生规则回归一致（desktop-change-flow-view 既有场景全绿） |
| AC-6 | 写通道唯一（重写：Rust 写面） | walker 全程零 CLI 子进程写 workflow.json 的代码路径，所有状态变更经 core/workflow 写面进程内直调；`infra/devteam` crate 不存在；orchestration → workflow 依赖方向成立；static-check spawn 为唯一进程 spawn 且落 infra 层（spawn 不进 core） |
| AC-7 | 会话挂靠与续走 | executor / evaluator / decision 会话 provenance 为 `source="change"`、`source_ref=<change>/<phase>/<role>/<attempt>`；按 provenance 反查会话集可派生节点状态；停止 / 桌面重启后重新发起 run 自 active_phase 续走，不重头执行已 pass 相位 |
| AC-8 | 管线合规 | `pnpm -C packages/desktop run client:check`、`pnpm -C packages/desktop run test`、rust 套件全绿；knip 无新增豁免条目；desktop 版本 0.4.0（`src-tauri/Cargo.toml` 不随动）；dev-team 插件零 bump（保持 2.10.44） |
| AC-9 | 写面等价性与缺陷修复（新增） | core/workflow 写面以对照功能验收：`phase_next` 路由 / 重试上限 / 白名单下发、`phase_start` attempt 计时、`phase_log` 落账、`backtrack` stale 标记 + 传播，逐项对照插件直跑行为语义验收（不建差分 oracle）；backtrack 回跳后同相位重评条目在 `phase_log` 全部落账（插件已知丢条目缺陷在写面不复发）；serde 写出的 workflow.json 可被插件 zod schema 读取 |
| AC-10 | 插件零新增回退（新增） | `walker-*` 七文件、`cli.ts` walker 注册、`package.json` 2.10.45 bump 全部回退（插件源码与已发布 2.10.44 形态一致）；三类 dist 交付产物（`claude-plugins/` / `cursor-plugins/` / `cursor-home-image/`）回退后 rebuild 刷新，产物中无 walker-* 痕迹 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| evaluator / 决策 agent 两处 JSON 结构化输出漂移 | verdict 误判或运行卡死 | 中 | prompt 约定封闭 schema + 解析失败即停给用户（不臆测 verdict、不静默）；解析器密集测试覆盖漂移形态 |
| Rust 写面与插件 TS 实现行为漂移（无差分 oracle） | 路由 / 落账语义与插件直跑不一致 | 中 | 对照功能验收逐项锁语义；AC-9 缺陷回归项；长期单实现接受微小漂移，但 schema 兼容 MUST 保持（serde 写出经插件 zod fixture 对照） |
| 双写并存窗口：过渡期 skill 路径（MCP）与 Rust 写面写同一 workflow.json | 一方写出另一方读不懂 | 低 | schema 形状不变为硬约束（零新字段）；serde 写出兼容 fixture 测试；schema 权威移交时点 design 显式声明 |
| static-check 定向反馈边与相位 retry 预算边界混淆 | 修复循环无限占用或提前烧尽预算 | 中 | 反馈边独立计数（≤5，沿 loop_limit 语义），超限升格相位 fail 才消耗 retry 预算；测试锁边界 |
| walker 自驱会话与前端 invoke 会话并存（StopRegistry / Channel 并发） | 停止寻址错乱或事件串流 | 低 | walker 会话与调试页会话同源内核；run_id 粒度与停止寻址键匹配关系 design 定稿并测试 |
| protect-files 实时护栏依赖插件 hook（过渡期） | 插件下线后桌面无步前护栏 | 确定（远期） | 过渡期成立且无动作；插件真下线前以独立 change 补桌面原生护栏（内核 permission 层方向），本期仅留痕 |
| 单 change 体量过大（写面 + 编排 + 视图 + 回退） | 实现与评审超载 | 中 | V1 只 requirement 工作流；walker 只硬编码走这一张图，不做通用图引擎 / 图定义文件格式（CLAUDE.md：不为单次使用造抽象） |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| workflow.json 写通道 | **（翻转）** core/workflow Rust 写面进程内直调 | 插件将逐步下线，承接者不依赖被承接物 CLI；core/workflow 已有读面，补全写面即单一权威；sessionAnchors 进程内复活，mid-phase interruption 可达 | CLI 子进程复用既有命令（插件零新增拍板下整体回退，否决） |
| CLI / MCP 工具迁移 | **（翻转）** 写语义移植 core/workflow，插件零新增 | walker-* 六命令为未发布工作树增量，全回退零遗留；插件现有 MCP / hooks 冻结待独立下线 | 为 walker 增补薄包装子命令面（已实现后回退，否决） |
| crate 处置 | 方案 A：不合并、职责重划（core/workflow 域权威 sync；core/orchestration 运行时域 tokio） | 写面与 schema 血亲同屋檐；合并把 tokio 拽进纯读叶库；拆开否则 workflow.json 知识跨两 crate | 合并单 crate（tokio 污染读路径，否决） |
| file_log 归账 | **（翻转）** desktop run 不记录 file_log；prompt 以 git diff 供变更上下文 | file_log 用途 = 让 agent 识别变更文件，git diff 等价承接；补录通道（方案 A/B 之争）整体失去必要性 | 转录提取 + change-files append（方案 A，已实现后出局）/ executor 保留只读 phase_next 刷绑定（方案 B，agent 沾状态面，否决） |
| phase_log 已知缺陷（backtrack 后丢重评条目） | Rust 写面顺手修复；插件侧冻结不修 | 长期无两套共存，修在唯一未来实现上 | 双侧同修（插件冻结原则冲突，否决） |
| 等价性验收口径 | 对照功能验收，不建差分 oracle | 长期单实现，差分 oracle 维护成本高于收益 | 差分 oracle 双跑比对（维护成本高，否决） |
| 推进节奏 | phase 内自动、phase 间停等确认 | 贴合 no-auto-archive 习惯；用户拍板点与 archive 边界对齐 | 全自动开关（V1 不做） |
| 决策权 | 白名单内自治 backtrack + ask 升级 + 写面二次校验 | 与插件 `test-execution-evaluator` 决议先例同构；坏决议损坏不了状态 | 全程问用户（打断常态）/ 全自治（越权风险） |
| 图存储 | 纯派生，不建 flow_runs 表 | workflow.json 唯一权威；无状态双写漂移 | flow_runs 表（双写漂移，否决） |
| ToolStep 形态 | 做成图节点而非命令式内联 | 可观测性均一（挂了 = 红节点） | 内联命令（失败不可见，否决） |
| 执行引擎 | claude code CLI 引擎 | 用户级安装即插件 dev-team 天然加载（protect-files 护栏过渡期可用）；与既有 CLI 引擎租户复用 | SDK 引擎（无插件加载机制，hooks 语义缺席，否决） |
| prompt 模板归属 | 单源入 core/workflow 相位表；orchestration 层运行时插值 | 相位表单源收敛（与插件二源合一）；git diff 上下文属运行时插值 | 模板留 orchestration（相位知识与运行时混居，否决） |
| static-check 承接 | 定向反馈边（同会话注入诊断，≤5 次）+ spawn 留 infra | 忠实承接原 SubagentStop loop_limit=5 修复循环语义；「spawn 不进 core」红线保持 | 计入相位 retry 预算（语义漂移，否决） |
| 路由权威 | 写面 phase-next 为唯一路由权威，walker 每步过渡问写面 | 红线语义升级：权威与 walker 同进程仍不自持规则；`PIPELINE_PHASES` 保持布局身份 | 桌面复制路由表（双权威漂移，否决） |
| desktop 版本 | 0.3.15 → 0.4.0（minor） | 特性级用户可见变更（run 编排 + 实时执行视图）；实现已就位 | 0.3.16（patch 不足以表达特性级，否决） |

### 待决问题

- core/workflow 写面模块划分与命名（phase 状态机 / 相位表 / 持久化的文件切分）；`snapshot.rs` 缩水 vs 并入新缝的去留（design 定稿）
- static-check spawn 缝形态：独立 port trait vs 复用 worker 缝扩展（design 定稿；spawn MUST NOT 进 core 为硬约束）
- schema 权威移交（zod → serde）的显式声明时点与兼容 fixture 形态（design 定稿）
- `source_ref=<change>/<phase>/<role>/<attempt>` 编码扩展的下游兼容最终确认（现消费方仅 explore 链的十进制串主键，`agentSessions` 按 source/sourceRef 过滤已就绪）
- run_id 粒度（每 run 发起一个）与 StopRegistry 停止寻址键的匹配形态（design 定稿）
- bug-fix / test-only 相位表的写面移植与编排支持（后续独立 change）

---
