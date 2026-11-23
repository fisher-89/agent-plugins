# 提案: backend-notification-hub

> **变更**: backend-notification-hub（后端通知模块 · 通知扇出 + 持久化收件箱 + 前端门面）
> **日期**: 2026-11-21
> **状态**: draft
> **来源**: 本 change 目录 `explore.md`（§一 现状盘点 / §二 目标形态 / §三 A 轴裁决 / §七 A2 接线细节 / §八 收件箱 UI / §九 词表与首批事件 / §十 文字结论）
> **性质**: **新增一条旁路能力 + 收束一条既有纪律**——新 core crate（通知词表 / 扇出点 / Sink 缝）+ 新 user 维度模型（收件箱）+ 新命令轨道（五命令）+ 前端门面（12 处 sonner 直调收口）+ 顶栏铃铛抽屉；零改动既有 `RunUpdate` / `RunNotice` 线面、零新增路由、零新增配置键
> **不含**: 写文件 / 其他应用 CLI / 远程 API 三渠道（仅留 Sink 缝，MUST NOT 预铺 crate 与配置）；系统级 OS 通知（`tauri-plugin-notification` 未安装、无 capability）；收件箱修剪（cap / TTL）；agent 会话收口事件接入；`infra/notification` crate

### 命名与口径（先钉死，防评审跑偏）

| 不要读成 | 因为 |
|---|---|
| 「后端唯一写入点」 | 裁决为 **A2 双写入点**：前端反馈轨（`notify()` 同步 toast + 异步落库）与后端事件轨（`publish` → 扇出）各写各的，共用同一模型与同一命令轨道。explore §三 A1（全回环）已被显式否决 |
| 「统一通知中心接管所有提示」 | 收件箱只列 `kind = Event`（后端事件）；前端动作反馈（`kind = Feedback`）**只落库、不进列表、不进未读**。收束的是「sonner 直调点」，不是「所有提示的语义」 |
| 「进程外通知 / 系统通知」 | 后端进程不在场即不产生通知；持久化的价值是**跨重启保留 + 上一窗口期未读补拉**（见 §六 诚实边界） |
| 「通知即信号」 | `RunNotice` 是 kind-only 失效信号（收通知后重查）；通知是**内容本身**，载荷全量出线，两者语义不同、通道也不同 |

---

## 问题

### 现状一：前端 12 处 sonner 直调散在 6 个文件，零持久化

实测（本轮复核，explore §1.2 的「5 个文件」系漏计）：

| 文件 | 处数 | 语义 |
|---|---|---|
| `src/hooks/use-workspaces.ts` | 2 | 动作失败（add / remove） |
| `src/views/agents/hooks/use-agent-instances.ts` | 3 | 动作失败（save / remove / setDefault） |
| `src/views/agents/hooks/use-agent-providers.ts` | 2 | 动作失败（save / remove） |
| `src/views/changes/hooks/use-change-list.ts` | 2 | 动作成功 / 动作成功带警告（删变更） |
| `src/views/architecture/components/contract-panel.tsx` | 1 | 动作成功（写契约文件，裸 `toast(...)`） |
| `src/views/explores/components/explore-open-file.tsx` | 2 | 被动错误（打开 / 建探索失败） |

合计 **12 处 / 6 文件**。`<Toaster />` 单点挂在 App 根（`components/ui/sonner.tsx` → `app.tsx`），消息一闪即逝：**没有任何一处留下可回看的历史**。用户原话的「支持持久化写入」正好落在这个缺口上。

### 现状二：后端知道「用户多半不在场」的时刻，却只把它当视图失效信号丢掉

```
change flow run（change_flow_start 早 resolve，walker 后台跑）
   ├─ WaitingAsk / WaitingConfirm      ← 流程阻塞，用户不来就死等
   └─ Finished(Completed|Stopped|Failed) ← 长跑结束，用户大概率已切页
```

`RunUpdate` 的六变体已在进程内带着完整载荷（kind-only 降位发生在 `control.publish` 之后），`ChangeFlowSink` 是现成的两 trait 视图适配器（`RunEventSink` + `AskPort`）——**加通知只是第三个消费者，零新线程、零新注册表**。而今天这些事件到前端只剩「请重查」四个字。

### 现状三：没有通知词汇、没有收件箱载体

后端零「notification」词汇，`crates/infra/store` 已有现成的双维度底座（全局库 3 模型 / workspace 库 12 模型）与「新增模型动四个点」的机械路径（model.rs / store.rs / envelope.rs / Store 方法，db inspector 自动出表）。缺的是：一条词表、一个扇出点、一张表、五条命令、一处渲染。

### 三条硬约束决定了落地只有一种形状

```
core/*   禁 Tauri、禁 spawn、禁 tokio 运行时
infra/*  禁 Tauri
壳(src/) 唯一允许 Tauri 的地方（命令面、Channel、注册表装配）
```

⇒ 「弹 toast」在物理上只能落在**壳 + 前端**：core 描述「有一条通知」，infra 负责落库，壳把 in-app 那一份翻成 `Channel`，前端只做渲染。这条链外加「通知是旁路」的不变量，构成本提案的全部形状。

---

## 提案

### 一 · 词汇与扇出点（新 crate `crates/core/notification`）

```rust
pub enum NotificationLevel  { Info, Success, Warning, Error }  // 对位 sonner 四档
pub enum NotificationKind   { Event, Feedback }                // 后端事件 / 前端动作反馈

pub struct Notification {          // 出线 DTO 亦是它（全量载荷，零 kind-only 降位）
    pub id: String,                // 事件：幂等串即主键；反馈：前端 randomUUID
    pub kind: NotificationKind,
    pub level: NotificationLevel,
    pub message: String,
    pub body: Option<String>,
    pub root: Option<String>,      // 工作区归属字符串（禁跨库 join 惯例）
    pub source_ref: Option<String>,// change id
    pub created_at: i64,
}

pub trait Sink { fn deliver(&self, n: &Notification); }          // 同步、无返回值、不 await
pub struct Hub { sinks: Vec<Arc<dyn Sink + Send + Sync>> }
impl Hub { pub fn publish(&self, n: &Notification) { /* 顺序遍历 */ } }
```

