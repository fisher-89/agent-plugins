# desktop-archi-api-check Specification（delta: module-entry-docs-semantics）

## Purpose

本 delta 只做**启用判据的换根**（E-1 目录迁移）：契约面的「冷启动纪律」由「`<archi_api_dir>` 递归含 ≥1 份 `*.md`」改为「`<archi_models_dir>` 递归含 ≥1 份 `*.md`」。**十条判据的词汇、互斥求值序、档位、纳管子集、报告形状与全部实测读数逐字不变**——本变更零 `check.rs` 改动，盘上契约文件只是搬了目录，`enabled` 仍为 `true`，`apiFindings` 仍是 `0 + 41 = 41`（`status = clean`、`passed = true`）。**边界说准**：上述读数的来源是**变更前构建的宿主门禁**；本 change 侧的判据面止于**源码两面 + 离线可推**（`paths::directory` 指 `<archi_models_dir>` / `enabled` 实现逐字未动 / `44 − 2 − 1 = 41`），而 `enabled === true`、41 条 `apiFindings` 与 `reports/archi/<timestamp>.json` 留痕的**运行态读数**属宿主重建后的**移交前置**（见「契约判据的验证语料与报告数基线」）。

## MODIFIED Requirements

### Requirement: 十条判据的定义与求值互斥顺序

检查 SHALL 对每个 **`path` 非空**的元素求值十条判据（容器元素 `path` 恒空即天然无契约；持 barrel 的目录型 `module` `App.Frontend.Components.Agent` 有 `path`，故纳入——判据只读 `path` 是否非空，不读 `kind`），求值顺序 SHALL 固定且**互斥**：

```
① apiEntryMissing（结构前提：该元素是否解析出可用入口）
   └─ 命中 → 该元素不再求 ② ③ ④（结构上不可判），仅继续求 ⑤
② apiMissing        契约 → 代码（含「declared 但契约文件缺失」与「契约声明的 op / 类型在代码里没有」）
③ apiUndeclared     代码导出集 → 契约声明集（档位 off，见下条 requirement）
④ apiSignatureMismatch  双向（契约签名与代码签名归一后比对）
⑤ apiUnexpected     留白 ↔ 契约文件存在性（与 ① 独立）
嵌套与透传面（独立于上述，逐元素求值）：
⑥ apiShadowing / ⑦ apiRefMissing / ⑧ apiNestedCrossCrate / ⑨ apiReexportDrift / ⑩ apiConstructorDeclared
```

