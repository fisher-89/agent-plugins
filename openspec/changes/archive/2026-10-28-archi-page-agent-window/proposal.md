# 提案: archi-page-agent-window

> **变更**: archi-page-agent-window
> **日期**: 2026-10-27
> **状态**: draft
> **类型**: requirement
> **来源**: `openspec/changes/archi-page-agent-window/explore.md`（两轮；用户已裁定写权 = B、知识 = 静态骨架 + 代码同源派生、三条用途、面板抽骨架本次即换 change chat、手动刷新必需、清单命令对称）
> **范围接缝**: 代码扫描 / 漂移判定 / 门禁 / 报告归 `archi-check-and-gate`（另一条线，未落地）；本变更只做「架构页的会话窗口 + 工具面 + 知识注入」，MUST NOT 产出漂移判定与门禁。
> **前置依赖**: 无阻塞项。注入装配单点 `assemble_injections` 已有 explore 臂与 change 臂两支先例；`SessionInjections.preamble` 字段全链路已透传（只差 sdk 消费）；`SessionToolset::Archi { module, search }` 与两 archi 只读工具已在线；`foundation::layout` 的架构域派生族与 `archi::dto` 的 `ElementKind::ALL` / `GateTier::ALL` 可直接派生词表；`archi-refresh`（`key` 重挂）刷新链路现成；change chat 面板与 `AgentSessionView` 注入形态可作骨架原料。

---

## 问题

### 现象一：架构页零会话入口，且既有注入面结构上排除它

`/architecture` 当前只有「标题 + 刷新 + 画布」。想「让 agent 帮我补一条模型 / 解释这个域怎么拆」，唯一的门路是去 change 详情左栏开会话——而那会话的 `cwd` 是某个 change 的 worktree，谈的是「这个 change 怎么改」，不是「这个 workspace 的架构长什么样」。

更硬的约束在装配层：`crates/infra/agent/src/compose.rs` 的 `assemble_injections` 只认三支——`explore` 来源（六工具 + `Explore`）、`change` 来源（按 `source_ref` **尾三段** `{phase}/{role}/{attempt}` 查 7 行矩阵）、其余恒 `SessionInjections::default()`。架构页现在落在第三支，**结构上拿不到任何 `archi_*` 工具**。所以「注入架构工具」不是加个开关，而是必须新开一个来源词并加一条装配臂。

### 现象二：「维护架构」要写，而两条既有 requirement 把写权钉死在 change 里

| spec | 现有约束（逐字要点） | 与「架构页会话」的冲突 |
|---|---|---|
| `desktop-archi-canvas`「能力差异」 | `full` 模式「MUST NOT 呈现任何可提交的 `gate` 编辑入口」，提示「改档需要开一个 change」 | 若把写入口放进画布，唯一一条「全量只读」要求当场破 |
| `desktop-archi-agent`「模型 = 源码」 | 「冷启动同样走 change」+「MUST NOT 存在 workspace 级直接改模型的通路」+「三类使用场景同源（全走 change dev-design）」 | 架构页会话按定义就是 workspace 级直改主仓 |

用户已裁定走 **B（有写权、直改主仓）**。于是本变更必须给出唯一能同时成立的措辞：**画布仍然只读（约束的是人类可提交入口），写权只落在会话里**。

### 现象三：知识没有通道可走

`desktop-agent-execution`「SDK 引擎系统提示词（AGENT.md 注入）」把系统提示词**唯一来源**钉成 workspace root 的 `AGENT.md`，并两次强调 `SessionInjections.preamble` **维持不消费**。相位 prompt 是 16 份 `include_str!` 静态 md，但挂在相位 step 上，非相位会话无入口。`config.json` 的 `context` / `rules` 已解析已出线但无人消费，且语义泛（"全局规则"）。

结果是：架构页会话即使拿到 `archi_*` 工具，模型**不知道怎么组织模型文件**——键形约定、`metadata` 必须内联、层级由 FQN 前缀推导、新元素 `gate` 缺省 `off`、不得写接口符号——这些都不在代码里、也不在任何会到达模型的文本里。

### 现象四：自检与可见性两头都缺

- **agent 侧**：写入路径「不校验」（写坏模型 = Rust 编译不过的同构），唯一的坏模型信号是读取面 `archi_module` 返回 `Err`（带 `file:line`）。没人告诉模型「改完要复查」，它就不会复查。
- **用户侧**：画布零 watch 零轮询，agent 写盘后**画布不会自己变**。若不显式说明，用户会把「点了没反应」当 bug 报；反过来，这个「手动刷新」正是自检失败时的天然兜底（刷新即暴露 `StructureErrorPanel` 的 `file:line` 清单，与 agent 看到的是同一份 `archi::dto` 诊断）。

### 现象五：面板会变成第二套实现

change 详情左栏的 chat 面板（`views/changes/agent/change-chat-panel.tsx`：头区「新会话 / 历史 ▾」+ 状态行 + 历史清单分页 + `AgentSessionView` 注入形态 + 折叠窄条）是会话管理骨架的唯一样本。架构页若照抄一份，等于在 `desktop-change-chat` 的「对话体单一实现」纪律边上再开一条裂缝；而 explore 详情左栏的面板**没有会话管理维度**（一条记录幂等绑一个会话），不能作为骨架的检验样本。

### 症结：来源归属必须新开一个词，且不能让相位解析吃到它

`source_ref` 的定式是**自由字符串**，change 流程用四段 `{cid}/{phase}/{role}/{attempt}`、change chat 用两段 `<change id>`，两者靠「尾三段解析失败即 fail-closed」区分。若架构页复用 `source = "change"` 并以某种 `source_ref` 骗过矩阵，等于把「畸形 fail-closed」这条护栏弄钝。所以：**新来源词 `archi` + `source_ref = null`**，装配以 `source` 判定、不碰相位解析。

