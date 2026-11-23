# 设计: backend-notification-hub

> **变更**: backend-notification-hub（后端通知模块 · 通知扇出 + 持久化收件箱 + 前端门面）
> **状态**: 设计定稿（含回溯 1 返工：事件轨注入面移入 orchestration；含回溯 2 返工：接线自检判据按合法命中面收窄；基于已定稿 proposal / 五份 delta spec：`desktop-notification-hub` / `desktop-app-shell` / `desktop-workspace-store` / `desktop-data-dimensions` / `repo-archi-model`）
> **基于**: `proposal.md`（§一 词汇与扇出点 / §二 双写入点 / §三 白名单与幂等主键 / §四 收件箱持久化 / §五 壳层铃铛 / §六 诚实边界；AC-1…AC-14；R-1…R-14）、`explore.md`（§七 A2 接线细节 / §八 收件箱 UI / §九 词表与首批事件 / §十 文字结论）
> **性质**: **一条新旁路能力 + 一次前端门面收口**——新 core crate（词表 / 扇出点 / Sink 缝）+ 一个 user 维度模型 + 一条新命令轨道（五命令）+ 前端门面（12 处 sonner 直调收口）+ 顶栏铃铛抽屉。零改动 `RunUpdate` / `RunNotice` 线面与 `ChangeFlowControl::publish` 函数体、零新增路由、零新增配置键、零新增外部依赖。
>
> **本轮返工（回溯 1）**: 事件轨的注入面落在 **orchestration**——`RunGuard` 持 `Arc<dyn RunNotifier>`、`walk_run` 增参、`notify::deliver` 是投影与投递的唯一组合点；壳层只提供出口实现（`NotificationBridge`）+ `AskPort::ask_started` 一行转发。成因与形态见 §起点核实 1、§事实校准 6、DD12。
>
> **本轮返工（回溯 2）**: 只改 §接线自检的判据措辞与 `tasks.md` 阶段十六的自查行（另附判据收窄说明；阶段十六的 `use` 组按 DD12 的 `notify::deliver` 调用形态对齐）——DD1–DD12、变更清单、AC 落点零改动。病灶是旧判据把**合法命中面**读成红：① `retire` 按 DD12 必须落在壳层出口面（`commands/notifications/mod.rs` 的 `NotificationBus::retire_run:79-80` + `NotificationBridge::retire`），且 `commands/exec/mod.rs:187` / `:322` 早已匹配执行流变体 `KernelOutput::TurnFinished` / `SessionStreamItem::Finished`；② `Hub` 按 DD1 就定义在 `crates/core/notification/src/lib.rs`（19 处命中）。收窄口径见 §接线自检。
>
> **本篇边界**: `crates/core/notification`、`crates/core/orchestration/src/{notify,control,walker}.rs`、`crates/infra/store`（模型 / 信封 / 六方法 / 落库 sink）、`src-tauri/src/commands/{notifications,change_flow}`、`src-tauri/src/main.rs`、`packages/desktop/src`（门面 / hook / 铃铛 / 六处调用点）、架构模型四文件 + ADR、`package.json` 版本行。测试用例与断言随动归测试相位（见末节交接清单），本篇不写测试策略、不执行静态检查。

---

## 提案与规格同步状态

proposal 的四条不变量、白名单三选三弃、幂等主键、过滤口径与五命令面，以及五份 delta spec 已把语义钉到键名 / 命令签名 / 主键字面 / 判据。本设计不重复论证，只做四件事：

1. **把已钉语义落到具体签名与代码形态**（DD1–DD12）；
2. **对 spec 措辞做工作树校准**（§事实校准 6 条）——其中三条会改动公共面清单（`NotifyContext` 五字段、门面 body 透传面、事件轨注入面从壳层移到 orchestration）；
3. **逐条 AC 给落点**（「验收标准落点」表）；
4. **给下游相位可执行的有序任务**（`tasks.md` 十二阶段 + 末段返工任务）。

---

## 起点核实（工作树实测；函数名为准，行号仅作辅助、随改动漂移）

- **事件轨的真实出口（本轮返工的直接成因）**：`RunUpdate` 六变体的发出面**不全在壳层**。① `ConfirmWait` 由 walker 经 `guard.emit` 发出（`crates/core/orchestration/src/walker.rs` 的两处：stop-before 停止门、逐相位确认块）；② `Finished` 由 `RunGuard::finish` 内的 `self.emit(RunUpdate::Finished { … })` 发出（`crates/core/orchestration/src/control.rs` 的 `finish`）；③ 两者都走 `RunGuard::emit` → `ChangeFlowControl::publish`，**从不进入** `RunEventSink::emit`；④ `RunEventSink::emit` 只由 worker 对 `SessionEvent` 调用（`crates/infra/agent/src/worker.rs` 的 `WorkerAgent::turn` 单点）；⑤ `Ask` 由 agent sdk 的 ask 执行体经 `AskPort::ask_started` 上报（`crates/infra/agent/src/sdk/loop.rs`），壳侧实现收到后自行 `control.publish`。⇒ 壳层两个 hook 点里只有 `AskPort::ask_started` 真实可达；`ConfirmWait` / `Finished` 的合法接线面是 **`RunGuard::emit`**，注入面因此必须在 orchestration 侧（DD12）。
- **壳层旁路接点（Ask 与 Channel 转发）**：`src/commands/change_flow/mod.rs` 的 `ChangeFlowSink` 是同一实例的两 trait 视图（`RunEventSink` 给 worker、`AskPort` 给引擎门面）；`ask_started` 拿到完整 `AskPayload`；同文件的 `spawn_channel_forward`（broadcast → `Channel` 的现成转发形态）是订阅转发先例。改造后该结构的字段为 `root` / `change_id` / `control` / `notifier: Arc<dyn RunNotifier>`（DD12）。
- **run 入口与出口的现状**：`walk_run(worker, tools, vcs, snapshot, history, control, guard, request)` 已 8 参、`RunGuard` 由 `begin_run` 单点铸造且字段全私有（`key` / `control`），`change_flow_start_with` 是 `walk_run` 的唯一生产调用点（`walker_test.rs` 另有 6 处、共 7 处调用）。
- **change 名零额外 IO**：`change_flow_start_with`（`:95`）已 `store.find_change_record(&id)` 读出 `record`（用于 `workflow_type` 与 `worktree` 两道前置校验）；`record.name` / `record.workflow_type` / `run_id` / `store` 四个值全在局部。
- **六变体与主键素材**：`crates/core/orchestration/src/state.rs` 的 `RunUpdate`（`Step` / `Edge` / `SessionEvent` / `Ask{ask}` / `ConfirmWait{phase,archive}` / `Finished{status,reason}`）、`AskPayload{question, options, tool_call_id, session_id}`、`ChangeRunStatus::is_terminal`。
- **相位人话标签的唯一来源**：`crates/core/workflow/src/write/phase_table.rs:18` `PhaseDefinition { id, description, … }`、`:632` `pub fn phase_table(workflow_type: &str) -> Option<&'static [PhaseDefinition]>`。
- **store 双维度与新增模型四个点**：`crates/infra/store/src/store.rs:321` `global_models()`（现有三模型：1 / 5 / 6）、`:341` `workspace_models()`（十二模型）、`:2380` `WorkspaceStores::global()`、`:2386` `global_shared()`；`crates/infra/store/src/envelope.rs:98` `MODEL_ENTRIES`（五指针：`count` / `scan` / `key_of` / `schema` / `walk`）；`crates/infra/store/src/model.rs:1687` `now_millis()`；`crates/infra/store/src/store.rs:202` `MAX_SESSION_PAGE_LIMIT = 500` 的分页先例、`:1256` `upsert_agent_provider` 的「单写事务内读改写」先例。
- **已用 `native_model` id**：1 / 2 / 4 / 5 / 6 / 7 / 8 / 9 / 10 / 11 / 12 / 13 / 15 / 16 / 17（空缺 3 / 14 / ≥18；本变更取 18）。
- **命令登记单点**：`src/commands/mod.rs` 的 `all_commands!`（现 64 条）；`src/bindings/mod_test.rs:127/188` 的 `COMMAND_WRAPPERS` / `COMMAND_NAMES` 各 57 条（注释里的「38 条」是陈旧文案）。
- **壳装配点**：`src/main.rs:101` `setup_hook`，`:104` `WorkspaceStores::open`，`:109` `app.manage(stores)`（`manage` 后不可再取 `WorkspaceStores` 所有权，故 `global_shared()` 必须先取）。
- **前端 12 处直调**：`hooks/use-workspaces.ts:105/120`、`views/agents/hooks/use-agent-instances.ts:90/104/118`、`views/agents/hooks/use-agent-providers.ts:112/126`、`views/changes/hooks/use-change-list.ts:50/52`、`views/architecture/components/contract-panel.tsx:305`（裸 `toast(...)`）、`views/explores/components/explore-open-file.tsx:37/68`。`from 'sonner'` 的生产代码命中 = 上述 6 文件 + `components/ui/sonner.tsx`。
- **页壳与抽屉**：`src/app.tsx:50` `ShellHeader`（内联件，props = `environment` / `update`）、`:115` 挂载点、`:123` `<Toaster />`；`components/standard/drawer.tsx` 的 `title` / `content` / `trigger` / `open` / `onOpenChange` 形态（`views/agent/components/agent-run-history.tsx:130` 为无 trigger 的受控用法先例，`views/db/db-inspector-view.tsx:254` 为单条详情用法）。
- **前端工具面**：`lib/ipc.ts:135` `ipc = new Proxy(commands, …)`（新命令零改动即覆盖）；`import.meta.env.DEV` 的留痕先例 `lib/ipc.ts` 的 `report()`；`@tauri-apps/api/core` 的 `Channel`（`hooks/use-file-watch-signal.ts` / `views/changes/hooks/use-change-flow-run.ts` 两先例）。
- **架构模型读数**（改动前）：`archi_drift` 实读 **55 元素 / 123 关系**、`status = clean`、`apiFindings` 36 条全 `docMissing`（落地后实读 56 / 127、`apiFindings` 37，见 tasks「落地留痕」）；四段表头 `02` 8/9、`03` 8/24、`04` 1/13、`06` 32/74；`06-frontend.toml:8` `App.Frontend.Shell` 的 `path` 四项；ADR `decisions/0001-model-granularity.md:9` 与 `:73-76` 的复算段。
- **版本**：`packages/desktop/package.json` `0.7.5`。

### 事实校准（proposal / spec 措辞 vs 工作树；不改需求意图，只定实现形态）

