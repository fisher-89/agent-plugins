# 测试设计: desktop-rust-ts-type-sync

> **日期**: 2026-09-28

---

## 验收范围

<!-- test_resolve_paths 解析 30 个 source -> test_file 对（errors 为空），本表逐 AC 路由其主承载测试文件；
     各消费文件章节的用例文本内标注 AC 关联（双向对齐）。 -->

| AC ID | 验收条件 | 被测文件或模块 |
|--------|---------|----------|
| AC-1 | serde JSON 序列化断言与枚举化前值域逐字一致（`"default"` / `"acceptEdits"` / `"running"` …）；`cargo test --workspace` 全绿（新增 `AgentRunStatus`；store 三字段 String → 枚举；`as_str()` 双轨退役） | packages/desktop/src-tauri/crates/core/agent/src/runner_test.rs |
| AC-2 | 含枚举化前记录的 fixture 库打开与重放成功，无手工迁移步骤（存量值域扫描 + native_model 版本演进） | packages/desktop/src-tauri/crates/infra/store/src/model_test.rs |
| AC-3 | 生成 bindings 文件入库，覆盖全部 22 条命令与全部出线 DTO；`agent_start` / `watch_subscribe` 的 Channel 参数 typed；PoC（`tag="kind"` + `rename_all_fields` 出线形态）留档 | packages/desktop/src-tauri/src/bindings_test.rs |
| AC-4 | check / build 前自动重导出；人为改 Rust 类型后不重导出提交，守卫以工作区 diff 报非干净 | packages/desktop/src-tauri/src/bindings_test.rs |
| AC-5 | `src/` 源码无裸 `invoke('command_name')` 字符串调用（生成物内部除外）的可自动化半边：10 个调用文件切 typed bindings 后线契约回归（前端一次性切换） | packages/desktop/src/lib/agent-transport.test.ts |
| AC-6 | 漏网旧 import 不炸编译的自动化半边：shim 化后既有 `from '../types/dto'` 导入编译与运行不受影响（dto.ts shim 化） | packages/desktop/src/lib/agent-transport.test.ts |

---

## 单元测试

<!-- 覆盖所有可在进程内验证的场景（Rust #[test] 共置 *_test.rs、前端 vite-plus 共置 *.test.ts(x)）。
     每个源文件对应一个独立的 `### <源文件> -> <测试文件>` 章节；跨模块组合用例挂靠链路入口模块的 `#### 用例` 表，
     不设独立集成测试章节。design.md 公共函数/API 表仅声明 bindings.rs / export-bindings.rs / runner.rs（as_str 删除）
     与 22 条命令汇聚行；derive-only 与机械替换类文件的待测功能取自 design.md「变更清单」「类型定义」表的对应行。 -->

### packages/desktop/src-tauri/crates/core/agent/src/event.rs -> packages/desktop/src-tauri/crates/core/agent/src/event_test.rs

<!-- design.md 类型定义表声明：AgentBlock / AgentEventKind / AgentEvent 加 specta::Type，serde attrs（tag /
     rename_all / rename_all_fields / flatten）逐字不动。derive-only 变更无运行时行为增量；不重复验证 specta
     derive 自身语义，线格式回归由既有 event_test.rs 套件全绿承载（AC-1 cargo test 全绿半边），出线同构性
     （PoC 判据）归 bindings.rs 章节的导出产物断言。本章节不设用例。 -->

#### 待测功能

<!-- design.md 公共函数/API 表无本文件行；类型定义为纯 derive 面，无新增可测函数。 -->

#### 用例

<!-- 不设用例：derive-only，既有套件即回归网。 -->

#### Mock策略

<!-- 不适用（不设用例；既有 event_test.rs 无进程边界依赖）。 -->

### packages/desktop/src-tauri/crates/core/agent/src/lib.rs -> packages/desktop/src-tauri/crates/core/agent/src/lib_test.rs

<!-- design.md 变更清单声明仅「导出 AgentRunStatus」（与既有两枚举同列的 pub use 面），无公共函数与运行时
     行为。不应创建 lib_test.rs：零行为声明无用例可写（见不可测试项 8），导出可见性由消费方（model.rs /
     flags.rs / commands）编译承载。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（仅导出声明）。 -->

#### 用例

<!-- 不设用例；不应创建该测试文件。 -->

#### Mock策略

<!-- 不适用。 -->

### packages/desktop/src-tauri/crates/core/agent/src/runner.rs -> packages/desktop/src-tauri/crates/core/agent/src/runner_test.rs

#### 待测功能

<!-- design.md 类型定义表 runner.rs 行；「公共函数/API」表声明两枚举 as_str 删除。 -->

