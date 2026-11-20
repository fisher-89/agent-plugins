# 测试设计: relation-api-dependency-check

> **变更**: relation-api-dependency-check（`[[relation]]` 第四键 `apis` + 名字级两桶判定 + 第 9 条命令 `archi_relation` + 右栏面板两态）
> **来源**: `design.md`（新增文件 3 + 修改文件 22 行 / 23 个路径；修改行内：实现面 13 + 测试面 3 + 数据面 2 + 生成物 3 + 版本 1，新增 3 全为实现面 ⇒ 实现面共 16）、`proposal.md`（AC-1…AC-13）、8 份 delta spec（`desktop-archi-model` / `desktop-archi-drift` / `desktop-archi-canvas` / `desktop-archi-api-check` / `desktop-archi-api` / `repo-archi-model` / `desktop-archi-agent` / `desktop-ipc-type-bindings`）
> **重核留痕**: design「落地面」段已落位（`06-frontend.toml` 32 元素 / 74 关系 + `App.Frontend.GeneratedBindings` + 12 条改指、ADR 七项读数、`INDEX.toml` 45 → 46、`api:check` 零 diff，产品代码零改动）。本稿按落位后实测重核：数据面 / 生成物读数按「落地面」复述，单元测试范围与用例面不变（产品代码零改动 ⇒ 新增用例语义零变化）。
> **AC-8 改写重跑留痕**: proposal / design / tasks 已把 AC-8 由「`scan` 三面逐条相同」改写为「既有条目零增删改 + 新增条目全部命中既有规则」并写实读数（`scannedFiles` / `unmatchedFiles` 412 → 415、`excluded` 341 → 352、候选 753 → 767）。本稿同批校正三处（`## 验收范围` 的 AC-8 行、「AC-8 口径校正」注、`## 不可测试项` 的 `06-frontend.toml` 行）；**单元测试范围与用例面零变化**——扫描面账目是数据面读数（报告 `scan` 段只出计数、条目明细不经报告出线），本就不由单元测试承接；判定面（`observations` 62 → 50）的产出代码零改动，其用例挂靠 `drift/mod.rs` 章节不变。
> **被测面口径**: 只覆盖 design「新增文件」+「修改文件」子表（**删除文件不纳入**，本变更无删除文件）。生成物（`graph/INDEX.toml` / `models/**/*.schema.json` / `types/generated/{bindings,bindings.zod}.ts`）、数据面（`06-frontend.toml` / ADR 计数）、版本行（`package.json`）与既有测试夹具的机械随动记入 `## 不可测试项`——它们是编译 / `tsc` / 静态链强制面，零新增用例语义。
> **测试框架（实测盘点）**: Rust 侧为语言内建 `#[test]` + `assert!` / `assert_eq!` / `assert_ne!`（**无第三方断言库**），测试文件与源文件同级、命名 `<模块>_test.rs`，经父模块 `#[cfg(test)] mod <模块>_test;` 注册；临时盘面用 std 自建的 `TempDir`（`crates/core/archi` 无 dev-dependency），`src-tauri` 侧命令测试用同款自建 `TempDir` + `tauri::test::mock_app()`。crate 级语料为 `crates/core/archi/tests/golden_test.rs` + `tests/corpus/drift/<case>/` 与 `tests/corpus/drift/expected/*.json` 金样，case 经 `DRIFT_CASES` 单点登记（未登记即红）。前端为 `vite-plus/test`（`describe` / `it` / `expect`、`vi.mock` 只替 `@tauri-apps/api/core` 的 `invoke`）+ `@testing-library/react`（`render` / `renderHook` / `waitFor` / `act` / `fireEvent`）+ jsdom；xyflow 只有观测垫片与度量垫片（环境缺口，非业务替身）。本 change 不引新测试依赖。
> **设计约束对齐**: `#### 待测功能` 条目优先取 design「公共函数 / API」表中「所在文件」为该源文件的行；该表无行时取 design「修改文件」子表 / 「类型定义」表为该文件声明的标识符，并在条目下方留 HTML 注释标出处，**不 grep 源文件导出声明虚构条目**；跨模块组合用例按链路发起方 / 最上层调用方挂靠到该入口模块的 per-file 章节（不新建独立集成测试章节）。

---

## 验收范围

| AC ID | 验收条件 | 被测文件或模块 |
|---|---|---|
| AC-1 | 未填 `apis` 的仓：`drift` 报告与既有 16 份金样逐字段相同（零噪声）——冷启动纪律；新增语料 case 只加不改 | `packages/desktop/src-tauri/crates/core/archi/src/drift/mod.rs`、`packages/desktop/src-tauri/crates/core/archi/src/parse/toml.rs`（省略 `apis` 零诊断）、`packages/desktop/src-tauri/crates/core/archi/tests/corpus/drift/**`、`packages/desktop/src-tauri/crates/core/archi/tests/golden_test.rs` |
| AC-2 | 填一条 `apis` 且漏一项 ⇒ 恰 1 条 `undeclaredApi`，档位按 `from.gate`（strict → `errors` / warn → `warnings` / off → 不出现） | `packages/desktop/src-tauri/crates/core/archi/src/parse/toml.rs`（声明面输入）、`packages/desktop/src-tauri/crates/core/archi/src/scan/mod.rs`、`packages/desktop/src-tauri/crates/core/archi/src/scan/rust.rs`、`packages/desktop/src-tauri/crates/core/archi/src/scan/ts.rs`（事实名输入）、`packages/desktop/src-tauri/crates/core/archi/src/drift/mod.rs`、`packages/desktop/src-tauri/crates/core/archi/src/drift/report.rs` |
| AC-3 | 填一条 `apis` 且多一项 ⇒ 恰 1 条 `unusedDeclaration`，且 `status` / `passed` 不变 | `packages/desktop/src-tauri/crates/core/archi/src/model.rs`（`Relation.apis` 三态）、`packages/desktop/src-tauri/crates/core/archi/src/drift/mod.rs`、`packages/desktop/src-tauri/crates/core/archi/src/drift/report.rs` |
| AC-4 | `ArchiDriftKind::ALL.len() == 5`、`ArchiObservationKind::ALL.len() == 2`（未扩）、`as_str` 逐值对拍 | `packages/desktop/src-tauri/crates/core/archi/src/drift/report.rs` |
| AC-5 | `archi_relation` 与 `archi_drift` 对同一 `(from, to)` 给出同一 `usedNotDeclared` / `declaredNotUsed`（同一纯函数；源码复核零第二套判定） | `packages/desktop/src-tauri/crates/core/archi/src/relation.rs`、`packages/desktop/src-tauri/crates/core/archi/src/dto.rs`（`ArchiRelationDetail` / `load` / `diagnostics_error`）、`packages/desktop/src-tauri/src/commands/archi/mod.rs` |
| AC-6 | 报告 `version` 仍为 3、字段集逐字不变（`errors` / `warnings` / `observations` / `apiFindings` 形状不变） | `packages/desktop/src-tauri/crates/core/archi/src/drift/mod.rs`、`packages/desktop/src-tauri/crates/core/archi/src/drift/report.rs`、`packages/desktop/src-tauri/crates/core/archi/src/dto.rs` |
| AC-7 | 点边取数失败：图与元素面板零影响（只落行内错误 + 重试）；未声明 `apis` 的边零取数 | `packages/desktop/src/views/architecture/hooks/use-archi-relation.ts`、`packages/desktop/src/views/architecture/components/relation-info.tsx`、`packages/desktop/src/views/architecture/components/architecture-canvas.tsx` |
| AC-8 | 模型面：55 元素 / 123 关系（容器 9 + 叶 46）、`06` 32 / 74、前端恒等式 **74 = 66 + 8**（barrel 端点边 3 出 + 5 入）；12 条命令面边由 `unfulfilledDeclaration` 转兑现（`observations` 62 → 50）；`model.warnings` 仍恰 9 条 `noPath`。**扫描面按条目级账目验收**（MUST NOT 写成「三面逐条相同」或三面计数相等）：既有 `excluded[]` 条目（`file` / `rule`）与 `unmatchedFiles[]` 路径**零增删改**（数组排序的位序平移不计）、新增条目**逐条可指到本变更新增文件且全部命中既有规则**——`scannedFiles` / `unmatchedFiles` 412 → **415**（+3 源文件）、`excluded` 341 → **352**（+11 测试 / 语料）、候选 753 → **767** 自洽（+14 = 3 + 11）；`matched` / `unresolved` 仍 0 | 数据面 `.dev-team/architecture/graph/model/06-frontend.toml` + `.dev-team/architecture/decisions/0001-model-granularity.md`（见 `## 不可测试项`）；模型面读数产出面 = `drift/mod.rs` 两桶装配 + `scan/{mod,rust,ts}.rs` 名字事实 + `parse/toml.rs` 声明面；扫描面条目级账目由桌面端命令 + 报告集合对拍采集（报告 `scan` 段只出计数、条目明细不经报告出线，见 `## 不可测试项`） |
| AC-9 | api 基线：本仓分母 46、`docMissing` 36（33 declared + 3 none）、其余十二值 0、`status = clean`、`passed = true`；`api/**` sidecar 仍 13 份 | `.dev-team/architecture/graph/model/06-frontend.toml` + `api:check`（数据面读数，见 `## 不可测试项`） |
| AC-10 | 生成物零手改（`INDEX.toml` 46 条、`bindings.ts` / `bindings.zod.ts` 的新类型与新命令只来自重导）；清单守卫四处计数随动 | `packages/desktop/src-tauri/crates/core/archi/src/lib.rs`（出口面）、`packages/desktop/src-tauri/crates/core/archi/src/dto.rs`（新 DTO 出线）、`packages/desktop/src-tauri/src/commands/mod.rs`、`packages/desktop/src-tauri/src/bindings/mod_test.rs`、`packages/desktop/src/types/generated/**`、`.dev-team/architecture/graph/INDEX.toml`、`.dev-team/architecture/models/**/*.schema.json`（生成物，见 `## 不可测试项`） |
| AC-11 | 知识文本：关系四键在场、`apis` 一句在场、`apis` 不进派生词表、体量锚不破 | `packages/desktop/src-tauri/crates/infra/agent/src/archi-knowledge.md` |
| AC-12 | 全管线绿：`pnpm -C packages/desktop run check` 与 `run test` 全通过 | 全仓（静态检查链 + 全量测试，见 `## 不可测试项`） |
| AC-13 | 版本纪律：`package.json` = `0.7.5`，`tauri.conf.json` 跟随、`src-tauri/Cargo.toml` 版本不随动 | `packages/desktop/package.json`（见 `## 不可测试项`） |

