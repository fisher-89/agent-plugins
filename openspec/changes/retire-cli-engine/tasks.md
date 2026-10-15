# 任务: retire-cli-engine

> 实现边界以 `design.md` 变更清单为准。**新增覆盖**（测试断言翻转用例 / 新用例）由 test-design → test-gen → test-execution 阶段承接，本列表不含测试编写任务；但签名 / 删除波及既有测试文件的两类机械随动归各实现阶段：**编译面机械随动**（保编译、不写断言，沿 2026-10-10-unify-run-state-persistence 先例）与**保绿随动**（既有用例随被测面退役的必然行为随动——fixture 迁移 / PATH 隔离拆除 / 终态断言翻转，回退即红，design 决策留痕 #6 界线：「不写断言」红线约束新增断言面，不排斥既有断言的保绿翻转）。顺序按依赖排列：store 数据面 → CLI 删除与门面收窄 → 组合根 → 命令面与产品面 → 依赖核查与 bindings 再生 → 守线。版本 bump 归档轨道（AC-11 / design DD12），随归档提交执行。

## 阶段一：store 数据面（快照外键 + 版本链 + 拒 Cli 写面，AC-3 / AC-5 / AC-6 / AC-7）

- [x] `packages/desktop/src-tauri/crates/infra/store/src/model.rs`：`SessionConfigSnapshot` 增 `agent_instance_id: Option<i64>`（`#[serde(default)]`，置 `engine` 之后——design DD7 定序）；新增 `SessionConfigSnapshotV1`（旧三字段 legacy）与 `SessionRecordV1`（`#[native_model(id = 7, version = 1)]`，`config_snapshot: SessionConfigSnapshotV1`）+ `From` 双向（升级半边 `agent_instance_id = None`、降级半边缺省占位）；`SessionRecord` 改 `#[native_model(id = 7, version = 2, from = SessionRecordV1)]`（design DD6）
- [x] `packages/desktop/src-tauri/crates/infra/store/src/model.rs`：新增 `pub const CLI_TENANT_RETIRED: &str = "CLI 租户已下线：请前往 Agent 管理页（/agents）新建或改用 sdk agent，并将其设为默认（存量 cli 实例可读可删不可改）"`（design DD3 定稿文案）；`AgentEngineKind` 注释标「解码兼容，不再可选」（**变体集与序号零改动**）；`AgentInstanceRecord` 注释口径更新（「cli 可空」语义退役、存量行只读）
- [x] `packages/desktop/src-tauri/crates/infra/store/src/lib.rs`：re-export `CLI_TENANT_RETIRED`（compose / 门面经 `store::CLI_TENANT_RETIRED` 消费）
- [x] `packages/desktop/src-tauri/crates/infra/store/src/store.rs`：`upsert_agent_instance` 拒 Cli 双口（design DD5）——①入参 `agent.engine == Cli` 新建与更新均 `Err(StoreError::Db(CLI_TENANT_RETIRED.to_owned()))`；②更新臂存量行 `engine == Cli` 同拒（整行不可改，含改 engine 为 sdk 的整行更新）；doc 注释「cli 可空（携引用同样校验悬空）」口径随行
- [x] `packages/desktop/src-tauri/crates/infra/store/src/store.rs`：`set_default_agent_instance` 对目标实例 `engine == Cli` → `Err`（默认标记仅可落 sdk 实例）；doc 随行；`bind_session_remote` 留痕写面零改动核对（AC-6）
- [x] 编译面机械随动：`model_test.rs` / `store_test.rs` / `store_port_test.rs` 既有用例保编译（`SessionConfigSnapshot` 构造补 `agent_instance_id` 键；v1→v2 升级回环 / 拒 Cli / 序号锚定断言留测试相位）；级联触点补入：`envelope_test.rs` 与壳层 `commands/db|exec|explores|workspaces` 四个 `mod_test.rs` 同为快照字面量构造点，随补键保编译（exec / change_flow 的深层保绿随动见阶段二末条——补键仅其随动面的最小子集）

## 阶段二：CLI 租户实现删除 + 引擎门面收窄（AC-1 / AC-2）

