# 提案: config-agent-session

> **变更**: config-agent-session
> **日期**: 2026-11-19
> **类型**: requirement
> **状态**: 待评估（explore 的设计形状与 24 条裁决已锁定；本提案落定 explore 未覆盖的三处 spec 改判面与一处存量漂移边界，见「决策表」D13–D16 与「风险」R6 / R8）
> **来源**: `explore.md`（切片 2；todo 原文「扩充 project 配置协议……配置页面支持 Agent 检测生成配置」的会话式写回部分）
> **切片关系**: 切片 1（`projects[].language` / `depends_on` + 拓扑序安装）已归档（`openspec/changes/archive/2026-11-19-project-config-protocol-extension`）。本变更只承载切片 2：`/config` 从「只读展示」变成「常驻 config 会话 + 候选提案 + 用户确认后写盘」。

---

## 问题

配置页（`/config`）今天是一条**单向只读链**：`workspace_config(root)` → `core/config::load` → 分区呈现，页面显式刷新模型，命令轨道零写命令（`desktop-workspace-config`「配置页呈现面」把「无编辑 / 保存 / 删除入口，全链无对 config.json 的写路径」写成了硬承诺）。用户要配一大批字段（coverage 三阈值、mutation、`tests[]` 多套、`projects[]` 含 `language` / `depends_on`、写保护 glob）只能手改 JSON，而**项目间依赖与框架识别恰恰是手改最不擅长的部分**：切片 1 刚落地的 `depends_on` 拓扑序安装没有任何检测入口，`projects[].language` 更是声明型字段（零工程消费者）——配置页把它们的值照原样呈现，却没有帮助用户把它们写出来。

三个具体障碍决定这条链不能靠「加一个确定性检测器」解决：

1. **检测器猜不准项目间依赖**：扫描 lockfile / 扩展名直方图 / 框架 registry 能给出语言与框架候选，但 `projects[i].depends_on`（谁依赖谁）是仓库意图，不是文件事实。选 agent 会话的代价要当场接受：**「检测出什么」不可断言**——测试只锚工具契约、写面行为与卡片形态，MUST NOT 断言「这个仓库应被识别为 2 个项目、依赖是 A ← B」。
2. **候选需要校验回环**：LLM 产出的 JSON 天然会带枚举外框架值、越界阈值、含 glob 的 `tests[].root`。把校验留到用户点「应用」时才发现，等于把失败推给最迟的时刻；必须有「脏候选当场退回自纠」的工具级回环。
3. **写回必须离开 agent 工具面**：`config.json` 是写入保护内置锁定的第一条（`infra/agent/src/sdk/protect/rules.rs::builtin_rules` 恒一条 `**/<配置名>`，不可配置、不可移除）——它自己是保护规则的权威，**agent 永远不能写它**。因此写回只能是「用户点击 → 应用进程自写」，不能是「模型调一个写工具」。

附带一个**已核实的落盘陷阱**：`WorkspaceConfig` 只实现 `Serialize` 且挂 `#[serde(rename_all = "camelCase")]`，wire face 与 file face 在四处不同名（`staticAnalysis` / `writeProtection` / `dependsOn` / `extra`），且 `extra` 是 `[{key,value}]` 包装。`serde_json::to_value(&cfg)` 一旦拿去落盘，重读时 `staticAnalysis` / `writeProtection` / `extra` 全落 passthrough（**写保护规则静默失效**）、`projects[i].dependsOn` 被元素级忽略（**拓扑序安装退化**），而现有自洽测试抓不到（`assert_self_consistent` 只断言「零诊断」）。写面必须是**独立的文件面写入器**。

## 提案

以「**一条新会话臂 + 一个自校验提案工具 + 一次用户触发的原子写回 + 一份新的文件面写入器**」收口切片 2：

