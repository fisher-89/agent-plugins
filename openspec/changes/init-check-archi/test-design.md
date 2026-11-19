# 测试设计: init-check-archi

> **变更**: init-check-archi（initialize 表头相位追加第三个基线检查步 `archi_check`：全量判定 + `on_fail=Terminal`）
> **角色**: test-design-planner（executor）—— 只设计测试范围与用例，不含评估结论、不含测试运行脚本细节
> **输入**: `proposal.md`（病灶三则 / 提案一~六 / 变更范围「实现文件 8 行 + 测试文件 6 行」/ AC-1…AC-11 / R-1…R-9）、`design.md`（事实校准 1–7 / D1–D14 / 修改文件 8 行 / 机械闭合面 5 行 / 公共函数·API 3 行 / 类型定义 5 行 / 配置 1 行 / 测试面归属边界）、6 份 delta spec、`tasks.md`（实现相位承接面 / 移交说明）
> **被测范围**: design「新增文件」= **0**（`<!-- 新增文件：本变更新增 0 文件 -->`）；「修改文件」子表 **全 8 项**——`phase_table.rs`、`port.rs`、`steps.rs`、`walker.rs`、`workflow/state.rs`（仅 doc）、`foundation/layout/mod.rs`（仅 doc）、`infra/archi/runner.rs`、`packages/desktop/package.json`（版本行）。删除文件零项、新增依赖零项、前端零改动、IPC / bindings 零 diff、判定域（`core/archi/src/{drift,scan,dto}`）零改动。
> **本变更性质**: 只改**挂载点与观察口径**——相位表多一步、port 多一个模式位、`steps.rs` 多一条分派、walker 多一个单发终态执行体、runner 多一处 `mode` → `ArchiDriftMode` 映射。判定规则（扫什么 / 怎么归属 / 定级 / 豁免 / 报告形状）与落盘单点零改动。

**测试框架与既有布局（读盘识别结果）**

| 面 | 运行器 | 文件形态 | 断言与装置 |
|---|---|---|---|
| Rust（`packages/desktop/src-tauri/crates/**`） | `cargo nextest run --workspace`（`pnpm -C packages/desktop run server:test`） | **sidecar 同目录同前缀** `*_test.rs`（`phase_table.rs` → `phase_table_test.rs`、`port.rs` → `port_test.rs`、`steps.rs` → `steps_test.rs`、`walker.rs` → `walker_test.rs`、`workflow/state.rs` → `state_test.rs`、`layout/mod.rs` → `layout/mod_test.rs`、`infra/archi/runner.rs` → `runner_test.rs`），源文件尾以 `#[cfg(test)] mod <name>_test;` 挂载 | 仅 `assert!` / `assert_eq!` / `assert_ne!` / `matches!`，零第三方断言库；测试函数名为中文短语；装置为手写端口假件（`FakeTools` / `FakeWorker` / `FakeVcs` / `RecordingArchiRunner` / `RecordingArchiCheck` / `NullArchiRunner` / `CountingArchiRunner` / `StubSnapshot`）与 `TestDb`（真实 sqlite）、`tempfile::TempDir`、`TempRoot`；源码自证经 `include_str!` 或 `env!("CARGO_MANIFEST_DIR")` + `std::fs::read_to_string`（`phase_table_test.rs:3570` `phase_table_source()`、`runner_test.rs:1000` `include_str!("runner.rs")`、`layout/mod_test.rs:1578` 先例） |
| 前端 / 生成物 / 配置 | — | **零测试路径**（`packages/desktop/src/**`、`src/types/generated/**`、`package.json` 均零代码改动或零行为面） | 登记于 `## 不可测试项` |

**关键事实（用例设计所依据的实测口径）**

- `INITIALIZE_STEPS` 为 `workflow` crate 内私有 `static`，两表（requirement / test-only）initialize 条目共用同一指针；断言面经 `phase_table(workflow_type)` 与 `std::ptr::eq(steps.as_ptr(), …)` 可判（既有 `两表initialize共用同一static单源且clean步独占` 先例）。
- step 数期望表：requirement `[3,2,2,2,4,3,2,1,1,9]`（29）→ **`[4,2,2,2,4,3,2,1,1,9]`（30）**；test-only `[3,2,2,2,3,2,9]`（23）→ **`[4,2,2,2,3,2,9]`（24）**。
- walker 的 initialize 路由此刻由 `steps_of(phase)`（真件 `phase_table` 克隆）驱动 ⇒ 四节点链用例**必须**经相位表真件（组合用例挂 walker 章节，见下文「跨模块组合用例挂靠」）。
- `settle_phase` 的 initialize 分支判据是「`cursor.test_outcome` 在场」，**不读 steps 内容**；机械 checklist 恒三条目（suite 结论一致 / 聚合计数一致 / mutation null 自动通过）——新步不新增 checklist 条目（回归面）。
- `infra/archi` 的 `Full` 语义由判定域既有实现决定：`scan.changed_files = 0`、`base = None`、`scanned_files` = 全根可判文件数、`changed` 在 `Full` 分支不被读（`crates/core/archi/src/scan/mod.rs:110`、`drift/mod.rs:399`）。
- `runner_test.rs` 已用 `include_str!("runner.rs")` 守「零第二套判定」「实现体与常量零新增分支」——本变更的映射单点判据与该断言族同形，不得与之冲突。

**参数类型 → 边界覆盖口径（本变更加载点）**

| 类型 | 覆盖要求与本变更加载点 |
|---|---|
| int / number（`limit: u32`、`scanned_files` / `changed_files: u32`、`occurrences` 计数） | `limit`：`0`（首败即升格的边界，不得无限循环）、`3`（`ARCHI_FEEDBACK_LIMIT` 真值）、`5`（`FEEDBACK_LOOP_LIMIT`，与 archi 可辨）、`u32::MAX`（不 panic、不溢出）；`-1` 在类型层不可表达；`changed_files`：`0`（全量必为 0）/ `1` / `3`（增量）；`scanned_files`：`0`（noChanges）/ 单文件 / 全根 N |
| str / string（`diagnostics`、`report_dir`、`name`、`root`、`change_id`、`{dir}` 占位值） | `""`（空 `report_dir` 占位 / 空 `name` 的目录派生）、超长（`diagnostics` > 1000 chars → `diagnose_brief` 截断 ≤ 200）、特殊字符（`\n` / `\0` / emoji → 压空白单行）、`None`（`Option<String>`：`report_ref` / `detail`） |
| bool（`passed`、`cancel.is_set()`、`skipped`） | `true` / `false` / 「字段缺席」（`fault = None` 的中性态）三形态；`passed` 与 `status` 五值交叉由 AC-5 / AC-4 用例承载 |
| list / array（`changed: Vec<String>`、`steps`、`nodes`、`edges`、`sessions`、`commands`） | `[]`（全量恒空 / 增量空集 → noChanges）、单元素、三元素（保序）、超大（40 条违例聚合 → 文本有界，既有 `反馈文本有界` 形态）；`steps`：3 → 4；`edges`：2 → 3 |
| dict / object（`ArchiCheckOutcome` 五字段、`ToolCommand::ArchiCheck` 两字段、`PhaseLogInput`） | `ArchiCheckOutcome` 五字段全给（少字段编译失败）；`report_ref`：`Some` / `None` 两态；`ToolCommand::ArchiCheck` 必须两字段齐（缺 `mode` 编译失败） |
| Optional[T] | `fault`：`None`（生产恒 None）/ `Some(Stopped)`；`report_ref`：`Some` / `None`；`detail`：`Some` / `None`；`base`：`None`（全量）/ `Some("HEAD")`（增量） |
| Enum（`ArchiCheckMode` / `ArchiDriftMode` / `ArchiCheckStatus` / `OnFail` / `StepNode` / `ReportSlot`） | `ArchiCheckMode`：`Full` / `Incremental` + 非法值（类型层不可表达，以字面词负向断言）；`ArchiCheckStatus`：五值各一（`skipped` / `noChanges` / `clean` / `warnings` / `errors`）；`OnFail`：`Terminal` / `FeedbackLoop` / `Escalate` / `None` 四态各一（`archi_check` 臂三分派）；`ReportSlot`：三值封闭不变 |
| float | **零 float 面**：本变更无 `f64` 入参 / 字段（`MutationTestOutcome.score` 属既有面、非本变更断言对象） |

**跨模块组合用例挂靠（按链路发起方 / 最上层调用方）**