---

## 提案

在架构页加一个**通用 agent 会话窗口**，其工具面与知识面由图之外的装配层供给：

```
/architecture（挂载点 A，画布仍只读）
┌──────────────────────────────────────────────────────────────────┐
│ 左栏 SessionPanel（可折叠，默认收起）        │ 右：标题 + 刷新 + 画布 │
│  source = "archi" / source_ref = null      │  key={refreshTick}    │
│  cwd = null → 后端回落 root（主仓）          │  零 watch 零轮询       │
└───────────────────┬────────────────────────┴───────────────────────┘
                    │ agent_start(source="archi", ...)
                    ▼
   compose::assemble_injections（新增 archi 臂，与 explore 臂同形）
     ├─ toolset = Archi { module: true, search: true }    ← 两 archi 只读工具
     ├─ tools   = None                                    ← 七通用工具全量（含 write / edit / bash）
     └─ preamble = Some(archi_knowledge())                ← 知识：静态骨架 + 代码派生词表
                    ▼
   sdk/loop.rs：单条 System 消息 = preamble（产品知识，在前） + AGENT.md（逐字，在后）
```

### 1. 会话归属与注入臂（新来源词 `archi`）

| 位 | 值 | 理由 |
|---|---|---|
| `source` | `"archi"` | 新来源词，装配以它判定，MUST NOT 走 `source_ref` 相位解析 |
| `source_ref` | `null` | 一个 workspace 一份历史；不挂 change / explore 身份 |
| `cwd` | `null` | 后端回落 workspace root（主仓）——「观察根 = 当前工作根」口径与画布同源 |
| `tools` | `None` | 七通用工具全量（含 `write` / `edit` / `bash`）——写权在会话里 |
| `toolset` | `Archi { module: true, search: true }` | 两 archi 只读工具按位下发（词汇零变化，不加第三位） |
| `preamble` | `Some(知识文本)` | 首激活 preamble 位（见 §2） |

`explore` 臂与 `change` 臂逐字不动：矩阵仍只对 `source = "change"` 生效，archi 臂排在 change 臂判定之前、由 `source` 短路命中。

### 2. 知识注入：静态骨架 + 代码同源派生

```
compose_turn
  └─ archi 臂 → archi_knowledge::knowledge() -> Option<String>   纯函数，可单测
       ├─ include_str!("archi-knowledge.md")         静态骨架（纪律 / 键形 / 边界 / 自检 / 写面）
       ├─ ElementKind::ALL            -> kind 四值           （已有常量）
       ├─ GateTier::ALL / default()   -> gate 三值 + 缺省 off （已有常量）
       ├─ DiagnosticKind::ALL         -> 诊断九类词           （本变更补 ALL）
       └─ foundation::layout 相对路径 -> <archi_dir> / models / decisions（本变更补相对形态）
```

三条纪律：

1. **词表只从代码派生**：知识文本里的 `kind` / `gate` / 诊断词 / 目录路径一律由常量与派生函数拼装，产品源码 MUST NOT 手抄一份词表（漂移即编译期或测试红）。
2. **静态骨架必须自证可解析**：知识文本内嵌一段示例 TOML，测试用 `archi` 的解析面解析该片段并要求**零结构诊断**——格式（字段名 / 内联 `metadata` / 键形）一变，测试立刻红。这比「导出一份私有字段名清单」更省且更硬。
3. **尽力增强、不炸 run**：拼装失败（派生异常 / 读失败）降级为 `preamble = None`，MUST NOT 让发起点 `Err`。

首版知识骨架的**必静态**部分（代码里没有单点，只能写文字）：

- 一元素一表头，键 = 带引号的完整 FQN 单键（`[element."App.Auth"]`），禁点分嵌套写法；
- `metadata` 用内联表（`metadata = { owner = "..." }`），禁 `[element."X".metadata]` 子表（子表会被读成元素 `X.metadata`，合法 TOML 但语义错）；
- 层级由 FQN 最长前缀推导（`App.Auth.Token ⊂ App.Auth`），子元素须先声明祖先；
- 新元素 `gate` 缺省 `off`（ratchet 逐块收紧）；
- 模型里不写接口 / 导出符号（契约权威在 `openspec/specs`，只经 `docs` 引用）；
- 改完立即复查（`archi_module()` 无参 → 坏则返回诊断清单，好则薄概览）；只改架构域模型与 `docs` 关联文档，改源码须用户明确要求；
- 直改主仓（无 worktree / 无归档确认），改动随 `git diff` 评审；写盘后画布不自动更新，请用户点刷新；
- 续接历史会话时先查当下模型再回答「现在是什么样」（工具永远新鲜，assistant 记忆会旧）。

### 3. 写权落点：两条通路，画布仍只读

```
通路 A（既有）  change worktree ──▶ 归档人工确认 ──▶ 主仓        确认在回合末
通路 B（新增）  架构页通用会话 ──▶ 直接改主仓 ──▶ git diff 评审   确认在事后 diff
```

- **不设专用写工具**（保留）：仍用 `write` / `edit`，模型文件与源码同权；因此「该给哪个相位发写工具」这个问题继续不存在。
- **写入不校验、读取报错**（保留）：写盘不新增校验插口；坏模型表现为读取面 `Err` + 画布停摆。
- **`write_protection` 零改动**：`<archi_dir>/models/` 本就不在内置锁面（既有裁定），B 通路正好依赖这一点；用户显式 glob 仍照常生效。规范内容零变化，仅 rationale 的「唯一通路」措辞随本变更更新。
- **画布零放宽**：`full` 模式仍无任何可提交 `gate` 入口；本变更只在 spec 里补一句消歧——只读的是**人类入口**，agent 写面不在画布能力内，两者不互证。

