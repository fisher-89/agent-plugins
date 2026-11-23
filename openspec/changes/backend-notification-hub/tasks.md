# 任务: backend-notification-hub

> 覆盖 `proposal.md`「变更范围 - 实现文件」全部条目（后端 15 项 / 前端 10 项 / 架构模型与资产 6 项）+ 设计层新增 6 处（`store/src/lib.rs` 模块登记与导出、`orchestration/src/lib.rs` 模块登记与导出、`06-frontend.toml` 的 `path` 追加、`package.json` 版本行、`orchestration/src/{control,walker}.rs` 的注入面）——逐条登记在 design「变更清单」，成因见 design DD3 / DD9 / DD10 / DD12 与「事实校准」1 / 3 / 6。
>
> **本相位只写实现，不改测试用例与断言、不执行任何静态检查 / 测试回归**：新增用例与既有断言随动归 test-design / test-gen / test-execution 相位——随动清单见 design 末节「交接给下游相位的随动清单」。
>
> **依赖序（十二阶段 + 返工段五阶段）**：
>
> - **阶段一**（`core/notification` crate）是一切的前置：词表类型与 `Sink` / `Hub` 是投影、落库、壳装配的公共词汇。
> - **阶段二**（`orchestration::notify` 投影单点）依赖阶段一（`Notification` / `NotificationLevel` / `NotificationKind`）。
> - **阶段三**（store 落库面）依赖阶段一（记录字段直接用词表枚举 + 落库 sink 实现 `Sink`）；与阶段二并列。
> - **阶段四**（壳通知命令轨道与装配）依赖阶段一、三（`StoreNotificationSink` 与六个 Store 方法）；`NotificationBus` 是阶段五的注入点。
> - **阶段五**（change flow sink 接线）依赖阶段二（投影）与阶段四（`NotificationBus`），并与阶段四构成**同一编译单元**——`NotificationBus` 一旦被 `change_flow` 引用，`app_with` 的托管面才完整。**（本阶段的接线形态已被返工段「阶段十三–十七」取代，见末节「取代关系」）**
> - **阶段六**（绑定再生成）依赖阶段四（命令面已定）。
> - **阶段七**（前端门面）依赖阶段六（`notificationRecord` 与 `NotificationLevel` 已出线）。
> - **阶段八**（12 处调用点收口）依赖阶段七。
> - **阶段九**（`use-notifications`）依赖阶段六、七（列表命令 + `present`）。
> - **阶段十**（铃铛与页壳）依赖阶段九。
> - **阶段十一**（架构模型资产）独立于代码，但 `Shell → Components.Standard.Drawer` 这条边的证据（`notification-bell.tsx` 的 import 行）须在阶段十之后核对。
> - **阶段十二**（版本 + 边界自查 + 交接）依赖全部。
> - **返工段 阶段十三**（core 出口词汇与组合点）无前置，只动 `notify.rs` / `lib.rs`。
> - **返工段 阶段十四**（注入面）依赖阶段十三；只动 `control.rs` / `walker.rs`。
> - **返工段 阶段十五**（壳侧出口实现）依赖阶段十三、十四；只动 `commands/notifications`。
> - **返工段 阶段十六**（change flow 收口 + 夹具随动）依赖阶段十四、十五；与阶段十五是同一编译单元。
> - **返工段 阶段十七**（生成物 / 模型 / 版本核对与交接）依赖全部。
> - **回溯 2**（判据收窄）零新增任务、零依赖：只改阶段十六自查那一条的措辞（详见末节「返工段」的判据收窄说明与 design §接线自检）。
>
> **关键约束（贯穿全部任务）**：
> ① **core 零基础设施**：`crates/core/notification` 的 `Cargo.toml` 只 `serde` / `specta`；源码零 `tokio` / 零 `tauri` / 零 `native_db` / 零 `async fn deliver` / 零 `spawn`。
> ② **投影与投递是唯一单点**：`commands/change_flow` 与 `commands/notifications` 内 MUST NOT 出现「按 `RunUpdate` 变体 `if` 过滤」；三变体排除与 `Finished` 终态清理只写在 `notify.rs`（`project` / `deliver`）。
> ③ **通知是旁路**：任何 sink 失败只 `eprintln!` 一行；`change_flow_*` 的命令签名、返回值与错误文案零变化；`MUST NOT` 为落库失败再发一条通知。
> ④ **逐字不动面**：`RunUpdate` / `RunNotice` / `ChangeRunStatus` / `AskPayload` / `ChangeFlowControl::publish` 函数体 / `ask_resolved`、`workspace_models()`、`commands/db/**`、`KNOWN_KEYS` 与 `config.schema.json`、`components/ui/sonner.tsx`、`components/standard/**`、`routes.tsx`、`app-sidebar.tsx`、`lib/ipc.ts`、`lib/ipc-drift.ts`、`hooks/use-file-watch-signal.ts`、`capabilities/**`、`crates/core/checks/**`（覆盖率阈值零调整——本轮的测试失败是接线缺陷的派生面）。
> ⑤ **生成物不手改**：`src/types/generated/**` 只经 `bindings:export`；`.dev-team/architecture/graph/INDEX.toml` 只经 `api:export`。
> ⑥ **门禁分工**：`pnpm -C packages/desktop run check`（`server:check` → `bindings:export` → `api:check` → `client:check`）、`run test` 与 `cargo test --workspace` 由桌面端 static-check / test-execution 相位执行，本相位只做源码级自查（`grep` 零命中、读码复核、`git status --porcelain` 边界比对）。

## 阶段一：`core/notification` 新 crate（词表 + 扇出点）

- [x] `packages/desktop/src-tauri/crates/core/notification/Cargo.toml`：新建。`[package] name = "notification"` / `version = "0.1.0"` / `edition = "2021"`；`[dependencies]` 恰两行 `serde = { workspace = true }`、`specta = { workspace = true }`；`[package]` 上方写 crate 级注释（「边界中立通知词汇：禁 Tauri / 禁 tokio 运行时 / 禁 workspace 内部依赖 / 不认识 sonner」）。
- [x] `packages/desktop/src-tauri/Cargo.toml`：① `[workspace] members` 在 `"crates/core/foundation"` 与 `"crates/core/orchestration"` 之间插 `"crates/core/notification"`；② `[workspace.dependencies]` 在同名字母序位置插 `notification = { path = "crates/core/notification" }`；③ 根包 `[dependencies]` 在 `orchestration = { workspace = true }` 相邻处插 `notification = { workspace = true }`（注释一行：通知命令面与事件投影的装配根）。
- [x] `packages/desktop/src-tauri/crates/core/notification/src/lib.rs`：新建，按此序落笔：
  1. `//!` 模块 doc：**三条不变量**（① 一 sink 失败 MUST NOT 影响其他 sink、MUST NOT 让业务动作失败——通知是旁路；② 通知 MUST NOT 为自己的失败再发通知（递归禁止）；③ 只保证投递一次，重试 / 退避 / 批量归 sink 内部，同 sink 内 FIFO、跨 sink 不保证顺序）+ 「本 crate 不认识任何渠道，渠道是 `Sink` 的实现」一句。
  2. `pub enum NotificationLevel { Info, Success, Warning, Error }`（`#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]` + `#[serde(rename_all = "camelCase")]`；每变体一行 `///`：对位 sonner 四档）。
  3. `pub enum NotificationKind { Event, Feedback }`（同上 derive / rename；doc 写明**变体序冻结**——落库以 serde 编码，重排即旧行解码错位）。
  4. `pub struct Notification { id, kind, level, message, body: Option<String>, root: Option<String>, source_ref: Option<String>, created_at: i64 }`（derive 同上；每字段 `///` doc；结构体 doc 写明「出线 DTO 亦是它，载荷全量、不含读状态」）。
  5. `impl Notification { pub fn feedback(input: NotificationFeedbackInput, created_at: i64) -> Self }`：`kind = Feedback`、`root = None`、`source_ref = None`、其余字段搬自入参。
  6. `pub struct NotificationPage { pub items: Vec<Notification>, pub total: u32 }`（derive 同上；`total` doc 写明「`kind = Event` 全量，非本页长度」）。
  7. `pub struct NotificationFeedbackInput { id: String, level: NotificationLevel, message: String, body: Option<String> }`（derive 同上；doc 写明「服务端补 `kind` / `root` / `source_ref` / `created_at`」）。
  8. `pub trait Sink { fn deliver(&self, notification: &Notification); }`（doc：同步、无返回值、非 async、无 `Result`）。
  9. `pub struct Hub { sinks: Vec<Arc<dyn Sink + Send + Sync>> }` + `pub fn new(sinks: Vec<Arc<dyn Sink + Send + Sync>>) -> Self` + `pub fn publish(&self, notification: &Notification)`（顺序遍历、逐 sink 一次、**不** `catch_unwind`——design 校准 2 写进 doc）。
  10. `#[cfg(test)] mod lib_test;` 声明行**留待测试相位**追加（本相位不加）。

