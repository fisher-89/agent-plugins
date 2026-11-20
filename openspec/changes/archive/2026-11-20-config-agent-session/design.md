# 设计: config-agent-session

> **变更**: config-agent-session
> **状态**: 设计定稿（基于已定稿 proposal D1–D16 / 明确不做清单 / 五份 delta spec）
> **基于**: `proposal.md`（决策表 D1–D16、变更范围、AC-1…AC-10、R1–R11）、`specs/{desktop-config-agent,desktop-workspace-config,desktop-agent-execution,desktop-archi-agent,desktop-change-chat}/spec.md`、`explore.md`（§2 事实 / §3 设计 / §7 裁决 C1–C24）
> **性质**: **一条新会话臂 + 一个提案工具 + 一次用户触发的原子写回 + 一份文件面写入器 + 一处面板与卡片装配**。零新增 crate、零新增依赖边、零 store 迁移、零写保护语义改动。
>
> **本篇边界**: `crates/core/config`（公共面 + 文件面写入器）、`crates/infra/agent`（注入臂 / 知识 / 提案工具 / per-turn 上下文）、`src-tauri/src/commands`（写面命令 + 清单命令 + `write_atomic` 搬移）、`packages/desktop/src`（会话面板 / 提案卡 / 配置页分栏）、架构模型三件套（`06-frontend.toml` 一条新边 + 表头 / ADR §9 复算 + barrel 契约散文）、`package.json` 版本行。

---

## 提案与规格同步状态

proposal 的十六条决策、`明确不做` 清单、AC-1…AC-10 与五份 delta spec 已把语义钉到键名 / 工具名 / 两模式 / 写面三段 / P1–P4 / 卡片五态粒度。本设计不重复论证，只做四件事：

1. **把 spec 已钉语义、未钉落点的形状收敛到具体签名与代码形态**（DD1–DD15）；
2. **对 proposal 措辞做工作树校准**（§事实校准，8 条）——其中三条会改动 proposal 的公共面 / 契约面清单（`to_file_value` 返回位、`load_file_face` 的必要新增、barrel 第 8 个导出名）；
3. **逐条 AC 给落点**（末尾「验收标准落点」表），不写测试策略；
4. **把「非显然正确」写成可实现形态**：`to_file_value` 的递归裁剪与 `extra` 展开顺序、`changes` 的数组叶子口径、卡片拿不到 `root` 时的动作面通道、`06` 表头计数的「只加自己那一条」口径。

---

## 起点核实（工作树实测；锚点为改动前行号）

- **注入臂**：`crates/infra/agent/src/compose.rs:45` `assemble_injections`（explore 臂 → archi 臂 → `:80` 非 change 兜底 → change 臂）；`:110` `injected`（成员校验 + 名字清单落地，签名已含 `preamble` / `finish` / `ask` 四参）；`:20-31` 三个来源常量（`EXPLORE_SOURCE` / `CHANGE_SOURCE` / `ARCHI_SOURCE`）。
- **挂载清单**：`crates/infra/agent/src/sdk/mount.rs:100` `registry()`、`:109` `Vec::with_capacity(13)`、`:110-158` 十三项按序构造；`ToolTier`（`:70`）/ `ToolExec`（`:81`）两枚举；`lookup` / `definitions`（`:160` 起，`ask` 位与 `finish` 位两条独立追加分支）。
- **沙箱豁免**：`crates/infra/agent/src/sdk/tools.rs:50` `input_path`（archi ×3 + db ×2 显式 `None` 白名单）；`:28` `MAX_RESULT_BYTES = 50_000`；`:86` `cap_result`。
- **per-turn 铸造**：`crates/infra/agent/src/sdk/runner.rs:458` `mint_archi_context(names, ctx_root)`（用 `open.ctx.workspace_root`）、`:469` `mint_db_context`；`:415` `PumpPayload`、`:443` `workspace_root: Option<PathBuf>`（主仓 root）；`:520-545` 三处铸造调用；`:577` `LoopTurn` 装配。**`payload.workspace_root` 已在手且语义正是主仓 root**（`:576` 同一值喂 `ProtectRules::load`）⇒ 本变更零 `PumpPayload` 字段新增。
- **loop 分派**：`crates/infra/agent/src/sdk/loop.rs:120` `LoopTurn`、`:146` `archi` 字段、`:683` / `:690` 两个 `match mount.exec` 臂（上下文缺席以 `is_error` 回灌）。
- **config crate**：`crates/core/config/src/lib.rs:27` `KNOWN_KEYS`（八键，私有）、`:40` `TestFramework`（八值，无 `ALL` / 无 `as_str`）、`:399` `parse_framework`（私有）、`:773` `assemble`（私有，收 `&Map`）、`:942` `load`（唯一公共函数，四分支读取）。`:1` 起**无模块 doc**（`assemble` 的 doc 却引用了「模块 doc『null 位语义』」）。
- **命令面**：`src-tauri/src/commands/config/mod.rs`（单命令 `workspace_config` + `*_inner`，无 State）；`src-tauri/src/commands/exec/mod.rs:429` `list_archi_chat_sessions`（`:33` `ARCHI_CHAT_SOURCE` 常量、`:36` `is_blank_root`）；`src-tauri/src/commands/archi/mod.rs:59` `write_atomic`（私有）+ `:157` `archi_set_gate_inner` 调用；`commands/mod.rs:30` `all_commands!` 宏体（`:82` 现登记 `config::workspace_config`）。
- **前端**：`views/architecture/agent/hooks/use-archi-chat.ts`（246 行，`ARCHI_CHAT_SOURCE` / 清单分页 / 面板本地态 / 重取时机 / 判据矩阵 / `note` 三态）；`views/architecture/agent/archi-chat-panel.tsx`（16 行，`SessionPanel` + composer 插槽）；`components/agent/session-panel.tsx:14` `SessionPanelState`（骨架唯一状态面）；`components/agent/agent-messages.tsx:78` `ToolPartView`（卡片入参装配点，`askAnswer` 是**唯一**卡片级动作面）；`components/agent/message-cards/tool-cards.tsx:494` `BUILD_IN_TOOL_CARDS`（7 键）、`:443` `EditCard`（added / removed 双 `CodeBlockLine` 先例）；`views/config/config-view.tsx`（单列 + 五分区 + `config-refresh`）；`components/agent/index.tsx`（barrel，6 条 re-export / 7 个名字）。
- **契约与模型**：`App.Frontend.Components.Agent`（`api = declared`，契约 md 的公共面 7 名**全留白**：3 个组件不进 `api-op`、4 个类型不进 `api-reexport`，成因逐条在案）；`App.Frontend.Components.Agent.ToolCards`（`api = declared`，公共面同样留白，卡组件不外露）；`App.Backend.Core.Config`（`api.status = absent` / 0 ops ⇒ 新增公共函数不新增门禁）；`06-frontend.toml:3` 表头「30 元素 / 71 关系」、`:40` `Views.Config`、`:93` `Components.Agent`；`decisions/0001-model-granularity.md:9 / :71-72` 复算表。
- **版本**：`packages/desktop/package.json:3` `0.7.2`。

### 事实校准（proposal / spec 措辞 vs 工作树；不改需求意图，只定实现形态）

