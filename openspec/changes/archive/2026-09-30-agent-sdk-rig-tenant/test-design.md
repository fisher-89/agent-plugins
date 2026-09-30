# 测试设计: agent-sdk-rig-tenant

> **日期**: 2026-09-30（v2 回溯修订：缺省引擎 `DEFAULT_ENGINE`（硬编码 Sdk/rig）、引擎选择 debug-only、hook 缺席透传）

---

## 验收范围

<!--
  逐条映射 proposal.md 验收标准的 10 个 AC，被测文件或模块为单值路由（承载该
  AC 主自动化半边的测试文件）；静态 grep / 真机端点 / spike 留痕类半边落
  「不可测试项」，并在对应行单元格以「（见不可测试项 N）」标注。组合用例挂靠
  链路入口模块的 per-file 章节（无独立集成层）。
  v2 回溯修订：proposal AC-3「不传 engine 的既有测试零改动通过」及变更实现列
  「缺省 cli」表述已随用户回溯失效（design「回溯修订差异表」#1/#2/#4/#5/#7；
  proposal/spec 文本同步留待评审确认）——本表保留 proposal 验收条件原文摘录并
  标注 v2 读法，断言一律按 v2 语义设计；断言 v1 缺省语义的既有测试修订归本
  test 阶段（见各章节「废弃」行）。
-->

