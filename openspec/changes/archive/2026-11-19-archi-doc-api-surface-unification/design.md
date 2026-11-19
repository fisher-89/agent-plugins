# 设计: archi-doc-api-surface-unification

> **变更**: archi-doc-api-surface-unification
> **日期**: 2026-11-19
> **基于**: `proposal.md`（病灶一 / 二 / 三 + 提案一～六 + AC-1…AC-9 + R-1…R-9）、`explore.md`（§1–§7 与 §5 八条裁决）、`specs/desktop-archi-api/spec.md`（2 MODIFIED requirement + Module Contract）与 `specs/desktop-archi-api-check/spec.md`（1 ADDED requirement，**零代码改动**）
> **性质**: **纪律成文化 + 装载期判据新增**——唯一改动点是一处私有抽取面 + `verify` 里第四段闸（`contract.rs`），另有仓库根文档两句与版本行一行。零生成物变化、零判据词汇变化、零 IPC / DTO / UI 变化、零契约资产改动、零新增文件。

---

## 起点实况核实（工作树实测；前缀省略为 `packages/desktop/src-tauri/crates/core/archi/src/api/`）

| 面 | 实测 |
|---|---|
| `contract.rs`（1005 行） | `parse` `:103`；`verify` `:277`（三闸 → 闸一七族 → 落点闸 `:490` `verify_x_function_positions` → `Ok(())`）；`collect_reexport_prose` `:949`（列表项形态，CJK 行跳过）；`is_identifier` `:984`；`is_cjk` `:994`；`finish_section` `:999`；`ContractSection` `:89`（`heading` / `line` / `body`，**fence 内容不入 body**）；`Contract.sections` `:73`（`verify` 期已在手，无需扩签名） |
| `contract.rs` 模块头 | `:23` 「**[`verify`] 是装载期的校验收口**：三条双向 gate + 闸一 + 落点闸 + 自洽闸。四者同函数、同趟收口」；`:30` 「错误一律带 `file:line` 与原文片段」**（该句对既有「`api-types` 缺键 / `api-reexport` 差集」两闸已不精确——它们只有 `file` 前缀）** |
| 既有机械锚点（**不可被动摇**） | `contract_test.rs` 断言 `verify闸一判据单点`（七族族别串 + `is_identifier` 单点）；`verify七族与公开面零回归` 断言 `source.matches("\npub fn ").count() == 3`（公开面恰三：`parse` / `verify` / `is_constructor`）与 `pub fn verify(` 恰 1 处；`fence零第二常量` 断言 `pub(crate) const FENCE_` 恰 2 处；`golden_test.rs:1482` 断言 `api/**` sidecar 恰 **13** 份 |
| 契约在盘 | `.dev-team/architecture/models/` 下恰 **10** 份 `*.md`（后端 2 + 前端 8） |
| 后端 2 份的表格行 | `Backend/Core/App.Backend.Core.Foundation.md`、`Backend/Infra/App.Backend.Infra.ArchiRuntime.md` **零表格行**（`grep -rn '^\s*\|'` 零命中） |
| 前端 8 份的表格行 | 数据行共 **27** 条（`6 + 2 + 2 + 3 + 3 + 2 + 2 + 7`）；8 份都含 `## 公共面` 段，其中顶层 `App.Frontend.Components.Agent.md` 是 4 列（多一列「来源文件」）、其余 7 份 3 列，首列均是反引号包裹的名字；`api-types` 全 `{}`、零 `api-op`、零 `api-reexport` ⇒ 三项全空 |
| 表头与分隔行形态 | 表头首列是 `名字`（无引号）、分隔行 `|---|---|` ⇒ 两者在当前抽取口径下**天然零抽取**（不作为额外规则） |
| 语料契约 | `tests/corpus/drift/api-mismatch/contracts/App.Mod.md` 零表格行；16 份 drift 金样零变化的前置成立 |
| 判据面 | `check.rs:199` 的 `ApiUndeclared.severity() != Off` 分支（档位恒 `Off`，永不执行）；`drift/report.rs:251` 的 `ApiUndeclared => Off` |
| 根文档 | `AGENT.md:24` / `CLAUDE.md:24` 的 `## 模块 API 契约` 节；第 ① 条在 `:26`；两文件全文 `diff` 恰 2 处（`:1` 标题、`:3` 首行说明） |
| 版本 | `packages/desktop/package.json` `"version": "0.7.0"`；`src-tauri/tauri.conf.json` `"version": "../package.json"`；`src-tauri/Cargo.toml` crate 版本 `0.1.0`（不随动） |