> AC-8 / AC-9 的读数与 AC-12 / AC-13 的门禁由桌面端命令与静态链采集（本相位不执行）；单元测试只覆盖产出这些读数的判定面与命令面（`drift` 两桶装配、`scan` 名字事实、`relation_detail`、清单守卫）。
>
> **AC-8 口径校正（proposal「事实校正」#9 ／ design 事实校准 8 / 「落地面」§5）**: 旧稿的「`scan` 三面逐条相同」是**结构性不可达**——`unmatchedFiles[]` 本就收纳 `gate = off` 的归属文件与无主文件（本仓 `gate` 全 `off`）、`excluded[]` 收纳既有排除规则命中项 ⇒ 本变更新增的 3 个源文件必然进 `unmatchedFiles[]`、11 份测试 / 语料必然进 `excluded[]`（1 个 `**/*_test.rs` + 8 个 `**/tests/**` 语料 TS + 2 个前端 `**/*.test.ts(x)`）。正确口径 = **条目级账目**：① 排除规则集零 diff（`DEFAULT_EXCLUDE_RULES` 13 条 + 配置追加 `openspec/**`）；② 既有源文件零删除 / 零重命名 ⇒ 既有 412 条 `unmatchedFiles` 与 341 条 `excluded` 的条目集合（含 `file` / `rule`）零增删改；③ 新增候选 14 条逐条点名并各带命中规则（3 源文件 → `unmatchedFiles[]`；1 个 `relation_test.rs` 命中 `**/*_test.rs`；`api-absent` / `api-declared-exact` / `api-declared-not-used` / `api-used-not-declared` 四 case 各 2 份语料 TS → 8 条命中 `**/tests/**`；`relation-info.test.tsx` / `use-archi-relation.test.ts` → 命中 `**/*.test.ts(x)`）。读数：`scannedFiles` / `unmatchedFiles` **412 → 415**、`excluded` **341 → 352**、候选 **753 → 767**（+14 = 3 源 + 11 测试 / 语料）。3 个生成文件前后都在 `unmatchedFiles` 内（`gate = off` 的既有分支）。`gate` 全 `off` 使 `matched` 恒空，故 MUST NOT 用 `scannedFiles` / `unmatchedFiles` / `excluded` 的计数相等充当「归属未变」或「判定面零变化」的证据（归属兑现判据是 `observations` 62 → 50）；扫描面账目本身是数据面读数（报告只出计数），不进单元测试，见 `## 不可测试项`。
>
> **测试承接面（design「修改文件」表所列测试文件）**: `drift/report_test.rs` ↔ AC-4 / AC-6、`src-tauri/src/bindings/mod_test.rs` ↔ AC-10、`crates/infra/agent/src/archi_knowledge_test.rs` ↔ AC-11——三者本身是测试文件（改动为计数 / 冻结值 / 清单随动），其用例已挂靠对应源文件章节（`drift/report.rs` / `commands/mod.rs` / `archi-knowledge.md`），文件级说明见 `## 不可测试项`。另有 `relation_test.rs`（新）与 `lib.rs` 的 `#[cfg(test)] mod relation_test;` 同批承接面（对应 `relation.rs -> relation_test.rs（新）` 章节）。

---

## 单元测试

### packages/desktop/src-tauri/crates/core/archi/src/parse/toml.rs -> packages/desktop/src-tauri/crates/core/archi/src/parse/toml_test.rs

#### 待测功能

- `TomlParser::parse(source): 诊断来源由「只有 serde」扩为「serde + to_model 值面校验」，ParseOutcome 形状不变（失败臂仍 Model::empty()）`
- `RawRelation.apis（图字段面）: 第四键反序列化（Spanned 保位置）+ deny_unknown_fields 保留`
- `validate_apis（私有面）: 值面五拒（空串 / 含空白 / 含 :: / 同名重复 / 显式空数组）各出 invalidValue 并带位置；to_model 返回 (Model, Vec<Diagnostic>)`

<!-- 出处：`TomlParser::parse` 取「公共函数 / API」表该文件行；`RawRelation.apis` / `validate_apis` / `to_model` 为该表未列的私有面，条目取 design「修改文件」子表 `parse/toml.rs` 行（契约由 D3 / D4 与 AC-2 / AC-3 承载），不 grep 导出声明。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `[[relation]].apis` 解析 | 正向 | `apis = ["AgentInput", "SessionPanel"]` → `Relation.apis` 逐名且**原文序**（零排序、零去重） | 单例 |
| `[[relation]].apis` 解析 | 正向 | 省略 `apis` → `Relation.apis == []` 且 `diagnostics` 为空（缺省不产诊断） | 单例 |
| `[[relation]].apis` 解析 | 边界 | 单元素数组 `apis = ["AgentInput"]` → 一名（数组与标量类型面区分：`apis = "AgentInput"` 另测） | 单例 |
| `[[relation]].apis` 解析 | 边界 | 空数组 `apis = []`（显式）→ **恰一条** `invalidValue`，文案点名「空数组与省略同义」 | 单例 |
| `[[relation]].apis` 解析 | 边界 | 超长名（单个名字 > 1000 chars，无空白无 `::`）→ 零诊断（值面只四查，不设长度上限） | 单例 |
| `[[relation]].apis` 解析 | 边界 | 特殊字符名（`"Agent🙂"` / `"a\"b"`）→ 无空白无 `::` 即通过，原样入模型 | 表驱动（2 行） |
| `[[relation]].apis` 解析 | 异常 | 空串名 `apis = [""]` → 恰一条 `invalidValue`，`message` 点名「只写名字」，`line` / `column` 指向该名字面 | 单例 |
| `[[relation]].apis` 解析 | 异常 | 含空白（空格 / 制表 / 换行三形态 `"a b"` / `"a\tb"` / `"a\nb"`）→ 各一条 `invalidValue` | 表驱动（3 行） |
| `[[relation]].apis` 解析 | 异常 | 含 `::`（`"a::b"` / `"crate::A"`）→ 各一条 `invalidValue`（路径形态不入值面） | 表驱动（2 行） |
| `[[relation]].apis` 解析 | 异常 | 同名重复 `["A", "A"]` → 恰**一条** `invalidValue`（不静默去重、不按重复次数铺开） | 单例 |
| `[[relation]].apis` 解析 | 异常 | 类型面违例：`apis = "A"`（标量）/ `apis = [1, 2]`（非字符串）→ serde `invalid type` 归 `invalidValue`，`Model::empty()` | 表驱动（2 行） |
| `[[relation]].apis` 解析 | 边界 | 拼错键 `api = ["A"]`（单数）→ 仍 `unknownField`（`deny_unknown_fields` 逐字保留） | 单例 |
| `[[relation]].apis` 解析 | 边界 | 多条关系各带 `apis`：一条违例不影响其余关系入模型；诊断条数 = 违例条数（不是「首错短路」） | 表驱动（2 行：单违例 / 双违例） |
| `[[relation]].apis` 解析 | 边界 | 关系缺 `to`（必填面）→ 既有 `missingField` 语义不变（`apis` 不改变必填面） | 单例 |
| `TomlParser::parse` 诊断合并 | 正向 | 健康源（含 `apis`）→ `diagnostics` 空、`relations` 逐条齐备、`ParseOutcome` 两字段形状不变 | 单例 |
| `TomlParser::parse` 诊断合并 | 边界 | 语法错源 → **恰一条** serde 诊断且 `Model::empty()`（合并面不追加值面诊断） | 表驱动（2 行：缺括号 / 重复键） |
| `TomlParser::parse` 诊断合并 | 边界 | 一处 serde 失败 + 同文件另有值面违例 → 仍只出 serde 诊断（整体失败即不取值面，零二段自证） | 单例 |
| `TomlParser::parse` 诊断合并 | 异常 | 值面多违例（两条关系各一违例）→ 两条诊断，`file` 仍留空（聚合层回填），各自 `line` / `column` 独立 | 单例 |
| `validate_apis` | 正向 | 四查全过（非空 / 无空白 / 无 `::` / 不重复）→ 零诊断 | 表驱动（3 行：单名 / 多名 / 乱序多名） |
| `validate_apis` | 边界 | 诊断消息含关系上下文（`from -> to` 或等价定位串），可回指该条关系 | 单例 |
| `validate_apis` | 异常 | 五类违例逐类 → 各恰一条、`kind == invalidValue`、文案点名对应纪律（名字 / 空白 / `::` / 重复 / 空数组） | 表驱动（5 行） |
| `validate_apis` | 边界 | 同一关系内「重复 + 含空白」叠加 → 至少各条纪律都被点名（不因首条即返回而漏第二条纪律） | 单例 |
| `Relation` 字面量夹具随动 | 组合 | `toml_test` 内既有关系断言（`from` / `to` / `description` / `file` / `line` / `span`）逐条保持绿，仅补 `apis` 断言 | 表驱动（回归） |
| `Relation` 字面量夹具随动 | 正向 | `toml_test` 既有全字段用例（含 `[[relation]]` 段）逐条保持绿，`Relation.apis` 与源文本同值 | 单例 |
| `Relation` 字面量夹具随动 | 异常 | 值面违例源的既有诊断断言（条数 / `kind`）不受 `apis` 缺省影响（缺省不产诊断） | 表驱动（2 行） |
| `Relation` 字面量夹具随动 | 边界 | 同文件两条关系（一条全字段带 `apis`、一条仅三键）→ 各自独立解析、`apis` 分别非空 / 空 | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock） | 只吃 `&str`：真实 `toml` crate 的 span / 原文案即被测契约（替身会抹掉行列判据） | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/archi/src/model.rs -> packages/desktop/src-tauri/crates/core/archi/src/model_test.rs

#### 待测功能

- `Relation（类型）: 新增 apis: Vec<String>（模型原文序；缺省空 = 不声明 API 粒度；file / line / span 零变化）`

<!-- 出处：「公共函数 / API」表无本文件行（`Relation` 是纯结构体，无新增函数出口）；条目取 design「类型定义」表与「修改文件」子表 `model.rs` 行（契约由 D3 与 AC-3 承载），不 grep 导出声明。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `Relation.apis` 三态 | 正向 | 直构 `apis = vec!["A".into(), "B".into()]` → 逐字段可读、序 = 声明序（零归一） | 单例 |
| `Relation.apis` 三态 | 边界 | `apis: Vec::new()`（= 省略）→ `apis.is_empty()` 且其余六字段不受影响 | 单例 |
| `Relation.apis` 三态 | 边界 | 六处既有字面量夹具随动（`diff_test` 1 / `graph_test` 2 / `model_test` 3）→ 编译期闭合、`apis == Vec::new()` | 表驱动（6 行） |
| `Relation.apis` 三态 | 边界 | 直构含重复名 `["A", "A"]`（绕过 parse 值面）→ 原样保留（模型层零去重，归一在 `api_buckets`） | 单例 |
| `Relation.apis` 三态 | 异常 | 同其余字段、仅 `apis` 不同 → `!=`（结构比较语义随字段扩展）；六字段全等 → `==` | 表驱动（2 行） |
| `Relation` 周边零变化 | 边界 | `file` / `line` / `span` 三字段语义与改造前逐字一致（字面量 + `span.is_some()` 断言） | 单例 |
| `Relation` 周边零变化 | 边界 | `Element` / `SourceSpan` / `Diagnostic` / `ParseOutcome` 字段集快照不变（结构体字面量全字段构造即编译期闭合） | 单例 |
| `Relation` 周边零变化 | 组合 | `TomlParser::parse` 产物与本文件直构产物逐字段相等（`file` 留空 / `line` = `from` 键行 / `span` 由 span 换算） | 单例 |
| `Relation` 周边零变化 | 正向 | `Model::element(fqn)` 命中 / 未命中两态与 `Model::empty()` 语义不变（扩展字段不改查找面） | 表驱动（3 行） |
| `Relation` 周边零变化 | 异常 | `apis` 为 `Vec::new()` 而其余字段任意 → 结构比较零 panic，`Diagnostic` 面零变化 | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock） | 纯结构体直构与结构比较；组合行复用真实 `TomlParser::parse`（不落盘） | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/archi/src/scan/mod.rs -> packages/desktop/src-tauri/crates/core/archi/src/scan/mod_test.rs

#### 待测功能

- `normalize_names(names): 去重 + 字典序单点（BTreeSet 语义；Rust / TS 两个抽取器共用）`
- `ImportFact::new(specifier, line, type_only, names): 唯一构造点（归一化在内，抽取器零二次归一）`
- `ImportFact.names / DependencyEdge.names（类型字段）: 空 = 取不到名字；成边处原样透传（零二次归一）`

