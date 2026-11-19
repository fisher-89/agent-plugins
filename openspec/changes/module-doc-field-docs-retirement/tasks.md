# 任务: module-doc-field-docs-retirement

> 覆盖 proposal「变更范围 - 实现文件」**全量 20 行**（`model.rs`、`parse/toml.rs`、`docs.rs` + `docs_test.rs` 删除、`dto.rs`、`search.rs`、`lib.rs`、`api/paths.rs`、`api/mod.rs`、`api/contract.rs`、`api/check.rs`、`drift/report.rs`、`drift/mod.rs`、`drift/gate.rs`（按实际落点 → `infra/archi/src/runner.rs`）、`foundation/src/layout/mod.rs`、`archi_knowledge.rs`、`archi-knowledge.md`、`sdk/mount.rs`、`element-drawer.tsx`、`types/generated/bindings.ts` + `.zod.ts`、`01-app.toml`、`AGENT.md` / `CLAUDE.md`、`package.json`），零新增源码文件。
>
> **不在本 change**：数据面 2b（两份工具 crate README + 41 份契约 md——MUST NOT 顺手新建）；`$ref` ↔ `relation` 对齐判据（explore 附录 A.2）。
>
> **测试面交接**：proposal「测试文件」表所列 **19 行**（含 `docs_test.rs` 删除、末行「验收留痕 `reports/`」→ 由本文件阶段十一承接；另加 `app.test.tsx` 的 `docs: []` 载荷——事实校准 9）归测试相位——本相位**不写用例、不跑测试**；`api:export` / `api:check` / `server:check` / `bindings:export` / `client:check` 四段门禁由桌面端 static-check 承接，本文件只登记边界与自查命令。
>
> **中间态允许不编译，提交前必须绿**：字段面换面（`Element.docs` → `doc`）与 `docs.rs` 删除互为对方的编译前提，只能同一提交落地；其余阶段亦按「一次提交」聚合。
>
> 自查 cwd：仓库根（模型与文档）；`packages/desktop/src-tauri`（Rust 侧）；`packages/desktop`（`package.json` / `bindings` / 前端）。

## 阶段一：载体派生单点（`api/paths.rs`）

- [x] `is_repo_relative` 由 `fn` 提为 `pub(crate) fn`，doc 去掉 `docs::normalize_rel` 引用，改写为「repo 相对路径判据的**唯一实现**（`docs.rs` 已退役）：`/` 分隔、无绝对前缀、无盘符、无 `~`、无 `.` / `..` / 空段、非空」；函数体逐字不动
- [x] 新增 `pub fn doc_path(root: &Path, doc: &str) -> Option<PathBuf>`：体为 `normalize_root(doc)` → `is_repo_relative` 通过 → `Some(root.join(normalized))`，否则 `None`；doc 写「`doc` 字段的归一与形态判据单点（`join` 越根的安全边界）」
- [x] 新增 `pub fn carrier_path(root: &Path, element: &ArchiElement) -> Option<PathBuf>`：`ApiStatus::Declared` → `contract_path(root, &element.fqn)`；`ApiStatus::None` → `element.doc.as_deref().and_then(|doc| doc_path(root, doc))`；doc 写「**入口文档载体的派生单点**——判据面 / 纳管面 / 检索面 / 投影面共用，MUST NOT 出现第二份载体派生」
- [x] 模块 doc 第 19 行与 `contract_path` 的 doc 改述：`contract_path` 的结果同时是 `api = declared` 元素的**入口文档**
- [x] 自查 阶段一：`grep -n 'docs::' crates/core/archi/src/api/paths.rs` **零命中**；`grep -c 'pub(crate) fn is_repo_relative'` = 1；`grep -c 'pub fn doc_path'` = 1、`grep -c 'pub fn carrier_path'` = 1；`domain_of` / `directory` / `index_path` / `schema_path` / `generated_path` / `entry_candidates` / `entry_state` / `normalize_root` / `checked_root` / `is_fqn` 五段零行变化