---

## 事实校准（不改 spec 意图，只定实现形态）

1. **「首列」= 去掉行首那个 `|` 之后的第一个 `|` 段**。表格行 `` | `is_set` | 判定 | `` 按 `|` 切分得 `` ["", " `is_set` ", " 判定 ", ""] ``，若取 `split('|').next()` 会永远拿到空串（空串正是行首那个 `|` 的左侧）。故抽取实现为「先去前导空白并 `strip_prefix('|')`，再 `split('|').next()`」，`trim()` 后即首列。
2. **闸位置在 `verify` 末尾**（`verify_x_function_positions(contract, schema)?;` 与 `Ok(())` 之间）。spec Module Contract 的枚举序即「三闸 + 闸一 + 落点闸 + 自洽闸之后追加第四条闸」；放在末尾也使既有语料的报错顺序零变化（现网零命中，无优先级争议）。
3. **错误文案零行号，与既有两个段落级闸同档**。proposal R-6 允许「parse 期记 `(name, line)`」的备选，本设计**否决**该备选：它要给 `Contract` 加字段，而 `Contract::PartialEq` 的既有断言与「`Contract` 结构体零字段新增」的 spec 条款都要随动；`file` + 交集名字足以定位（一份契约的表格行数是个位数）。
4. **`AGENT.md` 该节的要点数是 6 条，不是 5 条**。既有主 spec 写「恰含五条要点：①…⑤」，但实盘在第 ④ 条（`api = none`）与第 ⑤ 条（入口文档承诺）之间还有一条 **`doc` 是入口文档指针**（由已归档的 `module-doc-field-docs-retirement` 引入，主 spec 未同步枚举）。本变更的落地口径是**条目数相对 `HEAD` 不变（6 条）**：第 ① 条内追加一句，MUST NOT 删 `doc` 条目、MUST NOT 新增第七条、MUST NOT 为凑「5」而改结构。措辞口径的更正归 spec 修订，不属本变更实现面（见待决问题 1）。
5. **抽取面只覆盖 `##` 段正文**。`parse` 在 `### <op-id>` 处 `finish_section`，故逐 op 语义段的正文**从不进** `sections`；H1 与首个 `##` 之间的行同样不入段。这不是抽取面的缺口，而是 spec「作用域为全部 `##` 散文段」的直接结构后果——本设计不扩面（MUST NOT 为此改 `parse` 的分段状态机）。
6. **模块头 `:23` 的「四者」是计数措辞**，加第四闸后会漂移。改为零数字表述（「这些闸同函数、同趟收口」），避免下次加闸再改一次；同处「三条双向 gate」改为「四条装载期闸（既有三条双向 gate + 第四条名字清单闸）」。
7. **模块头 `:30` 的「错误一律带 `file:line`」顺带说明限定**：段落级判据（`api-types` 缺键 / `api-reexport` 差集 / 本变更的散文表格）只有 `file` 前缀。这是把既有不精确处说清，不是新增约束。

---

## 关键设计决策