1. **`source = "config"` 会话臂**（`compose::assemble_injections` 第四臂）：`tools = [read, grep, glob, ls, bash, propose_config]`——只读探测面 + 唯一提案出口，**无 `write` / `edit`、无 db 工具**；`preamble = 配置协议知识`（静态、零入参、零 IO）；`finish = None`（聊天面板不该有轮级终止工具）；`ask = Generic`。`source` 词常量沿既有做法在 compose / 命令层 / 前端 hook 三处同字面量。
2. **`propose_config` 工具**（`infra/agent/src/sdk/config_propose.rs`，挂载清单第 14 项，`ToolTier::Read`，`ToolExec::Config`）：`propose_config({})` = 只读模式，返回当前盘面（file face）+ 空候选；`propose_config({ candidate })` = 校验模式，零 `InvalidValue` 才 `Ok`，脏候选 `Err` 逐条诊断并附「修正后重交完整候选」。**不落盘、不落库、无副作用**——候选只活在会话转录里；输出恒为单条 file-face JSON 字符串，`changes` 由后端算（前端零配置语义）。
3. **`ConfigContext.root` = 主仓 root**（`PumpPayload.workspace_root`），**MUST NOT** 取 `ctx.workspace_root`（exec root / change worktree）：配置权威在仓根，archi 语义（「看我现在改的那棵树」）在此不适用。
4. **写面命令 `workspace_config_apply(root, candidateFileJson)`**：`check root → plan_apply_config`（`validate` 有任一 `InvalidValue` 即 `Err`，**零写入**）`→ to_file_value → to_string_pretty + 尾换行 → write_atomic`，返回新报告供页面直接替换。入参是 **file-face `serde_json::Value`**（TS `unknown`），MUST NOT 收 typed `WorkspaceConfig`；**双校验不是冗余**（propose 侧校验可能来自很久以前或历史会话重放，前端载荷可伪造）。
5. **文件面写入器 `to_file_value(config, template)`**（`core/config`，纯函数、零 IO）：**模板驱动**——按候选原文出现过的键裁剪，「写全字段」= 模板取全键集，不设第二形态、不设 flag；实现走结构变换（`to_value` → 三处 `rename_in_place`（递归全层级）→ `expand_extra` → 数值整数化 → 模板裁剪），让新增字段自动出线；四条机械锚 P1–P4 把「写入器 / `KNOWN_KEYS` / `config.schema.json` / 组装层」三面钉在同一事实上。
6. **前端装配**：`use-config-chat`（照 `use-archi-chat` 形态）+ 面板（复用骨架 `SessionPanel`，`testIdPrefix = "config-chat"`）+ `ConfigProposeCard`（盘面 vs 候选字段级 diff + 「应用」，注册进 `BUILD_IN_TOOL_CARDS`）+ 应用成功后 `useWorkspaceConfig().refresh()`。卡片**五态**：进行中 / 失败 / 可应用（`changes == []` 时禁用 +「与当前配置一致」）/ 只读模式（仅盘面）/ 内容不可解析（原文降级）。
7. **spec 改判与立规**：新建 `desktop-config-agent` 承载会话面板 / 归属 / 注入臂边界 / 工具契约 / 写面命令 / 写入器 / 提案卡；`desktop-workspace-config` 只改判「只读」条款与呈现面（并补一条「写面不算逐字段消费者」的纪律条款）；`desktop-agent-execution` 三臂转四臂（含两处「preamble 唯一生产者」表述）；`desktop-archi-agent` 收窄兜底枚举；`desktop-change-chat` 的「面板骨架两处唯一」改为三处。`desktop-write-protection` **零改动**（它的 scope 是「Agent 不得写」，apply 不经 agent 工具面）。

### 关键决策表

explore §7 的 C1–C24 逐条沿用，不再复述；下表只落 explore 未定 / 本提案新增的口径：

| 编号 | 决策 | 值 |
|---|---|---|
| D1 | 提案粒度 | 一次全量候选，MUST NOT 接受片段；缺省 `candidate` = 只读模式 |
| D2 | 写回入口唯一性 | 用户点「应用」；agent 零写权限（内置锁定零改动）；apply 不是工具、模型不可触发 |
| D3 | `propose_config` 输出形态 | 恒单条 file-face JSON 字符串；`current` / `candidate` / `changes` / `summary` 四键；`changes == []` 表示与盘面一致 |
| D4 | `current` 的取法 | 「盘面 file face」= `to_file_value(assemble(盘上值).config, 盘上值)` 投影；config.json 缺失即 `{}`；MUST NOT 用 `to_value`（wire face） |
| D5 | `ConfigContext.root` | 主仓 root（`payload.workspace_root`）；MUST NOT exec root / worktree |
| D6 | 写入器形态 | `to_file_value(config, template)` 模板驱动；不设「写全 / 写最小」flag |
| D7 | 数值形态 | `fract() == 0.0` 且绝对值 `<= 2^53` 出 `i64`，否则保浮点（否则用户写 `90` 会变 `90.0`） |
| D8 | 键序与格式 | 不开 `serde_json/preserve_order`；接受字母序 + pretty 重排，应用确认面须明示「写回会规范化文件（未知子键、键序与格式会丢失 / 变化）」 |
| D9 | `write_atomic` 落位 | 搬 `src/commands/atomic.rs`（`pub(crate)`）去重；`check_root` 不搬（archi 的两条 root 文案已分叉），config 轨道内抽私有 fn |
| D10 | 卡片失效语义 | 多次 propose 各一张卡、都可应用（最后点哪个算哪个）；不做「已被取代」标记；diff 基准 = payload `current`（MUST NOT 跨时刻比较页面 hook 数据） |
| D11 | 面板折叠缺省 | 沿骨架缺省（收起 + 窄条展开入口）；配置页 MUST NOT 为自身改骨架折叠契约（那是 `desktop-change-chat` 的骨架面） |
| D12 | 版本纪律 | `packages/desktop/package.json` patch +1（现值 `0.7.2` → `0.7.3`）；`src-tauri/Cargo.toml` 不跟 |
| D13 | 「两处骨架」表述 | `desktop-change-chat`「面板骨架归一」的「两处唯一 / 两套前缀」改判为**三处**（change 详情 / 架构页 / 配置页），前缀集合补 `config-chat` |
| D14 | 「preamble 唯一生产者」表述 | `desktop-agent-execution` 两处 + `desktop-archi-agent` 一处收窄为「生产者 = archi 臂与 config 臂」 |
| D15 | 架构模型资产 | 新增 import 边 `Views.Config → Components.Agent` 按 `repo-archi-model`「漂移纪律」**在同一变更内**声明（模型 toml + 表头 + ADR §9 复算表同步）；配置文件**不新建前端元素**（`views/config/` 内的新文件归既有 `App.Frontend.Views.Config`，与 `views/config/components/**` 现状同形） |
| D16 | 存量计数漂移 | `repo-archi-model` / `desktop-frontend-module-entry` 的计数复述（spec 文本写 53 元素 / 120 关系、`06` 段 30 / 71）与模型实读（**54 元素 / 122 关系**、`06` 段 31 / 73，多出 `App.Frontend.Components.Standard.Combobox` 元素）**已不一致**——这是先于本变更存在的漂移，**本提案不改写这些计数复述**，也不借 D15 的 +1 边顺手「修好」（见 R8） |