## 阶段二：fence 语言标可见性（`api/contract.rs`）

- [x] `const FENCE_OP` / `const FENCE_TYPES` 提为 `pub(crate) const`（值逐字不变：`"api-op"` / `"api-types"`），doc 补「供启用判据与 `apiDocMissing` 复用——MUST NOT 抄第二份字面量」
- [x] `fn info_tokens(info: &str) -> Vec<&str>` 提为 `pub(crate) fn`（切分语义逐字不变：空白 / 逗号）
- [x] 自查 阶段二：`git diff -- crates/core/archi/src/api/contract.rs` 只含三处可见性改动（零逻辑行）；`grep -c 'pub(crate) const FENCE_OP\|pub(crate) const FENCE_TYPES\|pub(crate) fn info_tokens'` = 3

## 阶段三：载体事实装配与启用判据（`api/mod.rs`）

- [x] 新增 `pub enum DocFact { Absent, Missing, Present { has_api_fence: bool } }`（`#[derive(Debug, Clone, Copy, PartialEq, Eq)]`，doc 三态逐条说明）
- [x] 新增私有 `fn doc_facts(root: &Path, elements: &[ArchiElement]) -> BTreeMap<String, DocFact>`：遍历 `path` 非空元素 → `carrier_path`；**载体派生不出路径时按 `element.doc.is_some()` 分流**（`Some` ⇒ `Missing`：声明在案但形态越根，MUST NOT 归 `Absent`；`None` ⇒ `Absent`：真未声明）；路径派生成功但不在盘 ⇒ `Missing`；在盘 ⇒ `read_to_string` **一次**（读失败按 `Present { has_api_fence: false }`）→ `Present { has_api_fence: has_api_fence(&source) }`；doc 写「唯一装配点 + 每份载体读一次即弃（不缓存全文）」
- [x] 新增私有 `fn has_api_fence(source: &str) -> bool`：扫行取 ``` 之后的 info string → `contract::info_tokens` → `tokens.contains(&contract::FENCE_OP) || tokens.contains(&contract::FENCE_TYPES)`；doc 写「只判有无语言标，不判 fence 合法性（那是 `contract::parse` 的 `Err` 面）」
- [x] `ApiSnapshot` 增 `pub doc_facts: BTreeMap<String, DocFact>`（doc 写明「载体事实与契约装载同批产出」）；`ApiCheckInput` 增 `pub doc_facts: &'a BTreeMap<String, DocFact>`
- [x] `load_contracts` 起始处 `let facts = doc_facts(root, elements);`（避免与函数同名遮蔽），返回快照时携入 `doc_facts: facts`；契约装载输入源（只认 `contract_path`）与 `api = none` 跳过判据逐字不动
- [x] `pub fn enabled(root: &Path) -> bool` 改 `pub fn enabled(root: &Path, elements: &[ArchiElement]) -> bool`，体为 `doc_facts(root, elements).values().any(|fact| matches!(fact, DocFact::Present { has_api_fence: true }))`；doc 写「启用的唯一判据：任一 `path` 非空元素的入口文档载体在盘且正文含 api fence 语言标；MUST NOT 退化为『递归任意 `*.md`』，命令层 MUST NOT 另判一套」
- [x] 删除私有 `fn has_markdown(directory: &Path, depth: usize) -> bool` 与私有 `const MARKDOWN_SUFFIX`（唯一消费者已消失；`MAX_API_WALK_DEPTH` **保留**，`api/diff.rs` 仍用）
- [x] `findings` 改为「先 `let snapshot = load_contracts(root, elements)?;`，再由 `snapshot.doc_facts` 派生 `enabled`，再构造 `ApiCheckInput { …, doc_facts: &snapshot.doc_facts, enabled }` 并调 `check`」；删掉独立启用前置与第二次装配
- [x] `gating_findings` 口径改 `match element.api`：`ApiStatus::Declared` → `carrier_path(root, element).is_some_and(|path| path.is_file())`；`ApiStatus::None` → `element.doc.is_some()`；函数签名与 `element = None` 落选规则逐字不变；doc 写「纳管子集口径 = 入口文档承诺已成立；`docMissing` 两侧都不成立 ⇒ 永不纳管」
- [x] `artifact_check` 增 `let elements = elements_of(root)?;` 并在构造 `ArchiApiCheck` 时用 `enabled(root, &elements)`；三类差异计算与 `meta` 装配零改动
- [x] 自查 阶段三：`grep -n 'has_markdown\|MARKDOWN_SUFFIX' crates/core/archi/src/api/mod.rs` **零命中**；`grep -n 'pub fn enabled(root: &Path) -> bool' crates/core/archi/src/api/mod.rs` **零命中**；`grep -c 'doc_facts' crates/core/archi/src/api/mod.rs` ≥ 6（类型装配 1 / 定义 1 / 快照字段 1 / `load_contracts` 调用与携入 2 / `enabled` 1 / `findings` 2）；`grep -c 'pub fn gating_findings(' crates/core/archi/src/api/mod.rs` = 1（单点自证锚不变）

