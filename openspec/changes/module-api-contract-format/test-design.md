# 测试设计: module-api-contract-format

> **变更**: module-api-contract-format
> **依据**: `proposal.md`（**AC-1…AC-20**、「变更范围」实现文件表 **27 行** / 测试文件表 **17 行**、风险 R-1…**R-15**）、`design.md`（**第二十九稿**：**D1–D60**，含 **D19-a / D19-b** 两条取数面子裁定、**D32** CLI 的 root 锚定（向上取最近的含架构域祖先、全不命中退化 cwd、零行内路径字面量）、**D33–D42** 十一条收口裁定（路径形态校验 / `--check` 孤儿扫描递归 / `build_schema` 键空间唯一 / `absent` 死数据删除 / fence 重复键文本层判 / `#[path]` 字面量 + 等值真值源 / knip 本期不读 / 透传 `<name>` 与 Rust 模块路径单点 / 形态由调用方传入 / `generated_path` 入参收窄）、**D43–D48** 六条类型与安全面裁定（位置词汇与短形各只有一份 / `apiMissing` 的 op 与 `$defs` 类型同口径 / 契约文本进生成器的两道人字闸 / 渲染器四参单形 + `render_scope` 私有 / `x-function` 落点封闭二处 / 模块段形态闸按语言分列）、**D49–D53** 五条收口裁定（`#[path]` 读盘面限根 / 关键字面 / `occurrences.line` 分列 / `extract` 显式留白 / 机械随动第四类与归属二分）、**D54–D57** 四条（映射规则资产 `common.json` 被渲染器消费 / `api::display` 升 `pub` 并经 `lib.rs` 出口 / 装置超时越界回滚 / 协议键两层口径）、**D58–D60** 三条（`EmitTarget` 收为三字段 / `oneOf` 分支负载单通路并删 `choice_payload` / 删 `pub use check::category_of`）；「变更清单」新增文件 **18 条 / 18 路径** + 修改文件 **22 行 / 24 路径**（清单 **40 行 / 42 路径**，机械随动白名单 **18 路径**、实现相位封闭数 **60 路径**、测试相位 **12 组 / 44 文件**、交付态并集 **104 路径**）；设计补入 **AC-21…AC-34 十四条** 与 **S-1…S-17 十七条**
> **范围**: design「变更清单」的**新增文件 18 条 / 18 路径**与**修改文件 22 行 / 24 路径**（删除文件无——本变更零退役）。**逻辑单元覆盖**：`Cargo.toml` 清单面、`package.json` / `knip.json` 配置面、四类生成物（`.schema.json` / `INDEX.toml` / `.i.rs` / `.i.ts`）、`bindings.ts` + `bindings.zod.ts`、`.dev-team/architecture/{models/05-backend-tools.toml, decisions/0001-model-granularity.md}` 数据资产、`crates/infra/agent/src/sdk/bash.rs` 的**可见性一提**（`resolve_timeout_ms` → `pub(crate)`，唯一产品面改动；六份 `sdk/**` 已移出修改表、`HEAD` 无 rustfmt 债务）、`AGENT.md` + `CLAUDE.md`（AC-18 的对象是两文件）、入口编排文件与 S 项文本面、`api/common.json`（映射规则资产，D54）。**测试文件映射**见下方「测试文件映射口径」段。
> **路径口径**: 全部路径为**项目根相对**。design「变更清单」修改文件的 Rust 简写 `crates/**` 对应 `packages/desktop/src-tauri/crates/**`，`src-tauri/src/**` 对应 `packages/desktop/src-tauri/src/**`；新增文件与前端文件一律写全路径。

## 测试框架与既有布局

- **Rust**：`cargo test --workspace`（本仓 `archi` 系列 crate 同套）；断言仅 `assert!` / `assert_eq!` / `assert_ne!` / `matches!`，零第三方断言库。单测布局为 **sidecar 同目录同前缀** `*_test.rs`，源文件尾以 `#[cfg(test)] mod <name>_test;` 挂载（`crates/core/archi/src/lib.rs` 的七条 sidecar 声明、`crates/core/foundation/src/layout/mod.rs` 的 `mod mod_test;`、`crates/infra/archi/src/{report,runner}_test.rs` 先例）；子目录模块的 sidecar 与被测文件同目录（`crates/core/archi/src/parse/mod_test.rs` / `scan/owner_test.rs` 先例）。crate 级集成测试驻 `tests/`（`crates/core/archi/tests/{vocabulary_test,golden_test,pilot_test}.rs` 先例——`pilot_test` 是试点三方对拍的执行面，design「测试相位产出白名单」的 `archi` crate 级集成用例组）。
- **临时目录装置**：`crates/core/archi` **无 `[dev-dependencies]`**（`Cargo.toml` 现无该段，本变更不新增）→ 自建 `TempDir` RAII + `AtomicU64` 计数（`drift/mod_test.rs` / `dto_test.rs` / `scan/mod_test.rs` 先例）；`crates/infra/archi` 已有 `tempfile = { workspace = true }`（`[dev-dependencies]` 现成）；`crates/tools/archi-api-gen` 现无 `[dev-dependencies]`——按两选一落地：① 引入工作区已收敛的 `tempfile` 作 dev-dependency（design「构建期与工具链依赖」表列它为既有收敛项），或 ② 沿 `archi` 先例自建 `TempDir` RAII（零清单改动）。后者的判据更严（不新增依赖面），本设计不预设取舍。
- **命名**：Rust 测试函数为中文短语 + 断言消息载判决；`describe` 级分组用注释分隔带（沿既有 `// ----` 分隔惯例）。
- **既有两个写模式门禁对本相位的影响（D28 / D29 / D30）**：`server:check` 的 fmt 段已改**校验模式** `cargo fmt --all --check`（D28）、`client:check` 仍为 `vp check --fix`（待决问题 9 明确不动）；因此单测**不得依赖任何「跑测试会顺手格式化源码」的行为**，且生成物断言必须落在**格式化器不动点**上（`.i.rs` = `rustfmt --edition 2021` 的正常形、`.i.ts` = `vp fmt` 的正常形，一律单引号——D29），否则 `api:check` 的字节零 diff 门与写模式前端门永久对撞（事实校准 19）。`pnpm run check` / `vp check` / `knip` / `rustfmt` 等机械步骤由桌面端 test-execution 执行，本设计只给判据，不复制运行脚本细节。
- **待测功能条目与 design「公共函数 / API」的逐行对拍（第三十稿口径：`variant_name` 按纪律①落在私有备案行）**：design 该表**现算 59 行**（`sed -n '/^### 公共函数 \/ API$/,/^### 类型定义$/p' design.md | grep -c '^| `\|^| \*\*`'` ⇒ **59**，其中「工具侧私有备案」行 **1**），本设计的 `#### 待测功能` 按「所在文件」逐行落位为——`layout` **3** / `dto.rs` **2** / `drift/report.rs` **4** / `api/mod.rs` **9**（八行 + **`display`**，D55）/ `api/paths.rs` **15** / `contract.rs` **3** / `schema.rs` **3** / `extract/mod.rs` **5** / `extract/rust.rs` **2** / `extract/ts.rs` **2** / `check.rs` **2** / `emit/mod.rs` **1** / `emit/rust.rs` **2**（`render` / `rust_comment_text`；`variant_name` 是私有 `fn`、落私有备案行）/ `emit/ts.rs` **3**（`render` / `ts_string_literal` / `ts_comment_text`）/ `main.rs` **3**（`main` + `api_gen::repository_root` + `api_gen::run`）= **59**，与表体现算值逐行相等。**另有五类私有备案行**在 `#### 待测功能` 中同样逐行登记、但**不计入 59**：`paths::is_module_segment`（`pub(crate)`，与私有 `const RUST_KEYWORDS` 成组）、`extract::render`（私有 `fn`）、`emit::{rust,ts}::render_scope`（私有 `fn`，本设计在 `emit/ts.rs` 章节登记一次）、`emit::rust::variant_name`（私有 `fn`，行为经 `render` 的 enum 成员断言间接观测）、`extract::Position` 与 `Named` 口径行（design 该二条在「类型定义」面，本设计只在 `extract/mod.rs` 章节作口径行）——五者的行为一律经公共面观测（D43 / D46 / D48 / D50），MUST NOT 为测试放宽可见性。**不入表的两类**：`api::parse_common` / `api::protocol_key` / `emit::{rust,ts}::common_type` 等 `pub(crate)` 或私有工序（D54 / D57 只以判据串点名，见 AC-31 / AC-32），以及 `ApiSymbol` / `EmitTarget` 一类纯类型（归「类型定义」面）。
- **第十二稿至第二十九稿新增裁定的落位（D43–D60）**：**类型词汇与签名短形单一**（D43）落 `api/extract/mod.rs`（`signature_of` / `schema_type` / `Position` / 私有 `render`）与 `api/check.rs`（零本地 `Position`、零本地短形）；**`apiMissing` 的类型口径与取数面缺口**（D44）落 `api/check.rs`（op 与 `$defs` 类型同一次求值）与 `api/extract/rust.rs`（`#[path]` 模块定位 + `pub use <mod>::*` 通配升格）；**两道人字闸**（D45）落 `api/contract.rs`（闸一：七族名字 ASCII 标识符 + 族别与 fence 行点名）与 `api/emit/{rust,ts}.rs`（闸二：`rust_comment_text` / `ts_string_literal` / `ts_comment_text` 三条单点）；**渲染器四参单形**（D46）落 `api/emit/{rust,ts}.rs`（`render` 恰一导出、`render_scope` 私有、零异名孪生）；**`x-function` 落点封闭二处**（D47）落 `api/contract.rs`（`verify` 同趟遍历、pointer 点名）与两侧 `value_type` 零 `x-function` 分支；**模块段形态闸按语言分列**（D48 / D50）落 `api/paths.rs`（`is_module_segment` + `RUST_KEYWORDS` 51 值）与 `api/emit/ts.rs`（specifier 转义零筛选）；**`#[path]` 读盘面限根**（D49）落 `api/extract/rust.rs`（反斜杠归一架在折叠之前 + 分段折叠 + `join` 后 `starts_with(root)`）；**`occurrences.line` 分列**（D51）落 `api/check.rs`（op = heading 行、`$defs` 类型 = `0`）；**`extract::extract` 显式留白**（D52）落 `api/extract/mod.rs` 的两处 doc 与行为面；**机械随动第四类与归属二分**（D53）落 `api/emit/{rust,ts}_test.rs` 的调用形态适配（编译前置，归实现相位）与「断言值 / 用例名」归测试相位。**第二十九稿三条收口裁定的落位**：**`EmitTarget` 三字段**（D58）落 `api/emit/mod.rs`（字段面恰 `kind` / `form` / `modules`，`path` / `element_root` 零残留）与 `api/mod.rs`（`render_all` 的三值构造，`path` / `scope` 只作局部变量），行为面落 `api/emit/{mod,rust,ts}_test.rs` 的同一契约渲染产物**逐字节相等**；**分支负载单通路**（D59）落 `api/emit/rust.rs`（`fn choice_payload` 零残留、`variants_block` 负载一律经 `value_type(choice, Position::Field, common)`），行为面落 `rust_test.rs` 的三类新增带负载正例；**`category_of` 出口面**（D60）落 `api/mod.rs`（`pub use check::category_of` 零命中）与 `api/check_test.rs`（`use super::check::{category_of, check}` 模块路径不变）。D58 / D59 / D60 的**测试面措辞改判已在本设计落地**（`emit/mod.rs` 说明段的字段枚举改三字段、两处「五字段」用例改「三字段面 / 三字段承载」并新增「删两字段零行为影响」、`contract_test.rs` 的 `choice_payload` 注释词面改指 `value_type`）；产品面由 `tasks.md` 阶段三十 ①②③④ 承担（`test-design.md` 三类写面边界的「测试相位产出」一行）。
- **第十稿新增裁定的落位（D37–D42）**：`contract::parse` 的**fence 内重复键文本层判据**（D37）落 `api/contract.rs` 章节；`mod_path_attr` 的**等值真值源**定位（D38）落 `api/paths.rs` 章节（行为断言不变，判据升格为「入口字面量的真值」）；**knip 不接线**（D39）落 `api/extract/ts.rs` 与 `api/mod.rs` 两章节的显式留白断言；`<name>` / `module_path` 的**单点产出**（D40）落 `api/paths.rs`（`module_name` / `rust_module_path`）与 `api/emit/{mod,rust,ts}` 三章节（渲染器零派生）；**形态由调用方传入**（D41）落 `api/emit/mod.rs`（`EmitTarget.form`）与 `api/emit/ts.rs`（零 `entry_form_of`）；`generated_path` **入参收窄为 `<root>` 字符串**（D42）落 `api/paths.rs`。`absent` 字段已按 D36 从 `ApiSnapshot` / `ApiCheckInput` 删除，本设计**零处断言该字段**。
- **测试文件映射口径**：本设计一律按 sidecar 惯例给出「一源文件 → 一测试文件」的映射。proposal「测试文件」表把三处折叠到别处（路径单点函数 → `api/mod_test.rs`；`extract::extract` / `normalize_signature` 未单列；`emit::render_target` 未单列），本设计按 sidecar 惯例拆为 `api/paths_test.rs` / `api/extract/mod_test.rs` / `api/emit/mod_test.rs`——**用例集不因拆分而减少**，proposal 的 `api/mod_test.rs` 章节只承载 `api/mod.rs` 自身**九函数**（`meta` / `enabled` / `load_contracts` / `render_all` / `check` / `bootstrap` / `api_ref` / `api_ref_of` / `display`；D55 升 `pub` 后为九行）。proposal 未列的 `api/extract/{rust,ts}_test.rs` / `api/emit/{rust,ts}_test.rs` / `api/check_test.rs` / `api/schema_test.rs` / `api/contract_test.rs` 与本惯例一致（均沿用 sidecar 同目录同前缀形态，非新建目录）。
- **既有装置的正确性前提（回溯补记，2026-10-31；非新增用例）**：`packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs` 的 `spawn_command_confirmer` 是**既有装置**（本变更零改动该文件），其退出判据 MUST 为「**见过 run 登记后**转 `None`」——装置线程先于 `begin_run` 启动，负载下首拍 `snapshot` 可能是 `None`（主线程尚未登记），以 `None` 直接退出会让应答回路消失、run 永挂确认门直到 `wait_for` 30s 上限。该判据属装置正确性而非本设计的测试用例面，实测与修法见 `reports/test-gen.md`。**D56 的越界回滚（本设计的门禁前提）**：该文件的装置超时被放宽为确认窗 180s + `register_window`、复合用例的收敛等待改为 `wait_for_up_to` 120s——**这属越界**：装置约定是 30s 收敛上限，且该文件与本变更零产品面关联。**owner = 测试相位**（实现相位对它零触碰，`tasks.md` 阶段二十九 ⑤ / AC-33 ③）：判据 = `grep -cF 'from_secs(180)'` **0**、`grep -cF 'register_window'` **0**、`wait_for` 回到 30s 约定上限、装置退出判据「见过 run 登记后转 `None`」保留；`git diff HEAD` 对该文件**只余测试相位认可的最小改动**（否则整文件回到 `HEAD`）。该回滚**已完成**（`git diff --stat HEAD` 对该文件为空），本节登记其归属以免被误读为本变更新增用例。
- **跨模块组合用例挂靠**：`load_contracts → contract::parse → schema::build_schema → verify → verify_defs` 整链挂 `api/mod.rs`；`enabled + load_contracts + check` 的装配链挂 `crates/core/archi/src/drift/mod.rs`（判定单点的最上层调用方）；四面词汇对拍挂 `crates/core/archi/src/dto.rs`（词汇单点，实际执行文件为 `tests/vocabulary_test.rs`）；判定语料 golden 挂 `crates/core/archi/src/drift/mod.rs`（快照目录 `tests/corpus/drift/expected/`，实际执行文件为 `tests/golden_test.rs`）；`archi_module` 出参面挂 `packages/desktop/src-tauri/src/commands/archi/mod.rs`（IPC 投影入口）；**试点元素的「契约 ↔ 生成物 ↔ 手写实现」三方对拍**（md 的 `$defs` / `x-function` ↔ `.i.rs` 的 `pub struct` / `signature witness` ↔ `runner.rs` 的 `impl` / `LocalArchiCheck::file_stem`）挂 `crates/infra/archi/src/runner.rs`（链路发起方 = 试点元素的实现入口，与「per-file 章节承载该源文件的用例」同一落点）；其**跨 crate 执行面**是 crate 级集成测试 `crates/core/archi/tests/pilot_test.rs`（design「测试相位产出白名单」的 `archi` crate 级集成用例组，本设计不另立章节——按「不新建独立集成测试章节」纪律，其用例面由 `runner.rs` 章节承载、执行落 `pilot_test`）。**D58 ⑤ 的构造形态适配**：`pilot_test.rs` 内 **1 处** `EmitTarget { … }` 字面量的两字段删除属**实现相位**（D53 第四类机械随动，`tasks.md` 阶段三十 ④ 的 19 处之一），本节不重复登记。
- **机械随动第四类的归属二分对本设计的约束（D53 / AC-30 ④，回溯补记）**：归**实现相位**的只有「`use` 行 / 实参补齐（元数）/ 装置同名提取」三种**编译前置**形态；归**测试相位**的是**五处**——① `api/emit/rust_test.rs` 的签名自证串、② `api/emit/mod_test.rs` 的「职责三件」清单、③④ `api/mod_test.rs` 的两处 `pub use …::AgentView;` 断言改写、⑤ `api/emit/ts_test.rs` 的六处用例名去 `_with_root`。**判据形态**：逐项问「撤掉该 `hunk` 后 `cargo clippy --workspace --all-targets` 是否仍绿」——仍绿即不在第四类射程内（越界）。本节因此把下列**行为断言**一并纳入测试相位写面：`api/mod.rs` 章节的 `render_all` 查表筛选（D48 / D50 的 Rust 分支拒收面）、`api/emit/rust.rs` 的透传 `pub use` 行、`runner_test` 的两条「再导出」否定断言——它们的产品面已在实现相位收口（`runner.rs` 现为 `use crate::LocalArchiCheck;` 普通 `use`、零 `pub use`），故断言串 MUST 与产品面**协同**改写，MUST NOT 保留只在特定排版下才成立的空转形态（见 `runner.rs -> runner_test.rs` 章节与「不可测试项」对应行）。

### 待测功能条目来源约定

`#### 待测功能` 只登记 design「公共函数 / API」子表中「所在文件」为该源文件的行（原文签名 + 出处标注「（公共函数 / API）」）。源文件在该子表**无行**时，条目改列 design 的**类型定义面**（「类型定义」表的该文件行）或**修改面**（「修改文件」表的该文件行），仍用 `- 标识符: 描述` 无序列表，并在条目后保留 HTML 注释说明「design.md 未声明该文件的公共 API 变更」及其条目来源；紧随的 `> 说明` 行给出 D 编号 / AC 挂靠。**不由 grep 源码导出声明反推**、不虚构条目。

### 参数类型 → 边界覆盖口径

| 类型 | 覆盖要求 | 本变更加载点（按 API 入参 / 字段） |
|---|---|---|
| int / number | `0`、MAX、`None` + 1 正常（`-1` 在 `u32` / `usize` 类型层不可表达 → 以 `0` 与越界表达） | `ContractOp.line` / `ContractSection.line` / `ApiSymbol.line` / `IndexEntry.{types,ops,degraded}`（`0` = 空契约 / 位置未知；MAX = 无饱和）、`version: u32`（`2` 档内）、`REPORT_VERSION`（常量断言）、**`occurrences[].line`（D51 分列：op 缺口取 heading 行 ≥ 1、`$defs` 类型缺口取 `0`）** |
| str / string | `""`、超长（>1000 chars）、特殊字符（`\n` / `\0` / emoji）、`None` + 1 正常 | `fqn`（四段 / 单段 / 空串 / 含 emoji）、`root` 形参（`""` / `"   "` / 不存在 / 非 ASCII 路径）、**`cwd` 形态（`repository_root` 的起点：仓根 / `packages/desktop` 子目录 / 四层深子目录 / 无架构域树 / 相对路径起手）**、`file`（`/` 与 `\` 混用、无扩展名）、`op_id`（`is_constructor` 边界）、`timestamp`、`$ref` 串、`category`（六值 + 拼错）、`message`、`common_digest`（恒 12 位十六进制）、`prose` 列表项；**`common.json` 文本（`parse_common` 的入参：非法 JSON / 顶层非对象 / 缺槽位 / 槽位内零 `{}`，D54）**、**`protocol_key` 的入参（`Outcome` / 已小写 `outcome` / 空串 / 超长协议名，D57）**、**`entry`（`extract_element` / `extract_crate` 的入口串：空串 / 不存在 / 目录占位）**、**`element_root`（D42 后 `paths::generated_path` 与 `paths::module_name` / `paths::rust_module_path` 的形参：空串 / 目录形态 / 单文件形态 / 含 emoji / 超长 / `..` 段 / 绝对路径）** |
| bool | `true`、`false`、`None` | `ApiCheckInput.enabled`（两值）、`ApiParam.optional`（两值）、`ApiSymbol.is_async`（两值）、`Contract` 的 op 是否带 fence（在场 / 缺 fence） |
| list / array | `[]`、单元素、超大、`None` + 1 正常 | `elements: &[ArchiElement]`（`[]` / 1 / 300）、`contracts` / `symbols`（各 `[]` / 单 / 超大；**`absent` 已按 D36 删除，零断言**）、`ops` / `reexport_prose` / `occurrences`、`GeneratedFile` 列表（`[]` / 4 类 / 1000 条）、`IndexEntry` 列表（`[]` / 单 / 1000）、`Visibility::ALL` / `ApiStatus::ALL` / `ArchiApiFindingKind::ALL` / `ApiFindingSeverity::ALL`（长度断言） |
| dict / object | `{}`、缺失必填字段、多余字段、`None` + 1 正常 | `$defs`（`{}` / 单 / 1000 键 / 重复键 / 嵌套 `$defs`）、`properties`（零 op 零类型形态）、`api-op` fence 内容（缺 `params` / 缺 `returns` / 多余键）、knip 报告 JSON（`None` / `{}` / 结构异常）、`IndexEntry.categories`（`{}` 整段省略 / 单键 / 六形状 / 六值外开放键 / 含 emoji 键）、**`EmitTarget`（D58 三字段 `{ kind, form, modules }`：`modules` 为 `{}` 空表 / 单键 / 超大表；`path` / `element_root` 已删 ⇒ 零输入面）** |
| Optional[T] | `None` | `generated_path`（`element_root` 形态非法 → `None`）、`entry_form`（四分支全不命中 / `path` 空 → `None`）、`domain_of`（单段 FQN）、`ArchiApiRef` 三路径（`none` 时全 `None`）、`apply_knip_report` 的 `Option<&Value>`、`EmitTarget.modules` 查不到目标 FQN（→ 不产 specifier，D40）、**`parse_common` 的可选槽位 `degraded` / `note`（D54：缺省即 `None`）**、`IndexEntry.generated_rel`、`ApiSymbol.owner`（自由函数） |
| Enum | 每个枚举值 + 非法枚举值（N+1） | `ApiStatus`(2)、`ApiFindingSeverity`(2)、`ArchiApiFindingKind`(10)、`ApiFormStatus`(2)、`ApiSymbolKind`(4)、`Visibility`(4 **词表值**；但**生产面可判值域按语言分列**——Rust 四值可判（`External` / `CrateOnly` / `TestOnly` + 调用方降级 `Unused`）；**TS 生产面只产 `External`**（`Unused` 的唯一产出点在被零调用的 `apply_knip_report` 内、接口测得到但生产面不可达，D39），`CrateOnly` / `TestOnly` 本期留白，D19-b)、`EntryForm`(3)、`GeneratedKind`(2)、`Language`(3)、`ApiType`(11)、`ApiError`(3)；**`ApiFindingSeverity` 与 `GateTier` 词面相同但类型分立（D14：无 `From` 实现，两枚举各自 `match` 穷尽）**；非法值以 serde 反序列化 `Err` / `as_str` 词汇外退化表达；**`protocol_key` 的键换算不是枚举面**（`Outcome` → `outcome` 是字符串变换，D57） |
| float | `0.0`、`-0.0`、`NaN`、`Inf`、`None` | **契约面零 float**：`$defs` 内的数值关键字由 `serde_json::Value` 原样透传，无 `f64` 判定面（透传即正确）。 |

---

## 验收范围

| AC ID | 验收条件 | 被测文件或模块 |
|---|---|---|
| AC-1 | 资产形态闭环：`<FQN>.md` 唯一手写真相 + H1 锚点 + 散文区 + 三类 fence；双向 gate 三条任一处不成立即契约错误并点名位置；**fence 内对象层级重复键在 `parse` 的文本层即报错**（D37：`serde_json` 折叠后不可达，`verify_defs` 只判折叠后的精确比较） | `packages/desktop/src-tauri/crates/core/archi/src/api/contract.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/schema.rs`（`verify_defs` 的零副作用面） |
| AC-2 | `schema.json` 纯派生不可手改：六字段逐项有 md 出处 + 头部警示；`INDEX.toml` 的 `category` 分布为 `$defs` **形状**分布（`categories` 六键开放值集、`sum == types`、`absent` 整段省略，D26）；手改已提交 → `api:check` 持久失败并列差异文件；手改未提交 → 下次 `generate` 覆盖 | `packages/desktop/src-tauri/crates/core/archi/src/api/schema.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs`（私有 `def_shape` 打标）、`packages/desktop/src-tauri/crates/tools/archi-api-gen/src/main.rs` |
| AC-3 | 引用与命名五规则：身份 = `$defs` 键名 / 嵌套提升原名保留 / `$ref` 只能指顶层 / 同类型只定义一次 / 禁按结构去重；**`build_schema` 的两处键空间各自唯一（`$defs` 内 `api-types` 与 `api-reexport` 不同名、`properties` 内 op 键与类型键不同名），撞名即 `ApiError::Contract` 而非静默覆盖**（D35） | `packages/desktop/src-tauri/crates/core/archi/src/api/schema.rs` |
| AC-4 | 生成物落点与入口查找：`.i.rs` / `.i.ts` 与 `<root>` 同级；`.i` 剥离四形态全对；`#[path]` 的值为**字面量**且 MUST 与 `paths::mod_path_attr` 输出逐字相等（D38），TS import 由 `paths::ts_import_specifier` 单点产出；**`<name>` / `module_path` 各只有一个判据**（`paths::module_name` / `paths::rust_module_path`，渲染器零二次派生，D40）；**形态由调用方 `paths::entry_form` 求出并传入**（D41）；`generated_path` 只收 `<root>` 字符串（D42）；Rust 三分支 / 前端四分支封闭（`<dir>/src/lib.rs` 14 命中 / 其余 0）；冲突报 E0255 | `packages/desktop/src-tauri/crates/core/archi/src/api/paths.rs`（含 `directory` / `index_path` 两个目录派生单点、`generated_kind` 语言分派单点、`module_name` / `rust_module_path` 透传两值单点）、`packages/desktop/src-tauri/crates/core/archi/src/api/emit/mod.rs`（`EmitTarget.form` / `modules`，D40 / D41）、`packages/desktop/src-tauri/crates/core/archi/src/api/emit/ts.rs`（零 `entry_form_of`）、`packages/desktop/src-tauri/crates/core/archi/src/scan/owner.rs`、`packages/desktop/src-tauri/crates/core/foundation/src/layout/mod.rs` |
| AC-5 | 类型映射两侧对等：七种协议在 `.i.rs` / `.i.ts` 各出正确形态；构造器由生成器产出且不在契约；位置分派为内部表；泛型零支持；`x-class` 两判据分居（错误进 `schema::verify_defs`、`properties` 全为函数未标的警告进 `emit::render_target` 的 stderr、不进判据十值） | `packages/desktop/src-tauri/crates/core/archi/src/api/emit/mod.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/emit/rust.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/emit/ts.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/extract/mod.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/schema.rs`（`x-class` 错误判据）、`packages/desktop/src-tauri/crates/core/archi/src/api/common.json`（映射规则资产，摘要经 emit 头注释断言）、`packages/desktop/src-tauri/crates/core/archi/src/api/check.rs` |
| AC-6 | TS 形状校验闭环：实现侧 `import type` + 标注返回类型 → `tsc` 报 TS2322；`as const` + `type` 同名共存；`knip.json` 只加一条 `src/**/*.i.ts` ignore | `packages/desktop/src-tauri/crates/core/archi/src/api/emit/ts.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/extract/ts.rs`、`packages/desktop/knip.json`（配置面，见「不可测试项」） |
| AC-7 | 十条判据与期望数逐项对拍（`apiEntryMissing` 28 / `apiMissing` 13 / 其余 0 / `apiUndeclared` 不适用）；多于期望 = 假阳、少于 = 假阴；`apiFindings` 不影响 `status` / `passed`；**语料落点唯一驱动面** = 既有 `DRIFT_CASES` + `corpus/drift/` + 十一份 `api/**` sidecar 用例（新增 `api-mismatch` 新 case 与三件 fixture、既有 15 份金样重写、登记面三形态失配皆红），**MUST NOT 另建 `tests/corpus/api/`** | `packages/desktop/src-tauri/crates/core/archi/src/api/check.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs`、`packages/desktop/src-tauri/crates/core/archi/src/drift/mod.rs`、`packages/desktop/src-tauri/crates/core/archi/tests/corpus/drift/{api-mismatch/**,expected/**}`（执行于 `tests/golden_test.rs`） |
| AC-8 | 判据词汇与报告槽位：`ArchiApiFindingKind` 十值 + `ALL` + `as_str` 双向对拍；`category` 开放值集；既有 14 字段逐字不变 + `apiFindings` + `version = 2`；金样仅追加 `"apiFindings": []` + 至少一个非空金样 | `packages/desktop/src-tauri/crates/core/archi/src/drift/report.rs`、`packages/desktop/src-tauri/crates/core/archi/src/dto.rs`、`packages/desktop/src-tauri/crates/core/archi/src/drift/mod.rs`、`packages/desktop/src-tauri/crates/core/archi/tests/corpus/drift/expected/`（执行于 `tests/golden_test.rs`）、`packages/desktop/src-tauri/crates/core/archi/tests/vocabulary_test.rs` |
| AC-9 | 判定单源：`archi::api::check` 是十判据唯一实现；命令面与门禁面同输入产出逐字段相等；`$defs` 只读 `schema.json`；代码导出集只来自 `extract::extract_element` 一处（第六稿标为「判据唯一取数面」） | `packages/desktop/src-tauri/crates/core/archi/src/api/check.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/extract/mod.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/extract/rust.rs`、`packages/desktop/src-tauri/crates/core/archi/src/drift/mod.rs`、`packages/desktop/src-tauri/src/commands/archi/mod.rs`、`packages/desktop/src-tauri/crates/core/archi/src/lib.rs`（出口 re-export，见「不可测试项」） |
| AC-10 | `api` 元素字段：封闭二值 + 缺省 `declared`；`api = "non"` 报 `invalidValue` 并列全合法值；`archi_module` 单元素响应含 `api` 并列字段；容器元素 `path` 空即天然无契约 | `packages/desktop/src-tauri/crates/core/archi/src/dto.rs`、`packages/desktop/src-tauri/crates/core/archi/src/model.rs`、`packages/desktop/src-tauri/crates/core/archi/src/parse/toml.rs`、`packages/desktop/src-tauri/src/commands/archi/mod.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/archi_knowledge.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/archi-knowledge.md` |
| AC-11 | 嵌套与透传：选择性透传在案；`api-reexport` 键集 ≡ `$defs` 透传键集；透传条目零结构关键字；父子 `$defs` 交集为空；非直接子元素引用 → `apiRefMissing`；跨 crate 父子 → `apiNestedCrossCrate`；**两侧渲染器的透传目标值只查表、零派生**（TS 读 `ModuleRef.name`、Rust 读 `ModuleRef.module_path`，D40） | `packages/desktop/src-tauri/crates/core/archi/src/api/check.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/schema.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/contract.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/emit/mod.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/emit/rust.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/emit/ts.rs` |
| AC-12 | 生成器工程面：`generate` 写四类且幂等；`--check` 零写盘 + 非零退出并列文件（**孤儿扫描全树递归、跳过构建产物目录、有深度上限**，D34——只扫顶层会让两层 / 三层落点的孤儿对门禁不可见）；`bootstrap` 只出 stdout 且**不读 knip**（D39）；`archi` 零写盘零 spawn；**root 由 D32 锚定**（向上取最近的含架构域祖先、全不命中退化为 cwd、零行内路径字面量 ⇒ 自 `packages/desktop` 起手调用也判同一个仓，`api:check` 零 diff 才可能成立）；**写盘面双层拦截**（`paths` 的形态校验 + `write` 的 `starts_with(root)` 复核，D33）；依赖面恰 `archi` + `foundation`（外部仅 `serde_json`）；**生成物须是格式化器不动点**（`.i.rs` = `rustfmt --edition 2021` 正常形、`.i.ts` = `vp fmt` 正常形且一律单引号，D29——否则字节零 diff 门与写模式前端门永久对撞） | `packages/desktop/src-tauri/crates/tools/archi-api-gen/src/main.rs`（含 `api_gen::repository_root` 根锚定单点、`write` 的落点复核）、`packages/desktop/src-tauri/crates/tools/archi-api-gen/Cargo.toml`（清单面，见「不可测试项」）、`packages/desktop/src-tauri/Cargo.toml` + `Cargo.lock`（清单面 / 构建产物，见「不可测试项」）、`packages/desktop/src-tauri/crates/core/archi/src/api/extract/rust.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/emit/rust.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/emit/ts.rs` |
| AC-13 | 试点元素端到端：md + `schema.json` + `.i.rs` 三者落地；`### new` 不存在；生成物与手写 `impl Default` 共存编译；`archi_module` 该元素 `api.status = declared`；判据零命中 | `packages/desktop/src-tauri/crates/infra/archi/src/runner.rs`、`packages/desktop/src-tauri/crates/infra/archi/src/report.rs`、`packages/desktop/src-tauri/crates/infra/archi/src/lib.rs`（入口编排，见「不可测试项」）、`packages/desktop/src-tauri/crates/infra/archi.i.rs`（生成物，见「不可测试项」）、`.dev-team/architecture/api/**`（数据资产，见「不可测试项」） |
| AC-14 | 交付面：`bindings:export` 重导出后零 diff（`scripts` 内只有 `bindings:export` / `bindings:zod-check`，无 `bindings:check` 键，事实校准 24）；`api:check` 零 diff；`version` = **当前 `HEAD` 值 + 1 patch**（判据是相对递增，MUST NOT 写死字面号，事实校准 25；`src-tauri/Cargo.toml` 不随动）；`vp check` / `knip` / `clippy -D warnings` 通过；**`server:check` 的 fmt 段为校验模式**（`cargo fmt --all --check` exit 0，D28），交付态下 `pnpm run check` 连跑两次后 `git status --porcelain` 逐行相等（树处于不动点） | `packages/desktop/package.json`（配置面，见「不可测试项」）、`packages/desktop/src/types/generated/bindings.ts` + `bindings.zod.ts`（生成物，见「不可测试项」）、`packages/desktop/knip.json`（配置面，见「不可测试项」） |
| AC-15 | 模型与新增 crate 同 change 同步：`05` 段 3 元素 / 3 关系（2 条出边逐条在案）；全基线 51 / 113（后端 49 + 前端 64）；自研 crate 16 ↔ 16；三处计数同值；`archi_graph` 零诊断零告警 | `.dev-team/architecture/models/05-backend-tools.toml`、`.dev-team/architecture/decisions/0001-model-granularity.md`（数据资产，核对以「命令 + 留痕」执行，见「不可测试项」；口径 D24） |
| AC-16 | 零债务消解副作用：不为消债建 barrel / 改元素根路径 / 挪类型；前端 `path` 写法逐字未变；**契约相关**代码形状改动限试点元素可见性提升与两处定义移除；**并列允许项零条**（第二十九稿口径：六份 `crates/infra/agent/src/sdk/**` 已移出修改表——`HEAD` 无 rustfmt 债务、实测 6/6 干净，清偿是空动作；`package.json` 行尾实测零处不等，不构成允许项）；**另注**：第四类机械随动（被删符号的调用形态适配，D53）的落点已在测试相位的 44 文件集内计数，**不增白名单项、不改 60 / 44 / 104**；「变更清单 ∪ 机械随动白名单」外文件零改动（封闭数按 design「变更清单」核对段**现行口径**：清单 **40 行 / 42 路径** + 白名单 **18 路径** = 实现相位封闭数 **60 路径**；测试相位 **12 组 / 44 文件**；交付态并集 **104 路径** = 60 ∪ 44） | `packages/desktop/src-tauri/crates/infra/archi/src/runner.rs`、`packages/desktop/src-tauri/crates/infra/archi/src/report.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/bash.rs`（`resolve_timeout_ms` 升 `pub(crate)`，唯一产品面改动，见「单元测试」`bash.rs -> bash_test.rs` 章节）、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/{context,loop,loop_test,resume,runner_test,tools_test}.rs`（**已移出修改表**，`HEAD` 无 rustfmt 债务，零改动，见「不可测试项」）、`AGENT.md` + `CLAUDE.md`（仓库面文档，AC-18 的对象是两文件，见「不可测试项」） |
| AC-17 | 语言无关纪律：契约格式与判据面零 `crate` / `Cargo.toml` / Rust 专有概念；`<FQN>.md` / `.schema.json` / `INDEX.toml` 内零 `crate` 词 | `packages/desktop/src-tauri/crates/core/archi/src/api/schema.rs`（`build_index` 输出）、`packages/desktop/src-tauri/crates/core/archi/src/api/emit/rust.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/emit/ts.rs`、`.dev-team/architecture/api/**`（生成物审查，见「不可测试项」） |
| AC-18 | `AGENT.md` / `CLAUDE.md` 契约纪律节：两文件各恰一节 `## 模块 API 契约`、节内五条要点（手写真相位置 + 三类 fence / 四类生成物勿手改 / `api:export`·`api:check` 两脚本与 `check` 链四段 / `api = none` 留白语义 / `apiFindings` 不进 `status` 且指向 `INDEX.toml`）、节内正文两文件**逐字相同**、MUST NOT 复制契约正文 / 判据长表（含 `ArchiApiFindingKind` 十值）/ fenced json 示例、节内零 `openspec/specs/**` 路径；判据 = `grep -c '^## 模块 API 契约'` 各 1、`grep -c 'apiEntryMissing'` 各 0、`diff AGENT.md CLAUDE.md` 恰 2 处差异且全在节外 | `AGENT.md`、`CLAUDE.md`（仓库面文档，纯文本项，无单测路径——见「不可测试项」） |
| AC-19 | `foundation::layout` 派生函数：`API_DIR_NAME` 常量 + `archi_api_dir(root)` + `archi_api_rel_dir()` 全从 `archi_dir` / `archi_rel_dir` 派生（逐字等于 `archi_dir(root).join(API_DIR_NAME)` / `archi_rel_dir() + "/" + API_DIR_NAME`）、纯推导零 IO、段名字面量 `"api"` 只在 `API_DIR_NAME` 一处、与 `archi_models_dir` / `archi_decisions_dir` / `archi_config_path` 互不为前缀；四条用例 + 四条机械 `grep` | `packages/desktop/src-tauri/crates/core/foundation/src/layout/mod.rs`、`packages/desktop/src-tauri/crates/core/foundation/src/layout/mod_test.rs` |
| AC-20 | bootstrap 取数面：AST 归一签名与 `visibility` 剪枝——① `normalize_signature` 单点定义恰 1 处、`check.rs` 逐字消费恰 1 处、对语言取数器直连零命中；② Rust 侧走 `syn::parse_file`（`regex` 零命中，多行签名全取）；③ `Visibility` 四值词表封闭（`ALL: [Visibility; 4]` + `as_str` 四词各 1）；④ 可判值域按语言分列（`rust.rs` 产 `External` / `CrateOnly` / `TestOnly`，`Unused` 零产出；`ts.rs` **生产面只产 `External`**（`Unused` 的唯一产出点在被零调用的 `apply_knip_report` 内 ⇒ 生产面不可达，**显式留白**，D39 / S-12），`CrateOnly` / `TestOnly` **留白**）；⑤ TS 取数面 = 导出面（`Statement::ExportDeclaration` 恰 1 条分支、六个非导出顶层分支 / `ArrowFunctionExpression` / `ExportDefault` 全零命中）；⑥ 不猜不用伪判据（`Language::Unknown => Vec::new()` 恰 2 处、`used_by` 零命中、`.test.` / `.spec.` / `__tests__` / `fixtures` 文件名族零非注释命中） | `packages/desktop/src-tauri/crates/core/archi/src/api/extract/mod.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/extract/rust.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/extract/ts.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/check.rs`（归一签名单点消费面） |
| D33–D42 收口裁定（第十 / 十二稿）与 D43–D53（第十二至二十二稿）——非 AC，design 层已裁定、判据落产品面 | **二十一条裁定**各有一条可机械核对的落点：D33 四类路径派生可失败（形态非法 → `None`）+ 写盘面 `starts_with(root)` 复核；D34 `--check` 孤儿扫描递归 + 跳过构建产物 + 深度上限；D35 `build_schema` 两处键空间撞名即 `ApiError::Contract`；D36 `absent` 字段零残留（字段面 + 读取面双零）；D37 fence 重复键在 `contract::parse` 文本层判；D38 `#[path]` 字面量与 `mod_path_attr` 等值；D39 knip 不接线（`apply_knip_report` 零产品调用者、`Unused` 生产面不可达）；D40 `<name>` / `module_path` 只经 `paths::{module_name, rust_module_path}` 产出；D41 `EmitTarget.form` 由调用方传入、渲染器零形态推导；D42 `generated_path` 只收 `(root, element_root, kind)`；**D43** 位置词汇 / 短形写出点各恰 1 处且判据只调用；**D44** `apiMissing` 的 op 与 `$defs` 类型同一次求值 + `#[path]` / `pub use <mod>::*` 两处取数面；**D45 / D47** 两道人字闸（七族名字 + `x-function` 落点封闭二处）；**D46** 渲染器四参单形 + `render_scope` 私有 + 零异名孪生；**D48 / D50** 模块段形态闸按语言分列（`is_module_segment` 含关键字面、TS 侧转义零筛选）；**D49** `#[path]` 读盘面三步限根；**D51** `occurrences.line` 两缺口分列；**D52** `extract::extract` 显式留白；**D53** 机械随动第四类与归属二分 | `packages/desktop/src-tauri/crates/core/archi/src/api/paths.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/contract.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/schema.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/emit/mod.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/emit/rust.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/emit/ts.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/extract/ts.rs`、`packages/desktop/src-tauri/crates/tools/archi-api-gen/src/main.rs` |
| AC-21…AC-25（设计补入，D43–D47） | ① **AC-21 类型词汇单一（D43）**：`^pub enum Position` 在产品侧恰 1 处（`extract/mod.rs`）、私有 `fn render(ty: &ApiType)` 恰 1 处且零 `pub`、`schema_type` / `signature_of` 各恰 1 处，`check.rs` 零本地 `Position` / 零本地短形 / 零 `"anyOf"` 直匹配，`emit` 独占语言文本形；关键反例：契约 `optional: true` + 代码 `Option<&str>` 归一后**两侧相等、零 `apiSignatureMismatch`**；② **AC-22 `apiMissing` 类型口径（D44）**：op 与 `$defs` 类型同一次求值（`fn missing_contracts` 恰 1）、三条排除面在案、`#[path]` 模块定位与 `pub use <mod>::*` 通配升格在位（试点自身的 `$defs` 类型零缺失，基线仍 28 / 13）；③ **AC-23 两道人字闸（D45）**：闸一七族名字 ASCII 标识符（`fn is_identifier` 恰 1 且调用点在 `verify` 体内、七族族别串在场、`$ref` 末段名复用 `ref_name`）+ `x-function` 落点封闭二处（D47）+ 闸二三条单点转义（`ts_string_literal` / `ts_comment_text` / `rust_comment_text` 各 1）+ 零裸插值 + 零 `serde_json::to_string` + `variant_name` 恰 1；④ **AC-24 渲染器出口单形（D46）**：逐产品文件带完整签名的 `^pub fn render(contract: &Contract, schema: &Value, meta: &ApiMeta, target: &EmitTarget) -> String` 各恰 1、零异名孪生（`render_with_root` / `render_with_target` 三文件各 0）、`render_scope` 私有各 1、产品侧直连恰 2 命中且命中文件去重后恰一行 `emit/mod.rs`；⑤ **AC-25 `x-function` 落点封闭（D47）**：四种越界形态各 `ApiError::Contract` 并点名 JSON pointer、两侧 `value_type` 内 `"x-function"` 零命中（逐文件锚函数体）、`function_param_names` 恰 1 且两调用点在案 | `packages/desktop/src-tauri/crates/core/archi/src/api/extract/mod.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/check.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/contract.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/emit/rust.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/emit/ts.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/emit/mod.rs` |
| AC-26…AC-29（设计补入，D49–D52） | ① **AC-26 `#[path]` 读盘面限根（D49）**：反斜杠归一架在折叠之前、`join` 后 `starts_with(root)` 复核在 `read_to_string` 之前、零新错误面（`ApiError::` 零命中）；行为面三条（反斜杠越根不命中且仓外哨兵未读 / 前向 `/` 形态零回归 / 未知形态零 panic）；② **AC-27 `is_module_segment` 关键字面（D50）**：`const RUST_KEYWORDS` 恰 1 且类型为 `[&str; N]`、零 `matches!(token` 长臂、判据函数恰 1 且体内同时含关键字判定与字符集判定、零第二形态（`"r#"` / `r#{` 逐产品文件 0）；行为面：`loop` / `crate` / `self` / `type` → `false`，`archi` / `agent` / `_x1` → `true`，关键字末段透传目标只出注释；③ **AC-28 `occurrences.line` 分列（D51）**：`line: op.line` ≥ 1、`$defs` 类型缺口 `line: 0`、`Contract` 零 `types_line` / `reexports_line` 且 `^pub struct Contract \{` 恰 1（字段仍 8）；行为面：op 缺口取 heading 行、类型缺口取 0；④ **AC-29 `extract::extract` 显式留白（D52）**：产品侧零调用者（滤测试后零命中）、留白句 ≥ 2（模块 doc + 函数 doc）、`^pub fn extract\(` 恰 1 且 `rust::extract_file` 未被连带降级、`extract` 与 `extract_element` 在单文件入口上符号集逐字段相等、`clippy -D warnings` exit 0 | `packages/desktop/src-tauri/crates/core/archi/src/api/extract/rust.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/extract/mod.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/paths.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/check.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/contract.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs`（`module_refs` 装配点） |
| AC-30（设计补入，D53——机械随动第四类的可判形态） | ① 编译前提在案：`cargo clippy --workspace --all-targets -- -D warnings` exit 0，且其通过**不依赖**任何断言值改动（逐个 `hunk` 撤掉后仍绿者即越界）；② 零新增白名单项（表体仍 18 行、区间内不含 `emit/{rust,ts}_test.rs`）；③ 零双计（实现相位 **60 路径**、测试相位 **12 组 / 44 文件**、并集 **104 路径** 三数不变）；④ **归属判定二分**：归实现相位只允许 `use` 行 / 实参补齐（元数）/ 装置同名提取三形态；归测试相位**五处**（签名自证串 / 职责清单 / 两处 `pub use` 断言改写 / 六处用例名去 `_with_root`）在 `tasks.md` 有接手红点与禁止项；**本设计对「两处 `pub use` 断言改写」的落点裁定为跨 crate 两处**——`api/mod_test.rs` 的 `render_all` 查表筛选面（A 的 `.i.rs` 不产 `pub use`）+ `crates/infra/archi/src/runner_test.rs` 的零第二出口面（`!runner.contains("pub use")`），两处 MUST 同为「锚全串 / 锚否定形态」而**非**各保留一条只在特定排版下成立的复合串（后者空转，判据力为零）；⑤ 零禁令冲突（定形串副判据：`（乙）` 行 0、`（甲）` / `（丙）` 各 1）；行为面：整批回退第四类落点 ⇒ `clippy` 必红 | `packages/desktop/src-tauri/crates/core/archi/src/api/emit/rust_test.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/emit/ts_test.rs`（第四类落点的调用形态适配归实现相位；断言值与用例名归测试相位，见「不可测试项」） |
| AC-31（设计补入，D54 / D57——映射规则资产为一等消费面） | ① **资产形态**：键 = 协议名字（首期唯一 `Outcome`，MUST NOT 小写）、每语言单槽位（`rust` / `typescript`）、槽位内恰 ≥1 个 `{}`、可选 `degraded` / `note`、零语言专属控制流；② **唯一解析点** `parse_common`（缺语言槽位 / 零 `{}` / 非法 JSON / 顶层非对象 四类坏形态各 `ApiError::Contract` 并点名键与语言），`api::meta()` 装载、`load_contracts` 以同一函数复核（静默跳过不允许）；③ **两侧渲染器收为查表单点** `common_type`，承载键经 `protocol_key` 求出、`value_type` 的硬编码分支删除、`emit::ts::function_type` 经同一出口；④ **改表即变产物**（改槽位后 `generate` 两侧产物文本随变）；⑤ **渲染器产品面零硬编码串**（映射文本形只许在 `common.json`）；⑥ **协议键两层口径**（资产键协议名 / 契约承载键小写，换算单点 `protocol_key`，两侧 MUST NOT 各写一份） | `packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs`（`parse_common` / `protocol_key` / `ApiMeta.common`）、`packages/desktop/src-tauri/crates/core/archi/src/api/emit/rust.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/emit/ts.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/common.json`（数据资产，判据经上三文件的用例与 `api:check` 观测） |
| AC-32（设计补入，D54 ③——查表单点的行为面） | `common_type` 在两侧各恰 1 处定义、调用点齐备（rust ≥ 2、ts ≥ 3）；**自造规则表**后产物文本随表变化且原文本形零残留（判据力：证明产物来自表而非渲染器硬编码）；**空表**回落普通 `type` 分派而不 panic / 不产空洞包装；渲染器**零 IO、四参签名不扩**（规则表经 `ApiMeta` 传入，D46 不受影响） | `packages/desktop/src-tauri/crates/core/archi/src/api/emit/rust.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/api/emit/rust_test.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/emit/ts.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/api/emit/ts_test.rs` |
| AC-33（设计补入，D55 / D56——跨 crate 单点与越界回滚） | ① **`api::display` 收为跨 crate 单点**（D55）：`pub fn display` + `lib.rs` 出口 `pub use api::display`；工具侧本地 `fn display` 整条删除、调用面改指 `archi::display`；`short_digest` / `is_product_rs` 仍 `pub(crate)`；② 行为面：`--check` 差异清单的路径串与 `archi::display` 输出逐字相等；③ **装置超时越界回滚**（D56）在 `packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs`——`from_secs(180)` / `register_window` 零命中、`wait_for` 恢复 30s 约定上限；该文件**不属本设计射程**（测试文件，回滚已由实现相位在 D56 裁定下执行、用例面零改动） | `packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs`、`packages/desktop/src-tauri/crates/core/archi/src/lib.rs`（出口，见「不可测试项」）、`packages/desktop/src-tauri/crates/tools/archi-api-gen/src/main.rs` -> `packages/desktop/src-tauri/crates/tools/archi-api-gen/src/main_test.rs`、`packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs`（既有装置，零用例改动） |
| AC-34（设计补入，D58 / D59 / D60——三条收口裁定的机械判据） | ① **`EmitTarget` 三字段**（D58）：定义体恰 `kind` / `form` / `modules`，`pub element_root` / `pub path: PathBuf` / `use std::path::PathBuf` 各 **0**，产品面读 `target.{kind,form,modules}` 的点滤 `*_test.rs` 后**恰 3 行**、读 `target.{path,element_root}` **0** 行；行为面：删两字段前后同一契约的渲染产物**逐字节相等**（两字段被读即不可能相等）；② **分支负载单通路**（D59）：`fn choice_payload` **0**、`variants_block` 内零第二份标量表（`"i64"` 由 **3** 降 **2**）、同一 `choice` schema 经 `value_type(choice, Position::Field, common)` 与经 `variants_block` 的负载文本**逐字相等**（含协议键 / `array` / 带 `additionalProperties` 的 `object` 三类新增带负载，而「`object` 无 `additionalProperties`」与「无 `type` 非 `$ref`」仍落 `Variant{index}`）；③ **`category_of` 出口面**（D60）：`pub use check::category_of` **0**、`check.rs` 的 `fn category_of` **1**、`check_test.rs` 的 `use super::check::{category_of, check}` **1**（测试面走模块路径）；④ 静态门与试点零影响：`cargo clippy --workspace --all-targets -- -D warnings` 零告警、`archi-api-gen --check` exit 0（试点零 `oneOf` / `anyOf` ⇒ 四类生成物逐字节不变） | `packages/desktop/src-tauri/crates/core/archi/src/api/emit/mod.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/emit/rust.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/check.rs`（判据面机械核对；行为面落 `emit/{mod,rust}_test.rs` 与 `api/mod_test.rs` 章节） |
| D54–D60 收口裁定（第二十五 / 二十九稿）——非 AC，design 层已裁定、判据落产品面 | **七条裁定**各有一条可机械核对的落点：**D54** `common.json` 是一等映射规则资产（两侧 `common_type` 查表、`value_type` 硬编码分支删除、`function_type` 经同一出口）；**D55** `api::display` 升 `pub` 并经 `lib.rs` 出口、工具侧本地实现删除；**D56** `change_flow/mod_test.rs` 的装置超时越界回滚（不属于本设计射程，见「不可测试项」）；**D57** 协议键两层口径（资产键 `Outcome` / 承载键 `outcome`，换算单点 `protocol_key`）；**D58** `EmitTarget` 三字段（`kind` / `form` / `modules`，`path` / `element_root` 零残留、`use std::path::PathBuf` 零命中、产品面读取点恰三行）；**D59** 分支负载单通路（`fn choice_payload` 零残留、`variants_block` 内零第二份标量表、同一分支经两路径产出的文本逐字相等）；**D60** `category_of` 出口面（`pub use check::category_of` 零命中、`check.rs` 的 `pub` 与 `check_test.rs` 的模块路径不动） | `packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/common.json`、`packages/desktop/src-tauri/crates/core/archi/src/api/emit/{mod,rust,ts}.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/check.rs`、`packages/desktop/src-tauri/crates/core/archi/src/lib.rs`（出口，见「不可测试项」）、`packages/desktop/src-tauri/crates/tools/archi-api-gen/src/main.rs` |
| S-1…S-17（规范文本同步，非 AC——规范↔实现冲突的 owner，D31） | **十七条**「原文须消失 / 替换后原文须在场」逐条双向 `grep -F` 对拍（**判据串 MUST 从 design 表内机械抽取**，手抄必漏——第三稿漏 `:395`、第六轮前漏乙类四处、收口稿前漏丁类三处即同一失效模式）。**甲类（门禁形态，六条）**：delta 四处（`specs/desktop-archi-api/spec.md:241` = S-1 / `:257` = S-2 / `:260` = S-3 / `:395` = S-6）由本 change 的 **spec 相位**落地；主基线两处（`openspec/specs/desktop-ipc-type-bindings/spec.md:292` = S-4 / `:273` = S-5）由**归档 SpecSync 步**落地，本 change 任何相位 MUST NOT 改。**乙类（取数面边界，四条，D19-a / D19-b / D39）**：同一 delta 文件的 `:329`+`:330` = S-7（`visibility` 词表封闭四值而**本期可控值域按语言分列**：Rust 四值可判 / TS 生产面只产 `external` + `Unused` 未接线留白，第十二稿射程由 `:295` 扩为这两处）/ `:352` = S-8（Scenario 同口径，第十二稿锚点校正为 `:352`，取代旧 `:318`）/ `:389` = S-9（箭头函数不作操作）/ `:323`+`:387` = S-10（`used_by` 留白），四条全归**本 change 的 spec 相位**。**丙类（可执行形态，两条，D38 / D39）**：同一 delta 文件的 `:201` = S-11（`#[path]` 的值 SHALL 为**字面量**且 MUST 与 `paths::mod_path_attr` 输出逐字相等）/ `:423` = S-12（`unused` 的降级接口**本期未接线**、生产面不产出 `Unused`），两条同归**本 change 的 spec 相位**。**丁类（收口稿补入，三条，D43 / D44 / D45）**：`:153` = S-13（位置分派条追加「位置词汇与签名短形各只有一份、判据只调用 `schema_type` + `signature_of`」，**保留型**：旧句须保留、判据只取新串）与 delta 另一份文件的 `specs/desktop-archi-api-check/spec.md` 的 `:12`+`:61`+Module Contract 表 `api/check.rs` 行 = S-14（`apiMissing` 的 op 与 `$defs` 类型**同一判据、同一次求值** + 三条排除面 + 两条结构前提；该表行的旧串「声明集 vs 导出集」是**丁类唯一须消失的旧原文**）、`四条明确不做` 末句之前新增一条 = S-15（两道人字闸 + `x-function` 落点封闭二处，**纯新增条**）。**戊类（安全面，两条，D49 / D50）**：`生成物落点与入口查找` 末段 = S-16（`#[path]` 读盘面三步限根，**纯新增条**）/ Module Contract 表 `api/paths.rs` 行 = S-17（`module_name` 是纯派生零形态判据 + 末段进源码的形态闸按语言分列、`checked_root` 只判整条不判末段字符集，**纯新增约束**），两条同归**本 change 的 spec 相位**。**条数（17 = delta 15 + 主基线 2）与旧原文串数（14）不是同一个数**（S-7 / S-10 各覆盖两处原文、S-14 另有 1 个替换型旧串、S-15 / S-16 / S-17 为零旧串的纯新增条），新原文串 **18**；三数 MUST NOT 混用 | `openspec/changes/module-api-contract-format/specs/desktop-archi-api/spec.md`（S-1 / S-2 / S-3 / S-6 / S-7 / S-8 / S-9 / S-10 / S-11 / S-12 / S-13 / S-15 / S-16 / S-17）、`openspec/changes/module-api-contract-format/specs/desktop-archi-api-check/spec.md`（S-14）、`openspec/specs/desktop-ipc-type-bindings/spec.md`（S-4 / S-5）——纯文本项，无单测路径（见「不可测试项」） |

---

## 单元测试

### packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/mod_test.rs

#### 待测功能

- `meta()`: 生成载元单点——`tool_version` 取 `CARGO_PKG_VERSION`、`common_digest` 取 `common.json` 的 `sha256` 摘要前 12 位、`schema_uri` / `generated_notice` 为常量；`schema` / `emit` / `INDEX` 三处共用同一份载元，纯计算零 IO（公共函数 / API）
- `enabled()`: `<archi_api_dir>` 存在且**递归**含 ≥1 份 `*.md` 即契约面启用；目录缺席 / 零 `*.md` → `false`；手写 walk、不跟随符号链接（公共函数 / API）
- `load_contracts()`: 逐 `path` 非空元素定位 md → `contract::parse` → `schema::build_schema` → `contract::verify` → `schema::verify_defs` → 收 `contracts` / `schemas` / `symbols`；`api = none` 跳过（留 `apiUnexpected` 自判）；缺 md **不是错误**（跳过该元素，**不记任何清单**——D36：`absent` 字段已删除，缺口由判据 ② 以 `element.api == Declared && contract.is_none()` 直接判）；任一错误即 `Err`（不静默跳过坏契约）（公共函数 / API）
- `render_all()`: 装配四类生成物为内存 `Vec<GeneratedFile>`（`<FQN>.schema.json` / `INDEX.toml` / `.i.rs` / `.i.ts`）；契约无类型无 op 时跳过 `.i.*`；`INDEX.toml` 覆盖全部 `path` 非空元素；返回顺序稳定；**`EmitTarget` 的 `form` 与 `modules` 在此一次算齐**（D40 / D41 的查表落点：`form` 经 `paths::entry_form`、`modules` 经元素表逐条 `paths::{module_name, rust_module_path}`，渲染器零派生）；`generated_path` 只传 `<root>` 字符串（D42，零 dummy `ArchiElement`）（公共函数 / API）
- `check()`: 十判据对外唯一入口；`input.enabled == false` → 空 `Vec`；否则委托 `check::check` 并按 `(kind, element, target)` 稳定排序（公共函数 / API）
- `bootstrap()`: 元素 → `entry_form` 定位入口 → 读源 → `extract::extract_element`（判据同一取数面）→ md 骨架文本（H1 + 空散文段 + 每 `Function` 一个 `### <op-id>` + `api-types`）；只出字符串、零写盘。**`used_by` 本期不产出**（D19-b 同源留白，S-10）：用例 MUST NOT 断言该字段（补齐需 import 反向图，待决问题 13）（公共函数 / API）
- `api_ref()`: 契约状态投影——`contract_path` 存在 → `ApiStatus::Declared` 且三路径为 repo 相对 POSIX 串；否则 `ApiStatus::None` 且三路径全 `None`；纯 `exists()` 判定、零回写模型字段（公共函数 / API）
- `api_ref_of()`: 同一投影的**元素形态入口**——消费方手上已有元素 `path` 时用，判定与 `api_ref` 逐字段同源（md 存在 → `Declared` 且三路径为 repo 相对串，否则 `none` + 三 `None`）；投影层不必为拿一条路径而二次读模型（公共函数 / API）
- `display()`: 绝对路径 → **repo 相对 POSIX 串**（剥 `root` 前缀、`\` → `/`、剥前导 `./`）；**D55 的跨 crate 唯一单点**——`crates/tools/archi-api-gen` 经 `archi::display`（`lib.rs` 的 `pub use api::display`）复用，工具侧本地逐字实现已删；纯函数、零 IO（公共函数 / API，D55 前为 `pub(crate)`）

- `parse_common()`（**私有备案行，`pub(crate)` 不入 design 公共函数表**）: `common.json` 文本 → 协议名 → 语言槽位的**唯一解析点**（D54 ②）；四类坏形态（非法 JSON / 顶层非对象 / 缺语言槽位 / 槽位内零 `{}`）各 `ApiError::Contract` 并**点名键与语言**；`meta()` 装载它、`load_contracts` 以同一函数复核（静默跳过不允许）
- `protocol_key()`（**私有备案行，`pub(crate)` 不入 design 公共函数表**）: 协议名字 → 契约 / schema 承载键的换算单点（D57 ②：`Outcome` → `outcome`）；两侧渲染器 MUST 经它求承载键，资产键 MUST NOT 改成小写
> 说明：`ApiError`（`Io` / `Contract` / `Element` + `message()`）/ `ApiSnapshot`（三分量、**无 `absent`**，D36）/ `ApiCheckInput`（六字段、**无 `absent`**，D36）/ `GeneratedFile` / `ApiMeta` 取自 design「类型定义」，用例按 D12（载元单点，`schema` / `emit` / `INDEX` 三处共用）、D16（求值互斥）/ D17（启用条件）/ D20（错误面映射）/ D22（`api_ref` 双层口径，`api_ref` 与 `api_ref_of` 两形态同源）/ D26（`IndexEntry.categories` 由 `$defs` **形状**打标、打标单点是本文件的私有 `def_shape`——design「类型定义」`IndexEntry` 行与 D26 明文）/ D29（生成物须为格式化器不动点）/ **D36**（`absent` 死数据已删，装载器不记清单）/ **D40 / D41 / D42**（`EmitTarget` 的 `form` 与 `modules` 在本层一次算齐、`generated_path` 只收 `<root>`）与 AC-1 / AC-2 / AC-4 / AC-7 / AC-12 / **AC-31 / AC-32（`parse_common` / `protocol_key` 与映射查表单点）/ AC-33（`display` 跨 crate 单点）/ AC-34 ①③（`EmitTarget` 三字段 / `category_of` 出口面）** 组织；跨模块组合用例（`load_contracts` 全链、`enabled + load_contracts + check` 装配）挂本节与 `crates/core/archi/src/drift/mod.rs` 章节，不另立集成章节。**D55 / D57 / D58 / D60 的五条落点**：`display` 升 `pub` 并经 `lib.rs` 出口（本节「`display` 跨 crate 单点面」用例）、`parse_common` / `protocol_key` 的 `pub(crate)` 私有备案（不入 design 公共函数表，行为经 `meta()` / 两侧 `common_type` 观测）、`render_all` 的三值构造（本节「`render_all` 的 `EmitTarget` 三值构造」用例）、`pub use check::category_of` 的移除（本节「`category_of` 出口面」用例）、**出口面整体归零**（D60 同口径的射程补全：`api/mod.rs` 零行首 `pub use`，唯一落点在 crate 根——本节「`api` 出口面整体归零」用例，判据串升为全串锚并以两注入自证判据力）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `meta` 载元三字段（AC-17 / D12） | 正向 | `meta().tool_version == env!("CARGO_PKG_VERSION")`；`common_digest` 为**恰 12 个字符**的十六进制串且与「独立对 `common.json` 原文算 `sha256` 再取前 12 位」的期望值逐字相等；`schema_uri` 含 `2020-12`；`generated_notice` 非空且含「勿手改」语义 | 单例 |
| `meta` 摘要稳定性（D12 幂等） | 边界 | 连续两次 `meta()` 逐字段相等（同载元即同生成物，`api:check` 零 diff 的前提）；`schema` / `emit` / `INDEX` 三处产出物头注释里的工具版本行与摘要串**逐字相同**（三处共用一份载元，零第二份载元来源） | 表驱动（2 断言） |
| `meta` 零 IO 零 spawn（AC-12） | 异常 | 调用前后 tempdir 全量字节快照相等；源码自证 `api/mod.rs` 的 `meta` 体内零 `fs::` / `read_to_string` / `std::process`（`common.json` 经 `include_str!` 编译期内嵌，不是运行期读盘） | 单例（行为 + 源码自证） |
| `enabled` 递归扫描（AC-7 前置） | 正向 | tempdir 落 `<archi_api_dir>/Infra/App.X.md` → `true`；同一棵树下只落 `INDEX.toml` / `*.schema.json` 而无任何 `*.md` → `false` | 表驱动（2 形态） |
| `enabled` 目录缺席 / 空目录 | 边界 | 根下无 `.dev-team` → `false`（零 panic）；`<archi_api_dir>` 存在但为空目录 → `false`；深层 `<Domain>/<Sub>/x.md`（两层）→ `true`（递归，D17 待决问题 6） | 表驱动（3 形态） |
| `enabled` 文件名边界 | 边界 | `<archi_api_dir>/README.md` → `true`（判据只看 `*.md` 在场，不看命名合法性——合法性归 `load_contracts`）；`x.MD` / `x.md.txt` / `x.md/`（同名目录）→ `false` | 表驱动（4 形态） |
| `enabled` 符号链接与深度上限（D17 / 实现纪律） | 边界 | 契约目录内的符号链接目录（指向含 `*.md` 的树）→ **不跟随**（`false` 或仍按真实子树判定，断言不 panic、不无限递归）；超过 `MAX_API_WALK_DEPTH` 的极深嵌套 → 返回 `false` 而**不**栈溢出（手写 walk + 深度上限，零 `ignore` crate 依赖） | 表驱动（2 形态） |
| `load_contracts` 全链（AC-1 + AC-9） | 正向 | 单元素 `path` 非空 + md 在案（H1 与文件名一致、1 op、1 类型）→ `Ok`：`contracts.len() == 1`、`schemas` 含该 FQN 且六字段齐、`symbols` key = FQN；`verify` / `verify_defs` 全过 | 单例 |
| `load_contracts` 缺 md 不记清单（**D36**） | 边界 | `api = declared` 元素无 md 文件（本仓迁移期主形态，13 条）→ `Ok` 且该 FQN **不在** `contracts`、**不在** `schemas`（不报 `Err`，缺口交 `apiMissing`，AC-7）；**快照上零 `absent` 字段**（D36 已删）——源码自证 `api/mod.rs` 内 `absent` **零命中**（字段面 + 读取面双零残留，`Absent` 词面仅存于 `ApiFormStatus::Absent` 的 `as_str` 与 doc） | 单例（行为 + 源码自证） |
| `load_contracts` `api = none` 跳过 | 边界 | 元素 `api = none` 且 md **不存在** → 不进 `contracts`、零噪声；`api = none` 且 md **存在** → 本函数仍跳过该元素（不解析），留 `apiUnexpected` 判据（D16 ⑤） | 表驱动（2 形态） |
| `load_contracts` `path` 空元素 | 边界 | 容器元素（`path = []`）→ 零参与（不进 `contracts` / `schemas` / `symbols`），零 panic；`elements = []` → `Ok` 空快照 | 表驱动（2 形态） |
| `load_contracts` 坏契约即 `Err`（AC-1） | 异常 | md 的 H1 与文件名不一致 / `### <op-id>` 后缺 `api-op` fence / `api-types` 键未在 `$defs` 出现 / 第二个 `api-types` fence 四形态 → 各 `Err(ApiError::Contract(..))`，文案含 `file:line`；**MUST NOT** 返回部分快照 | 表驱动（4 形态） |
| `load_contracts` IO 错误面（D20） | 异常 | `<archi_api_dir>` 下 md 是目录而非文件 / md 不可读（权限位模拟在 Unix 与 Windows 上不可移植，故以「md 路径被目录占位」表达）→ `Err(ApiError::Io(..))`；`message()` 非空且含路径 | 表驱动（2 形态） |
| `load_contracts` 规模边界 | 边界 | 300 元素（其中 100 条有 md）→ `Ok`，`contracts.len() == 100`、`schemas.len() == 100`（其余 200 条不产生任何条目，D36）；1000 键 `$defs` 的契约 → 装配成功不 panic | 表驱动（2 规模） |
| `render_all` 四类生成物齐备（AC-12） | 正向 | 试点形态输入（1 契约 / 1 类型 / 1 op）→ 返回文件集含 `<FQN>.schema.json`、`INDEX.toml`、`<root>.i.rs`，`.i.ts` 按元素语言缺席（试点为 Rust）；每个 `GeneratedFile.path` 与 `paths` 单点产出值逐字相等 | 单例 |
| `render_all` 零内容不生成 `.i`（事实校准 6） | 边界 | 契约零 op 零类型 → 无 `.i.rs` / `.i.ts` 条目（否则 `pub use api_gen::*;` 触发 `unused_imports`）；`INDEX.toml` 仍产出且该元素 `ops = 0` / `types = 0` | 单例 |
| `render_all` 建 `EmitTarget` 的两次求值（**D40 / D41**） | 正向 | 元素表含 A（`crates/infra/archi`，入口命中 `<dir>/src/lib.rs`）与 B（`views/x.tsx`，单文件）→ A 的 `EmitTarget.form == Crate`、B 的 `== SingleFile`（形态来自 `paths::entry_form`，非渲染器推导）；A 的 `modules` 含其透传目标的 `ModuleRef { name, module_path }`，`name` 与 `module_name(目标根)` 逐字相等、`module_path` 与 `rust_module_path(目标根)` 逐字相等；入口全不命中的元素**不产 `.i.*`**（D41：拿不到形态即不生成） | 单例（4 断言） |
| `render_all` 零 dummy `ArchiElement`（**D42**） | 边界 | 源码自证：`api/mod.rs` 内**零** `ArchiElement {` 字面量构造（D42：`generated_path` 只收 `<root>` 字符串，容器元素由 `element.path.first()?` 判，`paths` 层不判容器）；容器元素（`path = []`）→ `generated_rel` 为 `None`、**不产** `.i.*`，零 panic | 单例（源码自证 + 行为） |
| `render_all` 幂等（AC-2 / AC-12） | 边界 | 同输入两次调用 → 两次返回的 `Vec<GeneratedFile>` 逐字段相等（含 `INDEX.toml` 文本、`.i.rs` 头注释摘要）；`$defs` / `properties` 键序稳定（`serde_json::Map` 保序）；`.schema.json` 末尾换行在案 | 单例（两次对拍） |
| `render_all` INDEX 覆盖面（D17） | 边界 | 3 个 `path` 非空元素（1 有契约 / 2 缺）→ `INDEX.toml` 三条 entry，`status` 分别 `declared` / `absent`（`ApiFormStatus::Absent`，**文件事实**，与 D36 删掉的 `ApiSnapshot.absent` 字段无关）/ `absent`，条目按 FQN 字典序；`path` 空元素不出现 | 单例 |
| `render_all` INDEX 的 `categories` 形状分布（AC-2 / D26） | 正向 | 契约含 `$defs`= `{LocalArchiCheck: object}` + 一个 `api-reexport` 别名 → 该 entry 的 `categories` 为 `{"object": 1, "reexport": 1}` 且 `sum == types == 2`；`absent` 元素（无 `$defs`）→ `categories` 为空表、`build_index` 整段省略；单形状（5 个 `object`）→ `object = 5` 且 `types = 5` | 表驱动（3 形态） |
| `render_all` `def_shape` 六形状打标（D26 / 阶段九判序） | 边界 | 私有打标单点经 `render_all` → `index_entries` → `categories_of` 驱动（不直接调私有函数）；逐形状注入：含 `$ref` → `reexport`；`x-class: true` → `class`；含 `oneOf` / `anyOf` → `variant`；含 `enum` → `enum`；`type` 为 `string`/`integer`/`number`/`boolean` 且无 `enum` → `scalar`；其余（含 `object`）→ `object`；并存形态（`x-class` + `oneOf`）按判序取先者（`class` 先于 `variant`）；非 object 值（字符串 / 数组 / `null`）→ 退 `object` 不 panic | 表驱动（6 形状 + 1 并存 + 1 退化） |
| `render_all` `categories` 不借判据分类（D26 反例） | 异常 | 同一输入下判定面人为多报 10 条 finding → `render_all` 产出的 `INDEX.toml` **逐字不变**（`INDEX` 是纯投影，只随契约改动变化）；源码自证 `api/mod.rs` 内 `category_of` 零命中（形状分布不借判据分类） | 单例（两向断言） |
| `render_all` 空快照 | 边界 | 空 `ApiSnapshot` → 返回集只含 `INDEX.toml`（零 entry）；`root` 指向不存在路径 → 纯推导不 panic | 表驱动（2 形态） |
| `check` 未启用短路（D17） | 边界 | `ApiCheckInput { enabled: false, .. }` + 其它字段全非空（契约在案、符号在案）→ 空 `Vec`（启用条件优先，不因数据存在而产出，AC-7） | 单例 |
| `check` 稳定排序（D18） | 正向 | 构造多元素多 kind 命中（`apiEntryMissing` ×2 + `apiMissing` ×2 + `apiConstructorDeclared` ×1）→ 返回序按 `(kind, element, target)` 升序；**两次运行逐字段相等**（可逐字 diff 的报告） | 单例（两次对拍） |
| `check` 委托单点（AC-9） | 边界 | 同输入下 `api::check(input)` 与 `api::check::check(input)` 的结果逐字段相等且条数相等（对外入口只做短路 + 排序，零第二套判据） | 单例 |
| `bootstrap` 草稿形态（AC-12） | 正向 | 元素入口含 `pub fn a(&str) -> String` + `pub struct T` → 骨架文本 H1 = FQN、含 `## 职能` / `## 不变量` 空段、含 `### a` + `api-op` fence、含一个 `api-types` fence 且键含 `T`；每符号附 `visibility` 与位置注释 | 单例 |
| `bootstrap` 只出字符串（AC-12） | 边界 | 调用前后 tempdir 全量字节快照相等（零写盘）；`entry_form` 全不命中 → `Err(ApiError::Element(..))` 且文案含 FQN | 表驱动（2 形态） |
| `bootstrap` 不产 `used_by`（D19-b 留白 / S-10） | 边界 | 骨架文本中每符号的注记行只含 `visibility @ file:line` 形态，**零 `used_by` / 「引用方」字段**（本期留白，补齐需 import 反向图）；与 `extract_element` 的输出符号集逐项相等（共用取数面，零第二套抽取） | 单例（一条否定断言 + 一正例） |
| `bootstrap` 元素面异常 | 异常 | 元素 FQN 不存在于入参快照 / FQN 为空串 / FQN 含 emoji → `Err(ApiError::Element(..))`，零 panic | 表驱动（3 形态） |
| `api_ref` declared 三路径（AC-10 / D22） | 正向 | md 在案 → `status == Declared`；`contract_path` / `schema_path` / `generated_path` 均为 repo 相对 POSIX 串且分别以 `.md` / `.schema.json` / `.i.rs`（或 `.i.ts`，按元素 `path[0]` 扩展名）结尾；`.schema.json` 与 `.i` 尚未生成时**仍给路径串**（记录应生成物）；`status` 取自文件事实而**不**取自模型字段 | 单例 |
| `api_ref` / `api_ref_of` 两形态同源（AC-10 / D22） | 边界 | 同一 root + fqn 下「按 FQN 自取 `path`」与「由调用方直接给 `path`」两条分支结果逐字段相等（投影语义必须同源，不得因省一次读模型而变形）；`path` 空（容器元素）且 md 在案 → `generated_path` 为 `None` 而 `contract_path` / `schema_path` 仍 `Some`（md 存在即可枚举契约） | 表驱动（2 形态） |
| `api_ref_of` 元素形态入口的边界（AC-10 / D22） | 边界 | `element_path = []`（容器元素）+ md 在案 → `contract_path` / `schema_path` 仍 `Some`、`generated_path = None`（与 `api_ref` 逐字段相等）；`element_path` 多段（`["…/archi/", "…/archi/src/"]`）→ 只取 `path[0]`（D2：`<root>` 取首条），结果与单段形态逐字相等；`fqn` 与 `element_path` 冲突（FQN 说 `App.A`、path 指向 `B/`）→ 文档路径按 FQN 派生、生成物路径按 path 派生（两者各自单点，不互相污染） | 表驱动（3 形态） |
| `api_ref_of` 被 `dto::module_detail` 消费且零改写（AC-10 / AC-9） | 正向 | `module_detail(root, fqn).api` 与直接调 `api::api_ref_of(root, fqn, element.path)` 逐字段相等（`dto` 只做一次定位，零拼装）；模型字段 `element.api` 在 `api_ref_of` 报 `none` 时仍保持 `declared`（投影零回写，D22） | 单例（两向断言） |
| `api_ref` none 三路径全 `None` | 边界 | md 缺席 → `status == None` 且三路径全 `None`（MUST NOT 回写模型字段，D22）；`fqn` 为单段（`App`）→ 目录退化 `Root`，路径可判 | 表驱动（2 形态） |
| `render_all` 的 `EmitTarget` 装配含 `module_refs` 的 Rust 分支拒绝（**D48 / D50 / AC-27**） | 异常 | 透传目标元素根末段为 `loop`（`path = ["./x/loop/"]`）→ 该目标**不入** `EmitTarget.modules`，`.i.rs` 产物含 `// 透传自 <FQN>` 注释而**不含** `pub use`（`crate::loop` 的不可编译文本零产出）；对照合法末段 `archi` ⇒ 入表且 `name` / `module_path` 与 `paths::{module_name, rust_module_path}` 逐字相等；**TS 分支零筛选**（带扩展名末段照常入表） | 表驱动（3 形态） |
| `render_all` 按四参单形调 `render_target`（**D46 / AC-24 ③**） | 边界 | 源码自证：`api/mod.rs` 内 `render_target(` 的调用点实参为四元（`target` 在首位，`contract` / `schema` / `meta` 随后）；`emit/{rust,ts}::render` 在本文件**零直连**（批量入口只经 `render_target`） | 单例（源码自证 2 断言） |
| `bootstrap` 不读 knip / 不产 `unused`（**D39 / D52**） | 边界 | 源码自证：`api/mod.rs` 内 `knip` **零命中**；行为面：草稿文本内**零 `unused`** 词（`Visibility::Unused` 生产面不可达，`bootstrap` 不产生该降级）；`extract_element` 是唯一取数面（`extract::extract` 在本文件零调用） | 单例（源码自证 + 行为） |
| `ApiError` 三值与 `message()` | 异常 | `Io` / `Contract` / `Element` 三值各 `message()` 非空且互不相等（可辨错误面）；文案不外泄「检查已执行」语义（D20：资产非法 → 检查不执行） | 表驱动（3 值） |
| `parse_common` 正向（AC-31 ②） | 正向 | 真实资产解析出 `Outcome` 一条：`rust` / `typescript` 两槽位均含 `{}`、`degraded == Some("typescript")`、`note` 在场；`meta().common` 与解析结果逐项相等（载元与解析点同源，零第二份来源） | 单例 |
| `parse_common` 四类坏形态（AC-31 ②） | 异常 | 表驱动四形态：`"{"`（非法 JSON）/ `"[]"`（顶层非对象）/ 缺 `typescript` 槽 / `rust` 槽内零 `{}`；各返回 `ApiError::Contract`，文案含 `common.json` 且含期望片段（缺槽位形态另断言文案点名 `rust` / `typescript` 具体语言） | 表驱动（4 形态） |
| `protocol_key` 换算与资产键口径（AC-31 ⑥ / D57） | 边界 | `protocol_key("Outcome") == "outcome"` 且幂等（已小写输入同值）；真实资产**含** `Outcome` 键、**不含** `outcome` 键（资产键 MUST NOT 改成小写） | 表驱动（4 断言） |
| `display` 路径归一语义（**D55 / AC-33 ①**） | 正向 | `display(root, root.join("a/b"))` → `"a/b"`（剥 root 前缀）；Windows 分隔符输入（`a\b`）→ `"a/b"`（`\` → `/`）；前导 `./` 剥净；`path` 不带 `root` 前缀时**原样归一并照常替换分隔符**（`strip_prefix` 失败即 `unwrap_or(path)`，不 panic） | 表驱动（4 形态） |
| `display` 边界值（str 边界） | 边界 | `path == root` → 空串（不 panic）；`root` 为 `/` 或 `""` → 结果可判、不越界切片；非 ASCII / emoji / 尾斜杠路径 → 原样保留（零归一化）；超长路径（>1000 chars）→ 不 panic | 表驱动（5 形态） |
| `display` 跨 crate 单点面（**D55 / AC-33 ①** 源码自证） | 异常 | 源码自证：`api/mod.rs` 内 `pub fn display` 恰 **1** 处（不再是 `pub(crate)`）；`crates/core/archi/src/lib.rs` 内 `pub use api::display` 恰 **1** 行（成行写法——分组块会令判据恒 0）；`crates/tools/archi-api-gen/src/main.rs` 行首 `fn display` **0** 命中、`archi::display` **≥ 1**；`short_digest` 仍 `pub(crate)`（不随 D55 扩大出口面） | 单例（源码自证 4 断言） |
| `render_all` 的 `EmitTarget` 三值构造（**D58 / AC-34 ①**） | 正向 | 源码自证：`render_all` 内 `EmitTarget {` 字面量恰 **1** 处且实参为**三值** `kind` / `form` / `modules`（`path` / `element_root` 零命中）；`path` 与 `scope` 保留为**局部变量**并直接进 `GeneratedFile { path, content }`；`scope` 只再喂 `paths::{generated_kind, generated_path}` 与 `paths::entry_form`（派生入参，MUST NOT 成为渲染器入参）；行为面：同一快照两次调用产出的 `Vec<GeneratedFile>` 逐字段相等（删两字段零可观测影响） | 单例（源码自证 4 断言 + 行为对拍） |
| `render_all` 与 `EmitTarget` 三字段协同（**D58** 行为面） | 边界 | 产物文本只随 `kind` / `form` / `modules` 变化：固定契约 + 改 `form`（`Crate` → `SingleFile`）→ TS 侧 specifier 随变；改 `modules`（空 → 含目标）→ 透传行出现；`path` / `element_root` 已无可改（字段不存在）⇒ 编译器层面即排除该变量 | 表驱动（2 变量 × 1 对照） |
| `category_of` 出口面（**D60 / AC-34 ③**） | 边界 | 源码自证：`api/mod.rs` 内 `pub use check::category_of` **0** 命中（D60 删该行）；`api/check.rs` 内 `fn category_of` 仍 **1**（`pub` 与函数体不动）；`api/check_test.rs` 内 `use super::check::{category_of, check}` 仍 **1**（测试面走模块路径，删出口不动用例）；既有断言「`mod.rs` 内零 `category_of` 调用」**逐字保留**（删出口只会让它更真，MUST NOT 顺带改断言）。**同一事实不在两处固化**：`mod.rs` 的 `pub use` 归零只由本节「`api` 出口面整体归零」一行判，本行只判 `check.rs` 与 `check_test.rs` 两侧 | 单例（源码自证 3 断言 + 断言面不动） |
| `load_contracts` 资产复核同源（AC-31 ②） | 正向 | 源码自证：`load_contracts` 体内以 `parse_common(COMMON_JSON)?` 复核（与 `meta()` 同一函数，失败即 `ApiError::Contract`）；`meta` 体内零 `fs::` / `read_to_string` / `std::process`（资产编译期内嵌） | 单例（源码自证） |
| `api` 出口面整体归零（**D60 同口径的射程补全**；`tasks.md` 阶段三十 ⑧ d 的 `grep -cE '^pub use '` ⇒ 0） | 边界 | 源码自证：`api/mod.rs` 内**行首 `pub use `** 计数为 **0**（出口面唯一落点在 crate 根 `lib.rs` 的逐名 `pub use api::{…}` 与 `pub use api::display`）；九名（`ContractOp` / `ContractSection` / `schema_type` / `signature_of` / `ApiParam` / `Position` / `Visibility` / `build_schema` / `verify_defs`）**不在行首 `use` 声明面**（逐名与分组块两形态皆覆盖；`schema::build_schema(` 一类限定路径调用点不是 import 面，正是「走模块路径」的形态本身）。**判据力自证（空转禁例，见本节「复合串断言的失效形态」一行的同型先例）**：上一行的单名判据 `pub use check::category_of` 在出口面整体归零后**对「兄弟名被加回」与「分组块形态加回」两类回归均不可见**（前者是单一子串、后者连子串都对不上）⇒ 本行把判据串升为**行首全串 `pub use `**，并以两条注入（`pub use check::category_of;` / `pub use contract::{ContractOp, ContractSection};`）证明该判据**必红** | 单例（源码自证 3 断言 + 判据力自证 2 注入） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 文件系统 | **不 mock**：真实 tempdir（自建 `TempDir` RAII + `AtomicU64`，本 crate 无 dev-dependency）+ 真实 `fs::write` 落 md / 源码 / 模型；「IO 错误」以目录占位文件注入，不改权限 | 全部用例 |
| 契约解析 / 装配 / 判据 / 渲染 / 取数器 | **不 mock**：真实 `contract::parse` / `schema::*` / `check::check` / `emit::*` / `extract::*` 组合（`load_contracts` 与 `render_all` 的编排语义即被测对象，替身会引入第二事实源） | 全链用例、幂等与排序用例 |
| `foundation::layout` | **不 mock**：真实 `archi_api_dir` / `archi_rel_dir` 派生（路径单点即被测语义） | 全部落盘 / 路径断言 |
| 时钟 / 随机 / 环境变量 | **零依赖**：本模块零时钟、零 `env` 读取（`ApiMeta.tool_version` 取 `CARGO_PKG_VERSION` 编译期常量） | —— |
| 网络 / 子进程 / DB | **零依赖**：`api/**` 零 spawn、零 DB（AC-12） | —— |

---

### packages/desktop/src-tauri/crates/core/archi/src/api/paths.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/paths_test.rs

#### 待测功能

- `normalize_root()`: `\` → `/`、剥前导 `./`（循环）、去尾 `/`，**不去文件扩展名**（公共函数 / API）
- `contract_path()`: `<archi_api_dir>/<Domain>/<FQN>.md`，`<Domain>` = 父 FQN 末段；顶层元素退化 `Root`（公共函数 / API）
- `schema_path()`: 与 `contract_path` 同目录的 `<FQN>.schema.json`（公共函数 / API）
- `generated_path()`: `pub fn generated_path(root: &Path, element_root: &str, kind: GeneratedKind) -> Option<PathBuf>` → `<root>.i.rs` / `<root>.i.ts`（**入参是已归一的 `<root>` 字符串，不是整个 `ArchiElement`**——D42：调用方以 `element.path.first()?` 表达「容器元素无生成物」；`<root>` 形态非法（绝对路径 / 盘符 / `..` 段 / 空段 / 空串）→ `None`（`checked_root`），D33）（公共函数 / API）
- `generated_kind()`: 语言 → 生成物种类**分派单点**：`TypeScript` → `TypeScript`；`Rust` 与 `Unknown` → `Rust`（D2 下无扩展名的元素根唯一对应 Rust 形态）；调用方 MUST NOT 各自 `match` 语言（公共函数 / API）
- `entry_form()`: 按 Rust 三分支（`<dir>/src/lib.rs` → `<dir>/mod.rs` → `<dir>.rs`）/ 前端四分支（`<dir>/index.ts` → `index.tsx` → `<dir>.ts` → `<dir>.tsx`）顺序探测，返回形态与入口 repo 相对路径；`path` 空 / `<root>` 形态非法 / 全不命中 → `None`（公共函数 / API）
- `mod_path_attr()`: `#[path]` 值的**等值真值源**（D38：属性值本身是字面量，函数不产出它，而产出「入口行必须等于」的那份真值）——`"../../<name>.i.rs"` / `"../<name>.i.rs"` / `"<name>.i.rs"`；产品侧零调用者，消费面是 `paths_test` 三形态断言与 tasks 逐入口自查行（公共函数 / API）
- `ts_import_specifier()`: `"../<name>.i"`（目录形态）/ `"./<name>.i"`（单文件形态）（公共函数 / API）
- `ref_relative()`: 跨文件 `$ref` 前缀——同 `<Domain>` → `"./"`，跨目录 → `"../<Domain>/"`（公共函数 / API）
- `domain_of()`: 父 FQN 末段（顶层元素 `None`）（公共函数 / API）
- `directory()`: `<archi_api_dir>`（`archi_api_dir(root)` 的唯一转发点）；契约目录一切派生的根，即「MUST NOT 行内拼 `api` 段」的落点（公共函数 / API）
- `index_path()`: `<archi_api_dir>/INDEX.toml`（全局唯一一份；文件名常量 `INDEX_FILE_NAME` 单点持有）（公共函数 / API）
- `module_name()`: `<root>` 末段（`crates/infra/archi` → `archi`、`views/welcome-view.tsx` → `welcome-view.tsx`）；**透传 `<name>` 的唯一产出点**（D40：MUST NOT 取 FQN 末段）（公共函数 / API）
- `rust_module_path()`: **Rust 透传模块路径的单点**（D40）——`<root>` 末段 → `crate::<末段>`（`crates/infra/archi` → `crate::archi`、`crates/core/archi` → `crate::archi`）；MUST NOT 用目标 FQN（FQN 的点分段不是 Rust 路径）；零 crate 名推导（跨 crate 父子由 `apiNestedCrossCrate` 判不可表达）（公共函数 / API）
- `language_of()`: 扩展名判语言（`rs` → `Rust`；`ts` / `tsx` / `js` / `jsx` → `TypeScript`；其余 `Unknown`）（公共函数 / API）
- `is_module_segment()`: **模块段判据（私有备案：`pub(crate)`，唯一消费方是 `api::module_refs`）**——`[A-Za-z_][A-Za-z0-9_]*`（**空串不合法**、**零归一化**）**且非 Rust 关键字**（D50）；不成立即令该透传目标不入查表、渲染器只出 `// 透传自 <FQN>` 注释（零改名 / 零清洗）；**TS 侧 MUST NOT 用它**（specifier 落在字符串字面量里，由 `emit::ts::ts_string_literal` 转义、零筛选）；与 `contract::verify` 的 `is_identifier` **事实源不同**（那条判契约文本里写的名字，D48 / D50）（公共函数 / API，私有备案行）
- `RUST_KEYWORDS`: **私有常量** `const RUST_KEYWORDS: [&str; 51]`——38 个严格关键字 + 13 个保留字，唯一消费方是同文件的 `is_module_segment`；MUST 由一个 `const` 持有（MUST NOT 散在 `matches!` 长臂里）、MUST NOT 引入第二份关键字表（公共函数 / API，私有备案行）

> 说明：`EntryForm` / `GeneratedKind` / `Language` 取自 design「类型定义」；用例按 D2（两步归一、不去扩展名）/ D3（根同级落点）/ D4（三分支四分支封闭）/ D5（`#[path]` 与 TS import 的单点产出）/ D21（`INDEX.toml` 全局唯一一份）/ **D33**（四类路径派生可失败：形态非法 → `None`）/ **D38**（`#[path]` 是字面量，`mod_path_attr` 是等值判据的真值源）/ **D40**（`module_name` / `rust_module_path` 是透传两值的唯一产出点）/ **D41**（`entry_form` 是形态的唯一判据）/ **D42**（`generated_path` 只收 `<root>` 字符串）与 AC-4 / AC-17（`directory` 是「MUST NOT 行内拼 `api` 段」的唯一落点）组织。proposal 把路径单点用例记在 `api/mod_test.rs`，本设计按 sidecar 惯例落 `api/paths_test.rs`，用例集不减少。**D48 / D50**（模块段形态闸按语言分列、`RUST_KEYWORDS` 单点、关键字即拒绝）亦落本节。本文件 `#### 待测功能` 共 **15** 条**计入 design 公共函数表**（`pub fn` 十五行，与 design「公共函数 / API」的 `paths.rs` 段十五行逐项相等；`grep -c "^pub fn " paths.rs` 现算 ⇒ **15**）+ **2 条私有备案行**（`pub(crate) fn is_module_segment` 与私有 `const RUST_KEYWORDS`——design 该表零行，本设计仍逐行登记、行为经公共面观测，D48 / D50）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `normalize_root()` 四写法（D2） | 正向 | `"./packages/desktop/src-tauri/crates/infra/archi/"` / `"packages\\desktop\\src-tauri\\crates\\infra\\archi"` / `"packages/desktop/src-tauri/crates/infra/archi"` / `"packages/desktop/src-tauri/crates/infra/archi/"` → 四者结果**逐字相等**且等于无前缀无尾斜杠形态 | 表驱动（4 写法） |
| `normalize_root()` 不去扩展名（D2 反例） | 边界 | `"./packages/desktop/src/views/architecture.tsx"` → 保留 `.tsx`（MUST NOT 去扩展名；去扩展名会让前端单文件元素的 `generated_path` 落到错位文件名，AC-16 禁止） | 单例 |
| `normalize_root()` 边界值 | 边界 | `""` / `"./"` / `"."` / `"/"` / `"a//b"`（中段双斜杠）→ 不 panic、返回值可判（`""` / `"./"` 归约为空串或 `"."` 不 panic；中段双斜杠不折叠时断言「结果仍幂等」）；`"a".repeat(1200)` → 不 panic | 表驱动（5 形态） |
| `normalize_root()` 幂等 | 边界 | 任一形态 normalize 两次 == normalize 一次（`normalize_root(normalize_root(x)) == normalize_root(x)`） | 表驱动（3 形态） |
| `contract_path()` / `schema_path()` 同目录（AC-4） | 正向 | `App.Backend.Infra.ArchiRuntime` → `contract_path` 末两段为 `Infra/App.Backend.Infra.ArchiRuntime.md`、`schema_path` 末两段为 `Infra/App.Backend.Infra.ArchiRuntime.schema.json`；两者父目录**逐字相等**；均以 `archi_api_dir(root)` 为前缀（零行内 `api` 段） | 表驱动（2 函数 × 同 FQN） |
| `contract_path()` 顶层退化 `Root` | 边界 | `fqn = "App"` → 目录段为 `Root`；`fqn = ""` / `fqn = "."` / `fqn = "App."`（空段）→ `None`（`is_fqn` 判据：空段即非法，D33）；`fqn` 含 emoji 但无空段（`"App.🚀"`）→ `Some` 且不含 `..` 段（emoji 不是形态违例） | 表驱动（5 形态） |
| 路径派生四函数形态非法即 `None`（**D33** 安全边界） | 异常 | `contract_path` / `schema_path` / `entry_form` 对形态非法的 FQN 或 `<root>`（绝对路径 `/abs/x`、盘符 `C:/x`、`..` 段 `a/../b`、空段 `a//b`、空串）→ 全 `None`（`is_fqn` / `is_repo_relative` / `checked_root` 三个私有判据单点持有）；对照合法相对路径 → `Some`。**判据理由**：`Path::join` 遇绝对路径会替换 base、遇 `..` 会上溯 ⇒ 放行即让 `is_file()` 探测与工具 crates 的写盘越出仓库（同 `docs::normalize_rel` / `scan::rust::normalize_rel` 的既有纪律） | 表驱动（3 函数 × 5 非法形态 + 3 合法对照） |
| 形态判据单点（**D33** 源码自证） | 边界 | 源码自证：`paths.rs` 内 `is_fqn` / `is_repo_relative` / `checked_root` 三判据各**恰 1 处**定义，四类派生函数的形态校验**只经这三处**（零行内重复判据）；`abs` 前缀判据与会话根判据互不复用第二份实现 | 单例（源码自证 3 断言） |
| `generated_path()` 三种入口形态（AC-4 / D42） | 正向 | crate 形态（`element_root = "packages/desktop/src-tauri/crates/infra/archi"` + `GeneratedKind::Rust`）→ `…/crates/infra/archi.i.rs`（根同级，**不在** `archi/` 内）；`mod.rs` 形态 → `<dir 上级>.i.rs`；单文件形态（`views/welcome-view.tsx` + `TypeScript`）→ `views/welcome-view.tsx.i.ts`。**入参是 `<root>` 字符串**（D42：调用方以 `element.path.first()?` 表达「容器元素无生成物」，`paths` 层不判容器元素） | 表驱动（3 形态） |
| `generated_path()` 形态非法即 `None`（AC-4 / D33 / D42） | 异常 | `element_root = ""` / `"   "` / `"/abs/x"` / `"C:/x"` / `"a/../b"` / `"a//b"`（空段）→ 全 `None`（`checked_root` 判据：绝对路径 / 盘符 / `..` 段 / 空段）；对照 `"a/b"` → `Some`。**断言零穿越**：任何 `None` 分支都 MUST NOT 返回越出 `root` 的路径 | 表驱动（7 形态） |
| `generated_kind()` 语言三分派（D2：分派单点） | 正向 | `Language::TypeScript` → `GeneratedKind::TypeScript`；`Language::Rust` → `Rust`；`Language::Unknown` → `Rust`（D2 下无扩展名的元素根唯一对应 Rust 形态）；三值覆盖 `Language` 全值域（非法值在类型层不可表达） | 表驱动（3 值） |
| `generated_kind()` 是唯一 `match` 点（D40 同源纪律） | 边界 | 源码自证：`include_str!("paths.rs")` 内对 `Language` 的 `match` 恰 1 处（本函数体内），`api/` 其余产品文件对 `Language` 的 `match` **零命中**（调用方 MUST NOT 各自 match 语言再拼 `GeneratedKind`） | 单例（源码自证 2 断言） |
| `entry_form()` Rust 三分支顺序（D4） | 正向 | 同目录同时存在 `<dir>/src/lib.rs` 与 `<dir>/mod.rs` → 命中 **第一个分支**（`Crate`）；只存在 `mod.rs` → `SubdirModule`；只存在 `<dir>.rs` → `SingleFile`；返回的入口路径为 repo 相对 POSIX 串 | 表驱动（3 形态 + 优先级） |
| `entry_form()` 前端四分支顺序（D4） | 正向 | 同目录同时存在 `index.ts` / `index.tsx` / `<dir>.ts` / `<dir>.tsx` → 命中 `index.ts`（首分支）；逐个移除后依次命中后三支（4 步递进） | 表驱动（4 步） |
| `entry_form()` `<dir>/lib.rs` MUST NOT 在列 | 异常 | 只存在 `<dir>/lib.rs`（非 `src/lib.rs`）→ `None`（不在四/三分支列表内，如实报 `apiEntryMissing`） | 单例 |
| `entry_form()` 全不命中（AC-7 基线 28） | 异常 | `path` 指向不存在目录 / 目录内只有非入口名文件（`views/x.ts` 而元素根为 `views`）→ `None`；`path` 空 → `None`（容器元素天然无入口） | 表驱动（3 形态） |
| `mod_path_attr()` 三形态逐字（AC-4 / D5 / D38） | 正向 | `Crate` + `"archi"` → `"../../archi.i.rs"`；`SubdirModule` + `"archi"` → `"../archi.i.rs"`；`SingleFile` + `"archi"` → `"archi.i.rs"`（三值逐字相等，禁手算层数）。**本用例是等值判据的真值源**（D38：属性值是人写字面量，与这三值的等值性由本断言 + tasks 逐入口自查行双守） | 表驱动（3 形态） |
| `mod_path_attr()` 被入口行等值消费（D38） | 边界 | 对已编排入口 `crates/infra/archi/src/lib.rs`：其 `#[path = "…"]` **字面量**逐字等于 `mod_path_attr(EntryForm::Crate, "archi")` 的输出；源码自证该函数在产品 `.rs` 内**零调用者**（`grep -rn "mod_path_attr(" crates src --include=*.rs` 滤 `*_test.rs` 与 `pub fn` 定义后零命中）——「零产品调用者」不是缺陷而是 D38 的显式形态；`lib.rs` 的注释 MUST NOT 写「由该函数产出」（只可写「字面量 + 与函数输出等值」） | 单例（等值断言 + 源码自证 2 断言） |
| `mod_path_attr()` 解析指向根同级（AC-4 反例在案） | 边界 | 把 Crate 形态入口 `crates/infra/archi/src/lib.rs` 与值 `"../../archi.i.rs"` 做词法 join → 规范化后逐字等于 `crates/infra/archi.i.rs`；对照反例 `"../archi.i.rs"`（手写「入口同级」）→ 解析到 `crates/infra/archi/archi.i.rs`（错位，断言两值不相等） | 表驱动（正 / 反两值） |
| `ts_import_specifier()` 两形态 | 正向 | `Crate` / `SubdirModule` + `"archi"` → `"../archi.i"`；`SingleFile` + `"app"` → `"./app.i"`（单文件形态零 `../`） | 表驱动（3 形态） |
| `ref_relative()` 同域 / 跨域（AC-11） | 正向 | `App.Backend.Core.Archi` → `App.Backend.Core.Foundation`（同 `Core`）→ `"./"`；`App.Backend.Infra.ArchiRuntime` → `App.Backend.Core.Archi` → `"../Core/"`；目标为顶层元素 → `"../Root/"` | 表驱动（3 形态） |
| `ref_relative()` 撞名域（R-8） | 边界 | `App.Backend.Core.Agent` → `App.Frontend.Components.Agent`（两域末段同为 `Agent` 的子父段）→ 前缀按其 `domain_of` 算出且**互不相等**（撞名只影响布局，元素身份仍是 FQN）；`from == to` → `"./"` | 表驱动（2 形态） |
| `domain_of()` 形态 | 边界 | 四段 FQN → `Some("Infra")`；两段 FQN → `Some("App")`；单段 `"App"` → `None`；空串 → `None`；含尾点 `"App."` → 结果可判不 panic | 表驱动（5 形态） |
| `directory()` 单点转发（AC-4 / AC-17 前置） | 正向 | `directory(root)` 与 `foundation::layout::archi_api_dir(root)` **逐字相等**（不是相等前缀，是同一值）；`root` 含空格 / 非 ASCII / 尾分隔符三形态均成立 | 表驱动（3 root 形态） |
| `directory()` 零行内 `api` 段（AC-17） | 边界 | 来源文件内 `"api"` 字面量**只在** `directory` 一处的常量引用（`API_DIR_NAME` 经 `include_str!` 源码计数自证），其余 **16** 个函数体 / 常量零 `"api"` 字面量；`contract_path` / `schema_path` / `index_path` 三者的父目录逐字等于 `directory(root)` | 单例（源码自证 + 路径对拍） |
| `index_path()` 形态（D21 / 持久化二） | 正向 | `index_path(root)` 末段恰为 `INDEX.toml`、父目录恰为 `directory(root)`；与 `contract_path` / `schema_path`（`<Domain>/` 下一层）**不同层**（索引在 api 根、契约在域目录） | 表驱动（1 形态 + 2 层对比） |
| `index_path()` 全局唯一（边界） | 边界 | 对同一 root 下任意 FQN 反复调用 `index_path` → 恒同一路径（索引全局唯一一份，与元素数无关）；`root` 指向不存在路径 → 纯推导返回、不 panic、零 IO | 表驱动（2 形态） |
| `module_name()` 末段 | 正向 | `"crates/infra/archi"` → `"archi"`；`"views/welcome-view.tsx"` → `"welcome-view.tsx"`（不去扩展名，D2）；`"packages/…/archi/"`（带 `./` 与尾斜杠）→ `"archi"` | 表驱动（3 形态） |
| `module_name()` 边界 | 边界 | `""` → 空串或可判短路（不 panic）；`"/"` → 空串；`"a".repeat(1200)` → 不 panic；含 emoji 路径 → 原样末段 | 表驱动（4 形态） |
| `module_name()` 是透传 `<name>` 的唯一产出点（D40 正例 / 反例） | 正向 | 元素 `App.Backend.Infra.ArchiRuntime` 的 `module_name("…/crates/infra/archi")` == `"archi"` 而 **FQN 末段**是 `ArchiRuntime`——两值**不相等**（路径末段 vs FQN 末段，D40 的在案分叉）；同前缀形态（尾 `/`、前导 `./`）归一后结果不变。反例断言：源码自证 `emit/` 内零 `rsplit('.')`（FQN 派生第二套命名的在案反例） | 单例（3 断言 + 源码自证） |
| `rust_module_path()` 末段 → `crate::<末段>`（D40） | 正向 | `"packages/desktop/src-tauri/crates/infra/archi"` → `"crate::archi"`；`"packages/desktop/src-tauri/crates/core/archi"` → `"crate::archi"`（两个不同 crate 的同名元素给同一模块路径——跨 crate 父子已由 `apiNestedCrossCrate` 判不可表达，本函数零 crate 名推导）；`"src/views/welcome-view.tsx"` → `"crate::welcome-view.tsx"`（形态可判即可） | 表驱动（3 形态） |
| `rust_module_path()` 边界（D40） | 边界 | `""` → `"crate::"` 或可判短路（不 panic）；`"/"` → 同形；`"a".repeat(1200)` → 不 panic；含 `./` 前导与尾 `/` → 经 `module_name` 归一后与无前缀形态等价（**断言与 `module_name` 同源**：结果恒为 `format!("crate::{}", module_name(root))`）；输出**不含 FQN 点分段**（`App.X.Y` 形态即错） | 表驱动（5 形态 + 同源对拍） |
| `rust_module_path()` 是 Rust 透传模块路径的唯一产出点（D40 反例在案） | 异常 | 源码自证：`emit/rust.rs` 的 `pub use` 目标**不取自** `reexport_target` 的 FQN 形态（`App.Frontend.Components.Agent.Input` 点分段当 Rust 路径即非法）——该值来自 `EmitTarget.modules[<FQN>].module_path`（调用方经本函数算齐）；`rust.rs` 内零 `rsplit('.')`、零 `replace('.', "::")` | 单例（源码自证 3 断言） |
| `language_of()` 六扩展名（D19 分派） | 正向 | `x.rs` → `Rust`；`x.ts` / `x.tsx` / `x.js` / `x.jsx` → 四者全 `TypeScript` | 表驱动（5 形态） |
| `language_of()` 未知面（异常） | 异常 | `x.py` / `x`（无扩展名） / `x.rs.bak` / `""` / `x.RS`（大小写）→ 全 `Unknown`（不猜测；`api/extract` 对 `Unknown` 返回空 `Vec`） | 表驱动（5 形态） |
| `is_module_segment()` 关键字即拒绝（**D50** 行为判据） | 正向 | `is_module_segment("loop")` / `"crate"` / `"self"` / `"type"` / `"super"` / `"impl"` / `"use"` ⇒ **全 `false`**（本仓 `crates/infra/agent/src/sdk/mod.rs` 的 `pub(crate) mod r#loop;` 即真实来源）；`"archi"` / `"agent"` / `"_x1"` / `"x"` ⇒ **全 `true`**（形态合法且非关键字）；两组的判定**只经该函数公共语义**观测（`pub(crate)` 对同 crate 的 `paths_test` 可见，零 test-only `pub` 放宽） | 表驱动（7 拒绝 + 4 接受） |
| `is_module_segment()` 字符集边界（**D48 / D50**） | 边界 | `""`（空串）/ `"1abc"`（首字符数字）/ `"a-b"` / `"a b"` / `"a.b"` / `"a/b"` / `"welcome-view.tsx"` / `"中文"` ⇒ 全 `false`（正则 `[A-Za-z_][A-Za-z0-9_]*` 且空串不合法）；`"_"` / `"_9"` / `"A"` / `"a".repeat(1200)`（超长）⇒ 全 `true`（不 panic、零归一化）；**零归一化断言**：`"a-b"` **不**被清洗为 `"a_b"`（清洗即第二套命名，D40 明禁） | 表驱动（8 拒绝 + 4 接受） |
| `RUST_KEYWORDS` 封闭 51 值（**D50**） | 正向 | `RUST_KEYWORDS.len() == 51`（38 严格关键字 + 13 保留字）；逐值断言 `is_module_segment(词) == false`（51 值全覆盖，两段之间不插行注释以保逐值对拍）；去重后长度不变（漏值 / 重复不可绕过） | 表驱动（51 值） |
| `RUST_KEYWORDS` 单点持有（**D50** 源码自证） | 边界 | 源码自证：`paths.rs` 内 `const RUST_KEYWORDS` **恰 1 处**、类型为 `[&str; 51]`；零 `matches!(token` 长臂（关键字判据 MUST 由常量表持有、MUST NOT 散在匹配臂里）；`is_module_segment` 体内**同时**出现 `RUST_KEYWORDS` 引用与字符集判定（两项缺一即不成合取）；关键字集在 `api/` 其余产品文件内**零第二份**（`contract::verify` 的 `is_identifier` 只管字符集，关键字不在其射程） | 单例（源码自证 4 断言） |
| 零第二形态（`r#` 与清洗）（**D50** 源码自证） | 异常 | **逐产品文件**枚举（MUST NOT 目录级 `grep`——`*_test.rs` 的原始字符串字面量会把判据变噪声）：`paths.rs` 与 `emit/rust.rs` 内 `"r#"` 各 **0**、`r#{` 各 **0**（两种写出形态各钉一条）；`paths.rs` 内 `replace('-', '_')` / `trim_end_matches` 一类「清洗末段」的调用 **0** 处（零改名 / 零清洗） | 单例（逐文件源码自证 5 断言） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 文件系统 | **不 mock**：`entry_form` / `generated_path` 的存在性探测用真实 tempdir 落空文件（零内容写入即足以驱动三分支 / 四分支），其余函数零 IO 纯字符串 | 入口查找、生成物落点用例 |
| `foundation::layout::archi_dir` / `archi_api_dir` | **不 mock**：真实派生（路径单点即被测语义；`contract_path` 的前缀断言以真实派生值为基准） | 契约 / schema 路径用例 |
| 模型 | **不 mock**：`ArchiElement` 直接构造（纯值，不经解析）——`entry_form` 仍收整个元素（形态判定需 `path` 全量），`generated_path` 按 D42 只收 `<root>` 字符串（零 dummy 元素） | `generated_path` / `entry_form` 用例 |
| `RUST_KEYWORDS` / `is_module_segment` | **不 mock**：真实常量表与真实判据（51 值逐值对拍即被测语义；替一份关键字子集会让「关键字即拒绝」不可见） | 关键字面用例 |
| 时钟 / 网络 / 子进程 / 环境变量 | **零依赖** | —— |

### packages/desktop/src-tauri/crates/core/archi/src/api/contract.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/contract_test.rs

#### 待测功能

- `parse()`: 首个 `# ` 为 H1 并与文件名去 `.md` 严格比对；`## ` 起散文区、`### ` 起 op 段；fenced json 语言标 `api-op` 归属其**前一个** `###` heading；`api-types` / `api-reexport` 各只允许一次；**fence 内容在 `serde_json` 解析之前先做对象层级重复键扫描**（**D37**：`from_str` 会把重复键折叠成「后者胜」，故「同名两处定义即错」只在文本层可判——这是唯一可判处）；fence 内容解析失败或键重复 → `Contract` 错误带 `file:line`（公共函数 / API）
- `verify()`: 双向 gate 三条——(a) ops 的 id 集合 ≡ 带 `api-op` fence 的 heading 集合（差集双向点名）；(b) `types` 键集 ⊆ `schema["$defs"]` 键集（缺定义逐个点名）；(c) `reexports` 键集 ≡ `reexport_prose`（散文列表项集合，差集双向点名）；**同函数、同趟遍历另判两闸**（MUST NOT 拆成第二个公开入口）：**闸一（D45）七族名字 MUST 是 ASCII 标识符** `[A-Za-z_][A-Za-z0-9_]*`——① op id / ② `$defs` 键 / ③ 透传键 / ④ `params[].name` / ⑤ `$defs` 条目的 `properties` **数据字段键** / ⑥ `api-op` 的 `owner` / ⑦ 所有 `$ref` 的**末段名**，不成立即 `ApiError::Contract` 并点名族别与 fence 行；**`x-function` 落点封闭闸（D47）**——只许 `api-op` 顶层签名与 `$defs` 条目第一层 `properties` 的值二处，其余位置（`items` / `additionalProperties` / `oneOf` / `anyOf` 及更深层）即 `ApiError::Contract` 并**点名 JSON pointer**（公共函数 / API）
- `is_constructor()`: 构造器名封闭集——精确匹配 `new` / `default` / `build` / `empty` / `never` / `open`，前缀匹配 `from_` / `with_`；契约校验与 `apiConstructorDeclared` 共用同一函数（公共函数 / API）

> 说明：`Contract` / `ContractOp` / `ContractSection` 取自 design「类型定义」，位置语义五类封闭取自 D6，构造器封闭集取自 D11，**fence 内重复键的判据落点取自 D37**（文本层，`from_str` 之前），**两道人字闸的闸一 + `x-function` 落点封闭取自 D45 / D47**（均并入 `verify`，装载期唯一中止点、语言无关；MUST NOT 拆第二个公开入口），**逐入口自查行的 `#[path]` 等值真值源取自 D38**，**`Contract` 零 `types_line` / `reexports_line`（D51）**；用例按 AC-1 / AC-3（规则 1 / 规则 4 的「同名即错」在文本层实现）/ AC-13（试点无 `### new`）/ **AC-23（七族各一形态）/ AC-25（四种越界形态各一）/ AC-28（`Contract` 字段面仍 8）** 组织。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `parse` 完整契约（AC-1 正向） | 正向 | 试点形态源文（H1 + `## 职能` + `## 不变量` + `### file_stem` + `json api-op` fence + `json api-types` fence）→ `Ok`：`fqn` 取入参、`title` 取 H1、`ops.len() == 1` 且 `ops[0].id == "file_stem"`、`ops[0].line` 指向 `###` 行、`types` 键含 `LocalArchiCheck`、`reexports` 空、`sections.len() == 2` | 单例 |
| `parse` H1 与文件名一致（边界） | 边界 | H1 恰为 `<FQN>` → `Ok`；H1 带前后空白 / 带尾随空格 → 归一后仍 `Ok`（不因排版误报） | 表驱动（2 形态） |
| `parse` H1 与文件名不一致（异常，AC-1） | 异常 | 文件名为 `App.X.md` 而 H1 为 `App.Y` → `Err(ApiError::Contract(..))`，文案**同时含** H1 原文与文件名两处 + 行号；无 H1（首行即 `## 职能`）→ `Err`；H1 为空标题 `# ` → `Err` | 表驱动（4 形态） |
| `parse` `###` 后缺 `api-op` fence（异常，AC-1） | 异常 | `### foo` 之后直接跟 `## 职能` 或正文 → fence 归属集合缺该 op；`parse` 记为「供 `verify` 报缺」（不在 `parse` 立即 `Err`）→ 随后 `verify` 报双向差集 | 单例（`parse` + `verify` 两段断言） |
| `parse` fence 归属错位（异常） | 异常 | 一个 `###` 后出现**第二个** `api-op` fence → `Err`（归属唯一）；`api-op` fence 出现在任何 `###` 之前（文件头）→ `Err`（无归属）；`###` 后紧跟 `api-types` fence 而非 `api-op` → 该 op 记缺 fence、`api-types` 正常收 | 表驱动（3 形态） |
| `parse` 重复 op id（异常） | 异常 | 两个 `### file_stem`（同 id）→ `Err`（集合语义下重复即错，不与「heading ≡ fence」混淆）；`### file_stem` 与 `### File_stem`（大小写不同）→ 视作两 op（不做大小写归一，`Ok` 且 `ops.len() == 2`） | 表驱动（2 形态） |
| `parse` 块内不自包含 op 名（边界） | 边界 | `api-op` fence 内容为纯签名 JSON（`{"params": [...], "returns": {...}}`，无 `id` / `name` 键）→ `ContractOp.id` 取自 heading；fence 内额外带 `"id": "other"` → `id` 仍取 heading（块内键不覆盖归属，D6） | 表驱动（2 形态） |
| `parse` `api-types` / `api-reexport` 唯一（异常） | 异常 | 两个 `api-types` fence → `Err`；两个 `api-reexport` fence → `Err`；`api-types` 缺席（只有 op）→ `Ok` 且 `types` 空（合法：无自有类型）；`api-reexport` 缺席 → `Ok` 且 `reexports` / `reexport_prose` 均空 | 表驱动（4 形态） |
| `parse` fence 内重复键在文本层判（AC-1 / AC-3 规则 1·4 / **D37**） | 异常 | 三类 fence 各注入一处**对象层级**重复键（`api-types` 的 `$defs` 内同键两次 / `api-op` 的 `{"params":…,"params":…}` / `api-reexport` 同键两次）→ 各 `Err(ApiError::Contract(..))`，文案**点名该键** + fence 语言标 + `file:line`；`serde_json` 的折叠语义（后者胜）不构成合法形态（`MUST NOT` 断言「折叠即 `Ok`」——`schema_test` 的旧反向断言按 D37 改判，见该章节） | 表驱动（3 fence × 重复键） |
| `parse` 重复键扫描的判据面（D37 边界） | 边界 | ① 扫描**只覆盖对象**（数组内元素无身份）：`[{"a":1},{"a":2}]` → `Ok`（两元素各一个 `a`，跨元素不重复）；② 嵌套对象逐层扫描：`{"a":{"b":1,"b":2}}` → `Err` 点名 `b`；③ `A` 与 `a` 是**两个键**（零归一、零合并）→ `Ok`；④ 同一 JSON 里两处**不同**键 → `Ok`；⑤ 扫描与解析用**同一份文本**（`MUST NOT` 先 `from_str` 再比对——那已折叠）——源码自证 `contract.rs` 内 `serde_json::from_str` **恰 2 处**且**两处之前**各有一处键唯一性扫描（判据面按符号锚点核对，不写死行号） | 表驱动（5 形态 + 源码自证 2 断言） |
| `parse` fence 内 JSON 语法错（异常） | 异常 | `api-op` / `api-types` / `api-reexport` 三处各注入一处坏 JSON（缺右括号 / 尾逗号 / 单引号）→ `Err(ApiError::Contract(..))`，文案带 `file:line` 与原文片段 | 表驱动（3 处 × 3 错形中的对应一处） |
| `parse` 散文透传列表（AC-11） | 正向 | `## 透传` 段内含 `- AgentInput → App.Frontend.Components.Agent.Input` / `- AgentSessionView → …` 两项 + `api-reexport` fence 同两键 → `reexport_prose` 两项、`reexports` 两键；散文列表按 `-` 项起始、忽略空行与其余正文 | 表驱动（2 项） |
| `parse` 散文列表空 / 无列表（边界） | 边界 | `## 透传` 段为空 → `reexport_prose` 空；无 `## 透传` 段但有 `api-reexport` fence → `reexport_prose` 空（留给 `verify` 报差集） | 表驱动（2 形态） |
| `parse` 长文档 / 特殊字符（边界） | 边界 | 2000 行 500 个 op 的 md → `Ok` 且 `ops.len() == 500`、行号单调递增；H1 与 op id 含 emoji / 含 `\0` 与 `\n` 转义 → 不 panic、`Err` 或 `Ok` 可判；超长单行（>1000 chars 散文）→ 不 panic | 表驱动（3 形态） |
| `parse` CRLF / 无尾换行（边界） | 边界 | `\r\n` 行尾的同一 md（与 `\n` 版本）→ `ops` / `types` / `sections` 集合相等（行尾归一）；文件末无换行 → 不 panic、最后一段正常收 | 表驱动（2 形态） |
| `verify` 三条 gate 全过（正向） | 正向 | heading 集合 ≡ op fence 集合、`types` 键 ⊆ `$defs`、`reexports` ≡ 散文列表 → `Ok(())`（零文案） | 单例 |
| `verify` gate (a) 双向差集（异常，AC-1） | 异常 | (i) heading `### a` 有 fence 但 `Contract.ops` 缺 `a`；(ii) `ops` 有 `b` 而 heading 集合无 `b`；（iii）heading `### c` 无 fence → 三形态各 `Err` 且文案**点名两侧差异项**（缺哪一侧、哪些 id） | 表驱动（3 形态） |
| `verify` gate (b) 类型缺定义（异常，AC-3） | 异常 | `api-types` 键 `T` 而 `schema["$defs"]` 无 `T` → `Err` 且文案逐名点出 `T`；`$defs` 有而 `api-types` 无（超集方向）→ `Ok`（规则是 ⊆，单向） | 表驱动（2 形态） |
| `verify` gate (c) 透传键集不等（异常，AC-11） | 异常 | fence 两键而散文列表一项 → `Err` 点名缺失项；散文两项而 fence 一键 → `Err`；两处键序不同 → `Ok`（集合语义，非序列） | 表驱动（3 形态） |
| `verify` 空契约（边界） | 边界 | `ops` / `types` / `reexports` 全空 且 `$defs` 空 → `Ok`（空契约合法，`render_all` 会跳过 `.i.*`）；`schema` 无 `$defs` 键 → `types` 非空时 `Err`、`types` 空时 `Ok` | 表驱动（2 形态） |
| `is_constructor` 八名命中（AC-13 / D11） | 正向 | `new` / `default` / `build` / `empty` / `never` / `open` 六精确 + `from_` 前缀（`from_str` / `from_`） + `with_` 前缀（`with_capacity` / `with_`） → 全 `true` | 表驱动（10 形态） |
| `is_constructor` 未命中（异常，AC-13） | 异常 | `file_stem` / `compose_text` / `parse` / `run` / `fromage`（`from_` 前缀外）/ `newer` / `without_x` / `default_impl` → 全 `false`（前缀判据须匹配 `from_` / `with_` 字面量，不做词干启发） | 表驱动（8 形态） |
| `is_constructor` 词形边界 | 边界 | `""` / `"NEW"` / `"New"` / `"  new  "` → `false`（大小写与空白不归一，封闭集精确）；`"from_"` / `"with_"`（空前缀后无字符）→ `true`（前缀匹配）；超长（>1000 chars）→ `false` 不 panic | 表驱动（6 形态） |
| 闸一 七族名字逐族判错（**D45 / AC-23**） | 异常 | 七族各注入一处非 ASCII 标识符并断言 `Err(ApiError::Contract(..))`、文案**点名族别与该 fence 行**：① op id 含 `-`（`### file-stem`）、② `$defs` 键含空格、③ 透传键含 `/`、④ `params[].name` 为 `in-put`、⑤ `$defs` 条目 `properties` 键为 `a-b`、⑥ `api-op` 的 `owner` 为 `带-连字符`、⑦ `$ref` 末段为 `带 空格`；对照：七族全合法 → `Ok(())`（零文案）；**MUST NOT 产出不可编译文本**（错在装载期即中止） | 表驱动（7 族负例 + 1 正例） |
| 闸一 七族机制单点（**D45** 源码自证） | 边界 | 源码自证：标识符判据函数 `fn is_identifier` 在产品侧**恰 1 处**且其调用点在 `verify` 体内；七族的收集点各恰 1 处（`$defs` 条目的 `properties` 数据字段族别串在场、`"owner"` 键读取 ≥ 1、`$ref` 末段名**复用** `api::ref_name` 而非自写第二个末段切分：本文件 `fn ref_name` **0** 处且 `ref_name(` ≥ 1）；`emit/{rust,ts}.rs` 内 `is_identifier` **零命中**（渲染期不重复校验，D45 明文） | 单例（源码自证 6 断言） |
| 闸一 名字合法面不被误伤（**D45** 边界） | 边界 | `_x1` / `A` / `a9` / 超长（>1000 chars 的纯 ASCII 标识符）→ `Ok`（形态合法即放行，无长度上限）；`""`（空串 op id）/ `1abc` / emoji / `中文` → `Err`；`A` 与 `a`（大小写不同）→ 两名字均合法（零归一、零合并） | 表驱动（5 合法 + 5 非法） |
| `x-function` 落点封闭二处（**D47 / AC-25**） | 异常 | 四种越界形态各 `Err(ApiError::Contract(..))` 且文案点名 **JSON pointer**：① `$defs.T.properties.items` 的数组元素上、② `additionalProperties` 上、③ `oneOf` 分支上、④ `$defs.T` 自身顶层；合法二处不变：`api-op` 顶层签名 + `$defs` 条目第一层 `properties` 的值 → `Ok`（试点契约形状） | 表驱动（4 越界 + 2 合法） |
| `x-function` 落点闸的单点与收敛（**D47** 源码自证 / 行为面） | 边界 | 源码自证：落点判据在 `verify` 体内被调用（定形族别串在场，守卫函数符号名可换）；`fn function_param_names` **恰 1 处**且两调用点在案（`op.signature` / `methods_of`）；**判据形态不被绑死**——容器黑名单与父级白名单两种实现均可（故本用例**不用**「容器词计数 ≥ 1」作硬判据，它只作留痕：白名单式实现下该计数恒 0）；行为面：四种越界形态在 `load_contracts` 期即中止，**MUST NOT 走到渲染期**（否则 `params[].name` 会沿 `value_type` → `function_type` / `method_signature` 裸插） | 单例（源码自证 2 断言 + 行为面） |
| `choice_payload` 词面零残留（**D59** 与本文件的交汇） | 边界 | 源码自证：`contract.rs` 内 `choice_payload` **0** 命中（D59 把该私有函数整条删除，⑦ `$ref` 末段名族的注释改指 `value_type`）；`contract_test.rs` 的同一处注释词面同批改指（归测试相位，`tasks.md` 阶段三十 ⑤ c）；七族族别串本身**逐字不变**（判据面不动，只改注释里的函数名） | 单例（源码自证 2 断言） |
| `Contract` 字段面仍 8 且零行号死字段（**D51 / AC-28**） | 边界 | `Contract` 恰 8 字段（`fqn` / `title` / `file` / `ops` / `types` / `reexports` / `reexport_prose` / `sections`）；源码自证 `grep -c "types_line"` = **0** 且 `grep -c "reexports_line"` = **0**（两条分开判，MUST NOT 写成一条正则——竖线会破坏表格行结构）、`^pub struct Contract \{` = **1**（**MUST 用带 `{` 的定形串**：裸 `pub struct Contract` 会把 `ContractOp` / `ContractSection` 一并命中）；`ContractOp.line` 的 doc 明写「heading 所在行」 | 单例（源码自证 4 断言） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 文件系统 | **零依赖**：`parse` / `verify` 接 `source: &str`，用例以字符串夹具直传（零落盘、零 tempdir） | 全部用例 |
| serde_json | **不 mock**：真实解析 fence 内容（JSON 语法错面即被测语义） | 全部用例 |
| 契约装配（`build_schema`） | **不 mock**：`verify` 的 `schema` 入参由真实 `build_schema` 产出（gate (b) 的 ⊆ 关系即被测语义），另备手写 `serde_json::json!` 反例注入缺定义 | gate (b) 用例 |
| 时钟 / 网络 / 子进程 / 环境变量 | **零依赖** | —— |

---

### packages/desktop/src-tauri/crates/core/archi/src/api/schema.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/schema_test.rs

#### 待测功能

- `build_schema()`: 六字段装配——`$schema`（`meta.schema_uri` 常量）/ `$id`（`<FQN>.schema.json`，不合成 URI）/ `title`（`contract.title`）/ `$defs`（`contract.types` 原序 + `reexports` 每项转纯 `$ref`）/ `properties`（op 键 → `signature` 原样 + 每个 `$defs` 键 → `{"$ref": "#/$defs/<k>"}`）/ `$comment`（`meta.generated_notice`）；`serde_json::Map` 保序保证幂等（公共函数 / API）
- `verify_defs()`: 五条规则（键名唯一 / 嵌套 `$defs` 为空且 `$ref` 指顶层 / 所有 `$ref` target 为顶层条目 / 同键只定义一次 / 禁按结构去重——本函数不做任何合并）+ 透传条目纯 `$ref` 校验 + **`x-class` 带属性判错**（`x-class: true` 且含 `properties` 或 `required` → `ApiError::Contract`，点名类型与违规键；D27）（公共函数 / API）
- `build_index()`: `INDEX.toml` 文本——顶部三行注释（生成警示 / review 口径 / 生成器标识 + `common.json` 摘要前 12 位）、`[[entry]]` 按 FQN 字典序、七字段 + **`[entry.categories]` 形状分布子表**（非空时输出、`absent` 或空 `$defs` 整段省略、键序字典序；D26）、零语言词（公共函数 / API）

> 说明：`IndexEntry` / `ApiFormStatus` 取自 design「类型定义」，六字段出处取自 D7，五规则取自 D8，`x-class` 错误判据取自 D27 / 事实校准 16，`INDEX.toml` 形态取自 D13 / 持久化二，`categories` 形状分布语义取自 D26 / 事实校准 15（键空间六值开放字符串、`sum(categories) == types`、`absent` 整段省略）；用例按 AC-2 / AC-3 / AC-5 / AC-11 / AC-17 组织。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `build_schema()` 六字段齐备（AC-2） | 正向 | 试点形态契约（1 op `file_stem` + 1 类型 `LocalArchiCheck`）→ 顶层键**恰**六项：`$schema` == 2020-12 常量、`$id` == `App.Backend.Infra.ArchiRuntime.schema.json`（不合成 URI、无 `http`）、`title` == H1、`$defs` 含 `LocalArchiCheck`、`properties` 含 `file_stem` 与 `LocalArchiCheck`、`$comment` 非空且含「勿手改 / api:check」警示词 | 单例 |
| `build_schema()` `properties` 双来源（D7 / D35） | 正向 | op 键的值为 `api-op` fence **原样**（含 `params` / `returns`，零翻译）；类型键的值为 `{"$ref": "#/$defs/<k>"}`（仅此一键，无结构关键字）；op 键与类型键**不同名**时两处键集各自保持；同名歧义形态按 **D35** 判 `ApiError::Contract`（不在本节断言「后写者胜」——静默覆盖即缺陷，见下条撞名用例） | 表驱动（2 形态） |
| `build_schema()` `$defs` 键序（幂等，AC-2） | 边界 | `api-types` 声明序 `b, a` + `api-reexport` 序 `c` → `$defs` 键序为 `b, a, c`（**保序，不排序、不按结构合并**）；同输入两次 `to_string_pretty` 字节相等 | 单例（两次对拍） |
| `build_schema()` 透传条目纯 `$ref`（AC-11） | 正向 | `api-reexport = {"AgentInput": "App.Frontend.Components.Agent.Input"}` → `$defs.AgentInput` 为 `{"$ref": "<ref_relative 值>App.Frontend.Components.Agent.Input.schema.json#/$defs/AgentInput"}`；前缀取自 `paths::ref_relative`（同域 `./`、跨域 `../<Domain>/`），**零手算层数** | 表驱动（同域 / 跨域两形态） |
| `build_schema()` 空契约（边界） | 边界 | 零 op 零类型 → `$defs` 为 `{}`、`properties` 为 `{}`、其余四字段仍在（六字段恒在场）；`api-types` 为 `{}` → 同上；`api-reexport` 只给空 map → `$defs` 空 | 表驱动（3 形态） |
| `build_schema()` 大契约（边界） | 边界 | 200 op + 1000 `$defs` 键 → 装配成功、键数逐项相等、不 panic；单条 `$defs` 值含超长（>1000 chars）`description` → 原样透传（零改写） | 表驱动（2 规模） |
| `build_schema()` 非法类型值（异常） | 异常 | `api-types` 值为非对象（字符串 / 数组 / `null`）→ `Err(ApiError::Contract(..))` 且点名该键；`api-op` fence 内容为非对象 → `build_schema` 透传（形态合法性归 `contract::parse` / `verify`），断言不 panic | 表驱动（4 形态） |
| `verify_defs()` 键名身份唯一（AC-3 规则 1 / **D37 改判**） | 异常 | **重复键在文本层已由 `contract::parse` 报 `Err`**（D37：`serde_json` 折叠后 `&Value` 面**结构上不可达**），故本节断言的是**折叠后的零副作用面**：`verify_defs` 对同一 `Value` 的键集做**精确比较**、零归一、零合并——`A` 与 `a` 是两个键（`Ok`），语义相同的两条目并列（`Ok`，不做结构去重）；**契约测试的旧断言「重复键折叠合规、`verify_defs` 返回 `Ok`」按 D37 改判为「`parse` 报 `Err` + `verify_defs` 面保持零副作用」**（在盘 `schema_test.rs` 的同名用例须同步改名与改断言，本设计只登记方向） | 表驱动（2 形态） |
| `build_schema()` 两处键空间撞名即契约错误（AC-3 规则 4 / **D35**） | 异常 | ① `$defs` 内 `api-types` 与 `api-reexport` **同名**（`api-types: {X:…}` + `api-reexport: {X: "App.Y.X"}`）→ `Err(ApiError::Contract(..))`，文案点名该名与「`$defs` 键空间」；② `properties` 内 op 键与类型键**同名**（`### T` 与 `$defs.T` 并存）→ 同形 `Err` 并点名；③ 两处都**先判后写**（源码自证：`Map::insert` 之前有 `contains_key` 前置判定，`grep -c "contains_key"` 在两处判据各命中）；④ 无撞名 → `Ok` 且两处键集各自保持（`MUST NOT` 静默覆盖） | 表驱动（2 撞名形态 + 无撞名对照 + 源码自证） |
| `verify_defs()` 嵌套 `$defs` 须提升且原名保留（AC-3 规则 2） | 异常 | `$defs.A` 内含 `"$defs": {"B": {...}}` → `Err` 且文案点名「须提升且原名保留」；提升后顶层键为 `B`（原名，**不加父前缀**）→ `Ok`；提升时改名（`A_B`）→ 与契约 `api-types` 键集不一致 → 由 `contract::verify` gate (b) 报（本节断言 `verify_defs` 对两个不同名的顶层键均 `Ok`，禁静默改名在于不生成别名） | 表驱动（3 形态） |
| `verify_defs()` `$ref` 只能指顶层（AC-3 规则 3） | 异常 | `$ref: "#/properties/x"` / `"#/items"` / `"#/allOf/0"` / `"#/$defs/A/properties/y"` 四形态 → 全 `Err` 并给原文；`$ref: "#/$defs/A"` → `Ok`；外部文件形式 `"./X.schema.json#/$defs/T"` → `Ok`（跨文件引用合法） | 表驱动（7 形态） |
| `verify_defs()` 同类型只定义一次（AC-3 规则 4） | 边界 | 两个 `$ref` 指向同一 `#/$defs/A` → `Ok`（多引用一次定义即正确形态）；`A` 与 `a`（大小写不同）两键 → `Ok`（键名精确比较，不做归一） | 表驱动（2 形态） |
| `verify_defs()` 禁按结构去重（AC-3 规则 5） | 边界 | `ArchiDependsOn` 与 `ArchiDependedBy`（同形不同义，出入边语义相反）并列 → `Ok` 且两键**都保留**（`verify_defs` 不做任何合并）；`verify_defs` 前后 `$defs` 键数相等（无副作用） | 单例（前后对照） |
| `verify_defs()` 透传条目带结构关键字（AC-11） | 异常 | 透传条目含 `type` / `properties` / `enum` / `const` / `items` / `oneOf` 任一（六形态逐项）→ `Err` 且点名该键与违规关键字；纯 `$ref` 条目 → `Ok` | 表驱动（6 形态 + 正例） |
| `verify_defs()` 空 / 边缘形态（边界） | 边界 | `$defs` 缺席 → `Ok`（无类型即无规则冲突）；`$defs = {}` → `Ok`；`$defs` 值含 `anyOf` / `minLength` / `pattern` 等标准关键字（非结构去重 / 非嵌套 `$defs`）→ `Ok`（原生 JSON Schema 全可表达，D7） | 表驱动（3 形态） |
| `verify_defs()` `x-class` 带属性判错（AC-5 / D27） | 异常 | `{"x-class": true, "properties": {"count": {"type": "integer"}}}` → `Err(ApiError::Contract(..))`，文案点名类型名 + 违规键 `properties` + 「trait 仅允许方法」语义；`{"x-class": true, "required": ["a"]}` → 同形 `Err` 并点名 `required`；`{"x-class": true}`（零属性）→ `Ok`；`x-class: false` 且带 `properties` → `Ok`（`false` / 缺省不参与判定） | 表驱动（4 形态） |
| `verify_defs()` `x-class` 判据不遮蔽既有五规则（边界） | 边界 | 同一 `$defs` 里一条带嵌套 `$defs`（规则 2 违例）+ 一条 `x-class` 带属性（D27 违例）→ `Err`，且**单独去掉**其一后另一条仍各自 `Err`（两条判据互不吞并）；`$ref` 非顶层违例与 `x-class` 违例并存 → 同理；断言三次调用皆 `Err` 且文案可辨 | 表驱动（2 组合 × 3 次调用） |
| `verify_defs()` 判据不进 `filter` / 不增变体（边界） | 边界 | 判错返回的 `ApiError` 仍为三变体之一（`ApiError::Contract`，MUST NOT 新增第四变体）：源码自证 `include_str!("schema.rs")` 内 `ApiError::` 出现形态只含 `Io` / `Contract` / `Element`；`x-class` 判定不写进 `check.rs`（判据十值不扩第十一值） | 单例（源码自证） |
| `build_index()` 说明段三行（AC-17 / D13） | 正向 | 单 entry → 文本首三行分别为生成警示、review 口径（含「只读 `<FQN>.md`」与「机械噪声」）、生成器标识（`archi-api <CARGO_PKG_VERSION>` + `common.json@sha256:<12 位十六进制>`）；语言词禁令：文本不含 `crate` / `Cargo.toml` / `Rust` / `TypeScript` | 单例 |
| `build_index()` 条目七字段（持久化二） | 正向 | 单 entry（`fqn` / `status = Declared` / `types = 1` / `ops = 1` / `degraded = 0` / `schema = "Infra/…schema.json"` / `generated = "…archi.i.rs"`）→ 文本可被 `toml` 解析回等价结构，七字段逐项相等；`status = Absent` → 线词 `absent`、`schema` / `generated` 为空串 | 表驱动（2 形态） |
| `build_index()` `[entry.categories]` 输出（AC-2 / D26） | 正向 | `categories = {"object": 1}` 的 `Declared` entry → 文本含 `[entry.categories]` 子表且 `object = 1`；`toml` 反解析后该子表键值逐项相等；`sum(categories) == types` 对每个 `Declared` 条目成立 | 表驱动（2 形态：单形状 / 多形状） |
| `build_index()` `categories` 空表整段省略（D26 / 持久化二） | 边界 | `status = Absent` 的 entry（`categories` 空 `BTreeMap`）→ 文本**零** `categories` 段（MUST NOT 输出空表）；`Declared` 但 `$defs = {}`（空契约）→ `types = 0` 且 `categories` 段同样省略；`entries = []` → 全文零 `categories` | 表驱动（3 形态） |
| `build_index()` `categories` 与 `types` 同源自洽（D26） | 边界 | 六形状各一条（`object` / `enum` / `variant` / `class` / `scalar` / `reexport` 各 1）→ `types = 6`、`categories` 六键各 `1`、键序为 `BTreeMap` 字典序（`class, enum, object, reexport, scalar, variant`）；全部同形状（5 条 `object`）→ `types = 5`、`object = 5` | 表驱动（2 形态） |
| `build_index()` `categories` 键空间开放（D26） | 边界 | 注入形状名 `"未来新形状"`（六值外的开放字符串）→ 序列化成功、TOML 键原样落盘（值域开放、无 `ALL`、无封闭校验）；键名含 `-` / 含 emoji → TOML 引号转义后仍可反解析回原键 | 表驱动（3 形态） |
| `build_index()` `categories` 不取判据分类（D26 反例） | 异常 | 键集与判据分类六值（`entry` / `contract` / `signature` / `reexport` / `nesting` / `constructor`）**无交集**（形状键里 `reexport` 是唯一的同名者，断言它是纯 `$ref` alias 计数而**不是** `apiReexportDrift` 条数）；`INDEX` 文本不随契约债数变化（同契约 + 人为多报 10 条 finding 的两份输入 → `build_index` 输出逐字相等——`build_index` 入参无 finding，属结构保证） | 单例（两向断言） |
| `build_index()` 字典序（幂等） | 边界 | 入参乱序 `["C", "A", "B"]` → 输出三条按 `A, B, C`（函数内再排序一次，调用方序不影响结果）；同输入两次调用文本**逐字相等** | 单例（两次对拍） |
| `build_index()` 空条目（边界） | 边界 | `entries = []` → 说明段三行在场、零 `[[entry]]`、`toml` 解析为空表；1000 条 entry → 全部在场、序稳定、不 panic | 表驱动（2 规模） |
| `build_index()` 计数边界 | 边界 | `types = 0` / `ops = 0` / `degraded = 0`（空契约形态）→ 输出 `0` 而非省略键；`types = u32::MAX` → 文本为 `4294967295`（无饱和、无符号翻转）；`degraded` 计数 = 契约 `x-common.degraded` 条目数（2 条 → `2`） | 表驱动（3 形态） |
| `build_index()` 特殊字符（边界） | 边界 | FQN 含 emoji / `degraded` 原因字符串含 `\n` 与 `"` → TOML 转义后仍可解析回原值（不产生非法 TOML）；超长 FQN（>1000 chars）→ 不 panic | 表驱动（3 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 契约输入 | **不 mock**：`Contract` 由真实 `contract::parse` 产出（另备 `serde_json::json!` 直接构造 `$defs` 的非法形态，`verify_defs` 的 `schema` 入参即纯值，无需 `Contract`） | 全部用例 |
| serde_json / toml 解析 | **不 mock**：真实序列化 + `toml` 反解析（INDEX 可解析性即被测语义） | 全部用例 |
| `paths::ref_relative` | **不 mock**：真实调用（跨域前缀单点即被测语义） | 透传条目用例 |
| `ApiMeta` | **不 mock 数据、不造替身类型**：以字面量构造真实 `ApiMeta`（`tool_version` 取 `CARGO_PKG_VERSION`、`common_digest` 取真实 `sha2` 摘要前 12 位） | 说明段 / `$comment` 用例 |

---

### packages/desktop/src-tauri/crates/core/archi/src/api/extract/mod.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/extract/mod_test.rs

#### 待测功能

- `extract()`: 按元素语言分派 `rust` / `ts` 取数器；`Language::Unknown` → 空 `Vec`（不猜测）；元素 `path` 为空 → `<root>` 取空串，**MUST NOT 索引 `path[0]`**（容器元素无 `path`，索引即 panic）；**产品侧零调用者 = 显式留白（D52）**：判据与 bootstrap 一律经 `extract_element`，本函数是单文件入口面的预留接口——`grep -rn "extract::extract(" crates/ src/ --include=*.rs` 滤 `*_test.rs` 后**零命中是既定形态**；模块 doc 与函数 doc 各 MUST 写明该留白与退出条件；**MUST NOT 加产品调用者**、**MUST NOT 连带把 `rust::extract_file` / `ts::extract_file` 的 `pub` 降级**（公共函数 / API）
- `extract_element()`: **判据的「代码导出集」唯一取数面**——从实现入口出发，按语言决定是否走模块树（Rust → `extract::rust::extract_crate`；TS → 读入口后 `ts::extract_file`）；只看入口会把实现在私有模块里的 op 判成缺失（`bootstrap` 草稿失真同因）；入口读不到 → 空 `Vec`（缺口由 `apiEntryMissing` 报）（公共函数 / API）
- `normalize_signature()`: 归一单行签名 `fn(<类型序列>) -> <返回类型>`——参数名 / 空白 / 多行排列 / `is_async` 均不参与（公共函数 / API）
- `signature_of()`: **契约侧归一签名的唯一写出点**（D43）——把 `api-op` fence 原文（含 `x-function` 包裹形态）折成与 `normalize_signature` **同一短形词汇**的 `fn(<类型序列>) -> <返回>`：逐参数 `schema_type(param["schema"], Position::Param)`、返回位 `Position::Return`，**`optional == true` 的参数包 `ApiType::Optional`**（漏此分支即 `Option<T>` 入参恒假阳，是 D43 的直接溯源）；签名缺 `params` / `returns` 时按空序列 / `()` 处理；判据 MUST NOT 自写第二份（`check.rs` 零本地短形拼装）（公共函数 / API）
- `schema_type()`: **JSON Schema 片段 → 逻辑类型的单点**（D43）——`$ref` → `Unknown(ref_name(..))`；`outcome` → `Outcome(inner)`；**nullable 三分支**（`"type": "null"` / 含 `"null"` 的 `type` 数组 / `anyOf`-`oneOf` 内含 `"type": "null"` 分支）→ `Optional(inner)`；`string` 按位置 → `Str`（`Param`）/ `OwnedString`（`Return` / `Field`）；`integer` → `Int`；`number` → `Float`；`boolean` → `Bool`；`array` → `List(schema_type(items, Position::Field))`；`object` + 对象型 `additionalProperties` → `Map(schema_type(values, Position::Field))`；其余 → `Unknown(原文类型或空串)`；**语言无关**（不产出任何语言的文本形——那是 `emit` 的射程）（公共函数 / API）
- `render()`（**保持私有**）: 逻辑类型的规范短形**唯一写出点**（D43）——`Str` → `str`、`OwnedString` → `String`、`Int` → `int`、`Float` → `float`、`Bool` → `bool`、`List` → `list<T>`、`Map` → `dict<T>`、`Optional` → `optional<T>`、`Outcome` → `result<T>`、`Named` → 裸名、`Unknown(raw)` → `?raw`；唯一消费方是同文件的 `normalize_signature` 与 `signature_of`，**不升格 `pub`**（升格即「仅测试可见的导出」，沿仓库既有纪律）；行为经两个公开函数间接观测，**MUST NOT** 为测试放宽可见性（公共函数 / API，私有备案行）
- `Position`: **位置词汇的唯一落点**（D43）——`Param`（入参位）/ `Return`（返回位）/ `Field`（字段位与容器元素位）；`emit/{rust,ts}.rs` 与 `check.rs` MUST NOT 另定义 `Position`（只 `use`）（公共函数 / API）
- `Named` 的短形口径（**不新增函数**）: `Named` 裸名与 `$ref` 引用串归一由 `schema_type` 落到 `Unknown(ref_name(..))` + `render` 的 `?raw` 同形解决；**MUST NOT** 为对齐另造「引用串归一成 `Named`」的第二套规则（公共函数 / API 表内的口径行）

> 说明：`ApiSymbol` / `ApiSymbolKind` / `ApiParam` / `ApiType` / `Visibility` / **`Position`** 取自 design「类型定义」，归一与位置分派口径取自 D10（措辞已由 **D43** 修订：位置分派仍只在 `emit`，但**类型词汇与位置词汇与短形写出点各只有一份**、判据只调用 `schema_type` + `signature_of`），取数面归属与「导出面」纪律取自 D19 / D19-a / D19-b（`extract_element` 是判据唯一取数面），**`extract` 的显式留白取自 D52**；用例按 AC-5（归一后契约 `input: string` vs 代码 `input: &str` 判一致）/ AC-9（判定单源共用归一）/ **AC-21（位置词汇唯一、短形写出点唯一、`optional` 归一不假阳）** / **AC-29（`extract` 显式留白）** 组织。proposal 未单列本文件测试，按 sidecar 惯例落本节。本节 `#### 待测功能` 共 **8** 条 = design「公共函数 / API」的 `extract/mod.rs` 段**五行导出**（`extract` / `extract_element` / `normalize_signature` / `schema_type` / `signature_of`）+ **三条备案行**（私有 `render` / 类型定义面的 `Position` / `Named` 口径行）——前五条计入 header 的 **5**，三条备案行不计入 59 且行为一律经公共面观测（MUST NOT 为测试放宽可见性）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `extract` Rust 分派 | 正向 | 元素 `path` 首条为 `crates/x/src/lib.rs` + 源含 `pub fn parse(&str) -> String` → 返回 1 条 `Function` 符号（`name == "parse"`、`params.len() == 1`、`returns` 为 `OwnedString`） | 单例 |
| `extract` TS 分派（AC-5） | 正向 | 元素 `path` 首条为 `src/views/x.tsx` + 源含 `export function load(): string` → 返回 1 条 `Function` 符号（`is_async == false`） | 单例 |
| `extract` 未知语言（异常） | 异常 | `path` 首条为 `README.md` / 无扩展名 / `x.py` / `path` 空 → 全返回空 `Vec`（不猜测、零 panic；与 AC-12 的 `bootstrap` 缺口一致） | 表驱动（4 形态） |
| `extract` 空源 / 语法错源（边界） | 边界 | `source = ""` → 空 `Vec`；Rust 语法错（缺右括号）/ TS 语法错 → 空 `Vec` 或部分符号，断言仅「不 panic、返回型可判」；`source` 为 100 KB 单文件 → 不 panic | 表驱动（3 形态） |
| `normalize_signature` 四种位置分派后判等（AC-5 / D10） | 正向 | 契约侧 `input: string` 与代码侧 `input: &str` 归一后**逐字相等**；返回位 `String` vs `&'static str` 归一后相等；`&[T]` vs `Vec<T>` 归一等；字段位 `String` 与 `Option<String>` 分别归一出 `String` / `optional<String>` | 表驱动（4 形态） |
| `normalize_signature` 参数名不参与 | 边界 | 同名同型但参数名不同（`fn(a: str)` vs `fn(timestamp: str)`）→ 归一结果相等；参数顺序不同（`fn(a: str, b: int)` vs `fn(b: int, a: str)`）→ 归一结果**不相等**（顺序是签名语义） | 表驱动（2 形态） |
| `normalize_signature` 空白与多行排列 | 边界 | 多行签名（4 行换行 + 缩进）与单行版本归一相等；类型间多空格 / 无空格两版本归一相等（参数名与空白不参与） | 表驱动（2 形态） |
| `normalize_signature` 零参 / 零返回 | 边界 | `fn()` 无参 → `"fn() -> <返回>"`；`returns = Unknown("")` → 归一含 `?` 前缀（`Unknown` 使归一仍可比对且显式留痕）；`params = []` + 返回零值 → 不 panic | 表驱动（3 形态） |
| `normalize_signature` 全 `ApiType` 十一值（Enum N+1） | 正向 | `Str` / `OwnedString` / `Int` / `Float` / `Bool` / `List(List(Str))` / `Map(OwnedString)` / `Optional(Named("X"))` / `Named("Y")` / `Outcome(Bool)` / `Unknown("$T")` 十一值各归一出一个规范短形，且十一个结果两两不等（值域封闭可辨） | 表驱动（11 值） |
| `normalize_signature` `is_async` 不参与（AC-5） | 边界 | 同一符号 `is_async = true` / `false` 两版本 → 归一结果相等（异步性不进签名串，由 `x-function` 的 `async` 键承载） | 表驱动（2 形态） |
| `normalize_signature` 深度嵌套（边界） | 边界 | `Optional(List(Optional(Map(Named("T")))))` 四层嵌套 → 归一串可判且不 panic；1000 个参数 → 不 panic（无栈溢出） | 表驱动（2 形态） |
| `Visibility` 四值 `ALL`（AC-5 / AC-9） | 正向 | `ALL.len() == 4`；`external` / `crateOnly` / `unused` / `testOnly` 四值 `as_str()` 逐值逐字；`ALL` 去重后长度不变 | 表驱动（4 值） |
| `ApiSymbolKind` 四值封闭（N+1） | 正向 | `Type` / `Trait` / `Enum` / `Function` 四值可辨；构造器**不单独成值**（`is_constructor("new")` 判而非新 kind）——断言四值集合不含 `Constructor` 类词汇 | 表驱动（4 值） |
| `extract_element` 整元素导出集（AC-9 / D19） | 正向 | Rust 元素（`<dir>/src/lib.rs` 入口，op 实现在私有子模块 `src/op.rs` 内且由入口 `pub use` 出线）→ 返回集**含**该 op（只看入口会漏判，此用例即「判据唯一取数面」的正例）；符号 `owner` / `visibility` 与 `rust::extract_crate` 逐字段相等 | 单例 |
| `extract_element` TS 走单文件（D19） | 正向 | TS 元素（`<root>/index.ts` 入口）→ 返回集等于对入口源跑 `ts::extract_file` 的结果（TS 侧不走模块树，D19 的分派表）；`.tsx` 入口同形不 panic | 表驱动（2 扩展名） |
| `extract_element` 入口读不到（异常） | 异常 | 入口路径不存在 / 入口是目录 / 入口不可读（以目录占位表达）→ 空 `Vec`（**不** panic、**不**伪造符号；缺口由 `apiEntryMissing` 报，判据侧零双报）；`entry = ""` → 空 `Vec` | 表驱动（4 形态） |
| `extract_element` 语言未知（边界） | 边界 | 入口为 `README.md` / 无扩展名 / `x.py` → 空 `Vec`（与 `extract` 的 `Unknown` 分支同口径，不猜测） | 表驱动（3 形态） |
| `extract_element` 与 `bootstrap` 同源（AC-12） | 边界 | 同一元素下 `extract_element` 的符号集与 `api::bootstrap` 草稿中列出的候选符号集**逐项相等**（草稿失真同因：两者共用同一取数面，MUST NOT 各走一套） | 单例（两向对拍） |
| `Language::Unknown => Vec::new()` 两处（AC-20 ⑥） | 边界 | 源码自证 `extract/mod.rs` 内该形态**恰 2 处**（`extract` 与 `extract_element` 各一），未知语言返回空集不猜测；两处之外零「猜成 Rust / TypeScript」的回落分支（`grep -c` 2 + 回落实体零命中） | 单例（源码自证） |
| `normalize_signature` 单点定义与消费面（AC-20 ①） | 边界 | 源码自证：`normalize_signature` 在产品 `.rs` 内**恰 1 处** `pub fn` 定义（本文件）；`check.rs` 内 `normalize_signature` **恰 1 处**消费、对 `rust::extract_crate` / `ts::extract_file` 的直连**零命中**（判据只经 `extract_element` 取数，MUST NOT 各走一套）；`grep -c 'fn normalize_signature'` 全 `api/` 目录 = 1 | 单例（源码自证 3 断言） |
| `schema_type` 位置分派三分支（**D43 / AC-21**） | 正向 | `{"type":"string"}` × `Position::Param` → `Str`（代码 `&str`）；× `Return` → `OwnedString`（代码 `String`）；× `Field` → `OwnedString`（字段位 `String`）；`{"type":"integer"}` → `Int`；`{"type":"number"}` → `Float`；`{"type":"boolean"}` → `Bool`；**同一片段在不同位置产出不同类型、同一位置幂等**（位置是唯一差异源，零第二份表） | 表驱动（6 片段 × 3 位置） |
| `schema_type` nullable 三分支（**D43 直接溯源**） | 边界 | `{"type":"null"}` → `Optional(Unknown(""))`（或等价内部形态）；`{"type":["string","null"]}` → `Optional(Str/OwnedString 按位置)`；`{"anyOf":[{"type":"string"},{"type":"null"}]}` → `Optional(..)`；`{"oneOf":[{"type":"integer"},{"type":"null"}]}` → `Optional(Int)`；三者**同位置下短形逐字相等**（三条写法必须归一，漏任一分支即 `Option<T>` 入参恒假阳）；对照 `{"type":["string"]}`（单元素非 null）→ **不**包 `Optional` | 表驱动（4 nullable 写法 + 1 反例） |
| `schema_type` 容器与信封（**D43**） | 正向 | `{"type":"array","items":{"type":"string"}}` → `List(OwnedString)`（items 一律 `Position::Field`）；`{"type":"object","additionalProperties":{"type":"integer"}}` → `Map(Int)`；`{"type":"object"}`（无 `additionalProperties` / 非对象型）→ `Unknown("object")` 类退化；`outcome` 包裹 → `Outcome(inner)`；`{"$ref":"#/$defs/A"}` → `Unknown("A")`（`ref_name` 取末段）；空 `{}` → `Unknown("")` 不 panic | 表驱动（6 形态） |
| `schema_type` 边界与异常片（**D43** 边界） | 边界 | `Value::Null` / `[]` / `"裸字符串"` / `数字` 四形态 → 全 `Unknown(..)` 且不 panic（显式留痕，归一仍可比对）；超长类型名（>1000 chars）→ 原样进 `Unknown`；未知 `type` 值（`"timestamp"`）→ `Unknown("timestamp")`；`{"type":["string","integer"]}`（多类型无 null）→ 可判退化不 panic | 表驱动（7 形态） |
| `signature_of` 与 `normalize_signature` 同词汇（**D43 / AC-21**） | 正向 | `api-op` fence `{"params":[{"name":"input","schema":{"type":"string"}}],"returns":{"type":"string"}}` 折出 `fn(str) -> String`；代码侧 `fn parse(input: &str) -> String` 经 `normalize_signature` 折出**同一串**（参数名与空白不参与）；两函数产出的字符串**逐字相等**（同一短形写出点，零第二份拼装） | 表驱动（3 签名对） |
| `signature_of` 的 `optional` 归一（**D43 关键反例**） | 边界 | 契约写 `params: [{ "name": "input", "schema": { "type": "string" }, "optional": true }]` + 代码写 `fn parse(input: Option<&str>) -> String` ⇒ 归一后**两侧相等、零 `apiSignatureMismatch`**（上一稿在该形态上恒假阳，是本用例的直接溯源）；`optional: false` 与缺省 `optional` → 同形（不因键缺省而漂移）；`optional: true` 的契约 vs 代码非 `Option` ⇒ **不相等**（真报 1 条，不吞并） | 表驱动（3 形态：误报排除 2 + 真报 1） |
| `signature_of` 缺键与空签名（**D43** 边界） | 边界 | 缺 `params` → 按空序列（`fn() -> …`）；缺 `returns` → 按 `()`（`fn(..) -> ()`）；两者都缺 → `fn() -> ()` 可判、不 panic；`params: []` + `returns: {"type":"null"}` → `Optional(..)` 形态可判；`x-function` 包裹形态（`{"x-function": {"params": …}}`）与裸签名形态**折出同一串** | 表驱动（5 形态） |
| `render` 短形十一值（**D43** 私有备案 / 经公开函数观测） | 正向 | 经 `normalize_signature` / `signature_of` 间接观测：`str` / `String` / `int` / `float` / `bool` / `list<T>` / `dict<T>` / `optional<T>` / `result<T>` / 裸名 / `?raw` 十一短形各在场且两两可辨；`Named` 裸名与 `Unknown` 的 `?` 前缀**形态不同**（显式留痕 vs 已知名）；对照反例：`check.rs` / `emit/{rust,ts}.rs` 内对 `ApiType` 的 `match` 臂零命中（不自拼短形） | 表驱动（11 值 + 源码自证） |
| `render` 保持私有（**D43 / AC-21 ②**） | 异常 | 源码自证：`grep -c "fn render(ty: &ApiType)"` = **1** 且 `grep -c "pub fn render(ty: &ApiType)"` = **0**（不升格；升格即「仅测试可见的导出」）；行为面：短形的可分辨性由 `normalize_signature` / `signature_of` 两公开函数覆盖（用例不经私有符号直调） | 单例（源码自证 2 断言） |
| `Position` 词汇唯一落点（**D43 / AC-21 ①**） | 异常 | 源码自证：`^pub enum Position` 在 `api/` 产品文件内**恰 1 处**（本文件）；`emit/rust.rs` / `emit/ts.rs` / `check.rs` 各 **0** 处（只 `use crate::api::extract::Position;`）；三值 `Param` / `Return` / `Field` 齐备且可穷举 `match`（无 `_` 兜底臂） | 单例（源码自证 4 断言） |
| `schema_type` / `signature_of` 调用面单点（**D43 / AC-21 ③⑤**） | 边界 | 源码自证：`pub fn schema_type` 与 `pub fn signature_of` 各恰 **1** 处（本文件）；`check.rs` 内对 `"anyOf"` / `"null"` 的**直接匹配零命中**（nullable 只经 `schema_type`）、`grep -cE "^fn (normalize_signature\|normalize_schema\|contract_signature)" check.rs` = **0**（零本地归一函数） | 单例（源码自证 4 断言） |
| `extract` 显式留白（**D52 / AC-29**） | 边界 | 源码自证三条：① 产品侧零调用者（`extract::extract(` 滤 `*_test.rs` 后零命中，**否定断言在案**）；② 留白句 **≥ 2** 处（模块 doc + 函数 doc 各一处「本期产品路径零调用者」）；③ `^pub fn extract\(` 恰 **1**（接口未删；**MUST 用带 `(` 的定形串**——裸 `pub fn extract` 会把 `extract_element` 一并命中）；行为面：`extract` 与 `extract_element` 在**同一单文件 Rust 入口**上符号集逐字段相等（留白不是死代码，接口语义仍正确） | 单例（源码自证 3 断言 + 行为对拍） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| Rust / TS 取数器 | **不 mock**：真实 `extract::rust::extract_file` / `extract::ts::extract_file`（分派语义即被测对象，替身会让「语言判错」不可见） | 分派用例 |
| 文件系统 | **零依赖**：`extract` 接 `source: &str`，零落盘 | 全部用例 |
| knip 报告 | **不 mock**：`extract` 本身不读 knip（由 CLI 读后经 `apply_knip_report` 注入，见 `extract/ts.rs` 章节） | —— |
| `schema_type` / `signature_of` / `Position` / 私有 `render` | **不 mock**：真实单点函数（短形词汇即被测语义，替身会让「判据自建第二份」不可见）；`render` 保持私有，行为只经两条公开函数观测 | 归一 / 短形用例 |
| 契约侧 `api-op` fence 原文 | **不 mock**：以 `serde_json::json!` 直构真实 `Value`（签名折形即被测语义），不经 `contract::parse` | `signature_of` 用例 |
| 时钟 / 网络 / 子进程 / 环境变量 | **零依赖**：`api/**` 零 spawn（AC-12） | —— |

---

### packages/desktop/src-tauri/crates/core/archi/src/api/extract/rust.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/extract/rust_test.rs

#### 待测功能

- `extract_file()`: `syn` 遍历 `pub struct` / `pub trait` / `pub enum` / `pub fn`（自由 + `impl` 方法）；`visibility` 四值判定（`pub mod` 链可达 → `External`、`pub(crate)` → `CrateOnly`、私有 `mod` 内 `pub` → `CrateOnly`、`#[cfg(test)]` 树 → `TestOnly`）；AST 归一签名与行号（公共函数 / API）

- `extract_crate()`: 整包抽取——从实现入口出发走模块树（`mod x;` → `<dir>/x.rs` / `<dir>/x/mod.rs`；**`#[path = "..."] mod x;` 按属性值定位**，D44 缺口①），并用 crate 根的 `pub use` 白名单把私有模块内经 re-export 出线的名字升回 `External`（**`pub use <mod>::*` 的通配前缀参与升格**，D44 缺口②——否则 `pub use api_gen::*;` 形态全部漏采）；模块定位失败 → 跳过该子树（宏生成 `mod` 的已知近似，零伪造）；**`#[path]` 的读盘面与写盘面同一条限根纪律（D49）**：反斜杠归一架在折叠之前、按段折叠（`""` / `.` 跳过、`..` 弹栈、栈空即拒）、`join` 后复核目标仍在观察根之下（`starts_with(root)`），任一条不成立即**不命中该子树**（零新错误变体、零 panic）（公共函数 / API）

> 说明：`ApiSymbol` / `ApiSymbolKind` / `ApiParam` / `ApiType` / `Visibility` 取自 design「类型定义」，取数面归属与「导出面」纪律取自 D19 / D19-a（inherent 方法按接收类型可达性判定；Rust 侧 `visibility_of` 对私有项返回 `None`，只有 `pub` / `pub(crate)` 入表），**`#[path]` 模块定位与 `pub use <mod>::*` 通配升格取自 D44**（缺任一条则试点自身的 `$defs` 类型恒报缺失、基线由 13 变 14），**读盘面的限根三步取自 D49**；用例按 AC-5 / AC-9 / **AC-22（`apiMissing` 的取数面前提）** / **AC-26（`#[path]` 读盘面限根）** 与 proposal「测试文件」表的 Rust 抽取条目（多行签名 / `pub` 于私有 `mod` / `pub(crate)` / `#[cfg(test)]` 树 / glob 导入 / 同名符号 / 构造器识别）组织。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `extract_file` 四类声明抽取（AC-5） | 正向 | 源含 `pub struct S;` / `pub trait T {}` / `pub enum E { A }` / `pub fn f() -> String` → 四条符号，`kind` 分别 `Type` / `Trait` / `Enum` / `Function`，`name` 逐字，`line` 均为声明行（1 起始） | 表驱动（4 形态） |
| `extract_file` 多行签名 AST 归一（proposal 反例在案） | 正向 | 一个跨 5 行的 `pub fn f(\n  a: &str,\n  b: Vec<u8>,\n) -> Result<String, String> {` → 抽取的 `params.len() == 2`、`returns` 为 `Outcome(OwnedString)`；对照「按行截取」的 `grep` 反例只得到残缺参数（断言真实抽取与「截半」结果不相等） | 单例（正 / 反两值对照） |
| `pub mod` 链可达 → `External`（D19） | 正向 | `pub mod a { pub fn f() {} }` 顶层可达 → `External`；`pub struct S` 顶层 → `External` | 表驱动（2 形态） |
| 私有 `mod` 内 `pub`（proposal 要素） | 边界 | `mod inner { pub fn f() {} }` → `CrateOnly`（`pub` 不等于 external） | 单例 |
| `pub(crate)` 判据（proposal 要素） | 边界 | `pub(crate) fn write_report(..)` → `CrateOnly`；`pub(super)` / `pub(in crate::x)` → `CrateOnly` 或 `External` 可判（断言不 panic 且值域在四值内，禁第五值） | 表驱动（3 形态） |
| `#[cfg(test)]` 树 → `TestOnly`（proposal 要素） | 边界 | `#[cfg(test)] mod tests { pub fn helper() {} }` → `TestOnly`；`#[cfg(test)] pub struct S;`（顶层）→ `TestOnly`；同文件另含 `pub fn prod()` → 该条仍 `External`（互不污染） | 表驱动（3 形态） |
| `impl` 方法 owner 与可达性（D19） | 正向 | `impl S { pub fn m(&self) {} }` 且 `S` 为 `External` → 符号 `owner = Some("S")`、`kind = Function`、`visibility = External`；`impl` 内 `pub fn` 但接收类型为私有 → `CrateOnly` | 表驱动（2 形态） |
| 构造器如实抽名（不过滤，proposal 要素） | 边界 | `impl S { pub fn new() -> Self }` → 抽取结果**含** `new`（取数器不过滤；过滤只在 `check` / `emit` 侧经 `contract::is_constructor`，D11） | 单例 |
| glob 导入与同名符号（proposal 要素） | 边界 | `use x::*;` + 文件内 `pub struct S;` → 抽取 1 条（glob 导入不产生符号）；同名符号两处声明（`pub fn f` 与 `pub fn f`，不同 `mod`）→ 两条各自在案、`owner` / `line` 可辨（不静默去重） | 表驱动（2 形态） |
| `ApiType` 映射表（AC-5 / D10） | 正向 | `&str` → `Str`；`String` / `&'static str` → `OwnedString`；`i64` / `u32` / `usize` → `Int`；`f64` → `Float`；`bool` → `Bool`；`&[T]` / `Vec<T>` → `List(..)`；`Option<T>` → `Optional(..)`；`Result<T, String>` → `Outcome(..)`；`BTreeMap<K, V>` → `Map(..)`；`Custom` → `Named("Custom")` | 表驱动（11 形态） |
| 无法归类类型（异常留痕） | 异常 | `impl Trait` 参数 / `dyn Trait` / 关联类型 `<T as Tr>::Item` / 元组 `(A, B)` → 归 `Unknown(原文)` 且原文非空（归一仍可比对，D10） | 表驱动（4 形态） |
| 语法错源 / 空源（异常） | 异常 | `""` → 空 `Vec`；缺右括号 / 非法 token → 空 `Vec`（`syn::parse_file` 失败即不猜测）；`source` 为 100 KB → 不 panic | 表驱动（3 形态） |
| 行号边界（int 边界） | 边界 | 声明在首行 → `line == 1`；第 1000 行 → `line == 1000`；文件首部含 500 行注释 → `line` 仍准确（`proc-macro2` 的 `span-locations` 生效，缺特性则行号恒 0——断言 `line != 0`） | 表驱动（3 形态） |
| 宏 / 属性干扰（边界） | 边界 | `#[derive(Deserialize)] pub struct S;` → 抽取 1 条（属性不产生符号）；`macro_rules! m { .. }` → 零符号；`#[serde(rename_all = "camelCase")]` 值字符串不被当符号 | 表驱动（3 形态） |
| 大文件（边界） | 边界 | 500 个 `pub fn` → 符号数 500、无重复、无截断 | 单例 |
| `crate_name` 形参（边界） | 边界 | `crate_name = ""` / 含 emoji → 不 panic、抽取结果不受影响（形参只供内部路径判定，不改符号集） | 表驱动（2 形态） |
| `extract_crate` 沿入口走模块树（AC-9 / D19） | 正向 | `<dir>/src/lib.rs` 含 `pub mod op;`，`<dir>/src/op.rs` 含 `pub fn run()` → `extract_crate` 返回集**含** `run`（只抽入口文件会漏；判据的「代码导出集」因此不缺项）；返回集 ⊇ `extract_file(入口源)` 的结果 | 单例 |
| `extract_crate` `mod.rs` 形态与内联 `mod`（D19） | 正向 | `<dir>/src/op/mod.rs` 形态（`mod op;` 命中 `<dir>/src/op/mod.rs`）→ 子模块符号在案；入口内联 `pub mod a { pub fn f() {} }` → `f` 在案（同 `extract_file` 口径，零重复） | 表驱动（2 形态） |
| `extract_crate` crate 根 `pub use` 白名单升格（D19 / S-10 同源留白） | 边界 | 私有模块 `mod inner` 内 `pub struct S`，入口写 `pub use inner::S;` → `S` 的 `visibility == External`（经 re-export 出线即跨元素可达）；同文件**未**被 re-export 的 `pub fn helper` → 仍 `CrateOnly`；白名单不改变符号集本身（只升格，不增删） | 表驱动（2 形态 + 计数对拍） |
| `extract_crate` 模块定位失败即跳过（异常 / 零伪造） | 异常 | `mod x;` 但 `<dir>/src/x.rs` 与 `<dir>/src/x/mod.rs` 均不存在 → 跳过该子树、不 panic、不伪造符号；宏生成 `mod`（`macro_rules!` 展开出 `mod`）→ 同形跳过（已知近似显式留痕，不猜测） | 表驱动（2 形态） |
| `extract_crate` 入口缺失 / 非 Rust（异常） | 异常 | 入口路径不存在 / 入口为 `index.ts` → 空 `Vec`（不猜测语言；语言分派在 `extract_element` 侧，本函数只处理 Rust 形态）；`entry = ""` → 空 `Vec` | 表驱动（3 形态） |
| `extract_crate` 深度与规模（边界） | 边界 | 5 层嵌套模块链 → 全链符号在案、不 panic 不截断；300 个 `pub fn` 分布在 30 个模块 → 符号数 300（零重复、零丢失）；`mod` 自引用环（`a` → `b` → `a`）→ 不死循环（已访问集合去重） | 表驱动（3 形态） |
| `extract_crate` ∩ `extract_file` 语义同源（边界） | 边界 | 单文件 crate（入口即全部实现、无 `mod`）→ `extract_crate` 与 `extract_file(入口源)` 的符号集逐字段相等（整包抽取是同口径的超集形态，MUST NOT 出现两套 `visibility` 判据） | 单例（两向对拍） |
| Rust 侧走 AST 不截行（AC-20 ②） | 边界 | 源码自证 `rust.rs` 内 `syn::parse_file` **恰 3 处**（`extract_file` / `extract_crate` / 内层模块解析）、`regex` / `Regex` **零命中**（正则抽签名即「截半」缺陷的复发路径）；多行签名用例断言 `params.len()` 全取，且「按行截取得 0 个参数」的反例在案 | 单例（源码自证 + 用例） |
| `visibility` 可判值域按语言分列（AC-20 ④，Rust 侧） | 边界 | `rust.rs` 的三值产出点合计**恰 6 处**（`External` / `CrateOnly` / `TestOnly`）；`Visibility::Unused` 在 `rust.rs` **零命中**（`unused` 由调用方按引用事实降级，不造第二套死代码扫描）；`TestOnly` **由 `#[cfg(test)]` 子树语法判定**而非文件名（`grep` 该文件内零 `.test.` 字面量） | 表驱动（3 值 + 源码自证） |
| `#[path]` 模块定位（**D44 缺口① / AC-22 ③**） | 正向 | 入口 `<dir>/src/lib.rs` 含 `#[path = "../../archi.i.rs"] mod api_gen;`，该文件含 `pub struct LocalArchiCheck;` → `extract_crate` 返回集**含** `LocalArchiCheck`（按属性值定位；不认 `#[path]` 时整份不进符号集，试点自身的 `$defs` 类型恒报缺失）；对照 `mod op;` 的常规两候选（`<dir>/src/op.rs` 与 `<dir>/src/op/mod.rs`）行为不变 | 表驱动（2 形态） |
| `pub use <mod>::*` 通配升格（**D44 缺口② / AC-22 ④**） | 正向 | 私有模块 `mod api_gen` 内 `pub struct LocalArchiCheck;` + `pub fn file_stem(..)`，入口写 `pub use api_gen::*;` → 两者的 `visibility` **均 = `External`**（`flatten_use_tree` 的 `UseTree::Glob` 分支不再直接丢弃，通配前缀参与升格）；对照未 re-export 的 `pub fn helper` → 仍 `CrateOnly`；白名单**只升格不增删**（符号计数与未升格时相等） | 表驱动（2 形态 + 计数对拍） |
| `#[path]` 读盘面：反斜杠越根不命中（**D49 / AC-26 ①**） | 异常 | `#[path = "..\..\..\..\outside\x.rs"] mod x;`（五个反斜杠分隔的 `..`，段数足够不弹空栈）→ 该 `mod` 子树**零符号**，且**仓外哨兵文件未被读取**（用哨兵内容断言「未读」）；改动前该形态会被当作单段名、`Path::join` 纯拼接把 `..` 带到运行期解析 ⇒ 归一是正面证据 | 单例（行为 + 哨兵） |
| `#[path]` 读盘面：前向 `/` 形态零回归（**D49 / AC-26 ②**） | 边界 | `#[path = "../../archi.i.rs"]`（试点真实形态）→ 符号集与改动前**逐字段相等**（折叠原形保留：`stack` 初值即上界，前向 `/` 弹不出根）；`#[path = "../x.rs"]` 与 `#[path = "./x.rs"]` 两形态指向同一文件时符号集相等 | 表驱动（3 形态 + 逐字段对拍） |
| `#[path]` 读盘面：未知形态零 panic（**D49 / AC-26 ③**） | 边界 | `#[path = ""]` / `#[path = "."]` / `#[path = "../"]` / `#[path = "a//b.rs"]`（空段）/ `#[path]` 无值 → 各 `None` 或原形，**零 panic、零 `ApiError`**（越根走「不命中该子树」通路，与「模块定位失败即跳过」同口径）；源码自证 `extract/rust.rs` 内 `ApiError::` **零命中**（零新错误面） | 表驱动（5 形态 + 源码自证） |
| `#[path]` 限根三步的机械判据（**D49** 源码自证） | 边界 | `locate_declared_module` 体内同时出现 `replace(` 与 `"/"`（**人工核对顺序**：归一在 `for segment in … split` 之前）；`grep -cF "starts_with(root)"` **≥ 1** 且该复核在 `read_to_string` **之前**；`join` 后复核与 `scan/rust.rs` 的 `normalize_rel` / `docs.rs` 的 `resolved_within` 同一条纪律（三处对照留痕） | 单例（源码自证 3 断言） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `syn` / `proc-macro2` | **不 mock**：真实 AST 解析（多行签名与行号即被测语义，任何替身都会抹掉「截半」判据） | 全部用例 |
| 文件系统 | **`extract_file` 零依赖**（接 `source: &str`，零落盘）；**`extract_crate` 不 mock**：真实 tempdir 落 `<dir>/src/lib.rs` 与子模块文件（模块树遍历即被测语义，替身会让「模块定位失败即跳过」不可见），装置沿本 crate 自建 `TempDir` RAII（无 dev-dependency） | 模块树 / 白名单 / 定位失败用例 |
| `#[path]` 越根的仓外哨兵 | **不 mock 文件系统**：tempdir 外再建一个平级 tempdir 放哨兵文件（内容唯一），断言内容零读取；**MUST NOT** 用「路径字符串形态」代替「文件未被读」的行为判据 | 反斜杠越根不命中用例 |
| 时钟 / 网络 / 子进程 / 环境变量 | **零依赖** | —— |

---

### packages/desktop/src-tauri/crates/core/archi/src/api/extract/ts.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/extract/ts_test.rs

#### 待测功能

- `extract_file()`: `oxc` 遍历**只抽导出面**（D19-a）——仅 `Statement::ExportDeclaration` 一条分支（内部交 `collect_declaration` 分类）：`export function` → 操作（唯一操作判定，MUST NOT 从 `const` 初始化式推函数）；`export type` / `interface` / `class` / `enum` → 类型面；`export const` 三分类（`memo(...)` → 组件留白 / 对象与 Proxy 字面量 → 排除 / **箭头函数 → 不作操作**）；`export {…}` / `export * from` → 零符号（透传由契约 `api-reexport` 表达）；六个非导出顶层分支与 `export default` **均不产出符号**（D19-a / S-9）（公共函数 / API）
- `apply_knip_report()`: 把 knip 报的未使用导出降级为 `Visibility::Unused`；`None` 即不降级；不造第二套死代码扫描。**本期未接线（D39）**：产品路径**零调用者**，`Visibility::Unused` 在**生产面不可达**——这是**显式留白**而非疏漏；用例继续测它（「接口正确」的证据），接线随首个 TS bootstrap 变更（公共函数 / API）

> 说明：`ApiSymbol` / `ApiType` / `Visibility` 取自 design「类型定义」，取数面纪律（只抽导出面）取自 **D19-a**、TS 侧 `visibility` 的本期值域与留白口径取自 **D19-b**、**knip 本期不读**取自 **D39**（`apply_knip_report` 是「已实现但未接线」的预留接口，`Unused` 生产面不可达——模块 doc MUST 写明，MUST NOT 用「已实现」掩盖），`export const` 分类口径取自 D19 / proposal §9（`export const` 里零箭头函数实测 0/132）；用例按 AC-5 / AC-6 / AC-9 / AC-20 与 proposal「测试文件」表的 TS 抽取条目组织。**两条硬约束**：① 箭头函数 const **不**产出 `Function`（S-9；`export const` 箭头是「从初始化表达式推断函数」的启发式，spec `:294` 已明文禁）；② 文件内 `visibility` 的**生产面**值域 ⊆ `{External}`（`Unused` 的唯一产出点在被零调用的 `apply_knip_report` 内，故生产面不可达；接口面经该函数可产出 `Unused`，两值域 MUST NOT 混谈），`CrateOnly` / `TestOnly` **本期留白**（判据需跨文件 import 反向事实，取数器只吃单文件 AST）——**MUST NOT 以「文件名含 `.test.`」充当 `testOnly` proxy**（`*.spec.*` / `__tests__/**` / `fixtures/**` 文件名族同禁，`.test.` 说的是「被测试引用」不是「文件名」，二者不可互换）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `extract_file` 的 `export function` → 操作（AC-5） | 正向 | `export function load(a: string): string { … }` → 1 条 `Function`；`export async function f(): Promise<void>` → `is_async == true`；`export function* g()`（生成器）→ 抽取形可判不 panic | 表驱动（3 形态） |
| `extract_file` 的 `export type` / `interface` / `class` → 类型（AC-5） | 正向 | `export type A = string` / `export interface B { x: number }` / `export class C {}` → 三条全 `kind = Type`、`name` 逐字 | 表驱动（3 形态） |
| `export const = memo(...)` → 组件留白（不产出操作） | 边界 | `export const View = memo(() => null)` → 该 const **不**产生 `Function` 符号（本期 `api = none` 形态，proposal §9）；`export const View = memo(function V() {})` → 同上（不因内层函数改判） | 表驱动（2 形态） |
| `export const` 对象 / Proxy 排除 | 边界 | `export const OBJ = { a: 1 }` → 零符号或 `Unknown`（不产出 `Function`）；`export const P = new Proxy({}, {})` → 同上 | 表驱动（2 形态） |
| 箭头函数 const 不作操作（D19-a 裁定② / S-9） | 异常 | `export const f = (a: string) => a` → **MUST NOT** 产出 `Function` 符号（proposal 实测 `export const` 里零箭头函数；「从初始化表达式推断函数」是 spec `:294` 明文禁的启发式）；`export const g = async (x: string): Promise<string> => x` / `export const h = function inner() {}`（函数表达式）→ 同形零 `Function`；断言返回集中 `f` / `g` / `h` 均不作为操作出现（反例在案，与实现在盘相反时由实现删箭头分支转绿） | 表驱动（3 形态） |
| `export {…}` / `export * from` 透传 | 边界 | `export { a, b } from './x'` / `export * from './y'` / `export type { T } from './z'` → 零操作符号（本期透传不产出操作，proposal §9 实测 0 例）；与 `api-reexport` fence 的表达面无关（后者归契约） | 表驱动（3 形态） |
| 非导出声明不抽（D19） | 边界 | `function f() {}` / `const x = 1` / `interface I {}`（全无 `export`）→ 零符号（只抽导出面） | 表驱动（3 形态） |
| `Visibility` 本期值域（D19-b / D39 留白口径） | 正向 | **生产面**：`extract_file` 的产出值域 **⊆ `{External}`**（顶层 `export` → `External`；同文件只有 `export` 无引用者 → 仍 `External`）；**断言产出集中不出现 `CrateOnly` / `TestOnly` / `Unused`**（`CrateOnly` / `TestOnly` 本期留白、`Unused` 的唯一产出点在被零调用的 `apply_knip_report` 内 ⇒ 生产面不可达）；**接口面**（另一断言面）：把 `extract_file` 的输出喂给 `apply_knip_report` 后值域可扩到 `{External, Unused}`——两值域 MUST NOT 混为一谈（前者是没跑过降级接口的实况，后者是接口正确性证据） | 表驱动（2 值域 × 2 断言） |
| `Visibility` `.test.` 文件名不作 proxy（D19-b 反例在案） | 异常 | 源文件名 `foo.test.ts` / `foo.spec.ts` / `__tests__/a.ts` / `fixtures/b.ts`，内容均为 `export function f() {}` → 四者 `visibility` **全为 `External`**（文件名与引用关系无必然联系，用文件名冒充 `testOnly` 是伪判据）；`extract_file` 的入参面**无** `is_test_file` 之类形参（源码自证：签名恰 `(file, source)`，零 `` `.test.` `` 字面量 / 零 `contains(".test")` 判据） | 表驱动（4 文件名形态 + 源码自证） |
| `Visibility` 四值词表封闭（测词表面，不测产出面） | 边界 | `Visibility::ALL.len() == 4`、`as_str()` 四值逐字（这一面只在 `extract/mod.rs` 章节断言词表封闭）；**MUST NOT** 在本文件断言「取数器产得出 `TestOnly`」——本期不产出（D19-b） | 单例（一条否定断言） |
| `apply_knip_report` 降级（AC-6 / R-10 / D39 接口面） | 正向 | knip 报告列出 `f` 为未使用导出 → 该符号 `visibility == Unused`；未列出的符号保持原值（只降级不升级）。**本用例证明的是「接口正确」，不是「已接线」**（D39） | 表驱动（2 形态） |
| `apply_knip_report` 零产品调用者（**D39** 显式留白） | 边界 | 源码自证：`crates/**` 产品 `.rs`（滤 `*_test.rs`）内对 `apply_knip_report(` 的调用**零命中**（只有定义）；`api/mod.rs` 与 `crates/tools/archi-api-gen/src/main.rs` 内 `knip` 一词**各零命中**；`Visibility::Unused` 的产出点**唯一**（本函数体内），故生产面不可达；两处 doc（`extract/mod.rs` 的 `Visibility` 枚举 doc 与 `ts.rs` 模块 doc）**各写明**「本期生产面不产出 `Unused`（knip 未接线）+ `apply_knip_report` 是零调用者的预留接口 + 将来所需输入」——`MUST NOT` 用「已实现」掩盖；`bootstrap` 的草稿文本内**零 `unused`** 词 | 单例（源码自证 5 断言 + 行为对照） |
| `apply_knip_report` 无报告（边界） | 边界 | `None` → 符号集与 `visibility` **逐字段不变**（不降级）；`Some({})`（空报告）→ 同样不变 | 表驱动（2 形态） |
| `apply_knip_report` 报文形态异常（异常） | 异常 | `Some` 非对象（数组 / 字符串 / `null`）/ 缺未使用导出字段名 / 字段为字符串而非数组 三形态 → 不 panic、符号集不变（不猜测报文结构；报文版本兼容留给首个 TS bootstrap，待决问题 5） | 表驱动（3 形态） |
| 参数与返回类型（AC-5） | 正向 | `(a: string, b?: number) => R` 形态的 `export function` → `params[0].ty = Str`、`params[1].optional == true`、`params[1].ty = Float`（**与 `check::normalize_schema` 的判定单点同词汇**：参数位 `string` → `str`、`number` → `float`；`OwnedString` / `Int` 是 Rust 侧形态，TS 取数器不产）；`Promise<string>` 返回 → `returns = Outcome(Str)` 与 `is_async == true` 组合可判 | 表驱动（2 形态） |
| `source_type` 分派（边界） | 边界 | 同一源在 `file = "x.ts"` 与 `"x.tsx"` 两形态（含 JSX 的源）→ `.tsx` 不 panic 且符号集可判；`file = "x.js"` / `"x.jsx"` → 同样抽取 | 表驱动（4 扩展名） |
| 语法错源 / 空源（异常） | 异常 | `""` → 空 `Vec`；未闭合括号 / 非法 token → 空 `Vec` 或部分符号（不 panic）；100 KB 单文件 → 不 panic | 表驱动（3 形态） |
| 行号与规模（边界） | 边界 | 首行 `export function` → `line == 1`；第 500 行 → `line == 500`；500 个 `export function` → 符号数 500、无重复 | 表驱动（3 形态） |
| TS 取数面 = 导出面（AC-20 ⑤） | 边界 | 源码自证：`Statement::ExportDeclaration` **恰 1 条**分支（内部交 `collect_declaration` 分类）；六个非导出顶层分支（`FunctionDeclaration` / `ClassDeclaration` / `VariableDeclaration` / `TSTypeAliasDeclaration` / `TSInterfaceDeclaration` / `TSEnumDeclaration`）**零命中**；`ArrowFunctionExpression` **零命中**；`ExportDefault` **零命中**（本期留白，对外名由导入方决定） | 单例（源码自证 4 断言） |
| `visibility` 可判值域按语言分列（AC-20 ④，TS 侧留白） | 边界 | 源码自证 `ts.rs` 内 `CrateOnly` / `TestOnly` **零命中**（两值本期留白）；`pub fn apply_knip_report` **恰 1 处**（`unused` 的单一降级来源）；`External` 的产出点即 D19-a 的导出面过滤（不额外计数） | 单例（源码自证 3 断言） |
| 文件名族不作 proxy（AC-20 ⑥ / D19-b 反例在案） | 异常 | 产品侧 `ts.rs` 内 `.test.` / `.spec.` / `__tests__` / `fixtures` 四族**零非注释命中**；`used_by` 在产品 `.rs` 内**零命中**（本期留白，需 import 反向图）；把 `foo.test.ts` 送入 `extract_file` → 符号 `visibility` 仍为 `External`（与 `.test.` 文件名用例同源，此处只作产品侧自证） | 单例（源码自证 + 行为对照） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `oxc_parser` / `oxc_allocator` / `oxc_ast` | **不 mock**：真实 AST 解析（导出面过滤与三分类判定即被测语义） | 全部用例 |
| knip 报告 | **入参直传**：`apply_knip_report` 接 `Option<&serde_json::Value>`，用例以 `serde_json::json!` 构造（**不 spawn `knip`**——进程边界由 CLI 侧承担，`api/**` 零 spawn，AC-12） | 降级 / 无报告 / 异常报文用例 |
| 文件系统 | **零依赖**：`extract_file` 接 `source: &str`（`file` 形参只作 `source_type` 分派与行号语境，**不读盘**；故 `.test.` 文件名用例零落盘即可驱动） | 全部用例 |
| 跨文件 import 反向图 | **不引入（本期留白）**：`crateOnly` / `testOnly` 的判据需要它，本 change 明确不产出（D19-b）；用例以「**生产面**值域 ⊆ {External}」+「`.test.` 文件名不作 proxy」两向断言表达留白，**MUST NOT** 为跑通用例而注入假的引用事实 | `Visibility` 三行用例 |
| 时钟 / 网络 / 子进程 / 环境变量 | **零依赖** | —— |

---

### packages/desktop/src-tauri/crates/core/archi/src/api/emit/mod.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/emit/mod_test.rs

#### 待测功能

- `render_target()`: 按 `EmitTarget.kind` 分派到 `emit::rust` / `emit::ts`（单契约粒度；批量入口在 `api::render_all`）；产出文本、零写盘；载元类型只引用 `ApiMeta`（不另立第二个）；分派前对「`properties` 全为函数却未标 `x-class`」发一次 stderr 警告（每元素恰一次；D27）（公共函数 / API）

<!-- design.md「公共函数 / API」：本文件恰一行 `emit::render_target`。`EmitTarget`（**三字段**，D58）与 `ModuleRef` 是「类型定义」两行（D40 / D41 / D58 的唯一承载形态），按本设计「待测功能条目来源约定」改列类型定义面。 -->

> 说明：`EmitTarget`（**三字段** `kind` / **`form`** / **`modules: BTreeMap<String, ModuleRef>`**——`path` / `element_root` 已按 **D58** 删除，产品面零读取者）与 `ModuleRef`（`name` / `module_path`）取自 design「类型定义」与 **D40 / D41 / D58**（`form` 与 `modules` 两字段都只为「渲染器零形态推导、零 FQN 派生」而存在；`form` 由调用方经 `paths::entry_form` 求出、`modules` 由调用方从元素表逐条解析）；载元单点取自 D12，`$comment` / 头注释警示取自 D13，`x-class` 两判据的落点分工取自 D27 / 事实校准 16（**错误**进 `schema::verify_defs`、**警告**落本文件的 `render_target`，不进 `ArchiApiFindingKind`、不进 `apiFindings`、不改 `--check` 退出码）；用例按 AC-5（两侧对等）/ AC-11（透传两值只查表）/ AC-12（出字符串零写盘）/ AC-17（语言无关纪律的边界：语言特定只落 `emit/{rust,ts}`）/ **AC-34 ①（三字段面）** 组织。**specifier 只由 `form` 与 `ModuleRef.name` 决定**，无第三条输入。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `render_target` Rust 分派（AC-5） | 正向 | `kind = GeneratedKind::Rust` + 试点契约 → 返回文本含 `pub struct LocalArchiCheck;` 与 `pub fn new()`，不含 TS 语法（`export interface` / `as const`） | 单例 |
| `render_target` TS 分派（AC-5） | 正向 | `kind = GeneratedKind::TypeScript` + 同一契约 → 返回文本含 `export interface LocalArchiCheck`，不含 Rust 语法（`pub struct` / `impl`） | 单例 |
| `render_target` 两侧同一契约键集一致（AC-5） | 正向 | 同一契约（2 类型 + 1 op）经两个 `kind` 各渲染一次 → 两文本出现的类型名集合与 op 名集合**逐项相等**（可枚举断言，不比文本序） | 单例（两侧对拍） |
| 单契约粒度（边界） | 边界 | 一个 `EmitTarget` 只渲染一个契约（不产 `INDEX.toml`、不产 `.schema.json`——那是 `render_all` 的职责，AC-12）；**specifier 只由 `form` 与 `ModuleRef.name` 决定**，无第三条输入（`element_root` 已按 D58 删除，MUST NOT 再写「元素根影响 TS specifier 形态语境与诊断定位」这类读法——经不起核对） | 单例 |
| `EmitTarget` 三字段面（**D58 / AC-34 ①**） | 正向 | `EmitTarget { kind, form, modules }` **三字段**齐备且可构造；`form` 直接取自入参（`EntryForm::Crate` / `SubdirModule` / `SingleFile` 三值均可传入），本文件**零形态推导**；`modules` 为 `BTreeMap<String, ModuleRef>`，`ModuleRef { name, module_path }` 两字段可独立读取（TS 侧只读 `name`、Rust 侧只读 `module_path`）；源码自证 `pub element_root` / `pub path: PathBuf` / `use std::path::PathBuf` 在该文件各 **0** 命中、`pub kind` / `pub form` / `pub modules` 各 **1** | 表驱动（3 形态 + 字段面 6 断言） |
| `EmitTarget` 删两字段零行为影响（**D58 / AC-34 ①** 行为面） | 边界 | 同一契约 + 同一 `{ kind, form, modules }` 的渲染产物**逐字节相等**（判据力：`path` / `element_root` 若被产品面读取，删后行为不可能不变）；产品面读 `target.{kind,form,modules}` 的行**恰 3**（`emit/mod.rs` 的 `.kind`、`emit/rust.rs` 的 `.modules`、`emit/ts.rs` 的 `.form` 与 `.modules` 同行 ⇒ 判据取**行数不取串数**）、读 `target.{path,element_root}` **0 行**。**判据面封闭在 `emit/` 三个产品文件内**（`include_str!` 三份），不用全仓 `grep`——`EmitTarget` 的定义与消费面只在本层，全仓扫描会让任何名为 `target` 的无关局部变量进来干扰计数 | 单例（源码自证 2 断言 + 逐字节对拍） |
| `form` 由调用方决定、渲染器零推导（D41 反例在案） | 异常 | 源码自证：`emit/` 内**零** `entry_form_of`、**零** `rsplit('.')`、**零** `contains('.')` 形态推导分支（形态只从 `EmitTarget.form` 读）；把同一契约的 `form` 从 `Crate` 改成 `SingleFile` → 本文件分派到的输出随之改变（证明形态不是渲染器自算的）；`render_target` 内部零 `paths::entry_form` 调用（那需要盘上事实，渲染器零 IO） | 单例（源码自证 4 断言 + 行为对拍） |
| `modules` 查不到目标 FQN 的退化（D40 边界） | 边界 | 契约含一项 `api-reexport` 而 `EmitTarget.modules` **不含**该目标 FQN → 渲染结果只留 `// 透传自 <FQN>` 注释、**不产 specifier**（`MUST NOT` 退化到 FQN 末段——那正是 D40 要消灭的第二套命名）；`modules` 为空表 + 零透传条目 → 输出与 `modules` 非空时逐字相等（无透传即不参与） | 表驱动（2 形态） |
| 渲染异常面（异常） | 异常 | 契约含泛型声明（`$defs` 值含 `"typeParameters"` 或形状推导为泛型）→ `Err(ApiError::Contract(..))`（泛型零支持，AC-5）；`$defs` 值缺失 `type` 且无 `$ref` / `x-class` / `enum` → 判定可判（`Err` 或退化为 `Unknown` 文本，断言不 panic） | 表驱动（2 形态） |
| 空契约（边界） | 边界 | 零类型零 op → 返回空串或仅头注释（`render_all` 侧据此跳过 `.i.*`，事实校准 6）；断言返回值不 panic 且不含 `pub use` | 单例 |
| 零写盘（AC-12） | 边界 | 调用前后 tempdir 全量字节快照相等（`render_target` 纯出 `String`） | 单例 |
| 载元单点（D12 / AC-17） | 边界 | 两路渲染的头注释都含 `archi-api <CARGO_PKG_VERSION>` 与 `common.json@sha256:<12 位>`；断言不存在第二个载元类型（以「同一 `ApiMeta` 实例传入两路，两处摘要文本逐字相同」表达） | 单例（两侧对拍） |
| `x-class` 反向警告：未标 + 全函数 → 警告（AC-5 / D27） | 正向 | `$defs` 条目 `properties` 非空且**每项**都含 `x-function`、`x-class` 缺省（或 `false`）→ `render_target` 发一行 stderr，文案含契约 `file`、类型名与该提示语义；**返回值仍为 `Ok`**、`--check` 退出码不受影响 | 单例 |
| `x-class` 反向警告：已标 / 非全函数 → 零警告（D27） | 边界 | `x-class: true` + 全 `x-function` 的 `properties` → 零警告（已标即意图明确）；未标但 `properties` 含非函数项（字段 `type: "integer"`）→ 零警告（函数指针结构体是合法意图，不提示）；`properties` 为空 / 缺省 → 零警告 | 表驱动（4 形态） |
| `x-class` 警告单点、不双报（D27 / 事实校准 16） | 边界 | 同一元素经 `render_target` 渲染时警告**恰一次**（不是 rust / ts 两侧各一次）；源码自证 `grep -rn "eprintln!" src/api/emit/` 恰一处且只在 `emit/mod.rs`（`rust.rs` / `ts.rs` 零 `eprintln!`） | 单例（行为 + 源码自证） |
| `x-class` 警告不进报告面（D27） | 异常 | 触发警告的输入下：返回集零 `ArchiApiFinding`（`ArchiApiFindingKind` 仍十值、MUST NOT 扩第十一值）、`apiFindings` 零条目、`INDEX.toml` 文本与无警告输入**逐字相等**（警告对生成物零影响） | 单例（三向断言） |
| `x-class` 带属性属错误侧（D27 反例） | 异常 | `x-class: true` + `properties` 非全函数 → 契约已在 `schema::verify_defs` 判错（`ApiError::Contract`），故 `render_target` **不收**到该输入（`load_contracts` 已中止）——断言本文件不复制该错误判据（源码自证 `emit/` 内零 `x-class: true` + `properties` 的判错分支，错误只在 `schema.rs`） | 单例（源码自证 + 契约边界） |
| 产品侧直连面恰为分派器两处（**D46 / AC-24 ④**） | 边界 | `grep -rn "rust::render(\|ts::render(" crates/ src/ --include=*.rs` 滤 `*_test.rs` 后**恰 2 命中**且命中文件去重后**恰一行 `emit/mod.rs`**（`render_target` 的两臂各一）；**MUST NOT 用「零命中」作判据**——分派器本身必然调这两个函数，零命中只会在「没有任何调用者」时成立（假判据） | 单例（两向断言 + 判据形态留痕） |
| `render_target` 与 `render` 产出逐字相等（**D46** 行为面） | 正向 | 同一契约 + 同一 `EmitTarget` 经 `render_target` 与经 `emit::{rust,ts}::render` 直接调用 → 产出**逐字相等**（分派无额外语义）；`EmitTarget.kind` 两值各一对照 | 表驱动（2 语言） |
| `EmitTarget` 三字段承载（**D40 / D41 / D58**） | 正向 | 三字段齐备（`kind` / `form` / `modules: BTreeMap<String, ModuleRef>`；`path` / `element_root` 已删，源码自证两字段名与 `use std::path::PathBuf` 各 0 命中）；`ModuleRef { name, module_path }` 两字段可独立读取（TS 侧只读 `name`、Rust 侧只读 `module_path`）；`form` 由调用方传入、本文件零 `paths::entry_form` 调用（零 IO） | 单例（字段面 + 源码自证） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `emit::rust::render` / `emit::ts::render` | **不 mock**：真实渲染（分派正确性即被测语义） | 全部用例 |
| 契约 / schema 输入 | **不 mock**：真实 `contract::parse` + `schema::build_schema` 产出 | 全部用例 |
| 文件系统 | **零依赖**：出 `String`，写盘归工具 crate（D1 / AC-12） | 全部用例 |
| stderr（`x-class` 警告） | **不 mock 输出流、不造日志替身**：沿用「源码自证 + 返回集零影响」两向断言（`include_str!("mod.rs")` 内 `eprintln!` 恰一处 + 触发/未触发两态下返回码与生成物逐字相等），不引入 stderr 捕获装置（警告是运行期提示、不属产物面） | `x-class` 警告用例 |
| 时钟 / 网络 / 子进程 / 环境变量 | **零依赖** | —— |

> **D58 的装置写面（本设计三处用例的改判——「单契约粒度」、「`EmitTarget` 三字段面」、本节末尾的「`EmitTarget` 三字段承载」，`tasks.md` 阶段三十 ⑤ a）**：`EmitTarget` 三字段化后，本节的 `target(kind)` 装置与所有 `EmitTarget { … }` 字面量只写 `kind` / `form` / `modules`（`path` / `element_root` 两行初始化删除——那 **19 处**构造字面量 / **38 行**初始化行的删除属**实现相位**的第四类机械随动，`tasks.md` 阶段三十 ④；断言值 / 用例名与本节措辞归本相位）。

---

### packages/desktop/src-tauri/crates/core/archi/src/api/emit/rust.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/emit/rust_test.rs

#### 待测功能

- `render()`: `.i.rs` 文本——头注释块（工具版本 + `common.json` 摘要前 12 位 + 勿手改 / 覆盖 / `api:check` 失败 / impl 分离四句）、签名见证（每 op 一行 `const _: fn(<入参类型序列>) -> <返回类型> = super::<Owner>::<op>;`、每个零字段 `$defs` 类型一行 `const _: fn() -> super::<Type> = super::<Type>::new;`）、类型段（七种协议映射）、自动构造器、透传 `pub use`；**模块级导出恰一个、四参单形** `pub fn render(contract: &Contract, schema: &Value, meta: &ApiMeta, target: &EmitTarget) -> String`（**`target` 末位**，D46——「空 scope」由调用方传空 `modules` 的 `EmitTarget` 表达，MUST NOT 存在三参孪生、MUST NOT 存在 `render_with_target`；**不主张**与 `emit::render_target(target, contract, schema, meta)` 同序——后者 `target` 在首位）；注释类拼接**只经** `rust_comment_text`、枚举值走既有 `variant_name`（D45 闸二）；`value_type` **零 `x-function` 分支**（D47）（公共函数 / API）
- `rust_comment_text()`: **注释拼接的唯一转义点**（D45 闸二）——`\n` / `\r` / `U+2028` / `U+2029` → 空格（防断行），其余原样；`//` 与 `///` 两处的拼接都经它（公共函数 / API）

> 说明：七种协议映射取自 D9 / proposal §5 语言映射表，构造器三条取自 D11，头注释取自 D12 / D13，签名见证写法取自事实校准 5（`super::Type::method` 形态），透传模块路径取自 **D40**（只读 `EmitTarget.modules[..].module_path`、MUST NOT 用目标 FQN 的点分段），`x-class` 两判据的落点分工取自 D27 / 事实校准 16（**错误**侧由 `schema::verify_defs` 拦、**警告**侧由 `emit::render_target` 发，本渲染器只出字符串、零 stderr、注释须与实现一致），**出口单形取自 D46**（四参、`target` 末位、`render_scope` 私有、零异名孪生——`render` 的形参面口径由 D46 收口，旧稿的「三形参 vs D40 不自洽」已在 design 裁定），**闸二单点转义与 `value_type` 零 `x-function` 分支取自 D45 / D47**，**`oneOf` / `anyOf` 分支负载单通路取自 D59**（`variants_block` 的负载一律经 `value_type(choice, Position::Field, common)`，`fn choice_payload` 已整条删除——同一 schema 的同一分支经两路径产出**逐字相等**，MUST NOT 在 `variants_block` 内重写第二份标量表、MUST NOT 新增第三个判定函数），**映射查表取自 D54**（槽位文本形来自 `common.json`，渲染器零硬编码）；用例按 AC-5 / AC-11 / AC-13（试点生成物形态）/ **AC-23（闸二）/ AC-24（出口单形）/ AC-25（`value_type` 收敛）/ AC-31 / AC-32（映射查表）/ AC-34 ②（分支负载单通路）** 组织。`render_scope` 保持私有（唯一消费方是同文件的 `render`），其行为经 `render` 间接观测。

- `variant_name()`: 枚举值 / 类型名 → **合法 Rust 变体名**（驼峰，非标识符字符当分隔、首字符 ASCII 数字加 `V` 前缀、空串退化 `Unknown`）；是「契约任意文本 → 标识符」的**归一**（非转义，故 `enum` 值不经 `rust_comment_text`）（**私有备案行**：盘上形态即私有 `fn`，按 design 该小节纪律①「私有工序不入表」，故 design 公共函数表**零行**；行为经 `render` 的 enum 成员断言间接观测，MUST NOT 为测试放宽可见性）
- `common_type()`（**私有备案行，不入 design 公共函数表**）: 映射规则查表（D54 ③ 唯一查表面）——承载键经 `protocol_key` 求出、命中即取 `rust` 槽位并把 `{}` 替换为内容位类型；未命中 / 无内容位 ⇒ `None`（交回普通 `type` 分派）；规则表经 `ApiMeta.common` 传入，渲染器零 IO、四参签名不扩（行为经 `render` 间接观测，MUST NOT 为测试放宽可见性）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `render` 的 `object` → `struct`（AC-5） | 正向 | `$defs` 值为 `{"type":"object","properties":{...}}` → 文本含 `pub struct <Name> { … }`，字段名与类型逐项映射（`required` 之外加 `Option<T>`） | 表驱动（2 形态：带字段 / 零字段） |
| `render` 的 `enum` → `pub enum`（AC-5） | 正向 | `$defs` 值为 `{"enum":["a","b"]}`（或 `oneOf` 常量串）→ 文本含 `pub enum <Name>` 与两变体名；`Deserialize` 派生按元素 crate 现有依赖决定（本期只落「零 serde 依赖 ⇒ 不派生」分支，其余分支返回 `ApiError::Element` 并留 TODO，待决问题 1） | 表驱动（2 形态） |
| `oneOf` / `anyOf` → 带负载 enum（AC-5） | 正向 | `{"oneOf":[{"$ref":"#/$defs/A"},{"$ref":"#/$defs/B"}]}` → 文本含 `pub enum <Name>` 且两变体各带负载类型 `A` / `B`；`anyOf` 同形 | 表驱动（2 形态） |
| `oneOf` / `anyOf` 分支负载三类新增带负载（**D59 / AC-34 ②**） | 正向 | 同一 `oneOf` 的三个分支分别取 ① 协议键 `{"outcome": {"type": "string"}}`、② `{"type": "array", "items": {"type": "string"}}`、③ `{"type": "object", "additionalProperties": {"type": "integer"}}` → 三变体**各带负载**（此前落 `Variant{index}`；D59 把 Rust 侧对齐 `value_type` 通路）；逐分支断言「经 `value_type(choice, Position::Field, common)` 的返回文本」与「`variants_block` 产出的负载文本」**逐字相等**（协议键分支两路径都经 `common_type` 查表） | 表驱动（3 分支 × 两路径对拍） |
| `oneOf` 分支负载两类仍落 `Variant{index}`（**D59 边界**） | 边界 | 「`object` 无 `additionalProperties`」与「无 `type` 且非 `$ref`」两分支 → 仍产 `Variant{index}`（`value_type` 的兜底文本 `serde_json::Value` 即判定阈值）；对照四类带负载分支 → 均不落兜底；`$ref` 分支（`AA(A)` / `BB(B)`）与四种标量分支**逐字不变**（D59 的射程精确到此） | 表驱动（2 兜底 + 4 带负载对照） |
| `choice_payload` 零残留与单通路（**D59 / AC-34 ②** 源码自证） | 异常 | 源码自证：`grep -cF 'fn choice_payload'` 在 `rust.rs` **0**（函数已整条删除）；`variants_block` 体内**零标量表**（该函数体 `"i64"` 零命中）；`rust.rs` 全文 `"i64"` 由删前 **3** 降为 **2**（剩两处分别在 `value_type` 与 `scalar_type`——**MUST NOT 取 1**，那会让 `scalar_type` 的合法映射被误判为残留）；`contract.rs` / `contract_test.rs` 的注释词面已改指 `value_type`（`choice_payload` 在两文件零命中） | 单例（源码自证 4 断言） |
| `x-class` → `pub trait`（AC-5 / D27） | 正向 | `{"x-class": true, "properties": {"run": {"x-function": {...}}}}` → 文本含 `pub trait <Name>` 且方法签名逐项；零 `properties` 的 `x-class` → `pub trait <Name> {}`（空 trait 合法）；带属性的 `x-class` → `schema::verify_defs` 已判契约错误（`ApiError::Contract`），本渲染器**不复制**该判错（断言文本生成路径不产出 `pub struct` + 方法的混装） | 表驱动（2 形态） |
| 标量 → newtype（AC-5 / D9） | 正向 | `{"type":"integer"}` + 标量名 → 文本含 newtype `pub struct TimestampMs(pub i64);`（不是 trait、不进 `common.json`）；`{"type":"string"}` → `pub struct X(pub String);` | 表驱动（2 形态） |
| `x-function` → `pub fn`（AC-5 / D11） | 正向 | `{"x-function": {"params":[{"name":"input","type":"string"}], "returns":{"type":"string"}}}` 在 `$defs` 内 → 位置分派表生效：入参 `string` → `&str`、返回 `string` → `String`（内部表，MUST NOT 外露为配置项）；`async` 键为 `true` → 文本形态可判（Rust 侧不区分 `async` 帧，断言不 panic） | 表驱动（2 形态） |
| 非 `required` 字段 → `Option<T>`（AC-5） | 正向 | `object` 的 `properties` 两字段、`required` 只列其一 → 未列字段为 `Option<T>`、已列为裸 `T`；`required = []` → 全 `Option<T>` | 表驱动（2 形态） |
| 错误面 `Outcome` 信封 → `Result<T, String>`（AC-5） | 正向 | 返回类型 `Outcome(T)` → 文本为 `Result<T, String>`（std 类型，**不生成新类型**、零 vendor）；`common.json` 只承载映射规则（断言文本无新定义） | 单例 |
| 自动构造器（AC-5 / D11） | 正向 | 零字段 `object` → 文本含 `impl <Type> { pub fn new() -> Self { Self } }`；带字段 `object` → 构造器形态可判（不 panic），且**不走**契约声明面（构造器由 `$defs` 推论，MUST NOT 要求契约写 `### new`） | 表驱动（2 形态） |
| 签名见证形态（事实校准 5 / AC-13） | 正向 | 试点契约 → 文本含两行 `const _: fn(...) = super::LocalArchiCheck::file_stem;` 与 `const _: fn() -> super::LocalArchiCheck = super::LocalArchiCheck::new;`；断言用 `super::` 前缀（裸名会报 E0425）且**签名见证语句里不含**裸 `<Owner>::<op>` 形态 | 单例 |
| 透传 `pub use` 的模块路径取自查表（AC-11 / **D40**） | 正向 | `$defs` 纯 `$ref` 别名两项 + `EmitTarget.modules` 两键（各带 `ModuleRef { name, module_path }`）→ 文本含两行 `pub use <module_path>::<name>;`，`<module_path>` 逐字等于查表值（`crate::archi` 形态）而**不是**目标 FQN 的点分段（`App.Frontend.Components.Agent.Input` 形态即非法 Rust）；零结构关键字、零本地 `struct` 重定义 | 表驱动（2 项） |
| 透传模块路径零 FQN 派生（D40 反例在案） | 异常 | 源码自证：`emit/rust.rs` 内零 `rsplit('.')`、零 `replace('.', "::")`、零 `reexport_target(..)` 直接当模块路径使用（`reexport_target` 只供注释与查表键）；断言两形态不相等：`reexport_target("…/App.X.Y.schema.json#/$defs/Y")` == `"App.X.Y"`（FQN 形态）而 `ModuleRef.module_path` == `"crate::y"` 形态，文本取后者 | 单例（源码自证 3 断言 + 两值对照） |
| `modules` 查不到目标的退化（D40 边界） | 边界 | 透传条目有而 `modules` 无该键 → 只出 `// 透传自 <FQN>` 注释、**不产 `pub use`**（不退化到 FQN 末段）；`modules` 为空 + 零透传 → 输出与非空时逐字相等 | 表驱动（2 形态） |
| 头注释四句（AC-2 / D13） | 正向 | 文本前若干行为注释块，含「勿手改」/「下次 generate 时被覆盖」/「api:check 零 diff 门会失败」/「impl 块写在本模块任意手写文件」四句 + `archi-api <CARGO_PKG_VERSION>` + `common.json@sha256:<12 位>` | 单例 |
| 泛型零支持（AC-5 反例） | 异常 | `$defs` 值含泛型描述（`typeParameters` / `$ref` 带 `<T>`）→ `Err(ApiError::Contract(..))` 点名该类型，**MUST NOT** 产出 `<T>` 形态文本（AC-5 反例在案） | 表驱动（2 形态） |
| `enum` 派生分支未实现（异常） | 异常 | 元素 crate 已有 `serde` 依赖的 `enum` → 本期按待决问题 1 返回 `ApiError::Element(..)`（写 TODO 注释），断言 `Err` 且文案含「待确定」或类型名（不静默省略派生） | 单例 |
| 幂等与键序（边界） | 边界 | 同输入两次渲染 → 两文本**逐字相等**；类型段按 `$defs` 声明序（不排序、不按结构合并） | 单例（两次对拍） |
| 格式化器不动点（AC-12 / D29） | 边界 | 生成文本是 `rustfmt --edition 2021` 的**不动点**：对渲染结果跑一次 `rustfmt --check` 语义的判据（含注释缩进、`impl` 块空行、`const _:` 见证行折行形态）**零差异**；断言不含双空格对齐、不含尾随空白行、文件末尾恰一个换行（与 `crates/infra/archi.i.rs` 实测 `rustfmt --check` exit 0 同形）；**MUST NOT** 在渲染路径调用外部格式化器（零 spawn，源码自证零 `std::process`） | 单例（形态 + 源码自证） |
| 语言词禁令（AC-17 边界） | 边界 | `.i.rs` 是**语言特定**产物（Rust 语法天然在场），但不承载**跨语言词汇**：文本零 `crate` 定义段 / 零 `Cargo.toml` 字样（`crate::` 路径前缀不在此禁令内——禁令面是契约资产 `.md` / `.schema.json` / `INDEX.toml`，见 `schema.rs` 章节） | 单例 |
| 空契约 / 零 `$defs`（边界） | 边界 | 零类型零 op → 返回仅头注释（或空串），不含 `impl` / `pub use`（零内容不生成，事实校准 6） | 表驱动（2 形态） |
| 规模与特殊字符（边界） | 边界 | 1000 类型 + 200 op → 不 panic、条数逐项相等；类型名含 emoji / `$defs` 值含超长 `description`（>1000 chars）→ 原样进注释不破坏 Rust 语法形态（断言不含裸 `\0`） | 表驱动（2 形态） |
| `rust_comment_text` 断行防护（**D45 闸二 / AC-23 ②**） | 正向 | `"a\nb"` → `"a b"`（`\n` → 空格）；`"a\rb"` → `"a b"`；`"a<U+2028>b"` / `"a<U+2029>b"` → 转成空格；`"a  b"`（双空格）/ `""` / 纯 Unicode（emoji、中文）→ **原样**（不额外改写）；**断言输出恒为单行**（零 `\n` / `\r` / `U+2028` / `U+2029`） | 表驱动（7 形态） |
| `rust_comment_text` 是唯一拼接点（**D45 闸二** 源码自证） | 边界 | `pub fn rust_comment_text` 恰 **1** 处；`rust.rs` 内 `//` 与 `///` 的注释文本拼接**全经它**（裸 `format!("// {}", …)` 形态零命中，除非经该函数）；`ts.rs` 内零 `rust_comment_text`（跨语言不互借） | 单例（源码自证 3 断言） |
| `variant_name` 无损归一与 `ts_string_literal` 的分工（**D45 闸二**） | 异常 | 契约 `enum` 值含 `'` / `"` / `\` / 换行 → Rust 侧走 `variant_name` 产出**合法 Rust 标识符**（`grep -c "fn variant_name"` 恰 **1**，其输出对 `'` / 空格 / 首字符数字三形态均不产出非法标识符）；对照：同值在 TS 侧走 `ts_string_literal` 产出单引号串（见 `emit/ts.rs` 章节）——**两侧形态不同是正题**，MUST NOT 互借 | 表驱动（3 形态 + 源码自证） |
| 四参单形（**D46 / AC-24 ①**） | 正向 | 逐产品文件带完整签名的定形串 `^pub fn render(contract: &Contract, schema: &Value, meta: &ApiMeta, target: &EmitTarget) -> String` 在 `rust.rs` 恰 **1** 命中（该串同时钉住「恰一个导出」「四参」「`target` 末位」「返回 `String`」四事实，三参孪生下恒 0）；**MUST NOT** 只数 `^pub fn render\(`（三参孪生与四参单形下同为 1，无区分能力） | 单例（源码自证 1 断言 + 判据形态留痕） |
| 零异名孪生（**D46 / AC-24 ②**） | 异常 | `fn render_with_root` / `fn render_with_target` 在 `rust.rs` / `ts.rs` / `emit/mod.rs` **各 0**（**逐文件三条**；目录级 `grep -c` 会把 `*_test.rs` 的命中算进来，MUST NOT 用）；「空 scope」由调用方传空 `modules` 的 `EmitTarget` 表达——行为面：空 `modules` + 零透传条目的产物与旧三参形态产物**逐字相等**（退化关系由入参表达、不由函数数量表达） | 表驱动（3 文件 × 零命中 + 行为对拍） |
| `render_scope` 保持私有（**D46 / AC-24 ③**） | 边界 | `^fn render_scope` 在 `rust.rs` 恰 **1**、`pub fn render_scope` 恰 **0**（升格即「仅测试可见的导出」，与 D43 对 `extract::render` 的裁定同款）；唯一消费方是同文件的 `render`（源码自证调用点数 = 1） | 单例（源码自证 3 断言） |
| `value_type` 内零 `x-function` 分支（**D47 / AC-25 ②**） | 边界 | **逐文件、锚函数体**判：取 `rust.rs` 的 `fn value_type` 函数体文本后数 `"x-function"` ⇒ **0**（`rust.rs` 删前删后都是 0——既有形态即如此，本用例把它写成不变量）；**文件级计数 MUST NOT 用作判据**（`rust.rs` 合法命中点有 3 处：`struct_block` 的 `is_some` + `unwrap_or` + `trait_block`，任何「两侧相等 / 某处恰 N」的文件级断言都无区分能力） | 单例（逐文件源码自证 + 判据形态留痕） |

| `common_type` 按表展开且随表变化（AC-32） | 正向 | 自造规则表（`rust: "Mapped<{}>"`）后 `render` 产物含 `Mapped<String>` 且**不残留** `Result<String, String>` —— 证明文本来自表而非渲染器硬编码（判据力） | 表驱动（2 断言） |
| `common_type` 空表回落普通分派（AC-32） | 边界 | `meta.common` 置空表 ⇒ 产物回落到普通 `object` 分派（`serde_json::Value`），零 panic、零空洞包装 | 单例 |
| 渲染器产品面零硬编码映射串（AC-31 ⑤） | 正向 | 源码自证：`rust.rs` 全文 `"Result<{}, String>"` 零命中（该文本形只许在 `common.json`）；`fn common_type` 在场（查表单点） | 单例（源码自证） |
#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `sha2` 摘要 | **不 mock**：真实摘要（头注释的可分辨性即被测语义） | 头注释用例 |
| 契约 / schema 输入 | **不 mock**：真实 `contract::parse` + `schema::build_schema` | 全部用例 |
| `contract::is_constructor` | **不 mock**：真实调用（构造器封闭集单点，D11） | 构造器用例 |
| `value_type` / `common_type` / `variant_name` | **不 mock**：真实单点（D59 的「分支负载单通路」判据是「同一 schema 两路径产出逐字相等」，替任一侧即让该判据失去对象） | 分支负载 / 映射查表 / 枚举值用例 |
| `rust_comment_text` / `variant_name` | **不 mock**：真实转义单点（断行防护与标识符归一无损即被测语义，替身会让闸二变成不可见） | 注释与枚举值用例 |
| `EmitTarget` | **不 mock 数据、不造替身类型**：以字面量构造真实 **三字段** `EmitTarget { kind, form, modules }`（D58：`path` / `element_root` 已删，装置里亦不得保留；空 `modules` 用调用方装置 `empty_target()` 表达，MUST NOT 另立语义不同的装置——第四类机械随动口径，D53） | 四参单形 / 退化关系用例 |
| 文件系统 / 时钟 / 网络 / 子进程 | **零依赖**：纯出 `String`；格式化器**不 spawn**（不动点靠内建模板达成，D29） | —— |

---

### packages/desktop/src-tauri/crates/core/archi/src/api/emit/ts.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/emit/ts_test.rs

#### 待测功能

- `render()`: `.i.ts` 文本——头注释、`object` → `export interface`、`enum` → `export const X = [...] as const` + `export type X = (typeof X)[number]`、`x-class` → `export interface`（记 `degraded`）、标量 → 别名（按设计退化、不记 `degraded`）、`oneOf` / `anyOf` → union、`x-function` → `(…) => R` / `Promise<R>`（`async`）、非 `required` → `T | null`、透传 `export { X } from`（`isolatedModules` 下 `export type { … }`）；ambient-free（MUST NOT `.d.ts`）；**模块级导出恰一个、四参单形** `pub fn render(contract: &Contract, schema: &Value, meta: &ApiMeta, target: &EmitTarget) -> String`（`target` **末位**，D46；MUST NOT 有 `render_with_root` 或三参孪生）；**形态取 `EmitTarget.form`**（调用方经 `paths::entry_form` 求出，渲染器零形态推导，D41）、**透传 `<name>` 取 `ModuleRef.name`**（零 FQN 派生，D40）；`value_type` **删去 `x-function` 分支**（D47）（公共函数 / API）
- `ts_string_literal()`: **TS 字符串字面量的唯一转义点**（D45 闸二）——产出**单引号**串（`\` → `\`、`'` → `\'`、`\n` / `\r` / `\t` → 转义序列、`U+2028` / `U+2029` → `\u2028` / `\u2029` 形态）；`enum` 成员、`const` 值与一切来自契约的字符串都经它；**MUST NOT 用 `serde_json` 的 `to_string`**（双引号形态会破坏 D29 的 `vp fmt` 不动点）（公共函数 / API）
- `ts_comment_text()`: 同 `emit::rust::rust_comment_text`——注释内断行防护（`\n` / `\r` / `U+2028` / `U+2029` → 空格），`// 透传自 <FQN>` 一类拼接经它（公共函数 / API）
- `render_scope()`（**保持私有**）: 两渲染器之外、TS 侧的唯一实现体（D46）——`render` 是它唯一的消费方，升格 `pub` 即「仅测试可见的导出」；行为经 `render` 间接观测（`grep -c "^fn render_scope"` 各 1 且 `pub fn render_scope` 各 0）（公共函数 / API，私有备案行）

> 说明：映射表取自 proposal §5 与 D9，`degraded` 留痕取自症结二 / R-5，`export type` 重导出取自 D19 / AC-6，`x-class` 两判据的落点分工取自 D27（错误进 `schema::verify_defs`、警告进 `emit::render_target`，本文件 **MUST NOT** 出现 `eprintln!`），**字符串与注释的转义单点取自 D45 闸二**，**出口单形取自 D46**（旧稿的 `render_with_root` 已删——形态由 `EmitTarget.form` 传入、specifier 由 `paths::ts_import_specifier` 产出），**specifier 落字符串字面量的转义面（D48 TS 侧零筛选）** 亦落本节，**`value_type` 零 `x-function` 分支取自 D47**，**specifier 只由 `form` 与 `ModuleRef.name` 决定（D58）**——`EmitTarget` 的三字段里只读这两项，`path` / `element_root` 已删、不存在第三条输入；**分支负载单通路（D59）对本文件零新行为**（TS 侧一直经 `value_type`，`choice_payload` 只在 Rust 侧）；用例按 AC-5 / AC-6 / AC-11 / AC-17 / **AC-23（闸二）/ AC-24（出口单形）/ AC-25（`value_type` 收敛）/ AC-27（TS 侧零筛选）/ AC-31 / AC-32（映射查表）/ AC-34 ①（三字段只读两项）** 组织。本文件 `#### 待测功能` 共 **4** 条 = design「公共函数 / API」的 `emit/ts.rs` 段**三行导出**（`render` / `ts_string_literal` / `ts_comment_text`）+ `render_scope` 一行私有备案；`common_type` 另作**私有备案行**登记、不计入 59。

- `common_type()`（**私有备案行，不入 design 公共函数表**）: 映射规则查表（D54 ③ 唯一查表面）——与 Rust 侧同形，取 `typescript` 槽位；`emit::ts::function_type` 的协议级包装亦经**同一出口**（命中即整条签名交回资产槽位）；行为经 `render` 间接观测，MUST NOT 为测试放宽可见性
#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `render` 的 `object` → `export interface`（AC-5） | 正向 | `{"type":"object","properties":{"a":{"type":"string"}},"required":["a"]}` → 文本含 `export interface <Name> { a: string }`；零字段 `object` → `export interface <Name> {}` | 表驱动（2 形态） |
| `render` 的 `enum` → `as const` + `type`（AC-5 / AC-6） | 正向 | `{"enum":["a","b"]}` → 文本含 `export const <Name> = ["a", "b"] as const` 与 `export type <Name> = (typeof <Name>)[number]`（同名值 / 类型共存，实测编译通过）；零值 `enum` → 形态可判不 panic | 表驱动（2 形态） |
| `x-class` → `interface` + `degraded`（AC-5 / R-5） | 正向 | `{"x-class":true,"properties":{"run":{"x-function":{...}}}}` → `export interface <Name> { run: (...) => ... }`，且该条目在 `x-common.degraded` 留痕面（TS 结构化类型不强制实现，MUST NOT 静默弱一档）；带属性的 `x-class` → `schema::verify_defs` 已判契约错误，本渲染器不复制该判错 | 表驱动（2 形态） |
| 标量 → 别名（不记 `degraded`） | 正向 | `{"type":"integer"}` → `export type <Name> = number`；`{"type":"string"}` → `= string`；`{"type":"boolean"}` → `= boolean`；三者 `degraded` 计数不增（按设计退化，非降级） | 表驱动（3 形态） |
| `oneOf` / `anyOf` → union（AC-5） | 正向 | `{"oneOf":[{"$ref":"#/$defs/A"},{"$ref":"#/$defs/B"}]}` → `export type <Name> = A \| B`；`anyOf` 同形；单元素 `oneOf` → `= A`（无悬垂 `\|`） | 表驱动（3 形态） |
| `x-function` 同步 / 异步（AC-5） | 正向 | `{"x-function":{"params":[{"name":"a","type":"string"}],"returns":{"type":"string"}}}` → `export type <Name> = (a: string) => string`；`async = true` → 返回位为 `Promise<R>`（错误通道走 reject，**记 `degraded`**，R-5） | 表驱动（2 形态） |
| 非 `required` → `T \| null`（AC-5） | 正向 | `required` 只列一字段 → 未列字段为 `T \| null`；`required = []` → 全 `T \| null`；与 Rust 侧 `Option<T>` 逐项对应（同一契约两侧字段可选性一致） | 表驱动（2 形态） |
| 透传 `export { } from`（AC-11 / AC-6） | 正向 | 纯 `$ref` 别名两项 → 两行 `export type { X } from "<ts_import_specifier 值>"`（`isolatedModules` 下标 `export type`）；specifier 取自单点函数，**文本内零手算 `../`** | 表驱动（2 项） |
| 头注释（D13） | 正向 | 文本首部为 `//` 注释块，含「勿手改 / 下次 generate 覆盖 / `api:check` 失败 / impl 分离」四句 + 工具版本 + `common.json` 摘要前 12 位；注释块之外的语句可编译（不破坏 TS 语法形态） | 单例 |
| ambient-free（AC-6 反例在案） | 异常 | 渲染结果不含 `declare` / `declare module` / `.d.ts` 形态（ambient-free 硬约束）；文本与来源文件双自查零命中 | 单例（源码 + 文本双自查） |
| 泛型零支持（AC-5 反例） | 异常 | `$defs` 值含泛型描述 → `Err(ApiError::Contract(..))` 点名，**MUST NOT** 产出 `Array<T>` / `<T>` 形态（TS 侧无真泛型组件） | 表驱动（2 形态） |
| 无品牌类型（proposal §5） | 边界 | 标量别名**不**产出 `number & { __brand: … }` 形态（cast 传染禁令）；断言文本零 `__brand` 字面量 | 单例 |
| 幂等与键序（边界） | 边界 | 同输入两次渲染 → 两文本逐字相等；类型段按 `$defs` 声明序 | 单例（两次对拍） |
| 格式化器不动点（AC-12 / D29） | 边界 | 生成文本是 `vp fmt`（prettier 语义）的**不动点**：数组字符面一律**单引号**（`['a', 'b']` 形态，实测 prettier 会把 `["a", "b"]` 改写成单引号 ⇒ `as const` 数组逐元素单引号）；缩进 / 行宽 / 尾随逗号均为 prettier 正常形；断言文本零双引号数组字面量（`export { X } from "…"` 的模块串是否单引号由实现按 prettier 输出固化，断言「与 `vp fmt` 理想输出逐字相等」）；**MUST NOT** 在渲染路径 spawn `prettier` / `vp`（零子进程） | 单例（形态 + 源码自证） |
| 空契约（边界） | 边界 | 零类型零 op → 仅头注释或空串，不含 `export`（零内容不生成 `.i.ts`，事实校准 6） | 单例 |
| `render` 的 specifier 由 `form` + 查表两输入决定（AC-4 / AC-11 / **D40 / D41 / D46**） | 正向 | 同一契约 + 一项纯 `$ref` 透传：`EmitTarget { form: Crate, modules: {目标 FQN → ModuleRef { name: "archi", … }} }` → 文本含 `from '../archi.i'`（目录形态 `../<name>.i`）；`form: SingleFile` + `ModuleRef { name: "architecture.tsx", … }` → 文本含 `from './architecture.tsx.i'`（单文件形态）；两值均**逐字等于** `paths::ts_import_specifier(form, module_ref.name)` 的返回值（零手算层数）。**`<name>` 取自 `ModuleRef.name`（即 `paths::module_name` 的产出），MUST NOT 取目标 FQN 末段** | 表驱动（2 形态 × 2 输入） |
| `<name>` 取路径末段而非 FQN 末段（D40 反例在案） | 异常 | 目标元素 `App.Backend.Infra.ArchiRuntime`（FQN 末段 `ArchiRuntime`，元素根末段 `archi`）→ 文本 specifier 含 `archi` 而**不含** `ArchiRuntime`；源码自证 `emit/ts.rs` 内零 `rsplit('.')`（在案反例的形态）；`modules` 查不到该 FQN → 只出注释、不产 specifier（不退化到 FQN 末段） | 表驱动（2 断言 + 源码自证） |
| `render` 零形态推导（**D41** 源码自证） | 异常 | 来源文件内 **零 `entry_form_of`**（D41：该私有函数 MUST 删除）；零 `"../"` / `"./"` 字面量出现在 `render` / `render_scope` 体内（`include_str!` 源码计数自证：形态取 `EmitTarget.form`，值走 `paths::ts_import_specifier`）；零 `contains('.')` / `ends_with(".ts")` 一类形态猜测；把 `form` 从 `Crate` 改成 `SingleFile` → 输出随之改变且仍与单点函数一致（零硬编码） | 单例（源码自证 4 断言 + 行为对拍） |
| 空 scope 的退化关系由入参表达（**D46**） | 边界 | 「空 scope」由调用方传 `&EmitTarget { modules: BTreeMap::new(), .. }` 表达（**不是**第二个函数）；行为面：同一契约在「空 `modules` + 零透传条目」与「非空 `modules` + 零透传条目」两输入下输出**逐字相等**（无透传即不参与）；有透传条目但 `modules` 空 → 只出 `// 透传自 <FQN>` 注释、不产 specifier 且不 panic（两态对照） | 表驱动（3 形态） |
| 查表值特殊字符（D40 边界） | 边界 | `ModuleRef.name` 含空格 / 非 ASCII / emoji / 超长（>1000 chars）→ 不 panic；生成的 specifier 仍是可判字符串（不断言 TS 语义，只断言形态契约不破）；`ModuleRef.name` 由调用方给出（渲染器零归一，D40），故「带尾斜杠 / 前导 `./`」的归一责任在 `paths::module_name`（其用例见 `api/paths.rs` 章节） | 表驱动（4 形态） |
| `x-class` 告警边界（AC-5 / D27 交汇） | 边界 | `x-class: true` 的接口映射与禁止带属性的判错互不重叠：渲染文本只含方法成员、零属性成员；本文件零 `eprintln!`（警告只在 `emit/mod.rs`，源码自证），故同一契约不会出现「rust 侧警告 + ts 侧再警告」的双报 | 单例（源码自证 + 文本形态） |
| 特殊字符与规模（边界） | 边界 | 类型名含 emoji / 值含超长 `description`（>1000 chars）→ 转义后不破坏 TS 形态（断言不含裸 `\0`）；1000 类型 → 不 panic、条数相等 | 表驱动（2 形态） |
| `ts_string_literal` 单引号形态与三类转义（**D45 闸二 / AC-23 ②**） | 正向 | `"abc"` → `'abc'`（**单引号，零双引号**）；`"it's"` → `'it\'s'`（`'` 转义）；`"a\b"` → `'a\b'`（反斜杠转义）；`"a\nb"` / `"a\rb"` / `"a\tb"` → 转义序列（不产出裸控制字符）；`"a<U+2028>b"` / `"a<U+2029>b"` → `\u2028` / `\u2029` 形态；`""` → `''`；输出**恒为单引号包裹的单行串** | 表驱动（8 形态） |
| `ts_string_literal` 是唯一字面量写出点（**D45 闸二** 源码自证） | 异常 | `pub fn ts_string_literal` 恰 **1** 处；`ts.rs` 内**零裸插值**（`format!("'{}'", …)` 形态零命中——`enum` 成员不再裸拼单引号）；`ts.rs` 内 `ts_string_literal(` 的调用点 ≥ 1（`enum` 成员 / `const` 值 / specifier 全经它）；`serde_json::to_string` 在 `emit/` 内**零命中**（双引号形态会破 D29 不动点） | 单例（源码自证 4 断言） |
| `ts_comment_text` 断行防护（**D45 闸二**） | 正向 | `"a\nb"` → `"a b"`；`"a\rb"` → `"a b"`；`U+2028` / `U+2029` → 空格；输出恒单行；`// 透传自 <FQN>` 一类拼接经它（FQN 含换行时不破坏注释）；`pub fn ts_comment_text` 恰 **1** 处 | 表驱动（5 形态 + 源码自证） |
| 四参单形与 `target` 末位（**D46 / AC-24 ①**） | 正向 | 逐产品文件带完整签名的定形串 `^pub fn render(contract: &Contract, schema: &Value, meta: &ApiMeta, target: &EmitTarget) -> String` 在 `ts.rs` 恰 **1** 命中（三参孪生下恒 0）；**MUST NOT** 只数 `^pub fn render\(`（无区分能力）；**不主张**与 `emit::render_target(target, contract, schema, meta)` 同序（后者 `target` 在首位） | 单例（源码自证 1 断言 + 判据形态留痕） |
| 零异名孪生（`render_with_root`）（**D46 / AC-24 ②③**） | 异常 | `fn render_with_root` 在 `ts.rs` / `rust.rs` / `emit/mod.rs` **各 0**；`fn render_with_target` 同三文件各 **0**（**逐文件枚举**——目录级 `grep -c` 会命中 `ts_test.rs` 的既有命中、实现正确也恒红，MUST NOT 用）；`^fn render_scope` 各 **1** 且 `pub fn render_scope` 各 **0** | 表驱动（3 文件 × 零命中 + 私有性） |
| TS 侧 specifier 转义零筛选（**D48 / AC-27**） | 边界 | `ModuleRef.name` 为带扩展名末段（`architecture.tsx`）→ specifier `'./architecture.tsx.i'` 经 `ts_string_literal` 引用（**合法且必需**，前端单文件元素的引用形式）；`name` 含 `'` / `\` / 换行 → 转义后可判且不破坏 TS 形态（**TS 侧零筛选**：MUST NOT 用 `is_module_segment` 去筛——那会误杀合法透传）；源码自证 `ts.rs` 内零 `is_module_segment` 引用（两语言闸刻意不对称，D48） | 表驱动（4 形态 + 源码自证） |
| `value_type` 内零 `x-function` 分支（**D47 / AC-25 ②**） | 边界 | **逐文件、锚函数体**判：取 `ts.rs` 的 `fn value_type` 函数体文本后数 `"x-function"` ⇒ **0**（由删前 1 变 0）；**文件级计数 MUST NOT 用作判据**（删前 `ts.rs` 合法命中点合计 3 处 = `interface_block` + `function_of` + `value_type`，删后 2；`rust.rs` 恒 3——两文件合法命中个数本就不同，任何「两侧相等 / 某处恰 N」的文件级断言都无区分能力） | 单例（逐文件源码自证 + 判据形态留痕） |
| 同一 schema 两侧形态收敛（**D47** 行为面） | 边界 | 含 `x-function` 的合法二处（`api-op` 顶层签名 / `$defs` 条目第一层 `properties` 值）经两侧渲染：TS 出 `(…) => R`、Rust 出 `pub fn`（两侧**都**按各自语言的函数形态产出，零「一侧出函数、另一侧出 `serde_json::Value`」的静默分叉）；越界位置（`items` / `additionalProperties` / `oneOf` 分支）在装载期即中止，故渲染器不接收该输入 | 表驱动（2 合法位置 × 2 语言） |

| `common_type` 按表展开且随表变化（AC-32） | 正向 | 自造规则表后 `render` 产物含 `Mapped<string>` 且不残留 `ok:` 形态 —— 证明文本来自表而非渲染器硬编码 | 表驱动（2 断言） |
| `common_type` 空表回落普通分派（AC-32） | 边界 | `meta.common` 置空表 ⇒ 产物不产出资产文本形、零 panic | 单例 |
| 渲染器产品面零硬编码映射串（AC-31 ⑤） | 正向 | 源码自证：`ts.rs` 全文 `"{ ok: {} } \| { err: string }"` **零命中**（该文本形只许在 `api/common.json`）；`fn common_type` 在场（查表单点，`grep -c "fn common_type"` ⇒ 1）；`function_type` 经同一出口（≥3 调用点） | 单例（源码自证 3 断言） |
| 三字段只读两项（**D58 / AC-34 ①**） | 边界 | 源码自证：`ts.rs` 内 `target.path` / `target.element_root` **各 0** 命中（两字段已删）；`target.form` 与 `target.modules` 各 ≥ 1（specifier 由 `paths::ts_import_specifier(form, module_ref.name)` 单点产出 + 透传查表）；`ts.rs` 内经 `ts_import_specifier(form,` 取值。行为面：同一契约分别按目录入口（`views/architecture` ⇒ `SubdirModule`）与单文件入口（`views/architecture.tsx` ⇒ `SingleFile`）渲染 → specifier 从 `'../Input.i'` 变为 `'./Input.i'`，且**不存在第三条输入**能影响它（`element_root` 已不在结构上） | 单例（源码自证 3 断言 + 行为对拍） |
#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `paths::ts_import_specifier` | **不 mock**：真实单点函数（`../` 层数禁令即被测语义） | 透传用例 |
| `ts_string_literal` / `ts_comment_text` | **不 mock**：真实转义单点（单引号形态与注入防护即被测语义，替身会让闸二与 D29 不动点均不可见） | 字面量与注释用例 |
| 契约 / schema 输入 | **不 mock**：真实 `contract::parse` + `schema::build_schema` | 全部用例 |
| `tsc` / `knip` / `vp fmt` | **零依赖**：本模块只出字符串；形状校验在交付面（`client:check`）机械执行，不进单测（AC-6 的执行归 test-execution）；格式化器不动点靠内建模板、**不 spawn**（D29） | —— |
| stderr（`x-class` 警告） | **零依赖**：本渲染器零 `eprintln!`（警告单点在 `emit/mod.rs` 的 `render_target`，D27）；用例以「文本逐字相等 + 源码自证零 `eprintln!`」表达 | `x-class` 相关用例 |
| 文件系统 / 时钟 / 网络 / 子进程 | **零依赖** | —— |

---

### packages/desktop/src-tauri/crates/core/archi/src/api/check.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/check_test.rs

#### 待测功能

- `check()`: 十判据单点求值——固定序 ① `apiEntryMissing`（命中即不再求 ② ③ ④，仅继续 ⑤）→ ② `apiMissing`（**op 与 `$defs` 类型同一条判据、同一次求值**，存在性判定、构造器双向排除、透传纯 `$ref` alias 排除、`x-class` 与 `enum` 计入类型面，D44）→ ③ `apiUndeclared`（`severity() == Off` 直接跳过）→ ④ `apiSignatureMismatch`（**只经 `extract::signature_of` / `normalize_signature`，本文件零本地 `Position`、零本地短形拼装**，D43）→ ⑤ `apiUnexpected` → ⑥–⑩ 嵌套与透传五项；`$defs` 只读 `schema.json`；`occurrences` 同元素对聚合；**`occurrences.line` 按两类缺口分列**（op 缺口取 `ContractOp.line`（`### <op-id>` heading 行）、`$defs` 类型缺口取 `0`（位置未知），MUST NOT 为凑行号给 `Contract` 增字段，D51）（公共函数 / API）
- `category_of()`: 开放分类的单点打标六值（`entry` / `contract` / `signature` / `reexport` / `nesting` / `constructor`）（公共函数 / API）

> 说明：`ArchiApiFindingKind` / `ArchiApiFinding` / `ApiFindingSeverity` 取自 design「类型定义」（类型落在 `drift/report.rs`，本文件只引用不重定义），求值序与互斥取自 D16，分类六值取自 D15（**与 D26 的 `INDEX.categories` 形状分布语义不同**，两套值域不互借），聚合与稳定序取自 D18，`x-class` 两判据的落点分工取自 D27（本文件不承载二者），**签名归一只经 `extract` 两函数（D43）**、**`apiMissing` 的类型口径与三条排除面（D44）**、**`occurrences.line` 分列（D51）**；用例按 AC-7（基线 28 / 13 / 其余 0）/ AC-8 / AC-9 / AC-11 / **AC-21（判据侧零本地归一）/ AC-22（类型缺口同口径）/ AC-28（行号分列）** 与 proposal「测试文件」表的判据十值条目组织。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| ① `apiEntryMissing` 命中（AC-7） | 正向 | 元素 `path` 非空 + `entry_form` 为 `None` → 1 条 `ApiEntryMissing`、`category == "entry"`、`element = Some(FQN)`；不因无契约而报 `apiMissing` | 单例 |
| ① 与 ②③④ 互斥（AC-7 / D16） | 边界 | 同一元素**同时**具备「无入口」+「`api = declared` 但 md 缺失」+「代码有未声明导出」三条件 → 只报 1 条 `ApiEntryMissing`，`apiMissing` / `apiUndeclared` / `apiSignatureMismatch` 零条（结构上不可判）；⑤ `apiUnexpected` 仍独立判（`api = none` + md 存在 → 另报 1 条） | 单例（4 判据条数对拍） |
| ① 基线形态 28 条（AC-7） | 边界 | 构造 27 条无入口元素（壳 1 + 工具 2 + 前端 25 的等价形态）+ 1 条只有 `main.rs` 的新增工具 → `ApiEntryMissing` 恰 28 条、按 FQN 稳定序；`path` 空元素不报（容器天然无入口） | 表驱动（28 元素 + 容器对照） |
| ② `apiMissing` 契约文件缺失（AC-7 / **D36**） | 正向 | `api = declared` + 该 FQN **不在** `input.contracts`（D36：无 `absent` 清单，判据按 `element.api == Declared && contract.is_none()` 直接判）→ 1 条 `ApiMissing`、`category == "contract"`、`message` 含路径；「14 个命中入口 − 1 试点 = 13」的等价构造 → 恰 13 条 | 表驱动（13 + 1 试点对照） |
| ② `apiMissing` 实现缺失（op 不在导出集） | 异常 | 契约声明 `### compose_text` 而代码导出集无该符号 → 1 条 `ApiMissing`、`target = Some("compose_text")`，并给两侧差异（声明 / 实现） | 表驱动（2 形态） |
| ③ `apiUndeclared` 直接跳过（AC-7） | 边界 | `severity() == Off` → 即使代码 `external` 导出集比契约 `$defs` 键集多 10 项，`ApiUndeclared` **零条**（第一步即跳过，不是「报后过滤」）；断言返回集不含该 kind | 单例 |
| ④ `apiSignatureMismatch` 归一比对（AC-9 / D10） | 异常 | 契约 `input: string` vs 代码 `&str` → **不报**（归一后一致）；契约 `returns: integer` vs 代码 `String` → 报 1 条 `ApiSignatureMismatch`、`category == "signature"`、`message` 给两侧归一签名；多行签名与参数名差异 → 不报 | 表驱动（3 形态：误报排除 2 + 真报 1） |
| ⑤ `apiUnexpected` 单向（AC-7 / D16） | 异常 | `api = none` + md 存在 → 1 条 `ApiUnexpected`、`category == "entry"`；反方向（`api = declared` + md 缺失）→ **只**归 ②，两条 MUST NOT 双报（断言 `apiUnexpected` 条数为 0） | 表驱动（2 方向） |
| ⑥ `apiShadowing` 父子交集（AC-11） | 异常 | 父元素 `$defs` 与直接子元素 `$defs` 键交集非空（同名 `ArchiDependsOn`）→ 1 条 `ApiShadowing`、`category == "reexport"`、`target` 点名重复类型；交集空 → 零条 | 表驱动（2 形态） |
| ⑦ `apiRefMissing` 三种 message（AC-11） | 异常 | 跨文件 `$ref` 目标（i）文件不存在（ii）目标元素 `api = none`（iii）目标非直接子元素（FQN 前缀不相邻）→ 三形态各 1 条 `ApiRefMissing`，三条 `message` 互不相等（可辨）、`category == "reexport"`；直接子元素 + 契约在案 → 零条 | 表驱动（4 形态） |
| ⑧ `apiNestedCrossCrate`（AC-11） | 异常 | 父子元素 `path[0]` 的 crate 根不同（`crates/core/x/` vs `crates/infra/y/`）→ 1 条 `ApiNestedCrossCrate`、`category == "nesting"`；同 crate 父子 → 零条；前端父子（无 crate 概念）→ 零条 | 表驱动（3 形态） |
| ⑨ `apiReexportDrift` 只对落盘不一致（AC-11 / D16） | 异常 | 已落盘 `schema.json` 的透传键集与 md 的 `api-reexport` 键集不等 → 1 条 `ApiReexportDrift`、`category == "reexport"`；**解析期已由 `contract::verify` 报过的形态**不重复报（断言 `verify` 报 `Err` 的输入下 `check` 不产同源条目） | 表驱动（2 形态） |
| ⑩ `apiConstructorDeclared`（AC-13 / D11） | 异常 | 契约 `### new` / `### from_str` / `### with_capacity` → 各 1 条 `ApiConstructorDeclared`、`category == "constructor"`、`target` 点名该 op；`### file_stem` → 零条（试点无构造器） | 表驱动（4 形态） |
| 十 kind 各至少一条（AC-8） | 正向 | 构造覆盖全十值的输入 → 返回集 `kind` 去重后恰 10 个；每个 `category` 与 `category_of(kind)` 返回值逐字相等 | 表驱动（10 kind） |
| `x-class` 警告不进判据面（D27 / 事实校准 16） | 边界 | 「`properties` 全为函数未标 `x-class`」的输入 + 十判据全过的元素 → 返回集零条目（该提示是生成器运行期 stderr，**MUST NOT** 成为第十一 kind、不进 `apiFindings`）；来源文件 `check.rs` 内零 `x-class` 判错 / 提示分支（错误在 `schema::verify_defs`、警告在 `emit::render_target`，本文件不复制） | 单例（行为 + 源码自证） |
| `category_of` 六值打标（D15） | 正向 | 逐 kind 断言：`ApiEntryMissing` / `ApiUnexpected` → `entry`；`ApiMissing` / `ApiUndeclared` → `contract`；`ApiSignatureMismatch` → `signature`；`ApiReexportDrift` / `ApiShadowing` / `ApiRefMissing` → `reexport`；`ApiNestedCrossCrate` → `nesting`；`ApiConstructorDeclared` → `constructor`——十 kind 映射到恰六个互不相同的字符串 | 表驱动（10 kind × 6 值） |
| `category_of` 不经 `api` 出口面（**D60 / AC-34 ③**） | 边界 | 源码自证（**`check.rs` 侧**）：`check.rs` 内 `fn category_of` 仍 **1** 且 `pub`（D60 只删出口、不动定义与函数体）；`check_test.rs` 走 `use super::check::{category_of, check}` 模块路径（本节的十条 case 均能直调，**删出口不动用例**）；六个 kind 经该模块路径直调非空（行为面）。`api/mod.rs` 的 `pub use` 归零归「`api` 出口面整体归零」一行独判——同一事实两处固化会在改一处漏一处时给出假绿 | 单例（源码自证 2 断言 + 行为面 1 断言） |
| `category` 单点打标（D15） | 边界 | 返回集中每条 `category` 均可由 `category_of(kind)` 复算（`category` 集合 ⊆ 六值集合）；来源文件 `check.rs` 内零 `category: "` 形态（`include_str!` 源码自证） | 单例（源码自证） |
| `$defs` 只读 `schema.json`（AC-9 / D17） | 边界 | `schemas` 已装配而 `contracts` 的 `types` 被人为改为不同键集 → 判据仍按 `schemas` 判（不并读 md）；来源文件 `check.rs` 内零 `read_to_string` / `api/.*\.md` 形态（源码自证） | 单例（源码自证） |
| `occurrences` 聚合（D18） | 边界 | 同一元素对 3 处命中（3 op 同签名不符）→ 1 条 finding + `occurrences.len() == 3`（不铺成 3 条）；不同元素对 → 各自一条 | 表驱动（2 形态） |
| 稳定排序（D18 / AC-8） | 边界 | 构造乱序命中（kind / element / target 三序皆乱）→ 输出按 `(kind, element, target)` 升序；两次运行逐字段相等 | 单例（两次对拍） |
| `Option` 字段形态（AC-8） | 边界 | 元素级命中 → `element = Some(FQN)`、`target = None`；op 级命中 → 两者皆 `Some`；`occurrences` 空 → `[]`（不是省略字段） | 表驱动（3 形态） |
| 空输入（边界） | 边界 | `elements = []` + 全空 `contracts` / `schemas` / `symbols`（`ApiCheckInput` 六字段，**零 `absent`**，D36）+ `enabled = true` → 空 `Vec`（零 panic）；`enabled = false` → 空 `Vec`（不遍历） | 表驱动（2 形态） |
| 规模边界（边界） | 边界 | 300 元素（100 有契约）+ 1000 `$defs` 键 → 求值不 panic、条数可判；单条 `message` >1000 chars（超长类型名）→ 不截断不 panic | 表驱动（2 规模） |
| ② `apiMissing` 的 `$defs` 类型缺口（**D44 / AC-22 ①**） | 异常 | 契约 `api-types` 声明类型 `MissingType` 而代码导出集无该类型 → **同一条 `ApiMissing`**（与 op 缺口同 kind、同一次求值），`target = Some("MissingType")`；同一契约里 op 与类型**各自缺一项** → **两条** `ApiMissing`（同一判据、两类缺口各一条，MUST NOT 拆成第二条 kind）；源码自证 `fn missing_contracts` 恰 **1** 处且 op 与类型两段都在该函数体内 | 表驱动（3 形态 + 源码自证） |
| ② 三条排除面在案（**D44 / AC-22 ②**） | 边界 | 构造器双向排除：契约声明 `### new` 而代码有 `new` → 仍报 `ApiConstructorDeclared`（⑩），**不**因排除面而漏报；代码有 `new` 而契约无 → **不**报 `ApiMissing`（构造器不是契约项）；透传纯 `$ref` alias 排除：`$defs` 内纯 `$ref` 条目不算「本地类型缺口」；`x-class` 与 `enum` **计入**类型面（不因形状特殊被排除） | 表驱动（4 形态） |
| ④ `apiSignatureMismatch` 经单点归一（**D43 / AC-21 ⑤**） | 异常 | 契约 `optional: true` 的 `string` 入参 + 代码 `Option<&str>` → **零 mismatch**（漏 nullable 分支即恒假阳，D43 直接溯源）；契约 `returns: integer` + 代码 `String` → 恰 1 条并给两侧归一签名；源码自证：`check.rs` 内 `normalize_signature(` 恰 **1** 处消费、`^fn (normalize_signature\|normalize_schema\|contract_signature)` **0** 处、对 `"anyOf"` / `"null"` 的直接匹配 **0** 处、对 `rust::extract_crate` / `ts::extract_file` 的直连 **0** 处 | 表驱动（2 行为 + 源码自证 4 断言） |
| `occurrences.line` 两类缺口分列（**D51 / AC-28**） | 正向 | 契约声明 op `missing_op`（heading 在第 N 行）+ `api-types` 声明类型 `MissingType`，代码两侧都没有 → 一条 `ApiMissing`，其 `occurrences` 两项分别为 `{ line: N, specifier: "missing_op" }` 与 `{ line: 0, specifier: "MissingType" }`；源码自证 `line: op.line` ≥ **1**（op 走 heading 行）、`declared_types` 段的 `line: 0` ≥ **1**（类型缺口位置未知）；**MUST NOT** 为凑行号给 `Contract` 增 `types_line` / `reexports_line`（见 `contract.rs` 章节的字段面用例） | 表驱动（2 形态 + 源码自证 2 断言） |
| 基线 28 / 13 与互斥口径（**AC-7 / D44 / AC-22 基线**） | 边界 | 本仓形态全量输入 → `apiEntryMissing` = **28**、`apiMissing` = **13**、其余七条 **0**、`apiUndeclared` 不适用（`off`）；任一项不等即缺陷（多于期望 = 假阳、少于 = 假阴）；**若 `apiMissing` 得 14 则试点自身的 `$defs` 类型有真缺口**（`#[path]` 定位或 `pub use <mod>::*` 升格缺一，D44 两处缺口的结构前提） | 单例（四值对拍） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 契约 / 内存 schema / 符号集 | **不 mock**：真实 `contract::parse` / `schema::build_schema` / `extract::extract` 产出（判据的事实源即被测语义，替身会让「判定单源」不可见，AC-9） | 全部用例 |
| 文件系统 | **不 mock**：`entry_form` 与 `contract_path` 的存在性判定用真实 tempdir 落空文件 / 落 md（`check` 入参全为借用切片，落盘只为驱动） | ①③⑤⑦ 用例 |
| `contract::is_constructor` | **不 mock**：真实调用（契约校验与 ⑩ 共用同一函数，D11） | ⑩ 用例 |
| 时钟 / 网络 / 子进程 / 环境变量 | **零依赖**：判据纯函数出结论，零 spawn、零时钟（AC-12） | —— |

---

### packages/desktop/src-tauri/crates/core/archi/src/drift/report.rs -> packages/desktop/src-tauri/crates/core/archi/src/drift/report_test.rs

#### 待测功能

- `ArchiApiFindingKind::as_str()`: 十值 camelCase 线格式词（公共函数 / API）
- `ArchiApiFindingKind::severity()`: 判据级档位**唯一来源**——九条 `Warn`、`apiUndeclared` 为 `Off`（公共函数 / API）
- `ArchiApiFindingKind::ALL`: `[ArchiApiFindingKind; 10]`，十值顺序与 spec 表一致（新增变体未补即语料红）（公共函数 / API）
- `ApiFindingSeverity::as_str()` / `ApiFindingSeverity::ALL`: `off` / `warn`；与 `GateTier` 词面相同但**类型分立**（公共函数 / API）

> 说明：`ArchiApiFinding`（六字段）与 `ArchiDriftReport.api_findings`（末位追加）取自 design「类型定义」，档位与元素 `gate` 解耦取自 D14，`category` 开放值集取自 D15；用例按 AC-8（词汇双向对拍 + 既有 14 字段逐字不变 + `version = 2`）与 R-5（TS 侧降级留痕）组织。本节只覆盖词汇与线形；语料对拍挂 `drift/mod.rs` 与 `tests/golden_test.rs` 章节。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ArchiApiFindingKind::ALL` 十值封闭（AC-8） | 正向 | `ALL.len() == 10`；十值 `as_str()` 逐值为 `apiEntryMissing` / `apiMissing` / `apiUndeclared` / `apiSignatureMismatch` / `apiUnexpected` / `apiShadowing` / `apiRefMissing` / `apiNestedCrossCrate` / `apiReexportDrift` / `apiConstructorDeclared`；`ALL` 去重后长度不变（漏值 / 重复不可绕过） | 表驱动（10 值） |
| `ArchiApiFindingKind::as_str()` 与 serde 线形一致（AC-8） | 正向 | 十值各 `serde_json::to_value` → 字段值为 `as_str()` 同一字符串；`from_value` 往返逐值相等（词面单点） | 表驱动（10 值） |
| `ArchiApiFindingKind::severity()` 九 `Warn` 一 `Off`（AC-7 / AC-8） | 正向 | 十值逐值断言：仅 `ApiUndeclared` → `ApiFindingSeverity::Off`，其余九值 → `Warn`；`Warn` 计数恰 9、`Off` 计数恰 1（完备性判据唯一一条） | 表驱动（10 值 + 计数） |
| `ApiFindingSeverity::ALL` / `ApiFindingSeverity::as_str()` 二值（D14） | 正向 | `ALL.len() == 2`；`Off` → `"off"`、`Warn` → `"warn"`（与 `GateTier` 同名档位词面相同）；类型上不可与 `GateTier` 互换（编译期断言：两枚举各自独立 `match` 穷尽，无 `From` 实现） | 表驱动（2 值） |
| 词汇外反序列化（异常，Enum N+1） | 异常 | `"apiEntryMissing "` / `"unknown"` / `"ApiEntryMissing"` / `"api_entry_missing"` / `""` → 全 `Err`（封闭词汇严格，零宽松归一） | 表驱动（5 形态） |
| `ArchiApiFinding` 六字段线形（AC-8） | 正向 | 全字段填充（`kind` / `category` / `element = Some` / `target = Some` / `message` / `occurrences` 两条）→ `to_string` → `from_str` 逐字段相等（`occurrences` 亦逐项）；序列化文本含 `"kind"` / `"category"` / `"element"` / `"target"` / `"message"` / `"occurrences"`（camelCase 无差异，单字词） | 单例 |
| `Option` 字段 `null`（AC-8） | 边界 | `element = None` / `target = None` 两态 → 文本含 `"element":null` 且往返回 `None`；`occurrences = []` → `"occurrences":[]`（不是省略字段） | 表驱动（3 形态） |
| `category` 开放值集（D15） | 边界 | `category = "entry"` / `"contract"` / `"signature"` / `"reexport"` / `"nesting"` / `"constructor"` 六值各往返相等；`category = "未来新分类"`（值域外字符串）→ 序列化 / 反序列化均成功（开放值集，无 `ALL`、无封闭校验） | 表驱动（7 形态） |
| `ArchiDriftReport.api_findings` 空数组（AC-8） | 边界 | 既有 14 字段全填充 + `api_findings = []` → 文本含 `"apiFindings":[]`；反序列化后 14 字段逐字与改动前等价（字段面零变化）+ `api_findings` 空 | 单例 |
| 既有 14 字段逐字不变（AC-8） | 边界 | 以既有金样（`tests/corpus/drift/expected/*.json` 之一）为基线：新增字段之外的所有键名集合与值逐字相等；断言字段顺序上 `apiFindings` 位于 `status` 之后（末位追加，零插入） | 单例（金样对拍） |
| `version = 2` 档面（AC-8） | 边界 | `version` 为 `u32` 且可写 `2` / `0` / `u32::MAX` 三态往返（字段面不动，常量单点在 `drift/mod.rs`）；`version = 1` 的报告缺 `apiFindings` 时**反序列化失败**（消费方按版本分支，MUST NOT 补默认值——以 `#[serde]` 未加 `default` 的事实断言） | 表驱动（4 形态） |
| `occurrences` 复用既有形状（D18） | 边界 | `ArchiApiFinding.occurrences` 元素类型与 `ArchiDriftEntry.occurrences` 同一类型（同值构造可互相赋值）；`line = 0`（位置未知）与 `line = u32::MAX` 往返不溢出；1000 条 occurrence → 序列化成功、长度不变 | 表驱动（4 形态） |
| 超大 / 超长面（边界） | 边界 | 1000 条 `api_findings`（各 100 occurrence）→ 往返成功且长度不变；`message` >1000 chars / 含 emoji 与 `\0` → JSON 转义后往返逐字相等；两次 `to_vec_pretty` 字节相等（无 HashMap 序漂移） | 表驱动（3 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| serde / serde_json | **不 mock**：真实序列化与反序列化（线形即被测对象） | 全部用例 |
| 文件系统 / IO | **零依赖**：本模块纯数据（零 IO、零判定）；既有金样以 `include_str!` 读入（编译期内嵌，不落盘） | 金样对拍 |
| 时钟 / 随机 | **零依赖**：`timestamp` 以字面量构造 | 全部用例 |
| 网络 / 子进程 / DB | **零依赖** | —— |

---

### packages/desktop/src-tauri/crates/core/archi/src/drift/mod.rs -> packages/desktop/src-tauri/crates/core/archi/src/drift/mod_test.rs

#### 待测功能

- `REPORT_VERSION`: 报告版本常量 `1` → `2`，改常量即全分支随动（design「修改文件」表）
- `check()`: 既有判定单点在 ⑦ 观察项之后、`status` 推导之前插入 `api::check` 装配（design「修改文件」表）
- `skipped_report()`: 新增字段恒 `Vec::new()`（契约面未启用即空，D17）
- `ArchiDriftReport.api_findings`: 末位并列字段，不参与 `status` / `passed` 推导（design「类型定义」/ D18）

> 条目来源：design.md「公共函数 / API」子表对本文件**零行**（`drift::check` 的签名、判定序、`derive_status`、豁免三层与 `occurrences` 聚合全部零改动；`REPORT_VERSION` 与 `check()` 均未列入该表——`check` 是既有函数、签名不变）。四个条目改列 design「修改文件」表 `drift/mod.rs` 两行与「类型定义」`ArchiDriftReport.api_findings` 行。

> 说明：测试对象取自 design「修改文件」表 `drift/mod.rs` 两行（`REPORT_VERSION` 1 → 2；`check()` 在 ⑦ 之后、`status` 推导之前插 `api::check` 装配 + `skipped_report()` 恒空）与「类型定义」`ArchiDriftReport.api_findings`，口径取自 D17（契约面启用条件）/ D18（`api_findings` 不参与 `status`）/ D20（`ApiError` → `CheckError::Config`）/ D27（`x-class` 两处判据既不进 `apiFindings` 也不改退出码）；用例按 AC-7 / AC-8 / AC-9 组织。本节承载**跨模块组合用例**（`enabled` → `load_contracts` → `check` → 报告装配的整链，入口模块 = 判定单点 `drift::check`）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `check()` 契约面缺席 → `apiFindings = []`（D17 / AC-8） | 边界 | tempdir 有模型与源文件但无 `<archi_api_dir>` → `Ok`，`api_findings` 为空 `Vec`、`version == 2`、既有 `errors` / `warnings` / `suppressed` / `observations` / `status` 与改动前等价（逐字对拍同输入的旧报告形态） | 单例 |
| `check()` 零份 `*.md` 但目录存在（D17） | 边界 | `<archi_api_dir>` 存在且只含 `INDEX.toml`（无任何 `.md`）→ `api_findings` 空、既有语料逐字不变（「目录一旦存在即全量纳入」的前提是含 md；零 md 走未启用分支） | 单例 |
| `check()` + `ArchiDriftReport.api_findings` 试点在案 → 空 `apiFindings`（AC-13） | 正向 | 模型 1 元素命中 `<dir>/src/lib.rs` + 契约 md 在案且签名与代码一致 + 该元素为试点 → `api_findings` 空（十判据零命中）；`status` 与无契约时**相同**（`api_findings` 不影响推导） | 单例 |
| `check()` 契约在案但签名不符 → `apiSignatureMismatch`（AC-7 / AC-9） | 异常 | 契约 op 签名与代码不一致 → `api_findings` 恰 1 条 `ApiSignatureMismatch`；`status` 与 `passed`（经 `LocalArchiCheck` 派生）**逐值不变**（D18：首期不进 `status`） | 单例（报告 + `status` 双断言） |
| `check()` 缺 md → `apiMissing`（AC-7 / **D36**） | 正向 | `api = declared` 元素有入口无 md → 1 条 `ApiMissing`；判据按 `element.api == Declared && contracts` 中无该 FQN 直接判（**无 `absent` 清单**——D36 删除了该死数据字段，装配链少了第二事实源） | 单例 |
| `check()` 无入口 → `apiEntryMissing`（AC-7） | 正向 | 元素 `path` 非空但四分支全不命中 → 1 条 `ApiEntryMissing`；该元素**不再**参与 `apiMissing` / `apiUndeclared` / `apiSignatureMismatch`（互斥在装配后仍成立） | 单例 |
| `check()` `api = none` + 契约文件存在 → `apiUnexpected`（D16 ⑤） | 异常 | 模型 `api = "none"` + md 在案 → 1 条 `ApiUnexpected`；同时断言 `apiMissing` 零条（MUST NOT 双报） | 单例 |
| `check()` 坏契约 → `CheckError::Config`（D20 / AC-1） | 异常 | md 的 H1 与文件名不一致 → `check` 返回 `Err(CheckError::Config(..))`，`message()` 非空且含 `file:line`；**MUST NOT** 产出半真报告（`Ok` 分支不达）；`CheckError` 变体面仍三值（无第四变体） | 表驱动（2 形态：`Contract` 类错误 + `Io` 类错误） |
| `skipped_report()` 恒空（D17） | 边界 | 无模型（`skipped`）→ `api_findings` 为空 `Vec`、`version == 2`（未启用即空，零契约面参与） | 单例 |
| `REPORT_VERSION` = 2（AC-8） | 边界 | 任一分支（`skipped` / `noChanges` / `clean` / `errors`）产出的报告 `version` 均为 `2`；常量与报告字段同源（改常量即全分支随动） | 表驱动（4 分支） |
| 判定序零漂移（AC-9） | 边界 | 与改动前同输入的报告对比：`errors` / `warnings` / `suppressed` / `observations` / `scan` / `model` 逐字段相等、`status` 逐值相等（`api_findings` 之外零字节差异）；`derive_status` 的入参不含 `api_findings`（源码自证 `include_str!("mod.rs")` 无「api_findings 参与 status」的引用） | 单例（前后对拍 + 源码自证） |
| 零写盘（AC-12 / AC-9） | 边界 | `check` 前后 tempdir 全量字节快照相等（判据面零写盘；报告落盘仍在 `infra/archi`） | 单例 |
| 组合链规模（边界） | 边界 | 300 元素（部分有契约）+ 100 源文件 → `api_findings` 条数可判、既有面照常、不 panic；单元素同时命中多判据（⑥⑨⑩）→ 每条独立、`occurrences` 按元素对聚合 | 表驱动（2 规模） |
| `x-class` 两判据不串入报告装配（D27 交汇） | 边界 | 契约含 `properties` 全为函数未标 `x-class` 的元素 → `check` 的 `Ok` 报告 `api_findings` 条数不受影响（该提示只在生成器 stderr）、`status` 与无提示时逐值相等；契约含 `x-class` 带属性 → `Err(CheckError::Config(..))`（`ApiError::Contract` 映射，检查不执行、不产半真报告） | 表驱动（2 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 文件系统 / 模型 / 源文件 / 契约 md | **不 mock**：真实 tempdir（自建 `TempDir` + `AtomicU64`）落模型、源码、md、`config.json`（判定链路的输入事实即被测语义） | 全部用例 |
| `api::enabled` / `api::load_contracts` / `api::check` | **不 mock**：真实调用（装配单点即被测对象，替身会让「判定单源」不可见，AC-9） | 全部用例 |
| 取消信号 | **不 mock**：真实 `foundation::cancel::CancelFlag` | 取消分支 |
| 时钟 / 摘要 | **不断言具体值**：`timestamp` / `model_hash` 只断言形态（沿既有 `mod_test` 口径） | 全部用例 |
| 网络 / 子进程 / DB | **零依赖**：`drift` 零 spawn（AC-12） | —— |

---

### packages/desktop/src-tauri/crates/core/archi/tests/corpus/drift/expected/ -> packages/desktop/src-tauri/crates/core/archi/tests/golden_test.rs

本节覆盖**判定语料 golden 期望面**：`tests/corpus/drift/expected/` 下的快照文件与 `tests/golden_test.rs` 的 `DRIFT_CASES` 驱动表（执行于 crate 级集成测试）。

#### 待测功能

- `DRIFT_CASES`: 每行新增期望 `api_findings` 条数，并新增一行非空 `apiFindings` case（proposal「测试文件」表的漂移语料行 / AC-8）
- `expected/*.json`: 快照追加 `"apiFindings": []` 与 `"version": 2`，其余字段逐字不变；新增 case 产出一份非空 `apiFindings` 金样（AC-8）

> 条目来源：design.md「公共函数 / API」子表对本文件**零行**（本文件是 crate 级集成测试装置，无生产导出）。两条目改列 proposal「测试文件」表的漂移语料行与 AC-8。

> 说明：测试对象取自 proposal「测试文件」表的漂移语料行（既有 `expected/*.json` 追加 `"apiFindings": []` 且其余字段逐字不变 + 至少一个非空 `apiFindings` 金样）与 AC-8；`DESKTOP_GOLDEN_REWRITE=1` 显式重写并人工确认（既有机制，零新开关）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `DRIFT_CASES` 既有 13 case / 15 行驱动 `apiFindings` 空（AC-8） | 正向 | `DRIFT_CASES` 每行新增期望 `api_findings = 0`；逐 case 驱动 `scan → drift::check → report` 后 `report.api_findings` 为空（这些 case 均无 `<archi_api_dir>`）→ 全部通过，既有 14 字段断言逐字不变 | 表驱动（15 行） |
| `expected/*.json` 金样仅追加两处（AC-8） | 边界 | `expected/<case>.<mode>.json` 重写后与旧金样 diff：仅新增 `"apiFindings": []` 与 `"version": 1 → 2` 两处，其余字节逐字相同（人工确认 + 断言快照键名集合含 `apiFindings`） | 表驱动（11 份全量 + 4 份增量金样） |
| `DRIFT_CASES` 非空 `apiFindings` 金样（AC-8 新增） | 正向 | 新 case（tempdir 落模型 + 入口 + 不一致契约）→ 快照含非空 `apiFindings`、`kind` 为 `apiSignatureMismatch`、`occurrences` 逐项；`status` 仍按既有规则推导（契约面不进 `status`） | 单例（新 case 一行） |
| `DRIFT_CASES` 登记面与实际语料失配（异常） | 异常 | 新增 case 目录未登记在 `DRIFT_CASES` → 红（未登记即缺陷）；登记了但 case 目录不存在 → 红；快照缺 `apiFindings` 键（旧金样未重写）→ 断言点名该 case 与缺失键（不静默补默认值，D21 与报告 `version` 兼容说明同口径） | 表驱动（3 形态） |
| `expected/*.json` 重写开关（既有机制） | 边界 | `DESKTOP_GOLDEN_REWRITE=1` 单次运行 → 快照重写但断言仍按 `DRIFT_CASES` 登记面执行（快照不参与判定）；未设该变量时快照只读、零写盘 | 表驱动（两态） |
| `api-mismatch` 新 case 三 fixture + 非空金样（**AC-7 ① / AC-8**） | 正向 | 新增 case 目录 `corpus/drift/api-mismatch/` 的三件 fixture 齐备（`contracts/App/App.Mod.md` / `models/01-app.toml` / `repo/src/mod/mod.rs`）→ 驱动后快照含**非空** `apiFindings`，其 `kind` 为 `apiSignatureMismatch`、`occurrences` 逐项在案（`line` / `specifier` 与非空数组）；`status` 仍按既有规则推导（契约面不进 `status`，D18） | 单例（新 case 一行 + 四件 fixture 在场断言） |
| `DRIFT_CASES` 登记面三形态失配皆红（**AC-7 ③**） | 异常 | ① 新 case 目录**未登记**在 `DRIFT_CASES` → 红（未登记即缺陷）；② 登记了但 case 目录**不存在** → 红；③ 旧金样**缺 `apiFindings` 键**（未重写）→ 红并点名该 case 与缺失键（**不静默补默认值**——`version = 1` 的报告无该字段，消费方按版本分支，MUST NOT 补默认值）；三形态各一条断言，任一静默通过即缺陷 | 表驱动（3 形态） |
| 既有 15 份金样重写面（**AC-8 / AC-7 ②**） | 边界 | 12 份 `.full.json` + 3 份 `.incremental.json` 重写后与旧金样 diff **仅两处**（追加 `"apiFindings": []` 与 `"version": 1 → 2`），其余字节逐字相同（人工确认 + 键名集合含 `apiFindings`）；`DESKTOP_GOLDEN_REWRITE=1` 单次运行后零残留旧形态 | 表驱动（15 份） |
| 逐判据正反例由 sidecar 直驱、零第二套语料（**AC-7 ④ / 唯一驱动面**） | 边界 | 十判据的正反例断言全部落在 `crates/core/archi/src/api/**` 的十一份 sidecar 用例内（直驱 `check::check`，不依赖快照）；**MUST NOT 另建 `tests/corpus/api/`**（全仓该目录零存在——源码/目录自证）；新增 `corpus/drift/api-mismatch/` 只承载报告面金样，不承载逐判据反例 | 单例（目录零存在 + 归属断言） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 文件系统 | **不 mock**：真实临时目录（沿 `golden_test` 既有 `TempDir` 装置）+ 真实 case 目录读取；`repo/**` 与 `models/*.toml` 落盘后驱动 | 全部用例 |
| 判定链路 | **不 mock**：真实 `scan` / `drift::check` / 报告序列化（语料对拍即被测语义） | 全部用例 |
| 环境变量 | **不替换取值语义**：`DESKTOP_GOLDEN_REWRITE` 由测试进程真实读取（既有机制，`std::env::var` 直读）；用例以「设 / 不设」两态驱动，不注入假读取器 | 重写开关用例 |
| 子进程 / 网络 / DB | **零依赖** | —— |

---

### packages/desktop/src-tauri/crates/core/archi/src/dto.rs -> packages/desktop/src-tauri/crates/core/archi/src/dto_test.rs

#### 待测功能

- `ApiStatus::as_str()`: `declared` / `none` 线格式词（公共函数 / API）
- `ApiStatus::ALL`: `[ApiStatus; 2]` 全量常量（知识文本与语料对拍的唯一派生源）（公共函数 / API）

> 说明：`ApiStatus` / `ArchiApiRef` 与 `ArchiElement.api` / `ArchiElementBrief.api` / `ArchiModuleDetail.api` 三处出线取自 design「类型定义」与「修改文件」表 `dto.rs` 行，两层口径（模型意图 vs 文件在案事实）取自 D22；用例按 AC-10（`api` 字段封闭二值 + `archi_module` 并列字段）与 AC-8（四面词汇对拍的一侧）组织。四面词汇集与知识文本的对拍执行在 `tests/vocabulary_test.rs` 与 `infra/agent/src/archi_knowledge_test.rs` 章节（挂靠 `dto.rs` 为词汇单点）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ApiStatus::as_str()` / `ApiStatus::ALL` 二值封闭（AC-10） | 正向 | `ALL.len() == 2`；`Declared` → `"declared"`、`None` → `"none"`；serde 线形与 `as_str` 逐值一致；`serde_json` 往返逐值相等 | 表驱动（2 值） |
| `ApiStatus::ALL` 与缺省（AC-10） | 边界 | `ApiStatus::default() == Declared`（缺省 `declared`，模型未写该键即承诺有契约）；`ALL` 去重后长度不变 | 单例 |
| `ApiStatus` 词汇外反序列化（异常，Enum N+1） | 异常 | `"Declared"` / `"NONE"` / `"none "` / `"off"` / `""` → 全 `Err`（`rename_all = "lowercase"` 严格，零宽松归一） | 表驱动（5 形态） |
| `ArchiElement.api` 线形（AC-10） | 正向 | 全字段元素 + `api = Declared` / `None` 两态 → 序列化文本含 `"api":"declared"` / `"api":"none"`；往返逐字段相等；既有九字段名与值零变化（新增字段不入 `metadata`） | 表驱动（2 形态） |
| `ArchiElementBrief.api` 线形（AC-10） | 正向 | 薄条目四字段 + `api` → 文本含五键；`api` 与 `ArchiElement` 同值（同一模型字段的纯拷贝，零推导） | 单例 |
| `ArchiApiRef` 四字段（AC-10 / D22） | 正向 | `status = Declared` + 三路径 `Some` → 文本键为 `status` / `contractPath` / `schemaPath` / `generatedPath`（camelCase），值为 repo 相对 POSIX 串；`status = None` + 三路径 `null` → 往返后仍全 `None` | 表驱动（2 形态） |
| `ArchiApiRef` 部分路径形态（边界） | 边界 | `schema_path = None` 而 `contract_path = Some` → 序列化成功（四字段独立可空，但 D22 规定 `none` 时三者全 `None`——断言该组合不被 `dto` 投影产出，见 `api::api_ref` 章节） | 单例 |
| `ArchiModuleDetail.api` 并列字段（AC-10） | 正向 | 六字段详情 + `api: ArchiApiRef` → 文本键集含 `element` / `dependsOn` / `dependedBy` / `children` / `docs` / `warnings` / `api`；`api` **不在** `docs` 数组内（并列字段而非承重进 `docs`，AC-10 / `desktop-archi-module-docs` ADDED） | 单例 |
| `module_detail` 投影填 `api`（AC-10 / AC-13） | 正向 | tempdir 落元素 + 契约 md → `module_detail(root, fqn).api.status == Declared` 且三路径非 `None`；删 md → `status == None` 且三路径全 `None`（三路径由 `api::api_ref` 单点产出——`dto` 已持元素 `path`，故走其元素形态入口；零 `dto` 内拼装） | 表驱动（2 形态） |
| `module_detail` 投影零改写（D22） | 边界 | `api_ref` 报 `none` 时模型字段 `element.api` **仍为 `declared`**（投影只报文件事实，MUST NOT 回写模型字段）；`to_element` / `to_brief` 的 `api` 与模型字段逐值相等（纯拷贝） | 表驱动（2 断言） |
| `module_detail` 容器元素无契约（AC-10） | 边界 | `path = []` 的容器元素 → `module_detail.api.status == None` 且三路径全 `None`（天然无契约，零特判分支） | 单例 |
| `ArchiElement` / `ArchiElementBrief` / `ArchiModuleDetail` 既有字段面零变化（AC-8 / AC-10） | 边界 | `ArchiElement` 九字段 / `ArchiElementBrief` 四字段 / `ArchiModuleDetail` 六字段的键名集合与序列化顺序逐字与改动前一致（新增 `api` 为末尾追加）；`ArchiGraph` / `ArchiSearchResult` / `ArchiChangeSummary` 零新字段 | 单例 |
| 超长 / 特殊字符路径（边界） | 边界 | `contract_path` 为 >1000 chars 的 repo 相对串 / FQN 含 emoji → 序列化往返逐字相等（路径是字符串，不做归一） | 表驱动（2 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| serde / serde_json / specta | **不 mock**：真实序列化（线形即被测对象）；`specta::Type` 的注册面由 `bindings:export` 再生成后零 diff 人工核对（零 diff 守卫已退役，事实校准 24，不进单测） | 全部用例 |
| 文件系统 | **不 mock**：`module_detail` 的存在性判定用真实 tempdir 落元素目录与 md（一次 `exists()` 定位即被测语义） | `module_detail` 用例 |
| `foundation::layout::archi_api_dir` / `api::api_ref` | **不 mock**：真实派生与真实投影（D22 单点即被测语义） | 投影用例 |
| 时钟 / 网络 / 子进程 | **零依赖** | —— |

---

### packages/desktop/src-tauri/crates/core/archi/src/model.rs -> packages/desktop/src-tauri/crates/core/archi/src/model_test.rs

#### 待测功能

- `Element.api`: 新增 `pub api: ApiStatus`（契约状态指针，缺省 `declared`；协议正文 MUST NOT 进模型）（design「类型定义」/ AC-10）

<!-- design.md「公共函数 / API」：本文件零行（只追加一个字段，零函数面）。条目改列 design「类型定义」`Element.api`（修改）。 -->

> 说明：测试对象取自 design「类型定义」`Element.api` 与「修改文件」表 `model.rs` 行（`pub api: ApiStatus`，doc 要求「契约状态指针，缺省 `declared`；协议正文 MUST NOT 进模型」）；用例按 AC-10 与 AC-16（`path` 写法逐字未变、模型零批量改写）组织。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `Element.api` 缺省（AC-10） | 正向 | 模型 TOML 未写 `api` 键 → `Element.api == ApiStatus::Declared`；`fqn` / `kind` / `gate` / `path` / `docs` / `summary` / `metadata` / `file` / `line` 十字段逐值与改动前等价 | 单例 |
| `Element.api` 显式两值（AC-10） | 正向 | `api = "declared"` / `api = "none"` 两态 → 模型字段逐值相等（`Eq` 可比较） | 表驱动（2 值） |
| `Element.api` 字段面零扩写（AC-10 / AC-16） | 边界 | `Element` 字段数 11（原 10 + `api`）；`metadata` **不**承载 `api`（`metadata = { api = "non" }` 不改变 `Element.api`，留给 `parse` 章节的静默反例）；`path` 原文写法逐字保留（`./` 前缀 + 尾 `/` 不归一，AC-16） | 表驱动（3 断言） |
| `Element.api` 协议正文不进模型（AC-17） | 边界 | `Element` 上不存在任何签名 / `$defs` / `operations` 类字段（源码自证 `include_str!("model.rs")` 零 `defs` / `signature` / `operations` 字样）；`api` 只承载二值 | 单例（源码自证） |
| 值域封闭的编译期保证（异常，Enum N+1） | 异常 | `Element.api` 的类型封闭二值 —— 无法在 `Element` 上构造第五值（编译期断言：`match element.api` 两臂穷尽，无 `_` 兜底臂）；非法值只能经 `parse` 报 `InvalidValue`，`model` 侧不承载校验（职责边界断言） | 单例（编译级） |
| `Element.api` 大模型（边界） | 边界 | 300 元素（各带 `api` 值）→ 模型构建后逐元素字段可读、`Eq` 比较不 panic | 表驱动（2 规模） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 解析面 | **不 mock**：真实 `parse::parse_dir`（字段缺省与线形即被测语义） | 全部用例 |
| 文件系统 | **不 mock**：真实 tempdir 落 `models/*.toml` | 全部用例 |
| 时钟 / 网络 / 子进程 | **零依赖** | —— |

---

### packages/desktop/src-tauri/crates/core/archi/src/parse/toml.rs -> packages/desktop/src-tauri/crates/core/archi/src/parse/toml_test.rs

#### 待测功能

- `RawElement.api`: 新增 `#[serde(default)] api: ApiStatus`（缺省 `declared`；拼写错经 `deny_unknown_fields` / `classify` 归 `invalidValue` 并列出全合法值）（design「类型定义」/ AC-10）
- `classify()`: 零改动（`unknown variant` / `invalid type` 已归 `InvalidValue`）（design「修改文件」表）

<!-- design.md「公共函数 / API」：本文件零行（只追加一个 `#[serde(default)]` 字段）。条目改列 design「类型定义」`RawElement.api`（修改）与「修改文件」表 `parse/toml.rs` 行。 -->

> 说明：测试对象取自 design「类型定义」`RawElement.api` 与「修改文件」表 `parse/toml.rs` 行（`#[serde(default)] api: ApiStatus`；`deny_unknown_fields` 面对拼错键继续生效；`classify` 零改动——`unknown variant` / `invalid type` 已归 `InvalidValue`）；用例按 AC-10（`api = "non"` 报 `invalidValue` 并列全合法值 + 与 `metadata.api` 的口径差）组织。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `RawElement.api` 缺省（AC-10） | 正向 | 元素未写 `api` → 解析成功且 `Element.api == Declared`（`#[serde(default)]` 生效，不是「缺失即错」） | 单例 |
| `RawElement.api` 两合法值（AC-10） | 正向 | `api = "declared"` / `api = "none"` → 各解析成功、字段逐值相等；`to_model` 拷贝该字段（模型侧与 raw 侧逐值相等） | 表驱动（2 值） |
| `RawElement.api` = `"non"` 拼写错（异常，AC-10） | 异常 | `api = "non"` → `DiagnosticKind::InvalidValue` 一条，文案**列出全部合法值**（`declared` / `none`）；同行号 / 文件定位在案；`api = "None"` / `api = "NONE"` / `api = ""` 同形归 `InvalidValue` | 表驱动（4 形态） |
| `classify()` 零改动：`api` 类型错归 `InvalidValue`（异常） | 异常 | `api = 1` / `api = true` / `api = ["none"]` / `api = { v = "none" }` → 全 `InvalidValue`（`invalid type` 归既有 `classify`，零分类改动） | 表驱动（4 形态） |
| `RawElement.api` 与 `metadata.api` 的口径差（AC-10 反例在案） | 边界 | `metadata = { api = "non" }`（未知键在字符串表内）→ 解析**成功**、`Element.api` 仍为 `Declared`（`metadata` 是自由字符串表，零校验）；该形态记为在案反例：想留白必须写顶层 `api = "none"`，写进 `metadata` 是静默无效 | 单例（反例） |
| 拼错键名（异常） | 异常 | `appi = "none"` → `deny_unknown_fields` 报未知键（不是静默忽略）；`api` 与 `API` 并存 → 未知键 `API` 报错 | 表驱动（2 形态） |
| 既有字段面零变化（AC-16） | 边界 | `kind` / `gate` / `path` / `docs` / `summary` / `metadata` 六字段的解析结果与改动前逐值相等（新增字段不改变既有键的处理）；`path` 原文 `./` 前缀 + 尾 `/` 逐字保留 | 单例 |
| 契约正文键被拒（AC-17） | 异常 | 元素内写 `defs = {...}` / `operations = [...]` / `signature = "..."` → 全报未知键（协议正文进模型即错，模型只存状态指针） | 表驱动（3 形态） |
| 多元素 / 大文件（边界） | 边界 | 300 元素（部分带 `api`、部分不写）→ 解析成功、逐元素 `api` 值正确、诊断空；单文件 1000 行注释 → 行号仍准确 | 表驱动（2 规模） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| TOML 解析器 | **不 mock**：真实 `toml` + `serde`（`deny_unknown_fields` 与 `default` 语义即被测对象） | 全部用例 |
| 文件系统 | **不 mock**：真实 tempdir 落 `models/*.toml`（一次 `fs::read` + 解析） | 全部用例 |
| 时钟 / 网络 / 子进程 | **零依赖** | —— |

---

### packages/desktop/src-tauri/crates/core/archi/src/scan/owner.rs -> packages/desktop/src-tauri/crates/core/archi/src/scan/owner_test.rs

#### 待测功能

- `OwnerIndex::owner_of()`: 入参准备处增一条剥离——末段形如 `<stem>.i.<ext>` 时把该段替换为 `<stem>.<ext>`，再用既有 `prefix_matches` 判定（design「修改文件」表 / D3）
- `strip_generated()`（私有）: 判据只此一处；`ext` 集合取常量 `GENERATED_EXTENSIONS` 的**五值** `{rs, ts, tsx, js, jsx}`、标记段取常量 `GENERATED_MARKER = "i"`——常量与判据处 MUST NOT 各写一份（design「修改文件」表 / D3）。**私有函数不可直调**（`owner_test` 的声明在 `scan/mod.rs`，是 `owner` 的兄弟模块）：行为经 `OwnerIndex::owner_of` 公共面观测，`MUST NOT` 为测试放宽可见性
- `normalize()`: 零改动（`\` → `/`、剥 `./`、去尾 `/` 判据逐字不变，D3）
- `prefix_matches()`: 零改动（`rel == prefix || rel.starts_with(prefix + "/")` 判据逐字不变，D3）
- `OwnerIndex::build()`: 零改动（非 `.i` 路径判据逐字不变，D3）

<!-- design.md「公共函数 / API」：本文件零行（`owner_of` 的入参准备处增一条私有 `strip_generated`，其余三条零改动）。条目改列 design「修改文件」表 `scan/owner.rs` 行（`.i.<ext>` 剥离特例与两个常量，D3 / 事实校准 7）。 -->

> 说明：测试对象取自 design「修改文件」表 `scan/owner.rs` 行与事实校准 7（剥离判据是「去 `.i.<ext>` 整段」而非「只去 `.i`」）；`ext` 五值取自 D3（缺 `js` / `jsx` 会让这两类元素的生成物落到 `Owner::None`，属静默事实错误）；用例按 AC-4（四种形态各归其元素 + 非 `.i` 路径判据零变化）与 AC-16（`path` 写法未变）组织。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `OwnerIndex::owner_of()` crate 形态生成物归属（AC-4） | 正向 | 元素 `App.Backend.Core.Archi`（`path = ["./packages/desktop/src-tauri/crates/core/archi/"]`）+ 文件 `packages/desktop/src-tauri/crates/core/archi.i.rs` → `owner_of` 返回该元素（剥离后 `…/crates/core/archi.rs` 命中前缀 `…/crates/core/archi`） | 表驱动（1 形态） |
| `prefix_matches()` 单文件前端形态（AC-4） | 正向 | 元素 `App.Frontend.Views.Architecture`（`path = ["./packages/desktop/src/views/architecture.tsx"]`）+ 文件 `…/views/architecture.i.ts` → 归该元素（`prefix_matches` 的 `rel == prefix` 分支命中） | 表驱动（1 形态） |
| `OwnerIndex::owner_of()` 子目录模块形态（AC-4） | 正向 | 元素 `App.Frontend.Views.Welcome`（`path = ["./packages/desktop/src/views/welcome-view/"]`）+ 文件 `…/views/welcome-view.i.ts` → 归该元素（`<dir>` 同级生成物） | 表驱动（1 形态） |
| `OwnerIndex::owner_of()` 嵌套目录生成物（AC-4 第四形态） | 正向 | 元素 `App.Backend.Core.Archi`（`path = ["./…/crates/core/archi/"]`）+ 文件 `…/crates/core/archi/src/scan.i.rs` → 归该元素（目录内子模块的根同级生成物） | 表驱动（1 形态） |
| `strip_generated()` 剥离判据经 `owner_of` 观测（D3 / 事实校准 7） | 正向 | 该函数是 `owner` 模块私有、`owner_test` 是其**兄弟**模块（声明在 `scan/mod.rs`）⇒ **不可直调**，故经公共面观测：元素 `path = ["./x/archi/"]` 时 `owner_of("x/archi.i.rs")` / `owner_of("x/archi.i.ts")` / `owner_of("x/archi.i.tsx")` / `owner_of("x/archi.i.js")` / `owner_of("x/archi.i.jsx")` **五扩展名全归该元素**（`GENERATED_EXTENSIONS` 五值全命中）；`owner_of("x/archi.rs")` 等非生成物形态结果与改动前逐值相等。**MUST NOT** 为测得私有函数而临时放宽可见性（禁 test-only `pub`），剥离的正确性由「五扩展名命中 + 非生成物零变化 + 反例零误归」三向间接覆盖 | 表驱动（5 正例 + 4 反例） |
| `normalize()` / `prefix_matches()` 「只去 `.i`」残留形态反例（事实校准 7） | 异常 | 若按错误判据只去 `.i`（`architecture.ts` 对前缀 `views/architecture`）→ `rel == prefix` 与 `rel.starts_with(prefix + "/")` 皆不成立 → `None`；断言真实实现下 `architecture.i.ts` 归元素（正例），且**不**出现「去 `.i` 后误归到 `views/` 元素」的错配 | 表驱动（2 形态：正例 + 错配对照） |
| `normalize` / `prefix_matches` / `OwnerIndex::build` 零改动（非 `.i` 路径零变化，AC-16） | 边界 | `src/views/architecture.ts` / `src/views/welcome-view/index.ts` / `crates/core/archi/src/scan/rust.rs` 三形态的归属结果与改动前逐值相等（新增剥离只对 `.i.<ext>` 生效）；文件名含 `.i` 但非生成物（`a.i.txt` / `a.i`）→ 判据零变化 | 表驱动（5 形态） |
| `OwnerIndex::build()` 同长多命中仍 `Ambiguous`（边界） | 边界 | 两元素 `path` 前缀长度相同的不同前缀同时命中某 `.i.rs` 文件 → `Owner::Ambiguous(vec![..])` 且字典序（剥离不改变歧义判定路径） | 表驱动（2 元素 / 3 元素） |
| `OwnerIndex::owner_of()` 生成物在未覆盖目录（边界） | 边界 | `packages/desktop/src/orphan.i.ts` 而模型无对应元素路径 → `Owner::None`（剥离只改「如何比前缀」，不造归属）；`off` 元素覆盖的生成物 → 仍返回 `Element`（`gate` 折叠由调用侧决定） | 表驱动（2 形态） |
| `OwnerIndex::owner_of()` 多扩展名齐备（边界） | 边界 | `.i.rs` / `.i.ts` / `.i.tsx` / `.i.js` / `.i.jsx` 五扩展名各一文件（同前缀元素）→ 五者全归该元素（`GENERATED_EXTENSIONS` 五值与 `paths::language_of` 的同族归并一致；缺 `js` / `jsx` 会落 `Owner::None`，D3 反例） | 表驱动（5 扩展名） |
| `GENERATED_EXTENSIONS` / `GENERATED_MARKER` 单源（D3） | 边界 | `GENERATED_EXTENSIONS` 长度恰 5 且集合为 `{rs, ts, tsx, js, jsx}`；`GENERATED_MARKER == "i"`；来源文件内这两个常量只各定义一处、判据处零重复字面量（`include_str!("owner.rs")` 计数自证） | 单例（源码自证） |
| 非生成物 `.i` 名不误剥（D3 反例） | 异常 | `a.i`（无扩展名）/ `a.i.txt`（`txt` 不在五值内）/ `a.I.rs`（大写标记）→ 剥离不生效，归属按原路径判定（与改动前逐值相等） | 表驱动（3 形态） |
| `normalize()` 路径分隔与超长（边界） | 边界 | `crates\core\archi.i.rs`（`\` 分隔）→ 归元素（归一在 `normalize`，零改动）；超长路径（>1000 chars）→ 不 panic；含 emoji 目录名 → 归属可判 | 表驱动（3 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 模型 / 图 | **不 mock**：`ArchiElement` 直接构造（纯值，不经解析） | 全部用例 |
| 文件系统 | **零依赖**：纯字符串前缀匹配，零 IO | 全部用例 |
| 时钟 / 网络 / 子进程 | **零依赖** | —— |

---

### packages/desktop/src-tauri/crates/core/foundation/src/layout/mod.rs -> packages/desktop/src-tauri/crates/core/foundation/src/layout/mod_test.rs

#### 待测功能

- `archi_api_dir()`: `<archi_dir>/api`（段名取 `API_DIR_NAME`）；纯推导零 IO；契约面与生成器工具的唯一定位入口（公共函数 / API）
- `archi_api_rel_dir()`: `<archi_rel_dir>/api`；`/` 分隔、零前导 `./`；消费者限于知识文本与 `INDEX.toml` 说明段（公共函数 / API）
- `API_DIR_NAME` 常量: `"api"`（`archi_dir` 族其余函数与 `Layout` 三字段语义零变化）（公共函数 / API）

> 说明：三行取自 design「公共函数 / API」表 `layout` 三行；用例按 `workspace-layout-resolution` 的两条 MODIFIED（`archi_dir` 族派生 + 相对族）与 AC-4 组织。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `archi_api_dir` 与 `archi_dir` 同源（正向） | 正向 | `archi_api_dir(root) == archi_dir(root).join("api")` 逐字相等；对 `root = /tmp/x` 与含空格 / 非 ASCII 的 root 均成立 | 表驱动（3 root 形态） |
| `archi_api_dir` 纯推导零 IO（边界） | 边界 | `root` 指向**不存在**路径 → 正常返回（不 panic、不 `exists()` 检查）；`root = ""`（空 `Path`）→ 返回值可判、不 panic；调用前后无任何目录被创建 | 表驱动（3 形态 + 零 IO 断言） |
| `archi_api_dir` 互不遮蔽（边界） | 边界 | `archi_api_dir(root)` 与 `archi_models_dir(root)` / `archi_decisions_dir(root)` / `archi_config_path(root)` 两两不相等且互不为前缀（`api` 与 `models` / `decisions` 平行）；`archi_api_dir` 不等于 `archi_dir`（必须多一段） | 单例（4 组两两比较） |
| `archi_api_rel_dir` 形态（正向） | 正向 | 返回 `.dev-team/architecture/api`（`/` 分隔、零前导 `./`、零尾 `/`）；`== format!("{}/api", archi_rel_dir())` 且 `starts_with(archi_rel_dir())` | 表驱动（2 断言） |
| `archi_api_rel_dir` 相对族前缀一致（边界） | 边界 | `archi_api_rel_dir()` 与 `archi_models_rel_dir()` / `archi_decisions_rel_dir()` 共享同一前缀 `archi_rel_dir()`，三者互不相等；改 `ARCHITECTURE_DIR_NAME` 类常量时三者同源随动（以「三者共同前缀长度相等」表达） | 单例（3 值对拍） |
| `API_DIR_NAME` 零字面量纪律（AC-17 前置） | 边界 | 来源文件内 `"api"` 字面量**只**出现在 `API_DIR_NAME` 定义一处（`include_str!("mod.rs")` 计数 = 1）；两个函数体零 `"api"` 字面量（MUST NOT 行内拼段） | 单例（源码自证） |
| `archi_api_dir` / `archi_api_rel_dir` 的 root 形态异常不 panic（异常） | 异常 | `root` 为相对路径（`"a/b"`）/ 含 `..` 段（`"a/../b"`）/ 带尾分隔符 / 非 UTF-8 不可表达的 `OsStr` 形态 → 派生结果可判、不 panic、不做词法归一（归一交消费方）；`root` 指向一个**文件**而非目录 → 正常返回路径串（纯推导零 IO，存在性检查交上层） | 表驱动（5 形态） |
| AC-19 四条机械 `grep` 对拍（AC-19 判据面） | 边界 | ① `grep -c 'API_DIR_NAME' crates/core/foundation/src/layout/mod.rs` **恰 4**（常量定义 + doc 引用 + 两函数消费）；② `grep -rl 'API_DIR_NAME' --include=*.rs crates/ src/` 先滤 `*_test.rs` 后**恰 1 个文件**（常量零外溢）；③ `grep -rn '"api"' --include=*.rs crates/ src/` 先滤 `*_test.rs` 后**恰 1 处**；④ `grep -cE` 对 `archi_api_dir` 与 `archi_api_rel_dir` 的交替模式于 `crates/core/foundation/src/layout/mod.rs` **恰 2**（两函数签名行）。四值任一不等即该验收点红（判据面与 AC-19 原文逐字同源，不另立计数口径） | 单例（4 断言） |
| `archi_api_dir` / `archi_api_rel_dir` 加入后既有族零变化（边界） | 边界 | `archi_dir` / `archi_models_dir` / `archi_decisions_dir` / `archi_config_path` / `archi_rel_dir` / `archi_models_rel_dir` / `archi_decisions_rel_dir` 对同一 root 的返回值与改动前逐字相等；`Layout` 三字段（`changes_root` / `archive_root` / `explores_root`）语义零变化 | 单例（7 函数 + 3 字段对拍） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 文件系统 | **零依赖**：纯路径推导（`PathBuf::join`），零 IO；「零创建」以调用前后目录枚举对比断言 | 全部用例 |
| 环境变量 / 时钟 / 网络 / 子进程 | **零依赖** | —— |

---

### packages/desktop/src-tauri/crates/core/archi/src/dto.rs -> packages/desktop/src-tauri/crates/core/archi/tests/vocabulary_test.rs

本节承载**四面词汇对拍**，执行于 crate 级集成测试 `tests/vocabulary_test.rs`（`dto.rs` 是 `ApiStatus` 词汇单点，执行文件是集成测试）。

#### 待测功能

- `ApiStatus::ALL`: 四面词汇对拍的词汇来源一侧，`api` 状态二值与线格式词逐值对拍（公共函数 / API）

> 说明：本映射为 **crate 级集成测试的扩展**（既有 `tests/vocabulary_test.rs` 承载「进程内诊断集 / DTO 投影串 / 枚举全值词表」三面相等断言，本变更把 `api` 状态并入第四个面）。测试对象取自 `desktop-archi-model` MODIFIED「消费面词汇集一致性对拍」（四面词汇集相等且 `api` 状态两值在场）与 design D25（词表从 `ALL` 派生、知识文本落地表述）；用例按 AC-8 / AC-10 组织。挂靠 `dto.rs` 为词汇单点（`ApiStatus::ALL` 定义处）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ApiStatus::ALL` 四面词汇集新增 `api` 一面（AC-8） | 正向 | 既有三面（`DiagnosticKind::ALL` / `GateTier::ALL` / `ElementKind::ALL`）词数不变 + 新增 `ApiStatus::ALL` 两值全部在场；四面词集合各自去重后长度不变 | 表驱动（4 面） |
| `ApiStatus` 线词与 DTO 投影串相等（AC-10） | 正向 | `ApiStatus::ALL` 逐值 `as_str()` 与含该值的 `ArchiElement` 序列化文本中的 `"api"` 字段值逐字相等（两面对拍，零手抄词表） | 表驱动（2 值） |
| 知识文本在场（D25 交汇） | 正向 | 经真实 `archi_knowledge::knowledge()` 产出的文本含 `api` 状态两值（`declared` / `none`）与两值的连接形态（词表从代码派生，MUST NOT 手抄）；本断言与 `archi_knowledge_test` 章节同源，此处只作四面闭合 | 单例 |
| 值域封闭（异常，Enum N+1） | 异常 | 四面词集合两两不互相包含（`api` 词不与 `gate` 档词混用：`declared` / `none` 不出现在 `GateTier::ALL` 词集，`off` / `warn` / `strict` 不出现在 `ApiStatus::ALL` 词集） | 单例（两向差集） |
| 既有三面零变化（边界） | 边界 | 改动前三面的词集与词序逐字相等（`api` 面为追加，零重排 / 零删除） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 文件系统 | **不 mock**：真实临时目录（沿 `vocabulary_test.rs` 既有 `TempDir` 装置）落模型，驱动 `parse_dir` / `ModelGraph::build` / `dto::*` | 全部用例 |
| 词汇来源 | **不 mock**：真实 `ALL` 常量与真实 `archi_knowledge::build`（四面同源即被测语义，任何替身都会抹掉对拍价值） | 全部用例 |
| `bindings.ts` 产物 / IPC runtime | **零依赖**：不读生成物、不依赖 runtime（沿既有口径） | —— |
| 时钟 / 网络 / 子进程 | **零依赖** | —— |

---

### packages/desktop/src-tauri/crates/infra/agent/src/archi_knowledge.rs -> packages/desktop/src-tauri/crates/infra/agent/src/archi_knowledge_test.rs

#### 待测功能

- `knowledge()`: 派生段追加一行 `api 状态：<ApiStatus::ALL 逐值以 " / " 连接>`（缺省词在场），词表从 `ALL` 派生、MUST NOT 手抄（design「修改文件」表 / D25）
- `build()`: `use` 面扩 `ApiStatus`；骨架拼装与两条降级判据（空骨架 / `<>` 占位符）零变化（design「修改文件」表）

<!-- design.md「公共函数 / API」：本文件零行（`build` / `knowledge` / `join` 三个内部函数签名与行为零变化）。条目改列 design「修改文件」表 `archi_knowledge.rs` 行（派生段新增一行与 `use` 面扩展，D25）。 -->

> 说明：测试对象取自 design「修改文件」表 `archi_knowledge.rs` 行（派生段增 `api 状态：<ApiStatus::ALL 逐值以 " / " 连接>（缺省 …）`；`use archi::dto::{ApiStatus, …}`；模块 doc 词表列举补第四面）与 D25；用例按 AC-10（四面词汇对拍的知识文本一面）与 AC-17（语言词禁令）组织。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `knowledge()` 派生段 `api` 行在场（D25 / AC-10） | 正向 | `knowledge()` 文本含 `api 状态：` 引导语，且 `ApiStatus::ALL` 两值 `declared` / `none` **逐值**在场（不是一个词出现即过）；缺省词单独在场（`ApiStatus::default().as_str()`） | 表驱动（2 值 + 缺省） |
| `knowledge()` 词表从 `ALL` 派生（MUST NOT 手抄） | 边界 | 文本中 `api` 行的词序与 `ApiStatus::ALL` 迭代序逐字相等（`" / "` 分隔连接）；既有三面（kind / gate / 诊断）派生行与改动前逐字不变 | 单例（行对拍） |
| `build()` 骨架自证不被破坏（既有纪律） | 边界 | 真实骨架 `include_str!` 拼装成功（`knowledge()` 为 `Some`）；`build("")` → `Err("知识骨架为空")`；`build("含 <> 占位符")` → `Err`（既有两条降级判据零变化——本变更 MUST NOT 新增 `<>` 占位符） | 表驱动（3 形态） |
| 改写的「边界」表述在场（D25） | 正向 | 文本含「契约权威在…api 目录」（旧口径「契约权威在对应能力的 spec 文本」**不**在场）；含「模型只存状态指针」与「签名 … 不进模型」两句语义 | 表驱动（4 断言） |
| 语言词禁令（AC-17 / 自查面） | 异常 | 骨架 md 内零 `crate` / `Cargo.toml` 字样；零 `api 状态：` 手写行（词表只从代码派生，不落 md）；零 `<>` 占位符 | 表驱动（3 自查） |
| 既有三面零漂移（边界） | 边界 | `ElementKind::ALL` 四值 / `GateTier::ALL` 三值 + 缺省 / `DiagnosticKind::ALL` 九词逐值在场，且派生段四行的相对顺序与改动前一致（`api` 行插在 `gate 档` 行之后） | 表驱动（3 面 + 序） |
| 示例 TOML 自证（既有纪律） | 正向 | 骨架内唯一示例 TOML 经真实 `archi` 解析面 + `ModelGraph::build` 双零诊断；新增 `api` 字段写入示例后仍可解析（若示例补 `api`，须为合法二值） | 单例 |
| 体量上限（边界） | 边界 | 文本总字符数仍在既有上限内（新增一行不使其越界）；`api` 行单行长度可判（不引入超长行） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 词表常量与目录派生 | **不 mock**：真实 `ApiStatus::ALL` / `ElementKind::ALL` / `GateTier::ALL` / `DiagnosticKind::ALL` 与 `foundation::layout` 相对路径派生（词表漂移即被测语义） | 全部用例 |
| 骨架 | **不 mock**：`include_str!` 编译期内嵌的同一份 `archi-knowledge.md`（禁测试内复制副本） | 全部用例 |
| 解析面 / 图 | **不 mock**：真实 `TomlParser` / `ModelGraph`（示例写坏的判据依赖真实解析） | 示例自证用例 |
| 文件系统 / 时钟 / 网络 / 子进程 | **零依赖**：零落盘、零 tempdir | —— |

---

### packages/desktop/src-tauri/crates/infra/agent/src/archi-knowledge.md -> packages/desktop/src-tauri/crates/infra/agent/src/archi_knowledge_test.rs

#### 待测功能

- `格式约定`: 元素字段面补 `api`（`declared` 缺省 / `none`）与一句语义，六项既有字段追加非替换（design「修改文件」表 / D25）
- `边界`: 把「契约权威在对应能力的 spec 文本」改写为「契约权威在架构域 api 目录的契约资产，模型只存状态指针 `api`」（design「修改文件」表 / D25）
- `<>` 占位符: 零新增（既有纪律：占位符即「路径未派生」）（design「修改文件」表）

<!-- design.md「公共函数 / API」：本文件零行（静态 md 骨架，无函数面）。条目改列 design「修改文件」表 `archi-knowledge.md` 行（两处改写 + 占位符零新增，D25）。 -->

> 说明：测试对象取自 design「修改文件」表 `archi-knowledge.md` 行与 D25（骨架自证三纪律零变化）；断言执行于 `archi_knowledge_test.rs`（骨架经 `include_str!` 同一份字节）。用例按 AC-10 / AC-17 组织。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `格式约定` 字段面补 `api`（AC-10 / D25） | 正向 | 「格式约定」段落的元素字段枚举行含 `api`，且同处给出二值 `declared`（缺省）/ `none`；`kind` / `gate` / `path` / `docs` / `summary` / `metadata` 六项仍在（追加非替换） | 表驱动（2 断言） |
| `边界` 段改写（D25） | 正向 | 旧口径「契约权威在对应能力的 spec 文本」**零命中**；新表述含「api 目录」「模型只存状态指针」「签名 / 导出符号 / 接口正文不进模型」三段语义 | 表驱动（4 断言） |
| `<>` 占位符零新增（既有纪律） | 边界 | 全文零 `<>`（占位符即「路径未派生」，`build` 会显式拒绝） | 单例 |
| 语言词禁令（AC-17） | 异常 | 全文零 `crate` / `Cargo.toml`；零 `api 状态：` 手写行（词表由代码派生段注入，md 内不得出现派生行文本） | 表驱动（2 自查） |
| 示例 TOML 可解析（既有纪律） | 正向 | 文中唯一示例 TOML 与字段面一致（若示例元素补 `api`，取值为合法二值）→ 经真实 `TomlParser` + `ModelGraph::build` 双零诊断 | 单例 |
| 体量（边界） | 边界 | 全文行数与字符数仍在既有上限内（新增两处改写不引入长表 / 不复制契约正文与判据长表）；零 `### ` 级 op 段（契约正文不进知识文本） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 骨架字节 | **不 mock**：`include_str!("archi-knowledge.md")` 与生产同路径同一份文件（禁测试内副本） | 全部用例 |
| 解析面 | **不 mock**：真实 `TomlParser` / `ModelGraph`（示例可解析性即被测语义） | 示例用例 |
| 文件系统 / 时钟 / 网络 / 子进程 | **零依赖** | —— |

---

### packages/desktop/src-tauri/crates/tools/archi-api-gen/src/main.rs -> packages/desktop/src-tauri/crates/tools/archi-api-gen/src/main_test.rs

#### 待测功能

- `main()`: argv → `Command`；`Usage` 打用法并退出 `2`；错误 `eprintln` + 退出 `1`（CLI 入口）；root 经 `api_gen::repository_root()` 锚定（**MUST NOT** 直接取 `current_dir()` 当 root）（公共函数 / API）
- `repository_root()`: **观察根锚定（D32）**——从 `std::env::current_dir()` 起逐级 `parent()` 上溯，首个使 `foundation::layout::archi_dir(candidate).is_dir()` 为真的目录即 root；到文件系统根仍不命中 ⇒ 退化为 cwd（`current_dir()` 失败时退化为 `.`）；私有函数（仅由 `main` 调用、不出线），源内零路径字面量（`.dev-team` / `architecture` / `"api"` 一律经 `foundation::layout` 派生）；**不为可测性新增任何导出**——单测经子模块可见性直调 `super::repository_root()`，进程级「换起手 cwd 仍同值」一条归独立进程（见「不可测试项」）（公共函数 / API）
- `api_gen::run()`: 三子命令实现体（离 `main` 便于 tempdir 驱动）——`Generate` 写四类生成物 + 目录创建（幂等）、写盘前以 `starts_with(root)` 复核落点（**D33** 第二层拦截）；`Check` 生成到内存后与磁盘比对三类偏差（内容不符 / 应删未删 / 应生成未生成）、**零写入**、有差异 `1`、孤儿扫描**全树递归且跳过构建产物目录 + 深度上限**（**D34**）；`Bootstrap` 只打 stdout、**不读 knip**（**D39**）；目录创建经 `foundation::layout` 派生（公共函数 / API）

> 说明：`Command` 枚举取自 design「变更清单」`main.rs` 行（`Generate` / `Check` / `Bootstrap(String)` / `Usage`），门禁形态取自 D21（`--check` 单条、工具内读盘比对、零 shell 逻辑、覆盖 modified / untracked / 孤儿三态），写盘边界取自 D1 + **D33**（第二层 `starts_with(root)` 复核），孤儿扫描深度取自 **D34**（全树递归 + 跳过构建产物 + 深度上限），`bootstrap` 不读 knip 取自 **D39**，`INDEX.toml` 的 `categories` 纯派生态取自 D26，`x-class` 警告对退出码零影响取自 D27，生成物不动点取自 D29，root 锚定与零行内路径字面量取自 **D32**（第七稿；事实校准 23）；用例按 AC-2 / AC-12（`generate` 四类 + 幂等、`--check` 零写盘 + 非零退出、`bootstrap` 只出 stdout、root 锚定到同一个仓、零行内字面量）组织。`repository_root` 是**私有函数**、签名零形参（design 明确定为不可测性改签名），故用例分两层：① **行为层**——`main_test` 是 `main.rs` 的 `#[cfg(test)]` 子模块，经 `super::repository_root()` **直调**即可观测真实上溯（MUST NOT 为它放宽可见性，禁 test-only `pub`）；② **源码层**——`include_str!("main.rs")` 的四条机械断言（`fn repository_root` 恰 1 处 / `archi_dir(candidate).is_dir()` 恰 1 命中 / `current_dir` 恰 1 处且不出现在 `main` 的 root 取用处 / 零路径字面量）。需**换起手 cwd** 的两条判据（「全不命中退化为 cwd」的真实分支、「不同 cwd 同值」的进程级对拍）不在单测内——改进程 cwd 会污染同进程其它用例，故归独立进程（「不可测试项」）。

- `display` 的跨 crate 复用面（**消费面备案行，非本文件函数**，D55）：工具侧**不再持有**本地 `fn display`，一切路径归一经 `archi::display`（`archi` 的 `pub` 出口，本文件只作调用方）；差异清单的串形态即该函数输出
#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `repository_root` 向上锚定到最近架构域祖先（D32 / AC-12） | 正向 | 直调 `super::repository_root()`（**私有函数对子模块可见**，`main_test` 是 `main.rs` 的 `#[cfg(test)]` 子模块 ⇒ 无需放宽可见性）：返回值是 cwd 的**真祖先**且 `current_dir()` 以它开头（`!= current_dir()` 一条即足以否证「root 取 cwd」的旧缺陷）；返回值自身满足 `foundation::layout::archi_dir(结果).is_dir()`（即 `archi_api_dir` 的父目录在案）；**「最近」判据**：返回值再上一层使 `archi_dir(..).is_dir()` 为假（唯一命中，不是最外层祖先）；连续两次调用逐字相等（无状态、幂等） | 单例（4 断言：祖先 / 自身命中 / 上一层不命中 / 幂等） |
| `repository_root` 全不命中退化为 cwd（D32 边界） | 边界 | 「全不命中」需 cwd 起于无架构域之树，本进程内不可构造（改进程 cwd 会污染同进程其它用例）⇒ 以**不变量**表达：返回值恒满足「`archi_dir(返回).is_dir()` 为真」**或**「返回值 == `current_dir()`」二者之一（退化分支的唯一合法形态），MUST NOT 为 `None` / 空路径 / panic；返回值恒为非空 `PathBuf` 且可作 `Path` 使用（`archi_dir(返回)` 派生不 panic）。真实退化分支与「不同 cwd 同值」的可观测对拍归独立进程（见「不可测试项」） | 单例（两态不变量 + 3 断言） |
| `repository_root` 源码面机械断言（D32 / 零行内字面量） | 边界 | `include_str!("main.rs")`：`grep -c "fn repository_root"` 恰 `1`；`archi_dir(candidate).is_dir()` 恰 `1` 命中（锚定判据经 `foundation::layout` 派生）；`current_dir` 恰 `1` 处且**不出现在 `main` 的 root 取用处**（`main` 内只有 `let root = repository_root();` 形态）；零路径字面量：`.dev-team` / `architecture` / `"api"` 全零命中 | 单例（源码自证 4 断言） |
| `repository_root` 私有性（禁 test-only `pub`） | 异常 | 函数无 `pub` / `pub(crate)` 前缀（仅 `fn repository_root()`）；`main.rs` 的 `pub(crate)` 面**只有** `run`（`grep -c "pub(crate) fn"` 恰 `1`）；MUST NOT 为可测性给它加可见性（本用例存在即证明该纪律） | 单例（源码自证） |
| `Generate` 写四类生成物（AC-12） | 正向 | tempdir（模型 + 元素 + 入口 + 契约 md）驱动 `api_gen::run(Command::Generate, root)` → 返回 `0`；磁盘上出现 `<FQN>.schema.json` / `INDEX.toml` / `<root>.i.rs`（`.i.ts` 按元素语言定）四类中应生成的全部文件；每个文件内容与 `api::render_all` 的 `GeneratedFile.content` 逐字节相等（工具层零改写） | 表驱动（Rust 元素 / TS 元素两形态） |
| `Generate` 幂等（AC-2 / AC-12） | 边界 | 连跑两次 `Generate` → 第二次返回 `0` 且磁盘全量字节快照与第一次**逐字节相同**（内容相同也重写，字节一致）；`--check` 随后退出 `0` | 单例（两次对拍） |
| `Generate` 目录创建经 layout（AC-12 / AC-17） | 边界 | 初始 tempdir 无 `.dev-team` → 生成后 `<archi_api_dir>` 目录自动建成；来源文件内零 `".dev-team"` / `"architecture"` / `"api"` 路径字面量（`include_str!` 源码自证），目录全经 `foundation::layout` 派生 | 单例（源码自证 + 落盘断言） |
| `Generate` 契约错误（异常，D20） | 异常 | md 的 H1 与文件名不一致 → `Err(String)`（`run` 返回 `Err`，`main` 打 stderr 退 `1`）、**零写入**（失败前不得留下半成品：断言 tempdir 全量字节快照与调用前相等）；错误文案含 `file:line` | 单例 |
| `Check` 无差异（AC-12） | 正向 | 先 `Generate` 再 `Check` → 返回 `0`、stdout 打 `{"mismatched":[],"orphaned":[],"missing":[]}`（JSON 可解析、三数组皆空） | 单例 |
| `Check` 内容不符（AC-2 / D21） | 异常 | 手工改一处 `<FQN>.schema.json` 的一个字段 → `Check` 返回 `1`、`mismatched` 含该文件 repo 相对路径；**磁盘内容未被修复**（零写入：改坏的字节调用后仍在） | 单例 |
| `Check` 应删未删（孤儿生成物，D21） | 异常 | 磁盘上多一个匹配生成物命名规则但不在应生成集的文件（如契约删除后遗留的 `<FQN>.schema.json`）→ `Check` 返回 `1`、`orphaned` 含该文件；**该文件未被删除**（零写入） | 单例 |
| `Check` 孤儿扫描覆盖任意深度（**D34**） | 异常 | 在三处深度各落一个孤儿 `.i.rs`：仓库顶层（`x.i.rs`，**唯一能被「只看顶层」覆盖的形态**）、两层（`crates/infra/archi.i.rs` 形态）、三层（`src/views/y.tsx.i.ts` 形态）→ **三处全部**出现在 `orphaned`（D34：只扫顶层会让除顶层以外的全部孤儿对门禁不可见）；三层那条缺失即该用例红 | 表驱动（3 深度） |
| `Check` 扫描跳过构建产物目录（**D34**） | 边界 | 在 `target/` / `node_modules/` / `dist/` / `build/` / `.git/` 内各落一个 `x.i.rs` → **不出现在** `orphaned`（构建产物目录递归跳过）；同名文件落在正常目录 → 正常报出（跳过面精确到目录名而非文件名）；极深嵌套（超过 `MAX_MODULE_WALK_DEPTH`）→ 不栈溢出、返回不 panic | 表驱动（5 目录 + 1 深度上限） |
| `Generate` 零写盘跨界拦截（**D33** 第二层） | 异常 | 若某 `GeneratedFile.path`（被人为构造成）不以 `root` 开头（`/tmp/outside/x.i.rs` / `../outside/x.i.rs`）→ `write` 以 `starts_with(root)` 复核**拒绝写入**并返回 `Err`，磁盘上该路径**零文件**（第一层形态校验与第二层 join 后复核交叉验证，任一层失效另一层仍拦）；对照合法路径正常写入 | 表驱动（2 越界形态 + 1 合法对照） |
| `Check` 应生成未生成（D21） | 异常 | 契约新增但未跑 `Generate` → `Check` 返回 `1`、`missing` 含应生成路径；三类偏差同时存在 → 三个数组各自非空（互不吞并） | 表驱动（2 形态 + 三类并存） |
| `Check` 零写盘总断言（AC-12） | 边界 | 对三种偏差形态各跑一次 `Check`，调用前后 tempdir 全量字节快照**逐字节相等**（含目录 mtime 之外的全部内容）；来源文件内零 `fs::write` / `create_dir_all` 出现在 `Check` 分支（`include_str!` 源码自证） | 表驱动（3 形态） |
| `Check` 未启用契约面（D17） | 边界 | tempdir 无 `<archi_api_dir>` → 应生成集只含 `INDEX.toml`（其余空）；若磁盘也无任何生成物 → 返回 `1`（`missing` 列 `INDEX.toml`）或按「契约面未启用即无期望」返回 `0`——两态择一，断言「两态中其一且 `--check` 不 panic、零写入」（该分支口径由 test-gen 以实际实现为准固化） | 单例 |
| `Check` 检出 `categories` 段差异（AC-2 / D26） | 异常 | 手改已生成 `INDEX.toml` 的 `[entry.categories]`（把 `object = 1` 改成 `object = 2`，或整段删除）→ `Check` 返回 `1`、`mismatched` 含 `INDEX.toml`；**磁盘未被修复**（零写入）；对照：契约不变时重跑 `Generate` → 该段逐字节复原（`categories` 是纯派生） | 表驱动（2 变形形态） |
| `Check` 不受 `x-class` 警告影响（D27 / AC-12） | 边界 | 契约含「`properties` 全为函数却未标 `x-class`」的元素 → `Generate` / `Check` 退出码仍 `0`（警告只走 `emit::render_target` 的 stderr），`INDEX.toml` 与 `--check` 的 JSON 差异清单三数组逐字不受影响；同一输入两次 `Check` 退出码相同 | 单例（三向断言） |
| `Check` 生成物不动点守卫（AC-12 / D29） | 边界 | 生成后对 `.i.rs` 的 `rustfmt --check` 与 `.i.ts` 的 `vp fmt --check` 均零差异（`--check` 成功后追加的一次 `--check` 兜底，D29）；若手工把 `.i.rs` 的一个折行改坏，`api:check` 报 `mismatched` 而**不是**静默通过；本工具内零 `std::process`（源码自证：格式化器不 spawn，不动点靠模板 + 外部 `--check` 门兜住） | 表驱动（两态：不动点 / 人为破坏） |
| `Bootstrap` 只出 stdout（AC-12） | 正向 | `api_gen::run(Bootstrap("App.Backend.Infra.ArchiRuntime"), root)` → 返回 `0`；输出文本含 H1 与 `api-op` fence；调用前后 tempdir 字节快照相等（零写盘） | 单例 |
| `Bootstrap` 不读 knip（**D39**） | 边界 | 源码自证：`main.rs` 内 `knip` **零命中**、零子进程（`Command::new` 零命中）；行为面：草稿文本内**零 `unused`** 词（`Visibility::Unused` 生产面不可达，`bootstrap` 不产生该降级）；`--check` 的退出码不受任何 knip 相关分支影响 | 单例（源码自证 2 断言 + 行为） |
| `Bootstrap` 元素不存在（异常） | 异常 | `Bootstrap("App.Nope")` → `Err(String)` 且文案含该 FQN；零写盘 | 单例 |
| `Usage` 与 argv 解析（AC-12） | 边界 | 空 argv / 未知子命令 / `bootstrap` 缺元素参数 → `Usage` 分支（退出码 `2`）；`--check` 与 `check` 词面差异按 D21 固定为 `--check`；`cargo run … -- --check` 经 cargo 剥首个 `--` 后得单参 `--check` | 表驱动（4 形态） |
| 退出码面三分（AC-12） | 边界 | `0`（成功 / 无差异）/ `1`（契约或 IO 错误 / 有差异）/ `2`（用法错）三值互不混用；同一输入两次运行退出码相同（确定性） | 表驱动（3 码） |
| 零 spawn（AC-12 红线） | 边界 | 判据取 `Command::new` / `process::exit` **双零命中**（`include_str!("main.rs")` 源码自证；**`std::process` 路径本身不是判据**——`main` 返回 `ExitCode` 需 `use std::process::ExitCode;` 恰 1 行，与 design「变更清单」与 tasks 自查同口径）；`knip` 由人工驱动、不由本工具调 | 单例（源码自证 3 断言） |
| 空 root / 不存在 root（边界） | 边界 | `root` 指向不存在路径 → `Generate` 返回 `Err`（模型不可读）或 `0` 且只写 `INDEX.toml`，断言不 panic 且零残留半写文件；`root` 为空 `Path` → 不 panic | 表驱动（2 形态） |

| 路径归一单点跨 crate 复用且本地零副本（AC-33 ①） | 正向 | 源码自证：`main.rs` 行首 `fn display` 零命中（本地实现已删）；`archi::display` 命中 ≥ 1（调用面改指跨 crate 单点） | 单例（源码自证） |
| 差异清单路径串与跨 crate 单点同源（AC-33 ②） | 正向 | `archi::display(root, 绝对路径)` 输出 = 仓库根相对 POSIX 串；`--check` 的 `mismatched` 数组与之**逐字相等**（零第二份归一实现） | 单例 |
#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 文件系统 | **不 mock**：真实临时目录（`tempfile::TempDir`，或沿 `archi` 先例的自建 `TempDir` RAII——见「测试框架与既有布局」）+ 真实读写；三类偏差以「手工改字节 / 多落一个文件 / 少落一个文件」注入（不改权限、不依赖平台） | 全部用例 |
| `archi` 侧的解析 / 装配 / 渲染 / 判据 | **不 mock**：`run` 调真实 `api::load_contracts` / `render_all` / `bootstrap`（工具层零判定即被测语义，AC-12 / D1） | 全部用例 |
| `foundation::layout` | **不 mock**：真实派生（目录创建单点即被测语义） | 全部用例 |
| 进程边界（argv / stdin / stdout） | **不 mock 但不下探**：用例直调 `run(command, root)`（离 `main` 的设计目的）；`main` 的 argv 解析以 `parse_args` 的输入输出对驱动；不捕获真实 stdout、不断言进程退出码常量以外的东西（退出码经 `run` 返回值断言） | argv / 退出码用例 |
| 运行环境（cwd） | **零 mock 进程级 cwd、不 `set_current_dir`**：`repository_root` 的入参面是**隐式的进程 cwd**（签名 `fn repository_root() -> PathBuf` 零形参，design 明文不为可测性改签名），故用例**直调** `super::repository_root()` 观测真实上溯（私有可见性足够），但**不改**全局 cwd（`set_current_dir` 会污染同进程其它并行用例，属装置雷区）；「全不命中退化」与「不同 cwd 同值」两条需换起手位置的判据归独立进程（`cargo run -p archi-api-gen -- --check`，test-execution 机械步骤），不在单测内 | `repository_root` 五行用例 |
| 网络 / 时钟 / 环境变量 / DB | **零依赖**：本 crate 零 Tauri / 零 store / 零 config / 零 orchestration / 零 workflow（AC-12） | —— |

---

### packages/desktop/src-tauri/crates/infra/archi/src/runner.rs -> packages/desktop/src-tauri/crates/infra/archi/src/runner_test.rs

#### 待测功能

- `LocalArchiCheck::file_stem()`: 由私有自由函数迁入 `impl` 并提升为 `pub`，实现体逐字不动（design「修改文件」表 / D23）
- `LocalArchiCheck::new()`: 改由生成物 `.i.rs` 提供，手写 `impl` 内的 `new()` 删除；`impl Default` 保留并调 `Self::new()`（design「修改文件」表 / D23）
- `LocalArchiCheck`: 手写 `pub struct` 删除，由 `pub use api_gen::*;` 接管（否则 E0255）（design「修改文件」表）
- `use crate::LocalArchiCheck;`: 普通 `use` 行（**零 `pub use`**）——`LocalArchiCheck` 由 `lib.rs` 的 `pub use api_gen::*;` **单点**提供，`runner.rs` 只为本文件的 `impl` 块解析引入该名，零第二出口（design「修改文件」表 / tasks 阶段十四 ③ 的 D38 / A2 回溯改正）
- `compose_text()`: 保持私有，不进契约（design「修改文件」表 / D23）

<!-- design.md「公共函数 / API」：本文件零行（`runner.rs` 零行；`new` 改由生成物提供属「修改文件」表行，`compose_text` 保持私有）。条目改列 design「修改文件」表 runner.rs 行（删 struct 与 new / impl Default 保留 / file_stem 迁入 impl 并提升 pub / compose_text 保持私有）与 D23。 -->

> 说明：测试对象取自 design「修改文件」表 `runner.rs` 行与 D23（试点 op = `file_stem`；两处定义移除、一处可见性提升、实现体逐字不动）；用例按 AC-13（试点端到端：无 `### new`、生成物与手写 `impl Default` 共存、判据零命中）、AC-16（唯一代码形状改动限于可见性提升与两处定义移除）与 **AC-30 ④（机械随动第四类的归属二分——`runner.rs` 的零再导出面属实现相位已收口的产品形态，其「否定断言必须可判红」属测试相位写面）** 组织。既有 44 条 `runner_test` 用例全部保留，本节只列新增 / 改写用例。**回溯补记（本轮）**：产品面已是「`runner.rs` 零 `pub use`、只 `use crate::LocalArchiCheck;`」（`grep -cF "pub use" runner.rs` = 0），而既有断言串 `pub use crate::LocalArchiCheck;` 是其**超集** ⇒ 当前绿但**空转**（非报告所称「恒红」）；本设计把它改写为**锚全串 `pub use`** 的形态（`!runner.contains("pub use")`），与 doc「`LocalArchiCheck` 由 `pub use api_gen::*;` 单点提供」协同：doc 说「单点提供」，断言则证「`runner.rs` 侧零第二出口」。该断言与阶段十四 ③「`runner.rs` 零再导出」以及 AC-30 ④ 的（丙）档逐条同源，MUST NOT 退回复合串形态。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `LocalArchiCheck::file_stem()` 正例（AC-13） | 正向 | `"2026-10-20T08:31:12Z"` → `"20261020T083112Z"`（过滤出数字 + `T` + `Z`，无冒号）；`"2026-10-31T00:00:00+00:00"` → 结果只含数字与 `T` / `Z` | 表驱动（3 时间戳） |
| `LocalArchiCheck::file_stem()` 空输入退化（边界） | 边界 | `""` → `"report"`（既有退化分支逐字不变）；`"abc"`（零数字零 T/Z）→ `"report"`；`"T"` → `"T"`（有字符即不退化） | 表驱动（3 形态） |
| `LocalArchiCheck::file_stem()` 路径安全（边界） | 边界 | 任意输入下结果不含 `:` / `/` / `\` / `*` / `?` / `"` / `<` / `>` / `\|`；长度 < 255；超长输入（>1000 chars 全数字）→ 不 panic（返回值形态可判） | 表驱动（3 形态） |
| `LocalArchiCheck::file_stem()` 可见性与位置（AC-16） | 边界 | 函数可从 crate 外经 `LocalArchiCheck::file_stem` 调用（`pub` 生效）；`report.rs` 内零 `fn file_stem` 定义（实现体已迁移，非复制）；实现体与迁移前逐字相同（`git diff` 人工核对 + 行为用例覆盖） | 单例（编译级 + 源码自证） |
| `LocalArchiCheck::new()` 由生成物提供（AC-13） | 正向 | `LocalArchiCheck::new()` 可用且等于 `LocalArchiCheck`（零字段）；手写侧零 `pub struct LocalArchiCheck;` 定义（源文自证：`include_str!("runner.rs")` 不含该声明）；`pub use api_gen::*;` 编排下无 E0255 | 单例（编译级 + 源码自证） |
| 签名见证失配即编译错（异常，反例在案） | 异常 | 把 `LocalArchiCheck::file_stem` 的签名改成两个参数 → `cargo check -p archi-runtime` 报 **E0308** 并点名 `crates/infra/archi.i.rs` 的行号（生成物 `const _: fn(..)` 见证生效）；改回原签名即恢复（该反例留痕于 tasks 阶段十四自查，本节以「见证语句在生成物内且指向 `file_stem`」的断言固化判据） | 单例（编译级反例） |
| 生成物缺失即编译错（异常） | 异常 | 删除 `crates/infra/archi.i.rs` → `pub use api_gen::*;` 指向不存在文件 → 编译失败（E0583 / E0425）；断言「`LocalArchiCheck` 的类型与 `new()` 只来自生成物」（手写侧无第二份定义，故无静默降级路径） | 单例（编译级） |
| `LocalArchiCheck` 定义移除 + `Default` 共存（AC-13 / 事实校准 4） | 正向 | `LocalArchiCheck::default()` 与 `LocalArchiCheck::new()` 行为等价（同输入同 outcome）；`impl Default` 调 `Self::new()`（不是重复构造逻辑） | 单例 |
| `run` 行为零变化（AC-16） | 边界 | 既有 44 条用例（clean / warnings / errors / 结构错拦截 / `skipped` 逐字文案 / `noChanges` 逐字文案 / 报告同源 / 反馈有界 / 写盘失败降级 / 取消 / 生产路径 `fault` 恒 `None` / 槽位隔离 …）全部保持通过；新增断言：`run` 产出的报告 `api_findings` 为槽位存在但**不影响** `passed` / `diagnostics`（D18 在门禁面的体现） | 表驱动（既有全量 + 1 新断言） |
| 试点元素判据零命中（AC-13） | 正向 | tempdir 落试点形态（模型元素 + 入口 + 契约 md + 生成物）→ `run` 的 `passed == true` 且 `report.api_findings` 为空（除 `apiUndeclared` 关闭） | 单例 |
| `compose_text()` 保持私有（D23） | 边界 | `compose_text` 不进契约、不被 `pub use`（`include_str!("runner.rs")` 含 `fn compose_text` 无 `pub` 前缀；crate 外不可调——以「生成物内零 `compose_text` 见证」表达） | 单例（源码自证） |
| `LocalArchiCheck` 构造与 `Default` 共存的再导出面（AC-13） | 正向 | 生成物侧 `pub use` 面为零透传时（试点契约零 `api-reexport`）逐一见证：`.i.rs` 内 `pub use ` 计数为 **0**（`!pilot.contains("pub use ")`，与 `compose_text` 章节的生成物见证共用同一事实源）；该否定断言的对象是**生成物**、不是 `runner.rs`——两处断言的被检文件 MUST 分列，以免一处排版变化同时削弱两条判据 | 单例（源码自证） |
| `write_report` 不进契约（事实校准 11） | 边界 | `pub(crate) fn write_report` 形态保持；`lib.rs` 的 `pub use` 不含 `write_report`（E0364 反例不在案） | 单例（源码自证） |
| 本地引用面零第二出口（**AC-30 ④ / D38 / tasks 阶段十四 ③**） | 边界 | `include_str!("runner.rs")` **不含 `pub use`**（零再导出，`!runner.contains("pub use")`）——`runner.rs` 只以普通 `use crate::LocalArchiCheck;` 引入该名供本文件 `impl` 块解析；否定断言 MUST 锚**全串 `pub use`** 而非复合串（见下一行的失效形态反例）；`lib.rs` 的 `pub use api_gen::*;` 是 `LocalArchiCheck` 的唯一再导出单点 | 单例（源码自证） |
| 复合串断言的失效形态（**空转禁例，回溯补记**） | 异常 | 既有 `!runner.contains("pub use crate::LocalArchiCheck;")` 对当前 `runner.rs` **恒真**（`runner.rs` 内 `"pub use"` 计数为 **0**，复合串是其超集）⇒ 断言绿但**零判据力**：任何把 `use crate::LocalArchiCheck;` 误写成 `pub use crate::LocalArchiCheck;` 的回归只需排版或换行变化（如 `pub use\ncrate::LocalArchiCheck;`）即令该串失配而断言**保持绿**，形态漂移因此不可见。改写后该形态必须由上一行的全串断言捕获（判据 = 上一行断言的判据串是 `"pub use"`，本行断言其**判据力**：`include_str!("runner.rs")` 内 `grep -cF "pub use"` 为 0 而断言的否定形态仍可判红——以「向 `runner.rs` 注入 `pub use crate::X;` 一行则上一行必红」的性质保证） | 单例（失效形态反例 + 判据力自证） |
| 零债务消解（AC-16） | 边界 | `ArchiCheckRunner` 实现体、`FIX_HINT` / `TRUNCATED` 常量、报告装配逻辑逐字未变（`git diff` 人工核对 + 源码自证零新增分支） | 单例 |
| 第四类机械随动的调用形态适配（**D53 / AC-30 ①**） | 边界 | `crates/core/archi/src/api/emit/{rust,ts}_test.rs` 的 `use` 行、三参 `render(..)` → 四参、`render_with_root(..)` → 四参 `render(..)` 与 `EmitTarget { .. }` 字面量提成函数四项**编译前置**改动在案：整批回退后 `cargo clippy --workspace --all-targets` **必红**（E0061 / E0433），恢复后必绿——这是「编译必需、非可选清理」的正面证据；**MUST NOT** 借此改断言值 / 断言意图 / 用例名（那五项归测试相位，见「不可测试项」） | 单例（两态编译判据） |
| `modules` 查表值经 `paths` 单点（**D40 / D48**） | 边界 | 生成物内的 `pub use` 模块路径逐字等于 `paths::rust_module_path(目标元素根)`；TS 侧 specifier 逐字等于 `paths::ts_import_specifier(form, module_ref.name)`（两处真值源在本 crate 外，本 crate 只消费）；Rust 侧末段为关键字时**不产** `pub use`（只出注释）——生成器的形态闸在 `archi` 侧，本 crate 只承接结果 | 表驱动（3 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 判定（`archi::drift::check`） | **不 mock**：真实调用（同源即被测语义，AC-9 / AC-10） | 全部用例 |
| 写盘（`write_report`） | **不 mock**：真实写盘；失败面以「目录位置普通文件占位」注入 | 写盘失败降级 |
| 生成物 `.i.rs` | **不 mock**：真实编译期生成物（`pub struct` / `new()` 来自生成文件本身——这是 E0255 判据的唯一事实源）；`file_stem` 见证经生成的 `const _: fn(..)` 在编译期生效 | 全部用例 |
| 文件系统 / 模型 / 源文件 | **不 mock**：真实 `tempfile::TempDir`（`infra/archi` 既有 dev-dependency） | 全部用例 |
| 时钟 / 网络 / 子进程 | **零依赖**：`timestamp` 只断言形态；本 crate 零 spawn（AC-17 红线） | —— |

---

### packages/desktop/src-tauri/crates/infra/archi/src/report.rs -> packages/desktop/src-tauri/crates/infra/archi/src/report_test.rs

#### 待测功能

- `write_report()`: 调用点改为 `LocalArchiCheck::file_stem(&report.timestamp)`；签名与 `pub(crate)` 可见性零变化、不进契约（design「修改文件」表 / 事实校准 11）
- `file_stem()`: 私有定义删除（实现体迁往 `runner.rs` 的 `impl`，非复制）（design「修改文件」表 / D23）

<!-- design.md「公共函数 / API」：本文件零行（`write_report` 的签名与可见性零变化）。条目改列 design「修改文件」表 `report.rs` 行（删私有 file_stem + 调用点改经 LocalArchiCheck + write_report 仍 pub(crate)）与 D23。 -->

> 说明：测试对象取自 design「修改文件」表 `report.rs` 行与 D23（`file_stem` 的实现体迁往 `runner.rs`，`write_report` 仍 `pub(crate)`、`visibility = crateOnly`、不进契约）；用例按 AC-13（试点端到端）与 AC-16（实现体逐字不动）组织。既有 `report_test` 用例全部保留，本节只列新增 / 改写用例。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `write_report()` 调用点改经 `LocalArchiCheck::file_stem`（AC-13） | 正向 | `write_report(root, name, &report)` 且 `report.timestamp = "2026-10-20T08:31:12Z"` → 文件名为 `20261020T083112Z.json`（与迁移前逐字相同——行为零变化的直接断言） | 表驱动（3 时间戳） |
| `file_stem()` 迁移后零副本（AC-16） | 边界 | `report.rs` 内零 `fn file_stem` 定义（`include_str!("report.rs")` 零命中）；`file_stem` 的行为断言只在 `runner_test` 章节（单一事实源，禁两份等价实现） | 单例（源码自证） |
| 目录派生单点（既有） | 正向 | 落点逐字等于 `change_reports(root, name, ReportSlot::Archi)`；`reports/test` / `reports/baseline` 零创建 | 单例 |
| 内容往返与 `api_findings`（AC-8 交汇） | 正向 | 写完读回反序列化与入参报告逐字段相等（含新增 `api_findings` 与 `version == 2`）；`api_findings` 非空时亦逐项往返 | 表驱动（2 形态：空 / 非空） |
| 可见性零变化（事实校准 11） | 边界 | `write_report` 仍 `pub(crate)`（crate 外不可达；`lib.rs` 无 `pub use report::write_report`）；该函数不出现在契约声明的 op 集合（`visibility = crateOnly` ⟹ 不进契约） | 单例（编译级 + 源码自证） |
| 写盘失败降级（既有） | 异常 | 目录位置被普通文件占位 → `Err`、零半写残留（失败前置到 `create_dir_all`） | 单例 |
| `version = 1` 硬写随动（交接清单） | 边界 | 既有 `empty_report(timestamp)` 夹具里 `version: 1` 的**值面**留待测试相位按 `REPORT_VERSION = 2` 决定（tasks 收尾自查 E 单列的三处值面随动之一）——本节用例断言「夹具构造可编译 + 序列化往返成立」，不在此处固化版本号语义 | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 文件系统 | **不 mock**：真实 `tempfile::TempDir`；写盘失败以「同名普通文件占位」注入 | 全部用例 |
| `foundation::layout::change_reports` | **不 mock**：真实派生（目录单点即被测语义） | 全部用例 |
| `LocalArchiCheck::file_stem` | **不 mock**：真实调用（本变更的核心接缝） | 全部用例 |
| serde_json | **不 mock**：真实序列化 / 反序列化 | 内容往返 |
| 时钟 / 网络 / 子进程 / DB | **零依赖**：`timestamp` 由报告字段给出 | —— |

---

### packages/desktop/src-tauri/src/commands/archi/mod.rs -> packages/desktop/src-tauri/src/commands/archi/mod_test.rs

#### 待测功能

- `archi_module_inner()`: 出参 `ArchiModuleDetail` 含 `api` 并列字段（`status` / `contractPath` / `schemaPath` / `generatedPath`），由 `dto::module_detail` 填充、命令层零改动（design「路由 / API 设计」表）
- `archi_graph_inner()` / `archi_read_doc_inner()` / `archi_change_summary_inner()` / `archi_set_gate_inner()` / `archi_drift_inner()`: 签名与出参零变化，命令数量仍五个（design「路由 / API 设计」/ `desktop-archi-model` MODIFIED）

> 条目来源：design.md「公共函数 / API」子表对本文件**零行**（六命令与六个 `*_inner` 签名零变化；`archi_module` 的出参形状扩展由 `dto` 内部填充）。两条目改列 design「路由 / API 设计」表（`archi_module` 出参加 `api` 并列字段，命令层仍是薄包装）；本节为 AC-10 的验证面补入——design「变更清单」不含本文件（命令层零代码改动），但 proposal AC-10 的验证方法点名 `commands/archi/mod_test`，且 `desktop-archi-agent` MODIFIED 要求 `archi_module` 单元素响应含 `api` 并列字段，不出本节则该验收点无行可落。

> 说明：测试对象取自 design「路由 / API 设计」的 `archi_module` 行（`ArchiModuleDetail` 新增 `api: { status, contractPath, schemaPath, generatedPath }`）与「修改的能力」`desktop-archi-agent`（薄概览同步带 `api.status`）；用例按 AC-10（`archi_module` 单元素响应含 `api` 并列字段、无契约即 `status = none` 且路径 `null`）/ AC-13（试点元素 `api.status = declared`）/ AC-9（命令面与门禁面同源）组织。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `archi_module_inner()` 出参含 `api`（AC-10） | 正向 | tempdir 落元素 + 契约 md → `archi_module_inner` 返回 `ArchiModuleDetail`，其 `api.status == Declared`、三路径均 `Some` 且为 repo 相对 POSIX 串；序列化后文本含 `"api":{"status":"declared",…}` | 单例 |
| `archi_module_inner()` 无契约 → `none` 且路径 `null`（AC-10） | 边界 | 元素存在但无 md → `api.status == None`、`contractPath` / `schemaPath` / `generatedPath` 三键值为 `null`（零配置、可枚举「有契约 / 无契约」） | 单例 |
| `archi_module_inner()` 容器元素（AC-10） | 边界 | `path = []` 的容器元素 → `status == None` 且三路径 `null`（天然无契约，命令层零特判） | 单例 |
| `archi_module_inner()` 试点元素 declared（AC-13） | 正向 | 试点元素 + 试点契约在案 → `api.status == declared`；同时 `archi_drift` 命令面对同输入的报告 `api_findings` 为空（该元素判据零命中） | 单例（两命令交叉） |
| `archi_drift_inner()` 命令面与门禁面同源（AC-9） | 边界 | 同输入下 `archi_drift_inner` 的报告 `api_findings` 与 `LocalArchiCheck::run` 落盘报告反序列化后逐字段相等（仅 `timestamp` / `change` / 落盘路径允许不同）；命令层零新增判据分支（源码自证 `include_str!("mod.rs")` 零 `api_findings` 构造） | 单例（两面对拍 + 源码自证） |
| 命令层零领域规则（AC-9） | 边界 | `mod.rs` 内零 `archi::api::` 调用（`api` 字段全由 `dto::module_detail` 填）、零路径拼装；`archi_module_inner` 只做「参数转换 → `dto::module_detail` → 错误映射」 | 单例（源码自证） |
| `archi_graph_inner` / `archi_read_doc_inner` / `archi_change_summary_inner` / `archi_set_gate_inner` / `archi_drift_inner` 零变化（AC-16） | 边界 | `archi_graph` / `archi_read_doc` / `archi_change_summary` / `archi_set_gate` / `archi_drift` 的出参与错误映射逐字不变；命令数量仍五（`desktop-archi-model` MODIFIED 要求）；`archi_read_doc` **不**覆盖 `<archi_api_dir>/**`（契约正文不新增读取通路） | 表驱动（5 命令 + 1 否定断言） |
| 空白 / 无效 root（既有） | 异常 | `root` 为空白 / 不存在 / 文件而非目录 → `Err`（既有 `check_root` 口径，零变化）；错误文案不含契约面词汇 | 表驱动（3 形态） |
| 只读命令零写盘（既有） | 边界 | `archi_module` 调用前后 tempdir 字节快照相等（新增 `api` 字段只是一次 `exists()` 定位） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| Tauri 运行时 | **不 mock 领域**：进程边界装置（Tauri 运行时替身，`tauri::test::mock_app()`，非仓内模块 mock）+ 真实 `WorkspaceStores` 托管——State 相关命令经 `app.state` 直调，无 State 的四命令直调原签名 | 全部用例 |
| `archi::dto::module_detail` / `archi::drift::check` | **不 mock**：真实调用（投影与判定单源即被测语义，AC-9） | 全部用例 |
| 文件系统 / store | **不 mock**：真实 tempdir 落模型、源文件、契约 md；真实 `WorkspaceStores`（数据根指向 tempdir） | 全部用例 |
| 时钟 / 网络 / 子进程 | **零依赖**：本层零 spawn | —— |

---

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/bash.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/bash_test.rs

#### 待测功能

- `resolve_timeout_ms()`: `timeout_ms` 入参解析——缺省 `120s` 底座；显式值须在 `[1s, 30min]` 内，越界**显式 `Err`**（不静默钳位，模型据此自行调整申请值）；非整数 / 负值 / 缺失键三形态可判。**可见性由 `fn` 提升为 `pub(crate)`**：兄弟测试模块 `bash_test` 直测入参闸所需，实现体与返回语义**逐字不动**、不涉公共面（design「修改文件」表 `bash.rs` 行）

<!-- design.md「公共函数 / API」：本文件零行（`resolve_timeout_ms` 是 `pub(crate)` 内部工序，不进公共函数表）。条目改列 design「修改文件」表 `bash.rs` 行（`fn` → `pub(crate) fn` 的可见性一提 + doc 注明「不涉公共面」，实现体逐字不动）。 -->

> 说明：测试对象取自 design「修改文件」表 `bash.rs` 行（`resolve_timeout_ms` 提升为 `pub(crate)`，`git diff HEAD` 实测仅 **+2 / −1**：一行签名改写 + 一行 doc）；用例按 **AC-16 的「并列允许项零条」读法**组织——该可见性提升是**清单修改表成员**（不是测试相位白名单项，MUST NOT 双计），其判据是「实现体逐字不动 + 入参语义零变化」，既有 `bash_test` 用例全部保持通过。**本节新增一条源码自证用例**「`resolve_timeout_ms` 可见性提升的实现体零改动」（见下「用例」表末行）——它只固化可见性面与实现体零改动，不复测入参闸（后者由既有三条用例逐面覆盖）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `resolve_timeout_ms` 缺省底座（既有，口径登记） | 正向 | 无 `timeout_ms` 键 / 空对象 → `Ok(120_000)`（`DEFAULT_TIMEOUT_MS` 底座，不落校验分支） | 表驱动（2 形态） |
| `resolve_timeout_ms` 上下界原样放行（既有，int 边界） | 边界 | `1_000` / `1_800_000`（`[1s, 30min]` 两端）→ `Ok(原值)`（不钳位）；`999` / `1_800_001` → `Err`（显式拒绝，不静默钳位到边界值） | 表驱动（4 形态） |
| `resolve_timeout_ms` 类型与符号面（异常，int 边界） | 异常 | `-1` / `1.5` / `"1000"`（字符串）/ `null` → `as_u64` 失败即 `Err(「必须是非负整数毫秒」类文案)`；`0` → 落越界分支 `Err`（不在 `[1s, 30min]` 内） | 表驱动（5 形态） |
| `resolve_timeout_ms` 可见性提升的实现体零改动（AC-16） | 边界 | 源码自证：`git diff HEAD` 对该文件仅 **+2 / −1**（一行签名 `fn` → `pub(crate) fn` + 一行 doc），零逻辑行改动；`resolve_timeout_ms` 在 `bash.rs` 内**恰 1 处**定义、零 `pub`（不是 `pub`，不扩大公共面）；本行**不入**机械随动白名单（清单修改表成员，见「不可测试项」`bash.rs` 行）；行为面：`timeout_ms_of(json!({}))` ⇒ `120_000`（实现体逐字不动 ⇒ 语义零变化） | 单例（源码自证 3 断言 + 行为面 1 断言） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `serde_json::Value` 入参 | **不 mock**：以 `json!` 直构真实 `Value`（入参闸语义即被测对象） | 全部用例 |
| 子进程 / 壳 | **不 mock 但不下探**：本节用例只测 `resolve_timeout_ms` 的**入参闸**（纯函数，零 spawn）；真实 spawn 面归既有 `bash_test` 的端到端用例，不在本节 | —— |
| 时钟 / 网络 / 环境变量 | **零依赖** | —— |

---

## 不可测试项

以下条目属 design「变更清单」的新增 / 修改文件，但**无单元测试路径**（成因逐条说明）。它们不进入 `## 单元测试`，进入验收的方式落在「机械门面 / 命令 + 留痕 / 编译期」三类，与 proposal「测试文件」表的对应行一致。

| 文件 / 模块 | 类别 | 不可测试原因 | 验收方式 |
|---|---|---|---|
| `packages/desktop/src-tauri/crates/core/archi/src/api/common.json` | 静态映射规则资产 | 纯静态 JSON（`include_str!` 编译期内嵌、零函数面、零代码路径），无独立测试文件可落；其**展开结果**已由 `api/emit/rust.rs`（`Outcome` → `Result<T, String>`）与 `api/emit/ts.rs`（`Outcome` → `{ ok } \| { err }` + `degraded`）章节断言，`sha256` 摘要前 12 位由两侧头注释用例断言 | 章节交叉（见「单元测试」的 `api/emit/*` 两节）；「MUST NOT 长出带生成产物的类型」以内容审查核验（R-12） |
| `packages/desktop/src-tauri/crates/tools/archi-api-gen/Cargo.toml` | 清单面 | 声明文件（`name` / `edition` / 三个依赖 + 红线注释），无逻辑面；依赖面是编译期事实 | AC-12 依赖审查 + `cargo tree -p archi-api-gen -e normal` 零 `tauri` / `store` / `config` / `orchestration` / `workflow` / `infra` 命中（命令 + 留痕） |
| `packages/desktop/src-tauri/Cargo.toml` | 清单面 | workspace `members` 与 `[workspace.dependencies]` 两行追加，无逻辑面 | AC-12 / AC-15：`grep -n "archi-api-gen"` 恰两处且不在壳 `[dependencies]`；`cargo check -p archi-api-gen` 通过 |
| `packages/desktop/src-tauri/Cargo.lock` | 构建产物（设计补列） | cargo 自动写入的 `[[package]]` 条目（`name = "archi-api-gen"` + `archi` / `foundation` / `serde_json` 三条依赖），零手写面 | AC-12：条目在场且依赖面恰三条（`grep -A6 'name = "archi-api-gen"'` 人工核对）；`cargo tree -p archi-api-gen -e normal` 与锁内条目一致；文件随 `cargo check` 自动更新、不进手工编辑 |
| `packages/desktop/package.json` | 配置面 | `scripts` 四条（含 `api:export` / `api:check`，且 `server:check` 的 fmt 段改校验模式）+ `version` 一行；脚本内容为跨平台单命令、**不带 `cd` 前缀**（root 由工具侧 D32 锚定），执行归静态门（test-execution 机械步骤） | AC-14：`pnpm run api:check` 退 `0`（cwd = `packages/desktop`，**root 由 D32 锚定到仓库根**——脚本不带 `cd`，故此处无需 `cd` 则自证锚定生效）+ `bindings:export` 再生成后零 diff + `version` = **当前 `HEAD` 值 + 1 patch**（判据是相对递增，MUST NOT 写死字面号，事实校准 25）且 `src-tauri/Cargo.toml` 版本不随动（D30 + 事实校准 21 / 23）；`check` 链逐字为 `server:check && bindings:export && api:check && client:check`（第二段是 `bindings:export`，`scripts` 内零已退役 `bindings:check` 键，事实校准 24）；`server:check` 含 `cargo fmt --all --check` 而**不含**写模式 `cargo fmt --all &&`（D28）；`npx vp fmt package.json --check` exit 0（工作树为 `vp fmt` 不动点，行尾 LF；索引态 `i/crlf` 保留，事实校准 22） |
| `packages/desktop/knip.json` | 配置面 | 一条 `ignore` 追加；`knip` 是对生成物的死代码判定工具，其判定不在单测射程 | AC-6 / R-10：`client:check`（含 knip）通过；`ignoreExportsUsedInFile` / `ignoreDependencies` 零新增（`git diff` 审查） |
| `packages/desktop/src/types/generated/bindings.ts` + `packages/desktop/src/types/generated/bindings.zod.ts` | 生成物 | `bindings:export` 机械产出，零手写面；结构零 diff 由 `bindings:export` 再生成后人工核对（零 diff 守卫已退役，沿 `desktop-ipc-type-bindings` 三条纪律「沿用而非修改」，事实校准 24） | AC-14：`pnpm run bindings:export` 后 `git diff` 零行；含 `ApiStatus` / `ArchiApiRef` / `ArchiApiFindingKind` / `ArchiApiFinding` / `apiFindings`，**不含** `ApiFindingSeverity`（D14：进程内枚举、零 serde / spectra） |
| `.dev-team/architecture/api/Infra/App.Backend.Infra.ArchiRuntime.md` | 数据资产（手写契约） | markdown 契约资产（内容是数据不是代码）；其**正确性判据**已在 `api/contract.rs` / `api/schema.rs` / `api/check.rs` 章节以等价夹具全覆盖（H1 锚点、fence 归属、双向 gate、构造器禁令） | AC-13：`cargo run -p archi-api-gen -- --check` 退 `0`；`grep` 断言零 `### new`；人工读一遍（tasks 阶段十四） |
| `.dev-team/architecture/api/Infra/App.Backend.Infra.ArchiRuntime.schema.json` | 生成物（数据资产） | 生成物，零手改自由度；逐字段派生完备性由 `api/schema.rs` 章节的 `build_schema` 用例断言（六字段出处） | AC-2 / AC-13：`api:check` 零 diff；ajv（draft 2020-12 + `strict: "log"`）消费前置由外部工具承担 |
| `.dev-team/architecture/api/INDEX.toml` | 生成物（数据资产） | 生成物；文本形态与计数语义由 `api/schema.rs` 章节的 `build_index` 用例断言（说明段三行 / 七字段 / 字典序 / 零语言词 / `[entry.categories]` 形状分布与整段省略），打标口径由 `api/mod.rs` 章节的 `def_shape` 用例断言 | AC-2 / AC-17：`api:check` 零 diff；试点条目出现 `[entry.categories]` 且恰 `object = 1`，其余 `absent` 条目无该段，`sum(categories) == types` 逐条成立（tasks 阶段十九收口）；`grep -nE "crate\|Cargo\.toml\|Rust\|TypeScript"` 零命中 |
| `packages/desktop/src-tauri/crates/infra/archi.i.rs` | 生成物 | 生成物（`pub struct` + `new()` + 两行签名见证），零手写面；其**形态**由 `api/emit/rust.rs` 章节断言（含格式化器不动点，D29），其**生效**由 `runner_test` 的编译级断言（E0255 / E0308）覆盖 | AC-13：`cargo check -p archi-runtime` 通过；`rustfmt --edition 2021 --check` 于该文件 **exit 0**（D29 不动点，事实校准 19）；把 `file_stem` 参数改成两个 → 报 `E0308` 并点名生成物行号（tasks 阶段十四自查） |
| `packages/desktop/src-tauri/crates/core/archi/src/lib.rs` | crate 根（模块声明 + 出口） | 无 `lib_test.rs` sidecar；内容为 `pub mod api;` 与若干 `pub use`，无独立行为面（出口可达性是编译期事实） | AC-9：出口符号可被下游 import（`cargo check --all-targets` + `commands/archi` 与 `infra/archi` 两消费方编译通过）；`grep` 断言 `check as api_check` / `api_ref` / `bootstrap` 在出口内，且 `ApiFindingSeverity` 等进程内类型不误出线。**AC-33 ①（D55）**：`pub use api::display;` 在出口内且**成行**（`grep -cF "pub use api::display"` ⇒ **1**；分组块写法会让判据恒 0）；`CommonRule` **不出口**（无 crate 外消费者，D55 明禁顺手扩大出口面） |
| `packages/desktop/src-tauri/crates/infra/archi/src/lib.rs` | 入口编排 | 无独立 `lib_test.rs`；三行 `#[path]` / `mod api_gen;` / `pub use api_gen::*;` 是编译期编排（`#[path]` 值是**字符串字面量**，与 `paths::mod_path_attr` 的输出逐字相等——**不是**「由该函数产出」，D38：Rust 属性值不能由函数求值；等值性由 `api/paths.rs` 章节的三形态断言 + tasks 逐入口自查行双守，入口上方注释 MUST 如实表述、MUST NOT 再写「由该函数产出」） | AC-4 / AC-13：`cargo check -p archi-runtime` 通过（无 E0255 / E0425 / E0364）；`runner_test` 经同一 crate 编译即验证 `pub use` 生效；`#[path]` 值以 `grep` 断言为 `"../../archi.i.rs"` |
| `.dev-team/architecture/models/05-backend-tools.toml` | 数据资产（架构模型） | 数据文件（元素 + 2 条边），非代码；其语义由既有模型解析面承载，本变更加的是数据行 | AC-15：`grep -c '^\[element\.'` = 3、`grep -ch '^\[\[relation\]\]'` = 3；`archi_graph(root)` 零诊断零告警；双向覆盖比对命令 + 留痕落 `reports/` |
| `.dev-team/architecture/decisions/0001-model-granularity.md` | 数据资产（ADR） | 文档；计数复述四处 | AC-15：`grep -n "51 元素 / 113 关系\|49 条"` 三处同值；`grep -n "50 元素\|111 关系\|47 条\|2 元素 / 1 关系"` 零命中（陈旧计数清零） |
| `AGENT.md` + `CLAUDE.md` | 仓库面文档（**一行两文件**） | markdown 文档（各一节 `## 模块 API 契约`、五条要点，零代码）；文档正确性靠机械 `grep` + `diff` 与评审 | AC-18：`grep -c '^## 模块 API 契约'` 在两文件各 **1**；`grep -c 'apiEntryMissing'` 在两文件各 **0**（判据长表未复制）；节内零 `openspec/specs/**` 路径；`diff AGENT.md CLAUDE.md` 恰 **3** 处差异且全在节外（`:1` 标题 / `:3` 首行说明 / `CLAUDE.md` 缺末尾换行）；节内正文两文件逐字相同；人工核对「未复制契约正文、判据长表与 fenced json 示例」（只指向 `INDEX.toml` 与 capability 名） |
| `packages/desktop/src/**/*.i.ts`（本期零份） | 生成物（前端） | 本期试点在 Rust 侧，实际零份文件（待决问题 3）；`.i.ts` 路径全链路（`entry_form` 四分支 / `ts_import_specifier`）**有实现无真实数据** | 由 `api/paths.rs` / `api/emit/ts.rs` 章节的等价夹具单测覆盖（AC-4 / AC-5 / AC-6）；首个前端元素契约落地时才首次端到端 |
| CLI 根锚定的**进程级**可观测判据（`repository_root` 的「不同 cwd 同值」与「全不命中退化」两条真实分支） | 机械门面 / 命令 + 留痕 | 两条都需**换起手 cwd**：单测内 `set_current_dir` 会污染同进程其它并行用例（装置雷区），而 `run(command, root)` 的入参面按 design 明文**不变**（root 由调用方给），故进程内只能在**当前** cwd 下观测上溯与不变量（见「单元测试」`main.rs` 章节四条用例：祖先 / 自身命中 / 上一层不命中 / 幂等 + 退化的两态不变量 + 源码面 4 断言 + 私有性）。本节只登记必须换 cwd 的进程级部分 | AC-12 / tasks 阶段十三自查：在 `packages/desktop`（该 cwd 下无 `.dev-team`）跑 `cargo run -q --manifest-path src-tauri/Cargo.toml -p archi-api-gen -- --check` 与在仓库根跑同一命令，**两处输出逐字相同**、退出码同值；反向判据（在案）：若把 root 改回 cwd，自 `packages/desktop` 起手那次必然列全量 `missing` 并退 `1`（D32 / 事实校准 23） |
| 端到端零 diff 门（`api:check` 手工留痕） | 机械门面 | 属静态门机械步骤（`package.json` 脚本驱动），agent 内不重复执行、不复制测试运行脚本细节 | AC-2 / AC-14：改契约 md 后不重生成 → `api:check` 非零退出并列差异文件；重生成后零 diff（tasks 阶段十五自查 + `reports/` 留痕） |
| 判据基线对拍（`apiEntryMissing` 28 / `apiMissing` 13 / 其余 0） | 命令 + 留痕 | 依赖全仓真实模型与真实入口命中数（14 个 `src/lib.rs`），不是可复现的单测夹具（把本仓 42 个 `path` 非空元素塞进 crate 语料会违反「MUST NOT 把本仓模型塞进 crate 语料」） | AC-7 / tasks 收尾自查 C：本仓根跑一次全量判据，按 `kind` 分组统计并与基线表逐项核对，结果落 `openspec/changes/module-api-contract-format/reports/`；任一项不等即缺陷（MUST NOT 改基线表放行） |
| `crates/infra/agent/src/sdk/{context,loop,loop_test,resume,runner_test,tools_test}.rs`（六份，**已移出修改表**） | 人工核对（**不在本变更任何集合内**） | **第二十九稿已把六份移出修改表**：它们在 `HEAD` 已无 rustfmt 债务（`rustfmt --check` 对 `HEAD` 副本 **6/6** exit 0），「清偿」是空动作；因此它们不在清单、不在机械随动白名单、不在实现相位 60 路径内，判据不再有例外分支 | AC-16 零例外判据：`git diff HEAD -- crates/infra/agent/src/sdk/` **零行**（与「核对口径」段的 `comm -3` 双向零行同源）；`rustfmt --edition 2021 --check` 对 `HEAD` 副本 6/6 exit 0（`HEAD` 无债务的正面证据）。**唯一例外**是 `bash.rs` 的 `resolve_timeout_ms` 提升为 `pub(crate)`（兄弟测试模块 `bash_test` 直测所需，归清单修改表；不进本节） |
| 第四类机械随动的**断言面与用例名**（`api/emit/{rust,ts}_test.rs`，D53 / AC-30 ④——归测试相位，非实现相位） | 测试相位写面（实现相位**无**授权） | 五处改动与「只为可编译」的符号引用面**无关**：① `rust_test.rs` 的签名自证串；② `emit/mod_test.rs` 的「职责三件」清单；③④ **跨 crate 两处 `pub use` 断言改写**（`api/mod_test.rs` 的 `render_all` 查表筛选面 + `crates/infra/archi/src/runner_test.rs` 的零第二出口面——后者由本轮回溯按 `runner.rs` 零 `pub use` 的产品形态并入本档，见「单元测试」`runner.rs -> runner_test.rs` 章节）；⑤ `ts_test.rs` 的**六处用例名去 `_with_root`**（用例名零编译必要性，撤掉后静态门仍绿 ⇒ 不在第四类射程内）。**接手红点**：用例名与断言串指代的是已删的三参形态，语义上已落伍但 `cargo nextest` 不因它变红；**禁止项**：MUST NOT 借改名调整断言、MUST NOT 把本条改写成任何形态的实现相位灰度授权（MUST NOT 留下不可编译形态） | 归**测试相位**：由 test-gen 在本相位完成（判据 = 五处逐项在案、`tasks.md` 阶段二十四 ⑤ 的（丙）档与 disposal 一致）；实现相位只承担 `use` 行 / 实参补齐（元数）/ 装置同名提取三形态（见 `api/emit/rust.rs` 与 `ts.rs` 章节的第四类用例） |
| `openspec/changes/module-api-contract-format/specs/desktop-archi-api/spec.md`（S-1 / S-2 / S-3 / S-6 甲类 + S-7 / S-8 / S-9 / S-10 乙类 + S-11 / S-12 丙类 + S-13 / S-15 / S-16 / S-17 丁戊两类）与 `specs/desktop-archi-api-check/spec.md`（S-14） | 规范文本项（D31；本 change 的 spec 相位写面） | 规范措辞同步，非代码；判据是 `grep -F` 双向对拍（旧串零命中 + 新串恰命中），无单测路径。**本设计相位结束时的合法态是这十四处的旧原文仍在**（本 design 相位不做 `specs/**` 写入）。**丁类三条与戊类两条的判据形态与甲乙丙类不同**：S-13 / S-14 是**保留型**（旧句须保留，判据只取新串在场），S-15 / S-16 / S-17 是**纯新增条**（旧原文串 0，只判新串恰命中）；S-14 另有半条属替换型（其 Module Contract 表 `api/check.rs` 行的旧串「声明集 vs 导出集」须消失、新串须在场，是丁类唯一的旧原文串） | 十四条逐条双向 `grep -F`（`design.md`「spec 措辞同步项」表内逐行给出原文串与替换串）；新串两两不得互为子串（否则「恰命中 1」会假通过，D31 纪律：S-7 / S-10 / S-12 / S-13 / S-15 都含「本期留白 / 本期未接线 / 位置词汇 / 两道人字闸」类语素，故各取更长的定形句；丁 / 戊两类的判据串 MUST 取定形长句而非裸词——`Position` / `apiMissing` / `转义` / `keywords` 裸词在替换后文本里仍大量出现，属自证型假判据）——由 spec 相位 + 阶段二十自查 ② 执行（甲类命中计数 **4** + 乙类 **4** + 丙类 **2**，合计 **10**；丁 / 戊两类另行按 design 的「只取新串」口径核对） |
| `openspec/specs/desktop-ipc-type-bindings/spec.md`（S-4 / S-5） | 规范文本项（D31；主基线） | 主基线文本同步，**本 change 任何相位 MUST NOT 直接改**（proposal「不要修改」首条）；只有归档 SpecSync 步才有该写面 | 需回溯 spec 相位 / 归档 SpecSync：S-4（`:292` 的 CLI 表 `check` 行三段链原文）与 S-5（`:273` 的 Module Contract 表 `package.json` / `knip.json` 行「`bindings:export` / `check` 命令形态不变」）逐条双向 `grep -F`；未做则记为「待归档步消解」的文本漂移风险（待决问题 11，不进代码门禁） |
| **`EmitTarget` 两字段删除的 19 处构造形态适配**（D58 ⑤ / D53 第四类机械随动——归实现相位，本设计只登记） | 实现相位写面（测试相位无授权） | 五份既有用例里共 **19 处** `EmitTarget {` 构造字面量（逐文件 `grep -cE '^\s*(let [a-z_]+ = |&)?EmitTarget \{$'`：`emit/mod_test.rs` 2 + `emit/rust_test.rs` 5 + `emit/ts_test.rs` 10 + `api/paths_test.rs` 1 + `tests/pilot_test.rs` 1），两字段初始化行合计 **38 行**，删初始化行与随之无用的 `use` 是**编译前置**（不改用例名 / 断言值 / 断言意图 / 增删用例 / 夹具语义）；它们已在测试相位 44 文件集内计数，**不增机械随动白名单项** | `tasks.md` 阶段三十 ④（实现相位执行）；判据 = `cargo clippy --workspace --all-targets -- -D warnings` exit 0 且构造字面量全部为三字段形态（既有 19 处改后 + 本相位新增用例的 1 处 = 20 处）；断言面与用例名归本相位（见 `emit/mod.rs` 章节的「D58 装置写面」注） |
| `packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs`（D56 装置超时越界回滚） | 既有装置（**不在本变更任何集合内**） | 该文件与 `module-api-contract-format` 零产品面关联；回滚（确认窗回到 60s、删 `register_window`、复合用例收敛等待回到 `wait_for` 30s 约定上限）属**测试相位的门禁前提**，实现相位对它零触碰 | AC-33 ③（D56）：`grep -cF 'from_secs(180)'` **0**、`grep -cF 'register_window'` **0**、`wait_for` 回到 30s 约定上限、装置退出判据「见过 run 登记后转 `None`」保留；`git diff HEAD` 对该文件为空（已回滚）——归 test-execution 机械步骤复跑 |
| AC-21…AC-34 中「只可机械核对、无单测引用面」的部分（`grep` 判据与行为对拍的另一半） | AC 判据面（机械门 + 留痕） | 这些条件的判据是**源码定形串计数**与**行为对拍**，其中计数类无法写成断言（测试读自己的源码做 `include_str!` 断言会与 `cargo fmt` 的折行耦合，且部分判据要求「逐产品文件枚举」而非目录级扫描，属静态门形态）；行为类中的两条依赖**整批回退**（AC-24 的直连面恰 2 命中、AC-30 ① 的「撤 hunk 必红」）不可在单测内构造 | AC-24 ①②③④ / AC-25 ②③ / AC-26 ③ / AC-27 ②③ / AC-28 ③ / AC-29 ②③ / AC-30 ②③⑤ / **AC-31 ③⑤ / AC-32 / AC-33 ① / AC-34 ①②③④** 的**源码字符串判据**由 tasks 阶段二十三~三十与收尾自查逐条执行（判据串一律锚符号名与定形串、MUST NOT 写死行号）；行为面对拍部分落各 sidecar 章节的用例（见「单元测试」），两者之和即该 AC 的全部条件。**AC-31 / AC-32 的判据面例外**：`fn common_type` / `fn protocol_key` / `parse_common` 三个符号的计数判据可写成断言（`include_str!` + 定形串），其行为面（改表即变产物 / 空表回落）落 `emit/{rust,ts}.rs` 与 `api/mod.rs` 章节 |
| 判据基线表的两处「不可在 crate 语料内复现」判据（`apiEntryMissing` 28 / `apiMissing` 13 的全仓形态） | 命令 + 留痕 | 全仓 42 个 `path` 非空元素与 14 个 `src/lib.rs` 命中构成真实输入，MUST NOT 把本仓模型塞进 crate 语料；单测只构造**等价形态**（27 + 1 与 14 − 1）并断言条数与互斥关系，真实数字由全量实跑核对 | AC-7 / AC-22 基线：本仓根跑一次全量判据并按 `kind` 分组统计，结果落 `openspec/changes/module-api-contract-format/reports/`；任一项不等即缺陷（`apiMissing` 得 14 即试点自身 `$defs` 类型有真缺口，D44） |
| `#[path]` 读盘面的**仓外哨兵未读**（AC-26 ① 的否定断言） | 边界（装置面） | 需在 tempdir **之外**建第二个平级目录放唯一内容哨兵；单测可构造，但「未读」的判据是 `read_to_string` 未被调用而非文件内容比对——故只保留「符号集为零 + 哨兵内容不出现在任何符号/错误文案中」两条可判断言，**不捕获系统调用**（零 mock 红线） | AC-26 ①：符号集零 + 哨兵内容零出现；另由 `starts_with(root)` 的源码判据（见 `api/extract/rust.rs` 章节）承担结构前提 |
| **跨 crate 两处 `pub use` 断言改写**（AC-30 ④ / D53 / tasks 阶段二十四 ⑤（丙）③④ + 阶段二十六 / 二十七的接手红点——归测试相位，本轮回溯补入） | 测试相位写面（实现相位**无**授权） | 两处落点**都不是**「只为可编译」的符号引用面（撤掉改写 `hunk` 后 `cargo clippy --workspace --all-targets` 仍绿 ⇒ 不在第四类射程内）：① `packages/desktop/src-tauri/crates/core/archi/src/api/mod_test.rs` 的 `render_all建emit_target的两次求值`——D48 / D50 令「透传目标末段非法 ⇒ 不入查表 ⇒ 只出 `// 透传自 <FQN>` 注释、不产 `pub use`」后，旧断言「含 `pub use …::AgentView;`」**必红**（产品面已不产）；② `packages/desktop/src-tauri/crates/infra/archi/src/runner_test.rs` 的「本地引用面不经二次再导出」——产品面 `runner.rs` 已零 `pub use`（只 `use crate::LocalArchiCheck;`），既有复合串判据 `"pub use crate::LocalArchiCheck;"` 是超集 ⇒ **空转（绿但零判据力，非恒红）**，须改锚全串 `pub use`。两处的断言**意图**均须保留（① 仍见证「`modules` 值不经 FQN 派生」+ 新增见证「非法末段不产出不可编译文本」；② 仍见证「`LocalArchiCheck` 单点由 `pub use api_gen::*;` 提供、`runner.rs` 零第二出口」） | 归**测试相位**（本设计即其 owner）：判据 = 两处逐项在案且改写后**可判红**——① 以「把透传目标末段改回合法形态」为变异须令断言从绿转红；② 以「向 `runner.rs` 注入 `pub use …` 一行」为变异须令断言从绿转红；两处 MUST NOT 保留只在特定排版下成立的复合串，MUST NOT 借改写放宽断言（那属反向改产品面）；复跑验收范围由 test-execution 机械步骤承担 |
