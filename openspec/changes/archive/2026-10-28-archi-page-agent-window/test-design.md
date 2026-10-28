# 测试设计: archi-page-agent-window

> **变更**: archi-page-agent-window
> **依据**: `proposal.md`（AC-1…AC-15、「变更范围」实现文件 / 测试文件子表、范围裁决 B / 静态骨架 + 代码同源派生 / 面板抽骨架本次即换 / 手动刷新必需 / 清单命令对称）、`design.md`（D1–D16、「新增文件」「修改文件」子表、「公共函数 / API」、「类型定义」、事实校准 1–4、待决问题 1–6）、`specs/`（8 份 delta）
> **范围**: design「变更清单」的**新增源码 6 条**与**修改文件 13 条**。删除文件无。生成物与版本行为无单测路径（见「不可测试项」）。
> **路径口径**: 全部路径为**项目根相对**。design 的 `crates/**` / `src-tauri/src/**` 简写对应 `packages/desktop/src-tauri/**`；前端一律写全 `packages/desktop/src/**`。

## 测试框架与既有布局

- **Rust**：`cargo test --workspace`；断言仅 `assert!` / `assert_eq!` / `assert_ne!` / `matches!`，无第三方断言库。测试文件布局为 **sidecar 同目录同前缀** `*_test.rs`，源文件尾以 `#[cfg(test)] mod <name>_test;` 挂载（`crates/infra/agent/src/lib.rs` / `sdk/mod.rs` / `crates/core/archi/src/lib.rs` / `crates/core/foundation/src/layout/mod.rs` 先例）；crate 级集成测试在 `tests/`（`crates/core/archi/tests/vocabulary_test.rs` 先例）。
- **既有装置（沿用，零新范式）**：`tempfile::TempDir` 真实临时目录；`tauri::test::MockRuntime` + 真实 `WorkspaceStores`（`src/commands/exec/mod_test.rs` 的 `Env` / `app_with_stores` / `workspace_db_count`）；`compose_test` 的记录型假 runner（捕获 `open_session` 的 `injections`）；`loop_test` 的可编程假流模型（`FakeModel::with_turns` / `captured_requests` / `rewrite_file_on_first_stream`）。
- **前端**：`vite-plus` 的 `vp test`（vitest 4，导入面 `vite-plus/test`；全局 jsdom）；断言用 `@testing-library/react` + `data-testid`（禁样式类名）；进程边界 mock 恒为 `vi.mock('@tauri-apps/api/core')` 的 `invoke`（+ 可编程 `Channel` 类）；hook 测试用 `renderHook` / `act` / `waitFor`；假计时器 `vi.useFakeTimers`。
- **命名**：Rust 测试函数为中文短语 + 断言消息载判据；前端 `describe` / `it` 中文短语 + testid 定位。

### 待测功能条目来源约定

`#### 待测功能` 只登记 design.md「公共函数 / API」子表中「所在文件」为该源文件的行（原文签名 + 出处标注「（公共函数 / API）」）。源文件在该子表**无行**时，保留章节框架 + HTML 注释说明原因，**不 grep 源文件导出声明、不虚构条目**；此时测试对象来源写在紧随的 `> 说明` 行（design「类型定义」/「修改文件」单元格 / D 编号 / AC），仍不由源码反推。

### 参数类型 → 边界覆盖口径

| 类型 | 覆盖要求 | 本变更加载点（按 API 入参 / 字段） |
|---|---|---|
| int / number | `0`、`-1`、MAX、`None` + 1 正常 | `offset` / `limit`（`list_archi_chat_sessions` 的 `u32`：`0` / `1` / `20` / `u32::MAX`；`-1` 属线面非法值，命令层不构造 → 以 `0` / 越界 `offset` 表达）、`total`、`updatedAt`（分页键）、`refreshTick` |
| str / string | `""`、超长（>1000 chars）、特殊字符（`\n` / `\0` / emoji）、`None` + 1 正常 | `root`（`""` / `"   "`）、`source` / `source_ref`（`"archi"` / `null` / 畸形四段 / 大小写变体）、`title` / `testIdPrefix`、`note` / `disabledReason`、`sessionId` / `prompt`、知识文本（`<= 4KB`）与 `AGENT.md`（`> 32KB` 截断边界） |
| bool | `true`、`false`、`None` | `Archi { module, search }` 两布尔位（4 组合）、`collapsed` / `onToggleCollapsed`（`undefined` = 不呈窄条）、`listOpen` / `listLoading` / `canStart` / `running` / `disabled` |
| list / array | `[]`、单元素、超大、`None` + 1 正常 | `sessions`（`[]` / 单条 / 21+ 条分页）、`SessionPanelState.session.{messages,events,chain}`（`[]`）、`ElementKind::ALL`(4) / `GateTier::ALL`(3) / `DiagnosticKind::ALL`(9) |
| dict / object | `{}`、缺失必填字段、多余字段、`None` + 1 正常 | `SessionPanelState`（缺 `note` / 多 `canSend` 属类型层违例，编译期红）、`SessionSummary`（`row` + `stats` + `turns`）、示例 TOML 的 `metadata`（`{}` / 内联键 / 子表误写） |
| Optional[T] | `None` | `source_ref`（`null`）、`cwd`（`null` → 回落 root）、`note` / `disabledReason`、`agent_md` / `preamble`（`Option` 四形态）、`collapsed` / `onToggleCollapsed`、`root`（`null` → 零 invoke） |
| Enum | 每枚举值 + 非法值（N+1） | `SessionToolset`（`Archi` / `Explore`）、`ElementKind`(4)、`GateTier`(3)、`DiagnosticKind`(9)、`AgentPermissionMode`（3 + 非法档位不构造） |
| float | `0.0`、`-0.0`、`NaN`、`Inf`、`None` | **不适用**：本变更无 float 入参 / 字段（`costUsd` 属既有面），显式声明不覆盖——不虚构 float 面 |

---

## 验收范围