### 明确不做（知情边界，非遗漏）

- **确定性检测器**（扫 lockfile / 扩展名直方图 / 框架 registry）：零 token、可断言，但猜不准项目间依赖；退路保留——将来补检测器时把输出作为首轮 prompt 种子即可，注入臂不用改。现成积木（本期不接）：`commands/stats` 的 tokei 直方图、`infra/checks/src/testexec/detect.rs` 的 `REGISTRY`。
- **preamble 带盘面真值（A 方案）**：唯一开放项，指标 =「config 会话首轮是否明显多一轮工具往返」。形状已定（`ComposedTurn` 增 `workspace_root`、`assemble_injections(&provenance, root)`、4 个构造点 + 测试辅助一处），且须同时立「生产者每次装配读盘、MUST NOT 缓存」纪律；不预先做。
- **`context` / `rules` 的主动补全**：二者零工程消费（`desktop-workspace-config`「字段消费点表」在案的存量欠账）；preamble 说明其零消费，只在用户明确要求时写。
- **`SessionCtx.workspace_root` 的 doc 漂移修正**（`core/agent/src/runner.rs:114` 写「隐含当前 workspace root」，实为 exec root）：既存漂移，独立议题。
- **`source` 词常量收口**：既有 4 类来源已积 12 个散点，本期再加 3 处；收口另立议题。
- **`App.Backend.Core.Config` 的契约文档补全**：该元素 `status = "absent"` / 0 ops（`INDEX.toml`），新增公共函数不新增门禁（`apiUndeclared` 落 `off` 档）。
- **`depends_on` 影响面传播 / 按 `projects[].language` 分派 prompt 与检查**（切片 3）。
- **既存计数漂移修复**（D16 / R8）。
- **写保护语义任何放宽**（`desktop-write-protection` 零改动）。

## 能力

### 新增的能力

- **desktop-config-agent** — 承载配置页常驻 config 会话的完整能力边界，仿 `desktop-archi-chat` + `desktop-archi-agent` 二分但收敛为一份（config 侧体量不足以拆两份）：
  - 面板布局与骨架复用（左栏常驻、折叠自持、零取数纪律）；
  - 会话归属（`source = "config"` / `source_ref = null` / `cwd = null`）与专用清单薄命令 `list_config_chat_sessions`；
  - 禁发矩阵与状态行（仅本会话 `running`；无归档态、无 flow run 判据）；
  - `propose_config` 工具契约（两模式 / 校验回环 / file-face 输出 / `changes` / 无副作用 / 体量上限 / 挂载与档位 / 无路径沙箱豁免）；
  - `ConfigContext` 铸造与 **root = 主仓 root**；
  - 静态协议知识 preamble（骨架 + 代码派生词表 + 使用指引）；
  - 写面命令 `workspace_config_apply`（零 `InvalidValue` 门 / 失败零写入 / 原子写 / 返回新报告）与「agent 不可写、apply 不经工具面」边界；
  - 文件面写入器 `to_file_value`（模板驱动 / 整数化 / `extra` 三坑 / 四条机械锚 P1–P4）；
  - 提案卡五态与写回呈现纪律。

### 修改的能力