## 阶段四：判据求值重写（`api/check.rs`）

- [x] 模块头注释整段改写为「十三值 / 两支三段 / 首错短路 / pair 锚点」：写清 `none` 支三步、`declared` 支六步、pair 锚点三条与「分界不是排版而是依赖」（①–⑥ 零导出集依赖、⑦ 起需 `Resolved`）
- [x] `category_of` 加分支：`DocMissing | DocNotFound | ApiDocMissing => "doc"`（函数 doc 由「六值」改「七值」）；其余六值逐字不变
- [x] `check()` 重写为两支三段的元素锚点 + 三条 pair 锚点：
  - `api = none`：① 派生契约路径在盘（`contract_path(...).is_some_and(is_file)`）⇒ `apiUnexpected` + `continue`；② `doc_facts = Absent`（`Absent` 只对应「未声明 `doc`」——已声明而形态越根在 `doc_facts` 里已归 `Missing`）⇒ `docMissing`（文案「`api = none` 且未声明 `doc`」）+ `continue`；③ `Missing` ⇒ `docNotFound`（文案点名声明值；覆盖「路径指不到盘」与「声明值形态越根」两种成因，MUST NOT 落回 `docMissing`）
  - `api = declared`：④ `Missing | Absent` ⇒ `docMissing`（文案「派生契约 md 不在盘」+ 派生路径）+ `continue`；⑤ `Present { has_api_fence: false }` ⇒ `apiDocMissing` + `continue`；⑥ `element.doc.is_some()` ⇒ `apiUnexpected`（文案「入口文档由派生落点给出，`doc` 是第二个事实」）+ `continue`；⑦ `entry_state` 非 `Resolved` ⇒ `apiEntryMissing`（既有文案逐字保留）+ `continue`；⑧ `apiMissing`；⑨ 一致性首条命中即停（`signature → reexport → constructor`）
  - pair 锚点：`nested_cross_crate` → `ref_missing` → `shadowing`，仍在元素循环之外、每锚点一条/对、MUST NOT 引用 `Model.relations`