| # | 议题 | 定稿 | 理由 |
|---|---|---|---|
| D1 | 抽取面形态 | 私有 `fn collect_surface_table(sections: &[ContractSection]) -> Vec<String>`：逐段逐行 → `trim_start().strip_prefix('|')` → 取首个 `|` 段 → `trim()` → 要求 `` strip_prefix('`') `` + `` strip_suffix('`') `` 后**内部零反引号**且 `is_identifier(inner)` → 去重保序 `push` | proposal「提案 · 二」与 spec 抽取面三条（表格行 + 首列整体反引号 + ASCII 标识符）逐条落形；`is_identifier` 复用既有单点（MUST NOT 抄第二份字符判据） |
| D2 | 列表项与裸正文零抽取 | 抽取只认「表格行」一种形态；`collect_reexport_prose` 不动（列表项仍归第三条 gate） | `api-reexport` 散文透传列表是既有机械镜像面，把它并进抽取面会让「同一名字两处持有」的判据自己撞自己（proposal R-3） |
| D3 | 闸的前置（留白豁免） | `!(ops.is_empty() && types.is_empty() && reexports.is_empty())` 才判；三项全空直接跳过本闸 | 唯一来源，且取手写真假（`Contract` 字段）而非生成物 `$defs`；`reexports` 与另两项同权（只写透传的契约同样持有名字） |
| D4 | 断言方向 | `collect_surface_table(..)` ∩（`op.id` ∪ `types` 键 ∪ `reexports` 键）= ∅；命中保序输出 | 交集语义只判「同一名字不得两处持有」；等式语义要求散文反向镜像契约，落地当天即判死两份后端契约（proposal R-1 / AC-2） |
| D5 | 错误面与文案 | `ApiError::Contract`；私有 `fn surface_table_error(contract: &Contract, repeated: &[String]) -> ApiError`；文案固定含 `contract.file`、交集名字 `" / "` 连接、三段 fence 名（取 `FENCE_OP` / `FENCE_TYPES` / `FENCE_REEXPORT` 常量）与出路句「名字归 fence，散文只留语义」 | 与既有三处错误构造器（`identifier_error` / `x_function_position_error` / `required_dangling_error`）同族、同收口；零新增 finding kind；`ApiError::Contract` 终止 `load_contracts` |
| D6 | 抽取函数可见性 | **私有**（`fn`，非 `pub(crate)`） | spec Module Contract「零新增公开出口」；`contract_test` 是 `api` 的子模块，能测的是 `parse` / `verify` 一类公开面——与 `collect_reexport_prose` 经 `parse` 观测同范式（黑盒观测：`verify` 的 `Ok` / `Err` 文案即抽取面的可观察面） |
| D7 | 根文档纪律句 | `AGENT.md` / `CLAUDE.md` 第 ① 条在「三类 fenced json 块；」之后插入一句：`名字清单的唯一持有者也是这三类 fence——散文段 MUST NOT 用表格复述契约名字（重复持有即装载期契约错误），散文只留语义；` | 只表达一条事实（spec 明令）；不含判据条件 / 错误文案样例 / 豁免清单，也不含 `ApiError::Contract` 字面量；插入而非新增条目 ⇒ 条目数不变 |
| D8 | 零扩散边界 | 生成链、判据面、`api/mod.rs`、`api/check.rs`、`drift/**`、契约资产、语料、前端、`archi-knowledge.md` **逐字不动** | proposal「提案 · 五」与「不要修改」；`api/mod.rs::load_contracts` 的 doc 里「双向 gate」是泛称，MUST NOT 顺手改（否则违反 AC-9 的改动集约束） |
| D9 | 明确不做 | ① 段名白名单（若真误伤再收敛，独立变更）；② `parse` 期记录 `(name, line)`（事实校准 3）；③ 表格行读取点进判据面（`apiUndeclared` 声明集仍只取 `$defs`）；④ 抬 `apiUndeclared` 档位；⑤ 删前端 8 份表格；⑥ 知识文本追加纪律句 | 逐条对应 proposal R-1 / R-2 / R-5 / R-6 / R-9 与 AC-5 / AC-8 |

---