### 4. 自检：路径 a（纯 prompt），不新增工具

| 路径 | 判定 |
|---|---|
| **a. prompt 要求改完调 `archi_module()` 复查** | ✅ 采纳。零工具变更，与「写入不校验、读取报错」完全同源；失败兜底 = 用户点刷新看 `StructureErrorPanel`（同一份 `archi::dto` 诊断），安全性不依赖模型自觉 |
| b. 新增 `archi_check` 工具 | ❌ 本期不做。`SessionToolset::Archi` 是两位布尔（封闭集），加第三位须同改 core 词汇 + infra definitions + 全量对拍语料；等「agent 老是不自检」的实测证据 |
| c. 写盘后引擎自动校验并回灌 | ❌ 不做。会在 loop 里加第三个校验插口，破坏「校验只发生在读取 / 门禁 / 归档」三处的既有纪律 |

### 5. 前端：抽面板骨架，change chat 本次即换，explore 不动

```
components/agent/session-panel.tsx（新，骨架 = 唯一实现）
  入参：state（会话面 / 清单面 / 动作面 / note）、title、testIdPrefix、
        composer（render prop）、collapsed? / onToggleCollapsed?
  渲染：collapsed ? 窄条（竖向标题 + 展开入口）
                  : 头区动作（新会话 + 历史 ▾） → 状态行（note） → 历史清单（分页）
                    → AgentSessionView（注入形态）
消费者：change 详情左栏（testIdPrefix="change-chat"）、架构页左栏（"archi-chat"）
不消费者：explore 详情左栏（无会话管理维度，硬套只会带进恒空清单 / 恒禁新会话的哑参数）
```

- **业务判据不抽**：`canSend` / `disabledReason` / `runActive` / `archived` 留各自 hook；骨架只认 `SessionPanelState`（公共子集），MUST NOT 自判禁发。
- **状态行只认 `note`**：change 侧 `disabledReason ?? （空态 ? 草案文案 : null）`；archi 侧 `disabledReason ?? （空态 ? 草案文案 : 「模型改动不会自动刷新，改完点「刷新」」）`。
- **架构页布局**：左栏 `ResizablePanelGroup` 分栏、**默认收起**（画布是横向内容，要能全宽查看），页面头增面板显隐开关；折叠真相由页面开关 state 持有（不依赖 `onResize` 像素派生，与 change 详情的差异留痕于 spec）。
- **刷新链路零新代码**：`archi-refresh` 按钮 → `refreshTick` → 画布 `key` 重挂 → `useArchiGraph` 重新 invoke。会话收口自动刷新**不做**。

### 6. 命令面：与 change chat 对称的薄包装

`list_archi_chat_sessions(root, offset, limit) -> SessionPage`——`source` 词常量收在命令层（前端零来源词汇），blank root → 空页信封。前端 `use-archi-chat` 与 `use-change-chat` 因此结构同形（清单取数 + 新会话 + 历史切换 + 判据 + note）。

### 不做清单（与范围裁决一致）

- **选中元素即上下文**：不动 `ArchitectureCanvas` 的 props 面（本轮不做）。
- **代码偏差判定 / 演进预修改**：归 `archi-check-and-gate`；本变更不给 agent 专用漂移工具，也不写 spec 义务（仍可用 `read` / `grep` 自行抽查）。
- **`archi_check` 工具 / diff 工具**：见 §4。
- **调试页来源筛选扩 `archi` 值**：`desktop-agent-execution`「调试页历史来源筛选」是封闭三值，扩值属另一条改动；架构页自带清单已覆盖发现性。
- **会话收口自动刷新画布**：不做（手动刷新是必需项，自动刷新属可选增强）。
- **仓内领域文档通道（`<archi_dir>/AGENTS.md`）与 `config.json` 的 `context` / `rules` 通道**：不引入第二知识源；冷启动无内容且会与静态骨架分叉。
- **explore 面板换骨架**：不同类（无会话管理维度）。
- **架构页会话的自动提交 / 自动归档**：无。

---

## 能力

### 新增的能力

- **desktop-archi-chat** — 架构页通用 agent 窗口：① 左栏面板（骨架复用 + 默认收起 + 页面头开关 + 窄条展开）与画布并排；② 会话归属 `source = "archi"` / `source_ref = null` / `cwd = null`（回落 root）与 `list_archi_chat_sessions` 薄命令、空态 / 新会话 / 历史清单语义；③ 禁发矩阵（仅本会话 `running`；架构页无归档态、无 flow run 判据）与状态行（草案文案 vs 刷新提醒）；④ 写权落点（会话可直改主仓模型、画布仍零写入口、写后必须手动刷新、零自动提交、档位沿用会话选择器缺省 `bypassPermissions`）。

### 修改的能力