1. **`to_file_value` 的返回位是 `Result`，不是 `Value`**。spec Module Contract 行写 `(&WorkspaceConfig, &Value) -> Value`，但同一 requirement 的 scenario「键冲突显式失败」要求 `Err`。表行按简写读：定稿签名 `-> Result<Value, String>`（DD10）。归档合并时该表行随正文校正。
2. **只读模式的「组装结果 + 诊断逐条」无处安放**。explore §3.2 的使用指引提到只读模式返回「盘面 + 诊断逐条」，而 D3 把输出锁成 **四键**（`current` / `candidate` / `changes` / `summary`）。定稿：诊断**折进 `summary`**（条数 + 前若干 `path`），不新增第五键（DD4 / 待决问题 1）。
3. **proposal 的公共面清单缺一处读取入口，且必须补**。`current` 的取法（D4）需要「盘上原文」既作 `assemble` 输入又作 `to_file_value` 的模板，而 `config::load` 只出报告、不出原文。AGENT.md 硬规「工作区配置只经 `config` crate 读」禁止在 `infra/agent` 自行 `fs::read` + `layout::config_path` ⇒ 新增 `config::load_file_face(root)`（`FileFace` 的替代形态：`Result<(Value, Vec<ConfigDiagnostic>), String>`）。同时把 `load` 的四分支读取抽成**私有** `read_document`，两者共用（`load` 行为零变化，DD5）。
4. **提案卡拿不到 `root` / `onApplied`，而两条「加 props」的路都被封死**。卡片级动作面在仓内只有一条通道（`askAnswer`：`AgentSessionView` → `AgentMessages` → 卡片），而它的起点是 `SessionPanel` 的**硬编码字面量**（`session-panel.tsx` 里 `askAnswer={{ onAnswer: state.session.answer }}`），且该文件被 `desktop-change-chat` 的 Module Contract 锁定为**零改动**（`SessionPanelState` 不得加字段）。深层导入（`@/components/agent/message-cards/…`）被 `desktop-frontend-module-entry`「11 个入口的调用点 SHALL 经入口」禁止。定稿：React context（新文件 `message-cards/config-apply-scope.tsx`）+ barrel **第 8 个导出名**（DD12）。
5. **barrel 新增导出名会碰到 `api = declared` 元素**：`App.Frontend.Components.Agent` 的契约把 7 个名字全留白（无 `api-op` / 无 `api-reexport` / 无 `$defs`），故第 8 个名字**不写 fence**、只更新契约散文（职能计数 + 公共面表一行）。新文件本体不是元素（`message-cards/user-message.tsx` 同类先例：目录归属 `App.Frontend.Components.Agent`，不单建元素），与 D15「配置文件不新建前端元素」同向。
6. **业务代码调用姿态与既有偏离**：`use-archi-chat.ts:79` 直调 `commands.listArchiChatSessions(...)`，与 `desktop-ipc-runtime-guard`「业务代码 MUST NOT 直调 `commands.<name>`，一律经 `ipc`」冲突（既存偏离，**本变更不修**——不在范围内且会牵动 archi 面板用例）。新代码一律经 `ipc`：`ipc` 是 `commands` 的 Proxy，新命令**零改动**即覆盖（该 spec 的显式预期）。
7. **`session-panel.tsx:14` 的注释写「两处面板的公共状态子集」**：本变更后是三处。该文件被锁定零改动（spec 明文），此处仅登记口径，不随手改（待决问题 4）。
8. **`write_atomic` 的临时后缀 `.archi-tmp` 被三处测试断言**（`crates/core/archi/tests/golden_test.rs:410`、`src/commands/archi/mod_test.rs:492 / :503`）：搬移取**原样搬**（后缀逐字保留，注释说明其为通用原子写的临时后缀），使三处断言零随动（DD9）。

---

## 关键设计决策（DD1–DD15）

> proposal 的 D1–D16 与 explore 的 C1–C24 已定稿，本篇不重复。下表是本设计在 spec 已钉语义之外**新增的形状裁决**——每条都含「为什么另一种写法会坏」。