1. **`NotifyContext` 的三字段不足以铸主键与取相位人话名**。spec Module Contract 写 `NotifyContext`（`root` / `change_id` / `change_name`），但 `Finished` / `ConfirmWait` / `Ask` 的主键一律是 `run:{run_id}:…`、`ConfirmWait` 的 body 需按 `workflow_type` 查相位表——两值都不在 update 载荷里。定稿扩到**五字段**（+ `run_id` / `workflow_type`），五个值全部取自 `change_flow_start_with` 既有局部量（零额外 store 查询，不破 spec 的「零额外 IO」承诺）。归档合并时该表行随正文校正。
2. **「一 sink 失败不影响其他」的落地形态取「失败收场」而非「panic」**。spec 场景写「让第一个在 `deliver` 中 panic **或**以失败收场（落库 sink 的 `Err` 路径）」——定稿取后者：`StoreNotificationSink` 在自身内部把 `Err` 降级成 `eprintln!` 一行。`Hub::publish` **不** `catch_unwind`：仓库既有 sink 纪律明写「回调 panic 不被吞掉、MUST NOT 在 `emit` 内部静默捕获后继续」（`crates/core/orchestration/src/port_test.rs:3826`），通知 sink 不立第二套语义——panic 是实现缺陷而非「失败」。
3. **门面四档的 `body` 透传面不能只挂在 Warning 上**。spec 的四档表把 `{ description }` 只写在 `Warning` 行；但后端事件轨的 `Finished(Failed)` 是 `Error` + `body = reason`，若 Error 档丢 description，失败原因就只能在抽屉里看。定稿：**四档同一形态**——`body` 在场才补 `{ description: body }` 第二实参，缺席不补；由此 12 处既有调用点的实参形态逐字不变（只有 `use-change-list` 的 warning 带 body），19 处精确断言零改动的前提保持不变。
4. **`notification_subscribe` 的「不丢更新」与「不多算」不能同时成立**。spec 钉的顺序（先 receiver、再读计数）保证**不丢**；但订阅建立与计数读取之间的 publish 会既计入初值、又经 Channel 到达 ⇒ 前端未读数可能短时多 1。定稿：**不丢是硬判据**（`sum ≥ 真实未读`），多算窗口登记为已接受的边界；权威收敛面 = 抽屉打开时的 `notification_list` 与 `mark_*` 的返回值。MUST NOT 为消这个窗口加去重表、加第二次计数读或新增查询命令。
5. **`read_at` 出线禁令使「列表置灰」只能是会话本地态**。`Notification` 载荷 MUST NOT 含 `read_at`（spec 硬判据），故前端无法从列表得知历史已读态。定稿：抽屉内的灰态是本会话本地态（`readIds` / `allRead`），徽标与后端读数为权威；**重启后列表项的灰态不恢复**——登记为已知显示边界，MUST NOT 为它把 `read_at` 塞进载荷。
6. **「两个 hook 点」中的 `RunEventSink::emit` 不覆盖 `ConfirmWait` / `Finished`（spec 措辞与工作树相反，本轮返工的根因）**。spec 的「事件白名单与幂等主键」要求行与 Module Contract 的 `ChangeFlowSink` 行把 `RunEventSink::emit` 记作 `ConfirmWait` / `Finished` 的 hook 点（`explore.md` §七 A2 的接线草图同源）；实测该 trait **只**由 worker 对 `SessionEvent` 调用（`crates/infra/agent/src/worker.rs:72`），而 `ConfirmWait`（`walker.rs:292` / `:400`）与 `Finished`（`control.rs:448` 的 `finish`）经 `RunGuard::emit`（`control.rs:350`，转发在 `:351`）→ `ChangeFlowControl::publish` 直落广播面，绕开壳层。定稿：**注入面上移到 orchestration**——`RunGuard` 持 `Arc<dyn RunNotifier>`、`walk_run` 增参并在入口 attach、`notify::deliver` 是「投影 + 投递 + 终态清理」的唯一组合点；壳层保留 `AskPort::ask_started` 一行转发（该缝真实可达）与出口实现 `NotificationBridge`（DD12）。需求语义零变化（三变体 `None` / 三变体 `Some`、同 run 死未读清理、幂等主键逐字、`RunUpdate` / `RunNotice` / `publish` 线面零改动），变的只是「谁在哪个可达的缝上调投影」。该两处 spec 措辞在归档合并时按本校准校正（与校准 1 同机制；本轮不改 spec 正文）。

---

## 关键设计决策（DD1–DD12）

> proposal 的 A2 裁决与 explore §十 的定稿不重复。下表是本设计在 spec 已钉语义之外**新增的形状裁决**——每条都含「为什么另一种写法会坏」。

