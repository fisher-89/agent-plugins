# 设计: session-midturn-history-overwrite

> **变更**: session-midturn-history-overwrite
> **状态**: 设计定稿（基于已定稿 proposal + 4 份 delta spec + explore）
> **基于**: `proposal.md`（问题三投影 / 提案六条线 / 变更范围 / AC-1…AC-12 / 风险 R1–R10 / 决策表）、`specs/{desktop-workspace-store,desktop-agent-execution,desktop-agent-chat-infra,desktop-change-orchestration}/spec.md`、`explore.md`（A1 方案 + 关键文件索引）
> **性质**: **一条库态不变量 + 它的消费面**——`AgentRunStatus` 末位增 `Interrupted`、store 增一条启动标定（形态抄既有 `calibrate_interrupted_runs`）、`worker_settle` 由编译器强制补一条 failed 侧臂、前端补播种键来源与三处呈现。**不动 kernel 语义、不动命令签名、不新增事件词汇、不新增 IPC / 配置 / 依赖、零新增文件**。
>
> **本篇边界**: `crates/core/{agent,orchestration}/**`（值域 + 分类臂）、`crates/infra/store/**`（标定 + 一条 doc）、`src/types/generated/**`（再生）、`packages/desktop/src/{lib,components,views}/**`（播种键 + 三投影 + 列表文案）、`packages/desktop/package.json`（版本一行）。测试用例与门禁执行归独立相位，本篇不写测试策略，只登记**测试面已存在的事实**（校准 6）。

---

## 提案与规格同步状态

proposal 的三投影、六条线、AC-1…AC-12 与 R1–R10 均已定稿；4 份 delta 把语义钉到「判据 / 写入 / 幂等 / 呈现 / 文案」粒度（含 `desktop-change-orchestration` 的 `Interrupted` 臂归属与零扩展面）。本设计不重复论证，只做四件事：

1. **把 spec 已钉语义、未钉形状的八处收敛到具体签名 / 落点**（DD1–DD11）：枚举落库兼容的收敛面、标定函数的读扫 / 写入 / 返回形态、`open_workspace` 的调用形态与时刻源、flow 侧臂的文案与臂序、播种键改派后的 `expectSeed` 契约、轮状态下行位的类型与**缺省值**、终结轮未配对部件的 `TraceLine` 具体态、活卡三态矩阵的最终判据。
2. **对 proposal 的六处措辞做工作树校准**（见「事实校准」1–6）：活卡判据缺省值与 proposal R4 的措辞相反（流式消息在收口前没有 `metadata.record`）、「四处同形状」的准确边界（默认对卡不在这四处内）、值域 doc 的第二个出线面（`TurnSummary.status`）、`worker_settle` 是唯一被编译器强制的穷尽 match 点。
3. **逐条 AC 给落点**（末尾「验收标准落点」表）。
4. **把三处「非显然正确」写成可实现形态并留痕**：播种登记为何必须「首个事件即消费」而不是「等到匹配」（否则残留轮事件可无限推迟播种）、轮状态下行位为何只能是布尔且必须缺省 `true`（流式期无 record + `ChatBubble` memo 预算）、终结轮部件的 `TraceLine` 为何取「省略 `status`」而非 `pending`（`pending` 的既有语义是「思考中」的 streaming）。

---

## 起点核实（工作树实测；锚点为改动前行号）

