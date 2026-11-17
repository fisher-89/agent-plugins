# desktop-archi-api-check Specification（delta: frontend-component-element-coverage）

## Purpose

本 delta 只改**报告数基线与其出处说明**，判据种类（十值封闭）、求值互斥序、档位与纳管口径、语料守护两层**逐字不动**：

1. **分母扩容**：`path` 非空元素由 42 → **44**（`06` 新补两个文件型根元素），故全部 `path` 非空元素参与判据。
2. **② 桶 +2**：两个新元素「有入口（文件型根由约定默认解析）、无契约文件」⇒ 各一条 `apiMissing`，② 由 27 → **29**；① 不变（12 条仍是 8 视图目录根 + 3 视图内子域 + `Components.Ui`）。基线由 `12 / 27（总 39）` 换为 **`12 / 29（总 41）`**。
3. **四段读数串讲**：`28 + 13` → `15 + 26` → `12 + 27` → `12 + 29` 是同一分桶规则的四次读数；旧数 MUST NOT 被抹成单一现值，也 MUST NOT 被读成「债务增加」（本段只增加元素面，没有推翻任何既有结论）。

`status` / `passed` 不受影响（纳管子集仍只有 `App.Backend.Infra.ArchiRuntime` 一个契约在案元素且判据 0 条 ⇒ `status = clean`、`passed = true`）。

## MODIFIED Requirements

### Requirement: 报告并列槽位 apiFindings

`ArchiDriftReport` SHALL 新增**并列**字段 `api_findings: Vec<ArchiApiFinding>`（线格式 `apiFindings`），MUST NOT 改造既有 `errors` / `warnings` / `suppressed` / `observations` / `status`：

- **既有字段零改动**：`ArchiDriftReport` 既有 14 字段 SHALL 逐字不变（字段名 / 类型 / 序列化形态 / 语义）；新增字段插在报告末尾。既有 corpus 金样的 `errors` / `warnings` / `suppressed` / `observations` SHALL 逐字不变，金样只需**追加** `"apiFindings": []`。
- **`version` 递增**：`REPORT_VERSION` SHALL 由 2 递增为 3，并留显式兼容说明（消费方按版本分支的语义：`version = 1` 无本字段；`version = 2` 有本字段但 `status` 推导**不**读契约判据）。
- **契约在案即纳管 `status`**：`apiFindings` 对 `status` 的影响 SHALL 限于**纳管子集**——只取「`element` 属于**契约文件在案元素集合**」的条目参与推导（该集合 = 契约 md 存在且可读的 `api = declared` 元素；判据与 `ArchiApiRef` 的 `declared` 判定同源）。纳管子集非空 ⇒ `status` 至少为 `warnings`；纳管子集为空 ⇒ `status` 与不引入本字段时逐字相同。
  - **口径理由（契约即承诺）**：没写契约的元素，其 `apiEntryMissing` / `apiMissing` 是迁移 backlog（既有 requirement 明写「MUST NOT 为消债而重构」）；写了契约的元素，判据就是「自己刚承诺的东西没兑现」——只有后者该进状态词。该口径天然自激活：契约逐份落地，纳管面自动扩大，**MUST NOT 引入白名单、开关或配置键**。
  - **判据单点**：纳管子集的筛选 SHALL 在 `crates/core/archi` 的单点函数内完成（「契约文件在案」的 `is_file` 判定在该函数内一次性发生），`drift` 侧 MUST NOT 自行判文件存在性、MUST NOT 在前端或命令层二次筛选。
  - **`element` 为空的条目不纳管**：无归属元素的判据不参与 `status`（纳管以「某元素自己承诺」为前提）。