- **① 的判定 SHALL 单点取自 `paths::entry_state`**：`Resolved` ⇒ 继续 ②③④；`Absent`（无候选 / 约定默认全不落盘）与 `Invalid(<problem>)`（形态 / 落位 / 存在 / 唯一任一违反）⇒ 一条 `apiEntryMissing`。判据面 MUST NOT 自行判文件存在性、MUST NOT 保留第二份入口探测（旧 `has_generated` 双条件判定已随落点变更退役）。
- **互斥的理由**：`apiEntryMissing` 比 `apiMissing` 更基础——**在契约生成之前就失效**。若两条判据对同一元素同时报，报告数无法单调对拍（消一条减一），且反馈会把 executor 引向「去写代码实现一个不存在的模块入口」。
- **`apiMissing` 的声明集 / 导出集口径**：声明集 = 契约 `### 操作` 的 id 集（**构造器双向都不参与比对**——既不在契约声明集，也不计入导出集）；导出集 = 元素的 `external` 代码导出（Rust 经 `pub mod` 链或 `pub use` 白名单可达的 `pub fn`；TS 经**入口导出闭包**可达的导出——入口自身声明 + 沿 `export … from` 递归并入且目标归属在本元素 FQN 子树内的导出，定义见 `desktop-archi-api`「TS 导出闭包的取数面」，本能力 MUST NOT 另立第二份 TS 导出集口径）。签名比对 SHALL 在**归一后**进行（AST 归一单行、参数名与空白不参与）。
- **`apiUndeclared` 只读 `<FQN>.schema.json` 的 `$defs` 键集**，MUST NOT 并读 markdown。安全性由前序门禁保证：`$defs` 键集 = `api-types` ∪ `api-reexport`，而 `apiReexportDrift` 保证 fence 与散文一致、零 diff 门保证 schema 零手改自由度。
- **`apiUnexpected` 单向**：只判「`api = none` 而契约文件存在」（多出来的契约）。反方向（`api = declared` 而契约文件缺失）归 `apiMissing` —— 两条判据 MUST NOT 对同一事实双报，否则报告数无法单调对拍。
- **`apiMissing` 覆盖契约 → 代码的两类缺口**：① 元素 `api = declared` 而其契约文件不存在；② 契约文件在、但其声明的 op / `$defs` 类型在代码导出集里不存在。① 是迁移期主形态（绝大多数元素尚无契约），② 是契约与代码的真实漂移。两类缺口 SHALL 用**同一条判据、同一次求值**（存在性判定，签名归 `apiSignatureMismatch`）；共同的排除面三条 = 构造器双向排除 / 透传纯 `$ref` alias 排除 / `x-class` 与 `enum` 计入类型面；结构前提 SHALL 含 `#[path = "..."] mod` 的模块定位与 `pub use <mod>::*` 的通配升格，缺任一条则本判据在自有生成物的元素上恒假阳。
- **`apiMissing` 天然支持渐进**：契约里只写已确定的 op 即可，未写的 op 归 `apiUndeclared`（关档），不落在 `apiMissing`。
- **`apiConstructorDeclared` 是「契约自身矛盾」的判据**（合法新增判据的范围）：构造器由生成器自动产出，写进契约必然导致 `apiMissing` 误报——试点纸面验证已实测该误报。
- **`apiRefMissing` 的悬空引用在 schema 层由外部校验器编译期捕获**（ajv 对 schema 层的 `$ref` 报 `can't resolve reference`），但 `x-function` 内的 `$ref` **不被解析**（`x-function` 是未知关键字，校验器不下沉）——故函数签名里的跨模块引用只能由本判据与生成器保证。该边界 SHALL 在信心中如实体现：`schema.json` 的定位是「类型层的可校验投影」。
- **零第二份判定**：十条判据 SHALL 只有一处实现（`crates/core/archi/src/api/check.rs`），命令面与门禁面 MUST NOT 各写一份。
- **契约面启用条件（冷启动纪律）**：`<archi_models_dir>` 缺席或其中**递归**零份 `*.md` ⇒ 契约面判为**未启用**，`apiFindings` SHALL 为空数组且不产出任何一条契约判据。被监测的目录是本变更的换根点（原 `<archi_api_dir>` 已退役），**判据形态零变化**：仍是「递归含 ≥1 份 `*.md`」，`api::enabled` 单点判定。**模型文件 MUST NOT 构成启用证据**：`models/` 已同时住模型与契约资产，故一级 `NN-<域>.toml` 的在场不等于契约面启用——「有模型、零契约 md」必须是 `enabled = false`。理由与「无模型即 `skipped`」同款：迁移期仓库与全部既有漂移语料都没有契约资产，若把「`api = declared` 缺契约文件」在此刻全量展开，既有语料的 `apiFindings: []` 与「金样仅追加空槽位」的兼容承诺同时失效。MUST NOT 把该条件做成逃生口：目录一旦存在且含契约 md（在案，试点契约在内），**全部 `path` 非空元素**（本仓 44 个）重新纳入判据，`declared` 缺文件即 `apiMissing` / 无入口即 `apiEntryMissing`——不存在「部分元素豁免」的中间态，也不设白名单或开关。

#### Scenario: entry 优先互斥

- **WHEN** 某元素既无可用入口（`Absent`，或候选被判 `Invalid`）、`api = declared` 且其契约声明了 2 个 op
- **THEN** 只报一条 `apiEntryMissing`；同元素不出现 `apiMissing` / `apiUndeclared` / `apiSignatureMismatch`（含「契约文件缺失」这一 `apiMissing` 形态）；`apiUnexpected` 仍可独立报

