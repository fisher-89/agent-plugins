# desktop-archi-drift Specification（delta: module-api-contract-format）

## MODIFIED Requirements

### Requirement: 报告结构与 status 五值

检查 SHALL 产出单一报告结构（增量模式落 `reports/archi/<timestamp>.json`，全量模式仅进程内返回），字段面 SHALL 覆盖：

```jsonc
{
  "version": 2,
  "timestamp": "2026-10-20T08:31:12Z",
  "mode": "incremental",              // incremental | full
  "root": "/abs/path",
  "change": "add-billing-gate",       // 无 change 上下文（full 模式 / 命令面无 change）时为 null
  "base": "HEAD",                     // full 模式为 null（增量恒 HEAD 差，见「全量与增量两模式」）
  "modelHash": "sha256:…",            // models/*.toml 聚合哈希

  "model": {
    "elements": 24, "relations": 31,
    "byGate": { "strict": 8, "warn": 5, "off": 11 },
    "structuralErrors": [ … ],        // 读存量模型遇错也照录
    "warnings": [ … ]                 // pathNotFound / noPath / docNotFound
  },
  "scan": { "mode": …, "changedFiles": 17, "scannedFiles": 12,
            "excluded": [ … ], "unmatchedFiles": [ … ], "matched": [ … ],
            "unresolved": [ … ] },
  "errors":   [ { "kind": "undeclaredDependency", "from": …, "to": …, "gate": …, "occurrences": [ … ] } ],
  "warnings": [ … ],
  "suppressed": [ { "from": …, "to": …, "via": "config|inline", "reason": …, "file": …, "line": … } ],
  "observations": [ { "kind": "unfulfilledDeclaration", "from": …, "to": …, "message": … } ],
  "apiFindings": [ { "kind": "apiEntryMissing", "category": "entry", "element": …, "target": null,
                     "message": …, "occurrences": [ … ] } ],   // 契约判据（并列槽位，不影响 status）
  "status": "errors"                  // clean | errors | warnings | noChanges | skipped
}
```

- **`status` 五值封闭**：`clean`（有扫描、零 errors 零 warnings）、`errors`（`errors` 非空）、`warnings`（零 errors、`warnings` 非空）、`noChanges`（无文件可判）、`skipped`（无模型）。`skipped` 与 `noChanges` MUST NOT 被呈现或消费为「通过」——UI 与门禁日志 SHALL 显式区分，否则「没检查」被误读成「检查过了没问题」。
- **`observations` 与 `warnings` 分界**：`warnings` 是「有问题的依赖」（方向 / 目标 / 可解析性 / 归属不确定），随 `status` 影响提示强度；`observations` 是「设计态陈述」（声明未兑现 / 长期未使用），永不影响 `status`、永不进 `passed`。
- **`occurrences` 聚合**：同元素对的全部命中点聚合在一条记录内（按 `(from, to)` 聚合，逐条带 `file` / `line` / `specifier` / `typeOnly`），不按行铺开。
- **`suppressed` 必留痕**：豁免不等于不存在——保留被抑制条目 + 理由 + 来源，豁免了什么才可审计。
- **`modelHash`**：`models/*.toml` 聚合哈希，供消费方判断「手上报告是否对应当前模型」与历史报告追溯；MUST NOT 被用作缓存失效依据（本期零缓存）。
- **词汇封闭且单点**：报告自带**四组**封闭词汇——`status`（五值）、漂移条目 `kind`（`undeclaredDependency` / `pathNotFound` / `ambiguousOwner` / `noPath`）、观察项 `kind`（`unfulfilledDeclaration` / `unusedDeclaration`）、**契约判据 `kind`**（`ArchiApiFindingKind` 十值，定义与语义见 `desktop-archi-api-check`「契约判据词汇封闭十值与开放分类」）；每组 SHALL 在报告结构模块单点定义并附 `ALL` 常量，语料对拍断言「新增变体未同步即红」。报告词汇 MUST NOT 复用既有 DTO 词汇：`desktop-archi-model` 的 `DiagnosticKind`（结构诊断九类）、`EdgeMark` / `NodeMark`（画布标记）与 `WarningKind`（文档警告，单值 `docNotFound`）语义面各不相同——混用会让「结构错」「文档警告」「漂移违例」三类在类型上不可辨。
- **`version`**：报告结构版本号，字段演进 SHALL 递增并走显式兼容说明（本变更新增并列槽位 `apiFindings`，故由 1 递增为 2；`version = 1` 的报告无该字段，消费方按版本分支）。
- **`apiFindings` 为并列槽位（本变更新增）**：契约判据结果 SHALL 落本字段（空数组 = 无契约问题），MUST NOT 与 `errors` / `warnings` / `observations` 混装——三者事实源与定级语义各不相同（模型关系图 / 可解析性与归属 / 设计态陈述 vs 契约文件）。本字段首期 MUST NOT 影响 `status` / `passed`（与 `observations` 同款纪律），其严重度由判据级 `severity()` 决定（见 `desktop-archi-api-check`「判据级档位与元素 gate 解耦」）。条目形状 `{ kind, category, element, target, message, occurrences }`：`occurrences` 复用既有聚合形状，`category` 为开放值集字符串（无 `ALL`，渲染层据此分组）。

