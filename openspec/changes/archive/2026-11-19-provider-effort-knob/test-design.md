# 测试设计: provider-effort-knob

> **变更**: provider-effort-knob（provider 记录 `efforts{high, low}` sibling 列 + 引擎 effort 旁路 shaping + 静态模型目录 + `StandardCombobox`）
> **依据**: 本 change 目录 `proposal.md`（AC-1…AC-13）、`design.md`（新增 / 修改文件表、公共函数 / API 表、编译闭合批、DD1–DD14）、`specs/{desktop-agent-management,desktop-agent-execution,desktop-standard-components}/spec.md`
> **范围**: 只设计单元测试的路径、用例与 mock 策略；测试运行命令与静态检查链（`server:check` / `bindings:export` / `api:check` / `client:check` / `client:test`）由桌面端机械相位执行，本文件不复述。

## 验收范围

| AC ID | 验收条件 | 被测文件或模块 |
|---|---|---|
| AC-1 | 存储演进一条链：`AgentProviderRecord` 版本头 `5`（id 仍 5）、`AgentProviderRecordV4` 为 `pub(crate)` 中转、V4↔V5 双向 `From` 在场；v4 载荷字节经版本机制解码成功且 `efforts` 为空、其余逐字段相等；v1→v5 四级链与直接产出 v5 逐字段一致；V5→V4 丢 `efforts` 且其余保真 | `src-tauri/crates/infra/store/src/model.rs` -> `src-tauri/crates/infra/store/src/model_test.rs`；出口面 `src-tauri/crates/infra/store/src/lib.rs`（不可测试项 1） |
| AC-2 | 构造器面不动：`AgentProviderRecord::new` 参数面逐字不变（44 处调用点零改）、新增走 `with_efforts`；手写遮蔽 `Debug` 含 `efforts` 位且 api_key 位仍遮蔽 | `src-tauri/crates/infra/store/src/model.rs` -> `src-tauri/crates/infra/store/src/model_test.rs` |
| AC-3 | 保存期**只 `trim`**：两档各自裁两侧空白、空串 = 留空；自定义值（目录外字符串）可存可回读、不被拒收；`context_length` / `max_tokens` 的既有校验逐字保留 | `src-tauri/src/commands/agents/mod.rs` -> `src-tauri/src/commands/agents/mod_test.rs`；前端半 `packages/desktop/src/views/agents/hooks/use-agent-providers.ts` -> `packages/desktop/src/views/agents/hooks/use-agent-providers.test.ts` |
| AC-4 | 请求侧零痕迹：两档皆空时两协议请求的 `additional_params` 恒 `None`（openai 臂无 `reasoning_effort`、anthropic 臂无 `output_config`），与变更前逐字相同 | `src-tauri/crates/infra/agent/src/sdk/effort.rs` -> `src-tauri/crates/infra/agent/src/sdk/effort_test.rs`；`src-tauri/crates/infra/agent/src/sdk/loop.rs` -> `src-tauri/crates/infra/agent/src/sdk/loop_test.rs`；`src-tauri/crates/infra/agent/src/sdk/runner.rs` -> `src-tauri/crates/infra/agent/src/sdk/runner_test.rs`；挂载面 `src-tauri/crates/infra/agent/src/sdk/mod.rs`（不可测试项 2） |
| AC-5 | 请求侧形状：openai → `additional_params = {"reasoning_effort": v}`、anthropic → `{"output_config": {"effort": v}}`；与 `web_search` 同开时 `tools` 键与本载荷共存且互不覆盖 | `src-tauri/crates/infra/agent/src/sdk/effort.rs` -> `src-tauri/crates/infra/agent/src/sdk/effort_test.rs`；`src-tauri/crates/infra/agent/src/sdk/loop.rs` -> `src-tauri/crates/infra/agent/src/sdk/loop_test.rs` |
| AC-6 | 档位分发：`High` 取 `models.high` + `efforts.high`、`Low` 取 `models.low` + `efforts.low`；shaping 只在 `open_session` 一处；引擎日志含该轮生效 `effort=…`（留空记 `-`） | `src-tauri/crates/infra/agent/src/sdk/runner.rs` -> `src-tauri/crates/infra/agent/src/sdk/runner_test.rs`；`src-tauri/crates/infra/agent/src/lib.rs` -> `src-tauri/crates/infra/agent/src/lib_test.rs`；`src-tauri/crates/infra/agent/src/compose.rs` -> `src-tauri/crates/infra/agent/src/compose_test.rs`；挂载面 `src-tauri/crates/infra/agent/src/sdk/mod.rs`（不可测试项 2） |
| AC-7 | 目录五条不变量全绿（唯一 / 非空 / 零凭据 / `[1m]` 条目窗长 = 1M / `supportsThinking=false` ⇒ 显式空 `efforts`）；`list_model_catalog` 零参数零 IO 且已进 `all_commands!` | `src-tauri/crates/infra/agent/src/model_catalog.rs` -> `src-tauri/crates/infra/agent/src/model_catalog_test.rs`；`src-tauri/src/commands/agents/mod.rs` -> `src-tauri/src/commands/agents/mod_test.rs`；注册面 `src-tauri/src/commands/mod.rs` -> `src-tauri/src/bindings/mod_test.rs` |
| AC-8 | 目录**不进**运行期解析、**不进**保存期校验：取值链（`spec.length` → `context_length` → 128K；`max_tokens` → rig 名单 → 窗长分档）逐字不变，目录内容改动不影响任何请求形状 | `src-tauri/crates/infra/agent/src/compose.rs` -> `src-tauri/crates/infra/agent/src/compose_test.rs`；`src-tauri/crates/infra/agent/src/sdk/runner.rs` -> `src-tauri/crates/infra/agent/src/sdk/runner_test.rs`；`src-tauri/crates/infra/agent/src/model_catalog.rs` -> `src-tauri/crates/infra/agent/src/model_catalog_test.rs` |
| AC-9 | 前端控件：三档模型与两档 effort 均为 `StandardCombobox`（自由文本 + 建议清单）；其 colocated 测试查询只用 role / 文本 / `data-testid`；产品源码零因测改动 | `packages/desktop/src/components/standard/combobox.tsx` -> `packages/desktop/src/components/standard/combobox.test.tsx`；`packages/desktop/src/views/agents/components/provider-panel.tsx` -> `packages/desktop/src/views/agents/components/provider-panel.test.tsx`；`packages/desktop/src/views/agents/agents-view.tsx` -> `packages/desktop/src/views/agents/agents-view.test.tsx`；原语层 `packages/desktop/src/components/ui/combobox.tsx`（不可测试项 5，经标准层真实组合间接覆盖） |
| AC-10 | 预填四条纪律逐条可考：打开编辑态不自动预填、清空即回未配置、元数据缺席不预填、`[1m]` 条目窗长与后缀语义一致且「后缀优先」提示仍在；openai 协议不预填 `maxTokens` | `packages/desktop/src/views/agents/components/provider-panel.tsx` -> `packages/desktop/src/views/agents/components/provider-panel.test.tsx` |
| AC-11 | 文案：`context_length` / `max_tokens` 的表单标签为「上下文窗长（token）」/「输出上限（token）」，提示同步改述（不再裸写英文键名）；存储键 / IPC 参数名 / bindings 字段名零改动 | `packages/desktop/src/views/agents/components/provider-panel.tsx` -> `packages/desktop/src/views/agents/components/provider-panel.test.tsx` |
| AC-12 | 目录取数失败降级：`list_model_catalog` 抛错时表单照常可用（无建议、无阻断）、不轮询；提交载荷 `efforts` 恒携（空值以空串表达） | `packages/desktop/src/views/agents/hooks/use-model-catalog.ts` -> `packages/desktop/src/views/agents/hooks/use-model-catalog.test.ts`；`packages/desktop/src/views/agents/hooks/use-agent-providers.ts` -> `packages/desktop/src/views/agents/hooks/use-agent-providers.test.ts`；`packages/desktop/src/views/agents/agents-view.tsx` -> `packages/desktop/src/views/agents/agents-view.test.tsx` |
| AC-13 | 出线与契约：`bindings:export` 连跑两次零 diff；`api:check` 零 diff（含新元素 `Combobox` 生成物）；`cargo clippy` 零告警；`vp check` / `knip` / 全量测试全绿；`packages/desktop/package.json` 版本 `0.6.55` | 生成物 `src-tauri/src/types/generated/{bindings.ts,bindings.zod.ts,bindings.zod.check.ts}`（不可测试项 3）；`packages/desktop/package.json`（不可测试项 4）；`.dev-team/architecture/models/06-frontend.toml`（不可测试项 6）；`.dev-team/architecture/models/Frontend/**`、`graph/INDEX.toml`、`*.schema.json`、`*.i.ts`（不可测试项 7）；注册面 `src-tauri/src/commands/mod.rs` -> `src-tauri/src/bindings/mod_test.rs` |

