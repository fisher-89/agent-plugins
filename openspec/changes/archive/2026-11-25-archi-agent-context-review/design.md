# 设计: archi-agent-context-review

> 变更: `archi-agent-context-review`（三资产分工 · 接口真相搬移 · 写闸 · 判据收缩 · 工具面与知识面）
> 输入: 本目录 `proposal.md`（定稿）与 `specs/` 下 11 份 delta（`desktop-archi-api` / `desktop-archi-api-check` / `desktop-archi-agent` / `desktop-archi-model` / `desktop-archi-drift` / `desktop-archi-contract-panel` / `desktop-archi-canvas` / `desktop-agent-execution` / `desktop-phase-prompts` / `desktop-ipc-type-bindings` / `repo-archi-model`）
> 性质: 一次原子交付——真相搬移（读面换 schema）+ 写面补闸（`validate_schema`）+ 判据被动收缩（13 → 10）+ 工具面三资产 + Agent 上下文重写 + 生成面让位与规则面同步。**零模型字节改动**（55 元素 / 129 关系 / `modelHash` 逐字不变），**零 `.i.rs` / `.i.ts` 字节改动**，**零迁移**（11 份 schema 与 fence 逐项等价）。

## 架构组件 (Architecture Components)

### C1 · 接口校验单点（`api/schema.rs`）

- **职责**：接口定义文件的**唯一**合法性判据——`validate_schema(fqn, text)` 九道闸一趟走完并产出 `Contract`；写闸（`write` / `edit`）与装载期（`load_contracts`）共用同一函数，只差错误映射。
- **文件**：`packages/desktop/src-tauri/crates/core/archi/src/api/schema.rs`（改）。
- **依赖**：`serde_json`（文本 → `Value`）、`crate::api::contract::{Contract, duplicate_keys, verify}`、`crate::api::paths::{ref_relative, schema_fqn}`、`super::{kind_of, paths, short_digest, ApiError, ApiMeta, DEFS_FRAGMENT, SCHEMA_FILE_SUFFIX}`（既有导入面）。
- **技术**：闸序固定 = ① 文本层重复键 → ② JSON 可解析 → ③ identity → ④ 键空间（op 名与类型名互斥）→ ⑤ 形态白名单 → ⑥ 镜像 → ⑦ 跨元素 `$ref` 前缀 → ⑧ 顶层键封闭 → ⑨ 复用三闸 + `verify_defs`（同一趟，不另编号）。`build_schema` 退役；`verify_defs` / `degraded_marks` / `build_index` / `ApiFormStatus` / `DEGRADED_KINDS` 逐字保留。

### C2 · 契约投影与复用三闸（`api/contract.rs`）

- **职责**：`Contract` 由「md 解析器产物」转为「schema 投影」——`Contract::from_schema` 解包 `properties.<k>.x-function`、丢弃 `description`、按 `$defs` 分类 `types` / `reexports`；`verify` 只留七族名字 / `x-function` 落点 / `required ⊆ properties` 三闸。
- **文件**：`crates/core/archi/src/api/contract.rs`（改，约 1095 → 400–500 行）。
- **依赖**：`serde_json::{Map, Value}`、`super::{kind_of, ApiError}`、`super::paths`（无，`from_schema` 不派生路径）、`crate::api::reexport_target`（`api/mod.rs`，目标 FQN 反查单点）。
- **技术**：md 解析主体全部退役（`parse` / `finish_section` / `parse_object*` / `string_end` / `decode_key` / `file_title` / `collect_reexport_prose` / `collect_surface_table` / `is_cjk` / `info_tokens` / `duplicate_key_error` + `ContractSection` + `FENCE_OP` / `FENCE_TYPES` / `FENCE_REEXPORT` + `reexport_prose` / `sections` 字段）；`duplicate_keys` 保留（`pub(crate)`，供 C1 文本层）、`is_identifier` 提为 `pub`（供 `link` 校验）、`is_constructor` 保留。

### C3 · 落点与载体派生（`api/paths.rs`）

- **职责**：资产落点派生单点；资产①（文档）的落点**恒为**派生 `<models>/<分层目录…>/<FQN>.md`（与 `api` 无关），`carrier_path` / `doc_path` 退役。
- **文件**：`crates/core/archi/src/api/paths.rs`（改）。
- **依赖**：无新增（`foundation::layout` 既有）。
- **技术**：`contract_path` / `schema_path` / `index_path` / `entry_state` / `ref_relative` / `generated_path` / `directory` / `normalize_root` / `language_of` 逐字不变；新增 `pub fn schema_fqn(file_name: &str) -> Option<&str>`（文件名 → FQN 的**唯一**反查，`reexport_target` 与写闸同调）；`checked_root` 提为 `pub(crate)`（写面 `path` 存在性判定复用「归一 + 越根判据」，避免第二份派生）；`is_fqn` 提为 `pub`（`link` 的 FQN 形态判据复用）。

### C4 · 读面装配 / 判据入口 / 生成面（`api/mod.rs`）

- **职责**：`load_contracts` 只读 schema；`enabled` / `gating_findings` 换锚；`ref_projection` 三路径各自独立；`render_all` 只产三类生成物；`bootstrap` 出双草稿。
- **文件**：`crates/core/archi/src/api/mod.rs`（改）。
- **依赖**：`schema::validate_schema`（C1）、`contract::Contract`（C2）、`paths::{contract_path, schema_path, index_path, ...}`（C3）、`dto::{ApiStatus, ArchiApiCheck, ArchiApiRef, ArchiElement}`、`drift::report::ArchiApiFinding`、`extract::extract_element`。
- **技术**：`ApiSnapshot.doc_facts: BTreeMap<String, DocFact>` → `doc_present: BTreeMap<String, bool>`（`DocFact` 类型删除）；`doc_facts` → `doc_presence`（`contract_path(...).is_some_and(is_file)`）；删 `has_api_fence` / `facts_enabled`；`bootstrap` 返回新增 `BootstrapDrafts`；`BOOTSTRAP_NOTE` 删除。

### C5 · 三类差异与孤儿扫描（`api/diff.rs`）

- **职责**：应生成集 vs 盘上实况的三列比对；孤儿扫描收面。
- **文件**：`crates/core/archi/src/api/diff.rs`（改）。
- **依赖**：`super::render::workspace_render`、`crate::scan::walk_source_files`。
- **技术**：`api_dir_artifacts` 只留 `<archi_graph_dir>/INDEX.toml` 一个根（`*.schema.json` 退出枚举）；`orphaned` / `mismatched` / `missing` 三列语义与稳定序逐字不变；深度上限 / 符号链接不跟随 / 有界 `read_dir` 三条纪律逐字沿用。

### C6 · 十判据求值（`api/check.rs`）

- **职责**：十判据的唯一实现；`api = none` 支 2 步、`api = declared` 支 4 步 + pair 锚点三条。
- **文件**：`crates/core/archi/src/api/check.rs`（改）。
- **依赖**：`drift::report::{ApiFindingSeverity, ArchiApiFinding, ArchiApiFindingKind, ArchiDriftOccurrence}`、`dto::{ApiStatus, ArchiElement}`、`contract::{is_constructor, Contract}`、`extract`、`paths::{entry_state, EntryProblem, contract_path, schema_path}`。
- **技术**：删 `apiReexportDrift` / `apiDocMissing` / `docNotFound` 三支与 `reexport_drift` / `doc_not_found` / `api_doc_missing`；`ApiCheckInput.doc_facts` → `doc_present`；`doc_missing_*` 收成一个 `doc_missing_message`；`category_of` 七组（`doc` 只剩 `docMissing`）；`is_pure_ref_alias` 提为 `pub(crate)`——纯可见性、零行为改动，落地序在 C10 之前（`tasks.md` 阶段 1 单列此可见性提升，使阶段 2 的 `dto.rs` 二分不依赖阶段 3），C10 的 `interface` 三数组与本节 `declared_types` 共用同一份纯 alias 判据，MUST NOT 各写一份。

### C7 · 判据词汇与档位（`drift/report.rs`）

- **职责**：`ArchiApiFindingKind` 十三 → 十值；`ALL: [Self; 10]`；`as_str` / `severity` / category 打标随动。
- **文件**：`crates/core/archi/src/drift/report.rs`（改）；`crates/core/archi/src/drift/{mod,config}.rs` **零改动**（提案「不要修改」明列；`mod.rs` 两处纳管注释的旧措辞归清理轮，见 Q-8）。
- **依赖**：无新增。
- **技术**：`error` 恰 `ApiMissing` / `ApiSignatureMismatch` 两项；`off` 恰 `ApiUndeclared`；其余七项 `warn`；`ArchiApiFinding` / `ApiFindingSeverity` / `REPORT_VERSION`（3）零改动。

### C8 · 写面反馈五条（`edit/warnings.rs`）

- **职责**：把既有判据面的词在写面重述为「已落盘、未阻断」的事实——条件表 7 → **5** 行，`doc` 零诱发。
- **文件**：`crates/core/archi/src/edit/warnings.rs`（改）；`edit/mod.rs` 的调用点签名不变（`upsert_warnings(root, element, &changed, creating)` / `delete_warnings(root, fqn)`）。
- **依赖**：`api::paths::{contract_path, schema_path, checked_root}`、`ArchiDriftKind::as_str` / `ArchiApiFindingKind::{as_str, severity}`、`ApiFindingSeverity::as_str`、`Path::is_file`。
- **技术**：`path_on_disk` 改走 `checked_root` + `root.join`（`doc_path` 退役）；`contract_on_disk` → `schema_on_disk`；`api_warnings` 只剩两条（`none` ∧ schema 在盘 / `declared` 零条目）；`docMissing` 只由 `created` 诱发；`DocNotFound` 在写面零出现。

### C9 · 模块字段语义（`model.rs`）

- **职责**：`api` 的语义注释由「入口文档载体开关」改为「接口承诺开关」；`doc` 的「只有 `api = none` 可写」降为**约定**（零判据读者）。字段集与八字段顺序零改动。
- **文件**：`crates/core/archi/src/model.rs`（改，注释 + doc 字面量）。
- **依赖**：`crate::dto::{ApiStatus, DiagnosticKind, ElementKind, GateTier}`（既有导入面，零新增外部依赖、零新增 crate 边）。语义的**读者**是代码事实而非注释：C4（`ref_projection` 的 `status` 判 schema 在盘）/ C6（判据面 `declared` 支）/ C8（写面 `apiUnexpected`）/ C10（`ArchiApiRef` 与 `interface` 投影）各自按「`<FQN>.schema.json` 在盘」求值，故本组件与其余十二个组件**零编译期耦合**——它只产出两份字段文档，`api` / `doc` 的行为面零改动。与 `parse/**`（TOML 读面）同样零耦合：`doc` 的路径禁令无机械实现（既有债务，Q-9 归后续 change），本组件 MUST NOT 借注释改写顺手补一条判据。
- **技术**：纯注释 / 字段文档改写，零逻辑行、零类型行——`Element` 的字段集、字段顺序、类型与 `ApiStatus` 二值零改动，`Model` / `Relation` / `SourceSpan` 与聚合层逐字不变；`api` 注释删去「入口文档载体开关」一层（资产① 的落点恒为派生 `<FQN>.md`，与 `api` 无关），改述「接口承诺开关」（`declared` = 断言本模块应有一份 `<FQN>.schema.json`）；`doc` 注释标「**约定**（非机械判据）+ 零判据读者（判据 / 纳管 / 派生 / 检索都不读）」，并保留「单值、repo 相对 POSIX」口径；`metadata` / `entry` 等相邻字段注释逐字不动。

