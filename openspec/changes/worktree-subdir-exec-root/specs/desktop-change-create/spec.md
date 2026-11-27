# desktop-change-create Specification (Delta)

## MODIFIED Requirements

### Requirement: 写面 create 操作进程内创建 change

`core/workflow` 写面 `create` 操作 SHALL 创建 change 域并在同一次创建语义内分配 worktree（生命周期与落位细则见 desktop-change-worktree）：① 在 workspace 库建档（`ChangeRecord`：`id`（本次铸出的稳定唯一身份锚，UUID 形态——铸出点由 design 定稿）、`name` / `title`（人类可读标题，恒非空——`title` 入参空白时回退 `name`）/ workflow_type=入参（合法值 = `phase_table` 命中：`requirement` | `test-only`；非法值显式拒绝）/ created_at / status=active / worktree=本次分配的绝对路径（git 落位目录 host）/ exec_root=本次执行根（`rel` 非空时 = `worktree/<rel>`；根 workspace MUST NOT 写该列，见 desktop-change-worktree「子目录 workspace 的执行根对齐」）/ base_commit=创建基线）；② 以主仓 HEAD 为基线 `git worktree add` 落位于 git 根同级 `.worktrees/<仓库目录名>/`（落位与 branch `change/<name>` 仍 name 化）并建 branch `change/<name>`；③ 在**执行根**内创建 `openspec/changes/<name>/` 目录树（含父树，`create_dir_all` 语义）并写出 `explore.md`（goal 原文；`rel = ""` 时执行根即 worktree 根）；④ 执行确定性 bootstrap（扫描与安装锚 = 执行根）与脏仓探测、收集警告。四段 SHALL 构成一次创建语义（分段顺序与失败补偿边界由 design 定稿；MUST NOT 出现「目录在而记录缺」的可用性破口：建档先行的补偿语义保持，worktree 段失败的回收边界由 design 定稿，bootstrap 段失败 MUST NOT 回滚）。MUST NOT 产出 workflow.json。该操作 SHALL 为进程内同步调用（MUST NOT 引入 tokio / async runtime，沿写面 sync 纪律；落库与 worktree / bootstrap 执行均经 port 缝由壳层装配实现，core/workflow 零 infra 依赖、零进程 spawn），SHALL 经 `Layout` 取全部磁盘路径（MUST NOT 自拼 `openspec` 目录名字面量）。前置校验 SHALL 在任何写 IO 之前完成且拒绝面零副作用：名称 kebab-case + 长度、goal 非空白、`workflow_type` 合法值（`phase_table` 命中）、冲突检查（主仓 active 目录已存在或 db 已有同名 **active** 记录——name 无唯一约束，冲突判定为显式扫描查重，归档同名不拒）、git 可用性（git 不可发现或主仓非 git 仓）、branch `change/<name>` 已存在、worktree 目标目录已存在、**active 建档数上限**（db `status=active` 存量记录数 ≥ 5 时拒绝——上限 5 为硬编码定值，MUST NOT 进 `config.json`，对齐 `FEEDBACK_LOOP_LIMIT` 先例；错误文案 SHALL 含当前计数与上限并引导「归档或手动清理」）、**子目录跟踪判据**（workspace root 为 git 仓库子目录时，其相对偏移须在 `HEAD` 中在案——`rel = ""` 恒命中且 MUST NOT 新增 git spawn；缺失形态（未跟踪 / `HEAD` 已删 / 被 gitignore）显式拒绝并给跟踪引导，见 desktop-change-worktree「子目录 workspace 的执行根对齐」）、**worktree 磁盘实占保底门槛**（`.worktrees/<仓库目录名>/` 子树直属目录数 > 7 时拒绝——门槛 = 上限 5 + 冗余 2，db 与磁盘漂移（孤儿 worktree / 手动残留不占 active 额度但占磁盘）防线；错误文案 SHALL 含当前目录数、门槛与 worktree 落位根绝对路径，并以**双源**引导（`git worktree list` 对账 + 直接检查该落位根下的子目录；已注销孤儿不在 git 登记面——`list` 不显示、`prune` 无事可做，单源指引会让用户无从自救）。自愈不归本门槛（回收单点自带 OS 层兜底，见 desktop-change-worktree「worktree 回收两段式」），本门槛仅为失控防线）。两道上限检查为前置第八 / 九道，子目录跟踪判据为**第十道**（位置 MUST 在 `git worktree add` 与建档之前），拒绝面零 worktree、零建档、零目录、零 branch 触碰。`git worktree add` 成功后 SHALL 断言执行根目录在场（后置断言），缺席即走既有补偿链（回收 host + branch + 建档）后 `Err`。既有写面操作（`phase_next` / `phase_start` / `phase_log` / `backtrack` / `archive`）语义 MUST NOT 因新增语义改变（寻址参数随全库换锚改 change id，见 desktop-change-state-store）；写触点收口为 change 域目录内产物文件、workspace 库 change 状态与 worktree 建域三面，MUST NOT 泛化为任意 workspace 写通道。

