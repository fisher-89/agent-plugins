# 提案: archi-doc-api-surface-unification

> **变更**: archi-doc-api-surface-unification（文档内容面 · 散文表格与契约名字清单合一）
> **日期**: 2026-11-19
> **状态**: draft
> **来源**: 本 change 目录 `explore.md`（§1–§7 全篇 + §5 八条裁决）；§6 的 gate 形态与同节零命中实测互斥，本提案显式裁决（R-1）
> **性质**: **纪律成文化 + 装载期判据新增**——零生成物变化、零判据词汇变化、零 IPC / UI 变化。新增一条装载期闸（`ApiError::Contract`），把「散文不复述契约名字」从人肉纪律变成机械判据，并为前端 8 份「公共面」表的将来退役留下机械触发点（表随契约到场而必删，见「提案 · 四」）
> **不含**: 前端组件契约的接线（缺口 1 `$ref`→`Named` / 缺口 2 组件与 JSX 元素无契约词汇 / 缺口 3 `.i.ts` 末尾空行）与前端 8 份公共面表的退役——三条缺口与退役一并归另起的组件契约探索（explore §7）

---

## 问题

### 现状链路（文件层早已合一，分裂在同一份文件内部）

```text
.dev-team/architecture/models/<分层目录…>/<FQN>.md      ← 唯一手写真相（10 份在盘）
├── 散文段：职能 / 边界 / 不变量 / 承载理由 / 逐 op 语义 / 契约面留白
└── fenced json：api-op / api-types / api-reexport    ← 装载期四条闸的判据对象
        │ api:export（archi-api-gen）
        ▼
<FQN>.schema.json（同目录）+ graph/INDEX.toml + <入口主干>.i.rs / <入口主干>.i.ts
```

`api::paths::carrier_path` 已把 `declared` 元素的入口文档与契约落点合成为同一条派生（`paths.rs:278` 的注释写明「不是两份资产，只是同一落点的两个用途」）。所以「模块 doc 与 api-doc 是两份文件」在今天不成立：**分裂发生在同一份文件的两种书写形态之间**。

### 病灶一 · 名字清单有三个潜在持有者，只有一个被机械锁住

| 持有者 | 形态 | 判据 |
|---|---|---|
| ① 散文「公共面」表 | 手写 markdown 表格（首列即名字） | **零判据** |
| ② 契约 fence | `api-op` 的 `### <op-id>` / `api-types` 键 / `api-reexport` 键 | 与 ③ 锁死（`apiSignatureMismatch` 为 `error` 档） |
| ③ 代码导出面 | `extract::extract_element` 的导出闭包 | 机器事实，是 ② 的判据输入 |

- ②↔③：`normalize_signature`（代码侧）对 `signature_of`（契约侧），不符即 `apiSignatureMismatch`（`error`）。
- ①↔③：`apiUndeclared`（「代码有导出而契约未声明」）判据在，但 `severity() = Off` 且调用处按档位跳过 ⇒ **今天永不执行**（`check.rs` 的 `if severity != Off` 分支）。
- ①↔②：**不存在**。唯一先例是 `api-reexport` 的散文列表 ↔ fence 双向闸（第三条 gate），它只覆盖透传清单这一种形态。

### 病灶二 · 前端 8 份表格靠人肉纪律，且补契约时必须手抄一遍

本仓 8 份前端元素契约（`Frontend/Components/Agent.md` + 7 个子元素）用表格书写公共面，共 **27 条**数据行；8 份的 `api-types` 恒 `{}`、零 `api-op`——即契约面整体留白：

````text
## 公共面
| 名字 | 形态 | 来源文件 | 语义 |      ← 名字 / 形态 / 语义全部手写，零判据
## 类型
```json api-types
{}
```
````

实测这 8 份的表名字与 `components/agent/index.tsx` 的 barrel 导出**逐条一致**（7 条）。纪律还绿，但没有任何判据会把它拉回绿——将来补 fence 时人只要抄表（抄错零成本），表与 fence 就各存一份签名，与「唯一手写真相」正面冲突。

