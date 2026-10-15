# 提案: retire-cli-engine

> **变更**: retire-cli-engine
> **日期**: 2026-10-13
> **状态**: draft

---

## 问题

`packages/desktop/src-tauri/crates/infra/agent/src/cli/` 是 CLI 租户（本机 Claude Code CLI 引擎）的**实现落点**（2251 行 = 实现约 560 + 测试约 1690），但「CLI 引擎」作为**一枚产品枚举值**贯穿的面远大于目录本身：

1. **枚举值贯穿线格式**：`store::AgentEngineKind { Cli, Sdk }` 以 bincode 变体序号落库（`Cli`=0 / `Sdk`=1），携于 `AgentInstanceRecord.engine` 与 `SessionRecord.config_snapshot.engine` 两张表；`EngineKind { Cli, Sdk }` 门面枚举经 `resolve_agent_engine` 双臂解析。
2. **SDK 已是主线租户**：rig 进程内租户（七工具 + bash + L1-L3 上下文防线 + store 全史转录 resume + AGENT.md 注入）定位为「不依赖本地环境、开箱即用」，`desktop-phase-prompts` 的 14 个相位 prompt 已按桌面七工具适配（MUST NOT 携带插件 Agent / MCP 工具假设）；AGENT.md 明示桌面端已接管相位知识与编排、插件行将整体删除——CLI 租户的产品必要性已经消失，只剩维护面。
3. **契约面多处钉死 CLI**：`desktop-agent-execution`「Claude CLI 租户（MVP）」requirement、`desktop-change-orchestration` V1 边界「引擎恒 CLI（SDK MUST NOT 承载 change 执行）」、`desktop-change-archive` SpecSync 会话「CLI 引擎」措辞——下线的牵动面在枚举值与语义分叉，不在目录本身。
4. **两颗暗雷**：① 删/重排 `AgentEngineKind` 变体 → bincode 序号错位，`AgentInstanceRecord` + `SessionRecord` **两表整体报废（含全部 sdk 数据）**，不是只丢 cli 数据；② 存量 `is_default == true && engine == Cli` 的默认实例会让缺省发起（explore / change run）走进已删除路径。

---

## 提案

**C 档下线（删到契约面）**：删 `cli/` 实现 → 产品面收窄（管理面拒写 cli、前端下拉去 cli）→ 契约面退役（五份 spec delta）。以下硬边界写死，防止「顺手把引擎抽象也塌了」：

1. **退役的是 CLI 这个租户实现，不是多租户机制**——多租户（本机 CLI / 进程内 SDK / 远程 API）协议预留保留；
2. **`AgentRunner` trait / `EngineKind` 参数面 / `runner_for` 唯一 match 点结构 MUST NOT 塌掉**；
3. **`crates/core/agent` 协议面零 diff**（`AgentEvent` 信封、`AgentRunner`、run 状态机维持现状；`AgentStartError` 变体集不动——`CliMissing` 保留为本机 CLI 租户的协议预留词汇，退役后无生产方）；
4. **`AgentEngineKind` 的 `Cli` 变体永久保留**（bincode 解码锚，序号 0 钉死，注释标「解码兼容，不再可选」）；`EngineKind` 保留双值（`Cli` 臂退役为显式 `Err("CLI 租户已下线")`，枚举本身不塌）；
5. **CLI 独占语义逐条确认 SDK 不欠账**：`--resume` → store 全史转录重建、`--disallowedTools AskUserQuestion` → 封闭工具面天然无 ask、`discover()` 轮前预检 → `ConfigMissing` 归 `begin()` open 段、model 双档、`engine_session_id` 留痕（见下）。

**数据面零改动**：不删变体、不重排、零清理脚本（早期讨论的通用升级脚本已评估并移除）。唯一存量动作 = 存量默认 cli 实例的缺省发起**显式 `Err`**（文案引导管理页改默认；`set_default` 切换语义原子清旧默认，一次操作修复）——不写库、不清 `is_default`。

