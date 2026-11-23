# 后端通知模块：从散落的 toast 到「通知扇出 + 持久化收件箱」

workflow_type: requirement

探索目标（用户原话）：后端增加通知模块，收束桌面端通知（通用 api -> toast），支持持久化写入。
未来可能扩展其他通知渠道（远程 api、其他应用 cli、写文件）。

> **已裁决（本轮）**：采用 **A2 —— 前端门面 + 后端事件双写**。前端 `notify()` 同步
> toast、异步落库；后端事件走 `publish` → Channel → toast。接线细节见第七节。

---

## 一、现状盘点（代码实证）

### 1.1 后端：零通知概念

```
$ grep -rn "notification|Notification" --include=*.rs src crates    # 命中 0
```

后端没有任何「通知」词汇、表、命令、trait。今天能被称为「通知」的只有三条**薄信号**通道
（全部 `tauri::ipc::Channel`，零 emit/listen —— `src/bindings/mod.rs` 的显式纪律）：

| 命令 | Channel 载荷 | 语义 |
|---|---|---|
| `agent_start` / `agent_session_watch` | `AgentRunMessage` | 会话流式事件 + 终态轮行 |
| `change_flow_start` / `change_flow_watch` | `RunNotice`（kind-only） | 「统一视图请重查」的失效信号 |
| `watch_subscribe` | `FileWatchEvent`（无载荷） | 单文件失效信号 |

教训已经写在代码里（`use-file-watch-signal.ts` 的「划界原则」）：**通道按驱动源切分，
不按「都是信号」切分**。通知模块要插进来，必须先回答「它的驱动源是谁」。

### 1.2 前端：12 处 sonner 直调，5 个文件

```
hooks/use-workspaces.ts              ×2   toast.error(`添加 workspace 失败：…`)
views/agents/hooks/use-agent-instances.ts ×3  save / remove / setDefault
views/agents/hooks/use-agent-providers.ts ×2  save / remove
views/changes/hooks/use-change-list.ts    ×2  success / warning（删除变更）
views/architecture/components/contract-panel.tsx ×1  toast(`写入 N 份…`)
views/explores/components/explore-open-file.tsx  ×2  toast.error(String(err))
```

`<Toaster />` 单点挂在 App 根（`components/ui/sonner.tsx` → `app.tsx`）。测试面按
「toast.error 被调用 / `[data-sonner-toast]` DOM 存在」断言，散布 10 个测试文件（约 90 处
断言，其中大量是「零 toast 基线」的 `not.toHaveBeenCalled`，改门面后仍成立）。

### 1.3 通知的「话题来源」其实已经在后端产生了

后端今天知道这些**用户可能不在场**的事件，却只把它们当「视图失效信号」丢掉：

```
change flow run（change_flow_start 早 resolve，walker 后台跑）
   ├─ WaitingAsk / WaitingConfirm  ← 用户必须来一下，否则流程卡死
   └─ Finished(Completed | Stopped | Failed)  ← orchestration::state::ChangeRunStatus
agent 会话（agent_start 早 resolve，内核泵后台跑）
   └─ converge → SessionStreamRegistry 投终态
checks / mutation / archi gate 长跑步骤（flow 内部步，终态进 run 图）
```

`RunNotice::Finished` 与 `ChangeRunStatus::is_terminal` 就是天然的接入点：**恰好是
「前端可能已切页/最小化」的时刻**。

### 1.4 持久化的现成底座

`crates/infra/store`（native_db / redb）已有双维度布局：

- **user 维度** → 全局库 `desktop-global.redb`，现有 3 模型（workspace 注册表 / provider / agent 实例）
- **workspace 维度** → `workspaces/<identity>.redb` 每工作区一库，12 模型

新增一个模型要动的四个点（机械、已成例）：

```
1. model.rs          定义 struct + 字段 doc 注释（表/字段说明全派生，禁手写文案）
2. store.rs          global_models() 或 workspace_models() 里 define::<T>()
3. envelope.rs       MODEL_ENTRIES 登记一行（实际是 count/scan/key_of/schema/walk 五指针）
4. Store 方法        publish / list / mark_read 等读写面
→ db inspector 页面自动出现该表（零模型特定代码）
```

字段演进有 native_model 的 `id/version` 升级链（`AgentRunRecord` v3→v4 先例：无迁移代码路径）。

### 1.5 架构红线（决定落地只有一种形状）

```
core/*   禁 Tauri、禁 spawn、禁 tokio 运行时（foundation 甚至零 tokio）
infra/*  禁 Tauri；可用 std 线程 / 需要时用 tokio
壳(src/) 唯一允许 Tauri 的地方（命令面、Channel、注册表挂载）
```

