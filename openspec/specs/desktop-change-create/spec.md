# desktop-change-create Specification

## Purpose

desktop 桌面端补充"新建变更"入口：用户在变更清单页输入 change 名称与最初目标（goal），经 core/workflow 写面 `create` 操作在进程内创建 change 目录（explore.md）并在 workspace 库建档（`ChangeRecord`）；goal 原文写入 explore.md，经既有 proposal 相位交接通道进入工作流。MUST NOT 产出 workflow.json（双向墙），插件面零改动，禁止 CLI 子进程。

## Requirements

### Requirement: 写面 create 操作进程内创建 change

`core/workflow` 写面 `create` 操作 SHALL 创建 change 域并在同一次创建语义内分配 worktree（生命周期与落位细则见 desktop-change-worktree）：① 在 workspace 库建档（`ChangeRecord`：name / workflow_type=requirement / created_at / status=active / worktree=本次分配的绝对路径 / base_commit=创建基线）；② 以主仓 HEAD 为基线 `git worktree add` 落位身份派生目录并建 branch `change/<name>`；③ 在 worktree 内创建 `openspec/changes/<name>/` 目录树（含父树，`create_dir_all` 语义）并写出 `explore.md`（goal 原文）；④ 执行确定性 bootstrap 与脏仓探测、收集警告。四段 SHALL 构成一次创建语义（分段顺序与失败补偿边界由 design 定稿；MUST NOT 出现「目录在而记录缺」的可用性破口：建档先行的补偿语义保持，worktree 段失败的回收边界由 design 定稿，bootstrap 段失败 MUST NOT 回滚）。MUST NOT 产出 workflow.json。该操作 SHALL 为进程内同步调用（MUST NOT 引入 tokio / async runtime，沿写面 sync 纪律；落库与 worktree / bootstrap 执行均经 port 缝由壳层装配实现，core/workflow 零 infra 依赖、零进程 spawn），SHALL 经 `Layout` 取全部磁盘路径（MUST NOT 自拼 `openspec` 目录名字面量）。前置校验 SHALL 在任何 IO 之前完成且拒绝面零副作用：名称 kebab-case + 长度、goal 非空白、冲突检查（主仓 active 目录已存在或 db 已有同名 active 记录）、git 可用性（git 不可发现或主仓非 git 仓）、branch `change/<name>` 已存在、worktree 目标目录已存在。既有写面操作（`phase_next` / `phase_start` / `phase_log` / `backtrack` / `archive`）语义 MUST NOT 因新增语义改变；写触点收口为 change 域目录内产物文件、workspace 库 change 状态与 worktree 建域三面，MUST NOT 泛化为任意 workspace 写通道。

#### Scenario: 空白树上创建成功并分配 worktree

- **WHEN** 对无 `changes` 目录树的 git 仓 workspace 以合法名称与 goal 调用写面 `create`
- **THEN** worktree 落位于 `worktrees/{身份段}/<name>` 且基线为主仓 HEAD，worktree 内 `changes_root/<name>/explore.md` 存在且内容为 goal 原文（UTF-8，无附加结构包装），db 出现 `ChangeRecord`（status=active、created_at、worktree、base_commit 在案）
- **AND** worktree 目录内无 workflow.json，`eval.json` 与 `.openspec.yaml` 不存在；主仓 active 树不含该目录

#### Scenario: 新建 change 立即可见可发起

- **WHEN** 写面 `create` 成功后立即调用列表 / 详情查询与 `change_flow_start` 前置校验
- **THEN** 该 change 以 db 建档条目出现在清单（active 组）、详情可达（工件经 worktree 路径解析）、发起校验通过（已建档且 requirement 相位表在位）

#### Scenario: 已存在同名 active change 被拒绝且零副作用

- **WHEN** 主仓 `changes_root/<name>/` 已存在（或 db 已有同名 active `ChangeRecord`）时调用写面 `create`
- **THEN** 返回显式错误（错误信息含该目录路径或记录名）
- **AND** 既有目录内文件内容与结构零变化、既有 db 记录零改写、无新记录产生、零 worktree / branch 产生

#### Scenario: git 与 worktree 冲突面被拒绝

- **WHEN** 分别以 git 不可发现、主仓非 git 仓、branch `change/<name>` 已存在、worktree 目标目录已存在四种情形调用写面 `create`
- **THEN** 每次均返回显式 `Err`（含引导或冲突对象），零建档、零目录、零 worktree / branch

#### Scenario: goal 空白被拒绝

- **WHEN** 以空白（空串或全空白字符）goal 调用写面 `create`
- **THEN** 返回显式错误，目标目录与 db 记录均不产生

#### Scenario: sync 与 port 缝纪律保持

- **WHEN** 审查写面 `create` 的依赖与签名
- **THEN** 无 tokio / async runtime 依赖、函数为同步签名、crate 依赖零 infra/store 零 infra/vcs（落库与 worktree / bootstrap 执行均经 port 缝）、无 Tauri 相关依赖

### Requirement: 名称与入参校验

写面 `create` SHALL 对名称做权威校验：匹配 kebab-case 正则 `^[a-z][a-z0-9]*(-[a-z0-9]+)*$`、长度 ≤ 128 字符；任一不满足 SHALL 显式失败且不产生任何目录或文件。前端 SHALL 做同口径本地校验：名称不合法或 goal 为空白时 SHALL 禁用提交且不发起 invoke。错误信息 SHALL 抵达调用方（含失败原因），MUST NOT 静默降级。

