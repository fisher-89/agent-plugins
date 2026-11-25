# desktop-archi-drift Specification（delta: archi-agent-context-review）

## Purpose

本 delta 只做**引用同步**：报告结构、`derive_status` 五值分档、扫描 / 归属 / 三桶 / 豁免 / 配置面逐字不变。

- 正文里的「契约判据十三值」改十值（含目标 requirement 名）。
- 两处读数由 39 / 36 改为 **38**（= 35 条 `declared` 缺派生 md + 3 条 `api = none` 缺派生 md；现仓：49 个 `path` 非空元素 − 11 个资产齐全元素）。
- 两处 `apiDocMissing` 场景句换为仍存在的 `warn` 档判据（`apiEntryMissing`）；「把契约 md 移走」改判**派生 md 不在盘**（`docMissing` 的单因），「把 schema 移走」在本变更语义下是**零判据**（接口未写不产条目），故该场景句改写为「移走 schema ⇒ 两条判据均消失」。
- `docNotFound` / `apiDocMissing` 两值在 drift 侧的提及随之收口（名字复用彻底结束）。

## MODIFIED Requirements

### Requirement: 报告结构与 status 五值

检查 SHALL 产出单一报告结构（两模式共用同一形状；落盘与否由消费面决定——命令面全量仅进程内返回、门禁面全量 / 增量均落 `reports/archi/<timestamp>.json`），字段面 SHALL 覆盖：

```jsonc
{
  "version": 3,
  "timestamp": "2026-10-20T08:31:12Z",
  "mode": "incremental",              // incremental | full
  "root": "/abs/path",
  "change": "add-billing-gate",       // 无 change 上下文（命令面）时为 null；门禁面两模式均携 change 名
  "base": "HEAD",                     // full 模式为 null（增量恒 HEAD 差，见「全量与增量两模式」）
  "modelHash": "sha256:…",            // graph/model/*.toml 聚合哈希

  "model": {
    "elements": 24, "relations": 31,
    "byGate": { "strict": 8, "warn": 5, "off": 11 },
    "structuralErrors": [ … ],        // 读存量模型遇错也照录
    "warnings": [ … ]                 // pathNotFound / noPath / ambiguousOwner
  },
  "scan": { "mode": …, "changedFiles": 17, "scannedFiles": 12,
            "excluded": [ … ], "unmatchedFiles": [ … ], "matched": [ … ],
            "unresolved": [ … ] },
  "errors":   [ { "kind": "undeclaredDependency", "from": …, "to": …, "gate": …, "occurrences": [ … ] } ],
  "warnings": [ … ],
  "suppressed": [ { "from": …, "to": …, "via": "config|inline", "reason": …, "file": …, "line": … } ],
  "observations": [ { "kind": "unfulfilledDeclaration", "from": …, "to": …, "message": … } ],
  "apiFindings": [ { "kind": "docMissing", "category": "doc", "element": …, "target": null,
                     "message": …, "occurrences": [ … ] } ],   // 契约判据（并列槽位；纳管子集按档位抬 status，error 档进 passed）
  "status": "errors"                  // clean | errors | warnings | noChanges | skipped
}
```

