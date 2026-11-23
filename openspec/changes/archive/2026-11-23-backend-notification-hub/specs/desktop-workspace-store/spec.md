# desktop-workspace-store Specification（delta: backend-notification-hub）

## Purpose

本 delta 只在**全局库模型组**里补一个 user 维度模型（通知收件箱 `NotificationRecord`）并补两条判据；既有公开 API 形态、canonical key 语义、native_db 类型边界、信封 API、单进程约束与全部 workspace 维度陈述逐字不动。收件箱的字段面、读写方法与过滤口径归 `desktop-notification-hub` 持有，本 delta 只登记它的库归属。

## MODIFIED Requirements

### Requirement: 全局库与 workspace 库双库布局

store SHALL 按数据维度拆分落盘载体为两组 db 文件：**全局库**注册 user 维度模型组（`WorkspaceRecord` 与 agent 管理两记录 `AgentProviderRecord` / `AgentInstanceRecord`，见「AgentProviderRecord 与 AgentInstanceRecord 模型与管理操作面」；通知收件箱 `NotificationRecord`，见 desktop-notification-hub「收件箱持久化与命令面」）；**workspace 库**按 workspace 分立、每 workspace 恰一个独立 db 文件，注册 `AgentRunRecord`（轮统计行）/ 事件记录（会话转录）/ `ExploreRecord` / `SessionRecord` / `ChangeRecord` / `PhaseRecord` / `ChecklistItemRecord` / `StepRecord` 八个 workspace 维度模型（change 流程状态四模型见「ChangeRecord 状态模型组与 change 域操作面」与 desktop-change-state-store）。全局库 SHALL 位于**数据根**（prod 构建 `<home>/.dev-team`；dev 构建 `<home>/.dev-team/dev`——判别与解析见 desktop-build-modes「数据根三分优先级与路径校验」与「构建模式判别源单点」）并启用与旧单库不同的新文件名；workspace 库 SHALL 位于数据根下 `workspaces/` 子树。store 侧对本分裂零改动：根一分裂，全局库、`workspaces/` 子树与引擎日志 `logs/` 三者自动随根分裂。任一库的模型归属 MUST NOT 混注：workspace 域模型 MUST NOT 出现在全局库注册清单，user 维度模型 MUST NOT 出现在 workspace 库注册清单（维度语义由 db 文件归属承载，见 desktop-data-dimensions）。数据根 SHALL 由 desktop-app 注入，store 内零环境解析（既有 `Store::open` 注入式打开约束不变，MUST NOT 因本分裂新增环境变量读取 / 模式判断 / 第二份根解析）；静态模型注册 SHALL 拆为与两库一一对应的两组 `Models`。

#### Scenario: 模型注册分组

- **WHEN** 审查 store 的静态模型注册与两组 `Models` 的消费方
- **THEN** 全局库组定义 `WorkspaceRecord` / `AgentProviderRecord` / `AgentInstanceRecord` / `NotificationRecord`，workspace 库组仅定义 `AgentRunRecord` / 会话转录事件记录 / `ExploreRecord` / `SessionRecord` / `ChangeRecord` / `PhaseRecord` / `ChecklistItemRecord` / `StepRecord`，无交叉注册

#### Scenario: 数据分流写入

- **WHEN** `add_workspace` 注册目录后，对该 workspace 发起会话运行、新建 change 并推进相位，同时在管理页新建 provider 与 agent
- **THEN** 注册记录、provider / agent 记录与通知收件箱记录落在全局库，会话/转录/轮统计行与 change 流程状态落在该 workspace 库，全局库无 workspace 域记录混入

#### Scenario: 收件箱模型维度隔离

- **WHEN** 分别在全局库实例与某个 workspace 库实例上调用通知收件箱的读写方法（`publish_notification` 等，签名见 desktop-notification-hub）
- **THEN** 全局库实例正常读写；workspace 库实例返回 `StoreError::Db`（该库的模型组未注册 `NotificationRecord`，native_db 拒绝该类型操作），MUST NOT 静默写入或跨库委派；`NotificationRecord` 不出现在 workspace 库的模型清单与 db 文件内

#### Scenario: 注入式打开不变

- **WHEN** 审查 store 公共 API 与打开流程
- **THEN** 两库路径最终均来自调用方注入（数据根由 desktop-app 解析），store 源码无 home_dir / app data 目录解析逻辑、无构建模式判断

#### Scenario: dev 构建根分裂

- **WHEN** 同一 workspace 先后被 dev 构建与 prod 构建打开（两模式数据根分别为 `<home>/.dev-team/dev` 与 `<home>/.dev-team`）
- **THEN** 两库文件是**同名文件、不同父目录**（`<数据根>/workspaces/` 下同名），数据互不可见、无 redb 文件锁冲突；store 无任何跨进程锁或模式探测代码参与

## Module Contract

### 模块增量：全局库模型组（backend-notification-hub）

| 模块/概念 | 维度 | 关键契约 |
|---|---|---|
| 全局库模型组（`global_models()`） | user | 四模型：`WorkspaceRecord` / `AgentProviderRecord` / `AgentInstanceRecord` / `NotificationRecord`（新） |
| `NotificationRecord`（模型） | user | 主键 `id`（事件为幂等串、反馈为前端铸造 UUID）+ 载荷字段 + `read_at`；表与字段说明由 `///` doc 派生；`MODEL_ENTRIES` 登记一行 ⇒ db 查看器零模型特定代码可见；字段面与语义归 desktop-notification-hub |
| `Store` 的通知读写面 | user | `publish_notification` / `list_notifications` / `unread_notification_count` / `mark_notifications_read` / `mark_all_notifications_read` / `retire_run_notifications`；只在全局库实例可用（workspace 实例返回 `StoreError::Db`） |
| `StoreNotificationSink`（`crates/infra/store`） | user | `impl notification::Sink`：落库 + 失败 `eprintln!` 一行降级（通知是旁路，MUST NOT 上抛到业务调用） |
| workspace 库模型组 | workspace | 零改动（`NotificationRecord` MUST NOT 注册进该组） |
