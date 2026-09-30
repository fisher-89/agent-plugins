# desktop-agent-execution 变更（desktop-workspace-db-split）

## ADDED Requirements

### Requirement: run 事件落库与重放（workspace 维度）

agent 运行记录 SHALL 以 **workspace 维度**持久化（维度重裁定，2026-09-29：每条 run 的 cwd 恒为当前 workspace root，运行历史属 workspace 数据），落盘于所属 workspace 的独立 db 文件（全局数据目录 `workspaces/` 子树，归属与寻址见 desktop-workspace-store「全局库与 workspace 库双库布局」），MUST NOT 落全局库。两记录模型不变：`AgentRunRecord`（run 元数据）与 `AgentEventRecord`（事件流，key 为 `(run_id, seq)` 打包键）；模型 shape 与 native_model id / version 零变化，本裁定仅平移落盘归属。run id 语义演进：id 为所属 workspace 库域内自增（写事务内 max+1），MUST NOT 假定跨 workspace 全局唯一；跨库定位 SHALL 携 root（见「agent 执行命令面」root 寻址）。

事件投递 SHALL 双路 tee：Tauri Channel（命令作用域实时流，推前端时间线）+ store sink（逐事件落库），两路 seq 一致。历史查看 SHALL 走所属 workspace 库的 store 查询重放（`agent_runs` / `agent_run_events` invoke，携 root），MUST NOT 依赖全局事件广播或要求运行进程存活。MVP MUST NOT 实现留存清理（转录无上限增长为已知限制，留痕未来账）；workspace 库文件级治理（备份 / 清理）随 remove 保留语义留待二期。

#### Scenario: 双路 tee

- **WHEN** 运行产生事件
- **THEN** 每事件同时到达 Channel 与 store sink 且 seq 一致；run 结束后所属 workspace 库中的 run 元数据状态收敛

#### Scenario: 重放不依赖进程

- **WHEN** 运行结束后（或应用重启后）在原 workspace 下点开历史运行
- **THEN** 经 `agent_runs` / `agent_run_events`（携 root）查询还原完整事件时间线，与实时流内容一致

#### Scenario: workspace 库落位与隔离

- **WHEN** workspace A 与 B 各有运行历史后审查两库内容与查询结果
- **THEN** A 的 runs / 事件仅存在于 A 的 workspace 库文件，B 的任何查询不可见（反之亦然）；全局库无 run / 事件数据；两库允许出现相同 run id（域内自增）

## REMOVED Requirements

### Requirement: run 事件落库与重放（user 维度）

**Reason**: 维度重裁定（desktop-workspace-db-split，用户 2026-09-29）：agent 运行历史与事件流属 workspace 数据，归 **workspace 维度**，落盘载体改为所属 workspace 的独立 db 文件；原 user 维度落位裁定退役。原 requirement 文本尚残留 redb 时代表名（`user_agent_runs` / `user_agent_run_events`），一并由新 requirement 覆盖。

**Migration**: 由本变更 ADDED「run 事件落库与重放（workspace 维度）」承接；存量数据按用户裁定零迁移处置（旧单库惰性废弃，见 desktop-workspace-store「全新文件组冷启动与旧库惰性废弃」），无数据搬移。

## MODIFIED Requirements

### Requirement: agent 执行命令面

`commands/exec/` SHALL 承载执行与查询命令：

- `agent_start`：执行命令。body SHALL 维持三件事纪律（参数转换 → 调用 → 错误映射），编排 SHALL 收在 `run_agent()` 编排函数（组装 runner → 事件流 tee 双 sink → 状态收敛）；该函数与 `*_inner` 同列 app 层微形态（详见 desktop-app-shell）。命令 SHALL 增可选参数：`resume_session_id`（续会话）、`source`（来源受控字符串，缺省 `debug`）、`source_ref`（来源内定位）、`parent_run_id`（链上游 run）——编排 SHALL 将其写入 run 记录字段面；不传链参数时生成的记录 `source="debug"`、链字段为 `None`，字段面行为与演进前一致（返回时序演进为提前 resolve running 记录，见「agent_stop 终止与提前 resolve」）。落库 SHALL 经 root 解析所属 workspace 库（`WorkspaceStores::for_root`，见 desktop-workspace-store），run 与事件写入当前 workspace 的库文件
- `agent_runs` / `agent_run_events` / `agent_run_chain`：查询薄包装（无状态，参数含 root → workspace 库解析 → store 查询 → DTO）；`agent_run_chain` 的 `source_ref` 为 workspace 库域内的 explore 记录 id，root 寻址与库域内 id 配套消解跨库歧义
- `agent_stop`：终止命令（寻址、`stopped` 收敛与进程树击杀见「agent_stop 终止与提前 resolve」）；寻址 SHALL 携 root（run id 为 workspace 库域内自增，裸 id 跨库歧义由 root 消解），`RunStopRegistry` 寻址键随 root 演进

错误约定沿用既有模板 `Result<T, String>`：CLI 不可发现、spawn 失败、workspace 库打开失败等 SHALL 以 `Err` 抵达前端，MUST NOT 静默吞掉。

#### Scenario: agent_start 编排收口

- **WHEN** 审查 `agent_start` 实现
- **THEN** 命令体为参数转换 + 调用 `run_agent()` + 错误映射三段，runner 组装与 tee 在编排函数内，不膨胀命令体

#### Scenario: 来源与 resume 参数透传

- **WHEN** explore 页以 `source="explore"`、`source_ref=<记录键>`、`resume_session_id=<sid>` invoke `agent_start`
- **THEN** 生成 run 记录携带对应字段且进程以 `--resume` 续话；调试页 invoke（不传这些参数）生成的记录 `source="debug"`、链字段为 `None`

#### Scenario: 查询与停止 root 寻址

- **WHEN** 前端 invoke `agent_runs` / `agent_run_events` / `agent_run_chain` / `agent_stop`
- **THEN** 命令无状态、按 root 解析所属 workspace 库直查/寻址返回 DTO，无领域解释；workspace A 的调用看不到 B 的 runs，同 id 并行时停止命中发起方所在库的 run

#### Scenario: 错误 reject 传达

- **WHEN** CLI 不可发现或所属 workspace 库打开失败时发起 `agent_start`
- **THEN** 前端收到 `Err(String)` 并可呈现，无静默成功

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src-tauri/crates/infra/store`（run 持久化） | run / 事件持久化（workspace 维度） | `AgentRunRecord` / `AgentEventRecord` 落所属 workspace 库；run id 库域内自增；模型 shape 与 id / version 零变化；重放查询入口签名不变（实例即上下文） |
| `dev-team::commands::exec` | 执行 + 查询命令 | `agent_start`（三件事，编排收 `run_agent()`，落库即提前 resolve，经 root 解析 workspace 库）；`agent_stop(root, run_id)`（root 消歧，幂等，进程树击杀，`stopped` 收敛）；`agent_runs` / `agent_run_events` / `agent_run_chain` 携 root 薄包装；`Result<T, String>` 错误模板 |
| `run_agent()` 编排函数 | app 层微形态（后台任务） | 组装 runner → 事件流 tee（Tauri Channel + store sink）→ 状态收敛；后台任务持有 Channel 与 workspace 库句柄收尾，提前 resolve 后 tee 双 sink 不变；将来抽 crate 平移复用不重写 |
| `RunStopRegistry` | 运行中停止句柄注册表 | 寻址键演进为含 root（run id 域内化配套）；其余语义不变 |
