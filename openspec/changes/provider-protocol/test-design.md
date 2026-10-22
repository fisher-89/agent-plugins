# 测试设计: provider-protocol

> **变更**: provider-protocol
> **角色**: test-design-planner（executor）
> **输入**: `proposal.md`（AC-1 – AC-12）+ `design.md`（变更清单 / DD1 – DD20 / 公共函数 API 表 / 决策留痕）
> **范围**: 仅设计单元测试范围与公共 API 签名，不写测试命令细节。静态脚本链（`pnpm -C packages/desktop run check` 含 `bindings:check` / `knip` / `cargo clippy`、`run test` 汇总）由桌面端执行，agent 内不重复、不复述。

## 测试框架与测试区域识别

- **后端 `packages/desktop/src-tauri/`**：Rust `cargo test --workspace`；测试文件与源文件**同目录、同前缀 + `_test.rs`**，经 `#[cfg(test)] mod xxx_test;` 挂载（`sdk/mod.rs` 侧挂）或 `#[cfg(test)] #[path = "compose_test.rs"] mod compose_test;` 就地挂载（`compose.rs` 侧挂——子模块可触达父模块私有项，`resolve_agent_engine` 用例即依赖此形态）。断言库为内置 `assert!` / `assert_eq!` / `matches!`。既有装置沿用：
  - `crates/infra/store/src/*_test.rs`：`native_model::encode` / `decode` 内存往返（版本链解码锚）+ `tempfile` 数据根真库（`Store` / `WorkspaceStores`）；
  - `crates/infra/agent/src/sdk/runner_test.rs`：真实 `SdkRunner` + `base_url` 指本机 discard 端口（`http://127.0.0.1:9/v1`，恒拒连、不打网络），事件面经 `AgentSession.observations` 断言；
  - `src/commands/agents/mod_test.rs`：`tauri::test::mock_app()` + `app.manage(WorkspaceStores::open(tempdir))`，命令直调；
  - 异步：`#[tokio::test]`（引擎泵任务）/ `#[test]`（纯函数与解析面）。
  - **环境敏感装置的既有缺陷（test-execution 首轮暴露，已在本变更内修复，见 proposal「测试装置修复」/ tasks 阶段九）**：`crates/infra/checks/src/static_check_test.rs` 的 shim 用例须以 `PathPrefix::enter(ws.path())` 把临时 root 前置到 PATH 再登记**裸名**命令——仅落 `.cmd` 文件依赖 `cmd` 的 cwd 搜索，在 `NoDefaultCurrentDirectoryInExePath=1` 的宿主上 spawn 期才失败；且该 crate 的用例共用 `TEST_ENV_LOCK`，任一持锁 panic 会毒化 Mutex、把同 crate 其余用例连坐成 `PoisonError` 伪失败（首轮 57 条失败中 56 条属此类）。等价性一类用例另需「真执行」断言（`diagnostics` 含 shim 输出），否则「双不可达」下也会等值通过。
- **前端 `packages/desktop/src/`**：Vitest（`vite-plus/test` 提供 `describe` / `it` / `expect` / `vi`）；hook 用 `renderHook` + `act` / `waitFor`，组件用 `render` / `screen` / `fireEvent`（`@testing-library/react`）+ jsdom。测试文件与源文件同目录同前缀：`.ts` → `.test.ts`（hook / 纯函数）、`.tsx` → `.test.tsx`（组件）。
- **映射约定**：`xxx.rs` → `xxx_test.rs`；`src/commands/<域>/mod.rs` → `src/commands/<域>/mod_test.rs`；`xxx.ts` → `xxx.test.ts`；`xxx.tsx` → `xxx.test.tsx`（均与源文件同目录）。
- **组装面断言接缝（设计约束，实施归 test-gen / 实现相位）**：AC-5 / AC-8 / AC-9 需在**不打网络**前提下断言 `DynModel` 描述面（`name()` / `id()`）、`base_url` 归一化、输出上限取值链与 beta 条件。当前 `build_model` 为 `sdk/runner.rs` 私有函数，而 `runner_test` 是 `crate::sdk` 的**兄弟**模块（`sdk/mod.rs` 侧挂），私有项不可达。测试相位按最小面补齐可测性：`build_model` 提升为 `pub(crate)`（与 `sdk/context.rs` 既有 `pub(crate)` 内部面同型，**不出口 crate、不进 bindings**），或把臂内取值链拆为 `pub(crate)` 纯函数供 `runner_test` 直接断言。此点在 `sdk/runner.rs` 章节「Mock策略」中显式留痕，不新增公共 API。

---

## 验收范围

| AC ID | 验收条件 | 被测文件或模块 |
|-------|---------|----------------|
| AC-1 | store V2→V3 一次演进：新记录带 `protocol` / `max_tokens`；v2 存量行升级后 `protocol = openai`、`max_tokens = None`（行为零变化）；v1 存量行经 V1→V2→V3 两级链仍可读；版本头 3；线值 `"openai"` / `"anthropic"` | `packages/desktop/src-tauri/crates/infra/store/src/model.rs` |
| AC-2 | `AgentProviderProtocol` 变体序号 `Openai = 0` / `Anthropic = 1` 且无重排；`AgentProviderRecordV2` 仅作升级链中转、不注册进库模型组、不出公共查询面 | `packages/desktop/src-tauri/crates/infra/store/src/model.rs`、`packages/desktop/src-tauri/crates/infra/store/src/lib.rs`（导出面，见「不可测试项」`store/src/lib.rs` 行） |
| AC-3 | 遮蔽 `Debug` 输出含 `protocol` / `max_tokens` 位、`api_key` 位为 `sk-***abc` 形态且无明文；`PartialEq` 比较行为不受影响 | `packages/desktop/src-tauri/crates/infra/store/src/model.rs` |
| AC-4 | `EngineConfig` 携 `protocol`（门面 `Protocol`），`is_complete` 语义不变；`runner_for` 唯一 match 点与 `EngineKind` 双值不塌 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/config.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/lib.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/compose.rs` |
| AC-5 | 双臂组装：openai 臂与现状逐字等价（`name() == "openai"`、`id()` = 模型名）；anthropic 臂 `name() == "anthropic"`、`id()` = 裸名；两臂同型 `DynModel<Completion>`；anthropic `base_url` 带 `/v1` 不产生重复版本段 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs` |
| AC-6 | `parse_model_spec` 覆盖规则表全形态：`[1m]` 剥离且 `length = 1_000_000`、大小写不敏感、两端空白裁剪；`[2m]` / `[8k]` / `[]` / 非末尾方括号 → `Err`；无后缀 → `length = None` 且 `name` 为原文（trim 后） | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/model_spec.rs` |
| AC-7 | 窗长优先级可考：模型名后缀 > `context_length` > 128K 缺省，且**按档**独立解析；`EngineFacade::with_context_window` 签名不变 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/context.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/lib.rs` |
| AC-8 | anthropic 输出上限取值链：`provider.max_tokens` 优先；缺席时 rig 名单内模型保留官方默认值；名单外补 `32_000`；openai 协议 MUST NOT 写 `max_tokens` | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs` |
| AC-9 | anthropic beta 条件发：`protocol = anthropic` 且模型名带 `[1m]` → `anthropic-beta` 含 `context-1m-2025-08-07`；openai 或后缀缺席 → 不发 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs` |
| AC-10 | 保存期校验单点：非法后缀与 `max_tokens < 4096` 均 `Err` 且错误串不含 api_key；合法形态（含 `[1m]`、`max_tokens = 4096`）保存成功；复用引擎侧同一 `parse_model_spec` | `packages/desktop/src-tauri/src/commands/agents/mod.rs` |
| AC-11 | 管理页：新建缺省协议 `openai`；切 anthropic 后 `base_url` placeholder / 提示与协议徽标同步；`maxTokens` 留空 = 未配置、`< 4096` 拒绝提交；保存入参携 `protocol` / `maxTokens`；列表行协议徽标可见 | `packages/desktop/src/views/agents/components/provider-panel.tsx`、`packages/desktop/src/views/agents/hooks/use-agent-providers.ts` |
| AC-12 | `bindings:check` 零 diff（绑定再生成、非手改）；`run check` 与 `run test` 全绿 | `packages/desktop/src/types/generated/bindings.ts`、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/mod.rs`（见「不可测试项」`bindings.ts` / `sdk/mod.rs` / 静态检查链 三行） |

---

## 单元测试

> 说明：`路径类型` 取值——`正向` / `异常` / `边界` / `组合`（跨模块链路用例，按**链路发起方 / 最上层调用方**挂靠到入口模块章节，不另立集成层）；`迭代类型` 取值——`新增`（本轮新增用例）、`修改`（既有用例随行为 / 字段面改写）、`保留`（既有用例不变，仅作回归承载）。每份 `#### 用例` 表对每个 `#### 待测功能` 条目至少覆盖正向 / 异常 / 边界三类。

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/model_spec.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/model_spec_test.rs

