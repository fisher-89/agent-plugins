# 测试设计: backend-notification-hub

> **变更**: backend-notification-hub（后端通知模块 · 通知扇出 + 持久化收件箱 + 前端门面）
> **来源**: `design.md`（DD1–DD12、事实校准 6 条、「接线自检」、「新增文件」8 项 / 「修改文件」28 行 / 「公共函数 · API」38 行 / 「类型定义」15 行 / 「数据模型」/「路由 · API 设计」）、`proposal.md`（AC-1…AC-14、R-1…R-14）、`specs/{desktop-notification-hub,desktop-app-shell,desktop-workspace-store,desktop-data-dimensions,repo-archi-model}/spec.md`
> **性质**: 一条新旁路能力 + 一次前端门面收口——新 core crate（词表 / 扇出点 / Sink 缝）+ 投影与投递单点（`project` / `deliver` / `RunNotifier`）+ 事件轨注入面（`RunGuard::emit` / `walk_run` 第 9 参）+ user 维度收件箱（六方法 + 落库 sink）+ 新命令轨道（五命令 + `NotificationBridge` 出口）+ 前端门面（12 处直调收口）+ 顶栏铃铛抽屉。
> **被测范围口径**: 取 design「新增文件」8 项与「修改文件」28 行（删除文件零涉及）。清单声明面（4 份 `Cargo.toml`）、模块登记面（2 份 `lib.rs` 登记行）、架构模型资产（4 份 `.toml` + `INDEX.toml` + ADR）与版本行按「无法解析测试路径」口径处理，见末节「不可测试项」。
> **本轮返工（回溯 1 在测试设计侧的收口）**: 旧版本把事件轨断言锚在 `ChangeFlowSink::emit()`——该缝实测只由 worker 对 `SessionEvent` 调用，`ConfirmWait` / `Finished` 经 `RunGuard::emit` → `ChangeFlowControl::publish` 绕开壳层，故那 7 行在旧形态下是「投影有单测、生产链无线」的假绿（`start_sink携带记录四值…` / `start到finished全链路…` 两例因此超时）。现按 design 校准 6 / DD12 重锚：① 事件轨断言拆进 `crates/core/orchestration/src/{notify,control,walker}.rs` 三节（`notify::deliver` 组合点 / `RunGuard::emit` 旁路 / `walk_run` 第 9 参 + 真实 run 链）；② `commands/change_flow/mod_test.rs` 只保留 `AskPort::ask_started` 三行、`RunEventSink::emit` 的「零通知」反锚与装置随动；③ 端到端接线用例落 `change_flow` 的 `start → walk_run → 库行` 一行（design 交接清单点名的那一条）；④ 新增 `NotificationBridge` 用例组接住 core 与 store 之间的最后一跳。
> **可断言边界**: 本设计只锚**机械事实**——扇出顺序与失败隔离、投影白名单与主键字面、投递组合点（`deliver`）与注入面（`RunGuard::emit` / `walk_run` 第 9 参）的接线、upsert 读状态保真、死未读清理、过滤口径与页界、订阅四步顺序、门面四档调用形态与落库旁路、徽标三态与跨根顺序、命令清单随动。**文案质量、通知是否「该喊人」、抽屉观感**不作验收判据（字面唯一真相在 `notify.rs`，spec 只钉 level 映射与「必含变更名」要素）。
> **相位边界**: 本文档只写测试范围 / 断言判据 / mock 策略；不执行静态检查与测试回归，不复制运行脚本细节（`server:check → bindings:export → api:check → client:check` 与 `git diff` 复核由桌面端相位执行）。

## 测试框架与布局

| 域 | 框架 / 断言 | 测试文件形态 | 运行入口 |
|---|---|---|---|
| Rust 核心与基础设施（`packages/desktop/src-tauri/crates/core/**`、`crates/infra/**`） | 标准 `#[test]` + `assert!` / `assert_eq!`；无外部断言库；测试名用中文短语直述判据 | 同目录兄弟文件 `<模块>_test.rs`，经 crate 根 `#[cfg(test)] mod <模块>_test;` 登记（`crates/core/orchestration/src/lib.rs` 尾段、`crates/infra/store/src/lib.rs` 尾段同形） | 桌面端相位执行（本文档不写命令细节） |
| Rust 壳层（`packages/desktop/src-tauri/src/**`） | 同上 + `tauri::test::mock_app()` + `tauri::ipc::Channel::new(...)` 捕获 `InvokeResponseBody::Json`（`src/commands/change_flow/mod_test.rs` 的 `capturing_channel` 先例）+ 真实 tempdir `WorkspaceStores` | 同目录 `<模块>_test.rs`（`src/commands/notifications/mod_test.rs`、`src/commands/change_flow/mod_test.rs`）与 `src/bindings/mod_test.rs`（生成物磁盘文本面） | 同上 |
| 前端（`packages/desktop/src/**`） | `vite-plus/test`（`describe` / `it` / `expect` / `vi`）+ `@testing-library/react`（`render` / `renderHook` / `act` / `waitFor`）；jsdom 全局环境（`vite.config.ts` `test.environment`）+ `src/test-setup.ts` | 与被测模块同目录 `<模块>.test.ts(x)`（`include: ['src/**/*.test.ts?(x)']`） | `pnpm -C packages/desktop run test` |

沿用既有断言风格：进程边界一律 mock `@tauri-apps/api/core`（`invoke` 按命令名分派 + 可编程 `Channel`，`src/views/changes/hooks/use-change-flow-run.test.ts` 的 `ChannelMock` 形态）与 `sonner`（`toast` 观测，6 个既有测试文件形态）；内部模块（`lib/notify.ts`、`hooks/use-notifications.ts`、`components/notification-bell.tsx` 之间）真实组合，不互相 mock。磁盘面断言（`from 'sonner'` 恰两处、单文件 `new Channel` 恰一处、`native_model` 属性行）沿 `src/lib/ipc-contract.test.ts` / `src/module-entries.test.ts` 的 `readFileSync` 先例与 Rust 侧 `include_str!` 结构锚先例；事件轨接线另用同族锚（`include_str!("mod.rs")` 的 `walk_run(` 恰一处且含 `Some(`、`retire` 命中集、壳层零 `RunUpdate::Finished`）对齐 design「接线自检」的可 grep 判据。

本变更新建 7 个测试文件（`notification/src/lib_test.rs`、`orchestration/src/notify_test.rs`、`store/src/notification_test.rs`、`commands/notifications/mod_test.rs`、`lib/notify.test.ts`、`hooks/use-notifications.test.ts`、`components/notification-bell.test.tsx`）并扩容 14 个既有测试文件（`store_test.rs` / `model_test.rs` / `envelope_test.rs` / `orchestration/src/walker_test.rs` / `orchestration/src/control_test.rs`（既有用例零改、只追加接线组）/ `commands/change_flow/mod_test.rs` / `bindings/mod_test.rs` / `app.test.tsx` / `use-workspaces.test.ts` / `use-agent-instances.test.ts` / `use-agent-providers.test.ts` / `use-change-list.toast.test.ts` / `contract-panel.test.tsx` / `explore-open-file.test.tsx`），另 2 个仅期望随动（`describe_test.rs` / `commands/db/mod_test.rs`，源码零改动）；零新增测试专用导出（沿 AGENT.md「no test-only exports」）。

Rust 侧的 `RunNotifier` 与 `Sink` 替身一律写在测试内（记录型 / 失败收场型实现）——二者是被测对象的协作者而非进程边界，不引替身框架，也不为测试放宽生产导出（`RunGuard::with_notifier` 的 `pub(crate)` 在同一 crate 的测试内可直接调用）。

## 验收范围

| AC ID | 验收条件 | 被测文件或模块 |
|---|---|---|
| AC-1 | `crates/core/notification` 只依赖 `serde` / `specta`；`Hub::publish` 顺序遍历且逐 sink 独立（一 sink 失败不影响后续）；`trait Sink` 无返回值 / 无 `async` / 无 `Result`；三不变量在 crate doc；core 内零 `tokio::spawn` / 零 `tauri` / 零 `async fn deliver` | `packages/desktop/src-tauri/crates/core/notification/Cargo.toml`、`packages/desktop/src-tauri/crates/core/notification/src/lib.rs`（+ `src/lib_test.rs`） |
| AC-2 | 生产代码 `from 'sonner'` 恰两处（`components/ui/sonner.tsx` / `lib/notify.ts`）；12 处业务调用点全部改经门面；四档调用形态逐字一致且**不传 `id`**；既有 19 处精确参数断言零改动 | `packages/desktop/src/lib/notify.ts`、`packages/desktop/src/hooks/use-workspaces.ts`、`packages/desktop/src/views/agents/hooks/use-agent-instances.ts`、`packages/desktop/src/views/agents/hooks/use-agent-providers.ts`、`packages/desktop/src/views/changes/hooks/use-change-list.ts`、`packages/desktop/src/views/architecture/components/contract-panel.tsx`、`packages/desktop/src/views/explores/components/explore-open-file.tsx` |
| AC-3 | `notify(...)` 在 `notification_record` reject（含参数校验同步抛 `IpcArgsError`）时不抛出、不影响 toast、不重试、不再发通知；DEV 恰一行 `console.warn`；`present(...)` 零落库调用 | `packages/desktop/src/lib/notify.ts` |
| AC-4 | `project` 对 `Step` / `Edge` / `SessionEvent` 恒 `None`、对 `Ask` / `ConfirmWait` / `Finished` 恒 `Some`；level 映射与「message 含 change 名」逐条；主键串逐字（含 `tool_call_id` 缺位退化）；白名单唯一单点（壳层零按变体 `if` 过滤）；`deliver` 是投影 → 投递的唯一组合点且挂在 `RunGuard::emit`（`ConfirmWait` / `Finished` 的真实出口），`walk_run` 第 9 参是唯一注入面 | `packages/desktop/src-tauri/crates/core/orchestration/src/notify.rs`、`packages/desktop/src-tauri/crates/core/orchestration/src/control.rs`、`packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs`、`packages/desktop/src-tauri/src/commands/change_flow/mod.rs`、`packages/desktop/src-tauri/src/commands/notifications/mod.rs`（+ `notify_test.rs` / `control_test.rs` / `walker_test.rs` / `change_flow/mod_test.rs` / `notifications/mod_test.rs`） |
| AC-5 | 同主键二次 publish ⇒ 表中恰一行且 `message` / `body` 更新；首条 `mark_read` 后重投 ⇒ `read_at` 仍 `Some`（不复活已读）；`created_at` 不覆盖 | `packages/desktop/src-tauri/crates/infra/store/src/store.rs`、`packages/desktop/src-tauri/crates/infra/store/src/notification.rs`、`packages/desktop/src-tauri/crates/infra/store/src/model.rs -> packages/desktop/src-tauri/crates/infra/store/src/model_test.rs`（`NotificationRecord` 九字段线形与 `native_model` id 18 断言） |
| AC-6 | 某 run 先落 `confirm:*` / `ask:*` 未读，再落该 run `finished` ⇒ 等待类未读全变已读；其他 run 与 `Feedback` 行不受影响；miss 即无操作。规则判定在 core（`notify::deliver` 对 `Finished` 调 `RunNotifier::retire`），壳侧只做机械转发（`NotificationBridge::retire`） | `packages/desktop/src-tauri/crates/infra/store/src/store.rs`、`packages/desktop/src-tauri/crates/core/orchestration/src/notify.rs`、`packages/desktop/src-tauri/src/commands/notifications/mod.rs`、`packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs`（组合链） |
| AC-7 | `notification_list` 与未读计数恒只含 `kind = Event`；列表 `created_at` 降序且页界自洽（`items.len() ≤ limit`、`total` = Event 全量）；`mark_all_read` 含未加载页并返回 `0`；`limit` 超 200 钳位 | `packages/desktop/src-tauri/crates/infra/store/src/store.rs`、`packages/desktop/src-tauri/src/commands/notifications/mod.rs`、`packages/desktop/src-tauri/crates/infra/store/src/envelope.rs` |
| AC-8 | `notification_subscribe` 实现顺序可断言（先取 receiver → 再读未读计数 → 起转发 → 返回）；「返回初值 + 随后经 Channel 收到条数」不丢；对照读数取「再次订阅返回的初值」，不新增未读查询命令 | `packages/desktop/src-tauri/src/commands/notifications/mod.rs` |
| AC-9 | 铃铛壳态呈现（欢迎态不挂）；徽标 `0` / `N` / `99+` 三态；抽屉倒序列表、单条置已读、全部已读清零；跨根条目先 `select(root)` 再 `navigate(/changes/{source_ref})`，同根直接跳转，`source_ref` 不可达行惰性 | `packages/desktop/src/components/notification-bell.tsx`、`packages/desktop/src/app.tsx` |
| AC-10 | 订阅失败静默（不阻断壳、不白屏、不弹错误 toast）；列表失败 inline 持久；零轮询、零文件 watch 依赖；`Channel<Notification>` 构造与 `notification_subscribe` 调用各恰一处（`use-notifications.ts`） | `packages/desktop/src/hooks/use-notifications.ts`、`packages/desktop/src/components/notification-bell.tsx` |
| AC-11 | 生成物含五命令 typed 包装与对应 zod 表项；`all_commands!` 尾部按登记序追加五条、既有条目零位移；命令名 / 包装名 / 出线 DTO 清单与尾部追加序断言同 change 随动；`check` 四段全绿 | `packages/desktop/src/types/generated/bindings.ts`、`packages/desktop/src/types/generated/bindings.zod.ts`、`packages/desktop/src-tauri/src/commands/mod.rs`（+ `src/bindings/mod_test.rs`） |
| AC-12 | `archi_graph` 读数 = 56 元素 / 127 关系（`02` 9/10、`03` 8/25、`04` 1/14、`06` 32/75）；`diagnostics` 空；表头 / `repo-archi-model` 计数表 / ADR 三处同值；`api:export` 幂等；`apiFindings` 增量恰一条 `docMissing` | `.dev-team/architecture/graph/model/02-backend-core.toml`、`03-backend-infra.toml`、`04-backend-shell.toml`、`06-frontend.toml`、`.dev-team/architecture/graph/INDEX.toml`、`.dev-team/architecture/decisions/0001-model-granularity.md`（见「不可测试项」） |
| AC-13 | `package.json` `version` 恰 +1 patch（`0.7.5` → `0.7.6`）；`tauri.conf.json` 仍 `"version": "../package.json"`；`src-tauri/Cargo.toml` crate 版本行逐字不变；改动集 ⊆ 变更范围；`openspec/specs/` 除 5 份 delta 外零 diff | `packages/desktop/package.json`（见「不可测试项」）+ 全仓改动集（机械核对） |
| AC-14 | 诚实边界在案（进程不在场零通知 / 持久化 = 跨重启与补拉 / OS 通知不在本变更 / ui 反馈不跨窗口）；`tauri-plugin-notification` 零依赖、`capabilities/**` 零 diff | 五份 delta spec 文本、`packages/desktop/src-tauri/capabilities/**`、依赖面（部分由 AC-1 的清单锚覆盖，其余见「不可测试项」） |