- **`status` 五值封闭**：`clean`（有扫描、零 errors 零 warnings 且纳管的契约判据为空）、`errors`（`errors` 非空**或纳管子集含任一 `error` 档判据**）、`warnings`（零 errors，且 `warnings` 非空**或**纳管子集非空且其中无 `error` 档）、`noChanges`（无文件可判）、`skipped`（无模型）。**`status` 推导与模式无关**：全量模式在零可判文件时同样落 `noChanges`、无模型时同样落 `skipped`。`skipped` 与 `noChanges` MUST NOT 被呈现或消费为「通过」——UI 与门禁日志 SHALL 显式区分，否则「没检查」被误读成「检查过了没问题」。
- **`observations` 与 `warnings` 分界**：`warnings` 是「有问题的依赖」（方向 / 目标 / 可解析性 / 归属不确定），随 `status` 影响提示强度；`observations` 是「设计态陈述」（声明未兑现 / 长期未使用），永不影响 `status`、永不进 `passed`。
- **`observations` 的两种成因不对称（本变更显式豁免）**：`unusedDeclaration` 的两种成因（真没用 / 写错或看不见）不可机械分辨，故该值只作观察项、`message` SHALL 并列两种可能；这是「同一个值 MUST NOT 承载两种缺口」纪律的**唯一豁免点**，理由与补偿见「依赖 API 两桶对账与求差单点」——MUST NOT 在别处援引本条作为放宽口径的先例。
- **`occurrences` 聚合**：同元素对的全部命中点聚合在一条记录内（按 `(from, to)` 聚合，逐条带 `file` / `line` / `specifier` / `typeOnly`），不按行铺开。
- **`suppressed` 必留痕**：豁免不等于不存在——保留被抑制条目 + 理由 + 来源，豁免了什么才可审计。
- **`modelHash`**：`graph/model/*.toml` 聚合哈希（文件名 + 字节累加，不含父目录），供消费方判断「手上报告是否对应当前模型」与历史报告追溯；MUST NOT 被用作缓存失效依据（本期零缓存）。模型字节变化即哈希变化——**更早变更已落地**：`06-frontend.toml` 新增 `Desktop.GeneratedBindings` 一个元素表头、12 条命令面关系的 `to` / `desc` 与 `06` 表头注释字节随之改写（元素计数 54 → 55、关系计数 ±0），本仓 `modelHash` 已在那次落地时变化，该事实 MUST NOT 被写成「模型零变化」或「计数全不变」。**本变更零模型字节改动**（55 元素 / 129 关系、六个文件段计数逐字不动）⇒ 本变更前后两次运行的 `modelHash` 逐字相同，MUST NOT 把历史那次的变化算作本变更的差异面。
- **`scan` 三面按条目级账目验收（本条为条目级验收留痕，防同一误判复发）**：`scan.scannedFiles` / `scan.unmatchedFiles[]` / `scan.excluded[]` 是**事实面的直接投影**，计数随仓内源文件与新增测试 / 语料变化——`gate = off` 元素覆盖的源文件与无主文件同落 `unmatchedFiles[]`（故某个文件由「无主」改为「`off` 元素覆盖」时条目不变，只是成因变了），命中排除规则（默认 13 条 + 配置追加项）的新增文件必进 `excluded[]`（`rule` 记命中的具体形态）。因此「报告只因模型 / 判定面而变」SHALL 按**条目级账目**验收：既有 `excluded[]` 条目（`file` + `rule`）与 `unmatchedFiles[]` 路径零增删改（数组排序带来的位序平移不计），新增条目逐条可指到引入它的那次变更的新增文件与既有规则；MUST NOT 用三面计数相等充当「归属 / 判定面零变化」的证据（`gate` 全 `off` 的仓 `matched[]` 恒空，计数相等根本不含归属信息）。
- **词汇封闭且单点**：报告自带**四组**封闭词汇——`status`（五值）、漂移条目 `kind`（**五值**：`undeclaredDependency` / `undeclaredApi` / `pathNotFound` / `ambiguousOwner` / `noPath`；`ALL` 由四值扩为五值）、观察项 `kind`（`unfulfilledDeclaration` / `unusedDeclaration`，前者由 ① 产出、后者由 ①ᵃ 产出——两个值此前就有，更早变更已点亮后者的产出路径、`ALL` 长度未变）、**契约判据 `kind`**（`ArchiApiFindingKind` **十值**，定义与语义见 `desktop-archi-api-check`「契约判据词汇封闭十值与开放分类」）；每组 SHALL 在报告结构模块单点定义并附 `ALL` 常量，语料对拍断言「新增变体未同步即红」。报告词汇 MUST NOT 复用既有 DTO 词汇：`desktop-archi-model` 的 `DiagnosticKind`（结构诊断九类）与 `EdgeMark` / `NodeMark`（画布标记）语义面各不相同——混用会让「结构错」「漂移违例」两类在类型上不可辨。**`WarningKind`（旧文档警告，单值 `docNotFound`）已随 `docs` 三源退役删除**：文档面的读数改由契约判据的 `docMissing` 承载（`category = doc`）；契约判据侧的 `docNotFound` / `apiDocMissing` 两值随后也随「`doc` 字段惰性化」与「md 去结构化」删除（名字复用到此彻底结束），MUST NOT 在报告里恢复「文档警告」或这两个值。
- **`version`**：报告结构版本号 SHALL 走显式兼容说明（`version = 1` 无 `apiFindings` 字段；`version = 2` 有该字段但 `status` 推导不读契约判据；`version = 3` 纳管口径进 `status`）。**本变更不递增**：字段面无增删，变的只是判据取值、档位与纳管口径——`version` 仍为 **3**，MUST NOT 因语义收紧而无字段变化地递增（否则消费方按版本分支会读到「形状未变的伪新版本」）。**依赖 API 面同样不递增**：`undeclaredApi` 是既有漂移词汇组的取值扩充、`unusedDeclaration` 是既有空位的产出点亮，`errors[]` / `observations[]` 的**条目形状逐字不变**（名字写进 `message`，不新增字段）——故 `version` 仍为 3。
- **`apiFindings` 为并列槽位**：契约判据结果 SHALL 落本字段（空数组 = 无契约问题），MUST NOT 与 `errors` / `warnings` / `observations` 混装——三者事实源与定级语义各不相同（模型关系图 / 可解析性与归属 / 设计态陈述 vs 契约文件）。对 `status` 的影响 SHALL 限于**纳管子集**（口径、判据与单点实现归 `desktop-archi-api-check`「报告并列槽位 apiFindings」）——本变更把该子集的口径由「契约在案元素」改写为「**接口承诺已成立**」（`api = declared` ∧ `<FQN>.schema.json` 在盘；文档在盘与否**退出纳管口径**），并允许子集内 `error` 档条目把结果抬到 `errors`。`docMissing` 恒 `warn` 且两侧都不成立 ⇒ 永不纳管，存量文档债 MUST NOT 把状态词常规化为永久 `warnings`。条目形状 `{ kind, category, element, target, message, occurrences }`：`occurrences` 复用既有聚合形状，`category` 为开放值集字符串（无 `ALL`，渲染层据此分组）。
- **`passed` 与 `apiFindings` 的关系（本变更改写）**：门禁布尔 `passed` 的判据 SHALL 为 `errors` 空 ∧ `model.structuralErrors` 空 ∧ **`error` 档 `apiFindings` 为空**——原纪律「`apiFindings` MUST NOT 影响 `passed`」**作废**，MUST NOT 在任何 spec / 代码注释里残留（它只在「判据级档位最高到 `warn`」的前提下成立）。`unusedDeclaration` MUST NOT 进 `passed`（观察项）。
- **推导序不变**：`skipped` → `errors`（结构错 / `errors[]` 非空 / 纳管子集含 `error` 档）→ `noChanges`（可判文件为 0）→ `warnings`（`warnings[]` 非空 或 纳管子集非空且无 `error` 档）→ `clean`；按此序 `status == errors ⟺ passed == false` SHALL 恒成立（`error` 档判据的触发条件都蕴含纳管成立，故不存在「打断 `passed` 却不进 `status`」的旁路）。