#### Scenario: 契约声明 op 而代码无

- **WHEN** 元素入口存在、契约声明 `parse` 与 `all_conclusions` 两个 op，代码只有 `parse`
- **THEN** 报一条 `apiMissing`，点名 `all_conclusions`；`apiUndeclared`（`off`）不产出条目

#### Scenario: TS 导出集取闭包且判据语义不变

- **WHEN** 某 TS 元素的入口是纯 re-export barrel（7 个导出名全部经 `export … from` 从本元素子树内的文件汇入），契约声明了其中 2 个 op 与 2 个类型
- **THEN** ② `apiMissing` 对这 4 个声明零命中（导出集含闭包内符号），`apiUndeclared` 仍 `off` ⇒ 闭包扩容不产出任何新条目，④ 仍逐符号比对归一签名；判据种类（十值）、互斥求值序、判据级档位与报告形状逐字不变——**导出集变宽只可能减少 `apiMissing`，MUST NOT 出现「闭包扩张导致新增 finding」的路径**

#### Scenario: 签名归一后比对

- **WHEN** 契约声明 `parse_cargo_test(input: string) -> SummaryReport`，代码把签名排成多行且参数名不同但类型序列相同
- **THEN** 归一后判定一致、零 `apiSignatureMismatch`；把返回类型改为 `Vec<SummaryReport>` 时报一条 `apiSignatureMismatch` 并给出两侧归一签名

#### Scenario: 留白与缺文件各归一条判据

- **WHEN** 分别构造「`api = none` 但契约文件存在」与「`api = declared` 但契约文件缺失」
- **THEN** 前者报 `apiUnexpected`（文案指明多出的契约文件路径）；后者报 `apiMissing`（文案指明缺失的契约文件路径）；同一元素 MUST NOT 同时出两条

#### Scenario: 构造器声明即契约自身矛盾

- **WHEN** 契约声明 `### new` 而代码的 `new()` 由生成器产出
- **THEN** 报一条 `apiConstructorDeclared` 且**不**同时报 `apiMissing`（构造器不进声明集比对）

#### Scenario: 生成物在盘不豁免入口缺失

- **WHEN** 某元素无可用入口（未声明 `entry`、根为目录型且无 `index.ts` / `index.tsx`），而盘上仍残留一份旧位 `.i.ts`
- **THEN** 仍报一条 `apiEntryMissing`（`.i` 的存在不再是入口的替代证据）；该残留文件同时由 `api:check` 的 `orphaned` 列报出

#### Scenario: 启用判据的换根与「模型文件不是启用证据」

- **WHEN** 观察根下 `models/` 存在且恰有六个 `NN-<域>.toml`、零份 `*.md`；随后在 `models/<Domain>/` 放入一份契约 md 再跑一次
- **THEN** 前者 `enabled = false` 且 `apiFindings = []`（六个 toml 在场 MUST NOT 被读成启用）；后者 `enabled = true`、全部 `path` 非空元素纳入判据；两例的判定函数相同（`<archi_models_dir>` 递归含 ≥1 份 `*.md`），MUST NOT 在命令层另判一套

### Requirement: 契约判据的验证语料与报告数基线

契约判据的正确性 SHALL 以**报告数逐项对拍**验证（存量债务即验证语料），并 SHALL 有语料守护：

