# desktop-workspace-store Specification (Delta)

## MODIFIED Requirements

### Requirement: AgentRunRecord 状态字段类型化

`AgentRunRecord` 的 `env` / `permission_mode` / `status` 三字段 SHALL 类型化为 core/agent 枚举（`AgentEnvMode` / `AgentPermissionMode` / 新增 `AgentRunStatus`），MUST NOT 再以 `String` 承载受控值域；「受控字符串」模型约定（store 不引本地枚举）SHALL 退役。`AgentRunStatus` SHALL 定义于 `crates/core/agent`（与既有两枚举同列），变体为 `Running` / `Completed` / `Failed` / `Stopped` / `Interrupted`（camelCase serde rename 与现值域逐字一致；`Interrupted` 为**末位追加**——落库 bincode 变体索引只增不改，旧行解码值不变，零版本递增、零迁移）。`Interrupted` 的语义 SHALL 与 change run 域 `RunStatus::Interrupted` 同义：轮因桌面进程中断而客观终止；该值 SHALL 只由启动标定写入（见「agent 轮启动标定与 running 残留终结」），运行期写路径（轮收口 / 陈旧轮清理）MUST NOT 产生。

serde camelCase 线格式 SHALL 与枚举化前逐字不变（unit variant + `rename_all = "camelCase"`：`AcceptEdits` → `"acceptEdits"` 等）；落库编码形态若因枚举化变化（bincode 的 String 长度前缀与变体索引不等价，变化必然发生），SHALL 经 native_model 版本机制原地演进（`AgentRunRecord` 版本递增，`from` 枚举化前旧形态自动升级），MUST NOT 引入手工迁移步骤或 legacy 迁移层。枚举化前 SHALL 先完成存量库三字段值域扫描，确认无野值；转换遇野值的处理策略（报错 vs 兜底）由 design 定夺。`source` / `source_ref` SHALL 保持 `String`（两侧均为 string，不构成类型降级；收紧另开 change）。

落库写入路径 MUST NOT 再经 `as_str().to_owned()` 字符串降级（直写枚举）；`as_str` 的「与 serde 线格式一致」双轨口径 SHALL 退役。CLI flag 组装等真需字符串值处 SHALL 经单一来源取得，取值行为不变（design 定夺实现形态）。

#### Scenario: 线格式逐字不变

- **WHEN** 对枚举化后的 `AgentRunRecord` 做 serde JSON 序列化并与枚举化前值域比对
- **THEN** `env` / `permission_mode` / `status` 输出逐字一致（`"default"` / `"acceptEdits"` / `"running"` 等），IPC 与前端所见值域不变

#### Scenario: 存量记录自动升级

- **WHEN** 打开含枚举化前形态 `AgentRunRecord` 的存量库并重放历史 run
- **THEN** 记录经 native_model 版本机制自动升级可读，三字段值域正确还原，无手工迁移步骤、无 legacy 迁移层

#### Scenario: 存量值域扫描先行

- **WHEN** 执行枚举化落地
- **THEN** 存量库 `env` / `permission_mode` / `status` 三字段值域扫描先行完成并确认无野值，扫描结论留档

#### Scenario: 写入不再字符串降级

- **WHEN** 审查 run begin / finish 的落库写入路径
- **THEN** 三字段直写枚举，无 `as_str().to_owned()` 降级；`STATUS_RUNNING` 等裸字符串常量随之退役

#### Scenario: 前端 union 不降级

- **WHEN** 审查生成物中 `AgentRunRecord` 的 `status` / `env` / `permissionMode` 类型
- **THEN** 为字面量 union（`'running' | 'completed' | 'failed' | 'stopped' | 'interrupted'` 等），非 `string`——Rust 枚举成为单一事实源，TS 侧类型强度不降

#### Scenario: interrupted 值域往返与旧行解码

- **WHEN** 把一个 `status = Interrupted` 的轮统计行写入库再读出，并对一条中断值引入前落库的旧行（`status` 取既有四值之一）解码
- **THEN** 线格式为 `"interrupted"`、往返后枚举值不变；旧行解码得值与原四值逐一相同（变体索引不位移），无版本递增、无迁移步骤

## ADDED Requirements

### Requirement: agent 轮启动标定与 running 残留终结

store SHALL 提供 agent 轮启动标定操作 `calibrate_interrupted_agent_turns(now)`，在**所属 workspace 库首次打开**时执行——调用点与既有 change run 标定 `calibrate_interrupted_runs` 同址同层（`open_workspace`，先例见 desktop-change-state-store「启动标定与 active_phase 悬挂处置」）。判据是**真事实而非猜测**：启动瞬间 `AskRegistry` / `StopRegistry` / `SessionStreamRegistry` 均为空 ⇒ 库中仍为 `status = running` 的轮在数学上全部可证已死；判据 MUST NOT 依赖「该轮卡在哪个工具」（ask 只是工具之一，工具执行体三条动作注册 / 阻塞 / 回灌全在内存面，「仍在执行」在库里没有任何 durable 痕迹）。