## 阶段二：`orchestration::notify` 投影单点

- [x] `packages/desktop/src-tauri/crates/core/orchestration/Cargo.toml`：`[dependencies]` 增 `notification = { workspace = true }`（按字母序插在 `foundation` 与 `serde` 之间）。
- [x] `packages/desktop/src-tauri/crates/core/orchestration/src/notify.rs`：新建：
  - `pub struct NotifyContext { pub root: String, pub change_id: String, pub change_name: String, pub run_id: String, pub workflow_type: String }`（`#[derive(Debug, Clone, PartialEq, Eq)]`，不出线；doc 写明五个值全部由发起侧已读出的 `ChangeRecord` / run 局部量携带，零额外 store 查询）。
  - `pub fn project(update: &RunUpdate, ctx: &NotifyContext, now: i64) -> Option<Notification>`：`Step` / `Edge` / `SessionEvent` → `None`；`Ask` / `ConfirmWait` / `Finished` → `Some`；`Finished` 的非终态 `status` 防御性 `None`。
  - 主键按 spec 字面铸造：`format!("run:{}:finished", ctx.run_id)` / `format!("run:{}:confirm:{}", ctx.run_id, phase)` / `format!("run:{}:ask:{}", ctx.run_id, ask.tool_call_id.as_deref().unwrap_or(&now.to_string()))`（`tool_call_id` 缺位退化用 `now`，即 `created_at`）。
  - level 映射与文案字面（本文件是字面唯一真相）：`Ask` → `Warning` + `变更「{name}」在等你的回答` + body `Some(ask.question.clone())`（选项**不进** body）；`ConfirmWait` → `Warning` + `变更「{name}」停在 {phase} 前等你确认` + body `phase_description(&ctx.workflow_type, phase)`；`Finished(Completed)` → `Success` + `变更「{name}」已完成全部相位` + `None`；`Finished(Failed)` → `Error` + `变更「{name}」运行失败` + `reason`；`Finished(Stopped)` → `Info` + `变更「{name}」已停止` + `None`。
  - 私有 `fn phase_description(workflow_type: &str, phase: &str) -> Option<String>`：`phase_table(workflow_type)?.iter().find(|definition| definition.id == phase).map(|definition| definition.description.to_owned())`（找不得到即 `None`，MUST NOT 另立翻译表）。
  - 三条恒定值：`kind = NotificationKind::Event`、`root = Some(ctx.root.clone())`、`source_ref = Some(ctx.change_id.clone())`。
- [x] `packages/desktop/src-tauri/crates/core/orchestration/src/lib.rs`：`pub mod notify;`（按字母序插在 `pub mod decision;` 与 `pub mod port;` 之间）+ `pub use notify::{project, NotifyContext};`（插在既有 `pub use` 组内，按字母序）。

## 阶段三：store 落库面（模型 / 映射 / 六方法 / 信封）

- [x] `packages/desktop/src-tauri/crates/infra/store/Cargo.toml`：`[dependencies]` 增 `notification = { workspace = true }`（按字母序插在 `native_model` 与 `serde` 之间）。
- [x] `packages/desktop/src-tauri/crates/infra/store/src/model.rs`：追加 `NotificationRecord`（插在 `AgentInstanceRecord` 定义块之后、`SerdeJsonCodec` 实现段之前——与既有模型定义区集中）：
  - `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]` + `#[serde(rename_all = "camelCase")]` + `#[native_model(id = 18, version = 1)]` + `#[native_db]`。
  - 九字段与每字段 `///` doc（表与字段说明的唯一派生源）：`#[primary_key] id: String` / `kind: NotificationKind` / `level: NotificationLevel` / `message: String` / `body: Option<String>` / `root: Option<String>` / `source_ref: Option<String>` / `created_at: i64` / `read_at: Option<i64>`。
  - 结构体 doc：落全局库（user 维度）、两 kind 共用一张表、`read_at` 不进载荷、id 取「现役最大值 + 1」而非补空洞（id 3 / 14）的原因。
  - `use ::notification::{NotificationKind, NotificationLevel};`（design DD11：外部 crate 走前置 `::`）。
- [x] `packages/desktop/src-tauri/crates/infra/store/src/notification.rs`：新建：
  - `impl NotificationRecord { pub fn from_notification(notification: &Notification) -> Self /* read_at: None */ ; pub fn to_notification(&self) -> Notification /* 丢弃 read_at */ }`（一比一映射单点，MUST NOT 在 store 方法内手抄字段）。
  - `pub struct StoreNotificationSink { store: Arc<Store> }` + `pub fn new(store: Arc<Store>) -> Self` + `impl ::notification::Sink for StoreNotificationSink { fn deliver(&self, notification: &Notification) { if let Err(error) = self.store.publish_notification(notification) { eprintln!("[notification] 落库失败（{}）：{error}", notification.id); } } }`（旁路降级，MUST NOT 上抛）。
- [x] `packages/desktop/src-tauri/crates/infra/store/src/store.rs`：
  - `global_models()` 追加 `models.define::<NotificationRecord>().expect("定义 NotificationRecord 失败");`（`workspace_models()` 零改动）。
  - 常量 `const MAX_NOTIFICATION_PAGE_LIMIT: u32 = 200;`（doc：IPC 滥用兜底，前端恒传页宽 50，超上限截断）。
  - 六方法（doc 各写清口径）：
    1. `pub fn publish_notification(&self, notification: &Notification) -> Result<(), StoreError>`：单写事务内按主键读旧行；命中 → 只覆盖 `message` / `body` / `level`，**保留** `read_at` 与 `created_at`；未命中 → `NotificationRecord::from_notification` 全量插入；`commit()`。
    2. `pub fn list_notifications(&self, offset: u32, limit: u32) -> Result<NotificationPage, StoreError>`：读事务内扫全表 → 只留 `kind == Event` → 按 `(created_at desc, id asc)` 稳定排序 → `total = 过滤集长度` → `skip(offset).take(limit.min(MAX_NOTIFICATION_PAGE_LIMIT))`。
    3. `pub fn unread_notification_count(&self) -> Result<u32, StoreError>`：只数 `kind == Event && read_at.is_none()`。
    4. `pub fn mark_notifications_read(&self, ids: &[String]) -> Result<u32, StoreError>`：单写事务内按 id 集合扫（`HashSet<&str>`），只翻 `kind == Event && read_at.is_none()` 的行到 `Some(now_millis())`，未知 id 与 `Feedback` 行忽略；`commit()` 后调 `unread_notification_count()` 返回剩余未读。
    5. `pub fn mark_all_notifications_read(&self) -> Result<u32, StoreError>`：同上一事务内全量翻未读 `Event` 行；`commit()` 后返回 `unread_notification_count()`（恒 0）。
    6. `pub fn retire_run_notifications(&self, run_id: &str) -> Result<(), StoreError>`：单写事务内按前缀 `run:{run_id}:confirm:` / `run:{run_id}:ask:` 命中未读 `Event` 行置 `Some(now_millis())`；miss 即零写入（幂等）。
  - 顶部 `use ::notification::{Notification, NotificationPage};`；`crate::model::` 既有 `use` 组补 `NotificationRecord`（store.rs 本身零映射逻辑，映射只走 `crate::notification`）。
