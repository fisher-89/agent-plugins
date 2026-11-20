# 切片 2：配置页常驻 config 会话（检测 / 提案 / 写面）

workflow_type: requirement

> **出处**：`openspec/todo.md`「扩充 project 配置协议……配置页面支持 Agent 检测生成配置」。
> **切片关系**：切片 1（`projects[].language` / `depends_on` + 拓扑序安装）已归档（`openspec/changes/archive/2026-11-19-project-config-protocol-extension`）。本文只承载切片 2：配置页从「只读展示」变成「常驻 config 会话 + 提案 + 用户确认后写盘」。
> **一句话结论**：新增 `source = "config"` 会话臂（只读工具 + 单一 `propose_config`）+ 静态协议知识 preamble + 用户点「应用」触发的一次原子写回。写回必经「零 `InvalidValue` 门」与**文件面写入器**（不是 `serde_json::to_value`）。全部裁决见 §7。

---

## 1. 目标与边界

### 1.1 能力

配置页（`/config`）当前只读。本期加三件事：

1. 左栏常驻一个 **config 会话**（复用 `SessionPanel`，可对话、可续历史、边聊边改候选）；
2. 模型产出配置候选的唯一出口 `propose_config`（自带校验回环，脏候选当场退回自纠）；
3. 用户点「应用」→ 后端二次校验 → 原子写回 `config.json` → 页面刷新。

### 1.2 为什么是 agent 会话，而不是确定性检测器

确定性检测器（扫 lockfile / 扩展名直方图 / 框架 registry）零 token、可断言，但**猜不准项目间依赖**（`depends_on`），而这恰是本次需求的核心之一。选 agent 会话后要接受一个后果：

> **「检测出什么」不可断言。** 测试只锚四类事实（§6），MUST NOT 断言「这个仓库应被识别为 2 个项目、依赖是 A←B」。

退路保留：将来补一个检测器，把输出作为首轮 prompt 种子即可，注入臂不用改。现成积木（本期不做）：`commands/stats` 的 tokei 直方图、`infra/checks/src/testexec/detect.rs` 的 `REGISTRY`（每框架 `version_command` / `default_glob` / `config_flag`）。

### 1.3 写权限前提：agent 永远不能写 config.json

`config.json` 是写入保护的内置锁定第一条（`infra/agent/src/sdk/protect/rules.rs::builtin_rules` 恒一条 `**/<配置名>`，不可配置、不可移除）——它自己是保护规则的权威，自锁。整条链因此长成这样：

```
agent 会话（source = "config"）
   ├─ 只读工具：read / grep / glob / ls / bash（探测清单 / lockfile / 测试配置 / 现有 config.json）
   └─ propose_config(candidate) → 零 invalidValue 才 Ok，脏候选当场 Err 逐条诊断
             │  （不落盘、不落库，候选只活在会话转录里）
             ▼
        用户点「应用」（页面 diff：字段级新增 / 修改）
             ▼
        workspace_config_apply：check_root → plan_apply_config（二次校验）→ to_file_value → write_atomic
```

写面先例：`archi_set_gate`（`commands/archi/mod.rs:153`）= `check_root` → `plan_set_gate`（纯函数、任一校验失败零写入）→ `write_atomic`。

---

## 2. 关键事实（决定设计形状）

### 2.1 世界里有两个 root；配置真值属于主仓 root

```
compose_turn(workspace_root = 主仓 root)      ← 写保护规则装配根（= 配置权威所在）
   ├─ facade.with_workspace_root → PumpPayload.workspace_root
   │     └─ session_pump: ProtectRules::load(主仓 root)   ← 每轮一次，内部就是 config::load(root)
   └─ ComposedTurn { kernel, runner, query, store_kind, models, agent_instance_id }  ← 主仓 root 不落地

begin(ctx, provenance)
   └─ ctx.workspace_root = exec root（exec 轨道 = cwd ?? root；change_flow = worktree）
         ├─ turn.cwd = exec root（工具沙箱 / bash / AGENT.md）
         └─ mint_archi_context(..., ctx.workspace_root)   ← archi 工具在 change 会话读 worktree
```