- **基线表（本仓现状，实测复核；分母 = `path` 非空的 44 个元素）**：`apiEntryMissing` **0**（12 个前端目录根已由 `desktop-frontend-module-entry` 的 `index.ts` / `index.tsx` 经 ③ 命中 ⇒ ① 不再成立）、`apiMissing` **41**（14 个 crate 元素 − 1 个试点 = 13，+ 多文件根 `App.Frontend.Shell` + **14** 个前端单文件根元素 = 28，+ 壳 `App.Backend.Shell`，**+ 12 个前端目录根元素**——它们由 ① 面平移而来：入口可解析 ⇒ ① 不成立，而 `api = declared` 且契约文件缺失 ⇒ ② 成立）、其余七条一致性判据各 **0**、`apiUndeclared` 不适用（`off`）——两者即字面的 **0 / 41**、总 **41**。
- **两条切分口径**：① `apiEntryMissing` 只由「无可用入口」产生——`Absent`（未声明且无约定默认）与 `Invalid`（形态 / 落位 / 存在 / 唯一任一违反）同归此项；② 入口一旦可解析，该元素就不再享有 `apiEntryMissing` 的静默，改为承担「有门无契约」的 `apiMissing`（`api = none` 的元素除外：无库 API ⇒ ② 不适用，故 ① 退场即零判据）。故**总条目数 0 + 41 = 41** 全由这两条口径导出，且与历史读数连成同一规则的**五段**：`28 + 13 = 41`（A 之前，入口靠探测）→ `15 + 26 = 41`（A 落地，13 个前端文件型根由「猜不到入口」转为「有门无契约」，壳与两工具 crate 仍在 ① 面）→ `12 + 27 = 39`（A 的数据面换段：壳由 ① 平移至 ②，净 0；两工具 crate 各退出 ①，净 −2）→ `12 + 29 = 41`（`SessionPanel` / `Tooltip` 两个共享组件文件补建元素后，「有入口、无契约」使 ② +2，① 仍 12）→ `0 + 41 = 41`（12 个前端目录根补入口后整桶平移，① 归零、② 由 29 涨到 41）。五段 MUST NOT 被读成「债务增减」——它是**分桶**与**元素面扩容**的移动；**末段尤其必须读准：净 0、全平移**，MUST NOT 被写成「入口债已清」。
- **目录换根不改读数（本变更）**：契约资产由 `<archi_api_dir>` 搬至 `<archi_models_dir>/<Domain>/`、索引搬至 `<archi_graph_dir>` 之后，基线表 SHALL **逐项不变**——判据读的是「契约文件是否在案」（按 FQN 派生的新路径），搬目录既不新增也不消除任何一条；故本变更的结论 SHALL 记为「**同样的 0 / 41，换的只是契约文件的绝对位置**」，MUST NOT 被读成一次新的换段，MUST NOT 被写进五段历史链。
- **基线单段（数据不分批）**：三处显式 `entry`（壳 + 两个工具 crate）与 12 个目录模块入口（走 ③，零 `entry` 行）SHALL 随模型数据与盘上文件在案（`repo-archi-model`「基线落位与模型文件划分」、`desktop-frontend-module-entry`），故基线只有一段 **0 / 41（总 41）**；`12 / 29（总 41）`、`12 / 27（总 39）`、`15 / 26`、`28 + 13` 均为历史段读数，MUST NOT 再按其验收，MUST NOT 保留「后续变更再换段」的例外，也 MUST NOT 把该例外改写为「按某一历史段验收」的残留。
- **判定口径**：**报告数与基线表逐项相等**即「检查能正确提示」。多于基线 = 判据误报（假阳），少于基线 = 判据漏报（假阴），两者都是检查器缺陷——MUST NOT 以「报告少就是代码干净」自我解释；本段的总数未变（41），故「总数对得上」**不构成**段内分桶正确的证据，`kind` 分布 MUST 逐项对。
- **换段后读数不变的「本 change 可判面」（源码两面 + 离线推导）**：契约面读数 SHALL 由三条本 change 内可机械复核的面守：① `api::paths::directory(root)` 的实现体 == `archi_models_dir(root)`（契约目录与模型目录同址；索引另由 `archi_graph_dir` 派生，MUST NOT 由 `directory` join 得到）；② `api::enabled` 的实现体逐字为 `has_markdown(&paths::directory(root), 0)`——判据形态（递归含 ≥1 份 `*.md`、深度上限、符号链接不跟随）零改动，换根经 `directory` 单点传导，MUST NOT 在命令层另判一套；③ 离线可推：`path` 非空元素 **44** − `api = "none"` **2** − 契约在案 **1** = `apiMissing` **41**，与 `apiEntryMissing` **0** 合成 **0 / 41（总 41）**、`status = clean`，与基线表逐项一致。
- **反向判据（模型文件不是启用证据）**：观察根下 `models/` 一级保留六个 `NN-<域>.toml` 而 `models/**` 递归零份 `*.md` ⇒ `enabled = false`、`apiFindings = []`；在 `models/<Domain>/` 放入一份契约 md ⇒ `enabled = true`、全部 `path` 非空元素重新纳入。两例共用同一判定函数（`<archi_models_dir>` 递归含 ≥1 份 `*.md`）。
- **留痕报告是移交前置，不是本变更的交付物**：`<change>/reports/archi/<timestamp>.json` 由门禁进程写出，而承载判据的宿主门禁二进制是**变更前构建**（其 `paths` 仍指已退役的 `<archi_api_dir>` ⇒ `enabled = false` ⇒ 只能写出 `apiFindings: []`），**任何相位都无权重建 / 重启它**。故 `apiFindings` 长度 **41**（`kind` 分桶同基线表）、`status = clean` 的报告 SHALL 由宿主以**换段后源码重建后**的门禁 / verify 相位重跑生成（移交前置）。本变更侧 MUST NOT 以 `apiFindings: []` 的报告、空目录或 `reports/archi/` 的目录存在性充当「判过 41 条」的证据；`reports/archi/` SHALL 视为**门禁运行时产物**——不入库、不计入交付面、不作判红面（`git status` 出现 `??` 属预期），MUST NOT 为其设置隐藏 ACL / 权限护栏（不入库即可）。
- **基线须显式留痕**：基线数字写入 change 报告（`reports/`）与实现期实测对照；数字变更（新增 crate 使 entry-missing +1、前端补 barrel 使 `apiEntryMissing` 归零且 `apiMissing` +12、补三处显式 `entry` 使壳换桶且两工具 crate 退出、补建共享组件元素使 `apiMissing` +2）SHALL 在报告内说明成因，MUST NOT 直接在代码里改期望数。基线表本身**不写进断言**（易随元素增减漂移），断言只锚定「实现期留痕的实测表与报告逐项相等」。
- **语料守护（分两层，各有 owner）**：① **判定面逐判据用例**由 `crates/core/archi/src/api/**` 的十一份 sidecar 用例承担——契约解析（双向 gate 三条）/ `$defs` 规则五条 / 十判据各自正反例 / entry 互斥（含入口四理由）/ 签名归一 / 嵌套与透传（选择性 / 纯 `$ref` / 交集 / 直接子元素 / 跨 crate）/ 生成物渲染两侧键集一致；MUST NOT 另建 `tests/corpus/api/` 一类第二套判定语料（既有 crate 级语料只有 `corpus/dsl/` 与 `corpus/drift/` 两套，多一套即第二个驱动面）。② **crate 级报告面语料**沿既有 `crates/core/archi/tests/corpus/drift/` + `tests/golden_test.rs` 的 `DRIFT_CASES` 登记面驱动，快照为 `expected/<case>.<mode>.json`：既有 `api-mismatch` case（`contracts/App/App.Mod.md` + `models/01-app.toml` + `repo/src/mod/mod.rs`）SHALL 保持非空 `apiFindings`（其目录型根的 `src/mod/mod.rs` 由约定默认 `<root>/mod.rs` 命中，故 `modelHash` 与金样逐字不变）；新增 case 未登记进 `DRIFT_CASES` 即红、登记了但 case 目录不存在亦红。golden 更新 SHALL 沿既有 `DESKTOP_GOLDEN_REWRITE=1` 显式重写并人工确认（快照不参与判定）。**本变更零 `check.rs` 改动 ⇒ 十六份金样中十五份逐字节不变**；唯一例外是 `api-mismatch.full.json` 的**一行**：其 `occurrences[0].file` 是 repo 相对契约路径（`.dev-team/architecture/api/App/App.Mod.md` → `.dev-team/architecture/models/App/App.Mod.md`），由契约夹具暂存根随 `archi_models_rel_dir()` 派生而必然换段；该份的 `modelHash` / `kind` / `category` / `message` / `line` / `specifier` / `typeOnly` 与其余字段 SHALL 逐字不变。**MUST NOT 把本条写成「十六份金样逐字不变」**（与 `occurrences[].file` 存 repo 相对路径这一事实互斥），也 MUST NOT 因该行差异而回退夹具落点。语料里的契约夹具随新落点摆放（`models/<Domain>/`），除该行外不改任何读数。
- **MUST NOT 把本仓模型与契约塞进 crate 语料**：本仓 `models/**`（含契约子树）与 `graph/**` 是仓库数据资产，纳入 crate 测试语料会破坏「语料自足、可移植」；本仓核对以命令 + 留痕执行（落 change `reports/`）。
- **生成器正确性由自身单测守护**：`verify`（零 diff + 编译器）覆盖不到生成器自身的 bug——渲染、装配、抽取三面各有逐字段断言，MUST NOT 只靠端到端零 diff。

