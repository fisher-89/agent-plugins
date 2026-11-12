# 设计: archi-file-tree

> **变更**: archi-file-tree
> **日期**: 2026-11-11
> **基于**: `proposal.md`（问题 / 提案一~六 / 17 条决策 / 变更范围 / AC-1…AC-17 / R-1…R-14）、`explore.md`（只读，799 行：§12 启动前复核 + §13/§14 两项裁决）与七份 delta spec（`desktop-archi-file-tree` 6 ADDED / `desktop-archi-canvas` 3 MODIFIED / `desktop-archi-model` 1 RENAMED / `desktop-archi-api` 1 MODIFIED / `desktop-archi-drift` 1 MODIFIED / `desktop-ipc-type-bindings` 1 MODIFIED / `repo-archi-model` 1 MODIFIED + 1 ADDED）
> **性质**: `/architecture` 三段布局最左栏由「FQN 元素树」改为「文件树（缺省）+ 元素树（分段切换）」，外加两项判定面修正：生成物归属判据下沉 `OwnerIndex`（A2″ 并集）、架构域配置新增 `exclude` 一条。画布、抽屉、漂移标记、门禁、写入面零改动。

---

## 提案与规格同步状态

proposal 的范围裁决（左栏双视图、扁平 DTO、判定下沉、A2″ 并集、D5′ 完整排除矩阵、D6 前向防御）与七份 spec delta 均已定稿。本设计不重述论证，只做四件事：

1. 把 spec 已钉语义、未钉**代码形态**的四处收敛到可编译的具体签名与落点：`strip_candidates` 的候选集与 `owner_of` 的定档式（D1）、`contains` / 模块根表的**实现归属**（D2，决定 `scan/owner.rs` 是否多两个访问器）、`file_tree` 的内部拆分与节点合成规则（D5~D8）、前端状态面的挂载层级（D13）。
2. 把「枚举面 = 漂移判定面」在实现层落成一条链路（D9），并逐项点名复用点与**必须提权**的两个私有 helper（事实校准 1）。
3. 逐条 AC 给落点（「验收标准回应」表），**不写测试策略**（用例编写与门禁执行由独立相位承接）。
4. 把三处必须显式留痕的边界写成可实现形态：零命中提示文案不得断言成因（D14）、`exists = false` 的 kind 判定（D8）、跨 root 旧树不冒充（D13）。

### 起点实况核实（工作树实测；Rust 侧路径前缀 `packages/desktop/src-tauri/`，前端侧 `packages/desktop/src/`；锚点为改动前行号）

- **判据面**：`crates/core/archi/src/scan/owner.rs`（158 行）——`GENERATED_MARKER` `:19` / `GENERATED_EXTENSIONS` `:22`、`pub enum Owner` `:26-38`、`pub struct OwnerIndex { entries: Vec<OwnerEntry> }` `:41-43`、`struct OwnerEntry { fqn, prefix, gate }` `:46-50`、`fn normalize` `:53-60`（**私有**）、`OwnerIndex::build` `:64-80`、`OwnerIndex::owner_of` `:84-120`、`fn strip_generated` `:126-150`（**只做替换扩展名**）、`fn prefix_matches` `:153-158`。
- **扫描编排**：`crates/core/archi/src/scan/mod.rs`（371 行）——`SOURCE_EXTENSIONS` `:32`、`ExcludeSet::build(input.exclude)` `:107`、枚举段 `:117-134`、排除段 `:139-146`、归属段 `:151-194`、`fn walk_source_files` `:304-327`（**私有**，`WalkBuilder` + `filter_entry` 五剪枝）、`fn is_source_file` `:330-335`（**私有**）、`fn posix` `:338-345`（**私有**）；文件尾 `#[cfg(test)] mod *_test;` 五条 `:362-371`。
- **排除矩阵**：`crates/core/archi/src/scan/exclude.rs`（105 行）——`DEFAULT_EXCLUDE_RULES` `:28-49`（七条约定 → 13 条具体 glob）、`ExcludeSet::build(extra)` `:67-95`（默认在前、配置追加、非法条目两条显式 `Err`）、`matched(rel) -> Option<String>` `:98-104`（最小下标优先）。
- **配置读取**：`crates/core/archi/src/drift/config.rs`（127 行）——`RawConfig` 带 `#[serde(deny_unknown_fields)]` `:38-45`（只认 `ignore` / `exclude`）、`load_config(root, known)` `:64-127`（缺失即 `Default`；非法条目显式 `Err`）。路径经 `foundation::layout::archi_config_path`（`crates/core/foundation/src/layout/mod.rs:164`）。
- **出线投影**：`crates/core/archi/src/dto.rs`（748 行）——九组封闭词汇 `:16-264`（`ElementKind` `ALL` `:42`、`GateTier` `ALL` `:76`、`ArchiDiagnosticKind` + `ALL` `:159`、`WarningKind` / `DocKind` / `DocSource` / `EdgeMark` `/ NodeMark` `:230` / `SearchScope`）、线面 DTO `:267-480`、`fn load` `:498-506`（**私有**，`parse_dir` + `ModelGraph::build` 单点）、`pub fn graph_view` `:594-617`（停摆态返回 `Ok` + 空数组 + 诊断全量）、`pub(crate) fn graph_diagnostics_error(&[ArchiDiagnostic]) -> String` `:564-570`。
- **出口面**：`crates/core/archi/src/lib.rs`（65 行）——模块清单 `:21-31`、`pub use` 四段 `:33-50`（`api` / `drift` / `dto` / `model` / `scan`）、`#[cfg(test)] mod *_test;` 七条 `:52-64`。
- **命令面**：`src/commands/archi/mod.rs`（190 行）——文件头「五命令」`:1-2`（与实现不符：实为五条，与 `bindings/mod_test.rs` 的 `ARCHI_COMMANDS` 四条不一）、`check_root` `:36-49`、`write_atomic` `:52-72`、五条命令 + 五个 `*_inner`（`archi_graph` `:77-85` / `archi_module` `:87-95` / `archi_change_summary` `:97` / `archi_set_gate` `:~110-153` / `archi_drift` `:157-190`），`:19` 注释写「IPC 四命令契约」。
- **登记面**：`src/commands/mod.rs` `all_commands!` `:24-84`，archi 段 `:76-80`（`archi_graph` / `archi_module` / `archi_change_summary` / `archi_set_gate` / `archi_drift`），`:81` 为 `commands::fs::read_file`（尾条）。
- **清单守卫**：`src/bindings/mod_test.rs`（3038 行）——`COMMAND_WRAPPERS` `:127`（48 条）/ `COMMAND_NAMES` `:179`（48 条）/ `DTO_TYPES` `:231`（97 条）；`COMMAND_NAMES.len()` 三处断言 `:1047` / `:1951` / `:2423`、`DTO_TYPES.len()` `:1107`；`ARCHI_COMMANDS: [(&str, &str); 4]` `:1652`（**漏 `archi_drift`**）、`ARCHI_DTO_TYPES: [&str; 19]` `:1660`；尾部追加序期望 vec `:1714-1726`（`… archi_set_gate, archi_drift, read_file`）；`清单遗漏即产物覆盖断言必红` `:1770`（取 `ARCHI_COMMANDS` 末条做副本删一条 = `archi_set_gate`）。
- **前端左栏**：`src/views/architecture/components/architecture-canvas.tsx`（286 行）——`CanvasBody` `:162-232`，三段布局 `:195`（`<div className="flex h-full min-h-0 gap-2">`）→ `ElementTree` `:196` → 画布 `:197-222` → `ElementDrawer` `:223-229`；停摆分支 `:267-274`。
- **元素树**：`components/element-tree.tsx`（150 行）——`section` `:131`（`min-h-0 w-72 shrink-0 flex flex-col`，`data-testid="archi-element-tree"`）、`h2` `:132`、空态 `:133-136`、`ScrollArea` `:138-147`；节点 `data-testid="archi-tree-node"` `:98` + `data-fqn` `:99`。
- **展开态先例**：`views/info/components/dir-tree.tsx`（149 行）——`DEFAULT_EXPAND_SEGMENTS = 2` `:7`、`INDENT_STEP = 16` `:10`、`isExpanded(path, flipped)` `:16-19`（默认 XOR 翻转）、`flipped` 集合 `useState<ReadonlySet<string>>` `:124-135`、折叠不渲子树 `:104-113`。
- **取数先例**：`hooks/use-archi-graph.ts`（55 行）= 三触发 + `loadedRoot` 判据 + 无 watch；`hooks/use-archi-drift.ts`（70 行）= 失败清空 + `loadedKey` 判据。
- **版本面**：`packages/desktop/package.json:3` = `0.6.35`。
- **仓库配置现状**：`.dev-team/architecture/` 下只有 `api/` / `decisions/` / `models/`，**无配置**。