证据：`compose.rs:234`（`compose_turn` 的 root 参数）、`compose.rs:197`（`ComposedTurn` 无 root 字段）、`commands/exec/mod.rs:157`（传主仓 root）、`commands/change_flow/mod.rs:169-179`（注释「MUST NOT 传 exec_root / worktree 路径」）、`sdk/runner.rs:296` / `:576`（facade root → `ProtectRules::load`）、`runner.rs:458-462`（`mint_archi_context` 用 ctx root）。

⇒ **C8**：`ConfigContext.root` = `payload.workspace_root`（主仓），MUST NOT 取 `ctx.workspace_root`。`mint_archi_context` 用后者是因为 archi 语义是「看我现在改的那棵树」，配置语义相反（权威在仓根）。

顺带：`SessionCtx.workspace_root` 的 doc（`core/agent/src/runner.rs:114`「隐含当前 workspace root」）已名不副实（实为 exec root）——既存漂移，本期不修。

### 2.2 config 有两副面孔：wire face 与 file face

`WorkspaceConfig` 只实现 `Serialize`，且挂 `#[serde(rename_all = "camelCase")]`、`extra` 是 `[{key,value}]` 包装：

| 面 | 键形 | 用途 |
|---|---|---|
| **wire**（`to_value` / IPC / TS / zod） | `staticAnalysis` / `writeProtection` / `dependsOn` / `extra:[{key,value}]` | `workspace_config` 报告、前端展示 |
| **file**（config.json） | `static_analysis` / `write_protection` / `depends_on` / `extra` 展开回顶层 | 落盘 |

逐字段对位后真正不同名的**只有四处**（其余全是单词键，两种风格同形）：`static_analysis` / `write_protection` / `depends_on` / `extra`。`$schema` 已有显式 `#[serde(rename)]`，wire 面即正确。

⇒ **`serde_json::to_value(&WorkspaceConfig)` 绝不能用于落盘**：重读时 `staticAnalysis` / `writeProtection` / `extra` 全落 passthrough（写保护规则静默失效、`static_analysis` 丢失），`projects[i].dependsOn` 被元素级忽略（拓扑序安装退化）。而现有自洽测试抓不到——`assert_self_consistent`（`lib_test.rs:112`）只断言「零诊断」，其 doc 明写「线面 `extra` 回写再入场……不在无损断言之列」。

### 2.3 注入面是「每条用户消息」重算，不是会话快照

系统提示词在每次 LLM 调用前由 `turn.injections.preamble` + 盘上 `AGENT.md` 现拼（`sdk/loop.rs:307-315`）；`injections` 由 `ComposedTurn::begin` 每轮重算（`compose.rs:360`），而 `begin` 是每条用户消息的入口。

⇒ 静态 preamble 无时效性问题；**不需要**「配置已变更，自行 read 复核」或「应用后请开新会话」的提示（C21）。

### 2.4 `core/config` 的公共面今天只有 `load`

`pub fn load(&Path)`（`lib.rs:942`）是唯一公开函数；`assemble`（`:773`）、`assemble_project`（`:707`）、`opt_string` / `opt_string_list` / `threshold`、`KNOWN_KEYS`、`parse_framework` 全私有；`TestFramework` 既无 `ALL` 也无 `as_str`。

⇒ 新增公共面：`known_keys()`、`TestFramework::ALL` + `as_str()`（与 `parse_framework` 双向互证）、`validate(&Value) -> ConfigReport`、`to_file_value(&WorkspaceConfig, &Value) -> Value`、`plan_apply_config(&Path, &Value) -> Result<ConfigWrite, String>`。

`agent-runtime` 已依赖 `config`（`crates/infra/agent/Cargo.toml`），工具执行体**不新增 crate 依赖边**。

### 2.5 serde_json 两个事实（已核实源码）