<!-- 出处：`normalize_names` / `ImportFact::new` 取「公共函数 / API」表该文件行；两处字段取「类型定义」表行（契约由 D7 与 AC-2 / AC-3 承载）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `normalize_names` | 正向 | `["b", "a", "b"]` → `["a", "b"]`（去重 + 字典序） | 单例 |
| `normalize_names` | 边界 | 空迭代器 → `[]`；单元素 `["A"]` → `["A"]` | 表驱动（2 行） |
| `normalize_names` | 边界 | 超大列表（1200 名，乱序 + 大量重复）→ 长度 = 唯一名数、严格升序、零 panic | 单例 |
| `normalize_names` | 边界 | 大小写不同（`"Agent"` / `"agent"`）→ 两个名（逐字符，禁大小写折叠） | 单例 |
| `normalize_names` | 边界 | 已排序输入 → 输出逐字相同（幂等） | 单例 |
| `normalize_names` | 异常 | 特殊字符（emoji / `"a::b"` / `""` 空串元素）→ 原样保留、零过滤零 panic（值面校验不在此处） | 表驱动（3 行） |
| `ImportFact::new` | 正向 | `new("@/x", 12, false, ["B", "A"])` → `names == ["A", "B"]`，`specifier` / `line` / `type_only` 原样 | 单例 |
| `ImportFact::new` | 边界 | 空 names → `[]`（glob / `self` / default / namespace / `export *` 的合法形态） | 单例 |
| `ImportFact::new` | 边界 | 重复 names `["A", "A"]` → `["A"]` | 单例 |
| `ImportFact::new` | 边界 | `type_only = true` 与 names 共存 → 名字照产（type-only 只标注、不豁免） | 单例 |
| `ImportFact::new` | 异常 | 名字含空白 / `::`（抽取器理论不产）→ 仍按字符归一（本构造器零值面校验，零 panic） | 表驱动（2 行） |
| `ImportFact.names` / `DependencyEdge.names` 透传 | 正向 | 端到端（tempdir：A 元素文件 `use crate::b::B`，B 元素覆盖该文件）→ `edge.names == fact.names` 且逐字相等 | 单例 |
| `ImportFact.names` / `DependencyEdge.names` 透传 | 边界 | 同一元素对两条边（两文件各一命中）→ 每条边各带自己的 names（聚合留给 `drift::edge_names`） | 单例 |
| `ImportFact.names` / `DependencyEdge.names` 透传 | 边界 | 无名字形态（`export * from` / `use a::*`）→ `edge.names == []`（不产假名） | 表驱动（2 行） |
| `ImportFact.names` / `DependencyEdge.names` 透传 | 异常 | 目标不可归属的边（`Owner::None` / `Owner::Ambiguous`）→ 仍丢弃、零边产出（names 不改变丢弃语义） | 表驱动（2 行） |
| `ImportFact.names` / `DependencyEdge.names` 透传 | 边界 | `gate = off` 引入方的边 → 照常带 names（定级不在扫描面） | 单例 |
| 归一单点（源码自证） | 组合 | `rust.rs` / `ts.rs` 内 `ImportFact {` 结构体字面量零命中（构造点收敛到 `ImportFact::new`）；`normalize_names` 全 crate 仅一处实现 | 单例 |
| 归一单点（源码自证） | 正向 | `normalize_names` 在 `scan/mod.rs` 恰一处实现，`drift::edge_names` 只转调不复制 | 单例 |
| 归一单点（源码自证） | 异常 | 任一抽取器新增 `ImportFact { … }` 字面量（绕过 `new`）→ 源码断言红并点名文件 | 表驱动（2 行：`rust.rs` / `ts.rs`） |
| 归一单点（源码自证） | 边界 | 调用点面：`drift::edge_names` 与 `ImportFact::new` 各恰好一处转调 `normalize_names`（无第三个转调点） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock） | 真实 std 自建 `TempDir` 盘面（模型 + 源文件）+ 真实 `OwnerIndex` / 两套解析器；归一与构造为纯函数 | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/archi/src/scan/rust.rs -> packages/desktop/src-tauri/crates/core/archi/src/scan/rust_test.rs

#### 待测功能

- `RustResolver::extract(file_rel, source): 签名不变；每条 fact 带 names（别名取被引入名、self / glob 空）`
- `RustResolver::resolve(file_rel, specifier): 零改动（名字不走前缀匹配，签名与语义逐字不变）`
- `flatten_use_tree（私有面）: 一次产出「路径 + 可选尾段」两列`

<!-- 出处：`extract` / `resolve` 取「公共函数 / API」表该文件行；`flatten_use_tree` 为该表未列的私有面，条目取 design「修改文件」子表 `scan/rust.rs` 行（契约由 D5 承载）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `extract` 产名 | 正向 | `use crate::b::B;` → 一条 fact，`names == ["B"]` | 单例 |
| `extract` 产名 | 正向 | `use crate::b::{B, C};` → 两条 fact，各带 `["B"]` / `["C"]`（一条声明两 facts 的既有形态不变） | 单例 |
| `extract` 产名 | 正向 | `use crate::b::{B, C as D};` → `["B"]` 与 **`["C"]`**（被引入名，非别名 `D`） | 单例 |
| `extract` 产名 | 正向 | `use crate::b::B as W;` → `names == ["B"]`（`Rename` 取 `node.ident`） | 单例 |
| `extract` 产名 | 正向 | `use self::x::B;` → `["B"]`（`self` 非末段照常产名） | 单例 |
| `extract` 产名 | 边界 | `use crate::b::*;` → 一条 fact，`names == []` | 单例 |
| `extract` 产名 | 边界 | `use crate::b::{self};`（末段恰 `self`）→ `names == []` | 单例 |
| `extract` 产名 | 边界 | 嵌套 group `use a::{b::{C, D}, E};` → 三条 fact，names 分别 `C` / `D` / `E` | 单例 |
| `extract` 产名 | 边界 | 同一文件多条 `use` 声明 → 每条独立 fact（行号各自对应），跨声明零合并 | 单例 |
| `extract` 产名 | 边界 | 两条 `use a::B;` 重复声明 → 两条 fact 各 `["B"]`（去重只在判定侧） | 单例 |
| `extract` 产名 | 边界 | 名字极长（> 1000 chars）与多字节标识符 → 原样、零 panic | 表驱动（2 行） |
| `extract` 产名 | 异常 | 语法错源 → 一条 `unresolved`（「Rust 源文件解析失败」）、零 fact、零 panic | 单例 |
| `extract` 产名 | 异常 | 非 `use` 项（`fn` / `impl` / `mod` / `struct`）→ 零 fact（`Item::Use` 之外一律跳过） | 表驱动（4 行） |
| `extract` 产名 | 异常 | `use std::collections::HashMap;`（外部 crate）→ fact 照出、`names == ["HashMap"]`（产名与可否落盘解耦） | 单例 |
| `flatten_use_tree` | 正向 | 四形态（`Name` / `Rename` / `Glob` / `Group`）各一条 → 「路径 + 尾段」两列逐值 | 表驱动（4 行） |
| `flatten_use_tree` | 边界 | 空 prefix + `Glob`（`use *;` 不可解析形态由 `syn` 拒）→ 不产路径、零 panic | 单例 |
| `resolve` 零改动 | 正向 | 四形态（`crate::` / `self::` / `super::` / 首段 crate 名）逐条落盘结果与改造前一致 | 表驱动（4 行） |
| `resolve` 零改动 | 边界 | 尾段为类型名（`use crate::b::B`）→ 仍回模块文件（`b.rs` / `b/mod.rs`），尾段不参与匹配 | 表驱动（2 行） |
| `resolve` 零改动 | 异常 | 外部 crate / 空 specifier / 畸形 `::x` → `None`，零 panic | 表驱动（3 行） |
| `extract` 与 `resolve` 分工 | 组合 | 同一 `use crate::b::{B, C}` → 两 facts（names 各异）但 `resolve` 两 specifier 落到同一文件（模块前缀被吃掉） | 单例 |
| `flatten_use_tree` | 异常 | `Rename` 的 `node.ident` 与 `rename` 不同名（`use a::B as W`）→ 产 `B` 而非 `W`（别名面反例，与 D5 同向） | 单例 |
| `extract` 与 `resolve` 分工 | 边界 | 同模块多形态（`use crate::b;` 模块自身 + `use crate::b::*;`）→ 落盘同文件、names 分别 `[]`（globs）与非空 | 表驱动（2 行） |
| `extract` 与 `resolve` 分工 | 正向 | 同一条 `use` 的 names 与落盘结果同次遍历产出（一次 `syn` 解析，零二次扫文本） | 单例 |
| `extract` 与 `resolve` 分工 | 异常 | 落盘失败（外部 crate / 越界）而 names 已产 → fact 照出，边在扫描层丢弃（两面互不污染） | 表驱动（2 行：外部 crate / 越界） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock） | 真实 `syn 2`（`features = ["full"]`）+ 真实 std 自建 `TempDir`（`Cargo.toml` workspace 清单 + `src/lib.rs` 模块树）；零子进程（不调 `cargo metadata`） | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/archi/src/scan/ts.rs -> packages/desktop/src-tauri/crates/core/archi/src/scan/ts_test.rs

#### 待测功能

- `TsResolver::extract(file_rel, source): 签名不变；具名 specifier 产名（`type` 前缀剥除、照产名），default / namespace / `export *` 不产名`

<!-- 出处：`extract` 取「公共函数 / API」表该文件行；契约由 D6 与 AC-2 承载。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `extract` 产名 | 正向 | `import { A, B } from '@/types/generated/bindings';` → `names == ["A", "B"]`，`type_only == false` | 单例 |
| `extract` 产名 | 正向 | `import { A, type B } from 'x';` → `names == ["A", "B"]`（混合形态 `type_only` 保守取 false） | 单例 |
| `extract` 产名 | 正向 | `import type { A } from 'x';` → `names == ["A"]`、`type_only == true`（type-only 照产名） | 单例 |
| `extract` 产名 | 正向 | `export { A } from 'x';` → `names == ["A"]`（与 import 同取法） | 单例 |
| `extract` 产名 | 正向 | `export { A as B } from 'x';` → `names == ["A"]`（`local` 名，别名不入值） | 单例 |
| `extract` 产名 | 正向 | `export type { A } from 'x';` → `names == ["A"]`、`type_only == true` | 单例 |
| `extract` 产名 | 边界 | 字符串导出名 `import { "a-b" as X } from 'x';` → `names == ["a-b"]`（`ModuleExportName::StringLiteral` 取字面值） | 单例 |
| `extract` 产名 | 边界 | `import X from 'x';`（default）→ `names == []` | 单例 |
| `extract` 产名 | 边界 | `import * as ns from 'x';`（namespace）→ `names == []` | 单例 |
| `extract` 产名 | 边界 | `export * from 'x';`（`ExportAllDeclaration`）→ `names == []` | 单例 |
| `extract` 产名 | 边界 | `import {} from 'x';`（空具名清单）→ `names == []` | 单例 |
| `extract` 产名 | 边界 | 大小写不同（`{Agent, agent}`）→ 两个名（逐字符，零折叠） | 单例 |
| `extract` 产名 | 边界 | 超长名字（> 1000 chars 标识符）→ 原样；名字含 emoji（字符串导出名）→ 原样，零 panic | 表驱动（2 行） |
| `extract` 产名 | 边界 | 同 specifier 两声明（`import { A }` + `import X from`）→ 两条 fact（不合并），names 分别 `["A"]` / `[]` | 单例 |
| `extract` 产名 | 异常 | 语法错源 → `unresolved` + `parsed.panicked` 早退（零 fact） | 单例 |
| `extract` 产名 | 异常 | 动态 `import('x')` / `require('x')` → `unresolved`（既有语义），names 面不产 fact | 表驱动（2 行） |
| `extract` 产名 | 异常 | 同一文件混入两处语法错 → 逐条 `unresolved`，零 panic | 单例 |
| `extract` 产名 | 组合 | names 与 specifier 落盘互不影响：`resolve` 仍只吃 specifier（同文件 names 各异、落盘同值） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock） | 真实 `oxc_parser` / `oxc_resolver` + 真实 std 自建 `TempDir`（`tsconfig` 就近配置）；抽取为纯字符串入参 | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/archi/src/drift/mod.rs -> packages/desktop/src-tauri/crates/core/archi/src/drift/mod_test.rs

#### 待测功能

