# 任务: session-midturn-history-overwrite

> 覆盖 `proposal.md`「变更范围 - 实现文件」全部 11 项（0 新增文件 + 11 修改面，含 1 个版本行）+ 设计「范围补录」1 行（`crates/core/agent/src/session.rs` 的 `TurnSummary.status` doc，见 design 校准 3）+ 生成物再生（`bindings.ts` / `bindings.zod.ts`，非手改）。
>
> **本相位不写测试用例、不改断言、不跑测试回归**：proposal「测试文件」表逐行的用例归 test-design / test-gen / test-execution 相位；`pnpm -C packages/desktop run check` 整链与 `vp test` / `cargo nextest` 由桌面端 static-check / test-execution 相位执行。本相位对测试文件的唯一动作是**登记交接**（见阶段六），MUST NOT 新增用例、MUST NOT 改断言口径、MUST NOT 加 `#[cfg(test)] mod` 声明（本次全部测试面都是既有 sidecar 文件，零新文件）。
>
> **依赖序（六阶段）**：
>
> - **阶段一**（值域 + flow 分类臂 + 两处出线 doc）**零行为变化**（`Interrupted` 生产路径不可达；`match` 因补臂才通过编译），是其后一切的前置：生成物与前端 TS union 都由此派生。
> - **阶段二**（store 标定 + 打开点接线）依赖阶段一（`AgentRunStatus::Interrupted` 在场才可写入）；本阶段末「库态在重启后不再留 `running` 残留」闭环。
> - **阶段三**（生成物再生）依赖阶段一（枚举值域）与阶段二（无关系，但放此以免前端阶段读到旧的 union）；本阶段末 `src/types/generated/**` 含 `"interrupted"`。
> - **阶段四**（前端播种键，症状 1）依赖阶段三（TS 侧可比较 `interrupted`）；**不依赖**阶段五 / 六。
> - **阶段五**（前端透镜三投影，症状 2 / 3 + 状态行）依赖阶段三；与阶段四无代码交叉（不同文件、不同机制）。
> - **阶段六**（列表文案 + 版本 + 零 diff 自查 + 交接）依赖全部。
>
> **关键约束（贯穿全部任务）**：
> ⓪ **`interrupted` 只有一个写入点**：`calibrate_interrupted_agent_turns`。`kernel.rs` 的 `AgentRunState → AgentRunStatus` 映射、`finish_agent_turn`、陈旧轮清理等运行期路径 MUST NOT 出现 `Interrupted`（`grep -rn "AgentRunStatus::Interrupted"` 在生产代码里只应命中标定实现与 `worker_settle` 的 match 臂）。
> ① **落库收敛面零改动**：`#[native_model(id = 2, version = 4)]` / `From<AgentRunRecordV3>` 两向 / `AgentRunRecord` 字段面 MUST NOT 改（改则破 AC-2 的旧行零迁移）。
> ② **变体末位追加**：`Interrupted` MUST 在 `Stopped` 之后（插在中间会位移既有 `Completed` / `Failed` / `Stopped` 的落库变体索引，全库历史行解码错位）。
> ③ **`match` 保持穷尽、无 `_` 兜底臂**：`worker_settle` 的五臂 MUST 全部显式变体臂（`_` 会吃掉未来新增变体，正是本变更要收掉的静默面）。
> ④ **播种只投一次**：`seedIfMatching` 在首个 `event` 信封即消费登记（无论轮次是否匹配），MUST NOT 留「等下一次匹配」的分支（残留轮事件会让播种无限推迟）。
> ⑤ **选中轮的唯一依据是首个实时事件的 `turnId`**：`detail.turns` 只允许出现在状态门一处（`some(t => t.status === 'running')`），MUST NOT 参与选轮 / 播种。
> ⑥ **轮状态下行只穿原语**：`turnRunning` 是布尔、`ChatBubble` 的 props 面不变；MUST NOT 把 `record` / `status` 对象新增为 `ChatBubble` / `ChatBubbleBase` 的 prop（会击穿 `memo` 与 `renderItem` 的 `useCallback`）。
> ⑦ **缺省即运行中**：`turnRunning === undefined` 一律按运行中处理（流式消息在 `Record` 信封前没有 `metadata.record`，取反向缺省会打死调试页 / explore 的活卡主路径——design 校准 1）。
> ⑧ **生成物勿手改**：`src/types/generated/**` 只经 `bindings:export` 产生。
> ⑨ **门禁分工**：整链 `check` 与测试回归由桌面端相位执行；本相位自查只收源码级静态判据（`cargo clippy` / `grep` 零命中 / 读码复核 / `git diff` 边界比对 / 版本行）。

