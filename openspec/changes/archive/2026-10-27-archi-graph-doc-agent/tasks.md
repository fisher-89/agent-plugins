# 任务: archi-graph-doc-agent

> 覆盖 proposal「变更范围 - 实现文件」39 项（`crates/core/workflow/src/prompts/*.md` 7 份逐项计）+ 设计补入的 3 项（`crates/infra/agent/src/sdk/runner.rs`、`crates/infra/agent/src/sdk/mod.rs`，见 design 事实校准 2；`packages/desktop/src/views/architecture/lib/fqn.ts`，见 design 新增文件表）= **22 新增 + 25 修改**。
>
> **本相位不写测试**：`crates/core/archi/{corpus,tests}` / `src/*_test.rs` / `src/views/architecture/**/*.test.tsx` / `commands/archi/mod_test.rs` 的用例与装置、既有测试文件对新签名与新枚举变体的随动补丁，全部归 test-design / test-gen / test-execution 相位。本相位只为**可编译**承担两项机械随动：`lib.rs` 的 `#[cfg(test)] mod *_test;` 声明（先建仅含头注释的空测试模块，否则 `cargo fmt` 报 `failed to resolve mod`）与既有测试文件里 `SessionToolset` / `LoopTurn` / `archi_matrix` 相关的**编译级**补丁（不新增任何 `#[test]`）。
>
> 依赖序要点：**阶段一/二**（workspace 接线 + foundation 派生）解锁整条 Rust 链；**阶段三**（`model` + 词汇）解锁四～九；**阶段四～九**（parse / graph / docs / search / diff / edit）互相只按表中序单向依赖；**阶段十**（`dto` 投影入口）依赖三～九全绿；**阶段十一**（命令面）依赖十；**阶段十二/十三**（agent 面：词表 → 装配 → 定义 → 执行体 → 路由 → policy）内部严格顺序，且 `archi = { workspace = true }` 必须在阶段一到位；**阶段十四**（归档两处）与 Rust 侧其余互不依赖，可并行；**阶段十五**（生成绑定重导出）必须在阶段十一之后、阶段十六～十八之前（前端编译依赖生成类型：五命令与 `Archi*` 类型在 `bindings.ts` 里出现后，前端任务才可 `tsc` 通过）；**阶段十六～十八**（前端入口 → 画布域 → 两挂载点）按序推进，画布域必须先于两挂载点；**阶段十九**（prompt）与**阶段二十**（版本 / 边界自查）为收尾。
>
> 中间态约定：阶段十三让 `LoopTurn` 长出必填字段 `archi`，此后既有 `loop_test.rs` 的 `LoopTurn {}` 字面量会缺字段（编译级随动，见上）；阶段十八让 `RunControlPanel` / `ArchiveConfirmCard` 长出 `root` / `changeId` 入参，其调用点与既有测试构造器同批补齐。
>
> 自查命令 cwd 除特别标注外均为 `packages/desktop/src-tauri`；前端自查 cwd 为 `packages/desktop`。
>
> 不预铺：无 db 迁移 / 新表、无 `config.schema.json` 变更、无 `tauri.conf.json` 与 `capabilities/default.json` 变更、无 `crates/infra/watch/**` 变更、无 `crates/infra/vcs/**` 新能力、无 `archi_config_path`、无模块接口协议字段。

## 阶段一：workspace 接线与 crate 骨架

- [x] 修改 `packages/desktop/src-tauri/Cargo.toml`：`[workspace.dependencies]` 增 `archi = { path = "crates/core/archi" }`、`toml = "1"`、`toml_edit = "0.25"`、`petgraph = "0.8"`、`ignore = "0.4"`（按字母序插入既有条目之间；四者版本取 `Cargo.lock` 既有值，**零新增包**）
- [x] 新建 `packages/desktop/src-tauri/crates/core/archi/Cargo.toml`：`[package] name = "archi"` / `version = "0.1.0"` / `edition = "2021"`；`[dependencies]` = `foundation` + `serde` / `serde_json` / `specta` + `toml` / `toml_edit` / `petgraph` / `ignore`（全 `{ workspace = true }`）；doc 注释写明「纯读 core crate：零 Tauri / 零 store / 零 spawn / 零 `workflow` / 零 `config`」
- [x] 修改 `packages/desktop/src-tauri/Cargo.toml`：`[workspace] members` 增 `"crates/core/archi"`（插在 `"crates/core/agent"` 之后保持字母序）；壳 `[dependencies]` 增 `archi = { workspace = true }`，附一句注释「命令面薄包装：投影、诊断、diff 与待落盘产物全在 archi」
- [x] 新建 `crates/core/archi/src/lib.rs` 骨架：模块 doc（四个消费面同源、单点构建 / 投影零改写 / 词表对拍三条纪律）+ `pub mod model; pub mod parse; pub mod graph; pub mod docs; pub mod search; pub mod diff; pub mod edit; pub mod dto;`
- [x] 修改 `crates/infra/agent/Cargo.toml`：`[dependencies]` 在 `checks` 行之后增 `archi = { workspace = true }` + 两行注释（「agent 工具执行体复用 `archi::dto` 投影：三方一致性机械前提，MUST NOT 另拼 JSON」/「理由与 `agent-runtime → checks`（glob 方言单点）同构」）
- [x] 自查 阶段一：`cargo tree -p archi | grep -E "tauri|store|workflow|orchestration|config "` **零命中**；`cargo tree -p workflow | grep archi` **零命中**；`cargo tree -p archi -i foundation` 显示单边；`grep -n "archi" Cargo.toml` 恰三处（members / workspace.dependencies / 壳 dependencies）

## 阶段二：foundation 架构域路径派生（`crates/core/foundation/src/layout/mod.rs`）