- [x] `missing_contracts` 删「`api = None` 形态①（`declared` 而契约文件缺失）」分支，只剩形态②（声明 op / `$defs` 而代码无）；去掉不再需要的 `input` / `root` 形参，改名注释「② 只剩『有约定无兑现』」
- [x] `signature_mismatch` / `reexport_drift` / 构造器检查三处由「`findings.push`」改「返回 `Option<ArchiApiFinding>`」，调用点取首个 `Some` 后 `break`（首错）；三条判据自身的判据逻辑（归一比对、纯 `$ref` alias 排除、`is_constructor` 双向排除）逐字不动
- [x] 新增 `docMissing` / `docNotFound` / `apiDocMissing` 三处文案构造函数（`docMissing` 按成因两段文案；三者 `target` 均为 `None`、`occurrences` 为空）
- [x] 反向自查：`api = none` 元素零 `apiEntryMissing`（入口判据只服务 code 面）；`apiUnexpected` 两形态各自单向、MUST NOT 对同一事实双报
- [x] 自查 阶段四：`grep -c 'ArchiApiFindingKind::' crates/core/archi/src/api/check.rs` 覆盖 13 值（逐值至少一处构造）；`grep -c 'ArchiApiFindingKind::ApiUnexpected'` 恰 2（两形态各一处构造）；`grep -c 'pub fn category_of'` = 1；`grep -n 'DocMissing\|DocNotFound\|ApiDocMissing'` 只在 `doc` 分类与构造处（无第四处消费）

## 阶段五：档位、词汇与状态推导（`drift/report.rs` + `drift/mod.rs` + `infra/archi/src/runner.rs`）

- [x] `ApiFindingSeverity` 加 `Error`（`as_str` = `"error"`；`ALL: [ApiFindingSeverity; 3]`）；doc 由「封闭二值」改「封闭三值」
- [x] `ArchiApiFindingKind` 加 `DocMissing` / `DocNotFound` / `ApiDocMissing` 三变体（带 doc），`as_str` 补 `"docMissing"` / `"docNotFound"` / `"apiDocMissing"`，`ALL` 由 `[_; 10]` 扩 `[_; 13]`（顺序：原十值在前，新增三值在后）
- [x] `severity()` 改为 `ApiUndeclared => Off`、`DocNotFound | ApiMissing | ApiSignatureMismatch => Error`、`_ => Warn`（通配臂保留并留注释「新增变体落哪一档须显式决定」）；`ApiMissing` 的 doc 收窄为「有约定无兑现」
- [x] 新增 `pub fn has_error_tier(findings: &[ArchiApiFinding]) -> bool`（任一 `severity() == Error`）；doc 写「`derive_status` 与 `passed()` 的唯一共用判据，MUST NOT 各写一份」
- [x] 新增 `impl ArchiDriftReport { pub fn passed(&self) -> bool }`：`self.errors.is_empty() && self.model.structural_errors.is_empty() && !has_error_tier(&self.api_findings)`；doc 写「门禁布尔单点（`status == errors ⟺ passed == false`）」
- [x] `drift/report.rs` 模块头「四组词汇」表述校准：`WarningKind`（旧文档警告）已删除、`ArchiApiFindingKind` 为十三值
- [x] `drift/mod.rs`：① 模块 doc 与 `REPORT_VERSION` doc 把「契约在案的判据进 `status`、不进 `passed`」改写为「入口文档承诺成立的判据进 `status` **与** `passed`」；② `derive_status` 的 `errors` 分支追加 `|| report::has_error_tier(gating)`（`warnings` 分支保持「`warnings[]` 非空 **或** 纳管子集非空」）；③ `derive_status` 的 doc 改写为三档分档说明
- [x] `crates/infra/archi/src/runner.rs` 的 `let passed = report.errors.is_empty() && report.model.structural_errors.is_empty();` 改为 `let passed = report.passed();`（**仅此一行**；`compose_text` / 摘要 / `fault` / 报告落盘降级面不动）
- [x] 自查 阶段五：`grep -c 'pub const ALL: \[ArchiApiFindingKind; 13\]'` = 1；`grep -c 'pub const ALL: \[ApiFindingSeverity; 3\]'` = 1；`grep -n 'pub fn has_error_tier\|pub fn passed'` 各 1 处；`grep -n 'REPORT_VERSION: u32' crates/core/archi/src/drift/mod.rs` 值仍 `3`；`git diff -- crates/infra/archi/src/runner.rs` 恰 1 行 +/1 行 -

