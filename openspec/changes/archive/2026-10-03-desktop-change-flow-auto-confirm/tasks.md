# 任务: desktop-change-flow-auto-next-phase

> 测试文件的编写与执行由 test-design / test-gen / test-execution 阶段承接，本列表只覆盖 design.md 变更清单的实现文件。`walker_test.rs` 既有 `RunRequest` 构造点补 `auto_confirm: false`、`change_flow/mod_test.rs` 透传用例、`run-control-panel.test.tsx` 开关断言均归测试轨道。顺序按依赖排列：walker 分支（生效层）→ 命令面透传 + bindings 重导 → 前端 hook → 面板开关 → 守线。版本 bump 归档轨道（AC-6 / design D7），随归档提交执行，不在本列表。

## 阶段一：walker 停等节奏分支（生效层）

- [x] 修改 `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs`：`RunRequest` 增 `pub auto_confirm: bool` 字段，字段 doc 钉死双模语义（false 手动停等既有语义 / true 跳过 phase 间停等、不进 `WaitingConfirm`、零 `ConfirmWait` 事件、ask / 失败 / 终态不变，design D2）
- [x] 同文件 drive() 确认点：`if !request.auto_confirm { ... }` 整体包住 `guard.emit(RunUpdate::ConfirmWait { .. })` + `guard.wait_confirm().await` + 停等未获确认的 `Terminal::stopped` 收口——emit 与 wait 原子同支，分支内语句序列与既有代码零位移（design D1）；行内注释同步补 auto 分支说明（「拍板停等节奏：auto 模式跳过本停等直通下一相位」口径）；ask 路径（`decision_session`）、终态收口（`next.done` → completed）、static-check 反馈边、重试预算零改动
- [x] 同文件模块 doc「停等节奏」段改写（design D2）：由单一句式改为 run 级发起参数双模口径（手动模式 phase 内自动、phase 间停等确认；auto 模式跳过 phase 间停等直通终态、ask / 失败 / 终态仍停）；新注释措辞零路径引用，不含 layout 命名隔离扫描的双禁令字面量（磁盘域根目录名与配置文件名）

## 阶段二：命令面透传与 bindings 重导

- [x] 修改 `packages/desktop/src-tauri/src/commands/change_flow/mod.rs`：`change_flow_start`（tauri command + `#[specta::specta]`）与 `change_flow_start_with`（pub(crate) 泛型测试缝）同步加尾参 `auto_confirm: bool` 并透传组合根 `RunRequest` 构造点（design D3）；blank 守卫、两道前置校验、`begin_run`、组合根装配、订阅先行、spawn 时序零改动；bool 入参无格式检查面（`is_blank` 口径不适用）；`all_commands!` 登记零改动（命令名不变）
- [x] 执行 `pnpm -C packages/desktop run bindings:export` 重导 `packages/desktop/src/types/generated/bindings.ts`（design D6）：预期 diff 仅 `changeFlowStart` 一行——签名尾部增 `autoConfirm: boolean`、invoke 载荷增 `autoConfirm`；其余命令条目与 DTO 零变化

## 阶段三：前端 hook 与面板开关

- [x] 修改 `packages/desktop/src/views/changes/hooks/use-change-flow-run.ts`：`UseChangeFlowRunResult.start` 签名改 `start: (autoConfirm: boolean) => Promise<void>`（显式必填、不设缺省默认，design D4）；`start` 实现体改 `commands.changeFlowStart(channel, root, change, autoConfirm)`；`useCallback` 依赖数组不变（autoConfirm 为入参非闭包捕获）；`stop` / `confirm` / `answer` / 订阅生命周期 / 重挂恢复零改动
- [x] 修改 `packages/desktop/src/views/changes/flow/run-control-panel.tsx`：`RunActions` 增「自动确认」开关——label + 原生 `<input type="checkbox">`（`data-testid="run-auto-next-phase"`），置于发起 / 停止按钮左侧同行，组件本地 `useState(false)` 默认关，`disabled={active}` 运行期禁用定格（非隐藏，终局后恢复可编辑并保留上次取值，design D5）；发起调用改 `run.start(autoConfirm)`；开关下常驻一行 muted 轻提示文案：「自动确认：仅跳过相位间停等；ask 中断、失败与终态收口仍停下等待，不自动归档。」（不随开关状态条件渲染）；不引入 ui 新组件件（原生 input 沿 `AskCard` 先例）；`ConfirmCard` / `AskCard` / 终态记因 / 状态徽章零改动

## 阶段四：守线（静态，不含测试执行）

- [x] 静态检查全绿：`pnpm -C packages/desktop run server:check`（cargo fmt + clippy；cfg(test) 测试代码不在默认编译面，测试构造点适配由测试轨道承接）与 `pnpm -C packages/desktop run client:check`（vp check + knip，零新增豁免条目）；自动化套件的执行验证由 test-execution 阶段承接（AC-7 执行面）。server:check 与 client:check 全绿（守线拦截后既有用例 `use-change-flow-run.test.ts` 六处 `start()` 调用点按 test-design「既有用例构造点补参」机械补 `false` 实参——手动档语义回归零变更；新用例归 test-gen；knip 零输出零豁免）
- [x] bindings 重导幂等自查：再跑 `pnpm -C packages/desktop run bindings:export` 零新增 diff，`bindings.ts` 变化仅 `changeFlowStart` 单行（design D6）
- [x] 静态自查：禁改面零 diff——`crates/core/orchestration/src/control.rs`、`state.rs`、`crates/core/workflow/**`（含 golden 与 fixtures）、前端 `flow/run-state.ts`、`change_flow_answer` / `change_flow_confirm` / `change_flow_stop` / `change_flow_state` / `change_flow_watch` 五命令、`plugins/dev-team/**`；walker.rs 新注释零双禁令字面量命中
- [x] 变更清单核对：design.md 变更清单与实际触达文件双向一致（清单外零改动、清单内零遗漏）；`packages/desktop/package.json` 零触点（version 0.4.2 → 0.4.3 归档轨道执行，AC-6 / design D7）