### 事实校准（不改 spec 意图，只定实现形态）

1. **proposal 漏列第三处可见性提升（`scan::posix`）**：`walk_source_files` 返回的是 `root.join(...)` 绝对路径，转 repo 相对 POSIX 必须经 `scan::posix`（剥反斜杠 + 剥前导 `./`）。若在 `tree.rs` 另写一份就是 AC-6 明令禁止的「第二份归一」。裁定：`posix` 与 `walk_source_files` / `is_source_file` 一并由私有提为 `pub(crate)`（三处都只提权限、语义零变化）。本设计已补入变更清单。
2. **`OwnerIndex::normalize` 不需要提权**：spec 要求「归一复用 `OwnerIndex::normalize` 同一实现」的实质是「树侧不另写归一」。若树经 `OwnerIndex` 的访问器拿到**已归一**的模块根路径与 `owner_of` 结果，则 `normalize` 可保持私有——见 D2。这样 `owner.rs` 的改动面收敛为「改剥离判据 + 增两个只增不改的 `pub(crate)` 访问器」。
3. **「同长多命中」本仓两来源均已核实为 0**（`explore.md §4`）：`ambiguous` 分支只能由合成语料覆盖；实现上**不写特判**——`Owner::Ambiguous` 原样喂给 `FileTreeMark::Ambiguous`，与 `module_roots` 表项 ≥2 条同走一条分支（D6）。
4. **`dto::load` 私有、`graph_view` 返回 `Ok(停摆态)`**：`tree.rs` MUST NOT 二次 `parse_dir` + `ModelGraph::build`（图单点构建纪律）。裁定：`tree.rs` 经 `dto::graph_view(root)` 取元素与诊断，停摆态判据 = `!graph.diagnostics.is_empty()` → `Err(dto::graph_diagnostics_error(&graph.diagnostics))`（该函数已是 `pub(crate)`，零新增文案实现）。见 D3。
5. **`bindings/mod_test.rs` 的 `ARCHI_COMMANDS` 现缺 `archi_drift`（4 条 vs `all_commands!` 实有 5 条）**：这是既有漏登，本次必须一并补齐（`desktop-ipc-type-bindings` 已把「补漏 + 新增」写成条款），否则「把两步混淆成一步」的病灶留在原地。
6. **`commands/archi/mod_test.rs` 文件头已写「六命令薄包装」而实现是五条**：本变更落地后自然对齐，故该文件的「四命令」用例名改写是**口径订正**而非新增用例（归测试相位，本设计只登记）。
7. **`desktop-archi-file-tree` 的 `FileTree` Props 面**只列 `entries` / `focused` / `onFocus`，而同一 cap 又要求 loading / 错误 / 空三态。裁定见 D13：三态由**栏容器**（`architecture-canvas.tsx`）持有并渲染，`FileTree` 保持三参纯净；`archi-file-tree-empty` 由 `FileTree` 自身在 `entries.length === 0` 时渲（可从入参派生）。
8. **本变更不新增 / 不改模块 API 契约**：新增的 `pub` 项（`ArchiFileTree` / `ArchiFileEntry` / `FileEntryKind` / `FileTreeMark` / `tree::file_tree` / `archi_file_tree`）落在 `App.Backend.Core.Archi`（`INDEX.toml` 现状 `status = "absent"`）与 `App.Frontend.Views.Architecture` 元素下；`api:export` / `api:check` 的 `apiUndeclared` 档位为 `off`、`apiUnexpected` 只查「`api = none` 却写契约」——故**零新 finding、零 diff**。契约补录不在本变更范围。

---

## 关键设计决策

