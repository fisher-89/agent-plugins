# desktop-change-orchestration Specification (Delta)

## MODIFIED Requirements

### Requirement: run 双 root 组合与 exec root 解析

change run SHALL 以双 root 组合执行（能力语义见 desktop-change-worktree）：`change_flow_start` SHALL 解析 exec root——记录 `exec_root` 在案 → 该绝对路径；`exec_root=None` 且 `worktree` 在案 → 该 worktree 绝对路径；两者皆 `None`（legacy）→ 主 workspace root（访问器单点，`is_dir` 校验与文案沿用）——`RunRequest.root` SHALL 恒为 exec root，会话 cwd（`turn.root → SessionCtx.workspace_root`）、工件根（`Layout::resolve`）、git diff 与检查器（static-check / test-execution）执行目录 SHALL 全部随 exec root 落位。`RunRequest` SHALL 另携 `worktree: Option<String>`（**host**，即 git 落位目录；legacy → `None`）作 git 步与回收的唯一锚：walker 的 git 步 MUST NOT 从 exec root 反推 host（`reset --hard` 全树 vs `clean -fd` / `ls-files` 子树，反推即半域 / 缩域），legacy 判据 SHALL 读 `request.worktree.is_none()`、MUST NOT 以 `request.root == request.main_root` 这类路径相等代理判定（子目录 workspace 下二者恒不等，代理的成立只是巧合）。store / db 半边 SHALL 恒以 workspace root 解析：`compose_turn` 的 store 半边为命令层预解析注入的 workspace root 库实例，worktree 路径 MUST NOT 进入 `for_root`；`StoreSnapshot` 的 fs 半边（产物发现 / detail 装配）取 exec root、db 半边取注入的 store 实例（既有构造形态天然满足）。相位状态机零 fs、会话 resume 链（记录不存 cwd）MUST 保持零改动。SDK 路径沙箱前缀校验以 exec root 为锚（既有机制零改动），worktree change 的文件编辑囚于 worktree 内；worktree 内 `git diff HEAD` 恰为本 change 编辑集（「变更文件上下文降级 git diff」requirement 的 diff 面语义在 worktree 内自然收窄，无模板改动）。

#### Scenario: worktree run 全链落执行根

- **WHEN** 以带 worktree 记录的 change 发起 run（假引擎驱动一相位）
- **THEN** executor / evaluator 会话 cwd = 执行根（根 workspace 下即 worktree 根）、static-check 与 test-execution spawn 的执行目录 = 执行根、git diff 上下文取自 worktree 且仅含本 change 编辑集；`RunRequest.worktree` 携 host

#### Scenario: 子目录 workspace 的三根分离

- **WHEN** workspace root 为 git 仓库子目录（`rel = packages/desktop`）的 change 发起 run
- **THEN** `RunRequest.main_root` = workspace root、`RunRequest.root` = `host/packages/desktop`、`RunRequest.worktree` = `host`；会话 cwd / Layout / 检查器 spawn 均落执行根；归档与 initialize 的 git 步（`restore_baseline` / `worktree_snapshot` / `commit_all`）收到 host

#### Scenario: store 身份恒 workspace root

- **WHEN** 审查 worktree change run 的组合根装配与 compose_turn 调用
- **THEN** store 实例由命令层以 workspace root `for_root` 解析后注入（provider / 会话 / 建档 / StepRecord 落 workspace 库），全程无以 worktree 路径进 `for_root` 的调用点

#### Scenario: legacy run 零变化

- **WHEN** 以 `worktree=None` 的存量建档 change 发起 run
- **THEN** exec root 解析为主 workspace root、`RunRequest.worktree = None`（legacy 判据的唯一来源），既有 run 全链（cwd / Layout / diff / 检查器 / 快照）语义零变化

#### Scenario: 并行 change 各自 worktree

- **WHEN** 同 workspace 两个带各自 worktree 的 change 同时发起 run（假引擎）
- **THEN** 两 run 互不干扰（各自 exec root 的编辑 / diff / 检查隔离），db 写入经既有进程内并发支撑

