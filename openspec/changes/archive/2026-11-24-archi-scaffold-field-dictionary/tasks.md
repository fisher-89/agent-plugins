# 任务: archi-scaffold-field-dictionary

> **覆盖口径**：覆盖 proposal「变更范围 - 实现文件」**全量 5 行**（`parse/toml.rs`、`parse/mod.rs`、`sdk/archi_tool.rs`、`sdk/mount.rs`、`packages/desktop/package.json`）。proposal「测试文件」3 行**不属本相位、不入本文**——用例名 / needle / 块取法 / 反向锚由 design 末节「交接给下游相位的随动清单」单点持有。
>
> **不在本 change**：`mode = "draft" | "dict"` 双形态；纯 Markdown 词典；按字段查询（`field = "path"`）；`archi_module` 响应附「该元素可改字段」；知识骨架 `archi-knowledge.md` 的「字段纪律」段压缩与相位 prompt 的「取词典」指路句（归主篇切片 A）；`RawElement` / `Element` / `Relation` 字段面任何增删；`DiagnosticKind` / `ArchiApiFindingKind` / `REPORT_VERSION` 任何变化。
>
> **静态步骤归属**：`pnpm check` 链（`server:check` → `bindings:export` → `api:check` → `client:check`）与 `pnpm test` 由桌面端 static-check / test-execution 执行；本文件只登记边界与自查命令。`api:export` / `bindings:export` 属**生成步骤**（产物须入库），按阶段列出。
>
> **阶段收口即可编译**：阶段零零改动（只读核对三处旧文本与契约状态）；阶段一只动 `core/archi/src/parse/{toml,mod}.rs`；阶段二只动 `sdk/archi_tool.rs` 的模板字面；阶段三只动 `sdk/mount.rs` 的三处 doc / 描述与 `package.json` 的版本行；阶段四零代码改动（只跑生成步骤与核对）。每阶段收口时一次提交。
>
> **测试面交接**：本文件不为任何测试文件登记任务。与之相关的跨相位约定只有一条：断言面按 design 末节交接清单的 3 行落地（`toml_test.rs` / `archi_tool_test.rs` / `mount_test.rs`）——其中 `mount_test.rs` 的四枚旧 needle 在本相位落地后会短暂变红，属预期（测试相位同批换 needle，MUST NOT 为「让测试先绿」而回退描述措辞）。
>
> 自查 cwd：仓库根；Rust 侧 `packages/desktop/src-tauri`；生成物与版本 `packages/desktop`。

## 阶段零：起点核对（只读，不改文件）

- [x] 核对 `crates/infra/agent/src/sdk/archi_tool.rs` 的 `SCAFFOLD_TEMPLATE`（`:198`–`:234`）三处待改行**逐字**与 design「修改文件」表 `archi_tool.rs` 行一致（`:211` `entry` 行、`:224` `metadata` 行、`:233` `apis` 行）；不一致即 STOP 回报（模板已漂移，edit 的单点替换会失败）
- [x] 核对 `crates/core/archi/src/parse/toml.rs` 的 `struct RawElement` 块（`:33`–`:59`）恰八字段、声明序 `kind` / `gate` / `api` / `entry` / `path` / `doc` / `summary` / `metadata`，字段行均以 `,` 收尾、`///` 与 `#[serde(default)]` 各自成行（源码自证扫描的前提）
- [x] 核对 `sdk/mount.rs` 的三处描述面（`:59`–`:61` 常量 doc / `:343`–`:344` fn doc 首段 / `:352`–`:354` description）与 `archi_scaffold_definition()` 的参数面（`kind` enum = `ElementKind::ALL.map(as_str)`）
- [x] 核对契约状态与版本：`.dev-team/architecture/graph/INDEX.toml` 的 `Backend.Core.Archi`（`:24`）与 `Backend.Infra.AgentRuntime`（`:87`）均 `status = "absent"`；`packages/desktop/package.json` `version = 0.7.9`；`src-tauri/tauri.conf.json` 仍 `"version": "../package.json"`

## 阶段一：core 字段面清单（`parse/toml.rs` + `parse/mod.rs`）

