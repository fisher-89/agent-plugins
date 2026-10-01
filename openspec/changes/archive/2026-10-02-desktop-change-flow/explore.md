# desktop 整合 dev-team change 流程（agent graph 编排）— 探索笔记

> 拟议 change 名：`desktop-change-flow`（本笔记 stem 与之对齐）
> 日期：2026-10-01

## 目标与边界

- 在 `packages/desktop` 内整合 `plugins/dev-team` 的 change 流程。
- V1 执行引擎：claude code CLI（用户级安装，plugin dev-team 天然加载：MCP + hooks 可用）。
- **先不迁移 CLI / MCP 工具**：dev-team CLI 子进程（`phase-next` / `phase-start` / `phase-log` / `backtrack` / `change-create` / `change-files` / `static-check` 等一次性命令）是 workflow.json 的唯一写通道——复用工具，只换调用方。
- 与插件原形态的区别：
  - 原 subagent 步骤（executor / evaluator）改为桌面发起的**独立 agent 会话**（compose_turn 新会话，转录可观测、可停止）；
  - 编排从 skill 会话改为桌面**薄图运行时**；
  - workflow.json 操作权从 agent 转移到 desktop（用户拍板）。

## 现状盘点（两边已有能力）

| 侧 | 已有 | 缺口 |
|---|---|---|
| plugins/dev-team | phase 表 + phase_next 路由纯函数；CLI 命令面（bin/src/commands/）；agents/*.md（executor/evaluator 定义）；hooks（protect-files 写保护 / record-files file_log 归账 / SubagentStop static-check / sweep-phase） | — |
| packages/desktop | core/workflow（workflow.json 只读解析 + artifacts + list/detail 查询）；core/agent（session-domain kernel：TurnRequest / SessionProvenance / StopRegistry）；infra/agent/cli（claude 发现 .cmd shim + flags `-p stream-json --resume` + jsonl）；views/changes（列表/详情/**流程图**，边从执行史时间序推导，非转移表）；views/agents（agent 实例 + compose_turn 装配） | **编排循环**（本 change 的主体）+ 三条断链修补 |

关键事实：`buildFlowGraph` 的边由 eval 序 + interrupted + activePhase 归并推导，边类型由列索引差派生——桌面**没有复制路由语义**，`PIPELINE_PHASES` / detail.rs 相位列只是布局常量。路由真相唯一存在于插件 `phase_next`。

## V1 架构：薄图运行时

原则：**deterministic core, LLM at the edges**——常态下没有编排 agent 在场；图骨架确定性（桌面 walker），智能只注入两种节点。

```
每相位循环（walker）：
  ① dev-team phase-next（CLI, JSON）→ next_phase + executor/evaluator prompt（已插值）
  ② dev-team phase-start（开相位、attempt 计时）
  ③ spawn executor agent（compose_turn 新会话，provenance source="change"）
  ④ [implement/test-gen 后] dev-team static-check（ToolStep 节点，补 SubagentStop 门禁）
  ⑤ 从密封转录提取 Write/Edit 路径 → dev-team change-files append（file_log 补录）
  ⑥ spawn evaluator agent（新会话，输出 checklist JSON，不调 phase_log）
  ⑦ 解析 verdict → dev-team phase-log（桌面代写）
  ⑧ dev-team phase-next → pass 推进 / fail 重试(≤5) / 决策分叉
```

三类节点统一在 walker：

- **WorkerAgent 节点**：executor / evaluator / decision —— compose_turn 开新会话；
- **ToolStep 节点**：phase-start / static-check / change-files / phase-log —— CLI 子进程，无智能；
- **Gate 节点**：verdict 解析 / retry 计数 —— 纯 Rust 分支。

ToolStep 做成节点（而非命令式内联）的报酬：可观测性均一（static-check 挂了 = 图上红节点，与 executor 挂同等可见）。

**红线：图不持有转移规则**。walker 每步过渡都问 phase-next；相位列常量保持纯布局身份，不上位为路由权威。否则插件 bump 相位结构时两边在状态门控语义上漂移。

**执行图 = 展示图（同源）**：节点状态为派生视图，UI 流程图升级为实时执行视图（节点亮状态，点击看会话转录）。

## 决策协议（已拍板 #1：与 plugin/dev-team 决议权一致）

插件已有同构先例：`agents/test-execution-evaluator.md` 约束原文——诊断"无法判断"时不调 phase_log，返回结构化诊断摘要 + 建议 backtrack 选项，由主会话问用户。桌面把该协议推广为统一决策协议：

```
决策 agent（fail 且需跳转时唤起；retry 预算内不唤，walker 自走）
  输入（有界，来自 workflow.json）:
    - fail checklist（fail 项 + evidence）
    - allowed_backtrack_phases（phase-next 白名单）
    - 候选相位最近一次 eval report
  输出（封闭集）:
    { action: "backtrack", backtrack_to ∈ 白名单, reason ≤500 }  ← 自治，直接执行
    { action: "retry" }
    { action: "stop", reason }
    { action: "ask", question, options[] }                        ← 无法裁决 → UI 中断提问
  兜底: backtrack 工具二次校验白名单——坏决议损坏不了状态
```

## 图存储（已拍板 #2：纯派生）

- workflow.json schema 原样不动（插件 schema 保持唯一权威，无桌面私有状态污染）；
- 不建 flow_runs 表；
- 节点状态 = workflow.json 状态（active_phase / eval）× 按 provenance 反查 session 集 的派生视图；
- session provenance：`source="change"`，`source_ref = <change>/<phase>/<role>/<attempt>`（现 source_ref 为探索记录主键十进制串，需确认扩展编码的消费方兼容性）。

## 三条断链与修补

1. **file_log 归账绑定断**：record-files 靠会话内 MCP `phase_next`/`phase_start` 调用事件建 `session_id → change` 绑定；phase-start 移到桌面后 executor 会话永无绑定 → 编辑不入 file_log → Gate 2 必挂。修补（倾向 A，未终确认）：**A.** 桌面从密封转录提取 executor 的 Write/Edit 路径，调 `change-files append`（官方 fallback 通道；归账权威源 = 桌面转录）；**B.** executor prompt 前导保留一次只读 phase_next 刷新绑定（零新代码但 agent 沾状态面）。
2. **static-check 门禁静默消失**：挂在 SubagentStop 上，桌面模式无 subagent 不触发 → 桌面补调 `dev-team static-check`（ToolStep 节点）。
3. **evaluator verdict 回传改协议**：不再自调 MCP phase_log，prompt 约定最终消息输出 checklist JSON，桌面解析后代调 phase-log。解析失败停给用户（可控优先）。

## 已拍板

1. 决策 agent 决议权与 plugin/dev-team 保持一致：白名单内自主决策 backtrack；无法裁决时中断提问（ask 出口）。
2. workflow.json 结构保持不动，图状态纯派生，不建新表。

## 倾向项（proposal 阶段收敛）

- workflow.json 写通道：CLI 子进程（推荐，零迁移）vs Rust 直写（core/workflow 需连 phase 门控逻辑一起搬，实质迁移）。
- file_log 归账：方案 A 转录提取 + append（推荐）vs 方案 B。
- 推进节奏：phase 内自动（executor→evaluator→verdict）、phase 间停等确认（贴合 no-auto-archive 习惯）vs 全自动开关。
- executor 角色注入：prompt 前导写角色要点 vs 指示会话 Read 插件 agents/<role>.md（依赖插件安装路径，略脆）。
- V1 范围：先 requirement 全链（bug-fix / test-only 相位表后续）。

## 风险与验证点

- evaluator / 决策 agent 两处 JSON 结构化输出漂移（失败即停给用户）。
- `sessionAnchors` 为进程内状态，CLI one-shot 每次新进程、锚点重置——mid-phase interruption 检测路径需实测；run_id 粒度 = 每驱动周期一个。
- provenance source_ref 编码扩展的下游兼容。
- memory 已知坑：phase 间隙派 fix agent 须先 phase_start 再派（否则编辑不入 file_log）——walker 边序即责任链，补录节点必须紧跟 executor 节点。
- 图运行时不做通用图引擎 / 图定义文件格式（V1 只硬编码 walk 这一张图；CLAUDE.md：不为单次使用造抽象）。

## 原有 hooks 的处置（矩阵）

hooks 不迁移、不改造、不关闭——随插件继续挂在桌面 spawn 的每个会话上（继承用户级配置），四个挂点三种命运：

| Hook | 挂点 | 绑定依赖 | 桌面模式下 | 职责归属 |
|---|---|---|---|---|
| protect-files | PreToolUse (Write\|Edit\|Bash\|PowerShell) | 无（全局 glob + 项目级 write_protection.files） | 原样完整工作 | 保留 hook 层——桌面无法替代的唯一步前实时护栏（walker 不在工具循环内，只能节点间设卡）；spawn cwd 正确即无条件生效；桌面 ToolStep CLI 子进程非 claude 会话不受其约束，插件工具合法写 workflow.json 无冲突 |
| record-files | PostToolUse | 有（session→change 注册表） | 未绑定 → fail-open 静默 no-op（刻意设计） | 桌面转录提取 + change-files append 接管（官方 fallback 通道） |
| static-check | SubagentStop | — | 永不触发（无 subagent） | ToolStep 节点补调；注意原语义是 loop_limit=5 的**带反馈修复循环**而非一次性门禁——V1 简化为 fail 路径需明确：定向反馈边（lint 错误重派 executor）vs 消耗 retry 预算 |
| sweep-phase | UserPromptSubmit | 有（lookupChange） | 未绑定 → no-op | walker 兜底：phase-next 自身处理 mid-phase interruption、流程图三分类收集直接吃 activePhase，不归档不损正确性；损失仅中断 attempt 计时历史与 interrupted 节点精度。缺口：interruptActivePhase 无独立 CLI 出口（hook 子命令且要绑定）——V1 接受，后续可给插件提显式出口 |

方案 A/B 在 hooks 维度的对称性：选 B（executor 保留一次只读 phase_next）则绑定链复活，record-files 自动归账、sweep-phase 自动工作；选 A 则 hooks 全退位、桌面单一权威源。

## 关键文件索引

- 路由权威：`plugins/dev-team/bin/src/commands/phase-next.ts`、`lib/workflow.ts`（相位表）
- 绑定机制：`plugins/dev-team/bin/src/lib/session-registry.ts`、`hooks/hooks.canonical.json`
- 决议权先例：`plugins/dev-team/agents/test-execution-evaluator.md`（Constraints）
- 桌面装配：`packages/desktop/src-tauri/crates/infra/agent/src/compose.rs`、`cli/{discover,flags}.rs`
- 派生图：`packages/desktop/src/views/changes/flow/graph.ts`、`core/workflow/src/queries/detail.rs`

---

## 转向拍板（2026-10-01，实现中途 — 推翻「CLI 子进程写通道」）

> 触发：test-gen 评估暴露 5 处 CLI 边界语义分歧后，用户提出「walker 能否不依赖插件 CLI」。
> 动机（用户原话归纳）：① 未来逐步下线插件，不想给插件追加功能；② core/workflow 已有读面，本次 change 在此基础上**补全写面**。
> 本节推翻上文「倾向项」#1（写通道选 CLI 子进程）与 #2（file_log 方案 A/B）——后续以本节为准。

### 方向：core/workflow 成为 workflow.json 的 Rust 单一权威（读 + 写）

- walker 是「插件下线后的承接者」；承接者不应依赖被承接物的 CLI。原「Rust 直写否决」理由（两份实现永久同步漂移）在「TS 版冻结待下线、Rust 版是唯一未来」前提下失效——这是迁移，不是双实现。
- 插件**零新增**：walker-* 六命令、walker-io.ts、CLI 注册、2.10.45 版本 bump 全部回退（均本次工作树内未发布，零遗留）；插件现有 MCP/hooks 冻结，后续独立下线。
- D2 port 分层红利：walker 循环/解析器/control/prompt 层与 orchestration 测试面（fake port）基本不动；`ToolStepPort` 实现从「infra/devteam 子进程」换成「core/workflow 进程内适配」。infra/devteam crate（discover/runner）删除。

### 四点拍板

1. **写面落点**：core/workflow 补全写面，walker 进程内直调，零子进程写通道（AC-6 语义改写：Rust 进程内唯一写面）。移植清单：phase_next 路由状态机（`sessionAnchors` 进程内复活，V1 边界 #4「mid-phase interruption 不可达」消失）、backtrack（stale 标记 + 传播 + 白名单 + reason ≤500）、phase_start / phase_log、相位表收敛单源（walker 现硬编码单图，与插件相位表二源合一）。static-check 执行（spawn）留 infra 层——「spawn 不进 core」红线保持。
2. **phase_log 已知 bug 顺手修**：backtrack 后 phase_log 丢重评条目（memory: project_bug_phase_log_skip）在 Rust 写面修复；插件侧冻结不修（长期无两套共存）。
3. **desktop run 不记录 file_log**：file_log 的用途是让 agent 识别变更文件；desktop 的 prompt 降级为 **git diff** 提供变更文件上下文。→ walker-change-files / recordFileOps 移植 / D6 补录通道 / transcript `extract_write_paths` 全部出局；AC-3（节点文件区挂靠 via 补录）随之重写或删除；视图对既有（skill 路径）file_log 数据的显示不动。
4. **等价性验收口径**：写面以**对照功能验收**为准（不建差分 oracle），长期单实现，接受微小漂移。

### 随迁影响与过渡风险

- 决策表翻转：「写通道 = CLI 子进程复用既有命令」→「core/workflow 写面」；「不迁移 CLI / MCP 工具」→「移植写语义到 core/workflow」。AC-6 重写；specs 两 capability 中 CLI 子进程字样的 scenario 改写；变更范围：walker-* 六命令出、core/workflow 写面入。
- 双写并存窗口：过渡期 Claude 会话插件 MCP（skill 路径）与 Rust 写面写同一批 workflow.json——schema 兼容为既定事实（skill 与 desktop 路径本就并存），但 schema 权威 zod → serde 的移交时点需在设计中显式声明。
- protect-files 实时护栏仍靠插件 hook（执行引擎 claude code CLI 加载用户级插件）——过渡期成立；插件真下线前需独立 change 补桌面原生护栏（内核 permission 层方向）。
- record-files hook 在 desktop run 下本就未绑定 no-op（见上文矩阵），决策 3 使 file_log 断链修补（方案 A/B 之争）整体失去必要性。
- 红线「图不持有转移规则」保持，语义升级：路由权威进 core/workflow 后与 walker 同进程，walker 每步过渡仍须问 core/workflow 的 phase_next，不自持规则。

### crate 处置拍板（方案 A：不合并、职责重划）

- **core/workflow = workflow.json 域权威**（sync、无 tokio）：读面（既有）+ 写面（新增：phase_next 相位状态机 / phase_start / phase_log / backtrack stale+传播）+ 相位表（含 executor/evaluator prompt 模板）单源收敛 + workflow.json 持久化。写面为命令层可达的独立能力（未来 UI「手动 backtrack/重试」按钮可直调，不必拉起 walker run）。
- **core/orchestration = 运行时域**（async、tokio）：walker 循环、port 缝、agent 会话、control/state（IPC 类型）、prompt 组装与解析器。依赖方向 orchestration → workflow（snapshot.rs 先例）。
- **orchestration 模块处置**：walker / port（语义换血：ToolStepPort 从「CLI 工具步」变「相位机+工具步」进程内缝）/ control / state / prompt / verdict / decision 保留；transcript 砍半（`extract_write_paths` 出局，`final_assistant_text` 保留）；views.rs（CLI JSON 封闭视图）删除；snapshot.rs 缩水或并入新缝；**infra/devteam crate（discover/runner）整体删除**。
- **不合并的理由**：写面=相位状态机与 workflow.json schema 血亲，schema+门控同屋檐才是单一权威（TS 侧 phase-next.ts 住插件同理）；合并会把 tokio 拽进纯读叶库，读路径消费方陪跑运行时重编译；拆开放会让 workflow.json 知识跨两 crate（读在 workflow、写在 orchestration，最差形态）。
- **合并信号（未来重估）**：若 workflow 相位机函数仅剩 walker 一个消费者、且 orchestration 只剩 walker 一个模块——届时边界只剩仪式感，合并更诚实。当前看不到该信号。