## 架构组件 (Architecture Components)

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|---|---|---|---|---|
| 散文表格抽取面（**新增**） | 把 `##` 段正文里的「表格行 + 首列整体反引号 + ASCII 标识符」抽成名字集（去重保序）；列表项 / 裸正文 / fence 内容零抽取 | `crates/core/archi/src/api/contract.rs` | 既有 `ContractSection` / `is_identifier` | 纯函数（零 IO、零分配外依赖） |
| 装载期第四条闸（**改**） | `verify` 末尾追加：前置留白豁免 → 交集为空 → `ApiError::Contract`（点名文件 + 交集名字 + 出路句） | 同上（`verify` 体内 + 私有 `surface_table_error`） | 上述抽取面、`Contract`（`ops` / `types` / `reexports` / `sections`）、`FENCE_*` 常量 | 纯函数 |
| 仓库根纪律节（**改**） | `## 模块 API 契约` 第 ① 条补一句「名字唯一持有者是 fence，散文不复述名字清单」 | `AGENT.md` + `CLAUDE.md` | 无（agent 系统提示词逐字注入 `AGENT.md`） | markdown |
| 版本行（**改**） | `version` +1 patch | `packages/desktop/package.json` | 无 | JSON |

依赖方向零新增边：`contract` 仍只依赖 `serde_json` 与 `super::{kind_of, ApiError}`；`core/archi` 仍零写盘、零 infra 依赖。装载期闸的调用点零改动：

```
<FQN>.md ──parse──> Contract ──build_schema──> schema(Value)
                        │                            │
                        └──────── verify(contract, schema) ◄── 新增第四段
                                     ├─ 三闸（op/heading ≡ fence、types ⊆ $defs、reexport ≡ 散文列表）
                                     ├─ 闸一（七族名字）+ 自洽闸（required ⊆ properties）
                                     ├─ 落点闸（x-function 二处）
                                     └─ 第四条闸：collect_surface_table(sections) ∩ 名字集 = ∅
                                              ↓ 非空
                                    ApiError::Contract ⇒ load_contracts 中止（零半真报告）
```

---

## 变更清单 (Change Inventory)

### 新增文件

<!-- 本变更零新增文件：抽取面与第四闸都落在 contract.rs 的既有职责内（该文件已是「装载期闸群收口」），新开模块会造出第二处需要 `pub mod` 出线的面，并让「四条闸同函数、同收口」的既有纪律失效。 -->

无。

### 修改文件

| 文件 | 具体修改 | 说明 |
|---|---|---|
| `packages/desktop/src-tauri/crates/core/archi/src/api/contract.rs` | ① 新增私有 `fn collect_surface_table(sections: &[ContractSection]) -> Vec<String>`（置于 `collect_reexport_prose` 之后、`is_identifier` 之前）：逐段逐行按 D1 抽取，`is_identifier` 复用既有单点，名字去重保序；doc 写明「只认表格行 + 首列整体反引号 + ASCII 标识符三重收窄；列表项归 `collect_reexport_prose`、裸正文是语义散文、fence 内容结构上不在 `body` 里，三者一律零抽取；`### <op-id>` 段正文不进 `sections`（`parse` 的 `finish_section` 决定），故抽取面 == 全部 `##` 散文段」。② 新增私有 `fn surface_table_error(contract: &Contract, repeated: &[String]) -> ApiError`（置于 `required_dangling_error` 之后）：见 D5 的文案三段。③ `verify` 末尾（`verify_x_function_positions(contract, schema)?;` 之后、`Ok(())` 之前）追加第四段：前置判空白 → `collect_surface_table(&contract.sections).into_iter().filter(名字集 contains).collect()` → 非空即 `return Err(surface_table_error(...))`；注释写明「前置 = 留白豁免的唯一来源；断言 = 交集为空而不是等式（闸判的是同一名字不得两处持有）；只判点名、不判语义」。④ `verify` 的 doc 注释补一段第四闸说明（前置 / 断言 / 错误面 / 零行号）。⑤ 模块头 `:23` 「三条双向 gate」→「四条装载期闸（既有三条双向 gate + 第四条名字清单闸）」，`:25` 「四者同函数」→ 零数字表述，并补一句「第四条只判点名、不判语义」；`:30` 补段落级判据的 `file` 前缀限定（事实校准 6、7）。 | AC-1 / AC-2 / AC-3 / AC-4 / AC-5；公开面仍恰 3 个 `pub fn`（新增两个全是私有，既有断言 `matches("\npub fn ").count() == 3` 不红）；`pub(crate) const FENCE_` 仍恰 2 处（第四闸复用既有 `FENCE_REEXPORT`，不新增常量） |
| `AGENT.md` | `## 模块 API 契约` 第 ① 条（`:26`）在「…三类 fenced json 块；」之后插入 D7 那一句；该节其余条目（含 `doc` 条与第 ② / ③ / ④ / ⑤ 条）逐字不动 | AC-7；条目数相对 `HEAD` 不变（事实校准 4）；节内 `grep 'ApiError::Contract'` 零命中、零 `openspec/specs/` 路径、零 fenced json 示例 |
| `CLAUDE.md` | 与 `AGENT.md` 同一位置插入同一句（逐字相同） | AC-7；两文件该节正文仍逐字相同，全文 `diff` 仍恰 2 处且全在节外（`:1` / `:3`） |
| `packages/desktop/package.json` | `"version": "0.7.0"` → `"0.7.1"`（单行） | AC-9；`tauri.conf.json` 经 `"version": "../package.json"` 跟随；`src-tauri/Cargo.toml` 的 crate 版本行 MUST NOT 随动 |

