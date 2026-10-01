# 测试设计: desktop-change-flow

> **日期**: 2026-10-01（回溯重收敛后重写：写通道由「CLI 子进程」翻转为「core/workflow Rust 写面」，本版逐条核对现行 design.md 重推；旧版与被回退架构绑定的断言以 `废弃` 行显式退役）

---

## 验收范围

<!-- 每行 `被测文件或模块` 为该 AC 用例的主要承载测试文件（单值），半边路由如下：
     AC-1 的 workflow.json 演进对照半部落写面各写操作测试与 walker_test「真实写面组合演进对照」边界用例。
     AC-2 解析 / 预校验半边落 decision_test.rs；写面二次校验兜底半边落 backtrack_test.rs。
     AC-3 git diff spawn 半边落 git_diff_test.rs；每轮组装前取新 diff 半边落 walker_test.rs；file_log 零触碰断言落写面各测试。
     AC-4 spawn 实现半边落 static_check_test.rs；步委托缝落 steps_test.rs。
     AC-5 归并半边落 run-state.test.ts；联动半边落 use-session-transcript.test.ts / session-transcript-panel.test.tsx / detail-drawer.test.tsx；控制入口落 run-control-panel.test.tsx。
     AC-6 动态半边（进程内直调证据）落 steps_test.rs；静态半边（零 CLI 写通道、devteam 不存在、依赖方向、spawn 落 infra）见不可测试项 2。
     AC-7 装配半边落 worker_test.rs；续走半边（重入自 active_phase 不重跑已 pass 相位）落 walker_test.rs 边界用例；锚点标定半边落 phase_next_test.rs。
     AC-10 可回归半边（回退后注册面断言集）落 cli.test.ts；文件删除 / 版本 / dist 去痕见不可测试项 3。 -->

| AC ID | 验收条件 | 被测文件或模块 |
|--------|---------|----------|
| AC-1 | 以假引擎 + 假写面（fake port）驱动一次 requirement 工作流 run：walker 依写面 phase-next 返回走完 ①→⑦ 全循环（executor → static-check → evaluator → verdict → phase-log → phase-next），pass 相位自动推进、fail 重试 ≤5（与插件 `MAX_RETRY_TIMES` 一致），workflow.json 的 active_phase / eval 演进与插件直跑形态对照一致 | packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs |
| AC-2 | 重试上限时决策 agent 被唤起且输入有界（fail checklist + 白名单 + 候选 eval report）；白名单内 backtrack 被执行、越权 backtrack 被写面二次校验拒绝；`ask` 动作令运行中断并在 UI 呈现问题与选项；retry 预算内不唤决策 agent（walker 自走） | packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs |
| AC-3 | desktop run 全程不产生 file_log 条目（workflow.json `file_log` 零新增）；executor / evaluator prompt 组装含 git diff 变更文件上下文；既有 skill 路径 file_log 数据的流程图挂载与图外面板显示不动 | packages/desktop/src-tauri/crates/core/orchestration/src/prompt_test.rs |
| AC-4 | implement / test-gen 相位 executor 收口后 static-check spawn 步必经执行；失败诊断注入同一 executor 会话修复（≤5 次），超限升格相位 fail（代写 fail checklist 落账后进重试 / 决策分叉）；static-check 节点状态上图可观测 | packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs |
| AC-5 | 运行中流程图节点实时呈现运行态（WorkerAgent / ToolStep / Gate 各自可辨），点击 WorkerAgent 节点打开对应会话转录（运行中实时流、结束后重放一致）；运行结束后图与既有派生规则回归一致（desktop-change-flow-view 既有场景全绿） | packages/desktop/src/views/changes/flow/graph.test.ts |
| AC-6 | walker 全程零 CLI 子进程写 workflow.json 的代码路径，所有状态变更经 core/workflow 写面进程内直调；`infra/devteam` crate 不存在；orchestration → workflow 依赖方向成立；static-check spawn 为唯一进程 spawn 且落 infra 层（spawn 不进 core） | packages/desktop/src-tauri/crates/core/orchestration/src/steps_test.rs |
| AC-7 | executor / evaluator / decision 会话 provenance 为 `source="change"`、`source_ref=<change>/<phase>/<role>/<attempt>`；按 provenance 反查会话集可派生节点状态；停止 / 桌面重启后重新发起 run 自 active_phase 续走，不重头执行已 pass 相位 | packages/desktop/src-tauri/crates/infra/agent/src/worker_test.rs |
| AC-8 | `pnpm -C packages/desktop run client:check`、`pnpm -C packages/desktop run test`、rust 套件全绿；knip 无新增豁免条目；desktop 版本 0.4.0（`src-tauri/Cargo.toml` 不随动）；dev-team 插件零 bump（保持 2.10.44） | —（见不可测试项 1） |
| AC-9 | core/workflow 写面以对照功能验收：`phase_next` 路由 / 重试上限 / 白名单下发、`phase_start` attempt 计时、`phase_log` 落账、`backtrack` stale 标记 + 传播，逐项对照插件直跑行为语义验收（不建差分 oracle）；backtrack 回跳后同相位重评条目在 `phase_log` 全部落账（插件已知丢条目缺陷在写面不复发）；serde 写出的 workflow.json 可被插件 zod schema 读取 | packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next_test.rs |
| AC-10 | `walker-*` 七文件、`cli.ts` walker 注册、`package.json` 2.10.45 bump 全部回退（插件源码与已发布 2.10.44 形态一致）；三类 dist 交付产物（`claude-plugins/` / `cursor-plugins/` / `cursor-home-image/`）回退后 rebuild 刷新，产物中无 walker-* 痕迹 | plugins/dev-team/bin/src/cli.test.ts |

---

## 单元测试

<!-- Rust 侧套件为 src-tauri 工作区根注册的 cargo 测试（`*_test.rs` 共置模块，#[test] / #[tokio::test]）；前端为 packages/desktop 与 plugins/dev-team 的 vite-plus 测试运行器（`*.test.ts(x)` 共置）。
     主装置：walker 双缝（WorkerAgentPort 假引擎 + ToolStepPort 假写面，AC-1 口径）+ 真实 LocalToolSteps / 真实 workflow::write 组合（进程内缝真实组合，仅 fs 进程边界以 tempdir 真盘 fixture 承接）+ 写面各写操作 tempdir 真盘 fixture（插件直跑形态 fixture 为对照基准）。
     跨模块组合用例挂靠链路入口模块：`ToolCommand → workflow::write → workflow.json 持久化` 链路挂 steps_test、相位循环链路挂 walker_test、命令链路挂 change_flow/mod_test、前端实时链路挂 run-state.test / use-change-flow-run.test；describe 标题写链路方向；不设独立集成测试章节、不设 `__tests__/` 组合测试区。
     最小 mock 原则：仅进程边界（fs / 子进程 / IPC / PATH 等运行环境）与作为注入依赖显式传入的 port / hook 允许 mock；内部模块（写面、verdict / decision / prompt、run-state 纯函数、agent-adapter、AgentTimeline、useChangeDetail 等协作 hook）一律真实组合。
     随回退删除的测试文件（infra/devteam discover / runner 测试、views_test、walker-* 七测试、extract_write_paths 测试半边、CLI 假件缝）不再设章节；其被推翻的旧断言在存留章节以 `废弃` 行显式退役（walker_test / port_test / state_test / transcript_test / prompt_test / mod_test / cli.test）。 -->

### packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table_test.rs

#### 待测功能

- PhaseDefinition(): 单相位定义（id / description / executor / evaluator 两角色 `PhaseAgentSpec`，requirement 表静态项）
- PhaseAgentSpec(): 相位角色的 agent 引用与 prompt 模板（`__CALL_AGENT:<role>__` 约定原样保留）
- MAX_RETRY_TIMES(): 重试上限常量（与插件同名常量一致）
- phase_table(): 按工作流类型取相位表；V1 仅 `requirement` 返回 Some，其余 None（W8 前置校验依据）
- interpolate(): `<change>` / `<phase>` 占位符插值（与插件 `interpolatePrompt` 语义一致）
- allowed_backtrack_phases(): 当前相位在表中的全部前置相位（白名单）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| phase_table requirement 返回全表 | 正向 | phase_table("requirement") → Some：相位序与插件相位表逐项对照一致（移植面——AC-9 对照口径）、各相位 executor/evaluator 两角色 PhaseAgentSpec 齐备 | 新增 |
| phase_table 非 requirement 拒绝 | 异常 | "bug-fix" / "test-only" / "" / 任意未注册类型 → None（W8 写面侧显式拒绝依据，V1 只 requirement） | 新增 |
| MAX_RETRY_TIMES 锚定 | 边界 | == 5（与插件 `MAX_RETRY_TIMES` 一致——AC-1 重试上限语义的常量锚） | 新增 |
| interpolate 双占位符替换 | 正向 | 模板含 `<change>` 与 `<phase>`：两占位符各自替换、多次出现全替换、模板其余字节（换行 / 中文 / markdown 标记）保真 | 新增 |
| interpolate phase 缺席形态 | 边界 | phase=None：占位符按 None 语义处理不崩、`<change>` 照常替换（与插件 `interpolatePrompt` None 语义一致） | 新增 |
| interpolate 无占位符与未知占位符 | 边界 | 不含占位符模板原样返回；未知占位符（如 `<other>`）不误替换 | 新增 |
| 白名单首相位为空 | 边界 | 表首相位 → 空白名单（无前置可回溯——AC-2 空白名单出口的相位表面） | 新增 |
| 白名单表序前置 | 正向 | 表中段相位 → 全部前置相位按表序返回（AC-2 白名单下发的内容面） | 新增 |
| 白名单未知相位 | 异常 | current_phase 不在表中 → 空 Vec（不崩、不臆测） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（无进程边界依赖） | 相位表为内置静态数据、interpolate / 白名单为纯函数：纯字符串与静态表入参内存构造 | 全部 describe |

### packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next_test.rs

#### 待测功能