## 阶段一：值域 + flow 分类臂（`Interrupted` 成为既有枚举的正规变体）

> 本阶段零行为变化：新变体在生产路径不可达（进程中断时 walker 一并死亡，标定发生在下次启动），`worker_settle` 的新臂只为让穷尽 `match` 继续通过编译。既有四变体臂的断言口径与文案**逐字不动**。

- [x] `crates/core/agent/src/runner.rs`：`AgentRunStatus` 在 `Stopped` 之后追加 `Interrupted`（**末位**，约束 ②），doc 三句——①语义：轮因桌面进程中断而客观终止，与 change run 域 `RunStatus::Interrupted` 同义；②唯一写入点：启动标定 `calibrate_interrupted_agent_turns`（运行期收口 / 陈旧轮清理 MUST NOT 产生）；③落库兼容：末位追加以保 bincode 变体索引不变（旧行解码值不变、零版本递增、零迁移）。同文件枚举 doc「run 状态四档」→「五档」
- [x] `crates/core/agent/src/session.rs`：`TurnSummary.status` 的 doc 取值清单补 `interrupted`（**补录，纯注释**；该 doc 经 specta 生成到 `bindings.ts` 的 `TurnSummary.status` 注释，见 design 校准 3）。`git diff` 应只有一行注释变化
- [x] `crates/core/orchestration/src/walker.rs`：
  - `worker_settle` 增 `AgentRunStatus::Interrupted` 臂，**位置紧邻 `Stopped` 臂之后**（`Running` 漂移臂之前，便于与 stopped 侧对拍）：`let reason = "会话被中断".to_owned(); (StepSettle::failed(Some(reason.clone())), Some(Terminal::failed(reason)))`；臂内注释两句——「归 failed 侧：该步没有可信产出、且不是用户主动停止」+「生产路径不可达：进程中断时 walker 一并死亡，标定发生在下次启动；本臂是枚举扩值经编译器强制的显式裁决面」
  - `worker_settle` doc 的变体枚举（`Completed` → passed 无终态；`Failed` / `Stopped` / `Running` 漂移 → …）补 `Interrupted` 一词与「failed『会话被中断』」归属
  - `worker_settle_probe` doc「四臂逐文案」→「五臂逐文案」
  - **MUST NOT** 新增 `_` 兜底臂；**MUST NOT** 动 `Terminal` / `StepSettle` / `RunFault` / `ChangeStepStatus` / `ChangeRunStatus` 的变体面；**MUST NOT** 动相位机其余函数（`enter_phase` / `settle_phase` / `drive` / `run_step` / `run_worker`）
- [x] 自查 阶段一（静态判据）：
  - `grep -n "pub enum AgentRunStatus" -A 14 crates/core/agent/src/runner.rs` 读码确认 `Interrupted` 是**最后一个**变体（`Stopped` 之后、枚举闭合花括号之前）
  - `grep -rn "AgentRunStatus::Interrupted" crates src --include=*.rs | grep -v _test.rs` 只命中 `walker.rs` 的 match 臂（阶段二再加标定实现，届时共 2 处）
  - `grep -n "fn worker_settle" -A 40 crates/core/orchestration/src/walker.rs` 五臂全显式、零 `_`；`git diff` 的 hunk 只落在 `worker_settle` + 两处 doc
  - `git diff -- crates/core/agent/src/kernel.rs crates/core/orchestration/src/port.rs crates/core/orchestration/src/state.rs` 为空（约束 ⓪ / 范围外零触碰）
  - `cargo clippy -p agent -p orchestration --all-targets` 通过（`match` 补齐后不留 non-exhaustive 报错）

## 阶段二：store 启动标定与打开点接线（库态不再留死轮）

> 依赖阶段一。形态逐段抄既有 `calibrate_interrupted_runs`（读扫 → 判空 → 写事务 → commit → 返回清单），零新机制。既有 `calibrate_interrupted_runs` 函数体**逐字不动**（只在旁新增并列函数 + 打开点多一行调用）。

