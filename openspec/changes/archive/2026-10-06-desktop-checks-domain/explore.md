# 探索笔记：desktop test-execution（Rust 原生实现，对齐 static_check 模式）

> **日期**: 2026-10-06
> **状态**: 探索完成（2026-10-06 对码审查修订一轮，见 §12），待 phase-proposal 收敛
> **范围**: packages/desktop —— orchestration 门禁步 + infra/agent 执行器
> **事实基线**: 本仓 HEAD `aff4313`（desktop 0.4.10）。早期调研曾基于市场克隆
> （desktop 0.3.5 / 插件 2.10.44），该版本无 orchestration crate 与 static_check，
> 相关旧结论已按本仓现状修正。**本文件为权威副本**（另一份镜像在
> `~/.claude/plugins/marketplaces/wps-ai/openspec/explores/`，随插件刷新可能丢失）。

---

## 1. 现状与本仓已有资产（关键：大部分地基已经打好）

本仓 desktop 已落地**确定性变更流程引擎**与 **static_check 门禁**，test-execution
是同一模式的重型实例，不是从零建域：

```
crates/core/orchestration/          确定性流程引擎（walker 模式）
├─ port.rs      ToolCommand 封闭集：PhaseNext/PhaseStart/PhaseLog/Backtrack/
│               DecisionLog/StaticCheck
│               ToolStepOutput 封闭集：… + StaticCheck(StaticCheckOutcome)
│               StaticCheckRunner port（W4 红线：spawn 不进 core，独立 port）
│               DiffContextPort（git diff，infra/agent/git_diff.rs 实现）
│               WorkerAgentPort（agent 会话）/ WorkflowSnapshotPort / RunEventSink
├─ state.rs     ChangeStepKind：Executor/Evaluator/Decision/PhaseStart/
│               StaticCheck/PhaseLog/VerdictGate/RetryGate/WhitelistGate
│               ← 步骤记录的既有机制
├─ walker.rs    static_check_loop：门禁 + 反馈边（fail 落账 → Upgraded →
│               回 phase-next 分叉让 worker 收到诊断反馈）
└─ control.rs   ChangeFlowControl（变更流程控制入口）

crates/infra/agent/
├─ static_check.rs    ProcessStaticCheck（147 行：config 读命令 → shell spawn →
│                     退出码映射；无配置直过；程序不可达显式 Err）
├─ git_diff.rs        git diff 上下文（net-zero/突变范围可参考）
├─ sdk/ + cli/        agent 双租户（与本题无关，见 §2）
└─ worker.rs / store_port.rs / compose.rs

crates/core/config/   openspec/config.json 解析：static_analysis 与 tests[]
                      均已完整解析（TestSuite 八字段 typed 模型 + 永不失败
                      diagnostics 信封，checks 直接消费；§12 E1 修正）
```

## 2. 方向轨迹（三轮修正，均已否决前案）

```
① agent 驱动：用 infra/agent/src/sdk 租户跑 executor stance
      ↓ 否决：test-execution 绝大多数环节是确定性的，不该经过 agent
      （注：sdk 租户本仓已实现——rig 引擎、L1-L3 上下文防御、活性预算，
        当时讨论的是"用它跑 test-execution"，否决的是用途不是租户本身）
② CLI 子进程复用：spawn 内嵌 dev-team-cli.cjs test-execution
      ↓ 否决：不依赖 dev-team-cli，Rust 重实现
③ Rust 原生实现，对齐 static_check 既有模式（port + 门禁步 + infra 落点）← 最终
```

## 3. test-execution 的确定性成色

executor / evaluator 两份 agent 定义逐环节拆解：**绿的一次 test-execution 全程无需
agent**。CLI 本体（编排/探测/门禁/复用门/突变）100% 确定性。真正需要智能的仅两处且
条件触发：

| 环节 | 性质 | 说明 |
|---|---|---|
| 跑测试进程 | 纯机械 | 框架命令模板展开 + spawn |
| 报告存在/读取 | 纯机械 | schema 对齐 CLI zod |
| 完整性校验（Step3） | 纯机械 | 字段/计数/conclusion 一致性 checklist |
| 诊断树 4a/4c/4d/4e/4f | 准机械 | 阈值比对 + null 维度感知，规则写死 |
| 失败模式聚类（4b） | 半机械 | 按模块/错误类型/时长聚类可机械化，根因措辞是判断 |
| findings 写回（Step5） | 纯机械 | JSON append，禁改其他字段 |
| Eval T1/T2/T3 + verdict | 纯机械 | 静态 checklist |
| **修阻塞错误（Step1b）** | **agent** | 仅 `conclusion === "error"` 时触发 |
| **根因唯一性（T4）** | **agent** | 仅 `failed > 0` 时评估 |