- SessionAnchors.new(): 进程内会话锚点（内部 `Mutex<HashMap<(change, run_id), usize>>`，每 run 一个实例——W7）
- PhaseNextOutcome(): 路由产出（done / next_phase / round / executor / evaluator / allowed_backtrack_phases / last_result / error）
- PhaseNextError(): `MaxRetriesExceeded { phase, round }`（决策 agent 唤起触发点）
- LastResult(): 最近一次 eval 条目快照（决策输入面）
- phase_next(): 只读路由状态机：初始 / 推进 / fail 重试 / 重试上限 / mid-phase interruption（锚点比对）；不改 eval store

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| phase_next 初始路由 | 正向 | 初始 fixture（无 active_phase、eval 空）：next_phase=首相位、done=false、executor/evaluator prompt 已插值 `<change>`、白名单为空 | 新增 |
| phase_next pass 推进 | 正向 | 当前相位最新条目 pass：next_phase=下一相位、round 归位、白名单=新相位的表序前置集（AC-1 推进面） | 新增 |
| phase_next fail 预算内重试 | 正向 | 最新条目 fail 且 round < MAX_RETRY_TIMES：next_phase=同相位、round 递增、executor/evaluator prompt 同相位插值（AC-1 重试面） | 新增 |
| phase_next 重试上限恰达 | 边界 | round == MAX_RETRY_TIMES 的 fail 相位：error=MaxRetriesExceeded{phase, round}（决策分叉触发——AC-2 唤起触发面） | 新增 |
| phase_next 上限前一手 | 边界 | round == MAX_RETRY_TIMES - 1：仍同相位重试、error=None（上限边界不提前触发） | 新增 |
| phase_next 全部 pass 终态 | 正向 | 末相位 pass：done=true、next_phase=None（walker 收敛 completed 的判定输入） | 新增 |
| phase_next 白名单下发 | 正向 | 中段相位：outcome.allowed_backtrack_phases 与 allowed_backtrack_phases(表) 计算一致（表序前置） | 新增 |
| phase_next last_result 快照 | 正向 | 有 eval 史：last_result 携最近条目 phase/verdict/report/timestamp（决策输入面）；无 eval 史 → None | 新增 |
| 锚点首见记基线 | 边界 | 新 (change, run_id) 首次 phase_next：记录当时 eval 条目数基线（W7 锚点语义） | 新增 |
| 锚点 run_id 隔离 | 边界 | 同 change 不同 run_id：基线互不影响（per-run 实例、跨 run 不共享——W7）；同 (change, run_id) 复用同一基线 | 新增 |
| mid-phase interruption 承接 | 边界 | active_phase 残留 + 该相位已有 eval 条目（中断后重新发起）：phase_next 解析到 active_phase 相位承接（next_phase=残留相位、last_result 携既有条目），不跳相位、不重置已落账条目（差异表 #9 进程内复活——AC-7 续走伴生面） | 新增 |
| phase_next 只读性 | 边界 | 调用前后 workflow.json 字节不变（只读路由不改 eval store——写面红线锚） | 新增 |
| phase_next change 不存在 | 异常 | Err 显式（不静默空产出） | 新增 |
| phase_next workflow_type 非 requirement | 异常 | 相位表 None → Err（W8 写面侧出口，与命令层前置校验分层） | 新增 |
| phase_next workflow.json 不可解析 | 异常 | 损坏 fixture → Err 显式（不臆测路由） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无进程边界 mock（fs 真实组合） | tempdir 真实 change fixture（workflow.json 以插件直跑形态 fixture 真盘，沿 core/workflow detail_test 装置先例）；SessionAnchors 以真实实例参与（进程内值对象不 mock） | 全部 describe |

### packages/desktop/src-tauri/crates/core/workflow/src/write/phase_start.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/phase_start_test.rs

#### 待测功能

- PhaseStartOutcome(): 开相位产出（phase / attempt / start_at）
- phase_start(): `active_phase` 定点写入（attempt 自既有值递增，重入即新一轮计时）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| phase_start 开相位落盘 | 正向 | fixture 开相位：workflow.json active_phase 定点写入 {phase, attempt, start_at}，outcome 三字段与落盘一致（AC-9 attempt 计时对照面） | 新增 |
| attempt 自既有值递增 | 正向 | 既有 active_phase.attempt=2 → start 后 attempt=3（对照插件 `phase-start` 语义） | 新增 |
| phase_start 重入新一轮计时 | 边界 | 同相位重复 start：attempt 再递增、start_at 刷新（重入即新一轮计时） | 新增 |
| phase_start 非法相位拒绝 | 异常 | 相位不在 requirement 表 → Err 且文件零变更 | 新增 |
| phase_start workflow_type 非 requirement | 异常 | 非 requirement fixture → Err（W8 分层出口） | 新增 |
| 未知字段保形 | 边界 | 预置未知 / legacy 字段的 fixture：写后原样保留（W2 raw Value 定点改写——serde↔zod 兼容最强形态，AC-9） | 新增 |
| file_log 零触碰 | 边界 | 既有 file_log 条目写前后逐字节不变（AC-3 写面半边——desktop run 零 file_log 新增） | 新增 |
| pretty 写回可再读 | 边界 | 写回文件 serde 再解析成功、键集与字段形状不变（W3 zod 兼容 fixture 对照口径：插件直跑形态 fixture 为基准样本） | 新增 |
| phase_start change 不存在 | 异常 | Err 显式 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无进程边界 mock（fs 真实组合） | tempdir 真实 change fixture 真盘（合法 / 带 legacy 字段 / 损坏三族）；「零写入」以调用前后字节比对断言 | 全部 describe |

### packages/desktop/src-tauri/crates/core/workflow/src/write/phase_log.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/phase_log_test.rs

#### 待测功能

- PhaseLogInput(): 落账输入（phase / report / checklist / skipped；checklist 用 `workflow::model::ChecklistItem`）
- PhaseLogOutcome(): 落账产出（phase / attempt）
- phase_log(): verdict 推导（checklist 全 pass；skipped 约束）+ report ≤500 校验 + **纯追加落账（W9 缺陷修复）** + 落账后清 active_phase

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| phase_log pass 落账 | 正向 | checklist 全 pass：eval 追加条目 verdict=pass、phase/attempt/checklist/skipped/start_at 齐全（start_at 自 active_phase 继承）、落账后 active_phase 清除 | 新增 |
| phase_log fail 落账 | 正向 | checklist 含 fail 项 → verdict=fail 条目追加（进重试 / 决策分叉的输入面——AC-4 升格代写形态同通道） | 新增 |
| 纯追加缺陷回归 | 正向 | backtrack 回跳后同相位重评：既有 pass 条目在场仍追加新条目、attempt=该相位既有条目数+1，不因「相位已存在 pass 条目」短路跳过（W9——AC-9 缺陷不复发的主回归行） | 新增 |
| attempt 推导含历史条目 | 边界 | 同相位既有条目含 stale / pass / fail 混合历史：attempt 计数含全部历史条目 +1（不挑食） | 新增 |
| report 恰 500 字符 | 边界 | report=500 字符 → 落账成功（≤500 边界含端点） | 新增 |
| report 超长拒绝 | 异常 | report=501 字符 → Err 且 eval 零新增 | 新增 |
| skipped 形态 | 边界 | skipped=true（static-check 超限桌面代写升格 fail 场景）落账形态、约束校验不误拒 | 新增 |
| phase_log 无 active_phase 拒绝 | 异常 | 未开相位直接 phase_log → Err（start_at 无继承源） | 新增 |
| phase_log 非法相位拒绝 | 异常 | 相位不在表 → Err 且零写入 | 新增 |
| phase_log 保形与 file_log 零触碰 | 边界 | 未知字段保形 + file_log 逐字节不变 + pretty 写回可再读（W2/W3/AC-3 断言在三写操作各自承载） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无进程边界 mock（fs 真实组合） | tempdir 真实 change fixture 真盘（含 backtrack 后形态 fixture 供缺陷回归行驱动）；「零写入」以调用前后字节比对断言 | 全部 describe |

### packages/desktop/src-tauri/crates/core/workflow/src/write/backtrack.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/backtrack_test.rs

#### 待测功能

- BacktrackInput(): 回溯输入（phase / to / reason / allowed——allowed 随行走带，写面二次校验）
- BacktrackOutcome(): 回溯产出（phase / target）
- backtrack(): 白名单二次校验（越权 Err 不写）+ reason ≤500 + 最新条目标记 backtrack_to/reason + stale 标记与相位表依赖向后传播

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| backtrack 白名单内落盘 | 正向 | to ∈ allowed：目标相位回跳、最新条目标记 backtrack_to/reason、outcome{phase, target}（AC-2 写面执行半边） | 新增 |
| backtrack 越权拒绝零写入 | 异常 | to ∉ allowed：Err 且 workflow.json 字节零变更（写面二次校验兜底——坏决议损坏不了状态，AC-2/AC-9） | 新增 |
| backtrack 空白名单拒绝 | 边界 | allowed 空数组 + 任何 backtrack → Err（无跳转出口） | 新增 |
| backtrack reason 恰 500 | 边界 | reason=500 字符 → 成功（≤500 边界含端点） | 新增 |
| backtrack reason 超长拒绝 | 异常 | reason=501 字符 → Err 且零写入（≤500 约定的写面承载——decision_test 分层锚的对端） | 新增 |
| stale 按表序传播 | 正向 | 目标相位及表序其后相位的既有 eval 条目标 stale、更早相位条目不动（AC-9 传播面；白名单=表序前置与传播方向自洽） | 新增 |
| backtrack 目标为表首 | 边界 | 回溯到首相位：全部既有条目按表序标 stale、首相位自身承接重跑 | 新增 |
| backtrack 非法目标相位 | 异常 | to 不在表 → Err 零写入 | 新增 |
| backtrack 无 eval 条目拒绝 | 异常 | 全新 change 直接 backtrack → Err（无可标记条目） | 新增 |
| backtrack 保形与 file_log 零触碰 | 边界 | 未知字段保形 + file_log 逐字节不变 + pretty 写回可再读（W2/W3/AC-3 口径） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无进程边界 mock（fs 真实组合） | tempdir 真实 change fixture 真盘；「越权 / 超长零写入」以调用前后字节比对断言（沿原工具侧字节比对装置口径迁入写面） | 全部 describe |

### packages/desktop/src-tauri/crates/core/workflow/src/write/persist.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/persist_test.rs

#### 待测功能

<!-- design.md 公共函数/API 表未声明该文件的条目（crate 内私有持久层：「crate 内私有，无 crate 外导出」）。不建 persist_test.rs（防空套件红灯）：W2 raw Value 保形定点改写、pretty 写回与 W3 zod 兼容 fixture 对照断言由同 crate 四写操作测试承载（phase_start / phase_log / backtrack 各自的「未知字段保形 / file_log 零触碰 / pretty 写回可再读」行）。 -->

（design.md 未声明该文件的公共 API 变更——持久化行为经写操作测试承载，无独立测试文件。）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| （无独立条目——见上注） | — | raw Value 保形 / pretty 写回 / zod 兼容 fixture 断言由 phase_start_test / phase_log_test / backtrack_test 的对应行承载 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 不适用（不建测试文件） | — |

### packages/desktop/src-tauri/crates/core/workflow/src/write/mod.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/mod_test.rs

#### 待测功能

<!-- design.md 公共函数/API 表未声明该文件的条目（写面子模块声明与公共再导出，「无独立函数」）。不建 mod_test.rs（防空套件红灯），见不可测试项 4。 -->