- AgentRunStatus（新增枚举）: 四变体 Running / Completed / Failed / Stopped，serde rename_all camelCase 值域 running / completed / failed / stopped 与枚举化前 String 值域逐字一致；derive specta::Type
- AgentEnvMode: 加 specta::Type；serde 线格式不变；as_str() 双轨口径退役
- AgentPermissionMode: 加 specta::Type；serde 线格式不变；as_str() 双轨口径退役

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| AgentRunStatus serde 线格式 | 正向 | 四变体 serde JSON 序列化逐字为 "running" / "completed" / "failed" / "stopped"，反序列化 roundtrip 一致（与枚举化前 String 值域逐字一致，AC-1） | 新增 |
| AgentRunStatus 野值拒绝 | 异常 | "Running" / "run" / "succeeded" / "" 等清单外字符串反序列化返回 Err（值域受控） | 新增 |
| AgentEnvMode 线格式回归 | 边界 | 加 specta::Type 后两档 serde JSON 仍逐字 "default" / "bare" 且 roundtrip 一致（线格式零变化） | 新增 |
| AgentEnvMode as_str 双轨断言 | 废弃 | 既有「as_str() 与 serde 线格式一致」断言随 as_str 退役删除（落库双轨口径消灭，flag 组装改 serde 派生） | 废弃 |
| AgentPermissionMode 线格式回归 | 正向 | 加 specta::Type 后三档 serde JSON 仍逐字 "default" / "acceptEdits" / "bypassPermissions" 且 roundtrip 一致 | 新增 |
| AgentPermissionMode as_str 双轨断言 | 废弃 | 同上：三档 as_str 同值断言退役 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| —（无进程边界） | 纯类型 serde_json 内存序列化与 tokio mpsc 真实现；既有 FakeRunner 为 AgentRunner trait 测试替身经泛型入参注入（入参例外），本轮无新增 mock | 全部用例 |

### packages/desktop/src-tauri/crates/core/workflow/src/artifacts/envelope.rs -> packages/desktop/src-tauri/crates/core/workflow/src/artifacts/envelope_test.rs

<!-- design.md 类型定义表：ArtifactEnvelope / ArtifactDescriptor 加 specta::Type，语义不动（payload:
     serde_json::Value 出线口径随 PoC 留档）。derive-only 变更；该 colocated 测试文件现不存在，不应创建
     （见不可测试项 12）——线格式回归由既有 tests/artifacts_envelope_test.rs 目录级套件承载，出线口径归
     bindings.rs 导出断言。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（仅加 derive）。 -->

#### 用例

<!-- 不设用例；不应创建该测试文件。 -->

#### Mock策略

<!-- 不适用。 -->

### packages/desktop/src-tauri/crates/core/workflow/src/model/inventory.rs -> packages/desktop/src-tauri/crates/core/workflow/src/model/inventory_test.rs

<!-- design.md 类型定义表：Inventory 加 specta::Type（已是真枚举，仅加 derive，语义不动）。derive-only；
     该 colocated 测试文件现不存在，不应创建（见不可测试项 12）。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（仅加 derive）。 -->

#### 用例

<!-- 不设用例；不应创建该测试文件。 -->

#### Mock策略

<!-- 不适用。 -->

### packages/desktop/src-tauri/crates/core/workflow/src/model/workflow.rs -> packages/desktop/src-tauri/crates/core/workflow/src/model/workflow_test.rs

<!-- design.md 类型定义表：Verdict / FileLogOp / ChecklistItem / FileLogEntry / ActivePhase / InterruptedEntry
     加 specta::Type（已是真枚举/纯类型，仅加 derive，语义不动）；Verdict::as_str 不在退役范围（workflow 域
     自用）。derive-only；该 colocated 测试文件现不存在，不应创建（见不可测试项 12）。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（仅加 derive）。 -->

#### 用例

<!-- 不设用例；不应创建该测试文件。 -->

#### Mock策略

<!-- 不适用。 -->

### packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs -> packages/desktop/src-tauri/crates/core/workflow/src/queries/detail_test.rs

<!-- design.md 类型定义表：ChangeDetail / AttemptRecord / PhaseEntry 加 specta::Type，语义不动。derive-only，
     无运行时行为增量；既有 detail_test.rs 套件即线格式回归网（AC-1 cargo test 全绿半边），不重复验证
     specta derive 自身语义。本章节不设用例。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（仅加 derive）。 -->

#### 用例

<!-- 不设用例：derive-only，既有套件即回归网。 -->

#### Mock策略

<!-- 不适用。 -->

### packages/desktop/src-tauri/crates/core/workflow/src/queries/explore.rs -> packages/desktop/src-tauri/crates/core/workflow/src/queries/explore_test.rs

<!-- design.md 类型定义表：ExploreDoc / ExploreScanEntry 加 specta::Type，语义不动。derive-only；既有
     explore_test.rs 套件即回归网。本章节不设用例。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（仅加 derive）。 -->

#### 用例

<!-- 不设用例：derive-only，既有套件即回归网。 -->

#### Mock策略

<!-- 不适用。 -->

### packages/desktop/src-tauri/crates/core/workflow/src/queries/list.rs -> packages/desktop/src-tauri/crates/core/workflow/src/queries/list_test.rs

<!-- design.md 类型定义表：ChangeList / ChangeSummary / ArchiveGroup / ChangeSource 加 specta::Type，语义不动。
     derive-only；既有 list_test.rs 套件即回归网。本章节不设用例。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（仅加 derive）。 -->

#### 用例

<!-- 不设用例：derive-only，既有套件即回归网。 -->

#### Mock策略

<!-- 不适用。 -->

### packages/desktop/src-tauri/crates/infra/agent/src/flags.rs -> packages/desktop/src-tauri/crates/infra/agent/src/flags_test.rs

#### 待测功能

<!-- design.md 变更清单 flags.rs 行：--permission-mode flag 值改由 serde 序列化派生（serde_json::to_value 取
     字符串），替换 AgentPermissionMode::as_str 引用；flag 组装行为逐字不变。build_args 为该文件唯一模块级
     导出函数（既有），本轮改其取值来源。 -->

