# 任务: archi-agent-contract-knowledge-gap

> 顺序即依赖：前一组解锁后一组。每项以文件为入口，落地口径见 `design.md`（决策 `D-1`…`D-19`）与 `specs/` 七份 delta。

## 阶段 1 · 写面判据与引擎语义（`crates/core/archi`）

- [x] `edit/messages.rs`：`EditErrorKind` 扩 19 → 23——新增 `RelationDeleteHasFields` / `RelationDuplicateInBatch` / `CascadeCrossFile` / `RelationOwnedElsewhere`，同批补 `criterion()` 穷尽分支与 `ALL` 数组（长度 23）；既有 19 条判据句与 doc 逐字不动（D-12 / Q-1 措辞纪律：操作中立、逐值互异）。
- [x] `edit/mod.rs`：新增写面类型——`WriteAction { Upsert, Delete }`（`Default`、`ALL: [WriteAction; 2]`、`as_str`、`parse`）、`RelationSpec { to, desc, apis, action }`、`DeleteSet { elements, relations }`（构造子只 `of(fqn)`，零 `is_empty`——D-18）、`WriteCounts`、`RelationTally`、`WriteOutcome`；`ElementSpec` 追加 `relations` / `create` 两字段（既有字段与 `Default` 语义不变；`cascade` 只在 `plan_element_delete` 值参面，**不入 spec**——D-17）。
- [x] `edit/mod.rs`：`apply_element_spec` 增内联出边——元素字段写回后按 `RelationSpec.action` 语义逐条 upsert / delete（复用 `write_relation_fields` / `new_relation_table` / `relation_entries_mut` / `relation_indexes`，MUST NOT 复制第二套关系渲染），`finish` 的回读逐对覆盖（upsert 校验 `desc` / `apis`，delete 校验缺席）；`apply_element_delete` 改为 `(source, &DeleteSet)` 唯一形态——按集合删元素表与关系对，集合内关系缺席即 `RelationNotFound`（列该文件现有关系对），MUST NOT 保留第二个「只删一个元素」的删除实现（AC-5 / D-1）。
- [x] `edit/mod.rs`：`plan_element_upsert` 落点四情形——既有元素落其文件（`file` 不同值 ⇒ `TargetInvalid`）、新建省略 `file` 落父元素文件、单段 FQN 省略 `file` ⇒ `TargetInvalid` 点名需给 `file`、`file` 缺席且无 `create` ⇒ `Err` 提示 `create = true`；`create` 新建文件时按 D-5 前置表头 `# <file 去 .toml 后缀>（N 元素 / M 关系）`（`N` / `M` 取写后重解析），再解析最终文本自证零诊断后才算 `hash`。
- [x] `edit/mod.rs`：`plan_element_upsert` 的内联出边前置——同批重复 `to`（`RelationDuplicateInBatch`，1 基下标 + `action` + `to`）、`delete` 项带 `desc` / `apis`（`RelationDeleteHasFields`）、内联边已声明在别的模型文件（`RelationOwnedElsewhere`，点名文件并指向 `archi_relation`）、目标端点不在案（`RelationEndpointMissing`）；未列出的既有边零改动。
- [x] `edit/mod.rs`：`plan_element_delete(root, fqn, cascade, expect)`——`cascade = false` 保留两拒绝臂（`DeleteHasChildren` / `DeleteHasRelations`）；`cascade = true` 先算闭包（`graph.children` 递归子树 ∪ 任一端在闭包内的关系），全在目标文件即删，跨文件即 `CascadeCrossFile` 点名文件与阻塞项且零文本产出（D-1）；`cascade` 措辞只写「同一模型文件内」；级联开关只有这个值参一个承载位——`ElementSpec` MUST NOT 带同名（或任何）级联字段（D-17）。
- [x] `edit/warnings.rs`（新）：七条判定——`noPath` / `pathNotFound`（`ArchiDriftKind::as_str`，恒 `warn`）、`docMissing`（`declared` 契约 md 缺席 / `none` 未声明 `doc`）、`docNotFound`（`error`）、`apiUnexpected`（`none` 但契约 md 在盘；删除成功后契约 md 仍在盘）；`kind` / `severity` 全取 `as_str` / `severity()` 单点，路径全取 `api::paths::{contract_path, doc_path}`；降噪不变式（`created` 视为全字段，`noPath`/`pathNotFound` 只由 `path`/`kind` 诱发，契约三条只由 `api`/`doc` 诱发，delete 只第 7 条）；导出 `upsert_warnings` / `delete_warnings` 与 `EditWarning`。
- [x] `edit/mod.rs`：四个 `plan_*` 返回 `WriteOutcome` 并装配反馈面——`changed`（写前元素对比；`created` 时为实际给出的字段名）、`created`、`relations` 四计数（按每对 `to` 的写前状态；删除动作四项恒 0，D-7）、`counts`（写后该文件 `TomlParser::parse` 计数，D-2）、`model`（前值 ± 增量：元素 +1 或 −子树规模、关系 +created −deleted）、`warnings`（元素写回调 `upsert_warnings`、删除调 `delete_warnings`；`plan_relation_*` 恒空）。
- [x] `archi/src/lib.rs`：出口补 `WriteAction` / `RelationSpec` / `DeleteSet` / `WriteOutcome` / `WriteCounts` / `RelationTally` 与 `pub mod warnings` 下的 `EditWarning` / `upsert_warnings` / `delete_warnings`；crate 根 doc 五条纪律零改动。
- [x] `dto.rs`：`ArchiModuleDetail` 末尾追加 `file_hash: String`，`module_detail` 内以 `drift::file_hash(&root.join(&element.file))` 填充（D-9：追加表尾）；薄概览与其余 DTO 字段面零改动。

