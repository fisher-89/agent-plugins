# 设计: module-doc-field-docs-retirement

> **变更**: module-doc-field-docs-retirement（E-2 · `doc` 单值字段替代 docs 三源 + 十三值判据 + 三段两支求值）
> **日期**: 2026-11-18
> **基于**: `proposal.md`（病灶一至三 + 提案一至九 + AC-1…AC-11 + R-1…R-15）、`explore.md`（§0–§13 + 附录 A 二十条裁决）、`specs/` 下 9 份 delta（`desktop-archi-api-check` / `desktop-archi-api` / `desktop-archi-model` / `desktop-archi-agent` / `desktop-archi-canvas` / `desktop-archi-contract-panel` / `desktop-archi-drift` / `desktop-archi-module-docs`（整份退役）/ `repo-archi-model`）
> **性质**: **能力替换 + 判据面收紧**。`docs: Vec<String>` 与整个 `docs.rs` 三源关联能力退役，`doc: Option<String>` 单值取代且只服务 `api = none` 元素（`declared` 的入口文档恒为派生落点 `contract_path`）；判据面十值 → **十三值**、按 `api` 状态分**两条互斥支链各自首错短路** + **三条 pair 锚点**、档位二值 → **三值**、`passed` 判据首次纳入 `error` 档判据；线面 `ArchiModuleDetail.docs/warnings` 退场、`doc` **三处**进场（`ArchiModuleDetail.doc` / `ArchiSearchResult.doc` / `ArchiElement.doc`——后者是元素投影上唯一的**授权进场字段**），检索面语料改为「在盘入口文档载体」且 `scope` 词 `docs` → `doc`。
> **不含**: 数据面回填（2b：两份工具 crate README + 41 份契约 md）；`$ref` ↔ `relation` 同形对齐判据（explore 附录 A.2）。

---

## 起点实况（`HEAD` 基线实测）

| 面 | 实测 |
|---|---|
| 模型读数 | 53 元素 / 120 关系、`diagnostics` 空；`path` 非空 **44**（容器 / 包根 9 个无 `path`）；`api = "none"` **2**（`App.Backend.Tools.ExportBindings` / `App.Backend.Tools.ArchiApiGen`）；契约在案 **1**（`models/Infra/App.Backend.Infra.ArchiRuntime.md`，含 `api-op` + `api-types` fence） |
| `docs` 行 | **1 处 / 2 条**：`.dev-team/architecture/models/01-app.toml:7` 的 `docs = ["./README.md", "./AGENT.md"]`（`App` 是 `package`、无 `path`） |
| 两个工具 crate 的 README | **不存在**（`export-bindings/` 只有 `Cargo.toml` / `build.rs` / `src/`；`archi-api-gen/` 只有 `Cargo.toml` / `src/`）⇒ 第六段那 2 条 `docMissing` 是「新写」不是「回填」 |
| 字段面 | `model.rs:56` `docs: Vec<String>`；`parse/toml.rs:45` `RawElement.docs: Vec<Spanned<String>>`（`deny_unknown_fields` 白名单） |
| `docs.rs` | 325 行：三源关联（`explicit` / `adjacent` / `text`）、`DocsOutcome` / `DocWarning`、`normalize_rel` / `is_repo_relative`、`resolved_within`、`title_of`、`file_stem`、ADR 读盘、邻近扫描 + 内置排除谓词 |
| `docs.rs` 消费者 | `dto.rs`（14 处）/ `search.rs`（4）/ `infra/agent/src/sdk/mount.rs`（2，仅文档串中的 `WarningKind` 描述）/ `lib.rs`（2，模块出口）；`drift/report.rs` 实测 **零**（explore §11 记 1 处，实测为文档散文，非 `docs` 符号） |
| 判据面 | `check.rs` 690 行，单元素循环 + ⑤ `apiUnexpected` + 三条跨元素判据（`shadowing` / `ref_missing` / `nested_cross_crate`），**多处并报**（`constructor` / `reexport` / `signature` 各自独立 push） |
| 档位 | `ApiFindingSeverity { Off, Warn }`（`report.rs:159`），`severity()` 仅 `ApiUndeclared => Off`、其余 `_ => Warn` |
| `enabled` | `api/mod.rs:325`：`has_markdown(&paths::directory(root), 0)`——`<models>` 递归任意 `*.md` |
| 纳管 | `api/mod.rs:298` `gating_findings`：`element.api == Declared` ∧ `contract_path(…).is_file()` |
| `status` / `passed` | `derive_status` 在 `drift/mod.rs:338`（`gating` 非空一律 `warnings`）；`passed` 在 **`crates/infra/archi/src/runner.rs:95`**：`report.errors.is_empty() && report.model.structural_errors.is_empty()` |
| `foundation::layout` | `DECISIONS_DIR_NAME`（`:63`）/ `archi_decisions_dir`（`:155`）/ `archi_decisions_rel_dir`（`:205`）；`is_archi_rel_path`（`:180`） |
| 知识文本 | `archi_knowledge.rs:51` 派生段含「决策记录目录：{}」；骨架 `archi-knowledge.md` 字段面八项含 `docs`、示例含 `docs = ["docs/auth.md"]`、「写面与可见性」写「只改架构域模型与 `docs` 关联文档」 |
| 工具描述 | `mount.rs:229` 引用 `WarningKind::DocNotFound`；`archi_search` 的 `scope` enum = `["elements", "docs", "all"]` |
| 前端 | `element-drawer.tsx` 的 `DocRow`（`:71`）/ `DocsSection`（`:286`）/ 调用点（`:391`）；段序断言在 `element-drawer.test.tsx:799` |
| 语料守护 | `crates/core/archi/src/api/**` sidecar 恰 **13** 份（`golden_test.rs:1468` 断言）；`tests/corpus/api/` 零存在；`corpus/drift/expected/*.json` **16** 份 |
| 出线面 | `bindings.ts` / `bindings.zod.ts` 含 `DocRef` / `DocKind` / `DocSource` / `WarningKind` / `ArchiWarning` / `ArchiModuleDetail.docs` / `ArchiModuleDetail.warnings` |
| 版本 | `packages/desktop/package.json` 现值 **`0.6.52`**；`tauri.conf.json` 为 `"version": "../package.json"`；`src-tauri/Cargo.toml` 的 `[package] version = "0.1.0"` 不随动 |

---

## 事实校准（不改 spec 意图，只定实现形态）

1. **`passed` 的实际落点不是 `drift/gate.rs`（该文件不存在），而是 `crates/infra/archi/src/runner.rs:95`**。proposal 的「若 `passed` 落在此处，按实际落点改」即指向它，故 `runner.rs` 纳入变更面。同时 delta spec 明写「`LocalArchiCheck` 结论只取报告既有字段……本文件零逻辑改动，只随同源语义变化」——两者合起来给出的形态是：**判据下沉为 `ArchiDriftReport::passed()` 方法（`drift/report.rs`），`runner.rs` 只把它当既有字段读**（`let passed = report.passed();`），执行体零判定规则。
2. **`enabled` 重写会使 `has_markdown` 与 `MARKDOWN_SUFFIX` 成死代码**：两者唯一消费者是 `enabled`，随本条改写删除（私有 `const` / `fn` 不删即 `dead_code` 警告）。`MAX_API_WALK_DEPTH` **保留**——`api/diff.rs:109` 仍消费它。
3. **`artifact_check` 首次读模型 ⇒ `archi_api_check` 的错误面多一条既已声明的分支**：启用判据需要元素表，`artifact_check` 经 `elements_of(root)` 取表（`graph_view` 单点），模型结构诊断非空 ⇒ `ApiError::Element`。delta spec 的错误面清单里本就写着「模型结构诊断非空 ⇒ `Err`」，本变更只是首次兑现它；冷启动态（模型在、零载体）与正常态零变化。
4. **去掉 `findings` 里的独立 `enabled` 前置不引入新的 `Err` 面**：`load_contracts` 的**解析集是「载体在盘者」**（`contract_path` 命中 `is_file` 才 parse），与 `enabled` 判定完全无关；今日 `enabled = true` 的判定等价于「`<models>` 递归有 md」，其超集恰是「载体在盘」的 md 子集。故「先 `load_contracts` 再从 `snapshot.doc_facts` 派生 `enabled`」与今日的解析 / 报错行为逐条等价，且省掉一次事实装配（见 D6）。
5. **`check.rs` 的「零读盘」原义是零**内容**读**：既有 ⑤ 已用 `contract_path(…).is_file()` 探针，`entry_state` 内部亦探盘。本变更把「正文是否含 fence」这一**唯一**内容读前移到 load 阶段（`doc_facts`），check 侧只读结构体；探针形态不动。
6. **`docs.rs` 的 `resolved_within` / `normalize_rel` 是第 N 份路径归一实现**：删除后 `api::paths::is_repo_relative` 成为 `archi` crate 内唯一 repo 相对路径判据（`scan::rust::normalize_rel` 属扫描面另一域；`api/extract/rust.rs:35` 的模块 doc 仍以散文形式提到 `docs.rs`，属见 OQ-3 的陈旧散文）。
7. **`is_archi_rel_path` 在 `docs.rs` 删除后零非测试消费者**：它是 `pub fn`（无 `dead_code` 告警），且 proposal「不要修改」未授权删除 ⇒ 保留；其模块 doc 的「归档冲突分流的唯一判据」表述属既有散文，本变更不追改（OQ-4）。
8. **`dto.rs` 的 `ArchiApiRef` doc 串写着「MUST NOT 进 `docs` 列表」**：字段退役后该句失指，须同批改述；specta 把 `dto.rs` 的 doc 抄进 `bindings.ts`，但本变更的绑定面出 diff 的主因是**形状变化**（五个类型离场、三个字段进场），故 AC-6 的判据是「重导后幂等」而非「零 diff」。
9. **`packages/desktop/src/app.test.tsx:316` 的 `docs: []` / `warnings: []` 载荷不在 proposal 的「测试文件」表内**（漏列）：DTO 换面后该对象字面量在 `doc: null` 缺位时会 `tsc` 红，故属必改的机械面，归测试相位（见「测试面交接」注）。
10. **知识文本删除「决策记录目录」句后体量下降**：4096 上限断言仍成立；示例 TOML 改为两段形态后仍必须可被 `archi` 解析面零诊断通过。
11. **`archi_decisions_rel_dir` 的非测试消费者恰两处**：`docs.rs`（随文件删除）与 `archi_knowledge.rs:51`（本变更删该句）⇒ 三符号删除不产生悬空引用。
12. **首错改写必然改金样读数**：今日 `constructor` / `reexport` / `signature` 三条各自独立 push（同元素可多报），改后每元素只剩一条 ⇒ 十六份 `expected/*.json` 须经 `DESKTOP_GOLDEN_REWRITE=1` 显式重录，逐份留痕「哪一条因首错被短路」；sidecar 13 份计数不变。
13. **第六段读数 `0 / 43` 的成因必须写准**：41（`declared` 且派生契约 md 不在盘，**改名**自 `apiMissing` 形态①）+ 2（`api = none` 且未声明 `doc`，**首次入面**）= 43，分母 44 不变；`modelHash` 因 `01-app.toml` 删一行**必然变化**（元素 / 关系计数不变）。
14. **`api = none` 元素退出入口判据是显式语义变化**（R-4）：入口解析的唯一消费者是契约面的代码取数（`element_symbols` 需 `EntryState::Resolved`），`none` 支没有 code 面 ⇒ 不再报 `apiEntryMissing`；本仓读数 0 不变（两工具 crate 有显式 `entry`）。
15. **AC-1 的「`docs` 字样零命中」射程限定在架构域**：全仓产品源码里还有三类同名但无关的 `docs`——`infra/agent/src/sdk/db_tool.rs` 的局部变量、`infra/store/src/describe.rs` 的 specta `NamedDataType.docs` / `Field.docs`、`crates/tools/specta-zod-patched/src/*` 的 `docsrs`。判据射程 = `crates/core/archi/src/**`、`crates/core/foundation/src/layout/**`、`crates/infra/agent/src/{archi_knowledge.rs,archi-knowledge.md,sdk/mount.rs}`、`packages/desktop/src/**`（`docs_test.rs` 同批删除）；其中 `api/extract/rust.rs:35` 的 `docs.rs` 散文按 OQ-3 例外保留。
16. **`ArchiElement.doc` 是授权进场字段，判据面没有第二条取数路**：`paths::carrier_path` / `api::doc_facts` / `api::gating_findings` / `check` 的 ⑥ 与 ③ 的入参都是 `ArchiElement`（`dto` 是唯一进 `ApiCheckInput` 与 `doc_facts` 的投影），故 `doc` 事实必须随元素投影出线，否则「`api = none` 的载体」在判据面上不可达。定稿口径：与 `entry` 同型（单值 `Option<String>`、缺省 `null`、字段序紧跟 `entry`、显式字段不省略），`api = declared` 恒 `null`，**与 `ArchiModuleDetail.doc` 同值同源**（均拷自模型 `doc` 原文）——它不是第三个事实源。由此产生两处机械面：① `to_element` 提 `pub(crate)`（检索面复用同一份投影，MUST NOT 另拼第二份 `ArchiElement`）；② `ArchiElement` 字面量夹具必须补 `doc`（射程 = `api/extract/mod_test.rs`、`api/paths_test.rs`、`scan/mod_test.rs`、`scan/owner_test.rs`、`tests/pilot_test.rs`、`src/lib/ipc-contract.test.ts`、`element-tree.test.tsx`、`use-archi-graph.test.ts`、`change-detail-view.test.tsx`——proposal「测试文件」表未逐列，属字段面进场的必改机械面）。
17. **「`doc` 三处进场」是 DTO 字段面口径，生成物只反映两处**：`ArchiSearchResult`（连同 `SearchScope` / `ArchiElementBrief`）是 **agent 工具专用 DTO**，不经 IPC 命令面 ⇒ 从不在 `bindings.ts` / `bindings.zod.ts` 内（变更前后皆然）。故 `bindings:export` 的可见变化是 `ArchiModuleDetail.doc` + `ArchiElement.doc` 两处，`ArchiSearchResult.doc` 的换面由 `archi_search` 工具响应直接体现。`desktop-archi-model` / `desktop-archi-contract-panel` 的「三处经 `bindings:export` 反映到生成物」按此精确读法落地：三处枚举是 DTO 面口径，生成物侧两处字段进场、清单成员零增删（`ArchiElement` / `ArchiModuleDetail` 本就在清单内）。