- build_args(): permission-mode 档位值改由 serde 序列化派生（单一来源定夺见 design 设计定夺表），flag 组装行为逐字不变（AcceptEdits 值仍为 "acceptEdits"）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| build_args permission-mode 派生值 | 正向 | 档位值改 serde 派生后 AcceptEdits 仍组装 --permission-mode acceptEdits、值串逐字 "acceptEdits"，组装序列与既有基线逐项相等（AC-1 行为不变回归锁定） | 新增 |
| build_args 三档全组合 | 边界 | Default（无 permission flag）/ AcceptEdits / BypassPermissions 三档组装结果与改动前基线逐项相等（复用既有 legacy_args 基线对照装置） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| —（无进程边界） | 纯函数；既有基线对照装置 legacy_args 为真实对照序列 | 全部用例 |

### packages/desktop/src-tauri/crates/infra/store/src/envelope.rs -> packages/desktop/src-tauri/crates/infra/store/src/envelope_test.rs

<!-- design.md 变更清单/类型定义表：ModelInfo / RecordEnvelope 加 specta::Type（db 命令出线信封）。derive-only，
     无运行时行为增量；RecordEnvelope.key/value 的 serde_json::Value 出线口径随 PoC 归 bindings.rs 导出断言。
     既有 envelope_test.rs 套件即回归网。本章节不设用例。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（仅加 derive）。 -->

#### 用例

<!-- 不设用例：derive-only，既有套件即回归网。 -->

#### Mock策略

<!-- 不适用。 -->

### packages/desktop/src-tauri/crates/infra/store/src/model.rs -> packages/desktop/src-tauri/crates/infra/store/src/model_test.rs

#### 待测功能

<!-- design.md 类型定义表 model.rs 行 + 数据模型节；store.rs 零变更（不入变更清单），故无独立章节——
     store_test.rs 的 v1→v2 演进断言跟改（String 直比 → 枚举断言）随「v2 存量 fixture」组合用例一并承接，
     归属本章节（链路入口为被测变更所在模块）。 -->

- AgentRunRecord: 三字段 env / permission_mode / status String → core/agent 枚举；serde camelCase JSON 线格式逐字不变；native_model version 2 → 3
- AgentRunRecordV2（新增）: 枚举化前 16 字段形态版本化结构（不注册 #[native_db]），承载 v2 载荷解码
- From&lt;AgentRunRecordV2&gt; for AgentRunRecord: v2 存量自动升级（野值 fail-fast panic 记因，design 定夺）
- From&lt;AgentRunRecordV1&gt; for AgentRunRecord（既有）: v1 → v2 → v3 链式自动升级，source 缺省 debug、source_ref / parent_run_id 为 None

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| AgentRunRecordV2 载荷升级 | 正向 | 三字段为合法值域的 v2 载荷经 native_model 读路径升级 v3：三字段转对应枚举变体、其余 13 字段逐字段保真、version 头为 3（AC-2） | 新增 |
| v1→v2→v3 链式升级（跟改前） | 正向 | 既有 v1 字节升级断言以 String 直比 upgraded.env / permission_mode / status 与 v1 同值——枚举化后该形态编译不过，断言退役 | 废弃 |
| v1→v2→v3 链式升级（跟改后） | 正向 | 同链路断言改枚举逐值：env == AgentEnvMode::Default 等 + source 缺省 debug、两新字段 None、十三字段保真（AC-2 无手工迁移） | 新增 |
| AgentRunRecord serde 线格式回归 | 边界 | v3 记录 serde JSON 序列化 env / permissionMode / status 为受控 camelCase 字符串，与枚举化前逐字一致（"default" / "bypassPermissions" / "running"，AC-1 线格式逐字不变） | 新增 |
| 三枚举全组合值域 | 边界 | env 2 × permission-mode 3 × status 4 全组合的 serde 串值域逐字断言（值域扫描结论的机械化回归面） | 新增 |
| 野值 fail-fast | 异常 | v2 载荷 status 含清单外字符串（如 "succeeded"）→ 升级转换 panic 且 panic 信息含原字符串值（design 定夺：fail-fast 不兜底变体；should_panic 形态） | 新增 |
| v2 存量 fixture → 库打开与重放自动升级 | 正向 | 以枚举化前形态记录构造的 fixture 库打开后 AgentRunRecord 读取与事件重放成功、无手工迁移步骤（跨模块组合用例：v2 载荷 → store 读路径；AC-2） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 文件系统（db 文件） | tempdir 真库真实组合（Store::open，同 store_test.rs 既有 temp_store 装置先例），不 mock store；fixture 升级走 native_model 真实读路径 | v2 存量 fixture → 库打开与重放 describe |
| —（其余用例） | native_model encode/decode 内存往返为真实实现，无进程边界 | 其余用例 |

### packages/desktop/src-tauri/src/bin/export-bindings.rs -> packages/desktop/src-tauri/src/bin/export-bindings_test.rs

<!-- design.md 公共函数/API 表仅声明 main()：导出 bin 入口（调用 bindings::export_bindings()）。bin 进程入口
     不可进程内调用，导出行为已由 bindings.rs 章节直接对 export_bindings() 断言。不应创建
     export-bindings_test.rs（见不可测试项 10）。 -->

#### 待测功能

- main(): 导出 bin 入口（转发 export_bindings()，design 公共函数/API 表）

#### 用例

<!-- 不设用例：进程入口无 in-process 断言面；不应创建该测试文件。 -->

#### Mock策略

<!-- 不适用。 -->

### packages/desktop/src-tauri/src/bindings.rs -> packages/desktop/src-tauri/src/bindings_test.rs

#### 待测功能

<!-- design.md 公共函数/API 表 bindings.rs 两行；PoC 前置门判据见 design「PoC 前置门」表。 -->

