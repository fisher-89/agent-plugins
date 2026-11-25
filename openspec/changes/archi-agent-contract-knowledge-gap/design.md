# 设计: archi-agent-contract-knowledge-gap

> 变更: `archi-agent-contract-knowledge-gap`（架构模型知识工具化 · 切片 A）
> 输入: 本目录 `proposal.md`（定稿）与 `specs/` 下七份 delta（`desktop-archi-agent` / `desktop-archi-model` / `desktop-agent-execution` / `desktop-phase-prompts` / `desktop-archi-chat` / `desktop-write-protection` / `desktop-config-agent`）
> 性质: 写面（两工具）+ 引擎语义（内联出边 / 单文件级联 / 落点 / 反馈面）+ 知识骨架收缩 + 五处立场改写，一次原子交付。

## 架构组件 (Architecture Components)

### C1 · 写回原语族（引擎）

- **职责**：模型写回的**唯一**语义层——三态字段、内联出边（批量四条）、落点四情形、`cascade`（单文件闭包）、删除纯函数收敛为 `DeleteSet` 一形、反馈面（`changed` / `relations` / `counts` / `model` / `warnings`）装配、自证链（渲染 → 重解析 → 表头回填 → 重解析自证 → 回读），以及乐观锁 `expect`。
- **文件**：`packages/desktop/src-tauri/crates/core/archi/src/edit/mod.rs`（改）、`edit/messages.rs`（改）、`edit/warnings.rs`（新）。
- **依赖**：`foundation::layout::archi_graph_model_dir`、`toml_edit`、`crate::parse::{TomlParser, parse_dir}`、`crate::graph::ModelGraph`、`crate::api::paths`（契约与 `doc` 路径派生）、`crate::drift::report`（两词汇枚举的 `as_str` / `severity`）、`crate::drift::file_hash_of`。零 Tauri / 零 store / 零子进程 / 零写盘（写盘仍在 infra 侧 `write_atomic`）。
- **技术**：纯函数层（`apply_*`，零文件系统）与计划层（`plan_*`，读盘定位）两层不变；产出形状由 `ModelWrite` 扩为 `WriteOutcome`（内嵌 `write: ModelWrite`，三字段逐字不变）。
- **接线**：`archi::lib` 出口补 `WriteAction` / `RelationSpec` / `DeleteSet` / `WriteOutcome` / `WriteCounts` / `RelationTally` / `EditWarning` / `warnings` 模块；壳层 `archi_set_gate` 与 agent 写工具**同调** `plan_element_upsert`（同实现 ⇒ 同文本）。

### C2 · 写面反馈判据（七条 warnings）

- **职责**：把既有判据面的七个词（`noPath` / `pathNotFound` / `docMissing` ×2 / `docNotFound` / `apiUnexpected` ×2）在写面重述为「已落盘、未阻断」的事实，并施加降噪不变式（只由 `changed` 诱发，动作面一条例外）。
- **文件**：`edit/warnings.rs`（新，与 C1 同 crate 的兄弟模块）。
- **依赖**：`crate::api::paths::{contract_path, doc_path}`（路径派生单点）、`ArchiDriftKind::as_str` / `ArchiApiFindingKind::as_str` / `ArchiApiFindingKind::severity` / `ApiFindingSeverity::as_str`（词汇与档位单点）、`Path::is_file` / `exists`。**零** `drift::check`、零 scan、零第二套模型。
- **技术**：`EditWarning { kind: &'static str, severity: &'static str, message: String }`——`kind` / `severity` 存 `as_str` 的返回值，所以字面量持有者仍是既有两个枚举（MUST NOT 新增第三份词表）。

### C3 · 线面投影（`fileHash` 读侧闭环）

- **职责**：给 `archi_module` 单元素响应补单文件内容哈希，使写工具的 `expect` 有读侧基线。
- **文件**：`crates/core/archi/src/dto.rs`（改）。
- **依赖**：`crate::drift::file_hash`（已有 `pub`，同一函数同一 `sha256:` 前缀）。
- **技术**：`ArchiModuleDetail` 末尾追加 `file_hash: String`，在 `module_detail` 内用 `modelFile` 定位后取值（零额外读盘、零第二份哈希实现）；薄概览（`ArchiElementBrief`）不带。

### C4 · 工具挂载与定义面

- **职责**：工具面的唯一事实源——两写工具的常量、平铺 schema（八行上限取自同一组 `WRITE_LIMITS`）、准入档（`ToolTier::Write`）、执行体分类（`ToolExec::Archi`），以及静态清单 16 → 18 项。
- **文件**：`crates/infra/agent/src/sdk/mount.rs`（改）、`sdk/tools.rs`（改：`input_path` 名单 + 常量再导出）、`sdk/runner.rs`（改：铸造判据七名 + `with_protect` 接线）。
- **依赖**：`archi::dto::{ElementKind, GateTier, ApiStatus}`、`archi::edit::WriteAction`（值集派生）、`rig::completion::ToolDefinition`。
- **技术**：`enum` 由封闭词表 `ALL` 派生（`ElementKind::ALL` / `GateTier::ALL` / `ApiStatus::ALL` / `WriteAction::ALL`）；上限常量 `WRITE_LIMITS` 单点，schema 键值与执行体硬闸共用；清单插入位 = 五 archi 只读之后（七 archi 工具连续成组）。

### C5 · 写工具执行体

- **职责**：两个写工具的**参数映射 + 落盘 + 响应装配**——E1 入参解析与硬闸、E2 结构诊断前置、`plan_*` 调用、目标文件过写保护、`write_atomic` 落盘、三通道响应（`warnings` / `changed` / 其余）。
- **文件**：`crates/infra/agent/src/sdk/archi_tool.rs`（改）、`sdk/protect/mod.rs`（改：新增落点判定入口）。
- **依赖**：`archi::edit::{plan_element_upsert, plan_element_delete, plan_relation_upsert, plan_relation_delete, ElementSpec, RelationSpec, WriteOutcome, WriteAction, EditError}`、`archi::dto::{graph_view, graph_diagnostics_error}`、`archi::display`、`crate::sdk::atomic::write_atomic`、`crate::sdk::protect::ProtectRules`、`crate::sdk::mount::WRITE_LIMITS`、`tools::cap_result`。
- **技术**：入参是裸 `serde_json::Value`（链上无 JSON Schema 校验器），沿 `archi_api` / `db_query` 的 `ok_or_else` 手工解析形态；`ArchiContext` 增 `protect: Option<ProtectRules>` 字段（`with_protect` 构造子），写臂在落点解析后调用 `ProtectRules::evaluate_target`（写保护射程的唯一新消费入口，零第二份规则集）。
- **值域查表单源**：封闭词表查表收成一个文件内私有核 `domain_value(raw, name, all, as_str) -> Result<T, String>`——E1 的 `optional_domain`（`kind` / `gate` / `api` 三处调用）与 `archi_scaffold` 的 `kind` 同调它，判据句只剩一条（`{name} 非法值 "{raw}"（合法值：…）`，合法值清单仍由 `ALL` 现算）；MUST NOT 另持「非法 kind: …」式文案与第二份 `ElementKind::ALL` 查表（D-19）。
- **写面口径单源**：`scaffold_payload` 的函数 doc 与模块头、`SCAFFOLD_TEMPLATE` 首行同口径——落盘走 `archi_element` / `archi_relation`（工具表达不了的才手改 `write` / `edit`）；「写盘仍走既有 `write` / `edit`」式旧句零残留（D-19）。