- **desktop-archi-agent** — **MODIFIED**「SessionToolset::Archi 与 (phase, role) 注入矩阵」：新增 `source = "archi"` 装配臂（`tools = None` 七工具全量 + `Archi { module: true, search: true }` + `preamble = Some(知识)`，以 `source` 判定、不碰相位解析），矩阵仍只对 change 来源生效、explore / change / debug 臂逐字不变。**MODIFIED**「模型 = 源码（写纪律与不设专用写工具）」：由单一通路改写为**两条通路**（change worktree + 归档确认 / 架构页通用会话直改主仓 + `git diff` 评审），「不设专用写工具」「写入不校验读取报错」「`write_protection` 不纳入」三条保留，删除「冷启动同样走 change」「MUST NOT 存在 workspace 级直接改模型通路」「三类场景同源」。**ADDED**「架构会话的系统知识注入」：静态骨架 + `archi::dto` / `foundation::layout` 同源派生、示例片段可解析、拼装失败降级 `None`、单条 System 消息合并。
- **desktop-agent-execution** — **MODIFIED**「SDK 引擎系统提示词（AGENT.md 注入）」：`SessionInjections.preamble` 由「维持不消费」改为按来源装配（archi 臂）并**与 `AGENT.md` 合并为单条 System 消息**（产品知识在前、`AGENT.md` 逐字在后；仅一者在场时该条逐字）；每轮重读、无兜底回退、`AGENT.md` 32KB 口径不变。**MODIFIED**「会话级工具面注入与 explore 笔记工具」：compose 装配由两臂扩为三臂（新增 archi 臂），`preamble` 位措辞同步；Explore / change 两臂零回归。
- **desktop-archi-model** — **MODIFIED**「消费面词汇集一致性对拍」：消费面由三面扩为四面（新增**架构会话知识文本**），并钉死「封闭词汇各带 `ALL` 全量常量（本变更补 `DiagnosticKind::ALL`）、知识文本只从常量派生、MUST NOT 手抄词表」；对拍本体 `crates/core/archi/tests/vocabulary_test.rs` 随动扩展（既有三面 → 四面，既有断言保留）。
- **desktop-archi-canvas** — **MODIFIED**「能力差异（全量只读 / 增量可改档）」：保留 `full` 零写入口裁定，补一句消歧——只读约束的是**画布的人类可提交入口**；架构页 agent 会话的写面不在画布能力内，两者不互证（MUST NOT 以「会话能写」给画布加写入口，也 MUST NOT 以「画布只读」禁会话写）。
- **desktop-change-chat** — **ADDED**「面板骨架归一（共享 `SessionPanel`）」：change chat 改为骨架消费者（testid 前缀参数化、行为零变化），骨架为 change 与 archi 两处唯一实现，业务判据与 `note` 留各自 hook，折叠入参可选（explore 不消费）；Module Contract 补 `SessionPanel` 与 `useChangeChat` 的 `note` 位。
- **desktop-write-protection** — **MODIFIED**「架构域路径不在内置锁面」：规范内容（`<archi_dir>/models/**` 与架构域配置 MUST NOT 进内置锁面、用户显式 glob 仍生效）零变化，仅把 rationale 中「模型只能在 change 的 worktree 内改（唯一通路）」更新为两条通路，并补一条「架构页会话以主仓根为 cwd 直写模型时同样零命中」的 scenario。
- **workspace-layout-resolution** — **ADDED**「架构域 repo 相对路径派生（会话知识用）」：`archi_rel_dir()` / `archi_models_rel_dir()` / `archi_decisions_rel_dir()` 三函数（`/` 分隔、零 IO、全从目录名常量组派生），唯一消费者为架构会话知识文本；既有四函数与 `Layout` 三字段零变化。

### 零改动能力与理由

- **desktop-agent-chat-infra**：`useAgentSession` / `AgentSessionView` / `agent-transport` / `agent-adapter` 零改动——archi 会话走**注入形态**（来源侧自建 hook），`source` 参数本就是自由字符串；骨架组件是「面板」层能力，其 requirement 归 `desktop-change-chat`（共用）与 `desktop-archi-chat`（新）。
- **desktop-page-routing**：`/architecture` 路由与侧栏「架构」入口已存在（`nav-architecture`），本变更只在页内加分栏，零新增路由。
- **desktop-agent-management**：全局 agent 管理命令面 / 默认 agent 与运行发起解析 / `api_key` 机密边界全部零变化——新来源词只影响注入装配，不触 agent 解析与实例选择；`SessionToolset` / `SessionInjections` 的字段与变体也零变化（`Archi` 变体与 `preamble` 位均已在案，core 只补一个 `ALL` 常量），相关措辞随 `desktop-agent-execution` 的两条 MODIFIED 更新。
- **desktop-phase-prompts**：16 份相位角色 md 零改动——知识通道走 `preamble`（会话级前导），MUST NOT 塞进相位 prompt（相位 prompt 挂在 step 上，非相位会话无入口）。`phase_table_test` 的「16 条 `include_str!` 逐字对拍」与「非命中 md 零 archi 字样」两条断言因本变更**不变**（新 md 住 `infra/agent`，不在 `workflow/src/prompts/`）。
- **desktop-ipc-type-bindings**：「生成物入库与一致性守卫」已覆盖任意新命令（`bindings:check` 重导出后零 diff 属既有门禁），新增一条命令不需要新条款。
- **desktop-workspace-store / desktop-change-queries / desktop-change-state-store**：会话归属是 `agent_session` 既有两列（`source` / `source_ref`），无新表、无新字段、无迁移；架构页不新增模型字段。
- **desktop-workspace-config**：工作区 `config.json` 的 `KNOWN_KEYS` 与 `config.schema.json` 镜像零变化；本变更**不**消费 `context` / `rules`（拒绝第二知识源）。
- **desktop-file-watch / desktop-crate-layout / desktop-app-shell**：零 watch 红线被遵守而非改动；无新 crate、无新依赖边（`agent-runtime → archi` / `→ foundation` 已在案）；架构页会话走既有 `agent_start` 命令，命令轨枚举不增类。
- **desktop-checks-domain / desktop-test-execution / desktop-mutation-testing / desktop-release-pipeline / desktop-corpus-regression**：spawn 面、门禁面、发版面、语料面全不触碰。
- **repo-surface-scope**：新增能力 `desktop-archi-chat` 有可指认实现（`views/architecture/agent/**` + 骨架组件 + 一条命令）；不退役任何资产、不留空壳。

---

## 变更范围

### 实现文件

路径口径（两表一致）：`crates/**` 简写相对 `packages/desktop/src-tauri/`，其余一律写 `packages/desktop/**` 全路径。