## 阶段 2 · 落盘单点迁移

- [x] `crates/infra/agent/src/sdk/atomic.rs`（新）：迁入 `write_atomic(path: &Path, content: &str) -> Result<(), String>`——同目录临时文件 + `fs::rename`、失败清临时文件、父目录缺失即 `Err` 不建目录、`.archi-tmp` 后缀与两条错误文案逐字沿用；在 `sdk/mod.rs` 补一行 `pub(crate) mod atomic;`（D-13）。
- [x] `crates/infra/agent/src/lib.rs`：`pub use sdk::atomic::write_atomic;`（`sdk` 模块本身仍私有）。
- [x] `src/commands/atomic.rs`：函数体改为 `agent_runtime::write_atomic(path, content)` 薄委托——签名 / 可见性不变，函数体零 `fs::write` / 零 `fs::rename` / 零 `create_dir_all`，doc 改为「薄委托（唯一实现在 infra 侧）」。

## 阶段 3 · 工具面（挂载 / 保护 / 执行体 / 装配）

- [x] `sdk/mount.rs`：两常量 `ARCHI_ELEMENT_TOOL` / `ARCHI_RELATION_TOOL` 与 `WriteLimits` + `pub(crate) const WRITE_LIMITS`（八行上限数字单点）；`archi_element_definition()` / `archi_relation_definition()` 平铺 schema（`enum` 由 `ElementKind::ALL` / `GateTier::ALL` / `ApiStatus::ALL` / `WriteAction::ALL` 派生，`maxLength` / `maxItems` 取自 `WRITE_LIMITS`，逐字段 `description` 写明缺省与陷阱）；静态清单 16 → 18（两写项 `ToolTier::Write` / `ToolExec::Archi` 插在 `archi_scaffold` 之后、`db_tables` 之前）；`Vec::with_capacity(18)`、`registry()` / `build_registry()` 与 `ToolTier` / `ToolExec` 的项数与名单 doc、`source = "archi"` 工具名单描述、`archi_scaffold` 常量 doc 的「不在专用写工具之列」句同批改写。
- [x] `sdk/protect/mod.rs`：抽私有 `deny_normalized(tool, raw) -> Option<String>`（`rules::normalize` → `match_path` → `deny_for`）供 `evaluate_path` 复用；新增 `pub(crate) fn evaluate_target(&self, tool: &str, path: &Path) -> Option<String>` 作为写工具落点判定入口；模块 doc「射程边界」补一句「写模型工具经 `evaluate_target` 判定」；`evaluate` 的 `write` / `edit` / `bash` 三路分派与 `_ => None` 逐字不变。
- [x] `sdk/archi_tool.rs`：`ArchiContext` 增 `protect: Option<ProtectRules>` 与 `with_protect(self, rules: ProtectRules) -> Self`；模块头 doc 由「架构只读工具执行体」改为「五只读 + 两写」。
- [x] `sdk/archi_tool.rs`：新增 `ElementWriteInput` / `RelationWriteInput` 的 `parse`（E1）——`action` 值域（`WriteAction::parse`）、`element`/`from`/`to` 非空与 ≤256、`kind`/`gate`/`api` 由 `ALL` 查表、`path`/`metadata`/`relations`/`apis` 的类型与八行上限（取自 `WRITE_LIMITS`）、`metadata` 值必须字符串、`cascade` 仅 delete；违例一次报全（`；` 分隔，点名参数与合法值 / 上限，D-12）。
- [x] `sdk/archi_tool.rs`：`execute` 增第六、七臂 + `element_payload` / `relation_payload`——E1 → E2（`dto::graph_view` 非空诊断即 `dto::graph_diagnostics_error`）→ `plan_*`（`EditError::message()`）→ `ProtectRules::evaluate_target`（`protect` 缺席即 `Err` 并声明生产不可达）→ `atomic::write_atomic` → 三通道响应装配（`warnings` 三字段 / `action` / `element` 或 `from`+`to` / `file` 经 `archi::display` / `hash` / `created` / `changed` / 元素面另带 `relations` 四计数与 `counts` / `model`）；`archi_relation` 走薄形（`warnings` 恒空、无计数）；`SCAFFOLD_TEMPLATE` 首行改为指向两写工具。
- [x] `sdk/archi_tool.rs`：`scaffold_payload` 函数 doc（现 `:131-132`「只读零写盘——写盘仍走既有 `write` / `edit`」）改写为两写工具 canonical——「落盘走 `archi_element` / `archi_relation`（工具表达不了的才手改 `write` / `edit`）」，与模块头（`:6-9`）和 `SCAFFOLD_TEMPLATE` 首行（`:801` 一带）同口径（D-19 / AC-12(a)）。
- [x] `sdk/archi_tool.rs`：`kind` 解析收成一处文件内私有核 `fn domain_value<T: Copy>(raw: &str, name: &str, all: &[T], as_str: impl Fn(T) -> &'static str) -> Result<T, String>`——`scaffold_payload`（现 `:134-150` 自带查表 + 「非法 kind: …」文案）与 `optional_domain`（现 `:728`）同调它，判据句只剩 `optional_domain` 族一条（`{name} 非法值 "{raw}"（合法值：…）`），合法值清单仍由 `ALL` 现算；`archi_scaffold` 的省略语义（缺失 / 空串 / `null` / 非字符串 ⇒ `domain`）逐字不变（D-19）。
- [x] `sdk/runner.rs`：`mint_archi_context` 判据由五名扩为七名（任一在场即铸）；`session_pump` 在 `ProtectRules` 装配后接线 `archi.map(|context| context.with_protect(protect.clone()))`。
- [x] `sdk/tools.rs`：`input_path` 的 archi 名单加两名（恒 `None`）、`pub(crate) use crate::sdk::mount::{…}` 再导出两常量、doc 的「五个架构只读工具」措辞随动。
- [x] `compose.rs`：`archi_matrix` 的 `(proposal, executor)` / `(dev-design, executor)` 两行追加两写工具；archi 来源臂名字清单五 → 七；模块头 doc 的臂描述随动；`model_write_lock` 零改动。