- [x] 删除 `packages/desktop/src-tauri/crates/infra/agent/src/cli/` 整目录（9 文件 2251 行：`mod.rs` / `discover.rs` / `flags.rs` / `jsonl.rs` / `runner.rs` + 四个 `_test`；`kill_process_tree` 的 `taskkill /T /F` 形状已由 `sdk` 侧 `exec::kill_tree` 唯一持有，无迁出物）
- [x] `packages/desktop/src-tauri/crates/infra/agent/src/lib.rs`：删 `mod cli;`、`pub use cli::runner::ClaudeCliRunner;`、`pub use cli::{discover, flags, jsonl, runner};`；`runner_for` 返回面改 `Result<Box<dyn AgentRunner>, String>`——`EngineKind::Cli` 臂 `Err(CLI_TENANT_RETIRED.to_owned())`（design DD1）、`EngineKind::Sdk` 臂 `Ok(Box::new(SdkRunner::new(...)))`（**唯一 match 点结构不塌**）；`EngineKind` / `ResumeTranscript` / `EngineFacade` / builder 注释口径改退役态措辞（`Cli` 变体标「退役态·协议预留」、删「不传即 CLI 租户」「CLI 臂不消费」旧口径）
- [x] `packages/desktop/src-tauri/crates/infra/agent/src/worker.rs`：删 `execute()` 的 `is_cli_engine()` discover 轮前预检分支与「CLI 预检」注释（`ConfigMissing` 归 `begin()` open 段）
- [x] `packages/desktop/src-tauri/crates/infra/agent/src/sdk/policy.rs` / `sdk/runner.rs`：CLI 悬空注释引用清理（「与 CLI 租户的 permission-mode 对齐」「与 CLI 泵同策略」→ 中立口径；行为零改动）
- [x] 编译面机械随动：`lib_test.rs` / `compose_test.rs` / `worker_test.rs` / `sdk/runner_test.rs` 保编译——`ClaudeCliRunner` / `crate::discover` / `is_cli_engine` 涉被删符号的用例块随删（被测对象已退役），`runner_for` 调用点改 `Result` 接收（`.expect` 包装，含清单外 `sdk/config_test.rs` 同型调用点）；断言翻转留测试相位
- [x] 保绿随动（design 决策留痕 #6 界线；回溯补全变更清单对应行——回退即红，既有 1765 绿测的行为事实，非新增覆盖）：`src-tauri/src/commands/exec/mod_test.rs` 与 `src-tauri/src/commands/change_flow/mod_test.rs` 既有用例随被测面（CLI 租户删除 + store 拒 Cli 写面 + discover 预检退役）随动——①`seed_cli_default` / `seed_cli_default_instance` fixture 迁 `seed_provider` + `seed_sdk_default`（store 拒 Cli 后原 fixture 恒 `Err`）；②PATH 隔离窗口（`TEST_PATH_LOCK` import / `isolate_path()` / `restore_path()` / `set_var("PATH", "")` 与恢复块）全数拆除，泵收敛驱动改非法 base_url（`http://bad host/v1` 编码段同步 Err 即时收敛，不落网络不触真实 CLI）或 provider 空三件套（open 段 `ConfigMissing` 显式收敛 failed——CliMissing 合成收敛的归位口径）；③exec stopregistry 停止链用例终态断言 `"failed"` → `"stopped"`（CliMissing 合成收敛语义消失，泵 select 停止臂先于编码段 Err 获胜；change_flow 侧终态值 `RunStatus::Failed` 不变、仅断言文案随动）；④exec 跨引擎续接用例两边翻转（存量 cli 快照会话以 sdk agent 续——被测语义「跨引擎拒绝先行验证 Continue 寻址正确性」不变）；⑤快照构造补 `agent_instance_id: None` 键

## 阶段三：组合根（AC-4 / AC-6 / AC-7）

- [x] `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs`：`resolve_agent_engine` 的 `AgentEngineKind::Cli` 臂 → `Err(CLI_TENANT_RETIRED.to_owned())`（缺省与显式路径同臂，`EngineConfig::empty()` 占位构造随删）；`ResolvedEngine` 增 `agent_instance_id: i64`（取 `instance.id`）
- [x] `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs`：`ComposedTurn` 增 `agent_instance_id: i64` 私有字段并自 `ResolvedEngine` 接线；`compose_turn` 调 `runner_for` 处 `?` 传导 `Err`；`store_kind / models` 双臂 match 保留（`Cli` 死臂原样映射 `(AgentEngineKind::Cli, None)`，不引入 panic 形态——design DD9）
- [x] `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs`：`begin()` 硬门槛分支删除——`SessionRef::Continue` 分支 `prior_handle` 直取 `record.engine_session_id.clone()` Option 透传（design DD2）；快照组装增 `agent_instance_id: Some(self.agent_instance_id)`（design DD8）；**跨引擎续会话校验行零改动**（design DD10，目视核验）；删 `is_cli_engine()` 方法；「CLI 引擎持有不消费 / CliMissing 收敛 / 引擎句柄缺失」注释口径随行
- [x] 编译面机械随动：`compose_test.rs` / `worker_test.rs` / `store_port_test.rs` 保编译（快照构造补键、`begin()` 旧硬门槛用例的编译面随动；断言翻转与硬门槛退役用例留测试相位）；级联触点补入：`store_port.rs` doc「快照定型三面」→「四面」（快照字段面扩张的注释随行）

