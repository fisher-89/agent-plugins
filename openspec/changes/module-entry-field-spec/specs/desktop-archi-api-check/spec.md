# desktop-archi-api-check Specification（delta: module-entry-field-spec）

## Purpose

本 delta 只改 `apiEntryMissing` 的**语义与基线数字**，判据种类数（十值封闭）、求值互斥序、档位与纳管口径**逐字不动**：

1. **语义**（§一 / §二）：`apiEntryMissing` = 「该元素没有可用入口」——`Absent`（未声明且无约定默认）或 `Invalid`（形态 / 落位 / 存在 / 跨元素唯一任一违反）同归本值，文案点名理由；`.i` 生成物的存在不再充当入口的替代证据（`has_generated` 退役）。四条判据**挤进同一个值**，MUST NOT 加第 11 个 finding kind。
2. **基线**（§五）：分母仍是 42 个 `path` 非空元素，分桶由 28 + 13 变为 **15 + 26**（总 41，与旧基线总数相等）——13 个前端文件型根元素（12 个单文件根 + `App.Frontend.Shell`）由「猜不到入口（假报）」转为「有门无契约（真话）」；壳与两个工具 crate 仍在 ① 面（其显式 `entry` 数据归宿主重建后的后续变更，`api = none` 不豁免 ①），届时分桶回到 **12 + 27**（总 39）；`status` / `passed` 不受影响。

其余 requirements（报告并列槽位除数字外、验证语料的语料守护两层、契约面启用条件）**逐字不动**。

## MODIFIED Requirements

### Requirement: 契约判据词汇封闭十值与开放分类

契约判据 SHALL 归入**新平行封闭枚举** `ArchiApiFindingKind`（十值，线格式 camelCase），MUST NOT 并入既有 `ArchiDriftKind`（四值）：

| 枚举值 | 线格式词 | 语义 |
|---|---|---|
| `ApiEntryMissing` | `apiEntryMissing` | **该元素没有可用入口**：无候选（未声明 `entry` 且无语言约定默认）**或**候选被判不可用（形态非法 / 落在元素 `path` 之外 / 文件不在盘上 / 与另一元素同一入口）。解析链与四条判据见 `desktop-archi-api`「生成物落点与入口查找」 |
| `ApiMissing` | `apiMissing` | 契约 → 代码缺失：`api = declared` 而契约文件缺失，或契约声明的 op / 类型在代码里不存在。两类缺口 SHALL 用**同一条判据、同一次求值**（存在性判定，签名归 `apiSignatureMismatch`）；共同的排除面三条 = 构造器双向排除 / 透传纯 `$ref` alias 排除 / `x-class` 与 `enum` 计入类型面；结构前提 SHALL 含 `#[path = "..."] mod` 的模块定位与 `pub use <mod>::*` 的通配升格，缺任一条则本判据在自有生成物的元素上恒假阳 |
| `ApiUndeclared` | `apiUndeclared` | 代码有 `external` 面但契约未声明（**唯一完备性判据**） |
| `ApiSignatureMismatch` | `apiSignatureMismatch` | 契约签名与代码签名（归一后）不一致 |
| `ApiUnexpected` | `apiUnexpected` | 留白与其反面矛盾：`api = none` 而契约文件存在 |
| `ApiShadowing` | `apiShadowing` | 父 `$defs` 与子元素 `$defs` 类型名交集非空 |
| `ApiRefMissing` | `apiRefMissing` | 跨模块 `$ref` 目标缺失 / 未 `declared` / 非直接子元素 |
| `ApiNestedCrossCrate` | `apiNestedCrossCrate` | 嵌套元素父子分属不同 crate |
| `ApiReexportDrift` | `apiReexportDrift` | markdown 散文列表与 `api-reexport` fence 键集不等 |
| `ApiConstructorDeclared` | `apiConstructorDeclared` | 契约 `### 操作` 声明了构造器（应由生成器产出） |

