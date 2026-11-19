# desktop-archi-drift Specification（delta: init-check-archi）

## MODIFIED Requirements

### Requirement: 全量与增量两模式与观察根

检查 SHALL 有两种模式，二者共用同一判定实现与同一报告结构：

```
全量模式（workspace 级）
  缺省基准 = 无（当前工作区就是全部）
  扫描 = 整个观察根的源文件（遍历 + 排除约定）
  产出 = ①②④ 全部
  落盘 = 由消费面决定：命令面（archi_drift）不落盘（一次可重算的观测）；
         门禁面（initialize 基线检查）落盘 reports/archi/<timestamp>.json（审计面）
  用途 = 架构全景 / ratchet 决策 / 一次性体检 / 变更流程启动前的基线绿门

增量模式（change 级）
  基准 = 本 change 编辑集（`ArchiveVcsPort::change_edit_set(root, "HEAD")`，worktree 语义下即本 change 改动；唯一入口，MUST NOT 自算 diff）
  扫描 = 只扫变更集内的源文件
  产出 = ②④；① 无基准 → 只作观察项
  落盘 = 是（`reports/archi/<timestamp>.json`，门禁审计面）
```

- **模式由消费面声明**：判定纯函数收模式入参（`CheckInput.mode`），MUST NOT 按变更集是否为空、按相位名或按调用方类型推断。门禁面经 port 的 `ArchiCheckMode { Full, Incremental }` 声明（映射见 desktop-archi-gate「ArchiCheck runner port 与报告落盘」）：initialize 表头恒 `full`、implement 恒 `incremental`。
- **落盘者恒为调用方**：判定域本身（`crates/core/archi`）**零写盘**（两种模式同）——全量报告是否落盘不是判定语义，而是消费面的审计需要；落盘实现单点驻 `crates/infra/archi`（报告文件名与目录派生见 desktop-archi-gate）。
- **观察根显式**：命令签名 SHALL 带 `root`，由调用方给定（workspace 级 = 主仓根、change 级 = `detail.worktree`，归档 change 回落主仓根；initialize 基线检查 = 该 change 的 worktree）；MUST NOT 引入「当前选中 change」一类隐式上下文。
- **模型与源码同根**：模型 SHALL 从同一 `root` 派生读盘（架构域路径经 `foundation::layout`），MUST NOT 出现「模型读主仓、源码读 worktree」的混根。
- **一致性**：同一 commit、同一模型下，增量报告 SHALL 为全量报告在变更集上的子集（同一元素对不因模式不同而级别不同）。
- **零改代码面**：两种模式 MUST NOT 写源码或模型（判定域零写盘；唯一写动作是消费面对报告的落盘）。

#### Scenario: 增量只覆盖变更文件

- **WHEN** change 编辑集含 12 个源文件（其中 5 个被排除约定排除），执行增量检查
- **THEN** `scan.changedFiles = 12`、`scan.scannedFiles = 5`、`scan.excluded[]` 含那 5 条；未变更文件的违例不出现

#### Scenario: 全量由门禁面落盘

- **WHEN** 以同一 root 分别触发一次命令面全量检查（`archi_drift(root, "full")`）与一次 initialize 基线门禁全量检查
- **THEN** 两次判定结果逐字段相同（`mode` / `scan` / `errors` / `warnings` / `status` 全等，除 `timestamp` / `change`）；命令面零写盘，门禁面在 `reports/archi/` 落一份 `mode = "full"` 的报告

#### Scenario: 两模式级别一致

- **WHEN** 同一 commit 上分别跑全量与增量，且两者都命中了同一元素对 `App.Billing → App.Auth`
- **THEN** 两报告中该元素对级别相同（同为 `errors` 或同为 `warnings`），仅 `mode` / `changedFiles` / `base` 与扫描清单不同

#### Scenario: 无变更文件不算通过

- **WHEN** 增量模式下变更集内无源文件（如只改了文档）
- **THEN** 报告 `status = noChanges`（MUST NOT 为 `clean`），扫描计数为零、`errors` 为空；该形态正是「initialize 不能复用增量口径」的判据（基线时刻变更集只含本 change 的未跟踪文档与报告树，见 desktop-archi-gate「门禁挂载点」）

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

