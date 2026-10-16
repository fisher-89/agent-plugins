# desktop-change-state-store Specification (Delta)

## ADDED Requirements

### Requirement: RunNodeRecord 步级时间与 token 列

`RunNodeRecord` SHALL 增四列随行落库：`started_at: i64`（步开始时刻，UTC unix 毫秒）、`ended_at: Option<i64>`（步收口时刻；running 收口在途步 None）、`token_input: Option<u64>` / `token_output: Option<u64>`（本步 turn 增量；仅 WorkerAgent 步有值）。四列 SHALL 均带 `#[serde(default)]`（先例：`session_id` 列）——存量库旧行（无新列编码）解码为 `started_at=0` / `ended_at=None` / token None，零迁移零报错（旧 run 步级时间缺席，前端呈现语义见 desktop-change-flow-view「步级耗时与 token 节点呈现」）。`RunNodeEntry`（finish 载荷内联条目）SHALL 同步增四字段，`RunNodeRecord::new` 直复制；落库时序（每 run 两写、收口整包单事务）与零写放大口径 MUST NOT 改变。既有 `timestamp` 列（落包时刻 = `finished_at` 整包同刻）语义 SHALL 原样保留——步级时间为独立维度，MUST NOT 顶替或复用该列。`list_run_nodes` 读面 SHALL 逐字段透传四列进 `RunNodeStateRecord`（读面与写面同一份字段集）。

#### Scenario: 新行四列随行

- **WHEN** 一次含 WorkerAgent 步与工具步的 run 收口整包落库后查询 run_nodes
- **THEN** 各节点行四列与累积器快照逐字段一致（WorkerAgent 步 token 有值、工具步 None；已收口步 `ended_at` 在案）；同事务内落库、`timestamp` 列仍为整包同刻原语义

#### Scenario: 旧行解码缺省

- **WHEN** 打开含本变更前落库的 `RunNodeRecord` 行（无四列编码）的存量 workspace 库并查询
- **THEN** 解码成功零报错，四列读出 `started_at=0` / `ended_at=None` / token None；无迁移步骤、无手工回填

#### Scenario: 状态回环读写含新列

- **WHEN** 建档 change 推进相位并收口 run 后关闭重开同一 db 文件查询
- **THEN** RunNodeRecord 四列与写入逐字段一致（含瞬步 `started_at == ended_at`、stopped 步 `ended_at` 在案形态），状态查询不依赖任何磁盘文件

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/store/src/model.rs`（`RunNodeRecord`） | 加列 | 四列 `#[serde(default)]`（旧行解码 `0` / `None` / `None` / `None`）；`RunNodeRecord::new` 直复制 `RunNodeEntry` 四字段；`timestamp` 列语义不动 |
| `crates/core/workflow/src/state.rs`（`RunNodeEntry` / `RunNodeStateRecord`） | 载荷与读面随行 | 两结构增四字段；finish 命令内联条目与 port 读面同一份字段集 |
| `crates/infra/store/src/store.rs`（`list_run_nodes`） | 读面投影 | 四列逐字段透传 `RunNodeStateRecord`（node_seq 升序 = emit 序口径不变） |