#### Scenario: 契约面未启用时零判据

- **WHEN** 观察根下 `<archi_models_dir>` 不存在（或存在但递归零份 `*.md`，例如只有六个 `NN-<域>.toml`）时跑一次全量检查
- **THEN** `apiFindings = []`（零条任何契约判据），`errors` / `warnings` / `status` 与不加契约面时逐字相同；MUST NOT 产 `apiMissing` / `apiEntryMissing`

#### Scenario: 试点契约落地后全量纳入

- **WHEN** 同一根下加入试点契约（`<archi_models_dir>/Infra/App.Backend.Infra.ArchiRuntime.md`）后重跑
- **THEN** 契约面转为启用，全部 `path` 非空元素（44 个）参与判据：产出基线表所述 **0** 条 `apiEntryMissing` 与 **41** 条 `apiMissing`（试点元素自身零命中）；试点元素不因「有契约」而被豁免于其它判据

#### Scenario: 报告数与基线逐项相等

- **WHEN** 以本仓根跑一次全量检查并统计 `apiFindings` 按 `kind` 的分布
- **THEN** `apiEntryMissing = 0`、`apiMissing = 41`（13 个 crate + `App.Frontend.Shell` + **14** 个前端单文件根 + `App.Backend.Shell` + **12** 个前端目录根（`Components.Ui` + 8 个视图目录根 + 3 个视图内子域），逐条为「declared 但契约文件缺失」；两个较早补建的元素 `Components.Agent.SessionPanel` / `Components.Standard.Tooltip` 仍各居其中一条）、其余七条各 0、`apiUndeclared = 0`；壳 MUST NOT 同时出现在两个桶里（① 命中即跳过 ②）；两个工具 crate 两桶皆零（`entry` 命中 + `api = none`）；**12 个前端目录元素两桶合计恰一条**；`status` 仍为 `clean`（纳管子集为空）

