# 测试设计: archi-doc-api-surface-unification

> **变更**: archi-doc-api-surface-unification（文档内容面 · 散文表格与契约名字清单合一）
> **基于**: `proposal.md`（病灶一 / 二 / 三 + 提案一～六 + AC-1…AC-9 + R-1…R-9）、`design.md`（变更清单「新增文件 / 修改文件」两张表、公共函数 / API 表、D1–D9、事实校准 1–7、待决问题 1–4）、`tasks.md`（阶段一 / 二 / 三 / 四 + 移交段）、`specs/desktop-archi-api/spec.md`（2 MODIFIED requirement + Module Contract）、`specs/desktop-archi-api-check/spec.md`（1 ADDED requirement，零代码改动）
> **范围**: design「新增文件」为空表（零新增文件）；「修改文件」四行——`packages/desktop/src-tauri/crates/core/archi/src/api/contract.rs`（唯一源码面）、仓根 `AGENT.md` + `CLAUDE.md`（纪律文本）、`packages/desktop/package.json`（版本行）。删除文件为零 ⇒ per-file 映射对只有 **1 份源码**（`contract.rs`），但它有两个执行面：同层单测 `api/contract_test.rs` 与 crate 级组合用例 `tests/pilot_test.rs`。生成物、契约资产、静态文本与版本号不进单元测试面（见 `## 不可测试项`）。
> **口径提醒**: ① 本设计只钉「测什么、落在哪个测试文件、mock 到什么程度」，不出测试命令与运行脚本；② 抽取面 `collect_surface_table` 与错误构造器 `surface_table_error` 都是**私有 `fn`**（design D6 / 公共函数表只列模块级导出）⇒ 其可观察面只有 `verify` 的 `Ok` / `Err` 文案，本设计的形态用例一律**经 `verify` 黑盒观测**，MUST NOT 为可测性加测试专用导出（`AGENT.md` 第 5 条）；③ 新用例 MUST NOT 新建 `api/**` sidecar（`tests/golden_test.rs:1482` 断言恰 13 份），演示夹具 MUST NOT 落 `tests/corpus/**`。

## 测试框架与布局

- **框架**：Rust 内建 `#[test]`（工作区以 `cargo nextest` 运行）；零外部断言库、零 mock 库、`core/archi` 无 dev-dependency。断言为 `assert!` / `assert_eq!` / `panic!`，消息用简体中文并携带实际值（既有 `contract_error` / `verify_error` 两个辅助即该形态）。
- **布局**：单元测试是同层兄弟 `*_test.rs`，由 `packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs:1097` 的 `#[cfg(test)] mod contract_test;` 声明；crate 级集成测试在 `packages/desktop/src-tauri/crates/core/archi/tests/**`（`pilot_test.rs` / `golden_test.rs` / `vocabulary_test.rs`）。扩展名 `.rs`。
- **零 mock 传统**：`parse` 接 `source: &str` + `fqn` + `file`，夹具以字符串直传（零落盘）；`verify` 的 `schema` 入参由真实 `build_schema(&contract, &meta())` 产出，仅「缺 `$defs` 键」一类反例以手写 `json!` 注入（既有 `verify的gate_b类型缺定义` 同形）。
- **源码自证**（本仓既有形态）：`include_str!("contract.rs")` 断言某符号 / 字面量在产品源码内恰好 N 处——既有 `verify七族与公开面零回归` / `fence字面量唯一` / `fence零第二常量` 三例即该形态；本变更的「零新增公开出口 / 零第二常量 / 零族编号」判据全部落这一形态。
- **组合用例挂靠**：装载链 `parse → build_schema → verify` 的**整目录**用例（crate 级执行面 `tests/pilot_test.rs`）挂 `api/contract.rs`——它是本变更唯一改动的源码模块、第四闸的所在处，也是该链路在产品代码里的收口点（`api::load_contracts` 零改动，其既有中止语义回归锚在 `src/api/mod_test.rs::load_contracts坏契约即报错`）。按「不新建独立集成测试章节」纪律，该执行面不另立章节，用例写入 `contract.rs` 的第二条 per-file 章节。
- **`pilot_test` 收面口径**（design 测试文件面约束逐条落形）：递归根 = `archi_models_dir(&repository_root())`（`repository_root()` 上溯六层是既有单点）、只认 `*.md`、路径串经 `archi::api::display` 归一（`mod_test.rs` 断言 `display` 是跨 crate 单点，MUST NOT 自写第二份 `replace`）、`fqn` = 文件名去 `.md`；**不钉死份数**，改断言「非留白契约与留白契约各至少一份通过」。

