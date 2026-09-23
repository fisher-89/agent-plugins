# 测试设计: desktop-agent-execution

> **日期**: 2026-09-23

---

## 验收范围

<!-- 用模板表格逐条映射 proposal.md 的每个 AC：
  - `AC ID`：proposal 中`验收标准`编号
  - `验收条件`：原文摘录
  - `测试类型`：`单元测试` 或 `集成测试`
  - `被测文件或模块`：目标测试文件路径(单元测试) 或 构成关系的多个模块(集成测试) -->

| AC ID | 验收条件 | 测试类型 | 被测文件或模块 |
|--------|---------|---------|----------|
| AC-1 | `AgentEvent` 五变体信封（含 Block、`parent_tool_use_id`、seq/时间戳、Raw 透传）、`AgentRunner` trait、run 状态机落地；单测：五变体归一化、未知事件透传不炸、is_error 收敛 failed；Cargo.toml 无 tauri 系依赖、无 workspace 内依赖 | 单元测试 | `packages/desktop/src-tauri/crates/core/agent/src/event_test.rs`、`state_test.rs`、`runner_test.rs`（Cargo.toml 机械约束半边见不可测试项 5；另见集成关系 1/2） |
| AC-2 | `ClaudeCliRunner` 实现 trait；单测：default+bypass 组装含 `-p --output-format stream-json --verbose --dangerously-skip-permissions`、bare 组装含 `--bare`；JSONL fixture 逐行解析 seq 单调；未知 type 产出 Raw；CLI 缺失显式错误；Cargo.toml 无 tauri 系依赖 | 单元测试 | `packages/desktop/src-tauri/crates/infra/agent/src/flags_test.rs`、`jsonl_test.rs`、`discover_test.rs`、`runner_test.rs`（Cargo.toml 半边与真实 CLI 端到端半边见不可测试项 4/5；另见集成关系 1） |
| AC-3 | `user_agent_runs` / `user_agent_run_events` 落地，key `(run_id, seq)`；store_test 覆盖写入、按 run 重放、重开持久性；表名携 user 维度前缀 | 单元测试 | `packages/desktop/src-tauri/crates/infra/store/src/store_test.rs`、`model_test.rs`（另见集成关系 2/3） |
| AC-4 | `agent_start` 经 `run_agent()` tee 双 sink（Channel + store）；`agent_runs` / `agent_run_events` 无状态薄包装；命令层测试（MockRuntime）；CLI 不可发现返回 `Err(String)`；`main.rs` 注册齐全 | 集成测试 | `packages/desktop/src-tauri/src/commands/exec/agent.rs`（编排）+ `crates/core/agent/src/state.rs`（收敛）+ `crates/infra/store/src/store.rs`（落库）+ `crates/core/agent/src/runner.rs`（假 runner 注入缝）；见集成关系 2/3；薄包装单测与 `Err(String)` 映射见 `src/commands/exec/mod_test.rs` 章节；`main.rs` 注册半边见不可测试项 3 |
| AC-5 | 侧栏页面导航组切换无路由；参数面默认 env=default、permission-mode=bypassPermissions、prompt 必填；时间线呈现 assistant → tool_use → tool_result → result 汇总（num_turns / cost / duration / session_id 可复制）、子代理归因；原始 JSONL 切换；历史运行经 invoke 重放；前端测试全绿 | 集成测试 | `packages/desktop/src/views/agent/**`（View + 四子组件 + 两 hooks）+ `src/App.tsx` + `src/components/AppSidebar.tsx` + `src/types/dto.ts`；见集成关系 4/5/6；组件级单测见单元测试各对应章节 |
| AC-6 | 无 kill / cancel 入口；无 `--resume` / `--include-partial-messages` / model 参数；五条边界限制写进 spec 可考 | 单元测试 | `packages/desktop/src-tauri/crates/infra/agent/src/flags_test.rs`（禁用 flag 负向断言）+ `packages/desktop/src/views/agent/AgentDebugView.test.tsx`（无 kill 入口缺席断言）；spec 留痕半边见不可测试项 2 |
| AC-7 | `desktop-crate-layout`（五类分类学、依赖规则、租户表）、`desktop-app-shell`（exec 轨道、微形态、翻转信号落痕、导航组、流例外）、`desktop-data-dimensions`（user 维度注记）delta 与实现一致 | 不可测试 | —（见不可测试项 1） |
| AC-8 | `cargo test --workspace`（rust 套件注册于 src-tauri 根，新 crate 自动覆盖）全绿；`pnpm -C packages/desktop run client:check`（vp check --fix + knip）通过；`vp test` 全绿 | 单元测试 | 全部 `*_test.rs`（rust 套件于 src-tauri 根自动收编新 crate）与全部前端 `*.test.ts(x)` 套件（「套件全绿」回归半边，见单元测试/集成测试各章节）；client:check 半边见不可测试项 6 |

---

## 单元测试

<!-- 覆盖所有可在进程内验证的场景（Rust #[test]、前端 Vitest 纯函数/组件 mock 测试）。
  每个源文件对应一个独立的 `### <源文件> -> <测试文件>` 章节，该文件的所有测试用例和 Mock 策略均在对应章节内集中描述。 -->

<!-- 测试文件路径由 mcp test_resolve_paths 解析（强制）：proposal「测试文件」中的 `cli_runner_test.rs` 占位名按解析结果落为 `flags_test.rs` / `jsonl_test.rs` / `discover_test.rs` / `runner_test.rs` 四分册。Rust 侧沿仓库 `*_test.rs` 同目录共置惯例（dev-team 为纯 binary crate，命令层集成用例亦共置，见集成关系章节；共置同时遵守「无 test-only 导出」项目规则）。前端框架为 vite-plus（vp 套件），jsdom 装置沿 App.test.tsx / AppSidebar.test.tsx 既有惯例。 -->

### packages/desktop/src-tauri/crates/core/agent/src/lib.rs -> packages/desktop/src-tauri/crates/core/agent/src/lib_test.rs

<!-- design.md 公共函数/API 表未声明本文件的公共 API 变更（仅模块组装与 crate 级 doc），不设用例；crate 契约由 event/state/runner 各分册覆盖。「Cargo.toml 无 tauri 系依赖、无 workspace 内依赖」为 design 机械约束（评审按此核对），见不可测试项 5。 -->

### packages/desktop/src-tauri/crates/core/agent/src/event.rs -> packages/desktop/src-tauri/crates/core/agent/src/event_test.rs

#### 待测功能

- AgentEvent.stamp(): 盖 seq + 当前时钟毫秒的统一构造入口（生产者统一入口）
- AgentEvent（类型定义）: `{ seq, timestampMs, kind }` camelCase 线格式，kind 判别扁平进顶层
- AgentEventKind（类型定义）: runStarted / message / systemNotice / runResult / raw 五变体字段口径
- AgentBlock（类型定义）: text / thinking / toolUse{id,name,input} / toolResult{id,content,isError} 四变体

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| AgentEvent.stamp | 正向 | stamp(0, kind) 产出 seq=0 且 timestamp_ms 为当前时钟毫秒（大于 0） | 新增 |
| AgentEvent.stamp | 边界 | seq=u64::MAX 原样保留不截断 | 新增 |
| AgentEventKind serde | 正向 | 五变体各自序列化含 seq/timestampMs 顶层键，判别值为驼峰（runStarted/isError/parentToolUseId/rawJson），反序列化往返逐字段相等 | 新增 |
| AgentEventKind serde | 边界 | message 携带空 blocks 数组与四类块混合数组（text+thinking+toolUse+toolResult）均往返无损 | 新增 |
| AgentEventKind serde | 边界 | parentToolUseId 为 null 与有值两形态均保真 | 新增 |
| AgentEventKind serde | 边界 | raw 变体 rawJson 保留原文逐字节（含中文/emoji/换行/引号） | 新增 |
| AgentEventKind serde | 异常 | 缺 seq 或 timestampMs 必填键的 JSON 反序列化返回 Err，不产生半成品事件 | 新增 |