### C6 · 会话装配面（矩阵）

- **职责**：`(phase, role)` → archi 工具清单的矩阵单源（两写工具只给写权相位）+ archi 来源臂七名。
- **文件**：`crates/infra/agent/src/compose.rs`（改）。
- **依赖**：`mount` 的两新常量。
- **技术**：`archi_matrix` 两行追加；`model_write_lock` 零改动（矩阵与锁面**同集**由测试双向断言，不是放宽锁）。

### C7 · 落盘单点（`write_atomic` 迁移）

- **职责**：「模型与配置两写面共用同一份原子写」——实现迁至 infra 侧，壳层改薄委托。
- **文件**：`crates/infra/agent/src/sdk/atomic.rs`（新）、`crates/infra/agent/src/sdk/mod.rs`（改：一行 `pub(crate) mod atomic;`，见 D-13）、`crates/infra/agent/src/lib.rs`（改：门面导出）、`packages/desktop/src-tauri/src/commands/atomic.rs`（改：薄委托）。
- **依赖**：`std::fs`（`fs::write` / `fs::rename` 唯一位点）、壳层已有 `agent-runtime` 依赖。
- **技术**：签名 / 错误文案 / `.archi-tmp` 后缀 / 失败清临时文件 / 父目录缺失即 `Err` 逐字保留；`sdk` 模块仍私有，外部只经 `agent_runtime::write_atomic`。

### C8 · 知识骨架（四段）

- **职责**：`source = "archi"` 会话的 `preamble`——职责 / 工具面（读五指路 + 写两件 + **手改出口**）/ 留给判断 / 派生路径段；语法类知识（键形 / 字段纪律 / `kind` 四值 / 层级读数）迁出到工具面。
- **文件**：`crates/infra/agent/src/archi-knowledge.md`（改）、`archi_knowledge.rs`（改：模块头注释随动，拼装与降级语义逐字不变）。
- **依赖**：`foundation::layout`（两处派生目录由 `build` 追加）。
- **技术**：静态骨架 `include_str!` + 程序追加段；门限 **骨架 ≤ 1500 B / 全文 ≤ 1700 B**（`lf_bytes(trim_end)` 口径，实测值冻结）；零示例 TOML 围栏、零键形示例、零字段值域行、零结构诊断词。

### C9 · 相位角色知识（prompt）

- **职责**：两个写权相位的写面句由「经 `write` / `edit`（无专用写工具）」改为「`archi_element` / `archi_relation` 写 + 工具表达不了的才手改」，工具行补两写工具；其余 12 份 md 逐字不变。
- **文件**：`crates/core/workflow/src/prompts/proposal-planner.md`、`dev-design-planner.md`（改）。
- **依赖**：无（纯静态资产，编译期 `include_str!`）。
- **技术**：一行替换 + 工具行追加，零字段形态长表、零路径模板。

### C10 · 壳层命令面

- **职责**：`archi_set_gate` 取 `WriteOutcome.write` 落盘（行为零变化）；`archi_module` 出参随 `dto` 投影自动带 `fileHash`（命令体零新增逻辑）。
- **文件**：`packages/desktop/src-tauri/src/commands/archi/mod.rs`（改）。
- **依赖**：`archi::edit::{plan_element_upsert, ElementSpec}`、`crate::commands::atomic::write_atomic`。
- **技术**：仍是「参数转换 → 调用 → 错误映射」三件事；命令清单仍九条、写命令仍两条。

### C11 · 生成物面

- **职责**：`ArchiModuleDetail.fileHash` 的 TypeScript 投影。
- **文件**：`packages/desktop/src/types/generated/bindings.ts`、`bindings.zod.ts`（`bindings:export` 重导，MUST NOT 手改）；`packages/desktop/package.json`（version +1 patch）。
- **依赖**：`crates/tools/export-bindings`（既有）。
- **技术**：specta / specta-zod 生成；diff 恰为 `ArchiModuleDetail` 增一字段（`bindings.ts` 含 JSDoc、`bindings.zod.ts` 的 `ArchiModuleDetailSchema` 同字段）。

## 变更清单 (Change Inventory)

### 新增文件