## 验收范围

> 路径相对项目根；单元格内 `->` 左侧为被测源文件、右侧为其测试文件。AC 编号与 `proposal.md`「验收标准」一一对应。

| AC ID | 验收条件 | 被测文件或模块 |
|---|---|---|
| AC-1 | 抽取面形态封闭：只认「表格行 + 首列整体反引号 + ASCII 标识符」；列表项 / 裸正文 / fence 内表格 / 首列无引号 / 首列非标识符 / 表头与分隔行零抽取。反向判据：本仓 `Foundation` / `ArchiRuntime` 的散文列表项（含 16 条留白登记）抽不成名字 | `packages/desktop/src-tauri/crates/core/archi/src/api/contract.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/api/contract_test.rs`（形态封闭 11 组 + 作用域 2 态）；`packages/desktop/src-tauri/crates/core/archi/src/api/contract.rs` -> `packages/desktop/src-tauri/crates/core/archi/tests/pilot_test.rs`（真实语料零误伤） |
| AC-2 | 判据方向钉死为**交集为空**（非等式 / 非镜像）；错误 `ApiError::Contract`，文案含文件 + 交集名字 + 出路句，零行号 | `packages/desktop/src-tauri/crates/core/archi/src/api/contract.rs` -> `.../src/api/contract_test.rs` |
| AC-3 | 留白豁免结构性成立：前置 = `ops` / `types` / `reexports` 任一非空；三项全空 ⇒ 闸不生效。以本仓语料为判据：8 份前端契约（27 条表格数据行）+ 2 份后端契约（0 表格行）全量装载零错误 | `packages/desktop/src-tauri/crates/core/archi/src/api/contract.rs` -> `.../src/api/contract_test.rs`（前置真值表 8 组）；`packages/desktop/src-tauri/crates/core/archi/src/api/contract.rs` -> `.../tests/pilot_test.rs`（10 份全量装载） |
| AC-4 | 退役触发点可演示：留白契约 + 同名表格 ⇒ `Ok`；补一个 `api-types` 键且表格仍列同名 ⇒ 装载期 `Err` 点名该名；删该表格行 ⇒ 通过。演示 MUST NOT 落进 `tests/corpus/**` | `packages/desktop/src-tauri/crates/core/archi/src/api/contract.rs` -> `.../src/api/contract_test.rs`（三态同夹具顺序断言） |
| AC-5 | 判据面与词汇面零扩散：`ArchiApiFindingKind::ALL` 仍 13、七 `category`、三档 `severity()`、`apiUndeclared` 仍 `Off`、纳管口径、`passed` 判据、`REPORT_VERSION = 3` 逐字不变；`api/**` sidecar 仍 13 份；16 份 drift 金样零变化 | **无被测源码改动**（`check.rs` / `drift/report.rs` / `drift/mod.rs` 逐字不动）⇒ 既有断言零改动承接，见 `## 不可测试项` 3；装载期侧「只经 `ApiError::Contract`、不产 finding」由 `.../src/api/contract.rs` -> `.../src/api/contract_test.rs` 的错误面用例间接钉住 |
| AC-6 | 生成物零 diff：`api:export` 后 `.schema.json` / `INDEX.toml` / `.i.rs` / `.i.ts` 逐字节为空；`api:check` 零差异退出 | 无单元测试载体（读盘比对）⇒ 见 `## 不可测试项` 4；行为层等价面由 `.../src/api/contract.rs` -> `.../tests/pilot_test.rs` 的全量装载通过 + 既有 `盘上schema是真实源文的纯派生` 承接 |
| AC-7 | 纪律出处两份：`AGENT.md` 与 `CLAUDE.md` 的 `## 模块 API 契约` 第 ① 条含「散文不复述名字清单 / 名字唯一持有者为 fence」语义；要点数相对 `HEAD` 不变（实测 6 条，design 事实校准 4）；两文件该节正文逐字相同 | 无被测源文件（仓库根 markdown：`AGENT.md` + `CLAUDE.md`）⇒ 见 `## 不可测试项` 1 |
| AC-8 | 留白债在案且不销账：两份 delta spec 写明「前端 8 份表格为临时债、退役由第四条闸机械强制、退役动作归组件契约变更」与「该形态零判据、`apiUndeclared` 保持 `off`」 | 无被测源文件（spec 文本）⇒ 见 `## 不可测试项` 5 |
| AC-9 | 零扩散与版本纪律：改动集 ⊆ 4 个文件；`package.json` 的 `version` 相对 `HEAD` 恰 +1 patch；`tauri.conf.json` 仍 `"version": "../package.json"`；`src-tauri/Cargo.toml` crate 版本逐字不变；`openspec/specs/` 除本 delta 2 个能力外零 diff | 无单元测试载体（`packages/desktop/package.json` 版本行 + 工作树核对）⇒ 见 `## 不可测试项` 2、6 |