### C10 · 出线投影与三资产（`dto.rs` + `search.rs`）

- **职责**：`archi_module` 一次给全三份资产（`assets` / `reading`）；三路径各自独立；检索语料换两条固定规则。
- **文件**：`crates/core/archi/src/dto.rs`（改）、`crates/core/archi/src/search.rs`（改，机械连带见 D-21）。
- **依赖**：`api::paths::{contract_path, schema_path, display}`、`api::check::is_pure_ref_alias`、`api::ref_projection`（`api_ref_of` 单点）、`drift::file_hash`。
- **技术**：新增四出线类型 + `ArchiModuleDetail` 两字段（声明序在 `file_hash` 之后）；`interface` 只读 `<FQN>.schema.json` 的键名（`ops` = 带 `x-function` 的 `properties` 键、`types` = 非纯 alias 的 `$defs` 键、`reexports` = 纯 alias 的 `$defs` 键），`doc` 只读 `##` 段名 + 字节 + 行数；`reading` 为静态常量；`search` 语料 = 「在盘 `<FQN>.md` ∪ 在盘 `<FQN>.schema.json`」，与 `api` 状态 / `doc` 字段无关。

### C11 · 接口写闸（`sdk/tools.rs`）

- **职责**：`write` / `edit` 落 `<archi_models_dir>/**/<FQN>.schema.json` 前调 `archi::api::schema::validate_schema`，`Err` 零落盘、成功后响应追加「接口已校验」。
- **文件**：`crates/infra/agent/src/sdk/tools.rs`（改）。
- **依赖**：`archi::validate_schema`（经 C1 + `archi::lib` 出口）、`foundation::layout::archi_models_rel_dir`、`archi::schema_fqn`。
- **技术**：`write` / `edit` 增 `root: &Path` 入参（`execute` 两个调用点机械改动）；`edit` 校验 `old → new` 替换后的候选正文；路径判定用 sandbox 后的绝对路径 `starts_with(root.join(<models rel>))`；非该路径零校验、零额外 IO；不触 `ProtectRules` / 档位 / 沙箱。

### C12 · 工具面（`sdk/archi_tool.rs` + `sdk/mount.rs`）

- **职责**：`archi_api` 三动作（`check` / `bootstrap` / `link`）；`archi_module` 的 `assets` / `reading` 由 DTO 透传（零拼装）；三处描述同步。
- **文件**：`crates/infra/agent/src/sdk/archi_tool.rs`（改）、`crates/infra/agent/src/sdk/mount.rs`（改）。
- **依赖**：`archi::api::bootstrap`（返回 `BootstrapDrafts`）、`archi::api::paths::{ref_relative, is_fqn}`、`archi::is_identifier`、`tools::cap_result`、`WRITE_LIMITS`。
- **技术**：`bootstrap` 响应 `{ schema, doc }`；`link` 纯计算零读盘写盘（跨元素 `ref_relative(from,to) + to + ".schema.json#/$defs/<name>"`；本模块 `"#/$defs/<name>"`）；`action` 词汇外值 `is_error` 点名三值；`archi_module` 的执行体零改动；`mount.rs` 的 `archi_api` 参数面加 `from` / `to` / `name`。

### C13 · 装配与知识（`compose.rs` + `archi-knowledge.md` + `archi_knowledge.rs`）

- **职责**：`archi_matrix` 两格补 `archi_api`（顺序对齐 spec 列出的行序）；change 臂在写权放开时注入同一份架构知识（判据单点 `model_write_lock(provenance).is_none()`）；知识骨架重写为五段。
- **文件**：`crates/infra/agent/src/compose.rs`（改）、`crates/infra/agent/src/archi-knowledge.md`（改）、`crates/infra/agent/src/archi_knowledge.rs`（改）。
- **依赖**：`mount::ARCHI_API_TOOL`、`crate::archi_knowledge::knowledge()`、`foundation::layout`（派生段）。
- **技术**：知识为「尽力增强」——拼装失败降级 `None`，工具面与 `before_done` 不变；骨架 ≤ **1800 B**、全文 ≤ **2000 B**（`lf_bytes(trim_end)` 口径，实测冻结）；零围栏、零值域行、零诊断词、零 `<>` 占位符。

### C14 · 规则面 / CLI / 前端（prompts / `main.rs` / `AGENT.md` + `CLAUDE.md` / views）

- **职责**：三份相位 prompt 的设计顺序与接口同步句；CLI 三类产物与双草稿 JSON；仓库根契约纪律节恰五条；前端三处注释 / 文案。
- **文件**：`crates/core/workflow/src/prompts/{proposal-planner,dev-design-planner,implementation-generator}.md`、`crates/tools/archi-api-gen/src/main.rs`、`AGENT.md`、`CLAUDE.md`、`packages/desktop/src/views/architecture/**`（改）。
- **依赖**：`crates/tools/archi-api-gen/src/main.rs` **跨组件依赖 C4 的 `archi::api::bootstrap`**——`bootstrap` 的返回类型改为 `BootstrapDrafts { schema, doc }` 后，CLI 的 `Command::Bootstrap` 只取 `drafts.schema` / `drafts.doc` 两字段打印（P7：工具出口与 CLI 出口同源同形，MUST NOT 在 `main.rs` 另造一份骨架拼装，否则双草稿会分叉出第二份真相）；该类型经 `crates/core/archi/src/lib.rs` 出口面 `pub use` 为 `archi::BootstrapDrafts`（单点出口纪律）；单行 JSON 的序列化用 CLI crate **既有** `serde_json`（零新增依赖，MUST NOT 手写转义）。`crates/core/workflow/src/prompts/**` / `AGENT.md` / `CLAUDE.md` / `packages/desktop/src/views/architecture/**` 四者**零代码依赖**（纯文本改写，读者是人类与模型，不进编译单元）。
- **技术**：`main.rs` 的 `Command::Bootstrap` 把两段文本经 `serde_json` 序列化成单行 JSON（键序 `schema` / `doc`），`generate` 与 `Usage` 文案改三类产物（`.schema.json` 不再是产物），`Command` 枚举、子命令面、退出码语义（0 / 1 / 2）逐字不变；prompt 只加句、零路径模板、零字段形态长表、零占位符（其余 11 份逐字不变）；`AGENT.md` / `CLAUDE.md` 节内正文逐字相同（同提交改两份，R12）；前端三处只改注释 / 文案（`contract-panel.tsx` 冷启动一句、两处 JSDoc），渲染与取数零改动（D-22）。

## 变更清单 (Change Inventory)

### 新增文件

<!-- 本变更零新增源文件：四类新 DTO 类型落既有 `dto.rs`，校验闸落既有 `schema.rs`，`Contract::from_schema` 落既有 `contract.rs`，写闸落既有 `tools.rs`，知识骨架改写既有 md；测试一律落既有 sidecar（`api/**` 不新增文件，`tests/corpus/api/` 仍零存在）。 -->

### 修改文件