| # | 议题 | 定稿 | 理由 |
|---|---|---|---|
| DD1 | config 臂在 `assemble_injections` 中的落点与形态 | 在 archi 臂之后、`if provenance.source != CHANGE_SOURCE` 兜底之前插入第三段：`if provenance.source == CONFIG_SOURCE { const CONFIG_TOOLS: [&str; 6] = ["read", "grep", "glob", "ls", "bash", mount::PROPOSE_CONFIG_TOOL]; return injected(&CONFIG_TOOLS, crate::config_knowledge::knowledge(), None, Some(agent::AskMode::Generic), provenance); }`；新增 `const CONFIG_SOURCE: &str = "config"`（与命令层 `CONFIG_CHAT_SOURCE`、前端 `use-config-chat` 同字面量，三处注释互指） | 复用 `injected` = 成员校验与名字清单落地只有一份实现；`const` 数组允许 `mount::PROPOSE_CONFIG_TOOL` 参与（常量表达式）；**位置必须在兜底之前**——兜底是「非 change 即默认构造」，config 臂若放在其后就是死代码 |
| DD2 | 知识生产者形态 | `config_knowledge.rs` 与 `archi_knowledge.rs` 同构三件：`SKELETON = include_str!("config-knowledge.md")`、`knowledge() -> Option<String> = build(SKELETON).ok()`、`build(skeleton: &str) -> Result<String, String>`（骨架 trim 空 / 含 `<>` 即 `Err`，否则 `format!("{}\n\n<派生段>", skeleton.trim_end())`）。派生段两行：`config.json 顶层已知键：<known_keys 八键>`、`tests[].framework 枚举：<TestFramework::ALL 八值>`。体量上限 8 000 B（测试锚定） | 「派生段由代码产出」是词表唯一真相源：新增一个 `KNOWN_KEYS` 条目或一个 `TestFramework` 变体而骨架未改，知识随即含该词；把词表改成手写字面量则派生段测试判红。`build` 公开到 crate 内是为让「拼装失败降级」可测（`build("")` / `build("<>")`），与 `archi_knowledge::build` 同形 |
| DD3 | `propose_config` 定义要点 | 常量与定义**驻执行体模块**（`sdk/config_propose.rs`：`PROPOSE_CONFIG_TOOL` + `definition() -> ToolDefinition`），`mount.rs` 只 `pub(crate) use` 该常量并以 `config_propose::definition()` 挂第 14 项。参数面 `{"type":"object","properties":{"candidate":{"type":"object","description":"…"}},"required":[]}`；描述逐句写明两模式（不给候选 = 返回当前盘面；给候选 = 校验回环，`is_error` 后按逐条诊断修正重交**完整**候选）+「候选是整份文件面（`static_analysis` / `write_protection` / `depends_on` 用 snake_case）」 | 常量与定义同源一处，避免「名字在 mount、描述在 exec」两处分叉；`required: []` 是「候选缺省 = 只读模式」在参数面的唯一表达；**无 `path` 参数**（D1）——描述里必须点明「无路径参数，盘面由服务端按仓根解析」，否则模型会自造 `path` 键 |
| DD4 | `propose_config` 执行体三段与输出 | ① 读盘面：`config::load_file_face(&self.root)?` → `(current, diagnostics)`；② 分支：`input["candidate"]` 缺失 / `null` → 只读模式（`candidate = null`、`changes = []`、`summary` 携诊断摘要）；给出 → `config::validate(candidate)` 过滤 `InvalidValue`，**任一命中即 `Err`**（`"{path}: {message}"` 逐条 + 尾行「修正后重交完整候选（一次全量，不接受片段）」）；③ 干净候选 → `normalized = config::to_file_value(&report.config, candidate)?`、`changes = diff(&current, &normalized)`、`summary = "候选合法，共 N 处变更"` / `"候选合法，与当前配置一致"`；最终 `serde_json::to_string(&json!({...}))` 后过 `tools::cap_result` | **输出的是归一化候选而非原文回显**：使用户点「应用」写进盘的内容 == 校验过的内容 == 卡片展示的内容（三段同源），且模型即便误用 camelCase 也会在输出面被折成 file face；`Err` 与 `Ok` 都零副作用（只读盘） |
| DD5 | `current` 的取法与 `load` 的重构 | 新增 `pub fn load_file_face(root: &Path) -> Result<(Value, Vec<ConfigDiagnostic>), String>`：内部与 `load` 共用私有 `fn read_document(root: &Path) -> Result<Map<String, Value>, Vec<ConfigDiagnostic>>`（`NotFound` → `FileMissing`；读失败 → `ReadFailed`；语法错 → `JsonInvalid`；顶层非对象 → `InvalidValue($)`；四者都是 `Err(diags)` 且 `load` 与 `load_file_face` 各自按原语义收口）。成功臂：`assemble(&map)` → `to_file_value(&config, &Value::Object(map))` | `load` 的四分支语义被抽走而**输出逐字不变**（`Err(diags)` 分支不调 `assemble`，故顶层非对象仍恰一条诊断——若让非对象也走 `assemble`，会凭空多出 `schema` / `tests` 的 `defaultApplied`，是行为变化）；`config.json` 文件名与路径仍在 crate 内（`config_path`），infra 零 `fs` 读取 |
| DD6 | `changes` 的粒度与序 | 私有 `fn diff(current: &Value, candidate: &Value) -> Vec<Change>`：**对象递归**（键并集，按 `BTreeMap` 字母序），**数组与标量作叶子**（整体替换，不逐下标）；每项 `{path, kind, before, after}`，`kind ∈ {added, removed, changed}`，`path` 用点分 file-face 键路径（顶层即键名，无 `$` 前缀） | 数组逐下标会因「插一条 suite」把后面全部项标成 changed（噪声灌满卡片、且 `changes == []` 判据失效）；对象递归让「加了 `projects[].language`」这类真变化一眼可读。字母序是 `serde_json::Map = BTreeMap` 的直接结果，**不额外排序**（顺序即事实，卡片零重排） |
| DD7 | `ConfigContext` 形态与铸造判据 | `pub(crate) struct ConfigContext { root: PathBuf }`（`new` / `execute` 与 `ArchiContext` 同形）；`pub(crate) fn mint_config_context(names: &[String], workspace_root: Option<&Path>) -> Option<ConfigContext>`：判据 = 名字清单含 `PROPOSE_CONFIG_TOOL`，`root` = `workspace_root.map(ConfigContext::new)`。`session_pump` 调用点：`mint_config_context(open.injections.tools.as_deref().unwrap_or_default(), payload.workspace_root.as_deref())` | `mint_archi_context` 收 `&Path` 是因为它用 `open.ctx.workspace_root`（恒有值）；本函数**必须**收 `Option<&Path>`——它的语义是「取 payload 的主仓 root」，而 `payload.workspace_root` 是 `Option<PathBuf>`。**MUST NOT 改成 `open.ctx.workspace_root`**：那是 exec root（change 会话为 worktree），配置权威在仓根（C8/AC-3） |
| DD8 | 写面命令三段与私有 `check_root` | `workspace_config_apply_inner(root, candidate)` = `check_root(root)?`（**config 轨道私有 fn**：空白 / 缺失 / 不可读 / 非目录四文案，与 `commands/config` 既有 `workspace_config_inner` 的 metadata 口径同族）→ `config::plan_apply_config(root, candidate)?` → `commands::atomic::write_atomic(&plan.file, &plan.content)?` → `config::load(root)` → `WorkspaceConfigReport` 字段平移 | 不搬 `archi` 的 `check_root`（C16/D9：archi 的两条 root 文案已分叉，强行统一会改既有断言）；`plan_apply_config` 失败时 `write_atomic` 根本不在调用链上 ⇒ 「失败零写入」是结构性的，不靠一条 `if` |
| DD9 | `write_atomic` 搬移形态 | 新文件 `src/commands/atomic.rs`，`pub(crate) fn write_atomic(path: &Path, content: &str) -> Result<(), String>`，**函数体逐字搬**（含 `{name}.archi-tmp` 临时名与两条失败清理）；`commands/mod.rs` 增 `pub(crate) mod atomic;`；`commands/archi/mod.rs` 删本文件实现、改为 `use crate::commands::atomic::write_atomic;` | 后缀保留 `archi-tmp` 让三处既有断言（golden_test 的「无残留」与 mod_test 两处过滤）零随动；改名为通用 `.tmp` 只会让「临时文件不得残留」的既有机械判据失效一轮（见事实校准 8） |
| DD10 | `to_file_value` 算法与递归裁剪 | 五步结构变换：`serde_json::to_value(config)` → 递归 `rename_in_place`（三处多词键，**递归全层级**，`dependsOn` 在 `projects[i]` 内）→ `expand_extra`（`extra: [{key,value}]` 逐条插入顶层，**键冲突即 `Err`**（附冲突键），null 值条目照插）→ `integerize`（`f64.fract() == 0.0` 且 `abs <= 2^53` → `i64`，否则保浮点；递归）→ `trim(normalized, template)`。`trim` 规则：两边都是对象 → 只保留 `template` 出现的键、值递归；两边都是数组 → 按**下标配对**（`template[i]` 在场则递归，缺席则整值保留）；其余 → 取 `normalized` 侧 | 递归裁剪是「模板驱动」的唯一可实施解释：只裁顶层会让最小候选（`{schema, tests:[{root,framework}]}`）写出整套 suite 默认字段（= 变相「写全」），与 D6 的「不设第二形态」正面冲突。数组按下标配对而非按值匹配：candidate 即 template 时下标恒对齐，越界分支只为「template 短于值」的极端形态留一条不丢信息的兜底。结构变换而非手拼字段名清单，是「新增字段自动出线」的唯一实现（P1/P2 的自证前提） |
| DD11 | 数值整数化与 `extra` 三坑的实现位置 | `integerize` 是独立递归函数，**先于 `trim`**（裁剪只删键不改值，顺序对结果等价，但先整数化让 P4 的「字面量往返逐字相等」不被裁剪分支干扰）；`expand_extra` 先于 `integerize`（否则 `extra` 里的 `90.0` 不会被折成 `90`——`extra` 的原文保真是 `Value` 级，不归整数化）；`extra` 冲突判定用 `Map::contains_key`，命中即 `Err(format!("extra 键与已知字段冲突：{key}"))` | 先在值域做整数化再裁剪，使「P4 对全字段字面量往返」只剩一层判据；`extra` 走整数化会让「用户写 `1.0`」变 `1`（无损但改变字节），而 `extra` 是 passthrough 语义——**保真优先于规整**，故 `expand_extra` 之后再整数化只作用于「结构体字段派生值」，`extra` 值不被二次加工（实现上：先展开再整数化会让 `extra` 值也被扫到，故整数化**只作用于展开前的结构体侧**，即顺序为 rename → integerize → expand_extra → trim） |
| DD12 | 前端写回动作面形态 | 新文件 `components/agent/message-cards/config-apply-scope.tsx`：`interface ConfigApplyActions { root: string; onApplied: () => void }`、`ConfigApplyScope({value, children})`（Provider 薄封装）、`useConfigApplyActions(): ConfigApplyActions \| null`。`ConfigProposeCard` 内部消费（缺席 = 只读卡：应用按钮禁用 + 文案「应用只在配置页可用」）；`ConfigChatPanel` 以 `{root, onApplied}` 包裹 `SessionPanel`。barrel `components/agent/index.tsx` 增一条值导出（第 8 名） | 三条备选全被既有条款封死（校准 4）；context 是唯一「不动骨架 props、不动 `SessionPanelState`、不深层导入」的通道。缺席降级而非抛错，使 `tool-cards.test.tsx` 的既有渲染面（不包 provider）零改动即可跑 |
| DD13 | 前端调用与刷新姿态 | 清单与写回**一律经 `ipc`**（`ipc.listConfigChatSessions` / `ipc.workspaceConfigApply`）；应用成功后**忽略命令返回的报告**、统一经 `onApplied()` → `useWorkspaceConfig().refresh()` 重取 | `desktop-ipc-runtime-guard` 明文要求业务代码经 `ipc`（校准 6）；页面的取数路径只有 `useWorkspaceConfig`（含 `dataRoot` 防串根与 `cancelled` 防串轮），若把 apply 返回的报告直接塞进页面 state，就出现「两条写 `data` 的路径」＝第二真相源 |
| DD14 | 卡片五态的判据实现 | 判据全部落在 `ToolResult` 形态上：`output === null` → 进行中；`output.isError` → 失败（诊断原文 `<pre>`）；否则 `JSON.parse(output.content)` 失败 → 原文降级；`candidate === null` → 只读模式（呈盘面摘要）；否则可应用（`changes.length === 0` → 按钮 disabled +「与当前配置一致」）。`applying` / `applyError` / `applied` 三个局部态只承载**本卡片的**写回过程 | 五态互斥且判定只看 payload，满足「前端零配置语义」（不读 `KNOWN_KEYS`、不重算 diff）。`changes.length === 0` 是唯一「无差异」判据（不比较 `current` 与页面数据——那是跨时刻比较，C19 禁止） |
| DD15 | 架构模型资产形态 | `06-frontend.toml`：① 表头注释 `# 06 · 前端（30 元素 / 71 关系）` → `30 元素 / 72 关系`（元素计数不动——本变更零新元素）；② 在 `Views.Config` 的既有两条边之后追加一条 `[[relation]] from = "App.Frontend.Views.Config" to = "App.Frontend.Components.Agent"`，`desc = "经共享面板骨架渲染常驻 config 会话面板"`。`decisions/0001-model-granularity.md` §9：`前端 71 条` → `72 条`、`06(71) = 120` → `06(72) = 121`；首段「基线为 53 元素 / 120 关系」同步为 `53 / 121`。**既有漂移（实读 54 元素 / 122 关系、`06` 段 31 / 73）不修**，也不改 `repo-archi-model` / `desktop-frontend-module-entry` 的计数复述（D16 / R8） | 按 `repo-archi-model`「漂移纪律」只补**自己新增的那一条**，使「本变更的增量」与「历史欠账」在 diff 上可分离；关系证据是 `views/config/components/config-chat-panel.tsx` 与 `config-view.tsx` 的 `import … from '@/components/agent'`（目标归最具体已声明元素 = 父 barrel）；desc 不写「复用 SessionPanel」之外的实现细节（模型 relation 只记语义）。**barrel 自己新增的那条 re-export（`./message-cards/config-apply-scope`）不产生新边**——该文件归 `App.Frontend.Components.Agent` 的目录 `path`（与 `message-cards/user-message.tsx` 同形），`scan` 记下的 import 事实目标即本元素自身 |