（design.md 未声明该文件的公共 API 变更——不建测试文件，可达性经写面各 `*_test` 以 `write::` 路径导入编译期承载。）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| （无——见不可测试项 4） | — | 导出面无独立行为断言：phase_table_test 等以 `write::` 路径导入任一破坏即编译失败 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 不适用（不建测试文件） | — |

### packages/desktop/src-tauri/crates/core/workflow/src/lib.rs -> packages/desktop/src-tauri/crates/core/workflow/src/lib_test.rs

#### 待测功能

<!-- design.md 公共函数/API 表未声明该文件的条目（修改行：增 `pub mod write;` 与写面公共类型再导出，crate 根注释措辞同步修订）。不建 lib_test.rs（防空套件红灯），见不可测试项 4。 -->

（design.md 未声明该文件的公共 API 变更——不建测试文件，可达性经写面各 `*_test` 的 crate 根 use 导入编译期承载。）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| （无——见不可测试项 4） | — | crate 根导出面无独立行为断言：写面各 `*_test` 以 crate 根 use 导入任一破坏即编译失败 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 不适用（不建测试文件） | — |

### packages/desktop/src-tauri/crates/core/orchestration/src/port.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/port_test.rs

#### 待测功能

- ToolCommand(): 封闭集五变体：PhaseNext{change, run_id} / PhaseStart{change, phase} / PhaseLog{change, phase, input} / Backtrack{change, phase, input} / StaticCheck（载荷直载写面输入类型；ChangeFiles 变体出局）
- ToolStepOutput(): 五变体：PhaseNext(Box<PhaseNextOutcome>) / PhaseStart / PhaseLog / Backtrack / StaticCheck(StaticCheckOutcome)（载荷换 `workflow::write` 原生类型）
- StaticCheckOutcome(): 门禁产出（passed / diagnostics）
- StaticCheckRunner(): static-check spawn 缝 trait（`run(root) -> BoxToolFuture`——W4）
- DiffContextPort(): git diff 上下文缝 trait（`diff_context(root) -> BoxDiffFuture`——W5）
- BoxDiffFuture(): std-only Future 别名（`Pin<Box<dyn Future<Output = Result<String, String>> + Send>>`，不引 futures 依赖）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| ToolCommand 五变体封闭集 | 正向 | 五变体构造 + match 穷尽分发编译期锚定；载荷直载写面输入类型（PhaseLogInput / BacktrackInput 原样承接——AC-6 进程内缝命令面） | 新增 |
| ToolStepOutput 载荷换血 | 正向 | 各变体载荷（写面原生 PhaseNextOutcome / PhaseStartOutcome / PhaseLogOutcome / BacktrackOutcome / StaticCheckOutcome）构造与提取逐字段相等；PhaseNext 大变体 Box 收敛尺寸差 | 新增 |
| StaticCheckOutcome 字段面 | 边界 | passed true/false × diagnostics 空串 / 多行诊断：构造与读取（passed=false 走定向反馈边的载荷面——AC-4） | 新增 |
| StaticCheckRunner trait 面 | 正向 | 假实现经 Arc<dyn …> 注入、BoxToolFuture 返回可用、root 参数透传（object safety + Send+Sync 编译锚——W4 缝成立前提） | 新增 |
| DiffContextPort trait 面 | 正向 | 假实现经 Arc<dyn …> 注入、diff_context(root) 经 BoxDiffFuture resolve Result<String, String>（W5 缝成立前提——AC-3 组装链的注入面） | 新增 |
| BoxDiffFuture 别名 | 边界 | Pin<Box<dyn Future + Send>> 别名跨 await 持有可用、不引 futures 依赖（与 BoxToolFuture 同款手法） | 新增 |
| 既有三契约保持 | 边界 | WorkerAgentPort / WorkflowSnapshotPort / RunEventSink 与 WorkerTurnRequest / WorkerTurnOutcome 中性类型回归（编译期锚定不回退——AC-1 双缝装置前提） | 新增 |
| ToolCommand 含 ChangeFiles 变体 | 废弃 | 旧断言「六变体含 ChangeFiles / ToolStepOutput 六变体」随补录通道出局废弃（差异表 #3；封闭集收缩为五变体） | 废弃 |
| ToolStepError 三态映射 | 废弃 | 旧断言「Spawn / Exit / Drift 三变体互斥可辨」随 ToolStepError 类型出局废弃（统一 `Result<_, String>`——port 修改行）；步失败显式失败改由 walker_test「工具步失败显式失败」行以 Err(String) 承接 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 五 port 内存假实现（注入依赖，入参例外） | Vec 记录调用序与载荷、可编程产出 / Err(String)；trait 本身即注入面，真实实现分别在 steps_test / worker_test / snapshot_test / static_check_test / git_diff_test 组合 | 全部 describe |

### packages/desktop/src-tauri/crates/core/orchestration/src/steps.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/steps_test.rs

#### 待测功能

- LocalToolSteps.new(): 进程内 `ToolStepPort` 实现（`new(anchors, static_check)`）：相位机四步（phase-next / phase-start / phase-log / backtrack）直调 `workflow::write::*`（`foundation::layout::resolve(root)` 解析路径），StaticCheck 步委托注入的 `StaticCheckRunner`

<!-- 链路入口：`ToolCommand → workflow::write → workflow.json 持久化` 组合用例挂靠本节（链路发起方 = 步命令消费口），describe 标题写链路方向；不设独立集成章节。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `ToolCommand.PhaseNext → workflow::write → workflow.json` | 正向 | 真实 LocalToolSteps + tempdir 真盘 fixture：PhaseNext 命令 → ToolStepOutput::PhaseNext(Box<PhaseNextOutcome>) 载荷逐字段、路由产出与直调 phase_next 一致（AC-6「进程内直调」的动态证据） | 新增 |
| `PhaseStart → active_phase 落盘` | 正向 | PhaseStart 命令 → tempdir workflow.json active_phase 定点写入、ToolStepOutput::PhaseStart 承接（AC-1 ②步链路同源） | 新增 |
| `PhaseLog → eval 追加落盘` | 正向 | PhaseLog 命令携 PhaseLogInput → eval 条目落盘、ToolStepOutput::PhaseLog 承接（W9 纯追加语义经缝透传不变形） | 新增 |
| `Backtrack → stale 标记落盘` | 正向 | Backtrack 命令携 BacktrackInput（allowed 随行）→ 落盘标记、ToolStepOutput::Backtrack 承接 | 新增 |
| StaticCheck 委托注入 runner | 正向 | StaticCheck 命令 → 注入假 runner 记录 root 透传、StaticCheckOutcome 原样包装（AC-4 步词汇不变的实现端换血——W4） | 新增 |
| steps 写面 Err 统一上抛 | 异常 | change 不存在 / 相位非法：步返回 Err(String)（无 ToolStepError 包装——port 换血对端） | 新增 |
| 锚点实例复用语义 | 边界 | 同一 LocalToolSteps 连续 PhaseNext（同 change, run_id）：SessionAnchors 基线共享；换 run_id 实例基线独立（W7 per-run 装配锚） | 新增 |
| runner Err 透传 | 异常 | 注入假 runner 返回 Err：StaticCheck 步 Err(String) 原样（反馈边升格的记因面） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| StaticCheckRunner 假实现（注入依赖，入参例外） | 记录 root 调用、可编程 StaticCheckOutcome / Err(String) | StaticCheck describe |
| workflow::write 与 SessionAnchors 真实组合（不 mock） | tempdir 真盘 fixture（fs 进程边界真实组合）；进程内缝不再属进程边界，按最小 mock 原则真实驱动 | 相位机四步 describe |

### packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs

#### 待测功能

