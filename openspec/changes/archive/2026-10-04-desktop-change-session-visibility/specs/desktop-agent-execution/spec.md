# desktop-agent-execution Delta

## ADDED Requirements

### Requirement: session_detail 单查面

会话查询契约 SHALL 增加按 session id 的单查面：core `SessionQuery` trait SHALL 增加单查方法（按 id 返回会话行 + 聚合统计 + 轮行清单），infra 实现以 store `find_session` 为底座（store schema 零变更），轮行的运行状态 SHALL 自轮行清单推导（存在 running 轮行即 running，否则取终态）。命令面 SHALL 提供薄包装命令 `session_detail(root, session_id)`（经 builder 注册、bindings 再生成；命令体三件事纪律）。语义约定：不存在 session id SHALL 显式 `Err`（单查语义与清单空态区分，查无此 id 视为调用方错误）；blank root SHALL 返回空结果（与 `agent_sessions` / `agent_session_transcript` 同口径）；单查 SHALL 按 root 解析所属 workspace 库，跨库隔离语义与既有查询命令一致。既有 `agent_sessions` / `agent_session_transcript` 的签名与语义 MUST NOT 收窄或变更。

#### Scenario: 单查返回会话行与轮行

- **WHEN** 以已存在（含 running 轮行与已终态两种形态）的 session id invoke `session_detail`
- **THEN** 返回该会话行、聚合统计与轮行清单，running 判定自轮行推导；结果仅来自该 root 所属 workspace 库

#### Scenario: 查无此 id 显式失败

- **WHEN** 以不存在的 session id invoke `session_detail`
- **THEN** 前端收到 `Err(String)`，无静默空态、无新会话或记录产生

#### Scenario: blank root 空结果

- **WHEN** 以空白 root invoke `session_detail`
- **THEN** 返回空结果不进入库解析链路，与既有查询命令口径一致

### Requirement: 调试页历史来源筛选

Agent 调试页历史区 SHALL 提供会话来源筛选：debug / change / 全部三态，默认 debug（现状不变）。筛选 SHALL 参数化既有取数（`agentSessions(root, source或null, null)`，去硬编码 `'debug'`），切换筛选即重查；取数模型 SHALL 保持显式刷新模式（挂载自动取数 + refresh 重取），MUST NOT 引入轮询、定时刷新或事件订阅。重放链路（点开会话 → 转录重放）SHALL 对三态来源同等可用，复用 `AgentRunHistory` 整套，MUST NOT 为 change 来源另建第二套清单/重放 UI。

#### Scenario: 默认 debug 现状不变

- **WHEN** 打开调试页（未动筛选）
- **THEN** 历史区仅呈现 debug 来源会话，与既有行为一致

#### Scenario: 切换筛选重查

- **WHEN** 筛选切到 change（或全部）
- **THEN** 以对应 source（或不过滤）重查会话清单并呈现 change 会话；再次显式刷新按当前筛选重取，无自动轮询

#### Scenario: 三态来源重放同等可用

- **WHEN** 从筛选结果中点开一条 change 来源会话
- **THEN** 经 `agent_session_transcript` 重放完整转录，与 debug 来源会话同一套重放链路

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/agent/src/port.rs` `SessionQuery` | 单查契约 | 增按 id 单查方法（会话行 + 统计 + 轮行，running 自轮行推导）；`list_sessions` / `transcript` 不动 |
| `crates/infra/agent/src/store_port.rs` | 单查实现 | 以 store `find_session` 为底座（store schema 零变更）；workspace 库隔离 |
| `src/commands/exec/mod.rs` | `session_detail` 命令 | 薄包装三件事纪律；不存在 id → `Err`；blank root → 空结果；builder 注册 + bindings 再生成 |
| `packages/desktop/src/views/agent/hooks/use-agent-run-history.ts` | 来源筛选参数化 | 去硬编码 `'debug'`；三态筛选重查；显式刷新模式不变 |
| Agent 调试页历史区组件 | 筛选 UI | debug / change / 全部，默认 debug；data-testid 挂钩，不以样式类名查询 |
