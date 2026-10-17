# 测试设计: auto-named-explore

> **变更**: auto-named-explore
> **日期**: 2026-10-17
> **基于**: proposal.md（验收标准 1-10）、design.md（新增/修改文件、公共函数 / API 表及 crate 内公共面下注）

## 测试框架与布局（既有模式盘点）

- **Rust（`cargo test --workspace --manifest-path src-tauri/Cargo.toml`）**：同 crate 兄弟 `<module>_test.rs` 文件（`mod.rs` 内 `#[cfg(test)] mod <name>_test;` 声明）；断言 `assert_eq!` / `assert!` / `expect`；异步用例 `#[tokio::test]`；装置惯例为 `tempfile` RAII 临时环境（store 层 `Env`/`StoresEnv`、命令层 `Env` + 真实文件树）、rig `MockScript` 假流缝模型（loop 直驱）、discard 端点（`127.0.0.1:9` 恒拒连，失败收敛装置）、`counting_loader` 三形态闭包替身（转录装载缝）。
- **前端（`vp test`，vite-plus/test 装载 vitest + jsdom）**：colocated `<name>.test.ts` / `.test.tsx`；断言 `expect`（vitest 风格）；组件用 `@testing-library/react`（`render` / `renderHook` / `fireEvent` / `waitFor`）；进程边界 mock 统一 `vi.mock('@tauri-apps/api/core')`（`invokeMock` 按命令名分发 + `ChannelMock` 可编程类）；`list` 等 props 注入依赖用可编程假件（`makeList`）。

---

## 验收范围

<!-- 逐条映射 proposal.md「验收标准」1-10：
  - `AC ID`：proposal 验收标准编号
  - `验收条件`：原文摘录
  - `被测文件或模块`：承载用例的测试文件路径（相对项目根；源文件 → 测试文件的映射见下方「单元测试」各 `### <源文件> -> <测试文件>` heading，零测试路径的交付面文件见「不可测试项」） -->

| AC ID | 验收条件 | 被测文件或模块 |
|--------|---------|----------|
| AC-1 | 新建探索只输需求：不出现 kebab 输名入口；建档后详情页自动发出首轮（prompt = stance 前导 + 需求），无手动发送动作 | `packages/desktop/src/views/explores/components/explore-create-dialog.test.tsx`、`packages/desktop/src/views/explores/explore-view.test.tsx`、`packages/desktop/src/views/explores/explore-detail-view.test.tsx`、`packages/desktop/src/views/explores/hooks/use-explore-session.test.ts`、`packages/desktop/src/lib/explore-stance.test.ts`、`packages/desktop/src-tauri/src/commands/explores/mod_test.rs` |
| AC-2 | 占位建档：记录 `name` 匹配 `draft-<毫秒>`、`title` 为 `<时间戳>-分析中`、`session_id` 预建回写；`openspec/explores/` 无占位名文件；同毫秒重复建档经单调兜底获得互异名 | `packages/desktop/src-tauri/crates/infra/store/src/store_test.rs`、`packages/desktop/src-tauri/src/commands/explores/mod_test.rs` |
| AC-3 | 原子定名：agent 调 `create_explore_note(name="api-retry", …)` 成功后，记录 `name=api-retry`（`id` / `session_id` 不变）、磁盘出现 `openspec/explores/api-retry.md`、返回路径可供同会话 Edit 续用；世上不存在占位名文件 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/explore_note_test.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop_test.rs`、`packages/desktop/src-tauri/crates/infra/store/src/store_test.rs` |
| AC-4 | 改名收敛：run 终态后 URL replace 至新名、清单刷新、预览读新名文件、title / workflow_type 回填照常触发；全程无 MissingRecord 闪现 | `packages/desktop/src/views/explores/explore-view.test.tsx`、`packages/desktop/src/views/explores/explore-detail-view.test.tsx` |
| AC-5 | 工具面硬约束：explore 会话 definitions 不含 `write`；构造幻觉 `write` 调用被成员校验拒绝（`tool_unavailable` + is_error ToolResult，run 不中断）；`RunStarted.tools` 报实际下发面（六工具 + `create_explore_note`） | `packages/desktop/src-tauri/crates/core/agent/src/runner_test.rs`、`packages/desktop/src-tauri/crates/core/agent/src/lib_test.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/lib_test.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner_test.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools_test.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop_test.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/compose_test.rs` |
| AC-6 | `create_explore_note` 拒绝面：非法 kebab / 目标名已存在 / 文件已存在（错误附路径）/ 已定名记录异名（错误附现有名与路径）均显式 `Err` 回灌且零半边残留（前置校验拦截）；Default 档拒绝（policy 写面归属） | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/explore_note_test.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/policy_test.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop_test.rs` |
| AC-7 | 非 explore 来源零变化：debug / change 会话 definitions 仍为七工具、`RunStarted.tools` 报七工具 | `packages/desktop/src-tauri/crates/infra/agent/src/compose_test.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools_test.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop_test.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner_test.rs` |
| AC-8 | 导入路径回归：`Some(name)` 建档行为与现状逐字段一致（title = name）；已定名记录续话 stance 为笔记定位形 | `packages/desktop/src-tauri/crates/infra/store/src/store_test.rs`、`packages/desktop/src-tauri/src/commands/explores/mod_test.rs`、`packages/desktop/src/views/explores/components/explore-create-dialog.test.tsx`、`packages/desktop/src/lib/explore-stance.test.ts`、`packages/desktop/src/views/explores/hooks/use-explore-session.test.ts` |
| AC-9 | promote 门禁：对 `draft-` 前缀记录点击「启动变更」得显式 `Err`（引导先完成定名），行内呈现 | `packages/desktop/src-tauri/src/commands/explores/mod_test.rs`、`packages/desktop/src/views/explores/explore-detail-view.test.tsx` |
| AC-10 | 静态检查与测试（`pnpm run -C packages/desktop check` + 三测试根）由桌面端执行通过；插件产物重建后 `claude-plugins/` / `cursor-plugins/` / `cursor-home-image/` 同步 | 无测试文件——桌面端静态执行面（check + 三 Rust 测试根 + vitest + 插件三面产物重建），交付面文件与生成物见「不可测试项」 |

