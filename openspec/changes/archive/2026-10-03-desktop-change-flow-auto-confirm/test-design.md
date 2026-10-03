# 测试设计: desktop-change-flow-auto-next-phase

> **日期**: 2026-10-03

---

## 验收范围

| AC ID | 验收条件 | 被测文件或模块 |
|--------|---------|----------|
| AC-1 | walker `RunRequest.auto_confirm` 字段 + drive() 确认点分支：auto_confirm=true 的多相位 run 更新流零 `ConfirmWait` 信封、快照面全程不经 `waitingConfirm`，逐相位推进直通 `completed`（reason = "All phases have passed evaluation. Ready for archiving."）；walker_test 直测 | `packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs` |
| AC-2 | 手动模式分支保持：auto_confirm=false（默认）既有停等语义不变——`ConfirmWait` 逐相位流出、挂起至 `change_flow_confirm`，proceed=false / 等待期取消 → 受控 `stopped`（既有用例全绿） | `packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs` |
| AC-3 | `change_flow_start` + auto_confirm 透传：命令签名 + `auto_confirm: bool`，bindings 重导后 `changeFlowStart` 带 `autoConfirm`；mod_test 证明 auto 参数达 walker（事件/快照面无 `ConfirmWait`） | `packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs` |
| AC-4 | 前端发起开关：发起按钮旁开关默认关；开启后 start 携 `autoConfirm=true`；运行期开关定格不可切；测试以 data-testid 查询 | `packages/desktop/src/views/changes/flow/run-control-panel.test.tsx` |
| AC-5 | ask / 终态语义边界：auto 模式下决策 ask 照常进入 `waitingAsk` 停等应答（不自动选择）；全相位 pass 照常 `completed` 收口停给用户，不触发归档 | `packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs` |
| AC-6 | 版本：归档时 `packages/desktop/package.json` version 0.4.2 → 0.4.3（归档轨道，随归档提交执行） | —（见不可测试项 1） |
| AC-7 | 全管线：Rust / 前端自动化套件全绿；bindings 重导无 diff；knip 无新增豁免（执行聚合面） | —（见不可测试项 3） |

---

## 单元测试

### packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs

#### 待测功能

<!-- design.md `### 公共函数 / API` 表无 walker.rs 行（walk_run / new_run_id 签名零变化不列）；待测面取自 design.md 类型定义表 `RunRequest` 行与变更清单 walker.rs 行（D1 / D2 定稿）。 -->

