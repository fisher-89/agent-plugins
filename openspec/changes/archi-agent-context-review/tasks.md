# 任务: archi-agent-context-review

> 顺序即依赖：前一组解锁后一组。每项以文件为入口，落地口径见 `design.md`（决策 `D-1`…`D-23`）与 `specs/` 十一份 delta。
> 测试编写由独立相位承接：本清单不列测试任务；凡行为面变化会连带改既有断言的，已在 `design.md` 对应行注明「随动」。
> 阶段 7 的四条「若 `Q-n` 裁决为 …」为**条件项**：均已登记于 `design.md`「条件性修改（随待决问题裁决）」表，未裁决即零动作。

## 阶段 1 · 校验单点与契约投影（`crates/core/archi` 底座）

- [x] `crates/core/archi/src/api/contract.rs`：新增 `Contract::from_schema(fqn, file, schema) -> Contract`（D-1）——`properties` 里每个带 `x-function` 的键一项 `ContractOp`，**解包** `x-function` 存 `signature` 并**丢弃** `description`、`line = 0`；`types` = `$defs` 中非纯 `$ref` alias 的键；`reexports` = 纯 alias 且经 `crate::api::reexport_target` 可反查目标 FQN 的键（值 = 目标 FQN）；无文件部分的本地 alias 不入 `reexports`。
- [x] `crates/core/archi/src/api/contract.rs`：md 解析主体退役——删 `parse` / `finish_section` / `parse_object` / `parse_object_value` / `string_end` / `decode_key` / `file_title` / `collect_reexport_prose` / `collect_surface_table` / `is_cjk` / `info_tokens` / `duplicate_key_error`、`ContractSection`、`FENCE_OP` / `FENCE_TYPES` / `FENCE_REEXPORT`、`Contract.reexport_prose` / `Contract.sections`；`verify` 只留七族名字 / `x-function` 落点 / `required ⊆ properties` 三闸（删 op id 重复闸、types ⊆ `$defs` 死闸、散文表格闸、reexport 键集闸）；`duplicate_keys` 提为 `pub(crate)`、`is_identifier` 提为 `pub`；模块头 doc 重写为「schema 校验 + `Contract` 投影」（D-2）。
- [x] `crates/core/archi/src/api/paths.rs`：`carrier_path` / `doc_path` 删除（资产① 落点恒为派生 `<FQN>.md`，`doc` 退出派生面）；新增 `pub fn schema_fqn(file_name: &str) -> Option<&str>`（`SCHEMA_FILE_SUFFIX` 唯一反查，`reexport_target` 改调它）；`checked_root` 提为 `pub(crate)`、`is_fqn` 提为 `pub`；相关 doc 重写。
- [x] `crates/core/archi/src/api/check.rs`：`is_pure_ref_alias` 由私有提为 `pub(crate)`（**纯可见性、零行为改动**）。单列在此是为了让阶段 2 的 `dto.rs` 二分（`interface.types` / `interface.reexports`）不依赖阶段 3——它是 `declared_types`（阶段 3 判据面）与 `interface` 三数组（阶段 2 投影面）**共用**的唯一纯 alias 判据，MUST NOT 在 `dto.rs` 另写一份；本项不动 `check.rs` 的其余任何行（两支链、`category_of`、`doc_present` 全归阶段 3）。
- [x] `crates/core/archi/src/api/schema.rs`：`build_schema` 删除；新增 `verify_identity` / `verify_keyspace`（D-3：op 键集 ∩ `$defs` 键集 = ∅）/ `verify_shape`（D-4：恰一键二选一，`x-function` 值须对象）/ `verify_mirror` / `verify_ref_prefix`（跨元素 `$ref` 前缀逐字等于 `ref_relative`）/ `verify_top_level_keys`（六键封闭、`$id` / `title` / `$defs` / `properties` 必在场）；新增唯一公开入口 `validate_schema(fqn, text) -> Result<Contract, ApiError>`（闸序 ① 重复键 → ② 解析 → ③ identity → ④ 键空间 → ⑤ 形态 → ⑥ 镜像 → ⑦ `$ref` 前缀 → ⑧ 顶层键 → ⑨ `verify` + `verify_defs` 同趟）；`build_index` 说明段第二行改一句话（D-20 的源头文本），`present` 头注释改写；`verify_defs` / `degraded_marks` / `build_index` / `ApiFormStatus` / `DEGRADED_KINDS` 结构逐字保留。
- [x] `crates/core/archi/src/lib.rs`：出口面追加 `BootstrapDrafts`、`validate_schema`、`schema_fqn`、`is_fqn`、`is_identifier`（单点出口纪律，D-21）；既有逐名 `pub use` 行只增不改。