## 单元测试

### `packages/desktop/src-tauri/crates/core/archi/src/api/contract.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/api/contract_test.rs`

<!-- design「公共函数 / API」表对该文件只声明一行（`verify`，变化 = 追加第四段闸、签名与错误类型不变）；抽取面 `collect_surface_table` 与错误构造器 `surface_table_error` 是私有 `fn`（D6，表外）。下列用例的测试对象因此一律是 `verify`：抽取面的全部形态判据经 `verify` 的 `Ok` / `Err` 文案黑盒观测（与 `collect_reexport_prose` 经 `parse` 观测同范式），MUST NOT 为此新增公开出口或测试专用导出。 -->

#### 待测功能

- verify(...): 在既有三闸（op/heading ≡ fence 归属、`api-types` ⊆ `$defs`、`api-reexport` ≡ 散文列表）+ 闸一七族 + 落点闸 + 自洽闸之后追加第四条闸——前置 `ops` / `types` / `reexports` 任一非空（留白豁免的唯一来源），断言「散文表格抽取集 ∩（op id ∪ `api-types` 键 ∪ `api-reexport` 键）＝ ∅」，非空即 `ApiError::Contract`（点名文件 + 交集名字 + 出路句、零行号）；既有各项与调用点 `api::load_contracts` 逐字不动

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `verify`（第四闸） | 正向 | 契约非留白（1 op `is_set`）+ 散文零表格行 ⇒ `Ok(())`；同源再加一张首列 `` `unrelated_name` `` 的表格（与契约名字集零交集）⇒ 仍 `Ok`（R-2：参数表 / 对比表 / 枚举值表只要不撞名即合法——闸判的是交集，不是表格有无） | 单例 |
| `verify`（第四闸 · 三类撞名） | 正向 | 三类契约名字各造一处撞名，逐条 `Err(ApiError::Contract)`：① op id（`### is_set` + `api-op` + 表格 `` \| `is_set` \| 判定 \| ``）；② `api-types` 键（`{"LocalArchiCheck": {…}}` + 表格同名）；③ `api-reexport` 键（fence `{"AgentInput": "App.Demo.Input"}` + 匹配的散文列表项 `- AgentInput → App.Demo.Input` 使第三条 gate 先通过 + 表格同名）——每条文案含 `FILE`、交集名、三个 fence 语言标（`api-op` / `api-types` / `api-reexport`）与出路句语义（「名字归 fence」「散文只留语义」） | 表驱动（3 组） |
| `verify`（判据方向 · AC-2 反等式） | 正向 | 契约非留白（1 op + 1 type，名字集 2 项）而散文**零表格行** ⇒ `Ok`——等式 / 镜像语义下「抽取集 0 ≠ 名字集 2」必红，此组即「交集为空 ≠ 集合相等」的行为判据；同一夹具把散文改成首列 `` `is_set` `` 的表格 ⇒ 转 `Err`（两态对照，方向不可反） | 单例（两态） |
| `verify`（AC-4 退役触发点） | 正向 | 留白契约（`api-types` 为 `{}` + `## 公共面` 表格 7 行含 `AgentInput` 等名字）⇒ `Ok`；随后同源补一个 `api-types` 键 `AgentInput`（表格仍列同名）⇒ `Err` 且文案点名 `AgentInput`；再删该表格行（键保留）⇒ `Ok`——三态在同一夹具内顺序断言，源文以字符串直传（MUST NOT 落 `tests/corpus/**`） | 单例（三态） |
| `verify`（抽取面形态封闭 · 零抽取六类） | 异常 | 以「契约已声明 op `is_set`」为背景，六类形态都不得抽出 `is_set` ⇒ 全 `Ok`：① 列表项 `` - `is_set`：判定 ``（归既有 `collect_reexport_prose`）；② 裸正文 `is_set 是判定函数`；③ 围栏代码块（语言标 `text`）内的表格行（fence 内容不入 `section.body`）；④ 首列不带反引号 `\| is_set \| 判定 \|`；⑤ 首列整体反引号但内容非 ASCII 标识符（`` `is set` `` / `` `is-set` `` / `` `9set` `` / `` `名字` `` / 一对反引号内部零字符）；⑥ 表头 `\| 名字 \| 形态 \|` 与分隔行 `\|---\|---\|` 同时在场 | 表驱动（11 组） |
| `verify`（抽取面作用域 = 全部 `##` 段） | 异常 | 同名表格行放在 `### is_set` 之后（`api-op` fence 之后）⇒ 该行不属于任何 `##` 段正文（`parse` 在 `###` 处 `finish_section`，事实校准 5）⇒ 零抽取 ⇒ `Ok`；同源把该行移到任一 `##` 段正文 ⇒ 立刻 `Err`（作用域对照，证明前后两态只差段落位置） | 单例（两态） |
| `verify`（文案点名面） | 异常 | 表格一次列出两个交集名（`is_set` 与 `LocalArchiCheck`，两处都在契约名字集）⇒ 文案按抽取序以 `" / "` 连接两者；表格同时列一个非契约名（`unrelated_name`）⇒ 文案零该名（只有交集入文案，抽取集本身不外泄全量） | 单例 |
| `verify`（错误面与行号承诺） | 异常 | 上述各 `Err` 一律 `ApiError::Contract(..)`（`verify_error` 辅助断言形态，MUST NOT 落到 `ApiError::Io` / `ApiError::Element`）；文案以 `format!("{FILE}: ")` 起手而非 `{FILE}:<数字>: `（段落级判据零行号，与 `api-types` 缺键 / `api-reexport` 差集两闸同档，design 事实校准 7） | 单例 |
| `verify`（前置真值表 · D3 三项同权） | 边界 | `ops` / `types` / `reexports` 三位全枚举（8 组）：全空（含 `api-types` 的 `{}` 空对象 fence + 零 op + 零 reexport）⇒ 闸不生效 ⇒ `Ok`；其余 7 组任一非空 ⇒ 闸生效 ⇒ 同名表格行各 `Err`——含「仅 `reexports` 非空」一组（只写透传的契约同样持有名字，`reexports` 与另两项同权，MUST NOT 被漏判） | 表驱动（8 组） |
| `verify`（抽取面的行形态边界） | 边界 | 前导空格 / tab 缩进后以 `\|` 起手 ⇒ 抽取；单元格两侧多余空白 `` \|   `is_set`   \| `` ⇒ 抽取（`trim` 后判反引号包裹）；无尾随 `\|` 的 `` \| `is_set` `` ⇒ 抽取；首列为空 `\|\| x \|` ⇒ 零抽取；三列 / 四列表格 ⇒ 只看首列（4 列形态对应前端顶层契约多一列「来源文件」）；首列 `` `a` `b` ``（不止一对反引号）⇒ 零抽取 | 表驱动（7 组） |
| `verify`（抽取集去重保序） | 边界 | 同一名字在两个 `##` 段各出现一次 ⇒ 文案只出现一次（不以 `is_set / is_set` 形态重复）；两个不同交集名跨段声明（第一段 `is_set`、第二段 `LocalArchiCheck`）⇒ 文案按首次出现序 `is_set / LocalArchiCheck` | 表驱动（2 组） |
| `verify`（抽取面的值边界） | 边界 | 首列超长 ASCII 标识符（> 1000 字符、不在契约名字集）⇒ 抽取但交集空 ⇒ `Ok`；首列含 emoji / `\n` / `\0` ⇒ 非 ASCII 标识符 ⇒ 零抽取 ⇒ `Ok`；零 `##` 段的契约（只有 H1 + fence，`sections` 为空）⇒ 零抽取 ⇒ `Ok` | 表驱动（3 组） |
| `verify`（报错顺序零变化） | 边界 | 同一夹具同时违反「`api-types` 键在 `$defs` 缺失」与「表格复述该名」⇒ 报**第二条 gate** 的缺键（第四闸在 `verify` 末尾，既有报错顺序与优先级零变化，design 事实校准 2）——此组以手写 `json!({"$defs": {}})` 作 `schema` 入参 | 单例 |
| 结构锚（源码自证 `include_str!("contract.rs")`） | 边界 | `\npub fn ` 仍恰 3 处（`parse` / `verify` / `is_constructor`；design 变更清单指名的两个私有 `fn` 不进公开面）；`fn collect_surface_table(` / `fn surface_table_error(` 各恰 1 处；`pub(crate) const FENCE_` 仍恰 2 处（第四闸复用既有常量，MUST NOT 新增 fence 常量）；`"api-op"` / `"api-types"` 引号字面量仍恰 2 处（只落两常量定义行，闸内走常量插值）；`⑧` 零命中（第四闸不占七族编号）；模块头含「四条装载期闸」而「三条双向 gate」「四者同函数」零残留 | 单例（计数表驱动） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（纯字符串 + 纯函数） | 零 mock：契约源文以 `String` 直传 `parse`（`FILE` / `FQN` 常量），`schema` 由真实 `build_schema(&contract, &meta())` 产出 | 全部用例 |
| `schema` 的 `$defs`（既有反例注入形态） | 不替身函数，仅以手写 `json!({"$defs": {}})` 作 `verify` 入参（既有 `verify的gate_b类型缺定义` 同形） | 报错顺序用例（缺键先于第四闸） |
| 产品源码文本 | 真实 `include_str!("contract.rs")`（既有源码自证形态） | 结构锚用例 |
| 文件系统 / 网络 / 子进程 / 全局变量 / 运行环境 | 不适用（本文件零 IO、零环境依赖；夹具 MUST NOT 落盘 ⇒ 不新建 `api/**` sidecar、不写 `tests/corpus/**`） | 全部用例 |