三条不变量写进 crate doc 而不靠调用方自觉：**一 sink 失败不影响其他 sink**、**通知是旁路（不阻断业务、不为自身失败再发通知）**、**只保证投递一次（重试 / 退避 / 批量归 sink 内部）**。

依赖方向（三条新边，全部 infra/壳 → core，core 内部一条）：

```
core/notification ← core/orchestration   投影函数（哪些时刻值得喊人、喊什么）住引擎侧
core/notification ← infra/store          记录字段直接用词表枚举 + 落库 sink 实现
core/notification ← 壳                    Hub 装配 + in-app sink
```

**首批只有两个 sink**：`StoreSink`（落库，驻 `infra/store`）与 `InAppSink`（broadcast → `Channel`，驻壳）。explore §四的三个未来渠道（写文件 / CLI / 远程 API）**MUST NOT 在本变更内预铺**——按仓库「第二个消费者到场才下沉」的惯例，`infra/notification` crate、渠道开关与 `KNOWN_KEYS` 新键一律留到第二渠道立项时裁决。

`read_at` **不进食词表**：读状态属收件箱记录面（`NotificationRecord`），投递面是「不可变的一条内容」。

### 二 · 双写入点（A2 裁决落地的两条轨）

```
① 前端反馈轨（同步 toast，异步落库）
   6 文件 12 处直调 sonner
        └─→ src/lib/notify.ts        ← 新门面，唯一 import { toast } from 'sonner' 之处
               ├─ present(level, msg, body?)   仅 toast（供 ② 的 Channel 事件用）
               └─ notify(level, msg, body?)    toast + void ipc.notificationRecord(...)

② 后端事件轨（publish → 落库 → Channel → toast）
   ChangeFlowSink::emit(RunUpdate)  ← src/commands/change_flow/mod.rs（现成适配器）
        ├─ projection(update, ctx) → Option<Notification>   ← core/orchestration/src/notify.rs
        │     Step / Edge / SessionEvent ⇒ None（唯一单点，MUST NOT 用 if 白名单散在 sink 里）
        └─ Hub.publish(n)
               ├─ StoreSink：全局库 upsert（失败 eprintln 一行，静默降级不阻断）
               └─ InAppSink：broadcast → notification_subscribe 的 Channel
   AskPort::ask_started(ask) 走同一投影（`Ask` 不经 `emit`，两条 hook 点、一个投影函数）
```

**门面的调用形态 MUST 与现状逐字一致**（`toast.error(msg)` / `toast.warning(msg, { description })` / 裸 `toast(msg)`），且 **MUST NOT 给 sonner 传 `id`**：

- 既有 6 个测试文件以 `vi.mock('sonner', ...)` 拦截同一个模块，门面内部调用同样被拦截 ⇒ 零 toast 基线断言（大量 `not.toHaveBeenCalled()`）与 DOM 文本断言照旧成立；**19 处 `toHaveBeenCalledWith(<文案>)` 的精确参数断言因此零改动**。
- explore §7.2 原拟「toast 与落库同 `id`」在 A2 下无收益：前端反馈的 id 天生唯一（`crypto.randomUUID()`），不存在「同 id 更新」的需求；传 id 只会让 19 处断言同时变红。

**落库失败静默**：`void ipc.notificationRecord(...)` 包 try/catch + `.catch`（`ipc.ts` 的参数面校验会同步抛 `IpcArgsError`），DEV 下 `console.warn` 一行留痕（与 `lib/ipc-drift.ts` 的「soft-report、永不 throw」同族）；MUST NOT 因落库失败再发一条通知（递归）。

### 三 · 首批事件白名单与幂等主键（定稿，来自 explore §十）

| `RunUpdate` | 裁决 | level | message 必含要素 | body |
|---|---|---|---|---|
| `Step` | **永久排除** | — | — | — |
| `Edge` | **永久排除** | — | — | — |
| `SessionEvent` | **永久排除**（转录另有面） | — | — | — |
| `Ask` | 纳入 | `Warning` | 变更名 + 「等你的回答」 | `ask.question`（选项**不进** body，页面里已有 AskCard 承载） |
| `ConfirmWait` | 纳入 | `Warning` | 变更名 + 相位 + 「等确认」 | 相位表 `PhaseDefinition.description`（人话标签的唯一来源，MUST NOT 在通知侧另立翻译表） |
| `Finished` | 纳入 | `Completed→Success` / `Failed→Error` / `Stopped→Info` | 变更名 + 完成 / 失败 / 已停止 | `reason`（仅 `Failed`） |

文案措辞由投影函数单点持有（spec 只钉 level 映射与「必含变更名」这一要素，不复述字面），相位中文名一律取自 `workflow::write::phase_table`。

**主键 = 幂等串**（不开 `dedupe_key` 二级索引）：

| 事件 | 主键 | 备注 |
|---|---|---|
| `Finished` | `run:{run_id}:finished` | 终态只收敛一次；重投即 upsert 同一行 |
| `ConfirmWait` | `run:{run_id}:confirm:{phase}` | 停等点唯一 |
| `Ask` | `run:{run_id}:ask:{tool_call_id}`；`tool_call_id` 缺位退 `run:{run_id}:ask:{created_at}` | 多轮 ask 各一行，不互相覆盖 |
| 前端反馈 | `crypto.randomUUID()`（前端铸） | 天生唯一，无 dedupe 需求 |