| AC ID | 验收条件（缩写，原文见 proposal） | 被测文件或模块 |
|---|---|---|
| AC-1 | 注入臂：`source = "archi"` → `preamble = Some(..)` + `tools = None` + `toolset = Archi { module: true, search: true }`；`source_ref` 为 null / 非空 / 畸形四段一律同臂（不走相位解析、不 fail-closed） | `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs` |
| AC-2 | 零回归：explore 臂（六工具 + `Explore` + `preamble = None`）、change 臂 7 行矩阵与矩阵外 fail-closed、debug 缺省七工具面逐字不变 | `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs` |
| AC-3 | 知识同源派生：`kind` 四值 / `gate` 三值 + 缺省 `off` / 诊断九词 / 模型与决策目录相对路径全由 `ALL` 常量与 layout 派生取得；新增诊断词未同步 → 红 | `packages/desktop/src-tauri/crates/infra/agent/src/archi_knowledge.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/lib.rs`（模块挂载，见「不可测试项」）、`packages/desktop/src-tauri/crates/core/archi/src/dto.rs`、`packages/desktop/src-tauri/crates/core/foundation/src/layout/mod.rs` |
| AC-4 | 知识自证：内嵌示例 TOML 经 `archi` 解析零结构诊断；字段 / 内联 `metadata` / 键形任一漂移即红；零 `<>` 占位符、体量 ≤ 4KB | `packages/desktop/src-tauri/crates/infra/agent/src/archi-knowledge.md`、`packages/desktop/src-tauri/crates/infra/agent/src/archi_knowledge.rs` |
| AC-5 | 单条 System：双在场 → 恰一条（知识在前、`AGENT.md` 逐字在后）；仅 `AGENT.md` → 逐字；仅 preamble → 单条；皆无 → 零 System；每轮重读不变 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs` |
| AC-6 | 工具面与档位：archi 面 definitions = 七工具 + 两 archi 工具且与 `RunStarted.tools` 一致；`default` 档两 archi 工具放行、写面被拒；`acceptEdits` / `bypassPermissions` 写面放行 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs`（组合用例 + 既有 `sdk/tools.rs` / `sdk/policy.rs` 用例零改动回归） |
| AC-7 | 直改主仓通路：`source = "archi"` 且 `cwd = null` → 执行根回落 root；以 `write` 新建 `<archi_dir>/models/09-x.toml`、以 `edit` 改既有 `gate` 行成功；写入保护零命中；随后 `archi_module` 读到改动 | `packages/desktop/src-tauri/src/commands/exec/mod.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs`（内置锁面零改动由既有 `sdk/protect` 用例回归锚承载） |
| AC-8 | 写坏不 panic 且可自修：坏模型 → `archi_module` 结构化 `Err`（含 `file:line`）、run 不中断、零新增写前校验插口；用户点 `archi-refresh` 后画布呈同源诊断 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs`、`packages/desktop/src/views/architecture/architecture-view.tsx`（`StructureErrorPanel` 同源诊断由既有 `structure-error-panel` 用例回归承载） |
| AC-9 | 画布零放宽：`/architecture` 仍无任何可提交 `gate` 入口（`archi-gate-readonly` 在场、`archi-gate-select` / `archi-gate-submit` 缺席）；agent 写盘后画布不自动变化 | `packages/desktop/src/views/architecture/architecture-view.tsx` |
| AC-10 | 骨架归一：change chat 经 `SessionPanel` 渲染，`change-chat-*` testid 与文案逐字不变、675 行行为级用例原样通过；骨架无业务判据 | `packages/desktop/src/components/agent/session-panel.tsx`、`packages/desktop/src/components/agent/index.tsx`、`packages/desktop/src/views/changes/agent/change-chat-panel.tsx`、`packages/desktop/src/views/changes/agent/hooks/use-change-chat.ts` |
| AC-11 | 架构页归属：`source = "archi"` / `source_ref = null` / `cwd = null` 发起、落 workspace root 库；`list_archi_chat_sessions` 只返回该归属，change chat（两段）与流程会话（四段）一条不混；blank root → 空页 | `packages/desktop/src/views/architecture/agent/hooks/use-archi-chat.ts`、`packages/desktop/src-tauri/src/commands/exec/mod.rs`、`packages/desktop/src-tauri/src/commands/mod.rs` |
| AC-12 | 面板交互：空态可发（`sessionId: null` → 后端铸行）；「新会话」回空态零落库；历史点选以 `sessionId` 直查重放；运行中禁发（含新会话）而历史可用；状态行两态 | `packages/desktop/src/views/architecture/agent/hooks/use-archi-chat.ts`、`packages/desktop/src/views/architecture/agent/archi-chat-panel.tsx`、`packages/desktop/src/views/architecture/agent/archi-chat-composer.tsx` |
| AC-13 | 布局：左栏默认收起为窄条、页面头开关与窄条展开均可达；画布占右栏可全宽；`archi-refresh` 语义（`key` 重挂、恰一次 `archi_graph`）零变化；面板收起时零会话取数 | `packages/desktop/src/views/architecture/architecture-view.tsx`（壳层复核在既有 `packages/desktop/src/app.test.tsx`） |
| AC-14 | 路径字面量单点：产品 `.rs` 除 `foundation/src/layout/mod.rs` 外零 `openspec` / `architecture` 段 / `config.json` 字面量；知识文本目录路径全经派生注入 | `packages/desktop/src-tauri/crates/core/foundation/src/layout/mod.rs`（既有双禁令扫描回归）、`packages/desktop/src-tauri/crates/infra/agent/src/archi-knowledge.md`、`packages/desktop/src-tauri/crates/infra/agent/src/compose.rs`、`packages/desktop/src-tauri/src/commands/exec/mod.rs`、`packages/desktop/src/views/architecture/agent/hooks/use-archi-chat.ts` |
| AC-15 | 交付面：`bindings:check` 零 diff（`list_archi_chat_sessions` 在案）；version 0.6.1、`src-tauri/Cargo.toml` 版本行不随动；`vp check` / `knip` / `cargo clippy -D warnings` 通过 | `packages/desktop/src-tauri/src/commands/mod.rs`、`packages/desktop/src/types/generated/bindings.ts`、`packages/desktop/package.json` |

---

## 单元测试

### packages/desktop/src-tauri/crates/infra/agent/src/compose.rs -> packages/desktop/src-tauri/crates/infra/agent/src/compose_test.rs

#### 待测功能

design 未声明公共 API，本节无条目。

<!-- design.md「公共函数 / API」子表未声明本文件的公共 API 变更（`assemble_injections` 为模块私有；本变更新增 `ARCHI_SOURCE` 私有常量与一条臂）。 -->

> 说明：测试对象取自 design「修改文件」`compose.rs` ①②③④（`ARCHI_SOURCE` 常量、archi 臂、三臂 doc、臂序 explore → archi → change → default）、D1 / D15、`desktop-archi-agent`「SessionToolset::Archi 与 (phase, role) 注入矩阵」的 MODIFIED 段与 AC-1 / AC-2 / AC-14。既有 `injections_of(source, source_ref)` 装置（`super::assemble_injections` 直调）与 `DualEnv` / 记录型假 runner 沿用。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| archi 臂全字段 | 正向 | `source = "archi"`、`source_ref = None` → `preamble.is_some()`、`preamble == crate::archi_knowledge::knowledge()`（同一调用结果，逐字相等）、`tools == None`（七通用工具全量语义）、`toolset == Some(SessionToolset::Archi { module: true, search: true })`；返回 `Ok`（零 `Err` 路径） | 单例 |
| `source_ref` 形态无关（不碰相位解析） | 边界 | `source_ref` 七形态：`None` / `Some("")` / `Some("anything")` / `Some("chg-1")`（两段 change chat 形态） / `Some("chg-1/implement/executor/1")`（**合法**四段，change 臂会命中 `{module:true, search:false}`） / `Some("chg-1/implement/executor/not-a-number")`（畸形四段） / `Some("a/b/c/1/2")`（五段）→ 恒落 archi 臂（`preamble.is_some()` + `Archi { true, true }`），与 change 臂的可观察差异逐形态锁死 | 表驱动（7 形态） |
| 臂序（explore 在前） | 边界 | `source = "explore"` 且 `source_ref` 取上表任一形态 → 仍为 explore 臂（六工具 + `Explore` + `preamble == None`），archi 臂不抢占 | 表驱动（2 形态） |
| 臂序（archi 在 change 短路之前） | 异常 | `source = "archi"` 的三种变形 `source_ref`（`"chg-1/implement/executor/x"` 畸形四段 / `"chg-1/implement"` 短段 / `"a/b"`）不得落到 `SessionInjections::default()`（fail-closed 分支）——否则即「`source_ref` 变形丢工具」，逐形态断言 `toolset.is_some()` | 表驱动（3 形态） |
| `source` 词面精确 | 异常 | `"Archi"` / `"ARCHI"` / `" archi"` / `"archi "` / `"archi-chat"` / `""` → 非 archi 臂（走 change 短路 → `default()`：`preamble == None` 且 `toolset == None`）；受控词大小写敏感、零 trim 归一 | 表驱动（6 形态） |
| 降级范围单字段（D15） | 边界 | 臂内 `preamble` 位独立取 `knowledge()`：断言 `preamble` 与 `knowledge()` 逐字相等、而 `tools == None` 与 `toolset == Some(Archi{..})` 恒定（知识拼不出时只剩 `preamble` 缺席，工具面零连带失效——不得出现「preamble 无 ⇒ 整注入 default」） | 单例（逐字段） |
| explore 臂零回归 | 回归 | 六名 `["read","grep","glob","ls","edit","bash"]` 保序 + `toolset = Explore` + `preamble = None`（既有断言不改）；六名 ∉ `TOOL_NAMES` 的 `Err` 面保留 | 回归 |
| change 臂零回归 | 回归 | 7 行矩阵逐行命中位与既有逐字一致；矩阵外相位 / 畸形 `source_ref` / 短段恒 `default()`；`attempt` 段可解析性（`0` / `+1` / `u32::MAX` / `-1` / `1.0`）语义不变 | 回归（既有表驱动） |
| 其余来源零回归 | 回归 | `source = "debug"` / 未来源 / `source_ref = None` → `default()`（`preamble == None` / `tools == None` / `toolset == None`） | 回归（表驱动 2 形态） |
| 调用点链路 | 正向 | `compose_turn` 以 `SessionProvenance { source: "archi", source_ref: None }` 发起 → 记录型假 runner 捕获的 `TurnRequest.injections` 与 `assemble_injections` 直调结果逐字段全等（臂结果唯一来源，组合根零二次改写） | 单例 |
| `ARCHI_SOURCE` 字面量单点 | 边界 | 断言 `super::ARCHI_SOURCE == "archi"`（与命令层 `ARCHI_CHAT_SOURCE` / 前端 `use-archi-chat` 三处同值；跨语言一侧由前端用例的 invoke 参数承载，见 `use-archi-chat` 章节） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 装配纯函数 | **不 mock**：`SessionProvenance` 字面量直构（`super::assemble_injections` 直调），零 IO | 臂命中 / `source_ref` 形态 / 词面 / 降级 / 回归 |
| `WorkspaceStores` + 引擎门面 | **不 mock**：真实 temp 数据根 `DualEnv`（既有装置）+ 记录型假 runner（仅捕获 `injections`，不实现推理） | 调用点链路 |
| `archi_knowledge::knowledge` | **不 mock**：真实调用并与装配结果对拍（同源单点即被测语义） | archi 臂 / 降级 |
| 网络 / 时钟 / 环境变量 | **零依赖** | —— |

---

### packages/desktop/src-tauri/crates/infra/agent/src/archi_knowledge.rs -> packages/desktop/src-tauri/crates/infra/agent/src/archi_knowledge_test.rs

#### 待测功能

- `knowledge()`: 唯一入口 = `build(SKELETON).ok()`；`None` = 降级（compose 只丢 `preamble` 位），纯函数零 IO（公共函数 / API）
- `build(skeleton)`: 骨架校验（trim 空 → `Err("知识骨架为空")`；含 `<>` → `Err("知识骨架含占位符")`）+ 通过后追加代码派生段（`kind` 四值 / `gate` 三值 + 缺省 / 诊断九词 / 模型与决策目录相对路径）（公共函数 / API）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `knowledge` 词表派生（`kind`） | 正向 | 返回值 `Some`；`ElementKind::ALL` 四值逐值 `.as_str()` 在文本内**在场**（`package` / `domain` / `module` / `component`），且四值每个各出现（不是只出现一个词） | 表驱动（4 值） |
| `knowledge` 词表派生（`gate` + 缺省） | 正向 | `GateTier::ALL` 三值逐值在场（`off` / `warn` / `strict`）；`GateTier::default().as_str()`（`off`）与「缺省」语义措辞同行出现（新元素 `gate` 缺省 `off` 一节成立） | 表驱动（3 值 + 缺省锚） |
| `knowledge` 词表派生（诊断九词） | 正向 | `DiagnosticKind::ALL` 九值逐值 `.as_str()` 在场（`syntaxError` … `orphanHierarchy`）；缺失即点名缺失词（AC-3 的「新增诊断词未同步 → 红」由 `ALL` 驱动，测试侧不另抄字面量清单） | 表驱动（9 值） |
| `knowledge` 词汇漂移守卫 | 异常 | 断言 `ElementKind::ALL.len()==4` / `GateTier::ALL.len()==3` / `DiagnosticKind::ALL.len()==9`（封闭集加值即红），并断言三份 `ALL` 去重后长度不变（漏值 / 重复值不可绕过） | 表驱动（3 组长度 + 去重） |
| `knowledge` 目录派生注入 | 正向 | 文本含 `foundation::layout::archi_models_rel_dir()` 与 `archi_decisions_rel_dir()` 的**逐字返回值**（两次 `contains`），且文本内零 `\`（`/` 分隔）——目录串不自写 | 表驱动（2 路径） |
| `knowledge` 体量与占位符 | 边界 | `knowledge().unwrap().len() <= 4096`（AC-4 的 4KB 上限）；全文零 `<>` 连续两字符 | 单例（两条 `assert`） |
| `knowledge` 纯函数性 | 边界 | 连续三次调用结果逐字相等（零 IO、零时间 / 环境依赖）；`knowledge()` 与 `build(<真实骨架>)` 结果逐字相等（唯一装配面，禁第二份拼装） | 单例 |
| `build` 骨架顺序 | 正向 | 派生段在骨架之后：`kind` 词首次出现位置 > 骨架首条静态条款位置；骨架与派生段之间恰以 `\n\n` 分隔（不重复空行堆叠） | 单例（位置比较） |
| `build` 尾随空白归一 | 边界 | 骨架尾带多个 `\n` / 空格 → `Ok`，结果不以连续空行结尾（`trim_end()` 生效；派生段仍完整在场） | 表驱动（3 形态空白） |
| `build` 空骨架 | 异常 | `build("")` → `Err`，文案含「知识骨架为空」；`build("   \n\t ")` → 同 `Err`（trim 后为空即拒） | 表驱动（2 形态） |
| `build` 占位符 | 异常 | `build("框架 <>\n")` → `Err`，文案含「知识骨架含占位符」（带触发点）；`build("<archi_dir>")`（无连续 `<>`）→ `Ok`（判据 = 连续两字符 `<>`，`<...>` 单尖括号形态不触发；该形态的禁令由骨架章节用例承载） | 表驱动（2 形态） |
| 降级语义（`.ok()`） | 异常 | `build("").ok() == None`、`build("<>").ok() == None`（compose 侧只丢 `preamble` 位，run 不 `Err`——与 compose 章节「降级范围单字段」交叉闭合） | 表驱动（2 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 词表常量 | **不 mock**：真实 `ElementKind::ALL` / `GateTier::ALL` / `DiagnosticKind::ALL` / `GateTier::default()`（同源派生即被测语义） | 全部词表用例 |
| 目录路径 | **不 mock**：真实 `foundation::layout::archi_*_rel_dir()`（禁在测试内手抄路径串做期望值） | 目录派生 / 体量 |
| 静态骨架 | **不 mock**：`include_str!("archi-knowledge.md")` 编译期内嵌（与生产同一份字节） | 顺序 / 空白 / 纯函数性 |
| 文件系统 / 网络 / 时钟 | **零依赖**（纯函数零 IO） | —— |

---


### packages/desktop/src-tauri/crates/infra/agent/src/archi-knowledge.md -> packages/desktop/src-tauri/crates/infra/agent/src/archi_knowledge_test.rs

#### 待测功能

design 未声明公共 API，本节无条目。

<!-- design.md「公共函数 / API」子表未声明本文件的公共 API 变更（静态 markdown，经 `include_str!` 编译期内嵌，无函数面）。 -->

> 说明：测试对象取自 design「新增文件」`archi-knowledge.md` 行（静态骨架 + 唯一 toml 围栏示例块；零 `<>`、零目录字面量、零词表）、D3 / D4、待决问题 5、`desktop-archi-agent`「架构会话的系统知识注入」与 AC-4 / AC-14。自证取法见 D4：`archi::parse::TomlParser::parse(fence)`（`SourceParser` 公开）+ `archi::graph::ModelGraph::build(model).diagnostics()` 双零——零 tempdir、零 IO。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 唯一示例围栏块 | 正向 | md 内 toml 围栏恰 1 处（起止配对、围栏内非空）；无第二处 toml 围栏、无未闭合围栏 | 单例（计数 + 配对） |
| 示例解析自证 | 正向 | 围栏内容 `TomlParser::parse(fence)` → `diagnostics.is_empty()`（字段名 / 值域 / 内联 `metadata` / 键形任一漂移即红）；解析后元素 ≥ 2 且 FQN 精确含点（`App` 与 `App.Auth`） | 单例 |
| 示例图自证 | 正向 | `ModelGraph::build(model).diagnostics().is_empty()`（引用与层级自洽：零 `unknownReference` / 零 `orphanHierarchy` / 零 `cycle` / 零 `dependsOnAncestor`） | 单例 |
| 内联 `metadata` 锁点 | 异常防护 | 解析后 `App.Auth` 的 `metadata` 含 `owner` 键；元素集中**不存在** FQN 形如 `App.Auth.metadata` 的条目（写成 `[element."X".metadata]` 子表即被读成另一元素 → 本断言红） | 单例 |
| 键形锁点 | 异常防护 | 元素以带引号完整 FQN 单键落表（解析成功且 FQN 含 `.`）；同一元素不得以大小写变体双份出现 | 单例 |
| `gate` 词面 | 正向 | 示例中出现的 `gate` 取值 ∈ `GateTier::ALL`（逐值比对 `as_str()` 集）；不出现封闭集外档位串 | 单例 |
| 关系两端已声明 | 正向 | 每条 `relation` 的 `from` / `to` 均在元素集内（祖先 → 后代表达式由零 `dependsOnAncestor` 兜住）；`desc` 非空 | 单例 |
| 文档引用不触发结构错 | 边界 | `docs` 指向不存在的虚构路径不产生结构诊断（`docNotFound` 属 warning，不进 `diagnostics()` 自证面） | 单例 |
| 零占位符 | 异常 | md 全文零 `<>` 连续两字符；零 `<change>` / `<phase>` / `{{ }}` / `__INCLUDE:*` 一类模板片 | 单例（逐模式 `!contains`） |
| 零目录字面量 | 异常 | md 全文不含 `openspec` / `architecture` / `models/` / `decisions` 字面（目录路径由 `build()` 派生注入，AC-14） | 单例（逐串 `!contains`） |
| 体量切分 | 边界 | md `len() <= 3 * 1024`（「骨架 ≤ 3KB + 派生段 ≤ 1KB」切分余量，待决问题 5） | 单例 |
| 必静态条款在场 | 正向 | proposal §2「必静态部分」八条逐条关键词在场：① 带引号完整 FQN 单键 / 禁点分嵌套 ② 内联 `metadata` 表 / 禁子表 ③ 最长前缀层级 + 祖先先声明 ④ 新元素 `gate` 缺省 `off` ⑤ 不写接口与导出符号（契约权威在 specs，只经 `docs` 引用） ⑥ 改完调 `archi_module` 复查并按 `file:line` 修 ⑦ 直改主仓 + 随 `git diff` 评审 + 写盘后请用户点刷新 ⑧ 续接历史会话先取当下模型 | 表驱动（8 条） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 骨架 md | **不 mock**：`include_str!("archi-knowledge.md")` 编译期内嵌（生产同一份字节，禁测试内复制粘贴副本） | 全部用例 |
| `archi` 解析面 | **不 mock**：真实 `archi::parse::TomlParser` + `archi::graph::ModelGraph::build`（双重零诊断即被测判据，任何替身都会抹掉漂移信号） | 解析 / 图自证 |
| 文件系统 | **零 IO**：不落盘、不 tempdir（D4 明确避开 tempdir） | —— |
| 网络 / 时钟 / 环境 | **零依赖** | —— |

---

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop_test.rs

#### 待测功能

- `system_prompt(knowledge: Option<&str>, agent_md: Option<String>) -> Option<String>`: `(Some,Some)` → `"{knowledge}\n\n{agent_md}"`；`(Some,None)` → `knowledge.to_owned()`；`(None,Some)` → `agent_md`（逐字）；`(None,None)` → `None`（公共函数 / API）

> 说明：另有两条测试对象不来自「公共函数 / API」子表——① `run` 的系统消息合成单点改造（design「修改文件」`loop.rs` ② 行 + D5，原 `:193-198` 的 `if let Some(preamble) = preamble::load(..)` 改为一次 `system_prompt` 调用 + 一次 `push`）；② archi 面的工具面 / 写面链路（AC-6 / AC-7 / AC-8；`tools.rs` / `policy.rs` / `protect/**` 为 design 明列零改动面，其断言由既有用例回归承载）。既有三形态 `AGENT.md` 用例（`preamble每轮重读接入请求…` / `preamble缺席时请求无system消息不注入空串` / `preamble在场时请求系统提示词逐字含文件内容`）语义不变。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `system_prompt` 双在场 | 正向 | `(Some("K"), Some("A"))` → `Some("K\n\nA")`：知识在前、`AGENT.md` 逐字在后、恰一个空行（`\n\n`）分隔，无标题 / 无前缀包装 | 单例 |
| `system_prompt` 仅知识 | 正向 | `(Some("K"), None)` → `Some("K")`（不追加分隔符、不追加空行） | 单例 |
| `system_prompt` 仅 `AGENT.md` | 正向 | `(None, Some("A"))` → `Some("A")` 逐字（既有语义零改写：不加前后缀、不改写、不包装） | 单例 |
| `system_prompt` 两者皆无 | 边界 | `(None, None)` → `None`（不注入空串 System） | 单例 |
| `system_prompt` 空串入参 | 边界 | `(Some(""), Some("A"))` → `Some("\n\nA")`；`(Some("K"), Some(""))` → `Some("K\n\n")`（按公式拼接，不降级到单侧分支、不 panic、不吞首段） | 表驱动（2 形态） |
| `system_prompt` 特殊字符 | 边界 | 两段各含 `\n` / `\0` / emoji / CRLF → 逐字保留（零转义 / 零归一），仅两段之间插入一个 `\n\n`；分段位置可用 `split("\n\n")` 稳定还原为两段 | 表驱动（4 字符类） |
| `system_prompt` 超长与不截断 | 边界 | `agent_md` 长 40KB、`knowledge` 长 4KB → 返回长度 = 两段长度之和 + 2（`system_prompt` 不截断：32KB 截断责任在 `preamble::load`，知识不截，R-4） | 表驱动（2 长度） |
| `system_prompt` 纯函数性 | 边界 | 同参两次调用结果逐字相等；不读盘、不查环境（入参即全部输入） | 单例 |
| `run` 系统消息合成（双在场） | 正向 | `turn.injections.preamble = Some("K")` + root 有 `AGENT.md` → 首轮请求 `chat_history` 恰一条 `Message::System`，内容 = `K\n\n<AGENT.md 原文>`；不为两段各推一条 | 单例 |
| `run` 系统消息合成（仅 preamble） | 正向 | `preamble = Some("K")` + root 无 `AGENT.md` → 恰一条 System = `K`（既有「缺席不注入空串」语义扩展到单侧在场） | 单例 |
| `run` 系统消息合成（仅 `AGENT.md` / 皆无） | 回归 | 仅 `AGENT.md` → 逐字（既有断言全绿）；皆无 → 零 System（既有断言全绿）——新增分支不改变这两条既有可观察行为 | 回归（2 形态） |
| `run` 每轮重读 | 正向 | 两轮假流 + 首轮流后改写 `AGENT.md` → 两次请求的 System 均为合并体，且次轮的 `AGENT.md` 段为改后内容、知识段与首轮逐字相同（每轮重读 + 知识段来自会话注入不重读盘） | 单例 |
| archi 面工具面（AC-6） | 正向 | `turn.injections = SessionInjections { preamble: Some(K), tools: None, toolset: Some(Archi { module: true, search: true }) }` → `RunStarted.tools` = `TOOL_NAMES` 七名（含 `write` / `edit` / `bash`，保序）+ `archi_module` + `archi_search`，与实际 `request.tools` 逐名一致 | 单例（逐名） |
| archi 面档位（AC-6） | 回归 | `default` 档下 `archi_module` / `archi_search` 放行、`write` / `edit` / `bash` 被 policy 拒（既有 `policy_test` / `loop_test` 断言不改）；`acceptEdits` / `bypassPermissions` 档写面放行 | 回归 |
| archi 面写盘（AC-7） | 正向 | archi 面（`tools = None`）在 `bypassPermissions` 档下 `write` 新建 `<archi_dir>/models/09-x.toml`、`edit` 改既有 `gate` 行 → 工具结果 `is_error = false`、文件落盘、保护链零拦截（`<archi_dir>/models/**` 不在内置锁面） | 单例（两工具） |
| archi 面写坏（AC-8） | 异常 | 写入坏模型（非法 TOML / 缺必填字段）后调用 `archi_module` → 工具结果 `is_error = true` 且文案含 `file:line` 结构化诊断；run 不中断（后续续轮正常收敛）；无新增写前校验插口（写入本身零 `Err`） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 模型提供者 | **mock（既有装置）**：`FakeModel` 可编程假流（`with_turns` / `captured_requests` / `rewrite_file_on_first_stream`），禁用真实网络 | 全部 `run` 用例 |
| 文件系统 | **不 mock**：真实 `tempfile::TempDir` 落 `AGENT.md` 与 `openspec/architecture/models/*.toml`（每轮重读与读盘即被测语义） | 重读 / 写盘 / 写坏 |
| `preamble::load` | **不 mock**：真实读取 + 真实 32KB 截断口径（截断与知识段互不干扰是判据之一） | 合成 / 超长 |
| `system_prompt` 纯函数 | **不 mock**：直接调用（断言即契约） | 四形态 / 边界 |
| `tools::definitions` / `policy` / `protect` | **不 mock**：真实组合（工具面与档位判据来自真实单点；`protect` 内部规则文件真实装载） | archi 面工具 / 档位 / 写盘 |
| 网络 / 时钟 | **零依赖**（假流无真实时钟推进；活性预算既有常量） | —— |

---

### packages/desktop/src-tauri/crates/core/archi/src/dto.rs -> packages/desktop/src-tauri/crates/core/archi/src/dto_test.rs

#### 待测功能

design 未声明公共 API，本节无条目。

<!-- design.md「公共函数 / API」子表未声明本文件的公共函数变更（本变更只增关联常量 `ArchiDiagnosticKind::ALL`，属「类型定义」子表行；枚举名 / 线值 / specta 注册零变化）。 -->

> 说明：测试对象取自 design「类型定义」`ArchiDiagnosticKind`（别名 `DiagnosticKind`）行、D6、`desktop-archi-model`「消费面词汇集一致性对拍」MODIFIED 段与 AC-3 / R-9。既有 `词汇serde线形逐值` / `缺省与可选字段显式线形` 用例语义零改动（本变更不新增、不修改任何线值）。知识文本一侧的词汇集断言住 `archi_knowledge_test`（`archi` 不得反向依赖 `agent-runtime`，见「不可测试项」）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ALL` 与 `as_str` 逐值对拍 | 正向 | `DiagnosticKind::ALL` 九值逐项 `as_str()` 依次为 `syntaxError` / `missingField` / `invalidValue` / `unknownField` / `duplicateElement` / `cycle` / `dependsOnAncestor` / `unknownReference` / `orphanHierarchy`（顺序即 `as_str` 分支序，D6） | 表驱动（9 值逐位） |
| `ALL` 长度锚 | 边界 | `DiagnosticKind::ALL.len() == 9`（`[Self; 9]` 长度不符即编译期红；加变体忘补 `ALL` 时本断言与编译面双红） | 单例 |
| `ALL` 无重复 / 全覆盖 | 异常 | `ALL` 去重后长度仍为 9；以「逐变体构造的九项清单」与 `ALL` 做集合相等比较，缺失值点名报出（漏值不可绕过） | 单例（集合比较） |
| `ALL` 序与 `as_str` 同源 | 边界 | 逐位断言 `ALL[i].as_str()` 与 `as_str` 的 match 分支序一致（把 `ALL` 顺序打乱即红——`build()` 派生段的输出序依赖此序） | 表驱动（9 位） |
| 线值零变化 | 回归 | camelCase 逐值 serde 序列化 / 反序列化 + 非法值 `Err`（既有 `词汇serde线形逐值` 全绿）；`ALL` 新增不引入任何线面变化 | 回归 |
| 既有全量常量复用面 | 回归 | `ElementKind::ALL.len() == 4` 且四值 `as_str` 逐值；`GateTier::ALL.len() == 3` 且 `GateTier::default() == GateTier::Off`（`build()` 的 `gate` 段与缺省锚依赖此面，既有断言不改） | 回归（表驱动 2 组） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 关联常量 / `as_str` | **零 mock**：纯常量与纯函数直接断言 | 全部用例 |
| serde | **不 mock**：真实 `serde_json` 线值（既有用例） | 线值回归 |
| 知识文本 | **不在本 crate 断言**：`archi` 不得依赖 `agent-runtime`（依赖方向单向 infra → core），知识文本面落 `archi_knowledge_test` | —— |
| 文件系统 / 网络 / 时钟 | **零依赖** | —— |

---

### packages/desktop/src-tauri/crates/core/foundation/src/layout/mod.rs -> packages/desktop/src-tauri/crates/core/foundation/src/layout/mod_test.rs

#### 待测功能

- `archi_rel_dir() -> String`: `<域根段>/<架构域段>`（形如 `openspec/architecture`），`/` 分隔、零前导 `./`、零 IO、不接 root 入参（公共函数 / API）
- `archi_models_rel_dir() -> String`: `archi_rel_dir()` + `/{MODELS_DIR_NAME}`（公共函数 / API）
- `archi_decisions_rel_dir() -> String`: `archi_rel_dir()` + `/{DECISIONS_DIR_NAME}`（公共函数 / API）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 三函数前缀同源 | 正向 | 三者皆以 `archi_rel_dir()` 为前缀（`starts_with`）；`archi_models_rel_dir()` 尾段恰为 `models`、`archi_decisions_rel_dir()` 尾段恰为 `decisions`（不互相串位） | 表驱动（2 子目录） |
| 与绝对族一致 | 正向 | `Path::new(root).join(archi_models_rel_dir()) == archi_models_dir(root)`、`…join(archi_decisions_rel_dir()) == archi_decisions_dir(root)`、`…join(archi_rel_dir()) == archi_dir(root)`（`strip_prefix(root)` 逐段相等）；`root` 取三形态：绝对 tempdir 路径 / 尾分隔符 root / 空 root（`Path::new("")`） | 表驱动（3 root × 3 函数） |
| 分隔符与前后缀 | 边界 | 零 `\`（`/` 分隔）；`archi_rel_dir()` 恰含 1 个 `/`、两子目录恰含 2 个；零前导 `./`、零尾 `/`、零连续 `//`、零空段 | 表驱动（3 函数） |
| 常量组派生 | 异常 | 首段 == `domain_dir_name()`（公开访问器交叉）；第二段 == `archi_dir(root)` 的末段名（架构域段名唯一持有者在常量组，函数体内零行内字面量）；改 `ARCHITECTURE_DIR_NAME` 无第二处需同步 | 单例（逐段） |
| 零 IO / 纯函数 | 边界 | 三函数无 root 入参；连续三次调用逐字相等；不存在的路径 / 不存在的磁盘状态下同样成立（零 `exists()` 调用面） | 单例 |
| 与既有判据口径一致 | 正向 | `is_archi_rel_path(&archi_rel_dir())` 与 `is_archi_rel_path(&archi_models_rel_dir())`、`is_archi_rel_path(&archi_decisions_rel_dir())` 均 `true`（派生族与判据单点互证） | 表驱动（3 形态） |
| 既有四函数零回归 | 回归 | `archi_dir` / `archi_models_dir` / `archi_decisions_dir` / `is_archi_rel_path` 逐字不变；`Layout` 三字段 / `resolve` / `config_path` / 报告目录族语义不变（既有 30+ 用例全绿） | 回归 |
| 字面量双禁令扫描 | 回归 | 既有 `全包产品源码双禁令扫描_openspec与config_json字面量_layout_为唯一例外` 仍绿：新增三函数不使 `openspec` / `architecture` / `config.json` 字面量泄漏到 `openspec` 域外产品源码（AC-14） | 回归 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 常量组 / 派生函数 | **零 mock**：纯字符串拼接直接断言 | 全部用例 |
| 文件系统 | **零 IO**：不建 tempdir、不读盘（`root` 仅作 `Path` 参与比较，存在性不入判据） | 与绝对族一致 |
| 网络 / 时钟 / 环境 | **零依赖** | —— |

---

### packages/desktop/src-tauri/src/commands/exec/mod.rs -> packages/desktop/src-tauri/src/commands/exec/mod_test.rs

#### 待测功能

- `list_archi_chat_sessions(stores, root, offset, limit) -> Result<SessionPage, String>`: `#[tauri::command] #[specta::specta]`；blank root → `SessionPage { sessions: vec![], total: 0 }`；否则 `list_sessions(Some(ARCHI_CHAT_SOURCE), None, offset, limit)`（公共函数 / API）

> 说明：另有两条测试对象来自 design「修改文件」`exec/mod.rs` ① 行与事实校准 2——`ARCHI_CHAT_SOURCE` 常量（字面量单点，D13 / AC-14）与 `resolve_exec_root` 的 `cwd = null → root` 回落链（AC-7；函数本体既有、语义零改动，本节只加一条 archi 归属的端到端回落断言）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 归属命中 | 正向 | 库内 fixture：2 条 `source = "archi"` / `source_ref = None` + 2 条 change chat（`"chg-1"` 两段）+ 1 条流程四段 + 1 条 `debug` → 只返回 2 条 archi 行，`updated_at` 降序，`total == 2`（change chat 与流程会话一条不混） | 单例 |
| `source_ref` 不过滤 | 正向 | archi 会话行 `source_ref` 为非 null 字符串（列是自由字符串，生产恒 null）→ 仍在清单内（`None` 过滤语义，与 `list_change_chat_sessions` 的 id 过滤形成对照） | 单例 |
| 与 `agent_sessions` 同参全等 | 正向 | `list_archi_chat_sessions(state, root, 0, 20)` 与 `agent_sessions(state, root, Some("archi"), None, 0, 20)` 的 `total` 与 `sessions` 逐字段全等（含 `stats.turnCount`：轮行 fixture 随行，薄包装零改写聚合面） | 单例 |
| 分页信封 | 边界 | 3 条 fixture（`updated_at` 递增）→ `offset 0 / limit 1` 取最大者且 `total == 3`；`offset 1 / limit 1` 取第二条；`offset 0 / limit 0` → `sessions` 空而 `total` 仍为过滤集全量；`offset` 超出总量 → 空 `sessions` + `total` 不变 | 表驱动（4 形态） |
| 并列排序 | 边界 | 两条同 `updated_at` → 按 id 降序（行层排定） | 单例 |
| 空库 | 边界 | 无任何会话的 workspace → 空页 `{[], 0}`（`Ok`，非 `Err`） | 单例 |
| blank root 守卫 | 异常 | `""` / `"   "` → 空页信封且**零库寻址**（`workspace_db_count` 前后不变，不铸新库） | 表驱动（2 形态） |
| root 不可寻址 | 异常 | `for_root` 失败（root 为文件 / 不可建库）→ `Err` 透传不 panic（既有 `root不可寻址_三查询命令err透传不panic` 扩一条命令形态） | 单例 |
| 跨 root 隔离 | 异常 | A 库 2 条 archi 会话 → B 库同参查询空页（`total == 0`），不跨库泄漏 | 单例 |
| `ARCHI_CHAT_SOURCE` 词面 | 边界 | `super::ARCHI_CHAT_SOURCE == "archi"`（与 `compose::ARCHI_SOURCE` / 前端 `use-archi-chat` 三处同值，D13；注释互指由源码审查承载） | 单例 |
| `cwd = null` 执行根回落（AC-7） | 正向 | `agent_start(..., source = "archi", cwd = null, root = <tempdir>)` → 会话执行根回落 `root`（记录型 runner / 轮行落库可取证的执行根即 `root`），不经 `for_root` 二次寻址、不铸第二份库 | 单例 |
| 既有 change chat 命令 | 回归 | `list_change_chat_sessions` 四条用例语义不变（`source_ref` 精确相等、空白 root 空页、与 `agent_sessions` 全等、分页） | 回归 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| workspace 库 | **不 mock**：真实 `WorkspaceStores` + 真实 temp 数据根（既有 `Env` / `app_with_stores` / `workspace_db_count` 装置），`tauri::test::MockRuntime` 托管 `State` 直调 | 全部用例 |
| 会话 fixture | **不 mock 时钟**：`updated_at` 由 fixture 显式给定（排序键即数值）；轮行经真实 `begin_agent_turn` / `finish_agent_turn` 落库 | 归属 / 分页 / 全等 |
| `agent_start` 引擎 | **mock（既有装置）**：记录型假 runner / 假 Channel（既有 `agent_start` 用例装置），不启真实模型 IO | 执行根回落 |
| 网络 / 时钟 / 环境 | **零依赖** | —— |

---

### packages/desktop/src-tauri/src/commands/mod.rs -> packages/desktop/src-tauri/src/bindings/mod_test.rs

#### 待测功能

design 未声明公共 API，本节无条目。

<!-- design.md「公共函数 / API」子表未声明本文件的公共 API 变更（`all_commands!` 尾部追加一条命令路径）。 -->

> 说明：测试对象取自 design「修改文件」`commands/mod.rs` 行（`all_commands!` 唯一登记面尾部追加 `list_archi_chat_sessions`）+ 事实校准 1（漏登记则 IPC 与 `bindings.ts` 同时失守）+ AC-11 / AC-15。「命令登记面 → 生成产物」的既有守门测试住 `src/bindings/mod_test.rs`（`COMMAND_NAMES` / `COMMAND_WRAPPERS` / 生成文件字面断言三面），故本节落此。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| invoke 名登记 | 正向 | `COMMAND_NAMES` 含 `list_archi_chat_sessions`；`COMMAND_WRAPPERS` 含 `listArchiChatSessions` | 单例（两断言） |
| 命令总数 +1 | 边界 | `COMMAND_NAMES.len() == 44` 与 `COMMAND_WRAPPERS.len() == 44`（现 43，本变更恰 +1；漏登记或重复登记即红） | 单例 |
| 生成绑定逐字 | 正向 | `bindings.ts` 含 `listArchiChatSessions: (root: string, offset: number, limit: number) => __TAURI_INVOKE<SessionPage>("list_archi_chat_sessions", { root, offset, limit })`（三参透传 + 返回 `SessionPage`；无 `id` 参即与 change chat 版签名的区分点） | 单例 |
| 唯一登记 | 异常 | `list_archi_chat_sessions` 在 `COMMAND_NAMES` 中计数恰 1（无重复项）；`listArchiChatSessions` 同理 | 单例 |
| 既有条目零位移 | 回归 | 既有 43 条（含 `list_change_chat_sessions` / `agent_sessions` / `archi_*` 五条）逐条仍在册；`dto.ts` 星号 re-export 反向锚不变 | 回归 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 生成产物 | **不 mock**：真实读取 `src/types/generated/bindings.ts`（守门测试即读产物文本，`DESKTOP_GOLDEN_REWRITE` 一类重写开关不适用于本文件） | 生成绑定逐字 |
| 命令清单 | **不 mock**：`all_commands!` 展开后的常量清单真实消费（登记面唯一） | 登记 / 总数 / 唯一性 |
| IPC runtime | **零依赖**：不启 `tauri::Builder`、不 invoke | —— |
| 网络 / 时钟 / 环境 | **零依赖** | —— |

---

### packages/desktop/src/components/agent/session-panel.tsx -> packages/desktop/src/components/agent/session-panel.test.tsx

#### 待测功能

- `SessionPanel(props: SessionPanelProps): React.JSX.Element`: 骨架唯一实现；`testIdPrefix` 生成全部查询挂钩（`${prefix}-panel` / `-actions` / `-new` / `-history` / `-note` / `-history-list` / `-history-item` / `-history-more` / `-history-empty` / `-history-error` / `-rail` / `-expand` / `-session`）（公共函数 / API）

> 说明：入参面取自 design「类型定义」`SessionPanelState` / `SessionPanelProps` 两行 + D8 / D9，行为口径取自 `desktop-archi-chat` 面板骨架场景与 AC-10 / AC-12 / AC-13。`state` 以字面量直构（不经 hook），回调以 `vi.fn()` 观测；`SessionPanelProps` 不出桶（`index.test.tsx` 反向锚）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 前缀参数化（13 个查询挂钩） | 正向 | 同一 `state` 分别以 `testIdPrefix="change-chat"` 与 `"archi-chat"` 渲染，逐套断言 13 个 testid 生成正确（展开态 + `listOpen` + 空清单 + `note` 非 null 的组合态下全部在场）；两套前缀互不串味（`change-chat-*` 与 `archi-chat-*` 各命中一次） | 表驱动（2 前缀 × 13 testid） |
| 头区动作 | 正向 | `title` 呈现于 `-actions`；点 `-new` → `newSession` 恰调一次；点 `-history` → `toggleList` 恰调一次，且 `aria-expanded` 随 `listOpen` 两态（`true` / `false`） | 表驱动（2 形态） |
| 「新会话」禁用态 | 边界 | `canStart=false` → `-new` `disabled`（点击零回调）；`canStart=true` → 可用可点（骨架只消费该位，不自判） | 表驱动（2 形态） |
| 状态行只认 `note` | 边界 | `note=null` → `-note` **不在场**（不占位、不注入空串）；`note="模型改动不会自动刷新，改完点「刷新」"` → 逐字呈现 | 表驱动（2 形态） |
| 历史清单（展开 + 列表） | 正向 | `listOpen=true` + 2 条 `SessionSummary` → 每项 `-history-item` 在场，`data-session-id` 为完整 id、`title` 为完整 id、行内为 id 前缀（8 字符）+ 更新时间 + `N 轮`；点击某项 → `selectSession(该项 row.id)` 恰调一次 | 表驱动（2 项 + 点击） |
| 历史清单（加载 / 错误 / 空态） | 异常 | `listLoading=true && sessions.length===0` → 「加载中…」在场且 `-history-empty` 缺席；`listError="库不可读"` → `-history-error` 在场（文案含理由逐字）；`listLoading=false && listError===null && sessions=[]` → `-history-empty`（「（暂无历史会话）」）；三态互斥（同一拍只呈其一） | 表驱动（3 形态 + 互斥） |
| 分页「加载更多」 | 边界 | `sessions.length(2) < total(5)` → `-history-more` 在场，点击 → `loadMore`；`listLoading=true` 时该按钮 `disabled`（点击零回调）；`sessions.length === total` → 无 `-history-more`；`sessions.length > total`（异常数据）→ 同样不呈（判据 `<` 严格） | 表驱动（4 形态） |
| 折叠窄条 | 正向 | `collapsed=true` + `onToggleCollapsed` → 只渲 `-rail`（竖向 `title` + `-expand`，`-expand` 的 `title` 为 `展开{title}`）；展开体 testid（`-actions` / `-note` / `-history-list` / `-session`）全部缺席；点 `-expand` → `onToggleCollapsed` 恰调一次 | 单例 |
| 折叠不传回调（无死锁） | 异常 | `collapsed=true` 且 `onToggleCollapsed` 未传 → 渲展开体：`-rail` 与 `-expand` **均不在场**（不出现「收起但无展开入口」的死锁面板）；`collapsed=false` 且传了回调 → 同样渲展开体 | 表驱动（2 形态） |
| 会话体与 composer 插槽 | 正向 | `-session` 在场（`AgentSessionView` 注入形态，不经自持取数）；`composer(actions)` 收到 `{ running, loading, send, stop }`，逐位等于 `state.session` 的同名字段（骨架不另建会话流、不改造 actions） | 单例（逐字段） |
| 骨架无业务判据 | 异常 | 传入「看似禁发」的 state（`session.error` 非 null、`session.loading=true`、`sessions` 大量）→ `canStart` 未被骨架改写、`composer` 的 `running` / `loading` 原值透传（骨架内零 `canSend` / `disabledReason` / 归档 / run 期派生）；`SessionPanelState` 不含 `canSend` / `disabledReason` 由类型锚定承载（传该字段需额外属性，不属契约） | 单例 |
| 空集合形态 | 边界 | `sessions=[]` / `total=0` / `listOpen=false` / `note=null` / `session.session=null`（空态草案态）→ 展开体仍完整（`-actions` + `-session` 在场），无空白崩溃、无占位残留 | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 面板状态 | **不经 hook**：`SessionPanelState` 字面量直构（骨架不自取数即被测契约），回调为 `vi.fn()` 观测 | 全部用例 |
| `AgentSessionView` / `AgentInput` | **不 mock**：真实组合（注入形态零取数，`composer` 插槽的契约即真实 actions） | 插槽 / 会话体 |
| IPC | **mock（进程边界）**：`vi.mock('@tauri-apps/api/core')` 的 `invoke` + `Channel` 最小垫片（注入形态零调用，守护「骨架不自取数」） | 全部用例（含零调用断言） |
| 网络 / 时钟 | **零依赖** | —— |

---

### packages/desktop/src/components/agent/index.tsx -> packages/desktop/src/components/agent/index.test.tsx

#### 待测功能

design 未声明公共 API，本节无条目。

<!-- design.md「公共函数 / API」子表未声明本文件的公共 API 变更（桶文件只增 `SessionPanel` 组件与 `SessionPanelState` 类型两条 `export`，无函数签名）。 -->

> 说明：测试对象取自 design「修改文件」`index.tsx` 行（`SessionPanel` + `SessionPanelState` 两条导出；`SessionPanelProps` 留文件内、不出桶以免成为 knip 报告里的可疑导出）、D8 与 AC-10。既有 exporter 集合断言（`Object.keys(barrel).sort()`）位于本文件，故本节落此。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 运行时出口集合 | 正向 | `Object.keys(barrel).sort()` 逐字为 `['AgentInput', 'AgentSessionView', 'SessionPanel']`（新增一键、既有两键零位移）；`barrel` 不含 `SessionPanelProps` / `AgentMessages` / `AgentSessionViewContainer` | 单例 |
| 唯一实现单点 | 正向 | `barrel.SessionPanel` 与自 `./session-panel` 直接 import 的为同一引用（不建第二条面板实现） | 单例 |
| 类型出口 | 边界 | `SessionPanelState` 可自 barrel 导入并在类型位置使用（以 `const state: SessionPanelState = {…}` 构造锚定，编译期即验）；`SessionPanelProps` 自 barrel 导入应不可编译（以桶内键集合断言代替，不在测试内写非法 import） | 单例（编译面锚定） |
| barrel 冒烟渲染 | 正向 | 经 barrel 以最小 `SessionPanelState`（空清单 / 空态会话 / `note=null`）+ `title` / `testIdPrefix="barrel"` / `composer` 插槽渲染 → `barrel-panel` 与 `barrel-session` 在场，`invoke` 零调用 | 单例 |
| 既有出口零回归 | 回归 | `AgentInput` / `AgentSessionView` / `AgentSessionActions` / `AgentSessionViewProps` 出口与既有引用相等（"既有 AgentInput 出口不变" / "类型出口可自 barrel 导入" 两用例语义不变） | 回归 |
| 越界 / 多余导出 | 异常 | 桶内出现第二面板实现名（如 `SessionPanelV2`）或把 `SessionPanelProps` 出桶 → `Object.keys(barrel)` 集合断言红（knip 面同时把「仅类型消费者」的导出报为可疑导出）；`SessionPanel` 只经单点入口 | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| IPC | **mock（进程边界）**：`vi.mock('@tauri-apps/api/core')` 的 `invoke` + `Channel` 最小垫片（既有装置；barrel 冒烟渲染零取数） | 全部用例 |
| 内部模块 | **不 mock**：`AgentSessionView` / `AgentInput` / `SessionPanel` 真实组合（barrel 即 re-export，替身会抹掉「单点出口」判据） | 全部用例 |
| 网络 / 时钟 | **零依赖** | —— |

---

### packages/desktop/src/views/changes/agent/change-chat-panel.tsx -> packages/desktop/src/views/changes/agent/change-chat-panel.test.tsx

#### 待测功能

- `ChangeChatPanel(props: ChangeChatPanelProps): React.JSX.Element`: props 面零变化；内部改骨架消费者（`testIdPrefix="change-chat"`）（公共函数 / API）

> 说明：本文件的既有 675 行行为级用例是骨架抽取的**回归证据**（R-5）：本节用例的判据是「testid / 文案 / 交互序列逐字不变」，只允许调整查询挂钩，MUST NOT 改断言语义。判据仍留本地（`canSend` / `disabledReason` / 归档 / run 期）由 `useChangeChat` 章承载，本节只断言「面板把判据结果交给骨架 / composer」的可观察后果。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| testid 集合逐字 | 回归 | 展开态逐项在场：`change-chat-panel` / `-actions` / `-new` / `-history` / `-note` / `-session`；`-history` 点开后再断言 `-history-list` 与 `-history-item`；空清单断言 `-history-empty`；`sessions.length < total` 断言 `-history-more`；加载失败断言 `-history-error`；`collapsed=true` 断言 `-rail` 与 `-expand`（13 个 testid 全覆盖，前缀由骨架生成） | 表驱动（13 testid） |
| 文案逐字 | 回归 | 「通用 Agent」/「发送首条消息即开启新会话」/「（暂无历史会话）」/ `加载历史会话失败：` 前缀 / 「加载更多」/ 「›」+ `title="展开通用 Agent"` 逐字不变（`DRAFT_NOTE` 移入 hook 后呈面不变） | 回归（6 文案） |
| 交互序列 | 回归 | 展开历史 → 点选历史项 → `session_detail` 以该 id 直查重放 → 清单收起；「新会话」→ 空态（`sessionId=null`）零落库；输入并发送 → `agent_start` 参数与变更前逐字一致；运行中点停止 → `agent_stop` | 回归（4 序列） |
| 判据留本地 | 正向 | `readOnly=true`（归档）→ `-note` 呈「已归档 change 只读」、composer `disabled` 且呈 `change-chat-disabled-reason`、`-new` `disabled`；`runActive=true` → 呈「流程运行中」，同样全禁发而历史仍可浏览 | 表驱动（2 形态） |
| 空态不占位 | 边界 | 有会话（`currentSessionId` 非 null）且无禁用理由 → `-note` **缺席**（既有语义零变化，与 archi 侧常驻文案形成对照） | 单例 |
| 折叠窄条 | 边界 | `collapsed=true` → 仅 `-rail`；点 `-expand` → `onToggleCollapsed` 恰调一次（真相仍由分栏 `onResize` 侧持有，面板不自持） | 单例 |
| props 面零变化 | 回归 | 七个入参（`root` / `changeId` / `cwd` / `readOnly` / `runActive` / `collapsed` / `onToggleCollapsed`）语义不变：`cwd` 仅作执行根来源透传、`readOnly` 与 `runActive` 只进判据（既有用例全绿） | 回归 |
| 清单取数失败 | 异常 | `list_change_chat_sessions` reject（库不可读）→ `-history-error` 呈错误理由、`-history-list` 仍可展开 / 收起；会话面与 composer 不受牵连（发送链路照常），无未捕获异常 | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| IPC | **mock（进程边界）**：`vi.mock('@tauri-apps/api/core')` 的 `invoke`（按命令名分派 `agent_start` / `agent_stop` / `session_detail` / `agent_session_transcript` / `agent_session_watch` / `list_change_chat_sessions`）+ 可编程 `Channel` | 全部用例 |
| `SessionPanel` / `useChangeChat` / `AgentSessionView` / `ChangeChatComposer` | **不 mock**：真实组合（回归判据正是「换骨架后行为不变」，替身会抹掉它） | 全部用例 |
| 时钟 | **不 mock**：`updatedAt` 由 fixture 显式给定 | 历史清单 |
| 网络 | **零依赖** | —— |

---

### packages/desktop/src/views/changes/agent/hooks/use-change-chat.ts -> packages/desktop/src/views/changes/agent/hooks/use-change-chat.test.ts

#### 待测功能

- `useChangeChat(params: ChangeChatParams): ChangeChatState`: 返回面增 `note`（`disabledReason ?? （空态 ? 草案文案 : null）`），其余逐字不变（公共函数 / API）

> 说明：类型变更取自 design「类型定义」`ChangeChatState` 行（改为 `interface ChangeChatState extends SessionPanelState` + 增 `note: string | null`）+ D10；`DRAFT_NOTE` 常量移入本文件。既有 733 行用例（判据矩阵 / 清单取数 / 新会话 / 历史切换 / 会话面）语义零改动。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `note` 空态草案 | 正向 | `sessionId === null`（空态）且 `archived=false` / `runActive=false` → `note === '发送首条消息即开启新会话'` 逐字 | 单例 |
| `note` 有会话不占位 | 正向 | 首发后 `currentSessionId` 非 null 且无禁用理由 → `note === null`（骨架据此不渲状态行，既有「有会话且无理由时不占位」语义逐字平移） | 单例 |
| `note` 禁用理由优先 | 正向 | `archived=true` → `note === '已归档 change 只读'`（高于草案文案，即便 `sessionId === null`）；`runActive=true` → `note === '流程运行中'` | 表驱动（2 形态 × 2 空态/有会话） |
| `note` 类型面 | 边界 | 返回值在四象限（空态 / 有会话 × 有理由 / 无理由）覆盖 `string` 与 `null` 两 value 形态；不返回空串、不返回 `undefined` | 表驱动（4 象限） |
| 骨架契约继承 | 边界 | 返回对象可赋值给 `SessionPanelState`（`session` / `sessions` / `total` / `listOpen` / `listLoading` / `listError` / `toggleList` / `loadMore` / `newSession` / `selectSession` / `canStart` / `note` 逐字段在场，编译期锚定） | 单例 |
| 既有行为零回归 | 回归 | 判据矩阵（归档 / 流程运行中 → 全禁发、历史恒可用）、清单取数（挂载零 invoke、展开重取、分页累加、票号作废）、新会话 / 历史切换、会话面字段逐字不变（既有用例全绿） | 回归 |
| 取数失败不干扰 note | 异常 | `list_change_chat_sessions` reject → `listError` 为错误串且 `listLoading` 归位；`session_detail` reject → `session.error` 呈现；两种异常下 `note` 仍取 `disabledReason ?? （空态 ? 草案文案 : null）`（错误态不新增第三档文案） | 表驱动（2 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| IPC | **mock（进程边界）**：`vi.mock('@tauri-apps/api/core')` 的 `invoke`（按命令名分派）+ 可编程 `Channel`（既有装置） | 全部用例 |
| `useAgentSession` / `@ai-sdk/react` / `agent-transport` | **不 mock**：真实组合（hook 即链路入口） | note 两态 / 回归 |
| 时钟 | **不 mock**：`updatedAt` / `startedAt` 由 fixture 显式给定 | 清单 / 会话面 |
| 网络 | **零依赖** | —— |

---

### packages/desktop/src/views/architecture/agent/hooks/use-archi-chat.ts -> packages/desktop/src/views/architecture/agent/hooks/use-archi-chat.test.ts

#### 待测功能

- `useArchiChat(params: ArchiChatParams): ArchiChatState`: 会话面（`source: 'archi'` / `sourceRef: null` / `cwd: null`）+ 清单（`listArchiChatSessions(root, offset, 20)`，会话 id 确立 / 轮收口 / 展开历史时重取）+ 新会话（`setSessionId(null)` + `reset()`）+ 历史点选（`setSessionId`）+ 判据（`canSend = !running`、`canStart = !running`、`disabledReason = running ? '本会话运行中，请等待收口' : null`）+ `note` 两态（公共函数 / API）

> 说明：类型面取自 design「类型定义」`ArchiChatParams` / `ArchiChatState` 两行（后者 `extends SessionPanelState`）；行为口径取自 D10 / D13 / D14、`desktop-archi-chat` 的归属 / 交互 / 状态行场景与 AC-11 / AC-12。`source` 词常量（前端一份 `'archi'` 字面量）在此以 invoke 参数取证（AC-14 的跨侧同值由后端 `ARCHI_CHAT_SOURCE` / `compose::ARCHI_SOURCE` 用例闭合）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 归属参数逐字 | 正向 | 空态发送后 `agent_start` 参数逐字：`source === 'archi'`、`sourceRef === null`、`cwd === null`、`sessionId === null`（首发由后端铸行）、`prompt` = 输入文本、`permissionMode` 缺省 `bypassPermissions`（未选档即发） | 单例（逐字段） |
| 清单取数初值 | 正向 | 挂载后零清单 invoke（零轮询、零首发取数）；展开历史（`toggleList` 首拍）→ `list_archi_chat_sessions(root, 0, 20)` 恰一次，`listOpen=true`；再收起再展开 → 再取一次首屏（收起不清空） | 表驱动（3 拍） |
| 分页累加 | 边界 | 首屏返回 `total=25` / 20 条 → `loadMore()` → `list_archi_chat_sessions(root, 20, 20)`（`offset = 已加载条数`），items 追加、`total` 保持 25；`sessions.length >= total` 后 `loadMore` 不再发（组件侧按钮消失由骨架用例承载） | 表驱动（2 态） |
| 重取两事实 | 正向 | ① `agent_start` early-resolve 回写 `currentSessionId` → 重取首屏（`offset 0`）；② 轮收口（`running: true → false`）→ 再重取首屏；两拍各恰一次，无重复触发 | 表驱动（2 事实） |
| 新会话零落库 | 正向 | 已开会话后 `newSession()` → `sessionId === null` + `session.reset()`（消息 / 事件 / 链清空）；断言零 `agent_start`、零任何写库类命令 | 单例 |
| 历史点选直查 | 正向 | `selectSession('ses-1')` → `session_detail(root, 'ses-1')` 直查重放 + 清单收起；断言**不**用 `list_archi_chat_sessions` 做寻址（清单只作探测） | 单例 |
| 运行中禁发 | 边界 | `running=true` → `canSend === false`、`canStart === false`、`disabledReason === '本会话运行中，请等待收口'`；再调 `send` → 零第二发（`agent_start` 计数不变）；同拍 `toggleList` / `selectSession` / `loadMore` 仍可用（历史浏览不受禁发影响） | 单例 |
| `note` 两态与优先级 | 正向 | 空态（`currentSessionId === null`、无理由）→ `note === '发送首条消息即开启新会话'`；有会话且无理由 → `note === '模型改动不会自动刷新，改完点「刷新」'`；`running=true` → `note === disabledReason`（理由优先） | 表驱动（3 形态） |
| 清单失败 | 异常 | `list_archi_chat_sessions` reject（库不可读）→ `listError` 为错误串、`listLoading` 归位 false；首屏失败时 items 为空、分页失败时已加载 items 保留 | 表驱动（2 形态） |
| `root` 缺席 | 异常 | `root === null` → 全命令零 invoke（`list_archi_chat_sessions` / `session_detail` / `agent_session_transcript` / `agent_session_watch` / `agent_start` 全 0）；`send` 为 no-op、`toggleList` 不抛错 | 单例 |
| `root` 切根复位 | 边界 | `root` A → B：面板态（`sessionId` / `listOpen`）复位、在途清单请求作废（A 库残留页不落 B 库）；挂载首拍为同值复位（零 invoke）；`root` 由 `null` → 有值 后取数恢复正常 | 表驱动（3 形态） |
| 骨架契约继承 | 边界 | 返回对象可赋值给 `SessionPanelState`（`note` / `canStart` 等 12 字段逐字段在场，编译期锚定）；`ArchiChatState` 另含 `sessionId` / `currentSessionId` / `send` / `stop` / `canSend` / `disabledReason` | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| IPC | **mock（进程边界）**：`vi.mock('@tauri-apps/api/core')` 的 `invoke`（按命令名分派 `agent_start` / `agent_stop` / `session_detail` / `agent_session_transcript` / `agent_session_watch` / `list_archi_chat_sessions`）+ 可编程 `Channel`（直投 `{ ipc: 'record' }` 信封驱动 early-resolve 与收口） | 全部用例 |
| `useAgentSession` / `@ai-sdk/react` / `agent-transport` | **不 mock**：真实组合（hook 即链路入口，归属参数经真实链路到达 invoke） | 归属 / 交互 / 回归 |
| 时钟 | **不 mock**：`updatedAt` / `startedAt` 由 fixture 显式给定 | 清单 / 会话面 |
| 网络 | **零依赖** | —— |

---

### packages/desktop/src/views/architecture/agent/archi-chat-panel.tsx -> packages/desktop/src/views/architecture/agent/archi-chat-panel.test.tsx

#### 待测功能

- `ArchiChatPanel(props: ArchiChatPanelProps): React.JSX.Element`: `useArchiChat({ root })` → `SessionPanel`（`title="架构 Agent"` / `testIdPrefix="archi-chat"`）+ `ArchiChatComposer` 插槽（`running` / `disabled={!state.canSend}` / `disabledReason` / `onSend` / `onStop`）（公共函数 / API）

> 说明：入参面取自 design「类型定义」`ArchiChatPanelProps` 行；组合用例挂靠本组件（链路发起方 / 最上层调用方：面板 → `useArchiChat` → `useAgentSession` → transport → `agent_start`）。归属参数与清单判据的**单元**口径由 `use-archi-chat` 章承载，本节只断言面板层的装配与可观察后果（AC-11 / AC-12）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 骨架装配面 | 正向 | `archi-chat-panel` / `-actions` / `-session` 在场；头区标题逐字「架构 Agent」；composer 插槽呈 `archi-chat-composer` / `archi-chat-prompt` / `archi-chat-send`；`testIdPrefix` 与 change 侧不串味（`change-chat-*` 零命中） | 单例 |
| 归属链路（组合） | 正向 | 空态输入文本并点发送 → `agent_start` 参数逐字：`source='archi'` / `sourceRef=null` / `cwd=null` / `sessionId=null` / `prompt` = 输入 / `permissionMode='bypassPermissions'`（缺省档） | 单例（逐字段） |
| 首铸回写（组合） | 正向 | `agent_start` 早解析回写轮行 → 面板进入会话态（`archi-chat-session` 含 `session-meta`）、清单重取一次（`list_archi_chat_sessions` 计数 1→2 的可观察拍） | 单例 |
| 运行中禁发（组合） | 边界 | `running=true` → 状态行呈「本会话运行中，请等待收口」、`archi-chat-prompt` 与 `archi-chat-send` 同时 `disabled`、`archi-chat-new` `disabled`；再点发送零第二发（`agent_start` 计数不变）；同拍展开历史仍可用（`list_archi_chat_sessions` 可发） | 单例 |
| 历史与「新会话」（组合） | 正向 | 展开 `archi-chat-history` → `-history-list` 呈两条 → 点选一条 → `session_detail` 直查重放且清单收起；点 `-new` → 回空态（`-session` 呈草案态、`sessionId=null`）、零落库 | 单例 |
| 状态行两态 | 正向 | 空态 → 「发送首条消息即开启新会话」；有会话且非运行中 → 「模型改动不会自动刷新，改完点「刷新」」；`note` 恒占位（与 change 侧「null 不占位」的刻意差异） | 表驱动（2 形态） |
| 折叠（组合） | 正向 | `collapsed=true` → 仅 `archi-chat-rail`（竖向标题「架构 Agent」+ `archi-chat-expand`）；点 `-expand` → `onToggleCollapsed` 上抛（页面 state 为真源）；`collapsed=false` → 展开体（`-actions` / `-composer` 在场） | 表驱动（2 形态） |
| `root = null` | 异常 | 面板仍在场（局部空态 / 禁用态）且零会话 invoke（不 panic、不白屏）；`root` 恢复后发送可用 | 单例 |
| 零自动刷新 / 零自动提交 | 异常 | 全流程（发送 / 收口 / 写盘类工具事件）零 `archi_graph` 调用（画布刷新只由页面按钮驱动）、零提交 / 归档类命令（无 `git` 面、无 `archive_change`） | 单例（零调用断言） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| IPC | **mock（进程边界）**：`vi.mock('@tauri-apps/api/core')` 的 `invoke`（按命令名分派上列六命令）+ 可编程 `Channel` | 全部用例 |
| `useArchiChat` / `SessionPanel` / `ArchiChatComposer` / `AgentSessionView` | **不 mock**：真实组合（组合用例的入口即本组件，替身会抹掉链路判据） | 全部用例 |
| 时钟 | **不 mock**：`updatedAt` / `startedAt` 由 fixture 显式给定 | 清单 / 会话态 |
| 网络 | **零依赖** | —— |

---

### packages/desktop/src/views/architecture/agent/archi-chat-composer.tsx -> packages/desktop/src/views/architecture/agent/archi-chat-composer.test.tsx

#### 待测功能

- `ArchiChatComposer(props: ArchiChatComposerProps): React.JSX.Element`: `AgentInput`（`idPrefix="archi-chat"` / `label="架构 Agent"` / placeholder 文案）；档位 / 发送 / 停止收口在组件族（公共函数 / API）

> 说明：入参面取自 design「类型定义」`ArchiChatComposerProps` 行；`disabled` 语义口径取自 D10 与 design 待决问题 6（运行中整面板禁用 → 不呈停止入口），本节只断言 prop → `AgentInput` 的映射与显性后果，停止入口缺席的理由在用例中留痕。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 逐 prop 映射 | 正向 | `archi-chat-composer` / `archi-chat-prompt` / `archi-chat-send` / `archi-chat-permission-mode` 在场；`label` 逐字「架构 Agent」、`placeholder` 逐字；档位选项三项（`bypassPermissions` / `acceptEdits` / `default`） | 单例 |
| 发送透传 | 正向 | 输入文本 + 选 `default` 档 → 点发送 → `onSend({ text, permissionMode: 'default' })` 恰一次；输入框清空 | 单例 |
| 缺省档位 | 边界 | 未选档直接发送 → `onSend` 的 `permissionMode === 'bypassPermissions'`（缺省档，档位沿用会话选择器缺省） | 单例 |
| 空输入 | 边界 | `""` / `"   "`（纯空白）→ `archi-chat-send` `disabled`，点击零 `onSend`（既有 `AgentInput` 规则回归） | 表驱动（2 形态） |
| 运行中呈停止 | 正向 | `running=true` + `disabled=false` → `archi-chat-stop` 在场，点击 → `onStop` 恰一次 | 单例 |
| 整面板禁用 | 边界 | `disabled=true` → `archi-chat-prompt` 与 `archi-chat-send` 同时 `disabled`；`disabledReason` 呈于 `archi-chat-disabled-reason`（含换行 / 长文案时逐字不截断） | 表驱动（2 理由形态） |
| 禁用优先不呈停止 | 异常 | `running=true` + `disabled=true` → `archi-chat-stop` **不在场**（`AgentInput` 既有规则「整面板禁用不呈停止入口」，design 待决问题 6 的既定牺牲点：运行中无法从面板停止）；`disabled=false` 而 `disabledReason` 非 null → 理由不渲（理由随 `disabled` 才呈） | 表驱动（2 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `AgentInput` | **不 mock**：真实组合（薄适配的可观察后果即 `AgentInput` 的呈面语义） | 全部用例 |
| 回调 | **`vi.fn()` 观测**（`onSend` / `onStop` 参数形态） | 发送 / 停止 |
| IPC / 网络 / 时钟 | **零依赖**（composer 不取数） | —— |

---

### packages/desktop/src/views/architecture/architecture-view.tsx -> packages/desktop/src/views/architecture/architecture-view.test.tsx

#### 待测功能

- `ArchitectureView({ root }: { root: string | null }): React.JSX.Element`: props 面零变化；新增分栏与页面头开关（`archi-chat-toggle`）；`archi-refresh` / `refreshTick` / `key` 重挂语义零变化（公共函数 / API）

> 说明：测试对象取自 design「修改文件」`architecture-view.tsx` 行（整宽页头 + 横向分栏、左栏 `collapsible` / `defaultSize="30"` / `minSize="22"` / `collapsedSize="3rem"` / `panelRef`、折叠真相 = 页面 `useState(true)` + `useLayoutEffect` 挂载收紧一次、不接 `onResize`）、D11 / D12、待决问题 4、AC-8 / AC-9 / AC-13。既有五条用例（页头 / 刷新重发恰一次 / `root=null` 空态 / 全量只读 / 假计时器零取数）语义不变。壳层复核（`app.test.tsx`）并入本节最后一行。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 页面头与分栏结构 | 正向 | 标题「架构」/ 面板开关 `archi-chat-toggle` / `archi-refresh` 三者同在页头（可观察次序：标题 → 开关 → 刷新）；分栏之下左栏 `archi-chat-panel`、右栏 `archi-canvas`；分栏组 `data-slot="resizable-panel-group"` 在场 | 单例 |
| 默认收起 | 正向 | 首拍渲 `archi-chat-rail`、`archi-chat-actions` 缺席（左栏默认收起为窄条）；`useLayoutEffect` 收紧一次（`panel.collapse()` 恰一次） | 单例（含次数） |
| 页面开关切换 | 正向 | 点 `archi-chat-toggle` → 展开体在场（`-actions` / `-session`）、`-rail` 缺席；再点 → 回窄条；state 为唯一真源（两拍互逆，像素无关） | 表驱动（2 拍） |
| 窄条展开 | 正向 | 收起态点 `archi-chat-expand` → 展开体在场（与页面开关同一入口语义，两入口等价） | 单例 |
| 收起态零会话取数 | 边界 | 渲染完成后 `list_archi_chat_sessions` / `session_detail` / `agent_session_transcript` / `agent_session_watch` 调用数全 0；唯一启动 invoke 为 `archi_graph`（画布）；展开面板但不开历史 → 仍零清单 invoke | 单例（逐命令计数） |
| 刷新语义零变化 | 回归 | 点 `archi-refresh` → `archi_graph` 以同 root 重发恰一次（`key` 重挂）；面板展开 / 收起开关不触发 `archi_graph` | 回归 |
| 画布零放宽（AC-9） | 回归 | `mode="full"` → `archi-gate-readonly` 在场、「改档需要开一个 change」文案逐字、`archi-gate-select` / `archi-gate-submit` 缺席、零 `archi_set_gate` 调用 | 回归 |
| 写后不自动刷新（AC-9） | 边界 | 面板侧发送并收口后 `refreshTick` 不变（`archi_graph` 零重发）；假计时器推进 120s 同样零新增（零 watch 零轮询，既有断言扩展为「画布 + 会话双面无轮询」） | 表驱动（2 形态） |
| `root = null` | 异常 | 零 invoke + `archi-canvas-empty` 在场；面板与开关仍在场（可展开，不崩、不白屏） | 单例 |
| 刷新后停摆诊断（AC-8） | 异常 | `archi_graph` 返回 `diagnostics` 非空（模型被写坏）→ 画布呈停摆面板与 `file:line` 清单（与 agent 侧 `archi_module` 同源 `archi::dto` 诊断）；点刷新后从正常态切到停摆态各一次 | 表驱动（2 形态） |
| 壳层复核（`packages/desktop/src/app.test.tsx`） | 回归 | 进入 `#/architecture` 后 invoke 序列按 `filter` 口径复核：收起态下新增面板不引入会话取数，既有序列断言稳定；既有 10 条路由与兜底重定向零变化 | 回归 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| IPC | **mock（进程边界）**：`vi.mock('@tauri-apps/api/core')` 的 `invoke`（`archi_graph` / `archi_module` 两形态 + 会话六命令按名分派）+ 可编程 `Channel` | 全部用例 |
| xyflow 度量 | **mock（环境垫片）**：`ResizeObserver` / `offsetWidth` / `offsetHeight` / `DOMMatrixReadOnly` / `getBBox`（照抄既有先例，非业务 mock） | 画布渲染 |
| 计时器 | **mock（时间控制）**：`vi.useFakeTimers` 仅用于「零轮询 / 零自动刷新」断言 | 零取数用例 |
| `ArchiChatPanel` / `ArchitectureCanvas` / `react-resizable-panels` | **不 mock**：真实组合（折叠真相由页面 state 持有、不依赖 `onResize` 像素派生，jsdom 无布局引擎亦可断言；面板命令面调用次数以 `vi.spyOn` 观测不属 mock） | 全部用例 |
| 网络 | **零依赖** | —— |

---

## 不可测试项

| 模块 / 面 | 原因 |
|---|---|
| `packages/desktop/src-tauri/crates/infra/agent/src/lib.rs` | 仅两行挂载（`mod archi_knowledge;` + `#[cfg(test)] mod archi_knowledge_test;`），无行为面；可达性由 `archi_knowledge_test` 的编译期挂载机械保证（漏声明即 `failed to resolve mod`，`cargo fmt` / `cargo check --all-targets` 报错） |
| `packages/desktop/src/types/generated/bindings.ts`（AC-15） | 生成物（禁手改）；零 diff 门禁由 `bindings:check` 承载，内容断言由 `src/bindings/mod_test.rs` 的重导出用例承载（见 `commands/mod.rs` 章节），不在生成物内部设用例 |
| `packages/desktop/package.json`（AC-15 版本行） | 版本字符串无单测路径；由版本 diff 审查核验（`0.6.0` → `0.6.1`，`src-tauri/tauri.conf.json` 经 `"../package.json"` 跟随、`src-tauri/Cargo.toml` 版本行不随动） |
| `packages/desktop/src-tauri/crates/core/archi/tests/vocabulary_test.rs` 的「知识文本面」（proposal 四面提法） | `archi` **不得**依赖 `agent-runtime`（依赖方向单向 infra → core，含 dev-dependency 亦成环——design 事实校准 3 / 待决问题 1）；知识文本面断言落 `crates/infra/agent/src/archi_knowledge_test.rs`（逐值 `as_str()` 在场 + 零 `<>` + 体量上限），与 `archi` 侧三面 + `ALL`-vs-`as_str` 对拍传递闭合为「四面相等」。该文件本体**零改动**（既有三面用例语义不变） |
| AC-14 的「`crates/**` 零路径字面量扫描」与「三处同值字面量注释互指」 | `foundation/src/layout/mod_test.rs` 的命名隔离扫描只覆盖 `src-tauri/src` 与 `packages/desktop/src`，不覆盖 `crates/**`（既有裁定，MUST NOT 借机改该测试）；`compose.rs` / `knowledge.md` / 前端 hook 的字面量单点由源码审查 + 各自用例（`ARCHI_SOURCE` 断言、md 零目录字面量、invoke 参数取证）承载 |
| AC-15 的 `bindings:check` / `vp check` / `knip` / `cargo clippy -D warnings` | 静态脚本与机械门禁由桌面端执行；本设计只做用例映射，不复制运行命令细节 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/**` 与 `sdk/tools.rs` / `sdk/policy.rs` / `sdk/archi_tool.rs`（AC-6 / AC-7 / AC-8 的零改动面） | proposal「不要修改」与 design 明列零改动面：内置锁面仍恰为工作区 `config.json` 一条（不含架构域路径）、两 archi 工具定义与 policy 只读面不变、工具执行体 root 语义不变。其行为由既有 `rules_test` / `tools_test` / `policy_test` / `archi_tool_test` 回归锚承载，本变更不新增源文件映射 |
| `packages/desktop/src-tauri/crates/core/workflow/**`（16 份相位 prompt md 与 `phase_table_test`） | proposal「不要修改」：知识通道走会话级 `preamble`，MUST NOT 进相位 prompt；16 条 `include_str!` 逐字对拍与「非命中 md 零 archi 字样」两条既有断言因本变更**不变**，沿用既有回归 |
| 零改动能力（`desktop-agent-management` / `desktop-workspace-config` / `desktop-phase-prompts` / `desktop-workspace-store` / `desktop-change-queries` / `desktop-file-watch` / `desktop-page-routing` / `desktop-ipc-type-bindings` / `desktop-checks-domain` / `desktop-test-execution` / `desktop-mutation-testing` / `desktop-release-pipeline` / `desktop-corpus-regression` / `desktop-agent-chat-infra`） | proposal「零改动能力与理由」逐条明列；本变更不改其代码，沿用既有用例断言，不另设章节 |
| `openspec/changes/archi-page-agent-window/**`（proposal / design / tasks / specs / explore） | 文档面，无测试路径（`explore.md` 为只读输入） |