> 表内 `源文件` -> `测试文件` 为 colocated 映射对（Rust 侧省略 `packages/desktop/` 前缀，与 `design.md` 路径口径一致）；「不可测试项 N」指向 `## 不可测试项` 的第 N 条——该文件不在单测面，其正确性由编译闭合面 / 静态检查链 / 既有测试承接。

## 测试框架与既有模式

| 侧 | 运行器 / 断言库 | 测试布局 | 本变更沿用的模式 |
|---|---|---|---|
| Rust（`src-tauri/**`） | `cargo` 内建 `#[test]` / `#[tokio::test]`，`assert!` / `assert_eq!` / `serde_json::json!` | colocated `*_test.rs` 与源文件同目录，由父模块 `#[cfg(test)] mod xxx_test;` 声明（`store/src/lib.rs:33`、`crates/infra/agent/src/lib.rs:260`、`sdk/mod.rs` 序列） | `native_model::encode/decode` 内存往返（`model_test` 版本链）、`tauri::test::mock_app()` + tempdir 真库（`commands/agents/mod_test`）、引擎日志行断言（`runner_test::pump_start_line`）、`MockScript` 假流缝捕获 `CompletionRequest`（`loop_test`） |
| 前端（`packages/desktop/src/**`） | `vite-plus/test`（`describe` / `it` / `expect` / `vi`）+ `@testing-library/react`（`render` / `screen` / `fireEvent` / `waitFor` / `findByRole`） | colocated `*.test.tsx` / `*.test.ts` 与源文件同目录（`components/standard/*.test.tsx`、`views/agents/**/*.test.ts(x)`） | 组件层零 mock（真实组合 `ui/**` 底座，查询只用 role / 文本 / `data-testid`）；hook 层以 `vi.mock('@tauri-apps/api/core')` 注入 `invoke` 替身 + `vi.mock('sonner')` 注入 `toast` 替身（进程边界） |

**范围外（既有断言承接，本变更零改动故不新增用例）**：

- `crates/infra/agent/src/sdk/compact.rs` 的摘要请求 `additional_params: None`——该文件进「不要修改」清单（出现 diff 即偏离决策），既有 `compact_test` 断言原样承接；
- `crates/core/agent/**` 事件词汇、`SessionConfigSnapshot` / `SessionRecord` 结构面——零改动，既有用例零回归即命题成立；
- `sdk/{normalize,context,resume,reasoning,policy,tools,model_spec,web_search}.rs`——effort 不触面，零改动。

## 单元测试

### src-tauri/crates/infra/store/src/model.rs -> src-tauri/crates/infra/store/src/model_test.rs

#### 待测功能