- [x] `packages/desktop/src-tauri/crates/infra/store/src/envelope.rs`：① `MODEL_ENTRIES` 在 `agent_instance` 行之后追加 `ModelEntry { name: "notification", dimension: DbDimension::User, count: count_model::<NotificationRecord>, scan: scan_notifications, key_of: notification_key, schema: describe::schema_of::<NotificationRecord>, walk: walk_notifications }`；② 三个函数（与同族函数相邻）：`scan_notifications` / `walk_notifications`（各一行委托 `scan_model::<NotificationRecord>` / `walk_model::<NotificationRecord>`）+ `notification_key`（`Value` 对象取 `"id"` 字段，与 `workspace_key` 同形）；③ 顶部 `use crate::model::{…, NotificationRecord};`。
- [x] `packages/desktop/src-tauri/crates/infra/store/src/lib.rs`：① `mod notification;`（按字母序插在 `mod model;` 与 `mod query;` 之间）；② `pub use model::{…, NotificationRecord, …}`（并入既有 `pub use model::` 组，保持字母序）；③ 新增 `pub use notification::StoreNotificationSink;`。

## 阶段四：壳通知命令轨道与装配

- [x] `packages/desktop/src-tauri/src/commands/notifications/mod.rs`：新建：
  - 模块 doc：写命令落本轨道的纪律（通知是旁路业务面，MUST NOT 并入只读的 `commands/db/`）；收件箱只碰全局库（`NotificationBus` 只持全局库实例，维度隔离是结构性的）；`Channel<Notification>` 只承载 `kind = Event` 的实时增量、`Feedback` 永不广播。
  - `const NOTIFICATION_CAPACITY: usize = 128;`（doc：事件稀疏，溢出即丢增量，权威读数在 `notification_list`）。
  - 私有 `fn now_millis() -> i64`（与 `commands/change_flow/mod.rs:36` 同形：`SystemTime` 距 epoch 毫秒，早于 epoch 取 0、不 panic）——`Notification::feedback` 的 `created_at` 铸造点。
  - `pub struct NotificationBus { store: Arc<Store>, hub: Hub, sender: broadcast::Sender<Notification> }`：
    - `pub fn new(store: Arc<Store>) -> Self`：`broadcast::channel(NOTIFICATION_CAPACITY)` → `Hub::new(vec![Arc::new(StoreNotificationSink::new(Arc::clone(&store))), Arc::new(InAppSink { sender: sender.clone() })])`（**先落库 sink、后 in-app sink**——行在 toast 之前落库）。
    - `pub fn publish(&self, notification: &Notification)` → `self.hub.publish(notification)`。
    - `pub fn retire_run(&self, run_id: &str)` → `if let Err(error) = self.store.retire_run_notifications(run_id) { eprintln!("[notification] 死未读清理失败（{run_id}）：{error}"); }`（失败静默，R-11）。
    - `fn subscribe(&self) -> broadcast::Receiver<Notification>` / `fn store(&self) -> &Store`（私有，子模块测试可达）。
  - `struct InAppSink { sender: broadcast::Sender<Notification> }` + `impl Sink`：`deliver` = `let _ = self.sender.send(notification.clone());`（无订阅者静默）。
  - 私有 `fn spawn_notification_forward(on_event: Channel<Notification>, mut receiver: broadcast::Receiver<Notification>)`：`tauri::async_runtime::spawn` 内 `loop { match receiver.recv().await { Ok(n) => if on_event.send(n).is_err() { break }, Err(Lagged(_)) => continue, Err(Closed) => break } }`（`Lagged` 不 `break`，design DD5）。
  - 五条命令 + 五条 `*_with` 测试缝（`pub(crate)`、`R: tauri::Runtime`），签名与语义见 design「公共函数」表；`notification_subscribe_with` 的四步顺序（receiver → 计数 → 转发 → 返回）逐行落笔并写注释说明顺序不动点。
  - `notification_record_with`：空白 `id` / 空白 `message` 先显式 `Err`（「空白入参不进入库链路」），再 `Notification::feedback(record, now_millis())` → `store().publish_notification(...)`，**不广播**。
  - `use ::notification::{Hub, Notification, NotificationFeedbackInput, NotificationPage, Sink};` + `use store::{Store, StoreNotificationSink};`（外部 crate 前置 `::`，design DD11）。
  - `#[cfg(test)] mod mod_test;` 声明行留待测试相位。
- [x] `packages/desktop/src-tauri/src/commands/mod.rs`：① `pub mod notifications;`（按字母序插在 `pub mod fs;` 与 `pub mod stats;` 之间）；② `all_commands!` 宏体尾部按登记序追加 `$crate::commands::notifications::notification_subscribe` / `notification_list` / `notification_mark_read` / `notification_mark_all_read` / `notification_record`（既有 64 条零位移）。
- [x] `packages/desktop/src-tauri/src/main.rs`：`setup_hook` 内 `let stores = WorkspaceStores::open(&data_root)?;` 之后**立即** `let global = stores.global_shared();`（在 `app.manage(stores)` 之前），随后 `app.manage(Arc::new(NotificationBus::new(global)));` 并补一行注释（「通知扇出点：全局库 + `Hub` + broadcast 的单对象；必须先取 `global_shared` 再 `manage(stores)`」）；顶部 `use commands::notifications::NotificationBus;`（按既有 import 组风格）。

## 阶段五：change flow sink 接线

> **已被取代**：本阶段的事件轨接线形态（`ChangeFlowSink` 增 `change_name` / `run_id` / `workflow_type` / `bus` 四字段，`emit` / `ask_started` 各调一次 `project`）建立在「`RunEventSink::emit` 覆盖 `ConfirmWait` / `Finished`」这一不成立的断言上（实测该 trait 只收 `SessionEvent`，见 design「事实校准」6）。事件轨改由返工段「阶段十三–十七」落在 orchestration 的 `RunGuard::emit`；本阶段只有 `AskPort::ask_started` 的 Ask 面保留（形态改为一行 `notify::deliver` 转发）。下方各项作为首轮记录保留，**不再作为执行依据**。