### 病灶三 · explore §6 的闸形态与同节的零命中实测互斥（本提案需裁决）

explore §6 把闸写成「散文表格名字集 **≡** `ops ∪ $defs ∪ reexports`」，而同节「零命中实测」表声称两份后端契约（`Foundation` 9 op / `ArchiRuntime` 1 op + 1 type）**表格行 0、闸生效且不误伤**。两者不可能同时成立：等式语义下「表格 0 行 ≠ 9 个 op」立刻是契约错误。本提案在 R-1 里裁决取**交集为空**（见「提案 · 二」），理由是等式语义要求散文反向镜像契约，与本变更目标（散文不持有名字清单）相反，且会在落地当天把两份后端契约判死。

---

## 提案

### 一 · 名字清单的唯一持有者是契约 fence，散文只承载语义

| 面 | 承载 | 不承载 |
|---|---|---|
| `api-op` / `api-types` / `api-reexport` fence | **名字清单**（op id、`$defs` 键、透传键）+ 签名 | 语义散文（不写 `description`） |
| `## <中文二标题>` 散文段 | 职能 / 边界 / 不变量 / 承载理由 / 逐 op 语义 / 「契约面留白」登记 | **MUST NOT 以表格复述契约名字** |

- **唯一例外（既有，不动）**：`api-reexport` 的散文透传列表（列表项形态）——它本身就是第三条 gate 的机械镜像面，一致性由那条 gate 守；本禁令只针对**表格**这一未被判据覆盖的形态。该例外 SHALL 在 spec 里写明，避免「散文不持有名字清单」被读成与既有 gate 自相矛盾。
- **前端 8 份表格本变更原样不动**（explore 裁决 #7）：它们此刻合法，因为其契约三项（`ops` / `types` / `reexports`）全空 ⇒ 闸的前置不成立。留白是**临时债**，退役由新增的第四条闸机械强制（见「提案 · 四」）。

### 二 · 机械面：散文表格抽取 + 第四条装载期闸（`contract::verify`）

```text
新增 collect_surface_table(sections) -> Vec<String>            （私有，零新增公开出口）
  只认「表格行」：去前导空白后以 `|` 起手，按 `|` 切首列；再要求该列去空白后
  **整体被一对反引号包裹**且内容是 ASCII 标识符（`is_identifier`）——否则跳过。
  fence 内容天然不进 section body（parse 的 fence 状态机已隔断）；列表项零抽取
  （那是既有 collect_reexport_prose 的形态，正文名字绝不判）。

verify(contract, schema) 追加第四条闸（与第三条 gate 同族、同错误面、同一收口函数）
  前置（留白豁免）：契约非留白 —— ops / types / reexports 三项任一非空
  ⊢ 抽取集 ∩ 契约名字集(ops id ∪ api-types 键 ∪ api-reexport 键) = ∅
    非空 ⇒ ApiError::Contract，点名文件 + 交集名字 + 出路句（「名字归 fence，散文只留语义」）
```

- **前置是留白豁免的唯一来源**：三项全空即「契约面留白」，该元素的散文表格原样合法。放宽（如「有 fence 才判」）会让只写透传的契约逃逸；收紧（如「表存在即判」）会在落地当天判死 8 份前端文档。
- **不接受等式语义**（裁决见 R-1）：闸判的是「同一名字不得两处持有」，不是「表格必须镜像契约」；所以「散文表格中的参数表 / 对比表 / 枚举值表」只要不与契约名字撞名就合法（R-2）。
- **作用域 = 全部 `##` 散文段，不加段名白名单**：`## 公共面` 不是机器概念，把段名写进代码会把一个临时形态冻结成约定；而抽取面已被「表格行 + 首列整体反引号 + ASCII 标识符 + 与契约名字求交」三重收窄，误伤面足够小（若真误伤，收敛路径是段名白名单，见 R-2）。
- **错误面走既有 `ApiError::Contract`** ⇒ `load_contracts` 中止，**零新增 finding kind**（十三值 / 七 `category` / `severity()` / 金样面 / 13 份 sidecar 全部不动）。这与「契约资产的错须在装载期中止」的既有纪律同源（`contract.rs` 模块头注释）。
- **错误不带行号**（verify 期扫段落正文，body 行号未保留）：与既有两条 gate（`api-types` 缺键 / `api-reexport` 差集）同档，文案给文件 + 交集名字即可直接定位到表格那几行。
- **抽取实现落 `verify` 期而非 `parse` 期**：`Contract` 结构体因此零字段新增（`sections` 已在），`Contract::PartialEq` 的既有断言零随动。

