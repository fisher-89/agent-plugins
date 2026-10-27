# desktop-agent-execution Specification（delta: archi-graph-doc-agent）

## MODIFIED Requirements

### Requirement: 会话级工具面注入与 explore 笔记工具

`SessionInjections` SHALL 自本条起激活为引擎消费面（切片③收编），形态为「收窄 + 受控追加」双位：

- **core 契约扩展**：`SessionInjections` SHALL 新增受控枚举位 `toolset: Option<SessionToolset>`（值集 = `Explore`（explore 笔记工具面）与 `Archi { module: bool, search: bool }`（架构只读工具面，见 desktop-archi-agent））；既有 `tools: Option<Vec<String>>` 位激活为七工具子集收窄表达（值必须为七工具名子集，非法名在装配层 `Err`）。`preamble` 位维持不消费（系统提示词唯一来源仍为 `AGENT.md`，见「SDK 引擎系统提示词（AGENT.md 注入）」）。词汇封闭原则延续：专用工具仅经 `toolset` 枚举注入，自由字符串工具名 MUST NOT 进执行面；core 仅承载词汇，解释归 infra。新增 `Archi` 变体 MUST NOT 改变 `Explore` 臂的任何既有语义（`tools` 收窄位、成员校验、硬禁用、`RunStarted.tools` 上报四条零改写）。
- **compose 装配（生产路径激活）**：`begin` SHALL 以 `provenance` 判别注入面，分两臂——① `provenance.source == "explore"`：注入 `tools = [read, grep, glob, ls, edit, bash]`（`write` 移除）+ `toolset = Explore`；② `provenance.source == "change"`：SHALL 按 `source_ref` 定式 `{change_id}/{phase}/{role}/{attempt}` 的尾三段解析 `(phase, role)` 并查 `(phase, role)` → archi 工具子集矩阵（矩阵单源见 desktop-archi-agent「SessionToolset::Archi 与 (phase, role) 注入矩阵」），命中行注入 `toolset = Archi { .. }` 且 `tools` 位保持 `None`（七工具全量），未命中行与**解析失败（畸形 / 短段 / 未知 phase）**SHALL 恒 `SessionInjections::default()`（fail-closed，零 archi 工具）。其余来源（debug 等）SHALL 恒 `SessionInjections::default()`（七工具面，零变化）。
- **loop 组装与硬禁用**：definitions SHALL 按注入面组装（收窄子集 + `toolset` 命中的专用工具定义——`Explore` 追加 `create_explore_note`、`Archi { module, search }` 按位追加以对应 archi 工具）；`RunStarted.tools` SHALL 报本轮实际下发的工具面。工具执行前 SHALL 增**成员校验**：调用名 ∉ 本轮实际下发的 definitions 即拒绝——`SystemNotice{subtype:"tool_unavailable"}` + is_error=true 的 ToolResult 回灌（与 `permission_denied` 同构，run 不中断）。「从 definitions 移除」自此等于硬禁用，幻觉调用 MUST NOT 直通执行（现分发链 `policy → sandbox → execute` 无此校验的缺口收口）。
- **`create_explore_note(name, content)` 契约**：无路径参数，服务端解析目标。执行体 SHALL 在**执行时**（非轮建立快照）按会话指针圈定当前 explore 记录（记录 `session_id` == 本会话 id，经 store 探索记录清单过滤；miss → 显式 `Err`）：
  - 记录为占位名（`draft-` 前缀，占位语义见 desktop-workspace-store「ExploreRecord 模型与 explore 清单操作面」）→ 前置校验（name 合法 kebab / 目标名无既有记录 / 目标笔记文件不存在）后原子两步：① 笔记落盘 `Layout.explores_root/<name>.md`（create-only，缺失父目录补建）② store `rename_explore_record` in-place 改名（保主键保会话链）→ 返回笔记路径（repo 相对、`/` 分隔）。文件与记录自诞生即最终名；
  - 记录已定名 → 仅当 `name` == 记录名且笔记文件缺失时补建（会话外删除的孤儿重建）；否则 `Err` 附现有记录名与笔记路径（错误文案即恢复通道——agent 据此转用 Edit）；
  - 失败面（非法 kebab / 目标名已存在 / 文件已存在（附路径）/ 记录 miss）SHALL 显式 `Err` 回灌（agent 换名重试自愈）；①后②失败的残留窗口 SHALL 以 `Err` 附残留物与恢复指引收口（fs + db 跨存储非真原子，前置校验收窄窗口——promote 先例）。