---

## 单元测试

### packages/desktop/src-tauri/crates/core/agent/src/runner.rs -> packages/desktop/src-tauri/crates/core/agent/src/runner_test.rs

#### 待测功能

- SessionToolset: 会话级专用工具面受控枚举（首值 `Explore`，serde camelCase 线值）
- SessionInjections: 契约结构（`toolset` 位新增 + serde additive 挂起激活）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| SessionToolset 序列化线值 | 正向 | `Explore` 序列化为 `"explore"`，round-trip 等值 | 单次 |
| SessionToolset 反序列化 | 正向 | 线值 `"explore"` 反序列化为 `SessionToolset::Explore` | 单次 |
| SessionToolset 非法线值 | 异常 | `"Explore"`（大小写敏感）/ `"unknown"` / 空串反序列化 `Err` | 参数化迭代 |
| SessionInjections 全字段序列化 | 正向 | preamble / tools / toolset 三位齐备序列化，`toolset` 线值为 `"explore"` | 单次 |
| SessionInjections 旧 JSON additive | 边界 | 无 `toolset` 字段的旧线 JSON 反序列化 → `toolset == None`（缺省读兼容） | 单次 |
| SessionInjections 空对象 | 边界 | `{}` 反序列化 → 三位全 `None`（与 `default()` 等价） | 单次 |
| SessionInjections toolset null | 边界 | `"toolset": null` → 读出 `None` | 单次 |
| SessionInjections 子集组合 round-trip | 正向 | `tools` = 探索六名子集 + `toolset` = Explore 序列化后反序列化逐字段等值 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 | 纯 serde 单元，零进程边界依赖，真实组合 | 全部用例 |

### packages/desktop/src-tauri/crates/core/agent/src/lib.rs -> packages/desktop/src-tauri/crates/core/agent/src/lib_test.rs

<!-- 公共函数 / API 表未列该文件；条目自 design「修改文件」表声明映射（`pub use runner::{…}` 再导出清单增 `SessionToolset`，词汇本体驻 runner.rs 由上一章节承接；再导出破坏即编译失败）。 -->

#### 待测功能

- SessionToolset 再导出: `pub use` 清单增补的词汇出口——经 crate 根路径构造 `Explore` 变体并 `PartialEq` 等值锚定
- SessionInjections 出口回归: 既有再导出清单零丢失锚定（增补不挤掉既有词汇；缺省与非法线值语义经出口侧复验）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| SessionToolset 再导出可达 | 正向 | 经 crate 根路径 `agent::SessionToolset` 构造 `Explore` 变体并 `PartialEq` 等值断言（既有 lib_test 导出面锚定模式续用） | 单次 |
| 既有再导出面零回归 | 正向 | runner / session / port / kernel / state / event / usage 各域再导出词汇逐名可达（快照清单逐项锚定——`SessionInjections` 等既有出口不随清单增补丢失） | 逐项迭代 |
| toolset 非法线值经出口反序列化 | 异常 | crate 根路径 `from_str::<SessionInjections>` 喂 `{"toolset":"unknown"}` → serde `Err`（再导出面承载的失败语义不吞改，与 runner_test 非法线值臂同口径、不同进口） | 单次 |
| SessionInjections 缺省出口锚定 | 边界 | 经 crate 根路径构造 `SessionInjections::default()` → 三位全 `None`（additive 缺省读兼容的出口侧锚定） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 | 纯词汇出口与 serde 单元，零进程边界依赖，真实组合 | 全部用例 |

### packages/desktop/src-tauri/crates/infra/store/src/store.rs -> packages/desktop/src-tauri/crates/infra/store/src/store_test.rs

#### 待测功能

- EXPLORE_PLACEHOLDER_PREFIX: 占位名前缀常量（`"draft-"`，「未定名」判别口径单点）
- Store.create_explore_record: 签名改 `Option<&str>`（`None` 占位建档 / `Some` 既有显式名路径）
- Store.rename_explore_record: in-place 改名（新增消费方为引擎执行体，行为回归）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| EXPLORE_PLACEHOLDER_PREFIX 常量 | 正向 | 值为 `"draft-"`（与前端 `DRAFT_NAME_PREFIX` 双源互链锚点断言） | 单次 |
| create(None) 占位名 | 正向 | 返回 `name` 匹配 `^draft-\d+$` 且 kebab 天然合法（免二次校验口径） | 单次 |
| create(None) 临时标题 | 正向 | `title` 匹配 `^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}-分析中$`（UTC 秒级派生，与占位名同基值） | 单次 |
| create(None) session_id 预建回写 | 正向 | 返回记录 `session_id` 为 `Some(预建空会话 id)`，会话行同事务在库 | 单次 |
| create(None) 同毫秒撞名单调兜底 | 边界 | 同 root 连续两次 `create(None)` → 两名互异且均为 `draft-` 前缀（事务内 +1 兜底，不依赖时钟推进） | 单次 |
| create(None) 记录可寻址 | 正向 | 建档后 `list_explore_records` 含该记录、`find_explore_record` 按名命中 | 单次 |
| create(Some) 显式名回归 | 正向 | `Some("api-retry")` → `name` / `title` 均 `"api-retry"`、校验 / 查重 / 建档逐行为与现状一致（AC-8） | 单次 |
| create(Some) 名已存在 | 异常 | 预置同名记录 → `Err`（记录已存在文案） | 单次 |
| create(Some) 非法 kebab | 异常 | 大写 / 下划线 / 前导数字 / 空串 → `Err`（非法记录名文案） | 参数化迭代 |
| create(Some) 超长名 | 边界 | 129 字符合法 kebab → `Err`（长度上限 128） | 单次 |
| rename in-place 语义 | 正向 | `draft-` 记录改名至最终名 → `id` / `session_id` / `created_at` 不变，`name` 更新、`updated_at` 刷新 | 单次 |
| rename 目标名已存在 | 异常 | 预置目标名记录 → `Err` 且原记录零改动 | 单次 |
| rename 记录不存在 | 异常 | miss 名寻址 → `Err` | 单次 |
| rename 同名幂等臂 | 边界 | `new_name == name` → `Ok` 且记录不变 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| native_db workspace 库 + 文件系统 | `tempfile` RAII 临时环境（既有 `Env` / `StoresEnv` 装置），真实组合非 mock | 占位建档与改名全路径（store 层零磁盘操作，落盘断言归命令层） |