> **对位说明**：上表只列 AC 与承载模块；每个模块的测试文件路径、断言判据与 Mock 策略见下方对应 `###` 节（节标题即「源文件 -> 测试文件」映射对）。同一源文件挂同一测试文件的多处可观察面（如 `store.rs` 的六方法）合为一节；`bindings.ts` / `bindings.zod.ts` 成对合并为一行 AC，由单节承载（前端 `ipc-contract.test.ts` 的键表遍历面自动覆盖，本变更零改动）。
>
> **随动与非新增用例（期望随动，判据零改；实施时按此收窄或补期望序）**：
>
> - `packages/desktop/src-tauri/crates/infra/store/src/envelope_test.rs`：`entries()` 长度 `14 → 15`、名字清单在 `agent_instance` 之后插入 `notification`、user 维度行 `3 → 4`、全局库 `list_models` 三模型组 → 四模型组。
> - `packages/desktop/src-tauri/crates/infra/store/src/describe_test.rs`：`USER_TABLES` 数组 `3 → 4`（含 `notification`）及其两处「user 三行」期望。
> - `packages/desktop/src-tauri/crates/infra/store/src/model_test.rs`：全册 `native_model` id 唯一性清单补 `NotificationRecord`（id 18）。
> - `packages/desktop/src-tauri/src/commands/db/mod_test.rs`：global scope `db_models` 期望三行 → 四行（`notification` 计数 0 也列出）——`commands/db/mod.rs` 源码零改动，仅期望随动。
> - `packages/desktop/src-tauri/src/bindings/mod_test.rs`：`COMMAND_NAMES` / `COMMAND_WRAPPERS` 各 `57 → 62`、`DTO_TYPES` 补五出线类型、尾部追加序锚点随动。
> - `packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs`：`app_with` 的 `manage` 组补 `Arc<NotificationBus>`（须在 `manage(stores)` 之前取 `global_shared()`，现已在位）；**11 处 `ChangeFlowSink { … }` 字面构造按 DD12 收成四字段**（`root` / `change_id` / `control` / `notifier`，取值用 `Arc<NotificationBridge>` 或记录型 `RunNotifier` 替身）；既有「`sink.emit(ConfirmWait)` → 库一行」类的 7 组断言**重锚**——`RunEventSink::emit` 已不承载事件轨，同一断言改到 `orchestration` 侧（见 `control.rs` / `walker.rs` 两节），壳侧只留「零通知」反锚。
> - `packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs`：6 处 `walk_run(` 调用补第 9 参（无通知出口传 `None`）；`spawn_run` 装置增一个携 `Option<Arc<dyn RunNotifier>>` 的变体；`probe_guard` 经 `begin_run` 构造，零字面字段随动。
> - `packages/desktop/src-tauri/crates/core/orchestration/src/control_test.rs`：既有用例**零改动**（`begin_run` 签名未变、`RunGuard` 字面构造仅存在于生产代码），只追加 DD12 的旁路接线用例组。
> - `packages/desktop/src-tauri/crates/core/orchestration/src/notify_test.rs`：既有 14 个投影用例零改动，追加 `deliver` / `RunNotifier` 用例组与替身。
> - `packages/desktop/src-tauri/src/commands/notifications/mod_test.rs`：追加 `NotificationBridge` 的 `deliver` / `retire` / `context` 转发用例；首轮五命令与订阅顺序用例零改动。
> - `packages/desktop/src/app.test.tsx`：`commandsExceptEnvironment()` 的排除面补 `notification_subscribe`（或纳入期望序）；壳态挂载基线调用数随动；欢迎态用例零随动（`root === null` 不挂铃铛）。
> - 6 个既有 toast 测试文件：门面调用形态不变 ⇒ `toHaveBeenCalledWith(<文案>)` 断言零改动；仅「锁 `invoke` 总次数」的断言需按命令计数收窄（现有 `use-change-list.test.ts` 的两处 `toHaveBeenCalledTimes(1)` 位于无 toast 的取数路径，预期零改动）。
> - `packages/desktop/src/lib/ipc.test.ts` / `src/lib/ipc-contract.test.ts`：键遍历面（`Object.keys(commands)` / 三 zod 表键集）自动覆盖新命令，零改动。

## 单元测试

### packages/desktop/src-tauri/crates/core/notification/src/lib.rs -> packages/desktop/src-tauri/crates/core/notification/src/lib_test.rs

#### 待测功能

- `Hub::new()`: 装配 `Vec<Arc<dyn Sink + Send + Sync>>`，装配序即投递序；空列表合法、不去重、不探测 sink 能力
- `Hub::publish()`: 顺序遍历 sinks 各投递一次；无返回值、不重投、不 `catch_unwind`（sink 以失败收场时后续 sink 照常）
- `Sink::deliver()`: trait 方法形态（同步、无返回值、非 `async`、无 `Result`），对象安全可 `Arc<dyn Sink + Send + Sync>` 跨线程共享
- `Notification::feedback()`: 固定 `kind = Feedback` / `root = None` / `source_ref = None`，其余字段搬自 `NotificationFeedbackInput`

> **注记**：design「类型定义」子表同文件声明 `NotificationLevel` / `NotificationKind` / `Notification` / `NotificationPage` / `NotificationFeedbackInput` 五型；下表以类型名作测试对象承载线形与解码判据（来自该子表声明，非源码 grep 推断），不另立「待测功能」条目。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `Hub::publish()` | 正向 | 两 sink 记录收到的 `id` 序列：序 = 装配序（先 A 后 B），每个 sink 对同一条通知恰收一次 | 单点 |
| `Hub::publish()` | 正向 | 同一 Hub 连续 publish 两条不同通知 → 每 sink 各收 2 次，同 sink 内序 = 发布序（FIFO），跨 sink 不交错破坏计数 | 单点 |
| `Hub::publish()` | 异常 | 第一 sink 在 `deliver` 内把落库 `Err` 降级为内部记录（落库 sink 的失败收场形态，非 panic）→ 第二 sink 仍恰收一次，`publish` 返回 `()`、零 panic、不重投 | 单点 |
| `Hub::publish()` | 边界 | 空 sinks（`Hub::new(vec![])`）→ 零投递、零 panic、返回 `()` | 单点 |
| `Hub::publish()` | 边界 | 大 sink 列表（32 个）+ 空 `id` + 超长 `message`（>1000 字符）+ 特殊字符（`\n` / `\0` / emoji）→ 全部收到，内容按引用原样（零截断、零转义） | 矩阵迭代（4 形态） |
| `Hub::new()` | 正向 | 传入 n（1 / 2 / 8）个 sink → 单次 `publish` 的投递总次数恰为 n（装配与投递一一对应） | 矩阵迭代（3 规模） |
| `Hub::new()` | 边界 | `Vec::new()` 与同一 `Arc` 重复装配两次 → 后者投递两次（不去重、不按引用合并） | 矩阵迭代（2 形态） |
| `Hub::new()` | 异常 | 装配「必然失败」的自定义 sink（`deliver` 内部吞掉 `Err`）→ 构造成功（构造不做能力探测），失败只在投递期由 sink 自处 | 单点 |
| `Sink::deliver()` | 正向 | 签名面断言：`let _: () = sink.deliver(&notification);`（无返回值）；`Arc<dyn Sink + Send + Sync>` 克隆进 `std::thread::spawn` 后调用成功（`Send + Sync` 对象安全） | 单点 |
| `Sink::deliver()` | 边界 | 同一 sink 实例被两个 Hub 分别持有（`Arc::clone`）→ 两 Hub 投递互不影响，计数累加（core 不假设 sink 无状态） | 单点 |
| `Sink::deliver()` | 异常 | 实现体内部吞错后调用方零错误面：经 Hub 投递后无 `Result` / 无异常通道可观测（失败只能由 sink 自身留痕） | 单点 |
| `Notification::feedback()` | 正向 | `kind == Feedback`、`root == None`、`source_ref == None`、`created_at` == 注入值、`id` / `level` / `message` / `body` 逐项搬自入参 | 单点 |
| `Notification::feedback()` | 边界 | `body` 三形态（`None` / `Some("")` / `Some(>1000 字符)`）原样搬运；`id` 空串与超长串原样（词表层零校验，空白校验归命令层） | 矩阵迭代（3 形态） |
| `Notification::feedback()` | 异常 | `level` 四档逐一注入（含 `Info`）→ 原样保留（feedback 不按 level 特判、不静默降级） | 矩阵迭代（4 档） |
| `Notification` | 正向 | serde 线形八字段，多词键为 camelCase（`sourceRef` / `createdAt`）；序列化文本零 `read_at` / `readAt`（读状态不进载荷） | 单点 |
| `Notification` | 异常 | 解码拒绝：缺 `id` / 缺 `kind` / 缺 `level` / 缺 `message` / `kind = "unknown"` / `level = "debug"` 的 JSON → `Err`（不静默产出空载荷） | 矩阵迭代（6 形态） |
| `Notification` | 边界 | 三个 Option 字段（`body` / `root` / `source_ref`）的 `None` 与 `Some("")` 组合（4 形态）+ `created_at` 取 `0` / `-1` / `i64::MAX` → 线形原样（`Some("")` 不折成 `None`、时间不被钳位） | 矩阵迭代（4 形态 × 时间边界） |
| `NotificationKind` | 正向 | 两变体 JSON 线值恰为 `"event"` / `"feedback"`；`serde_json::to_value` 的变体序 = 声明序（`Event` 先）——变体序冻结契约 | 单点 |
| `NotificationKind` | 异常 | 非法线值 `"unknown"` / `"Event"` / `""` / `null` → `serde_json::from_str` 拒绝（旧行解码错位面反向锚） | 矩阵迭代（4 形态） |
| `NotificationKind` | 边界 | `kind` × `level` 交叉（`Event/Info` 与 `Feedback/Error` 两组合）在 `Notification` 载荷内编解码往返 → 两字段同值（枚举组合不互相污染） | 矩阵迭代（2 组合） |
| `NotificationLevel` | 正向 | 四变体往返闭合：`to_string(v)` 为 `info` / `success` / `warning` / `error`，`from_str` 还原同变体 | 矩阵迭代（4 值） |
| `NotificationLevel` | 边界 | 线形为裸字符串（非对象 / 非数字）；`Copy` 语义下同一值两次取用无 move 报错 | 单点 |
| `NotificationLevel` | 异常 | 非法线值 `"debug"` / `"Info"` / `""` / `null` / `1` → 全部拒绝（大小写不归一） | 矩阵迭代（5 形态） |
| `NotificationPage` | 正向 | `items` 两元素 + `total = 5` → 线形逐字保留（`total` 不被改写成 `items.len()`） | 单点 |
| `NotificationPage` | 边界 | `items == []` + `total == 0`；`total == u32::MAX` 可序列化（上界不截断、不溢出） | 矩阵迭代（2 形态） |
| `NotificationPage` | 异常 | 解码拒绝：缺 `items` / 缺 `total` / `total = -1`（非 `u32`）/ `items` 非数组 → `Err`（页界信封不静默退化为空页） | 矩阵迭代（4 形态） |
| `NotificationFeedbackInput` | 正向 | 四字段齐备的 JSON 往返逐字相等（`id` / `level` / `message` / `body`） | 单点 |
| `NotificationFeedbackInput` | 边界 | 缺 `body` 键 → 反序列化为 `body == None`；`body: null` 同；`id` / `message` 空串可解码（词表层不拒） | 矩阵迭代（3 形态） |
| `NotificationFeedbackInput` | 异常 | 缺 `id` / 缺 `message` / 缺 `level` / `level = "debug"` → 拒绝（三必填字段 + 枚举封闭） | 矩阵迭代（4 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| —（无 mock） | 纯数据类型 + 进程内 `Sink` 替身（测试内定义记录型 / 失败收场型实现，属被测对象的协作者而非进程边界，不引入替身框架） | 全部用例 |
| 时钟 | 不 mock：`created_at` 是 `Notification::feedback` 的显式入参，用例直接注入 `0` / 负值 / `i64::MAX` 字面量 | `Notification::feedback()` 与线形用例 |
| `panic` 面 | 不 mock、不 `catch_unwind`：project 的失败隔离取「sink 内部把 `Err` 降级」形态（design 校准 2），不构造 panic 用例 | `Hub::publish()` 异常行 |

### packages/desktop/src-tauri/crates/core/notification/Cargo.toml -> packages/desktop/src-tauri/crates/core/notification/src/lib_test.rs

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（「公共函数 / API」子表零本文件行；构建清单是声明面，非代码模块）。下列条目取自 design「新增文件」本文件行所述清单面，用于定位 AC-1 的依赖面判据，不构成新增公共 API，也不由源码 grep 反推导出面。 -->

- `[dependencies]` 段: 只 `serde` / `specta` 两行（各 `{ workspace = true }`）；禁 Tauri / 禁 tokio / 禁 workspace 内部依赖
- `[package]` 段: `name = "notification"`、`edition = "2021"`
- crate 级注释: 三条边界（禁 Tauri / 禁 tokio 运行时 / 禁 workspace 内部依赖）在案，与 spec 的边界中立表述一致

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 依赖面（`include_str!("../Cargo.toml")` 结构锚） | 正向 | `[dependencies]` 段恰 `serde` / `specta` 两行且均 `{ workspace = true }`（两依赖 manifest 逐字在案） | 单点 |
| 依赖面 | 异常 | 禁用面零命中：`tauri` / `tokio` / `native_db` / `async fn deliver` / 任一 workspace 内部 crate 名（`foundation` / `workflow` / `orchestration` / `store` / `agent` / `checks` / `notification` 自身除外） | 矩阵迭代（禁用名清单） |
| 依赖面 | 边界 | 内存副本注入一行 `tokio = { workspace = true }` → 同一判据检出（负向样本驱动，不改仓内文件） | 单点 |
| 依赖面 | 边界 | crate 级注释三句在案（禁 Tauri / 禁 tokio 运行时 / 禁 workspace 内部依赖），与 spec 的边界中立表述一致 | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| —（清单文本） | 不 mock：`include_str!("../Cargo.toml")` 编译期绑定同一份文件；异常形态只在内存字符串副本上就地改写 | 全部用例 |

