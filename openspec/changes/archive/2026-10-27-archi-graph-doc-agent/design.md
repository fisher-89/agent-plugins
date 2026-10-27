# 设计: archi-graph-doc-agent

> **变更**: archi-graph-doc-agent
> **日期**: 2026-10-26
> **基于**: `proposal.md` 与 `specs/{desktop-archi-model,desktop-archi-module-docs,desktop-archi-canvas,desktop-archi-agent,desktop-agent-execution,desktop-phase-prompts,workspace-layout-resolution,desktop-crate-layout,desktop-page-routing,desktop-change-archive,desktop-write-protection}/spec.md`

---

## 提案与规格同步状态

proposal 的范围裁决（D1–D10）、四份新能力 spec、七份 MODIFIED/ADDED 能力 spec 均已定稿。本设计不重复其论证，只做四件事：把「一份 TOML 模型如何变成四个消费面的同一份图」落成可实现的结构（crate 内部模块边界、span 取法、投影口径）、把 spec 未给的成形缺口以决策留痕收口（诊断位置未知的表达、`archi` 的读/写分工、矩阵解析的 fail-closed 判据、文案的 200 字上限）、给出逐函数签名与前端组件切分，并按 AC-1…AC-22（不含测试项）逐条对齐。

起点核实（工作树实况；路径前缀 `packages/desktop/src-tauri/`，锚点为改动前行号）：

- **foundation 现状**：`crates/core/foundation/src/layout/mod.rs`——常量组 `:3-25`（`DOMAIN_DIR_NAME` / `CHANGES_DIR_NAME` / `ARCHIVE_DIR_NAME` / `EXPLORES_DIR_NAME` / `CONFIG_FILE_NAME` / `REPORTS_DIR_NAME` / 两个槽位名）；`Layout` 三字段 `:39-48`；`domain_dir_name()` `:51`（**fn 包装形态**）；`resolve` `:56`；`config_path` `:66`。零架构域段。
- **`domain_dir_name()` 消费点两处**：`crates/infra/agent/src/sdk/protect/rules.rs`（批量还原结构 glob 的域根派生）与 foundation 自身；改形态会牵动 protect（本变更的零改动面）⇒ 公开形态取 spec 允许的「保留 fn 包装」臂（D3）。
- **注入词表现状**：`crates/core/agent/src/runner.rs:44-49` `SessionToolset` 仅 `Explore`（`#[serde(rename_all="camelCase")]` 线值 `"explore"`）；`SessionInjections` `:55-65`（`preamble` / `tools` / `toolset`，`toolset` 带 `#[serde(default)]`）。
- **组合根现状**：`crates/infra/agent/src/compose.rs:22` `assemble_injections(source: &str)`（**单参**，只看 `source`）；唯一调用点 `:210`（`begin` 内），`provenance` 在 `:151-157` 签名上在手。
- **工具面现状**：`crates/infra/agent/src/sdk/tools.rs:26` `TOOL_NAMES` 七名封闭集；`:31` `EXPLORE_NOTE_TOOL`；`:47` `definitions`（`if toolset == Some(Explore)` 追加专用定义）；`:187` `input_path`（`glob`→`pattern`、其余→`path`）；`:195` `execute`；`:214` `cap_result`（**私有**，L1 30 KB 收口，`:21` 常量）。
- **loop 现状**：`crates/infra/agent/src/sdk/loop.rs`——`LoopTurn` `:75-100`（`explore_note: Option<ExploreNoteContext>` `:92-93` 与 `protect` `:94-97` 并列）；四段守卫链注释与成员校验 `:430-440`、policy `:441`、sandbox `:450`、protect `:457`、执行臂路由 `:464-477`（`EXPLORE_NOTE_TOOL` 走专用上下文，其余走 `tools::execute`）。
- **泵铸造点**：`crates/infra/agent/src/sdk/runner.rs:329-337` `explore_note` per-turn 铸造（`toolset == Some(Explore)` 且 store 句柄在场）；`:357-368` `LoopTurn` 字面量。
- **policy 现状**：`crates/infra/agent/src/sdk/policy.rs:23` `READONLY_TOOLS: [&str; 4]`；`:38` `allows`（`Default` 档只放行只读面）。
- **归档编排现状**：`crates/core/orchestration/src/steps.rs:374` `run_archive_preflight`（**同时被** 归档前停等面 `walker.rs:280` 与 Preflight 步 `walker.rs:2283` 调用——故半程挡回不能落在该函数内，它没有错误通道，见 D12）；`crates/core/orchestration/src/walker.rs:2276` `step_preflight`（有 `Terminal` 收口能力）；`:2621` `step_resolve_conflicts`（冲突清单 `cursor.rebase_conflicts` → A 快照 → agent 会话 → 后验）；`:2601` `fail_archive_step`（开节点 + 落失败 + `Terminal`）；`:2767` `lean_converge`（**含 `rebase --abort`**，模型冲突臂 MUST NOT 复用）；`:3656` `diagnose_brief`（节点 detail 按 **200 字符**截断）。
- **vcs 现状**：`crates/core/orchestration/src/port.rs:414` `worktree_snapshot(&self, root) -> Result<WorktreeSnapshot, String>`，`:491-500` `WorktreeSnapshot.rebase_head: Option<String>`；`crates/infra/vcs/src/git.rs:387` `unmerged_files`（`diff --name-only --diff-filter=U -z`，**repo 相对、`/` 分隔**）⇒ 冲突清单正是 `is_archi_rel_path` 的入参形态。
- **命令面现状**：`src/commands/mod.rs` 模块清单 + `:24-76` `all_commands!`（**唯一登记面**，`src/main.rs` 的 `generate_handler` 与 `src/bindings/mod.rs` 的 `collect_commands` 同源消费）；`src/commands/config/mod.rs:43-70` 为「薄包装 + `*_inner` 纯函数」先例（离 Tauri 运行时可测）。
- **前端现状**：`src/routes.tsx`（10 条路由 + 兜底 `*`）；`src/components/app-sidebar.tsx`（`PageNavGroup` 四项：`nav-info` / `nav-changes` / `nav-explores` / `nav-config`，active 由 `useLocation().pathname` 派生）；`src/views/changes/change-detail-view.tsx`（`DetailContent` 单区呈现流程图 + 抽屉，右栏无页签）；`src/views/changes/flow/run-control-panel.tsx:250-276` `ArchiveConfirmCard`（警告行 / delta specs / 跳过同步 / 合入告知 / 确认按钮，`preflight` 为入参）；`src/components/ui/tabs.tsx` 已被 `views/changes/renderers/artifact-tabs.tsx` 消费（页签形态在位）。
- **依赖面**：`src-tauri/Cargo.toml` `[workspace.dependencies]` **无** `toml` / `toml_edit` / `petgraph` / `ignore` 条目，但锁内四者俱在（`toml 1.1.6+spec-1.1.0` ← `tauri-utils` / `embed-resource` / `tauri-plugin-fs`；`toml_edit 0.25.15` ← `proc-macro-crate`；`petgraph 0.8.3` ← `prost-build` / `datafusion-physical-expr`；`ignore 0.4.33` ← `tokei` / `globwalk`）⇒ 加进 `[workspace.dependencies]` **零新增包**。`toml 1.1.6` 重导出 `serde_spanned::Spanned`（`src/lib.rs:192 pub use serde_spanned::Spanned;`，传递 `serde_spanned 1.1.1`）⇒ span 不需额外显式依赖。
- **版本面**：`packages/desktop/package.json:3` = `0.5.32`。

**三处事实校准（不影响 proposal 的范围裁决，但决定实现形态）**：

1. **`archi` 的写面必须落在命令层**：spec 要求「`archi` 内 MUST NOT 出现写盘调用」，而「原子写回」又要求同一文件内「写临时文件 + rename」。故 `archi::edit` 只产**待落盘产物**（路径 + 全文），落盘动作归 `commands/archi`（D8）。这不是分层洁癖：`archi` 一旦能写盘，`/architecture` 全量只读视图的服务端就没有「能力差异」可言了。
2. **proposal 的实现文件表漏列两处机械挂载**：① `crates/infra/agent/src/sdk/runner.rs`——`LoopTurn` 新增 `archi: Option<ArchiContext>` 字段后，其**唯一铸造点**在泵起播（`runner.rs:329-368`，与 `explore_note` 同处）；② `crates/infra/agent/src/sdk/mod.rs`——`archi_tool` 模块的挂载行。两者本设计补入变更清单（D7），理由同 `explore_note`：per-turn 上下文由组合根铸造、loop 只消费；模块挂载是编译前置。
3. **命名隔离扫描当前不覆盖 `crates/**`**：`crates/core/foundation/src/layout/mod_test.rs:140-211` 的 `collect_rs_files` 只收 `.rs`，且扫描根构造为 `crates/<一级目录>/src`（即 `crates/core/src` 这类不存在目录）——实际覆盖 `src-tauri/src` 与 `packages/desktop/src` 的 `.rs`。故 `archi` 的「零 `architecture` / 零 `openspec` / 零 `config.json` 字面量」由**源码审查 + 自查 grep** 承载，MUST NOT 借机改 foundation 测试（该测试不在本变更实现范围）；前端新文件同样以逐字扫描自查（AC-9）。

---

## 关键设计决策