- walk_run(): 相位循环主入口 ①→⑦（`walk_run(worker, tools, diff, snapshot, control, guard, request)`——增 diff 参数；三类节点调度、feedback 边计数、停等点、fail 重试与决策分叉、停止收敛、run_id 铸造）
- STATIC_CHECK_FEEDBACK_LIMIT(): 定向反馈边独立上限（=5，沿 hook `loop_limit` 语义）
- STATIC_CHECK_PHASES(): 步门控布局常量（implement / test-gen，非路由权威）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| walk_run 全循环（假引擎+假写面）①→⑦ | 正向 | 假 ToolStepPort 预录 phase-next 序列、假 WorkerAgentPort 预录 executor/evaluator 转录：一次 run 的工具调用序恰为 phase-next → phase-start → executor → static-check（implement 站）→ evaluator → phase-log → phase-next（AC-1 全循环；补录步出局后由 ①→⑧ 收敛 ①→⑦——差异表 #3） | 新增 |
| walk_run ①→⑧ 全循环含 change-files 补录步 | 废弃 | 旧断言「change-files 步紧跟 executor、evaluator 前」随 file_log 出局废弃（差异表 #3/#4；extract_write_paths 半边同步删除） | 废弃 |
| walk_run pass 自动推进 | 正向 | 每相位 verdict=pass → 依 phase-next 推进直至 done=true → 收敛 completed；ToolCommand::PhaseStart/PhaseLog 的 change/phase/checklist 载荷与假写面捕获序列逐条对齐（AC-1） | 新增 |
| walk_run 真实写面组合演进对照 | 边界 | walker + 假 WorkerAgentPort + 真实 LocalToolSteps + tempdir 真盘 fixture（进程内缝不属进程边界，最小 mock 组合锚）：全程推进后 workflow.json 的 active_phase / eval 演进与插件直跑形态 fixture 对照一致（AC-1 尾句 + AC-9 对照口径的循环级承载） | 新增 |
| walk_run 重入自 active_phase 续走 | 边界 | 预置中段 fixture（前序相位已落 pass eval 记录、active_phase 指向中段相位；假写面首个 phase-next 响应承接该态、last_result 携既有 eval 记录）：发起 run 首次 phase-next 即解析到 active_phase、已 pass 相位零 phase-start/executor 调用（捕获断言、不重头执行）；中断相位承接重试而非新开回合（AC-7 续走半边——重入不重跑已 pass 相位） | 新增 |
| walk_run fail 预算内重试 | 正向 | verdict=fail 且 phase-next 返回同相位重试：attempt 递增重跑 executor/evaluator；重试上限判定不自建（写面 phase-next 权威返回，`MAX_RETRY_TIMES` 语义不复制——AC-1） | 新增 |
| walk_run 决策分叉唤起有界输入 | 正向 | phase-next 返回 MaxRetriesExceeded：决策 agent 恰被唤起一次，WorkerTurnRequest 捕获断言输入有界（fail checklist + 写面下发白名单 + 候选相位 eval report 取自 snapshot detail——AC-2 唤起半边） | 新增 |
| walk_run backtrack 决议执行 | 正向 | 决议 backtrack（to ∈ 白名单）→ ToolCommand::Backtrack 携 allowed 白名单发起 → 目标相位自该处重跑（AC-2 执行半边） | 新增 |
| walk_run retry 预算内自走 | 边界 | 预算内 fail 重试全程：决策 agent 零调用（phase-next 恒同相位重试，walker 自走——AC-2 尾句） | 新增 |
| walk_run 越权 backtrack 预校验拒绝 | 异常 | 决议 backtrack 越白名单：ensure_backtrack_allowed 预校验 Err → Backtrack 步零发起、写通道零调用（写面二次校验兜底见 backtrack_test——AC-2） | 新增 |
| walk_run ask 中断与应答回流 | 正向 | 决议 ask：RunUpdate::Ask 流出（question/options）+ wait_answer 挂起；answer 回流后以应答文本 Continue 决策会话重出封闭集（AC-2 ask 半边） | 新增 |
| walk_run phase 间停等确认 | 正向 | 相位推进间 RunUpdate::ConfirmWait 流出 + wait_confirm 挂起：proceed=true 继续、false → 受控终态 stopped（phase 内自动、phase 间停等节奏——定稿延续） | 新增 |
| walk_run 停止收敛 | 异常 | request_stop 置位：当前会话收口后终态 stopped、不再发起新相位/新会话（停止寻址键 = change 名，经 RunGuard.cancelled 观测——AC-7 停止半边） | 新增 |
| walk_run static-check 步门控 | 正向 | implement/test-gen 站（STATIC_CHECK_PHASES 命中）executor 收口后 static-check ToolStep 必经；非门控相位零 static-check 调用（AC-4 必经半边） | 新增 |
| walk_run 反馈边同会话注入 | 正向 | static-check 失败：诊断文本以 continue_session=当前 executor 会话 Continue 注入修复（同一会话，非新会话——AC-4） | 新增 |
| walk_run 反馈边独立计数与超限升格 | 边界 | 反馈循环恰 ≤5 次（STATIC_CHECK_FEEDBACK_LIMIT）且不消耗相位 retry 预算；第 5 次仍失败 → 升格相位 fail：桌面代写 fail phase-log（不跑 evaluator）后进 phase-next 重试/决策分叉（AC-4 超限半边） | 新增 |
| walk_run diff 上下文每轮取新 | 正向 | 每次 WorkerAgent prompt 组装前恰一次 diff_context 调用（executor / evaluator 双入口）；WorkerTurnRequest 载荷 prompt 含 diff 段；attempt 递进可见（第 2 attempt 收到更新的 canned diff——W5/AC-3） | 新增 |
| walk_run diff 源 Err 降级不阻断 | 异常 | diff_context 返回 Err：prompt diff 段降级为错误提示、run 不阻断（运行时依赖节「git 缺失降级」——AC-3） | 新增 |
| walk_run 步状态流出上图 | 正向 | 全程每步 emit RunUpdate::Step（phase/attempt/step/status 逐档可辨）：三类节点（WorkerAgent/ToolStep/Gate）状态均经状态流出（图上可观测输入面——AC-4/AC-5） | 新增 |
| walk_run 工具步失败显式失败 | 异常 | 假写面返回 Err(String)：run 收敛 failed 且原因经 Finished 流出（不静默空转——AC-6 失败面；ToolStepError 出局后统一 Err 承载） | 新增 |
| walk_run CLI 步失败三态映射 | 废弃 | 旧断言「Spawn/Exit/Drift 三态映射」随 ToolStepError 出局废弃（差异表 #7）；显式失败语义由上行 Err(String) 形态承接 | 废弃 |
| walk_run 补录空集不调 | 废弃 | 旧断言随 change-files 步出局废弃（file_log 出局——差异表 #3）；AC-3 写面断言改为「file_log 零触碰」落写面测试 | 废弃 |
| walk_run 空白名单 backtrack 出口 | 边界 | phase-next 白名单为空 + MaxRetriesExceeded：决策输入白名单段为空，backtrack 决议必被预校验拒绝（仅剩 retry/stop/ask 出口——AC-2） | 新增 |
| 步门控常量锚定 | 边界 | STATIC_CHECK_FEEDBACK_LIMIT == 5（沿 hook loop_limit 语义）；STATIC_CHECK_PHASES == ["implement", "test-gen"]（布局身份常量——转移判定恒问写面 phase-next，不据其路由，红线锚） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| WorkerAgentPort 假实现（注入依赖，入参例外） | 预录密封 AgentEvent 转录 fixture 组装 WorkerTurnOutcome、Mutex 捕获 WorkerTurnRequest（prompt / provenance / continue_session 断言）、可编程 Err 与停止时序 | walk_run 全部 describe |
| ToolStepPort 假写面（注入依赖，入参例外；AC-1「假写面」口径主装置） | 预录 ToolStepOutput（PhaseNextOutcome 等写面原生类型内存构造）、记录 ToolCommand 调用序与载荷、可编程 Err(String) | 步序精捕 / 决策分叉 / 故障注入 describe |
| StaticCheckRunner 假实现（注入依赖，入参例外） | 可编程 passed/diagnostics 序列（驱动反馈边计数） | 反馈边 describe |
| DiffContextPort 假实现（注入依赖，入参例外） | 按调用次序返回不同 canned diff 文本 + 调用计数（每轮取新 / attempt 递进断言）、可编程 Err（降级用例） | diff 上下文 describe |
| 真实 LocalToolSteps + 真实 workflow::write（进程内缝真实组合） | tempdir workflow.json fixture 真盘（插件直跑形态 fixture 家族）；仅 fs 属进程边界 | 真实写面组合演进对照 / 重入续走 describe |
| ChangeFlowControl / RunGuard 真实实现 | tokio 原语（broadcast/oneshot）真实参与，ask/confirm 挂起经真实通道回传，不 mock | 停等/ask/停止 describe |
| 内部模块真实组合 | verdict / decision / transcript / prompt / steps 真实参与（组合用例不 mock 内部模块） | 全部 describe |

### packages/desktop/src-tauri/crates/core/orchestration/src/verdict.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/verdict_test.rs

#### 待测功能

