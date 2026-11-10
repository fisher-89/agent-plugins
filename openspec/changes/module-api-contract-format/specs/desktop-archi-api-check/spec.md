# desktop-archi-api-check Specification（delta: module-api-contract-format）

## ADDED Requirements

### Requirement: 契约判据词汇封闭十值与开放分类

契约判据 SHALL 归入**新平行封闭枚举** `ArchiApiFindingKind`（十值，线格式 camelCase），MUST NOT 并入既有 `ArchiDriftKind`（四值）：

| 枚举值 | 线格式词 | 语义 |
|---|---|---|
| `ApiEntryMissing` | `apiEntryMissing` | 元素根路径下无生成物也无实现入口 |
| `ApiMissing` | `apiMissing` | 契约 → 代码缺失：`api = declared` 而契约文件缺失，或契约声明的 op / 类型在代码里不存在。两类缺口 SHALL 用**同一条判据、同一次求值**（存在性判定，签名归 `apiSignatureMismatch`）；共同的排除面三条 = 构造器双向排除 / 透传纯 `$ref` alias 排除 / `x-class` 与 `enum` 计入类型面；结构前提 SHALL 含 `#[path = "..."] mod` 的模块定位与 `pub use <mod>::*` 的通配升格，缺任一条则本判据在自有生成物的元素上恒假阳 |
| `ApiUndeclared` | `apiUndeclared` | 代码有 `external` 面但契约未声明（**唯一完备性判据**） |
| `ApiSignatureMismatch` | `apiSignatureMismatch` | 契约签名与代码签名（归一后）不一致 |
| `ApiUnexpected` | `apiUnexpected` | 留白与其反面矛盾：`api = none` 而契约文件存在 |
| `ApiShadowing` | `apiShadowing` | 父 `$defs` 与子元素 `$defs` 类型名交集非空 |
| `ApiRefMissing` | `apiRefMissing` | 跨模块 `$ref` 目标缺失 / 未 `declared` / 非直接子元素 |
| `ApiNestedCrossCrate` | `apiNestedCrossCrate` | 嵌套元素父子分属不同 crate |
| `ApiReexportDrift` | `apiReexportDrift` | markdown 散文列表与 `api-reexport` fence 键集不等 |
| `ApiConstructorDeclared` | `apiConstructorDeclared` | 契约 `### 操作` 声明了构造器（应由生成器产出） |

- **分立的理由**：`ArchiDriftKind` 是「模型 vs 代码」的漂移，本枚举是「契约 vs 代码」的漂移——事实源不同（relation 图 vs 契约文件）、定级语义不同（元素 `gate` vs 判据自带 `severity()`）、扩展节奏不同。并入会让既有 `match` 臂被迫处理不相关变体，并牵动既有 corpus 金样与前端 `driftMarks`。
- **`ALL` 与 `as_str` 双向对拍**：`ArchiApiFindingKind::ALL: [Self; 10]` SHALL 与 `as_str` 逐值对拍；新增变体未补入 `ALL`（长度断言）即测试红。
- **`category` SHALL 为开放值集字符串**（MUST NOT 设枚举、MUST NOT 设 `ALL`）：用于报告渲染分组；新增分类只改打标处，不动枚举、不动 `ALL`、不动金样词汇对拍。初始取值集 SHALL 为 `entry`（`apiEntryMissing` / `apiUnexpected`）/ `contract`（`apiMissing` / `apiUndeclared`）/ `signature`（`apiSignatureMismatch`）/ `reexport`（`apiReexportDrift` / `apiShadowing` / `apiRefMissing`）/ `nesting`（`apiNestedCrossCrate`）/ `constructor`（`apiConstructorDeclared`）；拼错的防护靠「单点打标 + 契约测试覆盖已知分类」，MUST NOT 靠类型系统。
- **条目形状**：`ArchiApiFinding { kind, category, element: Option<String>, target: Option<String>, message: String, occurrences: Vec<ArchiDriftOccurrence> }`——`element` 对应既有条目的「引入方」语义、`target` 对应「被引入方」语义，`occurrences` SHALL 复用既有 `ArchiDriftOccurrence` 形状（不另立第二份命中点类型）。

