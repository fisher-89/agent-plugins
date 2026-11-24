# 任务: archi-contract-authoring-guidance

> **覆盖口径**：覆盖 proposal「变更范围 - 实现文件」**全量 3 行**（`core/archi/src/api/mod.rs`、`infra/agent/src/sdk/mount.rs`、`packages/desktop/package.json`）。proposal「测试文件」的 2 行**不属本相位、不入本文**——用例名 / needle / 拆分边界由 design「变更清单 · 修改文件」的两行（`mod_test.rs` / `mount_test.rs`）与 D9 单点持有，交测试相位落地。
>
> **不在本 change**：`docMissing` 文案补 bootstrap 指路、`bootstrap` 新增参数、自动生成 `api-reexport` fence、注入矩阵扩展、散文语义变判据、存量 10 份契约回填作者注。
>
> **三个硬事实（本文件的任务全部建立在它们之上，见 design 事实校准 1–4）**：
> ① `contract::parse` 的 `finish_section`（`api/contract.rs:1089`）丢弃空 body 段 ⇒ `sections.len() == 6` 要求**六段各带正文**，故 `## 操作` / `## 类型` 各补一行含 CJK 的「（待补：…）」；
> ② 非 `api-*` fence 是解析器**结构性排除**的唯一载体（`api/contract.rs:164`）⇒ 作者注只能落 `text` fence；
> ③ 作者注含 `{}` 与 `### ` ⇒ 作者注取模块级 `const`（不进 `format!`），且既有「零 op 锚点」断言须改为行首扫描（属测试相位随动，见 design D9）。
>
> **静态步骤归属**：`pnpm check` 链（`server:check` → `bindings:export` → `api:check` → `client:check`）与 `pnpm test` 由桌面端 static-check / test-execution 执行；本文只登记边界与只读自查命令。本变更零生成步骤（无生成物变化）。
>
> **阶段收口即可编译**：阶段零零改动（只读核对）；阶段一只动 `api/mod.rs`（纯文本拼接）；阶段二只动 `mount.rs` 的描述字面串；阶段三只改一行版本号 + 只读核对。每阶段收口时一次提交。
>
> 自查 cwd：仓库根；`packages/desktop/src-tauri/crates/core/archi`（草稿骨架）；`packages/desktop/src-tauri/crates/infra/agent`（描述面）；`packages/desktop`（`package.json`）。

## 阶段零：起点核对（只读，零改动）

- [x] 核对六段入 `sections` 的前提：读 `crates/core/archi/src/api/contract.rs:1089` `finish_section`（`body.trim().is_empty()` 即不入 `sections`）与 `:177` `strip_prefix("### ")`（只有行首 `### ` 才是 op 锚点）——确认现草稿过 `parse` 后 `sections` 恰 2 段（职能 / 不变量）、加两段后仍只 4 段，故 `## 操作` / `## 类型` 必须各补一行正文
- [x] 核对惰性载体：读同文件 `:164` 的「其余 fence（示例代码等）不参与契约，内容不入散文区」+ `:1001` `collect_reexport_prose` / `:1048` `collect_surface_table`（两者只扫 `section.body`），确认作者注放文件末尾的 `text` fence 零判据
- [x] 核对起点：`crates/core/archi/src/api/mod.rs:759` `fn bootstrap_text(fqn: &str, symbols: &[ApiSymbol]) -> String` 与 `:690` `pub fn bootstrap(root: &Path, element: &str) -> Result<String, ApiError>` 的现状文本；`crates/infra/agent/src/sdk/mount.rs:296` / `:252` 两个描述函数现状文本；`packages/desktop/package.json:3` = `0.7.9`
- [x] 核对待决 1 的现状：`openspec/changes/archi-contract-authoring-guidance/specs/desktop-archi-api/spec.md` 的「占位句形态」bullet（要求**六段**占位正文）与无符号场景句「`## 操作` 段为空」不一致——本相位按 design 待决 1 取「六段各有占位正文」，不改 delta 文件

## 阶段一：草稿六段骨架 + 尾部惰性作者注（`crates/core/archi/src/api/mod.rs`）

