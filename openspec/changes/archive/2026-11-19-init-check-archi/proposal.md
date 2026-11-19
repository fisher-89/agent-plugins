# 提案: init-check-archi

> **变更**: init-check-archi（目标原文：变更初始阶段，检查架构）
> **日期**: 2026-11-18
> **状态**: draft
> **来源**: `openspec/changes/init-check-archi/explore.md`（目标一句话：「变更初始阶段，检查架构」——无展开笔记，本提案按代码事实补全设计面）
> **性质**: 在 **initialize 表头**（一次性基线检查相位）追加第三个基线检查步 `archi_check`（全量架构漂移判定，`on_fail=Terminal`）——把「基线里的架构债」挡在流程启动之前，与既有 static-check / test-execution 同级同语义。改的是**挂载点与观察口径**：相位表多一步、port 的 `ArchiCheck` 命令多一个模式位、runner 增全量落盘分支、walker 增单发终态执行体。判定规则（扫什么 / 怎么归属 / 定级 / 豁免 / 报告形状）零改动，零新增 IPC 命令，零新增出线类型，零 `.dev-team/architecture/**` 资产改动。
> **不含**: 判定面任何规则改动（归 `desktop-archi-drift`，本变更只补「全量模式的落盘由消费面决定」一条口径）；契约零 diff（`api:check`）——它已在基线 static-check 的 `pnpm check` 链内，不重复挂；gate 档位数据调整（本仓 53 个元素全 `off`，无调整需求）；test-only 表的独立门禁（表头 initialize 两表共享，天然覆盖）。

---

## 问题

### 现状链路（读过的真实代码）

```text
phase_table.rs  INITIALIZE_STEPS = Clean → StaticCheck(Terminal) → TestExecution(Terminal)
                                              │              │
                                              │              └─ ToolCommand::TestExecution{slot: Baseline} → checks 域 runner
                                              └─ ToolCommand::StaticCheck{slot: Baseline} → checks 域 runner

implement 相位  ArchiCheck(FeedbackLoop{3}) ──→ ToolCommand::ArchiCheck{change_id}
                                                  └─ steps.rs: vcs.change_edit_set(root, "HEAD") → changed
                                                     → archi-runtime::LocalArchiCheck::run
                                                        → archi::drift::check(mode = Incremental 硬编码)
                                                        → passed = errors.is_empty() && structuralErrors.is_empty()
```

initialize 相位是**两表共享的表头一次性基线检查相位**（`PhaseDefinition { executor: None, evaluator: None, backtrackable: false, artifact_manifest: &[] }`），语义是「基线的债不属于本次变更，红即停给用户」：`Terminal` 失败 → PhaseRecord 零条目 → 再发起时 Clean 恢复基线后干净重试；全绿 → 持久跳过。

架构漂移门禁今天只挂在 **implement 相位**（`desktop-archi-gate`「门禁只挂 implement 相位」明文写着「initialize 不挂：基线相位只做基线恢复与基线绿门（static-check / test-execution），基线里的架构债不属于本次变更」）。这条理由与本变更的目标**正好相反**：正因为基线里的架构债不属于本次变更，它才必须在流程启动时被拒之门外，而不是等 implement 相位第一次跑门禁时才以「本次变更的反馈」形态烧掉 3 次反馈预算。

### 病灶一 · 架构债的第一次判定发生在「已经写了代码之后」

implement 相位的 ArchiCheck 是 `FeedbackLoop { limit: 3 }`——失败即把违例注入同一 executor 会话要求修复。若违例其实来自基线（模型与代码早已不一致、模型文件本身结构错），executor 面对的是**本次变更碰不到也修不完**的既存债：要么硬改代码/模型把违例压下去（正是 runner 固定处置提示 `FIX_HINT` 明令禁止的那类动作），要么耗完 3 次预算升格相位 fail 再把问题还给用户。基线本可以在流程启动的一秒内把同一事实说出来。

### 病灶二 · 基线的架构检查不能复用 implement 的增量口径（会恒空转）

implement 门禁的观察口径是**增量**：`steps.rs` 用 `vcs.change_edit_set(root, "HEAD")` 取本 change 的编辑集，只扫其中的源文件。这个口径在 initialize 时刻必然退化：