- RunRequest.auto_confirm: 停等节奏发起定格字段（false 手动停等——phase 间挂起至 confirm 应答；true 跳过 phase 间停等——不进 `WaitingConfirm`、零 `ConfirmWait` 事件，ask / 失败 / 终态语义不变）
- walk_run drive() 确认点分支: `if !request.auto_confirm` 整体包住 `guard.emit(RunUpdate::ConfirmWait)` + `guard.wait_confirm().await` + 停等未获确认的 `Terminal::stopped` 收口（D1——emit 与 wait 原子同支，false 分支语句块零位移）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| walk_run auto_confirm=true 直通 | 正向 | auto_confirm=true 发起 ≥2 相位预录路由 run（全 pass）：逐相位 phase-start 在场（确经逐相位推进、非一次收敛）、收集更新流断言零 `confirmWait` 信封（ConfirmWait 信封是 `waitingConfirm` 态唯一入度——零信封即快照面全程不经 `waitingConfirm`）、Finished status=completed 且 reason 仍为 "All phases have passed evaluation. Ready for archiving."（终态口径不随 auto 分支漂移） | 新增 |
| walk_run auto_confirm=true 直通 | 边界 | auto_confirm=true 且不接 spawn_confirmer 测试半边：run 不挂起于 wait_confirm、自行推进至 completed（手动模式同 fixture 不接 confirmer 必停等挂起——直通的结构性证明） | 新增 |
| walk_run auto_confirm=true 直通 | 异常 | auto_confirm=true + 假写面 fail_on("phase-start")：run 照常显式收敛 failed 且原因经 Finished 流出（auto 只作用于 phase 间确认点——失败终态不停等也不跳过），更新流同样零 `confirmWait` 信封 | 新增 |
| walk_run auto_confirm=true 直通 | 边界 | auto_confirm=true + 决策分叉预录 ask 决议：RunUpdate::Ask 照常流出 + wait_answer 挂起停等（不自动选择），answer 回流后 Continue 决策会话收敛 completed；全程零 `confirmWait` 信封（auto 与 ask 互不干涉——AC-5 ask 半边，auto 模式的语义边界） | 新增 |
| walk_run auto_confirm=false 手动模式回归 | 正向 | 既有用例族「相位间停等确认 confirm 流出且 proceed 继续推进」构造点补 auto_confirm: false 后语义回归零变更：ConfirmWait 逐相位携相位载荷流出、确认后依序推进直至 completed（AC-2） | 新增 |
| walk_run auto_confirm=false 手动模式回归 | 异常 | 既有用例族「confirm 否决收敛 stopped」补 auto_confirm: false 后回归零变更：proceed=false → 受控 stopped、不再发起新相位 / 新会话、注册表除名（AC-2） | 新增 |
| walk_run auto_confirm=false 手动模式回归 | 边界 | 既有用例族「停止置位收敛 stopped」补 auto_confirm: false 后回归零变更：等待期取消置位（停止不必先应答）→ 受控 stopped（AC-2） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| WorkerAgentPort 假引擎（FakeWorker） | 被测 API 显式入参（port 注入依赖）：预录密封产出 + WorkerTurnRequest 全量捕获——auto 直通用例用缺省 PASS_JSON；ask 边界行 with_decision_reports 预录 ask + retry 决议 | 「walk_run auto_confirm=true 直通」全部行 |
| ToolStepPort 假写面（FakeTools） | 注入依赖：with_phase_next 预录多相位路由（route_outcome × N + done_outcome）；fail_on("phase-start") 注入 Err(String) 失败分支 | 「walk_run auto_confirm=true 直通」正向（多相位路由）与异常（失败注入）行 |
| DiffContextPort 假实现（FakeDiff） | 注入依赖：canned diff 队列（auto 用例不断言 diff 面，空队列即可） | 全部行 |
| WorkflowSnapshotPort（StubSnapshot / 真实 FsSnapshot） | 注入依赖：假双缝用例用 StubSnapshot 占位；ask 边界行用真实 FsSnapshot + TempRoot 真盘 DECISION_FIXTURE（决策会话 detail 读取真实组合，进程边界为 tempdir 文件系统） | 「walk_run auto_confirm=true 直通」ask 边界行 |
| ChangeFlowControl / RunGuard | 真实实现参与（tokio broadcast/oneshot 不 mock）：手动模式回归行接 spawn_confirmer 真实注册表 confirm 回路；auto 行不接 confirmer（直通结构性证明） | 「walk_run auto_confirm=false 手动模式回归」三行接 confirmer；auto 行不接 |

### packages/desktop/src-tauri/src/commands/change_flow/mod.rs -> packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs

#### 待测功能