- **desktop-workspace-config** —
  - **MODIFIED**「配置页呈现面」：页面由「只读」改判为「常驻 config 会话 + 应用」——新增会话面板挂载（面板契约归 `desktop-config-agent`）、写回唯一入口 = 提案卡「应用」按钮、agent 零写权限、命令轨道唯一写命令 `workspace_config_apply`；新增「写回规范化明示」与「应用成功后刷新」两条呈现面要求；「只读边界」scenario 逐条改判（写回唯一入口 / agent 零写 / 无其它写命令 / 无第二落盘路径）。
  - **Purpose 段残留**：该能力 Purpose 现写作「`/config` 只读配置页的呈现面」——Purpose 不属 delta 表达面，归档合并时 SHALL 同步为「常驻 config 会话 + 应用」的呈现面措辞（登记于此，防归档后残留旧定性）。
  - **MODIFIED**「项目协议语义边界与消费点纪律」：补一条「**写面是全字段透传，不算逐字段消费者**」（唯一字段名清单 = `KNOWN_KEYS` + 三处重命名表；写回保真度由 `desktop-config-agent` 的 P1–P4 锚承载），字段消费点表增「写面（`plan_apply_config` / `to_file_value`）」一行，并补一条 scenario。
- **desktop-agent-execution** —
  - **MODIFIED**「SDK 引擎（rig 进程内租户）」：系统提示词条目「`SessionInjections.preamble` 位按来源装配（唯一生产者 = archi 臂）」→「（生产者 = archi 臂与 config 臂）」。
  - **MODIFIED**「SDK 引擎系统提示词（AGENT.md 注入）」：开篇「当前唯一生产者 = `source = "archi"` 的架构会话知识……其余来源恒 `None`」改为「生产者 = archi 臂与 config 臂……其余来源恒 `None`」，并补一条「两臂共用同一单条 System 合成」scenario；截断口径、单条 System 规则逐字不动。
  - **MODIFIED**「会话级工具面注入与 explore 笔记工具」：三臂 → **四臂**（④ `source == "config"`：六名名字清单 + `preamble = Some(配置协议知识)` + `ask = Generic` + `finish = None`）；专用工具名清单补 `propose_config`；执行体装配缝的 per-turn 铸造判据补 `ConfigContext`（名字判据 `propose_config` 在场、root 取 `payload.workspace_root`）；tier 段落补 `propose_config` 归只读面；补 config 臂 scenario。
- **desktop-archi-agent** —
  - **MODIFIED**「archi 工具面注入与 (phase, role) 矩阵」：兜底表述由「其余非 change 来源（debug 等）SHALL 恒七内建全量」收窄为「其余**非 change、非 config** 来源（debug 等）」；`preamble` 位的生产者由「唯一 archi 臂」改为「archi 臂与 config 臂」。
- **desktop-change-chat** —
  - **MODIFIED**「面板骨架归一（共享 SessionPanel）」：消费方由「本能力与架构页两处」扩为**三处**（+ 配置页常驻 config 会话），testid 前缀集合由两套补为三套（`change-chat` / `archi-chat` / `config-chat`），「两处共用同一实现」scenario 改判为「三处共用」。

其余能力零改动，理由：