⇒ 「toast」这个渠道在物理上只能落在**壳 + 前端**：core 描述「有一条通知」，infra 负责
持久化/外部投递，壳把「in-app」那一份翻成 `Channel`，前端 sonner 只做渲染。

---

## 二、目标形态草图

```
                     ┌──────────────────── 壳 (src/) ────────────────────┐
                     │ commands::notifications                            │
  前端 sonner  ←─Channel──  notification_subscribe(on_event)  列表/未读查询 │
  (渲染 sink)        │        notification_list / notification_mark_read   │
                     │        notification_publish  (前端反馈型消息入口)   │
                     │                                                    │
                     │  InAppSink: broadcast → Channel 转发（同 change_flow）│
                     └───────────▲────────────────────────▲───────────────┘
                                 │                        │
                   ┌─────────────┴──────────┐   ┌─────────┴────────────────┐
                   │ core/notification      │   │ infra/notification       │
                   │  Notification 词表      │   │  StoreSink（持久化 inbox）│
                   │  Severity / Kind        │   │  RemoteApiSink（未来）    │
                   │  Hub 扇出 + 去重 + 保留 │   │  CliSink（未来）          │
                   │  trait Sink { deliver } │   │  FileSink（未来）         │
                   └────────────▲───────────┘   └──────────▲───────────────┘
                                │                          │
             事件源（谁 publish）                          │store crate
   flow Walker 终态 / ask-confirm 等待 / 会话收口 / gate 失败 ┘
```

一条通知的旅程：

```
publish(Notification)
  ├─[1] 铸 id + 落库（infra StoreSink；失败不吞，但也不阻断业务）
  ├─[2] in-app 扇出（broadcast → 每窗口 Channel → 前端 toast）
  └─[3] 其他 sink（顺序无关、失败隔离、各自重试语义自理）
       ↑ 前端此刻可能没有窗口/没订阅 ⇒ 未读靠 [1] 补拉，这正是持久化的价值
```

---

## 三、关键分歧轴（需要裁决的地方）

### A. 谁拥有「写入权」——收束到什么程度？

| 方案 | 前端反馈型消息（12 处 toast.error） | 后端事件型消息 | 代价 |
|---|---|---|---|
| **A1 后端唯一写入点** | 前端 `notify()` → IPC `notification_publish` → 统一扇出 → 回环 Channel → toast | 原生 | 每次 toast 多一次 RTT（动作失败 toast 变「慢半拍」）；错误文案由前端拼好后托付后端（后端只是信箱）；测试面整体改写 |
| **A2 双写入点（前端门面 + 后端事件）** | 前端 `notify()` 同步 sonner，另 fire-and-forget 落库 | 后端 publish → Channel | 两条写路径共用同一 model，需 dedupe/id 幂等；「收束」= 前端门面收束而非后端收束 |
| **A3 只收束前端门面，后端只做事件通知 + inbox** | 纯内存 toast，不进 inbox | 进 inbox | 最省；但「持久化写入」只覆盖后端事件，用户看不到历史动作反馈 |

我的读数：A1 是唯一能把「未来渠道（写文件/远程/CLI）」自动覆盖到前端消息上的形状，
但把一次瞬时反馈变成了异步链路；A2/A3 保住交互手感却在语义上留下两块地。

### B. 通知与「操作反馈」是不是同一个 model？

现有 12 处 toast 里其实有三类语义：**动作失败**（error 前缀 + `String(err)`）、**动作成功**
（删除变更、写契约文件）、**被动错误**（打开文件失败）。加上后端的**事件型**（run 终态、
等待确认）。可用一个 `kind` 字段区分，也可以拆两张表。影响 inbox 的过滤与「未读数」。

### C. 持久化维度：全局收件箱 vs 工作区收件箱

- **user 维度（推荐候选）**：跨工作区一个收件箱，切换 workspace 不丢未读；记录自带 `root`
  字符串归属（`ExploreRecord` 先例），但 store 禁跨库 join ⇒ 详情靠字符串寻址。
- **workspace 维度**：与 ChangeRecord/SessionRecord 同库，join 自然，但用户切根后「别的工作区
  跑完的 flow」通知去哪看？

### D. 保留与去重策略（不做会出事的三个点）

- **风暴**：一个 flow run 会产生大量 Step/Edge 事件——只有终态/等待类该进通知，`RunNotice`
  的 kind 面必须显式白名单。
