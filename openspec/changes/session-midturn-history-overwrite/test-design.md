# 测试设计: session-midturn-history-overwrite

> **日期**: 2026-11-17
> **输入**: `proposal.md`（三投影 / 六条线 / AC-1…AC-12 / R1–R10）、`design.md`（DD1–DD11 / 变更清单 13 路径 / 公共函数 API 表 / 内部契约面表 / 类型定义表 / 零 diff 登记）、`tasks.md`（阶段交接的测试面登记）
> **测试框架**: Rust 侧 `#[test]` sidecar（`*_test.rs`，同 crate 内私有可见，零新文件、零 `#[cfg(test)] mod` 声明）；前端 `vite-plus/test` + `@testing-library/react`（`*.test.ts(x)` 共置 sidecar，jsdom 环境按 `// @vitest-environment jsdom` 标注）
> **本相位只设计测试**：用例与 Mock 策略在此定稿，门禁执行（`vp check` / `vp test` / `knip` / `client:check` / `cargo nextest`）归 test-execution 相位。

---

## 验收范围

<!-- 逐条映射 proposal.md 的 AC-1…AC-12；`被测文件或模块` = 承载用例的测试文件（单值或多值并列）。
     跨模块组合用例挂靠链路发起方 / 最上层调用方模块的 per-file 章节（本仓无独立集成测试层）：
     「store 标定 → session_detail / agent_answer」组合行挂靠在 `src-tauri/src/commands/exec/mod.rs` 章节。 -->

| AC ID | 验收条件 | 被测文件或模块 |
|--------|---------|----------|
| AC-1 | `AgentRunStatus` 增 `Interrupted`（末位、线格式 `"interrupted"`）；既有四值线值逐字不变（值域侧） | `packages/desktop/src-tauri/crates/core/agent/src/runner_test.rs` |
| AC-1 | （同上——出线侧：`TurnSummary.status` 五值线格式、旧四值不变；doc 同批同步的生产面证据） | `packages/desktop/src-tauri/crates/core/agent/src/session_test.rs` |
| AC-1 | （同上——`bindings.ts` / `bindings.zod.ts` 再生成后 TS union 与 zod literal 含该值） | —（见不可测试项 1） |
| AC-2 | 旧行零迁移：`interrupted` 值域往返（写读）保真；中断值引入前的四值解码不变；`#[native_model(id = 2, version = 4)]` 与两向 `From` 零改动 | `packages/desktop/src-tauri/crates/infra/store/src/model_test.rs` |
| AC-3 | `calibrate_interrupted_agent_turns` 只翻 `AgentRunRecord.status == Running`：状态 + `finished_at` + 记因定式；统计三字段不虚构；幂等；已终态零改写；`RunRecord` 与转录零触碰 | `packages/desktop/src-tauri/crates/infra/store/src/store_test.rs` |
| AC-4 | 标定触发点：`open_workspace` 与既有 change run 标定同址并列（共享一次取时）；无残留时不产生写事务 | `packages/desktop/src-tauri/crates/infra/store/src/store_test.rs` |
| AC-4 | （同上——判据的命令面投影：标定后 `session_detail` 直接返 `interrupted` 轮行；前端消费面 `running` 不因 `interrupted` 派生） | `packages/desktop/src-tauri/src/commands/exec/mod_test.rs`、`packages/desktop/src/hooks/use-agent-session.test.ts` |
| AC-5 | `worker_settle` 对 `Interrupted` 显式落臂（`StepSettle::failed` + `Terminal::failed`，文案「会话被中断」，步 detail 与 run 终态同文）；五变体全显式、零 `_` 兜底；`Terminal` / `StepSettle` / `RunFault` 变体面零扩展 | `packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs` |
| AC-5 | （同上——既有明文「相位内步骤系统异常」分类枚举的整段改写留痕） | —（见不可测试项 3） |
| AC-6 | 症状 1 回归：残留 `running` 行在前、实时流属其后新轮时，播种按首个事件 `turnId` 选轮，增量到达后新轮已落库部件仍在；残留轮零预投、零串扰 | `packages/desktop/src/lib/agent-transport.test.ts` |
| AC-7 | 症状 2 呈现：终结轮内未配对工具部件的 `TraceLine` 不呈 `running`（无 shimmer 载体、无 spinner）；`running` 轮内同类部件仍呈运行态 | `packages/desktop/src/components/agent/tool-cards.test.tsx` |
| AC-8 | 症状 3 判据：无 `pendingToolCallId` 时活卡读该轮 `status === 'running'`；终结轮的无结果 ask 卡零应答入口、`running` 轮照常活卡；有 `pendingToolCallId` 时精确对账不变 | `packages/desktop/src/components/agent/agent-messages.test.tsx` |
| AC-9 | 状态行与可见性：空内容 `interrupted` 轮可见并呈「因应用重启中断」（`data-status="interrupted"`）；顶部失败横幅 MUST NOT 出现；空内容 `completed` 轮仍剔除 | `packages/desktop/src/components/agent/agent-messages.test.tsx`、`packages/desktop/src/lib/agent-virtual.test.ts`、`packages/desktop/src/components/agent/agent-error.test.ts` |
| AC-10 | 列表文案：会话历史列表对 `interrupted` 轮行呈「已中断」（不落 `failed` 兜底）；四值词面不变；`data-status` 钩子直出 | `packages/desktop/src/views/agent/components/agent-run-history.test.tsx` |
| AC-11 | 后端拒绝面可归因：标定后的 `agent_answer` 返回显式 `Err`（「当前无等待中的 ask」）；`agent_stop` miss 仍幂等 `Ok` | `packages/desktop/src-tauri/src/commands/exec/mod_test.rs` |
| AC-12 | 版本与门禁：`packages/desktop` version 0.6.47 → 0.6.48（`src-tauri/Cargo.toml` 不随动） | —（见不可测试项 2） |
| AC-12 | （同上——`vp check` / `vp test` / `knip` / `client:check` / rust nextest 全绿、基线不回退） | —（见不可测试项 4） |

### 源文件 → 测试文件映射（design 变更清单全覆盖）

<!-- design.md「变更清单 - 修改文件」13 个路径（0 新增文件）逐行落位；末两行为「零 diff 登记」表在册、
     由用例固化默认的既有文件（生产面零改动）。删除文件不纳入被测范围（本变更无删除）。 -->