| AC ID | 验收条件 | 被测文件或模块 |
|--------|---------|----------|
| AC-1 | `cargo test --workspace` 全绿；代码库无 `agent-cli` / `agent_cli` 残留（spec 历史叙述除外） | packages/desktop/src-tauri/crates/infra/agent/src/lib_test.rs |
| AC-2 | `crates/core/agent` 变更仅该变体及其 Display；grep 无 engine / rig / openai 字样 | packages/desktop/src-tauri/crates/core/agent/src/runner_test.rs |
| AC-3 | bindings 再生成含 engine 参数；不传 engine 的既有测试零改动通过；core 契约 grep 无 engine —— **v2 读法：缺省收敛 `DEFAULT_ENGINE`（硬编码 Sdk/rig），「缺省 cli」与「既有测试零改动通过」表述失效；断言 v1 缺省语义或 engine 入链前形状的既有断言修订归本阶段** | packages/desktop/src-tauri/src/commands/exec/mod_test.rs |
| AC-4 | normalize 纯函数 fixture 测试全绿且不打网络 | packages/desktop/src-tauri/crates/infra/agent/src/sdk/normalize_test.rs |
| AC-5 | 逐档断言测试全绿；拒绝产出 `SystemNotice{permission_denied}` + is_error ToolResult | packages/desktop/src-tauri/crates/infra/agent/src/sdk/policy_test.rs |
| AC-6 | `..\` 逃逸与符号链接绕过用例全部拦截（tempdir 密集测试全绿） | packages/desktop/src-tauri/crates/infra/agent/src/sdk/sandbox_test.rs |
| AC-7 | 转录 fixture 重建测试全绿；会话缺失返回 `AgentStartError` 中性变体（`Err` 抵达前端） | packages/desktop/src-tauri/crates/infra/agent/src/sdk/resume_test.rs |
| AC-8 | 假流缝测试：置位停止后泵任务终止、run 收敛 stopped、编排代码无引擎分支 | packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner_test.rs |
| AC-9 | 组件测试全绿；真机手填 `EngineConfig`（api_key / base_url / model）后 sdk 引擎跑通含工具调用的完整 loop，落库可重放、可停止，且无本地 claude CLI 环境可运行 | packages/desktop/src/views/agent/agent-debug-view.test.tsx |
| AC-10 | design 文档留痕实测入树体积（`default-features = false` + openai feature）与 API 形状结论，design 据实定稿 | —（见不可测试项 4） |

---

## 单元测试

<!--
  覆盖所有可在进程内验证的场景。框架识别结论（test_detect_frameworks）：
  Rust 侧套件注册于 packages/desktop/src-tauri 工作区根（rust 框架，新 crate
  经 workspace 套件自动覆盖，无需逐 crate 注册）；前端侧为 vite-plus
  （packages/desktop）。
  本版为 v2 回溯修订（design attempt 2）：引擎缺省语义由「缺省 cli」修订为
  「缺省收敛 DEFAULT_ENGINE（硬编码 Sdk/rig）」、引擎选择 UI debug-only、
  hook body 缺席透传不注入、调试页初始值改 sdk。上一版 test-design（v1，
  attempt 1）中断言 v1 缺省语义的行在本版对应章节以「废弃」行留痕并以新增行
  替代，不静默沿用；既有测试文件中携带 v1 语义或 engine 入链前形状的断言
  （agent-run-form.test.tsx 的 engine:'cli' 形状断言、use-agent-chat.test.ts
  与 agent-transport.test.ts 的 8-key 参数对象清单断言）同样以「废弃 → 新增」
  对修订，修订执行归本阶段 test-gen。
  跨模块组合用例挂靠链路入口模块（SdkRunner.start 为 SDK 引擎链路入口、
  agent_start 壳层命令为 IPC 链路入口），不设独立集成测试章节。
-->

### packages/desktop/src-tauri/crates/core/agent/src/runner.rs -> packages/desktop/src-tauri/crates/core/agent/src/runner_test.rs

#### 待测功能

- AgentStartError::ConfigMissing(): 新增中性变体（`ConfigMissing(String)`），Display 分支同步「配置缺失: {msg}」，中性命名（core 不出现 sdk / engine 字样）；承接 key 未配 / 模型缺失 / 会话缺失 / 非 SDK 会话成因

（既有 `AgentRunner` trait / `AgentEvent` / `AgentRunParams` / `RunHandle` 零变化——既有套件即回归载体，本节仅增新变体断言行。v2 未触碰 core。）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| config_missing 变体 | 正向 | Display 文案为「配置缺失: {msg}」且携带原因串（key 未配 / 模型缺失 / 会话缺失三类成因消息可区分），可直抵前端 | 新增 |
| config_missing 变体 | 异常 | msg 含中文 / 空格 / 换行 / emoji / 超长（>1000 字符）/ 空串等非规整载荷时 Display 原样携带不截断不 panic（空串时前缀「配置缺失: 」不变）；PartialEq 按载荷精确匹配 | 新增 |
| start_error 既有变体回归 | 边界 | `CliMissing` / `SpawnFailed` 既有两变体 Display 文案与匹配形态零变化（加法变体不回归既有断言） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 纯枚举 + Display 断言，无进程边界依赖，全部真实构造 | 全部用例 |

### packages/desktop/src-tauri/crates/infra/agent/src/lib.rs -> packages/desktop/src-tauri/crates/infra/agent/src/lib_test.rs

#### 待测功能

- EngineFacade.new(): 无 resume 注入的门面（CLI 路径零成本）
- EngineFacade.with_resume_transcript(): 携 store 转录 loader 的门面；CLI 引擎持有但不消费
- EngineFacade.runner_for(): 引擎构造唯一 match 点；返回形状 `Box<dyn AgentRunner>`（引擎具体类型不出门面）

（类型定义 `EngineKind`（serde/specta camelCase `"cli" | "sdk"`）与 `ResumeTranscript` 别名住本文件，线格式与注入形状随用例断言。v2 门面零修订——缺省裁决在壳层，`runner_for` 恒收显式 `EngineKind`。）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| EngineKind 线格式 | 正向 | serde 序列化逐字为 `"cli"` / `"sdk"` 且 roundtrip 一致；specta 收集面可出线（AC-3 引擎参数镜像前提） | 新增 |
| EngineKind 线格式 | 边界 | `"Cli"` / `"SDK"` / `"cli "` / 空串等清单外值反序列化一律 Err（值域受控，不静默兜底） | 新增 |
| EngineFacade.runner_for（cli 分发） | 正向 | runner_for(Cli, engine_cfg) 返回 `Box<dyn AgentRunner>` 且 start 走 CLI 租户：隔离 PATH 下 Err 为 CliMissing 文案，engine_cfg 不被消费（CLI 路径不读配置）；v2 起该路径仅显式选 cli（调试页显式可选项）才触达 | 新增 |
| EngineFacade.runner_for（sdk 分发） | 异常 | runner_for(Sdk, 空缺省 engine_cfg) 的 start 返回 Err(ConfigMissing) 且不产生任何事件（未手填显式失败；AC-2 变体的首个消费点）；v2 起该分发路径同时是壳层缺省收敛路径（`unwrap_or(DEFAULT_ENGINE)`），门面自身零缺省逻辑 | 新增 |
| EngineFacade.runner_for（分发纯度） | 边界 | 同一门面重复分发与多实例分发互不共享运行态；`Box<dyn AgentRunner>` trait object 形态可注入编排（引擎类型不出门面的编译期锚定） | 新增 |
| EngineFacade::new / with_resume_transcript | 正向 | new() 门面可发起 CLI 运行；with_resume_transcript(闭包 loader) 构造成功且 loader 为引擎中立数据——CLI 分发路径发起行为不受 loader 影响（持有不消费） | 新增 |
| 门面 re-export 对外形状 | 边界 | AC-1 平移后 crate 根对外形状不变：`discover` / `build_args` / `normalize_line` / `ClaudeCliRunner` / `EngineConfig` 经 crate 根可达（用例内 use 锚定，破坏即编译失败） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 运行环境（PATH） | std::env::set_var 将 PATH 隔离至空目录触发 CLI 不可发现，共享互斥锁串行化（沿 agent_test.rs 的 PATH_LOCK 先例） | runner_for（cli 分发） |
| 入参例外（ResumeTranscript loader） | 闭包替身：按 session_id 返回内存 Vec&lt;AgentEvent&gt; / None / Err 三形态 | with_resume_transcript 用例 |
| rig 流（经 SdkRunner） | 真实 SdkRunner + 空配置走启动校验路径即失败，不触 rig 网络 | runner_for（sdk 分发） |

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/config.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/config_test.rs

#### 待测功能

- EngineConfig::from_hardcoded_slot(): MVP 硬编码预留位构造（api_key / base_url / model 三件套；空缺省值——未手填时 SDK 启动以 ConfigMissing 显式失败）；与壳层 `DEFAULT_ENGINE` 常量同座位，后续换构造源时本函数消费面零改动

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| EngineConfig 硬编码位 | 正向 | from_hardcoded_slot() 返回三字段结构体，手填值（api_key / base_url / model）逐字段保真可读 | 新增 |
| EngineConfig 硬编码位 | 异常 | 未手填缺省形态三字段为空串——该形状是 SdkRunner start 以 ConfigMissing 失败的前提（空缺省形状锁定，防实现引入隐式缺省值；v2 起缺省收敛 DEFAULT_ENGINE 后该失败路径覆盖全部未传 engine 调用，含 explore 链） | 新增 |
| EngineConfig 纯函数性 | 边界 | 重复调用返回等值结构（无隐藏状态、无副作用）；字段含特殊字符（空格 / 中文 / emoji）时构造保真 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 纯结构体构造断言，无进程边界依赖 | 全部用例 |

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/policy.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/policy_test.rs

#### 待测功能

<!-- design.md 公共函数 / API 表未声明本文件条目（policy 为 sdk/ 内部纯决策表模块，函数面由实现定稿）；用例按 proposal AC-5 与 design「关键实现语义定稿」逐档设计，测试对象列为 describe 标题。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 权限三档决策表（Default 只读档） | 正向 | Default 档逐工具断言：read / grep / glob / ls 允许，write / edit 拒绝（AC-5 只读半边） | 新增 |
| 权限三档决策表（AcceptEdits 档） | 正向 | AcceptEdits 档：只读四工具 + write / edit 允许（+write/edit 档位语义） | 新增 |
| 权限三档决策表（BypassPermissions 档） | 正向 | BypassPermissions 档：六工具全放行；且默认档位为 BypassPermissions（与调试页既有默认一致） | 新增 |
| 权限三档决策表（bash 缺席） | 边界 | bash 工具名查询三档均拒绝——bash 不进 MVP 工具面，表中拒绝语义为将来引入 bash 时的预留档位（缺席断言锁定） | 新增 |
| 权限三档决策表（清单外工具名） | 边界 | 未知工具名 / 大小写变体（"READ"）/ 空串工具名 → 一律拒绝（清单外不静默放行） | 新增 |
| 拒绝流出形态 | 异常 | 拒绝决策的合成产物形状：`SystemNotice{subtype:"permission_denied"}` + 与 tool_use 同 id 的 is_error ToolResult（run 不中断的流出契约，AC-5 后半句） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 纯决策表（档位 × 工具名 → 允许/拒绝）逐格断言，无进程边界依赖 | 全部用例 |

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/sandbox.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/sandbox_test.rs

#### 待测功能

<!-- design.md 公共函数 / API 表未声明本文件条目（沙箱为 std 路径 API 纯函数模块）；用例按 proposal AC-6「canonicalize + workspace root 前缀校验，tempdir 密集测试」与参数类型边界映射设计。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 路径沙箱校验 | 正向 | root 内合法路径（相对 / 绝对 / 子目录嵌套）经 canonicalize + 前缀校验放行（tempdir 驱动） | 新增 |
| 路径沙箱校验（`..\` 逃逸） | 异常 | `root\..\outside` 相对逃逸与 root 外绝对路径 → 拦截（canonical 口径，非字符串前缀比对） | 新增 |
| 路径沙箱校验（符号链接绕过） | 异常 | root 内符号链接指向 root 外目标 → 解链后按真实目标拦截（tempdir 内建链；Windows 无特权符号链接场景备选 junction 目录） | 新增 |
| 路径沙箱校验（前缀边界） | 边界 | 恰等于 root 本身的路径放行；root 相似名兄弟目录（root-sibling）拦截；root 携尾分隔符入参行为锁定 | 新增 |
| 路径沙箱校验（不存在路径） | 边界 | canonicalize 目标缺失的路径 → 拦截或显式失败，不得静默放行（错误半边显式化） | 新增 |
| 路径沙箱校验（特殊形态入参） | 边界 | 空串路径 / 含中文空格 emoji 的路径 / 大小写变体（Windows 大小写不敏感 canonical 口径）行为逐一锁定 | 新增 |
| read/write/edit 全链一致性 | 正向 | 三类工具入参路径过同一校验函数：任一工具携 root 外路径同样拦截（全链单点，非逐工具副本） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 运行环境（文件系统） | tempfile tempdir 真实目录驱动（进程边界白名单以真实 FS 参与不 mock）；符号链接 / junction 以 std Windows FS API 在 tempdir 内构造 | 全部用例 |

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools_test.rs

#### 待测功能

<!-- design.md 公共函数 / API 表未声明本文件条目（六工具面为 sdk/ 内部模块）；用例按 design 架构组件表「read / grep / glob / ls / write / edit 的定义与执行体；bash MUST NOT 进 MVP」设计。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 六工具定义面 | 正向 | ToolDefinition 清单恰为 read / grep / glob / ls / write / edit 六工具，名称 / 描述 / JSON schema 入参形状齐全 | 新增 |
| 六工具定义面 | 边界 | bash MUST NOT 在面（缺席断言）；工具名清单外无多余条目 | 新增 |
| read 执行体 | 正向 | 读取 root 内文件返回内容（tempdir 驱动，含中文 / emoji 内容保真） | 新增 |
| read 执行体 | 异常 | 文件不存在 → is_error ToolResult；root 外路径 → 拒绝（沙箱链） | 新增 |
| grep 执行体 | 正向 | 行级子串匹配：多行文件多命中逐行返回 | 新增 |
| grep 执行体 | 边界 | 无命中返回空清单非错误；空文件零命中；匹配串含中文 / emoji / 正则元字符按子串字面语义 | 新增 |
| glob 执行体 | 正向 | glob 模式命中集返回（`*.rs` / `**/*.ts` 跨目录） | 新增 |
| glob 执行体 | 边界 | 无匹配返回空清单非错误；空模式行为锁定 | 新增 |
| ls 执行体 | 正向 | 目录列举返回条目清单 | 新增 |
| ls 执行体 | 边界 | 空目录返回空清单；目标非目录 → is_error | 新增 |
| write 执行体 | 正向 | root 内新文件创建写入 / 已有文件覆写替换 | 新增 |
| write 执行体 | 异常 | root 外写入 → 拦截（沙箱链；BypassPermissions 默认档下唯一护栏不旁路） | 新增 |
| edit 执行体 | 正向 | 精确串替换成功（含中文 / emoji 目标串），替换后内容落盘 | 新增 |
| edit 执行体 | 异常 | 目标串不存在 → is_error 失败且不落盘 | 新增 |
| edit 执行体 | 边界 | 旧串多处出现语义按实现定稿锁定（全替换或显式失败，二者取一留痕） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 运行环境（文件系统） | tempfile tempdir 真实目录驱动；glob 库以真实实现参与（workspace 直连依赖，不 mock） | 全部用例 |

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/normalize.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/normalize_test.rs

#### 待测功能

<!-- design.md 公共函数 / API 表未声明本文件条目（normalize 为 sdk/ 内部纯函数模块）；用例按 proposal AC-4 与 design「据实定稿」（reasoning → Thinking、未知 → Raw{sdk_stream}、seq 由调用方单调传入）设计。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| rig 流归一化（文本/思考） | 正向 | assistant 文本流项 → Message{assistant, Text 块}；`reasoning` 字段（serde rename reasoning_content）→ Thinking 块（spike 据实定稿映射，不缺席留痕） | 新增 |
| rig 流归一化（tool_calls） | 正向 | tool_calls 聚合项 → ToolUse 块（name / arguments / id 保真），与后续 ToolResult 同 id 成对口径（AC-4 成对半边） | 新增 |
| rig 流归一化（未知透传） | 异常 | 未知 rig 流项 / 非预期形态 → `Raw{event_type:"sdk_stream", raw_json}` 透传不丢不炸 | 新增 |
| rig 流归一化（空与块序） | 边界 | 空流项序列 → 零事件；纯 reasoning 轮 / 纯 tool_calls 轮（无文本）块序锁定 | 新增 |
| seq 盖戳 | 正向 | seq 由调用方单调传入（AgentEvent::stamp 语义），跨流项 0..n 无跳号（AC-4 seq 单调半边） | 新增 |
| 打网络约束 | 边界 | 全部 fixture 由 rig 归一化流项内存构造，断言无网络触达（AC-4「不打网络」） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 网络 / rig provider | 无网络：rig 归一化流项以内存 fixture 构造（rig 类型为注入数据非 mock，不触 provider client） | 全部用例 |

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/resume.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/resume_test.rs

#### 待测功能

<!-- design.md 公共函数 / API 表未声明本文件条目（resume 为 sdk/ 内部纯函数模块）；用例按 proposal AC-7 与 design「resume 语义定稿」（仅顶层、Raw 丢弃、子代理压平、sdk- 前缀归属校验）设计。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| store 转录重建 | 正向 | 顶层非 Raw 事件转录 → rig 对话历史消息序列：user / assistant Message 重建、ToolUse ↔ ToolResult 成对回灌、RunStarted 忽略 | 新增 |
| store 转录重建 | 边界 | Raw 事件丢弃；子代理事件压平（parent_tool_use_id 非 None 并入顶层流；保真度缺口已留痕 spec，不由实现隐式吸收） | 新增 |
| sdk- 前缀归属校验 | 异常 | session_id 非 `sdk-` 前缀 → 显式失败（Err，消息含「会话不存在或非 SDK 产出」语义；AC-7 显式启动失败半边） | 新增 |
| sdk- 前缀归属校验 | 边界 | `sdk-` 前缀但转录为空 / 仅 Raw / 仅 RunStarted（重建空历史）→ 视同会话缺失显式失败（design resume 语义定稿第 3 条） | 新增 |
| 重建保真 | 边界 | 千级事件长转录重建不丢消息不乱序（工具轮 user / assistant 交替序保持）；v2 起 explore 链默认产出 `sdk-` 会话，此前缀同时是 explore 链引擎归属的事实标记 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 入参例外（store 转录产物） | 转录输入以内存 Vec&lt;AgentEvent&gt; fixture 构造（store 转录产物同构形态；store 本体不引入不 mock） | 全部用例 |

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner_test.rs

#### 待测功能

<!-- design.md 公共函数 / API 表未声明本文件条目：`SdkRunner` 仅经 `Box<dyn AgentRunner>` 出门面、方法面不对外（design 公共函数/API 节明示）。本节为 SDK 引擎链路入口模块，按挂靠原则承载「假流缝 → loop → policy/sandbox/tools 回灌 → RunResult」跨模块组合用例（proposal 测试文件清单同将假流缝归本文件）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| SdkRunner.start → SDK loop 全链（无工具轮） | 正向 | 假流缝注入无 tool_calls 轮：RunStarted{model=engine_cfg.model, session_id=`sdk-` 前缀, tools=六工具名, mcp_servers:[]} → 文本/思考事件 → RunResult{is_error:false, num_turns=轮数, cost_usd:None} 收敛（AC-4 的 RunStarted 报 model/tools 与 cost_usd=None 半边 + `sdk-` 会话 id 数据模型） | 新增 |
| SdkRunner.start → SDK loop 全链（工具轮回灌） | 正向 | 含 tool_calls 轮：policy 允许 + 沙箱放行 → 工具执行 ToolResult 事件 → 回灌续轮 → RunResult{num_turns=2}（AC-5 / AC-6 允许半边全链） | 新增 |
| SdkRunner.start → SDK loop 全链（policy 拒绝） | 异常 | policy 拒绝的工具调用（如 Default 档 write）→ SystemNotice{permission_denied} + is_error ToolResult 回灌续轮不中断（AC-5 拒绝流出形态全链） | 新增 |
| SdkRunner.start → SDK loop 全链（沙箱拦截） | 异常 | 工具入参携 root 外路径 → SystemNotice{sandbox_denied} + is_error ToolResult（AC-6 拦截半边全链） | 新增 |
| SdkRunner.start → SDK loop 全链（API 错误） | 异常 | 假流注入 API 错误形态 → SystemNotice{api_retry / api_error}（开放枚举）+ RunResult{is_error:true}（运行内失败通道，与 CLI 租户「启动 / 运行内」失败分界一致） | 新增 |
| RunHandle 信号取消泵任务 | 正向 | 停止置位后泵任务终止、事件流以已产出事件收尾、不合成 RunResult（select 与 CLI pump_lines 同构，AC-8 停止半边） | 新增 |
| RunHandle 信号取消泵任务 | 边界 | 消费端关闭（receiver drop）泵自行退出不悬挂；停止晚于流 EOF 时按 EOF 语义收敛、无二次收敛 | 新增 |
| 启动校验（engine_cfg） | 异常 | 空缺省 engine_cfg → start 返回 Err(ConfigMissing) 且零事件（AC-2 变体的 key 未配成因）；v2 起 explore 链等未传 engine 调用在配置未就绪时均落此路径（显式失败不回退 CLI） | 新增 |
| 启动校验（resume） | 异常 | resume_session_id 非 `sdk-` 前缀 / loader 返回 None / loader 返回 Err / 重建空历史 → 均 Err(ConfigMissing) 且消息区分成因，不产生 run 行（AC-7 显式启动失败，`Err` 抵达前端） | 新增 |
| 有界事件通道 | 边界 | 事件洪峰经容量 256 有界 mpsc 背压全量送达零丢失（与 CLI 泵同策略） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 网络 / rig provider 流 | 假流缝注入：rig 归一化流以内存流替身驱动（进程/网络边界白名单；同 cli/runner_test 的 pump_lines 内存行流模式），不打网络 | SdkRunner.start → SDK loop 全链全部用例 |
| 入参例外（ResumeTranscript loader） | 闭包替身按 session_id 返回内存转录 / None / Err 三形态 | 启动校验（resume） |
| 运行环境（文件系统） | tempfile tempdir 真实目录供工具执行与沙箱校验 | 工具轮回灌 / 沙箱拦截用例 |

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop_test.rs

#### 待测功能

<!-- design.md 公共函数 / API 表未声明本文件条目（sdk/ 内部协作者，函数面随实现定稿）；其行为语义（轮循环 / 拒绝合成 / 工具执行回灌 / RunResult 填充 / 停止信号协同）全部经 sdk/runner.rs 节的假流缝组合用例承载（loop 以 rig 归一化流为唯一输入缝，属进程/网络边界可注入替身）。 -->

#### 用例

<!-- 不设用例、不建 loop_test.rs：proposal 测试文件清单将假流缝全链归 sdk/runner_test.rs；此处若另建空套件只会产生零用例文件（套件失败）或与 runner 节用例重复。详见不可测试项 6。 -->

#### Mock策略

<!-- 不适用（不建测试文件）。 -->

### packages/desktop/src-tauri/crates/infra/agent/src/cli/discover.rs -> packages/desktop/src-tauri/crates/infra/agent/src/cli/discover_test.rs

#### 待测功能

<!-- design.md 公共函数 / API 表未声明本文件条目：discover 自 src/ 平移至 src/cli/（移动非删除，内容零改动），无公共 API 变更。 -->

#### 用例

<!-- 不设新用例：既有 discover_test.rs 随目录平移承载 AC-1 回归半边（workspace 套件内的既有断言原样保留）；本变更对该文件零行为改动，另立新用例只会复述既有套件。 -->

#### Mock策略

<!-- 沿既有平移套件内策略，本变更不引入新 mock。 -->

### packages/desktop/src-tauri/crates/infra/agent/src/cli/flags.rs -> packages/desktop/src-tauri/crates/infra/agent/src/cli/flags_test.rs

#### 待测功能

<!-- design.md 公共函数 / API 表未声明本文件条目：flags 自 src/ 平移至 src/cli/（移动非删除，内容零改动），无公共 API 变更；CLI resume 仍走 --resume flag 组装（SDK 不消费 flag 面）。 -->

#### 用例

<!-- 不设新用例：既有 flags_test.rs 随目录平移承载 AC-1 回归半边；零行为改动不虚构新用例。 -->

#### Mock策略

<!-- 沿既有平移套件内策略，本变更不引入新 mock。 -->

### packages/desktop/src-tauri/crates/infra/agent/src/cli/jsonl.rs -> packages/desktop/src-tauri/crates/infra/agent/src/cli/jsonl_test.rs

#### 待测功能

<!-- design.md 公共函数 / API 表未声明本文件条目：jsonl 自 src/ 平移至 src/cli/（移动非删除，内容零改动），无公共 API 变更；SDK 侧归一化由 sdk/normalize.rs 独立承载，不复用本模块。 -->

#### 用例

<!-- 不设新用例：既有 jsonl_test.rs 随目录平移承载 AC-1 回归半边；零行为改动不虚构新用例。 -->

#### Mock策略

<!-- 沿既有平移套件内策略，本变更不引入新 mock。 -->

### packages/desktop/src-tauri/crates/infra/agent/src/cli/runner.rs -> packages/desktop/src-tauri/crates/infra/agent/src/cli/runner_test.rs

#### 待测功能

<!-- design.md 公共函数 / API 表未声明本文件条目：runner（ClaudeCliRunner / pump_lines / 击杀缝）自 src/ 平移至 src/cli/（移动非删除，内容零改动），无公共 API 变更；SDK 泵以「与 pump_lines 同构」为设计约束（AC-8），其断言落在 sdk/runner.rs 节。 -->

#### 用例

<!-- 不设新用例：既有 runner_test.rs（含 pump_lines 停止路径装置）随目录平移承载 AC-1 回归半边，并为 sdk/runner 节的同构断言提供既有模式参照；零行为改动不虚构新用例。 -->

#### Mock策略

<!-- 沿既有平移套件内策略（duplex 内存行流 + 捕获型击杀缝），本变更不引入新 mock。 -->

### packages/desktop/src-tauri/crates/infra/agent/src/cli/mod.rs -> packages/desktop/src-tauri/crates/infra/agent/src/cli/mod_test.rs

#### 待测功能

<!-- design.md 公共函数 / API 表未声明本文件条目：cli/mod.rs 为模块声明与 re-export 文件，无运行时行为。 -->

#### 用例

<!-- 不设用例、不建 cli/mod_test.rs：re-export 正确性由 lib_test.rs 的门面对外形状锚定用例与消费方编译期覆盖。详见不可测试项 5。 -->

#### Mock策略

<!-- 不适用（不建测试文件）。 -->

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/mod.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/mod_test.rs

#### 待测功能

<!-- design.md 公共函数 / API 表未声明本文件条目：sdk/mod.rs 为模块声明文件，无运行时行为。 -->

#### 用例

<!-- 不设用例、不建 sdk/mod_test.rs：sdk 各子模块行为由各自共置测试承载。详见不可测试项 5。 -->

#### Mock策略

<!-- 不适用（不建测试文件）。 -->

### packages/desktop/src-tauri/src/commands/exec/mod.rs -> packages/desktop/src-tauri/src/commands/exec/mod_test.rs

#### 待测功能

- agent_start(): 发起 agent 运行命令——尾部可选参数 `engine: Option<EngineKind>`，**v2 缺省收敛 `engine.unwrap_or(DEFAULT_ENGINE)`（硬编码默认 agent，当前 Sdk/rig；v1 的 `unwrap_or(EngineKind::Cli)` 语义废弃）**，收敛后传入编排；core `AgentRunParams` 零污染（AC-3）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| agent_start engine 缺省收敛（v2 默认 agent） | 正向 | 不传 engine（None）→ `unwrap_or(DEFAULT_ENGINE)` 收敛 SDK 引擎：收敛结果与显式 Some(Sdk) 同路径同错误形态（硬编码位未手填时 Err(ConfigMissing)，启动校验即失败不触网络）；`DEFAULT_ENGINE` 常量恒等 `EngineKind::Sdk` | 新增 |
| agent_start engine 缺省 cli（v1 断言） | 异常 | —— v1 行「不传 engine → 走 CLI 租户薄入口、隔离 PATH 下 Err 为 CliMissing」随缺省语义修订（回溯差异表 #1）废弃，不沿用；替代断言见上行 | 废弃 |
| agent_start 显式 cli 路径 | 正向 | 显式 Some(Cli) → 走 CLI 租户薄入口：隔离 PATH 下 Err 为 CliMissing 文案；既有薄入口用例（已携显式 Cli 入参）零改动通过——显式 cli 为调试页可选项，不受缺省修订影响 | 新增 |
| agent_start 缺省与显式 cli 等价（v1 断言） | 异常 | —— v1 行「Some(Cli) 与 None 走同一路径同一错误形态」随 v2 分道（None→Sdk、Some(Cli)→Cli）废弃 | 废弃 |
| CLI参数 → 门面分发 → SDK 启动校验 | 异常 | None（缺省收敛）与 Some(Sdk) 双形态：Err(ConfigMissing) 抵达命令面、workspace 库零 run 行、Channel 零推送（跨模块组合：IPC 参数 → 壳层 unwrap_or(DEFAULT_ENGINE) 映射 → EngineFacade.runner_for → SdkRunner 启动校验；SDK 校验分支在启动即失败，不触网络） | 新增 |
| agent_start engine 参数面稳定性 | 边界 | engine 为尾部末位位置参数：既有 10 参调用形态经 IPC 反序列化不受影响（Option 缺席即 None，serde 缺省承接；IPC 签名 v2 零变化）；blank root 守卫先于引擎分发（四命令守卫口径不变） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 运行环境（PATH） | std::env::set_var 隔离 PATH 触发 CLI 不可发现，PATH_LOCK 互斥锁串行化（既有装置复用） | 显式 cli 路径用例 |
| 运行环境（Tauri 托管态） | tauri::test::mock_app（MockRuntime）托管 tempdir 真实 WorkspaceStores 与 RunStopRegistry；store 不 mock | 全部用例 |
| 入参例外（引擎分发缝） | SDK 分支走真实 EngineFacade + SdkRunner（空配置在启动校验即失败，不触 rig 网络） | 缺省收敛 / SDK 启动校验用例 |

### packages/desktop/src-tauri/src/commands/exec/agent.rs -> packages/desktop/src-tauri/src/commands/exec/agent_test.rs

#### 待测功能

<!-- design.md 公共函数 / API 表未声明本文件条目（`start_agent_run_with` 的 runner 参数改 `&dyn AgentRunner` 为 app 层内部编排缝、`DEFAULT_ENGINE` 为 pub(crate) 常量，design 明示不列公共 API）；用例按 design 修改文件行（门面组装、dyn 缝、find_events_by_session 辅助、ResumeTranscript loader 组装、v2 DEFAULT_ENGINE 硬编码位）设计。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| start_agent_run_with dyn 注入缝 | 正向 | `Box<dyn AgentRunner>` 经 `&dyn` 缝注入编排：tee 双 sink 落库 + Channel 逐事件一致 + 终态 Record 流出，行为与原泛型形态一致（AC-1 / AC-8 编排零改动证据） | 新增 |
| 编排引擎中立（含 stopped 收敛） | 边界 | 同一编排体分别驱动 CLI 形态与 SDK 形态假 runner（均经 dyn 缝）：completed / stopped / failed 三收敛分支语义不因引擎形态漂移（AC-8「编排代码无引擎分支」与「run 收敛 stopped」的编排半边结构性证据） | 新增 |
| DEFAULT_ENGINE 硬编码位（v2） | 边界 | 常量恒等 `EngineKind::Sdk` 且 `pub(crate)` 可自 mod.rs 消费点触达；与 `EngineConfig::from_hardcoded_slot()` 同点同注释成对（可配置化换源时单一改动点——消费面 `unwrap_or` 与门面签名冻结的编译期锚定） | 新增 |
| find_events_by_session 辅助 | 正向 | store 内命中 session_id 的 run → 返回其事件转录（store 既有 API 组合，无 schema / API 变化） | 新增 |
| find_events_by_session 辅助 | 异常 | 无命中（不存在 session / 非 workspace 库）→ None（loader None 语义 = 会话不存在 / 非 SDK 产出） | 新增 |
| ResumeTranscript loader 组装 | 正向 | tempdir 真库 seed 的 `sdk-` 前缀 run 事件经组装 loader 读回 Vec&lt;AgentEvent&gt; 转录；非 `sdk-` 前缀 session_id → loader 返回 None（`sdk-` 前缀校验落点；v2 起 explore 链 run 亦产 `sdk-` 前缀，loader 对其同样命中） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 运行环境（Tauri 托管态 + store） | mock_app 托管 tempdir 真实 WorkspaceStores（真库 seed）；store 不 mock（既有装置复用） | 全部用例 |
| 入参例外（runner 缝） | FakeRunner 测试替身经 dyn 缝注入（CLI 形态 / SDK 形态两种预录事件序列） | dyn 注入缝 / 引擎中立用例 |
| Channel 进程边界 | capturing_channel 捕获信封；failing_channel 模拟页面已关（既有装置复用） | tee 双 sink 断言 |

### packages/desktop/src-tauri/src/bindings/mod.rs -> packages/desktop/src-tauri/src/bindings/mod_test.rs

#### 待测功能

<!-- design.md 公共函数 / API 表未声明本文件条目（export_bindings 为既有函数，bindings/ 为 specta builder 组装非手改面）；本节承载产物快照断言更新行（来源：proposal 变更范围测试文件清单 + AC-3「bindings 再生成含 engine 参数」半边）。v2 零再生成（IPC 签名与 EngineKind 不变，v1 已再生产物维持）——产物断言面与本节计划一致，不因 v2 变化。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 导出产物覆盖性 | 正向 | 出线 DTO 类型清单含 `EngineKind`：产物含 `export type EngineKind =` 且字面量 `"cli"` / `"sdk"`（AC-3 类型镜像半边） | 新增 |
| agent_start 绑定形态 | 正向 | agentStart 包装签名含尾部 engine 入参（`EngineKind \| null`）且「agentStart typed Channel 首参」逐字断言保持（v1 已再生产物维持，v2 零再生成） | 新增 |
| 导出幂等 | 边界 | 同输入连续两次导出逐字节一致的既有用例零改动通过（产物含 engine 后仍确定性输出） | 新增 |
| 过期产物纠正与目录缺失 | 异常 | 篡改产物 / 删除生成目录两类异常态下重导出恢复权威内容的既有用例零改动通过（含 engine 的新权威产物） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 运行环境（产物文件系统） | 真实产物文件 + file_lock 互斥锁串行化 + Drop 守卫恢复（既有装置复用；共享单一产物为进程边界，以真实文件驱动不 mock） | 全部用例 |

### packages/desktop/src/views/agent/components/agent-run-form.tsx -> packages/desktop/src/views/agent/components/agent-run-form.test.tsx

#### 待测功能

<!-- design.md 公共函数 / API 表未声明 TS 组件条目；本文件为 design「清单外补入」级联文件（引擎下拉的实际渲染落点），类型定义表行 `AgentStartInput` 增 `engine: 'cli' | 'sdk'`（调试页表单恒显式传值）、参数面工具行增引擎二值下拉（data-testid="agent-engine"，复用 ModeSelect）。v2 修订：下拉初始值 `'sdk'`（与后端 DEFAULT_ENGINE 一致，避免「页面默认」与「后端默认」两个心智；cli 为显式可选项），ENGINE_OPTIONS 注释改写 debug-only 语义——引擎选择 UI 仅调试页暴露，正式场景无选择入口。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 引擎下拉初始值（v2 默认 agent 对齐） | 正向 | 初始 `agent-engine` 下拉值为 sdk（对齐既有 permission-mode 默认档断言形态；与后端 `DEFAULT_ENGINE` 一致；AC-9） | 新增 |
| 引擎下拉初始 cli（v1 断言） | 边界 | —— 既有测试两处 onStart 形状逐字断言 `engine: 'cli'`（初始值 v1 语义）随初始值修订失效废弃，替换为 `engine: 'sdk'` 形状断言（v1 test-design「初始 cli」行同批废弃） | 废弃 |
| 引擎下拉可选面 | 正向 | 下拉含 cli / sdk 两值且切换回填后，onStart 以 { prompt, permissionMode, engine } 恰调用一次——切至 cli 断言 engine:'cli'，初始态断言 engine:'sdk' | 新增 |
| 引擎下拉清单外值 | 异常 | 注入清单外 option 值（如 'yolo'）→ 不回填不触发 onChange（ModeSelect 清单守卫），onStart 不携带非法 engine | 新增 |
| onStart 形状断言更新 | 边界 | 既有 onStart toHaveBeenCalledWith 断言更新为 `engine: 'sdk'` 初始形状（AgentStartInput 增补后的断言修订行，替代 v1 两处 `engine: 'cli'` 逐字断言） | 新增 |
| 禁用与输入保真回归 | 边界 | disabled=true 时点击不触发 onStart；prompt 含换行 / emoji / 超长（>1000 字符）保真——既有保真用例的形状断言随初始值同步修订（engine 值 'cli'→'sdk'），交互行为零改动（v1 行「既有用例零改动通过」措辞随形状断言修订失效，废弃） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 纯回调组件：onStart 以 vi.fn() 注入，无进程边界依赖（既有口径不变） | 全部用例 |

### packages/desktop/src/views/agent/agent-debug-view.tsx -> packages/desktop/src/views/agent/agent-debug-view.test.tsx

#### 待测功能

<!-- design.md 公共函数 / API 表未声明 TS 组件条目；用例按 proposal AC-9（调试页引擎选择；sdk 发起运行复用时间线 / 落库 / 重放）设计：start 透传 input.engine 至 sendMessage，时间线 / 落库 / 重放组件零改动复用。v2 修订：初始 sdk / 缺省与后端默认一致；引擎选择仅调试页暴露，正式场景无入口。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 调试页引擎选择（v2 初始 sdk） | 正向 | 参数面呈现引擎二值下拉（agent-engine）且初始 sdk；不动下拉发起 → invoke 入参 engine 为 'sdk'（调试页缺省与后端默认一致；AC-9） | 新增 |
| 调试页引擎选择（v1 断言） | 边界 | —— v1 行「默认 cli；不动下拉发起 → invoke engine 'cli'」随初始值修订废弃；「选择 sdk 后发起 → engine 'sdk'」行被上行初始 sdk 断言吸收，一并废弃不沿用 | 废弃 |
| 调试页引擎选择（显式 cli） | 正向 | 切至 cli 后发起 → start 透传 input.engine 至 sendMessage，invoke 入参 engine 为 'cli'（AC-9 显式选 cli 的调试运行路径） | 新增 |
| sdk 运行复用既有呈现面 | 正向 | sdk 运行的 ToolUse / ToolResult 成对事件经既有时间线组件呈现、终态 record 回流（时间线 / 落库 / 重放组件零改动复用的回归证据） | 新增 |
| SDK 启动失败呈现 | 异常 | agent_start reject（ConfigMissing 错误串）→ run-error 横幅呈现错误串（`Err` 抵达前端的 UI 半边，AC-7 / AC-9） | 新增 |
| explore 链缺省安全 | 边界 | 页面既有用例（不涉 engine 入参断言的时间线 / 停止 / 滚动框架）零改动通过——初始值修订只改变 invoke 入参 engine 值，既有断言未锁定 start 入参 engine（proposal「不要修改」条款） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC 进程边界（@tauri-apps/api/core） | invoke 按命令名分发 mock（可切换 resolve / reject 并记录入参）+ Channel 可编程 class 捕获 onmessage（既有装置复用） | 全部用例 |
| 内部协作者（use-agent-chat 等） | 不 mock：fixture 经 mock invoke / Channel 流入真实 hook（最小 mock 原则既有口径） | 全部用例 |

### packages/desktop/src/hooks/use-agent-chat.ts -> packages/desktop/src/hooks/use-agent-chat.test.ts

#### 待测功能

<!-- design.md 公共函数 / API 表未声明 TS hook 条目；本文件为 design「清单外补入」级联文件：AgentChatSendInput 增可选 engine。v2 修订：body 组装 `engine: input.engine ?? null`（缺席透传不注入——缺省裁决权归后端）；v1 的 `?? 'cli'` 缺省注入语义废弃。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 发送组装 engine 穿透 | 正向 | sendMessage input.engine='sdk' → body.engine='sdk'（invoke 入参断言） | 新增 |
| 发送组装 engine 缺席透传（v2） | 正向 | input.engine 不传 → body.engine=null（`?? null` 缺席透传不注入——缺省裁决权归后端 `unwrap_or(DEFAULT_ENGINE)`；debug 链显式传、正式链缺席透传） | 新增 |
| 发送组装 engine 缺省注入 cli（v1 断言） | 异常 | —— v1 行「input.engine 不传 → body.engine='cli'（`?? 'cli'` 缺省注入）」随回溯差异表 #3 废弃，不沿用 | 废弃 |
| 发送组装 engine 异常值 | 异常 | input.engine 为类型面外的运行时异常值（如 'yolo'）→ hook 不做清单校验原样穿透，由 transport readEngine 层拒绝（职责分界留痕） | 新增 |
| explore 链缺省透传（v2） | 边界 | explore 形态（来源侧 use-explore-session 不传 engine）发送 body.engine=null → 后端缺省收敛 DEFAULT_ENGINE（Sdk）——explore 会话链运行时行为自 v2 切换到 SDK 引擎（回溯差异表 #5；design「explore 链 v1 行为与演进前一致走 CLI」表述失效） | 新增 |
| explore 链缺省安全恒 cli（v1 断言） | 边界 | —— v1 行「explore 形态 body.engine 恒 'cli'、行为与演进前一致」随回溯差异表 #5 废弃（v2 起 explore 链默认产出 `sdk-` 会话） | 废弃 |
| engine 与链参数互不串线 | 边界 | engine 增补后 resumeSessionId / parentRunId / 来源三元组组装不变（body 逐字段断言） | 新增 |
| 生成绑定调用面 key 清单（engine 入链） | 边界 | invoke 参数对象 key 清单修订为 9 个（链参数 8 + onEvent）：engine 键恒在（缺席值为 null）——transport 恒以第 9 位置参传 engine，生成绑定参数对象恒含该键 | 新增 |
| 既有 8-key 清单断言 | 边界 | —— 既有「恰 8 个 key（链参数 7 + onEvent）」逐字清单断言为 engine 入链前形状，engine 入链后必然失效，废弃（修订执行归本阶段 test-gen） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC 进程边界（@tauri-apps/api/core） | invoke / Channel mock（既有装置复用；适配层与 transport 真实组合，不 mock useChat） | 全部用例 |

### packages/desktop/src/lib/agent-transport.ts -> packages/desktop/src/lib/agent-transport.test.ts

#### 待测功能

<!-- design.md 公共函数 / API 表未声明 TS 模块条目；本文件为 design「清单外补入」级联文件：AgentStartChainParams 增 engine（生成绑定派生位置参数）、readChainParams 增 engine 读取（清单校验 `'cli' | 'sdk'`，缺席 → null）、invokeStart 追加第 9 位置参数。v2 代码零改动（仅 readEngine 文档注释更新为「null → 后端默认 agent（硬编码 SDK）」）；本节另承接既有断言修订（engine 键恒在的参数对象形状）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| readChainParams engine 读取 | 正向 | body.engine='sdk' / 'cli' → 读取透传至 invoke 第 9 位置参数（生成绑定派生位置参数面） | 新增 |
| readChainParams engine 缺席 | 正向 | body 无 engine → engine=null 传递不抛错（缺席校验为 null，与 permissionMode 必填口径刻意不同——引擎全链可缺席；null → 后端默认 agent，v2 注释语义） | 新增 |
| engine 清单校验 | 异常 | body.engine='yolo'（清单外值）→「非法 engine」参数校验拒绝且不发起 invoke（对齐既有 permissionMode 校验口径） | 新增 |
| invokeStart 位置参数追加（断言修订） | 边界 | 既有「恰 8 个 key（链参数 7 + onEvent）」清单断言与 `{...CHAIN_BODY, onEvent}` toEqual 断言修订：key 清单 8 → 9（+engine）、穿透断言增 engine:null——engine 之外链参数 7 字段 camelCase 逐字不变（穿透不增删改写回归） | 新增 |
| 既有 engine 入链前形状断言 | 边界 | —— 既有 8-key 清单与不含 engine 的 toEqual 断言为 engine 入链前形状，transport 恒传 engine 后必然失效，废弃（修订执行归本阶段 test-gen） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC 进程边界（@tauri-apps/api/core） | invoke / Channel mock（既有装置复用；ReadableStream 与 chunk 消费真实实现） | 全部用例 |

### packages/desktop/src/types/generated/bindings.ts -> packages/desktop/src/types/generated/bindings.test.ts

#### 待测功能

<!-- design.md 公共函数 / API 表未声明本文件条目：本文件为 export-bindings 再生成产物（非手改面，proposal 变更范围标注「非手改」）。v2 零变化（IPC 签名与 EngineKind 不变，v1 已再生产物维持）。 -->

#### 用例

<!-- 不设用例、不建 bindings.test.ts：生成产物的出线形态由 packages/desktop/src-tauri/src/bindings/mod_test.rs 产物快照断言覆盖（EngineKind 出线 / agentStart 签名），类型正确性由消费方（agent-transport.ts / use-agent-chat.ts / agent-run-form.tsx）用例与前端类型检查编译期覆盖。针对生成物另立用例会与产物快照断言重复且随再生成漂移。详见不可测试项 7。 -->

#### Mock策略

<!-- 不适用（不建测试文件）。 -->

---

## 不可测试项

- AC-1「代码库无 `agent-cli` / `agent_cli` 残留（spec 历史叙述除外）」 — **原因**: 代码库文本 grep 边界检查属静态守线检查，非进程内行为用例；其验收条件前半句的全仓库套件全绿回归半边由既有套件全集（含平移后的 cli/ 四套件）在 test-execution 阶段整体承载，不设独立用例。
- AC-2「`crates/core/agent` 变更仅该变体及其 Display；grep 无 engine / rig / openai 字样」与 AC-3「core 契约 grep 无 engine」及 `AgentRunParams` 无 engine 字段 — **原因**: 源码文本静态约束与契约形状不变式；`AgentRunParams` 字段面由 core runner_test 既有 `run_params` 用例的构造面隐式锁定（若增字段则既有结构体字面量构造编译失败），grep 半边不可自动化为行为断言。
- AC-3「不传 engine 的既有测试零改动通过」（proposal 原文，v2 已失效的表述） — **原因**: v2 缺省语义修订后该表述不成立：既有测试中断言 v1 缺省语义或 engine 入链前形状的断言（agent-run-form.test.tsx 的 `engine: 'cli'` 形状断言、use-agent-chat.test.ts 与 agent-transport.test.ts 的 8-key 参数对象清单断言）须随本变更修订——已在对应章节以「废弃 → 新增」对留痕，修订执行归本阶段 test-gen，不作为独立验收用例。
- AC-9「真机手填 `EngineConfig`（api_key / base_url / model）后 sdk 引擎跑通含工具调用的完整 loop，落库可重放、可停止，且无本地 claude CLI 环境可运行」 — **原因**: 需真实外部 LLM 端点（网络 + 机密双边界），进程内自动化套件不打网络（AC-4 同一口径）；由 test-execution 阶段真机验证 / 用户验收承接。其自动化前置半边已路由：硬编码位构造（config_test.rs）、启动校验失败路径（sdk/runner_test.rs、mod_test.rs）、假流缝全链（sdk/runner_test.rs）。附带真机半边「无本地 claude CLI 环境可运行」同理属环境级验证，自动化仅以隔离 PATH 的 CLI 缺席用例近似（显式 cli 路径 Err 文案），不覆盖 sdk 引擎真机全 loop。
- AC-10「rig spike 双关卡：依赖足迹实测 + API 形状实测（Tool trait / streaming / 自定义 base_url / reasoning_content）」 — **原因**: 属 design 阶段必过关卡，已在 design「rig-core spike 结论」节实测留痕并据实定稿（feature 形状、新增 12 crate 入树、`=` pin 0.42.0、reasoning → Thinking 映射），是 design 文档事实核对而非本变更代码行为；依赖收敛条目正确性经编译期验证，不设独立用例。
- `packages/desktop/src-tauri/crates/infra/agent/src/cli/mod.rs` 与 `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mod.rs`（解析路径 cli/mod_test.rs / sdk/mod_test.rs） — **原因**: 模块声明与 re-export 文件，无运行时行为；re-export 正确性由 lib_test.rs 门面对外形状锚定用例与消费方编译期覆盖。**不应创建**两个空 mod_test.rs（防零用例套件失败）。
- `packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs`（解析路径 sdk/loop_test.rs） — **原因**: design.md 公共函数 / API 表未声明该文件条目（sdk/ 内部协作者，函数面随实现定稿）；其行为语义（轮循环 / 拒绝合成 / 工具执行回灌 / RunResult 填充 / 停止协同）全部经 sdk/runner.rs 节的假流缝组合用例承载。**不应创建**独立 loop_test.rs 空套件。
- `packages/desktop/src/types/generated/bindings.ts`（解析路径 bindings.test.ts） — **原因**: export-bindings 再生成产物，非手改面；出线形态由 src/bindings/mod_test.rs 产物快照断言覆盖，类型正确性由消费方用例与编译期类型检查覆盖。不建测试文件。
- `packages/desktop/src-tauri/Cargo.toml` 与 `packages/desktop/src-tauri/crates/infra/agent/Cargo.toml`（test_resolve_paths errors：Not a testable source file） — **原因**: 构建清单非源模块；包名改名 / workspace 键切换 / rig-core 收敛条目（`=0.42.0` pin、default-features = false、features `["reqwest", "native-tls"]`）由编译期与 workspace 套件全绿间接验证。