- **desktop-write-protection**：scope 是「Agent 不得写」。`config.json` 内置锁定一条不变；apply 是应用进程自写、用户点击触发、不经 agent 工具面，故「agent 不可写」与「用户可写」不冲突。该能力零改动即本变更的正确结论，不是遗漏。
- **desktop-workspace-store**：`session.source` 是自由字符串（`agent_start` 的 `source.unwrap_or("debug")`），无枚举、无 CHECK；explore 特判不会误纳新来源 ⇒ 零迁移、零注册模型。
- **desktop-ipc-type-bindings**：生成机制零改；`workspace_config_apply` 与前端 `ProjectConfig` 等出线由 `bindings:export` 自动带出（含 `candidate` 出线为 `unknown`）。
- **desktop-crate-layout**：不新增 crate、不新增跨 crate 依赖边（`agent-runtime` 已依赖 `config`；`foundation::layout::config_path` 已是既有依赖）。`config.json` 字面量禁令照旧（写面经 `layout::config_path`）。
- **desktop-archi-agent / desktop-archi-api / desktop-archi-api-check**：`App.Backend.Core.Config` 仍是 `absent` / 0 ops，`api:check` 预期零 diff（工具面与命令面不是模块公共 API 契约面）。
- **desktop-page-routing**：`/config` 路由与 nav 入口既在，页面内部形态变化不进该能力。

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs` | `assemble_injections` 增第四臂 `config`（六名名字清单 + `preamble = config_knowledge::knowledge()` + `ask = Generic` + `finish = None`）+ `CONFIG_SOURCE` 常量；doc 三臂转四臂；签名不变 |
| `packages/desktop/src-tauri/crates/infra/agent/src/config_knowledge.rs` + `config-knowledge.md`（新） | 静态 preamble 生产者：`include_str!` 骨架 + 代码派生词表（`known_keys()` 八键、`TestFramework::ALL` 八值）+ 使用指引；签名无入参、零 IO；拼装失败只丢 preamble 位 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/config_propose.rs`（新）+ `sdk/mod.rs` | `propose_config` 定义常量、两模式执行体、`changes` 计算、`Err` 诊断文案；输出 file-face JSON 字符串 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs` | `PROPOSE_CONFIG_TOOL` 常量 + 第 14 项挂载（`ToolTier::Read`、`ToolExec::Config`）；`with_capacity(13)` → 14 与两段 doc 随动 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools.rs` | `input_path` 无路径白名单补 `propose_config`（否则候选顶层出现未知键 `path` 会被 `sandbox_rewrite` 当工具路径改写） |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs` + `sdk/loop.rs` | `ConfigContext { root }` + `mint_config_context(names, payload.workspace_root)`；`LoopTurn.config` 字段与 `ToolExec::Config` 分派臂 |
| `packages/desktop/src-tauri/crates/core/config/src/lib.rs` | 新增公共面：`known_keys()`、`TestFramework::ALL` + `as_str()`（与 `parse_framework` 双向互证）、`validate(&Value) -> ConfigReport`、`to_file_value(&WorkspaceConfig, &Value) -> Value`、`plan_apply_config(&Path, &Value) -> Result<ConfigWrite, String>`；补模块 doc（既有 `assemble` doc 引用了不存在的「模块 doc null 位语义」） |
| `packages/desktop/src-tauri/src/commands/config/mod.rs` | `workspace_config_apply` + `*_inner`（file-face `serde_json::Value` 入参）+ 私有 root 校验 fn（不搬 `check_root`） |
| `packages/desktop/src-tauri/src/commands/atomic.rs`（新）+ `commands/archi/mod.rs` | `write_atomic` 搬移（`pub(crate)`）去重；`archi_set_gate` 改引用（`archi/mod_test.rs` 的「`fs::write` 恰一处」断言随落点更新） |
| `packages/desktop/src-tauri/src/commands/exec/mod.rs` + `commands/mod.rs` | `list_config_chat_sessions(root, offset, limit)`（照 `list_archi_chat_sessions`，`source` 常量收命令层、blank root → 空页）；`all_commands!` 登记 `workspace_config_apply` / `list_config_chat_sessions` |
| `packages/desktop/src/views/config/hooks/use-config-chat.ts`（新） | 会话状态面 + 清单分页 + 新会话 / 历史切换 + root 复位 + 禁发矩阵 + `note`（照 `use-archi-chat.ts`） |
| `packages/desktop/src/views/config/components/config-chat-panel.tsx`（新） | 面板装配：`SessionPanel`（`testIdPrefix = "config-chat"`）+ `AgentSessionView` 注入形态 + composer 插槽 |
| `packages/desktop/src/components/agent/message-cards/tool-cards.tsx`（扩） | `ConfigProposeCard` + `BUILD_IN_TOOL_CARDS` 注册（键 `propose_config`）：五态、字段级 diff、「应用」按钮、写回规范化明示 |
| `packages/desktop/src/views/config/config-view.tsx` | 挂载会话面板（横向分栏：左 = 会话，右 = 既有分区 + 显式刷新），页面从「只读」变为「会话 + 应用」；`useWorkspaceConfig().refresh()` 接应用成功回调 |
| `packages/desktop/src/types/generated/*` | `bindings:export` 再生成（`workspaceConfigApply` 的 `candidate` 出线 `unknown`；`listConfigChatSessions`）——MUST NOT 手改 |
| `.dev-team/architecture/graph/model/06-frontend.toml` | 新增一条关系 `App.Frontend.Views.Config → App.Frontend.Components.Agent`（`desc` 中文一句，沿用「经共享面板骨架渲染」语义）；不新建元素；表头注释与 `decisions/0001-model-granularity.md` §9 复算表按 `repo-archi-model` 漂移纪律同批同步 |
| `packages/desktop/package.json` | `version` patch +1（`0.7.2` → `0.7.3`）；`src-tauri/tauri.conf.json` 经 `../package.json` 跟随、`src-tauri/Cargo.toml` 不跟 |

### 测试文件

| 用例 / 断言 | 位置 | 处置 |
|---|---|---|
| config 臂装配：名字清单逐名（read / grep / glob / ls / bash / propose_config）+ `ask = Generic` + `finish = None` + `preamble` 非空；未知名 `Err` | `crates/infra/agent/src/compose_test.rs` | 新增用例 |
| 第 14 项挂载计数锚（`with_capacity` / `mount_test` / `policy_test` 三档等价表）与两处结构断言（`assemble_injections` 恰一处、逐来源下发面与装配单源一致） | `sdk/mount_test.rs` / `sdk/policy_test.rs` / `compose_test.rs` | 期望随动（判据零改） |
| `propose_config` 矩阵：只读模式返回盘面（`candidate: null` / `changes: []`）/ 脏候选逐条 `Err`（含 `path: message`）/ 干净候选 `Ok` 且两段均 file face / `changes` 与盘面差异一致 / 满配 fixture 输出 < 50 000 B | `sdk/config_propose_test.rs`（新） | 新增 |
| `ConfigContext.root` = 主仓 root（构造 `PumpPayload` 时 exec root 与 workspace root 不同，断言取后者） | `sdk/runner_test.rs` | 新增 |
| `propose_config` 无路径参数（`input_path` 返 `None`；候选含顶层键 `path` 时不被沙箱改写） | `sdk/tools_test.rs` / `sdk/sandbox` 侧既有用例 | 新增 |
| 知识文本：八键 / 八值派生自 `known_keys()` 与 `TestFramework::ALL`；零 `<>` 占位符；体量上限；拼装失败降级 `None` | `config_knowledge_test.rs`（新） | 新增 |
| 写面门：`plan_apply_config` 在任一 `InvalidValue` 时 `Err` 且 `ConfigWrite` 不产出（失败零写入）；成功时 `file == layout::config_path(root)` | `crates/core/config/src/lib_test.rs` | 新增 |
| 写入器 P1：文件面输出中 `extra` 值之外任何键名不含大写字母 | 同上 | 新增 |
| P2：满配 fixture 的顶层键集 == `known_keys()`（且无 `extra`） | 同上 | 新增 |
| P3：`assemble(to_file_value(cfg, 模板))` 与 `cfg` 逐字段相等（线面值比较） | 同上 | 新增 |
| P4：对「全字段显式」字面量 `L`（字母序、整数值）：`to_file_value(assemble(L).config, L) == L` | 同上 | 新增 |
| 整数化：`90` 出 `90`（非 `90.0`）；`90.5` 保浮点；`extra` 的 null 条目保留；键冲突显式 `Err` | 同上 | 新增 |
| 命令薄包装：`workspace_config_apply_inner` 与命令结果一致（不经 Tauri 运行时）；无效 root / 脏候选 → `Err`；成功后重读报告 | `src-tauri/src/commands/config/mod_test.rs` | 新增 |
| `write_atomic` 搬移后：`fs::write` 恰一处断言更新落点；只读目录失败且无残留 | `src-tauri/src/commands/archi/mod_test.rs` + `commands/atomic_test.rs`（新） | 断言随动 + 新增 |
| `list_config_chat_sessions`：`source` 过滤 / blank root 空页 / 不与 archi / change 会话相撞 | `src-tauri/src/commands/exec/mod_test.rs` | 新增 |
| `use-config-chat`：归属参数三件、禁发矩阵、新会话 / 历史点选 / root 复位 | `src/views/config/hooks/use-config-chat.test.ts`（新） | 新增 |
| `config-chat-panel`：骨架复用（`config-chat-*` testid）、收起零取数 | `src/views/config/components/config-chat-panel.test.tsx`（新） | 新增 |
| `ConfigProposeCard` 五态（进行中 / 失败 / 可应用 / `changes == []` 禁用 + 与当前一致 / 只读模式 / 解析失败降级）+ 应用调用 + 规范化明示 | `src/components/agent/message-cards/tool-cards.test.tsx` | 新增 |
| 配置页：会话面板挂载 + 应用成功后重取一次 | `src/views/config/config-view.test.tsx` | 扩用例 |
| 前端关系新增边可指到 `import`（`views/config` → `@/components/agent`） | 模型核对（实现期口径，非单测） | 手工核对 + `archi` 图读数 |

### 不要修改

- **`explore.md`**（本 change 的）：只读。
- **`crates/infra/agent/src/sdk/protect/**`**：写保护规则与内置锁定逐字不动；`config.json` 仍是不可移除的第一条内置锁。
- **`crates/core/agent/**`**：`SessionInjections` 四字段形态与事件词汇零扩展（`propose_config` 走名字清单通道；`ConfigContext` 在 infra 侧）。
- **`crates/infra/store/**`**：`session.source` 自由字符串语义不变，零迁移、零注册模型。
- **`KNOWN_KEYS` / `TestFramework` 八值 / `config.schema.json` 两断言**：本次不新增键、不改枚举、不加第三条断言；镜像零改动。
- **`assemble` 的单遍结构与 `opt_string` / `opt_string_list` 助手**：直接复用，不为写面新增参数。
- **`desktop-write-protection` 的 spec 与实现**：零改动（scope 是「Agent 不得写」）。
- **`plans` 与既有 `archi_set_gate` 的三条 root 文案**：`check_root` 不搬、不改文案；只搬 `write_atomic`。
- **`repo-archi-model` / `desktop-frontend-module-entry` 的计数复述**：既有漂移不在本变更修复范围内（R8）；本变更只按漂移纪律补一条新边与其表头 / ADR 复算。
- **`openspec/todo.md`**：待办勾选由流程面管理。

## 验收标准

| ID | 验收条件 | 验证方法 |
|----|---------|---------|
| AC-1 | config 臂装配逐名正确：`read` / `grep` / `glob` / `ls` / `bash` / `propose_config` 六名 + `ask = Generic` + `finish = None` + `preamble` 非空；**无** `write` / `edit` / db 工具；`RunStarted.tools` 与实际下发面一致；未知名装配层 `Err` | `compose_test` 新增用例 + 既有第四臂结构断言随动 |
| AC-2 | `propose_config` 三面行为：只读模式返回盘面（`candidate: null`、`changes: []`）；脏候选 `Err` 逐条诊断且**零副作用**；干净候选 `Ok` 且 `current` / `candidate` 均为 file face（无 `staticAnalysis` / `writeProtection` / `dependsOn` / `extra:[{key,value}]` 形态）；满配 fixture 输出 < 50 000 B | `config_propose_test` 矩阵 |
| AC-3 | `ConfigContext.root` 取主仓 root：以「`payload.workspace_root` ≠ `ctx.workspace_root`」的构造断言取前者（配置页会话 `cwd = null` 时二者同值；该断言把「将来被赋予 worktree 执行根」的形态提前钉住） | `runner_test` 新增用例 |
| AC-4 | 知识 preamble 零入参、零 IO、词表派生（八键 / 八值）、零占位符、体量有上限、拼装失败只丢 `preamble` 位 | `config_knowledge_test` |
| AC-5 | 写面门：`workspace_config_apply` 在任一 `InvalidValue` 时 `Err` 且**文件零变化**（mtime + 内容双证）；成功路径经 `to_file_value` → pretty + 尾换行 → `write_atomic`，返回重读报告；入参为 file-face `Value`（typed `WorkspaceConfig` 不得出现在签名上） | 命令测试 + 源码复核 |
| AC-6 | 写入器四条机械锚全绿：P1 键名无大写（`extra` 值内除外）、P2 满配顶层键集 == `known_keys()`、P3 `assemble ∘ to_file_value` 幂等于线面值、P4 全字段字面量往返逐字相等；整数化与 `extra` 三坑各有断言 | `lib_test` 新增矩阵 |
| AC-7 | 卡片五态齐备（进行中 / 失败 / 可应用 / `changes == []` 禁用 +「与当前配置一致」/ 只读模式 / 解析失败原文降级）；diff 基准取 payload `current`；多次 propose 各自独立可应用、无失效标记；应用成功后页面重取一次 | `tool-cards.test.tsx` + `config-view.test.tsx` |
| AC-8 | spec 改判零残留冲突：`desktop-workspace-config` 不再有「只读 / 无写路径」的活条款；`desktop-agent-execution` 三臂表述全部转四臂；`desktop-archi-agent` 兜底枚举已收窄；`desktop-change-chat` 骨架消费方为三处 | 基线全文检索「只读」「两处」「唯一生产者」「三臂」逐处复核 |
| AC-9 | 全管线绿：`pnpm -C packages/desktop run check`（`server:check` → `bindings:export` → `api:check` → `client:check`）与 `run test`、`cargo test --workspace` 全通过；生成物零手改（diff 只来自 `bindings:export`）；`package.json` version 为 `0.7.3`、`src-tauri/Cargo.toml` 版本不随动 | 静态检查链 + 全量测试 + `git diff` 复核 |
| AC-10 | 架构模型同步：`06-frontend.toml` 含 `Views.Config → Components.Agent` 一条新边且能在 `views/config/**` 找到 `import` 证据；模型可解析（零结构诊断）；表头注释与 ADR §9 与模型本体同值 | `archi` 读数 + 手工核对 |

> **验收面口径**：「检测出什么」**不可断言**——MUST NOT 以「该仓库应被识别为 N 个项目 / 依赖是 A ← B」作为验收条；验收只锚四类事实：注入臂装配、`propose_config` 契约矩阵、写面（门 + 失败零写入 + 写入器 P1–P4）、卡片五态。

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|---|---|---|---|
| **R1 候选质量不可断言**：agent 会话产出的候选取决于模型，可能漏项目、猜错依赖方向、给出不必要的字段 | 用户点「应用」写进一份「看起来对但不合预期」的配置 | 高 | 三重护栏：① 卡片呈现字段级 diff（盘面 vs 候选），用户逐项可见再点应用；② apply 侧二次校验（零 `InvalidValue` 才是合法配置）；③ 写回是可评审的文本文件，`git diff` 即回滚面。验收面口径把「检测准确性」排除在断言之外 |
| **R2 写回陷阱：用 wire face 落盘会静默毁配置**（`staticAnalysis` / `writeProtection` / `extra` 落 passthrough、`dependsOn` 被忽略） | 写保护规则静默失效、拓扑序安装退化，且现有自洽测试抓不到 | 中（只要有人图省事用 `to_value` 就必现） | `to_file_value` 为唯一落盘投影口；P1–P4 四条机械锚把「键名 / 键集 / 值 / 往返」钉死；spec 明文 MUST NOT 用 `to_value(&WorkspaceConfig)` 落盘 |
| **R3 写回会规范化文件**：字母序 + pretty 展开 + 行内数组拆行；`projects[i]` / `tests[i]` / `write_protection.files[i]` 的未知子键在组装路径上被丢弃 | 用户首次应用后看到一份「格式变了」的 diff，若不理解会视为缺陷 | 高（首次应用必然发生） | 裁决 C14/D8 接受规范化；应用确认面须**明示**「写回会规范化文件（未知子键、键序与格式会丢失 / 变化）」；不开 `preserve_order`（会把全仓 `Map` 换 `IndexMap`，波及所有 JSON 顺序断言） |
| **R4 双真值时刻**：preamble 若带盘面真值，会出现「preamble 拍 T0 / propose 拍 T1 / apply 拍 T2」 | 模型基于过期盘面提候选，用户看到陈旧 diff | 中 | 本期 preamble 只装静态协议知识、零盘面真值（C7）；盘面统一由 `propose_config` 只读模式提供（取数时机最晚）；A 方案不预先上，且上的前提是「每次装配读盘、MUST NOT 缓存」 |
| **R5 工具面机械扩散**：第 14 项要动 `mount.rs` 计数与两段 doc、`mount_test` 六处、`policy_test` 三档等价表、`compose_test` 两处结构断言、`tools.rs` 白名单 | 漏改 → 计数断言红 / 候选被沙箱当路径改写 | 中（机械但散） | 触点清单逐行列出；白名单漏改的后果被「候选含顶层键 `path` 不被改写」用例单独锚住；计数锚按期望随动、判据零改 |
| **R6 spec 改判漏改即实现违约**：`desktop-workspace-config` 的只读五处、`desktop-agent-execution` 的「唯一生产者」两处 + 三臂枚举、`desktop-archi-agent` 的兜底枚举、`desktop-change-chat` 的「两处唯一」都是硬承诺 | 实现完成仍被判违约 | 中 | 本变更 delta 已把这五处全部 MODIFIED（5 份能力 delta）；AC-8 以「基线全文是否有残留冲突句」为复核项（检索词：只读 / 两处 / 唯一生产者 / 三臂 / 其余非 change） |
| **R7 「写面不算字段消费者」的裁决可能被读成逃避**：切片 1 刚立「新增字段须有消费点」纪律 | 评审质疑「apply 动所有字段却自称不消费」 | 中 | 裁决写明理由：写面是**全字段透传**（唯一字段名清单 = `KNOWN_KEYS` + 三处重命名表，无任何按字段的语义分支），保真度交给 P1–P4；并把该行登记进字段消费点表，不靠沉默 |
| **R8 架构模型计数已存在漂移**：spec 文本写 53 元素 / 120 关系、`06` 段 30 / 71，模型实读 **54 / 122**、`06` 段 31 / 73（多出 `App.Frontend.Components.Standard.Combobox` 元素） | 若不声明，新增一条边后读数与 spec 差得更远；若顺手「修好」，则把两个议题混进一个变更 | 高（确定性） | 显式裁决 D16：本变更**只为新增边**同步模型本体与表头 / ADR 复算（按 `repo-archi-model` 漂移纪律），MUST NOT 在本变更内改写 `repo-archi-model` / `desktop-frontend-module-entry` 的计数复述文本；存量漂移单列为独立议题，避免「借机重排」掩盖真实增量 |
| **R9 配置页新增跨元素依赖被判纪律违约**：若实现只写 `import` 而不声明模型关系 | `repo-archi-model`「漂移纪律」判不合格（已知缺口清单不得为本次新增项开脱） | 中 | 触点清单含模型文件一行；AC-10 以「新边可指到 `import` 证据」为验收条 |
| **R10 应用后页面与转录不同步**：应用成功后盘面已变，但历史候选卡仍是旧 `current` | 用户可能基于旧卡再点应用（覆盖他人/自己的新改动） | 低 | 裁决 C21：应用成功即 `refresh()` 刷新页面；卡片 diff 恒以自身 payload `current` 为基准（C19），不做跨时刻比较；多次 propose 各卡独立、无失效标记（C20）——知情接受，回滚面仍是 `git diff` |
| **R11 架构建模的粒度争议**：`views/config/` 的会话面板是否该单建前端元素（先例 `Views.Changes.Agent`） | 若评审要求单建元素，元素 / 关系计数再变一次 | 低 | 裁决 D15 明确**不新建元素**：`views/config/**` 现状即全部归 `App.Frontend.Views.Config`（`components/` 目录不构成子域、`hooks/` 不建元素），新建面板文件沿用同形，最小化模型扰动 |