| 事实 | 依据 |
|---|---|
| initialize 时刻 worktree 相对 `HEAD`（= fork 基线 `base_commit`）**没有任何源文件改动**——`Clean` 步刚把 tracked 改动 reset、untracked 清掉（排除面只留依赖树、dot 目录与 `openspec/`） | `walker.rs` `step_clean` + `restore_baseline` 排除面组装 |
| `change_edit_set` = `git diff HEAD --name-only` ∪ untracked；此刻 untracked 只有本 change 自己的 `openspec/changes/<name>/**`（`explore.md` 与报告树），扩展名不在架构扫描白名单内 | `crates/infra/vcs/src/git.rs:690`；`scan` 只收 `rs` / `ts` / `tsx` / `js` / `jsx` |
| 因而增量口径在 initialize 恒落 `scan.scannedFiles = 0` → `status = noChanges` → runner 文案「未检查（无可判文件）」→ `passed = true` | `drift::derive_status`（`scanned_files == 0 → NoChanges`）；runner `compose_text` |
| 同形态实测：现网 `run_node` 数据里多条 `archi_check` 节点（如 `run-1794837878254`、`run-1794822829601`）detail 逐字为「未检查（无可判文件）」——只改文档的 change 在 implement 门禁即得此结论 | db `run_node`（`step = archi_check`） |

即：**把门禁原样搬到 initialize，得到的会是一个永远「未检查」的通过步**——比不挂更坏，因为它把「没检查」冒充成「基线架构没问题」（正是 `desktop-archi-gate`「未检查不等于通过」那条纪律要防的事）。

### 病灶三 · 全量口径在门禁面没有落盘先例

`desktop-archi-drift`「全量与增量两模式与观察根」写死了「全量模式：落盘 = 否（一次可重算的观测，落盘只会积累垃圾）」——那是**命令面**（`/architecture` 页手动体检）的口径。基线门禁是审计面（run 的终态 reason 必须能把人指回一份可读报告），全量报告需要落盘，且必须落 `reports/archi`（`ReportSlot::Archi`）——`reports/baseline` 是 checks 域 schema（summary 索引 + 覆盖率复用门），混入漂移报告 JSON 会互相污染（`desktop-checks-domain` 已明令 checks 两 runner 拒绝 `archi` 槽位，反向同理）。

### 本仓基线实测（决定「接入后会不会立刻红」）

| 项 | 实测值 |
|---|---|
| `status` | `clean` |
| `errors` / `warnings` / `model.structuralErrors` | `0` / `0` / `0` |
| `scan.scannedFiles` / `scan.excluded` | `396` / `329` |
| `scan.matched` / `scan.unmatchedFiles` | `0` / `396` |
| `model.elements` / `model.relations` / `by_gate` | `53` / `120` / `{ strict: 0, warn: 0, off: 53 }` |
| `apiFindings`（纳管子集） | `41`（`0` 条纳管 ⇒ `status` 不被抬到 `warnings`） |

结论：**本仓当前接入该步不会阻断任何流程**（全量判定 clean）。同一份数据也解释了「为什么这个变更不是多余的」：53 个元素全 `off` ⇒ implement 门禁在本仓近于空转，基线全量判定是唯一会真正读全仓的架构判据。

---

## 提案

### 一 · 步序列：`Clean → StaticCheck → ArchiCheck → TestExecution`

| 决策 | 理由 |
|---|---|
| 插在 StaticCheck 之后、TestExecution 之前 | 代价升序 + 失败即停：static-check（秒级，先确认「能不能编译/过 lint」）→ archi-check（进程内判定、零 spawn，判定域 p95 预算 3 s）→ test-execution（本仓实测 480 s）。基线架构红需要用户回基线动手，让它在一次 8 分钟测试跑之前露头，比之后露头便宜得多。既有两步的**相对次序不变**（static 仍在 test 之前） |
| `on_fail = Terminal` | 与同相位两步同语义：无反馈边、零 WorkerAgent 会话、无自动改代码——基线的债不属于本次变更 |
| `Clean` 步仍 `on_fail = None` | 不变（失败即步终态） |
| 两表共享同一 `INITIALIZE_STEPS` | test-only 表头同源继承该步；MUST NOT 为 test-only 另写一份 static（单源纪律，既有测试以指针等值断言） |
| legacy 记录不受影响 | `worktree = None` 走整相位短路（`enter_phase` + `settle_phase(skipped)`），新步随之零执行、零判定 |

### 二 · 观察口径：全量（Full）而非增量

- **initialize 步显式声明全量**：`ToolCommand::ArchiCheck { change_id, mode: ArchiCheckMode::Full }`。
- **全量模式零 vcs 依赖**：`steps.rs` 在全量臂 MUST NOT 调 `change_edit_set`（既省一次 git spawn，也避免「取数失败 → 步失败」这条与基线架构无关的失败面）；增量臂逐字保留既有取数、失败显式传播、`MUST NOT 降级空清单`。
- **port 词汇单点**：`ArchiCheckMode { Full, Incremental }`（封闭两值、纯进程内枚举——零 serde / 零 specta，不进 IPC），`as_str()` 出线词 `full` / `incremental` 与 `ArchiDriftMode` 同词面；映射只在 `infra/archi` 一处。
- **模式由消费面声明，runner MUST NOT 自行推断**：runner 收到什么模式就判什么模式（`mode` 是入参，不是派生量），与「不设恒一值形参」的既有纪律不冲突——该形参现在取两值。