### packages/desktop/src-tauri/crates/infra/agent/src/lib.rs -> packages/desktop/src-tauri/crates/infra/agent/src/lib_test.rs

#### 待测功能

- EngineFacade.with_explore_note_store: 组合根 store 句柄注入 builder（与 `with_context_window` 同型链式）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| builder 链式 | 正向 | `with_explore_note_store(store)` 返回 `Self`，可与 `with_context_window` 续链 | 单次 |
| 句柄注入后引擎选择回归 | 正向 | 带 store 句柄的 facade `runner_for(Sdk)` 仍产出 Sdk 引擎（builder 不破坏既有选择面） | 单次 |
| 默认构造零句柄 | 边界 | `new()` / `with_resume_transcript` 路径行为不随新字段变化（回归） | 单次 |
| 空配置启动失败面回归 | 异常 | 携 store 句柄的 facade + `EngineConfig::empty()` → `begin` / `open_session` 仍以 `AgentStartError::ConfigMissing`（配置项未填）显式失败——新字段与 builder 不吞改既有失败语义 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| Store（builder 显式入参注入依赖） | 真实临时 native_db 库（`tempfile`）构造句柄注入 | builder 装配用例 |
| provider 端点 | 既有 discard 端点底座（`http://127.0.0.1:9/v1` 恒拒连） | `runner_for` 接线与空配置失败面回归（不触真实网络） |

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner_test.rs

<!-- 公共函数 / API 表未列该文件（`sdk` 为 `pub(crate)` 模块）；条目自 design「类型定义」表（`SdkRunner` 增 `explore_store` 私有字段、`new` 签名增第四参）与「修改文件」表声明映射；泵的 per-turn 铸造行为另经 `sdk/loop_test.rs` 直驱覆盖。 -->

#### 待测功能

- SdkRunner::new(): 构造签名增第四参 `explore_store: Option<Arc<Store>>`（store 句柄持有；既有三参调用面以 `None` 兼容）
- SdkRunner.open_session(): 句柄传入 `session_pump`，per-turn 铸造 `ExploreNoteContext`（`toolset == Explore` 且句柄在场才铸，Continue id 缺席则 `None`）并随 `LoopTurn` 携带 `injections` / `explore_note`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| SdkRunner::new 第四参构造 | 正向 | `SdkRunner::new(config, resume, context_window, Some(store句柄))` 构造成功、既有三参调用面以 `None` 兼容（构造 + 引擎选择面编译期锚定） | 单次 |
| explore 注入经 open_session 端到端（链路：open_session → 泵铸造 → loop 下发面） | 正向 | Continue 预建空会话 + `injections.toolset = Explore` + 句柄在场 → `RunStarted.tools` 报六工具 + `create_explore_note`（泵内 LoopTurn 铸造贯通的行为面证明，AC-5 接线半边；模型 IO 走 discard 端点，RunStarted 先于 IO 可观察） | 单次 |
| default 注入零专用上下文 | 边界 | `toolset = None`（debug 来源装配缺省）→ 轮内不铸 `ExploreNoteContext`、`RunStarted.tools` 报七名（非 explore 会话零变化，AC-7 接线半边） | 单次 |
| 空配置失败面回归 | 异常 | `EngineConfig::empty()` + 句柄在场 → `open_session` 仍 `Err(ConfigMissing)`（配置三件套校验先于泵铸造，新字段不破坏既有失败面） | 单次 |
| 转录装载失败回归 | 异常 | 句柄在场 + resume loader `Err`（库读取失败）/ `None`（会话不存在）→ `ConfigMissing`，失败语义不随新字段漂移（既有 `counting_loader` 替身驱动） | 参数化迭代 |
| Explore 会话 Continue id 缺席 | 边界 | `toolset = Explore` 但 Continue id 缺席（New 会话形态，生产不可达）→ 上下文铸 `None`、run 照常开跑不 panic（loop 侧「执行上下文未装配」臂的前置半边） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| provider 模型端点 | 既有 discard 端点底座（`EngineConfig.base_url = http://127.0.0.1:9/v1` 恒拒连；RunStarted 先于模型 IO 可观察） | open_session 端到端与 default 轮用例 |
| ResumeTranscript 装载缝（builder 显式注入依赖） | 既有 `counting_loader` 三形态闭包替身（Ok(events) / Err / None）+ 调用计数 | 转录装载失败回归用例 |
| Store 句柄（`new` 显式入参注入依赖） | 真实临时 native_db 库（`tempfile`）构造 `Arc<Store>` 注入，不 mock | 全部携句柄用例 |

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools_test.rs

#### 待测功能

