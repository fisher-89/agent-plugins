# 设计: desktop-change-flow-auto-next-phase

> **变更**: desktop-change-flow-auto-next-phase
> **日期**: 2026-10-03

---

## 提案与规格同步状态

`proposal.md` 与 `specs/desktop-change-orchestration/spec.md`、`specs/desktop-change-flow-view/spec.md`（路径相对域根）已由提案阶段写入，属已完成产物，不在本变更清单与任务列表内。测试文件面（`walker_test.rs` 既有 `RunRequest` 构造点补 `auto_confirm: false` + auto 直通用例、`change_flow/mod_test.rs` 透传用例、`run-control-panel.test.tsx` 开关断言）由 test-design / test-gen / test-execution 阶段承接，不在本清单。

## 关键设计决策（design 定稿点）

| # | 问题 | 定稿 | 理由 |
|---|------|------|------|
| D1 | drive() 确认点分支形态 | `if !request.auto_confirm { ... }` **整体包住** `guard.emit(RunUpdate::ConfirmWait)` + `guard.wait_confirm().await` + 停等未获确认的 `Terminal::stopped` 收口——emit 与 wait 原子同支，不存在「只跳过 wait 仍广播事件」的中间形态；分支内语句序列与既有代码零位移（false 分支即原语句块原样缩进） | auto 模式要求零 `ConfirmWait` 信封且不进 `WaitingConfirm`（AC-1），拆开发射与挂起会产生有事件无停等的漂移态；语句块整体包裹使手动模式（AC-2）语义逐字不变，回归面最小 |
| D2 | walker 注释口径随行 | `RunRequest.auto_confirm` 字段 doc 钉死双模语义；模块 doc「停等节奏」段由单一句式改写为「run 级发起参数（发起时定格）：手动模式（false）phase 内自动、phase 间停等确认；auto 模式（true）跳过 phase 间停等直通终态，ask / 失败 / 终态仍停」；确认点行内注释同步补 auto 分支说明；新注释措辞零路径引用，不含 layout 命名隔离扫描的双禁令字面量（磁盘域根目录名与配置文件名） | 头注释「拍板停等节奏」是 run 级属性的活文档，节奏语义扩展必须随行；措辞钉死防止实现期顺手扩写；双禁令字面量由 foundation layout 扫描机械拦截，落笔即规避 |
| D3 | 命令面透传形态 | `change_flow_start`（tauri command + specta）与 `change_flow_start_with`（pub(crate) 泛型测试缝）同步加尾参 `auto_confirm: bool`，组合根 `RunRequest` 构造点透传；blank 守卫 / 前置校验 / 组合根装配 / 订阅先行 / spawn 时序零改动；bool 入参无格式检查面（`is_blank` 口径不适用） | 命令签名变化经 specta 编译期出线、tsc 拦截前端漂移（proposal 风险表缓解措施）；测试缝与生产入口同签名是本文件既有纪律（`agent_start_with` 先例），拆开会产生双口径 |
| D4 | 前端 hook 签名 | `UseChangeFlowRunResult.start` 改为 `start: (autoConfirm: boolean) => Promise<void>`——**显式必填，不设缺省默认**；`useCallback` 依赖数组不变（autoConfirm 为入参非闭包捕获）；`invoke` 调用点改 `commands.changeFlowStart(channel, root, change, autoConfirm)` | 单一生产调用点（`RunActions` 发起按钮），必填使发起模式在调用点显式可读；缺省默认会让「手动 / 自动」在类型面不可见，静默回退手动档与「发起时定格」语义相性差 |
| D5 | 面板开关形态与轻提示 | 发起区（`RunActions`）内 label + 原生 `<input type="checkbox">`（`data-testid="run-auto-next-phase"`）置于发起 / 停止按钮左侧同行；组件本地 `useState(false)` 默认关；**运行期禁用呈现（`disabled={active}`），非隐藏**——定格值保持可见，终局后恢复可编辑并保留上次取值；开关下常驻一行 muted 说明文案（不随开关状态条件渲染）：「自动确认：仅跳过相位间停等；ask 中断、失败与终态收口仍停下等待，不自动归档。」 | 无 ui 层 Switch / Checkbox 既有件，原生 input 沿 `AskCard` 自由应答 input 先例（不新造组件库件）；禁用优于隐藏——开关是发起模式的可见载体，隐藏会抹掉「本 run 以何节奏发起」的信息且测试锚点不稳；文案常驻满足「轻提示」可发现性且无状态耦合，明示 ask / 失败 / 终态三边界（proposal 风险表缓解） |
| D6 | bindings 重导纪律 | `pnpm -C packages/desktop run bindings:export` 重导后，`bindings.ts` 预期 diff **仅 `changeFlowStart` 一行**：签名尾部增 `autoConfirm: boolean`、invoke 载荷增 `autoConfirm`；其余命令条目与 DTO 零变化；重导幂等（再跑零 diff） | 命令名是 invoke 键、类型面是漂移拦截器，最小 diff 使 review 与回归面收敛到单点；幂等性是生成物纪律的既有验收口径 |
| D7 | 版本 | `packages/desktop/package.json` version 0.4.2 → 0.4.3 **随归档提交执行**（归档轨道，不在实现任务列表；`tauri.conf.json` 经 `../package.json` 跟随，`Cargo.toml` 不随动） | 用户可见变更（新发起模式）才提升（AC-6 / proposal 明示「不在实现任务列表」） |

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 相位循环 walker（停等节奏分支生效层） | `RunRequest` 承载 `auto_confirm` 发起定格；drive() 确认点双模分支（D1）；模块 doc 停等节奏口径补 auto 模式（D2）；ask 路径与终态收口零触达（AC-5 语义边界的结构保障） | `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` | `crate::control`（RunGuard / ChangeFlowControl）、`crate::state`（RunUpdate / ChangeRunStatus）、`workflow::write` 写面 | Rust（async 相位循环，tokio 挂起面） |
| change_flow 命令面发起入口 | `change_flow_start` / `change_flow_start_with` 尾参 `auto_confirm` 透传组合根 `RunRequest`（D3）；blank 守卫与前置校验、订阅先行、spawn 时序零改动 | `packages/desktop/src-tauri/src/commands/change_flow/mod.rs` | `orchestration::walker`（RunRequest / walk_run）、`orchestration::control`、`tauri` IPC、`specta` 出线、`agent_runtime` 组合根 | Rust（tauri command + specta） |
| specta bindings 出线（生成物重导） | `changeFlowStart` 增 `autoConfirm: boolean` 参数（D6）；由命令签名变化经 `bindings:export` 重导，非手写 | `packages/desktop/src/types/generated/bindings.ts` | `export-bindings` crate（specta builder，经 `all_commands!` 宏同源） | TypeScript（生成物） |
| run 控制 hook | `start(autoConfirm)` 必填传参（D4）；六命令 invoke 面与 Channel 订阅生命周期、重挂恢复零改动 | `packages/desktop/src/views/changes/hooks/use-change-flow-run.ts` | `@tauri-apps/api/core`（Channel）、`bindings`（commands / RunUpdate）、`flow/run-state.ts` 纯 reducer | React hook（TypeScript） |
| 运行控制面板发起区 | 「自动确认」开关（默认关、运行期禁用定格、常驻轻提示文案）+ `run.start(autoConfirm)` 调用（D5）；确认卡片 / ask 卡片 / 终态记因呈现零改动（auto run 天然不出确认卡片——无 `ConfirmWait` 事件） | `packages/desktop/src/views/changes/flow/run-control-panel.tsx` | `@/components/ui`（Button / Badge）、`UseChangeFlowRunResult` | React 组件（TSX + tailwind） |