### 三 · 结论文义：与 implement 门禁同一判据，落盘分树同树

```
passed = errors.is_empty() && model.structuralErrors.is_empty()
  status = errors                        → 步 failed → Terminal::failed(TXT_BASELINE_ARCHI_FAILED)
  status = clean / warnings              → 步 passed（detail 携模式 / 状态词 / 扫描文件数 / 违例或提示条数）
  status = skipped（无模型）             → 步 passed，detail 逐字「未检查（无模型）」——MUST NOT 写成「检查通过」
  status = noChanges（零可判文件，全量亦可发生）→ 步 passed，detail 逐字「未检查（无可判文件）」
  runner Err（看护配置非法 / 扫描硬失败）→ run_tool 直达 Terminal（「archi-check 失败: <内层文案>」）
  fault = Stopped（用户停止）            → 节点 stopped + run stopped（fault 先于结论）
```

- **`warnings` 不拦**：与 implement 门禁同一 `passed` 定义（`desktop-archi-gate` 既有条款），基线不另立更严判据——否则同一份代码在基线与 implement 两处得到相反结论。
- **报告落 `reports/archi`（`ReportSlot::Archi`）**：与 implement 门禁**同树**，靠报告体自述（`mode` / `change` / `timestamp`）与文件名时间戳区分；`reports/baseline` / `reports/test` 仍只放 checks 域 schema，三槽位边界不挪。写盘失败降级 `report_ref = None` 不阻断结论（runner 既有不变量）。
- **零模型工作区不阻断**：`skipped → passed`，detail 逐字「未检查（无模型）」——本应用服务于任意工作区，未接入架构模型是合法形态，MUST NOT 让它在 initialize 卡死。

### 四 · 失败文案：新增一条基线专用 Template

```text
TXT_BASELINE_ARCHI_FAILED =
  "基线架构检查未通过，请在基线上修复模型或代码后重新发起（基线的债不属于本次变更）；报告目录 {dir}"
```

- 与 `TXT_BASELINE_STATIC_FAILED` / `TXT_BASELINE_TEST_ERROR` / `TXT_BASELINE_TEST_FAILED` 同族、同 `{dir}` 占位（值 = `outcome.report_dir`，即 `reports/archi`），**不共用文案**（三态不共用文案是既有纪律）。
- `fault = Environment` 在 archi runner 生产路径不可达（零进程），故不新增环境文案；`Stopped` 走 `settle_fault`（`template = None`，与 implement 步的 archi 臂同形）。

### 五 · 判据面的机械补位（同批必须）

| 面 | 现状 | 本次动作 |
|---|---|---|
| `OnFail::Terminal` 的 doc | 「消费点仅 initialize 相位两检查步 StaticCheck / TestExecution」 | 补 `ArchiCheck`（三检查步） |
| `StepNode::ArchiCheck` 的 doc | 「定向反馈边、零进程」 | 改述为「按 `on_fail` 两态：`FeedbackLoop`（implement）/ `Terminal`（initialize），零进程」 |
| `ReportSlot::Baseline` 的 doc | 「initialize 相位两检查步」 | 「initialize 相位 static-check / test-execution 两步（archi 基线检查走 `Archi` 槽位）」 |
| `phase_table.rs` `INITIALIZE_STEPS` doc | 「`Clean → StaticCheck → TestExecution`：…两检查步」 | 四步序 + 三检查步 + 插入理由一句 |
| 计划图守卫测试 | `FeedbackLoop` 白名单 + `Escalate` 归属守卫 | 补 `ArchiCheck` 的 `Terminal` 白名单（仅 initialize）；`Escalate` 仍不得落 `archi_check` |
| `RunStepKind` 的 doc | 二十二字词汇表（**已含 `archi_check`**） | **零改动**——该 doc 直出 `bindings.ts`，动它就制造无意义生成物 diff |

### 六 · 明确不做（出 range）

