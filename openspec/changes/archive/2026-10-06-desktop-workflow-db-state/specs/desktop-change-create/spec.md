# desktop-change-create Specification (Delta)

## MODIFIED Requirements

### Requirement: 写面 create 操作进程内创建 change

`core/workflow` 写面 `create` 操作 SHALL 创建 `changes_root/<name>/` 目录（含父树，`create_dir_all` 语义）、写出 `explore.md`（goal 原文）并在 workspace 库建档（`ChangeRecord`：name / workflow_type=requirement / created_at / status=active）——三者为一次创建语义（db 建档与目录创建的双写顺序及失败补偿由 design 定稿；MUST NOT 出现「目录在而记录缺」的可用性破口：任一环节失败 SHALL 收敛为显式错误且可重试）。MUST NOT 产出 workflow.json。该操作 SHALL 为进程内同步调用（MUST NOT 引入 tokio / async runtime，沿写面 sync 纪律；落库经 port 缝由壳层装配 store 实现，core/workflow 零 infra 依赖），SHALL 经 `Layout` 取全部磁盘路径（MUST NOT 自拼 `openspec` 目录名字面量）。目标 active 目录已存在或 db 已有同名 `ChangeRecord`（status=active）时 SHALL 显式失败且 MUST NOT 改动既有目录内任何文件、MUST NOT 改写既有 db 记录。既有写面操作（`phase_next` / `phase_start` / `phase_log` / `backtrack` / `archive`）语义 MUST NOT 因新增语义改变；写触点收口为 change 域目录内产物文件与 workspace 库 change 状态，MUST NOT 泛化为任意 workspace 写通道。

#### Scenario: 空白树上创建成功且建档

- **WHEN** 对无 `changes` 目录树的 workspace 以合法名称与 goal 调用写面 `create`
- **THEN** `changes_root/<name>/` 目录被创建（含父树），`explore.md` 存在且内容为 goal 原文（UTF-8，无附加结构包装），db 出现 `ChangeRecord`（status=active、created_at 在案）
- **AND** 目录内无 workflow.json，`eval.json` 与 `.openspec.yaml` 不存在

#### Scenario: 新建 change 立即可见可发起

- **WHEN** 写面 `create` 成功后立即调用列表 / 详情查询与 `change_flow_start` 前置校验
- **THEN** 该 change 以 db 建档条目出现在清单、详情可达（active 状态面）、发起校验通过（已建档且 requirement 相位表在位）

#### Scenario: 已存在同名 active change 被拒绝且零副作用

- **WHEN** `changes_root/<name>/` 已存在（或 db 已有同名 active `ChangeRecord`）时调用写面 `create`
- **THEN** 返回显式错误（错误信息含该目录路径或记录名）
- **AND** 既有目录内文件内容与结构零变化、既有 db 记录零改写、无新记录产生

#### Scenario: goal 空白被拒绝

- **WHEN** 以空白（空串或全空白字符）goal 调用写面 `create`
- **THEN** 返回显式错误，目标目录与 db 记录均不产生

#### Scenario: sync 与 port 缝纪律保持

- **WHEN** 审查写面 `create` 的依赖与签名
- **THEN** 无 tokio / async runtime 依赖、函数为同步签名、crate 依赖零 infra/store（落库经 port 缝）、无 Tauri 相关依赖

## REMOVED Requirements

### Requirement: 新建 workflow.json 形状与既有消费面兼容

**Reason**: 双向墙裁定下 desktop 不再产出或消费 workflow.json——`create` 的兼容对象（parse 层代际检测、serde 写出可被插件 zod schema 解析、「与插件 `createChange` 逐字段 parity」）随载体退役整体消失；新建 change 的可见性与可发起性由 db 建档 + 查询读面承接（本 delta MODIFIED requirement「写面 create 操作进程内创建 change」的「立即可见可发起」scenario）。

**Migration**: 新建 change 经 `ChangeRecord` 立即出现在清单与详情、`change_flow_start` 前置校验以建档为准；插件侧 `createChange` 零改动照旧服务 CLI 工作流（其 workflow.json 产物 desktop 不再读取）；存量由旧版 desktop 创建的 change（有 workflow.json 无 db 记录）以文档形态展示，不自动补建档。

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src-tauri/crates/core/workflow/src/write/create.rs` | change 创建域操作 | kebab-case（`^[a-z][a-z0-9]*(-[a-z0-9]+)*$`、≤128）+ goal 非空白校验；目录 + explore.md（goal 原文）+ db 建档三合一；无 workflow.json 产出；同名 active 冲突拒绝零副作用；sync、零 Tauri、落库经 port 缝 |
| `packages/desktop/src-tauri/crates/infra/store/src/store.rs`（change 域操作面） | 建档半边 | `ChangeRecord` 写入（name 唯一、status=active）；与目录创建的双写顺序 / 失败补偿 design 定稿 |
| `packages/desktop/src-tauri/src/commands/changes/mod.rs`（不改面） | `create_change` IPC 命令 | 三件事薄包装；blank root → `Err`；返回 DTO 仅 `name` + `created`（created 改取 db created_at，出线 ISO 口径不变）；`#[specta::specta]` |
| `packages/desktop/src/views/changes/components/change-create-dialog.tsx`（不改） | 新建对话框 | 名称（kebab-case 本地校验）+ goal 必填；错误行内呈现；成功回调刷新 + 导航 |
| `crates/core/workflow/src/write/create.rs` 内 workflow.json 初始文档写出段 | **（退役）** | 初始 workflow.json 写出职责随载体退役删除（`persist.rs` raw Value 保形改写退役见 desktop-workflow-write-face delta） |