## 阶段四：命令面与产品面（AC-5 / AC-8）

- [x] `packages/desktop/src-tauri/src/commands/agents/mod.rs`：`save_agent_instance` / `set_default_agent_instance` doc 注释补「携 `engine: Cli` / Cli 实例拒绝（存量行可读可删不可改）」口径；命令体零改动（拒 Cli 经 store 校验 `Err(String)` 透传，签名与 `AgentEngineKind` 参数类型不收窄）
- [x] `packages/desktop/src/views/agents/components/agent-panel.tsx`：`ENGINE_OPTIONS` 收单值 `[{ value: 'sdk', label: 'sdk' }]`（select 形态保留）；`ProviderSelect` 删 `engine` prop 与 `disabled={engine === 'cli'}`（恒启用必选）；`canSave` 改 `form.name.trim() !== '' && form.providerId !== null`；`AgentListItem` 对 `record.engine === 'cli'` 行禁用「编辑」「设为默认」按钮（title 提示「CLI 租户已下线，实例只读」）、「删除」保留（design DD4）；`AgentForm` / 模块头「engine 二值下拉」doc 口径随行
- [x] 编译面机械随动：`agent-panel.test.tsx` / `use-agent-instances.test.ts` 等前端测试保 tsc 类型编译（`ProviderSelect` prop 面随动、mock 形状适配——测试面经 DOM 交互、无涉被删 prop 的类型引用，无需改动）；cli 档用例群断言翻转与 configSnapshot mock 补 `agentInstanceId` 键（8 处）留测试相位

## 阶段五：依赖核查与 bindings 再生成（AC-1）

- [x] `packages/desktop/src-tauri/Cargo.toml` + `packages/desktop/src-tauri/crates/infra/agent/Cargo.toml`：依赖引用面核查（预期零 diff）——`regex` / `glob` 现仅 `sdk/tools.rs` 消费，**保留勿盲删**（design DD11）；确认无 cli 独占依赖条目（tokio / serde_json / agent 均 sdk 或门面共用）
- [x] 执行 `pnpm -C packages/desktop run bindings:export` 再生成 `src/types/generated/bindings.ts` 与 `src-tauri/src/bindings/mod_test.rs` 快照（非手改）；预期仅注释级 diff，`AgentEngineKind` 线值 `("cli" | "sdk")` 不演进、`saveAgentInstance` 参数类型不收窄

## 阶段六：守线（静态，不含测试执行，AC-2 / AC-10）