- sdk::tools::definitions(injections): 定义面参数化组装（七通用工具保序过滤 + Explore 追加专用工具）
- sdk::tools::EXPLORE_NOTE_TOOL: 专用工具名常量（`"create_explore_note"`）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| EXPLORE_NOTE_TOOL 常量 | 正向 | 值为 `"create_explore_note"` | 单次 |
| definitions default 注入 | 正向 | `SessionInjections::default()` → 七定义与 `TOOL_NAMES` 同源同序（含 `write`，回归） | 单次 |
| definitions explore 收窄 | 正向 | `tools` = 六名 + `toolset` = Explore → 定义不含 `write`、序同 `TOOL_NAMES` 保序过滤、追加 `create_explore_note` | 单次 |
| definitions 空 tools | 边界 | `tools = Some([])` + `toolset` None → 零通用定义；`toolset` = Explore → 仅 `create_explore_note` | 参数化迭代 |
| definitions 单工具 | 边界 | `tools = Some(["read"])` → 单定义 `read` | 单次 |
| definitions 非法名过滤 | 异常 | `tools` 含 `"hacker"` / 重复名 → 定义面仅保 `TOOL_NAMES` 内成员（保序滤非法） | 参数化迭代 |
| create_explore_note 定义形状 | 正向 | 入参 `name` / `content`（无 `path` 字段）、description 非空且注明无路径参数、服务端按会话解析、返回 repo 相对路径 | 单次 |
| 七通用定义体逐字不变 | 正向 | 重构为 `generic_definition` 后各定义 name / description / schema 与既有内容逐字一致（回归快照断言） | 逐项迭代 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 | 纯组装函数，零进程边界依赖 | 全部用例 |

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/policy.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/policy_test.rs

<!-- 公共函数 / API 表未列该文件（决策表纯函数扩展、签名零变化）；条目自 design「修改文件」表声明映射。拒绝流出形态（`permission_denied` + is_error 回灌）由 `sdk/loop_test.rs` 分发链组合承接。 -->

#### 待测功能

- WRITE_TOOLS: 写面工具常量扩为三值 `["write", "edit", "create_explore_note"]`
- create_explore_note 档位判定: 归写面——Default 拒绝、AcceptEdits / BypassPermissions 放行（拒绝形态与 write / edit 同构，AC-6）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| create_explore_note 两写档放行 | 正向 | AcceptEdits / BypassPermissions 档 `allows(_, "create_explore_note")` = true（与 write / edit 同构写面语义） | 参数化迭代 |
| create_explore_note Default 档拒绝 | 异常 | Default 档 `allows(_, "create_explore_note")` = false（写面归属，AC-6；拒绝形态与 write / edit 同构，`permission_denied` 回灌归 loop_test 组合用例） | 单次 |
| 决策表封闭集逐格回归 | 边界 | 三档 ×（七通用工具 + `create_explore_note`）逐格断言与扩展后矩阵一致（write / edit / bash 档位行为零漂移）+ 未知名（空串、`Write` 大写变体、带空白变体）三档一律拒绝（工具面封闭清单口径） | 参数化迭代 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 | 纯三档决策表，零进程边界依赖，真实组合 | 全部用例 |

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop_test.rs

#### 待测功能

- LoopTurn.injections / LoopTurn.explore_note: 新增两字段（definitions 组装唯一源 + 专用工具执行上下文路由）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| LoopTurn 注入字段装配 | 正向 | `injections` 在场时 run 顶部 definitions 按其组装（`dispatched` 自 definitions 派生，单源口径） | 单次 |
| explore 轮 RunStarted.tools | 正向 | explore 注入轮 → `RunStarted.tools` = read/grep/glob/ls/edit/bash/create_explore_note（本轮实际下发面，AC-5） | 单次 |
| default 轮 RunStarted.tools | 正向 | default 注入轮 → 七名与 `TOOL_NAMES` 逐名一致（与上线前一致，AC-7） | 单次 |
| 幻觉 write 成员校验拒绝 | 异常 | explore 注入轮发起 `write` 调用 → `SystemNotice{tool_unavailable}` + is_error ToolResult 同 id 回灌、history 收对、run 不中断续轮正常、不触执行体（AC-5） | 单次 |
| 未知名拒绝形态迁移 | 异常 | default 轮未知名调用 → `tool_unavailable`（自 `permission_denied` 形态迁移的回归断言） | 单次 |
| 成员校验先于 policy | 边界 | 未知名 + Default 档 → `tool_unavailable` 而非 `permission_denied`（分发循环顶部次序断言） | 单次 |
| create 工具路由 | 正向 | explore 轮 `create_explore_note` 调用 + `explore_note` 上下文在场 + AcceptEdits → 路由 `ExploreNoteContext::execute`，ToolResult 承接返回路径、ToolUse/ToolResult 成对入史（AC-3 链路收口） | 单次 |
| create 上下文缺席 | 异常 | 上下文 `None`（生产不可达臂）→ is_error 回灌「执行上下文未装配」，run 不中断 | 单次 |
| create + Default 档 | 异常 | `create_explore_note` 调用 + Default 档 → `permission_denied`（policy 写面归属组合用例，链路方向 loop → policy，AC-6） | 单次 |
| create + BypassPermissions | 边界 | 放行且执行体承接（与 AcceptEdits 两写档对照） | 单次 |
| 探索面六工具收窄不误伤 | 正向 | explore 轮 `read` / `grep` / `glob` / `ls` / `edit` / `bash` 下发面内调用 → 正常执行 | 参数化迭代 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| rig 流模型（LLM provider IO 进程边界） | 既有 `MockScript` 假流缝 + `FakeState` 请求捕获装置（loop_test 既有模式） | 全部用例的工具轮脚本驱动 |
| ExploreNoteContext.store（`LoopTurn` 显式注入上下文依赖） | 真实临时 native_db 库 + `tempfile` 文件树（真实组合，不 mock） | create 工具路由用例（落盘与改名真实发生） |

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/explore_note.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/explore_note_test.rs

<!-- 新增源文件与新建测试文件（`sdk/mod.rs` 随动声明 `pub(crate) mod explore_note;` + `#[cfg(test)] mod explore_note_test;`，模块声明本体见「不可测试项」）。 -->

#### 待测功能