#### Scenario: 五值可辨

- **WHEN** 分别以「零模型」「无变更文件」「零违例」「仅 warn 档违例」「存在 strict 档违例」构造五次检查
- **THEN** `status` 依次为 `skipped` / `noChanges` / `clean` / `warnings` / `errors`；`skipped` 与 `noChanges` 的报告 `errors` 均为空且不带任何「通过」语义字段

#### Scenario: 观察项不影响结论

- **WHEN** 报告含 3 条 `observations` 与零 `errors`
- **THEN** `status = clean`（或 `warnings` 仅当其 `warnings` 非空或纳管子集非空）、`passed = true`；`observations` 不进门禁判据

#### Scenario: 报告词汇与 DTO 词汇互不混用

- **WHEN** 审查报告 `kind` 词汇与 `desktop-archi-model` 的 `DiagnosticKind` / `EdgeMark` / `NodeMark`
- **THEN** 各词汇组各自单点定义、互不复用；报告结构模块为 `status`、漂移条目、观察项与**契约判据**各持一份 `ALL` 常量（`ArchiCheckStatus` / `ArchiDriftKind` / `ArchiObservationKind` / `ArchiApiFindingKind`），语料对拍覆盖全部取值；`WarningKind` 不在任何一组内（已删除）

#### Scenario: modelHash 随模型变化

- **WHEN** 修改任一 `graph/model/*.toml` 字节后重跑（历史实证：更早变更在 `06` 新增一个元素表头、改 12 条关系的 `to` / `desc` 与 `06` 表头注释，即属此列；本变更零模型字节改动，不触发本场景的 WHEN）
- **THEN** `modelHash` 变化；未修改模型时两次运行的 `modelHash` 相同（本变更前后两次运行即属此列）

#### Scenario: apiFindings 并列而不混装

- **WHEN** 报告同时含 1 条 `warnings` 与 38 条 `apiFindings`（全部 `docMissing`，且这些元素均无派生文档）
- **THEN** `warnings` 仍只 1 条、`status = warnings`（成因是 `warnings` 非空而非契约判据）、`passed` 不受这 38 条影响；两类条目不互相出现

#### Scenario: schema 在案的判据按档位抬 status