- **重复**：重挂/重进页面会重放？重复 publish 同 (root, change, run, kind) 需要 dedupe 键。
- **增长**：cap（每 kind 保 N 条）或 TTL；修剪时机（publish 时 / 启动时）与 unread 计数的一致性。

### E. sink 的同步性与失败隔离（core 不能 await）

core 无 tokio 运行时 ⇒ `trait Sink { fn deliver(&self, n: &Notification); }` 只能同步入队，
IO/网络 sink 在 infra 内部自己起线程/`tokio::spawn`。横切规则要在 core 里写成不变量：

- 一 sink 失败 MUST NOT 影响其他 sink，MUST NOT 让业务动作失败（通知是旁路）；
- core 只保证「投递一次」，重试/退避/批量归 sink 内部；
- 顺序：同 sink 内 FIFO 即可，跨 sink 不保证。

### F. 应用不在场时的不可能面（要写进 spec 的诚实边界）

后端进程没起 ⇒ 不可能产生通知。持久化的真实价值是：**跨重启保留历史 + 上一窗口期未读补拉**，
不是「进程外通知」。若要做系统级 OS 通知（应用最小化），那是壳层
`tauri-plugin-notification`（当前未安装、无 capability 配置）——另一个 sink，另一份裁决。

---

## 四、渠道扩展点（未来三渠道的可行性预判）

| 渠道 | 落点 | 需要的额外机制 | 风险 |
|---|---|---|---|
| in-app toast | 壳（Channel）+ 前端 sonner | 窗口订阅注册表（学 `SessionStreamRegistry`） | 已成型 |
| 写文件 | infra/notification 的 FileSink | 追加写 + 轮转；路径经 `foundation::layout` 派生 | 低 |
| 其他应用 CLI | infra/notification 的 CliSink | `infra/exec` 已有 argv 直传边界；超时/退出码/不阻塞 | 中（进程卫生） |
| 远程 API | infra/notification 的 RemoteApiSink | HTTP 客户端（锁内无 reqwest 直依赖，rig 里带 `reqwest`）；密钥从 workspace config 读 | 中高（凭据/重试/离线队列） |

渠道开关与参数天然落在 **workspace config** 的新顶层键（`KNOWN_KEYS` 是手维护 8 键 +
`schema.json` 镜像 + 一致性测试双向守）；「全局设置」目前**没有落点**（data-dimensions 的
「未来租户：设置」仍未落地）——这可能是本变更的第一个岔路口。

---

## 五、机械成本清单（本仓的「隐形工作量」）

```
后端
  [ ] crates/core/notification/   新 crate（词表 + Hub + Sink trait + 去重/保留纯逻辑 + 测试）
  [ ] crates/infra/notification/  新 crate（StoreSink 等 adapter + 测试）
  [ ] Cargo.toml workspace members + workspace.dependencies 两行
  [ ] store: NotificationRecord + global_models/MODEL_ENTRIES/Store 读写方法 + model_test
  [ ] 壳: src/commands/notifications/ + all_commands! 登记 + setup 挂通知注册表
  [ ] bindings:export → src/types/generated/{bindings.ts,bindings.zod.ts} 刷新（+ zod-check）
前端
  [ ] lib/notify.ts 门面（替换 5 文件 12 处直调 sonner）
  [ ] hooks/use-notifications.ts（Channel 订阅 + 未读 + 列表刷新）
  [ ] 若加通知中心：views/notifications/index.ts 入口（barrel）+ routes + sidebar 项（spec 同步改）
  [ ] 测试面：use-workspaces / use-agent-* / use-change-list 等 toast 断言改写
架构 / 规格
  [ ] .dev-team/architecture/graph/model/02,03,04,06.toml 新元素 + relation 边（core/infra/壳/前端）
  [ ] api:export 重生成 INDEX.toml（+ 可选契约 md、.i.rs/.i.ts 生成物）
  [ ] openspec: 新 capability spec（如 desktop-notification-inbox）+ 相关 spec 交叉引用
  [ ] packages/desktop/package.json version patch bump
```

---

## 六、留给下一轮的问题

1. 「收束」的判据是什么——(a) 前端零 `import { toast } from 'sonner'`（除 Toaster 单件），
   还是 (b) 全仓只有后端一处 `publish`（前端动作反馈也回环）？
2. 前端动作反馈要不要进收件箱？（噪声 vs 完整性）
3. 收件箱在哪看？顶栏铃铛 + 抽屉 / 独立 `views/notifications` 页 / 只在 toast 里历史（不做 UI）？
4. 第一批接入哪些后端事件？最小可信集合候选：flow `WaitingConfirm` / `WaitingAsk`、
   flow 终态 Completed/Failed/Stopped、agent 会话收口。