## 阶段 4 · 壳层命令 / 知识骨架 / 相位 prompt

- [x] `src/commands/archi/mod.rs`：`archi_set_gate_inner` 取 `plan_element_upsert(...)?.write` 再 `write_atomic`（签名 / 错误面 / 失败零写入不变），模块头 doc 写明「写工具与命令面共用同一 `plan_element_upsert`」「命令面不暴露 `expect`」。
- [x] `crates/infra/agent/src/archi-knowledge.md`：六段收缩为四段（职责 / 工具面 / 留给判断 / 派生路径段不在骨架内）——采纳 `design.md` 附录 A 草案；保留「手改出口」一句（工具表达不了的才手改、改完 `archi_module` 自查、坏模型由读取面报错带 `file:line`）、`config.json` 一行、画布不自动刷新与续接先取当下模型两句；零示例 TOML 围栏 / 零键形示例 / 零字段值域行 / 零结构诊断词；骨架 ≤1500 B、全文 ≤1700 B（超限先删同段冗余，不抬锚）。
- [x] `crates/infra/agent/src/archi_knowledge.rs`：模块头 doc 的骨架描述由六段改为四段；`build` / `knowledge` 的拼装与降级语义逐字不变。
- [x] `crates/core/workflow/src/prompts/proposal-planner.md`：工具行补 `archi_element` / `archi_relation`（措辞改为「额外可用 archi 工具：读三件 + 写两件」）；写面句改为「用 `archi_element` / `archi_relation` 写（工具表达不了的才手改）」；保留「只为已确定元素落笔」与 `archi_module` 自查句。
- [x] `crates/core/workflow/src/prompts/dev-design-planner.md`：同上（工具行由「四个只读工具」改为「读四件 + 写两件」，自查句仍 `archi_drift` + `archi_module`）。

## 阶段 5 · 生成物 / 版本 / 自查