#### Scenario: 十值词汇双向对拍

- **WHEN** 遍历 `ArchiApiFindingKind::ALL` 与 `as_str`
- **THEN** 十项一一对应、线格式词唯一且与语料词一致；从 `ALL` 删掉一项或新增变体未补 `ALL` 时测试红

#### Scenario: 与既有漂移词汇互不混用

- **WHEN** 审查 `ArchiApiFindingKind` 与 `ArchiDriftKind` / `ArchiCheckStatus` / `ArchiObservationKind` 的定义与 `match` 使用点
- **THEN** 四组词汇各自单点定义、互不复用；`ArchiDriftKind` 的 `ALL` 仍为四值、`match` 臂零改动

#### Scenario: 新分类不改枚举

- **WHEN** 给 `apiUnexpected` 换一个 `category` 值（如新增 `config` 分类）
- **THEN** 只改打标处，枚举、`ALL`、词汇对拍与金样零改动；渲染层按新分组聚合

### Requirement: 十条判据的定义与求值互斥顺序

检查 SHALL 对每个 **`path` 非空**的元素求值十条判据（容器元素 `path` 恒空即天然无契约；唯一持 barrel 的容器 `App.Frontend.Components.Agent` 有 `path`，故纳入），求值顺序 SHALL 固定且**互斥**：

```
① apiEntryMissing（结构前提：入口 / 生成物是否存在）
   └─ 命中 → 该元素不再求 ② ③ ④（结构上不可判），仅继续求 ⑤
② apiMissing        契约 → 代码（含「declared 但契约文件缺失」与「契约声明的 op / 类型在代码里没有」）
③ apiUndeclared     代码导出集 → 契约声明集（档位 off，见下条 requirement）
④ apiSignatureMismatch  双向（契约签名与代码签名归一后比对）
⑤ apiUnexpected     留白 ↔ 契约文件存在性（与 ① 独立）
嵌套与透传面（独立于上述，逐元素求值）：
⑥ apiShadowing / ⑦ apiRefMissing / ⑧ apiNestedCrossCrate / ⑨ apiReexportDrift / ⑩ apiConstructorDeclared
```

