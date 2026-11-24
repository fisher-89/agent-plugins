# 测试设计: archi-contract-authoring-guidance

> **日期**: 2026-11-24
> **输入**: 本 change 目录 `proposal.md`（AC-1…AC-10 / R-1…R-8）、`design.md`（「变更清单 - 修改文件」5 行 / 「公共函数 · API」4 行 / D1–D9 / 事实校准 14 条 + 待决 4 条）、`tasks.md`（三阶段 + 自查 grep 面）、`specs/desktop-archi-api/spec.md`（ADDED 一条）、`specs/desktop-archi-agent/spec.md`（MODIFIED 两条）
> **性质**: 零新增测试文件、零新增公共导出——全部用例落既有 `api/mod_test.rs` 与 `sdk/mount_test.rs`。本变更只改「一个纯 `String` 拼接函数的返回文本」与「两个 `ToolDefinition.description` 字面串」，故测试面是三件事：**扩写既有 needle / 新增两条「草稿过 `parse`」的自证用例 / 把两处 `contains("### ")` 与一处 `matches("### ")` 改成行首扫描**。
> **路径口径**: 一律写仓库相对全路径（`packages/desktop/src-tauri/crates/**`）。
> **断言面随动清单（design D9 + 本文实测第 4 处）**: design 只登记了 `mod_test.rs` 三处（`bootstrap草稿形态` 扩六段 / 两条新自证用例 / `bootstrap空符号集草稿` 的 `###` 锚）。实测**还有一处必改**：`mod_test.rs:4872` 的 `bootstrap组合闭包草稿名单` 用 `text.matches("### ").count() == 3` 计 op 数，而 `BOOTSTRAP_NOTE` 块内含 `` `### <op-id>` `` 字面 ⇒ 计数变 4、该用例必红。本文按「行首扫描」同口径处置（见 `## 单元测试` 之 `api/mod.rs` 用例表第 8 行），MUST NOT 用「删掉作者注里的 `### <op-id>`」来换绿——那是零收益的让步，说明块的四要点由 spec 的 ADDED requirement 持有。
> **待决 1 的执行口径**: `## 操作` / `## 类型` 各带一行含 CJK 的「（待补：…）」占位正文，故 `sections` 在**有符号 / 无符号**两形态都恰六段；无符号场景句「`## 操作` 段为空」按「零 `### ` 行首锚点、零 `api-op` fence」读。

---

## 验收范围

<!-- 逐条映射 proposal.md 的 10 条 AC。`被测文件或模块` = 生产源文件 + 承载用例的测试文件；
     纯静态面（版本号 / 零外溢 / 生成物 / 既有零改动用例）写「—（见不可测试项 N）」，并给替代保证。 -->