- **`passed` 仍不受影响**：`apiFindings` MUST NOT 影响门禁布尔 `passed`——`passed` 仍只看 `errors` 与 `model.structuralErrors`（与 `observations` 的既有先例同款）。理由：判据级 `severity()` 当前最高只到 `warn`，且「契约面一启用即红门」与「存量债务不消解」的前提直接冲突。
- **将来收紧**：`apiFindings` 的影响面由判据级 `severity()` 决定（见上条 requirement）；把某类判据改 `error` 档与改 `passed` 判据 SHALL 同批进行，属独立变更，本能力 MUST NOT 预铺。
- **聚合与稳定序**：`apiFindings` SHALL 按 `(kind, element, target)` 稳定排序（同 `kind` 内按元素 FQN 字典序），使报告可逐字 diff；命中点复用 `occurrences` 聚合（同元素对多处命中聚在一条内，不按行铺开）。
- **词汇组扩为四组**：报告自带封闭词汇由三组（`status` / 漂移条目 `kind` / 观察项 `kind`）扩为**四组**（新增契约判据 `ArchiApiFindingKind` 十值）；四组各自单点定义并附 `ALL`，语料对拍断言「新增变体未同步即红」。
- **判定单源**：`archi_findings` 的装配 SHALL 在 `crates/core/archi/src/drift/mod.rs` 的 `check()` 内单点完成（调 `api::check`）；命令面（`archi_drift`）与门禁面（`LocalArchiCheck`）产出 SHALL 逐字段相等（除 `timestamp` / `change` 与落盘路径）。
- **呈现面**：`apiFindings` SHALL 经架构页左栏「契约」视图与元素抽屉「契约」段呈现（`desktop-archi-contract-panel`），两处 MUST NOT 重算判据、MUST NOT 对同一报告二次 invoke。

#### Scenario: 既有字段与金样零改动

- **WHEN** 对既有漂移语料（契约面未启用、或契约在案元素零判据）跑检查
- **THEN** `errors` / `warnings` / `suppressed` / `observations` / `model` / `scan` 与本变更前逐字相同、`status` 不变；金样差异仅 `"version": 3`（16 份逐份重写并人工确认）。唯一例外是**契约在案且判据非空**的语料 `api-mismatch`：其 `status` 由 `clean` 变 `warnings`（纳管口径的直接结果，MUST 在金样与语料注释两处留痕）

#### Scenario: 契约在案判据进 status 而 passed 不动

- **WHEN** 报告含 1 条 `apiSignatureMismatch`（该元素契约文件在案）、`errors` 为空且无结构错
- **THEN** `status = warnings`，门禁步 `passed = true`；同报告若把该元素的契约文件移走（契约不在案）⇒ 该条判据落选纳管子集、`status` 回落到 `clean`

#### Scenario: 存量 backlog 不拉黄灯

- **WHEN** 报告含 12 条 `apiEntryMissing` 与 29 条 `apiMissing`（全部来自**契约不在案**的元素）
- **THEN** 纳管子集为空、`status` 与引入 `apiFindings` 前逐字相同（本仓现状：唯一契约在案元素判据 0 条 ⇒ `status` 不变）；MUST NOT 因这 41 条把状态词常规化为 `warnings`（壳从此组落在 ② 桶里，仍是 backlog 而非承诺未兑现）

#### Scenario: 稳定序与聚合

- **WHEN** 同一元素对的三处签名不一致被三处命中
- **THEN** `apiFindings` 内该元素对恰一条，`occurrences` 含 3 项（各带 `file` / `line`）；整体排序为 `(kind, element, target)` 稳定序，两次运行逐字相同

#### Scenario: 命令面与门禁面同源

- **WHEN** 以同一 root 分别经 `archi_drift` 命令与门禁 runner 触发检查
- **THEN** 两份报告的 `apiFindings` 与 `status` 逐字段相等（含排序、`category` 与纳管结论）

### Requirement: 契约判据的验证语料与报告数基线

契约判据的正确性 SHALL 以**报告数逐项对拍**验证（存量债务即验证语料），并 SHALL 有语料守护：