| 项 | 理由 / 归属 |
|---|---|
| 判定规则（扫描范围 / 归属 / 定级 / 豁免） | `desktop-archi-drift` 的既有契约，本变更零改动 |
| 新增 IPC 命令或出线类型 | 基线步不经命令面；`ArchiCheckMode` 是进程内 port 词汇，零 specta |
| 把契约零 diff（`api:check`）也挂进 initialize | 基线 static-check 已跑 `pnpm check`（`server:check → bindings:export → api:check → client:check`），重复挂只会多一个同源红点 |
| 把 archi 基线报告改落 `reports/baseline` | 三槽位语义分树是既有纪律（checks schema vs 漂移 JSON） |
| 为 initialize 引入可配置开关 / 按工作区豁免 | 门禁挂载点只由相位表声明承载，「无运行时开关」条款不变 |
| 前端新面板 / 新节点类型 | `archiCheck` 的 `STEP_LABEL` / 归一 / 分组徽章已在案（`desktop-change-flow-view` 上一变更落地），新节点直接吃既有渲染 |
| 基线红时自动修复模型或代码 | 零自动改写是 runner 不变量的既有条款 |
| gate 档位从 `off` 上抬（ratchet） | 独立议题：本变更只接入判据，不改模型数据 |

---

## 能力

| 能力 | 分类 | 变更内容 |
|---|---|---|
| `desktop-archi-gate` | **修改** | ① RENAMED「门禁只挂 implement 相位」→「门禁挂载点：implement 反馈边与 initialize 基线终态」（两挂载点表 + initialize 必经 Scenario + 其它相位零触点 Scenario 收敛 + 无运行时开关保留）；② MODIFIED「ArchiCheck 步结论语义与反馈边」：补 `on_fail` 两态分派（`Terminal` 单发终态：零反馈边、零会话、零自动改码、`warnings` 不拦、`skipped` / `noChanges` 照旧不等于通过）+ 基线终态文案；③ MODIFIED「ArchiCheck runner port 与报告落盘」：`ArchiCheckMode`（`Full` / `Incremental`）进 port 与 `ToolCommand`、两次调用点同树落 `reports/archi`、全量 mode 零 vcs；④ MODIFIED「门禁输入口径与观察根」：两挂载点的观察口径分叉（增量经 `change_edit_set` 唯一入口；全量零变更集、零 vcs 触碰） |
| `desktop-change-orchestration` | **修改** | ① MODIFIED「initialize 基线检查相位执行体」：步序四步、ArchiCheck 单发终态执行体（fault 先于结论 / 三态收口 / `TXT_BASELINE_ARCHI_FAILED` / 报告槽位 `Archi` / `skipped` 不冒充通过）、一次性语义与 legacy 短路覆盖新步；② MODIFIED「ArchiCheck 步接线与反馈边」：接线从「implement 专用」改为「按 `on_fail` 分派两执行体」，反馈边条款逐字保留并显式限定 implement |
| `desktop-workflow-write-face` | **修改** | MODIFIED「相位定义有序 step 序列与 artifact 清单」：`INITIALIZE_STEPS` 三步 → 四步（`Clean → StaticCheck → ArchiCheck → TestExecution`）、`ArchiCheck` 在 initialize 表项为 `Terminal`、`Escalate` 归属守卫不变、`ArchiCheck` 允许出现在 test-only 表（经共享表头）而仍不得出现在 test-only 的其它相位 |
| `desktop-change-flow-view` | **修改** | MODIFIED「initialize 相位列与 clean 步词呈现」：新 run 的 initialize 节点由三变四（多 `ArchiCheck`）、archi 报告不经 `outputRef`（零 exec-outputs）、基线失败终态 reason 补 archi 文案一类；`STEP_LABEL` / 归一 / 分组徽章零改动 |
| `desktop-archi-drift` | **修改** | ① MODIFIED「全量与增量两模式与观察根」：全量模式的「落盘 = 否」改述为「由消费面决定——命令面不落盘、门禁面（initialize 基线检查）落 `reports/archi`」，判定域自身仍零写盘；② MODIFIED「报告结构与 status 五值」：删「全量模式仅进程内返回」的绝对表述；③ MODIFIED「判定单源与消费面同源」：消费面两处 → 三处调用点（命令面 / implement 门禁 / initialize 基线门禁，后两者同一 runner），模式由消费面声明、runner 零推断 |
| `desktop-checks-domain` | **修改** | MODIFIED「baseline 报告槽位参数化」：补一条边界——initialize 的**架构**基线检查不由 checks 域两 runner 承担、也不走 `baseline` 槽位（落 `archi` 槽位），槽位三值与其语义逐字不变 |

