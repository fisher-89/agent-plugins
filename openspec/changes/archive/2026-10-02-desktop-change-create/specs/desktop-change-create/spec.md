# desktop-change-create Specification

## ADDED Requirements

### Requirement: 写面 create 操作进程内创建 change

`core/workflow` 写面 SHALL 新增 `create` 操作：给定 `Layout`、change 名称与 goal 文本，创建 `changes_root/<name>/` 目录（含父树，`create_dir_all` 语义）并写出两个文件——`workflow.json` 与 `explore.md`。该操作 SHALL 为进程内同步调用（MUST NOT 引入 tokio / async runtime，沿写面 sync 纪律），SHALL 仅依赖 `foundation`（零 Tauri、零 store），SHALL 经 `Layout` 取全部磁盘路径（MUST NOT 自拼 `openspec` 目录名字面量）。目标 active 目录已存在时 SHALL 显式失败且 MUST NOT 改动既有目录内任何文件。既有写面四操作（`phase_next` / `phase_start` / `phase_log` / `backtrack`）语义 MUST NOT 因新增操作改变；写触点收口为 change 域目录内上述两文件，MUST NOT 泛化为任意 workspace 写通道。

#### Scenario: 空白树上创建成功且文件形状正确

- **WHEN** 对无 `changes` 目录树的 workspace 以合法名称与 goal 调用写面 `create`
- **THEN** `changes_root/<name>/` 目录被创建（含父树）
- **AND** `workflow.json` 内容恰为 `{"workflow_type":"requirement","created":"<UTC 日期 YYYY-MM-DD>","file_log":[]}`（2 空格 pretty + 尾换行，键序 workflow_type → created → file_log）
- **AND** `workflow.json` 不含 `eval` 键，`eval.json` 与 `.openspec.yaml` 不存在
- **AND** `explore.md` 存在且内容为 goal 原文（UTF-8，无附加结构包装）

#### Scenario: 已存在同名 active change 被拒绝且零副作用

- **WHEN** `changes_root/<name>/` 已存在时调用写面 `create`
- **THEN** 返回显式错误（错误信息含该目录路径）
- **AND** 既有目录内 workflow.json / explore.md 及其他文件内容与结构零变化

#### Scenario: goal 空白被拒绝

- **WHEN** 以空白（空串或全空白字符）goal 调用写面 `create`
- **THEN** 返回显式错误，目标目录与任何文件均不产生

#### Scenario: sync 与零 Tauri 纪律保持

- **WHEN** 审查写面 `create` 的依赖与签名
- **THEN** 无 tokio / async runtime 依赖、函数为同步签名、crate 依赖仅 foundation 与 serde 系、无 Tauri 相关依赖

### Requirement: 新建 workflow.json 形状与既有消费面兼容

写面 `create` 产出的 `workflow.json` SHALL 与插件 `createChange`（`plugins/dev-team/bin/src/commands/change-create.ts`）输出逐字段一致：键序 `workflow_type` → `created` → `file_log`，`created` 取 UTC 当前日期 `YYYY-MM-DD`，`file_log` 恒为空数组。该产出 SHALL 被既有消费面无缝识别：parse 层代际检测判为 v2、`list_changes` / `change_detail` 正常呈现、`change_flow_start` 三项前置校验（存在、可解析、requirement 相位表在位）通过、serde 写出形状可被插件 zod `workflowFileSchema` 解析。`workflow_type` SHALL 恒为 `"requirement"`（V1 唯一支持的工作流类型，与发起前置校验同口径），MUST NOT 作为入参暴露枚举选择。

#### Scenario: 新建 change 立即可见可发起

- **WHEN** 写面 `create` 成功后立即调用 `list_changes` / `change_detail` / `change_flow_start` 前置校验
- **THEN** 该 change 以 v2 代际出现在清单、详情可达、发起校验三项全通过（无 workflow.json 无法解析或 workflow_type 不支持类错误）

#### Scenario: 与插件 zod schema 兼容

- **WHEN** 以插件 `workflowFileSchema` 解析写面 `create` 产出的 workflow.json
- **THEN** 解析通过且语义等价（workflow_type / created / file_log 三字段齐备，未知键为零）

#### Scenario: created 取 UTC 日期

