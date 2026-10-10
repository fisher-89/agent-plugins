# desktop-change-create Specification (Delta)

## MODIFIED Requirements

### Requirement: 写面 create 操作进程内创建 change

`core/workflow` 写面 `create` 操作 SHALL 创建 change 域并在同一次创建语义内分配 worktree（生命周期与落位细则见 desktop-change-worktree）：① 在 workspace 库建档（`ChangeRecord`：`id`（本次铸出的稳定唯一身份锚，UUID 形态——铸出点由 design 定稿）、`name` / workflow_type=requirement / created_at / status=active / worktree=本次分配的绝对路径 / base_commit=创建基线）；② 以主仓 HEAD 为基线 `git worktree add` 落位身份派生目录（落位与 branch `change/<name>` 仍 name 化）并建 branch `change/<name>`；③ 在 worktree 内创建 `openspec/changes/<name>/` 目录树（含父树，`create_dir_all` 语义）并写出 `explore.md`（goal 原文）；④ 执行确定性 bootstrap 与脏仓探测、收集警告。四段 SHALL 构成一次创建语义（分段顺序与失败补偿边界由 design 定稿；MUST NOT 出现「目录在而记录缺」的可用性破口：建档先行的补偿语义保持，worktree 段失败的回收边界由 design 定稿，bootstrap 段失败 MUST NOT 回滚）。MUST NOT 产出 workflow.json。该操作 SHALL 为进程内同步调用（MUST NOT 引入 tokio / async runtime，沿写面 sync 纪律；落库与 worktree / bootstrap 执行均经 port 缝由壳层装配实现，core/workflow 零 infra 依赖、零进程 spawn），SHALL 经 `Layout` 取全部磁盘路径（MUST NOT 自拼 `openspec` 目录名字面量）。前置校验 SHALL 在任何 IO 之前完成且拒绝面零副作用：名称 kebab-case + 长度、goal 非空白、冲突检查（主仓 active 目录已存在或 db 已有同名 **active** 记录——name 无唯一约束，冲突判定为显式扫描查重，归档同名不拒）、git 可用性（git 不可发现或主仓非 git 仓）、branch `change/<name>` 已存在、worktree 目标目录已存在。既有写面操作（`phase_next` / `phase_start` / `phase_log` / `backtrack` / `archive`）语义 MUST NOT 因新增语义改变（寻址参数随全库换锚改 change id，见 desktop-change-state-store）；写触点收口为 change 域目录内产物文件、workspace 库 change 状态与 worktree 建域三面，MUST NOT 泛化为任意 workspace 写通道。

#### Scenario: 空白树上创建成功并分配 worktree

- **WHEN** 对无 `changes` 目录树的 git 仓 workspace 以合法名称与 goal 调用写面 `create`
- **THEN** worktree 落位于 `worktrees/{身份段}/<name>` 且基线为主仓 HEAD，worktree 内 `changes_root/<name>/explore.md` 存在且内容为 goal 原文（UTF-8，无附加结构包装），db 出现 `ChangeRecord`（id 铸出在案、status=active、created_at、worktree、base_commit）
- **AND** worktree 目录内无 workflow.json，`eval.json` 与 `.openspec.yaml` 不存在；主仓 active 树不含该目录

#### Scenario: 新建 change 立即可见可发起

- **WHEN** 写面 `create` 成功后立即调用列表 / 详情查询（以返回的 id 寻址）与 `change_flow_start` 前置校验
- **THEN** 该 change 以 db 建档条目出现在清单（active 组，携 id）、详情可达（工件经 worktree 路径解析）、发起校验通过（已建档且 requirement 相位表在位）

#### Scenario: id 铸出且不复用

- **WHEN** 建档 change（或删除 / 归档其记录后）以同名再次 create
- **THEN** 新记录 id 与既往记录 id 相异（id 为一次性铸出、不复用），CreateOutcome.id 与库内记录逐字一致

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