- ExploreNoteContext::new(store, root, session_id): per-turn 专用工具执行上下文铸造
- ExploreNoteContext::execute(&self, input): `create_explore_note` 执行体（占位两步定名 / 已定名补建与拦截 / 失败面恢复指引，async）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| ExploreNoteContext::new | 正向 | 持 store 句柄 / root / session_id 三元组构造，供 `execute` 消费（构造即用断言） | 单次 |
| 占位记录两步定名 | 正向 | `draft-` 记录 + 合法 `name="api-retry"` → ① `explores_root/api-retry.md` 落盘（content 原样）② 记录 in-place 改名（`id` / `session_id` 不变）、返回 repo 相对 `/` 分隔路径（AC-3） | 单次 |
| 零占位名残留 | 正向 | 定名成功后磁盘不存在任何 `draft-*` 笔记文件、`find_explore_record` 旧名 miss（世上无占位名文件） | 单次 |
| 路径分隔符归一 | 边界 | root 为 Windows 反斜杠绝对路径 → 返回值 `/` 分隔（`strip_prefix` + 归一，供 Edit 续用） | 单次 |
| 父目录缺失补建 | 边界 | `openspec/explores/` 目录不存在 → 补建父目录后落盘成功 | 单次 |
| content 边界落盘 | 边界 | 空串 / 超长（>1000 字符）/ 特殊字符（换行、引号、emoji、中文）→ 原样落盘不改写 | 参数化迭代 |
| 非法 kebab name | 异常 | 大写 / 下划线 / 前导数字 / 空串 → 显式 `Err` 零残留（文件与记录均无变化，前置校验先于落盘） | 参数化迭代 |
| name 超长 | 异常 | 129 字符 → `Err`（长度 ≤128 三处同口径） | 单次 |
| 目标名已有记录 | 异常 | 预置同名记录 → `Err` 零残留 | 单次 |
| 目标文件已存在 | 异常 | 预置 `explores_root/<name>.md`（会话外文件 / 残留态复现）→ `Err` 文案附路径（恢复通道） | 单次 |
| 已定名同名补建 | 正向 | 已定名记录 + `name == record.name` + 文件缺失（孤儿）→ 补建文件、记录不变 | 单次 |
| 已定名异名拦截 | 异常 | 已定名记录 + 异名 → `Err` 附现有名与笔记路径（错误文案即恢复通道，导入场景常态命中） | 单次 |
| 会话指针 miss | 异常 | `session_id` 无对应 explore 记录 → 显式 `Err`（按调用时圈定失败） | 单次 |
| input 形态拒绝面 | 异常 | 缺 `name` / 缺 `content` / 字段非字符串（`name` 为数字等）/ 非 object Value（string / array / null）→ 显式 `Err`（入参抽取失败文案） | 参数化迭代 |
| input 多余字段宽容面 | 边界 | 入参携多余字段（含幻觉 `path` 参数与嵌套对象字段）→ additive 宽容：仅取 `name` / `content` 两字段、不消费不报错、定名流程照常完成（与 `sdk/tools.rs` `string_field` 七通用工具同款 additive 口径，D6 定稿执行体为 `input.get` 式 Value 抽取） | 单次 |
| 目标名为 draft- 前缀 | 边界 | 合法 kebab 且 `draft-` 前缀目标名 → 按普通定名处理（改名后仍占位前缀，promote 门禁延续拦截——风险 7 同形留痕） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（真实组合） | native_db workspace 库 + 文件系统 + `foundation::layout` 探索目录派生均为 `tempfile` RAII 临时环境；`ExploreNoteContext` 为被测 API 显式构造入参，不 mock | 全部用例（`Arc<Store>` 具体类型、create-only 落盘与 in-place 改名真实发生） |

### packages/desktop/src-tauri/crates/infra/agent/src/compose.rs -> packages/desktop/src-tauri/crates/infra/agent/src/compose_test.rs

#### 待测功能

- compose_turn / ComposedTurn.begin: 签名不变；`begin` 内部按 `provenance.source` 装配注入（explore 六工具 + `Explore` 枚举，其余 default）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| explore 来源装配 | 正向 | `begin(source = "explore")` → `TurnRequest.injections.tools` = 六名（无 `write`、序同 `TOOL_NAMES`）+ `toolset = Explore` | 单次 |
| 六名子集校验等价断言 | 边界 | explore 装配产物逐名 ∈ `sdk::tools::TOOL_NAMES`（装配层校验的可达面收窄断言） | 逐项迭代 |
| debug 来源 default | 正向 | `source = "debug"` → injections = default（`None` / `None`）（AC-7） | 单次 |
| change 来源 default | 正向 | `source = "change-flow"` 等 change 系字面量 → default（AC-7） | 参数化迭代 |
| 空 / 未知来源 | 边界 | `source = ""` → default 臂（缺省命令层来源天然走 default） | 单次 |
| 三处字面量同源 | 正向 | 以 `store::EXPLORE_RUN_SOURCE` 字面量发起 → 命中 explore 装配臂（compose 与 store 跨 crate 双源断言） | 单次 |
| 装配产物真实驱动（组合：compose → pump → loop 下发面） | 正向 | `compose_turn` 带 store 句柄 + explore 来源 + Continue 预建空会话 → RunStarted.tools 报六工具 + `create_explore_note`（注入经泵铸造与下发链端到端，AC-5） | 单次 |
| 装配产物真实驱动 default（组合） | 正向 | debug 来源同样驱动 → RunStarted.tools 报七名（AC-7 端到端） | 单次 |
| 注入面不破坏失败语义 | 异常 | explore 装配 + discard 端点（模型 IO 恒拒连）→ `api_error` 失败收敛形态与既有一致（回归） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| WorkspaceStores / workspace 库 | 真实临时数据根（既有 `DualEnv` 装置 + provider / instance seed） | 全部用例 |
| provider 模型端点 | discard 端点底座（`127.0.0.1:9` 恒拒连；RunStarted 先于模型 IO 可观察，失败收敛为装置形态而非 mock） | 真实驱动与失败语义用例 |

### packages/desktop/src-tauri/src/commands/explores/mod.rs -> packages/desktop/src-tauri/src/commands/explores/mod_test.rs

#### 待测功能