- `api_buckets(declared, used): 求差唯一实现，返回 (used ∖ declared, declared ∖ used)；两侧去重 + 字典序、逐字符比对`
- `edge_names(edges): 元素对事实名并集（去重 + 字典序，转调 scan::normalize_names）`
- `occurrences_of(edges): 可见性提 pub(crate)，函数体逐字不变（按 file / line 排序的命中点聚合）`
- `check(input): 签名与既有语义不变；⑦ 之后追加两桶装配段（仅 apis 非空的关系，档位只看引入方）`

<!-- 出处：四条均取「公共函数 / API」表该文件行（契约由 D8 / D9 / D10 与 AC-2 / AC-3 / AC-5 / AC-6 承载）。语料 case 与金样对拍（`tests/corpus/drift/**` / `tests/golden_test.rs`）作为 `check` 的端到端用例挂靠本章（入口模块 = 判定面），不另设集成章节。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `api_buckets` | 正向 | 全匹配（declared `["A","B"]` / used `["B","A"]`）→ `([], [])` | 单例 |
| `api_buckets` | 正向 | 少一项（declared `["A","B"]` / used `["A"]`）→ `used∖declared = []`、`declared∖used = ["B"]` | 单例 |
| `api_buckets` | 正向 | 多一项（declared `["A"]` / used `["A","C"]`）→ `(["C"], [])` | 单例 |
| `api_buckets` | 正向 | 两侧各有差（declared `["A","B"]` / used `["B","D"]`）→ `(["D"], ["A"])`（两桶同序字典序） | 单例 |
| `api_buckets` | 边界 | 双空 → `([], [])`（`apis` 省略的关系零产） | 单例 |
| `api_buckets` | 边界 | 乱序 + 重复（declared `["A","A","B"]` / used `["C","A","C"]`）→ 输出恒去重 + 字典序 | 单例 |
| `api_buckets` | 边界 | 大小写不同（`Agent` / `agent`）→ 两侧各出一项（逐字符比对，禁折叠 / 驼峰归一） | 单例 |
| `api_buckets` | 边界 | 边界值：空串名（直构）/ 单字符名 / 超长名（> 1000 chars）→ 按字符参与比较，零 panic | 表驱动（3 行） |
| `api_buckets` | 边界 | 超大列表（各 1200 名，仅一项差）→ 恰一项差异（零 panic、可接受的耗时） | 单例 |
| `api_buckets` | 异常 | declared 含 `::` / 空白（直构，绕过模型值面）→ 仍按逐字符比较（纯函数不校验值面，也不静默剔除） | 表驱动（2 行） |
| `api_buckets` | 组合 | 交换入参 → 两桶互换（对称语义，零第二套规则） | 单例 |
| `edge_names` | 正向 | 两条边 names `["B"]` / `["A","B"]` → `["A","B"]` | 单例 |
| `edge_names` | 边界 | 零边 → `[]`；边 names 全空（default / glob）→ `[]` | 表驱动（2 行） |
| `edge_names` | 边界 | 跨边重复名 → 去重；跨边乱序 → 字典序 | 单例 |
| `edge_names` | 组合 | 与 `scan::normalize_names` 直调同一输入逐字相等（单一归一实现） | 表驱动（3 行） |
| `occurrences_of` | 正向 | 多文件多行 → 按 `file` 再 `line` 升序；`specifier` / `type_only` 原样 | 单例 |
| `occurrences_of` | 边界 | 零边 → `[]`；同文件多行（乱序输入）→ 升序 | 表驱动（2 行） |
| `occurrences_of` | 边界 | `line = 0`（未知位置）→ 排最前，零 panic | 单例 |
| `occurrences_of` | 组合 | 与 `relation_detail.occurrences` 对同一元素对逐字段相等（同形复用，零第二份聚合） | 单例 |
| `check` 两桶装配 | 正向 | strict 引入方 + used 多出（declared `["A"]` / 事实 `["A","C"]`）→ `errors` 恰 1 条 `undeclaredApi`，`gate = Some(strict)`，`message` 点名 `C` 与档位 | 单例 |
| `check` 两桶装配 | 正向 | warn 引入方同形态 → `warnings` 恰 1 条，`errors` 空、`status = warnings`、`passed = true` | 单例 |
| `check` 两桶装配 | 正向 | off 引入方同形态 → 零条目（`errors` / `warnings` / `observations` 皆不出现该名字） | 单例 |
| `check` 两桶装配 | 正向 | declared 多一项（事实少 `B`）→ `observations` 恰 1 条 `unusedDeclaration`，`status = clean`、`passed = true` | 单例 |
| `check` 两桶装配 | 边界 | 关系省略 `apis`（冷启动）→ 零新增条目（`errors` / `warnings` / `observations` 三数组与本变更前逐条相同） | 单例 |
| `check` 两桶装配 | 边界 | declared 与 used 完全相同 → 两桶皆空、零条目 | 单例 |
| `check` 两桶装配 | 边界 | 同一元素对同时无 import 且 `apis` 非空 → `unfulfilledDeclaration` 与 `unusedDeclaration` 并存（两条事实不同，允许并存） | 单例 |
| `check` 两桶装配 | 边界 | 观察项文案并列两成因（「不再依赖」+「名字写错或扫描看不见」）且点名「本对无 import 时另有 `unfulfilledDeclaration` 一条」（R4 / R5） | 单例 |
| `check` 两桶装配 | 边界 | 观察项文案点名三条取数上限（方法调用 / 内联限定路径 / glob） | 单例 |
| `check` 两桶装配 | 异常 | `from` 元素缺席于 `elements`（gate 取不到，直构 DTO 面）→ 跳过装配、零 panic | 单例 |
| `check` 两桶装配 | 异常 | 两桶条目的 `occurrences` 与该元素对全部命中边逐条同形（按 file / line 排序，`line` 非 0） | 单例 |
| `check` 两桶装配 | 边界 | 报告字段集与 `version`：顶层键集逐字不变、`version == 3`（AC-6） | 单例 |
| `check` 两桶装配 | 边界 | 定级分支单点：`errors.push` / `warnings.push` 只出现在既有 ⑥ 与新增两桶段（源码复核，无第三处） | 单例 |
| `check` 语料对拍 | 组合 | 既有 16 份 `tests/corpus/drift/expected/*.json` 与新增两桶后逐字节不变（`apis` 省略的语料零产；AC-1） | 表驱动（16 行） |
| `check` 语料对拍 | 组合 | 新增四 case（`api-declared-exact` / `api-used-not-declared` / `api-declared-not-used` / `api-absent`）登记进 `DRIFT_CASES` 且各自 expected 逐字段对拍 | 表驱动（4 行） |
| `check` 语料对拍 | 异常 | 新增 case 未登记 / 登记了不存在的 case → 既有「单点登记面」断言仍红（零旁路） | 表驱动（2 行） |
| `check` 语料对拍 | 组合 | 新增 case 的 `errors[]` / `warnings[]` / `observations[]` 条目键集与既有金样条目逐键相同（只多取值，不扩字段） | 表驱动（4 行） |
| `edge_names` | 异常 | 边 names 含重复 + 空串元素（直构）→ 去重 + 排序、零 panic（本函数零值面校验） | 单例 |
| `occurrences_of` | 异常 | `specifier = None` 形态（直构，如 `path` 类留痕）→ 原样保留、零 panic | 单例 |
| `check` 语料对拍 | 正向 | 既有 16 case 在改造后逐 case 与金样逐字段相等（零噪声，AC-1） | 表驱动（16 行） |
| `check` 语料对拍 | 边界 | 新增 `api-absent` case（关系省略 `apis`）→ `observations` / `errors` / `warnings` 与该盘面基线逐条相同 | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock） | 真实 std 自建 `TempDir`（模型 / 源文件 / 架构域 `config.json`）+ 真实 `dto::graph_view` + 真实两套解析器；时钟与 `modelHash` 只断言形态与变化性（既有纪律） | 全部用例 |
| 无（语料面零 mock） | `tests/corpus/drift/**` 为真实盘面 + 真实 `check`，金样 `expected/*.json` 是判定产物本身 | 语料对拍用例 |

---

### packages/desktop/src-tauri/crates/core/archi/src/drift/report.rs -> packages/desktop/src-tauri/crates/core/archi/src/drift/report_test.rs

#### 待测功能

- `ArchiDriftKind::as_str(self): 补 "undeclaredApi"；ALL 扩为 [ArchiDriftKind; 5]（新值置末）`
- `ArchiObservationKind::UnusedDeclaration（枚举变体）: 产出点亮为「declared ∖ used 时产出」，ALL 仍二值`