Step 1b / T4 的 agent 能力**不进本期**：出现 error/failed 时以 findings 呈现，
走 static_check 同款反馈边（Upgraded → worker 收到诊断）或留给用户决策。

## 4. 落位结论：三层对齐 static_check 模式

```
① orchestration::port（core，纯契约）
   ToolCommand::TestExecution { change, … }        ← StaticCheck 旁新变体
   ToolStepOutput::TestExecution(TestExecutionOutcome)
   TestExecutionRunner port                        ← StaticCheckRunner 同款
   （W4 红线照抄：spawn 不进 core，独立 port）
   TestExecutionOutcome 载荷形状待定（最小：conclusion + 计数 + 诊断摘要
   + 报告路径；findings 全量留在报告文件里，步载荷保持轻）

② orchestration::state + walker（core，流程接入）
   ChangeStepKind::TestExecution                   ← 步骤记录白拿
   walker 接入点：对齐 static_check_loop 的门禁 + 反馈边模式
   （具体挂在哪个相位边界 = 待决策 §8）

③ infra/checks（IO 落点；**修订：移出 infra/agent，见 §11**）
   static_check.rs           ProcessStaticCheck 平移（147 行，零逻辑改动）
   testexec/                 TestExecutionRunner 实现（编排：探测→门禁→
                             复用门→逐条执行→汇总→findings）
     detect.rs   tests[] + glob → plan entries（框架注册表）
     runner.rs   模板展开（{results_file}/{report_dir}/{config_args}/{files}）
                 → spawn → 超时 → 输出捕获（cmd /C shim 经验复用）
     report.rs   报告写盘编排

④ core/config：tests[] 解析已就位（§12 E1 修正：HEAD 已实现 TestSuite 八
   字段 + 八值框架枚举 + 永不失败 diagnostics 信封——违例 suite 剔除、阈值
   越界吃默认；checks 直接消费 typed 模型，无需新解析）
```

## 5. 移植范围（CLI 权威参考 → Rust）

CLI 生产代码 ~4800 行 TS（主链 3168 + 解析器 1680）+ ~8500 行测试，
路径 `plugins/dev-team/bin/src/`：

```
编排层  commands/test-execution.ts  583   主流程/变更门禁/复用门/突变范围/net-zero
探测层  commands/test-detect-frameworks 307   config 解析 + glob 探测
        test-framework.ts           243   框架描述子注册表（命令模板/覆盖格式）
        test-plan.ts                226   plan 解析(root/cwd/id/files)
        test-exclude.ts             155   排除过滤
        test-path-naming.ts          92   colocated 命名映射
执行层  test-runner.ts              778   命令组装 + 进程执行 + 超时
解析层  lib/test-parser/           1680   js/text/go/mutation/stryker/coverage
报告层  test-report.ts             784   子报告/汇总/阈值/override 聚合
报告 schema：bin/src/schemas（zod，权威对齐源）
```

外围依赖与复用：glob（Rust crate 替 fast-glob）、git show HEAD（net-zero 去噪，
git_diff.rs 同 crate 可参考）、mtime 递归扫描（复用门）、workflow.json file_log
（`core/workflow` 已解析，变更门禁直接调用）。

## 6. 已决策（2026-10-06 拍板）

1. **Rust 原生实现**；不依赖 dev-team-cli，不经过 agent。
2. **落位对齐 static_check**：orchestration port/step + infra 实现（§11 修订：
   实现自 infra/agent 移 infra/checks，port 留 orchestration 原地）。
3. **框架矩阵**：jest / vitest / vite-plus（istanbul 族，`coverage-summary.json`，
   解析器几乎共享）+ rust（cargo test + llvm-cov，本仓自吃需要）+ node-test
   （2026-10-06 第二轮拍板并入，§12 #7）。**暂不支持 go、python、bun**（遇
   配置显式报错而非静默跳过；go-parser / coverage-py 不移植；注：rust 与
   node-test 结果均路由 parseTextOutput 文本解析，text-parser 以该两分支
   进移植范围——原"text-parser 不移植"表述据此修正，见 §12 #7）。