| AC ID | 验收条件 | 被测文件或模块 |
|---|---|---|
| AC-1 | 草稿含 H1 + 六段，段序为 职能 / 不变量 / 操作 / 类型 / 边界 / 承载理由；H1 仍 = 元素 FQN | `packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs`（用例落 `api/mod_test.rs`：六段 needle 扩写 + `sections` 段序断言） |
| AC-2 | 作者注在文件末尾的非 `api-*` fenced 块内，含「api-reexport」与 `{ "导出名": "直接子元素 FQN" }` | `packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs`（用例落 `api/mod_test.rs`：末块语言标 + 块体 needle + 位置下标） |
| AC-3 | 草稿自证可解析：`contract::parse(draft, fqn, "<FQN>.md")` 零错误、`sections.len() == 6`、`reexport_prose` 空、六段 body 零「作者注」 | `packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs`（组合用例，挂本节：`api::bootstrap` → `contract::parse`） |
| AC-4 | 无符号元素草稿仍六段（零行首 `### ` 锚点、`api-types` 为 `{}`），仍过 `parse` | `packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs`（组合用例，挂本节：`api::bootstrap` → `contract::parse`） |
| AC-5 | `archi_api` 描述含三句：bootstrap 指路（针 `第一步`）/ `.schema.json` + `INDEX.toml` + `.i.` 点名 / `enabled = false` 口径（同句含 `findings`） | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs`（用例落 `sdk/mount_test.rs`：`archi_api入参schema形态` needle 扩写 + 同句切分断言） |
| AC-6 | `archi_module` 描述含 `schemaPath` 与 `generatedPath` | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs`（用例落 `sdk/mount_test.rs`：`archi_module描述补三句指路` needle 扩写 + 落位下标） |
| AC-7 | 描述面两条既有边界仍绿：零本仓专用命令名、`archi_module` 零代码围栏 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs`（用例落 `sdk/mount_test.rs`：`工具描述零项目专用命令` / `archi_module描述零字段形态教学` 维持） |
| AC-8 | 知识骨架 / 全文体量锚逐字不变（2596 / 2739 B），`archi-knowledge.md` 零 diff | —（见不可测试项 4：零改动用例面；既有 `archi_knowledge_test.rs` 两条冻结锚 + 零 diff 承接） |
| AC-9 | 零写盘不变：`bootstrap` 调用前后架构域字节相同；工具直出与 `api::bootstrap` 逐字相等 | `packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs`（既有 `bootstrap只出字符串` 快照用例维持）+ `packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool_test.rs`（既有逐字相等断言，见不可测试项 6） |
| AC-10 | 零外溢：DTO / IPC / `bindings` / 四类生成物 / 前端零 diff，`api:check` 与 `client:check` 零差异 | —（见不可测试项 2：静态链与零 diff 门） |

---

## 单元测试

<!-- 框架识别结论（依既有测试文件的扩展名与导入样式）：
     - 运行器 / 语言：Rust（`packages/desktop/src-tauri` 工作区，`cargo test`）；本变更不涉及前端
       `vitest`（前端零消费草稿，design 事实 12）。
     - 用例形态：`#[test]` 函数 + std `assert!` / `assert_eq!` / `matches!`；JSON 断言用
       `serde_json`（`Value` 比较）；零第三方断言库、零 `proptest`。
     - 文件布局：与源文件**同目录共置**的 `*_test.rs`，经 `#[cfg(test)] mod <name>_test;` 登记
       （`crates/core/archi/src/api/mod.rs:1103`、`crates/infra/agent/src/sdk/mod.rs:60`）。
       本节两对映射即该布局本身，**零新增测试文件**。
     - 用例名混用 ASCII 标识符与中文，`crates/core/archi` 与 `crates/infra/agent` 均零
       `#[allow(non_snake_case)]`（CJK 不触发该 lint），新增用例名沿用同风格。

     既有断言面的硬约束（落地前必读，违反即红）：
     ① `mod_test.rs::注记面entry退出源码自证` 以源文本断言 `fn bootstrap_text(fqn: &str, symbols: &[ApiSymbol])`
        与 `pub fn bootstrap(root: &Path, element: &str) -> Result<String, ApiError>` 各恰 1 次、`symbol.file`
        写出点恰 2 处、零 `@ {entry}` / `@ 入口` ⇒ 作者注取模块级 `const` 后这两处字面串与 `symbol.file`
        计数 MUST NOT 变（新增 `const` 的 doc 也 MUST NOT 引入上述三个 forbidden needle）；
     ② 草稿文本面四条否定断言仍须绿：`!text.contains("unused")`（`mod_test.rs:3732`、
        `archi-api-gen/src/main_test.rs:852`）、`!text.contains("used_by")` / `"usedBy"` / `"引用方"`
        （`mod_test.rs:2150`–`2160`）⇒ 作者注只许引入 `api-op` / `api-types` / `api-reexport` / `MUST NOT` /
        `enabled` / `findings` / `archi_api` 等词（design 事实 14，已逐词核对零交集）；
     ③ `mount_test.rs::mount描述enum词面零旧值` 扫**整份 `mount.rs` 源文本**，禁 `"docs"` ⇒ 新增句 MUST NOT
        出现带引号的 `"docs"`；同文件 `mount描述含doc纪律两半` 要求 `单值 doc` / `api = none` /
        `api.contractPath` / `入口文档路径` 四片段仍在；
     ④ `sdk/tools_test.rs::archi工具描述零旧多值词` 的禁词表含 `findings`，但其夹具
        `archi_injections(true, true)`（`tools_test.rs:1422`）只下发 `archi_module` + `archi_search`，
        **不含 `archi_api`** ⇒ 冷启动口径句可写 `findings`，该用例仍绿（design 事实 7，已核对夹具）；
     ⑤ `mod_test.rs` 对 `### ` 的三处旧判据必须迁移：`:5048` / `:5070` 的 `!text.contains("### ")`
        与 `:4872` 的 `matches("### ").count() == 3`（后者 design 未登记，本文新增）。

     范围说明（design「变更清单」为界）：
     - `crates/tools/archi-api-gen/src/main.rs` 在「公共函数 / API」表标「零变化」且不在变更清单 ⇒
       不设独立章节（见不可测试项 3）。
     - `sdk/archi_tool.rs` / `sdk/archi_tool_test.rs` 设计明列零改动（同一份草稿文本）⇒ 既有断言自动
       承接新形态，不新增用例（见不可测试项 6）。
     - `contract.rs` 零改动：本节只把它当**真实被测协作方**调用（`parse`），不为它建章节、不为它新增用例。
     - 跨模块组合用例全部挂链路发起方章节：`api::bootstrap` → `contract::parse` 的用例在 `mod.rs` 章节；
       描述面 → 注入清单（`registry()` / `lookup()`）的用例在 `mount.rs` 章节。不新建集成章节。 -->

### packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/mod_test.rs

<!-- 挂 AC-1 / AC-2 / AC-3 / AC-4 / AC-9（零写盘面）。
     条目取自 design「公共函数 / API」表中「所在文件 = `crates/core/archi/src/api/mod.rs`」的唯一一行
     （`archi::api::bootstrap`）。私有 `bootstrap_text(fqn, symbols)` 与新增 `const BOOTSTRAP_NOTE`
     不在该子表内（design 明写「私有函数（不列）」）⇒ 不作 `#### 待测功能` 条目，只经 `bootstrap`
     公共行为面观测（见不可测试项 5）。 -->

#### 待测功能

- `bootstrap(root, element)`: 返回值文本形态由「H1 + 四段」改为「H1 + 六段（职能 / 不变量 / 操作 / 类型 / 边界 / 承载理由，各带含 CJK 的「（待补：…）」占位正文）+ 末尾语言标非 `api-*` 的 fenced 作者注」；既有四段的 heading 与前缀逐字保留、相对次序不动；签名、错误面（空 FQN / 元素不存在 / 四类入口理由）、`extract::extract_element` 取数面与零写盘逐字不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `bootstrap()` · 六段骨架 | 正向 | 既有 `bootstrap草稿形态` 夹具（`App.Core.Alpha` + `pub struct T;` + `pub fn a(input: &str) -> String`）→ H1 = `# <FQN>`、六段 heading 齐在、`### a` + `api-op` fence 与 `api-types` 键 `T` 仍出；扩写点 = `## 边界` / `## 承载理由` 两枚新 needle 与段序（`text.find("## 类型") < text.find("## 边界") < text.find("## 承载理由")`）——`bootstrap草稿形态` | 既有（扩写 needle） |
| `bootstrap()` · 段序与段数（组合：`api::bootstrap` → `contract::parse`） | 正向 | 同一夹具的草稿直喂 `parse(&draft, fqn, "<FQN>.md")` → 零错误；`sections` 的 heading 序列逐字逐序 == `["职能", "不变量", "操作", "类型", "边界", "承载理由"]`；`ops.len() == 1`、`types` 非空、`reexports` 空（MUST NOT 自动生成 `api-reexport` fence）——`bootstrap草稿过parse段序恰六段` | 单点 |
| `bootstrap()` · 说明块零抽取（组合：`api::bootstrap` → `contract::parse`） | 正向 | 同草稿 → `reexport_prose` 为空集；六段 `body` 逐个不含「作者注」且不含「三类 fence」（fence 状态机对非 `api-*` 块显式 `continue`，说明块结构上进不了 `body`）——`bootstrap作者注不进任何散文段` | 矩阵迭代（6 段） |
| `bootstrap()` · 作者注位置与内容 | 正向 | 草稿末块形态：`trim_end().ends_with("```")`、`trim_end().matches("```text").count() == 1`、`rfind("```text")` 的下标大于 `rfind("## 承载理由")`、块体含「api-reexport」与 `{ "导出名": "直接子元素 FQN" }`、块体含四要点关键词（`api-op` / `api-types` / `api-reexport` / `enabled` 与 `findings` 同段）——`bootstrap作者注在末尾非api块内` | 单点 |
| `bootstrap()` · 占位句形态与取值 | 边界 | 六段 `body.trim()` 逐段等于 design D3 / 数据模型 §1 的原文（`（待补：一句职能）` / `（待补）` / `（待补：每个操作一个三级标题 + 紧随一个 api-op fence）` / `（待补：本模块自有 $defs，一个 api-types fence，同文件唯一）` / `（待补：本模块**不**负责什么）` / `（待补：为什么它是独立元素）`）；逐段首字符非 `-` / `*` / `|`、含 CJK、不含 `### ` 与 `- ` 起手——`bootstrap六段占位句形态` | 矩阵迭代（6 段） |
| `bootstrap()` · 无符号元素仍六段（组合：`api::bootstrap` → `contract::parse`） | 边界 | 零导出入口（`views/agent/src/lib.rs` 只有注释）→ 行首 `### ` 计数 == 0、草稿含 ```json api-types\n{}\n```、`parse` 零错误、`sections.len() == 6`、`ops` 空、`types` 空、`reexport_prose` 空——`bootstrap无符号元素仍六段且过parse` | 单点 |
| `bootstrap()` · 闭包零符号变体（既有用例的判据迁移） | 边界 | 既有 `bootstrap空符号集草稿` 的第二夹具（barrel 目标归属另一元素、全被拒）→ 与上一行同判据；两处 `!text.contains("### ")`（`:5048` / `:5070`）改为行首扫描 `!text.lines().any(\|line\| line.starts_with("### "))`（`parse` 的 op 锚点只有行首才是，作者注块内的 `` `### <op-id>` `` 不是锚点）——`bootstrap空符号集草稿` | 既有（必改） |
| `bootstrap()` · op 计数不被作者注污染（既有用例的判据迁移） | 边界 | 既有 `bootstrap组合闭包草稿名单`（barrel 闭包：3 op + 4 类型）→ `text.matches("### ").count() == 3` 改为 `text.lines().filter(\|line\| line.starts_with("### ")).count() == 3`；同时保留「跨元素条目零命中」（`### Other` / `### AgentInputExtraProps`）与 `api-types` 键集断言——`bootstrap组合闭包草稿名单` | 既有（必改） |
| `bootstrap()` · 两判据分叉的钉子 | 边界 | 有符号草稿 → `text.contains("### ")` 为**真**（命中作者注块内的 `` `### <op-id>` ``），而行首扫描计数 == op 数（1）——把「只有行首 `### ` 才是 op 锚点」钉成可读锚，防止后续把该断言改回 `contains` | 单点 |
| `bootstrap()` · op 数增长下段数恒定 | 边界 | 源文件由用例生成 40 个 `pub fn op_00()` … `pub fn op_39()`（超大符号列表）→ `sections.len()` 仍 == 6（占位段不被 op 段挤掉）、行首 `### ` 计数 == 40、作者注块恰一个（`matches("作者注（本块不参与契约解析）").count() == 1`）、草稿仍过 `parse`——`bootstrap四十op段数恒定` | 单点 |
| `bootstrap()` · 零写盘 | 边界 | 既有 `bootstrap只出字符串`：`bootstrap` 调用前后 tempdir 全量字节快照相等（`fs::` 写调用零新增），并保留无入口 → `Err(ApiError::Element)` 且文案含 FQN——`bootstrap只出字符串` | 既有（维持） |
| `bootstrap()` · 签名自证与错误面 | 异常 | 既有四例逐字维持：`注记面entry退出源码自证`（两处签名各恰 1 次、`symbol.file` 恰 2、零 `@ {entry}` / `@ 入口`）、`bootstrap元素面异常`、`bootstrap无入口文案列候选`、`bootstrap入口四理由文案可判`——改形态 MUST NOT 波及源码自证字面串与错误文案 | 既有（维持） |