5. 渠道配置的落点：workspace config 新键 vs 先只做 in-app（零配置），把渠道注册表留成
   代码内静态表直到第二个渠道到场（「第二个消费者到场才下沉」的仓库惯例）。

---

## 七、A2 决议后的接线细节（本轮追加，全部有代码坐标）

### 7.1 两条流，各自的接点已经找到

```
① 前端反馈轨（同步 toast，异步落库）
   use-workspaces / use-agent-instances / use-agent-providers /
   use-change-list / contract-panel / explore-open-file
        └─→ src/lib/notify.ts        ← 新文件，5 文件 12 处直调 sonner 收口到此
               ├─ toast[level](msg, { id, description })   // 同步，手感不变
               └─ void ipc.notificationRecord({ id, … })   // fire-and-forget 落库

② 后端事件轨（publish → 落库 → Channel → toast）
   ChangeFlowSink::emit(RunUpdate)         ← src/commands/change_flow/mod.rs
      match { Ask | ConfirmWait | Finished }            （Step / Edge 显式排除＝风暴闸门）
        └─→ core/notification Hub.publish(Notification::run_*（…）)   // 文案在 core，壳零规则
               ├─ StoreSink：写 user 维度 inbox（失败静默，不阻断、不二次 toast）
               └─ InAppSink：broadcast → 订阅命令 → Channel（照抄 spawn_channel_forward）
```

两个接点的性质不同，值得记一笔：

- **`ChangeFlowSink` 是现成的适配器**（已是 `RunEventSink` + `AskPort` 两 trait 视图），
  加通知只是第三个消费者，零新线程、零新注册表。
- **agent 会话收口（`agent_start` 的 `running.drive(...)` 尾）是弱点**：闭包里只剩
  `session_id`，要造有意义的通知得补 root / source_ref 上下文，且调试页会话
  （`source=debug`）大概率是噪声。**建议首批不接**，或按 `source` 白名单（只接
  `change` / `explore`）。

### 7.2 id 与幂等（A2 的唯一机械难点）

```
前端铸造 id：crypto.randomUUID()
   toast(msg, { id }) 与 IPC 落库同 id
   ⇒ 后端 upsert-by-id，重投不产生第二条

后端事件：dedupe_key = root | change_id | run_id | kind
   ⇒ 同 run 的 Completed 只留一条；id 可再生（`ntf-{millis}-{seq}`，沿 ses- 先例，零新依赖）

回环抑制：Channel 消息带 origin ∈ { ui, backend }
   前端只对 origin = backend 弹 toast；origin = ui 只刷未读
   ⇒ 既不双 toast，也不靠 sonner「同 id = update」兜底（那会重置计时、闪一下）
```

多窗口语义顺带定下：某窗口发起的 ui 通知，其他窗口**只涨未读、不弹 toast**。

### 7.3 数据结构草案（user 维度）

```rust
#[native_model(id = N, version = 1)]     // id 须与全局库现有 1..3 号模型不冲突
#[native_db]
pub struct NotificationRecord {
    #[primary_key] pub id: String,          // 发起方铸造（见 7.2）
    pub kind: NotificationKind,             // Feedback（前端动作）| Event（后端事件）
    pub level: NotificationLevel,           // Info | Success | Warning | Error（对位 sonner 四档）
    pub message: String,
    pub body: Option<String>,               // sonner description；后端事件可放补充说明
    pub origin: NotificationOrigin,         // Ui | Backend
    pub root: Option<String>,               // 工作区归属字符串（禁跨库 join 惯例）
    pub source_ref: Option<String>,         // change id / run id / session id
    pub dedupe_key: Option<String>,
    pub created_at: i64,
    pub read_at: Option<i64>,
}
```

表/字段说明由 `///` doc 自动派生（`describe.rs` 机械面），`MODEL_ENTRIES` 登记一行
（实际要 count/scan/key_of/schema/walk 五指针）+ `global_models()` 一行
+ Store 三方法（`publish` / `list` / `mark_read`）+ db inspector 自动出表。

### 7.4 A2 会碰到的既有纪律（逐条已核）