**`engine_session_id` 退役寻址、保留留痕**：`compose.begin()` 的寻址硬门槛（「会话缺少引擎句柄，无法续接」`Err` 分支）随 CLI 退役删除——它在用 CLI 的 `--resume` 寻址语义卡所有引擎，SDK 每轮新铸句柄不消费。双 id 映射留痕保留（`SessionRecord.engine_session_id` + `SessionRow.remoteSessionId` + `bind_session_remote` 全保留）：「这轮落在哪个引擎侧会话」是引擎中立的观测/考古事实。

**快照补实例外键（多 agent 预留）**：`SessionConfigSnapshot` 增 `agent_instance_id: Option<i64>`——把会话↔实例关联从「靠 `engine` 间接猜」改为显式外键（快照复制值，非跨库引用，实例删除不伤历史会话）；`engine` 字段保留（历史事实，不删）。存储演进走既有 `native_model` `from` 链（`SessionRecord` v1→v2，旧快照 `agent_instance_id` 读出 `None`；`AgentProviderRecordV1→V2` 等 6 处先例同模式）；`compose` 的 `ResolvedEngine` 须携实例 id，新会话恒 `Some`。`compose.begin()` 跨引擎续会话校验**不动**（CLI 走后该门自动失效，快照 `engine` 作历史事实保留）。

**契约面一并翻案的两处（explore 清单外、grep 盘点补齐）**：`desktop-change-orchestration` V1 边界「引擎恒 CLI」与 `desktop-change-archive` SpecSync「CLI 引擎」措辞——change 执行会话本就走组合根默认 agent 解析（`compose_turn(..., None)`），CLI 删除后必由 SDK 承载；不改则语料自相矛盾（「SDK MUST NOT 承载 change 执行」对上「CLI 已删」）。附带后果明示：change run 发起自此依赖 SDK 连接配置；protect-files 插件 hook 护栏路径失去载体（缺口显式留痕，桌面原生护栏仍归独立 change）。

---

## 能力

### 新增能力

- 无——本变更全部落点为既有能力的收窄/退役（CLI 租户下线 + 快照外键补强），MUST NOT 新立能力 spec。

### 修改的能力

- **desktop-agent-execution** — CLI 租户退役主战场：「Claude CLI 租户（MVP）」requirement 整段 REMOVED（Reason/Migration）；MODIFIED 九条——「agent 边界与双 crate 分层」（`cli/` 引擎实现行、spawn 落点）、「会话域内核公共 API 与会话一等公民」（双 id 映射寻址语义退役为留痕、快照字段面）、「AgentRunner trait 与逻辑事件流抽象」（两租户措辞→SDK 单租户 + 协议预留）、「agent_stop 终止与提前 resolve」（CLI 进程树击杀分支）、「引擎门面与 EngineKind 参数选择」（`Cli` 臂显式 `Err`、`runner_for` 结构不塌）、「SDK 引擎 bash 工具」（`cli/runner.rs` 悬空引用清理）、「引擎泵一轮一命与 headless 无问收口」（CLI flag 组装段落、`--resume` 寻址口径）、「agent 执行命令面」（错误面与续接口径）、「MVP 边界与已知限制」（CLI 残留边界项）。
- **desktop-agent-management** — engine 写面收窄与解析拒收：「全局 agent 管理命令面」MODIFIED（engine 线值 `cli`|`sdk` 保留但写面仅收 `sdk`、provider 恒必选、「cli 可空」语义退役）；「默认 agent 与运行发起解析」MODIFIED（解析产物值域、`Cli` 实例显式 `Err`、默认标记仅可落 sdk 实例、`runner_for` 结构承诺改述）。
- **desktop-workspace-store** — 模型面与写面：「SessionRecord 模型与会话转录单表」MODIFIED（`config_snapshot` 增 `agent_instance_id`、`engine_session_id` 留痕语义、`SessionRecord` v1→v2 `from` 链）；「AgentProviderRecord 与 AgentInstanceRecord 模型与管理操作面」MODIFIED（`AgentEngineKind` 解码锚不变量、写面拒 `engine: Cli`、`set_default` 拒 Cli）。
- **desktop-change-orchestration** — 「V1 范围与边界留痕」MODIFIED：边界 #1「引擎恒 CLI」翻案为「引擎随默认 agent 解析（SDK 承载）」；边界 #5 protect-files 过渡依赖改写为缺口显式留痕；「引擎边界生效」scenario 重写。
- **desktop-change-archive** — 「spec 同步 agent 与归档链语义自持」MODIFIED：SpecSync 会话「CLI 引擎」措辞改「SDK 引擎（默认 agent 解析；CLI 租户已退役）」。