- [x] 常量组（`:3-25`）追加三个**私有**常量并各带 doc：`ARCHITECTURE_DIR_NAME = "architecture"`（架构域子目录名，唯一持有者）/ `MODELS_DIR_NAME = "models"` / `DECISIONS_DIR_NAME = "decisions"`
- [x] 文件尾（`change_baseline_reports_in` 之后、`#[cfg(test)]` 之前）追加 `pub fn archi_dir(root: &Path) -> PathBuf`：`root.join(DOMAIN_DIR_NAME).join(ARCHITECTURE_DIR_NAME)`（doc：架构域根派生单点，纯推导零 IO，不存在的 root 亦正常返回）
- [x] 同处追加 `pub fn archi_models_dir(root: &Path) -> PathBuf` 与 `pub fn archi_decisions_dir(root: &Path) -> PathBuf`：一律 `archi_dir(root).join(<子目录常量>)`（**MUST NOT** 二次拼 `architecture` 段）
- [x] 同处追加 `pub fn is_archi_rel_path(rel: &str) -> bool`：`\` → `/` 归一 → 剥前导 `./` 与尾部分隔符 → 按 `/` 切段 → 判 `segments.len() >= 2 && segments[0] == DOMAIN_DIR_NAME && segments[1] == ARCHITECTURE_DIR_NAME`（doc：`openspec/architecture` 与 `openspec/architecture/models/x.toml` 均命中、`openspec/archived/...` 不命中；core 侧归档冲突分流的唯一判据）
- [x] 自查 阶段二：`grep -n "architecture\|\\bmodels\\b\|decisions" crates/core/foundation/src/layout/mod.rs` 命中**仅**三处常量定义与派生函数体（无其他字面量）；`Layout` struct 与 `resolve` / `config_path` / `domain_dir_name` `git diff` 为空；`cargo check -p foundation` 通过

## 阶段三：AST 与封闭词汇（`model.rs` + `dto.rs` 词汇面）

- [x] 新建 `crates/core/archi/src/model.rs`：`Model { elements: Vec<Element>, relations: Vec<Relation> }` / `Element`（`fqn` / `kind: ElementKind` / `gate: GateTier` / `path: Vec<String>` / `docs: Vec<String>` / `summary: Option<String>` / `metadata: BTreeMap<String, String>` / `file: String` / `line: u32`）/ `Relation`（`from` / `to` / `description` / `file` / `line` / `span: Option<SourceSpan>`）/ `SourceSpan { start, end }` / `Diagnostic`（`kind` / `message` / `file` / `line` / `column: Option<u32>`）/ `ParseOutcome { model, diagnostics }`；各 `derive Debug, Clone, PartialEq, Eq`（`Diagnostic` 额外 `Hash` 便于对拍）；doc 写明 `line == 0` 表示位置未知（design D5）、关系 `span` 为占位
- [x] 同文件增 `impl Model { pub fn element(&self, fqn: &str) -> Option<&Element> }`（线性扫描；模型规模按元素数百计）
- [x] 新建 `crates/core/archi/src/dto.rs` 的词汇面：`ElementKind` / `GateTier`（`Default = Off`）/ `DiagnosticKind`（九值）/ `WarningKind`（`docNotFound`）/ `DocKind` / `DocSource` / `EdgeMark` / `NodeMark` / `SearchScope`，各 `derive Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type` + `#[serde(rename_all = …)]`（小写 / camelCase 按 design 类型表逐项）；`impl SearchScope { pub fn parse(raw: &str) -> Result<SearchScope, String> }`（三值，其余 `Err` 并列出合法值）
- [x] `model.rs` 顶部 `use crate::dto::{DiagnosticKind, ElementKind, GateTier};`（词汇单点在 `dto.rs`，design D4）
- [x] 自查 阶段三：`grep -n "pub enum" crates/core/archi/src/dto.rs` 恰九处；`DiagnosticKind` 恰九个变体且线值与 spec 词表逐字一致（`syntaxError` / `missingField` / `invalidValue` / `unknownField` / `duplicateElement` / `cycle` / `dependsOnAncestor` / `unknownReference` / `orphanHierarchy`）；`grep -n "enum ElementKind\|enum GateTier" crates/core/archi/src/model.rs` **零命中**（词汇不双写）；`cargo check -p archi` 通过

## 阶段四：解析层（`parse/mod.rs` + `parse/toml.rs`）

- [x] 新建 `crates/core/archi/src/parse/mod.rs`：`pub trait SourceParser { fn parse(source: &str) -> ParseOutcome; }`（关联函数无接收者，doc：单文件契约、`file` 字段留空由聚合层回填）+ `pub struct ParseOutcome` 复用 `model`
- [x] 同文件实现 `pub fn parse_dir(root: &Path) -> ParseOutcome`：`foundation::layout::archi_models_dir(root)` → `fs::read_dir`（缺席 / 不可读即返回空模型零诊断）→ 只收 `.toml` → 按**文件名**字典序排序 → 逐文件 `fs::read_to_string`（读失败 → `syntaxError` 诊断，`message` 前缀「模型文件读取失败」，design D18）→ `TomlParser::parse` → 诊断 `file` 回填该文件的 repo 相对路径（`strip_prefix(root)` + `\` → `/`）→ 元素 / 关系按文件序 + 声明序合并 → 跨文件重复 FQN 检出（`BTreeMap` 已见集合；命中即 `DiagnosticKind::DuplicateElement`，`file` / `line` 取字典序靠后者）
- [x] 新建 `crates/core/archi/src/parse/toml.rs`：私有 `RawDocument { element: BTreeMap<toml::Spanned<String>, RawElement>, relation: Vec<RawRelation> }`、`RawElement { kind: ElementKind, gate: GateTier, path: Vec<toml::Spanned<String>>, docs: Vec<toml::Spanned<String>>, summary: Option<String>, metadata: BTreeMap<String, String> }`、`RawRelation { from: toml::Spanned<String>, to: toml::Spanned<String>, desc: Option<String> }`，全部 `#[derive(Deserialize)] #[serde(deny_unknown_fields)]`（`element` / `relation` 加 `#[serde(default)]`）
- [x] 同文件实现 `pub struct TomlParser;` + `impl SourceParser for TomlParser`：`toml::from_str::<RawDocument>` 成功 → 逐元素产 `Element`（`line = line_of(source, key_span.start)`，`path` / `docs` 保原文不归一）+ 逐关系产 `Relation`（`line = line_of(source, from_span.start)`，`span = Some(SourceSpan::from(from_span))`）；失败 → 单条诊断（`message = err.to_string()` 原文含 line/column/片段/caret；`line` / `column` 由 `err.span()` 换算，`span()` 为 `None` 时 `line = 0` / `column = None`）
- [x] 同文件实现错误归类私有 `fn classify(message: &str) -> DiagnosticKind`：含 `unknown variant` 或 `invalid type` → `InvalidValue`（值域违例：`kind` / `gate` 非法取值与 `metadata` 非字符串值同档，spec「非字符串值即 `invalidValue`」/ D19）；含 `unknown field` → `UnknownField`；含 `missing field` → `MissingField`；含 `duplicate key` → `DuplicateElement`；其余 → `SyntaxError`（design D6）
- [x] 同文件实现 `pub(crate) fn line_of(source: &str, byte: usize) -> u32`（`1 + source[..min(byte, len)].matches('\n').count()`，char 边界安全回退）与 `pub(crate) fn column_of(source: &str, byte: usize) -> u32`（同行前缀按 **char** 计数 + 1）
- [x] 自查 阶段四：`grep -n "regex\|split('\\n').nth" crates/core/archi/src/parse/*.rs` 零命中（**MUST NOT** 二次扫文本反推行列）；`grep -c "deny_unknown_fields" crates/core/archi/src/parse/toml.rs` 恰三处；`grep -n "unwrap()\|expect(" crates/core/archi/src/parse/*.rs` 零命中（生产路径不 panic）；`cargo check -p archi` 通过