- **互斥的理由**：`apiEntryMissing` 比 `apiMissing` 更基础——**在契约生成之前就失效**。若两条判据对同一元素同时报，报告数无法单调对拍（消一条减一），且反馈会把 executor 引向「去写代码实现一个不存在的模块入口」。
- **`apiMissing` 的声明集 / 导出集口径**：声明集 = 契约 `### 操作` 的 id 集（**构造器双向都不参与比对**——既不在契约声明集，也不计入导出集）；导出集 = 元素的 `external` 代码导出（Rust 经 `pub mod` 链或 `pub use` 白名单可达的 `pub fn` / TS 跨元素被 import 的 `export function`）。签名比对 SHALL 在**归一后**进行（AST 归一单行、参数名与空白不参与）。
- **`apiUndeclared` 只读 `<FQN>.schema.json` 的 `$defs` 键集**，MUST NOT 并读 markdown。安全性由前序门禁保证：`$defs` 键集 = `api-types` ∪ `api-reexport`，而 `apiReexportDrift` 保证 fence 与散文一致、零 diff 门保证 schema 零手改自由度。
- **`apiUnexpected` 单向**：只判「`api = none` 而契约文件存在」（多出来的契约）。反方向（`api = declared` 而契约文件缺失）归 `apiMissing` —— 两条判据 MUST NOT 对同一事实双报，否则报告数无法单调对拍。
- **`apiMissing` 覆盖契约 → 代码的两类缺口**：① 元素 `api = declared` 而其契约文件不存在；② 契约文件在、但其声明的 op / `$defs` 类型在代码导出集里不存在。① 是迁移期主形态（绝大多数元素尚无契约），② 是契约与代码的真实漂移。两类缺口 SHALL 用**同一条判据、同一次求值**（存在性判定，签名归 `apiSignatureMismatch`）；共同的排除面三条 = 构造器双向排除 / 透传纯 `$ref` alias 排除 / `x-class` 与 `enum` 计入类型面；结构前提 SHALL 含 `#[path = "..."] mod` 的模块定位与 `pub use <mod>::*` 的通配升格，缺任一条则本判据在自有生成物的元素上恒假阳。
- **`apiMissing` 天然支持渐进**：契约里只写已确定的 op 即可，未写的 op 归 `apiUndeclared`（关档），不落在 `apiMissing`。
- **`apiConstructorDeclared` 是「契约自身矛盾」的判据**（合法新增判据的范围）：构造器由生成器自动产出，写进契约必然导致 `apiMissing` 误报——试点纸面验证已实测该误报。
- **`apiRefMissing` 的悬空引用在 schema 层由外部校验器编译期捕获**（ajv 对 schema 层的 `$ref` 报 `can't resolve reference`），但 `x-function` 内的 `$ref` **不被解析**（`x-function` 是未知关键字，校验器不下沉）——故函数签名里的跨模块引用只能由本判据与生成器保证。该边界 SHALL 在信心中如实体现：`schema.json` 的定位是「类型层的可校验投影」。
- **零第二份判定**：十条判据 SHALL 只有一处实现（`crates/core/archi/src/api/check.rs`），命令面与门禁面 MUST NOT 各写一份。
- **契约面启用条件（冷启动纪律）**：`<archi_api_dir>` 缺席或其中零份 `*.md` ⇒ 契约面判为**未启用**，`apiFindings` SHALL 为空数组且不产出任何一条契约判据。理由与「无模型即 `skipped`」同款：迁移期仓库与全部既有漂移语料都没有契约资产，若把「`api = declared` 缺契约文件」在此刻全量展开，既有语料的 `apiFindings: []` 与「金样仅追加空槽位」的兼容承诺同时失效。MUST NOT 把该条件做成逃生口：目录一旦存在（本变更落地即存在，试点契约在内），**全部 `path` 非空元素**（本仓 42 个）重新纳入判据，`declared` 缺文件即 `apiMissing` / 无入口即 `apiEntryMissing`——不存在「部分元素豁免」的中间态，也不设白名单或开关。

#### Scenario: entry 优先互斥

- **WHEN** 某元素既无实现入口、`api = declared` 且其契约声明了 2 个 op
- **THEN** 只报一条 `apiEntryMissing`；同元素不出现 `apiMissing` / `apiUndeclared` / `apiSignatureMismatch`（含「契约文件缺失」这一 `apiMissing` 形态）；`apiUnexpected` 仍可独立报

#### Scenario: 契约声明 op 而代码无

- **WHEN** 元素入口存在、契约声明 `parse` 与 `all_conclusions` 两个 op，代码只有 `parse`
- **THEN** 报一条 `apiMissing`，点名 `all_conclusions`；`apiUndeclared`（`off`）不产出条目

#### Scenario: 签名归一后比对

- **WHEN** 契约声明 `parse_cargo_test(input: string) -> SummaryReport`，代码把签名排成多行且参数名不同但类型序列相同
- **THEN** 归一后判定一致、零 `apiSignatureMismatch`；把返回类型改为 `Vec<SummaryReport>` 时报一条 `apiSignatureMismatch` 并给出两侧归一签名

#### Scenario: 留白与缺文件各归一条判据

- **WHEN** 分别构造「`api = none` 但契约文件存在」与「`api = declared` 但契约文件缺失」
- **THEN** 前者报 `apiUnexpected`（文案指明多出的契约文件路径）；后者报 `apiMissing`（文案指明缺失的契约文件路径）；同一元素 MUST NOT 同时出两条

#### Scenario: 构造器声明即契约自身矛盾