配套两条不变量：**upsert 只更新 `message` / `body`，MUST NOT 把 `read_at` 从 `Some` 翻回 `None`**（重投不得复活已读）；**`Finished` 落库后同 run 的 `confirm:*` / `ask:*` 未读行一并置已读**（否则「等你确认」的死未读永远等不到人）。

**show 面的分流**（explore §7.5-①/⑤、§9.5-① 的裁决）：`Stopped` 的 `Info` **落库但不弹 toast** —— 实现为机械规则「`level = Info` 不弹 toast」，零新增字段（不引入 `silent`）。前端 ui 反馈恒不弹第二遍（不经 Channel 广播）。

### 四 · 收件箱持久化（user 维度 + 五命令）

```
NotificationRecord（全局库，native_model 新 id，user 维度）
   id(主键) / kind / level / message / body / root / source_ref / created_at / read_at
   ↑ 表与字段说明由 /// doc 自动派生；MODEL_ENTRIES 登记一行；db inspector 自动出表
```

| 命令（新轨道 `commands/notifications/`） | 形态 | 说明 |
|---|---|---|
| `notification_subscribe(on_event: Channel<Notification>) -> u32` | 订阅 | 先注册 receiver **再**读未读计数，同刻返回 —— 消灭「订阅前丢更新」窗口；照 `watch_subscribe` / `change_flow_watch` 形 |
| `notification_list(offset, limit) -> NotificationPage` | 读 | 倒序（`created_at` desc）+ 页界信封（`items` + `total`）；抽屉打开时补拉 |
| `notification_mark_read(ids) -> u32` | 写 | 点单条；返回剩余未读 |
| `notification_mark_all_read() -> u32` | 写 | 全部已读（含未加载页）；返回剩余未读 |
| `notification_record(record) -> Result<(), String>` | 写 | 前端反馈轨落库（fire-and-forget，前端不等待） |

- **写命令落本轨道，不并入 db 轨**（`commands/db/` 只读纪律）。
- **过滤口径单点**：`notification_list` 与未读计数**恒只认 `kind = Event`**；`Feedback` 只落库、不进列表、不进未读（否则每天几十条「保存失败」把收件箱淹掉）。
- **不做修剪**：本变更零 cap、零 TTL（explore §7.5-④ 的「先不修剪，观察一周」）；增长面由列表分页与 db inspector 承载，修剪属独立变更（R-2）。
- **反馈记录不带工作区归属**：`notification_record` 的 `root` / `source_ref` 恒 `None` —— 门面是 `src/lib/` 纯函数（不持有当前根），而为一条「保存失败」把 root 逐个穿过 12 处调用点不值得；唯一消费面（db inspector 审计留痕）不依赖它。
- **未被读取的项与「应用不在场」**：进程不在场不产生通知；重启后历史与未读靠上述读面补拉 —— 这是持久化的全部价值，MUST NOT 扩写为「进程外通知」。

### 五 · 壳层铃铛与抽屉（收件箱的唯一 UI 面）

```
app.tsx
  └─ ShellHeader（现内联件）
        └─ <NotificationBell root select>   ← 新文件 components/notification-bell.tsx
              铃铛 + 未读徽标（99+ 上限）+ StandardDrawer 收件箱
              列表项：level 图标 / message / body / 时间 / 点击置已读
              跨根条目点击 = 先 select(root) 再 navigate(`/changes/{source_ref}`)
  <Toaster /> 恒不动      AppRoutes 零改动（不新增路由、不新增侧栏项）
```

- `StandardDrawer`（`components/standard/drawer.tsx`）的 `trigger` + `content` 形态直接承载列表，零新原语、零新样式体系。
- **铃铛组件是新文件**（不内联进 `app.tsx`：`max-lines-per-function: 50` 与页壳可读性；且 `ShellHeader` 已在 `app.tsx` 内）。代价是架构模型两行：`App.Frontend.Shell` 的 `path` 追加一项（`components/app-sidebar.tsx` 先例）+ 一条 `Shell → Components.Standard.Drawer` 边。**MUST NOT 让该文件落在模型 `path` 之外**——未在 `path` 内的源文件会静默落 `unmatchedFiles` 且不产出任何边（`2026-11-17-frontend-component-element-coverage` 的病灶一）。
- `src/hooks/use-notifications.ts`（订阅 + 未读 + 分页 + 标记）与 `src/lib/notify.ts` 落 `src/hooks/` / `src/lib/`，两者**不建元素、零边**（包根 hooks / lib 的既有口径）。
- 通道纪律：`Channel<Notification>` 是**第三类执行流通道**（已有两类：agent 会话流、change run 变更通知；沿同一先例），MUST NOT 并入 `use-file-watch-signal.ts`（那件的划界原则：按驱动源切分，文件失效信号 ≠ 后端事件），MUST NOT 引入轮询。

### 六 · 诚实边界（写进 spec，不写进营销）

1. 后端进程不在场 ⇒ 不可能产生通知；本能力 MUST NOT 被表述为「进程外通知」。
2. 系统级 OS 通知（应用最小化时的桌面提醒）是另一个 sink、另一份裁决：`tauri-plugin-notification` 未安装、无 capability 配置，不在本变更。
3. 同一进程内多窗口：某窗口发起的 ui 反馈在其他窗口不可见（不广播）——ui 反馈是**本地交互反馈**，不是跨窗口事件。

---

## 能力

### 新增能力

| 能力 | 新增 requirement |
|---|---|
| `desktop-notification-hub` | ① 通知词表与扇出（Hub + Sink + 三不变量 + 渠道缝）；② 双写入点与前端门面；③ 事件白名单与幂等主键（含死未读清理）；④ 收件箱持久化与命令面（user 维度、过滤口径、页界、订阅初值、零修剪）；⑤ 壳层铃铛与抽屉（徽标、跨根跳转、通道纪律）；⑥ 应用不在场的不可能面 |

