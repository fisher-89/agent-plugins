# desktop-file-watch Specification

## Purpose

定义通用「watch 文件变化」推送通道契约：notify 单文件订阅、失效信号语义（通知不含内容）、壳层 Channel 桥接与生命周期边界。该通道是与执行流通道（`agent_start` Channel）并列的第二个被认可的推送语义，是「显式取数、无 watch、无轮询」既有取数纪律的唯一显式例外。前端消费侧由共享消费件单点收拢（Channel 建立、订阅、防抖、退订），取数与状态仍归各消费方。

## Requirements

### Requirement: 单文件 watch 订阅通道

desktop SHALL 新增通用「watch 文件变化」推送通道：订阅目标是**单个文件**（当前消费者：explore 详情页的当前 `explore.md`）。实现 SHALL 分层落位——notify 单文件订阅与信号流归 `crates/infra/watch/` 新 crate（零 Tauri，产出「文件被修改」信号流）；Tauri Channel 包装 SHALL 留在壳层命令（core 禁 Tauri 纪律不变）。命令面 SHALL 为 `watch_subscribe` / `watch_unsubscribe` 薄命令（新 `commands/watch/` 轨道）：subscribe 建立订阅并绑定 Channel，unsubscribe 解除订阅并停止信号流；对同一文件的重复订阅 SHALL 幂等（不产生重复信号）。订阅目标可能尚未存在或已被删除：subscribe SHALL NOT 因目标缺失报错（信号流就绪，目标（重）出现后事件生效）。

#### Scenario: 修改信号到达

- **WHEN** 订阅 `openspec/explores/foo.md` 后该文件被外部进程修改保存
- **THEN** 前端经 Channel 收到该文件的修改信号（编辑器 rename-replace 原子保存形态下不丢事件）

#### Scenario: 目标缺失不报错

- **WHEN** 对尚未落盘的 `explore.md` 发起 `watch_subscribe`
- **THEN** 订阅成功无错误；文件随后被创建时修改信号正常到达

### Requirement: 失效信号语义（非数据通道）

watch 通知 SHALL 只携带「目标文件被修改」信号与订阅标识，MUST NOT 携带任何文件内容——通道是**失效信号通道而非数据通道**。数据面 SHALL 维持既有取数纪律：前端收到信号后防抖，再经既有取数命令（如 `read_explore`）显式拉取刷新；后端 SHALL 只透传修改事件、MUST NOT 下沉去抖逻辑（防抖窗口由前端持有）。MUST NOT 将该通道扩展为内容推送、目录树事件广播或任意轮询替代品。

前端持有防抖窗口的实现 SHALL 单点化：Channel 建立、布局派生路径订阅、trailing 防抖合并、卸载退订四件事 SHALL 集中在共享消费件 `packages/desktop/src/hooks/use-file-watch-signal.ts`（见「共享文件 watch 信号消费件」）；消费方 hook MUST NOT 自建 `Channel`、MUST NOT 直呼 `watch_subscribe` / `watch_unsubscribe`、MUST NOT 各自实现文件信号防抖。**单点口径的范围**：本纪律约束的是**生产代码**（`packages/desktop/src` 下排除 `*.test.ts` / `*.test.tsx`）；测试装置内的订阅探针与 `Parameters<typeof ipc.watchSubscribe>` 类类型断言是验证手段、MUST NOT 被当成第二消费点，也 MUST NOT 为满足本纪律而被删改（测试经公开 API 驱动消费方）。**划界**：本纪律只约束「文件失效信号」驱动源；以 run 内存通知（如 `change_flow_watch` 的 kind-only `RunNotice`）驱动、且无路径解析 / 无订阅 / 无退订的防抖（`views/changes/hooks/use-change-detail.ts` 的 `useDebouncedBump`）MUST NOT 被并入本件，其归属与语义零改动。

#### Scenario: 通知无内容且触发显式拉取

- **WHEN** 一次修改信号到达前端
- **THEN** 通知体不含文件内容字节；前端防抖后恰发起一次 `read_explore` 刷新（防抖窗口内多次信号合并为一次拉取）

#### Scenario: 消费侧样板单点化

- **WHEN** 在**非测试生产文件**（`packages/desktop/src` 下排除 `*.test.ts` / `*.test.tsx`）内检索 `new Channel<FileWatchEvent>` 与 `ipc.watchSubscribe` / `ipc.watchUnsubscribe` 的调用点
- **THEN** 命中行的文件路径唯一，且为 `packages/desktop/src/hooks/use-file-watch-signal.ts`（该件内 `Channel<FileWatchEvent>` 构造 1 处、`ipc.watchSubscribe` 与 `ipc.watchUnsubscribe` 各 1 处，同属一个消费点——判据是文件路径唯一，不按命中行数计）；explore 详情页取数 hook 内零文件 watch `Channel` 构造、零 watch 命令调用、零防抖器实现（`Channel<AgentRunMessage>` / `Channel<RunNotice>` 属执行流与 change run 通知语义；`*.test.ts(x)` 内的订阅探针与类型断言属验证手段——两组均不在本纪律管辖内）

