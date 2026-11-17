# desktop-archi-api-check Specification（delta: frontend-barrel-entry-migration）

## Purpose

本 delta 只换**读数与两处叙述实例**，判据种类（十值封闭）、求值互斥序、判据级档位、纳管口径、报告形状、语料守护与判据实现（`api/check.rs`）**逐字不动**（本变更**零 Rust 改动**）：

1. **基线第五段 `0 + 41 = 41`（净 0、全平移）**：12 个前端目录根补 `index.ts(x)` 后，它们由 ① `apiEntryMissing` 面整体平移至 ② `apiMissing` 面——① 归零、② 由 29 涨到 41，总数仍 41。这与 A 数据面那一步「真正收了两条债」**性质相反**，MUST 在基线表里点明，否则下一个人会读成「入口债已清 ⇒ 判据面干净」。
2. **两处叙述实例翻新**：`App.Frontend.Components.Agent` 已由「容器 + 文件型 `path`」翻转为目录型 `module`，故「唯一持 barrel 的容器有 `path`，故纳入」与「MUST NOT 顺带为消 `apiEntryMissing` 而补 12 个前端目录的 barrel（那是独立变更）」两句改准——**判据面纪律不变**：本能力仍 MUST NOT 要求建入口或改代码分层，12 个 barrel 是由独立变更落地的代码事实，不是判据面的产出。
3. **零门禁代价的机械化**：12 个前端元素零契约文件 ⇒ 纳管子集仍为空 ⇒ `status = clean`、`passed = true`；`apiFindings` 只是并列槽位的如实呈现（纳管口径逐字不动）。

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

### Requirement: 判据级档位与元素 gate 解耦

十条判据的档位 SHALL 由判据自身承载（`ArchiApiFindingKind::severity()`，**判据级档位**），MUST NOT 读元素 `gate`：

| 类别 | 判据 | 判据级档位 |
|---|---|---|
| **一致性判据**（契约说 A、代码是 B） | `apiEntryMissing` / `apiMissing` / `apiSignatureMismatch` / `apiUnexpected` / `apiShadowing` / `apiRefMissing` / `apiNestedCrossCrate` / `apiReexportDrift` / `apiConstructorDeclared` | **`warn`** |
| **完备性判据**（代码有的契约都得有） | `apiUndeclared` | **`off`** |

- **分界线写死**：一致性判据报的是「债」（可枚举、可消除，可即开 `warn` 验证检查器是否工作）；完备性判据报的是「海」（迁移期必然爆炸，必须后置）。`apiUndeclared` 是唯一后者。
- **与元素 `gate` 解耦的理由**：`ArchiDriftKind` 的定级看引入方元素的 `gate`；契约判据若也接 `gate`，收紧契约就要改模型，两套语料同时动、收紧节奏无法独立。故元素 `gate` 全 `off` 与本能力判据全 `warn` SHALL 同时成立——**「统一目录」与「判据全 off」不能同时成立**：相邻 + off 尚有软提醒，统一 + off 是零提醒零校验 → 契约静默腐烂。
- **未来收紧路径**：收紧由 `severity()` 单值改动驱动（一个元素或一类判据一个 change 收紧），MUST NOT 引入运行时开关、MUST NOT 引入配置键。
- **`warn` 报告即迁移 backlog**：`warn` 模式下的报告输出天然成为迁移待办表（`apiEntryMissing` 每一条都是待定论的任务：补入口声明 / 补约定入口 / 判 `api = none`——三条都是真话，MUST NOT 一律按「去建入口」理解；本仓该桶现已归零，故 ② 面的 41 条 `apiMissing` 是当前唯一 backlog），比手写 todo 更可靠（直接来自代码事实）。每消一条，报告数减一，可单调验证。
- **MUST NOT 为消债而重构**：本能力 MUST NOT 要求建入口文件、改元素根路径去扩展名、挪子→父类型依赖或上移壳元素根路径（存量债务只作检查器的验证语料）。唯一例外是**为让契约有效而暴露已存在的私有逻辑**（改可见性 + 挪进 `impl`，不动实现逻辑）——判据不同：前者是「格式追代码」，后者是「代码追上契约应有内容」。入口面同理：`entry` 字段与解析链只让「本来存在的入口」可被表达；12 个前端目录的 `index.ts(x)` 由独立变更 `frontend-barrel-entry-migration` 落地（那是**代码侧结构选择**，不在本能力内推进），本能力 MUST NOT 在判据面要求补入口，也 MUST NOT 因「12 条 ① 已归零」把 ② 面的 41 条重新包装成「待补入口」。