| # | 议题 | 定稿 | 理由 |
|---|------|------|------|
| D1 | 生成物并集的实现形态 | `fn strip_candidates(rel_path: &str) -> Vec<String>`（私有）：白名单外的路径返回**单候选**（`vec![归一后的原路径]`）；白名单内的生成物返回**两候选**（`[整组剥离 <name>, 替换扩展名 <stem>.<ext>]`）；`owner_of` 对每个候选调同一个 `longest_prefix_hits(&rel)` 收集 `(fqn, prefix_len)`，合并去重后按 `prefix_len` 取最大档，档内 `hits.len() == 1` → `Element`、`>= 2` → `Ambiguous` | 「候选短路」禁止靠「先各自求完整命中集、再合并定档」自然成立（没有任何一条路径能提前返回）；候选集合并后同长仍归 `Ambiguous`，与既有语义一致；单候选形态使非 `.i.<ext>` 路径的归属逐值不变 |
| D2 | 模块根表与 `contains` 的实现归属 | **两者都进 `scan/owner.rs` 的 `OwnerIndex`**：`pub(crate) fn module_roots(&self) -> BTreeMap<&str, Vec<&str>>`（归一路径 → FQN 字典序去重列表）、`pub(crate) fn contains_count(&self, path: &str) -> u32`（严格前缀 `"{path}/"`，按 FQN 去重、不含自身）。`tree.rs` 只查表，源码零 `starts_with` / 零归一 | AC-6 是**源码判据**（「`src` 全域无第二份 `path` 前缀匹配」）：把 `contains` 的严格前缀判断写在 `tree.rs` 就是第二份前缀实现，评审无法机械判白。放进 `owner.rs` 则剥离 / 归一 / 最长前缀 / 后代计数四点同文件，`normalize` 与 `prefix_matches` 仍可私有（`module_roots` 的键已是归一形态）。见待决问题 1 |
| D3 | 模型读取面 | `tree.rs` 唯一读模型入口 = `dto::graph_view(root)`；停摆判据 `!diagnostics.is_empty()` → `Err(dto::graph_diagnostics_error(&diagnostics))`；**空模型不是错**（`elements` 空 + 诊断空 ⇒ 继续枚举，全条目 `mark = none`） | 图单点构建纪律（`dto::load` 是 `ModelGraph::build` 的唯一调用点）；`graph_diagnostics_error` 是既有文案单点，禁止在 `tree.rs` 重拼诊断串 |
| D4 | `Tree` 的 DTO 与词表位置 | 四类型全部落 `dto.rs`（紧邻既有九组封闭词汇与线面 DTO），`FileTreeMark` 用 `#[serde(rename_all = "camelCase")]` + `ALL: [FileTreeMark; 5]` + `as_str`，`FileEntryKind` 用 `lowercase` + `ALL: [FileEntryKind; 2]`（单词值两种 `rename_all` 同形，取与同族 mark 枚举一致者） | 线面 DTO 与词汇单点纪律（`dto.rs:13` 注释「词汇单点」）；`FileTreeMark` 不进四面词汇对拍（`tests/vocabulary_test.rs` 的 `封闭词汇值域九组` 零改动） |
| D5 | 枚举与过滤链路 | `walk_source_files(root)` → 逐条 `strip_prefix(root)` + `scan::posix` → `is_source_file` → `ExcludeSet::build(&config.exclude)` 的 `matched(rel).is_some()` 丢弃 → `sort` + `dedup`。**排除矩阵只作用于枚举出的文件**：模块根是「元素的声明」而非「枚举结果」，不过排除矩阵（否则「声明了却在树上消失」变成静默失败，与 D6 同一理由；本仓 45 条 `path` 无一命中排除矩阵，此优先级无实际分歧） | 与 `scan::scan()` 的 ①②③ 段逐条同序（枚举 → 模式过滤 → 排除），差异仅「无模式过滤」（树恒全量）；`exclude.rs` / `exclude_test.rs` 零 diff（`build(extra)` 语义即所需，`build_config_only` 作废） |
| D6 | 节点集合与 `mark` 判定 | 节点集合 = 枚举文件 ∪ 模块根（含盘上不存在的）∪ 两者全部祖先目录（去重）；逐节点：① `module_roots.get(path)` 命中 ⇒ `len == 1 → elementRoot` / `len >= 2 → ambiguous`；② 否则 `owner_of(path)`：`Element → member` / `Ambiguous → ambiguous` / `None → contains > 0 ? containsElements : none`。源码零 `ambiguous` 特判（两条来源同走一条分支） | spec「判据顺序 ①→②→③」逐字落地；`contains` 与 `mark` 正交（每节点都算） |
| D7 | 缩进 / 展开态的派生面 | 后端只出扁平条目 + `path` 字节序；**层级、兄弟序、缩进全部前端派生**（`buildForest`：按 `path` 切段挂父，子序 = 目录先于文件、各自 `name` 序）；展开态 = per-path 翻转集合（`DEFAULT_EXPAND_SEGMENTS = 2`，与 `dir-tree.tsx` 同式） | spec「渲染顺序」与「展开态」明文归前端；后端出确定性字节序 + 前端派生 = 两侧判据各只有一处 |
| D8 | `kind` / `exists` 的判定 | 对每个模块根在盘探测一次：在盘目录 → `Dir`、在盘文件 → `File`、**不在盘 → `Dir`**；`exists = !(kind == Dir && 是模块根 && 不在盘)`（其余恒 `true`）。枚举出的文件恒 `File` / `exists = true`，合成目录恒 `Dir` / `exists = true` | spec Scenario 明写「声明但目录不存在仍出节点且 `kind = "dir"`」⇒ 缺失时只能取 `Dir`；`exists` 判据**只看盘**（不掺 `pathNotFound` 文档诊断） |
| D9 | 排除配置的读取面 | `known` = `graph.elements` FQN 集合 → `drift::config::load_config(root, &known)` → 只取 `config.exclude`（`ignore` 不参与）→ `ExcludeSet::build(&config.exclude)`；配置非法 → 整个命令 `Err`（文案沿用 `load_config` / `ExcludeSet::build` 既有显式文案） | 「树面 = 判定面」零分叉；零新增配置解析器、零新增入口（`ignore` 在树侧无意义——它按元素对匹配，与文件枚举无关） |
| D10 | 排序与确定性 | `entries.sort_by(\|a, b\| a.path.cmp(&b.path))`（`String` 字节序），节点集合用 `BTreeMap<String, …>` 承载（天然去重 + 有序）；`owners` / `module_roots` 的值字典序去重 | spec「同输入同输出」（幂等）；`BTreeMap` 让「同一文件与同一目录各恰一条」在结构上成立 |
| D11 | 命令层薄包装形态 | `archi_file_tree(root: String) -> Result<ArchiFileTree, String>` = `archi_file_tree_inner(Path::new(&root))`；`_inner` = `check_root` → `archi::tree::file_tree`。命令层零归属判定、零目录合成、零标记推导 | 沿既有五个 `*_inner` 形态（「三件事纪律」）；离 Tauri 可测 |
| D12 | 前端取数 hook 的形态 | `useArchiFileTree(root: string \| null) -> { tree, loading, error }`：三触发（挂载 / `root` 变化 / 上层 `key` 重挂）**不含** `refresh` 成员；失败与 `root` 变化都清空 `tree` + `loadedRoot`（旧树不冒充）；`root === null` 零 invoke | `refresh` 成员会让「手动刷新」有第二个入口；`ArchitectureView` 的 `refreshTick → key` 重挂已覆盖第三种触发（与 `useArchiGraph` 的差异是有意的：graph 的 `refresh` 被抽屉改档后回用，树没有改档面）；`loadedRoot` 判据同 `use-archi-graph` |
| D13 | 三态（loading / error / 空）的挂载层级 | `FileTree` Props **恰为** `{ entries, focused, onFocus }`；`architecture-canvas.tsx` 在栏容器内按序判：`error !== null` → `archi-file-tree-error` 行内提示；`tree === null` → `archi-file-tree-loading`「解析中…」（**判据是 `tree === null` 而非 `loading`**——`useEffect` 首拍前 `loading` 仍为 `false`，用 `tree` 判可让「首拍、在途、失败后未清」三态都落到加载行，零闪烁）；否则渲 `<FileTree />`（其内 `entries` 空则 `archi-file-tree-empty`「仓库内无源文件」） | spec Module Contract 已钉 Props 三参（事实校准 7）；状态行与 `archi-canvas-loading` / `archi-drift-error` 同挂载层级，语义一致（都是「取数面信息」而非「数据面渲染」）；`tree` 判据与 `use-archi-drift` 的 `loadedKey` 判据同族（未载入即不呈现旧数据） |
| D14 | 零命中提示与图例文案 | 零命中提示逐字：`该元素在树上无可定位节点（容器元素不持有文件，或其源文件均在判定面之外）`（**并列两种成因**，不选其一）；图例行逐字：`仅显示判定面源文件（测试文件与架构配置 exclude 命中项不在树中；none 行 = 无元素认领）` | spec「文案 MUST NOT 断言『该元素是容器』」——`path` 非空但源文件全被排除矩阵吃掉时同样零命中；两种成因并列即不断言 |
| D15 | 滚动定位与强制展开的实现 | 命中行 ref 收集后对首条执行 `element.scrollIntoView?.({ block: 'nearest' })`（**可选调用**）；聚焦命中的祖先链**由派生集合 `forcedOpen` 强制展开**（`isExpanded = forcedOpen.has(path) \|\| 翻转 XOR 默认`），**MUST NOT 用 `useEffect` 写 `flipped`** | 仓库既有代码零 `scrollIntoView` 使用、测试侧无对应桩（`agent-messages.test.tsx` 只桩 `scrollTop` / `scrollTo`）；可选调用让「不滚动」与「滚动」两态在 jsdom 下都不抛错，无需新增环境桩。强制展开走派生而非写 state，才能「用户翻转优先保持、聚焦切换不污染操作态」（AC-11 的「展开态按 path 保持」与「聚焦展开祖先链」两条同时成立） |
| D16 | 分段控件的归属 | `TreeModeSwitch` 在 `file-tree.tsx` 内导出、由 `architecture-canvas.tsx` 挂载并持有 `useState<'file' \| 'element'>('file')`；栏容器宽度随模式 `w-80` / `w-72`，内层 `min-h-0 flex-1 flex` 包住当前视图（`ElementTree` 自身的 `w-72` / `min-h-0` 逐字保留） | proposal 明写「控件由 `architecture-canvas.tsx` 挂载与持有状态，本件只导出并渲它」；`flex` 内层保证两个树组件仍按今日「行向 flex 项纵向拉伸」取得满高（零改 `ElementTree`） |
| D17 | 明确不做 | ①树不消费漂移报告（零违例标记）；②`ambiguous` / `containsElements` / `none` 行零候选弹层；③不与 `dir-tree.tsx` 抽公共件；④不给 agent 加 `archi_file_tree` 工具；⑤不做单层懒加载（实测 475 节点 / 84.2 KB）；⑥不加「显示测试文件」开关；⑦不改 `graph_view` 停摆语义；⑧不改 `scan()` 的六段与 `ExcludeSet` | 逐条对应 proposal「不要修改」与 AC-17；越界项一律拒 |

---