#### Scenario: 驱动源划界不越界

- **WHEN** 审查 `views/changes/hooks/use-change-detail.ts`
- **THEN** 该文件未 import 共享消费件、`useDebouncedBump` 与 300ms 通知去抖常量逐字不变（run 通知驱动源不在本件职责内）

### Requirement: 生命周期与平台边界

订阅生命周期 SHALL 即消费页面生命周期：详情页卸载（含 workspace 切换 replace 回清单）即 `watch_unsubscribe`，退订后 MUST NOT 再有信号到达前端。平台边界 SHALL 显式留痕：本应用目标平台为 Windows（notify 底层经父目录实现，单文件 watch 在编辑器 rename-replace 原子保存下不丢事件）；类 Linux/inotify 平台单文件 watch 存在 rename 失效陷阱，跨平台支持时再议（本变更 MUST NOT 为此预建抽象）。

#### Scenario: 卸载退订

- **WHEN** 用户从 explore 详情页切换 workspace（replace 回 `/explores`）或关闭页面
- **THEN** `watch_unsubscribe` 被调用；此后修改该文件前端不再收到信号

#### Scenario: 平台边界留痕可考

- **WHEN** 查阅 watch 实现的代码注释或 crate 文档
- **THEN** Windows 目标平台语义与 Linux inotify 陷阱均有显式说明，无跨平台兼容层代码

### Requirement: 共享文件 watch 信号消费件

前端 SHALL 提供唯一共享消费件 `packages/desktop/src/hooks/use-file-watch-signal.ts`，导出单一 hook `useFileWatchSignal(params) => void`，参数面 SHALL 恰为三项：

```
armKey: string | null          —— 订阅身份键；null = 不订阅
resolvePath: () => Promise<string | null>   —— 布局派生路径（命令层给出）
onSignal: () => void           —— 信号到达（防抖后）调用
```

该件 SHALL 单点收拢五件事，且 MUST NOT 承担超出这五件的职责：

1. **Channel 建立**：`Channel<FileWatchEvent>` 实例建立并挂派发；
2. **订阅**：`resolvePath()` 返回 null 则不订阅（零 `watch_subscribe`）；否则以派生路径 `watch_subscribe(channel, path)`；
3. **trailing 防抖**：窗口 SHALL 为文件私有常量 `SIGNAL_DEBOUNCE_MS = 500`，窗口内多次信号合并为一次 `onSignal`；MUST NOT 把窗口宽度开成参数（无第二取值消费者，不为单次使用造抽象）；
4. **退订与清理**：臂键变化 / 卸载时取消失效计时并 `watch_unsubscribe(subscriptionId)`；订阅迟到就绪（清理已发生）MUST 立即补发退订；
5. **订阅失败不阻断数据面**：订阅链 reject MUST NOT 冒泡为消费方错误态、MUST NOT 阻断消费方任何取数路径（页面仍可显式刷新，仅失自动刷新）。

边界纪律：信号 SHALL 视为**无载荷**——本件 MUST NOT 读取、解释、转发信号载荷（载荷只作「有事发生」判据）；本件 MUST NOT 取数（不调 `read_explore` 等数据命令）、MUST NOT 触碰消费方的 `loading` / `error` 状态、MUST NOT 在挂载或重臂时替消费方发起取数（取数时机属消费方语义）；`resolvePath` / `onSignal` SHALL 以最新引用生效，MUST NOT 因回调身份变化而重订订阅。

#### Scenario: 500ms 窗口合并为一次信号

- **WHEN** 订阅就绪后 500ms 内连续到达 3 条修改信号
- **THEN** `onSignal` 恰被调用 1 次；窗口内新信号到达即重置计时（trailing），最后一拍必达

#### Scenario: 派生路径为空则不订阅

- **WHEN** `resolvePath()` 解析为 `null`（如非法名 / 空 root）
- **THEN** 不发 `watch_subscribe`、不建立有效订阅；卸载时也不调用 `watch_unsubscribe`（无订阅可退）

#### Scenario: 订阅失败不阻断数据面

- **WHEN** `watch_subscribe` 被后端拒绝
- **THEN** 消费方无错误态、页面不崩、显式刷新仍可拉取数据；本件不抛、不重试、不上报新错误通道

#### Scenario: 订阅迟到就绪仍退订