#### Mock策略

<!-- 无外部依赖（纯内存构造 + serde_json），不需要 Mock。 -->

### packages/desktop/src-tauri/crates/core/agent/src/state.rs -> packages/desktop/src-tauri/crates/core/agent/src/state_test.rs

#### 待测功能

- RunStateMachine.new(): 初始 Running
- RunStateMachine.apply(): RunResult 驱动收敛；收敛后拒绝变更
- RunStateMachine.current(): 当前状态观测

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| RunStateMachine.new | 正向 | new() 后 current() 为 Running | 新增 |
| RunStateMachine.apply | 正向 | RunResult 且 is_error=false → 收敛 Completed | 新增 |
| RunStateMachine.apply | 正向 | RunResult 且 is_error=true → 收敛 Failed（AC-1「is_error 收敛 failed」） | 新增 |
| RunStateMachine.apply | 正向 | 非 RunResult 事件（runStarted/message/systemNotice/raw）→ 状态保持 Running | 新增 |
| RunStateMachine.apply | 边界 | 收敛后再 apply 任意事件 → 终态不被改写（拒绝变更） | 新增 |
| RunStateMachine.apply | 边界 | 连续两个 RunResult（先 is_error=false 后 true）→ 首个收敛生效 | 新增 |
| RunStateMachine.current | 边界 | 每次 apply 返回值与随后 current() 观测一致 | 新增 |

#### Mock策略

<!-- 无外部依赖，不需要 Mock。 -->

### packages/desktop/src-tauri/crates/core/agent/src/runner.rs -> packages/desktop/src-tauri/crates/core/agent/src/runner_test.rs

#### 待测功能

- AgentRunner.start(): 唯一 trait 方法，进程模型不出现在签名（Send + Sync 契约，三租户预留面）
- AgentRunParams（类型定义）: `{ prompt, cwd, env, permission_mode }`
- AgentEnvMode / AgentPermissionMode（类型定义）: 单元 enum，serde camelCase 值 + as_str() 供落库
- AgentStartError（类型定义）: CliMissing / SpawnFailed，实现 Display
- AgentRun / RunHandle（类型定义）: `events: mpsc::Receiver<AgentEvent>` + 句柄半边

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| AgentRunner.start | 正向 | 假 runner 实现 trait 并经 mpsc 交付预录事件：start 返回 AgentRun，事件按预录顺序自 events 接收（proposal「假 runner 事件流」） | 新增 |
| AgentRunner.start | 边界 | 假 runner 可作 trait object 注入（满足 Send + Sync bound），供 run_agent_with 泛型缝使用 | 新增 |
| AgentEnvMode serde | 正向 | default / bare 序列化为 "default" / "bare"，as_str() 同值 | 新增 |
| AgentPermissionMode serde | 正向 | 三值序列化为 "default" / "acceptEdits" / "bypassPermissions"，as_str() 同值 | 新增 |
| AgentEnvMode / AgentPermissionMode serde | 异常 | 非法字符串（"Bare"、"bypass"、""）反序列化返回 Err（枚举 N+1 边界） | 新增 |
| AgentStartError Display | 正向 | CliMissing / SpawnFailed 的 Display 文案携带原因串（错误串可直抵前端） | 新增 |
| AgentRunParams serde | 边界 | cwd 含中文/空格/尾分隔符的 PathBuf 序列化往返无损 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| AgentRunner 事件源 | 测试内实现假 runner（trait 测试替身，非外部进程 mock）：预录 AgentEvent 序列经 mpsc 交付 | AgentRunner.start 两用例 |

### packages/desktop/src-tauri/crates/infra/agent/src/lib.rs -> packages/desktop/src-tauri/crates/infra/agent/src/lib_test.rs

<!-- design.md 公共函数/API 表未声明本文件的公共 API 变更（仅模块组装与 ClaudeCliRunner 导出），不设用例；行为由 discover/flags/jsonl/runner 四分册覆盖。「Cargo.toml 无 tauri 系依赖」为 design 机械约束，见不可测试项 5。 -->

### packages/desktop/src-tauri/crates/infra/agent/src/discover.rs -> packages/desktop/src-tauri/crates/infra/agent/src/discover_test.rs

<!-- design.md 公共函数/API 表未声明本文件条目；测试对象取自 design 决策 D14（`discover_in(dirs)` 纯函数 + 读 PATH 薄包装 + `CliMissing`）与 AC-2「CLI 缺失显式错误」，非虚构。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| discover_in | 正向 | 合成目录含 claude.cmd → 发现并返回该路径 | 新增 |
| discover_in | 边界 | 同目录 claude.cmd / claude.bat / claude.exe 三者并存 → 按 .cmd > .bat > .exe 优先级取 claude.cmd（D14 顺序） | 新增 |
| discover_in | 边界 | 仅 claude.bat 或仅 claude.exe → 依序降级发现对应文件 | 新增 |
| discover_in | 边界 | 多目录各含候选 → 先声明目录优先 | 新增 |
| discover_in | 边界 | 目录列表含不存在目录（PATH 失效项）→ 跳过不 panic | 新增 |
| discover_in | 异常 | 目录列表为空 / 全部目录均无候选 → 显式 `AgentStartError::CliMissing`（Display 含检索线索） | 新增 |
| PATH 薄包装 | 正向 | 以合成 PATH 指向含伪 claude.cmd 的 tempdir → 发现该文件（env 修改以互斥锁串行化保护） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 文件系统 PATH 候选 | tempfile 合成目录 + 写入空的伪 claude.cmd / claude.bat / claude.exe 文件（不依赖真实 CLI 安装） | discover_in / PATH 薄包装全部用例 |

### packages/desktop/src-tauri/crates/infra/agent/src/flags.rs -> packages/desktop/src-tauri/crates/infra/agent/src/flags_test.rs

<!-- design.md 公共函数/API 表未声明本文件条目；测试对象取自 design「flag 组装口径」表与 AC-2 明文断言，非虚构。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| flag 组装 | 正向 | env=default + permission-mode=bypassPermissions → 恰含 `-p <prompt>`、`--output-format stream-json`、`--verbose`、`--dangerously-skip-permissions`（AC-2 四 flag 断言） | 新增 |
| flag 组装 | 正向 | env=bare → 追加 `--bare`；env=default → 不含 `--bare`（AC-2 bare 断言） | 新增 |
| flag 组装 | 正向 | permission-mode=acceptEdits → `--permission-mode acceptEdits`；permission-mode=default → 无 permission flag（CLI `-p` 默认档） | 新增 |
| flag 组装 | 边界 | 空 prompt（""）→ 仍组装 `-p ""`（组装层恒有 -p，必填把关在 UI 层） | 新增 |
| flag 组装 | 边界 | prompt 含空格/引号/换行/中文/emoji → 作为单个 arg 原样保留不拆分 | 新增 |
| flag 组装 | 边界 | 超长 prompt（>1000 字符）完整保留于 arg | 新增 |
| flag 组装（AC-6 负向） | 边界 | 3 env × 3 permission-mode 全组合 → 组装结果均不含 `--resume`、`--include-partial-messages`、`--model` | 新增 |
| cwd | 边界 | cwd 不产生任何 flag（仅 `Command::current_dir` 传递，见 flag 组装口径表） | 新增 |

#### Mock策略

<!-- 纯函数，无外部依赖，不需要 Mock。 -->