## 架构组件 (Architecture Components)

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 归属判据（**改**） | 候选集剥离（整组 ∪ 替换扩展名）+ 合并定档；另增两个只增不改的 `pub(crate)` 访问器（`module_roots` / `contains_count`） | `crates/core/archi/src/scan/owner.rs` | `crate::dto::{ArchiElement, GateTier}` | 既有纯函数（零 IO） |
| 扫描单点（**可见性提权**） | `walk_source_files` / `is_source_file` / `posix` 提为 `pub(crate)`；`scan()` 六段与 `SOURCE_EXTENSIONS` 零改动 | `crates/core/archi/src/scan/mod.rs` | `ignore::WalkBuilder` | 既有适配器 |
| 排除矩阵（**零改动**） | `ExcludeSet::build(extra)` 的默认 13 条 + 配置追加 | `crates/core/archi/src/scan/exclude.rs` | `globset` | 既有 |
| 看护配置读取（**零改动**） | `load_config(root, known)` 只取 `exclude` | `crates/core/archi/src/drift/config.rs` | `serde_json` | 既有 |
| 文件树投影（**新增**） | `file_tree(root)`：读模型 → 配置 → 枚举 + 排除 → 模块根表 / 归属 → 目录合成 → `mark` / `owners` / `contains` / `exists` → 字节序 | `crates/core/archi/src/tree.rs`（新） | `crate::{dto, scan, drift::config}` | 纯读盘、零写盘、零 spawn |
| 出线 DTO 与词表（**改**） | `ArchiFileTree` / `ArchiFileEntry` / `FileEntryKind` / `FileTreeMark` | `crates/core/archi/src/dto.rs` | `serde` / `specta` | 既有线面 DTO 形态 |
| 出口面（**改**） | `pub mod tree;` + `file_tree` 与四类型的 `pub use` | `crates/core/archi/src/lib.rs` | — | 既有 `pub use` 段 |
| 命令面（**改**） | `archi_file_tree` 薄包装 + `archi_file_tree_inner`；文件头两处口径订正 | `src-tauri/src/commands/archi/mod.rs` | `archi::tree::file_tree` | `#[tauri::command] #[specta::specta]` |
| 登记面（**改**） | `all_commands!` archi 段追加一行 | `src-tauri/src/commands/mod.rs` | — | 既有宏 |
| 清单守卫（**随动**） | 命令 / 包装 / DTO 三清单 + archi 两族清单与尾部序期望 | `src-tauri/src/bindings/mod_test.rs` | — | 既有 |
| 架构域配置（**新增**） | `{ "exclude": ["openspec/**"] }`（**不带 `$schema`**） | `.dev-team/architecture/config.json`（新） | — | JSON |
| 树取数 hook（**新增**） | `useArchiFileTree(root)`：三触发、零 watch、失败清空 | `packages/desktop/src/views/architecture/hooks/use-archi-file-tree.ts`（新） | 生成绑定 `commands.archiFileTree` | React 19 hooks |
| 左栏文件树（**新增**） | `FileTree`（扁平 → 森林、展开态、五值标记、双向联动、图例）+ `TreeModeSwitch` | `packages/desktop/src/views/architecture/components/file-tree.tsx`（新） | `ui/scroll-area`、`ui/button`、`lucide-react` | React |
| 画布接线（**改**） | `CanvasBody` 左栏 = 分段控件 + 条件渲 `FileTree` / `ElementTree`；三态行渲染；`useArchiFileTree` 接线 | `packages/desktop/src/views/architecture/components/architecture-canvas.tsx` | 上述 hook 与组件 | React |
| 版本行（**改**） | `version` `0.6.35 → 0.6.36` | `packages/desktop/package.json` | — | — |

依赖方向零新增边：`archi` 仍不依赖任何 infra crate（`tree.rs` 只用 `archi` 内部模块 + `foundation` / `globset` / `ignore` / `serde`，全在既有依赖内）；前端零新增 npm 依赖。

### `tree.rs` 的内部拆分（私有，不入公共 API 表）

```
pub fn file_tree(root: &Path) -> Result<ArchiFileTree, String>
  ├ fn candidate_files(root: &Path, exclude: &ExcludeSet) -> Vec<String>
  ├ fn node_set(files: &[String], roots: &BTreeMap<&str, Vec<&str>>) -> BTreeMap<String, FileEntryKind>
  ├ fn mark_of(path: &str, roots: &BTreeMap<&str, Vec<&str>>, owners: &OwnerIndex) -> (FileTreeMark, Vec<String>)
  └ fn exists_of(root: &Path, path: &str, kind: FileEntryKind, is_root: bool) -> bool
```

`file_tree` 主流程（可执行伪码）：

```
graph = dto::graph_view(root)?                       // D3：图单点构建
if !graph.diagnostics.is_empty() → Err(dto::graph_diagnostics_error(..))
known = graph.elements.map(fqn)                      // BTreeSet<String>
config = drift::config::load_config(root, &known)?    // D9：只取 exclude
exclude = ExcludeSet::build(&config.exclude)?
files = candidate_files(root, &exclude)              // walk → posix → is_source_file → matched 丢弃
owners = OwnerIndex::build(&graph.elements)
roots = owners.module_roots()                        // D2：归一路径 → FQN 列表
nodes = node_set(&files, &roots)                     // 文件 ∪ 模块根 ∪ 全部祖先目录
for (path, kind) in nodes:
    (mark, owners_list) = mark_of(path, &roots, &owners)
    contains = owners.contains_count(path)           // D6③：正交计数
    exists   = exists_of(root, path, kind, roots.contains_key(path))
entries = nodes → ArchiFileEntry { path, name: 末段, kind, mark, owners, contains, exists }
entries.sort_by(path)                                // D10
```

---

## 变更清单 (Change Inventory)

> 路径口径：Rust 侧 `crates/…` / `src/…` 省略 `packages/desktop/src-tauri/` 前缀，前端侧写全 `packages/desktop/src/…`。
> **测试用例编写归测试相位**；本清单只登记 proposal「测试文件」表中的两类**机械随动**：① `bindings/mod_test.rs` 的清单守卫（spec 要求同 change 随动，否则门禁必红）；② `tree_test.rs` 的空模块可解析（见「编译闭合随动」表）。
> **覆盖核对**：proposal「变更范围 - 实现文件」14 行全部落位——`owner.rs` / `scan/mod.rs` / `tree.rs` / `dto.rs` / `archi/lib.rs` / `commands/archi/mod.rs` / `commands/mod.rs` / `bindings/mod_test.rs` / `.dev-team/architecture/config.json` / `use-archi-file-tree.ts` / `file-tree.tsx` / `architecture-canvas.tsx` / `package.json` / 两份生成物。

### 新增文件