- [x] `packages/desktop/src-tauri/src/commands/change_flow/mod.rs`：
  - `ChangeFlowSink` 增四字段：`change_name: String` / `run_id: String` / `workflow_type: String` / `bus: Arc<NotificationBus>`（doc：四值来自 `change_flow_start_with` 既有局部量，零额外 store 查询）。
  - 私有 `fn context(&self) -> NotifyContext`：把五字段装配成投影上下文。
  - `RunEventSink::emit`：既有 `SessionEvent` 槽同步与 `control.publish` 逐字保留；在 `control.publish` **之前**算投影 `let notice = project(&update, &self.context(), now_millis());`，`control.publish` 之后 `if let Some(notification) = notice { self.bus.publish(&notification); }`，再 `if matches!(update_status_finished) { self.bus.retire_run(&self.run_id); }`（`Finished` 判定用投影前抓住的 `matches!` 布尔量，避免二次借用）。
  - `AskPort::ask_started`：`let update = RunUpdate::Ask { ask };` → 投影 → `control.publish` → `bus.publish`（与 `emit` 同一投影函数，MUST NOT 另写一份白名单）。
  - `ask_resolved` 逐字不动。
  - `change_flow_start_with` 的 `sink_impl` 构造补齐四值：`change_name: record.name.clone()` / `run_id: run_id.clone()` / `workflow_type: record.workflow_type.clone()` / `bus: Arc::clone(app.state::<Arc<NotificationBus>>().inner())`（在 `begin_run` 之后、`compose_turn` 之前取出）。
  - 顶部 `use orchestration::notify::{project, NotifyContext};` + `use crate::commands::notifications::NotificationBus;`。

## 阶段六：绑定类型再生成

- [x] 运行 `pnpm -C packages/desktop run bindings:export`，确认 `src/types/generated/bindings.ts` 与 `bindings.zod.ts` 的 diff 恰含：五条命令的 typed 包装（`notificationSubscribe` / `notificationList` / `notificationMarkRead` / `notificationMarkAllRead` / `notificationRecord`）+ 出线类型 `Notification` / `NotificationPage` / `NotificationLevel` / `NotificationKind` / `NotificationFeedbackInput`；**MUST NOT 手改生成物**。
- [x] 复核生成物：`Notification` 类型上**没有** `readAt` 字段（spec 硬判据）；`NotificationLevel` 的线值为 `"info" | "success" | "warning" | "error"`（前端门面据它取词）。

## 阶段七：前端门面 `lib/notify.ts`

- [x] `packages/desktop/src/lib/notify.ts`：新建：
  - 文件头 doc：本文件是 sonner 的**唯一**生产持有者（除 `components/ui/sonner.tsx` 的 `Toaster`）；两个面 `present`（只渲染，供后端事件轨——记录已由后端落库）与 `notify`（渲染 + fire-and-forget 落库，供前端动作反馈轨）；**不向 sonner 传 `id`**（前端 id 天生唯一，传 id 只会让既有精确参数断言失效）。
  - `import { toast } from 'sonner';` / `import { ipc } from './ipc';` / `import type { NotificationLevel } from '../types/generated/bindings';`。
  - `export function present(level: NotificationLevel, message: string, body?: string): void`：`switch (level)` 四臂，各落 `toast(message)` / `toast.success(message)` / `toast.warning(message)` / `toast.error(message)`，`body === undefined` 与否决定是否补第二实参 `{ description: body }`（design 校准 3）。
  - `export function notify(level: NotificationLevel, message: string, body?: string): void`：先 `present(...)`，再 `record(level, message, body)`（私有函数）。
  - 私有 `function record(...)`：`try { void ipc.notificationRecord({ id: crypto.randomUUID(), level, message, body: body ?? null }).catch(fail); } catch (cause) { fail(cause); }`；`fail` 只在 `import.meta.env.DEV === true` 时 `console.warn('[notify] 落库失败（旁路，静默）', cause)`（沿 `lib/ipc.ts` 的 `report()` 形状）；零重试、零再发通知、MUST NOT 加 `crypto.randomUUID` 的降级分支。
- [x] 自查：`grep -rn "from 'sonner'" packages/desktop/src --include=*.ts --include=*.tsx | grep -v '\.test\.'` 此时应为三处（`components/ui/sonner.tsx` + `lib/notify.ts` + 尚未收口的六个文件）——阶段八收口后恰两处。

## 阶段八：12 处 sonner 直调收口到门面

- [x] `packages/desktop/src/hooks/use-workspaces.ts`：`import { toast } from 'sonner';` → `import { notify } from '../lib/notify';`；`:105` / `:120` 两处 `toast.error(...)` → `notify('error', ...)`（模板字面量与固定前缀逐字保留）。同文件里描述「直调 toast.error」的注释行随实情改为「经 `lib/notify.ts` 门面」，不改任何语义描述。
- [x] `packages/desktop/src/views/agents/hooks/use-agent-instances.ts`：同上（`:90` / `:104` / `:118` 三处；import 路径与同文件既有的 `'../../../lib/ipc'` 同风格 ⇒ `'../../../lib/notify'`）。
- [x] `packages/desktop/src/views/agents/hooks/use-agent-providers.ts`：同上（`:112` / `:126` 两处；同上相对路径）。
- [x] `packages/desktop/src/views/changes/hooks/use-change-list.ts`：`:50` `toast.warning(msg, { description: outcome.warnings.join('\n') })` → `notify('warning', msg, outcome.warnings.join('\n'))`；`:52` `toast.success(msg)` → `notify('success', msg)`（文案与次序逐字不变）。
- [x] `packages/desktop/src/views/architecture/components/contract-panel.tsx`：`:305` 裸 `toast(...)` → `notify('info', ...)`（对位门面的 `Info` → 裸 `toast(message)`）。
- [x] `packages/desktop/src/views/explores/components/explore-open-file.tsx`：`:37` / `:68` 两处 `toast.error(String(err))` → `notify('error', String(err))`。
- [x] 自查：`grep` 复核六文件零 `sonner` 引用、12 处调用点全部经 `notify(`、文案字符串逐字未改。

## 阶段九：`hooks/use-notifications.ts`（订阅与收件箱状态面）

- [x] `packages/desktop/src/hooks/use-notifications.ts`：新建：
  - 文件头 doc：本文件是 `Channel<Notification>` 构造与 `notification_subscribe` 调用的**唯一**位置（第三类执行流通道，沿 agent 执行流与 change run 通知先例；MUST NOT 并入 `use-file-watch-signal.ts`——按驱动源切分）；零轮询、零 `setInterval`。
  - `const PAGE_SIZE = 50;`（抽屉首屏；与后端 `MAX_NOTIFICATION_PAGE_LIMIT` 脱钩）。
  - `export interface NotificationInbox { unread: number; items: Notification[]; total: number; loading: boolean; error: string | null; readIds: ReadonlySet<string>; allRead: boolean; load: () => void; markRead: (ids: string[]) => Promise<void>; markAllRead: () => Promise<void> }`。
  - `export function useNotifications(): NotificationInbox`：
    - 挂载 effect：`new Channel<Notification>()`，`channel.onmessage = (notification) => { if (notification.level !== 'info') present(notification.level, notification.message, notification.body ?? undefined); setItems(prev => [notification, ...prev]); setUnread(v => v + 1); setTotal(v => v + 1); }`；`ipc.notificationSubscribe(channel).then((value) => { if (alive) setUnread(typeof value === 'number' ? value : 0); }, () => { /* 订阅失败静默 */ });`；cleanup 置 `alive = false`（Channel 无退订 API：卸载后发送失败会让后端转发任务自然退出）。
    - `load()`：`setLoading(true)` + `ipc.notificationList(0, PAGE_SIZE)` → 成功覆盖 `items` / `total`、`setLoading(false)`；失败 `setError(String(err))` + `setLoading(false)`（inline 持久）。
    - `markRead(ids)`：本地先并入 `readIds`，再 `ipc.notificationMarkRead(ids)` → 成功 `setUnread(value)`；失败 `setError(String(err))`。
    - `markAllRead()`：`ipc.notificationMarkAllRead()` → 成功 `setUnread(value)` + `setAllRead(true)`；失败 `setError(String(err))`。
    - 类型导入：`Notification` / `NotificationLevel` 等一律取 `../types/generated/bindings`；`present` 取 `../lib/notify`。