> **参数类型 → 边界映射（本节口径）**：`bootstrap(root: &Path, element: &str)` 的 `element` 是 `str`，其边界（`""` 空串 / 超长 / 特殊字符 / 不存在）**不属本变更改动面**，由既有 `bootstrap元素面异常` / `bootstrap无入口文案列候选` 全量承接（上表末行）；变更面实际被驱动的是 `&[ApiSymbol]` 这一 list 入参与其派生的 `api-types` dict——**list** 边界由上表第 6 行（`[]` 空）+ 第 1 行（单元素）+ 第 10 行（40 元素超大）+ 第 7 行（闭包全拒后的 `[]`）承接；**dict** 边界由第 6 行（`{}`）+ 第 1 行（非空 `$defs`）承接；`Optional[T]` / `float` / `bool` / `Enum` 在本函数无入参位（`None` 位由 Rust 的「无 `None` 形态」消解，`&[ApiSymbol]` 用 `[]` 承担）。

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock（文件系统真实） | 真实临时目录：沿用 `mod_test.rs` 自建 `TempDir` RAII（`std` + `AtomicU64`，本 crate 无 dev-dependency）+ `write_model` 落 `.dev-team/architecture/graph/model/01-test.toml`。`elements_of` / `paths::entry_state` / `extract::extract_element` 走真实派生与真实读盘——「取数面即被测契约」的前置，替身会引入第二事实源 | 全部行为用例 |
| 无 mock（`contract::parse` 真实调用） | 不替身解析器：`parse` 是 crate 内纯函数、只吃 `&str`、零 IO ⇒ 无进程边界，且「草稿零错误装载 + `sections` 恰六段 + `reexport_prose` 空」正是被测语义（替身会把 AC-3 / AC-4 变成同义反复） | 第 2 / 3 / 6 / 10 行的组合用例 |
| 零第二类 mock 主体 | `bootstrap` 只读盘、零 spawn、零网络、零全局可变状态、零配置读取 ⇒ 无 DB / 接口 / 子进程 / 运行环境边界需要替身；`std::env::temp_dir` 是唯一环境接触点（既有夹具已隔离进程 id + 自增序号） | 全节 |

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount_test.rs