- **WHEN** 契约声明 `### new` 而代码的 `new()` 由生成器产出
- **THEN** 报一条 `apiConstructorDeclared` 且**不**同时报 `apiMissing`（构造器不进声明集比对）

### Requirement: 判据级档位与元素 gate 解耦

十条判据的档位 SHALL 由判据自身承载（`ArchiApiFindingKind::severity()`，**判据级档位**），MUST NOT 读元素 `gate`：

| 类别 | 判据 | 判据级档位 |
|---|---|---|
| **一致性判据**（契约说 A、代码是 B） | `apiEntryMissing` / `apiMissing` / `apiSignatureMismatch` / `apiUnexpected` / `apiShadowing` / `apiRefMissing` / `apiNestedCrossCrate` / `apiReexportDrift` / `apiConstructorDeclared` | **`warn`** |
| **完备性判据**（代码有的契约都得有） | `apiUndeclared` | **`off`** |

- **分界线写死**：一致性判据报的是「债」（可枚举、可消除，可即开 `warn` 验证检查器是否工作）；完备性判据报的是「海」（迁移期必然爆炸，必须后置）。`apiUndeclared` 是唯一后者。
- **与元素 `gate` 解耦的理由**：`ArchiDriftKind` 的定级看引入方元素的 `gate`；契约判据若也接 `gate`，收紧契约就要改模型，两套语料同时动、收紧节奏无法独立。故元素 `gate` 全 `off` 与本能力判据全 `warn` SHALL 同时成立——**「统一目录」与「判据全 off」不能同时成立**：相邻 + off 尚有软提醒，统一 + off 是零提醒零校验 → 契约静默腐烂。
- **未来收紧路径**：收紧由 `severity()` 单值改动驱动（一个元素或一类判据一个 change 收紧），MUST NOT 引入运行时开关、MUST NOT 引入配置键。
- **`warn` 报告即迁移 backlog**：`warn` 模式下的报告输出天然成为迁移待办表（`apiEntryMissing` 每一条都是待建入口 / 待改名任务），比手写 todo 更可靠（直接来自代码事实）。每消一条，报告数减一，可单调验证。
- **MUST NOT 为消债而重构**：本能力 MUST NOT 要求建 barrel、改元素根路径去扩展名、挪子→父类型依赖或上移壳元素根路径（存量债务只作检查器的验证语料）。唯一例外是**为让契约有效而暴露已存在的私有逻辑**（改可见性 + 挪进 `impl`，不动实现逻辑）——判据不同：前者是「格式追代码」，后者是「代码追上契约应有内容」。

#### Scenario: 元素 gate 全 off 而判据仍报

- **WHEN** 模型内全部元素 `gate = off`（本仓现状）且存在 28 个无入口元素
- **THEN** 28 条 `apiEntryMissing` 照常产出（判据级 `warn`），与元素 `gate` 无关；把某元素 `gate` 改成 `strict` 后该 28 条数量与内容零变化

#### Scenario: 完备性判据保持 off

- **WHEN** 代码有 `external` 导出而契约未声明
- **THEN** `apiUndeclared` 档位为 `off`，报告不产出该条目（`apiFindings` 内零 `apiUndeclared`）；把档位改为 `warn` 属独立裁决（MUST NOT 在本变更内顺手打开）

#### Scenario: 零运行时开关

- **WHEN** 审查判据档位的来源
- **THEN** 唯一来源是 `ArchiApiFindingKind::severity()`；无配置键、无环境变量、无命令行 flag 可改档位

### Requirement: 报告并列槽位 apiFindings

`ArchiDriftReport` SHALL 新增**并列**字段 `api_findings: Vec<ArchiApiFinding>`（线格式 `apiFindings`），MUST NOT 改造既有 `errors` / `warnings` / `suppressed` / `observations` / `status`：