- builder(): 22 条命令全量注册的 specta Builder 组装（collect_commands!，迁自 generate_handler!）；main 与导出 bin 共用同一注册面；events 面不引入
- export_bindings(): 幂等导出 TS bindings 到 src/types/generated/bindings.ts（CARGO_MANIFEST_DIR 定位、父目录 create_dir_all、输出无时间戳/机器路径）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| export_bindings 覆盖性 | 正向 | 导出产物包含全部 22 条命令的 typed 包装名（与 generate_handler! 时代命令清单逐条一致）与全部出线 DTO 类型名（AC-3 覆盖性） | 新增 |
| Channel 参数 typed | 正向 | 产物中 agent_start / watch_subscribe 绑定签名为 typed Channel 参数，信封类型（AgentRunMessage tag ipc 双变体 / FileWatchEvent 载荷）来自生成物（AC-3） | 新增 |
| AgentEvent 出线形态（PoC 留档） | 正向 | AgentEvent 出线为 kind 判别的 discriminated union、seq / timestampMs 摊平、变体字面量 camelCase——与 dto.ts 现镜像同构（PoC 判据的自动化留档，AC-3） | 新增 |
| 特殊字段出线口径 | 边界 | serde_json::Value 字段（payload / usage / input / RecordEnvelope.key / value）与 OffsetDateTime 字段出线口径落产物留档（JsonValue / string），与 dto.ts 现 unknown / string \| null 的差异项可断言（纳入切换期清单） | 新增 |
| 导出幂等 | 边界 | 同输入连续两次导出，两次产物逐字节一致（无时间戳 / 绝对路径嵌入；AC-4 幂等判据、PoC 通过判据） | 新增 |
| 过期产物纠正 | 异常 | 预先篡改产物文件内容后重导出，文件恢复为权威内容（「入库物悄悄过期必被重导出暴露」的机械半边，AC-4） | 新增 |
| 目标目录缺失 | 边界 | 产物父目录不存在时 create_dir_all 先行、导出成功（CARGO_MANIFEST_DIR 定位路径健壮性） | 新增 |
| Result 错误通道留档 | 边界 | 产物中命令错误面形态（Result&lt;T, String&gt; → Promise reject）可断言，前端 hook error 态接法依据随之落档（随 PoC 口径） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 文件系统 | 导出目标重定向 tempdir（或对真实路径产物做内容快照比对，实现期随 export_bindings 路径注入形态定夺）；specta builder 与命令注册面真实组装、不 mock | 全部用例 |

### packages/desktop/src-tauri/src/commands/exec/agent.rs -> packages/desktop/src-tauri/src/commands/exec/agent_test.rs

#### 待测功能

<!-- design.md 变更清单 exec/agent.rs 行 + 类型定义表；公共函数/API 表的 22 条命令汇聚行覆盖本文件的
     agent_start（Rust 签名零变更，仅直写枚举与信封 derive 变化）。 -->

- AgentRunMessage: 加 specta::Type；tag ipc 双变体（event / record）信封 serde 形态不动
- running_record(): running 记录初值——env / permission_mode / status 直写枚举（as_str().to_owned() 与 STATUS_RUNNING 等四常量退役）
- drive_agent_run(): EOF 收敛——AgentRunState → AgentRunStatus 显式 match（STATUS_COMPLETED / FAILED / STOPPED 退役）
- abort_with_store_failure(): store 写失败收敛——status 直写 AgentRunStatus::Failed

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| drive 收敛 completed（跟改前） | 正向 | 既有 RunResult 驱动收敛断言以 STATUS_COMPLETED 字符串常量比较 record.status 与落库行——常量退役后该形态编译不过，断言跟改 | 废弃 |
| drive 收敛 completed（跟改后） | 正向 | 返回记录与落库行 status 均为 AgentRunStatus::Completed，Channel Record 信封 JSON 逐字 "completed"；汇总字段与 tee 双路断言保持（AC-1 枚举化跟改） | 新增 |
| drive in-band failed（跟改前） | 正向 | 既有 is_error result 收敛断言以 STATUS_FAILED 常量比较——同上退役 | 废弃 |
| drive in-band failed（跟改后） | 正向 | 收敛 AgentRunStatus::Failed + 落库行 failed（枚举断言） | 新增 |
| drive EOF stopped（跟改前） | 边界 | 既有停止信号收敛断言以 STATUS_STOPPED 常量比较 record.status 与信封 JSON——退役 | 废弃 |
| drive EOF stopped（跟改后） | 边界 | 收敛 AgentRunStatus::Stopped、error 为 None、信封出线值逐字 "stopped"（用户主动终止非失败语义不变） | 新增 |
| drive 兜底 failed（跟改前） | 异常 | 既有无 result 无 stop 的 EOF 断言以 STATUS_FAILED 常量比较——退役 | 废弃 |
| drive 兜底 failed（跟改后） | 异常 | 收敛 AgentRunStatus::Failed 且 error 记因文案逐字不变 | 新增 |
| running_record 初值 | 正向 | 初值三字段直写枚举：env == AgentEnvMode::Default、permission_mode == 入参档位变体、status == AgentRunStatus::Running（原 as_str().to_owned() + STATUS_RUNNING 组装退役；提前 resolve 返回的 running 记录随之枚举化） | 新增 |
| abort_with_store_failure（跟改前） | 异常 | 既有收敛助手断言以 STATUS_FAILED / STATUS_RUNNING 常量比较——退役 | 废弃 |
| abort_with_store_failure（跟改后） | 异常 | 收敛 AgentRunStatus::Failed、error 记因逐字、终态行尽力落库（枚举断言） | 新增 |
| AgentRunMessage 线格式回归 | 边界 | 加 specta::Type 后信封 serde JSON 形态逐字不变：{"ipc":"event","event":…} / {"ipc":"record","record":…} 双变体 camelCase（tee 双路对读断言的前提不变式） | 新增 |

