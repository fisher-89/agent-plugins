# 提案: desktop-checks-domain

> **变更**: desktop-checks-domain
> **日期**: 2026-10-06
> **状态**: draft

---

## 问题

桌面端 change run 已落地确定性编排（薄图 walker + static_check 门禁），但测试执行域存在三处缺口：

1. **test-execution 相位无确定性执行器**。桌面 walker 的 9 站流水线含 test-execution 相位（`PIPELINE_PHASES`，`crates/core/workflow/src/queries/detail.rs`），但相位主体只能靠 executor agent 会话驱动（跑外部 CLI 或手工操作）。而 CLI 权威实现（`plugins/dev-team/bin/src/commands/test-execution.ts` 及 `lib/test-*`，生产约 4800 行 TS）逐环节拆解证明：**绿的一次 test-execution 全程无需 agent**——主链（探测 / 门禁 / 复用 / 执行 / 报告 / 聚合 / 机械 eval）100% 确定性，真正需要智能的仅修阻塞错误与根因裁决两处且条件触发。现状下绿跑也要烧 agent 会话与预算，且桌面与插件工作流对同一 change 的测试结论无对拍保障。
2. **检查域寄居 agent crate**。static_check（`crates/infra/agent/src/static_check.rs`，147 行）是「读 config 声明 → spawn 外部进程 → pass/fail 结论」的检查域成员，与 infra/agent 的会话租户身份（cli / sdk / worker / compose / store_port）异族；test-execution 同族更重，继续塞入将加速 agent crate 杂物化。
3. **编排 spec 措辞将与落位事实失配**。desktop-change-orchestration spec 载明「static-check spawn 为编排域唯一 spawn 例外」，Module Contract 将 static-check spawn 缝记在 `crates/infra/agent/src/worker.rs` 名下——test-execution 落地后这些措辞必须随事实修订，否则 spec 与实现漂移。

---

## 提案

**Rust 原生移植 CLI test-execution 确定性主链，落位新建 checks 边界，对齐 static_check 既有模式（port + 门禁步 + 反馈边）**：

- **新边界（checks 边界）**：`crates/core/checks/`（纯层：报告 schema 模型对齐 CLI zod 权威、istanbul / llvm-cov / coverage / text 解析器、阈值与 override 聚合、完整性校验与诊断树确定性分支、复用门判定）+ `crates/infra/checks/`（进程执行：ProcessStaticCheck 自 infra/agent 平移零逻辑改动、testexec runner 探测 / 模板展开 / spawn / 超时 / 捕获 / 写盘）。runner port（`TestExecutionRunner`）留 `orchestration::port` 原地——port 属于消费者，W4 红线延续（spawn 不进 core）。
- **框架矩阵**：jest / vitest / vite-plus（istanbul 族，`coverage-summary.json`）+ rust（cargo test + llvm-cov，本仓自吃）+ node-test（`--test-reporter=spec` 文本结果，与 rust 共用 text-parser）；bun / go / pytest 遇配置显式报错而非静默跳过；go-parser / coverage-py / stryker mutation 链路不移植（报告 `mutation: null` 合法）。
- **walker 接入**：对齐插件相位语义——test-execution 相位收敛点执行确定性门禁步；`ChangeStepKind::TestExecution` 落账上图；结论 pass 以机械 eval checklist 代写 `phase_log`、全程零 agent；fail/error 走 static_check 同款反馈边（findings 诊断注入 executor 会话修复，重入经复用门自动失效重跑），独立预算 5 次与 static-check 分立，超限升格相位 fail。
- **幂等基石**：复用门进 MVP（mtime 新鲜度，输入未变复用上次 summary）——复用门是相位自动触发不等于反复全量跑的前提；net-zero 突变范围去噪后补。
- **防漂移**：报告 schema 以 CLI zod schema（`bin/src/schemas`）为权威对齐源，corpus 黄金测试用真实工作区产出的 summary.json / report.json fixtures 对拍（`core/workflow` corpus_golden_test 先例同款）。

探索笔记：`openspec/changes/desktop-checks-domain/explore.md`（三轮方向修正 + 两轮逐条对码审查 E1–E5，事实基线 HEAD `aff4313`，本变更直接消费其 §11 落位修订与 §12 拍板项）。