| 路径 | 具体修改 | 说明 |
|---|---|---|
| `packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs` | ① 模块头 doc 重写；② `ApiSnapshot.doc_facts: BTreeMap<String, DocFact>` → `doc_present: BTreeMap<String, bool>`，`DocFact` 删除；③ `ApiCheckInput.doc_facts` → `doc_present`；④ `doc_facts` → `doc_presence`，删 `has_api_fence` / `facts_enabled`（后者的输入 `doc_facts` 已退役，启用判据并入 `enabled`）；⑤ `load_contracts` 只读 schema（`schema::validate_schema` 装配 `Contract`；不在盘 ⇒ 跳过；坏 schema ⇒ `ApiError` 停摆）；⑥ `enabled` = 任一 `path` 非空 ∧ `api = declared` 元素 schema 在盘（`artifact_check` 与 `findings` 同调本函数）；⑦ `gating_findings` = `declared` ∧ schema 在盘；⑧ `ref_projection` 三路径各自按自身在盘事实出线 + `status` 改判 schema 在盘；⑨ `render_all` 只产 `.i.rs` / `.i.ts` / `INDEX.toml`；⑩ `index_entries` 的 `present` 改判 schema 在盘；⑪ `bootstrap` 返回 `BootstrapDrafts`，`bootstrap_text` 拆 `schema_draft` / `doc_draft`，删 `BOOTSTRAP_NOTE`；⑫ 新增 `SCHEMA_WRITER_NOTICE`（`GENERATED_NOTICE` 逐字保留，D-12）；⑬ `check` 的 doc 措辞「十三条」→「十判据」 | 读面换真相的装配单点；`enabled` / `gating_findings` 同源，命令层零第二份判定 |
| `crates/core/archi/src/api/contract.rs` | ① `Contract::from_schema` 新增（解包 `x-function`、丢弃 `description`、`line = 0`；`types` = 非纯 alias 的 `$defs`；`reexports` = 可反查目标 FQN 的纯 alias）；② md 解析主体与 `ContractSection` / `FENCE_*` / `reexport_prose` / `sections` 删除；③ `verify` 保留七族名字 / 落点 / `required ⊆ properties` 三闸，删 op id 重复闸与 types⊆$defs 死闸、散文表格闸、reexport 键集闸；④ `duplicate_keys` → `pub(crate)`；⑤ `is_identifier` → `pub`；⑥ 模块头 doc 重写 | 由「md 解析器」退为「schema 校验 + `Contract` 投影」（约 1095 → 400–500 行）；`is_constructor` / `verify_x_function_positions` 及辅助件逐字保留 |
| `crates/core/archi/src/api/schema.rs` | ① `build_schema` 删除；② 新增 `verify_identity` / `verify_keyspace` / `verify_shape` / `verify_mirror` / `verify_ref_prefix` / `verify_top_level_keys`；③ 新增唯一公开入口 `validate_schema(fqn, text)`（九道闸 + `verify` + `verify_defs` 同趟）；④ `build_index` 的 `present` 头注释改写 + 说明段第二行改一句话（review 口径 + `declared` = 接口文件在案）；⑤ 模块头 doc 重写 | `verify_defs` / `degraded_marks` / `build_index` / `ApiFormStatus` / `DEGRADED_KINDS` 结构逐字保留；闸面是「文件级自洽」，零跨元素 |
| `crates/core/archi/src/api/paths.rs` | ① `carrier_path` / `doc_path` 删除；② 新增 `schema_fqn(file_name)`（`SCHEMA_FILE_SUFFIX` 唯一反查），`reexport_target` 改调它；③ `checked_root` → `pub(crate)`；④ `is_fqn` → `pub`；⑤ `is_repo_relative` 仍有 `paths.rs` 内读者（`checked_root` / `entry_state`），可见性不变（`pub(crate)`，非「无读者即删」）；⑥ 相关 doc 重写 | `contract_path` / `schema_path` / `index_path` / `entry_state` / `ref_relative` / `generated_path` 逐字不变；资产① 落点恒为派生 `<FQN>.md` |
| `crates/core/archi/src/api/diff.rs` | `api_dir_artifacts` 收面只留 `<archi_graph_dir>/INDEX.toml`；`*.schema.json` 退出孤儿枚举；模块头 / 函数 doc 随动 | 与 `render_all` 让位同一提交落地（R1）；不去掉即 11 份手写文件全进 `orphaned`（症状「全红」） |
| `crates/core/archi/src/api/check.rs` | ① `none` 支 2 步（`apiUnexpected` → `docMissing`）、`declared` 支 4 步（`docMissing` → `apiEntryMissing` → `apiMissing` → 一致性）；② `doc_present` 取代 `doc_facts`；③ 删 `reexport_drift` / `doc_not_found` / `api_doc_missing` / `doc_missing_none` / `doc_missing_declared`（并为一条 `doc_missing_message`）；④ `category_of` 七组；⑤ `is_pure_ref_alias` → `pub(crate)`；⑥ 模块头 doc 两支链重写 | 三值删除是事实源消失（不是放宽）；`apiUndeclared` 保留 `off`；pair 锚点三条逐字不变；「接口未写」零判据（⑤⑥ 无声明集） |
| `crates/core/archi/src/edit/warnings.rs` | 条件表 7 → 5：删三条 `doc` 事实；`docMissing` = `created` ∧ 派生 `<FQN>.md` 不在盘（与 `api` 无关）；`apiUnexpected` = `none` ∧ schema 在盘（写 / 删两处）；`path_on_disk` 改走 `checked_root`；`contract_on_disk` → `schema_on_disk`；`api_induced` 去掉 `doc`；模块头 doc 与降噪不变式重写 | 提案的 `crates/core/archi/src/edit/**` 在实现上只落本文件（`edit/mod.rs` 的两个调用点签名不变，无需改动）。`kind` / `severity` / 路径仍全取既有单点；零 `drift::check`、零第二套模型；`doc` 的任何取值都不产 warning（`DocNotFound` 零出现） |
| `crates/core/archi/src/model.rs` | `Element.api` 语义注释改为「接口承诺开关」；`Element.doc` 注释改为「约定：只有 `api = none` 会写；零判据读者」 | 字段集与八字段顺序零改动；零机械判据新增 |
| `crates/core/archi/src/drift/report.rs` | `ArchiApiFindingKind` 删 `ApiReexportDrift` / `DocNotFound` / `ApiDocMissing`；`ALL: [Self; 10]`；`as_str` 十词；`severity` 的 `error` 恰两项；相关 doc（「十三」「四组」表述）随动 | `ArchiApiFinding` / `ApiFindingSeverity` / `ArchiCheckStatus` / `REPORT_VERSION` 零改动 |
| `crates/core/archi/src/dto.rs` | ① `ArchiApiRef` JSDoc 同步（`status` = 接口文件在案；`contractPath` = 文档路径；`schemaPath` = 接口路径；字段名 / 顺序 / 类型不变）；② `ArchiModuleDetail` 增 `assets: ArchiAssetSet` + `reading: String`（声明序在 `file_hash` 之后）；③ 新增 `ArchiAssetSet` / `ArchiTopologyAsset` / `ArchiInterfaceAsset` / `ArchiDocAsset`；④ `module_detail` 装配三资产，`doc` 改为原样投影（去掉 `api = none` 过滤，D-23）；⑤ `ArchiSearchResult.doc` 注释改为「资产路径（文档 / 接口）」 | 三资产各自 `present`（不搞全或全无）；`interface` 只给键名摘要，`doc` 只给段名 / 行数 / 字节；`reading` 为静态常量（零变量插值）；坏 schema 降级读法见 D-9 |
| `crates/core/archi/src/search.rs` | 语料两规则：`path` 非空元素的在盘 `<FQN>.md` ∪ 在盘 `<FQN>.schema.json`；删 `carrier_path` 调用；`SearchOutcome.doc` 注释改写 | 与 `api` 状态、`doc` 字段无关；`MUST NOT` 复活的第二次载体派生（D-21 机械连带） |
| `crates/core/archi/src/lib.rs` | 出口面追加 `BootstrapDrafts`、`validate_schema`、`schema_fqn`、`is_fqn`、`is_identifier`；既有逐名 `pub use` 行随动 | 单点出口纪律（D60）不变；`api::` 模块路径仍可达 |
| `crates/infra/agent/src/sdk/tools.rs` | `execute` 把 `root` 传给 `write` / `edit`；两函数增 `root: &Path`；落盘前命中接口文件即 `validate_schema`（`Err` 零落盘、不建父目录），成功追加「接口已校验」；`edit` 校验替换后正文；`input_path` 的 doc 随动 | 射程只 `<archi_models_dir>/**/<FQN>.schema.json`；不属保护面、不改档位；`execute` 对外签名零改动 |
| `crates/infra/agent/src/sdk/archi_tool.rs` | `api_payload` 三动作：`check` 不变、`bootstrap` 回 `{ schema, doc }`、新增 `link`（两形态互斥 + FQN / 标识符校验）；`SCAFFOLD_TEMPLATE` 首行 / 描述句按接口面更新（接口草稿指 `archi_api` 的 `bootstrap`，词典正文不动）；模块头 doc 与 `api_payload` doc 重写 | `archi_module` / `archi_search` / 写工具执行体零改动；`link` 纯计算零读盘写盘；输出仍过 `cap_result` |
| `crates/infra/agent/src/sdk/mount.rs` | `archi_module_definition()` 五句（三资产一次给全 / 接口唯一凭据 / 文档整读 / `resolvedEntry` + `fileHash` / 两路径语义）；`archi_api_definition()` 四句 + `action` enum 加 `link` + `from` / `to` / `name` 参数；`archi_scaffold_definition()` 一句（接口草稿指路） | 工具名常量 / 参数 schema（除 `archi_api`）/ `ToolTier::Read` / `ToolExec::Archi` / `build_registry` 项数零变化 |
| `crates/infra/agent/src/compose.rs` | `archi_matrix` 的 `(proposal, executor)` / `(dev-design, executor)` 两行补 `ARCHI_API_TOOL` 并按下表序排列；change 臂 `preamble = model_write_lock(provenance).is_none().then(knowledge).flatten()`；模块头 doc 的臂描述随动 | 单一判据、与两写工具下发同集；`model_write_lock` 零改动；archi 臂 / config 臂逐字不变 |
| `crates/infra/agent/src/archi-knowledge.md` | 五段重写：三资产分工 / 设计顺序 / 三条不可越界 / 工具面（读五写二 + 接口三动作 + 手改出口 + 自检）/ 留给判断 + 兼容（草案见 D-17） | 骨架 ≤ 1800 B、全文 ≤ 2000 B；零围栏 / 零值域行 / 零诊断词 / 零 `<>` 占位符 |
| `crates/infra/agent/src/archi_knowledge.rs` | 模块头 doc 的骨架描述改为五段；派生段改为「模型目录 + 契约资产（`<FQN>.md` 理由 + `<FQN>.schema.json` 接口，同目录）」 | `build` / `knowledge` 的占位符 / 空骨架校验与降级语义逐字不变 |
| `crates/core/workflow/src/prompts/proposal-planner.md` | 工具行补 `archi_api`（`bootstrap` 取双草稿 / `link` 算 `$ref`）；新增设计顺序句（先拓扑、后接触面、最后理由）与接口面纪律句（留白写空 schema `$defs {}` + `properties {}`，MUST NOT 编造导出） | 零路径模板、零字段形态长表；「只为已确定元素落笔」与 `archi_module` 自查句保留 |
| `crates/core/workflow/src/prompts/dev-design-planner.md` | 同上（工具行由「读四件」改「读五件」+ `archi_api`）；既有 `archi_drift` / `archi_scaffold` 行逐字保留 | 同上 |
| `crates/core/workflow/src/prompts/implementation-generator.md` | 单加一句「改了导出面（新增 / 删除 / 改签名）就同批同步接口定义文件，改完用 `archi_api` 的 `check` 自查」 | 工具行三名（`archi_module` / `archi_api` / `archi_drift`）逐字不变 |
| `crates/tools/archi-api-gen/src/main.rs` | `generate` 文档 / usage 改三类产物；`bootstrap` 打印单行 JSON `{ "schema": "…", "doc": "…" }`（P7）；`Command` doc 随动 | 子命令面与退出码语义不变；`--check` 零写入不变 |
| `AGENT.md` + `CLAUDE.md` | 「模块 API 契约」节由 6 条改写为**恰 5 条**（三资产分工 / 三类生成物 / 两脚本段序 / 留白 = 空 schema / 纳管口径），逐条文本见 D-19；两份节内正文逐字相同 | 零 `api-*` fence 示例、零 `openspec/specs/` 路径、零判据长表；`.schema.json` 不再称生成物 |
| `packages/desktop/package.json` | `version` 相对落地时 `HEAD` 现值 +1 patch（当前 `0.7.16` ⇒ 目标 `0.7.17`，以落地时 `HEAD` 为准） | `src-tauri/tauri.conf.json` 经 `"version": "../package.json"` 跟随；`src-tauri/Cargo.toml` 的 crate 版本行 MUST NOT 随动 |
| `.dev-team/architecture/graph/INDEX.toml` | `api:export` 重生成：说明段第二行改一句话（review 口径 = 读 schema 与 md、`declared` = 接口文件在案、本文件与 `.i.*` 的 diff 视为机械噪声）；值域与条目字典序不变 | 除该行外逐字节相同（派生出线由 C1 的 `build_index` 生成，MUST NOT 手改） |
| `packages/desktop/src/views/architecture/components/dashboard/contract-panel.tsx` | 冷启动文案「契约面未启用」→「接口面未启用」（与 `enabled` 新语义一致） | 功能代码零改动（chip 由 `apiFindings` 聚合、五态 / 聚焦 / 打开通路逐字不变） |
| `packages/desktop/src/views/architecture/components/model-info.tsx` | `ContractSection` 的 JSDoc 由「`api.status` + 三路径行 + 本元素判据」改为「本元素判据」（三路径面归右栏「文档」分段） | 渲染结构零改动 |
| `packages/desktop/src/views/architecture/components/dashboard/architecture-node.tsx` | 右栏「文档」分段三条路径 push 处补一行注释：三路径各自独立在盘事实（`contractPath` = 文档、`schemaPath` = 接口、`generatedPath` = 生成物） | 组件行为零改动（只渲非空项已天然正确） |