| 不涉及 | 理由 |
|---|---|
| 新增能力 | 无新用户可见能力：既有步词、既有 runner、既有报告树，只多一个挂载点 |
| `desktop-change-state-store` | 零新步词（`archi_check` 已在二十二值内）、零新表、零字段 |
| `desktop-test-execution` | baseline 槽位与复用门语义零改动（test-execution 步本身一字未动） |
| `desktop-ipc-type-bindings` | 零新命令 / 零新出线类型；`ArchiCheckMode` 不进 specta ⇒ `bindings.ts` 生成物零 diff |
| `desktop-change-queries` / `desktop-phase-prompts` / `desktop-workspace-config` / `repo-archi-model` | 零字段、零 prompt、零配置键、零元素 / 关系变化（全部改动落在既有元素内） |
| `desktop-archi-api` / `desktop-archi-api-check` | 契约资产与判据词汇零改动（`api:check` 仍由基线 static-check 承担） |

---

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `crates/core/workflow/src/write/phase_table.rs` | `INITIALIZE_STEPS` 插 `StepDefinition { node: ArchiCheck, prompt_ref: None, on_fail: Some(OnFail::Terminal) }`（位于 `StaticCheck` 与 `TestExecution` 之间）；doc 同步（四步序 + 插入理由） |
| `crates/core/orchestration/src/port.rs` | 新增 `ArchiCheckMode { Full, Incremental }`（封闭两值 + `as_str`，零 serde / 零 specta）；`ToolCommand::ArchiCheck { change_id, mode }`；`ArchiCheckRunner::run(root, name, mode, changed, cancel)`；doc 同步（含「两挂载点同树落盘」一句） |
| `crates/core/orchestration/src/steps.rs` | `ArchiCheck` 臂按 `mode` 分派：`Incremental` → 既有 `change_edit_set` 取数（失败显式传播）；`Full` → `changed = Vec::new()`、**零 vcs 调用**；runner 调用点透传 `mode` |
| `crates/core/orchestration/src/walker.rs` | ① 新增 `TXT_BASELINE_ARCHI_FAILED`；② `step_archi_check` 改为按 `on_fail` 分派（`Terminal` → 新 `step_archi_check_terminal`；`FeedbackLoop` → 既有反馈边体原样搬入 `step_archi_check_feedback`；`Escalate` / `None` → 计划图漂移 `Err`）；③ 终端执行体与 `step_static_check_terminal` 逐段同型（fault 先行 / passed 收口 / failed Terminal 携 `{dir}`） |
| `crates/core/workflow/src/state.rs` | `OnFail::Terminal` 的 doc 补第三处消费点（三检查步）、`StepNode::ArchiCheck` 的 doc 改述为「按 `on_fail` 两态」——两者均**非出线类型**（`OnFail` 与 `StepNode` 不进 specta），`RunStepKind` 的 doc 逐字不动（它直出 `bindings.ts`） |
| `crates/core/foundation/src/layout/mod.rs` | `ReportSlot::Baseline` 与 `ReportSlot` 的 doc 措辞：两检查步 → static-check / test-execution（archi 基线检查走 `Archi` 槽位）——**零逻辑改动** |
| `crates/infra/archi/src/runner.rs` | `run` 收 `mode`，映射到 `ArchiDriftMode`（唯一映射点）；`changed` 全量模式忽略；其余（落盘 / 降级 / 取消 / 文本装配）逐字不动 |
| `packages/desktop/src-tauri/src/commands/change_flow/mod.rs` | 零改动（`LocalToolSteps` 注入面与四条 runner 装配不变）——**列出以明示不在范围**，若实现期需要动它即偏离本提案 |
| `packages/desktop/package.json` | `version` `0.6.47 → 0.6.48`（`tauri.conf.json` 经 `../package.json` 跟随；`src-tauri/Cargo.toml` 的 crate 版本行 MUST NOT 随动） |

**生成物与资产零 diff 的判据面**：`packages/desktop/src/types/generated/bindings.ts` / `bindings.zod.ts`（`bindings:export` 后零 diff——无新命令 / 新出线类型，`RunStepKind` doc 不动）、`.dev-team/architecture/**`（零模型 / 契约 / gate 改动）、`openspec/specs/**`（只在 change 目录出 delta）。三者以 `git diff --stat` 机械核对（AC-9）。

### 测试文件