## 阶段五：图单点与四类结构诊断（`graph.rs`）

- [x] 新建 `crates/core/archi/src/graph.rs`：`pub struct ModelGraph { model: Model, node_of: BTreeMap<String, NodeIndex>, fqn_of: Vec<String>, graph: petgraph::graph::DiGraph<(), ()>, out_edges: BTreeMap<String, Vec<usize>>, in_edges: BTreeMap<String, Vec<usize>> }`（非 `Clone`，doc：唯一构建点，四消费面 MUST NOT 再建图）
- [x] 同文件实现 `pub fn build(model: Model) -> ModelGraph`：元素建节点（`node_of` / `fqn_of` 双表）；关系建边（仅两端皆已声明者入图；未定义引用的边**不入图**，留给诊断）+ `out_edges` / `in_edges` 下标表
- [x] 同文件实现 `pub fn model(&self) -> &Model`、`pub fn depends_on(&self, fqn) -> Vec<&Relation>`、`pub fn depended_by(&self, fqn) -> Vec<&Relation>`（均按 `(from, to)` 字典序）、`pub fn children(&self, fqn) -> Vec<&str>`（FQN 前缀一级 + 字典序）、`pub fn neighborhood(&self, fqn) -> Neighborhood<'_>`
- [x] 同文件实现 `pub fn diagnostics(&self) -> Vec<Diagnostic>`：① `orphanHierarchy`（遍历元素，逐级剥 `.` 段，任一祖先 FQN 无声明即一条，`file` / `line` 取该元素声明）② `cycle`（`petgraph::algo::tarjan_scc`，`len > 1` 或 `len == 1` 且自环；`message` 含环路径 `A -> B -> A` 形态，`file` / `line` 取环上字典序首个元素的声明）③ `dependsOnAncestor`（边 `from` 是 `to` 的后代，即 `from` 以 `to + "."` 起手；`message` 指明元素与祖先）④ `unknownReference`（边的 `from` 或 `to` 不在 `node_of`；`message` 指明引用方与缺失 FQN，`file` / `line` 取该关系声明）。四条**不短路**（一次列全），顺序固定为 ①②③④
- [x] 自查 阶段五：`grep -n "DiGraph\|tarjan_scc" crates/core/archi/src/*.rs` 只在 `graph.rs` 命中（单点）；`grep -rn "\.unwrap()\|\.expect(" crates/core/archi/src/graph.rs` 零命中；`grep -n "Neighborhood" crates/core/archi/src/graph.rs` 覆盖定义与 `neighborhood` 返回值；`cargo check -p archi` 通过

## 阶段六：文档关联（`docs.rs`）

- [x] 新建 `crates/core/archi/src/docs.rs`：`pub struct DocsOutcome { docs: Vec<DocRef>, warnings: Vec<DocWarning> }`、`pub struct DocWarning { element: String, path: String }`
- [x] 同文件实现私有归一 `fn normalize_rel(raw: &str) -> Option<String>`：剥前导 `./`、`\` → `/`、去尾分隔符、空串 → `None`（跳过该条）
- [x] 同文件实现 repo 相对路径**双重拦**：`fn is_repo_relative(&str) -> bool`（绝对前缀 / UNC / 盘符 `C:` / `~` 起始 / `.`、`..`、空段一律拒收）+ `fn resolved_within(root, relative) -> Option<PathBuf>`（判据通过后 `join` 再复核仍在 `root` 之下）；`normalize_rel`、`docs` 逐条收集、`scan_adjacent` 目标解析与 `title_of` 取文全部经此两道（`root.join(绝对路径)` 会替换 base，放行即成任意文件读取通道）
- [x] 同文件实现 `pub fn element_docs(graph: &ModelGraph, root: &Path, fqn: &str) -> DocsOutcome`：① `explicit`：元素 `docs` 逐条归一 → `root.join(path)` 存在即条目、不存在即 `DocWarning`（不进 `docs`）② `adjacent`：元素 `path` 逐条归一 → `ignore::WalkBuilder::new(root.join(entry))`（`.git_ignore(true)` 缺省、跳过隐藏 / `node_modules` / `target`）收集 `*.md`（`path` 指向**单个文件**时按精确匹配、不经递归；指向目录时递归），排除内置谓词命中的目录（`__tests__` / `tests` / `test` / `node_modules` / `target` / `dist` / `build` / `.git` / `.svn` / `.hg`）与 `foundation::layout::is_archi_rel_path` 命中的路径 ③ `text`：`foundation::layout::archi_decisions_dir(root)` 下 `*.md` 全读，正文含该 FQN 即命中（`kind = Adr`、`source = Text`）
- [x] 同文件实现条目组装与排序：按 `path` 去重取**最强**来源（`Explicit > Adjacent > Text`）、`kind` 由落地路径决定（`decisions/` 内恒 `Adr`，其余 `Md`）、`title` 取首个非空行若为 ATX 标题（`#` 起手剥 `#` 与空白）否则取文件名去扩展名；排序 = 来源强度 → `path` 字典序（design D17）
- [x] 同文件实现 `pub fn all_docs(graph: &ModelGraph, root: &Path) -> DocsOutcome`：`decisions/` 正文**只读一次**并按元素 FQN 命中分发（避免 O(元素 × ADR) 次读盘），逐元素合入 `explicit` / `adjacent` 后全量去重 + `path` 字典序
- [x] 自查 阶段六：`grep -rn "openspec\|specs/" crates/core/archi/src/docs.rs` 零命中（不碰 specs，路径全经 `foundation`）；`grep -n "WalkBuilder" crates/core/archi/src/docs.rs` 恰一处（遍历单点）；文档正文**不进** `DocRef`（`grep -n "content\|body" crates/core/archi/src/docs.rs` 不出现于 `DocRef` 字段面）；`cargo check -p archi` 通过

## 阶段七：轻量检索（`search.rs`）

- [x] 新建 `crates/core/archi/src/search.rs`：`pub struct SearchOutcome { element_fqns: Vec<String>, docs: Vec<DocRef> }`
- [x] 同文件实现私有 `fn tokenize(raw: &str) -> Vec<String>`：按非字母数字切 + camelCase / 数字边界切 → 小写 → 去重保序（doc：`AuthToken` → `auth` / `token`）
- [x] 同文件实现 `pub fn search(graph: &ModelGraph, root: &Path, query: &str, scope: SearchScope) -> Result<SearchOutcome, String>`：`tokenize` 结果为空 → `Err("query 不能为空")`；元素侧匹配对象 = `fqn` / `summary` / `metadata` 各值（小写后**包含全部段**即命中，design D16）；文档侧经 `all_docs` 取全量后按 `path` 与文件名匹配；`scope` 收窄（`Elements` → `docs` 恒空、`Docs` → `element_fqns` 恒空）；输出按 FQN 字典序 / 文档稳定序
- [x] 自查 阶段七：`grep -n "read_to_string\|fs::read" crates/core/archi/src/search.rs` 零命中（**不读正文**）；`grep -n "specs\|openspec" crates/core/archi/src/search.rs` 零命中；`grep -n "HashMap<String, Vec\|index\|cache" crates/core/archi/src/search.rs` 零命中（无倒排无缓存）；`cargo check -p archi` 通过