<!-- 出处：`as_str` 取「公共函数 / API」表该文件行；`UnusedDeclaration` 取「类型定义」表行（契约由 D8 / D18 与 AC-3 / AC-4 承载）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ArchiDriftKind::as_str` | 正向 | 五值逐值 → `undeclaredDependency` / `pathNotFound` / `ambiguousOwner` / `noPath` / `undeclaredApi` | 表驱动（5 行） |
| `ArchiDriftKind::as_str` | 边界 | `ALL.len() == 5`、新值置末、`ALL` 与 `as_str` 逐值互映且无重名 | 单例 |
| `ArchiDriftKind::as_str` | 边界 | 四旧值 `as_str` 逐字不变（既有条目零位移） | 表驱动（4 行） |
| `ArchiDriftKind::as_str` | 异常 | 非法串反序列化（`"undeclaredAPI"` / `"UndeclaredApi"` / `""` / `"other"`）→ `Err`（封闭五值，零第六值） | 表驱动（4 行） |
| `ArchiDriftKind::as_str` | 组合 | 序列化线格式 camelCase：`serde_json::to_value` 的 `kind` 为 `"undeclaredApi"`（与 `as_str` 同串） | 单例 |
| `ArchiDriftKind::as_str` | 边界 | 条目键集（`kind` / `from` / `to` / `gate` / `message` / `occurrences`）逐字不变（新增取值不扩字段，AC-6） | 单例 |
| `ArchiObservationKind` | 正向 | `ALL.len() == 2`（未扩）、两值 `as_str` 逐值对拍 | 表驱动（2 行） |
| `ArchiObservationKind` | 边界 | `UnusedDeclaration` 序列化/反序列化线格式 `"unusedDeclaration"` 往返一致 | 单例 |
| `ArchiObservationKind` | 边界 | 观察项键集（`kind` / `from` / `to` / `message`）零变化（不新增 `occurrences`） | 单例 |
| `ArchiObservationKind` | 异常 | 非法串反序列化（`"unusedDeclarations"` / `"UnusedDeclaration"` / `""`）→ `Err` | 表驱动（3 行） |
| `ArchiObservationKind` | 组合 | 注释面与产出面一致：`declared ∖ used` 的装配侧只在 `drift::check` 一处（源码复核零第二处产出） | 单例 |
| `ArchiDriftReport` 形状 | 边界 | `REPORT_VERSION == 3`、顶层字段集快照不变（`version` / `timestamp` / `mode` / `root` / `change` / `base` / `modelHash` / `model` / `scan` / `errors` / `warnings` / `suppressed` / `observations` / `apiFindings` / `status`） | 单例 |
| `ArchiDriftReport` 形状 | 正向 | 健康报告序列化后的顶层键集与既有金样逐键相同（嵌套段落亦不扩字段） | 单例 |
| `ArchiDriftReport` 形状 | 异常 | 报告 JSON 混入未知字段 → 反序列化按 serde 缺省忽略（旧报告读取零破） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock） | 纯枚举 / 纯 serde 值；`ALL` 取真实常量，不落盘 | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/archi/src/dto.rs -> packages/desktop/src-tauri/crates/core/archi/src/dto_test.rs

#### 待测功能

- `load(root): 可见性提 pub(crate)（relation 复用同一份装载，不新增读盘路径）`
- `diagnostics_error(diagnostics): 可见性提 pub(crate)（结构错文案单点，relation 的 Err 复用）`
- `Loaded（类型）: 两字段 pub(crate)（graph / diagnostics）——load 的返回体；relation_detail 复用同一装载（MUST NOT 第二份 parse → build）`
- `ArchiRelation.apis（类型字段）: 末尾追加、字段不省略、序按模型原文`
- `ArchiRelationDetail（类型）: 12 字段 + camelCase 线名，occurrences 复用 ArchiDriftOccurrence`

<!-- 出处：`load` / `diagnostics_error` 取「公共函数 / API」表该文件行；`Loaded` / `ArchiRelation.apis` / `ArchiRelationDetail` 取「类型定义」表行（契约由 D11 / D13 与 AC-5 / AC-6 / AC-10 承载）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `load` | 正向 | 健康盘 → `graph` 与 `graph_view` 直调的元素 / 关系逐字段相等（单一装载点） | 单例 |
| `load` | 边界 | 空模型目录 → `graph` 零元素零关系、`diagnostics` 空（不 Err） | 单例 |
| `load` | 边界 | 解析诊断 + 图诊断并存 → `diagnostics` 为两源合并后的全量（条数 = 两源之和） | 单例 |
| `load` | 异常 | 坏模型（结构诊断）→ `diagnostics` 非空、零 panic、`graph` 可读 | 单例 |
| `load` | 组合 | 同一盘面连调两次 → 结果逐字段相等（无缓存也无副作用；`ModelGraph::build` 仅此一处调用） | 单例 |
| `Loaded` | 正向 | 两字段恒在场（`graph` / `diagnostics`）——健康盘 `diagnostics` 空、坏盘非空，`graph` 两种情况均可读（不被诊断吞掉） | 表驱动（2 行） |
| `Loaded` | 组合 | 单一装载点：`relation_detail` 与 `graph_view` 对同一盘面同源（`ModelGraph::build` 在 crate 内仅 `load` 一处调用，源码复核零第二份 parse → build） | 单例 |
| `Loaded` | 边界 | 空模型目录 → `graph` 零元素零关系、`diagnostics = []`（两字段仍齐备，装载不返回 `Option`） | 单例 |
| `Loaded` | 异常 | 坏模型多条诊断 → `diagnostics` 全量非空而 `graph` 仍可读（零 panic / 零 `Option` 空），消费侧由 `diagnostics_error` 收敛为 `Err` | 单例 |
| `diagnostics_error` | 正向 | 两条诊断 → 首行「模型结构错（共 2 条）：」+ 逐条 `kind @ file:line` | 单例 |
| `diagnostics_error` | 边界 | 空清单 → 仅首行（「共 0 条」形态）、零 panic | 单例 |
| `diagnostics_error` | 边界 | `line = 0` → 仅 `kind @ file`（无 `:0` 尾巴） | 单例 |
| `diagnostics_error` | 异常 | 路径含空格 / emoji / 反斜杠 → 原样插入（不转义、不 panic） | 表驱动（3 行） |
| `diagnostics_error` | 组合 | 与 `graph_diagnostics_error` 同一 `(kind, file, line)` 输入逐字相等（文案单点） | 表驱动（3 行） |
| `ArchiRelation.apis` | 正向 | 模型三态（省略 / 单名 / 多名）→ 投影 `apis` 逐字、序按原文（零归一 / 零排序） | 表驱动（3 行） |
| `ArchiRelation.apis` | 边界 | 省略 → `apis: []` 仍**在场**（字段不省略，线格式出现 `"apis": []`） | 单例 |
| `ArchiRelation.apis` | 边界 | 字段位置：`apis` 为末尾第六字段（前五字段 `from` / `to` / `description` / `mark` / `reason` 零位移） | 单例 |
| `ArchiRelation.apis` | 异常 | 结构错模型 → `graph_view` 停摆态 `relations` 为空（零关系投影，不产半成品 `apis`） | 单例 |
| `ArchiRelation.apis` | 组合 | `to_relation` 拷贝语义：模型 `["B","A"]` → 投影 `["B","A"]`（不乱序、不去重） | 单例 |
| `ArchiRelationDetail` | 正向 | 12 字段齐备、camelCase 线名（`modelFile` / `modelLine` / `usedNotDeclared` / `declaredNotUsed`） | 单例 |
| `ArchiRelationDetail` | 边界 | 空值三态：`reason = null`、`description = null`、`occurrences = []` 各自**不省略** | 单例 |
| `ArchiRelationDetail` | 边界 | 字段序按声明序出线（`from` … `occurrences`），`occurrences` 条目复用 `ArchiDriftOccurrence` 四字段 | 单例 |
| `ArchiRelationDetail` | 异常 | 反序列化非法 `mark` 串（`"missing"` / `""`）→ `Err`（`EdgeMark` 封闭四值，零第五值——边失配徽标不做） | 表驱动（2 行） |
| `ArchiRelationDetail` | 组合 | 与 `graph.relations` 同元素对的 `from` / `to` / `description` / `mark` / `reason` 逐字段相等（同源，零第二份判定） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock） | 真实 std 自建 `TempDir` 模型盘面 + 真实 `parse_dir` / `ModelGraph::build`；投影只裁剪，替身会引入第二事实源 | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/archi/src/relation.rs -> packages/desktop/src-tauri/crates/core/archi/src/relation_test.rs（新）

#### 待测功能

- `relation_detail(root, from, to): 模型面（声明 / 声明行 / mark / reason）+ 定向扫描事实 + 两桶（调 drift::api_buckets）+ occurrences；零写盘 / 零缓存 / 零 Report`
- `from_side_sources（私有助手）: 归属规则复用 OwnerIndex（from 元素 path 下源文件白名单），MUST NOT 第二份前缀归一`

<!-- 出处：`relation_detail` 取「公共函数 / API」表该文件行；`from_side_sources` 为该表未列的私有面，条目取 design「新增文件」子表 `relation.rs` 行（契约由 D11 / D12 与 AC-5 承载）。`#[cfg(test)] mod relation_test;` 与 `relation_test.rs` 同批落地。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `relation_detail` 模型面 | 正向 | 健康盘（关系 `apis = ["A","B"]`，from 侧文件引入 `A` / `B` / `C`）→ `declared = ["A","B"]`、`used = ["A","B","C"]`、`usedNotDeclared = ["C"]`、`declaredNotUsed = []` | 单例 |
| `relation_detail` 模型面 | 正向 | `modelFile` / `modelLine` = 该关系声明所在文件与 `from` 键行（可点开定位） | 单例 |
| `relation_detail` 模型面 | 正向 | `mark = none`、`reason = null`（本变更不做边失配徽标，标记只在画布侧派生） | 单例 |
| `relation_detail` 模型面 | 正向 | `description` 透传：有 `desc` → 原文；无 `desc` → `null` | 表驱动（2 行） |
| `relation_detail` 模型面 | 边界 | `apis` 省略 → `declared = []`、`declaredNotUsed = []`（事实面照出 `used`；两桶按 `api_buckets` 单点求差，UI 空态① 不渲染该段） | 单例 |
| `relation_detail` 模型面 | 边界 | `occurrences` 逐条含 `file` / `line` / `specifier` / `typeOnly`，按 file / line 排序；同文件多行不聚合 | 单例 |
| `relation_detail` 模型面 | 边界 | 关系 `apis` 含乱序多名 → `declared` 按模型原文序出线（求差在纯函数内做归一） | 单例 |
| `relation_detail` 模型面 | 边界 | 模型值面违例（`apis = [""]` / 重复名）→ `Err`（`invalidValue` 诊断文案，模型整体停摆） | 表驱动（2 行） |
| `relation_detail` 模型面 | 异常 | 关系未声明（元素对缺席）→ `Err` 文案点名该元素对与关系总数 | 单例 |
| `relation_detail` 模型面 | 异常 | `from` / `to` 为空串或空白串 → `Err`（元素对不存在），零 panic | 表驱动（4 行：空 / 空白 × 两侧） |
| `relation_detail` 模型面 | 异常 | 结构诊断非空（坏模型）→ `Err` 走 `diagnostics_error` 同一文案（零部分详情） | 单例 |
| `relation_detail` 模型面 | 异常 | 模型目录缺席（零元素零关系）→ `Err`（计数 0 的文案） | 单例 |
| `relation_detail` 事实面 | 正向 | 定向扫描只覆盖 `from` 元素 `path` 下文件：`to` 侧文件的反向 import 不计入 `used` | 单例 |
| `relation_detail` 事实面 | 边界 | `from` 侧零源文件（`path` 指向空目录）→ `used = []`、`occurrences = []`、`declaredNotUsed` = 全部声明名 | 单例 |
| `relation_detail` 事实面 | 边界 | 同元素对多文件命中 → `used` 去重 + 字典序；`occurrences` 逐条保留（不聚合） | 单例 |
| `relation_detail` 事实面 | 边界 | 名字取不出的形态（`export *` / `use a::*`）→ 不产假名（`used` 少该名，`declaredNotUsed` 侧如实呈现） | 表驱动（2 行） |
| `relation_detail` 事实面 | 异常 | 扫描失败（非法 `exclude` 规则）→ `Err` 经 `CheckError::message()` 透传（含成因） | 单例 |
| `relation_detail` 事实面 | 异常 | 架构域配置非法（未知键 / 缺 `reason`）→ `Err`（`load_config` 文案直传），零 panic | 表驱动（2 行） |
| `relation_detail` 事实面 | 边界 | 零取消语义：`CancelFlag::never()` → 单次调用恒 `Ok`（零 `Stopped`） | 单例 |
| `from_side_sources` | 正向 | 归属规则与 `OwnerIndex` 一致：只有归属为 `from` 的文件入白名单（兄弟元素文件排除） | 单例 |
| `from_side_sources` | 边界 | 非源文件扩展名（`.md` / `.json`）不入白名单；被架构域 `exclude` 覆盖的源文件**仍入**白名单（排除在扫描层判，白名单只管归属） | 表驱动（2 行） |
| `from_side_sources` | 异常 | 元素 `path` 指向不存在目录 → 空白名单（零 panic，`used = []`） | 单例 |
| 单点纪律 | 组合 | 源码复核：`api_buckets(` 在产品代码（非 `*_test.rs`）恰 2 处调用（`drift/mod.rs` 与 `relation.rs`）；`relation.rs` 零第二套集合差 | 单例 |
| 单点纪律 | 组合 | 零写盘：调用前后 `tempdir` 盘面字节快照逐一相同；零缓存：两次调用之间改盘面 → 第二次读到新状态 | 表驱动（2 行） |
| 单点纪律 | 组合 | 不组装报告：返回值只含 `ArchiRelationDetail` 字段（无 `status` / `passed` / `errors` / `warnings`） | 单例 |
| 单点纪律 | 组合 | 与 `drift::check` 对同一 `(from, to)`（`apis` 非空）→ `usedNotDeclared` / `declaredNotUsed` 逐字相等（同一 `api_buckets`；AC-5） | 表驱动（3 行：全匹配 / 漏项 / 多项） |
| 单点纪律 | 异常 | 出现第三处 `api_buckets(` 调用或第二套集合差（源码复核）→ 断言红（AC-5 的判定唯一性锚） | 单例 |
| 单点纪律 | 正向 | 一次调用四条纪律同时成立：零写盘 + 零缓存 + 零 `Report` + 单一求差 | 单例 |
| 单点纪律 | 边界 | 返回字段集恰 12 个（不扩 `via` / 不带 `status` / `passed`），嵌套只复用 `ArchiDriftOccurrence` | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock） | 真实 std 自建 `TempDir`（`graph/model/*.toml` + 两族元素源文件 + 架构域 `config.json`）+ 真实 `dto::load` / `scan::scan` / `drift::api_buckets`；`CancelFlag::never()` 真实值 | 全部用例 |

---

### packages/desktop/src-tauri/src/commands/archi/mod.rs -> packages/desktop/src-tauri/src/commands/archi/mod_test.rs

#### 待测功能

- `archi_relation(root, from, to): IPC 第九条命令薄包装（参数转换 → 调 archi::relation::relation_detail → 错误映射），零领域规则`
- `archi_relation_inner(root, from, to): check_root(root) → archi::relation::relation_detail，离 Tauri 运行时可测`

