# 下线 CLI 引擎（retire-cli-engine）

> 探索日期：2026-10-13　状态：数据面「不动」方案已定；`AgentEngineKind` 边界越界待拍
> 拟议 change 名：`retire-cli-engine`（与本文主题对齐）
> **注**：早期讨论的通用升级脚本（`store-upgrade-script`）已评估并移除——对当前程序过重，且要解决的问题在「数据面不动」方案下不存在。

## 起点

用户提出：下线 `packages/desktop/src-tauri/crates/infra/agent/src/cli`。

`cli/` 只是**实现落点**（2251 行，实现约 560 + 测试约 1690）。"CLI 引擎"作为**一枚产品枚举值**贯穿 store 线格式、IPC 绑定、前端 agent 面板与两份 spec。下线的牵动面远大于目录本身。

## 现状地形

```
                ┌──────────────────────────────────────────────┐
                │        EngineKind { Cli, Sdk }  (lib.rs)     │
                │        EngineFacade::runner_for 唯一 match    │
                └───────┬──────────────────────────┬───────────┘
                        │                          │
          ┌─────────────▼──────────┐   ┌───────────▼────────────┐
          │  cli/  ← 要下线的目录  │   │  sdk/  (rig 进程内)    │
          │  discover/flags/jsonl  │   │  loop/tools/bash/...   │
          │  runner (spawn+泵)     │   │                        │
          └─────────────┬──────────┘   └───────────┬────────────┘
                        │                          │
        ┌───────────────┴──────────────────────────┴──────────┐
        │  compose.rs：resolve_agent_engine 双臂解析            │
        │  worker.rs：is_cli_engine() → discover() 轮前预检     │
        └──────────────────────────┬──────────────────────────┘
                                   │
        ┌──────────────────────────▼──────────────────────────┐
        │  store::AgentEngineKind { Cli, Sdk }  持久化枚举      │
        │  线值 "cli" | "sdk" → redb 行 + bindings.ts + 前端    │
        └──────────────────────────┬──────────────────────────┘
                                   │
        ┌──────────────────────────▼──────────────────────────┐
        │  agent-panel.tsx：engine 下拉 ['sdk','cli']           │
        │  cli 档 provider 可空（SDK 必选）                     │
        └─────────────────────────────────────────────────────┘
```

**规模**：`cli/` 共 2251 行——实现约 560（discover 51 + flags 49 + jsonl 201 + runner 258 + mod 26），测试约 1690。删目录是轻活；重活在枚举值与语义分叉。

**已有纪律站在下线这一边**：`desktop-agent-execution` spec 写明 SDK 引擎"定位为不依赖本地环境的 agent，开箱即用，**不提供 CLI-only 裁剪档**"——下线 CLI 是回到主线，不是逆架构。但 spec 同时写明多租户（本机 CLI / 进程内 SDK / 远程 API）都 SHALL 落在协议预留内，故 **`AgentRunner` trait 这层抽象不该跟着删**。

## 已决：C 档下线深度

| 档位 | 动什么 | 问题 |
|------|--------|------|
| A. 只删实现 | `src/cli/` + `ClaudeCliRunner` + worker 预检 + `EngineKind::Cli` 臂改 Err | 留下"枚举值在、无实现"的僵尸态 |
| B. 删到产品面 | A + store 枚举去 `Cli` + 前端下拉去 'cli' | 存量 redb 行成解码雷 |
| **C. 删到契约面（已决）** | B + 两份 spec 的 CLI 租户 requirement 退役 + 双枚举是否合一 | 干净，改动面最大 |

**C 档终态硬边界**（proposal 须写死，防止"顺手把引擎抽象也塌了"）：

- **退役的是 CLI 这个租户实现**，不是多租户机制
- `AgentRunner` trait / `EngineKind` 参数面 / `runner_for` 唯一 match 点结构 **MUST NOT** 塌掉
- `crates/core/agent` 的协议面（AgentEvent 信封、AgentRunner、run 状态机）零污染，维持现状
- **`AgentEngineKind` 的 `Cli` 变体永久保留**（解码锚，非产品能力）——见下文「数据面不动」
- CLI 独占语义逐条确认 SDK 不欠账（见下表）

### CLI 独占语义的去向