| 源文件（相对项目根） | 测试文件 | 落位 |
|--------|---------|------|
| `packages/desktop/src-tauri/crates/core/agent/src/runner.rs` | `.../core/agent/src/runner_test.rs` | 单元测试章节 1 |
| `packages/desktop/src-tauri/crates/core/agent/src/session.rs` | `.../core/agent/src/session_test.rs` | 单元测试章节 2 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` | `.../core/orchestration/src/walker_test.rs` | 单元测试章节 3 |
| `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | `.../infra/store/src/store_test.rs` | 单元测试章节 4 |
| `packages/desktop/src-tauri/crates/infra/store/src/model.rs` | `.../infra/store/src/model_test.rs` | 单元测试章节 5 |
| `packages/desktop/src-tauri/src/commands/exec/mod.rs` | `.../src/commands/exec/mod_test.rs` | 单元测试章节 6（跨模块组合用例挂靠） |
| `packages/desktop/src/types/generated/bindings.ts` | — | 不可测试项 1（生成物） |
| `packages/desktop/src/types/generated/bindings.zod.ts` | — | 不可测试项 1（生成物） |
| `packages/desktop/src/lib/agent-transport.ts` | `.../src/lib/agent-transport.test.ts` | 单元测试章节 7 |
| `packages/desktop/src/lib/agent-virtual.ts` | `.../src/lib/agent-virtual.test.ts` | 单元测试章节 8 |
| `packages/desktop/src/components/agent/agent-messages.tsx` | `.../src/components/agent/agent-messages.test.tsx` | 单元测试章节 9 |
| `packages/desktop/src/components/agent/tool-cards.tsx` | `.../src/components/agent/tool-cards.test.tsx` | 单元测试章节 10 |
| `packages/desktop/src/views/agent/components/agent-run-history.tsx` | `.../src/views/agent/components/agent-run-history.test.tsx` | 单元测试章节 13 |
| `packages/desktop/package.json` | — | 不可测试项 2（版本行） |
| `packages/desktop/src/hooks/use-agent-session.ts`（零 diff 固化） | `.../src/hooks/use-agent-session.test.ts` | 单元测试章节 11 |
| `packages/desktop/src/components/agent/agent-error.ts`（零 diff 固化） | `.../src/components/agent/agent-error.test.ts` | 单元测试章节 12 |

---

## 单元测试

<!-- 待测功能条目来源：design.md「公共函数 / API」表（calibrate_interrupted_agent_turns / open_workspace /
     filterConversation / reconnectToStream）+ 同文档「内部契约面」表（worker_settle / isLiveAsk / TurnStatusLine /
     PartView / ToolPartView / createPendingSeed / unpairedRunning / statusLabel）+「类型定义」表
     （AgentRunStatus / AgentRunRecord.status / TurnSummary.status / AgentToolCardProps / PendingSeed / EnvelopeStream.expectSeed）。
     三表均为 design 明文，未从源码导出声明反推；私有单点（isLiveAsk / statusLabel / unpairedRunning）经组件渲染驱动，
     不为测试新增导出（knip 纪律）。零改动面（use-agent-session / agent-error）由「零 diff 登记」表登记、用例固化默认。 -->

### packages/desktop/src-tauri/crates/core/agent/src/runner.rs -> packages/desktop/src-tauri/crates/core/agent/src/runner_test.rs

#### 待测功能

- `AgentRunStatus`: 五变体值域 `Running` / `Completed` / `Failed` / `Stopped` / `Interrupted`（**末位追加**）；`#[serde(rename_all = "camelCase")]` 线值 `"interrupted"`，既有四值线值逐字不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `AgentRunStatus` 线值 | 正向 | 五变体逐一 `serde_json` 序列化 → 线值 `running` / `completed` / `failed` / `stopped` / `interrupted` 逐字；反序列化回原值（逐值 round-trip 相等） | 新增 |
| `AgentRunStatus` 线值 | 正向 | 枚举扩值不改既有 rename 结果：四值线串逐字比对（AC-2 的出线半边——旧转录 / 旧库行读面不因新值位移） | 新增 |
| `AgentRunStatus` 变体序 | 边界 | 落库 tag 序由变体声明序决定：`agent::AgentRunStatus::{Running, Completed, Failed, Stopped, Interrupted} as u32` 逐值等于 `0..=4`（末位追加的程序化判据，`Stopped` 恒为 3） | 新增 |
| `AgentRunStatus` 值域 | 异常 | 枚举**无** `#[serde(other)]` 之类的兜底承接：越过值域的线值（`"paused"` / `"interrupted2"` / `""`）反序列化显式 `Err`，不静默折进任一既有变体 | 新增 |
| `AgentRunStatus` 定义面 | 边界 | 源码自证：`AgentRunStatus` 的 derive 行仍含 `specta::Type`（值域经 specta 出线到生成物），`Interrupted` 声明位于枚举闭合花括号前（末位判据的第二个观测面） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 零 mock：纯值域类型 + 真实 `serde_json` 编解码（agent crate 无 bincode 依赖，落库 tag 面用变体序判据 + store 侧往返互补） | 全部用例 |

### packages/desktop/src-tauri/crates/core/agent/src/session.rs -> packages/desktop/src-tauri/crates/core/agent/src/session_test.rs

#### 待测功能

- `TurnSummary.status`: 出线文档「running \| completed \| failed \| stopped \| interrupted」（design「范围补录」：该 doc 直接生成到 `bindings.ts` 的 `TurnSummary.status` 注释）；线格式五值随 `AgentRunStatus` 透出，结构面零改动

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `TurnSummary.status` 线格式 | 正向 | 五值逐一 `serde_json` 序列化 → `status` 键为 `running` / `completed` / `failed` / `stopped` / `interrupted` 逐字，round-trip 逐字段相等（既有四值用例扩值） | 修改 |
| `TurnSummary.status` 线格式 | 异常 | 越过线值域的状态串（`"paused"`）反序列化显式 `Err`（轮行 DTO 不接受词表外状态） | 新增 |
| `TurnSummary.status` 线格式 | 边界 | `interrupted` 形态的完整字段面：`finishedAt` 为数值（标定时刻）、`error` 为标定记因、`numTurns` / `costUsd` / `durationMs` 为 `null`（不虚构统计）——`running` / 终态两形态既有断言逐字不动 | 新增 |
| `TurnSummary` doc 同步 | 边界 | 源码自证：`TurnSummary.status` 的 doc 取值清单含 `interrupted`（该 doc 是 `bindings.ts` 注释源，design DD11 要求与枚举同批，防「union 五值 / 注释四值」自相矛盾） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 零 mock：DTO 值类型 + 真实 `serde_json` 编解码；源码自证经 `env!("CARGO_MANIFEST_DIR")` 读本 crate 源文件（既有装置） | 全部用例 |

### packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs

#### 待测功能

- `worker_settle(outcome: &WorkerTurnOutcome) -> (StepSettle, Option<Terminal>)`: 五变体全显式分类单点，新增 `AgentRunStatus::Interrupted` 臂 → `StepSettle::failed` + `Terminal::failed`，文案「会话被中断」（步 detail 与 run 终态同文）；归 **failed 侧**，与 `Stopped` 的 stopped 侧归属不可互换；`match` 仍穷尽、零 `_` 兜底
- `worker_settle_probe(outcome) -> WorkerSettleProbeView`: 七字段探针面零扩展，新臂经既有直调面自动可测（`step_status` / `detail` / `terminal_status` / `terminal_reason` / `session_id` / 两 token）
- `worker_settle` / `worker_settle_probe` doc 字面：变体枚举补 `Interrupted` 与归属，「四臂」→「五臂」

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `worker_settle`（探针） | 正向 | `Interrupted` + 空转录：`step_status = Some(Failed)`、`detail = Some("会话被中断")`（逐字）、`terminal_status = Some(Failed)`、`terminal_reason == detail`（步与终态同文）、`session_id` 随行、`token_input` / `token_output` 皆 `None`（空转录不虚构 token） | 新增 |
| `worker_settle`（探针） | 正向 | 与 `Stopped` 对拍可辨：`Stopped` → `ChangeStepStatus::Stopped` + `ChangeRunStatus::Stopped` + 「会话被终止」；`Interrupted` → failed 侧 + 「会话被中断」——两侧归属与文案 MUST NOT 互换（两段断言同用例内并列） | 新增 |
| `worker_settle`（探针） | 正向 | 既有四段断言逐字不动（保留）：`Completed` → passed 无终态 / `Failed` 携因与成因缺席两形态与超长折叠 / `Stopped` → stopped 侧 / `Running` 漂移 → failed「会话收口缺终态」，扩五态后逐行相等 | 保留 |
| `worker_settle`（探针） | 异常 | `Interrupted` + transcript 携 `api_error` 成因：文案仍为固定「会话被中断」，不被 `failure_reason` 覆盖（与 `Failed` 臂的成因分支结构性不同——MUST NOT 复用「会话失败收敛: …」前缀） | 新增 |
| `worker_settle`（探针） | 异常 | `Interrupted` 判据只看 status：空 session_id 原样随行；1000 条无关事件（`messageDelta`）在场时文案与终态不变；转录内 `TurnDone` usage 仍按既有 `worker_tokens` 求和（两 token 与 `Interrupted` 文案互不影响） | 新增 |
| `worker_settle` 结构 | 边界 | 源码自证（`byte_window` 读 `walker.rs`）：`fn worker_settle` 函数体内含 `AgentRunStatus::Interrupted =>` 臂且该臂体含 `StepSettle::failed` 与 `Terminal::failed`；函数体内零 `_ =>` 兜底臂；臂位置紧邻 `AgentRunStatus::Stopped =>` 臂之后（对拍位） | 新增 |
| `worker_settle` doc | 边界 | 源码自证：`worker_settle` doc 的变体枚举含 `Interrupted` 与「会话被中断」归属；`worker_settle_probe` doc 为「五臂」（字面与实现同步，零行为变化） | 新增 |
| 变体面零扩展 | 边界 | 源码自证：`Terminal::interrupted` / `StepSettle::interrupted` 全仓零命中；`RunFault` 变体清单无新增（`port.rs` 与 `state.rs` 的变体面零扩展——AC-5 的「零扩展」半边） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（`ChangeFlowControl` / `emit_node` 快照面） | 零 mock：`worker_settle_probe` 直调 + 真实控制面无状态装置（既有探针装置沿用）；`WorkerTurnOutcome` 由既有 `worker_outcome` 辅助构造 | 全部终态映射用例 |
| worker 端口 / 会话执行（`WorkerAgentPort`） | 不参与：分类单点在 `worker_settle`，不经 `walk_run` 端到端（`Interrupted` 臂生产路径不可达——design DD4） | — |
| 源码文本（`walker.rs` / `port.rs` / `state.rs`） | 进程边界（文件系统）：`std::fs::read_to_string` + `env!("CARGO_MANIFEST_DIR")` 真实读取（既有源码自证装置），零替身 | 结构 / doc / 变体面自证用例 |

### packages/desktop/src-tauri/crates/infra/store/src/store.rs -> packages/desktop/src-tauri/crates/infra/store/src/store_test.rs

#### 待测功能