| # | 议题 | 定稿 | 理由 |
|---|---|---|---|
| DD1 | `core/notification` crate 的公共面与依赖面 | `Cargo.toml` 只 `serde` / `specta`（两行 `{ workspace = true }`，其 crate doc 明写「零 workspace 内部依赖、零 tauri / tokio / native_db」）。公共面五项：`NotificationLevel`（四档）/ `NotificationKind`（`Event` / `Feedback`）/ `Notification`（八字段）/ `NotificationPage { items, total }` / `NotificationFeedbackInput { id, level, message, body }` + `trait Sink { fn deliver(&self, n: &Notification); }` + `Hub { sinks }` 的 `new` / `publish` + `Notification::feedback(input, created_at)` 构造器 | 页信封与反馈入参都属通知词汇（`NotificationPage` 被 store 与壳共用、`NotificationFeedbackInput` 是命令形参），放在词表 crate 才有单一类型真相；`Notification::feedback` 把「kind 恒 Feedback、root / source_ref 恒 None」这条映射从壳移进词表（壳层零域规则）。`toast` 相关语义零出现——core 不认识 sonner |
| DD2 | 投影单点落在 `orchestration::notify` | 新文件 `notify.rs`：`NotifyContext` 五字段（DD 校准 1）+ `pub fn project(update: &RunUpdate, ctx: &NotifyContext, now: i64) -> Option<Notification>`。三变体 `None`、三变体 `Some`；`ConfirmWait` 的 body 走私有 `phase_description(workflow_type, phase) -> Option<String>`（`phase_table(workflow_type)` 内按 `id` 找到该相位并取 `description`）；主键按 spec 字面铸造；文案字面**只在此文件出现一次** | 「哪些时刻值得喊人」是流程领域知识，住引擎侧；壳只喂 `RunUpdate` 与上下文。相位名查表而不另立翻译表——表是唯一权威，找不到就 `None`（不猜测、不回落英文） |
| DD3 | 落库 sink 与 store 六方法 | `store/src/notification.rs`：`impl NotificationRecord { from_notification(&Notification) / to_notification() }` 一比一映射单点 + `StoreNotificationSink { store: Arc<Store> }`。六方法：`publish_notification`（主键 upsert，命中则只改 `message` / `body` / `level`，**保留 `read_at` 与 `created_at`**）、`list_notifications`（只 `Event`，`(created_at desc, id asc)` 稳定序，`limit.min(MAX_NOTIFICATION_PAGE_LIMIT = 200)` 钳位）、`unread_notification_count`、`mark_notifications_read`（只 `Event`、只翻 `None → Some(now)`）、`mark_all_notifications_read`、`retire_run_notifications`（按 `run:{run_id}:confirm:` / `run:{run_id}:ask:` 前缀命中未读行置已读） | 排序必须带次级键：同毫秒落库的两条若只按 `created_at desc` 排，分页边界会不稳定（同一行可能出现在两页 / 一页不出现）；upsert 保留 `created_at` 让「首次事件时刻」稳定。`mark_*` 返回剩余未读，实现为「写事务提交后再读一次计数」——不追求跨两事务的原子快照，返回值的语义就是「此刻还剩几条」。`retire` 用主键前缀而非二级索引，与「主键即幂等串」同源 |
| DD4 | 壳层装配形态：`NotificationBus` 而非裸 `Hub` | `commands/notifications/mod.rs` 定义 `pub struct NotificationBus { store: Arc<Store>, hub: notification::Hub, sender: broadcast::Sender<Notification> }`，`new(store)` 内构造 sender（容量 128）、`Hub::new(vec![StoreNotificationSink, InAppSink])`。方法：`publish(&Notification)`、`retire_run(&str)`（内部 `eprintln!` 降级）、私 `subscribe()` / `store()`。`main.rs` 只多一行 `app.manage(Arc::new(NotificationBus::new(global)))` | proposal 的草图是 `manage(Arc::new(hub))`，但订阅要 `Sender::subscribe()`——Hub 只有 sinks，另 manage 一个 sender 就是两个状态对象、两条「谁在广播」的路径。bus 把「全局库 + 扇出点 + 广播端」收成一个对象，顺带让命令面**只能**碰全局库（`store()` 返回的就是全局库实例），spec 的「收件箱只在全局库」因此是结构性的而非纪律性的 |
| DD5 | 订阅命令的四步顺序与失败面 | `notification_subscribe_with`：① `let mut receiver = bus.subscribe();` ② `let unread = bus.store().unread_notification_count().map_err(…)?;` ③ `spawn` 转发任务 ④ `Ok(unread)`。转发循环写 `Ok(n) => send` / `Err(Lagged(_)) => continue` / `Err(Closed) => break` | ② 失败即整体 `Err`（前端静默、徽标缺席），不返回半成品订阅。`Lagged` 不 `break`——与 `spawn_channel_forward` 的「滞后即断流」不同：那一侧靠统一视图重查兜底，这一侧的权威读数是抽屉打开时的列表，断流会让「铃铛从此不再响」且无自愈面 |
| DD6 | 前端门面的两面与调用形态 | `lib/notify.ts` 导出 `present(level, message, body?)`（只渲染）与 `notify(level, message, body?)`（`present` + fire-and-forget 落库）。渲染用显式 `switch`，四档分别落 `toast(msg)` / `toast.success(msg)` / `toast.warning(msg)` / `toast.error(msg)`，`body !== undefined` 才补第二实参 `{ description: body }`；**不向 sonner 传 `id`**。落库走 `ipc.notificationRecord({ id: crypto.randomUUID(), level, message, body: body ?? null })`，`try/catch` 罩同步的 `IpcArgsError` + `.catch` 罩 reject，DEV 恰一行 `console.warn`，零重试、零再发通知 | 「逐字同形」只能靠显式四臂而不是查表——查表会把 `toast` 当值调用（`this` 语义与 sonner 内部实现耦合）。`body` 的透传面按校准 3 统一到四档。落库必须先于/后于 toast 都无所谓（fire-and-forget），但**异常绝不能穿出**门面：既有 6 个测试文件的 `not.toHaveBeenCalled()` 基线断言依赖调用方零异常 |
| DD7 | `use-notifications.ts` 的通道纪律与状态面 | 单文件收口：`Channel<Notification>` 构造 1 处 + `ipc.notificationSubscribe` 调用 1 处（effect 挂载一次，root 无关——收件箱是 user 维度）。Channel 消息：`level !== 'info'` 才 `present(level, message, body ?? undefined)`；前插 `items`、`unread + 1`、`total + 1`。抽屉打开 → `load()` 拉 `notificationList(0, 50)`。`markRead` / `markAllRead` 以命令返回值回写 `unread`，本地 `readIds`（Set）+ `allRead` 承载灰态。失败分流：订阅静默、`list` / `mark_*` 置 `error`（inline 持久、抽屉不关） | 通知通道与文件失效信号分属不同驱动源（`use-file-watch-signal.ts` 的划界原则），故不并入；`Info` 不弹是机械规则（spec 钉死），故不在组件里按 status 分叉。灰态本地化是校准 5 的直接结果。`PAGE_SIZE = 50` 与 `MAX_NOTIFICATION_PAGE_LIMIT` 脱钩：前者是 UI 首屏，后者是 IPC 滥用兜底 |
| DD8 | 铃铛与抽屉的落位与交互 | 新文件 `src/components/notification-bell.tsx`：`export function NotificationBell(props: { root: string; onSelect: (root: string) => void })`。`Button`（ghost / icon，`Bell` 图标）+ `Badge`（`0` 不渲染 / `≤99` 数字 / `>99` `99+`）+ `StandardDrawer`（`trigger` 承载铃铛按钮、`content` 承载「全部已读」按钮 + 列表 + 空态 + inline 错误）。行点击：先 `markRead([id])`；`sourceRef === null` 即返回；`root !== null && root !== props.root` 时先 `props.onSelect(root)`；随后 `navigate('/changes/' + sourceRef)`。时间列 `new Date(createdAt).toLocaleString()` | 不内联进 `app.tsx`（`max-lines-per-function: 50` 与页壳可读性），代价是模型 `path` 追加一项（DD9）。跨根跳转取 explore §8.5 方案 B（切根后跳转），`sourceRef` 为 null 的行惰性——不猜、不报错。不新增 `lib/format-time.ts`：仓库惯例就是行内 `toLocaleString()` |
| DD9 | 架构模型三处同值的逐文件落笔 | `02`：`Checks` 之后追加元素 `App.Backend.Core.Notification`（`kind = "module"`、`gate = "off"`、`path = ["./packages/desktop/src-tauri/crates/core/notification/"]`、零 `entry` / 零 `api` / 零 `doc`）+ `Orchestration → Notification` 边（排在 `Orchestration` 组内 `Foundation` 之后、`Workflow` 之前）；表头 `8 / 9` → `9 / 10`。`03`：`Store → Core.Notification` 边（`Foundation` 与 `Workflow` 之间）+ 表头 `8 / 24` → `8 / 25`，并把「唯一跨域边 store → core.workflow」改写成「跨域边（store → core.workflow / core.notification）」。`04`：`Shell → Core.Notification` 边（`Foundation` 与 `Orchestration` 之间）+ 表头 `1 / 13` → `1 / 14`。`06`：`Shell` 的 `path` 末尾追加 `./packages/desktop/src/components/notification-bell.tsx` + `Shell → Components.Standard.Drawer` 边（排在 `Shell` 组首，按 `to` 字母序）+ 表头 `32 / 74` → `32 / 75`。ADR `§1` / `§3` / `§5` / `§9` 同批换段（56 / 127；后端 52 = 10+25+14+3；前端 75 = 67+8；容器 9 + 叶 47；`Shell` 出边 13 → 14） | 关系声明的排列在当前四文件里是按 `from` → `to` 字典序，新边必须插进对应位置而非追加到文件尾（否则 diff 位置失序、后续人工核对失效）。元素追加到文件尾（既有顺序不是字典序而是依赖序，`Checks` 是上一轮追加的尾部）。ADR 只换「模型表头 / 本表 / ADR」三处同值口径内的读数——其余 capability spec 里的历史读数不在追改面内 |
| DD10 | 生成物与版本纪律 | `src/types/generated/{bindings.ts,bindings.zod.ts}` 只经 `bindings:export` 再生（勿手改）；`.dev-team/architecture/graph/INDEX.toml` 只经 `api:export` 再生；`packages/desktop/package.json` `0.7.5` → `0.7.6` | 生成物是派生面，手改即「零 diff 门」失败；版本补丁号是 desktop 源改动的硬纪律，`tauri.conf.json` 经 `../package.json` 跟随、`src-tauri/Cargo.toml` 的 crate 版本行不动 |
| DD11 | `store` 内新模块与外部 crate 同名 | 新模块文件 `crates/infra/store/src/notification.rs` 与外部 crate `notification` 同名 ⇒ 本 crate 内引用外部 crate 一律写 `use ::notification::…`（前置 `::` 走 extern prelude）；本地模块走 `crate::notification::…` | 与 `src/commands/change_flow/mod.rs` 的 `use ::agent::…` 同形先例。不走这条会踩 2018 edition 的 uniform-path 歧义（`use notification::Sink` 同时匹配外部 crate 与 crate 内模块） |
| DD12 | **事件轨的注入面（本轮返工）** | ① `notify.rs` 增 `pub trait RunNotifier: Send + Sync { fn context(&self) -> &NotifyContext; fn deliver(&self, notification: &Notification); fn retire(&self); }` 与 `pub fn deliver(update: &RunUpdate, notifier: &dyn RunNotifier, now: i64)`（= `project` → `deliver`；`RunUpdate::Finished` 再 `retire`）。② `control.rs`：`RunGuard` 增私有字段 `notifier: Option<Arc<dyn RunNotifier>>`（`begin_run` 铸 `None`）、`pub(crate) fn with_notifier(self, notifier: Option<Arc<dyn RunNotifier>>) -> Self`；`RunGuard::emit` 在 `control.publish` **之前** `if let Some(notifier) = &self.notifier { notify::deliver(&update, notifier.as_ref(), now_millis()); }`（`publish` 函数体逐字不动）；`StepEmitter` 同携该字段（`emitter()` 克隆、`attach_output_ref` 的合成 guard 带上）。③ `walker.rs`：`walk_run` 尾部追加入参 `notifier: Option<Arc<dyn RunNotifier>>`，函数体第一行 `let guard = guard.with_notifier(notifier);`（在起点 `history.run_started` 之前——fail-fast 终态也要发 `Finished`）。④ 壳层：`commands/notifications::NotificationBridge { context: NotifyContext, bus: Arc<NotificationBus> }` 实现该 trait（`deliver` → `bus.publish`、`retire` → `bus.retire_run(&context.run_id)`、`context` 返回自持上下文）；`commands/change_flow` 的 `AskPort::ask_started` 调同一个 `notify::deliver`，`RunEventSink::emit` 零通知逻辑 | 三个「另一种写法」都有硬缺陷：① **投影留在壳层 `emit`**——`ConfirmWait` / `Finished` 根本到不了该 trait（实测只收 `SessionEvent`），生产链上通知永不发生（本轮返工的根因）；② **注入点放 `ChangeFlowControl::publish`**——control 从「进程内零 IO」变成「每拍可能写库」，违反合并语料的 `ChangeFlowControl` 零 IO 不变量（`openspec/specs/desktop-change-orchestration/spec.md` 明写「store 写经 walker 侧 port / 命令层装配注入」），且 `publish` 的既有用例面全被牵连；③ **`begin_run` 加参**——148 处测试夹具机械随动，而 `walk_run` 是 run 的唯一入口、调用点仅 7 处（生产 1 + 测试 6）。`RunGuard::emit` 是 `ConfirmWait` / `Finished` 的唯一共同出口，旁路挂此处既不碰 `publish`、也不碰 `RunUpdate` / `RunNotice` 线面；旁路在 `publish` 之外发生（不持 `runs` 锁做 IO），`now` 复用 `control::now_millis`（既有 `pub(crate)` 时钟，零新时钟缝），投递是同步写（与 walker 既有 `history.run_started` / `run_finished` 同族，零新线程、零新任务）；投影先过滤 ⇒ `Step` / `Edge` / `SessionEvent` 三变体零出口调用、零 IO |

---

## 接线自检（实现期一眼判据；只读命令，不跑门禁）

本轮返工的病灶是「设计断言了不存在的 hook 点」，故把接线写成可 grep 的判据。**判据只对生产代码生效**（`src/**` 与 `crates/**` 一律排除 `*_test.rs`），且必须绕开两处**合法命中面**——`commands/exec` 的 `KernelOutput::TurnFinished` / `SessionStreamItem::Finished`（执行流终态，DD12 零改动）与 `crates/core/notification` 自定义的 `Hub`（DD1 定义在 `crates/core/notification/src/lib.rs`）。按字面把这两处读成红，等于要求删掉既有执行流终态分支与词表 crate 的扇出点。

- `grep -rn "walk_run(" packages/desktop/src-tauri/src` ⇒ 生产恰一处，且该处传 `Some(`（`None` 只允许出现在测试与无头调用）。
- `grep -rn "deliver(&update" packages/desktop/src-tauri/crates/core/orchestration/src/{control,walker}.rs` ⇒ `control.rs` 的 `RunGuard::emit` 一处（`notify::deliver` 是事件轨投递的唯一组合点）。
- 终态判定不入壳：`grep -rn "RunUpdate::Finished" packages/desktop/src-tauri/src/commands --include=*.rs | grep -v '_test\.rs'` ⇒ 零命中（终态判定与死未读清理判定只在 core 的 `notify::deliver`）；`commands/exec` 的 `KernelOutput::TurnFinished` / `SessionStreamItem::Finished` 不属本判据、逐字不动。
- 终态清理的实现面收口：`grep -rn "retire" packages/desktop/src-tauri/src/commands --include=*.rs | grep -v '_test\.rs'` 的命中集 ⊆ `commands/notifications/mod.rs`（`NotificationBus::retire_run` 一行 store 委托 + `NotificationBridge::retire` 一行转发）；`commands/change_flow/**` 零 `retire` / 零 `retire_run`。
- `grep -rn "from 'sonner'" packages/desktop/src`（排除测试）⇒ 恰两处（`components/ui/sonner.tsx` / `lib/notify.ts`）。
- core 不认壳层扇出对象：`grep -rn "NotificationBus\|Hub" packages/desktop/src-tauri/crates/core --include=*.rs --exclude='*_test.rs' --exclude-dir=notification` ⇒ 零命中（`Hub` 的定义在 `crates/core/notification` 词表 crate 内，本判据排除该目录；`orchestration` 只认 `Notification` 与 `RunNotifier`）。

---

## 架构组件

### 1. 通知词表与扇出点（`crates/core/notification`）

- **职责**：定义「一条通知是什么」与「怎么把它投出去」，不含任何 IO、渠道与业务知识。
- **文件位置**：`crates/core/notification/{Cargo.toml,src/lib.rs}`（新 crate，`src/lib.rs` 是唯一入口）。
- **依赖**：`serde` / `specta`（workspace）；零 tauri / tokio / native_db / workspace 内部 crate。
- **技术**：纯数据类型 + 同步顺序遍历；三条不变量写在 crate doc（`//!`），实现面零 `spawn`、零 `async`。

### 2. 事件投影、投递与注入面（`crates/core/orchestration`）