<!-- 出处：两条均取「公共函数 / API」表该文件行（契约由 D11 与 AC-5 承载）。AC-5 的跨命令对拍作为组合用例挂靠本章（入口模块 = 最上层调用方命令面）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `archi_relation` ↔ `_inner` | 正向 | 同一入参下包装与 `_inner` 结果逐字段相等（一一对应，既有三命令同款断言） | 单例 |
| `archi_relation_inner` | 正向 | 健康盘 → `declared` / `used` / 两桶 / `modelFile` + `modelLine` / `occurrences` 齐备 | 单例 |
| `archi_relation_inner` | 边界 | `root` 空白串 / 不存在 / 是文件而非目录 / 不可读四形态 → `Err`（`check_root` 既有文案，零 panic） | 表驱动（4 行） |
| `archi_relation_inner` | 边界 | `from` / `to` 空串 → `Err`（元素对不存在，文案含关系总数） | 表驱动（2 行） |
| `archi_relation_inner` | 边界 | `apis` 省略的关系 → `declared = []`、`occurrences` 仍取事实（命令语义单一，零早退分支） | 单例 |
| `archi_relation_inner` | 异常 | 未知元素对 → `Err` 点名 `from -> to` 与关系总数 | 单例 |
| `archi_relation_inner` | 异常 | 坏模型（结构诊断）→ `Err` 含诊断清单文案（经 `diagnostics_error` 单点） | 单例 |
| `archi_relation_inner` | 异常 | 架构域配置非法 / 扫描失败 → `Err` 文案透传（命令层零二次加工） | 表驱动（2 行） |
| `archi_relation_inner` | 边界 | 只读：调用前后 `tempdir` 盘面字节快照逐一相同（第九条命令加入既有「其余七条 MUST NOT 写盘」口径） | 单例 |
| 既有八命令回归 | 正向 | 既有命令 ↔ `_inner` 一致性（含新增第九条）、读命令零写盘、`archi_set_gate` 原子写回等既有断言逐条保持绿 | 表驱动（回归） |
| AC-5 跨命令对拍 | 组合 | 同一 `(from, to)`（`apis` 非空）下 `archi_relation_inner` 与 `archi_drift_inner` 的 `usedNotDeclared` / `declaredNotUsed` 逐字相等（同一纯函数，三条形态各一行） | 表驱动（3 行） |
| AC-5 跨命令对拍 | 组合 | `archi_drift` 报告的 `observations` / `errors` 中该元素对的 `occurrences` 与 `archi_relation` 的 `occurrences` 逐条相等（同形复用） | 单例 |
| AC-5 跨命令对拍 | 异常 | 同一元素对在 drift 侧零产（`apis` 省略）而命令侧仍出事实 → 两面口径差异显式成立（对拍只在 `apis` 非空域内判定） | 单例 |
| `archi_relation` ↔ `_inner` | 边界 | 合法 `root` 但模型目录缺席 → 包装与 `_inner` 同串 `Err`（错误映射一致） | 单例 |
| `archi_relation` ↔ `_inner` | 异常 | `_inner` 拒绝时包装同样 reject 且文案逐字相同（命令层零二次加工） | 单例 |
| 既有八命令回归 | 边界 | 既有「八命令 root 校验四形态」表驱动扩到九条（`archi_relation` 并入，其余八条零改动） | 表驱动（9 行） |
| AC-5 跨命令对拍 | 边界 | 两面均零产（`apis` 省略 + 无 import）→ drift 侧零条目、命令侧 `declared = []`（口径差异显式） | 单例 |
| AC-5 跨命令对拍 | 正向 | 全匹配形态（`apis` 非空且事实恰好同集）→ 两面两桶皆空、`used` 逐字相等 | 单例 |
| 既有八命令回归 | 异常 | 既有八命令的错误文案在新增第九条后零改动（同一 `check_root` 断言集，零文案漂移） | 表驱动（8 行） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock） | 真实 std 自建 `TempDir` 工作区根 + 真实模型 / 源文件盘面；`archi_relation` 不取 `State`（无 store / provider 依赖），直调原签名 | 全部用例 |

---

### packages/desktop/src-tauri/src/commands/mod.rs -> packages/desktop/src-tauri/src/bindings/mod_test.rs

#### 待测功能

- `all_commands! archi 段 8 → 9（archi_relation 追加于 archi_api_generate 之后、read_file 之前）`

<!-- 出处：「公共函数 / API」表无本文件行（宏清单，无函数出口）；条目取 design「修改文件」子表 `commands/mod.rs` 行与 `bindings/mod_test.rs` 行（契约由 D22 白名单与 AC-10 承载）。断言面在既有清单守卫：四处计数随动 + 尾部序期望 vec；`DTO_TYPES` / `ARCHI_DTO_TYPES` 的计数来源是 `dto.rs` 新 DTO 与 `bindings:export` 重导，同一守卫一章覆盖。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 命令清单计数 | 正向 | `COMMAND_NAMES.len() == 57` 且 `COMMAND_WRAPPERS.len() == 57`（一一对应） | 单例 |
| 命令清单计数 | 正向 | `archi_relation` / `archiRelation` 在册且插在 `archi_api_generate` / `archiApiGenerate` 之后、`read_file` / `readFile` 之前 | 表驱动（2 行） |
| 命令清单计数 | 边界 | 尾部序期望 vec 逐项相等（`app_environment` … `archi_api_generate`, `archi_relation`, `read_file`），既有条目零位移 | 单例 |
| 命令清单计数 | 边界 | `ARCHI_COMMANDS.len() == 9`、`DTO_TYPES.len() == 105`、`ARCHI_DTO_TYPES.len() == 22` | 表驱动（3 行） |
| 命令清单计数 | 边界 | `ArchiRelationDetail` 在 `ARCHI_DTO_TYPES` 中字典序插在 `ArchiRelation` 之后 | 单例 |
| 产物面命中 | 正向 | 生成物含 `archiRelation:` 包装、`"archi_relation"` invoke 名、`export type ArchiRelationDetail =`（逐名 contains） | 表驱动（3 行） |
| 产物面命中 | 边界 | 首尾锚 `ActivePhase` / `NodeMark` 断言零改动（尾锚随封闭词汇末项） | 表驱动（2 行） |
| 产物面命中 | 异常 | 清单含产物未生成的名字 → 既有 `contains` 断言红并点名缺项（新增名不得靠「漏登」过门） | 单例 |
| 清单自洽 | 组合 | `ARCHI_COMMANDS` 每条同时在 `COMMAND_NAMES` / `COMMAND_WRAPPERS` 在册（既有断言随 8 → 9 保持成立） | 表驱动（9 行） |
| 清单自洽 | 组合 | 快照机制：真实 `export_bindings()` 产物即被测面（不 mock 产物文件），连做两次重导零 diff（幂等） | 单例 |
| 命令清单计数 | 异常 | 清单长度与条目数不一致（漏登 `archi_relation`）→ 计数断言红并点名 | 单例 |
| 清单自洽 | 边界 | 清单只作连带核对：产物类型总数与 `DTO_TYPES` 命中数不等时不断言语义（本清单不作判据） | 单例 |
| 清单自洽 | 正向 | `ARCHI_COMMANDS` 九条与 `ARCHI_DTO_TYPES` 二十二条全部在产物中命中（零孤儿名） | 表驱动（2 组） |
| 清单自洽 | 异常 | 产物含 `ArchiRelationDetail` 而清单漏登 → `DTO_TYPES` 长度断言红（双向守，不做单侧信任） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock） | 真实 `tauri_specta` 产物快照（`authoritative_snapshot()` + 文件锁 + `Drop` 还原）；清单是产物的子集判据由真实 `contains` 承担 | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/archi-knowledge.md -> packages/desktop/src-tauri/crates/infra/agent/src/archi_knowledge_test.rs

#### 待测功能

- `骨架文本: 关系键面由三键改四键（from / to / desc / apis）+ 一句 apis 纪律（只写名字 / 可留空 / 生成物面留空 / 结论看 archi_drift）`
- `体量锚: 骨架与全文冻结值随实测更新，区间 2000..=2150 / 2550..=2750 与门限 2252 / 2867 逐字不抬高`