## 阶段 2 · 读面换真相与生成面让位（`crates/core/archi`）

- [x] `crates/core/archi/src/api/mod.rs`：模块头 doc 重写（三资产 / 零 md 回落 / 十判据 / 三类生成物）；`ApiSnapshot.doc_facts: BTreeMap<String, DocFact>` → `doc_present: BTreeMap<String, bool>` 并删 `DocFact`；`ApiCheckInput.doc_facts` → `doc_present`；`doc_facts` → `doc_presence`（`contract_path(fqn).is_some_and(is_file)`）、删 `has_api_fence` / `facts_enabled`。
- [x] `crates/core/archi/src/api/mod.rs`：`load_contracts` 只读 `<FQN>.schema.json`——`api = none` / `path` 空跳过；文件不在盘跳过；在盘即 `fs::read_to_string` → `schema::validate_schema` 装配 `Contract` + `schemas`，并把 `Contract.file` 覆写为 repo 相对路径（`display`，报告 `occurrences.file` 的口径）；任一份失败 ⇒ `ApiError` 停摆（零半真快照）。
- [x] `crates/core/archi/src/api/mod.rs`：`enabled` 改判「任一 `path` 非空 ∧ `api = declared` 元素 schema 在盘」并作为**单点纯函数**被 `artifact_check` 与 `findings` 同调（D-5）；`gating_findings` 入选 = `element` 非空 ∧ `api = declared` ∧ schema 在盘。
- [x] `crates/core/archi/src/api/mod.rs`：`ref_projection` 重写——`status` = schema 在盘 ? `Declared` : `None`；`contract_path` / `schema_path` 各自在盘才出串；`generated_path` 有可用入口即给路径串（即便未生成）；删「三路径全或全无」分支（D-11）。
- [x] `crates/core/archi/src/api/mod.rs`：`render_all` 只产 `.i.rs` / `.i.ts` / `INDEX.toml`（删 schema 序列化与写出分支、删其错误臂）；`index_entries` 的 `present` 直判 `paths::schema_path(...).is_file()`（D-13）。
- [x] `crates/core/archi/src/api/mod.rs`：`bootstrap` 改返回 `BootstrapDrafts { schema, doc }`；`bootstrap_text` 拆成 `schema_draft(fqn, symbols)`（D-14：六顶层键、每 op 一项 `x-function` 含 `description: ""`、每类型一项 `$defs` + `properties` 镜像；空符号集 ⇒ 两空对象）+ `doc_draft(fqn)`（`# <FQN>` + 一句指引，零段名）；删 `BOOTSTRAP_NOTE`；新增 `SCHEMA_WRITER_NOTICE`（`GENERATED_NOTICE` 逐字保留，D-12）。
- [x] `crates/core/archi/src/api/diff.rs`：`api_dir_artifacts` 收面只留 `<archi_graph_dir>/INDEX.toml`（`*.schema.json` 退出孤儿枚举），模块头 / 函数 doc 随动；与上一项同一提交语义（R1）。
- [x] `crates/core/archi/src/dto.rs`：`ArchiApiRef` JSDoc 换语义（`status` = 接口文件在案、`contractPath` = 文档路径、`schemaPath` = 接口路径；字段名 / 类型 / 顺序不变）；`ArchiModuleDetail` 追加 `assets: ArchiAssetSet` + `reading: String`（声明序在 `file_hash` 之后）；新增 `ArchiAssetSet` / `ArchiTopologyAsset`（`in_degree` 经 `#[serde(rename = "in")]`，D-7）/ `ArchiInterfaceAsset`（`abstract_note` 经 `#[serde(rename = "abstract")]`）/ `ArchiDocAsset`，均 `specta::Type` + `serde(rename_all = "camelCase")`。
- [x] `crates/core/archi/src/dto.rs`：`module_detail` 装配三资产——`topology` 取模型事实（`file` / `line` / 出入度数，零额外读盘）；`interface` 读 schema 键名（`ops` = 带 `x-function` 的 `properties` 键、`types` / `reexports` = `$defs` 键按 `check::is_pure_ref_alias` 二分）、`abstract` 六形态判读（D-9，坏 JSON 降级不 `Err`）、`present` / `bytes` 按盘上事实；`doc` 只读 `##` 段名 + `lines()` + 字节（零正文）；`reading` 取静态常量（D-10）；`doc` 字段改为原样投影 `element.doc`（D-23）；`ArchiSearchResult.doc` 注释改为「资产路径（文档 / 接口）」。
- [x] `crates/core/archi/src/search.rs`：语料换两条固定规则——`path` 非空元素的在盘 `<FQN>.md` ∪ 在盘 `<FQN>.schema.json`；删 `carrier_path` 调用（`paths::{contract_path, schema_path}` 各判 `is_file`）；`SearchOutcome.doc` 注释随动；`MUST NOT` 复活第二次载体派生（D-21）。