- **`apiEntryMissing` 的四种不可用理由 SHALL 挤进同一个值**：入口的形态 / 落位 / 存在 / 跨元素唯一四条判据的违反一律报本值，文案 SHALL 点名具体理由（越界目标 / 缺失文件 / 冲突方 FQN / 形态非法），MUST NOT 只写通用「无入口」。**MUST NOT 新增第 11 个值**（如 `apiEntryInvalid`）：四条判据与本值同族同档同 `category = entry`，新增变体会牵动 `ALL` 长度、四面词汇对拍与金样，收益不足；「无可用入口」这一措辞本身已覆盖四种成因。
- **`.i` 生成物的存在 MUST NOT 作为入口的替代证据**（旧口径「既无生成物也无入口」的两条件判定已退役）：生成物路径是入口的函数，无入口即无生成物路径可派生。
- **`api = none` 不豁免本值**：「无入口」与「无公共 API」是两个不同事实（前者是缺门、后者是明说无 API），故 `api = none` 的元素照常可报 `apiEntryMissing`。
- **分立的理由**：`ArchiDriftKind` 是「模型 vs 代码」的漂移，本枚举是「契约 vs 代码」的漂移——事实源不同（relation 图 vs 契约文件）、定级语义不同（元素 `gate` vs 判据自带 `severity()`）、扩展节奏不同。并入会让既有 `match` 臂被迫处理不相关变体，并牵动既有 corpus 金样与前端 `driftMarks`。
- **`ALL` 与 `as_str` 双向对拍**：`ArchiApiFindingKind::ALL: [Self; 10]` SHALL 与 `as_str` 逐值对拍；新增变体未补入 `ALL`（长度断言）即测试红。
- **`category` SHALL 为开放值集字符串**（MUST NOT 设枚举、MUST NOT 设 `ALL`）：用于报告渲染分组；新增分类只改打标处，不动枚举、不动 `ALL`、不动金样词汇对拍。初始取值集 SHALL 为 `entry`（`apiEntryMissing` / `apiUnexpected`）/ `contract`（`apiMissing` / `apiUndeclared`）/ `signature`（`apiSignatureMismatch`）/ `reexport`（`apiReexportDrift` / `apiShadowing` / `apiRefMissing`）/ `nesting`（`apiNestedCrossCrate`）/ `constructor`（`apiConstructorDeclared`）；拼错的防护靠「单点打标 + 契约测试覆盖已知分类」，MUST NOT 靠类型系统。
- **条目形状**：`ArchiApiFinding { kind, category, element: Option<String>, target: Option<String>, message: String, occurrences: Vec<ArchiDriftOccurrence> }`——`element` 对应既有条目的「引入方」语义、`target` 对应「被引入方」语义，`occurrences` SHALL 复用既有 `ArchiDriftOccurrence` 形状（不另立第二份命中点类型）。

#### Scenario: 十值词汇双向对拍

- **WHEN** 遍历 `ArchiApiFindingKind::ALL` 与 `as_str`
- **THEN** 十项一一对应、线格式词唯一且与语料词一致；从 `ALL` 删掉一项或新增变体未补 `ALL` 时测试红；入口四条判据的违反不新增第 11 值（`ALL` 长度仍为 10）

#### Scenario: 与既有漂移词汇互不混用

- **WHEN** 审查 `ArchiApiFindingKind` 与 `ArchiDriftKind` / `ArchiCheckStatus` / `ArchiObservationKind` 的定义与 `match` 使用点
- **THEN** 四组词汇各自单点定义、互不复用；`ArchiDriftKind` 的 `ALL` 仍为四值、`match` 臂零改动

#### Scenario: 新分类不改枚举

- **WHEN** 给 `apiUnexpected` 换一个 `category` 值（如新增 `config` 分类）
- **THEN** 只改打标处，枚举、`ALL`、词汇对拍与金样零改动；渲染层按新分组聚合

#### Scenario: 四种入口不可用理由各报一条且指名理由