### packages/desktop/src-tauri/crates/infra/agent/src/jsonl.rs -> packages/desktop/src-tauri/crates/infra/agent/src/jsonl_test.rs

<!-- design.md 公共函数/API 表未声明本文件条目；测试对象取自 design「JSONL → 变体映射」归一化口径表与决策 D8，非虚构。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| system init 行 | 正向 | type=system, subtype=init → runStarted，提取 model/session_id/tools/mcp_servers | 新增 |
| system 其余行 | 正向 | type=system 其余 subtype → systemNotice{subtype, payload=原 JSON}（不封闭枚举） | 新增 |
| assistant / user 行 | 正向 | → message{role=type, blocks 由 content 数组映射 text/thinking/tool_use/tool_result, parent_tool_use_id} | 新增 |
| result 行 | 正向 | → runResult，total_cost_usd→costUsd、duration_ms→durationMs、is_error/num_turns/usage/session_id 提取 | 新增 |
| 未知 type 行 | 正向 | 合法 JSON 未知 type → raw{eventType=type, rawJson=原文}（AC-2「未知 type 产出 Raw」） | 新增 |
| 非 JSON 行 | 异常 | → raw{eventType="unparsable", rawJson=原文}，永不炸解析、永不丢事件 | 新增 |
| 空白行 | 边界 | 跳过且不占 seq（D8） | 新增 |
| tool_result 块 | 边界 | content 数组拼接扁平化为字符串；非数组形态 JSON 序列化兜底 | 新增 |
| 字段缺失 | 边界 | init 缺 model/session_id → null、tools/mcp_servers 缺 → 空数组；result 缺 cost/duration/num_turns/session_id → null | 新增 |
| seq 单调 | 边界 | 多行 fixture（混入空白行）归一化 seq 从 0 单调递增、空白行不占号（AC-2「seq 单调」） | 新增 |
| 空输入 | 边界 | 零行输入 → 零事件不报错 | 新增 |
| 洪峰 | 边界 | 300 行混合 fixture 全部产出事件，不丢不炸 | 新增 |
| 深嵌套 | 边界 | payload 深嵌套对象与超长字符串（>1000 字符）不炸且 rawJson 保真 | 新增 |

#### Mock策略

<!-- 行字符串 fixture 以内嵌常量构造，无进程/文件边界，不需要 Mock。 -->

### packages/desktop/src-tauri/crates/infra/agent/src/runner.rs -> packages/desktop/src-tauri/crates/infra/agent/src/runner_test.rs

#### 待测功能

- ClaudeCliRunner.new(): 无状态 runner 构造
- ClaudeCliRunner.start(): impl AgentRunner；discover → spawn（`current_dir(cwd)`、stdout piped）→ tokio 泵任务逐行归一化发入有界 mpsc → EOF 无 result 补发合成 RunResult（决策 D6/D8；泵逻辑以可注入行流测，不 spawn 真实进程）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 泵任务 | 正向 | 完整会话 fixture（init → assistant(tool_use) → user(tool_result) → result）逐行入泵 → 事件按序入 mpsc、seq 0..n 单调、正常 result 收尾后不补发合成事件（D6） | 新增 |
| 泵任务 | 异常 | EOF 无 result（模拟进程异常退出，携带退出码 N）→ 补发合成 RunResult：subtype=error_process_exit、is_error=true、error 记退出码（D6） | 新增 |
| 泵任务 | 边界 | 空输出流即退出 → 仅一条合成 RunResult（seq=0） | 新增 |
| 泵任务 | 边界 | 有界通道容量 256：315 行洪峰全量送达（背压不丢事件，D1 通道口径） | 新增 |
| 泵任务 | 边界 | 混入未知 type 与非 JSON 行 → Raw 占 seq 透传（与 jsonl 口径一致） | 新增 |
| ClaudeCliRunner.new | 边界 | 两次 new 实例互不共享状态 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 真实 claude 进程 | 不 mock 真实进程：泵逻辑以内存行流 + 可注入退出码驱动；真实 spawn 端到端见不可测试项 4 | 全部泵任务用例 |
| stdout 字节流 | fixture JSONL 字符串常量经行切分注入 | 全部泵任务用例 |

### packages/desktop/src-tauri/crates/infra/store/src/lib.rs -> packages/desktop/src-tauri/crates/infra/store/src/lib_test.rs

<!-- design.md 公共函数/API 表未声明本文件的公共 API 变更（仅 `pub use model::AgentRunRecord;` 重导出），不设用例；AgentRunRecord 行为见 model.rs / store.rs 分册。 -->

### packages/desktop/src-tauri/crates/infra/store/src/model.rs -> packages/desktop/src-tauri/crates/infra/store/src/model_test.rs

<!-- design.md 公共函数/API 表未声明本文件条目；测试对象取自变更清单「AgentRunRecord（含 encode/decode JSON 编解码，沿 WorkspaceRecord 模式）」与数据模型节，非虚构。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| encode | 正向 | 编码产物为 camelCase JSON 字节串，顶层键恰为 id/prompt/cwd/env/permissionMode/status/startedAt/finishedAt/numTurns/costUsd/durationMs/sessionId/error | 新增 |
| encode/decode 往返 | 正向 | 全字段填充记录 encode→decode 往返逐字段相等 | 新增 |
| Option 字段 | 边界 | running 行全 Option=None：序列化含 null 键、decode 回 None（汇总字段留空形态） | 新增 |
| 字符串字段 | 边界 | prompt/error 含中文/emoji/换行与超长串（>1000 字符）往返无损 | 新增 |
| status 受控串 | 边界 | running/completed/failed 三值直存直取（store 不引本地枚举，见数据模型节） | 新增 |
| decode | 异常 | 非法 JSON 字节 decode 返回 Err 不 panic | 新增 |

#### Mock策略

<!-- 纯编解码，无外部依赖，不需要 Mock。 -->

### packages/desktop/src-tauri/crates/infra/store/src/store.rs -> packages/desktop/src-tauri/crates/infra/store/src/store_test.rs

#### 待测功能

- Store.begin_agent_run(): 写事务内 max(id)+1 分配 id，落 running 行，返回含 id 记录（决策 D4）
- Store.append_agent_run_events(): 单事务批量追加，key `(run_id, seq)`，seq 取事件自带值
- Store.finish_agent_run(): 以传入记录整行替换（status / finished_at / 汇总 / error）
- Store.list_agent_runs(): started_at 降序
- Store.list_agent_run_events(): `(run_id, seq)` 区间扫描，seq 升序

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| begin_agent_run | 正向 | 空库首跑返回 id=1、status=running、started_at 落值（AC-3） | 新增 |
| begin_agent_run | 边界 | 连续 begin → id 严格递增（max+1 分配） | 新增 |
| begin_agent_run | 边界 | 传入记录的 id 字段不参与匹配：以返回记录（含分配 id）落行为准 | 新增 |
| append_agent_run_events | 正向 | 批量 serde_json::Value 事件落 `(run_id, seq)`，list_agent_run_events 按 seq 升序读回（AC-3 重放） | 新增 |
| append_agent_run_events | 边界 | 空切片 → Ok 且不产生行 | 新增 |
| append_agent_run_events | 边界 | 乱序 seq 写入仍按 key 序升序读回 | 新增 |
| append_agent_run_events | 边界 | 事件 Value 含中文/emoji/深嵌套 → 进出无损（D3：事件行以 Value 进出 store） | 新增 |
| finish_agent_run | 正向 | finish 后 status/finished_at/汇总/error 整行替换，list_agent_runs 反映终态 | 新增 |
| list_agent_runs | 正向 | 多 run 按 started_at 降序（与 workspace 清单同哲学） | 新增 |
| list_agent_runs | 边界 | 空库 → 空向量不报错 | 新增 |
| list_agent_run_events | 边界 | 不存在 run_id → 空向量不报错 | 新增 |
| 两 run 隔离 | 边界 | run A 与 run B 同 seq 互不串扰（复合键半开区间扫描按 run_id 隔离） | 新增 |
| 重开持久性 | 正向 | run + events 写入 → drop → 重开同一 db 文件 → 记录与事件完整（AC-3「重开持久性」） | 新增 |
| 表名前缀 | 边界 | 裸 redb 只读句柄可见 `user_agent_runs` / `user_agent_run_events` 两表（user 维度前缀，沿既有 TEST 表定义检查手法） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| redb 数据库 | 不 mock：tempdir 真开 db 文件（沿既有 Env 装置），存储层语义真实（沿 store_test.rs 既有口径） | 全部用例 |