- **相位表 → 相位机**（`phase_table()` / `steps_of` 真件 → `run_step` 步迭代 → `settle_phase`）：最上层调用方 = walker ⇒ 组合用例写 `walker.rs -> walker_test.rs` 章节（`initialize四步序…` 用例），**不新建独立集成章节**。
- **walker → steps → 门禁 runner**（`ToolCommand::ArchiCheck` 命令构造 → `steps.rs` 模式分派 → `archi.run(mode)`）：入口在 walker ⇒ 红 / 绿 / `skipped` / 取消 / 漂移五类整链用例落 walker 章节；`steps.rs` 章节只承接「命令 → vcs + runner + 审计行」这一段（发起方 = `steps.rs` 的 `ToolStepPort::run`）。
- **门禁 runner → 判定域 → 报告树**（`LocalArchiCheck::run` → `drift::check` → `write_report`）：入口 = `infra/archi/runner.rs` ⇒ 自映射章节。
- **基线语义 vs implement 反馈语义同源对照**（同一 runner、同一 `passed` 派生、两挂载点）：入口 = walker（两挂载点分派都在 `step_archi_check`）⇒ 对照断言落 walker 章节，不另设「一致性测试区」。

---

## 验收范围

<!-- 被测面两类都必须进范围：
     ① 行为面 —— design「公共函数 / API」子表逐条映射（port.rs 两行、infra/archi/runner.rs 一行），另按 design「修改文件」子表补齐无 API 行的文件（phase_table.rs / steps.rs / walker.rs / workflow/state.rs / layout/mod.rs）——它们的判据由 AC-1 / AC-3 / AC-5…AC-7 / AC-10 与 D1 / D5 / D6 / D7 / D8 / D11 承载，不是从导出声明反推。
     ② 机械面 —— `ArchiCheckRunner` 签名与 `ToolCommand` 载荷变更的编译扩散（design「机械闭合面」5 行：walker_test 两个假件、steps_test 两个假件 + 13 处构造 + 1 模式臂、port_test 的 RecordingArchiCheck + 两处 UFCS 调用 + 1 模式臂、infra/archi/runner_test 三处 `.run(`、infra/checks 的 UnusedArchiSeam），只补 `mode` 形参 / 字段 / 实参位，零断言语义改动；补位后既有断言全绿即回归判据。
     路径前缀 `packages/desktop/src-tauri/` 在章节标题中逐字保留（避免同名 crate 混淆），表格内可按后缀辨认。 -->

| AC ID | 验收条件 | 被测文件或模块 |
|---|---|---|
| AC-1 | 相位表四步单源：两表 `initialize` 逐位 `Clean → StaticCheck → ArchiCheck → TestExecution`（指针等值证明同一 static）；三检查步 `on_fail = Terminal`、`Clean = None`；两表其余相位零 `ArchiCheck`（test-only 的 initialize 之外）；`Escalate` 仍不得落 `archi_check` | `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs` -> `.../write/phase_table_test.rs` |
| AC-2 | port 词汇与分派：`ArchiCheckMode` 封闭两值、`as_str` 逐值 `full` / `incremental`；`ToolCommand::ArchiCheck` 携 `mode`（第 2 位）；`ArchiCheckRunner::run` 收 `mode`（第 3 位） | `.../core/orchestration/src/port.rs` -> `.../orchestration/src/port_test.rs`；穷尽 `match` 的编译期锚见 `## 不可测试项` |
| AC-3 | 全量零 vcs：initialize 全量臂 vcs 缝零调用（`FakeVcs` 调用计数 0）、`changed` 为空、runner 收到 `Full`；增量臂逐字保留（取数失败 → 步失败、`MUST NOT` 降级空清单） | `.../core/orchestration/src/steps.rs` -> `.../orchestration/src/steps_test.rs`；`.../core/orchestration/src/walker.rs` -> `.../orchestration/src/walker_test.rs`（命令载荷 `mode` 逐值） |
| AC-4 | 全量落盘：以 `Full` 跑真实模型 → 报告落 `reports/archi/<timestamp>.json`、报告体 `mode = full`、`change` = change 名、`report_ref` 为该文件；`reports/baseline` / `reports/test` 字节零变化 | `.../infra/archi/src/runner.rs` -> `.../infra/archi/src/runner_test.rs` |
| AC-5 | 三态收口：`clean` / `warnings` → 节点 passed + Proceed；`errors`（含结构错）→ 节点 failed + `Terminal::failed(TXT_BASELINE_ARCHI_FAILED 替换 {dir})`；`skipped` → passed 且 detail 逐字「未检查（无模型）」；零可判文件 → detail 逐字「未检查（无可判文件）」；取消 → 节点 stopped + run stopped | `.../core/orchestration/src/walker.rs` -> `.../orchestration/src/walker_test.rs` |
| AC-6 | 零会话零反馈边零改写：archi 基线红时零 WorkerAgent 会话注入、反馈边计数 0、模型与源码字节零变化、PhaseRecord 零 initialize 条目；再发起时 Clean 后干净重试 | `.../core/orchestration/src/walker.rs` -> `.../orchestration/src/walker_test.rs` |
| AC-7 | 全绿链四节点：initialize 全绿 run 的 emit 序 `clean / static_check / archi_check / test_execution`，forward 边 3 条、四节点恒 passed、零 `phaseStart` / `phaseLog` 节点；全绿后持久跳过语义不变 | `.../core/orchestration/src/walker.rs` -> `.../orchestration/src/walker_test.rs`（组合用例：相位表真件 → 相位机） |
| AC-8 | 本仓基线实证（**非 MUST**，登记留痕）：全量判定改动前后均 `status = clean`（`errors = 0` / `warnings = 0` / `structuralErrors = 0` / `scannedFiles = 396` / `by_gate` 全 `off`） | 无单元测试面（手工全量判定输出留痕），见 `## 不可测试项` |
| AC-9 | 生成物与资产零 diff：`bindings.ts` / `bindings.zod.ts` 零 diff；`.dev-team/architecture/**` 零 diff；`api:check` 零差异 | 机械核对面（`git diff --stat` + `pnpm api:check`），见 `## 不可测试项`；反面对应在 `layout/mod.rs` / `workflow/state.rs` 的「出线类型 doc 零改动」源自证 |
| AC-10 | 口径一致：源码注释与 spec 中「initialize 相位两检查步」「三节点（Clean / StaticCheck / TestExecution）」两串零残留（或仅剩改述）；`TXT_BASELINE_*` 四条文案互不相同 | `.../write/phase_table.rs` -> `phase_table_test.rs`（doc 口径行）；`.../core/orchestration/src/walker.rs` -> `walker_test.rs`（四条文案对拍）；`.../core/workflow/src/state.rs` -> `state_test.rs`；`.../core/foundation/src/layout/mod.rs` -> `layout/mod_test.rs`；跨文件 `grep` 终态见 `## 不可测试项` |
| AC-11 | 门禁与版本：`pnpm -C packages/desktop run check` 绿；`cargo nextest run --workspace` 全绿；`package.json` `0.6.47 → 0.6.48`、`src-tauri/Cargo.toml` crate 版本行不变 | 门禁命令由桌面端执行 + `packages/desktop/package.json` 版本行（见 `## 不可测试项`）；全绿门禁的承托面 = 上表全部既有 + 新增用例 |
| —（非 AC） | 版本纪律单行（`version` +1 patch、`tauri.conf.json` 经 `../package.json` 跟随、`Cargo.toml` crate 版本行不随动） | `packages/desktop/package.json`，见 `## 不可测试项` |

**被测文件全覆盖对照**（design「修改文件」8 行 → 上表落点）：`phase_table.rs` → AC-1 / AC-10；`port.rs` → AC-2；`steps.rs` → AC-3；`walker.rs` → AC-3 / 5 / 6 / 7 / 10；`workflow/state.rs` → AC-10（doc-only）；`layout/mod.rs` → AC-10（doc-only）；`infra/archi/runner.rs` → AC-4；`packages/desktop/package.json` → AC-11 + 非 AC 版本行（无单元测试面）。

---

## 单元测试

### packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table_test.rs

既有 sidecar（3676 行）**改写 + 追加**：`INITIALIZE_STEP_ORDER` 常量 3 → 4 元素、步数期望表两表各 +1、`expected_on_fail` 增 `ArchiCheck if initialize → Terminal`、`Terminal` / `Escalate` 归属守卫扩充、`on_fail回归锚` 组一扩为三检查步、doc 步序串断言改写。**零新增测试文件**。

<!-- design.md「公共函数 / API」子表无本文件行（`INITIALIZE_STEPS` 为 crate 内私有 static，非公共 API）——下列条目的出处是 design「修改文件」表本文件行 + D1 / D11 + AC-1 / AC-10，不是从导出声明反推。 -->

#### 待测功能

