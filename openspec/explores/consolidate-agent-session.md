# Agent 会话启动逻辑归一到 core/agent 的探索

workflow_type: requirement

## 主题与背景

用户诉求：packages/desktop 代码中多处启动 agent 会话（New or Continue）的逻辑归一到
`crates/core/agent` 下；其他相关 agent 能力外溢也一并收口。

现状结论先行：**"启动会话"没有散落失控，但被切成三段**——词汇转换段（Option<String> ↔
SessionRef，4 处手工重复）、语境组装段（SessionCtx / provenance / 档位，2 处 + 1 处弱类型镜像）、
治理校验段（Continue 存在性 / 引擎匹配 / prior_handle 提取，驻 infra compose）。core/agent 的
`SessionKernel::begin_turn` 只承接了内核段（铸 id / 建行 / 开轮 / 登记）。另有 agent 能力外溢
两处：`final_assistant_text` / `failure_reason` 转录提取器住在 core/orchestration（致
agent-runtime 反向依赖 orchestration，与 crate-layout spec 漂移）；TurnSummary running/终态
装配住在 app 命令层。

## 现状全景：New/Continue 的全部表达位点

```
前端 src/
  use-agent-chat.ts  sendMessage: sessionId = currentSessionIdRef.current
  │                    (null → New；有值 → Continue —— 判定在前端镜像)
  agent-transport.ts invokeStart → commands.agentStart(...)
  │
  ▼
app 命令层 src-tauri/src/commands/
  exec/mod.rs agent_start_with          change_flow/mod.rs change_flow_start_with
  │ ① Option→SessionRef (L76-79)        │ (组合根装配，会话启动委托 worker)
  │ ① SessionCtx 构造 (High 硬编码)     │
  │ ① provenance 缺省 "debug"           ▼
  │         compose_turn ────────► infra/agent compose.rs compose_turn
  │                                      │
  │                                      ▼
  │                            infra/agent compose.rs ComposedTurn::begin (L92-147)
  │                                      │ ② Continue 校验三连:
  │                                      │    存在性 / 快照 engine 匹配 /
  │                                      │    prior_handle(engine_session_id) 提取
  │                                      │ ② 档位定型 + config_snapshot 组装
  │ ③ spawn drive→Channel               ▼
  │    exec/agent.rs                     core/agent kernel.rs SessionKernel::begin_turn
  │    running_summary /                 │ ④ session_id 铸造/沿用 + open
  │    outcome_summary                   │ ④ New 建会话行 / seq 基点 / 开轮 / 停止登记
  ▼                                     ▼
                                infra/agent worker.rs KernelWorkerPort::execute
                                  ①' Option→SessionRef (L47-50)   ← 与 ① 同型重复
                                  ①' SessionCtx 构造 (L51-55)
                                  ⑤ sealed 转录累积 + final_assistant_text
                                  ⑥ CLI 预检 is_cli_engine→discover
                                        ▲
                                        │ WorkerAgentPort (port.rs)
  core/orchestration walker.rs ──────────┘
    ⑦ decision_session: continue_session 状态变量 (L2789/2836)
    ⑦ step_static_check / step_test_execution:
       cursor.executor_session 反馈边续注 (L806-830)
    ⑦ WorkerTurnRequest.continue_session: Option<String>  ← 弱类型词汇
```

引擎侧（infra 内，属 runner 契约实现非外溢）：
- `cli/flags.rs`: prior_handle → `--resume <id>`
- `sdk/runner.rs::continue_session_id`: SessionRef → Option<String>（重建史寻址，反向转换）

## 重复 / 外溢清单（收口候选）