| # | 议题 | 定稿 | 理由 |
|---|------|------|------|
| D1 | `archi` 的模块切分 | 八模块：`model`（格式无关 AST）/ `parse`（`mod.rs` 聚合 + `toml.rs` 实现）/ `graph`（`ModelGraph` 单点）/ `docs` / `search` / `diff` / `edit` / `dto`；`lib.rs` 只做模块装配 + 关键类型 `pub use`。`graph` 单点持有 `Model`（`ModelGraph::build(model: Model)`），`docs` / `search` / `dto` 一律吃 `&ModelGraph` | spec Module Contract 逐模块给定；图与模型同体使「谁都能再建一份图」在类型上不成立（拿不到 `Model` 的独立副本）。`parse` 仍按 `SourceParser` 抽象，换格式只动 `parse/` 一个文件 |
| D2 | span 取法与行列换算 | 反序列化用 `toml::Spanned<T>`（`toml` 1.1.6 重导出 `serde_spanned` 1.1.1）：元素**表头键** `Spanned<String>` → `modelLine`；关系 `from` 值 `Spanned<String>` → 关系 `modelLine` + 关系级 `Option<SourceSpan>`；`path` / `docs` 每项 `Spanned<String>` 在解析期消费。行列换算 `line_of(source, byte) = 1 + source[..byte].matches('\n').count()`、`column_of` 按 **char** 计（与 parser 的 caret 同单位） | 零二次扫文本、零 grep 反推（spec 明令）；`serde_spanned` 的值语义 `Ord` 使 `BTreeMap<Spanned<String>, _>` 可直接聚合 |
| D3 | `domain_dir_name` 的公开形态 | **保留 `fn domain_dir_name()` 包装**（`DOMAIN_DIR_NAME` 常量保持私有），三个新段名常量（`ARCHITECTURE_DIR_NAME` / `MODELS_DIR_NAME` / `DECISIONS_DIR_NAME`）**同私有**，只经派生函数暴露 | spec MODIFIED 明写「公开形态 design 定夺：导出 `pub const` 或保留 fn 包装」；消费点 `protect/rules.rs` 属零改动面，改签名要动它。新段名只经派生函数是 spec 硬要求（`architecture` 段单点持有） |
| D4 | 词汇枚举的落点 | `ElementKind` / `GateTier` / `DiagnosticKind` / `WarningKind` / `DocKind` / `DocSource` / `EdgeMark` / `NodeMark` / `SearchScope` 全部定义在 `dto.rs`（带 `specta::Type`）；`model.rs` 反向 `use crate::dto::{…}` | spec Module Contract 把封闭词汇钉在 `dto.rs`；不新增 spec 未列的第九模块。同 crate 模块互引合法，且词汇只有一处定义（解析层与线面共用，不存在两份枚举漂移） |
| D5 | 诊断位置的「未知」表达 | `Diagnostic.line: u32`，`0` 表示**位置未知**（`toml::de::Error::span()` 为 `None` 时），`column: Option<u32>` 独立缺省；DTO 同形。前端 `file:line` 在 `line == 0` 时只渲染 `file` | serde 自定义错误（如 `missing field`）在某些形态下无 span；`line = 0` 让「位置未知」可机械判别，优于伪造行号或用 `Option` 污染整条链路。常态下 span 俱在（proposal 症结一实测） |
| D6 | 免费五类诊断的映射 | 全部走一条通道：`toml::from_str::<RawDocument>` 的 `Err` → 按 `err.message()` 文本前缀归类（`unknown variant` / `invalid type`→`invalidValue`、`unknown field`→`unknownField`、`missing field`→`missingField`、`duplicate key`→`duplicateElement`、其余→`syntaxError`）；`Diagnostic.message = err.to_string()`（**原样**含 `TOML parse error at line N, column M` + 源码片段 + `^^^^`）。未引号点分形态（`[element.App.Auth]`）因此天然落入 `unknownField`（子键 `Auth` 不被 `RawElement` 的严格字段面接受）且**不会**与 `"App.Auth"` 合并 | spec「文案 / line / column / 源码片段与 caret SHALL 原样沿用 parser 输出」——判定 kind 只看前缀（分类学需要机器可辨），文案与位置零加工；「未引号点分形式被拒」由 `deny_unknown_fields` 机械保证，无需自研键形校验 |
| D7 | `archi` 的读写分工 | `archi` **只读盘不写盘**：解析（模型目录）、文档关联（元素 `path` 覆盖目录 + `decisions/`）、`plan_set_gate`（读模型 + 读目标文件 → 产出待落盘全文）。唯一写动作（临时文件 + rename）在 `commands/archi` | spec「`archi` 内除解析与关联读盘外 MUST NOT 出现写盘调用」+「gate 写回纯函数 MUST NOT 触碰文件系统」；`plan_set_gate` 让「元素 → 声明文件」的定位（领域规则）留在 `archi`，命令层只做 IO（三件事纪律） |
| D8 | `ModelGraph` 的单点与邻域口径 | `ModelGraph { model, node_of, fqn_of, graph: petgraph::DiGraph<(),()>, out_edges, in_edges }`；`build` 一次建图，SCC 用 `petgraph::algo::tarjan_scc`（`len > 1` 或 `len == 1` 且自环即环）；邻域只暴露 `depends_on` / `depended_by` / `children` 三项 + 由三者组合的 `neighborhood` | spec「唯一构建点」+「1-hop SHALL 由邻域查询组合，MUST NOT 做二次图遍历求可达性」。`self-loop` 显式归 `cycle`（`A -> A` 单点 SCC 的落法，spec 未列但环定义必然涵盖） |
| D9 | `archi_graph` 的停摆判据 | 停摆**不设布尔位**：判据 = `diagnostics` 非空（停摆时 `elements` / `relations` 恒空数组）；空模型（零元素零关系零诊断）走空态而非停摆态 | 少一个字段即少一处可与诊断集漂移的第二事实源；spec「停摆态 elements / relations 皆空」与「空模型合法」在数据上已可区分 |
| D10 | 图级是否带文档警告 | `ArchiGraph` **不含** `warnings`：文档关联（含 `docNotFound`）只在 `ArchiModuleDetail`（按元素、按需）产出 | 画布挂载若全量算邻近关联，等于每次开页都全仓扫 md（R-10）；spec 要求「`docNotFound` MUST NOT 触发停摆」，不带该字段即恒成立。若评审要求图级呈现，加字段属纯增量 |
| D11 | 注入矩阵的解析与 fail-closed | `source_ref` 以 `/` 切分，`len >= 4` 且**末段可解析为 `u32`** 才认（`attempt`→`role`→`phase` 自尾三段取）；phase / role 查 7 行矩阵，未命中或解析失败 → `SessionInjections::default()`；命中时 `tools: None` + `toolset: Some(Archi{..})` | spec「取尾三段定式」+「解析不出即 fail-closed 零 archi 工具」。末段数字校验是唯一的「段数不足 / 畸形」判别手段（`<id>/implement/executor` 三段 → 拒）；不引入新契约字段 |
| D12 | 归档半程挡回的落点 | 探测函数在 `steps.rs`（`check_rebase_halfway(vcs, worktree) -> Result<(), String>`），`Terminal` 收口在 `walker::step_preflight`（payload 到手、目录存在性分支之后、`settle_archive_node(passed)` 之前） | `run_archive_preflight` 被归档前停等面与 Preflight 步**共用**且返回 `Option`（无错误通道）——把挡回塞进去会让停等面也「失败」，与「确认面只看数据」冲突。proposal 的 `steps.rs` 行由此落实为「探测 + 文案」 |
| D13 | 半程探测失败的取向 | `worktree_snapshot` 自身 `Err` → **挡回**（fail-closed），文案附探测错误与两条出路；`worktree` 为 `None` 或目录不存在 → 不探测（既有分支已处置） | 探测失败无法区分「半程」与「干净」，而误放行的代价是 rebase 中间态被 `git add -A && git commit` 污染（R-7 明写「极难排查」）；误挡回只需用户按两条出路自查重跑。安全侧取向与「矩阵 fail-closed」同款 |
| D14 | 归档失败文案的 200 字上限 | 模型冲突臂与半程挡回臂的 `Terminal` 文案 SHALL ≤ 200 字符（含 worktree 绝对路径），三要素并列：worktree 绝对路径 / 处置动作 / 「再重跑归档」 | `diagnose_brief`（`walker.rs:3656`）按 200 字符截断节点 detail——超限会把命令吃掉，用户看到半句话。≤200 使「节点 detail == `run-error` 全文」，无需为失败面加第二个取数通道 |
| D15 | 模型冲突分流的判据与时机 | `step_resolve_conflicts` 中，冲突清单非空、`archive_context` 取到 worktree 之后、**任何快照 / 会话之前**：`pending.iter().any(foundation::layout::is_archi_rel_path)` → `fail_archive_step(..)`（零会话、零 `rebase_abort`） | spec「判据单点 `is_archi_rel_path`」、「零 agent 会话、不 abort」、「混合冲突走模型臂」。放在快照之前保证「零 git 副作用」也成立（连 A 快照都不取） |
| D16 | `archi_search` 的匹配口径 | 分词：按非字母数字切 + camelCase 边界切 → 小写段（去重保序）；命中判据 = 目标文本（小写）**包含全部段**（AND）；匹配对象恰为 FQN / `summary` / `metadata` 值 / 文档 `path` 与文件名；`scope` 收窄在投影层；空 / 空白 `query` 或未知 `scope` → `Err` | spec 逐条给定匹配对象与「不搜正文、不碰 specs、无相关度打分」；AND 语义让 `auth token` 与 `AuthToken` 收敛到同一行为（OR 会让多词 query 变成噪声放大器） |
| D17 | 排序与稳定序 | 三组输出一律稳定序：元素 / 子元素 / FQN 清单按 FQN 字典序；文档条目按「来源强度（explicit < adjacent < text）→ `path` 字典序」；关系统一按（`from`, `to`）字典序 | spec「稳定序便于断言与 diff」（文档条），其余沿用同一纪律；排序属「裁剪型投影」，不构成重算 / 补默认值 |
| D18 | 读盘失败的表达 | 模型文件读取失败（`fs::read_to_string` Err）→ `syntaxError` 诊断（`message` 前缀「模型文件读取失败」+ 系统错误文本）；文档读取失败 → 该条目按 `docNotFound` 处置 | 9 类诊断是封闭集，不新开 `ioError`（spec 明令词汇封闭）；「该文件读不成模型」语义上正是语法面失败，且读取面 MUST NOT 静默丢文件 |
| D19 | `metadata` 与非字符串值 | `metadata: BTreeMap<String, String>`（serde 严格）；非字符串值由 serde 报 `invalidValue` | spec「`metadata` SHALL 为字符串→字符串自由表；非字符串值即 `invalidValue`」；未知键天然被 map 接受（无 `deny_unknown_fields` 的 value 类型） |
| D20 | `archi_set_gate` 对「无显式 gate 行」的元素 | 允许插入该行（`toml_edit` 表格内追加 `gate = "<tier>"`），其余内容与注释逐字保留 | spec 只要求「元素存在 / 值合法 / 渲染自证」，未禁插入；显式化缺省 `off` 是 ratchet 的正当动作。若评审要求“只改既有行”，改动局限于 `edit.rs` 一个分支（见待决问题 6） |
| D21 | 前端 1-hop 的位置 | 后端 `archi_graph` 返**完整图**（零结构错时）；1-hop 由画布按「与聚焦元素直连的边 + 直接子元素」在前端过滤，聚焦切换**不触发取数** | 取数只有「挂载 / `root` 变化 / 手动刷新」三触发（spec），聚焦切换必须零 IPC；过滤规则与 `ModelGraph` 邻域三项同构（不引入第二套图语义），且后端 `neighborhood` 仍被 `archi_module` 消费（非死代码） |
| D22 | 抽屉 markdown 装配 | `element-drawer.tsx` 内本地装配 `<Markdown remarkPlugins={REMARK_PLUGINS}>`（模块级常量两行，与 `markdown-doc-renderer.tsx` 同款） | 复用 `MarkdownDocRenderer` 需伪造 `ArtifactEnvelope`（其契约是产物信封，正文直取不是产物）——浅重复优于错用契约；不新增 spec 未列的共享文件 |
| D23 | 空白 root 的失败面 | 五个 `archi_*` 命令对空白 / 不可读 root 一律 `Err`（沿 `workspace_config` 的「无效 root 统一 Err」裁定） | 画布没有 root 就画不出任何东西；若静默返回空图，会与「模型为空」（冷启动合法态）不可辨——spec 特意区分了停摆态 / 空模型态 |