- **既有字段零改动**：`ArchiDriftReport` 既有 14 字段 SHALL 逐字不变（字段名 / 类型 / 序列化形态 / 语义）；新增字段插在报告末尾。既有 corpus 金样的 `errors` / `warnings` / `suppressed` / `observations` SHALL 逐字不变，金样只需**追加** `"apiFindings": []`。
- **`version` 递增**：`REPORT_VERSION` SHALL 由 1 递增为 2，并留显式兼容说明（消费方按版本分支的语义：`version = 1` 无本字段）。
- **首期不进 `status` / `passed`**：`apiFindings` 的严重度 MUST NOT 影响 `status` 五值与门禁 `passed`——与 `observations`（「永不影响 status」的既有先例）同款。理由：迁移期 28 + 13 条 warn 若进 `status`，门禁立刻变红，与「存量债务不消解」的前提直接冲突。
- **将来收紧**：`apiFindings` 的影响面由判据级 `severity()` 决定（见上条 requirement），门禁侧改造属独立变更，本能力 MUST NOT 预铺。
- **聚合与稳定序**：`apiFindings` SHALL 按 `(kind, element, target)` 稳定排序（同 `kind` 内按元素 FQN 字典序），使报告可逐字 diff；命中点复用 `occurrences` 聚合（同元素对多处命中聚在一条内，不按行铺开）。
- **词汇组扩为四组**：报告自带封闭词汇由三组（`status` / 漂移条目 `kind` / 观察项 `kind`）扩为**四组**（新增契约判据 `ArchiApiFindingKind` 十值）；四组各自单点定义并附 `ALL`，语料对拍断言「新增变体未同步即红」。
- **判定单源**：`archi_findings` 的装配 SHALL 在 `crates/core/archi/src/drift/mod.rs` 的 `check()` 内单点完成（调 `api::check`）；命令面（`archi_drift`）与门禁面（`LocalArchiCheck`）产出 SHALL 逐字段相等（除 `timestamp` / `change` 与落盘路径）。

#### Scenario: 既有字段与金样零改动

- **WHEN** 对既有漂移语料（无任何契约资产）跑检查
- **THEN** `errors` / `warnings` / `suppressed` / `observations` / `status` / `model` / `scan` 与本变更前逐字相同；金样差异仅 `"version": 2` 与新增的 `"apiFindings": []`（两个字段，显式重写并人工确认）

#### Scenario: 首期不影响 status 与 passed

- **WHEN** 报告含 28 条 `apiEntryMissing` 与 13 条 `apiMissing`（全部 `warn` 级），`errors` 为空且无结构错
- **THEN** `status = clean`（或 `warnings` 仅当其 `warnings` 非空）、门禁步 `passed = true`；`apiFindings` 非空但不参与两者推导

#### Scenario: 稳定序与聚合

- **WHEN** 同一元素对的三处签名不一致被三处命中
- **THEN** `apiFindings` 内该元素对恰一条，`occurrences` 含 3 项（各带 `file` / `line`）；整体排序为 `(kind, element, target)` 稳定序，两次运行逐字相同

#### Scenario: 命令面与门禁面同源

- **WHEN** 以同一 root 分别经 `archi_drift` 命令与门禁 runner 触发检查
- **THEN** 两份报告的 `apiFindings` 逐字段相等（含排序与 `category`）

### Requirement: 契约判据的验证语料与报告数基线

契约判据的正确性 SHALL 以**报告数逐项对拍**验证（存量债务即验证语料），并 SHALL 有语料守护：

