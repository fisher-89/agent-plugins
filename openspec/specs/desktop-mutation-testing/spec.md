# desktop-mutation-testing Specification

## Purpose

定义桌面突变测试（mutation testing）契约：以 change 编辑集（`change_edit_set`）为突变范围唯一单源，js 族 Stryker / rust cargo-mutants 分框架执行与 outcome 映射、score 口径与逐 suite 阈值判定、分段顺序执行与 sandbox 跨段复用、双时点报告指引式反馈边、段级复用门与断点续跑。

## Requirements

### Requirement: 突变范围清单封装（change_edit_set 单点）

桌面突变范围 SHALL 以 change 编辑集为唯一权威：vcs 边界 SHALL 单点封装 `change_edit_set(root, base) -> Vec<String>`——跟踪改动取 `git diff <base> --name-only -z`（worktree 语义下 base=HEAD，恰为本 change 编辑集）、untracked 面经 `git ls-files --others --exclude-standard -z` 并入（新建源文件不 add 也入清单）、排序去重并 POSIX 归一后返回。该封装是**突变范围与文件清单的唯一入口**：`MutationRunner::run(root, name, files)` 收清单入参，清单获取经 vcs 缝注入（实现落 `crates/infra/vcs`，port 归消费者），checks-runtime 依赖面 MUST NOT 扩 vcs；全仓 MUST NOT 存在第二处取 diff / 反推突变文件的触点（CLI 侧清单反推逻辑不移植、不动）。

#### Scenario: 清单恰为编辑集含 untracked

- **WHEN** worktree 内一 change 编辑了 3 个已跟踪文件并新建 1 个未 add 的源文件，调用 `change_edit_set(root, HEAD)`
- **THEN** 返回恰为 4 个文件路径（跟踪 3 + untracked 1），字典序去重、POSIX 分隔符归一；无 net-zero 去噪、无 change 清单反推

#### Scenario: 唯一入口多消费者同源

- **WHEN** MutationRunner 与未来其他消费者（如 diff 摘要）各自需要变更文件清单
- **THEN** 两者均经 `change_edit_set` 取数（同源同口径）；infra/vcs 之外无 `git diff --name-only` / `ls-files --others` 清单拼装逻辑

### Requirement: 突变执行框架矩阵与映射表

突变执行 SHALL 按框架分派执行器：js 族（jest / vitest / vite-plus）经 Stryker（无 in-diff，`mutate` 面 = `change_edit_set` 清单，沿 CLI `restrictMutationScope` 同款口径）；rust 经 cargo-mutants（`--in-diff` 行级精度优先，untracked 面经 `git add -N` 或 `--file` 补充，形态 design 定稿；`--json` 输出；`--baseline skip`——TestExecution 绿跑已保证基线）。突变并发 SHALL 为一半 CPU（`--jobs = available_parallelism()/2`，延续 CLI 现状口径）并配 per-mutant timeout 护栏（防单突变体挂死，与分段预算正交）。cargo-mutants outcome 映射 SHALL 定案为：`caught→killed / missed→survived / timeout→timeout / unviable→compileError / not-mutated→ignored`。无突变执行器的框架（node-test 等）SHALL 跳过该 suite 突变并记 evidence，MUST NOT Err、MUST NOT 静默无痕。

#### Scenario: cargo-mutants 映射表口径

- **WHEN** cargo-mutants 报告含 caught / missed / timeout / unviable / not-mutated 五类 outcome
- **THEN** 逐类映射为 `MutationMeasured` 的 killed / survived / timeout / compileError / ignored（unviable MUST NOT 映射 ignored——分母排除口径与 Stryker compileError 同语义）

#### Scenario: 无执行器框架跳过留痕

- **WHEN** tests[] 声明 node-test suite 且其文件入突变范围
- **THEN** 该 suite 突变面跳过并记 evidence（无突变执行器），runner 不 Err、不静默；其余 suite 突变照常执行

### Requirement: score 口径与逐 suite 阈值判定

突变得分 SHALL 沿既有公式：`score = (killed + timeout) / (total - ignored - compileError - runtimeError)`（分母排除 ignored + compileError + runtimeError；分母为 0 → 100.0），MUST NOT 引入百分比平均等新口径。阈值 SHALL 取 suite 配置 `mutation.score`（`openspec/config.json` 既有字段：desktop 80 / src-tauri 70）。门禁结论 SHALL 为逐 suite 判定、全过即 pass（任一 suite `score < threshold` 即 fail）。分段 / 多 suite 计数归并 SHALL 沿 `aggregate_mutation` 既有计数求和公式（段当子报告，MUST NOT 新增归并公式）；归并 MutationBlock SHALL 回填 `summary.json` 的 `mutation` 位（在位时），未跑 / diff 空跳过时位保持 null（null 合法观察期语义不变，见 desktop-test-execution）。