---

## 关键设计决策

| # | 议题 | 定稿 | 理由 |
|---|---|---|---|
| D1 | 字段面 | `Element.doc: Option<String>`（`model.rs`）+ `RawElement.doc: Option<String>`（`parse/toml.rs`，`#[serde(default)]`），`docs` 出白名单；**`01-app.toml` 的 `docs` 行与白名单同一提交删除** | `deny_unknown_fields` 删 `docs` 后仍写着它的模型立刻 `unknownField` 而停摆（画布 / `archi_module` / 门禁同时失效）；数组形态由 serde `invalid type` 归 `invalidValue`（`doc = [..]` 直接拒收，MUST NOT 静默取首条） |
| D2 | 载体派生单点 | 新增 `paths::doc_path`（归一 + `is_repo_relative` + `join`）与 `paths::carrier_path`（`declared → contract_path` / `none → doc_path`）；`is_repo_relative` 提 `pub(crate)` | `docs.rs::normalize_rel` 随文件消失，若不收进单点，`api` / `search` / `dto` 会各写第二份归一（R-9）；载体取值只有一处，判据面 / 纳管面 / 检索面 / 投影面四个消费者共用 |
| D3 | `api` 状态 = 载体开关 | 三处判断一律 `match element.api`：判据求值两支（D8）、纳管子集（D12）、投影与检索载体（D2） | 同一开关表达同一事实；在任一处另写「`declared` 才…」的旁路判据都会与其它两处漂移 |
| D4 | `DocFact` 三态与装配点 | `pub enum DocFact { Absent, Missing, Present { has_api_fence: bool } }`；私有 `doc_facts(root, elements) -> BTreeMap<String, DocFact>` 是**唯一装配点**（每份载体至多读一次、读一次即弃）；`ApiSnapshot` 与 `ApiCheckInput` 各携一份 `doc_facts` | fence 事实只有一处产出 ⇒ 启用判据与 `apiDocMissing` 同源；读失败按 `Present { has_api_fence: false }` 处置（启用判定是**探针**，不得因一个坏文件把 `archi_api_check` 整条打成 `Err`），而契约解析读失败仍是既有 `ApiError::Io` |
| D5 | 启用判据收紧 | `pub fn enabled(root: &Path, elements: &[ArchiElement]) -> bool` = `doc_facts(…).values().any(Present { has_api_fence: true })`；删除 `has_markdown` 与 `MARKDOWN_SUFFIX` | 「`models/` 放一份笔记即误启用」是 E-1 移交的真缺陷；字面口径含 `api = none` 元素的 `doc` 载体（同一条事实、零 `api` 分支），见 OQ-2 |
| D6 | `findings` 的启用判定位置 | `findings` 只调一次 `load_contracts`，从 `snapshot.doc_facts` 派生 `ApiCheckInput.enabled`；`api::check` 的对外入口仍保留 `enabled == false → 空 Vec` 短路 | 两次装配必然在「载体存在但不可读」一类边界上分叉（事实校准 4）；解析集与今日等价 ⇒ 零行为回退 |
| D7 | fence 语言标复用 | `contract::FENCE_OP` / `FENCE_TYPES` / `info_tokens` 提 `pub(crate)`，`api/mod.rs::has_api_fence` 用它们扫行 | 检查侧 MUST NOT 抄第二份字面量、MUST NOT 补写半个 markdown 解析器；`json api-op` 与 `api-op` 的同判语义与解析器逐字相同 |
| D8 | 判据求值形态 | 两支三段首错：`none` 支 `apiUnexpected` → `docMissing` → `docNotFound`；`declared` 支 `docMissing` → `apiDocMissing` → `apiUnexpected` → `apiEntryMissing` → `apiMissing` → 一致性；**一致性内部序 = `apiSignatureMismatch` → `apiReexportDrift` → `apiConstructorDeclared`**；三条跨元素判据落 **pair 锚点**（`nested → ref → shadowing`），独立于元素循环、不吃元素名额 | 一号位到六号位只读模型 + 盘（零导出集依赖），七号位起需 `Resolved` 入口才有所指对象 ⇒ 短路的是「尚不存在的对象」而非真问题；pair 锚点 MUST NOT 引用 `Model.relations`（父子无边、`$ref` 目标可能不在模型内，写成 relation 会先被 `graph.rs` 判结构错） |
| D9 | 三值语义各自单成因 | `docMissing` = 入口文档未就位（declared：契约 md 未写 / none：`doc` 未声明）；`docNotFound` = 声明指不到；`apiDocMissing` = 有文档无约定；`apiMissing` 收窄为「有约定无兑现」 | 同一值承载两种缺口就无法分档、无法分文案；文案 MUST 点名成因（派生路径 / 声明值），MUST NOT 只写通用「无入口文档」 |
| D10 | `apiUnexpected` 双形态各自单向 | ① `api = none` ∧ 派生契约路径在盘（none 支**一号位**，`continue`）；② `api = declared` ∧ `doc` 被写（declared 支**五号位**，`continue`，且该值不参与任何其它判定） | ① 排第一是为防「缺 README 的元素遮住放错位置的契约」（非纳管判据遮蔽纳管判据）；② 排五号位是为让「矛盾」只在「已具契约者」身上出现，而不是成为第一个报出的错 |
| D11 | 档位三值与 `passed` 单点 | `ApiFindingSeverity { Off, Warn, Error }`；`severity()` = `ApiUndeclared ⇒ Off`、`DocNotFound \| ApiMissing \| ApiSignatureMismatch ⇒ Error`、其余 `_ => Warn`（保留通配 + 注释，落档须显式决定）；新增 `report::has_error_tier(findings) -> bool` 作为唯一判据，`derive_status` 与 `ArchiDriftReport::passed()` 共用 | `error` 档必须同时驱动 `status` 与 `passed`（既有 spec 要求的同批动作）；两处共用同一函数即兑现 `status == errors ⟺ passed == false` 不变量，runner 侧零第二份判定 |
| D12 | 纳管子集口径 | 纳管 ⟺ element 非空 ∧（declared：载体在盘 / none：`doc` 已声明）；`docMissing` 两侧都不成立 ⇒ **永不纳管**；`gating_findings` 签名不变（`is_file` 仍是同一次调用，单点仍在此函数） | `docMissing` 43 条若纳管会把 `status` 常态化为永久 `warnings`；none 侧取「已声明」而非「在盘」是唯一能同时满足档位表与纳管表的写法（R-2） |
| D13 | 线面换面与投影纪律 | 三处都是 `doc` 的**投影**（不是新事实源）：① `ArchiModuleDetail.doc` = **模型原文**（与 `path` / `entry` 同一「只裁剪零归一」纪律），且**只有 `api = none` 元素出线**（`declared` 恒 `null`）；② `ArchiElement.doc` = **模型字段拷贝**（与 `entry` 同型、字段序紧跟 `entry`、缺省 `null`、显式字段不省略，`declared` 恒 `null`），供 `paths::carrier_path` / `api::doc_facts` / `api::gating_findings` / `check` 的 ⑥ 与 ③ 取 `doc` 事实，`ArchiElementBrief` MUST NOT 带 `doc`；③ `ArchiSearchResult.doc: Vec<String>` 出**归一后的 repo 相对串**（与 `declared` 侧经 `api::display` 派生的串同形） | `dto` 是唯一进 `ApiCheckInput` / `doc_facts` 的投影 ⇒ 不随元素投影出线则 `api = none` 的载体在判据面上不可达（事实校准 16）；抽屉侧仍兑现「一条入口文档」（AC-9 的 MUST NOT 两条）——declared 走契约段、none 走入口文档段；检索条目要与派生路径并表，故取归一形态；模型矛盾（declared 写了 doc）由判据 `apiUnexpected` 报，不在投影面复述 |
| D14 | 检索面 | 语料 = `path` 非空元素的在盘载体（`carrier_path(...).is_some_and(is_file)`，条目经 `api::display` 归一）；`SearchScope::Doc`（线格式词 `doc`，无别名）；`matches_doc(path: &str, …)`；`file_stem` 从 `docs.rs` 迁入 `search.rs`；排序 = 路径字典序 | `matches_doc` 原本只吃 `path` + `file_stem`（`title` 零作用）⇒ 条目退化为路径串零功能损失；`docs` 别名会让 scope 语义二义（R-10）；语料集合与判据面同一集合（`path` 非空），MUST NOT 另立第二份载体派生 |
| D15 | 前端入口文档段 | `api = none` ⇒ 段恒渲染：`doc` 非空 → 一条「入口文档」行（`FileViewer` + `read_file`）；`doc` 为 `null` → 空态文案「无入口文档」+ testid `archi-entry-doc-empty`；`api = declared` ⇒ 段**不渲染**（入口文档由契约段 `ContractPathRow("契约")` 承担） | AC-9 要求空态 testid 随字段面更名 ⇒ none 侧必须有空态；declared 侧必须零第二条入口文档。spec 内两处措辞互斥（见 OQ-1） |
| D16 | 呈现面的诚实边界 | `runner.rs` 只改 `passed` 一行（读 `report.passed()`）；`compose_text` 的 `Warnings` / `Errors` 两分支**本变更不改** | delta spec 明写执行体零逻辑改动；`status = errors` 由 error 档判据单独触发时 detail 文案的「N 条违例」不含契约判据（报告内 `apiFindings` 有全量）——已知呈现缺口，见 OQ-5 |
| D17 | `REPORT_VERSION` | 仍为 **3** | 报告字段面无增删（`apiFindings` 形状不变，只换取值 / 档位 / 纳管口径）⇒ 不足以为版本递增提供理由 |
| D18 | 零扩散白名单 | 允许出 diff：本 change 目录文稿与规格面、`model.rs`、`parse/toml.rs`、`api/paths.rs`、`api/mod.rs`、`api/contract.rs`（可见性）、`api/check.rs`、`drift/report.rs`、`drift/mod.rs`、`infra/archi/src/runner.rs`（`passed` 一行）、`dto.rs`、`search.rs`、`lib.rs`、`docs.rs` + `docs_test.rs`（删除）、`foundation/src/layout/mod.rs`、`infra/agent/src/archi_knowledge.rs`、`archi-knowledge.md`、`sdk/mount.rs`、`element-drawer.tsx`、`types/generated/bindings.ts` + `bindings.zod.ts`、`.dev-team/architecture/models/01-app.toml`（一行）、`AGENT.md` / `CLAUDE.md`、`package.json`、proposal「测试文件」表所列测试文件 + `ArchiElement` 字面量机械面九处（事实校准 16）、`app.test.tsx`（事实校准 9）；`openspec/specs/` 除 9 个 delta 能力外零 diff | 出现白名单外 diff 即回退；`desktop-archi-file-tree` 零 diff 是判据之一。白名单内除本 change 目录的文稿与规格面外，**不含任何新增文件**（源码与产品文档零新增） |
| D19 | 版本纪律 | `package.json` `0.6.52` → `0.6.53`；`tauri.conf.json` 保持 `"version": "../package.json"`；`src-tauri/Cargo.toml` 的 crate 版本逐字不变 | AGENT.md 版本纪律，无例外 |