### 三 · 为什么不走 B / C / D（三条路同撞前端 TS 契约留白）

| 路 | 为什么本变更不做 |
|---|---|
| B · 散文 ↔ fence 双向闸 | 前端 8 份 `$defs` 为 `{}` ⇒ 立刻红 ⇒ 必须先写前端契约，而那要先解三个缺口（`$ref` 归一不对称 / 组件形态无词汇 / `.i.ts` 末尾空行） |
| C · 表退役改渲染 | 同 B，且需架构页/文档视图从契约渲染人读 API 参考（前端零契约内容 ⇒ 空表） |
| D · 统一「每符号一节」 | 同 B，且 `### <op-id>` 无 fence 是结构性错，不能只写 heading |

三条路都卡在 explore §4 的三个缺口上，而 explore 裁决 #6 已定「非阻塞缺口都不修，随组件契约探索一并处理」——本提案沿该裁决，只做不依赖缺口的那条路（A 的变体：散文 ↔ 契约名字闸，且方向取「不得重复持有」）。

### 四 · 留白登记与退役路径（本变更只埋触发点，不执行退役）

```text
今天        8 份前端契约三项全空  ⇒ 第四条闸不生效  ⇒ 表格合法（临时债）
组件契约变更  名字进 $defs / api-op ⇒ 前置成立        ⇒ 表格里同一批名字 ⇒ 装载期红
                                                            ↓
                                                    表必删（或行必删），名字只留 fence
```

即退役不是本变更的动作，而是组件契约变更的**机械必然后果**：闸不提醒「该退役了」，它只让「退役前」的形态落不了盘。本变更 MUST NOT 顺手删表（会立刻丢掉 27 条人读语义，且无处安放——`$defs` 还没写），也 MUST NOT 抬 `apiUndeclared`（留白期抬档会把临时债读成判据面债务，explore 裁决 #5）。

### 五 · 零扩散边界（本变更刻意不动的东西）

| 面 | 为什么不动 |
|---|---|
| 生成链（`schema.rs` / `emit/**` / `render.rs` / `diff.rs` / `api_artifacts.rs`） | 第四条闸在 `verify` 里，零写盘、零产物变化 ⇒ `api:export` / `api:check` 逐字节零 diff |
| 判据面（`check.rs` / `drift/report.rs` / `drift/mod.rs`） | 零新增 finding kind ⇒ 十三值、七 `category`、三档 `severity()`、纳管口径、`REPORT_VERSION = 3` 全部逐字不变 |
| 知识文本（`archi-knowledge.md`） | 骨架当前 **3071 / 3072 字节**，已贴 3KB 上限（`体量切分骨架不超3kb`）；加一句纪律须同步抬上限，属独立变更（R-5）。纪律落 `AGENT.md`（agent 系统提示词已注入）+ 装载期错误文案自解释 |
| 前端产品代码 | 零 UI / 零 IPC / 零 DTO 变化；契约面板两轴读数不变 |
| 本仓 10 份契约资产 | 前端 8 份表格原样留（留白临时债），后端 2 份零表格 ⇒ 全仓装载零错误 |

### 六 · 本仓读数（落地当天应成立）