## 阶段六：线面与检索面换面（`dto.rs` + `search.rs` + `lib.rs` + 删除 `docs.rs` / `docs_test.rs`）

- [x] `dto.rs` 删 `WarningKind`（含 `as_str`）、`DocKind`、`DocSource`、`DocRef`、`ArchiWarning` 五类型族；删 `use crate::docs;`
- [x] `ArchiModuleDetail`：删 `docs` 与 `warnings` 两字段，在 `children` 与 `api` 之间新增 `pub doc: Option<String>`（doc 写「单值、仅 `api = none` 元素有值；`declared` 恒 `null`，其入口文档走 `api.contractPath`」）
- [x] `module_detail` 装配：删 `docs_outcome` 与 `warnings` 映射；`doc` 取 `(element.api == ApiStatus::None).then(|| element.doc.clone()).flatten()`；其余邻域 / `children` / `api` 装配逐字不动
- [x] `ArchiSearchResult.docs: Vec<DocRef>` → `pub doc: Vec<String>`（doc 写「`path` 非空元素的在盘入口文档载体，路径字典序」）；`dto::search` 装配 `outcome.doc`
- [x] `ArchiElement` 在 `entry` 之后新增 `pub doc: Option<String>`（**授权进场字段**：模型字段的拷贝，与 `entry` 同型——单值、缺省 `null`、显式字段不省略、`api = declared` 恒 `null`；doc 写「入口文档指针（模型字段的拷贝；仅 `api = none` 元素有值，`declared` 的入口文档由派生 `contract_path` 给出）」）；`to_element` 补 `doc: element.doc.clone()`，并把 `fn to_element` 提为 `pub(crate) fn`（检索面复用同一份投影取 `doc` 事实，MUST NOT 另拼第二份 `ArchiElement`）；`ArchiElementBrief` MUST NOT 加 `doc`
- [x] 删除 `pub fn linked_doc_paths(root: &Path) -> Result<BTreeSet<String>, String>`（连同其 doc 与不再需要的 `BTreeSet` 导入，若无其它消费者）
- [x] `search.rs`：删 `use crate::docs;`；`SearchOutcome.docs: Vec<DocRef>` → `pub doc: Vec<String>`；语料改为 `graph.model().elements.iter().filter(|e| !e.path.is_empty()).filter_map(|e| paths::carrier_path(root, &dto::to_element(e))).filter(|p| p.is_file()).map(|p| api::display(root, &p))`（经元素投影取 `doc`，不自建归一；排序：路径字典序，按键去重）；`matches_doc(doc: &str, tokens: &[String])` 吃路径串；`file_stem` 迁入本文件；scope 分支改 `SearchScope::Doc`
- [x] 删除 `crates/core/archi/src/docs.rs` 与 `crates/core/archi/src/docs_test.rs`
- [x] `lib.rs`：删 `pub mod docs;` 与 `#[cfg(test)] mod docs_test;`；`pub use dto::{…}` 删 `DocKind` / `DocRef` / `DocSource` / `WarningKind`；模块 doc 第 1 行「文档关联」改「入口文档」、词汇条去掉「文档警告」
- [x] 自查 阶段六：`grep -rn 'crate::docs\|docs::' --include=*.rs crates packages/desktop/src-tauri/src` **零命中**；`grep -rn 'DocRef\|DocKind\|DocSource\|WarningKind\|ArchiWarning\|linked_doc_paths' --include=*.rs crates src | grep -v '_test\.rs'` **零命中**；`ls crates/core/archi/src/docs*.rs` 不存在；`grep -c 'pub doc:' crates/core/archi/src/dto.rs` = 3（`ArchiElement` / `ArchiModuleDetail` / `ArchiSearchResult`）；`grep -c 'pub(crate) fn to_element' crates/core/archi/src/dto.rs` = 1；`grep -c 'Docs' crates/core/archi/src/dto.rs` **零命中**（`SearchScope::Doc` + 文案已换词）