- **基线表（本仓现状，实测复核；分母 = `path` 非空的 44 个元素）**：`apiEntryMissing` **12**（8 个视图目录根 + 3 个视图内子域 + `components/ui`——它们缺 `index.ts` / `index.tsx`，全为 `Absent`）、`apiMissing` **29**（14 个 crate 元素 − 1 个试点 = 13，+ 多文件根 `App.Frontend.Shell` + **14** 个前端单文件根元素 = 28，**+ 壳 `App.Backend.Shell`**——显式 `entry` 命中后 ① 不再成立，`declared` 但契约文件缺失使 ② 成立）、其余七条一致性判据各 **0**、`apiUndeclared` 不适用（`off`）——两者即字面的 **12 / 29**、总 **41**。
- **两条切分口径**：① `apiEntryMissing` 只由「无可用入口」产生——`Absent`（未声明且无约定默认）与 `Invalid`（形态 / 落位 / 存在 / 唯一任一违反）同归此项；② 入口一旦可解析，该元素就不再享有 `apiEntryMissing` 的静默，改为承担「有门无契约」的 `apiMissing`（`api = none` 的元素除外：无库 API ⇒ ② 不适用，故 ① 退场即零判据）。故**总条目数 12 + 29 = 41** 全由这两条口径导出，且与三次历史读数连成同一规则的四段：`28 + 13 = 41`（A 之前，入口靠探测）→ `15 + 26 = 41`（A 落地，13 个前端文件型根由「猜不到入口」转为「有门无契约」，壳与两工具 crate 仍在 ① 面）→ `12 + 27 = 39`（A 的数据面换段：壳由 ① 平移至 ②，净 0；两工具 crate 各退出 ①，净 −2）→ `12 + 29 = 41`（本段：`SessionPanel` / `Tooltip` 两个共享组件文件补建元素后，「有入口、无契约」使 ② +2，① 仍 12）。四段 MUST NOT 被读成「债务增减」——它是**分桶**与**元素面扩容**的移动，其中只有 A 的数据面那一步真正收了两条债。
- **基线单段（数据不分批）**：三处显式 `entry`（壳 + 两个工具 crate）SHALL 随模型数据在案（`repo-archi-model`「基线落位与模型文件划分」），故基线只有一段 **12 / 29（总 41）**；`12 / 27（总 39）` 是补建两个共享组件元素**之前**的读数，旧「数据分两批、基线分两段（本段 15 / 26，待换段 12 / 27）」的口径已退场——MUST NOT 再按 15 / 26 或 12 / 27 验收，MUST NOT 保留「后续变更再换段」的例外，也 MUST NOT 把该例外改写为「按第一段验收」的残留。
- **判定口径**：**报告数与基线表逐项相等**即「检查能正确提示」。多于基线 = 判据误报（假阳），少于基线 = 判据漏报（假阴），两者都是检查器缺陷——MUST NOT 以「报告少就是代码干净」自我解释。
- **基线须显式留痕**：基线数字写入 change 报告（`reports/`）与实现期实测对照；数字变更（如新增 crate 使 entry-missing +1、前端补 barrel 使 `apiEntryMissing` 归零、补三处显式 `entry` 使壳换桶且两工具 crate 退出、或补建共享组件元素使 `apiMissing` +2）SHALL 在报告内说明成因，MUST NOT 直接在代码里改期望数。基线表本身**不写进断言**（易随元素增减漂移），断言只锚定「实现期留痕的实测表与报告逐项相等」。
- **语料守护（分两层，各有 owner）**：① **判定面逐判据用例**由 `crates/core/archi/src/api/**` 的十一份 sidecar 用例承担——契约解析（双向 gate 三条）/ `$defs` 规则五条 / 十判据各自正反例 / entry 互斥（含入口四理由）/ 签名归一 / 嵌套与透传（选择性 / 纯 `$ref` / 交集 / 直接子元素 / 跨 crate）/ 生成物渲染两侧键集一致；MUST NOT 另建 `tests/corpus/api/` 一类第二套判定语料（既有 crate 级语料只有 `corpus/dsl/` 与 `corpus/drift/` 两套，多一套即第二个驱动面）。② **crate 级报告面语料**沿既有 `crates/core/archi/tests/corpus/drift/` + `tests/golden_test.rs` 的 `DRIFT_CASES` 登记面驱动，快照为 `expected/<case>.<mode>.json`：既有 `api-mismatch` case（`contracts/App/App.Mod.md` + `models/01-app.toml` + `repo/src/mod/mod.rs`）SHALL 保持非空 `apiFindings`（其目录型根的 `src/mod/mod.rs` 由约定默认 `<root>/mod.rs` 命中，故 `modelHash` 与金样逐字不变）；新增 case 未登记进 `DRIFT_CASES` 即红、登记了但 case 目录不存在亦红。golden 更新 SHALL 沿既有 `DESKTOP_GOLDEN_REWRITE=1` 显式重写并人工确认（快照不参与判定）。
- **MUST NOT 把本仓模型与契约塞进 crate 语料**：本仓 `models/**` 与 `api/**` 是仓库数据资产，纳入 crate 测试语料会破坏「语料自足、可移植」；本仓核对以命令 + 留痕执行（落 change `reports/`）。
- **生成器正确性由自身单测守护**：`verify`（零 diff + 编译器）覆盖不到生成器自身的 bug——渲染、装配、抽取三面各有逐字段断言，MUST NOT 只靠端到端零 diff。