| 项 | 读数 |
|---|---|
| 契约在盘 | 10 份（后端 2 + 前端 8） |
| 第四条闸生效面 | **2 份**（`App.Backend.Core.Foundation` 9 op、`App.Backend.Infra.ArchiRuntime` 1 op + 1 type）——两份表格行均 **0** ⇒ 抽取集为空 ⇒ 交集为空 ⇒ 通过 |
| 闸不生效面 | **8 份**（前端，三项全空）——27 条表格数据行原样合法 |
| 生成物 | `api:export` 零 diff（`.schema.json` / `INDEX.toml` / `.i.*` 三类逐字节不变） |
| 判据面 | 第六段 `docMissing 43`、`status = clean`、`passed = true` 逐字不变 |
| 语料 | `api/**` sidecar 仍 13 份；`corpus/drift/api-mismatch/contracts/App.Mod.md` 零表格行 ⇒ 十六份金样零变化 |

---

## 能力

### 新增能力

无。

### 修改的能力

| 能力 | delta | 改什么 |
|---|---|---|
| `desktop-archi-api` | **MODIFIED ×2** | ①「契约资产形态与唯一手写真相」：名字清单唯一持有者 = 契约 fence（散文 MUST NOT 以表格复述）、五类位置语义的散文行补一条禁令、双向闸三条 → **四条**（前置留白豁免 + 交集为空 + `ApiError::Contract` 错误面）、`api-reexport` 列表项的显式例外、前端 8 份留白为临时债与「退役由第四条闸机械强制」；新增两条 scenario（表格复述即装载期错 / 留白契约的表格原样合法）②「仓库根文档的契约纪律节」：第 ① 条要点补「散文不复述名字清单」一句（条目数仍 5、两文件同源）。Module Contract 补 `contract.rs` 的抽取面与 `verify` 闸行 |
| `desktop-archi-api-check` | **ADDED ×1** | 新关注点「装载期名字清单闸与判据面的分工」：第四条闸归装载期、MUST NOT 计入 `apiFindings` / MUST NOT 新增第十四值；判据面 MUST NOT 读散文表格（`apiUndeclared` 声明集仍只取 schema `$defs`）；留白期该形态**零判据**为显式留白、`apiUndeclared` 保持 `off` MUST NOT 抬档；第六段读数 / 13 份 sidecar / 金样零变化的守护条。本能力**零代码改动** |

### 零 delta 的能力（出现 spec diff 即误改）

| 能力 | 理由 |
|---|---|
| `desktop-archi-agent` | 知识文本体量贴边（R-5），纪律改由 `AGENT.md` 承载；`archi_module` / `archi_search` / `archi_api` 三个工具面零变化 |
| `desktop-archi-contract-panel` | 零新 finding kind、零新 `category`、零 DTO 变化 ⇒ 两轴呈现与状态矩阵逐字不变 |
| `desktop-archi-model` / `repo-archi-model` | 模型字段面零变化（本变更不碰 `api` / `doc` / `entry`） |
| `desktop-frontend-module-entry` | 「入口 = 公共面」的代码侧等式不变；8 份表格是文档形态，不是入口纪律 |
| `desktop-corpus-regression` / `desktop-archi-gate` | 金样零变化、门禁布尔口径零变化 |

---

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `packages/desktop/src-tauri/crates/core/archi/src/api/contract.rs` | 新增私有抽取面（表格行 → 首列整体反引号 → `is_identifier`，去重保序）+ `verify` 第四条闸（前置：`ops` / `types` / `reexports` 任一非空；断言：抽取集 ∩ 契约名字集为空；错误 `ApiError::Contract` 点名文件 + 交集 + 出路句）；模块头注释「三条双向 gate」→「四条」并补一句「只判点名、不判语义」 |
| `AGENT.md` + `CLAUDE.md` | `## 模块 API 契约` 第 ① 条补一句：散文段 MUST NOT 用表格复述契约名字（名字的唯一持有者是 fence）；条目数仍 5、其余四条逐字不动、两文件该节正文逐字相同 |
| `packages/desktop/package.json` | `version` +1 patch（桌面源码改动，无例外） |