| 文件 | 改动 |
|---|---|
| `crates/infra/agent/src/compose.rs` | 增 `ARCHI_SOURCE` 常量与 archi 臂（`preamble = Some(knowledge())`、`tools = None`、`toolset = Archi { module: true, search: true }`）；臂序：explore → archi → change → default；explore / change 两臂逐字不变；模块注释同步 |
| `crates/infra/agent/src/archi_knowledge.rs`（新） | `pub(crate) fn knowledge() -> Option<String>`：`include_str!("archi-knowledge.md")` 骨架 + `ElementKind::ALL` / `GateTier::ALL` / `DiagnosticKind::ALL` 词表 + `foundation::layout` 相对路径派生；纯函数（零 IO），拼装失败返回 `None` |
| `crates/infra/agent/src/archi-knowledge.md`（新） | 静态骨架文字（键形 / 字段 / 内联 `metadata` / 层级推导 / `gate` 缺省 / 边界 / 自检 / 写面 / 历史会话先取现状）+ 一段**可解析**的示例 TOML；`<>` 类占位符 MUST NOT 出现（目录路径由派生拼装注入） |
| `crates/infra/agent/src/lib.rs` | `mod archi_knowledge;` 与测试模块挂载（`#[cfg(test)] mod archi_knowledge_test;`） |
| `crates/infra/agent/src/sdk/loop.rs` | 系统消息合成单点：`injections.preamble` 与 `preamble::load(cwd)` 合并为**一条** `Message::system`（知识在前、`AGENT.md` 逐字在后，空行分隔）；仅一者在场即用其一；两者皆无 → 不注入 |
| `crates/core/archi/src/dto.rs` | `ArchiDiagnosticKind::ALL: [Self; 9]`（与 `as_str` 同源；既有 `ElementKind::ALL` / `GateTier::ALL` 复用） |
| `crates/core/foundation/src/layout/mod.rs` | 相对路径派生族：`archi_rel_dir()` / `archi_models_rel_dir()` / `archi_decisions_rel_dir()`（`/` 分隔、零 IO、零行内字面量，全从既有目录名常量组派生） |
| `packages/desktop/src-tauri/src/commands/exec/mod.rs` | 新增 `list_archi_chat_sessions(root, offset, limit) -> SessionPage`（`ARCHI_CHAT_SOURCE` 常量收本层 + blank root 空页信封），与 `list_change_chat_sessions` 同式 |
| `packages/desktop/src/components/agent/session-panel.tsx`（新） | 面板骨架：`SessionPanelState` + `title` + `testIdPrefix` + `composer` render prop + 可选 `collapsed` / `onToggleCollapsed`；头区动作 / 状态行 / 历史清单（分页）/ `AgentSessionView` 注入 / 折叠窄条；不自判禁发、不建第二条会话流 |
| `packages/desktop/src/components/agent/index.tsx` | 桶导出 `SessionPanel` 与 `SessionPanelState` 类型 |
| `packages/desktop/src/views/changes/agent/change-chat-panel.tsx` | 改为骨架消费者（`testIdPrefix="change-chat"`，文案与 composer 插槽保留）；`change-chat-*` testid 集合逐字不变 |
| `packages/desktop/src/views/changes/agent/hooks/use-change-chat.ts` | 增 `note`（`disabledReason ?? （空态 ? 草案文案 : null）`）；判据 / 清单 / 会话面零改动 |
| `packages/desktop/src/views/architecture/architecture-view.tsx` | 改为左栏分栏（`ResizablePanelGroup` + `ResizablePanel collapsible`，默认收起）+ 右栏「标题 + 面板开关 + `archi-refresh` + 画布（`key={refreshTick}`）」；折叠真相由页面开关 state 持有 |
| `packages/desktop/src/views/architecture/agent/archi-chat-panel.tsx`（新） | 架构页面板薄封装：`SessionPanel`（`testIdPrefix="archi-chat"`）+ 归属参数（`source="archi"` / `sourceRef=null` / `cwd=null`）+ composer 插槽 |
| `packages/desktop/src/views/architecture/agent/archi-chat-composer.tsx`（新） | `AgentInput` 薄适配（`idPrefix="archi-chat"`、文案留在本文件，档位 / 发送 / 停止收口在组件族） |
| `packages/desktop/src/views/architecture/agent/hooks/use-archi-chat.ts`（新） | 会话面（`useAgentSession` 注入）+ 清单取数（`list_archi_chat_sessions` 分页、首发后 / 轮收口重取）+ 新会话 / 历史切换 + 运行中禁发判据 + `note` 两态 |
| `packages/desktop/src/types/generated/bindings.ts` | `bindings:export` 重导出（新增一条命令） |
| `packages/desktop/package.json` | `version` 0.6.0 → 0.6.1（`src-tauri/tauri.conf.json` 经 `../package.json` 跟随；`src-tauri/Cargo.toml` 版本行不随动） |

### 测试文件