### `packages/desktop/src-tauri/crates/core/archi/src/api/contract.rs` -> `packages/desktop/src-tauri/crates/core/archi/tests/pilot_test.rs`

<!-- 组合用例挂靠：被测链路 = `parse` → `build_schema` → `verify` 在**在盘真实语料**上的全线通过。挂靠入口模块取 `api/contract.rs`（本变更唯一改动源码、第四闸所在处、装载链在产品侧经 `api::load_contracts` 的收口点），按「不新建独立集成测试章节」纪律写入本 per-file 章节。`待测功能` 仍取自 design 公共函数表同一行（`verify`），不虚构条目。 -->

#### 待测功能

- verify(...)（组合链路 `parse → build_schema → verify`）: `<archi_models_dir>` 递归全部 `*.md`（当前 10 份）逐份全线通过——把「本仓语料零误伤」从人读结论变成机械面：后端 2 份（闸生效、散文零表格行）与前端 8 份（三项全空走留白豁免、27 条表格数据行）都不得报错

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `verify`（组合链路） | 正向 | `archi_models_dir(&repository_root())` 递归全部 `*.md`（当前 10 份），逐份以 `archi::api::display` 归一路径串、以文件名去 `.md` 作 `fqn`，走 `parse` → `build_schema` → `verify` ⇒ 全 `Ok(())`；**不钉死份数**，改断言「至少一份非留白契约（闸生效面）与至少一份留白契约（豁免面）各有一份通过」——目录漂移时用例不空转 | 循环迭代（目录全部 md） |
| `verify`（组合链路 · 后端两份） | 正向 | `App.Backend.Core.Foundation` / `App.Backend.Infra.ArchiRuntime` 落在上一轮的通过集内，且各自散文零表格行 ⇒ 通过（AC-1 反向判据：其散文列表项含 16 条留白 op 名，零抽取、不误伤） | 表驱动（2 组） |
| `verify`（组合链路 · 前端八份） | 正向 | 8 份前端契约（`api-types` 全 `{}`、零 `api-op`、零 `api-reexport`、27 条表格数据行）落在通过集内 ⇒ 通过（AC-3 留白豁免的行为证据：表格仍在盘而装载零错误，MUST NOT 把它们读成回归） | 循环迭代（8 组） |
| `verify`（组合链路 · 失败可见） | 异常 | 任一份失败即 `panic!` 且消息含该文件的 repo 相对路径串与错误文案（失败可定位）；MUST NOT 在循环内 `continue` 静默跳过失败项（否则「零误伤」不可判） | 单例 |
| `verify`（组合链路 · 收面口径） | 边界 | 收面恰为 `<archi_models_dir>/**/*.md`：`graph/` 是兄弟目录（`graph/model/*.toml` 天然不在收面）、不递归第二根、不按文件名白名单豁免——契约目录里出现非契约 `*.md` 即装载失败（design 待决问题 3 显式接受的活体报警，MUST NOT 在用例里加豁免） | 单例 |
| `verify`（组合链路 · 夹具单点复用） | 边界 | 与同文件既有 `旧契约目录已退役` / `试点三件资产按单点派生落点在盘` 等用例共存且互不遮蔽：复用既有 `repository_root()` / `api_dir()` / `read_repo()` 三个夹具，MUST NOT 另写第二份上溯层数或第二份路径串 `replace`（`display` 是跨 crate 单点） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 真实磁盘（仓库内契约资产） | **不作为 mock 对象**：`archi_models_dir(&repository_root())` + `fs::read_to_string` 原文读取，只读零写盘；替身会让「本仓语料零误伤」不可见 | 全部用例 |
| `parse` / `build_schema` / `verify` | 零替身：真实链路，路径串经 `archi::api::display` 归一 | 全部用例 |
| 目录遍历 | 真实 `std::fs::read_dir` 递归，不注入文件清单（份数漂移由「两档各至少一份」断言兜住） | 全部用例 |
| 网络 / 子进程 / 全局变量 / 运行环境 | 不适用（唯一进程边界是只读文件系统，且它是被测事实本身） | 全部用例 |