| # | 模式 | 位置 | 层 | 性质 |
|---|---|---|---|---|
| ① | `Option<String> → SessionRef` 手工 match，4 处（含反向 2 处） | exec/mod.rs:76、worker.rs:47、compose.rs:99、sdk/runner.rs:137 | app/infra | 词汇重复 |
| ② | Continue 治理校验（存在性/引擎匹配/prior_handle 提取） | compose.rs::begin 99-118 | infra | 会话启动治理段外溢 |
| ③ | TurnSummary running/终态装配 | exec/agent.rs | app | 类型在 core、装配在 app |
| ⑤ | 转录提取器 `final_assistant_text`/`failure_reason` | core/orchestration/transcript.rs | orchestration | agent 能力外溢（输入是 `&[AgentEvent]`） |
| ⑦ | `WorkerTurnRequest` 以 `Option<String>` + root/permission/model_level 散列字段重复表达 SessionRef + SessionCtx | orchestration/port.rs:50-67 | orchestration | 词汇弱化 + 字段面重复 |
| ⑧ | worker 的转录累积 + final_message 提取 | infra/agent/worker.rs:58-84 | infra | 转录收集属会话能力，可议 |

## 约束与坑

- **spec 漂移**：`desktop-crate-layout` 规定 `agent-runtime SHALL 仅依赖 agent`，实际
  Cargo.toml 依赖 orchestration / store / config / exec / foundation / rig。worker.rs 依赖
  orchestration 是漂移主因（port 类型 + final_assistant_text）。收口方向若把提取器搬回
  core/agent，可顺带消掉 orchestration 依赖边。
- **core 纯度红线**：`desktop-agent-execution` spec 规定 core/agent 不解释 config_snapshot、
  不认识 engine。② 的引擎匹配校验（`record.config_snapshot.engine != store_kind`）要下沉
  core，须经 port 抽象（如 sink/query 侧提供「会话可续接性判定」），不能直接带 engine 概念进 core。
- **双轨道差异**：exec 轨道（前端直发、单轮、Channel 流出、不累积转录）与 change 轨道
  （walker 编排、反馈边续注、转录累积出 final_message）对同一内核的消费形状不同，收口 API
  须同时服务两形。
- **spec 愿景 vs 实现**：spec 愿景是 core 提供 `run_session(New | Continue{id}, prompt)`
  形状公共 API；实际是 `SessionKernel::begin_turn(runner, TurnRequest)` + 外部三段。归一
  工作天然是这个 gap 的收窄。

## 可跟进的收口线索（按侵入度递增）

- **线索 A（词汇收口，低风险）**：core/agent 为 SessionRef 提供 `from_option(Option<String>)`
  构造（或 `TurnRequest` 直接吃 Option），消灭 4 处手工 match；orchestration 的
  `WorkerTurnRequest` 字段面改用 SessionRef / SessionCtx（或映射单点化）。
- **线索 B（提取器回家）**：`final_assistant_text` / `failure_reason` 自 orchestration 搬
  core/agent（纯函数、零依赖障碍）；infra→orchestration 依赖边随 worker 适配层重组消解。
- **线索 C（治理段下沉）**：Continue 校验经 SessionQuery port 扩展（如
  `resolve_continue(id) -> Option<{prior_handle, 可续接}>`）下沉进 SessionKernel，compose::begin
  瘦身为纯组装；引擎匹配的 engine 概念留在 port 实现侧。
- **线索 D（摘要装配收口）**：running_summary / outcome_summary 挪 core/agent（TurnSummary
  本就是 core 类型），exec/agent.rs 只剩 IPC 信封。

## 关键文件索引

- `packages/desktop/src-tauri/crates/core/agent/src/{kernel,runner,port,session}.rs` — 收口目标
- `packages/desktop/src-tauri/crates/infra/agent/src/{compose,worker}.rs` — 治理段 + 适配层
- `packages/desktop/src-tauri/crates/core/orchestration/src/{port,walker,transcript}.rs` — 弱类型词汇 + 外溢提取器
- `packages/desktop/src-tauri/src/commands/exec/{mod,agent}.rs` — app 层词汇转换 + 摘要装配
- `packages/desktop/src/hooks/use-agent-chat.ts`、`src/lib/agent-transport.ts` — 前端 New/Continue 判定
- spec: `openspec/specs/desktop-agent-execution`、`desktop-crate-layout`、`desktop-agent-chat-infra`