- [x] `crates/infra/store/src/store.rs`：`impl Store` 新增 `pub fn calibrate_interrupted_agent_turns(&self, now: i64) -> Result<Vec<i64>, StoreError>`（置于 `calibrate_interrupted_runs` 之后）：
  - 读扫：`self.read_all::<AgentRunRecord>("遍历轮统计行")?.into_iter().filter(|record| record.status == agent::AgentRunStatus::Running).collect::<Vec<_>>()`；空 → `Ok(Vec::new())`（**不开写事务**，幂等 + 零写入）
  - 写：单 `rw_transaction` 内逐行 `status = agent::AgentRunStatus::Interrupted`、`finished_at = Some(now)`、`error = Some("重启标定：桌面进程中断，会话轮客观已终止".to_owned())`；`num_turns` / `cost_usd` / `duration_ms` / `session_id` / `started_at` **原样带过**（不虚构、不重写轮起点）；`upsert` + `commit`
  - 返回：被标定 `id` 清单（`Vec<i64>`）
  - doc 五句——判据（启动瞬间三张内存注册表必空 ⇒ 残留 `running` 在数学上全部可证已死，MUST NOT 依赖「卡在哪个工具」）／扫描面（只 `AgentRunRecord.status == Running`；不碰 `RunRecord`、不碰 `SessionRecord` 与 sealed 转录）／写入（状态 + `finished_at` + 记因定式，统计三字段不虚构）／幂等（零残留零写事务、二次调用零变化、已 `interrupted` 不被二次改写）／唯一写入点（运行期写路径 MUST NOT 产生该值）
- [x] `crates/infra/store/src/store.rs`：`open_workspace` 改为 `let now = now_millis();` + `let _ = store.calibrate_interrupted_runs(now);` + `let _ = store.calibrate_interrupted_agent_turns(now);`（**两族共享一次取时**；吞错形态 `let _ =` 沿用现状）；函数 doc 一句「打开点并列两族启动标定：change run（含 `active_phase` 悬挂）与 agent 轮 `running` 残留」
- [x] `crates/infra/store/src/model.rs`：`AgentRunRecord.status` 的 doc 取值清单补 `interrupted`（当前为 `running | completed | failed | stopped`）。**MUST NOT** 动 `#[native_model(id = 2, version = 4)]`、两向 `From`、字段面；`AgentRunRecordV3.status`（历史形态）的 doc 不改
- [x] 自查 阶段二（静态判据）：
  - `grep -n "fn calibrate_interrupted_agent_turns" -A 6 crates/infra/store/src/store.rs` 读码确认：读扫在写事务之前、空则早返回（两段形态）
  - `grep -n "rw_transaction" crates/infra/store/src/store.rs` 在 `calibrate_interrupted_agent_turns` 函数体内恰 1 处
  - `grep -n "fn open_workspace\b" -A 8 crates/infra/store/src/store.rs` 读码确认两族标定并列且共享 `now`
  - `git diff -- crates/infra/store/src/store.rs` 读 diff：`calibrate_interrupted_runs` 函数体零 hunk；`open_workspace_probed` 零 hunk
  - `git diff -- crates/infra/store/src/model.rs` 只有 1 行注释变化（版本头 / 字段 / `From` 零 hunk）
  - `grep -rn "AgentRunStatus::Interrupted" crates src --include=*.rs | grep -v _test.rs` 共 **2** 处（`walker.rs` 臂 + 本函数）
  - `cargo clippy -p store --all-targets` 通过

## 阶段三：生成物再生（`"interrupted"` 进 TS union 与 zod literal）

> 依赖阶段一。生成物**入库、勿手改**；再跑一次必须零新增 diff（幂等）。

- [x] 执行 `pnpm -C packages/desktop run bindings:export`（生成 `src/types/generated/bindings.ts` 与 `src/types/generated/bindings.zod.ts`）
- [x] 自查 阶段三（静态判据）：
  - `grep -n "AgentRunStatus" -A 2 packages/desktop/src/types/generated/bindings.ts` 的 union 含 `"interrupted"`（末位）
  - `grep -c '"interrupted"' packages/desktop/src/types/generated/bindings.zod.ts` ≥ 1（zod literal 随动）
  - `grep -n "轮状态" packages/desktop/src/types/generated/bindings.ts` 的 `TurnSummary.status` 注释含 `interrupted`（阶段一 doc 生效的证据）
  - `git diff -- packages/desktop/src/types/generated` 只含上述两文件的值域 / 注释变化（无命令签名条目改动）
  - 再跑一次 `bindings:export`，`git diff` 无新增行（幂等）
  - `bindings:export` 已内置 `bindings:zod-check`（`tsc --noEmit -p tsconfig.zod.json`）并通过（生成物与 zod schema 自洽；整链门仍归桌面端 static-check）

