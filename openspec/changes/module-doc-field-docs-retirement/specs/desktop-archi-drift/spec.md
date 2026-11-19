# desktop-archi-drift Specification（delta: module-doc-field-docs-retirement）

## Purpose

本 delta 只改报告结构与 `status` 推导的两处口径（E-2）：`apiFindings` 的纳管子集由「契约在案元素」改为「入口文档承诺已成立（declared：契约 md 在盘 / none：`doc` 已声明）」，且子集内任一 `error` 档条目可把 `status` 抬到 `errors`（不再只有 `warnings` 一档）——门禁布尔 `passed` 随之纳入 error 档判据（归 `desktop-archi-api-check` 定义）。`WarningKind`（旧文档警告单值）随 `docs` 三源退役从「互不混用」的词汇对照里移除，`REPORT_VERSION` 仍为 3。另把 `desktop-archi-module-docs` 退役后仍有效的「看护与门禁不读 `specs`」边界以 ADDED requirement 承接过来。

## MODIFIED Requirements

### Requirement: 报告结构与 status 五值

检查 SHALL 产出单一报告结构（增量模式落 `reports/archi/<timestamp>.json`，全量模式仅进程内返回），字段面 SHALL 覆盖：

```jsonc
{
  "version": 3,
  "timestamp": "2026-10-20T08:31:12Z",
  "mode": "incremental",              // incremental | full
  "root": "/abs/path",
  "change": "add-billing-gate",       // 无 change 上下文（full 模式 / 命令面无 change）时为 null
  "base": "HEAD",                     // full 模式为 null（增量恒 HEAD 差）
  "modelHash": "sha256:…",            // models/*.toml 聚合哈希

  "model": {
    "elements": 24, "relations": 31,
    "byGate": { "strict": 8, "warn": 5, "off": 11 },
    "structuralErrors": [ … ],        // 读存量模型遇错也照录
    "warnings": [ … ]                 // pathNotFound / noPath / ambiguousOwner / 等模型面警告
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

- **`status` 五值封闭**：`clean`（有扫描、零 errors 零 warnings 且纳管的契约判据为空）、`errors`（`errors` 非空**或纳管子集含任一 `error` 档判据**）、`warnings`（零 errors，且 `warnings` 非空**或**纳管子集非空且其中无 `error` 档）、`noChanges`（无文件可判）、`skipped`（无模型）。`skipped` 与 `noChanges` MUST NOT 被呈现或消费为「通过」——UI 与门禁日志 SHALL 显式区分。
- **`observations` 与 `warnings` 分界**：`warnings` 是「有问题的依赖」（方向 / 目标 / 可解析性 / 归属不确定），随 `status` 影响提示强度；`observations` 是「设计态陈述」，永不影响 `status`、永不进 `passed`。
- **`occurrences` 聚合**：同元素对的全部命中点聚合在一条记录内，不按行铺开。
- **`suppressed` 必留痕**：豁免不等于不存在——保留被抑制条目 + 理由 + 来源。
- **`modelHash`**：`models/*.toml` 聚合哈希，供消费方判断「手上报告是否对应当前模型」；MUST NOT 被用作缓存失效依据（本期零缓存）。模型字节变化即哈希变化（本变更删 `01-app.toml` 一行 `docs` ⇒ 本仓 `modelHash` 必然变，元素 / 关系计数不变）。
- **词汇封闭且单点**：报告自带**四组**封闭词汇——`status`（五值）、漂移条目 `kind`（`undeclaredDependency` / `pathNotFound` / `ambiguousOwner` / `noPath`）、观察项 `kind`（`unfulfilledDeclaration` / `unusedDeclaration`）、**契约判据 `kind`**（`ArchiApiFindingKind` **十三值**，定义与语义见 `desktop-archi-api-check`「契约判据词汇封闭十三值与开放分类」）；每组 SHALL 在报告结构模块单点定义并附 `ALL` 常量，语料对拍断言「新增变体未同步即红」。报告词汇 MUST NOT 复用既有 DTO 词汇：`desktop-archi-model` 的 `DiagnosticKind`（结构诊断九类）与 `EdgeMark` / `NodeMark`（画布标记）语义面各不相同——混用会让「结构错」「漂移违例」两类在类型上不可辨。**`WarningKind`（旧文档警告，单值 `docNotFound`）已随 `docs` 三源退役删除**：入口文档面的读数改由契约判据的 `docMissing` / `docNotFound` / `apiDocMissing` 承载（`category = doc`），MUST NOT 在报告里恢复「文档警告」这一类别。
- **`version`**：报告结构版本号 SHALL 递增并走显式兼容说明（`version = 1` 无 `apiFindings`；`version = 2` 有该字段但 `status` 推导不读契约判据；`version = 3` 纳管口径进 `status`）。**本变更不递增**：字段面无增删，变的只是判据取值、档位与纳管口径——`version` 仍为 **3**，MUST NOT 因语义收紧而无字段变化地递增（否则消费方按版本分支会读到「形状未变的伪新版本」）。
- **`apiFindings` 为并列槽位**：契约判据结果 SHALL 落本字段（空数组 = 无契约问题），MUST NOT 与 `errors` / `warnings` / `observations` 混装。对 `status` 的影响 SHALL 限于**纳管子集**（口径、判据与单点实现归 `desktop-archi-api-check`「报告并列槽位 apiFindings」）——本变更把该子集的口径由「契约在案元素」改写为「**入口文档承诺已成立**」（`api = declared` 且派生契约 md 在盘 / `api = none` 且 `doc` 已声明），并允许子集内 `error` 档条目把结果抬到 `errors`。`docMissing` 永不纳管 ⇒ 存量入口文档债 MUST NOT 把状态词常规化为永久 `warnings`。条目形状 `{ kind, category, element, target, message, occurrences }`：`occurrences` 复用既有聚合形状，`category` 为开放值集字符串（无 `ALL`，渲染层据此分组）。
- **`passed` 与 `apiFindings` 的关系（本变更改写）**：门禁布尔 `passed` 的判据 SHALL 为 `errors` 空 ∧ `model.structuralErrors` 空 ∧ **`error` 档 `apiFindings` 为空**——原纪律「`apiFindings` MUST NOT 影响 `passed`」**作废**，MUST NOT 在任何 spec / 代码注释里残留（它只在「判据级档位最高到 `warn`」的前提下成立）。
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

- **WHEN** 修改任一 `models/*.toml` 字节后重跑（本变更删 `01-app.toml` 的 `docs` 一行即属此列）
- **THEN** `modelHash` 变化；未修改模型时两次运行的 `modelHash` 相同

#### Scenario: apiFindings 并列而不混装

- **WHEN** 报告同时含 1 条 `warnings` 与 43 条 `apiFindings`（全部 `docMissing`，且这些元素均未写入口文档）
- **THEN** `warnings` 仍只 1 条、`status = warnings`（成因是 `warnings` 非空而非契约判据）、`passed` 不受这 43 条影响；两类条目不互相出现

#### Scenario: 入口文档在案的判据按档位抬 status

- **WHEN** 报告 `errors` 为空、`warnings` 为空，`apiFindings` 含一条属于「契约 md 在案」元素的 `apiSignatureMismatch`（`error` 档）
- **THEN** `status = errors`、`passed = false`；而同元素只含一条 `apiDocMissing`（`warn` 档）时 `status = warnings`、`passed = true`；把该元素的契约 md 移走后两者都退化为 `docMissing` ⇒ `status = clean`、`passed = true`

#### Scenario: 存量入口文档债不进状态词

- **WHEN** 报告含 43 条 `docMissing`（本仓第六段读数）、零 `errors`、零 `warnings`
- **THEN** 纳管子集为空 ⇒ `status = clean`、`passed = true`；MUST NOT 因这 43 条把状态词抬成 `warnings`

#### Scenario: 版本未递增而口径已变

- **WHEN** 分别读取 `version = 3` 的新报告与 `version = 3` 的历史报告
- **THEN** 两者字段面相同；差异只在 `apiFindings` 的取值 / 档位与 `status` 推导结论（历史报告由旧口径产生，不回写、不重算）；消费方 MUST NOT 期望靠 `version` 区分两种口径——本次口径变化由 `desktop-archi-api-check` 的 spec 与 change 留痕承载

## ADDED Requirements

### Requirement: 看护与门禁不读 specs（迁移边界）

架构看护（结构校验 / 图诊断）与门禁结论 SHALL NOT 读取 `openspec/specs/`，SHALL NOT 依赖 `spec ↔ element` 映射：

| # | 边界 | 理由 |
|---|---|---|
| 1 | 结构校验 / 图诊断不读 `specs` | 否则「模块入口文档替代 `specs`」完成那天，看护逻辑要整体改写 |
| 2 | 门禁结论（`passed` / `status`）不依赖 `specs ↔ element` 映射 | 该映射将来由「入口文档 ↔ element」承担，不由 `specs` 承担 |
| 3 | `specs` 不参与扫描语料与判据语料 | 扫描面由 `ExcludeSet` 与源扩展名界定，判据语料只取模型 + 盘上载体 |

- **零读取路径**：产品源码 MUST NOT 出现 `openspec` 路径拼接（路径一律经 `foundation::layout`），看护链 MUST NOT 新增 `specs` 读取、解析或哈希。
- **零映射预铺**：MUST NOT 引入 `spec ↔ element` 映射、MUST NOT 让 `specs` 参与 `apiFindings` / `errors` / `warnings` 的任一条。
- **零替代**：本能力 MUST NOT 为 `specs` 提供第四条读取来源以「顺便保住检索」——ADR 与文档面的替代路径见 `desktop-archi-agent`「检索面纪律与迁移边界」。
- **迁移归独立变更**：「模块入口文档替代 `specs`」的迁移（含 acceptance 可断言结构与归档 `SpecSync` 增量合并语义的对应物缺失问题）需独立变更裁决，本能力零预铺。

#### Scenario: 扫描与判据语料不含 specs

- **WHEN** 以本仓根跑全量检查并审查 `scan.matched` / `scan.unmatchedFiles` / `apiFindings`
- **THEN** `openspec/` 前缀的条目只出现在 `scan.excluded[]`（`rule = "openspec/**"`，见 `repo-archi-model` 的架构域看护配置）；`apiFindings` 内零条目的成因是 `specs` 内容

#### Scenario: 门禁结论与 specs 无关

- **WHEN** 增 / 删 / 改 `openspec/specs/` 下任意文件后重跑
- **THEN** `modelHash` / `errors` / `warnings` / `apiFindings` / `status` / `passed` 逐字段不变（除 `timestamp`）

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/drift/report.rs` | 报告结构与词汇 | `ArchiApiFindingKind` **十三值** + `ALL` + `as_str` + `severity()`（三值档位）；`ArchiDriftReport.api_findings`；`REPORT_VERSION = 3` 不递增；`WarningKind` 相关表述删除 |
| `crates/core/archi/src/drift/mod.rs` | 状态推导 | `check()` 装配 `api_findings` 并取纳管子集；`derive_status` 增加 `errors` 分支（纳管子集含 `error` 档）；`warnings` 分支条件调整为「纳管子集非空且无 `error` 档」；`passed` 判据追加「error 档 `apiFindings` 空」 |
| `crates/infra/archi/src/runner.rs`（`LocalArchiCheck`） | 门禁执行体 | 结论只取报告既有字段；`passed` 的口径变化经 `drift::check` 单点传导，执行体 MUST NOT 自行改写（本文件零逻辑改动，只随同源语义变化） |
| `crates/core/archi/src/drift/config.rs` | 看护配置 | 零改动：`ignore` / `exclude` 两键封闭 + `deny_unknown_fields`；求值顺序写死在 `check.rs`，MUST NOT 引入配置键 |
| `crates/core/archi/tests/golden_test.rs` + `corpus/drift/**` | 报告面语料 | 十六份金样随「首错短路 + 纳管分档」显式重录并逐份留痕成因；`DESKTOP_GOLDEN_REWRITE=1` 显式重写 |