#### 待测功能

- `parse_model_spec(raw: &str) -> Result<ModelSpec, ModelSpecError>`: 模型名后缀解析**唯一**规则源（保存期命令层与启动期引擎共用）。末尾 `[1m]`（ASCII 大小写不敏感）剥离出裸名并给 `length = Some(1_000_000)`；其余方括号形态（`[2m]` / `[8k]` / `[]` / 非末尾 / 内空白）一律 `Err`；无后缀 → `name` = trim 后原文、`length = None`；裸名两侧再次 trim
- `ModelSpec::length` 消费面（`ModelSpec { name: String, length: Option<u64> }`）: `length == Some(1_000_000)` 即「后缀在场」的唯一可考表示，同时驱动窗长（DD11）与 beta 判定（DD7）；`ModelSpecError { raw, reason }` + `Display`（`模型名后缀非法: {raw}（{reason}）`）只承载模型名原文，结构上不含凭据

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `parse_model_spec` 后缀剥离 | 正向 | `"claude-sonnet-4-5[1m]"` → `name == "claude-sonnet-4-5"`、`length == Some(1_000_000)`（1000 进制，Q4b）；大小写三形态 `"x[1m]"` / `"x[1M]"` / `"x[1M]"`（全大写）同结果且裸名 `"x"` 保真（ASCII 大小写不敏感只作用于后缀，不改裸名大小写）；`"claude-haiku-4-5[1m]"`（含连字符数字）同样剥离 | 新增 |
| `parse_model_spec` 无后缀 | 正向 | `"claude-opus-4-1"` → `name` 逐字原文、`length == None`；`"gpt-4o-mini"` 同理（无方括号即零改写，openai 侧回归面） | 新增 |
| `parse_model_spec` 空白裁剪 | 正向 | `"  x[1m]  "` → `name == "x"`；`"x [1m]"` → `name == "x"`（剥离后裸名再 trim，DD8 ⑤）；前后空白 + 无后缀 → `name` 为 trim 后原文 | 新增 |
| `parse_model_spec` 拒收面 | 异常 | `"[2m]"` / `"[200k]"` / `"[8k]"` / `"[]"` 逐个 `Err`，`err.raw` 为传入原文（未 trim 前的原文）、`reason` 非空；`"x[2m]"` 同样 `Err`（不做单位换算、不降级为无后缀） | 新增 |
| `parse_model_spec` 方括号位置 | 异常 | 非末尾形态 `"x[1m]y"` / `"[1m]x"` / `"x[1m]y[1m]"`（尾缀剥离后裸名残留 `[`）→ `Err`；`"[[1m]"` → `Err`（剥离后裸名含 `[`）；重复后缀 `"x[1m][1m]"` → `Err`（不贪婪多剥） | 新增 |
| `parse_model_spec` 内空白与裸名空后缀 | 异常 | `"[ 1m ]"` / `"[1 m]"` / `"[1m ]"` → `Err`（内空白不做宽松匹配）；裸名为空 + 后缀 `"[1m]"` → `Err`（方括号语义悬空，DD8 ②） | 新增 |
| 空输入与纯空白 | 边界 | `""` / `"   "` / `"\t\n"` → `Ok(ModelSpec { name: "", length: None })`（**保现状**：provider 三档模型名可留空保存，保存期不得新增拒收面，DD8 ①）；`length` 恒 `None` | 新增 |
| 字符串边界载荷 | 边界 | 超长（>1000 字符）无后缀 / 带 `[1m]` 后缀 → 正常剥离且不 panic、首尾无截断；特殊字符 `"模型-🎉[1m]"` → `name == "模型-🎉"`（UTF-8 边界不切碎，DD9 字节切片安全性）；含 `\n` / `\0` / emoji 的裸名原样保真；`"\0[2m]"` → `Err` 且 `Display` 不 panic | 新增 |
| 长度阈值形态 | 边界 | 恰 4 字节 `"[1m]"`（后缀最低长度分支）→ `Err`（裸名空）；3 字节 `"[1"` → `Err`（含 `[` 但非完整后缀）；`"a[1m]"`（裸名恰 1 字符）→ `Ok`；`"ab"`（无方括号、长度 < 4）→ `Ok` | 新增 |
| `ModelSpec` / `ModelSpecError` 值语义 | 边界 | `ModelSpec` / `ModelSpecError` 的 `PartialEq` / `Clone` / `Debug` 派生可用（错误相等按 `raw` + `reason` 精确匹配，非按消息字符串）；`Display` 输出逐字含 `raw` 与 `reason` 两段且不含任何凭据类字段（结构上无 `api_key`，AC-10 错误串约束由结构保证）；`length` 的 `Some(1_000_000)` 与 `Some(1_048_576)`（1024 基）**不相等**（防实现误用 1024 基，Q4b / R11） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 纯 std 函数 + 值对象，无 I/O / 网络 / store / 全局态，全部真实调用（`model_spec.rs` 新增文件本身即「无进程边界依赖」设计） | 全部用例 |

### packages/desktop/src-tauri/crates/infra/store/src/model.rs -> packages/desktop/src-tauri/crates/infra/store/src/model_test.rs

#### 待测功能

- `AgentProviderRecord::new(name: String, protocol: AgentProviderProtocol, base_url: String, api_key: String, models: AgentModelTiers, context_length: Option<u64>, max_tokens: Option<u64>) -> Self`: 七参构造（DD17，`protocol` 紧跟 `name`），`id` 恒置 0（写事务内 max+1 覆盖）；`protocol` 决定 `base_url` 语义，`max_tokens` 为 `None` = 按协议缺省