- `calibrate_interrupted_agent_turns(now: i64) -> Result<Vec<i64>, StoreError>`: 读扫 `read_all::<AgentRunRecord>` → 过滤 `status == Running` → 空则 `Ok(vec![])`（**不开写事务**）→ 否则单 `rw_transaction` 逐行写 `status = Interrupted` + `finished_at = Some(now)` + `error = Some("重启标定：桌面进程中断，会话轮客观已终止")`，`session_id` / `started_at` / 统计三字段原样带过 → `commit` → 返回被标定轮 `id` 清单
- `Store::open_workspace(path: &Path)`: 签名与既有语义不变；函数体内 `let now = now_millis();` 后并列两族标定（`calibrate_interrupted_runs(now)` + `calibrate_interrupted_agent_turns(now)`，共享一次取时，`let _ =` 吞错沿用）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `calibrate_interrupted_agent_turns` | 正向 | 会话 + `begin_agent_turn`（未收口，`running`）后 `calibrate(9000)`：返回 `vec![turn_id]`；`list_sessions` 得该轮 `status = Interrupted`、`finished_at = Some(9000)`、`started_at` 原值、`session_id` 原值、`error` 含「重启标定」与「会话轮客观已终止」、`num_turns` / `cost_usd` / `duration_ms` 仍 `None`（统计三字段不虚构） | 新增 |
| `calibrate_interrupted_agent_turns` | 正向 | 多会话多残留（≥2 条 `running` 行，分属两 session）：全部标定、返回清单按 turn id 升序稳定；两会话各自轮行均翻 `Interrupted` | 新增 |
| `calibrate_interrupted_agent_turns` | 正向 | 混合库：`completed` / `failed` / `stopped` 三行 + 一条 `running` → 只翻 `running`；三个已终态行逐字段（status / finished_at / error / 统计）零改写 | 新增 |
| `calibrate_interrupted_agent_turns` | 异常 | 不误伤 change run 域与转录：库内同时存在 `running` 的 change run（`start_change_run` 未收口）与密封转录 → 调用后 `list_change_runs` 该 run 仍 `Running`、change 建档 `active_phase` 原值不变、`list_session_events` 条数与内容逐条相等（R1 的写面隔离断言） | 新增 |
| `calibrate_interrupted_agent_turns` | 边界 | 零残留（全部已终态）：返回空 `Vec`；既有轮行全字段（含 `finished_at` / `error` / 统计）与 `SessionRecord.updated_at` 原值不变（零写事务的可观测面） | 新增 |
| `calibrate_interrupted_agent_turns` | 边界 | 幂等：`calibrate(9000)` → `calibrate(9500)`，第二次返回空 `Vec`；`finished_at` 仍 `Some(9000)`、`error` 未被二次改写为 9500 语境（已 `interrupted` 行不被二次改写） | 新增 |
| `calibrate_interrupted_agent_turns` | 边界 | 孤儿轮行（`session_id = None`）+ `running` → 同样被标定并出现在返回清单（扫描面是 `AgentRunRecord` 表，不按会话归属圈定） | 新增 |
| `calibrate_interrupted_agent_turns` | 边界 | `now` 极值两形态（`0` / `i64::MAX`）→ `finished_at` 逐字等于入参（显式实参注入，不读系统钟、不钳位） | 新增 |
| `Store::open_workspace` | 正向 | 打开点内嵌：首轮打开后种残留（`begin_agent_turn` 未收口）→ drop 句柄重开同一 db 文件 → 该轮已 `Interrupted`、`finished_at` 在位、记因在位（标定挂点与既有 change run 标定同址） | 新增 |
| `Store::open_workspace` | 正向 | 两族共享一次取时：库内同时有残留 change run 与残留 agent 轮 → 重开后两族 `finished_at` 相等（同一 `now_millis()`）、change run 亦翻 `interrupted`（既有面零回归） | 新增 |
| `Store::open_workspace` | 边界 | 无残留库重开：零标定副作用，重开后 `begin_agent_turn` / `finish_agent_turn` 正常（标定不阻断正常写面），既有轮行逐字段不变 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| workspace 库（redb 数据文件） | 进程边界：tempdir 真库 `Store::open_workspace` 装置（`Env` / `StoresEnv` RAII，既有装置沿用）真实组合，零 store / db mock | 全部用例 |
| 系统时钟 | 不 mock：标定时刻一律以显式 `i64` 实参注入（`calibrate(now)`）；`open_workspace` 内嵌面只断言两族 `finished_at` 相等（不绑定墙钟值） | 全部用例 |

### packages/desktop/src-tauri/crates/infra/store/src/model.rs -> packages/desktop/src-tauri/crates/infra/store/src/model_test.rs

#### 待测功能

- `AgentRunRecord.status`（`AgentRunStatus` 五值落库面）: `interrupted` 经 `native_model` 编解码往返保真；既有四值往返不变；`#[native_model(id = 2, version = 4)]` / 字段面 / 两向 `From<AgentRunRecordV3>` 零改动

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `AgentRunRecord` v4 往返 | 正向 | `run_record_v4()` 改 `status = Interrupted` → `native_model::encode` / `decode` 往返逐字段相等，`version` 仍为 `4`（零版本递增、零迁移），`error` 携标定记因、统计随行 | 新增 |
| `AgentRunRecord` v4 往返 | 正向 | 既有四值（`Completed` / `Failed` / `Stopped` / `Running`）逐一往返逐字段相等（既有断言保留，扩为五值循环后逐值绿） | 保留 |
| `AgentRunRecord` 落库 tag | 边界 | 末位追加的字节面观测：同一 fixture 的 `Interrupted` 与 `Stopped` 编码字节可辨（不相等），且各自解码回自身的 `status`（新值不覆盖既有 tag、旧值不位移） | 新增 |
| `AgentRunRecord` 线格式 | 边界 | `serde_json::to_value` 的 `status` 为 `"interrupted"` 逐字；`sessionId` 为 `None` 的孤儿轮行 + `Interrupted` 组合往返逐字段相等（字段面零改动） | 新增 |
| `AgentRunRecord` 升级链 | 异常 | 存量 v3 载荷经版本机制升级为 v4（既有用例保留）：`status` 逐字保留、`session_id` 置 `None`、退役字段不入——枚举扩值不改变升级链行为 | 保留 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 零 mock：`native_model` / `serde_json` 真实内存编解码（不落库、不触文件），时间戳与统计以显式值注入（与钟面无关） | 全部用例 |

### packages/desktop/src-tauri/src/commands/exec/mod.rs -> packages/desktop/src-tauri/src/commands/exec/mod_test.rs

#### 待测功能

<!-- design.md「公共函数 / API」表未声明该文件（六命令签名零 diff、「零 diff 登记」表在册）；
     本章节按「跨模块组合用例挂靠链路最上层调用方」承载 store 标定 → 命令面查询 / 应答的组合行（AC-4 / AC-11）。 -->

- `session_detail(stores, root, session_id)`: 标定后的会话单查直接返 `interrupted` 轮行（`TurnSummary.status` 出线，三件套形状与顶层零状态字段不变）
- `agent_answer(registry, root, session_id, text)`: 已标定会话（三张内存注册表为空）→ 既有 miss 臂 `Err`「当前无等待中的 ask」；不因 `interrupted` 新增分支

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `session_detail` × 标定（组合） | 正向 | seed 会话 + `begin_agent_turn`（未收口）+ 密封转录（含未配对 `toolUse`）→ 直调 `store.calibrate_interrupted_agent_turns(now)` → `session_detail` 返回 `turns[0].status = Interrupted`、`finishedAt = Some(now)`、`error` 为标定记因；出线键恒 `row` / `stats` / `turns` 三件（无顶层 `status`） | 新增 |
| `session_detail` × 标定（组合） | 正向 | 同一库内 `agent_sessions`（清单面）与 `session_detail`（单查面）对同一会话逐字段全等，末轮状态同为 `interrupted`（聚合形状单一来源，标定后两读面同口径） | 新增 |
| `agent_answer`（组合） | 异常 | 标定后对同一 `session_id` 调 `agent_answer`：`Err` 文案逐字「当前无等待中的 ask」（既有 miss 臂；注册表零触点、零新分支）；同会话 `agent_stop` miss 仍 `Ok`（行为不变——AC-11 两命令并列表述） | 新增 |
| `session_detail` / `agent_answer` | 边界 | 未标定的 `running` 轮（不调用标定）：`session_detail` 仍返 `Running`（`interrupted` 只有标定一个写入点）、`agent_answer` 同样 miss `Err`——证明命令面零新增 `interrupted` 分支 | 新增 |
| `session_detail` | 边界 | 多轮混合（`completed` 轮 + `interrupted` 轮）：`turns` 按发起顺序、状态逐值（既有轮零改写），`stats.turnCount` 不受标定影响 | 新增 |
| `session_detail` | 边界 | 跨 root 隔离：会话与标定落 root A 库，经 root B 单查 → `Err`（既有 `for_root` 寻址语义保留） | 保留 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| tauri app（运行环境边界） | 既有 `tauri::test::mock_app()` + `app.manage(...)` 管理真实 `WorkspaceStores` 与空注册表，直调命令函数（零 tap / 零 mock ipc） | 全部用例 |
| workspace 库（数据文件） | 进程边界：tempdir 双根（`data_dir` + `ws_root`）+ 既有 `seed_sdk_session` / `append_session_events` 真实落库 fixture | 全部用例 |
| `AskRegistry` / `StopRegistry` / `SessionStreamRegistry` | 不 mock：真实内存注册表 `Arc`（与 `main.rs` 同型托管；标定后为空表 ⇒ 自然命中 miss 臂） | `agent_answer` / `agent_stop` 用例 |