#### Scenario: 契约面未启用时零判据

- **WHEN** 观察根下 `<archi_api_dir>` 不存在（或存在但零份 `*.md`）时跑一次全量检查
- **THEN** `apiFindings = []`（零条任何契约判据），`errors` / `warnings` / `status` 与不加契约面时逐字相同；MUST NOT 产 `apiMissing` / `apiEntryMissing`

#### Scenario: 试点契约落地后全量纳入

- **WHEN** 同一根下加入试点契约（`<archi_api_dir>/Infra/App.Backend.Infra.ArchiRuntime.md`）后重跑
- **THEN** 契约面转为启用，全部 `path` 非空元素（44 个）参与判据：产出基线表所述 12 条 `apiEntryMissing` 与 29 条 `apiMissing`（试点元素自身零命中）；试点元素不因「有契约」而被豁免于其它判据

#### Scenario: 报告数与基线逐项相等

- **WHEN** 以本仓根跑一次全量检查并统计 `apiFindings` 按 `kind` 的分布
- **THEN** `apiEntryMissing = 12`（逐条为 8 个视图目录根 + 3 个视图内子域 + `App.Frontend.Components.Ui`）、`apiMissing = 29`（13 个 crate + `App.Frontend.Shell` + **14** 个前端单文件根 + `App.Backend.Shell`，逐条为「declared 但契约文件缺失」；两个新元素 `Components.Agent.SessionPanel` / `Components.Standard.Tooltip` 各居其中一条）、其余七条各 0、`apiUndeclared = 0`；壳 MUST NOT 同时出现在两个桶里（① 命中即跳过 ②）；两个工具 crate 两桶皆零（`entry` 命中 + `api = none`——「有入口」与「无库 API」两条事实同时在场，MUST NOT 只留一条）；`status` 仍为 `clean`（纳管子集为空）

#### Scenario: 假阳与假阴都可辨

- **WHEN** 人为让某入口可解析的元素也被判 `apiEntryMissing`（假阳），或让某无入口元素漏报（假阴）
- **THEN** 前者的报告数变为 13（多于基线）后者变为 11（少于基线）；两种偏离都判为检查器缺陷，MUST NOT 更新基线表放行

#### Scenario: 判定面用例与报告面语料各司其职

- **WHEN** 运行 `crates/core/archi/src/api/**` 的 sidecar 用例与 `crates/core/archi/tests/golden_test.rs` 的 `DRIFT_CASES` 驱动面
- **THEN** 前者逐判据覆盖正反例（判据产出由 `check::check` 直驱断言，不依赖快照）；后者逐 case 驱动 `scan → drift::check → 报告`，每 case 的 `apiFindings` 与 `expected/<case>.<mode>.json` 逐字段一致（含 `kind` / `category` / `element` / `target` / `occurrences` 计数）；`api-mismatch` case 产非空 `apiFindings`（`apiSignatureMismatch`），其余既有金样 `apiFindings` 为空且其余字节不变；MUST NOT 存在第三套判定语料目录

#### Scenario: 新 case 未登记即红

- **WHEN** 新增 `corpus/drift/<case>/` 却未登记进 `DRIFT_CASES`，或登记了而 case 目录不存在，或旧金样未重写而缺 `apiFindings` 键
- **THEN** 三种情形各报红并点名该 case（缺失键 MUST NOT 静默补默认值）；golden 变更只经 `DESKTOP_GOLDEN_REWRITE=1` 显式重写并人工确认

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| 本 requirement 的基线表（正文，非代码常量） | 报告数对拍基线 | **12 / 29（总 41）**、分母 44；基线不写进断言，断言只锚定「实现期留痕的实测表与报告逐项相等」 |
| `.dev-team/architecture/models/06-frontend.toml` 的两个新元素（数据面） | ② 桶 +2 的唯一来源 | `App.Frontend.Components.Agent.SessionPanel` / `App.Frontend.Components.Standard.Tooltip`：文件型根 ⇒ 入口 `Resolved`、无契约文件 ⇒ 各一条 `apiMissing`（`kind` 与文案由既有判据单点产出，不新增判据） |
| 数据面（无 Function / API / CLI / Component 改动） | 判据实现零改动 | `api::check`、`ArchiApiFindingKind::severity()`、纳管子集 `gating_findings`、`REPORT_VERSION = 3` 与报告形状 MUST NOT 改动；本 delta 只换数字与读数串讲 |