1. **未开 `preserve_order`**（`Cargo.toml:137` 只开 `raw_values`）⇒ `Map` 是 `BTreeMap`，键按**字母序**输出。真实 `.dev-team/config.json` 是手写直觉序（`$schema` → `schema` → `static_analysis` → …），写回后 `schema` 会从第二位掉到第五位。
2. **`Number` 的整数与浮点不相等**（serde_json `number.rs:37-46`：`(PosInt, Float)` 落 `_ => false`）⇒ `json!(90) != json!(90.0)`。`threshold()` 返回 `f64`，直接序列化必出 `90.0`。

### 2.6 工具面有强机械扩散（十三项计数锚）

加第 14 项要动：`sdk/mount.rs:108`（`with_capacity(13)` + 两段 doc）、`mount_test.rs:17/62/65/70/175/567/1070`、`policy_test.rs:87`（三档等价表）/`:163`；另 `compose_test.rs:3860`（「`fn assemble_injections` 恰一处」）与 `:4937`（逐来源下发面与装配单源一致）两处结构断言会跟着红。执行体还要新增 `ToolExec` 变体 + `loop.rs:676-710` 的 match 臂。

另：`sdk/tools.rs:50-61` 的「无路径工具白名单」今天只列 archi ×3 / db ×2，**`propose_config` 必须加进去**——否则候选里若出现顶层未知键 `path`，`sandbox_rewrite`（`loop.rs:1097`）会把它当工具路径改写甚至沙箱拒绝。

### 2.7 不受影响的面

- **store**：`session.source` 是自由字符串（`agent_start` 的 `source.unwrap_or("debug")`），无枚举 / 无 CHECK；`store.rs:1024` / `:1168` 的 explore 特判不会误纳新来源。零迁移。
- **契约面**：`App.Backend.Core.Config` = `status = "absent"` / 0 ops（`.dev-team/architecture/graph/INDEX.toml:33-38`）。新增公共函数不新增门禁（`apiUndeclared` 落 `off` 档），也不会自动长契约文档。
- **写保护**：内置锁的 scope 是「Agent 不得写」，apply 是应用进程自写、用户点击触发，不冲突也不需改 spec。
- **架构域配置**：`layout::archi_config_path` 是 `<root>/.dev-team/architecture/config.json`，与本文件不同文件，也不在内置锁 glob 内。

---

## 3. 设计

### 3.1 注入臂（`compose::assemble_injections` 第四臂）

```
source = "config"
  tools    = [read, grep, glob, ls, bash, propose_config]   ← 只读 + 单一 propose，无 write/edit，无 db 工具
  preamble = config_knowledge::knowledge()                  ← 静态协议知识，签名无入参
  finish   = None                                           （不是 evaluator / decision 形态）
  ask      = Generic                                        （与 explore / archi 同档）
```

`source` 词常量三处同字面量（compose / 命令层 / 前端 hook）：既有 4 类来源已积 12 个散点，本期再加 3 处，收口另立议题。

### 3.2 preamble：静态协议知识

生产者仿 `infra/agent/src/archi_knowledge.rs`（`build(SKELETON).ok()`、`include_str!` 骨架、显式拒绝 `<>` 占位符、失败只丢 preamble 位）：

```
静态骨架 config-knowledge.md   ← 字段语义 / 值域 / 语义纪律 / projects 两个软规则字段的含义
+ 代码派生词表                  ← known_keys() 八键、TestFramework::ALL 八值
+ 使用指引                     ← 「propose_config 不给候选时返回当前盘面（组装结果 + 诊断逐条）」
```

**盘面真值不进 preamble**（C7）：由 `propose_config` 的只读模式提供——取数时机更晚更准，且不产生「preamble 拍 T0 / propose 拍 T1 / apply 拍 T2」的双真值时刻。将来若要「首轮即知现状」，走 §8 的 A 方案，且必须同时立「生产者每次装配读盘、MUST NOT 缓存」纪律。

### 3.3 `propose_config` 工具

```
propose_config({})                    → Ok { current, candidate: null, changes: [], summary }   ← 只读模式
propose_config({ candidate: {...} })  → Ok { current, candidate, changes: [...], summary }      ← 零 invalidValue
                                      → Err(is_error) 纯文本逐条诊断（path: message）+「修正后重交完整候选」
```