- **WHEN** 跨 UTC 日界时刻调用写面 `create`
- **THEN** `created` 值为当时的 UTC 日历日期（与插件 `toISOString().slice(0, 10)` 同式）

### Requirement: 名称与入参校验

写面 `create` SHALL 对名称做权威校验：匹配 kebab-case 正则 `^[a-z][a-z0-9]*(-[a-z0-9]+)*$`、长度 ≤ 128 字符；任一不满足 SHALL 显式失败且不产生任何目录或文件。前端 SHALL 做同口径本地校验：名称不合法或 goal 为空白时 SHALL 禁用提交且不发起 invoke。错误信息 SHALL 抵达调用方（含失败原因），MUST NOT 静默降级。

#### Scenario: 非法名称被写面拒绝

- **WHEN** 分别以 `"My Feature"`（大写与空格）、`"my_new_feature"`（下划线）、`"a".repeat(129)`（超长）调用写面 `create`
- **THEN** 每次均返回显式错误（kebab-case 或长度原因），目标目录与文件零产生

#### Scenario: 前端本地校验拦截

- **WHEN** 用户在新建对话框输入不合法名称（或清空 goal）后尝试提交
- **THEN** 提交按钮不可用，未发起任何 invoke 调用；名称合法且 goal 非空白后按钮可用

### Requirement: create_change IPC 命令面

desktop-app 命令层 SHALL 新增 `create_change` 命令（落位 `commands/changes/` 新命令组）：三件事纪律薄包装（参数转换 → 调写面 `create` → 错误映射），blank root 显式 `Err`，返回 `Result<T, String>`，返回 DTO 仅含 `name` 与 `created`（磁盘路径知识 MUST NOT 下沉前端）。命令 SHALL 经 `#[specta::specta]` 出线并登记入 `all_commands!`，TS bindings 随既有管线重导出且一致性守卫通过。

#### Scenario: 命令注册与类型出线

- **WHEN** 审查 specta builder 组装与生成 bindings
- **THEN** `create_change` 在 `all_commands!` 清单与 bindings 中均有对应 typed 包装，参数与返回类型受编译期校验

#### Scenario: blank root 显式失败

- **WHEN** 以空白 root 调用 `create_change`
- **THEN** 返回 `Err`（写无空结果语义），不进入写面链路

#### Scenario: 返回 DTO 不携带路径

- **WHEN** 审查 `create_change` 返回类型的字段集
- **THEN** 仅含 `name` 与 `created`，无任何绝对 / 相对磁盘路径字段

### Requirement: goal 经 explore.md 进入工作流

写面 `create` SHALL 将 goal 原文写入 `changes_root/<name>/explore.md`（UTF-8，free-form，MUST NOT 添加章节等结构假设），作为该变更的最初目标载体。goal 进入工作流 SHALL 仅依赖既有机制：proposal 相位 executor prompt 的 explore 交接行（相位表既有文本）指引 proposal-planner 将 explore.md 作为 free-form explore context 读取——相位表与 prompt 模板 MUST NOT 为 goal 新增占位符或改写，workflow.json MUST NOT 为 goal 新增字段。explore.md SHALL 经 markdown-doc 产物插件收录，在 change 详情产物清单中以「探索」条目可见。

#### Scenario: proposal 相位既有通道消费 goal

- **WHEN** 审查相位表 proposal 相位 executor prompt 与新建 change 的 explore.md
- **THEN** prompt 含既有交接行（指引读取 `changes/<change>/explore.md`，文本零改动），explore.md 内容即 goal 原文，goal 无需任何模板或 schema 变更即达 proposal-planner

#### Scenario: goal 在详情页可见

- **WHEN** 新建 change 后打开其详情页产物清单
- **THEN** explore.md 以「探索」条目在列（markdown-doc 插件既有收录规则），可经信封读取渲染

### Requirement: 清单页新建入口

变更清单页 SHALL 提供新建变更入口：toggle 展开新建对话框（沿 explore-create-dialog 先例），对话框含名称输入（kebab-case 提示）与 goal 多行输入，均必填；提交调用 `create_change`，成功后回调父层刷新清单并导航 `/changes/<name>`；后端错误 SHALL 行内呈现（break-all 错误块）。测试挂钩 SHALL 使用 data-testid（沿用既有命名风格），MUST NOT 以样式类名作查询挂钩。