---

## 架构组件 (Architecture Components)

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 模型 AST | 格式无关的 `Model` / `Element` / `Relation` / `SourceSpan` / `Diagnostic` / `ParseOutcome`；`derive PartialEq`（UI diff 预留）；关系级 `Option<SourceSpan>` 占位 | `crates/core/archi/src/model.rs`（新） | `crate::dto`（词汇）、`std` | 纯数据结构 |
| 解析聚合 | `trait SourceParser`、`parse_dir(root)`：字典序读 `*.toml` → 单文件解析 → 合并 → 跨文件 `duplicateElement` → `file` 回填（repo 相对）；空目录 → 空模型 | `crates/core/archi/src/parse/mod.rs`（新） | `foundation::layout::archi_models_dir`、`crate::model`、`crate::parse::toml` | `BTreeMap` 聚合 + 声明序保留 |
| TOML 实现 | `RawDocument` / `RawElement` / `RawRelation`（`deny_unknown_fields`）+ `toml::Spanned` span 提取 + parser 错误 → 结构化诊断 + 行列换算 | `crates/core/archi/src/parse/toml.rs`（新） | `toml`、`serde`、`crate::dto::{ElementKind, GateTier, DiagnosticKind}` | `toml 1.1.6` + `serde_spanned` |
| 图单点 | `ModelGraph` 唯一构建点；SCC 成环 / 依赖祖先 / 未定义引用 / 孤儿层级四类自实现诊断；`depends_on` / `depended_by` / `children` / `neighborhood` | `crates/core/archi/src/graph.rs`（新） | `petgraph`、`crate::model` | `petgraph 0.8.3`（`DiGraph` + `algo::tarjan_scc`） |
| 文档关联 | 三源（explicit / adjacent / text）+ 强度去重 + `docNotFound` 警告 + 邻近扫描内置排除谓词 + ADR 全文 FQN 命中；`docs` / `path` 条目收面即按 **repo 相对路径判据**（无绝对前缀 / 盘符 / `~` / `.` / `..` / 空段）拦截，拼接后再复核落在 `root` 之下（双重拦，越界不成关联集成员）；`repo 相对路径` 取法单点（`parse::relative_path`） | `crates/core/archi/src/docs.rs`（新） | `ignore`、`foundation::layout::{archi_decisions_dir, is_archi_rel_path}`、`crate::graph` | `ignore::WalkBuilder`（自动尊重 gitignore） |
| 轻量检索 | 分词 + 子串匹配（FQN / summary / metadata 值 / 文档路径与文件名）；scope 三值；不读正文、不碰 `openspec/specs/` | `crates/core/archi/src/search.rs`（新） | `crate::graph`、`crate::docs` | 纯函数（每次现算，无索引无缓存） |
| 结构 diff | 以 FQN 与 `(from,to)` 为身份键的元素 / 关系增删计数 + `gate` 变更清单 | `crates/core/archi/src/diff.rs`（新） | `crate::model`、`crate::dto::GateTier` | 纯函数 |
| gate 写回 | `set_gate(source, fqn, gate)` 纯函数（`toml_edit` 解析 → 改 AST → 渲染 → **自证可解析**）+ `plan_set_gate(root, fqn, gate)`（读模型定位文件 → 读源文本 → 产待落盘全文） | `crates/core/archi/src/edit.rs`（新） | `toml_edit`、`crate::dto::GateTier`、`crate::parse` | `toml_edit 0.25`（保留注释 / 空行 / 键序） |
| 三面投影 | 封闭词汇枚举 + `Archi*` DTO + `graph_view` / `module_detail` / `module_overview` / `search` / `change_summary` / `linked_doc_paths`；只裁剪不重算 | `crates/core/archi/src/dto.rs`（新） | `serde` / `serde_json` / `specta`、全兄弟模块 | `specta::Type` 派生 → `bindings.ts` |
| 路径派生 | `archi_dir` / `archi_models_dir` / `archi_decisions_dir` / `is_archi_rel_path`（纯推导零 IO，`architecture` 段单点持有） | `crates/core/foundation/src/layout/mod.rs`（修改） | `std::path` | 既有布局模块 |
| 命令面 | 五命令薄包装 + `*_inner` 纯函数 + 原子写回；零领域规则 | `src-tauri/src/commands/archi/mod.rs`（新）、`commands/mod.rs`（修改） | `archi`（投影 + 落盘产物）、`store`（change 记录）、`tauri` / `specta` | 沿 `commands/config` 三件事纪律 |
| 注入词表 | `SessionToolset::Archi { module: bool, search: bool }`（serde camelCase additive，旧线缺字段读 `None`） | `crates/core/agent/src/runner.rs`（修改） | `serde`（core 只承载词汇） | 既有枚举 |
| 注入装配 | `assemble_injections(provenance)`：explore 臂逐字不变；change 臂解析 `source_ref` 尾三段查 7 行矩阵；未命中 / 畸形 → default | `crates/infra/agent/src/compose.rs`（修改） | `agent::{SessionProvenance, SessionInjections, SessionToolset}` | 既有组合根 |
| 工具定义面 | 两条工具名常量 + schema + `definitions` 按 `toolset` 位追加 + `input_path` 显式 None 臂 + `cap_result` 升 `pub(crate)` | `crates/infra/agent/src/sdk/tools.rs`（修改） | `rig::completion::ToolDefinition` | 既有工具面 |
| 工具执行体 | `ArchiContext { root }`：`archi_module` 两形态（定位 / 薄概览）+ `archi_search` 三 scope；经 `archi::dto` 投影，L1 收口 | `crates/infra/agent/src/sdk/archi_tool.rs`（新） | `archi::dto`、`std::path`、`serde_json` | 纯 async 执行体（零 store 句柄） |
| 执行臂路由 | `LoopTurn.archi: Option<ArchiContext>`；两 archi 名路由至执行体，上下文缺席 → `is_error` 回灌 | `crates/infra/agent/src/sdk/loop.rs`（修改）、`sdk/runner.rs`（修改：per-turn 铸造） | `crate::sdk::archi_tool` | 与 `EXPLORE_NOTE_TOOL` 同式 |
| 静态权限表 | `READONLY_TOOLS` 扩至六名（三档全放行） | `crates/infra/agent/src/sdk/policy.rs`（修改） | `agent::AgentPermissionMode` | 既有决策表 |
| 归档挡回 | `check_rebase_halfway` + 文案单点；`step_preflight` 在写动作之前 `Terminal` | `crates/core/orchestration/src/steps.rs`、`walker.rs`（修改） | `ArchiveVcsPort::worktree_snapshot` | 既有 port（零新 git 能力） |
| 归档冲突分流 | `step_resolve_conflicts` 冲突清单前置分流：含架构域路径 → `Terminal::failed`（零会话、零 abort） | `crates/core/orchestration/src/walker.rs`（修改） | `foundation::layout::is_archi_rel_path` | 纯路径判定 |
| 画布域 | 共享画布 + 元素树 + 抽屉 + 停摆面板 + 标记映射 + 短名单点 + 两取数 hook | `packages/desktop/src/views/architecture/**`（新 9 文件） | `@xyflow/react`、`react-markdown`、`ui/*`、生成绑定 | React 19 + xyflow 12 |
| 入口与两挂载点 | `/architecture` 路由 + 侧栏「架构」项 + change 详情「架构」页签（`root = detail.worktree`，归档回落主仓根） | `src/routes.tsx`、`src/components/app-sidebar.tsx`、`src/views/changes/change-detail-view.tsx`（修改） | 生成绑定 `archiGraph` / `archiModule` | react-router 7 |
| 归档确认区块 | `ArchiveConfirmCard` 增「架构变更」区块（独立取数，零 `ArchivePreflight` 字段扩展） | `src/views/changes/flow/run-control-panel.tsx`（修改） | 生成绑定 `archiChangeSummary` | 既有卡片形态 |

依赖方向：`desktop-app → archi`、`agent-runtime → archi` 两条新增边，`archi → foundation` 唯一 workspace 内依赖；`workflow` ⇄ `archi` 零依赖；`archi` 零 Tauri / 零 store / 零 spawn / 零 `config`。

---

## 变更清单 (Change Inventory)

> 路径口径：**新增文件**用仓库根相对全路径；**修改文件**与 proposal 同口径（Rust 侧 `crates/…` 与 `src/…` 省略 `packages/desktop/src-tauri/` 前缀，前端侧写全 `packages/desktop/src/…`）。
> 测试文件（`corpus/` / `*_test.rs` / `*.test.tsx`）不在本清单射程——测试由独立相位承接。

### 新增文件