> **不在本清单的对象**（proposal「不要修改」+ D8）：`.dev-team/architecture/models/**`（前端 8 份表格原样，后端 2 份零表格）、`.dev-team/architecture/graph/INDEX.toml`、`api/{schema,render,diff}.rs`、`api/emit/**`、`api/extract/**`、`api/mod.rs`、`api/check.rs`、`drift/**`、`crates/infra/agent/src/archi-knowledge.md`、`tests/corpus/**`、`packages/desktop/src/**`、`.dev-team/architecture/decisions/**`。
>
> **测试文件面**（`api/contract_test.rs` / `tests/pilot_test.rs`）的用例本体归测试相位——本设计只登记它们必须遵守的两条机械约束：新用例 MUST NOT 新建 `api/**` sidecar（`golden_test.rs:1482` 的 13 份计数不动）；`pilot_test` 的全量装载用例收面口径 = `archi_models_dir(&repository_root())` 递归 `*.md`、路径串经既有单点 `archi::api::display` 归一（`mod_test.rs` 断言 `display` 是跨 crate 单点，MUST NOT 自写第二份 `replace`）。

### 公共函数 / API

| 标识符 | 所在文件 | 变化 | 完整签名 | 说明 |
|---|---|---|---|---|
| `verify` | `crates/core/archi/src/api/contract.rs` | 修改（追加第四段闸，签名与错误类型不变） | `pub fn verify(contract: &Contract, schema: &Value) -> Result<(), ApiError>` | 既有三闸 / 闸一七族 / 自洽闸 / 落点闸逐字不动，末尾追加「散文表格名字 ∩ 契约名字集 = ∅（前置：`ops` / `types` / `reexports` 任一非空）」；不成立即 `ApiError::Contract`。调用点（`api::load_contracts` 的装载序）零改动 |

> 新增的 `collect_surface_table` 与 `surface_table_error` 均为**私有** `fn`（D6），按本表口径不列（只列模块级导出）。公开面仍恰三：`parse` / `verify` / `is_constructor`；`FENCE_OP` / `FENCE_TYPES` 两个 `pub(crate)` 常量零增删。

### 类型定义

<!-- 整段省略：本变更零新增 / 零修改类型。`Contract` / `ContractOp` / `ContractSection` / `ApiError` / `ArchiApiFinding` / `IndexEntry` 全部逐字不变；抽取在 `verify` 期读既有 `sections`，故 `Contract` 零字段新增、`Contract::PartialEq` 的既有断言零随动。 -->

### 配置

| 配置键 | 所在文件 | 变化 | 值类型 | 默认值 / 现值 | 说明 |
|---|---|---|---|---|---|
| `version` | `packages/desktop/package.json` | 修改 | `string` | `0.7.1`（现值 `0.7.0`，+1 patch） | `tauri.conf.json` 经 `"version": "../package.json"` 自动跟随；`src-tauri/Cargo.toml` 的 crate 版本行 MUST NOT 随动 |