#### Scenario: 五值可辨

- **WHEN** 分别以「零模型」「无变更文件」「零违例」「仅 warn 档违例」「存在 strict 档违例」构造五次检查
- **THEN** `status` 依次为 `skipped` / `noChanges` / `clean` / `warnings` / `errors`；`skipped` 与 `noChanges` 的报告 `errors` 均为空且不带任何「通过」语义字段

#### Scenario: 观察项不影响结论

- **WHEN** 报告含 3 条 `observations` 与零 `errors`
- **THEN** `status = clean`（或 `warnings` 仅当其 `warnings` 非空）、`passed = true`；`observations` 不进门禁判据

#### Scenario: 报告词汇与 DTO 词汇互不混用

- **WHEN** 审查报告 `kind` 词汇与 `desktop-archi-model` 的 `DiagnosticKind` / `WarningKind` / `EdgeMark` / `NodeMark`
- **THEN** 各词汇组各自单点定义、互不复用；报告结构模块为 `status`、漂移条目、观察项与**契约判据**各持一份 `ALL` 常量（`ArchiCheckStatus` / `ArchiDriftKind` / `ArchiObservationKind` / `ArchiApiFindingKind`），语料对拍覆盖全部取值（新增值未同步即红）

#### Scenario: modelHash 随模型变化

- **WHEN** 修改任一 `models/*.toml` 字节后重跑
- **THEN** `modelHash` 变化；未修改模型时两次运行的 `modelHash` 相同

#### Scenario: apiFindings 并列而不混装

- **WHEN** 报告同时含 1 条 `warnings` 与 28 条 `apiFindings`（全部 `apiEntryMissing`）
- **THEN** `warnings` 仍只 1 条、`status = warnings`（`apiFindings` 不参与推导）、`passed` 不受 `apiFindings` 影响；两类条目不互相出现

#### Scenario: 版本递增与旧报告兼容

- **WHEN** 分别读取 `version = 2` 的新报告与 `version = 1` 的历史报告
- **THEN** 前者含 `apiFindings` 字段（可为空数组），后者无该字段；消费方按 `version` 分支，MUST NOT 对旧报告补默认值

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/drift/report.rs` | 报告结构与四组词汇单点 | `ArchiDriftReport` 既有 14 字段零改动 + 新增 `api_findings: Vec<ArchiApiFinding>`；`REPORT_VERSION` 1 → 2；四组 `ALL`（`ArchiCheckStatus` / `ArchiDriftKind` / `ArchiObservationKind` / `ArchiApiFindingKind`） |
| `crates/core/archi/src/drift/mod.rs` | 报告装配单点 | `check()` 内调 `api::check` 装配 `api_findings`；`status` 推导与 `passed` 派生面 MUST NOT 读 `api_findings` |
| `crates/core/archi/src/drift/report_test.rs` | 报告面测试 | `apiFindings` 空 / 非空两态、`version = 2`、既有 14 字段逐字不变、JSON 往返、稳定序 |
| `crates/core/archi/tests/corpus/drift/expected/*.json` | 既有金样 | 仅追加 `"apiFindings": []`（其余字节不变）+ 至少一个非空 `apiFindings` 金样；沿 `DESKTOP_GOLDEN_REWRITE=1` 显式重写 |