| 文件路径 | 说明 | 关联条目 |
|----------|------|----------|
| `packages/desktop/src-tauri/crates/core/archi/Cargo.toml` | crate 声明：`foundation` + `serde` / `serde_json` / `specta` + `toml` / `toml_edit` / `petgraph` / `ignore`（全走 `workspace = true`）；零 Tauri / 零 store / 零 spawn / 零 `workflow` / 零 `config` | 依赖表；D1 / D7；`desktop-crate-layout`「core/archi crate 落地登记」 |
| `…/crates/core/archi/src/lib.rs` | 八模块装配 + 关键类型 `pub use`（`ArchiGraph` / `ArchiModuleDetail` / `ArchiSearchResult` / `ArchiChangeSummary` / `ElementKind` / `GateTier` / `DiagnosticKind` / `EdgeMark` / `NodeMark` / `DocRef` / `SearchScope`）+ 各模块 `#[cfg(test)] mod *_test;` 声明 | 类型表全部行；D1 |
| `…/crates/core/archi/src/model.rs` | `Model` / `Element` / `Relation` / `SourceSpan` / `Diagnostic` / `ParseOutcome`；`Model::element` 查找；`derive PartialEq` | 公共函数表 `Model::element` 行；类型表 `Model` / `Element` / `Relation` / `SourceSpan` / `Diagnostic` / `ParseOutcome` 行；D2 / D5 / D19 |
| `…/crates/core/archi/src/parse/mod.rs` | `trait SourceParser`；`parse_dir(root)`：字典序聚合、跨文件重复 FQN 检出、`file` 回填、读盘失败落 `syntaxError` | 公共函数表 `parse_dir` / `SourceParser::parse` 行；D2 / D18 |
| `…/crates/core/archi/src/parse/toml.rs` | `RawDocument` / `RawElement` / `RawRelation`（`deny_unknown_fields` + `Spanned`）+ `TomlParser` 实现 + 错误归类 + `line_of` / `column_of` | 公共函数表 `TomlParser` / `line_of` / `column_of` 行；D2 / D6 |
| `…/crates/core/archi/src/graph.rs` | `ModelGraph`（构建 / `model` / 三邻域 / `neighborhood` / `diagnostics`）+ `Neighborhood` | 公共函数表 `ModelGraph::*` 行；类型表 `ModelGraph` / `Neighborhood` 行；D8 |
| `…/crates/core/archi/src/docs.rs` | 三源关联 + 强度排序去重 + 内置排除谓词 + `docNotFound` + ADR 正文 FQN 命中（`all_docs` 单次读 `decisions/` 分发） | 公共函数表 `element_docs` / `all_docs` 行；类型表 `DocsOutcome` / `DocWarning` 行；D10 / D17 |
| `…/crates/core/archi/src/search.rs` | 分词 + 子串匹配 + scope 收窄 + 稳定序 | 公共函数表 `search`、`SearchScope::parse` 行；类型表 `SearchOutcome` / `SearchScope` 行；D16 |
| `…/crates/core/archi/src/diff.rs` | `model_diff(base, next)`：元素 / 关系增删 + `gate` 变更（FQN 与 `(from,to)` 为身份键） | 公共函数表 `model_diff` 行；类型表 `ModelDiff` / `GateChange` 行；D17 |
| `…/crates/core/archi/src/edit.rs` | `set_gate` 纯函数（`toml_edit` 渲染 + 自证可解析）+ `plan_set_gate`（定位 + 产待落盘全文） | 公共函数表 `set_gate` / `plan_set_gate` 行；类型表 `GateWrite` 行；D7 / D20 |
| `…/crates/core/archi/src/dto.rs` | 封闭词汇枚举 + 全部 `Archi*` / `DocRef` DTO（`specta::Type`）+ 六个投影入口 + 内部 `load` 管线（`parse_dir` → `ModelGraph::build` → 诊断合并） | 公共函数表 `graph_view` / `module_detail` / `module_overview` / `search` / `change_summary` / `linked_doc_paths` 行；类型表全部 DTO 行；D4 / D9 / D10 |
| `packages/desktop/src-tauri/src/commands/archi/mod.rs` | 五命令薄包装 + 五个 `*_inner` + `write_atomic`（同目录临时文件 + `rename`） | 公共函数表五命令 + 五 inner 行；D23 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool.rs` | `ArchiContext { root }` + `execute(&self, name, input)`：`archi_module` 两形态 / `archi_search` 三 scope，`serde_json::to_string` 出文本，`tools::cap_result` 收口 | 公共函数表 `ArchiContext::new` / `ArchiContext::execute` 行；D7 / D16 |
| `packages/desktop/src/views/architecture/architecture-view.tsx` | 挂载点 A：页头（标题 + 显式刷新）+ `<ArchitectureCanvas root mode="full" />` | 前端组件表；`desktop-archi-canvas`「单一画布组件与两处挂载点」 |
| `packages/desktop/src/views/architecture/components/architecture-canvas.tsx` | 共享画布（`{ root, mode }`）：三段布局 + 1-hop 缺省（聚焦切换零取数）+ 两态切换（正常 / 停摆）+ 节点边映射与简单拓扑定位（节点渲染为本文件内局部组件，不新建节点文件） | 前端组件表；D21 |
| `packages/desktop/src/views/architecture/components/element-tree.tsx` | 左元素树：FQN 前缀派生层级 + 节点状态点（`nodeDot`）+ 停摆态零节点 | 前端组件表；D17 |
| `packages/desktop/src/views/architecture/components/element-drawer.tsx` | 右抽屉：概览（`kind` / `path` / `gate` / `modelFile:line`）+ `docs` 列表 + ADR 条目按需取正文（markdown 渲染）+ `incremental` 下 `gate` 编辑入口 + 行内错误 | 前端组件表；D22 |
| `packages/desktop/src/views/architecture/components/structure-error-panel.tsx` | 停摆面板：逐条 `kind` + `message` + `file:line` + 「修好后再看」 | 前端组件表；D9 |
| `packages/desktop/src/views/architecture/lib/marks.ts` | `edgeVisual(mark)` / `nodeDot(mark)`：边四值 / 节点两值 + 词汇外退化中性 | 公共函数表前端两行；类型表 `EdgeVisual` 行 |
| `packages/desktop/src/views/architecture/lib/fqn.ts` | `shortName(fqn)`：末段短名**单点**（元素树节点与画布节点共用同一取法，两处各写一份会各自演化，同一元素在树与画布呈现不同名字） | 公共函数表 `shortName` 行；`desktop-archi-canvas`「同一元素同一名字」 |
| `packages/desktop/src/views/architecture/hooks/use-archi-graph.ts` | `archiGraph` 取数（挂载 / `root` 变化 / 手动刷新三触发，零 watch 零轮询），停摆与正常两形状透出 | 公共函数表 `useArchiGraph` 行；D21 |
| `packages/desktop/src/views/architecture/hooks/use-archi-module.ts` | `archiModule`（抽屉元素详情）+ `archiReadDoc`（ADR 正文按需，仅关联集内路径） | 公共函数表 `useArchiModule` / `useArchiDoc` 行 |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `crates/core/foundation/src/layout/mod.rs` | 常量组（`:3-25`）追加三个**私有**常量 `ARCHITECTURE_DIR_NAME = "architecture"` / `MODELS_DIR_NAME = "models"` / `DECISIONS_DIR_NAME = "decisions"`（各带 doc）；文件尾追加四个公开函数 `archi_dir` / `archi_models_dir` / `archi_decisions_dir` / `is_archi_rel_path`（`Layout` / `resolve` / `config_path` / `domain_dir_name` **零改动**） | `architecture` 段单点持有，三子路径全从 `archi_dir` 派生（D3）；`archi_config_path` 显式不产出 |
| `src-tauri/Cargo.toml` | `[workspace] members` 增 `"crates/core/archi"`（按字母序插在 `crates/core/agent` 之后）；`[workspace.dependencies]` 增 `archi = { path = "crates/core/archi" }`、`toml = "1"`、`toml_edit = "0.25"`、`petgraph = "0.8"`、`ignore = "0.4"`（四者版本取锁内既有值，零新增包）；壳 `[dependencies]` 增 `archi = { workspace = true }`（附用途注释「命令面薄包装：投影与落盘产物全在 archi」） | `desktop-app → archi` 边登记（`desktop-crate-layout`）；`[package] version` 不随动 |
| `crates/infra/agent/Cargo.toml` | `[dependencies]` 在 `checks` 行之后增 `archi = { workspace = true }`，附两行注释「agent 工具执行体复用 `archi::dto` 投影（三方一致性机械前提，MUST NOT 另拼 JSON）；理由与 `agent-runtime → checks`（glob 方言单点）同构」 | `agent-runtime → archi` 边登记（用途注释为 spec 硬要求） |
| `crates/core/agent/src/runner.rs` | `SessionToolset`（`:44-49`）增 `Archi { module: bool, search: bool }` 变体（doc：两工具封闭集上的子集表达；serde camelCase additive，旧线缺字段读 `None`；新增第三工具须同改词汇与语料）；`SessionInjections` doc（`:51-54`）的「explore 来源六工具」一句补 archi 面 | `Explore` 臂的四条既有语义零改写（spec 明令） |
| `crates/infra/agent/src/compose.rs` | ① `assemble_injections`（`:22`）签名由 `(source: &str)` 改为 `(provenance: &SessionProvenance)`：explore 臂逐字不变（先判 `source == EXPLORE_SOURCE`），change 臂解析 `source_ref` 尾三段查矩阵，其余来源 default；② 新增私有 `fn archi_matrix(phase: &str, role: &str) -> Option<SessionToolset>`（7 行矩阵单源）与私有 `fn change_phase_role(source_ref: &str) -> Option<(&str, &str)>`（切分 + `u32` 校验）；③ 调用点 `:210` 改传 `&provenance` | 矩阵单源（desktop-archi-agent 那张表在本文件实现一次）；fail-closed（D11）；`resolve_agent_engine` 零改动 |
| `crates/infra/agent/src/sdk/mod.rs` | 模块清单按字母序插 `pub(crate) mod archi_tool;`（`archi_tool` 的唯一挂载点，本文件其余行零改动） | 新模块的挂载点；`explore_note` 同位先例（`:19`）；proposal 实现文件表未列，同属补入的机械项 |
| `crates/infra/agent/src/sdk/tools.rs` | ① 增两常量 `ARCHI_MODULE_TOOL` / `ARCHI_SEARCH_TOOL`（`pub(crate)`，doc 指向 `archi_tool`）；② `definitions`（`:47`）把末尾的 `if toolset == Explore` 改为 `match`：`Explore` 追加笔记工具、`Archi { module, search }` 按位追加两条定义（schema 见「工具定义面」）、`None` 零追加；③ `input_path`（`:187`）增显式臂：两 archi 名恒 `None`（不进路径沙箱，未来入参改名不引入改写面）；④ `cap_result`（`:214`）可见性 `fn` → `pub(crate) fn`（零行为变化，供 `archi_tool` 复用同一 L1 上限，禁第二份实现） | `TOOL_NAMES` 七名封闭集零改动；`execute` 纯函数签名零改动 |
| `crates/infra/agent/src/sdk/loop.rs` | ① `LoopTurn`（`:75-100`）增字段 `pub archi: Option<ArchiContext>`（doc：per-turn 铸造随轮携带；`None` = 未装配——非 Archi 会话恒 None，调用以 `is_error` 回灌）；② `:464` 执行臂由 `if/else` 改为三分派：`EXPLORE_NOTE_TOOL` → `explore_note`、两 archi 名 → `turn.archi`（`None` → `("专用工具执行上下文未装配".to_owned(), true)`）、其余 → `tools::execute`；③ 守卫链注释补一句「两个 archi 工具为读面，落在 `input_path` 的显式 None 臂上，沙箱与写入保护零介入」 | 成员校验 / policy / sandbox / protect 四段次序与语义零改动（spec 明令） |
| `crates/infra/agent/src/sdk/runner.rs` | ① `use crate::sdk::archi_tool::ArchiContext;`；② 泵起播（`:329-337` 旁）增 per-turn 铸造 `let archi = match open.injections.toolset { Some(SessionToolset::Archi { .. }) => Some(ArchiContext::new(open.ctx.workspace_root.clone())), _ => None };`；③ `LoopTurn` 字面量（`:357-368`）补 `archi` | proposal 漏列，补入（事实校准 2）；`explore_note` 铸造与 `PumpPayload` 零改动 |
| `crates/infra/agent/src/sdk/policy.rs` | `READONLY_TOOLS`（`:23`）`[&str; 4]` → `[&str; 6]`，追加 `"archi_module"` / `"archi_search"`（doc 补「两个 archi 只读工具在 `default` 档亦放行，与 read / grep / glob / ls 同款」） | `WRITE_TOOLS` / `EXECUTE_TOOLS` / `allows` 结构零改动（三档对两工具均放行由只读面天然给出） |
| `crates/core/orchestration/src/steps.rs` | 在 `run_archive_preflight`（`:374`）之后增 `pub(crate) fn check_rebase_halfway(vcs: &dyn ArchiveVcsPort, worktree: &str) -> Result<(), String>`：`worktree_snapshot` → `rebase_head.is_some()` 即 `Err(rebase_halfway_message(worktree))`；`worktree_snapshot` 自身 `Err` → `Err(探测失败文案)`（fail-closed，D13）；再增私有 `fn rebase_halfway_message(worktree: &str) -> String`（三要素、≤200 字，D14） | `run_archive_preflight` 本体**零改动**（停等面共用，不能长错误通道，D12）；零新 vcs 能力、零 spawn |
| `crates/core/orchestration/src/walker.rs` | ① `step_preflight`（`:2276`）在 `payload` 到手、worktree 目录存在性分支之后、`settle_archive_node(passed)` 之前插入：`if let Some(worktree) = payload.worktree.as_deref() { if let Err(reason) = check_rebase_halfway(ctx.vcs, worktree) { settle failed + return Err(Terminal::failed(reason)) } }`；② `step_resolve_conflicts`（`:2621`）在 `let mut pending = conflicts;` 之后、A 快照之前插入模型冲突分流：`if pending.iter().any(|path| foundation::layout::is_archi_rel_path(path)) { return Err(fail_archive_step(ctx, trace, cursor, kind, archi_conflict_message(&worktree, &pending))); }`，并增私有 `fn archi_conflict_message(worktree: &str, conflicts: &[String]) -> String`（≤200 字，D14） | 两处均为**前置短路**：Commit 步零执行（半程臂）、零 agent 会话与零 `rebase_abort`（冲突臂）；`lean_converge` / 后验三查 / pathspec 纪律 / rich-lean 分档逐字不变 |
| `src-tauri/src/commands/mod.rs` | 模块清单增 `pub mod archi;`（字母序首位，`agents` 之前）；`all_commands!`（`:24-76`）在 `env::app_environment` 之后**尾部追加**五条 `$crate::commands::archi::{archi_graph, archi_module, archi_read_doc, archi_change_summary, archi_set_gate}`（既有条目零位移） | 五命令的唯一登记面——`src/main.rs` 的 `generate_handler` 与 `src/bindings/mod.rs` 的 `collect_commands` 同源消费，漏登记则命令既不进 IPC 也不进 `bindings.ts`（AC-20 的生成物门面同时失守）；proposal 实现文件表与 tasks 阶段十一均已列入 |
| `packages/desktop/src/routes.tsx` | import `ArchitectureView`；`/agent` 与 `/explores` 之间（或 `/info` 之后，按现有次序就近）插 `<Route path="/architecture" element={<ArchitectureView root={root} />} />` | 既有 10 条路由与兜底 `*` 零改动（spec 明令） |
| `packages/desktop/src/components/app-sidebar.tsx` | `PageNavGroup` 在「探索」与「配置」之间插一项：`active={pathname === '/architecture'}` + `icon={<Network />}`（lucide）+ `label="架构"` + `testId="nav-architecture"` + `to="/architecture"`；函数 doc 的组序注释更新为 `[基础信息] [变更] [探索] [架构] [配置]` | 既有四项 testid / active 语义零改动；系统工具组零改动 |
| `packages/desktop/src/views/changes/change-detail-view.tsx` | ① 右栏内容区增页签（`ui/tabs`）：`detail-tab-flow`（流程图 + 产物抽屉，即现 `DetailContent`）与 `detail-tab-architecture`（`<ArchitectureCanvas root={archiRoot} mode="incremental" />`），缺省 `flow`；② 新增 `archiRoot` 派生单点：`detail.source === 'archive' || detail.worktree === null ? root : detail.worktree`；③ 架构页签用**条件挂载**（仅激活时渲染）以免未激活即取数；④ `DetailActionPanels` / `RunControlPanel` 增 `root` 透传（主仓根，供 `archiChangeSummary` 开库） | 页签切换不重复取数（条件挂载）；既有流程图 / 抽屉 / 左栏 Agent 面板语义零改动 |
| `packages/desktop/src/views/changes/flow/run-control-panel.tsx` | ① `ArchiveConfirmCard` 增「🏛 架构变更」区块（独立 hook 取数 `commands.archiChangeSummary(root, changeId)`）：`summary === null` → **零占位**（不渲染标题 / 空框 / 0 计数）；`summary !== null` → 元素 ± 计数 + FQN 清单、关系 ± 计数、`gate` 变更逐条（`fqn`：`from` → `to`）、`diagnostics` 非空时红字逐条 `kind` + `message` + `file:line` + 「修复后再归档」（**不阻断**确认按钮）、两个入口「查看模型 diff」（就地展开明细）/「查看新图预览」（就地展开 `<ArchitectureCanvas root={worktree} mode="incremental" />`）；② `RunControlPanel` 与 `ArchiveConfirmCard` 增 `root` / `changeId` 入参并下传 | 零 `ArchivePreflight` 字段扩展（spec 明令）；既有警告行 / delta specs / 跳过同步 / 合入告知 / `run-error` 面零改动 |
| `packages/desktop/src/types/generated/bindings.ts` | 由 `pnpm run bindings:export` **重导出**（禁手改）：五命令 + 全部 `Archi*` / `DocRef` / `SearchScope` / 词汇枚举 + `SessionToolset` 新变体 | `bindings:check` 零 diff 为交付门（AC-20） |
| `packages/desktop/package.json` | `version` `"0.5.32"` → `"0.6.0"`（单行） | 用户可见变更（新页面 / 新页签 / 新归档区块 / 两个 agent 工具）；`src-tauri/Cargo.toml` 的 crate version 不随动 |
| `crates/core/workflow/src/prompts/proposal-planner.md` | 补一句 `archi_search` 用法（发现：还不知道功能归谁管时先搜元素与文档路径；返回元素薄条目与四字段文档条目） | 矩阵命中 7 份之一（仅 `search`）；零路径模板、零写模型指令 |
| `crates/core/workflow/src/prompts/dev-design-planner.md` | 补两句：`archi_search`（发现）+ `archi_module`（定位：返回邻接、直接子元素、文档引用与 `modelFile` + `modelLine`，可直接 `edit` 到声明行） | 矩阵命中（两者） |
| `crates/core/workflow/src/prompts/dev-design-evaluator.md` | 同上两句（用读面核对设计是否与既有模型一致） | 矩阵命中（两者） |
| `crates/core/workflow/src/prompts/test-design-planner.md` | 补一句 `archi_search`（发现：把用例归到正确模块） | 矩阵命中（仅 `search`） |
| `crates/core/workflow/src/prompts/implementation-generator.md` | 补一句 `archi_module`（定位：改动前先确认模块边界与依赖方向） | 矩阵命中（仅 `module`） |
| `crates/core/workflow/src/prompts/code-review-evaluator.md` | 补一句 `archi_module`（核对改动是否越界依赖） | 矩阵命中（仅 `module`） |
| `crates/core/workflow/src/prompts/acceptance-evaluator.md` | 补一句 `archi_module`（核对验收面与模型声明的边界一致） | 矩阵命中（仅 `module`） |

> 未命中矩阵的 9 份 md（`proposal-evaluator` / `test-design-evaluator` / `implementation-evaluator` / `test-gen-*` 两份 / `test-execution-*` 两份 / `code-analyze-*` 两份）**零改动**——`__init__` 相位无角色 md，`archive` 的 SpecSync 走 `code-analyze` 语料而非矩阵命中文档（spec：未命中者 MUST NOT 出现 archi 字样）。`phase_table.rs` 的 16 条 `include_str!` 字面路径与角色表**逐字不变**。

### 公共函数 / API

> 口径：Rust 侧列 `pub` / `pub(crate)` 模块级函数与导出 class 的公共方法；前端列跨文件导出函数。私有函数（`dto` 内的 `load` 管线与 `to_*` 映射、`compose` 的 `archi_matrix` / `change_phase_role`、`write_atomic`、`check_root`、`rebase_halfway_message`、`archi_conflict_message`、`classify` / `normalize_rel` / `tokenize` 等）不入表。

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|-----------|------|------|
| `layout::archi_dir` | `crates/core/foundation/src/layout/mod.rs` | 新增 | `pub fn archi_dir(root: &Path) -> PathBuf` | `<域根>/architecture`；`architecture` 段唯一持有者。纯推导、零 IO、不存在的 root 亦正常返回 |
| `layout::archi_models_dir` | 同上 | 新增 | `pub fn archi_models_dir(root: &Path) -> PathBuf` | `archi_dir(root).join(MODELS_DIR_NAME)` |
| `layout::archi_decisions_dir` | 同上 | 新增 | `pub fn archi_decisions_dir(root: &Path) -> PathBuf` | `archi_dir(root).join(DECISIONS_DIR_NAME)` |
| `layout::is_archi_rel_path` | 同上 | 新增 | `pub fn is_archi_rel_path(rel: &str) -> bool` | repo 相对路径判定：`\` → `/` 归一、剥 `./`、按 `/` 切段后判「段 0 == 域根 && 段 1 == architecture 段」（`openspec/architecture` 与 `openspec/architecture/models/x.toml` 均命中）；归档冲突分流唯一判据 |
| `Model::element` | `crates/core/archi/src/model.rs` | 新增 | `pub fn element(&self, fqn: &str) -> Option<&Element>` | 按 FQN 取元素引用（`elements` 线性扫描，模型规模按元素数百计）；`dto` 投影、`edit::plan_set_gate` 的「元素 → 声明文件」定位与 `search` 的薄条目组装共用，禁第二份查找实现 |
| `SourceParser::parse` | `crates/core/archi/src/parse/mod.rs` | 新增 | `fn parse(source: &str) -> ParseOutcome` | 单文件解析契约（关联函数，无接收者）；`file` 字段留空，由聚合层回填 |
| `parse::parse_dir` | 同上 | 新增 | `pub fn parse_dir(root: &Path) -> ParseOutcome` | 聚合入口：`archi_models_dir(root)` 下 `*.toml` 按**文件名**字典序逐个解析 → 合并（声明序保留）→ 跨文件重复 FQN 检出（`file` / `line` 指字典序靠后者）→ `file` 回填 repo 相对（`/` 分隔）；目录缺席 / 空目录 → 空模型零诊断 |
| `toml::TomlParser` | `crates/core/archi/src/parse/toml.rs` | 新增 | `pub struct TomlParser;` + `impl SourceParser for TomlParser` | TOML 实现：`RawDocument` 反序列化（`deny_unknown_fields`）→ span 提取 → 诊断（D6 归类） |
| `toml::line_of` | 同上 | 新增 | `pub(crate) fn line_of(source: &str, byte: usize) -> u32` | 字节偏移 → 行号（1 起始） |
| `toml::column_of` | 同上 | 新增 | `pub(crate) fn column_of(source: &str, byte: usize) -> u32` | 字节偏移 → 列（1 起始，**char** 计） |
| `ModelGraph::build` | `crates/core/archi/src/graph.rs` | 新增 | `pub fn build(model: Model) -> ModelGraph` | **唯一**建图点：节点 = 元素 FQN、边 = 关系（`from → to`）；持有 model 本体 |
| `ModelGraph::model` | 同上 | 新增 | `pub fn model(&self) -> &Model` | 模型只读访问（避免 `(model, graph)` 双参数形态） |
| `ModelGraph::depends_on` | 同上 | 新增 | `pub fn depends_on(&self, fqn: &str) -> Vec<&Relation>` | 出边（按 `(from,to)` 字典序） |
| `ModelGraph::depended_by` | 同上 | 新增 | `pub fn depended_by(&self, fqn: &str) -> Vec<&Relation>` | 入边（同上序） |
| `ModelGraph::children` | 同上 | 新增 | `pub fn children(&self, fqn: &str) -> Vec<&str>` | 直接子元素（FQN 前缀一级，字典序） |
| `ModelGraph::neighborhood` | 同上 | 新增 | `pub fn neighborhood(&self, fqn: &str) -> Neighborhood<'_>` | 1-hop 三项组合（`desktop-archi-model`「ModelGraph 单点构建」的邻域查询单点） |
| `ModelGraph::diagnostics` | 同上 | 新增 | `pub fn diagnostics(&self) -> Vec<Diagnostic>` | 四类自实现结构诊断（`cycle` / `dependsOnAncestor` / `unknownReference` / `orphanHierarchy`）：一次列全、各带 `file` + `line`，`cycle` 的 `message` 含环路径、`unknownReference` 含引用方与缺失目标 |
| `docs::element_docs` | `crates/core/archi/src/docs.rs` | 新增 | `pub fn element_docs(graph: &ModelGraph, root: &Path, fqn: &str) -> DocsOutcome` | 单元素三源关联（`explicit` > `adjacent` > `text`）+ 同文件合并取强 + 稳定序 + `docNotFound` 警告（不进 `docs`） |
| `docs::all_docs` | 同上 | 新增 | `pub fn all_docs(graph: &ModelGraph, root: &Path) -> DocsOutcome` | 全元素并集（去重、稳定序）；`decisions/*.md` 正文只读一次并按 FQN 得分发（避免 O(元素×ADR) 次读盘） |
| `search::search` | `crates/core/archi/src/search.rs` | 新增 | `pub fn search(graph: &ModelGraph, root: &Path, query: &str, scope: SearchScope) -> Result<SearchOutcome, String>` | 分词 + 全段命中（D16）；空白 query → `Err`；`Elements` 时 `docs` 恒空、`Docs` 时 `element_fqns` 恒空 |
| `SearchScope::parse` | `crates/core/archi/src/dto.rs` | 新增 | `pub fn parse(raw: &str) -> Result<SearchScope, String>` | `"elements"` / `"docs"` / `"all"`（其余 → `Err` 并列出合法值） |
| `diff::model_diff` | `crates/core/archi/src/diff.rs` | 新增 | `pub fn model_diff(base: &Model, next: &Model) -> ModelDiff` | 元素增删（FQN 键、字典序清单）、关系增删计数（`(from,to)` 键）、`gate` 变更清单；文件 / 行序不参与身份 |
| `ModelDiff::is_empty` | 同上 | 新增 | `pub fn is_empty(&self) -> bool` | 三组皆空（「模型无改动零占位」的判据） |
| `edit::set_gate` | `crates/core/archi/src/edit.rs` | 新增 | `pub fn set_gate(source: &str, fqn: &str, gate: GateTier) -> Result<String, String>` | 纯函数：`toml_edit` 解析 → 改 AST → 渲染；元素缺席（附现有 FQN 清单）/ 渲染后不可解析 / 目标值回读不符 → `Err`（零文本产出）；注释空行键序逐字保留；零文件系统 |
| `edit::plan_set_gate` | 同上 | 新增 | `pub fn plan_set_gate(root: &Path, fqn: &str, gate: GateTier) -> Result<GateWrite, String>` | 读模型定位声明文件（`modelFile`）→ 读源文本 → `set_gate` → 返回待落盘 `{ file, content }`；**不写盘** |
| `dto::graph_view` | `crates/core/archi/src/dto.rs` | 新增 | `pub fn graph_view(root: &Path) -> Result<ArchiGraph, String>` | 画布数据单点：`parse_dir` → `ModelGraph::build` → 诊断合并；有结构诊断 → `elements` / `relations` 空（停摆），否则完整图（元素 / 关系按 FQN、`(from,to)` 稳定序，`mark` 恒 `none`） |
| `dto::module_detail` | 同上 | 新增 | `pub fn module_detail(root: &Path, fqn: &str) -> Result<ArchiModuleDetail, String>` | 单元素投影：`element` + `dependsOn` / `dependedBy` / `children`（经 `neighborhood`）+ `docs` + `warnings`；元素缺席 → `Err`（附总量提示）；结构错 → `Err`（附诊断清单文本） |
| `dto::module_overview` | 同上 | 新增 | `pub fn module_overview(root: &Path) -> Result<Vec<ArchiElementBrief>, String>` | 全元素薄概览（`fqn` / `kind` / `gate` / `summary`，字典序）；结构错 → 同上 `Err` |
| `dto::search` | 同上 | 新增 | `pub fn search(root: &Path, query: &str, scope: SearchScope) -> Result<ArchiSearchResult, String>` | 检索投影（薄条目 + 四字段文档条目） |
| `dto::change_summary` | 同上 | 新增 | `pub fn change_summary(main_root: &Path, next_root: &Path) -> Result<Option<ArchiChangeSummary>, String>` | 两侧模型 diff → 摘要；`next` 侧诊断随行（红字面）；两侧等价（`is_empty` 且零诊断）或两侧模型目录俱缺席 → `None`（零占位） |
| `dto::linked_doc_paths` | 同上 | 新增 | `pub fn linked_doc_paths(root: &Path) -> Result<BTreeSet<String>, String>` | 关联集路径集（`archi_read_doc` 越界校验的唯一判据） |
| `commands::archi::archi_graph` | `src-tauri/src/commands/archi/mod.rs` | 新增 | `pub fn archi_graph(root: String) -> Result<ArchiGraph, String>` | `#[tauri::command] #[specta::specta]`；薄包装 |
| `commands::archi::archi_graph_inner` | 同上 | 新增 | `pub fn archi_graph_inner(root: &Path) -> Result<ArchiGraph, String>` | root 有效性检查（空白 / 非目录 → `Err`）→ `archi::dto::graph_view` |
| `commands::archi::archi_module` | 同上 | 新增 | `pub fn archi_module(root: String, element: String) -> Result<ArchiModuleDetail, String>` | 薄包装 |
| `commands::archi::archi_module_inner` | 同上 | 新增 | `pub fn archi_module_inner(root: &Path, element: &str) -> Result<ArchiModuleDetail, String>` | root 检查 → `dto::module_detail` |
| `commands::archi::archi_read_doc` | 同上 | 新增 | `pub fn archi_read_doc(root: String, path: String) -> Result<String, String>` | 薄包装 |
| `commands::archi::archi_read_doc_inner` | 同上 | 新增 | `pub fn archi_read_doc_inner(root: &Path, path: &str) -> Result<String, String>` | root 检查 → `dto::linked_doc_paths` 成员校验（越界显式 `Err`，零内容返回）→ 归一后读盘（只读） |
| `commands::archi::archi_change_summary` | 同上 | 新增 | `pub fn archi_change_summary(stores: State<'_, WorkspaceStores>, root: String, change_id: String) -> Result<Option<ArchiChangeSummary>, String>` | 薄包装 + store 记录解析（`for_root` → `find_change_record` → worktree / 归档回落） |
| `commands::archi::archi_change_summary_inner` | 同上 | 新增 | `pub fn archi_change_summary_inner(main_root: &Path, worktree: Option<&str>) -> Result<Option<ArchiChangeSummary>, String>` | `next_root = worktree`（在同一 repo 内即为该 worktree 根）否则 `main_root` → `dto::change_summary` |
| `commands::archi::archi_set_gate` | 同上 | 新增 | `pub fn archi_set_gate(root: String, element: String, gate: GateTier) -> Result<(), String>` | 薄包装 |
| `commands::archi::archi_set_gate_inner` | 同上 | 新增 | `pub fn archi_set_gate_inner(root: &Path, element: &str, gate: GateTier) -> Result<(), String>` | root 检查 → `plan_set_gate`（写前校验全在此）→ 原子写回（同目录临时文件 + `rename`）；任一校验失败零写入 |
| `ArchiContext::new` | `crates/infra/agent/src/sdk/archi_tool.rs` | 新增 | `pub(crate) fn new(root: PathBuf) -> Self` | per-turn 上下文（泵起播铸造，`root` = 会话 cwd，change 会话即 worktree） |
| `ArchiContext::execute` | 同上 | 新增 | `pub(crate) async fn execute(&self, name: &str, input: &Value) -> Result<String, String>` | `archi_module`（有 / 无 `element` 两形态）、`archi_search`（`query` + `scope`）；坏模型 / 参数缺失 → `Err`（loop 侧 `is_error` 回灌，不 panic）；输出经 `tools::cap_result` |
| `tools::cap_result` | `crates/infra/agent/src/sdk/tools.rs` | 修改（仅可见性） | `pub(crate) fn cap_result(text: String) -> String` | L1 30 KB 收口单点，供 `archi_tool` 复用（禁第二份上限实现）；零行为变化 |
| `useArchiGraph` | `packages/desktop/src/views/architecture/hooks/use-archi-graph.ts` | 新增 | `useArchiGraph(root: string \| null): { graph: ArchiGraph \| null; loading: boolean; error: string \| null; refresh: () => void }` | 三触发取数（挂载 / `root` 变化 / 手动刷新），零 watch 零轮询；`root` 为 `null` 时不发 invoke |
| `useArchiModule` | `packages/desktop/src/views/architecture/hooks/use-archi-module.ts` | 新增 | `useArchiModule(root: string \| null, fqn: string \| null): { detail: ArchiModuleDetail \| null; loading: boolean; error: string \| null }` | 抽屉元素详情（`fqn` 变化即重取；`null` 不发） |
| `useArchiDoc` | 同上 | 新增 | `useArchiDoc(root: string \| null): { body: string \| null; loading: boolean; error: string \| null; load: (path: string) => void; reset: () => void }` | ADR 正文按需取（展开才发 invoke；`Err` 落 `error` 行内呈现） |
| `edgeVisual` | `packages/desktop/src/views/architecture/lib/marks.ts` | 新增 | `edgeVisual(mark: EdgeMark \| string): EdgeVisual` | 边标记四值 → `{ stroke, dasharray }`；词汇外 → 与 `none` 同款中性（不抛错、不空白） |
| `nodeDot` | 同上 | 新增 | `nodeDot(mark: NodeMark \| string): string` | 节点标记两值 → `'⚪' \| '🟡'`；词汇外 → `'⚪'` |
| `shortName` | `packages/desktop/src/views/architecture/lib/fqn.ts` | 新增 | `shortName(fqn: string): string` | FQN 末段短名（`App.Auth.Token` → `Token`）；元素树与画布节点同一取法 |

