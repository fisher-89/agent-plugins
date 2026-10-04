# move-queries-command-to-change — desktop-change-create 变更集

> `commands/changes/` 组的组成发生变化：自 move-queries-command-to-change 起，该组同组承载 change 域读命令（`list_changes` / `get_change_detail` / `read_artifact`，读 + 记录面同组沿 explores 先例），不再是 create 单命令组。`create_change` 自身的命令面语义（三件事薄包装 / blank root / DTO 两字段 / specta 出线）零变化。Module Contract 的 `commands/changes/mod.rs` 行随归档同步（新组 → change 域读 + 记录面命令组）。

## MODIFIED Requirements

### Requirement: create_change IPC 命令面

desktop-app 命令层 SHALL 新增 `create_change` 命令（落位 `commands/changes/` 命令组；该组自 move-queries-command-to-change 起同组承载 change 域读命令 `list_changes` / `get_change_detail` / `read_artifact`——读 + 记录面同组沿 explores 组先例，组内 blank root 双口径并存且不互换，见 desktop-app-shell「Tauri command 轨道组织」）：三件事纪律薄包装（参数转换 → 调写面 `create` → 错误映射），blank root 显式 `Err`，返回 `Result<T, String>`，返回 DTO 仅含 `name` 与 `created`（磁盘路径知识 MUST NOT 下沉前端）。命令 SHALL 经 `#[specta::specta]` 出线并登记入 `all_commands!`，TS bindings 随既有管线重导出且一致性守卫通过。

#### Scenario: 命令注册与类型出线

- **WHEN** 审查 specta builder 组装与生成 bindings
- **THEN** `create_change` 在 `all_commands!` 清单与 bindings 中均有对应 typed 包装，参数与返回类型受编译期校验

#### Scenario: blank root 显式失败

- **WHEN** 以空白 root 调用 `create_change`
- **THEN** 返回 `Err`（写无空结果语义），不进入写面链路

#### Scenario: 返回 DTO 不携带路径

- **WHEN** 审查 `create_change` 返回类型的字段集
- **THEN** 仅含 `name` 与 `created`，无任何绝对 / 相对磁盘路径字段