- **WHEN** 分别构造入口形态非法、落在元素 `path` 之外、文件不在盘上、与另一元素同一入口四个元素
- **THEN** 四者各产一条 `apiEntryMissing`（`category = entry`、`warn`），文案分别点名「形态非法」「越界目标」「缺失文件」「冲突方 FQN」；`api = none` 的元素也不因留白被豁免

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
- **`apiMissing` 的声明集 / 导出集口径**：声明集 = 契约 `### 操作` 的 id 集（**构造器双向都不参与比对**——既不在契约声明集，也不计入导出集）；导出集 = 元素的 `external` 代码导出（Rust 经 `pub mod` 链或 `pub use` 白名单可达的 `pub fn` / TS 跨元素被 import 的 `export function`）。签名比对 SHALL 在**归一后**进行（AST 归一单行、参数名与空白不参与）。
- **`apiUndeclared` 只读 `<FQN>.schema.json` 的 `$defs` 键集**，MUST NOT 并读 markdown。安全性由前序门禁保证：`$defs` 键集 = `api-types` ∪ `api-reexport`，而 `apiReexportDrift` 保证 fence 与散文一致、零 diff 门保证 schema 零手改自由度。
- **`apiUnexpected` 单向**：只判「`api = none` 而契约文件存在」（多出来的契约）。反方向（`api = declared` 而契约文件缺失）归 `apiMissing` —— 两条判据 MUST NOT 对同一事实双报，否则报告数无法单调对拍。
- **`apiMissing` 覆盖契约 → 代码的两类缺口**：① 元素 `api = declared` 而其契约文件不存在；② 契约文件在、但其声明的 op / `$defs` 类型在代码导出集里不存在。① 是迁移期主形态（绝大多数元素尚无契约），② 是契约与代码的真实漂移。两类缺口 SHALL 用**同一条判据、同一次求值**（存在性判定，签名归 `apiSignatureMismatch`）；共同的排除面三条 = 构造器双向排除 / 透传纯 `$ref` alias 排除 / `x-class` 与 `enum` 计入类型面；结构前提 SHALL 含 `#[path = "..."] mod` 的模块定位与 `pub use <mod>::*` 的通配升格，缺任一条则本判据在自有生成物的元素上恒假阳。
- **`apiMissing` 天然支持渐进**：契约里只写已确定的 op 即可，未写的 op 归 `apiUndeclared`（关档），不落在 `apiMissing`。
- **`apiConstructorDeclared` 是「契约自身矛盾」的判据**（合法新增判据的范围）：构造器由生成器自动产出，写进契约必然导致 `apiMissing` 误报——试点纸面验证已实测该误报。
- **`apiRefMissing` 的悬空引用在 schema 层由外部校验器编译期捕获**（ajv 对 schema 层的 `$ref` 报 `can't resolve reference`），但 `x-function` 内的 `$ref` **不被解析**（`x-function` 是未知关键字，校验器不下沉）——故函数签名里的跨模块引用只能由本判据与生成器保证。该边界 SHALL 在信心中如实体现：`schema.json` 的定位是「类型层的可校验投影」。
- **零第二份判定**：十条判据 SHALL 只有一处实现（`crates/core/archi/src/api/check.rs`），命令面与门禁面 MUST NOT 各写一份。
- **契约面启用条件（冷启动纪律）**：`<archi_api_dir>` 缺席或其中零份 `*.md` ⇒ 契约面判为**未启用**，`apiFindings` SHALL 为空数组且不产出任何一条契约判据。理由与「无模型即 `skipped`」同款：迁移期仓库与全部既有漂移语料都没有契约资产，若把「`api = declared` 缺契约文件」在此刻全量展开，既有语料的 `apiFindings: []` 与「金样仅追加空槽位」的兼容承诺同时失效。MUST NOT 把该条件做成逃生口：目录一旦存在（在案，试点契约在内），**全部 `path` 非空元素**（本仓 42 个）重新纳入判据，`declared` 缺文件即 `apiMissing` / 无可用入口即 `apiEntryMissing`——不存在「部分元素豁免」的中间态，也不设白名单或开关。

#### Scenario: entry 优先互斥

