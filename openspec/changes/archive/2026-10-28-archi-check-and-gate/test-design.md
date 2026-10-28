# 测试设计: archi-check-and-gate

> **变更**: archi-check-and-gate
> **依据**: `proposal.md`（AC-1…AC-21、「变更范围」实现文件 / 测试文件两子表、范围裁决八条）、`design.md`（D1–D25、「变更清单」新增文件 16 条 + 修改文件 26 条、「公共函数 / API」、「类型定义」、「配置」、「依赖」、事实校准 1–5、待决问题 1–4）、`specs/`（13 份 delta）
> **范围**: design「变更清单」的**新增文件 16 条**与**修改文件 26 条**（含 `.dev-team/architecture/{models/03-backend-infra.toml, models/04-backend-shell.toml, decisions/0001-model-granularity.md}` 三件套——R-13 口径更正后它们是**实现文件**，不再是零改动面），另加 proposal「测试文件」表与 `desktop-corpus-regression` MODIFIED delta 明列的 `crates/core/workflow/tests/corpus_golden_test.rs`（AC-16 全词汇语料 + golden 扩展，见对应 per-file 章节）。删除文件无（本变更零退役）。三份 `Cargo.toml` 的清单面、`bindings` 生成物两件的**结构面**（零 diff / 重导出）、`package.json` 版本行、`crates/core/archi/tests/README.md` 实测表、`openspec/changes/archi-check-and-gate/reports/archi/model-sync-check.md` 核对留痕、模型三件套（数据资产，AC-21 以「命令 + 留痕」核验）与 `tests/perf.rs`（p95 装置归 test-execution 执行）无单测路径（见「不可测试项」）。`bindings` 生成物的**运行期形态面**（drift/scan schema 图对合法 / 非法 payload 的接受与拒绝）另设 `packages/desktop/src/lib/ipc.ts` per-file 章节——回溯补正：原稿把生成物整体列为不可测试项，导致前端函数覆盖 88.19%（1726/1957）的缺口无行可落。
> **路径口径**: 全部路径为**项目根相对**。design「修改文件」表的 Rust 简写 `crates/**` 对应 `packages/desktop/src-tauri/crates/**`，`src-tauri/src/**` 对应 `packages/desktop/src-tauri/src/**`；新增文件与前端文件一律写全路径。

## 测试框架与既有布局

- **Rust**：`cargo test --workspace`；断言仅 `assert!` / `assert_eq!` / `assert_ne!` / `matches!`，无第三方断言库。单测布局为 **sidecar 同目录同前缀** `*_test.rs`，源文件尾以 `#[cfg(test)] mod <name>_test;` 挂载（`crates/core/archi/src/lib.rs`、`crates/core/orchestration/src/lib.rs`、`crates/infra/checks/src/lib.rs` 先例）；子目录模块的 sidecar 与被测文件同目录（`crates/core/archi/src/parse/mod_test.rs` 先例）。crate 级集成测试驻 `tests/`（`crates/core/archi/tests/golden_test.rs` 先例：case 目录 + 测试内单点期望登记面 + `DESKTOP_GOLDEN_REWRITE=1` 显式重写）。
- **临时目录装置**：`crates/core/archi` **无 dev-dependency**（`Cargo.toml` 现无 `[dev-dependencies]`，本变更不新增）→ 自建 `TempDir` RAII + `AtomicU64` 计数（`dto_test.rs` / `parse/mod_test.rs` / `docs_test.rs` 先例）。`crates/infra/archi` 按 design 依赖表取 `tempfile::TempDir`（`infra/checks` 同式）。`orchestration` / `workflow` 沿既有 `TestDb` / `TempWs` / 假 runner 装置。
- **前端**：vite-plus 的 `vp test`（vitest 4，导入面 `vite-plus/test`；全局 jsdom）；`@testing-library/react` 的 `render` / `renderHook` / `act` / `waitFor`，定位走 `data-testid`（禁样式类名）；进程边界 mock 恒为 `vi.mock('@tauri-apps/api/core')` 的 `invoke` 按命令名分发；纯函数用例零 mock 直调（`marks.test.ts` 先例）。
- **命名**：Rust 测试函数为中文短语 + 断言消息载判据；前端 `describe` / `it` 中文短语 + testid 定位。

### 覆盖率分母口径（前端函数 90%）

workspace `openspec/config.json` 的 `tests[]` 只有三档百分比阈值（`coverage.functions = 90`），**没有**「覆盖率分母排除」字段；其 `excludes` 语义是 suite 圈定 / 测试文件枚举过滤（`crates/infra/checks/src/testexec/detect.rs` 的收集面与 `mutation` 归桶，纯层见 `crates/core/checks/src/aggregate.rs` 的 `is_file_excluded` / `in_suite_scope`）。两处分母由此分开：suite override 块按 `excludes` 圈范围（剔除 `src/types/generated/**` 后前端 1228/1293 = 94.97%），而全局 `measured` 取框架上报的 `total`（`weighted_measured`，含生成物，1726/1957 = 88.19%），门禁 `pass` 要求两者同时达标。

因此「把生成物剔出分母」需改 `packages/desktop/vite.config.ts` 的 `coverage.exclude`——该文件不在 design「变更清单」射程内，且剔分母会让生成物的运行期形态面整体失去观测（`bindings:check` 只锁结构零 diff，不锁「`archiDrift` 报告形态能否被前端接受」）。本设计取另一路：让 drift/scan schema 图被**真实执行**——`packages/desktop/src/lib/ipc.ts` 章节的「全字段 payload parse」用例一次遍历该图全部字段 getter（本变更新增 65 个）。越线差额只需 36 个新增覆盖函数（1762 − 1726），本节覆盖贡献 ≈ 60+（`ArchiDriftReport` 14 + `ArchiDriftModel` 5 + `ArchiScanReport` 7 + 三处共用 `ArchiDriftEntry` 6 + `ArchiDriftOccurrence` 4 + `ArchiDriftSuppressedEntry` 6 + `ArchiDriftObservation` 4 + `ArchiGateCount` 3 + `ArchiDiagnostic` 5 + `ArchiExcludedFile` 2 + `ArchiMatchedElement` 2 + `ArchiUnresolvedFile` 3 + 三张表项 getter），余量充足。MUST NOT 以调阈值 / 剔分母 / 快照生成物替代该节用例。

### 待测功能条目来源约定

`#### 待测功能` 只登记 design「公共函数 / API」子表中「所在文件」为该源文件的行（原文签名 + 出处标注「（公共函数 / API）」）。源文件在该子表**无行**时，保留章节框架 + HTML 注释说明原因，**不 grep 源文件导出声明、不虚构条目**；此时测试对象来源写在紧随的 `> 说明` 行（design「类型定义」/「修改文件」单元格 / D 编号 / AC），仍不由源码反推。

### 参数类型 → 边界覆盖口径