## 阶段十：铃铛与页壳

- [x] `packages/desktop/src/components/notification-bell.tsx`：新建：
  - 文件头 doc：壳态顶栏的收件箱入口（铃铛 + 徽标 + 抽屉）；不新增路由、不新增侧栏项；跨根条目先切根再跳转（explore §8.5 方案 B）；灰态是会话本地态（`read_at` 不出线）。
  - `export function NotificationBell({ root, onSelect }: { root: string; onSelect: (root: string) => void }): React.JSX.Element`。
  - 状态与数据：`const inbox = useNotifications();` + `const [open, setOpen] = useState(false);` + `const navigate = useNavigate();`。
  - 徽标：`inbox.unread === 0` 不渲染；`inbox.unread > 99` 呈 `99+`；否则呈数字（`Badge` + `data-testid`）。
  - 抽屉：`<StandardDrawer title="通知" trigger={<Button variant="ghost" size="icon" aria-label="通知"><Bell /></Button>} open={open} onOpenChange={(value) => { setOpen(value); if (value) inbox.load(); }} content={…} />`；`content` 自上而下 = 头部「全部已读」按钮（`onClick={() => void inbox.markAllRead()}`）+ inline 错误（`inbox.error !== null` 时）+ 空态（`items.length === 0`）+ 列表（`inbox.items.map(...)`）。
  - 列表行：`level` 图标（四档各取一个 `lucide-react` 图标）+ `message` + `body`（在场才渲染）+ `new Date(createdAt).toLocaleString()`；已读灰态 = `inbox.allRead || inbox.readIds.has(id)`（`data-state` 或 className 承载）。
  - 行点击：`void inbox.markRead([id]);` → `if (notification.sourceRef === null) return;` → `if (notification.root !== null && notification.root !== root) onSelect(notification.root);` → `navigate(`/changes/${notification.sourceRef}`);`。
- [x] `packages/desktop/src/app.tsx`：
  - `ShellHeader` 增 `root: string` / `onSelect: (root: string) => void` 两 props（并更新其 doc：终态 = 折叠钮 + 标题 + 版本/更新指示 + 通知铃铛），在 `<UpdateIndicator state={update} />` 之前挂 `<NotificationBell onSelect={onSelect} root={root} />`。
  - `App` 的挂载点改为 `<ShellHeader environment={environment} onSelect={workspaceState.select} root={workspaceState.root} update={update} />`（`root` 在此分支非 null 由外层的 `workspaceState.root === null` 判定保证）。
  - `<Toaster />`（`:123`）、`AppRoutes`、侧栏与布局其余零改动。

## 阶段十一：架构模型资产（四文件 + ADR + 索引）

- [x] `.dev-team/architecture/graph/model/02-backend-core.toml`：① 表头 `# 02 · core 分层（8 元素 / 9 关系）` → `（9 元素 / 10 关系）`；② `App.Backend.Core.Checks` 之后追加元素 `App.Backend.Core.Notification`（`kind = "module"`、`gate = "off"`、`path = ["./packages/desktop/src-tauri/crates/core/notification/"]`、一行中文 `summary`，零 `entry` / 零 `api` / 零 `doc` / 零 `metadata`）；③ `Orchestration` 组内（`Foundation` 边之后、`Workflow` 边之前）追加 `[[relation]] from = "App.Backend.Core.Orchestration"` / `to = "App.Backend.Core.Notification"` / `desc = "终态与停等更新投影为通知词表"`。
- [x] `.dev-team/architecture/graph/model/03-backend-infra.toml`：① 表头 `（8 元素 / 24 关系）` → `（8 元素 / 25 关系）`；② 第 2 行的「唯一跨域边 store → core.workflow」改写为「跨域边（store → core.workflow / core.notification）与生产依赖一致」；③ `Store` 组内（`Core.Foundation` 边之后、`Core.Workflow` 边之前）追加 `→ App.Backend.Core.Notification` / `desc = "收件箱记录字段复用通知词表并实现落库 sink"`。
- [x] `.dev-team/architecture/graph/model/04-backend-shell.toml`：① 表头 `（1 元素 / 13 关系）` → `（1 元素 / 14 关系）`；② `Shell` 组内（`Core.Foundation` 边之后、`Core.Orchestration` 边之前）追加 `→ App.Backend.Core.Notification` / `desc = "通知命令面装配扇出点与事件投影"`。
- [x] `.dev-team/architecture/graph/model/06-frontend.toml`：① `App.Frontend.Shell` 的 `path` 数组末尾追加 `"./packages/desktop/src/components/notification-bell.tsx"`（追加而**不是**替换：既有四项逐字保留）；② 表头 `（32 元素 / 74 关系）` → `（32 元素 / 75 关系）`；③ `Shell` 关系组首（`Components.Ui` 边之前）追加 `→ App.Frontend.Components.Standard.Drawer` / `desc = "收件箱抽屉复用标准抽屉"`。
- [x] 运行 `pnpm -C packages/desktop run api:export` 重生成 `.dev-team/architecture/graph/INDEX.toml`（**MUST NOT 手改**）；复核 `git diff` 只有计数与索引行的机械变化。
- [x] `.dev-team/architecture/decisions/0001-model-granularity.md`：`§1` 基线 `55 元素 / 123 关系` → `56 元素 / 127 关系`、后端 `49（9 + 24 + 13 + 3）` → `52（10 + 25 + 14 + 3）`、前端 `74` → `75`；`§3` 「基线 55 个元素」→ 56；`§5` 「`App.Backend.Shell` 出边 13 条」→ 14；`§9` 复算段整段换值（元素侧 `02(8) → 02(9)`，其余段元素数不变；关系侧 `02(9) → 02(10)`、`03(24) → 03(25)`、`04(13) → 04(14)`、`06(74) → 06(75)`，合计 `56` / `127`、`叶 46` → `47`、前端 `74 = 66 + 8` → `75 = 67 + 8`）；`§8` 的 11 条已知缺口逐字不动。
- [x] 自查：对 `06-frontend.toml` 的 `App.Frontend.Shell.path` 与 `notification-bell.tsx` 的 `import ... from '@/components/standard/drawer'` 行做交叉核对（`path` 归属与边端点必须成对成立，R-12）。

## 阶段十二：版本、边界自查与交接

- [x] `packages/desktop/package.json`：`version` `0.7.5` → `0.7.6`；确认 `src-tauri/tauri.conf.json` 仍为 `"version": "../package.json"`、`src-tauri/Cargo.toml` 的 crate 版本行逐字未动。
- [x] 边界自查（只读复核，不跑门禁）：
  - `grep -rn "tokio\|tauri\|native_db\|async fn deliver" packages/desktop/src-tauri/crates/core/notification/src` 零命中；该 crate 的 `Cargo.toml` 恰两依赖。
  - `grep -rn "from 'sonner'" packages/desktop/src --include=*.ts --include=*.tsx | grep -v '\.test\.'` 恰两处（`components/ui/sonner.tsx` / `lib/notify.ts`）。
  - `grep -rn "setInterval\|from 'sonner'" packages/desktop/src/hooks/use-notifications.ts` 零命中；`grep -c "new Channel" packages/desktop/src/hooks/use-notifications.ts` = 1；`grep -c "notificationSubscribe(" packages/desktop/src/hooks/use-notifications.ts` = 1。
  - `grep -rn "RunUpdate::" packages/desktop/src-tauri/src/commands/{change_flow,notifications}` 只剩 `RunUpdate::Ask {` / `RunUpdate::SessionEvent {` 两处构造与匹配（无按变体过滤白名单）；MUST NOT 出现 `Step` / `Edge` 判定。
  - `grep -rn "tauri-plugin-notification" packages/desktop/src-tauri` 零命中；`git status --porcelain` 的 `capabilities/**` 零条目。
  - `git status --porcelain` 的改动集 ⊆ design「变更清单」所列路径（`openspec/specs/**` 零 diff）。