- **职责**：把 `RunUpdate` 投影成「值得喊人的一条通知」（白名单、level 映射、主键铸造、文案字面全在这一处）；提供**投影 → 投递 → 终态清理**的唯一组合点 `deliver` 与出口 trait `RunNotifier`；并把这套逻辑钉在 `ConfirmWait` / `Finished` 的唯一共同出口上（DD12）。
- **文件位置**：新文件 `src/notify.rs`（`NotifyContext` / `project` / `RunNotifier` / `deliver` / 私有 `phase_description`）+ `src/control.rs`（`RunGuard` / `StepEmitter` 增私有 `notifier` 字段、`with_notifier`、`emit` 一行转发）+ `src/walker.rs`（`walk_run` 尾部追加入参 + 函数体第一行 attach）+ `src/lib.rs`（模块声明与根导出）。
- **依赖**：`notification`（新边）、`workflow::write::phase_table`（既有）、既有 `control::now_millis`（`pub(crate)` 时钟）；`serde` / `specta` 无需新增。
- **技术**：纯函数 `project` / `deliver`（`now` 由调用方注入，测试确定性）；`Option<Arc<dyn RunNotifier>>` 随 `RunGuard` 走——**不落注册表、不碰 `runs` 锁、不碰 `ChangeFlowControl::publish` 函数体**；三层零成本（`project` 先过滤 ⇒ `Step` / `Edge` / `SessionEvent` 连 trait 调用都不发生）。

### 3. 收件箱持久化与落库 sink（`crates/infra/store`）

- **职责**：user 维度收件箱模型、读写方法、db 查看器可发现性，以及 `impl notification::Sink` 的落库适配器。
- **文件位置**：`src/model.rs`（+ 模型）、`src/notification.rs`（新：映射 + sink）、`src/store.rs`（+ `global_models` 一行 + 六方法）、`src/envelope.rs`（+ 注册表一行与三函数）、`src/lib.rs`（+ 模块声明与导出）、`Cargo.toml`（+ `notification`）。
- **依赖**：`notification`（新边）、既有 `native_db` / `native_model` / `serde` / `specta`。
- **技术**：native_db 单写事务内的「读 → 改 → upsert → commit」；`///` doc 派生表与字段说明（`describe.rs` 机械面，零模型特定代码）。

### 4. 通知命令轨道与装配（`src-tauri/src/commands/notifications/`）

- **职责**：五条命令的薄包装、`InAppSink`（broadcast）、`NotificationBus`（全局库 + Hub + 广播端的单一装配对象）、**`NotificationBridge`（`RunNotifier` 的壳侧实现：把 core 的投递转发到 `NotificationBus`）**、broadcast → `Channel` 转发。
- **文件位置**：新文件 `src/commands/notifications/mod.rs`；`src/commands/mod.rs` 增 `pub mod notifications;` 与五条登记；`src/main.rs` 增 `NotificationBus` 装配。
- **依赖**：`notification`、`store`（`StoreNotificationSink` / `Store`）、`orchestration::notify::{NotifyContext, RunNotifier}`、`tauri::ipc::Channel`、`tokio::sync::broadcast`。
- **技术**：同步命令（毫秒级单行 upsert / 分页扫描 / 计数），转发任务走 `tauri::async_runtime::spawn`（与 `spawn_channel_forward` 同族）；bridge 是零状态转发（上下文由 `NotifyContext` 单份持有）。

### 5. change flow sink 接线（`src-tauri/src/commands/change_flow/mod.rs`）

- **职责**：**只剩 Ask 面**——`AskPort::ask_started` 构造 `RunUpdate::Ask`、调 `notify::deliver`、再 `control.publish`；`RunEventSink::emit` 回到「会话槽 + `control.publish`」的纯桥形态（事件轨的 `ConfirmWait` / `Finished` 由 orchestration 侧 `RunGuard::emit` 负责）。
- **文件位置**：同文件（`ChangeFlowSink` 收成四字段：`root` / `change_id` / `control` / `notifier: Arc<dyn RunNotifier>`；`change_flow_start_with` 在 `walk_run` 之前构造 bridge 并两处喂入）。
- **依赖**：`orchestration::notify::{deliver, NotifyContext, RunNotifier}`（经 orchestration 根导出）、`NotificationBridge`、既有 `now_millis`。
- **技术**：既有 `control.publish` 逐字不动；旁路投递在其**之前**（先落库、后广播），失败静默不影响命令返回值。

### 6. 前端通知门面与收件箱（`packages/desktop/src`）

- **职责**：sonner 的唯一持有者（`lib/notify.ts`）、收件箱订阅与状态面（`hooks/use-notifications.ts`）、顶栏铃铛与抽屉（`components/notification-bell.tsx`）。
- **文件位置**：上述三新文件 + `app.tsx`（`ShellHeader` 扩 props）+ 6 个既有调用点。
- **依赖**：`sonner`（既有）、`@tauri-apps/api/core` 的 `Channel`（既有）、`react-router` 的 `useNavigate`（既有）、`lucide-react` 图标（既有）、`@/components/standard/drawer` + `@/components/ui/{button,badge}`（既有）。
- **技术**：React 19 hooks；`Channel` 单实例 + 一次订阅；订阅失败静默、列表失败 inline（沿错误呈现双轨）。

---

## 变更清单

> **覆盖口径**：下列四张子表（新增文件 / 修改文件 / 公共函数 · API / 类型定义）覆盖 proposal「变更范围 - 实现文件」全部条目（后端 15 行 / 前端 10 行 / 架构模型与资产 6 行），并补设计层新增 6 处（`crates/infra/store/src/lib.rs` 的模块登记与导出、`crates/core/orchestration/src/lib.rs` 的模块登记与导出、`package.json` 版本行、`06-frontend.toml` 的 `path` 追加、`crates/core/orchestration/src/{control,walker}.rs` 的注入面）——成因见 DD3 / DD9 / DD10 / DD12。测试文件的期望随动**不属本相位改动**，见末节「交接给下游相位的随动清单」。

### 新增文件

| 文件 | 说明 |
|---|---|
| `packages/desktop/src-tauri/crates/core/notification/Cargo.toml` | 新 core crate 清单：`[package] name = "notification"`、`edition = "2021"`，`[dependencies]` 只 `serde` / `specta`（各 `{ workspace = true }`），crate 级注释写明「禁 Tauri / 禁 tokio / 禁 workspace 内部依赖」 |
| `packages/desktop/src-tauri/crates/core/notification/src/lib.rs` | 词表与扇出点：`NotificationLevel` / `NotificationKind` / `Notification` / `NotificationPage` / `NotificationFeedbackInput` / `Notification::feedback` / `trait Sink` / `Hub`；`//!` 段写三条不变量（旁路 / 不递归 / 只投一次） |
| `packages/desktop/src-tauri/crates/core/orchestration/src/notify.rs` | 投影与投递单点：`NotifyContext`（五字段）+ `project` + `pub trait RunNotifier { context / deliver / retire }` + `pub fn deliver(update, notifier, now)` + 私有 `phase_description`（DD2 / DD12） |
| `packages/desktop/src-tauri/crates/infra/store/src/notification.rs` | `impl NotificationRecord { from_notification / to_notification }` 一比一映射 + `StoreNotificationSink`（`impl ::notification::Sink`，失败 `eprintln!("[notification] …")` 一行降级） |
| `packages/desktop/src-tauri/src/commands/notifications/mod.rs` | 模块 doc（写命令落本轨道的纪律 + 事件轨出口适配器）+ `NotificationBus` + `InAppSink` + **`NotificationBridge`（`RunNotifier` 的壳侧实现）** + 转发任务 + 五命令与五条 `*_with` 测试缝 |
| `packages/desktop/src/lib/notify.ts` | 门面：`present` / `notify`（DD6） |
| `packages/desktop/src/hooks/use-notifications.ts` | 订阅 + 收件箱状态面：`NotificationInbox` / `useNotifications`（DD7） |
| `packages/desktop/src/components/notification-bell.tsx` | 铃铛 + 徽标 + `StandardDrawer` 收件箱 + 跨根跳转（DD8） |

### 修改文件