### packages/desktop/src-tauri/crates/core/orchestration/src/notify.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/notify_test.rs

#### 待测功能

- `project()`: 六变体白名单单点（三变体 `None` / 三变体 `Some`）、level 映射、主键按幂等串铸造、文案字面唯一持有、`ConfirmWait` 的 body 查相位表 `PhaseDescription`
- `deliver()`: 投影 → 投递的唯一组合点（`project` → `notifier.deliver`；`RunUpdate::Finished` 再 `notifier.retire`）；排除变体零投递、零终态清理、不加料不改写
- `RunNotifier::context()`: 出口自持的 run 级投影上下文（五字段只存一份）由 `deliver` 消费，投影零额外查询
- `RunNotifier::deliver()`: 同步、无返回值、非 `async`、无 `Result`；实现内部失败自降级（旁路）
- `RunNotifier::retire()`: 终态收口的出口方法（幂等、失败静默），`run_id` 取自自持上下文而非入参

> **注记**：design「类型定义」子表同文件声明 `NotifyContext`（五字段，不出线）、`RunNotifier`（`Send + Sync`，挂 `Arc<dyn RunNotifier>` 跨 crate 边界）与私有 `phase_description`；下表以「五字段消费面」与「相位表查表降级」两行覆盖 `NotifyContext`，不为其新增公共导出。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `project()` | 正向 | 三变体各一次 `Some`：`Ask` / `ConfirmWait` / `Finished(Completed)` 均返回 `Some`，`kind` 恒 `Event`、`root == Some(ctx.root)`、`source_ref == Some(ctx.change_id)` | 矩阵迭代（3 变体） |
| `project()` | 正向 | `Ask`：`level == Warning`、`message` 含 `ctx.change_name`、`body == Some(ask.question)`、`id == "run:{run_id}:ask:{tool_call_id}"`、`created_at == now` | 单点 |
| `project()` | 正向 | `ConfirmWait`：`level == Warning`、`message` 含变更名与相位 id、`body` 与 `phase_table("requirement")` 中该相位 `description` 逐字相等（不另立翻译表） | 单点 |
| `project()` | 正向 | `Finished` 三终态：`Completed → Success`（body `None`）/ `Failed → Error`（body `== reason`）/ `Stopped → Info`（body `None`），三者 `id` 均 `"run:{run_id}:finished"` | 矩阵迭代（3 终态） |
| `project()` | 正向 | 三变体排除：`Step` / `Edge` / `SessionEvent` 各一次 → 均 `None`（零通知产生） | 矩阵迭代（3 变体） |
| `project()` | 异常 | `Finished` 非终态 status（`Running` / `WaitingConfirm` / `WaitingAsk`）→ `None`（防御性，不伪造终态通知） | 矩阵迭代（3 状态） |
| `project()` | 异常 | 相位表降级：`workflow_type` 未知串（`"nope"` / `""`）或相位 id 不在表（`"archive-x"`）→ `ConfirmWait` 的 `body == None`，`Some` 仍产出（不 panic、不回落英文译文） | 矩阵迭代（3 形态） |
| `project()` | 边界 | `Ask` 的 `tool_call_id` 缺位（`None`）→ 主键退化 `"run:{run_id}:ask:{now}"`（与 `created_at` 同源） | 单点 |
| `project()` | 边界 | 多轮 ask 不互相覆盖：同 run 两次 `Ask`（`tool_call_id` 分别 A / B）→ 两主键逐字不同；`options` 非空时 `body` 仍逐字等于 `question`（选项串零出现） | 矩阵迭代（2 形态） |
| `project()` | 边界 | 空值/极端值：`change_name == ""`、`run_id == ""`、`phase == ""` → 仍 `Some` 且串逐字拼接（零 panic）；`change_name` 超长（>1000 字符）与含 `\n` / emoji → 原样嵌入 `message` | 矩阵迭代（4 形态） |
| `project()` | 边界 | 确定性：同一 `(update, ctx, now)` 两次调用结果相等（`Notification: PartialEq`）；`now` 变化只改 `created_at` 与退化主键，其余字段不变 | 单点 |
| `project()` | 正向 | 五字段消费面（`NotifyContext`）：逐字段单独替换 → 输出对应字段变化（`root` → `root`；`change_id` → `source_ref`；`change_name` → `message`；`run_id` → 主键；`workflow_type` → `ConfirmWait` 的 `body`） | 矩阵迭代（5 字段） |
| `project()` | 边界 | `ConfirmWait` 的 `archive` 载荷（`Some(...)` 与 `None`）→ 投影结果完全相同（归档前置数据不进通知） | 矩阵迭代（2 形态） |
| `deliver()` | 正向 | 三纳入变体各一次 → 替身 `deliver` 恰 1 次/次，且投递载荷与 `project(update, ctx, now)` 直调结果逐字段相等（组合点不加料、不改写、不二次铸造主键） | 矩阵迭代（3 变体） |
| `deliver()` | 正向 | `Finished` 三终态（`Completed` / `Failed` / `Stopped`）→ `deliver` 1 次 + `retire` 1 次；同一 emit 内序为 deliver 先、retire 后 | 矩阵迭代（3 终态） |
| `deliver()` | 正向 | 三排除变体（`Step` / `Edge` / `SessionEvent`）→ `deliver` / `retire` 各 0 次（trait 投递调用不发生；`context` 各读 1 次——投影本身需要上下文） | 矩阵迭代（3 变体） |
| `deliver()` | 边界 | 同一 update 连续两次 → 两次 `deliver`（组合点不缓存、不去重，幂等归 store 主键）；`Finished` 两次 → `retire` 两次（幂等由 `retire_run_notifications` 承担，接口不裁剪） | 单点 |
| `deliver()` | 边界 | `now` 三态（`0` / 负值 / `i64::MAX`）→ 投递载荷的 `created_at` 与 `Ask` 退化主键原样（组合点不钳位） | 矩阵迭代（3 值） |
| `deliver()` | 异常 | 替身按旁路语义在 `deliver` 内吞掉自身失败（记录但不 panic / 不上抛）→ `deliver` 返回 `()`、零 panic、后续 `retire` 仍按变体发生（不 `catch_unwind`，与校准 2 同族） | 单点 |
| `RunNotifier::context()` | 正向 | 每个 `Some` 变体经 `deliver` 读取恰 1 次；返回的自持上下文五字段逐项进入投递载荷（`root` → `root`；`change_id` → `source_ref`；`change_name` → `message`；`run_id` → 主键；`workflow_type` → `ConfirmWait` 的 `body`） | 矩阵迭代（3 变体） |
| `RunNotifier::deliver()` | 正向 | 替身签名面断言：`let _: () = notifier.deliver(&notification);`（无返回值）；`Arc<dyn RunNotifier>` 克隆进 `std::thread::spawn` 后调用成功（`Send + Sync` 对象安全） | 单点 |
| `RunNotifier::deliver()` | 边界 | 同一替身两次 `deliver`（不同通知）→ 记录序 = 调用序（接口不假设单次投递、不假设无状态） | 单点 |
| `RunNotifier::retire()` | 正向 | `retire` 的 `run_id` 取自 `context()`（替身按 `context().run_id` 编排记录），与 `Finished` 载荷里的 run 同源；`deliver` 与 `retire` 各恰 1 次 | 单点 |
| `RunNotifier::retire()` | 边界 | 非终态变体（含三排除变体与 `Ask` / `ConfirmWait`）零 `retire`；终态重复两轮 → 2 次（接口不做幂等裁剪） | 矩阵迭代（2 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| —（无 mock） | `project` / `deliver` 为进程内纯组合函数；`now` 为显式入参（确定性时钟注入），相位表取真实 `workflow::write::phase_table`（禁在测试里复制 `description` 字面——复制会抹掉「相位表漂移」判据） | 全部用例 |
| `RunNotifier` | 测试内替身（`Send + Sync` 的记录型实现：`context()` 返回真实 `NotifyContext`，`deliver` / `retire` 追加到共享调用日志；另一形态在 `deliver` 内吞错但不上抛）——协作对象替身，不引替身框架 | `deliver` 与 `RunNotifier` 全部用例 |
| 相位表降级形态 | 不 mock 相位表：以未知 `workflow_type` / 未知相位 id 驱动 `None` 分支 | 两条降级用例 |

### packages/desktop/src-tauri/crates/core/orchestration/src/control.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/control_test.rs

#### 待测功能

- `RunGuard::emit()`: 签名与既有语义不变；有 notifier 时先在 `control.publish` 之前 `notify::deliver(&update, notifier, now_millis())`，`publish` 函数体逐字不动；无 notifier 时行为与改动前逐字一致

> **注记**：design「类型定义」子表同文件声明 `RunGuard` / `StepEmitter` 各增一个私有 `notifier` 字段；`RunGuard::with_notifier` 是 `pub(crate)`（不入「公共函数 / API」表），下表经 `begin_run` + `with_notifier` 与 `emitter()` / `attach_output_ref()` 两个公开面观察字段是否生效，不为其新增导出。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `RunGuard::emit()` | 正向 | 有 notifier：`emit(ConfirmWait { phase })` → 替身恰 1 条 `Warning`（`id == "run:{run_id}:confirm:{phase}"`），且替身在 `deliver` 内对同一 receiver `try_recv` 为空（旁路先于 `publish`，DD12 的「先旁路、后主线」序） | 单点 |
| `RunGuard::emit()` | 正向 | 有 notifier：`finish(Completed)`（消费 self 的终态出口）→ `deliver` 1 次 + `retire` 1 次，且注册表除名后 `snapshot` 为空（终态仍走唯一出口） | 单点 |
| `RunGuard::emit()` | 正向 | 有 notifier：`emit(Step)` / `emit(Edge)` / `emit(SessionEvent)` → 替身 `deliver` / `retire` 各 0 次；`RunNotice` 广播面照旧（`Step` 通知仍流出、`SessionEvent` 仍写会话槽） | 矩阵迭代（3 变体） |
| `RunGuard::emit()` | 边界 | 无 notifier（`begin_run` 铸 `None`）→ 零替身调用、广播与快照面与改动前逐字一致（既有 `control_test` 用例即此分支，断言零改） | 单点 |
| `RunGuard::emit()` | 边界 | 直接 `emit(Finished)` 后再 `finish(Completed)`（同一终态两次）→ `deliver` 2 次 / `retire` 2 次（守卫不去重，幂等由 store 主键与 `retire_run_notifications` 承担） | 单点 |
| `RunGuard::emit()` | 边界 | `emitter().attach_output_ref(...)` 的合成 guard → 走既有 `emit_node` 路径零 panic、零 `deliver`（`Step` 属排除变体），广播面照旧 | 单点 |
| `RunGuard::emit()` | 异常 | 替身按旁路语义在 `deliver` / `retire` 内吞错 → `emit` 返回 `()`、`publish` 照常、`RunNotice` 订阅端仍收到该条（主线不因旁路失败中断，R-9 的 core 侧判据） | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `RunNotifier` | 测试内记录型替身（`deliver` / `retire` 调用序 + `context` 读取计数；另一形态在 `deliver` 内 `try_recv` 以断言旁路先于 `publish`） | 全部用例 |
| 广播 / 注册表 | 不 mock：真实 `ChangeFlowControl::new(Arc<AskRegistry>)` + `begin_run` + `subscribe`（`try_recv` 观测），注册表语义即被测面 | 全部用例 |
| 时钟 | 不 mock：`now_millis` 为生产 `pub(crate)` 时钟，断言只取主键字面与 `level`（不断言毫秒值） | 全部用例 |

### packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs

#### 待测功能

- `walk_run()`: 尾部第 9 参 `notifier: Option<Arc<dyn RunNotifier>>`——函数体第一行 attach 到 guard（早于起点 `history.run_started` 写缝）；`None` = 无通知出口（引擎测试 / 无头调用），引擎语义逐字不变

> **注记**：注入面是「修改文件」行的行为面（「公共函数 / API」表的 `walk_run` 修改行）；`RunGuard::with_notifier` 为 `pub(crate)`，故 `walk_run` 是本 crate 内唯一合法接入口，也是生产链唯一入口。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `walk_run()` | 正向 | 端到端接线（回溯 1 的根因反锚）：携 `Some(替身)` 跑两相位 run（既有 `spawn_run` + `spawn_confirmer(true)` 装置）全放行至 `Completed` → 替身收恰 2 条 `confirm:*` + 恰 1 条 `run:{run_id}:finished`（`Success`）+ `retire` 恰 1 次 | 单点 |
| `walk_run()` | 正向 | 投递序 = 投影发生序：`confirm:proposal` → `confirm:dev-design` → `finished`，`finished` 之后零投递（终态收尾单点） | 单点 |
| `walk_run()` | 正向 | fail-fast 起点：`RunHistoryPort::run_started` 返回 `Err` → 替身收 1 条 `Finished(Failed)` + `retire` 1 次（attach 在函数体第一行，早于起点写），`walk_run` 仍返回 `Failed`、零相位执行 | 单点 |
| `walk_run()` | 正向 | 受控停止（`confirm` 否决 / 取消置位）→ 终态 `Stopped` → 替身收 1 条 `Finished`（`Info`），等待类通知不被伪造 | 单点 |
| `walk_run()` | 边界 | `None` → 与既有基线逐字一致：`ChangeRunStatus`、`RunNotice` 流条数、`RecordingHistory` 落包捕获面全等；零 panic | 单点 |
| `walk_run()` | 边界 | 整 run 的运行期变体不产通知：成功 run 内 `Step` / `Edge` / `SessionEvent` 大量 emit → 替身 `deliver` 计数恰等于纳入变体数（白名单在生产链上成立，不止在 `project` 单测里） | 单点 |
| `walk_run()` | 边界 | 三相位 run → 3 条 `confirm:*` 主键互不相同（相位区分）；`retire` 只在终态 1 次（中间相位的 `confirm` 不清死未读） | 单点 |
| `walk_run()` | 异常 | 替身 `deliver` / `retire` 内部吞错 → run 终态、落包捕获面与返回值零变化（旁路不改业务；`walk_run` 签名与既有返回值语义零变） | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `RunNotifier` | 测试内记录型替身（`context()` 返回真实 `NotifyContext`，投递序列进共享日志） | 全部携 notifier 用例 |
| 引擎三缝 / 历史缝 | 沿用既有 `FakeWorker` / `FakeTools` / `FakeVcs` / `RecordingHistory` 装置（进程边界替身，与本次变更无关）；`spawn_run` 增携 notifier 的变体，既有 6 处调用补 `None` | 全部用例 |
| 时钟 | 不 mock：`Request.started_at` 沿用既有常量注入；断言只取主键字面与 `level` | 全部用例 |