### 类型定义

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|--------|----------|-----------|------|
| `Model` | `crates/core/archi/src/model.rs` | 新增 | `{ elements: Vec<Element>, relations: Vec<Relation> }`（声明序保留）；`derive Debug, Clone, PartialEq, Eq` |
| `Element` | 同上 | 新增 | `{ fqn: String, kind: ElementKind, gate: GateTier, path: Vec<String>, docs: Vec<String>, summary: Option<String>, metadata: BTreeMap<String, String>, file: String, line: u32 }`；`file` 为 repo 相对路径 |
| `Relation` | 同上 | 新增 | `{ from: String, to: String, description: Option<String>, file: String, line: u32, span: Option<SourceSpan> }`；`span` 为占位（当前只消费行号） |
| `SourceSpan` | 同上 | 新增 | `{ start: usize, end: usize }`（字节区间；进程内，不进线面、无 serde） |
| `Diagnostic` | 同上 | 新增 | `{ kind: DiagnosticKind, message: String, file: String, line: u32, column: Option<u32> }`；`line == 0` = 位置未知（D5） |
| `ParseOutcome` | 同上 | 新增 | `{ model: Model, diagnostics: Vec<Diagnostic> }` |
| `ModelGraph` | `crates/core/archi/src/graph.rs` | 新增 | 持有 `model` + `node_of: BTreeMap<String, NodeIndex>` + `fqn_of: Vec<String>` + `graph: petgraph::DiGraph<(), ()>` + `out_edges` / `in_edges: BTreeMap<String, Vec<usize>>`；非 `Clone`（单点持有语义） |
| `Neighborhood<'a>` | 同上 | 新增 | `{ depends_on: Vec<&'a Relation>, depended_by: Vec<&'a Relation>, children: Vec<&'a str> }` |
| `DocsOutcome` | `crates/core/archi/src/docs.rs` | 新增 | `{ docs: Vec<DocRef>, warnings: Vec<DocWarning> }` |
| `DocWarning` | 同上 | 新增 | `{ element: String, path: String }`（`docNotFound` 的进程内形态；DTO 映射为 `ArchiWarning`） |
| `SearchOutcome` | `crates/core/archi/src/search.rs` | 新增 | `{ element_fqns: Vec<String>, docs: Vec<DocRef> }` |
| `ModelDiff` | `crates/core/archi/src/diff.rs` | 新增 | `{ elements_added: Vec<String>, elements_removed: Vec<String>, relations_added: u32, relations_removed: u32, gate_changes: Vec<GateChange> }` |
| `GateChange` | 同上 | 新增 | `{ fqn: String, from: GateTier, to: GateTier }` |
| `GateWrite` | `crates/core/archi/src/edit.rs` | 新增 | `{ file: PathBuf, content: String }`（绝对路径 + 待落盘全文） |
| `ElementKind` | `crates/core/archi/src/dto.rs` | 新增 | 封闭四值 `package` / `domain` / `module` / `component`（serde `lowercase`；`specta::Type`；解析层与线面共用） |
| `GateTier` | 同上 | 新增 | 封闭三值 `off` / `warn` / `strict`（serde `lowercase`；`Default = Off`） |
| `DiagnosticKind`（**线面导出名 `ArchiDiagnosticKind`**） | 同上 | 新增 | 封闭九值 `syntaxError` / `missingField` / `invalidValue` / `unknownField` / `duplicateElement` / `cycle` / `dependsOnAncestor` / `unknownReference` / `orphanHierarchy`（serde `camelCase`）。命名注记：specta 的类型注册表按**裸类型名**唯一，工作区配置侧已有同名 `DiagnosticKind`（两侧同名 → 注册冲突），故 wire 面必须带 `Archi` 前缀；进程内以 `pub use ArchiDiagnosticKind as DiagnosticKind` 别名引用（`bindings.ts` 里是 `ArchiDiagnosticKind`，`ArchiDiagnostic.kind` 亦指向它） |
| `WarningKind` | 同上 | 新增 | 单值 `docNotFound`（serde `camelCase`）——与 `DiagnosticKind` 分立，使「文档警告不停摆」在类型上成立（D9） |
| `DocKind` | 同上 | 新增 | `md` / `adr`（serde `lowercase`；由落地路径决定，D17 备注） |
| `DocSource` | 同上 | 新增 | `explicit` / `adjacent` / `text`（serde `lowercase`；强度序即此序） |
| `EdgeMark` | 同上 | 新增 | `none` / `unfulfilled` / `crossKind` / `exempt`（serde `camelCase`） |
| `NodeMark` | 同上 | 新增 | `none` / `unfulfilled`（serde `camelCase`）——**不含**结构错值（结构错一律停摆） |
| `SearchScope` | 同上 | 新增 | `elements` / `docs` / `all`（serde `lowercase`） |
| `DocRef` | 同上 | 新增 | `{ path: String, title: String, kind: DocKind, source: DocSource }`——恒四字段，无正文 / 摘要 / 行区间 |
| `ArchiGraph` | 同上 | 新增 | `{ elements: Vec<ArchiElement>, relations: Vec<ArchiRelation>, diagnostics: Vec<ArchiDiagnostic> }`（停摆判据 = `diagnostics` 非空，D9） |
| `ArchiElement` | 同上 | 新增 | `{ fqn, kind, gate, path: Vec<String>, summary: Option<String>, metadata: BTreeMap<String, String>, model_file: String, model_line: u32, mark: NodeMark }` |
| `ArchiRelation` | 同上 | 新增 | `{ from: String, to: String, description: Option<String>, mark: EdgeMark, reason: Option<String> }`（`reason` 本期恒 `null`，`exempt` 场景的呈现载体） |
| `ArchiDiagnostic` | 同上 | 新增 | `{ kind: DiagnosticKind, message: String, file: String, line: u32, column: Option<u32> }`（`kind` 的线面类型名 `ArchiDiagnosticKind`，见上表命名注记） |
| `ArchiWarning` | 同上 | 新增 | `{ kind: WarningKind, message: String, element: Option<String>, path: Option<String> }` |
| `ArchiElementBrief` | 同上 | 新增 | `{ fqn, kind, gate, summary: Option<String> }`（薄概览 / 检索条目） |
| `ArchiModuleDetail` | 同上 | 新增 | `{ element: ArchiElement, depends_on: Vec<ArchiDependsOn>, depended_by: Vec<ArchiDependedBy>, children: Vec<String>, docs: Vec<DocRef>, warnings: Vec<ArchiWarning> }` |
| `ArchiDependsOn` | 同上 | 新增 | `{ target: String, description: Option<String> }`（出边对端） |
| `ArchiDependedBy` | 同上 | 新增 | `{ source: String, description: Option<String> }`（入边对端）——与 `ArchiDependsOn` 分立而非共用一个 `{fqn}`，使 `desktop-archi-agent` 的 `target` / `source` 字段面逐字成立 |
| `ArchiSearchResult` | 同上 | 新增 | `{ elements: Vec<ArchiElementBrief>, docs: Vec<DocRef> }` |
| `ArchiChangeSummary` | 同上 | 新增 | `{ elements_added: Vec<String>, elements_removed: Vec<String>, relations_added: u32, relations_removed: u32, gate_changes: Vec<ArchiGateChange>, diagnostics: Vec<ArchiDiagnostic> }` |
| `ArchiGateChange` | 同上 | 新增 | `{ fqn: String, from: GateTier, to: GateTier }` |
| `SessionToolset` | `crates/core/agent/src/runner.rs` | 修改 | 增 `Archi { module: bool, search: bool }` 变体；线值 `{"archi":{"module":bool,"search":bool}}`，旧线无该字段读 `None`；`Explore` 臂零变化 |
| `EdgeVisual` | `packages/desktop/src/views/architecture/lib/marks.ts` | 新增 | `{ stroke: string; dasharray: string \| null }` |