| 文件 | 具体修改内容 | 说明 |
|---|---|---|
| `packages/desktop/src-tauri/Cargo.toml` | ① `[workspace] members` 在 `crates/core/foundation` 与 `crates/core/orchestration` 之间插 `"crates/core/notification"`；② `[workspace.dependencies]` 同位置插 `notification = { path = "crates/core/notification" }`；③ 根包 `[dependencies]` 在 `orchestration` 相邻处插 `notification = { workspace = true }` | 三条声明缺一即编译失败或模型少边（`members` 缺 ⇒ 不参与构建；根包 `[dependencies]` 缺 ⇒ `Shell → Core.Notification` 这条模型边没有证据） |
| `packages/desktop/src-tauri/crates/core/orchestration/Cargo.toml` | `[dependencies]` 增 `notification = { workspace = true }` | 投影单点的依赖边（模型 `Orchestration → Core.Notification` 的证据） |
| `packages/desktop/src-tauri/crates/infra/store/Cargo.toml` | `[dependencies]` 增 `notification = { workspace = true }` | 记录字段复用词表枚举 + 落库 sink（模型 `Store → Core.Notification` 的证据） |
| `packages/desktop/src-tauri/crates/core/orchestration/src/lib.rs` | `pub mod notify;`（按字母序插在 `decision` 与 `port` 之间）+ `pub use notify::{deliver, project, NotifyContext, RunNotifier};`（插在既有 `pub use` 组内，按字母序） | 模块不登记即不参与编译；根导出供壳 `use orchestration::notify::{deliver, RunNotifier, NotifyContext}` |
| `packages/desktop/src-tauri/crates/core/orchestration/src/control.rs` | ① `RunGuard` 增私有字段 `notifier: Option<Arc<dyn RunNotifier>>`（`begin_run` 铸 `None`）；② `pub(crate) fn with_notifier(self, notifier: Option<Arc<dyn RunNotifier>>) -> Self`（`walk_run` 入口单点 attach）；③ `RunGuard::emit` 在 `self.control.publish(...)` **之前**插一行 `if let Some(notifier) = &self.notifier { notify::deliver(&update, notifier.as_ref(), now_millis()); }`（`publish` 函数体逐字不动）；④ `StepEmitter` 增同名字段，`RunGuard::emitter()` 克隆它，`attach_output_ref` 的合成 guard 携带它 | `ConfirmWait` / `Finished` 的唯一共同出口就在这里（DD12）；`notifier` 不落 `RunEntry`、不碰 `runs` 锁、不改 `ChangeFlowControl` 的零 IO 不变量 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` | `walk_run` 尾部追加入参 `notifier: Option<Arc<dyn RunNotifier>>`；函数体**第一行** `let guard = guard.with_notifier(notifier);`（早于起点 `history.run_started` 写缝） | 放第一行：起点 fail-fast 终态也要发 `Finished` 通知；`walk_run` 是 run 的唯一入口（生产 1 处 / 测试 6 处），注入面因此不可能被绕过 |
| `packages/desktop/src-tauri/crates/infra/store/src/model.rs` | 追加 `NotificationRecord`（九字段 + 每字段 `///` doc；`#[native_model(id = 18, version = 1)]`、`#[native_db]`、`id` 上 `#[primary_key]`） | id 取「现役最大值 + 1」而非补空洞 3 / 14（DD3 说明见「数据模型」节）；字段 doc 是 db 查看器表与字段说明的唯一派生源 |
| `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | ① `global_models()` 增 `models.define::<NotificationRecord>().expect(…)`；② 常量 `MAX_NOTIFICATION_PAGE_LIMIT: u32 = 200`；③ 六方法（`publish_notification` / `list_notifications` / `unread_notification_count` / `mark_notifications_read` / `mark_all_notifications_read` / `retire_run_notifications`） | 只动全局库组；`workspace_models()` 零改动（收件箱 MUST NOT 进 workspace 库） |
| `packages/desktop/src-tauri/crates/infra/store/src/envelope.rs` | ① `MODEL_ENTRIES` 追加 `{ name: "notification", dimension: DbDimension::User, count / scan / key_of / schema / walk }`；② 三个函数 `scan_notifications` / `walk_notifications` / `notification_key`（`json["id"]` 取值） | 注册表是「新模型 = 定义 struct + 登记一行」的落地；缺登记则 db 查看器看不到该表 |
| `packages/desktop/src-tauri/crates/infra/store/src/lib.rs` | ① `mod notification;`（按字母序插在 `model` 与 `query` 之间）；② 导出 `NotificationRecord`（并入 `pub use model::{…}`）与 `StoreNotificationSink`（新 `pub use notification::StoreNotificationSink;`） | 壳与 store 自身测试都要能构造这两件；模块不登记不参与编译 |
| `packages/desktop/src-tauri/src/commands/mod.rs` | ① `pub mod notifications;`（按字母序插在 `fs` 与 `stats` 之间）；② `all_commands!` 尾部按登记序追加五条（`notification_subscribe` / `notification_list` / `notification_mark_read` / `notification_mark_all_read` / `notification_record`） | 既有 64 条零位移；`all_commands!` 是 `generate_handler` 与 specta 的唯一登记面 |
| `packages/desktop/src-tauri/src/commands/change_flow/mod.rs` | ① `ChangeFlowSink` 收成四字段：`root` / `change_id` / `control` / `notifier: Arc<dyn RunNotifier>`（**删去**首轮的 `change_name` / `run_id` / `workflow_type` / `bus` 与私有 `context()`——上下文随 `NotifyContext` 单份住在 bridge 里）；② `RunEventSink::emit` 回到「`SessionEvent` 槽同步 + `control.publish`」的纯桥形态（事件轨的 `ConfirmWait` / `Finished` 归 orchestration，见 DD12）；③ `AskPort::ask_started`：`let update = RunUpdate::Ask { ask };` → `notify::deliver(&update, self.notifier.as_ref(), now_millis());` → `control.publish(...)`（**投影组合不再手写**）；④ `change_flow_start_with`：在 `walk_run` 之前把 5 个既有局部量装成 `NotifyContext` → `let bridge: Arc<dyn RunNotifier> = Arc::new(NotificationBridge::new(context, bus));` → 一份进 sink、一份作 `walk_run` 的新入参；`bus` 取值点从 sink 构造前移到 `begin_run` 之后、sink 构造之前 | `RunUpdate` / `RunNotice` / `control.publish` / `ask_resolved` 零改动；`change_flow_*` 命令签名与返回值零变化（R-9 可断言）。壳内零「按变体过滤」：`RunUpdate::` 只剩 `Ask` 构造与 `SessionEvent` 匹配两处 |
| `packages/desktop/src-tauri/src/main.rs` | `setup_hook`：`let stores = WorkspaceStores::open(...)` 之后、`app.manage(stores)` 之前取 `let global = stores.global_shared();`，随后 `app.manage(Arc::new(NotificationBus::new(global)));`（`use dev_team::commands::notifications::NotificationBus;`） | 顺序是硬约束：`manage(stores)` 之后再取 `global_shared()` 已无所有权（compile error 而非运行期惊喜） |
| `packages/desktop/src/types/generated/bindings.ts` / `bindings.zod.ts` | `bindings:export` 再生成：五命令的 typed 包装 + 出线类型（`Notification` / `NotificationPage` / `NotificationLevel` / `NotificationKind` / `NotificationFeedbackInput`） | **MUST NOT 手改**；diff 只应来自生成 |
| `packages/desktop/src/app.tsx` | ① `ShellHeader` 增 `root: string` / `onSelect: (root: string) => void` 两 props，在更新指示之前挂 `<NotificationBell root={root} onSelect={onSelect} />`；② `App` 的挂载点传 `root={workspaceState.root}` / `onSelect={workspaceState.select}` | `<Toaster />`（`:123`）与 `AppRoutes` 零改动；欢迎态不渲染 header ⇒ 铃铛随壳态挂载（零 `root === null` 分支） |
| `packages/desktop/src/hooks/use-workspaces.ts` | `import { toast } from 'sonner'` → `import { notify } from '../lib/notify'`；`:105` / `:120` 两处 `toast.error(…)` → `notify('error', …)`（文案逐字不变） | 动作失败语义零变化；`error` 态双轨不变 |
| `packages/desktop/src/views/agents/hooks/use-agent-instances.ts` | 同上（`:90` / `:104` / `:118` 三处） | 同上 |
| `packages/desktop/src/views/agents/hooks/use-agent-providers.ts` | 同上（`:112` / `:126` 两处） | 同上 |
| `packages/desktop/src/views/changes/hooks/use-change-list.ts` | `:50` `toast.warning(msg, { description })` → `notify('warning', msg, outcome.warnings.join('\n'))`；`:52` `toast.success(msg)` → `notify('success', msg)` | warning 的 `description` 走门面 `body`，实参形态逐字对位 |
| `packages/desktop/src/views/architecture/components/contract-panel.tsx` | `:305` 裸 `toast(…)` → `notify('info', …)` | 对应门面的 `Info` → 裸 `toast(message)`；该文件的 DOM 文本断言零改动 |
| `packages/desktop/src/views/explores/components/explore-open-file.tsx` | `:37` / `:68` 两处 `toast.error(String(err))` → `notify('error', String(err))` | 同上 |
| `.dev-team/architecture/graph/model/02-backend-core.toml` | 表头 `8 元素 / 9 关系` → `9 元素 / 10 关系`；`Checks` 之后追加 `App.Backend.Core.Notification` 元素；`Orchestration` 组内追加 `→ App.Backend.Core.Notification` 边（DD9） | 新元素零 `entry` / 零 `api` / 零 `doc` / 零 `metadata` |
| `.dev-team/architecture/graph/model/03-backend-infra.toml` | 表头 `8 元素 / 24 关系` → `8 元素 / 25 关系`；第 2 行「唯一跨域边 store → core.workflow」改写为「跨域边（store → core.workflow / core.notification）」；`Store` 组内追加 `→ App.Backend.Core.Notification` 边 | 跨域边不再是「唯一」，措辞必须随动 |
| `.dev-team/architecture/graph/model/04-backend-shell.toml` | 表头 `1 元素 / 13 关系` → `1 元素 / 14 关系`；`Shell` 组内（`Core.Foundation` 与 `Core.Orchestration` 之间）追加 `→ App.Backend.Core.Notification` 边 | 壳是「唯一全依赖汇聚点」，这条边是 `[dependencies]` 的直读结果 |
| `.dev-team/architecture/graph/model/06-frontend.toml` | ① `App.Frontend.Shell` 的 `path` 数组末尾追加 `"./packages/desktop/src/components/notification-bell.tsx"`；② 表头 `32 元素 / 74 关系` → `32 元素 / 75 关系`；③ `Shell` 组首（`Components.Ui` 之前）追加 `→ App.Frontend.Components.Standard.Drawer` 边 | `path` 追加让新文件被归属（否则静默落 `unmatchedFiles` 且不产出边，R-12 的历史病灶）；元素零新增 |
| `.dev-team/architecture/graph/INDEX.toml` | `api:export` 重生成（新元素 + 一条计数器变化） | **MUST NOT 手改** |
| `.dev-team/architecture/decisions/0001-model-granularity.md` | `§1`：`55 元素 / 123 关系` → `56 / 127`、后端 `49（9+24+13+3）` → `52（10+25+14+3）`、前端 `74` → `75`；`§3` 的「基线 55 个元素」→ 56；`§5` 的「`App.Backend.Shell` 出边 13 条」→ 14；`§9` 复算段整段换值（含 `叶 46` → `47`、`前端 74 = 66 + 8` → `75 = 67 + 8`） | 「计数复述三处同值」的第三处；`§8` 的 11 条已知缺口逐字不动（本变更只新增一条**已声明**边） |
| `packages/desktop/package.json` | `version` `0.7.5` → `0.7.6` | `tauri.conf.json` 经 `../package.json` 跟随；`src-tauri/Cargo.toml` 的 crate 版本行**不动** |

### 公共函数 / API

| 标识符 | 所在文件 | 新增/修改 | 完整签名 | 说明 |
|---|---|---|---|---|
| `Hub::new` | `crates/core/notification/src/lib.rs` | 新增 | `pub fn new(sinks: Vec<Arc<dyn Sink + Send + Sync>>) -> Self` | 装配顺序即投递顺序（壳侧固定「先落库 sink、后 in-app sink」） |
| `Hub::publish` | 同上 | 新增 | `pub fn publish(&self, notification: &Notification)` | 顺序遍历 sinks 各投递一次；无返回值、不重投、不 `catch_unwind`（校准 2） |
| `Sink::deliver` | 同上 | 新增（trait 方法） | `fn deliver(&self, notification: &Notification)` | 同步、无返回值、非 `async`、无 `Result`；重试 / 退避 / 批量归实现内部 |
| `Notification::feedback` | 同上 | 新增 | `pub fn feedback(input: NotificationFeedbackInput, created_at: i64) -> Self` | 固定 `kind = Feedback`、`root = None`、`source_ref = None`；壳的命令不再自拼域语义 |
| `NotificationBus::new` | `src-tauri/src/commands/notifications/mod.rs` | 新增 | `pub fn new(store: Arc<Store>) -> Self` | 构造 broadcast（容量 128）+ `Hub::new(vec![StoreNotificationSink, InAppSink])`；`store` 必须是全局库实例 |
| `NotificationBus::publish` | 同上 | 新增 | `pub fn publish(&self, notification: &Notification)` | 委托 `Hub::publish` |
| `NotificationBus::retire_run` | 同上 | 新增 | `pub fn retire_run(&self, run_id: &str)` | 委托 `Store::retire_run_notifications`，失败 `eprintln!` 一行降级（旁路，R-11） |
| `notification_subscribe` | 同上 | 新增 | `pub fn notification_subscribe(app: AppHandle, on_event: Channel<Notification>) -> Result<u32, String>` | Tauri 命令薄包装；实现见 `notification_subscribe_with` |
| `notification_subscribe_with` | 同上 | 新增 | `pub(crate) fn notification_subscribe_with<R: tauri::Runtime>(app: AppHandle<R>, on_event: Channel<Notification>) -> Result<u32, String>` | 四步顺序（DD5）：先取 receiver → 再读未读计数 → 起转发 → 返回计数 |
| `notification_list` | 同上 | 新增 | `pub fn notification_list(app: AppHandle, offset: u32, limit: u32) -> Result<NotificationPage, String>` | 薄包装：`bus.store().list_notifications(offset, limit)` |
| `notification_list_with` | 同上 | 新增 | `pub(crate) fn notification_list_with<R: tauri::Runtime>(app: AppHandle<R>, offset: u32, limit: u32) -> Result<NotificationPage, String>` | 泛型测试缝 |
| `notification_mark_read` | 同上 | 新增 | `pub fn notification_mark_read(app: AppHandle, ids: Vec<String>) -> Result<u32, String>` | 返回剩余未读 |
| `notification_mark_read_with` | 同上 | 新增 | `pub(crate) fn notification_mark_read_with<R: tauri::Runtime>(app: AppHandle<R>, ids: Vec<String>) -> Result<u32, String>` | 泛型测试缝 |
| `notification_mark_all_read` | 同上 | 新增 | `pub fn notification_mark_all_read(app: AppHandle) -> Result<u32, String>` | 恒返回 `0`（全量置已读后的剩余未读） |
| `notification_mark_all_read_with` | 同上 | 新增 | `pub(crate) fn notification_mark_all_read_with<R: tauri::Runtime>(app: AppHandle<R>) -> Result<u32, String>` | 泛型测试缝 |
| `notification_record` | 同上 | 新增 | `pub fn notification_record(app: AppHandle, record: NotificationFeedbackInput) -> Result<(), String>` | 空白 `id` / 空白 `message` 显式 `Err`；`Notification::feedback(record, now_millis())` → `bus.store().publish_notification(…)`；**不广播** |
| `notification_record_with` | 同上 | 新增 | `pub(crate) fn notification_record_with<R: tauri::Runtime>(app: AppHandle<R>, record: NotificationFeedbackInput) -> Result<(), String>` | 泛型测试缝 |
| `project` | `crates/core/orchestration/src/notify.rs` | 新增 | `pub fn project(update: &RunUpdate, ctx: &NotifyContext, now: i64) -> Option<Notification>` | 六变体白名单单点；主键按 spec 字面；相位 body 查相位表 |
| `deliver` | 同上 | 新增 | `pub fn deliver(update: &RunUpdate, notifier: &dyn RunNotifier, now: i64)` | 投影 → 投递的唯一组合点（`project` → `notifier.deliver`；`RunUpdate::Finished` 再 `notifier.retire`）；两个调用者各一行：`RunGuard::emit`、`AskPort::ask_started`（DD12） |
| `RunNotifier::context` | 同上 | 新增（trait 方法） | `fn context(&self) -> &NotifyContext` | 出口自持 run 级投影上下文（五字段只存一份） |
| `RunNotifier::deliver` | 同上 | 新增（trait 方法） | `fn deliver(&self, notification: &Notification)` | 同步、无返回值、非 `async`、无 `Result`；实现内部失败自降级（旁路） |
| `RunNotifier::retire` | 同上 | 新增（trait 方法） | `fn retire(&self)` | 终态收口：同 run 的 `confirm:*` / `ask:*` 未读置已读（幂等、失败静默）；`run_id` 取自自持上下文 |
| `RunGuard::emit` | `crates/core/orchestration/src/control.rs` | 修改 | `pub fn emit(&self, update: RunUpdate)` | 签名与既有语义不变；新增一行旁路转发（有 notifier 时先 `notify::deliver`，再既有 `control.publish`） |
| `walk_run` | `crates/core/orchestration/src/walker.rs` | 修改 | `pub async fn walk_run(worker: Arc<dyn WorkerAgentPort>, tools: Arc<dyn ToolStepPort>, vcs: Arc<dyn ArchiveVcsPort>, snapshot: Arc<dyn WorkflowSnapshotPort>, history: Arc<dyn RunHistoryPort>, control: Arc<ChangeFlowControl>, guard: RunGuard, request: RunRequest, notifier: Option<Arc<dyn RunNotifier>>) -> ChangeRunStatus` | 尾部追加第 9 参（注入面）；函数体第一行 attach 到 guard；`None` = 无通知出口（引擎测试 / 无头调用） |
| `NotificationBridge::new` | `src-tauri/src/commands/notifications/mod.rs` | 新增 | `pub fn new(context: NotifyContext, bus: Arc<NotificationBus>) -> Self` | 壳侧出口：`deliver` → `bus.publish`、`retire` → `bus.retire_run(&context.run_id)`、`context` 返回自持值 |
| `Store::publish_notification` | `crates/infra/store/src/store.rs` | 新增 | `pub fn publish_notification(&self, notification: &Notification) -> Result<(), StoreError>` | 主键 upsert；命中则只更新 `message` / `body` / `level`，保留 `read_at` 与 `created_at` |
| `Store::list_notifications` | 同上 | 新增 | `pub fn list_notifications(&self, offset: u32, limit: u32) -> Result<NotificationPage, StoreError>` | 只 `Event`；`(created_at desc, id asc)`；`limit.min(200)` 钳位；`total` = Event 全量 |
| `Store::unread_notification_count` | 同上 | 新增 | `pub fn unread_notification_count(&self) -> Result<u32, StoreError>` | 只 `Event` 且 `read_at` 为空 |
| `Store::mark_notifications_read` | 同上 | 新增 | `pub fn mark_notifications_read(&self, ids: &[String]) -> Result<u32, StoreError>` | 只翻 `Event` 行的 `None → Some(now)`；未知 id 与 `Feedback` 行忽略；返回剩余未读 |
| `Store::mark_all_notifications_read` | 同上 | 新增 | `pub fn mark_all_notifications_read(&self) -> Result<u32, StoreError>` | 全量 `Event` 未读行置已读（含未加载页）；返回剩余未读（恒 0） |
| `Store::retire_run_notifications` | 同上 | 新增 | `pub fn retire_run_notifications(&self, run_id: &str) -> Result<(), StoreError>` | 同 run 的 `confirm:*` / `ask:*` 未读 `Event` 行置已读；miss 即无操作（幂等） |
| `StoreNotificationSink::new` | `crates/infra/store/src/notification.rs` | 新增 | `pub fn new(store: Arc<Store>) -> Self` | 落库 sink 构造 |
| `NotificationRecord::from_notification` | 同上 | 新增 | `pub fn from_notification(notification: &Notification) -> Self` | 载荷 → 记录（`read_at = None`）；映射单点 |
| `NotificationRecord::to_notification` | 同上 | 新增 | `pub fn to_notification(&self) -> Notification` | 记录 → 载荷（丢弃 `read_at`）；映射单点 |
| `present` | `packages/desktop/src/lib/notify.ts` | 新增 | `export function present(level: NotificationLevel, message: string, body?: string): void` | 只渲染（不落库）；`body` 在场才补 `{ description }`；不传 `id` |
| `notify` | 同上 | 新增 | `export function notify(level: NotificationLevel, message: string, body?: string): void` | `present` + fire-and-forget `ipc.notificationRecord`；失败静默（DEV 一行 `console.warn`） |
| `useNotifications` | `packages/desktop/src/hooks/use-notifications.ts` | 新增 | `export function useNotifications(): NotificationInbox` | 订阅 + 初值未读 + 首屏 50 + 标记已读 + 失败分流（DD7） |
| `NotificationBell` | `packages/desktop/src/components/notification-bell.tsx` | 新增 | `export function NotificationBell(props: { root: string; onSelect: (root: string) => void }): React.JSX.Element` | 铃铛 + 徽标 + 抽屉（DD8） |

> 私有件不入本表：`phase_description`（`notify.rs`）、`scan_notifications` / `walk_notifications` / `notification_key`（`envelope.rs`）、`InAppSink` / 转发任务 / `NotificationBus::{subscribe, store}` / 五条 `*_with` 之外的转发与订阅内部件（`commands/notifications`）、`RunGuard::with_notifier`（`pub(crate)`，`walker` 同 crate 内单点调用）、`record` 内部函数（`notify.ts`）。

### 类型定义

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|---|---|---|---|
| `NotificationLevel` | `crates/core/notification/src/lib.rs` | 新增 | `pub enum NotificationLevel { Info, Success, Warning, Error }`；`#[serde(rename_all = "camelCase")]`（线值 `info` / `success` / `warning` / `error`）+ `Debug` / `Clone` / `Copy` / `PartialEq` / `Eq` / `Serialize` / `Deserialize` / `Type` |
| `NotificationKind` | 同上 | 新增 | `pub enum NotificationKind { Event, Feedback }`（`event` / `feedback`）；**变体序冻结**——`NotificationRecord.kind` 以 serde 编码落库，重排即旧行解码错位 |
| `Notification` | 同上 | 新增 | `pub struct Notification { pub id: String, pub kind: NotificationKind, pub level: NotificationLevel, pub message: String, pub body: Option<String>, pub root: Option<String>, pub source_ref: Option<String>, pub created_at: i64 }`；出线 DTO 亦是它；**不含 `read_at`** |
| `NotificationPage` | 同上 | 新增 | `pub struct NotificationPage { pub items: Vec<Notification>, pub total: u32 }`；`total` = `kind = Event` 全量（非本页长度） |
| `NotificationFeedbackInput` | 同上 | 新增 | `pub struct NotificationFeedbackInput { pub id: String, pub level: NotificationLevel, pub message: String, pub body: Option<String> }`；`notification_record` 的形参（服务端补 `kind` / `root` / `source_ref` / `created_at`） |
| `NotifyContext` | `crates/core/orchestration/src/notify.rs` | 新增 | `pub struct NotifyContext { pub root: String, pub change_id: String, pub change_name: String, pub run_id: String, pub workflow_type: String }`（校准 1）；`Debug` / `Clone` / `PartialEq` / `Eq`，不出线 |
| `RunNotifier` | 同上 | 新增 | `pub trait RunNotifier: Send + Sync { fn context(&self) -> &NotifyContext; fn deliver(&self, notification: &Notification); fn retire(&self); }`（DD12）；挂 `Arc<dyn RunNotifier>` 跨 crate 边界传，core 只认 trait |
| `RunGuard` | `crates/core/orchestration/src/control.rs` | 修改 | `pub struct RunGuard { key: (String, String), control: Arc<ChangeFlowControl>, notifier: Option<Arc<dyn RunNotifier>> }`（**字段全私有**，公开面只多出 `walk_run` 的入参与 `pub(crate) with_notifier`） |
| `StepEmitter` | 同上 | 修改 | `pub struct StepEmitter { key: (String, String), control: Arc<ChangeFlowControl>, notifier: Option<Arc<dyn RunNotifier>> }`（`#[derive(Clone)]` 不变；`attach_output_ref` 的合成 guard 携带同一出口） |
| `NotificationRecord` | `crates/infra/store/src/model.rs` | 新增 | native_model 模型（`id = 18, version = 1`）：九字段（八载荷字段 + `read_at: Option<i64>`），`#[primary_key] id: String`；`Debug` / `Clone` / `PartialEq` / `Eq` / `Serialize` / `Deserialize` / `Type` |
| `NotificationBus` | `src-tauri/src/commands/notifications/mod.rs` | 新增 | `pub struct NotificationBus { store: Arc<Store>, hub: Hub, sender: broadcast::Sender<Notification> }`（字段全私有）；挂 Tauri `State`（`Arc<NotificationBus>`） |
| `NotificationBridge` | 同上 | 新增 | `pub struct NotificationBridge { context: NotifyContext, bus: Arc<NotificationBus> }` + `impl orchestration::notify::RunNotifier`（`deliver` → `bus.publish`；`retire` → `bus.retire_run(&self.context.run_id)`）；零状态转发，run 级一次构造 |
| `InAppSink` | 同上 | 新增（私有） | `struct InAppSink { sender: broadcast::Sender<Notification> }` + `impl Sink`（`deliver` = `let _ = self.sender.send(n.clone())`，无订阅者静默） |
| `StoreNotificationSink` | `crates/infra/store/src/notification.rs` | 新增 | `pub struct StoreNotificationSink { store: Arc<Store> }` + `impl ::notification::Sink`（失败 `eprintln!("[notification] 落库失败（{id}）：{err}")`） |
| `NotificationInbox` | `packages/desktop/src/hooks/use-notifications.ts` | 新增 | `interface NotificationInbox { unread: number; items: Notification[]; total: number; loading: boolean; error: string \| null; readIds: ReadonlySet<string>; allRead: boolean; load: () => void; markRead: (ids: string[]) => Promise<void>; markAllRead: () => Promise<void> }` |