- create_explore_record（IPC）: 命令签名 `name: Option<String>` 透传（`None` 占位建档 / `Some` 导入显式名）
- promote_explore（IPC）: 签名不变；`promote_explore_with` 增占位前缀门禁（先于笔记读取）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| create(None) 占位建档 | 正向 | 返回 `name` 匹配 `draft-<毫秒>`、`title` 为 `<yyyy-MM-dd HH:mm:ss>-分析中`、`session_id` 回写；`openspec/explores/`（layout 真实派生）下无占位名文件（AC-2） | 单次 |
| create(None) 重复建档互异 | 边界 | 连续两次 `create(None)` → 互异名（透传 store 单调兜底，AC-2） | 单次 |
| create(Some) 导入回归 | 正向 | `Some("api-retry")` → 逐字段与现状一致（`title = name`）（AC-8） | 单次 |
| create blank root | 异常 | 空 root → `Err` 照旧（既有校验回归） | 单次 |
| create(Some) 非法名透传 | 异常 | 非 kebab `Some` 名 → store `Err` 透传（薄包装零吞改） | 单次 |
| promote 占位门禁 | 异常 | `draft-` 前缀记录 promote → 显式 `Err` 引导先在会话中完成定名（AC-9） | 单次 |
| 门禁先于笔记读取 | 边界 | 占位记录且笔记文件不存在 → 命中占位门禁 `Err` 而非「笔记未落盘」`Err`（步序断言：workflow_type 校验 → 占位门禁 → 读笔记） | 单次 |
| 门禁不拦正常名 | 正向 | 非占位记录 promote → 既有搬运流程照旧（回归） | 单次 |
| promote 参数校验先行 | 异常 | 非法 workflow_type / blank root → 既有 `Err` 照旧（门禁位于参数校验之后，步序回归） | 参数化迭代 |
| 用户自建 draft- 前缀名 | 边界 | 显式导入 `draft-20260101…` 名记录 → promote 同被门禁拦截（proposal 风险 7 留痕口径） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| WorkspaceStores + workspace 文件树 | 真实临时数据根 + 真实 `openspec/explores/` 文件树（既有 `Env` 装置），不 mock | 全部用例 |
| tauri State | 既有测试 App 装置（`tauri` test app + Manager 取 state） | 命令调用装载 |

### packages/desktop/src/views/explores/components/explore-create-dialog.tsx -> packages/desktop/src/views/explores/components/explore-create-dialog.test.tsx

#### 待测功能

- ExploreCreateDialogProps.onCreated: 签名改 `(name: string, autoPrompt: string | null) => void`（建档成功回调携需求原文）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| TopicEntry 需求输入建档 | 正向 | textarea 输入自然语言需求（中文 / 空格 / 换行）→ 提交启用、`invoke('create_explore_record', { root, name: null })` 恰一次、`onCreated(占位名, requirement.trim())`（占位名取返回记录 `name`） | 单次 |
| 空白需求禁提交 | 边界 | 空串 / 全空白（空格、换行）→ 提交按钮禁用、零 invoke | 参数化迭代 |
| 超长需求 | 边界 | >1000 字符需求 → 正常提交、trim 后原样上抛不被截断 | 单次 |
| kebab 校验退役 | 边界 | 输入含大写 / 下划线 / 前导数字的文本 → 不出现 `explore-topic-invalid` 错误分支、提交仍可用（校验面退役回归） | 参数化迭代 |
| 建档失败呈现 | 异常 | invoke reject → 错误行呈现、`onCreated` 不调用 | 单次 |
| ImportEntry 显式名回归 | 正向 | 导入成功 → `onCreated(entry.name, null)`（显式名路径零语义变化，AC-8） | 单次 |
| 入口结构迁移 | 正向 | `explore-topic-requirement` testid 在场、`explore-topic-name` 不在场（需求输入替换单行输名的结构断言） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| invoke（IPC 进程边界） | `vi.mock('@tauri-apps/api/core')` + `invokeMock` 按命令名分发（`scan_explores` / `create_explore_record` 可编程，既有模式） | 全部用例（组件与内部状态真实组合） |

### packages/desktop/src/views/explores/explore-view.tsx -> packages/desktop/src/views/explores/explore-view.test.tsx

<!-- 公共函数 / API 表未列该文件（`onCreated` 携 `autoPrompt` 的签名变更归属 explore-create-dialog.tsx 行）；条目自 design「修改文件」表声明的两处组件内改动映射。本节兼承跨模块组合用例挂靠——链路发起方 = 清单页：AC-1 前半链（新建 → 导航携 autoPrompt）与 AC-4 派生三级链（id 桥兜底、无 MissingRecord 闪现）。 -->

#### 待测功能

- onCreated(created, autoPrompt): 新建回调处理体——`list.refresh()` 后 `navigate(/explores/<created>)`，`autoPrompt !== null` 时携 `state.autoPrompt`、`null` 时不携带 state
- lastRecordRef: 记录派生三级链快照缓存——URL 名 find miss 时按快照 id 在清单重寻、仍 miss 以快照本体兜底（派生非空时渲染期刷新维护）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| onCreated → navigate 携 autoPrompt（链路：清单页 → 详情页首轮） | 正向 | `onCreated(created, autoPrompt 非 null)` → `list.refresh()` 后 `navigate(/explores/<created>, { state: { autoPrompt } })`（AC-1） | 单次 |
| onCreated 无 autoPrompt（链路：导入路径） | 边界 | `autoPrompt === null` → navigate 不携带 state（AC-8 回归） | 单次 |
| 三级链 id 重寻（链路：URL 旧名 → 清单新名） | 正向 | URL 名 miss（记录已改名）+ 清单含新名记录 → 按 `lastRecordRef` 快照 id 重寻命中、渲染新名详情（AC-4） | 单次 |
| 快照本体兜底 | 边界 | URL 名与清单双 miss → 以快照本体兜底渲染、不闪现 MissingRecord（MUST NOT 断言） | 单次 |
| 刷新间隙旧名 miss | 异常 | `list.refresh()` 尚未返回新名清单的窗口 → 快照兜底维持渲染、全程无 MissingRecord 闪现（AC-4） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| invoke + Channel（IPC 进程边界） | `vi.mock('@tauri-apps/api/core')`（`invokeMock` 按命令名分发 + `ChannelMock` 可编程类，既有模式） | 全部用例（页面内部 hooks 真实组合，fixture 经 mock IPC 流入） |