| CLI 独有 | SDK 侧现状 | 下线后处置 |
|---|---|---|
| `--resume <engine_session_id>` | store 全史转录重建（resume 装载缝） | 无欠账 |
| `--disallowedTools AskUserQuestion` | 工具面封闭清单，天然无 ask | 无欠账 |
| `discover()` 轮前预检（CliMissing） | `ConfigMissing` 归 `begin()` open 段 | worker 预检分支随 CLI 退役 |
| model 必须走 provider 双档 | 同上 | 无欠账 |
| `engine_session_id` 为寻址句柄 | **不消费**（每轮新铸，仅留痕） | 见下节 |

## 已决：`engine_session_id` 退役消费点、保留留痕

查证结论（**反直觉**）：

```
                    写入侧                          读出侧
              ┌──────────────────┐          ┌───────────────────────────┐
 TurnDone     │ kernel 取 event  │          │ compose.begin() Continue  │
 .session_id ─▶ TurnDone.session│──bind──▶  │  → engine_session_id 是   │
 (CLI/SDK    │ _id → stats      │  落库     │    硬门槛，缺失即 Err      │
  都上报)     │ remote_session_id│          │  （唯一寻址消费者）        │
              └──────────────────┘          ├───────────────────────────┤
                                            │ store.rs:127 → SessionRow │
                                            │ .remoteSessionId → IPC    │
                                            │ （观测面，bindings 在册）  │
                                            └───────────────────────────┘
```

证据：`sdk/runner.rs` 注释原文——

> 先行句柄 sdk 引擎不消费（引擎侧标识每轮铸造，重建源即会话全史转录）

`sdk/resume.rs` 头注释进一步说明 `sdk-<n>-<ms>` 前缀 id"**每轮铸造**……**不再承担归属校验**"。

| | CLI | SDK |
|---|---|---|
| 上报 `engine_session_id` | 是（`--resume` 句柄） | 是（每轮新铸，留痕） |
| 续会话**靠它寻址** | **是** | 否（走转录重建，寻址用 core 铸的 `ses-` id） |

**处置**：

1. **退役 `compose.rs` 的寻址硬门槛**——`begin()` 里"会话缺少引擎句柄，无法续接"的 Err 分支。它在用 CLI 的寻址语义卡所有引擎；CLI 走后唯一剩余消费者消失，该分支随 CLI 退役（删掉或降级为不阻断续会话的观测）。
2. **保留双 id 映射留痕**：`SessionRecord.engine_session_id` + `SessionRow.remoteSessionId` 是**引擎中立**的协议字段（"这轮落在哪个引擎侧会话"），非 CLI 私有。续会话不再靠它寻址，但留痕价值仍在。

这与 C 档"退役租户、保留多租户机制"同一原则。

**前端消费查证（已定）**：`remoteSessionId` **生产代码零消费**。全仓引用只在两处——生成绑定 `bindings.ts`（specta 自动出线，非手写）与**大量测试 mock 数据**（`use-agent-chat.test.ts` / `use-agent-run-history.test.ts` / `agent-debug-view.test.tsx` / `use-session-transcript.test.ts` / `session-transcript-panel.test.tsx` / `change-detail-view.test.tsx` / `detail-drawer.test.tsx` / `use-explore-session.test.ts` 等）。`use-agent-chat.ts:79` 的 `remoteSessionId: null` 只是**构造镜像投影时填 null**，不是消费。

**修正：此处不触发 golden wire 冻结流程。** golden 契约（`desktop-corpus-regression` + `core/workflow/tests/golden/` 的 8 份快照）**只覆盖 change 域**（archive / backtrack / list / multi-attempt / run-history / run-interrupted / slots-null / worktree），零 `engine` / `remoteSessionId` / `AgentEngineKind` 引用。agent 域的 DTO 演进走的是 **specta 重导出 + bindings 一致性守卫**（`src/bindings/mod_test.rs`）这条线，不是 golden。

故「列留不留」的约束强度降一档：**留**（双 id 映射留痕 + 测试 mock 面大），但**若要删也无 golden 阻力**，只需重导 bindings 并同步清理测试 mock。倾向**留**——留痕价值真实，删除收益仅是少一个 `Option` 字段。

## 已决：数据面不动（存储形态不变）

### 拦路虎：bincode 变体序号

早期讨论的"删变体 + 清理脚本"路线，撞上 bincode 1.3 的编码事实（`bincode-1.3.3/src/ser/mod.rs:220` / `de/mod.rs:284`）：