#### Mock策略

<!-- 既有装置全部沿用（真实组合优先，替身仅两类白名单）： -->

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 运行环境（AppHandle） | tauri::test::mock_app()（MockRuntime）托管 Store 与 RunStopRegistry；编排对 runtime 泛型，生产解析 Wry | spawn 链路端到端用例 |
| IPC（Channel） | tauri::ipc::Channel::new 真实构造：捕获型（逐信封收 JSON 快照）与恒失败型（页面已关），不 mock | tee 双路 / EOF 收敛用例 |
| AgentRunner trait | FakeRunner 假实现经 start_agent_run_with 泛型缝注入（被测 API 显式入参，入参例外允许替身） | 全部编排用例 |
| 文件系统（db） | tempdir 真库（Store 真实组合，不 mock store） | 全部落库断言 |

### packages/desktop/src-tauri/src/commands/watch/mod.rs -> packages/desktop/src-tauri/src/commands/watch/mod_test.rs

<!-- design.md 类型定义表：FileWatchEvent 加 specta::Type；命令签名不动，桥接线程与幂等键逻辑不动。
     derive-only 变更无运行时行为增量；Channel&lt;FileWatchEvent&gt; 出线形态归 bindings.rs 导出断言（PoC 判据），
     线格式回归由既有 watch/mod_test.rs 套件全绿承载。本章节不设用例。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（仅加 derive；watch_subscribe / watch_unsubscribe 签名零变更）。 -->

#### 用例

<!-- 不设用例：derive-only，既有套件即回归网。 -->

#### Mock策略

<!-- 不适用。 -->

### packages/desktop/src-tauri/src/lib.rs -> packages/desktop/src-tauri/src/lib_test.rs

<!-- design.md 变更清单声明仅 pub mod bindings; + pub mod commands;（根包 lib 化入口，Tauri 2 上游模板形态），
     无公共函数与运行时行为。不应创建 lib_test.rs（见不可测试项 8）：组装正确性由 bindings.rs 用例与
     main / bin 编译承载。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（仅模块声明）。 -->

#### 用例

<!-- 不设用例；不应创建该测试文件。 -->

#### Mock策略

<!-- 不适用。 -->

### packages/desktop/src-tauri/src/main.rs -> packages/desktop/src-tauri/src/main_test.rs

<!-- design.md 变更清单声明：invoke_handler 从 generate_handler![...] 切换为 bindings::builder().invoke_handler()、
     mod commands 改从 lib 目标引用（setup / 插件挂载 / db 路径解析不动）。薄壳进程组装面，无 in-process
     可断言公共函数。不应创建 main_test.rs（见不可测试项 9）：命令注册正确性由 bindings.rs 的 builder /
     export 覆盖性断言承载。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（薄壳组装面）。 -->

#### 用例

<!-- 不设用例；不应创建该测试文件。 -->

#### Mock策略

<!-- 不适用。 -->

### packages/desktop/src/hooks/use-agent-chat.ts -> packages/desktop/src/hooks/use-agent-chat.test.ts

#### 待测功能

<!-- design.md 变更清单：agent_run_chain / agent_run_events / agent_stop 裸 invoke → typed bindings，机械替换。
     下述用例为「机械替换后线契约不变」的回归锁定（AC-5）；既有行为用例（重放装载、发送组装、停止、边界）
     经 @tauri-apps/api/core mock 机制全部存活，无需改写。 -->

- useAgentChat: agent_run_chain（链发起）/ agent_run_events（逐 run 重放）/ agent_stop（停止）三调用点切 typed bindings，行为不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| use-agent-chat → 生成绑定调用面 | 正向 | 经生成绑定入口发起后 invoke 收到 "agent_run_chain" 命令名与 camelCase 参数（root / prompt / permissionMode / resumeSessionId / parentRunId / source / sourceRef）逐字不变，返回 running 记录透传（AC-5 回归锁定） | 新增 |
| use-agent-chat → 生成绑定调用面 | 边界 | 逐 run 重放经生成绑定后 invoke 收到 "agent_run_events" 与 { runId }，空链 / 多 run 事件集透传与消息重建不变 | 新增 |
| use-agent-chat → 生成绑定调用面 | 异常 | 停止经生成绑定后 invoke 收到 "agent_stop" 与 { runId }，reject 路径 error 置位 / running 复位行为不变 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC（@tauri-apps/api/core） | vi.mock 该模块：invoke 为可编程 vi.fn（按命令名分发 / resolve / reject），Channel 为可编程 class（既有模式）；生成绑定底层仍走同模块 invoke，mock 机制切换后依旧生效 | 全部用例 |

### packages/desktop/src/hooks/use-change-list.ts -> packages/desktop/src/hooks/use-change-list.test.ts

#### 待测功能

<!-- design.md 变更清单：list_changes 裸 invoke → typed bindings，机械替换。 -->