- **执行体装配缝**：专用工具的执行上下文 SHALL 经组合根（compose）以 per-turn 形态铸造（workspace store 句柄 + root），sdk loop 经 `LoopTurn` 携带的上下文路由；`tools::execute` 对七通用工具 SHALL 保持 `(root, name, input)` 纯函数语义，MUST NOT 把 store 句柄塞进其签名。
- **policy 档位归属**：`create_explore_note` SHALL 归写面档位（Default 拒绝、AcceptEdits / BypassPermissions 放行）——决策表纯函数扩展（见「SDK 引擎静态权限与路径沙箱」表）；两个 archi 只读工具 SHALL 归只读面（三档全放行，与 `read` / `grep` / `glob` / `ls` 同款），写面 / 执行面清单零变化（契约细节见 desktop-archi-agent「只读档位归属与成员校验不变」）。

#### Scenario: explore 会话注入面

- **WHEN** 以 `source="explore"` 发起会话并审查该轮请求组装
- **THEN** definitions 为 read / grep / glob / ls / edit / bash / create_explore_note（无 `write`），`RunStarted.tools` 逐名一致上报该面

#### Scenario: 幻觉 write 调用被拒

- **WHEN** explore 会话中模型发出已从 definitions 移除的 `write` 工具调用
- **THEN** 成员校验拒绝：`SystemNotice{subtype:"tool_unavailable"}` + is_error=true 的 ToolResult（与 tool_use 同 id）回灌，run 不中断、不触执行体

#### Scenario: create 原子定名

- **WHEN** 占位记录（`draft-1760000000000`）会话中模型调用 `create_explore_note(name="api-retry", content=<初稿>)`
- **THEN** 磁盘出现 `openspec/explores/api-retry.md`（内容 = content），store 记录 `name="api-retry"`（`id` / `session_id` 不变），返回文本含笔记路径；占位名文件全程不存在

#### Scenario: create 拒绝面零半边残留

- **WHEN** 分别以非法 kebab 名、撞已有记录名、目标文件已存在（既有笔记）调用 `create_explore_note`
- **THEN** 均显式 `Err` 回灌（文件已存在臂附路径），磁盘与库内零变化（前置校验先于任何写动作）

#### Scenario: 已定名记录异名拦截

- **WHEN** 已定名记录（`api-retry`，笔记在盘）会话中模型调用 `create_explore_note(name="other-name", …)`
- **THEN** `Err` 附现有名 `api-retry` 与笔记路径，零改动；同名且文件缺失时补建成功（孤儿重建）

#### Scenario: 非 explore 来源零变化

- **WHEN** 以 `source="debug"`（或不传来源）发起会话
- **THEN** definitions 与 `RunStarted.tools` 仍为七工具（含 `write`），成员校验对七工具全放行，行为与注入机制上线前一致

#### Scenario: change 来源按矩阵注入 archi 工具面

- **WHEN** 以 `source="change"` 且 `source_ref` 为 `<change_id>/dev-design/executor/1` 发起会话
- **THEN** `toolset = Archi { module: true, search: true }` 且 `tools` 位为 `None`（七通用工具全量）；definitions 含 `archi_module` / `archi_search` 两定义，`RunStarted.tools` 逐名一致上报

#### Scenario: 矩阵未命中行与畸形 source_ref fail-closed

- **WHEN** 分别以 `<change_id>/test-gen/executor/1`、`<change_id>/implement/executor`（段数不足）与空 `source_ref` 发起 change 会话
- **THEN** 三者均落 `SessionInjections::default()`（七工具面、零 archi 工具），不抛装配错误、不回退为全量 archi 工具

#### Scenario: Explore 臂零回归

- **WHEN** 以 `source="explore"` 发起会话并审查装配产物
- **THEN** 六工具收窄子集与 `toolset = Explore` 逐字与本次变更前一致；新增 `Archi` 变体不改变该臂的任何可观察行为

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/agent/src/runner.rs` | `SessionToolset` 词表 | 值集 = `Explore` \| `Archi { module: bool, search: bool }`；serde camelCase additive（旧线缺字段读 `None`） |
| `crates/infra/agent/src/compose.rs` | 注入装配双臂 | explore 臂逐字不变；change 臂按 `source_ref` 尾三段 `{phase}/{role}/{attempt}` 查矩阵；未命中 / 解析失败 fail-closed；命中时 `tools = None` + `toolset = Archi{..}` |
| `crates/infra/agent/src/sdk/tools.rs` | definitions 组装 | 按 `toolset` 位追加 archi 工具定义；`RunStarted.tools` 与成员校验自 definitions 派生（下发面单源不变） |
| `crates/infra/agent/src/sdk/policy.rs` | 只读面扩展 | `READONLY_TOOLS` += `archi_module` / `archi_search`；写面 / 执行面清单零变化 |
| `crates/infra/agent/src/sdk/loop.rs` | 执行臂路由 | 两个 archi 名路由至专用执行体（上下文缺席以 `is_error` 回灌），成员校验先于 policy 的次序不变 |