- 落位：`infra/agent/src/sdk/config_propose.rs` + `mount.rs` 常量 + 挂载表第 14 项（`tier = Read`，`exec = ToolExec::Config`），只对 `config` 源可见。定义参数面 `{ candidate?: object }`，`required: []`。
- **不落盘、不落库、无副作用**：候选只活在会话转录（工具 input/output 已持久化，历史会话重放时卡片照常出现）。
- **不复用 `finish`**：`FinishSpec` 是轮级终止工具（evaluator checklist / decision 两变体封闭集），聊天面板不该有终止面；propose 一轮内可反复调用。
- 输出恒为 JSON **字符串**（`ToolResult.content` 是扁平字符串，`core/agent/src/event.rs:68-72`），两段配置一律 **file face**；`changes` 由后端算（先例 `archi_change_summary`），前端零配置语义。
- **体量**：`cap_result` 上限 50 000 B（`tools.rs:28`），截断即成不可解析串 ⇒ 需一条「满配 fixture 输出 < 上限」的测试。

### 3.4 写面命令

```
workspace_config_apply(root, candidateFileJson)
   check_root（commands/config 内私有 fn）
   → plan_apply_config(root, candidate)：validate → 有 invalidValue 即 Err（零写入）
   → to_file_value(&report.config, candidate)     ← 文件面投影（模板驱动）
   → to_string_pretty + 尾换行
   → write_atomic(layout::config_path(root), content)
   → 返回新报告（前端直接替换，省一次往返）
```

- **双校验不是冗余**：propose 侧校验可能发生在很久以前、可能来自历史会话重放；apply 侧不信任调用方（前端载荷可伪造）。
- `plan_apply_config` 放 `core/config`（纯函数、零 IO、可测），沿 `archi::edit::GateWrite` 形状返回 `ConfigWrite { path, content }`。`foundation::layout::config_path` 已是既有依赖，不引新边、不触 `config.json` 字面量禁令。
- 入参 = **file-face `serde_json::Value`**（TS 出线 `unknown`，前端原样搬运 payload 的 `candidate`），MUST NOT 用 typed `WorkspaceConfig`（C11）。
- `write_atomic` 搬去新文件 `src/commands/atomic.rs`（`pub(crate)`）去重；`check_root` **不搬**（archi 的两条 root 文案已分叉，强行统一会改既有断言），在 `commands/config/mod.rs` 内抽私有 fn 自用（C16）。

### 3.5 文件面写入器（本切片最容易埋雷的一块）

**签名**：`to_file_value(config: &WorkspaceConfig, template: &Value) -> Value`（**模板驱动**，C12）。

- `WorkspaceConfig` 不记录 provenance（`cwd: "."` 无法区分「文件写了 `.`」与「吃了默认」），所以「写全 / 写最小」不是 flag，而是**入参类型**：按 `template`（= 候选原文）出现过的键裁剪，值取规范化结果。
- 「写全字段」= 模板取全键集的同一特例，**不设第二形态、不设 flag**。

**实现**：结构变换（不是逐字段手拼 Map）：`to_value` → 三处 `rename_in_place`（`staticAnalysis`→`static_analysis`、`writeProtection`→`write_protection`、`dependsOn`→`depends_on`，**递归全层级**：`dependsOn` 在 `projects[i]` 里）→ `expand_extra`（`extra:[{key,value}]` 展开回顶层）→ 数值整数化 → 按模板裁剪。

- 结构变换让「新增字段自动出线」；手拼 Map 会多出第二份字段清单，新增必漏。
- **数值整数化**（C13）：`fract() == 0.0` 且绝对值不超过 `2^53` 时出 `i64`，否则保留浮点（否则用户写 `90` 会变 `90.0`）。
- **`extra` 三个坑**：① null 值条目必须保留（丢 null 规则只作用于已知 Option 字段，MUST NOT 作用于 extra，否则回读少一条）；② 键序按字母序重排（不可保真，接受）；③ 键冲突必须显式 `Err`——若将来给结构体加字段却忘进 `KNOWN_KEYS`，字段与 extra 副本会同键，`Map::insert` 会静默覆盖。