#### Scenario: 公式与分母排除

- **WHEN** 某 suite 突变计数为 killed=80 / survived=10 / timeout=5 / ignored=3 / compileError=2 / runtimeError=0 / total=100
- **THEN** score = (80+5)/(100-3-2-0) = 89.47（两位舍入），与既有 `aggregate_mutation` 公式逐字一致

#### Scenario: 逐 suite 判定全过即 pass

- **WHEN** 一次 MutationTest 覆盖 desktop（阈值 80）与 src-tauri（阈值 70）两 suite，得分 82 / 68
- **THEN** 门禁结论 fail（src-tauri 未达标），反馈携未达标 suite 的 score / threshold 与报告路径清单

### Requirement: 分段顺序执行与自适应修正

待突变体量估算超段预算时 SHALL 自动拆段顺序执行：段预算目标 5 分钟 / 段（半 CPU 约 250~300 mutants 按 1~2 s/mutant 折算）、段级软护栏 10 分钟（软目标，MUST NOT 硬失败语义），单文件超预算 SHALL 接受（段是分组粒度非硬界）。段切分 SHALL 为字典序均分 k 段 + 自适应修正：首段按保守静态初值（文件行数估 mutant 密度或文件数兜底常数）切分，后续段按上一段实测速率（秒 / mutant）动态调整段大小。每段 SHALL 产出段级原始报告 `mutation-seg-<i>.json`（原始 Stryker / cargo-mutants 报告全程留存 `reports/test/<plan>/mutation/` 子树，MUST NOT 清理），计数归并出总块（沿 `aggregate_mutation`）。

#### Scenario: 分段归并出总块

- **WHEN** 一个 change 的突变面被切成 3 段顺序执行完毕
- **THEN** `mutation-seg-0/1/2.json` 全程留存，计数求和归并出 MutationBlock 回填 summary mutation 位；归并公式与单段直跑逐字一致

#### Scenario: 单文件超预算接受

- **WHEN** 某单文件估算突变体量超 10 分钟软护栏
- **THEN** 该文件独段执行不被截断不报错，段耗时超软目标仅观察留痕，结论照常判定

### Requirement: sandbox 跨段复用与清理时点

分段多轮执行 MUST 复用 sandbox，避免多轮初始化：Stryker 各段 SHALL 共用同一 `tempDirPath`（sandbox 建置与依赖链接 / 安装只发生一次，后续段只换 `mutate` 文件列表与临时 config）；cargo-mutants 各段 SHALL 共享 workspace `CARGO_TARGET_DIR`（增量编译跨段复用，workspace 只全量编译一次，MUST NOT 每段重置 target dir）。清理 SHALL 移到全部段完成后一次执行，且只清 `_stryker-tmp` 与临时 config 文件——段级原始报告与归并报告等报告工件 MUST NOT 被清理。

#### Scenario: 共享 sandbox 只初始化一次

- **WHEN** 3 段 Stryker 顺序执行
- **THEN** `tempDirPath` 建置 / 依赖链接仅发生一次（段 2、3 复用），各段仅 mutate 面与临时 config 不同

#### Scenario: 清理不触报告工件

- **WHEN** 全部段完成后执行清理
- **THEN** `_stryker-tmp` 与临时 config 被清、`mutation-seg-<i>.json` 与归并报告原样在位

### Requirement: 失败升格交决策（报告指引式信息面）

MutationTest 失败 SHALL 一律升格相位 fail（零 WorkerAgent 会话，不注入修复、不续注会话），两时点同式：突变执行**中断**（段执行 error / harness 崩 / 超时）与全部段正常跑完但**未达标**（score < threshold）均以固定三段收场——代写一条 fail checklist → `phase_log` 落账 → 截断本相位余下步（`StepFlow::Upgraded`）回 `phase_next` 进决策分叉；达标 SHALL pass 直接收口（机械 checklist 落账、零会话）。升格落账的 `report` SHALL 携完整反馈文本（`feedback_text`，上限 `MAX_REPORT_CHARS = 2000` 字符）：中断时为中断错误摘要 + 已产报告路径清单，未达标时为 score / threshold 结论摘要 + 原始报告路径清单；evidence SHALL 携诊断摘要（`diagnose_brief` 口径）+ 报告目录。反馈内容 MUST NOT 内联存活突变体清单——决策 agent SHALL 依决策 prompt 的指引自行 `read` 段级 / 归并报告定位（报告指引式取向由决策信息面承继）。失败步 `on_fail` SHALL 为 `OnFail::Escalate`（与 test-execution 两步同），MUST NOT 声明 `FeedbackLoop`；失败条目三会话槽位恒 `None`。`OnFail::Escalate` 的归属守卫 SHALL 保证其只出现在 TestExecution / MutationTest 步上。