| 纪律 | 出处 | A2 的应对 |
|---|---|---|
| db 轨道只读 | `commands/db/mod.rs` 头注释 | 写命令新开 `commands/notifications/` 轨道，绝不并进 db 轨 |
| 全 Channel、零 emit/listen | `src/bindings/mod.rs` | 订阅命令走 `Channel<NotificationEvent>` 形参（照 `watch_subscribe`） |
| core 禁 Tauri / 禁 spawn | workspace Cargo 注释 + 多层 crate 头注释 | Hub 同步扇出；IO sink 自己排队，core 不 await |
| 通知是旁路 | —（本变更要新立的不变量） | 落库失败静默；MUST NOT 让业务动作失败；MUST NOT 为「落库失败」再发通知（递归） |
| 前端入口 = 公共面 | desktop-frontend-module-entry | `lib/notify.ts` 不在 44 个 path 元素内（`@/lib/*` 直引），零 barrel 成本；但受 knip unused-export 纪律 |

### 7.5 A2 新长出来的待裁决问题

1. **ui 反馈要不要进收件箱**：每天几十条「保存失败」会把 inbox 淹了。候选：`kind=Feedback`
   只留最近 N 条且默认不进列表 / 干脆不落库（那就退回 A3 的一半）。
2. **落库失败的留痕面**：静默是必须的，但完全无痕不可调试。候选：DEV 下 `console.warn`
   一行 / 复用 `lib/ipc-drift.ts` 的观测面（那里已经是「soft-report、永不 throw」的现成形状）。
3. **文案面**：单 `message` 还是 `message + body` 两段（sonner 支持 description，现有
   `use-change-list` 的 warning 已经在用 `description` 拼 warnings）。
4. **保留策略**：cap N（写事务内删最旧）vs TTL（启动时清）vs 先不修剪（观察一周再定）。
5. **未读的口径**：只数 `kind=Event`？`read_at` 逐条标记还是「一键全读」为主？

### 7.6 下一轮可直接做的落地调研（都是可验证的小实验）

- sonner 2.0 同 `id` 的更新语义实测（决定回环抑制是不是必须的，还是同 id update 已足够）
- `SessionRecord.id`（String 主键 + `ses-…` 铸造）→ 通知 id 同型落库的读写面成本
- 全局库写事务里「insert + count + 修剪最旧」的原子性写法（照 `begin_run` 的 max+1 事务先例）
- 窗口关闭/重开时 Channel 订阅的生命周期（`change_flow_watch` 的补订语义说明「注册表在飞
  条目 + 统一查询兜底」这一对，通知是否同样需要「订阅前先补拉」的顺序不变量）

---

## 八、收件箱 UI 决议：顶栏铃铛 + 抽屉（本轮追加）

### 8.1 落位与复用

```
app.tsx
  ├─ ShellHeader（已有内联件：SidebarTrigger / DEV 徽标 / UpdateIndicator）
  │     └─ NotificationBell ← 新增（铃铛 + 未读数徽标 + StandardDrawer 抽屉）
  ├─ <Toaster />  恒不动（sonner 仍是唯一渲染器；门面内部才 import sonner）
  └─ AppRoutes    零改动（不做新路由）

components/standard/drawer.tsx  ← 现成 StandardDrawer
    props: title / content / trigger / open / onOpenChange / modal / containerClassName
    content 自带 ScrollArea + 「关闭」footer —— 收件箱列表直接灌进去，零新原语
```

`StandardDrawer` 的 `trigger?: UseRenderRenderProp<DrawerTriggerState>` 形态正好接一个
图标 Button，与既有 `TextTooltip` 的用法同族。

### 8.2 命令面（新轨道 `commands/notifications/`）

| 命令 | 形态 | 说明 |
|---|---|---|
| `notification_subscribe(on_event: Channel<NotificationEvent>)` | 订阅 | 直播增量；照 `watch_subscribe` / `change_flow_watch` 形 |
| `notification_list(limit, before_id?)` | 读 | 抽屉打开时补拉历史（倒序，页界信封） |
| `notification_unread_count()` | 读 | 铃铛徽标初值（订阅后本地自增，无需每次查） |
| `notification_mark_read(ids: Vec<String>)` | 写 | 点单条 / 全部已读（空数组 + `all` 语义二选一，见 8.6） |
| `notification_record(id, level, message, body?)` | 写 | 前端反馈轨落库（fire-and-forget，返回 `Result<(), String>`，前端不等待） |

写命令落在**本轨道**而不是 db 轨（db 轨只读纪律，见 7.4）。

### 8.3 前端数据流（一次性补拉 + 增量）

```
use-notifications（新 hook，src/hooks/ 不建边，零元素成本）
   mount → unread_count（徽标）+ subscribe(Channel)
   drawer open → list（倒序首屏 N 条）
   Channel 消息 → prepend 列表 + unread+1；origin=ui 只 unread+1 不 toast
   mark_read → 本地置灰 + IPC 写
```

丢弃面：订阅失败静默（不阻断壳），列表失败 inline error（与 db inspector 同口径）。