#### Scenario: 创建成功流转

- **WHEN** 用户在对话框输入合法名称与 goal 并提交成功
- **THEN** 清单刷新且新 change 出现在进行中分组，URL 导航至 `/changes/<name>` 进入详情

#### Scenario: 错误行内呈现

- **WHEN** 提交因名称冲突等原因失败
- **THEN** 对话框行内呈现后端错误信息，清单与路由不变化，用户可修改后重试

### Requirement: V1 范围与边界留痕

以下边界 SHALL 作为 V1 显式限制留痕（后续变更偿还，不由实现隐式吸收）：

1. **工作流类型恒 requirement**：不入参暴露 workflow_type 枚举；bug-fix / test-only 等类型随其相位表变更一起开放；
2. **名称用户输入**：不由 agent 从 goal 派生名称（异步、成本高）；agent 派生留作后续增强；
3. **仅拒 active 冲突**：与插件 `createChange` parity，归档树同名不拒绝（清单以代际徽标区分）；
4. **创建后不自动发起 run**：run 由用户在详情 / flow 视图显式触发；
5. **goal 无桌面编辑入口**：详情只读渲染；goal 修正走既有文件手改或后续变更；
6. **插件侧零改动**：`plugins/dev-team` 全部冻结（`change-create.ts`、MCP 工具面、版本 2.10.44、三类交付产物）。

#### Scenario: 边界留痕可考

- **WHEN** 查阅本 spec
- **THEN** 六条边界均可考，后续变更无需重新论证是否知情

### Requirement: 版本交付

本变更 SHALL 将 `packages/desktop/package.json` 的 `version` 由 `0.4.0` 升级为 `0.4.1`（用户可见新功能；`src-tauri/tauri.conf.json` 经 `../package.json` 自动跟随，`src-tauri/Cargo.toml` 版本不随动）。本变更 SHALL NOT 变更 `plugins/dev-team`。

#### Scenario: 版本号升级

- **WHEN** 本变更实现完成
- **THEN** `packages/desktop/package.json` version 为 0.4.1，`plugins/dev-team/package.json` version 仍为 2.10.44

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src-tauri/crates/core/workflow/src/write/create.rs`（新） | change 创建域操作 | kebab-case（`^[a-z][a-z0-9]*(-[a-z0-9]+)*$`、≤128）+ goal 非空白校验；`create_dir_all` 建树；workflow.json `{workflow_type:"requirement", created:<UTC YYYY-MM-DD>, file_log:[]}`（键序固定、无 eval 键、2 空格 pretty + 尾换行）+ explore.md goal 原文；已存在拒绝零副作用；sync、零 Tauri、仅依赖 foundation |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/mod.rs` | 写面导出面 | 追加导出 `create`（入参 `&Layout` / name / goal；签名与返回 DTO 形状 design 定稿）；既有四操作导出不变 |
| `packages/desktop/src-tauri/src/commands/changes/mod.rs`（新组） | `create_change` IPC 命令 | 三件事薄包装；blank root → `Err`；`Result<T, String>`；返回 DTO 仅 `name` + `created`（路径不下沉）；`#[specta::specta]` |
| `packages/desktop/src-tauri/src/commands/mod.rs` | 单一登记面 | `all_commands!` 追加 `create_change` |
| `packages/desktop/src/types/generated/bindings.ts`（重导出） | 前端唯一 IPC 类型面 | `createChange` typed 包装 + 返回 DTO 出线；check/build 前置重导出 + diff 守卫既有管线 |
| `packages/desktop/src/views/changes/components/change-create-dialog.test.tsx` 同目录 `change-create-dialog.tsx`（新） | 新建对话框 | toggle 展开；名称（kebab-case 本地校验）+ goal 必填；不合法禁提交不发起 invoke；错误行内呈现；data-testid 挂钩 |
| `packages/desktop/src/views/changes/change-list-view.tsx` | 入口挂载与流转 | 挂新建对话框；成功回调 refresh 清单 + `navigate('/changes/<name>')` |
| `packages/desktop/package.json` | 版本交付 | 0.4.0 → 0.4.1；`plugins/dev-team` 不动 |