## 阶段八：结构 diff（`diff.rs`）

- [x] 新建 `crates/core/archi/src/diff.rs`：`pub struct ModelDiff { elements_added: Vec<String>, elements_removed: Vec<String>, relations_added: u32, relations_removed: u32, gate_changes: Vec<GateChange> }`、`pub struct GateChange { fqn: String, from: GateTier, to: GateTier }`
- [x] 同文件实现 `pub fn model_diff(base: &Model, next: &Model) -> ModelDiff`：元素身份 = `fqn`（`BTreeSet` 差集，字典序清单）；关系身份 = `(from, to)`（多重集合差，计数即可）；`gate` 变更 = 同名元素档位不等（含缺省 `Off` 的显式比较）；**文件与行号不参与身份**
- [x] 同文件实现 `impl ModelDiff { pub fn is_empty(&self) -> bool }`（三组皆空）
- [x] 自查 阶段八：`grep -n "file\|line" crates/core/archi/src/diff.rs` 不出现于身份键（仅注释可提）；`cargo check -p archi` 通过

## 阶段九：gate 写回纯函数（`edit.rs`）

- [x] 新建 `crates/core/archi/src/edit.rs`：`pub struct GateWrite { file: PathBuf, content: String }`
- [x] 同文件实现 `pub fn set_gate(source: &str, fqn: &str, gate: GateTier) -> Result<String, String>`：`source.parse::<toml_edit::DocumentMut>()`（失败即 `Err` 带 parser 原文）→ 取 `doc["element"][fqn]`（缺席 → `Err` 附**现有 FQN 清单**，由表格键枚举）→ 写 `doc["element"][fqn]["gate"] = toml_edit::value(gate.as_str())`（无该行则插入，design D20）→ `doc.to_string()` → **自证可解析**：重跑 `parse::toml::TomlParser::parse` 要求零诊断且该元素档位回读等于目标值，否则 `Err` 且不产出文本
- [x] 同文件实现 `pub fn plan_set_gate(root: &Path, fqn: &str, gate: GateTier) -> Result<GateWrite, String>`：`parse_dir(root)` 得有诊断即 `Err`（结构错模型不写，与「写坏模型 = 读出错」同构）→ `model.element(fqn)` 定位 `file` → `root.join(file)` 读源文本 → `set_gate` → 返回 `GateWrite { file: 绝对路径, content }`；**零写盘**
- [x] 自查 阶段九：`grep -n "fs::write\|fs::rename\|File::create" crates/core/archi/src/**/*.rs crates/core/archi/src/*.rs` 零命中（`archi` 全程不写盘）；`grep -n "replace(" crates/core/archi/src/edit.rs` 零命中（**禁字符串替换**）；`cargo check -p archi` 通过

## 阶段十：三面投影（`dto.rs` 投影面 + `lib.rs` 出口）

- [x] 扩充 `crates/core/archi/src/dto.rs` 的 DTO 面：`DocRef` / `ArchiGraph` / `ArchiElement` / `ArchiRelation` / `ArchiDiagnostic` / `ArchiWarning` / `ArchiElementBrief` / `ArchiModuleDetail` / `ArchiDependsOn` / `ArchiDependedBy` / `ArchiSearchResult` / `ArchiChangeSummary` / `ArchiGateChange`，全部 `#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]` + `#[serde(rename_all = "camelCase")]`；可选字段**显式序列化为 `null`**（不省字段，与 IPC 对齐，R-3）
- [x] 同文件实现私有 `fn load(root: &Path) -> Loaded`（`parse_dir` → `ModelGraph::build` → `diagnostics = parse_diagnostics ++ graph.diagnostics()`）与私有 `fn to_element(&Element) -> ArchiElement` / `fn to_relation(&Relation) -> ArchiRelation` / `fn to_diagnostic(&Diagnostic) -> ArchiDiagnostic`（**只裁剪**：不补默认值、不推导、不重算）
- [x] 同文件实现 `pub fn graph_view(root: &Path) -> Result<ArchiGraph, String>`：有诊断 → `elements` / `relations` 空 + `diagnostics` 全量（停摆）；无诊断 → 完整图（元素 FQN 序、关系 `(from,to)` 序、`mark = NodeMark::None` / `EdgeMark::None`、`reason = None`）
- [x] 同文件实现 `pub fn module_detail(root: &Path, fqn: &str) -> Result<ArchiModuleDetail, String>`：有诊断 → `Err`（`message` = 逐条 `kind @ file:line` 拼行，使 agent 看出「模型坏了」而非「没有这个模块」）→ 元素缺席 → `Err`（附元素总数提示）→ `neighborhood` 三项 + `docs::element_docs` + `warnings`（`DocWarning` → `ArchiWarning{kind: DocNotFound, …}`）
- [x] 同文件实现 `pub fn module_overview(root: &Path) -> Result<Vec<ArchiElementBrief>, String>` 与 `pub fn search(root: &Path, query: &str, scope: SearchScope) -> Result<ArchiSearchResult, String>`（后者的元素条目由 `search::SearchOutcome.element_fqns` + `model.element` 组 `ArchiElementBrief`）
- [x] 同文件实现 `pub fn change_summary(main_root: &Path, next_root: &Path) -> Result<Option<ArchiChangeSummary>, String>`：两侧 `parse_dir` → `model_diff` → `next` 侧诊断随行；`diff.is_empty() && next 侧诊断空` → `None`；**两侧** `archi_models_dir` 均缺席 → `None`
- [x] 同文件实现 `pub fn linked_doc_paths(root: &Path) -> Result<BTreeSet<String>, String>`（`all_docs` 的 `path` 集）
- [x] 修改 `crates/core/archi/src/lib.rs`：doc 补「单点构建 / 投影零改写 / 词汇全量对拍」三条机械纪律与四条消费面；`pub use dto::{ArchiChangeSummary, ArchiGraph, ArchiModuleDetail, ArchiSearchResult, DiagnosticKind, DocRef, DocSource, DocKind, EdgeMark, ElementKind, GateTier, NodeMark, SearchScope, WarningKind};` + `pub use model::{Diagnostic, Element, Model, Relation};`；末尾追加各模块 `#[cfg(test)] mod *_test;` 声明（测试模块建成仅含头注释的空文件，否则 `cargo fmt` 报 `failed to resolve mod`）
- [x] 自查 阶段十：`grep -n "unwrap_or_default\|unwrap_or(" crates/core/archi/src/dto.rs` 零命中（可选字段不填占位）；`grep -c "specta::Type" crates/core/archi/src/dto.rs` ≥ 22（九词汇 + 十三 DTO）；`grep -n "ModelGraph::build" crates/core/archi/src/*.rs` 恰一处（`dto.rs` 的 `load`——单点构建）；`cargo check -p archi` 通过