### 条件性修改（随待决问题裁决）

> 本表四行是「待决问题」Q-1 / Q-6 / Q-7 / Q-8 的**修正落点**：获裁决即按「具体修改」一侧执行，未获裁决即零动作。四行全为文本 / 注释面——零行为条款、零判据变化、零生成物字节影响；前三行属**本 change 自己的** `specs/` delta（本目录下的 `specs/**`），`openspec/specs/**` 基线仍零改动（见「零改动面」）。

| 路径 | 具体修改 | 说明 |
|---|---|---|
| `openspec/changes/archi-agent-context-review/specs/desktop-archi-agent/spec.md` | `link` 场景（「link 两形态各出正确串」）的示例串改一行：`"../../Core/Foundation/Backend.Core.Foundation.schema.json#/$defs/Summary"` → `"../Core/Backend.Core.Foundation.schema.json#/$defs/Summary"`（= `ref_relative("Backend.Infra.ArchiRuntime", "Backend.Core.Foundation") + "Backend.Core.Foundation.schema.json#/$defs/Summary"` 的实算值）；「前缀与 `paths::ref_relative` 逐字一致」一句保留 | Q-1 / D-15：示例串与函数单点不符，验收锚是 `ref_relative` 输出而非示例字面；行为条款与其余 spec 文字零改动 |
| `openspec/changes/archi-agent-context-review/specs/desktop-archi-model/spec.md` | 两处表述改为「呈现源是工具面（`archi_scaffold` 词典与参数 `description`）；知识文本只给纪律句、不给词表」：① 「`ALL` 全量常量为唯一派生源」条目里的「呈现源是 `archi-knowledge.md`——`kind` 由「元素类型」语义 bullet 承载，`gate` / `api` 由「字段纪律」各一行承载」；② 场景「知识文本只承载字段值域与可执行词」的 THEN 句 | Q-6：与 `desktop-archi-agent` 的「语法知识零残留」冲突（基线债务）；**词汇集对拍本体零改动**（四组 `ALL` 与 `as_str` 逐值对拍、`kind` / `gate` / `api` 的呈现责任边界仍由该能力承载） |
| `openspec/changes/archi-agent-context-review/specs/desktop-archi-api/spec.md` | Module Contract 行删去 `ContractType` token（`Contract` / `ContractOp` 与 `types` 字段逐字保留） | Q-7：仓内不存在 `ContractType`（`Contract.types` 是 `Map<String, Value>` 字段）；按现状实现，token 视为笔误 |
| `crates/core/archi/src/drift/mod.rs` | 两处注释的「入口文档承诺已成立」→「接口承诺已成立（`declared` ∧ schema 在盘）」（模块头 doc 与 `derive_status` 处，含 361 行邻近的装配注释）；其余逐字不动 | Q-8：该文件在提案「不要修改」表内（扫描 / 归属 / 定级 / 豁免 / 配置面零改），故注释追改**需评审放行**；否则归清理轮，本变更零动作 |

### 零改动面（核对结论）

| 对象 | 理由 |
|---|---|
| `.dev-team/architecture/graph/model/*.toml`（55 元素 / 129 关系） | 零模型字节改动：本变更不写模型，`modelHash` 逐字不变（落地前实测 `sha256:adcaabca…066851c7af`，交付后须逐字相同） |
| `.dev-team/architecture/models/**` 的 11 份 `.md` 与 11 份 `.schema.json` 正文（含 `$comment`） | 零迁移（内容与 fence 逐项等价，实测 11 对资产全部满足本设计的九道闸）；`$comment` 旧文案归清理轮（P10 / R5） |
| `crates/core/archi/src/api/emit/**` 与 `extract/**` | 渲染面与取数面逐字不变 ⇒ `.i.rs` / `.i.ts` 逐字节不变的机械前提 |
| `crates/infra/archi/src/api_artifacts.rs` | 写盘单点零改动（只是应生成集少一类） |
| `crates/core/archi/src/api/render.rs` | `workspace_render` 装配不变（应生成集内部由 `render_all` 收窄） |
| `packages/desktop/src-tauri/src/commands/archi/mod.rs` | 命令签名、薄包装、`*_inner` 测试缝、错误映射零改动（`ArchiModuleDetail` 新字段经 DTO 自动出线；`enabled` 语义变化在 core） |
| `packages/desktop/src-tauri/crates/core/archi/src/api/paths.rs` 的 `contract_path` / `schema_path` / `index_path` / `entry_state` / `ref_relative` / `generated_path` | 逐字不变（`ref_relative` 是 `link` 与 `verify_ref_prefix` 的共同单点） |
| 写入保护面（`sdk/protect/**`、`policy.rs`、`SessionProtectRule`） | 写闸是「坏文本不可产出」，不属保护面；相位锁 / 内置锁 / shell 写面提取逐字不变 |
| `crates/infra/agent/src/sdk/archi_drift_view.rs` | `byKind` 由报告动态聚合（词汇收缩自动生效） |
| `packages/desktop/src-tauri/crates/core/archi/src/api/check.rs` 的 pair 锚点三条与 `undeclared` 实现 | 形状与内部序逐字不变；`apiUndeclared` 仍 `off` |
| `crates/core/archi/src/drift/{mod,config}.rs` | 提案「不要修改」明列：扫描 / 归属 / 定级 / 豁免 / 配置面零改；`mod.rs` 两处纳管注释的旧措辞（「入口文档承诺已成立」）归清理轮（Q-8） |
| `openspec/specs/**` 基线、`openspec/changes/**` 的既成 delta | delta-only：本变更只写自己 `specs/` 的增量；基线由归档合并维护 |

### 公共函数 / API

| 标识符 | 所在文件 | 新增 / 修改 | 签名 | 说明 |
|---|---|---|---|---|
| `validate_schema` | `core/archi/src/api/schema.rs` | 新增 | `pub fn validate_schema(fqn: &str, text: &str) -> Result<Contract, ApiError>` | 唯一公开入口：九道闸一趟走完并产出 `Contract`（`Contract.file` = `<FQN>.schema.json`，装载期覆写为 repo 相对路径）；写闸与装载期共用（只差错误映射） |
| `verify_identity` | 同上 | 新增 | `pub fn verify_identity(fqn: &str, value: &Value) -> Result<(), ApiError>` | `$id` == `<FQN>.schema.json`、`title` == FQN、文件主干 == FQN |
| `verify_keyspace` | 同上 | 新增 | `pub fn verify_keyspace(value: &Value) -> Result<(), ApiError>` | op 键集（`properties` 里带 `x-function` 者）∩ `$defs` 键集 = ∅（D-3） |
| `verify_shape` | 同上 | 新增 | `pub fn verify_shape(value: &Value) -> Result<(), ApiError>` | `properties.<k>` 恰二选一：`{ "x-function": {…} }`（值须对象）或 `{ "$ref": "#/$defs/<名>" }` 且 `<名> ∈ $defs` |
| `verify_mirror` | 同上 | 新增 | `pub fn verify_mirror(value: &Value) -> Result<(), ApiError>` | `$defs` 每键在 `properties` 有指向自己的 `$ref` |
| `verify_ref_prefix` | 同上 | 新增 | `pub fn verify_ref_prefix(fqn: &str, value: &Value) -> Result<(), ApiError>` | 跨元素 `$ref` 前缀逐字等于 `ref_relative(from, to)`；文案并列当前串与期望串 |
| `verify_top_level_keys` | 同上 | 新增 | `pub fn verify_top_level_keys(value: &Value) -> Result<(), ApiError>` | 顶层键封闭六项；`$id` / `title` / `$defs` / `properties` 必在场 |
| `build_schema` | 同上 | 删除 | — | 真相方向倒置后无产物 |
| `Contract::from_schema` | `core/archi/src/api/contract.rs` | 新增 | `pub fn from_schema(fqn: &str, file: &str, schema: &Value) -> Contract` | 无失败面（在 ③–⑧ 闸通过后调用）：解包 `x-function` 至 `signature`（丢 `description`）、`line = 0`、`types` = 非纯 alias 的 `$defs`、`reexports` = 可反查目标 FQN 的纯 alias |
| `parse` | 同上 | 删除 | — | md 解析器整体退役（含全部私有解析件） |
| `verify` | 同上 | 修改 | `pub fn verify(contract: &Contract, schema: &Value) -> Result<(), ApiError>` | 同签名，输入改为 schema 投影；保留七族名字 / `x-function` 落点 / `required ⊆ properties` 三闸 |
| `is_constructor` | 同上 | 不变 | `pub fn is_constructor(op_id: &str) -> bool` | 三条消费面共用（`check` / `bootstrap` / `emit`） |
| `load_contracts` | `core/archi/src/api/mod.rs` | 修改 | `pub fn load_contracts(root: &Path, elements: &[ArchiElement]) -> Result<ApiSnapshot, ApiError>` | 同签名；只读 `<FQN>.schema.json`，经 `validate_schema` 装配；不在盘跳过、坏文件 `ApiError` 停摆；通过后把 `Contract.file` 覆写为 repo 相对路径（`display`）——`validate_schema` 无路径入参，其 `Contract.file` 只能是 `<FQN>.schema.json`，报告的 `occurrences.file` 仍须 repo 相对（spec 定） |
| `enabled` | 同上 | 修改 | `pub fn enabled(root: &Path, elements: &[ArchiElement]) -> bool` | 同签名；任一 `path` 非空 ∧ `api = declared` 元素 schema 在盘 |
| `gating_findings` | 同上 | 修改 | `pub fn gating_findings(root: &Path, elements: &[ArchiElement], findings: &[ArchiApiFinding]) -> Vec<ArchiApiFinding>` | 同签名；入选 = `element` 非空 ∧ `api = declared` ∧ schema 在盘 |
| `render_all` | 同上 | 修改 | `pub fn render_all(root: &Path, snapshot: &ApiSnapshot) -> Result<Vec<GeneratedFile>, ApiError>` | 同签名；只产三类生成物（不再写 `.schema.json`） |
| `bootstrap` | 同上 | 修改 | `pub fn bootstrap(root: &Path, element: &str) -> Result<BootstrapDrafts, ApiError>` | 返回 `{ schema, doc }` 双草稿；失败面（元素不存在 / 无可用入口 / 符号抽取失败）逐字不变 |
| `api_ref` / `api_ref_of` | 同上 | 不变 | `pub fn api_ref(root: &Path, fqn: &str) -> ArchiApiRef` / `pub fn api_ref_of(root: &Path, elements: &[ArchiElement], element: &ArchiElement) -> ArchiApiRef` | 同签名；投影语义换锚（三路径各自独立 + `status` 判 schema） |
| `carrier_path` / `doc_path` | `core/archi/src/api/paths.rs` | 删除 | — | 资产① 落点恒为派生 `<FQN>.md`；`doc` 退出派生面 |
| `schema_fqn` | 同上 | 新增 | `pub fn schema_fqn(file_name: &str) -> Option<&str>` | 文件名 → FQN 的唯一反查（`<FQN>.schema.json` 去后缀）；写闸与 `reexport_target` 同调 |
| `is_fqn` | 同上 | 修改（可见性） | `pub fn is_fqn(fqn: &str) -> bool` | 私有 → 公开；`link` 的 FQN 形态判据复用（零第二份） |
| `checked_root` | 同上 | 修改（可见性） | `pub(crate) fn checked_root(raw: &str) -> Option<String>` | 写面 `path_on_disk` 复用「归一 + 越根」判据 |
| `category_of` | `core/archi/src/api/check.rs` | 修改 | `pub fn category_of(kind: ArchiApiFindingKind) -> &'static str` | 同签名；`doc` 只剩 `docMissing`，七组不变 |
| `is_pure_ref_alias` | 同上 | 修改（可见性） | `pub(crate) fn is_pure_ref_alias(definition: &Value) -> bool` | 私有 → `pub(crate)`；`declared_types` 与 `dto` 的 `interface.types/reexports` 分派共用 |
| `module_detail` | `core/archi/src/dto.rs` | 修改 | `pub fn module_detail(root: &Path, fqn: &str) -> Result<ArchiModuleDetail, String>` | 同签名；出参增 `assets` / `reading`，`doc` 原样投影 |
| `search` | 同上 | 修改 | `pub fn search(root: &Path, query: &str, scope: SearchScope) -> Result<ArchiSearchResult, String>` | 同签名；语料换两条固定规则（实现落 `search.rs`） |
| `tools::execute` | `infra/agent/src/sdk/tools.rs` | 不变 | `pub async fn execute(root: &Path, name: &str, input: &Value) -> Result<String, String>` | 对外签名零改动；内部把 `root` 传给 `write` / `edit` |