### packages/desktop/src/lib/agent-transport.ts -> packages/desktop/src/lib/agent-transport.test.ts

#### 待测功能

- `TauriAgentTransport.reconnectToStream(options)`: 签名与返回语义不变（`Promise<ReadableStream<AgentUIMessageChunk> | null>`）；状态门保留（`detail.turns.some(t => t.status === 'running')`，**仅此一处**消费 `detail.turns`），选轮键退役——播种键由首个实时事件 `turnId` 自证
- `createPendingSeed(state, controllerOf)`（`PendingSeed` / `EnvelopeStream.expectSeed` 契约）: `expect(events: readonly AgentEvent[])` 登记已装载镜像；`seedIfMatching` 首个 `event` 信封即**消费登记**（先清空）→ 按 `message.event.turnId` 过滤镜像 → 逐条预投；`turnId` 为 `null` / `undefined` 时消费登记且不投

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `reconnectToStream` 播种（AC-6 核心回归） | 正向 | 残留 `running` 行在前（`session_detail` = `turn 11 running` + `turn 13 running`）+ `observedEvents` 镜像含 turn 11 与 turn 13 的密封事件（`seq <= sinceSeq`）+ `Channel` 首投 turn 13 的 `messageDelta` → chunk 序列先按 turn 13 既有事件重建（`start` 的 `messageId = turn-13` + 已落库部件）再投增量；turn 11 的部件文本零出现（当前实现必红：按行选轮取到 11 → 首事件 13 不匹配 → 零播种） | 新增 |
| `reconnectToStream` 播种（AC-6 reducer 面） | 正向 | 上例 chunk 序列经真实 `useChat` 归约：该轮恰一条 assistant（`id = turn-13`），文本 = 已落库正文 + 增量（既有部件不被冲掉，症状 1 的归约侧判据） | 新增 |
| `reconnectToStream` 播种 | 正向 | 单 `running` 行与实时轮一致的既有场景零回归：播种照旧（同 turnId 的密封事件按序预投），`session_detail` 恰 1 次、`agent_session_watch` 参数 `{root, sessionId, sinceSeq}` 逐字不变 | 保留 |
| `reconnectToStream` 播种 | 异常 | 首个实时事件 `turnId` 为 `null`：登记被消费但零预投；其后同轮事件同样零预投（不推迟、不重复），增量照常投递、`start` 恰一次 | 新增 |
| `reconnectToStream` 播种 | 异常 | 首个信封为 `record`（非 `event`）：登记不被消费、零预投；其后首个 `event` 到达时照常播种（消费时机由 event 决定） | 新增 |
| `reconnectToStream` 播种 | 边界 | 镜像为空数组：首个事件即消费、零预投，流从增量起步（零空洞、零重复——`start` 恰一次且 `messageId` 取该事件轮次） | 新增 |
| `reconnectToStream` 播种 | 边界 | 镜像全属他轮（无与实时轮同 `turnId` 的事件）：零预投、零串扰（本轮流 `start` 的 `messageId` 为实时轮，他轮部件零出现） | 新增 |
| `reconnectToStream` 播种 | 边界 | 镜像含同 `turnId` 多事件（含被外层过滤掉的 `messageDelta` 与超 `sinceSeq` 事件）：预投只含密封事件、按 `seq` 升序保序，同一部件不重复（部件 id / 文本唯一） | 新增 |
| `reconnectToStream` 播种 | 边界 | 只投一次：登记消费后再到第二个 `event` → 零第二次预投（部件文本只出现一次、气泡数不倍增） | 新增 |
| `reconnectToStream` 状态门 | 异常 | `detail.turns` 无 `running`（全 `completed` / 全 `interrupted`）→ 返回 `null`、`agent_session_watch` 零调用（状态门保留，`interrupted` 不误开订阅） | 保留 |
| `reconnectToStream` 异常面 | 异常 | `agent_session_watch` reject → 流以错误收尾；`abortSignal` 调用前 / 订阅中 abort → 流关闭、后续信封零 chunk、命令 resolve 迟到不重复关闭（既有面零回归） | 保留 |
| `sendMessages` / 两出口 | 边界 | 发起流不受播种改派影响：`agent_start` 参数 8 键逐字、事件转 chunk 保序、`Record` 收口关流；两出口 `Record` 收口 chunk 组逐条相等（既有断言不让步） | 保留 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `@tauri-apps/api/core` 的 `invoke` / `Channel`（进程边界 IPC） | 既有 `vi.hoisted` + `vi.mock`：`invokeMock` 按命令名分发（`session_detail` 可编程 turns 清单 / reject，`agent_session_watch` 可挂起或 reject 并记录参数），`ChannelMock` 与真实 `Channel` 同构（私有字段投递）供直投信封；`beforeEach` 复位实例表 | 全部用例 |
| `observedEvents` 镜像来源（注入依赖） | 不 mock 模块：经构造函数 options 注入纯函数返回固定事件数组（`AgentEvent` 显式入参），即 spec 里「已装载镜像」的真实形态 | 播种相关用例 |
| `ReadableStream` / `AbortController` / `useChat` reducer | 不 mock：真实 `ReadableStream` + 既有 `drain()` 消费、真实 `AbortController`、既有 `replayTransport` / `reducedBy` 归约装置 | 全部用例 |
| 内部模块（`createPendingSeed` / `frameChunks` / `eventToChunk`） | 不 mock：真实组合（播种与簿记同源，经流 chunk 序列与归约结果断言） | 全部用例 |