### 修改的能力

| 能力 | delta | 改什么 |
|---|---|---|
| `desktop-app-shell` | **MODIFIED ×3** | ①「Tauri command 轨道组织」：四轨表述 + 新 `commands/notifications/` 轨道（写命令落本轨道、MUST NOT 并入 db 轨）+ 一条 scenario；②「Sidebar 壳层布局」：header 终态由「折叠钮 + 标题 + 版本更新」扩为「… + 通知铃铛」，Toaster 单点陈述不变；③「React 前端刷新取数模型」：补一处 Channel 例外（通知订阅；agent 会话流之外的第二处登记，语义归新能力，此处只登记例外与「不属轮询取数」） |
| `desktop-workspace-store` | **MODIFIED ×1** | 「全局库与 workspace 库双库布局」：全局库模型组补 `NotificationRecord`（四模型），scenario 的模型分组断言随动 |
| `desktop-data-dimensions` | **MODIFIED ×1** | 「数据两维度模型」：user 维度代表补「通知收件箱（`NotificationRecord`）」，并补一条「收件箱归 user 维度」scenario |
| `repo-archi-model` | **MODIFIED ×1** | 「基线落位与模型文件划分」：计数表与合计换段 —— 元素 **55 → 56**、关系 **123 → 127**（`02` 9/10、`03` 8/25、`04` 1/14、`06` 32/75；后端 52 + 前端 75；容器 9 + 叶 47；前端恒等式 67 + 8），scenario 的读数与「计数复述单源」条目同批写实 |

> **为何 `repo-archi-model` 必须动**：本变更新增一个 crate（`crates/core/notification`）⇒ 一个元素 + 一条 core 内边；`store` / 编排 / 壳各增一条出边；壳的前端元素增一条 import 边。该能力自带「表头 / 本表 / ADR 三处同值」的追改面纪律，MUST NOT 让模型本体与计数表各说各话。

### 零 delta 的能力（出现 spec diff 即误改）

| 能力 | 理由 |
|---|---|
| `desktop-change-orchestration` | `RunUpdate` / `RunNotice` / `ChangeRunStatus` / `control.publish` 线面与语义**逐字不动**；通知投影是**旁路消费者**，「哪些 update 值得通知」是通知侧知识（由新能力持有，MUST NOT 在两处复述） |
| `desktop-change-flow-view` | 其「变更通知 → 重查统一视图」例外面零变化；通知不是视图取数通道 |
| `desktop-ipc-type-bindings` | 「清单守卫随单 change 随动」已是通则（`bindings/mod_test.rs` 的命令 / DTO 清单 + 尾部追加序在同一 change 内随动），本变更按该条执行，无陈述需改 |
| `desktop-file-watch` | 划界原则（按驱动源切分）明文只约束文件失效信号；`Channel<Notification>` 不属该件管辖（新能力登记该事实，不在此处补写） |
| `desktop-workspace-config` | 零新配置键、零渠道开关（第二渠道到场才裁决落点） |
| `desktop-db-inspector` | 自动表格零模型特定代码，新表自动出现在全局库 scope |
| `desktop-standard-components` | `StandardDrawer` 零改动（只被消费） |
| `desktop-ipc-runtime-guard` | 新命令自动进生成面；`notification_record` 的 args 走既有 strict 校验，无第二套校验 |

---

## 变更范围

### 实现文件 · 后端

| 文件 | 改动 |
|---|---|
| `src-tauri/Cargo.toml` | `[workspace] members` += `crates/core/notification`；`[workspace.dependencies]` += `notification = { path = … }`；根包 `[dependencies]` += `notification` |
| `crates/core/notification/Cargo.toml` | 新建（`serde` + `specta` 两依赖，零 workspace 内部依赖） |
| `crates/core/notification/src/lib.rs` | 新建：`NotificationLevel` / `NotificationKind` / `Notification` / `trait Sink` / `Hub` + 三不变量 doc |
| `crates/infra/store/Cargo.toml` | += `notification` |
| `crates/infra/store/src/model.rs` | += `NotificationRecord`（九字段 + `///` doc；`native_model` 新 id，MUST 与全局库既有 1/5/6 不冲突） |
| `crates/infra/store/src/notification.rs` | 新建：`NotificationRecord ↔ Notification` 的一比一映射单点 + `StoreNotificationSink`（`impl notification::Sink`，失败 `eprintln!` 降级） |
| `crates/infra/store/src/store.rs` | `global_models()` += `define::<NotificationRecord>()`；六个方法：`publish_notification`（upsert，保 `read_at`）/ `list_notifications`（Event、desc、页界）/ `unread_notification_count` / `mark_notifications_read` / `mark_all_notifications_read` / `retire_run_notifications(run_id)`（Finished 清死未读） |
| `crates/infra/store/src/envelope.rs` | `MODEL_ENTRIES` += 一行（`User` 维度五指针） |
| `crates/core/orchestration/Cargo.toml` | += `notification` |
| `crates/core/orchestration/src/notify.rs` | 新建：`NotifyContext { root, change_id, change_name }` + `project(&RunUpdate, &NotifyContext, now) -> Option<Notification>`（三变体纳入、三变体 `None`；相位中文名取 `workflow::write::phase_table`；主键按 §三 表铸造）+ `pub mod` 声明 |
| `src-tauri/src/commands/notifications/mod.rs` | 新建：五命令 + `InAppSink`（broadcast → `Channel`，照 `spawn_channel_forward`）+ 模块头注释（写命令轨道纪律） |
| `src-tauri/src/commands/mod.rs` | `pub mod notifications;` + `all_commands!` 尾部 += 五命令（保持登记序，既有条目零位移） |
| `src-tauri/src/commands/change_flow/mod.rs` | `ChangeFlowSink` 增 `hub` / `store` / `change_name` / `run_id` 字段；`emit`：投影 + `publish` + `Finished` 分支 `retire_run_notifications`；`AskPort::ask_started`：同投影；`change_flow_start_with` 把 `record.name` 与新句柄传入 sink |
| `src-tauri/src/main.rs` | `setup_hook` 增：`broadcast::channel` → `Hub::new(vec![StoreSink(global), InAppSink(tx)])` → `app.manage(Arc::new(hub))`（先取 `stores.global_shared()` 再 `manage(stores)`） |
| `src/types/generated/bindings.ts` / `bindings.zod.ts` | `bindings:export` 生成（勿手改） |