**四条机械锚**（C15）：

| # | 性质 | 抓住什么 |
|---|---|---|
| P1 | 文件面输出中，`extra` 值内部以外**任何键名不含大写字母** | 新增多词字段忘登记重命名表（自证，零维护） |
| P2 | 满配 fixture 的**顶层键集 == `known_keys()`**（且无 extra） | 写入器漏写 / `KNOWN_KEYS` 漏登记。使 `KNOWN_KEYS` 成为写入器键集唯一持有者，并继承既有闭包「`KNOWN_KEYS` 与 `config.schema.json` 的一致性断言」⇒ 写入器与镜像自动同源 |
| P3 | `assemble(to_file_value(cfg, 模板))` 与 `cfg` **逐字段相等**（线面值比较，`WorkspaceConfig` 无 `PartialEq`） | 值映射错误 / 字段整块丢弃 / extra 丢项 |
| P4 | 对「全字段显式」字面量 `L`：`to_file_value(assemble(L).config, L) == L` | 写入器与组装层**双向**对位；字面量本身即 fixture 完备性。字面量须字母序（§2.5）且数值写整数 |

**写回会重排键序与格式**（字母序 + pretty 展开，行内数组全拆，C14）⇒ 应用确认面须明示「写回会规范化文件（未知子键、键序与格式会丢失 / 变化）」。不开 `preserve_order`（会把全仓 `Map` 换 `IndexMap`，波及所有 JSON 顺序断言）。

**未知子键会丢**：`extra` 只捕获**顶层**未知键；`projects[i]` / `tests[i]` / `write_protection.files[i]` 内的未知子键在组装路径上被静默丢弃——接受（规范化），UI 明示。

**`defaultApplied` 不拦**（C4）：它是正常形态，前端渲染成弱化的「未设（默认 N）」标注而非错误（`basic-config-section.test.tsx:92`）。它只由 `defaulted(` 的八处触发：`schema` 缺、`tests` 缺、suite 的 `cwd` / coverage 三阈值 / mutation score；`context` / `rules` / `static_analysis` / `write_protection` / `$schema` 走 `opt_string` **永不产** `defaultApplied`。故「清零噪音」的准确含义是给**有默认值的**字段写真值，而不是给 Option 字段写 `null`。

### 3.6 前端装配

| 要做的 | 照抄谁 |
|---|---|
| `use-config-chat`（清单分页 / 新会话 / 历史切换 / root 复位 / `running` 禁发矩阵） | `src/views/architecture/agent/hooks/use-archi-chat.ts` |
| 会话面板（草稿态 / 运行中 / 停止 / 补充说明） | `<SessionPanel>` / `<AgentSessionView>` |
| 清单命令 `list_config_chat_sessions(root, offset, limit)` | `commands/exec/mod.rs::list_archi_chat_sessions`（来源词常量收在命令层） |
| `ConfigProposeCard`（盘面 vs 候选字段级 diff + 「应用」） | `EditCard`（`tool-cards.tsx`）的 added / removed 双 `CodeBlockLine`；注册进 `BUILD_IN_TOOL_CARDS` |
| 应用成功后刷新页面 | `useWorkspaceConfig().refresh()` |

卡片**五态**（C18；判据锚在 `ToolResult` 形态）：无 `ToolResult` 块 = 进行中；`is_error` = 失败（诊断原文）；`candidate != null` = 可应用（`changes == []` 时禁用 + 「与当前配置一致」）；`candidate == null` = 只读模式（仅盘面展示）；`content` 不可解析 = 原文降级渲染。

diff 基准 = payload 里的 `current`（与 candidate 同刻同源），MUST NOT 读页面 hook 数据做跨时刻比较（C19）。多次 propose 各一张卡、都可应用（最后点哪个算哪个），不做「已被取代」失效标记（C20）。

---

## 4. spec 改判面