### packages/desktop/src/lib/agent-virtual.ts -> packages/desktop/src/lib/agent-virtual.test.ts

#### 待测功能

- `filterConversation(all: AgentUIMessage[]) -> AgentUIMessage[]`: 空内容轮可见性白名单由 `failed` / `stopped` 扩为 `failed` / `stopped` / `interrupted`（签名与判据单点不变；虚拟项数与吸附几何随之自动跟随）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `filterConversation` | 正向 | 空内容 assistant + `record.status = 'interrupted'` → 保留一条、`metadata.record.status` 原样可读、`parts` 为空（与 failed / stopped 同列） | 新增 |
| `filterConversation` | 正向 | 三值白名单参数化（`failed` / `stopped` / `interrupted`）→ 各保留一条；有内容 assistant + `interrupted` 轮行 → 保留且不追加判定 | 修改 |
| `filterConversation` | 异常 | 空内容 + `completed` → 仍剔除（既有断言逐字不动）；空内容 + `running` → 仍剔除；轮行缺席（`record` 为 `null`）→ 剔除 | 保留 |
| `filterConversation` | 异常 | 越过类型的非法 status（`'paused'` 脏值）→ 同 `completed` 剔除、不抛错（白名单是包含判据，扩值不引入兜底放行） | 保留 |
| `filterConversation` | 边界 | 1000 条混合列表（system / 有内容 / 空内容 completed / 空内容 failed / 空内容 interrupted）→ 输出条数与顺序严格按判据（扩白名单后项数 = 原项数 + interrupted 条数） | 修改 |
| `filterConversation` | 边界 | 空数组 → 空数组；单元素三形态（单 interrupted 保留 / 单 completed 剔除 / 单 system 剔除）；入参 `undefined` → 抛 `TypeError`（既有越签名脏值断言保留） | 保留 |
| `hasVisibleParts` × 白名单 | 边界 | 只有 `data-*` / `step-start` 部件的 `interrupted` 轮 → `hasVisibleParts` 为假但白名单放行 ⇒ 保留（可见性判据单一来源，两函数口径不冲突） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 零 mock：纯函数 + 真实折叠（`foldSessionMessages`）构造 fixture，零 IO / 零 React | 全部用例 |

### packages/desktop/src/components/agent/agent-messages.tsx -> packages/desktop/src/components/agent/agent-messages.test.tsx

#### 待测功能

- `isLiveAsk(part, pendingToolCallId, turnRunning)`: 三态矩阵——`null` → `false`；字符串 → `part.toolCallId` 精确相等；`undefined` → `part.state === 'input-available' && turnRunning !== false`（结果缺席只作必要条件，判据主体是该轮状态）。私有单点，经 `AgentMessages` 渲染驱动
- `TurnStatusLine({ message, record })`: 增 `interrupted` 支（`data-testid="turn-status-line"` + `data-status="interrupted"` + 内层 `turn-status-line-interrupted`，文案「因应用重启中断」，`text-muted-foreground`，**不读** `record.error` / `noticeError`）
- `PartView` / `ToolPartView`: 新增可选 `turnRunning?: boolean` 并原样下行到特化卡；`ChatBubbleBase` 内算 `turnRunning = record === null ? true : record.status === 'running'`，只向下穿布尔

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `TurnStatusLine` | 正向 | 空 `parts` + `record('interrupted', error: 标定记因)` → 状态行在场、`data-status="interrupted"`、内层文案「因应用重启中断」；**零** `turn-status-line-raw`、零「Agent 运行失败」标题（记因不展开、不泄漏成第二个失败）；气泡内零文本块 | 新增 |
| `TurnStatusLine` | 正向 | 既有两支零回归（保留）：`failed` 呈通用标题 + 可展开原文 + 四形态原文来源；`stopped` 呈「已停止」且零 details；`parts` 非空时两态均不叠加状态行 | 保留 |
| `isLiveAsk` 第三态 | 正向 | 未提供 `pendingToolCallId`（`undefined`）+ 轮 `record.status = 'running'` → ask 卡仍为活卡：提交入口与自由文本输入在场，提交经 `onAnswer` 上行原文（活卡主路径不被缺省打死） | 新增 |
| `isLiveAsk` 第三态 | 正向 | 未提供 `pendingToolCallId` + 轮已终结（`interrupted` / `failed` / `stopped` 三值参数化）→ ask 卡静态：选项只读、零输入、零提交；点选项不产生 `onAnswer` 调用 | 新增 |
| `isLiveAsk` 第三态 | 异常 | 轮行缺席（`metadata.record === null`）+ 未提供 `pendingToolCallId` → 按运行中处理（活卡），与改动前逐条等价（流式期无 `record` 的既有主路径零回归） | 保留 |
| `isLiveAsk` 精确对账 | 异常 | 提供字符串 `pendingToolCallId` 时不看轮状态：终结轮内 `toolCallId` 相等者仍为活卡、不匹配者全静态（精确对账语义 MUST NOT 被轮状态改写）；`pendingToolCallId = null` → 全静态（既有保留） | 修改 |
| 轮状态下行 | 边界 | 同一 `messages` 引用重渲染 → 气泡节点数与 ask 卡状态不变（下行位只穿原语、无副作用）；`ChatBubble` props 面仍只有 `message` / `askAnswer`（memo 判据逐字不动，`agent-render-memo.test.tsx` 既有断言不让步） | 保留 |
| 轮状态下行 | 边界 | 既有四变体气泡 / 工具对卡 / 折叠 / 虚拟窗口断言在 `turnRunning` 缺省（`undefined`）下逐条相等（四卡旧调用点零改动） | 保留 |
| 轮状态下行 | 异常 | `record.status` 越过类型的脏值（`'paused'`）→ 不抛错、状态行缺席、ask 卡按「非 running」呈静态（`turnRunning` 缺省逻辑不反转为运行中） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `@/components/agent/tool-cards` 注册表键 `BUILD_IN_TOOL_CARDS['AskUserQuestion']` | 既有零行为探针：`vi.mock` + `importOriginal` 保留全部真实实现，仅包一层记录 `askAnswer` / `turnRunning` props 后原样渲染真实卡（注册表为组件注入面；不为测试开 `export`） | ask 判活矩阵 / 下行位断言 |
| jsdom 运行环境（`ResizeObserver` / `scrollTo` / `scrollHeight`） | 既有替身与实例级滚动桩（环境边界），组件树不 mock | 虚拟窗口与贴底相关用例 |
| 内部模块（`filterConversation` / `foldSessionMessages` / `ToolPartView` / `TurnStatusLine`） | 不 mock：真实组合，经渲染树断言（私有单点不导出） | 全部用例 |