| 类型 | 覆盖要求 | 本变更加载点（按 API 入参 / 字段） |
|---|---|---|
| int / number | `0`、`-1`、MAX、`None` + 1 正常 | `line: u32`（`0` = 位置未知 / `1` 首行 / 中间 / `u32::MAX`；`-1` 在 `u32` 类型层不可表达 → 以 `0` 与越界表达）、`version: u32`（`0` / `1` / `u32::MAX`）、计数族 `elements` / `relations` / `changed_files` / `scanned_files`（`0` / `1` / 300 元素与 1000 文件级）、`ARCHI_FEEDBACK_LIMIT = 3`（计数 0 / 1 / 3 / 4 超限）、`MAX_REPORT_CHARS = 2000`（文本 0 / 恰好 / 越界截断） |
| str / string | `""`、超长（>1000 chars）、特殊字符（`\n` / `\0` / emoji）、`None` + 1 正常 | `root`（`""` / `"   "` / 不存在 / 文件而非目录 / 含空格与非 ASCII）、`specifier`（相对 / 别名 / 外部包 / `@scope/pkg` / 超长 / emoji）、`file_rel` / `rel_path`（`\` 分隔 / `./` 前缀 / 尾分隔符 / 绝对路径）、`reason`（`""` / 纯空白 / 超长）、`fqn`（含点 / 空格 / glob 形态 `src/**`）、`name` / `change`、`message`、`timestamp`、`model_hash` |
| bool | `true`、`false`、`None` | `type_only`（两值）、`passed`（两值）、`ArchiCheckOutcome` 五字段的 `fault: None` 与 `Some(Stopped)` |
| list / array | `[]`、单元素、超大、`None` + 1 正常 | `changed: &[String]`（`[]` / 单 / 1000 条）、`exclude: &[String]`（`[]` / 单 / 100 条 / 含非法项）、`elements: &[ArchiElement]`（`[]` / 1 / 300）、`errors` / `warnings` / `suppressed` / `observations` / `occurrences` / `unresolved` / `excluded` / `matched`（各自 `[]` / 单 / 超大）、`Version` 与 `ALL` 常量（长度断言） |
| dict / object | `{}`、缺失必填字段、多余字段、`None` + 1 正常 | `CheckInput` / `ScanInput` 字段面（缺失字段在 Rust 侧为编译期红 → 以「字段齐备」+ 语义边界表达）、`ignore[]` 条目（缺 `reason` / `from` 非 FQN → `Err`）、`<archi_dir>/config.json`（`{}` / 未知键 / 顶层非对象）、`ArchiDriftReport` 14 字段（Option 族 `change` / `base` 两态） |
| Optional[T] | `None` | `report_ref`（`None` 写盘失败）、`change`（命令面恒 `None`）、`base`（全量 `None` / 增量 `Some("HEAD")`）、`specifier`、`inline_ignore`、`reason`、`gate`、`from` / `to`（path 类条目）、`suppressed[].file` / `line`（`via = config` 时双 `None`） |
| Enum | 每个枚举值 + 非法枚举值（N+1） | `ArchiCheckStatus`(5)、`ArchiDriftKind`(4)、`ArchiObservationKind`(2)、`ArchiDriftMode`(2)、`ArchiSuppressVia`(2)、`ReportSlot`(3)、`CheckError`(3)、`Owner`(3)、`GateTier`(3)、`RunFault`（`Stopped` 侧）、`StepNode` / `RunStepKind` / `ChangeStepKind`(各 22)、审计 `StepKind`(9)；非法值以 serde 反序列化 `Err` 与 `as_str` 词汇外退化表达 |
| float | `0.0`、`-0.0`、`NaN`、`Inf`、`None` | **不适用**：本变更零 float 入参 / 字段（运行期 p95 预算归 test-execution 相位计时装置，不进单测断言）——显式声明不覆盖，不虚构 float 面 |

---

## 验收范围

| AC ID | 验收条件（缩写，原文见 proposal） | 被测文件或模块 |
|---|---|---|
| AC-1 | 扫描范围与测试 scope 正交：测试文件跨元素 import 零违例 + `scan.excluded[]` 留痕；改 `tests[].excludes` 后扫描面不变；配置 `exclude` 为追加 | `packages/desktop/src-tauri/crates/core/archi/src/scan/exclude.rs`、`packages/desktop/src-tauri/crates/core/archi/src/scan/mod.rs` |
| AC-2 | 归属单点四形态：最长前缀 / 同长多命中 `ambiguousOwner` + unmatched / 仅被 `off` 覆盖计 unmatched / `path` 缺失 `noPath` | `packages/desktop/src-tauri/crates/core/archi/src/scan/owner.rs` |
| AC-3 | 两套解析器一套归属：TS 别名经 tsconfig `paths`；Rust 四种模块树形态反查文件；外部 crate 不进归属；长尾逐条 `unresolved` | `packages/desktop/src-tauri/crates/core/archi/src/scan/ts.rs`、`packages/desktop/src-tauri/crates/core/archi/src/scan/rust.rs`、`packages/desktop/src-tauri/crates/core/archi/src/scan/mod.rs` |
| AC-4 | 定级只看引入方：同一 `A 引入 B` 三档出 error / warning / 零记录；目标文件无主（目标不可归属）→ 丢边 + `scan.unmatchedFiles[]` 留痕、零违例（D7）；`to` 元素名不在模型内（spec 的防御臂）→ warning；`import type` 同权 | `packages/desktop/src-tauri/crates/core/archi/src/drift/mod.rs` |
| AC-5 | `occurrences` 按元素对聚合：3 文件各 1 处 → 1 条记录 + 3 项 occurrences | `packages/desktop/src-tauri/crates/core/archi/src/drift/mod.rs` |
| AC-6 | 豁免两层留痕且粒度封闭：内联 / 配置各不报并进 `suppressed[]`；`reason` 缺失、FQN 不存在、glob 形态 `ignore` 三者显式 `Err`；补声明后转正常声明 | `packages/desktop/src-tauri/crates/core/archi/src/drift/config.rs`、`packages/desktop/src-tauri/crates/core/archi/src/drift/mod.rs` |
| AC-7 | 两模式一致且落盘面分离：增量 ⊆ 全量（元素对级）；全量不落盘 / 增量落 `reports/archi/<timestamp>.json`；无变更 → `noChanges` | `packages/desktop/src-tauri/crates/core/archi/src/drift/mod.rs`、`packages/desktop/src-tauri/crates/infra/archi/src/runner.rs` |
| AC-8 | `status` 五值可辨且 `skipped` / `noChanges` 不被当通过；`observations` 不影响 `status`；`modelHash` 随模型变化 | `packages/desktop/src-tauri/crates/core/archi/src/drift/report.rs`、`packages/desktop/src-tauri/crates/core/archi/src/drift/mod.rs`、`packages/desktop/src-tauri/crates/core/archi/src/lib.rs`（出口 re-export，见「不可测试项」） |
| AC-9 | 看护配置面写死：`<archi_dir>/config.json` 缺失即空 / 未知键拒绝；工作区配置零变化；产品 `.rs` 无 `config.json` 字面量拼接 | `packages/desktop/src-tauri/crates/core/archi/src/drift/config.rs`、`packages/desktop/src-tauri/crates/core/foundation/src/layout/mod.rs` |
| AC-10 | 判定单源：命令面与门禁 runner 同输入产出的报告除 `timestamp` / `change` / 落盘路径外逐字段相等；无第二份归属 / 定级 / status 实现 | `packages/desktop/src-tauri/src/commands/archi/mod.rs`、`packages/desktop/src-tauri/crates/infra/archi/src/runner.rs` |
| AC-11 | 门禁只挂 implement：implement 7 步且 `ArchiCheck{3}` 位于 static-check 与 evaluator 之间；其余相位与 test-only 表零 `ArchiCheck`；无运行时开关 | `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs` |
| AC-12 | 反馈边语义：违例注入同一 executor 会话（`EdgeKind::Feedback` + `instance_no` 递增）；3 次超限升格（代写 fail `phase_log`、不跑 evaluator）；通过进 evaluator；文本含固定处置提示；16 份 prompt md 零变化 | `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` |
| AC-13 | `skipped` / `noChanges` 不冒充通过：步 `passed` 且 detail 逐字「未检查（无模型）」/「未检查（无可判文件）」；报告 `status` 原样保留 | `packages/desktop/src-tauri/crates/infra/archi/src/runner.rs`、`packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` |
| AC-14 | runner 落盘与降级：报告落 `reports/archi/` 且 `report_ref` 回填；目录不可写时结论保留 + `report_ref = None` + 步不失败；`reports/test` / `reports/baseline` 字节零变化 | `packages/desktop/src-tauri/crates/infra/archi/src/report.rs`、`packages/desktop/src-tauri/crates/infra/archi/src/runner.rs`、`packages/desktop/src-tauri/crates/infra/archi/src/lib.rs`（出口挂载，见「不可测试项」）、`packages/desktop/src-tauri/crates/infra/checks/src/static_check.rs`、`packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod.rs`、`packages/desktop/src-tauri/crates/core/foundation/src/layout/mod.rs` |
| AC-15 | 输入口径：模型与源码同根；`change_edit_set` 失败即步失败（不降级空清单）；零读工作区配置 | `packages/desktop/src-tauri/crates/core/orchestration/src/steps.rs`、`packages/desktop/src-tauri/crates/core/orchestration/src/port.rs`、`packages/desktop/src-tauri/src/commands/change_flow/mod.rs` |
| AC-16 | 步词汇 22 值全链路：四枚举同词面 1:1、`archi_check` 尾部追加、detail 出线在案、前端 `normalizeKind` / `stepWord` / `STEP_LABEL` / 分组穷尽、语料 golden 覆盖 | `packages/desktop/src-tauri/crates/core/workflow/src/state.rs`、`packages/desktop/src-tauri/crates/core/orchestration/src/state.rs`、`packages/desktop/src-tauri/crates/core/orchestration/src/run_history.rs`、`packages/desktop/src-tauri/crates/core/orchestration/src/lib.rs`（出口 re-export，见「不可测试项」）、`packages/desktop/src-tauri/crates/core/orchestration/src/control.rs`、`packages/desktop/src-tauri/crates/core/workflow/tests/corpus_golden_test.rs`、`packages/desktop/src/views/changes/flow/run-state.ts`、`packages/desktop/src/views/changes/flow/run-step-node.tsx` |
| AC-17 | 依赖与分层：`archi` 的 workspace 内依赖仅 `foundation`、零 spawn；`archi-runtime` 依赖恰为 `archi` + `orchestration` + `foundation`；`workflow` 零 `archi` 依赖；壳依赖带用途注释 | `packages/desktop/src-tauri/crates/core/archi/Cargo.toml`、`packages/desktop/src-tauri/crates/infra/archi/Cargo.toml`、`packages/desktop/src-tauri/Cargo.toml`（清单面，见「不可测试项」） |
| AC-18 | 成本与预算：`Cargo.lock` 净增 ≤ 40；`archi_check` p95 ≤ 3 s（1000 文件 / 300 元素 release）；五项实测表落 `tests/README.md` 并抄进 `design.md`；越硬线时回退序留痕 | `packages/desktop/src-tauri/crates/core/archi/tests/README.md`（测量记录，见「不可测试项」）；`crates/core/archi/tests/perf.rs`（p95 装置，执行与回填归 test-execution 相位，见「不可测试项」） |
| AC-19 | 标记生产与呈现：`unfulfilledDeclaration` → 边 + 节点 `unfulfilled`、`suppressed` → `exempt`（抽屉 reason）、其余 `none`；`clean` / `skipped` / `noChanges` 或取数失败 → 全 `none` 且图照常渲染；`marks.ts` 零改动 | `packages/desktop/src/views/architecture/lib/drift-marks.ts`、`packages/desktop/src/views/architecture/hooks/use-archi-drift.ts`、`packages/desktop/src/views/architecture/components/architecture-canvas.tsx` |
| AC-20 | 交付面：`bindings:check` 零 diff；`version` 0.6.5 且 `src-tauri/Cargo.toml` 不随动；`vp check` / `knip` / `cargo clippy -D warnings` 通过 | `packages/desktop/src-tauri/src/commands/mod.rs`、`packages/desktop/src/lib/ipc.ts`（运行期校验面：`ipc.archiDrift` / `resultSchemas.archiDrift` 全字段 parse，覆盖 drift/scan schema 图）、`packages/desktop/src/types/generated/bindings.ts` + `bindings.zod.ts`（生成物结构面与版本行见「不可测试项」）、`packages/desktop/package.json` |
| AC-21 | 模型与新增 crate 同 change 同步：`crates/infra/archi` ↔ `App.Backend.Infra.ArchiRuntime`（`kind = module` / `gate = off` / `path` 逐字 / `summary` 非空不复述 FQN）；`03` 段 8 元素 24 关系、`04` 段 13 关系、全基线 50 元素 / 111 关系（后端 47 + 前端 64）；15 自研 crate ↔ 15 crate 元素零差集（`specta-zod-patched` 单列已知缺口）；模型表头 / spec 计数表 / ADR `§9` 三处同值；`archi_graph(root)` 零诊断零 `warnings`、`warn` / `strict` 零命中；四条边在案 | `.dev-team/architecture/models/03-backend-infra.toml`、`.dev-team/architecture/models/04-backend-shell.toml`、`.dev-team/architecture/decisions/0001-model-granularity.md`（数据资产，核对以「命令 + 留痕」执行，见「不可测试项」）；`openspec/changes/archi-check-and-gate/reports/archi/model-sync-check.md`（核对留痕） |

---

## 单元测试

### packages/desktop/src-tauri/crates/core/archi/src/scan/mod.rs -> packages/desktop/src-tauri/crates/core/archi/src/scan/mod_test.rs

#### 待测功能

- `scan()`: 扫描编排单点——gitignore 感知枚举 → 排除 → 归属 → 双解析器抽取 → 跨元素边聚合 → `ScanOutcome { report, edges }`；取消置位 → `Err(CheckError::Stopped)`；零写盘、零 spawn（公共函数 / API）
- `inline_ignore()`: 取该行与上一行匹配 `// archi-ignore-next-line:`，冒号后非空文本即 `reason`；空 `reason` 视为未标记（公共函数 / API）

> 说明：`ScanInput` / `ScanOutcome` / `ImportFact` / `DependencyEdge` 字段面取自 design「类型定义」，用例按 D4（scan 只出事实）/ D5（内联标记在 scan 侧判定）/ D22（计数不变量）/ AC-1 / AC-3 组织；跨模块组合用例（`scan → drift::check`）挂 `drift/mod.rs` 章节，本节只覆盖扫描面自身。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `scan` 全链路（单跨元素边） | 正向 | tempdir：模型两元素（`App.Auth` / `App.Billing` 各持 `./src/auth/`、`./src/billing/`）+ `src/billing/x.ts` 写 `import '../auth/token'` → `edges` 恰 1 条（`from` / `to` 按归属单点、`file` 为仓相对 `/` 分隔、`line > 0`、`specifier` 原文、`type_only = false`、`inline_ignore = None`），`report.matched[]` 两元素（`files` 字典序），`unmatched_files` 空 | 单例 |
| 同元素内边剔除 | 边界 | 同元素两文件互 import → `edges` 空且 `report.unresolved` 空（同元素内不报是判定对象边界，非留痕项） | 单例 |
| 逐文件一行不聚合 | 正向 | 同一 `(from, to)` 在 3 文件各 1 处 → `edges` 3 行（`scan` 只出事实，按 `(from,to)` 聚合归 `drift`，D4/AC-5） | 表驱动（3 文件） |
| 测试文件排除留痕（AC-1） | 正向 | `src/auth/auth.test.ts` 跨元素 import → `report.excluded[]` 一条 `{ file, rule }`（`rule` 为命中的具体 glob，D8），`edges` 空 | 表驱动（`*.test.ts` / `*.spec.tsx` / `__tests__/x.ts` / `x_test.rs` / `tests/` 下 / `fixtures/` 下 / `__mocks__/` 下，7 形态） |
| 配置追加不替换（AC-1） | 边界 | `exclude = ["src/legacy/**"]` + 一个 `*.test.ts` → 两文件分别命中配置规则与默认规则（`excluded[]` 两条，`rule` 各异），默认七条仍生效 | 表驱动（追加后默认 + 配置各命中） |
| 工作区测试 scope 零耦合（AC-1） | 边界 | tempdir 内额外落 `openspec/config.json` 含 `tests[].excludes: ["src/"]` → `scanned_files` 与该文件缺席时**逐值相同**（scan 零读工作区配置） | 单例（前后对照） |
| `scannedFiles` 不变量（D22） | 边界 | 0 可判源文件 / 全部被排除 / 单文件三形态：`report.scanned_files == 可判源文件数 − excluded.len()` 恒成立；`unresolved` 不计入 `scanned_files` | 表驱动（3 形态 + 不变量断言） |
| `changedFiles` 模式分流（D22） | 边界 | `mode = full` → `changed_files = 0`；`mode = incremental` + `changed = []` → `changed_files = 0` 且 `scanned_files = 0`（零变更集不扫）；`changed` 含非源扩展名（`.md` / `.json`）→ 不计入；`changed` 含测试文件 → 计入 `changed_files` 但进 `excluded[]` | 表驱动（4 形态） |
| 增量模式只扫变更集 | 正向 | `changed = [变更文件]` + 仓内另有未变更跨元素文件 → `edges` 只含变更文件产出的边（R-6 的增量语义，逐条断言 `edges[].file ∈ changed`） | 表驱动（含 / 不含变更文件） |
| 大列表边界 | 边界 | `changed` 1000 条（多数不存在的路径）→ 不 panic；`changed` 单元素 / 空 → 上表覆盖 | 表驱动（3 形态） |
| 取消（异常） | 异常 | `cancel` 预置位 → `Err(CheckError::Stopped)`（零部分结果，`ScanOutcome` 不返回） | 单例 |
| 归属不确定留痕（AC-2 交汇） | 异常 | 同长多命中的文件 → 进 `report.unmatched_files`、`matched[]` 不含它、`edges` 空（不任选，D7） | 单例 |
| `path` 缺失元素（AC-2 交汇） | 异常 | 模型含无 `path` 元素 → 该元素不进 `matched[]`、零归属副作用（`noPath` 条目由 `drift` 装配，D4） | 单例 |
| 未解析留痕（AC-3） | 异常 | 动态 `import('x')` / `require('y')` / 不可解析 specifier 三形态 → `report.unresolved[]` 各一条（`file` / `specifier: Some(..)` / `message` 非空），`errors` 面（由 `drift` 装配）不因它们产生 | 表驱动（3 形态） |
| 源文件读盘失败 | 异常 | `changed` 指向不存在文件 → 不 panic（该项不进 `unresolved` 也不进 `excluded`，`scanned_files` 不变量仍成立） | 单例 |
| 零写盘 | 边界 | 扫描前后 tempdir 全量字节快照相等（`archi` 只读盘纪律的装置化断言） | 单例 |
| `inline_ignore` 同行命中 | 正向 | 5 行源文、第 5 行为 import 且行尾注 `// archi-ignore-next-line: 迁移期` → `Some("迁移期")` | 单例 |
| `inline_ignore` 上一行命中 | 正向 | 注释在第 4 行（import 上一行）→ 同样返回 `Some("迁移期")`（两种位置同权） | 单例 |
| `inline_ignore` reason 文本边界 | 边界 | 冒号后为 `""` / 纯空白 / 无冒号 / 大小写变体（`// Archi-Ignore-Next-Line: x`）→ 全 `None`；reason 含 `\n` / `\0` / emoji / >1000 chars → 原样返回不 panic；`\r\n` 行尾仍命中 | 表驱动（6 形态） |
| `inline_ignore` 行号边界 | 边界 | `line = 1`（首行 import：只看该行）、`line = 0`（位置未知 → `None`）、`line` 超总行数（越界 → `None`）、源文 `""` 空串（→ `None`） | 表驱动（4 边界） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 文件系统 | **不 mock**：真实 tempdir（自建 `TempDir` RAII，本 crate 无 dev-dep）+ 真实 `fs::write` 落盘夹具 | 全部用例 |
| 排除 / 归属 / 双解析器 | **不 mock**：真实 `ExcludeSet` / `OwnerIndex` / `TsResolver` / `RustResolver` 组合（`scan` 的编排语义即被测对象，替身会引第二事实源） | 全部用例 |
| 取消信号 | **不 mock**：`foundation::cancel::CancelFlag` 真实原子（`never()` / `handle().set()`），不注入假信号 | 取消 / 正常两态 |
| 时间 / 网络 / 子进程 | **零依赖** | —— |

---

### packages/desktop/src-tauri/crates/core/archi/src/scan/exclude.rs -> packages/desktop/src-tauri/crates/core/archi/src/scan/exclude_test.rs

#### 待测功能

- `ExcludeSet::build()`: 默认七条约定（`DEFAULT_EXCLUDE_RULES` 具体 glob）+ 配置追加，globset 编译一次复用；非法配置 glob / 无通配符条目 → `Err`（公共函数 / API）
- `ExcludeSet::matched()`: 命中返回命中的规则原文（最小下标优先，确定性）；未命中 `None`（公共函数 / API）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 默认七条逐条命中（AC-1） | 正向 | 七条约定各取一条代表路径：`src/a.test.ts` / `src/a.spec.tsx` / `src/__tests__/a.ts` / `crates/x/src/a_test.rs` / `tests/a.ts` / `fixtures/a.ts` / `__mocks__/a.ts` → `matched()` 恒 `Some(规则原文)`，且七条命中的 `rule` 互不相同 | 表驱动（7 约定） |
| 默认规则常量形态（D8） | 边界 | `DEFAULT_EXCLUDE_RULES.len() == 13`（七条约定的展开式 `4 + 4 + 1 + 1 + 1 + 1 + 1 = 13`：`*.test.{ts,tsx,js,jsx}` 四值 / `*.spec.{ts,tsx,js,jsx}` 四值 / `__tests__/` / `*_test.rs` / `tests/` / `fixtures/` / `__mocks__/`；design D8 的「12 条」是算术笔误，常量、语料与用例一律以 13 为准）；每条含通配符且 `Glob::new` 均可编译；无 `{` / `}`（globset 无花括号方言） | 表驱动（13 条逐条） |
| 扩展名变体全覆盖 | 边界 | `*.test.{ts,tsx,js,jsx}` 四扩展名与 `*.spec.{ts,tsx,js,jsx}` 四扩展名各自命中（8 形态，防展开漏项） | 表驱动（8 形态） |
| 未命中 | 正向 | `src/auth/token.ts` / `src/auth/token.tsx` / `src/legacy/x.rs`（无配置时）→ `None` | 表驱动（3 形态） |
| 配置追加非替换（AC-1） | 正向 | `build(&["src/legacy/**"])` → 该目录文件命中配置规则；同时 `src/a.test.ts` 仍命中默认规则（两规则各自可辨） | 表驱动（配置 + 默认各一） |
| 空配置 | 边界 | `build(&[])` → `Ok`，行为与默认集逐条相同（7 命中 + 未命中集合相等） | 单例 |
| 最小下标优先（确定性） | 边界 | 一条路径同时命中默认规则与配置规则（如 `build(&["**/*.test.ts"])` 与默认同形）→ 两次 `matched()` 返回值相等，且恒为下标最小那条（不随 HashMap 序漂移） | 表驱动（两次调用对拍） |
| 配置非法 glob（异常） | 异常 | `build(&["src/[legacy"])` → `Err`，文案含该条目原文 | 单例 |
| 配置无通配符（异常） | 异常 | `build(&["src/legacy"])` / `build(&[""])` / `build(&["src/"])` → 全 `Err`（目录须写 `src/legacy/**`） | 表驱动（3 形态） |
| 超长 / 特殊字符条目 | 边界 | `build(&["a".repeat(1200).as_str()])`（>1000 chars + 通配符）→ 不 panic（`Err` 或 `Ok` 皆可，断言仅「不崩 + 返回型可判」）；含 emoji 的非法条目 → `Err` | 表驱动（2 形态） |
| 超大配置列表 | 边界 | `build` 接收 100 条合法 glob → `Ok`；建一次后在 1000 次 `matched()` 调用中结果稳定（编译复用，非每次重编译） | 单例（复用 + 批量） |
| 路径形态归一 | 边界 | `matched("src\\a.test.ts")` / `matched("./src/a.test.ts")` / `matched("/src/a.test.ts")` → 与 `matched("src/a.test.ts")` 同结果（`\` → `/`、剥 `./`，设计未归一者以「不 panic + 结果可判」表达） | 表驱动（3 形态） |
| 大小写敏感 | 边界 | `src/A.Test.TS` → `None`（不做大小写归一，词面精确） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| globset 引擎 | **不 mock**：真实 `globset::GlobSet` 编译与匹配（排除语义即被测对象） | 全部用例 |
| 文件系统 | **零依赖**：本模块纯字符串匹配，零 IO（夹具为路径字面量） | 全部用例 |
| 配置来源 | **不 mock**：`build` 的 `extra` 以参数直传（配置读取归 `drift/config.rs`，本节不触盘） | 追加 / 非法条目 |
| 进程 / 网络 / 时间 | **零依赖** | —— |

---

### packages/desktop/src-tauri/crates/core/archi/src/scan/owner.rs -> packages/desktop/src-tauri/crates/core/archi/src/scan/owner_test.rs

#### 待测功能

- `OwnerIndex::build()`: 只收 `path` 非空元素；条目归一（`\` → `/`、剥 `./`、去尾分隔符）（公共函数 / API）
- `OwnerIndex::owner_of()`: 最长前缀——唯一 → `Element`；同长多命中 → `Ambiguous`（字典序）；无命中 → `None`（公共函数 / API）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 最长前缀唯一命中（AC-2） | 正向 | 元素 `App`(`./src/`) + `App.Auth`(`./src/auth/`)，`owner_of("src/auth/token.ts")` → `Element { fqn: "App.Auth", gate: <该元素档> }`（取最长而非首个命中） | 表驱动（`App` / `App.Auth` 两级各一文件） |
| `gate` 透传 | 正向 | 三档元素（`strict` / `warn` / `off`）各覆盖一文件 → `Owner::Element.gate` 逐档相等（定级依据随归属单点出线，AC-4） | 表驱动（3 档） |
| `off` 仍可归属（D6） | 边界 | 文件只被 `gate = off` 元素覆盖 → 仍返回 `Element`（不折叠成 `None`）；「计 unmatched」由调用侧决定（`scan` 章节承载） | 单例 |
| 同长多命中不任选（AC-2） | 异常 | 两元素 `path` 长度相同的不同前缀同时覆盖某文件 → `Ambiguous(vec![..])`，向量**字典序**且两次调用相等（确定性） | 表驱动（两元素 / 三元素同长） |
| 无命中 | 异常 | 空索引 / 索引只覆盖 `src/auth/` 时 `owner_of("src/billing/x.ts")` → `Owner::None` | 表驱动（2 形态） |
| `path` 归一三写法 | 边界 | 元素 `path` 分别写 `./src/auth/`、`src\auth`、`src/auth` → 对同一文件三者的 `owner_of` 结果相等（`\` → `/`、剥 `./`、去尾分隔符） | 表驱动（3 写法 × 同文件） |
| 文件级 `path` | 边界 | 元素 `path = ["./src/auth/index.ts"]` → 该文件命中、同目录 `src/auth/other.ts` → `None`（单文件粒度精确） | 表驱动（2 文件） |
| 根 / 空前缀 | 边界 | 元素 `path = ["./"]` 与 `path = [""]`：`""` 条目不入选（`build` 只收非空）；`./` 归一为空前缀 → 覆盖一切文件且**不抢占**更长的 `src/auth/`（最长前缀优先） | 表驱动（2 形态 + 抢占对照） |
| 空元素列表 | 边界 | `build(&[])` → 任何 `owner_of` 恒 `None`（零 panic） | 单例 |
| 300 元素规模 | 边界 | 300 元素（其中 100 条同长重叠路径）→ 命中项恒为最长且同长时 `Ambiguous`（表驱动 5 个探针文件）；构建一次后多次查询结果稳定 | 表驱动（5 探针） |
| 大小写 / 空格 | 边界 | `owner_of("src/Auth/token.ts")` 对 `src/auth/` → `None`（不做大小写归一）；带前后空格的 `rel_path` → 不 panic，结果可判 | 表驱动（2 形态） |
| 非 ASCII 与超长路径 | 边界 | `owner_of("src/认证/token.ts")` 命中 `src/认证/`；>1000 chars 路径 → `None` 不 panic | 表驱动（2 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 模型 / 图 | **不 mock**：`ArchiElement` / `GateTier` 直接构造（纯值，不经解析，AC-2 用例要求「直接造模型，不经解析」） | 全部用例 |
| 文件系统 | **零依赖**：纯字符串前缀匹配，零 IO | 全部用例 |
| 进程 / 网络 / 时间 | **零依赖** | —— |

---

### packages/desktop/src-tauri/crates/core/archi/src/scan/ts.rs -> packages/desktop/src-tauri/crates/core/archi/src/scan/ts_test.rs

#### 待测功能

- `TsResolver::new()`: 观察根锚（specifier 落盘与 tsconfig 向上查找）（公共函数 / API）
- `TsResolver::extract()`: 静态 `import` / `import type` / `export … from` 的 specifier + 行号 + `typeOnly`；动态 `import()` / `require()` / 语法错 → `unresolved`（公共函数 / API）
- `TsResolver::resolve()`: 就近 tsconfig / jsconfig 的 `baseUrl` + `paths`（解析器按配置树缓存）→ node 规则；失败 `None`（公共函数 / API）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `extract` 静态 import 三形态 | 正向 | `import a from './x'` / `import { b } from './y'` / `import * as c from './z'` → 3 条 `ImportFact`（specifier 原文、`line` 为 import 关键字所在行、`type_only = false`） | 表驱动（3 形态） |
| `extract` `import type`（AC-4） | 正向 | `import type { T } from './types'` → 1 条且 `type_only = true`（与普通 import 同权产出，只标注） | 单例 |
| `extract` `export … from` | 正向 | `export { a } from './a'` / `export * from './b'` / `export type { T } from './c'` → 3 条（末条 `type_only = true`） | 表驱动（3 形态） |
| `extract` 行号精度 | 边界 | 跨行的多行 import（大括号换行 5 行）→ `line` 为 `import` 关键字行；文件首行 import → `line == 1`；第 1000 行 import → `line == 1000` | 表驱动（3 位置） |
| `extract` 动态导入 / `require`（AC-3 长尾） | 异常 | `const m = await import('./dyn')` / `const r = require('./req')` / `import(\`./${x}\`)` 三形态 → 各产 `unresolved`（`file` / `specifier` / `message` 非空），零 `ImportFact` | 表驱动（3 形态） |
| `extract` 语法错（异常） | 异常 | 半个文件截断 / 未闭合括号 / `<<<>>>` → `unresolved` 一条且**不 panic**、已解析出的前缀 `ImportFact` 允许保留（断言：无 panic + `unresolved` 非空） | 表驱动（3 形态） |
| `extract` 零 fact 面 | 边界 | `""` 空源 / 仅注释 / 仅类型声明无 import → 零 `ImportFact` 零 `unresolved`（空串与注释不冒充留痕） | 表驱动（3 形态） |
| `extract` specifier 文本边界 | 边界 | `'@scope/pkg'` / `'#internal'` / `'./a?raw'` / emoji 路径 / >1000 chars specifier → 各产 1 条 fact 且原文逐字（不做归一、不截断） | 表驱动（5 形态） |
| `extract` 混合文件 | 正向 | 同文件含 2 静态 + 1 动态 + 1 `import type` → `ImportFact` 3 条 + `unresolved` 1 条，行号互不串位 | 单例 |
| `resolve` 相对路径（node 规则） | 正向 | `./x` 对应 `src/x.ts` / `src/x/index.ts` / `src/x.tsx` 三形态（三种文件布局各一）→ `Some("src/x…")` 仓相对 `/` 分隔 | 表驱动（3 布局） |
| `resolve` tsconfig `paths`（AC-3） | 正向 | tsconfig `{ baseUrl: ".", paths: { "@/*": ["src/*"] } }` + `resolve("src/a.ts", "@/auth/token")` → `Some("src/auth/token.ts")` | 单例 |
| `resolve` jsconfig 同权 | 正向 | 仅 `jsconfig.json`（同 `baseUrl` / `paths`）→ 与 tsconfig 同结果 | 单例 |
| `resolve` 就近配置（缓存分树） | 边界 | 根 tsconfig（`@/* → src/*`）+ `packages/web/tsconfig.json`（`@/* → packages/web/src/*`）→ 两子树内同名 specifier 各落各自映射（`&mut self` 缓存按配置树不串味） | 表驱动（2 子树） |
| `resolve` 配置缺失 / 只有 `baseUrl` | 边界 | 无 tsconfig / jsconfig → node 规则仍可用；仅 `baseUrl` 无 `paths` → `baseUrl` 相对解析生效；`paths` 无 `baseUrl` → 按配置树同目录解析（不 panic） | 表驱动（3 形态） |
| `resolve` 外部包（AC-3） | 异常 | `resolve("src/a.ts", "react")` / `"node:fs"` / `"lodash/get"` → `None`（外部包静默丢弃，不当 unresolved） | 表驱动（3 形态） |
| `resolve` 不可落盘（异常） | 异常 | `"./missing"` / 畸形 specifier `"\0"` / >1000 chars → `None` 且不 panic | 表驱动（3 形态） |
| `resolve` 确定性 | 边界 | 同一 `(file_rel, specifier)` 连续三次 `resolve` 结果相等（缓存不改变结论） | 单例 |
| `new` 根边界 | 边界 | 不存在的 root / 文件而非目录的 root / 含空格与非 ASCII 的 root → 构造零 panic，后续 `extract` / `resolve` 返回型可判 | 表驱动（3 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| oxc 词法 / AST 与模块解析 | **不 mock**：真实 `oxc_parser` / `oxc_allocator` / `oxc_resolver`（解析语义即被测对象；替身会掩盖版本与特性面问题，R-1） | 全部用例 |
| 文件系统 | **不 mock**：真实 tempdir + 真实 tsconfig / jsconfig / 源文件落盘（`resolve` 的落盘判定必须真盘） | `resolve` 全部用例 |
| 时间 / 网络 / 子进程 | **零依赖** | —— |

---

### packages/desktop/src-tauri/crates/core/archi/src/scan/rust.rs -> packages/desktop/src-tauri/crates/core/archi/src/scan/rust_test.rs

#### 待测功能

- `RustResolver::new()`: 解析 workspace 清单 + member glob → crate 清单 → 逐 crate 模块树；清单不可解析 → `Err`（公共函数 / API）
- `RustResolver::unresolved()`: 建树期不可判定项（宏 `mod` / `include!` / 孤儿 `.rs` / `mod` 定位失败）（公共函数 / API）
- `RustResolver::extract()`: 单文件 `use` 树抽取（`crate::` / `self::` / `super::` / workspace crate 名）；`syn` 解析失败 → `unresolved`（公共函数 / API）
- `RustResolver::resolve()`: 模块路径最长前缀匹配落到文件；外部 crate → `None`（静默丢弃）（公共函数 / API）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `new` 清单解析（零子进程） | 正向 | tempdir workspace（`[workspace] members = ["crates/*"]`）+ 两 member 各带 `Cargo.toml` → 构造 `Ok`；两 member 的文件可被 `resolve` 命中（member glob 展开生效） | 表驱动（2 member） |
| 模块树形态一：`mod foo;` | 正向 | `src/lib.rs` 的 `mod foo;` + `src/foo.rs` → `resolve("src/lib.rs", "crate::foo::Bar")` → `Some("src/foo.rs")`（AC-3 形态 1） | 单例 |
| 模块树形态二：`foo/mod.rs` | 正向 | `src/foo/mod.rs` + `src/foo/bar.rs`，`mod foo;` + `mod bar;` → 两条 `use` 分别落 `src/foo/mod.rs` / `src/foo/bar.rs`（AC-3 形态 2） | 表驱动（2 目标） |
| 模块树形态三：`#[path]` 优先 | 正向 | `#[path = "custom/x.rs"] mod x;` 且同目录另有 `x.rs` → `resolve` 落 `custom/x.rs`（`#[path]` 覆盖默认推导，AC-3 形态 3） | 单例 |
| 模块树形态四：`cfg` 两臂同路径 | 边界 | `#[cfg(unix)] mod a;` + `#[cfg(windows)] mod a;` → 建树不报重复、`unresolved` 空、`resolve` 落同一文件（AC-3 形态 4） | 单例 |
| 内联 `mod x { … }` | 边界 | 内联模块内的 `use crate::y::Z;` → 按内联路径上溯落到 `src/y.rs`（内联模块不产文件） | 单例 |
| `use` 前缀四形态 | 正向 | `crate::a::B` / `self::x::Y` / `super::z::W` / `other_crate::P` 四形态各一 → 前三落各自文件、第四落 workspace crate 根（`lib.rs`） | 表驱动（4 形态） |
| 最长前缀匹配 | 边界 | `crate::a` 与 `crate::a::b` 均建树，`use crate::a::b::C;` → 落 `b` 的文件（最长，不落 `a`） | 单例 |
| 外部 crate 静默丢弃（AC-3） | 异常 | `use std::collections::HashMap;` / `use serde::Deserialize;` → `resolve` 恒 `None`，且 `extract` 不把它们记入 `unresolved`（`errors` / `warnings` 面零副作用） | 表驱动（2 形态） |
| 建树长尾留痕（AC-3） | 异常 | 宏生成的 `mod`（`macro_rules! make_mod`）/ `include!("gen.rs")` / 无 `mod` 声明的孤儿 `src/orphan.rs` / `mod ghost;` 却无对应文件 → 四类各一条 `unresolved`（`file` / `message` 非空），`unresolved()` 与 `extract` 返回的并集可查 | 表驱动（4 形态） |
| `extract` 语法错 | 异常 | 截断的 `.rs` / 未闭合泛型 → 一条 `unresolved` 且不 panic；同文件已抽出的 `use` 允许保留 | 表驱动（2 形态） |
| `extract` 行号与多 use | 正向 | 单文件 3 条 `use`（首行 / 中间 / 末行）→ 3 条 `ImportFact`，`line` 各自正确、`type_only` 恒 `false`（Rust `use` 无类型专用形态） | 表驱动（3 位置） |
| `extract` 零 use 面 | 边界 | 空文件 / 仅注释 / 仅 `fn` 定义 → 零 fact 零 `unresolved` | 表驱动（3 形态） |
| 清单不可解析（异常） | 异常 | `Cargo.toml` 非法 TOML / 缺 `[workspace]` 且顶层非 package / 空文件 → `new` 返回 `Err`（文案指明清单），不 panic | 表驱动（3 形态） |
| member glob 无匹配 | 边界 | `members = ["crates/*"]` 但零子目录 → `new` 仍 `Ok`（crate 清单空），所有 `resolve` 恒 `None` | 单例 |
| `resolve` 未命中 | 异常 | 未知模块路径 / 空 specifier `""` / 超长（>1000 chars）→ `None` 不 panic | 表驱动（3 形态） |
| 规模边界 | 边界 | 1000 个 `use` 行 + 300 个模块 → `extract` 全量返回、`resolve` 命中稳定（构建一次复用，不每查询重建树） | 单例（批量 + 复用） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `syn` 解析与模块树 | **不 mock**：真实 `syn 2`（`features = ["full"]`）+ 真实 `toml` 清单解析 | 全部用例 |
| 文件系统 | **不 mock**：真实 tempdir workspace（清单 / crate / 模块文件全落盘；`#[path]` 与孤儿文件判定必须真盘） | 全部用例 |
| `cargo metadata` / 子进程 | **零依赖**：`RustResolver` 零 spawn（AC-3 纪律），测试亦不 spawn | —— |
| 时间 / 网络 | **零依赖** | —— |

---

### packages/desktop/src-tauri/crates/core/archi/src/drift/mod.rs -> packages/desktop/src-tauri/crates/core/archi/src/drift/mod_test.rs

#### 待测功能

- `check()`: 判定唯一入口——模型读取（`dto::graph_view`）→ 看护配置 → 扫描 → ①/②/④ 对账 → 定级（只看引入方）→ 豁免裁决（声明 > 配置 > 内联）→ `occurrences` 聚合 → `status` 推导 → 报告装配（公共函数 / API）
- `CheckError::message()`: 错误面人读文案（`Stopped` → 「检查已取消」；其余透传内层）（公共函数 / API）

> 说明：`CheckInput` 字段面取自 design「类型定义」，`status` 推导序取自 D10、结构错归属取自 D11、观察项口径取自 D13、扫描计数取自 D22；**跨模块组合用例**（`dto::graph_view → drift::check → ArchiDriftReport` 与 `scan → drift::check` 的判定语料）按链路入口挂靠本节，`describe` 标题写链路方向，不另设集成章节。
> **目标面的两种形态须分清（回溯重定）**：①**目标文件无主**（`Owner::None` / `Owner::Ambiguous`）→ 按 D7 丢弃该边，文件已有 `scan.unmatchedFiles[]` 留痕，`errors[]` / `warnings[]` 全空、`status` 可为 `clean`（语料 `tests/corpus/drift/expected/target-unowned.full.json` 即此形态：`warnings` 空、`status = clean`——MUST NOT 改写成「记 warning」）；②**`to` 元素名不在模型内**（spec AC-4 逐字）在实现里是防御臂，只作源码旁证（运行期不可达，列「不可测试项」）。两者都不得互相顶替。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `check` 无模型 → `skipped`（D3） | 边界 | tempdir 无 `<archi_dir>/models` → `status == skipped`、`errors` / `warnings` 全空、零调 `graph_view`（以源码旁证 + 报告形态断言） | 单例 |
| 模型目录在但零 `*.toml` → `skipped`（D3） | 边界 | `models/` 空目录 / 只含非 `.toml` 文件（`README.md`）→ `status == skipped`（与「空模型」可辨） | 表驱动（2 形态） |
| 空模型合法 → `clean`（冷启动） | 正向 | 模型在但零元素（`models/01.toml` 仅注释）/ 单元素零关系 → `status == clean`、`passed` 面（`errors` + `structural_errors` 空）成立 | 表驱动（2 形态） |
| `warnings` 档 | 正向 | `gate = warn` 元素引入未声明跨元素依赖 → `status == warnings`、`warnings[]` 1 条、`errors[]` 空 | 单例 |
| `errors` 档 | 正向 | `gate = strict` 元素引入未声明依赖 → `status == errors`、`errors[]` 1 条（`kind = undeclaredDependency`、`from` 为引入方、`to` 为目标、`gate = strict`） | 单例 |
| 五值全覆盖且互斥（AC-8） | 正向 | 五次构造分别命中 `skipped` / `noChanges` / `clean` / `warnings` / `errors`；逐值断言 `ArchiCheckStatus::ALL` 中每个值都可达（五值无死值、无第十值） | 表驱动（5 值 × 一构造） |
| 推导序：结构错先于 `noChanges`（D10） | 异常 | 坏模型（`dependsOnAncestor`）+ 全部源文件被排除（可判数 0）→ `status == errors`（**不是** `noChanges`） | 单例 |
| 推导序：`errors` 先于 `warnings`（D10） | 边界 | 同一次检查同时命中 strict 违例与 warn 违例 → `status == errors`，两数组各非空 | 单例 |
| `noChanges` 判据（AC-7） | 边界 | 增量模式 `changed = []` → `status == noChanges` + `scanned_files == 0`；全量模式零可判文件（全部被排除）→ 同样 `noChanges` | 表驱动（2 形态） |
| 结构错不进 `errors[]`（D11） | 边界 | 坏模型 + 零源码违例 → `model.structural_errors` 非空、`errors[]` 空、`status == errors`；结构错条目的 `file` / `line` 与 `dto::ArchiDiagnostic` 同形 | 单例 |
| 定级只看引入方（AC-4） | 正向 | 同一 `A 引入 B` 在 `A.gate = strict` / `warn` / `off` 三模型下：`errors[]` 1 条 / `warnings[]` 1 条 / 两者皆空（`off` 时该文件进 `unmatched_files`）；`B.gate` 三档分别替换 → 结论不变（三档 × 两端对照） | 表驱动（3 档 + 目标档对照） |
| 目标 `off` 仍可作 `to`（D6） | 边界 | `strict` 元素引入 `off` 元素 → 仍产 `errors[]` 1 条（`to` 为该 off 元素 FQN，`gate` 为引入方档） | 单例 |
| 目标文件无主 → 丢边 + unmatched 留痕、零违例（D7 / AC-4） | 异常 | tempdir：`App.Billing`（`gate = strict`、`path = ["./src/billing/"]`）+ `src/billing/x.ts` 写 `import '../shared/token'`，模型无元素覆盖 `src/shared/` → `scan.unmatched_files == ["src/shared/token.ts"]`；`errors[]` / `warnings[]` / `suppressed[]` / `observations[]` 全空；`status == clean`（目标不可归属一律**不冒充违例**、也不产 `warnings`；已建模的目标档位不影响该结论） | 表驱动（直接构造 / 语料 `target-unowned`） |
| 「`to` 元素名不在模型内」防御臂（spec AC-4 逐字） | 边界 | 源码旁证：`include_str!("mod.rs")` 含 `!known.contains(to)` 分支与「目标未建模，归因不明」文案，该分支产 `kind = undeclaredDependency`、`gate: None` 的 `warnings[]` 条目；运行时经公开 API 不可达（`owner_of` 只返模型内元素 ⇒ 无输入可达该分支），运行期断言面列「不可测试项」 | 单例（源码自证） |
| `type_only` 同权（AC-4） | 边界 | `import type` 引入未声明依赖 → 与普通 import 同档产出，`occurrences[0].type_only == true`；同元素对同时有普通 import 与 type import → 两条 occurrence | 表驱动（2 形态） |
| `occurrences` 聚合（AC-5） | 正向 | 同一 `(from, to)` 在 3 文件各 1 处 → `errors[]` 恰 1 条 + `occurrences.len() == 3`（各带 `file` / `line` / `specifier` / `type_only`），**不铺开**成 3 条 | 表驱动（单文件 1 处 / 3 文件各 1 处 / 单文件 3 处） |
| 聚合序确定性 | 边界 | 同上 3 occurrence 用例连续两次 `check` → `occurrences` 顺序逐字段相等（扫描序稳定，禁用 HashMap 序） | 单例（两次对拍） |
| 配置豁免留痕（AC-6） | 正向 | `ignore = [{ from, to, reason }]` 命中一条未声明依赖 → 不进 `errors` / `warnings`；`suppressed[]` 一条 `{ via: config, reason, file: None, line: None }` | 单例 |
| 内联豁免留痕（AC-6） | 正向 | 源文件 import 行注 `// archi-ignore-next-line: 迁移期` → `suppressed[]` 一条 `{ via: inline, reason: "迁移期", file: Some(..), line: Some(..) }`，不进 `errors` | 单例 |
| 豁免层序（声明 > 配置 > 内联，AC-6） | 边界 | 同一对被配置 `ignore` 且内联标注且模型已声明 → 模型声明生效：零 `errors`、零 `suppressed`（声明即豁免，配置 / 内联不再留痕）；仅配置 + 内联同时命中（无声明）→ `suppressed` 只记 `via = config` 一条（层级更高者胜） | 表驱动（2 组合） |
| ① 声明未兑现 → `observations`（D13） | 正向 | 模型声明 `A -> B`、代码零 import → `observations[]` 1 条（`kind = unfulfilledDeclaration`、`message` 含「本次扫描面内未见」）；`status` 仍为 `clean`（不因观察项变 `warnings`） | 单例 |
| ① 在增量模式同样产出（D13） | 边界 | `mode = incremental` + `changed` 不含该对相关文件 → `observations` 仍产 1 条，`status` 不变 | 单例 |
| ④ path 失效按元素档（AC-4） | 异常 | `strict` 元素 `path` 指向不存在目录 → `errors[]` 1 条（`kind = pathNotFound`、`occurrences` 一条 `line = 0`）；`warn` 元素同形态 → `warnings[]`；`off` 元素同形态 → `errors[]` 空且 `model.warnings[]` / `warnings[]` 留痕（spec scenario 逐字） | 表驱动（3 档） |
| ④ `noPath` warning | 异常 | 元素无 `path` → `model.warnings[]` 1 条（`kind = noPath`）、不参与归属、`errors[]` 空 | 单例 |
| `ambiguousOwner` warning（AC-2 交汇） | 异常 | 同长多命中文件 → `warnings[]` 1 条（`kind = ambiguousOwner`）+ `unmatched_files` 含该文件 + `errors[]` 空 | 单例 |
| `modelHash` 随模型变化（AC-8） | 边界 | 模型字节不变 → 两次 `check` 的 `model_hash` 相等且以 `sha256:` 前缀；改一个 `gate` 值 → `model_hash` 变化（零缓存、每次实算） | 表驱动（2 形态） |
| 报告字段面与计数 | 正向 | `model.elements` / `model.relations` / `by_gate.{strict,warn,off}` 与构造模型逐值相等；`scan` 段逐字段等于 `scan()` 直出（同一事实不在 `drift` 二次改写） | 单例 |
| `timestamp` / `version`（D9） | 边界 | `timestamp` 匹配 RFC3339 UTC 形态（`YYYY-MM-DDTHH:MM:SSZ`、无小数、结尾 `Z`）；`version` 为常量非 0；同一进程两次 `check` 的 `timestamp` 可相等（秒级）但不回退 | 单例（形态断言） |
| 取消（异常） | 异常 | `cancel` 预置 → `Err(CheckError::Stopped)`（不产报告） | 单例 |
| 配置非法即不执行（AC-6 / AC-9） | 异常 | `<archi_dir>/config.json` 含未知键 → `Err(CheckError::Config(msg))`，msg 指明条目；目录内零报告写出 | 单例 |
| `exclude` 语法非法 → 扫描面 `Err`（AC-6 的另一形态） | 异常 | `<archi_dir>/config.json` 含 `exclude: ["src/[legacy"]` → `load_config` 的形态校验放行（本层只管空白 / 无通配符，见 `drift/config_test.rs`）→ `scan` 建 `ExcludeSet` 时 `Err` ⇒ `check` 返回 `Err(CheckError::Scan(msg))`、`msg` 含条目原文；零报告产出，且与 `Config` 面错误变体可辨（非法 glob 的直证挂 `scan/exclude_test.rs`） | 表驱动（2 形态：`src/[legacy` / `src/a[bc`） |
| 根路径边界 | 边界 | `root` 指向不存在路径 / 指向普通文件 → 模型目录必然缺席 ⇒ `status == skipped`（D3 早退规则的自然推论），零 panic、零 `Err`；`root` 含空格与非 ASCII 的合法目录 → 正常走判定 | 表驱动（3 形态） |
| 零写盘（AC-14 反向） | 边界 | `check` 前后 tempdir 全量字节快照相等（全量模式不落盘、判定纯读盘） | 单例 |
| 两模式元素对级一致（AC-7） | 边界 | 同一 tempdir 分别以 `full`（`changed = []`）与 `incremental`（`changed = 全量源文件`）跑 → `incremental` 的 `errors` / `warnings` 元素对集合 ⊆ `full` 的同集合（按 `(from, to)` 比较，不断言集合相等） | 表驱动（2 档构造） |
| 判定义务边界（D4） | 边界 | 同一元素对在 300 文件里各一处 → `errors` 仍 1 条（聚合不随规模膨胀）；`occurrences.len() == 300` | 单例（规模边界） |
| `CheckError::message`（三值） | 异常 | `Stopped` → 「检查已取消」（逐字）；`Config("x")` / `Scan("y")` → 原样透传且非空；三值互不相等 | 表驱动（3 值） |
| 判定语料 golden（组合，链路 `scan → drift::check → report`） | 正向 | `tests/corpus/<case>/` 13 个 case 逐条驱动 `check`，与 `tests/golden_test.rs` 内单点登记的期望形态对比：未声明三档 / 聚合 occurrences / 目标不可归属（丢边 + unmatched 留痕、零 warnings） / 测试文件排除 / `import type` / 内联豁免 / 配置豁免 / 解析失败 / 声明未兑现（全量 + 增量双模式）/ 结构错 / 空模型 / 无变更 / 两模式一致；新增 case 未登记即红、登记但目录缺席亦红 | 表驱动（13 case） |
| 语料重写闸（沿既有先例） | 边界 | `DESKTOP_GOLDEN_REWRITE=1` 时渲染快照且**断言仍按登记面执行**（快照不参与判定）；未置位时不写盘（字节零变化） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 模型读取（`dto::graph_view`） | **不 mock**：真实读取 tempdir 内 `<archi_dir>/models/*.toml`（D2 明令复用 `graph_view`，替身会掩盖停摆态与结构错形态） | 全部用例 |
| 扫描面（`scan` / 双解析器 / 排除 / 归属） | **不 mock**：真实 tempdir 源文件 + 真实解析器（判定链路的输入事实即被测语义，D4） | 对账 / 定级 / 聚合 / 语料 |
| 看护配置 | **不 mock**：真实 `config.json` 落盘（`{}` / 合法 / 非法三形态）；配置解析单点由 `drift/config_test.rs` 深测，本节只验「非法即不执行」 | 豁免 / 配置非法 |
| 时钟与摘要 | **不 mock**：`time::OffsetDateTime::now_utc()` 与 `sha2` 真实调用，`timestamp` 只断言形态与单调、`model_hash` 只断言前缀与变化性（不做绝对时间替身） | `timestamp` / `model_hash` |
| 取消信号 | **不 mock**：真实 `CancelFlag`（`never()` / `handle().set()`） | 取消 / 正常两态 |
| 网络 / 子进程 / 数据库 | **零依赖** | —— |

---

### packages/desktop/src-tauri/crates/core/archi/src/drift/config.rs -> packages/desktop/src-tauri/crates/core/archi/src/drift/config_test.rs

#### 待测功能

- `load_config()`: 经 `archi_config_path` 定位；文件缺失 → 空配置；未知键 / `reason` 缺失或空白 / FQN 不存在 / 非法 `exclude` → `Err`（文案指明条目）（公共函数 / API）

> 说明：`exclude` 的显式 `Err` 只覆盖**形态**两项（空白条目 / 无通配符条目，即「不是文件级 glob」）；`Glob::new` 的语法合法性**不在本层判**——非法即 `Err` 的直证挂 `scan/exclude_test.rs` 的 `ExcludeSet::build` 行（design 公共函数表两行的分工），`check` 侧的合并表现为 `Err(CheckError::Scan)`（见 `drift/mod_test.rs`），本层只做「放行 + 原文保序」。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 文件缺失即空配置（AC-9） | 边界 | 无 `<archi_dir>/config.json` → `Ok(CheckConfig::default())`（`ignore` / `exclude` 双空，第 2 层豁免与追加排除均不生效） | 单例 |
| 空文件 / 空对象 | 边界 | `""` 与 `{}` 两形态 → `Ok` 且两字段皆空（冷启动合法起点，零报错） | 表驱动（2 形态） |
| 两键齐备 | 正向 | `{ "ignore": [{ "from": "A", "to": "B", "reason": "迁移期" }], "exclude": ["src/legacy/**"] }` → 两字段逐值解析（`reason` 逐字、`exclude` 顺序保留） | 单例 |
| 单键缺席 | 边界 | 只有 `ignore` / 只有 `exclude` → 另一字段为空，`Ok` | 表驱动（2 形态） |
| 未知键拒绝（AC-9） | 异常 | 顶层多余键（`{"rules": []}` / `{"archi": {}}`）→ `Err`，文案含该键名；`deny_unknown_fields` 不静默忽略 | 表驱动（2 形态） |
| `reason` 缺失（AC-6） | 异常 | `ignore: [{ from: "A", to: "B" }]` → `Err`（文案指明条目） | 单例 |
| `reason` 空白（AC-6） | 异常 | `reason: ""` / `"   "` / `"\n\t"` 三形态 → 全 `Err`（`trim` 后为空即拒） | 表驱动（3 形态） |
| `from` / `to` 非模型 FQN（AC-6） | 异常 | `from = "A.Deleted"`（模型内不存在）/ `to = "src/**"`（glob 形态）→ `Err`，文案指明该 FQN；模型内 FQN 全量集合来自 `known` 参数 | 表驱动（2 形态 + 合法对照） |
| glob 形态 `ignore` 拒绝（AC-6） | 异常 | `from = "App.*"` / `to = "**"` → `Err`（豁免粒度封闭为元素对，禁 glob） | 表驱动（2 形态） |
| `exclude` 无通配符（AC-6） | 异常 | `["src/legacy"]` / `[""]` / `["src/"]` → 全 `Err`（目录须写 `src/legacy/**`） | 表驱动（3 形态） |
| `exclude` 语法非法条目本层放行（校验归 `ExcludeSet::build`） | 边界 | `["src/[legacy"]` / `["src/a[bc"]`（含 `[` 且非空白 → 过形态关）→ `Ok` 且 `exclude` 逐字保序（`load_config` 不引 `globset` 编译，避免第二份合法性判定；非法即 `Err` 的直证在 `scan/exclude_test.rs`）；对照：`["src/{a"]` 不含 `*` / `?` / `[` → 按「无通配符」`Err` | 表驱动（3 形态：2 放行 + 1 形态拒） |
| JSON 语法 / 顶层形态（异常） | 异常 | 非法 JSON（缺闭括号）/ 顶层数组 `[]` / 顶层字符串 `"x"` / `null` → 全 `Err` 且不 panic | 表驱动（4 形态） |
| 条目形状违例 | 异常 | `ignore` 为对象而非数组 / `exclude` 为字符串 / `ignore[]` 元素为字符串 → 全 `Err`（类型面封闭） | 表驱动（3 形态） |
| 空数组 / 空串条目 | 边界 | `{ "ignore": [], "exclude": [] }` → `Ok`；`exclude: [""]` → `Err`（无通配符，见上表） | 表驱动（2 形态） |
| 超大列表 | 边界 | 100 条合法 `ignore` + 100 条合法 `exclude` → `Ok` 且逐条保序（不截断、不去重） | 单例 |
| 特殊字符 | 边界 | `reason` 含 emoji / `\n` / `\0` / >1000 chars → `Ok` 且 `reason` 原文保留（不做长度上限）；`exclude` 条目含 emoji → `Ok`（通配符合法即收） | 表驱动（4 形态） |
| 定位单点（AC-9） | 边界 | 配置实际落 `<archi_dir>/config.json` 时被读到；落 `<root>/config.json`（工作区配置位置）或 `<archi_dir>/models/config.json` → 均**不被**读取（空配置），证「经 `archi_config_path` 唯一合法定位」 | 表驱动（3 位置） |
| 产品源码零字面量（AC-9） | 边界 | `include_str!("config.rs")` 不含 `config.json` 连续字面量（文件名只允许出现在 `foundation/src/layout/mod.rs`；既有命名隔离扫描不覆盖 `crates/**`，本用例在该缺口内自证） | 单例 |
| 确定性 | 边界 | 同一文件连续两次 `load_config` 结果逐字段相等（`exclude` 保序、`ignore` 保序） | 单例 |
| 根路径边界 | 边界 | `root` 含空格 / 非 ASCII / 极深路径 → 仍按 `archi_config_path` 定位且能读到（不 panic）；不存在 root → 视为「文件缺失」→ `Ok(空配置)` | 表驱动（3 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 文件系统 | **不 mock**：真实 tempdir 落 `<archi_dir>/config.json`（缺失 / 合法 / 非法三形态） | 全部用例 |
| `archi_config_path` | **不 mock**：真实 `foundation::layout::archi_config_path`（定位单点即被测语义；替身会掩盖「与工作区 `config_path` 混用」缺陷，AC-9 / R-12） | 定位单点用例 |
| `known` FQN 集合 | **不 mock**：`BTreeSet<String>` 直构（模型 FQN 全集由调用方注入，D2 的 `check` 承担） | FQN 校验 |
| serde_json | **不 mock**：真实解析（错误形态即被测面） | 全部异常用例 |
| 网络 / 时间 / 子进程 | **零依赖** | —— |

---

### packages/desktop/src-tauri/crates/core/archi/src/drift/report.rs -> packages/desktop/src-tauri/crates/core/archi/src/drift/report_test.rs

#### 待测功能

design 未声明该文件的公共 API 变更，本节无条目。

<!-- design.md「公共函数 / API」子表未声明本文件的公共 API 变更（`report.rs` 全为 wire 类型与 `ALL` 常量，无模块级函数；`pub use ArchiDriftReport as Report` 为类型别名）。 -->

> 说明：测试对象取自 design「类型定义」`ArchiCheckStatus` / `ArchiDriftMode` / `ArchiDriftKind` / `ArchiObservationKind` / `ArchiSuppressVia` / `ArchiDriftReport`（`Report`）/ `ArchiDriftModel` / `ArchiGateCount` / `ArchiScanReport` / `ArchiExcludedFile` / `ArchiMatchedElement` / `ArchiUnresolvedFile` / `ArchiDriftEntry` / `ArchiDriftOccurrence` / `ArchiDriftSuppressedEntry` / `ArchiDriftObservation` 十六类、D1（线面 `Archi*` 前缀与别名）、D10（`status` 五值序）、D11（结构错类型复用 `dto::ArchiDiagnostic`）与 AC-8；语料对拍挂 `drift/mod.rs` 章节，本节只覆盖词汇与线形。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ArchiCheckStatus` 五值封闭（AC-8） | 正向 | `ALL.len() == 5`；五值 `as_str()` 与 serde 线词逐值一致（`clean` / `errors` / `warnings` / `noChanges` / `skipped`，`noChanges` 为 camelCase 线形）；`ALL` 去重后长度不变（漏值 / 重复不可绕过） | 表驱动（5 值） |
| `ArchiDriftMode` 二值 | 正向 | `ALL.len() == 2`；`full` / `incremental` 两值 `as_str()` 与 serde 往返逐值一致 | 表驱动（2 值） |
| `ArchiDriftKind` 四值 | 正向 | `ALL.len() == 4`；`undeclaredDependency` / `pathNotFound` / `ambiguousOwner` / `noPath` 逐值 camelCase 线形与往返 | 表驱动（4 值） |
| `ArchiObservationKind` 二值 | 正向 | `ALL.len() == 2`；`unfulfilledDeclaration` / `unusedDeclaration` 逐值在案（后者本变更不产出，词汇位保留，D1） | 表驱动（2 值） |
| `ArchiSuppressVia` 二值 | 正向 | `config` / `inline` 两值 lowercase 线形与往返 | 表驱动（2 值） |
| 词汇外反序列化（非法枚举值） | 异常 | `"status": "clean "` / `"unknown"` / `"Clean"` / `"no_changes"` → 全 `Err`（封闭词汇严格，零宽松归一） | 表驱动（4 形态） |
| `Report` 别名同一类型 | 边界 | `Report` 与 `ArchiDriftReport` 可互相赋值（编译期同一类型）+ 同值序列化字节相等 | 单例 |
| 完整报告 JSON 往返 | 正向 | 构造 14 字段全填充的 `Report`（含 1 条 error / 1 条 warning / 1 条 suppressed / 1 条 observation / 结构诊断 1 条）→ `to_string` → `from_str` 逐字段相等（嵌套 `occurrences` 亦逐项） | 单例 |
| 字段线形（camelCase） | 边界 | 序列化文本含 `modelHash` / `scannedFiles` / `changedFiles` / `structuralErrors` / `byGate` / `typeOnly` / `unmatchedFiles`；不含 `model_hash` / `structural_errors`（线面口径单点） | 单例 |
| Option 字段 `null` | 边界 | `change` / `base` 为 `None` → 文本含 `"change":null` 且往返回 `None`；`ArchiDriftEntry.from` / `to` / `gate` 为 `None`、`ArchiDriftSuppressedEntry.file` / `line` 双 `None`、`ArchiUnresolvedFile.specifier` 为 `None` 同样往返成立 | 表驱动（6 字段） |
| 空报告形态 | 边界 | `errors` / `warnings` / `suppressed` / `observations` / `occurrences` / `excluded` / `matched` / `unresolved` 全 `[]` → `status = clean` 的报告可序列化；两次 `to_vec_pretty` 字节相等（无 HashMap 序漂移，确定性） | 单例（两次对拍） |
| `ArchiDiagnostic` 复用（D11） | 边界 | `Report.model.structural_errors` 元素类型为 `dto::ArchiDiagnostic`（同值构造可赋值）+ 其 `kind` 走 `DiagnosticKind` 词汇（`ALL.len() == 9` 不变，模型面零新增词） | 单例 |
| 位置未知口径 | 边界 | `ArchiDriftOccurrence.line = 0` 与 `ArchiDiagnostic.line = 0` 同口径可序列化；`line = u32::MAX` 往返不溢出 | 表驱动（2 边界） |
| 计数边界 | 边界 | `ArchiGateCount { strict: 0, warn: 0, off: 0 }` 与 `{u32::MAX, …}` 两形态往返相等（无饱和 / 无符号翻转） | 表驱动（2 边界） |
| 超大 / 超长面 | 边界 | 1000 条 `errors`（各带 100 occurrence）→ 序列化与反序列化成功且长度不变；`message` >1000 chars / 含 emoji 与 `\0`（JSON 转义）→ 往返逐字相等 | 表驱动（2 形态） |
| 契约面（不携带 `passed`） | 边界 | 序列化文本不含 `"passed"`（结论由门禁侧派生，不双写，D10） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| serde / serde_json | **不 mock**：真实序列化与反序列化（线形即被测对象） | 全部用例 |
| 文件系统 / IO | **零依赖**：本模块纯数据（零 IO、零判定），用例全内存构造 | 全部用例 |
| 时钟 / 随机 | **零依赖**：`timestamp` 以字面量构造（`check` 侧才取真实时钟） | 全部用例 |
| 网络 / 子进程 / 数据库 | **零依赖** | —— |

---

### packages/desktop/src-tauri/crates/infra/archi/src/report.rs -> packages/desktop/src-tauri/crates/infra/archi/src/report_test.rs

#### 待测功能

- `write_report()`: 目录经 `change_reports(root, name, ReportSlot::Archi)` 派生 + `create_dir_all` + 文件名（`<YYYYMMDDTHHMMSSZ>.json`）+ `serde_json::to_vec_pretty`；返回文件路径（公共函数 / API）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 目录派生单点（AC-14） | 正向 | `write_report(root, "demo", &report)` → 落点逐字等于 `change_reports(root, "demo", ReportSlot::Archi)`（`<changes_root>/demo/reports/archi/`），零行内路径字面量 | 单例 |
| 文件名形态（D9） | 正向 | `report.timestamp = "2026-10-20T08:31:12Z"` → 文件名为 `20261020T083112Z.json`（过滤出数字 + `T` + `Z`，无冒号） | 表驱动（3 个时间戳） |
| 文件名跨平台合法性 | 边界 | 文件名不含 `:` / `/` / `\` / `*` / `?` / `"` / `<` / `>` / `\|`；长度 < 255；解析出的 `YYYYMMDDTHHMMSSZ` 前 8 位为日期、末位 `Z` | 表驱动（3 时间戳） |
| 多级目录创建 | 边界 | tempdir 下 `<changes_root>/demo` 不存在（连 `reports/archi` 两级缺失）→ 写盘成功且目录建成 | 单例 |
| 内容往返 | 正向 | 写入后 `fs::read` → `serde_json::from_slice::<ArchiDriftReport>` 与入参报告逐字段相等（`to_vec_pretty` 一次整写，无拼接） | 单例 |
| 返回值可用 | 正向 | 返回路径 `Path::exists()` 为真且 `== change_reports(..).join(<文件名>)` | 单例 |
| 同时间戳覆盖 | 边界 | 同报告写两次 → 单文件、内容为后写者（同一次门禁一份，不堆积同秒副本） | 单例 |
| 不同时间戳堆积 | 边界 | 两个不同 `timestamp` → 两文件并存（不删旧报告，R-10 口径） | 表驱动（2 写） |
| 槽位隔离（AC-14） | 边界 | 写盘后 `reports/test` 与 `reports/baseline` 目录**不存在**（零创建）；既有两目录预置字节快照前后相等 | 单例 |
| 目录不可写降级面（AC-14 前置） | 异常 | 在 `<change>/reports/archi` 位置预置一个**普通文件**（占位冒充目录）→ `write_report` 返回 `Err`，且零半写文件残留（失败前置到 `create_dir_all`） | 单例 |
| 根 / name 边界（异常） | 异常 | `name = ""` / 含 `..` 的 name / 含空格与非 ASCII 的 name → 路径派生结果可判且不 panic（成功或 `Err` 皆可，断言仅「不崩 + 不写出报错路径外文件」） | 表驱动（3 形态） |
| 大报告 | 边界 | 1000 条 `errors`（各 10 occurrence）→ 写盘成功、读回长度不变（无截断） | 单例 |
| 空报告 | 边界 | 全空数组 + `status = clean` → 写盘成功且读回逐字段相等 | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 文件系统 | **不 mock**：真实 `tempfile::TempDir`（design 依赖表已声明 `[dev-dependencies] tempfile`，`infra/checks` 同式）；写盘失败以「同名普通文件占位」注入，不改权限、不依赖平台 | 全部用例 |
| `foundation::layout::change_reports` | **不 mock**：真实派生（目录单点即被测语义，禁替身） | 全部用例 |
| serde_json | **不 mock**：真实序列化 / 反序列化 | 内容往返 / 大报告 |
| 时钟 | **零依赖**：`timestamp` 由报告字段给出（替身只需构造字符串），本模块不取系统时间 | 全部用例 |
| 网络 / 子进程 / 数据库 | **零依赖** | —— |

---

### packages/desktop/src-tauri/crates/infra/archi/src/runner.rs -> packages/desktop/src-tauri/crates/infra/archi/src/runner_test.rs

#### 待测功能

- `LocalArchiCheck::new()`: 零参构造（另附 `impl Default`）（公共函数 / API）
- `LocalArchiCheck::run()`（`impl ArchiCheckRunner`）: `check`（增量、`change = Some(name)`）→ `write_report` → `{ passed, diagnostics, fault, report_dir, report_ref }`；`Stopped` → `fault = Some(RunFault::Stopped)`；写盘失败 → `report_ref = None` 且结论保留（公共函数 / API）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `run` clean 全字段 | 正向 | tempdir 有模型零违例 → `passed == true`、`diagnostics` 为有界摘要（非空且不含「未检查」）、`fault == None`、`report_dir == change_reports(root, name, ReportSlot::Archi)`、`report_ref = Some(文件路径)` 且该文件存在 | 单例 |
| `run` warnings 档（AC-12 三态分派） | 正向 | `gate = warn` 元素引入未声明依赖 → `passed == true`、`diagnostics` 含「N 条提示」量词且不含固定处置提示 | 单例 |
| `run` errors 档（AC-12 三态分派） | 正向 | `gate = strict` 违例 → `passed == false`、`diagnostics` 含元素对 / 档位 / 出现次数 / 至多两处 `file:line` / 固定处置提示（「回到 dev-design 补声明」） | 单例 |
| 结构错拦截（AC-12） | 异常 | 模型含 `dependsOnAncestor` → `passed == false`、`diagnostics` 含结构错的 file / line / 元素名、零 panic | 单例 |
| `skipped` 逐字文案（AC-13） | 边界 | 无 `<archi_dir>/models` → `passed == true`、`diagnostics` **逐字**为「未检查（无模型）」（不含路径、不含「通过」） | 单例 |
| `noChanges` 逐字文案（AC-13） | 边界 | `changed = []`（增量零变更）→ `passed == true`、`diagnostics` 逐字为「未检查（无可判文件）」；报告 `status` 原样为 `noChanges` | 单例 |
| 报告内容与判定同源（AC-10 前置） | 正向 | 落盘文件反序列化后与同输入 `archi::drift::check` 直出报告逐字段相等，仅 `timestamp` 允许不同；`report.change == Some(name)`、`report.mode == incremental`、`report.base == Some("HEAD")` | 单例 |
| 反馈文本有界（AC-12） | 边界 | 报告 12 条 `errors`（其中一条聚合 40 处 occurrence）→ `diagnostics.chars().count() <= MAX_REPORT_CHARS`（复用 `orchestration::verdict::MAX_REPORT_CHARS`，零新常量），每条至多两处 `file:line`，全量明细只可从报告文件回查 | 单例 |
| 超长单条 message | 边界 | 一条 `errors` 的 `message` > 1000 chars → 文本仍截断到上限内且不 panic（不出现 UTF-8 边界截断崩） | 单例 |
| 写盘失败降级（AC-14） | 异常 | 报告目录位置被普通文件占位 → `run` 返回 `Ok`（不 `Err`）、`passed` 与 `diagnostics` 与正常一致、`report_ref == None`、`report_dir` 照常为预期目录 | 单例 |
| 取消（AC-13 / D18） | 异常 | `cancel` 在扫描前预置 → `Ok(outcome)` 且 `fault == Some(RunFault::Stopped)`（取消**不**以 `Err` 表达），`passed` / `report_ref` 形态可判 | 单例 |
| 生产路径 `fault` 恒 `None`（AC-14 零进程） | 边界 | clean / warnings / errors / skipped 四态（非取消）→ `fault` 全 `None`；`run` 不创建进程（`include_str!("runner.rs")` 不含 `std::process` / `Command::new` / `tokio::process`） | 表驱动（4 态 + 源码自证） |
| 零第二套判定（AC-10） | 边界 | `include_str!("runner.rs")` 不含归属 / 定级 / status 推导关键词的实现面（`owner_of` / `is_empty()` 之外的状态推导），`status` 只从报告读取 | 单例（源码自证） |
| 变更集注入（AC-15） | 边界 | 传入 `changed = [文件A]` → 报告 `scan.changed_files == 1` 且只判 A；传入 `changed = []` → `noChanges`（不降级为假通过，见上行） | 表驱动（2 形态） |
| 构造面 | 边界 | `LocalArchiCheck::new()` 与 `LocalArchiCheck::default()` 行为等价（同输入同 outcome）；零参构造不触盘、不 panic | 单例 |
| 根边界 | 边界 | `root` 指向不存在路径 → `Ok`（`skipped`，模型目录缺席）且不 panic；`name` 空串 → 报告目录派生结果可判（与 `report_test` 的 name 边界一致） | 表驱动（2 形态） |
| 槽位隔离（AC-14） | 边界 | 一次 `run` 后 `reports/test` / `reports/baseline` 零创建、零字节变化（与 `report_test` 的写盘面单点交叉闭合） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 判定（`archi::drift::check`） | **不 mock**：真实调用（同源即被测语义，AC-10；替身会让「第二套判定」缺陷不可见） | 全部用例 |
| 写盘（`write_report`） | **不 mock**：真实写盘；失败面以「目录位置普通文件占位」注入（进程边界真实、零桩） | 写盘失败降级 |
| 文件系统 / 模型 / 源文件 | **不 mock**：真实 `tempfile::TempDir` 落模型与源码 | 全部用例 |
| 取消信号 | **不 mock**：真实 `foundation::cancel::CancelFlag` | 取消用例 |
| 时钟 | **零依赖于断言**：`timestamp` 只断言形态，不做替身 | 全链路用例 |
| 网络 / 子进程 / vcs / store | **零依赖**：本 crate 零 spawn / 零 vcs / 零 store（AC-17），测试亦不注入 | —— |

---

### packages/desktop/src-tauri/crates/core/foundation/src/layout/mod.rs -> packages/desktop/src-tauri/crates/core/foundation/src/layout/mod_test.rs

#### 待测功能

- `archi_config_path()`: `<archi_dir>/config.json`（从 `archi_dir` 派生、复用 `CONFIG_FILE_NAME`）；纯推导零 IO（公共函数 / API）
- `report_dir_name()`: 二分臂 → 三分臂（`test` / `baseline` / `archi`）；私有，消费面经 `change_reports`（公共函数 / API）

> 说明：`ReportSlot` 三值与 `ARCHI_REPORTS_DIR_NAME` 常量取自 design「类型定义」/「修改文件」，用例按 AC-9（配置路径单点 / 与工作区配置互不遮蔽）与 AC-14（槽位三分臂隔离）组织；既有「命名隔离双禁令扫描」用例作回归锚（零改动，`crates/**` 覆盖缺口见「不可测试项」）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `archi_config_path` 落点（AC-9） | 正向 | `archi_config_path(root) == archi_dir(root).join(CONFIG_FILE_NAME)`（逐字相等，非「包含」）；末段为 `config.json`、父段为架构域段名 | 单例 |
| 与工作区配置互不遮蔽（AC-9） | 边界 | `archi_config_path(root) != config_path(root)`，且两者父目录不同（一在 `<archi_dir>`、一在工作区根）；对同一 root 两条路径互不为前缀 | 单例 |
| 纯推导零 IO | 边界 | root 下零目录存在 → 仍返回路径（`exists()` 为假但值可判，零 IO 零 panic） | 单例 |
| 三路径同根 | 边界 | `archi_config_path` / `archi_models_dir` / `archi_decisions_dir` 三者父链同源于 `archi_dir(root)`（同根三子项） | 单例 |
| `ReportSlot::Archi` 派生（AC-14） | 正向 | `change_reports(root, "demo", ReportSlot::Archi)` → `<changes_root>/demo/reports/archi`；`change_reports_in(dir, ReportSlot::Archi)` → `<dir>/reports/archi`（两入口同值） | 表驱动（2 入口） |
| 三槽位互异 | 正向 | `Test` / `Baseline` / `Archi` 三值派生目录名分别为 `test` / `baseline` / `archi`，两两不等（`report_dir_name` 三分臂穷尽，无兜底臂） | 表驱动（3 值两两对照） |
| 槽位隔离（AC-14） | 边界 | 三槽位派生路径互不为前缀（`reports/archi` 不被 `reports/test` 覆盖）；`change_reports` 与 `change_test_reports` / `change_baseline_reports` 对同一 change 三值互异 | 单例 |
| 既有两槽位零回归 | 回归 | `change_test_reports` / `change_baseline_reports` 与 `change_reports(root, name, Test/Baseline)` 逐字相等（新增 `Archi` 臂不改既有两臂） | 回归（既有断言不改） |
| 常量值 | 边界 | `ARCHI_REPORTS_DIR_NAME == "archi"`；与 `report_dir_name(ReportSlot::Archi)` 同源（单一常量） | 单例 |
| root / change 名边界 | 边界 | root 含空格 / 非 ASCII / 尾分隔符 / 相对路径 → 派生路径照常可判（纯拼接，不归一、不 panic）；change 名 `""` / `"."` / `".."` / 超长（>255）→ 派生出现象可判（不 panic，不触盘） | 表驱动（4 + 4 形态） |
| 非法 root 输入（异常） | 异常 | root 为 `Path::new("")` / 仅 `..` 段 / 含 NUL 字符 / 含 emoji 与 >1000 chars 段 → `archi_config_path` 与三槽位派生均返回可判 `PathBuf`，零 panic、零 IO、零触盘（路径拼贴在内存层完成，不落实体） | 表驱动（4 形态） |
| 命名隔离扫描零回归（AC-9） | 回归 | 既有双禁令扫描（`openspec` / `config.json` 字面量，`layout/mod.rs` 唯一例外）仍全绿；新增 `ARCHI_REPORTS_DIR_NAME` 与 `archi_config_path` 未引入新字面量触点 | 回归 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 路径推导 | **不 mock**：纯函数直调（`PathBuf` 比较），零 IO 零 env | 全部用例 |
| 文件系统 | **零依赖**：不建目录、不读盘（既有 `TempDir` 装置仅服务于既有用例的 §「命名隔离扫描」，本节新增用例不用） | 全部用例 |
| 工作区配置 / 时钟 / 网络 | **零依赖** | —— |

---

### packages/desktop/src-tauri/crates/core/workflow/src/state.rs -> packages/desktop/src-tauri/crates/core/workflow/src/state_test.rs

#### 待测功能

- `StepNode::as_str()` / `StepNode::group()` / `StepNode::role()`: 各补 `ArchiCheck` 臂（`"archi_check"` / `StepGroup::Exec` / `None`）（公共函数 / API）
- `RunStepKind::as_str()`: 补 `ArchiCheck => "archi_check"`（公共函数 / API）
- `StepKind::as_str()`: 审计包络八臂 → 九臂（公共函数 / API）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `StepNode::as_str` 22 值全词汇（AC-16） | 正向 | 既有 21 值断言逐字不变 + `StepNode::ArchiCheck.as_str() == "archi_check"`；serde wire 与 `as_str` 一字不差；22 值词面互异（无重复） | 表驱动（22 值） |
| 尾部追加顺序（R-8 bincode 纪律） | 边界 | `StepNode::ArchiCheck as usize == 21`（末位，既有 0..=20 下标零位移）；`RunStepKind::ArchiCheck as usize == 21`；审计 `StepKind::ArchiCheck as usize == 8`（八 → 九臂末位） | 表驱动（三枚举各一） |
| `RunStepKind::as_str` 22 值 | 正向 | 既有 21 值 + `archi_check` 逐值 snake_case；封闭集 `len == 22`、词面互异 | 表驱动（22 值） |
| `StepNode` ↔ `RunStepKind` 1:1 | 正向 | 既有 21 对拍行 + `(ArchiCheck, ArchiCheck, "archi_check")` 补一行；22 对全部同词面 | 表驱动（22 对） |
| `group()` 归 `Exec` | 正向 | `StepNode::ArchiCheck.group() == StepGroup::Exec`；`groupOf` 四分类总和仍为 22（Git 六 / Exec 十一 / WorkerAgent 五边界不变） | 表驱动（分组计数） |
| `role()` 归 `None` | 边界 | `StepNode::ArchiCheck.role().is_none()`（工具步无角色）；五类 WorkerAgent 步的 `Some` 面零变化 | 表驱动（2 形态） |
| `StepKind` 九臂穷尽 | 正向 | `StepKind::ArchiCheck.as_str() == "archi_check"`；八既有臂逐字不变；九臂词面互异 | 表驱动（9 值） |
| 非法词反序列化（异常） | 异常 | `"ArchiCheck"` / `"archiCheck"` / `"archi-check"` / `""` 于 `StepNode` / `RunStepKind` 上 → 全 `Err`（snake_case 封闭）；`ChangeStepKind` 侧 `"archi_check"` 非法（camel 口径，见 `orchestration/state_test`） | 表驱动（4 形态 × 2 枚举） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| serde / serde_json | **不 mock**：真实序列化与反序列化（词面即被测对象） | 全部用例 |
| 进程边界（DB / FS / 网络 / 时钟） | **零依赖**：中性类型纯内存构造（既有「无 mock」纪律不变） | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table_test.rs

#### 待测功能

design 未声明该文件的公共 API 变更，本节无条目。

<!-- design.md「公共函数 / API」子表未声明本文件的公共 API 变更（`IMPLEMENT_STEPS` 与 `ARCHI_FEEDBACK_LIMIT` 为模块内静态表 / `pub(crate)` 常量，非模块级 pub 函数；消费面经 `phase_table()` 与 `phase_next` 派生）。 -->

> 说明：测试对象取自 design「修改文件」`phase_table.rs` 单元格（`ARCHI_FEEDBACK_LIMIT: u32 = 3`、`IMPLEMENT_STEPS` 7 步插入位）、AC-11 与 `desktop-workflow-write-face` MODIFIED delta；既有断言（相位表全表序、`on_fail` 逐项、回溯白名单）作回归锚。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| implement 7 步序（AC-11） | 正向 | `phase_table("implement")` 节点序列逐字为 `PhaseStart → Executor → StaticCheck → ArchiCheck → Evaluator → VerdictGate → PhaseLog`（恰 7 步，`ArchiCheck` 位于 static-check 与 evaluator 之间） | 单例（全序断言） |
| `ArchiCheck` 的 `on_fail`（AC-11） | 正向 | 该步 `on_fail == Some(OnFail::FeedbackLoop { limit: 3 })`；`ARCHI_FEEDBACK_LIMIT == 3`（`pub(crate)` 常量与表内值同源） | 单例 |
| 其余相位零 `ArchiCheck`（AC-11） | 异常 | 遍历全部 requirement 相位（initialize / proposal / dev-design / test-design / test-gen / test-execution / code-review / acceptance / archive 与 implement 之外的 9 个）→ 无一步 `node == StepNode::ArchiCheck` | 表驱动（9 相位） |
| test-only 表零 `ArchiCheck`（AC-11） | 异常 | test-only 工作流全相位 → 零 `ArchiCheck` 且零 `ArchiCheckRunner` 可达（表层面） | 表驱动（test-only 全相位） |
| 无运行时开关（AC-11） | 边界 | 表结构无 `cfg!` / 环境变量 / 相位名判断分支：`IMPLEMENT_STEPS` 为 `static` 常量、步数不随入参变化（同入参两次调用序列相等） | 单例（两次对拍） |
| 步序索引稳定（R-8） | 边界 | 插入后 `StaticCheck` 的步内序位不大于 `ArchiCheck`、`Evaluator` 不小于之（相对序稳定）；既有 9 相位步序与既有断言逐字不变 | 单例 |
| 既有 `on_fail` 面零回归 | 回归 | `StaticCheck` / `VerdictGate` 仍为 `FeedbackLoop { limit: FEEDBACK_LOOP_LIMIT }`（5）；`PhaseLog` / `Executor` 仍 `None`；回溯白名单 `allowed_backtrack_phases` 逐字不变 | 回归（既有断言） |
| 相位表头与 prompt 引用零回归 | 回归 | `ArchiCheck` 的 `prompt_ref == None`（工具步无 prompt）；16 份角色 md 的 `include_str!` 引用集不变（prompt 面零改动，AC-12） | 回归 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 相位表 | **不 mock**：`phase_table()` 真实调用（静态 const 表即被测对象） | 全部用例 |
| prompt md / 文件系统 | **不 mock 且零读盘**：`prompt_ref` 只比较枚举 / 字符串常量（既有 `include_str!` 编译期装配，测试不重读盘） | prompt 引用回归 |
| 时钟 / 网络 / DB / 子进程 | **零依赖** | —— |

---

### packages/desktop/src-tauri/crates/core/workflow/tests/corpus_golden_test.rs（全词汇语料构造 + golden 对拍） -> packages/desktop/src-tauri/crates/core/workflow/tests/corpus_golden_test.rs

#### 待测功能

design 未声明该文件的公共 API 变更，本节无条目。

<!-- design.md「公共函数 / API」子表未声明本文件的公共 API 变更（本文件是 workflow crate 的集成测试目标：语料种子构造器 + golden 对拍 + 全词汇断言，无导出函数；被测语义是语料维度与快照本身）。 -->

> 说明：测试对象取自 `desktop-corpus-regression` MODIFIED delta「run 运行史语料维度」的 Module Contract 行（「全词汇样本补 `archi_check` 节点（implement 相位，feedback 边样本可复用既有边）；golden 显式重写留痕」）与 proposal「测试文件」表 `crates/core/workflow/tests/corpus_golden_test.rs` 行（「全词汇样本补 `archi_check` 节点 / detail 投影 / 快照」「扩展（显式重写）」），并按 AC-16 的「语料 golden 覆盖样本在案」收口。既有装置事实：全词汇断言常驻于 `corpusrun全史语料_golden对拍`（`FULL_WORDS` 十一值 + `nodes_seen == 16`），archive 9 步词样本由 `corpus-archive-prefix` golden 承载，initialize 基线步 `clean` 由 `corpus-initialize` golden 承载；golden 对拍与显式重写由 `check_or_rewrite` + `DESKTOP_GOLDEN_REWRITE` 开关承载，语料集合一致性由 `语料完整性_golden目录与语料集合一致` 守卫。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 22 值全词汇在案（AC-16） | 正向 | 全词汇断言表由十一值扩为十二值（补 `archi_check`）；`corpus-run-history` 与 `corpus-archive-prefix` / `corpus-initialize` 三份投影合起来，`runs[].nodes[].step` 的去重词集恰为 22 类（implement 十二词 + archive 九词 + initialize 基线 `clean` 一词），逐词断言「在案」且无越集词 | 表驱动（22 值逐词 + 越集断言） |
| `archi_check` 节点落位（AC-16） | 正向 | `build_run_history` 的 run-1 增一条 `archi_check` 节点：`step == "archi_check"`、`instanceNo == 0`、状态 `passed`、`sessionId == null`（工具步无会话）、`outputRef == null`、`detail` 非空；同 attempt 内位于 `static_check` 之后、`evaluator` 之前（与相位表 7 步序一致） | 单例（落位 + 字段面） |
| 节点计数随动（边界） | 边界 | `nodes_seen` 由 16 → 17（run-1 十二条 → 十三条，run-2 一条与 run-3 三条不变）；`runs.len()` 仍 3、`runs[1]` / `runs[2]` 的节点集逐字不变（不因新增样本改道别的 run） | 表驱动（3 run 逐一计数） |
| feedback 重跑面零扰动（AC-16） | 正向 | 既有 feedback 边端点逐字不变（`implement:1:static_check:0` → `implement:1:executor:1`）；`executor` 的 `instanceNo` 序列仍为 `[0, 1]`（新增 `archi_check` 节点不插入重跑链、不改实例号）；`outputRef` 样本仍为 `exec-1727000019000` | 单例 |
| 尾部追加词面（AC-16 / R-8） | 边界 | 落库后回读的 `archi_check` 词面为 snake_case 字面量 `archi_check`（非 `archiCheck`）；其库内枚举下标为末位 21（既有 0..=20 行解码语义逐字不变），golden 中该词按文本钉死 | 单例 |
| `corpus-run-history` golden 显式重写（AC-16） | 边界 | 未重写轮：`check_or_rewrite` 报差异并提示以 `DESKTOP_GOLDEN_REWRITE=1` 重写并 review diff（不静默通过）；重写轮后再跑复核轮：`corpus-run-history.json` 与投影字节相等；差异范围限于 `runs[0].nodes` 内新增一条节点（其余 golden 文件字节零变化） | 表驱动（重写轮 / 复核轮两轮） |
| golden 缺失或非法（异常） | 异常 | 重写开关未置位且 `corpus-run-history.json` 缺失（或非合法 JSON）→ 用例以明确文案失败（「golden … 缺失（先以 DESKTOP_GOLDEN_REWRITE=1 生成）」），不静默新建 golden、不以空期望放过 | 表驱动（缺失 / 非法 JSON 两形态） |
| 重写后键集守卫（AC-16） | 边界 | `corpusgolden重写后_diff范围键集断言` 对 `corpus-run-history` 仍绿：`id` / `activePhase` / `worktree` / `title` / `runs` 键恒在场、`steps` 键仍退役（不因新增节点复活）、`retry` 边仍投影缺席；重写模式该断言显式跳过（延至复核轮） | 单例 |
| 语料集合一致性（AC-16） | 边界 | `语料完整性_golden目录与语料集合一致`：`CORPORA` 九项 + `README.md` 与 `tests/golden/` 实际文件集逐名相等——本变更零新增语料目录、零遗留 golden；重写模式下该断言显式跳过 | 单例 |
| 语料种子经操作面（边界） | 边界 | 新增 `archi_check` 节点仍经 `run_finish`（`finish_command` 整包）落库，MUST NOT 裸表插桩；`tests/fixtures/README.md` 覆盖面矩阵行集不变（`语料完整性_fixtures说明文档在位` 保绿） | 单例 |
| 既有八份 golden 零回归 | 回归 | `corpus-multi-attempt` / `corpus-backtrack-stale` / `corpus-slots-null` / `corpus-list-mixed` / `corpus-worktree` / `corpus-run-interrupted` / `corpus-archive-prefix` / `corpus-initialize` 八份 golden 与既有断言（含「文档形态零发现反例」「半包形态整包拒绝」）逐字不变 | 回归（既有用例） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| db / store | **不 mock**：真实 tempfile workspace 库 + store change / run 域操作面（既有 `Corpus` 装置；种子一律经操作面，禁裸表插桩） | 全部用例 |
| 磁盘产物树 | **不 mock**：真实 tempdir 目录树与真实 `fs::write`（语料装置既有能力） | 全部用例 |
| golden 文件 | **不 mock**：真实读盘对拍 / 真实覆写（`check_or_rewrite` 单点；`tests/golden/` 为仓内文件） | 重写 / 复核 / 缺失用例 |
| `DESKTOP_GOLDEN_REWRITE`（运行环境） | **不 mock**：真实环境变量，以「重写轮 / 复核轮」两次进程执行表达；单轮进程内不改写环境（避免并发污染），开关判据读真值 | 重写 / 复核用例 |
| 时钟 / 网络 / 子进程 | **零依赖**：时间戳为语料定值族（`T0` / `t(n)` / `STEP_T_BASE`），零 wall-clock | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/state.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/state_test.rs

#### 待测功能

- `ChangeStepKind::as_str()`: 补 `ArchiCheck => "archi_check"`（节点身份串口径）（公共函数 / API）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 22 值 camel 词汇（AC-16） | 正向 | 既有 21 值断言逐字不变 + `serde_json::to_value(ChangeStepKind::ArchiCheck) == json!("archiCheck")`；`as_str()` 同源为 `"archi_check"`；22 值词面互异、封闭集 `len == 22` | 表驱动（22 值） |
| 尾部追加顺序（R-8） | 边界 | `ChangeStepKind::ArchiCheck as usize == 21`（末位；既有 0..=20 的 bincode 下标零位移） | 单例 |
| 与 `RunStepKind` 词面 1:1（AC-16） | 正向 | `ChangeStepKind::ArchiCheck.as_str() == RunStepKind::ArchiCheck.as_str()`（`"archi_check"`，与 workflow 侧同词面） | 单例 |
| 非法词反序列化（异常） | 异常 | `"archi_check"` / `"Archicheck"` / `""` → 全 `Err`（camel 封闭）；`"archiCheck"` 必通 | 表驱动（4 形态） |
| 分组中立性 | 边界 | `ChangeStepKind` 无分组字段（分组在前端 `groupOf` 与 workflow `StepNode::group`），本节不新增分组断言——`clean` / `cleanup` 既有同形词面断言零变化 | 回归 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| serde / serde_json / specta | **不 mock**：内存构造与出线断言（既有纪律） | 全部用例 |
| 进程边界 | **零依赖** | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/port.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/port_test.rs

#### 待测功能

- `ArchiCheckRunner::run()`: 门禁 runner port；`changed` 由 `steps.rs` 经 `ArchiveVcsPort::change_edit_set(root, "HEAD")` 注入；槽位恒 `ReportSlot::Archi`（不设形参）；取消以 `fault = Some(RunFault::Stopped)` 正常返回（公共函数 / API）

> 说明：`ArchiCheckOutcome` 五字段面与 `ToolCommand::ArchiCheck { change_id }` / `ToolStepOutput::ArchiCheck` 尾部追加取自 design「类型定义」与 AC-15 / AC-16；port 契约用例（对象安全 / 入参透传 / 变体匹配）与 `steps_test` / `walker_test` 的行为用例互为闭合。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ArchiCheckOutcome` 字段面 | 正向 | 五字段可构造并逐字段可读（`passed` / `diagnostics` / `fault` / `report_dir` / `report_ref`）；`derive Debug + Clone + PartialEq + Eq` 可用（克隆后相等、可作断言值） | 单例 |
| 零 `Serialize` | 边界 | `ArchiCheckOutcome` 不进 IPC：编译面以「无 serde 派生」为准（与既有三 outcome 同型）——以 `assert_send_sync::<ArchiCheckOutcome>()` 表达其仅在进程内传递 | 单例 |
| `Option` 两态 | 边界 | `fault = None` 与 `Some(RunFault::Stopped)`、`report_ref = None` 与 `Some("…")` 四组合逐字段可辨（`Eq` 区分） | 表驱动（4 组合） |
| `ToolCommand::ArchiCheck` 尾部追加 | 边界 | 变体携带 `change_id`；`ToolCommand` 变体序末位（与既有 `ArchiveSeal` 之后）；模式匹配穷尽在 `steps.rs`（编译面） | 单例 |
| `ToolStepOutput::ArchiCheck` 匹配 | 正向 | `ToolStepOutput::ArchiCheck(outcome)` 与 `TryFrom` 形态：`ArchiCheckOutcome::try_from(ToolStepOutput::ArchiCheck(..))` 成功且逐字段相等；与 `StaticCheck` / `MutationTest` 变体交叉匹配 → `Err` | 表驱动（4 变体） |
| port 对象安全 | 边界 | `Arc<dyn ArchiCheckRunner>` 可构造并被 `LocalToolSteps` 接收（`dyn` 兼容，方法签名不引泛型） | 单例 |
| 入参透传（四参） | 正向 | 记录型假 runner 断言 `run(root, name, changed, cancel)` 四参逐值到达（`changed` 保序、含空集与单元素两形态）；槽位不在签名内（无 `slot` 形参，设计裁决） | 表驱动（2 形态） |
| 取消透传 | 边界 | `cancel` 置位后假 runner 观测到 `is_set() == true`（信号真件透传，不做决策） | 单例 |
| 错型转换恒 `Err`（异常） | 异常 | `ArchiCheckOutcome::try_from` 对全部非 `ArchiCheck` 变体（`PhaseNext` / `PhaseStart` / `PhaseLog` / `Backtrack` / `DecisionLog` / `StaticCheck` / `TestExecution` / `MutationTest`）→ 恒 `Err` 且不 panic；错串可判（不静默取默认值、不把别的 outcome 当 archi outcome 消费） | 表驱动（8 错型变体） |
| 既有变体零回归 | 回归 | `StaticCheck` / `TestExecution` / `MutationTest` 与 `PhaseNext` 等变体的匹配与 `TryFrom` 用例逐字不变（尾部追加不扰动既有臂） | 回归 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `ArchiCheckRunner` 实现 | **假件**：测试内记录型空实现（仅捕获入参并返回固定 outcome）——port 契约用例的被测对象就是「接口形状」，不涉判定 | 入参透传 / 取消透传 |
| 其余四 runner / vcs / store | **不 mock 且不注入**：本节不构造 `LocalToolSteps`（装配与分派由 `steps_test` 承载） | —— |
| `CancelFlag` | **不 mock**：真实 `never()` / `handle().set()` | 取消透传 |
| 网络 / 时钟 / 子进程 | **零依赖** | —— |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/steps.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/steps_test.rs

#### 待测功能

- `LocalToolSteps::new()`: 第四 runner 插在 mutation 之后（组合根形参序即声明序）（公共函数 / API）

> 说明：`ToolCommand::ArchiCheck` 分派与审计行取自 design「修改文件」`steps.rs` 单元格（name 解析 → `change_edit_set` → runner → `StepKind::ArchiCheck` 审计行）与 D16 / AC-15；`LocalToolSteps::new` 的第 5 参（`archi`）位置由签名固定，用例以「ArchiCheck 命令到达 archi runner、其余命令零到达」观测形参序。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `new` 第八参装配（形状） | 正向 | 以记录型假 archi runner + 真实 vcs / store 构造 → `ToolCommand::ArchiCheck` 抵达假 archi runner 1 次，抵达其余三 runner 0 次（形参序错位即红） | 单例 |
| id → 记录 → name 解析 | 正向 | 已建档 change（id ≠ name）→ 假 runner 收到的 `name` 为**记录 name**（非 id），`root` 为请求 root | 单例 |
| 未建档（异常） | 异常 | `change_id` 不存在于 store → `Err`（文案指明未建档），假 runner 零调用（与既有三 runner 同式） | 单例 |
| 变更集注入（AC-15） | 正向 | 记录型 vcs 捕获 `change_edit_set(root, "HEAD")` 一次：`base == "HEAD"`、`root == 请求 root`；返回清单按序交给 runner 的 `changed` | 表驱动（空集 / 单元素 / 三元素） |
| 取数失败即步失败（AC-15） | 异常 | 假 vcs 返回 `Err("git 不可用")` → `execute` 返回 `Err`（**不**降级空清单），假 runner 零调用（空清单会静默变 `noChanges` = 假通过） | 单例 |
| 产出类型漂移（异常） | 异常 | 假 runner 错误返回 `ToolStepOutput::StaticCheck(..)` → `Err`（`TryFrom` 不匹配按错误上抛，不静默包装） | 单例 |
| 审计行成功面（D16） | 正向 | 成功路径落一条 `StepKind::ArchiCheck` 记录：summary 以 `archi_check passed=true` 起（**不含** `status=`）、`reference == outcome.report_dir`、`change_id` 归键正确、`run_id` 串链正确 | 表驱动（passed true / false 两形态） |
| 审计行失败面 | 异常 | `Err` 路径同样落一条审计记录（失败留痕，不吞审计），summary 载错误串 | 单例 |
| summary 有界 | 边界 | outcome `diagnostics` > 500 chars → 落库 summary 截断（既有 ≤500 口径），不溢出不 panic | 单例 |
| 取消透传 | 边界 | 请求 `cancel` 置位 → 假 runner 观测 `is_set() == true`；outcome `fault = Some(Stopped)` 原样进 `ToolStepOutput::ArchiCheck`（`steps.rs` 不做决策） | 单例 |
| 审计失败不阻断臂 | 异常 | store 注入 `StoreFault` → 业务 outcome 仍返回 `Ok`（审计面失败不淹没主流程，沿既有纪律） | 单例 |
| 既有七臂零回归 | 回归 | `PhaseNext` / `PhaseStart` / `PhaseLog` / `Backtrack` / `DecisionLog` / `StaticCheck` / `TestExecution` / `MutationTest` 既有断言逐字不变（新增臂不扰动既有分派与审计形态） | 回归（既有用例） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `ArchiCheckRunner` | **假件**：记录型实现（捕获四参、可编程返回 `ArchiCheckOutcome` 或错型变体）——与既有 `FakeRunner` / `FakeTestExecutionRunner` 同式 | 分派 / 审计 / 漂移 |
| `ArchiveVcsPort` | **假件**：可编程 `change_edit_set`（成功清单 / `Err`），捕获 `(root, base)`——进程边界（git），`steps.rs` 的取数注入面即被测对象 | 注入 / 取数失败 |
| `ChangeStateStore` | **真件**：真实 tempfile workspace 库（`store` dev-dep，既有 `TestDb` 先例）+ 故障注入臂 `StoreFault`（进程边界真实、故障可编程） | name 解析 / 审计落库 |
| 其余三 runner | **假件**：既有 `NullRunner` / 记录型（仅用于断言零调用与零回归） | 形参序 / 零回归 |
| 时钟 / 网络 | **零依赖**（store 时间戳由既有装置固定） | —— |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs

#### 待测功能

design 未声明该文件的公共 API 变更，本节无条目。

<!-- design.md「公共函数 / API」子表未声明本文件的公共 API 变更（`step_archi_check` / `step_gate_fail_phase_log` / `settle_fault` / `diagnose_brief` 均为模块私有；对外只经 `walk_run` 的行为面）。 -->

> 说明：测试对象取自 design「修改文件」`walker.rs` 单元格（① `run_step` 增 `StepNode::ArchiCheck` 臂；② 新增 `step_archi_check`；③ `step_fail_phase_log` → `step_gate_fail_phase_log`；④ `impl TryFrom<ToolStepOutput> for ArchiCheckOutcome`）、D14 / D15 / D17 / D18、AC-12 / AC-13 / AC-15；装置沿既有 `walker_test` 的假 runner + `TestDb` + `LocalToolSteps` 组合。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 步分派（AC-12） | 正向 | implement 全流：executor → static-check 绿 → `archi_check` 节点出现（`ChangeStepKind::ArchiCheck`、`group = exec`）→ 无注入时进 evaluator | 单例（全序） |
| 通过进 evaluator（AC-12） | 正向 | archi outcome `passed = true` → 无 `EdgeKind::Feedback` 边、evaluator 节点被执行、`instance_no` 不增 | 表驱动（clean / warnings 两档） |
| 违例续注同一 executor 会话（AC-12） | 正向 | `passed = false` + 诊断文本 → 一条 `EdgeKind::Feedback` 边、executor 重跑且 `instance_no` 递增 1、注入文本 == outcome `diagnostics`（原样，不二次拼装） | 单例 |
| 反馈文本经 `diagnose_brief` 直通（D15） | 边界 | `diagnostics` 含 >200 chars 单行 → 注入文本按既有 `diagnose_brief` 200 字符截断口径（walker 侧零新常量） | 单例 |
| 3 次超限升格（AC-12） | 异常 | 连续 3 次 `passed = false` → 第 3 次后相位 fail：代写一条 fail `phase_log`（`ArchiCheck` 归 `GateFailLog { item: "架构检查", … }`、`skipped = false`、仅 executor 槽位）、`StepFlow::Upgraded`、evaluator 零执行 | 单例（计数 1→2→3） |
| 三段计数互不占用（AC-12） | 边界 | 同一 attempt 内 static-check 失败 2 次 + archi 失败 3 次 → archi 侧恰在自身第 3 次升格（不被 static-check 计数消耗，反之亦然） | 单例（两反馈边交叉） |
| `skipped` 步 passed + 逐字 detail（AC-13） | 边界 | outcome `passed = true` + `diagnostics = "未检查（无模型）"` → 步状态 `passed`、节点 `detail` **逐字**为「未检查（无模型）」（不含报告路径、不写「通过」） | 单例 |
| `noChanges` 逐字 detail（AC-13） | 边界 | 同上以「未检查（无可判文件）」→ 步 `passed` 且 detail 逐字；报告 `status` 原样（不在 walker 侧改写） | 单例 |
| 报告路径呈现（D15） | 正向 | `status ∈ {clean, warnings, errors}` 且有 `report_ref` → 节点 `detail` 形如「<摘要>；报告 <ref>」；`output_ref` 恒 `None`（`report_ref` MUST NOT 写 `output_ref`） | 表驱动（3 态 + output_ref 断言） |
| 变更集取数失败即步失败（AC-15） | 异常 | `steps.rs` 返回 `Err`（vcs 失败）→ archi_check 节点 failed、`Terminal` 收口（不进入 evaluator、不降级 `noChanges`） | 单例 |
| 取消（D18） | 异常 | outcome `fault = Some(RunFault::Stopped)` → run 以 stopped 收口（`Terminal::stopped`），无 fail `phase_log`、无反馈边 | 单例 |
| `settle_fault` 同形保留（D18） | 边界 | 注入 `fault = Some(Environment(..))`（生产不可达）→ 走既有 `settle_fault` 分支且 `template = None`（消息即 `fault` 原文），零新增环境文案常量 | 单例（防御面） |
| `StepKind` 审计包络 | 正向 | archi 步失败升格时 `phase_log` 写入产生的审计行步词为 `StepKind::PhaseLog`（既有包络），archi 步自身审计由 `steps.rs` 承载（分层不重复） | 单例 |
| `TryFrom<ToolStepOutput>` 形态 | 边界 | `ArchiCheckOutcome::try_from(ToolStepOutput::ArchiCheck(..))` 成立；对 `StaticCheck` / `TestExecution` / `MutationTest` 变体 → `Err`（错型防护） | 表驱动（4 变体） |
| 16 份 prompt md 零变化（AC-12） | 回归 | `crates/core/workflow/src/prompts/**` 的 16 条 `include_str!` 逐字对拍断言不变；反馈文本不含 prompt 内嵌字样（提示来自反馈模板常量） | 回归（既有断言） |
| 零自动改写（AC-12） | 边界 | 三态（clean / warnings / errors）跑动后 tempdir 源码字节快照相等（无文件写入、无自动修复） | 单例 |
| 既有 static-check 反馈边零回归 | 回归 | `static-check` 的 fault 先行 / 计数 / 超限升格 / 续注用例逐字不变（`step_gate_fail_phase_log` 参数化后行为等价，D17） | 回归（既有用例） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `ArchiCheckRunner`（经 `LocalToolSteps` 注入） | **假件**：可编程 `ArchiCheckOutcome` 序列（`VecDeque`，既有 `FakeRunner` 同式）——`walker` 的反馈边 / 计数 / 升格语义即被测对象，判定不在本节 | 全部用例 |
| 其余三 runner / `ArchiveVcsPort` | **假件**：既有记录型（vcs 失败注入用于 AC-15 用例） | 分派 / 取数失败 |
| `ChangeStateStore` | **真件**：真实 tempfile 库（`TestDb` 装置）+ 故障臂 | 阶段落账 / 升格留痕 |
| `Agent` 会话（WorkerAgentPort） | **假件**：既有可编程假 port（记录 `continue_session` 的注入与 `instance_no`） | 续注 / 会话隔离 |
| 时钟 | **不 mock**：`Instant` / 时间戳只用于超时护栏，断言不依赖绝对时刻 | —— |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/run_history.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/run_history_test.rs

#### 待测功能

design 未声明该文件的公共 API 变更，本节无条目。

<!-- design.md「公共函数 / API」子表未声明本文件的公共 API 变更（`mapped_step` 为模块私有函数，消费面经 `finish_command` 与 `StoreRunHistory`）。 -->

> 说明：测试对象取自 design「修改文件」`run_history.rs` 单元格（`mapped_step` 增 `ChangeStepKind::ArchiCheck => RunStepKind::ArchiCheck` 臂，21 → 22）与 AC-16 / `desktop-change-state-store` MODIFIED delta。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 22 值直映射（AC-16） | 正向 | 既有 21 值映射断言逐字不变 + `ChangeStepKind::ArchiCheck` → `RunStepKind::ArchiCheck`（`"archi_check"` 词面双向一致） | 表驱动（22 值） |
| 零过滤 | 边界 | 22 值逐值映射后节点集合长度 == 输入长度（无 `None` 过滤、无静默丢步） | 表驱动（22 值） |
| `finish_command` 整包透传 | 正向 | 含一个 `archi_check` 节点的 run 收口 → `RunFinishCommand.nodes` 中该节点 `step == RunStepKind::ArchiCheck`、`detail` / `session_id` / `output_ref` 三态逐字段透传 | 单例 |
| `ChangeStepKind` 22 值穷尽匹配 | 边界 | 新增臂后 `mapped_step` 为穷尽匹配（删一臂即编译失败）；`ArchiCheck` 不与 `StaticCheck` / `MutationTest` 混淆（逐值对拍） | 表驱动（2 近邻词） |
| 未知 / 退役步词不复活（异常） | 异常 | 读面投影遇到退役 `steps` 键 / `retry` 边（既有屏蔽点）→ 零节点、零 panic；库行步词下标越界（人为构造 22 及以上）→ 解码 `Err`（不静默回落末位词 `archi_check`，防 R-8 错位被掩盖） | 表驱动（2 形态） |
| 既有 21 臂零回归 | 回归 | 既有映射用例（含 `cleanup` / `clean` / `mutationTest` 特殊词面）逐字不变 | 回归 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 映射纯函数 | **不 mock**：`finish_command` 直调（纯函数，零 IO） | 映射 / 透传 |
| `StoreRunHistory` | **真件**：真实 tempfile 库（沿既有 `TestDb` 装置） | 落库回读（既有用例） |
| 时钟 / 网络 / 子进程 | **零依赖** | —— |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/control.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/control_test.rs

#### 待测功能

design 未声明该文件的公共 API 变更，本节无条目。

<!-- design.md「公共函数 / API」子表未声明本文件的公共 API 变更（`is_session_step` 为模块私有 `matches!` 正向枚举，本变更零代码改动——事实校准 2 的复核项）。 -->

> 说明：测试对象取自 design「修改文件」`control.rs` 行的「零代码改动（复核项）」与事实校准 2（`is_session_step` 为正向枚举，`ArchiCheck` 天然判假）；断言经 `ChangeFlowControl::publish` 的行为面表达（私有函数不可直调）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ArchiCheck` 不挂会话 id（复核） | 边界 | run 中发一条 running 的 `ChangeStepKind::ArchiCheck` 节点（`session_id = None`），随后 `publish(RunUpdate::SessionEvent { session_id })` → 该节点 `session_id` 仍为 `None`，且不发补拍 `Step` 通知 | 单例 |
| `ArchiCheck` 节点正常出线（正向） | 正向 | running / passed 两态的 `ArchiCheck` 节点经既有快照与广播面 → 节点在快照中在场、步词为 `archi_check`、`session_id` 恒 `None`，一次发布一拍、零补拍（新增词不改既有发拍节奏） | 表驱动（2 态） |
| 会话型步仍挂载（零回归） | 回归 | 同一装置下 running 的 `Executor` / `Evaluator` / `Decision` / `SpecSync` / `ResolveConflicts` 五类节点 → 首拍挂载 `session_id` 且补发一拍 `Step`；二次会话事件不重复挂载 | 表驱动（5 类） |
| 零代码改动的行为等价 | 边界 | 同一 run 同时含 running 的 `ArchiCheck` 与 running 的 `Executor` → 会话 id 挂在后出现的会话型节点上（正向枚举语义，不因新增步词走否定分支） | 单例 |
| 迟到 / 无宿主会话事件（异常） | 异常 | 会话事件在 run 已收口后到达、或 run 内无 running 节点（全部终态）时发布 → 不挂载、不补拍 `Step`、不 panic；同一 `session_id` 重复发布 → 幂等（不重复挂载、不重复补拍） | 表驱动（3 形态） |
| 既有控制面零回归 | 回归 | 停止 / 应答 / 快照 / 广播既有用例逐字不变（本变更为 `ChangeStepKind` 加值，不改控制逻辑） | 回归 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `ChangeFlowControl` | **真件**：进程内 tokio 原语真实组合（既有「无 mock」纪律） | 全部用例 |
| 时钟 / 网络 / DB | **零依赖** | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/checks/src/static_check.rs -> packages/desktop/src-tauri/crates/infra/checks/src/static_check_test.rs

#### 待测功能

design 未声明该文件的公共 API 变更，本节无条目。

<!-- design.md「公共函数 / API」子表未声明本文件的公共 API 变更（本变更只在 `execute` 首部加一处 `slot == ReportSlot::Archi` 前置守卫，三行、执行语义零分叉）。 -->

> 说明：测试对象取自 design「修改文件」`static_check.rs` 单元格 + D19 / 事实校准 3 与 `desktop-checks-domain` MODIFIED delta（「两 runner MUST NOT 接受 `archi`，传入即 `Err`」）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 非 `Archi` 槽位放行（正向） | 正向 | `ReportSlot::Test` 于可执行命令的 tempdir → 守卫不触发、`execute` 进入既有执行链并返回既有 outcome 面（不是槽位 `Err`），证守卫仅收窄第三值 | 单例 |
| `archi` 槽位显式 `Err`（D19） | 异常 | `run(root, "demo", ReportSlot::Archi, CancelFlag::never())` → `Err`，文案指明「archi 槽位归架构门禁 runner」（不静默执行） | 单例 |
| 守卫先于一切 IO | 边界 | tempdir 内**无** `openspec/config.json`、无测试命令、root 为空目录 → 仍返回上述 `Err`（不是「缺配置」/「命令缺失」类错误：守卫首行前置） | 单例 |
| 零写盘 | 边界 | `Archi` 槽位调用前后 tempdir 字节快照相等；`reports/archi` **零创建**（不进报告目录派生） | 单例 |
| 零 spawn | 边界 | 以 PATH 隔离窗口（既有 `EnvWindow` 装置）令任何外部命令不可执行 → 仍返回槽位 `Err`（守卫先于 spawn，不因环境错短路成别的错误） | 单例 |
| 幂等 | 边界 | 连续两次 `Archi` 调用 → 两次均 `Err` 且文案逐字相等 | 单例 |
| 既有两槽位零回归 | 回归 | `ReportSlot::Test` / `ReportSlot::Baseline` 的既有执行语义断言（退出码映射 / 报告目录 / 输出记录 / 瞬态重试）逐字不变 | 回归（既有用例） |
| 错误文案归因 | 边界 | `Err` 串含槽位词 `archi` 与「架构门禁」字样（用户可据此定位配置错误来源，R-12 的混淆面收口） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 子进程 / shell | **不 mock**：真实 `PATH` 隔离窗口（既有 `EnvWindow` + `lock_test_env`，进程边界真实收窄） | 零 spawn 用例 |
| 文件系统 | **不 mock**：真实 `TempWs`（既有装置） | 全部用例 |
| `ReportSlot` | **不 mock**：真值直传（枚举三值即被测面） | 全部用例 |
| 网络 / 数据库 / 时钟（除超时护栏） | **零依赖** | —— |

---

### packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod.rs -> packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod_test.rs

#### 待测功能

design 未声明该文件的公共 API 变更，本节无条目。

<!-- design.md「公共函数 / API」子表未声明本文件的公共 API 变更（`execute` 首部加一处 `ReportSlot::Archi` 前置守卫，与 `static_check.rs` 同式）。 -->

> 说明：测试对象取自 design「修改文件」`testexec/mod.rs` 单元格 + D19 与 `desktop-checks-domain` MODIFIED delta（两 runner 必须显式拒收 `archi`，防「漂移 JSON 冒充 checks schema」的混居）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 非 `Archi` 槽位放行（正向） | 正向 | `ReportSlot::Test` / `ReportSlot::Baseline` 于具备项目清单的 tempdir → 守卫不触发、进入既有 detect / 计划 / 执行链并返回既有 outcome 面（零槽位 `Err`） | 表驱动（2 槽位） |
| `archi` 槽位显式 `Err`（D19） | 异常 | `run(root, "demo", ReportSlot::Archi, CancelFlag::never())` → `Err`，文案指明槽位归属 | 单例 |
| 守卫先于计划检测 | 边界 | tempdir 内无任何项目清单（无 `package.json` / `Cargo.toml`，detect 本会 `Err`）→ 仍返回槽位 `Err`（守卫先于 detect，两类错误可辨） | 单例 |
| 零写盘 / 槽位隔离 | 边界 | 调用前后 tempdir 字节快照相等；`reports/archi` 零创建；`reports/test` / `reports/baseline` 零变化 | 单例 |
| 零 spawn | 边界 | `PATH` 隔离窗口下仍返回槽位 `Err`（不进入执行链） | 单例 |
| 幂等 | 边界 | 连续两次 `Archi` 调用 → 文案逐字相等 | 单例 |
| 既有两槽位零回归 | 回归 | `Test` / `Baseline` 的既有用例（detect / 计划 / 报告 / 覆盖率 / 复用门）逐字不变 | 回归（既有用例） |
| 错误文案归因 | 边界 | `Err` 串含 `archi` 与「架构门禁」字样 | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 子进程 / 测试命令 | **不 mock**：`PATH` 隔离窗口（既有装置） | 零 spawn / 守卫先行 |
| 文件系统 | **不 mock**：真实 `TempDir`（`tempfile`，既有装置） | 全部用例 |
| 网络 / 数据库 | **零依赖** | —— |

---

### packages/desktop/src-tauri/src/commands/archi/mod.rs -> packages/desktop/src-tauri/src/commands/archi/mod_test.rs

#### 待测功能

- `archi_drift()`: `#[tauri::command] #[specta::specta]`；构造 `ProcessArchiveVcs` 后转 `*_inner`（公共函数 / API）
- `archi_drift_inner()`: `check_root` → 增量经 `vcs.change_edit_set(root, "HEAD")?` 取变更集 / 全量空集 → `drift::check`（`change: None`、`cancel: CancelFlag::never()`）→ `CheckError::message` 映射；命令层零判定（公共函数 / API）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| full 模式（AC-7） | 正向 | `archi_drift_inner(root, Full, &fake_vcs)` → 报告 `mode = full`、`base = None`、`change = None`、`changed_files = 0`；`fake_vcs` 零调用（全量不取变更集） | 单例 |
| incremental 模式（AC-15） | 正向 | `archi_drift_inner(root, Incremental, &fake_vcs)` → `mode = incremental`、`base = Some("HEAD")`、`changed_files` 反映注入清单；`fake_vcs` 捕获 `(root, "HEAD")` 一次 | 表驱动（空集 / 单元素 / 多元素） |
| 变更集失败即 `Err`（AC-15） | 异常 | `fake_vcs` 返 `Err("git 不可用")` → `Err`（**不**降级空清单），零报告 | 单例 |
| root 校验（既有纪律） | 异常 | `root = ""` / `"   "` / 不存在路径 / 指向普通文件 → `Err`（`check_root` 前置，零 vcs 零判定调用） | 表驱动（4 形态） |
| 无模型 → `skipped`（AC-8） | 边界 | tempdir 无 `<archi_dir>/models` → `Ok(report)` 且 `status = skipped`（不 `Err`） | 单例 |
| 坏模型 → 结构化错误不 panic（AC-8 / AC-12） | 异常 | 模型含 `cycle` → `Ok(report)` 且 `model.structural_errors` 非空、`status = errors`（命令层不把结构错映射成 `Err`） | 单例 |
| 配置非法 → `Err` 文案 | 异常 | `<archi_dir>/config.json` 含未知键 → `Err`，文案含条目名（`CheckError::message` 透传） | 单例 |
| 全量不落盘（AC-7） | 边界 | full 调用前后 tempdir 字节快照相等（`reports/` 零创建） | 单例 |
| 与 runner 同源（AC-10） | 正向 | 同 root、同变更集：命令面报告与 `LocalArchiCheck::run(root, name, changed, ..)` 落盘报告逐字段比较——除 `timestamp` / `change`（`None` vs `Some(name)`）外全等（`mode` / `base` / `scan` / `errors` / `warnings` / `suppressed` / `observations` / `status` / `model_hash`） | 单例（逐字段对拍） |
| 命令层零判定（AC-10） | 边界 | `include_str!("mod.rs")` 不含归属 / 定级 / status 推导实现（`owner_of` / `ArchiCheckStatus::` 构造等），命令只做参数 → 调用 → 错误映射 | 单例（源码自证） |
| 零副作用面 | 边界 | 报告 `root` 字段为入参 root 逐字（不归一）；模型与源码同根读取（worktree 场景下主仓零读取：以两个 tempdir 交叉断言） | 表驱动（2 root 形态） |
| 根路径边界 | 边界 | root 含空格 / 非 ASCII / 极深路径 → `Ok` 或 `Err` 均可但零 panic；报告 `root` 字段逐字回填入参 | 表驱动（3 形态） |
| 既有五命令零回归 | 回归 | `archi_graph` / `archi_module` / `archi_read_doc` / `archi_change_summary` / `archi_set_gate` 既有用例逐字不变（新命令不改旧命令的 root 校验与零写盘面） | 回归（既有用例） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `ArchiveVcsPort` | **假件**：可编程 `change_edit_set`（清单 / `Err`），捕获入参——进程边界（git 子进程），命令面只透传 | incremental / 取数失败 |
| 判定（`archi::drift::check`） | **不 mock**：真实调用（AC-10 同源即被测语义） | 全部取值用例 |
| 文件系统（模型 / 源码 / 配置） | **不 mock**：真实 tempdir（沿既有 `archi` 命令测试的 `TempDir` 装置） | 全部用例 |
| `tauri` State | **不 mock**：`archi_drift` 无 State 依赖 → `*_inner` 直调原签名（既有四命令同式） | 全部用例 |
| 网络 / 时钟 | **零依赖**（`timestamp` 只断言形态） | —— |

---

### packages/desktop/src-tauri/src/commands/change_flow/mod.rs -> packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs

#### 待测功能

design 未声明该文件的公共 API 变更，本节无条目。

<!-- design.md「公共函数 / API」子表未声明本文件的公共 API 变更（组合根只把 `Arc::new(LocalArchiCheck::new())` 作为第四 runner 传入 `LocalToolSteps::new`，零判定）。 -->

> 说明：测试对象取自 design「修改文件」`commands/change_flow/mod.rs` 单元格与「装配」组件行（形参插在 `mutation` 之后、`vcs` 之前）；既有两处 `LocalToolSteps::new` 直调点随签名扩参同步（编译面）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 装配可达（形参序） | 正向 | 以真件 `LocalArchiCheck` 装配的 `LocalToolSteps` 处理 `ToolCommand::ArchiCheck { change_id }`（tempdir 无模型）→ 返回 `ToolStepOutput::ArchiCheck(outcome)` 且 `outcome.report_dir` 为 `<change>/reports/archi`（形参错位会返回别的 runner 产出型别 → `Err`） | 单例 |
| 组合根零判定 | 边界 | `include_str!("mod.rs")` 不含归属 / 定级 / status 推导实现，只含装配与注入 | 单例（源码自证） |
| run 启动路径可达 | 正向 | `change_flow_start`（既有 mock app 装置）驱动 implement 相位时 `ArchiCheck` 命令由注入的 runner 承接（假 `LocalArchiCheck` 替换为记录型 runner → 观测到恰好一次调用） | 单例 |
| 注入 runner 失败面（异常） | 异常 | 注入的 archi runner 返回 `Err`（配置非法 / 扫描硬失败）→ `change_flow_start` 收口为错误面（fail 落账或 `Terminal`），**不**被降级成 `noChanges` 假通过；static-check / test-execution / mutation 三 runner 零误调用 | 单例 |
| 既有装配零回归 | 回归 | static-check / test-execution / mutation 三 runner 与 vcs / store / anchors / run_id 的装配顺序与既有断言逐字不变；既有 `mod_test.rs` 的两处 `LocalToolSteps::new` 直调点扩为八实参后行为等价 | 回归 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `LocalArchiCheck` | **真件**（装配用例）+ **记录型假件**（启动路径用例，观测调用次数与入参）——替换点即注入点 | 两用例各一 |
| 其余 runner / vcs / store / 引擎 | **沿用既有装置**（`PATH` 隔离窗口 + 假 provider + 真实 tempfile 库） | 启动路径 |
| 文件系统 / 数据库 | **不 mock**：真实 tempdir（既有 `mod_test` 装置） | 全部用例 |
| 网络 | **零依赖** | —— |

---

### packages/desktop/src-tauri/src/commands/mod.rs -> packages/desktop/src-tauri/src/bindings/mod_test.rs

#### 待测功能

design 未声明该文件的公共 API 变更，本节无条目。

<!-- design.md「公共函数 / API」子表未声明本文件的公共 API 变更（`all_commands!` 为登记宏，新增一行 `$crate::commands::archi::archi_drift,`；出口断言落既有 bindings 重导出用例）。 -->

> 说明：测试对象取自 design「修改文件」`commands/mod.rs` 单元格（唯一登记面）+ AC-20（`bindings:check` 零 diff）与 `desktop-ipc-type-bindings` 零改动理由；用例沿既有 `src/bindings/mod_test.rs` 的产物快照 / 清单覆盖先例。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `all_commands!` 登记（AC-20） | 正向 | 宏登记面 +1（`archi_drift` 尾部登记，登记序即 `generate_handler` 序）；补录清单两份（`COMMAND_NAMES` / `COMMAND_WRAPPERS`）各 +1（44 → 45），尾锚由 `archi_set_gate` 移到 `archi_drift`；清单每条都能在产物命中（无遗漏） | 单例（计数 + 尾锚 + 覆盖） |
| 产物出线（AC-20） | 正向 | `bindings.ts` 含 `archiDrift` 命令与 `ArchiDriftMode` 联合（`'full' \| 'incremental'`）；`Archi*` 报告 DTO 类型在案（`ArchiDriftReport` / `ArchiDriftEntry` / `ArchiDriftOccurrence` / `ArchiDriftSuppressedEntry` / `ArchiDriftObservation` / `ArchiDriftModel` / `ArchiGateCount` / `ArchiScanReport` / `ArchiExcludedFile` / `ArchiMatchedElement` / `ArchiUnresolvedFile` / `ArchiCheckStatus` / `ArchiDriftKind` / `ArchiObservationKind` / `ArchiSuppressVia` 共 15 类 + 命令面 `ArchiDriftMode`）；`ChangeStepKind` 含 `archiCheck`、`RunStepKind` / `StepNode` 含 `archi_check` | 表驱动（4 断言组） |
| 幂等重导出 | 边界 | `export_bindings()` 连续两次产物字节相等（零 diff 前提）；手写旁路零接触 | 单例 |
| 遗漏登记即红 | 异常 | 清单中移除 `archi_drift` 后重导出 → 产物不含 `archiDrift`（用例以「清单 → 产物」双向核对表达：登记而产物缺 / 产物有而清单无 双向失败） | 单例（双向核对） |
| 既有条目零回归 | 回归 | 既有 44 条命令（`COMMAND_NAMES` / `COMMAND_WRAPPERS` 清单长度 44）与既有 DTO / zod 断言逐字不变（新增一条不扰动既有出线） | 回归（既有用例） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| specta 导出 | **不 mock**：真实 `export_bindings()` 重导出 + 真实产物读盘（幂等确定性，既有 `file_lock` 串行化） | 全部用例 |
| 文件系统 | **不 mock**：真实产物文件（`packages/desktop/src/types/generated/`）与 `Drop` 守卫恢复 | 变更类用例 |
| 网络 / 时钟 | **零依赖** | —— |

---

### packages/desktop/src/lib/ipc.ts -> packages/desktop/src/lib/ipc.test.ts

#### 待测功能

design 未声明该文件的公共 API 变更，本节无条目。

<!-- design.md「公共函数 / API」子表未声明本文件的公共 API 变更（`ipc.ts` 是既有运行期校验装配层：`ipc` 的键集与签名完全由 `bindings.ts` 的 `commands` 推导，本变更零改本文件）。

本节存在的理由写在紧随的 `> 说明`：被测面由 design「修改文件」表 `bindings.ts` + `bindings.zod.ts` 那一行带出——`archiDrift` 命令的三张生成表项与 drift/scan schema 图的**运行期形态面**，其唯一消费入口就是本模块导出的 `ipc.archiDrift`；按「跨模块组合用例挂靠链路发起方」的口径，用例落此处，不落生成物内部。 -->

> 说明：本变更为 `archiDrift` 命令新增三张生成表项（`argsSchemas.archiDrift` / `resultSchemas.archiDrift` / `channelArgs.archiDrift`）与 drift/scan schema 图；`src/types/generated/bindings.zod.ts` 是 design「修改文件」在册的生成物（禁手改），而该图在现有测试里**从未被解析**——`use-archi-drift.test.ts` 与 `architecture-canvas.test.tsx` 走的是 `bindings.ts` 的 `commands` 包装函数（零 zod），`ipc.test.ts` 既有的 `Object.keys(commands)` 形参名遍历只触到 `argsSchemas.archiDrift` 的 2 个形参键。故本节以 `ipc.archiDrift`（经被 mock 的 `invoke`）与 `resultSchemas.archiDrift`（直接 `safeParse`）两入口做**全字段 payload parse**，判据是「drift/scan 图逐字段的接受 / 拒绝」而非「生成物文本快照」（`bindings:check` 已锁结构零 diff，本节只锁运行期形态）。覆盖率分母口径见「覆盖率分母口径（前端函数 90%）」。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `resultSchemas.archiDrift` 全字段 parse（主判据） | 正向 | `ArchiDriftReport` 14 字段全非空（`version` / `timestamp` / `mode` / `root` / `change` / `base` / `modelHash` / `model` / `scan` / `errors` / `warnings` / `suppressed` / `observations` / `status`）；`model` 5 字段（`elements` / `relations` / `byGate` 三档 / `structuralErrors` 1 条 5 字段 `ArchiDiagnostic` / `warnings` 1 条全字段 entry）；`scan` 7 字段（`mode` / `changedFiles` / `scannedFiles` / `excluded` 1 条 / `unmatchedFiles` 1 项 / `matched` 1 条 / `unresolved` 1 条）；`errors[0]` 与 `warnings[0]` 6 字段含 `occurrences` 1 条 4 字段 → `safeParse` 成功（`success === true`），一次遍历即覆盖 drift/scan 图全部字段 getter | 单例 |
| `ipc.archiDrift` 端到端全字段（零漂移） | 正向 | 同一全字段 fixture 经 `invoke` 返回 → `driftCount() === 0`（结果面零漂移即「图逐字段接受」）、返回值为原引用（非拷贝）、`invoke` 首实参为 `'archi_drift'`、payload 键集恰 `['root', 'mode']` 且 `mode` 与入参逐字相等 | 单例 |
| 枚举词面全值（N+1） | 边界 | `status` 5 值（`clean` / `errors` / `warnings` / `noChanges` / `skipped`）× `mode` 2 值 × `ArchiDriftKind` 4 值（挂 `errors[].kind`）× `ArchiObservationKind` 2 值（挂 `observations[].kind`）× `ArchiSuppressVia` 2 值（挂 `suppressed[].via`）逐值 parse 通过；词外值 `'unknown'` / `'ArchiCheck'`（大小写变体）/ `''` / `null` → parse 失败 | 表驱动（15 合法值 + 4 词外值） |
| 可空位两态（Optional） | 边界 | `change` / `base` / `errors[].from` / `errors[].to` / `errors[].gate` / `occurrences[].specifier` / `suppressed[].file` / `suppressed[].line` / `model.structuralErrors[].column` / `scan.unresolved[].specifier` 各 `null` 与有值两态 → 全 parse 通过（`null` 若被生成成非 nullable 即在此断） | 表驱动（10 键 × 2 态） |
| 必填字段缺失 | 异常 | 根级 14 键逐一删除（14 形态）+ 嵌套代表键 `model.byGate.strict` / `scan.matched[0].files` / `errors[0].occurrences[0].typeOnly` / `suppressed[0].reason` / `observations[0].to` 删除 → parse 失败，且首个 issue 的 `path` 指回被删键（`#/model/byGate/strict` 形态） | 表驱动（19 形态） |
| 错类型必拒（降级探测） | 异常 | 对 14 个根级键与嵌套代表键逐一给错类型值（`version` 给 `null`、`mode` 给 `'dirty'`、`model` 给数组、`scan.excluded` 给字符串、`occurrences[].line` 给 `'12'`）→ 全拒且首个 issue 的 `path` 指回该键。判据价值：若生成物把某字段静默降级成 `z.unknown()`（`degraded` 哨兵，即「新增 65 个 getter 只覆盖 4 个」那类形态），只有本行能断出来 | 表驱动（根级 14 + 嵌套 5） |
| 多余字段被拒（strict） | 异常 | 根级多一键 / `model` 内多一键 / `errors[0]` 内多一键（三处 strictObject）→ 全拒（防生成物退化成 loose）；经 `ipc.archiDrift` 时同形脏值只记 1 条漂移、返回值原样透传、Promise 正常 resolve | 表驱动（3 位置 + 1 装配层） |
| int 边界（`u32` 面） | 边界 | `version` `0` / `4294967295` 通过；`-1` / `1.5` / `4294967296` / `NaN` → 拒；`scan.changedFiles` / `scan.scannedFiles` / `model.elements` / `model.relations` / `occurrences[].line` 的 `0` 与 `4294967295` 通过（Rust 侧 `u32` 与 zod `int().min(0).max(4294967295)` 同界） | 表驱动（6 键 × 2 界 + 4 越界） |
| 字符串边界 | 边界 | `root` / `timestamp` / `modelHash` / `message` / `excluded[].rule` 取 `""`、`>1000` chars、含 `\n` / `\0` / emoji 全通过（schema 无 `minLength`：空串在 wire 面合法，语义裁定归 `drift/` 侧）；非 nullable 字符串键给 `null` → 拒 | 表驱动（5 键 × 4 形态） |
| 数组边界（九处） | 边界 | `errors` / `warnings` / `model.warnings` / `model.structuralErrors` / `suppressed` / `observations` / `scan.excluded` / `scan.matched` / `scan.unresolved` 九处各取空数组通过、各取单元素通过；`errors` 取 1000 条 → 单次 parse 在常规超时内完成且通过 | 表驱动（九处 × 2 形态 + 规模一例） |
| 结果面脏值只上报（装配层语义） | 异常 | fixture 把 `model.modelHash` 给 number → `driftCount() === 1`、漂移 `path === '#/model/modelHash'`、`expected === 'string'`、`got` 为该 number；返回值仍为原引用、Promise 正常 resolve（soft-report 不 throw，是 AC-19 降级面的前提） | 表驱动（`modelHash` / `status` / `scan.scannedFiles` 三处脏值） |
| 参数面本地拒（零 IPC 往返） | 异常 | `ipc.archiDrift(42 as never, 'incremental')` → `IpcArgsError`、`path === '#/root'`、`invoke` 零调用；`mode` 传词外值 `'dirty'` / `null` → 本地拒且 `path === '#/mode'` | 表驱动（3 形态） |
| `channelArgs.archiDrift` 空表 | 边界 | 表项为 `{}`、`Object.keys` 为空 → `ipc.archiDrift(root, mode)` 不包裹任何对象、零信封漂移（该命令无 `Channel` 形参，命令面不引入流式订阅） | 单例 |
| 三表表项形态与无状态残留 | 边界 | `argsSchemas` / `resultSchemas` / `channelArgs` 的 `archiDrift` 表项皆为 getter（descriptor `get` 为函数）；`resultSchemas.archiDrift` 两次读取得到不同实例但同一 payload 判定一致；`argsSchemas.archiDrift.shape` 键序恰 `['root', 'mode']`（与 `bindings.ts` 形参序对位，生成面双源一致） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core` 的 `invoke` | **mock（进程边界）**：`vi.mock` + `vi.hoisted` 分发器按命令名 `archi_drift` 返回全字段 fixture / reject，并记录调用序列与 payload（沿 `ipc.test.ts` 既有同式） | 全部经 `ipc.archiDrift` 的用例 |
| `bindings.zod.ts` 三表（`argsSchemas` / `resultSchemas` / `channelArgs`） | **不 mock**：本节观测对象即真实 schema 图——替身化则被测面消失（契约判据与覆盖判据同时失效） | 全部用例 |
| `bindings.ts` 的 `commands` | **不 mock**：真实包装函数（命令名与形参序由生成物给出，payload 键集断言依赖它） | 端到端 / 参数面用例 |
| `ipc-drift` 收集器 | **不 mock**：真实 `reportDrift` / `driftCount` / `driftReports` / `resetDrift`（漂移计数即判据，`beforeEach` 重置） | 全部用例 |
| 时钟 / 文件系统 / 网络 / 子进程 | **零依赖**：本模块零 IO（`invoke` 是唯一进程边界） | —— |

---

### packages/desktop/src/views/architecture/lib/drift-marks.ts -> packages/desktop/src/views/architecture/lib/drift-marks.test.ts

#### 待测功能

- `driftMarks()`: 只读 `observations` / `suppressed` 两段——`unfulfilledDeclaration` → 边 `unfulfilled` + `from` 节点 `unfulfilled`；`suppressed` → 边 `exempt` + `reason`；其余保持 `none`；`report === null` 原样返回（公共函数 / API）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `report === null` | 边界 | `driftMarks(null, graph)` → 返回图与入参逐字段相等（零标记，`mark` 全 `none`、`reason` 全 `null`） | 单例 |
| `unfulfilledDeclaration` 边 + 节点 | 正向 | 报告 1 条 `{ from: 'App.Auth', to: 'App.Billing' }` + 图中该边 → 该边 `mark === 'unfulfilled'`；`App.Auth` 节点 `mark === 'unfulfilled'`；`App.Billing` 节点仍 `none`（只标 `from`） | 单例 |
| `suppressed` 边 + reason（AC-19） | 正向 | 报告 1 条 `{ from, to, via: 'inline', reason: '迁移期' }` → 该边 `mark === 'exempt'`、`reason === '迁移期'`；两节点 `mark === 'none'`（豁免不改节点） | 表驱动（`via = inline` / `via = config` 两形态） |
| 两段冲突取 `unfulfilled`（D20） | 边界 | 同一 `(from, to)` 同时在 `observations` 与 `suppressed` → 边 `mark === 'unfulfilled'`（更值得提示的偏差优先，不静默丢信息） | 单例 |
| 未命中项保持 `none` | 边界 | 报告条目指向图中不存在的边 / 不存在的 `from` 元素 → 全图零标记、不新增节点、不抛错 | 表驱动（2 形态） |
| `clean` / `skipped` / `noChanges` 三报告 | 边界 | 三态报告的 `observations` / `suppressed` 皆空 → 全 `none`（AC-19「三态全 none 且图照常渲染」） | 表驱动（3 态） |
| 只读两段（`errors` / `warnings` 零参与） | 边界 | 报告 `errors[]` 非空（`warnings` / `model.warnings` 亦非空）但两段空 → 全 `none`（标记不认判定，只认两段，避免与门禁结论耦合） | 单例 |
| 同 `from` 多 `to` | 边界 | 同一 `from` 对 3 个 `to` 各 1 条观测 → 3 条边 `unfulfilled` + 该 `from` 节点仍为单个 `unfulfilled`（幂等） | 表驱动（3 边） |
| 既有 `crossKind` 值不被覆写 | 边界 | 图中某边已是 `crossKind`（非本函数产出）且不在报告两段内 → 保持 `crossKind`（只覆写命中的两项，未命中零改写） | 单例 |
| 纯函数性 | 边界 | 调用后入参 `graph` 对象深比较未变（不可变返回新值）；同入参两次调用结果深相等 | 单例（不可变 + 幂等） |
| 空图 | 边界 | `elements` / `relations` 皆 `[]` + 非空报告 → 返回空图（零新增、零抛错） | 单例 |
| 大列表 | 边界 | 1000 条 `observations` + 1000 条 `suppressed` + 1000 条边 → 命中项全部正确、未命中项保持 `none`（不 O(n²) 爆时：以单次调用在常规超时内完成表达） | 单例（规模边界） |
| `reason` 缺失防御 | 边界 | 报告项 `reason` 为空串（畸形输入）→ 边 `exempt` 且 `reason` 为空串（不崩、不擅自补文案） | 单例 |
| 畸形报告不抛错（异常） | 异常 | 越集 `kind` 值（未来 / 旧版本词汇）、`observations` / `suppressed` 为非数组（`null` / 字符串）、`from` / `to` 为空串、`report` 为 `undefined` → 函数不抛错：命中不了的条目一律忽略、未命中项保持 `none`、图照常返回（前向兼容，D21 降级面另有钩子承载） | 表驱动（5 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 报告 / 图 | **不 mock**：生成绑定类型字面量直构（`ArchiDriftReport` / `ArchiGraph`，纯函数直调） | 全部用例 |
| IPC / `invoke` | **零依赖**：本函数不做取数（取数归 `use-archi-drift.ts`） | 全部用例 |
| 时钟 / 文件系统 / 网络 | **零依赖** | —— |

---

### packages/desktop/src/views/architecture/hooks/use-archi-drift.ts -> packages/desktop/src/views/architecture/hooks/use-archi-drift.test.ts

#### 待测功能

- `useArchiDrift()`: 三触发（挂载 / `root` 变化 / 手动刷新），零 watch；`root === null` 不 invoke；失败落 `error` 且 `report = null`（公共函数 / API）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 挂载即取数（触发一） | 正向 | `renderHook(() => useArchiDrift('/ws', 'incremental'))` → `invoke('archi_drift', …)` 恰 1 次；`loading` 由 `true` → `false`；`report` 为返回值、`error === null` | 单例 |
| `mode` 透传 | 正向 | 两档各一次：`invoke` 载荷 `mode === 'incremental'` / `'full'`（`root` 同步与入参逐字相等） | 表驱动（2 档） |
| `root` 变化（触发二） | 正向 | `rerender` 换 root → 再 invoke 1 次（新 root 载入）；旧请求晚回被丢弃（`report` 为新 root 的结果，`loadedRoot` 语义与 `use-archi-graph` 同式） | 单例 |
| 手动刷新（触发三） | 正向 | `act(() => result.current.refresh())` → 再 invoke 1 次；连续两次 refresh → 累计 3 次（显式触发不去重） | 单例 |
| 零轮询 / 零 watch | 边界 | 渲染后等待一个宏任务窗口（`await Promise.resolve()` / `waitFor`）→ invoke 次数不增长；无定时器注册（零 `setInterval` / `setTimeout` 语义以「等待后计数不变」表达） | 单例 |
| `root === null` | 边界 | `useArchiDrift(null, 'full')` → 零 invoke、`report === null`、`loading === false`、`error === null`；`refresh()` 后仍零 invoke | 表驱动（2 形态） |
| 失败降级（AC-19） | 异常 | `invoke` reject（`'boom'` / `Error('x')` 两形态）→ `error` 为字符串（非空）、`report === null`、`loading === false`、不抛（render 不崩） | 表驱动（2 形态） |
| 失败后重试成功 | 异常 | 首次 reject、`refresh()` 后 resolve → `error === null`、`report` 为新值（错误面不粘滞） | 单例 |
| 卸载后不回写 | 边界 | 取数在飞行中 `unmount()` 后 resolve → 无状态写入告警、零报错（`cancelled` 语义） | 单例 |
| 竞态（root 快切） | 边界 | root 由 A → B 后在 A 的 promise resolve 之前 B 返回 → 最终 `report` 为 B 的结果（迟滞响应不覆盖新值） | 单例 |
| 返回面形状 | 边界 | 返回对象含 `report` / `loading` / `error` / `refresh` 四键；`refresh` 为稳定函数引用（`useCallback`，两次 render 恒等） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core` 的 `invoke` | **mock（进程边界）**：`vi.mock` + `vi.hoisted` 分发器，按命令名 `archi_drift` 返回 fixture / reject，并记录调用序列与载荷（沿 `use-archi-graph.test.ts` 同式） | 全部用例 |
| 定时器 | **不 mock**：零轮询断言以「等待后计数不变」表达，不引假计时器 | 零轮询 |
| 渲染环境 | **不 mock**：`renderHook` / `act` / `waitFor` 真实 React 19（jsdom） | 全部用例 |
| 文件系统 / 网络 | **零依赖** | —— |

---

### packages/desktop/src/views/architecture/components/architecture-canvas.tsx -> packages/desktop/src/views/architecture/components/architecture-canvas.test.tsx

#### 待测功能

design 未声明该文件的公共 API 变更，本节无条目。

<!-- design.md「公共函数 / API」子表未声明本文件的公共 API 变更（画布为既有组件，本变更只在组件内接线 `useArchiDrift` + `driftMarks` 并新增一处行内提示；导出面 `ArchitectureCanvas` / `ArchitectureCanvasProps` 不变）。 -->

> 说明：测试对象取自 design「修改文件」`architecture-canvas.tsx` 单元格（`useArchiDrift(root, mode)` 接线、`CanvasBody` 收 `driftError` 渲染行内提示、`const marked = driftMarks(report, graph)` 后交既有投影）、D20 / D21、AC-19 与事实校准 4（三处挂载共用同一组件，单点接线天然同源）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 报告驱动标记上屏（AC-19） | 正向 | `archi_drift` 返回含 1 条 `unfulfilledDeclaration` + 1 条 `suppressed` → `ReactFlow` 入参中该边 `mark` 分别为 `unfulfilled` / `exempt`（`reason` 同随）、`from` 节点 `mark === 'unfulfilled'`；其余项保持 `none` | 单例 |
| 三态与失败全 `none`（AC-19） | 边界 | 报告 `clean` / `skipped` / `noChanges` 三态 → 全图零标记且节点 / 边照常渲染（节点数、边数与无漂移时相等） | 表驱动（3 态） |
| 漂移取数失败降级（D21） | 异常 | `archi_drift` reject 而 `archi_graph` 成功 → 图照常渲染（零标记）+ 行内提示 `data-testid="archi-drift-error"` 在场；不出现停摆面板（`archi-structure-error` 不在场） | 单例 |
| 图失败优先于漂移失败 | 异常 | `archi_graph` 与 `archi_drift` 双 reject → 既有停摆 / 错误面接管（`archi_drift` 失败提示不掩盖图错误） | 单例 |
| `mode` 透传 | 正向 | `mode="full"` / `mode="incremental"` 两挂载形态 → invoke `archi_drift` 载荷 `mode` 逐档相等、`root` 为组件入参 | 表驱动（2 档） |
| `root === null` | 边界 | `root={null}` → 零 `archi_drift` invoke、零标记、无提示（壳态无根不取数） | 单例 |
| 聚焦切换零取数 | 边界 | 点击节点切换聚焦（既有交互）→ `archi_drift` 调用次数不增（D21 三触发纪律） | 单例 |
| 标记与既有样式契约解耦 | 边界 | `marks.ts` 的 `edgeVisual` / `nodeDot` 四值 / 两值样式契约零改动（既有 `marks.test.ts` 用例全绿作回归锚）；画布消费的 `mark` 值仍在封闭词表内（不出现新值） | 回归 |
| 既有画布面零回归 | 回归 | 全或无停摆、三段布局、缺省 1-hop 聚焦、`mode` 透传改档、`archi-refresh` 三触发等既有断言逐字不变 | 回归（既有用例） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core` 的 `invoke` | **mock（进程边界）**：按命令名分发 `archi_graph` / `archi_module` / `archi_set_gate` / `archi_drift`（既有分发器扩一条） | 全部用例 |
| `@xyflow/react` 度量垫片 | **环境垫片（非业务 mock）**：`ResizeObserver` / `offsetWidth` / `DOMMatrixReadOnly` / `getBBox` 照抄 `change-flow-graph.test.tsx`；`ReactFlow` 观测垫片记录入参后交真实组件渲染 | 标记 / 布局断言 |
| `driftMarks` / `marks.ts` | **不 mock**：真实纯函数组合（映射语义与样式契约各自单测，画布只验接线结果） | 全部用例 |
| `useArchiDrift` | **不 mock**：真实 hook（经被 mock 的 `invoke` 取数） | 全部用例 |
| 文件系统 / 网络 / 定时器 | **零依赖** | —— |

---

### packages/desktop/src/views/changes/flow/run-state.ts -> packages/desktop/src/views/changes/flow/run-state.test.ts

#### 待测功能

design 未声明该文件的公共 API 变更，本节无条目。

<!-- design.md「公共函数 / API」子表未声明本文件的公共 API 变更（`normalizeKind` / `stepWord` / `groupOf` 均为模块私有，消费面经导出的 `runStepNodes` / `nodeIdentityKey` / `unifiedRunNodes`；本变更只补两处 `case`，`groupOf` 走既有 `default`）。 -->

> 说明：测试对象取自 design「修改文件」`run-state.ts` 单元格（`normalizeKind` 补 `case 'archi_check'`、`stepWord` 补 `case 'archiCheck'`、`groupOf` 零改动）与 AC-16 / `desktop-change-flow-view` MODIFIED delta；断言经公开投影函数表达（`runStepNodes` 出线 + `nodeIdentityKey` 内嵌 `stepWord`）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `normalizeKind` 新词 | 正向 | 步记录 `step: 'archi_check'` 经 `runStepNodes` → 节点 `runStepKind === 'archiCheck'`（snake → camel 单双向在案） | 单例 |
| `stepWord` 反向归一 | 正向 | `nodeIdentityKey` 内嵌步词 → `{phase}:{attempt}:archi_check:{instanceNo}`（与后端边表端点逐字可比） | 单例 |
| 22 值 round-trip | 正向 | 22 值 snake 词逐词 `stepWord(normalizeKind(w)) === w`（词汇漂移即断；既有 21 值表扩 1 行） | 表驱动（22 值） |
| `groupOf` 归 `toolStep` | 边界 | `archiCheck` 经 `groupOf` 的 `default` 分支 → `group === 'toolStep'`（exec 组）；role 恒 `null` | 单例 |
| 词汇外步词退化 | 边界 | 越集步词（`'ARCHI_CHECK'` / `'archicheck'` / `''`）→ 分组回落 `toolStep` 且不崩（既有稳健面不变） | 表驱动（3 形态） |
| 畸形步记录（异常） | 异常 | 步记录 `step` 缺失 / `step: null` / `step` 为非字符串（旧报告或反序列化残留）、`instanceNo` 缺失 → `runStepNodes` 不抛错：`runStepKind` 原值透传、`group` 回落 `toolStep`、身份键仍可构造（不伪造 `archiCheck` 词） | 表驱动（4 形态） |
| 既有 21 值零回归 | 回归 | 既有归一 / 分组 / 身份键用例逐字不变（新臂为纯增量） | 回归 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 步记录 / 节点 | **不 mock**：宽松默认值构造 `ChangeRunEntry` / `ChangeStepState` 字面量（既有装置） | 全部用例 |
| React / IPC / 文件系统 | **零依赖**：纯函数直调（零 react） | 全部用例 |

---

### packages/desktop/src/views/changes/flow/run-step-node.tsx -> packages/desktop/src/views/changes/flow/run-step-node.test.tsx

#### 待测功能

design 未声明该文件的公共 API 变更，本节无条目。

<!-- design.md「公共函数 / API」子表未声明本文件的公共 API 变更（`STEP_LABEL` 为模块常量穷尽表 `Record<ChangeStepKind, string>`，本变更补 `archiCheck: '架构检查'` 一行；`GROUP_LABEL` 四类不变）。 -->

> 说明：测试对象取自 design「修改文件」`run-step-node.tsx` 单元格与 AC-16；断言经 `RunStepNode` 渲染的 `data-testid="run-step-kind"` / 分组徽章表达，穷尽表由类型系统兜底（缺键即编译红）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `STEP_LABEL` 新键上屏 | 正向 | 节点载荷 `runStepKind: 'archiCheck'` → `data-testid="run-step-kind"` 文案逐字为「架构检查」 | 单例 |
| 分组徽章归 exec | 正向 | 同一节点 `group: 'toolStep'` → `data-testid="run-step-group"` 文案为既有 exec 类徽章（`GROUP_LABEL` 四类零改动） | 单例 |
| 22 值穷尽表 | 边界 | 22 值 `ChangeStepKind` 逐值渲染均得非空文案（无 `undefined` 上屏）；穷尽性由 `Record<ChangeStepKind, string>` 类型保证，缺键即编译失败 | 表驱动（22 值） |
| detail 逐字上屏（AC-13 交汇） | 边界 | 节点 `detail = '未检查（无模型）'` → 详情区逐字呈现（不追加「通过」、不截断）；`detail = null` → 无详情区且不崩 | 表驱动（2 形态） |
| 超长 detail | 边界 | >1000 chars detail → 渲染不崩（样式截断由 CSS 承担，文案不改写） | 单例 |
| 非法 `runStepKind` 兜底（异常） | 异常 | 节点载荷 `runStepKind` 为越集值（`'archi_check'` 反灌 / `''` / `undefined`）→ 渲染不崩、不上屏 `undefined` 文案（渲染按 `STEP_LABEL[kind] ?? 原值` 类兜底表达），既有 testid 结构不变 | 表驱动（3 形态） |
| 既有 21 值零回归 | 回归 | 既有步词 / 分组 / role 三面断言逐字不变 | 回归 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@xyflow/react` | **不 mock**：真实 `ReactFlowProvider` 包裹渲染（既有装置） | 全部用例 |
| IPC / 文件系统 / 网络 / 定时器 | **零依赖** | —— |

---

## 不可测试项

| 模块 / 面 | 原因 |
|---|---|
| `packages/desktop/src-tauri/crates/core/archi/src/lib.rs`（AC-8 出口） | 本变更只增两行 `pub mod drift; pub mod scan;` + 一批 `pub use`（含 `ArchiCheckStatus` 等五类型与 `check` / `CheckInput` / `CheckError` / `CheckConfig` / `IgnoreRule` / `scan` / `ScanInput` / `ScanOutcome`），无行为面；可达性由各 `drift_*` / `scan_*` sidecar 的编译期挂载机械保证（漏声明即 `failed to resolve mod`），出口正确性由 `drift/mod_test.rs` / `scan/mod_test.rs` 经 `crate::` 路径使用即证 |
| `packages/desktop/src-tauri/crates/infra/archi/src/lib.rs`（AC-14 出口） | 仅 `mod report;` + `pub use runner::LocalArchiCheck;` 两行挂载，无行为面；`LocalArchiCheck` 的语义由 `runner_test.rs` 覆盖，`report` 的写盘面由 `report_test.rs` 覆盖 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/lib.rs`（AC-16 出口） | 仅 `pub use port::{… ArchiCheckOutcome, ArchiCheckRunner …}` 字母序插入，无行为面；`port_test.rs` / `steps_test.rs` 经 crate 内路径使用即证 |
| AC-16 语料 golden 重写的「人工确认留痕」 | 显式重写流程中的人工确认是流程性步骤，不可自动化断言；等值回归（重写后与复核轮字节相等）由 `corpus_golden_test.rs` 章节承载，重写轮仅在重写流程启用 |
| `packages/desktop/src-tauri/crates/core/archi/Cargo.toml`、`packages/desktop/src-tauri/crates/infra/archi/Cargo.toml`、`packages/desktop/src-tauri/Cargo.toml`（AC-17） | 清单面（依赖声明 / `members` / `[workspace.dependencies]`）无单测路径；且「`archi` workspace 内依赖仅 `foundation`」「`archi-runtime` 零 spawn / 零 vcs / 零 store / 零 config」「`workflow` ⇄ `archi` 双向零依赖」三条**编译器即断言**（依赖缺席 ⇒ `use` 不成立 ⇒ 编译红），无需运行时用例。`Cargo.lock` 净增包数（AC-18 硬线）与 `cargo tree` 方向审查由桌面端机械步骤核验 |
| `packages/desktop/src/types/generated/bindings.ts` + `bindings.zod.ts`（AC-20 生成物本体） | 生成物（禁手改）：**结构面**（零 diff / 重导出 / 命令清单）由 `bindings:check` 与 `src/bindings/mod_test.rs` 的覆盖用例承载（见 `commands/mod.rs` 章节）；生成物内部**不设文本快照式用例**（排序 / 格式化级变化即噪声）。其**运行期形态面**（`archiDrift` 报告与 scan 报告能否被前端接受 / 该拒的形态是否真被拒）不是无测试路径——断言经消费方入口表达，per-file 章节见 `packages/desktop/src/lib/ipc.ts`（`ipc.archiDrift` / `resultSchemas.archiDrift` 全字段 parse，覆盖 drift/scan schema 图 65 个字段 getter） |
| `packages/desktop/package.json`（AC-20 版本行） | 版本字符串无单测路径；由版本 diff 审查核验（`0.6.4` → `0.6.5`，`src-tauri/tauri.conf.json` 经 `"../package.json"` 跟随、`src-tauri/Cargo.toml` 版本行不随动） |
| `packages/desktop/src-tauri/crates/core/archi/tests/README.md`（AC-18） | 成本实测表为**测量记录**（NSIS / 主二进制 / 锁包数 / CI 冷构建 / 本地增量五项 + 回退序留痕），非测试代码；与 `design.md` 表逐字同步由评审核验 |
| `packages/desktop/src-tauri/crates/core/archi/tests/perf.rs`（AC-18 运行期预算装置，proposal「测试文件」表新增） | design §6 明定「单次检查 p95（300 元素 / 1000 源文件，release）由 test-execution 相位执行 `tests/perf.rs` 后回填 `tests/README.md` 占位」——该文件是**计时装置**（`std::time::Instant` 十次取 p95、生成 1000 源文件 + 300 元素夹具），其断言对象是机器性能而非判定语义；本设计不为计时脚本设 per-file 用例章节，也不复制阈值 / 重跑细节。判定语义由 `drift/mod.rs` 与 `scan/mod.rs` 章节承载 |
| AC-9 的「`crates/**` 产品源码零 `config.json` 字面量」全局扫描面 | 既有「命名隔离双禁令扫描」只覆盖 `packages/desktop/src` 与 `src-tauri/src`（`crates/**` 缺口为既有裁定，proposal「不要修改」未授权扩面）。本变更在 `drift/config.rs` 章节以「读自身源码零字面量」用例在缺口内自证；其余 crate 面（`archi` / `archi-runtime`）由源码审查承载 |
| AC-20 的 `bindings:check` / `vp check` / `knip` / `cargo clippy -D warnings` | 静态脚本与机械门禁由桌面端执行；本设计只做用例映射，不复制运行命令细节 |
| `packages/desktop/src-tauri/crates/core/workflow/src/prompts/**`（16 份角色 md，AC-12 零改动面） | proposal「不要修改」：门禁失败诊断经既有反馈边注入，不给 prompt 加架构说明；16 条 `include_str!` 逐字对拍由既有用例承载（`walker_test.rs` 回归行引用，不新增映射） |
| `packages/desktop/src-tauri/crates/core/archi/src/{model,parse,graph,docs,search,diff,edit,dto}.rs`（零改动面） | proposal「不要修改」：模型解析 / 图构建 / 结构诊断 / 文档关联 / 检索 / diff / gate 写回 / 三面投影零改动；本变更只经 `dto::graph_view` 消费（D2）。其行为由既有 `model_test` / `parse/*_test` / `graph_test` / `docs_test` / `search_test` / `diff_test` / `edit_test` / `dto_test` 回归锚承载 |
| `packages/desktop/src-tauri/crates/infra/{vcs,store,exec,watch,agent}/**`（零改动面） | proposal「不要修改」：git 能力 / 库表结构 / 输出记录 / 文件监视 / 工具面零改动；`archi-runtime` 不经这些 crate（AC-17）。既有用例即回归锚 |
| `packages/desktop/src-tauri/crates/core/config/**` + `config.schema.json` + `packages/desktop/src` 的 config 页（AC-9 零改动面） | 架构域配置不是工作区配置：`KNOWN_KEYS` 七项、schema 镜像、前端 config 页零变化；既有一致性用例（`config/src/lib_test.rs`）与前端 config 用例即回归锚 |
| `openspec/specs/**` 主基线与 `openspec/changes/archi-check-and-gate/**`（proposal / design / tasks / specs / explore） | 主基线落主基线只在归档 `SpecSync` 步；本 change 目录内文档为设计输入产物，无测试路径（`explore.md` 为只读输入） |
| `.dev-team/architecture/models/03-backend-infra.toml`、`.dev-team/architecture/models/04-backend-shell.toml`、`.dev-team/architecture/decisions/0001-model-granularity.md`（AC-21 被测数据资产） | TOML / Markdown **数据资产**，无源文件 → 测试文件的映射对，也无单测路径：proposal 明定核对「按 `repo-archi-model`『核对以命令 + 留痕执行』，MUST NOT 把本仓模型塞进 `crates/core/archi/tests/corpus/**`」。核验手段为 `archi_graph(root)`（零诊断零 `warnings`）+ `archi_module("App.Backend.Infra.ArchiRuntime")`（三出边 + Shell 入边）+ 目录 ↔ 元素双向差集比对 + 三处计数 `grep`，结果留痕 `reports/archi/model-sync-check.md`。`ArchiRuntime` 本体的语义（`kind` / `gate` / `path` 形状）归 `desktop-archi-model`，其既有 `model_test` / `parse` 用例即回归锚 |
| `openspec/changes/archi-check-and-gate/reports/archi/model-sync-check.md`（新增，AC-21 核对留痕） | change 目录内**核对证据**（非漂移报告、无消费面）：不写入 `<timestamp>.json` 报告序列、不被门禁 / 命令面 / 画布读取。内容正确性由 AC-21 的命令取证与评审比对核验，无自动化断言面 |
| `.dev-team/architecture/models/{01-app,02-backend-core,05-backend-tools,06-frontend}.toml`、`models/**` 的 `gate` 档位（全 `off`）与 `.dev-team/architecture/config.json`（不创建）（零改动面） | proposal「不要修改」：四文件不因本变更增减元素或边；`gate` 全 `off` 是冷启动 ratchet 既定状态（追加只让图重新自洽，不设门禁，R-4）；架构域看护配置缺失即空配置，示例配置只出现在用例 tempdir。既有模型解析用例与 `archi_graph` 正常态取证即回归锚 |
| `packages/desktop/src-tauri/crates/core/archi/src/drift/mod.rs` 的「`to` 元素名不在模型内」防御臂（spec AC-4 逐字） | **运行时不可达**：`to` 恒来自 `scan::owner::OwnerIndex::owner_of` 的 `Owner::Element { fqn }`（只可能是模型内元素），故 `!known.contains(to)` 分支无公开 API 输入可达。目标文件无主的真实形态走「丢边 + `scan.unmatchedFiles[]` 留痕」（运行期用例在 `drift/mod_test.rs`，语料 `target-unowned` 对拍）；该防御臂只保留源码旁证（同节 `include_str!("mod.rs")` 行），**MUST NOT** 为它虚构可构造的运行时输入（否则等于在与设计相悖的断言上写测试） |

