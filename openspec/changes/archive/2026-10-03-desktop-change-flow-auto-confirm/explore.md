# desktop-change-flow-auto-next-phase 探索笔记

desktop change flow run 增加「自动确认」开关：打开后 phase 间停等确认跳过，
run 自动推进下一相位，全程无需手动点「继续」。

## 现状链路（2026-10-03 摸底）

- 停等点唯一：`orchestration/src/walker.rs` drive() 主循环，⑥ phase-log 落账
  pass 后 `guard.emit(RunUpdate::ConfirmWait)` + `guard.wait_confirm().await`
  挂起，至 `change_flow_confirm` 命令应答（proceed bool oneshot，单次）。
- 控制面：`orchestration/src/control.rs` `ChangeFlowControl::confirm`；快照面
  `WaitingConfirm` 状态经 `ChangeRunSnapshot` 供前端重挂恢复。
- 前端：`run-control-panel.tsx` ConfirmCard（继续 / 终止运行）→
  `use-change-flow-run.ts` `confirm(bool)` → `commands.changeFlowConfirm`。
- 节奏定位：walker.rs 头注释「拍板停等节奏：phase 内自动、phase 间停等确认」
  ——停等节奏是 run 级属性；run 生命周期由后端注册表承载，不依赖前端在位。

## 已收敛的设计决策

1. **生效层 = walker 分支**（否决前端自动应答 / 后端 watcher 两条路）：
   - 前端自动应答致命伤：推进依赖前端挂载，切视图 / 窗口事件断流即卡死在
     waitingConfirm，违背 run 生命周期后端承载原则。
   - `RunRequest` 加 `auto_confirm: bool`；walker 确认点 `if !request.auto_confirm`
     才 emit + wait。auto 模式 run 根本不进 WaitingConfirm 态。
2. **开关粒度 = 发起参数，发起时定格**（否决全局持久化设置 / 运行中切换）：
   - `change_flow_start` 命令加 `auto_confirm` 参数（bindings 再生成）。
   - UI 为发起按钮旁的开关；运行中不可切——手动改自动可停止重发（续走自
     active_phase，成本低）；auto 模式下用户干预唯一合法动作就是停止。
3. **语义边界一：ask 中断不自动应答**。waitingAsk 来自决策 agent 重试预算
   耗尽后的真实提问，恰恰需要人的判断；auto 模式下 ask 到来 → 停等人是正确
   行为。自动确认只作用于 phase 间停等。
4. **语义边界二：终态不自动归档**。全相位 pass 收口 Completed（"Ready for
   archiving"），归档保持手动（与用户 no-auto-archive 偏好一致，现有代码即
   如此）。

## 改动面预估（最小）

- `RunRequest`（walker.rs）+ auto_confirm 字段；drive() 确认点分支。
- `change_flow_start` / `change_flow_start_with`（src/commands/change_flow/mod.rs）
  + 参数透传 RunRequest。
- 前端 `RunControlPanel` 发起区开关 + `useChangeFlowRun.start(autoConfirm)`
  传参；`run-state.ts` 零改动（无新事件——phase 边界推进感由既有
  PhaseLog passed → 下一 PhaseStart running 步流表达）。

## 已验证不受影响的面

- golden wire 契约在 workflow.json 解析层（core/workflow/tests/golden），
  不涉 change_flow 命令签名。
- `ChangeRunSnapshot` / `RunUpdate` DTO 零改动：auto 模式不产生新事件、
  不进 WaitingConfirm，重挂恢复路径天然兼容。
- 终态收口（Completed）前本就无停等，auto 语义自洽。

## 遗留开放问题

- auto 模式 UI 是否需要轻提示（无人监督连续烧 token 跑完 9 相位）——倾向
  开关旁一行说明文案即可，不做阻断式确认。