### Requirement: initialize 基线检查相位执行体

Walker SHALL 以两表表头一次性 `initialize` 相位（`INITIALIZE_STEPS`，见 desktop-workflow-write-face「相位定义有序 step 序列与 artifact 清单」）承接变更流程启动前的基线绿门：步序 `Clean → StaticCheck → ArchiCheck → TestExecution`（开相与相位收口不在 `steps` 内，由 `enter_phase` / `settle_phase` 承载），全部**检查** SHALL 在 exec root 内执行（主仓未提交改动不进 worktree，基线语义纯净）；**恢复（Clean 步）SHALL 锚 host 全树**——`reset --hard` 覆盖全树而 `clean -fd` 只认 cwd 子树，锚 exec root 会得半域恢复（域外 tracked 改动被丢、域外 untracked 垃圾留下），故 Clean 的锚恒为 `RunRequest.worktree`（host）。initialize 相位 SHALL 为确定性相位（零 executor / evaluator / decision 会话，全程零 WorkerAgent）：

- **Clean 步**（vcs 直调先例同归档 git 步，不经 ToolCommand 枚举）：walker 经 `ArchiveVcsPort::restore_baseline(host, 排除面)` 恢复 worktree 至基线态（锚 = `RunRequest.worktree`（host），MUST NOT 传 exec root）——tracked 修改 reset、untracked 清除，排除面 = foundation 共享的依赖与工具产物树名单（node_modules / dist / build / target / coverage / .nyc_output + dot 目录，见 desktop-checks-domain「依赖与工具产物树名单共享单源」）+ `openspec/` 工件树（proposal 等过程文档与报告树为未跟踪面，MUST NOT 被 clean 删除）。Clean 步 MUST 仅在 host 在案（`RunRequest.worktree.is_some()`）时执行；失败即 `Terminal`（`on_fail=None`）。
- **StaticCheck / TestExecution 步**：复用既有检查域 runner（`StaticCheckRunner` / `TestExecutionRunner`，报告槽位为 baseline 变体，见 desktop-test-execution「baseline 报告槽位与树隔离」），MUST NOT 执行 mutation（基线检查明确不含突变）。两步声明 `on_fail=Terminal`：失败 SHALL 以 `Terminal` 收敛 run failed，MUST NOT 走反馈边（无 agent 会话可注入，重试基线无意义）。失败文案 SHALL 区分：test-execution `conclusion=Error`（环境 / 配置问题——引导查配置与环境后重新发起）与 `conclusion=Fail`（测试红——引导在基线上修复测试后重新发起）；static-check 失败引导在基线上修复后重新发起；文案 SHALL 携 baseline 报告目录路径供回查。
- **ArchiCheck 步（本挂载点为全量基线检查）**：复用既有架构漂移门禁 runner（`ArchiCheckRunner`，报告槽位恒 `ReportSlot::Archi`——**MUST NOT** 落 `reports/baseline`），命令 SHALL 为 `ToolCommand::ArchiCheck { change_id, mode: ArchiCheckMode::Full }`，变更集为空且 `steps.rs` MUST NOT 触碰 vcs 缝（全量模式忽略变更集；增量口径在基线上恒 `noChanges` = 空转，见 desktop-archi-gate「门禁挂载点」）。该步声明 `on_fail=Terminal`：`passed` → 节点 passed 后继续 test-execution；`failed`（`errors` 或模型结构错）→ 节点 failed + `Terminal::failed`（文案为基线架构专用模板，携 `reports/archi` 目录——三检查步不共用文案）；`skipped`（无模型）/ `noChanges`（零可判文件）→ 步 passed，但 detail MUST 逐字写「未检查（无模型）」/「未检查（无可判文件）」，MUST NOT 冒充「检查通过」（未接入架构模型的工作区 MUST NOT 因本步被阻断）；取消 → 节点 stopped + run stopped（fault 先于结论）。该臂 MUST NOT 发起 WorkerAgent 会话、MUST NOT 消耗反馈边计数、MUST NOT 自动改写模型或源码。