### packages/desktop/src-tauri/src/commands/exec/agent.rs -> packages/desktop/src-tauri/src/commands/exec/agent_test.rs

#### 待测功能

- run_agent(): 薄入口，组装 `ClaudeCliRunner` 后委托 `run_agent_with`（真实 CLI 正向路径不直测，见不可测试项 4；Err 映射经 mod_test.rs 承载）
- run_agent_with(): 泛型编排（tee 双 sink + 状态机收敛），假 runner 注入缝（决策 D1/D2/D5/D7/D8）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| run_agent_with | 正向 | 假 runner 预录 completed 会话 → 事件全量落库 `(run_id, seq)` 且 Channel 逐事件一致、两路 seq 一致 → 返回 completed 记录（AC-4「tee 双 sink」） | 新增 |
| run_agent_with | 正向 | 预录 is_error result → 返回 Ok(failed 记录)：status=failed、error 记因（D7 in-band 失败不 Err） | 新增 |
| run_agent_with | 正向 | 事件流中 RunResult 驱动状态收敛（core/agent state.rs 经编排集成生效，AC-1） | 新增 |
| run_agent_with | 异常 | 假 runner start 返回 Err → 编排返回 Err(String) 且 store 无 run 行（D5「启动失败不留行」，AC-4「CLI 不可发现返回 Err(String)」同型路径） | 新增 |
| run_agent_with | 异常 | store 写入失败（以并行写事务独占 redb 文件构造）→ run 收敛 failed（error 记因）且 tee 终止、已送达 Channel 的事件保留（D1「落库兜底失败不可静默」） | 新增 |
| run_agent_with | 边界 | Channel 接收端先行关闭（模拟页面已关）→ 落库继续完整、命令仍返回最终记录（D1「Channel 发送失败不中断落库」） | 新增 |
| run_agent_with | 边界 | 预录事件 seq 含缺口/乱序 → tee 透传不重排（D8） | 新增 |
| run_agent | 异常 | 隔离 PATH（CLI 不可发现，env 修改互斥串行化）→ run_agent 走完整薄入口链（组装 ClaudeCliRunner → start 失败）返回 Err(String) 且 store 无 run 行 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| AgentRunner 事件源 | 假 runner 测试替身（core trait 实现）：预录 AgentEvent 序列 / 可控 start Err | 正向与异常编排用例 |
| Tauri Channel | `tauri::ipc::Channel::new` 捕获回调收集事件；以显式丢弃接收端构造「页面已关」 | tee 双路 / Channel 关闭用例 |
| redb store | 不 mock：tempdir 真库；写失败以并行写事务独占构造 | store 写失败用例 |
| 真实 claude CLI | 不引入：正向编排由假 runner 承载（真实进程见不可测试项 4） | — |

### packages/desktop/src-tauri/src/commands/exec/mod.rs -> packages/desktop/src-tauri/src/commands/exec/mod_test.rs

#### 待测功能

- agent_start(): async 命令，body 三件事（参数转换 → `run_agent()` → 错误映射），IPC 键 camelCase（`onEvent`/`permissionMode`）
- agent_runs(): 无状态薄包装，startedAt 降序
- agent_run_events(): 薄包装，Value → AgentEvent 反序列化后返回

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| agent_runs | 正向 | MockRuntime manage 真实 Store：命令面与直连 `list_agent_runs` 的 serde 值一致（薄包装不加工，沿 workspaces passthrough 先例） | 新增 |
| agent_runs | 边界 | 空库 → serde_json 值为 `[]` | 新增 |
| agent_run_events | 正向 | 落库事件经命令面读回为反序列化后的 AgentEvent（seq 升序、五变体保真） | 新增 |
| agent_run_events | 边界 | 不存在 runId → 空数组 | 新增 |
| agent_start | 异常 | 启动阶段失败（以隔离 PATH 构造 CLI 不可发现，env 修改互斥串行化）→ Err(String) 且 store 无 run 行（AC-4 / D5） | 新增 |
| agent_start | 边界 | agent_start 为 async 且阻塞至 run 结束的签名契约（设计 D7；以假 runner 编排等价的 run_agent_with 侧断言承载，命令体仅参数转换） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| Tauri runtime | `tauri::test::mock_app()`（MockRuntime，无窗口无事件循环）manage 真实 Store（tempdir 真库），沿 workspaces/mod_test.rs 惯例 | 全部用例 |
| 真实 claude CLI | agent_start 正向不直测；以隔离 PATH 触发 CliMissing 走 Err 分支（PATH 环境变量修改以互斥锁串行化） | agent_start 异常用例 |

### packages/desktop/src-tauri/src/commands/mod.rs -> packages/desktop/src-tauri/src/commands/mod_test.rs

<!-- design.md 公共函数/API 表未声明本文件的公共 API 变更：本变更仅改模块 doc（exec 轨道由「预留空轨道」改为「已开通」注记），无行为变更，不设用例。既有三查询 / 四 workspace 命令语义属 proposal「不要修改」范围，由既有 queries/workspaces 两个测试文件回归守护。 -->

### packages/desktop/src-tauri/src/main.rs -> packages/desktop/src-tauri/src/main_test.rs

<!-- design.md 公共函数/API 表未声明本文件的公共 API 变更：invoke_handler 注册三命令为声明式宏展开；tauri::test MockRuntime 不经 main.rs 的 handler 表，进程内无法断言注册 → 不设用例，见不可测试项 3。 -->

### packages/desktop/src/App.tsx -> packages/desktop/src/App.test.tsx

<!-- design.md TS 公共函数/API 表未声明 App.tsx 条目；被测行为取自变更清单「page 顶层切换 state（changes | agent）+ SidebarInset 按 page 渲染（决策 D10）」，非虚构。既有用例全部保留回归。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 顶层视图切换 | 正向 | 启动默认呈 changes 页（change 清单内容在场），Agent 调试页不在场（无路由，state 切视图） | 新增 |
| 顶层视图切换 | 正向 | 侧栏点击「Agent 调试」→ AgentDebugView 呈现、ChangeView 内容卸载 | 新增 |
| 顶层视图切换 | 正向 | Agent 页点击「变更」→ 切回清单视图 | 新增 |
| 顶层视图切换（D10） | 边界 | 进入 change 详情后切到 Agent 页再切回 → 选中重置回清单（不保留详情选中、不以旧选中重发 get_change_detail） | 新增 |
| 顶层视图切换（D10） | 边界 | useChangeList 留在 App 层不随页面卸载：切页往返不重发 list_workspaces（清单数据不丢） | 新增 |
| 顶层视图切换 | 边界 | root=null（无 workspace）时仍可切到 Agent 页：页面自呈现、启动入口禁用、不崩 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| @tauri-apps/api/core invoke | 沿既有 vi.mock 装置：按命令名分发可控行为（本组用例仅涉及既有清单命令） | 全部用例 |
| matchMedia / 窗口视口 | 沿既有 stubViewport 装置（SidebarProvider 消费 useIsMobile） | 全部用例 |