#### Scenario: 换段验收不依赖门禁在线读数

- **WHEN** 本变更在宿主门禁**未重建**的工作树上验收（二进制仍按已退役的 `<archi_api_dir>` 派生路径，故它写出的报告只能是 `apiFindings: []`）
- **THEN** 契约面的换根结论 SHALL 只以源码面（`paths::directory` 指 `<archi_models_dir>`、`enabled` 实现逐字未动）与离线推导（`44 − 2 − 1 = 41`）判定；`reports/archi/` 内的 `apiFindings: []` 报告、空目录或该目录的存在性 MUST NOT 被读作「判过 41 条」；41 条 `apiFindings` 与 `status = clean` 的报告由宿主以换段后源码重建后的门禁 / verify 相位补留痕（移交前置）

#### Scenario: 假阳与假阴都可辨

- **WHEN** 人为让某入口可解析的元素也被判 `apiEntryMissing`（假阳），或让某 `declared` 缺契约文件的元素漏报 `apiMissing`（假阴）
- **THEN** 前者使 `apiEntryMissing` 由 0 变 1（多于基线）；后者使 `apiMissing` 由 41 变 40（少于基线）；两种偏离都判为检查器缺陷，MUST NOT 更新基线表放行

#### Scenario: 判定面用例与报告面语料各司其职