### 配置

<!-- 本变更零配置键改动：`KNOWN_KEYS` 八键、`config.schema.json`、渠道开关与「全局设置」均逐字不动（proposal「不要修改」明列；渠道配置落点留给第二渠道立项时裁决）。故本子表整段省略。 -->

---

## 数据模型

### `NotificationRecord`（user 维度，全局库新增一模型）

| 字段 | 类型 | 语义 | 线名 |
|---|---|---|---|
| `id` | `String`（主键） | 事件 = 幂等串（`run:{run_id}:finished` / `run:{run_id}:confirm:{phase}` / `run:{run_id}:ask:{tool_call_id\|created_at}`）；反馈 = 前端铸造 UUID | `id` |
| `kind` | `NotificationKind` | `Event`（后端事件）/ `Feedback`（前端动作反馈） | `kind` |
| `level` | `NotificationLevel` | 四档呈现级别 | `level` |
| `message` | `String` | 主文案（含变更名，字面唯一真相在 `notify.rs`） | `message` |
| `body` | `Option<String>` | 副文案（sonner `description`）：事件轨按 proposal §三 表（`Ask` 的 question / `ConfirmWait` 的相位说明 / `Failed` 的 reason）；反馈轨随门面 `body`（如 `use-change-list` 的 warning 注记），无则 `None` | `body` |
| `root` | `Option<String>` | 工作区归属字符串（跨库禁 join 惯例）；反馈轨恒 `None` | `root` |
| `source_ref` | `Option<String>` | 触发源（事件 = change id）；反馈轨恒 `None` | `sourceRef` |
| `created_at` | `i64` | 产生时刻（UTC unix 毫秒）；upsert **不覆盖** | `createdAt` |
| `read_at` | `Option<i64>` | 已读时刻（`None` = 未读）；upsert **只保不翻** | `readAt` |