## 阶段七：字段面换面（`model.rs` + `parse/toml.rs` + `01-app.toml` **同一提交**）

- [x] `model.rs`：`pub docs: Vec<String>` → `pub doc: Option<String>`（doc 写「入口文档指针：单值、repo 相对 POSIX；只有 `api = none` 元素可写；`declared` 的入口文档恒为派生 `contract_path`」）；`api` 字段 doc 补「入口文档载体的开关（`declared` → 派生落点 / `none` → `doc` 声明）」
- [x] `parse/toml.rs`：`RawElement` 的 `docs: Vec<::toml::Spanned<String>>` → `#[serde(default)] doc: Option<String>`（doc 写「`doc` 是单值；数组形态由 serde `invalid type` 归 `invalidValue`」）；`to_model` 的字段传递同步
- [x] `.dev-team/architecture/models/01-app.toml` 删第 7 行 `docs = ["./README.md", "./AGENT.md"]`（其余字节不变；`App` 不写 `doc`——它是 `package`、无 `path`）
- [x] 反向核对：白名单删 `docs` 后，任一份模型写回 `docs = [..]` 必须 `unknownField`（本仓 `01-app.toml` 已同批删行，`archi_graph` 零诊断、53 元素 / 120 关系不变、`modelHash` 变化）
- [x] 自查 阶段七（射程见 design 事实校准 15：`db_tool.rs` 局部变量 / `store/describe.rs` 的 specta `docs` / `specta-zod-patched` 的 `docsrs` 不属射程，`api/extract/rust.rs:35` 散文按 OQ-3 例外）：`grep -rn '\bdocs\b' --include=*.rs packages/desktop/src-tauri/crates/core/archi/src packages/desktop/src-tauri/crates/core/foundation/src/layout packages/desktop/src-tauri/crates/infra/agent/src | grep -v '_test\.rs'` 只余 `api/extract/rust.rs:35` 一处；`grep -n 'docs' .dev-team/architecture/models/*.toml` **零命中**；`grep -c 'doc: Option<String>' crates/core/archi/src/model.rs crates/core/archi/src/parse/toml.rs` 各 1

## 阶段八：布局派生族、知识文本与工具描述（`foundation` + `infra/agent`）

- [x] `foundation/src/layout/mod.rs`：删 `const DECISIONS_DIR_NAME`、`pub fn archi_decisions_dir`、`pub fn archi_decisions_rel_dir`（各连整段 doc；`archi_dir` / `archi_models_*` / `archi_graph_*` / `archi_config_path` / `is_archi_rel_path` / `Layout` / `resolve` / `ReportSlot` 逐字不动）
- [x] `archi_knowledge.rs`：`build` 的格式串删「；决策记录目录：{}」一段，实参序少一个 `archi_decisions_rel_dir()`；保留「模型目录 / 契约资产 / 契约索引」三处方位；模块 doc 与 `build` doc 的方位清单同步（四面词表注入 / `join` / `<>` 拒绝 / 降级 `None` 逐字不动）
- [x] `archi-knowledge.md`：① 字段面 `docs` → `doc` 并补两条纪律（单值；只有 `api = none` 可写，`declared` 的入口文档 = 派生落点 `<models>/<Domain>/<FQN>.md`）；② 边界段「`api` 与 `entry` 都是指针」扩为三个指针（`api` / `entry` / `doc`）；③ 写面与可见性「只改架构域模型与 `docs` 关联文档」→「只改架构域模型与 `doc` 所指入口文档」；④ 示例 TOML 改两段形态：`api = "none"` + `doc = "..."` 的元素一段、declared 元素零 `doc` 行（以注释指路派生落点）
- [x] `sdk/mount.rs`：`use archi::dto::{ElementKind, GateTier}`（去 `WarningKind`）；`archi_module` 描述删「关联文档（四字段条目）」与「`docNotFound` 警告（只报不拦）」，改为「入口文档单值 `doc`（仅 `api = none` 元素有值；`declared` 走 `api.contractPath`）」；`archi_search` 描述「四字段文档条目」→「入口文档路径」，`scope` enum → `["elements", "doc", "all"]`
- [x] 自查 阶段八：`grep -rn 'archi_decisions' --include=*.rs crates | grep -v '_test\.rs'` **零命中**（`*_test.rs` 里的用例由测试相位同批清理）；`grep -n '决策记录目录' crates/infra/agent/src/*.rs` **零命中**；`grep -n 'docs' crates/infra/agent/src/archi-knowledge.md` **零命中**；`grep -n '"elements", "doc", "all"' crates/infra/agent/src/sdk/mount.rs` = 1；`grep -n 'WarningKind' crates/infra/agent/src/sdk/mount.rs` **零命中**

