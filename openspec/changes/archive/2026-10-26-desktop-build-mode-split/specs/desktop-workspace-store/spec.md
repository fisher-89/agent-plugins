# desktop-workspace-store Specification（delta: desktop-build-mode-split）

## MODIFIED Requirements

### Requirement: 全局库与 workspace 库双库布局

store SHALL 按数据维度拆分落盘载体为两组 db 文件：**全局库**注册 user 维度模型组（`WorkspaceRecord` 与 agent 管理两记录 `AgentProviderRecord` / `AgentInstanceRecord`，见「AgentProviderRecord 与 AgentInstanceRecord 模型与管理操作面」）；**workspace 库**按 workspace 分立、每 workspace 恰一个独立 db 文件，注册 `AgentRunRecord`（轮统计行）/ 事件记录（会话转录）/ `ExploreRecord` / `SessionRecord` / `ChangeRecord` / `PhaseRecord` / `ChecklistItemRecord` / `StepRecord` 八个 workspace 维度模型（change 流程状态四模型见「ChangeRecord 状态模型组与 change 域操作面」与 desktop-change-state-store）。全局库 SHALL 位于**数据根**（prod 构建 `<home>/.dev-team`；dev 构建 `<home>/.dev-team/dev`——判别与解析见 desktop-build-modes「数据根三分优先级与路径校验」与「构建模式判别源单点」）并启用与旧单库不同的新文件名；workspace 库 SHALL 位于数据根下 `workspaces/` 子树。store 侧对本分裂零改动：根一分裂，全局库、`workspaces/` 子树与引擎日志 `logs/` 三者自动随根分裂。任一库的模型归属 MUST NOT 混注：workspace 域模型 MUST NOT 出现在全局库注册清单，user 维度模型 MUST NOT 出现在 workspace 库注册清单（维度语义由 db 文件归属承载，见 desktop-data-dimensions）。数据根 SHALL 由 desktop-app 注入，store 内零环境解析（既有 `Store::open` 注入式打开约束不变，MUST NOT 因本分裂新增环境变量读取 / 模式判断 / 第二份根解析）；静态模型注册 SHALL 拆为与两库一一对应的两组 `Models`。

#### Scenario: 模型注册分组

- **WHEN** 审查 store 的静态模型注册与两组 `Models` 的消费方
- **THEN** 全局库组定义 `WorkspaceRecord` / `AgentProviderRecord` / `AgentInstanceRecord`，workspace 库组仅定义 `AgentRunRecord` / 会话转录事件记录 / `ExploreRecord` / `SessionRecord` / `ChangeRecord` / `PhaseRecord` / `ChecklistItemRecord` / `StepRecord`，无交叉注册

#### Scenario: 数据分流写入

- **WHEN** `add_workspace` 注册目录后，对该 workspace 发起会话运行、新建 change 并推进相位，同时在管理页新建 provider 与 agent
- **THEN** 注册记录与 provider / agent 记录落在全局库，会话/转录/轮统计行与 change 流程状态落在该 workspace 库，全局库无 workspace 域记录混入

#### Scenario: 注入式打开不变

- **WHEN** 审查 store 公共 API 与打开流程
- **THEN** 两库路径最终均来自调用方注入（数据根由 desktop-app 解析），store 源码无 home_dir / app data 目录解析逻辑、无构建模式判断

#### Scenario: dev 构建根分裂

- **WHEN** 同一 workspace 先后被 dev 构建与 prod 构建打开（两模式数据根分别为 `<home>/.dev-team/dev` 与 `<home>/.dev-team`）
- **THEN** 两库文件是**同名文件、不同父目录**（`<数据根>/workspaces/` 下同名），数据互不可见、无 redb 文件锁冲突；store 无任何跨进程锁或模式探测代码参与

## Module Contract

| 模块 | 面 | 关键契约 |
|---|---|---|
| `WorkspaceStores::open(data_root: &Path)` | Function | 语义零改动：只接收调用方注入的数据根，由此派生全局库文件与 `workspaces/` 子树；本变更只改变调用方传入的根（prod / dev 子根），store 内零模式判断、零环境变量读取、零第二份根解析 |
| `GLOBAL_DB_FILE_NAME` / `WORKSPACES_DIR_NAME`（`crates/infra/store`） | API | 由 `pub(crate)` 升 `pub` 并在 crate 根重导出：零行为改动，唯一用途是让 desktop 壳的 `--self-check` 摘要与 `app_environment` IPC 不复写库文件名 / 子树名（名字单点） |
| 数据根注入面（store 边界） | Function | 根分裂的**全部**影响收敛于注入值：全局库 / `workspaces/` / 引擎日志三者随根走；store MUST NOT 因本变更引入模式参数、双根并存或迁移逻辑 |