---

## 变更范围

### 实现文件

以下均相对 `packages/desktop/`：

- `src-tauri/crates/infra/agent/src/cli/` — **整目录删除**（discover / flags / jsonl / runner / mod + 各 `_test`，9 文件 2251 行）
- `src-tauri/crates/infra/agent/src/lib.rs` — 删 `mod cli`、`pub use cli::…` re-export、`ClaudeCliRunner`；`EngineKind` 双值保留（注释标「`Cli` 退役态」）；`runner_for` 的 `EngineKind::Cli` 臂退役为显式 `Err("CLI 租户已下线")` 形态（返回面收 `Result` 或等价显式失败，design 定稿），**唯一 match 点结构不塌**；`ResumeTranscript` / 门面注释口径随行更新
- `src-tauri/crates/infra/agent/src/compose.rs` — `resolve_agent_engine` 的 `AgentEngineKind::Cli` 臂 → `Err("CLI 租户已下线…")`（缺省与显式路径同，文案引导管理页）；`begin()` 的 `engine_session_id` 寻址硬门槛分支删除（`prior_handle` 直取 `record.engine_session_id` Option 透传）；`is_cli_engine()` 退役；`ResolvedEngine` 增 `agent_instance_id`，`begin()` 组装快照填 `Some(id)`；**跨引擎续会话校验不动**
- `src-tauri/crates/infra/agent/src/worker.rs` — `is_cli_engine()` 的 discover 轮前预检分支删除
- `src-tauri/crates/infra/store/src/model.rs` — `SessionConfigSnapshot` 增 `agent_instance_id: Option<i64>`（`#[serde(default)]`）；`SessionRecord` native_model `version = 2, from = SessionRecordV1` + 双向 `From`（旧快照 `agent_instance_id` 填 `None`、降级半边缺省占位）；`AgentEngineKind` 注释「解码兼容，不再可选」；`AgentInstanceRecord` 注释口径更新
- `src-tauri/crates/infra/store/src/store.rs` — `upsert_agent_instance` 拒 `engine: Cli`（新建与更新整行均拒）；`set_default_agent_instance` 拒 Cli 实例；`bind_session_remote` 留痕写面保留
- `src-tauri/src/commands/agents/mod.rs` — save / set_default 的拒 Cli 错误面随 store 单点校验透传（命令体薄包装不变）
- `src-tauri/src/types/generated/bindings.ts` — 经 export-bindings 再生成（非手改；预期仅注释级 diff，`AgentEngineKind` 线值 `("cli" | "sdk")` 不演进、参数类型不收窄——校验层拒绝）
- `src/views/agents/components/agent-panel.tsx` — engine 下拉 `['sdk','cli']` → 仅 `sdk`；`canSave` 的 `form.engine === 'cli'` 分支退役；provider 选择器禁用逻辑退役（恒启用必选）；存量 cli 行只读可删
- `src-tauri/Cargo.toml`（agent crate）— 依赖按实际引用面核查：`regex` / `glob` 现仅 `sdk/tools.rs` 消费，保留勿盲删

### 测试文件