- useChangeList: list_changes 调用点切 typed bindings，行为不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| use-change-list → 生成绑定调用面 | 正向 | 经生成绑定入口后 invoke 收到 "list_changes" 与 { root }，返回 ChangeList DTO 透传 data 更新（AC-5 回归锁定） | 新增 |
| use-change-list → 生成绑定调用面 | 边界 | root 为 null 时不发起任何调用（零 IPC 语义在绑定切换后保持） | 新增 |
| use-change-list → 生成绑定调用面 | 异常 | invoke reject → error 置位、data 保持原值、不抛未捕获异常（错误路径行为不变） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC（@tauri-apps/api/core） | vi.mock：invoke 可编程 vi.fn（既有模式，生成绑定底层仍走同模块） | 全部用例 |

### packages/desktop/src/hooks/use-workspaces.ts -> packages/desktop/src/hooks/use-workspaces.test.ts

#### 待测功能

<!-- design.md 变更清单：list_workspaces / add_workspace / remove_workspace 裸 invoke → typed bindings，机械替换。 -->

- useWorkspaces: list_workspaces / add_workspace / remove_workspace 三调用点切 typed bindings，行为不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| use-workspaces → 生成绑定调用面 | 正向 | 三命令经生成绑定入口后 invoke 命令名与参数（root 等）逐字不变，返回值透传（清单内部刷新 / root 切换 / remove 返回 true）（AC-5 回归锁定） | 新增 |
| use-workspaces → 生成绑定调用面 | 边界 | add 传入空字符串 root 参数原样穿透（把关责任仍在后端，绑定切换不引入前端改写） | 新增 |
| use-workspaces → 生成绑定调用面 | 异常 | add / remove reject → toast 固定前缀 + 错误串、返回 null / false 的错误双轨行为不变 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC（@tauri-apps/api/core） | vi.mock：invoke 可编程 vi.fn（既有模式） | 全部用例 |

### packages/desktop/src/types/dto.ts -> packages/desktop/src/types/dto.test.ts

<!-- design.md 变更清单：dto.ts 收敛为纯 re-export shim（export type * from './generated/bindings' + PoC 名单内
     逐名映射），无任何手写 interface / type 残留——纯类型模块，无运行时行为。不应创建 dto.test.ts（见
     不可测试项 7）：类型正确性由消费方用例（agent-transport.test.ts 等）编译期承载，零用例套件会令
     vitest 报 No test suite found 红灯。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（纯 re-export shim，无运行时行为）。 -->

#### 用例

<!-- 不设用例；不应创建该测试文件。 -->

#### Mock策略

<!-- 不适用。 -->

### packages/desktop/src/types/generated/bindings.ts -> packages/desktop/src/types/generated/bindings.test.ts

<!-- 生成物（specta-typescript 产物，knip ignore 目录），非手写代码：不应创建 bindings.test.ts（见不可测试项 6）。
     产物正确性由 bindings.rs 章节的导出断言（覆盖性 / 幂等 / 出线形态）与消费方编译期校验承载。 -->

#### 待测功能

<!-- 生成物，无 design.md 公共 API 声明（导出面归 bindings.rs）。 -->

#### 用例

<!-- 不设用例；不应创建该测试文件。 -->

#### Mock策略

<!-- 不适用。 -->

### packages/desktop/src/lib/agent-transport.ts -> packages/desktop/src/lib/agent-transport.test.ts

#### 待测功能

<!-- design.md 变更清单 agent-transport.ts 行；AC-5 / AC-6 的主承载章节（验收范围路由）。 -->

- TauriAgentTransport.sendMessages(): 裸 invoke('agent_start') 切生成绑定入口（命令名 / 参数 key camelCase / 返回类型转编译期校验）；body 原样穿透，readChainParams / readPermissionMode / readNullableString / readNullableNumber 运行时校验原样保留
- TauriAgentTransport 信封翻译: 手写 AgentRunMessage / AgentStartChainParams 镜像删除；new Channel&lt;AgentRunMessage&gt;() 保留（@tauri-apps/api 底层构造），类型来自生成绑定

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| transport → 生成绑定调用面 | 正向 | 经生成绑定入口发起后 invoke 收到 "agent_start"、链参数 7 字段 camelCase key 与值逐字不变且 body 原样穿透（AC-5 回归锁定） | 新增 |
| transport → 生成绑定调用面 | 边界 | Channel&lt;AgentRunMessage&gt; 信封（ipc event / record 双变体）类型来自生成物——事件转 chunk 保序 / Record 收尾关流 / 未知信封忽略流不断全部不变 | 新增 |
| transport 错误通道接法 | 异常 | 生成绑定错误通道（Result&lt;T, String&gt; → reject）下启动失败路径：错误串透传、流以错误 reject、非 Error 原因包装行为不变（PoC 错误面接法的回归锁定） | 新增 |
| 手写镜像退役 | 废弃 | 本地 AgentRunMessage / AgentStartChainParams interface 及涉及镜像形状的编译期断言随镜像删除退役；测试 fixture 类型导入改自生成物（经 dto shim） | 废弃 |
| dto shim 兼容 | 正向 | 既有 from '../types/dto' 类型导入在 shim 化后编译与运行不受影响——recordRow fixture 与断言照常工作（AC-6「漏网旧 import 不炸」的自动化半边） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC（@tauri-apps/api/core） | vi.mock：invoke 可编程（resolve running 记录 / reject / 记录入参）、Channel 可编程 class 捕获 onmessage（既有模式；生成绑定底层仍走同模块，mock 机制切换后依旧生效） | 全部用例 |
| —（流消费） | 真实 ReadableStream + drain 消费，不 mock | 流断言 |

### packages/desktop/src/views/agent/hooks/use-agent-run-history.ts -> packages/desktop/src/views/agent/hooks/use-agent-run-history.test.ts