---

## 架构组件 (Architecture Components)

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|---|---|---|---|---|
| 字段面（**改**） | `docs: Vec<String>` → `doc: Option<String>`；白名单同步 | `crates/core/archi/src/model.rs`、`parse/toml.rs` | `serde` / `toml` | 单值 `Option<String>`（数组形态由 serde 归 `invalidValue`） |
| 载体派生单点（**改**） | 入口文档载体 = `declared → contract_path` / `none → doc`；`doc` 形态归一与判据；`is_repo_relative` 提 `pub(crate)` | `crates/core/archi/src/api/paths.rs` | `foundation::layout` | 纯路径运算 |
| 载体事实装配（**改**） | `DocFact` 三态 + `doc_facts` 单点 + `has_api_fence` 扫行；`ApiSnapshot` / `ApiCheckInput` 携入 | `crates/core/archi/src/api/mod.rs` | `contract::{FENCE_OP, FENCE_TYPES, info_tokens}` | 每份载体一次 `read_to_string`（读一次即弃） |
| 启用判据（**改**） | `enabled(root, elements)` = 任一载体 `Present { has_api_fence: true }`；`artifact_check` 经 `elements_of` 取表 | 同上 | `dto::graph_view` | 纯读、零 spawn |
| fence 语言标（**改可见性**） | `FENCE_OP` / `FENCE_TYPES` / `info_tokens` 提 `pub(crate)` | `crates/core/archi/src/api/contract.rs` | — | 既有 `info_tokens` 切分（空格 / 逗号） |
| 判据求值（**改**） | 十三值两支三段首错 + pair 锚点 + `category_of` 七值 | `crates/core/archi/src/api/check.rs` | `paths`、`ApiCheckInput.doc_facts`、`contract`、`extract` | 纯读 `input`（零内容读、零写盘、零 spawn） |
| 判据词汇与档位（**改**） | `ArchiApiFindingKind` 十三值 + `ALL` / `as_str` / `severity()`（三值）+ `has_error_tier` + `ArchiDriftReport::passed()` | `crates/core/archi/src/drift/report.rs` | `serde` / `specta` | 封闭枚举 + 稳定序 |
| 状态推导（**改**） | `derive_status` 在纳管子集上分档（任一 `Error` ⇒ `errors`） | `crates/core/archi/src/drift/mod.rs` | `report` | 推导序不变 |
| 门禁执行体（**改一行**） | `passed = report.passed()` | `crates/infra/archi/src/runner.rs` | `archi::drift` | 薄读 |
| 线面投影（**改**） | `ArchiModuleDetail.doc`、`ArchiSearchResult.doc: Vec<String>`、`ArchiElement.doc`（授权进场）、`SearchScope::Doc`；五类型退役；`linked_doc_paths` 退役 | `crates/core/archi/src/dto.rs` | `api::paths`、`search` | 投影零改写 |
| 检索面（**改**） | 语料 = 在盘载体；`matches_doc(path)`；`file_stem` 迁入 | `crates/core/archi/src/search.rs` | `api::paths`、`dto::SearchScope` | 每次现算（零索引、零缓存） |
| 三源关联（**删除**） | `docs.rs` / `docs_test.rs` 整文件退役 | `crates/core/archi/src/` | — | — |
| 布局派生族（**改**） | 删 `DECISIONS_DIR_NAME` / `archi_decisions_dir` / `archi_decisions_rel_dir` | `crates/core/foundation/src/layout/mod.rs` | 纯推导 | 纯函数删除 |
| 架构会话知识（**改**） | 派生段删「决策记录目录」；骨架字段面 / 示例 / 写面换面 | `crates/infra/agent/src/archi_knowledge.rs`、`archi-knowledge.md` | `foundation::layout`、`archi::dto` 词表 | 静态骨架 `include_str!` + 派生段拼装 |
| agent 工具描述（**改**） | `archi_module` 描述换面；`archi_search` 的 `scope` enum → `["elements", "doc", "all"]` | `crates/infra/agent/src/sdk/mount.rs` | `archi::dto` 词表 | 工具定义单源 |
| 元素抽屉（**改**） | `DocsSection` / `DocRow` 删；入口文档单条（仅 `api = none`） | `packages/desktop/src/views/architecture/components/element-drawer.tsx` | `FileViewer` + `read_file` | React |
| 出线绑定（**生成物**） | `bindings:export` 重导：五类型离场、IPC 面 `doc` 两处进场（`ArchiModuleDetail.doc` / `ArchiElement.doc`）；DTO 面第三处 `ArchiSearchResult.doc` 属 agent 工具专用，不进生成物（事实校准 17） | `packages/desktop/src/types/generated/bindings.ts` / `bindings.zod.ts` | specta | 生成物（入库） |
| 模型数据（**改一行**） | `01-app.toml` 删 `docs` 行 | `.dev-team/architecture/models/01-app.toml` | — | TOML |

依赖方向零新增边：`core/archi` 仍只依赖 `foundation`（既有边）；`infra/agent` → `archi` / `foundation` 既有边不变；`infra/archi` → `core/archi` 既有边不变。`core` 仍零写盘、零 infra 依赖。

### 数据流（改后）

```text
模型 models/*.toml ──parse_dir──> Element { …, api, doc: Option<String> }
                                        │
                          dto::to_element（模型 → 元素投影；doc 逐元素拷贝）
                                        │
                                        ▼
                ArchiElement { …, entry, doc }（判据 / 纳管 / 检索 / 投影的共同入参）
                                        │
                     paths::carrier_path(root, element)
                     ├── api = declared → <models>/<Domain>/<FQN>.md（派生，= contract_path）
                     └── api = none     → root.join(normalize(doc))
                                        │
              ┌─────────────────────────┼─────────────────────────┐
              ▼                         ▼                         ▼
  api::doc_facts(root, elements)   判据面 api::check       search 语料 / dto 投影
  Absent / Missing / Present{…}    （十三值两支首错）      （在盘者出路径串）
              │                         │
              ├── enabled(root, elements) = 任一 Present{fence}
              └── gating_findings（declared: 载体在盘 / none: doc 已声明）
                                        │
                          derive_status（任一 Error ⇒ errors）
                                        │
                          report.api_findings / status / passed()
```

---

## 变更清单 (Change Inventory)

> 下表与「修改文件」表的 `crates/**` / `src/**` 均为 `packages/desktop/src-tauri/` 之下的路径（省略该前缀）；`packages/desktop/src/**`、`.dev-team/**`、仓库根文件与 `openspec/**` 按全路径书写。proposal「变更范围 - 实现文件」列的 `crates/core/archi/src/drift/gate.rs`（「若 `passed` 落在此处，按实际落点改」）在本仓**不存在**——`passed` 实际落点是 `packages/desktop/src-tauri/crates/infra/archi/src/runner.rs:95`，故该行由 `runner.rs` 承接（见「修改文件」表 `runner.rs` 行与事实校准 1）。

### 新增文件

<!-- 本变更零新增源码文件：proposal「变更范围 - 实现文件」全为修改 / 删除。数据面的两份工具 crate README 与 41 份契约 md 属 2b（proposal 八 / explore §9），本 change MUST NOT 新建。 -->

无。

### 删除文件