## 阶段九：前端抽屉与出线绑定（`element-drawer.tsx` + `types/generated/**`）

- [x] `element-drawer.tsx`：删 `DocRow` 与 `DocsSection`（含 `archi-doc-item` / `archi-docs-empty` 两 testid）
- [x] 新增 `EntryDocSection`：`api = none` 时渲染——`doc` 非空 ⇒ 一条「入口文档」行（`FileViewer` + 通用 `read_file` 打开）+ `data-testid="archi-entry-doc"`；`doc = null` ⇒ 空态文案「无入口文档」+ `data-testid="archi-entry-doc-empty"`；`api = declared` ⇒ **返回 `null`**（入口文档由契约段 `ContractPathRow("契约")` 承担，MUST NOT 出现两条）
- [x] 调用点段序改为「概览 → 契约（`ContractSection`）→ 入口文档（`EntryDocSection`）→ 豁免 → 改档入口」；`ElementDrawer` 的组件 doc 段序说明同步
- [x] 执行 `pnpm bindings:export` 重导 `packages/desktop/src/types/generated/bindings.ts` 与 `bindings.zod.ts` 并入库（`DocRef` / `DocKind` / `DocSource` / `WarningKind` / `ArchiWarning` 离场；`doc` 在 **IPC 可见面两处**进场：`ArchiModuleDetail.doc` / `ArchiElement.doc`——两者都本就在清单内，清单成员不增删、只有生成物内容变化；DTO 面第三处 `ArchiSearchResult.doc` 与 `SearchScope` 是 agent 工具专用（不进 `bindings.ts`），换面由 `archi_search` 工具响应体现）
- [x] `ArchiElement` 字面量夹具补 `doc`（字段面进场的机械面，事实校准 16）：`api/extract/mod_test.rs`、`api/paths_test.rs`、`scan/mod_test.rs`、`scan/owner_test.rs`、`tests/pilot_test.rs`、`src/lib/ipc-contract.test.ts`、`element-tree.test.tsx`、`use-archi-graph.test.ts`、`change-detail-view.test.tsx`（`doc: None` / `doc: null`，判定零变化）
- [x] 自查 阶段九：`grep -n 'DocsSection\|DocRow\|archi-docs-empty' packages/desktop/src/views/architecture/components/element-drawer.tsx` **零命中**；`grep -n 'entry-doc' …element-drawer.tsx` ≥ 2；`grep -n 'DocRef\|WarningKind\|ArchiWarning' packages/desktop/src/types/generated/bindings.ts` **零命中**；重复执行 `pnpm bindings:export` 后 `git diff -- packages/desktop/src/types/generated/` 零新增行（幂等）

## 阶段十：仓库根文档纪律节与版本行