相位收口 SHALL 由 `settle_phase` 以机械 checklist 纯桌面代写（零 agent，沿 `test_execution_phase_log_input` 先例——steps 含 TestExecution 步则 `test_outcome` 自动挂载、三分支天然分派），report 指向 baseline 报告目录，三会话槽位恒 `None`；ArchiCheck 步的既存面（`mode=full`）MUST NOT 改变该分派（判据是「steps 是否含 TestExecution 步」，MUST NOT 引入按步新增分支）。

一次性语义 SHALL 由既有落账机制天然成立：任一步 `Terminal` 失败时 PhaseRecord 零 initialize 条目（图节点照常落 `RunNodeRecord` 留痕可回查）→ 再发起时 `phase_next` 仍路由 initialize、Clean 步清残留后干净重试；全绿 pass 落 PhaseRecord（机械 checklist，verdict=pass）→ 持久跳过（后续 run 不再重跑基线检查）。**三检查步在一次性语义上完全同权**：archi 基线红与 static 红、测试红一样，零 PhaseRecord 条目、零「部分检查已通过」残留。initialize 收口后的 phase 间停等 SHALL 与全相位一致（`auto_next_phase=false` 时照常 `ConfirmWait` 停等，零特判豁免）。legacy 记录（`ChangeRecord.worktree = None` ⟺ `RunRequest.worktree = None`——判据 SHALL 读后者，MUST NOT 以 `root == main_root` 路径相等代理）SHALL 整相位短路：步序列零执行、零 spawn、零判定（含 archi 全量扫描零执行）、主仓 MUST NOT 被 reset 或 clean，以 `enter_phase` → `settle_phase(skipped=true)` 收口（跳过因注明 legacy 主仓执行无 worktree 基线，一次性持久同 pass 语义）。

#### Scenario: 全绿序曲与一次性跳过

- **WHEN** 新建 requirement change 发起 run（基线 static-check、archi-check 与 test-execution 全绿，auto_next_phase=true）
- **THEN** initialize 相位先于 proposal 执行：Clean / StaticCheck / ArchiCheck / TestExecution 四节点按序上图（零 WorkerAgent 会话），checks 域两报告落 `reports/baseline`、漂移报告落 `reports/archi`；相位收口机械 checklist 落账 pass 后后续相位照常推进；同 change 再次发起 run 时 initialize 被持久跳过（零重跑）

#### Scenario: 基线 static-check 红 Terminal 停止

- **WHEN** 基线 static-check 失败（真件 shim 配置为红）
- **THEN** run 以 `Terminal` 收敛 failed，文案引导「在基线上修复后重新发起」并携 baseline 报告目录；PhaseRecord 零 initialize 条目、零反馈边注入；后续步（含 archi-check 与 test-execution）零执行；再发起时 Clean 步恢复基线后重试

#### Scenario: 基线 archi-check 红 Terminal 停止

- **WHEN** 基线全量架构判定返回 `errors` 非空或 `model.structuralErrors` 非空（且 static-check 已绿）
- **THEN** archi-check 节点 failed（detail 为诊断摘要）、test-execution 步零执行；run 以 `Terminal::failed` 收敛，终态文案为基线架构专用模板并携 `reports/archi` 目录；零 WorkerAgent 会话、零反馈边计数、模型与源码零改写；PhaseRecord 零 initialize 条目；再发起时 Clean 恢复基线后重跑四步

#### Scenario: 基线无模型不阻断

- **WHEN** 观察根下不存在架构模型（`<archi_dir>/models` 缺席或零 `*.toml`）
- **THEN** archi-check 节点 passed 且 detail 逐字「未检查（无模型）」，流程继续 test-execution；run 不因本步失败，报告 `status = skipped` 原样保留

#### Scenario: test-execution Error 与 Fail 分文案