### 配置

<!-- 本变更零新增配置键：架构域配置（扫描排除 / 豁免键集）与其读取面显式归 archi-check-and-gate（`archi_config_path` 本期不产出）；工作区 config.json 的 KNOWN_KEYS 七项与 config.schema.json 镜像零变化，故本子表整段省略。 -->

---

## 数据模型 (Data Model)

### 持久化：`<archi_dir>/models/*.toml`

```
[element."App"]                     # 键 = 带引号的完整 FQN 单键（硬约定）
kind  = "package"                   # 必填，封闭四值
gate  = "strict"                    # 可选，封闭三值，缺省 off
path  = ["./src/auth/"]             # 可选，字符串数组（缺省空）
docs  = ["./src/auth/README.md"]    # 可选，字符串数组（缺省空）
summary = "认证与令牌签发"            # 可选字符串
metadata = { owner = "platform" }   # 可选内联表（字符串→字符串，未知键忽略）

[[relation]]
from = "App.Auth"                   # 必填
to   = "App.Shared"                 # 必填
desc = "复用错误类型"                # 可选
```

| 面 | 规则 |
|---|---|
| 文件组织 | `models/*.toml` 多文件，按**文件名**字典序聚合；同一 FQN 跨文件重复 → `duplicateElement`（`file` / `line` 指字典序靠后者）；同文件重复由 TOML map 语义拒绝 |
| 层级 | 由 **FQN 最长前缀**推导（`App.Auth.Token` 之父 = `App.Auth`），MUST NOT 由表嵌套推导；祖先未声明 → `orphanHierarchy`（不自动补建隐式祖先） |
| 字段封闭性 | 未知**字段**拒绝（`unknownField`）；`metadata` 内未知**键**接受；`metadata` MUST NOT 用子表形态 |
| 空态 | `models/` 缺席或空 → 空模型（零元素零关系零诊断），画布呈空态而非停摆 |
| 定位 | 元素 `modelFile`（repo 相对）+ `modelLine`（表头键行）；关系 `modelLine` = `from` 键行；均取自解析期 span |