---

## 架构组件

### 1. 配置协议公共面（`crates/core/config`）

- **职责**：把今天只有 `load` 一个公共函数的 crate 扩成「读 / 校验 / 文件面投影 / 写面规划」四面，且四面共用同一份词表与同一套校验。
- **文件位置**：`crates/core/config/src/lib.rs`（唯一源文件，零新文件）。
- **依赖**：既有 `foundation`（`layout::config_path`）+ `serde` / `serde_json` / `specta`，**零新增**。
- **技术**：单遍组装（既有 `assemble` 零改）+ 结构变换（`to_value` → 三次递归重命名 → 整数化 → `extra` 展开 → 模板裁剪）+ 纯函数零 IO。

### 2. 文件面写入器 `to_file_value`（`crates/core/config`）

- **职责**：把「永远合法的 `WorkspaceConfig`」投影成 config.json 的文件面字节语义（DD10），并让「写入器 / `KNOWN_KEYS` / `config.schema.json` / 组装层」四面被 P1–P4 钉在同一事实上。
- **文件位置**：同上（`lib.rs` 内新增私有 helpers：`rename_in_place` / `integerize` / `expand_extra` / `trim`）。
- **依赖**：`serde_json::{Map, Value}`；零新依赖。
- **技术**：不可变输入 + 单次可变中间件（`to_value` 产物），失败经 `Result<_, String>` 出（键冲突）。

### 3. 提案工具 `propose_config`（`crates/infra/agent/src/sdk/config_propose.rs`）

- **职责**：config 会话唯一的提案出口：读盘面、校验候选、算字段级差异、输出单条 file-face JSON；零落盘、零落库、零会话状态改动。
- **文件位置**：`crates/infra/agent/src/sdk/config_propose.rs`（新）+ `sdk/mod.rs`（模块登记）+ `sdk/mount.rs`（第 14 项挂载与 `pub(crate) use`）+ `sdk/tools.rs`（`input_path` 白名单）。
- **依赖**：`config`（已依赖）、`foundation`（经 `config` 间接；本文件零路径拼装）、`serde_json`。
- **技术**：同步执行体（`ArchiContext::execute` 同族：非 async、盘面读取一次、结果过 `cap_result`），`ConfigContext { root }` 由 `session_pump` per-turn 铸造。

### 4. 注入臂与知识 preamble（`crates/infra/agent/src/compose.rs` + `config_knowledge.rs` + `config-knowledge.md`）

- **职责**：`source = "config"` 的会话拿到六名工具清单（只读探测 + 唯一提案出口）与一段静态协议知识；知识拼装失败只丢 `preamble` 位。
- **文件位置**：`compose.rs`（第四臂 + `CONFIG_SOURCE`）、`config_knowledge.rs` + `config-knowledge.md`（新）。
- **依赖**：`config::{known_keys, TestFramework::ALL}`（词表派生）、`foundation::layout`（**不需要**——知识不含路径，见 DD2）。
- **技术**：`include_str!` 编译期内嵌 + `build(skeleton)` 派生段拼接；`Option<String>` 降级面。

### 5. per-turn 上下文与 loop 分派（`crates/infra/agent/src/sdk/{runner,loop}.rs`）

- **职责**：把「主仓 root」铸成 `ConfigContext` 随轮携带，`ToolExec::Config` 分派到执行体；上下文缺席以 `is_error` 回灌。
- **文件位置**：`sdk/runner.rs`（`mint_config_context` + `session_pump` 调用点 + `LoopTurn` 装配）、`sdk/loop.rs`（`LoopTurn.config` 字段 + 分派臂）。
- **依赖**：既有 `PumpPayload.workspace_root`（零字段新增）。
- **技术**：与 `mint_archi_context` / `mint_db_context` 并列的名字清单判据；`LoopTurn` 内是「装配点算好」的载荷。

### 6. 写面命令轨道（`src-tauri/src/commands/{config,atomic}/`）

- **职责**：`workspace_config_apply` 三段（root 检查 → 零 `InvalidValue` 门 → 原子写 + 重读报告）；`write_atomic` 收为命令层共用的唯一落盘点。
- **文件位置**：`src-tauri/src/commands/config/mod.rs`（扩）、`src-tauri/src/commands/atomic.rs`（新）、`src-tauri/src/commands/archi/mod.rs`（改引用）、`src-tauri/src/commands/mod.rs`（`pub(crate) mod atomic;` + `all_commands!` 登记两条命令）。
- **依赖**：`config`（`plan_apply_config` / `load`）、`std::fs`（临时文件 + rename）。
- **技术**：`spawn_blocking` 不需要（写面是毫秒级本地写，与 `archi_set_gate` 同姿态、同线程模型）。

### 7. 清单命令（`src-tauri/src/commands/exec/mod.rs`）

- **职责**：`list_config_chat_sessions(root, offset, limit)` 薄包装（`source` 词常量收本层、blank root → 空页）。
- **文件位置**：同上（`ARCHI_CHAT_SOURCE` 旁新增 `CONFIG_CHAT_SOURCE`）。
- **依赖**：`agent_runtime::session_query(store).list_sessions(...)`。
- **技术**：与 `list_archi_chat_sessions` 逐行同形（`source_ref = None`）。

### 8. 前端会话面板与提案卡（`packages/desktop/src`）

- **职责**：`/config` 左栏常驻 config 会话（骨架复用 + 归属 + 禁发矩阵 + 状态行），右栏既有分区 + 显式刷新；提案卡五态 + 唯一写回入口。
- **文件位置**：`views/config/hooks/use-config-chat.ts`（新）、`views/config/components/config-chat-panel.tsx`（新，含私有 composer）、`components/agent/message-cards/config-apply-scope.tsx`（新）、`components/agent/message-cards/tool-cards.tsx`（扩）、`components/agent/index.tsx`（第 8 个导出）、`views/config/config-view.tsx`（改）。
- **依赖**：`components/agent`（barrel：`SessionPanel` / `AgentInput` / `ConfigApplyScope`）、`components/ui`（`ResizablePanelGroup` / `ResizableHandle` / `ResizablePanel`）、`ipc`（`listConfigChatSessions` / `workspaceConfigApply`）。
- **技术**：React 19 函数组件 + `useAgentSession` 注入形态 + React context（卡片级动作面）+ `@tanstack/react-virtual`（既有对话体，无需改动）。

---

## 变更清单