- `AgentProviderRecord::with_efforts()`: 构造器链式缝（`new` 签名不动的唯一实现路径），保存期无条件接受、无协议门控
- `AgentProviderRecord::new()`: 签名逐字不变（`efforts` 缺省 `AgentEffortTiers::default()`）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `with_efforts` | 正向 | `new(...).with_efforts({high:"max", low:"low"})` ⇒ 两档逐字段保真，其余字段（含 `web_search` / `context_length`）不受影响 | 单次 |
| `with_efforts` | 边界 | 两档全空串（`AgentEffortTiers::default()`）⇒ 记录与未经 `with_efforts` 的构造逐字段相等 | 单次 |
| `with_efforts` | 边界 | 超长字符串（>1000 chars）与特殊字符（内部 `\n`、emoji、引号）⇒ 原样承载（store 层零校验） | 参数化（3 组） |
| `with_efforts` | 正向 | 与 `with_web_search(true/false)` 两种次序组合 ⇒ `efforts` 与 `web_search` 两列互不覆盖 | 参数化（2 值 × 2 序） |
| `new` | 正向 | 七参构造 ⇒ `efforts == AgentEffortTiers::default()`（两档皆空串）、`id == 0` | 单次 |
| `new` | 边界 | 七参签名零改锚：`AgentProviderRecord::new` 以函数指针赋给七参类型后编译可达（44 处既有调用点零改的编译面证据） | 单次 |
| `AgentProviderRecord`（v5 编解码） | 正向 | `native_model::encode` / `decode` 往返 ⇒ `version == 5`、`efforts` 逐字段保真（空 / 双非空 / 单档非空） | 参数化（3 组） |
| `AgentProviderRecord` | 边界 | 版本头锚点：`native_model_version() == 5`、`native_model_id() == 5` | 单次 |
| `AgentProviderRecord` | 边界 | JSON 线格式：序列化含 `efforts: {high, low}`（camelCase）；缺 `efforts` 键反序列化 ⇒ 两档空（`#[serde(default)]` 只兜 JSON 面） | 参数化（2 形态） |
| `AgentProviderRecord` | 异常 | 非法 JSON 载荷（`efforts` 为字符串 / `efforts` 缺 `low` 子键）⇒ `Err`（不静默补值） | 参数化（2 形态） |
| `AgentEffortTiers` | 边界 | `Default` 两档空串；`PartialEq` 区分 high / low 交换后的两值 | 单次 |
| `From<AgentProviderRecordV4>` | 正向 | V4 全字段搬运 + `efforts` 补空（两档空串），其余字段逐一相等 | 单次 |
| `From<AgentProviderRecordV4>` | 正向 | v4 载荷字节（`AgentProviderRecordV4` 经 `native_model::encode`）经 `decode::<AgentProviderRecord>` ⇒ 升级成功、`efforts` 空、其余逐字段相等 | 单次 |
| `From<AgentProviderRecordV4>` | 正向 | v1→v2→v3→v4→v5 四级链产物与「直接构造 v5」逐字段一致 | 单次 |
| `From<AgentProviderRecord> for AgentProviderRecordV4` | 边界 | V5→V4 丢 `efforts`、其余保真；再升回 V5 ⇒ `efforts` 空（降级不作数据承诺） | 单次 |
| `AgentProviderRecordV4` | 边界 | 中转形态版本头 `native_model_version() == 4`，字段面不含 `efforts` | 单次 |
| 手写遮蔽 `Debug`（v5） | 正向 | `efforts` 位在场（值可读），`api_key` 位仍为 `sk-***890` 遮蔽形态且明文全文不出现 | 单次 |
| `AgentModelTiers` | 边界 | 结构零改动回归：三档（含 `medium`）逐字段保真往返，`efforts` 为 sibling 列不嵌入 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock） | `native_model::{encode,decode}` 与 `serde_json` 全内存，不触 DB 文件；v4 载荷由真实 `AgentProviderRecordV4` 结构体编码产出（不手搓字节） | 全部升级 / 降级链、线格式与 Debug 用例 |

### src-tauri/crates/infra/agent/src/sdk/effort.rs -> src-tauri/crates/infra/agent/src/sdk/effort_test.rs

#### 待测功能

- `effort::params()`: 两侧空白裁剪后空 ⇒ `None`；`Openai → {"reasoning_effort": v}`；`Anthropic → {"output_config": {"effort": v}}`；零值域校验、零 IO

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `params` | 正向 | `Openai` + `"high"` ⇒ 恰 `{"reasoning_effort":"high"}`，顶层对象键数 == 1（无多余键） | 单次 |
| `params` | 正向 | `Anthropic` + `"high"` ⇒ 恰 `{"output_config":{"effort":"high"}}`，外层与内层键数各 == 1 | 单次 |
| `params` | 边界 | `effort = ""` ⇒ `None`（两协议逐臂） | 参数化（2 协议） |
| `params` | 边界 | 纯空白 `"   "` / `"\t\n"` / `"\u{3000}"`（全角空格）⇒ `None`（两协议逐臂） | 参数化（3 形态 × 2 协议） |
| `params` | 边界 | 两侧空白 `"  low  "` / `"\tmax\n"` ⇒ 载荷值为 `"low"` / `"max"`（调用方零预处理） | 参数化（2 协议） |
| `params` | 边界 | 超长值（>1000 chars）原样透传（不截断） | 单次 |
| `params` | 边界 | 特殊字符（内部 `\n`、emoji、中间空格 `"a b"`）原样透传（只裁两侧） | 参数化（3 组） |
| `params` | 异常 | 目录外取值 `"ultra-thinking"` / `"none"` / `"MAX"` 原样透传（零值域校验、零大小写归一） | 参数化（3 值） |
| 常量面 | 正向 | 输出键名逐字为 `reasoning_effort` / `output_config` / `effort`（键名单点，值与常量逐字对齐） | 参数化（2 协议） |
| 纯函数性 | 边界 | 同一入参连续两次调用结果相等（无状态、零 IO、零全局） | 单次 |

> Enum 维度说明：`Protocol` 为两变体封闭枚举（Rust 无法构造非法变体），故以「两臂穷举 + 每臂正 / 反 / 边界三类」承担 N+1 的等价覆盖。

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock） | 纯函数零 IO、零环境依赖；直接以 `Protocol` 变体与字符串入参驱动 | 全部用例 |

### src-tauri/crates/infra/agent/src/model_catalog.rs -> src-tauri/crates/infra/agent/src/model_catalog_test.rs

#### 待测功能

- `catalog()`: 纯静态表读取（零 IO、零参数、零凭据），返回两级目录

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `catalog` | 正向 | `protocols` 非空，且 `openai` 与 `anthropic` 两协议皆在场（不变量①前段） | 单次 |
| `catalog` | 正向 | 每条协议 `models` 非空、协议级 `efforts` 非空（建议值域非空；不变量①后段） | 参数化（逐协议） |
| `catalog` | 正向 | 同协议内 `value` 无重复（去重后长度 == 原长度，不变量①） | 参数化（逐协议） |
| `catalog` | 边界 | `value` 含 `[1m]` 后缀的条目 ⇒ `context_length == Some(1_000_000)`（不变量④；筛选集非空由首版内容保证，筛选为空即用例前置失败） | 参数化（逐命中条目） |
| `catalog` | 边界 | `supports_thinking == false` 的条目 ⇒ `efforts == Some(vec![])`（显式空，不靠继承协议级；不变量⑤） | 参数化（逐命中条目） |
| `catalog` | 边界 | `supports_thinking == true` 且 `efforts == Some(非空)` 的条目 ⇒ 模型级覆盖协议级；`efforts == None` ⇒ 语义为继承协议级（不变量②语义面） | 参数化（逐命中条目） |
| `catalog` | 异常 | 零凭据 / 零端点：整目录 `serde_json::to_value` 全文不含 `api_key` / `apiKey` / `base_url` / `baseUrl` / `sk-`（不变量③） | 单次 |
| `catalog` | 边界 | 每条条目 `context_length` / `max_tokens` 为 `None` 或正数（不落 `0` 字面值） | 参数化（逐条目） |
| `catalog` | 边界 | 每次调用返回新构造对象：两次调用值相等（`PartialEq`）且均成功（无 `OnceLock` 共享可变态、无锁） | 单次 |
| `catalog` | 正向 | `protocol` 复用 store 线值枚举：序列化后逐字为 `openai` / `anthropic`（零平行枚举） | 参数化（逐协议） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock） | 纯编译期字面量，零 IO、零 DB、零网络；断言只读返回值 | 全部用例 |

### src-tauri/crates/infra/agent/src/lib.rs -> src-tauri/crates/infra/agent/src/lib_test.rs