### packages/desktop/src/components/AppSidebar.tsx -> packages/desktop/src/components/AppSidebar.test.tsx

#### 待测功能

- TopPage: `'changes' | 'agent'` 顶层视图判别类型（type-only，无运行时行为；运行时被测面为新增页面导航组 props：page / onPageChange）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 页面导航组 | 正向 | 「页面」导航组渲染于 workspace 清单组上方，含「变更」与「Agent 调试」两项 | 新增 |
| 页面导航组 | 正向 | 当前 page 项呈激活态、另一项不激活 | 新增 |
| 页面导航组 | 正向 | 点击「Agent 调试」→ onPageChange('agent') 恰一次；点击「变更」→ onPageChange('changes') 恰一次 | 新增 |
| 页面导航组 | 边界 | page='agent' 时 workspace 清单组照常渲染、语义不变（既有清单/右键/Tooltip 用例全部保留回归） | 新增 |
| 页面导航组 | 边界 | 两入口各带 lucide 图标（以 DOM 结构断言，非观感） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| matchMedia / ResizeObserver | 沿既有 stubEnvironment 装置（jsdom 缺口兜底） | 全部用例 |

### packages/desktop/src/types/dto.ts -> packages/desktop/src/types/dto.test.ts

<!-- design.md 公共函数/API 表未声明本文件的函数条目：本文件为纯类型定义（AgentEvent kind 判别 union / AgentBlock / AgentRunRecord / 三枚举字面量 union），无运行时行为，不设用例；类型与 Rust serde camelCase 线格式的一致性由编译期检查与集成关系 4/5 的线格式断言间接守护（见不可测试项 6）。 -->

### packages/desktop/src/views/agent/AgentDebugView.tsx -> packages/desktop/src/views/agent/AgentDebugView.test.tsx

#### 待测功能

- AgentDebugView({ root }): 页面骨架（参数面 + 实时时间线 + 原始流切换 + 历史运行区组装），props `{ root: string | null }`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| AgentDebugView | 正向 | 渲染四区块：参数面（AgentRunForm）、事件时间线、原始 JSONL 切换入口、历史运行区 | 新增 |
| AgentDebugView | 正向 | 原始 JSONL 切换：切至 AgentRawStream 呈现、切回时间线（D12 数据源为事件落库形态） | 新增 |
| AgentDebugView | 边界 | root=null → 启动入口禁用（无 workspace 不可发起），页面不崩 | 新增 |
| AgentDebugView | 边界 | running 中表单禁用（disabled 向 AgentRunForm 传导） | 新增 |
| MVP 边界（AC-6） | 边界 | 全页无「停止 / 取消 / kill」入口（缺席断言，AC-6「无 kill / cancel 入口」） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| agent 域 hooks | vi.mock `useAgentRun` / `useAgentRunHistory` 返回受控 fixture state（组件纯 props 驱动） | 全部用例 |

### packages/desktop/src/views/agent/components/AgentRunForm.tsx -> packages/desktop/src/views/agent/components/AgentRunForm.test.tsx

#### 待测功能

- AgentRunForm({ disabled, onStart }): 参数面（prompt 必填 / env 双档默认 default / permission-mode 三档下拉默认 bypassPermissions / 无 cwd 与 model 输入）；`AgentStartInput = { prompt, env, permissionMode }`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 默认值（AC-5） | 正向 | 初始 env=default、permission-mode=bypassPermissions（默认档断言） | 新增 |
| prompt 必填（AC-5） | 边界 | prompt 为空串 → 启动按钮禁用；输入任意非空后启用 | 新增 |
| onStart | 正向 | 填写后点击启动 → onStart 以 `{ prompt, env, permissionMode }` 恰调用一次 | 新增 |
| env 双档 | 正向 | 切 bare → 认证前提提示在场；切回 default → 提示消失（风险表「bare 认证前提提示」） | 新增 |
| permission-mode 三档 | 边界 | 下拉含 default / acceptEdits / bypassPermissions 三档且均可选回填 | 新增 |
| disabled | 边界 | disabled=true → 启动入口禁用且点击不触发 onStart | 新增 |
| 输入保真 | 边界 | prompt 含换行 / emoji / 超长（>1000 字符）→ onStart 原样上抛 | 新增 |
| MVP 边界（AC-6） | 边界 | 无 cwd 输入、无 model 选择（缺席断言，D9：cwd 由 invoke 隐含传 root） | 新增 |

#### Mock策略

<!-- 纯回调组件（onStart 以 vi.fn() 注入），无进程边界，不需要 Mock。 -->

### packages/desktop/src/views/agent/components/AgentEventTimeline.tsx -> packages/desktop/src/views/agent/components/AgentEventTimeline.test.tsx

#### 待测功能

- AgentEventTimeline({ events, running }): 事件时间线（实时与重放共用）：对话流、tool_use/tool_result 折叠块成对、`parentToolUseId` 子代理分组、result 汇总卡（numTurns/cost/duration/sessionId 可复制）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 对话流 | 正向 | assistant message 的 text / thinking 块按序呈现且 role 标识在场（AC-5「assistant →」） | 新增 |
| 成对折叠块 | 正向 | tool_use 与紧随的 tool_result 折叠块成对呈现（同 id 关联，AC-5「tool_use → tool_result」） | 新增 |
| 子代理归因 | 正向 | parentToolUseId 有值的事件归入子代理分组、与主线区分（AC-5「子代理归因」） | 新增 |
| result 汇总卡 | 正向 | runResult 渲染 numTurns / cost / duration / sessionId 且各值可复制（AC-5「可复制」） | 新增 |
| result 汇总卡 | 边界 | 汇总字段为 null（缺失）→ 呈现占位不崩 | 新增 |
| is_error 呈现 | 异常 | ToolResult is_error=true 与 failed result → 错误标记 / failed 终态呈现 | 新增 |
| 空态 | 边界 | events=[] → 空态占位不崩 | 新增 |
| running | 边界 | running=true → 进行中标记；false 且无 result → 静止态 | 新增 |
| Raw 透传 | 边界 | raw 事件以透传占位呈现（eventType + 原文入口）不崩 | 新增 |
| 容量 | 边界 | 200 事件长列表全量渲染无丢失、超长 content（>1000 字符）折叠不撑爆布局 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 剪贴板 | 若复制实现走 `navigator.clipboard` 则以 vi.stubGlobal stub 并断言调用；不依赖真实剪贴板权限 | result 汇总卡可复制用例 |

### packages/desktop/src/views/agent/components/AgentRawStream.tsx -> packages/desktop/src/views/agent/components/AgentRawStream.test.tsx

#### 待测功能

- AgentRawStream({ events }): 原始 JSONL 面板，渲染每个事件的落库 JSON 形态（决策 D12 同源）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| JSON dump | 正向 | 每事件渲染一行 JSON，键形态与线格式一致（camelCase） | 新增 |
| Raw 保真 | 正向 | raw 变体呈现 rawJson 原文（不二次转义失真） | 新增 |
| 空态 | 边界 | events=[] → 空面板不崩 | 新增 |
| 字符保真 | 边界 | 含中文 / emoji / 换行的 payload 完整呈现 | 新增 |
| 容量 | 边界 | 大事件列表（300 条）渲染不崩 | 新增 |

#### Mock策略

<!-- 纯 props 渲染，无进程边界，不需要 Mock。 -->

### packages/desktop/src/views/agent/components/AgentRunHistory.tsx -> packages/desktop/src/views/agent/components/AgentRunHistory.test.tsx