- **值域**：`crates/core/agent/src/runner.rs:24` doc「run 状态四档」、`:30` `pub enum AgentRunStatus { Running, Completed, Failed, Stopped }`（`#[serde(rename_all = "camelCase")]` + `specta::Type`，四 unit variant）。
- **运行期写入点只有三处终态**（`Interrupted` MUST NOT 由其产生）：`crates/core/agent/src/kernel.rs:337-345` 的 `AgentRunState → AgentRunStatus` 映射（`Completed` / `Failed` / `Stopped` 三臂，`Running` 漂移折 `Failed`）；同文件 `:258` / `:316` 两处落库失败折 `Failed`。
- **flow 侧分类单点**：`crates/core/orchestration/src/walker.rs:3282` `fn worker_settle(outcome: &WorkerTurnOutcome) -> (StepSettle, Option<Terminal>)`——四变体全显式、**无 `_` 兜底臂**；`:3810-3812` `worker_settle_probe` doc「四臂逐文案」、`WorkerSettleProbeView` 七字段。
- **`AgentRunStatus` 的穷尽 match 全仓仅此一处**（`grep -rn "match .*\.status\b"` 实证：`workflow/src/queries/{detail,list}.rs`、`infra/archi/runner.rs`、`infra/vcs/git.rs`、`src/commands/archi/mod.rs` 的 match 主体分别是 `ChangeStatus` / `RunStatus` / 报告状态 / `ExitStatus`，与本枚举无关）⇒ 「编译器强制落臂」的落点确为单点。
- **标定先例**：`crates/infra/store/src/store.rs:425-430` `open_workspace` = `open_workspace_probed` + `let _ = store.calibrate_interrupted_runs(now_millis());`；`:2105-2153` `calibrate_interrupted_runs(&self, now: i64) -> Result<Vec<String>, StoreError>`（两段形态：`read_all` 读扫 → 有残留才开 `rw_transaction` 写 → 返回 id 清单）；`:2276` `fn read_all<T: ToInput + DeserializeOwned>(&self, context: &'static str) -> Result<Vec<T>, StoreError>`；`model.rs:1572` `pub(crate) fn now_millis() -> i64`。
- **轮行面**：`crates/infra/store/src/model.rs:128` `#[native_model(id = 2, version = 4, from = AgentRunRecordV3)]`、`:137` `/// 轮状态（running | completed | failed | stopped）`、`:138` `pub status: AgentRunStatus`；`:73` 为 `AgentRunRecordV3`（历史形态）的同款 doc。`store.rs:592` `list_agent_turns`（`read_all::<AgentRunRecord>` + `session_id` 过滤 + `turn_summary` 映射）、`:615` `begin_agent_turn`（写 `Running`）、`:696` `finish_agent_turn`（整组替换终态五字段）。
- **出线 doc 的第二处源**：`crates/core/agent/src/session.rs:71` `/// 轮状态（running | completed | failed | stopped）`——经 specta 直接生成到 `packages/desktop/src/types/generated/bindings.ts:2235`（`TurnSummary.status` 的注释），是本变更**必读的照面**（校准 3）。
- **播种面**：`packages/desktop/src/lib/agent-transport.ts:133` `EnvelopeStream.expectSeed: (turnId: number | null, events: readonly AgentEvent[]) => void`；`:260-289` `PendingSeed` / `createPendingSeed`（`pending.turnId !== message.event.turnId` 即返回、**保留登记等下一次**）；`:333-370` `reconnectToStream`（`:343` 状态门 `detail.turns.some(t => t.status === 'running')`、`:346-347` `detail.turns.find(t => t.status === 'running')` 取选轮键 + 判别早返回、`:353-361` 播种登记、`:395-419` `envelopeStream` 的 `pendingSeed.seedIfMatching(message)` 先于 chunk 投递）；`:41` `observedEvents?: () => readonly AgentEvent[]`（镜像来源，唯一生产调用点 `hooks/use-agent-session.ts:699`）。
- **对话透镜面**：`components/agent/agent-messages.tsx:66` `isLiveAsk(part, pendingToolCallId)`（第三态 `undefined` → `part.state === 'input-available'` 兜底）、`:73` `ToolPartView`、`:140` `PartView`、`:181` `TurnStatusLine`（`failed` 两支 + `stopped` 一支、其余 `null`）、`:310` `ChatBubbleBase`（`record = message.metadata?.record ?? null`；`:318-321` 状态行判据）、`:435-465` `AgentMessages`（`stableAskAnswer` 引用稳定，`pendingToolCallId` 三态不归一）；`components/agent/tool-cards.tsx:46` `AgentToolCardProps`（六字段）、`:220` / `:266` / `:298` / `:345` 四处 `TraceLine status={output ? (output.isError ? 'error' : undefined) : 'running'}`，对应卡 = AskUserQuestion / Bash / Read / Write；`:222`（Ask）/ `:268`（Bash）的 `Spinner` 与 `output === null` 同条件；`:421` `BUILD_IN_TOOL_CARDS` 七项（`edit` / `finish_step` / `web_search` 三卡无 running 标记）。
- **可见性单点**：`lib/agent-virtual.ts:42` `turnStatusOf`、`:52-62` `filterConversation`（白名单 `failed` / `stopped`）。
- **列表面**：`views/agent/components/agent-run-history.tsx:53-58` `statusLabel`（`running` / `completed` / `stopped` 三支 + `失败` 兜底）、`:81` `data-status={status ?? 'empty'}`、`:84-97` 行色三支（`failed` / `completed` / 其余 muted）。
- **生成物 / 版本**：`bindings.ts:451` `turnId?: number | null`（播种键可空的事实依据）、`:2235` `TurnSummary.status` doc；`packages/desktop/package.json:3` `version = "0.6.47"`；`src-tauri/tauri.conf.json` 经 `"version": "../package.json"` 跟随；`src-tauri/Cargo.toml` 自有 crate 版本行不随动。
- **架构契约面**：`.dev-team/architecture/api/INDEX.toml` 中 `App.Backend.Core.Agent` / `App.Backend.Core.Orchestration` / `App.Backend.Infra.Store` 均 `status = "absent"`（模型 `api = none`）⇒ 本变更**零契约文件动作**（写了反而触发 `apiUnexpected`）；`apiFindings` 只对「契约在案元素」折算，本变更不触任何在案元素（唯一在案元素 `App.Backend.Infra.ArchiRuntime` 零改动）。

### 事实校准（proposal 措辞 vs 工作树实测；不改需求意图，只定实现形态）

1. **活卡判据的缺省值必须「按运行中」**（与 proposal R4 的倾向相反，取 spec delta 与实测事实）：流式消息在轮收口前**没有** `metadata.record`——`startChunks` 的 `messageMetadata` 只有 `{ seq, parentToolUseId }`，`record` 由 `recordChunks` 在 `Record` 信封时才挂上。若「无 `record` ⇒ 不判活」，调试页 / explore 的**活卡主路径**（`askAnswer={{ onAnswer }}`，`pendingToolCallId` 恒 `undefined`）会在整个停等期呈静态卡，功能被直接打死。故 DD6 定稿：`record` 缺席按「运行中」处理（`turnRunning` 缺省 `true`）；proposal R4 的「旧数据不再可应答」不再是本设计的代价，代价变为「无轮行的旧转录仍可能误呈可应答 → 一次 `Err`」，与改动前逐字等价（零回归）。
2. **「四处同形状」的准确边界**：四处 = `TraceLine status` 的 `running` 站点（Ask / Bash / Read / Write 四张特化卡）。默认对卡 `ToolPairCard`（`agent-blocks.tsx:191`）的 `TraceLine` **无** `running`、无 shimmer，其「看起来在跑」的载体是 `Spinner` 图标（`state === 'input-available'`）；它既不在 proposal 实现文件清单、也不在 proposal 测试文件清单（无 `agent-blocks.test.tsx` 条目）⇒ 本变更**不触它**，登记为范围边界（见「待决问题」1）。
3. **值域 doc 的第二个出线面**：`TurnSummary.status` 的 doc（`session.rs:71`）直接进生成物注释。不同批改会留下「union 五值、同文件注释四值」的自相矛盾 ⇒ 本设计**补录一行 doc**（纯注释，零线格式影响，理由见 DD11）。
4. **`worker_settle` 是唯一被编译器强制的点**（起点核实第 4 项）⇒ AC-5 的「穷尽 match 保持穷尽」只需守这一处；`AgentRunState`（kernel 内部状态机）是另一枚举，`Interrupted` 不进它（DB 事实 ≠ 内存状态机）。
5. **标定读扫沿用 `read_all` + 内存过滤**：既有 change run 标定即此形态；轮行与已有会话/转录同量级（页面级数据量，单 workspace 库），且 `open_workspace` 在 `WorkspaceStores` 内按 root 缓存（每进程每库一次）⇒ 不引入 secondary 索引扫描（那是另一套读面）。
6. **播种路径当前零测试**：`grep -rn "播种\|seed"` 在 `*.test.ts(x)` 零命中（既有 `reconnectToStream` 用例只覆盖状态门 / 簿记 / 收口 / abort）。故 AC-6 的回归用例是**新增面**，不是改写既有断言——测试相位不必担心「既有断言语义」冲突，但必须自建夹具（残留 running 行在前 + 实时流属其后新轮）。

