# desktop-archi-api-check Specification（delta: ts-export-closure-extraction）

## Purpose

本 delta 只改**导出集口径的一句话**，判据种类（十值封闭）、求值互斥序、判据级档位、纳管口径、报告形状、语料守护与读数基线 **12 / 29（总 41）** 逐字不动：

1. **口径更正**：`apiMissing` / `apiUndeclared` / `apiSignatureMismatch` 的「代码导出集」在 TS 侧由「跨元素被 import 的 `export function`」改为「经**入口导出闭包**可达的导出」——旧措辞既与今天的实现不符（取数器只读入口单文件），也与 barrel 的公共面定义不符；闭包的口径与外层约束归 `desktop-archi-api`「TS 导出闭包的取数面」，本能力 MUST NOT 另立第二份 TS 导出集判据。
2. **语义不变的机械判据**：闭包只可能让导出集**变宽** ⇒ ② `apiMissing` 只可能**减少**、④ `apiSignatureMismatch` 仍逐符号比对；③ `apiUndeclared` 档位仍 `off` ⇒ 闭包扩容**不产生任何新条目**。
3. **本仓读数零变化**：唯一持 barrel 的 `App.Frontend.Components.Agent` 尚无契约文件（② 报的是「declared 但契约缺失」，与导出集宽度无关），故基线仍 `12 / 27 + 2` = **12 / 29（总 41）**、分母 44、`status = clean`、`passed = true`。

## MODIFIED Requirements

### Requirement: 十条判据的定义与求值互斥顺序

检查 SHALL 对每个 **`path` 非空**的元素求值十条判据（容器元素 `path` 恒空即天然无契约；唯一持 barrel 的容器 `App.Frontend.Components.Agent` 有 `path`，故纳入），求值顺序 SHALL 固定且**互斥**：

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
- **契约面启用条件（冷启动纪律）**：`<archi_api_dir>` 缺席或其中零份 `*.md` ⇒ 契约面判为**未启用**，`apiFindings` SHALL 为空数组且不产出任何一条契约判据。理由与「无模型即 `skipped`」同款：迁移期仓库与全部既有漂移语料都没有契约资产，若把「`api = declared` 缺契约文件」在此刻全量展开，既有语料的 `apiFindings: []` 与「金样仅追加空槽位」的兼容承诺同时失效。MUST NOT 把该条件做成逃生口：目录一旦存在（在案，试点契约在内），**全部 `path` 非空元素**（本仓 44 个）重新纳入判据，`declared` 缺文件即 `apiMissing` / 无入口即 `apiEntryMissing`——不存在「部分元素豁免」的中间态，也不设白名单或开关。

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

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/api/check.rs` | 十条判据单点求值 | **零改动**：声明集 / 导出集比对逻辑逐字不变；导出集由 `extract::extract_element` 给出（本变更只把 TS 侧的取数面换成语义等价更宽的入口闭包）⇒ ② 只减不增、③ 仍 `off`、④ 仍逐符号归一比对。判据面 MUST NOT 自行解析 re-export、MUST NOT 内联 owner 归属判断（那是取数面的单点） |
| `crates/core/archi/src/api/check_test.rs` | 判据语料 | **断言值零变化**：`function()` / `type_symbol()` 夹具补 `ApiSymbol.file` 字段（形状适配，不新增判据用例）；既有十判据正反例、`entry` 互斥、签名归一、留白双向等语料逐字保留 |
| `crates/core/archi/src/drift/report.rs` + `drift/mod.rs` | 报告槽位与 `status` 推导 | **零改动**：`ArchiApiFindingKind` 十值、`severity()`、纳管子集 `gating_findings`、`REPORT_VERSION = 3` 与 `apiFindings` 排序逐字不变 |
| 读数基线（本 requirement 正文，非代码常量） | 报告数对拍 | 本仓仍 **12 / 29（总 41）**、分母 44、`status = clean`、`passed = true`：唯一持 barrel 的元素尚无契约文件（② 报的是「declared 但契约缺失」，与导出集宽度无关），`apiUndeclared` 仍 `off`。基线 MUST NOT 写进断言（易随元素增减漂移），断言只锚定「实现期留痕的实测表与报告逐项相等」 |

### API: 无

本 delta 不改任何 IPC 命令、CLI 子命令或脚本面（`api:check` / `api:export` / `check` 四段逐字不变）。

### Component: 无

呈现面（架构页「契约」视图 / 元素抽屉「契约」段）按同一份 `apiFindings` 工作，条目数与 `category` 分布不变 ⇒ 零改动。