#### 待测功能

- `EngineFacade::with_efforts()`: 门面旁路第四项（**未 shaping 的 provider 原值对**），门面零协议分支

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `with_efforts` | 正向 | `EngineFacade::new().with_efforts({high:"max", low:"low"})` ⇒ `facade.efforts` 逐字段相等 | 单次 |
| `with_efforts` | 边界 | 未调用 builder（缺省）⇒ `facade.efforts == AgentEffortTiers::default()`（两档空），与既有旁路缺省口径同型 | 单次 |
| `with_efforts` | 边界 | 与 `with_context_window` / `with_max_tokens` / `with_web_search` 四载荷两种次序组合 ⇒ 各列互不覆盖（四载荷共存） | 参数化（2 序） |
| `with_efforts` | 正向 | 门面 `runner_for(Sdk)` 与直构 `SdkRunner::new(...).with_efforts(同值)` 对拍：`open_continue` 错误面逐字一致、`New` 会话 open 均成功 | 单次 |
| `with_efforts` | 正向 | 两协议（`openai` / `anthropic`）× 非空 `efforts` ⇒ `runner_for` 均成功且载荷原样抵达（门面零协议门控、零改写） | 参数化（2 协议） |
| `with_efforts` | 边界 | 两档皆空 ⇒ `New` 会话 open 成功（effort 空是合法态，不参与配置齐备校验、不触发 `ConfigMissing`） | 单次 |
| `with_efforts` | 异常 | `EngineKind::Cli` 臂在携 `efforts` 时仍逐字 `Err("CLI 租户已下线")`（新载荷不改变退役面） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock） | 真实 `EngineFacade` + 真实 `SdkRunner` 组装；`base_url` 指向拒连端点，只在启动校验 / 构造面断言，不触网 | 全部用例 |
| 装载缝 `ResumeTranscript` | 既有闭包替身（`counting_loader`，作为被测门面 API 的显式入参注入） | `open_continue` 对拍用例 |

### src-tauri/crates/infra/agent/src/compose.rs -> src-tauri/crates/infra/agent/src/compose_test.rs

#### 待测功能

<!-- design.md 未在「公共函数 / API」子表声明本文件的公共 API 变更（`ResolvedEngine` 与 `resolve_agent_engine` 均为 crate 私有）；下列条目与用例取自 design「修改文件」表 `compose.rs` 行与「测试文件」表为 `compose_test.rs` 登记的原样带出 / 零回归义务（AC-6 / AC-8），不另立 design 未声明的导出面。 -->