| 文件 | 说明 |
|---|---|
| `crates/core/archi/src/docs.rs` | 三源关联整文件退役（`DocsOutcome` / `DocWarning` / `element_docs` / `all_docs` / `normalize_rel` / `is_repo_relative` / `resolved_within` / `title_of` / `make_doc` / `merge_doc` / `scan_adjacent` / `keep_entry` / `is_excluded` / `read_decisions` / `is_decision_path` / `EXCLUDED_DIR_NAMES` / `MARKDOWN_SUFFIX`）；`file_stem` **迁驻** `search.rs`（检索命中仍在用） |
| `crates/core/archi/src/docs_test.rs` | 与 `docs.rs` 同批删除（其「不读 `specs`」用例的语义面由 `desktop-archi-agent` 的 ADDED 检索面 requirement 承接） |

### 修改文件

| 文件 | 具体修改 | 说明 |
|---|---|---|
| `crates/core/archi/src/model.rs` | `Element.docs: Vec<String>` → `doc: Option<String>`（doc 写「入口文档指针：单值、repo 相对 POSIX；只有 `api = none` 元素可写；`declared` 的入口文档恒为派生 `contract_path`」）；`api` 字段 doc 补「入口文档载体的开关」一句 | D1；`Relation` / `SourceSpan` / `Diagnostic` 零变化 |
| `crates/core/archi/src/parse/toml.rs` | `RawElement.docs: Vec<Spanned<String>>`（`#[serde(default)]`）→ `doc: Option<String>`（`#[serde(default)]`，doc 注明「数组形态由 serde `invalid type` 归 `invalidValue`」）；`to_model` 的字段传递同步 | D1；`deny_unknown_fields` 主体与 `classify` 零变化 |
| `crates/core/archi/src/api/paths.rs` | ① 模块 doc 与 `is_repo_relative` 的 doc 去掉 `docs::normalize_rel` 引用（改指「本函数是 repo 相对路径判据的唯一实现」）；② `is_repo_relative` 由私有提 `pub(crate)`；③ 新增 `pub fn doc_path(root: &Path, doc: &str) -> Option<PathBuf>`（`normalize_root` + `is_repo_relative` → `root.join`；非法 → `None`）；④ 新增 `pub fn carrier_path(root: &Path, element: &ArchiElement) -> Option<PathBuf>`（按 `api` 状态取载体）；⑤ `contract_path` 的 doc 补「同时是 `api = declared` 元素的入口文档载体」 | D2 / D3；`domain_of` / `directory` / `index_path` / `schema_path` / `generated_path` / `entry_candidates` / `entry_state` / `normalize_root` / `checked_root` / `is_fqn` 逐字不动 |
| `crates/core/archi/src/api/mod.rs` | ① 新增 `pub enum DocFact { Absent, Missing, Present { has_api_fence: bool } }`；② 新增私有 `fn doc_facts(root: &Path, elements: &[ArchiElement]) -> BTreeMap<String, DocFact>`（唯一装配点，每份载体至多一次 `read_to_string`，读失败按 `Present { has_api_fence: false }`）与私有 `fn has_api_fence(source: &str) -> bool`（扫行取 ``` 后的 info string，经 `contract::info_tokens` 判 `FENCE_OP` / `FENCE_TYPES`）；③ `ApiSnapshot` 增 `doc_facts: BTreeMap<String, DocFact>`；④ `ApiCheckInput` 增 `doc_facts: &'a BTreeMap<String, DocFact>`；⑤ `load_contracts` 起始处调 `doc_facts` 并写入快照；⑥ `enabled(root: &Path) -> bool` 改 `enabled(root: &Path, elements: &[ArchiElement]) -> bool`（判据 = 任一 `Present { has_api_fence: true }`），删除 `has_markdown` 与 `MARKDOWN_SUFFIX`；⑦ `findings` 改为「一次 `load_contracts` → 从 `snapshot.doc_facts` 派生 `enabled`」，删掉独立启用前置；⑧ `gating_findings` 口径改 `match element.api`（`Declared` → 载体 `is_file()`；`None` → `element.doc.is_some()`），签名不变；⑨ `artifact_check` 增 `let elements = elements_of(root)?;` 后调 `enabled(root, &elements)`；⑩ `meta()` / `element_symbols` / `api_ref_of` / `render_all` / `bootstrap` 逐字不动 | D4 / D5 / D6 / D12；`ApiError` 三变体不动 |
| `crates/core/archi/src/api/contract.rs` | `FENCE_OP` / `FENCE_TYPES` / `info_tokens` 由私有提 `pub(crate)`（doc 补「供启用判据与 `apiDocMissing` 复用，MUST NOT 抄第二份字面量 / 第二份切分」） | D7；解析、双向 gate、`$defs` 规则、落点闸、`duplicate_keys` 逐字不动 |
| `crates/core/archi/src/api/check.rs` | ① 模块头注释改为「十三值 / 两支三段 / 首错短路 / pair 锚点」并写明分界依据；② `category_of` 加 `doc`（`DocMissing` / `DocNotFound` / `ApiDocMissing` → `"doc"`）；③ `check()` 重写为两支三段首错：`none` 支（`apiUnexpected` → `docMissing` → `docNotFound`）/ `declared` 支（`docMissing` → `apiDocMissing` → `apiUnexpected(写了 doc)` → `apiEntryMissing` → `apiMissing` → 一致性），一致性内部序 `signature → reexport → constructor` 且**首错即停**；④ 三条跨元素判据保留为独立 pair 锚点（`nested_cross_crate` → `ref_missing` → `shadowing`），与元素锚点互不挤占；⑤ `missing_contracts` 删形态①（`declared` 而契约文件缺失），只剩形态②，`input` / `root` 参数随之不需要；⑥ `signature_mismatch` / `reexport_drift` / `constructor` 三处由「push」改「返回 `Option<ArchiApiFinding>`」以支撑首错；⑦ 新增 `docMissing` / `docNotFound` / `apiDocMissing` 三处文案（`docMissing` 按成因分两段文案） | D8 / D9 / D10；`$defs` 只读 `schema.json`、类型 / 位置词汇单点在 `extract`、`is_pure_ref_alias` 共用等纪律零变化 |
| `crates/core/archi/src/drift/report.rs` | ① `ApiFindingSeverity` 加 `Error`（`as_str` = `"error"`，`ALL: [_; 3]`）；② `ArchiApiFindingKind` 加 `DocMissing` / `DocNotFound` / `ApiDocMissing`（`as_str` 三词，`ALL: [_; 13]`）；③ `severity()` = `ApiUndeclared ⇒ Off`、`DocNotFound \| ApiMissing \| ApiSignatureMismatch ⇒ Error`、其余 `_ => Warn`（通配处留注释「新增变体落哪一档须显式决定」）；④ 新增 `pub fn has_error_tier(findings: &[ArchiApiFinding]) -> bool`（唯一判据）；⑤ 新增 `impl ArchiDriftReport { pub fn passed(&self) -> bool }`（`errors` 空 ∧ `model.structural_errors` 空 ∧ `has_error_tier(&api_findings)` 为假）；⑥ 模块头「四组词汇」表述随 `WarningKind` 退役校准 | D11 / D17；`ArchiApiFinding` 形状、`api_findings` 槽位、`REPORT_VERSION = 3` 逐字不变 |
| `crates/core/archi/src/drift/mod.rs` | ① 模块 doc 与 `REPORT_VERSION` doc 把「契约在案元素的判据 / 不进 `passed`」改写为「入口文档承诺成立的判据进 `status` 与 `passed`」；② `derive_status` 的 `warnings` 分支保持「`warnings[]` 非空 **或** 纳管子集非空」，`errors` 分支条件追加 `report::has_error_tier(gating)`；③ `check()` 的 ⑧ 段注释改口径（`api_findings` 装配与 `gating` 入参逐字不变） | D11 / D12；推导序（`skipped → errors → noChanges → warnings → clean`）与 `REPORT_VERSION` 不变 |
| `crates/infra/archi/src/runner.rs` | `let passed = report.errors.is_empty() && report.model.structural_errors.is_empty();` → `let passed = report.passed();` | D16 / 事实校准 1；**仅此一行**，`compose_text` / 摘要 / `fault` 面不动（spec 明写执行体零逻辑改动） |
| `crates/core/archi/src/dto.rs` | ① 删 `WarningKind` / `DocKind` / `DocSource` / `DocRef` / `ArchiWarning`（含各自 `as_str` / `ALL`）；② `ArchiModuleDetail`：删 `docs` 与 `warnings`，在 `children` 与 `api` 之间新增 `doc: Option<String>`；③ `ArchiSearchResult.docs: Vec<DocRef>` → `doc: Vec<String>`；④ `SearchScope::Docs` → `Doc`（`#[serde(rename_all = "lowercase")]` 即得线格式词 `doc`），`parse` 匹配臂 `"docs"` → `"doc"`，非法值文案列 `elements / doc / all`；⑤ `module_detail` 的装配：`doc` 取 `api = none` 时的模型原文、`declared` 恒 `null`，删 `docs_outcome` / `warnings` 映射；⑥ `search` 装配 `outcome.doc`；⑦ 删 `pub fn linked_doc_paths`；⑧ **`ArchiElement` 在 `entry` 之后新增 `doc: Option<String>`**（授权进场字段：模型字段的拷贝，与 `entry` 同型——单值、缺省 `null`、显式字段不省略、`declared` 恒 `null`；`to_element` 逐元素拷贝）；⑨ `fn to_element` 提 `pub(crate)`（检索面经同一投影取 `doc` 事实，MUST NOT 另拼第二份 `ArchiElement`）；⑩ `ArchiApiRef` / `SearchScope` 相关 doc 串改述（「MUST NOT 进 `docs` 列表」等） | D13 / D14 + 事实校准 16；`ArchiElementBrief` / `ArchiApiRef` / `ArchiGraph` / 其余投影逐字不变 |
| `crates/core/archi/src/search.rs` | ① 删 `use crate::docs`；② `SearchOutcome.docs: Vec<DocRef>` → `doc: Vec<String>`（doc 注明「`path` 非空元素的在盘入口文档载体，路径字典序」）；③ 语料改为 `elements.filter(!path.is_empty()).filter_map(|e| carrier_path(root, &dto::to_element(e))).filter(is_file).map(api::display)`（经元素投影取 `doc`，不自建归一、不另拼 `ArchiElement`）；④ `matches_doc(path: &str, tokens: &[String])`（命中面仍只吃路径 + `file_stem`，不读正文）；⑤ `file_stem` 由 `docs.rs` 迁入本文件（`fn file_stem(relative: &str) -> String`）；⑥ scope 分支改 `SearchScope::Doc` | D14；`tokenize` / `split_camel` / `contains_all` / `matches_element` 逐字不动 |
| `crates/core/archi/src/lib.rs` | ① 删 `pub mod docs;` 与 `#[cfg(test)] mod docs_test;`；② `pub use dto::{…}` 删 `DocKind` / `DocRef` / `DocSource` / `WarningKind`；③ 模块 doc 第 1 行「文档关联」改「入口文档」、第 3 条词汇清单去掉「文档警告」 | D18；其余出口逐字不变 |
| `crates/core/foundation/src/layout/mod.rs` | 删 `const DECISIONS_DIR_NAME`、`pub fn archi_decisions_dir`、`pub fn archi_decisions_rel_dir`（各连整段 doc） | proposal 七；`archi_dir` / `archi_models_*` / `archi_graph_*` / `archi_config_path` / `is_archi_rel_path` / `Layout` / `resolve` / `ReportSlot` 逐字不动 |
| `crates/infra/agent/src/archi_knowledge.rs` | `build` 的格式串删「；决策记录目录：{}」一段（实参序少一个 `archi_decisions_rel_dir()`），保留「模型目录 / 契约资产 / 契约索引」三处方位；模块 doc 与 `build` doc 的方位清单同步 | proposal 七 / AC-8；四面词表注入、`join` 单点、`<>` 拒绝、降级 `None` 逐字不动 |
| `crates/infra/agent/src/archi-knowledge.md` | ① 格式约定字段面 `docs` → `doc`，并补两条纪律（单值；只有 `api = none` 可写，`declared` 的入口文档恒为派生落点）；② 边界段「`api` 与 `entry` 都是指针」扩为「`api` / `entry` / `doc` 三个指针」；③ 写面与可见性「只改架构域模型与 `docs` 关联文档」→「只改架构域模型与 `doc` 所指入口文档」；④ 示例 TOML 改两段形态（`api = "none"` + `doc = "…"` 一段；declared 元素**零 `doc` 行**、以注释指路一段） | AC-8；零 `<>` 占位符、体量受上限约束不变 |
| `crates/infra/agent/src/sdk/mount.rs` | ① `use archi::dto::{ElementKind, GateTier}`（去 `WarningKind`）；② `archi_module` 描述删「关联文档（四字段条目）」与「`docNotFound` 警告（只报不拦）」，改为「入口文档单值 `doc`（仅 `api = none` 元素有值；`declared` 走 `api.contractPath`）」；③ `archi_search` 描述「四字段文档条目」→「入口文档路径」，`scope` enum `["elements","docs","all"]` → `["elements","doc","all"]` | AC-6；工具名常量、参数形状、`READONLY_TOOLS`、注入矩阵零变化 |
| `packages/desktop/src/views/architecture/components/element-drawer.tsx` | ① 删 `DocRow` / `DocsSection`；② 新增 `EntryDocSection`：`api = none` 时渲染（`doc` 非空 → 一条「入口文档」行 + `FileViewer`；`doc = null` → 空态文案「无入口文档」+ `data-testid="archi-entry-doc-empty"`），`api = declared` 时不渲染；③ 调用点改为「概览 → 契约 `ContractSection` → 入口文档 `EntryDocSection` → 豁免」；④ 组件 doc 的段序说明同步 | D15 / AC-9；`Overview` / `ContractSection` / `ContractPathRow` / `ExemptSection` / `GateEditor` 逐字不动 |
| `packages/desktop/src/types/generated/bindings.ts` + `bindings.zod.ts` | `bindings:export` 重导后入库：`DocRef` / `DocKind` / `DocSource` / `WarningKind` / `ArchiWarning` 离场；`doc` 在 **IPC 可见面两处**进场（`ArchiModuleDetail.doc` / `ArchiElement.doc`，`SearchScope` 与本文件无关——它不属 IPC DTO）；DTO 面第三处 `ArchiSearchResult.doc` 只出现在 agent 工具响应（事实校准 17） | 生成物（MUST NOT 手改）；重复导出幂等 |
| `.dev-team/architecture/models/01-app.toml` | 删第 7 行 `docs = ["./README.md", "./AGENT.md"]`，其余字节不变 | **与 D1 的 parser 白名单同一提交**（R-1：不同批则本仓模型当场停摆）；数据面「`App` 写 `doc`」不属本变更 |
| `AGENT.md` / `CLAUDE.md` `## 模块 API 契约` | ① 新增一条要点：`doc` 是入口文档指针（单值），**只有 `api = none` 元素可写**，`declared` 的入口文档 = 派生 `contract_path`；② 原「契约在案元素的判据进 status、不进 passed」改写为「**入口文档承诺成立的 `error` 档判据进 `status` 与 `passed`**（`docMissing` 是 warn 且永不纳管）」，并把「十三值 / 两支三段首错求值」一句指路补进同一条；③ 两份该节正文**逐字相同** | AC-11；要点数 5 → 6，其余三条零改动 |
| `packages/desktop/package.json` | `"version"`: `0.6.52` → `0.6.53`（落地时若 `HEAD` 已滚动则按新现值 +1 patch） | D19；`tauri.conf.json` 经 `"version": "../package.json"` 跟随，`src-tauri/Cargo.toml` 不随动 |