- [x] 交接：把 design 末节的「交接给下游相位的随动清单」逐项交给测试相位（新建 6 个测试文件、`bindings/mod_test.rs` 清单 +5、`app.test.tsx` 命令序列与计数断言、`change_flow/mod_test.rs` 的 `app_with` 托管补一行）。

---

## 落地留痕（实现相位）

- 全部 40 项任务已落盘并按依赖序验证：`cargo check -p notification -p orchestration -p store`、`cargo check -p dev-team`、`cargo fmt --all`、`tsc --noEmit` 均通过；`bindings:export` 与 `api:export` 已重生成（生成物 diff 只来自生成）。
- 架构读数实读 **56 元素 / 127 关系**、`status = clean`、`diagnostics` 空；`apiFindings` 37 条（原 36 + 新元素一条 `docMissing`，warn 不纳管）。`INDEX.toml` 新增 `App.Backend.Core.Notification` 一条 `status = absent` 登记。
- ADR 计数同批换段：`§1`（56 / 127、后端 52 = 10+25+14+3、前端 75）、`§4`（dev 边排除后后端仍 52）、`§5`（Shell 出边 14）、`§6`（基线 56 元素）、`§9` 复算段（容器 9 + 叶 47、前端 75 = 67 + 8）；`§8` 的已知缺口逐字未动。
- **测试夹具随动（第 1 轮静态检查反馈修复，纯增量、零断言改动）**：`src-tauri/src/commands/change_flow/mod_test.rs` 收 45 行新增——① `app_with` 按 `main.rs` 同序补 `global_shared() → manage(stores) → manage(Arc::new(NotificationBus::new(global)))`（否则 `change_flow_start_with` 的 `app.state::<Arc<NotificationBus>>()` 在测试里 panic）；② 新增 `notification_bus(&app)` 装置 helper；③ 8 处 `ChangeFlowSink { … }` 字面构造补齐旁路四字段（`change_name` / `run_id` / `workflow_type` / `bus`）。全部为夹具接线，未新增/修改任何用例与断言。
- **前端门禁随动（第 2 轮静态检查反馈修复）**：① `components/notification-bell.tsx` 的 lucide 多行 import 按 `vp check --fix` 收成单行（该仓 printWidth 宽于 100，手写折行即格式错）；② `hooks/use-notifications.ts` 的 `useNotifications` 函数体 52 行超 `max-lines-per-function: 50`，把 `load` / `markRead` / `markAllRead` 抽成同文件私有 hook `useInboxActions(setState)`（`Pick<NotificationInbox, …>` 返回，`dispatch` 作依赖），`useNotifications` 降到 19 行——零行为变化、公开类型面零变化。
- **交接提醒（`--all-targets` 已编译通过；下列是测试相位的运行时随动）**：
  1. 按 tasks 阶段四/阶段一的约定，`commands/notifications/mod.rs` 与 `crates/core/notification/src/lib.rs` 的 `#[cfg(test)] mod mod_test;` / `mod lib_test;` 声明行**留待测试相位与测试文件同批追加**（本相位不声明缺失模块）。
  2. `crates/infra/store/src/lib.rs` 的三个既有测试声明（`model_test` / `envelope_test` / `store_test`）已在案，测试相位只改既有文件、零声明改动；`crates/core/orchestration/src/lib.rs` 需与 `notify_test.rs` 同批追加 `#[cfg(test)] mod notify_test;`。
  3. 运行期读数随动：`envelope_test.rs` / `store_test.rs` 的全局库模型组断言（3 → 4 模型）、`bindings/mod_test.rs` 的命令名与包装名清单（57 → 62）、`app.test.tsx` 的命令序列与调用计数。

---

## 返工段（回溯 1）：事件轨注入面移入 orchestration

> **成因**：首轮设计把 `RunEventSink::emit` 记为 `ConfirmWait` / `Finished` 的 hook 点，实测该 trait 只由 worker 对 `SessionEvent` 调用（`crates/infra/agent/src/worker.rs:72`），两个变体经 `RunGuard::emit`（`crates/core/orchestration/src/control.rs`）→ `ChangeFlowControl::publish` 直落广播面 ⇒ 生产链上事件轨通知永不发生，首轮用例只在 `ChangeFlowSink::emit` 上自证。定稿形态见 design「事实校准」6 与 DD12：**注入面放 `RunGuard`，经 `walk_run` 入参进入，`notify::deliver` 是唯一组合点**。
>
> **回溯 2（判据收窄，零代码影响）**：阶段十六自查的 grep 判据只认**生产代码**，并按 design §接线自检绕开合法命中面——`retire` 的实现面按 DD12 就在 `commands/notifications/mod.rs`（`NotificationBus::retire_run` + `NotificationBridge::retire`），`commands/exec` 的 `TurnFinished` / `SessionStreamItem::Finished` 与 `crates/core/notification` 的 `Hub` 定义均不属判据。判据不得要求删除这三处；DD / 变更清单 / AC 零改动。
>
> **取代关系**：

| 首轮任务 | 处置 | 说明 |
|---|---|---|
| 阶段四（`NotificationBus` / `InAppSink` / 五命令 / `all_commands!` / `main.rs`） | **保留** | 只增一项 `NotificationBridge`（阶段十五） |
| 阶段五全部（`ChangeFlowSink` 四字段、`emit` / `ask_started` 各调投影、sink 构造四值） | **被阶段十六取代** | 事件轨移入 orchestration；`ask_started` 改为一行 `notify::deliver`；sink 字段收成 `root` / `change_id` / `control` / `notifier` |
| 「不要修改」表的 `crates/core/orchestration/src/{state,control}.rs` 整体红线 | **收窄** | `state.rs` 与 `ChangeFlowControl`（含 `publish` 函数体）、`port.rs` 既有 trait 面仍逐字不动；`RunGuard` / `StepEmitter` 的字段与 `emit` 的一行转发进入改动面 |
| 阶段十一 / 阶段十二（架构模型 + 版本） | **保留，不再新增** | DD12 零新元素、零新边、零新命令 ⇒ 模型表头 / ADR / `INDEX.toml` / `bindings` 均不应因本轮再变 |

### 阶段十三：core 出口词汇与组合点（`orchestration/src/{notify,lib}.rs`）