- [x] 重导生成物：跑 `bindings:export`，确认 `bindings.ts` 与 `bindings.zod.ts` 的 diff **恰为** `ArchiModuleDetail` 增 `fileHash` 一字段（前者含 JSDoc、后者 `ArchiModuleDetailSchema` 同字段），无其它 diff；MUST NOT 手改生成物。
- [x] `packages/desktop/package.json`：`version` 0.7.14 → 0.7.15（+1 patch）；`src-tauri/Cargo.toml` 与 `tauri.conf.json` 不动。
- [x] 落笔后自查（只读）：用 `archi_drift` 确认 `.dev-team/architecture/**` 零漂移（55 元素 / 129 关系 / 表头计数不变），用 `archi_module` 抽查新写工具落点的 `fileHash` 与 `hash` 同值；并 `grep` 四条源码锚——「`fs::write(&temporary` / `fs::rename(&temporary, path)` 各恰一处」「删除纯函数只 `apply_element_delete` 一形」「写面限额数字只在 `WRITE_LIMITS` 一处」「`无专用写工具` / `不设专用写工具` / `MUST NOT 存在专用写` 在产品源码与两份 prompt 零命中」。

## 阶段 6 · 回溯收口（死声明清除 / 文档口径对齐）

> 本组是设计面回溯裁决（`design.md` 的 D-17 / D-18 / D-19；D-19 的两件 `archi_tool.rs` 修正补在阶段 3 末尾），实现相位 MUST 按此修正，MUST NOT 以「已实现」为由保留死声明或双份文案。

- [x] `crates/core/archi/src/edit/mod.rs`：删 `ElementSpec.cascade` 字段及其 doc 尾句（现 `:96-98`）——级联的唯一承载位是 `plan_element_delete` 的值参 `cascade: bool`（现 `:463` 一带，逐字不动）；`crates/infra/agent/src/sdk/archi_tool.rs` 的 `ElementWriteInput::spec()` 同批删 `cascade: self.cascade,` 一行（现 `:426`）。`ElementWriteInput.cascade` 本身**保留**：`plan_element_delete` 调用点（现 `:218`）与 E1 的「`cascade` 仅 `action = delete`」硬闸仍用它。
- [x] `crates/core/archi/src/edit/mod.rs`：删 `DeleteSet::is_empty`（现 `:168-171`）——产品路径零读者；`DeleteSet` 只留 `of(fqn)` 构造子，集合判空由调用方按公开字段判（D-18）。
- [x] `crates/core/archi/src/edit/mod_test.rs`：同批修既有断言口径（不新增用例）——`:699` 的 `ElementSpec` 全字面构造删 `cascade: false`；`ElementSpec缺省扩展不破既有简写`（`:4420-4432`）改判「两新字段」（`relations` / `create`）并删 `assert!(!spec.cascade, …)`；`单元素删除是集合退化形`（`:3752-3761`）删两处 `is_empty()` 断言，保留字段面断言（`elements.len()` / `relations.is_empty()`）。
- [x] `openspec/changes/archi-agent-contract-knowledge-gap/specs/desktop-archi-model/spec.md`：Module Contract 清单行（现 `:216`）删去 `/ cascade: bool` 一并列 token——`relations: Vec<RelationSpec>` 与正文的「`cascade` 闭包（子树 ∪ 出入边，跨文件 `Err`）」/ `plan_element_delete(root, fqn, cascade, expect)` 条款逐字保留；MUST NOT 改其余 spec 文字、MUST NOT 触碰 `openspec/specs/**` 基线（D-17，AC-16 允许面内）。
- [x] `openspec/changes/archi-agent-contract-knowledge-gap/test-design.md`：同批改口径（不新增用例）——`:197` 的 `DeleteSet` 类型描述删 `/ is_empty()`；`:219` 的退化形用例删两处 `is_empty()` 断言表述（保留「`of(fqn)` ≡ 单元素 + 零关系」判据）；`:237` 的「`of` / `is_empty` 两臂」改「`of(fqn)` 单构造子 + 公开字段判空」；`:239` 的 `ElementSpec` 描述由「既有八字段 + 三字段 / 三新字段」改「既有字段 + `relations` / `create` 两字段」（D-17 / D-18）。
- [x] 收口后重跑阶段 5 的只读自查：`ElementSpec` / `DeleteSet` 结构锚与 `archi_scaffold非法kind报错点名四值` 全绿；`grep "fn is_empty" crates/core/archi/src/edit/mod.rs` 零命中；「写盘仍走既有」「非法 kind:」在 `packages/desktop/src-tauri/**` 零命中（阶段 3 补的两件落地后按同判据复跑）；`archi_drift` 仍零漂移（55 元素 / 129 关系）；`git status --porcelain` 的改动集仍 ⊆ 提案文件表 + `sdk/mod.rs` 一行 + 本 change 目录。