| 用例 / 装置 | 位置 | 处置 |
|---|---|---|
| archi 臂装配：`source="archi"` → `preamble.is_some()` + `tools == None` + `toolset == Archi { module: true, search: true }`；`source_ref` 为 `null` / 非空 / 畸形四段一律同臂（不因 `source_ref` 变形 fail-closed 丢工具） | `crates/infra/agent/src/compose_test.rs` | 扩展 |
| 零回归：explore 臂（六工具 + `Explore` + `preamble == None`）、change 臂 7 行矩阵与矩阵外 fail-closed、debug 缺省面逐字不变 | `crates/infra/agent/src/compose_test.rs` | 扩展（既有断言不改） |
| 知识拼装：含 `kind` 四值 / `gate` 三值 + 缺省 `off` / 诊断九词 / 模型与决策目录相对路径；零 `<>` 占位符；体量 ≤ 4KB；拼装失败降级 `None` | `crates/infra/agent/src/archi_knowledge_test.rs`（新） | 新增 |
| 知识漂移守卫：内嵌示例 TOML 经 `archi` 解析面解析 → 零结构诊断（字段 / 内联 `metadata` / 键形任一改动即红） | `crates/infra/agent/src/archi_knowledge_test.rs`（新） | 新增 |
| 系统消息合成：preamble + `AGENT.md` → **一条** System（知识在前、`AGENT.md` 逐字在后）；仅 `AGENT.md` → 逐字（既有回归）；仅 preamble（无 `AGENT.md`）→ 单条；两者皆无 → 零 System | `crates/infra/agent/src/sdk/loop_test.rs` | 扩展 |
| 工具面：archi 臂 definitions = 七工具 + `archi_module` / `archi_search`，`RunStarted.tools` 逐名一致；成员校验 / 沙箱 / 写入保护对该面既有语义不变 | `crates/infra/agent/src/sdk/tools_test.rs` + `loop_test.rs` | 扩展 |
| `DiagnosticKind::ALL` 与 `as_str` 九值逐值对拍（新增 / 删除词汇未同步即红） | `crates/core/archi/src/dto_test.rs` | 扩展 |
| 四面词汇集对拍：既有三面（进程内诊断面 / DTO 投影面 / 枚举全值词表面）保留，**新增架构会话知识文本面**——同一份全词汇语料驱动四面，断言四面词汇集相等；知识文本缺任一 `kind` / `gate` / 结构诊断词（手抄绕过 `ALL`）即点名缺失红 | `crates/core/archi/tests/vocabulary_test.rs` | 扩展（既有用例语义不变，新增知识文本面） |
| 相对路径派生族：三函数前缀同源、`/` 分隔零反斜杠、与对应绝对路径尾部一致（`strip_prefix(root)`）；零 IO | `crates/core/foundation/src/layout/mod_test.rs` | 扩展 |
| 命令面：`list_archi_chat_sessions` blank root → 空页；只返回 `source="archi"` 且 `source_ref=null` 的会话；change chat / 流程会话不混入；分页与 `updated_at` 降序 | `packages/desktop/src-tauri/src/commands/exec/mod_test.rs` | 扩展 |
| 骨架行为：头区两动作（新会话禁用态随 `canStart`）/ 状态行 `note === null` 不占位 / 历史清单加载·错误·空态·分页 / 折叠窄条与展开 / testid 前缀参数化（两套前缀各命中一次） | `packages/desktop/src/components/agent/session-panel.test.tsx`（新） | 新增 |
| change chat 零回归：既有 675 行行为级用例原样通过（testid、文案、交互序列零变化；若需微调只允许改查询挂钩，MUST NOT 改断言语义） | `packages/desktop/src/views/changes/agent/change-chat-panel.test.tsx` | 原样（回归证据） |
| `useArchiChat`：归属参数（`source="archi"` / `sourceRef=null` / `cwd=null`）逐字到达 invoke；清单取数走 `list_archi_chat_sessions`；新会话回空态零落库；历史点选以 `sessionId` 直查；运行中禁发 + `note` 两态 | `packages/desktop/src/views/architecture/agent/hooks/use-archi-chat.test.ts`（新） | 新增 |
| 架构页：左栏默认收起 / 开关与窄条展开；`archi-chat-*` testid 在场；面板收起时零会话取数零 invoke；`archi-refresh` 重挂恰一次 `archi_graph`；`mode="full"` 零写入口（既有断言保留） | `packages/desktop/src/views/architecture/architecture-view.test.tsx` | 扩展（既有四条用例语义不变） |
| 壳层复核：`/architecture` 挂载的 invoke 面不新增会话取数（收起态），`app.test.tsx` 既有序列断言按 `filter` 口径复核 | `packages/desktop/src/app.test.tsx` | 复核 |

### 不要修改

- `openspec/specs/**` 主基线（落主基线只在归档 `SpecSync` 步）。
- `crates/core/workflow/**`：16 份相位角色 md、`phase_table.rs` 的 `include_str!` 字面路径与角色表、`phase_table_test.rs` 的逐字对拍**全部零改动**——知识走会话级 `preamble`，MUST NOT 进相位 prompt。
- `crates/core/agent/src/runner.rs`：`SessionInjections` / `SessionToolset` 词汇零变化（`preamble` 位与 `Archi` 变体均已在案）。
- `crates/infra/agent/src/sdk/policy.rs`：两 archi 工具已在只读面；写面 / 执行面清单零变化。
- `crates/infra/agent/src/sdk/protect/**`：内置锁面零变化（`<archi_dir>/models/` 本就不在锁面）。
- `crates/infra/agent/src/sdk/archi_tool.rs`：工具执行体零改动（root 已取会话 cwd，archi 会话 cwd = 主仓根即得正确观察根）。
- `crates/infra/store/**`：无新表、无新字段、无迁移。
- `crates/core/config/**` 与 `crates/core/config/config.schema.json`：`KNOWN_KEYS` 与 schema 镜像零变化（不消费 `context` / `rules`）。
- `crates/infra/watch/**`：零 watch 红线；画布与会话都不引入监听。
- `packages/desktop/src/hooks/use-agent-session.ts`、`src/lib/agent-transport.ts`、`src/lib/agent-adapter.ts`、`src/components/agent/agent-session-view.tsx`、`agent-input.tsx`：注入形态与会话基建零改动。
- `packages/desktop/src/views/explores/**`：explore 面板不换骨架。
- `packages/desktop/src/views/changes/change-detail-view.tsx`：左栏分栏既有形态（`onResize` 折叠真相）零改动。
- `packages/desktop/src/views/agent/**`：调试页来源筛选三值（`debug | change | all`）不扩。
- `packages/desktop/src/routes.tsx`、`src/components/app-sidebar.tsx`：路由与侧栏入口既存。
- `packages/desktop/src-tauri/tauri.conf.json`、`packages/desktop/src-tauri/Cargo.toml`（版本行）、`src-tauri/capabilities/default.json`、`.github/workflows/**`。
- `openspec/changes/archi-page-agent-window/explore.md`（只读）。

