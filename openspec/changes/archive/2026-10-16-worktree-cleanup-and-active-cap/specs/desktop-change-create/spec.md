# desktop-change-create Specification (Delta)

## MODIFIED Requirements

### Requirement: 写面 create 操作进程内创建 change

`core/workflow` 写面 `create` 操作 SHALL 创建 change 域并在同一次创建语义内分配 worktree（生命周期与落位细则见 desktop-change-worktree）：① 在 workspace 库建档（`ChangeRecord`：`id`（本次铸出的稳定唯一身份锚，UUID 形态——铸出点由 design 定稿）、`name` / `title`（人类可读标题，恒非空——`title` 入参空白时回退 `name`）/ workflow_type=入参（合法值 = `phase_table` 命中：`requirement` | `test-only`；非法值显式拒绝）/ created_at / status=active / worktree=本次分配的绝对路径 / base_commit=创建基线）；② 以主仓 HEAD 为基线 `git worktree add` 落位身份派生目录（落位与 branch `change/<name>` 仍 name 化）并建 branch `change/<name>`；③ 在 worktree 内创建 `openspec/changes/<name>/` 目录树（含父树，`create_dir_all` 语义）并写出 `explore.md`（goal 原文）；④ 执行确定性 bootstrap 与脏仓探测、收集警告。四段 SHALL 构成一次创建语义（分段顺序与失败补偿边界由 design 定稿；MUST NOT 出现「目录在而记录缺」的可用性破口：建档先行的补偿语义保持，worktree 段失败的回收边界由 design 定稿，bootstrap 段失败 MUST NOT 回滚）。MUST NOT 产出 workflow.json。该操作 SHALL 为进程内同步调用（MUST NOT 引入 tokio / async runtime，沿写面 sync 纪律；落库与 worktree / bootstrap 执行均经 port 缝由壳层装配实现，core/workflow 零 infra 依赖、零进程 spawn），SHALL 经 `Layout` 取全部磁盘路径（MUST NOT 自拼 `openspec` 目录名字面量）。前置校验 SHALL 在任何 IO 之前完成且拒绝面零副作用：名称 kebab-case + 长度、goal 非空白、`workflow_type` 合法值（`phase_table` 命中）、冲突检查（主仓 active 目录已存在或 db 已有同名 **active** 记录——name 无唯一约束，冲突判定为显式扫描查重，归档同名不拒）、git 可用性（git 不可发现或主仓非 git 仓）、branch `change/<name>` 已存在、worktree 目标目录已存在、**active 建档数上限**（db `status=active` 存量记录数 ≥ 5 时拒绝——上限 5 为硬编码定值，MUST NOT 进 `config.json`，对齐 `FEEDBACK_LOOP_LIMIT` 先例；错误文案 SHALL 含当前计数与上限并引导「归档或手动清理」）、**worktree 磁盘实占保底门槛**（`worktrees/{身份段}/` 子树直属目录数 > 7 时拒绝——门槛 = 上限 5 + 冗余 2，db 与磁盘漂移（孤儿 worktree / 手动残留不占 active 额度但占磁盘）防线；错误文案 SHALL 含当前目录数与门槛并引导 `git worktree list` 对账与手动清理）。两道上限检查为前置第八 / 九道，拒绝面零 worktree、零建档、零目录、零 branch 触碰。既有写面操作（`phase_next` / `phase_start` / `phase_log` / `backtrack` / `archive`）语义 MUST NOT 因新增语义改变（寻址参数随全库换锚改 change id，见 desktop-change-state-store）；写触点收口为 change 域目录内产物文件、workspace 库 change 状态与 worktree 建域三面，MUST NOT 泛化为任意 workspace 写通道。

#### Scenario: 空白树上创建成功并分配 worktree

- **WHEN** 对无 `changes` 目录树的 git 仓 workspace 以合法名称、goal 与 `workflow_type="requirement"` 调用写面 `create`（title 传空白）
- **THEN** worktree 落位于 `worktrees/{身份段}/<name>` 且基线为主仓 HEAD，worktree 内 `changes_root/<name>/explore.md` 存在且内容为 goal 原文（UTF-8，无附加结构包装），db 出现 `ChangeRecord`（id 铸出在案、`title` 回退 = name、`workflow_type="requirement"`、status=active、created_at、worktree、base_commit）
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

#### Scenario: sync 与 port 缝纪律保持

- **WHEN** 审查写面 `create` 的依赖与签名
- **THEN** 无 tokio / async runtime 依赖、函数为同步签名、crate 依赖零 infra/store 零 infra/vcs（落库与 worktree / bootstrap 执行均经 port 缝）、无 Tauri 相关依赖

#### Scenario: active 建档数超限被拒绝

- **WHEN** db 已有 5 条 `status=active` 记录时以合法名称与 goal 调用写面 `create`
- **THEN** 返回显式 `Err`（文案含当前 active 计数 5 与上限 5、引导「归档或手动清理」），零建档、零目录、零 worktree / branch；4 条时第 5 个创建成功；archived 记录不计入计数

#### Scenario: worktree 磁盘实占超门槛被拒绝

- **WHEN** `worktrees/{身份段}/` 子树直属目录数已达 8（如 3 个 active change 的 worktree + 5 个孤儿 / 手动残留）时调用写面 `create`
- **THEN** 返回显式 `Err`（文案含当前目录数 8 与门槛 7、引导 `git worktree list` 对账与手动清理），零建档、零目录、零 worktree / branch；目录数 ≤ 7 时照常创建

#### Scenario: 上限硬编码不进配置

- **WHEN** 审查 `config.json` schema 与写面常量面
- **THEN** 上限 5 与门槛 7（= 5 + 2 派生）为源码常量单点，`config.json` 零新增键、config schema 零新条目

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/src/write/create.rs`（前置八 / 九道，新） | 上限两道执法 | 前置⑧ active 计数（store 读 `list_change_records` 过滤 `status=active`）；前置⑨ `worktrees/{身份段}/` 直属目录数（fs 扫描，worktree_root 父锚消费）；上限常量驻 create 私有（沿 `BOOTSTRAP_LOCKFILE_COMMANDS` / `FEEDBACK_LOOP_LIMIT` 先例，命名 design 定稿）；文案含计数 / 上限 / 引导 |
| `crates/core/workflow/src/write/create_test.rs` | 拒绝面覆盖 | 第 6 active 拒、8 目录拒、4 active / 7 目录边界通过、文案断言、拒绝面零副作用（零 vcs 调用可断言——两道检查先于 git 探测） |