> **不在本清单的对象**（proposal「不要修改」+ D18）：`api/contract.rs` 的解析 / 双向 gate / `$defs` 规则（只提可见性）、`api/extract/**`（产品源码；其 `mod_test.rs` 的 `ArchiElement` 字面量补 `doc` 属字段面机械面，见事实校准 16）、`api/emit/**`、`api/render.rs`、`api/schema.rs`、`api/diff.rs`、`infra/archi/src/api_artifacts.rs`、`drift/config.rs`（两键封闭 + `deny_unknown_fields`）、`tree.rs` / `scan/**`（产品源码；其 `*_test.rs` 的字面量补 `doc` 同上）、`contract-panel.tsx` / `use-archi-api-check.ts` / `use-archi-module.ts` / `drift-marks.ts` / `architecture-canvas.tsx`、`archi_tool.rs`、`sdk/policy.rs`、`.dev-team/architecture/models/{05-backend-tools.toml,Infra/**}`、`decisions/0001-model-granularity.md`、两个工具 crate 的 README 与 41 份契约 md（2b）、`openspec/specs/**`（9 个 delta 之外）。
>
> **测试面交接**：proposal「测试文件」表所列 **19 行**（含 `docs_test.rs` 的删除与末行「验收留痕 `reports/`」；后者不是测试文件而是留痕动作，见「阶段十一」）归测试相位——本相位**不写用例、不跑测试**；设计侧只钉住必改的机械面：① `api/**` sidecar 仍恰 **13** 份（新判据用例落 `check_test.rs` / `mod_test.rs`，MUST NOT 新建文件）；② `enabled` 用例族由「递归任意 md」重录为「载体在盘且含 fence」（`mod_test.rs:5099` 的「判定单点自证」断言源码串 `pub fn enabled(root: &Path) -> bool {` 须随新签名更新）；③ `gating_findings` 用例补 `none` 支；④ `ApiCheckInput` 字面量补 `doc_facts`（机械）；⑤ `infra/archi/src/runner_test.rs` 的「`api_findings` MUST NOT 影响 `passed`」断言**反转**；⑥ `app.test.tsx` 的 `docs: []` / `warnings: []` 载荷（事实校准 9）；⑦ 十六份 drift 金样经 `DESKTOP_GOLDEN_REWRITE=1` 显式重录并逐份留痕成因；⑧ **`ArchiElement` 字面量夹具补 `doc`**（事实校准 16 的九处：`api/extract/mod_test.rs`、`api/paths_test.rs`、`scan/mod_test.rs`、`scan/owner_test.rs`、`tests/pilot_test.rs`、`src/lib/ipc-contract.test.ts`、`element-tree.test.tsx`、`use-archi-graph.test.ts`、`change-detail-view.test.tsx`——不补即编译 / `tsc` 红）。

### 公共函数 / API