- **落盘归属**：全局库（`global_models()` 第四个模型）。`workspace_models()` 零改动 ⇒ 在 workspace 库实例上调用六方法由 native_db 拒绝（`StoreError::Db`），spec 的维度隔离场景因此是类型事实而非纪律。
- **`native_model` id = 18**：现役已用 1/2/4/5/6/7/8/9/10/11/12/13/15/16/17；取「最大值 + 1」而不是补空洞 3 / 14——空洞无法证明从未被历史模型占用，而上界之外必不冲突。
- **索引面**：零 `#[secondary_key]`。过滤（`kind = Event`）、排序（`created_at desc`）、前缀清理（`run:{run_id}:confirm:*`）全在主键全表扫描上做；收件箱规模为百～千级，索引与修剪同属独立变更（R-2）。
- **关系**：与 `WorkspaceRecord` / `AgentProviderRecord` / `AgentInstanceRecord` 同库无外键；`root` / `source_ref` 是字符串寻址，不做跨库 join。
- **持久化时机**：两条写入点——后端事件经 `StoreNotificationSink`（Hub 扇出的第一个 sink），前端反馈经 `notification_record` 命令直写（不经 Hub、不广播）。事件轨的完整调用链（DD12）：`RunGuard::emit`（`ConfirmWait` / `Finished` / `Ask` 面由 `AskPort::ask_started` 汇入同一个 `notify::deliver`）→ `notify::deliver` → `NotificationBridge::deliver` → `NotificationBus::publish` → `Hub::publish` → `StoreNotificationSink::deliver`（先落库）/ `InAppSink::deliver`（后广播）；`Finished` 尾巴再 `RunNotifier::retire` → `NotificationBus::retire_run`。

### 前端收件箱状态（进程内，不持久化）

| 状态 | 类型 | 生命周期 | 语义 |
|---|---|---|---|
| `unread` | `number` | 订阅返回值 / 每条 Channel 消息 +1 / `mark_*` 返回值覆盖 | 徽标唯一来源（后端读数为权威） |
| `items` / `total` | `Notification[]` / `number` | 抽屉打开时 `notification_list(0, 50)`；Channel 消息前插 | 抽屉列表（仅 `Event`，后端已过滤） |
| `readIds` / `allRead` | `Set<string>` / `boolean` | 本会话 | 列表灰态（校准 5：`read_at` 不出线，重启不恢复） |

---

## 路由 / API 设计

> 零新增路由、零新增侧栏项（收件箱只在顶栏抽屉里，`routes.tsx` 与 `app-sidebar.tsx` 零 diff）。通知的对外面就是 Tauri IPC 命令面：

| 通道 | 名称 | 说明 | 输入 | 输出 | auth |
|---|---|---|---|---|---|
| Tauri IPC | `notification_subscribe` | 订阅后端事件通知（`Channel` 形参；先订阅后读计数） | `onEvent: Channel<Notification>` | `number`（当前未读数） | 本地 IPC，无鉴权；仅壳态前端可达（无后端调用面） |
| Tauri IPC | `notification_list` | 收件箱分页读（倒序 + 页界信封） | `offset: number`、`limit: number`（超 200 钳位） | `NotificationPage`（`items` / `total`） | 本地 IPC，无鉴权 |
| Tauri IPC | `notification_mark_read` | 逐条置已读 | `ids: string[]` | `number`（剩余未读） | 本地 IPC，无鉴权 |
| Tauri IPC | `notification_mark_all_read` | 全量置已读（含未加载页） | 无 | `number`（剩余未读，恒 0） | 本地 IPC，无鉴权 |
| Tauri IPC | `notification_record` | 前端反馈轨落库（不广播） | `record: NotificationFeedbackInput` | `null`（`Result<(), String>`） | 本地 IPC，无鉴权；前端不等待结果 |

- **错误面**：五命令一律 `Result<T, String>`（`ErrorHandlingMode::Throw` ⇒ 生成 `Promise<T>` 的 reject 语义），错误映射 `.to_string()` 单点。
- **`Channel<Notification>` 的语义边界**：只承载 `kind = Event` 的实时增量（`Feedback` 永不进 Hub）；滞后丢增量由抽屉打开时的列表补拉（R-10）。

---

## 依赖

| 类别 | 依赖 | 用途 |
|---|---|---|
| 运行时 · 内部（后端） | `notification` ← `orchestration` / `store` / 壳 | 三条新边，全部 infra/壳 → core，core 内部一条（`source: Cargo.toml [dependencies]`） |
| 运行时 · 外部（后端） | 无新增 | 新 crate 只用既有 workspace 依赖 `serde` / `specta`；`tokio::sync::broadcast` 经既有 `tokio`；`tauri::ipc::Channel` 经既有 `tauri` |
| 运行时 · 外部（前端） | 无新增 | `sonner`（既有）、`@tauri-apps/api` 的 `Channel`（既有）、`react-router`（既有）、`lucide-react`（既有）、`@/components/standard/drawer` + `@/components/ui/{button,badge}`（既有） |
| 构建 / 测试 | 无新增 | 沿用 `cargo test --workspace` / `pnpm -C packages/desktop run check`（`server:check` → `bindings:export` → `api:check` → `client:check`）既有链；不新增 crate 外部依赖、不新增 npm 包 |

---

## 待决问题

1. **未读的「窗口多算」是否可接受**（校准 4）：定稿接受（不丢为硬判据，多算自愈于列表与 `mark_*` 返回值）。若评审要求严格等值，唯一解是让订阅命令把「计数时刻之后入队的事件」从转发流里排除——那需要给每条通知打序号，成本远大于收益。
2. **已读灰态不跨重启恢复**（校准 5）：这是「`read_at` 不出线」的直接代价。若评审要求历史灰态，须改 spec 的载荷形态（加 `read_at` 或另开一条「已读 id 清单」命令），属独立裁决。
3. **收件箱无界增长**（R-2）：本变更零 cap / 零 TTL / 零启动清理；`Feedback` 是主要增长源。修剪属独立变更，MUST NOT 顺手加。
4. **新元素无契约资产**（R-8）：`App.Backend.Core.Notification` 取缺省 `api = declared` 且不写契约 md ⇒ `apiFindings` 恰多一条 `docMissing`（warn，`status` 仍 clean）。契约资产属独立变更，MUST NOT 在本变更新建 `models/Backend/Core/App.Backend.Core.Notification.md`。
5. **`RunNotifier` 是否该迁进 `port.rs`**（DD12）：本轮把它放在 `notify.rs`——投影与投递是同一关注点的两侧，且这样 `port.rs` 的既有 trait 面零 diff。若后续出现第二个「投影消费者 port」，再统一迁入 `port.rs`；本轮迁移只会让 `port.rs` 多一条与通知域耦合的 trait，收益为零。
6. **`walk_run` 的第 9 个位置参数**（DD12）：位置参数已 8 个，再加一个是可读性负债；不改 `RunRequest` 的结构体字段（它 `derive(Debug, Clone, PartialEq, Eq)`，装不下 `dyn` trait），也不引 `RunOptions` 结构体（会把 6 处既有调用点全改一遍且与仓库「位置参数推进器」风格相左）。登记为已知形态债，若将来引入 run 级 options 结构体一并收敛。
7. **`notification_list` 的全表载入排序**：当前规模（百～千级）可接受；若观察到万级，索引 / 修剪 / 增量游标都要独立裁决——本设计只留 `MAX_NOTIFICATION_PAGE_LIMIT` 一处钳位作为 IPC 滥用兜底。
8. **`crypto.randomUUID()` 在测试环境的可用性**：实现面按 spec 用原生 API（WebView2 安全上下文与 Node 20+ 均在位）；若 jsdom 环境缺该函数，由测试相位补桩，**MUST NOT** 在门面里加降级分支（降级分支等于第二套 id 语义）。