> **覆盖口径**：三张主表覆盖 proposal「变更范围 - 实现文件」全部 17 行，并补 5 处设计层新增（`load_file_face` 的公共面、`infra/agent/src/lib.rs` 的模块登记、`message-cards/config-apply-scope.tsx`、`components/agent/index.tsx` 的 barrel 导出、`App.Frontend.Components.Agent` 契约散文），逐条在「事实校准」与 DD 中给出成因。测试文件的期望随动（第 14 项计数锚 / 结构断言 / 新增矩阵）**不属本相位改动**，见末尾「交接给下游相位的随动清单」。

### 新增文件

| 文件 | 说明 |
|---|---|
| `packages/desktop/src-tauri/crates/infra/agent/src/config_knowledge.rs` | 静态 preamble 生产者：`SKELETON = include_str!("config-knowledge.md")`、`knowledge()`、`build(skeleton)`（DD2）；对应条目见「公共函数」表 |
| `packages/desktop/src-tauri/crates/infra/agent/src/config-knowledge.md` | 静态骨架：字段语义与值域（八键）、阈值 0–100、`tests[].root` / `projects[].root` 路径禁令、`projects[].language` / `depends_on` 软规则、`context` / `rules` 零工程消费、`defaultApplied` 是正常形态、`config.json` 是 agent 不可写文件（事实陈述，判据归 `desktop-write-protection`）、`propose_config` 两模式使用指引；**零 `<>` 占位符、零词表手抄、零盘面真值** |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/config_propose.rs` | `propose_config` 常量 + 定义 + `ConfigContext`（`new` / `execute`）+ 私有 `diff` / `summary` 拼装（DD3 / DD4 / DD6） |
| `packages/desktop/src-tauri/src/commands/atomic.rs` | `pub(crate) fn write_atomic(&Path, &str) -> Result<(), String>`（原样搬自 `commands/archi/mod.rs:59`，DD9） |
| `packages/desktop/src/views/config/hooks/use-config-chat.ts` | 会话状态面 + 清单分页 + 新会话 / 历史切换 + root 复位 + 运行中禁发判据 + `note` 三态（照 `use-archi-chat.ts`，DD13 / DD14） |
| `packages/desktop/src/views/config/components/config-chat-panel.tsx` | 面板装配：`useConfigChat` + `ConfigApplyScope` 包裹 + `SessionPanel`（`testIdPrefix = "config-chat"`）+ 私有 composer（`AgentInput`，`idPrefix = "config-chat"`） |
| `packages/desktop/src/components/agent/message-cards/config-apply-scope.tsx` | 卡片级写回动作面：`ConfigApplyActions` / `ConfigApplyScope` / `useConfigApplyActions`（DD12） |

### 修改文件

| 文件 | 具体修改内容 | 说明 |
|---|---|---|
| `packages/desktop/src-tauri/crates/core/config/src/lib.rs` | ①**补模块 doc**（`//!` 段：两副面孔 wire / file、`null` 位语义、`KNOWN_KEYS` 与 `extra` 的口径、写面纪律「落盘只经 `to_file_value`」）——修掉 `assemble` doc 引用不存在的「模块 doc null 位语义」这一悬空引用；②`TestFramework` derive 增 `PartialEq, Eq` 并加 `ALL` / `as_str`；③`known_keys()`；④`validate(&Value) -> ConfigReport`；⑤抽私有 `read_document`，`load` 改经它（行为零变化，DD5）；⑥`load_file_face`；⑦`to_file_value` + 私有 `rename_in_place` / `integerize` / `expand_extra` / `trim`；⑧`ConfigWrite` + `plan_apply_config` | `assemble` / `assemble_suite` / `assemble_project` / `assemble_file_rule` / `opt_string` / `opt_string_list` / `threshold` / `KNOWN_KEYS` / 诊断文案**逐字不动**；`config.schema.json` 零改动（不新增键、不改枚举） |
| `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs` | ①新增 `const CONFIG_SOURCE: &str = "config"`（三处同字面量注释）；②`assemble_injections` 增 config 臂（DD1）；③模块 doc 与函数 doc 的「三臂」全部改「四臂」，兜底表述收窄为「非 change、非 config 来源（debug 等）」 | 签名不变、`injected` 与成员校验语义不变、explore / archi / change 三臂逐字不动 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mod.rs` | 增 `pub(crate) mod config_propose;`（`mod config;` 之后，保持字母序） | 模块登记单点 |
| `packages/desktop/src-tauri/crates/infra/agent/src/lib.rs` | `mod archi_knowledge;` 之后增 `mod config_knowledge;`（保持字母序）；`#[cfg(test)]` 区的 `mod config_knowledge_test;` 由 test-gen 相位随新用例文件补 | `config_knowledge.rs` 不登记则不参与编译，阶段六的 `crate::config_knowledge::knowledge()` 无从解析——该行与「新增 `config_knowledge.rs`」是同一条改动的两半 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs` | ①`pub(crate) use crate::sdk::config_propose::PROPOSE_CONFIG_TOOL;`；②`build_registry` 追加第 14 项（`definition: config_propose::definition()`、`tier: ToolTier::Read`、`exec: ToolExec::Config`）；③`Vec::with_capacity(13)` → `Vec::with_capacity(14)`；④两段 doc（`registry` 的「十三项」与 `build_registry` 的「十三个字面构造项按序」）随动；⑤`ToolExec` 增 `Config` 变体（doc：`propose_config`：`LoopTurn.config` 上下文） | `TOOL_NAMES` 七件与 `builtin()` 不动（`tools: None` 语义不变）；`finish_step` / `AskUserQuestion` 的追加分支零改动 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools.rs` | `input_path` 的无路径白名单补 `PROPOSE_CONFIG_TOOL`（`pub(crate) use` 列表同批带出该常量）；doc 补「四类专用工具均无路径参数」 | 否则候选里出现顶层未知键 `path` 会被 `sandbox_rewrite` 当工具路径改写（R5） |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs` | ①`use crate::sdk::config_propose::ConfigContext;`；②新增 `mint_config_context(names: &[String], workspace_root: Option<&Path>) -> Option<ConfigContext>`（doc 写明 root 取 `payload.workspace_root`、MUST NOT 取 `ctx.workspace_root`）；③`session_pump` 增铸造调用（`open.injections.tools.as_deref().unwrap_or_default()` + `payload.workspace_root.as_deref()`）；④`LoopTurn` 装配增 `config` | `mint_archi_context` / `mint_db_context` / `ProtectRules::load` 零改动；`PumpPayload` 零字段新增 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs` | ①`LoopTurn` 增 `pub config: Option<ConfigContext>`（字段 doc：per-turn 铸造、缺席以 `is_error` 回灌）；②`match mount.exec` 增 `ToolExec::Config` 臂（`Some(ctx) => ctx.execute(&input)`，`None` → `("专用工具执行上下文未装配", true)`，与 `Archi` / `Db` 同形） | 分派语义零改：沙箱闸 / 写保护闸 / 成员校验三条前置不变 |
| `packages/desktop/src-tauri/src/commands/config/mod.rs` | ①模块 doc 改「双命令轨道」（读 + 写）并写明「写回唯一入口 = 提案卡应用、不经 agent 工具面」；②私有 `fn check_root(root: &Path) -> Result<(), String>`（空白 / 缺失 / 非目录，四文案）；③`workspace_config_apply` + `workspace_config_apply_inner`（DD8） | `workspace_config` / `workspace_config_inner` 逐字不动 |
| `packages/desktop/src-tauri/src/commands/archi/mod.rs` | 删除私有 `write_atomic`（`:59-82`），改 `use crate::commands::atomic::write_atomic;`；`archi_set_gate_inner` 调用点不变 | 本文件其余命令与 `check_root` 零改动 |
| `packages/desktop/src-tauri/src/commands/mod.rs` | ①`pub(crate) mod atomic;`；②`all_commands!` 增 `commands::config::workspace_config_apply`（`config::workspace_config` 之后）与 `commands::exec::list_config_chat_sessions`（`exec::list_archi_chat_sessions` 之后） | 宏体是单一登记面，`ipc` 自动覆盖 |
| `packages/desktop/src-tauri/src/commands/exec/mod.rs` | ①`const CONFIG_CHAT_SOURCE: &str = "config";`（与 `ARCHI_CHAT_SOURCE` 相邻，注释指明三处同字面量）；②`list_config_chat_sessions(stores, root, offset, limit)`（照 `list_archi_chat_sessions`：blank root → 空页、`Some(CONFIG_CHAT_SOURCE)` + `None`） | 既有三条清单命令零改动 |
| `packages/desktop/src/components/agent/message-cards/tool-cards.tsx` | ①新增 `ConfigProposeCard`（五态 + 字段级 diff（沿用 `EditCard` 的 added / removed 双 `CodeBlockLine`）+「应用」+ 规范化明示，DD14）；②`BUILD_IN_TOOL_CARDS` 增 `propose_config: ConfigProposeCard`（第 8 键）；③文件头 doc 补「第 8 张卡与写回入口」 | `ConfigProposeCard` **不导出**（`ToolCards` 契约「卡组件不进公共面」零改动）；既有 7 卡零改动 |
| `packages/desktop/src/components/agent/index.tsx` | 增一条值导出 `export { ConfigApplyScope } from './message-cards/config-apply-scope';`（barrel 第 8 名，DD12 / 校准 5） | 只加值导出、不加类型导出（`ConfigApplyActions` 只被卡与服务侧同元素内消费，不外露） |
| `packages/desktop/src/views/config/config-view.tsx` | ①单列 → 横向分栏（`ResizablePanelGroup orientation="horizontal"`：`ConfigChatPanel` → `ResizableHandle` → `ResizablePanel minSize="40"` 承载既有分区）；②`onApplied={state.refresh}`；③页头 doc / `ConfigHeader` doc 改判（不再「只读页面」） | 五分区、`config-refresh`、错误 / 空态 / loading 三面与 `data-testid` 全保留 |
| `packages/desktop/src/types/generated/bindings.ts` / `bindings.zod.ts` | `bindings:export` 再生成：`workspaceConfigApply`（`candidate: unknown`）、`listConfigChatSessions` | **MUST NOT 手改**；diff 只应来自生成 |
| `.dev-team/architecture/graph/model/06-frontend.toml` | 表头注释 `71 关系` → `72 关系`；`Views.Config` 段后追加一条 `[[relation]]`（DD15） | 元素零新增、其余关系逐字不动 |
| `.dev-team/architecture/decisions/0001-model-granularity.md` | §9 复算：前端 `71` → `72`、`06(71) = 120` → `06(72) = 121`、首段基线 `120 关系` → `121 关系`（元素与其余分段不动） | 既有漂移（实读 54 / 122）**不修**（D16 / R8） |
| `.dev-team/architecture/models/Frontend/Components/App.Frontend.Components.Agent.md` | 职能段「6 条 re-export 透出 7 个名字」→「7 条 / 8 个名字」；公共面表增 `ConfigApplyScope` 一行（组件 / `message-cards/config-apply-scope.tsx` / 语义）；不写 `api-reexport` fence（7 个既有名字同样留白，成因在案） | 校准 5：barrel 契约是散文 + 留白形态，新增名只动散文 |
| `packages/desktop/package.json` | `version` `0.7.2` → `0.7.3` | `src-tauri/tauri.conf.json` 经 `../package.json` 跟随；`src-tauri/Cargo.toml` **不跟** |