- [x] `AGENT.md` `## 模块 API 契约` 新增一条要点（置于 `api = none` 那条之后）：`doc` 是入口文档指针（单值），**只有 `api = none` 元素可写**；`declared`（缺省）元素的入口文档 = 派生 `<models>/<Domain>/<FQN>.md`，写了即 `apiUnexpected`
- [x] `AGENT.md` 同节把「契约在案元素的判据进 status、不进 passed」整条改写为「**入口文档承诺已成立的 `error` 档判据进 `status` 与 `passed`**：`docMissing`（入口文档未就位）是 warn 且永不纳管，`status = errors` 时 `passed = false`；十三值判据与『两支三段首错 + pair 锚点』定义见 `desktop-archi-api-check`」
- [x] `CLAUDE.md` 同步该节两处改动，两份该节正文**逐字相同**（要点数 5 → 6，其余三条零改动）
- [x] `packages/desktop/package.json` 的 `"version"`：`0.6.52` → `0.6.53`（相对落地时 `HEAD` 现值 +1 patch；`tauri.conf.json` 与 `src-tauri/Cargo.toml` 零改动）
- [x] 自查 阶段十：`diff <(sed -n '/^## 模块 API 契约/,/^## Coding Guidelines/p' AGENT.md) <(sed -n '/^## 模块 API 契约/,/^## Coding Guidelines/p' CLAUDE.md)` **零输出**；节内 `grep -c '^- '` = 6；`grep -n '"version": "0.6.53"' packages/desktop/package.json` = 1；`grep -n '\.\./package.json' packages/desktop/src-tauri/tauri.conf.json` = 1

## 阶段十一：收尾核对与留痕（不重复执行桌面端静态步骤）

- [x] 第六段读数留痕（`reports/`）：`apiFindings` 逐 `kind` 统计 = `docMissing 43`（41 = `declared` 且派生契约 md 不在盘 / 2 = `api = none` 且未声明 `doc`）、`apiEntryMissing 0`、`apiMissing 0`、其余十值各 0，`status = clean`、`passed = true`，分母 44、`api = none` 2、契约在案 1 在案；写明「不是新增 43 条债」（41 改名 + 2 首入面）
- [x] 模型读数留痕：53 元素 / 120 关系 / `diagnostics` 空不变；`modelHash` **必然变化**（`01-app.toml` 删一行）且如实记录，MUST NOT 写成「`modelHash` 不变」
- [x] 生成物与租约面留痕：`api:export` 后 `.dev-team/architecture/graph/INDEX.toml` / `models/Infra/*.schema.json` / `.i.*` 逐字节零变化（`doc` 不是生成物）；`api:check` 三列空（由桌面端 static-check 执行，本相位只登记边界）
- [x] 语料守护核对：`crates/core/archi/src/api/**` sidecar 仍恰 **13** 份、`tests/corpus/api/` 仍零存在；十六份 `corpus/drift/expected/*.json` 的变化逐份留痕「哪一条因首错短路而变化」（重录归测试相位，`DESKTOP_GOLDEN_REWRITE=1` 显式执行 + 人工确认）
- [x] 零扩散核对：`git status --porcelain` 的改动集 ⊆ design D18 白名单；出现 `api/extract/**`、`api/emit/**`、`api/render.rs`、`api/schema.rs`、`api/diff.rs`、`infra/archi/src/api_artifacts.rs`、`drift/config.rs`、`tree.rs`、`scan/**`、`contract-panel.tsx` / `use-archi-api-check.ts` / `use-archi-module.ts` / `drift-marks.ts` / `architecture-canvas.tsx`、`archi_tool.rs`、`sdk/policy.rs`、`models/0[2-6]-*.toml`、`models/Infra/**`、`decisions/**`、`openspec/specs/**`（9 个 delta 之外）任一 diff 即回退
- [x] 边界核对：`git status --porcelain` 中 MUST NOT 出现任何**新建**的 `README.md`（两个工具 crate）或 `models/<Domain>/*.md`（41 份契约）——数据面 2b 不在本 change
- [x] 版本纪律核对：`packages/desktop/package.json` 相对落地时 `HEAD` 恰 +1 patch；`src-tauri/tauri.conf.json` 仍 `"version": "../package.json"`；`src-tauri/Cargo.toml` 的 `[package] version` 逐字不变