标定语义 SHALL 为：

- **扫描面**：只扫 `AgentRunRecord.status == Running` 的行；MUST NOT 触碰 change run 域 `RunRecord`（由既有标定负责）、MUST NOT 触碰 `SessionRecord` 与 sealed 转录（转录是事实，不改写）；
- **写入**：`status` → `Interrupted`、`finished_at` → `Some(now)`、`error` → 记因定式（「重启标定：桌面进程中断，会话轮客观已终止」）；统计三字段（`num_turns` / `cost_usd` / `duration_ms`）MUST NOT 虚构；
- **幂等**：读扫零残留即不开写事务（沿既有标定「先读扫、有残留才写」两段形态）；二次调用零变化；已 `interrupted` 的行 MUST NOT 被二次改写；
- **不动已终态**：`completed` / `failed` / `stopped` 行 MUST NOT 被改写（标定只终结「未收口」的轮）；
- **返回**：被标定轮 id 清单（`pub`，测试与语料构造中断样本直调，沿既有标定形态）。

`interrupted` SHALL 是 agent 轮状态的**唯一非运行期值**：标定是它唯一的写入点。由此「这一轮还在跑吗」只有一个权威答案（库态），会话查询面（`session_detail` / `SessionSummary.turns`）与 `AgentRunStatus` 前端 union 自动跟随，无需第二套「是否存活」判据。

#### Scenario: 残留 running 轮被标定

- **WHEN** 构造库中含 `status = running` 的 agent 轮行（模拟工具执行中进程死亡）后执行启动标定
- **THEN** 该行翻为 `interrupted`、`finished_at` 落标定时刻、`error` 为标定记因定式；统计三字段保持原值（不虚构）；同会话的其他已终态轮行零变化

#### Scenario: 幂等与零写入

- **WHEN** 对同一库连续两次执行标定
- **THEN** 第一次返回被标定轮 id 清单，第二次返回空清单且库内零写入（无残留时不开写事务）；已 `interrupted` 的行不被二次改写

#### Scenario: 不误伤 change run 与转录

- **WHEN** 库中同时存在 `RunRecord.status = running`（change 流程残留）与 sealed 转录事件后执行 agent 轮标定
- **THEN** 只有 `AgentRunRecord` 行被翻转，`RunRecord` 状态原样保留（交由既有 change run 标定），转录事件零改动

#### Scenario: 运行期零写入

- **WHEN** 审查轮收口（`finish_agent_turn`）与陈旧轮清理等运行期写路径
- **THEN** 无任何运行期路径写入 `interrupted`；该值只出现在标定实现与标定后的库读史中

## Module Contract

### Function: `crates/infra/store/src/store.rs`

| 项 | 签名 / 变化 | 描述 |
|------|------|------|
| `calibrate_interrupted_agent_turns` | `pub fn calibrate_interrupted_agent_turns(&self, now: i64) -> Result<Vec<i64>, StoreError>` | 启动标定：只扫 `AgentRunRecord.status == Running`，翻 `Interrupted` + `finished_at` + 记因；读扫零残留不开写事务；幂等；返回被标定轮 id 清单 |
| `open_workspace` | 追加一次调用 | 与既有 `calibrate_interrupted_runs` 同层同址（吞错形态沿用现状——播种修复是不依赖标定成功的独立防线，见 desktop-agent-chat-infra） |

### Enum: `crates/core/agent/src/runner.rs`

| 项 | 变化 | 描述 |
|------|------|------|
| `AgentRunStatus` | 末位增 `Interrupted` | 既有契约行「四变体」口径随本 delta 扩为五变体（`Running` / `Completed` / `Failed` / `Stopped` / `Interrupted`，序不变）；线格式 `"interrupted"`；bincode 变体索引只增不改 ⇒ 旧行解码值不变；仅启动标定写入 |

### Type: `packages/desktop/src/types/generated/bindings.ts` + `bindings.zod.ts`

| 项 | 变化 | 描述 |
|------|------|------|
| `AgentRunStatus` | 再生成 | TS union 增 `"interrupted"`、zod literal union 随动；`TurnSummary.status` / `AgentRunRecord.status` 自动跟随，命令签名零改动 |

### 零改动面

| 模块 | 说明 |
|------|------|
| `crates/infra/store/src/model.rs` | `AgentRunRecord` 字段面零改动（值域由枚举承载）；仅随枚举更新注释 |
| `RunRecord` / `calibrate_interrupted_runs` | change run 域标定与状态词面零改动 |
| `SessionRecord` / `SessionEventRecord` | 标定不改写会话行与转录（转录不可变） |
| `finish_agent_turn` / `begin_agent_turn` | 运行期写路径零改动（MUST NOT 写入 `interrupted`） |
