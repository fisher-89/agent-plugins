# desktop-explore-queries Specification

## Purpose

定义 workflow crate 的 explore 读面查询契约：`read_explore` 单文件读取（防穿越）、导入扫描，以及纯读纪律与 explore 薄命令面。

## Requirements

### Requirement: read_explore 单文件读取

workflow crate 的 queries SHALL 新增 `read_explore(layout, name) -> Option<ExploreDoc>`：以 `Layout.explores_root`（既有字段，零新增布局解析）定位 `explores_root/<name>.md`，返回 UTF-8 文本内容与名称。`name` SHALL 为单个普通文件名 stem——校验口径与 `locate_change` 的 `is_change_name` 一致（非空、非 `.` / `..`、不含 `/` `\` `:`），拒绝路径穿越；文件不存在或不可读为 UTF-8 时 SHALL 返回 `None` 而非报错。explores 是扁平 markdown 文件集合（无 active / archive 两棵树），MUST NOT 引入树定位语义。

#### Scenario: 读取既有笔记

- **WHEN** 对含 `openspec/explores/api-design.md` 的 workspace 调用 `read_explore(layout, "api-design")`
- **THEN** 返回该文件 UTF-8 文本，内容与磁盘一致

#### Scenario: 未知与穿越名称

- **WHEN** 分别以不存在的 stem、`../changes`、`a/b` 调用 `read_explore`
- **THEN** 均返回 `None`（穿越名在校验层即被拒），不触碰 `explores_root` 之外的任何路径

#### Scenario: 缺失目录降级

- **WHEN** workspace 无 `openspec/explores/` 目录
- **THEN** `read_explore` 返回 `None`，不报错

### Requirement: 导入扫描

queries SHALL 新增目录扫描（`scan_explores(layout)` 或等价 API）：列出 `explores_root` 下全部顶层 `*.md` 文件，每条至少含 stem 名称（修改时间可得时一并提供）；目录缺失 SHALL 返回空结果。扫描 SHALL 只列目录、MUST NOT 过滤「已绑定」——绑定状态属 store 域（workflow MUST NOT 认识 store），「未绑定」过滤 SHALL 在命令层以 `ExploreRecord` 清单求差完成。

#### Scenario: 全量列出与空目录降级

- **WHEN** explores 目录含 `a.md`、`b.md`、`c.txt`（或目录不存在）
- **THEN** 返回 `a`、`b` 两个 stem（非 md 忽略）；目录缺失时返回空列表，不报错

#### Scenario: 未绑定过滤在命令层

- **WHEN** 审查 `scan_explores` 与 explore 扫描命令的实现
- **THEN** workflow 扫描不含绑定过滤逻辑；命令层以 store 清单滤除已绑定 stem 后返回

### Requirement: 查询层纪律与命令面

查询层纪律 SHALL 不变：explore 读面为纯读操作（MUST NOT 写入、移动或修改任何文件——explore.md 由 agent 会话流程创建，应用无落盘路径）；域 crate（workflow）MUST NOT 依赖 Tauri。命令层 SHALL 开通 explore 薄命令（新 `commands/explores/` 轨道或 queries 轨道扩展，dev-design 定）：`read_explore` / 导入扫描 / explore 记录 CRUD——无状态、参数转换 → 调用 → DTO，blank root 返回空结果（照 `commands/queries/mod.rs` 既有纪律）；`read_explore` 返回纯 markdown 文本 DTO，MUST NOT 套用 change 域的 `ArtifactEnvelope` 信封。

#### Scenario: 只读保证

- **WHEN** 连续调用 `read_explore` 与导入扫描
- **THEN** workspace 目录树的文件内容与结构无任何变化

#### Scenario: 命令薄包装与空 root

- **WHEN** 前端以空/空白 `root` invoke `read_explore` 或扫描命令
- **THEN** 命令不进入查询链路直接返回空结果语义，无 panic、无错误弹窗

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/src/queries/explore.rs`（新） | explore 读面 | `read_explore(layout, name)`（stem 校验防穿越，缺失 → None）；`scan_explores(layout)`（顶层 `*.md` stem，缺失目录空）；纯读、零 Tauri |
| `crates/core/workflow/src/queries/mod.rs` | 模块注册与导出 | explore 读面挂入 queries，既有 change 查询不动 |
| `src-tauri/src/commands/explores/`（新轨道） | 薄命令 | `read_explore` / 扫描 / 记录 CRUD；无状态三件事；blank root 空结果；纯文本 DTO 不套信封 |