- [x] `crates/core/archi/src/parse/toml.rs`：在 `struct RawElement`（`:33`）之后、`struct RawRelation`（`:61`）之前新增 `pub const ELEMENT_FIELD_NAMES: [&str; 8] = ["kind", "gate", "api", "entry", "path", "doc", "summary", "metadata"];`（逐项一行或单行均可，项序 = 声明序）
- [x] 同文件为该常量补 doc 三句：① 机器可读副本、顺序 = `RawElement` 声明序；② 权威是 `RawElement` 的字段定义、两者同批维护、`toml_test` 的源码自证对拍同序同集；③ 消费侧（`archi_scaffold` 草稿的字段全集断言）遍历本常量、MUST NOT 手抄字段名字面量；另加一句命名理由（`parse` 命名空间里裸 `FIELD_NAMES` 指代不明）。doc 内 **MUST NOT** 出现 `struct RawElement {` 字面（事实校准 8 的定位锚）
- [x] 同文件其余行逐字不动：`RawElement` / `RawDocument` / `RawRelation` / `to_model` / `validate_apis` / `validate_path` / `classify` / `line_of` / `column_of` / 模块 doc
- [x] `crates/core/archi/src/parse/mod.rs`：`:12` `pub use toml::TomlParser;` → `pub use toml::{TomlParser, ELEMENT_FIELD_NAMES};`（fmt 实序：rustfmt version-sort 把 CamelCase 排在 SHOUTY 前，先例 `sdk/archi_tool.rs:19` 的 `drift_summary, DRIFT_SCOPES`；`cargo fmt --all --check` 零退出）；`pub mod toml;` / `SourceParser` / `parse_dir` / `relative_path` / 两个 `#[cfg(test)] mod` 逐字不变；`crates/core/archi/src/lib.rs` 零改动（不进根 `pub use` 清单）
- [x] 自查 阶段一：`grep -c 'pub const ELEMENT_FIELD_NAMES: \[&str; 8\]' packages/desktop/src-tauri/crates/core/archi/src/parse/toml.rs` = 1；`grep -c 'struct RawElement {' …/parse/toml.rs` = 1（doc 注释不得命中）；`grep -c 'ELEMENT_FIELD_NAMES' …/parse/mod.rs` = 1；`grep -rn 'ELEMENT_FIELD_NAMES' packages/desktop/src-tauri/crates/core/archi/src/lib.rs` 零命中

## 阶段二：词典正文（`sdk/archi_tool.rs` 模板三块五行）

- [x] `crates/infra/agent/src/sdk/archi_tool.rs` 的 `SCAFFOLD_TEMPLATE` 改 `entry` 行（一行内改，不换行）：`# entry：单值，只在约定默认指不到时写（repo 相对 POSIX；单文件根 = 根文件自身 / 目录型根 = index.ts(x) / crate = src/lib.rs）`
- [x] 同模板 `metadata` 行由 1 行扩为 2 行（续行按 design 的对齐：`#` + 11 空格）：首行 `# metadata：选填自由表（字符串 → 字符串）——值 MUST 是字符串（数字 / 布尔等非字符串值被拒）；`，续行 `#           子表与内联表等价（[element."X".metadata] 写在已声明元素之后零诊断；成为唯一声明才缺 kind）`
- [x] 同模板 `apis` 行由 1 行扩为 2 行（续行 `#` + 7 空格）：首行 ``# apis：选填名字清单（可留空 = 不声明 API 粒度）——只写名字（禁签名 / 类型形状 / `::` 限定路径 / 空白）；``，续行 ``#       逐项非空、同名不重复；`apis = []` 与省略同义（显式空数组被拒）；生成物面留空，差额看 archi_drift``
- [x] 同模板其余行逐字不动（`# kind：` 四值语义、`# gate：` / `# api：` / `# path：` / `# doc：` / `# summary：` 的纪律行与全部 `# {f} = …` 示例行、关系块六行）；`scaffold_draft` / `scaffold_payload` / 常量 doc（`:192`–`:197`）/ 模块 doc 逐字不变；零新增字段行、零裸 TOML 行、零新占位符
- [x] 自查 阶段二（只读 grep）：`grep -c '# entry：单值'` = 1；`grep -c '值 MUST 是字符串'` = 1；`grep -c '非字符串值被拒'` = 1；`grep -c '逐项非空'` = 1；`grep -c '同名不重复'` = 1；`grep -c '显式空数组被拒'` = 1；`grep -cF '\`apis = []\` 与省略同义'` = 1；`grep -c '只写名字\|可留空\|生成物面留空\|archi_drift'` ≥ 4；`grep -c 'invalidValue\|unknownField\|missingField\|syntaxError' packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool.rs` = 0；模板区内未注释赋值行仍恰 `kind = "__KIND__"` 一条（`sed -n '198,240p'` 目视或 `awk` 核）

## 阶段三：描述面去枚举 + 版本（`sdk/mount.rs` + `package.json`）