- **删除**：`cli/discover_test.rs` / `flags_test.rs` / `jsonl_test.rs` / `runner_test.rs`（随目录）
- `src-tauri/crates/infra/agent/src/lib_test.rs` / `compose_test.rs` / `worker_test.rs` — Cli 臂显式 Err 用例；discover 预检分支删除回归；**硬门槛退役用例**（缺 `engine_session_id` 的续会话不再 Err，`prior_handle` 透传 `None`）；快照 `agent_instance_id` 填充（缺省/显式路径均 `Some`）；跨引擎校验保持用例
- `src-tauri/crates/infra/store/src/model_test.rs` / `store_test.rs` — `SessionRecord` v1→v2 升级回环（旧快照 `agent_instance_id=None`、其余字段原样）；`upsert_agent_instance` / `set_default_agent_instance` 拒 Cli 用例；**`AgentEngineKind` 变体序号锚定测试**（bincode 字节断言 `Cli`=0 / `Sdk`=1，解码锚回归钉）
- `src-tauri/src/commands/agents/mod_test.rs` — save / set_default 拒 Cli 透传
- `src/views/agents/components/agent-panel.test.tsx` — cli 档用例群翻转（下拉单值 / `canSave` 无 cli 分支 / provider 恒必选 / 存量 cli 行只读保存拒）
- `src/views/agents/hooks/use-agent-instances.test.ts` — cli mock 数据处置
- configSnapshot mock 补 `agentInstanceId` 键：`use-agent-chat.test.ts` / `use-agent-run-history.test.ts` / `agent-debug-view.test.tsx` / `use-session-transcript.test.ts` / `session-transcript-panel.test.tsx` / `change-detail-view.test.tsx` / `detail-drawer.test.tsx` / `use-explore-session.test.ts`
- `src-tauri/src/bindings/mod_test.rs` — bindings 一致性守卫随再生成同步

### 删除文件

- `src-tauri/crates/infra/agent/src/cli/` 整目录（9 文件 2251 行，见「实现文件」）

### 不要修改