（同文件的设计声明面，非公共 API 表行：`AgentProviderProtocol` 变体序号锚（AC-2）、`AgentProviderRecordV2` 中转形态与逐级 `From` 链（DD18）、手写遮蔽 `Debug` 扩位（AC-3）、`#[serde(default)]` 缺列读兼容。）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `AgentProviderRecord::new` 七参构造 | 正向 | `new("端点", Anthropic, "https://api.anthropic.com", "sk-ant-x", tiers, Some(200_000), Some(8192))` → 七字段逐字段保真、`id == 0`；`protocol = Openai` + `max_tokens = None` 变体同理（两臂各一行） | 新增 |
| `AgentProviderRecord::new` 可空列边界 | 边界 | `context_length` / `max_tokens` 四组合（`None/None`、`Some(0)/None`、`None/Some(0)`、`Some(u64::MAX)/Some(u64::MAX)`）逐字段透传，构造侧零校验、零缺省字面（不落 0 / 128000 / 32000） | 新增 |
| `AgentProviderRecord::new` 字符串边界 | 边界 | 空串 `name` / `base_url` / `api_key`、三档 `models` 全空串、超长（>1000 字符）`base_url`、含 `\n` / `\0` / emoji / 中文的字段 → 构造合法且保真（保存期拒收面在命令层，store 不得新增拒绝）；`Clone` / `PartialEq` 往返相等 | 新增 |
| v2 → v3 升级 | 正向 | `native_model::encode` 出 v2 载荷（`AgentProviderRecordV2`）→ `decode::<AgentProviderRecord>` 成功，`protocol == Openai`、`max_tokens == None`、其余字段逐字段相等（存量行行为零变化） | 新增 |
| v1 → v2 → v3 两级链 | 正向 | `AgentProviderRecordV1` 载荷 → `decode::<AgentProviderRecord>` 成功：`protocol == Openai`、`max_tokens == None`、`context_length == None`（逐级 `From` 对齐，无跳级 impl） | 修改 |
| `AgentProviderRecordV2` 直达升级 | 正向 | `AgentProviderRecordV2::from(v1)` → `AgentProviderRecord::from(v2)` 单步 `From` 各自字段面正确（V1→V2 补 `context_length = None`；V2→V3 补 `protocol` / `max_tokens`） | 新增 |
| 降级半边 | 边界 | `AgentProviderRecord::from` → `AgentProviderRecordV2` 丢 `protocol` / `max_tokens` 两列、其余保真；`V2 → V1` 再丢 `context_length`；降级产物不再可逆（不承诺降级数据真实性） | 新增 |
| 版本头 | 边界 | 新记录 `native_model` 编码后版本头逐字为 `3`（`native_model::Model` 版本读取面断言）；`AgentProviderRecordV2` 版本头为 `2`、`AgentProviderRecordV1` 为 `1` | 修改 |
| 变体序号锚（AC-2） | 边界 | `AgentProviderProtocol` bincode 载体序号：`Openai` 编码序号 0、`Anthropic` 序号 1，编解码往返两变体相等；变体声明序漂移即本行失败（解码锚 MUST NOT 重排） | 新增 |
| 模型组注册（AC-2） | 边界 | `global_models()` 模型组恰含 `AgentProviderRecord`（id 5 / v3）与既有记录面，**不含** `AgentProviderRecordV1` / `AgentProviderRecordV2`（中转形态不进库模型组、不出公共查询面） | 新增 |
| serde 线值（AC-1） | 正向 | `serde_json::to_value` → `protocol` 线值逐字 `"openai"` / `"anthropic"`（camelCase）、`maxTokens` 键在线（`Some(8192)` → `8192`、`None` → `null`）；`from_value` 反序列化后与 `PartialEq` 相等 | 新增 |
| serde 缺列兼容 | 边界 | `protocol` 键缺席 → 反序列化为 `Openai`（`#[serde(default)]`）；`maxTokens` 键缺席 → `None`；两键同时缺席（v2 线格式）→ 记录可读且语义等于升级结果 | 新增 |
| serde 非法载荷 | 异常 | `protocol` 非法线值（`"Openai"` / `"claude"` / `""` / `null`）→ 反序列化 `Err`（值域受控，不静默兜底）；`maxTokens` 负值 / 小数 / 字符串 → `Err`；`maxTokens` 超 `u64` 上界 → `Err` | 新增 |
| 遮蔽 `Debug`（AC-3） | 正向 | `format!("{:?}")` 输出含 `protocol` 与 `max_tokens` 两个位（`2` 变体各断一次）、`api_key` 位为 `sk-***890` 形态、全文不含明文 key；两列可选值 `None` 时呈现 `None`（位不缺失） | 修改 |
| 遮蔽 `Debug` 兜底 | 边界 | `api_key` 为空串 / 长度 < 3 时恒 `sk-***` 全遮蔽（既有兜底不回归）；`max_tokens = Some(0)` / `u64::MAX` 位面原样呈现；`Debug` 不改变 `PartialEq`（同内容两记录 `==` 为真、仅 `protocol` 不同则不等） | 修改 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 数据文件 / DB（版本链真库路径） | 复用既有 `envelope_test.rs` / `store_test.rs` 的 `tempfile` 数据根 + 真实 `native_db` 库（仅 1–2 处 `new(...)` 调用点补 `protocol` / `max_tokens` 实参 = 编译面随动）；`model.rs` 主体用例走 `native_model::encode` / `decode` **内存往返**，不落库 | 版本链 / 线值 / 模型组用例 |
| 内部模块 | 不 mock：`AgentModelTiers`、`mask_api_key`、`AgentEngineKind` 真实组合 | 全部用例 |
| 序列化器 | 不 mock：serde / bincode / native_model 真实实现（被测行为的载体本身） | 全部用例 |

### packages/desktop/src-tauri/src/commands/agents/mod.rs -> packages/desktop/src-tauri/src/commands/agents/mod_test.rs

#### 待测功能