- `ResolvedEngine.efforts`: 增 `store::AgentEffortTiers` 字段——provider 原值对原样带出、零协议门控（`resolve_agent_engine` 的 Sdk 臂，位置紧随 `max_tokens`）
- `compose_turn()`: 门面链在 `.with_web_search(..)` 之后追加 `.with_efforts(resolved.efforts)`，`runner_for` 唯一 match 点与 `web_search` 门控判定行零改动

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ResolvedEngine.efforts` | 正向 | 落库 provider `{high:"max", low:"low"}` + `protocol = openai` ⇒ `resolved.efforts` 逐字段相等 | 单次 |
| `ResolvedEngine.efforts` | 正向 | 同记录 `protocol = anthropic` ⇒ 同样原样带出（零门控：两协议结果同形，与 `web_search` 的门控语义形成对照） | 参数化（2 协议） |
| `ResolvedEngine.efforts` | 边界 | 两档皆空 ⇒ `resolved.efforts` 两档空串（不落缺省字面值、不落 `None` 二态） | 单次 |
| `ResolvedEngine.efforts` | 边界 | 目录外自定义值 ⇒ 原样带出（组合根零校验） | 参数化（2 值） |
| `compose_turn` 门面链 | 正向 | `compose_turn` 产物构造 Success：`efforts` 非空时 `New` 会话 open 成功（`.with_efforts(..)` 链接入不塌 `runner_for`） | 单次 |
| 取值链零回归 | 边界 | `efforts` 三态（空 / 单档 / 双档）逐一驱动：`resolved.context_window` / `resolved.max_tokens` / `resolved.config` 与无 efforts 情形逐字一致（AC-8：目录与 efforts 均不进解析期取值链） | 参数化（3 组） |
| 门控正交 | 正向 | `web_search ∈ {false,true}` × `efforts` 非空 ⇒ `resolved.web_search` 仍按 `provider.web_search && protocol == Anthropic` 单点判定，不受 efforts 影响 | 参数化（2 组） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock） | `WorkspaceStores` 打开 tempdir 真库（`OnceLock` 缓存沿用既有装置）；provider / agent 记录真实落库后经 `resolve_agent_engine` 解析 | 全部用例 |
| 时钟 / 目录 | 不 mock：tempdir 提供真实 workspace 根与数据根 | 落库与解析装置 |

### src-tauri/crates/infra/agent/src/sdk/runner.rs -> src-tauri/crates/infra/agent/src/sdk/runner_test.rs

#### 待测功能

- `SdkRunner::with_efforts()`: 旁路 builder（`new` 签名不动、`..self` 形态）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `with_efforts` | 正向 | `new(四参).with_efforts({high:"max", low:"low"})` + `ModelLevel::High` ⇒ 泵启动日志行含 `effort=max` | 单次 |
| 档位分发 | 正向 | `High → efforts.high`、`Low → efforts.low`（`{high:"max", low:"low"}` 两档各自命中，与同档裸模型名同行成对） | 参数化（2 档） |
| 档位分发 | 边界 | 单档留空：`{high:"", low:"low"}` ⇒ High 行 `effort=-`、Low 行 `effort=low`（两档独立，无跨界冒用） | 单次 |
| 档位分发 | 边界 | 纯空白 `{high:"   ", low:"\t"}` ⇒ 两档日志均 `effort=-`（裁剪判据在 shaping 单点） | 单次 |
| `with_efforts` | 边界 | 裸四参 `new`（未装配）与显式 `.with_efforts(AgentEffortTiers::default())` ⇒ 两条日志除 `session=` 外逐列同形、均 `effort=-` | 单次 |
| 泵启动日志 | 正向 | `effort=` 与 `protocol=` / `model=` / `window=` / `max_tokens=` / `web_search=` / `raw=` 同行并列（与既有诊断列同列同型） | 单次 |
| 泵启动日志 | 边界 | 自定义值 / 超长值（>1000 chars）/ 特殊字符 ⇒ 日志呈原值（不 JSON 转义、不截断） | 参数化（3 组） |
| `open_session` 校验面 | 异常 | `efforts` 非空 + 缺 `api_key` ⇒ 仍 `ConfigMissing` 且消息三成因（api_key / base_url / model）逐字可辨，不含 effort | 单次 |
| 构造面 | 边界 | 四参 `SdkRunner::new` 以函数指针形态赋给四参类型后编译可达（既有调用点零改的编译面锚） | 单次 |

> AC-6 的「shaping 只在 `open_session` 一处」属源码结构命题：`loop` / 门面 / 组合根零协议判定与零键名字面量由静态检查相位的源码复核承担，不在本表造断言。

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 引擎日志目录（文件系统进程边界） | 不 mock：经 `crate::sdk::log_test::engine_log_dir()` 真实初始化，`pump_start_line` 以 `RunStarted.session_id` 回找本用例那一行 | 全部日志断言用例 |
| 模型流 | 不 mock 被测内部模块：`RunStarted` 先于请求发起，取回首事件后即 drop 会话（`base_url` 指向拒连端点，不触网） | 起泵装置 |

### src-tauri/crates/infra/agent/src/sdk/loop.rs -> src-tauri/crates/infra/agent/src/sdk/loop_test.rs

#### 待测功能

<!-- design.md 未在「公共函数 / API」子表声明本文件的公共 API 变更（`LoopTurn` 为 `pub(crate)` 类型，`run` 签名不动）；下列条目与用例取自 design「修改文件」表 `loop.rs` 行与「测试文件」表为 `loop_test.rs` 登记的请求形状义务（AC-4 / AC-5）与 spec 的三条不变量。 -->

- `LoopTurn.effort_params`: 增 `Option<serde_json::Value>` 字段——已按协议 shaping 的 wire 载荷，loop 侧零协议判定、零键名字面量
- `run()`: 请求构造位 `additional_params` 由恒 `None` 改为 `turn.effort_params.clone()`，`provider_tool(..)` 挂载代码逐字不动（rig 按 `tools` 键合并）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `LoopTurn.effort_params` → `additional_params`（`run()` 请求构造位） | 正向 | `Some({"reasoning_effort":"max"})` ⇒ 请求 `additional_params` 深等于该对象（顶层键数 1） | 单次 |
| 同上 | 正向 | `Some({"output_config":{"effort":"max"}})` ⇒ 请求 `additional_params` 深等于该对象（嵌套键数各 1） | 单次 |
| 同上 | 边界 | `None` ⇒ `additional_params` 恒 `None`；且 `temperature` / `max_tokens` / `tool_choice` / `output_schema` / `documents` 与基线逐字一致（AC-4「留空零痕迹」） | 单次 |
| 同上 | 边界 | `Some(json!({}))`（空对象）⇒ `additional_params == Some({})`（loop 零判定：不做「空对象归一为 `None`」，归一职责在 shaping 单点） | 单次 |
| 与 `web_search` 共存 | 正向 | `effort_params = Some(anthropic 载荷)` + `web_search = true` ⇒ `additional_params` 同含 `output_config.effort` 与 `tools`，两键互不覆盖，`tools` 深等于既有三键一项 | 单次 |
| 与 `web_search` 共存 | 边界 | `effort_params = Some` + `web_search = false` ⇒ 仅 effort 键在场（`tools` 缺席） | 单次 |
| 与 `web_search` 共存 | 边界 | `effort_params = None` + `web_search = true` ⇒ 仅 `tools` 键在场（既有 web_search 用例口径不回归） | 单次 |
| 与 `web_search` 共存 | 边界 | 两种赋值次序（先 effort 后 tool / 先 tool 后 effort 的实现路径）⇒ 合并结果深等（次序无关，顶层键序不作判据） | 单次 |
| `LoopTurn.effort_params` | 正向 | 两轮会话：每轮请求均携同一载荷（turn 级 `clone`，跨轮不累积、不改写） | 参数化（2 轮） |
| `LoopTurn.effort_params` | 异常 | `web_search` 与 effort 同时缺席 ⇒ 请求 `additional_params` 恒 `None`（双空零痕迹的合成面） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 模型流（进程边界：外部 provider） | 既有装置 `MockScript` + 脚本化 transport（`FakeModel`）捕获 `CompletionRequest`；属测试替身而非被测模块 mock | 全部请求形状用例 |
| 工具 / 文件系统 | 不 mock：`LoopTurn` 直构（`loop_turn` 夹具 + 字段覆盖），cwd 为 tempdir | 全部用例 |

### src-tauri/src/commands/agents/mod.rs -> src-tauri/src/commands/agents/mod_test.rs

#### 待测功能

- `save_agent_provider()`: 末位第 10 参 `efforts`；两档各自 `trim()`（空串 = 留空）；零值域校验、零协议门控；既有校验逐字保留
- `list_model_catalog()`: 零参数、零 State、零 IO 薄包装（恒 `Ok`）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `save_agent_provider` | 正向 | 第 10 参 `{high:"  low  ", low:"ultra-thinking"}` ⇒ 回读 `{high:"low", low:"ultra-thinking"}`（只裁空白、零拒收） | 单次 |
| `save_agent_provider` | 边界 | 两档纯空白 `"   "` / `"\t"` ⇒ 回读两档空串（= 留空，不落 `"high"` 之类缺省字面） | 参数化（2 组） |
| `save_agent_provider` | 边界 | `AgentEffortTiers::default()`（两档空）⇒ 保存成功、回读空；与「显式留空」形态逐字段等价 | 单次 |
| `save_agent_provider` | 正向 | 两协议（`openai` / `anthropic`）× 非空 `efforts` ⇒ 均原样入库（零协议门控、零静默清零） | 参数化（2 协议） |
| `save_agent_provider` | 异常 | 目录外自定义值（不属任何建议清单）不被拒收，可存可回读 | 参数化（3 值） |
| `save_agent_provider` | 边界 | 超长值（>1000 chars）/ emoji / 内部换行 ⇒ 原样入库（只裁两侧） | 参数化（3 组） |
| `save_agent_provider` | 正向 | `id = Some(...)` 整行更新：`efforts` 按入参替换（不累积、不粘滞） | 参数化（2 组：置值 / 清空） |
| `save_agent_provider` | 异常 | 既有校验零回归：非法后缀 `[2m]` / `max_tokens = 4095` ⇒ `Err` 且库中记录不变；错误串全文不含 api_key 明文（含 `efforts` 非空时） | 参数化（2 组） |
| `save_agent_provider` | 边界 | 留空 api_key 回填与 `efforts` 正交：留空回填成功且 `efforts` 同时落库 | 单次 |
| `save_agent_provider` | 边界 | 位置参数逐位对齐：10 个具名实参调用（编译器保证位数），`web_search` 与 `efforts` 双向取值（true + 空 effort / false + 非空 effort）不串位 | 参数化（2 组） |
| `list_model_catalog` | 正向 | 零参数直调 ⇒ `Ok`，返回值 `protocols` 非空且含两协议 | 单次 |
| `list_model_catalog` | 正向 | 返回值与 `agent_runtime::catalog()` 逐字段相等（薄包装零加工，`Result` 恒 `Ok`） | 单次 |
| `list_model_catalog` | 边界 | 连续两次调用结果相等（零 IO、无缓存态、无副作用） | 单次 |
| `list_agent_providers`（零改动面） | 正向 | 保存后清单自动带出 `efforts`（新列随记录出线，删除 / 查询语义不变） | 参数化（2 组：空 / 非空） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| Tauri runtime（进程边界） | 不 mock 命令体：`#[tauri::command]` 保留原函数可直调，`tauri::test::mock_app()` + `app.manage(真实 WorkspaceStores)` 取 `State`（MockRuntime 无窗口无事件循环） | 全部命令用例 |
| 全局库（数据文件边界） | 不 mock：tempdir 真开 redb 全局库（`Env` RAII 自动清理） | 全部持久化用例 |