4. **变更流程自动执行 + 记录步骤**：挂在既有 walker 门禁位（见 §7）。

## 7. 变更流程自动 check 与步骤记录（接入既有机制，不新建）

static_check 已示范完整模式，test-execution 照抄骨架：

- **触发点**：walker 在指定相位边界执行（static_check 亦非每步——仅
  implement / test-gen 相位 executor 收口后经 STATIC_CHECK_PHASES 门控，
  §12 E2 修正；test-execution 同样只在目标相位执行——**复用门是自动化的
  前提**：输入未变则复用上次 summary（mtime 新鲜度），幂等化后"自动触发"
  不等于"反复全量跑"）。
- **步骤记录**：`ChangeStepKind::TestExecution` 落账，与 StaticCheck 步骤
  并列可见（前端流程视图现有步骤渲染直接吃）。
- **失败反馈边**：conclusion=fail/error 时对齐 `StaticCheckFlow::Upgraded`
  语义——落账后回相位分叉，worker（executor agent）收到 findings 诊断反馈；
  Step1b 的"修环境"能力留在这条边上（agent 修完重入，复用门自动失效重跑）。

## 8. 待决策清单

| # | 事项 | 倾向 |
|---|---|---|
| 1 | walker 接入相位边界：test-execution 相位收敛时？还是 implement/test-gen 后的门禁？ | 对齐插件工作流的 test-execution 相位语义 |
| 2 | mutation MVP 砍不砍 | 倾向砍（报告 `mutation: null` 合法，evaluator T3 null 自动通过）。**dogfood 冲突**：本仓 config 仅 desktop 档(80)/src-tauri rust 档(70) 开着 mutation（plugins/dev-team 档未开，§12 E3），MVP 期桌面与 CLI 结论不一致，需明知 |
| 3 | TestExecutionOutcome 载荷形状 | 最小载荷 + 报告路径；全量 findings 留报告文件 |
| 4 | 诊断树/完整性校验归属 | 纯函数放 core（orchestration 或 config 邻域），corpus 可测 |
| 5 | 复用门/变更门禁/net-zero 进 MVP 的范围 | 复用门建议进（幂等基石）；net-zero 去噪后补 |
| 6 | spec delta | W4 红线（spawn 不进 core）不变，但 spec 必改三条：orchestration "唯一 spawn 例外"措辞、static-check 相位矩阵条款、Module Contract worker.rs 行（§12 E4）；外加 crate-layout 增 checks 边界成员。非"核对措辞"级别 |

## 9. 风险与对策

**双实现漂移（主风险）**：CLI 的 test-execution 仍被插件工作流（executor agent）
长期使用，Rust 版与 TS 版并存。门禁语义/报告 schema/复用门判定漂移会导致同一
change 两处结论不一致。对策：**corpus 黄金测试**——用真实工作区产出的
summary.json / report.json 做 fixture 对拍（`core/workflow` 已有
corpus_golden_test 先例）；报告 schema 以 CLI zod schema 为权威对齐源。

**Windows 进程细节**：jest/vitest/npx 均为 `.cmd` shim；static_check.rs 的
shell spawn、程序解析前置检查（避免 shell 吞命令缺失烧反馈边预算）经验直接复用。

## 10. 建议切片顺序

```
切片 0  orchestration port 契约（ToolCommand/ToolStepOutput/
        TestExecutionRunner 变体，纯类型）——core/config tests[] 解析已
        就位（§12 E1），本切片较原计划砍半
切片 1  纯层 + corpus 黄金测试：report schema 对齐 zod、完整性校验、
        诊断树确定性分支（fixtures 用真实 summary.json/report.json）
切片 2  istanbul 执行链：detect(jest/vitest/vite-plus) + runner + 解析器
        + 子报告/汇总写盘 → infra TestExecutionRunner 可独立调用
切片 3  walker 接入：ChangeStepKind::TestExecution + 门禁循环 + 反馈边
        + 复用门（幂等）
切片 4  rust 档：cargo test + llvm-cov 解析（本仓自吃）；含文本结果解析
        （parseTextOutput）移植，node-test 复用同引擎（§12 #7）
切片 5  mutation（可选后补）：stryker-config + mutation-parser +
        突变范围/net-zero 去噪
```