```rust
fn serialize_unit_variant(self, _name, variant_index: u32, _variant) -> Result<()> {
    O::IntEncoding::serialize_u32(self, variant_index)   // 写序号，不写名字
}
// 读侧：
let idx: u32 = O::IntEncoding::deserialize_u32(self)?;
let val = seed.deserialize(idx.into_deserializer());     // 按序号认变体
```

`native_model` 默认 codec 为 **bincode 1.3**（`native_model_macro` 默认 `with = bincode_1_3::Bincode`）。`AgentEngineKind` 是两变体枚举，序号钉死：

```rust
enum AgentEngineKind { Cli, Sdk }   // Cli = 0, Sdk = 1
```

三态风险全集：

| 变体集 | 存量 `Cli` 行（字节 `0`） | 存量 `Sdk` 行（字节 `1`） |
|---|---|---|
| `Cli, Sdk`（现状） | → `Cli` ✓ | → `Sdk` ✓ |
| **`Sdk` only**（删 Cli） | 越界 → **整行炸** | 越界 → **整行炸** |
| **`Sdk, Cli`**（重排） | → **误读成 `Sdk`** | → **误读成 `Cli`** |

**推论（硬）**：删 `Cli` 变体后，`Sdk` 行（序号 1）**同样越界失败**——不是"cli 数据读不出"，是 **`AgentInstanceRecord` 和 `SessionRecord` 两张表整体报废，包括所有 sdk 数据**。重排更阴险：**静默污染**（存量 sdk 会话被标成 cli，在下线 cli 的当口反向污染）。

### 决断：`AgentEngineKind` 变体集不动

**数据面 = 零改动。** `AgentEngineKind` 的 `Cli` 变体**永久保留**——它现在是"历史数据的解码锚"，不是产品能力。注释标明"解码兼容，不再可选"。

```
              代码面（删/改）                    数据面（零改动）
        ┌──────────────────────────┐      ┌──────────────────────────┐
        │ 删 src/cli/  2251 行     │      │ AgentEngineKind 变体集   │
        │ EngineKind::Cli 臂 → Err │      │ 保持 Cli, Sdk 不动       │
        │ worker discover 预检删   │      │   → 存量行照读，零清理   │
        │ compose 硬门槛退役       │      │                          │
        │ 前端下拉去 'cli'         │      │ 仅处理：                 │
        │ save_agent 拒新增 Cli    │      │  默认 agent 是 cli 的那条 │
        │                          │      │  → 清 is_default / 或    │
        │ EngineKind 不出 bindings │      │    resolve 时显式 Err    │
        │  → 可塌成单值，零阻力    │      │                          │
        └──────────────────────────┘      └──────────────────────────┘
```

### 存量数据逐表清点（结论：几乎不用处理）

```
全局库
  AgentInstanceRecord (id=6, bincode)   engine: AgentEngineKind
      ├─ 不删变体 → 零处理，行照读
      ├─ 删变体   → 全表报废（含 sdk）   ← 禁止
      ★ 真问题：is_default == true 且是 Cli 的那条
         → 缺省发起走 EngineKind::Cli 臂 → 必失败
         ⇒ 唯一需要动作的存量数据

workspace 库
  SessionRecord (id=7, bincode)   config_snapshot.engine: AgentEngineKind
      ├─ 不删变体 → 零处理，行照读
      ├─ 删变体   → 全表报废（含 sdk）   ← 禁止
      △ 存量 cli 会话续接 → Cli 臂 Err（"CLI 租户已下线"）
        这是预期行为，不是数据问题

  SessionEventRecord (id=8, SerdeJsonCodec=JSON)
      └─ 完全不碰枚举，转录是 AgentEvent，删变体也不炸
         零处理

  ExploreRecord (id=4, bincode)   session_id: Option<String>
      └─ 不指向 engine，是 session_id 外键
         △ 若会话行还在 → 反查可达，正常
         △ 若会话行删了 → find_session 返回 None
           use-explore-session.ts:19 是 `record?.sessionId ?? null`
           → 传 null → use-agent-chat 的"新会话"态（现成兜底）
         零处理

  AgentRunRecord (轮统计行)
      └─ 不含 engine 字段（AgentEngineKind 只落 AgentInstanceRecord +
         SessionConfigSnapshot 两处，grep 已确认）
         零处理
```

**唯一需要动作的**：全局库里 `is_default == true && engine == Cli` 的那一条实例。它会让"缺省发起运行"走进已删除的 CLI 路径。处置：清 `is_default`（或 `resolve_agent_engine` 缺省路径显式 `Err`）。

### 未来真正清理 cli 残留的出口（非本次）