#### Scenario: 中断立即升格

- **WHEN** 第 2 段执行中 harness 崩溃（第 1 段已产报告）
- **THEN** 立即升格相位 fail（不等第 3 段跑完）：零 WorkerAgent 会话、一条 fail PhaseRecord（report 携中断错误摘要 + 第 1 段报告路径清单），回 `phase_next` 进决策分叉

#### Scenario: 未达标升格

- **WHEN** 全部段跑完且汇总 score 低于 threshold
- **THEN** 升格相位 fail：report 携 score / threshold 结论摘要与原始报告路径清单，MUST NOT 内联存活突变体明细；零会话请求

#### Scenario: 达标零会话收口

- **WHEN** 全部段跑完且逐 suite 全部达标（或突变面为空跳过）
- **THEN** MutationTest pass，机械 checklist 落账进入 PhaseLog，推进下一相位，零 WorkerAgent 会话发起

#### Scenario: 两口径并存红线随反馈边退役

- **WHEN** 审阅 test-execution 与 MutationTest 的失败处置表述
- **THEN** 两处均为「升格交决策」单一口径，无「有界内联自足 vs 报告指引式反向并存」红线（两反馈边均已不存在）；如未来重引相位内修复会话，须经用户重新裁决并显式提出 spec 变更

### Requirement: 段级复用门与断点续跑

MutationTest SHALL 以段级复用门保证幂等：段报告在位且输入（突变面源文件 + 测试文件）mtime 未变 SHALL 复用该段结果（零 spawn）；中断 / 重试 SHALL 只重跑未完段（段级断点续跑，已完段不重跑）；任一输入 mtime 变化 SHALL 使受影响段失效重跑。复用判定 MUST NOT 演变为反复全量跑。

#### Scenario: 断点续跑只重跑未完段

- **WHEN** 3 段中第 2 段中断后修复重入（输入未变）
- **THEN** 第 1 段复用（零 spawn），第 2、3 段执行，总块由三段归并

#### Scenario: 输入变化段失效

- **WHEN** 上次突变后某源文件 mtime 更新
- **THEN** 含该文件的段失效重跑，其余段复用，归并块更新

### Requirement: 突变常量参数与 config schema 不扩展

突变执行参数（jobs / per-mutant timeout / 段预算目标 / 段软护栏 / 分段静态初值常数）SHALL 以代码常量观察落地，MUST NOT 进 `suite.mutation` 字段、MUST NOT 扩展 config schema；`suite.mutation` SHALL 维持既有 `{ cwd?, score }` 形态。参数调整需求经常量变更走代码评审，观察期后若需用户可配再另立 schema 变更。

#### Scenario: schema 零扩展

- **WHEN** 审查 `openspec/config.json` schema 与 `MutationConfig` 结构
- **THEN** `suite.mutation` 仍为 `{ cwd?, score }`，无 jobs / timeout / 分段预算字段；对应常量在 mutation 执行模块内可考

### Requirement: 段执行取消与进程树击杀

突变段执行 SHALL 经 `exec::run_shell`（见 desktop-exec-runtime），并在既有 `SegmentFailure` 二分（`Fatal` = 工具不可达 / 配置不支持 / 写盘失败 → runner `Err` 停给用户；`Interrupted` = 段中断 → `conclusion = Error` 即时反馈）之上新增第三变体 `Cancelled`：

- `SegmentFailure::Cancelled` SHALL 由 `RunOutcome::Cancelled` 映射产生，并沿 `execute → run_suite → execute_segment` 三层贯穿；`Display` 形态与另两变体同型（单串）。
- 取消 SHALL NOT 走 `Fatal`（那会把用户停止呈现为「环境坏了停给用户」）；取消的收敛口径 SHALL 为段中断面——`conclusion = Error`、`feedback_text` 记「突变执行已取消」+ 已产报告路径清单（过渡形态：MUST NOT 在本变更内引入 run stopped）。
- 段超时预算 SHALL 维持 1800 s（`SEGMENT_SPAWN_TIMEOUT_MS` 不变），但超时臂 SHALL 改为 `RunOutcome::TimedOut` + `exec::kill_tree` 整树回收（此前只杀壳，`cargo mutants` 的 rustc 子孙继续写 `CARGO_TARGET_DIR`——与两检查域同一棵树）；`TimedOut` 仍映射 `SegmentFailure::Interrupted`，超时文案逐字不变。
- 失败桶临时面保留纪律（失败 / 中断的 Stryker 桶临时 config 与 `_stryker-tmp` 不清理、成功照清）SHALL 对取消面同样适用。