---

## 验收标准

| ID | 验收条件 | 验证方法 |
|----|---------|----------|
| AC-1 | 注入臂：`source = "archi"` 会话装配为 `preamble = Some(..)` + `tools = None`（七工具全量）+ `toolset = Archi { module: true, search: true }`；`source_ref` 为 null / 非空 / 畸形四段一律命中同一臂（不走相位解析、不 fail-closed 丢工具） | `compose_test` 逐形态断言 |
| AC-2 | 零回归：explore 臂（六工具 + `Explore` + `preamble = None`）、change 臂 7 行矩阵与矩阵外 fail-closed、debug 缺省七工具面逐字不变 | `compose_test` 既有断言不改且全绿 |
| AC-3 | 知识同源派生：知识文本含 `kind` 四值 / `gate` 三值 + 缺省 `off` / 诊断九词 / 模型与决策目录相对路径，全部由 `archi::dto` 的 `ALL` 常量与 `foundation::layout` 相对路径函数取得；新增一个诊断词而知识未同步 → 测试红 | `archi_knowledge_test` + `dto_test`（`ALL` 与 `as_str` 对拍）+ `vocabulary_test`（知识文本面纳入四面词汇集对拍） |
| AC-4 | 知识自证：内嵌示例 TOML 经 `archi` 解析零结构诊断；字段名 / 内联 `metadata` / 键形任一漂移即红；知识文本零 `<>` 占位符、体量 ≤ 4KB | `archi_knowledge_test` |
| AC-5 | 单条 System：preamble 与 `AGENT.md` 同场 → 恰一条 System 消息（知识在前、`AGENT.md` 逐字在后）；仅 `AGENT.md` → 逐字（既有语义）；仅 preamble → 单条；两者皆无 → 零 System；每轮重读语义不变 | `loop_test` 四形态用例 |
| AC-6 | 工具面与档位：archi 臂 definitions = 七工具 + 两 archi 工具且 `RunStarted.tools` 一致；`default` 档两 archi 工具放行、`write` / `edit` / `bash` 被拒；`acceptEdits` / `bypassPermissions` 档写面放行 | `tools_test` + `policy_test` + `loop_test` |
| AC-7 | 直改主仓通路成立：`source = "archi"`（`cwd = null` → root）会话以 `write` 新建 `<archi_dir>/models/09-x.toml`、以 `edit` 改既有 `gate` 行均成功，写入保护零命中；随后 `archi_module` 读到该改动 | 命令面 / loop 级用例 + `protect` 既有用例复核 |
| AC-8 | 写坏不 panic 且可自修：写坏模型后 `archi_module` 返回结构化诊断 `Err`（含 `file:line`）、run 不中断、无新增写前校验插口；用户点 `archi-refresh` 后画布呈 `StructureErrorPanel` 同源诊断 | `archi_tool_test` + `architecture-view.test` |
| AC-9 | 画布零放宽：`/architecture` 仍无任何可提交 `gate` 入口（`archi-gate-readonly` 在场、`archi-gate-select` / `archi-gate-submit` 缺席）；agent 写盘后画布不自动变化（假计时器推进零取数） | `architecture-view.test` 既有用例 |
| AC-10 | 骨架归一：change chat 经 `SessionPanel` 渲染，`change-chat-*` testid 集合与文案逐字不变，675 行行为级用例原样通过；骨架无业务判据（`canSend` / `disabledReason` / 归档 / run 期判定不在骨架内） | `change-chat-panel.test` + `session-panel.test` + 源码审查 |
| AC-11 | 架构页归属：面板会话以 `source = "archi"` / `source_ref = null` / `cwd = null` 发起，会话行落在 workspace root 所属库；`list_archi_chat_sessions` 只返回该归属，change chat（两段）与流程会话（四段）一条不混；blank root → 空页 | `use-archi-chat.test` + `commands/exec/mod_test` |
| AC-12 | 面板交互：空态可发（首发 `sessionId: null` → 后端铸行）；「新会话」回空态零落库；历史点选以 `sessionId` 直查重放；运行中禁发（含新会话）而历史浏览可用；状态行两态（草案文案 / 「模型改动不会自动刷新，改完点「刷新」」） | `use-archi-chat.test` + `architecture-view.test` |
| AC-13 | 布局：架构页左栏默认收起为窄条、页面头开关与窄条展开均可达；画布占右栏可全宽；`archi-refresh` 语义（`key` 重挂、恰一次 `archi_graph`）零变化；面板收起时零会话取数 | `architecture-view.test` |
| AC-14 | 路径字面量单点：产品 `.rs` 除 `foundation/src/layout/mod.rs` 外零 `openspec` / `architecture` 段 / `config.json` 字面量；知识文本的目录路径全经派生函数注入 | 既有命名隔离扫描 |
| AC-15 | 交付面：`bindings:check` 重导出后零 diff（`list_archi_chat_sessions` 在案）；`packages/desktop` version 0.6.1、`src-tauri/Cargo.toml` 版本行不随动；`vp check` / `knip` / `cargo clippy -D warnings` 通过 | 机械门面 |

---

## 风险