- [x] `crates/infra/agent/src/sdk/mount.rs` `:59`–`:61` `ARCHI_SCAFFOLD_TOOL` 常量 doc 改为：`` `archi_scaffold` 专用只读工具名（新元素 TOML 草稿 = 字段词典：逐字段一行纪律与值域 + 关系四键模板）。执行体同上；**只读零写盘**——写面仍是既有 `write` / `edit`，故本工具不在「专用写工具」之列。``（折行按文件既有宽度，内容逐字；design「落盘文本 C」为准）
- [x] 同文件 `:343`–`:344` `archi_scaffold_definition()` fn doc 首段改为：`` `archi_scaffold` 定义（字段词典：新元素 TOML 草稿，逐字段一行纪律与值域 + 关系四键模板；零写盘）。``（折行按文件既有宽度）；第二段（`kind` 的 enum 由元素种类词表派生 / 草稿过 `TomlParser` 零诊断——「示例 TOML 从知识骨架删除」的自证锚）逐字保留
- [x] 同文件 `:351`–`:355` description 三行改为：`"取新元素 TOML 草稿，同时是字段词典：模型元素字段面逐字段一行纪律与值域 + 关系四键模板，"` / `"只读零写盘——写面仍是既有 write / edit。kind 省略取 domain，值集见 enum。"` / `"草稿可直接抄改（单段占位 FQN，多段时先声明祖先），改完用 archi_drift 自查"`；参数面（`kind` enum 与描述、`required: []`）逐字不变
- [x] `packages/desktop/package.json`：`"version"` `0.7.9` → `0.7.10`（相对落地时 `HEAD` 恰 +1 patch）；其余字段零改动；`src-tauri/tauri.conf.json` 仍 `"version": "../package.json"`；`src-tauri/Cargo.toml` 的 `[package] version` 逐字不变
- [x] 自查 阶段三：`grep -c '字段词典' packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs` = 3（常量 doc / fn doc / description 各一）；`grep -c '逐字段一行纪律与值域'` = 3；`grep -n '字段序\|path 必填提示\|关系模板' …/sdk/mount.rs` **零命中**（注意 `关系四键模板` 与折行后的 `关系四键` + `模板` 均不含连续子串「关系模板」）；`grep -rnF '内联 metadata' packages/desktop/src-tauri --include=*.rs | grep -v '_test.rs'` **零命中**；description 仍含 `kind 省略取 domain` / `只读零写盘` / `write / edit`

## 阶段四：生成步骤与零扩散核对（不重复执行桌面端静态步骤）

- [x] 执行 `pnpm -C packages/desktop run api:export`（生成步骤）：`git status --porcelain -- .dev-team/architecture` **零输出**（`INDEX.toml` / `models/**/*.schema.json` / `.i.rs` / `.i.ts` 零 diff）；随后 `pnpm -C packages/desktop run api:check` 零差异退出
- [x] 执行 `pnpm -C packages/desktop run bindings:export`（生成步骤）：`git diff -- packages/desktop/src/types/generated` **零输出**（常量无 serde / specta；`bindings.ts` / `bindings.zod.ts` 逐字不变）；MUST NOT 手改生成物
- [x] 读数复核（`archi_drift` 恒全量）：`elements` 55 / `relations` 129 / `byGate.off` 55 / 结构诊断 0 / `modelWarnings` 0 / `observations` 53 / `apiFindings` 39（全 `docMissing`）/ `errors` / `warnings` 空 / `status = clean` / `passed = true` / `modelHash` `sha256:6cc50dbf…`；`scan` = 422 scanned / 359 excluded / 422 unmatched / 0 matched / 0 unresolved（零新增 / 删除文件）
- [x] 零扩散核对（`git status --porcelain` 的改动集 ⊆ {本 change 的 `design.md` / `tasks.md`、`crates/core/archi/src/parse/toml.rs`、`crates/core/archi/src/parse/mod.rs`、`crates/infra/agent/src/sdk/archi_tool.rs`、`crates/infra/agent/src/sdk/mount.rs`、`packages/desktop/package.json`} ∪ 测试相位承接面 {`parse/toml_test.rs`、`sdk/archi_tool_test.rs`、`sdk/mount_test.rs`}）：出现 `archi-knowledge.md`、`archi_knowledge_test.rs`、`crates/core/workflow/src/prompts/**`、`.dev-team/architecture/**`、`src-tauri/{Cargo.toml,tauri.conf.json}`、`openspec/specs/**`、`openspec/changes/archive/**`、`packages/desktop/src/**`（含 `types/generated/**`）、`crates/core/archi/tests/**` 任一 diff 即回退
- [x] 验收对照：逐条回应 design「验收标准回应」表（AC-1…AC-9）——AC-1 / AC-4 由阶段二的自查与测试相位断言背书；AC-2 / AC-3 由阶段一实现 + 测试相位断言背书；AC-5 由「零改执行体 + 既有九支不动 + 体量 2213 B / 39 行」背书；AC-6 由阶段三自查；AC-7 由上三条生成步骤与读数复核；AC-8 由阶段三版本核对；AC-9 由 2 份 spec delta 承接（本相位不改写）