| 文件路径 | 说明 | 关联条目 |
|----------|------|----------|
| `crates/core/archi/src/tree.rs` | 文件树投影唯一实现：`file_tree` + 四个私有 helper（见「内部拆分」）；纯读盘、零写盘、零 spawn；MUST NOT 自带归属 / 前缀 / 剥离 / 归一实现 | 函数表 `tree::file_tree` 行；D3 / D5 / D6 / D8 / D9 / D10 |
| `.dev-team/architecture/config.json` | 架构域看护配置：`{ "exclude": ["openspec/**"] }`，**无 `$schema` 键**（`RawConfig` 带 `deny_unknown_fields`，写 `$schema` 会让三条消费面同时 `Err`） | 配置表 `exclude` 行；D9；`repo-archi-model` ADDED |
| `packages/desktop/src/views/architecture/hooks/use-archi-file-tree.ts` | 树取数：三触发、零 watch / 零轮询、失败清空、跨 root 不冒充 | 函数表 `useArchiFileTree` 行；类型表 `ArchiFileTreeState` 行；D12 |
| `packages/desktop/src/views/architecture/components/file-tree.tsx` | 左栏文件树 + 分段控件：扁平派生森林、per-path 翻转展开、折叠不渲 DOM、五值标记与 tooltip、点击语义、focus 反查（多行高亮 + 滚到首条 + 零命中提示）、底部图例 | 函数表 `FileTree` / `TreeModeSwitch` 行；类型表三行；D7 / D13 / D14 / D15 / D16 |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `crates/core/archi/src/scan/owner.rs` | ① `strip_generated` → `fn strip_candidates(rel_path: &str) -> Vec<String>`（私有）：白名单**外**返回单候选（归一后的原路径）、白名单**内**返回两候选（整组剥离 `<name>` / 替换扩展名 `<stem>.<ext>`）；② `owner_of` 改为「对每个候选各跑 `longest_prefix_hits` → 按 `(fqn, 前缀长度)` 合并去重 → 取最大档 → 档内 1 / ≥2 定档」，**MUST NOT 候选短路**；③ 新增 `pub(crate) fn module_roots(&self) -> BTreeMap<&str, Vec<&str>>`；④ 新增 `pub(crate) fn contains_count(&self, path: &str) -> u32`；⑤ 文件头 doc 与剥离段的 doc 同步为「双形态并集」 | D1 / D2；`normalize` / `prefix_matches` / `OwnerIndex::build` / `Owner` 三值与 `GENERATED_MARKER` / `GENERATED_EXTENSIONS` 常量**逐字不动**；非 `.i.<ext>` 路径归属零变化 |
| `crates/core/archi/src/scan/mod.rs` | `walk_source_files` / `is_source_file` / `posix` 三处 `fn` → `pub(crate) fn`（各加一行 doc 说明「供文件树复用，语义零变化」） | 事实校准 1；`scan()` 六段、`SOURCE_EXTENSIONS`、`ImportFact` / `DependencyEdge` / `ScanInput` / `ScanOutcome` 与文件尾测试声明**逐字不动** |
| `crates/core/archi/src/dto.rs` | ① 词表区（`NodeMark` `:230` 之后、`SearchScope` 之前）新增 `FileEntryKind`（`Dir` / `File` + `ALL: [FileEntryKind; 2]` + `as_str`）；② 同区新增 `FileTreeMark`（五值 + `ALL: [FileTreeMark; 5]` + `as_str`，doc 写明「MUST NOT 复用 `NodeMark`、不进四面词汇对拍」）；③ 线面 DTO 区（`ArchiGraph` 之后）新增 `ArchiFileTree` / `ArchiFileEntry`（七字段按声明序） | D4；既有封闭词汇组（spec 记作「九组」）与全部出线 DTO 的字段名 / 顺序 / 语义**零变化**；`FileTreeMark` 不进 `tests/vocabulary_test.rs` 的四面词汇对拍（该文件零 diff） |
| `crates/core/archi/src/lib.rs` | ① 模块清单 `:31` `pub mod search;` 之后增 `pub mod tree;`；② `pub use` 段（`dto` 段尾）追加 `ArchiFileEntry, ArchiFileTree, FileEntryKind, FileTreeMark`；另起一行 `pub use tree::file_tree;` | 跨 crate 唯一出口形态沿既有 `pub use` 段；`#[cfg(test)] mod` 段零改动（`tree_test` 由 `tree.rs` 自身声明） |
| `packages/desktop/src-tauri/src/commands/archi/mod.rs` | ① 文件头 `:1-2`「五命令」→「六命令」并补 `archi_file_tree`、`*_inner` 数量 5 → 6；② `:19`「IPC 四命令契约」→「六命令」；③ `:14` 一行「其余四条 MUST NOT 写盘」→「其余五条」；④ 文件尾（`archi_drift_inner` 之后）新增 `#[tauri::command] #[specta::specta] pub fn archi_file_tree(root: String) -> Result<ArchiFileTree, String>` 与 `pub fn archi_file_tree_inner(root: &Path) -> Result<ArchiFileTree, String>`（`check_root` → `archi::tree::file_tree`）；⑤ `use` 区补 `archi::dto::ArchiFileTree` | D11；既有五命令与五个 `*_inner` 签名 / 语义逐字不变；`archi_set_gate` 仍是唯一写命令 |
| `packages/desktop/src-tauri/src/commands/mod.rs` | `all_commands!` archi 段 `:80` `archi_drift` 之后、`:81` `read_file` 之前追加 `$crate::commands::archi::archi_file_tree,` | 唯一登记面；既有条目零位移（`read_file` 仍为尾条） |
| `packages/desktop/src/views/architecture/components/architecture-canvas.tsx` | ① `import`：`useArchiFileTree`、`FileTree` / `TreeModeSwitch`；② `CanvasBody` 内增 `treeMode` state（缺省 `'file'`）与 `useArchiFileTree(root)` 解构；③ 左栏 `:196` 改为「栏容器（宽度随模式 `w-80` / `w-72`）+ `TreeModeSwitch` + 内层 `min-h-0 flex-1 flex`」；④ 内层按 D13 判三态（`archi-file-tree-error` / `archi-file-tree-loading` / `<FileTree>`）或 `<ElementTree>`；⑤ 组件 doc `:160`「三段布局（元素树 / 画布 / 抽屉）」→「左栏树（缺省文件树）/ 画布 / 抽屉」 | D13 / D16；`toNodes` / `toEdges` / `neighborhoodPositions` / `neighborhood` / `defaultFocus` / `ArchiNodeView` / `StructureErrorPanel` 分支 / `ElementDrawer` 接线 / `driftMarks` 合并 / 停摆态分支`零改动`；`ElementTree` 入参与 testid 零变化 |
| `packages/desktop/package.json` | `version` `"0.6.35"` → `"0.6.36"`（一次 patch），`scripts` / 依赖集零变化 | `src-tauri/tauri.conf.json` 经 `"version": "../package.json"` 跟随；`src-tauri/Cargo.toml` 的 crate 版本行**不跟** |
| `packages/desktop/src/types/generated/bindings.ts` | 由 `pnpm run bindings:export` **重导出**（禁手改）：`archiFileTree` 一条 typed 包装 + 四个 `export type` | 入库前经 `git diff` 人工核对 |
| `packages/desktop/src/types/generated/bindings.zod.ts` | 同批重导出：该命令的 `argsSchemas` / `resultSchemas` 表项 + 四个具名 schema | 纯接受面、零修饰；`bindings:zod-check` 纳入 `bindings:export` |

### 公共函数 / API

