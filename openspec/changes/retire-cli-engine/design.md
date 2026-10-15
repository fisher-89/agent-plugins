# 设计: retire-cli-engine

> **变更**: retire-cli-engine
> **日期**: 2026-10-13

---

## 提案与规格同步状态（信息性）

- 本变更的五份能力 spec delta（`desktop-agent-execution` / `desktop-agent-management` / `desktop-workspace-store` / `desktop-change-orchestration` / `desktop-change-archive`）已随提案阶段写入本变更目录的 `specs/`，属提案层产物，**不进变更清单与任务**（AC-9 的「五份 spec delta 落盘」已完成；实现阶段仅代码面收窄，不新增/改写 delta）。
- 实施边界重申（proposal 硬边界）：退役的是 CLI 租户**实现**，不是多租户机制——`AgentRunner` trait / `EngineKind` 双值 / `runner_for` 唯一 match 点结构 / `crates/core/agent` 零 diff / `AgentEngineKind` 变体集与 bincode 序号 MUST NOT 动。

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| CLI 租户实现（退役删除） | 本机 Claude Code CLI 引擎（discover / flags / jsonl / runner + 各 `_test`，9 文件 2251 行）——整目录删除；`kill_process_tree` 的 `taskkill /T /F` 形状已由 `sdk` 侧 `exec::kill_tree` 唯一持有，无迁出物 | `packages/desktop/src-tauri/crates/infra/agent/src/cli/` | agent（core 契约）、tokio、serde_json | （删除） |
| 引擎门面（收窄） | `EngineKind` 双值（`Cli` 退役态）+ `runner_for` 唯一 match 点；Cli 臂退役为显式 `Err`（返回面收 `Result`）；`ClaudeCliRunner` / `pub use cli::…` re-export 删除；注释口径随行 | `packages/desktop/src-tauri/crates/infra/agent/src/lib.rs` | agent（core `AgentRunner`）、sdk 引擎 | Rust trait object + match 分发 |
| 组合根解析与快照组装 | `resolve_agent_engine` 的 `AgentEngineKind::Cli` 臂 → `Err`（缺省与显式路径同臂）；`ResolvedEngine` / `ComposedTurn` 携 `agent_instance_id`；`begin()` 引擎句柄寻址硬门槛删除（`prior_handle` Option 透传）+ 快照填 `Some(id)`；`is_cli_engine()` 退役；跨引擎续会话校验零改动 | `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs` | store（全局库 + workspace 库）、门面、core 内核 | 组合根单点 + `serde_json::Value` 不透明快照 |
| Worker 执行器（收窄） | `is_cli_engine()` 的 discover 轮前预检分支删除（`ConfigMissing` 归 `begin()` open 段） | `packages/desktop/src-tauri/crates/infra/agent/src/worker.rs` | compose、orchestration port | async 执行体 |
| store 模型面 | `SessionConfigSnapshot` 增 `agent_instance_id: Option<i64>`（快照复制值的显式外键）；`SessionRecord` v1→v2 `from` 链（legacy 双结构 + 双向 `From`）；`AgentEngineKind` 解码锚注释；`AgentInstanceRecord` 注释口径；`CLI_TENANT_RETIRED` 共享错误文案常量 | `packages/desktop/src-tauri/crates/infra/store/src/model.rs` | native_db / native_model / serde | bincode 1.3 + native_model 版本链 |
| store 写面校验单点 | `upsert_agent_instance` 拒 `engine: Cli`（入参口 + 存量行口双拒）、`set_default_agent_instance` 拒 Cli 实例；`bind_session_remote` 留痕写面零改动 | `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | model | 单写事务校验 |
| 管理命令面（薄包装） | save / set_default 拒 Cli 错误面随 store 单点校验透传（命令体三件事纪律不变，签名与参数类型不收窄） | `packages/desktop/src-tauri/src/commands/agents/mod.rs` | store、tauri-specta | `Result<T, String>` 模板 |
| 管理页产品面收窄 | engine 下拉收单值 `sdk`；provider 选择器恒启用必选；`canSave` 的 cli 分支退役；存量 cli 行「编辑」「设为默认」禁用、「删除」保留 | `packages/desktop/src/views/agents/components/agent-panel.tsx` | bindings、既有 hooks | React 受控表单 |
| IPC 类型镜像（生成物） | 经 export-bindings 再生成（非手改）；`AgentEngineKind` 线值 `("cli" \| "sdk")` 不演进、参数类型不收窄 | `packages/desktop/src/types/generated/bindings.ts`、`packages/desktop/src-tauri/src/bindings/mod_test.rs` | export-bindings crate | specta 重导出 + 一致性守卫 |
| core 会话内核（零 diff 硬边界） | `AgentEvent` 信封 / `AgentRunner` / run 状态机 / `AgentStartError` 变体集（`CliMissing` 保留为协议预留词汇，退役后无生产方）/ `TurnRequest.prior_handle` 参数位全部不动 | `packages/desktop/src-tauri/crates/core/agent/**` | — | 零 diff 守线断言 |

---

## 设计定值（design 拍板）

| 项 | 定值 | 依据 |
|----|------|------|
| DD1 `runner_for` 的 `EngineKind::Cli` 臂形态（收口待决 #1） | **`Result` 包装**：签名改 `pub fn runner_for(&self, kind: EngineKind, engine_cfg: EngineConfig) -> Result<Box<dyn AgentRunner>, String>`；`EngineKind::Cli` 臂 `Err(CLI_TENANT_RETIRED.to_owned())`、`EngineKind::Sdk` 臂 `Ok(Box::new(SdkRunner::new(...)))` | proposal 倾向显式 `Err`；R8 明示弃 `unreachable!`（旁路构造 panic 面）；`Result` 是唯一同时满足「显式失败面」与「唯一 match 点结构不塌」的形态。生产路径不可达（resolve 先行拒 Cli），该臂为结构保留 + 防御兜底 |
| DD2 `begin()` 硬门槛退役形态（收口待决 #2） | **纯删 + Option 透传**：`SessionRef::Continue` 分支的 `prior_handle` 直取 `record.engine_session_id.clone()`（`Option<String>` 原样透传），不降级为观测留痕 | proposal 实现文件条目已定「硬门槛分支删除（`prior_handle` 直取 Option 透传）」；观测留痕已由 `bind_session_remote` 落库半边承载，再造观测点零增量 |
| DD3 「CLI 租户已下线」错误文案（收口待决 #3） | 共享常量（`crates/infra/store/src/model.rs`，经 `store` lib re-export）：`pub const CLI_TENANT_RETIRED: &str = "CLI 租户已下线：请前往 Agent 管理页（/agents）新建或改用 sdk agent，并将其设为默认（存量 cli 实例可读可删不可改）"`；四个拒点（`runner_for` / `resolve_agent_engine` / `upsert_agent_instance` / `set_default_agent_instance`）逐字复用、不加前后缀 | 同一产品事实同一文案，防多处措辞漂移；常量落 store（依赖方向 agent-runtime → store，store 不能反向取词）；文案含「CLI 租户已下线」+ 管理页引导（AC-4）+ 改默认引导（AC-5） |
| DD4 存量 cli 行呈现形态（收口待决 #4） | **编辑入口禁用**为主形态：`AgentListItem` 对 `engine === 'cli'` 行禁用「编辑」与「设为默认」按钮（title 提示「CLI 租户已下线，实例只读」），「删除」保留；表单不进 cli 编辑态。后端 save / set_default 拒为兜底防线（API 直调仍拒，文案同 DD3） | 「存量行可读可删不可改」；表单只读 + 保存拒（备选）会把用户引到必失败的保存动作，误读为回归（R5）；双层（入口禁用 + 写面拒）互为兜底 |
| DD5 拒 Cli 校验落点 | store 写面单点：`upsert_agent_instance` **双口拒**——①入参 `agent.engine == Cli`（新建与更新均拒）；②`id > 0` 且存量行 `engine == Cli`（整行不可改，含「改 engine 为 sdk 的整行更新」）；`set_default_agent_instance` 对目标实例 `engine == Cli` 拒。命令面零校验逻辑、纯透传 | desktop-agent-management「校验在 store 单点，命令面透传」；spec 场景明示「含改 engine 为 sdk 的整行更新」亦拒 |
| DD6 `SessionRecord` 演进形态 | `#[native_model(id = 7, version = 2, from = SessionRecordV1)]` + legacy 双结构 `SessionRecordV1`（`#[native_model(id = 7, version = 1)]`，`config_snapshot: SessionConfigSnapshotV1`）与 `SessionConfigSnapshotV1`（旧三字段）+ `From` 双向（升级半边 `agent_instance_id = None`、降级半边丢弃字段） | 既有 `AgentProviderRecordV1→V2` 等 6 处 `from` 链先例；bincode 嵌套结构变布局必须整结构换型（serde `default` 不救 bincode 缺字段），故旧快照需独立 V1 形态 |
| DD7 快照字段定序 | `SessionConfigSnapshot` 新字段 `agent_instance_id: Option<i64>` 置 `engine` 之后（关联键与引擎语义相邻）；`#[serde(default)]` 承接旧 JSON 形态（`SessionRow.configSnapshot` 载荷与前端 mock 的 `agentInstanceId` 键随 serde camelCase 自然出线） | v2 bincode 布局以本定序为准；serde default 兼容 `store_port.rs` 的 `serde_json::from_value` 旧 Value 与测试构造 |
| DD8 实例 id 载荷形态 | `ResolvedEngine.agent_instance_id: i64` / `ComposedTurn.agent_instance_id: i64`（实例 id 恒在，非 `Option`）；`begin()` 组装快照恒填 `Some(self.agent_instance_id)`（缺省与显式路径同——两路径同经 `resolve_agent_engine` 取实例） | 「新会话恒 Some、存量行 None」（AC-7）；快照侧 `Option` 仅为存量行缺列语义 |
| DD9 `compose_turn` 的双臂 match 保留 | `store_kind / models` 的 `match resolved.kind` 保持双臂（`EngineKind::Cli` 臂为不可达死臂，映射 `(AgentEngineKind::Cli, None)` 原样，**不引入 panic 形态**） | R8 同理；多租户机制不塌（未来远程 API / 本机 CLI 租户仍落协议预留），死臂零成本 |
| DD10 跨引擎续会话校验零改动 + R5 文案张力裁决 | `begin()` 的 `config_snapshot.engine != self.store_kind` 校验**逻辑与文案均不动**（「不要修改」硬边界优先）。CLI 走后 `store_kind` 恒 `Sdk`，该门仅对存量 cli 会话续接触发 `Err("会话非当前引擎产出（跨引擎续会话不支持）")`——恰为 AC-4「存量 cli 会话续接显式失败」的落点（不落库不推流）。R5 的「文案指明 CLI 租户已下线」指引由 DD3 四拒点承载；会话侧 gate 文案不动的差异在此留痕 | proposal「不要修改」清单钉死该校验；AC-4 的文案要求明文只约束 `resolve_agent_engine`，「存量 cli 会话续接显式失败」无文案约束；硬边界与风险缓解冲突时以硬边界为准 |
| DD11 `EngineConfig` 与依赖面 | `EngineConfig` 字段面零改动（`empty()` 保留——测试与未来租户占位仍消费）；`regex` / `glob` 保留（现仅 `sdk/tools.rs` 消费）；cli 删除不带走任何依赖条目（tokio / serde_json / agent 均 sdk 共用） | proposal 硬边界 + R9（勿盲删） |
| DD12 版本交付 | `packages/desktop/package.json` `version` 0.5.9 → 0.5.10（归档时执行）；`tauri.conf.json` 维持 `../package.json` 引用、`src-tauri/Cargo.toml` 不随动；`plugins/dev-team` 保持 2.10.44 零改动 | AC-11 |

---

## 变更清单

<!-- 以文件为入口逐层展开，实现阶段以此清单为边界。覆盖 proposal「变更范围 - 实现文件」全部条目；标注「清单外补入」的文件为级联触点，均附理由。纯编译面机械随动与测试相位承接的测试文件（agent crate 的 lib_test / compose_test / worker_test / sdk/runner_test / sdk/config_test、store crate 的 model_test / store_test / store_port_test / envelope_test、壳层 db|explores|workspaces mod_test、agents mod_test 拒 Cli 新用例群、前端 configSnapshot mock 面测试文件）按决策留痕 #6 界线不逐行列出——编译面随动见 tasks 各阶段条目，新增覆盖归 test-design → test-gen → test-execution；生成物（bindings.ts / bindings mod_test.rs）与保绿随动（exec / change_flow mod_test.rs）例外入表。 -->

### 新增文件

<!-- 无新增文件：legacy 解码结构（SessionRecordV1 / SessionConfigSnapshotV1）与共享文案常量均落既有 model.rs（6 处 from 链先例同文件收拢），退役为纯收窄型变更。 -->

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src-tauri/crates/infra/agent/src/lib.rs` | 删 `mod cli;`、`pub use cli::runner::ClaudeCliRunner;`、`pub use cli::{discover, flags, jsonl, runner};`；`runner_for` 返回面改 `Result<Box<dyn AgentRunner>, String>`（DD1）；`EngineKind` / `ResumeTranscript` / `EngineFacade` / `new` / `with_resume_transcript` 的「CLI 臂不消费 / cli 引擎持有不消费 / 不传即 CLI 租户」注释口径改为退役态措辞（`Cli` 变体标「退役态·协议预留」） | 引擎门面收窄；`EngineKind` 双值与唯一 match 点结构不塌（AC-2） |
| `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs` | `resolve_agent_engine` 的 `AgentEngineKind::Cli` 臂 → `Err(CLI_TENANT_RETIRED.to_owned())`（缺省与显式路径同臂，`EngineConfig::empty()` 占位构造随删）；`ResolvedEngine` 增 `agent_instance_id: i64`；`ComposedTurn` 增 `agent_instance_id: i64` 私有字段并接线；`compose_turn` 调 `runner_for` 处 `?` 传导 `Err`、`store_kind / models` 双臂 match 保留（DD9）；`begin()` 硬门槛分支删除（`prior_handle` 直取 `record.engine_session_id.clone()` Option 透传，DD2）、快照组装增 `agent_instance_id: Some(self.agent_instance_id)`（DD8）、跨引擎续会话校验行零改动（DD10）；`is_cli_engine()` 方法删除；「CLI 引擎持有不消费 / CliMissing 收敛」注释口径随行 | 组合根解析显式拒绝 + 快照实例外键 + 硬门槛退役三线落点（AC-4 / AC-6 / AC-7） |
| `packages/desktop/src-tauri/crates/infra/agent/src/worker.rs` | `execute()` 的 `if composed.is_cli_engine() { crate::discover::discover() … }` 预检分支与「CLI 预检」注释删除 | discover 轮前预检随 CLI 退役（AC-1）；`ConfigMissing` 归 `begin()` open 段 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/policy.rs`（清单外补入·注释级） | 模块头「档位语义与 CLI 租户的 permission-mode 对齐」措辞改中立口径（档位枚举为 core 契约、无 CLI 对齐对象） | 悬空引用清理（desktop-agent-execution「SDK 引擎 bash 工具」MODIFIED 随行项） |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs`（清单外补入·注释级） | `// 事件通道有界容量（与 CLI 泵同策略…）` 注释改中立口径（背压阻塞 send 语义本身零改动） | 同上：cli 删除后「CLI 泵」引用悬空 |
| `packages/desktop/src-tauri/crates/infra/agent/src/store_port.rs`（清单外补入·注释级） | `store_record` doc「store 定型三面」→「store 定型四面」（快照字段面 `agent_instance_id` 扩张的注释随行，逻辑零改动） | 快照字段面扩张的级联注释触点（tasks 阶段三已声明） |
| `packages/desktop/src-tauri/crates/infra/store/src/model.rs` | `SessionConfigSnapshot` 增 `agent_instance_id: Option<i64>`（`#[serde(default)]`，DD7 定序）；新增 `SessionConfigSnapshotV1`（旧三字段 legacy）与 `SessionRecordV1`（v1 形态、`config_snapshot: SessionConfigSnapshotV1`）+ `From` 双向（升级填 `None` / 降级丢弃）；`SessionRecord` 改 `#[native_model(id = 7, version = 2, from = SessionRecordV1)]`；新增 `pub const CLI_TENANT_RETIRED: &str`（DD3）；`AgentEngineKind` 注释标「解码兼容，不再可选」（变体集与序号零改动）；`AgentInstanceRecord` 注释口径更新（「cli 可空」语义退役、存量行只读） | 快照实例外键 + 版本链 + 解码锚红线 + 共享文案（AC-7 / AC-3 / AC-5） |
| `packages/desktop/src-tauri/crates/infra/store/src/lib.rs`（清单外补入） | re-export `CLI_TENANT_RETIRED`（compose / 门面经 `store::CLI_TENANT_RETIRED` 消费） | 级联触点：常量须出 crate 边界才可被 agent-runtime 复用 |
| `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | `upsert_agent_instance` 增拒 Cli 双口（DD5：入参 `engine == Cli` → `Err`；更新臂存量行 `engine == Cli` → `Err`）+ doc 注释「cli 可空」口径更新；`set_default_agent_instance` 对目标 `engine == Cli` → `Err` + doc 随行；`bind_session_remote` 留痕写面零改动 | 写面拒 Cli 单点（AC-5）；双 id 映射留痕保留（AC-6） |
| `packages/desktop/src-tauri/src/commands/agents/mod.rs` | `save_agent_instance` / `set_default_agent_instance` 的 doc 注释补「携 `engine: Cli` / Cli 实例拒绝（存量行可读可删不可改）」口径；命令体零改动（拒 Cli 经 store 校验 `Err(String)` 透传，签名与 `AgentEngineKind` 参数类型不收窄） | 管理命令面错误口径（AC-5）；校验层拒绝、非类型层收窄（bindings 零演进） |
| `packages/desktop/src/views/agents/components/agent-panel.tsx` | `ENGINE_OPTIONS` 收单值 `[{ value: 'sdk', label: 'sdk' }]`（select 形态保留，下拉无 `cli` 档）；`ProviderSelect` 删 `engine` prop 与 `disabled={engine === 'cli'}`（恒启用必选）；`canSave` 改 `form.name.trim() !== '' && form.providerId !== null`（cli 分支退役）；`AgentListItem` 对 `record.engine === 'cli'` 行禁用「编辑」「设为默认」按钮（title 提示「CLI 租户已下线，实例只读」），「删除」保留（DD4）；`AgentForm` / 模块头「engine 二值下拉」doc 口径随行 | 产品面收窄（AC-8）；存量 cli 行只读可删 |
| `packages/desktop/src/types/generated/bindings.ts` | 经 export-bindings 再生成（非手改）；预期仅注释级 diff，`AgentEngineKind` 线值 `("cli" \| "sdk")` 不演进、`saveAgentInstance` 参数类型不收窄 | IPC 镜像走 specta 重导出 + bindings 一致性守卫既有通道（非 golden） |
| `packages/desktop/src-tauri/src/bindings/mod_test.rs` | 随再生成同步（Rust 侧快照镜像，非手改） | 生成物强耦合的快照随动 |
| `packages/desktop/src-tauri/Cargo.toml` | 依赖引用面核查（预期零 diff）：`[workspace.dependencies]` 的 `regex` / `glob` 条目保留——现仅 `sdk/tools.rs` 消费（R9 勿盲删）；无 cli 独占条目 | AC-1 依赖核查线 |
| `packages/desktop/src-tauri/crates/infra/agent/Cargo.toml`（清单外补入） | 同上核查（预期零 diff）：`regex` / `glob` / `tokio` / `serde_json` 均 sdk 或门面共用，cli 删除不带走任何依赖条目 | 级联触点：workspace 条目的实际消费声明面 |
| `packages/desktop/src-tauri/crates/infra/checks/src/mutation/cargo_mutants.rs`（清单外补入·测试管线修复） | cargo-mutants 段执行面对齐真实工具（cargo-mutants 27.1.0）实测行为：①argv 模板增 `--workspace`（suite 根为混合 workspace，缺省只扫根包——`crates/**` 业务主场永不在突变面）；②段报告权威落位改 `mutants.out/outcomes.json`（运行面 stdout 只有人类可读进度行，`--json` 仅作用于 `--list`——原「stdout JSON 载荷」口径对真实工具恒不可满足，成功运行也会被误判中断）；③spawn 前清场 `mutants.out`（陈旧报告不误读）；④退出 0 且无报告 = diff 零可突变交叠（cargo-mutants EmptyDiff / NoSourceFiles / NoMutants 恒退出 0 不产报告）→ 零突变体段合成空计数面，不再误判 `Interrupted` | 本变更 test-execution 门 mutation 相位首跑即以此失败（「cargo-mutants 段执行失败（退出码 0，未产出可解析 JSON 段报告）」）——AC-10 mutation 门在位的前置修复；行为语义与 Stryker 段报告文件面同约定 |
| `packages/desktop/src-tauri/crates/infra/checks/src/mutation/segments.rs`（清单外补入·测试管线修复） | `slice_patch` 增 suite 根前缀剥离：清单匹配恒项目根相对、落盘段补丁的 `diff --git a/… b/…` / `--- a/…` / `+++ b/…` 路径行剥 suite 根前缀——cargo-mutants `--in-diff` 以 spawn cwd 相对路径对源树匹配，项目根相对路径原样传入恒零命中（与 Stryker `mutate` 清单 `relative_to_cwd` 同约定） | 同上：段补丁路径不剥前缀则 rust 突变面恒空（「No mutants to filter」空转） |
| `packages/desktop/src-tauri/crates/infra/checks/src/mutation/mod.rs`（清单外补入·测试管线修复） | `execute_segment` CargoMutants 臂计算 suite 执行目录相对项目根的 POSIX 路径传 `slice_patch`（suite 锚项目根时空串透传） | 同上：段补丁路径重写基准 |
| `packages/desktop/src-tauri/crates/infra/checks/src/mutation/cargo_mutants_test.rs`（清单外补入·测试管线修复） | 测试随行：argv 模板断言更新（十参含 `--workspace`）、canned 换真实 `outcomes.json` 全形态解析对拍、零突变体段 / 陈旧报告清场两新用例、`json_payload`（stdout 提取）用例随实现删除 | 被测面契约演进的测试随行（既有 Mock 装置零扩展——报告文件面经 `FAKE_SPAWN_CARGO_OUTCOMES` 编程） |
| `packages/desktop/src-tauri/crates/infra/checks/src/mutation/segments_test.rs`（清单外补入·测试管线修复） | `slice_patch` 既有 11 处调用点补空串实参（透传形态逐字等值）+ 新增前缀剥离用例（修改 / 删除 / 新增三类路径行、`/dev/null` 侧保留、前缀不匹配不误剥） | 同上 |
| `packages/desktop/src-tauri/crates/infra/checks/src/mutation/mod_test.rs`（清单外补入·测试管线修复） | 假 `mutants` 执行体增 `FAKE_SPAWN_CARGO_OUTCOMES`（+ALT）报告文件落位编程；cargo canned fixture 换真实 outcomes.json 计数面形态；组合根链路段补丁断言改「切片 + suite 根前缀剥离」逐字等值；`SpawnWindow` PATH 前置增 msys 工具目录（`Git\usr\bin`——`cp` / `mkdir` 所在，探测失败跳过）：桌面应用门禁进程（GUI 拉起）PATH 无 usr/bin，脚本 `mkdir -p mutants.out` / `cp` 恒 command not found（stderr 随零突变体臂丢弃、不显形）→ 段报告文件静默缺位 → 全零计数 → score 恒 100 假绿，5 处断言败相（left (100.0, 80.0)）+ `TEST_ENV_LOCK` 中毒级联 36 处 | 同上 |
| `packages/desktop/src-tauri/crates/infra/checks/src/mutation/report_test.rs` / `mutation/stryker.rs` / `mutation/stryker_test.rs` / `static_check_test.rs` / `testexec/mod_test.rs` / `testexec/runner_test.rs`（清单外补入·rustfmt 机械随动，六文件） | rustfmt 风格重排（行折叠 / 展开 / import 排序，零 token / 语义变化——`cargo fmt --check` 幂等性可证：diff 仅空白与断行差异） | `server:check` 链的 `cargo fmt` 为 **fix 模式**（`cargo fmt && cargo clippy`，非 `--check` 门禁），随 mutation 管线修复触碰 checks crate 时对基线不合规处全量机械重排；**保留口径**：当前工作区 `cargo fmt --check` 已幂等全绿，回退六文件则下次 `server:check` 复现同 diff，故声明保留而非回退 |
| `packages/desktop/src/views/db/db-inspector-view.test.tsx`（清单外补入·门禁稳定） | 行选中态断言改按现行首行重查（`bodyRows()[0]` 进 `waitFor`），弃「点前捕获节点引用」：门禁高并行下列表重渲染换行节点时捕获引用指向游离节点，5s 等待预算内恒 null（test-execution 门实跑复现 1 例） | 断言用户可见结果而非节点身份——门禁反脆弱；断言语义（点行 → 选中 + 抽屉）不变 |
| `packages/desktop/pnpm-workspace.yaml`（清单外补入·环境级级联） | 增 `nodeLinker: hoisted`（附 7 行成因注释）：isolated 布局下 `vp check` 经 `import.meta.url` realpath 解析出的 `tsgolint.cmd` 落 `.pnpm` 深路径（本机长 worktree 路径下 252 字符），oxlint 经 `cmd.exe` 批处理中转超 ~250 字符阈值（CreateProcess MAX_PATH 边界）以「The system cannot find the path specified」失败；hoisted 扁平布局将候选收敛至 `node_modules/.bin`（149 字符）后 `server:check` / `bindings:export` / `client:check` 全链 exit 0 | 环境级门禁解锁（AC-10 前置）；布局声明非依赖面变化，`allowBuilds` / `minimumReleaseAgeExclude` 既有键零改动 |
| `packages/desktop/package.json` | `version` 0.5.9 → 0.5.10 | AC-11，归档时执行（见 DD12） |
| `packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs`（清单外补入·保绿随动） | `seed_cli_default_instance` → `seed_sdk_default_instance`（provider 空三件套随建——executor 轮在 `begin()` open 段以 `ConfigMissing` 显式收敛 failed，CLI 退役后原 CliMissing 合成收敛的归位口径，不 spawn 进程不落网络）；`isolate_path()` / `restore_path()` 辅助与各用例 PATH 隔离窗口（`TEST_PATH_LOCK` / `set_var("PATH", "")` / 恢复块）全数拆除（discover 预检退役后隔离无被测对象）；run 终态断言**文案**随动（「PATH 隔离下合成收敛 failed」→「open 段 ConfigMissing 收敛 failed」，终态值 `RunStatus::Failed` 不变——无断言翻转）；模块头 / doc 注释「PATH 隔离驱动收敛」口径随行；快照构造补 `agent_instance_id: None` 键 | 既有用例随被测面（CLI 租户删除 + store 拒 Cli 写面 + discover 预检退役）的必然机械随动，回退即红（AC-4 既有用例保绿半边）；归实现相位非测试相位（决策留痕 #6 界线） |
| `packages/desktop/src-tauri/src/commands/exec/mod_test.rs`（清单外补入·保绿随动） | 同上口径的保绿随动：①`seed_cli_default` fixture 删除，三用例迁 `seed_provider` + `seed_sdk_default`（store 写面拒 Cli 后原 fixture 恒 `Err` 落不了库）；②PATH 隔离窗口（`TEST_PATH_LOCK` import / `set_var("PATH", "")` 与恢复块）随 CLI discover 预检退役全数拆除——泵收敛驱动改非法 base_url（`http://bad host/v1`：open 段配置校验通过、请求编码段同步 Err 即时收敛，不落网络不触真实 CLI）；③stopregistry 停止链用例终态断言 `"failed"` → `"stopped"`（原「CLI 缺失合成收敛 failed」观测半边随退役消失，泵 select 停止臂先于编码段 Err 获胜——既有断言的保绿翻转，非新增断言面）；④跨引擎续接用例两边翻转（sdk agent 续存量 cli 快照会话——cli agent 无法再落库；被测语义「跨引擎拒绝先行验证 Continue 寻址正确性」不变）；⑤`seed_sdk_session` 快照构造补 `agent_instance_id: None` 键 | 同上（AC-4 / AC-5 既有用例保绿半边）；「不写断言」红线约束新增断言面，不排斥既有断言随被测面语义变化的保绿翻转 |

### 删除文件

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/src-tauri/crates/infra/agent/src/cli/` 整目录（9 文件 2251 行） | `mod.rs` / `discover.rs` / `flags.rs` / `jsonl.rs` / `runner.rs` + `discover_test.rs` / `flags_test.rs` / `jsonl_test.rs` / `runner_test.rs`——CLI 租户实现整目录退役（AC-1）；`ClaudeCliRunner` / `pub use cli::…` re-export 随 `lib.rs` 行删除；`kill_process_tree` 的 `taskkill /T /F` 形状已由 `sdk` 侧 `exec::kill_tree` 唯一持有，无迁出物 |

### 公共函数 / API

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `CLI_TENANT_RETIRED` | `…/crates/infra/store/src/model.rs` | 新增 | `pub const CLI_TENANT_RETIRED: &str` | 共享拒收文案（DD3）；四拒点逐字复用 |
| `EngineFacade::runner_for` | `…/agent/src/lib.rs` | 修改 | `pub fn runner_for(&self, kind: EngineKind, engine_cfg: EngineConfig) -> Result<Box<dyn AgentRunner>, String>` | 返回面收 `Result`（DD1）；`Cli` 臂 `Err(CLI_TENANT_RETIRED.to_owned())`、`Sdk` 臂 `Ok(Box::new(SdkRunner::new(engine_cfg, self.resume.clone(), self.context_window)))`；唯一 match 点结构不塌 |
| `compose_turn` | `…/agent/src/compose.rs` | 修改 | `pub fn compose_turn(stores: &WorkspaceStores, registry: Arc<StopRegistry>, store: Arc<Store>, agent: Option<i64>) -> Result<ComposedTurn, String>` | 签名不变；`runner_for` 的 `Err` 经 `?` 传导；`ResolvedEngine` / `ComposedTurn` 携 `agent_instance_id` 接线 |
| `ComposedTurn::begin` | `…/agent/src/compose.rs` | 修改 | `pub fn begin(&self, session: SessionRef, question: String, ctx: SessionCtx, provenance: SessionProvenance) -> Result<RunningTurn, String>` | 签名不变；`prior_handle` Option 透传（硬门槛退役，DD2）；快照填 `agent_instance_id: Some(...)`；跨引擎校验零改动 |
| `Store::upsert_agent_instance` | `…/crates/infra/store/src/store.rs` | 修改 | `pub fn upsert_agent_instance(&self, agent: AgentInstanceRecord) -> Result<AgentInstanceRecord, StoreError>` | 签名不变；拒 Cli 双口（入参口 + 存量行口，DD5） |
| `Store::set_default_agent_instance` | `…/crates/infra/store/src/store.rs` | 修改 | `pub fn set_default_agent_instance(&self, id: i64) -> Result<AgentInstanceRecord, StoreError>` | 签名不变；目标实例 `engine == Cli` → `Err`（默认标记仅可落 sdk 实例） |
| `save_agent_instance` | `…/src-tauri/src/commands/agents/mod.rs` | 修改 | `pub fn save_agent_instance(stores: State<'_, WorkspaceStores>, id: Option<i64>, name: String, engine: AgentEngineKind, provider_id: Option<i64>) -> Result<AgentInstanceRecord, String>` | 签名与参数类型不变（校验层拒绝，非类型层收窄）；拒 Cli 错误面透传 |
| `set_default_agent_instance` | `…/src-tauri/src/commands/agents/mod.rs` | 修改 | `pub fn set_default_agent_instance(stores: State<'_, WorkspaceStores>, id: i64) -> Result<AgentInstanceRecord, String>` | 签名不变；拒 Cli 错误面透传 |

<!-- 被删除的导出（`ClaudeCliRunner`、`pub use cli::{discover, flags, jsonl, runner}`）与 crate 内部方法（`ComposedTurn::is_cli_engine`）随「修改文件」lib.rs / compose.rs 行与「删除文件」表承载，不另立「删除」类型的 API 行（子表类型面维持新增/修改二值约定）。 -->

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `SessionConfigSnapshot` | `…/crates/infra/store/src/model.rs` | 修改 | 增 `agent_instance_id: Option<i64>`（`#[serde(default)]`，置 `engine` 之后）；`engine` 字段保留为历史事实（不删）；快照复制值的显式外键（多 agent 关联键，禁引申为跨库 join） |
| `SessionConfigSnapshotV1` | `…/crates/infra/store/src/model.rs` | 新增 | `pub(crate)` legacy 快照形态（`engine` / `model` / `permission_mode` 旧三字段），仅作 `SessionRecordV1` 升级链解码目标，不注册库模型组 |
| `SessionRecord` | `…/crates/infra/store/src/model.rs` | 修改 | `#[native_model(id = 7, version = 2, from = SessionRecordV1)]`；字段面零改动（`engine_session_id` 列留痕保留） |
| `SessionRecordV1` | `…/crates/infra/store/src/model.rs` | 新增 | `pub(crate)` legacy 结构（`#[native_model(id = 7, version = 1)]`，`config_snapshot: SessionConfigSnapshotV1`）+ `From` 双向（升级半边 `agent_instance_id = None`、降级半边缺省占位——只保升级语义真实性） |
| `AgentEngineKind` | `…/crates/infra/store/src/model.rs` | 修改 | 注释级：标「解码兼容，不再可选」；**变体集 `Cli, Sdk` 与 bincode 序号（`Cli`=0 / `Sdk`=1）MUST NOT 增删重排**（删/重排即两表报废含 sdk 数据） |
| `AgentInstanceRecord` | `…/crates/infra/store/src/model.rs` | 修改 | 注释级：「cli 可空」语义退役（`sdk` 唯一活档 provider 恒必选；存量 cli 行 provider 可空只读保留） |
| `EngineKind` | `…/agent/src/lib.rs` | 修改 | 注释级：双值保留（`Cli` 臂退役态·协议预留）；不出 bindings 零改动 |
| `EngineFacade` | `…/agent/src/lib.rs` | 修改 | `runner_for` 返回面收 `Result`（DD1）；`resume` / `context_window` 字段面与 builder 零改动 |
| `ResolvedEngine` | `…/agent/src/compose.rs` | 修改 | 增 `agent_instance_id: i64`（组合根内部结构） |
| `ComposedTurn` | `…/agent/src/compose.rs` | 修改 | 增 `agent_instance_id: i64` 私有字段（快照组装载荷）；`is_cli_engine()` 方法删除 |

### 配置

| 配置键 | 所在文件 | 类型 | 默认值 | 说明 |
|--------|----------|------|--------|------|
| `version` | `packages/desktop/package.json` | 修改 | `"0.5.10"` | AC-11，归档时执行 0.5.9 → 0.5.10（DD12） |

---

## 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `SessionConfigSnapshot`（嵌套 struct） | `engine: AgentEngineKind`（历史事实，保留）、`agent_instance_id: Option<i64>`（新增：装配时点实例 id 的**快照复制值**，`None` = 存量行缺列）、`model: Option<String>`、`permission_mode: AgentPermissionMode` | 「会话 ↔ 实例」显式关联键（多 agent 过滤/聚合锚）；复制值非跨库引用——`AgentInstanceRecord` 在全局库、session 在 workspace 库，实例改名/删除不伤历史会话；MUST NOT 引申为跨库 join 或存在性约束 | 嵌 `SessionRecord`（bincode）；对外经 `serde_json::Value` 不透明承载（core 不解释），JSON 线键 `agentInstanceId`（camelCase） |
| `SessionRecord`（v2） | `id`(PK, String)、`engine_session_id: Option<String>`（留痕保留）、`config_snapshot`、`source` / `source_ref`、`created_at` / `updated_at` | 挂 `SessionEventRecord` 转录单表与 `AgentRunRecord` 轮统计行 | workspace 库，`native_model(id = 7, version = 2, from = SessionRecordV1)`，bincode 1.3；旧行经版本链自动升级（`agent_instance_id` 读出 `None`），零迁移代码路径 |
| `SessionRecordV1` / `SessionConfigSnapshotV1`（legacy） | v1 原形（旧快照三字段） | 仅作 envelope 版本链升级/降级解码目标 | 不落新库；旧 payload 解码中转（`AgentProviderRecordV1→V2` 先例同型） |
| `AgentEngineKind`（枚举） | `Cli`(=0) / `Sdk`(=1) | 落 `AgentInstanceRecord.engine` 与 `SessionConfigSnapshot.engine` 两处 | bincode 变体序号钉死（解码锚）；线值 `("cli" \| "sdk")` 不演进 |
| `AgentInstanceRecord` | `id`(PK)、`name`(唯一)、`engine`、`provider_id: Option<i64>`、`is_default` | 被快照 `agent_instance_id` 复制值引用（非强约束）；引用 `AgentProviderRecord` | 全局库 `native_model(id = 6, version = 1)` 零改动；存量 Cli 行可读可删不可改 |

---

## 路由 / API 设计

<!-- 不涉及：无 HTTP 端点；Tauri IPC 命令签名零变化（save / set_default 拒 Cli 在 store 校验层，参数类型不收窄），`AgentEngineKind` 线值不演进，故无路由/端点设计面。 -->

---

## 依赖

### 运行时依赖

- 无新增、无删除 —— `regex` / `glob` 保留（现仅 `sdk/tools.rs` 消费，R9）；`tokio` / `serde_json` 为 sdk 引擎与门面共用，不随 cli 删除；`native_db` / `native_model`（bincode 1.3 默认 codec）承接 v1→v2 版本链零新增

### 构建/测试依赖

- 无新增 —— export-bindings（`pnpm bindings:export`）为既有生成通道；测试依赖面归测试相位

---

## 决策留痕（边界张力与红线）

1. **「不要修改」 vs R5 文案指引**（DD10）：跨引擎续会话校验的逻辑与 Err 文案均不动；存量 cli 会话续接经该门显式 `Err`（不落库不推流、列表/转录照常可读），已满足 AC-4；「CLI 租户已下线」文案由 DD3 四拒点承载。若后续认为会话侧错误文案仍需指明退役事实，属独立文案变更，不得借本变更顺手改校验。
2. **双枚举不合一、`EngineKind` 不塌单值**：store 禁拖引擎门面 crate + `AgentEngineKind` 需解码锚（proposal 决策表）；`Cli` 臂 `Err` ≠ 枚举塌单值，多租户（本机 CLI / 进程内 SDK / 远程 API）协议预留保留。
3. **数据面零改动**：不删变体、不重排、零清理脚本；唯一存量动作 = 缺省发起对默认 cli 实例显式 `Err`（不清 `is_default`、零数据写入；`set_default` 切换语义原子清旧默认，一次操作修复）。未来清理 cli 残留数据走独立 change（`native_model` version bump + `From` 映射设计），本变更不预建。
4. **protect-files 护栏缺口**：插件 hook 路径随 CLI 退役失去载体，缺口在 `desktop-change-orchestration` 边界 #5 显式留痕（spec delta 已含）；桌面原生护栏归独立 change，不由本变更隐式吸收。
5. **change 执行引擎边界翻案**：change run 发起自此依赖 SDK 连接配置（默认 sdk agent + provider），缺失时按既有显式 `Err` 面失败（R3 知情接受）；契约面翻案文本已内含于五份 spec delta（提案层产物）。
6. **测试面归属与保绿随动界线**：proposal「测试文件」清单（cli 测试随目录删除除外）的**新增覆盖**——拒 Cli 透传用例群（`agents/mod_test.rs`）、v1→v2 升级回环、变体序号锚定测试、configSnapshot mock 补键、cli 档用例群翻转——归 test-design → test-gen → test-execution 相位。实现相位承接两类机械随动：**编译面机械随动**（保编译、不写断言，沿 2026-10-10-unify-run-state-persistence 先例）与**保绿随动**（既有用例随被测面退役的必然行为随动，回退即红——测试套件 1765 绿测在当前代码面为既定事实）。保绿随动的判定标准：被测面（CLI 租户删除 / store 拒 Cli 写面 / discover 预检退役）已变，既有用例不随动则必红——①fixture 迁移（`seed_cli_default` 系随写面拒 Cli 恒 `Err`）；②PATH 隔离拆除（隔离的被测对象 discover 预检已退役）；③终态断言 `"failed"` → `"stopped"`（CliMissing 合成收敛语义消失，停止臂获胜口径）；④跨引擎续接用例两边翻转（cli agent 无法再落库，被测语义不变）。落点为 `exec/mod_test.rs` / `change_flow/mod_test.rs` 两条清单行（清单外补入·保绿随动）。「不写断言」红线约束的是**新增断言面**，不排斥既有断言的保绿翻转；新增覆盖（如 `agents/mod_test.rs` 拒 Cli 用例群）仍不入实现清单。

---

## 验收标准对齐

| AC | 设计落点 |
|----|----------|
| AC-1 | 「删除文件」表（`cli/` 9 文件）+ 修改文件 lib.rs / worker.rs 行；`regex` / `glob` 核查行（Cargo.toml 两行，DD11）；`cargo test --workspace` 全绿归测试相位 |
| AC-2 | DD1 / DD9 + 硬边界（`crates/core/agent` 零 diff 守线断言；`AgentStartError` 变体集不动；`AgentEngineKind` 变体集不动） |
| AC-3 | 数据模型表（解码锚注释）；bincode 序号锚定断言测试归测试相位（`model_test.rs`） |
| AC-4 | `resolve_agent_engine` Cli 臂 `Err`（缺省与显式同臂）+ DD3 文案；存量 cli 会话续接经跨引擎门显式失败（DD10）；不落库不推流（`begin()` 前置 `Err`）；既有用例保绿半边（exec / change_flow 跨引擎续接随动，决策留痕 #6） |
| AC-5 | DD5 双口拒 + `set_default` 拒 + DD4 前端入口禁用；存量默认 cli 实例缺省发起 `Err` 引导改默认（不清 `is_default`，零写入） |
| AC-6 | DD2 硬门槛退役（`prior_handle` Option 透传）；`SessionRecord.engine_session_id` / `SessionRow.remoteSessionId` / `bind_session_remote` 留痕保留（数据模型表 + store.rs 行） |
| AC-7 | DD6 / DD7 / DD8（新会话恒 `Some`、存量行 v1→v2 读出 `None`）；前端 mock 补 `agentInstanceId` 键归测试相位 |
| AC-8 | agent-panel.tsx 行（下拉单值 / `canSave` 无 cli 分支 / provider 恒启用必选 / 存量 cli 行只读可删） |
| AC-9 | 五份 spec delta 已落盘（提案层产物，本节信息性首条）；「引擎恒 CLI」与 SpecSync「CLI 引擎」措辞翻案、「Claude CLI 租户（MVP）」整段 REMOVED 均内含 delta 文本 |
| AC-10 | 守线阶段（server:check / client:check / bindings:check / 零 diff 断言 / `openspec/config.json` 无新增 excludes）；既有用例保绿随动（exec / change_flow `mod_test.rs` 行）与 rustfmt 机械随动（checks 六文件行）在修改文件表声明；覆盖率阈值与 mutation score 归 test-execution 相位 |
| AC-11 | 配置表 `version` 行 + DD12（归档时执行）；`plugins/dev-team` 2.10.44 零改动 |

---

## 待决问题

无 —— proposal 移交的四项待决已在「设计定值」DD1–DD4 全部收口。两条已知出口（非本变更范围、不预建）留痕于此：

- 未来清理 cli 残留数据（`AgentEngineKind` 变体退役/改写）：独立 change 走 `native_model` version bump + `From` 映射设计（6 处先例）。
- 桌面原生写保护护栏（内核 permission 层方向）：独立 change，为 protect-files 缺口的既定出口。