### src-tauri/src/commands/mod.rs -> src-tauri/src/bindings/mod_test.rs

#### 待测功能

<!-- design.md 未在「公共函数 / API」子表声明本文件的公共 API 变更（`all_commands!` 为宏登记面、无函数体）；下列条目与用例取自 design「修改文件」表 `src/commands/mod.rs` 行与「测试文件」表登记的「`list_model_catalog` 注册与出线形状」义务，落于既有 bindings 出线快照装置（`generate_handler` / `collect_commands` 同源，宏无 colocated 测试文件）。 -->

- `all_commands!()`: agents 轨道末行（`set_default_agent_instance` 之后）追加 `$crate::commands::agents::list_model_catalog,`——单一登记面，`generate_handler` / `collect_commands` 同源

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `all_commands!` 登记 | 正向 | 命令清单含 `list_model_catalog` 恰一次（单一登记面，不重不漏） | 单次 |
| 出线 DTO 类型名 | 正向 | DTO 类型清单含 `AgentEffortTiers` / `ModelCatalog` / `CatalogProtocol` / `CatalogModel` 四名 | 参数化（4 名） |
| 命令形参面 | 正向 | `bindings.ts` 中 `saveAgentProvider` 为十形参且末位为 `efforts`；`listModelCatalog` 零形参（载荷 `{}`） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock） | 读 `bindings:export` 产物文本与 `collect_commands` 清单快照（既有装置），零进程边界 | 全部用例 |

### packages/desktop/src/components/standard/combobox.tsx -> packages/desktop/src/components/standard/combobox.test.tsx

#### 待测功能

- `StandardCombobox()`: 可选可填单值控件（自由输入 + 建议清单；空串 = 留空；`items` 空退化纯输入框）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `StandardCombobox` | 正向 | `items = ["deepseek-reasoner","deepseek-chat"]`，逐字符键入 `my-custom-model`（目录外）⇒ `onChange` 依次收到键入原文、末次为完整串；输入框 `data-value` 等于当前原文 | 参数化（逐键） |
| `StandardCombobox` | 正向 | 展开清单（`findByRole('listbox')`）并选中 `deepseek-reasoner` ⇒ 输入框呈该原文、`onChange` 收到该值；再次展开清单项与 `items` 逐项一致 | 单次 |
| `StandardCombobox` | 边界 | 原值 `low` 后清空输入 ⇒ `onChange('')`、`data-value` 为空串（键在场而非缺席）、组件不报错、不回退到某项建议 | 单次 |
| `StandardCombobox` | 边界 | `items = []` ⇒ 聚焦 + 键入后文档中无 `listbox`、`onChange` 照常回调（退化纯输入框） | 单次 |
| `StandardCombobox` | 边界 | `disabled = true` ⇒ 输入框 disabled、聚焦不展开 `listbox`、`onChange` 零调用 | 单次 |
| `StandardCombobox` | 边界 | `value = ""` ⇒ `data-value = ""`；`placeholder` 缺省为「可输入或选择」 | 单次 |
| `StandardCombobox` | 边界 | `value` 为超长串（>1000 chars）/ 特殊字符（emoji、内部空格）/ 不在 `items` 内的自定义值 ⇒ 原样透出、零拦截 | 参数化（3 组） |
| `StandardCombobox` | 正向 | `id` / `className` / `placeholder` / `data-testid` 透传到**输入框元素**（非外层容器） | 单次 |
| `StandardCombobox` | 正向 | `items` 多于一项时逐项渲染为可选条目（数量与原文逐一对应，多余 / 缺项皆红） | 参数化（2 组规模） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock） | MUST NOT mock 被测组件或 `components/ui/combobox.tsx` 底座：base-ui 真实渲染；`onChange` 以 `vi.fn()` 作为被测 API 的显式入参注入；弹层以 `findByRole('listbox')` / `waitFor` 异步收敛，禁止类名查询 | 全部用例 |

### packages/desktop/src/views/agents/hooks/use-model-catalog.ts -> packages/desktop/src/views/agents/hooks/use-model-catalog.test.ts

#### 待测功能

- `useModelCatalog()`: 挂载一次取数、无轮询；失败降级为 `catalog = null`（不置 error、不 toast、不重试）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `useModelCatalog` | 正向 | 挂载 ⇒ `invoke('list_model_catalog')` 恰一次；`loading` 由 `true` 收敛为 `false`、`catalog` 回填（三类型逐字段：`protocols[].protocol/efforts/models[]`） | 单次 |
| `useModelCatalog` | 边界 | 无轮询：非重挂载的重新渲染 / 时间推进 ⇒ 调用计数不再增长 | 单次 |
| `useModelCatalog` | 异常 | `invoke` reject ⇒ `catalog === null`、`loading === false`，hook 不暴露 error 态、`toast.error` 零调用、无重试（调用数仍为 1） | 单次 |
| `useModelCatalog` | 边界 | 空目录（`protocols: []`）⇒ `catalog` 非 `null` 且 `protocols` 为空数组（「取到空目录」与「取数失败」两态可辨） | 单次 |
| `useModelCatalog` | 边界 | 卸载先于 resolve（`cancelled` 标志）⇒ 不 setState、无告警、`catalog` 保持 `null` | 单次 |
| `useModelCatalog` | 边界 | 条目元数据全缺席（`contextLength` / `maxTokens` / `efforts` 皆 `null`）⇒ 原样承载（hook 零归一化、零剔除） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（进程边界：IPC） | `vi.mock` 注入 `invoke` 替身（`vi.hoisted`），按命令名 `list_model_catalog` 分发 resolve / reject，并记录调用序列 | 全部用例 |
| `sonner`（进程边界：toast） | `vi.mock` 注入 `toast.error` 替身，断言零调用 | 降级用例 |