### 测试文件

| 对象 | 判断 |
|---|---|
| `crates/core/archi/src/api/contract_test.rs` | 扩：抽取面正反例（合法表格行 / 首列无引号 / 首列非标识符 / 表格分隔行 / 列表项零抽取 / fence 内表格零抽取 / 多段合并去重）+ 闸正反例（表格复述 op id、`$defs` 键、`api-reexport` 键各一条 ⇒ `Err` 且文案含交集名）+ 留白契约 + 表格 ⇒ `Ok`（前置不成立）+ 表格列非契约名字 ⇒ `Ok`。**全部落在既有文件内，MUST NOT 新增 `api/**` 文件**（`golden_test.rs` 断言 sidecar 恰 13 份） |
| `crates/core/archi/tests/pilot_test.rs` | 新增一条 crate 级用例：`<archi_models_dir>` 递归全部 `*.md`（当前 10 份）经 `parse` + `build_schema` + `verify` 全线通过——把「本仓语料零误伤」变成机械面（前端 8 份走留白豁免、后端 2 份零表格行） |

### 不要修改

| 对象 | 理由 |
|---|---|
| `.dev-team/architecture/models/**`（10 份契约 md 与 10 份 `.schema.json`） | 前端 8 份「公共面」表**原样不动**（explore 裁决 #7：留白临时债，随组件契约变更退役）；后端 2 份零表格。本变更 MUST NOT 新建 / 删改任何契约资产 |
| `.dev-team/architecture/graph/INDEX.toml` | 纯派生生成物；第四条闸不产生产物变化 ⇒ 重生成应零 diff |
| `crates/core/archi/src/api/{schema,render,diff}.rs` · `api/emit/**` · `api/extract/**` | 生成链与取数面零改动（闸在 `verify`，不读导出集） |
| `crates/core/archi/src/api/mod.rs`（`load_contracts` / `enabled` / `gating_findings`） | 装载入口、启用判据、纳管口径零改动；`verify` 的调用点与签名不变 |
| `crates/core/archi/src/api/check.rs` · `drift/{report,mod}.rs` | 十三值 / 档位 / `category` / 纳管 / `passed` / `REPORT_VERSION` 逐字不变；本变更 MUST NOT 造第十四值，也 MUST NOT 抬 `apiUndeclared` 档位 |
| `crates/infra/agent/src/archi-knowledge.md` + `archi_knowledge_test.rs` | 骨架 3071 / 3072 字节已贴 3KB 上限；本变更不加纪律句（R-5） |
| `crates/core/archi/tests/corpus/**` | 语料契约零表格行 ⇒ 金样与 `DRIFT_CASES` 登记面零改动 |
| `packages/desktop/src/**`（前端产品代码与类型生成物） | 零 IPC / 零 DTO / 零 UI 变化 ⇒ `bindings:export` 幂等、`client:check` 零 diff |
| `.dev-team/architecture/decisions/**`（ADR） | 本变更不写 ADR（决策留在 spec 与本提案） |

---

## 验收标准