### 实现文件 · 前端

| 文件 | 改动 |
|---|---|
| `src/lib/notify.ts` | 新建门面：`present(level, message, body?)` / `notify(level, message, body?)`；四档映射到既有 sonner 调用形态（`info` → 裸 `toast(...)`），**不传 `id`**；`notify` 追加 fire-and-forget `ipc.notificationRecord`（`randomUUID()` 主键、静默失败、DEV 一行 `console.warn`） |
| `src/hooks/use-notifications.ts` | 新建：`notification_subscribe` 建 Channel 并取初值未读 → Channel 消息按 level 调 `present`（`Info` 不弹）+ 前插列表 + 未读 +1 → 抽屉打开时 `notification_list` 首屏 50 → `mark_read` / `mark_all_read` 以返回值回写徽标；subscribe 失败静默、list 失败 inline |
| `src/components/notification-bell.tsx` | 新建：铃铛（未读徽标 99+）+ `StandardDrawer` 收件箱（列表 / 单条已读 / 全部已读 / 跨根点击切根跳转 / 空态） |
| `src/app.tsx` | `ShellHeader` 消费新组件并透传 `root` / `select`；`<Toaster />` / 布局其余零改动 |
| `src/hooks/use-workspaces.ts` | 2 处 `toast.error` → `notify('error', …)`（文案逐字不变） |
| `src/views/agents/hooks/use-agent-instances.ts` | 3 处同上 |
| `src/views/agents/hooks/use-agent-providers.ts` | 2 处同上 |
| `src/views/changes/hooks/use-change-list.ts` | `toast.success` / `toast.warning(msg, {description})` → `notify('success' / 'warning', …)`（description 走 `body`） |
| `src/views/architecture/components/contract-panel.tsx` | 裸 `toast(...)` → `notify('info', …)` |
| `src/views/explores/components/explore-open-file.tsx` | 2 处 `toast.error(String(err))` → `notify('error', …)` |

### 架构模型与资产

| 文件 | 改动 |
|---|---|
| `.dev-team/architecture/graph/model/02-backend-core.toml` | 新元素 `App.Backend.Core.Notification`（`kind = "module"`、`gate = "off"`、`path = ./packages/desktop/src-tauri/crates/core/notification/`、summary）+ `Orchestration → Notification` 一条边 + 表头 `8 元素 / 9 关系` → `9 元素 / 10 关系` |
| `.dev-team/architecture/graph/model/03-backend-infra.toml` | `Store → Core.Notification` 一条边 + 表头 `8 / 24` → `8 / 25`（表头「唯一跨域边 store → core.workflow」的措辞随之改写为「跨域边」） |
| `.dev-team/architecture/graph/model/04-backend-shell.toml` | `Shell → Core.Notification` 一条边 + 表头 `1 / 13` → `1 / 14` |
| `.dev-team/architecture/graph/model/06-frontend.toml` | `App.Frontend.Shell` 的 `path` += `./packages/desktop/src/components/notification-bell.tsx` + `Shell → Components.Standard.Drawer` 一条边 + 表头 `32 / 74` → `32 / 75` |
| `.dev-team/architecture/graph/INDEX.toml` | `api:export` 重生成（勿手改） |
| `.dev-team/architecture/decisions/0001-model-granularity.md` | `§1` / `§9` 复算表与「§3 已知缺口」旁证随计数换段（56 / 127） |

### 测试文件

| 对象 | 改动 |
|---|---|
| `crates/core/notification/src/lib_test.rs` | 新建：扇出顺序与失败隔离（一 sink panic/Err 不拦后者）、空 sinks、Kind/Level 出线形态 |
| `crates/infra/store/src/model_test.rs` / `store_test.rs` / `envelope_test.rs` | 新模型往返 + 六个方法（upsert 保 `read_at`、Event 过滤、倒序页界、未读计数、mark_read / all、retire）+ `MODEL_ENTRIES` / 全局库分组随动 |
| `crates/core/orchestration/src/notify_test.rs` | 新建：三变体纳入 / 三变体 `None`、level 映射、主键串逐字、`tool_call_id` 缺位退化、相位中文名取自相位表 |
| `src-tauri/src/commands/notifications/mod_test.rs` | 新建：五命令薄包装 + 泛型测试缝（`notification_*_with`）+ `subscribe` 先订阅后读计数的顺序不变量 |
| `src-tauri/src/commands/change_flow/mod_test.rs` | 追加：`emit` 的 `Step` / `Edge` / `SessionEvent` 零通知、`ConfirmWait` / `Finished` 各一条、`Finished` 触发 `retire_run_notifications`、`ask_started` 一条 |
| `src-tauri/src/bindings/mod_test.rs` | 命令名 / 包装名清单 +5、出线 DTO 清单（`Notification` / `NotificationPage` / `NotificationLevel` / `NotificationKind` / `NotificationRecord` 视实际出线）+ 尾部追加序随动 |
| `src/lib/notify.test.ts` | 新建：四档映射到 sonner 调用形态（含 `info` 裸调用、`warning` 带 `description`、**不传 id**）+ `notification_record` 落库一次 + 落库失败不影响 toast |
| `src/hooks/use-notifications.test.ts` | 新建：订阅初值、Channel 消息前插 + 未读 +1、`Info` 不弹 toast、`mark_read` / `all` 回写、subscribe 失败静默、list 失败 inline |
| `src/components/notification-bell.test.tsx` | 新建：徽标（0 / N / 99+）、抽屉列表与空态、单条 / 全部已读、跨根点击先 `select` 后 navigate、同根直接 navigate |
| `src/app.test.tsx` | 桩新命令（`notification_subscribe` / `notification_list` / `notification_mark_*`）；`commandsExceptEnvironment()` 一类**命令序列断言**与「点击自身零新增调用」计数断言需把通知命令排除或补进期望序 |
| `src/hooks/use-workspaces.test.ts` 等 6 个既有 toast 测试 | 原则上零改动（门面调用形态不变）；若某断言锁了「invoke 调用集合」需局部收窄 |
| `src/views/architecture/components/contract-panel.test.tsx` | 零改动（DOM 文本断言，`notify('info')` 仍是裸 toast） |