- **WHEN** 运行 `crates/core/archi/src/api/**` 的 sidecar 用例与 `crates/core/archi/tests/golden_test.rs` 的 `DRIFT_CASES` 驱动面
- **THEN** 前者逐判据覆盖正反例（判据产出由 `check::check` 直驱断言，不依赖快照）；后者逐 case 驱动 `scan → drift::check → 报告`，每 case 的 `apiFindings` 与 `expected/<case>.<mode>.json` 逐字段一致（含 `kind` / `category` / `element` / `target` / `occurrences` 计数）；`api-mismatch` case 产非空 `apiFindings`（`apiSignatureMismatch`），其余既有金样 `apiFindings` 为空且其余字节不变；MUST NOT 存在第三套判定语料目录

#### Scenario: 新 case 未登记即红

- **WHEN** 新增 `corpus/drift/<case>/` 却未登记进 `DRIFT_CASES`，或登记了而 case 目录不存在，或旧金样未重写而缺 `apiFindings` 键
- **THEN** 三种情形各报红并点名该 case（缺失键 MUST NOT 静默补默认值）；golden 变更只经 `DESKTOP_GOLDEN_REWRITE=1` 显式重写并人工确认

## Module Contract

### Function: 受影响入口（判据实现零改动，只换输入目录）

| 模块 | 变化 | 关键契约 |
|---|---|---|
| `crates/core/archi/src/api/mod.rs::enabled` | 改（启用判据换根） | `enabled(root)` 的判据形态（递归含 ≥1 份 `*.md`、深度上限、符号链接不跟随）逐字不变，只把被监测目录由 `<archi_api_dir>` 换成 `<archi_models_dir>`；本能力**不另判一套**（`desktop-archi-contract-panel` 的 `enabled` 字段直接取它） |
| `crates/core/archi/src/api/check.rs` | **零改动** | 十条判据的词汇、互斥求值序、声明集 / 导出集口径、档位与纳管子集逐字不变；本变更零 `check.rs` diff，出现 diff 即误动 |
| `crates/core/archi/src/api/*_test.rs` | 改（夹具落点） | 契约夹具由 `<archi_api_dir>/<Domain>/` 换到 `<archi_models_dir>/<Domain>/`；每条判据的正反例与断言值逐字不变 |
| `crates/core/archi/tests/corpus/drift/api-mismatch/**` + `expected/*.json` | 改夹具路径、**零读数改动** | 契约夹具随新落点摆放；十六份金样的 `apiFindings` 与 `modelHash` 逐字不变（`desktop-archi-drift` 的收面零改动）；字节面唯一例外 = `api-mismatch.full.json` 的 `occurrences[0].file` 一行换 base（repo 相对契约路径由 `archi_models_rel_dir()` 派生） |
| `<change>/reports/archi/<timestamp>.json`（门禁留痕） | **移交前置（非本变更交付物）** | 报告由门禁进程写出，而宿主门禁二进制是变更前构建（`paths` 仍指已退役的 `<archi_api_dir>` ⇒ 它只能写 `apiFindings: []`），任何相位无权重建 / 重启它；`apiFindings` 长度 **41**、`status = clean` 的报告 SHALL 由宿主以换段后源码重建后的门禁 / verify 相位重跑生成。本变更侧判据 = 源码两面（`paths::directory` 指 `<archi_models_dir>` / `enabled` 实现逐字未动）+ 离线可推 `44 − 2 − 1 = 41`；MUST NOT 以 `apiFindings: []` 或空目录充当证据；`reports/archi/` 为运行时产物（不入库、不计入交付面、不作判红面，`??` 属预期） |