- `save_agent_provider(stores, id, name, protocol, base_url, api_key, models, context_length, max_tokens) -> Result<AgentProviderRecord, String>`: 保存期校验**单点**（DD16）——①三档模型名按 `high → medium → low` 顺序经 `agent_runtime::parse_model_spec`（首个 `Err` 即返回，错误串含档位名 + `ModelSpecError` 原文，**不含 api_key**）；②`max_tokens = Some(n)` 且 `n < MIN_OUTPUT_TOKENS` → `Err`（文案含 `4096`）；③通过后执行既有留空 key 回填原值；④`upsert_agent_provider`（写面为 **upsert**：`id != 0` 时整行替换、行数不增，重复同 id 同载荷保存是幂等而非 append——design DD16 ④）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `save_agent_provider` 合法形态 | 正向 | `protocol = Anthropic` + 三档含 `[1m]`（如 `high: "claude-sonnet-4-5[1m]"`）+ `max_tokens = Some(4096)` → `Ok`，返回记录 `protocol` / `maxTokens` 逐字段保真、`[1m]` 原文**原样落库**（后缀不做归一化，摘除在引擎侧） | 新增 |
| `save_agent_provider` 未配置上限 | 正向 | `max_tokens = None` → `Ok` 且记录 `maxTokens == None`；`protocol = Openai` 同理（勿在保存期按协议拒收，R10 边界：字段照记） | 新增 |
| 留空 key 回填 | 正向 | `id = Some(存量)` + `api_key = ""` → 回填库中明文原值后 `Ok`（既有语义零改，与新增校验共存：校验前置不影响回填时机）；`id = None` + 空 key → 按新建语义保存 | 修改 |
| 非法后缀（high 档） | 异常 | `high: "x[2m]"` → `Err` 且文案含档位名 `high` 与原文 `x[2m]`；`low` / `medium` 档同为非法形态（各一行，锁三档均被校验） | 新增 |
| 非法后缀形态表 | 异常 | `"[8k]"` / `"[]"` / `"[ 1m ]"` / `"x[1m]y"` / `"[1m]"`（裸名空）逐个 `Err`；`Err` 串与原串逐字对应（与引擎侧同一 `parse_model_spec`，无第二套规则） | 新增 |
| `max_tokens` 下限 | 异常 | `Some(4095)` / `Some(0)` / `Some(1)` → `Err` 且文案含 `4096`（下限单点）；`Some(4096)` → `Ok`（恰下限放行，AC-10） | 新增 |
| 错误串机密边界 | 异常 | 构造带独特明文 key（如 `sk-ant-UNIQUE-LEAK-CANARY`）的调用：任一 `Err` 串均不含该 key 子串、不含 `api_key` 值本身（错误面只含档位名 / 模型名原文 / 数字下限） | 新增 |
| 校验顺序（DD16） | 边界 | 三档模型名非法 **且** `max_tokens = Some(1)` 同时成立 → 返回**模型名**错误（①先于②）；`high` 与 `low` 双非法 → 先报 `high`（档序 `high → medium → low`）；校验前置不产生库 IO 副作用（失败后 `list_agent_providers` 计数不变） | 新增 |
| 空模型名不新增拒收面 | 边界 | 三档全 `""` → `Ok`（保现状：齐备性由启动期 `ConfigMissing` 承担，DD8 ①）；单档 `""` 其余带后缀 → `Ok` | 新增 |
| 重复保存幂等（同 id upsert，T7） | 边界 | 同一 `id = Some(存量)` + **同一载荷**（`protocol = Anthropic` / `max_tokens = Some(8192)` / 三档含 `[1m]`）连续保存两次，断言三面：①第二次返回记录 `id` 与首次逐字相同、`protocol` / `maxTokens` / `models.high`（`"claude-sonnet-4-5[1m]"` 原文不叠加成 `[1m][1m]`）/ `base_url` / `name` / `contextLength` 七字段与首次逐字一致；②`list_agent_providers` 计数恒 `1`（行数不增、无重复行、无新 id）；③第二次把 `api_key = ""` 再存 → 回填原值，返回记录 `api_key` 与首次逐字相同、计数仍 `1`（写面为 upsert 整行替换，非 append；design DD16 ④） | 新增 |
| 数值 / 字符串边界 | 边界 | `max_tokens = Some(u64::MAX)` → `Ok`（上限不设拒收）；`context_length` 与 `max_tokens` 双可空独立组合（四态）；超长模型名（>1000 字符）带 `[1m]` → `Ok`；模型名含 emoji / `\0` / 换行 → `Ok` 且落库保真 | 新增 |
| 组合：保存 → 清单 → 重开库（AC-10 落库半边） | 组合 | 入口 = 命令层（IPC 链路发起方）：`save_agent_provider(Anthropic, max_tokens = 8192, 三档含 [1m])` → `list_agent_providers` 回读 `protocol` / `maxTokens` / 模型原文逐字一致 → `WorkspaceStores` 重开后仍一致（真库往返，v3 版本链顺带落地） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 运行环境（Tauri 运行时） | 既有 `tauri::test::mock_app()` + `app.manage(WorkspaceStores::open(tempdir))`（`Env` + `app_with_stores` 装置），命令以 `State` 直调 | 全部用例 |
| 数据文件 / DB（进程边界） | `tempfile::TempDir` 真实数据根 + 真实 `native_db` 库，**不 mock store**（校验顺序 / 落库往返须经真库） | 全部用例 |
| `agent_runtime::parse_model_spec` / `MIN_OUTPUT_TOKENS` | **不 mock**（最小 mock 原则：上界校验必须与引擎侧同一函数同一常量，mock 掉即失去「无第二套规则」的证明力） | 全部用例 |
| 内部模块 | 不 mock：`store` 的 `upsert_agent_provider` / `find_agent_provider` / `list_agent_providers` 真实组合 | 全部用例 |

### packages/desktop/src-tauri/crates/infra/agent/src/lib.rs -> packages/desktop/src-tauri/crates/infra/agent/src/lib_test.rs

#### 待测功能

- `EngineFacade::with_max_tokens(self, max_tokens: Option<u64>) -> Self`: 与 `with_context_window` 同型的旁路 builder（`max_tokens` MUST NOT 进 `EngineConfig`）
- `EngineFacade::with_context_window(self, context_window: Option<u64>) -> Self`: 签名不变，体内补 `max_tokens` 透传，语义降为「窗长缺省来源」
- `EngineFacade::runner_for(&self, kind: EngineKind, engine_cfg: EngineConfig) -> Result<Box<dyn AgentRunner>, String>`: sdk 臂尾挂 `.with_max_tokens(self.max_tokens)`；唯一 match 点结构与 `EngineKind` 双值不塌

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `with_max_tokens` 分发等价 | 正向 | `EngineFacade::new().with_max_tokens(Some(8192)).runner_for(Sdk, cfg)` 与 `SdkRunner::new(cfg, None, None, None).with_max_tokens(Some(8192))` 在同一 `SessionOpen` 下事件面 / 错误面行为一致（既有「与直构一致」对拍模式的延伸） | 新增 |
| builder 链式组合 | 正向 | `with_context_window(Some(200_000)).with_max_tokens(Some(8192))` 与反向顺序 `with_max_tokens(..).with_context_window(..)` 两链：两载荷互不覆盖（窗长与上限各自生效，用带 `[1m]` / 无后缀两组配置分别对拍） | 新增 |
| `with_max_tokens` 缺省 | 边界 | 不调 builder（`new()` / `with_resume_transcript()` / `with_explore_note_store()` 三条既有构造路径）→ `max_tokens` 恒 `None`，行为与现状一致（三处初始化补 `None` 的漏改即失败） | 新增 |
| `with_max_tokens` 载荷边界 | 边界 | `Some(0)` / `Some(u64::MAX)` / `None` 三态均构造可达且原样透传（门面不做下限校验——下限只在命令层，DD6 单点）；重复调用 builder 后者覆盖前者 | 新增 |
| `runner_for` sdk 臂启动校验 | 异常 | `EngineConfig::empty()`（`protocol` 缺省 Openai）→ `ConfigMissing` 且成因集逐字为既有三档（`api_key` / `base_url` / 模型档），`with_max_tokens` 不改变成因集、不新增变体 | 修改 |
| `runner_for` Cli 臂回归 | 异常 | `EngineKind::Cli` → `Err(CLI_TENANT_RETIRED)` 逐字、不构造引擎、不触装载缝（调用计数恒零）；builder 新增不影响该臂 | 保留 |
| `EngineKind` 双值穷举 | 边界 | 两变体 `match` 恰一分支可达（穷举锚：新增协议**不进** `EngineKind`，DD1 / 决策留痕 #7）；`runner_for` 结构化返回值仍为 `Box<dyn AgentRunner>`（引擎具体类型不出门面） | 修改 |
| crate 根导出面锚定 | 边界 | `parse_model_spec` / `ModelSpec` / `ModelSpecError` / `MIN_OUTPUT_TOKENS` 经 crate 根可达（用例内 `use` 锚定，破坏即编译失败；命令层同一函数消费的前提） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 网络（进程边界） | `EngineConfig.base_url` 指本机 discard 端口 `http://127.0.0.1:9/v1`（恒拒连，组装不触网） | 全部分发 / 对拍用例 |
| 续会话转录（被测 API 显式入参） | `ResumeTranscript` 闭包替身（既有 `counting_loader`：按 session_id 返 `Vec<AgentEvent>` / `None` / `Err`，并计数） | `runner_for` 校验面 / Cli 零触达 |
| workspace 库句柄（被测 API 显式入参） | `tempfile` 真库 `Arc<Store>`（既有 `temp_store` 装置） | `with_explore_note_store` 缺省路径用例 |
| 内部模块 | 不 mock：`SdkRunner` / `EngineConfig` / `EngineKind` 真实组合（门面行为必须与直构对拍） | 全部用例 |

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/config.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/config_test.rs