- **WHEN** 某元素既无可用入口（`Absent`，或候选被判 `Invalid`）、`api = declared` 且其契约声明了 2 个 op
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
- **`warn` 报告即迁移 backlog**：`warn` 模式下的报告输出天然成为迁移待办表（`apiEntryMissing` 每一条都是待定论的任务：补入口声明 / 补约定入口 / 判 `api = none`——三条都是真话，MUST NOT 一律按「去补 barrel」理解），比手写 todo 更可靠（直接来自代码事实）。每消一条，报告数减一，可单调验证。
- **MUST NOT 为消债而重构**：本能力 MUST NOT 要求建 barrel、改元素根路径去扩展名、挪子→父类型依赖或上移壳元素根路径（存量债务只作检查器的验证语料）。唯一例外是**为让契约有效而暴露已存在的私有逻辑**（改可见性 + 挪进 `impl`，不动实现逻辑）——判据不同：前者是「格式追代码」，后者是「代码追上契约应有内容」。入口面同理：`entry` 字段与解析链只让「本来存在的入口」可被表达，MUST NOT 顺带为消 `apiEntryMissing` 而补 12 个前端目录的 barrel（那是独立变更）。

#### Scenario: 元素 gate 全 off 而判据仍报

- **WHEN** 模型内全部元素 `gate = off`（本仓现状）且存在 15 个无可用入口元素
- **THEN** 15 条 `apiEntryMissing` 照常产出（判据级 `warn`），与元素 `gate` 无关；把某元素 `gate` 改成 `strict` 后该 15 条数量与内容零变化

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

- **WHEN** 报告含 15 条 `apiEntryMissing` 与 26 条 `apiMissing`（全部来自**契约不在案**的元素）
- **THEN** 纳管子集为空、`status` 与引入 `apiFindings` 前逐字相同（本仓现状：唯一契约在案元素判据 0 条 ⇒ `status` 不变）；MUST NOT 因这 41 条把状态词常规化为 `warnings`

#### Scenario: 稳定序与聚合

- **WHEN** 同一元素对的三处签名不一致被三处命中
- **THEN** `apiFindings` 内该元素对恰一条，`occurrences` 含 3 项（各带 `file` / `line`）；整体排序为 `(kind, element, target)` 稳定序，两次运行逐字相同

#### Scenario: 命令面与门禁面同源

- **WHEN** 以同一 root 分别经 `archi_drift` 命令与门禁 runner 触发检查
- **THEN** 两份报告的 `apiFindings` 与 `status` 逐字段相等（含排序、`category` 与纳管结论）

### Requirement: 契约判据的验证语料与报告数基线

契约判据的正确性 SHALL 以**报告数逐项对拍**验证（存量债务即验证语料），并 SHALL 有语料守护：