---

## 验收标准落点

| AC | 落点 |
|---|---|
| AC-1 词表与扇出形态（依赖面 / 顺序与失败隔离 / `trait Sink` 形态 / 三不变量） | DD1（`Cargo.toml` 两依赖 + crate doc 三不变量）+ DD2（`Hub::publish` 顺序遍历、无 `catch_unwind`，校准 2）+ 组件 1 |
| AC-2 双写入点收口（`from 'sonner'` 恰两处 / 12 处改门面 / 四档形态逐字 / 不传 id） | DD6（显式四臂 + `body` 透传面，校准 3）+ 「修改文件」表的 6 个调用点 + 组件 6 |
| AC-3 门面落库的旁路语义（不抛、不影响 toast、不重试、DEV 一行） | DD6（`try/catch` + `.catch` + `import.meta.env.DEV` 单点）+ 公共面 `present` / `notify` 分工 |
| AC-4 白名单与幂等（三 `None` / 三 `Some` / level 与 message / 主键字面 / 唯一单点） | DD2（`project` 是唯一切点）+ **DD12（`notify::deliver` 是唯一组合点，挂在 `RunGuard::emit`——`ConfirmWait` / `Finished` 的真实出口；壳层 `RunEventSink::emit` 零通知逻辑、`ask_started` 只一行转发）**（校准 6）+ 组件 2 / 5 |
| AC-5 幂等与读状态保真（同主键一行、只更新正文、`read_at` 不复活） | DD3（`publish_notification` 的命中臂只改三字段）+ 数据模型表 |
| AC-6 死未读清理（同 run 等待类置已读、他 run 与 `Feedback` 不受影响） | DD3（`retire_run_notifications` 前缀匹配）+ **DD12（`Finished` 判定落在 core 的 `notify::deliver`，`RunNotifier::retire` 是壳侧机械转发——规则不再散在组合根，R-11 的张力因此消失）** |
| AC-7 收件箱读面口径（只 `Event` / 降序与页界自洽 / `mark_all` 含未加载页并返回 0） | DD3（过滤与排序在 store 单点；`limit.min(200)` 钳位口径在此定夺） |
| AC-8 订阅初值无丢更新（先 receiver 后计数、顺序可断言） | DD5（四步顺序）+ 校准 4（不丢为硬判据，多算窗口登记在案） |
| AC-9 壳层收件箱与跨工作区（徽标三态 / 抽屉交互 / 先切根再跳转 / 惰性行） | DD8（徽标分档、`onSelect` → `navigate` 顺序、`sourceRef === null` 早退）+ 「修改文件」表 `app.tsx` 行 |
| AC-10 通道纪律与失败面（订阅静默 / 列表 inline / 零轮询 / `Channel` 与订阅各一处） | DD7（单文件收口、失败分流）+ DD8（`use-file-watch-signal.ts` 零 diff） |
| AC-11 出线面与命令清单随动（`bindings:export` / `all_commands!` 尾部追加 / 清单断言同 change 随动 / `pnpm check` 四段） | DD10（生成物纪律）+ 「修改文件」表 `commands/mod.rs` 与生成物两行 + 交接清单第 1 项 |
| AC-12 架构模型三处同值（56 / 127；诊断空；`INDEX.toml` 幂等；`apiFindings` 恰多一条 `docMissing`） | DD9（逐文件增量与排列位置）+ 「修改文件」表四文件与 ADR 行 + 待决问题 4。**DD12 的注入面零新增 crate 边 / 零新增元素**（`Shell → Core.Orchestration`、`Orchestration → Core.Notification` 两条边均已在册），故 56 / 127 与 `apiFindings` 37 条不动；`api:export` 本轮幂等（`App.Backend.Core.Orchestration` 与 `App.Backend.Shell` 均无契约资产 ⇒ `INDEX.toml` 零 diff），`bindings:export` 亦然（命令面与出线类型零变化） |
| AC-13 版本与范围纪律（`0.7.5` → `0.7.6`、`tauri.conf.json` 跟随、crate 版本行不动、改动集 ⊆ 范围） | DD10 + 「修改文件」表 `package.json` 行 + 本清单的覆盖口径 |
| AC-14 诚实边界在案（进程不在场 / 持久化真实价值 / OS 通知不在本变更 / ui 反馈不跨窗口） | DD4（反馈不经 Hub、不广播）+ 数据模型（两份写入点）+ `tauri-plugin-notification` 零依赖、`capabilities/**` 零 diff（「不要修改」面） |

---

## 不要修改（实现期红线）

| 对象 | 理由 |
|---|---|
| `crates/core/orchestration/src/state.rs` + `control.rs` 的 `ChangeFlowControl`（含 `publish` 函数体）/ `port.rs` 的既有 trait | `RunUpdate` / `RunNotice` / `ChangeRunStatus` / `AskPayload` / 广播面与 `publish` 逐字不动；控制注册表不持 store 句柄、不落 notifier（DD12 把出口挂在 `RunGuard` 上）；`port.rs` 既有 trait 面零 diff（新 trait 落 `notify.rs`） |
| `crates/core/checks/**` | 覆盖率阈值与报告语义零改动——本轮的测试失败是接线缺陷的派生面，MUST NOT 调阈值「修绿」 |
| `src-tauri/src/commands/db/mod.rs` | db 轨道只读纪律；收件箱写命令落新轨道 |
| `crates/core/config/**` + `config.schema.json` | 零新配置键（渠道开关不在本变更） |
| `src/components/ui/sonner.tsx` / `src/components/standard/**` | `Toaster` 仍是唯一渲染器；`StandardDrawer` 只被消费 |
| `src/routes.tsx` / `src/components/app-sidebar.tsx` | 零新路由、零新侧栏项 |
| `src/lib/ipc.ts` / `src/lib/ipc-drift.ts` | Proxy 形态自动覆盖新命令 |
| `crates/infra/exec/**` / `crates/infra/watch/**` | 零 CLI 渠道；`Channel<Notification>` 不属文件失效信号管辖 |
| `openspec/specs/**` | 除本 change 的 5 份 delta 外零 diff（8 份零 delta 能力见 proposal「能力」章） |
| `.dev-team/architecture/models/**` | 不新建契约资产（新元素缺契约 ⇒ `docMissing` warn，永不纳管；R-8） |
| `capabilities/**` / `tauri-plugin-notification` | OS 通知不在本变更（AC-14） |

---

## 交接给下游相位的随动清单（非本相位改动）

| 文件 | 随动原因（只登记，不写策略） |
|---|---|
| `src-tauri/src/bindings/mod_test.rs` | `COMMAND_WRAPPERS` / `COMMAND_NAMES` 各 +5（57 → 62）、`DTO_TYPES` 补出线类型、尾部追加序锚点随动（**本轮 DD12 零新命令 ⇒ 该文件不再有第二轮随动**） |
| `src-tauri/src/commands/change_flow/mod_test.rs` | ① `app_with` 的 `manage` 组补 `Arc<NotificationBus>`（`NotificationBus::new(stores.global_shared())`，须在 `manage(stores)` 之前取）；② **11 处 `ChangeFlowSink { … }` 字面构造随 DD12 收成四字段**（`root` / `change_id` / `control` / `notifier`，用 `Arc<NotificationBridge>` 或测试替身）；③ 既有「`sink.emit(ConfirmWait)` → 库一行」一类用例**必须重锚**——`RunEventSink::emit` 已不承载事件轨（校准 6），改断言 `notify::deliver` / `RunGuard::emit` 链路；④ `ask_started` 面用例保留（该缝真实可达） |
| `crates/core/orchestration/src/walker_test.rs` | 6 处 `walk_run(` 调用补第 9 参（无通知出口时传 `None`）、`probe_guard` 的 `RunGuard` 字面构造补 `notifier: None` |
| `crates/core/orchestration/src/control_test.rs` | **零随动**（`begin_run` 签名未变；DD12 的注入走 `walk_run` 入参）——本轮明确不碰该文件 |
| `crates/core/orchestration/src/notify_test.rs` | 既有投影用例不动；新增 `deliver` 组合点用例（`Finished` 触发 `retire`）与 `RunNotifier` 测试替身 |
| `src-tauri/src/commands/notifications/mod_test.rs` | 首轮已建（五命令薄包装、`*_with` 测试缝、订阅顺序不变量）；**本轮补 `NotificationBridge` 的 `deliver` / `retire` 转发两例** |
| `crates/core/notification/src/lib_test.rs` | 新建：扇出顺序与失败隔离、空 sinks、出线形态 |
| `crates/infra/store/src/{model,store,envelope}_test.rs` | 新模型往返、六方法、`MODEL_ENTRIES` 与全局库分组随动 |
| `packages/desktop/src/{lib/notify,hooks/use-notifications,components/notification-bell}.test.ts(x)` | 新建（门面四档、hook 状态面、铃铛三态与跨根跳转） |
| `packages/desktop/src/app.test.tsx` | 桩 `notification_subscribe` / `notification_list` / `notification_mark_*`；`commandsExceptEnvironment()` 一类命令序列断言与调用计数断言随动 |
| 6 个既有 sonner 桩测试文件 | 原则上零改动（门面调用形态不变）；锁了 `invoke` 调用集合的断言需局部收窄（`notification_record` 新增一次 invoke） |
| `openspec/changes/backend-notification-hub/test-design.md` | **重定接线断言锚点**：`ChangeFlowSink::emit()` 那组行（现 7 行）按校准 6 / DD12 改锚到 `RunGuard::emit` → `notify::deliver` → `NotificationBridge`（`ask_started` 的 3 行仍锚原缝，其中 `ask_resolved` 那行零改动）；并补一条**端到端接线用例**（真实 `walk_run` 链或 `RunGuard::emit` + spy `RunNotifier` ⇒ `Store::publish_notification` 落行），使「投影有单测、生产链无线」的假绿不再可能 |