- **WHEN** 基线 test-execution 分别以 `conclusion=Error`（框架探测失败 / spawn 失败等环境问题）与 `conclusion=Fail`（测试红）收场
- **THEN** 两者均以 `Terminal` 停止 run，但终态 reason 文案可辨：Error 引导查配置 / 环境，Fail 引导修基线测试；两态不共用同一段文案，且两者都与基线架构文案相异

#### Scenario: clean 恢复基线且排除面保留

- **WHEN** 上一次 run 失败后 worktree 残留未提交代码改动与 untracked 文件，再发起进入 initialize 的 Clean 步
- **THEN** tracked 修改被 reset、untracked 残留被清除，worktree 恢复基线态（**全树范围**：host 根与执行根子树内的残留一并清除）；node_modules / dist / build / target / coverage / .nyc_output 与 dot 目录、`openspec/` 工件树原样保留（依赖与过程文档不丢，后续基线测试可跑、已落产物与既存 `reports/archi` 报告可读）

#### Scenario: 子目录 workspace 的 Clean 锚 host 全树

- **WHEN** 子目录 workspace（`rel = packages/desktop`）的 worktree change 进入 initialize 的 Clean 步，且 worktree 内存在两类残留：执行根内的 tracked 修改、执行根之外的 tracked 修改与 untracked 文件（上一轮手工实验）
- **THEN** `restore_baseline` 收到 host（`RunRequest.worktree`）；host 全域的 tracked 修改被 reset、全域 untracked 被清除（无半域残留）；`openspec/` 工件树与依赖产物树原样保留；节点 detail 与既有文案形态一致

#### Scenario: legacy 整相位短路

- **WHEN** 以 `worktree=None` 的存量记录发起 run
- **THEN** initialize 步序列零执行（零 vcs / 零检查 spawn / 零架构全量扫描），主仓零 reset 零 clean，相位经 `enter_phase` + `settle_phase(skipped=true)` 落账（跳过因可见），后续相位照常路由

#### Scenario: 收口停等与全相位一致

- **WHEN** 手动档（auto_next_phase=false）run 的 initialize 相位收口落账 pass
- **THEN** run 照常进入 phase 间停等（`ConfirmWait`），用户确认后续走 proposal；无「发起即确认」特判豁免

## Module Contract

### 模块增量：三根分离与 Clean 锚 host（worktree-subdir-exec-root）

主基线既有行逐字沿用；本变更受影响模块如下：

| 模块 | 职责 | 关键契约（Function / API / CLI / Component） |
|------|------|------|
| `crates/core/orchestration/src/walker.rs`（`RunRequest`） | 运行请求 | 增 `pub worktree: Option<String>`（host；legacy → `None`）；`root` 恒 exec root、`main_root` 恒 workspace root 两条语义不变 |
| `crates/core/orchestration/src/walker.rs`（initialize 面） | 相位短路与 Clean | `step_clean` 的 `restore_baseline` 实参改 `request.worktree`（host）；initialize 短路判据与 Clean 双保险改 `request.worktree.is_none()`；其余步执行体逐字不变 |
| `crates/core/orchestration/src/steps.rs`（`run_archive_preflight`） | 归档前置载荷 | `ArchivePreflight.worktree` 填记录 host、`execRoot` 填记录执行根；`missing_artifacts` / `detect_delta_specs` 改吃执行根（见 desktop-change-archive） |
| `crates/core/orchestration/src/state.rs`（`ArchivePreflight`） | DTO | 增 `pub exec_root: Option<String>`（serde camelCase → `execRoot`）；既有七字段与语义不变 |
| `src/commands/change_flow/mod.rs` | 发起前置校验 | exec root 解析改记录访问器；`RunRequest.worktree` 由 `record.worktree` 直供 |
| `crates/core/orchestration/src/{walker,steps}_test.rs` | 测试 | 子目录夹具三根断言、Clean 锚 host 的全树断言、legacy 判据显式化 |
