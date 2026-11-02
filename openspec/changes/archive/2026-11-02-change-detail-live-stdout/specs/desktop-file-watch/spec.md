# desktop-file-watch Specification（delta: change-detail-live-stdout）

## MODIFIED Requirements

### Requirement: 单文件 watch 订阅通道

desktop SHALL 新增通用「watch 文件变化」推送通道：订阅目标是**单个文件**（当前消费者：explore 详情页的当前 `explore.md`、change 详情页运行中脚本步的 exec 输出记录 `.log`）。实现 SHALL 分层落位——notify 单文件订阅与信号流归 `crates/infra/watch/` 新 crate（零 Tauri，产出「文件被修改」信号流）；Tauri Channel 包装 SHALL 留在壳层命令（core 禁 Tauri 纪律不变）。命令面 SHALL 为 `watch_subscribe` / `watch_unsubscribe` 薄命令（新 `commands/watch/` 轨道）：subscribe 建立订阅并绑定 Channel，unsubscribe 解除订阅并停止信号流；对同一文件的重复订阅 SHALL 幂等（不产生重复信号）。订阅目标可能尚未存在或已被删除：**目标文件**缺失而**其父目录在**时 subscribe SHALL NOT 报错（信号流就绪，目标（重）出现后事件生效）；**父目录亦缺失**时 subscribe SHALL 显式失败（`WatchError::ParentUnavailable`，见「父目录缺失的显式失败与订阅顺序不变量」），MUST NOT 以 `Ok` 返回一个永不产信号的订阅。

#### Scenario: 修改信号到达

- **WHEN** 订阅 `openspec/explores/foo.md` 后该文件被外部进程修改保存
- **THEN** 前端经 Channel 收到该文件的修改信号（编辑器 rename-replace 原子保存形态下不丢事件）

#### Scenario: 目标缺失不报错

- **WHEN** 对尚未落盘的 `explore.md`（其父目录 `openspec/explores/` 已存在）发起 `watch_subscribe`
- **THEN** 订阅成功无错误；文件随后被创建时修改信号正常到达

#### Scenario: 父目录缺失显式失败不返回空订阅

- **WHEN** 对 `<change>/reports/test/exec-outputs/<exec_id>.log` 发起 `watch_subscribe`，而 `exec-outputs/` 目录尚未创建
- **THEN** subscribe 返回 `Err(WatchError::ParentUnavailable)`（经壳层错误串映射出线，携带缺失的父目录路径），MUST NOT 返回一个永不产信号的 `Ok` 订阅——「订阅不可能生效」成为可观测事实而非静默空转

## ADDED Requirements

### Requirement: 父目录缺失的显式失败与订阅顺序不变量

watch crate SHALL 区分两种「目标不可达」形态，MUST NOT 合并为同一静默 `Ok`：

- **文件缺失、父目录在**：挂父目录监听，目标（重）出现后事件生效（既有设计意图，语义零变）；
- **父目录亦缺失**：SHALL 以 `WatchError::ParentUnavailable(路径)` 显式失败——原形态下 `watcher.watch()` 压根未被调用而 `Result` 仍是 `Ok`，调用方无法分辨两者。

失败形态 SHALL 只表达「本次订阅不可能生效」，MUST NOT 替调用方造目录（零 fs 副作用纪律不变）、MUST NOT 递归挂祖先目录（无上界递归的成本不可控）、MUST NOT 引入退避重试或重挂状态机——恢复路径为**顺序不变量**：写入方 SHALL 先创建记录（目录与文件落盘）再向消费侧暴露可订阅地址，使「订阅时父目录必然已在」成为调用序列的性质而非运行期兜底。

消费侧的处置责任 SHALL 落在调用方，watch crate MUST NOT 替其决定（MUST NOT 内建轮询兜底）。两处消费方的落地口径 SHALL 为：

- **change 详情 exec 输出面板**：订阅经共享消费件（第 5 条「订阅失败不阻断数据面」——reject 静默、不冒泡为消费方错误态，本变更 MUST NOT 为此改共享件参数面）。其内容面因此**从不依赖 watch 是否存活**：装载首拍与终态整读各取一次全量，watch 只负责运行中的增量跟随；`ParentUnavailable` 在本消费点由「顺序不变量」消除（订阅时父目录必然已在），失败仅作后人破坏顺序时的可观测信号（后端错误 + 前端静默降级为「只在装载 / 收口时看到输出」）。
- **explore 详情页**：父目录 `openspec/explores/` 恒存在，该失败分支实际零触发；其订阅链语义零改动。

#### Scenario: 两形态可分辨

- **WHEN** 分别订阅「父目录已存在而目标文件不在」与「父目录尚不存在」两个路径
- **THEN** 前者返回 `Ok` 订阅（文件创建后出信号）；后者返回 `Err(WatchError::ParentUnavailable(缺失父目录))`——两者可经返回值分辨，不再同为 `Ok`