#### 待测功能

- AgentRunHistory({ state }): 历史运行区：run 列表（状态 / 时间 / 摘要）→ 点开重放 + 显式刷新按钮

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| run 列表 | 正向 | runs 渲染状态 / 时间 / 摘要三要素 | 新增 |
| 点开重放 | 正向 | 点击 run → openRun(id) 恰调用一次（AC-5「历史运行经 invoke 重放」触发半边） | 新增 |
| 显式刷新 | 正向 | 点击刷新 → refresh() 恰调用一次（D13 显式触发，run 结束不自动刷新） | 新增 |
| 选中态 | 边界 | selectedRunId 匹配项呈激活态标识 | 新增 |
| 空态 | 边界 | runs=[] → 空态文案不崩 | 新增 |
| loading / error | 异常 | loading=true 呈现加载态；error 非空呈现错误条 | 新增 |

#### Mock策略

<!-- state 对象（AgentRunHistoryState）以 fixture 直传，无进程边界，不需要 Mock。 -->

### packages/desktop/src/views/agent/hooks/useAgentRun.ts -> packages/desktop/src/views/agent/hooks/useAgentRun.test.ts

#### 待测功能

- useAgentRun(root): `AgentRunState = { events, running, error, result, start }`；`new Channel<AgentEvent>()` 订阅 + invoke `agent_start`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| start | 正向 | start(input) → invoke('agent_start', { onEvent: Channel 实例, root, prompt, env, permissionMode }) 参数契约（IPC 键 camelCase） | 新增 |
| start | 正向 | Channel 逐事件回调 → events 按序累积；发起后 running=true | 新增 |
| start | 正向 | invoke resolve 记录 → result 置记录、running=false（completed 与 failed 两形态，D7） | 新增 |
| start | 异常 | invoke reject(string) → error 置串、running=false、可再次 start（错误如实呈现） | 新增 |
| start | 边界 | root=null → start 不发起 invoke（禁用语义） | 新增 |
| start | 边界 | 事件洪峰（300 条）全部累积不丢 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| @tauri-apps/api/core | vi.mock：invoke 按命令名分发可控行为；Channel mock 为可编程 class（捕获回调、测试内手动喂事件） | 全部用例 |

### packages/desktop/src/views/agent/hooks/useAgentRunHistory.ts -> packages/desktop/src/views/agent/hooks/useAgentRunHistory.test.ts

#### 待测功能

- useAgentRunHistory(): `AgentRunHistoryState = { runs, events, selectedRunId, loading, error, refresh, openRun }`；invoke `agent_runs`（显式刷新）+ `agent_run_events`（点开重放），沿 `useChangeDetail` 形态

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| refresh | 正向 | refresh() → invoke('agent_runs') → runs 更新（startedAt 降序原样承接） | 新增 |
| openRun | 正向 | openRun(id) → invoke('agent_run_events', { runId: id }) → events 更新、selectedRunId=id | 新增 |
| refresh / openRun | 异常 | invoke reject → error 置串、loading 复位（显式触发失败不静默） | 新增 |
| loading | 边界 | 取数进行中 loading=true、返回后 false | 新增 |
| 空态 | 边界 | agent_runs 返回 [] → runs=[] 不崩 | 新增 |
| 切换 run | 边界 | 连续 openRun 不同 id → events 以最后一次为准、selectedRunId 跟随 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| @tauri-apps/api/core | vi.mock：invoke 按命令名分发可控行为 | 全部用例 |

---

## 集成测试

<!-- 集成测试验证跨模块交互。单元测试通过 Mock 已覆盖模块依赖，此处聚焦模块组合时才暴露的行为。
  每项跨模块交互对应一个独立的 `### <关系标题> → <测试文件>` 章节，关系标题自由命名。 -->

<!-- Rust 侧集成用例与单元用例共置于同目录 `*_test.rs`（dev-team 为纯 binary crate 无库目标、泵/编排函数为 pub(crate) 不可经 tests/ 目录触达，沿 workspaces/mod_test.rs 既有共置先例，同时遵守「无 test-only 导出」规则）；前端集成用例落测试区域 `src/__tests__/`。 -->

### CLI stdout JSONL 泵 → core::agent AgentEvent 逻辑事件流 → `packages/desktop/src-tauri/crates/infra/agent/src/runner_test.rs`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `packages/desktop/src-tauri/crates/infra/agent/src/jsonl.rs` | 归一化生产方（行 → 事件口径） |
| `packages/desktop/src-tauri/crates/infra/agent/src/runner.rs` | 泵任务 / 逻辑事件流生产方（seq 赋值方，D8） |
| `packages/desktop/src-tauri/crates/core/agent/src/event.rs` | 信封契约消费方（五变体线格式） |
| `packages/desktop/src-tauri/crates/core/agent/src/runner.rs` | AgentRunner trait 契约（AgentRun 事件通道形状） |

**关联AC**: AC-1, AC-2

**关系描述**:

infra/agent 的泵任务把 CLI stdout 逐行产出的异构 JSONL 归一化为 core/agent 定义的 `AgentEvent` 信封，并发入有界 mpsc 形成逻辑事件流。两个 crate 的接缝处最容易出错：infra 产出的事件必须能被 core 的 serde 线格式无损表达（infra 不能私造 core 不认识的变体）；seq 由 infra 在归一化时赋值、必须对 core 与 store 的 `(run_id, seq)` 有序假设负责；「未知事件 Raw 透传、永不丢事件、永不炸解析」是跨层的兜底承诺，单独 mock 任何一侧都验证不了。出错模式：字段命名在 rustc 可过的前提下与 camelCase 线格式漂移、seq 在空白行/未知行上跳号、EOF 合成事件与正常 result 双双入场。

#### 场景: 完整会话 fixture 端到端归一化

前置条件为以合成 JSONL 会话（init → assistant(tool_use) → user(tool_result) → result）驱动泵任务（内存行流，不 spawn 真实进程），输入含未知 type 行、非 JSON 行与空白行；预期输出为事件序列全部符合 core 信封线格式、可被 core 反序列化、seq 从 0 单调递增且以 RunResult 收尾。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 合法会话 fixture 逐行入泵 → 事件按序送达且每条均可被 core::agent::AgentEvent 反序列化（两 crate 线格式一致） | 新增 |
| 边界 | 混入未知 type 与非 JSON 行 → Raw 占 seq 透传、零丢失（AC-1「未知事件透传不炸」× AC-2「未知 type 产出 Raw」的接缝验证） | 新增 |
| 边界 | seq 单调性跨归一化与通道全程成立（含空白行不占号，AC-2「seq 单调」在接缝处复核） | 新增 |
| 异常 | EOF 无 result → 合成 RunResult（error_process_exit / is_error=true / 退出码）补位收尾，事件流恒以 RunResult 终止（D6 单一收敛机制） | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| 真实 claude 进程 | 不 mock：以内存行流 fixture 驱动泵任务；真实 spawn 见不可测试项 4 | 全部用例 |

#### 场景: 有界通道背压不丢事件

前置条件为泵任务通道容量 256、输入 315 行洪峰 fixture；预期输出为消费端最终收到全量事件（含 EOF 收尾事件），生产端因背压阻塞而非丢弃。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 边界 | 超容量洪峰（315 > 256）→ 全量送达零丢失，通道无界增长不存在（D1 通道口径的跨模块复核） | 新增 |

---

