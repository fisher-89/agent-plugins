# desktop-change-chat Specification (Delta)

## MODIFIED Requirements

### Requirement: change 环境边界（worktree 为 cwd）

面板发起的会话 SHALL 在 **change 环境**下运行，其执行根（cwd）SHALL 为 `detail.worktree ?? root`，且该值只作会话 cwd、MUST NOT 作 store 身份锚：

- **cwd 派生**：change 带 worktree（`detail.worktree !== null`）时 SHALL 以该绝对路径为 cwd；legacy 记录（`worktree === null`）SHALL 回落 workspace root。cwd SHALL 只进会话 cwd 面（工具路径沙箱根 / bash `current_dir` / 每轮 `AGENT.md` 与前置注入的读取根），**store 寻址 SHALL 恒为 workspace root**（双 root 不变量，与 change 流程同构）。
- **存在性策略**：归档 change（worktree 已被 Cleanup 移除）SHALL 以只读态处理（见「禁发与只读状态矩阵」），MUST NOT 因 cwd 缺失向用户抛发起点 `Err`；非归档 change 的 cwd 缺失 SHALL 由命令层 `is_dir` 探测显式 `Err`（文案含路径），MUST NOT 让失败深埋进引擎工具调用。
- **提示面**：执行根（worktree 路径）SHALL 由 change 详情头的 worktree 信息行（`detail-worktree`）可见；面板内 MUST NOT 再复制一条执行根文案行（避免与详情头重复）。沙箱根收缩（看不到主工作区未提交改动）属既定边界，MUST NOT 以文案承诺跨根可读。
- **范围**：本变更只落 cwd 层——工具面 SHALL 按注入面单源装配（`desktop-agent-execution`「会话级工具面注入与 explore 笔记工具」：default 七工具 + 矩阵勾选的 archi 工具 + 恒在的 ask / 只读 DB 工具两件 `db_tables` / `db_query`），MUST NOT 为 change 来源新设只读工具集 / 新 `SessionToolset` 变体，提示词前导（stance 模板）显式不在本变更内。
- **续话一致性留痕**：cwd 不落会话配置快照，故非来源侧（如调试页按 `change` 筛选）续话不会携带 cwd——来源侧 SHALL 恒显式传 cwd；按归属反查 worktree 的隐式兜底为已知缺口，MUST NOT 在本变更内隐式实现。

#### Scenario: 会话跑在 worktree 内

- **WHEN** 在带 worktree 的非归档 change 面板中发起一轮包含路径读取的对话
- **THEN** 该轮工具路径解析与 bash `current_dir` 均在 worktree 目录内（沙箱根 = cwd 可验），且会话行落在 workspace root 所属库（workspace 侧清单可查、未新生第二个 workspace 库）

#### Scenario: legacy 记录回落主 root

- **WHEN** 对 `worktree === null` 的 legacy change 发起面板对话
- **THEN** cwd 回落 workspace root，行为与既有 exec / explore 轨会话一致（零新增参数语义）

#### Scenario: cwd 缺失显式失败

- **WHEN** 非归档 change 的 worktree 路径在盘上不存在（被手动删除）时尝试发起面板对话
- **THEN** 发起点返回显式错误（携带该路径），不进入引擎、不产生会话行、不留半截状态

#### Scenario: 工具面按单源装配且提示词面不扩

- **WHEN** 审查 change 来源会话的工具面装配
- **THEN** 注入面按 `desktop-agent-execution` 的名字清单单源装配（default 七工具 + 矩阵命中的 archi 工具 + 恒在的 `AskUserQuestion` 与只读 DB 工具两件；无 change 专属工具集分支、无新 `SessionToolset` 变体），首轮提示词无 change 环境前导模板插入


## Module Contract

主基线既有行逐字沿用；本 delta 受影响模块如下：

| 模块 | 职责 | 关键契约（Function / API） |
|------|------|------|
| `openspec/specs/desktop-change-chat`（表述对齐） | change 面板会话工具面口径 | 「change 环境边界（worktree 为 cwd）」的「范围」bullet 与「工具面按单源装配且提示词面不扩」scenario：工具面口径改为指向 `desktop-agent-execution` 注入面单源（不再写死「default 七工具」）；cwd 派生、双 root 不变量、存在性策略、续话一致性留痕逐字不变 |
| `crates/infra/agent/src/compose.rs`（零 cwd 语义改动） | change 臂装配 | cwd 仍只进 `SessionCtx.workspace_root` 与工具沙箱根；change 臂新增的两 DB 工具名不读 cwd、库身份锚恒 workspace root（见 desktop-agent-db-tools「会话可见性与执行上下文铸造」） |