### 8.4 模型声明的真实成本（这里有个坑）

`App.Frontend.Shell` 的 `path` 是**显式文件枚举**（`app.tsx` / `routes.tsx` / `main.tsx` /
`components/app-sidebar.tsx`）。铃铛组件落位两选：

| 落位 | 架构模型改动 | 代价 |
|---|---|---|
| ① 内联进 `app.tsx`（ShellHeader 扩容） | **零**（文件已在 path 内） | `app.tsx` 变长（抽屉列表渲染 + hook 装配挤在页壳） |
| ② 新建 `components/notification-bell.tsx` | Shell 的 `path` 数组**追加一项**（同 `app-sidebar.tsx` 先例）+ 若 import `StandardDrawer` 需补一条 `Shell → Components.Standard.Drawer` 边 | 多一个文件 + 两条模型行；收益是页壳不膨胀 |

判据出处：未在 `path` 内的源文件会落 `unmatchedFiles` 且**不产出任何边**
（`openspec/changes/archive/2026-11-17-frontend-component-element-coverage` 的病灶一）。
即「隐式新增组件文件」= 图上少一个节点 + 入边出边全丢，且增量模式扫描时静默。

### 8.5 跨工作区的点击跳转（收件箱是 user 维度才会冒出来的问题）

user 维度 inbox ⇒ 列表里会出现「别的工作区」的通知。点一条想要跳 `/changes/:id`，
得先切根（`useWorkspaces().select(root)` 是纯本地切换，零 IPC），再 navigate。
候选策略：

- **A** 列表按当前 root 过滤（简单，但失去「跨工作区收件箱」的全部价值）
- **B** 全部展示 + 跨根条目点击时先 `select(root)` 再跳（体验好，需 useWorkspaces 在
  壳层可见——`app.tsx` 已持有 workspaceState，天然可传）
- **C** 全部展示但跨根条目不可跳（只读文本）——最省，可作为第一步

### 8.6 铃铛抽屉带来的新小裁决

1. 「全部已读」的形态：`mark_read(ids)` 逐条 vs `mark_all_read()` 专命令（后者少一次
   列表回传，但多一个命令面）。
2. 列表容量与分页：抽屉里滚动加载（页界信封）还是首屏 cap N（如 50）就够——未读数与
   列表容量脱钩（徽标可能是 999+）。
3. 徽标上限（99+）与颜色：level 有四档，徽标只用「未读总数」一种语义（建议）。
4. 时间显示：仓库惯例是行内 `new Date(ms).toLocaleString()`（无共享 formatter），
   通知行沿用即可，不为它新立 `lib/format-time.ts`。

---

## 九、通知词表与首批事件集（本轮追加）

### 9.1 依赖图：可以只加一个 core crate

```
                 core/notification      （零 workspace 内部依赖；serde + specta）
                   ▲                ▲
                   │                │
   core/orchestration            infra/store
   （RunUpdate → Notification      （NotificationRecord 字段直接复用词表枚举
     的投影函数住这里）              + publish/list/mark_read/unread 四方法）
                   ▲                ▲
                   └──── src/ (壳) ──┘   ChangeFlowSink 调投影 → Hub.publish
                                        InAppSink（broadcast → Channel）实现 core trait
```

关键判断：**首批不新增 `infra/notification` crate**。理由是本仓的「第二个消费者到场才
下沉」惯例——写文件 / CLI / 远程这三家没到场前，开了 crate 只会是一个空壳加两份清单。

- `store` 依赖 `notification`：与其现有风格一致（记录字段直接用 `agent::AgentRunStatus`
  / `workflow::model::Verdict` 这类领域枚举），零映射层。
- `orchestration` 依赖 `notification`：**投影是流程领域的知识**（「什么时刻值得喊人、
  喊什么」），放 core 而不是壳——壳层零域规则是硬纪律。方向为引擎 → 旁路，旁路不反向
  依赖引擎，所以 notification 可被将来任意 crate 复用。
- 壳只做两件事：把 `RunUpdate` 喂给投影函数、把 Hub 的两个 sink 装起来。

### 9.2 词表骨架