## 阶段 3 · 判据面被动收缩（`crates/core/archi`）

- [x] `crates/core/archi/src/api/check.rs`：两支链重写（D-6）——`none` 支 2 步（`apiUnexpected` = schema 在盘 → `docMissing` = 派生 md 不在盘）、`declared` 支 4 步（`docMissing` → `apiEntryMissing` → `apiMissing` → 一致性）；删 `reexport_drift` / `doc_not_found` / `api_doc_missing` / `doc_missing_none` / `doc_missing_declared`，并成一条 `doc_missing_message`（文案单因单义、不出现「入口文档」指针措辞）；`doc_present` 取代 `doc_facts`；`apiUnexpected` 单形态。
- [x] `crates/core/archi/src/api/check.rs`：`category_of` 收成七组（`doc` 只剩 `DocMissing`）；模块头 doc 两支链与「十三」字面重写；`declared_types` / `undeclared` / pair 锚点三条 / `entry_missing_message` 逐字保留（`apiUndeclared` 仍 `off`）；`is_pure_ref_alias` 的可见性已在阶段 1 提升，本项零改动该函数。
- [x] `crates/core/archi/src/drift/report.rs`：`ArchiApiFindingKind` 删 `ApiReexportDrift` / `DocNotFound` / `ApiDocMissing`；`ALL: [Self; 10]`；`as_str` 十词；`severity` 的 `error` 恰 `ApiMissing` / `ApiSignatureMismatch` 两项；相关 doc（「十三」「四组」表述）随动；`ArchiApiFinding` / `ApiFindingSeverity` / `ArchiCheckStatus` / `REPORT_VERSION` 零改动。
- [x] `crates/core/archi/src/edit/warnings.rs`：条件表 7 → 5（D-6 / C8）——`noPath` / `pathNotFound` 不变；`docMissing` = `created` ∧ 派生 md 不在盘（与 `api` 无关）；`apiUnexpected` = `none` ∧ schema 在盘（upsert 与 delete 两处）；`path_on_disk` 改走 `checked_root` + `root.join`；`contract_on_disk` → `schema_on_disk`；`api_induced` 去掉 `doc`；模块头 doc 与降噪不变式（只改 `doc` ⇒ 恒空）重写。
- [x] `crates/core/archi/src/model.rs`：`Element.api` 注释改为「接口承诺开关」；`Element.doc` 注释改为「约定（非机械判据）：只有 `api = none` 会写；零判据读者」；字段集与八字段顺序零改动。