### 不要修改

| 对象 | 理由 |
|---|---|
| `crates/core/orchestration/src/state.rs` / `control.rs` | `RunUpdate` / `RunNotice` / `ChangeRunStatus` / `AskPayload` / `publish` 广播面逐字不动（投影在旁路，反向不依赖引擎） |
| `src-tauri/src/commands/db/mod.rs` | db 轨道只读纪律；收件箱写命令落新轨道 |
| `crates/core/config/**` + `config.schema.json` | 零新配置键（渠道开关不在本变更） |
| `src/components/ui/sonner.tsx` / `src/components/standard/**` | Toaster 仍是唯一渲染器；`StandardDrawer` 只被消费 |
| `src/routes.tsx` / `src/components/app-sidebar.tsx` | 零新路由、零新侧栏项（收件箱只在顶栏抽屉里） |
| `src/lib/ipc.ts` / `src/lib/ipc-drift.ts` | Proxy 形态自动覆盖新命令，零手改 |
| `crates/infra/exec/**` | 零 CLI 渠道 |
| `openspec/specs/**` | 除本 change 的 5 份 delta 外零 diff（8 份零 delta 能力见「能力」章） |
| `.dev-team/architecture/models/**` | 不新建契约资产（新元素缺契约 ⇒ `docMissing` warn，永不纳管；见 R-8） |

---

## 验收标准

### AC-1 · 词表与扇出形态（core 侧零基础设施依赖）

`crates/core/notification` 只依赖 `serde` / `specta`（`Cargo.toml` 可读）；`Hub::publish` 顺序遍历 sinks 且逐 sink 独立（一 sink 失败不影响后续）；`trait Sink` 无返回值、无 `async`、无 `Result`；三不变量在 crate doc 在案。反形态判据：core 内零 `tokio::spawn` / 零 `tauri` / 零 `async fn deliver`。

### AC-2 · 双写入点收口（前端）

`grep -r "from 'sonner'" packages/desktop/src --include=*.ts --include=*.tsx | grep -v '\.test\.'` 恰两处：`components/ui/sonner.tsx`（Toaster）与 `lib/notify.ts`（门面）；12 处业务调用点全部改为 `notify(...)`；门面对 sonner 的调用形态与改动前逐字一致（`info` → 裸 `toast(message)`、`warning` → `toast.warning(message, { description })`、`success` / `error` → 单参），且**不向 sonner 传 `id`**。既有 19 处 `toHaveBeenCalledWith(<文案>)` 断言零改动通过。

### AC-3 · 门面落库的旁路语义

`notify(...)` 在 `notification_record` reject（含参数校验同步抛错）时 MUST NOT 抛出、MUST NOT 影响 toast、MUST NOT 重试或再发通知；DEV 下恰一行 `console.warn`（`present(...)` 零落库调用）。

### AC-4 · 白名单与幂等（后端事件轨）

投影函数对 `Step` / `Edge` / `SessionEvent` 恒 `None`（三条用例），对 `Ask` / `ConfirmWait` / `Finished` 恒 `Some`（三条用例）；`level` 映射与「message 含 change 名」逐条断言；主键串逐字 `run:{run_id}:finished` / `run:{run_id}:confirm:{phase}` / `run:{run_id}:ask:{tool_call_id}`（缺位退化用例一条）。投影是唯一白名单单点：`commands/change_flow` 与 `commands/notifications` 内零「按变体 if 过滤」。

### AC-5 · 幂等与读状态保真

同主键二次 publish ⇒ 表中恰一行且 `message` / `body` 更新；首条被 `mark_read` 后重投 ⇒ `read_at` 仍为 `Some`（MUST NOT 复活已读）。

### AC-6 · 死未读清理

某 run 先落 `confirm:{phase}` / `ask:*` 未读，再落 `run:{run_id}:finished` ⇒ 该 run 的等待类未读全变已读；其他 run 的行与 `Feedback` 行不受影响。

### AC-7 · 收件箱读面口径

`notification_list` 与未读计数恒只含 `kind = Event`（插入若干 `Feedback` 行后读数不变）；列表严格按 `created_at` 降序且页界自洽（`items.len() ≤ limit`、`total` = 该 kind 全量）；`notification_mark_all_read` 把未加载页一并置已读并返回 `0`。

### AC-8 · 订阅初值无丢更新

`notification_subscribe` 的实现顺序可断言：先取 broadcast receiver、再读未读计数、再返回；竞争用例（订阅建立与 publish 交错）下「返回初值 + 随后经 Channel 收到的条数」恰等于该时刻真实未读数（后端用例以「再次订阅返回的初值」作对照读数，MUST NOT 为此新增未读查询命令）。