> `archi_api` / `archi_module` / `write` / `edit` 是 **agent 工具**，不是 IPC 命令：执行体即其全部执行面，命令面仍九条（见「路由 / API 设计」）。

**内部接线面**（非公共 API；仅登记落位与签名，防实现分叉）：

| 标识符 | 所在文件 | 新增 / 修改 | 签名 | 说明 |
|---|---|---|---|---|
| `ApiSnapshot.doc_present` | `core/archi/src/api/mod.rs` | 修改 | `BTreeMap<String, bool>` | 派生 `<FQN>.md` 是否存在（原 `doc_facts` 三态收成单布尔） |
| `ApiCheckInput.doc_present` | 同上 | 修改 | `&'a BTreeMap<String, bool>` | 判据面唯一读取面（零读盘）；spec 的「`doc_present: bool`」是逐元素说明，`ApiCheckInput` 是每轮结构，形态只能是「FQN → bool」映射 |
| `doc_presence` | 同上 | 新增（替 `doc_facts`） | `fn doc_presence(root: &Path, elements: &[ArchiElement]) -> BTreeMap<String, bool>` | `contract_path(fqn).is_some_and(is_file)`，逐元素一次 |
| `BootstrapDrafts` 的两个产出核 | 同上 | 新增 | `fn schema_draft(fqn: &str, symbols: &[ApiSymbol]) -> String` / `fn doc_draft(fqn: &str) -> String` | 双草稿唯一装配点；两出口（CLI / 工具）共用 |
| `duplicate_keys` | `core/archi/src/api/contract.rs` | 修改（可见性） | `pub(crate) fn duplicate_keys(text: &str) -> Option<(String, usize)>` | 文本层重复键扫描（`serde_json` 会折叠重复键） |
| `is_identifier` | 同上 | 修改（可见性） | `pub fn is_identifier(token: &str) -> bool` | 七族名字闸的唯一判据；`link` 的 `name` 校验复用 |
| `interface_asset` | `core/archi/src/dto.rs` | 新增（文件内私有） | `fn interface_asset(root: &Path, fqn: &str) -> ArchiInterfaceAsset` | 读 schema 键名与 `abstract` 判读；坏 JSON 降级见 D-9 |
| `doc_asset` | 同上 | 新增（文件内私有） | `fn doc_asset(root: &Path, fqn: &str) -> ArchiDocAsset` | 只读 `##` 段名 + 字节 + 行数（零正文） |
| `topology_asset` | 同上 | 新增（文件内私有） | `fn topology_asset(element: &Element, neighborhood: &Neighborhood<'_>) -> ArchiTopologyAsset` | 零额外读盘（`file` / `line` / 出入度数） |
| `schema_target_fqn` | `infra/agent/src/sdk/tools.rs` | 新增（文件内私有） | `fn schema_target_fqn(root: &Path, path: &str) -> Option<String>` | 路径命中 `<root>/<models rel>/**` 且文件名为 `<FQN>.schema.json` ⇒ `Some(FQN)` |
| `write` / `edit` | 同上 | 修改 | `async fn write(root: &Path, input: &Value) -> Result<String, String>` / `async fn edit(root: &Path, input: &Value) -> Result<String, String>` | 落盘前过写闸（`write` 先校验后建目录；`edit` 校验候选正文） |
| `link_payload` | `infra/agent/src/sdk/archi_tool.rs` | 新增（文件内私有） | `fn link_payload(&self, input: &Value) -> Result<Value, String>` | `{ from, to, name }` / `{ element, name }` 两形态互斥 + 校验 |

### CLI: `crates/tools/archi-api-gen/src/main.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `generate` | 产物集少一类 | 写 `INDEX.toml` / `<入口主干>.i.rs` / `<入口主干>.i.ts`；MUST NOT 写 `.schema.json`；幂等语义不变 |
| `--check` | 随动（收面） | 生成到内存后读盘比对、零写入；差异即非零退出并列清单；`.schema.json` 永不出现在三列 |
| `bootstrap <元素>` | 出口形状改 | 打印**单行 JSON** `{ "schema": "…", "doc": "…" }`（与 agent 工具同形、同源、零写盘） |
| `Usage` | 文案随动 | 三类产物与 `bootstrap` 双草稿的描述 |

### API: agent 工具面（响应形状）

| 工具 | 变化 | 描述 |
|---|---|---|
| `archi_module` | 加字段 | 单元素响应增 `assets`（`topology` / `interface` / `doc` 三段各自 `present`）与 `reading`；既有字段位置与形状不变；薄概览零变化 |
| `archi_search` | 语料换面 | 两条固定规则（在盘 `<FQN>.md` ∪ 在盘 `<FQN>.schema.json`），与 `api` 状态 / `doc` 字段无关 |
| `archi_api` | 动作扩面 | `action` 三值（`check` / `bootstrap` / `link`）；`bootstrap` 回 `{ schema, doc }`；`link` 回 `{ "ref": "<串>" }` |
| `write` / `edit` | 条件回执 + 新 `Err` 类 | 目标是接口文件时成功后追加「接口已校验」；失败 `Err` 第一行自足（文件 + 期望值 + 出路）；其余路径响应逐字不变 |

### 类型定义

| 类型名 | 所在文件 | 新增 / 修改 | 说明 |
|---|---|---|---|
| `BootstrapDrafts` | `core/archi/src/api/mod.rs` | 新增 | `struct { schema: String, doc: String }`；`bootstrap` 的唯一返回形（两出口共用） |
| `ArchiAssetSet` | `core/archi/src/dto.rs` | 新增 | `struct { topology: ArchiTopologyAsset, interface: ArchiInterfaceAsset, doc: ArchiDocAsset }`，`specta::Type` + `serde(rename_all = "camelCase")` |
| `ArchiTopologyAsset` | 同上 | 新增 | `struct { file: String, line: u32, out: usize, in_degree: usize }`——`in_degree` 经 `#[serde(rename = "in")]` 出线为 `in`（`in` 是 Rust 关键字；见 D-7 / Q-3） |
| `ArchiInterfaceAsset` | 同上 | 新增 | `struct { path: Option<String>, present: bool, bytes: u64, ops: Vec<String>, types: Vec<String>, reexports: Vec<String>, abstract_note: String }`——`abstract_note` 经 `#[serde(rename = "abstract")]` 出线（`abstract` 是 Rust 保留字） |
| `ArchiDocAsset` | 同上 | 新增 | `struct { path: Option<String>, present: bool, bytes: u64, lines: u32, headings: Vec<String> }` |
| `ArchiModuleDetail` | 同上 | 修改 | 追加 `assets: ArchiAssetSet` + `reading: String`（声明序在 `file_hash` 之后）；既有八字段名字 / 类型 / 顺序不变 |
| `ArchiApiRef` | 同上 | 修改（仅 doc） | 四字段零改名零改型；`status` 语义 = 接口文件在案；`contractPath` = 文档路径 |
| `ArchiApiFindingKind` | `core/archi/src/drift/report.rs` | 修改 | 十三 → 十值（删 `ApiReexportDrift` / `DocNotFound` / `ApiDocMissing`）；`ALL: [Self; 10]` |
| `ArchiApiFinding` / `ApiFindingSeverity` | 同上 | 不变 | 形状与档位词逐字不变 |
| `DocFact` | `core/archi/src/api/mod.rs` | 删除 | 三态载体事实收成 `doc_present: bool` |
| `ContractSection` | `core/archi/src/api/contract.rs` | 删除 | 散文段概念随 md 解析器退役 |
| `Contract` | 同上 | 修改 | 删 `reexport_prose` / `sections`；`types` / `reexports` 语义改为 schema 投影（字段名不变）；`fqn` / `title` / `file` / `ops` 保留 |
| `ContractOp` | 同上 | 不变 | `{ id, line, signature }`；`line` 恒 `0`（schema 无行号，位置未知的既有口径） |
| `ApiSnapshot` | `core/archi/src/api/mod.rs` | 修改 | `doc_facts` → `doc_present`；其余字段不变 |
| `SearchOutcome` / `ArchiSearchResult` | `core/archi/src/search.rs` / `dto.rs` | 修改（仅 doc） | 字段名 / 类型不变（`doc: Vec<String>` 仍只承载路径串） |

<!-- 配置：本变更不新增 / 不修改任何配置键（无 `config.json` 键、无环境变量、无构建特性）。子表整段省略。 -->

## 数据模型 (Data Model)

### 磁盘资产（形态变化为零，语义换锚）