1. **AC-1 · 抽取面形态封闭**：抽取只认「去前导空白后以 `|` 起手的表格行 + 首列去空白后整体被一对反引号包裹 + 内容是 ASCII 标识符」；列表项（形如 `` - `名字`：… ``）、裸正文、fence 内表格、首列无引号、首列非 ASCII 标识符**零抽取**。反向判据：`Foundation` / `ArchiRuntime` 的散文列表项（含 16 条留白登记）不得被抽成名字。
2. **AC-2 · 第四条闸的判据方向一经裁决即钉死**：断言为**交集为空**（抽取集 ∩ `ops id ∪ api-types 键 ∪ api-reexport 键` = ∅），MUST NOT 落地成「等式 / 镜像」语义；错误为 `ApiError::Contract`，文案含文件、交集名字与出路句（名字归 fence、散文只留语义）。
3. **AC-3 · 留白豁免的结构性成立**：前置 = 契约三项（`ops` / `types` / `reexports`）任一非空；三项全空 ⇒ 闸不生效。以本仓语料为判据：8 份前端契约（27 条表格数据行）装载零错误、2 份后端契约（0 表格行）装载零错误——即「10 份契约全量 `parse` + `build_schema` + `verify` 通过」由 `pilot_test` 一条用例机械守。
4. **AC-4 · 退役触发点可演示**：在某留白契约里补一个 `api-types` 键、并让它的散文表格列出同名 ⇒ 装载期 `ApiError::Contract` 点名该名；同批删除该表格行后通过（用例覆盖该两态）。该演示 MUST NOT 落进 `corpus/**` 金样语料。
5. **AC-5 · 判据面与词汇面零扩散**：`ArchiApiFindingKind::ALL` 长度仍 13、七 `category`、三档 `severity()`、纳管口径、`passed` 判据、`REPORT_VERSION = 3` 逐字不变；`api/**` sidecar 仍 **13** 份（`golden_test.rs` 的计数断言不改）；十六份 drift 金样零变化（语料契约零表格行）。反向判据：全仓 `grep` 无第 14 个 finding kind 的痕迹。
6. **AC-6 · 生成物零 diff**：`api:export` 后 `git diff` 对 `.schema.json` / `INDEX.toml` / `.i.rs` / `.i.ts` 逐字节为空；`api:check` 零差异退出。
7. **AC-7 · 纪律出处两份**：`AGENT.md` 与 `CLAUDE.md` 的 `## 模块 API 契约` 第 ① 条含「散文不复述名字清单 / 名字唯一持有者为 fence」语义的一句，该节要点数仍恰 **5**、两文件该节正文逐字相同；节内仍零 `openspec/specs/` 路径、零判据长表、零 fenced json 示例。
8. **AC-8 · 留白债在案且不销账**：`desktop-archi-api` 的留白条写明「前端 8 份公共面表为临时债、退役由第四条闸机械强制、退役动作归组件契约变更」；`desktop-archi-api-check` 写明该形态**零判据**且 `apiUndeclared` 保持 `off`。spec 内 MUST NOT 出现「表格已退役」「名字清单已统一」一类既成事实句。
9. **AC-9 · 零扩散与版本纪律**：`git status --porcelain` 的改动集 ⊆ 变更范围所列文件（注意 `AGENT.md` / `CLAUDE.md` 两份都要改、版本号一处）；`packages/desktop/package.json` 的 `version` 相对落地时 `HEAD` 恰 +1 patch；`src-tauri/tauri.conf.json` 仍 `"version": "../package.json"`；`src-tauri/Cargo.toml` 的 crate 版本逐字不变；`openspec/specs/` 下除本 delta 的 2 个能力外零 diff（`desktop-archi-agent` / `desktop-archi-contract-panel` 零 diff 是判据之一）。

---

## 风险