#### Scenario: 空白树上创建成功并分配 worktree

- **WHEN** 对无 `changes` 目录树的 git 仓 workspace 以合法名称、goal 与 `workflow_type="requirement"` 调用写面 `create`（title 传空白）
- **THEN** worktree 落位于 `<git 根父目录>/.worktrees/<git 根目录名>/<name>` 且基线为主仓 HEAD，执行根内（根 workspace 下即 worktree 根）`changes_root/<name>/explore.md` 存在且内容为 goal 原文（UTF-8，无附加结构包装），db 出现 `ChangeRecord`（id 铸出在案、`title` 回退 = name、`workflow_type="requirement"`、status=active、created_at、worktree、base_commit）
- **AND** worktree 目录内无 workflow.json，`eval.json` 与 `.openspec.yaml` 不存在；主仓 active 树不含该目录

#### Scenario: test-only 建档落类型

- **WHEN** 以 `workflow_type="test-only"` 调用写面 `create`
- **THEN** `ChangeRecord.workflow_type = "test-only"`，随后 `phase_table("test-only")` 返回 `Some`（六相位表）；详情 / 发起面按 test-only 分派

#### Scenario: 非法 workflow_type 被拒绝

- **WHEN** 以 `workflow_type="bug-fix"`（或空白）调用写面 `create`
- **THEN** 返回显式错误（合法值引导），零建档、零目录、零 worktree / branch

#### Scenario: 新建 change 立即可见可发起

- **WHEN** 写面 `create` 成功后立即调用列表 / 详情查询（以返回的 id 寻址）与 `change_flow_start` 前置校验
- **THEN** 该 change 以 db 建档条目出现在清单（active 组，携 id、title）、详情可达（工件经 worktree 路径解析）、发起校验通过（已建档且其 workflow_type 相位表在位）

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

#### Scenario: title 回退与显式继承

- **WHEN** 手动路径以空白 title 调用写面 `create`，promote 路径以 explore.title（如「API 重试调研」）调用写面 `create`
- **THEN** 手动路径 `ChangeRecord.title` 回退 = name；promote 路径 `ChangeRecord.title` 为「API 重试调研」，两者均非空

#### Scenario: 子目录未跟踪即拒绝且零副作用

- **WHEN** workspace root 为 git 仓库子目录且该子目录在 `HEAD` 中不存在（未跟踪 / 已删除 / 被 ignore）时调用写面 `create`
- **THEN** 返回显式 `Err`（文案含相对偏移、成因与跟踪引导）；零建档、零目录、零 worktree、零 branch；主仓 `git status` 与 `git worktree list` 零变化

#### Scenario: sync 与 port 缝纪律保持

- **WHEN** 审查写面 `create` 的依赖与签名
- **THEN** 无 tokio / async runtime 依赖、函数为同步签名、crate 依赖零 infra/store 零 infra/vcs（落库与 worktree / bootstrap 执行均经 port 缝）、无 Tauri 相关依赖

#### Scenario: active 建档数超限被拒绝

- **WHEN** db 已有 5 条 `status=active` 记录时以合法名称与 goal 调用写面 `create`
- **THEN** 返回显式 `Err`（文案含当前 active 计数 5 与上限 5、引导「归档或手动清理」），零建档、零目录、零 worktree / branch；4 条时第 5 个创建成功；archived 记录不计入计数

#### Scenario: worktree 磁盘实占超门槛被拒绝

- **WHEN** `.worktrees/<仓库目录名>/` 子树直属目录数已达 8（如 3 个 active change 的 worktree + 5 个孤儿 / 手动残留）时调用写面 `create`
- **THEN** 返回显式 `Err`（文案含当前目录数 8、门槛 7、worktree 落位根绝对路径，并以双源引导——`git worktree list` 对账 + 直接检查该落位根下的子目录），零建档、零目录、零 worktree / branch；目录数 ≤ 7 时照常创建

#### Scenario: 上限硬编码不进配置

- **WHEN** 审查 `config.json` schema 与写面常量面
- **THEN** 上限 5 与门槛 7（= 5 + 2 派生）为源码常量单点，`config.json` / config schema 中不存在承载上限的键（`projects` 键只承载项目锚点声明——`root` / `language` / `depends_on` 三项，见 desktop-workspace-config 与 desktop-change-worktree，不承载上限）

### Requirement: create_change IPC 命令面