<!-- 出处：「公共函数 / API」表无该文件行（静态 md 经 `include_str!` 内嵌，产物由 `archi_knowledge::knowledge()` 出）；条目取 design「修改文件」子表 `archi-knowledge.md` 行与 D20 / AC-11 的体量锚，不 grep 导出声明。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 关系四键 | 正向 | 键面串 `（from / to / desc / apis）` 逐字在场（三键旧串缺场） | 表驱动（2 行） |
| 关系四键 | 边界 | `apis` 纪律句含四要素：只写名字 / 可留空 / 生成物面留空 / 结论看 `archi_drift` | 表驱动（4 行） |
| 关系四键 | 边界 | `apis` 与 `doc` / `entry` 同列（开放值集语义句在场），且不进任何派生词表 | 单例 |
| 关系四键 | 异常 | 零判据长表：`undeclaredApi` / `unusedDeclaration` / `docMissing` / `usedNotDeclared` 在骨架中零命中 | 表驱动（4 行） |
| 关系四键 | 边界 | 零示例围栏（`` ``` `` 计数 0）、零 `<>` 占位符（既有断言保留） | 表驱动（2 行） |
| 关系四键 | 组合 | 派生词表连接串逐字不变（新增 `apis` 不扩张词表；`kind` / `gate` / `api` 状态 / 结构诊断九词仍在场） | 表驱动（4 组） |
| 既有纪律保留 | 正向 | 四条 C 档纪律在场（带引号 FQN 单键 / `metadata` 内联表 / 目录尾 `/` / 容器 `path` 恒空） | 表驱动（4 行） |
| 既有纪律保留 | 边界 | 写面与可见性三句 + 历史会话续接一句仍在场 | 表驱动（4 行） |
| 体量锚 | 边界 | 骨架字节 `skeleton().trim_end().len()` 等于新冻结实测值，且落 `2000..=2150`、`<= 2252` | 单例 |
| 体量锚 | 边界 | 全文（骨架 + 派生段）字节等于新冻结实测值，且落 `2550..=2750`、`<= 2867` | 单例 |
| 体量锚 | 异常 | 区间与门限常量逐字不抬高（源码复核：`2252` / `2867` 与两区间未改） | 单例 |
| 体量锚 | 组合 | `knowledge()` 全文 = 骨架 + 派生段（前缀相等、包含关系不变）；`build` 的 `<>` 拒绝与降级语义不变 | 表驱动（2 行） |
| 既有纪律保留 | 异常 | 四条 C 档纪律任一被压缩掉 → 对应断言红（新增 `apis` 句不得挤掉静默陷阱） | 表驱动（4 行） |
| 体量锚 | 正向 | `apis` 增量（约 +60–130 B）后骨架仍落 `2000..=2150`：区间而非门限是最后一道（净增长靠同段压缩抵平） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock） | `include_str!("archi-knowledge.md")` 取编译期内嵌的同一份字节（禁测试内复制副本）；词表 / 目录取真实派生函数 | 全部用例 |

---

### packages/desktop/src/views/architecture/hooks/use-archi-relation.ts -> packages/desktop/src/views/architecture/hooks/use-archi-relation.test.ts（新）

#### 待测功能

- `useArchiRelation(root, edge): 按需取数——无根 / 无选中即零 invoke；同一元素对零重复 invoke（引用缓存）；失败保留 error 供重试；retry() 强制重取；零 watch 零轮询`
- `RelationEdge / ArchiRelationState（类型）: 选中边身份 { from, to } 与状态面 { detail, loading, error, retry }`

<!-- 出处：`useArchiRelation` 取「公共函数 / API」表该文件行；两 interface 取「类型定义」表行（契约由 D13 / D17 与 AC-7 承载）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `useArchiRelation` 取数 | 正向 | `root` 非空 + `edge` 非空 → 恰一次 `archi_relation`，载荷 `{ root, from, to }`；`detail` 落地、`loading` 由 true → false | 单例 |
| `useArchiRelation` 取数 | 正向 | 同一元素对重复选中（对象引用变化、值相同）→ invoke 计数恒 1（缓存命中，不依赖对象引用） | 单例 |
| `useArchiRelation` 取数 | 正向 | 切到另一对再切回 → 第二次命中缓存、零新增 invoke（跨面板两态切换仍成立） | 单例 |
| `useArchiRelation` 取数 | 正向 | `retry()` → 删键并重新 invoke 恰一次；成功后 `error = null`、`detail` 为新值 | 单例 |
| `useArchiRelation` 取数 | 边界 | `root = null` → 零 invoke、`detail = null`、`error = null`、`loading = false` | 单例 |
| `useArchiRelation` 取数 | 边界 | `edge = null`（未声明 `apis` 的边由调用方传 null）→ 零 invoke（AC-7 的「零取数」锚） | 单例 |
| `useArchiRelation` 取数 | 边界 | `root` 由 A 变 B → 以新 root 重新 invoke（缓存键含 root，旧缓存不串味） | 单例 |
| `useArchiRelation` 取数 | 边界 | 超大重复次数（同一对选中 1000 次）→ invoke 计数恒 1（无定时器 / 无轮询放大器） | 单例 |
| `useArchiRelation` 取数 | 边界 | `edge` 的 `from` / `to` 含超长串（> 1000 chars）→ 原样作载荷、零 panic | 单例 |
| `useArchiRelation` 取数 | 异常 | invoke reject → `error` 为 reject 文案、`detail = null`、`loading = false`，**不写缓存** | 单例 |
| `useArchiRelation` 取数 | 异常 | 失败后再次选中同一对 → 重新 invoke（失败不落缓存） | 单例 |
| `useArchiRelation` 取数 | 异常 | 竞态：先发的 A 对回包晚于后发的 B 对 → 状态只落最后一次（cancelled 语义，旧回包零写入） | 单例 |
| `useArchiRelation` 取数 | 异常 | 卸载后回包 → 零 setState（cancelled 早退），零 warning | 单例 |
| `useArchiRelation` 取数 | 组合 | 零 watch 零轮询：源码零 `setInterval` / 零订阅；用例不装假定时器仍绿（D14 / S3 的机械锚） | 单例 |
| `RelationEdge` 身份 | 边界 | 身份 = `(from, to)` 元素对：`from` / `to` 顺序不同视为不同对（零排序归一） | 单例 |
| `ArchiRelationState` | 边界 | 四字段恒在场：`detail` / `loading` / `error` / `retry`（不因状态切换缺席） | 单例 |
| `RelationEdge` 身份 | 正向 | 同一对（值相同、对象引用不同）→ 命中同一缓存键、零重复 invoke（身份 = 值不是引用） | 单例 |
| `RelationEdge` 身份 | 异常 | `from` / `to` 含 `->` 字符（非常规 FQN）→ 缓存键仍按 `root\0from\0to` 拼接，与边 id 的 `->` 语义解耦（零串味） | 单例 |
| `ArchiRelationState` | 正向 | 取数成功 → `detail` 非空、`loading = false`、`error = null`、`retry` 可调用 | 单例 |
| `ArchiRelationState` | 异常 | 切到另一对并失败 → `detail` 清空（不残留上一对详情）、`error` 为新失败文案 | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core` 的 `invoke`（进程边界） | `vi.mock` 替 `invoke`，按命令名 `archi_relation` 分发 fixture 并记录调用序列与载荷（生成的 `commands.archiRelation` 最终经此调用） | 全部用例 |
| 无（其余零 mock） | 真实 `renderHook` / `act` / `waitFor`；不装假定时器（「零定时器」本身就是判据之一） | 全部用例 |

---

### packages/desktop/src/views/architecture/components/relation-info.tsx -> packages/desktop/src/views/architecture/components/relation-info.test.tsx（新）

#### 待测功能

- `RelationInfo(props): 七段固定序（关系 / 说明 / 声明 API / 实际使用 / 两桶 / 命中点 / 模型行号）+ 四空态优先级 + 行内错误与重试；不自行求差、不读漂移报告`
- `RelationInfoProps（类型）: { root, edge, relation, state, exemptVia, onRetry }——数据全由调用方注入（面板零自取数、零读报告）`

<!-- 出处：`RelationInfo` 取「公共函数 / API」表该文件行；`RelationInfoProps` 取「类型定义」表行（契约由 D13 / D14 / D19 与 AC-7 承载）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `RelationInfoProps` | 正向 | 六字段全注入（`root` / `edge` / `relation` / `state` / `exemptVia` / `onRetry`）→ 面板按注入值渲染（`edge` 决定关系行、`relation.apis` 决定声明段、`state.detail` 决定事实段），零自取数 | 单例 |
| `RelationInfoProps` | 边界 | `root = null` → 面板照常渲染（不塌段），模型段「查看」不发起 `read_file`（无根不取数） | 单例 |
| `RelationInfoProps` | 异常 | `state.detail` 的 `from` / `to` 与 `edge` 不一致（陈旧回包）→ 关系行仍按 `edge` 渲染、事实段不串味，零 panic | 单例 |
| `RelationInfo` 七段 | 正向 | 健康详情 → 七段逐段在场（根 testid `archi-relation-info` + 关系行 `from → to` + 标记徽标 + 说明 + 声明 API + 实际使用 + 两桶 + 命中点 + 模型行号） | 单例 |
| `RelationInfo` 七段 | 正向 | 声明 API 段零取数：`state.detail = null` 时仍逐名渲染 `relation.apis`（数据源是图载荷） | 单例 |
| `RelationInfo` 七段 | 正向 | 命中点逐条 `file:line` + `specifier`；多条按入参序（不重排） | 单例 |
| `RelationInfo` 七段 | 正向 | 模型段 `modelFile:modelLine` + 「查看」触发 `read_file`（复用 `FileViewer` 既有范式） | 单例 |
| `RelationInfo` 七段 | 边界 | 两桶皆空（`usedNotDeclared = []` / `declaredNotUsed = []`）→ 各显「—」占位（不塌段） | 单例 |
| `RelationInfo` 七段 | 边界 | `declaredNotUsed` 行附并列成因措辞（「不再依赖」+「名字写错或扫描看不见」）——MUST NOT 只写「没用」 | 单例 |
| `RelationInfo` 七段 | 边界 | `description = null` → 说明段不占文案（不渲空串、不塌相邻段） | 单例 |
| `RelationInfo` 七段 | 边界 | 超长名字 / 超长路径（> 1000 chars）与特殊字符（emoji / 换行）→ 原样渲染、不解析为 HTML、不 panic | 表驱动（3 行） |
| `RelationInfo` 空态 | 边界 | 空态① `relation.apis = []` → 恰渲「未声明依赖 API（选填）」（`archi-relation-undeclared`），**不渲染** 实际使用 / 两桶 / 命中点 / 模型四段 | 单例 |
| `RelationInfo` 空态 | 边界 | 空态② `detail.occurrences = []` → 「扫描面无该关系的 import」（`archi-relation-no-import`） | 单例 |
| `RelationInfo` 空态 | 边界 | 空态③ `mark = 'exempt'` + `exemptVia = 'config'` / `'inline'` + `reason` → 「已豁免」+ 来源 + 理由（`archi-relation-exempt`） | 表驱动（2 行） |
| `RelationInfo` 空态 | 边界 | 优先级：①`error` > ②`exempt` > ③`apis` 空 > ④`occurrences` 空（四组交叉冲突各一行，同时只渲一个空态） | 表驱动（4 行） |
| `RelationInfo` 空态 | 异常 | 空态④ `state.error !== null` → `archi-relation-error` + 「重试」按钮；点击触发 `onRetry` 恰一次 | 单例 |
| `RelationInfo` 空态 | 异常 | `error` 与 `apis` 空同时成立 → 错误优先（① > ③），零双重空态并排 | 单例 |
| `RelationInfo` 空态 | 边界 | loading 态（`loading && detail === null`）→ 「取数中…」独立呈现，**不占**四空态槽位 | 单例 |
| `RelationInfo` 空态 | 边界 | `error = null` 后重试成功 → 空态④ 消失、七段恢复 | 单例 |
| 职责边界 | 组合 | 零二次求差 / 零报告读取：组件内不做集合差、不发 `archi_drift` invoke（源码复核 + invoke 计数）；`exemptVia` 只由 prop 传入 | 单例 |
| `RelationInfo` 七段 | 异常 | `occurrences` 含 `line = 0` → 呈现 `file` 无行号、零 panic（不伪造行号） | 单例 |
| `RelationInfo` 空态 | 正向 | 健康详情且四空态条件皆假 → 四个空态 testid 全缺席、七段齐备 | 单例 |
| 职责边界 | 正向 | 零自取数：渲染期间 `archi_relation` / `archi_drift` invoke 计数恒 0（数据全来自 prop） | 单例 |
| 职责边界 | 边界 | `mark = 'exempt'` 而 `exemptVia = null` → 空态③ 仍渲染（来源句省略，理由照渲） | 单例 |
| 职责边界 | 异常 | `state.error` 与 `state.detail` 同时非空（不一致态）→ 只渲错误态、七段不渲染、零 panic | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core` 的 `invoke`（进程边界） | `vi.mock` 替 `invoke`，只为 `read_file`（「查看」）与「零 `archi_drift` invoke」判据记录调用 | 模型段与职责边界用例 |
| 无（其余零 mock） | 真实 `render` / `fireEvent` / `screen`；`FileViewer` 复用真实组件（既有实现） | 全部用例 |

---

### packages/desktop/src/views/architecture/components/architecture-canvas.tsx -> packages/desktop/src/views/architecture/components/architecture-canvas.test.tsx

#### 待测功能

- `ArchitectureCanvas({ root, mode }): 入参与导出不变；内部新增选中态、边点击取数与 CardHeader 两态（ModelInfo ⇄ RelationInfo）`
- `CanvasBody / ModelRelationGraph（私有面）: onEdgeClick 置选中、onNodeClick 清选中、选中边高亮（既有 stroke 变量，零新标记词汇）`
- `exemptVias（私有派生面）: 从 report.suppressed[] 派生 Map<"from->to", 'config' | 'inline'>（零新 invoke、零重算）`

<!-- 出处：`ArchitectureCanvas` 取「公共函数 / API」表该文件行（「零改动（签名）」）；三个私有面取 design「修改文件」子表 `architecture-canvas.tsx` 行（契约由 D14 / D15 / D16 / D17 与 AC-7 承载）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `onEdgeClick` 与边点击 | 正向 | 点一条 `apis` 非空的边 → `CardHeader` 切到 `RelationInfo`（`archi-relation-info` 在场、`ModelInfo` 段缺席）；恰一次 `archi_relation`（载荷 `{ root, from, to }`） | 单例 |
| `onEdgeClick` 与边点击 | 正向 | 选中边高亮：仅换既有 `style.stroke`（`marks.ts` 四值词汇零扩张，零新标记值） | 单例 |
| `onEdgeClick` 与边点击 | 正向 | 点节点 → 回元素态（`ModelInfo` 在场、`RelationInfo` 缺席、选中清空、零新增 `archi_relation` invoke） | 单例 |
| `onEdgeClick` 与边点击 | 边界 | 未声明 `apis` 的边（`relation.apis = []`）→ 面板照常开、空态① 渲染、`archi_relation` invoke 计数为 **0**（AC-7） | 单例 |
| `onEdgeClick` 与边点击 | 边界 | `mark = 'unfulfilled'` / `'exempt'` 的边点击 → 徽标/来源按既有标记呈现，面板零重判（决策只在标记面） | 表驱动（2 行） |
| `onEdgeClick` 与边点击 | 边界 | 同一元素对重复点开、面板两态来回切 → invoke 计数恒 1（hook 挂 `CanvasBody`，缓存不随面板卸载丢失） | 单例 |
| 边身份 | 组合 | `toEdges` 的 `id` 仍 `from->to`；`onEdgeClick` 取 `edge.source` / `edge.target` 与该 id 同源（既有身份零改动） | 单例 |
| `exemptVias` | 正向 | `report.suppressed[]` 含该对（`via = config` / `inline`）→ `RelationInfo.exemptVia` 与 `reason` 同时命中空态③ | 表驱动（2 行） |
| `exemptVias` | 边界 | `suppressed[]` 为空 / 报告为 `null` → `exemptVias` 空、点边零抛错（`exemptVia = null`） | 表驱动（2 行） |
| `selectedEdge` 选中与清空 | 边界 | `root` 变化 → 选中清空（`useEffect`）并以新 root 重新取数，旧对缓存不串味 | 单例 |
| `selectedEdge` 选中与清空 | 边界 | 选中对已不在重建后的 `graph.relations` → 按无选中渲染 `ModelInfo`（派生态，零副作用 / 零抖动） | 单例 |
| `selectedEdge` 选中与清空 | 边界 | 显式刷新（`key={refreshTick}` 重挂载）→ 选中自然复位、面板回元素态 | 单例 |
| `selectedEdge` 选中与清空 | 边界 | 图停摆（`diagnostics` 非空）→ 结构错面板独占、零边点击面、零 `archi_relation` invoke | 单例 |
| 失败隔离 | 异常 | `archi_relation` reject → 面板空态④ 行内错误 + 重试；图节点 / 边 / 左栏三视图 / `ModelInfo` 零影响（`archi_graph` / `archi_drift` invoke 计数不变） | 单例 |
| 失败隔离 | 异常 | `archi_drift` reject（既有降级）→ 零标记图照常渲染、`exemptVias` 空、点边不抛错（只落 `archi-drift-error` 行内提示） | 单例 |
| 失败隔离 | 边界 | `archi_graph` 失败（既有 error 态）→ 零 `archi_relation` invoke（图无源即不取详情） | 单例 |
| 纪律面 | 组合 | 零 watch 零轮询：源码零 `setInterval`；用例不装假定时器仍绿（S3 的机械锚） | 单例 |
| 既有回归 | 正向 | 既有断言逐条保持绿：节点 / 边集合、节点坐标、`react-flow__edge` DOM、`ReactFlow` 选项、入边分区、停摆分支、`mode` 透传 | 表驱动（回归） |
| `onEdgeClick` 与边点击 | 异常 | 点击不在 `graph.relations` 的陈旧边 id → 选中不变、零 `archi_relation` invoke | 单例 |
| 边身份 | 正向 | 选中边的 `id` = `<from>-><to>` 与 hook 载荷的 `from` / `to` 逐字同源（零二次解析） | 单例 |
| 边身份 | 边界 | 同一边同时出现在入边分区（`incomingZone`）→ 边集合仍只按 `from` / `to` 过滤（既有过滤语义不变） | 单例 |
| `exemptVias` | 异常 | `suppressed[]` 含不在图内的元素对 → 派生 Map 有键但零边消费，点边不误触空态③ | 单例 |
| `selectedEdge` 选中与清空 | 异常 | 重挂载后旧选中不复活（`key` 变化重建 state，零残留 `RelationInfo`） | 单例 |
| 失败隔离 | 正向 | 取数成功 → `RelationInfo` 七段完整、图节点 / 边 / `ModelInfo` 与取数前逐条相同 | 单例 |
| 纪律面 | 正向 | 挂载与 `root` 变化零 `archi_relation` invoke（点边取数只由显式用户动作发起，S3） | 单例 |
| 既有回归 | 异常 | 结构错（停摆）优先于取数失败：既有分支序不变（停摆面板独占、零降级样式） | 单例 |
| 边身份 | 异常 | 模型同一元素对重复声明（id 重复）→ 选中按 `(from, to)` 身份去重，零双面板 | 单例 |
| `selectedEdge` 选中与清空 | 正向 | 点节点 → `focus` 更新且选中置 `null`（与边点击互斥，两态不同时成立） | 单例 |
| 纪律面 | 异常 | 若点边取数被写成自动重取（watch / 轮询）→ 定时器与源码断言双红（S3 的机械守） | 单例 |
| 纪律面 | 边界 | 只在显式动作时 invoke：挂载 + `root` 变化 + 重挂载各零 `archi_relation`，点边恰一次 | 表驱动（4 行） |
| 既有回归 | 边界 | 三条早退分支（无根 / 停摆 / 空模型）在新增选中态后零变化（选中面不参与早退判据） | 表驱动（3 行） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core` 的 `invoke`（进程边界） | `vi.mock` 替 `invoke`，按命令名分发 `archi_graph` / `archi_module` / `archi_drift` / `archi_file_tree` / `archi_set_gate` / `archi_relation` / `read_file` 并记录调用序列与载荷 | 全部用例 |
| `@xyflow/react` 的 `ReactFlow`（观测垫片） | `vi.mock` 只记录入参后交真实 `ReactFlow` 渲染（非语义替身）；`ResizeObserver` / `offsetWidth` / `DOMMatrixReadOnly` / `getBBox` 为 jsdom 环境缺口垫片 | 边 / 节点 / 选项断言 |
| 无（其余零 mock） | 真实 `render` / `fireEvent` / `within` / `cleanup`；`driftMarks` / `RelationInfo` / `ModelInfo` / `useArchiRelation` 全真实组合 | 全部用例 |