#### Scenario: 元素 gate 全 off 而判据仍报

- **WHEN** 模型内全部元素 `gate = off`（本仓现状）且合成语料中存在若干无可用入口元素（本仓现基线为 **0**；判据面语料自足、不依赖本仓现状）
- **THEN** 每个无可用入口元素各产一条 `apiEntryMissing`（判据级 `warn`），与元素 `gate` 无关；把某元素 `gate` 改成 `strict` 后该数量与内容零变化

#### Scenario: 完备性判据保持 off

- **WHEN** 代码有 `external` 导出而契约未声明
- **THEN** `apiUndeclared` 档位为 `off`，报告不产出该条目（`apiFindings` 内零 `apiUndeclared`）；把档位改为 `warn` 属独立裁决（MUST NOT 在本变更内顺手打开）

#### Scenario: 零运行时开关

- **WHEN** 审查判据档位的来源
- **THEN** 唯一来源是 `ArchiApiFindingKind::severity()`；无配置键、无环境变量、无命令行 flag 可改档位

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

- **WHEN** 报告含 41 条 `apiMissing`（本仓第五段读数：12 条由 ① 面平移而来 + 29 条原有；全部来自**契约不在案**的元素）
- **THEN** 纳管子集为空、`status` 与引入 `apiFindings` 前逐字相同（本仓现状：唯一契约在案元素判据 0 条 ⇒ `status` 不变）；MUST NOT 因这 41 条把状态词常规化为 `warnings`（这些元素都是 backlog 而非承诺未兑现，**「12 条从 ① 面搬到 ② 面」不改变纳管结论**）

#### Scenario: 稳定序与聚合

- **WHEN** 同一元素对的三处签名不一致被三处命中
- **THEN** `apiFindings` 内该元素对恰一条，`occurrences` 含 3 项（各带 `file` / `line`）；整体排序为 `(kind, element, target)` 稳定序，两次运行逐字相同

#### Scenario: 命令面与门禁面同源

- **WHEN** 以同一 root 分别经 `archi_drift` 命令与门禁 runner 触发检查
- **THEN** 两份报告的 `apiFindings` 与 `status` 逐字段相等（含排序、`category` 与纳管结论）

### Requirement: 契约判据的验证语料与报告数基线
契约判据的正确性 SHALL 以**报告数逐项对拍**验证（存量债务即验证语料），并 SHALL 有语料守护：