| 文件 | 新增 / 改写 |
|---|---|
| `crates/core/workflow/src/write/phase_table_test.rs` | `INITIALIZE_STEP_ORDER` 3 → 4 元素；initialize 步序 / 步数断言（3 → 4）逐处改；`expected_on_fail` 增 `ArchiCheck if initialize → Terminal`；`on_fail` 白名单守卫（`Terminal` 允许集补 initialize 的 `ArchiCheck`；`Escalate` 仍拒）；`on_fail回归锚` 组一扩为三检查步；`两表initialize共用同一static单源` 保持绿（末步仍 `TestExecution`）；doc 注释步序串断言（「`Clean → StaticCheck → TestExecution`」→ 四步）同步 |
| `crates/core/orchestration/src/walker_test.rs` | `NullArchiRunner` / `CountingArchiRunner` 签名补 `mode`；`FakeTools` 的 `ToolCommand::ArchiCheck` 臂补字段；initialize 全绿链用例：emit 序 → `clean / static_check / archi_check / test_execution`、forward 边 3 条、四节点恒 passed；新增 archi 红 → `Terminal::failed` 携 `TXT_BASELINE_ARCHI_FAILED` + `{dir}`、PhaseRecord 零条目、零会话 / 零反馈边；`skipped` → passed 且 detail 逐字「未检查（无模型）」；取消 → 节点 stopped + run stopped；`Escalate` 误声明 → 计划图漂移 `Err` |
| `crates/core/orchestration/src/steps_test.rs` | 既有四处 `ToolCommand::ArchiCheck { change_id }` 构造补 `mode`；`RecordingArchiRunner` 记录入参模式：全量臂 vcs **零调用**（FakeVcs 调用计数）+ `changed` 空；增量臂照旧经 `change_edit_set`、取数失败仍显式 `Err` |
| `crates/infra/archi/src/runner_test.rs` | 「恒增量模式」断言改述为按入参模式（`Incremental` 报告 `mode = Incremental`、`Full` 报告 `mode = Full`）；新增全量用例（零变更集 + 真实模型 → 报告落 `reports/archi`、`scan.changedFiles = 0`、`passed` 随 errors 空否） |
| `crates/core/orchestration/src/port_test.rs` | `ArchiCheckMode::as_str` 逐值对拍（`full` / `incremental`）与枚举封闭性断言 |
| `crates/infra/checks/src/mutation/mod_test.rs` | 组合根链路占位假件 `UnusedArchiSeam` 的 `run` 签名补 `mode` 形参（零行为变化——该臂恒 `Err`，用例断言不变）；checks 域**实现**零改动 |
| 无新增测试文件 | 前端零代码改动（`archiCheck` 标签 / 归一 / 分组已在案），故不新增前端用例；`desktop-change-flow-view` 的四节点口径只由后端 emit 序用例与 spec 文本承载 |

### 不要修改

| 对象 | 理由 |
|---|---|
| `crates/core/archi/src/drift/**`、`crates/core/archi/src/scan/**` | 判定规则零改动（本变更只换观察口径与落盘面） |
| `crates/core/archi/src/{dto,api}/**`、`.dev-team/architecture/**` | 零出线类型、零契约资产、零模型数据（`api:check` 与 `archi` 判定面应零 diff） |
| `crates/core/orchestration/src/port.rs` 的 `ReportSlot` 语义 / `crates/infra/checks/**`（实现面） | 三槽位分树语义不变；checks 两 runner 仍拒绝 `archi` 槽位（该 crate 的唯一改动是组合根用例占位假件的签名跟随，见「测试文件」表） |
| `crates/infra/archi/src/{report.rs,api_artifacts.rs}` | 落盘单点与文件名规则不变 |
| `crates/core/workflow/src/state.rs` 的 `RunStepKind` doc | 二十二值词汇已在案；该 doc 直出 `bindings.ts`，动它即制造无意义生成物 diff |
| `packages/desktop/src/views/changes/flow/**`（含 `run-state.ts` / `run-step-node.tsx`） | 步词呈现面已在案，新节点零代码改动直接渲染 |
| `crates/core/workflow/src/prompts/**`（14 份角色 md） | initialize 为确定性相位（零 agent 会话），零 prompt 面改动 |
| `.dev-team/config.json` / `crates/core/config/**` | 零配置键（门禁挂载点不可配置） |
| `openspec/changes/init-check-archi/explore.md` | 只读（目标原文留底） |

---

## 验收标准