## 阶段十一：命令面五命令（`src-tauri/src/commands/archi/`）

- [x] 新建 `packages/desktop/src-tauri/src/commands/archi/mod.rs`：模块 doc（五命令清单、唯一写命令 `archi_set_gate`、三件事纪律、零领域规则、空白 root 一律 `Err`）+ 私有 `fn check_root(root: &Path) -> Result<(), String>`（`fs::metadata` 失败 / 非目录 → `Err`，沿 `commands/config` 同款文案）+ 私有 `fn write_atomic(path: &Path, content: &str) -> Result<(), String>`（同目录 `<name>.archi-tmp` 写入 + `fs::rename` 覆盖；失败清临时文件）
- [x] 同文件实现 `archi_graph` / `archi_graph_inner`、`archi_module` / `archi_module_inner`、`archi_read_doc` / `archi_read_doc_inner`（`linked_doc_paths` 成员校验 → 越界 `Err` → `fs::read_to_string` 只读）、`archi_set_gate` / `archi_set_gate_inner`（`plan_set_gate` → `write_atomic`）、`archi_change_summary` / `archi_change_summary_inner`（`stores.for_root` → `find_change_record` → `status == Archived` 时 `worktree = None` → `dto::change_summary`）；五条 `#[tauri::command] #[specta::specta]` 薄包装只做参数转换与错误映射
- [x] 修改 `packages/desktop/src-tauri/src/commands/mod.rs`：模块清单增 `pub mod archi;`（字母序首位）；`all_commands!` 在 `env::app_environment` 之后追加五条 `$crate::commands::archi::{archi_graph, archi_module, archi_read_doc, archi_change_summary, archi_set_gate}`（尾部追加，既有条目零位移）
- [x] 自查 阶段十一：`grep -rn "fn archi_" src/commands/archi/mod.rs` 命中十条（五命令 + 五 inner）；`grep -n "archi::" src/commands/archi/mod.rs` 只出现 `archi::edit` / `archi::dto`（命令层零领域规则，无 `archi::graph` / `parse` 直调）；`grep -rn "fs::write\|fs::rename" src/` 只命中 `commands/archi/mod.rs` 的 `write_atomic`；`cargo check` 通过

## 阶段十二：注入词表与装配矩阵（`core/agent` + `infra/agent`）

- [x] 修改 `crates/core/agent/src/runner.rs`：`SessionToolset`（`:44-49`）增 `Archi { module: bool, search: bool }`（doc：两工具封闭集上的子集表达；serde camelCase additive，旧线缺字段读 `None`；新增第三工具须同改词汇与对拍语料）；`SessionInjections` doc 补 archi 面一句
- [x] 修改 `crates/infra/agent/src/compose.rs`：`assemble_injections`（`:22`）签名改 `(provenance: &SessionProvenance) -> Result<agent::SessionInjections, String>`；explore 臂（`source == EXPLORE_SOURCE`）逐字保留；change 臂 → 私有 `fn change_phase_role(source_ref: &str) -> Option<(&str, &str)>`（按 `/` 切，`len >= 4`，末段 `parse::<u32>()` 成功才认，取尾三段）→ 私有 `fn archi_matrix(phase: &str, role: &str) -> Option<SessionToolset>`（7 行：`proposal/executor` → `{module:false, search:true}`；`dev-design/executor|evaluator` → 两者；`test-design/executor` → `{false,true}`；`implement/executor`、`code-review/evaluator`、`acceptance/evaluator` → `{true,false}`；其余 `None`）→ 命中行返回 `SessionInjections { preamble: None, tools: None, toolset: Some(..) }`，未命中 / 解析失败 / 其余来源 → `SessionInjections::default()`
- [x] 同文件调用点（`:210`）改为 `assemble_injections(&provenance)`
- [x] 自查 阶段十二：`grep -n "archi_matrix\|change_phase_role" crates/infra/agent/src/compose.rs` 各命中定义 + 调用；矩阵恰 7 行（`grep -c "=>" crates/infra/agent/src/compose.rs` 人工核对矩阵臂）；`grep -n "EXPLORE_TOOLS" crates/infra/agent/src/compose.rs` 六名逐字未变；装配层仍对 explore 六名做 `TOOL_NAMES` 成员校验（archi 面经枚举天然封闭，零自由字符串）；`cargo check -p agent-runtime` 通过（既有 `compose_test` 的调用点随动归测试相位，本阶段若编译受阻先补 `&SessionProvenance` 字面量，不新增用例）

## 阶段十三：工具定义、执行体、路由与档位