> 配置项零新增：`.dev-team/config.json`、`crates/core/config/**`（含 `config.schema.json` 镜像）、`KNOWN_KEYS`、`tauri.conf.json`、`capabilities/default.json` 全部零 diff；`package.json` 的 `api:export` / `api:check` / `check` 三段命令串逐字不变。

<!-- 路由 / API 设计 (Route / API Design) 整段省略：本变更零新增 IPC 命令、零新增 HTTP 端点、零新增 CLI 子命令；`archi-api-gen` 的 `generate` / `--check` / `bootstrap` 三子命令形态与退出码逐字不变，改动全在 core 装载期的一个进程内函数。 -->

---

## 数据模型 (Data Model)

零新增持久化对象，只给**既有手写资产的一处形态**加一条判据。

### 契约资产（手写面，文件零改动）

| 段 / 键 | 形态 | 本变更带来的约束 |
|---|---|---|
| `## <中文二标题>` 段正文的表格行，首列形如 `` `名字` `` | markdown 表格 | 该名字 MUST NOT 同时出现在同文件的三类 fence 名字集（`### <op-id>` 的 op id / `api-types` 键 / `api-reexport` 键）；违反即 `ApiError::Contract`。三项 fence 全空的契约（当前前端 8 份）不受此约束 |
| 段正文的列表项（`` - `名字`：… ``） | markdown 列表 | 不属本约束：它是 `api-reexport` 散文透传列表与留白登记的既有形态，一致性归第三条 gate（既有）；本闸零抽取 |
| 三类 fence（`api-op` / `api-types` / `api-reexport`） | json | **零改动**——仍是名字清单的唯一持有者 |

### 生成物（零变化）

| 产物 | 语义 | 现网读数 |
|---|---|---|
| `<FQN>.schema.json` | `Contract` 的纯派生 | 逐字节不变（闸在 `verify`，不参与渲染） |
| `graph/INDEX.toml` | 索引与计数 | 逐字节不变 |
| `<入口主干>.i.rs` / `<入口主干>.i.ts` | 入口同级生成物 | 逐字节不变 |

### 进程内（零新增）

`collect_surface_table` 的一次调用产出一个 `Vec<String>`（去重保序，随用随弃）；`surface_table_error` 只借交集切片，不产出任何持有超过一次调用的结构。`Contract` 零字段新增。

**持久化面零扩散**：不产生新文件、不改文件落点、零删除面；`render_all` 的产物集合与路径由既有 `paths::*` 单点产出，逐字不变。

---

## 依赖 (Dependencies)

运行时依赖：**零新增**。

| 依赖 | 变化 | 用途 |
|---|---|---|
| `serde_json`（`archi` 已在案） | 零新增 | `Contract` / `Value` 读取（本变更只用 `Contract` 字段与 `String` / `char` 判据） |
| `std` | 零新增 | `str::strip_prefix` / `trim` / `split` / `lines` |

构建 / 测试依赖：**零新增**。`Cargo.lock` 零净增包；`[workspace.dependencies]` / `members` 零改动；前端零触碰，故零 npm 依赖变化（`bindings:export` 应幂等）。

---

## 验收标准回应