- [x] 在 `bootstrap_text` 之上新增模块级常量 `const BOOTSTRAP_NOTE: &str`，内容**逐字**取 design「数据模型 §1」的 8 行块（首行 ```text、末行 ```，含两处续行缩进）；doc 一句「作者注：解析器对非 `api-*` fence 显式跳过 ⇒ 惰性、零判据；MUST NOT 移出 fence（放进裸正文 / `## ` 段正文 / `### ` heading 各有装载期后果）」；MUST NOT 插值、MUST NOT 进 `format!`（块内含 `{ "导出名": … }`，会破坏 `format!` 解析）
- [x] 同文件 `bootstrap_text` 起始 `format!(...)` 的末段由 `"## 操作\n\n"` 改为 `"## 操作\n\n（待补：每个操作一个三级标题 + 紧随一个 api-op fence）\n\n"`（占位句 MUST NOT 含行首 `### `、MUST NOT 以 `- ` / `* ` / `|` 起手、MUST 含 CJK）
- [x] 同函数 `:779` 行改为 `text.push_str("## 类型\n\n（待补：本模块自有 $defs，一个 api-types fence，同文件唯一）\n\n```json api-types\n");`（`api-types` fence 的位置与内容源 `bootstrap_types(symbols)` 不动）
- [x] 同函数 `:781`–`:782` 的收尾（`"\n```\n"`）改为 `"\n```\n\n## 边界\n\n（待补：本模块**不**负责什么）\n\n## 承载理由\n\n（待补：为什么它是独立元素）\n\n"`，随后 `text.push_str(BOOTSTRAP_NOTE);`，再 `text`——`## 边界` / `## 承载理由` 的占位句逐字取 proposal 表
- [x] 同函数 doc comment 更新为「md 骨架文本（H1 + 六段：职能 / 不变量 / 操作 / 类型 / 边界 / 承载理由 + 末尾非 `api-*` fence 作者注）」并保留既有两段说明（注记行取符号的定义文件与行；入口退出注记面）
- [x] 不动面核对：`bootstrap` / `bootstrap_op` / `bootstrap_types` / `json_type` / `entry_problem_reason` 与全部错误文案逐字不变；`bootstrap_text(fqn: &str, symbols: &[ApiSymbol])` 与 `pub fn bootstrap(root: &Path, element: &str) -> Result<String, ApiError>` 两处签名逐字不变（`:4988` 的源码自证断言依赖这两个字面串）
- [x] 自查 阶段一（只读 grep，不跑测试）：
  `grep -c 'const BOOTSTRAP_NOTE' crates/core/archi/src/api/mod.rs` = 1；
  `grep -c 'BOOTSTRAP_NOTE' crates/core/archi/src/api/mod.rs` = 2（定义 + 使用）；
  `grep -c '## 边界\|## 承载理由' crates/core/archi/src/api/mod.rs` = 2；
  `grep -c '（待补：每个操作一个三级标题' …/api/mod.rs` = 1 且 `grep -c '（待补：本模块自有 \$defs' …/api/mod.rs` = 1；
  `grep -n '### ' …/api/mod.rs` 只在两处既有位置命中：`format!("### {}\n\n…")` 的 op 锚点与 `BOOTSTRAP_NOTE` 块内那一行（无第三处）；
  `grep -c 'fs::write\|fs::create' crates/core/archi/src/api/mod.rs` 与改动前同值（零新增写调用）；
  `grep -c 'symbol.file' crates/core/archi/src/api/mod.rs` 仍 2（注记面两处写出点不动）

## 阶段二：工具描述补位（`crates/infra/agent/src/sdk/mount.rs`）