<!-- 挂 AC-5 / AC-6 / AC-7。
     条目取自 design「公共函数 / API」表中「所在文件」含 `crates/infra/agent/src/sdk/mount.rs` 的两行
     （`archi_api`（工具）/ `archi_module`（工具）——两行的「所在文件」列写明「描述在 mount.rs，
     执行体在 sdk/archi_tool.rs」，本变更只动描述侧，故本节只覆盖描述面；执行面零改动用例见不可测试项 6）。
     同表另两行（`archi-api-gen bootstrap` CLI / `archi::api::bootstrap`）不属本节文件，未纳入。 -->

#### 待测功能

- `archi_api_definition()`（design 行 `archi_api`（工具），改）: 描述文本四处改动——① 首句 `（自检）` → `（自检与草稿）`；② `bootstrap` 指路「新建 / 补契约的第一步就取它」；③ 泛称「生成物」换成四类点名（契约 md 同目录 `<FQN>.schema.json`、架构域 `INDEX.toml`、入口同级 `.i.rs` / `.i.ts`）且保留「过期用既有生成命令重生成，MUST NOT 手改生成物」；④ 冷启动口径「`check` 的 `artifacts.enabled = false` 即契约面未启用（此时 `findings` 恒空，不等于零问题）」。`parameters`（`action` 二值 enum / `element` / `required`）、名字常量、`ToolTier::Read` / `ToolExec::Archi` 归属逐字不变
- `archi_module_definition()`（design 行 `archi_module`（工具），改）: 描述末尾补第四句「`api.schemaPath` / `api.generatedPath` 给另两条生成物路径（`contractPath` 已指手写真相）」，落点在 `archi_scaffold` 一句之后、诊断收尾句之前；其余句子（`hasPath` / `resolvedEntry` / 单值 `doc` 两半 / `modelFile` + `modelLine` 可直接 `edit`）、零代码围栏与零「三档」纪律逐字保留

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `archi_api_definition()` · 三句补位 | 正向 | `lookup(ARCHI_API_TOOL).description` 含 `第一步` / `.schema.json` / `INDEX.toml` / `.i.rs` / `.i.ts` / `enabled` / `findings` 七枚新 needle，且既有 `MUST NOT` / `手改生成物` / `重生成` 三条仍绿（针表由 3 枚扩到 10 枚）——`archi_api入参schema形态` | 既有（扩写 needle） |
| `archi_api_definition()` · 冷启动口径同句 | 正向 | 以 `。` 切句后，含 `enabled` 的那一段同时含 `findings` 与「不等于零问题」（把 AC-5 的「同句」钉成机械判据，不依赖换行宽度；若被拆成两句即红）——`archi_api冷启动口径同句` | 单点 |
| `archi_api_definition()` · 首句语义对齐 | 正向 | 描述含 `自检与草稿`，且 `!description.contains("（自检）")`（旧单义括号形态零残留，与 delta 的 requirement 标题同口径）——`archi_api首句自检与草稿` | 单点 |
| `archi_api_definition()` · 落位与尾句 | 边界 | 下标单调：`find("bootstrap") < find("第一步") < find(".schema.json") < find("enabled")`；且 `trim_end().ends_with("（先修模型）")`（诊断收尾句仍在末位、冷启动句不越位；原描述以「MUST NOT 手改生成物」收尾，改写后尾句须换位）——`archi_api描述落位与尾句` | 单点 |
| `archi_api_definition()` · 参数 schema 零变化 | 边界 | `parameters["type"] == "object"`、`action` 的 enum 逐字 `["check", "bootstrap"]`、`element.type == "string"`、`required == ["action"]`——描述改写 MUST NOT 波及参数面 | 既有（维持） |
| `archi_module_definition()` · 第四句 | 正向 | 描述含 `schemaPath` / `generatedPath` / `contractPath` /「另两条生成物路径」，且既有 `hasPath` / `resolvedEntry` / `archi_scaffold` 三针与「入口解析结果」或「显式命中」仍在——`archi_module描述补三句指路`（用例名沿用既有，不改为「四句」：改名会与 delta「Module Contract」表的既有引用脱钩，收益为零） | 既有（扩写 needle） |
| `archi_module_definition()` · 第四句落位 | 边界 | `find("archi_scaffold") < find("schemaPath") < find("模型写坏时")` 且 `find("schemaPath") > 0`——插在 `archi_scaffold` 一句之后、诊断收尾句之前（放段首或段尾即红）——`archi_module描述第四句落位` | 单点 |
| 描述面禁则 · 零本仓专用命令 | 异常 | 既有 `工具描述零项目专用命令`：遍历 `registry()` 全部 16 项描述，`pnpm` / `cargo run` / `api:export` / `archi-api-gen` 零命中（新增的是文件后缀 `.schema.json` / `INDEX.toml` / `.i.rs` / `.i.ts`，不是命令名） | 既有（维持） |
| 描述面禁则 · `archi_module` 零字段形态教学 | 异常 | 既有 `archi_module描述零字段形态教学`：描述零代码围栏（零三反引号）、零「三档」，且 `modelFile` + `modelLine` + `edit` 仍在——第四句只点名两个字段名，不引入示例围栏 | 既有（维持） |
| 描述面禁则 · 整份 `mount.rs` 旧词面 | 异常 | 既有 `mount描述enum词面零旧值`（`!source.contains("\"docs\"")`、`"elements", "doc", "all"` 仍在）与 `mount描述含doc纪律两半`（`单值 doc` / `api = none` / `api.contractPath` / `入口文档路径`）双绿——新增句 MUST NOT 带出 `"docs"` 旧复数词，也不得冲掉 `contractPath` 的既有交代 | 既有（维持） |
| `archi_api` 工具 · 档位与执行路由零变化 | 边界 | 既有三例维持：`archi_api的档位与执行路由`（`ToolTier::Read` / `ToolExec::Archi`）、`archi_api三档恒放行`、`archi_api非法名与既有查表语义`（未知名 / 尾空格 / 空串 → `lookup` 为 `None`）——本变更只改描述字面串与参数 schema 无关面 | 既有（维持） |