- `crates/core/agent/**` — **零 diff 硬边界**：`AgentEvent` 信封 / `AgentRunner` trait / run 状态机 / `AgentStartError` 变体集（`CliMissing` 保留为本机 CLI 租户协议预留词汇，退役后无生产方）/ `TurnRequest.prior_handle` 参数位
- `AgentEngineKind` 变体集与序号（`Cli, Sdk` **永久保留**——bincode 解码锚；删/重排即 `AgentInstanceRecord` + `SessionRecord` 两表报废含 sdk 数据）
- `EngineKind` 双值（`Cli` 臂 Err ≠ 枚举塌单值）；`runner_for` 唯一 match 点结构；`EngineConfig` 三字段
- `SessionRecord.engine_session_id` 列 / `SessionRow.remoteSessionId` 字段 / `bind_session_remote` —— 双 id 映射**留痕保留**（生产代码零消费不删列；删列收益仅少一个 `Option`）
- `compose.begin()` 跨引擎续会话校验（决策① 越界不修）；快照 `engine` 字段（历史事实）
- `SessionEventRecord`（SerdeJson，不触枚举）/ `AgentRunRecord` / `ExploreRecord`（`session_id` 外键已有 `?? null` 兜底）—— 零处理
- golden 线面契约（`core/workflow/tests/golden/` 8 份快照零 `engine` 引用，不涉 golden 重写流程）；agent 域 DTO 演进走 specta 重导出 + bindings 一致性守卫既有通道
- `plugins/dev-team` 全部（版本 2.10.44 与三类交付产物零改动）；`openspec/config.json` 无新增 excludes / 豁免
- 未来清理 cli 残留数据（变体删除/改写）—— 独立 change 走 `native_model` version bump + `From` 映射设计（6 处先例），本变更不预建

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | cli/ 引擎删除面 | `crates/infra/agent/src/cli/` 零残留；`lib.rs` 无 `cli::` / `ClaudeCliRunner` 引用；`cargo test --workspace` 全绿；`regex` / `glob` 按实际引用面保留（现仅 `sdk/tools.rs` 消费） |
| AC-2 | 多租户机制不塌 | `AgentRunner` trait / `EngineKind` 双值 / `runner_for` 唯一 match 点结构零塌；`crates/core/agent` 零 diff（`AgentStartError` 变体集不变）；`AgentEngineKind` 变体集 `Cli, Sdk` 不动 |
| AC-3 | 变体序号锚定 | `AgentEngineKind` bincode 序号断言测试在位（`Cli`=0 / `Sdk`=1 字节级）；含存量两变体行的库 roundtrip 照读零错位 |
| AC-4 | 解析层显式拒绝 | `resolve_agent_engine` 对 `engine: Cli` 实例（缺省与显式路径同）返回 `Err`（文案含「CLI 租户已下线」与管理页引导）；存量 cli 会话续接显式失败（不落库不推流），会话列表 / 转录照常可读 |
| AC-5 | 写面拒 Cli | `save_agent` 新建/更新携 `engine: Cli` 均拒（存量行可读可删不可改）；`set_default_agent_instance` 对 Cli 实例拒；存量默认 cli 实例的缺省发起显式 `Err` 且引导改默认（**不清 `is_default`，零数据写入**；改标 sdk 默认一次修复） |
| AC-6 | 引擎句柄硬门槛退役 | `begin()` 不再因 `engine_session_id` 缺失拒续会话（`prior_handle` Option 透传）；`SessionRecord.engine_session_id` / `SessionRow.remoteSessionId` / `bind_session_remote` 留痕保留 |
| AC-7 | 快照实例外键 | `SessionConfigSnapshot.agent_instance_id` 新会话恒 `Some(实例 id)`（缺省与显式路径同）；存量 `SessionRecord` 经 v1→v2 `from` 链读出 `None`（零迁移代码路径）；前端 configSnapshot mock 补 `agentInstanceId` 键 |
| AC-8 | 产品面收窄 | agent-panel engine 选项仅 `sdk`（无 'cli' 档）；`canSave` 无 cli 分支；provider 选择器恒启用必选；存量 cli 行只读可删 |
| AC-9 | 契约面一致 | 五份 spec delta 落盘（REMOVED 带 Reason/Migration、MODIFIED header 逐字一致、scenario 标记恰 4 个 #）；「引擎恒 CLI」边界与 SpecSync「CLI 引擎」措辞同步翻案；「Claude CLI 租户（MVP）」整段退役 |
| AC-10 | 测试与管线 | 测试文件清单全数更新；`pnpm -C packages/desktop run client:check` 与 `run test` 全绿、`cargo test --workspace` 全绿；desktop 套件覆盖率达阈值（lines 90 / branches 85 / functions 90）、mutation score 达 80；`openspec/config.json` 无新增 excludes |
| AC-11 | 版本交付（归档时执行） | `packages/desktop/package.json` version 0.5.9 → 0.5.10（`tauri.conf.json` 维持 `../package.json` 引用，`src-tauri/Cargo.toml` 不随动）；`plugins/dev-team` 保持 2.10.44 零改动 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| R1 bincode 变体序号误动（删/重排 `Cli`） | `AgentInstanceRecord` + `SessionRecord` 两表整体报废（**含全部 sdk 数据**），非仅 cli 数据 | 极低（知情红线） | 硬边界写死 + AC-3 序号锚定测试；`Cli` 变体注释「解码兼容，不再可选」；未来清理走独立 change |
| R2 protect-files 护栏缺口显式打开 | 插件 hook 路径随 CLI 退役失去载体，步前写保护面仅剩 SDK 路径沙箱（workspace root 前缀校验） | 高（知情接受） | `desktop-change-orchestration` 边界 #5 措辞更新显式留痕；桌面原生护栏（内核 permission 层方向）独立 change 为缺口出口（既有留痕，不隐式吸收） |
| R3 change 执行依赖 SDK 连接配置 | 无默认 sdk agent / provider 未配置时 change run 发起失败（原 CLI 开箱即跑） | 中 | 缺省解析显式 `Err` + 管理页引导（既有错误面）；AC-4/AC-5 锚定文案；边界 #1 改写显式声明依赖 |
| R4 存量默认 cli 实例缺省发起失败 | 用户缺省发起（explore / change run）持续 `Err` 直至改默认 | 中 | `Err` 文案引导管理页；`set_default` 切换语义原子清旧默认（一次操作修复）；`set_default` 拒 Cli 防再坏（AC-5） |
| R5 存量 cli 会话/实例只读的观感 | 历史 cli 会话续接、cli 实例编辑保存被拒，易被误读为回归 | 中 | 呈现兜底明文（列表/转录可读、续接显式 `Err` 属预期）；错误文案指明「CLI 租户已下线」而非泛化失败 |
| R6 `SessionRecord` v1→v2 链漏建/降级半边失真 | 存量会话行读取失败或快照错位 | 低 | 走既有 `native_model` `from` 链（6 处先例）；降级半边缺省占位惯例；AC-7 升级回环测试 |
| R7 前端 mock 面波及 | 8 处 configSnapshot mock 键面漏改致测试红 | 低 | AC-7/AC-10 显式清单；`client:check` / `test` 全绿关卡 |
| R8 `runner_for` Cli 臂形态误用 panic | 若取 `unreachable!` 且未来出现旁路构造 Cli → panic 面 | 低 | 归 design 定稿（倾向显式 `Err` 形态）；红线仅锁结构不塌（AC-2） |
| R9 依赖盲删 | `regex`/`glob` 照 Cargo.toml 盲删而 `sdk/tools.rs` 仍消费 → 编译红 | 低 | AC-1 按实际引用面核查（现状两依赖仅 sdk 消费） |
| R10 双枚举合一/单值化误判 | 未来顺手把 `AgentEngineKind` 或 `EngineKind` 塌单值/重排 | 低 | 硬边界 + AC-2/AC-3 钉住；spec delta 明文「变体集与序号 MUST NOT 增删重排」 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|------|------|------|----------|
| 下线深度 | C 档删到契约面 | A 留「枚举值在、无实现」僵尸态、B 留存量 redb 解码雷 | A 只删实现（否）/ B 删到产品面（否） |
| 数据面 | 零改动，`Cli` 变体永久保留（解码锚） | bincode 写序号不写名字：删变体 → 越界**全表报废含 sdk**；重排 → 静默反向污染 | 删变体 + 清理脚本（撞编码事实，否） |
| `engine_session_id` | 退役寻址硬门槛、保留双 id 映射留痕 | 硬门槛在用 CLI 的 `--resume` 寻址语义卡所有引擎；SDK 每轮铸造不消费；留痕是引擎中立事实 | 整删列（留痕价值真实 + mock 面大，否）/ 保留硬门槛（否） |
| 快照 `agent_instance_id` | 增 `Option<i64>` 实例外键 | 「引擎是实例属性」命题下会话关联应显式外键；多 agent 既定方向的关联键 | 只靠 `engine` 猜关联（语义漂移，否） |
| 跨引擎续会话校验 | 不动（决策① 越界不修） | CLI 走后此门自动失效；快照 `engine` 作历史事实有考古价值 | 一并重构为实例 id 校验（收益≈0，否） |
| 双枚举合一 | 不合一 | store 禁拖引擎门面 crate（零新增依赖）；`AgentEngineKind` 需解码锚 | 合一（否） |
| `EngineKind` 塌单值 | 保留双值，`Cli` 臂显式 `Err` | 多租户机制不塌（未来远程 API / 本机 CLI 租户仍落协议预留） | 塌单值（否） |
| 存量默认 cli 实例 | 不清数据，resolve 显式 `Err` + 引导 | 数据面零写入哲学；`set_default` 切换语义天然修复 | 清 `is_default`（需数据写入通道，否） |
| 写面拒 Cli 形态 | save / set_default 双口拒；存量行可读可删不可改 | 与「CLI 租户已下线」错误面一致，防造出必失败的缺省发起 | 仅收回 provider 可空语义（留僵尸档，否） |
| change 执行引擎边界 | 随 CLI 退役一并翻案（desktop-change-orchestration / change-archive） | 不改则语料自相矛盾（「SDK MUST NOT 承载」对上「CLI 已删」） | 留独立 change（归档即矛盾，否） |
| 呈现兜底 | 存量 cli 会话/实例只读可查、续接显式 `Err` | 历史可读不伤；自动迁移为 sdk 属臆测语义 | 自动迁移（否） |

### 待决问题（归 design）

- `runner_for` 的 `EngineKind::Cli` 臂具体形态（`Result` 包装 vs 显式失败 stub vs `unreachable!`——倾向显式 `Err`；红线仅锁唯一 match 点结构与 `EngineKind` 双值不塌）
- `begin()` 硬门槛退役的落点形态（纯删 vs 降级为不阻断续会话的观测留痕）
- 「CLI 租户已下线」错误文案定稿（管理页引导措辞）
- agent-panel 存量 cli 行的只读呈现形态（编辑入口禁用 vs 表单只读 + 保存拒）