- [x] 修改 `crates/infra/agent/src/sdk/tools.rs`：增 `pub(crate) const ARCHI_MODULE_TOOL: &str = "archi_module"` 与 `ARCHI_SEARCH_TOOL: &str = "archi_search"`（doc 指向 `archi_tool` 执行体与矩阵单源）
- [x] 同文件 `definitions`（`:47`）把 `if injections.toolset == Some(Explore)` 改为 `match &injections.toolset`：`Some(Explore)` → 追加笔记工具（逐字不动）；`Some(Archi { module, search })` → 按位追加两条定义（`archi_module`：`{ "element": string（可选，省略返回全元素薄概览） }`，`required: []`；`archi_search`：`{ "query": string（必填）, "scope": string enum [elements, docs, all]（缺省 all） }`，`required: ["query"]`）；`None` → 零追加
- [x] 同文件 `input_path`（`:187`）首行增显式臂：`name == ARCHI_MODULE_TOOL || name == ARCHI_SEARCH_TOOL` → `None`（doc：两工具无路径参数，不进沙箱改写）
- [x] 同文件 `cap_result`（`:214`）可见性改 `pub(crate) fn`（零行为变化），doc 补「`archi_tool` 复用同一 L1 上限，禁第二份实现」
- [x] 新建 `crates/infra/agent/src/sdk/archi_tool.rs`：`pub(crate) struct ArchiContext { root: PathBuf }` + `pub(crate) fn new(root: PathBuf) -> Self` + `pub(crate) async fn execute(&self, name: &str, input: &Value) -> Result<String, String>`：`archi_module` 有 `element` → `archi::dto::module_detail`，无 `element` → `archi::dto::module_overview`；`archi_search` → `SearchScope::parse(input["scope"] 缺省 "all")` + `archi::dto::search`；未知名 → `Err`；输出 `serde_json::to_string`（紧凑，节省上下文）后过 `tools::cap_result`；坏模型 / 参数缺失 → `Err`（`is_error` 回灌，不 panic）
- [x] 修改 `crates/infra/agent/src/sdk/mod.rs`：模块清单按字母序插 `pub(crate) mod archi_tool;`（`archi_tool` 的唯一挂载点，同位先例 `explore_note`；本文件其余行与 `#[cfg(test)]` 段零改动）——漏挂载则 `archi_tool.rs` 不参与编译，`loop` / `runner` 的引用随即失败
- [x] 修改 `crates/infra/agent/src/sdk/loop.rs`：`LoopTurn` 增 `pub archi: Option<ArchiContext>`（doc：per-turn 铸造随轮携带，`None` = 未装配——非 Archi 会话恒 None，调用以 `is_error` 回灌）；执行臂（`:464`）改三分派（笔记工具 → `explore_note`；两 archi 名 → `turn.archi`，`None` 时 `("专用工具执行上下文未装配".to_owned(), true)`；其余 → `tools::execute`）；守卫链注释补一句「两 archi 工具为读面：`input_path` 显式 `None` 臂使沙箱原样返回，`protect::evaluate` 分派表 `_ => None`，二者零介入」
- [x] 修改 `crates/infra/agent/src/sdk/runner.rs`：`use crate::sdk::archi_tool::ArchiContext;`；泵起播按 `open.injections.toolset` 铸造 `archi`（`Some(SessionToolset::Archi { .. })` → `Some(ArchiContext::new(open.ctx.workspace_root.clone()))`，其余 `None`）；`LoopTurn` 字面量补 `archi` 字段
- [x] 修改 `crates/infra/agent/src/sdk/policy.rs`：`READONLY_TOOLS`（`:23`）扩为 `[&str; 6]` 并追加两 archi 名；`WRITE_TOOLS` / `EXECUTE_TOOLS` / `allows` 零改动；文件头 doc 补「两个 archi 只读工具在 `default` 档亦放行」
- [x] 自查 阶段十三：`grep -rn "archi" crates/infra/agent/src/sdk/` 覆盖 tools / archi_tool / loop / runner / policy 五文件与 `sdk/mod.rs` 的挂载行（**勿漏模块挂载**）；`grep -n "mod archi_tool" crates/infra/agent/src/sdk/mod.rs` 命中一处；`grep -n "serde_json::to_string_pretty" crates/infra/agent/src/sdk/archi_tool.rs` 零命中（紧凑出线）；`grep -n "to_string_pretty\|MAX_RESULT_BYTES" crates/infra/agent/src/sdk/archi_tool.rs` 只出现 `tools::cap_result` 调用（无第二份上限）；`cargo check -p agent-runtime` 通过

## 阶段十四：归档编排两处（`core/orchestration`）

- [x] 修改 `crates/core/orchestration/src/steps.rs`：在 `run_archive_preflight` 之后增 `pub(crate) fn check_rebase_halfway(vcs: &dyn ArchiveVcsPort, worktree: &str) -> Result<(), String>`：`vcs.worktree_snapshot(Path::new(worktree))` → `rebase_head.is_some()` → `Err(rebase_halfway_message(worktree))`；探测 `Err` → `Err(探测失败文案 + 两条出路)`（fail-closed，design D13；**前置 = worktree 目录在场**——目录缺失由 walker 的存在性分支处置，故 fail-closed 只覆盖「目录在场而探测读不出」，函数 doc 明写该前提）；再增 `fn rebase_halfway_message(worktree: &str, error: Option<&str>) -> String`（三要素：worktree 绝对路径 / `git rebase --continue` 后重跑 / `git rebase --abort` 后重跑；探测失败形态先要求核对目录在场与 git 可读、**不把「进目录执行 rebase」当唯一动作**；**总长 ≤ 200 字符**，design D14）
- [x] 修改 `crates/core/orchestration/src/walker.rs` `step_preflight`（`:2276`）：在 payload 到手与 worktree 目录存在性分支之后、`settle_archive_node(passed)` 之前插入半程挡回——`if let Some(worktree) = payload.worktree.as_deref().filter(|path| Path::new(path).is_dir()) { if let Err(reason) = check_rebase_halfway(ctx.vcs, worktree) { settle_archive_node(.. failed(diagnose_brief(&reason)) ..); return Err(Terminal::failed(reason)); } }`（**目录不在场即不探测**（design D13）：worktree 被手动删除且分支已合入时上面的存在性分支已放行，此处若仍探测会把 snapshot 的「不是仓库」错误 fail-closed 成半程，挡回合法放行态；`cursor.preflight` 不被赋值，后续步不启动）
- [x] 修改同文件 `step_resolve_conflicts`（`:2621`）：在 `let mut pending = conflicts;`（`:2651`）之后、A 快照（`:2655`）之前插入模型冲突分流——`if pending.iter().any(|path| foundation::layout::is_archi_rel_path(path)) { return Err(fail_archive_step(ctx, trace, cursor, kind, archi_conflict_message(&worktree, &pending))); }`；同文件增私有 `fn archi_conflict_message(worktree: &str, conflicts: &[String]) -> String`（worktree 绝对路径 + 冲突文件数（含前两个路径）+「解完模型冲突后重跑归档（未自动 abort，rebase 半程保留于 worktree）」；**总长 ≤ 200 字符**）
- [x] 自查 阶段十四：`grep -n "is_archi_rel_path" crates/core/orchestration/src/*.rs` 只在 `walker.rs` 命中（判据单点，core 零 `architecture` 字面量：`grep -rn "architecture" crates/core/orchestration/src/ | grep -v _test.rs` 零命中）；`grep -n "rebase_abort" crates/core/orchestration/src/walker.rs` 不新增调用点（模型冲突臂零 abort）；`grep -n "std::process\|Command::new" crates/core/orchestration/src/{steps,walker}.rs` 零命中（零 spawn）；两条文案逐条 `chars().count() <= 200` 人工核对；`cargo check -p orchestration` 通过

## 阶段十五：生成绑定重导出（前置：前端全部任务依赖本步）

- [x] `packages/desktop/src` 执行 `pnpm run bindings:export`（cargo 经 `crates/tools/export-bindings` 重导出 `src/types/generated/bindings.ts`）：五命令 + 全部 `Archi*` / `DocRef` / `SearchScope` / 九词汇枚举 + `SessionToolset` 新变体（`"archi" | "explore"` 判别形态）**禁手改**
- [x] 自查 阶段十五：`pnpm run bindings:check` 零 diff；`grep -n "archiGraph\|archiModule\|archiReadDoc\|archiChangeSummary\|archiSetGate" packages/desktop/src/types/generated/bindings.ts` 五条俱在；`grep -n "ArchiGraph\|ArchiModuleDetail\|EdgeMark\|NodeMark\|docNotFound" packages/desktop/src/types/generated/bindings.ts` 覆盖；再跑一次导出确认幂等（连续两次零 diff）

