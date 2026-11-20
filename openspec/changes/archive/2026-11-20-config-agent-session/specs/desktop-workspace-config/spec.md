# desktop-workspace-config Specification (Delta)

## MODIFIED Requirements

### Requirement: 配置页呈现面

前端 SHALL 在侧栏「页面」组新增 [配置] 入口（`data-testid="nav-config"`），经路由 `/config` 承载（路由与 NavLink 契约见 desktop-page-routing）。配置页 SHALL 以**当前工作区根**为数据基准（同基础信息页，根为 sidebar 本地态不进 URL）；仅壳态可达：欢迎态（root 为 null）MUST NOT 渲染壳与该入口。

页面的交互形态 SHALL 为「**常驻 config 会话 + 配置呈现 + 应用**」：左栏常驻 `source = "config"` 的会话面板（契约归 `desktop-config-agent`「配置页常驻 config 会话面板」），右栏为配置呈现区与显式刷新入口。写回 SHALL 遵守**唯一入口**边界：

- 写回 `config.json` 的唯一入口 SHALL 为会话提案卡（`propose_config`）上的「应用」按钮 → `workspace_config_apply` 命令（契约归 `desktop-config-agent`「写面命令 `workspace_config_apply` 与写权边界」）；命令轨道 MUST NOT 提供第二个写回命令，页面 MUST NOT 提供编辑 / 保存 / 删除入口或内联 JSON 编辑器；
- **agent 零写权限**：`config.json` 的内置写保护语义不变（见 `desktop-write-protection`）；模型 MUST NOT 经工具面写该文件，apply 不经 agent 工具面（由用户点击触发）；
- **零 `InvalidValue` 门**：候选不合法时写面 `Err` 且文件零变化；合法写回后页面 SHALL 重取一次报告（`useWorkspaceConfig().refresh()`），新值就地呈现；
- **规范化明示**：可应用态 SHALL 明示「写回会规范化文件（未知子键、键序与格式会丢失 / 变化）」——用户在下笔前即知整份文件会被重排（字母序 + pretty 展开）。

呈现面 SHALL 分区：

- **基础配置**：`$schema` / `schema` / `context` / `static_analysis` / `projects`（逐 project 呈现 `root`；`language` / `depends_on` 已设时并列呈现，未设时标注「未设」——呈现面 MUST NOT 隐藏已配置值；空列表标注）/ `rules`；
- **tests 面板**：逐 suite 呈现 root / framework / cwd / config / includes / excludes / coverage 阈值（lines / branches / functions）/ mutation（cwd / score）；未设阈值 SHALL 标注「未设（默认 N）」以区分文件值与默认值；
- **write_protection 面板**：逐条 glob 规则与 reason；
- **未知字段**：passthrough 字段可见（呈现形态 design 定夺）；
- **diagnostics 警示区**：单列呈现违例项（哪项非法、哪些字段吃了默认），与正常配置区视觉可分。

config.json 不存在 SHALL 呈现空态而非错误；坏 JSON 与命令 reject SHALL 按「错误呈现双轨」inline 持久呈现（testid 承载、无 toast 顶替）。

#### Scenario: 分区渲染

- **WHEN** 对含 tests 两个 suite 与 write_protection 规则的 fixture 工作区完成解析
- **THEN** 基础配置、tests 面板（逐 suite 全字段）、write_protection 面板各自呈现对应内容，字段值与文件一致

#### Scenario: 未设阈值标注默认

- **WHEN** 某 suite 的 coverage 未设 lines
- **THEN** 该阈值呈现为「未设（默认 80）」类标注，与文件显式设值的呈现形态可区分

#### Scenario: diagnostics 警示区

- **WHEN** config.json 含非法字段（吃默认）或未知字段
- **THEN** 警示区逐项列出违例与默认值来源，passthrough 未知字段仍在配置区可见，页面不崩溃

#### Scenario: projects 语言与依赖呈现