#### 待测功能

- `Protocol::as_str(self) -> &'static str`: 恒 `"openai"` / `"anthropic"`（日志与诊断面 `protocol=…`，DD1 / DD12）
- `EngineConfig::empty() -> Self`: 体内补 `protocol: Protocol::Openai`（签名不变）
- `EngineConfig::is_complete(&self) -> bool`: 实现与语义**零改**（协议恒有值，不参与校验），仅 doc 口径改「连接配置字段面齐备检查」

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `Protocol::as_str` | 正向 | `Openai.as_str() == "openai"`、`Anthropic.as_str() == "anthropic"` 逐字（日志锚，不得为 `debug` 名或大小写变体） | 新增 |
| `Protocol` 缺省与比较 | 边界 | `Protocol::default() == Openai`（`EngineConfig::empty()` 的缺省来源）；`Copy` / `PartialEq` / `Eq` 可用：两变体互不相等、拷贝后等值；不派生 serde / specta（编译面锚：不出 crate、不进 bindings，DD1） | 新增 |
| `EngineConfig::empty` | 正向 | 四字段全空串 + `protocol == Openai`（缺省形状锁定，防实现引入隐式缺省）；手填构造（含 `Protocol::Anthropic`）逐字段保真可读 | 修改 |
| `is_complete` 语义不变 | 正向 | 四字段齐备（`api_key` / `base_url` / `model_high` / `model_low` 均非空）→ `true`，且 `protocol` 为 `Anthropic` 时仍 `true`（协议不参与校验） | 修改 |
| `is_complete` 逐字段缺失 | 异常 | 四字段各自单独为空 → `false`（四行或表驱动）；多字段同时缺失仍 `false`；`protocol` 为任一值均不影响判定（与现状逐字等价） | 修改 |
| 字段载荷边界 | 边界 | 全空白串 `"   "` / `\0` / emoji / 超长（>1000 字符）字段 → `is_complete` 只判 `is_empty()`（空白串非空即 `true`，语义与现状一致，不新增 trim 行为）；`PartialEq` 两配置仅 `protocol` 不同则不等、其余字段相同 + 同协议则相等 | 修改 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 纯值类型 + 纯函数断言，无进程边界依赖，全部真实构造（`EngineConfig` 刻意不派生 `Debug`——比较一律走字段面 / `assert!`，沿用既有用例口径） | 全部用例 |

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner_test.rs

#### 待测功能

- `SdkRunner::with_max_tokens(self, max_tokens: Option<u64>) -> Self`: 旁路载荷注入（crate 内 API，DD14）；`SdkRunner::new` 四参签名**不动**、内给 `max_tokens: None`（36 处调用点零改）
- `MIN_OUTPUT_TOKENS: u64 = 4_096`: provider `max_tokens` 保存期下限单点（经 crate 根再导出，DD6）

（同文件的设计声明行为面，非公共 API 表行：`open_session` 按档 `parse_model_spec` + 裸名下发 + 按档现算窗长（DD11 / DD12）、`build_model(config, spec, max_tokens)` 双臂（DD3 / DD4 / DD5）、`OUTPUT_TOKENS_FALLBACK = 32_000`（DD5）、`PumpPayload` 日志载荷。）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `with_max_tokens` 构造 | 正向 | `SdkRunner::new(cfg, None, None, None).with_max_tokens(Some(8192))` 可达且 `open_session` 行为与不带该载荷时相同（载荷不改变非相关行为）；`SdkRunner::new` 四参调用形态编译可达（36 处零改的编译面锚） | 新增 |
| `MIN_OUTPUT_TOKENS` | 正向 | 常量值逐字 `4_096`，且经 crate 根 `agent_runtime::MIN_OUTPUT_TOKENS` 可达（保存期与引擎同一门槛，无第二套字面量） | 新增 |
| 双臂组装：openai（AC-5） | 正向 | `protocol = Openai` + 无后缀模型名 → 产物 `DynModel::name() == "openai"`、`id()` / 描述面为模型名原文；臂内**不写入**任何请求参数（`max_tokens` 保持省略，AC-8 后半） | 新增 |
| 双臂组装：anthropic（AC-5） | 正向 | `protocol = Anthropic` + `"claude-sonnet-4-5[1m]"` → 产物 `name() == "anthropic"`、`id()` = 裸名 `claude-sonnet-4-5`；两臂同型 `DynModel<Completion>`（同一变量位接受两臂产物即编译期锚，loop 零改动可消费） | 新增 |
| `base_url` 归一化（AC-5 / R8） | 边界 | anthropic 臂 `base_url` 三形态 `"https://api.anthropic.com"` / 带 `/v1` / 带 `/v1/messages` → 归一化后不产生重复版本段（逐字请求路径面断言）；openai 臂 `"…/v1"` 保持原样拼接（逐字等价现状） | 新增 |
| 裸名上报（DD12 / R9） | 正向 | `open_session`（anthropic，模型名 `"claude-x[1m]"`）→ 首轮观察流 `RunStarted.model == Some("claude-x")`（裸名，反映实际发送）；泵启动日志行 `protocol=anthropic model=claude-x window=1000000 raw=claude-x[1m]` 四段齐备 | 新增 |
| 按档窗长：后缀胜出（AC-7） | 组合 | 入口 = `open_session`（引擎装配链路发起方）：`EngineConfig { model_high: "claude-x[1m]", .. }` + `context_window = Some(64)` + 会在 64 窗下越过 L2 的小体量重建史 → **无** L2 `SystemNotice`（1M 窗压过 `context_length`）；同一 runner 的 `low: "claude-y"`（无后缀）轮 + 同等体量史 → **有** L2 notice（回落到 64），两档独立解析（`Mock` 面见下） | 新增 |
| 按档窗长：`context_length` 与缺省（AC-7） | 边界 | 无后缀 + `context_window = Some(64)` → 防线窗 64；`context_window = None`（或 `Some(0)`）→ 128K 缺省（`ContextDefense::resolve` 既有归一化不变，与 `context_test` 缺省锚同源）；`high` / `low` 各带不同后缀形态 → 两档窗长各不同 | 新增 |
| 非法后缀启动失败 | 异常 | `model_high = "x[2m]"` → `open_session` 返回 `Err(ConfigMissing)`，文案含档位 `model_high` 与 `ModelSpecError` 原文（不 panic、不起泵、零事件产出）；`model_low = "x[1m]y"` 的低档轮同样 `Err`（按档解析，`high` 档轮照常） | 新增 |
| 空模型名既有路径 | 异常 | 三档中 `model_high = ""` → 既有 `ConfigMissing`（配置校验**先于** `parse_model_spec`，空名不进入 parse 分支、不产生后缀错误）；`""` + `context_window` 在场仍先报配置缺失 | 修改 |
| 输出上限取值链（AC-8） | 边界 | anthropic 臂三态：①`with_max_tokens(Some(8192))` → 生效值 8192（显式优先，不被 rig 官方值覆盖）；②`None` + 名单内模型（`claude-opus-4*` / `claude-sonnet-4*` / `claude-haiku-4-5`）→ 保留 rig 官方 `default_max_tokens`（不被降级）；③`None` + 名单外模型（`claude-3-5-sonnet-*` / 第三方网关名）→ 兜底 `32_000` | 新增 |
| beta 条件发（AC-9） | 边界 | 四组合：anthropic + `[1m]` → `anthropic-beta` 含 `context-1m-2025-08-07`；anthropic + 无后缀 → 不发；openai + `[1m]` → 不发（本地窗长仍为 1M，但不发 beta，R4）；openai + 无后缀 → 不发；beta 判定只认可考表示 `spec.length == Some(1_000_000)` | 新增 |
| 字符串 / 数值边界 | 边界 | 模型名超长（>1000 字符，带 / 不带后缀）/ emoji / `\0` / 换行 → 组装不 panic 且裸名保真；`with_max_tokens(Some(0))` / `Some(u64::MAX)` → 原样作为 wire 默认值（引擎不做下限校验）；`context_window = Some(u64::MAX)` 下阈值计算不溢出 | 新增 |
| 组合：引擎三模块链路 | 组合 | 入口 = `open_session`：`model_spec`（解析）+ `context`（按档防线）+ 双臂 `build_model` → 一次 `open_session` 同时断言 裸名上报 + 窗长 + 上限载荷（三面同源、单次装配不打架；discard 端点，不打网络） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 网络 / provider 端点（进程边界） | `base_url` 指本机 discard 端口 `http://127.0.0.1:9/v1`（既有 `complete_config()` 底座，恒拒连）——断言面限于 `DynModel` 描述面 / 臂内构造中间值，**不打真端点**（AC-5 / AC-8 / AC-9 的断言纪律） | 双臂 / 上限链 / beta 用例 |
| 组装面可测性接缝（实现相位配合，非 mock） | `build_model` 提升 `pub(crate)`（或取值链 / beta 条件拆 `pub(crate)` 纯函数），使 `runner_test` 可直断 `name()` / `id()` / 上限值 / betas 计数；不新增 crate 外导出、不进 bindings | 双臂 / 上限链 / beta / `base_url` 归一化用例 |
| 全局日志状态（运行环境） | 复用 `sdk/log.rs` 的 `init_engine_log(tempdir)`：进程级 `OnceLock` 一次性初始化 → 日志断言以「按 session id 定位 + `contains` 分段匹配」实施（不断言整行全等），未初始化时以 `RunStarted.model` 面替代裸名断言 | 裸名上报 / 泵启动日志载荷 |
| 续会话装载缝（被测 API 显式入参） | 既有 `loader` 闭包替身（`minimal_transcript` / `oversized_transcript` 两底座：后者用于触 L2 水位） | 按档窗长 / 续会话回归 |
| workspace 库句柄（被测 API 显式入参） | `temp_store`（`tempfile` 真库 `Arc<Store>`） | explore 注入回归用例（保留） |
| 内部模块 | 不 mock：`model_spec` / `context` / `resume` / `loop` 真实组合（协议差异对 loop 不可见是本变更的核心承诺，mock 掉即失去证明力） | 全部用例 |
| `rig` crate | 不 mock：真实 `rig::providers::anthropic` / `openai` 构造（fake 会掩盖 wire 形态差异） | 全部用例 |

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/context.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/context_test.rs