---

## 变更清单

<!-- 以文件为入口逐层展开，实现阶段以此清单为边界。测试文件归 test-design / test-gen / test-execution 阶段，不在本清单。 -->

<!-- 新增文件：无 —— 全部触点为既有文件增量修改，省略此子节。 -->

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` | `RunRequest` 增 `pub auto_confirm: bool` 字段（doc 钉死双模语义）；drive() 确认点 `if !request.auto_confirm` 整体包住 emit + wait + stopped 收口（D1）；模块 doc「停等节奏」段改写为 run 级发起参数双模口径 + 确认点行内注释补 auto 分支说明（D2） | 唯一生效层；ask 路径（`decision_session`）、终态收口（`next.done` → completed）、static-check 反馈边、重试预算零改动；其余 `RunRequest` 消费点（`run_id` / `root` / `change` 字段读取）不受影响 |
| `packages/desktop/src-tauri/src/commands/change_flow/mod.rs` | `change_flow_start` 与 `change_flow_start_with` 尾参 `auto_confirm: bool`；`RunRequest` 构造点透传（D3） | 命令体其余逻辑（blank 守卫 / 两道前置校验 / `begin_run` / 组合根装配 / 订阅先行 / spawn）零改动；`all_commands!` 登记条目零改动（命令名不变） |
| `packages/desktop/src/types/generated/bindings.ts` | 生成物重导（`pnpm -C packages/desktop run bindings:export`）：仅 `changeFlowStart` 行增 `autoConfirm: boolean`（D6） | 非手写文件；重导幂等 + 其余条目零 diff 是守线验收面 |
| `packages/desktop/src/views/changes/hooks/use-change-flow-run.ts` | `UseChangeFlowRunResult.start` 签名改 `(autoConfirm: boolean) => Promise<void>`；`start` 实现体 invoke 调用补第三实参（D4） | `stop` / `confirm` / `answer` / 订阅生命周期 / 重挂恢复零改动；`run-state.ts` 零触达（无新事件词汇） |
| `packages/desktop/src/views/changes/flow/run-control-panel.tsx` | `RunActions` 增「自动确认」开关（label + 原生 checkbox，`data-testid="run-auto-next-phase"`，默认关，`disabled={active}` 运行期定格）+ 常驻一行 muted 轻提示文案 + 发起调用改 `run.start(autoConfirm)`（D5） | `ConfirmCard` / `AskCard` / 终态记因 / 状态徽章零改动；不引入 ui 新组件件 |
| `packages/desktop/package.json` | version 0.4.2 → 0.4.3 | **归档轨道**（AC-6 / D7），随归档提交执行，不在实现任务列表 |

### 公共函数 / API

Rust 签名为 Rust 语法；TS 签名为 TS 语法。

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `change_flow_start` | `src-tauri/src/commands/change_flow/mod.rs` | 修改 | `pub async fn change_flow_start(app: AppHandle, on_event: Channel<RunUpdate>, root: String, change: String, auto_confirm: bool) -> Result<ChangeRunSummary, String>` | tauri command + specta；尾参透传测试缝 |
| `change_flow_start_with` | 同上 | 修改 | `pub(crate) async fn change_flow_start_with<R: tauri::Runtime>(app: AppHandle<R>, on_event: Channel<RunUpdate>, root: String, change: String, auto_confirm: bool) -> Result<ChangeRunSummary, String>` | 泛型测试缝（生产注入 Wry / 测试注入 MockRuntime），与生产入口同签名 |
| `start`（`UseChangeFlowRunResult.start`） | `packages/desktop/src/views/changes/hooks/use-change-flow-run.ts` | 修改 | `start: (autoConfirm: boolean) => Promise<void>` | 显式必填（D4）；invoke `changeFlowStart(channel, root, change, autoConfirm)` |

<!-- `walk_run` / `new_run_id` / `RunControlPanel` / `change_flow_stop` / `change_flow_confirm` / `change_flow_answer` / `change_flow_state` / `change_flow_watch` 签名零变化不列；`RunActions` 为面板模块内部组件（非导出）不列。 -->

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `RunRequest` | `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` | 修改 | 增 `pub auto_confirm: bool` 字段：停等节奏发起定格——false 为手动停等（既有语义：phase 间挂起至 confirm 应答）；true 跳过 phase 间停等（不进 `WaitingConfirm`、零 `ConfirmWait` 事件），ask / 失败 / 终态语义不变 |
| `UseChangeFlowRunResult` | `packages/desktop/src/views/changes/hooks/use-change-flow-run.ts` | 修改 | `start` 成员签名变更（见公共函数表）；其余成员（state / stop / confirm / answer / error）零变化 |

<!-- bindings 侧 `commands.changeFlowStart` 类型为生成物，随 D6 重导体现，不单列。 -->

<!-- 配置：无 —— 零配置键触点；`packages/desktop/package.json` version 归档轨道 bump（AC-6 / D7），见修改文件表标注，非配置键新增/修改语义。 -->

---

## 数据模型

本变更零 schema 变更、零新模型、零持久化触点——`auto_confirm` 是 run 内存生命周期内的发起定格参数（注册表条目随 run 存续），不落盘、不进 DTO：

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `RunRequest`（修改，内存入参结构） | `root: String`、`change: String`、`run_id: String`、`auto_confirm: bool`（新增） | 命令面组合根铸造 → `walk_run` / `drive()` 消费；确认点读 `auto_confirm` 定分叉 | 不持久化（run 级内存参数；`ChangeFlowControl` 注册表条目随 run 收口释放） |
| `ChangeRunSummary`（既有 IPC DTO，零变更） | `run_id: String`、`status: ChangeRunStatus` | `change_flow_start` 发起应答；auto 模式同以 `Running` 起步 | 不持久化（IPC 瞬时面） |
| `RunUpdate` / `ChangeRunSnapshot`（既有，零变更） | 既有信封词汇（Step / SessionEvent / Ask / ConfirmWait / 状态机六档） | auto 模式零新事件、不经 `waitingConfirm`，快照面天然兼容重挂 | 不持久化（广播 + 快照镜像） |

---

## 路由/API 设计

本变更不涉及 HTTP API；IPC 命令面（Tauri invoke 契约）单点扩展：

| IPC 命令 | 输入 | 输出 | 变化 |
|----------|------|------|------|
| `change_flow_start` | `on_event: Channel<RunUpdate>, root: string, change: string, autoConfirm: boolean` | `ChangeRunSummary` | 签名 + `autoConfirm` 尾参（specta → bindings 重导，D6）；语义：true 时 run 全程不停等相位间确认 |
| `change_flow_confirm` / `change_flow_answer` / `change_flow_stop` / `change_flow_state` / `change_flow_watch` | 各自既有入参 | 各自既有出参 | 零变化（ask 应答 / 停止 / 快照 / 重挂补订在 auto 模式下语义不变——ask 仍需应答、停止仍受控） |

---

## 验收标准对齐

| AC | 设计落点 |
|----|----------|
| AC-1 | 阶段一 D1 分支（`if !request.auto_confirm` 包住 emit + wait）+ D2 字段与注释；auto 多相位 run 零 `ConfirmWait` 信封、不经 `waitingConfirm` 直通 completed 的 walker 直测归 test-design / test-gen / test-execution 阶段承接 |
| AC-2 | D1 分支包裹形态：false 分支即原语句块零位移，手动停等语义（`ConfirmWait` 流出、挂起至 confirm、proceed=false / 等待期取消 → 受控 stopped）逐字保持；既有用例回归归测试轨道 |
| AC-3 | 阶段二 D3 命令双入口尾参透传 + D6 bindings 重导（`changeFlowStart` + `autoConfirm`）；auto 参数达 walker 的 mod_test 证明归测试轨道 |
| AC-4 | 阶段三 D4 hook 必填签名 + D5 面板开关（默认关、`disabled={active}` 运行期定格、`data-testid="run-auto-next-phase"`、start 携 `autoConfirm=true`）；断言面归测试轨道 |
| AC-5 | 零代码改动面即保障：ask 路径（`decision_session` ask 分叉 + `change_flow_answer`）与终态收口（`next.done` → completed 停给用户、不触发归档）本变更零触达；auto run 无 `ConfirmWait` 事件故确认卡片天然不呈现；D5 文案明示三边界 |
| AC-6 | D7：version 0.4.2 → 0.4.3 随归档提交执行（归档轨道，修改文件表已标注，不在实现任务列表） |
| AC-7 | 阶段四守线静态面（`server:check` / `client:check` + knip 零新增豁免 / bindings 重导幂等仅单行 diff）；自动化套件的执行验证由 test-execution 阶段承接（AC-7 执行面），不入本设计任务面 |

---

## 依赖

### 运行时依赖

- 无新增 — `tauri`（IPC / Channel）、`specta`（出线）、`tokio`（挂起面）、`@tauri-apps/api`（invoke / Channel）、React 均为既有依赖，`auto_confirm` 为纯语言内 bool 传参

### 构建/测试依赖

- 无新增 — 沿 `pnpm -C packages/desktop run server:check`（cargo fmt + clippy）、`client:check`（vp check + knip）、`bindings:export` / `bindings:check` 既有工具链；cargo workspace 套件注册（src-tauri 根）零触达

---

## 待决问题

- 无 — proposal 两项预告归 design 定夺的问题已定稿：轻提示措辞与呈现形态（D5：常驻一行 muted 文案）与开关运行期形态（D5：禁用呈现、非隐藏）；停等节奏分支形态（D1）、hook 签名缺省与否（D4）一并钉死。
- 留痕（非待决）：auto 模式下「手动改自动」的唯一路径是停止重发自 `active_phase` 续走（proposal 决策表既有口径），本设计未新增任何运行中切换控制面；开关终局后保留上次取值属 UI 便利语义，不影响 run 级定格（每次发起以点击时刻开关值为准）。