### 进程内结构

```
parse_dir(root)
   └─ ParseOutcome { model: Model, diagnostics: Vec<Diagnostic> }   // 免费五类
        └─ ModelGraph::build(model)                                 // 唯一建图点
             ├─ ModelGraph::diagnostics()  → 自实现四类 Diagnostic
             ├─ 邻域查询（depends_on / depended_by / children / neighborhood）
             └─ dto::*  三面投影（只裁剪）
                  ├─ ArchiGraph           → IPC archi_graph / 画布
                  ├─ ArchiModuleDetail    → IPC archi_module + agent archi_module
                  ├─ ArchiSearchResult    → agent archi_search
                  └─ ArchiChangeSummary   → IPC archi_change_summary / 归档确认区块
```

- **身份键**：元素 = `fqn`；关系 = `(from, to)`；文档 = `path`（repo 相对、`/` 分隔）。文件与行号**不参与身份**（diff / 去重 / 稳定序均不依赖）。
- **文档关联**：`explicit`（元素 `docs`）> `adjacent`（元素 `path` 覆盖目录内 `*.md`，内置排除测试 / 构建 / VCS / 架构域目录 + gitignore 语义）> `text`（`decisions/*.md` 正文出现该 FQN）；同文件合并取最强；`kind` 由落地路径决定（`decisions/` 内恒 `adr`，其余 `md`）。**不提供跨元素反查**：`all_docs` 输出为扁平去重清单（`DocRef` 不含元素归属字段），元素的文档与其归属只在 `element_docs` 一侧成对出现——「这份文档描述哪些模块」的反向索引不在本期（spec 明令，防用户开始写"给 archi 看的文档"）。
- **无库表**：模型不落库（零新表 / 零新字段 / 零迁移）；画布不缓存、不建倒排（每次现算）。
- **写路径**：worktree 内 `models/*.toml` 与源码同权（agent 用 `write` / `edit`）；`gate` 单字段写回经 `archi_set_gate`（服务端写前校验 + `toml_edit` 渲染 + 自证可解析 + 原子 rename）。