> **命名对齐（落地须一致）**：proposal「测试文件」表与 `specs/desktop-archi-agent/spec.md` 的 Module Contract 称该用例为 `archi_api定义描述与参数形状`，而 `mount_test.rs:1169` 的实际用例名是 `archi_api入参schema形态`（design D9 用的是后者）。本节按**既有实际名**扩写，不改名；落地时若决定改名，须同批更新 delta 的 Module Contract 行，MUST NOT 两处不一致。
>
> **跨文件回归面（不在本节用例表内，只登记挂靠）**：AC-8 的「四条补位零 preamble 字节」由 `crates/infra/agent/src/archi_knowledge_test.rs::骨架体量锚`（2596 B）与 `::全文体量锚`（2739 B）两例在既有断言上兜住——该文件本变更零改动，故不新增用例（见不可测试项 4）。AC-9 的「工具直出与 `api::bootstrap` 逐字相等」同理由 `sdk/archi_tool_test.rs:618` 既有断言兜住（见不可测试项 6）。

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | `lookup()` / `registry()` / `definitions()` 全为进程内纯函数（`OnceLock` 一次性静态表，零 IO / 零 spawn / 零网络）；`ToolDefinition.description` 就是被测字面量，替身会抹掉 needle 判据——与既有 `mount_test.rs` 头部注释的零 mock 纪律同源 | 全节 |
| 词表真实派生（不替身） | `ElementKind::ALL` / `GateTier::ALL` 经 `vocabulary()` 真实拼接进 `archi_module` 描述，MUST NOT 手抄词表字面量（否则第二事实源） | 既有 `archi_module` 描述的 kind / gate 词表（回归面：第四句只加字段名，不替换词表） |
| 零第二类 mock 主体 | 描述函数只做 `String` / `serde_json::json!` 构造，零配置读取、零运行环境依赖 ⇒ 无数据文件 / 配置 / DB / 接口 / 子进程边界 | 全节 |