> 口径：Rust 侧列 `pub` / `pub(crate)` 模块级函数与公开方法；前端列跨文件导出函数。私有（`tree.rs` 四个 helper、`strip_candidates`、`longest_prefix_hits`、组件内 `buildForest` / `FileRow` / `TreeNodeList`）不入表。

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|-----------|------|------|
| `tree::file_tree` | `crates/core/archi/src/tree.rs` | 新增 | `pub fn file_tree(root: &Path) -> Result<ArchiFileTree, String>` | 唯一入口：读模型（诊断非空 → `Err`；空模型 → 全 `none`）→ 读 `exclude` → 枚举 + 排除 → 模块根表 / 归属 → 目录合成 → 四字段定档 → `path` 字节序 |
| `owner::OwnerIndex::owner_of` | `crates/core/archi/src/scan/owner.rs` | 修改 | `pub fn owner_of(&self, rel_path: &str) -> Owner` | 签名不变；体内改为「候选集合并定档」（D1），同长多命中仍 `Ambiguous` |
| `owner::OwnerIndex::module_roots` | 同上 | 新增 | `pub(crate) fn module_roots(&self) -> BTreeMap<&str, Vec<&str>>` | 归一路径 → FQN 列表（每表项字典序去重）；树侧查 `elementRoot` / `ambiguous` 与合成模块根节点 |
| `owner::OwnerIndex::contains_count` | 同上 | 新增 | `pub(crate) fn contains_count(&self, path: &str) -> u32` | 严格前缀 `"{path}/"` 的模块根元素数（按 FQN 去重、不含自身）；`path` 非归一形态时由 `normalize` 先行归一 |
| `scan::walk_source_files` | `crates/core/archi/src/scan/mod.rs` | 修改（可见性） | `pub(crate) fn walk_source_files(root: &Path) -> Vec<std::path::PathBuf>` | 私有 → `pub(crate)`；gitignore 感知 + 五剪枝语义零变化 |
| `scan::is_source_file` | 同上 | 修改（可见性） | `pub(crate) fn is_source_file(rel: &str) -> bool` | 私有 → `pub(crate)`；五扩展名判据零变化 |
| `scan::posix` | 同上 | 修改（可见性） | `pub(crate) fn posix(path: &str) -> String` | 私有 → `pub(crate)`；`\` → `/` + 剥前导 `./` 零变化（事实校准 1） |
| `commands::archi::archi_file_tree` | `src-tauri/src/commands/archi/mod.rs` | 新增 | `pub fn archi_file_tree(root: String) -> Result<ArchiFileTree, String>` | `#[tauri::command] #[specta::specta]`；薄包装 = `archi_file_tree_inner(Path::new(&root))`；只读、零写盘、零 spawn |
| `commands::archi::archi_file_tree_inner` | 同上 | 新增 | `pub fn archi_file_tree_inner(root: &Path) -> Result<ArchiFileTree, String>` | `check_root` → `archi::tree::file_tree`；命令层零领域规则 |
| `useArchiFileTree` | `packages/desktop/src/views/architecture/hooks/use-archi-file-tree.ts` | 新增 | `function useArchiFileTree(root: string \| null): ArchiFileTreeState` | `commands.archiFileTree(root)`；三触发（挂载 / `root` 变化 / 上层 `key` 重挂）；失败与换 root 都清空 `tree`；`root === null` 零 invoke |
| `FileTree` | `packages/desktop/src/views/architecture/components/file-tree.tsx` | 新增 | `function FileTree(props: FileTreeProps): React.JSX.Element` | 扁平 → 森林（目录先于文件）→ 递归行；per-path 翻转展开（默认前 2 段）；折叠不渲子树；五值标记 + tooltip；`elementRoot` / `member` 行点击 → `onFocus(owners[0])`，其余三值只切展开；`focused` 命中条目全量高亮 + 强制展开祖先链（`forcedOpen` 派生集合，**零 setState**）+ 滚到首条；零命中顶部弱提示；底部可折叠图例 |
| `TreeModeSwitch` | 同上 | 新增 | `function TreeModeSwitch(props: TreeModeSwitchProps): React.JSX.Element` | 分段控件 `data-testid="archi-tree-mode"` + 两钮 `data-mode="file"` / `data-mode="element"`（`aria-pressed` 表达选中态） |

### 类型定义

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|--------|----------|-----------|------|
| `FileEntryKind` | `crates/core/archi/src/dto.rs` | 新增 | 封闭二值 `Dir` / `File` + `ALL: [FileEntryKind; 2]` + `as_str`；`#[serde(rename_all = "lowercase")]` → 线值 `"dir"` / `"file"`；`Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type` |
| `FileTreeMark` | 同上 | 新增 | 封闭五值 `ElementRoot` / `Ambiguous` / `Member` / `ContainsElements` / `None` + `ALL: [FileTreeMark; 5]` + `as_str`（线值 `elementRoot` / `ambiguous` / `member` / `containsElements` / `none`）；`#[serde(rename_all = "camelCase")]` + 同 derive 集；doc 声明「MUST NOT 复用 `NodeMark`、MUST NOT 进四面词汇对拍、不承载漂移违例」 |
| `ArchiFileEntry` | 同上 | 新增 | 七字段按声明序 `path: String` / `name: String` / `kind: FileEntryKind` / `mark: FileTreeMark` / `owners: Vec<String>` / `contains: u32` / `exists: bool`；`#[serde(rename_all = "camelCase")]` + `specta::Type` |
| `ArchiFileTree` | 同上 | 新增 | 单字段 `entries: Vec<ArchiFileEntry>`（扁平、`path` 字节序、不产 `""` 根条目）；同 derive 集 |
| `ArchiFileTreeState` | `packages/desktop/src/views/architecture/hooks/use-archi-file-tree.ts` | 新增 | `{ tree: ArchiFileTree \| null; loading: boolean; error: string \| null }` |
| `TreeMode` | `packages/desktop/src/views/architecture/components/file-tree.tsx` | 新增 | `type TreeMode = 'file' \| 'element'`（`TreeModeSwitchProps.mode` 与 `CanvasBody` state 的共用别名） |
| `FileTreeProps` | 同上 | 新增 | `{ entries: ArchiFileEntry[]; focused: string \| null; onFocus: (fqn: string) => void }`（spec 三参，D13） |
| `TreeModeSwitchProps` | 同上 | 新增 | `{ mode: TreeMode; onChange: (mode: TreeMode) => void }` |
| `FileTreeNode` | 同上 | 新增（**文件内，不导出**） | `{ entry: ArchiFileEntry; children: FileTreeNode[] }`；`buildForest` 的返回元素（无跨文件消费者，导出会进 knip 未用面） |

### 配置

| 配置键 | 所在文件 | 新增/修改 | 值类型 | 默认值 | 说明 |
|--------|----------|-----------|--------|--------|------|
| `exclude` | `.dev-team/architecture/config.json`（新增） | 新增落位 | `string[]`（文件级 glob，须含通配符） | `[]`（文件缺失即空配置，由 `load_config` 的 `Default` 承担） | 本期唯一一条 `"openspec/**"`；由 `drift::config::RawConfig` 严格解析（`deny_unknown_fields`，只认 `ignore` / `exclude`）——**MUST NOT 写 `$schema`** |

<!-- 工作区配置子表：本变更零改动（`.dev-team/config.json`、`KNOWN_KEYS`、`config.schema.json` 镜像与一致性测试均零 diff）；架构域配置不是工作区配置，不参与镜像。 -->
<!-- 路由子表见「路由 / API 设计」；CLI 子命令零新增（`api:export` / `api:check` / `bindings:export` / `check` 脚本逐字不变）。 -->

### 编译闭合随动（**空模块，零用例**）

> 依据：`tree.rs` 底部声明 `#[cfg(test)] mod tree_test;` 后，`tree_test.rs` 缺席会让 `cargo clippy --all-targets` 报 `failed to resolve mod`（AC-16 的机械门面）。本相位只建**仅头注释、零用例、零断言**的空文件；全部用例由 test-design / test-gen / test-execution 相位填充（沿 `archi-page-agent-window` 的 `archi_knowledge_test.rs` 先例）。

| 文件路径 | 落点 | 建什么 |
|----------|------|--------|
| `crates/core/archi/src/tree_test.rs` | 与 `tree.rs` 同级 | 文件头注释（「`tree::file_tree` 的单元测试：真实 tempdir + 真实 walker，零 mock」）+ 零用例 |

### 清单守卫随动（测试文件，**不新增用例**）

> 依据：`desktop-ipc-type-bindings` MODIFIED「清单守卫随单 change 随动」是 MUST；新增 1 条命令 + 4 个出线类型会让 5 处计数与 2 组族清单当场变红。本相位只**改清单条目与计数**、不写新用例（既有用例名口径订正见下）。