#### Scenario: 取消映射为段中断

- **WHEN** 对正在执行的 mutation 段置位 cancel（用户停止 run）
- **THEN** 执行体返回 `Cancelled`，段执行返回 `SegmentFailure::Cancelled`，runner 以 `conclusion = Error` 收场且 `feedback_text` 含取消文案与已产报告路径；MUST NOT 返回 `Err`（`Fatal`）

#### Scenario: 段超时进程树消失

- **WHEN** 某段超过 1800 s 预算未退出（rustc 子孙仍在写 target）
- **THEN** 执行体 `kill_tree` 整树回收（Windows 上壳进程与 rustc 子孙的 pid 均不可再观测），段以 `Interrupted` 收场、文案仍为「段执行超时（1800 秒）强制终止」

#### Scenario: 既有分流不变

- **WHEN** 工具不可达 / 临时 config 写盘失败 / 段报告缺失等既有失败面被触发
- **THEN** 仍分别映射 `Fatal` / `Interrupted`（文案骨架与呈现口径零变），`Cancelled` 变体不参与既有面

#### Scenario: 取消后临时面保留

- **WHEN** Stryker 桶因取消（`Cancelled`）中断
- **THEN** 该桶临时 config 与 `_stryker-tmp` 按失败面保留（反馈面附路径供单段复现），成功桶照常清理

#### Scenario: 复用门与断点续跑不受影响

- **WHEN** 取消后重入同一 change 的 MutationTest（输入未变）
- **THEN** 已完段仍走段级复用门（零 spawn），未完成段重跑；段报告与归并块口径零变

## Module Contract

| 模块 | 定位 | 关键契约（Function / API） |
|------|------|------|
| `crates/core/workflow/src/write/phase_table.rs` | 步声明 | `TEST_EXECUTION_STEPS` 的 MutationTest 步 `on_fail = Some(OnFail::Escalate)` |
| `crates/core/orchestration/src/walker.rs`（`step_mutation_test`） | 门禁步执行体 | 删反馈循环与 `fix_session`；`Escalate` 前置分支（fail / error 均升格）；保留 runner `Err → Terminal` |
| `crates/core/orchestration/src/walker.rs`（`step_mutation_test_upgraded_phase_log`） | 升格落账 | report = `feedback_text`（≤2000）、evidence = `diagnose_brief` + 报告目录、三会话槽位 None |
| `crates/core/orchestration/src/port.rs` | 载荷语义（字段面不变） | `MutationTestOutcome.feedback_text` 消费者由「反馈边修复 prompt」改为「升格 report」 |
| `crates/core/workflow/src/write/phase_table_test.rs` | 声明守卫 | `Escalate` 仅允许出现在 TestExecution / MutationTest 步的断言 |
| `crates/infra/checks/src/mutation/mod.rs` | 段调度与失败面 | `enum SegmentFailure { Fatal(String), Interrupted(String), Cancelled(String) }`（`Display` 同型）；`execute(root, change, files, patch, cancel)` / `run_suite(..., cancel)` / `execute_segment(..., cancel)` 贯穿 cancel；`Cancelled` → `conclusion = Error` + 取消文案 |
| `crates/infra/checks/src/mutation/cargo_mutants.rs` | 段执行（rust） | `run_segment(patch_path, exec_dir, cancel)` 经 `run_shell`（`env CARGO_TARGET_DIR`、1800 s）；`TimedOut` → `Interrupted`（文案不变）、`Cancelled` → `Cancelled` |
| `crates/infra/checks/src/mutation/stryker.rs` | 段执行（js 族） | `run_segment(segment, cancel)` 经 `run_shell`（`cwd = suite_cwd`、1800 s）；同 `TimedOut` / `Cancelled` 映射 |
| `crates/core/orchestration/src/port.rs` | port 契约 | `MutationRunner::run(&self, root: &str, name: &str, files: &[String], patch: &str, cancel: CancelFlag) -> BoxToolFuture` |
| `crates/infra/checks/src/mutation/mod.rs`（port 实现） | runner 实现 | `impl MutationRunner for ProcessMutation`：签名加 `cancel` 并透传 `execute` |
| `crates/core/checks` | 纯层 | 零改动（`parse_*_seg` / score / 归并 / 阈值判定不变） |