#### 待测功能

- `ContextDefense::window(&self) -> u64`: `pub(crate)` 只读访问器（DD13）——返回生效窗长 `self.context_window`，为泵启动日志 `window=…`（DD12）的唯一不加规则来源（128K 缺省与「0 → 缺省」归一化留在 `resolve` 单点，runner 侧不重算）
- `ContextDefense::resolve(window: Option<u64>) -> Self`: 既有归一化入口（签名与语义零改）——`None` / `Some(0)` → 128K 缺省、非零原样；`window()` 取值的唯一来源
- `ContextDefense::over_l2(&self, history) -> bool` / `ContextDefense::over_l3(&self, history) -> bool`: 既有水位比判定回归面——阈值仍按 `window * 3 / 4` / `window * 9 / 10` 派生，新增 getter 不复制水位常量、不改算法

design.md 公共函数 / API 子表未声明本文件的公共 API 变更（上述均为 `pub(crate)` 内部面，`ContextDefense` 以「修改（只读）」入类型定义表）；用例按 AC-7 与 `context_test` 既有缺省锚设计。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `ContextDefense::window` 生效窗长 | 正向 | `resolve(Some(200_000)).window() == 200_000`；`resolve(Some(1_000_000)).window() == 1_000_000`（后缀档的窗长来源在 runner 侧 `spec.length.or(...)` 解析后照旧传入，本行只验 getter 语义） | 新增 |
| `ContextDefense::window` 缺省归一化 | 边界 | `resolve(None).window() == 128 * 1024`、`resolve(Some(0)).window() == 128 * 1024`（「0 → 缺省」归一化留在本模块单点，不在 runner 侧重算，DD13） | 新增 |
| `window()` 只读且不改规则 | 边界 | 重复调用恒等（无副作用）；`window()` 值与 `l2_threshold` / `l3_threshold` 推导一致（`window * 3 / 4` / `window * 9 / 10`）——即 getter 不复制水位常量；`Some(u64::MAX)` 窗长下阈值计算不 panic / 不溢出 | 新增 |
| 防线语义零改回归 | 保留 | 既有 L2 剪裁 / L3 compaction / 保护窗 40k / prune 门槛 20k 用例全部保留（`window()` 新增不改变任何阈值与算法；`prune` / `protected_start` / `estimate_history` 断言逐字不变） | 保留 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | `ContextDefense` 为纯内存结构，消息 fixture 真实构造（既有 `user_at_tokens` 自校准装置），无 I/O / 网络 / 全局态 | 全部用例 |

### packages/desktop/src-tauri/crates/infra/agent/src/compose.rs -> packages/desktop/src-tauri/crates/infra/agent/src/compose_test.rs

#### 待测功能

- `resolve_agent_engine(..)`: 组合根解析臂（既有签名与拒绝语义零改）——sdk 臂把 `provider.protocol` 经 `map_protocol` 写入 `EngineConfig.protocol`，并把 `provider.max_tokens` 与 `context_length` 同型旁路到 `ResolvedEngine.max_tokens`
- `map_protocol(protocol: AgentProviderProtocol) -> Protocol`: 私有单点映射（DD15）——`Openai` / `Anthropic` 一对一，无第三分支、无兜底缺省
- `ResolvedEngine.max_tokens: Option<u64>`: 组合根内部结构字段（旁路载荷，与 `context_window` 并列）——`None` 原样透传、不落缺省字面（DD5 缺省归引擎臂）
- `compose_turn(..)`: 运行发起链路末端装配——runner 构造链插 `.with_max_tokens(resolved.max_tokens)`，协议 + 上限 + 后缀三面同源下发