- **基线表（本仓现状，实测复核；分母 = `path` 非空的 44 个元素）**：`apiEntryMissing` **0**（12 个前端目录根已由 `desktop-frontend-module-entry` 的 `index.ts` / `index.tsx` 经 ③ 命中 ⇒ ① 不再成立）、`apiMissing` **41**（14 个 crate 元素 − 1 个试点 = 13，+ 多文件根 `App.Frontend.Shell` + **14** 个前端单文件根元素 = 28，+ 壳 `App.Backend.Shell`，**+ 12 个前端目录根元素**——它们本轮由 ① 面平移而来：入口可解析 ⇒ ① 不成立，而 `api = declared` 且契约文件缺失 ⇒ ② 成立）、其余七条一致性判据各 **0**、`apiUndeclared` 不适用（`off`）——两者即字面的 **0 / 41**、总 **41**。
- **两条切分口径**：① `apiEntryMissing` 只由「无可用入口」产生——`Absent`（未声明且无约定默认）与 `Invalid`（形态 / 落位 / 存在 / 唯一任一违反）同归此项；② 入口一旦可解析，该元素就不再享有 `apiEntryMissing` 的静默，改为承担「有门无契约」的 `apiMissing`（`api = none` 的元素除外：无库 API ⇒ ② 不适用，故 ① 退场即零判据）。故**总条目数 0 + 41 = 41** 全由这两条口径导出，且与历史读数连成同一规则的**五段**：`28 + 13 = 41`（A 之前，入口靠探测）→ `15 + 26 = 41`（A 落地，13 个前端文件型根由「猜不到入口」转为「有门无契约」，壳与两工具 crate 仍在 ① 面）→ `12 + 27 = 39`（A 的数据面换段：壳由 ① 平移至 ②，净 0；两工具 crate 各退出 ①，净 −2）→ `12 + 29 = 41`（`SessionPanel` / `Tooltip` 两个共享组件文件补建元素后，「有入口、无契约」使 ② +2，① 仍 12）→ `0 + 41 = 41`（**本段**：12 个前端目录根补入口后整桶平移，① 归零、② 由 29 涨到 41）。五段 MUST NOT 被读成「债务增减」——它是**分桶**与**元素面扩容**的移动；**本段尤其必须读准：净 0、全平移**（与 A 数据面那一步「真正收了两条债」性质相反），MUST NOT 被写成「入口债已清」。
- **基线单段（数据不分批）**：三处显式 `entry`（壳 + 两个工具 crate）与 12 个目录模块入口（走 ③，零 `entry` 行）SHALL 随模型数据与盘上文件在案（`repo-archi-model`「基线落位与模型文件划分」、`desktop-frontend-module-entry`），故基线只有一段 **0 / 41（总 41）**；`12 / 29（总 41）`、`12 / 27（总 39）`、`15 / 26`、`28 + 13` 均为历史段读数，MUST NOT 再按其验收，MUST NOT 保留「后续变更再换段」的例外，也 MUST NOT 把该例外改写为「按某一历史段验收」的残留。
- **判定口径**：**报告数与基线表逐项相等**即「检查能正确提示」。多于基线 = 判据误报（假阳），少于基线 = 判据漏报（假阴），两者都是检查器缺陷——MUST NOT 以「报告少就是代码干净」自我解释；本段的总数未变（41），故「总数对得上」**不构成**段内分桶正确的证据，`kind` 分布 MUST 逐项对。
- **基线须显式留痕**：基线数字写入 change 报告（`reports/`）与实现期实测对照；数字变更（新增 crate 使 entry-missing +1、**前端补 barrel 使 `apiEntryMissing` 归零且 `apiMissing` +12（净 0 平移，本段即此例）**、补三处显式 `entry` 使壳换桶且两工具 crate 退出、补建共享组件元素使 `apiMissing` +2）SHALL 在报告内说明成因，MUST NOT 直接在代码里改期望数。基线表本身**不写进断言**（易随元素增减漂移），断言只锚定「实现期留痕的实测表与报告逐项相等」。
- **语料守护（分两层，各有 owner）**：① **判定面逐判据用例**由 `crates/core/archi/src/api/**` 的十一份 sidecar 用例承担——契约解析（双向 gate 三条）/ `$defs` 规则五条 / 十判据各自正反例 / entry 互斥（含入口四理由）/ 签名归一 / 嵌套与透传（选择性 / 纯 `$ref` / 交集 / 直接子元素 / 跨 crate）/ 生成物渲染两侧键集一致；MUST NOT 另建 `tests/corpus/api/` 一类第二套判定语料（既有 crate 级语料只有 `corpus/dsl/` 与 `corpus/drift/` 两套，多一套即第二个驱动面）。② **crate 级报告面语料**沿既有 `crates/core/archi/tests/corpus/drift/` + `tests/golden_test.rs` 的 `DRIFT_CASES` 登记面驱动，快照为 `expected/<case>.<mode>.json`：既有 `api-mismatch` case（`contracts/App/App.Mod.md` + `models/01-app.toml` + `repo/src/mod/mod.rs`）SHALL 保持非空 `apiFindings`（其目录型根的 `src/mod/mod.rs` 由约定默认 `<root>/mod.rs` 命中，故 `modelHash` 与金样逐字不变）；新增 case 未登记进 `DRIFT_CASES` 即红、登记了但 case 目录不存在亦红。golden 更新 SHALL 沿既有 `DESKTOP_GOLDEN_REWRITE=1` 显式重写并人工确认（快照不参与判定）。**本变更零 `check.rs` 改动 ⇒ 十六份金样逐字不变**（crate 语料的模型内无 barrel，`api-mismatch` 的读数不受影响）；12 条平移是本仓数据现象，MUST NOT 落成 crate 语料。
- **MUST NOT 把本仓模型与契约塞进 crate 语料**：本仓 `models/**` 与 `api/**` 是仓库数据资产，纳入 crate 测试语料会破坏「语料自足、可移植」；本仓核对以命令 + 留痕执行（落 change `reports/`）。
- **生成器正确性由自身单测守护**：`verify`（零 diff + 编译器）覆盖不到生成器自身的 bug——渲染、装配、抽取三面各有逐字段断言，MUST NOT 只靠端到端零 diff。