#### Scenario: 父目录出现后重订即生效

- **WHEN** 先以父目录缺失的路径 subscribe 得到 `Err`，随后该父目录被创建，再次 subscribe 同一路径
- **THEN** 第二次订阅返回 `Ok`；目标文件随后被写入时修改信号正常到达（无需 refresh、无退避等待）

#### Scenario: 顺序不变量使失败面不可达

- **WHEN** 审查 change 详情脚本步的 live 记录链（`LiveExec::begin` → 上抛 `exec_id` → 前端订阅）
- **THEN** 上抛回调触发时刻父目录与 `.log` 文件均已在案；前端拿到 `exec_id` 后订阅的路径父目录必然存在，`ParentUnavailable` 在该消费点不可达（失败分支仅作后人破坏顺序时的安全网）

#### Scenario: 零副作用与零重试

- **WHEN** 审查 `crates/infra/watch/src/lib.rs` 的失败臂实现
- **THEN** 无 `create_dir` / `create_dir_all`（不替调用方造目录）、无祖先目录递归挂载、无退避重试 / 重挂状态机 / 后台线程；`watch_target_of` 的「父目录缺失返回 `None`」休眠语义本体保留，仅其出口由静默 `Ok` 改为显式 `Err`

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/watch/`（零 Tauri） | notify 单文件订阅 | 订阅/退订；「文件被修改」信号流（std mpsc）；零内容、零去抖；**本轮扩失败面**：`WatchError` 由元组结构体改为封闭枚举 `Notify(notify::Error)` / `ParentUnavailable(PathBuf)`——文件缺失（父目录在）= `Ok`（监听休眠于父目录，目标出现即生效），父目录缺失 = `Err(ParentUnavailable)`（不再静默 `Ok` → 零监听）；`subscribe` 的 `watch_target_of` 判定臂是本轮唯一改动点（不预建目录、不递归挂祖先、不重试）；消费者新增 change 详情 exec 输出 `.log` |
| `src-tauri/src/commands/watch/`（命令轨道） | Channel 桥接薄命令 | `watch_subscribe`（绑定 Tauri Channel）/ `watch_unsubscribe`；同文件重复订阅幂等；`Result<T, String>` 错误模板；`ParentUnavailable` 经既有错误串映射出线（**零改动**——错误模板不变，只是多一类因） |
| `packages/desktop/src-tauri/Cargo.toml` | workspace 注册 | `crates/infra/watch` 进 members；notify 依赖收敛于该 crate（**零改动**） |
| `packages/desktop/src/hooks/use-file-watch-signal.ts` | 前端信号消费（单点） | Channel 建立 / `resolvePath` → `watch_subscribe` / trailing 防抖（`SIGNAL_DEBOUNCE_MS = 500`）/ 卸载与臂键变化退订（含迟到补发）/ 订阅失败不阻断数据面；零取数、零状态、零载荷语义；参数面恰三项——explore 详情页与 change 详情脚本输出共用（**本变更零改动**：第二个消费方接入不开新参数、不新增常量） |
| `packages/desktop/src/views/explores/hooks/use-explore-doc.ts` | explore 详情页消费方 | 持有取数语义与 `loading` / `error` 状态；`armKey = root + name`；信号重拉不置 `loading`、不清 `error`（**零改动**：explore 父目录恒存在，`ParentUnavailable` 实际零触发） |
| `packages/desktop/src/views/changes/hooks/use-exec-output.ts` | change 详情脚本输出消费方（新增第二消费方） | 仅经 `useFileWatchSignal` 订阅（`armKey = running ? root + execId : null`），MUST NOT 自建 `Channel` / 直呼 watch 命令 / 自建防抖；订阅失败静默降级（面板仍可经装载首拍与终态整读取到内容） |

### Function: `crates/infra/watch/src/lib.rs`

| 项 | 签名 / 形态 | 描述 |
|------|------|------|
| `WatchError` | `enum { Notify(notify::Error), ParentUnavailable(PathBuf) }`（实现 `Display` + `Error`） | 失败面两分：notify 初始化 / 挂载失败 vs 父目录不可用；`Display` 文案携带缺失父目录路径 |
| `subscribe` | `(path: PathBuf, sender: Sender<FileWatchSignal>) -> Result<Watcher, WatchError>` | 签名不变；仅在导出臂语义收紧——父目录缺失即 `Err`，其余零改动 |
| `watch_target_of` | `fn(&Path) -> Option<PathBuf>`（文件私有） | 判定本体零改动（父目录非空且是目录才返 `Some`）；其 `None` 出口在 `subscribe` 内改为 `ParentUnavailable` |

### Component

无。本能力 MUST NOT 引入 UI 面（零新增组件、零新增路由、零可见状态）；explore 消费方呈现层零改动。