| ID | 验收条件 | 验证方法 |
|---|---|---|
| AC-1 | **相位表四步单源**：两表 `initialize` 的 `steps` 逐位为 `Clean → StaticCheck → ArchiCheck → TestExecution`（指针等值证明同一 `static`）；`ArchiCheck` / `StaticCheck` / `TestExecution` 三步 `on_fail = Terminal`、`Clean` 为 `None`；两表其余相位零 `ArchiCheck`（test-only 的 initialize 之外） | `cargo nextest -p workflow`（`phase_table_test`） |
| AC-2 | **port 词汇与分派**：`ArchiCheckMode` 封闭两值、`as_str` 逐值 `full` / `incremental`；`ToolCommand::ArchiCheck` 携 `mode`；`ArchiCheckRunner::run` 收 `mode` | `port_test` + 编译期穷尽匹配 |
| AC-3 | **全量零 vcs**：initialize 全量臂 vcs 缝零调用（`FakeVcs` 调用计数为 0）、`changed` 为空、runner 收到 `Full`；增量臂逐字保留（取数失败 → 步失败、`MUST NOT` 降级空清单） | `steps_test` + `walker_test` |
| AC-4 | **全量落盘**：以 `Full` 跑一次真实模型 —— 报告落 `reports/archi/<timestamp>.json`、报告体 `mode = Full`、`change` = change 名、`report_ref` 为该文件；`reports/baseline` / `reports/test` 字节零变化 | `infra/archi` `runner_test` |
| AC-5 | **三态收口**：`clean` / `warnings` → 节点 passed + Proceed；`errors`（含结构错）→ 节点 failed + `Terminal::failed(TXT_BASELINE_ARCHI_FAILED 替换 {dir})`；`skipped` → passed 且 detail 逐字「未检查（无模型）」；零可判文件 → detail 逐字「未检查（无可判文件）」；取消 → 节点 stopped + run stopped | `walker_test` |
| AC-6 | **零会话零反馈边零改写**：archi 基线红时零 WorkerAgent 会话注入、反馈边计数为 0、模型与源码字节零变化、PhaseRecord 零 initialize 条目；再发起时 Clean 后干净重试 | `walker_test` |
| AC-7 | **全绿链四节点**：initialize 全绿 run 的 emit 序为 `clean / static_check / archi_check / test_execution`，forward 边 3 条、四节点恒 passed、零 `phaseStart` / `phaseLog` 节点；全绿后持久跳过语义不变 | `walker_test` |
| AC-8 | **本仓基线实证（不作为 MUST，登记留痕）**：全量判定在改动前后均为 `status = clean`（`errors = 0` / `warnings = 0` / `structuralErrors = 0` / `scannedFiles = 396` / `by_gate` 全 `off`）⇒ 接入后本仓任一新 change 的 initialize 不被阻断 | 手工全量判定（`archi_drift` 命令 full 模式）输出留痕 |
| AC-9 | **生成物与资产零 diff**：`bindings:export` 后 `bindings.ts` / `bindings.zod.ts` 零 diff；`.dev-team/architecture/**`（模型 / 契约 / `INDEX.toml` / `config.json`）零 diff；`api:check` 零差异 | `git diff --stat` + `pnpm api:check` |
| AC-10 | **口径一致（机械断言）**：源码注释与 spec 中「initialize 相位两检查步」「三节点（Clean / StaticCheck / TestExecution）」两串零残留（`grep -rn '两检查步' packages/desktop/src-tauri/crates` 零命中，或仅剩「架构基线检查走 Archi 槽位」类改述）；`TXT_BASELINE_*` 四条文案互不相同（三检查步不共用文案） | `grep` + `walker_test` 文案断言 |
| AC-11 | **门禁与版本**：`pnpm -C packages/desktop run check` 绿（`server:check` → `bindings:export` → `api:check` → `client:check`）；`cargo nextest run --workspace` 全绿；`package.json` `0.6.47 → 0.6.48`、`src-tauri/Cargo.toml` crate 版本行不变 | 门禁命令 + `git diff` |

**AC ↔ spec requirement 对应**：

| AC | spec requirement / scenario |
|---|---|
| AC-1 | `desktop-workflow-write-face` MODIFIED「相位定义有序 step 序列与 artifact 清单」 |
| AC-2 / AC-3 | `desktop-archi-gate` MODIFIED「ArchiCheck runner port 与报告落盘」+ MODIFIED「门禁输入口径与观察根」 |
| AC-4 | `desktop-archi-gate` MODIFIED「ArchiCheck runner port 与报告落盘」+ `desktop-archi-drift` MODIFIED「全量与增量两模式与观察根」 |
| AC-5 / AC-6 / AC-7 | `desktop-change-orchestration` MODIFIED「initialize 基线检查相位执行体」+ MODIFIED「ArchiCheck 步接线与反馈边」+ `desktop-archi-gate` MODIFIED「ArchiCheck 步结论语义与反馈边」 |
| AC-8 | 无 requirement 对应（实证留痕，写进提案「问题」与风险 R-1） |
| AC-9 / AC-11 | 工程验收面（`desktop-archi-drift`「判定单源」与 `desktop-checks-domain`「baseline 报告槽位参数化」的零改动条款背书） |
| AC-10 | `desktop-change-flow-view` MODIFIED「initialize 相位列与 clean 步词呈现」+ `desktop-archi-gate` RENAMED 挂载点 requirement |

---

## 风险