#### Scenario: 非法名称被写面拒绝

- **WHEN** 分别以 `"My Feature"`（大写与空格）、`"my_new_feature"`（下划线）、`"a".repeat(129)`（超长）调用写面 `create`
- **THEN** 每次均返回显式错误（kebab-case 或长度原因），目标目录与文件零产生

#### Scenario: 前端本地校验拦截

- **WHEN** 用户在新建对话框输入不合法名称（或清空 goal）后尝试提交
- **THEN** 提交按钮不可用，未发起任何 invoke 调用；名称合法且 goal 非空白后按钮可用

### Requirement: create_change IPC 命令面

desktop-app 命令层 SHALL 提供 `create_change` 命令（落位 `commands/changes/` 命令组；该组同组承载 change 域读命令 `list_changes` / `get_change_detail` / `read_artifact`，见 desktop-app-shell「Tauri command 轨道组织」）：三件事纪律薄包装（参数转换 → 调写面 `create` → 错误映射），blank root 显式 `Err`，返回 `Result<T, String>`。返回 DTO SHALL 为 `CreateOutcome`：`name`、`created`、`worktree`（本次分配的 worktree 绝对路径）、`warnings`（警告清单：脏仓引导 / bootstrap 结果 / 未知管理器注记等，干净且顺利时为空）——主仓 openspec 目录树路径知识仍 MUST NOT 下沉前端；worktree 绝对路径为**刻意出线的执行锚**（用户 review / 手动 commit / merge 可达性拍板，见 desktop-change-worktree「worktree 路径 UI 可见性」）。命令 SHALL 经 `#[specta::specta]` 出线并登记入 `all_commands!`，TS bindings 随既有管线重导出且一致性守卫通过；`CreateOutcome` 字段演进 SHALL 走 golden 显式重写流程（wire contract 冻结约束）。

#### Scenario: 命令注册与类型出线

- **WHEN** 审查 specta builder 组装与生成 bindings
- **THEN** `create_change` 在 `all_commands!` 清单与 bindings 中均有对应 typed 包装，参数与返回类型受编译期校验

#### Scenario: blank root 显式失败

- **WHEN** 以空白 root 调用 `create_change`
- **THEN** 返回 `Err`（写无空结果语义），不进入写面链路

#### Scenario: 返回 DTO 字段集

- **WHEN** 审查 `create_change` 返回类型的字段集
- **THEN** 恰为 `name` / `created` / `worktree` / `warnings` 四字段，无主仓 openspec 目录树路径字段

#### Scenario: 警告抵达前端

- **WHEN** 以脏主仓（或 bootstrap 失败夹具）调用 `create_change` 成功
- **THEN** 返回 DTO 的 `warnings` 含对应警告文案，前端对话框可行内呈现

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
| `packages/desktop/src-tauri/crates/core/workflow/src/write/create.rs` | change 创建域操作 | kebab-case（`^[a-z][a-z0-9]*(-[a-z0-9]+)*$`、≤128）+ goal 非空白校验；目录 + explore.md（goal 原文）+ db 建档三合一；无 workflow.json 产出；同名 active 冲突拒绝零副作用；sync、零 Tauri、落库经 port 缝 |
| `packages/desktop/src-tauri/crates/infra/store/src/store.rs`（change 域操作面） | 建档半边 | `ChangeRecord` 写入（name 唯一、status=active）；与目录创建的双写顺序 / 失败补偿 design 定稿 |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/mod.rs` | 写面导出面 | 追加导出 `create`（入参 `&Layout` / name / goal；签名与返回 DTO 形状 design 定稿）；既有四操作导出不变 |
| `packages/desktop/src-tauri/src/commands/changes/mod.rs`（不改面） | `create_change` IPC 命令 | 三件事薄包装；blank root → `Err`；返回 DTO 仅 `name` + `created`（created 改取 db created_at，出线 ISO 口径不变）；`#[specta::specta]` |
| `packages/desktop/src-tauri/src/commands/mod.rs` | 单一登记面 | `all_commands!` 追加 `create_change` |
| `packages/desktop/src/types/generated/bindings.ts`（重导出） | 前端唯一 IPC 类型面 | `createChange` typed 包装 + 返回 DTO 出线；check/build 前置重导出 + diff 守卫既有管线 |
| `packages/desktop/src/views/changes/components/change-create-dialog.test.tsx` 同目录 `change-create-dialog.tsx`（不改） | 新建对话框 | 名称（kebab-case 本地校验）+ goal 必填；错误行内呈现；成功回调刷新 + 导航 |
| `packages/desktop/src/views/changes/change-list-view.tsx` | 入口挂载与流转 | 挂新建对话框；成功回调 refresh 清单 + `navigate('/changes/<name>')` |
| `packages/desktop/package.json` | 版本交付 | 0.4.0 → 0.4.1；`plugins/dev-team` 不动 |
| `crates/core/workflow/src/write/create.rs` 内 workflow.json 初始文档写出段 | **（退役）** | 初始 workflow.json 写出职责随载体退役删除（`persist.rs` raw Value 保形改写退役见 desktop-workflow-write-face） |