### run_agent_with 编排 → tee 双路（Channel 实时 + store 落库）与状态收敛 → `packages/desktop/src-tauri/src/commands/exec/agent_test.rs`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `packages/desktop/src-tauri/src/commands/exec/agent.rs` | 编排方（tee 循环 + 双 sink 分发 + 收敛） |
| `packages/desktop/src-tauri/crates/core/agent/src/state.rs` | 状态收敛（RunResult 驱动） |
| `packages/desktop/src-tauri/crates/core/agent/src/runner.rs` | 事件源契约（假 runner 注入缝） |
| `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | 落库 sink（user 维度两表写入方） |

**关联AC**: AC-1, AC-3, AC-4

**关系描述**:

`run_agent_with` 是本变更编排密度最高的一点：从 runner trait 事件流消费事件，一路经 Tauri Channel 推给前端、一路以 `serde_json::Value` 逐事件追加进 store，同时把 RunResult 喂给 core 的状态机收敛 run 终态并整行替换落库记录。三个 sink 的一致性（两路 seq 一致、终态与最后落库行一致）只有模块组合时才可验证。出错模式：Channel 接收端消失（切页/关页）时误中断落库、store 写失败被静默吞掉（违反 D1「落库兜底失败不可静默」）、in-band 失败误返回 Err（违反 D7）、启动失败误落 run 行（违反 D5）。此关系是 AC-4 的主覆盖路由。

#### 场景: 假 runner 预录会话 tee 全链路

前置条件为 MockRuntime 下 manage 真实 Store（tempdir）、注入预录 completed 会话的假 runner；输入为预录事件序列（含 tool_use/tool_result 成对与 RunResult）；预期输出为 store 落全量事件与 completed 记录、Channel 逐事件一致、命令返回含汇总的最终记录。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 预录会话跑通 → Channel 事件序列与落库事件序列逐条一致（同 seq 同内容），返回 completed 记录（AC-4 tee 双 sink） | 新增 |
| 正向 | 事件流中 RunResult 驱动状态机收敛且终态写入落库行（AC-1 状态机经编排生效） | 新增 |
| 正向 | is_error result → Ok(failed 记录) 而非 Err（D7） | 新增 |
| 异常 | 假 runner start Err → Err(String) 且 store 零 run 行（D5「启动失败不留行」） | 新增 |
| 异常 | store 写入失败 → run 收敛 failed、tee 终止、Channel 已送达事件保留（D1） | 新增 |
| 边界 | Channel 接收端先关闭 → 落库完整、命令正常返回（D1「不中断落库」） | 新增 |
| 边界 | 预录 seq 缺口/乱序 → tee 透传不重排，落库 key 与事件自带 seq 一致（D8） | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| AgentRunner 事件源 | 假 runner 测试替身（预录事件 / 可控 Err）——内部模块替身，非跨进程 mock | 全部用例 |
| Tauri Channel | `tauri::ipc::Channel::new` 捕获；丢弃接收端构造页面已关 | tee / 关闭用例 |
| redb store | 不 mock：tempdir 真库；写失败以并行写事务独占构造 | store 写失败用例 |

---

### exec 查询命令面 → user 维度两表重放 → `packages/desktop/src-tauri/src/commands/exec/mod_test.rs`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `packages/desktop/src-tauri/src/commands/exec/mod.rs` | 查询命令面（薄包装 + Value→AgentEvent 反序列化） |
| `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | 读写方（两表写入与区间扫描） |
| `packages/desktop/src-tauri/crates/infra/store/src/model.rs` | 行编解码（AgentRunRecord JSON 进出） |

**关联AC**: AC-3, AC-4

**关系描述**:

`agent_runs` / `agent_run_events` 是前端历史重放的唯一后端入口，其价值在于把 store 的 `(run_id, seq)` 复合键扫描还原成前端可用的 `AgentEvent[]` 与 `AgentRunRecord[]`。薄包装本身无逻辑，但「store 落库形态 → Value → AgentEvent 反序列化」的链路上任何一处线格式漂移（camelCase 键、五变体判别）都会让重放静默失真，必须经真实落库后由命令面读回验证。出错模式：Value 直接透传未反序列化、重放顺序未按 seq、重开后数据不可读。

#### 场景: 落库后经命令面重放

前置条件为 MockRuntime manage 真实 Store（tempdir），经 store API begin/append/finish 写入两个 run（各含多事件，混入未知 type 的 Raw 事件）；预期输出为命令面读回与直连 store 一致、事件 seq 升序且可反序列化、重开后仍可读。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 两 run 各落多事件后：agent_run_events 按 seq 升序返回反序列化后的 AgentEvent（含 Raw 变体保真）（AC-3 重放 × AC-4 薄包装） | 新增 |
| 正向 | agent_runs 返回 startedAt 降序、finish 后终态可见、与直连 store serde 值一致 | 新增 |
| 边界 | 不存在 runId → 空数组；空库 → `[]` | 新增 |
| 边界 | drop 后重开同一 db → 命令面重放结果与重开前一致（AC-3「重开持久性」经命令面复核） | 新增 |
| 边界 | 两 run 事件隔离经命令面复核：以 runId=A 查询不串入 run B 的任何事件 | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| Tauri runtime | `tauri::test::mock_app()` manage 真实 Store（tempdir 真库，沿 workspaces/mod_test.rs 惯例） | 全部用例 |

---

### AgentRunForm 启动 → useAgentRun → agent_start 实时流 → AgentEventTimeline 呈现 → `packages/desktop/src/__tests__/agent_run_pipeline.test.tsx`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `packages/desktop/src/views/agent/components/AgentRunForm.tsx` | 触发方（AgentStartInput 组装） |
| `packages/desktop/src/views/agent/hooks/useAgentRun.ts` | IPC 适配（Channel 订阅 + invoke） |
| `packages/desktop/src/types/dto.ts` | 线格式契约（AgentEvent 判别 union 消费） |
| `packages/desktop/src/views/agent/components/AgentEventTimeline.tsx` | 呈现方（实时流渲染） |

**关联AC**: AC-5

**关系描述**:

页面「启动一次运行并实时观察 loop」是五段链路：表单组装 `AgentStartInput` → hook 建 Channel 并 invoke `agent_start` → 命令侧 tee 推送 → Channel 回调逐事件入 state → 时间线渲染。前端各组件单测各自 mock 掉邻接层，只有组合时才暴露的断裂面包括：invoke args 键名与后端 camelCase 契约漂移（`onEvent` / `permissionMode`）、AgentEvent 判别 union 的 `kind` 值与 Rust serde 线格式不一致导致时间线漏渲染变体、running 态与 result 收敛时序错位。出错模式：事件回调闭包过期（stale state）、reject 后 running 悬挂、failed 记录被当成功呈现。

#### 场景: 启动参数契约与实时 loop 呈现

前置条件为 mock IPC 装置（invoke 按命令名分发、可编程 Channel）、root 指向已选 workspace；输入为表单填写 prompt 后点击启动并经 mock Channel 喂入完整事件序列；预期输出为 invoke 参数逐字符合契约、时间线实时呈现对话流与成对折叠块、result 后汇总卡与终态呈现。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 表单启动 → invoke('agent_start', { onEvent: Channel 实例, root, prompt, env: 'default', permissionMode: 'bypassPermissions' }) 逐字段断言（默认值随链路上抛） | 新增 |
| 正向 | Channel 依次喂 runStarted → message(text) → message(toolUse) → message(toolResult) → runResult → 时间线实时呈现对话流、成对折叠块与汇总卡（AC-5 loop 可见性全链） | 新增 |
| 正向 | runResult is_error=false → result 置记录、running=false、表单解禁 | 新增 |
| 异常 | invoke reject(string) → error 呈现、running 复位、表单可重试 | 新增 |
| 异常 | runResult is_error=true（in-band 失败）→ 呈现 failed 终态而非成功汇总 | 新增 |
| 边界 | 子代理事件（parentToolUseId 有值）喂入 → 时间线按子代理归因分组呈现 | 新增 |
| 边界 | root=null 时点击启动 → 不发起 invoke | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| @tauri-apps/api/core | vi.mock：invoke 按命令名分发；Channel mock 为可编程 class（捕获回调、测试内手动喂事件）——真实 Tauri IPC 传输语义属库自带语义，见不可测试项 7 | 全部用例 |