---

## 11. 落位修订（2026-10-06 第三轮：static_analysis / test_execution 移出 infra/agent）

**动因**：infra/agent 的身份是 Claude 会话租户（cli/sdk/worker/compose/store_port），
static_check 是"读工作区 config 声明 → spawn 外部进程 → pass/fail 结论"的检查域成员，
与 agent 会话无关；test-execution 同族更重，塞入会加速 agent crate 杂物化。

**walker 直接调用步的分流法则**（按执行性质定归属）：

```
├─ 进程内直调（相位机步→workflow 写面）   → core/orchestration LocalToolSteps（不动）
├─ 外部进程 + pass/fail 结论门禁          → checks 边界（core/checks + infra/checks）
├─ 外部进程 + agent 会话                  → agent 边界（infra/agent，现状 worker）
└─ 外部进程 + 纯上下文文本（git_diff）    → 归属按消费者定（暂留 agent，挂账）
```

**选定结构（方案 A1）**：

```
crates/core/checks/     ★新 检查域纯层：
  model.rs              tests[]/plan/report schema（对齐 CLI zod）
  parser/               js(istanbul)/rust(llvm-cov)/coverage 解析器（纯函数）
  aggregate.rs          阈值/override 聚合 + conclusion 判定
  diagnose.rs           完整性校验 + 诊断树确定性分支
  reuse.rs              复用门判定（mtime 新鲜度比较）
crates/infra/checks/    ★新 进程执行：
  static_check.rs       ProcessStaticCheck 自 infra/agent 平移（零逻辑改动）
  testexec/             TestExecutionRunner（spawn/模板展开/超时/捕获/写盘）
crates/infra/agent/     回归会话身份（cli/sdk/worker/compose/store_port/git_diff）
```

- **runner port 留 orchestration::port（A1）**：六边形"port 属于消费者"——
  ToolCommand/ToolStepOutput/StaticCheckRunner 是 walker 协议，原地不动；
  移出的只是实现，orchestration 零改动，改动面 = 组合根装配 import。
  （A2 port 移 checks 会牵动 ToolStepOutput 封闭集引用链，否决。）
- **否决方案 B**（infra/orchestration 适配器合集）：与仓库"边界 = 外部系统
  关注点"哲学不合，test-execution 纯域无家，且会换名重演杂物化。
- **否决方案 C**（infra/tools 通用抽屉）：与顶层 tools/（export-bindings
  构建工具）撞名，无域语义。
- **git_diff（81 行）暂留 infra/agent**：给 executor/决策供上下文，与
  worker/compose 同属会话供料链路；将来纯上下文工具多了再立域，不为
  81 行预建边界。
- **core/config 的 tests[] 解析已就位**（§12 E1 修正，原"补解析"系误报）：
  与 static_analysis 同文件同源，checks 直接消费 typed 模型（core→core
  依赖有先例）。
- spec delta：新增 checks 边界叙述 + agent crate 职责收窄，proposal 阶段并入。

**已按此修订 §4③**（探索历史保留：v2 曾落位 infra/agent 同处，v3 修正）。

---

## 12. 审查修订（2026-10-06 第二轮：逐条对码核实）

> 将 §1-§11 事实基线逐条核到本仓 HEAD `aff4313` 代码：错误五处（E1-E5）
> 已就地修正原文并留档于下；遗漏与新增决策项并入本节。方向性结论（Rust
> 原生、三层落位、checks 独立域、A1 结构）经核实全部站得住。

### 核实通过面（不再展开）

- orchestration port 全集：ToolCommand 六变体 / ToolStepOutput 封闭集 /
  StaticCheckRunner / DiffContextPort / WorkerAgentPort / WorkflowSnapshotPort /
  RunEventSink（port.rs 逐项命中）
- ChangeStepKind 九变体（含 VerdictGate / RetryGate / WhitelistGate，state.rs）
- static_check.rs 恰 147 行、git_diff.rs 恰 81 行
- static_check_loop + StaticCheckFlow::Upgraded 反馈边 + 独立 5 次上限
  STATIC_CHECK_FEEDBACK_LIMIT（walker.rs:24 / 195 / 329 / 378）