### packages/desktop/src/components/agent/tool-cards.tsx -> packages/desktop/src/components/agent/tool-cards.test.tsx

#### 待测功能

- `unpairedRunning(output, turnRunning)`: 新增模块内单点判据，返回 `output === null && turnRunning !== false`；四处 `TraceLine status`（AskUserQuestion / Bash / Read / Write）与 Ask / Bash 的 `Spinner` 判据共用（R6 的形状收口手段）。私有单点，经四张卡渲染驱动
- `AgentToolCardProps`（`turnRunning?: boolean`）: 六字段 → 七字段（缺席按运行中处理）；`AgentToolCard` 类型别名不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `unpairedRunning`（四卡） | 正向 | 四张特化卡（注册键 `AskUserQuestion` / `bash` / `read` / `write`）在 `turnRunning = false` + `output = null` → 卡内 `[data-slot="trace-line"]` 的 `data-status` 为 `null`（不呈 `running`，无 shimmer 载体） | 新增 |
| `unpairedRunning`（四卡） | 正向 | 同上四卡在 `turnRunning = true` 与**缺席**（`undefined`）两形态 → 仍呈 `data-status="running"`（运行中轮不变；既有 output 三态断言逐字不动） | 保留 |
| `unpairedRunning`（图标面） | 正向 | Ask / Bash 在 `turnRunning = false` + `output = null` → `[data-slot="spinner"]` 零命中、特化图标（`MessageCircleQuestionIcon` / `TerminalIcon` 的 svg）在场；`turnRunning = true` → spinner 在场（既有旋载图标参数化用例扩第三列） | 修改 |
| `unpairedRunning` | 异常 | 终结轮但已有结果：`output = {isError: false}` → `data-status` 为 `null`；`output = {isError: true}` → `data-status = "error"`（状态判据以结果为先，轮状态不覆盖结果态两分支） | 新增 |
| `unpairedRunning` | 异常 | `turnRunning = false` + `output = null` → `data-status` **不是** `pending`（DD7：复用一个「思考中」态会重造歧义；断言取值恒为 `null`，源码面零 `'pending'`） | 新增 |
| `unpairedRunning` | 边界 | 三张无 running 标记的卡（`edit` / `finish_step` / `web_search`）在 `turnRunning = false` 下与既有断言逐条相等（不在本变更范围，零变化） | 保留 |
| `unpairedRunning` | 边界 | 默认对卡 `ToolPairCard`（不在四处内、`props` 无 `turnRunning`）→ 呈现与改动前逐条相等（范围边界：其 `Spinner` 残留登记为 design 待决问题 1 的已知残余，不认账） | 保留 |
| `unpairedRunning` | 边界 | Ask 卡交互面不受轮状态影响：`turnRunning = false` 但注入 `askAnswer`（活卡由 `agent-messages` 判据决定）时提交仍走 `onAnswer`；卡片自身不新增第二份判活逻辑（判据单点在 `isLiveAsk`） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `@/lib/highlighter`（shiki 语法包的动态 import 与重型 grammar 装载——进程边界） | 既有 `vi.mock` 替身：`loadHighlighter` 产出可断言行并记录语言实参；`beforeEach` 复位 | Write / Edit 卡行渲染 |
| `BUILD_IN_TOOL_CARDS` 注册表 / `Trace*` / `CodeBlock` / `lib/parse` | 不 mock：真实组合（卡渲染间接驱动私有判据 `unpairedRunning`，不为测试新增导出） | 全部用例 |

### packages/desktop/src/hooks/use-agent-session.ts -> packages/desktop/src/hooks/use-agent-session.test.ts

#### 待测功能

<!-- design.md「零 diff 登记」表在册：本文件生产面零改动（running 派生已是「轮行含 running」、订阅判据已同源）。
     本章节用例为新枚举值的默认行为固化（`interrupted` 天然不派生运行中、不触发订阅），改动前即应绿。 -->

- `useAgentSession(params)`: `running = mirrors.turns.some(t => t.status === 'running')`——`interrupted` 轮行不派生运行中、不触发订阅；重放装载与折叠对 `interrupted` 轮行零特判

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `useAgentSession.running` | 正向 | `session_detail` 轮行全 `interrupted` → `running === false`、`agent_session_watch` 零调用（不订阅、不轮询）；`loading` 回 `false` | 新增 |
| `useAgentSession.running` | 正向 | 轮行 `[completed, interrupted]` → `running === false`；`messages` 末条 `metadata.record.status = 'interrupted'`（重放折叠原样透出，链路零特判） | 新增 |
| `useAgentSession.running` | 异常 | 轮行 `[running, interrupted]`（残留新轮并存）→ `running === true`、恰一次订阅（判据为 `some` 而非末轮，`interrupted` 不压制他轮运行中派生） | 新增 |
| `use-agent-session` 边界 | 边界 | 零轮行（空会话）→ `running === false`、零订阅（既有保留）；`interrupted` 空内容轮在折叠后仍进 `messages`（与 `filterConversation` 白名单一致），`messages` 非空 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `@tauri-apps/api/core` 的 `invoke` / `Channel`（进程边界 IPC） | 既有 `vi.hoisted` + `vi.mock`：按命令名 + 调用序可编程（`session_detail` / `agent_session_transcript` / `agent_session_watch`），`ChannelMock` 直投信封 | 全部用例 |
| transport / 折叠 / `@ai-sdk/react` `useChat` | 不 mock：真实组合（hook 即链路入口） | 全部用例 |

### packages/desktop/src/components/agent/agent-error.ts -> packages/desktop/src/components/agent/agent-error.test.ts

#### 待测功能

<!-- design.md「零 diff 登记」表在册：`latestAgentFailure` 生产面零改动（只认 `failed` 与「running 且失败 done」）。
     `interrupted` 不进失败横幅（AC-9 半边）由本章节固化默认。 -->