- **基线表（本仓现状，实测复核；分母 = `path` 非空的 42 个元素）**：`apiEntryMissing` **28**（27 条既有清单 + 本变更新增的 `crates/tools/archi-api-gen`——它只有 `main.rs`；既有清单含壳 `App.Backend.Shell`，其根路径 `src-tauri/src` 按三分支拼成 `src-tauri/src/src/lib.rs` / `src-tauri/src/mod.rs` / `src-tauri/src.rs` 三者皆不存在，故同归 entry-missing）、`apiMissing` **13**（命中入口的 **14** 个 crate 元素 − 1 个试点；试点契约就位后其 `apiMissing` 归零，其余 13 条为「declared 但契约文件缺失」）、`apiSignatureMismatch` / `apiUnexpected` / `apiShadowing` / `apiRefMissing` / `apiNestedCrossCrate` / `apiReexportDrift` / `apiConstructorDeclared` 各 **0**、`apiUndeclared` 不适用（`off`）。
- **判定口径**：**报告数与基线表逐项相等**即「检查能正确提示」。多于基线 = 判据误报（假阳），少于基线 = 判据漏报（假阴），两者都是检查器缺陷——MUST NOT 以「报告少就是代码干净」自我解释。
- **基线须显式留痕**：基线数字写入 change 报告（`reports/`）与实现期实测对照；数字变更（如新增 crate 使 entry-missing +1）SHALL 在报告内说明成因，MUST NOT 直接在代码里改期望数。基线表本身**不写进断言**（易随元素增减漂移），断言只锚定「实现期留痕的实测表与报告逐项相等」。
- **语料守护（分两层，各有 owner）**：① **判定面逐判据用例**由 `crates/core/archi/src/api/**` 的十一份 sidecar 用例承担——契约解析（双向 gate 三条）/ `$defs` 规则五条 / 十判据各自正反例 / entry 互斥 / 签名归一 / 嵌套与透传（选择性 / 纯 `$ref` / 交集 / 直接子元素 / 跨 crate）/ 生成物渲染两侧键集一致；MUST NOT 另建 `tests/corpus/api/` 一类第二套判定语料（既有 crate 级语料只有 `corpus/dsl/` 与 `corpus/drift/` 两套，多一套即第二个驱动面）。② **crate 级报告面语料**沿既有 `crates/core/archi/tests/corpus/drift/` + `tests/golden_test.rs` 的 `DRIFT_CASES` 登记面驱动，快照为 `expected/<case>.<mode>.json`：本变更 SHALL 新增 `api-mismatch` case（`contracts/App/App.Mod.md` + `models/01-app.toml` + `repo/src/mod/mod.rs` 三个 fixture 与其非空 `apiFindings` 金样），并 SHALL 逐份重写既有 15 份金样（`version` 1 → 2 + 追加 `"apiFindings": []`，其余字节不变）；新增 case 未登记进 `DRIFT_CASES` 即红、登记了但 case 目录不存在亦红。golden 更新 SHALL 沿既有 `DESKTOP_GOLDEN_REWRITE=1` 显式重写并人工确认（快照不参与判定）。
- **MUST NOT 把本仓模型与契约塞进 crate 语料**：本仓 `models/**` 与 `api/**` 是仓库数据资产，纳入 crate 测试语料会破坏「语料自足、可移植」；本仓核对以命令 + 留痕执行（落 change `reports/`）。
- **生成器正确性由自身单测守护**：`verify`（零 diff + 编译器）覆盖不到生成器自身的 bug——渲染、装配、抽取三面各有逐字段断言，MUST NOT 只靠端到端零 diff。

#### Scenario: 契约面未启用时零判据

- **WHEN** 观察根下 `<archi_api_dir>` 不存在（或存在但零份 `*.md`）时跑一次全量检查
- **THEN** `apiFindings = []`（零条任何契约判据），`errors` / `warnings` / `status` 与不加契约面时逐字相同；MUST NOT 产 `apiMissing` / `apiEntryMissing`

#### Scenario: 试点契约落地后全量纳入

- **WHEN** 同一根下加入试点契约（`<archi_api_dir>/Infra/App.Backend.Infra.ArchiRuntime.md`）后重跑
- **THEN** 契约面转为启用，全部 `path` 非空元素（42 个）参与判据：产出基线表所述 28 条 `apiEntryMissing` 与 13 条 `apiMissing`（试点元素自身零命中）；试点元素不因「有契约」而被豁免于其它判据

#### Scenario: 报告数与基线逐项相等