### packages/desktop/src-tauri/crates/infra/store/src/notification.rs -> packages/desktop/src-tauri/crates/infra/store/src/notification_test.rs

#### 待测功能

- `StoreNotificationSink::new()`: 落库 sink 构造（持 `Arc<Store>`；`store` 必须是全局库实例）
- `NotificationRecord::from_notification()`: 载荷 → 记录（`read_at = None`），一比一映射单点
- `NotificationRecord::to_notification()`: 记录 → 载荷（丢弃 `read_at`），同一映射单点

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `NotificationRecord::from_notification()` | 正向 | 九字段逐项对应：`id` / `kind` / `level` / `message` / `body` / `root` / `source_ref` / `created_at` 搬运自载荷，`read_at == None` | 单点 |
| `NotificationRecord::from_notification()` | 边界 | 四 Option 字段组合：`body` / `root` / `source_ref` 的 `None` 与 `Some("")` 交叉（4 形态）；`id` 空串与超长（>1000）原样；`created_at` 取 `0` / `-1` / `i64::MAX` | 矩阵迭代（4 形态 × 时间边界） |
| `NotificationRecord::from_notification()` | 异常 | 映射不掺域语义：`Event` 载荷（`root = Some`）与 `Feedback` 载荷（`root = None`）分别映射 → 输出逐字段对应、`kind` 原样（零特判、零归一） | 矩阵迭代（2 载荷形态） |
| `NotificationRecord::to_notification()` | 正向 | 记录（`read_at = Some(now)`）→ 载荷八字段相等且无读状态字段 | 单点 |
| `NotificationRecord::to_notification()` | 边界 | `read_at` 三态（`None` / `Some(0)` / `Some(i64::MAX)`）→ 载荷完全相同（丢读状态是恒定行为） | 矩阵迭代（3 形态） |
| `NotificationRecord::to_notification()` | 异常 | 记录 `kind = Feedback` → 载荷 `kind` 保持 `Feedback`（不归一成 `Event`，过滤口径只在 store 查询层） | 单点 |
| `NotificationRecord` 往返（from → to） | 正向 | `to_notification(from_notification(&n)) == n`（`Notification: PartialEq`，八字段全等） | 单点 |
| `NotificationRecord` 往返 | 边界 | 极端字段值往返保真：超长 `message`、含 `\n` / `\0` / emoji、`body == None`、`root == Some("")` | 矩阵迭代（4 形态） |
| `StoreNotificationSink::new()` + `deliver()` | 正向 | 全局库 store 构造 sink 后 `deliver` 一条 → 库中恰一行，字段与载荷逐项一致，`unread_notification_count() == 1`，`list_notifications(0, 10)` 可读 | 单点 |
| `StoreNotificationSink::new()` + `deliver()` | 边界 | 同一 store 构造两个 sink，各 `deliver` 同一主键（`message` 不同）→ 库中仍一行（幂等由 store 承担）、`read_at` 保持（sink 不掺读状态语义） | 单点 |
| `StoreNotificationSink::deliver()` | 异常 | sink 持 workspace 维度库实例 → `deliver` 不 panic、零上抛、零记录（失败降级为 `eprintln!` 一行，旁路语义 R-9） | 单点 |
| `StoreNotificationSink::deliver()`（组合：sink → `Store::publish_notification`） | 正向 | 链路闭环：`deliver` 落库 → `mark_notifications_read([id])` → 再次 `deliver` 同主键 → `list_notifications` 行的 `message` 更新而读状态不复位（AC-5 在 sink 链路成立） | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `Store`（库实例） | 不 mock：真实 tempdir 红库（沿 `store_test.rs` 的 `Env` 装置），全局库 / workspace 库各开一次表达两种维度 | 全部用例（含失败降级） |
| `eprintln!` 面 | 不捕获 stderr：失败判据取「不 panic / 零返回错误 / 库中零行」，不锁定日志格式（日志字面属实现细节） | `deliver` 异常行 |
| 时钟 | 不 mock：`read_at` 只断言 `is_some()` 与「第二次投递后不变」，不断言具体毫秒值 | 幂等与读状态保真行 |

### packages/desktop/src-tauri/crates/infra/store/src/model.rs -> packages/desktop/src-tauri/crates/infra/store/src/model_test.rs

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（「公共函数 / API」子表零本文件行）；模型类型 `NotificationRecord` 的声明在 design「类型定义」子表与「数据模型」节。下列条目取自该两处，用于定位字段 / id / 编码判据，不构成新增公共 API。 -->

- `NotificationRecord`: 九字段结构体声明（八载荷字段 + `read_at: Option<i64>`）；`#[native_model(id = 18, version = 1)]`、`#[native_db]`、`id` 上 `#[primary_key]`
- 字段 `///` doc: 每字段一行，是 db 查看器表与字段说明的唯一派生源（零模型特定代码）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `NotificationRecord`（类型定义表） | 正向 | serde 线形九字段，多词键 camelCase（`sourceRef` / `createdAt` / `readAt`）；编码 → 解码逐字段相等 | 单点 |
| `NotificationRecord` | 正向 | `native_model_id() == 18`、`native_model_version() == 1`；该 id 纳入全册唯一性清单后无冲突（现役 1/2/4/5/6/7/8/9/10/11/12/13/15/16/17 + 18） | 单点 |
| `NotificationRecord` | 边界 | 四 Option 字段（`body` / `root` / `source_ref` / `read_at`）的 `None` / `Some("")` 组合（4 形态）；`created_at` 取 `0` / `-1` / `i64::MAX` | 矩阵迭代（4 形态） |
| `NotificationRecord` | 边界 | 枚举交叉编码：`kind` 两变体 × `level` 四档抽样（`Event/Warning` 与 `Feedback/Error`）→ 编码后解码还原同值（变体序冻结的反向锚） | 矩阵迭代（2 组合） |
| `NotificationRecord` | 异常 | 解码拒绝：缺 `id` / 缺 `kind` / `kind = "unknown"` / `level = "debug"` 的 JSON → 解码 `Err`（不静默产出空记录） | 矩阵迭代（4 形态） |
| `NotificationRecord` 结构锚 | 边界 | `include_str!("model.rs")`：`#[native_model(id = 18, version = 1)]` 与 `#[primary_key]` 各恰一处，九字段的 `///` doc 行齐备（表与字段说明的唯一派生源） | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| —（无 mock） | 纯 serde / `native_model` 面；库实例只做真实落盘往返（不 mock 库、不复制编解码器） | 全部用例 |
| 全册 id 清单 | 不 mock：沿用 `model_test.rs` 既有 `native_model::Model::native_model_id()` 聚合断言，补 `NotificationRecord` 一项 | id 唯一性行 |

### packages/desktop/src-tauri/crates/infra/store/src/store.rs -> packages/desktop/src-tauri/crates/infra/store/src/store_test.rs

#### 待测功能

- `Store::publish_notification()`: 主键 upsert；命中只更新 `message` / `body` / `level`，保留 `read_at` 与 `created_at`
- `Store::list_notifications()`: 只 `Event`；`(created_at desc, id asc)` 稳定序；`limit.min(200)` 钳位；`total` = Event 全量
- `Store::unread_notification_count()`: 只数 `kind = Event` 且 `read_at` 为空
- `Store::mark_notifications_read()`: 只翻 `Event` 未读行的 `None → Some`；未知 id 与 `Feedback` 忽略；返回剩余未读
- `Store::mark_all_notifications_read()`: 全量 `Event` 未读行置已读（含未加载页）；返回剩余未读（恒 0）
- `Store::retire_run_notifications()`: 按 `run:{run_id}:confirm:` / `run:{run_id}:ask:` 前缀命中未读 `Event` 行置已读；miss 即无操作

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `Store::publish_notification()` | 正向 | 新主键 → 恰一行、九字段逐一相符、`read_at` 为 `None` | 单点 |
| `Store::publish_notification()` | 正向 | 同主键二次（`message` / `body` / `level` 均改）→ 仍一行，三字段取后者，`created_at` 保留首次值 | 单点 |
| `Store::publish_notification()` | 边界 | 先 `mark_notifications_read([id])` 再以同主键重投 → `read_at` 仍 `Some`（不复活已读，AC-5） | 单点 |
| `Store::publish_notification()` | 边界 | 极端字段：`body` / `root` / `source_ref` 为 `None`；`message` 超长（>1000）与含 `\n` / `\0` / emoji → 逐字往返；`id` 空串与超长串可落库（词表层零校验） | 矩阵迭代（4 形态） |
| `Store::publish_notification()` | 边界 | 零修剪：连续 500 条 `publish` → `list_notifications(0, 200).total == 500`，零淘汰、零 TTL 清理 | 单点 |
| `Store::publish_notification()` | 异常 | 在 workspace 维度库实例上调用 → `Err(StoreError::Db)`，零行写入（收件箱只在全局库的**结构性**判据） | 单点 |
| `Store::list_notifications()` | 正向 | 只 `Event`：插 N 条 `Event` + M 条 `Feedback` → `items.len()` 与 `total` 均只含 N（`Feedback` 零出现） | 单点 |
| `Store::list_notifications()` | 正向 | 稳定降序：`created_at` 降序；构造同毫秒两行 → 按 `id` 升序补齐次级键；两页（`offset 0/limit 1` 与 `offset 1/limit 1`）无重叠、无遗漏 | 单点 |
| `Store::list_notifications()` | 边界 | 空库 → `items == []`、`total == 0`；`offset` 超界（`u32::MAX`）→ `items == []` 且 `total` 不变 | 矩阵迭代（2 形态） |
| `Store::list_notifications()` | 边界 | `limit == 0` → `items == []`（`total` 仍全量）；`limit > 200`（205 条写入后取 500）→ 返回恰 200 条（钳位口径） | 矩阵迭代（2 形态） |
| `Store::list_notifications()` | 边界 | `total` 与 `items.len()` 脱钩：`offset = 0, limit = 5` 于 12 条 `Event` 上 → `items.len() == 5`、`total == 12` | 单点 |
| `Store::list_notifications()` | 异常 | workspace 维度库实例 → `Err(StoreError::Db)` | 单点 |
| `Store::unread_notification_count()` | 正向 | 混合样本（`Event` 未读 2 + `Event` 已读 1 + `Feedback` 未读 3）→ 返回 2 | 单点 |
| `Store::unread_notification_count()` | 边界 | 空库 → 0；全部 `Event` 已读（`Feedback` 仍未读）→ 0 | 矩阵迭代（2 形态） |
| `Store::unread_notification_count()` | 异常 | workspace 维度库实例 → `Err(StoreError::Db)` | 单点 |
| `Store::mark_notifications_read()` | 正向 | 3 条未读中标记 1 条 → 返回 2；该行 `read_at` 为 `Some`（`>= 调用前毫秒`），另两行仍 `None` | 单点 |
| `Store::mark_notifications_read()` | 边界 | 空 `ids` → 零变化且返回当前未读；未知 id → 忽略；已读 id 二次标记 → `read_at` 不被改写、返回值不变 | 矩阵迭代（3 形态） |
| `Store::mark_notifications_read()` | 边界 | `Feedback` 行 id 与重复 id：`Feedback` 命中不计、重复 id 只算一次；标记后 `list_notifications` 的 `total` 与 `created_at` 零变化 | 矩阵迭代（2 形态） |
| `Store::mark_notifications_read()` | 异常 | workspace 维度库实例 → `Err(StoreError::Db)` | 单点 |
| `Store::mark_all_notifications_read()` | 正向 | 12 条 `Event` 未读（超过一页 5 条）→ 返回 0；分页读出的每行 `read_at` 均 `Some`（含未加载页） | 单点 |
| `Store::mark_all_notifications_read()` | 边界 | 空库 → 0；已读行的 `read_at` 不被改写（幂等）；`Feedback` 行 `read_at` 仍 `None`（不受影响） | 矩阵迭代（3 形态） |
| `Store::mark_all_notifications_read()` | 异常 | workspace 维度库实例 → `Err(StoreError::Db)` | 单点 |
| `Store::retire_run_notifications()` | 正向 | 预置 `run:r1:confirm:archive` 与 `run:r1:ask:A` 未读 + `run:r1:finished` + `run:r2:confirm:dev-design` + 一条 `Feedback` → retire(`r1`) 后两条等待类变已读，`finished` / `r2` / `Feedback` 三行 `read_at` 不变 | 单点 |
| `Store::retire_run_notifications()` | 边界 | miss（无该 run 行）→ `Ok(())` 零写入（幂等）；空 `run_id` → 零匹配（不误伤全部行） | 矩阵迭代（2 形态） |
| `Store::retire_run_notifications()` | 边界 | 前缀碰撞：`run:r1:*` 与 `run:r10:*` 并存 → retire(`r1`) 只命中前者（前缀含尾冒号） | 单点 |
| `Store::retire_run_notifications()` | 异常 | workspace 维度库实例 → `Err(StoreError::Db)` | 单点 |
| `Store` 组合（`publish_notification` → `list` / `count`） | 正向 | 事件轨写入 → 列表与计数同源一致：3 条 `Event` 落库后 `total == 3` 且 `unread == 3`；标记 1 条后 `unread == 2` | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `Store`（库实例） | 不 mock：真实 tempdir 红库，全局库与 workspace 库各一（沿 `store_test.rs` 既有 `Env` 装置与 `open_global` / `open_workspace`） | 全部用例 |
| 时钟（`now_millis()`） | 不 mock（无注入点）：断言只取「`read_at.is_some()`」与「重投/重标记后不变」，`>= 调用前毫秒` 作松判；MUST NOT 为断言具体毫秒给生产代码加时钟注入 | `mark_*` / `publish` 幂等行 |

### packages/desktop/src-tauri/crates/infra/store/src/envelope.rs -> packages/desktop/src-tauri/crates/infra/store/src/envelope_test.rs

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（「公共函数 / API」子表零本文件行）。下列条目取自 design「修改文件」本文件行，用于定位「注册表登记一行 = db 查看器可见」判据；`scan_notifications` / `walk_notifications` / `notification_key` 均为私有实现，经 `pub(crate) entries()` / `entry()` / `scan` / `describe::table` 观察。 -->