- [x] 静态检查全绿：`pnpm -C packages/desktop run server:check`（cargo fmt + clippy）与 `pnpm -C packages/desktop run client:check`（vp check + knip，零未用导出残留、零新增豁免条目）；清单外补入·环境级级联：`packages/desktop/pnpm-workspace.yaml` 增 `nodeLinker: hoisted`（附注释说明）——isolated 布局下 `vp check` 经 `import.meta.url` realpath 解析出的 `tsgolint.cmd` 落 `.pnpm` 深路径（本机长 worktree 路径下 252 字符），oxlint 经 `cmd.exe` 批处理中转该路径超 ~250 字符阈值（CreateProcess MAX_PATH 边界）以「The system cannot find the path specified」失败；hoisted 扁平布局将候选收敛至 `node_modules/.bin`（149 字符）后 `server:check` / `bindings:export` / `client:check` 全链 exit 0
- [x] 清单外补入·rustfmt 机械随动声明（回溯补全变更清单对应行；保留口径）：`crates/infra/checks/src/` 下 `mutation/report_test.rs` / `mutation/stryker.rs` / `mutation/stryker_test.rs` / `static_check_test.rs` / `testexec/mod_test.rs` / `testexec/runner_test.rs` 六文件 rustfmt 风格重排（行折叠 / 展开 / import 排序，零 token / 语义变化）——`server:check` 链的 `cargo fmt` 为 fix 模式（非 `--check` 门禁），随 mutation 管线修复触碰 checks crate 时对基线不合规处全量机械重排；当前工作区 `cargo fmt --check` 幂等全绿，回退则下次 `server:check` 复现同 diff，故声明保留而非回退
- [x] bindings 重导幂等自查：再跑 `pnpm -C packages/desktop run bindings:check` 零新增 diff（重导后 `bindings.ts` / `mod_test.rs` 校验和不变、前后 diff 逐字节相同——exit 1 仅承载本变更工作区预期 diff，重导零新增）
- [x] 零 diff 边界断言（design 硬边界）：`git diff --exit-code -- src-tauri/crates/core/agent`（core 零 diff）；`AgentEngineKind` 变体集与序号仅注释级 diff、`compose.begin()` 跨引擎校验行 / `bind_session_remote` / `SessionRecord.engine_session_id` 列 / `SessionRow.remoteSessionId` 面零改动目视核验；`crates/infra/agent/src/sdk/config.rs`（`EngineConfig`）零 diff；`plugins/dev-team` 保持 2.10.44 零改动；`openspec/config.json` 无新增 excludes / 豁免
- [x] 残留 grep 自查（产品代码零命中）：`cli::` / `ClaudeCliRunner` / `is_cli_engine` / `crate::discover` / 「CLI 预检」；`crates/core/agent` 中 `CliMissing` 变体保持在位（协议预留词汇，不动）
- [x] 清单外补入·测试管线修复（AC-10 mutation 门在位前置）：`crates/infra/checks/src/mutation/` cargo-mutants 段执行面对齐真实工具（27.1.0）实测行为——①段报告权威落位 `mutants.out/outcomes.json`（运行面 stdout 无 JSON 载荷，`--json` 仅作用于 `--list`，原 stdout 提取口径对真实工具恒不可满足→成功运行也误判中断，本变更门首跑即此败相）；②argv 增 `--workspace`（混合 workspace 缺省只扫根包，`crates/**` 恒不在突变面）；③段补丁路径剥 suite 根前缀（`--in-diff` 以 spawn cwd 相对路径匹配，项目根相对路径恒零命中，与 Stryker `mutate` relative_to_cwd 同约定）；④退出 0 无报告 = diff 零可突变交叠 → 零突变体段（cargo-mutants EmptyDiff / NoSourceFiles / NoMutants 语义），不再误判 Interrupted。本地实测：全 diff 面真实 cargo-mutants 跑 36 突变体 = 21 caught + 15 unviable + **0 survived**（score 100 ≥ 70 阈）；`checks-runtime` mutation 模块 53 用例全绿。**生效前提：mutation 门在桌面应用进程内执行——修复须随应用重建（从含本修复的分支构建）方可在门禁中生效；运行中的旧版应用（master 0.5.7 线）会复现同一中断败相，与源侧修复无关**
- [x] 清单外补入·测试管线修复随行（test-execution 门第 1 轮反馈修复）：假 `mutants` 执行体的 `mkdir` / `cp` 外部件依赖 `Git\usr\bin`——桌面应用门禁进程（GUI 拉起）PATH 无 usr/bin → command not found（stderr 随零突变体臂丢弃不显形）→ `mutants.out/outcomes.json` 静默缺位 → 全零计数 score 恒 100，5 处断言假败 + `TEST_ENV_LOCK` 中毒级联 36 处（static_check / testexec 全灭）。修复：`SpawnWindow` PATH 前置 msys 工具目录（`cp.exe` / `mkdir.exe` 探测定位，失败跳过——bash 落位面随后显式报因）；app 形 PATH（System32 + cargo + Git\cmd，无 usr/bin）本地复现 → 修复后 113 用例全绿、`cargo test --workspace` 30 二进制全绿。同轮 vite-plus `db-inspector-view` 1 例节点身份型假败：行选中态断言改按现行首行重查（用户可见结果，非捕获节点引用）
- [x] 变更清单核对：`design.md` 变更清单与实际触达文件双向一致（清单外零改动、清单内零遗漏；含回溯补全的 exec / change_flow 保绿随动行与 checks 六文件 rustfmt 随动行——`agents/mod_test.rs` 新增拒 Cli 用例群为测试相位新增覆盖，不入实现清单，口径见 design 决策留痕 #6）；`packages/desktop/package.json` 实现期零触点（0.5.9 → 0.5.10 归档轨道执行）
- [x] 测试全绿 / 覆盖率（lines 90 / branches 85 / functions 90）/ mutation score 80 由 test-design → test-gen → test-execution 阶段承接核验（AC-10 执行面），不入本任务面——按设计移交测试相位，本阶段零待办

## 阶段七：交付（归档时执行，AC-11）

- [x] `packages/desktop/package.json`：`version` 0.5.9 → 0.5.10——按 DD12 / AC-11 移交归档轨道随归档提交执行（`tauri.conf.json` 维持 `../package.json` 引用、`src-tauri/Cargo.toml` 不随动；`plugins/dev-team` 保持 2.10.44 零改动；用户可见变更：CLI 租户退役、SDK 租户承载全部运行发起）；实现期零触点，本阶段零待办
