# desktop-archi-api-check Specification（delta: api-gen-check-in-desktop）

## Purpose

本 delta 只改一处：报告并列槽位 `apiFindings` 与 `status` 的关系——由「首期完全不参与推导」改为「**契约在案即纳管**（`status` 可为 `warnings`，`passed` 仍不动）」，并相应递增 `REPORT_VERSION`。判据词汇十值、十条判据的定义与求值序、判据级档位与元素 `gate` 解耦三条 requirement 逐字不变。

## MODIFIED Requirements

### Requirement: 报告并列槽位 apiFindings

`ArchiDriftReport` SHALL 新增**并列**字段 `api_findings: Vec<ArchiApiFinding>`（线格式 `apiFindings`），MUST NOT 改造既有 `errors` / `warnings` / `suppressed` / `observations` / `status`：

- **既有字段零改动**：`ArchiDriftReport` 既有 14 字段 SHALL 逐字不变（字段名 / 类型 / 序列化形态 / 语义）；新增字段插在报告末尾。既有 corpus 金样的 `errors` / `warnings` / `suppressed` / `observations` SHALL 逐字不变，金样只需**追加** `"apiFindings": []`。
- **`version` 递增**：`REPORT_VERSION` SHALL 由 2 递增为 3，并留显式兼容说明（消费方按版本分支的语义：`version = 1` 无本字段；`version = 2` 有本字段但 `status` 推导**不**读契约判据）。
- **契约在案即纳管 `status`**：`apiFindings` 对 `status` 的影响 SHALL 限于**纳管子集**——只取「`element` 属于**契约文件在案元素集合**」的条目参与推导（该集合 = 契约 md 存在且可读的 `api = declared` 元素；判据与 `ArchiApiRef` 的 `declared` 判定同源）。纳管子集非空 ⇒ `status` 至少为 `warnings`；纳管子集为空 ⇒ `status` 与不引入本字段时逐字相同。
  - **口径理由（契约即承诺）**：没写契约的元素，其 `apiEntryMissing` 是迁移 backlog（既有 requirement 明写「MUST NOT 为消债而重构」）；写了契约的元素，判据就是「自己刚承诺的东西没兑现」——只有后者该进状态词。该口径天然自激活：契约逐份落地，纳管面自动扩大，**MUST NOT 引入白名单、开关或配置键**。
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

- **WHEN** 报告含 28 条 `apiEntryMissing` 与 13 条 `apiMissing`（全部来自**契约不在案**的元素）
- **THEN** 纳管子集为空、`status` 与引入 `apiFindings` 前逐字相同（本仓现状：唯一契约在案元素判据 0 条 ⇒ `status` 不变）；MUST NOT 因这 41 条把状态词常规化为 `warnings`

#### Scenario: 稳定序与聚合

- **WHEN** 同一元素对的三处签名不一致被三处命中
- **THEN** `apiFindings` 内该元素对恰一条，`occurrences` 含 3 项（各带 `file` / `line`）；整体排序为 `(kind, element, target)` 稳定序，两次运行逐字相同

#### Scenario: 命令面与门禁面同源

- **WHEN** 以同一 root 分别经 `archi_drift` 命令与门禁 runner 触发检查
- **THEN** 两份报告的 `apiFindings` 与 `status` 逐字段相等（含排序、`category` 与纳管结论）

## Module Contract

### API: `crates/core/archi/src/drift/mod.rs` + `src/api/mod.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `api::gating_findings` | 新增 | `(root, &[ArchiElement], &[ArchiApiFinding]) -> Vec<ArchiApiFinding>`：纳管子集筛选单点（「契约文件在案」判据与 `api_ref` 同源；`element = None` 落选） |
| `drift::check` | 改 | `api_findings` 装配后取纳管子集传给 `derive_status`；其余装配序与字段逐字不变 |
| `derive_status` | 改 | 新增纳管子集入参；`warnings` 分支条件扩为「`warnings[]` 非空 **或** 纳管子集非空」；`skipped` / `errors` / `noChanges` 的先后序与语义逐字不变 |
| `drift/report.rs` | 改 | `REPORT_VERSION` 2 → 3 与兼容说明；14 字段形状零改动 |

### Component: 呈现面（归 `desktop-archi-contract-panel`）

| 项 | 描述 |
|---|---|
| `views/architecture/components/contract-panel.tsx` | 判据轴按 `category` 分组呈现同一份 `apiFindings`（零重算） |
| `views/architecture/components/element-drawer.tsx` | 按 `element === fqn` 过滤呈现本元素判据（零二次 invoke） |