## 不可测试项

| 对象 | 原因 | 承接方 |
|---|---|---|
| 1. `AGENT.md` + `CLAUDE.md` 的 `## 模块 API 契约` 第 ① 条纪律句（AC-7） | 仓库根 markdown，crate 内零内容消费面（`preamble::load` 只逐字注入、不断言正文）；design「测试文件面」只授权 `contract_test.rs` / `pilot_test.rs`，且 `pilot_test` 的收面口径是 `<archi_models_dir>/**/*.md`（不含仓根文档）⇒ 在 crate 里加断言会越出 design 测试面，并给文档措辞加测试锁（后续修订连带判红）。另有口径分歧：spec 写「恰 5 条要点」，实盘 6 条（`doc` 条在案），落地口径是「相对 `HEAD` 不变」（design 事实校准 4 / 待决问题 1），不宜写成绝对数字断言 | desktop static-check 的阶段二自查：`diff AGENT.md CLAUDE.md` 仍恰 2 处且全在节外、该节正文两文件逐字相同、节内 `grep 'ApiError::Contract'` 零命中、节内 `grep 'openspec/specs/'` 零命中、节内零 fenced json、要点数相对 `HEAD` 不变（当前 6 条） |
| 2. `packages/desktop/package.json` 的 `version`（`0.7.0` → `0.7.1`）+ `src-tauri/tauri.conf.json` / `src-tauri/Cargo.toml` 版本行（AC-9） | 纯配置行，无运行时行为可断言 | 版本纪律 + release workflow（`.github/workflows/desktop-release.yml`）；阶段四 `git diff -U0 packages/desktop/package.json`（只 `version` 一行变化）与 `tauri.conf.json` 跟随 / crate 版本不变核对 |
| 3. AC-5 判据面与词汇面零扩散（十三值 / 七 `category` / 三档 `severity()` / `apiUndeclared` 仍 `Off` / `REPORT_VERSION = 3` / `api/**` sidecar 13 份 / 16 份 drift 金样） | design 明令零源码改动（`check.rs` / `drift/report.rs` / `drift/mod.rs` 逐字不动、零新增 finding kind）⇒ 无被测代码；新断言只会与既有断言重复，且金样逐字节面是读盘比对而非单测对象 | 既有断言零改动自动守：`src/api/check_test.rs:1285` / `src/drift/report_test.rs:587` 的 `ALL.len() == 13`、`src/drift/report.rs:251` 的 `ApiUndeclared => Off`、`src/drift/mod_test.rs:1024` 的 `version == 3`、`tests/golden_test.rs:1482` 的 sidecar 恰 13 份；另有 desktop static-check |
| 4. AC-6 生成物零 diff（`.schema.json` / `INDEX.toml` / `.i.rs` / `.i.ts`）与 `api:check` 零差异退出 | 生成物零 diff 由 `api:export` + `git diff` 读盘比对判定，属机械静态步骤；「闸不参与渲染」的行为层等价面已由 `pilot_test` 全量装载通过与既有 `盘上schema是真实源文的纯派生`（真实源文重算逐字节相等）承接 | desktop static-check 四段链（`server:check` → `bindings:export` → `api:check` → `client:check`） |
| 5. AC-8 留白债在案（两份 delta spec 的留白条 / 零判据条） | spec 文本内容，无运行时行为；判据是「文本里写明了什么、MUST NOT 出现既成事实句」 | 桌面端 spec 校验与评审（`openspec validate` 一类机械步骤）；`openspec/specs/` 的 diff 集核对见第 6 项 |
| 6. AC-9 零扩散（改动集 ⊆ 4 个文件、`openspec/specs/` 除 2 个能力外零 diff）与 `.dev-team/architecture/models/**` 零改动（10 份 md + 10 份 `.schema.json`） | 工作树 / 资产内容核对，无代码可断言；契约资产的具体行数（前端 27 条表格数据行）属读数，按 `desktop-archi-api-check` 纪律**基线读数不进断言**（会随元素增减漂移） | 阶段四 `git status --porcelain` / `git diff --stat .dev-team/architecture/models/ .dev-team/architecture/graph/` 自查 + desktop static-check；「表格仍在盘却装载零错误」正是 `pilot_test` 全量装载的豁免行为证据（不以行数断言表达） |
| 7. 抽取面的收敛预案（段名白名单 / `parse` 期记录 `(name, line)` / 等式语义） | design D9 显式不做（MUST NOT 预留段名白名单或豁免开关；带行号要给 `Contract` 加字段并随动 `PartialEq` 断言）；无实现即无被测代码 | 真实误伤出现时另起变更裁决；本设计仅以形态封闭用例钉住现形态（含「参数表 / 对比表不撞名即合法」与「零行号文案」两组） |