```rust
pub enum NotificationLevel  { Info, Success, Warning, Error }   // 对位 sonner 四档
pub enum NotificationKind   { Event, Feedback }                 // 后端事件 / 前端动作反馈
pub enum NotificationOrigin { Backend, Ui }                     // 回环抑制判据（见 7.2）

pub struct Notification {
    pub id: String,
    pub kind: NotificationKind,
    pub level: NotificationLevel,
    pub message: String,
    pub body: Option<String>,
    pub origin: NotificationOrigin,
    pub root: Option<String>,
    pub source_ref: Option<String>,     // change id / run id / session id
    pub created_at: i64,
    pub read_at: Option<i64>,
}

pub trait Sink { fn deliver(&self, n: &Notification); }   // 同步、无返回值、不 await
pub struct Hub { sinks: Vec<Arc<dyn Sink + Send + Sync>> }
impl Hub { pub fn publish(&self, n: &Notification) { for s in &self.sinks { s.deliver(n) } } }
```

三条不变量写进 core 的 doc 而不是靠调用方自觉：**一 sink 失败不影响其他**、**通知是旁路
（不阻断业务、不为自身失败发通知）**、**core 只保证投递一次（重试/退避归 sink）**。

### 9.3 首批事件白名单（`RunUpdate` 六变体实测）

`ChangeFlowSink::emit(RunUpdate)` 拿到的是**带载荷**的全量更新（`FlowNotice` 的
kind-only 降位发生在它之后），所以文案可以从载荷里长出来：

| `RunUpdate` | 入通知 | level | message 草样 | body 来源 | 幂等 id |
|---|---|---|---|---|---|
| `Step` | ✗（风暴） | — | — | — | — |
| `Edge` | ✗（风暴） | — | — | — | — |
| `SessionEvent` | ✗（转录已有面） | — | — | — | — |
| `Ask { ask }` | ✓ | Warning | 变更「X」在等你的回答 | `ask.question`（+ options 摘要？） | `run:{run_id}:ask` |
| `ConfirmWait { phase, archive }` | ✓ | Warning | 变更「X」停在 {phase} 前等确认 | `archive` 的未过相位 / 缺产物摘要（可选） | `run:{run_id}:confirm:{phase}` |
| `Finished { status, reason }` | ✓ | Completed→Success / Failed→Error / Stopped→Info | `变更「X」全部相位已完成` / `运行失败` / `已停止` | `reason`（仅 Failed） | `run:{run_id}:finished` |

**change 名零额外 IO**：`change_flow_start` 里已经 `store.find_change_record(&id)` 拿到
`record`（还用它校验了 `workflow_type` 与 `worktree`），顺手把 `record.name` 塞进
`ChangeFlowSink` 即可。这是个「不加查询就拿得到」的巧合，值得写进 design 免得后人又去读一次 store。

`ArchivePreflight` 里现成的摘要字段：`completed` / `incomplete_phases: Vec<String>` /
`missing_artifacts: Vec<String>` / `delta_specs` / `worktree` / `branch` / `merge_target`
——body 想写厚一点有素材，想写薄就只留 message。

### 9.4 幂等：把 dedupe 串直接当主键

| 方案 | 机制 | 代价 |
|---|---|---|
| **① id = dedupe 串**（推荐） | 后端事件 id 就是 `run:{run_id}:finished` 这类可读串；重投即 upsert-by-primary-key | 主键不再是「随机标识」，但它本来就只当键用；`read_at` 覆盖需显式保留（重投不得把已读翻回未读） |
| ② `dedupe_key` + 二级索引 | id 独立铸造，投递前按索引查重 | 多一个 `#[secondary_key]` + 一条「先查后写」事务路径（`SessionEventRecord.session_id` 是现成范式） |

前端 `origin = Ui` 的消息 id 用 `crypto.randomUUID()`，天然唯一，不需要 dedupe。

### 9.5 本轮尚未定的小口子

1. **Stopped 的 level**：用户自己按的停止键，弹 Info 是否算噪声？（候选：Stopped 落库但不弹 toast）
2. **Ask 的 options 要不要进 body**：进则 body 变长（选项可能四五条），不进则用户得点进页面看。
3. **ConfirmWait 的 phase 是否翻译成人话**：相位名是 `dev-design` 这类键，直接进 message
   会显得生硬；翻译表放 core/notification 会引入「相位表第二真相」的风险（相位表唯一
   权威在 `workflow::write::phase_table`），候选是复用表里的中文名。
4. **同一 run 的 Ask 多次**（多轮 ask）：`run:{run_id}:ask` 会互相覆盖 ⇒ 幂等串要带
   `ask.tool_call_id` 或轮次。
5. **`Finished` 之后是否顺带清掉该 run 的 Ask/Confirm 未读**：否则一条 run 可能留下
   「等待确认」的死未读。

---

## 十、① 的文字结论（白名单 + 幂等串定稿）

以下为**结论**而非候选，可直接作为 proposal / design 的输入；末尾仍标出三处留待裁决。

### 10.1 前提核实