design.md 公共函数 / API 子表未声明本文件的公共 API 变更（`map_protocol` 为私有映射、`ResolvedEngine.max_tokens` 为组合根内部结构字段）；用例按 AC-4 与 spec「协议透传到引擎」设计。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 协议透传（两臂映射） | 正向 | provider `protocol = Anthropic` → `resolve_agent_engine` 产物 `config.protocol == Protocol::Anthropic`；`protocol = Openai` → `Protocol::Openai`（两臂各一行，映射不全即失败）；`config` 其余字段（`api_key` / `base_url` / `model_high` / `model_low`）逐字段保真 | 新增 |
| `max_tokens` 旁路携带 | 正向 | provider `max_tokens = Some(8192)` → `ResolvedEngine.max_tokens == Some(8192)`（与 `context_window` 并列）；`None` 透传 `None`（不落缺省字面） | 新增 |
| 双可空列并存 | 边界 | `context_length` 与 `max_tokens` 四组合（`None/None`、`Some/None`、`None/Some`、`Some/Some`）→ 两字段各自独立携带、互不覆盖；`max_tokens = Some(0)` 原样携带（解析层不做下限校验，拒面在命令层） | 新增 |
| 既有解析语义零改 | 异常 | `EngineKind::Cli` 实例 → `Err(CLI_TENANT_RETIRED)` 逐字（不构造引擎）；默认实例缺席 / 显式 id 不存在 / provider 悬空 / `provider_id = None` → 各成因文案逐字不变（新增字段不改变既有拒面） | 修改 |
| 组合：provider → 门面 → 引擎（协议 + 上限 + 后缀全链） | 组合 | 入口 = 组合根（`resolve_agent_engine` / `compose_turn`，运行发起链路最上层）：真库落 provider（`Anthropic` + `max_tokens = 8192` + `high: "claude-x[1m]"`）→ 解析 → `EngineFacade::with_context_window(..).with_max_tokens(..)` → `runner_for(Sdk)` → `open_session` 首轮：`RunStarted.model == Some("claude-x")`（裸名上报）、`SessionConfigSnapshot.model` 仍为配置原文 `"claude-x[1m]"`（R9 裁定双面）、泵启动日志 `protocol=anthropic … window=1000000 raw=claude-x[1m]` | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 数据文件 / DB（进程边界） | 既有 `DualEnv`（`tempfile` 数据根 + workspace 根双临时目录）与真实 `WorkspaceStores`（`seed_provider` / `seed_sdk_instance` 装置补 `protocol` / `max_tokens` 实参） | 全部用例 |
| 网络（进程边界） | provider `base_url` 指本机 discard 端口 `http://127.0.0.1:9/v1`（解析仅组装不触网） | 全部用例 |
| 全局日志状态（运行环境） | 同 `sdk/runner.rs`：`init_engine_log(tempdir)` + 按 session id 定位 + `contains` 分段匹配 | 组合用例 |
| 内部模块 | 不 mock：`compose.rs` 的 `map_protocol`、`EngineFacade`、`SdkRunner` 真实组合（「协议抵达引擎」必须端到端可考，mock 掉任一层即失去 AC-4 的证明力） | 全部用例 |

### packages/desktop/src/views/agents/hooks/use-agent-providers.ts -> packages/desktop/src/views/agents/hooks/use-agent-providers.test.ts

#### 待测功能

- `AgentProviderSaveInput.protocol` / `AgentProviderSaveInput.maxTokens`: 前端内部入参接口（DD20）——`protocol` 紧跟 `name`、`maxTokens` 留空 = 按协议缺省存 `null`（MUST NOT 落 0 / 32000 字面）
- `useAgentProviders().save(input)`: IPC 入参序映射单点——`commands.saveAgentProvider(...)` 八参新序 `(id, name, protocol, baseUrl, apiKey, models, contextLength, maxTokens)`，返回值透传记录；失败轨既有 toast + `null` 双轨语义零改
- `AgentProvidersState.providers`: `list_agent_providers` 回读面——原样承载新增 `protocol` / `maxTokens`（hook 零裁剪、零归一化）

design.md 公共函数 / API 子表未声明本文件的公共 API 变更（`AgentProviderSaveInput` 为前端内部接口，hook 对外仍为 `AgentProvidersState`）；用例按 AC-11「保存入参」设计。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `save` 入参序（DD20） | 正向 | `save({ id, name, protocol: 'anthropic', baseUrl, apiKey, models, contextLength, maxTokens })` → `invoke('save_agent_provider', { id, name, protocol, baseUrl, apiKey, models, contextLength, maxTokens })` 逐键断言（键名 = 命令签名序，位置错序即失败）；返回值透传记录 | 修改 |
| `maxTokens` 未配置语义 | 正向 | `maxTokens: null` → invoke 实参 `maxTokens: null`（留空 = 按协议缺省存 null，MUST NOT 落 0 / 32000 字面）；与 `contextLength: null` 双空并存 | 新增 |
| `save` 失败轨回归 | 异常 | reject → `toast.error('保存 provider 失败：' + String(err))` 恰一次、返回 `null`、不置 `error` 态（既有双轨语义零改；新增字段不改变错误路径） | 保留 |
| `list` 回读新字段 | 边界 | `list_agent_providers` 返回含 `protocol` / `maxTokens`（两变体 + `null` / 数值各一行）→ `providers` 态原样承载（hook 零裁剪、零归一化） | 新增 |
| 数值边界 | 边界 | `maxTokens: 0` / `4096` / `Number.MAX_SAFE_INTEGER` → 原样透传（hook 不做边界校验，`< 4096` 拒面在面板 `canSave` 与后端）；`contextLength` 与 `maxTokens` 各自独立透传 | 新增 |
| 组合：面板实参 → hook → invoke | 组合 | 入口 = hook（IPC 链路发起方）：以面板提交形态的入参（`protocol: 'anthropic'` + `maxTokens: 8192` + 三档含 `[1m]`）调 `save` → invoke 载荷逐键与 `save_agent_provider` 签名对齐（面板侧字段名 → IPC 载荷名的映射单点可考） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `@tauri-apps/api/core` invoke（进程边界 IPC） | 既有 `vi.hoisted` + `vi.mock`：`invokeMock` 按命令名分发（`list_agent_providers` / `save_agent_provider` / `delete_agent_provider`），记录入参可逐键断言 | 全部用例 |
| `sonner` toast（进程边界 UI 作用域） | `vi.mock('sonner')` → `toast.error` 替身（断言调用次数与前缀文案） | 错误轨用例 |
| 内部模块 | 不 mock：hook 内 `commands` 包装、`useState` / `useEffect` 真实 React 运行时（`renderHook`） | 全部用例 |

### packages/desktop/src/views/agents/components/provider-panel.tsx -> packages/desktop/src/views/agents/components/provider-panel.test.tsx

#### 待测功能

- `ProviderFormState.protocol` / `ProviderFormState.maxTokens`: 受控表单内部接口（DD19）——`EMPTY_FORM` 缺省 `'openai'`、`maxTokens` 空串 = 未配置（提交时转 `null`）
- `ProtocolSelect(..)`: `StandardSelect` 驱动的协议选择器（row 0 前置）——`onChange(null)` 保持现协议值、不置空、不崩溃
- `BASE_URL_HINTS[form.protocol]`: `base_url` placeholder 与 `provider-base-url-hint` 文案的唯一来源，随协议双向切换
- `parseMaxTokens(value: string)`: 边界解析缝——`''` → `null`、非整数 / 精度外 → 非法、`< 4096` 拒绝提交
- `canSave()`: 提交门汇总——含 `parseMaxTokens` 判定与既有 `contextLength` 判定，非法即 `disabled` 且不调 `state.save`
- `toFormState(editing)`: 编辑态回填——`protocol` 原值、`maxTokens` 由 `null` → `''`
- `ProviderList(..)`: 清单行呈现面——`provider-protocol-badge` 协议徽标与 `provider-suffix-hint` 后缀提示（逐档列出 / 条件呈现）