| 标识符 | 所在文件 | 变化 | 完整签名 | 说明 |
|---|---|---|---|---|
| `doc_path` | `crates/core/archi/src/api/paths.rs` | 新增 | `pub fn doc_path(root: &Path, doc: &str) -> Option<PathBuf>` | `normalize_root`（`\`→`/`、剥前导 `./`、去尾 `/`）+ `is_repo_relative` 通过后 `root.join`；绝对 / 盘符 / `~` / `..` / 空段 / 空串 → `None`（`join` 越根的安全边界） |
| `carrier_path` | 同上 | 新增 | `pub fn carrier_path(root: &Path, element: &ArchiElement) -> Option<PathBuf>` | **入口文档载体派生单点**：`ApiStatus::Declared → contract_path(root, &element.fqn)`；`ApiStatus::None → element.doc.as_deref().and_then(\|doc\| doc_path(root, doc))`（`element.doc` 即元素投影上的授权进场字段，事实校准 16）；MUST NOT 有第二份实现（判据 / 纳管 / 检索 / 投影共用） |
| `is_repo_relative` | 同上 | 修改（可见性） | `pub(crate) fn is_repo_relative(relative: &str) -> bool` | 由私有提 `pub(crate)`；doc 去掉 `docs::normalize_rel` 引用（该实现已删除，本函数成唯一判据） |
| `DocFact` | `crates/core/archi/src/api/mod.rs` | 新增（类型，见「类型定义」） | — | 载体三态 |
| `enabled` | 同上 | 修改（签名 + 判据） | `pub fn enabled(root: &Path, elements: &[ArchiElement]) -> bool` | = `doc_facts(root, elements)` 中任一 `Present { has_api_fence: true }`；MUST NOT 退化为「递归任意 `*.md`」，命令层 MUST NOT 另判一套 |
| `load_contracts` | 同上 | 修改（快照增字段；签名不变） | `pub fn load_contracts(root: &Path, elements: &[ArchiElement]) -> Result<ApiSnapshot, ApiError>` | 契约装载输入源零改动（仍只认 `contract_path`）；同批装配并携带 `doc_facts` |
| `findings` | 同上 | 修改（内部形态） | `pub fn findings(root: &Path, elements: &[ArchiElement]) -> Result<Vec<ArchiApiFinding>, ApiError>` | 一次 `load_contracts` → 由 `snapshot.doc_facts` 派生 `enabled` → `api::check`；零第二份装配 |
| `gating_findings` | 同上 | 修改（口径；签名不变） | `pub fn gating_findings(root: &Path, elements: &[ArchiElement], findings: &[ArchiApiFinding]) -> Vec<ArchiApiFinding>` | 入选 = `element` 非空 ∧（`Declared`：载体的 `is_file()` / `None`：`doc.is_some()`）；`docMissing` 两侧都不成立 ⇒ 永不纳管 |
| `artifact_check` | 同上 | 修改（内部） | `pub fn artifact_check(root: &Path) -> Result<ArchiApiCheck, ApiError>` | 增 `elements_of(root)?`（启用判据需元素表）；三类差异计算与 DTO 形状不动；模型结构诊断 ⇒ `ApiError::Element` |
| `check` | `crates/core/archi/src/api/check.rs` | 修改（求值形态） | `pub fn check(input: ApiCheckInput<'_>) -> Vec<ArchiApiFinding>` | 十三值两支三段首错 + 三条 pair 锚点；零读内容、零写盘、零 spawn |
| `category_of` | 同上 | 修改（七值） | `pub fn category_of(kind: ArchiApiFindingKind) -> &'static str` | `DocMissing` / `DocNotFound` / `ApiDocMissing` → `"doc"`；其余六值逐字不变 |
| `severity` | `crates/core/archi/src/drift/report.rs` | 修改（三值） | `pub fn severity(self) -> ApiFindingSeverity` | `ApiUndeclared ⇒ Off`；`DocNotFound` / `ApiMissing` / `ApiSignatureMismatch ⇒ Error`；其余 `Warn` |
| `as_str`（`ArchiApiFindingKind` / `ApiFindingSeverity`） | 同上 | 修改（逐值补词） | `pub fn as_str(self) -> &'static str` | 线格式词 `docMissing` / `docNotFound` / `apiDocMissing` / `error` |
| `has_error_tier` | 同上 | 新增 | `pub fn has_error_tier(findings: &[ArchiApiFinding]) -> bool` | 「纳管子集含任一 `Error` 档」的唯一判据；`derive_status` 与 `passed()` 共用（MUST NOT 各写一份） |
| `ArchiDriftReport::passed` | 同上 | 新增 | `pub fn passed(&self) -> bool` | `errors` 空 ∧ `model.structural_errors` 空 ∧ `has_error_tier(&api_findings)` 为假；门禁布尔单点，`runner.rs` 只读 |
| `SearchScope::parse` | `crates/core/archi/src/dto.rs` | 修改（词面） | `pub fn parse(raw: &str) -> Result<SearchScope, String>` | `"elements"` / `"doc"` / `"all"`；`"docs"` 为非法值并在文案里列三个合法值（MUST NOT 保留别名） |
| `module_detail` | 同上 | 修改（投影） | `pub fn module_detail(root: &Path, fqn: &str) -> Result<ArchiModuleDetail, String>` | `doc` = `api = none` 时的模型原文、`declared` 恒 `null`；`docs` / `warnings` 装配删除 |
| `search` | 同上 | 修改（结果字段） | `pub fn search(root: &Path, query: &str, scope: SearchScope) -> Result<ArchiSearchResult, String>` | 结果携 `doc: Vec<String>` |
| `linked_doc_paths` | 同上 | **删除** | `pub fn linked_doc_paths(root: &Path) -> Result<BTreeSet<String>, String>`（原签名） | 关联集路径集随关联集消失；产品侧零消费者 |
| `search` | `crates/core/archi/src/search.rs` | 修改（语料；签名不变） | `pub fn search(graph: &ModelGraph, root: &Path, query: &str, scope: SearchScope) -> Result<SearchOutcome, String>` | 语料 = `path` 非空元素的在盘载体（经 `carrier_path` + `is_file` + `api::display`），排序 = 路径字典序 |
| `to_element` | `crates/core/archi/src/dto.rs` | 修改（可见性） | `pub(crate) fn to_element(element: &Element) -> ArchiElement` | 模型 → 元素投影单点（私有 `fn` 提 `pub(crate)`）：`doc` 逐元素拷贝；检索面复用同一份投影取 `doc` 事实，MUST NOT 另拼第二份 `ArchiElement`（模块级 `pub` 不变，出线类型清单成员不增删） |
| `archi_decisions_dir` | `crates/core/foundation/src/layout/mod.rs` | **删除** | `pub fn archi_decisions_dir(root: &Path) -> PathBuf`（原签名） | 唯一非测试消费者 `docs.rs` 已删 |
| `archi_decisions_rel_dir` | 同上 | **删除** | `pub fn archi_decisions_rel_dir() -> String`（原签名） | 唯一非测试消费者 `archi_knowledge.rs` 同步删该句 |
| `build` | `crates/infra/agent/src/archi_knowledge.rs` | 修改（格式串与实参） | `pub(crate) fn build(skeleton: &str) -> Result<String, String>` | 派生段少「决策记录目录」一段，其余三处方位与校验逐字不变 |
| `FENCE_OP` / `FENCE_TYPES` / `info_tokens` | `crates/core/archi/src/api/contract.rs` | 修改（可见性） | `pub(crate) const FENCE_OP: &str = "api-op";` / `pub(crate) const FENCE_TYPES: &str = "api-types";` / `pub(crate) fn info_tokens(info: &str) -> Vec<&str>` | 供 `has_api_fence`（启用判据与 `apiDocMissing`）复用；MUST NOT 抄第二份字面量或第二份切分 |

> 私有函数不进本表：`api::doc_facts` / `has_api_fence`（D4）、`search.rs::file_stem`（迁移后的私有函数）、`check.rs` 的 `missing_contracts` / `signature_mismatch` / `reexport_drift` / `shadowing` / `ref_missing` / `nested_cross_crate`。CLI 面零新增子命令（`archi-api-gen` 三子命令形态与退出码不变）；IPC 命令面零新增命令、零签名变化（出参字段换面见「路由 / API 设计」）。

### 类型定义

| 类型 | 所在文件 | 变化 | 说明 |
|---|---|---|---|
| `DocFact` | `crates/core/archi/src/api/mod.rs` | 新增 `enum`（`#[derive(Debug, Clone, Copy, PartialEq, Eq)]`，进程内不进 IPC） | `Absent`（`api = none` 且未声明 `doc`，无载体）/ `Missing`（载体已派生或已声明但不在盘）/ `Present { has_api_fence: bool }`（在盘 + 正文是否含 api fence 语言标） |
| `ApiSnapshot.doc_facts` | 同上 | 新增字段 | `pub doc_facts: BTreeMap<String, DocFact>`；FQN → 载体事实（`load_contracts` 同批产出） |
| `ApiCheckInput.doc_facts` | 同上 | 新增字段 | `pub doc_facts: &'a BTreeMap<String, DocFact>`；check 侧唯一读取面 |
| `Element.doc` | `crates/core/archi/src/model.rs` | 换面（`docs: Vec<String>` → `doc: Option<String>`） | 缺省 `None` |
| `RawElement.doc` | `crates/core/archi/src/parse/toml.rs` | 换面（`docs: Vec<Spanned<String>>` → `doc: Option<String>`） | `#[serde(default)]`；数组形态归 `invalidValue` |
| `ArchiModuleDetail` | `crates/core/archi/src/dto.rs` | 修改 | 删 `docs: Vec<DocRef>` 与 `warnings: Vec<ArchiWarning>`；新增 `doc: Option<String>`（position：`children` 之后、`api` 之前） |
| `ArchiSearchResult` | 同上 | 修改 | `docs: Vec<DocRef>` → `doc: Vec<String>` |
| `ArchiElement.doc` | 同上 | **新增（授权进场字段）** | `pub doc: Option<String>`，字段序紧跟 `entry`：模型字段的拷贝（与 `entry` 同型——单值、缺省 `null`、显式字段不省略），`api = declared` 恒 `null`；`to_element` 逐元素拷贝；与 `ArchiModuleDetail.doc` 同值同源（同一模型 `doc` 原文），是判据 / 纳管 / 检索 / 投影唯一的 `doc` 入参来源，**不是第三个事实源**；`ArchiElementBrief` MUST NOT 带 `doc` |
| `SearchScope` | 同上 | 修改 | 变体 `Docs` → `Doc`（线格式词 `doc`）；三值保留 |
| `DocRef` / `DocKind` / `DocSource` / `ArchiWarning` / `WarningKind` | 同上 | **删除** | 四字段条目、来源强度、形态标记、文档警告单值枚举全数退役（`WarningKind` 成空枚举不留壳） |
| `ApiFindingSeverity` | `crates/core/archi/src/drift/report.rs` | 修改 | 二值 → 三值（`Off` / `Warn` / `Error`），`ALL: [ApiFindingSeverity; 3]` |
| `ArchiApiFindingKind` | 同上 | 修改 | 十值 → 十三值（+ `DocMissing` / `DocNotFound` / `ApiDocMissing`），`ALL: [ArchiApiFindingKind; 13]` |
| `SearchOutcome` | `crates/core/archi/src/search.rs` | 修改 | `docs: Vec<DocRef>` → `doc: Vec<String>`（进程内） |
| `ArchiElement` | `crates/core/archi/src/dto.rs` | 修改 | 字段面唯一变化 = 在 `entry` 之后新增 `doc: Option<String>`（见上一行）；`fqn` / `kind` / `gate` / `path` / `summary` / `metadata` / `modelFile` / `modelLine` / `mark` / `api` / `entry` 逐字不变 |
| `ArchiElementBrief` / `ArchiApiRef` / `ArchiGraph` / `ArchiApiCheck` / `ArchiApiMeta` / `ArchiDriftReport`（字段面） | `crates/core/archi/src/dto.rs` / `drift/report.rs` | 零改动（字段面；`ArchiApiRef` 的 doc 散文串改述除外） | `ArchiElementBrief` 五项（`fqn` / `kind` / `gate` / `api` / `summary`）MUST NOT 带 `doc`；`ArchiApiRef.contractPath` 的语义扩为「同时是 `declared` 元素的入口文档路径」，字段名 / 顺序 / `null` 语义不变；`REPORT_VERSION = 3` |

### 配置

| 配置键 | 所在文件 | 变化 | 值类型 | 默认值 / 现值 | 说明 |
|---|---|---|---|---|---|
| `version` | `packages/desktop/package.json` | 修改 | `string` | `0.6.52` → `0.6.53`（+1 patch；落地时按 `HEAD` 现值递增，MUST NOT 照抄本表数字） | `tauri.conf.json` 经 `"version": "../package.json"` 跟随；`src-tauri/Cargo.toml` 的 `[package] version = "0.1.0"` MUST NOT 随动 |

> 配置项零新增：`drift/config.rs` 保持 `ignore` / `exclude` 两键封闭 + `deny_unknown_fields`（求值顺序写死在 `check.rs` 函数体，MUST NOT 引入配置键、MUST NOT 留空壳键或 TODO）；`package.json` 的 `api:export` / `api:check` / `check` 三段命令串逐字不变；`.dev-team/config.json`、`.dev-team/architecture/config.json`、`crates/core/config/**`（含 `config.schema.json` 镜像）、`tauri.conf.json`、`capabilities/default.json` 全部零 diff。

---

## 数据模型 (Data Model)

### 1 · 元素字段面（换后）

```toml
[element."App.Backend.Tools.ExportBindings"]     # api = none：入口文档由 doc 声明
kind  = "module"
path  = ["./packages/desktop/src-tauri/crates/tools/export-bindings/"]
api   = "none"
doc   = "./packages/desktop/src-tauri/crates/tools/export-bindings/README.md"   # 2b 新建该文件

[element."App.Backend.Infra.ArchiRuntime"]       # api 缺省 = declared：零 doc 行
kind  = "module"
path  = ["./packages/desktop/src-tauri/crates/infra/archi/src/"]
# 入口文档 = .dev-team/architecture/models/Infra/App.Backend.Infra.ArchiRuntime.md（派生，不写）
```

| `api` | 载体 | `doc` 字段 | 判定 |
|---|---|---|---|
| `declared` | 派生 `contract_path` | 不写 | 在盘 + 有 fence ⇒ 正常；不在盘 ⇒ `docMissing`；在盘无 fence ⇒ `apiDocMissing`；写了 `doc` ⇒ `apiUnexpected` |
| `none` | `doc` 声明 | 可选 | 在盘 ⇒ 正常；不在盘 ⇒ `docNotFound`；未写 ⇒ `docMissing`；派生路径冒出契约 ⇒ `apiUnexpected` |

### 2 · 载体事实（`DocFact`）

```text
for element in elements where !element.path.is_empty():
    载体 = carrier_path(root, element)                  // declared → contract_path / none → doc_path
    载体缺省（none 且未写 doc）        ──▶ Absent
    载体不在盘                          ──▶ Missing
    载体在盘 → 读一次文本 → 扫 fence     ──▶ Present { has_api_fence }
```

- **fence 判据**：行首 ``` 之后的 info string，按空白 / 逗号切 token（`contract::info_tokens`），`FENCE_OP ∈ tokens ∨ FENCE_TYPES ∈ tokens`。
- **已知精度上限（可接受）**：只判「有无语言标」，不判 fence 是否合法 / `### <op-id>` 是否齐 / `api-types` 是否重复——那些是 `contract::parse` 的 `Err` 面。
- **读失败**：`Present { has_api_fence: false }`（启用判据是探针）；契约解析读失败仍 `ApiError::Io`（既有面）。
- **持久化**：零。`DocFact` 是进程内事实，每次检查现算，零缓存、零落盘。
- **入参形态**：`elements` 是 `ArchiElement` 投影（`doc` 为其授权进场字段，事实校准 16）——模型 AST 不直接进判据面，故 `none` 支的载体只能从投影的 `doc` 取；检索面同样经 `dto::to_element` 复用这一投影（单点）。

### 3 · 判据面（十三值 / 三档 / 两锚点）

| 判据值 | 线格式词 | category | 档位 | 纳管 `status` | 成因（单值单成因） |
|---|---|---|---|---|---|
| `DocMissing` | `docMissing` | `doc` | warn | **否** | 入口文档未就位（declared：派生契约 md 不在盘 / none：`doc` 未声明） |
| `DocNotFound` | `docNotFound` | `doc` | **error** | 是 | `doc` 声明指不到盘（仅声明类有此形态） |
| `ApiDocMissing` | `apiDocMissing` | `doc` | warn | 是 | 契约 md 在盘但正文无 api fence 语言标 |
| `ApiMissing` | `apiMissing` | `contract` | **error** | 是 | 契约声明 op / `$defs` 而代码导出集里没有（形态① 已改名 `docMissing`） |
| `ApiSignatureMismatch` | `apiSignatureMismatch` | `signature` | **error** | 是 | 归一签名不一致 |
| `ApiUnexpected` | `apiUnexpected` | `entry` | warn | 是 | ① `api = none` 而派生契约路径在盘；② `api = declared` 而写了 `doc` |
| `ApiEntryMissing` | `apiEntryMissing` | `entry` | warn | 是 | 入口 `Absent` / `Invalid`（不含 `api = none` 元素） |
| `ApiUndeclared` | `apiUndeclared` | `contract` | **off** | —— | 代码有 external 面而契约未声明（不产出） |
| `ApiShadowing` / `ApiRefMissing` / `ApiNestedCrossCrate` | 同左 | `reexport` / `reexport` / `nesting` | warn | 是 | pair 锚点三条（MUST NOT 引用 `Model.relations`） |
| `ApiReexportDrift` / `ApiConstructorDeclared` | 同左 | `reexport` / `constructor` | warn | 是 | 透传键集不等 / 契约声明构造器 |

**求值序（写死，零配置键）**

```text
for element in elements where element.path 非空:
  api = none 支（不依赖入口）
    ① 派生契约路径在盘 → apiUnexpected（continue）
    ② 载体 Absent（doc 未声明）→ docMissing（continue）
    ③ 载体 Missing（doc 指不到）→ docNotFound
  api = declared 支
    ④ 载体 Missing / Absent → docMissing（continue）
    ⑤ 载体 Present { has_api_fence: false } → apiDocMissing（continue）
    ⑥ element.doc 被写了 → apiUnexpected（continue）
    ⑦ entry_state = Absent / Invalid → apiEntryMissing（continue）
    ⑧ 契约声明 op / $defs 而代码无实现 → apiMissing（continue）
    ⑨ 一致性（首条命中即停）：apiSignatureMismatch → apiReexportDrift → apiConstructorDeclared
pair 锚点（不进元素循环，与元素锚点互不挤占）
    ⓐ apiNestedCrossCrate → ⓑ apiRefMissing → ⓒ apiShadowing
```

分界依据：①–⑥ 只读模型 + 盘（零导出集依赖）；⑦ 起经 `element_symbols` 需 `EntryState::Resolved`——无入口即空符号集，⑧⑨ 无对象可判。

### 4 · `status` 与 `passed`

```text
纳管(entry) ⟺ entry.element 非空 ∧ (
      api = declared : 载体在盘
    ∨ api = none     : `doc` 字段已声明        ← 声明本身即承诺（docNotFound 判的正是它）
)

derive_status: skipped → errors(structural | errors[] | 纳管子集含 Error)
             → noChanges(可判文件 0) → warnings(warnings[] 非空 | 纳管子集非空) → clean
passed       = errors[] 空 ∧ structuralErrors 空 ∧ error 档 apiFindings 空
不变量        status == errors ⟺ passed == false
```

- `docMissing` 两侧都不满足纳管条件 ⇒ **永不纳管**（43 条在场而 `status = clean`、`passed = true`）。
- 结构性安全网：`error` 档只落在「入口文档已就位」的元素上（`apiDocMissing` / `apiMissing` / `apiSignatureMismatch` 需契约在盘含 fence；`docNotFound` 需 `doc` 已声明）⇒ 零入口文档的存量项目永不红。

### 5 · 线面（出线 DTO）

| DTO | 变化 | 取值来源 |
|---|---|---|
| `ArchiModuleDetail.doc: Option<String>` | 新增 | `api = none` → `element.doc` 原文；`declared` → `null` |
| `ArchiModuleDetail.docs` / `.warnings` | 删除 | —— |
| `ArchiSearchResult.doc: Vec<String>` | 换面 | `path` 非空元素的在盘载体，经 `api::display` 归一，路径字典序 |
| `ArchiElement.doc: Option<String>` | 新增（元素投影上的授权进场字段） | `to_element` 逐元素拷贝模型 `doc` 原文；`declared` 恒 `null`，字段不省略；`archi_graph` 的 `elements[]` 与 `archi_module` 的 `element` 同值同源；`ArchiElementBrief` 零 `doc` |
| `SearchScope` 线格式词 | `elements` / `doc` / `all` | `"docs"` 非法（文案列合法值） |

出线类型清单随 `bindings:export` 重导：`DocRef` / `DocKind` / `DocSource` / `WarningKind` / `ArchiWarning` 离场；`doc` 在 **DTO 面三处**进场（`ArchiModuleDetail.doc` / `ArchiSearchResult.doc` / `ArchiElement.doc`），其中 `ArchiSearchResult` / `SearchScope` 不属 IPC 命令面（agent 工具专用 DTO，事实校准 17）⇒ 生成物侧两处字段进场，且 `ArchiElement` / `ArchiModuleDetail` 本就在清单内 ⇒ **清单成员零增删、只有生成物内容变化**（`bindings/mod_test.rs` 的清单与归档断言同批随动，归测试相位）。

### 6 · 检索语料

| 面 | 改前 | 改后 |
|---|---|---|
| 语料 | 三源（`explicit` / `adjacent` / `text`，含 ADR 正文命中） | `path` 非空元素的在盘载体（declared → 派生 `contractPath`、none → `doc`） |
| 条目 | `DocRef` 四字段 | 一条 repo 相对路径串 |
| 命中 | `path` + `file_stem`（不读正文） | 同（`file_stem` 迁入 `search.rs`） |
| ADR（`decisions/*.md`） | 经 `text` 源自动关联 | **退出语料**；替代抵达路径 = 入口文档内链接 / 左栏文件树 / 通用文件搜索 |

---

## 路由 / API 设计 (Route / API Design)

IPC 命令面**零新增命令、零签名变化**（仍八条，全部显式 `root`，错误面 `Result<T, String>`，无 auth 概念），但三条读命令的出参语义换面：

| 命令 | method / 入口 | 输入 | 输出变化 | 说明 |
|---|---|---|---|---|
| `archi_module` / `archi_graph` | IPC（Tauri command） | `root` + `element`（前者） / `root`（后者） | `ArchiModuleDetail`：`docs` / `warnings` 退场、`doc: Option<String>` 进场；`ArchiGraph.elements[]` 的元素对象同一处换面（`ArchiElement.doc` 授权进场，`declared` 恒 `null`） | 单一取数（`archi::dto` 投影）：两条命令共用 `to_element`，故 `doc` 同值同源；agent 工具 `archi_module` 同一投影，响应随之换面 |
| `archi_search` | agent 工具（非 IPC，前端零消费者） | `query` + `scope`（`elements` / `doc` / `all`） | `ArchiSearchResult.doc: Vec<String>` | `scope = "docs"` 显式 `Err`；语料零正文 |
| `archi_drift` | IPC + 门禁 runner | `root` + `mode` | `apiFindings` 的取值 / 档位 / 条数换面；`status` 可被 error 档抬到 `errors`；`passed` 语义扩 | `REPORT_VERSION` 仍 3；判定仍单源 `drift::check` |
| `archi_api_check` | IPC | `root` | `enabled` 语义收紧（同一 `bool` 字段，判据变） | 模型结构诊断 ⇒ `Err`（既有错误面首次兑现）；三列差异与 `meta` 字段零变化 |

无 HTTP 端点、无 CLI 子命令变化：`archi-api-gen` 的 `generate` / `--check` / `bootstrap` 三子命令形态与退出码不变（`doc` 不是生成物）。

---

## 依赖 (Dependencies)

| 类别 | 依赖 | 用途 | 变化 |
|---|---|---|---|
| 运行时（Rust） | `std::fs` / `std::path` | 载体探盘与单次读文本（`doc_facts`）、路径派生 | 零新增外部 crate |
| 运行时（Rust，**删除**） | `ignore`（`WalkBuilder`） | 原用于 `docs.rs` 的邻近扫描 | `docs.rs` 删除后 `core/archi` 不再需要该依赖；`Cargo.toml` 若有该依赖条目须同步（`cargo` 未用依赖不报错，删除可选——**本变更不动 `Cargo.toml`**，见 OQ-6） |
| 运行时（Rust） | `foundation`（workspace 内） | `archi_models_dir` / `archi_graph_*` / `is_archi_rel_path` | 既有边不变；`archi_decisions_*` 消费者归零 |
| 运行时（Rust） | `serde` / `serde_json` / `specta` / `sha2` / `toml` | 模型解析、DTO 出线、摘要 | 零变化 |
| 运行时（TypeScript） | React / `FileViewer` / `read_file` | 抽屉入口文档单条 | 零新增依赖 |
| 构建 / 测试 | `cargo fmt` / `clippy` / `nextest`、`vp check` / `vp test` / `knip` | 门禁四段 | 零新增工具；`package.json` 脚本面零变化 |
| 构建 / 测试 | `bindings:export`（specta 生成） | `bindings.ts` / `bindings.zod.ts` 重导 | 命令串零变化，仅执行一次 |
| 构建 / 测试 | `DESKTOP_GOLDEN_REWRITE=1` | 十六份 drift 金样显式重录 | 既有开关 |

---

## 验收标准回应 (AC Mapping)

| AC | 设计落点 | 判据形态 / 观测 |
|---|---|---|
| AC-1 字段面换面干净 | D1 + 修改文件表前两行 + `01-app.toml` 行 + 删除 `docs.rs` | `model.rs` / `parse/toml.rs` 只有 `doc: Option<String>`；**架构域产品源码**（射程见事实校准 15）`docs` 字样零命中（`api/extract/rust.rs:35` 的散文例外，OQ-3）；白名单含 `doc` 不含 `docs`；反向判据：模型写回 `docs = [..]` ⇒ `unknownField` 停摆（`01-app.toml` 已同批删行，`archi_graph` 零诊断） |
| AC-2 `doc` 适用面结构性成立 | D3 / D8 / D10 + `paths::doc_path` / `carrier_path` | `declared` + `doc` ⇒ 一条 `apiUnexpected`（`category = entry`、warn）且不参与其它判定；`none` + `doc` 指不到 ⇒ 一条 `docNotFound`（`category = doc`、error）；`doc` 形态校验只在 `api::paths`（`docs::normalize_rel` 已删，零第二份归一） |
| AC-3 十三值与三段两支 | D8 / D9 / D11 + `category_of` 七值 | `ALL` 长度 13、`as_str` 逐值对拍、`severity()` 三值（`Error` 恰 `docNotFound` / `apiMissing` / `apiSignatureMismatch`）；每元素锚点首错（同元素只一条）；pair 锚点独立且序 `nested → ref → shadowing`；反向判据：单元素不同时报两支 |
| AC-4 `passed` 与 `status` 同批 | D11 / D12 + `has_error_tier` 单点 + `ArchiDriftReport::passed` | `passed = errors 空 ∧ structuralErrors 空 ∧ error 档 apiFindings 空`；`derive_status` 在纳管子集上分档；纳管口径 = `element` 非空 ∧（declared：载体在盘 / none：`doc` 已声明），`docMissing` 永不纳管；三条不变量（`status == errors ⟺ passed == false` / 零入口文档 ⇒ `clean` / 43 条 `docMissing` 在场而 `status = clean`） |
| AC-5 第六段读数与成因 | 事实校准 13 + 「数据模型」§3/§4 | `docMissing 43`（41 改名 + 2 首入面）、`apiMissing 0`、其余十一值 0、`status = clean`、`passed = true`、分母 44；spec 与 `reports/` 写明「不是新增 43 条债」，`41` 与 `43` MUST NOT 并排写成债务增长 |
| AC-6 线面与检索面换面 | D13 / D14 + 事实校准 16 + `dto.rs` / `search.rs` / `lib.rs` / `mount.rs` | `ArchiModuleDetail` 无 `docs` / `warnings`、有 `doc`（仅 `api = none` 有值）；`ArchiSearchResult.doc: Vec<String>`；`ArchiElement` 只在 `entry` 之后新增 `doc: Option<String>`（模型拷贝、与 `entry` 同型、`declared` 恒 `null`；`archi_graph` 与 `archi_module` 同值同源），`ArchiElementBrief` / `ArchiApiRef` 字段面零变化；`scope` 词 `elements` / `doc` / `all`，`"docs"` 非法并列合法值；`DocRef` / `DocKind` / `DocSource` / `WarningKind` / `ArchiWarning` / `DocWarning` / `linked_doc_paths` 全仓零命中；`bindings:export` 幂等 |
| AC-7 启用判据收紧 | D5 / D6 / D7 + 事实校准 2–4 | `enabled(root, elements)` = 任一 `path` 非空载体在盘且含 `api-op` / `api-types` 语言标；常量单点复用 `contract::{FENCE_OP, FENCE_TYPES}`；反向判据：`models/notes.md`（非载体）与 `<Domain>/notes.md`（无 fence）都不构成启用证据；本仓仍 `true` |
| AC-8 adr 派生与知识文本清理 | D18 + `foundation/layout` 行 + 知识文本两行 | `archi_decisions_dir` / `archi_decisions_rel_dir` / `DECISIONS_DIR_NAME` 全仓零命中（含 `*_test.rs`）；`archi_knowledge()` 输出零「决策记录目录」串、含 `doc` 字段纪律与两段示例、体量上限不变；`decisions/0001-model-granularity.md` 仍在盘 |
| AC-9 前端单条入口文档 | D15 + `element-drawer.tsx` 行 | 零 `DocsSection` / 零 `DocRef` 依赖；`api = none` 且 `doc` 非空 ⇒ 一条「入口文档」（`FileViewer` 可开）；`declared` ⇒ 只有契约段一条 `ContractPathRow("契约")`，MUST NOT 两条；空态 testid / 文案随字段面更名 |
| AC-10 金样与语料守护 | 事实校准 12 + 测试面交接注 | `api/**` sidecar 仍恰 13 份（新用例落既有文件）；十六份 drift 金样 `DESKTOP_GOLDEN_REWRITE=1` 显式重写 + 人工确认，逐份留痕成因；`desktop-archi-api-check` 的「已知缺口」句就地留痕「十判据各自正反例」MUST NOT 读成「十三判据各自正反例」 |
| AC-11 零扩散与版本纪律 | D18 / D19 + 配置表 | `git status --porcelain` ⊆ D18 白名单（含 `01-app.toml` 一行删除）；`package.json` +1 patch（`0.6.52` → `0.6.53`）；`tauri.conf.json` 仍 `"version": "../package.json"`；`src-tauri/Cargo.toml` crate 版本逐字不变；`openspec/specs/` 除 9 个 delta 能力外零 diff（`desktop-archi-file-tree` 零 diff 是判据之一） |

---

## 待决问题 (Open Questions)

| # | 事项 | 现状 | 处置 |
|---|---|---|---|
| OQ-1 | `desktop-archi-canvas` delta 内部两处措辞互斥：「declared 元素只有契约段一条入口文档」scenario 写「`declared` 与 `api = none` 未声明 `doc` 时『入口文档』段整段不渲染」，而「空态可辨」scenario 要求 `api = none` 且未声明 `doc` 时呈空态文案 | **本稿取 D15**：`declared` 不渲染、`none` 恒渲染（`doc` 非空 → 单条；`null` → 空态文案 + `archi-entry-doc-empty`）——依据是 AC-9 要求「空态 testid 与文案随字段面更名」，无空态则该要求无对象 | spec 措辞需在归档前把第一处改成「`declared` 时该段整段不渲染」；本变更实现按 D15 落地 |
| OQ-2 | `api::enabled` 的字面判据含 `api = none` 元素的 `doc` 载体（一份带 `api-op` 示例 fence 的 README 会启用契约面） | **按字面口径落地**（同一条 `doc_facts` 事实、零 `api` 分支）；风险极低（`none` 元素的 README 通常不带契约 fence） | 若将来要限定为「`api = declared` 元素的载体」，属独立 spec 裁决（同时要解释为何不读 `none` 的 doc）；本变更 MUST NOT 自行收窄 |
| OQ-3 | `crates/core/archi/src/api/extract/rust.rs:35` 的模块 doc 仍以散文形式引用已删除的 `docs.rs` / `resolved_within` | 该文件在 proposal「不要修改」表内（`api/extract/**` 零改动） | 保留（陈旧散文不产生编译 / 判据影响）；清理属独立 doc 变更；本变更 MUST NOT 顺手改它 |
| OQ-4 | `foundation::is_archi_rel_path` 在 `docs.rs` 删除后零非测试消费者，模块 doc 的「归档冲突分流的唯一判据」表述亦失指 | `pub fn`（无 `dead_code` 告警）；提案七只授权删 `decisions` 三符号 | 保留符号；删符号 / 改 doc 属独立清理变更（若删，需同批改 `mod_test.rs` 的三条用例） |
| OQ-5 | 门禁 detail 文本的已知呈现缺口：`status = errors` 由 error 档判据**单独**触发时，`compose_text` 的「N 条违例」只计 `errors[]`（不含 `apiFindings`），文案会写「0 条违例」 | `desktop-archi-drift` delta 明写 `runner.rs` 零逻辑改动（只随同源语义变化）⇒ 本变更只改 `passed` 一行（D16） | 报告内 `apiFindings` 有全量、架构页「契约」视图可见；文案补齐需同批改 spec 与 `runner_test` 的 detail 断言，建议单开一个小变更（或在 verify 相位由 spec 侧裁决后追加一行计数） |
| OQ-6 | `core/archi/Cargo.toml` 的 `ignore` 依赖在 `docs.rs` 删除后是否仍需保留 | `scan/mod.rs:19` 仍消费 `ignore::WalkBuilder`（既有）⇒ 本变更**不动 `Cargo.toml`** | 无动作；若将来依赖清理，属独立变更 |
| OQ-7 | 三处「双读」取舍：`findings` 下在盘载体被读两次（一次 fence 扫描、一次契约解析） | 本仓在盘载体 **1** 份，且两次读服务两个不同事实（D4 / D6 以「装配单点」优先于省一次读） | 保留；若将来契约数量到百级，可在 `load_contracts` 内复用同一份 source 算 `has_api_fence`（`ApiSnapshot.doc_facts` 的字段面不变） |
| OQ-8 | `packages/desktop/src/app.test.tsx:316` 的 `docs: []` / `warnings: []` 载荷未列入 proposal「测试文件」表 | 换面后不修改即 `tsc` 红（事实校准 9） | 归测试相位按机械面随动（`doc: null`）；建议在 proposal 测试表补一行 |
| OQ-9 | `desktop-archi-model` / `desktop-archi-contract-panel` 的措辞「`doc` 三处进场……经 `bindings:export` 反映到生成物」在生成物侧只兑现两处（`ArchiSearchResult` / `SearchScope` 是 agent 工具专用 DTO，不进 `bindings.ts`） | 本稿按事实校准 17 的精确读法落地：三处是 **DTO 字段面**口径，生成物侧两处、清单成员零增删 | 不改 spec（枚举本身正确，只是「经 `bindings:export`」的修饰语在工具面 DTO 上不成立）；若 spec 侧要求字面严格，可在归档前把该句改成「两处（IPC 面）+ 一处（工具响应面）」，本变更实现零改动 |
