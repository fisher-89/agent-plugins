# desktop-archi-drift Specification（delta: api-gen-check-in-desktop）

## Purpose

本 delta 只改一处 requirement（报告结构与 `status` 五值）：报告 `version` 2 → 3，`status` 的 `warnings` 定义多一个条件（**契约在案元素**的判据非空亦为 `warnings`），`apiFindings` 由「不进 `status`」改为「经纳管子集影响 `status`、仍不影响 `passed`」。扫描面 / 归属 / 定级 / 豁免 / `occurrences` 聚合 / 架构域配置 / 判定单源 / 运行期预算七条 requirement 逐字不变。

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
                     "message": …, "occurrences": [ … ] } ],   // 契约判据（并列槽位；经纳管子集影响 status，不进 passed）
  "status": "errors"                  // clean | errors | warnings | noChanges | skipped
}
```

- **`status` 五值封闭**：`clean`（有扫描、零 errors 零 warnings 且纳管的契约判据为空）、`errors`（`errors` 非空）、`warnings`（零 errors，且 `warnings` 非空**或**契约在案元素的判据非空）、`noChanges`（无文件可判）、`skipped`（无模型）。`skipped` 与 `noChanges` MUST NOT 被呈现或消费为「通过」——UI 与门禁日志 SHALL 显式区分，否则「没检查」被误读成「检查过了没问题」。
- **`observations` 与 `warnings` 分界**：`warnings` 是「有问题的依赖」（方向 / 目标 / 可解析性 / 归属不确定），随 `status` 影响提示强度；`observations` 是「设计态陈述」（声明未兑现 / 长期未使用），永不影响 `status`、永不进 `passed`。
- **`occurrences` 聚合**：同元素对的全部命中点聚合在一条记录内（按 `(from, to)` 聚合，逐条带 `file` / `line` / `specifier` / `typeOnly`），不按行铺开。
- **`suppressed` 必留痕**：豁免不等于不存在——保留被抑制条目 + 理由 + 来源，豁免了什么才可审计。
- **`modelHash`**：`models/*.toml` 聚合哈希，供消费方判断「手上报告是否对应当前模型」与历史报告追溯；MUST NOT 被用作缓存失效依据（本期零缓存）。
- **词汇封闭且单点**：报告自带**四组**封闭词汇——`status`（五值）、漂移条目 `kind`（`undeclaredDependency` / `pathNotFound` / `ambiguousOwner` / `noPath`）、观察项 `kind`（`unfulfilledDeclaration` / `unusedDeclaration`）、**契约判据 `kind`**（`ArchiApiFindingKind` 十值，定义与语义见 `desktop-archi-api-check`「契约判据词汇封闭十值与开放分类」）；每组 SHALL 在报告结构模块单点定义并附 `ALL` 常量，语料对拍断言「新增变体未同步即红」。报告词汇 MUST NOT 复用既有 DTO 词汇：`desktop-archi-model` 的 `DiagnosticKind`（结构诊断九类）、`EdgeMark` / `NodeMark`（画布标记）与 `WarningKind`（文档警告，单值 `docNotFound`）语义面各不相同——混用会让「结构错」「文档警告」「漂移违例」三类在类型上不可辨。
- **`version`**：报告结构版本号，字段演进 SHALL 递增并走显式兼容说明（`version = 1` 无 `apiFindings` 字段；`version = 2` 有该字段但 `status` 推导不读契约判据；本变更把纳管口径引入推导，故递增为 3，消费方按版本分支）。
- **`apiFindings` 为并列槽位**：契约判据结果 SHALL 落本字段（空数组 = 无契约问题），MUST NOT 与 `errors` / `warnings` / `observations` 混装——三者事实源与定级语义各不相同（模型关系图 / 可解析性与归属 / 设计态陈述 vs 契约文件）。本字段 MUST NOT 影响 `passed`（与 `observations` 同款纪律）；对 `status` 的影响 SHALL 限于**契约在案元素的判据**（纳管子集，口径、判据与单点实现归 `desktop-archi-api-check`「报告并列槽位 apiFindings」），MUST NOT 让存量 backlog 把状态词常规化为永久 `warnings`。条目形状 `{ kind, category, element, target, message, occurrences }`：`occurrences` 复用既有聚合形状，`category` 为开放值集字符串（无 `ALL`，渲染层据此分组）。
- **推导序不变**：`skipped` → `errors`（结构错 / `errors[]` 非空）→ `noChanges`（可判文件为 0）→ `warnings`（`warnings[]` 非空 或 纳管子集非空）→ `clean`；按此序 `status == errors ⟺ passed == false` SHALL 恒成立（纳管只可能把结果从 `clean` 抬到 `warnings`，永不产生 `errors`）。

#### Scenario: 五值可辨

- **WHEN** 分别以「零模型」「无变更文件」「零违例」「仅 warn 档违例」「存在 strict 档违例」构造五次检查
- **THEN** `status` 依次为 `skipped` / `noChanges` / `clean` / `warnings` / `errors`；`skipped` 与 `noChanges` 的报告 `errors` 均为空且不带任何「通过」语义字段

#### Scenario: 观察项不影响结论

- **WHEN** 报告含 3 条 `observations` 与零 `errors`
- **THEN** `status = clean`（或 `warnings` 仅当其 `warnings` 非空或纳管子集非空）、`passed = true`；`observations` 不进门禁判据

#### Scenario: 报告词汇与 DTO 词汇互不混用

- **WHEN** 审查报告 `kind` 词汇与 `desktop-archi-model` 的 `DiagnosticKind` / `WarningKind` / `EdgeMark` / `NodeMark`
- **THEN** 各词汇组各自单点定义、互不复用；报告结构模块为 `status`、漂移条目、观察项与**契约判据**各持一份 `ALL` 常量（`ArchiCheckStatus` / `ArchiDriftKind` / `ArchiObservationKind` / `ArchiApiFindingKind`），语料对拍覆盖全部取值（新增值未同步即红）

#### Scenario: modelHash 随模型变化

- **WHEN** 修改任一 `models/*.toml` 字节后重跑
- **THEN** `modelHash` 变化；未修改模型时两次运行的 `modelHash` 相同

#### Scenario: apiFindings 并列而不混装

- **WHEN** 报告同时含 1 条 `warnings` 与 28 条 `apiFindings`（全部 `apiEntryMissing`，且这些元素的契约文件均不在案）
- **THEN** `warnings` 仍只 1 条、`status = warnings`（成因是 `warnings` 非空而非契约判据）、`passed` 不受 `apiFindings` 影响；两类条目不互相出现

#### Scenario: 契约在案判据把 status 抬到 warnings

- **WHEN** 报告 `errors` 为空、`warnings` 为空，且 `apiFindings` 含一条属于契约在案元素的 `apiSignatureMismatch`
- **THEN** `status = warnings`（纳管子集非空）；该元素契约文件移走后同一场景 `status = clean`；两次 `passed` 均为 `true`

#### Scenario: 版本递增与旧报告兼容

- **WHEN** 分别读取 `version = 3` 的新报告、`version = 2` 与 `version = 1` 的历史报告
- **THEN** `version = 3` 含 `apiFindings` 且 `status` 已含纳管结论；`version = 2` 含该字段但 `status` 不含契约判据的影响；`version = 1` 无该字段；消费方按 `version` 分支，MUST NOT 对旧报告补默认值

## Module Contract

### API: `crates/core/archi/src/drift/{mod,report}.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `REPORT_VERSION` | 2 → 3 | 兼容说明三点（1 无字段 / 2 有字段但 `status` 不读判据 / 3 纳管进 `status`） |
| `derive_status` | 改 | 新增纳管子集入参；`warnings` 分支扩一个「或」条件；其余四值序与 `passed` 派生面零改动 |
| `drift::check` | 改 | `api_findings` 与纳管子集各自装配；报告字段 / 排序 / 语料面零改动 |
| 扫描 / 归属 / 定级 / 豁免 / 配置 / 单源 / 预算 | 零改动 | 七条既有 requirement 逐字不变 |

### CLI / 语料

| 项 | 变化 | 描述 |
|---|---|---|
| `tests/corpus/drift/expected/*.json` | 16 份重写 | `version` 2 → 3；`api-mismatch.full.json` 的 `status` 由 `clean` 变 `warnings`（契约在案判据纳管），其余 15 份除 `version` 外逐字不变 |
| `tests/corpus/drift/api-mismatch/models/01-app.toml` | 注释随动 | 文件头「契约面 MUST NOT 进 `status` 推导 …… `status = clean`」改为纳管口径（`status = warnings`） |