desktop-app 命令层 SHALL 提供 `create_change` 命令（落位 `commands/changes/` 命令组；该组同组承载 change 域读命令 `list_changes` / `get_change_detail` / `read_artifact`，见 desktop-app-shell「Tauri command 轨道组织」）：三件事纪律薄包装（参数转换 → 调写面 `create` → 错误映射），blank root 显式 `Err`，返回 `Result<T, String>`。返回 DTO SHALL 为 `CreateOutcome`：`id`（本次铸出的 change 身份锚——前端以 `/changes/<id>` 进入详情）、`name`、`created`、`worktree`（本次分配的 worktree 绝对路径——git 落位目录）、`execRoot`（本次执行根；根 workspace / legacy 形态 → `null`）、`warnings`（警告清单：脏仓引导 / bootstrap 结果 / 未知管理器注记等，干净且顺利时为空）——主仓 openspec 目录树路径知识仍 MUST NOT 下沉前端；`worktree` 与 `execRoot` 为**刻意出线的两列**（前者供 review / 手动 commit / merge 可达性，后者指向用户实际编辑的工作面，见 desktop-change-worktree「worktree 路径 UI 可见性」），两者 MUST NOT 互相顶替。手动新建路径 SHALL 在命令层以 `title = name` 调写面 `create`（IPC 签名为四参 `(root, name, goal, workflow_type)`，title 不加输入框，workflow_type 自类型选择器——默认 requirement）；promote 路径经 `create_change_with` 泛型缝显式传 `explore.title` 与 workflow_type。命令 SHALL 经 `#[specta::specta]` 出线并登记入 `all_commands!`，TS bindings 随既有管线重导出（过期由提交前人工核对）；`CreateOutcome` 字段演进 SHALL 走 golden 显式重写流程（wire contract 冻结约束）。

#### Scenario: 命令注册与类型出线

- **WHEN** 审查 specta builder 组装与生成 bindings
- **THEN** `create_change` 在 `all_commands!` 清单与 bindings 中均有对应 typed 包装（四参），参数与返回类型受编译期校验

#### Scenario: blank root 显式失败

- **WHEN** 以空白 root 调用 `create_change`
- **THEN** 返回 `Err`（写无空结果语义），不进入写面链路

#### Scenario: 返回 DTO 字段集

- **WHEN** 审查 `create_change` 返回类型的字段集
- **THEN** 恰为 `id` / `name` / `created` / `worktree` / `execRoot` / `warnings` 六字段（title 与 workflow_type 不进 CreateOutcome，经详情 / 清单出线），无主仓 openspec 目录树路径字段；根 workspace 的 `execRoot` 为 `null`

#### Scenario: 警告抵达前端

- **WHEN** 以脏主仓（或 bootstrap 失败夹具）调用 `create_change` 成功
- **THEN** 返回 DTO 的 `warnings` 含对应警告文案，前端对话框可行内呈现

#### Scenario: workflow_type 透传

- **WHEN** 以 `workflow_type="test-only"` 调用 `create_change`
- **THEN** 命令层经 `create_change_with` 把 `"test-only"` 透传写面 `create`，铸出 change 的 `workflow_type = "test-only"`；非法值写面拒绝、命令层原样回传 `Err`

## Module Contract

### 模块增量：create 双列建档与前置第十道（worktree-subdir-exec-root）

主基线既有行逐字沿用；本变更受影响模块如下：

| 模块 | 职责 | 关键契约（Function / API / CLI / Component） |
|------|------|------|
| `crates/core/workflow/src/write/create.rs`（`create`） | 建域组合 | 前置校验新增第十道 `vcs.tracked_at_head(main_root, rel)`（`rel = ""` 零 spawn）；建档 `ChangeStateRecord` 同时携 `worktree`（host）与 `exec_root`；`resolve(&exec_root)` 建目录树 + `explore.md`；后置 `exec_root.is_dir()` 断言；`bootstrap(&exec_root, projects, …)` |
| `crates/core/workflow/src/write/create.rs`（`CreateOutcome`） | 出线 DTO | 增 `pub exec_root: Option<String>`（serde camelCase → `execRoot`）；`worktree: String` 语义钉为 host |
| `crates/core/workflow/src/write/promote.rs` | promote 透传 | 逐字不变（`promote_explore` → `create` 同参数，执行根由 create 承担；源 explore 读取仍锚主仓 workspace root） |
| `src/commands/changes/mod.rs`（`create_change` / `create_change_with`） | 命令面 | 薄包装零新增参数；`CreateOutcome` 原样透传（新增字段随 specta 出线） |
| `packages/desktop/src/views/changes/components/change-create-dialog.tsx` | 成功面 | 落位行 + 条件执行根行（`execRoot = null` 零占位） |
| `crates/core/workflow/src/write/create_test.rs` | 测试 | 前置第十道六形态、双列建档、后置补偿、子目录建域四件套（见 desktop-change-worktree 与提案 AC-2 / AC-3） |