---

### 历史重放链 agent_runs / agent_run_events → AgentRunHistory → AgentEventTimeline 还原 → `packages/desktop/src/__tests__/agent_run_pipeline.test.tsx`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `packages/desktop/src/views/agent/hooks/useAgentRunHistory.ts` | 查询适配（显式刷新 + 点开重放） |
| `packages/desktop/src/views/agent/components/AgentRunHistory.tsx` | 历史区呈现（列表与触发） |
| `packages/desktop/src/views/agent/components/AgentEventTimeline.tsx` | 重放呈现（与实时流共用组件，D11/D12 同源还原） |

**关联AC**: AC-5

**关系描述**:

历史重放要求「点开任一历史 run，时间线以与实时流相同的形态还原当时的 loop」——两段式取数（先 `agent_runs` 列表、再 `agent_run_events` 事件）经共用时间线组件还原，同源性是本关系的核心价值：重放与实时走同一个渲染面，任何只对重放路径成立的特殊分支都是坏味道。出错模式：重放事件顺序未按 seq、Raw 事件在重放路径渲染失真、刷新/点开的显式触发被自动取数污染（违反 D13「run 结束不自动刷新历史」）。

#### 场景: 两段式取数还原时间线

前置条件为 mock IPC 返回预置 runs 列表与某 run 的落库事件序列（含 Raw 事件）；输入为 refresh → 点开目标 run；预期输出为列表渲染三要素、点开后 invoke 携带正确 runId、时间线以与实时流同组件形态还原事件序列。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | refresh → invoke('agent_runs') → 列表呈现状态/时间/摘要；点开 → invoke('agent_run_events', { runId }) → 共用时间线还原（AC-5「历史运行经 invoke 重放」） | 新增 |
| 正向 | 重放序列含 Raw 事件 → 与实时路径同形态呈现（D12 同源） | 新增 |
| 异常 | agent_run_events reject → error 呈现、loading 复位、列表仍可用 | 新增 |
| 边界 | agent_runs 返回 [] → 空态、无 invoke('agent_run_events') 发生 | 新增 |
| 边界 | run 结束（result 到达）不自动触发 agent_runs（D13：历史刷新仅显式动作） | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| @tauri-apps/api/core | vi.mock：invoke 按命令名分发，返回预置 runs / events fixture | 全部用例 |

---

### 侧栏页面导航组 → App 顶层视图切换（无路由）→ `packages/desktop/src/__tests__/agent_page_nav.test.tsx`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `packages/desktop/src/components/AppSidebar.tsx` | 触发方（页面导航组，TopPage 判别） |
| `packages/desktop/src/App.tsx` | 视图切换（page state，D10 选中重置） |
| `packages/desktop/src/views/agent/AgentDebugView.tsx` | 目标页呈现 |

**关联AC**: AC-5

**关系描述**:

侧栏首次出现非 workspace 入口语义：页面导航组（变更/Agent 调试）与 workspace 清单组共存于同一 Sidebar，App 以顶层 `page` state 切换内容区且无路由。跨模块风险在于导航组回调与 App 状态的对接（`onPageChange` 携带的 TopPage 值与渲染分支一一对应）、以及切页对既有 change 域状态的副作用（D10 选中重置、`useChangeList` 不卸载）。出错模式：两入口语义串扰（点 Agent 调试误触 workspace touch）、切页后清单重取数（违反「查询显式触发」约束）。

#### 场景: 导航切换与 change 域状态共存

前置条件为既有 App.test.tsx 的 mock IPC 装置、有记录启动进入 changes 页；输入为点击导航组两项往返；预期输出为页面按 TopPage 精确切换、workspace 语义不被导航触发、change 域数据在切页往返后保留且选中按 D10 重置。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 点击「Agent 调试」→ AgentDebugView 呈现且 ChangeView 卸载；点击「变更」→ 切回清单（AC-5「侧栏页面导航组切换无路由」） | 新增 |
| 正向 | 导航点击不触发任何 workspace 命令（touch_workspace / list_workspaces 调用次数不变，两入口语义不串扰） | 新增 |
| 边界 | 进入 change 详情 → 切 Agent 页 → 切回 → 选中重置回清单、get_change_detail 不以旧选中重发（D10） | 新增 |
| 边界 | 切页往返不重发 list_workspaces（清单数据驻留 App 层，查询仍显式触发） | 新增 |
| 异常 | 无已选 workspace（root=null）进入 Agent 页 → 页面自呈现、启动禁用、App 不崩 | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| @tauri-apps/api/core invoke | 沿 App.test.tsx 既有装置：按命令名分发并记录调用序列（次数断言依赖记录） | 全部用例 |
| matchMedia / 窗口视口 | 沿既有 stubViewport 装置 | 全部用例 |

---

## 不可测试项

<!-- 列出 proposal 范围内但无法通过自动化测试验证的条目，每项说明原因。 -->

- AC-7 全部（`desktop-crate-layout` / `desktop-app-shell` / `desktop-data-dimensions` 三 spec delta 与实现一致） — **原因**: spec markdown 文本与实现的静态一致性无进程内断言面，由提案评审与 archive 合并把关（纯静态 AC 先例口径，见仓库既有 test-design 的 AC 处理）。
- AC-6 之「五条边界限制写进 spec 可考」 — **原因**: spec 文本静态约束，无运行时断言面；其余半边（无 `--resume` / `--include-partial-messages` / model 参数、无 kill/cancel 入口）已路由为 flags_test.rs 与 AgentDebugView.test.tsx 的负向/缺席断言。
- AC-4 之「`main.rs` 注册齐全」 — **原因**: `generate_handler!` 展开结果仅在真实 App 启动时可观测，`tauri::test::MockRuntime` 不经 main.rs 的 handler 表；三命令本身的可调用性与语义已由 mod_test.rs / agent_test.rs 直调覆盖。
- AC-2 / AC-4 之真实 claude CLI 端到端（真实 spawn → 真实 stdout 流 → 认证凭据下的完整 loop） — **原因**: 依赖本机 CLI 安装与 Anthropic 凭据，CI 不可复现且事件内容不可稳定断言；以 JSONL fixture 归一化（关系 1）与假 runner 编排（关系 2）等价承载除进程边界外的全部行为。
- AC-1 / AC-2 之 Cargo.toml 机械约束（两新 crate 无 tauri 系依赖、core/agent 无 workspace 内依赖、spawn 调用仅在 infra/agent） — **原因**: design「机械约束」节明确「实现须遵守，评审按此核对」，属清单/源码静态文本检查而非测试断言面。
- AC-8 之「client:check（vp check --fix + knip）通过」半边 — **原因**: 静态检查与死代码报告属管线命令职责，非测试执行；「rust 全套件 + 前端套件全绿」回归半边已路由为单元测试（验收范围 AC-8 行）。
- AC-5 之真实 Tauri IPC 传输语义（Channel 跨进程推送、invoke 序列化） — **原因**: `@tauri-apps/api` 与 tauri runtime 的库自带语义（本仓库口径：不逐项验证库语义）；前端以可编程 Channel mock、命令侧以 `Channel::new` 捕获等价驱动。
- 孤儿进程风险（无 kill 的 MVP 限制）与留存膨胀（无清理策略） — **原因**: 「不作为」类运行期风险，由 spec 已知限制留痕，无自动化断言面。