## 阶段四：前端播种键改由首个实时事件自证（症状 1）

> 依赖阶段三。改动只在 `reconnectToStream` 与 `createPendingSeed` 两处；既有状态门、`sinceSeq` 计算、`envelopeStream` 的 chunk 投递顺序**逐字不动**。

- [x] `packages/desktop/src/lib/agent-transport.ts`：
  - `EnvelopeStream.expectSeed` 类型改 `expectSeed: (events: readonly AgentEvent[]) => void`（`turnId` 入参退役），doc 改写为「登记已装载镜像（非 delta、游标以内）；轮次由首个实时事件自证」
  - `PendingSeed` 改 `{ expect: (events: readonly AgentEvent[]) => void; seedIfMatching: (message: AgentRunMessage) => void }`，接口 doc 说明为何改派（`detail.turns` 的 `running` 行可能是进程中断残留、也可能与实时轮并发，按行选轮一旦选错就冲掉该轮已落库部件）
  - `createPendingSeed`：`pending` 改为 `readonly AgentEvent[] | null`；`expect` 直接登记；`seedIfMatching` —— 非 `event` 信封或未登记即返回；**取走登记先清空**（首个事件即消费，约束 ④）；`message.event.turnId` 为 `null` / `undefined` 时直接返回（无权威轮次不投）；否则按 `seeded.turnId === turnId` 过滤后逐条 `frameChunks(state, seeded)` 投递
  - `reconnectToStream`：删除 `const running = detail.turns.find(...)` 与随后的 `if (running === undefined) return null;` 两处；`expectSeed` 改传 `(this.observedEvents?.() ?? []).filter((event) => event.kind !== 'messageDelta' && event.seq <= sinceSeq)`；函数 doc 的播种段改写为「播种键 = 首个实时事件的 `turnId`；`detail.turns` 只作状态门，MUST NOT 用作选轮依据」
- [x] 自查 阶段四（静态判据）：
  - `grep -n "detail.turns" packages/desktop/src/lib/agent-transport.ts` 只剩状态门那一处（`some(...)`），零 `find(`（约束 ⑤）
  - `grep -n "expectSeed" packages/desktop/src/lib/agent-transport.ts` 的签名无 `turnId` 入参
  - `grep -n "pending = null\|pending === null" packages/desktop/src/lib/agent-transport.ts` 读码确认「取走即清空」在投递之前
  - `git diff -- packages/desktop/src/lib/agent-transport.ts` 的 hunk 只落在 `EnvelopeStream` doc/类型、`PendingSeed`、`createPendingSeed`、`reconnectToStream` 四处；`StreamState` / `frameChunks` / `recordChunks` / `envelopeOutcome` / `invokeStart` 零 hunk

## 阶段五：对话透镜三投影 + 状态行（症状 2 / 3 + 可见性）

> 依赖阶段三。轮状态只在 `ChatBubbleBase` 算一次（布尔），向下只穿原语；判据单点在 `unpairedRunning` 与 `isLiveAsk` 各一处，MUST NOT 在四张卡里各写一份。

- [x] `packages/desktop/src/components/agent/tool-cards.tsx`：
  - `AgentToolCardProps` 增 `turnRunning?: boolean`（doc：该轮是否仍在运行；缺席按运行中处理——无轮行的历史转录与流式未挂 `record` 两情形）
  - 新增模块内单点判据 `function unpairedRunning(output: AgentToolOutput | null, turnRunning: boolean | undefined): boolean`（返回 `output === null && turnRunning !== false`，doc 一句：有结果即非运行态；无结果且轮已终结不呈运行态——未配对只说明没有结果落库，不说明正在跑）
  - 四处 `TraceLine status`（AskUserQuestion / Bash / Read / Write）改判据：`output ? (output.isError ? 'error' : undefined) : unpairedRunning(output, turnRunning) ? 'running' : undefined`（终结轮无结果 → **省略 status**，MUST NOT 取 `pending`——`pending` 的既有语义是「思考中」的 streaming）
  - Ask / Bash 的图标判据同步：`unpairedRunning(...)` 为真才 `<Spinner />`，否则该卡特化图标（Ask → `MessageCircleQuestionIcon`、Bash → `TerminalIcon`）；Read / Write 图标与结果无关，零改动
  - 四张卡的解构都补 `turnRunning`（`EditCard` / `finish_step` / `web_search` 三卡零改动；默认对卡 `ToolPairCard` 不接该位——范围边界见 design 校准 2）