| 文件路径 | 落点 | 改什么 |
|----------|------|--------|
| `src-tauri/src/bindings/mod_test.rs` | `COMMAND_WRAPPERS` `:127` / `COMMAND_NAMES` `:179` | 48 → 49：`archiFileTree` / `archi_file_tree` 插在 `archiDrift` / `archi_drift` 之后、`readFile` / `read_file` 之前 |
| 同上 | `DTO_TYPES` `:231` | 97 → 101：`ArchiFileEntry` / `ArchiFileTree` 插在 `ArchiExcludedFile` 与 `ArchiGateChange` 之间；`FileEntryKind` / `FileTreeMark` 插在 `EdgeMark` 与 `GateTier` 之间；首锚 `ActivePhase` / 尾锚 `WarningKind` 不动 |
| 同上 | `ARCHI_COMMANDS: [(&str, &str); 4]` `:1652` | 4 → 6：**补 `archi_drift`（既有漏登）+ 新增 `archi_file_tree`**；doc「架构域四命令」→「架构域命令族」 |
| 同上 | `ARCHI_DTO_TYPES: [&str; 19]` `:1660` | 19 → 23：`ArchiFileEntry` / `ArchiFileTree` 插在 `ArchiDependsOn` 与 `ArchiDiagnostic` 之间；`FileEntryKind` / `FileTreeMark` 插在 `EdgeMark` 与 `GateTier` 之间 |
| 同上 | `:1047` / `:1951` / `:2423` 三处 `COMMAND_NAMES.len()` 与 `:1048` / `:1954` / `:2426` 三处 `COMMAND_WRAPPERS.len()`；`:1108` `DTO_TYPES.len()` | 48 → 49（六处）/ 97 → 101（一处）；断言的**语义**不变，只随清单长度 |
| 同上 | `登记面_总数四十八且尾锚零位移` 用例名（`:2423` 语境） | 用例名内的「四十八」→「四十九」（**只改名字**；尾锚断言仍为 `read_file` / `readFile`，零改） |
| 同上 | 尾部追加序期望 vec `:1714-1726` | 增一条 `"archi_file_tree"`（位于 `archi_drift` 之后、`read_file` 之前） |
| 同上 | 用例名 `archi四命令在册且既有条目零位移`（`:1684`） | 用例名改符实（`archi命令族在册且既有条目零位移`）——口径订正，不新增用例 |
| 同上 | `清单遗漏即产物覆盖断言必红` `:1770` | 「取 `ARCHI_COMMANDS` 末条做副本删一条」的对照臂随族末条由 `archi_set_gate` 变 `archi_file_tree` 自动改判（断言体零改） |

---

## 数据模型 (Data Model)

### 线面 DTO（出线到 `bindings.ts` / `bindings.zod.ts`）

```
ArchiFileTree { entries: ArchiFileEntry[] }                 // 扁平、path 字节序、无 "" 根条目

ArchiFileEntry
  path     : String         repo 相对 POSIX（无前导 "./"、无尾 "/"；顶层条目即其单段名）
  name     : String         末段名（path.rsplit_once('/') 的后段，无 '/' 时等于 path）
  kind     : "dir" | "file"
  mark     : "elementRoot" | "ambiguous" | "member" | "containsElements" | "none"
  owners   : String[]       归属 FQN，字典序去重：elementRoot 恰 1 / ambiguous >= 2 / 其余 []
  contains : u32            后代模块数（严格前缀、按 FQN 去重、不含自身）；kind = file 恒 0
  exists   : bool           Dir 且为模块根时该目录是否在盘（其余恒 true）
```

字段序即声明序（`serde` + `specta` 同序）；投影零重算、零补默认值。

### 进程内中间结构（不下线、不落库）

| 结构 | 形状 | 语义 |
|------|------|------|
| 模块根表 | `BTreeMap<&str, Vec<&str>>`（`OwnerIndex::module_roots()`） | 归一路径 → FQN 列表；`len == 1` ⇒ `elementRoot`、`len >= 2` ⇒ `ambiguous`；同时是「声明但目录不存在仍出节点」的节点来源 |
| 节点集合 | `BTreeMap<String, FileEntryKind>` | 枚举文件（`File`）∪ 模块根（盘上探测：目录 → `Dir`、文件 → `File`、不在盘 → `Dir`）∪ 两者全部祖先目录（`Dir`）；`BTreeMap` 天然去重 + 有序 |
| 归属结果 | `Owner`（既有三值） | `Element { fqn, gate }` / `Ambiguous(Vec<String>)` / `None`；`gate` 不参与树的判定（树只表达归属，D17①） |

### 持久化（**零变更**）

| 面 | 现状 | 本变更 |
|----|------|--------|
| 架构模型 `models/*.toml` | 51 元素 / 113 关系 | **零改动**（`views/architecture/**` 已是单元素 `App.Frontend.Views.Architecture`；新增前端文件不建元素） |
| 架构域配置 | 无该文件 | 新增 `config.json`（单键单条）；**零 schema / 零迁移**（`RawConfig` 已是手写严格解析，无镜像文件） |
| 工作区配置 | `.dev-team/config.json` | **零改动**（`KNOWN_KEYS` 七项、`config.schema.json` 镜像、一致性测试全零 diff） |
| 会话 / 数据库 / 缓存 | — | **零改动**：树是「每次挂载重算的只读投影」，零表、零列、零缓存、零落盘 |

---

## 路由 / API 设计 (Route / API Design)

### IPC 命令（新增一条）

| 方法 | 路径 | 说明 | 输入 | 输出 | auth |
|------|------|------|------|------|------|
| IPC | `archi_file_tree` | 仓库源文件与模块根的扁平条目投影（左栏文件树数据面） | `root: String`（空白 / 缺失 / 非目录 → `Err`） | `Result<ArchiFileTree, String>` | 无（本机 IPC，与既有命令面同款） |

- 前端经生成绑定 `commands.archiFileTree(root)` 消费（零裸 `invoke` 字符串）。
- **只读**：零写盘（唯一写命令仍是 `archi_set_gate`）、零 spawn、零 watch、零轮询。
- **失败面**：结构诊断非空 → `Err`（与 `archi_module` 同口径）；配置非法 → `Err`（沿用既有显式文案）；**空模型 → `Ok`**（全条目 `mark = none`）。
- archi 段命令总数 **5 → 6**；`all_commands!` 单一登记面（builder 注册与 `generate_handler!` 同源）。

### 路由

| 方法 | 路径 | 说明 | 输入 | 输出 | auth |
|------|------|------|------|------|------|
| route | `/architecture` | **既有路由，零变化**（`root` 取壳态当前根）；左栏缺省内容由元素树改为文件树 | — | `ArchitectureView` | 无 |

- 无新增路由 / 页签；`ArchitectureView` 的 props、`archi-refresh` 与 `key={refreshTick}` 重挂语义零变化。

---

## 依赖 (Dependencies)

### 运行时依赖（**零新增**）

| 依赖 | 归属 | 用途 |
|------|------|------|
| `foundation`（既有） | `archi` | `layout::archi_config_path` / `archi_models_dir`（配置与模型定位，`tree.rs` 不出现文件名与 `openspec` 字面量） |
| `ignore`（既有） | `archi` | `walk_source_files` 的 gitignore 感知遍历（经 `scan` 单点） |
| `globset`（既有） | `archi` | `ExcludeSet` 的矩阵编译（经 `scan::exclude` 单点） |
| `serde_json`（既有） | `archi` | 架构域配置解析（经 `drift::config` 单点） |
| `specta` / `serde`（既有） | `archi` | 四个新出线类型的 derive |
| `@base-ui/react`（既有，经 `ui/scroll-area`） | 前端 | 文件树滚动区 |
| `lucide-react`（既有） | 前端 | 目录 / 文件行图标与分段控件图标（与 `dir-tree.tsx` 同一组） |

**零新增包**：`Cargo.toml`（含 `archi` crate 与 app 包）与 `package.json` 的依赖集均不变；`Cargo.lock` 零变化。

### 构建 / 测试依赖（**零新增**）

| 依赖 / 工具 | 用途 |
|-------------|------|
| `bindings:export`（`crates/tools/export-bindings`） | 生成 `archiFileTree` 包装与四类型（AC-14） |
| `server:check`（`cargo fmt --check` + `clippy --all-targets`） | 可见性提权后的未用告警 / 清单随动的编译闭合 |
| `client:check`（`vp check --fix` + `knip`） | 新组件与 hook 的导出面（`FileTreeNode` 不导出即免 knip 污染） |
| `api:check`（`archi-api-gen --check`） | 零 diff 复核（新增 `pub` 项不产生 finding，事实校准 8） |

---

## 验收标准回应