| ID | 风险 | 影响 | 概率 | 缓解措施 |
|----|------|------|------|----------|
| R-1 | **主仓直写无硬隔离**：archi 会话 `cwd` = 主仓根，工具面含 `write` / `edit` / `bash`，且 `AgentInput` 缺省档为 `bypassPermissions` ⇒ agent 可改任意文件（含源码），越界只靠 prompt 软约束 | 一次误改即污染工作区（虽可 `git diff` 回滚，但用户可能未提交） | 高（结构性） | spec 明写「只改架构域模型与 `docs` 关联文档，改源码须用户明确要求」为**软约束**、写入面无硬隔离属**已知缺口**（MUST NOT 被当成能力承诺）；不提供自动提交 / 自动归档；状态行提示刷新与 diff 评审；用户可切到 `default` 档（写面被 policy 拒）当只读顾问用。若实测越界频发，升级项 = 给 archi 臂加「写面白名单」（新增 spec 条款，不在本期） |
| R-2 | **知识漂移**：静态骨架与真实格式（字段名 / 内联 `metadata` / 键形）或词表漂移 | 模型产出的 TOML 读不进来，用户看到"agent 不会写" | 中 | 词表一律从 `archi::dto` 的 `ALL` 与 `foundation::layout` 相对路径函数派生（MUST NOT 手抄）；静态骨架内嵌示例经 `archi` 解析零诊断作为漂移守卫（AC-4）；知识体量 ≤ 4KB 保证不被截断 |
| R-3 | **`AGENT.md` 逐字语义被改写**：既有 requirement 两次强调「唯一来源 + 不包装不改写」，合并成单条 System 会触碰逐字断言 | 既有 `preamble` 相关测试或 spec 逐字措辞不一致 | 中 | 只在 `preamble` 在场时合并；`AGENT.md` 单独在场路径**逐字**与上线前一致（AC-5 单列该用例）；spec MODIFIED 时把「合并规则 + 顺序 + 分隔」写死，避免实现漂移 |
| R-4 | **合并体量**：`AGENT.md` 32KB 上限 + 知识文本 ⇒ 单条 System 变大 | 上下文窗压力、或知识被 `AGENT.md` 截断逻辑误伤 | 低 | 知识文本长度以测试锚定（≤ 4KB）；截断只作用于 `AGENT.md` 自身（既有 `preamble::load` 口径不变），知识不截；spec 明写两段各自口径 |
| R-5 | **骨架抽取的回归面**：`change-chat-panel.test.tsx` 675 行是行为级断言，骨架可能改变 DOM 次序 / 包装层 | 用例红被误当功能回归，或为迁就测试改动行为 | 中 | 骨架逐字保留既有 testid 与「头区 → 状态行 → 历史清单 → 会话体」次序；先抽骨架再跑既有用例，红则改骨架不改断言；文案全部留在两侧消费者文件 |
| R-6 | **架构页折叠真相**：change 详情用分栏 `onResize` 像素派生折叠态，jsdom 无布局引擎时不可靠 | 折叠 / 展开用例不稳定，或运行时窄条与面板态脱节 | 中 | 架构页折叠态由页面开关 state 持有（不依赖 `onResize`），与 change 详情的差异在 spec 留痕；面板收起时不发起任何会话取数（可测的断言点） |
| R-7 | **archi 来源在调试页不可筛**：`desktop-agent-execution`「调试页历史来源筛选」是封闭三值 | 架构页会话的发现性依赖架构页自带清单 | 低 | 架构页清单（`list_archi_chat_sessions`）即主入口；扩调试页筛选值属纯增量（另 change），spec 记为已知边界 |
| R-8 | **历史会话上下文过期**：模型文件是活的，重开会话时 assistant 记忆是旧的，可能凭记忆答「现在长啥样」而不调工具 | 用户被旧描述误导 | 中 | 知识文本明写「续接历史会话先取当下模型再回答」；工具调用永远实时读盘（架构侧无缓存）；不自动恢复最近一条，用户显式点选即已知是旧会话 |
| R-9 | **`DiagnosticKind::ALL` 与 `as_str` 双份清单**：新增常量可能漏值或漏词 | 知识文本少一个诊断词，静默不显 | 低 | 逐值对拍测试（`ALL` 逐项 `as_str` 断言，长度 9）；`desktop-archi-model` 的四面词汇对拍把知识文本纳入射程 |
| R-10 | **`preamble` 首激活的连带语义**：字段自「不消费」变为「按来源消费」，第三方可能据此推断"任意来源都能塞 preamble" | 未来误用成第二知识源 | 低 | spec 明写：`preamble` 只由装配层按来源决定（当前唯一生产者 = archi 臂），自由字符串 / 前端 MUST NOT 直接供 `preamble`；消费点单点（loop 系统消息合成） |
| R-11 | **壳层与绑定机械面**：新命令未重导出即前端编译失败；`app.test.tsx` 的 invoke 序列断言可能被架构页新增树形结构影响 | 交付门红 | 中 | `bindings:export` 后 `bindings:check` 零 diff 为 AC；架构页收起态零会话 invoke 使既有序列断言稳定，必要时按 `filter` 口径复核 |

### explore 未决项的本轮落法

| explore 开放项 | 本轮落法 |
|---|---|
| 知识拼装失败的降级形态 | `knowledge()` 返回 `Option<String>`，失败即 `None`（尽力增强，不炸 run） |
| 骨架入参最终命名 | spec 只钉「公共子集 + 不含业务判据」的语义契约，具体命名（`SessionPanelState` / `labels` / `testIdPrefix`）由 design 定型；本提案的表名仅为工作命名 |
| 架构页左栏尺寸 | design 定（沿 change 详情 ~30 / ~22 与 explore 55 / 25 之间取），spec 不锚定具体数值，只要求「默认收起 + 可展开 + 窄条留可见入口」 |