| 资产 | 落点 | 本变更后的角色 | 变化 |
|---|---|---|---|
| 文档 | `<archi_models_dir>/<分层目录…>/<FQN>.md` | 纯散文（人 + AI 共读，产品面） | 正文零改动；读面零解析（历史 `api-*` fence 视为普通内容） |
| 接口 | 同目录 `<FQN>.schema.json` | **手写真相**（唯一凭据） | 正文零改动（与 fence 逐项等价）；不再是生成物；写前过 `validate_schema` |
| 索引 | `<archi_graph_dir>/INDEX.toml` | 生成物（`declared` 判定改判 schema 在盘） | 头注释一句改；条目字典序 / 值域不变 |
| 生成物 | 入口同级 `<入口主干>.i.rs` / `.i.ts` | 生成物 | 逐字节不变 |
| 模型 | `<archi_graph_model_dir>/NN-<域>.toml` | 拓扑真相 | 零字节改动（55 元素 / 129 关系 / `modelHash` 不变） |

### 进程内结构

```text
ApiSnapshot { contracts: Vec<Contract>, schemas: BTreeMap<String, Value>,
              symbols: BTreeMap<String, Vec<ApiSymbol>>,
              doc_present: BTreeMap<String, bool> }        // 原 doc_facts

BootstrapDrafts { schema: String, doc: String }            // 新

Contract { fqn, title, file, ops: Vec<ContractOp>,
           types: Map<String, Value>,                      // 非纯 $ref alias 的 $defs
           reexports: Map<String, Value> }                 // 纯 $ref alias（值 = 目标 FQN）
```

- **`Contract.reexports` 的值是目标 FQN**（由 `$ref` 文件主干经 `schema_fqn` 反查）：`module_refs`（透传渲染）与 `verify`（名字闸）消费它；`$ref` 无文件部分的本地 alias 不是透传，不入 `reexports`（其名字由 `$defs` 键名闸覆盖）。`dto` 的 `interface.reexports` 则按「纯 alias」全量列出（它报的是键名事实，不做跨元素判断）。
- **`doc_present`** 只回答「派生 `<FQN>.md` 在不在盘」，与 `api` 状态、`doc` 字段无关。

### 线面响应（agent 工具 / IPC）

```jsonc
// archi_module（单元素；新增字段标 ★）
{ "element": { … }, "dependsOn": [ … ], "dependedBy": [ … ], "children": [ … ],
  "doc": null, "api": { "status": "declared", "contractPath": "…/<FQN>.md",
                        "schemaPath": "…/<FQN>.schema.json", "generatedPath": "…/lib.i.rs" },
  "resolvedEntry": { … }, "fileHash": "sha256:…",
  "assets": {                                            // ★
    "topology":  { "file": "…/06-frontend.toml", "line": 105, "out": 0, "in": 3 },
    "interface": { "path": "…/<FQN>.schema.json", "present": true, "bytes": 276,
                   "ops": [], "types": [], "reexports": [],
                   "abstract": "接口面留白（schema 在盘且无 op / 无类型）" },
    "doc":       { "path": "…/<FQN>.md", "present": true, "bytes": 3680, "lines": 62,
                   "headings": ["职能", "公共面", "不变量"] } },
  "reading": "理解本模块已足够：…" }                      // ★

// archi_api（三动作）
{ "action": "bootstrap", … } → { "schema": "<JSON 骨架>", "doc": "# <FQN>\n\n…" }
{ "ref": "../Core/Backend.Core.Foundation.schema.json#/$defs/Summary" }   // ref_relative 实算（Q-1）
```

- `interface` 零 content、`doc` 零正文（只段名 / 行数 / 字节）：反查与整读仍走通用 `read`。
- 薄概览（省略 `element`）MUST NOT 携带 `assets` / `reading`（一元素多份读盘，不给全表付账）。

### 持久化

- 无新增持久化：无新表、无新库字段、无新事件类型；模型 / 配置 / 报告落盘面零改动；`write` / `edit` 的写盘路径与原子性语义不变。

## 路由 / API 设计 (Route / API Design)

> `auth`：不适用——本机进程内 agent 工具与 Tauri IPC 命令均无鉴权面；准入由 `policy::allows`（档位）+ 成员校验两级表达，写面另加 `ProtectRules`。

### IPC 命令面（零新增，形状一处扩面）

| 命令 | 入参 | 出参 | 变更 |
|---|---|---|---|
| `archi_module` | `root: string`, `element: string` | `ArchiModuleDetail` | 出参增 `assets` / `reading`（命令体零改动） |
| `archi_api_check` | `root: string` | `ArchiApiCheck` | `enabled` 语义换锚；三列内容少一类（`.schema.json` 零出现） |
| `archi_api_generate` | `root: string` | `ArchiApiWriteOutcome` | 零改动（应生成集内部收窄） |
| 其余六条 | 不变 | 不变 | 零改动（命令清单仍九条、写命令仍两条） |

### agent 工具面

| 工具 | 入参（平铺） | 出参 | 错误面 |
|---|---|---|---|
| `archi_module` | `element`（可选 FQN） | 单元素：既有字段 + `assets` + `reading`；薄概览：`fqn` / `kind` / `gate` / `summary` / `api.status` / `hasPath` | 模型结构错 ⇒ 结构化诊断 `Err`（带 `file:line`），不 panic |
| `archi_search` | `query`（必填）、`scope`（`elements` / `doc` / `all`，缺省 `all`） | `{ elements, doc }`（`doc` = 两条规则命中的资产路径） | 空 `query` / 词汇外 `scope` ⇒ `Err` 并列合法值 |
| `archi_api` | `action`（`check` / `bootstrap` / `link`）、`element`、`from`、`to`、`name` | `check`：两轴结构；`bootstrap`：`{ schema, doc }`；`link`：`{ ref }` | 词汇外动作 / 参数互斥 / 非标识符 `name` / 结构错 ⇒ `is_error`，零写盘 |
| `write` | `path` / `content` | 成功回执（接口文件追加「接口已校验」） | 接口校验失败 ⇒ `Err` 零落盘（第一行自足） |
| `edit` | `path` / `old_string` / `new_string` / `replace_all` | 同上 | 同族；校验对象是替换后正文 |

### 下发面（`(phase, role)` 矩阵，本变更两格）

| phase | role | `module` | `search` | `api` | `drift` | `scaffold` | `element` | `relation` |
|---|---|---|---|---|---|---|---|---|
| `proposal` | executor | ✅ | ✅ | **✅（新）** | ❌ | ✅ | ✅ | ✅ |
| `dev-design` | executor | ✅ | ✅ | **✅（新）** | ✅ | ✅ | ✅ | ✅ |
| `dev-design` | evaluator | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| `test-design` | executor | ❌ | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| `implement` | executor | ✅ | ❌ | ✅ | ✅ | ❌ | ❌ | ❌ |
| `code-review` / `acceptance` | evaluator | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| 其余 | 任意 | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |

- 两格下标量顺序（`compose::archi_matrix` 的向量序）= spec 所列：`proposal` = module / search / api / scaffold / element / relation；`dev-design` = module / search / api / drift / scaffold / element / relation（D-16）。
- `archi_api` 档位仍 `ToolTier::Read`；两写工具的下发集合与 `model_write_lock == None` 的集合逐格相等（扩面不扩写权）。

## 依赖 (Dependencies)

### 运行时依赖

| 依赖 | 用途 | 变化 |
|---|---|---|
| `serde_json`（`core/archi` / `infra/agent` 既有） | schema 文本 → `Value`、DTO 投影与工具响应 | 零新增（无 `preserve_order`，`Map` 键序为字典序，与既有生成物序一致） |
| `toml_edit`（`core/archi` 既有） | `INDEX.toml` 装配 | 零新增 |
| `foundation::layout`（既有） | `<archi_models_dir>` / `<archi_graph_dir>` / `<archi_graph_model_dir>` 派生 | 零新增（无新派生函数） |
| `specta` / `specta-zod`（既有） | 四新出线类型 + 两字段 + 枚举三值离场 | 零新增 |
| `sha2`（既有） | `fileHash` / `modelHash` | 零改动 |

### 构建 / 测试依赖

| 依赖 | 用途 | 变化 |
|---|---|---|
| `crates/tools/export-bindings`（既有） | `bindings.ts` / `bindings.zod.ts` 重导 | 零新增 |
| `tempfile` / `DualEnv` 等既有夹具（`infra/agent` dev-dependencies） | 写闸 / `link` / 三资产夹具的临时目录 | 零新增 |
| `crates/core/archi/tests`（既有 golden 面） | `tests/corpus/drift/**` 的 `contracts/**` 夹具补 schema 两态 + `DESKTOP_GOLDEN_REWRITE=1` 重写 | 零新增文件类别（`tests/corpus/api/` 仍零存在） |

## 决策留痕