#### 待测功能

<!-- design.md 变更清单：agent_runs / agent_run_events 裸 invoke → typed bindings，机械替换。 -->

- useAgentRunHistory: agent_runs（清单）/ agent_run_events（点开重放）两调用点切 typed bindings，行为不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| use-agent-run-history → 生成绑定调用面 | 正向 | 经生成绑定入口后 invoke 收到 "agent_runs"（挂载恰一次）与 "agent_run_events" + { runId }，清单降序承接与事件透传不变（AC-5 回归锁定） | 新增 |
| use-agent-run-history → 生成绑定调用面 | 边界 | agent_runs 返回 [] → runs 为空数组不崩（绑定切换不改变空态） | 新增 |
| use-agent-run-history → 生成绑定调用面 | 异常 | refresh / openRun reject → error 置串、loading 复位、runs 清单不受影响（错误路径不变） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC（@tauri-apps/api/core） | vi.mock：invoke 可编程 vi.fn（既有模式） | 全部用例 |

### packages/desktop/src/views/changes/hooks/use-change-detail.ts -> packages/desktop/src/views/changes/hooks/use-change-detail.test.ts

#### 待测功能

<!-- design.md 变更清单：get_change_detail / read_artifact 裸 invoke → typed bindings，机械替换。 -->

- useChangeDetail: get_change_detail（详情）/ read_artifact（产物信封）两调用点切 typed bindings，行为不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| use-change-detail → 生成绑定调用面 | 正向 | 经生成绑定入口后 invoke 收到 "get_change_detail" 与 "read_artifact"，详情 + 产物信封同周期组装透传不变（AC-5 回归锁定） | 新增 |
| use-change-detail → 生成绑定调用面 | 边界 | detail 为 null / 产物清单空数组 → 零后续调用保持；read_artifact 返回 null 降级 Fallback 信封不变 | 新增 |
| use-change-detail → 生成绑定调用面 | 异常 | 单个 read_artifact reject → 该产物 Fallback 其余正常；get_change_detail reject → 错误态清空数据不变 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC（@tauri-apps/api/core） | vi.mock：invoke 可编程 vi.fn（既有模式） | 全部用例 |

### packages/desktop/src/views/db/hooks/use-db-inspector.ts -> packages/desktop/src/views/db/hooks/use-db-inspector.test.ts

#### 待测功能

<!-- design.md 变更清单：db_models / db_records 裸 invoke → typed bindings，机械替换。 -->

- useDbInspector: db_models（模型清单）/ db_records（分页记录）两调用点切 typed bindings，行为不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| use-db-inspector → 生成绑定调用面 | 正向 | 经生成绑定入口后 invoke 收到 "db_models" 与 "db_records" + { model, offset, limit }，分页参数与取数透传不变（AC-5 回归锁定） | 新增 |
| use-db-inspector → 生成绑定调用面 | 边界 | 未选中模型不取记录（零 IPC 纪律保持）、翻页 offset 步进参数不变 | 新增 |
| use-db-inspector → 生成绑定调用面 | 异常 | db_models / db_records reject → 双轨 inline 错误态置位、已选模型不丢失不变 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC（@tauri-apps/api/core） | vi.mock：invoke 可编程 vi.fn（既有模式） | 全部用例 |

### packages/desktop/src/views/explores/components/explore-create-dialog.tsx -> packages/desktop/src/views/explores/components/explore-create-dialog.test.tsx

<!-- design.md 变更清单：scan_explores / create_explore_record 两调用点机械替换；公共函数/API 表无本文件行，
     组件交互面无既有测试文件。机械替换无行为增量：调用契约由生成绑定覆盖性断言（bindings.rs 章节）与
     tsc 编译期校验承载。不应为本次替换新建 explore-create-dialog.test.tsx（见不可测试项 11）。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（调用点机械替换）。 -->

#### 用例

<!-- 不设用例；不应创建该测试文件。 -->

#### Mock策略

<!-- 不适用。 -->

### packages/desktop/src/views/explores/hooks/use-explore-doc.ts -> packages/desktop/src/views/explores/hooks/use-explore-doc.test.ts

#### 待测功能

<!-- design.md 变更清单：read_explore / explore_doc_path / watch_subscribe / watch_unsubscribe 裸 invoke →
     typed bindings；Channel&lt;FileWatchEvent&gt; 底层构造保留，类型来自生成物。 -->

- useExploreDoc: read_explore（文档拉取）/ explore_doc_path（订阅寻址）/ watch_subscribe / watch_unsubscribe（订阅生命周期）四调用点切 typed bindings，行为不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| use-explore-doc → 生成绑定调用面 | 正向 | 四命令经生成绑定入口后命令名与参数逐字不变；watch 信号仅按 { path } 处理（FileWatchEvent 生成类型形态，载荷注入仍只按信号处理）（AC-5 回归锁定） | 新增 |
| use-explore-doc → 生成绑定调用面 | 边界 | explore_doc_path 返回 null 不订阅、500ms 防抖合并重拉语义不变（信封载荷类型来自生成物） | 新增 |
| use-explore-doc → 生成绑定调用面 | 异常 | read_explore / watch_subscribe reject → 错误态与订阅生命周期解耦行为不变 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC（@tauri-apps/api/core） | vi.mock：invoke 可编程 vi.fn + Channel 可编程 class（既有模式；watch_subscribe 的 Channel 底层构造保留） | 全部用例 |

### packages/desktop/src/views/explores/hooks/use-explore-list.ts -> packages/desktop/src/views/explores/hooks/use-explore-list.test.ts