- [x] `archi_api_definition()`（`:296`）描述按 design「数据模型 §2」逐字改写：首句 `（自检）` → `（自检与草稿）`；`bootstrap` 一句补「新建 / 补契约的第一步就取它」；把泛称「生成物」换成四类点名（契约 md 同目录 `<FQN>.schema.json`、架构域 `INDEX.toml`、入口同级 `.i.rs` / `.i.ts`）并保留「过期用既有生成命令重生成，MUST NOT 手改生成物」；补冷启动口径「`check` 的 `artifacts.enabled = false` 即契约面未启用（此时 `findings` 恒空，不等于零问题）」；保留 `concat!` 形态，`parameters` 的 `action` 二值 enum / `element` / `required` 逐字不动
- [x] `archi_module_definition()`（`:252`）在「新元素的字段形态用 archi_scaffold 取模板」之后、诊断收尾句之前插入「；`api.schemaPath` / `api.generatedPath` 给另两条生成物路径（`contractPath` 已指手写真相）」；其余句子（含 `modelFile` + `modelLine` 可直接 `edit`、`hasPath`、`resolvedEntry`、单值 `doc` 两半）逐字保留
- [x] 不动面核对：`ARCHI_API_TOOL` / `ARCHI_MODULE_TOOL` 常量、`registry()` 项数与次序、`lookup()`、`definitions()`、`ToolTier::Read` / `ToolExec::Archi` 归属零变化
- [x] 自查 阶段二（只读 grep）：
  `grep -c '第一步' crates/infra/agent/src/sdk/mount.rs` = 1；
  描述内含 `.schema.json` / `INDEX.toml` / `.i.rs` / `.i.ts` / `enabled` / `findings`（同句）/ `MUST NOT` / `手改生成物` / `重生成`；
  `grep -c 'schemaPath' …/mount.rs` = 1 与 `grep -c 'generatedPath' …/mount.rs` = 1（均在 `archi_module` 描述内）；
  禁词零命中：`grep -c 'pnpm\|cargo run\|api:export\|archi-api-gen' crates/infra/agent/src/sdk/mount.rs` = 0；
  `archi_module` 描述内零 ``` 与零「三档」（脚本判：`grep -c '三档' …/mount.rs` 仍 **1**——唯一命中是 `:68` 的挂载档 doc，描述内零新增）；
  `grep -c '"docs"' …/mount.rs` = 0

## 阶段三：版本与零外溢核对（不重复执行桌面端静态步骤）

- [x] `packages/desktop/package.json`：`"version"` `0.7.9` → `0.7.10`；`src-tauri/tauri.conf.json` 仍 `"version": "../package.json"`（不改）；`src-tauri/Cargo.toml` 的 `[package] version`（`0.1.0`）逐字不变
- [x] 零外溢核对（`git status --porcelain` 的改动集 ⊆ design D7 白名单 = 实现面 3 行 + 本 change 文稿 + 测试相位承接的 2 行）：出现 `crates/core/archi/src/api/contract.rs`、`crates/infra/agent/src/archi-knowledge.md` / `archi_knowledge.rs` / `archi_knowledge_test.rs`、`crates/core/workflow/src/prompts/**`、`.dev-team/architecture/**`、`crates/core/archi/src/dto.rs`、`api/paths.rs`、`src-tauri/src/**`（IPC）、`src/types/generated/**`、`src/**`（前端产品代码）、`crates/tools/archi-api-gen/**`、`sdk/archi_tool.rs`、`sdk/archi_tool_test.rs`、`AGENT.md` / `CLAUDE.md`、`openspec/specs/**` 任一 diff 即回退
- [x] 零写盘与零生成物核对（只读）：`bootstrap` 的调用链（`elements_of` → `paths::entry_state` → `extract::extract_element` → `bootstrap_text`）零 `fs::` 写调用；`git status --porcelain -- packages/desktop/src-tauri/crates/core/archi/tests packages/desktop/.dev-team .dev-team` **零输出**（语料 / 金样 / 模型 / 契约资产逐字节不变）；`graph/INDEX.toml` 与 `models/**` 零 diff（`api:export` 预期零写入，生成步骤由桌面端 `api:check` 判定）
- [x] 验收对照：逐条回应 design「验收标准回应」表（AC-1…AC-10）——AC-1 / AC-2 / AC-9 由阶段一自查背书；AC-3 / AC-4 由「六段各有占位正文」+ `bootstrap_types` 零符号返回 `{}` 背书（断言由测试相位落地）；AC-5 / AC-6 / AC-7 由阶段二自查背书；AC-8 由本条零 diff 核对背书；AC-10 由零外溢白名单与桌面端 `check` 链背书