---

## 能力

### 新增能力

- **desktop-checks-domain** — 检查域边界：分流法则（外部进程 + pass/fail 结论门禁 → checks 边界）、core/checks 纯层 + infra/checks 进程执行双层落位与依赖规则、static_check 平移、agent crate 职责收窄。
- **desktop-test-execution** — test-execution 确定性门禁：port 契约三件套（ToolCommand::TestExecution / ToolStepOutput::TestExecution / TestExecutionRunner）、五框架执行链（探测 → 完整性门禁 → 复用门 → 执行 → 报告 → 聚合 → 诊断）、复用门幂等、反馈边与独立预算、步骤可观测与最小载荷、报告落盘、双实现对拍黄金语料。

### 修改的能力

- **desktop-change-orchestration** — spawn 例外措辞由「static-check 唯一例外」修订为「检查域（static-check / test-execution）落 checks 边界」；walker 相位循环步④与三类节点 ToolStep 描述扩展；static-check 门禁步执行归属改指 `crates/infra/checks`；Module Contract `port.rs` 封闭集扩展、`walker.rs` 门禁接入、`worker.rs` 收窄（static-check spawn 缝移出）。
- **desktop-crate-layout** — 边界分类学五类→六类（新增 checks 边界）；Module Contract 增 checks 两 crate 行、agent-runtime 行收窄。

---

## 变更范围

### 实现文件

- 新增 `packages/desktop/src-tauri/crates/core/checks/`（crate 裸名 `checks`）：model（tests[] / plan / report schema 对齐 CLI zod）/ parser（js istanbul、rust llvm-cov、coverage、text 的 node-test / rust 分支）/ aggregate（阈值 / override 聚合 + conclusion 判定）/ diagnose（完整性校验 + 诊断树确定性分支）/ reuse（复用门判定）
- 新增 `packages/desktop/src-tauri/crates/infra/checks/`（crate 裸名 `checks-runtime`）：static_check.rs（自 infra/agent 平移，零逻辑改动）+ testexec/（detect 框架注册表与 glob 探测、runner 命令模板展开 + spawn + 超时 + 输出捕获、report 写盘编排）
- `packages/desktop/src-tauri/crates/core/orchestration/src/port.rs`：`ToolCommand::TestExecution` / `ToolStepOutput::TestExecution`（最小载荷：conclusion + 计数 + 诊断摘要 + 报告路径）/ `TestExecutionRunner` port
- `packages/desktop/src-tauri/crates/core/orchestration/src/state.rs`：`ChangeStepKind::TestExecution`
- `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs`：test-execution 相位门禁步接入 + 反馈边（独立计数器）+ 复用门装配
- `packages/desktop/src-tauri/crates/core/foundation/src/layout/mod.rs`：change 报告目录（reports/test）路径推导常量
- `packages/desktop/src-tauri/crates/infra/agent/src/lib.rs`：static_check 模块导出移除
- 组合根装配（desktop-app 命令组构造处）：ProcessStaticCheck 新路径 import + TestExecutionRunner 装配注入
- `packages/desktop/src-tauri/Cargo.toml`：checks 两 crate 注册 workspace members
- `packages/desktop/package.json`：version 0.4.11 → 0.4.12（AC 声明、归档时执行）

### 测试文件

- `crates/core/checks/` 各模块 `*_test.rs`：解析器 / 聚合 / 诊断树 / 复用门纯函数测试（fixtures 直驱）
- `crates/core/checks/tests/corpus_golden_test.rs`：真实工作区 summary.json / report.json fixtures 双实现对拍（corpus_golden_test 先例同款）
- `crates/infra/checks/src/static_check_test.rs`（随源平移）+ `testexec` detect / runner 测试（Windows .cmd shim、超时、程序解析前置检查）
- `crates/core/orchestration/src/` walker_test / port_test / state_test 扩展：TestExecution 步、反馈边独立预算、复用门、超限升格路径
- `crates/core/foundation` layout 测试：报告路径推导（mod_test 命名隔离扫描自动覆盖）

### 删除文件