---

## 不可测试项

| 模块 | 原因 | 替代保证 |
|---|---|---|
| 1 · `packages/desktop/package.json`（`"version"` 0.7.9 → 0.7.10）与 `src-tauri/tauri.conf.json` / `src-tauri/Cargo.toml` 的版本行 | 一行 JSON 字面量，无函数 / 分支 / 输出可断言；版本纪律不是行为面 | 桌面端 `git diff` 核对（`package.json` 恰 +1 patch、`tauri.conf.json` 仍 `"version": "../package.json"`、`Cargo.toml` 的 crate 版本行逐字不变）+ `.github/workflows/desktop-release.yml` 发布流程承接（AGENT.md 版本纪律） |
| 2 · AC-10 的零外溢面（DTO / IPC 命令面 / `src/types/generated/**` / 四类生成物 `.schema.json`·`INDEX.toml`·`.i.rs`·`.i.ts` / 前端产品代码 / `bindings` 零 diff；`api:check` 与 `client:check` 零差异） | 断言形态是「零改动」与「零差异」，没有可执行单元；生成物由生成器产出，本变更零生成步骤（`bootstrap` 不写盘） | 桌面端 `check` 链（`server:check` → `bindings:export` → `api:check` → `client:check`）与 `git status --porcelain` 的 D7 白名单核对；行为面背书 = `api/mod_test.rs::bootstrap只出字符串`（`bootstrap` 调用前后字节快照相等） |
| 3 · `crates/tools/archi-api-gen/src/main.rs`（CLI `bootstrap` 出口） | design「变更清单」未列入、公共 API 表标「零变化」⇒ 不进本变更被测范围，故不建独立 per-file 章节（不是无法测试，而是零改动） | 既有 `main_test.rs` 三例随 `api::bootstrap` 自动承接新形态：`bootstrap只出stdout`（零写盘 + `println!("{text}")`）、`bootstrap不读knip`（`!draft.contains("unused")`——作者注已核对零 `unused` 词）、`bootstrap元素不存在`（错误文案 + 零写盘） |
| 4 · `crates/infra/agent/src/archi-knowledge.md` / `archi_knowledge.rs` / `archi_knowledge_test.rs`（AC-8 的「零字节变化」面） | 本变更不改这三文件 ⇒ 无新增用例；骨架体量锚是既有断言的**维持**，不是新判据 | 既有 `archi_knowledge_test.rs::骨架体量锚`（2596 B，区间 2300–2600、门限 2700 不抬）与 `::全文体量锚`（2739 B，区间 2450–2750、门限 2850 不抬）两例 + 桌面端零 diff 核对 |
| 5 · 私有件 `const BOOTSTRAP_NOTE` 与 `bootstrap_text(fqn, symbols)`（`api/mod.rs`） | 私有不进公共面；为其加 `pub` 或 test-only 导出违反 AGENT.md「no test-only exports」，且会污染 `api` 的逐名出口（`lib.rs`） | 经公共 `bootstrap(root, element)` 行为面全覆盖：六段骨架 / 段序 / 说明块位置与零抽取 / 零符号 / 四十 op / 零写盘；签名冻结另由既有 `注记面entry退出源码自证` 的源文本断言守 |
| 6 · `crates/infra/agent/src/sdk/archi_tool.rs` 与 `sdk/archi_tool_test.rs`（AC-9 的「工具直出与 `api::bootstrap` 逐字相等」） | design D7 与 proposal「不要修改」明列零改动：两条出口都直出 `api::bootstrap` 的同一份文本 ⇒ 既有断言自动覆盖新形态，新增用例只会复制同一判据 | 既有 `archi_api_bootstrap出草稿`（`archi_tool_test.rs:618`）的 `assert_eq!(draft, direct)` 逐字相等断言维持绿——六段 + 作者注形态由它无改动承接 |