- [x] `packages/desktop/src/components/agent/agent-messages.tsx`：
  - `isLiveAsk` 增第三参 `turnRunning: boolean | undefined`，函数体改三态矩阵：`null` → `false`；字符串 → `part.toolCallId === pendingToolCallId`；`undefined` → `part.state === 'input-available' && turnRunning !== false`；doc 改写（`undefined` 态由「结果缺席」改读轮状态，`input-available` 只作必要条件；MUST NOT 以结果缺席单独判活）
  - `ToolPartView` / `PartView` 增可选 `turnRunning?: boolean` 并原样下行（`ToolPartView` → `Card`；`PartView` → `ToolPartView`），`ToolPartView` 内 `isLiveAsk(part, askAnswer.pendingToolCallId, turnRunning)`
  - `ChatBubbleBase` 内算 `const turnRunning = record === null ? true : record.status === 'running';`（`record` 已在场，零新取数），只在 assistant 分支把它传给 `PartView`；**`ChatBubble` 的 props 面与 `memo` 判据零改动**（约束 ⑥）
  - `TurnStatusLine` 在 `stopped` 支之后增 `interrupted` 支：`data-testid="turn-status-line"`、`data-status="interrupted"`、内层 `data-testid="turn-status-line-interrupted"`、文案「因应用重启中断」、`text-muted-foreground`；**MUST NOT** 读 `record.error` / `noticeError`、MUST NOT 复用失败标题与「原始错误」展开面；doc 的「其他 status 不渲染」改为「`completed` / 轮行缺席不渲染」
- [x] `packages/desktop/src/lib/agent-virtual.ts`：`filterConversation` 的空内容轮白名单由 `failed` / `stopped` 扩为 `failed` / `stopped` / `interrupted`；函数 doc 的「判定 `failed` / `stopped` 保留」同步为三值（判据单点不变，虚拟项数与吸附几何自动跟随）
- [x] 自查 阶段五（静态判据）：
  - `grep -c "unpairedRunning" packages/desktop/src/components/agent/tool-cards.tsx` ≥ 5（1 定义 + 4 卡引用；Ask / Bash 各含图标与 status 两处则更多）——读码复核「四处同形状」只此一个判据来源（design R6 的收口手段）
  - `grep -n "TraceLine status={output" packages/desktop/src/components/agent/tool-cards.tsx` 命中 **4** 处且都经 `unpairedRunning`；`grep -n "'pending'" packages/desktop/src/components/agent/tool-cards.tsx` **零命中**（终结轮不取 `pending`）
  - `grep -n "function isLiveAsk" -A 10 packages/desktop/src/components/agent/agent-messages.tsx` 读码确认三态矩阵与 `turnRunning !== false` 缺省（约束 ⑦）
  - `grep -n "const ChatBubble = memo" -B 3 packages/desktop/src/components/agent/agent-messages.tsx` 的 props 面仍只有 `message` / `askAnswer`（约束 ⑥）
  - `grep -n "data-status=\"interrupted\"" packages/desktop/src/components/agent/agent-messages.tsx` 命中 1 处；该支内无 `record.error` / `statusRawText` 引用
  - `grep -n "'interrupted'" packages/desktop/src/lib/agent-virtual.ts` 命中 1 处
  - `git diff -- packages/desktop/src/components/agent/agent-blocks.tsx packages/desktop/src/hooks/use-agent-session.ts packages/desktop/src/components/agent/agent-error.ts` 为空（范围边界 / 零改动面）

## 阶段六：列表文案 + 版本 + 边界自查 + 交接

> 依赖全部。本阶段只做词面映射、版本行与收尾核对。