---

## 关键设计决策

> proposal 决策表的十项已定稿，本篇不重复。下表 DD1–DD11 是本设计在 spec 已钉语义之外**新增的形状裁决**——每条含「为什么另一种写法会坏」。

| # | 议题 | 定稿 | 理由 |
|---|---|---|---|
| DD1 | 枚举追加的落库收敛面 | `AgentRunStatus` **末位**追加 `Interrupted`；**不动** `#[native_model(id = 2, version = 4)]`、不动 `From<AgentRunRecordV3>` 两向映射、不动 `begin_agent_turn` / `finish_agent_turn` | 变体索引只增不改 ⇒ 旧行解码值不变（AC-2）；版本递增会强制全库升级重写（不必要的数据改写，且与「旧行零迁移」冲突）。`Interrupted` 只在下次启动由标定写入，运行期写路径三处（kernel 两处 + `finish_agent_turn`）逐字不动 |
| DD2 | 标定函数的形态 | `pub fn calibrate_interrupted_agent_turns(&self, now: i64) -> Result<Vec<i64>, StoreError>`：`read_all::<AgentRunRecord>("遍历轮统计行")` → `filter(status == Running)` → 空则 `Ok(vec![])`（**不开写事务**）→ 否则单 `rw_transaction` 内逐行 `status = Interrupted` + `finished_at = Some(now)` + `error = Some(记因)` → `commit` → 返回被标定 `id` 清单 | 与 `calibrate_interrupted_runs` **逐段同形**（读扫→判空→写事务→commit→返回清单），零新机制、零新抽象。统计三字段不写（「不虚构」）；只 `upsert` 轮行本体（`session_id` / `started_at` 原样带过，避免 `finish_agent_turn` 那类「整组替换」误伤） |
| DD3 | 标定的触发点与时刻源 | `open_workspace` 内**同层并列**两行：`let now = now_millis(); let _ = store.calibrate_interrupted_runs(now); let _ = store.calibrate_interrupted_agent_turns(now);`（两族标定共享一次取时） | 同库同打开点的既有先例；`let _ =` 吞错形态沿用（proposal 决策：播种修复是不依赖标定成功的第二道防线）。共享 `now` 让两族 `finished_at` 同源（同一启动瞬间的事实），且只读一次时钟。不新增启动钩子、不动 `main.rs` 的 `setup_hook` |
| DD4 | flow 侧 `Interrupted` 臂 | `AgentRunStatus::Interrupted => { let reason = "会话被中断".to_owned(); (StepSettle::failed(Some(reason.clone())), Some(Terminal::failed(reason))) }`，臂序**紧邻 `Stopped` 之后**（`Running` 漂移臂之前）；`worker_settle` doc 的三变体枚举补 `Interrupted`、`worker_settle_probe` doc 的「四臂」→「五臂」 | 归 failed 侧（该步无可信产出、且不是用户主动停止）；文案与 `Stopped` 臂的「会话被终止」**可辨**（AC-5 对拍）；步 detail 与 run 终态同文（复用 `reason` 克隆，与 `Failed` / `Running` 两臂同形）。**无 `_` 兜底臂**——新变体由编译器强制，这正是本变更要的显式裁量面 |
| DD5 | 播种键改派的契约 | `PendingSeed.expect` 去掉 `turnId` 入参，改收「已过滤镜像」：`expect: (events: readonly AgentEvent[]) => void`；`seedIfMatching` 改为**首个 `event` 信封即消费登记**（无论轮次是否匹配），随后以 `message.event.turnId` 过滤镜像再预投；`turnId` 为 `null` / `undefined` 时消费登记且**不投**（无权威轮次可对账） | 「等到匹配」的旧形态在改派后无法成立：登记里已无期望轮次，若遇不匹配事件仍保留登记，则残留轮事件会无限推迟播种、直至某条同轮事件到达——「只投一次」的时序不变量失守。消费即弃 + 按事件 `turnId` 过滤 ⇒ 只投一次、跨轮零串扰、残留轮零预投。`reconnectToStream` 侧只删掉取选轮键的两行（`:346-347`），状态门（`:343`）**保留**——它是「零进度会话不订阅」的既有语义，不是选轮依据 |
| DD6 | 轮状态下行位的类型与缺省 | `ChatBubbleBase` 内算 `const turnRunning = record === null ? true : record.status === 'running'`，**只向下穿一个布尔**（`PartView` → `ToolPartView` → `Card` 的 `turnRunning?: boolean`）；`ChatBubble` 的 props 面**不变**（不新增 prop） | 「按运行中」是唯一安全缺省（校准 1：流式期无 `record`）。布尔而非 `AgentRunStatus`：卡层不需要认识状态词表（枚举扩值不该波及四张卡），且原始布尔引用稳定——若传 `record` 对象或 `status` 字符串的新派生对象会击穿 `ChatBubble` 的 memo 与 `renderItem` 的 `useCallback`（R5 的预算）。`ChatBubble` props 面不变 ⇒ memo 判据逐字不动 |
| DD7 | 终结轮未配对部件的 `TraceLine` 态 | 取**省略 `status`（`undefined`）**，MUST NOT 取 `pending`；同时该卡的「运行图标」（`Spinner`）随同退场（Ask → `MessageCircleQuestionIcon`、Bash → `TerminalIcon`；Read / Write 图标本就与结果无关） | `TraceLine` 的 `pending` 在既有代码里是 `ThinkingBlock` 的 **streaming** 语义（`opacity-60` + 旋转之外的「进行中」暗示），复用它会把「没拿到结果」误读成「还在进行」——正是本变更要消灭的歧义。省略 `status` 与默认对卡（`agent-blocks.tsx:191` 的 `output?.isError ? 'error' : undefined`）同口径：「无结果」由卡内空输出区呈现。Spinner 必须同批退场，否则「无 shimmer 但还在转」仍是症状 2 |
| DD8 | 活卡三态矩阵的最终判据 | `isLiveAsk(part, pendingToolCallId, turnRunning)`：①`pendingToolCallId === null` → `false`（当前无等待）；②字符串 → `part.toolCallId === pendingToolCallId`（精确对账不变）；③`undefined` → `part.state === 'input-available' && turnRunning !== false`（结果缺席**只作为必要条件**，判据主体是该轮状态） | 保住 `input-available` 前置是必要的：已应答卡（`output-available`）在 `turnRunning === true` 的轮内仍须静态（spec「已应答回填 … 零应答入口」）。把「结果缺席」降为必要条件、轮状态升为充分条件，即 spec 要求的「由结构保证」而非调用侧纪律 |
| DD9 | 中断轮的状态行与可见性 | `TurnStatusLine` 在 `stopped` 支之后增 `interrupted` 支：`data-testid="turn-status-line"` + `data-status="interrupted"` + 内层 `data-testid="turn-status-line-interrupted"`，文案「因应用重启中断」，`text-muted-foreground`（与 `stopped` 同色），**不读** `record.error`（不进失败标题与原文展开面）；`filterConversation` 白名单 `failed` / `stopped` → `failed` / `stopped` / `interrupted` | 与「已停止」同为非故障侧（R7：不得泄漏成第二个失败）；标定记因是机器事实不是用户待处置故障，故不展开。白名单是可见性单点（虚拟列表 `count` 与吸附层几何都以它为真相），扩在白名单处即让空内容中断轮「可见 + 有状态行」 |
| DD10 | 列表文案与配色 | `statusLabel` 增 `if (status === 'interrupted') return '已中断';`（置于 `stopped` 之后、`失败` 兜底之前）；行色**零改动**（`interrupted` 自然落既有的 `text-muted-foreground` 分支，与 `stopped` 同列）；`data-status` 已是 `status ?? 'empty'` 直出，自动呈 `interrupted` | 沿用 change 流程 `run-control-panel` 的 `RUN_STATUS_LABEL` 词面先例（「已中断」）；为同一个 muted 色再加一条三元分支是噪声，配色语义「非故障 = 灰」已由既有分支表达 |
| DD11 | 版本与生成物纪律 | `packages/desktop/package.json` `0.6.47` → `0.6.48`（`tauri.conf.json` 经 `"version": "../package.json"` 跟随；`src-tauri/Cargo.toml` crate 版本行 **MUST NOT** 随动）；`bindings.ts` / `bindings.zod.ts` 由 `bindings:export` 再生成（**MUST NOT 手改**）；同批更新两处出线 doc 源（`model.rs:137` 与补录的 `session.rs:71`） | 版本纪律是仓规（每次桌面源码改动 +1 patch）。两处出线 doc 若不同批，`bindings.ts` 会出现「`AgentRunStatus` union 五值、`TurnSummary.status` 注释四值」的自相矛盾（校准 3）——这是纯注释、零线格式风险，故与实现同批。`model.rs:73`（`AgentRunRecordV3` 历史形态）**不改**：它描述的是枚举化前的历史落库形态，属历史事实 |

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 轮状态值域（**扩**） | `AgentRunStatus` 末位增 `Interrupted`（线格式 `"interrupted"`，仅启动标定写入；doc 扩五档）；`TurnSummary.status` doc 同步（出线注释） | `crates/core/agent/src/runner.rs`、`crates/core/agent/src/session.rs`（doc） | 既有 `serde` / `specta` | unit 枚举变体追加（bincode 索引只增不改） |
| flow 侧分类臂（**扩**） | `worker_settle` 补 `Interrupted` 臂（failed 侧、文案「会话被中断」、紧邻 `Stopped`）；函数与探针 doc 的「四态 / 四臂」→ 五态 / 五臂 | `crates/core/orchestration/src/walker.rs` | `agent::AgentRunStatus` | 穷尽 `match` + 既有 `StepSettle` / `Terminal` 面（零变体扩展） |
| agent 轮启动标定（**新增**） | `calibrate_interrupted_agent_turns(now)`：只扫 `AgentRunRecord.status == Running`，翻 `Interrupted` + `finished_at` + 记因；零残留不开写事务；返回 id 清单 | `crates/infra/store/src/store.rs` | `native_db`、既有 `read_all` / `db_err` | 两段形态（读扫 → 写事务），与 `calibrate_interrupted_runs` 同构 |
| 打开点接线（**改**） | `open_workspace` 内共享一次 `now_millis()`，两族标定并列调用（`let _ =` 吞错沿用） | 同上 | — | 三行机械改动 |
| 落库类型 doc（**改**） | `AgentRunRecord.status` 的取值清单补 `interrupted`（字段面 / 版本头零改动） | `crates/infra/store/src/model.rs` | — | 注释 |
| 生成物（**再生**） | `AgentRunStatus` TS union + zod literal 增 `"interrupted"`；`TurnSummary.status` / `AgentRunRecord`（store 内部不出线）自动跟随 | `packages/desktop/src/types/generated/{bindings.ts,bindings.zod.ts}` | `bindings:export` 管线 | specta + zod 生成（勿手改） |
| 订阅播种（**改**） | 播种键由首个实时事件自证：`expectSeed(events)` 登记镜像，首个 `event` 信封消费登记并以该事件 `turnId` 过滤后预投；状态门保留 | `packages/desktop/src/lib/agent-transport.ts` | `@ai-sdk/react`、`ipc` | 纯函数 + 既有 `StreamState` 簿记 |
| 对话透镜（**改**） | `turnRunning` 下行（布尔，缺省 `true`）；`isLiveAsk` 第三态改读轮状态；`TurnStatusLine` 增 `interrupted` 支；四张特化卡的 `TraceLine` / `Spinner` 判据加支 | `packages/desktop/src/components/agent/{agent-messages,tool-cards}.tsx` | React、`@shadcn/react/questionnaire`、既有 `TraceLine` | props 下行（只穿原语）+ 单点判据函数 |
| 可见性单点（**改**） | `filterConversation` 白名单补 `interrupted` | `packages/desktop/src/lib/agent-virtual.ts` | — | 纯函数 |
| 会话历史列表（**改**） | `statusLabel` 增「已中断」档（行色 / `data-status` 零改动） | `packages/desktop/src/views/agent/components/agent-run-history.tsx` | 生成物 union | 词面映射 |
| 版本交付（**改**） | `version` `0.6.47` → `0.6.48` | `packages/desktop/package.json` | — | — |