## 阶段十六：前端入口（路由 + 侧栏）

- [x] 修改 `packages/desktop/src/routes.tsx`：import `ArchitectureView`（`./views/architecture/architecture-view`）；在 `/config` 之后插 `<Route path="/architecture" element={<ArchitectureView root={root} />} />`；doc 注释补「/architecture 根取壳态当前根，URL 不携根段」
- [x] 修改 `packages/desktop/src/components/app-sidebar.tsx`：`PageNavGroup` 在「探索」项与「配置」项之间插一项（`active={pathname === '/architecture'}` + `icon={<Network />}`（lucide-react）+ `label="架构"` + `testId="nav-architecture"` + `to="/architecture"`），并更新组注释为 `[基础信息] [变更] [探索] [架构] [配置]`
- [x] 自查 阶段十六：`grep -n "nav-architecture\|/architecture" packages/desktop/src/routes.tsx packages/desktop/src/components/app-sidebar.tsx` 各命中；既有五项 testid（`nav-info` / `nav-changes` / `nav-explores` / `nav-config` / `nav-agents` / `nav-agent`）逐字未变

## 阶段十七：画布域（hooks → marks → 四组件 → 视图）

- [x] 新建 `packages/desktop/src/views/architecture/hooks/use-archi-graph.ts`：`useArchiGraph(root)`——`useEffect([root])` 挂载 / 根变化取数、`useCallback` 手动刷新（返回 `refresh`）、`root === null` 不发 invoke、`error` 承接 rejection；doc 写明「三触发取数、零 watch 零轮询（`desktop-archi-canvas` 显式取数）」
- [x] 新建 `packages/desktop/src/views/architecture/hooks/use-archi-module.ts`：`useArchiModule(root, fqn)`（`fqn` 变化重取）+ `useArchiDoc(root)`（`load(path)` 展开才发、`reset()` 清态）
- [x] 新建 `packages/desktop/src/views/architecture/lib/marks.ts`：`export interface EdgeVisual { stroke: string; dasharray: string | null }` + `edgeVisual(mark: EdgeMark | string): EdgeVisual`（`none`/未知 → `var(--chart-2)` 实线；`unfulfilled` → `var(--muted-foreground)` + `6 4`；`crossKind` → `var(--warn)` + `4 3`；`exempt` → `var(--muted-foreground)` 实线）+ `nodeDot(mark: NodeMark | string): string`（`unfulfilled` → `'🟡'`，其余 → `'⚪'`）；doc：词汇封闭、词汇外退化为中性且不抛错
- [x] 新建 `packages/desktop/src/views/architecture/lib/fqn.ts`：`export function shortName(fqn: string): string`（按 `.` 切段取末段，空数组回退原 FQN）；doc 写明「元素树节点与画布节点共用同一取法——两处各写一份会各自演化，同一元素在树与画布上呈现不同名字」
- [x] 新建 `packages/desktop/src/views/architecture/components/structure-error-panel.tsx`：逐条 `kind` + `message` + `file:line`（`line === 0` 只渲 `file`）+ 「修好后再看」引导；`data-testid="archi-error-panel"` / `archi-error-row`（零图数据、零降级样式类）
- [x] 新建 `packages/desktop/src/views/architecture/components/element-tree.tsx`：按 FQN 前缀派生层级（父 = 已声明的最长前缀，无父即根）→ 递归渲染可展开节点；节点状态点取 `nodeDot(element.mark)`（`data-testid="archi-tree-node"` / `archi-node-mark`）；空模型 / 停摆态渲染零节点
- [x] 新建 `packages/desktop/src/views/architecture/components/element-drawer.tsx`：概览（`kind` / `path` / `gate` / `modelFile:line`，`data-testid="archi-drawer"` / `archi-drawer-position`）+ `docs` 列表（按强度序，`data-testid="archi-doc-item"`）+ ADR 条目（展开即 `useArchiDoc().load(path)`，正文经 `<Markdown remarkPlugins={REMARK_PLUGINS}>` 渲染，`data-testid="archi-adr-body"`；`exempt` 边 reason 行）+ `mode === 'incremental'` 时 `gate` 三档选择 + 提交（`commands.archiSetGate`，成功后 `onChanged()` 触发父级重取，失败行内错误 `data-testid="archi-gate-error"`）+ `mode === 'full'` 时「改档需要开一个 change」提示（零可提交控件）；本期**零**「相邻报告」区块
- [x] 新建 `packages/desktop/src/views/architecture/components/architecture-canvas.tsx`：入参 `{ root: string | null; mode: 'full' | 'incremental' }`；`useArchiGraph` 取数 → `diagnostics.length > 0` → 全或无停摆（只渲 `StructureErrorPanel`，元素树零节点、画布零节点零边）；否则 `ReactFlowProvider` + `<ReactFlow>`（节点渲染为**本文件内的局部组件**——不新建节点文件；节点 / 边经 `marks.ts` 上色）+ 左 `ElementTree` + 右 `ElementDrawer` 三段布局；缺省只渲聚焦元素 1-hop（聚焦缺省 = 首个顶层元素（FQN 无 `.`）；邻域 = 直连出边 / 入边 + 直接子元素，聚焦切换用本地 state，**零取数**）；本地纯函数 `neighborhoodPositions`（聚焦居中、出向右、入向左、子元素下方，网格堆叠）与 `toNodes` / `toEdges`（`data-testid="archi-canvas"` / `archi-canvas-empty` 空态）
- [x] 新建 `packages/desktop/src/views/architecture/architecture-view.tsx`：页头（标题 + 刷新按钮 `data-testid="archi-refresh"`）+ `<ArchitectureCanvas root={root} mode="full" />`；doc 写明挂载点 A（全量只读、显式刷新）
- [x] 自查 阶段十七：`grep -rn "watch\|setInterval\|setTimeout" packages/desktop/src/views/architecture/` 零命中（零 watch 零轮询）；`grep -rn "data-testid" packages/desktop/src/views/architecture/` 覆盖画布 / 元素树 / 抽屉 / 停摆面板四组件（测试挂钩禁样式类名）；`grep -rn "archiSetGate" packages/desktop/src/views/architecture/` **只在** `element-drawer.tsx` 且受 `mode === 'incremental'` 分支保护；`pnpm exec tsc --noEmit` 通过

## 阶段十八：两挂载点与归档确认区块