| 路径 | 说明 |
|---|---|
| `packages/desktop/src-tauri/crates/core/archi/src/edit/warnings.rs` | 七条写面 warning 推导 + 降噪不变式；`EditWarning` 与两个入口函数；kind / severity / 路径全取既有单点；零 `drift::check`、零写盘（签名见「公共函数 / API」，字段面见「类型定义」） |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/atomic.rs` | `write_atomic(path, content)`——模型与配置两写面的**唯一**落盘实现（同目录临时文件 + rename、失败清临时文件）；`.archi-tmp` 后缀与错误文案逐字沿用壳层原实现 |

### 修改文件

| 路径 | 具体修改 | 说明 |
|---|---|---|
| `crates/core/archi/src/edit/mod.rs` | ① 新类型 `WriteAction` / `RelationSpec` / `DeleteSet` / `WriteCounts` / `RelationTally` / `WriteOutcome`；② `ElementSpec` 增 `relations` / `create`（`cascade` 只在 `plan_element_delete` 的值参面，**不入 spec**，D-17）；③ `apply_element_spec` 增内联出边渲染与回读（复用 `write_relation_fields` / `new_relation_table` / `relation_indexes` 等既有私有件）；④ `apply_element_delete(source, &DeleteSet)` 取代单元素形态（元素集 + 关系对集，关系缺席即 `RelationNotFound`；构造子只 `of(fqn)` 一个，零 `is_empty`，D-18）；⑤ `plan_*` 四个返回 `WriteOutcome`，`plan_element_upsert` 增批量四条与跨文件内联边检查、端点存在性前置、落点四情形、新文件表头；`plan_element_delete` 增 `cascade` 闭包与跨文件拒绝；⑥ 反馈面装配（`changed_fields` / `relation_tally` / `counts_of` / `model` 增量 / `warnings` 调用）；⑦ 模块头 doc 重写（三层形状与新增语义） | 既有渲染、自证、三态、表头回填、`expect`、关系落点规则**逐字不动**；`archi` 内仍零写盘、零 `drift::check` |
| `crates/core/archi/src/edit/messages.rs` | `EditErrorKind` 19 → **23**（`RelationDeleteHasFields` / `RelationDuplicateInBatch` / `CascadeCrossFile` / `RelationOwnedElsewhere`）+ `ALL` 数组长度 + `criterion()` 穷尽分支同批扩；既有 19 条判据句逐字不变 | `ALL` ↔ `criterion()` 的 `match` 集合仍是编译期/测试的机械锚 |
| `crates/core/archi/src/lib.rs` | `pub mod edit` 的出口补 `WriteAction` / `RelationSpec` / `DeleteSet` / `WriteOutcome` / `WriteCounts` / `RelationTally`；新增 `pub mod warnings` 子模块出口（`EditWarning` / `upsert_warnings` / `delete_warnings`） | 出口形态沿既有 `pub use edit::{…}` 单行式；crate 根 doc 五条纪律零改动 |
| `crates/core/archi/src/dto.rs` | `ArchiModuleDetail` 末尾追加 `file_hash: String`（`sha256:<hex>`）；`module_detail` 组装补该字段（`drift::file_hash(&root.join(&element.file))`） | 薄概览零改动；命令层零新增计算 |
| `crates/infra/agent/src/sdk/mount.rs` | ① 两常量 `ARCHI_ELEMENT_TOOL = "archi_element"` / `ARCHI_RELATION_TOOL = "archi_relation"`；② `WriteLimits` + `WRITE_LIMITS` 单点常量（八行上限）；③ `archi_element_definition()` / `archi_relation_definition()`（平铺 schema、逐字段 `description`、`enum` 由 `ALL` 派生）；④ 静态清单 16 → **18**（两写项 `ToolTier::Write` / `ToolExec::Archi` 插在 `archi_scaffold` 之后、`db_tables` 之前）；⑤ `Vec::with_capacity(18)` 与 `registry()` / `build_registry()` 两段 doc 的项数措辞、`ToolTier::Read`/`Write` 与 `ToolExec::Archi` 的名单 doc、`archi_scaffold` 常量 doc 的「不在专用写工具之列」句 | 描述面 MUST NOT 复述字段形态长表（形态归 `archi_scaffold` 词典与参数 `description`） |
| `crates/infra/agent/src/sdk/archi_tool.rs` | ① 模块头 doc 由「只读工具执行体」改为「五只读 + 两写」；② `ArchiContext` 增 `protect: Option<ProtectRules>` + `with_protect(self, ProtectRules) -> Self`；③ `execute` 增第六、七臂；④ `element_payload` / `relation_payload`（E1 解析与硬闸 → E2 前置 → `plan_*` → `ProtectRules::evaluate_target` → `write_atomic` → 响应装配）；⑤ 入参解析类型 `ElementWriteInput` / `RelationWriteInput`，值域查表收成私有核 `domain_value`（`optional_domain` 与 `archi_scaffold` 共用，判据句一条，D-19），上限仍取 `WRITE_LIMITS`；⑥ `SCAFFOLD_TEMPLATE` 首行由「写盘仍用既有 write / edit」改为指向两写工具（草稿文本随写面口径）；⑦ `scaffold_payload` 函数 doc 改写为两写工具 canonical（现文「写盘仍走既有 `write` / `edit`」零残留，与模块头、模板首行同口径）；⑧ `ElementWriteInput::spec()` 删 `cascade` 赋值（值参直供 `plan_element_delete`，D-17） | 零自拼 TOML、零第二份判据、零第二份诊断渲染（E2 用 `dto::graph_diagnostics_error`）、输出仍过 `tools::cap_result` |
| `crates/infra/agent/src/sdk/runner.rs` | `mint_archi_context` 的判据由五名扩为**七名**（任一在场即铸）；`session_pump` 在 `ProtectRules` 装配之后接线 `archi.map(|context| context.with_protect(protect.clone()))` | 铸造判据形态（名字清单包含）与调用点单点不变 |
| `crates/infra/agent/src/sdk/tools.rs` | `input_path` 的 archi 名单加两名（返回 `None`——无 `path` 入参）；`pub(crate) use crate::sdk::mount::{…}` 再导出两常量；`input_path` doc 的「五个架构只读工具」措辞随动 | 沙箱改写面零扩宽 |
| `crates/infra/agent/src/compose.rs` | `archi_matrix` 的 `(proposal, executor)` 与 `(dev-design, executor)` 两行追加两写工具；archi 来源臂名字清单五 → 七；模块头 doc 的臂描述随动 | `model_write_lock` 零改动；其余矩阵行逐字不变 |
| `crates/infra/agent/src/sdk/protect/mod.rs` | 新增 `pub(crate) fn evaluate_target(&self, tool: &str, path: &Path) -> Option<String>`（与 `write`/`edit` 路径判定同一 `normalize` → `match_path` → `deny_for` 链，抽成私有 `deny_normalized` 供 `evaluate_path` 复用）；模块 doc 的「射程边界」补一句：写模型工具经 `evaluate_target` 判定、不经 `evaluate` 分派 | 零新规则、零新拒串、零第二份规则集；`evaluate` 三路分派与 `_ => None` 逐字不变 |
| `crates/infra/agent/src/sdk/mod.rs` | 一行 `pub(crate) mod atomic;`（新模块声明） | 见 D-13：提案实现文件表遗漏的机械连带 |
| `crates/infra/agent/src/lib.rs` | `pub use sdk::atomic::write_atomic;`（`sdk` 模块本身仍私有） | 壳层经 `agent_runtime::write_atomic` 引用 |
| `crates/infra/agent/src/archi-knowledge.md` | 六段 2597 B → 四段（职责 / 工具面 / 留给判断 / 派生路径段），骨架 ≤1500 B（草案见 D-10） | 手改出口一句为永久内容；零语法教学段 |
| `crates/infra/agent/src/archi_knowledge.rs` | 模块头 doc 的骨架描述由六段改为四段；`build` / `knowledge` 拼装与降级语义逐字不变 | 派生段两处目录逐字保留 |
| `packages/desktop/src-tauri/src/commands/atomic.rs` | 函数体改为 `agent_runtime::write_atomic(path, content)` 薄委托；签名 / 可见性 / doc 改为「薄委托」 | 零 `fs::write` / 零 `fs::rename` / 零 `create_dir_all`；错误文案与后缀由被委托实现透传 |
| `packages/desktop/src-tauri/src/commands/archi/mod.rs` | `archi_set_gate_inner` 取 `plan_element_upsert(...)?.write` 再 `write_atomic`；模块头 doc 的写面描述随动（写工具与命令面共用同一 `plan_element_upsert`） | `archi_module` 命令体零代码改动（`fileHash` 随投影出线）；签名与错误面逐字不变 |
| `crates/core/workflow/src/prompts/proposal-planner.md` | 工具行补 `archi_element` / `archi_relation`（改为「额外可用 archi 工具：读三件 + 写两件」措辞）；写面句改为「用两写工具写 + 工具表达不了的才手改」；保留「只为已确定元素落笔」与自查句 | 零字段形态长表、零路径模板 |
| `crates/core/workflow/src/prompts/dev-design-planner.md` | 同上（工具行由「四个只读工具」改为「读四件 + 写两件」；自查句仍 `archi_drift` + `archi_module`） | 同上 |
| `packages/desktop/src/types/generated/bindings.ts` | `bindings:export` 重导：`ArchiModuleDetail` 增 `fileHash`（含 JSDoc），其余零 diff | MUST NOT 手改 |
| `packages/desktop/src/types/generated/bindings.zod.ts` | `bindings:export` 重导：`ArchiModuleDetailSchema` 增 `fileHash`，其余零 diff | MUST NOT 手改 |
| `packages/desktop/package.json` | `version` 0.7.14 → 0.7.15（+1 patch） | `src-tauri/Cargo.toml` / `tauri.conf.json` 不跟随 crate 版本行 |

**change 内 spec 面一致性修正（零行为条款变化）**：`specs/desktop-archi-model/spec.md` 的 Module Contract 清单行（`edit/mod.rs` 那行）现文写「`ElementSpec` 增 `relations: Vec<RelationSpec>` / `cascade: bool`」——D-17 删掉 `ElementSpec.cascade` 后该行须同批删去 `/ cascade: bool` 一并列 token（「`cascade` 闭包（子树 ∪ 出入边，跨文件 `Err`）」与 `plan_element_delete(... cascade ...)` 正文条款**逐字保留**）。这是同一交付内的事实性修正（落点在本 change 目录，AC-16 已列的允许面），MUST NOT 顺手改其余 spec 文字，MUST NOT 触碰 `openspec/specs/**` 基线。

### 零改动面（核对结论）

| 对象 | 理由 |
|---|---|
| `.dev-team/architecture/**`（模型 / 契约资产 / 生成物 / 表头计数） | 零数据变化（AC-15）：55 元素 / 129 关系 / 六个表头逐字不动；`api:export` 与 `api:check` 零 diff |
| `core/archi/src/parse/**`、`model.rs`、`graph.rs`、`drift/**`（除 `file_hash` 消费）、`api/**` | 判据面 / 投影面 / 哈希面零变化：写工具 MUST NOT 重实现 `path` / `apis` / `kind × path` 判定（parser 是唯一判据），MUST NOT 跑 `drift::check`；`warnings` 只消费 `api::paths` 与两词汇枚举 |
| `openspec/specs/**` 基线、本 change 的 `explore.md` | delta-only：手改基线即「同一事实两份持有者」；旧立场句与 `write_atomic` 旧落位由本 change 的 delta 在归档合并时替换（AC-12 豁免面 / AC-17 的 `FROM:` 要求） |
| `src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json` | crate 版本行不跟随 `package.json`（版本纪律） |
| `AGENT.md` / `CLAUDE.md` | 规范面，本变更不触 |
| `crates/infra/agent/src/sdk/policy.rs` | 准入判据仍是 `allows(mode, tier)` 单点比较；两写工具只是**以 `Write` 档进清单**，判定逻辑零行改动 |
| `crates/core/config/**`、`infra/{store,exec,vcs,watch}/**`、前端产品代码（除生成物） | 与写面无关；画布 / 契约面板 / 门禁命令面与报告结构零变化 |

### 公共函数 / API

| 标识符 | 所在文件 | 新增 / 修改 | 签名 | 说明 |
|---|---|---|---|---|
| `plan_element_upsert` | `core/archi/src/edit/mod.rs` | 修改 | `pub fn plan_element_upsert(root: &Path, spec: &ElementSpec) -> Result<WriteOutcome, EditError>` | 返回形状扩为 `WriteOutcome`；增批量四条、跨文件内联边、端点前置、落点四情形、新文件表头 |
| `plan_element_delete` | 同上 | 修改 | `pub fn plan_element_delete(root: &Path, fqn: &str, cascade: bool, expect: Option<&str>) -> Result<WriteOutcome, EditError>` | 增 `cascade`（单文件闭包）与跨文件 `Err`；`cascade = false` 两拒绝臂逐字保留 |
| `plan_relation_upsert` | 同上 | 修改 | `pub fn plan_relation_upsert(root: &Path, from: &str, to: &str, desc: Option<Option<&str>>, apis: Option<&[String]>, expect: Option<&str>) -> Result<WriteOutcome, EditError>` | 参数面不变，返回形状扩为 `WriteOutcome`（`warnings` 恒空、`relations` 计一条） |
| `plan_relation_delete` | 同上 | 修改 | `pub fn plan_relation_delete(root: &Path, from: &str, to: &str, expect: Option<&str>) -> Result<WriteOutcome, EditError>` | 同上 |
| `apply_element_spec` | 同上 | 修改 | `pub fn apply_element_spec(source: &str, spec: &ElementSpec) -> Result<String, EditError>` | 同签名的行为扩展：元素字段写回后按 `relations` 批量 upsert / delete 并逐对回读（同一自证链） |
| `apply_element_delete` | 同上 | 修改 | `pub fn apply_element_delete(source: &str, set: &DeleteSet) -> Result<String, EditError>` | 删除纯函数**唯一**形态（元素集 + 关系对集）；单元素删除 = `DeleteSet::of(fqn)` 退化形 |
| `apply_relation_upsert` | 同上 | 不变 | `pub fn apply_relation_upsert(source: &str, from: &str, to: &str, desc: Option<Option<&str>>, apis: Option<&[String]>) -> Result<String, EditError>` | 内联删边与独立删边共用同一判据与文案族 |
| `apply_relation_delete` | 同上 | 不变 | `pub fn apply_relation_delete(source: &str, from: &str, to: &str) -> Result<String, EditError>` | 同上 |
| `upsert_warnings` | `core/archi/src/edit/warnings.rs` | 新增 | `pub fn upsert_warnings(root: &Path, element: &Element, changed: &[String], created: bool) -> Vec<EditWarning>` | 六条 upsert 判据 + 降噪过滤；`element` 取写后文件重解析结果 |
| `delete_warnings` | 同上 | 新增 | `pub fn delete_warnings(root: &Path, fqn: &str) -> Vec<EditWarning>` | 仅第 7 条（孤儿契约 md），恒进场 |
| `module_detail` | `core/archi/src/dto.rs` | 修改 | `pub fn module_detail(root: &Path, fqn: &str) -> Result<ArchiModuleDetail, String>` | 同签名；出参增 `file_hash` |
| `write_atomic` | `infra/agent/src/sdk/atomic.rs` | 新增（迁移） | `pub fn write_atomic(path: &Path, content: &str) -> Result<(), String>` | 唯一落盘点；壳层经 `agent_runtime::write_atomic` 引用 |
| `archi_set_gate_inner` | `src/commands/archi/mod.rs` | 修改 | `pub fn archi_set_gate_inner(root: &Path, element: &str, gate: GateTier) -> Result<(), String>` | 同签名；内部取 `outcome.write` 再落盘 |

> 两写工具是 **agent 工具**，不是 IPC 命令：`execute` 的两条新臂即其全部执行面，命令面仍九条（见「路由 / API 设计」）。

**内部接线面**（`pub(crate)`，非公共 API；仅登记落位与签名，防实现分叉）：

| 标识符 | 所在文件 | 新增 / 修改 | 签名 | 说明 |
|---|---|---|---|---|
| `ArchiContext::new` | `infra/agent/src/sdk/archi_tool.rs` | 不变 | `pub(crate) fn new(root: PathBuf) -> Self` | 读面上下文（`protect = None`） |
| `ArchiContext::with_protect` | 同上 | 新增 | `pub(crate) fn with_protect(self, rules: ProtectRules) -> Self` | 装配侧接线（`session_pump` 一处） |
| `ArchiContext::execute` | 同上 | 修改 | `pub(crate) async fn execute(&self, name: &str, input: &Value) -> Result<String, String>` | 增两写臂；输出仍过 `cap_result` |
| `ProtectRules::evaluate_target` | `infra/agent/src/sdk/protect/mod.rs` | 新增 | `pub(crate) fn evaluate_target(&self, tool: &str, path: &Path) -> Option<String>` | 落点解析后的路径判定（与 `write`/`edit` 同一匹配链、同一拒串族） |
| `domain_value` | `infra/agent/src/sdk/archi_tool.rs` | 新增（文件内私有） | `fn domain_value<T: Copy>(raw: &str, name: &str, all: &[T], as_str: impl Fn(T) -> &'static str) -> Result<T, String>` | 值域查表唯一核：E1 的 `optional_domain`（`kind` / `gate` / `api`）与 `archi_scaffold` 的 `kind` 同调；判据句 `{name} 非法值 "{raw}"（合法值：…）` 单条（D-19） |
| `write_atomic`（壳层） | `src/commands/atomic.rs` | 修改 | `pub(crate) fn write_atomic(path: &Path, content: &str) -> Result<(), String>` | 薄委托（函数体一行转调 infra 实现） |

### 类型定义

| 类型名 | 所在文件 | 新增 / 修改 | 说明 |
|---|---|---|---|
| `WriteAction` | `core/archi/src/edit/mod.rs` | 新增 | `enum { Upsert, Delete }`，`Default = Upsert`；`pub const ALL: [WriteAction; 2]`、`as_str()`（`"upsert"` / `"delete"`）、`parse(raw) -> Option<Self>`。内联边 `action` 与两工具顶层 `action` 共用**一份**二值词表（MUST NOT 手抄字面量） |
| `RelationSpec` | 同上 | 新增 | `struct { to: String, desc: Option<Option<String>>, apis: Option<Vec<String>>, action: WriteAction }`（三态 + 动作）；内联出边的唯一入参形态 |
| `DeleteSet` | 同上 | 新增 | `struct { elements: BTreeSet<String>, relations: BTreeSet<(String, String)> }` + `of(fqn) -> Self`（**只此一个构造子**；零 `is_empty`——无产品读者，D-18） |
| `WriteCounts` | 同上 | 新增 | `struct { elements: usize, relations: usize }`（`counts` 与 `model` 共用一形） |
| `RelationTally` | 同上 | 新增 | `struct { created: usize, updated: usize, deleted: usize, unchanged: usize }` |
| `WriteOutcome` | 同上 | 新增 | `struct { write: ModelWrite, warnings: Vec<EditWarning>, changed: Vec<String>, created: bool, relations: RelationTally, counts: WriteCounts, model: WriteCounts }` |
| `ElementSpec` | 同上 | 修改 | 既有字段 + `relations: Vec<RelationSpec>` / `create: bool`（`Default` 仍成立）。**不带 `cascade`**：级联只作用于删除，唯一承载位是 `plan_element_delete` 的值参（D-17） |
| `EditWarning` | `core/archi/src/edit/warnings.rs` | 新增 | `struct { kind: &'static str, severity: &'static str, message: String }`——kind / severity 存既有单点的 `as_str()` 返回值 |
| `EditErrorKind` | `core/archi/src/edit/messages.rs` | 修改 | 19 → 23 值（+ `RelationDeleteHasFields` / `RelationDuplicateInBatch` / `CascadeCrossFile` / `RelationOwnedElsewhere`），`ALL: [EditErrorKind; 23]` |
| `ArchiModuleDetail` | `core/archi/src/dto.rs` | 修改 | 末尾追加 `file_hash: String`（`sha256:<hex>`）；其余七字段声明序不变 |
| `WriteLimits` | `infra/agent/src/sdk/mount.rs` | 新增 | `struct { fqn, path_items, path_item, metadata_items, metadata_key, metadata_value, relations, relation_apis, relation_api, text, pointer, file: usize }` + `pub(crate) const WRITE_LIMITS: WriteLimits`（256 / 32 / 1024 / 32 / 64 / 512 / 64 / 32 / 128 / 512 / 1024 / 256） |
| `ElementWriteInput` / `RelationWriteInput` | `infra/agent/src/sdk/archi_tool.rs` | 新增 | 两工具的 E1 解析产物（私有）：`ElementWriteInput { action, element, kind, gate, api, entry, path, doc, summary, metadata, relations, expect, file, create, cascade }` / `RelationWriteInput { action, from, to, desc, apis, expect }` |
| `ArchiContext` | 同上 | 修改 | 增私有字段 `protect: Option<ProtectRules>` |

<!-- 配置：本变更不新增 / 不修改任何配置键（无 `config.json` 键、无环境变量、无构建特性），子表整段省略。`WRITE_LIMITS` 是编译期常量，已登记在「类型定义」。 -->

## 数据模型 (Data Model)

### 模型文件（磁盘持久化，形态零变化）

- TOML 结构、键形、字段序、`[[relation]]` 四键、表头计数括号**全部不变**；本变更不写 `.dev-team/architecture/**`（55 元素 / 129 关系 / 六文件表头逐字不变）。
- **唯一新增的写面形态**是「新建模型文件」：`create = true` + `file` 且目标文件缺席时，`plan_element_upsert` 产出的内容以表头行起手——

  ```
  # <file 去 .toml 后缀的字面>（N 元素 / M 关系）
  ```

  `N` / `M` 取**写后重解析结果**（与既有表头回填同一口径）；`<名>` 不剥序号、不猜语义（D-4）。既有文件的表头回填规则逐字不变（首部注释块第一处括号，块外零改动，源无括号不新增）。

### 内存类型（新增 / 变更）

```
WriteOutcome
├─ write:     ModelWrite { file: PathBuf(绝对), content: String, hash: "sha256:<hex>" }
├─ warnings:  Vec<EditWarning { kind, severity, message }>      // 恒在场，空数组 = 无异常
├─ changed:   Vec<String>                                       // 参数面字段名子集
├─ created:   bool                                              // 幂等凭据
├─ relations: RelationTally { created, updated, deleted, unchanged }
├─ counts:    WriteCounts { elements, relations }               // 本文件（写后重解析）
└─ model:     WriteCounts { elements, relations }               // 全模型（前值 ± 增量）
```

- `changed` / `created` / `relations` 的求值口径：`changed` 对比**写前**元素（`created` 时 = 实际给出的字段名）；`relations` 按每对 `to` 的写前状态（缺席 = `created`、存在且 `desc`/`apis` 相同 = `unchanged`、不同 = `updated`、`action = delete` = `deleted`），四项之和恒等于 `relations` 入参长度；`model` 按前值 ± 增量（元素 +1 / 子树规模、关系 +created −deleted）。
- 删除动作的 `relations` 四项恒 `0`（无 `relations` 入参 ⇒ 守恒式仍成立）、`changed = []`、`created = false`；级联删掉的关系条数由 `counts` / `model` 的差值观测（D-7）。

### 线面响应（agent 工具输出）

```jsonc
// archi_element（成功）
{ "warnings": [], "action": "upsert", "element": "Backend.Core.Billing",
  "file": ".dev-team/architecture/graph/model/02-backend-core.toml",
  "hash": "sha256:…", "created": true, "changed": ["kind", "summary"],
  "relations": { "created": 1, "updated": 0, "deleted": 1, "unchanged": 2 },
  "counts": { "elements": 10, "relations": 10 },
  "model":  { "elements": 56, "relations": 129 } }

// archi_relation（成功；薄形）
{ "warnings": [], "action": "upsert", "from": "…", "to": "…",
  "file": "…", "hash": "sha256:…", "created": true, "changed": ["desc"] }
```

- `file` 为 repo 相对 POSIX 串（`archi::display` 单点转换绝对路径）。
- 响应不含元素 / 关系全文、不含协议正文；`warnings` 恒在场但**不设键序断言**（成功响应结构上界 ≈ 2 KB ≪ `MAX_RESULT_BYTES`）。
- `hash` 与 `drift::file_hash` 同函数同前缀，可直接作下一次 `expect`。

### IPC 出线类型

- `ArchiModuleDetail` 增 `fileHash: String`（`sha256:<hex>`）——只可由自身前后比较或直接喂 `expect`，**不可与** `ArchiDriftReport.modelHash`（全资产聚合）比较、不承担「模型是否被改动」判定（那是 `assets_hash`）。

### 持久化

- 无新增持久化：无新表、无新库字段、无新事件类型；模型写盘仍经 `write_atomic` 单点；写工具不落库、不缓存。

## 路由 / API 设计 (Route / API Design)

### IPC 命令面（零新增）

| method（Tauri command） | 入参 | 出参 | 变更 |
|---|---|---|---|
| `archi_module` | `root: string`, `element: string` | `ArchiModuleDetail`（八字段） | 出参增 `fileHash`；命令体零逻辑改动 |
| `archi_set_gate` | `root: string`, `element: string`, `gate: GateTier` | `Result<(), String>` | 内部取 `WriteOutcome.write`；签名 / 错误面 / 失败零写入逐字不变；不暴露 `expect` |
| 其余七个读命令 | 不变 | 不变 | 零改动（命令清单仍九条、写命令仍两条） |

### agent 工具面（新增两条）

> `auth`：不适用——本机进程内 agent 工具与 Tauri IPC 命令均无鉴权面；准入由 peer 两级闸表达（`policy::allows` 档位 + 成员校验），写面另加 `ProtectRules` 保护闸。

| 工具 | 入参（平铺） | 出参 | 错误面 |
|---|---|---|---|
| `archi_element` | `action`（`upsert` / `delete`，必填）；`element`（FQN，必填，≤256）；`kind`（`ElementKind::ALL`，新建必填）；`gate`（`GateTier::ALL`）；`api`（`ApiStatus::ALL`）；`entry`（`string \| null`，≤1024）；`path`（`string[]` ≤32 × ≤1024，`[]` / `null` = 清键）；`doc`（`string \| null`，≤1024）；`summary`（`string \| null`，≤512）；`metadata`（`object<string,string>` ≤32，键 ≤64 / 值 ≤512）；`relations`（`[{ to, desc?, apis?, action? }]` ≤64，`apis` ≤32 × ≤128，同批 `to` 唯一，`delete` 项禁带 `desc`/`apis`）；`expect`（`sha256:<hex>`，可选）；`file`（模型声明目录一级 `*.toml`，≤256）；`create`（bool，缺省 false）；`cascade`（bool，仅 `delete`，缺省 false） | 见「线面响应」 | E1 入参（`Err` 一次报全、点名参数与合法值）/ E2 结构诊断全量形 / E3 语义（`EditError`）/ E4 落点 / E5 删除 / E6 自证 / E7 并发；保护闸拒绝串沿用既有族 |
| `archi_relation` | `action`（`upsert` / `delete`，必填）；`from` / `to`（FQN，必填，≤256）；`desc`（`string \| null`，≤512）；`apis`（`string[]` ≤32 × ≤128）；`expect`（可选） | 薄形（见上） | 同族；关系面 `warnings` 恒空 |

- **档位 / 下发**：两工具 `ToolTier::Write`（`default` 拒绝、`acceptEdits` / `bypassPermissions` 放行），只在 `(proposal, executor)` / `(dev-design, executor)` / `source = "archi"` 三处下发；矩阵外调用由成员校验拒绝（`tool_unavailable` + `is_error`）。
- **落盘链（唯一顺序）**：E1 解析与硬闸 → E2 诊断前置 → `plan_*`（零写盘）→ 解析出的目标文件过 `ProtectRules::evaluate_target` → `atomic::write_atomic` → 响应装配 → `cap_result`。任一前置失败即零落盘。
- **无第三条通路**：不新增 IPC 写命令、无画布写入口；`archi_set_gate` 与写工具共用 `plan_element_upsert`。

## 依赖 (Dependencies)

### 运行时依赖

| 依赖 | 用途 | 变化 |
|---|---|---|
| `toml_edit`（`core/archi` 既有） | 内联出边与元素字段的 AST 改写（键序与 decor 保序） | 零新增 |
| `serde_json`（`core/archi` / `infra/agent` 既有） | 工具响应的 JSON 装配与投影 | 零新增 |
| `foundation::layout`（既有） | 模型声明目录派生 / 知识派生段 | 零新增 |
| `std::fs`（`infra/agent`） | `sdk/atomic.rs` 的临时文件 + rename | 零新 crate（原用点从壳层迁至 infra） |
| `checks::globmatch`（既有） | 写保护 glob 匹配（`ProtectRules` 既有面） | 零新增 |

### 构建 / 测试依赖

| 依赖 | 用途 | 变化 |
|---|---|---|
| `tempfile`（`infra/agent` dev-dependency 既有） | 写工具 / 保护闸 / 原子写的临时目录夹具 | 零新增 |
| `crates/tools/export-bindings`（既有） | `bindings.ts` / `bindings.zod.ts` 重导 | 零新增 |
| `specta` / `specta-zod`（既有） | DTO 出线 | 零新增 |

## 决策留痕

| # | 决策 | 理由 / 边界 |
|---|---|---|
| D-1 | `cascade` 只做**同一模型文件**内的子树 ∪ 出入边闭包；跨文件即 `Err`（`CascadeCrossFile`）点名文件与阻塞项，零落盘 | 一次 `plan` = 一个 `ModelWrite` = 一次原子写；多文件事务无解，「拒绝并点名」优于半成功。spec 与工具描述三处都写「同一模型文件内」，不写「深度优先」 |
| D-2 | `counts` 由「写后交付文本的重解析」取得（`TomlParser::parse(&content)`），`apply_*` 签名不扩 | 计数唯一权威是重解析；`apply_*` 保持「源文本进、文本出」形态，MUST NOT 改行数统计。代价是写面多一次单文件解析（自证链本已解析两次），换来零第二份计数实现 |
| D-3 | 写保护判定挂在**执行体内、`plan_*` 之后**（`ProtectRules::evaluate_target`），不经 loop 的 `evaluate` 分派 | 落点只有 `plan_*` 之后才知道；规则序 / 首命中即拒 / 拒串族全复用，`evaluate` 的 `write`/`edit`/`bash` 三路分派形态逐字不变（无「工具无 `path` ⇒ 不判」的空分支） |
| D-4 | 新建文件表头的 `<名>` = `file` 去 `.toml` 后缀的**字面**（含序号，不剥不猜） | D9：文件名里的语义序号不可推导；「语义部分不猜」在本设计里落为「原样照抄文件名」，零新隐式知识 |
| D-5 | 表头在**渲染完成之后**前置：`apply_*` 交出正文与计数 → 新建文件路径前置 `# <名>（N 元素 / M 关系）\n\n` → 再解析该最终文本自证零诊断 → `hash` 按最终文本算 | 不赌 `toml_edit` 对「只有注释的空源」的前缀 decor 保序；被交付的文本恒等于被自证的文本；仅 `create` 新建文件时调用（既有文件源无括号不新增） |
| D-6 | `EditWarning.kind` / `severity` 以 `&'static str` 承载 `ArchiDriftKind::as_str` / `ArchiApiFindingKind::as_str` / `ArchiApiFindingKind::severity().as_str()` 的返回值 | 词表与档位的持有者仍是既有两个枚举与 `severity()` 单点；写面零新词表、零新档位。`message` 只写「判据句 + 事实 + 下一步」，不重复 kind 字面量 |
| D-7 | 删除动作的 `relations` 四项恒 `0`、`changed = []`、`created = false` | 守恒式「四者之和 == `relations` 入参长度」对删除无条件成立（删除无 `relations` 入参）；级联删掉的关系条数经 `counts` / `model` 差值观测，不为破坏守恒式另立语义 |
| D-8 | `WriteAction` 一个二值枚举同时服务内联边 `action` 与两工具顶层 `action` | 「upsert / delete」是写面同一对动作词；两处共用 `ALL` ⇒ schema `enum` 与 E1 值域检查零手抄字面量 |
| D-9 | `ArchiModuleDetail.file_hash` 追加在**字段声明末尾**（`resolved_entry` 之后） | 生成物按声明序出线；末尾追加使 `bindings/mod_test.rs` 的字段序断言恰为「八字段 + 追加表尾」，与 P3「新键追加表尾」同向 |
| D-10 | 知识骨架草案（约 1458 B，全文 ≈ 1595 B）如下；实现以实测值冻结，超限先删同段冗余，MUST NOT 抬锚 | 四段结构与「手改出口」句为永久内容；语法段（键形 / 字段纪律 / `kind` 四值 / 层级读数）全部迁往工具面。草案：见附录 A |
| D-11 | 写工具的落盘用同步 `write_atomic`（小文件），不进阻塞池 | 与壳层命令面同一函数（「唯一实现」优先）；模型文件体量在数十 KB 量级，单次写不构成泵任务的可感知阻塞 |
| D-12 | E1 校验「一次报全」：收集全部违例后一次 `Err`（`；` 分隔），值域违例点名 `ALL` 派生值，上限违例点名字段与上限 | 沿「第一行自足 / 不猜 / 一次报全」四条 Err 纪律；E1 不做语义判断（批量四条、祖先、端点全在 `plan_*`） |
| D-13 | 变更集为提案实现文件表 **+ `crates/infra/agent/src/sdk/mod.rs` 一行模块声明** | 新增 `sdk/atomic.rs` 必须由父模块声明（Rust 模块规则），而 `desktop-config-agent` 的 Module Contract 又钉住门面行 `pub use sdk::atomic::write_atomic;`——两处取其一只能改 `sdk/mod.rs`。该文件的改动是纯机械连带（`pub(crate) mod atomic;`），不含语义；AC-16 的文件集按「所列文件 + 该一行模块声明」读 |
| D-14 | E2 用 `dto::graph_view` + `dto::graph_diagnostics_error`（全量形）做前置，接受写面多一次全模型 `parse_dir` | 诊断渲染是四消费面共用的单点，为写工具另设聚合即第二份渲染；「一次报全」与 `file:line` 可定位优先于一次解析开销 |
| D-15 | 写工具的 `action = delete` 只取 `element` / `cascade` / `expect`；字段面与 `relations` 在 delete 上不生效（参数 `description` 明写），`cascade = true` 配 `action = "upsert"` 即 E1 `Err` | 顶层动作是显式二值，语义在参数描述里可见；跨字段互斥（`cascade` × `upsert`）按 spec 明文拒绝。内联 `delete` 项带 `desc`/`apis` 的互斥仍按 spec 由引擎拒绝（`RelationDeleteHasFields`） |
| D-16 | `archi_relation` 工具名与 IPC 命令 `archi_relation` 同名 | 沿提案 R-7：两命名空间（agent 工具 vs Tauri 命令）功能不冲突；文档处处以「工具 / 命令」限定。若评审要求改名，回落面 = 两常量 + 两份定义 + spec 两处 |
| D-17 | **`cascade` 只有一个承载位**：`plan_element_delete(root, fqn, cascade, expect)` 的值参。`ElementSpec` **不带** `cascade`（连同 `ElementWriteInput::spec()` 里那一行赋值一并删）；工具顶层 `cascade` 经 `ElementWriteInput.cascade` 直供值参 | 级联只作用于删除，`ElementSpec` 是 upsert 规格——字段在 upsert 路径零读者，且与值参重复即「同一事实两份持有者」。spec 的行为条款（`plan_element_delete(... cascade ...)`、闭包语义）逐字不动；Module Contract 清单行删 `/ cascade: bool` 一并列 token（见「修改文件」表后说明）。**同批修断言口径**（既有锚，不新增测试）：`edit/mod_test.rs` 的 `ElementSpec` 全字面构造删 `cascade: false`、`ElementSpec缺省扩展不破既有简写` 改判「两新字段」（`relations` / `create`）并删 `spec.cascade` 断言；`test-design.md` 的 `ElementSpec` 用例描述同步删 `cascade`（字段数与缺省值表随之改两字段） |
| D-18 | **`DeleteSet` 零 `is_empty`**：只留 `of(fqn)` 一个构造子，集合判空由调用方按公开字段（`elements` / `relations`）直接判 | 全仓唯一读者是测试自己的断言（产品路径不用「空删除集」这一概念：`plan_element_delete` 恒有 `fqn`），留着就是把测试需求写进产品类型。**同批修断言口径**：`edit/mod_test.rs` 的 `单元素删除是集合退化形` 删两处 `is_empty()` 断言，保留 `elements.len()` / `relations.is_empty()` 的字段面断言；`test-design.md` 三处 `DeleteSet` 描述（类型面「`of(fqn)` / `is_empty()`」、退化形用例、「`of` / `is_empty` 两臂」）改为「`of(fqn)` 单构造子 + 公开字段判空」，用例判据不变 |
| D-19 | **值域查表与写面口径各收成一处**：① 私有核 `domain_value` 承载「值域查表 + 判据句」，`optional_domain`（E1 的 `kind` / `gate` / `api`）与 `archi_scaffold` 的 `kind` 同调，判据句只剩 `{name} 非法值 "{raw}"（合法值：…）` 一条（现存两条：`archi_scaffold` 的「非法 kind: …」与 `optional_domain` 的「kind 非法值 …」两者只留后者）；② `scaffold_payload` 函数 doc 改为两写工具 canonical，与模块头、`SCAFFOLD_TEMPLATE` 首行同口径 | 同一判据两种文案 = 第二事实源；`archi_scaffold` 的既有断言只查「四值逐个在场」，改文案不破判据。doc 口径不一致等于把「`write` / `edit` 是唯一写面」的旧立场留在产品源码里（AC-12(a) 精神的落地面）。**同批修断言口径**：无（`archi_scaffold非法kind报错点名四值` 判据不变） |

## 验收标准回应

| AC | 设计回应 |
|---|---|
| AC-1 工具面在册且档位正确 | C4：静态清单 18 项（七内建 + `create_explore_note` + 五只读 + 两写 + 两 DB + `propose_config`），两写项 `ToolTier::Write` / `ToolExec::Archi`，插在 `archi_scaffold` 之后；五只读项档位逐字不变 |
| AC-2 矩阵与写权锁同集 | C6：`archi_matrix` 两行追加、archi 臂七名；`model_write_lock` 零改动；测试双向断言「矩阵含 `archi_element`」⟺「`model_write_lock == None`」 |
| AC-3 落点四情形 | C1：`check_target` 四支（既有 / 省略落父 / 单段 FQN 拒绝 / `file` + `create`）；新文件表头按 D-4 / D-5 |
| AC-4 内联出边四条约束 | C1：`RelationSpec` + 批量四条（同批重复 `to` → `RelationDuplicateInBatch` 带 1 基下标 / `action` / `to`；`delete` 带字段 → `RelationDeleteHasFields`；跨文件 → `RelationOwnedElsewhere` 点名文件并指向 `archi_relation`；未列出不动），四计数守恒 |
| AC-5 删除与级联 | C1 + D-1 + D-17 / D-18：`cascade = false` 两拒绝臂逐字保留；`cascade = true` 单文件闭包删净、跨文件 `CascadeCrossFile` 零落盘；删除纯函数唯一（`apply_element_delete(&DeleteSet)`）；`cascade` 只有值参一个承载位（`ElementSpec` 无此字段）、`DeleteSet` 只有 `of(fqn)` 一个构造子 |
| AC-6 反馈三通道 | C5：`Err`（E1–E7，零落盘）/ `warnings`（恒在场，空数组）/ `changed`（幂等凭据）；E1 文案含「重试：」或「改用 archi_relation」等出路，一次报全（D-12） |
| AC-7 七条 warning 与降噪 | C2：七条判定 + 降噪不变式（只由 `changed` 诱发，`created` 视为全字段，delete 仅第 7 条）；kind / severity / 路径全取既有单点；`edit/` 零 `drift::check` |
| AC-8 响应体量与计数 | C1 + 数据模型：`counts` 取写后该文件重解析（与表头同源）、`model` 取前值 ± 增量；响应结构上界 ≈ 2 KB ≪ `MAX_RESULT_BYTES`，不设键序断言 |
| AC-9 读侧闭环 | C3：`archi_module` 的 `fileHash` = `drift::file_hash(声明文件)`；写工具 `expect` 与响应 `hash` 同函数同前缀；`modelHash` 不可互比（doc + 数据模型两处写明） |
| AC-10 入参面 | C4 + C5 + D-8 / D-12 / D-19：八行上限进 schema（取自 `WRITE_LIMITS`）与执行体 E1 硬闸；`enum` 全由 `ALL` 派生；限额数字单点；值域查表单源（`domain_value` 一处，`archi_scaffold` 与 E1 同文案同清单） |
| AC-11 知识收缩 | C8 + D-10：四段在场、手改出口一句、零示例 TOML / 零键形示例 / 零值域行 / 零结构诊断词；骨架 ≤1500 B、全文 ≤1700 B（草案实测 1458 / 1595） |
| AC-12 旧立场零残留 | C4（两常量 doc 与 scaffold doc 改写）、C9（两 prompt 改写）、C5（`SCAFFOLD_TEMPLATE` 首行 + `scaffold_payload` 函数 doc 改写，D-19）；判据面 = 产品源码 + prompts + delta 承接块，基线不改。产品源码面含「`scaffold_payload` doc 与模块头、模板首行同口径」——旧句「写盘仍走既有 `write` / `edit`」零残留 |
| AC-13 两通路等价与画布不变 | C10 + C1：`archi_set_gate` 与 `archi_element(gate)` 同调 `plan_element_upsert`（同文本）；`write_atomic` 恰一份实现（C7），壳层薄委托 |
| AC-14 保护闸射程 | C5 + D-3：写臂在落点解析后过同一 `ProtectRules`（`evaluate_target`），同一规则序与拒串族；未命中放行 |
| AC-15 生成物与读数 | C11：`bindings.ts` / `bindings.zod.ts` 各增 `fileHash` 一字段、无其它 diff；`.dev-team/architecture/**` 零字节变化（本设计零改动面）；`api:check` 零差异 |
| AC-16 版本与改动集 | C11 + D-13 + D-17 / D-19：`package.json` +1 patch；`src-tauri/Cargo.toml` 不动；改动集 = 提案实现文件表（D-19 两件落在 `archi_tool.rs`，不新增文件）+ `sdk/mod.rs` 一行 + 测试文件（D-17 / D-18 的断言口径修正）+ 本 change 目录（含 `specs/desktop-archi-model/spec.md` 清单行一 token 修正） |
| AC-17 spec delta 形态 | 七份 delta 已在 `specs/` 定稿（本设计只消费其条款）；RENAMED / MODIFIED / ADDED 承接块与 `## Module Contract` 段逐份在案，实现与测试照其条款落地 |

## 待决问题 (Open Questions)

| # | 事项 | 处置建议 |
|---|---|---|
| Q-1 | `EditErrorKind` 四个新变体的判据句最终措辞（草案：`RelationDeleteHasFields` = 「内联删边项带了字段」/ `RelationDuplicateInBatch` = 「同一批 relations 重复声明同一目标」/ `CascadeCrossFile` = 「级联闭包跨模型文件」/ `RelationOwnedElsewhere` = 「内联边声明在别的模型文件」） | 实现时以既有纪律收口：操作中立、逐值非空且互异、零「本操作专属」词；改措辞不需回改本设计 |
| Q-2 | 两工具 `description` 的逐句字数（须含「原子写 / 坏文本不可产出」「`expect` ↔ `hash` 闭环」「`cascade` 只同一文件」三事，且 MUST NOT 复述字段形态长表） | 以 spec 的三条必答项为准，措辞由实现定；超长先删冗余不删三事 |
| Q-3 | 七条 warning 的 `message` 逐条措辞（草案已给「判据句 + 事实 + 下一步」形态） | 实现时保证 ≤200 B 与「不含 kind 字面量」；无机械断言场景下以人工复核为准 |
| Q-4 | 知识骨架最终文本与冻结值（草案 1458 B / 全文 1595 B） | 实现后按 `lf_bytes(trim_end)` 实测并冻结；门限 1500 / 1700 不抬 |
| Q-5 | `sdk/mod.rs` 一行是否被接受为「提案所列文件的机械连带」（D-13） | 已裁决按连带计入；若评审要求严格按提案表，唯一替代是 `#[path]` 挂模块，但那会违反 `desktop-config-agent` 的门面导出行，不建议 |
| Q-6 | `specs/desktop-archi-model/spec.md` 的 Module Contract 清单行需删 `/ cascade: bool` 一并列 token（D-17 的连带）——是否算「同一交付内的事实性修正」而非「spec 重开」 | 按 D-17 处置：只删该 token，闭包语义与 `plan_element_delete(... cascade ...)` 条款逐字保留；若评审坚持 spec 字数冻结，退路是归档后追改——代价是本变更交付时留一处「清单行说 `ElementSpec` 带 `cascade`、代码没有」的不一致，不建议 |

## 附录 A · 知识骨架草案（骨架 ≈1458 B，全文 ≈1595 B）

> 围栏内的 `##` 行是**骨架 md 自身的小节标题**（实现时写入 `archi-knowledge.md`），不是本文档的章节。

```markdown
# 架构模型须知

## 职责
模型只写**结构**（元素 / 依赖）与三个**指针**（`api` / `entry` / `doc`）：接口、导出符号、协议正文归契约资产。

只改架构目录内文件；直改主仓随 `git diff` 评审；写盘后画布**不会自动更新**，提示用户刷新；续接历史会话先取当下模型。

## 工具面
字段面、键形、缺省、值域、祖先与计数全由工具承担——MUST NOT 手拼 TOML：
- 读：`archi_search`（发现）/ `archi_module`（定位）/ `archi_drift`（漂移读数）/ `archi_api`（契约两轴 + 草稿）/ `archi_scaffold`（字段词典）
- 写：`archi_element`（元素 upsert / 删除，可内联出边）/ `archi_relation`（单条关系 upsert / 删除）——参数面即字段清单，坏文本零落盘
- 自查：`archi_drift` 非空即逐条修；`archi_api`（`check`）查两轴；生成物用既有命令重生成，MUST NOT 手改

工具表达不了的（重排 / 批量整理）才手改 TOML，改完必须 `archi_module` 自查——坏模型由读取面报错（`file:line`）。

架构域 `config.json` 管 `ignore` / `exclude`，未知键或缺 `reason` 即非法（`archi_drift` 点名）。

## 留给判断
`kind` 选哪个、`path` 覆盖到哪、`api` 是否 `declared`、元素之间是否真有依赖——没有机械答案；工具的 `kind` / `gate` / `api` 参数面给出合法值与语义；纯分组容器用 `composite`。
```

- 「派生路径段」不在骨架内：由 `archi_knowledge::build` 追加（模型声明目录 + 契约资产目录，逐字保留今日形态）。
- 门限：`lf_bytes(trim_end)` 口径骨架 ≤1500 B、全文 ≤1700 B；草案实测即上表（约 1458 / 1595 B），实现阶段以实测冻结。