- **`status` 五值封闭**：`clean`（有扫描、零 errors 零 warnings 且纳管的契约判据为空）、`errors`（`errors` 非空）、`warnings`（零 errors，且 `warnings` 非空**或**契约在案元素的判据非空）、`noChanges`（无文件可判）、`skipped`（无模型）。**`status` 推导与模式无关**：全量模式在零可判文件时同样落 `noChanges`、无模型时同样落 `skipped`——`skipped` 与 `noChanges` MUST NOT 被呈现或消费为「通过」，UI 与门禁日志 SHALL 显式区分，否则「没检查」被误读成「检查过了没问题」。
- **`observations` 与 `warnings` 分界**：`warnings` 是「有问题的依赖」（方向 / 目标 / 可解析性 / 归属不确定），随 `status` 影响提示强度；`observations` 是「设计态陈述」（声明未兑现 / 长期未使用），永不影响 `status`、永不进 `passed`。
- **`occurrences` 聚合**：同元素对的全部命中点聚合在一条记录内（按 `(from, to)` 聚合，逐条带 `file` / `line` / `specifier` / `typeOnly`），不按行铺开。
- **`suppressed` 必留痕**：豁免不等于不存在——保留被抑制条目 + 理由 + 来源，豁免了什么才可审计。
- **`modelHash`**：`models/*.toml` 聚合哈希，供消费方判断「手上报告是否对应当前模型」与历史报告追溯；MUST NOT 被用作缓存失效依据（本期零缓存）。
- **词汇封闭且单点**：报告自带**四组**封闭词汇——`status`（五值）、漂移条目 `kind`（`undeclaredDependency` / `pathNotFound` / `ambiguousOwner` / `noPath`）、观察项 `kind`（`unfulfilledDeclaration` / `unusedDeclaration`）、**契约判据 `kind`**（`ArchiApiFindingKind` 十值，定义与语义见 `desktop-archi-api-check`「契约判据词汇封闭十值与开放分类」）；每组 SHALL 在报告结构模块单点定义并附 `ALL` 常量，语料对拍断言「新增变体未同步即红」。报告词汇 MUST NOT 复用既有 DTO 词汇：`desktop-archi-model` 的 `DiagnosticKind`（结构诊断九类）、`EdgeMark` / `NodeMark`（画布标记）与 `WarningKind`（文档警告，单值 `docNotFound`）语义面各不相同——混用会让「结构错」「文档警告」「漂移违例」三类在类型上不可辨。
- **`version`**：报告结构版本号，字段演进 SHALL 递增并走显式兼容说明（`version = 1` 无 `apiFindings` 字段；`version = 2` 有该字段但 `status` 推导不读契约判据；`version = 3` 起把纳管口径引入推导，消费方按版本分支）。**本变更零字段演进**：两模式落盘只改「谁落盘」与「落哪棵树」，`version` 仍为 3。

#### Scenario: 全量报告形状与增量同构

- **WHEN** 取一份 initialize 基线全量报告与一份 implement 增量报告
- **THEN** 两者字段集完全一致（无「全量独有 / 增量独有」字段），差异只在字段值：`mode`（`full` / `incremental`）、`base`（`null` / `HEAD`）、`scan.changedFiles`（`0` / 变更数）与扫描清单

#### Scenario: 无模型与零可判文件两态可辨

- **WHEN** 分别以「模型目录缺席」与「模型在但零源文件通过排除约定」的观察根执行全量检查
- **THEN** 前者 `status = skipped`、后者 `status = noChanges`；两者 `errors` 皆空但 MUST NOT 呈 `clean`，消费方（门禁 / UI）按各自文案分列「未检查（无模型）」与「未检查（无可判文件）」

### Requirement: 判定单源与消费面同源

架构漂移的判定 SHALL 只有一处实现（`crates/core/archi` 的漂移判定纯函数：输入 = 观察根 + 模式 + 变更集 + 配置 + 模型，输出 = 报告）。消费面 SHALL 为三处调用点——IPC 命令面（`archi_drift(root, mode)`）、implement 相位门禁 runner（增量）、initialize 基线门禁 runner（全量）——后两者为**同一 runner 的两次挂载**（见 desktop-archi-gate「门禁挂载点」），MUST NOT 各写一份归属 / 定级 / 状态推导；报告结构体 SHALL 为全部消费面共用的唯一形状（命令面出线经同一 DTO 投影，门禁面从同一结构派生 `passed` / 诊断文本）。

#### Scenario: 命令面与门禁面逐字段同源

- **WHEN** 以同一 root、同一模式（增量）与同一变更集分别经 `archi_drift` 命令与门禁 runner 触发检查
- **THEN** 两份报告除 `timestamp` / `change`（命令面无 change 上下文）与落盘路径外逐字段相等（`model` / `scan` / `errors` / `warnings` / `suppressed` / `observations` / `status` 全等）

#### Scenario: 全量面同源

- **WHEN** 以同一 root 分别经 `archi_drift(root, "full")` 与 initialize 基线门禁触发全量检查
- **THEN** 两份报告除 `timestamp` / `change` / 落盘与否外逐字段相等；`passed` 派生面（`errors.is_empty() && model.structuralErrors.is_empty()`）两处一致——MUST NOT 出现「命令面 clean 而基线门禁红」的同根分歧

#### Scenario: 零第二套判定

- **WHEN** 审查命令面与 infra runner 的源码
- **THEN** 三者调用点均只做「取输入 → 调判定纯函数 → 投影 / 落盘」，无归属匹配、无定级分支、无 status 推导的第二份实现；模式差异只体现为入参，不体现为实现分支

## Module Contract

| 模块 | 职责 | 关键契约（Function / API） |
|------|------|------|
| `crates/core/archi/src/drift/mod.rs` | 判定单点（零逻辑改动） | `check(CheckInput) -> Result<Report, CheckError>`：`mode` 为入参、`changed` 全量模式忽略；`derive_status` 与模式无关；零写盘、零 spawn |
| `crates/core/archi/src/drift/report.rs` | 报告形状（零改动） | `ArchiDriftMode { Full, Incremental }`（`as_str` → `full` / `incremental`）、`ArchiDriftReport.mode` / `.change` / `.base` 字段面与 `version = 3` 不变 |
| `crates/core/archi/src/scan/**` | 扫描与归属（零改动） | 全量 / 增量共用同一遍历、排除矩阵与 `OwnerIndex` |
| `crates/infra/archi/src/runner.rs` | 落盘消费面 | port `ArchiCheckMode` → `ArchiDriftMode` 唯一映射；全量 / 增量同树落 `reports/archi/<timestamp>.json`（目录经 `foundation::layout::change_reports(root, name, ReportSlot::Archi)`） |
| `src-tauri/src/commands/archi/mod.rs` | 命令面（零改动） | `archi_drift(root, mode)` 两模式语义不变：`full` → 空变更集、进程内返回、零写盘 |