## 阶段 4 · 写闸与工具面（`crates/infra/agent`）

- [x] `crates/infra/agent/src/sdk/tools.rs`：`execute` 把 `root` 传给 `write` / `edit`（两调用点）；新增私有 `schema_target_fqn(root, path) -> Option<String>`（绝对路径 `starts_with(root.join(foundation::layout::archi_models_rel_dir()))` ∧ 文件名经 `archi::schema_fqn`）；`write` 落盘前校验 `content`、`edit` 校验替换后正文，失败即 `Err` 且零落盘（`write` 先校验后 `create_dir_all`）；成功且目标是接口文件时响应追加「接口已校验」一行；非该路径零校验；`input_path` / `execute` 对外签名零改动（C11）。
- [x] `crates/infra/agent/src/sdk/archi_tool.rs`：`api_payload` 三动作——`check` 不变；`bootstrap` 回 `{ schema, doc }`（取 `archi::api::bootstrap` 的 `BootstrapDrafts`）；新增 `link`（D-15：`{ from, to, name }` → `ref_relative(from,to) + to + ".schema.json#/$defs/<name>"`；`{ element, name }` → `"#/$defs/<name>"`；两形态互斥、`from` / `to` 经 `paths::is_fqn`、`name` 经 `archi::is_identifier`；返回 `{ "ref": … }`；纯计算零读盘）；词汇外 `action` 的 `Err` 文案并列三值；`SCAFFOLD_TEMPLATE` 首行 / 描述句按接口面更新（接口草稿指 `archi_api` 的 `bootstrap`，词典正文不动）；模块头 doc 与 `api_payload` doc 重写。
- [x] `crates/infra/agent/src/sdk/mount.rs`：`archi_module_definition()` 五句（三资产一次给全 / 接口唯一凭据 / 文档整读 / `resolvedEntry` + `fileHash` / `contractPath` 是文档、`schemaPath` 是接口）；`archi_api_definition()` 四句（双草稿 / `link` 勿手拼 / 三类生成物勿手改 / 冷启动口径）+ `action` enum 加 `link` + `from` / `to` / `name` 参数；`archi_scaffold_definition()` 一句（接口草稿用 `archi_api` 的 `bootstrap`）；工具名常量 / 参数 schema（除 `archi_api`）/ 档位 / 清单项数零变化。

## 阶段 5 · 装配与知识（`crates/infra/agent`）

- [x] `crates/infra/agent/src/compose.rs`：`archi_matrix` 的 `(proposal, executor)` 行补 `ARCHI_API_TOOL`（order = module / search / api / scaffold / element / relation）、`(dev-design, executor)` 行补 `ARCHI_API_TOOL`（order = module / search / api / drift / scaffold / element / relation）；change 臂 `preamble = model_write_lock(provenance).is_none().then(crate::archi_knowledge::knowledge).flatten()`（单一判据、失败降级 `None`）；模块头 doc 的臂描述随动；`model_write_lock` / archi 臂 / config 臂逐字不变（D-16）。
- [x] `crates/infra/agent/src/archi-knowledge.md`：五段重写——三资产分工（含「文档是产品面」）/ 设计顺序（先边界、后接触面、最后理由）/ 三条不可越界（文档不列接口 / schema 不写理由 / TOML 不造空元素）/ 工具面（读五写二、接口三动作、手改出口、自检、`config.json`）/ 留给判断 + 兼容（空 schema、历史 fence 读面不读）；采纳 `design.md` 附录 A 草案并按实测冻结体量（骨架 ≤ 1800 B、全文 ≤ 2000 B；零围栏 / 零值域行 / 零诊断词 / 零 `<>` 占位符）。
- [x] `crates/infra/agent/src/archi_knowledge.rs`：模块头 doc 的骨架描述改为五段；派生段改为「模型目录：…；契约资产：…/<分层目录…>/<FQN>.md（理由）+ <FQN>.schema.json（接口，手写真相）」；`build` / `knowledge` 的占位符 / 空骨架校验与降级语义逐字不变。