design.md 公共函数 / API 子表未声明本文件的公共 API 变更（`ProviderFormState` 为前端内部接口，`ProtocolSelect` / `parseMaxTokens` / `BASE_URL_HINTS` 为模块内实现缝）；用例按 AC-11 设计，锚名（`provider-protocol-badge` / `provider-base-url-hint` / `provider-max-tokens-hint` / `provider-suffix-hint`）由 DD19 钉死供本相位消费。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 新建缺省协议 | 正向 | 点「新建」→ 协议选择器现值 `openai`（`StandardSelect` 真组件）；`base_url` placeholder 逐字为 openai 形态（含版本段提示 `https://…/v1`） | 新增 |
| 协议切换联动 | 正向 | 选择器切 `anthropic` → `base_url` placeholder 与 `provider-base-url-hint` 文案切换为 anthropic 形态（含「`/v1` 等后缀由引擎自动剥离后拼 `/v1/messages`」提示）；切回 `openai` 复原（受控表单双向） | 新增 |
| 保存入参（含新字段） | 正向 | 填完必填项 + `maxTokens = '8192'` + 协议 `anthropic` → 点保存：`state.save` 以 `{ id, name, protocol: 'anthropic', baseUrl, apiKey, models, contextLength, maxTokens: 8192 }` 恰调用一次 | 修改 |
| 列表协议徽标 | 正向 | `providers` 含 `openai` / `anthropic` 两条 → 每行 `provider-protocol-badge` 呈现且文案与记录 `protocol` 一致（不渲染空徽标 / 不串行） | 新增 |
| 后缀提示（DD19） | 正向 | 某档带 `[1m]` 且 `contextLength` 非空 → `provider-suffix-hint` 追加「`<档位>` 档窗长已被模型名覆盖」；无后缀（或 `contextLength` 空）→ 该提示不呈现；多档带后缀 → 逐档列出 | 新增 |
| 编辑态回填 | 正向 | 点编辑 → 表单 `protocol` / `maxTokens` 回填记录值（`maxTokens: null` → 空串「未配置」语义，不渲染 `"null"`）；提交后 `protocol` / `maxTokens` 原样回传 | 新增 |
| `maxTokens` 下限拒提交 | 异常 | `'0'` / `'4095'` / `'-1'` → 保存按钮 `disabled`、点击不触发 `state.save`（`canSave` 拦截）；`'4096'` → 可保存（恰下限放行，与后端 `MIN_OUTPUT_TOKENS` 对齐） | 新增 |
| `maxTokens` 非法形态 | 异常 | `'abc'` / `'1.5'` / `'8e3'` / `'8192abc'` → 保存按钮 `disabled`、不提交（非整数一律非法）；同时校验既有 `contextLength` 非法拒提交行为不回归 | 新增 |
| 协议选择器空值回调 | 异常 | `StandardSelect.onChange(null)`（清空 / 失焦形态）→ 表单保持现协议值、不置空、不崩溃（DD19 显式约束） | 新增 |
| `maxTokens` 字符串边界 | 边界 | `''`（留空）→ 提交 `maxTokens: null`；`' 8192 '`（前后空白）→ 裁剪后 `8192`；超长数字串（>20 位）→ 非法拒提交（`Number` 精度外不静默截断）；`'04096'` → `4096` 合法（数值语义） | 新增 |
| 既有面回归 | 保留 | 清单 name / base_url / 三档 model 呈现、`MaskedApiKey` 遮蔽、编辑态 api_key 恒空、删除动作、error / loading / empty 三态、`contextLength` 边界与提交 —— 既有用例全部保留（新增控件不改既有布局与可访问锚） | 保留 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `AgentProvidersState`（被测 API 显式入参） | `vi.fn` 替身（`save` / `remove` 返回 `Promise`，可覆写为 resolve / reject），组件不直调 IPC（面板经 props 注入 state） | 全部用例 |
| 运行环境（jsdom） | 既有 `src/test-setup.ts` 垫片（`ResizeObserver` / `getAnimations` / `BASE_UI_ANIMATIONS_DISABLED`）——`StandardSelect` 真组件需要 | 全部渲染用例 |
| 内部模块 | 不 mock：`StandardSelect`、`MaskedApiKey`、`Button` 真实组件与真实受控表单状态（协议联动 / 徽标 / 提示文案必须经真实组合呈现） | 全部用例 |

---

## 不可测试项

| 被测文件或模块 | 原因 |
|----------------|------|
| `packages/desktop/src/types/generated/bindings.ts` | codegen 生成物禁手改：`AgentProviderProtocol` 枚举、记录 `protocol` / `maxTokens`、`saveAgentProvider` 八参新序由 `pnpm -C packages/desktop run bindings:export` 再生，零漂移由 `bindings:check`（`git diff --exit-code -- src/types/generated`）承接（AC-12 静态半边）。无进程内测试落点 |
| `packages/desktop/src-tauri/crates/infra/store/src/lib.rs` | 改动为单条 `pub use model::{… AgentProviderProtocol …}`（纯再导出、零运行时逻辑）。可达性由编译面承载（命令层 `use store::{AgentProviderProtocol, …}` 与 `bindings:export` 消费同一导出面，漏导出即编译失败）；AC-2 的「不出公共查询面」由 `model_test.rs` 的模型组清单断言（`AgentProviderRecordV2` 未注册）+ `pub(crate)` 声明承载 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mod.rs` | 改动为两行模块挂载声明（`pub(crate) mod model_spec;` / `#[cfg(test)] mod model_spec_test;` 归测试相位）。无运行时逻辑；挂载正确性由编译面 + `model_spec_test.rs` 全部用例可编译可执行承载 |
| AC-12 静态检查链（`pnpm -C packages/desktop run check` 含 `bindings:check` / `knip` / `cargo clippy -- -D warnings`、`run test` 汇总） | 机械门禁由桌面端执行，非 agent 侧单元测试面；本文件只提供服务端 / 前端各自用例设计，不复制测试运行脚本细节 |
| AC-8 后半 `loop.rs` / `compact.rs` 零 diff（「anthropic 请求 `max_tokens` 仍由 wire 默认值兜底」的守线半） | 该约束是「代码面不存在变更」的静态性质，无运行时断言落点；由守线任务核验 diff + `runner_test` 的上限链用例（DD4 字段写入让 wire `encode` 取 `default_max_tokens`）间接锁定。同理 `crates/core/agent` 全量、`normalize.rs` / `resume.rs` 零改动以 diff 守线承载 |
| openai 协议「MUST NOT 写 `max_tokens`」的请求体级证明（AC-8 末项） | 请求体只在真实请求发送时可观测，进程内无法取得（discard 端点会拒连）。自动化面以「openai 臂不执行上限定型分支 + 产物描述面与现状逐字等价」锁定（`runner.rs` 章节「双臂组装：openai」行），请求体级实证不在本变更验收面（验收止于单元测试与静态检查链，真机冒烟不属验收条件） |
| 跨协议续会话的 reasoning 静默丢弃（Q3 / R3） | 知情边界，非缺陷亦无断言面：rig `replayable_to` 只回放本 dialect issuer 的 reasoning 块，改协议后 `Continue` 的思考块丢失是接受行为；`resume::rebuild` 口径零改，由既有 `resume_test.rs` 回归承载 |
| `[1m]` 对非 1M 模型的真实超窗行为（R4） | 后缀只抬本地防线估算与 beta 头，真实窗长由 provider 决定；错配后的超窗失败经 L4（provider 报错收敛）表现为真机行为，进程内无确定性断言面（表单提示与 spec 边界条已留痕） |