- parse_verdict(): 最终消息提取并校验 checklist JSON（容忍围栏代码块；漂移 Err 停给用户；签名不变）
- EvaluatorChecklist(): verdict 封闭结构——checklist 载荷换 `workflow::model::ChecklistItem`（自有 ChecklistItemView 出局，差异表 #15）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 合法裸 JSON 解析 | 正向 | evaluator 最终消息即 checklist JSON：phase/attempt/verdict/report/checklist/skipped 解析逐字段相等；checklist 行落 `workflow::model::ChecklistItem` 域类型（载荷换血承接——差异表 #15） | 新增 |
| 围栏代码块容忍 | 正向 | ```json 围栏包裹、围栏前后混杂叙述文本：提取解析成功（容忍围栏约定） | 新增 |
| verdict 值域封闭 | 边界 | pass/fail 两值可辨；值域外字符串 → Err（封闭集） | 新增 |
| report 超长拒绝 | 异常 | report 大于 500 字符 → Err 拒绝（解析层先拒、写面落账层再拒双闸——AC-9） | 新增 |
| checklist 行解析 | 边界 | 空数组、多行 item/pass/evidence、attempt 缺席形态解析无损 | 新增 |
| 结构漂移显式失败 | 异常 | 缺 verdict、checklist 行缺 evidence、非 JSON 文本、纯叙述无 JSON：均 Err 停给用户（不臆测 verdict、不静默） | 新增 |
| skipped 形态 | 边界 | skipped true/false/缺席三形态解析（桌面代写升格 fail 场景的载荷面） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（无进程边界依赖） | 最终消息以字符串 fixture 内存构造（合法 / 围栏 / 漂移 / 超长四族） | 全部 describe |

### packages/desktop/src-tauri/crates/core/orchestration/src/decision.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/decision_test.rs

#### 待测功能

- ensure_backtrack_allowed(): 越权 backtrack 预校验（walker 侧第一道；写面 backtrack 二次校验兜底——白名单类型换 `Vec<String>`）
- （保持面）决策解析器: 决策 agent 最终消息 → 四动作封闭集 + 围栏容忍（design「保持不变」清单延续，非本变更 API 新增——用例为回归锚）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 四动作合法解析（回归锚） | 正向 | backtrack{to, reason} / retry / stop{reason} / ask{question, options[]} 四变体各自解析（封闭集——AC-2 协议面） | 新增 |
| 围栏容忍（回归锚） | 正向 | 围栏代码块包裹的决策 JSON 提取解析成功（与 verdict 同款容忍） | 新增 |
| 越权 backtrack 预校验 | 异常 | backtrack to ∉ 白名单（Vec<String> 字面集）：ensure_backtrack_allowed Err（walker 第一道闸——AC-2） | 新增 |
| 白名单命中放行 | 正向 | backtrack to ∈ 白名单：Ok（写面 backtrack 二次校验仍兜底——双层防线锚定） | 新增 |
| 空白名单 backtrack | 边界 | allowed 空数组 + 任何 backtrack：Err（无跳转出口） | 新增 |
| 结构漂移显式失败（回归锚） | 异常 | 未知 action、backtrack 缺 to/reason、options 非数组、非 JSON 文本：均 Err 停给用户（不臆测决议） | 新增 |
| reason 超长解析层承接 | 边界 | reason 大于 500 字符：解析层原样承接不崩；超长拒绝由写面 backtrack 承载（分层锚更新：原「walker-backtrack 工具侧校验」改写面——backtrack_test 对端行） | 新增 |
| DecisionInput 有界组装（回归锚） | 边界 | fail_checklist 空 / 多行、candidates verdict/report 双 None（候选相位从未评估）形态构造合法；白名单已换 Vec<String>（决策 prompt 组装输入面） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（无进程边界依赖） | 决策文本与 DecisionInput / 白名单 fixture 内存构造 | 全部 describe |

### packages/desktop/src-tauri/crates/core/orchestration/src/transcript.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/transcript_test.rs

#### 待测功能

<!-- design.md 公共函数/API 表未声明该文件的条目（修改行：删 `extract_write_paths` 半边，`final_assistant_text` 保留——verdict/决策 JSON 载体）。本节承载砍半断言：提取器族标废弃，final_assistant_text 为保留面回归锚（来源：变更清单修改行 + AC-2/AC-3 回溯相关行）。不建独立新行为用例。 -->

（design.md 未声明该文件的公共 API 变更——保留面 final_assistant_text 与删除面 extract_write_paths 的断言见下表。）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| final_assistant_text 提取（保留面回归锚） | 正向 | 末条 assistant Message 多文本块按序拼接（verdict / 决策 JSON 载体保真——AC-2 解析输入面） | 新增 |
| final_assistant_text 空态（保留面回归锚） | 异常 | 无 assistant Message（仅 user/工具事件）→ None（解析层显式缺席，verdict Err 的前置） | 新增 |
| extract_write_paths 提取族 | 废弃 | 旧断言「Write/Edit/NotebookEdit 路径提取、去重保序」随半边删除废弃（差异表 #4；file_log 出局——AC-3 重写为 git diff 上下文） | 废弃 |
| extract_write_paths 空态与非目标工具族 | 废弃 | 旧断言「空集合法空态 / 非目标工具忽略 / 多轮合并」同上废弃 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（无进程边界依赖） | 密封 AgentEvent 转录以内存 Vec fixture 构造（多文本块 / 空态两族） | 保留面 describe |

### packages/desktop/src-tauri/crates/core/orchestration/src/prompt.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/prompt_test.rs

#### 待测功能

- executor_prompt(): 内置角色要点前导 + 已插值 phase prompt + git diff 上下文段（增 diff 参数）
- evaluator_prompt(): 输出协议附录（禁调 MCP phase-log；最终消息输出 checklist JSON 形状）+ git diff 上下文段（change/phase 信封已由写面插值，参数出局）
- decision_prompt(): 决策有界输入组装（fail checklist + 白名单 + 候选 eval report + 四动作封闭集说明 + reason ≤500；保持）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| executor_prompt 三段组装 | 正向 | agent_type + 已插值 phase_prompt + diff_context：角色要点前导在场、prompt 主体保真（换行 / 中文 / 特殊字符不损）、diff 段在场（AC-3 组装半边） | 新增 |
| executor_prompt 空 diff_context | 边界 | diff 上下文缺席形态（git 缺失降级）组装不崩、段缺席形态稳定 | 新增 |
| evaluator_prompt 协议附录 + diff 段 | 正向 | 附录含禁调 MCP phase-log 指令与 checklist JSON 形状约定；diff_context 段在场（AC-2 evaluator 输出协议 + AC-3 的 prompt 半边） | 新增 |
| evaluator_prompt change/phase 信封插值 | 废弃 | 旧断言「附录含 change/phase 信封插值」随参数出局废弃（写面插值接管——API 表 evaluator_prompt 行） | 废弃 |
| decision_prompt 有界组装（保持面） | 正向 | DecisionInput 全量组装：fail checklist 行、白名单行、候选 eval report、四动作封闭集说明、reason ≤500 约定逐段在场（AC-2 有界输入半边） | 新增 |
| decision_prompt 空集边界 | 边界 | fail_checklist 空 / 白名单空 / candidates 空三态组装不崩、段落缺席形态稳定 | 新增 |
| prompt 确定性 | 边界 | 同输入重复组装输出逐字节一致（无时钟/随机参与） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（无进程边界依赖） | phase_prompt / diff_context / DecisionInput 纯字符串与结构体入参内存构造；角色要点表为内置静态数据不 mock | 全部 describe |

### packages/desktop/src-tauri/crates/core/orchestration/src/state.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/state_test.rs

#### 待测功能

- RunUpdate(): Channel 信封枚举（tag `ipc`）：Step / SessionEvent / Ask / ConfirmWait / Finished（serde + specta Type）
- ChangeRunStatus(): running/waitingConfirm/waitingAsk/completed/stopped/failed（camelCase 线格式；is_terminal()）
- ChangeStepKind(): 九变体步词汇：executor/evaluator/decision/phaseStart/staticCheck/phaseLog/verdictGate/retryGate/whitelistGate（ChangeFiles 变体出局）
- ChangeStepStatus / ChangeStepState(): 步状态四档；步状态行 { phase, attempt, step, status, session_id, detail }
- AskPayload / ChangeRunSnapshot / ChangeRunSummary(): ask 载荷；重挂快照；发起提前 resolve 返回值

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| RunUpdate 五变体线格式 | 正向 | tag=ipc 判别逐变体（step/sessionEvent/ask/confirmWait/finished）camelCase 键 serde 往返逐字段相等（IPC 直用线面——AC-5 状态流信封前提） | 新增 |
| SessionEvent 变体 | 正向 | session_id + 密封 AgentEvent 载荷往返保真（节点转录联动实时流信封） | 新增 |
| Finished 变体 | 正向/边界 | status + reason（None 与有值两形态）往返（终态流出信封） | 新增 |
| ChangeRunStatus 终态矩阵 | 正向 | 六值线格式逐字断言；is_terminal() 互补矩阵（completed/stopped/failed 为 true，running/waitingConfirm/waitingAsk 为 false） | 新增 |
| ChangeStepKind 九值词汇 | 边界 | 九变体线格式逐字断言、三类可辨：executor/evaluator/decision（WorkerAgent）、phaseStart/staticCheck/phaseLog（ToolStep）、verdictGate/retryGate/whitelistGate（Gate）（AC-5 可辨半边的词汇面；ChangeFiles 出局后收缩形态） | 新增 |
| ChangeStepKind 含 changeFiles 变体 | 废弃 | 旧断言「十值词汇含 changeFiles」随补录通道出局废弃（差异表 #3；类型定义表收缩行） | 废弃 |
| ChangeStepState 步状态行 | 边界 | running/passed/failed/stopped 四档；字段（phase/attempt/step/status/sessionId/detail）缺省形态往返无损 | 新增 |
| ChangeRunSnapshot / ChangeRunSummary | 正向 | 重挂快照与发起摘要全字段 serde/specta 往返（use-change-flow-run 恢复输入面） | 新增 |
| specta Type 可达 | 边界 | 上述类型经 specta 导出面编译期锚定（IPC 绑定生成前提，bindings 零 diff 链路的类型半边） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（无进程边界依赖） | serde_json / specta 内存构造与往返断言，AgentEvent 载荷以固定时间戳构造 | 全部 describe |

### packages/desktop/src-tauri/crates/core/orchestration/src/control.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/control_test.rs

#### 待测功能

<!-- design.md 公共函数/API 表未声明该文件的条目（修改行：注释与错误消息 CLI 通道措辞清理，逻辑零改动）。本节为回归锚：既有注册表语义用例全数保留（零行为变更的回归面）；待测功能来源为组件表「run 控制注册表」行。 -->

- ChangeFlowControl.new(): 空注册表
- ChangeFlowControl.begin_run(): 并行冲突检测（同 change 已有 run → Err）；登记 cancel 标志与 broadcast
- ChangeFlowControl.subscribe(): `change_flow_start` / `change_flow_watch` 共用订阅入口
- ChangeFlowControl.request_stop(): 置 cancel 标志；miss（无运行 run）幂等返回 false
- ChangeFlowControl.answer / confirm(): ask 应答 / phase 间确认经单次通道回传 walker（无等待方 Err）
- ChangeFlowControl.snapshot(): 视图重挂快照恢复（进程内，run 终态后为 None）
- RunGuard(): emit / current_session 槽 / wait_confirm / wait_answer / cancelled / finish（终态除名、幂等收口）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| begin_run 登记与并行冲突 | 正向 | 新 change begin_run 成功返回 RunGuard；同 change 二次 begin → Err（并行冲突检测——change_flow_start 前置校验第三分支的输入面） | 新增 |
| 跨 change 并行隔离 | 边界 | 两个 change 各自 begin_run 并行互不干扰（键 = change 名） | 新增 |
| request_stop 置位与幂等 | 正向 | 运行中置位 → true 且 guard.cancelled() 同步观测；无运行 run → false 幂等不报错（change_flow_stop 幂等语义的输入面——AC-7 停止半边） | 新增 |
| subscribe 有无运行 | 边界 | 有 run → Some(receiver)；无 run → None（change_flow_watch 消费形态） | 新增 |
| emit → broadcast 流出 | 正向 | guard.emit(update) 经 subscribe receiver 收到同值信封（RunUpdate 流出口） | 新增 |
| set_session 槽往返 | 边界 | 置 Some/None 往返（停止寻址的当前会话槽） | 新增 |
| confirm 应答回传 | 正向 | guard.wait_confirm 挂起后 confirm(proceed=true/false) 回传布尔（false 的收敛归 walker） | 新增 |
| confirm 无等待方 | 异常 | 无 guard 挂起时 confirm → Err（单次通道语义，change_flow_confirm 错误面输入） | 新增 |
| answer 应答回传 | 正向 | wait_answer 挂起后 answer(text) 回传原文（ask 应答回流） | 新增 |
| answer 无等待方 | 异常 | 无等待方 answer → Err | 新增 |
| finish 终态除名 | 正向 | finish(status) 后 snapshot → None、request_stop → false、begin_run 可重新登记（幂等收口——AC-7「run 仅进程内」半边） | 新增 |
| finish 幂等 | 边界 | 重复 finish 不改写首个终态；snapshot 在 finish 前返回 Some（状态机镜像） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（进程内 tokio 原语真实组合） | #[tokio::test] 多任务挂起/唤醒：broadcast/oneshot/Mutex 真实参与，不 mock | 全部 describe |

### packages/desktop/src-tauri/crates/core/orchestration/src/snapshot.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/snapshot_test.rs

#### 待测功能

<!-- design.md 公共函数/API 表未声明该文件的条目（修改行：doc 注释语义换血，W6 保留缩水——FsSnapshot 与 detail 只读装配原样）。回归锚：待测功能来源为组件表「快照 port 实现」行。 -->

- FsSnapshot.new(): `WorkflowSnapshotPort` 实现：foundation Layout + `workflow::change_detail` 只读装配（W6 缩水后纯读）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| detail 只读装配 | 正向 | tempdir 真实 change fixture（合法 workflow.json + openspec 目录骨架）：ChangeDetail 装配逐字段（决策输入与前置校验的输入面——AC-2 有界输入来源） | 新增 |
| change 不存在 | 异常 | detail(root, 不存在 change) → Err 显式（change_flow_start 前置校验第二分支输入面） | 新增 |
| workflow.json 不可解析 | 异常 | 预置损坏 workflow.json → Err 显式（不静默空 detail） | 新增 |
| 只读性 | 边界 | 调用前后 workflow.json 字节不变（读不属写通道约束的锚——AC-6） | 新增 |
| root 寻址 | 边界 | root 指向不同 workspace 目录：各取各 change（跨 workspace 隔离） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 store mock（fs 进程边界真实组合） | tempdir 真实 change fixture（workflow.json 以真盘落盘），沿 core/workflow detail_test 装置先例 | 全部 describe |

### packages/desktop/src-tauri/crates/core/orchestration/src/lib.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/lib_test.rs

#### 待测功能

<!-- design.md 公共函数/API 表未声明该文件的条目（修改行：摘除 `mod views` 声明、挂载 `mod steps`——模块清单面）。不建 lib_test.rs（防空套件红灯），见不可测试项 4。 -->

（design.md 未声明该文件的公共 API 变更——不建测试文件，可达性经 walker_test / steps_test 等的 crate 根 use 导入编译期承载。）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| （无——见不可测试项 4） | — | crate 根导出面无独立行为断言：walker_test / steps_test 以 crate 根 use 导入任一破坏即编译失败 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 不适用（不建测试文件） | — |

### packages/desktop/src-tauri/crates/infra/agent/src/static_check.rs -> packages/desktop/src-tauri/crates/infra/agent/src/static_check_test.rs

#### 待测功能

- ProcessStaticCheck.new(): `StaticCheckRunner` 实现：`core::config` 读 static_analysis 命令 → `tokio::process` spawn（cwd=workspace root）→ 诊断捕获 + 退出码；无配置/空命令 = passed 直接过（W4）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 通过形态 | 正向 | tempdir workspace + config 预置可通过命令：passed=true、diagnostics 空、退出码 0；cwd=root 断言（命令以相对路径读到 root 下文件——W4 spawn 语义） | 新增 |
| 失败形态 | 异常 | 预置失败命令：passed=false、diagnostics 含诊断文本（stderr/stdout 捕获不丢——反馈边注入原料，AC-4） | 新增 |
| 无配置/空命令直接过 | 边界 | config 无 static_analysis / 命令空串：passed=true 且零 spawn（W4 直过语义） | 新增 |
| 非零退出码判定 | 边界 | 命令非零退出但无诊断输出：passed=false（退出码即判据，不因诊断空而误判通过） | 新增 |
| spawn 失败 | 异常 | 命令不存在 → Err(String)（显式失败不静默 passed——AC-4 记因面） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock（子进程边界真实组合） | 真实可执行命令 fixture（回显 / 退出码可编程的脚本）+ tempdir workspace 的 `core::config` 真实读取；不 mock `tokio::process` 本体（沿既有真实子进程装置先例） | 全部 describe |

### packages/desktop/src-tauri/crates/infra/agent/src/git_diff.rs -> packages/desktop/src-tauri/crates/infra/agent/src/git_diff_test.rs

#### 待测功能

- GitDiffSource.new(): `DiffContextPort` 实现：`git status --porcelain` 文件清单 + `git diff HEAD` 补丁体拼接，`DIFF_CONTEXT_LIMIT` 字符截断带省略标记（W5）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 变更清单与补丁拼接 | 正向 | tempdir git 仓库（已跟踪修改 + 未跟踪新文件）：输出含 porcelain 清单（含 untracked——W5 porcelain 补齐 `git diff HEAD` 盲区语义）与 diff HEAD 补丁体（AC-3 spawn 半边） | 新增 |
| DIFF_CONTEXT_LIMIT 截断 | 边界 | 超大 diff → 截断且带省略标记（不无限膨胀 prompt） | 新增 |
| 干净工作树空态 | 边界 | 无变更 → 空上下文（合法空态，prompt 段缺席形态） | 新增 |
| 非 git 目录 | 异常 | tempdir 非 git 目录：Err 显式（降级由消费方承接；不 panic、不以空串冒充成功） | 新增 |
| git 缺失 PATH | 异常 | PATH 隔离无 git：Err 显式（运行时依赖节「缺失时 diff 上下文降级为错误提示」的源头面） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock（子进程边界真实组合） | 真实 git 二进制 + tempdir 仓库 fixture（git init/add/commit 装置）；git 缺失用例以 PATH 目录替换 + 共享互斥锁串行化、测毕恢复（沿既有 PATH 隔离装置） | 全部 describe |

### packages/desktop/src-tauri/crates/infra/agent/src/worker.rs -> packages/desktop/src-tauri/crates/infra/agent/src/worker_test.rs

#### 待测功能

<!-- design.md 公共函数/API 表未声明该文件的条目（修改行：既有实现继续，`WorkerAgentPort` 契约未变，仅随 port 中性类型调整核对编译面）。本节为 AC-7 装配半边回归锚：待测功能来源为组件表「WorkerAgentPort 内核适配」行；实现期沿 `*_with` 泛型缝先例择缝，测试缝不属本设计新增导出。 -->

- WorkerAgentPort 内核适配(): compose_turn 装配 + begin_turn + 泵驱动收集密封转录与终态；事件经 `RunEventSink` 透传；provenance / permission 组装（既有工作树文件继续）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| provenance/permission/continue 装配 | 正向 | WorkerTurnRequest → 会话请求转换捕获断言：source="change"、source_ref 按 `<change>/<phase>/<role>/<attempt>` 定式、permission 恒 bypassPermissions、continue_session Some → Continue 引用（AC-7 装配半边；实现期沿 `*_with` 泛型缝先例择缝） | 新增 |
| 缺省解析 Err | 异常 | 空库（无默认 agent）：Err 携管理页引导语义（显式失败不静默） | 新增 |
| CLI 引擎不可达 | 异常 | PATH 隔离触发 CliMissing：Err 传播且无半成品记录（open 阶段失败不产生任何记录的内核契约承接） | 新增 |
| 泵收集与透传 | 正向 | compose 装配产物 + 假 runner 装置（沿 kernel_test 同型装置互锚）：transcript 收密封事件全集、final_message 承接终态、观察事件以 RunUpdate::SessionEvent 透传假 sink 载荷保真（AC-7 反查数据面 / AC-5 实时流来源） | 新增 |
| 停止收敛 | 边界 | 停止置位后驱动终止：status=stopped 收敛、已收集事件保留（AC-7 停止半边） | 新增 |
| 角色三值 provenance | 边界 | executor/evaluator/decision 三角色：sourceRef role 段逐字对应（决策会话与转录联动共用同一定式） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| RunEventSink 假实现（注入依赖，入参例外） | Arc+Mutex Vec 捕获 RunUpdate::SessionEvent 流（构造经 new 注入） | 泵收集与透传 describe |
| 会话引擎侧假 runner 装置（注入依赖） | compose 装配产物与 kernel_test 同型假 runner 直接驱动泵（预录密封事件、可编程 Err/停止时序），不 spawn 真实 claude | 泵收集/停止 describe |
| tempdir 真库 WorkspaceStores（DB 进程边界真实组合） | 会话行/provenance 经真库落盘断言；缺省解析用空库 fixture | 全部 describe |
| PATH 环境变量（进程全局变量边界） | 空 PATH 目录替换 + 共享互斥锁串行化、测毕恢复，触发 CliMissing | CLI 引擎不可达 describe |

### packages/desktop/src-tauri/crates/infra/agent/src/lib.rs -> packages/desktop/src-tauri/crates/infra/agent/src/lib_test.rs

#### 待测功能

<!-- design.md 公共函数/API 表未声明该文件的条目（修改行：增 `mod static_check;` / `mod git_diff;` 与再导出——清单面）。不建 lib_test.rs（防空套件红灯），见不可测试项 4。 -->

（design.md 未声明该文件的公共 API 变更——不建测试文件，可达性经 static_check_test / git_diff_test / worker_test 的 crate 根 use 导入编译期承载。）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| （无——见不可测试项 4） | — | crate 根导出面无独立行为断言：static_check_test / git_diff_test 以 crate 根 use 导入任一破坏即编译失败 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 不适用（不建测试文件） | — |

### packages/desktop/src-tauri/src/commands/change_flow/mod.rs -> packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs

#### 待测功能

- change_flow_start(): 发起：前置校验三道换血（workflow_type=requirement / change 存在且 workflow.json 可解析 / 无并行 run——W8）→ 提前 resolve 返回运行态摘要；组合根装配换血（LocalToolSteps + ProcessStaticCheck + GitDiffSource + FsSnapshot + run 级 SessionAnchors——W7/W8）；walker 后台驱动
- change_flow_stop() / change_flow_answer() / change_flow_confirm() / change_flow_state() / change_flow_watch(): 五命令保持（签名不变，逻辑零改动）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| start 前置校验三失败分支（换血后） | 异常 | workflow_type 非 requirement（写面 phase_table None → 显式 Err，W8）/ change 不存在或 workflow.json 不可解析 / 同 change 已有并行 run：各自 Err 且成因互不重合、零 spawn 副作用 | 新增 |
| start 前置校验含 CLI 可发现项 | 废弃 | 旧断言「CLI 不可发现（env 指坏路径）显式报错」随 discover_cli 出局废弃（差异表 #8/#14；DEV_TEAM_PLUGIN_ROOT 环境变量随 discover.rs 删除出局） | 废弃 |
| start 提前 resolve 与组合根装配 | 正向 | `*_with` 泛型缝注入假 port 组（沿 agent_start_with 先例）：返回 ChangeRunSummary（running 态）且 Channel 首事件到达、后台驱动不阻塞；装配面为 LocalToolSteps + ProcessStaticCheck + GitDiffSource + FsSnapshot + run 级 SessionAnchors（W7/W8 装配锚） | 新增 |
| stop 运行中置位 | 正向 | 运行中 change_flow_stop：注册表 cancelled 置位 + 当前会话槽承接（Ok） | 新增 |
| stop 幂等忽略 | 边界 | 非运行态/未知 change：Ok 幂等忽略不报错（AC-7 停止幂等半边） | 新增 |
| answer/confirm 无等待方 | 异常 | 无挂起 ask/确认时调用：Err（单次通道错误面透传） | 新增 |
| state 快照查询 | 正向 | 运行中 → Some(ChangeRunSnapshot)；无 run / 终态后 → None（重挂恢复输入面） | 新增 |
| watch 补订 | 正向 | 运行中 change_flow_watch：订阅成功且后续 RunUpdate 到达（重挂补订） | 新增 |
| 参数转换守卫 | 边界 | blank root / blank change：Err（root 寻址与 `Result<T, String>` 模板保留） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| tauri mock_app（MockRuntime，Tauri 运行环境替身） | 托管真实 ChangeFlowControl（tempdir root），State 取用（沿既有命令测试装置） | 全部 describe |
| `*_with` 泛型缝注入假 port 组（注入依赖，入参例外） | 预录输出假 ToolStepPort + 预录转录假 WorkerAgentPort + 假 RunEventSink；成功链路提前 resolve | start 提前 resolve describe |
| Channel 捕获（IPC 边界） | tauri::ipc::Channel::new 捕获回调（首事件/补订流断言） | start/watch describe |
| tempdir 真实 change fixture（fs 进程边界） | 前置校验失败/成功分支以真盘 fixture 驱动（CLI env 替身随回退出局，不再 mock 环境变量） | 前置校验 describe |

### packages/desktop/src/views/changes/flow/run-state.ts -> packages/desktop/src/views/changes/flow/run-state.test.ts

#### 待测功能

- initialRunState(): 快照 → 前端 run 视图模型初值
- applyRunUpdate(): 纯归并：步状态 / SessionEvent 缓存 / ask / 确认等待 / 终态
- runStepNodes(): run 视图模型 → 图 overlay 运行步节点（id `run:<phase>:<attempt>:<step>[:<seq>]`）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| initialRunState 空态 | 边界 | null 快照 → null（无运行 run 的空闲态，重挂恢复输入面） | 新增 |
| initialRunState 快照恢复 | 正向 | ChangeRunSnapshot → 视图模型（状态机镜像 + 步状态表逐字段恢复） | 新增 |
| applyRunUpdate Step 归并 | 正向 | 新步插入与既有步状态推进（running→passed/failed）：纯归并、入参 state 不被突变（纯函数无副作用——AC-5 归并半边） | 新增 |
| SessionEvent 缓存 | 正向 | 按会话缓存最近事件（节点转录联动 liveEvents 输入面） | 新增 |
| Ask / ConfirmWait 等待态 | 正向 | question/options 与 phase 等待态设置（控制面板卡片输入面） | 新增 |
| Finished 终态收口 | 正向 | status+reason 收口；终态后后续 update 不再改写（幂等收口） | 新增 |
| 乱序与重复 update | 边界 | 重复 Step 幂等归并、终态后 update 忽略（Channel 流乱序容忍） | 新增 |
| runStepNodes 定式 | 正向 | 视图模型 → RuntimeFlowNode[]：id `run:<phase>:<attempt>:<step>[:<seq>]` 逐字定式、runStepKind/role/status/sessionId 载荷承接、三分类可辨（AC-5 overlay 输入面） | 新增 |
| runStepNodes 空态 | 边界 | 空步表 → 空数组（overlay 缺省输出与现状一致的输入半边） | 新增 |
| 空缓存与缺省字段 | 边界 | 无 SessionEvent 缓存、detail 缺省、role null 形态归并/推导不崩 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（无进程边界依赖） | RunUpdate/ChangeRunSnapshot fixture 以绑定类型内存构造；纯函数直测零 react 依赖 | 全部 describe |

### packages/desktop/src/views/changes/hooks/use-change-flow-run.ts -> packages/desktop/src/views/changes/hooks/use-change-flow-run.test.ts

#### 待测功能

- useChangeFlowRun(): invoke 六命令 + Channel 订阅生命周期（运行期订阅、收口释放）+ 重挂快照恢复
- UseChangeFlowRunResult(): 控制面板与视图消费面（state/start/stop/confirm/answer/error）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 挂载快照恢复 | 正向 | 挂载 invoke("change_flow_state")：Some → initialRunState 恢复、None → null state（重挂恢复） | 新增 |
| start 发起与订阅 | 正向 | start() → invoke("change_flow_start") 携 root/change、Channel 订阅建立；RunUpdate 信封流入归并 state（运行期订阅） | 新增 |
| stop/confirm/answer 透传 | 正向 | 三操作各自 invoke 传参透传（root/change/proceed/answer 原样） | 新增 |
| 终态释放订阅 | 边界 | Finished 收口后 unlisten 释放（收口释放纪律）；终态后到站信封不再改写 state | 新增 |
| invoke reject | 异常 | start/state reject：error 呈现可重试（state 操作不崩） | 新增 |
| 重挂补订 | 正向 | 卸载重挂：state 快照恢复 + change_flow_watch 补订、实时流不中断 | 新增 |
| 返回形状消费面 | 边界 | UseChangeFlowRunResult 键集与形状（state/start/stop/confirm/answer/error）不回退 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| invoke/Channel（@tauri-apps/api/core，进程边界） | invoke 按命令名分发（六命令，可切换 resolve/reject 并记录入参）；Channel 可编程 class 投递 RunUpdate 信封驱动归并 | 全部 describe |
| run-state 纯函数（不 mock） | 真实实现参与归并（内部模块不 mock——最小 mock 原则） | 全部 describe |

### packages/desktop/src/views/changes/hooks/use-session-transcript.ts -> packages/desktop/src/views/changes/hooks/use-session-transcript.test.ts

#### 待测功能

- useSessionTranscript(): `agent_sessions(source='change', sourceRef)` 反查 → `agent_session_transcript` 重放 → agent-adapter 适配 → 实时事件按 seq 去重并入

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 反查重放装载 | 正向 | 反查命中（source='change' + sourceRef 精确过滤）→ 转录重放 → agent-adapter 折叠 messages（AC-5 转录联动重放半边） | 新增 |
| sourceRef null 空闲态 | 边界 | sourceRef null：零 invoke、messages 空（未选中节点的空闲态） | 新增 |
| 反查无会话 | 边界 | 反查空清单：messages 空、error null（节点尚无会话的合法空态） | 新增 |
| 实时去重并入 | 正向 | liveEvents 按 seq 去重：重放已含 seq 的实时事件不重复并入；新 seq 事件追加增长（运行中实时——AC-5 实时半边） | 新增 |
| running 标志 | 边界 | 会话运行中/收口两形态 running 标志切换 | 新增 |
| invoke reject | 异常 | 反查/重放 reject：error 呈现（面板可感知） | 新增 |
| 反查参数定式 | 边界 | invoke 入参 source 恒 'change'、sourceRef 原样透传（exact-match 过滤契约） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| invoke（@tauri-apps/api/core，进程边界） | invoke 按命令名分发（agentSessions/agentSessionTranscript，可切换 resolve/reject 并记录入参） | 全部 describe |
| agent-adapter（不 mock） | 真实实现参与折叠与去重（内部模块不 mock） | 全部 describe |

### packages/desktop/src/views/changes/flow/run-step-node.tsx -> packages/desktop/src/views/changes/flow/run-step-node.test.tsx

#### 待测功能

- RunStepNode(): 类型徽章（WorkerAgent/ToolStep/Gate）+ pulse 运行态 + 失败红态
- RunStepFlowNode(): react-flow 节点类型（`Node<RunStepNodeData, 'runStep'>`）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 三分类徽章可辨 | 正向 | runStepKind 三族九值（executor/evaluator/decision；phaseStart/staticCheck/phaseLog；verdictGate/retryGate/whitelistGate——ChangeFiles 出局后收缩形态）：WorkerAgent/ToolStep/Gate 徽章渲染可辨（data-testid 查询挂钩——AC-5 可辨半边） | 新增 |
| pulse 运行态 | 正向 | status running → pulse 呈现（AC-5 实时呈现半边） | 新增 |
| 失败红态 | 异常 | status failed → 失败红态呈现（static-check 挂了 = 红节点与 executor 挂同等可见） | 新增 |
| 静态终态 | 边界 | passed/stopped：无 pulse、样式收敛不残留运行态 | 新增 |
| 载荷缺省形态 | 边界 | sessionId/role 缺省（null）：渲染不炸、徽章仍可辨 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（无进程边界依赖） | NodeProps fixture 以 react-flow Node 类型内存构造直传（data-testid 查询挂钩） | 全部 describe |

### packages/desktop/src/views/changes/flow/run-control-panel.tsx -> packages/desktop/src/views/changes/flow/run-control-panel.test.tsx

#### 待测功能

- RunControlPanel(): 发起/停止/phase 间确认/ask 应答卡片；生命周期状态与可用操作对齐（收口后停止不再为主操作）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 空闲发起入口 | 正向 | state null/终态：发起为主操作、停止不再为主操作（收口对齐断言） | 新增 |
| running 停止入口 | 正向 | running：停止可用且为主操作（AC-5 运行控制入口半边） | 新增 |
| waitingConfirm 卡片 | 正向 | ConfirmWait：确认/终止卡片呈现，proceed true/false 分别触发 confirm（phase 间拍板点） | 新增 |
| waitingAsk 卡片 | 正向 | Ask：question 与 options[] 呈现、选择/应答回流 answer（AC-2 ask 卡片 UI 半边） | 新增 |
| error 呈现 | 异常 | error 字段呈现（发起失败可见可重试） | 新增 |
| 六态操作矩阵 | 边界 | running/waitingConfirm/waitingAsk/completed/stopped/failed 六态的可用操作对齐矩阵不回归 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| UseChangeFlowRunResult 替身（注入 props，入参例外） | 可编程 state（六态 fixture）与 start/stop/confirm/answer spy；无 invoke 参与（hook 边界在 use-change-flow-run.test 锁定） | 全部 describe |

### packages/desktop/src/views/changes/flow/session-transcript-panel.tsx -> packages/desktop/src/views/changes/flow/session-transcript-panel.test.tsx

#### 待测功能

- SessionTranscriptPanel(): role 分页 + `AgentTimeline`（运行中实时 / 收口重放一致，无第二套时间线组件）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| role 分页选择 | 正向 | roleRefs 多角色（executor/evaluator/decision）：分页可切换、选中 role 的 sourceRef 定式透传反查（role×attempt → sourceRef 组装——AC-5 联动半边） | 新增 |
| AgentTimeline 渲染 | 正向 | 密封重放消息经 AgentTimeline 呈现（组件复用锚：运行中实时与收口重放同一条时间线） | 新增 |
| liveEvents 透传 | 正向 | 运行中实时事件 props 下传到达反查 hook（并入增长） | 新增 |
| 空态 | 边界 | roleRefs 空 / 无会话：面板空态呈现不炸 | 新增 |
| 单角色退化 | 边界 | roleRefs 单元素：无分页歧义、直接呈现 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| invoke（@tauri-apps/api/core，进程边界） | fixture 化反查/转录响应（经 use-session-transcript 底层流入真实 hook） | 全部 describe |
| AgentTimeline（不 mock） | 真实实现参与渲染（内部组件不 mock，沿 agent-run-history 先例） | 全部 describe |

### packages/desktop/src/views/changes/flow/graph.ts -> packages/desktop/src/views/changes/flow/graph.test.ts

#### 待测功能

<!-- design.md 公共函数/API 表未声明该文件的条目；本节锚定「修改文件」表行：buildFlowGraph(detail, runNodes?) 增可选第二参——运行步节点按归并序参与同一条链参与边推导；时间序边推导规则、9 列布局、attempt 缺号兜底不变；缺省/空参输出与现状一致。 -->

- buildFlowGraph(): 增可选运行步节点 overlay 参数（既有纯转换函数签名扩展，缺省语义不变）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 缺省/空参零 diff 回归 | 边界 | buildFlowGraph(detail) 与 buildFlowGraph(detail, []) 输出逐字段相等且与既有场景基线一致（缺省输出与现状一致——AC-5 回归半边、desktop-change-flow-view 既有场景全绿） | 新增 |
| runNodes 参与归并链 | 正向 | 传入 RuntimeFlowNode[]：运行步节点按归并序插入同一条链、边推导规则不变（每个非首节点恰一条入边延伸至运行步节点——执行图=展示图） | 新增 |
| 三分类 overlay 上图 | 正向 | WorkerAgent/ToolStep/Gate 三类运行步节点经 overlay 上图节点可辨（kind/runStepKind 载荷承接） | 新增 |
| attempt 交错归并 | 边界 | 运行步节点与既有事件节点按时间序交错（attempt 归并序、缺号兜底不变） | 新增 |
| 混合终态 overlay | 边界 | runNodes 含 failed/stopped 节点混合归并稳定不炸 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（无进程边界依赖） | ChangeDetail 与 RuntimeFlowNode fixture 沿既有手工构造装置 | 全部 describe |

### packages/desktop/src/views/changes/flow/types.ts -> packages/desktop/src/views/changes/flow/types.test.ts

#### 待测功能

<!-- design.md 公共函数/API 表未声明该文件的条目（类型定义表：RuntimeFlowNode / RunStepNodeData / RoleSessionRef——纯类型模块）。本 change 不建 types.test.ts（防空套件红灯），见不可测试项 4。 -->

（纯类型模块无运行时行为——不建测试文件，形状由 run-state.test.ts / graph.test.ts / run-step-node.test.tsx 的构造与消费编译期锚定。）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| （无——见不可测试项 4） | — | RuntimeFlowNode/RunStepNodeData/RoleSessionRef 无运行时断言落点：构造面由消费方用例编译期承载 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 不适用（不建测试文件） | — |

### packages/desktop/src/views/changes/flow/change-flow-graph.tsx -> packages/desktop/src/views/changes/flow/change-flow-graph.test.tsx

#### 待测功能

<!-- design.md 公共函数/API 表未声明该文件的导出函数；本节锚定「修改文件」表行：nodeTypes 增 runStep 注册、ChartNode 联合扩展、交互上抛不变。 -->

（design.md 未声明该文件的公共 API 变更——nodeTypes 注册面与渲染行为见下表。）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| runStep 节点渲染 | 正向 | 含 RuntimeFlowNode 的图：nodeTypes 解析渲染 runStep 组件（data-testid 可辨、不炸——AC-5 上图半边） | 新增 |
| 既有三分类渲染回归 | 边界 | 既有事件/列节点渲染与交互上抛零回归（nodeTypes 扩展为加法——ChartNode 联合扩展不破坏既有分支） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（沿既有装置） | 沿既有 change-flow-graph.test.tsx 装置（ResizeObserver / getBBox jsdom 垫片），无新增进程边界 mock | 全部 describe |

### packages/desktop/src/views/changes/flow/detail-drawer.tsx -> packages/desktop/src/views/changes/flow/detail-drawer.test.tsx

#### 待测功能

<!-- design.md 公共函数/API 表未声明该文件的导出函数；本节锚定「修改文件」表行：WorkerAgent 运行节点与历史 eval 节点的会话转录联动区（role×attempt → sourceRef 组装 + 实时事件透传）。 -->

（design.md 未声明该文件的公共 API 变更——转录联动接线行为见下表。）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| WorkerAgent 运行节点联动 | 正向 | 选中运行 executor 节点：SessionTranscriptPanel 渲染且 sourceRef 按 `<change>/<phase>/<role>/<attempt>` 定式组装（AC-5 点击联动半边） | 新增 |
| eval 历史节点联动 | 正向 | 选中历史 eval 节点：历史会话转录联动（role=evaluator/attempt 对应重放） | 新增 |
| 非会话节点无转录区 | 边界 | ToolStep/Gate/既有事件节点：无转录联动区（既有三分节回归——detail 既有场景零 diff） | 新增 |
| liveEvents 透传 | 边界 | 运行中实时事件 props 下传到达面板（sessionRefs/liveEvents 链路接通） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| invoke（@tauri-apps/api/core，进程边界） | fixture 化反查/转录响应；SessionTranscriptPanel 与既有分节组件真实组合（信封 fixture 沿既有装置） | 转录联动 describe |

### packages/desktop/src/views/changes/change-detail-view.tsx -> packages/desktop/src/views/changes/change-detail-view.test.tsx

#### 待测功能

<!-- design.md 公共函数/API 表未声明该文件的导出函数；本节锚定「修改文件」表行：调 useChangeFlowRun、头部区挂 RunControlPanel、run 步节点并入 buildFlowGraph、抽屉转录 props 下传、run 终态触发一次显式 refresh。 -->

（design.md 未声明该文件的公共 API 变更——页面组装行为见下表。）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 控制面板组装 | 正向 | 页面头部区 RunControlPanel 在场（发起/停止入口随 run 态可用——AC-5 控制入口组装半边） | 新增 |
| run overlay 并入 | 正向 | 运行中：runStep 节点出现在图（overlay 输入接通——实时执行视图） | 新增 |
| 终态显式 refresh | 边界 | run 终态：恰一次显式 refresh 触发（图回落派生规则，desktop-change-flow-view 回归不变） | 新增 |
| 转录 props 下传 | 边界 | 抽屉选中节点：liveEvents/sessionRefs 下传链路接通 | 新增 |
| 既有页面组装回归 | 边界 | Header + 流程图区 + workflow 独立面板 + 产物区既有组装零回归（新增为加法） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| invoke/Channel（@tauri-apps/api/core，进程边界） | invoke 按命令名分发 fixture + Channel 可编程 class 投递 RunUpdate 信封——useChangeDetail/useChangeFlowRun 均真实组合（内部协作 hook 不 mock，hook 本体契约零改动），fixture 经 mock IPC 流入，页面组装断言观察渲染输出；ResizeObserver/getBBox 垫片沿既有装置 | 全部 describe |

### plugins/dev-team/bin/src/cli.ts -> plugins/dev-team/bin/src/cli.test.ts

#### 待测功能

<!-- design.md 公共函数/API 表未声明该文件的导出函数；本节锚定「修改文件」表行：回退 walker-* 注册（恢复 2.10.44 已发布形态）——AC-10 可回归半边。MCP 注册面与相位表逻辑零改动（冻结承诺）。 -->

（design.md 未声明该文件的公共 API 变更——回退后注册面回归断言见下表。）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 六 walker-* 命令注册 | 废弃 | 旧断言「cac 注册面含 walker-phase-next/phase-start/phase-log/backtrack/change-files/static-check 六命令」随插件零新增回退废弃（差异表 #2；AC-10）——回退后 cli.test.ts 恢复 2.10.44 断言集，walker-* 注册断言不再存在 | 废弃 |
| walker-* 路径 JSON stdout 与退出码透传 | 废弃 | 旧断言随 walker-io 信封层回退废弃（walker-* 七文件删除清单） | 废弃 |
| MCP 注册面与既有命令断言回归 | 边界 | 回退后既有注册断言（MCP 工具注册面、顶层命令）全绿、零新增条目——净效果与 2.10.44 基线一致（AC-10 回归半边；MCP/hooks 冻结承诺锚） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| process.stdout/stderr 与 process.exit（进程全局边界） | 输出捕获 + exit 替身（沿既有 cli.test.ts 装置） | 既有注册断言 describe |

---

## 不可测试项

- AC-8 管线合规（client:check / 前端与 rust 套件全绿、knip 无新增豁免条目、desktop 0.4.0 且 `src-tauri/Cargo.toml` 不随动、dev-team 零 bump 保持 2.10.44） — **原因**: 静态守线与版本/清单核查，无进程内可断言行为；套件的执行验证由 test-execution 阶段承接（design「验收标准对齐」同款口径）。
- AC-6 静态半边与 AC-10 产物半边：walker 全程零 CLI 子进程写 workflow.json 的代码路径、`crates/infra/devteam` 不存在、orchestration → workflow 依赖方向成立、static-check 与 git diff 为仅有的进程 spawn 且均落 infra 层（spawn 不进 core）、三类 dist 交付产物 rebuild 后零 walker-* 痕迹 — **原因**: 静态结构约束与构建产物核查而非进程内可断言行为：由分层依赖（orchestration core 零进程 spawn / 零 fs 写面、写面唯一经 `workflow::write` 进程内直调）与源码文本 / 产物扫描在评审阶段核查；动态半边（进程内直调证据、工具步 Err 显式失败）由 steps_test / walker_test 承载。
- AC-10 文件与版本回退半边：`walker-*` 七文件删除、`cli.ts`/`cli.test.ts` 注册回退、`plugins/dev-team/package.json` 2.10.45 → 2.10.44、`packages/desktop/package.json` 0.3.15 → 0.4.0（已就位） — **原因**: 仓库状态与配置值核查，无运行时行为；回退后的可回归面（既有注册断言集全绿、walker-* 断言不存在）由 cli.test.ts 章节承载。
- 声明 / 清单 / 生成物组：`crates/core/workflow/src/lib.rs`、`write/mod.rs`、`crates/core/orchestration/src/lib.rs`、`crates/infra/agent/src/lib.rs`（crate 根与模块声明 / 再导出面）、`flow/types.ts`（纯类型模块）、`write/persist.rs`（crate 内私有持久层，无 crate 外导出面）、三个 Cargo.toml、Cargo.lock、`src/commands/mod.rs` 登记、main.rs 托管、`bindings/mod.rs` 零手改、`types/generated/bindings.ts` 再生成 — **原因**: 纯导出 / 类型 / 清单 / 生成物无可执行行为，不建对应测试文件（防空套件红灯）；可达性由各 `*_test` 的 crate 根 use 导入与前端消费方用例编译期承载；persist.rs 的 W2 保形 / pretty 写回 / W3 zod 兼容 fixture 断言由写面四写操作测试承载（同 crate 内私有可达，非不可测、系无独立测试文件）；`all_commands!` 登记由编译（specta builder）与 bindings 零 diff 自检静态核查；main.rs 托管行为需完整 Tauri 运行时启动，注册表语义经 control_test 锁定。
- AC-3 静态半边：既有 skill 路径 file_log 数据的流程图挂载与图外面板显示不动 — **原因**: 零改动承诺无新增可断言行为；由既有 attachments / file-log-table 既有场景套件回归承载（AC-8 套件全绿口径），本期无新增用例。
- 真实端到端（真实 claude CLI 引擎 spawn 的 WorkerAgent 会话全程、真实 run 全链驱动整条 requirement 工作流、桌面重启后重新发起 run 自 active_phase 续走的端到端体验、ask/确认卡片的最终视觉呈现） — **原因**: 进程 / 环境 / UI 边界不可进进程内单测；行为面以假引擎 + 假写面双缝（walker_test）、真实写面组合（walker_test / steps_test）、mock IPC 装置（前端 hook / 命令测试）锁定；续走语义的进程内半边由 walker_test「重入自 active_phase 续走」与 phase_next_test「mid-phase interruption 承接」承载，端到端呈现属验收阶段人工核查。