| AC | 落点 | 归属（tasks） |
|---|---|---|
| AC-1 抽取面形态封闭 | D1（三重收窄）+ D2（列表项 / 裸正文零抽取）+ 事实校准 1（首列切分）+ 事实校准 5（只覆盖 `##` 段）；反向面：后端 2 份的列表项（含 16 条留白登记）抽不到名字 | 一 |
| AC-2 判据方向钉死为交集为空 | D3 + D4（`ops` / `types` / `reexports` 三源名字集，MUST NOT 取 `$defs`）+ D5（`ApiError::Contract` 文案三件 + 出路句） | 一 |
| AC-3 留白豁免结构性成立 | D3（前置唯一来源）；本仓判据 = 10 份契约全量 `parse` + `build_schema` + `verify` 通过（后端 2 份生效零表格、前端 8 份走豁免）——机械面归 `pilot_test` 一条用例（测试相位落，收面口径已定：递归 `*.md` + `display` 归一） | 一（代码）／测试相位（用例） |
| AC-4 退役触发点可演示 | D3 + D5：留白契约补一个 `api-types` 键（该键由 `build_schema` 进入 `$defs`，第二条 gate 不拦）后表格仍列同名 ⇒ 第四闸点名；删该表格行 ⇒ 通过。演示落 `contract_test` 夹具，MUST NOT 落 `corpus/**` | 一（代码）／测试相位（用例） |
| AC-5 判据面与词汇面零扩散 | D8 + D9 ③④：零新增 finding kind、`check.rs` / `drift/**` 零改动、`apiUndeclared` 仍 `Off`、sidecar 仍 13 份、16 份金样零变化（语料契约零表格行，起点核实已核） | 一 / 四（核对） |
| AC-6 生成物零 diff | D8：闸在 `verify`，不参与 `render_all` / `api_artifacts`；本仓两份后端契约零表格行 ⇒ 装载通过 ⇒ `.schema.json` / `INDEX.toml` / `.i.*` 逐字节不变 | 四（核对）／桌面端门禁承接 |
| AC-7 纪律出处两份 | D7（第 ① 条一句、条目数不变、两文件逐字相同）；事实校准 4 说明「恰 5」为既有陈旧表述，落地口径是「相对 `HEAD` 不变」 | 二 |
| AC-8 留白债在案且不销账 | D9 ⑤（MUST NOT 删表）+ spec `desktop-archi-api` 留白条与 `desktop-archi-api-check` 零判据条（两者已在 delta spec 在案，本变更零追加实现）；错误文案自带出路句（D5），作者不必回溯 spec | 一 / 二 |
| AC-9 零扩散与版本纪律 | D8（`api/mod.rs` 的泛称 doc 也不动）+ 修改文件清单恰 4 行 + 版本表 | 三 / 四 |

> **不列为 AC 的流程项**：`check` 四段链（`server:check` → `bindings:export` → `api:check` → `client:check`）与生成物零 diff 的实际执行由桌面端 static-check 承接；本设计的实现完成判据止于四个文件改动落地 + 阶段四的工作树核对。

---

## 待决问题 (Open Questions)

1. **根文档该节的要点数措辞**：主 spec 与本次 delta spec 都写「恰含五条要点：①…⑤」，而实盘是 6 条（`doc` 条目由 `module-doc-field-docs-retirement` 引入且未同步枚举）。本变更按「条目数相对 `HEAD` 不变」落地（事实校准 4），MUST NOT 为对齐数字删条。是否把 spec 的枚举与计数更正为 6（或把 `doc` 并进第 ④ 条）属独立 spec 修订，建议与下一次碰该节的变更一并处理。
2. **抽取面的收敛预案**（proposal R-2）：若实现期或后续变更出现「参数表 / 对比表 / 枚举值表」的首列恰好与契约名字撞名的真实误伤，收敛路径是「只扫 `## 公共面` 段」——但那会把一个临时段名冻结成机器约定，故本变更 MUST NOT 预留段名白名单或豁免开关，等真误伤再单列裁决。
3. **`pilot_test` 全量 md 收面的副作用**：该用例把「`<archi_models_dir>` 下出现一份非契约的 `*.md`（笔记、临时文件）」变成装载失败。本设计**接受**该副作用（它与「`<archi_models_dir>` SHALL 只容纳契约资产」的既有 spec 条款同源，是活体报警而非误伤）；若将来确需在契约目录放杂项 md，应先改那条资产容器条款，MUST NOT 在该用例里加文件名豁免。
4. **错误定位精度**（proposal R-6 的残留）：文案只有 `file` 与交集名字。若将来单份契约的表格行数增长到「按名字找不到那一行」的规模，再评估在 `parse` 期记录 `(name, line)`——届时须同时处理 `Contract` 字段与 `PartialEq` 断言，属独立变更。