若未来要清掉 cli 残留数据，那是**独立 change**，走 `native_model` version bump（仓库已有 6 处 `from =` 先例：`AgentRunRecordV3→V4`、`ExploreRecordV1→V2→V3→V4`、`ChangeRecordV3→V4`、`AgentProviderRecordV1→V2`），且 `From` 实现需为 `Cli` 决定映射去处（`Retired` 变体？弃行？）。那才是真正需要设计的地方，现在不必预建。

## 探讨：`AgentEngineKind` 边界是否越界

**用户命题**：只有 agent 实例自身知道 `AgentEngineKind`；所有 agent 执行/查询/统计都以 agent 实例作为基础对象。当前是否有越界逻辑？

### 全部非测试消费点（7 个文件）

| 文件 | 层 | 消费形态 | 判定 |
|---|---|---|---|
| `store/model.rs` | infra | 枚举定义 + 两处持久化（`AgentInstanceRecord.engine` / `SessionConfigSnapshot.engine`） | 见下 |
| `store/store.rs` | infra | `engine == Sdk` → provider 必填校验；`remote_session_id` 读写 | ✅ |
| `commands/agents/mod.rs` | 命令 | `save_agent(engine)` 透传 store | ✅ |
| `compose.rs` | infra | `resolve_agent_engine` 两臂 match → `EngineKind` 映射；`begin()` 跨引擎续会话校验 | ✅ 见下 |
| `worker.rs` | infra | `is_cli_engine()` → discover 预检 | ✅（随 CLI 退役） |
| `lib.rs` | infra | `EngineKind` 枚举 + `runner_for` match | ✅（门面专有） |
| `store/lib.rs` | infra | re-export | ✅ |

### 两处越界

**① `SessionConfigSnapshot.engine`（持久化 + IPC 出线）**

```
  AgentInstanceRecord.engine ──compose──▶ SessionConfigSnapshot.engine
  （配置：这个 agent 是什么引擎）          （快照：这个会话是用什么引擎跑的）
                                                 │
                                                 ▼
                                          SessionRow.configSnapshot
                                          → bindings.ts `configSnapshot: unknown`
                                          → 前端测试 mock 断言 `engine: 'sdk'`
```

- 快照本身是**装配事实记录**（"当时是用什么引擎发起的"），core 以 `serde_json::Value` 不透明承载——架构上设计成"core 不解释"
- 但它**复制**了 `AgentEngineKind` 进会话域：会话现在直接携带引擎二值，而按用户命题，引擎应是**实例属性**，会话只需知道用了哪个实例
- `SessionRow` **不携带 `agent_instance_id`**（grep 确认），所以会话↔实例的关联只能靠快照里的 `engine`（而 `engine` 不是实例 id）——这是真正的语义漂移

**判定：越界（轻度）。** 快照作为**历史事实**有价值，但作为**控制流输入**越界——`compose.begin()` 用它做跨引擎续会话校验（`config_snapshot.engine != self.store_kind` → Err），这是把"历史记录"当"路由决策"用。

**② `compose.begin()` 跨引擎续会话校验**

```rust
if record.config_snapshot.engine != self.store_kind {
    return Err("会话非当前引擎产出（跨引擎续会话不支持）");
}
```

这用快照的 `engine` 做**路由门**。CLI 走后 `EngineKind` 只剩 `Sdk`，此门自动失效（永远相等）。但它现在的设计暗示"引擎是会话的属性"，与用户命题"引擎是实例的属性"不符。

**判定：越界（但 CLI 下线后自动失效）。** 是否随 CLI 退役一并清理（或改为以实例 id 校验），待拍。

### 其余面均未越界

| 关注点 | 事实 | 判定 |
|---|---|---|
| **执行** | `agent_start` 不收 engine 参数，只收 `agent: Option<i64>`（实例 id）；引擎选择完全由 `resolve_agent_engine` 从实例记录推导 | ✅ |
| **查询** | `list_sessions(source, source_ref)` 不按 engine 过滤；`find_session_detail` 不看 engine | ✅ |
| **统计** | `SessionStats`（轮数/墙钟/token）无 engine 维度 | ✅ |
| **转录** | `SessionEventRecord` 载荷是 `AgentEvent`（core 纯类型），不含引擎概念 | ✅ |
| **前端** | `configSnapshot` 在生产代码零消费（仅测试 mock 断言引擎值） | ✅（但 mock 断言反映 DTO 设计意图） |
| **IPC** | `AgentEngineKind` 出线仅在 `save_agent_instance` 的 `engine` 参数（实例管理面，合理）；`EngineKind`（门面枚举）**不出 bindings** | ✅ |