### AC-9 · 壳层收件箱与跨工作区

铃铛在壳态呈现（欢迎态不挂），徽标 `0 / N / 99+` 三态；抽屉打开后列表按倒序呈现、单条点击置已读、全部已读清零；跨根条目点击先 `select(root)` 再 `navigate(/changes/{source_ref})`，同根直接 navigate，`source_ref` 不可达的行点击惰性（不报错、不跳转）。

### AC-10 · 通道纪律与失败面

订阅失败静默（不阻断壳、不白屏、不弹错误 toast）；列表失败 inline 持久（沿「错误呈现双轨」查询轨）；零轮询（无 `setInterval` / `setTimeout` 轮询）、零文件 watch 依赖、`use-file-watch-signal.ts` 零 diff；`Channel<Notification>` 构造与 `notification_subscribe` 调用各恰一处（`src/hooks/use-notifications.ts`）。

### AC-11 · 出线面与命令清单随动

`bindings:export` 后生成物含五命令的 typed 包装与对应 zod 表项，且生成目录的 diff 入库；`all_commands!` 尾部按登记序追加五条、既有条目零位移；`bindings/mod_test.rs` 的命令名 / 包装名 / 出线 DTO 清单与尾部追加序断言同 change 随动；`pnpm check` 四段（`server:check` → `bindings:export` → `api:check` → `client:check`）全绿。

### AC-12 · 架构模型三处同值

`archi_graph` 读数 = **56 元素 / 127 关系**（`02` 9/10、`03` 8/25、`04` 1/14、`06` 32/75；容器 9 + 叶 47；后端 52 + 前端 75；前端 `67 + 8`），`diagnostics` 空、`status = clean`；模型表头注释、`repo-archi-model` 计数表、ADR `§1`/`§9` 三处同值；`api:export` 幂等（`INDEX.toml` 无残留 diff）；`archi_api_check` 的 `apiFindings` 增量恰为「新元素一条 `docMissing`」（warn，不进 `passed`）。

### AC-13 · 版本与范围纪律

`packages/desktop/package.json` 的 `version` 相对落地时 `HEAD` 恰 +1 patch（`0.7.5` → `0.7.6`）；`tauri.conf.json` 仍 `"version": "../package.json"`；`src-tauri/Cargo.toml` 的 crate 版本行逐字不变；`git status --porcelain` 的改动集 ⊆ 本变更范围所列文件；`openspec/specs/` 下除 5 份 delta 外零 diff。

### AC-14 · 诚实边界在案

spec 与本文档均写明：进程不在场不产生通知、持久化 = 跨重启 + 上一窗口期补拉、OS 通知不在本变更、ui 反馈不跨窗口广播；`tauri-plugin-notification` 零依赖、`capabilities/**` 零 diff。

---

## 风险

| # | 风险 | 处置 |
|---|---|---|
| R-1 | **`Feedback` 只落库不展示** 会被读成「死数据」：它不进列表、不进未读，唯一消费面是 db inspector 与将来的通知中心过滤 | 显式接受为 A2 的形态（用户原话要「持久化写入」）；spec 明写口径与理由；若评审判为投机，可**就地退回 A3 的一半**（`notify` 只 toast 不落库）——改动仅限门面一个函数体 + `notification_record` 命令删除，其余零影响 |
| R-2 | **零修剪 ⇒ 无界增长**（`Feedback` 是主要增长源） | 登记为已知限制（列表分页 + `limit` 上限 + db inspector 可观察）；修剪（cap / TTL）属独立变更，MUST NOT 在本变更顺带加 |
| R-3 | **`Ask` 主键退化**：`tool_call_id` 缺位时退 `created_at` 毫秒，理论上同毫秒两条会互相覆盖 | `tool_call_id` 缺位只出现在旧线载荷（新流程恒有）；覆盖后果为「两条等待通知合一」，不丢流程（页面 AskCard 才是权威面）；登记为已知边界，MUST NOT 为此加 run 内计数器 |
| R-4 | **既有测试断言面连带**：门面若误传 sonner `id`，19 处精确参数断言同时变红；`app.test.tsx` 的「命令序列 / 调用计数」断言会因铃铛挂载新增两条命令而失败 | AC-2 把「不传 id」写成机械判据；`app.test.tsx` 的序列断言与计数断言同批收窄或补期望序（列在测试文件表）；其余 6 个 toast 测试原则零改动 |
| R-5 | **user 维度跨工作区收件箱的 UX 语义**：某条通知属别的工作区，点击要先切根 —— 切根会重置当前视图（详情页 → 清单） | 采纳 explore §8.5 方案 B 并在 spec 写明；`source_ref` 不可达时行惰性；若评审要求最小面可退方案 C（跨根行不可点），改动限于铃铛一处点击处理 |
| R-6 | **每次 toast 一次 IPC**（fire-and-forget） | 落在既有 `invokeGuarded` 面，参数校验本地通过才发；失败静默 + DEV 一行 `console.warn`；MUST NOT 为省这一次往返而缓存批量写（会引出队列与丢批语义） |
| R-7 | **模型计数三处漂移**（表头 / `repo-archi-model` 表 / ADR） | AC-12 把三处同值写成判据；读数在本提案内已算死（见「能力」章的逐文件增量），实现时逐项核对 |
| R-8 | **新元素无契约资产** | `api` 取缺省 `declared` + 不写契约 ⇒ 恰一条 `docMissing`（warn，永不纳管；`status` 仍 clean）。契约资产属独立变更，MUST NOT 在本变更新建 |
| R-9 | **「通知是旁路」被实现成吞错** | 三条不变量 + `StoreSink` 的 `eprintln!` 降级 + 前端 DEV `console.warn` 给出可观测面；MUST NOT 因落库失败改业务返回值（`change_flow_*` 签名零变化可断言） |
| R-10 | **广播滞后丢条**（`broadcast` lagged）⇒ 未读少一条 | 与 `change_flow_watch` 同口径：Channel 只是增量面，抽屉打开时的 `notification_list` 是权威补拉；spec 明写「通知通道不是未读的唯一真相源」 |
| R-11 | **死未读清理落在组合根**（`ChangeFlowSink::emit` 的 `Finished` 分支调 `retire_run_notifications`）与「壳层零域规则」张力 | 规则陈述归 spec、实现为一次机械调用（照 `ask_resolved` 清理先例）；失败静默（`let _`），MUST NOT 让 walker 终态依赖它 |
| R-12 | **铃铛新增文件落模型 `path` 之外** ⇒ 静默少节点、边全丢（历史病灶） | 变更范围已把 `06-frontend.toml` 的 `path` 追加与 `Shell → Drawer` 边写死；AC-12 的读数即判据 |
| R-13 | 文案口径二源（core 投影 vs spec） | spec 只钉「level 映射 + message 必含变更名 + 相位名取自相位表」，不复述字面；字面唯一真相 = `core/orchestration/src/notify.rs` |
| R-14 | 通知数量对 `change_flow` 终态路径的时序敏感（`Finished` 与 store 写竞态） | 通知落库与 run 落库互不依赖（后者在 walker 落库缝）；`retire_run_notifications` 读不到行即无操作（幂等），零顺序不变量 |

