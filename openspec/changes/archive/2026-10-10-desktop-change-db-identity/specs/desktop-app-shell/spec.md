# desktop-app-shell Specification (Delta)

## ADDED Requirements

### Requirement: change 域命令面 id 寻址

desktop-app 全部 change 寻址命令 SHALL 以 change id 作定位参数（参数名 `id`，String）：change 域命令组（`get_change_detail` / `read_artifact` / `archive_change`）与 change flow / archive flow 两命令组（`change_flow_start` / `change_flow_stop` / `change_flow_answer` / `change_flow_confirm` / `change_flow_watch` / `archive_flow_preflight` / `archive_flow_start` / `archive_flow_stop` / `archive_flow_state` / `archive_flow_watch`）MUST NOT 再以 name 定位（name 为可变属性，见 desktop-change-state-store「change 身份锚与 name 属性分离」）。命令 SHALL 以 id 读取记录后供 core 与磁盘 / git 面消费（worktree 路径 / name 分辨率单点收口）；id 不可达（无记录）SHALL 走既有口径：读命令空结果语义、写命令显式 `Err`（组内 blank root 双口径不变）。`list_changes` 返回条目恒携 `id`；`create_change` 返回 `id`（见 desktop-change-create）。薄包装（三件事纪律）与无状态语义 MUST NOT 改变。

#### Scenario: change 域命令 id 寻址

- **WHEN** 前端以 id 分别 invoke `get_change_detail` / `read_artifact` / `archive_change`
- **THEN** 命令以 id 读取记录（worktree / name → 磁盘定位，归档改名与产物读取照常），行为与既有语义一致；name / 前缀目录名不再构成可达寻址

#### Scenario: 无建档 id 写命令显式拒绝

- **WHEN** 以不可达 id invoke `archive_change` 或两 flow 组的写侧命令（start / stop / answer / confirm）
- **THEN** 返回显式 `Err`（无 ChangeRecord / 未建档原因），零落账零磁盘变化

#### Scenario: bindings 再生成跟随

- **WHEN** 命令面与 DTO 演进后执行 bindings 重导出
- **THEN** 全部 change 寻址命令签名定位参数为 `id`，`ChangeSummary` / `ChangeDetail` / `CreateOutcome` 携 `id`；一致性守卫（git diff --exit-code）绿

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `dev-team::commands::changes` | change 域命令组 id 寻址 | `get_change_detail` / `read_artifact` / `archive_change` 参数 `id`；`create_change` 返回 `CreateOutcome.id`；读空结果 / 写显式 Err 双口径不变 |
| `dev-team::commands::change_flow` + `commands::archive_flow` | 两 flow 命令组 id 寻址 | 全域命令定位参数 `id`；装配（exec root 解析 / 互斥校验 / 前置 gate）以 id 读取记录；`Result<T, String>` 模板不变 |
| 前端页面域 hooks（`views/changes/hooks/`） | 取数契约 id 化 | `useChangeDetail(root, id)` / `useChangeFlowRun({ root, id })` / `useArchiveFlow({ root, id })` 等；显式刷新取数模型与 hooks 收口不变 |
| `packages/desktop/src/app.tsx` + `routes.tsx` | 路由表随动 | `/changes/:id`（原 `:name`）；`ChangeListView` / `ChangeDetailView` 路由挂载与选中重置语义不变 |
