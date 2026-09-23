# desktop-workspace-store Specification

## Purpose

定义 workspace 注册表的 redb 本地持久化契约：Store 打开与命令面、canonical path 作 key 的 upsert 语义、记录模型与清单排序、redb 类型边界与单进程约束。

## Requirements

### Requirement: Store 打开与命令面

store crate SHALL 暴露 `Store::open(path: &Path)` 打开（不存在则创建）redb 数据库文件；db 路径 MUST NOT 在 store crate 内解析（home_dir 依赖 Tauri 上下文），SHALL 由 desktop-app 解析后注入。store SHALL 提供三个同步操作（redb `Database` 为 `Send + Sync` 进程内 MVCC 单写多读，同步调用，无需 async）：

- `add_workspace(root) -> WorkspaceRecord`：canonicalize + upsert
- `list_workspaces() -> Vec<WorkspaceRecord>`：默认序（表主键 canonical root 自然序升序）
- `remove_workspace(root)`：按 canonical key 删除

MUST NOT 提供「按打开时间刷新/排序」类操作（`touch_workspace` 已移除：sidebar 清单顺序与使用时间无关，不因打开/切换而重排）。

#### Scenario: tempdir 全链路

- **WHEN** 在临时目录打开 Store 依次执行 add / list / remove，并重开同一 db 文件
- **THEN** 各操作返回预期结果，且重开后记录状态与操作结果一致

#### Scenario: db 路径注入

- **WHEN** 审查 store crate 源码
- **THEN** 无 app data 目录解析逻辑，db 文件路径完全来自 `Store::open` 入参

### Requirement: canonical path 作 key 的 upsert 语义

WORKSPACES 表 SHALL 以 canonical root path 为 key，value SHALL 为 `WorkspaceRecord` 的 JSON 编码。`add_workspace` SHALL 即 upsert：同一目录重复添加 MUST NOT 产生第二条记录，原记录 SHALL 原样返回（`added_at` 保留首添值，MUST NOT 刷新任何时间戳）。目录移动 SHALL 视为删旧加新的两次用户操作，store MUST NOT 追踪路径身份连续性（将来需要时再迁 u64 id 方案）。Windows 路径的 canonicalize 口径（UNC `\\?\` 前缀处理、大小写不敏感归一化）SHALL 固定为单一策略并全链路一致：存库 key、去重比较、remove 命中、前端展示同源。

#### Scenario: 等价路径去重

- **WHEN** 对同一目录先后两次 `add_workspace`（路径存在大小写或尾部分隔符等书写差异）
- **THEN** 清单中仅一条记录，第二次调用原样返回首添记录（`added_at` 保留首添值）

#### Scenario: canonical 口径全链路一致

- **WHEN** `add_workspace` 后再以等价路径执行 `remove_workspace`
- **THEN** 命中同一条记录，不产生孤儿条目

### Requirement: WorkspaceRecord 模型与清单排序

`WorkspaceRecord` SHALL 含 `root`（canonical 完整路径）、`name`（目录名最后一段，展示用）、`added_at` 三字段。`list_workspaces` SHALL 按表主键（canonical root）自然序升序返回（默认序）：顺序与添加/打开时间无关、稳定可复现，清单呈现 MUST NOT 因使用而重排。存量库记录中的历史 `lastOpenedAt` 字段在解码时 SHALL 被忽略（serde 默认忽略未知字段），无需数据迁移。

#### Scenario: 默认序与添加顺序无关

- **WHEN** 乱序 add 多个 workspace（如 gamma → alpha → beta）后调用 `list_workspaces`
- **THEN** 返回顺序恒为 canonical root 字典序升序（alpha、beta、gamma），两次调用顺序一致

#### Scenario: name 取目录名最后一段

- **WHEN** `add_workspace("D:\\work\\my-project")`
- **THEN** 记录 `name` 为 `my-project`，`root` 保留完整路径

### Requirement: redb 类型不泄漏与 schema_version

store 公共 API SHALL 只暴露自有类型（`WorkspaceRecord` 等）；redb 类型（`redb::Database`、`Table` 等）MUST NOT 出现在 store 公共签名、desktop-app 命令签名与前端 DTO。db SHALL 自第一天含 META 表记录 `schema_version`（redb 无内建迁移，为将来演进留位）；业务表命名 SHALL 携带数据维度语义（user 维度），为 workspace 维度表留格。

#### Scenario: 公共 API 无 redb 类型

- **WHEN** 扫描 store 公共 API、desktop-app 命令签名与前端 `types/dto`
- **THEN** 无任何 redb 类型出现，前端 DTO 字段与 `WorkspaceRecord` 一一对应

#### Scenario: 新库写入 schema_version

- **WHEN** `Store::open` 打开一个全新 db 文件
- **THEN** META 表中 `schema_version` 已写入当前版本值

### Requirement: 单进程约束显式化

redb 面向单进程嵌入场景，双开（如 dev 与已装正式版指向同一 db 文件）SHALL 列为已知约束：store MUST NOT 自行实现跨进程锁兜底；进程单实例治理（tauri-plugin-single-instance 等）SHALL 不进本变更范围；该约束 SHALL 以代码注释或 crate 文档显式声明。

#### Scenario: 约束声明可见

- **WHEN** 审查 store crate 文档或代码注释
- **THEN** 单进程模型与双开风险被显式说明，且无自研跨进程锁代码

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/store`（crate 名 `store`） | redb 本地库，db 边界第一成员 | `Store::open(path)` 注入式打开；add/list/remove/touch 四操作；公共 API 仅自有类型；零 Tauri、零 core crate 依赖 |
| `WorkspaceRecord`（DTO） | 清单记录模型 | root / name / added_at / last_opened_at；JSON 编码入 WORKSPACES 表；name 为目录名最后一段 |
| `WORKSPACES` / `META` 表 | user 维度持久化 | WORKSPACES key = canonical path；META 存 schema_version；表命名带维度语义 |