| ID | 风险 | 影响 | 缓解 |
|---|---|---|---|
| R-1 | **基线一旦真实有违例，会阻断所有新 change** | 这是设计意图（先修基线），但用户若把模型 gate 从 `off` 上抬到 `strict` 且代码未跟上，会一次把全部在途流程挡在 initialize | 终态文案明写「请在基线上修复模型或代码后重新发起」并携报告目录；本仓当前全 `off` ⇒ 接入当刻零阻断（AC-8 实证）；gate 档位上抬是独立议题，MUST NOT 与本变更同批 |
| R-2 | **全量扫描的启动时延** | 每次 run 首次 enter initialize 多一次全仓扫描（本仓观察根 396 个源文件；判定域 p95 ≤ 3 s 的预算条款已在 `desktop-archi-drift`「运行期预算与语料」在案，本变更不新增预算） | 该步 `warnings` 不拦、失败即停；实测数据登记在本提案；若未来超大仓库超预算，改「按元素根分段 + 增量语义」需另立变更，MUST NOT 在本变更内放宽判据 |
| R-3 | **报告树共用（`reports/archi`）被误读** | initialize 与 implement 的报告同目录，人工排查时可能取错文件 | 报告体自述 `mode`（`full` / `incremental`）+ `change` + `timestamp`；文件名即时间戳；节点 detail 携模式与状态词；MUST NOT 新增第四槽位（三槽位语义分树是既有纪律） |
| R-4 | **port 签名变更的机械扩散** | `ArchiCheckRunner` 的五个假件（`NullArchiRunner` / `CountingArchiRunner` / `UnusedArchiRunner` / `RecordingArchiRunner` / `UnusedArchiSeam`）与 `FakeTools` 的命令臂、`steps_test` 的四处 `ToolCommand` 构造点全需补字段，漏一处即编译红 | 机械面一次改齐（AC-2 / AC-3）；新增字段放 `run` 第 3 位（`mode`）与 `ToolCommand` 第 2 位，形式与既有 `slot` 先例同型 |
| R-5 | **「两检查步」口径残留** | 该措辞散在 `layout.rs`、`phase_table.rs`、`state.rs`、三份 spec 与多处测试注释——漏改即同一事实两种自称（本仓历史高频病灶） | AC-10 以 `grep` 机械断言；改述清单逐条列在提案 §五 |
| R-6 | **runner `Err`（看护配置非法 / 扫描硬失败）文案不带基线引导** | 该臂经 `run_tool` 直达 `Terminal::failed("archi-check 失败: …")`，用户需自行理解为基线资产问题 | 内层文案已点名非法条目（`drift::config` / `scan` 的既有显式文案）；与 initialize 既有两步的 `Err` 臂同形（static-check 亦然），MUST NOT 为 archi 单开包装文案（会造成「同一相位三种 Err 文案形态」） |
| R-7 | **未接入架构模型的工作区** | 若 `skipped` 被判成失败，所有未用架构域的工作区会永久卡在 initialize | `skipped → passed` + detail 逐字「未检查（无模型）」（AC-5 反向钉住）；该文案 MUST NOT 被改写成「检查通过」 |
| R-8 | **`Escalate` 误落到 `archi_check`** | 升格截断余下步依赖步序，误声明会静默失效 | 计划图守卫测试补 `Escalate` 拒收臂（AC-1）；执行体对 `Escalate` / `None` 一律 `计划图漂移` `Err`（不静默 `Proceed`） |
| R-9 | **全量模式在 `scannedFiles = 0` 时落 `noChanges`** | 全空模型工作区（模型在、源码全排除）会得「未检查（无可判文件）」并通过——诚实但可能被误读成「检查过」 | 文案逐字「未检查（无可判文件）」而非「通过」（`desktop-archi-gate`「未检查不等于通过」既有条款覆盖全量模式） |

---

## explore 未决项的归属

`explore.md` 只有一句目标原文（无展开笔记、零追加 re-explore），本提案已把其中的「初始阶段 / 检查架构」两面按代码事实落成设计：**初始阶段** = `initialize` 表头相位（既有一次性基线检查相位，非新相位）；**检查架构** = 既有 `archi_check` 步词与 `archi-runtime` runner 的**全量模式挂载**（非新判定规则）。以下为本提案自行裁定的三点，留档以免后续变更重开：

| 待决 | 本提案裁定 |
|---|---|
| 是否需要新相位或新步词 | 都不要：`initialize` 与 `archi_check` 皆已在案，扩挂载点即达标 |
| 基线检查用全量还是增量 | 全量（增量在 initialize 恒 `noChanges`，见「问题 · 病灶二」） |
| 基线失败是否也走反馈边 | 不走：`Terminal`（与同相位两步同语义；基线债不属于本次变更） |