- **基线表（本仓现状，实测复核；分母 = `path` 非空的 42 个元素）**：`apiEntryMissing` **15**（8 个视图目录根 + 3 个视图内子域 + `components/ui`——它们缺 `index.ts` / `index.tsx`，全为 `Absent`；**+ 壳 `App.Backend.Shell` 与两个工具 crate**——三者同样 `Absent`：目录型壳的约定候选 `src-tauri/src/src/lib.rs` 不在盘、二进制 crate 无 `src/lib.rs`）、`apiMissing` **26**（14 个 crate 元素 − 1 个试点 = 13，+ 多文件根 `App.Frontend.Shell` + 12 个前端单文件根元素；后 13 条是入口可解析之后才首次出现的「`declared` 但契约文件缺失」）、其余七条一致性判据各 **0**、`apiUndeclared` 不适用（`off`）。
- **两条切分口径**：① `apiEntryMissing` 只由「无可用入口」产生——`Absent`（未声明且无约定默认）与 `Invalid`（形态 / 落位 / 存在 / 唯一任一违反）同归此项；② 入口一旦可解析，该元素就不再享有 `apiEntryMissing` 的静默，改为承担「有门无契约」的 `apiMissing`。故**总条目数 15 + 26 = 41 与旧基线（28 + 13 = 41）相等**：纯**分桶变化**（13 个前端文件型根元素从 `entry` 面移到 `contract` 面），不是新增债务；`api = none` 不豁免 ①（两个工具 crate 仍在 ① 面）。
- **数据分两批，基线分两段**：壳与两个工具 crate 的显式 `entry` SHALL 归宿主重建后的后续变更（版本耦合理由见 `repo-archi-model` 同名 delta 的首段）。**本段基线 = 15 / 26**；后续变更落地后 SHALL 更新为 **12 / 27**（总 39）：壳与两工具 crate 从 ① 面退役（壳转 `apiMissing`、两工具 crate 因 `api = none` 零判据）。MUST NOT 在本变更内按第二段数字验收。
- **判定口径**：**报告数与基线表逐项相等**即「检查能正确提示」。多于基线 = 判据误报（假阳），少于基线 = 判据漏报（假阴），两者都是检查器缺陷——MUST NOT 以「报告少就是代码干净」自我解释。
- **基线须显式留痕**：基线数字写入 change 报告（`reports/`）与实现期实测对照；数字变更（如新增 crate 使 entry-missing +1、或前端补 barrel 使 `apiEntryMissing` 归零）SHALL 在报告内说明成因，MUST NOT 直接在代码里改期望数。基线表本身**不写进断言**（易随元素增减漂移），断言只锚定「实现期留痕的实测表与报告逐项相等」。
- **语料守护（分两层，各有 owner）**：① **判定面逐判据用例**由 `crates/core/archi/src/api/**` 的十一份 sidecar 用例承担——契约解析（双向 gate 三条）/ `$defs` 规则五条 / 十判据各自正反例 / entry 互斥（含入口四理由）/ 签名归一 / 嵌套与透传（选择性 / 纯 `$ref` / 交集 / 直接子元素 / 跨 crate）/ 生成物渲染两侧键集一致；MUST NOT 另建 `tests/corpus/api/` 一类第二套判定语料（既有 crate 级语料只有 `corpus/dsl/` 与 `corpus/drift/` 两套，多一套即第二个驱动面）。② **crate 级报告面语料**沿既有 `crates/core/archi/tests/corpus/drift/` + `tests/golden_test.rs` 的 `DRIFT_CASES` 登记面驱动，快照为 `expected/<case>.<mode>.json`：既有 `api-mismatch` case（`contracts/App/App.Mod.md` + `models/01-app.toml` + `repo/src/mod/mod.rs`）SHALL 保持非空 `apiFindings`（其目录型根的 `src/mod/mod.rs` 由约定默认 `<root>/mod.rs` 命中，故 `modelHash` 与金样逐字不变）；新增 case 未登记进 `DRIFT_CASES` 即红、登记了但 case 目录不存在亦红。golden 更新 SHALL 沿既有 `DESKTOP_GOLDEN_REWRITE=1` 显式重写并人工确认（快照不参与判定）。
- **MUST NOT 把本仓模型与契约塞进 crate 语料**：本仓 `models/**` 与 `api/**` 是仓库数据资产，纳入 crate 测试语料会破坏「语料自足、可移植」；本仓核对以命令 + 留痕执行（落 change `reports/`）。
- **生成器正确性由自身单测守护**：`verify`（零 diff + 编译器）覆盖不到生成器自身的 bug——渲染、装配、抽取三面各有逐字段断言，MUST NOT 只靠端到端零 diff。

#### Scenario: 契约面未启用时零判据

- **WHEN** 观察根下 `<archi_api_dir>` 不存在（或存在但零份 `*.md`）时跑一次全量检查
- **THEN** `apiFindings = []`（零条任何契约判据），`errors` / `warnings` / `status` 与不加契约面时逐字相同；MUST NOT 产 `apiMissing` / `apiEntryMissing`

#### Scenario: 试点契约落地后全量纳入

- **WHEN** 同一根下加入试点契约（`<archi_api_dir>/Infra/App.Backend.Infra.ArchiRuntime.md`）后重跑
- **THEN** 契约面转为启用，全部 `path` 非空元素（42 个）参与判据：产出基线表所述 15 条 `apiEntryMissing` 与 26 条 `apiMissing`（试点元素自身零命中）；试点元素不因「有契约」而被豁免于其它判据

#### Scenario: 报告数与基线逐项相等

- **WHEN** 以本仓根跑一次全量检查并统计 `apiFindings` 按 `kind` 的分布
- **THEN** `apiEntryMissing = 15`（逐条为 8 个视图目录根 + 3 个视图内子域 + `App.Frontend.Components.Ui` + `App.Backend.Shell` + `App.Backend.Tools.ExportBindings` + `App.Backend.Tools.ArchiApiGen`）、`apiMissing = 26`（13 个 crate + `App.Frontend.Shell` + 12 个前端单文件根，逐条为「declared 但契约文件缺失」）、其余七条各 0、`apiUndeclared = 0`；两个工具 crate 只落 ①（`api = none` 不豁免「无入口」，其显式 `entry` 归后续变更）；`status` 仍为 `clean`（纳管子集为空）