- [x] 修改 `packages/desktop/src/views/changes/change-detail-view.tsx`：① 右栏内容区改页签（复用 `@/components/ui/tabs`）：`detail-tab-flow`（现 `DetailContent`，缺省选中）与 `detail-tab-architecture`（`<ArchitectureCanvas root={archiRoot} mode="incremental" />`，**仅激活时条件挂载**）；② 增 `archiRoot` 派生（`detail.source === 'archive' || detail.worktree === null ? root : detail.worktree`，单一表达式单点）；③ `DetailActionPanels` / `RunControlPanel` 增 `root` 透传
- [x] 修改 `packages/desktop/src/views/changes/flow/run-control-panel.tsx`：① `RunControlPanel` / `ArchiveConfirmCard` 增 `root: string | null` 与 `changeId` 入参（下传）；② 新增 `ArchitectureChangeBlock`（`data-testid="run-archi-block"`）：区块内 hook 在 `changeId` 在场时调 `commands.archiChangeSummary(root, changeId)`；`summary === null` → **return `null`**（零占位：无标题、无空框、无 0 计数行）；有数据 → 元素 +N / −N（含 FQN 清单）、关系 +N / −N、`gateChanges` 逐条（`fqn`：`from` → `to`）、两个入口按钮 `run-archi-diff`（就地展开明细）/ `run-archi-preview`（就地展开 `<ArchitectureCanvas root={worktree} mode="incremental" />`）、`diagnostics` 非空 → 红字逐条 `kind` + `message` + `file:line` + 「修复后再归档」（`data-testid="run-archi-error"`，**不阻断**确认按钮）；③ 区块插入位置：`ArchiveDeltaSpecs` 之后、`ArchiveSyncSkip` 之前
- [x] 自查 阶段十八：`grep -n "archiChangeSummary" packages/desktop/src/views/changes/flow/run-control-panel.tsx` 恰一处调用（独立取数，零 `ArchivePreflight` 字段使用）；`grep -n "worktree" packages/desktop/src/views/changes/change-detail-view.tsx` 的 `archiRoot` 派生单点（不出现第二处 `detail.worktree` 判定）；归档确认面既有 testid（`run-archive-confirm-card` / `run-archive-warning` / `run-archive-delta-specs`）逐字未变；`pnpm exec tsc --noEmit` 通过

## 阶段十九：角色 prompt 七份（`crates/core/workflow/src/prompts/`）

- [x] 修改 `proposal-planner.md`：补一句 `archi_search` 用法（何时用 = 提案阶段还不知道功能归谁管；返回什么 = 元素薄条目 + 四字段文档条目）
- [x] 修改 `dev-design-planner.md` 与 `dev-design-evaluator.md`：各补两句（`archi_search` 发现 + `archi_module` 定位：邻接 / 直接子元素 / 文档引用 / `modelFile` + `modelLine` 可直接 `edit`）
- [x] 修改 `test-design-planner.md`：补一句 `archi_search`（把用例归到正确模块）
- [x] 修改 `implementation-generator.md` / `code-review-evaluator.md` / `acceptance-evaluator.md`：各补一句 `archi_module`（改动前确认模块边界 / 核对越界依赖 / 核对验收面与模型声明一致）
- [x] 自查 阶段十九：`grep -rn "archi_module\|archi_search" crates/core/workflow/src/prompts/ | cut -d: -f1 | sort -u` 恰 7 份；其余 9 份 `grep -l "archi" crates/core/workflow/src/prompts/*.md` 零命中；`grep -rn "__MCP:\|<change>\|<phase>\|{{ \|__INCLUDE:\|__MODEL_\|__DEV_TEAM_ROOT__" crates/core/workflow/src/prompts/` 零命中；`grep -rn "openspec/architecture\|architecture/" crates/core/workflow/src/prompts/` 零命中（零路径模板）；`git diff --stat crates/core/workflow/src/write/phase_table.rs` **空**（16 条 `include_str!` 与角色表逐字不变）；`cargo check -p workflow` 编译无误（本相位不跑 `cargo test`：既有 prompt 文本断言的期望值随本次文本更新归测试相位）

## 阶段二十：版本面与边界自查

- [x] 修改 `packages/desktop/package.json`：`version` `"0.5.32"` → `"0.6.0"`（单行；`src-tauri/tauri.conf.json` 经 `../package.json` 跟随；`src-tauri/Cargo.toml` 的 `[package] version` **不**随动）
- [x] 核对分层与依赖边：`cargo tree -p archi` 的 workspace 内依赖恰 `foundation`；`cargo tree -p workflow -i archi` 零命中且反向亦零；`grep -n "tauri\|store\|workflow\|orchestration\|config" crates/core/archi/Cargo.toml` 零命中；`grep -n "archi" crates/infra/agent/Cargo.toml Cargo.toml` 命中三处（两条新边 + 壳）
- [x] 核对零 spawn / 零写盘：`grep -rn "std::process\|tokio::process\|Command::new" crates/core/archi/src/` 零命中；`grep -rn "fs::write\|fs::rename\|File::create\|OpenOptions" crates/core/archi/src/` 零命中（写盘只在 `src/commands/archi/mod.rs`）
- [x] 核对路径字面量单点：`grep -rn "architecture" crates/core/archi/src/ crates/core/orchestration/src/ crates/infra/agent/src/ src/ packages/desktop/src/ --include=*.rs --include=*.ts --include=*.tsx | grep -v _test` **零命中**；`grep -rn "openspec\|config\.json" crates/core/archi/src/ crates/infra/agent/src/sdk/archi_tool.rs src/commands/archi/ packages/desktop/src/views/architecture/ | grep -v _test` 零命中（含注释）
- [x] 核对写入保护零扩张：`grep -rn "archi\|models" crates/infra/agent/src/sdk/protect/rules.rs` 零命中（内置锁面仍恰 `config.json` 一条；用户显式 glob 语义由既有判定承载）
- [x] 核对零 watch / 零轮询与零自动刷新：`grep -rn "watch_subscribe\|useEffect.*Interval" packages/desktop/src/views/architecture/` 零命中
- [x] 核对范围接缝未越界：`git status` 仅含本清单文件；`git diff --stat crates/core/config/ openspec/specs/ crates/infra/store/ crates/infra/vcs/ crates/infra/watch/ src-tauri/tauri.conf.json src-tauri/capabilities/ .github/` **全空**；`git diff --stat crates/core/workflow/src/write/phase_table.rs` **空**
- [x] 登记测试相位承接清单（不在本相位执行）：`crates/core/archi/tests/{golden_test,vocabulary_test}.rs` + `corpus/dsl/<case>/*.toml`（含 `dup-cross-file` / `orphan` 两 case）；`src/{model,graph,docs,search,diff,edit,parse/toml}_test.rs`；`foundation/src/layout/mod_test.rs` 扩展；`commands/archi/mod_test.rs`；`infra/agent` 的 `compose_test` / `tools_test` / `archi_tool_test` / `loop_test` / `policy_test`；`core/agent/lib_test.rs`；`orchestration` 的 `steps_test` / `walker_test`；前端 `views/architecture/**/*.test.tsx` + `lib/marks.test.ts` + `change-detail-view.test.tsx` / `run-control-panel.test.tsx` 扩展 + `app.test.tsx` 的 invoke 序列按 `filter` 口径核对