### packages/desktop/src/views/explores/explore-detail-view.tsx -> packages/desktop/src/views/explores/explore-detail-view.test.tsx

<!-- 公共函数 / API 表未列该文件（`useExploreSession` 签名扩展归属其自身行）；条目自 design「修改文件」表声明的三处组件内改动映射。本节兼承跨模块组合用例挂靠——链路发起方 = 详情页：AC-1 后半链（autoPrompt 读数 → auto-send 入口）、AC-4 收敛链（run 终态刷新 + 名变 navigate replace）、AC-9 门禁 Err 行内呈现。 -->

#### 待测功能

- autoPrompt 读数: `useLocation().state` 宽容解析（非 string 一律 null）传入 `useExploreSession` 第三参
- list.refresh(): run 终态（`running` true→false 既有 effect）增补清单刷新，与既有 `doc.refresh()` 同点
- navigate()（名变收敛 effect）: `record.name !== routeName`（`useParams<'name'>()` 名与派生记录名分叉）→ `navigate(/explores/<新名>, { replace: true })`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| autoPrompt 读数传递（链路：location.state → useExploreSession） | 正向 | `location.state.autoPrompt` 为 string → `useExploreSession` 第三参收到该值（AC-1 链路入口半边） | 单次 |
| state 宽容解析 | 边界 | `state.autoPrompt` 为对象 / 数字 / undefined / null → 一律传 `null`（非 string 宽容口径） | 参数化迭代 |
| run 终态清单刷新（链路：详情页 → 清单） | 正向 | `running` true→false 既有 effect → `list.refresh()` 与既有 `doc.refresh()` 同点触发（AC-4） | 单次 |
| 名变收敛导航（链路：id 桥 → URL replace） | 正向 | 派生 `record.name !== routeName`（清单重寻得新名）→ `navigate(/explores/<新名>, { replace: true })` 且不携带 state（AC-4） | 单次 |
| 名变后预览重订阅 | 正向 | navigate replace 后 `useExploreDoc` 以新名读取（`read_explore` / `explore_doc_path` 入参为新名，name 依赖重订阅既有机制） | 单次 |
| 稳态零导航 | 边界 | `record.name === routeName` → 零 navigate（名变 effect 不误触发回归） | 单次 |
| 中间窗口停留占位名 | 边界 | create 已执行、run 未终态 → 页面停留占位名、标题「分析中」、预览空态、无 MissingRecord（proposal 风险 5 留痕口径） | 单次 |
| promote 门禁行内呈现（链路：详情页 → 命令 Err） | 异常 | `list.promote` reject（占位门禁文案）→ 行内错误呈现、不跳转（AC-9） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| invoke + Channel（IPC 进程边界） | `vi.mock('@tauri-apps/api/core')`（`invokeMock` + `ChannelMock`，既有模式） | 全部用例（`useExploreDoc` / `useExploreSession` 真实组合） |
| `list`（props 显式注入依赖） | 可编程假件（既有 `makeList`：`refresh` / `promote` 等动作可编程 resolve / reject） | 终态刷新与门禁呈现用例 |

### packages/desktop/src/views/explores/hooks/use-explore-session.ts -> packages/desktop/src/views/explores/hooks/use-explore-session.test.ts

#### 待测功能

- useExploreSession(root, record, autoPrompt): 增第三参与 auto-send 闸门（四条件 + per-mount 一次性 ref）；既有返回面零变化

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| auto-send 闸门齐备触发 | 正向 | `autoPrompt` 在场 + `!session.loading` + `chain.length === 0` + record / sessionId 就绪 → `agent_start` 恰一次、prompt = stance 前导 + autoPrompt 原文（AC-1） | 单次 |
| auto-send 档位口径 | 正向 | auto-send 的 `agent_start` 入参 `permissionMode = 'bypassPermissions'`（与 composer 缺省档同源常量） | 单次 |
| autoPrompt null 缺省不发 | 正向 | 未传第三参 / 传 null → 零自动发送（既有行为回归） | 参数化迭代 |
| chain > 0 拒发 | 边界 | 重放装载完成后 `chain` 非空 + location state 复现 → 不重发（back/forward 闸门） | 单次 |
| loading 在途守卫 | 边界 | 装载期 `autoPrompt` 在场 → 装载完成后才触发（不早发、不漏发） | 单次 |
| record / sessionId 未就绪 | 边界 | `record = null` / `sessionId` 空缺 → 不自动发送 | 参数化迭代 |
| 一次性消费 ref | 边界 | 同 mount 内 effect 重复执行（StrictMode 双调用形态）→ `send` 恰一次 | 单次 |
| per-mount 重武装 | 边界 | unmount 后携带 state 重挂载 → ref 重新武装、可再次 auto-send（免费重试语义） | 单次 |
| 首轮失败复现重试 | 边界 | 首轮 send 失败（`chain` 仍 0）+ state 复现重挂载 → 恰构成一次重发（proposal 风险 6 留痕口径） | 单次 |
| autoPrompt 空串 | 边界 | `''`（非 null）→ 按在场口径发首轮（与 null 的区分断言） | 单次 |
| 占位记录 stance 分形 | 正向 | `record.name` 为 `draft-` 前缀 → send prompt 为命名指令形（不含任何文件路径插值，AC-1 / D11） | 单次 |
| 已定名记录 stance 分形 | 正向 | `record.name` 已定名 → send prompt 为笔记定位形（插值 `record.name` 与 `openspec/explores/<name>.md`，AC-8） | 单次 |
| auto-send 失败语义 | 异常 | `agent_start` reject → 既有 error 面承接、链路可续（失败不破坏 hook 状态） | 单次 |
| 既有返回面回归 | 正向 | messages / chain / events / loading / running / error / send / stop 形状与语义不变（契约面回归） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| invoke + Channel（IPC 进程边界） | `vi.mock('@tauri-apps/api/core')`：`invokeMock` 按命令名分发（`agent_sessions` / `agent_session_transcript` / `agent_start` 入参记录、可切换 reject / pending）+ `ChannelMock` 投递 AgentRunMessage 信封驱动流与终态（既有模式） | 全部用例 |
| buildExplorePrompt（内部模块） | 不 mock，真实实现参与 send 拼接（stance 分形断言经 prompt 原文承载） | stance 分形与 auto-send prompt 用例 |