- [x] `crates/core/orchestration/src/notify.rs`：追加 `pub trait RunNotifier: Send + Sync`——`fn context(&self) -> &NotifyContext` / `fn deliver(&self, notification: &Notification)` / `fn retire(&self)`，每个方法一行 `///` 写清「旁路：失败由实现自降级、MUST NOT 上抛」；trait doc 写明「core 只认 trait，实现驻壳层」。
- [x] `notify.rs`：追加 `pub fn deliver(update: &RunUpdate, notifier: &dyn RunNotifier, now: i64)`——`if let Some(notification) = project(update, notifier.context(), now) { notifier.deliver(&notification); }`，随后 `matches!(update, RunUpdate::Finished { .. })` 时 `notifier.retire()`；doc 写明「六变体旁路投递的唯一组合点；两处调用者（`RunGuard::emit` / `AskPort::ask_started`）只做一行转发，白名单与终态清理判定只写在本函数」。
- [x] `notify.rs` 模块 doc 补一句「本模块是投影与投递的唯一组合点（`project` / `deliver`），调用面只有 `RunGuard::emit` 与壳层 `AskPort::ask_started`」。
- [x] `crates/core/orchestration/src/lib.rs`：`pub use notify::{deliver, project, NotifyContext, RunNotifier};`（按字母序，替换既有 `pub use notify::{project, NotifyContext};` 行）；`#[cfg(test)] mod notify_test;` 已在案，零改动。
- [x] 自查：`notify.rs` 零 `SystemTime` / 零 `store` / 零 `tauri` / 零 `spawn`；`deliver` 内只有一条变体判定（`Finished` 终态清理），白名单仍全在 `project`。

### 阶段十四：注入面（`orchestration/src/{control,walker}.rs`）

- [x] `crates/core/orchestration/src/control.rs`：`RunGuard` 增私有字段 `notifier: Option<Arc<dyn RunNotifier>>`（doc：「run 级旁路出口；`walk_run` 入口单点 attach；**不落注册表**——`ChangeFlowControl` 保持进程内零 IO、`publish` 逐字不动」）；`begin_run` 的 `Ok(RunGuard { … })` 字面补 `notifier: None`（签名零改动）。
- [x] `control.rs`：新增 `pub(crate) fn with_notifier(self, notifier: Option<Arc<dyn RunNotifier>>) -> Self`（消费 self、写字段返回；doc 写明唯一调用点是 `walk_run`）。
- [x] `control.rs`：`RunGuard::emit` 在 `self.control.publish(...)` **之前**插 `if let Some(notifier) = &self.notifier { notify::deliver(&update, notifier.as_ref(), now_millis()); }`，并补 doc（「旁路先于广播：先落库、后广播；失败不影响随后的 `publish`」）；`ChangeFlowControl::publish` 函数体零改动。
- [x] `control.rs`：`StepEmitter` 增同名字段；`RunGuard::emitter()` 克隆它；`attach_output_ref` 的合成 `RunGuard` 字面补 `notifier: self.notifier.clone()`。
- [x] `crates/core/orchestration/src/walker.rs`：`walk_run` 尾部追加入参 `notifier: Option<Arc<dyn RunNotifier>>`；函数体**第一行** `let guard = guard.with_notifier(notifier);` + 一行注释（「注入点：早于起点写缝，fail-fast 终态也要发 `Finished`」）。`use` 组补 `RunNotifier`。
- [x] 自查：`git diff crates/core/orchestration/src/control.rs` 内 `publish` 函数体零 diff；`grep -n "notifier" control.rs` 命中面只有 `RunGuard` / `StepEmitter` / `emit` / `with_notifier` / `emitter` 五处。

### 阶段十五：壳侧出口实现（`src/commands/notifications/mod.rs`）

- [x] 新增 `pub struct NotificationBridge { context: NotifyContext, bus: Arc<NotificationBus> }` + `pub fn new(context: NotifyContext, bus: Arc<NotificationBus>) -> Self`（doc：run 级一次构造、零状态转发；`bus` 必须是全局库实例——收件箱的维度隔离由 bus 的结构保证）。
- [x] `impl orchestration::notify::RunNotifier for NotificationBridge`：`context` 返回 `&self.context`；`deliver` → `self.bus.publish(notification)`；`retire` → `self.bus.retire_run(&self.context.run_id)`。
- [x] 模块 doc 补一段「事件轨出口适配器」：core 侧 `RunGuard` 只认 `RunNotifier`，壳提供实现；删除动作只在本文件（bridge 不持 store 句柄，只经 `NotificationBus`）。
- [x] `use` 组补 `orchestration::notify::{NotifyContext, RunNotifier}`。
- [x] 自查：`NotificationBus` / `InAppSink` / 五命令 / 五条 `*_with` 在本轮零 diff；bridge 内零 `store::` 调用。

### 阶段十六：change flow 收口 + 夹具机械随动（`src/commands/change_flow`）

- [x] `src-tauri/src/commands/change_flow/mod.rs`：`ChangeFlowSink` 收成四字段 `root` / `change_id` / `control` / `notifier: Arc<dyn RunNotifier>`；**删去** `change_name` / `run_id` / `workflow_type` / `bus` 与私有 `fn context(&self)`；结构 doc 改写（「只承载 Ask 面与会话槽；事件轨在 `RunGuard`」）。
- [x] `impl RunEventSink for ChangeFlowSink::emit`：回到「`SessionEvent` 槽同步（`control.set_session`）→ `control.publish`」，零投影、零 `Finished` 分支、零 `bus` 调用。
- [x] `impl AskPort for ChangeFlowSink::ask_started`：`let update = RunUpdate::Ask { ask };` → `notify::deliver(&update, self.notifier.as_ref(), now_millis());` → `self.control.publish(&self.root, &self.change_id, update);`；`ask_resolved` 逐字不动。
- [x] `change_flow_start_with`：`begin_run` 之后取 `bus`（`app.state::<Arc<NotificationBus>>()`）→ 用 `root` / `id` / `record.name` / `run_id` / `record.workflow_type` 装 `NotifyContext` → `let bridge: Arc<dyn RunNotifier> = Arc::new(NotificationBridge::new(context, bus));` → 一份进 `ChangeFlowSink { notifier }`、一份作 `walk_run` 第 9 参 `Some(Arc::clone(&bridge))`。
- [x] `use` 组：`use orchestration::notify::{self, NotifyContext, RunNotifier};`（模块路径导入，调用点写 `notify::deliver(...)`）+ `use crate::commands::notifications::{NotificationBridge, NotificationBus};`（旧的 `project` 引用删除）。
- [x] 夹具机械随动（**零断言改动**）：`crates/core/orchestration/src/walker_test.rs` 的 6 处 `walk_run(` 调用补第 9 参 `None`、`probe_guard` 的 `RunGuard` 字面补 `notifier: None`；`src-tauri/src/commands/change_flow/mod_test.rs` 的 11 处 `ChangeFlowSink { … }` 收成四字段（`notifier` 用 `Arc<NotificationBridge>` 或测试替身）。`control_test.rs` 零改动（`begin_run` 签名未变）。
- [x] 自查（grep 级，判据只对生产代码生效——每条命令一律 `| grep -v '_test\.rs'`）：
  1. `grep -rn "RunUpdate::" src/commands/{change_flow,notifications} --include=*.rs` 只剩 `Ask {` 构造与 `SessionEvent {` 匹配（零按变体过滤）。
  2. `grep -rn "RunUpdate::Finished" src/commands --include=*.rs` 零命中（终态判定只在 core 的 `notify::deliver`）；`commands/exec` 的 `KernelOutput::TurnFinished` / `SessionStreamItem::Finished` 是执行流变体，不属本判据、MUST NOT 顺手删。
  3. `grep -rn "retire" src/commands --include=*.rs` 的命中集 ⊆ `src/commands/notifications/mod.rs`（`NotificationBus::retire_run` 的 store 委托 + `NotificationBridge::retire` 的一行转发）；`commands/change_flow/**` 零 `retire` / 零 `retire_run`。
  4. `grep -rn "walk_run(" src --include=*.rs` 生产恰一处且传 `Some(`。
  5. `grep -rn "deliver(&update" src crates/core/orchestration/src --include=*.rs` 命中恰两处（`control.rs` 的 `RunGuard::emit` / `change_flow/mod.rs` 的 `ask_started`）。
  6. `grep -rn "NotificationBus\|Hub" crates/core --include=*.rs --exclude='*_test.rs' --exclude-dir=notification` 零命中（`Hub` 的定义在 `crates/core/notification` 本目录内，不属此判据；`orchestration` 只认 `Notification` 与 `RunNotifier`）。

