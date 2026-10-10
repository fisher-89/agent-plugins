# desktop-explore-queries Specification (Delta)

## MODIFIED Requirements

### Requirement: 导入扫描

queries SHALL 新增目录扫描（`scan_explores(layout)` 或等价 API）：列出 `explores_root` 下全部顶层 `*.md` 文件，每条至少含 stem 名称（修改时间可得时一并提供）；目录缺失 SHALL 返回空结果。扫描 SHALL 只列目录、MUST NOT 过滤「已绑定」——绑定状态属 store 域（workflow MUST NOT 认识 store），「未绑定」过滤 SHALL 在命令层以 `ExploreRecord` 清单求差完成。命令层 SHALL 在未绑定过滤之外再过滤 **非 kebab-case stem**（不列入可绑定清单，避免「点击后报错」；workflow 扫描口径保持宽松、旧非 kebab 文件仍可经 `read_explore` 读取）。

#### Scenario: 全量列出与空目录降级

- **WHEN** explores 目录含 `a.md`、`b.md`、`c.txt`（或目录不存在）
- **THEN** 返回 `a`、`b` 两个 stem（非 md 忽略）；目录缺失时返回空列表，不报错

#### Scenario: 未绑定过滤在命令层

- **WHEN** 审查 `scan_explores` 与 explore 扫描命令的实现
- **THEN** workflow 扫描不含绑定过滤逻辑；命令层以 store 清单滤除已绑定 stem 后返回

#### Scenario: 非 kebab stem 在命令层过滤

- **WHEN** explores 目录含 `MyNote.md`（非 kebab，未绑定）与 `api-retry.md`（kebab，未绑定）
- **THEN** 命令层 `scan_explores` 仅返回 `api-retry`；workflow `scan_explores` 仍返回两者（口径宽松不变）

### Requirement: 查询层纪律与命令面

查询层纪律 SHALL 不变：explore 读面为纯读操作（MUST NOT 写入、移动或修改任何文件——explore.md 由 agent 会话流程创建，应用无落盘路径）；域 crate（workflow）MUST NOT 依赖 Tauri。命令层 SHALL 开通 explore 薄命令（`commands/explores/` 轨道）：`read_explore` / 导入扫描 / explore 记录 CRUD / `update_explore_title` / `promote_explore`——无状态、参数转换 → 调用 → DTO，blank root 返回空结果（读命令）或显式 `Err`（写命令，照 `commands/queries/mod.rs` 既有纪律）；`read_explore` 返回纯 markdown 文本 DTO，MUST NOT 套用 change 域的 `ArtifactEnvelope` 信封。`promote_explore` 为跨域写命令（见「promote_explore 命令与笔记搬运」），MUST NOT 在命令层直接执行磁盘删除——笔记搬运半边 SHALL 经 workflow 写面完成。

#### Scenario: 只读保证

- **WHEN** 连续调用 `read_explore` 与导入扫描
- **THEN** workspace 目录树的文件内容与结构无任何变化

#### Scenario: 命令薄包装与空 root

- **WHEN** 前端以空/空白 `root` invoke `read_explore` 或扫描命令
- **THEN** 命令不进入查询链路直接返回空结果语义，无 panic、无错误弹窗；`promote_explore` / `update_explore_title` 以空白 root invoke 返回显式 `Err`

## ADDED Requirements

### Requirement: update_explore_title 回填命令

命令层 SHALL 提供 `update_explore_title(root, name, title)`（落 `commands/explores/`）：blank root / 空白 title → 显式 `Err`；经 `for_root` 路由所属 workspace 库并调 store `set_explore_title`，返回更新后的 `ExploreRecord`。该命令 SHALL 为显式写命令（title 回填唯一写入口），读取路径 `read_explore` MUST NOT 回填或写入任何状态。

#### Scenario: 回填成功与拒绝面

- **WHEN** 以合法 root / name / 非空白 title 调用 `update_explore_title`
- **THEN** store 记录 `title` 更新且 `updated_at` 刷新，返回记录逐字段一致；空白 title / blank root 返回显式 `Err`，库内零变化

#### Scenario: 读取路径零写入

- **WHEN** 连续调用 `read_explore` 后查询 store 记录
- **THEN** `ExploreRecord.title` / `updated_at` 零变化（title 回填仅经 `update_explore_title`）

### Requirement: promote_explore 命令与笔记搬运

命令层 SHALL 提供 `promote_explore(root, name)`（落 `commands/explores/`）：blank root / 非 kebab name → 显式 `Err`。语义 SHALL 为 **move**：

1. 读主仓笔记全文（`read_explore` 命中且内容非空白，否则显式 `Err` 引导先完成探索）；
2. 读 `ExploreRecord`（存在且 `promoted_to` 为空，否则显式 `Err`）；
3. 调 workflow 写面 `promote_explore`（复用 change `create`：`name = explore.name`、`goal = 笔记全文`、`title = explore.title`，成功后删主仓 `explores/<name>.md`）；
4. 写面成功后调 store `mark_explore_promoted(root, name, change_id)` 打标（会话链保留，MUST NOT 删 explore 记录）。

命令 SHALL 返回 `PromoteOutcome`（`change_id` / `change_name`）。磁盘笔记删除 SHALL 仅经 workflow 写面完成，命令层 MUST NOT 直接 `remove_file`。写面 `promote_explore` SHALL 为 sync、零 Tauri、零 tokio（沿写面纪律），经 `Layout` 取全部磁盘路径（MUST NOT 自拼 `openspec` 字面量）。

#### Scenario: promote move 成功

- **WHEN** 对已落盘且非空白的 kebab explore（`api-retry`，笔记首行 `# API 重试调研`）调用 `promote_explore`
- **THEN** 返回 `PromoteOutcome`（change_id / change_name=api-retry）；worktree 内 `changes/api-retry/explore.md` 内容为笔记全文；主仓 `explores/api-retry.md` 已删；store 记录 `promoted_to = change_id` 且会话链保留

#### Scenario: promote 拒绝面

- **WHEN** 分别以未落盘 / 空白内容 / 非 kebab name / 已 promoted / 同名 active change 冲突调用 `promote_explore`
- **THEN** 均返回显式 `Err`，explore 记录与笔记零改动、change 侧零建档（冲突拒绝发生在 create 前置，create 失败补偿链由既有语义承担）

#### Scenario: 命令层零直接磁盘删除

- **WHEN** 审查 `promote_explore` 命令实现
- **THEN** 命令层无 `std::fs::remove_file` 直接调用；笔记删除经 workflow 写面 `promote_explore` 完成

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/src/queries/explore.rs` | explore 读面 | `read_explore` / `scan_explores` 口径不变（宽松单分量 / 全量 md）；纯读、零 Tauri |
| `crates/core/workflow/src/write/promote.rs`（新） | 笔记搬运（move）半边 | `promote_explore`：复用 `create`（title=explore.title、goal=笔记全文）成功后删 `explores/<name>.md`；sync、零 Tauri、经 `Layout` 取路径 |
| `crates/core/workflow/src/write/mod.rs` | 写面导出 | 追加导出 `promote_explore`；`create` 签名增 title |
| `src-tauri/src/commands/explores/` | 薄命令轨道 | `scan_explores` 命令层 kebab 过滤；新增 `update_explore_title` / `promote_explore`；三件事纪律；blank root 双口径（读空结果 / 写显式 `Err`） |