- **WHEN** fixture 的 `projects` 为 `[{root: "packages/desktop", language: "typescript"}, {root: "packages/desktop/src-tauri", depends_on: ["packages/desktop"]}]`
- **THEN** 逐 project 行呈现 `root` 与其已设字段（首行呈现 `language`，次行呈现 `depends_on` 列表）；未设字段呈现「未设」标注而非空白；呈现分区本身零编辑控件（写回只经左栏提案卡的「应用」）

#### Scenario: 空态

- **WHEN** 工作区无 config.json
- **THEN** 页面呈现空态文案（非错误、无 error-note），说明该工作区未配置

#### Scenario: 错误态 inline 持久

- **WHEN** `workspace_config` reject 或报告携 JSON 语法非法情形
- **THEN** 页面 inline 持久呈现错误（testid 承载、不自动消失），无 toast 顶替

#### Scenario: 写权边界（唯一入口）

- **WHEN** 审查配置页组件、命令轨道与会话工具面
- **THEN** 写回唯一入口为提案卡「应用」→ `workspace_config_apply`；命令轨道无第二个写命令、页面无编辑 / 保存 / 删除入口；config 会话名字清单不含写工具，agent 无法写 `config.json`（内置写保护锁定在案）

#### Scenario: 应用成功后重取

- **WHEN** 用户在提案卡上点击「应用」且写面返回 `Ok`
- **THEN** 页面重取一次 `workspace_config`（新值就地呈现，diagnostics 与写后盘面一致），无非显式轮询、无自动重取叠加

#### Scenario: 写面失败零变化且可见

- **WHEN** 「应用」触发 `workspace_config_apply` 返回 `Err`（脏候选 / 写失败）
- **THEN** 页面 inline 持久呈现错误原因，`config.json` 内容与 mtime 逐字不变，该卡保持可重试

#### Scenario: 规范化明示

- **WHEN** 提案卡处于可应用态
- **THEN** 卡片呈「写回会规范化文件（未知子键、键序与格式会丢失 / 变化）」提示；提示 MUST NOT 被藏进 tooltip 或折叠区

#### Scenario: 切换工作区重取

- **WHEN** 用户在 `/config` 页点击 sidebar 另一 workspace 清单项
- **THEN** 页面以新根重新发起解析，旧根配置不再呈现


### Requirement: 项目协议语义边界与消费点纪律

新增 `projects[].language` / `projects[].depends_on` 时 SHALL 同时立规，防「已声明、零消费」第三次重演（`context` / `rules` 为存量欠账）：

- **`depends_on` 是平级边，MUST NOT 充当架构模块层级来源**：A 依赖 B 只表达安装顺序与（将来的）影响面传播方向；架构模块的包含 / 父子关系唯一真相源为元素 FQN 最长前缀（`core/archi` 的 `OwnerIndex`）与模型声明纪律「子元素须先声明祖先」。第二真相源 MUST NOT 出现。
- **`language` 协议层不归一，消费侧归一单点**：协议只存原文（不 trim、不大小写 / 别名归一）；将来按语言分派 prompt / 静态检查 / 测试 SHALL 经**唯一归一入口**（`language_key` 类函数，落 `core/config` 导出）实现，MUST NOT 在各消费点各写一套映射。
- **新增字段须有消费点或显式标注为声明型**：配置字段的消费点 SHALL 在本 spec 可考；暂无工程消费者的新字段 SHALL 显式标注「声明型 + 计划消费者」，MUST NOT 以「先配起来，将来再说」形态沉默落盘。
- **写面不算逐字段消费者**：写面（`plan_apply_config` / `to_file_value`）SHALL 被视为**全字段透传**而非逐字段消费——它唯一的字段名清单是 `KNOWN_KEYS` 键集与三处文件面重命名表（`static_analysis` / `write_protection` / `depends_on`），MUST NOT 出现任何按字段的语义分支（值域校验归 `validate`、键集归 `KNOWN_KEYS`、保真度归 `desktop-config-agent`「文件面写入器」的 P1–P4 锚）。故写面不为字段消费点表新增「消费者」，而作为一行**全字段透传**登记在案。
- **字段消费点表**（本期口径，随实现同步）：`$schema` / `schema` → 纯展示（`schema` 恒 `spec-driven`）；`context` / `rules` → ∅ 零消费（**存量欠账**，本变更不改，登记在案）；`static_analysis` → `checks::static_check` 单条命令；`projects[].root` → create / promote bootstrap 项目锚点；`projects[].depends_on` → bootstrap **拓扑序安装**（唯一消费者，见 desktop-change-worktree「确定性依赖 bootstrap」）；`projects[].language` → **声明型**（计划消费者：按语言 prompt 附录 / 按语言检查分派，后续切片）；`tests[]` → testexec / mutation / aggregate；`write_protection.files[]` → agent protect 规则；**写面（`plan_apply_config` / `to_file_value`）→ 全字段透传（八键全写、按模板裁剪，无字段语义分支）**。