- `packages/desktop/src-tauri/crates/infra/agent/src/static_check.rs` + `static_check_test.rs`（平移后原位删除）

### 不要修改

- `plugins/dev-team/**`：CLI test-execution 为权威参考实现且长期并存（插件工作流 executor agent 路径继续使用），零改动；插件版本保持 2.10.44
- `openspec/specs/` 插件侧测试域能力（cli-unit-test-execute / unit-test-executor / test-execution-diagnostics / test-detect-frameworks / test-exclude-filter / test-path-resolver 等）：桌面为独立新能力，不改 CLI spec
- `packages/desktop/src-tauri/crates/infra/agent/src/git_diff.rs`：纯上下文工具暂留 agent（81 行，消费者为 executor / 决策供料链路，不为单文件预建边界，挂账）
- `crates/core/workflow`（写面路由权威零触点）、`crates/infra/store`
- `crates/core/orchestration/src/control.rs` 与 `RunUpdate` / `ChangeRunSnapshot` / `ChangeRunStatus` DTO（零触点红线）
- go-parser / coverage-py / stryker-config mutation 链路：不移植（后续切片偿还）

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | checks 两 crate 落位并注册 workspace | `crates/core/checks` 与 `crates/infra/checks` 存在且为 workspace member；`cargo test --workspace` 全绿 |
| AC-2 | ProcessStaticCheck 平移 | `static_check.rs` + 测试落 `infra/checks`，`infra/agent` 无 static_check 残留；平移后测试全绿且逻辑零改动 |
| AC-3 | orchestration port 三件套扩展 | `port.rs` 具 `ToolCommand::TestExecution` / `ToolStepOutput::TestExecution` / `TestExecutionRunner`；core/orchestration 源码零进程 spawn |
| AC-4 | 框架矩阵探测 | detect 对 jest / vitest / vite-plus / rust / node-test 产出 plan；bun / go / pytest 配置显式 `Err` 不静默跳过 |
| AC-5 | 执行链绿跑 | fixture 驱动 detect → 完整性门禁 → 执行 → 报告写盘 → 聚合全链，conclusion=pass 且报告落 change 报告目录 |
| AC-6 | 复用门幂等 | 输入未变复用上次 summary 零 spawn；输入变化失效重跑（单测覆盖两分支） |
| AC-7 | walker 接入与反馈边 | test-execution 相位门禁步上图（`ChangeStepKind::TestExecution`）；fail/error 反馈边独立计数 5 次超限升格相位 fail；绿跑零 agent 会话 |
| AC-8 | corpus 黄金对拍 | 真实 summary / report fixtures 的解析与聚合结论和 CLI zod 权威一致 |
| AC-9 | 报告路径经 layout 推导 | change 报告目录推导入 foundation layout；desktop 产品源码零磁盘路径字面量（命名隔离双禁令扫描通过） |
| AC-10 | 装配与 IPC 面 | 组合根装配新 runner；bindings 再生成（ChangeStepKind 新变体出线）；前端流程视图步骤渲染零改动直接吃 |
| AC-11 | spec delta 措辞修订随归档生效 | orchestration「唯一 spawn 例外」与 Module Contract worker.rs 行、crate-layout 边界分类学六类措辞与实现一致 |
| AC-12 | 版本交付 | `packages/desktop/package.json` version 0.4.12（用户可见变更：run 内新增确定性测试执行门禁） |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 双实现漂移：CLI test-execution 长期并存，门禁语义 / 报告 schema / 复用门判定漂移致同一 change 两处结论不一致 | 高 | 中 | corpus 黄金对拍（真实工作区 fixtures）；报告 schema 以 CLI zod 为权威对齐源；复用门判定逐字段对照 CLI |
| Windows 进程细节：jest / vitest / npx 均 `.cmd` shim，shell 会吞命令缺失烧反馈边预算 | 中 | 中 | 复用 static_check shell spawn + 程序解析前置检查（PATH / cwd 探测、可执行后缀候选）与 cmd /C shim 经验 |
| dogfood 口径不一致：本仓 desktop 档(80) / src-tauri rust 档(70) 开着 mutation，MVP 报告 mutation 恒 null，桌面与插件跑同一 change 结论不同 | 低 | 高（已知必然） | 明知接受并留痕于 spec V1 边界；报告 mutation 位显式 null；后续 mutation 切片偿还 |
| walker 相位语义改动波及前端流程视图 | 低 | 低 | `ChangeStepKind` 封闭集扩展为白名单式，现有步骤渲染直接吃；bindings 再生成 + `bindings:check` 守卫拦截 |
| 报告 / 聚合语义全对齐工作量（zod schema + coverage override 聚合 + 诊断树） | 中 | 中 | 切片推进：纯层 + corpus 先行、执行链次之；fixtures 用真实产出而非手造 |
| 未决项：glob crate 选型（globset 候选）、runner 超时值来源 | 低 | 低 | design 阶段定（见待决问题） |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| 实现路径 | Rust 原生移植，不依赖 dev-team-cli、不经过 agent | 主链 100% 确定性（绿跑零 agent）；CLI 子进程复用引入运行时依赖，sdk 租户驱动误用 agent（探索三轮方向修正均否决前案） | ① sdk 租户跑 executor stance（否决）② spawn 内嵌 CLI 子进程（否决） |
| 落位结构 | checks 边界双层：core/checks 纯层 + infra/checks 执行层；runner port 留 `orchestration::port`（A1） | port 属于消费者（六边形）；移 port 牵动 `ToolStepOutput` 封闭集引用链；agent crate 回归会话身份 | A2 port 移 checks（否决）；B infra/orchestration 适配器合集（否决，域语义缺失）；C infra/tools（否决，与顶层 tools 撞名） |
| static_check 归属 | 自 infra/agent 平移 `infra/checks`，零逻辑改动 | 检查域成员与会话租户异族；分流法则：外部进程 + pass/fail 结论门禁 → checks 边界 | 留 agent（杂物化，否决） |
| 框架矩阵 | jest / vitest / vite-plus / rust / node-test；bun / go / pytest 显式报错；text-parser 以 node-test / rust 分支进移植范围 | 本仓自吃需要 rust 与 vite-plus；istanbul 族解析器共享；node-test 第二轮拍板并入（rust 结果同路由 parseTextOutput） | go / python / bun 全矩阵（成本高，显式报错兜底） |
| walker 接入点 | test-execution 相位收敛点执行（对齐插件相位语义：test-execution 即独立相位） | 与插件工作流相位表一致；复用门使自动触发幂等 | implement 后第二门禁步（非独立相位语义，否决） |
| mutation | MVP 砍（报告 `mutation: null` 合法，evaluator T3 null 自动通过） | 解析器 + 突变范围 + net-zero 去噪成本高；后续切片偿还 | MVP 含 stryker 链路 |
| 反馈边预算 | 独立计数上限 5 次（与 static-check 计数器分立，各自计满各自升格） | 沿「反馈边不计相位 retry 预算」先例；取值对齐 static-check 先例（`STATIC_CHECK_FEEDBACK_LIMIT`） | 共享 static-check 计数器 |
| Step1b / T4 agent 能力 | 不进本期：红跑以 findings 呈现走反馈边或留给用户决策 | 域收敛（确定性主链）先行；agent 修复能力后续独立偿还 | MVP 含 agent 修阻塞错误 / 根因裁决 |
| 复用门 / net-zero | 复用门进 MVP；net-zero 突变范围去噪后补 | 复用门是自动触发幂等的基石 | 全量对齐 CLI 一步到位 |
| 报告落盘 | 恒 change 报告目录 `reports/test/`；不做 artifacts registry 登记（不进 file inventory） | desktop 无无-change 场景（CLI `<root>/reports/test/` 分支不实现）；报告非变更文件 | `<root>/reports/` 分支 + registry 登记 |

### 待决问题

- glob crate 选型（globset 候选）——design 阶段定
- runner 超时值来源（CLI timeout 语义未摘录）——design 阶段定
- 机械 eval checklist 的字段形态与落账信封（T1–T3 静态 checklist 如何进 `phase_log`）——design 阶段定
- `TestExecutionOutcome` 最小载荷字段终形（候选：conclusion + 计数 + 诊断摘要 + 报告路径）——design 阶段定

---