## 阶段 6 · 规则面 / prompt / CLI / 前端 / 版本 / 生成物

- [x] `crates/core/workflow/src/prompts/proposal-planner.md`：工具行补 `archi_api`（`bootstrap` 取双草稿 / `link` 算 `$ref`）；写面句追加设计顺序（先拓扑、后接触面、最后理由）与接口面纪律（留白写空 schema `$defs {}` + `properties {}`，MUST NOT 编造导出；改完导出面同批改接口）；零路径模板、零字段长表（D-18）。
- [x] `crates/core/workflow/src/prompts/dev-design-planner.md`：同上（工具行改「读五件」+ `archi_api`）；既有 `archi_drift` / `archi_scaffold` 行与自查句逐字保留。
- [x] `crates/core/workflow/src/prompts/implementation-generator.md`：单加一句「改了导出面（新增 / 删除 / 改签名）就同批同步该元素的接口定义文件，改完用 `archi_api` 的 `check` 自查」；工具行三名逐字不变。
- [x] `AGENT.md` + `CLAUDE.md`：「模块 API 契约」节改写为**恰 5 条**（逐条文本见 `design.md` 附录 B），两份节内正文逐字相同；零 `api-*` fence 示例、零 `openspec/specs/` 路径、零判据长表、`.schema.json` 不称生成物（D-19 / R12）。
- [x] `crates/tools/archi-api-gen/src/main.rs`：`generate` 与 `Usage` 文案改三类产物（`.schema.json` 不再是产物）；`bootstrap` 打印单行 JSON `{ "schema": "…", "doc": "…" }`（P7）；`Command` / `run` 的退出码语义逐字不变。
- [x] `packages/desktop/src/views/architecture/components/dashboard/contract-panel.tsx`：冷启动文案「契约面未启用」→「接口面未启用」（功能代码零改动）。
- [x] `packages/desktop/src/views/architecture/components/model-info.tsx`：`ContractSection` 的 JSDoc 改为「本元素判据」（删「三路径行」表述）；渲染结构零改动。
- [x] `packages/desktop/src/views/architecture/components/dashboard/architecture-node.tsx`：三路径 push 处补一行注释（`contractPath` = 文档、`schemaPath` = 接口、`generatedPath` = 生成物，各自独立在盘事实）；组件行为零改动。
- [x] `packages/desktop/package.json`：`version` 相对落地时 `HEAD` 现值 +1 patch（当前 `0.7.16`）；`src-tauri/Cargo.toml` 的 crate 版本行与 `tauri.conf.json` 手改零处（后者经 `"../package.json"` 跟随）。
- [x] 重生成生成物（既有脚本，零手改）：`pnpm run api:export` → `.dev-team/architecture/graph/INDEX.toml` 的 diff 只允许说明段一行（D-20），`.i.rs` / `.i.ts` 零 diff；`pnpm run bindings:export` → diff 只为 `ArchiModuleDetail` 的 `assets` / `reading`、四个新 DTO 类型与 `ArchiApiFindingKind` 三值离场（`bindings.ts` 的对应 JSDoc 随 `ArchiApiRef` / `ArchiSearchResult` 注释同步）；若由桌面端统一执行，则在交付前核对同一 diff 结论。

## 阶段 7 · 收口自查（只读）