| # | 风险 | 处置 |
|---|---|---|
| R-1 | **explore §6 的闸形态与同节零命中实测互斥**（写「表格名字集 ≡ `ops ∪ $defs ∪ reexports`」，又声称两份后端契约 0 表格行时「闸生效但不误伤」） | 裁决取**交集为空**：等式语义要求散文反向镜像契约，与本变更「散文不持有名字清单」目标相反，且落地当天即判死 `Foundation` / `ArchiRuntime`。AC-2 把方向钉死，spec 内 MUST NOT 同时保留「≡」措辞 |
| R-2 | **抽取面误伤**：散文里出现参数表 / 对比表 / 枚举值表，首列恰好与契约名字同名 | 三重收窄（表格行 + 首列整体反引号 + ASCII 标识符）；错误文案点名交集名字（作者一眼看出是哪一行）；若仍误伤，收敛路径是「只扫 `## 公共面` 段」——属局部改动，MUST NOT 预留配置开关或豁免清单 |
| R-3 | **「散文不持有名字清单」与既有 `api-reexport` 散文列表冲突**（那边的列表项正是散文名字清单） | spec 写明显式例外：透传列表是第三条 gate 的机械镜像面，一致性由那条 gate 守；本禁令只针对未被判据覆盖的**表格**形态。禁用句式：「散文零名字」 |
| R-4 | **前端 8 份表格的合法性完全依赖留白豁免**——组件契约变更的作者若在同一提交补了 `$defs` 却不清表，装载期立即红 | 这是设计意图（机械强制退役），但须提前说清：本提案「提案 · 四」、`desktop-archi-api` 留白条与 `desktop-archi-api-check` 的零判据条同批写明触发条件；错误文案自带出路句，不需人再去读 spec |
| R-5 | **知识文本承载不了新纪律**（骨架 3071 / 3072 字节，`体量切分骨架不超3kb` 贴边） | 纪律落 `AGENT.md`（agent 系统提示词已注入）+ 错误文案自解释；`archi-knowledge.md` 本变更零改动。将来若须进知识文本，须同批调整体量上限并说明余量来源（独立变更） |
| R-6 | **错误不带行号**（verify 期扫段落正文） | 与既有两条 gate 同档；文案给文件 + 交集名字 + 出路句。若实现期发现定位困难，可在 `parse` 期记 `(name, line)` 后传入 verify——该改动会动 `Contract` 结构体，须在 design 期显式裁决，MUST NOT 悄悄两处各扫一遍 |
| R-7 | **生成物零 diff 的结论依赖「语料契约零表格行」** | AC-6 用 `api:export` 实跑 + `git diff` 守；`pilot_test` 的全量装载用例再守一层。若出现非预期 diff，说明抽取面读到了不该读的东西（活体报警） |
| R-8 | **「散文只有一处被判据覆盖」容易被读成「散文从此受判据管」** | spec 明写：本闸**只判点名，不判语义**——职能 / 边界 / 不变量 / 承载理由仍是人审面（与既有「散文无人审判据」现状一致）。MUST NOT 由此扩张出第二类散文判据 |
| R-9 | **留白债被读成回归**（8 份表格仍在 ⇒ 有人以为本变更没做完） | AC-8 要求 spec 写明留白为临时债、退役归组件契约变更；本提案「六 · 读数」表并排列出「闸生效 2 份 / 不生效 8 份」，避免把「表格仍 27 行」读成交付缺口 |

---

## explore 未决项的归属

| explore 条目 | 处置 |
|---|---|
| §6「待 design 期定」· gate 作用域 | **已定**：扫全部 `##` 散文段（不加段名白名单——`## 公共面` 是临时形态，MUST NOT 冻结成机器约定）；收敛预案见 R-2 |
| §6「待 design 期定」·「契约非留白」的判据取 `contract.ops` / `contract.types` | **采纳并扩一项**：取 `ops` / `types` / `reexports` 三项任一非空——只写透传的契约同样持有名字（本仓当前零 `api-reexport` ⇒ 该扩展零现存影响）。MUST NOT 取 `schema["$defs"]`（生成物不是手写真假） |
| §6 的闸形态（「≡」） | **改写为交集为空**，理由见 R-1 |
| §7 · 组件契约探索（缺口 2 主干 + 缺口 1 / 3 顺带） | **不在本 change**：另起变更；其终点 = 前端 8 份表格退役、名字进 `$defs` / `api-op` |
| §7 · 留白债的机器计数（INDEX 债务列） | **不在本 change**：留白清账时再议；本变更只保证留白不被判据误伤、也不被抬档 |
| explore §5 裁决 #3 / #4 / #5 / #6 / #7 | **逐条沿用**：前端契约临时留白、不加 fence、`apiUndeclared` 不抬档、两处非阻塞缺口不修、8 份表格原样不动 |