- `INITIALIZE_STEPS`（私有 `static`，修改）: `Clean(None) → StaticCheck(Terminal) → ArchiCheck(Terminal) → TestExecution(Terminal)` 四步序；两表 initialize 共用同一 static（指针等值）；`ArchiCheck` 在 initialize 表项 `prompt_ref: None`、`on_fail: Some(OnFail::Terminal)`
- `INITIALIZE_STEP_ORDER`（测试面期望常量，修改）: 3 → 4 元素，序与真实表逐位对拍
- `expected_on_fail()`（测试面期望表，修改）: `ArchiCheck if initialize → Some(OnFail::Terminal)`；`ArchiCheck if implement → FeedbackLoop { limit: ARCHI_FEEDBACK_LIMIT }` 逐字保留
- `INITIALIZE_STEPS` doc 与 initialize `PhaseDefinition` 注释（doc-only，修改）: 四步序串 + 三检查步 + 插入理由一句；「三步」零残留

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `INITIALIZE_STEPS` / `INITIALIZE_STEP_ORDER` | 正向 | 两表 initialize `steps` 逐位 `[Clean, StaticCheck, ArchiCheck, TestExecution]`（4 元素期望常量逐位对拍，2 表各一；常量元素数恰 4） | 参数化迭代（2 表 × 4 位） |
| `INITIALIZE_STEPS` | 正向 | 步声明逐字段：三检查步 `on_fail = Some(OnFail::Terminal)`、`Clean = None`、四步 `prompt_ref` 恒 `None`；`ArchiCheck` 落位在 `StaticCheck` 之后、`TestExecution` 之前（末步仍 `TestExecution`） | 参数化迭代（2 表 × 4 步） |
| `INITIALIZE_STEPS` | 正向 | 单源证明：两表 initialize `steps` 指针等值（`std::ptr::eq(steps.as_ptr(), …)`）+ 逐元素等值；相位其余字段不变（`artifact_manifest` 空 / `prerequisites` 空 / 双角色 `None` / `backtrackable = false`） | 单次 |
| `INITIALIZE_STEPS` | 异常 | 越界声明：两表除 initialize 外的 15 个相位零 `ArchiCheck` 步（test-only 的 archi 只能经共享表头出现在 initialize）；`Clean` 仍 initialize 独占 | 组合迭代（2 表 × 15 相位） |
| `INITIALIZE_STEPS` | 异常 | `Terminal` 白名单守卫（新增）：全表遍历，`Some(OnFail::Terminal)` 仅允许 initialize 的 `StaticCheck` / `ArchiCheck` / `TestExecution` 三步——其它相位任一步携 `Terminal` 即红；agent 步（`role().is_some()`）零 `Terminal`、零 `Escalate`；`FeedbackLoop` 白名单与既有相同（不得把 `ArchiCheck` 的白名单扩到 initialize） | 组合迭代（2 表 × 全步） |
| `INITIALIZE_STEPS` | 异常 | `Escalate` 归属守卫（既有，保持绿）：`Escalate` 仅 test-execution 的 `TestExecution` / `MutationTest`；`archi_check` 在 initialize 携 `Escalate` 时被 `expected_on_fail` 对拍拒收（表驱动不命中即红） | 参数化迭代（2 值：`Escalate` / `None`） |
| `INITIALIZE_STEPS` | 边界 | 步数边界（两表逐项 + 合计）：requirement `[4,2,2,2,4,3,2,1,1,9]` 合计 30；test-only `[4,2,2,2,3,2,9]` 合计 24；initialize `steps.len() == 4`（恰 +1，非 ±2） | 参数化迭代（2 表 + 2 合计） |
| `expected_on_fail()` | 正向 | `implement` 的 `ArchiCheck` 仍 `FeedbackLoop { limit: ARCHI_FEEDBACK_LIMIT }` 且 `ARCHI_FEEDBACK_LIMIT == 3`（与 static-check 的 `FEEDBACK_LOOP_LIMIT == 5` 可辨、互不占用） | 参数化迭代（3 判据） |
| `expected_on_fail()` | 异常 | 反馈上限负向面：`ArchiCheck` 的 `limit` 不得取 `0`（零上限反馈无意义）也不得取 5 / `u32::MAX`（与 static 混档）；三档非法值经对拍表拒收 | 参数化迭代（4 档：0 / 3 / 5 / `u32::MAX`） |
| `expected_on_fail()` | 边界 | initialize 外的 `ArchiCheck` 不得为 `Terminal`：`phase_id != "initialize"` 时该步期望值恒 `FeedbackLoop{3}`（反向断言，防把白名单放松成全相位） | 单次 |
| `on_fail回归锚` | 正向 | 组一扩为三检查步（`StaticCheck` / `ArchiCheck` / `TestExecution` 均 `Terminal`）；组二 / 三（implement / test-gen 的 static 反馈边 5 次）、组四（archive 9 步全 `None`）、`FEEDBACK_LOOP_LIMIT == 5` 逐字保持 | 参数化迭代（4 组） |
| doc 注释口径（源自证） | 正向 | `phase_table.rs` **原文**含四步序串与两处关键表述：三检查步 `on_fail=Terminal`（失败即停 / 无反馈边 / 零 agent 会话）与插入理由（`StaticCheck` 之后、`TestExecution` 之前，「代价升序 / 失败即停」一类表述）；「开相与相位收口不在 `steps` 内」+ `enter_phase` / `settle_phase` 仍在场 | 参数化迭代（4 判据） |
| doc 注释口径（源自证） | 异常 | 旧口径零残留（AC-10）：`phase_table.rs` 内「三步序」/「两检查步」/「`Clean → StaticCheck → TestExecution`」三检查步式表述 / initialize 注释「三步」四串零命中 | 参数化迭代（4 串） |
| doc 注释口径（源自证） | 边界 | 相邻段零误伤：`ARCHIVE_STEPS` 九步串、`TEST_EXECUTION_STEPS` 的 `TestExecution → MutationTest`、`ARCHI_FEEDBACK_LIMIT` 的「与 static-check 的 5 次互不占用」三处既有口径串逐字保留（改述只作用于 initialize 段） | 参数化迭代（3 判据） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 零 mock | 被测面是编译期 `const` / `static` 表与纯查表函数（`phase_table`），无 IO、无时钟、无注入依赖 | 行为行全部 |
| 不 mock：`phase_table("requirement")` / `phase_table("test-only")` | 真实静态表——指针等值与步序判据的唯真值源；以手写步序副本替代会让 AC-1 空转 | 全部步序 / 步数 / on_fail 行 |
| 文件读（源自证） | `env!("CARGO_MANIFEST_DIR")` + `std::fs::read_to_string` 读 `phase_table.rs` **原文**（既有 `phase_table_source()` 装置，不剔注释） | doc 口径三行 |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/port.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/port_test.rs

既有 sidecar（4268 行）**追加 + 改写**：`ArchiCheckMode` 的逐值对拍与封闭性、`ToolCommand::ArchiCheck` 载荷形状、`ArchiCheckRunner::run` 五参透传（`RecordingArchiCheck` 由四元组捕获改为五元组）、trait doc 措辞改述。「四参」类措辞（假件 doc / `ArchiCall` doc / 测试 doc / 测试名 / 断言消息）随断言改写一并清零。

#### 待测功能