---

## explore 未决项的归属

| explore 条目 | 处置 |
|---|---|
| §三 A 轴（写入权） | **A2 采纳**；A1 否决（把瞬时反馈变异步链路 + 文案托付后端），A3 作为 R-1 的退路留档 |
| §三 B（通知 vs 操作反馈是否同一 model） | **同一 model + `kind` 字段**（`Event` / `Feedback`）；不拆表（过滤与未读口径靠 `kind` 单点） |
| §三 C（持久化维度） | **user 维度**（跨工作区收件箱、切根不丢未读）；`root` 作字符串归属（`ExploreRecord` 先例） |
| §三 D（风暴 / 重复 / 增长） | 风暴 = 三变体白名单（`Step`/`Edge`/`SessionEvent` 永久排除）；重复 = 主键即幂等串；增长 = **不修剪**（R-2） |
| §三 E（sink 同步性与失败隔离） | 落为三条不变量（AC-1） |
| §三 F（应用不在场） | 落为独立 requirement（§六 诚实边界，AC-14） |
| §四 渠道扩展点 + 渠道配置落点 | **本变更零渠道配置**：不新增 `KNOWN_KEYS`、不建 `infra/notification`、不做「全局设置」；第二渠道到场才裁决 |
| §七 7.1 agent 会话收口接入 | **首批不接**（闭包里只剩 `session_id`，补 root / source_ref 需改 drive 尾；调试页会话是噪声）；登记为范围外 |
| §七 7.2 回环抑制（origin ∈ {ui, backend}） | **不需要 origin**：ui 反馈不经 Channel 广播（只本窗口 toast），无环可抑；`kind` 已足够区分两条轨 |
| §七 7.2 前端铸 id 与 toast 同 id | **保留前端铸 id**（落库主键），**取消「toast 同 id」**（不传 sonner id，保 19 处断言零改动） |
| §七 7.5 ① ui 反馈进不进收件箱 | **不进**（只落库）；理由与退路见 R-1 |
| §七 7.5 ② 落库失败的留痕面 | 后端 `eprintln!` 一行、前端 DEV `console.warn` 一行（不新立观测面） |
| §七 7.5 ③ 文案面 message / body | 保留两段（sonner `description` 对应 `body`），既有 `use-change-list` 的 warning 形态零变化 |
| §七 7.5 ④ 保留策略 | **不修剪**（R-2） |
| §七 7.5 ⑤ 未读口径 | 只数 `kind = Event` 且 `read_at` 为空；逐条 + 「全部已读」并存 |
| §八 8.4 铃铛落位 | **新文件 + 模型 path 追加**（不内联进 `app.tsx`，`max-lines-per-function` 与页壳可读性） |
| §八 8.5 跨根跳转 | **方案 B**（切根后跳转） |
| §八 8.6 ① 全部已读形态 | **专命令** `notification_mark_all_read`（语义清晰，少一次列表回传） |
| §八 8.6 ② 列表容量 | 首屏 50 + 页界信封；滚动加载归后续（不阻塞本变更） |
| §八 8.6 ③ 徽标 | 单一语义「未读总数」，上限 `99+` |
| §八 8.6 ④ 时间显示 | 行内 `new Date(ms).toLocaleString()`，不新立 formatter |
| §九 9.5 ① Stopped 弹不弹 | **落库不弹**（机械规则：`Info` 不弹 toast，零新增字段） |
| §九 9.5 ② Ask 的 options | **不进 body** |
| §九 9.5 ③ 相位人话标签 | 复用相位表 `PhaseDefinition.description`；MUST NOT 在通知侧另立翻译表 |
| §九 9.5 ④ 多轮 ask | 主键带 `tool_call_id`（缺位退 `created_at`，R-3） |
| §九 9.5 ⑤ Finished 清死未读 | 采纳为规则（AC-6） |
| §十 10.6 三处留待裁决 | 逐条收敛（`Info` 不弹 / options 不进 body / 相位名取相位表） |
| §五 机械成本清单 | 全部落进「变更范围」；其中「通知中心独立页 + 路由 + 侧栏项」改为**顶栏抽屉**（§八 决议），故前端零路由改动 |