- CLI 行数逐一命中：583 / 307 / 243 / 226 / 155 / 92 / 778 / 784，
  test-parser 生产 1680
- corpus_golden_test 先例（core/workflow/tests/corpus_golden_test.rs）；
  file_log 已解析（core/workflow/src/parse/ + model/）
- zod schema 权威源在位（schemas/test-execution-output.schema.ts 258 行等）
- §3 "绿跑无需 agent" 拆解与 test-execution skill 职责描述吻合，方向可信

### 就地修正留档（E1-E5）

| # | 原错误 | 修正 |
|---|---|---|
| E1 | §1 / §4④ / §11 / §10 切片 0 称 core/config "未解析 tests[]，待补" | HEAD 已完整实现：TestSuite 八字段 + 八值框架枚举 + assemble_suite + 永不失败 diagnostics 信封（config/lib.rs:62 / 205 / 419）。容错口径实为"违例剔除 / 吃默认 + diagnostics"，严于原设想的 fail-open；切片 0 相应砍半 |
| E2 | §7 称 "static_check 是每步收敛门禁" | 相位限定单点门禁：仅 implement / test-gen 相位 executor 收口后经 STATIC_CHECK_PHASES 门控（walker.rs:184-185，spec 同款措辞） |
| E3 | §8-2 称 "vite-plus(80)/rust(70) 开着 mutation" | 实为 desktop 档(80) / src-tauri rust 档(70)；plugins/dev-team 档（同为 vite-plus）未开 |
| E4 | §8-6 倾向 "无需新修订 spawn 边界" | 必改三条：orchestration spec "唯一 spawn 例外"（:91）、相位矩阵条款（:133）、Module Contract worker.rs 行（:261）；另 crate-layout 增 checks 边界成员（§11 已提） |
| E5 | §5 生产 "~3800 行"、detect 归组暗示在 lib/ | 生产实为 ~4800（主链 3168 + 解析器 1680）；detect 在 commands/（是子命令，另有独立 schema）；parser 生产精确 1680 |

### 新增决策项（并入 §8，编号接排；2026-10-06 拍板）

| # | 事项 | 拍板 |
|---|---|---|
| 7 | bun / node-test 处置 | **支持 node-test，暂不支持 bun**（bun 与 go / pytest 同口径：遇配置显式报错）。连带核实：node-test 结果为 `--test-reporter=spec` 文本（results.txt）经 parseTextOutput，覆盖率走 coverage-parser 'node-test' 文本表格分支，且 rust 结果同样路由 parseTextOutput（parser/index.ts:119）——**text-parser 以 node-test / rust 分支进移植范围**，§6-3 原"text-parser 不移植"表述已修正，仅 go-parser / coverage-py 维持不移植 |
| 8 | test-execution 反馈边预算 | **独立预算**：与 static-check 的 5 次计数器分立，各自计满各自升格相位 fail；上限取值与 spec 措辞随 proposal 定（沿"反馈边不计相位 retry 预算"先例） |
| 9 | 报告落盘 | **照抄 change 路径**：恒 `openspec/changes/<change>/reports/test/`（test-execution.ts:59-64 的 change 分支）；desktop 无无-change 场景，`<root>/reports/test/` 分支不实现。不做 artifacts registry 登记——报告不进 file inventory（MVP 口径，后续 workflow-file-inventory 有需要再议） |

### §8-1 补充（原倾向不变）

插件工作流的 test-execution 是**独立相位**，static-check 挂 implement / test-gen
executor 之后——"test-execution 相位收敛时跑" vs "implement 后第二门禁步"
才是 #1 的决策核心分叉；倾向维持"对齐插件相位语义"。

### 小项（design 层可后补）

- glob crate 选型（globset 候选）未定
- runner 超时值来源（CLI timeout 语义未摘录）未定

---

**关联**：CLI 权威实现 `plugins/dev-team/bin/src/commands/test-execution.ts` 及
`lib/test-*.ts` / `lib/test-parser/`；报告 schema `bin/src/schemas`（zod）；
本仓参照物 `crates/core/orchestration/src/port.rs`（StaticCheckRunner/ToolCommand）、
`crates/infra/agent/src/static_check.rs`（ProcessStaticCheck）、
`crates/core/orchestration/src/walker.rs`（static_check_loop 反馈边模式）。