- `MODEL_ENTRIES` 登记行: `{ name: "notification", dimension: DbDimension::User, count / scan / key_of / schema / walk }`
- `scan_notifications()` / `walk_notifications()` / `notification_key()`（私有）: `key` 取 `json["id"]`；db 查看器零模型特定代码即可见

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 注册表行（`entry` / `entries`） | 正向 | `entry("notification")` 命中且 `dimension == DbDimension::User`；`entries()` 长度 15、名字唯一、user 段序为 `workspace` / `agent_provider` / `agent_instance` / `notification` | 单点 |
| 注册表行 | 正向 | 全局库 `list_models()` 含 `notification` 且计数 = 实际写入量；写入 3 条后计数 3（登记面 additive，空库 0 也列出） | 单点 |
| 注册表行 | 边界 | `dimension_of("notification") == Some(DbDimension::User)`；近似名（`"notifications"` / `"Notification"` / `""` / 空白）→ `None`（精确名判据，零归一） | 矩阵迭代（4 形态） |
| 注册表行 | 异常 | workspace 库 `list_models()` 零 `notification` 行（收件箱 MUST NOT 落 workspace 库的注册面判据） | 单点 |
| `scan` 面（经 `scan(User, "notification", …)`） | 正向 | `RecordsPage.records` 每条 `key` == 记录 `id`（`notification_key` 取 `json["id"]`）；`total` == 全量行数 | 单点 |
| `scan` 面 | 边界 | 空库 → 空分页信封（`records == []` / `total == 0`）；`limit > 500` → 钳位 500（`MAX_SCAN_LIMIT` 口径不变） | 矩阵迭代（2 形态） |
| `scan` 面 | 异常 | 以 workspace 维度查 `notification` → `Err`（未知模型，跨维度不可达） | 单点 |
| `schema` / 字段说明（`describe::table(User, "notification")`） | 正向 | 表名 `notification`、九字段、每字段 `doc` 非空（`///` doc 派生）；`kind` / `level` 渲染为枚举引用 | 单点 |
| `schema` / 字段说明 | 边界 | 四个 Option 字段（`body` / `root` / `source_ref` / `read_at`）渲染为可空类型；`id` 标记为主键 | 矩阵迭代（4 字段） |
| `schema` / 字段说明 | 异常 | `describe::table(Workspace, "notification")` → `Err`（维度不匹配即未知模型，不静默回退全局库） | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 注册表 / 库 | 不 mock：`crate::envelope::entries()` 只读出口 + 真实 tempdir 库；`describe` / `scan` 取真实实现（替身会抹掉「登记缺失 / 维度错放」判据） | 全部用例 |
| 既有期望随动 | 不新增 mock：`entries()` 长度、user 段序、全局库模型组三处既有断言的期望值按上文「随动清单」更新，判据语句不变 | 注册表两行 |

### packages/desktop/src-tauri/src/commands/notifications/mod.rs -> packages/desktop/src-tauri/src/commands/notifications/mod_test.rs

#### 待测功能

- `NotificationBus::new()`: 构造 broadcast（容量 128）+ `Hub::new(vec![StoreNotificationSink, InAppSink])`（先落库、后广播）；只持全局库实例
- `NotificationBus::publish()`: 委托 `Hub::publish`
- `NotificationBus::retire_run()`: 委托 `Store::retire_run_notifications`，失败 `eprintln!` 降级
- `notification_subscribe()` / `notification_subscribe_with()`: 四步顺序（取 receiver → 读未读计数 → 起转发 → 返回计数）
- `notification_list()` / `notification_list_with()`: `bus.store().list_notifications(offset, limit)` 薄包装
- `notification_mark_read()` / `notification_mark_read_with()`: 返回剩余未读
- `notification_mark_all_read()` / `notification_mark_all_read_with()`: 返回剩余未读（恒 0）
- `notification_record()` / `notification_record_with()`: 空白校验 → `Notification::feedback(record, now)` → `publish_notification`；不广播
- `NotificationBridge::new()`: 壳侧 `RunNotifier` 出口构造（自持 `NotifyContext` + `Arc<NotificationBus>`，run 级一次构造，零状态转发）
- `NotificationBridge::deliver()`: → `bus.publish`（先落库 sink、后 in-app sink 的 Hub 装配序）；`NotificationBridge::retire()`: → `bus.retire_run(&context.run_id)`；`NotificationBridge::context()`: 返回自持上下文（五字段只存一份）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `NotificationBus::new()` + `publish()` | 正向 | 先 subscribe 再 publish 一条 → 全局库恰一行 + Channel 收到恰一条（其后落库在前、广播在后的装配序可观察为「库行先于 Channel 消息产生」） | 单点 |
| `NotificationBus::new()` + `publish()` | 边界 | 无订阅者时连续 publish 200 条（超容量 128）→ 零 panic、落库全量 200 行（广播丢增量不落库） | 单点 |
| `NotificationBus::new()` + `publish()` | 异常 | 以 workspace 维度 store 构造 bus → 构造成功；`publish` 不 panic、库零行、但订阅端仍收到该条（一 sink 失败不影响 in-app sink） | 单点 |
| `NotificationBus::retire_run()` | 正向 | 预置同 run 两条等待类未读 + 一条他 run 未读 → retire 后同 run 转已读、他 run 不变 | 单点 |
| `NotificationBus::retire_run()` | 边界 | 未知 run / 空 `run_id` → 零写入、零 panic | 矩阵迭代（2 形态） |
| `NotificationBus::retire_run()` | 异常 | workspace 维度 store 构造的 bus → 不 panic、零上抛（失败降级，R-11） | 单点 |
| `notification_subscribe_with()` | 正向 | 已有 3 条未读 → 返回 3；随后 publish 1 条 → Channel 收 1 条；「初值 + 收到条数 == 4」与真实未读相等（不丢，AC-8 硬判据） | 单点 |
| `notification_subscribe_with()` | 正向 | 对照读数（AC-8）：subscribe #1 返回 `U1` → publish N 条 → subscribe #2 返回 `U2`；断言 `U2 == U1 + N`（两次订阅各自独立 receiver，不新增未读查询命令） | 单点 |
| `notification_subscribe_with()` | 边界 | 空库 → 返回 0、Channel 零消息；订阅前已落库的通知不补投（Channel 零条，`notification_list` 可读） | 矩阵迭代（2 形态） |
| `notification_subscribe_with()` | 边界 | 超容量滞后：单次订阅期间连续 publish 200 条 → 收到 ≥ 1 条且不 panic（`Lagged` 继续、`Closed` 才 break）；随后再 publish 一条仍能收到（滞后不断流） | 单点 |
| `notification_subscribe_with()` | 异常 | 计数读取失败（workspace 维度 store 的 bus）→ `Err(String)`，不返回半成品订阅（不 spawn 转发任务） | 单点 |
| `notification_list_with()` | 正向 | `offset` / `limit` 逐值透传至 store；只 `Event`；返回 `NotificationPage { items, total }` | 单点 |
| `notification_list_with()` | 边界 | `limit = 500` → 经命令面复核钳位 200（IPC 滥用兜底）；`offset` 超界 → `items == []`、`total` 不变 | 矩阵迭代（2 形态） |
| `notification_list_with()` | 异常 | workspace 维度 store → `Err(String)`（错误映射 `.to_string()` 单点） | 单点 |
| `notification_mark_read_with()` | 正向 | 3 条未读标 1 条 → 返回 2；再标同一条 → 仍 2（幂等） | 单点 |
| `notification_mark_read_with()` | 边界 | 空 `ids` → 返回当前未读、零变化；未知 id 与 `Feedback` id → 忽略 | 矩阵迭代（2 形态） |
| `notification_mark_read_with()` | 异常 | workspace 维度 store → `Err(String)` | 单点 |
| `notification_mark_all_read_with()` | 正向 | 12 条未读（含未加载页）→ 返回 0；后续 `list` 每行读状态 `Some`（经重订阅读数复核） | 单点 |
| `notification_mark_all_read_with()` | 边界 | 空库 → 0；`Feedback` 行零影响（未读数与列表均不变） | 矩阵迭代（2 形态） |
| `notification_mark_all_read_with()` | 异常 | workspace 维度 store → `Err(String)` | 单点 |
| `notification_record_with()` | 正向 | 合法入参 → 库中恰一行：`kind == Feedback`、`root == None`、`source_ref == None`、`id` == 入参、`created_at > 0`；未读计数不变、Channel 零收到（不广播） | 单点 |
| `notification_record_with()` | 边界 | `body` 三形态（`None` / `Some("")` / 超长）→ 落库保真（`""` 不折成 `None`）；`id` / `message` 极长（>1000）可落库 | 矩阵迭代（3 形态） |
| `notification_record_with()` | 异常 | 空白 `id`（`""` / `"  "` / `"\n"`）或空白 `message` → `Err`，零行写入（空白入参不进入库链路） | 矩阵迭代（3 形态） |
| `notification_record_with()` | 异常 | workspace 维度 store → `Err(String)`（store 失败上抛为命令错误；前端静默属 `lib/notify.ts` 断言面） | 单点 |
| 过滤口径单点（组合：record → list / count） | 正向 | 先 3 条事件 + 2 条反馈 → `notification_list` 只 3 条、未读 3；`notification_mark_all_read` 后未读 0 而反馈行不受影响 | 单点 |
| `NotificationBridge::new()` + `deliver()` | 正向 | 真实 bus（tempdir 全局库）构造 bridge → `deliver(event)` → 库恰一行（字段与载荷逐项一致）+ 已订阅 Channel 恰收一条（`deliver` → `bus.publish` → Hub 两 sink 的接线闭环，接住 core 与 store 之间的最后一跳） | 单点 |
| `NotificationBridge::deliver()` | 边界 | 同一 bridge 连续 `deliver` 同主键两条（`message` 不同）→ 库仍一行、`message` 取后者、`read_at` 保持（幂等归 store，bridge 零状态、不去重） | 单点 |
| `NotificationBridge::deliver()` | 异常 | bridge 持 workspace 维度 store 的 bus → 不 panic、零上抛、库零行，订阅端仍收到该条（与 `bus_publish在workspace维度…` 同形态） | 单点 |
| `NotificationBridge::retire()` | 正向 | 预置同 run 两条等待类未读 + 他 run 一条 → `retire()` 后同 run 转已读、他 run 不变（`run_id` 取自 `context()`，不由入参传） | 单点 |
| `NotificationBridge::retire()` | 边界 | 未知 run / 空 `run_id` → 零写入零 panic（幂等，R-14：miss 即无操作） | 矩阵迭代（2 形态） |
| `NotificationBridge::context()` | 正向 | `context()` 返回 `new` 时传入的五字段逐字相等，且不随 `deliver` / `retire` 变化（自持单份） | 单点 |
| 接线结构锚（AC-4 / DD12） | 异常 | `include_str!("mod.rs")` 两断言：`RunUpdate::` 零命中（壳层零按变体过滤，白名单只在 `notify::project`）；`retire` / `retire_run` 各恰 1 处（本文件是终态清理的唯一实现面：`NotificationBus::retire_run` 一行 store 委托 + `NotificationBridge::retire` 一行转发；`commands/change_flow` 侧零命中由该节结构锚反向证） | 矩阵迭代（2 断言） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| Tauri app / State | 真实 `tauri::test::mock_app()` + `manage(Arc<NotificationBus>)` + 真实 tempdir `WorkspaceStores`（沿 `commands/db/mod_test.rs` / `change_flow/mod_test.rs` 的 `app_with` 装置）；`global_shared()` 须在 `manage(stores)` 之前取 | 全部用例 |
| `Channel<Notification>` | 真实 `tauri::ipc::Channel::new(闭包)` 捕获 `InvokeResponseBody::Json` 文本后反序列化（沿 `change_flow/mod_test.rs` 的 `capturing_channel` 先例）；不 mock 转发任务 | subscribe 转发与滞后行 |
| 广播等待 | 真线程 + `tokio::time::timeout` / 轮询等待（不 mock 调度器）；断言用「最终态」而非固定 sleep | subscribe 两行 |
| 时钟 | 不 mock：`created_at` 只断言 `> 0` | `notification_record_with()` 正向行 |
| `RunNotifier` 出口实现 | **不 mock**：用真实 `Arc<NotificationBridge>`（`new(NotifyContext, bus)`）——bridge 的转发即被测面；需要观察「投递恰一次」时另用记录型 `RunNotifier` 替身（同一 trait） | bridge 全部用例 |

### packages/desktop/src-tauri/src/commands/change_flow/mod.rs -> packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（「公共函数 / API」子表零本文件行）；下列条目取自 design「修改文件」子表本文件行（DD12 收成四字段、`emit` 回纯桥、`ask_started` 一行转发）与 spec `ChangeFlowSink` 契约行所述行为面，用于定位测试，不构成新增公共 API。 -->