---

## 不可测试项

| 模块 / 面 | 原因 |
|---|---|
| `.dev-team/architecture/graph/model/06-frontend.toml` | 纯数据面（+1 元素 / 12 条 `to` 改指 / 表头计数），无运行时行为可断言；**已落位**（表头 32 元素 / 74 关系、新元素 `modelLine = 188`、`to = "App.Frontend.GeneratedBindings"` 恰 12 条且 `to = "App.Backend.Shell"` 零命中、`api = "none"` 3 处）。AC-8 的**模型面读数**（55 元素 / 123 关系、`06` 32 / 74、74 = 66 + 8、`observations` 62 → 50、`model.warnings` 9 条 `noPath`）由桌面端 `archi_graph` / `archi_drift` 采集并落本 change `reports/`，agent 内不重复执行。**扫描面按条目级账目验收**（既有条目零增删改 + 新增 14 条全部命中既有规则：`scannedFiles` / `unmatchedFiles` 415、`excluded` 352、候选 767）——报告 `scan` 段只出计数、`unmatchedFiles[]` / `excluded[]` 条目明细不经报告出线，故对拍取 `reports/archi/` 基线与变更后 full 报告加 `git status` 规则集 / 文件集复核，不进单元测试；MUST NOT 以三面计数相等充当归属或判定面读数（见「AC-8 口径校正」）。 |
| `.dev-team/architecture/decisions/0001-model-granularity.md` | 文档面（`§1` / `§2` / `§3` / `§6` / `§7` / `§8` / `§9` 计数与判据行写实），无测试可断言；**已落位七项**（55 / 123、41 条 `export *`、端点边 8 = 3 出 + 5 入、55 个元素、`§7` 映射改指与「有主 ⇒ 兑现」、`§8` 两条新判据、`§9` 74 = 66 + 8）；判据为 `git diff` 复核（与模型 / 报告读数一致）。 |
| AC-8 扫描面条目级账目（`scannedFiles` / `unmatchedFiles` / `excluded` / 候选 767） | 读数不可在 `tempdir` 单测中复现：报告 `scan` 段只出计数，`unmatchedFiles[]` / `excluded[]` 条目明细不经报告出线。判据 = `reports/archi/20261120T1042330799149Z.json`（full、变更前）与变更后 full 报告的集合对拍（既有条目零增删改、新增 14 条各带命中规则）+ `scan/exclude.rs` 的 `DEFAULT_EXCLUDE_RULES` 与配置 `exclude` 零 diff + `git status` 无 `D` / `R` 条目，由桌面端命令 / 静态链采集。单元测试只覆盖产出名字事实与两桶判定的代码面（`scan/{mod,rust,ts}.rs` / `drift/mod.rs` 章节），不承接该账目。 |
| `.dev-team/architecture/graph/INDEX.toml`、`.dev-team/architecture/models/**/*.schema.json` | 生成物，禁手改（AC-10）。**已落位**：`INDEX.toml` 45 → 46 条 `[[entry]]`（新元素 `status = "absent"` / `types = 0` / `ops = 0`）、`api:check` 零 diff；`models/**/*.schema.json` 仍 10 份且逐字节不变（`api = "none"` 不产 schema）。implement 只需保持零 diff；断言面由 `pnpm api:export` + `api:check` 静态链与 `git diff` 承担，由桌面端执行。 |
| `packages/desktop/src/types/generated/bindings.ts` / `bindings.zod.ts` | 生成物，禁手改（AC-10）。断言面是「重导后 diff 恰为 `ArchiRelationDetail` / `ArchiRelation.apis` / `commands.archiRelation` 及其 schema 表项」，由 `bindings:export`（幂等两次零 diff）+ `client:check`（`tsc`）与 `git diff` 复核，由桌面端执行。 |
| `packages/desktop/src-tauri/crates/core/archi/src/lib.rs` | 本 change 只加 `pub mod relation;` + `pub use relation::relation_detail;`（+ 与 `relation_test.rs` 同批的 `#[cfg(test)] mod relation_test;`），无运行时可断言行为；缺注册即模块解析 / 编译失败，判据由 `cargo check --all-targets`（`server:check`）承担。 |
| `packages/desktop/src-tauri/crates/core/archi/src/drift/report_test.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/archi_knowledge_test.rs` 的**冻结值 / 计数行本身** | 两文件本身就是测试文件：本 change 对它们的改动只是计数 4 → 5、`as_str` 补值与体量冻结值随实测更新——机械闭合、零新增用例语义（对应语义已分别落 `drift/report.rs` 与 `archi-knowledge.md` 章节）。 |
| `packages/desktop/src-tauri/crates/core/archi/src/{diff_test.rs, graph_test.rs}` 的 `Relation` 字面量随动 | `diff.rs` / `graph.rs` 本 change 零改动（D22 白名单），两处只是补 `apis: Vec::new()` 的机械夹具闭合；缺口由编译期强制暴露，不构成新测试设计项（`model_test.rs` 的同款随动已在 `model.rs` 章节覆盖）。 |
| `packages/desktop/src/lib/ipc-contract.test.ts`、`packages/desktop/src/views/architecture/components/model-info.test.tsx`、`packages/desktop/src/views/architecture/lib/drift-marks.test.ts`、`packages/desktop/src/views/architecture/hooks/use-archi-graph.test.ts` 与 `views/architecture/**` 其余既有 DTO 夹具 | 这些文件本身就是测试文件，本 change 只补 `ArchiRelation.apis` / `ArchiRelationDetail` 的必填字段（如 `relation()` 夹具补 `apis: []`）——机械闭合、零新增断言；缺口由 `tsc`（`client:check`）强制暴露。 |
| AC-9（api 基线 46 / 36 / 十二值 0 / `status = clean` / `passed = true` / `api/**` 13 份） | 本仓读数：由 `archi_api_check` / `archi_drift` 命令采集 + `api:check` 静态链判定，单元测试只覆盖判据面之外的字段装配（`dto.rs` / `drift/*`），读数本身不可在 `tempdir` 单测中复现。 |
| AC-12（全管线绿：`pnpm check` 四段链 + 全量 `pnpm test`） | 整链静态与全量测试门禁，由桌面端 static-check / test-execution 相位执行；本设计只保证各文件的用例落点覆盖 AC-1…AC-11 的断言面。 |
| AC-13（版本纪律：`package.json` `0.7.5` / `tauri.conf.json` 跟随 / `src-tauri/Cargo.toml` 不随动） | 数据行，无运行时行为；判据为 `git diff` 复核，由桌面端执行。 |

