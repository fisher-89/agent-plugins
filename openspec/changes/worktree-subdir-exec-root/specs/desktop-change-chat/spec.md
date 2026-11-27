# desktop-change-chat Specification (Delta)

## MODIFIED Requirements

### Requirement: change 环境边界（worktree 为 cwd）

面板发起的会话 SHALL 在 **change 环境**下运行，其执行根（cwd）SHALL 为 `detail.execRoot ?? detail.worktree ?? root`（执行根列在案时优先；根 workspace 下执行根列缺省、回落落位目录；legacy 两者皆缺、回落 workspace root），且该值只作会话 cwd、MUST NOT 作 store 身份锚：

- **cwd 派生**：`detail.execRoot` 非 null 时 SHALL 以该绝对路径为 cwd（workspace root 为仓库子目录时的执行面）；`execRoot` 为 null 且 `worktree` 非 null（根 workspace）时 SHALL 以 `worktree` 为 cwd；两者皆 null（legacy 记录）SHALL 回落 workspace root。cwd SHALL 只进会话 cwd 面（工具路径沙箱根 / bash `current_dir` / `AGENT.md` 与前置注入的读取根——该文件在会话首轮快照拍存时读取一次，见 `desktop-agent-execution`「会话系统提示词快照的拍存与续轮复用」），**store 寻址 SHALL 恒为 workspace root**（双 root 不变量，与 change 流程同构）。
- **存在性策略**：归档 change（worktree 已被 Cleanup 移除）SHALL 以只读态处理（见「禁发与只读状态矩阵」），MUST NOT 因 cwd 缺失向用户抛发起点 `Err`；非归档 change 的 cwd 缺失 SHALL 由命令层 `is_dir` 探测显式 `Err`（文案含路径），MUST NOT 让失败深埋进引擎工具调用。
- **提示面**：执行根 SHALL 由 change 详情头的信息面可见（`execRoot` 非空时两行：落位目录 + 执行根；`execRoot` 为 null 时落位目录一行即执行根，见 desktop-change-worktree「worktree 路径 UI 可见性」）；面板内 MUST NOT 再复制一条执行根文案行（避免与详情头重复）。沙箱根收缩（看不到主工作区未提交改动）属既定边界，MUST NOT 以文案承诺跨根可读。
- **范围**：本变更只落 cwd 层——工具面 SHALL 按注入面单源装配（`desktop-agent-execution`「会话级工具面注入与 explore 笔记工具」：default 七工具 + 矩阵勾选的 archi 工具 + 恒在的 ask / 只读 DB 工具两件 `db_tables` / `db_query`），MUST NOT 为 change 来源新设只读工具集 / 新 `SessionToolset` 变体，提示词前导（stance 模板）显式不在本变更内。
- **续话一致性留痕**：cwd 不落会话配置快照，故非来源侧（如调试页按 `change` 筛选）续话不会携带 cwd——来源侧 SHALL 恒显式传 cwd；按归属反查 worktree 的隐式兜底为已知缺口，MUST NOT 在本变更内隐式实现。

#### Scenario: 会话跑在执行根内

- **WHEN** 在带 worktree 的非归档 change 面板中发起一轮包含路径读取的对话
- **THEN** 该轮工具路径解析与 bash `current_dir` 均在执行根内（沙箱根 = cwd 可验），且会话行落在 workspace root 所属库（workspace 侧清单可查、未新生第二个 workspace 库）

#### Scenario: 子目录 workspace 的 cwd 落执行根

- **WHEN** 在子目录 workspace（`rel = packages/desktop`）的 worktree change 面板中发起一轮包含路径读取的对话
- **THEN** 会话 cwd = `host/packages/desktop`（`detail.execRoot`），工具路径解析与 bash `current_dir` 均在该子树内；沙箱拒绝跳出该子树（改不到 host 根与主仓）；会话行仍落 workspace root 所属库

#### Scenario: legacy 记录回落主 root

- **WHEN** 对 `worktree === null` 的 legacy change 发起面板对话
- **THEN** cwd 回落 workspace root，行为与既有 exec / explore 轨会话一致（零新增参数语义）

#### Scenario: cwd 缺失显式失败

- **WHEN** 非归档 change 的 worktree 路径在盘上不存在（被手动删除）时尝试发起面板对话
- **THEN** 发起点返回显式错误（携带该路径），不进入引擎、不产生会话行、不留半截状态

#### Scenario: 工具面按单源装配且提示词面不扩

- **WHEN** 审查 change 来源会话的工具面装配
- **THEN** 注入面按 `desktop-agent-execution` 的名字清单单源装配（default 七工具 + 矩阵命中的 archi 工具 + 恒在的 `ask_user_question` 与只读 DB 工具两件；无 change 专属工具集分支、无新 `SessionToolset` 变体），首轮提示词无 change 环境前导模板插入

## Module Contract

### 模块增量：会话 cwd 取执行根（worktree-subdir-exec-root）

主基线既有行逐字沿用；本变更受影响模块如下：

| 模块 | 职责 | 关键契约（Function / API / CLI / Component） |
|------|------|------|
| `packages/desktop/src/views/changes/agent/hooks/use-change-chat.ts` | cwd 派生 | `cwd = archived ? null : (execRoot ?? worktree)`；参数面增 `execRoot: string \| null`（与 `worktree` 并列），归档恒 `null` 语义不变 |
| `packages/desktop/src/views/changes/change-detail-view.tsx` | 传参 | `ChangeChatPanel` 的 `cwd` 改由该表达式供给；`change-chat-panel.tsx` 零改动（仍按 props 收 `cwd`） |
| `crates/core/workflow/src/queries/detail.rs`（`ChangeDetail`） | 出线来源 | `execRoot` 字段（见 desktop-change-queries） |
| `packages/desktop/src/views/changes/agent/hooks/use-change-chat.test.ts` | 测试 | cwd 三态（执行根在案 / 仅 worktree / 皆 null）+ 归档恒 null |