- `latestAgentFailure(events, turns) -> AgentFailure | null`: 最新轮 `interrupted` 时返 `null`（顶部失败横幅零呈现），与 `stopped` 同列

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `latestAgentFailure` | 正向 | 最新轮 `interrupted` + 无事件 → `null`（不弹红条） | 新增 |
| `latestAgentFailure` | 正向 | 最新轮 `interrupted` 且 `record.error` 携标定记因 → 仍 `null`（记因是机器事实、不是失败横幅触发面） | 新增 |
| `latestAgentFailure` | 异常 | 最新轮 `interrupted` + 该轮内 `api_error` 通知（时间戳在轮开始之后）→ `null`（通知不把中断升级成失败） | 新增 |
| `latestAgentFailure` | 异常 | 最新轮 `interrupted` + 其后一条失败 `turnDone`（`timestampMs >= startedAt`，兜底判据成立）→ `null`（`runningJustFailed` 要求 `status === 'running'`，中断不落入兜底） | 新增 |
| `latestAgentFailure` | 边界 | 上一轮 `failed`、最新轮 `interrupted` → `null`（旧错误不泄漏，既有判据保留）；对照两行：最新轮 `running` + 失败 done → 非 `null`、最新轮 `failed` → 非 `null`（既有行为零回归） | 保留 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 零 mock：纯函数，`AgentEvent` / `TurnSummary` fixture 显式构造（时间戳显式注入，与钟面无关） | 全部用例 |

### packages/desktop/src/views/agent/components/agent-run-history.tsx -> packages/desktop/src/views/agent/components/agent-run-history.test.tsx

#### 待测功能

- `statusLabel(status: AgentRunStatus) -> string`: 五值映射（`running` → 运行中 / `completed` → 已完成 / `stopped` → 已停止 / `interrupted` → 已中断 / 其余 → 失败兜底），`interrupted` 置于 `失败` 兜底之前。私有单点，经 `AgentRunHistory` 渲染驱动
- `SessionRow` 的 `data-status` 与徽章配色: `data-status={status ?? 'empty'}` 直出 `interrupted`；行色零改动（`interrupted` 自然落 `text-muted-foreground` 分支，与 `stopped` 同列）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `statusLabel` | 正向 | 末轮 `interrupted` → 徽标文本「已中断」、`data-status="interrupted"`、徽标 class 含 `text-muted-foreground` 且不含 `text-fail`（不落失败配色） | 新增 |
| `statusLabel` | 正向 | 五值词面参数化（`running` / `completed` / `stopped` / `failed` / `interrupted` → 运行中 / 已完成 / 已停止 / 失败 / 已中断），既有四值断言逐字不动（保留） | 修改 |
| `statusLabel` | 异常 | 越过类型的脏 status（`'paused'`）→ 仍落「失败」兜底、不抛错（兜底面保留，扩值不引入未识别态） | 保留 |
| `SessionRow` | 边界 | 无轮行空会话 → 「空会话」+ `data-status="empty"`（既有保留，`interrupted` 不误占空态词面） | 保留 |
| `SessionRow` | 边界 | `interrupted` 轮携 `turn.error`（标定记因）→ 徽标「已中断」与行内 `run-row-error`（记因原文）并存（既有呈现面零改动；文案 MUST NOT 复用失败标题） | 新增 |
| `SessionRow` | 边界 | 多轮（`completed` → `interrupted`）→ 终态取自末轮（`interrupted`、词面「已中断」），非首轮（既有「终态取最新轮行」判据扩值） | 修改 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `@tauri-apps/api/core` 的 `invoke` / `Channel`（进程边界 IPC） | 既有 `vi.hoisted` + `vi.mock`：清单态由 `state` fixture 直传，重放区命令按名分发并计数 | 全部用例 |
| `useAgentRunHistory` / `AgentSessionView` / `AgentMessages` | 不 mock：真实组合 | 重放区相关用例 |

---

## 不可测试项

- `packages/desktop/src/types/generated/bindings.ts` / `bindings.zod.ts`（`AgentRunStatus` union 与 zod literal 增 `"interrupted"`） — **原因**: specta / zod 再生成物，非手写可测单元，不建共置测试（会成空套件）；一致性由机械闭环守——`bindings:export` 无 stale diff、`bindings:zod-check`（`tsc --noEmit -p tsconfig.zod.json`）与 `client:check` 在 test-execution 相位执行，值域行为面由 `runner_test.rs` / `session_test.rs` 与 `model_test.rs` 覆盖（AC-1 / AC-2）。
- `packages/desktop/package.json`（`version` 0.6.47 → 0.6.48） — **原因**: 版本行为静态纪律（`tauri.conf.json` 经 `"version": "../package.json"` 跟随、`src-tauri/Cargo.toml` 不随动），无运行时行为与夹具面；判据为 `git diff` / `node -p` 静态核对（AC-12 半边）。
- `openspec/changes/session-midturn-history-overwrite/specs/desktop-change-orchestration/spec.md`（「相位内步骤系统异常」分类枚举整段改写为 `Failed` / `Stopped` / `Interrupted`） — **原因**: 规范散文留痕，非可执行单元；行为半边由 `walker_test.rs` 的 `worker_settle` 新臂用例承载，字面一致性由 `grep` 口径核对与归档 spec 审计承接（AC-5 的「明文留痕」半边）。
- AC-12 门禁聚合（`vp check`（fmt / lint 含 50 行函数限）/ `vp test` / `knip` / `client:check` / `cargo nextest --workspace` 全绿、基线 8976 例与覆盖率阈值不回退） — **原因**: 执行聚合面——由本文件各章节用例全集在 Rust / 前端自动化套件运行时综合承载，无独立被测单元与新增用例；由 test-execution 相位执行。
- `packages/desktop/src-tauri/crates/core/agent/src/kernel.rs` / `ask.rs` / 三张内存注册表 / `infra/agent/src/sdk/resume.rs` / 既有 `calibrate_interrupted_runs` 函数体（design 「零 diff 登记」表） — **原因**: 本变更零改动面，无新增行为可测；守线判据为 `git diff` 为空 + `AgentRunStatus::Interrupted` 在生产代码仅两处命中（静态判据），其行为不被本变更加以断言以免越界固化他域实现细节。
- `packages/desktop/src/components/agent/agent-blocks.tsx`（默认对卡 `ToolPairCard` 的 `Spinner` 在终结轮仍旋转） — **原因**: design 校准 2 / 待决问题 1 显式登记的范围外已知残余（其 `TraceLine` 无 `running`、无 shimmer，不违反 AC-7 判据）；本变更不新增生产改动与用例，验收判据不予认账，后续单开 change 收口。