- [x] 生成物轴：`pnpm run api:check` 零差异（`mismatched` / `orphaned` / `missing` 三列与改前相同，`orphaned` 不含任何 `.schema.json`）；`.i.rs` / `.i.ts` 逐字节不变。
- [x] 读数轴（实现相位按模型与盘面核对：49 个 `path` 非空元素 − 11 份派生 md = 38 条 `docMissing`，其中 35 条 `declared` + 3 条 `api = none`；纳管子集 = 11 个 `declared` ∧ schema 在盘元素 ⇒ `status = clean` / `passed = true`；`archi_drift` 实跑由桌面端执行）：`archi_drift` 跑一次——`apiFindings.total` 仍 38、`byCategory` 仍全 `doc`、`status = clean`、`passed` 不变；除 `timestamp` / `durationMs` 与 38 条 `docMissing` 的 `message` 文案外逐字段不变；`modelHash` 逐字相同（零模型字节改动）。
- [x] 词汇轴：`grep` 三条锚——`ArchiApiFindingKind::ALL` 长度 10、`severity` 的 `error` 恰两项、`apiReexportDrift` / `docNotFound` / `apiDocMissing` 在产品源码 / 生成物 / 前端三处零命中（枚举定义与生成物逐字比对）。
- [x] 退役轴：源码零 `carrier_path` / `doc_path` 引用（含 `search.rs` / `edit/warnings.rs`）；`build_schema` / `contract::parse` / `BOOTSTRAP_NOTE` / `FENCE_*` 零命中；`.dev-team/architecture/models/**` 的 11 份 `.md` 与 11 份 `.schema.json` 字节零变化。
- [x] 工具面轴（实现相位静态核对；运行时三态响应形状由桌面端执行）：`archi_module` 单元素响应含 `assets` 三段与 `reading`（缺 / 空 / 齐三态各看一次响应形状）；`bootstrap` 返回 `{ schema, doc }` 且 schema 骨架原样 `write` 落盘通过写闸并回「接口已校验」；`link` 两形态出的串被 `apiRefMissing` 判可达（前缀与 `ref_relative` 逐字一致）。
- [x] 上下文轴：骨架 / 全文按 `lf_bytes(trim_end)` 实测 ≤ 1800 / 2000 B（冻结值与断言随动由测试相位承接）；change 臂两写权相位 `preamble` 在场、`implement/executor` 与各 evaluator 相位缺席；`(proposal, executor)` / `(dev-design, executor)` 的 `RunStarted.tools` 含 `archi_api` 且与 definitions 逐名一致。
- [x] 规则轴：`AGENT.md` 与 `CLAUDE.md` 的 `## 模块 API 契约` 节各恰 1 处、节内恰 5 条要点、节内正文 `diff` 零行；该节零 `api-*` fence 示例、零 `openspec/specs/` 路径。
- [x] 版本轴：`git diff` 的版本面只有 `packages/desktop/package.json` 的 `version` 一行（+1 patch）；`src-tauri/Cargo.toml` / `tauri.conf.json` 的版本行零变化。
- [ ] 若 `design.md` Q-1 裁决为「修 spec 示例」：`openspec/changes/archi-agent-context-review/specs/desktop-archi-agent/spec.md` 的 `link` 场景示例串一行改为 `ref_relative` 实算值（行为条款与其余文字零改动）。
- [ ] 若 `design.md` Q-6 裁决为「追改陈旧句」：`openspec/changes/archi-agent-context-review/specs/desktop-archi-model/spec.md` 的两处表述（「`kind` 由元素类型 bullet 承载、`gate` / `api` 各一行」与场景「知识文本只承载字段值域与可执行词」）改为「呈现源是工具面（`archi_scaffold` 词典与参数 `description`）；知识文本只给纪律句、不给词表」，与 `desktop-archi-agent` 的「语法知识零残留」同口径（词汇集对拍本体零改动）。
- [ ] 若 `design.md` Q-7 裁决为「修笔误」：`openspec/changes/archi-agent-context-review/specs/desktop-archi-api/spec.md` 的 Module Contract 行删去 `ContractType` 一并列 token（`Contract` / `ContractOp` / `types` 字段逐字保留；行为条款零改动）。
- [ ] 若 `design.md` Q-8 裁决为「放行注释追改」：`crates/core/archi/src/drift/mod.rs` 两处注释的「入口文档承诺已成立」改为「接口承诺已成立（`declared` ∧ schema 在盘）」，其余逐字不动（提案「不要修改」面，需评审放行；否则归清理轮）。