#### Scenario: 假阳与假阴都可辨

- **WHEN** 人为让某入口可解析的元素也被判 `apiEntryMissing`（假阳），或让某无入口元素漏报（假阴）
- **THEN** 前者的报告数变为 16（多于基线）后者变为 14（少于基线）；两种偏离都判为检查器缺陷，MUST NOT 更新基线表放行

#### Scenario: 判定面用例与报告面语料各司其职

- **WHEN** 运行 `crates/core/archi/src/api/**` 的 sidecar 用例与 `crates/core/archi/tests/golden_test.rs` 的 `DRIFT_CASES` 驱动面
- **THEN** 前者逐判据覆盖正反例（判据产出由 `check::check` 直驱断言，不依赖快照）；后者逐 case 驱动 `scan → drift::check → 报告`，每 case 的 `apiFindings` 与 `expected/<case>.<mode>.json` 逐字段一致（含 `kind` / `category` / `element` / `target` / `occurrences` 计数）；`api-mismatch` case 产非空 `apiFindings`（`apiSignatureMismatch`），其余既有金样 `apiFindings` 为空且其余字节不变；MUST NOT 存在第三套判定语料目录

#### Scenario: 新 case 未登记即红

- **WHEN** 新增 `corpus/drift/<case>/` 却未登记进 `DRIFT_CASES`，或登记了而 case 目录不存在，或旧金样未重写而缺 `apiFindings` 键
- **THEN** 三种情形各报红并点名该 case（缺失键 MUST NOT 静默补默认值）；golden 变更只经 `DESKTOP_GOLDEN_REWRITE=1` 显式重写并人工确认

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/drift/report.rs` | 判据词汇与报告槽位 | `ArchiApiFindingKind`（十值 + `ALL` + `as_str` + `severity()`）、`ArchiApiFinding { kind, category, element, target, message, occurrences }`、`ArchiDriftReport.api_findings`、`REPORT_VERSION = 3`（纳管口径入 `status` 推导）；`ApiEntryMissing` 的 doc 语义为「无可用入口」（四理由同值，MUST NOT 加第 11 值） |
| `crates/core/archi/src/api/check.rs` | 十条判据单点求值（经 `api::check` 出口暴露） | `check(ApiCheckInput) -> Vec<ArchiApiFinding>`：① 经 `paths::entry_state`（`Absent` / `Invalid(_)` ⇒ 一条 `apiEntryMissing`，文案按 `EntryProblem` 分列；`Resolved` ⇒ 继续 ②③④）→ 声明集（op **与 `$defs` 类型**）vs 导出集 → 签名归一比对 → 留白矛盾 → 嵌套透传五项；**已删** `has_generated`（`.i` 存在不再充当入口证据）；`$defs` 只读 `schema.json`；零第二份判定 |
| `crates/core/archi/src/api/check_test.rs` | 判据语料 | 十判据正反例 / entry 互斥（含 `Absent` 与四种 `Invalid` 理由各一条）/ 「`.i` 在盘但无入口仍报 ①」/ `api = none` + `entry` 有值零判据 / 签名归一 / 留白双向 / 交集 / 非直接子元素 / 跨 crate / 透传漂移 / `severity()` 与 `ALL` 逐值对拍 |
| `crates/core/archi/tests/corpus/drift/**` + `tests/golden_test.rs` | 判定面语料与 golden | 既有 16 份金样逐字不变（`api-mismatch` 的 `apiSignatureMismatch` 与 `status = warnings` 在案；其目录型根 `src/mod/mod.rs` 由约定默认命中 ⇒ `modelHash` 不变）；`DRIFT_CASES` 登记面即驱动表（未登记 / 目录不存在 / 缺 `apiFindings` 键三种失配皆红） |
| `crates/core/archi/tests/vocabulary_test.rs` | 四面词汇对拍 | `ArchiApiFindingKind::ALL` 与 `as_str` 逐值（长度仍 10）；知识文本须含 `api` 状态词 |