- **WHEN** 消费方在本件订阅链就绪前卸载（或臂键已变更）
- **THEN** `watch_subscribe` 返回的 `subscription_id` 被立即用于补发一次 `watch_unsubscribe`，无残留订阅

#### Scenario: 零取数与零状态

- **WHEN** 一条信号经本件防抖后调用 `onSignal`
- **THEN** 本件自身未调用任何数据命令、未修改 `loading` / `error` 等消费方状态；取数只由消费方的 `onSignal` 实现发起

### Requirement: 订阅臂键与重订边界

订阅重建 SHALL 只由 `armKey` 变化触发。`armKey` SHALL 为**订阅身份**（寻址维度，如 `root` + `name`）的无碰撞编码；同一 `armKey` 连续渲染 MUST NOT 触发重订（零 `watch_unsubscribe` / `watch_subscribe` 对）。数据面重查维度（如显式刷新 tick）MUST NOT 进入 `armKey`：显式刷新 MUST NOT 触发退订或重订。`armKey` 为 null SHALL 表示「不订阅」（不建 Channel、零 watch 命令、零退订），由消费方自行处理其空态语义。

#### Scenario: 显式刷新不重订

- **WHEN** 消费方在订阅就绪后调用其显式 `refresh()`（数据面重取数）
- **THEN** `watch_subscribe` 仍为 1 次、`watch_unsubscribe` 为 0 次；数据面照常重新取数

#### Scenario: 订阅身份变化即重订

- **WHEN** `armKey` 由旧身份变为新身份（如 explore 名从 `old-doc` 变为 `new-doc`，或切换 workspace 根）
- **THEN** 旧订阅以旧 `subscription_id` 退订、新派生路径重新 `watch_subscribe`；退订后旧路径的信号不再到达前端

#### Scenario: 非定址态不订阅

- **WHEN** 消费方以 `armKey = null` 渲染（空参 / 未选中 / 非运行态）
- **THEN** 本件零 `Channel` 构造、零 `watch_subscribe` / `watch_unsubscribe` 调用

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/watch/`（零 Tauri） | notify 单文件订阅 | 订阅/退订；「文件被修改」信号流（std mpsc）；目标缺失不报错；零内容、零去抖 |
| `src-tauri/src/commands/watch/`（命令轨道） | Channel 桥接薄命令 | `watch_subscribe`（绑定 Tauri Channel）/ `watch_unsubscribe`；同文件重复订阅幂等；`Result<T, String>` 错误模板 |
| `packages/desktop/src-tauri/Cargo.toml` | workspace 注册 | `crates/infra/watch` 进 members；notify 依赖收敛于该 crate |
| `packages/desktop/src/hooks/use-file-watch-signal.ts` | 前端信号消费（单点） | Channel 建立 / `resolvePath` → `watch_subscribe` / trailing 防抖（`SIGNAL_DEBOUNCE_MS = 500`）/ 卸载与臂键变化退订（含迟到补发）/ 订阅失败不阻断数据面；零取数、零状态、零载荷语义；参数面恰三项，不开 `debounceMs` / `enabled` / `retry` / 错误回调——explore 详情页与后续 change 详情脚本输出共用 |
| `packages/desktop/src/views/explores/hooks/use-explore-doc.ts` | explore 详情页消费方 | 持有取数语义（`read_explore` + 失败降级信封）与 `loading` / `error` 状态；`armKey = root + name`（不含刷新 tick）；信号重拉**不置** `loading`、不清 `error`；对外仅 `ExploreDocState` / `useExploreDoc` 两个导出 |

### Function: `packages/desktop/src/hooks/use-file-watch-signal.ts`

| 项 | 签名 / 形态 | 描述 |
|------|------|------|
| `useFileWatchSignal` | `(params: { armKey: string \| null; resolvePath: () => Promise<string \| null>; onSignal: () => void }) => void` | 文件 watch 失效信号消费件；effect 依赖仅 `[armKey]`，`resolvePath` / `onSignal` 经 ref 取最新引用；五件事见「共享文件 watch 信号消费件」 |
| `SIGNAL_DEBOUNCE_MS` | `500`（文件私有，**不导出**） | 铁律级常量单点；消费方 MUST NOT 各自定义防抖窗口 |

调用面约束：本件 SHALL 经 `src/lib/ipc.ts` 代理面调用 `watchSubscribe` / `watchUnsubscribe`（MUST NOT 绕回裸 `commands.*`），`Channel` 类型取自生成物 `Channel<FileWatchEvent>`。

### Component

无。本能力 MUST NOT 引入 UI 面（零新增组件、零新增路由、零可见状态）；消费方呈现层（`explore-detail-view.tsx` / `components/**`）零改动。