### packages/desktop/src/views/agents/hooks/use-agent-providers.ts -> packages/desktop/src/views/agents/hooks/use-agent-providers.test.ts

#### 待测功能

<!-- design.md 未在「公共函数 / API」子表声明本文件的公共 API 变更（`AgentProviderSaveInput` 为类型、`useAgentProviders` 签名不变）；下列条目与用例取自 design「修改文件」表 `use-agent-providers.ts` 行与「测试文件」表登记的「`efforts` 入参与位置参数逐位对齐」义务（AC-12 提交载荷恒携 efforts）。 -->

- `AgentProviderSaveInput.efforts`: 入参类型末位增 `efforts: AgentEffortTiers`，与 `save_agent_provider` 第 10 形参逐位对齐
- `save()`: `ipc.saveAgentProvider(...)` 九实参改十实参、末位追加 `input.efforts`；错误双轨语义（固定前缀「保存 provider 失败：」+ 返回 `null`）不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `AgentProviderSaveInput.efforts` | 正向 | `save({..., efforts:{high:'max', low:'low'}})` ⇒ `invoke('save_agent_provider')` 载荷 `efforts` 逐键相等，且位于 `webSearch` 之后（十形参逐位对齐） | 单次 |
| `AgentProviderSaveInput.efforts` | 边界 | 两档空串 ⇒ 载荷恒含 `efforts: {high:'', low:''}`（不省略、不落 `undefined` / `null`——省略会与「保持原值」混淆） | 参数化（2 组：双空 / 单空） |
| `AgentProviderSaveInput.efforts` | 边界 | 自定义值 / 带两侧空白值 ⇒ 原样透传（hook 零归一化，`trim` 归后端单点） | 参数化（3 组） |
| `AgentProviderSaveInput.efforts` | 正向 | `list_agent_providers` 回读含 `efforts` 的记录 ⇒ 两协议 × 空/非空 四组合原样承载 | 参数化（4 组） |
| `AgentProviderSaveInput.efforts` / `save()` | 异常 | `save` reject ⇒ 既有固定前缀「保存 provider 失败：」与返回 `null` 不变，`error` 态不置位（新形参不改错误双轨语义） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（进程边界：IPC） | `vi.mock` + 按命令名分发的可变库存替身（`stored` 数组），记录调用序列与载荷 | 全部用例 |
| `sonner`（进程边界：toast） | `vi.mock` 注入 `toast.error` 替身 | 错误双轨用例 |

### packages/desktop/src/views/agents/components/provider-panel.tsx -> packages/desktop/src/views/agents/components/provider-panel.test.tsx

#### 待测功能

<!-- design.md 未在「公共函数 / API」子表声明本文件的公共 API 变更（`ProviderFormState` / `ProviderComboKey` / `ProviderTextField` 为类型，`ProviderPanel` 签名仅增 `catalog` 入参）；下列条目与用例取自 design「修改文件」表 `provider-panel.tsx` 行、DD9 / DD11 / DD12 / DD13 与「测试文件」表登记的 AC-9 / AC-10 / AC-11 / AC-12 前端义务。 -->

- `ProviderPanel()`: 增 `catalog` 入参；三档模型与两档 effort 改走 `StandardCombobox`、预填 +「来自目录」弱标记、两项标签与提示中文化、清单行呈 `effort high/low`
- `toFormState()`: `ProviderFormState` 增 `effortsHigh` / `effortsLow`；打开编辑态 `prefilled` 恒空集（预填纪律 1 的落点）
- `protocolCatalog()`: 按协议取目录条目（协议缺席 ⇒ 空，全部建议退化为无建议）
- `catalogEntry()`: 按 `(协议, 模型原文)` 取目录条目（目录外自定义值 ⇒ `null`）
- `modelSuggestions()`: 协议级模型建议清单（`value` 原文清单）
- `effortSuggestions()`: 模型级 `efforts` 覆盖协议级建议（`Some([])` = 显式无建议，不回落）
- `prefillOf()`: 条目元数据 → 预填值（`[1m]` 条目窗长 1M；openai 协议不预填 `maxTokens`；元数据缺席不预填）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 表单控件形态（`ProviderPanel`） | 正向 | 新建态五个组合框（`provider-model-high` / `provider-model-medium` / `provider-model-low` / `provider-effort-high` / `provider-effort-low`）均在文档且为输入框形态（`data-value` 在场），文本字段表只剩五个 Input（name / baseUrl / apiKey / contextLength / maxTokens） | 单次 |
| 建议联动（`modelSuggestions`） | 正向 | `protocol = anthropic` + `catalog` 在场 ⇒ `provider-model-high` 建议项等于该协议目录条目 `value` 清单 | 单次 |
| 建议联动（`effortSuggestions` 模型级覆盖） | 正向 | 选中某模型条目 ⇒ `provider-effort-high` 建议项切换为该条目模型级 `efforts`（覆盖协议级） | 单次 |
| 建议联动（`effortSuggestions` 显式空覆盖） | 边界 | 条目 `efforts = []`（`supportsThinking = false`）⇒ effort 建议为空清单（不回落协议级） | 单次 |
| 建议联动（`catalogEntry` 目录外回落） | 边界 | 模型值为目录外自定义 ⇒ effort 建议回落协议级清单 | 单次 |
| 建议联动（`protocolCatalog` 协议切换） | 正向 | 协议切换 `openai ↔ anthropic` ⇒ 模型与 effort 建议项随协议切换（两协议条目不同） | 参数化（2 向） |
| 建议联动（`ProviderPanel` 降级） | 边界 | `catalog === null`（取数失败降级）⇒ 五控件退化纯输入框、文档中无 `listbox`、表单照常可提交 | 单次 |
| 预填纪律 1（`toFormState` 打开编辑态） | 边界 | 打开存量 `contextLength = null` / `maxTokens = null` 编辑态且不动任何控件 ⇒ 两框空、提交载荷 `contextLength: null` / `maxTokens: null`（零自动预填） | 单次 |
| 预填纪律 2（`prefillOf` 预填） | 正向 | 选中带 `contextLength = 200000` 的条目 ⇒ 窗长框预填 200000、弱标记「来自目录」在场；保存载荷 `contextLength: 200000` | 单次 |
| 预填纪律 2（清空回未配置） | 边界 | 预填后把该框清空再保存 ⇒ 载荷 `contextLength: null`（回未配置，非 0）、弱标记撤除 | 单次 |
| 预填纪律 3（`prefillOf` 元数据缺席） | 边界 | 条目元数据缺席（`contextLength` / `maxTokens` 皆 `null`）⇒ 两框不预填、无弱标记 | 单次 |
| 预填纪律 3（`prefillOf` 协议边界） | 边界 | `openai` 协议 + 条目带 `maxTokens` ⇒「上下文窗长（token）」预填、「输出上限（token）」保持空（该协议恒不写上限） | 单次 |
| 预填纪律 4（`prefillOf` `[1m]` 一致） | 边界 | 选中 `value` 含 `[1m]` 的条目 ⇒ 窗长预填 `1000000` 且「后缀优先于上下文窗长」提示在场 | 单次 |
| 预填弱标记 | 边界 | 预填后手动编辑该字段 ⇒ 弱标记撤除（仅「预填后未动」态呈现） | 单次 |
| 文案（`context_length`） | 正向 | 标签为「上下文窗长（token）」，提示不含裸写英文键名 `context_length` | 单次 |
| 文案（`max_tokens`） | 正向 | 标签为「输出上限（token）」，提示不含裸写英文键名 `max_tokens`，且改述后仍含分档推导 / 32000 / ≥4096 / 仅 anthropic 生效语义 | 单次 |
| 文案（后缀提示） | 边界 | 改述后的后缀提示仍在：`后缀优先于上下文窗长`（原「后缀优先于 context_length」随文案改述） | 单次 |
| 提交载荷 | 正向 | 提交载荷含 `efforts: {high, low}`（空值以空串表达，不省略） | 参数化（2 组：空 / 非空） |
| 清单行 | 正向 | 记录 `efforts` 非空 ⇒ 行文本呈 `effort high … / low …` | 单次 |
| 清单行 | 边界 | 记录 `efforts` 两档皆空 ⇒ 行内零 effort 标签（不呈空值占位） | 单次 |
| 既有行为回归 | 边界 | 组合框化后既有断言仍绿：协议切换 `base_url` 提示联动、`web_search` 仅 anthropic 渲染、切协议清零勾选、`max_tokens = 4095` 拒绝提交 | 参数化（4 组） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock 组件与底座） | `state: AgentProvidersState` 以 `fakeState`（`save` / `remove` 为 `vi.fn()`）作为被测 API 显式入参注入；`catalog` 以 props 注入真实形状 fixture（含 `null` 态） | 全部用例 |
| 取数 hook | 不组合：面板只消费 props（取数在 `agents-view`，其装配用例覆盖），保持本文件为纯表单契约 | 全部用例 |