#### Scenario: 层级唯一真相源

- **WHEN** 审查架构模块层级判定与 `depends_on` 的全部消费点
- **THEN** 包含 / 父子关系判定只见 FQN 最长前缀与模型声明纪律，`depends_on` 只出现在安装顺序（与将来的影响面传播方向）；无任何「按 `depends_on` 推父子层级」的代码或契约

#### Scenario: 语言归一单点

- **WHEN** 审查 `projects[].language` 的读取点
- **THEN** 协议层（`assemble_project`）无 trim / 大小写 / 别名转换；消费侧语言归一经唯一入口函数，各消费点无第二套映射

#### Scenario: 声明型字段在案

- **WHEN** 审查 `projects[].language` 的工程消费者
- **THEN** 本 spec 显式标注其为声明型字段并列出计划消费者；MUST NOT 声称本期已有运行时效果（本期唯一活消费者是 `depends_on` → 拓扑序安装）

#### Scenario: 消费点表可考

- **WHEN** 将字段消费点表与 `KNOWN_KEYS` 八项键集、`ProjectConfig` 字段集逐项对位
- **THEN** 每个字段都有消费点或「声明型」标注，无未登记字段；`context` / `rules` 的零消费欠账在案；写面作为「全字段透传」一行登记在案

#### Scenario: 写面零字段语义分支

- **WHEN** 审查写面实现（`plan_apply_config` / `to_file_value`）的字段名出现面
- **THEN** 字段名清单只有 `KNOWN_KEYS` 键集与三处重命名表（`static_analysis` / `write_protection` / `depends_on`），零按字段的值域判断、零「某字段特殊处理」分支；字段值域校验只由 `validate` 承担


## Module Contract

### 模块增量：config 会话写面与呈现面改判（config-agent-session）

主基线既有行逐字沿用；本变更受影响模块如下：

| 模块 | 类型 | 职责 / 契约 |
|------|------|-------------|
| `packages/desktop/src-tauri/src/commands/config/mod.rs` | API | `workspace_config_apply(root: String, candidate: serde_json::Value) -> Result<WorkspaceConfigReport, String>`：check root → `plan_apply_config`（零 `InvalidValue` 门）→ `write_atomic` → 重读返回；入参为 file-face `Value`（出线 `unknown`）；不落库、无缓存；写面契约细则归 `desktop-config-agent` |
| `packages/desktop/src-tauri/crates/core/config/src/lib.rs` | Function | `validate` / `to_file_value` / `plan_apply_config` / `known_keys` / `TestFramework::ALL` / `as_str`（纯函数、零 IO；`config.json` 字面量禁令照旧，路径经 `foundation::layout::config_path`） |
| `packages/desktop/src-tauri/src/commands/mod.rs` | API（登记） | `all_commands!` 增 `workspace_config_apply`（`commands/config` 轨道）与 `list_config_chat_sessions`（`commands/exec` 轨道） |
| `packages/desktop/src/views/config/config-view.tsx` | Component | 单列 → 横向分栏：左栏常驻 config 会话面板（`desktop-config-agent`），右栏既有分区 + `config-refresh`；应用成功经 `useWorkspaceConfig().refresh()` 重取 |
| `packages/desktop/src/types/generated/bindings.ts` / `bindings.zod.ts` | API（生成物） | 经 `bindings:export` 再生成：`workspaceConfigApply`（`candidate` 出线 `unknown`）、`listConfigChatSessions`；MUST NOT 手改 |
