# desktop-archi-api-check Specification（delta: archi-doc-api-surface-unification）

## Purpose

本 delta 只登记**一条分工边界**，本能力**零代码改动**：名字清单的「散文表格复述」形态由装载期闸（`ApiError::Contract`，定义归 `desktop-archi-api`）判定，MUST NOT 进入判据面——十三值词汇 / 七 `category` / 三档 `severity()` / 纳管子集 / `passed` 判据 / `REPORT_VERSION = 3` / 第六段读数（`docMissing 43`）/ 13 份 sidecar / 16 份金样逐项不变。留白期（8 个前端组件元素契约三项全空）该形态**零判据**为显式留白，`apiUndeclared` 保持 `off`、MUST NOT 借本变更抬档。

## ADDED Requirements

### Requirement: 装载期名字清单闸与判据面的分工

名字清单的「散文表格复述」形态 SHALL 由**装载期闸**判定，MUST NOT 进入本能力的判据面；本能力 SHALL 在本变更内**零代码改动**（`check.rs` / `drift/report.rs` / `drift/mod.rs` 与纳管子集函数逐字不变）。

- **分工单点**：第四条闸（散文表格名字 ∩ 契约名字集 = ∅）的定义与实现归 `desktop-archi-api`「契约资产形态与唯一手写真相」，错误面为 `ApiError::Contract`（`load_contracts` 中止）。分界理由：判据面报的是「契约 vs 代码的比较结论」（可枚举、可渐进消债、进报告数基线），而「契约文件内部两处持有同一个名字」是**契约资产自身的错**——它与代码导出集无关，也不该被算成一条债进入报告（否则一个错字会同时污染基线表与 `status`）。
- **零新增判据词汇**：`ArchiApiFindingKind` SHALL 保持**十三值**（`ALL` 长度断言不变），`category` 保持七值、`severity()` 保持三档；该闸 MUST NOT 产出 `apiFindings` 条目、MUST NOT 参与纳管子集与 `passed` 判据、MUST NOT 改动 `REPORT_VERSION`（仍 3）。
- **判据输入面 MUST NOT 含散文表格**：十三判据的输入 SHALL 仍为「模型 + 契约 fence / `<FQN>.schema.json` + 代码导出集 + `doc_facts`」；`apiUndeclared` 的声明集 SHALL 仍**只取 `<FQN>.schema.json` 的 `$defs` 键集**，MUST NOT 并读 markdown 散文、MUST NOT 新增表格行读取点（判据面 MUST NOT 补写半个 markdown 解析器——与「`apiDocMissing` 只判布尔、不接 `contract::parse`」同一条纪律）。
- **留白期零判据（显式留白，不是疏漏）**：8 个前端组件元素的契约三项（`ops` / `types` / `reexports`）当前全空，其散文「公共面」表共 27 条数据行、名字已存在于代码导出面而未进契约 —— 该形态在本能力**零判据**：MUST NOT 报 `apiUndeclared`（档位保持 `off`），MUST NOT 为此新增第十四值或第八个 `category`，MUST NOT 借本变更为「表里列了未声明名字」另立判据。**`apiUndeclared` MUST NOT 抬档**：留白期抬档会把临时债读成判据面债务，且与本能力「完备性判据报的是海、迁移期必然爆炸必须后置」的分档动机冲突；抬档是独立裁决，触发条件是组件契约到场（届时表格已由第四条闸强制退役）。
- **读数与语料零漂移**：本仓第六段基线 SHALL 逐项不变——`docMissing` **43**（41 + 2）、`apiEntryMissing` **0**、`apiMissing` **0**、其余十值各 **0**、`status = clean`、`passed = true`；`api/**` sidecar SHALL 仍恰 **13** 份（新用例落在 `contract_test.rs` / `pilot_test.rs`，MUST NOT 新建 `api/**` 文件）；`corpus/drift/**` 的 16 份金样零变化（语料契约 `App.Mod.md` 零表格行）。

#### Scenario: 表格复述契约名字只装载期中止，不进报告

- **WHEN** 某 `api = declared` 元素的契约已声明 op `is_set`，其散文段又有一张以 `` `is_set` `` 起首的表格行
- **THEN** 装载期判 `ApiError::Contract` 并中止；报告面 MUST NOT 产出任何新条目（`apiFindings` 内零该事实、`ALL` 长度仍 13、`category` 七值、`severity()` 三档不变）；`status` / `passed` 的推导路径零改动