### 公共函数 / API

| 标识符 | 所在文件 | 新增/修改 | 完整签名 | 说明 |
|---|---|---|---|---|
| `known_keys` | `crates/core/config/src/lib.rs` | 新增 | `pub fn known_keys() -> &'static [&'static str]` | `KNOWN_KEYS` 八键的只读出口（知识派生段与 P2 锚共用） |
| `TestFramework::ALL` | 同上 | 新增 | `pub const ALL: &'static [TestFramework; 8]` | 八值枚举清单（与 `parse_framework` 双向互证） |
| `TestFramework::as_str` | 同上 | 新增 | `pub fn as_str(&self) -> &'static str` | 值 → 文件面字面量（`"jest"` …）；`parse_framework` 复用同一 match 语义（双向互证） |
| `validate` | 同上 | 新增 | `pub fn validate(value: &Value) -> ConfigReport` | 任意 JSON 值 → 同一套 `assemble` 校验；顶层非对象落 `InvalidValue($)` + 全默认（与 `load` 的非对象分支同口径） |
| `load_file_face` | 同上 | 新增 | `pub fn load_file_face(root: &Path) -> Result<(Value, Vec<ConfigDiagnostic>), String>` | 盘面 file face 投影（D4 公式）+ 同一次装配的诊断；文件缺失 / 不可读 / 语法错 / 顶层非对象 → `(json!({}), 该条诊断)`；键冲突（不可达）→ `Err` |
| `to_file_value` | 同上 | 新增 | `pub fn to_file_value(config: &WorkspaceConfig, template: &Value) -> Result<Value, String>` | 模板驱动的 file face 投影（五步结构变换，DD10 / DD11）；`extra` 键冲突 → `Err`（附冲突键） |
| `plan_apply_config` | 同上 | 新增 | `pub fn plan_apply_config(root: &Path, candidate: &Value) -> Result<ConfigWrite, String>` | 纯函数写面规划：`validate` → 任一 `InvalidValue` 即 `Err`（诊断逐条）→ `to_file_value(&report.config, candidate)` → `to_string_pretty` + `"\n"` → `ConfigWrite`；零 IO |
| `write_atomic` | `src-tauri/src/commands/atomic.rs` | 新增（搬移） | `pub(crate) fn write_atomic(path: &Path, content: &str) -> Result<(), String>` | 临时文件 + rename 的原子写；命令层唯一 `fs::write` 位点（`archi_set_gate` 与 `workspace_config_apply` 共用） |
| `workspace_config_apply` | `src-tauri/src/commands/config/mod.rs` | 新增 | `pub fn workspace_config_apply(root: String, candidate: serde_json::Value) -> Result<WorkspaceConfigReport, String>` | Tauri 命令（`#[tauri::command]` + `#[specta::specta]`）薄包装；入参 file-face `Value`（出线 TS `unknown`） |
| `workspace_config_apply_inner` | 同上 | 新增 | `pub fn workspace_config_apply_inner(root: &Path, candidate: &serde_json::Value) -> Result<WorkspaceConfigReport, String>` | 离 Tauri 运行时可测的三段实现（DD8）；成功返回**写后重读**的报告 |
| `list_config_chat_sessions` | `src-tauri/src/commands/exec/mod.rs` | 新增 | `pub fn list_config_chat_sessions(stores: State<'_, WorkspaceStores>, root: String, offset: u32, limit: u32) -> Result<SessionPage, String>` | 薄包装：blank root → 空页；否则 `list_sessions(Some("config"), None, offset, limit)` |
| `knowledge` | `crates/infra/agent/src/config_knowledge.rs` | 新增 | `pub(crate) fn knowledge() -> Option<String>` | 静态骨架 + 派生词表 + 使用指引；零入参、零 IO；失败 `None`（只丢 `preamble` 位） |
| `build` | 同上 | 新增 | `pub(crate) fn build(skeleton: &str) -> Result<String, String>` | 拼装单点（骨架空 / 含 `<>` → `Err`）；供降级路径可测 |
| `ConfigContext::new` | `crates/infra/agent/src/sdk/config_propose.rs` | 新增 | `pub(crate) fn new(root: PathBuf) -> Self` | per-turn 铸造产物构造 |
| `ConfigContext::execute` | 同上 | 新增 | `pub(crate) fn execute(&self, input: &Value) -> Result<String, String>` | 两模式执行体（DD4）：只读返回盘面 / 候选校验回环；输出单条 file-face JSON 字符串（过 `cap_result`）；零副作用 |
| `PROPOSE_CONFIG_TOOL` | 同上 | 新增 | `pub(crate) const PROPOSE_CONFIG_TOOL: &str = "propose_config"` | 工具名常量（`mount.rs` 再导出，装配 / 白名单 / `input_path` 三处引用同一常量） |
| `definition` | 同上 | 新增 | `pub(crate) fn definition() -> rig::completion::ToolDefinition` | `{ candidate?: object }` + `required: []`（DD3）；挂载清单第 14 项的定义来源 |
| `mint_config_context` | `crates/infra/agent/src/sdk/runner.rs` | 新增 | `pub(crate) fn mint_config_context(names: &[String], workspace_root: Option<&Path>) -> Option<ConfigContext>` | 名字清单含 `propose_config` 且主仓 root 在场才铸（DD7） |
| `useConfigChat` | `packages/desktop/src/views/config/hooks/use-config-chat.ts` | 新增 | `useConfigChat(params: { root: string \| null }): ConfigChatState` | 会话状态面单源（`ConfigChatState extends SessionPanelState`，另含 `sessionId` / `currentSessionId` / `send` / `stop` / `canSend` / `disabledReason`）；归属三件（`source = "config"` / `sourceRef = null` / `cwd = null`）由本 hook 承担。**写回动作面不经本 hook**（待决问题 2） |
| `ConfigChatPanel` | `packages/desktop/src/views/config/components/config-chat-panel.tsx` | 新增 | `ConfigChatPanel(props: { root: string \| null; onApplied: () => void }): React.JSX.Element` | 面板薄封装：`useConfigChat` + `ConfigApplyScope`（`root` 非 null 时提供）+ `SessionPanel`（`testIdPrefix = "config-chat"`）+ composer 插槽 |
| `ConfigApplyScope` | `packages/desktop/src/components/agent/message-cards/config-apply-scope.tsx` | 新增 | `ConfigApplyScope(props: { value: ConfigApplyActions; children: React.ReactNode }): React.JSX.Element` | 卡片级写回动作面的 Provider 薄封装（barrel 第 8 个导出名） |
| `useConfigApplyActions` | 同上 | 新增 | `useConfigApplyActions(): ConfigApplyActions \| null` | 卡片侧读取（`null` = 无写回通道 → 卡呈只读态） |