desktop-app 命令层 SHALL 提供 `create_change` 命令（落位 `commands/changes/` 命令组；该组同组承载 change 域读命令 `list_changes` / `get_change_detail` / `read_artifact`，见 desktop-app-shell「Tauri command 轨道组织」）：三件事纪律薄包装（参数转换 → 调写面 `create` → 错误映射），blank root 显式 `Err`，返回 `Result<T, String>`。返回 DTO SHALL 为 `CreateOutcome`：`id`（本次铸出的 change 身份锚——前端以 `/changes/<id>` 进入详情）、`name`、`created`、`worktree`（本次分配的 worktree 绝对路径）、`warnings`（警告清单：脏仓引导 / bootstrap 结果 / 未知管理器注记等，干净且顺利时为空）——主仓 openspec 目录树路径知识仍 MUST NOT 下沉前端；worktree 绝对路径为**刻意出线的执行锚**（用户 review / 手动 commit / merge 可达性拍板，见 desktop-change-worktree「worktree 路径 UI 可见性」）。命令 SHALL 经 `#[specta::specta]` 出线并登记入 `all_commands!`，TS bindings 随既有管线重导出且一致性守卫通过；`CreateOutcome` 字段演进 SHALL 走 golden 显式重写流程（wire contract 冻结约束）。

#### Scenario: 命令注册与类型出线

- **WHEN** 审查 specta builder 组装与生成 bindings
- **THEN** `create_change` 在 `all_commands!` 清单与 bindings 中均有对应 typed 包装，参数与返回类型受编译期校验

#### Scenario: blank root 显式失败

- **WHEN** 以空白 root 调用 `create_change`
- **THEN** 返回 `Err`（写无空结果语义），不进入写面链路

#### Scenario: 返回 DTO 字段集

- **WHEN** 审查 `create_change` 返回类型的字段集
- **THEN** 恰为 `id` / `name` / `created` / `worktree` / `warnings` 五字段，无主仓 openspec 目录树路径字段

#### Scenario: 警告抵达前端

- **WHEN** 以脏主仓（或 bootstrap 失败夹具）调用 `create_change` 成功
- **THEN** 返回 DTO 的 `warnings` 含对应警告文案，前端对话框可行内呈现

### Requirement: 清单页新建入口

变更清单页 SHALL 提供新建变更入口：toggle 展开新建对话框（沿 explore-create-dialog 先例），对话框含名称输入（kebab-case 提示）与 goal 多行输入，均必填；提交调用 `create_change`，成功后回调父层刷新清单并导航 `/changes/<id>`（以返回 DTO 的 id 为路由参数）；后端错误 SHALL 行内呈现（break-all 错误块）。测试挂钩 SHALL 使用 data-testid（沿用既有命名风格），MUST NOT 以样式类名作查询挂钩。

#### Scenario: 创建成功流转

- **WHEN** 用户在对话框输入合法名称与 goal 并提交成功
- **THEN** 清单刷新且新 change 出现在进行中分组（携其 id），URL 导航至 `/changes/<id>` 进入详情

#### Scenario: 错误行内呈现

- **WHEN** 提交因名称冲突等原因失败
- **THEN** 对话框行内呈现后端错误信息，清单与路由不变化，用户可修改后重试

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/src/write/create.rs` | 建域铸 id | `create` 铸出 uuid 形态 id 入建档载荷（铸出点 design 定稿）；前置冲突查重仍按 name（仅拒同名 active；无唯一索引依赖）；worktree / branch / 目录树仍 name 化 |
| `crates/infra/store/src/store.rs`（建档半边） | id 主键写入 | `create_change_record` 主键 id（同名不构成主键冲突）；同名 active 拒绝由写面前置扫描承担 |
| `src/commands/changes/mod.rs` | `create_change` 出线 | `CreateOutcome` 增 `id`（五字段）；blank root / 错误映射 / 三件事纪律不变 |
| `packages/desktop/src/views/changes/components/change-create-dialog.tsx` + `change-list-view.tsx` | 成功流转 id 化 | 成功回调以 `outcome.id` 导航 `/changes/<id>`；对话框字段与错误行内呈现不变 |
| `packages/desktop/src/types/generated/bindings.ts` | 类型跟随 | `createChange` 返回 `CreateOutcome.id` 再生成；`bindings:check` 守卫拦截漂移 |