- change_flow_start(app, on_event, root, change, auto_confirm): tauri command + specta；尾参透传测试缝
- change_flow_start_with(app, on_event, root, change, auto_confirm): 泛型测试缝（生产注入 Wry / 测试注入 MockRuntime），与生产入口同签名；组合根 `RunRequest` 构造点透传（D3）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| change_flow_start auto_confirm 透传受理 | 正向 | auto_confirm=true 合法 change 发起：Ok 返回 running 态摘要（run- 前缀 run_id）、订阅先行（run 登记即快照可见）、捕获 Channel 收集更新流至 Finished 全程零 `confirmWait` 信封（AC-3「auto 参数达 walker」受理断言）；PATH 隔离下后台合成收敛照常（终态 failed、注册表除名、phase-start 落盘证据在场）——auto 与手动的行为分叉语义直测归 walker_test，本用例证明参数面受理与透传布线不破命令面 | 新增 |
| change_flow_start auto_confirm 透传受理 | 边界 | auto_confirm=false 默认档受理面回归：同 fixture 发起 Ok、前置校验 / begin_run 登记 / 订阅先行 / spawn 时序零变更、更新流与终态面语义不变（尾参不改变既有受理契约） | 新增 |
| change_flow_start auto_confirm 透传受理 | 异常 | auto_confirm=true + 不存在 change 发起：Err 携 change 名且注册表零登记（auto 参数不绕过 blank 守卫与两道前置校验——新签名下异常输入面行为不变） | 新增 |
| change_flow_start 既有用例构造点补参 | 正向 | 既有用例族「提前 resolve 返回 running 摘要且 Channel 首事件到达」的 change_flow_start_with 调用点补 auto_confirm 实参后全绿：受理 / 订阅先行 / 后台驱动 / 终态除名链路语义零变更 | 新增 |
| change_flow_start 既有用例构造点补参 | 异常 | 既有用例族「前置校验三失败分支」「同 change 并行冲突」调用点补 auto_confirm 实参后全绿：三失败成因与并行冲突记因零漂移、失败分支零登记 | 新增 |
| change_flow_start 既有用例构造点补参 | 边界 | 既有用例族「blank root / blank change 守卫」调用点补 auto_confirm 实参后全绿：bool 入参无格式检查面（is_blank 口径不适用——blank 守卫模板沿用零新增分支，D3） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| tauri MockRuntime app | 运行环境边界：tauri::test::mock_app() manage 真实 WorkspaceStores / `Arc<ChangeFlowControl>` / `Arc<StopRegistry>` 后直调 `*_with` 泛型测试缝（既有装置沿用） | 全部用例 |
| WorkspaceStores 数据文件 | 进程边界：tempdir Env 双根（data_dir + ws_root）+ workflow.json fixture 真实读写（前置校验 / 组合根真盘组合） | 全部用例 |
| PATH 环境变量 | 进程边界（全局变量）：空 PATH 隔离窗口（commands 级 TEST_PATH_LOCK 串行化），executor CliMissing 合成收敛、不 spawn 真实 CLI；窗口覆盖至后台 turn 收敛后再恢复 | 「change_flow_start auto_confirm 透传受理」两用例（后台驱动收敛观测） |
| tauri ipc Channel | IPC 边界：capturing_channel 逐信封收下出线 JSON（零 `confirmWait` 断言 / Finished 观测）；discarding_channel 丢弃型 | 「透传受理」两用例用捕获型；守卫 / 失败分支既有用例用丢弃型 |

### packages/desktop/src/views/changes/hooks/use-change-flow-run.ts -> packages/desktop/src/views/changes/hooks/use-change-flow-run.test.ts

#### 待测功能

- start（UseChangeFlowRunResult.start）: (autoConfirm: boolean) => Promise<void>——显式必填（不设缺省默认，D4）；invoke changeFlowStart(channel, root, change, autoConfirm)

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| useChangeFlowRun.start autoConfirm 传参 | 正向 | start(true) → invoke change_flow_start 携 root / change / 新 Channel 实例外增 autoConfirm: true（经真实 bindings 生成物出线——commands 模块不 mock） | 新增 |
| useChangeFlowRun.start autoConfirm 传参 | 边界 | start(false) → autoConfirm: false 显式出线（默认档亦为显式实参——签名必填无缺省，缺省回退手动档在类型面不可见是 D4 否决形态） | 新增 |
| useChangeFlowRun.start autoConfirm 传参 | 边界 | root / change 为 null 时 start(true / false) 均 no-op 零 invoke（守卫先行于传参——既有 null 守卫语义不随签名扩展漂移） | 新增 |
| useChangeFlowRun.start autoConfirm 传参 | 异常 | 既有 start reject 面用例（error 呈现 → 重试成功后 error 复位、Channel 复用不叠加）调用点补 autoConfirm 实参后回归零变更 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| @tauri-apps/api/core（invoke + Channel） | 进程边界（IPC）：vi.mock 按命令名分发的 invokeMock（可切 resolve / reject 并记录入参）+ 可编程 ChannelMock class（捕获 onmessage，既有装置沿用） | 全部用例 |
| bindings commands 模块 | 真实组合（不 mock）：commands.changeFlowStart 真实生成物参与调用——autoConfirm 入参经真实 bindings 布线到达 invoke 载荷（生成物面隐式回归） | 「start autoConfirm 传参」正向 / 边界用例 |
| flow/run-state.ts 纯 reducer | 真实组合（不 mock，内部模块）：归并逻辑真实参与 | 既有信封归并回归用例 |

### packages/desktop/src/views/changes/flow/run-control-panel.tsx -> packages/desktop/src/views/changes/flow/run-control-panel.test.tsx

#### 待测功能

<!-- design.md `### 公共函数 / API` 表无 run-control-panel.tsx 行（RunActions 为面板模块内部组件非导出不列）；待测面取自变更清单 run-control-panel.tsx 行与 D5 定稿。 -->