#### Scenario: 留白期散文表格零判据

- **WHEN** 8 个前端组件元素的契约三项全空、散文表格共 27 条数据行，且这些名字在代码导出面成立
- **THEN** 装载零错误、`apiFindings` 零新增条目；`apiUndeclared` 不产出（档位仍 `off`）；把某元素契约补一个 `api-types` 键 `AgentInput` 后，报错来自装载期（`ApiError::Contract`）而**不是** `apiUndeclared`

#### Scenario: 判据输入面零表格读取

- **WHEN** 审查 `check.rs` 的输入装配、`ApiCheckInput` 的字段面与 `apiUndeclared` 的声明集来源
- **THEN** 零表格行读取点（散文表格的唯一读者是 `contract::verify`）；`apiUndeclared` 声明集只取 `<FQN>.schema.json` 的 `$defs` 键集；`doc_facts` 三态与 `entry_state` 口径逐字不变

#### Scenario: 第六段读数与语料守护零漂移

- **WHEN** 对本仓模型跑一次检查并核对语料面
- **THEN** `docMissing 43`、其余十二值各 0、`status = clean`、`passed = true` 逐项与基线表相等；`api/**` sidecar 仍 13 份；16 份 drift 金样逐字节不变；本能力在 `git diff` 中零代码改动（`check.rs` / `drift/report.rs` / `drift/mod.rs` 无差异）

## Module Contract

| 模块 | 职责 | 关键契约 |
|---|---|---|
| `crates/core/archi/src/api/check.rs` | 十三判据单点求值 | **零改动**：两支首错短路序、`category_of` 七值、`apiUndeclared` 的 `off` 档与声明集口径、`doc_facts` 读取逐字不变；MUST NOT 新增表格行读取点 |
| `crates/core/archi/src/drift/report.rs` | 判据词汇与报告槽位 | **零改动**：`ArchiApiFindingKind` 十三值 + `ALL`、`ApiFindingSeverity` 三档、`ArchiApiFinding` 形状、`REPORT_VERSION = 3` 逐字不变 |
| `crates/core/archi/src/drift/mod.rs` | 报告装配与 `status` 分档 | **零改动**：`api_findings` 装配单点、纳管子集、`passed` 判据逐字不变 |
| `crates/core/archi/src/api/mod.rs::gating_findings` | 纳管子集筛选单点 | **零改动**：入选判据（declared 载体在盘 / none `doc` 已声明）逐字不变；装载期闸不产出条目 ⇒ 无需纳管口径扩展 |

### Function: 判据求值与档位（零变化）

| 函数 | 变化 | 描述 |
|---|---|---|
| `api::check(ApiCheckInput) -> Vec<ArchiApiFinding>` | 零改动 | 输入面（模型 / 契约 fence / `schema.json` / 导出集 / `doc_facts`）不变；表格行不是它的输入 |
| `ArchiApiFindingKind::severity()` | 零改动 | 三档封闭；`apiUndeclared` 仍 `Off`，MUST NOT 借本变更抬档 |
| `api::gating_findings(...) -> Vec<ArchiApiFinding>` | 零改动 | 纳管子集口径逐字不变 |

### API: 对外面（零变化）

| 项 | 变化 | 描述 |
|---|---|---|
| `archi_api` / `archi_drift` 命令响应 | 零改动 | `apiFindings` 形状与取值面不变；无新增 `category` / `kind` ⇒ `bindings:export` 幂等 |
| 门禁 `LocalArchiCheck` 报告 | 零改动 | `status` / `passed` 判据与命令面同源，逐字段相等保持 |

### CLI: 零变化

| 项 | 变化 | 描述 |
|---|---|---|
| `pnpm api:check` / `pnpm check` 链 | 零变化 | 段序与命令串逐字不变；本能力零代码改动 ⇒ 差异只可能来自 `desktop-archi-api` 的装载期闸 |

### Component: 呈现面（零变化）

| 项 | 描述 |
|---|---|
| `views/architecture/components/contract-panel.tsx` / `model-info.tsx` | 判据轴按 `category` 分组呈现同一份 `apiFindings`，零重算；本变更无 `kind` / `category` 增删 ⇒ 两处零改动 |