- `ArchiCheckMode::as_str()`: 新增——出线词 `full` / `incremental`，与 `ArchiDriftMode::as_str` 同词面；零 serde / 零 specta
- `ArchiCheckRunner::run()`: 修改——`fn run(&self, root: &str, name: &str, mode: ArchiCheckMode, changed: Vec<String>, cancel: CancelFlag) -> BoxToolFuture`；`mode` 第 3 位
- `ArchiCheckMode`（类型，新增）: `pub enum ArchiCheckMode { Full, Incremental }`，derive `Debug, Clone, Copy, PartialEq, Eq`（零 serde / 零 specta / 无 `ALL`）
- `ToolCommand`（类型，修改）: `ArchiCheck { change_id: String, mode: ArchiCheckMode }`——`mode` 第 2 位；其余十变体与 derive 集零改动
- `ArchiCheckRunner`（trait doc，修改）: 「**五参显式化**」（`root` / `name` / `mode` / `changed` / 末位取消）；「报告槽位不设形参」逐字保留；补「模式由消费面声明，runner MUST NOT 推断」与「两挂载点同树落 `reports/archi`」

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ArchiCheckMode::as_str()` | 正向 | 逐值对拍：`Full → "full"`、`Incremental → "incremental"`（AC-2 唯一出线词面） | 参数化迭代（2 值） |
| `ArchiCheckMode::as_str()` | 异常 | 词面封闭负向：取值集合恒 `["full", "incremental"]`；不等于 `"Full"` / `"FULL"` / `""` / `"incremental "`（尾空格）/ `"full-mode"`（连字符）——大写 / 空串 / 空白 / 连字符五反例全不命中 | 参数化迭代（5 反例） |
| `ArchiCheckMode::as_str()` | 边界 | 两值互异且可判：`Full.as_str() != Incremental.as_str()`；`as_str` 幂等（同值连调两次等值）；`Copy` 语义下经值传递后仍可取词（不被 move 消耗） | 参数化迭代（3 判据） |
| `ArchiCheckMode`（类型） | 边界 | 封闭两值（编译期锚 + 源自证）：穷尽 `match` 缺任一臂即编译失败；`port.rs` 内 `pub enum ArchiCheckMode {` 定义恰 1 处、变体计数恰 2；该 enum 的 `impl` 块内零 `ALL` 常量（与 `ReportSlot` 同形，port 面零枚举遍历消费者） | 单次 |
| `ArchiCheckMode`（类型） | 异常 | 零 serde / 零 specta（不进 IPC）：`ArchiCheckMode` 定义邻域零 `Serialize` / `Deserialize` / `specta::Type`；`port.rs` 内零 `impl Display for ArchiCheckMode`（`as_str` 是唯一词面出口）；零 `str::parse` 回转实现（不存在「由字符串构造模式」的第二入口） | 参数化迭代（4 判据） |
| `ArchiCheckMode`（类型） | 边界 | 与判定域词面同源（跨 crate 不引依赖）：本 crate 零 `archi::` 依赖，故以字面表 `["full","incremental"]` 对拍；跨 crate 一致性由 `infra/archi` 映射单点用例 + `drift/report_test` 既有 `(Full, "full")` 对拍共同承载（本节不重复判定域断言） | 单次 |
| `ArchiCheckRunner::run()` | 正向 | `mode` 第 3 位逐值透传：捕获五元组 `(root, name, mode, changed, cancel.is_set())`，`Full` 与 `Incremental` 各一次逐值相等 | 参数化迭代（2 值） |
| `ArchiCheckRunner::run()` | 正向 | 其余四参逐值 + `changed` 保序：`root` / `name` 原样、`changed` 三形态（`[]` / 单元素 / 三元素）顺序不变 | 参数化迭代（3 形态） |
| `ArchiCheckRunner::run()` | 异常 | 取消真件透传：`cancel` 置位后假 runner 观测 `is_set() == true`（信号本身不做决策，取消语义由产出 `fault` 承载） | 单次 |
| `ArchiCheckRunner::run()` | 异常 | 产出类型漂移：假 runner 返回 `StaticCheck` 等八种非 `ArchiCheck` 变体 → `ArchiCheckOutcome::try_from` 恒 `Err` 且不 panic（错串可判，不静默取默认值） | 参数化迭代（8 变体） |
| `ArchiCheckRunner::run()` | 边界 | 零 slot 形参：源码签名行逐字为五参、`slot` 不在签名内（报告槽位恒 `ReportSlot::Archi` 由 runner 内部取用）；位次序 `root` → `name` → `mode` → `changed` → `cancel`（`mode` 必须在 `changed` 之前，与既有「变更集 + 取消末二元」读法不冲突） | 参数化迭代（2 判据） |
| `ArchiCheckRunner::run()` | 边界 | 对象安全与共享注入：`Arc<dyn ArchiCheckRunner>` 可构造、`Send + Sync`、可 `Arc::clone` 共享（无泛型方法）；`LocalToolSteps` 接收面可消费 | 单次 |
| `ToolCommand::ArchiCheck` | 正向 | 载荷可判：同 `change_id` 不同 `mode` 的两命令 `!=`（`PartialEq` 覆盖新字段，防「模式不进载荷」的静默退化）；本文件的 `describe` 穷尽 `match` 恒 `"archi-check"`（与模式无关；`walker_test` 的 `command_label` 同词面） | 参数化迭代（2 值） |
| `ToolCommand::ArchiCheck` | 边界 | 字段位次（源自证）：`port.rs` 内字面 `ArchiCheck { change_id: String, mode: ArchiCheckMode }` 在场（`mode` 第 2 位，与 `StaticCheck` / `TestExecution` 的 `slot` 同位同形）；`describe` 穷尽 `match` 仍 11 变体（新增字段不破封闭集） | 单次 |
| `ArchiCheckRunner` trait doc（源自证） | 正向 | 「**五参显式化**」在场且形参清单含 `mode`；「报告槽位不设形参」逐字保留；「模式由消费面声明」与「`reports/archi`」两处新表述在场 | 参数化迭代（4 判据） |
| `ArchiCheckRunner` trait doc（源自证） | 异常 | 旧措辞零残留：「**四参显式化**」/「捕获 `run(root, name, changed, cancel)` 四参」/「四参逐值到达」三串在本文件零命中（测试名与断言消息一并改述） | 参数化迭代（3 串） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `ArchiCheckRunner` 的录制假件（`RecordingArchiCheck`） | **不 mock**——trait 形状与对象安全性没有实现替代品：手写 trait 实现即被测面的唯一测试途径，记录元组由四元升为五元 `(root, name, mode, changed, cancel.is_set())` | 五参透传 / 取消透传 / 对象安全 |
| `ArchiCheckMode` / `ToolCommand` / `ArchiCheckOutcome` | 真值直取（`Copy` 可值传递、`PartialEq` 可逐值对拍），零替身类型 | 全部行为行 |
| 文件读（源自证） | `env!("CARGO_MANIFEST_DIR")` + `std::fs::read_to_string` 读 `port.rs` **原文**（既有 `finalized两臂穷尽且port零panic升级` 装置先例） | 封闭性 / 零 serde / 字段位次 / doc 措辞行 |
| 判定域（`archi::drift`） | 不在本 crate 引入（依赖方向单向）；词面一致性由字面表 + 下游映射单点用例承载，非 mock | 词面同源行 |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/steps.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/steps_test.rs

既有 sidecar（5652 行）**改写 + 追加**：13 处既有 `ToolCommand::ArchiCheck { change_id }` 构造点补 `mode: ArchiCheckMode::Incremental`（断言语义零改动）、1 处模式臂改 `{ change_id, .. }`、`RecordingArchiRunner` / `UnusedArchiRunner` 签名补 `mode`；新增全量臂用例（vcs 零调用 + 空变更集 + 模式透传）。

<!-- design.md「公共函数 / API」子表无本文件行（`LocalToolSteps` 的 `run` 是 `ToolStepPort` 的既有 trait 实现，签名零变化）——下列条目出处为 design「修改文件」表本文件行 + D5 + AC-3。 -->

#### 待测功能

- `LocalToolSteps::run()` 的 `ToolCommand::ArchiCheck` 臂（修改）: 按 `mode` 分派变更集——`Full` → `Vec::new()` 且 **零 vcs 调用**；`Incremental` → `vcs.change_edit_set(root, "HEAD")?`（取数失败显式传播，`MUST NOT` 降级空清单）；runner 调用点透传 `mode`；`resolve_change_name` 与审计行（`StepKind::ArchiCheck` / 摘要 `archi_check passed=<bool>` / `reference = report_dir`）逐字不动

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ArchiCheck` 臂（增量分派） | 正向 | `Incremental`：`change_edit_set(root, "HEAD")` 恰一次、清单按序注入 runner 的 `changed`（`[]` / 单元素 / 三元素三形态保序）、runner 收到 `Incremental` | 参数化迭代（3 形态） |
| `ArchiCheck` 臂（全量分派） | 正向 | `Full`：`FakeVcs` 调用计数为 0（无 `change_edit_set:HEAD` 记录）、runner 收到 `Full`、`changed` 为空集（AC-3 主判据） | 单次 |
| `ArchiCheck` 臂（全量分派） | 异常 | `Full` + 注入 vcs 取数 `Err("git 不可用")` → 步仍 `Ok`（零 vcs ⇒ 基线判定不受 git 可达性影响）、runner 收到 `Full` 与空 `changed` | 单次 |
| `ArchiCheck` 臂（增量分派） | 异常 | `Incremental` + 注入 vcs 取数 `Err` → 步 `Err("git 不可用")`、runner 零调用（`MUST NOT` 降级空清单 = 假通过）；两模式对照并列 | 参数化迭代（2 模式） |
| `ArchiCheck` 臂（name 解析） | 异常 | 未建档 change → `Err`（文案「change \"<id>\" 未建档（无 ChangeRecord），无从执行架构漂移门禁」段逐字等价）、runner 零调用（`Full` / `Incremental` 两模式同形） | 参数化迭代（2 模式） |
| `ArchiCheck` 臂（产出错型） | 异常 | runner 返回 `StaticCheck` 变体 → 臂 `Err`「archi_check 输出漂移：产出类型不匹配」且审计行落 `Err` 态（`Full` / `Incremental` 同形） | 参数化迭代（2 模式） |
| `ArchiCheck` 臂（审计行） | 边界 | 审计形状零变化：`StepKind::ArchiCheck` 归键、成功面 `status = ok`、`reference = outcome.report_dir`、摘要逐字 `archi_check passed=<bool>`（**不含** `mode` 词——审计面 schema 零改动）；`passed = true` / `false` 两态 | 参数化迭代（2 模式 × 2 态） |
| `ArchiCheck` 臂（审计行） | 边界 | 审计落行失败 best-effort：store 写失败不阻断臂业务产出，`ArchiCheck` 变体逐字段原样上抛（两模式） | 参数化迭代（2 模式） |
| `ArchiCheck` 臂（取消） | 边界 | 取消透传：`cancel` 置位 → runner 观测 `is_set() == true`，产出 `fault = Some(Stopped)` 原样上抛（不以 `Err` 表达取消；两模式） | 参数化迭代（2 模式） |
| `ArchiCheck` 臂（摘要文本有界） | 边界 | 超长诊断（> 1000 chars）不进摘要（审计摘要仍 `archi_check passed=<bool>`，逐字不含诊断正文）；错误串超长按既有截断面处理（两模式零差异） | 参数化迭代（2 模式） |
| 既有 13 处构造点（机械面回归） | 正向 | 补 `mode: ArchiCheckMode::Incremental` 后既有断言语义逐字不变（委托一次 / name 取记录名 / 变更集注入 / 取数失败 / 未建档 / 漂移 / 审计成功面 / 取消 / 有界摘要 / best-effort）——断言对象零改写即绿 | 单次（回归） |
| 组合链路（steps → vcs + runner + store） | 正向 | 一次 `Full` 命令的完整段：vcs 缝零调用 + runner 一次（`Full` + 空变更集）+ store 一行 `StepKind::ArchiCheck` 审计 + 产出透传（发起方 = `steps.rs` 的 `ToolStepPort::run`） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `ArchiveVcsPort`（`FakeVcs`） | 进程边界假缝（真实调用计数 + 清单注入 + `Err` 注入）——「全量零 vcs」的判据只能靠可计数的假缝（真实 git 无法证明「零调用」） | 全量零调用 / 增量注入 / 取数失败 |
| `ArchiCheckRunner`（`RecordingArchiRunner` / `UnusedArchiRunner`） | 端口假件：记录 `(root, name, mode, changed)` + `cancel` 序，可编程 `Err` / 产出错型 / 取消产出——被测对象是 `steps.rs` 的分派而非判定域 | 全部分派与透传行 |
| `ChangeStateStore`（`TestDb`） | **真件**（tempdir sqlite，进程边界真库）：审计行形状与 best-effort 语义必须读真库 | 审计行 / 未建档 / 落行失败 |
| 不 mock：`resolve_change_name` / `audit_outcome` | 真实内部函数——审计 shape 与未建档文案的单点即被测面，替身会让「零第二处拼装」不可见 | 未建档 / 审计行 |
| `LocalToolSteps::new` 装配 | 真实组合根（显式注入 archi runner + vcs 双缝，`assemble_with_archi` 先例），零测试专用构造分支 | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs

既有 sidecar（21550 行）**改写 + 追加**：`NullArchiRunner` / `CountingArchiRunner` 签名补 `mode`；`FakeTools` 的 `ToolCommand::ArchiCheck` 臂补字段；initialize 全绿链三节点 → 四节点（emit 序 / 边数 / 命令载荷 `mode=Full`）；新增 archi 红 / `skipped` / `noChanges` / 取消 / `Escalate` 漂移 / 文案四条对拍 / 零改写五类用例。

<!-- design.md「公共函数 / API」子表无本文件行（`step_archi_check*` 与 `TXT_BASELINE_ARCHI_FAILED` 均为 crate 内私有面）——下列条目出处为 design「修改文件」表本文件行 + D6 / D7 / D8 / D11 + AC-3 / AC-5 / AC-6 / AC-7 / AC-10。 -->

#### 待测功能

- `step_archi_check()`（修改）: 按 `step.on_fail` 三分派——`Terminal` → `step_archi_check_terminal`；`FeedbackLoop { limit }` → `step_archi_check_feedback`；`Escalate` / `None` → `Err(Terminal::failed("计划图漂移：phase \"…\" 的 archi-check 步缺 OnFail::Terminal / FeedbackLoop 声明"))`（不静默 `Proceed`）
- `step_archi_check_terminal()`（新增）: 单发终态（fault 先行 / `passed` 收口 / failed `Terminal`）——命令携 `mode: Full`、`identity: None`（节点 `output_ref` 恒 `null`）；passed → `StepSettle::passed(diagnose_brief(&diagnostics))` + `Proceed`；failed → `StepSettle::failed(diagnose_brief(…))` + `Terminal::failed(TXT_BASELINE_ARCHI_FAILED.replace("{dir}", &report_dir))`
- `step_archi_check_feedback()`（搬移 + 唯一改动）: 既有反馈边体逐字（fault 不烧预算 / `passed` 收口 / 失败进同会话修复 / 超限代写 fail 落账升格），命令携 `mode: Incremental`
- `TXT_BASELINE_ARCHI_FAILED`（新增常量）: 「基线架构检查未通过，请在基线上修复模型或代码后重新发起（基线的债不属于本次变更）；报告目录 {dir}」
- initialize 相位词汇区注释与相位内计划步执行体区段注释（doc-only，修改）: 词汇区「baseline 三态失败文案」→「baseline 四条失败文案（static / archi / test Error / test Fail，四条互不共用；`{dir}` 为各步报告目录占位）」；区段注释「三步序 / 两检查步」→ 四步序 + 三检查步

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `step_archi_check()` 分派 | 正向 | `Terminal` 声明（initialize 表项）→ 走单发终态体：一次 `run_tool`、零反馈边、零 `WorkerAgent` 会话、单节点树（`instance_no = 0`） | 单次 |
| `step_archi_check()` 分派 | 正向 | `FeedbackLoop { limit: 3 }` 声明（implement 表项）→ 既有反馈边体逐字：失败一次 + 复查一次 = 2 次命令、1 条 `feedback` 边、`instance_no` 0→1 递增、修复 prompt 含「第 1/3 次」+ 违例明细（回归） | 单次 |
| `step_archi_check()` 分派 | 异常 | `Escalate` 误声明 → `Err` 且文案含 `phase` 名与「archi-check 步缺 `OnFail::Terminal` / `FeedbackLoop` 声明」；runner 零调用、零 `archi_check` 节点、零反馈边（R-8 守卫） | 单次 |
| `step_archi_check()` 分派 | 异常 | `on_fail = None` 误声明 → 同上漂移 `Err`（不得退化为 `Proceed`、不得由 `feedback_limit` 兜底成「缺 FeedbackLoop 声明」文案） | 单次 |
| `step_archi_check()` 分派 | 边界 | `FeedbackLoop { limit: 0 }` → 首败即升格相位 fail（不无限循环、不注入修复会话）；升格代写 fail checklist 一条（`item = "架构检查"`） | 单次 |
| `step_archi_check()` 分派 | 边界 | `FeedbackLoop { limit: u32::MAX }` → 首败进入反馈边（计数 `1/u32::MAX` 不溢出、不 panic）；`limit = 5`（static 档）不改变 archi 计数语义（三段计数互不占用，回归） | 参数化迭代（2 档） |
| `step_archi_check_terminal()` | 正向 | `clean`（`passed = true`，诊断含 `mode=full` / `status=clean` / 扫描文件数）→ 节点 `passed`、`detail` 为 `diagnose_brief` 压缩后的诊断串、`output_ref` 恒 `None`、`Proceed`（下一步 `test_execution` 照常执行） | 单次 |
| `step_archi_check_terminal()` | 正向 | `warnings`（`passed = true`）→ 节点 `passed` + `Proceed`（基线不另立更严判据：与 implement 门禁同一 `passed` 定义） | 单次 |
| `step_archi_check_terminal()` | 异常 | `errors`（含 `model.structuralErrors`，`passed = false`）→ 节点 `failed`、`detail` 有界、run `Terminal::failed` 且 reason 逐字含「基线架构检查未通过」+「基线的债不属于本次变更」+ `reports/archi`（`{dir}` = `outcome.report_dir`）；`test_execution` 步零执行 | 单次 |
| `step_archi_check_terminal()` | 异常 | `fault = Some(Stopped)` → 节点 `stopped` + run `stopped`（`Terminal::stopped("用户请求停止")`），**不落**基线红文案、零升格、零反馈边 | 单次 |
| `step_archi_check_terminal()` | 异常 | `fault = Some(Environment(..))` 同形保留臂：`template = None` ⇒ 节点 `failed` 的 detail 与 run reason 均为 fault 原文（零新增环境文案常量，与 implement 步 archi 臂同形） | 单次 |
| `step_archi_check_terminal()` | 边界 | `report_dir = ""`（空串占位）→ failed 文案仍成型（`{dir}` 替换为空串，不 panic、不留 `{dir}` 字面）；`report_ref = None` 不影响结论（写盘降级面） | 单次 |
| `step_archi_check_terminal()` | 边界 | `diagnostics` 超长（> 1000 chars，含 40 条聚合违例形态）→ 节点 `detail` ≤ 200 字符且以 `…` 收尾（`diagnose_brief` 单点）；`Terminal` 文案**不含**诊断正文（只含固定引导 + `{dir}`） | 单次 |
| `step_archi_check_terminal()` | 边界 | `diagnostics` 特殊字符（`\n` / `\0` / emoji）→ `detail` 压空白为单行（`diagnose_brief` 不截断短串）；`skipped` / `noChanges` 两个短串**逐字**透出不被改写 | 参数化迭代（3 形态 + 2 逐字串） |
| `step_archi_check_terminal()` | 边界 | `skipped`（无模型 → `passed = true`）→ 节点 `passed`、`detail` 逐字「未检查（无模型）」、`Proceed`——MUST NOT 写成「检查通过」（R-7 反向钉住） | 单次 |
| `step_archi_check_terminal()` | 边界 | `noChanges`（零可判文件，全量亦可发生 → `passed = true`）→ 节点 `passed`、`detail` 逐字「未检查（无可判文件）」、`Proceed`（R-9） | 单次 |
| `step_archi_check_terminal()` | 边界 | 零自动改写（AC-6）：`clean` / `warnings` / `errors` 三态各跑一次后，workspace 源码与模型文件字节快照逐字相等（`三态跑动后源码字节零变化` 形态，initialize 侧复刻） | 参数化迭代（3 态） |
| `step_archi_check_feedback()` | 正向 | 命令携 `mode = Incremental`（`FakeTools` 捕获的命令载荷逐值断言）；其余反馈行为与改述前逐字等价（诊断注入同会话 / 反馈文本 `diagnose_brief` 两百字符 / 超限升格不跑 evaluator） | 单次 |
| `TXT_BASELINE_ARCHI_FAILED` | 正向 | 文案逐字三要素：含「基线架构检查未通过」、「基线的债不属于本次变更」（与 static / test 三条同族引导）、`{dir}` 占位；替换后不含残留 `{dir}` / 不含 `{fault}` 占位（与 `TXT_ENV_*` 零共用引导句） | 参数化迭代（4 判据） |
| `TXT_BASELINE_ARCHI_FAILED` | 异常 | 四条文案互不相同（AC-10）：`TXT_BASELINE_STATIC_FAILED` / `TXT_BASELINE_ARCHI_FAILED` / `TXT_BASELINE_TEST_ERROR` / `TXT_BASELINE_TEST_FAILED` 两两 `!=`（同一 `{dir}` 值替换后仍互不相同） | 参数化迭代（6 对） |
| `TXT_BASELINE_ARCHI_FAILED` / 词汇区注释 | 边界 | 常量定义面恰四条 `const TXT_BASELINE_`（源自证，`-A2` 逐条比对）；词汇区注释改述为「四条」且不再自称「三态」（doc-only 面判据） | 单次 |
| initialize 四步链（组合：相位表真件 → 相位机） | 正向 | 全绿链 emit 序 `["clean", "static_check", "archi_check", "test_execution"]`；forward 边 3 条逐位端点（`initialize:1:<word>:0`）；四节点恒 `passed`；零 `phase_start` / `phase_log` 节点；零 agent 会话；PhaseRecord 落 `pass` | 单次 |
| initialize 四步链（组合：相位表真件 → 相位机） | 正向 | 命令载荷面：static / test 两步仍携 `slot = Baseline`；archi 步携 `mode = Full` 且无 `slot` 字段（构造点分派单源）；检查步 `root` 恒 exec root（worktree） | 参数化迭代（3 步） |
| initialize 四步链（组合：相位表真件 → 相位机） | 边界 | 收口判据不受新步影响：机械 checklist 仍三条目（suite 结论一致 / 聚合计数一致 / mutation null 自动通过）、报告串携 conclusion 计数与 baseline 报告目录、三会话槽位恒 `None`；全绿后持久跳过语义不变（二次发起不再进 initialize） | 参数化迭代（4 判据） |
| initialize 四步链（组合：相位表真件 → 相位机） | 边界 | legacy 短路（`worktree = None`）：整相位跳过、`archi_check` 步零执行、零 `ToolCommand::ArchiCheck` 命令、零判定、`PhaseRecord.skipped = true`（新步随相位短路，零特判） | 单次 |
| initialize archi 红链（组合） | 异常 | `reports/archi` 报告目录携带面：failed 节点 `output_ref` 恒 `None`（架构报告非 exec-outputs，不经 `outputRef`）；`report_ref` 只体现于 detail 文本 | 单次 |
| initialize archi 红链（组合） | 异常 | 零会话 / 零反馈边 / 零改写（AC-6）：`WorkerAgent` 请求数 0（executor / evaluator / decision 三槽全空）、`edges` 内零 `feedback` 边、`PhaseRecord` 零 `initialize` 条目、源码与模型字节零变化 | 参数化迭代（4 判据） |
| initialize archi 红链（组合） | 边界 | 一次性语义与重试：archi 红 `Terminal` 收敛后再发起 → 仍路由 initialize 且 `Clean` 先恢复基线、archi 复查一次（干净重试，不继承前次失败） | 单次 |
| initialize archi 红链（组合） | 边界 | `run_tool` 返回 `Err`（看护配置非法 / 扫描硬失败）→ 直达 `Terminal`，reason 前缀「archi-check 失败: <内层文案>」，不落基线红文案、不代写 fail checklist（与 initialize 另两步 `Err` 臂同形，R-6） | 单次 |
| 相位内计划步执行体区段注释（源自证） | 正向 | `walker.rs` 代码行外注释含四步序与「三检查步 `on_fail=Terminal`」表述；「两检查步」在该文件零命中（AC-10 本文件面） | 参数化迭代（3 判据） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `ToolStepPort`（`FakeTools`） | 端口假写面：预录 `ArchiCheckOutcome` 序列（`with_archi_check`）+ 捕获 `ToolCommand` 序与载荷 + 可编程 `Err`——被测对象是执行体分派与收口，判定域不在本层 | 三态收口 / 漂移 / 取消 / 升格 |
| `WorkerAgentPort`（`FakeWorker`） | 端口假件：记录会话请求（角色 + prompt）与 session 登记——「零会话」判据只能经可计数假件（真实 agent 不可用于 AC-6） | 零会话 / 反馈注入 / 零注入 |
| `ArchiveVcsPort`（`FakeVcs`） | 端口假件：`restore_baseline` 载荷捕获（Clean 步排除面）+ 调用计数 | legacy 短路 / Clean 恢复 / red→重试 |
| `ChangeStateStore`（`TestDb`） | **真件**（tempdir sqlite）：PhaseRecord 零条目 / 落账语义必须读真库 | 红链 / 重试 / 全绿落账 |
| `WorkflowSnapshotPort`（`StubSnapshot`） | 既有快照桩（只读面，零行为） | 全部运行用例 |
| 不 mock：`phase_table` / `steps_of` | 真件克隆步序——四步链的序判据必须经真表，手写步序副本会让 AC-1 / AC-7 空转 | 四步链全部行 |
| 不 mock：`settle_fault` / `diagnose_brief` / `emit_node` | 真实收口单点（fault 先行、detail 有界、节点落包即判据面） | 取消 / 环境错 / detail 边界 |

---

### packages/desktop/src-tauri/crates/core/workflow/src/state.rs -> packages/desktop/src-tauri/crates/core/workflow/src/state_test.rs

既有 sidecar（2175 行）**零行为改写 + 追加源自证**。本文件仅两处 doc 改动，既有词汇冻结 / 22 值 / 序 / 线词断言零改写即回归判据。

<!-- design.md「公共函数 / API」子表无本文件行；「修改文件」表本文件行明写「仅两处 doc：`OnFail::Terminal`、`StepNode::ArchiCheck`；`RunStepKind` 与 `StepKind` 及其 doc 逐字不动」。下列条目出处为该行 + design D11 ③④ + AC-10。 -->

#### 待测功能

- `OnFail::Terminal`（doc-only，修改）: 消费点由「initialize 相位两检查步 StaticCheck / TestExecution」改述为「initialize 表头三检查步（StaticCheck / ArchiCheck / TestExecution）」；「基线绿门失败即停 / 无反馈边 / 零 agent 会话」语义句保留
- `StepNode::ArchiCheck`（doc-only，修改）: 「架构漂移判定，定向反馈边、零进程」改述为「按 `on_fail` 两态：`FeedbackLoop`（implement）/ `Terminal`（initialize），零进程」
- `RunStepKind` / `StepKind`（零改动冻结面）: 22 值词汇、变体序与 doc 逐字不动（`RunStepKind` doc 直出 `bindings.ts`，动它即制造生成物 diff）；`StepKind::ArchiCheck` 的「定向反馈边」措辞保留

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `OnFail::Terminal` doc（源自证） | 正向 | `state.rs` **原文**中 `Terminal` 变体邻域含 `ArchiCheck`，且三检查步三名齐（`StaticCheck` / `ArchiCheck` / `TestExecution` 同段命中）；「无反馈边」与「零 agent 会话」语义句仍在场 | 参数化迭代（3 判据） |
| `OnFail::Terminal` doc（源自证） | 异常 | 旧口径零残留：「两检查步」在 `state.rs` 零命中；`Terminal` doc 不得出现「仅 StaticCheck / TestExecution 两步」类二步式表述 | 参数化迭代（2 串） |
| `StepNode::ArchiCheck` doc（源自证） | 正向 | `StepNode::ArchiCheck` 变体邻域同段含 `FeedbackLoop` 与 `Terminal` 两态词（两挂载点语义），并保留「零进程」；「定向反馈边」不再作为该步的**唯一**语义（不再出现独占式表述） | 参数化迭代（3 判据） |
| `StepNode::ArchiCheck` doc（源自证） | 边界 | 相邻变体零误伤：`StepNode::StaticCheck` doc 的「定向反馈边」、`OnFail::FeedbackLoop` doc 的「定向反馈边」仍各自在案（改述只作用于 `ArchiCheck` 那一条） | 参数化迭代（2 处） |
| `RunStepKind` 冻结面（源自证） | 正向 | `RunStepKind` 的 22 值 doc 段落零改动：词汇序、`archi_check` 词条原文与「定向反馈边」措辞逐字保留（AC-9 绑定零 diff 的源码侧证） | 单次 |
| `StepNode` / `StepKind` 语义回归 | 正向 | 既有断言零改写即绿：`StepNode` 22 值 snake 线词逐字且序不变、`as_str()` 与 `RunStepKind` 同源、`StepKind` 9 值词面（含 `archi_check`）、`StepGroup` 分组计数不变 | 参数化迭代（4 组） |
| `StepNode` / `StepKind` 语义回归 | 边界 | 「注释不改语义」侧证：doc-only 改动后 22 值 / 序 / 线词 / 分组四类断言全绿；非法线词反序列化仍拒绝（封闭集防线零改动） | 参数化迭代（2 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 零 mock | 被测面是封闭枚举 + 派生访问器 + doc 原文，无 IO、无端口、无时钟 | 全部行为行 |
| 不 mock：`StepNode` / `StepKind` / `RunStepKind` 真值直取 | 枚举本体即被测对象；字符串表替代会让 22 值 / 序自证失效 | 语义回归行 |
| 文件读（源自证） | `env!("CARGO_MANIFEST_DIR")` + `std::fs::read_to_string` 读 `state.rs` **原文**（既有 `词汇冻结_exec组doc…` 装置先例，不剔注释） | doc 口径三行 |

---

### packages/desktop/src-tauri/crates/core/foundation/src/layout/mod.rs -> packages/desktop/src-tauri/crates/core/foundation/src/layout/mod_test.rs

既有 sidecar（1734 行）**零行为改写 + 追加源自证**。本文件仅 doc 改动（`ReportSlot` 类型 doc 补一句、`Baseline` 变体 doc 收敛），三值封闭集与 `report_dir_name` 映射零改动。

<!-- design.md「公共函数 / API」子表无本文件行；「修改文件」表本文件行明写「仅 doc：`ReportSlot::Baseline` 变体注释收敛 + `ReportSlot` 类型 doc 补一句；逻辑逐字不动」。条目出处 = 该行 + D11 ⑤ + AC-5 / AC-10。 -->

#### 待测功能

- `ReportSlot`（类型 doc，doc-only 修改）: 补「`Archi` 由 initialize 基线检查与 implement 门禁两挂载点共用」一句；「checks 两 runner MUST NOT 接受该槽位」纪律句保留
- `ReportSlot::Baseline`（变体 doc，doc-only 修改）: 「initialize 相位两检查步」→「initialize 相位 static-check / test-execution 两步（archi 基线检查走 `Archi` 槽位）」

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ReportSlot` 类型 doc（源自证） | 正向 | `layout/mod.rs` **原文**类型 doc 含「两挂载点共用」类表述（initialize 基线检查 + implement 门禁）与 `Archi` 槽位语义；`crates/infra/checks` 「MUST NOT 接受该槽位」纪律句仍在场 | 参数化迭代（3 判据） |
| `ReportSlot::Baseline` doc（源自证） | 异常 | 旧口径零残留：「initialize 相位两检查步」在 `mod.rs` 零命中；`Baseline` 变体 doc 不得把 archi 基线检查挂到 `baseline` 槽位（反向断言：变体 doc 邻域含 `Archi` 槽位指引） | 参数化迭代（2 判据） |
| `ReportSlot` 语义回归 | 正向 | 三值封闭集与映射零改动：`Test` / `Baseline` / `Archi` 三值，`report_dir_name` 分别 `"test"` / `"baseline"` / `"archi"`，`change_reports(root, name, slot)` 三段路径互不相等且尾段单段（既有断言零改写即绿） | 参数化迭代（3 值） |
| `ReportSlot` 语义回归 | 边界 | doc-only 侧证：改动后 `layout` 既有全量用例（路径派生族 / 相对路径族 / `"api"` 字面量纪律）全绿；`report_dir_name` 源码映射分支数不变（三值穷尽 `match`，零新增第四臂） | 参数化迭代（2 判据） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 零 mock | 纯路径派生函数 + 封闭枚举 + doc 原文，无 IO（除源自证读文件）、无时钟 | 全部行 |
| 不 mock：`change_reports` / `report_dir_name` | 真实单点（槽位语义的唯一实现），替身会让「三值映射零改动」不可见 | 语义回归行 |
| 文件读（源自证） | `include_str!("mod.rs")`（既有 `api_dir_name零字面量纪律` 装置先例） | doc 口径两行 |

---

### packages/desktop/src-tauri/crates/infra/archi/src/runner.rs -> packages/desktop/src-tauri/crates/infra/archi/src/runner_test.rs

既有 sidecar（1278 行）**改写 + 追加**：`run(...)` 测试辅助函数补 `mode` 实参（既有 3 处 `.run(` 调用 + 1 个 helper）、「恒增量模式」（`:337`）断言改述为按入参模式、新增全量落盘用例族与映射单点源自证。

#### 待测功能

- `impl ArchiCheckRunner for LocalArchiCheck::run()`: 修改——收 `mode`（第 3 位），映射到 `ArchiDriftMode` 后调 `drift::check(CheckInput { mode: drift_mode(mode), changed: &changed, … })`；落盘 / 降级 / 取消 / `compose_text` / `passed` 派生逐字不动
- `drift_mode()`（私有映射，新增；出处 = design D10，非公共 API 表行）: `Full → ArchiDriftMode::Full`、`Incremental → ArchiDriftMode::Incremental`，crate 内唯一映射点；`changed` 不在 runner 侧清空（全量忽略变更集是判定域既有语义）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `run()`（全量） | 正向 | 真实模型 + `changed = []` + `Full` → 报告落 `reports/archi/<stem>.json`（`report_ref` 即该文件且在盘）、报告 `mode = full`、`base = None`、`change = Some(name)`、`scan.changed_files = 0`、`scan.scanned_files` = 全根可判文件数（> 0）| 单次 |
| `run()`（全量） | 正向 | `passed` 随结论派生：`clean` → `passed = true`；`warnings`（warn 档违例）→ `passed = true`；`errors`（strict 违例 / 结构错）→ `passed = false`（三态 × `passed` 组合） | 参数化迭代（3 态） |
| `run()`（全量） | 正向 | 落盘报告与同输入 `drift::check(CheckInput { mode: Full, … })` 直出逐字段相等（仅 `timestamp` 允许不同）——全量不引入第二套判定 | 单次 |
| `run()`（增量） | 正向 | 「恒增量模式」断言改述：`Incremental` + `changed = ["src/billing/x.ts"]` → 报告 `mode = incremental`、`base = Some("HEAD")`、`scan.changed_files = 1`、`scanned_files = 1`；与直出判定逐字段相等 | 单次 |
| `run()`（全量） | 异常 | `Full` + 非空 `changed`（含变更集外的违例文件）→ 变更集被忽略：`changed_files = 0`、扫描全根、变更集内外的违例照报（全量不看编辑集） | 单次 |
| `run()`（全量） | 异常 | `skipped`（无 `<archi_dir>/models`）→ `passed = true`、`fault = None`、detail 逐字「未检查（无模型）」、报告 `status = skipped` 且 `mode = full`（全量下同样可能出现） | 单次 |
| `run()`（全量） | 异常 | `noChanges`（模型在但观察根内零可判文件，如源码全被排除）→ `passed = true`、detail 逐字「未检查（无可判文件）」、`scan.scanned_files = 0` | 单次 |
| `run()`（全量） | 异常 | `cancel` 预置 → `Ok(outcome)` 且 `fault = Some(RunFault::Stopped)`、`report_ref = None`（取消不以 `Err` 表达） | 单次 |
| `run()`（全量） | 异常 | 写盘失败降级：`reports/archi` 位置被普通文件占位 → `report_ref = None`、`passed` 结论保留、detail 含违例明细（降级不阻断结论） | 单次 |
| `run()`（两模式） | 异常 | `Err` 面（看护配置非法 / 扫描硬失败）两模式同形：返回 `Err(<内层文案>)`，不产部分结论、不落报告 | 参数化迭代（2 模式） |
| `run()`（全量） | 边界 | `name` 边界：`""` / 超长（> 1000 chars）/ 特殊字符（`\n` / emoji）→ 报告目录仍在 `root` 内、文件名主干取 `file_stem(timestamp)`（与增量同规则），不 panic | 参数化迭代（3 形态 × 2 模式） |
| `run()`（全量） | 边界 | 槽位隔离：一次全量跑动后 `reports/test` / `reports/baseline` 零创建（`reports/archi` 恰一个文件） | 单次 |
| `run()`（全量） | 边界 | 同树共居（R-3）：同 root 同 name 先 `Full` 后 `Incremental` → 两个报告文件并存于同一 `reports/archi` 目录，各自身自述 `mode`（不因同目录而互相覆盖或串档） | 单次 |
| `run()`（两模式） | 边界 | 报告字段面零改动：反序列化后 `version == 3`、14 字段 + `apiFindings` 槽位齐备（全量报告与增量报告同形状） | 参数化迭代（2 模式） |
| `drift_mode()` | 边界 | 映射单点（源自证）：`runner.rs` 内 `fn drift_mode(` 定义恰 1 处、`mode: drift_mode(mode),` 调用恰 1 处、`ArchiDriftMode::` 在 `run` 体内零出现；`changed.is_empty()` / `if changed` 类分支零命中（runner 不做第二套模式判定，D10） | 参数化迭代（4 判据） |
| `run()`（回归） | 正向 | 既有断言零改写即绿：`clean全字段` / `warnings档通过` / `errors档失败` / `结构错拦截` / `报告与判定同源`（改述后）/ `变更集注入`（增量形态）/ `反馈文本有界` / `超长单条message` / `写盘失败降级` / `取消落fault` / `生产路径fault恒None且零进程` / `零第二套判定` / `构造面等价` / `根边界` / `槽位隔离` | 参数化迭代（既有用例集） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 零 mock（判定域） | 真实 `archi::drift::check`——判定域即被测语义（「判定单源」）；替身会让「全量 = 同一判定 + 另一口径」不可判 | 全部结论面行 |
| 零 mock（盘面） | 真实 `tempfile::TempDir` + 真实 `std::fs` 写模型 / 源码 / 报告；失败面以「目录位置普通文件占位」注入（进程边界真实、零桩） | 全量落盘 / 写盘降级 / 槽位隔离 |
| 零 mock（执行器） | crate 零 tokio 依赖 ⇒ 既有手写 noop-waker `block_on` 推进 future（新用例沿用同一装置，不引入 dev-dependency） | 全部 `run` 调用 |
| 文件读（源自证） | `include_str!("runner.rs")`（既有 `零第二套判定` / `实现体与常量零新增分支` 装置先例） | 映射单点行 |
| 时钟 | 不 mock：`timestamp` 只用于文件名与报告字段，用例以「文件存在 + `read_report` 反序列化」间接判（`report_test` 已单测文件名规则） | 全量落盘行 |

---

## 不可测试项

| 模块 | 原因 | 替代验证 |
|---|---|---|
| `packages/desktop/package.json`（`version` `0.6.47 → 0.6.48`） | 配置 / 元数据面，无运行期行为可断言；判据是**相对递增**（MUST NOT 写死字面号） | 交付门 diff 审查：`git diff -- packages/desktop/package.json` 版本行恰 +1 patch；`src-tauri/tauri.conf.json` 仍为 `"version": "../package.json"`（零手改）；`src-tauri/Cargo.toml` crate 版本行零改动（AC-11） |
| `packages/desktop/src/types/generated/{bindings.ts, bindings.zod.ts}` | specta 生成物零手改；本变更零新命令 / 零新出线类型（`ArchiCheckMode` 不进 specta），`RunStepKind` / `ChangeStepKind` doc 明令不动 ⇒ 再生成应零 diff（AC-9） | `pnpm -C packages/desktop run bindings:export` 后 `git diff` 为空（桌面端 static-check）；本设计的源码侧证落 `workflow/state.rs` 章节的「`RunStepKind` 冻结面」行 |
| `.dev-team/architecture/**`（模型 / 契约 / `INDEX.toml` / `config.json`） | 零元素 / 零关系 / 零契约改动（判定域与契约面出 range）；`api:check` 应零差异（AC-9、design 事实校准 7） | `git diff --stat .dev-team/architecture` 为空 + `pnpm -C packages/desktop run api:check` 零差异（桌面端执行） |
| `packages/desktop/src/views/changes/flow/**`（含 `run-state.ts` / `run-step-node.tsx`） | 前端**零代码改动**：`archiCheck` 的 `STEP_LABEL` / 归一 / 分组徽章已在案，新节点直接吃既有渲染 ⇒ 不新增前端用例（改写即违反 AC-9） | `git diff -- packages/desktop/src` 为空；`desktop-change-flow-view` 的「initialize 节点由三变四」由 `walker_test` 的四节点 emit 序用例（后端事实源）+ delta spec 文本承载 |
| `packages/desktop/src-tauri/src/commands/change_flow/mod.rs`（装配面） | 组合根零改动：`LocalToolSteps::new` 的四 runner / vcs / store / run_id 形参与实参不变（第四 runner 仍是 `LocalArchiCheck`）；改它即偏离提案 | `git diff` 为空 + `cargo clippy --workspace --all-targets` 绿（签名变更已在 port 与 infra 两侧闭合，装配面无需跟随） |
| `packages/desktop/src-tauri/crates/infra/archi/src/{report.rs, api_artifacts.rs}` | 落盘单点与文件名规则「逐字不动」（design D9 / D12）；两挂载点同树共居由 `runner_test` 的「同树共居」用例间接覆盖 | `git diff` 为空 + 既有 `report_test` / `api_artifacts_test` 全绿 |
| `packages/desktop/src-tauri/crates/core/archi/src/{drift,scan,dto}/**`（判定域） | 判定规则（扫什么 / 归属 / 定级 / 豁免 / 报告形状）零改动；出 range 面 | `git diff` 为空 + 既有 `drift/mod_test` / `drift/config_test` / `drift/report_test` / `scan/mod_test` / `scan/owner_test` 全绿 |
| `packages/desktop/src-tauri/crates/infra/checks/src/mutation/mod_test.rs` 的 `UnusedArchiSeam::run` | **测试文件内的组合根占位假件**（该臂恒 `Err`），非产品代码；本变更只补 `mode` 形参（checks 域实现零改动） | `cargo check --all-targets` / `cargo clippy --workspace --all-targets` 绿（漏改即编译红）；该假件承载的既有用例断言零改动即绿 |
| `ArchiCheckRunner` 的其余机械闭合面（`walker_test` 的 `NullArchiRunner` / `CountingArchiRunner`、`steps_test` 的 `UnusedArchiRunner` / `RecordingArchiRunner` / 13 处构造点 / 1 模式臂、`port_test` 的 `RecordingArchiCheck` / 2 处 UFCS 调用 / 1 模式臂、`runner_test` 的 3 处 `.run(` + helper） | 纯编译闭合（只补 `mode` 形参 / 字段 / 实参位），零用例语义改动——没有「新行为」可设计用例 | 各章节用例表内的「既有断言零改写即绿」行（回归面）+ 编译门禁（漏一处即 `cargo check` 红，R-4） |
| 穷尽 `match` / `ArchiCheckMode` 封闭性（AC-2 的编译期半边） | 编译期事实（`match` 非穷尽即编译失败、枚举新增变体即触发未覆盖臂），无运行期断言面 | `cargo clippy --workspace --all-targets` 绿；运行期半边由 `port_test` 的逐值对拍 + 源自证（变体计数 2 / 零 `ALL`）承载 |
| AC-8 本仓基线实证（全量判定 `status = clean` / `scannedFiles = 396` / `by_gate` 全 `off`） | 提案明示「不作为 MUST，登记留痕」；依赖本仓真实模型与 `archi_drift` 命令的现场输出，非稳定单测面（元素 / 关系数随仓演进） | 手工全量判定（`archi_drift` 命令 full 模式）输出留痕进 run 记录；本变更内不设断言 |
| AC-11 门禁链（`pnpm -C packages/desktop run check`、`cargo nextest run --workspace`、`pnpm api:check`） | 机械门禁由桌面端 static-check / test-execution 相位执行，agent 内不发起（阶段约束） | 桌面端执行上述命令并取退出码；本设计的用例即其承托面 |
| 跨文件「口径串零残留」（AC-10 的 `grep -rn '两检查步' crates/` 终态） | 源码位置事实的**跨文件**部分运行期不可观测；同文件内的在场 / 缺席事实已由各章节源自证用例承载 | 桌面端 static-check 的 `grep` 判据（`grep -rn "两检查步" crates/ --include=*.rs \| grep -v "_test.rs"` 零命中）；测试文件侧的同类串随本设计各章节断言改写一并清零 |
| `openspec/specs/**` 与 `openspec/changes/init-check-archi/**`（proposal / design / tasks / 6 份 delta spec 文本） | 规格与设计文档，非可执行代码；本相位只消费不验证 | 合并评审（spec 与实现的 D11 改述清单逐条对照） |