| # | 决策 | 理由 / 边界 |
|---|---|---|
| D-1 | **`Contract::from_schema` 是投影，不是校验**：无 `Result`，只在 ③–⑧ 闸通过后调用；解包 `x-function` 存 `ContractOp.signature`、`description` 丢弃、`line = 0` | `emit/rust.rs` 直接读 `op.signature.get("returns")` / `.get("owner")`——保留包裹态会让 `.i.rs` 的签名见证静默退化（R2）。「schema 无行号」用既有 `ArchiDriftOccurrence::line = 0` 口径，MUST NOT 为凑行号新增只此一处消费的字段 |
| D-2 | **`validate_schema` 落 `api/schema.rs`**（P1）：九道闸 + `verify` + `verify_defs` 同趟走完并返回 `Contract`；`contract.rs` 只留 `Contract` 投影与可复用三闸 | 校验主体与「$defs 规则」同属 schema 面；`contract.rs` 退为投影类型（约 1095 → 400–500 行）。`verify_defs` 归入 ⑨ 的「复用」面、不另编号——它入参本就是 `&Value`，零改造 |
| D-3 | **闸 ④ 是「op 键集 ∩ `$defs` 键集 = ∅」**，不是「`properties` 键集与 `$defs` 键集无重名」 | ⑥ 镜像要求每个 `$defs` 键在 `properties` 有指向自己的 `$ref`（今 11 份资产实测全成立）⇒ 两键集**必须**相交；真正要拒的是「同名既是 op 又是类型」。spec 括注「op id 与类型名不撞」即本义，本设计按此实现（闸序 ④ 先于 ⑥，撞名文案点名两键空间） |
| D-4 | **形态白名单是「恰一键」**：`properties.<k>` 只允许 `{ "x-function": {…} }`（值须对象）或 `{ "$ref": "#/$defs/<名>" }`（`<名> ∈ $defs`） | `x-function` 内层自由形状（`description` / `async` / 将来的键不校验），故 `description` 必须内嵌而非兄弟键——兄弟键会让白名单多一维（explore §9.4） |
| D-5 | **`enabled` 是单点纯函数**（`declared` ∧ `path` 非空 ∧ schema 在盘），`artifact_check` 与 `findings` 同调它；不从 `ApiSnapshot` 反推 | 「命令层另判一套」是 spec 明禁；两次 `is_file` 的代价小于两套判据漂移的代价。`api = none` 元素在盘 schema 是 `apiUnexpected` 违约，不构成启用证据 |
| D-6 | **判据面两支两段**：`none` = `apiUnexpected` → `docMissing`；`declared` = `docMissing` → `apiEntryMissing` → `apiMissing` → 一致性（签名 / 构造器）。`docMissing` 单文案单成因（派生 `<FQN>.md` 不在盘）；「接口未写」（schema 不在盘）零判据 | 三值删除是事实源消失（R6），不是放宽；③ 短路时 ⑤⑥ 自然无对象；`docMissing` 文案不再出现「入口文档」指针措辞（唯一变化文本） |
| D-7 | **`ArchiTopologyAsset` 的 `in` / `ArchiInterfaceAsset` 的 `abstract` 用 `#[serde(rename = "...")]` 承载**（Rust 字段名 `in_degree` / `abstract_note`） | `in` 是 Rust 关键字、`abstract` 是保留字；specta 既有 `rename_all` 读 serde 属性 ⇒ 同一属性应可出线同名。验收锚 = `bindings/mod_test.rs` 的字段名断言（`out` / `in` / `abstract`），不成立即补 `#[specta(rename = "…")]`（Q-3） |
| D-8 | **三资产的 `present` 各自独立**；`interface` 只给键名摘要（`ops` = 带 `x-function` 的 `properties` 键；`types` / `reexports` = `$defs` 键按纯 alias 二分），`doc` 只给 `##` 段名 + 行数 + 字节 | 旧 `ref_projection` 的「三路径全或全无」把三份资产的存在事实压成一个开关；整读 schema / 内联正文会灌爆上下文并抢走 `read` 的翻页能力 |
| D-9 | **`archi_module` 遇坏 schema 不 `Err`**：`present = true`、`bytes` 照实、三数组为空、`abstract` = 「接口文件不可解析（用 `archi_api` 的 `check` 看诊断）」 | `archi_module` 是定位 / 导航面，坏文件不能让 agent 连拓扑都看不见（R3 的停摆代价只该落在报告面）；报告面（`findings` / `drift` / `check`）仍 `ApiError` 停摆，不产半真报告 |
| D-10 | **`reading` 是静态常量**（零变量插值），表达三事：理解本模块已足够（拓扑看 file/line 与出入边、接口看键名、文档看段名）；文档正文用 `read` 整读（无结构，MUST NOT 用 `grep` / `offset` 拼凑）；接口不必整读（键名即定义） | 这段话写进 prompt 会被 `AGENT.md` 体量淹没；写进工具返回值才在调用点生效（Q5） |
| D-11 | **`ArchiApiRef` 不改名**（P2）：四字段名 / 类型 / 顺序零变化，`status` 改判 schema 在盘，三路径各自独立出线 | 避免 breaking IPC 与 4 处前端夹具重写；`contractPath` 一直是 doc 路径，JSDoc 点明即可；改名留清理轮 |
| D-12 | **`GENERATED_NOTICE` 逐字保留**，另加 `SCHEMA_WRITER_NOTICE` 供 `bootstrap` 骨架的 `$comment` | `GENERATED_NOTICE` 经 `emit::notice_lines` 进 `.i.rs` / `.i.ts` 头注释——改它即改生成物字节，与验收 2「`.i.*` 逐字节不变」直接冲突。提案「`GENERATED_NOTICE` 语义改写」按验收 2 优先降级为「新增写者纪律注」；`.i.*` 头注释里「由契约 markdown 生成」的旧措辞与存量 `$comment` 同属清理轮（P10 / R5） |
| D-13 | **`render_all` 让位与 `diff::api_dir_artifacts` 收面同一提交落地**；`index_entries.present` 直判 schema 在盘（不依赖 `contract.is_some()`） | 只改一半的症状是「11 份手写文件全进 `orphaned`」（全红而非少一条，R1）。`api = none` 的越界 schema 行仍出 `declared` 但统计字段取 `snapshot`（不另读盘）——违约态下 INDEX 不是判据面，可接受 |
| D-14 | **`bootstrap` 的 schema 骨架可直接过写闸**：`$schema` / `$id` / `title` / `$comment`（写者纪律）/ `$defs` / `properties`；每个 op 一项 `{ "x-function": { "description": "", "params": […], "returns": … } }`；每个类型一项 `$defs.<名>` + `properties.<名> = { "$ref": "#/$defs/<名>" }`（镜像）；空符号集 ⇒ 两空对象；`$comment` = `SCHEMA_WRITER_NOTICE`（「本文件是手写接口真相（不是生成物）：导出名 / 签名 / 类型的唯一持有者；`x-function.description` 只写调用方要知道的语义、MUST NOT 复述签名；改完用 `archi_api` 的 `check` 自查」） | `description: ""` 与缺省语义等价，「（待补）」会被读者当正文渲染；骨架不含 `api-reexport` 占位（透传是设计判断）。doc 骨架 = `# <FQN>` + 一句「段落按内容自定，先例见同目录同类元素」（零段名、零 fence） |
| D-15 | **`link` 的串 = `ref_relative(from, to) + to + ".schema.json" + "#/$defs/" + name`**；本模块形态 = `"#/$defs/" + name`；两形态互斥；`from` / `to` 用 `paths::is_fqn`、`name` 用 `contract::is_identifier` | `ref_relative` 只回目录前缀（`"../Core/"`），文件主干必须由目标 FQN 拼——`build_schema` 退役前同式。**spec 场景示例串（`"../../Core/Foundation/..."`）与 `ref_relative` 实算不符**，以函数单点为验收锚（Q-1） |
| D-16 | **矩阵两格补 `archi_api` 并对齐 spec 行序**；change 臂 `preamble` 判据 = `model_write_lock(provenance).is_none()`（单一判据，与两写工具下发同集） | `bootstrap` 双草稿 + `link` 的价值恰在起草相位；`implement/executor` 有 archi 只读工具但不注入（知识含写模型纪律，与相位锁矛盾，P5 / R10） |
| D-17 | **知识骨架五段草案**（草案实测 1778 B ≤ 1800 B；全文 = 骨架 + 派生段 ≈ 1.95 KB ≤ 2000 B，实现按实测冻结，超限先删同段冗余、门限不抬） | 见附录 A。`archi_knowledge.rs` 派生段改为「模型目录 + 契约资产（`<FQN>.md` 理由 + `<FQN>.schema.json` 接口，同目录）」；骨架零目录字面量 |
| D-18 | **三份 prompt 的改写面**：两份写权相位各补三句（工具行含 `archi_api`；「先边界、后接触面、最后理由」；「留白写空 schema `$defs {}` + `properties {}`，MUST NOT 编造导出；改完导出面同批改接口」）；`implementation-generator` 单加同步句 | 相位才是做设计决策的地方；零路径模板、零字段形态长表、零占位符；其余 11 份逐字不变 |
| D-19 | **`AGENT.md` / `CLAUDE.md` 的「模块 API 契约」节恰五条**（逐条文本见附录 B），现第六条（`doc` 字段）并入 ④/⑤ 的语义后删除 | spec 明禁增删要点；`doc` 的「只有 `api = none` 可写」已降为约定、不配占一条；两份节内正文逐字相同（R12） |
| D-20 | **`INDEX.toml` 说明段只改第二行（一句话）**：`review 口径：读 <FQN>.schema.json（接口，手写真相）与 <FQN>.md（理由）；status 的 declared = 接口文件在案；本文件与 *.i.* 的 diff 视为机械噪声。` | 验收 2 允许「头部说明段一句」；同时承载 P6 的命名债注解（值域不动 ⇒ 条目逐字节不变） |
| D-21 | **两处机械连带**（不在提案实现文件表，属必然）：`core/archi/src/search.rs`（语料换面，`carrier_path` 退役后唯一读者）、`core/archi/src/lib.rs`（出口面补五名） | 提案把 `dto.rs` 写作检索语料的改动点，实际实现在 `search.rs`（`dto::search` 只是投影）；`lib.rs` 是单点出口纪律（D60）的要求。`drift/mod.rs` 属提案「不要修改」面，其陈旧注释另见 Q-8 |
| D-22 | **前端只动三处注释 / 文案**：`contract-panel.tsx` 冷启动文案改「接口面未启用」、`model-info.tsx` 的 `ContractSection` JSDoc 改「本元素判据」、`architecture-node.tsx` 三路径 push 处补一行语义注释 | P3：零功能改动（chip 由数据聚合、文档段只渲非空项、三路径独立后自然正确）。提案写作「两处 JSDoc」，第三处（冷启动文案）是 `desktop-archi-contract-panel` spec 的显式文案变更（「契约面未启用」→「接口面未启用」），属必然连带；测试夹具断言随动归测试相位 |
| D-23 | **`ArchiModuleDetail.doc` 去掉 `api = none` 过滤**（原样投影 `element.doc`） | spec 三处写「`declared` 元素写了 `doc` 也照原样投影」「该字段零判据读者」；投影零改写是本 crate 纪律。前端「入口文档」段仍只在 `api = none` 渲染（呈现规则不变） |

## 验收标准回应