### 阶段十七：生成物 / 模型 / 版本核对与交接

- [x] `pnpm -C packages/desktop run bindings:export` ⇒ `src/types/generated/**` **零 diff**（本轮零命令面 / 零出线类型变化）；若出现 diff 即说明误改了命令面，先回退再查。
- [x] `pnpm -C packages/desktop run api:export` ⇒ `.dev-team/architecture/graph/INDEX.toml` **零 diff**（`App.Backend.Core.Orchestration` / `App.Backend.Shell` 均无契约资产，新 trait 不进契约面）；`archi_drift` 复核 `status = clean`、`56 元素 / 127 关系`、`apiFindings` 37 条（零新元素 / 零新边）。
- [x] 模型与 ADR 零改动核对：`02` / `03` / `04` / `06` 与 `decisions/0001-model-granularity.md` 逐字不动（DD12 不新增依赖边——`Shell → Core.Orchestration`、`Orchestration → Core.Notification` 均已在册）。
- [x] 版本：`packages/desktop/package.json` 维持首轮的 `0.7.6`（同一 change 内不二次 bump）；`src-tauri/tauri.conf.json` 仍 `"version": "../package.json"`；`src-tauri/Cargo.toml` 的 crate 版本行逐字不动。
- [x] 边界自查：`git status --porcelain` 的改动集 ⊆ design「变更清单」（本轮新增允许项：`crates/core/orchestration/src/{control,walker}.rs`）；`openspec/specs/**` 零 diff；`crates/core/checks/**` 零 diff。
- [x] 交接：把 design 末节「交接给下游相位的随动清单」逐项交给测试相位——重点三项：① `test-design.md` 的 `ChangeFlowSink::emit()` 用例行重锚到 `RunGuard::emit` → `notify::deliver` → `NotificationBridge`；② 补一条端到端接线用例（真实 `walk_run` 链或 `RunGuard::emit` + spy `RunNotifier` ⇒ 库落行），使「投影单测全绿、生产链零通知」的假绿不可能再现；③ `walker_test.rs` / `change_flow/mod_test.rs` 的夹具面。

---

## 落地留痕（返工段）

- **阶段十三–十七全部落盘**（事件轨注入面移入 orchestration）。编译读数：`cargo check --workspace --all-targets` 零 error / 零 warning；`cargo fmt --all --check` 通过。
- **注入链（实读，行号以终稿为准）**：`RunGuard` 持 `notifier: Option<Arc<dyn RunNotifier>>`（`begin_run` 铸 `None`）→ `with_notifier` 由 `walk_run` 函数体第一行消费（早于起点 `history.run_started`，fail-fast 终态也发 `Finished`）→ `RunGuard::emit` 在 `control.publish` **之前**一行 `notify::deliver(&update, notifier.as_ref(), now_millis())` → `NotificationBridge`（`deliver` → `bus.publish` / `retire` → `bus.retire_run`）。`ChangeFlowControl::publish` 函数体逐字未动（`git diff` 复核）；`StepEmitter` 随 `emitter()` / `attach_output_ref` 携带同一出口。
- **壳层收口**：`ChangeFlowSink` 收成四字段（`root` / `change_id` / `control` / `notifier`），`RunEventSink::emit` 回到「会话槽 + `publish`」纯桥；`AskPort::ask_started` 只做一行 `notify::deliver` 转发；`change_flow_start_with` 用既有五个局部量装 `NotifyContext` → `Arc<NotificationBridge>` → 一份进 sink、一份作 `walk_run` 第 9 参 `Some(...)`。
- **夹具机械随动（零断言改动）**：`walker_test.rs` 6 处 `walk_run(` 补第 9 参 `None`；`change_flow/mod_test.rs` 10 处 `ChangeFlowSink` 字面收成四字段，新增同文件私有装置 `notification_bridge(root, change, run_id, workflow_type, bus)`（真实 `NotificationBridge` + 可注入 bus），`notification_sink` 复用之。`control_test.rs` 零改动。
- **自查 grep（阶段十六六条，只对生产代码生效）**：① `src/commands/{change_flow,notifications}` 的 `RunUpdate::` 只剩 `Ask {` 构造与 `SessionEvent {` 匹配；② `src/commands` 零 `RunUpdate::Finished`；③ `retire` 命中集 ⊆ `commands/notifications/mod.rs`（`retire_run` 一行 store 委托 + `Bridge::retire` 一行转发），`change_flow/**` 零命中；④ `src` 内 `walk_run(` 生产恰一处且传 `Some(`；⑤ `deliver(&update` 恰两处（`control.rs` 的 `RunGuard::emit` / `change_flow` 的 `ask_started`）；⑥ `crates/core`（排除 `notification/`、排除 `*_test.rs`）零 `NotificationBus` / 零 `Hub`。
- **生成物零 diff 复核**：`bindings:export` 后 `src/types/generated/**` 的 diff 仍是首轮那 147 行（五命令 + 出线类型），零本轮内容（`git diff | grep -c "notifier\|Notifier\|notify"` = 0）；`api:export` 报「0 份生成物已写入（13 份未变）」，`INDEX.toml` 的 diff 仍是首轮 `App.Backend.Core.Notification` 一条 `status = absent` 登记。
- **架构读数**：`archi_drift` 实读 `status = clean`、**56 元素 / 127 关系**、`diagnostics` 空、`apiFindings` 37 条全 `docMissing`（warn 不纳管）——DD12 零新元素 / 零新边，四文件与 ADR 本轮零改动。
- **版本**：`packages/desktop/package.json` 维持 `0.7.6`（同一 change 不二次 bump）；`tauri.conf.json` 仍 `"version": "../package.json"`；`src-tauri/Cargo.toml` 的 crate 版本行（`0.1.0`）逐字未动。
- **接线验证**：`cargo test -p orchestration` 752 passed / 0 failed；`commands::notifications::mod_test` 25 passed；两条端到端接线用例（`start_sink携带记录四值且通知经托管bus抵达订阅端` / `start到finished全链路一条通知且命令面返回值逐字不变`）经真实 `walk_run` 链跑通 —— 生产链落库由此可证。
- **交测试相位（本轮明确的红项）**：`src/commands/change_flow/mod_test.rs` 的 5 条**旧锚点**用例仍红——`通知投影confirmwait落warning行且body取相位表` / `通知投影finished清同run等待类未读` / `通知投影finished失败与停止两终态映射` / `通知投影同相位二次emit仍一行且读状态不复位` / `通知投影ask主键退化与多轮ask各一行`：它们经 `ChangeFlowSink::emit(...)` 触发事件轨，而该缝按校准 6 / DD12 已不承载事件轨（本相位只做夹具编译随动、零断言改动）。重锚面见 design 末节交接清单第 ① / ② 项（改锚到 `RunGuard::emit` → `notify::deliver` → `NotificationBridge`，并补 spy `RunNotifier` 的 `deliver` / `retire` 组合点用例）。