- `ChangeFlowSink::ask_started()`: `Ask` 不经 `emit`——本缝是壳层唯一真实可达的事件轨 hook 点：`notify::deliver(&RunUpdate::Ask { … }, notifier, now_millis())` → 既有 `control.publish`（投递在广播之前，主线逐字不动）
- `ChangeFlowSink::emit()`（`RunEventSink` 视图）: 回到纯桥——`SessionEvent` 同步会话槽 + `control.publish`，**零通知逻辑**（`ConfirmWait` / `Finished` 归 orchestration；此处零 `retire` / 零 `RunUpdate::Finished` 判定）
- `change_flow_start_with()`: 构造 `NotifyContext`（五值取自既有局部量，零额外 store 查询）→ `Arc<NotificationBridge>`；同一 `Arc<dyn RunNotifier>` 一份进 sink、一份作 `walk_run` 第 9 参

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ChangeFlowSink::emit()` | 正向 | 反锚（DD12 / 校准 6）：直接 `emit(ConfirmWait { phase })` 与 `emit(Finished(Completed))` → 库零行、订阅端 Channel 零条（该 trait 在旧形态被误当事件轨出口）；同时 `control.publish` 的 `RunNotice` 转发仍发生 | 矩阵迭代（2 变体） |
| `ChangeFlowSink::emit()` | 正向 | `emit(SessionEvent { session_id })` → 会话槽同步（既有断言零改）+ `RunNotice` 转发 + 库零行（纯桥形态的完整面） | 单点 |
| `ChangeFlowSink::emit()` | 边界 | `emit(Step)` / `emit(Edge)` → `control.publish` 照旧、库零行（三排除变体在壳侧同样零通知，白名单仍只在 `notify::project`） | 矩阵迭代（2 变体） |
| `ChangeFlowSink::ask_started()` | 正向 | `ask_started(ask)` → 库恰一行 `Warning`、`id == "run:{run_id}:ask:{tool_call_id}"`、`body == question`、`root` / `source_ref` 取上下文；`control.publish` 的 `Ask` 更新仍流出；`options` 串零出现 | 单点 |
| `ChangeFlowSink::ask_started()` | 边界 | `tool_call_id` 缺位 → 退化主键 `"run:{run_id}:ask:{created_at}"`；同 run 两次不同 `tool_call_id` → 两行（不互相覆盖） | 矩阵迭代（2 形态） |
| `ChangeFlowSink::ask_started()` | 异常 | `ask_resolved` 逐字不动：resolve 后零新增通知行（既有清理路径零随动） | 单点 |
| `change_flow_start_with()` | 正向 | 以 change 记录启动 → bridge 上下文五字段 == `record.name` / `record.workflow_type` / `run_id` / `root` / `id`；`message` 含该 change 名、`body` 由记录 `workflow_type` 查相位表（零额外 store 查询的形态判据） | 单点 |
| `change_flow_start_with()` + `walk_run`（组合：真实 run 链 → 库行） | 正向 | **端到端接线（design 交接清单点名的那一条）**：start → 真实 walker 起跑到停等（`WaitingConfirm`）→ 库恰一行 `run:{run_id}:confirm:archive` 且订阅端恰收一条；另一形态跑至 `Completed` → 库一行 `run:{run_id}:finished`（`Success`）且同 run 等待类未读经 `retire` 转已读；`change_flow_*` 摘要与错误文案逐字不变（既有 `start_sink携带记录四值…` / `start到finished全链路…` 两例在 DD12 后必须转绿） | 矩阵迭代（2 形态） |
| 装置随动 | 边界 | `app_with` 托管 `Arc<NotificationBus>`（`global_shared()` 在 `manage(stores)` 之前取）；11 处 `ChangeFlowSink { … }` 收成四字段（`notifier` 取 `Arc<NotificationBridge>` 或记录型替身）；既有 `change_flow_*` 用例判据零改动 | 单点 |
| 接线结构锚（AC-4 / DD12） | 异常 | `include_str!("mod.rs")` 四断言：`walk_run(` 恰 1 处且实参含 `Some(`；`retire` / `retire_run` 零命中；`RunUpdate::Finished` 零命中（终态判定只在 core）；`notify::deliver` 恰 1 处（`ask_started`） | 矩阵迭代（4 断言） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| Tauri app / 双库 | 沿用既有 `app_with` + 真实 tempdir `WorkspaceStores`，托管 `Arc<NotificationBus>`；故障注入以「workspace 维度 store 构造 bus」实现，不改生产代码 | 全部用例 |
| `RunNotifier` 出口 | **不 mock**：真实 `Arc<NotificationBridge>`（`new(NotifyContext, notification_bus(app))`，沿用既有 `notification_bus()` 辅助件）——bridge 转发即判据；需要观察「投递恰一次 / 零投递」时改用记录型 `RunNotifier` 替身（同一 trait，零生产导出） | emit 反锚与结构锚用例 |
| provider / 引擎侧 | 沿用既有 scripted provider 桩与 `seed_sdk_default_instance` 装置（进程边界替身），与本次变更无关 | start 链路用例 |
| `Channel` 捕获 | 真实 `Channel::new` 捕获（同 `notifications` 节）+ 既有 `CapturedNotifications` 辅助件，不 mock 转发 | 全链路组合行 |

### packages/desktop/src-tauri/src/commands/mod.rs -> packages/desktop/src-tauri/src/bindings/mod_test.rs

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（「公共函数 / API」子表零本文件行）。下列条目取自 design「修改文件」本文件行，用于定位「命令清单随动」判据（AC-11），不构成新增公共 API。 -->

- 模块登记 `pub mod notifications`: 字母序插在 `fs` 与 `stats` 之间
- `all_commands!` 登记面: 尾部按登记序追加五条（`notification_subscribe` / `notification_list` / `notification_mark_read` / `notification_mark_all_read` / `notification_record`）；既有 64 条零位移

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 命令名清单（`COMMAND_NAMES`） | 正向 | 尾部按登记序为 `notification_subscribe` / `notification_list` / `notification_mark_read` / `notification_mark_all_read` / `notification_record`；既有 57 条前缀逐项相等（零位移） | 单点 |
| 包装名清单（`COMMAND_WRAPPERS`） | 正向 | 尾部为 `notificationSubscribe` / `notificationList` / `notificationMarkRead` / `notificationMarkAllRead` / `notificationRecord`；既有前缀逐项相等 | 单点 |
| 两清单长度 | 边界 | 均为 `62`（`57 → 62`），两清单长度恒相等 | 单点 |
| 登记面反向锚 | 异常 | `include_str!("../commands/mod.rs")`：`notification_` 前缀命令名恰 5 处、零重复；`pub mod notifications;` 恰一处（漏登记则命令不可达、清单断言必红） | 矩阵迭代（2 断言） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 生成物产物 | 不 mock：沿用既有 `bindings/mod_test.rs` 的文件锁 + 真实 `export_bindings()`，读磁盘产物文本 | 全部用例 |
| 命令清单 | 不 mock：清单常量即被测期望面（`ALL_COMMANDS` / 宏登记序由生成物侧反向证） | 全部用例 |

### packages/desktop/src/types/generated/bindings.ts / bindings.zod.ts -> packages/desktop/src-tauri/src/bindings/mod_test.rs

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（生成物是派生面，禁手改）。下列条目取自 design「修改文件」该行所述「五命令 typed 包装 + 出线类型随 `bindings:export` 生成」，用于定位 AC-11 判据，不构成新增公共 API。 -->

- 命令包装: 五命令 typed 包装（`commands.notificationSubscribe` / `notificationList` / `notificationMarkRead` / `notificationMarkAllRead` / `notificationRecord`）随 `bindings:export` 生成
- 出线类型与 zod 表项: `Notification` / `NotificationPage` / `NotificationLevel` / `NotificationKind` / `NotificationFeedbackInput` 的类型与 `argsSchemas` / `resultSchemas` / `channelArgs` 表项同批生成

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 生成物（`bindings.ts`） | 正向 | 含五包装名与五 invoke 命令名字面（`notificationSubscribe` / `notification_subscribe` …），与 `COMMAND_NAMES` / `COMMAND_WRAPPERS` 尾部五条一致 | 单点 |
| 生成物 | 正向 | 出线类型 `Notification` / `NotificationPage` / `NotificationLevel` / `NotificationKind` / `NotificationFeedbackInput` 均在 `export type` 集合（`DTO_TYPES` 补录五型） | 矩阵迭代（5 类型） |
| 生成物 | 边界 | `Notification` 类型体零 `readAt` / `read_at`（载荷不含读状态的硬判据）；`NotificationLevel` 联合恰四字面量 `"info"` / `"success"` / `"warning"` / `"error"` | 矩阵迭代（2 断言） |
| 生成物（`bindings.zod.ts`） | 正向 | `argsSchemas` / `resultSchemas` / `channelArgs` 三表各含五新命令键，且与 `commands` 键序一致（前端 `ipc-contract.test.ts` / `ipc.test.ts` 的键遍历面自动覆盖，零改动） | 单点 |
| 生成幂等 | 边界 | 连续两次 `export_bindings()` 后磁盘产物字节相等（无手改、无残留 diff） | 单点 |
| 手改零容忍 | 异常 | 产物与再生成结果逐字节相等（任一字段手改即红） | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 生成物 | 不 mock：真实 `export_bindings()` 写盘后读文本（既有文件锁与 `RestoreOnDrop` 守卫沿用） | 全部用例 |
| TS 侧 | 不 mock：`ipc-contract.test.ts` / `ipc.test.ts` 的 `Object.keys(commands)` 遍历面自动纳入新命令，零改动 | zod 表键集行 |

### packages/desktop/src/lib/notify.ts -> packages/desktop/src/lib/notify.test.ts

#### 待测功能

- `present()`: 四档显式 `switch` 渲染；`body` 在场才补 `{ description }` 第二实参；不向 sonner 传 `id`；零落库
- `notify()`: `present(...)` + fire-and-forget `ipc.notificationRecord`（`crypto.randomUUID()` 主键、静默失败、DEV 一行 `console.warn`、零重试、零再发通知）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `present()` | 正向 | 四档调用形态逐字：`info → toast(message)`（裸调用）/ `success → toast.success(message)` / `warning → toast.warning(message)` / `error → toast.error(message)`；均无第二实参（无 `body`） | 矩阵迭代（4 档） |
| `present()` | 正向 | `body` 在场（四档各一次）→ 第二实参恰为 `{ description: body }`；实参对象只含 `description` 一个键（**无 `id`**） | 矩阵迭代（4 档） |
| `present()` | 正向 | `present(...)` 全程 `notification_record` 调用数为 0（呈现不落库，供后端事件轨复用） | 单点 |
| `present()` | 边界 | `body === ""` → 仍补 `{ description: "" }`（判据是 `!== undefined`，不做真值判断）；`body === undefined` → 零第二实参 | 矩阵迭代（2 形态） |
| `present()` | 边界 | 超长 `message`（>1000 字符）与特殊字符（`\n` / emoji / 引号）→ 原样透传（零截断、零转义） | 矩阵迭代（3 形态） |
| `present()` | 异常 | 四档外的未知 level（类型断言后的运行期脏值）→ 不抛、零 toast 调用（`switch` 无兜底臂即静默；禁伪造第五档行为） | 单点 |
| `notify()` | 正向 | 先呈现 + 落库各一次：`toast.*` 恰一次、经真实 `lib/ipc.ts` 转发的 `invoke('notification_record', { record })` 恰一次，`record` 为 `{ id: 非空 UUID 形态串, level, message, body: body ?? null }` | 单点 |
| `notify()` | 正向 | 四档 level 逐档透传至落库入参（`info` / `success` / `warning` / `error` 原值出线） | 矩阵迭代（4 档） |
| `notify()` | 边界 | `body` 缺席 → 落库入参 `body === null`；`body === ""` → 落库入参 `body === ""`（不归一为 null） | 矩阵迭代（2 形态） |
| `notify()` | 边界 | 两次 `notify` → 两次 `invoke('notification_record', …)` 的 `record.id` 互不相同（`crypto.randomUUID` 每次铸造） | 单点 |
| `notify()` | 异常 | `invoke('notification_record', …)` 返回 reject → 零外溢异常、toast 仍恰一次、该命令的 `invoke` 调用次数仍 1（零重试）、DEV 下 `console.warn` 恰一行 | 单点 |
| `notify()` | 异常 | 真实 `lib/ipc.ts` 的参数面校验同步抛 `IpcArgsError`（由越界入参构造，非替身伪造；`invoke` 零往返）→ 同上：零外溢、toast 恰一次、`console.warn` 恰一行 | 单点 |
| `notify()` | 边界 | `vi.stubEnv('DEV', false)` 下的失败 → 零 `console.warn`、零 `console.error`（prod 零留痕） | 单点 |
| 结构锚（AC-2） | 正向 | 递归扫 `src/**`（排除 `*.test.ts` / `*.test.tsx`）：`from 'sonner'` 恰两处命中——`src/components/ui/sonner.tsx` 与 `src/lib/notify.ts`；12 处原调用点文件零命中 | 矩阵迭代（命中面 2 + 零命中面 6） |
| 结构锚 | 异常 | 六文件零 `toast(` / `toast.error(` / `toast.success(` / `toast.warning(` 直调残留；六文件 import 面均含 `notify` | 矩阵迭代（6 文件 × 2 断言） |
| 结构锚（R-1 / 口径） | 边界 | `lib/notify.ts` 内 `crypto.randomUUID` 恰一处、零降级分支；`ipc.notificationRecord` 调用恰一处（经真实 `lib/ipc.ts`，不批量、不缓存） | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `sonner`（`toast`） | `vi.mock('sonner', ...)` 提供四档 `vi.fn()`（与 6 个既有测试文件同一替身形态，进程边界：渲染器） | 全部调用形态断言 |
| `@tauri-apps/api/core`（`invoke`） | `vi.mock('@tauri-apps/api/core', …)` 提供可编程 `invoke`（按命令名分派，可 resolve / reject / 让参数面校验就地失败）；**真实组合 `lib/ipc.ts`**（Proxy + `argsSchemas` strict 校验 + `IpcArgsError` 同步抛）——与「测试框架与布局」节「进程边界一律 mock `@tauri-apps/api/core`」的口径及仓库先例（`lib/ipc.test.ts`：core 为桩，`bindings` / `ipc-drift` / `ipc` 真实组合）一致；替身化 `./ipc` 会绕开被测的「门面 → ipc 代理 → invoke」链路 | `notify()` 全部用例 |
| `console.warn` | `vi.spyOn(console, 'warn')` 观测调用次数与首参语义；`vi.stubEnv('DEV', false)` 切 prod 分支（沿 `ipc.test.ts` 先例） | DEV 留痕两行 |
| `crypto.randomUUID` | 环境缺该函数时 `vi.stubGlobal('crypto', { randomUUID: vi.fn(...) })`（design 待决 6：测试侧补桩，**MUST NOT** 在门面加降级分支） | `notify()` 全部用例 |
| 源文本面 | 不 mock：`readFileSync` + 目录递归（沿 `module-entries.test.ts` 先例） | 结构锚三行 |

### packages/desktop/src/hooks/use-notifications.ts -> packages/desktop/src/hooks/use-notifications.test.ts

#### 待测功能

- `useNotifications()`: 订阅（`Channel<Notification>` 构造 1 次 + `notification_subscribe` 调用 1 次，root 无关）、初值未读、Channel 消息分流（`Info` 不弹 / 前插 / 未读 +1 / total +1）、`load()` 首屏 50、`markRead` / `markAllRead` 以返回值回写、失败分流（订阅静默 / 列表与标记 inline）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `useNotifications()` 订阅 | 正向 | 挂载一次 → `new Channel` 恰 1 实例、`notification_subscribe` 恰 1 次且实参为该 Channel；resolve 值（`3`）写入 `unread` | 单点 |
| `useNotifications()` 订阅 | 正向 | Channel 消息（`warning` / `error` / `success` 各一条）→ 前插 `items`（最新在前）、`unread` 与 `total` 各 +1、`present` 对应档 toast 恰一次 | 矩阵迭代（3 档） |
| `useNotifications()` 订阅 | 正向 | `Info` 消息 → 前插 + `unread` / `total` 各 +1，但 `toast` 零调用（机械规则，不按 status 分叉） | 单点 |
| `useNotifications()` 订阅 | 边界 | 零未读（subscribe 返回 `0`）→ 徽标态 `0`、零 toast；subscribe 返回非数字（`null`）→ `unread` 归 0（`typeof` 判据） | 矩阵迭代（2 形态） |
| `useNotifications()` 订阅 | 边界 | 卸载后再投递 Channel 消息 → 零 setState 告警、零新增订阅（cleanup 置 `alive = false`）；二次挂载 → `notification_subscribe` 再 1 次（各挂载各一次） | 矩阵迭代（2 形态） |
| `useNotifications()` 订阅 | 异常 | `notification_subscribe` reject → 静默：`error` 仍为 `null`、零 toast、组件可渲染、`unread` 保持 0（订阅失败不阻断壳） | 单点 |
| `useNotifications().load()` | 正向 | `load()` → `notification_list(0, 50)` 恰一次；返回 `{ items, total }` 覆盖状态，`loading` 由 `true` 回到 `false` | 单点 |
| `useNotifications().load()` | 边界 | 空页（`items == []` / `total == 0`）→ 状态清空、零错误；连续两次 `load()` → 两次命令、最后一次结果胜出（无竞态残留） | 矩阵迭代（2 形态） |
| `useNotifications().load()` | 异常 | `notification_list` reject → `error == String(err)`（inline 持久），`items` / `total` 保持原值、`loading` 归 `false` | 单点 |
| `useNotifications().markRead()` | 正向 | `markRead([id])` → 本地 `readIds` 先并入该 id、`notification_mark_read([id])` 恰一次；返回 `2` → `unread === 2`（以返回值回写，不由前端推算） | 单点 |
| `useNotifications().markRead()` | 边界 | `markRead([])` → 命令照发（透传空集合）且返回值回写；连续两次同一 id → 命令两次、`readIds` 仍单元素 | 矩阵迭代（2 形态） |
| `useNotifications().markRead()` | 异常 | 命令 reject → `error` 置位、`unread` 保持原值（前端不自行减一） | 单点 |
| `useNotifications().markAllRead()` | 正向 | `markAllRead()` → `notification_mark_all_read` 恰一次、`unread === 0`、`allRead === true` | 单点 |
| `useNotifications().markAllRead()` | 边界 | 已全读态再调用 → 命令仍一次、`allRead` 保持 `true` | 单点 |
| `useNotifications().markAllRead()` | 异常 | 命令 reject → `error` 置位、`allRead` 不翻转、`unread` 保持原值 | 单点 |
| 跨模块组合（链路：Channel → `present` → sonner） | 正向 | 真实 `lib/notify.ts` + `sonner` 桩：Channel 投 `warning` 带 `body` → `toast.warning(message, { description: body })` 逐字一次；投 `error` 带 `body` → `toast.error(message, { description: body })`（校准 3 的四档同形） | 矩阵迭代（2 档） |
| 跨模块组合（链路：Channel → 状态面） | 边界 | 三条连续消息 → `items` 三条且序 = 到达逆序、`total` = 初值 + 3（初值来自 subscribe 返回） | 单点 |
| 结构锚（AC-10） | 正向 | 本文件内 `new Channel` 恰 1 处、`notificationSubscribe(` 恰 1 处；零 `setInterval`、零 `use-file-watch-signal` 引用（`include_str!` 等价：`readFileSync` 源文本） | 矩阵迭代（4 断言） |
| 结构锚 | 异常 | `use-file-watch-signal.ts` 零 diff（文件内容锚定其既有实现，本次变更零提及） | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（`invoke` / `Channel`） | `vi.mock` 提供可编程 `invoke`（按命令名分派、可切 resolve / reject）+ `ChannelMock` 类（原型 accessor + 私有字段，沿 `use-change-flow-run.test.ts` 形态，避免投递绕过装配层） | 全部用例 |
| `sonner` | `vi.mock('sonner', ...)` 提供四档 `vi.fn()`（观测 `present` 的落点） | 消息分流与链路用例 |
| `lib/notify.ts` | **不 mock**：真实组合（AC-3 / AC-10 的链路判据依赖真实门面；替身会让「Info 也弹 toast」类回归不可见） | 全部消息用例 |
| 时钟 / 定时器 | 不引入假定时器、不 mock 调度：无轮询实现，断言用 `waitFor` 的最终态 | 全部用例 |

### packages/desktop/src/components/notification-bell.tsx -> packages/desktop/src/components/notification-bell.test.tsx

#### 待测功能

- `NotificationBell()`: 铃铛 + 徽标三态（`0` 不渲染 / `≤99` 数字 / `>99` `99+`）+ `StandardDrawer` 收件箱（列表 / 单条已读 / 全部已读 / 空态 / inline 错误）+ 跨根跳转（先 `select` 再 `navigate`，`source_ref` 为 null 惰性）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `NotificationBell()` 徽标 | 正向 | 未读 `0` → 徽标节点不渲染；`7` → 文本 `"7"`；`250` → 文本 `"99+"` | 矩阵迭代（3 态） |
| `NotificationBell()` 徽标 | 边界 | 未读 `99` → `"99"`（上限内数字态）；`100` → `"99+"`（阈值边界）；`1` → `"1"` | 矩阵迭代（3 态） |
| `NotificationBell()` 徽标 | 异常 | subscribe reject → 无徽标、铃铛仍在文档（壳不白屏、零 toast） | 单点 |
| 抽屉打开 | 正向 | 点击铃铛 → 抽屉打开且 `notification_list(0, 50)` 恰一次；列表按后端返回序呈现（倒序由后端保证，前端不再排序） | 单点 |
| 抽屉内容 | 正向 | 行内容齐备：`message` 文本、`body` 在场才渲染、时间列 `new Date(createdAt).toLocaleString()` 形态；四档 `level` 各有可辨图标 | 矩阵迭代（4 档） |
| 抽屉内容 | 边界 | 空态：`items == []` → 空态文案在场、零列表行；`body == null` 的行 → 零 body 节点（不渲染空串） | 矩阵迭代（2 形态） |
| 抽屉内容 | 异常 | `notification_list` reject → 抽屉内 inline 错误文案在场、抽屉不关闭、列表区零行 | 单点 |
| 单条置已读 | 正向 | 点击某行 → `notification_mark_read([id])` 恰一次；该行置灰态（`readIds` 命中） | 单点 |
| 单条置已读 | 异常 | `notification_mark_read` reject → 错误 inline 呈现、抽屉保持打开 | 单点 |
| 全部已读 | 正向 | 点击「全部已读」→ `notification_mark_all_read` 恰一次、所有行灰态、徽标消失（未读归 0） | 单点 |
| 全部已读 | 异常 | `notification_mark_all_read` reject → 错误在抽屉内 inline 呈现、抽屉保持打开、`allRead` 不翻转、未读徽标保持原值（前端不自行清零） | 单点 |
| 跨根跳转（AC-9） | 正向 | 当前根 A，点击 `root = "B"` + `source_ref = <change id>` 的行 → 调用序为 `onSelect("B")` 先、`navigate("/changes/<id>")` 后（URL 探针断言 pathname） | 单点 |
| 跨根跳转 | 正向 | 同根行（`root === props.root`）→ 零 `onSelect`、直接 `navigate` | 单点 |
| 跨根跳转 | 边界 | `sourceRef === null` → 零 `navigate`、零 `onSelect`、不抛错（行仍可点击置已读）；`root === null` 且有 `sourceRef` → 直接 `navigate`（不切根） | 矩阵迭代（2 形态） |
| 跨根跳转 | 边界 | 切根动作零 workspace 动作命令：`add_workspace` / `remove_workspace` 调用数为 0（`select` 是本地状态切换） | 单点 |
| 行渲染健壮性 | 边界 | 超长 `message`（>1000）与特殊字符（`\n` / emoji）→ 文本原样在文档中，渲染不 panic | 矩阵迭代（2 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core` | `vi.mock` 提供 `invoke`（按命令名分派：`notification_subscribe` / `notification_list` / `notification_mark_*` 可编程）+ `ChannelMock`（可投递） | 全部用例 |
| `sonner` | `vi.mock('sonner', ...)` 四档 `vi.fn()`（观测 Channel 消息分流；抽屉内不重复弹） | 徽标与消息行 |
| `react-router` | 不 mock `useNavigate`：以 `MemoryRouter` + URL 探针组件读 `useLocation().pathname`（沿 `app-sidebar.test.tsx` / `change-detail-view.test.tsx` 先例），`onSelect` 用 `vi.fn()` 观察调用序 | 跨根跳转三行 |
| `hooks/use-notifications.ts` / `lib/notify.ts` | **不 mock**：真实组合（组件用例的判据是「徽标 + 抽屉 + 跳转」与真实状态面的耦合；替身会掩盖「未读回写口径」回归） | 全部用例 |

### packages/desktop/src/app.tsx -> packages/desktop/src/app.test.tsx

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（「公共函数 / API」子表零本文件行）。下列条目取自 design「修改文件」该行所述行为面（`ShellHeader` 增 `root` / `onSelect` 并挂载铃铛、挂载点透传、`<Toaster />` 与路由零改动），用于定位测试，不构成新增公共 API。 -->

- `ShellHeader()`: 增 `root: string` / `onSelect: (root: string) => void` 两 props，在更新指示之前挂 `<NotificationBell root={root} onSelect={onSelect} />`
- `App()`: 挂载点传 `root={workspaceState.root}` / `onSelect={workspaceState.select}`；`<Toaster />` 与 `AppRoutes` 零改动

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ShellHeader()` 挂载 | 正向 | 壳态（有 root）渲染后铃铛在场（`aria-label="通知"` 的可访问节点）；`<Toaster />` 单点仍在（既有断言零改） | 单点 |
| `ShellHeader()` 挂载 | 边界 | 欢迎态（`workspaceState.root === null`）→ 铃铛零渲染、`notification_subscribe` 零调用（壳态隔离，零 `root === null` 分支） | 单点 |
| `ShellHeader()` 挂载 | 正向 | 挂载基线：`notification_subscribe` 恰一次，既有 `app_environment` / `list_workspaces` / `list_changes` 次数不变 | 单点 |
| 跨根跳转（组合：铃铛 → 壳层 select → 路由） | 正向 | 点击跨根通知行 → 当前根切换为通知的 `root`（零 `add_workspace` / `remove_workspace`）+ 路由跳 `/changes/<id>`；随后 `list_changes` 以新根取数 | 单点 |
| 订阅失败 | 异常 | `notification_subscribe` reject → 壳照常渲染（侧栏 / 页壳 / 既有视图可用）、零错误 toast、零白屏 | 单点 |
| 命令序列断言随动 | 边界 | `commandsExceptEnvironment()` 的排除面纳入 `notification_subscribe`（或期望序补入）；「点击自身零新增调用」计数基线把 subscribe 计入并保持不变 | 矩阵迭代（2 断言） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core` 等进程边界 | 沿用 `app.test.tsx` 既有 `vi.mock`（`invoke` 按命令名分派 + `plugin-updater` / `plugin-dialog` / `api/app` 桩 + `HashRouter` props 观测）；新增命令在 `mockDispatch` 兜底分支返回 `Promise.resolve(null)` 即被 hook 容忍 | 全部用例 |
| `sonner` | 沿用真实 `toast`（既有 `toast.dismiss()` 清残留），失败面断言以「零 `[data-sonner-toast]`」形态 | 订阅失败行 |
| 路由 | 真实 `HashRouter`（既有形态），不 mock `useNavigate` | 跨根跳转行 |

### packages/desktop/src/hooks/use-workspaces.ts -> packages/desktop/src/hooks/use-workspaces.test.ts

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（「公共函数 / API」子表零本文件行）。下列条目取自 design「修改文件」本文件行（2 处 `toast.error` → `notify('error', …)`，文案逐字不变），用于定位测试，不构成新增公共 API。 -->

- `add` / `remove` 失败提示（原 `:105` / `:120`）: `toast.error(…)` → `notify('error', …)`，文案逐字不变
- import 面: `import { toast } from 'sonner'` → `import { notify } from '../lib/notify'`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| add / remove 失败提示 | 正向 | `add_workspace` 失败 → `toast.error` 收到原文案（既有精确参数断言零改），且 `notification_record` 恰一次、`level == "error"`、`body == null` | 矩阵迭代（2 动作） |
| add / remove 失败提示 | 边界 | 失败提示文案含 `String(err)` 原文（`"Error: ..."` 形态逐字保留，门面不改写） | 单点 |
| 成功路径 | 正向 | add / remove 成功 → `toast` 零调用、`notification_record` 零调用（反馈轨只在提示处触发） | 单点 |
| 落库旁路 | 异常 | `notification_record` reject → 既有 `toast.error` 断言仍成立、hook 无异常外溢（`select` / `refresh` 行为不变） | 单点 |
| 既有调用计数断言 | 边界 | 「`select` 零后端调用」「无轮询」两类 `invoke` 计数断言不受门面影响（失败路径才新增一次 `notification_record`，且按命令计数口径判定） | 矩阵迭代（2 断言） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（`invoke`） | 沿用既有 `invokeMock` 按命令名分派（`notification_record` 走兜底 `Promise.resolve(null)`，失败面按需 override） | 全部用例 |
| `sonner` | 沿用既有 `vi.mock('sonner', ...)`：`toast.error` 观测点（门面内部调用同样被拦截，零改动前提） | 全部用例 |
| `lib/notify.ts` | **不 mock**：真实门面（AC-2 的「文案逐字 + 落库一次」判据必须在真实门面上成立） | 全部用例 |

### packages/desktop/src/views/agents/hooks/use-agent-instances.ts -> packages/desktop/src/views/agents/hooks/use-agent-instances.test.ts

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（「公共函数 / API」子表零本文件行）。下列条目取自 design「修改文件」本文件行（3 处 `toast.error` → `notify('error', …)`），用于定位测试，不构成新增公共 API。 -->

- `save` / `remove` / `setDefault` 失败提示（原 `:90` / `:104` / `:118`）: `toast.error(…)` → `notify('error', …)`，文案逐字不变
- import 面: `import { toast } from 'sonner'` → `import { notify } from '../../../lib/notify'`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| save / remove / setDefault 失败 | 正向 | 三个动作各失败一次 → `toast.error` 收到原文案（既有断言零改），`notification_record` 各恰一次（`level == "error"`、`body == null`） | 矩阵迭代（3 动作） |
| save / remove / setDefault 失败 | 边界 | 文案含 `String(err)` 原文与固定前缀（逐字不变）；提示调用次数与动作失败次数一一对应（不叠发） | 矩阵迭代（3 动作） |
| 成功路径 | 正向 | 三动作成功 → `toast` 零调用、`notification_record` 零调用 | 矩阵迭代（3 动作） |
| 落库旁路 | 异常 | `notification_record` reject → 既有 `toastErrorMock` 断言仍成立、清单刷新行为不变 | 单点 |
| 既有断言面 | 边界 | 锁 `invoke` 总次数的断言只在无提示路径（成功 / 取数），失败路径断言按命令计数收窄 | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（`invoke`） | 沿用既有 `invokeMock` 按命令名分派（`notification_record` 走兜底） | 全部用例 |
| `sonner` | 沿用既有 `vi.mock('sonner', ...)` | 全部用例 |
| `lib/notify.ts` | **不 mock**（真实门面，同 `use-workspaces` 口径） | 全部用例 |

### packages/desktop/src/views/agents/hooks/use-agent-providers.ts -> packages/desktop/src/views/agents/hooks/use-agent-providers.test.ts

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（「公共函数 / API」子表零本文件行）。下列条目取自 design「修改文件」本文件行（2 处 `toast.error` → `notify('error', …)`），用于定位测试，不构成新增公共 API。 -->

- `save` / `remove` 失败提示（原 `:112` / `:126`）: `toast.error(…)` → `notify('error', …)`，文案逐字不变
- import 面: `import { toast } from 'sonner'` → `import { notify } from '../../../lib/notify'`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| save / remove 失败 | 正向 | 两动作各失败一次 → `toast.error` 原文案（既有断言零改）+ `notification_record` 各恰一次（`body == null`） | 矩阵迭代（2 动作） |
| 失败提示 | 边界 | 文案逐字不变（含 `String(err)` 形态）；同一动作重复失败两次 → 两次提示、两次落库（不去重） | 矩阵迭代（2 形态） |
| 成功路径 | 正向 | 两动作成功 → `toast` 与 `notification_record` 均零调用 | 矩阵迭代（2 动作） |
| 落库旁路 | 异常 | `notification_record` reject → 既有断言仍成立、清单刷新零变化 | 单点 |
| 既有断言面 | 边界 | 「取数链稳定后 `toHaveBeenCalledTimes(1)`」类断言位于无提示路径，预期零改动；若有例外按命令计数收窄 | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（`invoke`） | 沿用既有 `invokeMock` 按命令名分派（`notification_record` 走兜底） | 全部用例 |
| `sonner` | 沿用既有 `vi.mock('sonner', ...)` | 全部用例 |
| `lib/notify.ts` | **不 mock**（真实门面） | 全部用例 |

### packages/desktop/src/views/changes/hooks/use-change-list.ts -> packages/desktop/src/views/changes/hooks/use-change-list.toast.test.ts

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（「公共函数 / API」子表零本文件行）。下列条目取自 design「修改文件」本文件行（`toast.warning(msg, { description })` → `notify('warning', msg, body)`、`toast.success(msg)` → `notify('success', msg)`），用于定位测试（AC-2，校准 3 的 body 透传面），不构成新增公共 API。 -->

- 删变更带警告（原 `:50`）: `toast.warning(msg, { description })` → `notify('warning', msg, outcome.warnings.join('\n'))`
- 删变更成功（原 `:52`）: `toast.success(msg)` → `notify('success', msg)`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 删变更成功带警告 | 正向 | 警告结果 → `toast.warning(message)` + 第二实参 `{ description: outcome.warnings.join('\n') }`（逐字对位）；`notification_record` 恰一次且 `body` 同为该串、`level == "warning"` | 单点 |
| 删变更成功 | 正向 | 无警告成功 → `toast.success(message)` 单参；`notification_record` 恰一次（`level == "success"`、`body == null`） | 单点 |
| 文案与次序 | 边界 | 前缀文案逐字不变（既有精确参数断言零改）；同一动作多次 → 提示与落库次数一一对应 | 矩阵迭代（2 形态） |
| 空警告集 | 边界 | `warnings == []` → 走成功档（`toast.success` 单参），不产出 `{ description: "" }` | 单点 |
| 落库旁路 | 异常 | `notification_record` reject → 既有 toast 断言仍成立、清单刷新不变 | 单点 |
| 兄弟测试 | 边界 | `use-change-list.test.ts` 零改动（其 `toHaveBeenCalledTimes(1)` 位于无提示的取数路径） | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（`invoke`） | 沿用既有 `dispatch`（`notification_record` 走兜底 `Promise.resolve(null)`） | 全部用例 |
| `sonner` | 沿用既有 `vi.mock('sonner', () => ({ toast: toastMock }))` | 全部用例 |
| `lib/notify.ts` | **不 mock**（真实门面；`description` 透传面是 AC-2 校准 3 的判据） | 全部用例 |

### packages/desktop/src/views/architecture/components/contract-panel.tsx -> packages/desktop/src/views/architecture/components/contract-panel.test.tsx

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（「公共函数 / API」子表零本文件行）。下列条目取自 design「修改文件」本文件行（裸 `toast(...)` → `notify('info', …)`），用于定位测试（AC-2 的 `Info` → 裸 `toast` 对位），不构成新增公共 API。 -->

- 写契约文件成功提示（原 `:305`）: 裸 `toast(…)` → `notify('info', …)`（对位门面 `Info` → `toast(message)` 单参形态）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 写契约文件成功提示 | 正向 | 生成成功 → 真实 `<Toaster />` 的 `[data-sonner-toast]` 文本含既有文案（`写入 N 份` / `未变 M 份`，DOM 断言零改） | 单点 |
| 落库面 | 正向 | 同一次成功 → `notification_record` 恰一次、`level == "info"`、`body == null`（裸 `toast` 对位单参形态） | 单点 |
| 生成失败 | 边界 | 生成 reject → inline 错误态（既有断言），零 `notification_record`（失败走错误呈现双轨，不经提示轨） | 单点 |
| 落库旁路 | 异常 | `notification_record` reject → DOM toast 文案断言仍成立、面板状态不变 | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（`invoke`） | 沿用既有按命令名分派（`notification_record` 走兜底）；`archi_api_generate` 可编程 | 全部用例 |
| `sonner` | **不 mock**：真实 `toast` + 真实 `<Toaster />`（既有形态，DOM 文本即判据） | DOM 断言行 |
| `lib/notify.ts` | **不 mock**（真实门面） | 全部用例 |

### packages/desktop/src/views/explores/components/explore-open-file.tsx -> packages/desktop/src/views/explores/components/explore-open-file.test.tsx

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（「公共函数 / API」子表零本文件行）。下列条目取自 design「修改文件」本文件行（2 处 `toast.error(String(err))` → `notify('error', String(err))`），用于定位测试，不构成新增公共 API。 -->

- 打开失败 / 建探索失败（原 `:37` / `:68`）: `toast.error(String(err))` → `notify('error', String(err))`，文案逐字不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 打开失败 / 建探索失败 | 正向 | 两处失败各一次 → `toastMock.error` 收到 `String(err)` 原文（既有断言零改）+ `notification_record` 各恰一次（`level == "error"`、`body == null`） | 矩阵迭代（2 处） |
| 失败文案 | 边界 | `String(err)` 形态逐字保留（`"目录不可读"` / `"笔记已被占用"` 两例）；错误类型为 Error 实例与裸字符串两形态 → 均转同一文案 | 矩阵迭代（2 形态） |
| 成功路径 | 正向 | 正常扫描 / 正常创建 → `toast.error` 零调用、`notification_record` 零调用 | 单点 |
| 落库旁路 | 异常 | `notification_record` reject → 既有 `toastMock.error` 断言仍成立、`onCreated` 回调行为不变 | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（`invoke`） | 沿用既有 `invokeMock` 分派（`scan_explores` / `create_explore_record` 可编程；`notification_record` 走兜底） | 全部用例 |
| `sonner` | 沿用既有 `vi.mock('sonner', () => ({ toast: toastMock }))` | 全部用例 |
| `lib/notify.ts` | **不 mock**（真实门面） | 全部用例 |

## 不可测试项

| 模块 | 原因 | 替代保证 |
|---|---|---|
| `packages/desktop/src-tauri/Cargo.toml`（`[workspace] members` + `[workspace.dependencies]` + 根包 `[dependencies]` 三条声明） | 构建清单声明面，无函数 / 分支 / 输出可断言；`members` 缺则该 crate 不参与构建、根包依赖缺则 `Shell → Core.Notification` 的模型边无证据——两者均以编译失败直接暴露 | 编译期强制（任一缺失即 `cargo build` 失败）+ 桌面端 `api:check` 的模型边读数（AC-12） |
| `packages/desktop/src-tauri/crates/core/orchestration/Cargo.toml`、`packages/desktop/src-tauri/crates/infra/store/Cargo.toml` | 同上：单行依赖声明，行为面零可断言 | 编译期强制（`use ::notification::…` 在缺依赖时无法解析）+ AC-1 的边界锚（`notification` crate 侧禁反向依赖） |
| `packages/desktop/src-tauri/crates/core/orchestration/src/lib.rs`（`pub mod notify;` + `pub use notify::{deliver, project, NotifyContext, RunNotifier};`） | 模块登记与再导出行，无行为 | 编译期强制：漏登记则 `notify_test` / `control.rs` 的 `notify::deliver` 与 `commands/{change_flow,notifications}` 的 `use orchestration::notify::…` 立即失败 |
| `packages/desktop/src-tauri/crates/infra/store/src/lib.rs`（`mod notification;` + `NotificationRecord` / `StoreNotificationSink` 导出） | 模块登记与再导出行，无行为 | 编译期强制：漏登记则 `crate::notification::…` 与壳的 `use store::{StoreNotificationSink, …}` 立即失败 |
| `packages/desktop/src-tauri/src/main.rs`（`setup_hook` 的 `global_shared()` → `NotificationBus::new` → `manage`） | binary crate 根：`fn main` 不可被测试模块挂载（仓内零 `main_test.rs` 先例）；`setup_hook` 需真实 WebView runtime，`mock_app` 无法覆盖 `Builder::setup` 的真实执行 | 类型级顺序约束（`manage(stores)` 之后再取 `global_shared()` 即编译错误）+ `NotificationBus` 构造与五项行为由 `commands/notifications/mod_test.rs` 全覆盖 + 桌面端启动自检 |
| `.dev-team/architecture/graph/model/02-backend-core.toml`、`03-backend-infra.toml`、`04-backend-shell.toml`、`06-frontend.toml` | 架构模型资产：声明式 TOML，无函数 / 分支；仓内无害模型 golden 断言（`archi` 的既有测试全用合成 fixture，不读仓内模型目录） | 桌面端 `archi_graph` / `archi_drift` 读数核对（AC-12：56 / 127、`diagnostics` 空、`status = clean`）+ `06-frontend.toml` 的 `path` 与 `notification-bell.tsx` 的 `@/components/standard/drawer` import 行交叉核对（R-12）+ `git diff` 只含自己那几条 |
| `.dev-team/architecture/graph/INDEX.toml` | `api:export` 派生生成物 | 桌面端 `api:export` 幂等复核（AC-12：`INDEX.toml` 无残留 diff） |
| `.dev-team/architecture/decisions/0001-model-granularity.md`（`§1` / `§3` / `§5` / `§9` 复算段） | 决策记录散文，纯文本资产 | 桌面端手工核对「表头 / `repo-archi-model` 计数表 / ADR」三处同值（AC-12） |
| `packages/desktop/package.json`（`version` `0.7.5` → `0.7.6`） | 一行字面量，无行为可断言 | 桌面端 `git diff` 复核 + `tauri.conf.json` 仍为 `"version": "../package.json"`、`src-tauri/Cargo.toml` crate 版本行逐字不变（AC-13） |
| AC-11 的机械门禁部分（`check` 四段链、`cargo test --workspace`、生成物零手改、`INDEX.toml` 幂等） | 静态脚本与全量管线由桌面端相位执行，不在单元测试面内 | 桌面端相位执行；本文档只声明覆盖范围与断言判据，不复制运行脚本细节 |
| AC-12 的读数部分（`archi_graph` / `archi_drift` 输出）、AC-13 的改动集纪律、AC-14 的边界在案（spec 文本 + `capabilities/**` 零 diff + `tauri-plugin-notification` 零依赖） | 前两者是仓级机械读数，后者是文本 / 配置面；均无代码单元可断言（`capabilities/**` 与 spec 文本不是可执行单元） | 桌面端架构页与 `git status --porcelain` 边界比对；AC-14 另有 `notification` crate 清单锚（禁 `tauri` / 禁 `tokio`）与前端零系统通知 API 调用的源文本锚 |
| 私有件：`phase_description`（`notify.rs`）、`RunGuard::with_notifier` 与 `RunGuard` / `StepEmitter` 的私有 `notifier` 字段（`control.rs`）、`scan_notifications` / `walk_notifications` / `notification_key`（`envelope.rs`）、`InAppSink` / `spawn_notification_forward` / `NotificationBus::{subscribe, store}`（`commands/notifications`）、`record` 内部函数（`lib/notify.ts`）、`now_millis`（`commands/notifications`） | 私有函数 / 字段不外露；为其加 `pub` 只服务测试，污染导出面（AGENT.md「no test-only exports」）。`with_notifier` 是 `pub(crate)`，由同 crate 的 `walker::walk_run`（唯一接入口）与测试共用 | 经公共入口行为面覆盖：`project` / `deliver`（相位 body、投递组合点）、`RunGuard::emit` / `finish` / `emitter().attach_output_ref`（旁路接线）、`walk_run` 第 9 参（生产链端到端）、`envelope::entries` / `scan` / `describe::table`、五命令与 `*_with` 测试缝、`NotificationBridge` 三方法、`emit` / `ask_started`、`notify` / `present`；异常形态用内存副本或故障注入（workspace 维度 store）驱动 |
| `Notification` 载荷的「文案质量」判据（措辞是否得体 / 是否「该喊人」） | 不可机械判定；字面唯一真相在 `notify.rs`，spec 只钉 level 映射与「message 必含变更名」要素 | 断言只锚 level / 主键 / 必含要素 / body 来源（相位表 / question / reason），措辞在人工评审面 |