| AC | 设计回应 |
|---|---|
| AC-1 零行为回归（读面） | 11 份 schema 与 fence 逐项等价 ⇒ `load_contracts` 投影零差；38 条 `docMissing` 的条数 / 构成不变（③ 判派生 md），只有 `message` 文案换面（D-6）；`status = clean` / `passed` 不变（`gating_findings` 新口径的集合 = 今日 11 个元素，D-5） |
| AC-2 生成物零漂移 | `render_all` 不再产 schema（D-13）；`api_dir_artifacts` 收面 ⇒ `orphaned` 不含 `.schema.json`；`GENERATED_NOTICE` 逐字保留 ⇒ `.i.rs` / `.i.ts` 逐字节不变（D-12）；`INDEX.toml` 只差说明段一行（D-20） |
| AC-3 绑定面可归因 | 形状差异 = `assets` / `reading` + 四新类型 + `ArchiApiFindingKind` 三值离场；`ArchiApiRef` / `ArchiSearchResult` 仅注释随动（如实声明，D-11） |
| AC-4 写闸三类坏输入 | C1 + C11：① 文本层重复键 / ② JSON 解析 / ③ identity / ④ 键空间各一条 `Err` 且零落盘（`write` 先校验后建目录）；成功追加「接口已校验」 |
| AC-5 写闸与装载同源 | `validate_schema` 是唯一实现；手工改坏后 `archi_drift` ⇒ `ApiError` 停摆（`drift::check` 把 `api::findings` 的 `Err` 映射为 `CheckError::Config`），文案点名文件与期望值 |
| AC-6 三资产一次给全 | C10 / D-8：`assets` 三段各自 `present`；`interface` 键名摘要 + `abstract`；`doc` 段名 / 行数 / 字节；`reading` 静态常量；薄概览不带 |
| AC-7 双草稿与 `link` | D-14 / D-15：`bootstrap` 回 `{ schema, doc }`（schema 直接过写闸、doc 两行零段名）；`link` 前缀与 `ref_relative` 逐字一致、可被 `apiRefMissing` 判可达；两出口同源（C14） |
| AC-8 词汇与档位 | C7：`ALL` 长度 10、`as_str` 十词、`error` 恰两项、`off` 恰一项、`category` 七组无空组（D-6） |
| AC-9 纳管口径 | D-5：`declared` ∧ schema 在盘 = 11（与今日同值）；`docMissing` 恒 `warn` 且两侧都不成立 ⇒ 永不纳管；`status == errors ⟺ passed == false` 由「error 判据蕴含 schema 在盘」保证 |
| AC-10 上下文面 | D-16 / D-17 / D-18：骨架五段 + 体量锚 1800 / 2000；三份 prompt 新句在场、其余 11 份零 archi 字样；change 臂两写权相位 `preamble` 在场、`implement/executor` 缺席 |
| AC-11 规则面 | D-19：`AGENT.md` / `CLAUDE.md` 各恰 5 条、节内正文逐字相同、零 `api-*` fence 示例、零 `openspec/specs/` 路径 |
| AC-12 写面 warnings 收窄 | C8：五条 conditions 逐条可绿；`docMissing` 只由 `created` 诱发；只改 `doc` 三次 `warnings` 均为 `[]`；`DocNotFound` 零出现 |
| AC-13 `docMissing` 单因单义 | D-6 / D-9：判据面与写面同判「派生 `<FQN>.md` 不在盘」；「md 在盘、schema 缺」⇒ 两面零条目（⑤⑥ 无声明集）；「接口未写」零 kind、零 warning |
| AC-14 版本纪律 | `package.json` 相对落地时 `HEAD` +1 patch（当前 `0.7.16` ⇒ `0.7.17`）；`tauri.conf.json` 跟随；`src-tauri/Cargo.toml` 不动 |
| AC-15 `archi_search` 语料换面 | C10 / D-21：语料 = 在盘 `<FQN>.md` ∪ 在盘 `<FQN>.schema.json`，与 `api` / `doc` 无关；`scope` / 空 `query` 错误面逐字不变；源码零 `carrier_path` / `doc_path` |
| AC-16 `archi_api` 两格下发 | D-16：两格清单含 `archi_api`（proposal 六名 / dev-design 七名），矩阵其余行不变；definitions 与 `RunStarted.tools` 逐名一致；`ToolTier::Read` 与两写工具同集纪律不变 |

## 待决问题 (Open Questions)

| # | 事项 | 处置建议 |
|---|---|---|
| Q-1 | `specs/desktop-archi-agent/spec.md` 的 `link` 场景示例串 `"../../Core/Foundation/Backend.Core.Foundation.schema.json#/$defs/Summary"` 与 `ref_relative("Backend.Infra.ArchiRuntime", "Backend.Core.Foundation")` 实算（`"../Core/"`）不一致 | 以函数单点为验收锚（D-15）；实现期同批修正该示例串一行（不改行为条款、不改其余 spec 文字）。若评审要求保留示例，则验收改判「前缀 == `ref_relative` 输出」而不比示例字面。落点已登记于「条件性修改」表 |
| Q-2 | `assets.interface.abstract` 在 spec 四形态之外还有两读法（「既有操作也有类型」/「接口文件不可解析」） | 本设计按六形态实现（D-9），优先级 = 不可解析 > 含透传 > 空 > 只有操作 > 只有类型 > 两者都有；四形态夹具（spec 场景）逐字命中即可，两补充形态不写进 spec 也行 |
| Q-3 | `#[serde(rename = "in")]` / `#[serde(rename = "abstract")]` 是否被 specta 透传为同名出线 | 验收锚 = `bindings/mod_test.rs` 的字段名断言；不成立即在两字段补 `#[specta(rename = "…")]`（零 API 变化） |
| Q-4 | 知识骨架最终文本与实测体量（草案 1778 B；全文预计 ≈ 1.95 KB） | 实现后按 `lf_bytes(trim_end)` 实测冻结；门限 1800 / 2000 不抬，超限先删同段冗余（D-17） |
| Q-5 | `ArchiModuleDetail.doc` 去掉 `api = none` 过滤是否被接受（D-23） | 若评审要求维持旧过滤，回落面 = 恢复 `api == none` 条件后再投影 `doc`（`then` + `flatten` 一行）；前端与判据面均无影响 |
| Q-6 | `specs/desktop-archi-model/spec.md` 的「消费面词汇集一致性对拍」仍保留陈旧句「知识文本 `kind` 由元素类型 bullet 承载、`gate` / `api` 各一行承载」与其场景「知识文本只承载字段值域与可执行词」——与 `specs/desktop-archi-agent/spec.md` 的「值域 / 缺省全部由工具面承载」「语法知识零残留」直接冲突（该口径在 `archi-agent-contract-knowledge-gap` 已收缩，基线未追改） | 以 `desktop-archi-agent` 为准（它是被重写的一侧且与现行实现一致）；同批把 `desktop-archi-model` 的两处表述改为「呈现源是工具面（`archi_scaffold` 词典与参数 `description`）；知识文本只给纪律句、不给词表」（不改词汇集对拍本体）。若评审判为基线债务不追改，则本期不动、测试不新增强断言（反正现状与现行实现一致）。落点已登记于「条件性修改」表 |
| Q-7 | `specs/desktop-archi-api/spec.md` 的 Module Contract 行提「保留 `Contract` / `ContractOp` / `ContractType`」，但仓内不存在 `ContractType`（`Contract.types` 是 `Map<String, Value>` 字段） | 按现状实现（`Contract` / `ContractOp` 保留，`types` 字段保留）；该 token 视为 spec 笔误，实现期可同批删去（不改行为条款）。落点已登记于「条件性修改」表 |
| Q-8 | `crates/core/archi/src/drift/mod.rs` 的两处注释写着「入口文档承诺已成立元素的判据」，纳管口径改后陈旧；但该文件在提案「不要修改」表内 | 本设计按提案零改动（交付面不含该文件）；若评审放行一行注释追改，改法 = 两处「入口文档承诺已成立」→「接口承诺已成立（`declared` ∧ schema 在盘）」，其余逐字不动；否则归清理轮。落点已登记于「条件性修改」表（需评审放行项） |
| Q-9 | `specs/desktop-archi-model/spec.md` 保留「`doc` 值 MUST NOT 指向 `<archi_dir>/models/**` 与 `openspec/specs/**`（机械拒收 `invalidValue`）」条款，但现状 `parse/**` 未实现该判据（既有债务，本变更实现文件表不含 `parse/**`） | 本变更零动作（不新增机械判据、不改 `doc` 语义）；如需对齐，另起独立 change 在 `parse/toml.rs` 补一条 `invalidValue`（带位置），不塞进本次 |

## 附录 A · 知识骨架草案（D-17，实测 1778 B）

```text
# 架构模型须知

## 三资产分工
TOML 答拓扑：谁在谁的里面、谁依赖谁。schema（唯一凭据）答接触面：跨过这条边时它长什么样。md 答理由（无结构，MUST 整读）：为什么这样切、有什么坑。过度设计 = 回答自己不该回答的那个。文档是产品面：架构页直接渲染给用户读。

## 设计顺序
① `archi_search` 发现 → `archi_module` 取邻居与资产。② `archi_element` / `archi_relation` 画拓扑——先边界、后接触面、最后理由。③ `archi_api` 的 `bootstrap` 取双草稿 → 接口用 `write` / `edit` 写 `.schema.json`（坏文本零落盘）→ 理由写 `.md`。

## 三条不可越界
- 文档不列接口：导出名的唯一持有者是 schema；历史 fence 读面不读、MUST NOT 新增（清理中）。
- schema 不写理由：`x-function.description` 只写调用方要知道的语义，MUST NOT 复述签名；`x-` 面封闭为 `x-function` / `x-class` / `x-common`。
- TOML 不造空元素：每个元素至少一条接触面。

## 工具面
读五件：`archi_search` 发现 / `archi_module` 定位 / `archi_drift` 漂移 / `archi_scaffold` 词典；写两件：`archi_element` / `archi_relation`；`archi_api` 三动作：`bootstrap` 草稿 / `link` 算 `$ref` / `check` 两轴；MUST NOT 手拼 TOML。工具表达不了的才手改，改完 `archi_module` 自查（坏模型由读取面报 `file:line`）；生成物过期用既有命令重生成、MUST NOT 手改；`archi_drift` 非空即逐条修；`config.json` 管 `ignore` / `exclude`。

## 留给判断
`kind` / `path` / `api` / 依赖的选择没有机械答案；纯分组容器用 `composite`。接口面留白就写空 schema（`$defs {}` + `properties {}`），不要编造导出。
```

## 附录 B · `AGENT.md` / `CLAUDE.md` 契约纪律节五条（D-19）

1. **三资产分工与落位**：`<archi_models_dir>/<分层目录…>/<FQN>.md` 写理由（文档，无结构；产品面：架构页直接渲染给用户读）；同目录 `<FQN>.schema.json` 写接口（**唯一凭据**，手写真相，写前过校验）。文档 MUST NOT 枚举导出名（名字清单的唯一持有者是 schema）。历史 `api-*` fence 是遗留内容（读面不读，清理中），MUST NOT 新增。
2. **三类生成物勿手改**：`INDEX.toml`（`<archi_graph_dir>/`）、实现入口同级的 `<入口主干>.i.rs` / `<入口主干>.i.ts`。手改已提交即零 diff 门失败，未提交会被下次生成静默覆盖。
3. **两个脚本**：`api:export` 生成、`api:check` 读盘比对（零写入，差异即非零退出）。`check` 链为四段：`server:check` → `bindings:export` → `api:check` → `client:check`。
4. **`api = none` 是显式留白**（模型元素字段，非「还没写」）：断言本模块无公共 API，写了接口文件反而触发 `apiUnexpected`；留白的正规形态是空 schema（`$defs {}` + `properties {}`），与「接口还没写（文件缺）」严格可分。
5. **纳管口径**：漂移报告并列槽位 `apiFindings` 只把「`api = declared` ∧ schema 在盘」元素的判据折算进 `status`（`error` 档还进 `passed`），文档债 `docMissing` 恒 `warn`、永不纳管；判据定义见 `desktop-archi-api-check`。索引与计数经 `<archi_graph_dir>/INDEX.toml`，两轴（生成物新鲜度 / 契约 vs 代码）经架构页「契约」视图一眼看全。