### 决断：不动 + 快照补 `agent_instance_id`

**① 越界不修（用户拍板「不动」）**：`compose.begin()` 的跨引擎校验与快照 `engine` 字段均维持现状。理由：CLI 下线后 `EngineKind` 只剩 `Sdk`，那道门自动失效，主动退役的收益接近零；快照 `engine` 作为**历史事实**（"当时用什么引擎发起"）有考古价值，不必为对齐概念纯度而动它。

**② 快照增 `agent_instance_id`（用户拍板）**：在 `SessionConfigSnapshot` 上补 `agent_instance_id: Option<i64>`，把会话↔实例的关联从"靠 `engine` 间接猜"改成**显式外键**。这才是对越界①的正解——不删 `engine`（历史事实），但补上真正的关联键。

**③ 多 agent 是既定方向（用户确认）**：未来会加入多 agent，故 agent 实例抽象**MUST NOT** 塌成单值/隐式单例。`agent_instance_id` 正是为多 agent 预留的关联键——会话将来可按实例过滤/聚合。

**落地形态**：

```rust
pub struct SessionConfigSnapshot {
    pub engine: AgentEngineKind,        // 保留：历史事实（"当时用什么引擎"）
    pub agent_instance_id: Option<i64>, // 新增：实例外键（多 agent 关联键）
    pub model: Option<String>,
    pub permission_mode: AgentPermissionMode,
}
```

**实现牵动**：

- **存储演进走既有通道**：`SessionConfigSnapshot` 嵌在 `SessionRecord`（`native_model(id=7, version=1)`，bincode）内，加字段改 bincode 布局 → 走 `native_model` **`from` 链**（`SessionRecordV1 → SessionRecordV2`，`From` 里旧快照的 `agent_instance_id` 填 `None`）。这是仓库**已确立 6 次**的通道（`AgentProviderRecordV1→V2` 加 `context_length`、`ExploreRecord` 三连、`AgentRunRecordV3→V4`、`ChangeRecordV3→V4`），**不是** `store.rs` 文档禁的"第三条 decode/migrate 路径"——那条禁的是**库格式层**（`format_version` 标记，旧库整体作废重建），model 版本层的 `from` 链是正路。
- **`ResolvedEngine` 须携实例 id**：`compose.rs` 的 `ResolvedEngine` 现只有 `kind/config/context_window`，`resolve_agent_engine` 解析出实例后须把 `instance.id` 带出来，`begin()` 组装快照时填入。缺省路径（`agent: None` → 默认实例）同样解析出实例 id，故新会话恒 `Some`；仅存量行是 `None`。
- **`compose.begin()` 的跨引擎校验不动**（见①），本次不改其逻辑。
- **DTO 出线**：`SessionConfigSnapshot` 经 `SessionRow.configSnapshot: unknown` 出线（非结构化），前端测试 mock 需同步补 `agentInstanceId` 键——bindings 一致性守卫会检查。

**与「数据面不动」的关系**：本决策**不推翻**上文「数据面不动」——`AgentEngineKind` 变体集仍然零改动。这里是**新增一列**（经既有 `from` 链升级），不是改枚举值域/重排变体。两者区别：

| | 数据面不动（已决） | 快照加列（本决策） |
|---|---|---|
| 动什么 | `AgentEngineKind` 值域/序号 | `SessionConfigSnapshot` 字段集 |
| 风险 | bincode 序号错位 → 全表报废 | 走 `from` 链，旧行读出 `None` |
| 通道 | **禁止任何改动** | 既有 `native_model` 版本链（6 处先例） |

## 下线面清单（供 proposal 的任务分解参考）

### 实现落点（删）
- `crates/infra/agent/src/cli/`（discover / flags / jsonl / runner + 各自 test，2251 行）
- `lib.rs`：`pub use cli::...` re-export、`EngineKind::Cli` 臂、`ClaudeCliRunner`
- `worker.rs`：`is_cli_engine()` 轮前 `discover()` 预检分支
- `compose.rs`：`resolve_agent_engine` 的 `AgentEngineKind::Cli` 臂（→ `Err("CLI 租户已下线")`）；`begin()` 的 `engine_session_id` 硬门槛分支（退役）；跨引擎续会话校验（视上文拍板处置）
- `Cargo.toml`：检查 `regex` / `glob` 是否为 cli 独占（现状 `glob`/`regex` 仅 `sdk/tools.rs` 消费，cli 反而没用——依赖清理须按实际引用面走，勿照 Cargo.toml 盲删）

