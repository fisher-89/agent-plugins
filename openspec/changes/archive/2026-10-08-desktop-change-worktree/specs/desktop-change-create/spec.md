# desktop-change-create Specification (Delta)

## MODIFIED Requirements

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