> 私有函数不入本表（非公共 API）：`read_document`（`core/config` 读取单点）、`rename_in_place` / `integerize` / `expand_extra` / `trim`（写入器四步）、`diff`（`config_propose` 字段级差异）、`check_root`（`commands/config` 私有 root 校验）、`ConfigProposeCard`（注册表内部卡组件，不外露）。

### 类型定义

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|---|---|---|---|
| `ConfigWrite` | `crates/core/config/src/lib.rs` | 新增 | `pub struct ConfigWrite { pub file: PathBuf, pub content: String }`，`#[derive(Debug, Clone)]`；沿 `archi::edit::GateWrite` 的 `{ file, content }` 形状（不 derive serde / specta——它是写面内部产物，不出线） |
| `TestFramework` | 同上 | 修改 | derive 增 `PartialEq, Eq`（双向互证的比较前提）；变体与 `#[serde(rename)]` 八条逐字不动 |
| `ToolExec` | `crates/infra/agent/src/sdk/mount.rs` | 修改 | 增 `Config` 变体（`propose_config` 执行体分类；与 `Archi` / `Db` 并列的「上下文执行体」族） |
| `ConfigContext` | `crates/infra/agent/src/sdk/config_propose.rs` | 新增 | `pub(crate) struct ConfigContext { root: PathBuf }`；per-turn 铸造、随 `LoopTurn` 携带 |
| `ConfigApplyActions` | `packages/desktop/src/components/agent/message-cards/config-apply-scope.tsx` | 新增 | `interface ConfigApplyActions { root: string; onApplied: () => void }`（服务侧提供、卡片侧消费；**不经 barrel 导出**） |
| `ConfigChatState` | `packages/desktop/src/views/config/hooks/use-config-chat.ts` | 新增 | `interface ConfigChatState extends SessionPanelState { sessionId: string \| null; currentSessionId: string \| null; send: (input: { text: string; permissionMode: AgentPermissionMode }) => void; stop: () => void; canSend: boolean; disabledReason: string \| null }` |
| 提案卡载荷（**非 TS 类型**，只在卡片内宽容解析） | `tool-cards.tsx`（`ConfigProposeCard` 内） | 新增（解析面） | `{ current: unknown; candidate: unknown \| null; changes: Array<{ path: string; kind: 'added' \| 'removed' \| 'changed'; before?: unknown; after?: unknown }>; summary: string }`——由后端单源产出，前端只 `asRecord` / `textOf` 宽容读，**不建导出类型**（避免把内部载荷形状变成公共承诺） |

### 配置

<!-- 本变更零配置键改动：`KNOWN_KEYS` 八键、`TestFramework` 八值、`config.schema.json` 三者逐字不动（proposal「不要修改」明列）；新增能力不引入任何新配置项（会话来源是受控字符串常量、写面路径经 `foundation::layout::config_path`）。故本子表整段省略。 -->

---

## 数据模型

### 两副面孔与写入器的位置

| 面 | 键形 | 生产者 | 消费者 |
|---|---|---|---|
| wire（`to_value` / IPC / TS） | `staticAnalysis` / `writeProtection` / `dependsOn` / `extra: [{key,value}]` | `serde_json::to_value(&WorkspaceConfig)` | `workspace_config` 报告、页面分区渲染 |
| file（config.json） | `static_analysis` / `write_protection` / `depends_on` / `extra` 展开回顶层 | **仅** `config::to_file_value` | 落盘、`propose_config` 的 `current` / `candidate` |

四处不同名（其余单词键同形）；`$schema` 已有显式 `#[serde(rename)]`，两面同键。**任何落盘路径 MUST NOT 经 `to_value`**（R2：写保护规则静默失效 + 拓扑序安装退化，且现有自洽测试抓不到）。

### `ConfigWrite`（写面规划产物，不落库）

| 字段 | 类型 | 语义 |
|---|---|---|
| `file` | `PathBuf` | 目标文件绝对路径（`foundation::layout::config_path(root)`，crate 内零 `config.json` 字面量） |
| `content` | `String` | 已规范化字节：`to_string_pretty` + 尾换行（D8/C17） |

### 提案载荷（`propose_config` 的 OK 输出，恒单条 file-face JSON 字符串）

| 键 | 类型 | 只读模式 | 校验模式 |
|---|---|---|---|
| `current` | `Value`（file face） | 盘面投影（缺文件 `{}`） | 同左（与 `candidate` 同刻同源） |
| `candidate` | `Value \| null`（file face，`to_file_value` 归一化后） | `null` | 归一化候选 |
| `changes` | `Array<{path, kind, before, after}>` | `[]` | `diff(current, candidate)`（对象递归、数组作叶子、字母序，DD6） |
| `summary` | `String` | `"只读模式：返回当前盘面（诊断 N 条：a、b…）"` / `"…（无诊断）"` | `"候选合法，共 N 处变更"` / `"候选合法，与当前配置一致"` |

**关系**：`changes[i].path` 是 file-face 键路径（`static_analysis` / `projects` / `$schema` 一类），与 `current` / `candidate` 的键一一对位；`kind` 封闭三值。

**持久化**：候选**不落盘、不落库**——只活在会话转录（工具 input / output 已持久化，历史重放时卡片照常出现）。唯一落盘动作是用户点「应用」触发的 `workspace_config_apply`，其结果是一份规范化后的 `config.json`（键序字母序 + pretty 展开；`projects[i]` / `tests[i]` / `write_protection.files[i]` 内的未知子键在组装路径上丢弃——R3 已在卡片明示）。

### 会话归属（store 零迁移）

`source = "config"` / `source_ref = null` / `cwd = null`；`session.source` 保持自由字符串语义（无枚举 / 无 CHECK / 无注册模型）；`list_config_chat_sessions` 以精确 `source` 过滤，与 `archi` / `change`（两段） / change 流程（四段）互不相撞。

---

## 路由 / API 设计

| 通道 | 名称 | 说明 | 输入 | 输出 | auth |
|---|---|---|---|---|---|
| Tauri IPC | `workspace_config_apply` | 写回唯一入口（用户点击提案卡触发） | `root: string`（工作区/主仓根）、`candidate: unknown`（file-face 整份候选） | `WorkspaceConfigReport`（写后重读）；失败 `Err(String)`（逐条诊断或 root / 写失败文案） | 本地 IPC，无鉴权；agent 工具面**不可达**（不在任何 `<tool>` 定义内） |
| Tauri IPC | `list_config_chat_sessions` | config 会话清单（探测语义薄包装） | `root: string`、`offset: number`、`limit: number` | `SessionPage`（`sessions: SessionSummary[]` / `total: number`）；blank root → 空页 | 本地 IPC，无鉴权 |
| agent 工具 | `propose_config` | config 会话唯一提案出口（只读） | `{ candidate?: object }` | `Ok`：file-face JSON 字符串（四键）；`Err`：逐条 `path: message` + 修正指引 | 仅 `source = "config"` 会话下发；`ToolTier::Read`三档全放行；无路径参数 |