### 产品面（改）
- `store/model.rs`：`AgentEngineKind::Cli` 变体**保留**（解码锚，注释标"不再可选"）；`AgentEngineKind` / `EngineKind` 文档注释更新
- `store` 校验：`upsert_agent_instance` 对 Cli 的 provider 可空语义收回（或拒新增 Cli）
- `src/commands/agents/mod.rs`：`save_agent` 拒新增 `engine: Cli`（存量 Cli 行可读不可改）
- **唯一存量数据动作**：全局库 `is_default == true && engine == Cli` → 清 `is_default` 或 `resolve_agent_engine` 缺省路径显式 `Err`
- `src/views/agents/components/agent-panel.tsx`：engine 下拉 `['sdk','cli']` → 仅 sdk；`canSave` 的 `form.engine === 'cli'` 分支；provider 选择器禁用逻辑
- 前端测试：`agent-panel.test.tsx` 的 cli 档用例群（provider 可空提交 / 下拉二值 / 编辑存量 cli 实例）
- 前端测试 mock：`configSnapshot: { engine: 'cli' ... }` 等 mock 断言（若快照 engine 消费面收窄，mock 面同步收）

### 契约面（spec delta）
- `desktop-agent-execution`：「Claude CLI 租户（MVP）」requirement 整段退役；`cli/` 引擎协议实现行、"双引擎租户"措辞、`AgentStartError` CLI 变体、停止语义的 CLI 进程树击杀分支、headless 禁 ask 的 CLI flag 组装段落
- `desktop-agent-management`：engine 二值（`cli` | `sdk`）措辞、"cli 时 provider 可空"校验、解析产物的 `EngineKind`（`cli` | `sdk`）值域
- 双枚举是否合一（`EngineKind` / `AgentEngineKind`）：`EngineKind` 不出 bindings 可塌成单值；`AgentEngineKind` 必须保留两变体（解码锚），二者分离（"store 禁拖引擎门面 crate"的刻意设计）维持

### 线面（bindings，非 golden）
- `AgentEngineKind` 线值 `("cli" | "sdk")` **不演进**（变体保留，值域不变）；若 `save_agent` 拒新增 Cli，参数类型仍是 `AgentEngineKind`（校验层拒绝，非类型层收窄）
- golden 契约**不覆盖 agent 域**（8 份快照只在 change 域），故不走 golden 显式重写流程

### 呈现兜底（无需清理脚本，仅存量 cli 会话的运行时兜底）
- 存量 cli 会话续接 → `resolve_agent_engine` 两臂中 Cli 臂返回 `Err("CLI 租户已下线")`——显式错误，非空指针
- explore 页面：`ExploreRecord.session_id` 指向存量 cli 会话 → `find_session` 可达（会话行未删），但续接会走 Cli 臂 Err
- 会话列表 / 转录面板：cli 会话行照常显示（历史可读），仅续接失败

## 未决问题

1. ~~**清理粒度**~~ **已定**：数据面不动，零清理脚本。
2. ~~**前端是否消费 `remoteSessionId`**~~ **已定**：生产代码零消费；倾向留列。
3. ~~**存量数据规模**~~ **不再是问题**：数据面不动，唯一动作是默认 cli 实例清 `is_default`。
4. ~~**`AgentEngineKind` 边界越界**~~ **已定**：越界①（快照 engine 被当控制流输入）**不动**；快照补 `agent_instance_id` 作显式外键（正解，不删历史事实）。多 agent 是既定方向，agent 实例抽象 MUST NOT 塌成单值。
5. ~~**`EngineKind` 是否塌成单值**~~ **已定**：**保留双值** `EngineKind { Cli, Sdk }`——Cli 臂退役后返回 `Err("CLI 租户已下线")`，枚举本身不塌。与多 agent 方向同理：多租户机制（`AgentRunner` trait + `runner_for` 唯一 match 点）MUST NOT 塌，未来远程 API 租户仍落在协议预留内（见 C 档硬边界）。

**全部问题已收敛，地形完整。下一步：起 `dev-team:phase-proposal` for `retire-cli-engine`**（proposal skill 会把本文提升进 `openspec/changes/retire-cli-engine/explore.md`）。