- RunActions 自动确认开关: label + 原生 `<input type="checkbox">`（data-testid="run-auto-next-phase"），组件本地 useState(false) 默认关；disabled={active} 运行期禁用定格（非隐藏——终局恢复可编辑并保留上次取值）；开关下常驻一行 muted 说明文案（不随开关状态条件渲染）
- RunActions 发起调用: run.start(autoConfirm)——以点击时刻开关值为实参（发起时定格，运行期不可切）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| RunControlPanel 发起区自动确认开关 | 正向 | 默认关：渲染后 run-auto-next-phase 复选框未选中；未勾选直接点击发起 → start spy 携 false 调用恰一次（data-testid 查询，AC-4） | 新增 |
| RunControlPanel 发起区自动确认开关 | 正向 | 勾选开关（fireEvent.click）后点击发起 → start spy 携 true 调用恰一次（开启后 start 携 autoConfirm=true，AC-4） | 新增 |
| RunControlPanel 发起区自动确认开关 | 正向 | 常驻轻提示文案：默认态与开启态均渲染同一行 muted 说明文案（明示仅跳过相位间停等、ask 中断 / 失败 / 终态仍停、不自动归档），不随开关状态条件渲染（D5） | 新增 |
| RunControlPanel 发起区自动确认开关 | 边界 | 运行期三态（running / waitingConfirm / waitingAsk）开关 disabled 定格：click 切档无效、当前取值保持可见非隐藏（AC-4「运行期开关定格不可切」） | 新增 |
| RunControlPanel 发起区自动确认开关 | 边界 | 终局三态（completed / stopped / failed）恢复可编辑且保留上次取值：勾选过的开关终局后仍选中（UI 便利语义，不影响 run 级定格——每次发起以点击时刻值为准） | 新增 |
| RunControlPanel 发起区自动确认开关 | 异常 | 发起失败（error 非空、state 保持 null）时开关可编辑且取值保留（未进入运行态——可改档后重试发起） | 新增 |
| RunControlPanel 发起区自动确认开关 | 边界 | auto run 天然不出确认卡片：confirmPhase 恒 null（无 ConfirmWait 事件流入——零新事件词汇）→ 无确认卡片；waitingAsk 态 ask 卡片照常呈现；由既有停等 / ask 卡片用例族回归承载（runStub 的 start spy 断言补 autoConfirm 实参后全绿） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| UseChangeFlowRunResult 替身（runStub） | 入参例外（hook 结果为组件显式 prop 注入依赖）：state 六态 fixture 可编程 + start / stop / confirm / answer vi.fn spy；start spy 以 toHaveBeenLastCalledWith 断言 autoConfirm 实参（既有装置沿用，hook 契约在 use-change-flow-run.test.ts 锁定） | 全部用例 |
| @testing-library/react（render + fireEvent） | 真实 DOM 渲染与事件驱动：checkbox 切档用 fireEvent.click、按钮点击用 fireEvent.click（真实组件树组合，ui 层 Button / Badge 真实参与，不 mock 内部组件） | 全部用例 |

---

## 不可测试项

- AC-6 `packages/desktop/package.json` version 0.4.2 → 0.4.3 — **原因**: 归档轨道版本号变更（随归档提交执行，不在实现任务列表），非可执行代码单元（test_resolve_paths 判定 Not a testable source file）；由归档提交人工核对承载，无自动化断言面。
- AC-3 bindings 重导半边（`bindings.ts` 增 `autoConfirm`） — **原因**: specta 生成物重导非手写可测单元，不建 `bindings.test.ts`（防空套件红灯）；重导正确性（仅 changeFlowStart 单行 diff、重导幂等、其余条目零变化）属生成物守线面（design D6），前端 invoke 载荷携 autoConfirm 的布线已由 use-change-flow-run.test.ts 经真实 bindings 模块隐式覆盖。
- AC-7 全管线执行聚合面（自动化套件全绿 / bindings 重导无 diff / knip 无新增豁免） — **原因**: 执行聚合面——由本文件各章节用例全集在 Rust / 前端自动化套件运行时综合承载（test-execution 阶段承接），无独立被测单元与独立用例；bindings 一致性与死代码守线为仓库级静态检查面，非单元用例承载。
- AC-5 终态「不触发归档」半边 — **原因**: 归档为归档轨道人工动作（no-auto-archive 偏好），run 代码路径零归档触点（本变更零改动即结构保障）——无归档调用缝可供断言；自动化面仅承载 completed 收口停给用户（walker_test 直通用例终态断言）。