| 能力 | 位置 | 处置 |
|---|---|---|
| **新建 `desktop-config-agent`** | — | 承载 config 会话面板与归属 / 注入臂 / 知识 preamble / `propose_config` 工具面 / 写面命令与「agent 不可写」边界 / 提案卡。仿 `desktop-archi-chat` + `desktop-archi-agent` 二分，但收敛为一份（config 侧体量不足拆两份） |
| `desktop-workspace-config` | `:5`（Purpose）/ `:156`（主条款）/ `:198`（只读边界 scenario）/ `:287`、`:302`（契约表两行） | 只读条款五处改判：新边界 = 写回唯一入口「应用」按钮、agent 无写权限、写面零 invalidValue 门 |
| 同上 | `:252`（字段消费点表） | 登记新消费面，或显式裁决「写面不算字段消费者」——切片 1 刚立的纪律条款 |
| `desktop-agent-execution` | `:377`（三臂逐字枚举）/ `:1830`（契约表） | 三臂转四臂 + 工具名清单 |
| `desktop-archi-agent` | `:170` / `:174` | 「其余非 change 来源恒七内建全量」与 config 臂直接矛盾，收窄为「debug 等」的显式枚举 |
| `desktop-write-protection` | — | **零改动**（scope 是「Agent 不得写」）；「apply 不经 agent 工具面」写进 `desktop-config-agent` |

---

## 5. 触点清单

| # | 文件 | 改动 |
|---|---|---|
| 1 | `crates/infra/agent/src/compose.rs` | `assemble_injections` 加 `config` 臂 + `CONFIG_SOURCE`；doc 三臂转四臂；**签名不变** |
| 2 | `crates/infra/agent/src/config_knowledge.rs` + `config-knowledge.md`（新） | 静态 preamble 生产者（骨架 + 派生词表 + 使用指引），签名无入参、零 IO |
| 3 | `crates/infra/agent/src/sdk/config_propose.rs`（新）+ `sdk/mod.rs` + `sdk/mount.rs` + `sdk/tools.rs:50` 白名单 | `propose_config` 常量、第 14 项挂载（`Read` / `ToolExec::Config`）、执行体 |
| 4 | `crates/infra/agent/src/sdk/runner.rs` + `sdk/loop.rs` | `ConfigContext { root }` + `mint_config_context(names, payload.workspace_root)` + `LoopTurn` 字段 + `ToolExec::Config` 分派臂 |
| 5 | `crates/core/config/src/lib.rs` | `known_keys()`、`TestFramework::ALL` / `as_str()`（双向互证）、`validate`、`to_file_value(config, template)`、`plan_apply_config`；补模块 doc（现有 `assemble` doc 引用了不存在的「模块 doc null 位语义」）；P1–P4 锚 |
| 6 | `src-tauri/src/commands/config/mod.rs` | `workspace_config_apply` + `*_inner`（file-face `Value` 入参）+ 私有 root 校验 |
| 7 | `src-tauri/src/commands/atomic.rs`（新） | `write_atomic` 搬移；`commands/archi/mod.rs` 改引用，`archi/mod_test.rs:1193`「`fs::write` 恰一处」断言随落点更新 |
| 8 | `src-tauri/src/commands/exec/mod.rs` | `list_config_chat_sessions`（照 `list_archi_chat_sessions`） |
| 9 | `src/views/config/hooks/use-config-chat.ts`（新） | 会话状态 / 清单分页 / 禁发矩阵 |
| 10 | `src/views/config/components/config-chat-panel.tsx`（新） | 面板装配 |
| 11 | `src/components/agent/message-cards/*` | `ConfigProposeCard` + `BUILD_IN_TOOL_CARDS` 注册 + 五态测试 |
| 12 | `src/views/config/config-view.tsx` | 挂载会话面板（页面从只读变为「会话 + 应用」） |
| 13 | `src/types/generated/*` | `bindings:export` 刷新（`apply` 入参出线 `unknown`） |
| 14 | `crates/infra/agent/src/sdk/mount_test.rs` / `policy_test.rs` / `compose_test.rs` | 第 14 项计数锚与结构断言跟随（§2.6） |
| 15 | spec | §4 |
| 16 | `packages/desktop/package.json` | patch +1（现值 `0.7.2`） |