> `/config` 路由与 nav 入口既在（`desktop-page-routing` 零改动），本变更只改页面内部形态。

---

## 依赖

| 类别 | 依赖 | 用途 |
|---|---|---|
| 运行时 | 无新增 | Rust：`config` / `serde_json` / `foundation` 均已是 `agent-runtime` 与 shell 的既有依赖；前端：`react` / `react-resizable-panels` / `lucide-react` 均既有 |
| 构建 / 测试 | 无新增 | 沿用 `cargo` / `pnpm` / `bindings:export` / `cargo test --workspace` 既有链；不新增 crate、不新增 npm 包、不新增 `Cargo.toml` 依赖条目 |

---

## 待决问题

1. **只读模式的诊断信息只有一个落点**：D3 锁死四键，故诊断（`defaultApplied` / `invalidValue` 逐条）被折进 `summary` 的单行文本。若评审要求模型能读到完整的 `path` 清单，出路只有「加第五键」（破 D3）或「让模型改用 `read` 自己看文件」（现状可选）。本设计按 D3 收口。
2. **`useConfigChat` 的入参少了 `onApplied`**：spec Module Contract 行写 `(params: { root; onApplied })`，本设计让 `onApplied` 落在 `ConfigChatPanel` 并由 `ConfigApplyScope` 下发——会话 hook 不持有写回动作（否则 hook 既是会话状态面又是写面触发器）。归档合并时该表行随正文校正。
3. **barrel 第 8 个导出名对 `api:check` 的读数**：契约 md 该元素的 7 个名字全留白（无 `api-reexport` fence），本变更只补散文。若 `api:check` 报出新的 `apiUndeclared`（该元素 `gate = "off"`，按十三值判据不纳管门禁），登记在案即可；**不得**为了让报告变绿而手写 `api-reexport` fence（会连带生成物 `.i.ts` 变化，与 AC-9「生成物 diff 只来自 `bindings:export`」冲突）。
4. **`session-panel.tsx:14` 注释「两处面板」已过期**：该文件被 spec 锁定零改动，本变更不改这行注释。若评审要求事实一致，属独立治理变更（一行 doc 注释，不涉行为）。
5. **应用成功后按钮是否禁用**：本设计不禁用（写面幂等、卡片保留可重试语义），仅在成功后呈现「已应用」弱化标注。若评审要求「应用后卡死」，需在卡片加 `applied` 终态（与「多次 propose 各卡独立可应用」不冲突，但会让「同一卡二次应用」不可达）。
6. **`extra` 值的整数化边界**：`extra` 是 passthrough（保真优先），故整数化只作用于结构体派生值（DD11）。若将来用户期望「`extra` 里的 `1.0` 也折成 `1`」，那是 `extra` 语义变更，须独立裁决。

---

## 验收标准落点

| AC | 落点 |
|---|---|
| AC-1 config 臂逐名 / `ask = Generic` / `finish = None` / `preamble` 非空 / 无写与 db 工具 / 未知名 `Err` | DD1（`injected` 复用 ⇒ 成员校验同源）+ 组件 4；`RunStarted.tools` 由 `definitions` 单源上报（既有机制，零改动） |
| AC-2 `propose_config` 三面行为（只读 / 脏 `Err` 零副作用 / 干净 `Ok` file face / `<50KB`） | DD3 + DD4（输出归一化候选、`Err` 早退于读盘之后零写）+ 组件 3；体量由 `cap_result` 上限与满配 fixture 用例守 |
| AC-3 `ConfigContext.root` 取主仓 root | DD7（`payload.workspace_root` 为唯一来源；`Option<&Path>` 形参使「取 ctx root」在类型上就写不出来）+ 组件 5 |
| AC-4 知识零入参零 IO / 词表派生 / 零占位符 / 体量上限 / 失败只丢 preamble 位 | DD2（`build` 的 `Err` 面 + 派生段两行）+ 组件 4 |
| AC-5 写面门（零 `InvalidValue` 才落盘、失败零写入双证、成功返回重读报告、入参 file-face `Value`） | DD8（三段结构：`plan_apply_config` 失败时 `write_atomic` 不在调用链上）+ 公共面 `workspace_config_apply(_inner)` 签名（typed `WorkspaceConfig` 在签名上不出现） |
| AC-6 P1–P4 与整数化 / `extra` 三坑 | DD10 + DD11（五步顺序、递归裁剪、冲突 `Err`、`extra` null 保留与不二次整数化） |
| AC-7 卡片五态 + diff 基准 + 多次提案独立 + 应用后重取一次 | DD14（判据矩阵）+ DD12（动作面通道与缺席降级）+ DD13（刷新经 `refresh()` 单路）+ 组件 8 |
| AC-8 spec 改判零残留冲突 | 本变更的五份 delta 已逐处 MODIFIED（proposal §能力）；实现相位以「只读 / 两处 / 唯一生产者 / 三臂 / 其余非 change」全文检索逐处复核（无代码侧落点） |
| AC-9 全管线绿 / 生成物零手改 / 版本口径 | DD9（`write_atomic` 原样搬 ⇒ 三处既有断言零随动）+ DD13（`ipc` 姿态）+ 变更清单末行（`0.7.3`）；门禁与测试回归由桌面端相位执行 |
| AC-10 架构模型同步（一条新边 + 可解析 + 表头 / ADR 同值） | DD15（新边 + 表头 `30/72` + ADR §9 复算；元素零新增；既有漂移不修）+ 组件 8 的 import 证据 |

> **验收面口径**（与 proposal 一致）：「检测出什么」**不可断言**——MUST NOT 以「该仓库应被识别为 N 个项目 / 依赖是 A ← B」作为验收条；验收只锚四类事实：注入臂装配、`propose_config` 契约矩阵、写面（门 + 失败零写入 + 写入器 P1–P4）、卡片五态。

---

## 交接给下游相位的随动清单（非本相位改动）

> 下列文件在实现后**会因机械锚变化而需要随动**，其用例与断言归 test-gen / test-execution 相位；本相位既不写用例、也不改断言。

| 文件 | 随动点 |
|---|---|
| `crates/infra/agent/src/sdk/mount_test.rs` | `EXPECTED_NAMES` 13 → 14（末项 `propose_config`）、`registry` 计数断言、三档等价表 |
| `crates/infra/agent/src/sdk/policy_test.rs` | 三档等价表把 `propose_config` 归 `[true, true, true]`；名字两两不同用例计数 |
| `crates/infra/agent/src/compose_test.rs` | 「`fn assemble_injections` 恰一处」与「逐来源下发面与装配单源一致」两处结构断言（新增 config 臂不改判据，只加用例）；新增 config 臂用例 |
| `src/commands/archi/mod_test.rs` | 「`fs::write` 恰一处」由 `archi/mod.rs` 源码扫描改指 `commands/atomic.rs`（DD9 的落点随动） |
| `src-tauri/src/bindings/mod_test.rs` | 新命令出线断言（`workspaceConfigApply` / `listConfigChatSessions`） |
| `crates/infra/agent/src/lib.rs` / `crates/infra/agent/src/sdk/mod.rs` / `src-tauri/src/commands/mod.rs` | 新用例文件的 `#[cfg(test)] mod …;` 登记（`config_knowledge_test` / `config_propose_test` / `atomic_test`）——本相位只落非测试行（`mod config_knowledge;` / `pub(crate) mod config_propose;` / `pub(crate) mod atomic;`），test-gen 在同一文件追加测试行 |
| 既有前端 fixture | `config-view.test.tsx` 需随分栏改造（`ResizablePanelGroup` 上下文）；`tool-cards.test.tsx` 新增五态用例 |