- [x] `packages/desktop/src/views/agent/components/agent-run-history.tsx`：`statusLabel` 增 `if (status === 'interrupted') return '已中断';`（置于 `stopped` 之后、`return '失败'` 兜底之前）；行色三元分支**零改动**（`interrupted` 自然落 `text-muted-foreground`）；`data-status={status ?? 'empty'}` 直出零改动
- [x] `packages/desktop/package.json`：`version` `0.6.47` → `0.6.48`（`src-tauri/tauri.conf.json` 经 `"version": "../package.json"` 跟随，零手改；`src-tauri/Cargo.toml` 的 crate 版本行 **MUST NOT** 随动）
- [x] 自查 阶段六（静态判据，边界比对）：
  - `grep -n "function statusLabel" -A 8 packages/desktop/src/views/agent/components/agent-run-history.tsx` 五值映射齐、`已中断` 在 `失败` 兜底之前
  - `node -p "require('./packages/desktop/package.json').version"` 为 `0.6.48`；`git diff -- packages/desktop/src-tauri/Cargo.toml packages/desktop/src-tauri/tauri.conf.json` 为空
  - 零 diff 登记面逐条比对（design「零 diff 登记」表）：`kernel.rs` / `ask.rs` / `resume.rs` / `calibrate_interrupted_runs` 函数体 / `port.rs`(`RunFault`) / 六命令签名 / `crates/core/agent/src/event.rs` / `openspec/specs/**`
  - `git diff --stat` 的文件集恰为 **13 个路径**（proposal 11 行清单展开的 12 个文件 + 范围补录 1 个 `session.rs`），**无第三个杂项文件**
- [x] 交接 阶段六（测试面登记，本相位不执行）：以下用例归桌面端 test-design / test-gen / test-execution 相位——**全部为既有 sidecar 追加，零新文件、零 `#[cfg(test)] mod` 声明**：
  - `crates/infra/store/src/store_test.rs`：标定只动 `running` / 记因文案 / 幂等（二次零变化）/ 不动已终态 / 不误伤 `RunRecord` 与转录
  - `crates/infra/store/src/model_test.rs`：`interrupted` 值域往返 + 旧行四值解码不变
  - `crates/core/orchestration/src/walker_test.rs`：`worker_settle` 对 `Interrupted` 的终态映射（步 detail 与 run 终态同文、`match` 无 `_`）；`worker_settle四态分类与成因折叠与边界` 用例名与文件头「四态」字面同步扩五态、既有四段断言逐字不动
  - `src-tauri/src/commands/exec/mod_test.rs`：`session_detail` 返 `interrupted` 轮行；`agent_answer` 对已标定会话 → `Err`
  - `packages/desktop/src/lib/agent-transport.test.ts`：**残留 `running` 行在前 + 直播轮在后的回归**（增量到达后直播轮已落库部件仍在；当前实现必红）+ 跨轮不串扰。注：播种路径现**零测试**（design 校准 6），夹具需自建
  - `packages/desktop/src/hooks/use-agent-session.test.ts`：`interrupted` 轮行不派生 `running`、不触发订阅
  - `packages/desktop/src/lib/agent-virtual.test.ts`：保留空内容 `interrupted` 轮、仍剔除空内容 `completed` 轮
  - `packages/desktop/src/components/agent/agent-messages.test.tsx`：`interrupted` 状态行；无 `pendingToolCallId` 时终结轮 ask 卡静态、`running` 轮照常活
  - `packages/desktop/src/components/agent/tool-cards.test.tsx`：终结轮未配对工具卡不呈 `running`（无 shimmer 标记）；运行中轮不变
  - `packages/desktop/src/views/agent/components/agent-run-history.test.tsx`：终态文案五值映射与 `data-status` 钩子
  - `packages/desktop/src/components/agent/agent-error.test.ts`：最新轮 `interrupted` 时 `latestAgentFailure` 返 `null`（顶部失败横幅零呈现；该文件生产面零 diff，由测试固化默认）
- [x] 交接 阶段六（门禁，本相位不执行）：`pnpm -C packages/desktop run check` 整链（`server:check` → `bindings:export` → `api:check` → `client:check`，含 `vp check` 的 50 行函数限与 `knip`）、`vp test`、`cargo nextest --workspace` 由桌面端 static-check / test-execution 相位执行；`api:check` 零写入比对预期零差异（三目标元素均 `api = none`，本变更不写契约文件）。本相位不以任何门禁结果为完成判据。**交付物 = 期望登记与移交**