---

## 6. 测试锚（四类，MUST NOT 越界断言）

1. 注入臂装配（工具名清单 / 未知名 `Err`）+ `ConfigContext.root` 取主仓 root（MUST NOT exec root）；
2. `propose_config` 契约矩阵（只读模式返回盘面 / 脏候选逐条 `Err` / 干净候选 `Ok` file-face / 输出 < 50KB）；
3. 写面（零 invalidValue 门 / 失败零写入 / 写入器 P1–P4）；
4. 卡片五态（含只读模式与解析失败降级）。

---

## 7. 裁决台账

| # | 裁决 |
|---|---|
| C1 | `projects[].language` / `depends_on` 协议面零改动（切片 1 定稿），本期只在文件面原样读写 |
| C2 | propose 一次全量候选，不接受片段；候选缺省 = 只读模式返回盘面 |
| C3 | 写回唯一入口 = 「应用」按钮；agent 零写权限；内置写保护语义不变 |
| C4 | 零 `InvalidValue` 才落盘；`defaultApplied` 不拦 |
| C5 | `context` / `rules` 不主动补，只在用户明确要求时写；preamble 说明二者零消费 |
| C6 | 工具面 = read / grep / glob / ls / bash / propose_config；无 write / edit、无 db 工具 |
| C7 | preamble = 静态协议知识（无入参、零 IO）；盘面真值走 `propose_config` 只读模式；A 方案不预先上 |
| C8 | `ConfigContext.root` = 主仓 root，MUST NOT 取 exec root |
| C9 | `propose_config` 输出恒为 file-face JSON；MUST NOT 用 `to_value(&WorkspaceConfig)` |
| C10 | 与「检测支持的 agent 引擎」是两条独立待办，preamble 不注入 provider 列表 |
| C11 | apply 入参 = file-face `serde_json::Value`，MUST NOT 用 typed `WorkspaceConfig` |
| C12 | 写入器 = 模板驱动 `to_file_value(config, template)`，不设第二形态 / flag |
| C13 | 数值整数化（`fract() == 0` 且绝对值不超过 `2^53` 时出 i64） |
| C14 | 不开 `serde_json/preserve_order`；接受字母序 + pretty 重排，UI 明示 |
| C15 | 四条机械锚 P1–P4 全上 |
| C16 | `write_atomic` 搬 `src/commands/atomic.rs` 去重；`check_root` 不搬，config 内私有 |
| C17 | 落盘 = `to_string_pretty` + 尾换行 |
| C18 | 卡片五态 |
| C19 | diff 基准 = payload `current`，不跨时刻比较 |
| C20 | 多次 propose 各一张卡、都可应用、不做失效标记 |
| C21 | apply 后 `refresh()`；不需要「请开新会话」提示 |
| C22 | 新建 `desktop-config-agent`；`desktop-workspace-config` 只删只读条款 + 扩呈现面 |
| C23 | `desktop-agent-execution` 三臂转四臂（含 `desktop-archi-agent` 兜底表述收窄） |
| C24 | `desktop-write-protection` 零改动 |

---

## 8. 知情边界与唯一开放项

**不做**（知情，非遗漏）：确定性检测器（种子保留为将来退路）/ 架构模块按 project 划分 / 按语言分派 prompt 与检查（切片 3）/ `depends_on` 影响面传播 / `.dev-team/config.json` 自身字段补全 / 来源词常量收口（现 12 散点 + 3）/ `SessionCtx.workspace_root` doc 漂移修正 / `App.Backend.Core.Config` 契约文档补全。

**唯一开放项**：A 方案（preamble 带真值）是否补。指标 = 「config 会话首轮是否明显多一轮工具往返」。形状已定（`ComposedTurn` 增 `workspace_root`、`assemble_injections(&provenance, root)`、4 个构造点 `compose.rs:289` / `:455` + `compose_test.rs:1242` / `:2425`、测试辅助 `injections_of` 一处），并须同时立「生产者每次装配读盘、MUST NOT 缓存」纪律。不预先做。