---

## 路由 / API 设计 (Route / API Design)

### IPC 命令（5 条，全部带显式 `root`，错误面 `Result<T, String>`）

| 方法 | 路径（命令名） | 说明 | 输入 | 输出 | auth |
|------|---------------|------|------|------|------|
| invoke | `archi_graph` | 画布数据（停摆 / 正常两态） | `root: string` | `ArchiGraph` | 无（本地 IPC） |
| invoke | `archi_module` | 单元素投影（与 agent 工具同一投影） | `root: string`, `element: string` | `ArchiModuleDetail` | 同上 |
| invoke | `archi_read_doc` | 文档正文（仅关联集内路径） | `root: string`, `path: string` | `string` | 同上 |
| invoke | `archi_change_summary` | 归档确认面的模型结构 diff 摘要 | `root: string`, `change_id: string` | `Option<ArchiChangeSummary>`（`null` = 无改动 / 无可比基线） | 同上 |
| invoke | `archi_set_gate` | 改档写回（唯一写命令） | `root: string`, `element: string`, `gate: GateTier` | `()` | 同上 |

- 五条命令全部登记进 `all_commands!`，随 `bindings:export` 出线为 `commands.archiGraph(...)` 等（camelCase）。
- 前四条 MUST NOT 写盘；`archi_set_gate` MUST NOT 出现在 `mode = full` 的前端调用路径（前端只读视图不渲染编辑入口）。
- 空白 / 不可读 `root` → 五条一律 `Err`（D23）。

### 前端路由

| 方法 | 路径 | 说明 | 输入 | 输出 | auth |
|------|------|------|------|------|------|
| route | `/architecture` | 架构页（挂载点 A，`mode="full"`，根取壳态当前根，URL 不携根段） | — | `ArchitectureView` | 无 |

- 侧栏「页面」组新增 `nav-architecture`（active 由 `pathname === '/architecture'` 派生）。
- change 详情「架构」页签不是路由：`/changes/:id` 内的页签态（本地 state，缺省 `flow`）。

---

## 依赖 (Dependencies)

### 运行时依赖

| 依赖 | 归属 | 用途 |
|------|------|------|
| `foundation` | `archi`（workspace 内唯一） | 架构域路径派生（`archi_models_dir` / `archi_decisions_dir` / `is_archi_rel_path`） |
| `serde` / `serde_json` / `specta` | `archi` | DTO 序列化 + `specta::Type`（→ `bindings.ts`）；`serde_json` 另用于词表对拍的 JSON 面 |
| `toml` (`1`) | `archi` | 类型化反序列化 + `Spanned` span + 原文错误（含 line/column/片段/caret）；**零自研 parser** |
| `toml_edit` (`0.25`) | `archi` | gate 写回的「解析 → 改 AST → 渲染」（保留注释 / 空行 / 键序） |
| `petgraph` (`0.8`) | `archi` | `DiGraph` + `algo::tarjan_scc`（成环检出） |
| `ignore` (`0.4`) | `archi` | 邻近扫描的目录遍历（自动尊重 gitignore，跳过 `node_modules` / `target`） |
| `archi` | `agent-runtime`（infra → core，新增边） | 工具执行体复用 `archi::dto` 同一投影（三方一致性的机械前提） |
| `archi` | `desktop-app`（壳 → core，新增边） | 五命令薄包装 |
| `@xyflow/react`（既有 `^12.11.0`） | 前端 | 画布渲染（仅复用库与 `ui/*`，**不复用** `flow/` 的布局与节点组件，R-5） |
| `react-markdown` / `remark-gfm`（既有） | 前端 | ADR 正文与模型 snippet 的 markdown 渲染 |

### 构建 / 测试依赖

| 依赖 | 归属 | 用途 |
|------|------|------|
| `tempfile`（既有） | `archi` dev-deps（如需）+ app dev-deps | 语料 / 文档关联的 tempdir 驱动 |
| `vite-plus`（`vp test`） | 前端 | jsdom 组件测试；xyflow 度量垫片沿用 `change-flow-graph.test.tsx` 先例（测试相位承接） |

四者（`toml` / `toml_edit` / `petgraph` / `ignore`）版本均取锁内既有值 ⇒ **零新增包**、`Cargo.lock` 打包集合不变。

---

## 待决问题 (Open Questions)

1. **`toml` 1.x 的 span 重导出**：当前 `toml::Spanned`（= `serde_spanned::Spanned`）可用且 `BTreeMap<Spanned<String>, _>` 的 `Ord` 成立。若未来 `toml` 1.x 调整该重导出，需显式引入 `serde_spanned`（锁内已有 1.1.1，仍属零新增包）。观察项，不阻塞。
2. **`archi_search` 的 AND 语义**：多词 query 要求全段命中（D16）。若 agent 侧反馈误 miss（例如 `auth legacy`），可放宽为 OR——契约面（匹配对象 / 排序 / scope）不变，属语义调优。
3. **图级文档警告**：`ArchiGraph` 不含 `warnings`（D10，为免画布挂载全扫目录）。若评审认为画布须显示 `docNotFound` 汇总，加字段属纯增量（不动既有字段与停摆判据）。
4. **关系级 span 的列区间**：本期只消费 `from` 键行号；将来「点击一条边跳到声明」可能还需 `to` 键位置（届时加 `Option<SourceSpan>` 第二槽位，属纯增量）。
5. **`exempt` 边的 `reason` 呈现细节**：`ArchiRelation.reason` 本期恒 `null`（生产者为 check 侧）；抽屉内的呈现样式（灰实线 + reason 行）留 check 侧接入时定稿，契约面（字段已在位）不变。
6. **`archi_set_gate` 对「无显式 `gate` 行」的元素**：本期允许插入该行（D20，缺省显式化）。若评审要求「只改既有行」，改动局限 `edit.rs::set_gate` 一个分支（`Err("该元素未声明 gate")`），需同步 `desktop-archi-model`「gate 档写回纯函数」的验收措辞。
7. **模型目录的 `reports/` 等 check 侧子目录**：本期 `archi` 不读、不建，仅 `models/` 与 `decisions/` 两个子目录在射程内；`archi-check-and-gate` 落地时以 `<archi_dir>/config.json` 承载排除键集（`archi_config_path` 明确延后，不在本期公开面）。