- **change id 全局唯一**：`workflow::write::create` 铸 `uuid::Uuid::now_v7().to_string()`
  （create.rs 246–248，D1 铸点在前置九道全过之后、建档之前）。⇒ 幂等串只要含 change id
  就跨 workspace 唯一，user 维度全局 inbox **不需要** root 参与去重。
- **run id** 由 `change_flow` 的 `new_run_id()` 铸造，同一 change 内不复用。
- `ChangeFlowSink::emit` 拿到的是带载荷的 `RunUpdate`（kind-only 降位在 `publish` 里发生），
  文案所需字段全部在手上。
- **change 名零额外 IO**：`change_flow_start_with` 已有 `record`（校验 `workflow_type` /
  `worktree` 时读出），把 `record.name` 与 `id` 一并塞进 `ChangeFlowSink` 即可。

### 10.2 首批入通知的 RunUpdate 三变体（其余三个永久排除）

| `RunUpdate` | 裁决 | 理由 |
|---|---|---|
| `Step` | **排除** | 一次 run 数十上百条，是「视图重查」信号不是「事件」 |
| `Edge` | **排除** | 同上 |
| `SessionEvent` | **排除** | 转录另有面（会话面板 / 转录表），通知它等于重复推送 |
| `Ask` | **纳入** | 流程阻塞，用户不来就死等 |
| `ConfirmWait` | **纳入** | 流程阻塞，同上 |
| `Finished` | **纳入** | 长跑结束，用户多半不在页面上 |

排除的实现形态：投影函数对三变体 `return None`，MUST NOT 用 `if` 白名单散落在 sink 里
（判据要可测、要单点）。

### 10.3 三变体的落库形态（定稿）

```
Ask          level=Warning  message="变更「{name}」在等你的回答"      body=? 见 10.6-②
ConfirmWait  level=Warning  message="变更「{name}」停在 {phase_label} 等确认"
                            body=archive 摘要（可选，见 10.6-③）
Finished     Completed → level=Success  message="变更「{name}」已完成全部相位"
             Failed    → level=Error    message="变更「{name}」运行失败"      body=reason
             Stopped   → level=Info     message="变更「{name}」已停止"        body=None
```

`kind` 三变体恒 `NotificationKind::Event`、`origin` 恒 `NotificationOrigin::Backend`、
`source_ref` 恒 change id（`root` 也一并带上，便于 UI 判「是否当前工作区」）。

### 10.4 幂等串（定稿）

主键**直接就是幂等串**（不开 `dedupe_key` 二级索引）：

| 事件 | 主键形态 | 重投语义 |
|---|---|---|
| Finished | `run:{run_id}:finished` | 终态只可能收敛一次；重投 = upsert 同一行 |
| ConfirmWait | `run:{run_id}:confirm:{phase}` | 停等点唯一，同 phase 重投即同一行 |
| Ask | `run:{run_id}:ask:{tool_call_id}`；`tool_call_id` 缺位时退 `run:{run_id}:ask:{seq}`（seq 为该 run 内 ask 计数） | 多轮 ask 各自一行，不互相覆盖 |
| 前端 ui 反馈 | `crypto.randomUUID()`（前端铸） | 天生唯一，无 dedupe 需求 |

配套不变量：**upsert 只更新 `message` / `body`，MUST NOT 把 `read_at` 从 `Some` 翻回 `None`**
（重投不得复活已读）。

### 10.5 「死未读」清理（定稿为规则，实现留给持久化那束）

`Finished` 事件落库时，**同 run 的 `confirm:*` 与 `ask:*` 未读行一并置 `read_at`**
（或标记 `superseded`——二选一看持久化方案）。理由：`WaitingConfirm` 时用户按停止键，
run 收敛到 `Stopped`，收件箱里那条「等你确认」永远等不到人。

### 10.6 三处留待裁决（不影响骨架，只影响文案与显示）

1. **Stopped 是否弹 toast**：用户自己按的停止键，弹出来算不算噪声？
   倾向：**落库但不弹**（`origin` 之外再加一个 `silent: bool`，或前端按 `status` 判）。
2. **Ask 的 options 进不进 body**：进则 body 可能四五行长，不进则用户必须点进页面。
   倾向：**不进 body，只留 question**；选项在页面里已经有 AskCard 承载。
3. **`phase` → 人话标签**：`dev-design` 这类键直接进 message 生硬。翻译表 **MUST NOT**
   落在 notification（相位表唯一权威在 `workflow::write::phase_table`），应复用相位表里的
   中文名——投影函数因此要拿到相位表访问面，或由 `ChangeFlowSink` 侧预先查好传入。