#### Scenario: 契约面未启用时零判据

- **WHEN** 观察根下 `<archi_api_dir>` 不存在（或存在但零份 `*.md`）时跑一次全量检查
- **THEN** `apiFindings = []`（零条任何契约判据），`errors` / `warnings` / `status` 与不加契约面时逐字相同；MUST NOT 产 `apiMissing` / `apiEntryMissing`

#### Scenario: 试点契约落地后全量纳入

- **WHEN** 同一根下加入试点契约（`<archi_api_dir>/Infra/App.Backend.Infra.ArchiRuntime.md`）后重跑
- **THEN** 契约面转为启用，全部 `path` 非空元素（44 个）参与判据：产出基线表所述 **0** 条 `apiEntryMissing` 与 **41** 条 `apiMissing`（试点元素自身零命中）；试点元素不因「有契约」而被豁免于其它判据

#### Scenario: 报告数与基线逐项相等

- **WHEN** 以本仓根跑一次全量检查并统计 `apiFindings` 按 `kind` 的分布
- **THEN** `apiEntryMissing = 0`、`apiMissing = 41`（13 个 crate + `App.Frontend.Shell` + **14** 个前端单文件根 + `App.Backend.Shell` + **12** 个前端目录根（`Components.Ui` + 8 个视图目录根 + 3 个视图内子域），逐条为「declared 但契约文件缺失」；两个较早补建的元素 `Components.Agent.SessionPanel` / `Components.Standard.Tooltip` 仍各居其中一条）、其余七条各 0、`apiUndeclared = 0`；壳 MUST NOT 同时出现在两个桶里（① 命中即跳过 ②）；两个工具 crate 两桶皆零（`entry` 命中 + `api = none`——「有入口」与「无库 API」两条事实同时在场，MUST NOT 只留一条）；**12 个前端目录元素两桶合计恰一条**（只在 ② 面，MUST NOT 同时出现在 ① 面——那是「入口未补」的旧读数）；`status` 仍为 `clean`（纳管子集为空）

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

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/api/check.rs` | 十条判据单点求值 | **零改动**：① 仍经 `paths::entry_state`（`Absent` / `Invalid(_)` ⇒ 一条 `apiEntryMissing`；`Resolved` ⇒ 继续 ②③④）；②③④ 的声明集 / 导出集 / 签名归一逐字不变；本变更只改变本仓**数据**（12 个元素由 ① 桶进 ② 桶），判据代码零行改动 |
| `crates/core/archi/src/drift/report.rs` | 判据词汇与报告槽位 | **零改动**：`ArchiApiFindingKind` 十值 + `ALL` + `severity()`、`ArchiApiFinding` 形状、`api_findings` 槽位、`REPORT_VERSION = 3` 逐字不变 |
| `crates/core/archi/src/drift/mod.rs` | 报告装配与 `status` 推导 | **零改动**：纳管子集筛选（`gating_findings`）、`derive_status` 入参、`passed` 判据逐字不变；本变更后纳管子集仍为空（12 个前端目录元素零契约文件）⇒ `status = clean`、`passed = true` |
| `.dev-team/architecture/models/06-frontend.toml` + `packages/desktop/src/**` 的 12 个入口 | 本仓读数来源 | 12 个目录根元素由 ③ 命中入口 ⇒ ① 面归零；`api` 仍为缺省 `declared` 且零契约文件 ⇒ ② 面 +12；`api = none` 仍恰两处（两个工具 crate），MUST NOT 被用来「清」这 12 条 |
| `crates/core/archi/tests/corpus/drift/**` + `expected/**` | 判定面语料与金样 | **零改动**：十六份金样逐字不变（`check.rs` 零改动、语料内无 barrel）；本仓 12 条平移 MUST NOT 落成 crate 语料 |

### API: 无

本能力不新增命令 / IPC / 判据变体；`api::check` / `gating_findings` / `derive_status` 的签名与语义逐字不变。

### CLI: 无

`api:check` 三列（`missing` / `mismatched` / `orphaned`）应保持全空：12 个入口不产生 `.i.ts`（零契约），`INDEX.toml` 零 diff。

### Component: 无

呈现面（`desktop-archi-contract-panel`）零改动：判据轴仍按 `category` 分组呈现同一份 `apiFindings`，本变更只让 `entry` 分类归零、`contract` 分类由 29 涨到 41。