### packages/desktop/src/lib/explore-stance.ts -> packages/desktop/src/lib/explore-stance.test.ts

#### 待测功能

- buildExplorePrompt(userInput, recordName): 按 `recordName` 的 `draft-` 前缀分两形（命名指令形 / 笔记定位形）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 命名指令形 | 正向 | `recordName` 为 `draft-<millis>` → 含 kebab 最终名指引、`create_explore_note(name, content)` 叙事、返回路径供 `edit` 续用、优先专用工具而非 bash；MUST NOT 出现任何文件路径插值（`openspec/explores` 零出现断言） | 单次 |
| 笔记定位形 | 正向 | `recordName = "api-retry"` → 插值 `api-retry` 与 `openspec/explores/api-retry.md`、`edit` 修改指引、文件缺失同名补建句 | 单次 |
| 两形公共段共享 | 正向 | 开篇句、立场五条、`# <标题>` 与 `workflow_type:` 两行约定、中文回答句在两形中均在（公共段单函数复用断言） | 逐项迭代 |
| 前缀判别精确性 | 边界 | `"my-draft-thing"`（中缀含 draft）→ 定位形；`"draft-"`（恰前缀空尾）→ 命令形 | 参数化迭代 |
| DRAFT_NAME_PREFIX 常量 | 正向 | 值为 `'draft-'`（与后端 `EXPLORE_PLACEHOLDER_PREFIX` 双源互链锚点） | 单次 |
| userInput 空串 | 边界 | 前导 + 空尾拼接、不抛错 | 单次 |
| userInput 特殊字符 | 边界 | 换行 / 引号 / 反引号 / 插值形貌 `${…}` / emoji → 原样保留在尾部、前导不被改写 | 单次 |
| userInput 超长 | 边界 | >1000 字符 → 原样拼接不被截断 | 单次 |
| 前导唯一性回归 | 边界 | 开篇句在输出中恰出现一次（既有结构性断言随两形改造迁移） | 单次 |
| 非法形态记录名判别 | 异常 | `recordName` 为大写 / 下划线 / 含 `/` 与 `..` 的路径形态 → 前缀判别仅按 `startsWith('draft-')` 执行、非前缀一律定位形（判别器不做 kebab 校验——校验归 create 执行体前置校验），不抛错 | 参数化迭代 |
| recordName 空串 | 异常 | `''`（无前缀）→ 定位形、插值空名、不抛错（生产不可达防御面：记录名恒非空，留痕口径） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 | 纯函数，零进程边界依赖 | 全部用例 |

---

## 不可测试项

| 模块 | 原因 |
|---|---|
| `packages/desktop/src-tauri/crates/infra/store/Cargo.toml` | 依赖声明（构建面）：`time = { workspace = true }` 无独立单元测试路径，经 `store_test.rs` 临时标题 UTC 格式断言间接验证 |
| `packages/desktop/src-tauri/crates/infra/store/src/lib.rs` | 再导出声明：该 crate 无 `lib_test.rs` 兄弟文件先例（与 core/agent、infra/agent 不同），不为单行再导出新建测试文件；`EXPLORE_PLACEHOLDER_PREFIX` 出口经消费侧（`commands/explores/mod_test.rs` 门禁、`sdk/explore_note_test.rs` 占位判别，均以 `store::` 导入路径使用）真实覆盖 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mod.rs` | 纯模块声明（`pub(crate) mod explore_note;` + `#[cfg(test)]` 声明随测试相位），无逻辑面；`explore_note` 行为由 `explore_note_test.rs` 承接 |
| `packages/desktop/src/types/generated/bindings.ts` | specta 生成物（非手改）：由 `bindings:check` 守卫拦截漂移（AC-10 桌面端执行面）；`createExploreRecord` 参数类型契约经前端测试 mock invoke 消费侧间接覆盖 |
| `plugins/dev-team/skills/openspec-explore/SKILL.md` | Markdown 文档（双源同步纪律，无执行面）：与 desktop stance 两形的同步正确性靠双侧注释互链与评审承接，不设机械断言 |
| `plugins/dev-team/package.json`、`packages/desktop/package.json` | 版本 bump（交付面）：由桌面端静态检查与插件三面产物重建流程承接（AC-10；`pnpm -C plugins/dev-team run build` 后 `claude-plugins/` / `cursor-plugins/` / `cursor-home-image/` 同步为构建断言） |
| `compose::assemble_injections` 非法名 `Err` 臂 | 六名注入名单为模块内常量，单元测试无注入口（防御臂生产不可达）；以「explore 装配产物逐名 ∈ `TOOL_NAMES`」断言等价收窄（`compose_test.rs` 用例承接） |
| `create_explore_note` ①落盘与②改名间并发失败窗口 | `ExploreNoteContext` 持具体类型 `Arc<Store>`（真实 native_db 组合，无改名失败注入口）；以前置校验零残留用例 +「文件已存在附路径」恢复通道用例等价收窄（design D12 留痕窗口） |
| `session_pump` 的 `ExploreNoteContext` 铸造内部态 | 铸造发生在泵内部 `LoopTurn` 字面量，无私有观察缝；经 `sdk/runner_test.rs`（open_session 端到端，RunStarted.tools 下发面口径证明注入链贯通）、`compose_test.rs`（装配产物真实驱动）与 `sdk/loop_test.rs` 直驱（`explore_note` 路由与上下文缺席臂）三侧行为面夹逼覆盖 |