<!-- 架构模型零改动：App.Backend.Core.Agent / App.Backend.Core.Orchestration / App.Backend.Infra.Store
     三元素均 api = none（INDEX.toml status = absent），新增 pub 面不写契约文件（写了触发 apiUnexpected）；
     .dev-team/architecture/models/** 零改动。 -->

---

## 变更清单

> 路径口径：Rust 侧省略 `packages/desktop/src-tauri/` 前缀。测试文件与用例归测试相位；本清单只列实现文件（13 个路径），另设「范围补录」段登记 proposal 清单之外的 **1 行纯 doc**（`session.rs`，理由见校准 3）与「零 diff 登记」段列明 proposal「不要修改」面的守线口径。

<!-- 新增文件：本变更零新增文件——枚举值 / 标定函数 / 分类臂 / 播种键 / 三处呈现全部落在既有文件的既有位置上。 -->

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `crates/core/agent/src/runner.rs` | ①`AgentRunStatus` 末尾追加 `Interrupted`（doc：语义与 change run 域 `RunStatus::Interrupted` 同义 / 唯一写入点是启动标定 / 末位追加以保落库 bincode 索引不变）；②枚举 doc「run 状态四档」→「五档」 | 值域单一事实源；出线与落库形态同源（specta + serde 自动跟随） |
| `crates/core/agent/src/session.rs` | `TurnSummary.status` doc「running \| completed \| failed \| stopped」→ 补 `interrupted`（**补录**，纯注释） | 该 doc 经 specta 生成到 `bindings.ts` 的 `TurnSummary.status` 注释（校准 3）；不同批会留下 union 五值 / 注释四值的自相矛盾 |
| `crates/core/orchestration/src/walker.rs` | ①`worker_settle` 增 `AgentRunStatus::Interrupted` 臂（`StepSettle::failed` + `Terminal::failed`，同文「会话被中断」，臂序紧邻 `Stopped` 之后，`match` 仍全显式无 `_`）；②`worker_settle` doc 的变体枚举补 `Interrupted` 与归属一句；③`worker_settle_probe` doc「四臂」→「五臂」 | 编译器强制的显式裁量面（DD4）；`Terminal` / `StepSettle` / `RunFault` 变体面零扩展 |
| `crates/infra/store/src/store.rs` | ①`impl Store` 新增 `calibrate_interrupted_agent_turns(&self, now: i64) -> Result<Vec<i64>, StoreError>`（置于 `calibrate_interrupted_runs` 之后，DD2 形态；doc 写明判据 / 写入 / 幂等 / 唯一写入点 / 与 change run 标定的并列关系）；②`open_workspace` 内加 `let now = now_millis();` 与一行 `let _ = store.calibrate_interrupted_agent_turns(now);`（既有 `calibrate_interrupted_runs(now_millis())` 改传 `now`），doc 一句「打开点并列两族启动标定」 | 标定语义与触发点（AC-3 / AC-4）；吞错形态沿用现状（R3 的第二道防线是播种修复） |
| `crates/infra/store/src/model.rs` | `AgentRunRecord.status` doc 取值清单补 `interrupted`（**字段面 / `#[native_model(id = 2, version = 4)]` / 两向 `From` 零改动**） | 枚举扩值的落库面只增 doc（DD1）；`:73` 的 `AgentRunRecordV3` doc 属历史形态，不改（DD11） |
| `packages/desktop/src/types/generated/bindings.ts` | 经 `pnpm -C packages/desktop run bindings:export` 再生成：`AgentRunStatus` union 增 `"interrupted"`、`TurnSummary.status` doc 跟随（**勿手改**） | AC-1；`bindings.zod.ts` 同批再生 |
| `packages/desktop/src/types/generated/bindings.zod.ts` | 同上（`AgentRunStatus` 的 zod literal union 随动） | `client:check`（`bindings:zod-check`）守卫 |
| `packages/desktop/src/lib/agent-transport.ts` | ①`PendingSeed.expect` 改为 `(events: readonly AgentEvent[]) => void`；`seedIfMatching` 改为「首个 `event` 信封即消费登记 → 以该事件 `turnId` 过滤镜像后预投，`turnId` 为空则不投」；②`reconnectToStream` 删除 `detail.turns.find(...)` 选轮两行，`expectSeed` 改传「镜像全量（非 delta、`seq <= sinceSeq`）」；③`EnvelopeStream.expectSeed` 类型与三处 doc（`reconnectToStream` / `createPendingSeed` / `expectSeed`）同步改写：播种键 = 首个实时事件自证，`detail.turns` 只作状态门（MUST NOT 用作选轮依据） | AC-6 / 症状 1；状态门与 `sinceSeq` 计算逐字不动 |
| `packages/desktop/src/lib/agent-virtual.ts` | `filterConversation` 可见性白名单 `failed` / `stopped` → `failed` / `stopped` / `interrupted`；doc「判据语义」一句同步 | AC-9；判据单点不变（虚拟项数 / 吸附几何自动跟随） |
| `packages/desktop/src/components/agent/agent-messages.tsx` | ①`isLiveAsk` 增第三参 `turnRunning`（三态矩阵见 DD8）；②`ToolPartView` / `PartView` 增可选 `turnRunning` 并下行到 `Card`；③`ChatBubbleBase` 内算 `turnRunning = record === null ? true : record.status === 'running'` 并只传布尔；④`TurnStatusLine` 增 `interrupted` 支（文案 / testid / 配色见 DD9），doc 同步 | AC-8 / AC-9；`ChatBubble` 的 props 面与 memo 判据零改动（DD6） |
| `packages/desktop/src/components/agent/tool-cards.tsx` | ①`AgentToolCardProps` 增可选 `turnRunning?: boolean`（doc：缺席按运行中处理）；②新增模块内单点判据 `unpairedRunning(output, turnRunning)`；③四处 `TraceLine status` 与 Ask / Bash 的 `Spinner` 判据改用该单点（终结轮无结果 → `status` 省略 + 特化图标，DD7） | AC-7 / 症状 2；`edit` / `finish_step` / `web_search` 三卡零改动；默认对卡不在范围（校准 2） |
| `packages/desktop/src/views/agent/components/agent-run-history.tsx` | `statusLabel` 增 `interrupted` → 「已中断」支（行色 / `data-status` 零改动） | AC-10 |
| `packages/desktop/package.json` | `version` `0.6.47` → `0.6.48` | AC-12；`src-tauri/Cargo.toml` 不随动 |

### 公共函数 / API

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|-----------|------|------|
| `calibrate_interrupted_agent_turns` | `crates/infra/store/src/store.rs` | 新增 | `pub fn calibrate_interrupted_agent_turns(&self, now: i64) -> Result<Vec<i64>, StoreError>` | 启动标定：只扫 `AgentRunRecord.status == Running`，翻 `Interrupted` + `finished_at = Some(now)` + 记因「重启标定：桌面进程中断，会话轮客观已终止」；读扫零残留不开写事务；幂等；返回被标定轮 id 清单（`pub`，语料 / 测试构造中断样本直调，沿既有标定形态） |
| `Store::open_workspace` | 同上 | 修改 | `pub fn open_workspace(path: &Path) -> Result<Self, StoreError>` | 签名不变；函数体追加一次并列标定调用（共享 `now`），吞错形态沿用 `let _ =` |
| `filterConversation` | `packages/desktop/src/lib/agent-virtual.ts` | 修改 | `export function filterConversation(all: AgentUIMessage[]): AgentUIMessage[]` | 空内容轮白名单补 `interrupted`（签名不变） |
| `TauriAgentTransport.reconnectToStream` | `packages/desktop/src/lib/agent-transport.ts` | 修改 | `public async reconnectToStream(options: { chatId: string; abortSignal?: AbortSignal; body?: object }): Promise<ReadableStream<AgentUIMessageChunk> \| null>` | 签名与返回语义不变；内部播种键改由首个实时事件自证，`detail.turns` 只留状态门 |

### 内部契约面（非导出；spec 模块契约在案的落点）

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|-----------|------|------|
| `worker_settle` | `crates/core/orchestration/src/walker.rs` | 修改 | `fn worker_settle(outcome: &WorkerTurnOutcome) -> (StepSettle, Option<Terminal>)` | 增 `Interrupted` 臂（签名不变，五变体全显式） |
| `isLiveAsk` | `packages/desktop/src/components/agent/agent-messages.tsx` | 修改 | `function isLiveAsk(part: AgentToolPart, pendingToolCallId: string \| null \| undefined, turnRunning: boolean \| undefined): boolean` | 三态矩阵（DD8）：`null` → false；字符串 → id 相等；`undefined` → `part.state === 'input-available' && turnRunning !== false` |
| `TurnStatusLine` | 同上 | 修改 | `function TurnStatusLine({ message, record }: { message: AgentUIMessage; record: TurnSummary }): React.JSX.Element \| null` | 增 `interrupted` 支（DD9）；`message` 参数在 `interrupted` 支不消费（不读 `api_error` 通知） |
| `PartView` | 同上 | 修改 | `function PartView({ part, askAnswer, turnRunning }: { part: AgentUIMessage['parts'][number]; askAnswer?: AskAnswerActions; turnRunning?: boolean }): React.JSX.Element \| null` | 只向 `ToolPartView` 透传布尔；user 气泡调用点不传（缺省按运行中，无工具部件故无影响） |
| `ToolPartView` | 同上 | 修改 | `function ToolPartView({ part, askAnswer, turnRunning }: { part: AgentToolPart; askAnswer?: AskAnswerActions; turnRunning?: boolean }): React.JSX.Element \| null` | `isLiveAsk` 第三参；`turnRunning` 下行到四张特化卡（默认对卡 `ToolPairCard` 不接该位——校准 2） |
| `createPendingSeed` | `packages/desktop/src/lib/agent-transport.ts` | 修改 | `function createPendingSeed(state: StreamState, controllerOf: () => ReadableStreamDefaultController<AgentUIMessageChunk> \| null): PendingSeed` | 签名不变，契约改（DD5） |
| `unpairedRunning` | `packages/desktop/src/components/agent/tool-cards.tsx` | 新增 | `function unpairedRunning(output: AgentToolOutput \| null, turnRunning: boolean \| undefined): boolean` | 返回 `output === null && turnRunning !== false`；四处 `TraceLine` 与 Ask / Bash 图标判据的**同一单点**（R6 的收口手段：形状只剩一处） |
| `statusLabel` | `packages/desktop/src/views/agent/components/agent-run-history.tsx` | 修改 | `function statusLabel(status: AgentRunStatus): string` | 五值映射，`interrupted` → 「已中断」；不落 `失败` 兜底 |

### 类型定义

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|--------|----------|-----------|------|
| `AgentRunStatus` | `crates/core/agent/src/runner.rs` | 修改（变体） | 五变体 `Running` / `Completed` / `Failed` / `Stopped` / `Interrupted`（序不变，`Interrupted` 末位）；`#[serde(rename_all = "camelCase")]` + `specta::Type` 不变；线值 `"interrupted"` |
| `AgentRunRecord.status` | `crates/infra/store/src/model.rs` | 修改（doc） | 字段类型与 `#[native_model]` 头零改动；只有 doc 取值清单补 `interrupted` |
| `TurnSummary.status` | `crates/core/agent/src/session.rs` | 修改（doc） | 同上（出线注释源，校准 3） |
| `AgentToolCardProps` | `packages/desktop/src/components/agent/tool-cards.tsx` | 修改 | 六字段 → 七字段：增 `turnRunning?: boolean`（缺席按运行中处理；`AgentToolCard` 类型别名不变） |
| `PendingSeed` | `packages/desktop/src/lib/agent-transport.ts` | 修改 | `{ expect: (events: readonly AgentEvent[]) => void; seedIfMatching: (message: AgentRunMessage) => void }` |
| `EnvelopeStream.expectSeed` | 同上 | 修改 | `expectSeed: (events: readonly AgentEvent[]) => void`（`turnId` 入参退役） |
| `AskAnswerActions` | `packages/desktop/src/components/agent/agent-messages.tsx` | **零改动** | 三态 `pendingToolCallId` 语义不变（`undefined` = 未提供 → 轮状态兜底） |
| `WorkerSettleProbeView` | `crates/core/orchestration/src/walker.rs` | **零改动** | 七字段探针面复用（新臂经既有直调面自动可测） |

<!-- 配置：本变更零配置键改动——.dev-team/config.json、config.schema.json、KNOWN_KEYS、
     TestFramework、EngineConfig、provider 记录字段面均零 diff（中断是库态事实，无开关）。 -->

---

## 数据模型

### `AgentRunStatus` 值域与线格式

| 变体 | 线值 | 语义 | 写入点 |
|------|------|------|--------|
| `Running` | `"running"` | 轮进行中 | `begin_agent_turn`（唯一） |
| `Completed` | `"completed"` | 正常收敛（TurnDone `is_error = false`） | `finish_agent_turn`（经 kernel 收口） |
| `Failed` | `"failed"` | 失败收敛（TurnDone error / 无终态异常终止 / 落库失败） | 同上（含 kernel 两处折 `Failed`） |
| `Stopped` | `"stopped"` | 用户主动终止 | 同上 |
| `Interrupted` | `"interrupted"` | **进程中断后的客观终止**（与 change run 域 `RunStatus::Interrupted` 同义） | `calibrate_interrupted_agent_turns`（**唯一**；运行期写路径 MUST NOT 产生） |

**落库兼容**：`AgentRunRecord` 经 `native_model(id = 2, version = 4)` 落 workspace 库。变体末位追加 ⇒ 已落库行的变体索引不位移（旧行解码值与原四值逐一相同），**不升版本、不写迁移、不动两向 `From`**。AC-2 由测试相位以「值域往返 + 旧行四值解码」两向断言固化。

### `AgentRunRecord`（字段面零改动）

| 字段 | 类型 | 本变更 |
|------|------|--------|
| `id` | `i64`（主键） | 标定返回的 id 清单即此列 |
| `session_id` | `Option<String>` | 标定原样带过（不重算归属） |
| `status` | `AgentRunStatus` | `Running → Interrupted`（唯一被翻的边） |
| `started_at` | `i64` | 原样（不重写轮起点） |
| `finished_at` | `Option<i64>` | `None → Some(now)`（标定时刻） |
| `num_turns` / `cost_usd` / `duration_ms` | `Option<…>` | **不虚构**：原值保留（通常为 `None`） |
| `error` | `Option<String>` | `None → Some("重启标定：桌面进程中断，会话轮客观已终止")`（定式；与 change run 记因同格式，无相位语境 —— 轮没有 `active_phase` 对应物） |

### 状态迁移（唯一非法边被关掉）

| 起点 | 触发 | 终点 |
|------|------|------|
| （无行） | `begin_agent_turn` | `running` |
| `running` | TurnDone `is_error = false` | `completed` |
| `running` | TurnDone error / 无终态 / 落库失败 | `failed` |
| `running` | 用户 stop | `stopped` |
| `running` | **下次 `open_workspace` 的启动标定** | `interrupted` |
| 任一终态 | 启动标定 | **不变**（标定只读扫 `running`；`interrupted` 亦不被二次改写） |

不变量：**进程中断 ⇔ 未收口的轮被标为 `interrupted`**，与轮停在哪个工具无关（不依赖停等态 / 工具类型）；由此「轮还在跑吗」只有一个权威答案（库态），前端三投影同时归位。

### 打开点流程（`open_workspace`）

```
open_workspace(path)
  └─ open_workspace_probed(path)                  // 既有：格式探测 / 旧库作废重建
  └─ let now = now_millis();                      // 一次取时（两族同源）
  └─ let _ = calibrate_interrupted_runs(now);     // 既有：change run + active_phase 悬挂
  └─ let _ = calibrate_interrupted_agent_turns(now);  // 新增：agent 轮 running 残留
```

两族互不触碰对方表（`RunRecord` / `ChangeRecord` ↔ `AgentRunRecord`），转录（`SessionEventRecord`）与 `SessionRecord` 零触碰（转录是事实，不改写）。

### 关系与持久化

| 项 | 现状 | 本变更 |
|----|------|--------|
| 库文件 / 维度 | workspace 库（`workspaces/<identity>.redb`） | 零改动 |
| 表结构 / 模型版本头 | `AgentRunRecord` v4 | 零改动（只增枚举变体与注释） |
| 读面 | `session_detail` → `list_agent_turns` → `TurnSummary` | 零改动（`status` 自动透出 `interrupted`） |
| 写面 | `begin_agent_turn` / `finish_agent_turn` / 标定 | **仅新增标定一个写点** |
| 事件词汇 | `AgentEventKind` 六变体 | 零新增（中断是库态事实，不是事件） |

---

<!-- 路由 / API 设计：本变更零新增命令、零签名改动（agent_start / agent_stop / agent_answer /
     agent_steer / agent_session_watch / session_detail 全部逐字不动），零 HTTP 端点，
     零新事件词汇。中断的可见性全部经既有查询面（session_detail / SessionSummary.turns）与既有
     命令面（agent_answer 的既有 miss 臂）表达，故本段省略。 -->

---

## 依赖

- **运行时依赖**：**零新增**。Rust 侧只用既有 `serde` / `specta` / `native_db`（枚举变体与标定函数不引任何新 crate）；前端只用既有 React / ai-sdk reducer 面（播种改派是纯函数改写）。
- **构建 / 测试依赖**：**零新增**（无新 feature、无新 dev-dependency、无测试装置引入）。
- **生成物管线**：既有 `bindings:export`（specta → `bindings.ts` + `bindings.zod.ts`），本变更只消费不扩展。
- **依赖方向**：零新依赖边（`core/agent` ← `core/orchestration`、`core/agent` ← `infra/store`、`infra/store` ← `src-tauri` 全为既有方向；前端 `lib → types/generated`、`components → lib` 零新增 import 边）。

---

## 验收标准落点

> 只给落点，不含测试策略（用例与门禁由独立相位承接）。

| AC | 落点 |
|----|------|
| AC-1 枚举值全链在场 | `runner.rs` 变体追加 → `bindings:export` 再生（`AgentRunStatus` union + zod literal 双含 `"interrupted"`）；`TurnSummary.status` 自动跟随（doc 源已同批同步）；六命令签名零 diff |
| AC-2 旧行零迁移 | 变体末位追加 + `#[native_model(id = 2, version = 4)]` 与两向 `From` 零改动（DD1）；`bincode` 变体索引不位移 |
| AC-3 标定语义 | `calibrate_interrupted_agent_turns`（DD2）：只扫 `running`、写 `status` / `finished_at` / 记因、统计三字段不写、零残留不开写事务、返回 id 清单、已终态零改写、`RunRecord` 与转录零触碰 |
| AC-4 标定触发点 | `open_workspace` 同层并列调用（DD3，共享 `now`）；`session_detail` 经既有 `list_agent_turns` 直接返 `interrupted` 行 |
| AC-5 flow 侧收口 | `worker_settle` 的 `Interrupted` 臂（DD4）：`StepSettle::failed` + `Terminal::failed`「会话被中断」、步 detail 与 run 终态同文、五变体全显式无 `_`；`Terminal` / `StepSettle` / `RunFault` 零扩展；既有明文枚举扩写已在 `desktop-change-orchestration` delta 整段留痕（本设计不改 spec） |
| AC-6 症状 1 回归 | `agent-transport.ts` 播种键自证（DD5）：状态门保留、选轮键退役、首个 `event` 即消费登记并按 `turnId` 过滤；残留轮零预投、跨轮零串扰 |
| AC-7 症状 2 呈现 | `unpairedRunning` 单点 + 四处 `TraceLine status` + Ask / Bash 的 `Spinner`（DD7）：终结轮无结果 → 无 `running` / 无 shimmer、特化图标；`running` 轮沿用现状 |
| AC-8 症状 3 判据 | `isLiveAsk` 三态矩阵（DD8）：`undefined` 态读轮状态（`input-available` 仅作必要条件）；`pendingToolCallId` 精确对账不变 |
| AC-9 状态行与可见性 | `TurnStatusLine` 的 `interrupted` 支（DD9）+ `filterConversation` 白名单；`agent-error.ts` 的 `latestAgentFailure` 只认 `failed` / 「running 且失败 done」⇒ 中断零改动静默不触发（零 diff，由测试固化） |
| AC-10 列表文案 | `statusLabel` 增「已中断」（DD10）；`data-status` 直出；四值词面零改动 |
| AC-11 后端拒绝面可归因 | 命令面**零 diff**：`agent_answer` 的既有 miss 臂 `Err`（注册表空 ⇒ 必然 miss）、`agent_stop` miss 幂等 `Ok` 行为不变；库态与之一致由标定保证 |
| AC-12 版本与门禁 | `package.json` `0.6.48`（DD11）；`vp check` / `vp test` / `knip` / `client:check`（含 `bindings:export` 无 stale diff 与 zod 一致）/ rust nextest 由桌面端 static-check / test-execution 相位执行（本相位只登记交接） |

### 范围补录（超出 proposal 实现文件清单的 1 行，纯 doc 注释）

| 文件 | 改动 | 为何必须同批 |
|------|------|--------------|
| `crates/core/agent/src/session.rs`（1 行 doc） | `TurnSummary.status` doc 补 `interrupted` | 该 doc 直接生成到 `bindings.ts` 的 `TurnSummary.status` 注释；不同批则生成物自相矛盾（union 五值 / 注释四值）——校准 3 |

> 除上表 1 行（纯注释、零线格式 / 零逻辑影响）外，**无任何超出 proposal 清单的实现改动**。

### 零 diff 登记（proposal「不要修改」面的守线口径）

| 面 | 守线判据 |
|----|----------|
| `crates/core/agent/src/kernel.rs` | `git diff` 为空；`Interrupted` 不经 `AgentRunState` 映射产生 |
| `crates/core/agent/src/ask.rs` 与三张内存注册表 | `git diff` 为空（`register` / `answer` / `remove` / `is_pending` 零改动） |
| `crates/infra/agent/src/sdk/resume.rs` | `git diff` 为空（`INTERRUPTED_RESULT` 既有兜底） |
| `calibrate_interrupted_runs` 与 change 域 `RunStatus` | 函数体逐字不动（只在旁新增并列函数 + 打开点多一行调用） |
| `crates/core/orchestration/src/port.rs`（`RunFault`）与 `Terminal` / `StepSettle` / `ChangeStepStatus` / `ChangeRunStatus` | 变体面零扩展（`grep` 零新增变体） |
| 相位机其余面（`enter_phase` / `settle_phase` / `drive` / `run_step` / `run_worker`） | `git diff -- walker.rs` 的 hunk 只落在 `worker_settle` + 两处 doc |
| 命令签名面六命令 + `AgentEventKind` | `git diff -- src-tauri/src/commands/**` / `crates/core/agent/src/event.rs` 为空 |
| 会话接续（`resume_turn` / loop 第三入口）/ flow 停等态落库 / ask 取消通路 | 零新增代码（`grep` 零命中新标识符） |
| `openspec/specs/**` 既有基线 | 零改动（本变更只写 `openspec/changes/<name>/specs/**` delta） |

---

## 待决问题

1. **默认对卡 `ToolPairCard` 的 `Spinner` 未收口**（校准 2）：它在终结轮的无结果部件上仍会旋转（其 `TraceLine` 无 `running`、无 shimmer，故不违反 AC-7 的字面判据，也不在 proposal 的文件清单与测试面内）。本变更显式不动它——把该位也收口会引入 `agent-blocks.tsx` 的生产改动与 `agent-blocks.test.tsx` 的新用例，属**超范围**。建议后续单开 change（同一「轮状态」下行位复用即可，届时把 `turnRunning` 一并传给 `ToolPairCard`）。此项登记为已知残余，MUST NOT 在本变更的验收判据里认账。
2. **标定失败的可见面**：`open_workspace` 的 `let _ =` 吞错沿用现状（proposal 决策），即标定失败在启动日志外无用户可见面。本变更**不新增**日志 / 提示面（播种修复是功能侧第二道防线，AC-6 独立成立）；若后续需要「标定失败可见」，属启动可观测面，另开 change。
3. **播种登记的镜像裁剪口径**：`expectSeed` 收到的镜像仍是「非 `messageDelta` + `seq <= sinceSeq`」既有过滤（`reconnectToStream` 侧完成）。改派后此处**不再**按轮过滤（轮次在首个事件到达时才知），过滤改在 `seedIfMatching` 内做——两处过滤的职责边界在本设计定稿（外层管「游标 / 增量」，内层管「轮次」），若后续要给 `expectSeed` 加别的入口，须守此分工。
4. **`interrupted` 在 change 流程视图的呈现**：flow 侧把它折成 failed（节点红 + 「会话被中断」），故 `run-step-node.tsx` 的 `ChangeStepStatus` 词表与配色**零改动**；若产品后续希望流程节点区分「中断」与「失败」，需要新节点词面，属另一变更（本变更已明文 MUST NOT 新增 `Terminal` / `StepSettle` 变体）。