> 只给落点，不含测试策略（用例编写与门禁执行由独立相位承接）。

| AC | 落点 |
|----|------|
| AC-1 新命令可数且只读 | D11（`archi_file_tree` + `_inner`）+「修改文件」表 `commands/mod.rs` 行（archi 段 5 → 6）+「清单守卫随动」表（48 → 49、`ARCHI_COMMANDS` 4 → 6 含补登 `archi_drift`）；`archi_set_gate` 仍是唯一写命令 |
| AC-2 结构诊断非空即 `Err`、空模型 `Ok` | D3（`graph_view` 停摆判据 → `graph_diagnostics_error`）+ D9（配置非法 `Err`）+ `file_tree` 伪码的「诊断非空 → `Err`」与「`elements` 空 ⇒ 继续枚举、全 `none`」两分支 |
| AC-3 DTO 扁平且确定 | D10（`BTreeMap` 节点集合 + `path` 字节序 `sort`）+「类型定义」表 `ArchiFileEntry` / `ArchiFileTree` 行（七字段 / 一字段按声明序） |
| AC-4 `FileTreeMark` 封闭五值 | D4（五值 + `ALL` + `as_str` + 两个 `rename_all` 的选择理由）+ 不进四面词汇对拍（`tests/vocabulary_test.rs` 零改动） |
| AC-5 归属判定正确 | D2（`module_roots` / `contains_count` 落在 `OwnerIndex`）+ D6（判据顺序 ①②③）+ 事实校准 3（`ambiguous` 两条来源同分支） |
| AC-6 判定单源 | D1（剥离单点）+ D2（前缀 / 计数单点）+ D5 / D9（枚举与配置单点）+ 事实校准 1（`posix` 提权而非复制）；`tree.rs` 源码零 `starts_with` / 零 `replace(` / 零 `.i.` 字面量 |
| AC-7 A2″ 并集生效 | D1（白名单内两候选 + 合并定档，禁止短路；白名单外单候选 ⇒ 归属逐值不变）；`GENERATED_EXTENSIONS` / `GENERATED_MARKER` 原样承载白名单与零误剥面；`crates/infra/archi.i.rs` → `member of App.Backend.Infra.ArchiRuntime` 由「整组剥离」候选命中原目录形前缀；`src/dir.i.ts` 由 `None` 翻 `Element` 由「合并后取最长档」自然成立（`src/dir` 与 `src/dir.ts` 同场时后者更长） |
| AC-8 枚举面 = 漂移判定面 | D5（枚举 → 扩滤 → 排除逐条同序）+ D9（`ExcludeSet::build(config.exclude)` 复用，零新增入口）+ `scan/exclude.rs` / `exclude_test.rs` 零 diff |
| AC-9 本仓配置落位 | 「配置」表 `exclude` 行（**无 `$schema`**，理由见 D9 与 `RawConfig` 的 `deny_unknown_fields`）+ `repo-archi-model` ADDED 的连带留痕（drift `excluded` +3 / `unmatchedFiles` −3） |
| AC-10 前向防御分支 | D8（`kind` / `exists` 判定：不在盘 → `Dir` + `exists = false`；判据只看盘）+ `node_set` 把模块根无条件并入节点集合 |
| AC-11 左栏替换与分段切换 | D16（控件归属与宽度）+ D13（左栏三态）+ D14（零命中文案并列成因）+ `FileTree` 的点击语义与 focus 反查（多行高亮 / 滚到首条 / 零命中不滚） |
| AC-12 树取数独立失败 | D12（失败只落 `error`，`tree` 清空）+ D13（`archi-file-tree-error` 行内提示在栏容器，画布与抽屉照常）+ D3（停摆态整块 `StructureErrorPanel`，树不挂载——既有分支零改） |
| AC-13 零 watch 零轮询 | D12（三触发、无 `refresh` 成员、零定时器）+ D16（切换只换左栏组件，不触达取数） |
| AC-14 清单守卫随动 | 「清单守卫随动」表七行（48 → 49 / 97 → 101 / 4 → 6 / 19 → 23 + 尾部序 + 两处口径订正）+ 生成物两文件重导出 |
| AC-15 本仓实测基线 | D10（确定性排序）+ D17⑤（不做懒加载，475 节点 / 84.2 KB 已实测）；基线数字不作为 MUST，由一次性统计用例或 ad-hoc 脚本留痕 |
| AC-16 门禁 | 「依赖」节（零新增依赖）+ `package.json` 0.6.36（`Cargo.toml` 不跟）+ 「编译闭合随动」表（`tree_test.rs` 空模块） |
| AC-17 边界零 diff | D17 + 组件表「零改动」列（`drift/**` / `infra/archi/**` / `infra/agent/**` / `exclude.rs` / `api/**` / `models/**` / `decisions/**` / `.dev-team/config.json` / `element-tree.tsx` / 抽屉 / 面板 / `marks.ts` / `drift-marks.ts` / `fqn.ts`） |

---

## 待决问题 (Open Questions)

1. **`contains_count` / `module_roots` 落在 `scan/owner.rs`（超出 proposal 明列的两项改动）**：proposal 的 `owner.rs` 行只写「剥离 → 候选集 + `owner_of` 合并定档」，而 AC-6 的源码判据禁止 `tree.rs` 出现前缀匹配。本设计的落法是把严格前缀计数与模块根表放进 `OwnerIndex`（**只增不改**，`normalize` / `prefix_matches` / `build` / `Owner` 语义逐字不动）。备选是 `tree.rs` 自写 `starts_with(format!("{p}/"))`——代价是 AC-6 只能靠 review 判、不可机械判白。请确认取前者；若评审要求 `owner.rs` 改动面严格按 proposal 字面，则需把 AC-6 的判据放宽为「前缀匹配仅限 `owner.rs` 与 `tree.rs` 的 contains 计数一处」。
2. **`scan::posix` 提权（proposal 漏列）**：见事实校准 1。不提升则该 helper 必被复制一份，与 D2 的同一条纪律冲突。若评审不接受三处提权，退路是「`tree.rs` 复用 `walk_source_files` 但自写 `strip_prefix` + 反斜杠替换」——即接受一处受控的重复归一。请裁决。
3. **`FileTree` 的 loading / error 态挂载层级**：D13 依 spec Module Contract 的三参 Props，把三态行放在 `architecture-canvas.tsx` 的栏容器里（`archi-file-tree-loading` / `archi-file-tree-error` 由该文件渲染）。若评审更希望「树组件自带三态」，则 Props 需扩到五参（与 spec 表不一致，需同步改 spec 的 Module Contract）。二者都满足 AC-12，差别只在「测试查询哪个文件」。不阻塞实现。
4. **零命中提示的 testid 命名**：spec 只列了 `archi-file-tree` / `archi-file-node` / `archi-file-mark` / `archi-tree-mode`（+ 表内 `archi-file-tree-error` / `archi-file-tree-empty`），本设计另需三个查询挂钩：`archi-file-tree-loading` / `archi-file-tree-focus-miss` / `archi-file-tree-legend`。命名沿既有 `archi-*` 前缀语义，若评审要求全部进 spec 的 testid 清单，需一行追加（不改语义）。
5. **`ARCHI_COMMANDS` 补登 `archi_drift` 的连带面**：`清单遗漏即产物覆盖断言必红` 的对照臂依赖「族末条」，补登后末条变为 `archi_file_tree`；断言体零改即自动改判（本设计已登记）。若评审希望该用例显式点名末条，则需在测试相位加一条注释性断言——不影响实现。
6. **「无元素根元素在树上变空壳」的呈现细节**（R-11）：D8 保证元素根节点恒在场（`exists` 只看盘），但若某元素根的可见文件全部被排除矩阵吃掉，节点 `contains = 0`、无子行。本设计接受这一形态（图例已说明树面 = 判定面）；若日后要区分「空目录」与「全被排除」，需新增字段（`excludedCount` 一类）——不在本变更范围。