- **WHEN** 报告 `errors` 为空、`warnings` 为空，`apiFindings` 含一条属于「schema 在案」元素的 `apiSignatureMismatch`（`error` 档）
- **THEN** `status = errors`、`passed = false`；而同元素只含一条 `apiEntryMissing`（`warn` 档）时 `status = warnings`、`passed = true`；把**前一例**元素的 schema 移走后该元素**零判据**（接口未写不产条目；派生 md 若在盘则 `docMissing` 也不成立）⇒ `status = clean`、`passed = true`（`apiEntryMissing` 与 schema 无关，后一例移走 schema 后仍成立）

#### Scenario: 存量文档债不进状态词

- **WHEN** 报告含 38 条 `docMissing`（本仓当前读数：35 条 `declared` 缺派生 md + 3 条 `api = none` 缺派生 md）、零 `errors`、零 `warnings`
- **THEN** 纳管子集为空 ⇒ `status = clean`、`passed = true`；MUST NOT 因这 38 条把状态词抬成 `warnings`

#### Scenario: 版本未递增而口径已变

- **WHEN** 分别读取 `version = 3` 的新报告与 `version = 3` 的历史报告
- **THEN** 两者字段面相同；差异只在 `apiFindings` 的取值 / 档位与 `status` 推导结论（历史报告由旧口径产生，不回写、不重算）；消费方 MUST NOT 期望靠 `version` 区分两种口径——本次口径变化由 `desktop-archi-api-check` 的 spec 与 change 留痕承载

#### Scenario: 依赖 API 面不新增报告字段

- **WHEN** 审查含 `undeclaredApi` 与 `unusedDeclaration` 的报告 JSON
- **THEN** `errors[]` / `observations[]` 的条目键集与不含二者时逐字相同（`kind` / `from` / `to` / `gate` / `message` / `occurrences`）；多出的名字只出现在 `message` 内；`version` 仍为 3

#### Scenario: 全量报告形状与增量同构

- **WHEN** 取一份 initialize 基线全量报告与一份 implement 增量报告
- **THEN** 两者字段集完全一致（无「全量独有 / 增量独有」字段），差异只在字段值：`mode`（`full` / `incremental`）、`base`（`null` / `HEAD`）、`scan.changedFiles`（`0` / 变更数）与扫描清单

#### Scenario: 无模型与零可判文件两态可辨

- **WHEN** 分别以「模型目录缺席」与「模型在但零源文件通过排除约定」的观察根执行全量检查
- **THEN** 前者 `status = skipped`、后者 `status = noChanges`；两者 `errors` 皆空但 MUST NOT 呈 `clean`，消费方（门禁 / UI）按各自文案分列「未检查（无模型）」与「未检查（无可判文件）」


## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/drift/report.rs` | 报告结构与四组词汇单点（**本变更只随判据词汇收缩**） | `ArchiDriftReport` 既有 14 字段零改动 + `api_findings: Vec<ArchiApiFinding>`（线格式 `apiFindings`）；`REPORT_VERSION` = 3 不递增；四组 `ALL` 常量（`ArchiCheckStatus` / `ArchiDriftKind` / `ArchiObservationKind` / `ArchiApiFindingKind`）；**`ArchiApiFindingKind`（十值 + `ALL` + `as_str` + `severity()`）**、`ApiFindingSeverity`（三值）、`ArchiApiFinding { kind, category, element, target, message, occurrences }`；`ArchiCheckStatus` 五值封闭；`WarningKind` 相关表述删除 |
| `crates/core/archi/src/drift/mod.rs` | 判据装配与 `status` 推导 | `api_findings` 经 `api::findings` 单点；`derive_status` 五值分档逐字不变；纳管子集口径换锚（`declared` ∧ schema 在盘）归 `desktop-archi-api-check`，本能力只保证「纳管子集非空 ⇒ 至少 `warnings`；含 `Error` ⇒ `errors`」的推导不变 |
| `crates/core/archi/src/drift/{report,mod}_test.rs` | 语料 | 十三值断言改十值；`status` / `passed` 的五值分档语料逐字保留；新增「`api = none` 带 schema 不纳管」一例 |

### 零改动面（同批断言）

| 对象 | 理由 |
|---|---|
| 扫描 / 归属 / 三桶 / 豁免 / 配置（`scan` / `owner` / `config`） | 本变更不动代码与模型面 |
| `message` 之外的报告字段与形状 | 只有 `docMissing` 的 `message` 文案随口径换面改写（38 条），字段面零增删 |
| `reports/archi/**` 运行产物 | 门禁运行产物，不入库、不作判红面 |