#### 待测功能

<!-- design.md 变更清单：list_explore_records / create_explore_record / rename_explore_record /
     delete_explore_record 裸 invoke → typed bindings，机械替换。 -->

- useExploreList: 清单 + create / rename / remove 动作四调用点切 typed bindings，行为不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| use-explore-list → 生成绑定调用面 | 正向 | 四命令经生成绑定入口后命令名与参数逐字不变，动作后以当前 root 重新取数语义不变（AC-5 回归锁定） | 新增 |
| use-explore-list → 生成绑定调用面 | 边界 | root 为 null 时四动作均 no-op（零 IPC 守卫在绑定切换后保持） | 新增 |
| use-explore-list → 生成绑定调用面 | 异常 | 动作 reject → error 置位、清单不被污染（清单保持原值）不变 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC（@tauri-apps/api/core） | vi.mock：invoke 可编程 vi.fn（既有模式） | 全部用例 |

---

## 不可测试项

<!-- proposal / design 范围内无法通过进程内自动化用例验证的条目，逐项给原因；「不建测试文件」声明亦列于此。 -->

1. `packages/desktop/package.json` / `packages/desktop/knip.json` / 4 个 `Cargo.toml` 的配置变更（`bindings:export` / `bindings:check` 脚本串接、knip ignore、三件套 `=` pin） — **原因**: 配置文件无进程内可断言行为（test_detect_frameworks 判 unknown）；脚本挂点与 ignore 生效由静态检查链自身运行验证，依赖 pin 由 cargo 解析验证，不属单元测试面。
2. AC-4 的守卫端到端场景（人为改 Rust 类型后不重导出提交 → `bindings:check` 以工作区 diff 报非干净） — **原因**: 需 worktree 级源码变更与 git 状态断言，超出进程内单元测试面；其机械半边（重导出覆盖过期产物、幂等导出）已由 bindings.rs 章节「过期产物纠正」「导出幂等」用例承载。
3. AC-2 的「存量三字段值域扫描确认无野值」子句 — **原因**: 对既有本地 db 的一次性实施期核查（结论回填 design），非可重复自动化用例；野值 fail-fast 与 fixture 自动升级的可自动化半边由 model.rs 章节承载。
4. AC-5 的「`src/` 无裸 invoke 字符串调用（生成物内部除外）」与「`pnpm check` 全绿」子句 — **原因**: 前者为源码静态属性（机械 grep 面），后者为静态检查链（tsc + knip + cargo）运行结果，均非进程内用例；10 个调用文件的线契约回归已由各消费文件章节的「生成绑定调用面」用例承载。
5. AC-6 的「shim 文件仅 re-export」与「knip 报告 shim 引用计数（零引用即触发删除）」子句 — **原因**: 前者为源码静态约束（无运行时行为），后者为 knip 静态分析输出；「漏网旧 import 不炸编译」的自动化半边由 agent-transport.test.ts 章节「dto shim 兼容」用例承载。
6. `packages/desktop/src/types/generated/bindings.test.ts` — **原因**: 不应创建。生成物（knip ignore 目录）非手写代码，纯类型与生成包装，正确性由 bindings.rs 导出断言与消费方编译期校验承载；vitest 对零用例文件直接报错，空套件会令测试套件红灯。
7. `packages/desktop/src/types/dto.test.ts` — **原因**: 不应创建。dto.ts 为纯 re-export shim（无运行时行为），类型正确性由消费方用例编译期覆盖；零用例套件会令 vitest 报 No test suite found 红灯。
8. `packages/desktop/src-tauri/src/lib_test.rs` — **原因**: 不应创建。lib.rs 仅 `pub mod` 声明（根包 lib 化入口），无公共函数与运行时行为；组装正确性由 bindings.rs 用例与 main / bin 编译承载。
9. `packages/desktop/src-tauri/src/main_test.rs` — **原因**: 不应创建。main 薄壳（invoke_handler 切 builder 注册）为进程组装面，无 in-process 可断言函数；命令注册正确性由 bindings.rs 的 builder / export 覆盖性断言承载。
10. `packages/desktop/src-tauri/src/bin/export-bindings_test.rs` — **原因**: 不应创建。bin 的 `main()` 仅转发 `export_bindings()`，进程入口不可进程内调用；导出行为已由 bindings.rs 直接对 `export_bindings()` 断言。
11. `packages/desktop/src/views/explores/components/explore-create-dialog.test.tsx` — **原因**: 不应为机械替换新建。组件交互面无既有测试且本次无行为增量；`scan_explores` / `create_explore_record` 调用契约由生成绑定覆盖性断言与 tsc 编译期校验承载。
12. `packages/desktop/src-tauri/crates/core/workflow/src/model/inventory_test.rs` / `model/workflow_test.rs` / `artifacts/envelope_test.rs`（colocated 解析路径） — **原因**: 不应创建。对应源文件仅加 `specta::Type` derive（零运行时行为增量，`Verdict::as_str` 不在退役范围），类型正确性由 bindings 导出与既有 `tests/` 目录级套件回归承载；零用例文件无价值。
13. `crates/infra/store/src/store_test.rs` 的演进断言跟改无独立章节 — **原因**: store.rs 零变更（不入变更清单，`test_resolve_paths` 不为其产出映射）；其 v1→v2 演进用例的 String 直比断言跟改随 model.rs 章节「v1→v2→v3 链式升级」与「v2 存量 fixture」用例一并承接，test-gen 按 store_test.rs 既有装置就近跟改即可。