- **WHEN** 以本仓根跑一次全量检查并统计 `apiFindings` 按 `kind` 的分布
- **THEN** `apiEntryMissing = 28`、`apiMissing = 13`、其余七条各 0、`apiUndeclared = 0`；`apiEntryMissing` 的逐条 `element` 与既有 27 条清单逐字一致、另加 `App.Backend.Tools.ArchiApiGen` 一条；`apiMissing` 的 13 条逐条为「declared 但契约文件缺失」的元素 FQN

#### Scenario: 假阳与假阴都可辨

- **WHEN** 人为让某入口命中的元素也被判 `apiEntryMissing`（假阳），或让某无入口元素漏报（假阴）
- **THEN** 前者的报告数变为 29（多于基线）后者变为 27（少于基线）；两种偏离都判为检查器缺陷，MUST NOT 更新基线表放行

#### Scenario: 判定面用例与报告面语料各司其职

- **WHEN** 运行 `crates/core/archi/src/api/**` 的 sidecar 用例与 `crates/core/archi/tests/golden_test.rs` 的 `DRIFT_CASES` 驱动面
- **THEN** 前者逐判据覆盖正反例（判据产出由 `check::check` 直驱断言，不依赖快照）；后者逐 case 驱动 `scan → drift::check → 报告`，每 case 的 `apiFindings` 与 `expected/<case>.<mode>.json` 逐字段一致（含 `kind` / `category` / `element` / `target` / `occurrences` 计数）；`api-mismatch` case 产非空 `apiFindings`（`apiSignatureMismatch`），其余既有 15 份金样 `apiFindings` 为空且其余字节不变；MUST NOT 存在第三套判定语料目录

#### Scenario: 新 case 未登记即红

- **WHEN** 新增 `corpus/drift/<case>/` 却未登记进 `DRIFT_CASES`，或登记了而 case 目录不存在，或旧金样未重写而缺 `apiFindings` 键
- **THEN** 三种情形各报红并点名该 case（缺失键 MUST NOT 静默补默认值）；golden 变更只经 `DESKTOP_GOLDEN_REWRITE=1` 显式重写并人工确认

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/drift/report.rs` | 判据词汇与报告槽位 | `ArchiApiFindingKind`（十值 + `ALL` + `as_str` + `severity()`）、`ArchiApiFinding { kind, category, element, target, message, occurrences }`、`ArchiDriftReport.api_findings`、`REPORT_VERSION = 2` |
| `crates/core/archi/src/api/check.rs` | 十条判据单点求值（经 `api::check` 出口暴露，`api/mod.rs` 单行委托） | `check(ApiCheckInput) -> Vec<ArchiApiFinding>`：entry 优先互斥 → 声明集（op **与 `$defs` 类型**）vs 导出集 → 签名归一比对 → 留白矛盾 → 嵌套透传五项；`$defs` 只读 `schema.json`；零第二份判定 |
| `crates/core/archi/src/drift/mod.rs` | 报告装配单点 | `check()` 内调 `api::check` 装配 `api_findings`（与模型面并列）；`apiFindings` 不参与 `status` 推导 |
| `crates/core/archi/src/api/check_test.rs` | 判据语料 | 十判据正反例 / entry 互斥 / 签名归一 / 留白双向 / 交集 / 非直接子元素 / 跨 crate / 透传漂移 / `severity()` 与 `ALL` 逐值对拍 |
| `crates/core/archi/tests/corpus/drift/**` + `tests/golden_test.rs` | 判定面语料与 golden | `corpus/drift/<case>/{contracts,models,repo}` + `expected/<case>.<mode>.json`；`DRIFT_CASES` 登记面即驱动表（未登记 / 目录不存在 / 缺 `apiFindings` 键三种失配皆红）；新增 `api-mismatch` case（非空 `apiFindings`）+ 既有 15 份金样重写；golden 沿 `DESKTOP_GOLDEN_REWRITE=1` 显式重写 |
| `crates/core/archi/tests/vocabulary_test.rs` | 四面词汇对拍扩展 | 新增契约判据十值一面；`ArchiApiFindingKind::ALL` 与 `as_str` 逐值；知识文本须含 `api` 状态词 |