### packages/desktop/src/views/agents/agents-view.tsx -> packages/desktop/src/views/agents/agents-view.test.tsx

#### 待测功能

<!-- design.md 未在「公共函数 / API」子表声明本文件的公共 API 变更（`AgentsView` 无 props，仅装配 `useModelCatalog()` 并下传）；下列条目与用例取自 design「修改文件」表 `agents-view.tsx` 行，锚定 AC-12「目录取数失败降级」与 AC-9 的装配面。 -->

- `AgentsView()`: 在视图根装配 `const catalogState = useModelCatalog();`（与 `providersState` 同址）并把 `catalog` 下传 `ProviderPanel`；面板保持无取数职责

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 目录取数装配（`AgentsView`） | 正向 | 挂载视图 ⇒ `invoke` 面为 `list_agent_providers` / `list_agent_instances` / `list_model_catalog` 各恰一次（既有「仅两命令」断言随本变更扩为三命令） | 单次 |
| 目录下传 | 正向 | 目录成功 ⇒ 打开 provider 表单后 `provider-model-high` 建议项在场（目录确已从视图下传） | 单次 |
| 目录降级 | 异常 | `list_model_catalog` reject ⇒ 视图照常渲染、provider 清单正常、表单可提交（无建议项）、无阻断 error、`toast.error` 零调用 | 单次 |
| 目录降级 | 边界 | 取数仍在途（`loading`）⇒ 表单组合框已在场且可键入（loading 不阻断表单） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（进程边界：IPC） | `vi.mock` + 按命令名分发的替身（三命令三分支，可单独切 reject），记录调用序列 | 全部用例 |
| `sonner`（进程边界：toast） | `vi.mock` 注入 `toast.error` 替身，断言降级路径零调用 | 降级用例 |

## 不可测试项

1. **`src-tauri/crates/infra/store/src/lib.rs`**：变更内容只有 `pub use model::{…, AgentEffortTiers, …}` 一行出口面（无公共函数、无行为）。出口面是否成立由编译器与跨 crate 消费者用例机械判定（`compose_test` / `commands/agents/mod_test` 取到类型即证），为其单造断言只能重复 `use` 语句。该文件同时是 `mod model_test;` 的声明处。
2. **`src-tauri/crates/infra/agent/src/sdk/mod.rs`**：变更内容只有 `pub(crate) mod effort;` 与（与 sidecar 同批的）`#[cfg(test)] mod effort_test;` 两行挂载。挂载正确性由「`effort_test` 能被编译并运行」这一事实承担（声明与文件不同批即 `cargo check --all-targets` 红），无独立可断言行为。
3. **`src-tauri/src/types/generated/{bindings.ts,bindings.zod.ts,bindings.zod.check.ts}`**：`bindings:export` 生成物，非手改。其出线形状（`AgentEffortTiers` / provider `efforts` 键 / `saveAgentProvider` 第 10 形参 / 目录三类型 / `listModelCatalog`）由 `src/bindings/mod_test.rs` 的类型名与形参面快照断言 + `bindings:export` 连跑两次零 diff 机械闭环守住；对生成物再写测试只会与生成器重复。
4. **`packages/desktop/package.json`**：变更内容为版本号 `0.6.54` → `0.6.55`（版本纪律）。属静态检查链的机械项（`vp check` / 版本纪律复核），无测试语义。
5. **`packages/desktop/src/components/ui/combobox.tsx`**：原语层包装，**零业务语义**（无建议来源、无留空策略）；spec 明令「MUST NOT mock `src/components/ui/**` 底座」，其行为由 `standard/combobox.test.tsx` 的真实组合整体覆盖。同层先例 `components/ui/select.tsx` 同样无 colocated 测试（由 `standard/select.test.tsx` 覆盖）。单测此文件只能断言 base-ui 的 DOM 结构与样式，既违「不以类名查询」纪律，也需要为可测性改动产品源码。
6. **`.dev-team/architecture/models/06-frontend.toml`**：手写 archi 模型（一枚元素 `App.Frontend.Components.Standard.Combobox` + 两条关系 + 域 summary 补词），非代码行为。一致性由 `api:check`（零写入读盘比对）与生成物零 diff 门机械守护；新元素带来的 `docMissing` 是 warn 且永不纳管 `passed`（proposal R14）。
7. **`.dev-team/architecture/models/Frontend/**`、`.dev-team/architecture/graph/INDEX.toml`、`*.schema.json`、`*.i.ts`**：`api:export` 生成物（四类生成物勿手改）。手改已提交即零 diff 门失败，未提交会被下次生成静默覆盖；其正确性判据是 `api:check` 零 diff，不是测试断言。
