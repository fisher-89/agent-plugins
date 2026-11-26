# 设计: archi-generator-access-gap

> 变更: `archi-generator-access-gap`（架构生成物的「能力在、通路不在」· 通路补齐 + 契约作者纪律）
> 输入: 本目录 `proposal.md`（定稿）与 `specs/` 下九份 delta（`desktop-archi-agent` / `desktop-write-protection` / `desktop-crate-layout` / `desktop-archi-api` / `desktop-archi-api-check` / `desktop-archi-contract-panel` / `desktop-archi-canvas` / `desktop-phase-prompts` / `desktop-agent-execution`）
> 性质: **一条新通路 + 一条纪律收窄的原子交付**——新增 agent 工具 `archi_generate`（写生成物、不写模型、不写契约）、把 `models/**` 收进相位锁、把「生成物过期」的三处悬空指代换成可执行工具引用，并把提示面 / 知识面的死指代一次清干净。
> 模型面: **零 delta**——`AgentRuntime → Core.Archi` 与 `Shell → Infra.ArchiRuntime` 两条边已在案（`Backend.Core.Archi` 的 `dependedBy` 四条含 `Backend.Infra.AgentRuntime`），本变更不调 `archi_element` / `archi_relation`，`.dev-team/architecture/**` 零字节变化。落笔前只读实测（`archi_drift`）：`status = clean`、`passed = true`、55 元素 / 129 关系、`apiFindings` 37 条全 `docMissing`——本变更不改变这四项读数。

## 架构组件 (Architecture Components)

### C1 · 写盘端口（`core/archi`）

- **职责**：给 agent 面一条**类型级**的写盘缝——声明「把一批应生成物写到某个 root」这一动作，**不实现**它。端口是 agent 面与写盘单点之间唯一的连接件，使 `agent-runtime` 无需依赖 `archi-runtime`。
- **文件**：`packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs`（新增 trait，紧跟 `GeneratedFile` 定义之后）、`crates/core/archi/src/lib.rs`（出口再导出）。
- **依赖**：`std::path::Path`、既有 `GeneratedFile`（`crate::api`）与 `ArchiApiWriteOutcome`（`crate::dto`）。零新外部依赖、零新 workspace 内依赖。
- **技术**：`pub trait ArtifactWriter: Send + Sync`，一方法 `write`。crate 的五条既有纪律（`lib.rs` 模块头）第 5 条「契约面纯读」**零改写**——端口只增类型，`archi` 仍零 `fs::write` / 零 `create_dir_all` / 零 `std::process`。
- **接线**：`agent-runtime` 消费类型（`Arc<dyn archi::ArtifactWriter>`），`src-tauri` 提供实现（C6）。

### C2 · 工具挂载与描述面（`sdk/mount.rs`）

- **职责**：工具面的唯一事实源——`archi_generate` 的名字常量、定义（无参数 + 五句描述）、准入档（`ToolTier::Write`）、执行体分类（`ToolExec::Archi`），以及静态清单 18 → **19** 项；同批改写所有计数与指路措辞。
- **文件**：`crates/infra/agent/src/sdk/mount.rs`。
- **依赖**：`rig::completion::ToolDefinition`、`serde_json`（空 `parameters`）。
- **技术**：定义形 `parameters = {"type": "object", "properties": {}}`（无属性、无 `required`）；清单插入位 = 两 archi 写工具之后、两 DB 只读之前（archi 八项连续成组）。同批计数点：`build_registry` 的 `Vec::with_capacity(18)` → `19`、`registry()` / `build_registry()` 两段 doc 的「十八项 / 十八个字面构造项」、`ToolTier::Write` 的 doc 名单、`ToolExec::Archi` 的「七个 archi 工具（五只读 + 两写）」。
- **指路换面（同文件两处旧句）**：`archi_api` 描述第 ③ 句「过期用既有生成命令重生成」→「过期调 `archi_generate` 重生成」；`ARCHI_API_TOOL` 常量 doc 的「重生成仍由会话经既有 `bash` 命令完成」整句删除并改指 `archi_generate`（`archi-runtime` 仍不对 agent 面可达，写盘改由端口委托）。

### C3 · 生成物执行体与 `remedy`（`sdk/archi_tool.rs`）

- **职责**：`archi_generate` 的执行链三步（渲染 → 逐份保护判定 → 端口写盘）+ 三字段响应装配；`archi_api(check)` 的处置结论 `remedy` 三态投影。
- **文件**：`crates/infra/agent/src/sdk/archi_tool.rs`。
- **依赖**：`archi::workspace_render`（渲染单点）、`archi::display`（repo 相对 POSIX 单点）、`archi::ArtifactWriter`（端口）、`ProtectRules::evaluate_target`（保护判定同一入口）、`tools::cap_result`（L1 上限单点）。
- **技术**：`ArchiContext` 由「root + protect」扩为「root + protect + writer + generate」；`generate: bool` 是 `remedy` 第二 / 第三态的判据位（mint 时按名字清单一次置位，执行体零回读注入面）。渲染 / 比对 / 写盘**零重实现**——执行体只是三个既有单点的串接：

```
archi_generate
  ├─ writer 缺席            → Err（装配缺失，生产不可达）
  ├─ archi::workspace_render(root)            ← 纯读、零写盘、零 spawn；坏模型即 Err 带 file:line
  ├─ 逐份 ProtectRules::evaluate_target(…, file.path)   ← protect 缺席同款 Err（fail-closed）
  │     └─ 任一份命中 ⇒ { written: [], unchanged: [], conflicts: [...] }（整批零写盘，仍 Ok）
  └─ writer.write(root, &files)               → 壳层委托 write_artifacts → { written, unchanged }
```

- **响应形状（工具响应层）**：`{ written: string[], unchanged: string[], conflicts: [{ path, reason }] }`；`ArchiApiWriteOutcome` / IPC DTO / bindings **逐字不变**。
- **`remedy`（`check` 分支）**：在既有 `json!({ artifacts, findings })` 上追加顶层 `remedy`（`null` / 指向工具 / 指向架构页会话），三态由 `artifacts` 三列是否全空 + `self.generate` 决定；`artifacts` 与 `findings` 两轴内容零改动。
- **接线**：`execute` 第八臂 `tools::ARCHI_GENERATE_TOOL => self.generate_payload()`；输出仍过 `tools::cap_result`（零第二份截断）。

### C4 · 会话装配与相位锁（`compose.rs` + `protect/reason.rs`）

- **职责**：`(phase, role)` → archi 工具名清单的矩阵单源（新增 `archi_generate` 两格 + `test-design` 加 `archi_module` + archi 臂七名 → 八名）；模型写权相位锁由「一条」改「**两条**」（模型目录 + 契约资产目录），并给契约目录开**第二份拒串**。
- **文件**：`crates/infra/agent/src/compose.rs`、`crates/infra/agent/src/sdk/protect/reason.rs`。
- **依赖**：`mount::ARCHI_*` 常量、`foundation::layout::{archi_graph_model_rel_dir, archi_models_rel_dir}`（路径派生零新字面量）、`agent::SessionProtectRule`。
- **技术**：锁面第二条规则 `glob = format!("**/{}/**", archi_models_rel_dir())`、`reason = contract_lock_template()`；拒串是**会话类模板的第二个实例**（`ReasonKind::Session(template)` 承载），故 `reason.rs` 模块头「五类文案形态」计数**不变**，既有四串逐字不动。`model_write_lock` 的成因注释整段改写：删「契约资产 md 不锁（`implement/executor` 持有 `archi_api`，改代码相位必须能同步契约）」，如实写「判据在 `implement` 内不再可自愈，实现相位发现契约错的唯一出路是 backtrack」。
- **接线**：`archi_matrix` 两格；`assemble_injections` 的 archi 臂；`injected` 的成员校验与 `protect` 位零改动。

### C5 · 注入链（端口从壳层流到执行体）

- **职责**：把 `Arc<dyn archi::ArtifactWriter>` 从组合根一路送到 per-turn 上下文——**五跳单点** + 一个执行体消费点（提案记为「六处」），每跳一个 builder 缝（与 `with_ask_port` 同型），无分支、无全局。
- **文件**：`crates/infra/agent/src/sdk/runner.rs`、`crates/infra/agent/src/lib.rs`（门面）、`crates/infra/agent/src/compose.rs`、`src-tauri/src/commands/{exec,change_flow}/mod.rs`。
- **依赖**：全链只经 `archi::ArtifactWriter` 这一个类型；`agent-runtime` 的 `Cargo.toml` 零改动。
- **技术**：

```
src-tauri 组合根  Some(Arc::new(ArchiArtifactWriter))
  ▼  compose_turn(…, artifact_writer)
EngineFacade::with_artifact_writer
  ▼  runner_for → .with_artifact_writer
SdkRunner::with_artifact_writer
  ▼  open_session → PumpPayload { artifact_writer }
session_pump → ArchiContext::with_writer(…)
  ▼  LoopTurn.archi
archi_generate 执行体 → writer.write(root, &files)
```

- **铸造判据**：`mint_archi_context` 由「七名任一在场」扩为「**八名**任一在场」，并在同一处置 `generate`；端口缺席（生产不可达）由执行体 `is_error` 回灌，MUST NOT panic。

### C6 · 唯一实现与组合根注入（壳层）

- **职责**：`archi::ArtifactWriter` 的**唯一实现**——薄委托 `archi_runtime::api_artifacts::write_artifacts`（写盘单点），零自有写盘逻辑；两个 `compose_turn` 调用点各构造一份并传入。
- **文件**：`src-tauri/src/commands/archi/mod.rs`（实现）、`src-tauri/src/commands/exec/mod.rs` / `src-tauri/src/commands/change_flow/mod.rs`（注入）。
- **依赖**：`archi`（trait 与 `GeneratedFile` / `ArchiApiWriteOutcome`）、`archi_runtime::api_artifacts`（写盘单点）。
- **技术**：`pub struct ArchiArtifactWriter;` 单元结构 + `impl archi::ArtifactWriter`（函数体一行）；两调用点以 `Some(Arc::new(ArchiArtifactWriter) as Arc<dyn archi::ArtifactWriter>)` 传入（`None` 仅测试调用点用）。零新 IPC 命令、零 `Cargo.toml` 变化。

### C7 · 契约作者纪律的 prompt 面（`crates/core/workflow/src/prompts`）

- **职责**：把「契约作者 vs 契约实现者」的分界写进四份 md——两 planner 拿到 `archi_generate` 的可执行指路（+ `proposal-planner` 的 md 责任句、`test-design-planner` 的 `archi_module` 读接口纪律），`implementation-generator` 的接口同步句**反转**为「不改契约 + 受锁 + 两条出路」。
- **文件**：`proposal-planner.md`、`dev-design-planner.md`、`test-design-planner.md`、`implementation-generator.md`。
- **依赖**：无运行时依赖；措辞与 `desktop-phase-prompts` / `desktop-write-protection` 的 delta 一一对应。
- **技术**：只写「何时用 / 返回什么」两类信息，零路径模板、零字段形态长表；`test-design-planner` 的 4b 禁令（「不得 grep 源文件导出声明或虚构条目」）**逐字保留**，新增边界句只补「接口定义是手写真相，MUST NOT 用 grep 源码导出替代它」（前者读 schema、后者扫源码，两者不冲突）。

### C8 · 知识骨架换指（`infra/agent`）

- **职责**：`archi-knowledge.md` 的「生成物过期用既有命令重生成、MUST NOT 手改」→「生成物过期调 `archi_generate` 重生成、MUST NOT 手改生成物」（逐字见附录 D）；`archi_knowledge.rs` 模块头措辞随动。
- **文件**：`crates/infra/agent/src/archi-knowledge.md`、`crates/infra/agent/src/archi_knowledge.rs`。
- **依赖**：骨架体量锚（骨架 ≤ 1831 B / 全文 ≤ 2031 B，锚不抬）；`include_str!` 编译期内嵌与「尽力增强不炸 run」降级语义零改动。
- **技术**：**单句换句**，零第二条字节增量；`archi_generate` 也在 `mount::registry()` 内，故骨架的「指路不指向不存在工具」断言方向不变（现在是「工具在册 + 知识指它」的正向闭环）。

### C9 · 提示面兜底（`contract-panel.tsx`）

- **职责**：`full` 模式的一行提示去死指代——「生成需开 change（或让左侧会话执行本仓的生成命令）」→「生成需开 change，或让左侧会话生成」。
- **文件**：`packages/desktop/src/views/architecture/components/dashboard/contract-panel.tsx`。
- **依赖**：无（纯文案）。
- **技术**：文案串与 `data-testid` / 类名 / 位次零改动；`full` 分支零 `archiApiGenerate` 的既有断言逐字不变，`incremental` 分支零改动。**画布与面板仍零生成入口**——页内会话的生成能力由 `archi_generate` 承载（`desktop-archi-canvas` 的「合法路径 = 让页内会话生成」由此从空头支票变为可执行）。

### C10 · 版本与改动集边界

- **职责**：`packages/desktop/package.json` 的 `version` 相对落地时 `HEAD` +1 patch；`src-tauri/Cargo.toml` 与 `tauri.conf.json` 逐字不变；改动集 ⊆ 本设计「修改文件」表（含标注的偏离项）。
- **文件**：`packages/desktop/package.json`。
- **技术**：无运行时依赖；版本纪律归 `AGENT.md`，规范内不写死版本号。

## 变更清单 (Change Inventory)

<!-- 新增文件：本变更新增 0 个文件——端口是既有 crate 内的新 trait，实现在既有命令模块内，故「新增文件」子表整段省略。 -->

### 修改文件

| 文件 | 具体修改 | 说明 |
|---|---|---|
| `packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs` | 紧跟 `GeneratedFile` 定义新增 `pub trait ArtifactWriter: Send + Sync`（一方法 `write`，签名见下）；`use crate::dto::ArchiApiWriteOutcome;` 随动；模块头 doc 补一句「端口是纯类型：本 crate 仍零写盘、零 spawn」 | 端口是 agent 面与写盘单点之间唯一的缝（C1） |
| `.../crates/core/archi/src/lib.rs` | 既有 `pub use api::{…}` 名单并入 `ArtifactWriter`；crate 根 doc 第 5 条纪律句补「写盘端口是类型，不破本 crate 只读盘」 | 出口随动（C1） |
| `.../crates/infra/agent/src/sdk/mount.rs` | 常量 `ARCHI_GENERATE_TOOL = "archi_generate"`；`archi_generate_definition()`（空 `parameters` + 五句描述，附录 B 逐字）；`build_registry` 在两 archi 写之后插一项（`ToolTier::Write` / `ToolExec::Archi`）；`Vec::with_capacity(18)` → `19`；`registry()` / `build_registry()` doc 的「十八项 / 十八个字面构造项」→ 十九；`ToolTier::Read` / `Write` 与 `ToolExec::Archi` 的 doc 名单与计数句随动；`archi_api` 描述第 ③ 句换指 `archi_generate`；`ARCHI_API_TOOL` 常量 doc 删「重生成由会话经既有 `bash` 命令完成」 | 定义 / 描述 / 档位唯一真源（C2） |
| `.../crates/infra/agent/src/sdk/archi_tool.rs` | `ArchiContext` 增 `writer: Option<Arc<dyn archi::ArtifactWriter>>` 与 `generate: bool`；构造子 `with_writer(self, Option<Arc<…>>) -> Self`（与 `with_protect` 同型）与 `with_generate(self, available: bool) -> Self`；`execute` 第八臂 + `generate_payload`（附录 E 形态）；`api_payload` 的 `check` 分支加顶层 `remedy`（附录 C）；模块头「五只读 + 两写」→「五只读 + 三写（两模型写 + 一生成物写）」、`execute` doc 臂清单与「七支共用同一 per-turn 上下文」→「八支」 | 写盘经端口委托；零第二次渲染 / 零第二份写盘（C3） |
| `.../crates/infra/agent/src/sdk/runner.rs` | `SdkRunner` 增 `artifact_writer` 字段（`new` 置 `None`）+ `pub fn with_artifact_writer`；`PumpPayload` 同名字段并在 `open_session` 载荷字面量填 `self.artifact_writer.clone()`；`mint_archi_context` 判据七名 → 八名并置 `generate`；`session_pump` 链 `.with_writer(payload.artifact_writer.clone())`；`mint_archi_context` doc 随动 | 注入链第 3/4 跳（C5） |
| `.../crates/infra/agent/src/lib.rs` | `EngineFacade` 增 `artifact_writer` 字段；10 处穷尽 `Self { … }` 字面量补字段（`new` / `with_resume_transcript` 置 `None`，其余传 `self.artifact_writer`）；新增 `pub fn with_artifact_writer`；`runner_for` 的 sdk 臂链上该方法 | 门面是引擎构造唯一 match 点（C5） |
| `.../crates/infra/agent/src/compose.rs` | `compose_turn` 末位增参 `artifact_writer: Option<Arc<dyn archi::ArtifactWriter>>` 并链到门面；`archi_matrix` 两格加 `ARCHI_GENERATE_TOOL`、`("test-design","executor")` 改 `[ARCHI_SEARCH_TOOL, ARCHI_MODULE_TOOL]`；archi 臂 push 第八名；`model_write_lock` 的 `Some` 分支第二条规则（契约资产目录 + `contract_lock_template()`）；模块头 doc（archi 臂「七 archi 名」→ 八、`protect` 位「`Some(一条)`」→「`Some(两条)`」）、矩阵 doc、`model_write_lock` 成因注释整段改写 | 矩阵与写权单源（C4 / C5） |
| `.../crates/infra/agent/src/sdk/protect/reason.rs` | 新增 `CONTRACT_LOCK_TEMPLATE`（附录 A）+ `pub(crate) fn contract_lock_template() -> &'static str`；模块头 doc 补一句「契约串是会话类模板的第二个实例，五类形态计数不变」；既有四串逐字不动 | 第二份拒串（C4） |
| `.../crates/infra/agent/src/archi-knowledge.md` | 单句换指：「生成物过期用既有命令重生成、MUST NOT 手改」→「生成物过期调 `archi_generate` 重生成、MUST NOT 手改生成物」（附录 D）；零其它字节改动 | 骨架是 archi 臂与两写权相位的唯一知识来源（C8） |
| `.../crates/infra/agent/src/archi_knowledge.rs` | 模块头 doc 的「两写工具」→「三写工具（两模型写 + 一生成物写）」；门限表述按实测重冻（数字门限在测试侧，本文件零数字） | 拼装与降级语义零改动（C8） |
| `.../src-tauri/src/commands/archi/mod.rs` | 新增 `pub struct ArchiArtifactWriter;` + `impl archi::ArtifactWriter for ArchiArtifactWriter`（函数体只调 `archi_runtime::api_artifacts::write_artifacts(root, files)` 一次）；模块头 doc 补「agent 工具 `archi_generate` 经端口委托同一写盘单点」 | 唯一实现，零自有写盘逻辑（C6） |
| `.../src-tauri/src/commands/exec/mod.rs` | `compose_turn(...)` 末位传 `Some(Arc::new(ArchiArtifactWriter) as Arc<dyn archi::ArtifactWriter>)`；import 补 `use crate::commands::archi::ArchiArtifactWriter;` | 通用会话（含 `/architecture` 页内会话）的注入点（C6） |
| `.../src-tauri/src/commands/change_flow/mod.rs` | 同上（同型一处） | change 流程相位会话的注入点（C6） |
| `.../crates/core/workflow/src/prompts/implementation-generator.md` | 接口同步句整段反转：① 不改契约（`.schema.json` / `.md` 受相位锁）② 改完代码用 `archi_api` 的 `check` 自查两轴、据 `remedy` 转告用户 ③ 两条出路（改代码追上契约 / 停下述冲突由用户 backtrack）；删「生成物过期用既有命令重生成」与「同批同步接口定义」两类正句 | 契约实现者纪律的唯一载体（C7） |
| `.../prompts/dev-design-planner.md` | 工具行补 `archi_generate`（含「入口不在盘时零产出正常」）；接口面纪律句删「（实现相位同一条纪律）」 | 主战场相位（C7） |
| `.../prompts/proposal-planner.md` | 工具行补 `archi_generate`；补「本 change 涉及元素的 `<叶段>.md` 由本相位写，路径经 `archi_module` 的 `contractPath` 取（勿手拼），且文档不列接口」 | 契约起草相位（C7） |
| `.../prompts/test-design-planner.md` | `archi_search` 行后补 `archi_module` 用法句（取 `assets.interface` 的 `ops` / `types` 键名核对 design 的公共面）+「接口定义是手写真相，MUST NOT 用 grep 源码导出替代它」边界句；4b 禁令逐字保留 | 契约定死之后要看得见接口键名（C7） |
| `packages/desktop/src/views/architecture/components/dashboard/contract-panel.tsx` | `full` 分支文案改「生成需开 change，或让左侧会话生成」；类名 / 位次 / 生成按钮分支零改动 | 去死指代；画布仍零生成入口（C9） |
| `packages/desktop/package.json` | `version` 相对落地时 `HEAD` +1 patch | 版本纪律（C10）；`tauri.conf.json` 经相对路径跟随，`src-tauri/Cargo.toml` 不跟随 |
| `.../crates/infra/agent/src/sdk/tools.rs`（**偏离提案清单，见 D-10**） | 再导出名单补 `ARCHI_GENERATE_TOOL`；`input_path` 的显式 `None` 臂补该名；doc「七个架构工具（五只读 + 两写）」→「八个 archi 工具（五只读 + 三写：两模型写 + 一生成物写）」 | 使「`tools::input_path` 对其恒 `None`」在入参夹带 `path` 时也成立，并防 doc 计数失真 |

### 零改动面（核对结论）

| 对象 | 理由 |
|---|---|
| `.../crates/infra/agent/src/sdk/mod.rs` | 提案列于清单，但该文件只有模块声明——「五只读 + 两写」的措辞实体在 `archi_tool.rs` 模块头（D-9）；本文件零 diff |
| `.../crates/core/archi/src/api/{render.rs,check/**,paths.rs}`、`.../crates/core/archi/src/{parse,edit,drift}/**` | 渲染 / 三类差异比对 / 判据面是单点：`archi_generate` 只**调用**，MUST NOT 复制或分叉 |
| `.../crates/infra/archi/src/api_artifacts.rs` | 写盘单点零改动（只增只改、从不删、非原子语义逐字不变）；本变更只多一个**调用方** |
| `.dev-team/architecture/**`（模型 / 契约资产 / `INDEX.toml`） | 拓扑零 delta（两条既有边已覆盖）；`Backend.Core.Archi.schema.json` 不补 `ArtifactWriter`（`apiUndeclared` 档 `off`，见 Q-1 / R8）；`api:export` 与 `api:check` 应零 diff |
| `openspec/specs/**` 基线 | delta-only：基线里的旧表述（`models/**` ❌ 的锁面行、「MUST NOT 存在专用写工具」、「本仓的生成命令」、「实现相位同一条」）由本 change 的 delta 在归档合并时替换 |
| `src-tauri/Cargo.toml` / `tauri.conf.json`、`.../crates/infra/agent/Cargo.toml` | 零新依赖边（`agent-runtime → archi`、`src-tauri → archi-runtime` 皆既有）；crate 版本行 MUST NOT 跟随 `package.json` |
| `.../crates/infra/agent/src/sdk/protect/{mod,rules,shell,inline}.rs` | 判定面零改动：只多一条**会话规则**（`compose` 注入）与一份**文案**（`reason.rs`）；规则序 / glob 方言 / 写面提取逐字不变 |
| `packages/desktop/src/types/generated/**`、`src/bindings/**` | `remedy` 与生成响应都在工具响应层（字符串），不进 specta 出线面 ⇒ bindings 零 diff |
| `.../crates/tools/archi-api-gen/**` | CLI 驱动面零改动（已是同一单点的另一个调用方） |
| 前端其余产品代码（画布 / 面板结构 / 生成按钮 / 路由） | 零 UI 变化：`full` 模式仍无生成入口，`incremental` 交互逐字不变 |
| 既有测试断言面（矩阵 / 写权 / 计数 / 知识体量 / prompt） | 由测试相位承接；本设计只保证被断言的标识符与计数就位于单点 |

### 公共函数 / API

| 标识符 | 所在文件 | 新增/修改 | 完整签名 | 说明 |
|---|---|---|---|---|
| `ArtifactWriter::write` | `crates/core/archi/src/api/mod.rs` | 新增 | `fn write(&self, root: &Path, files: &[GeneratedFile]) -> Result<ArchiApiWriteOutcome, String>` | 端口唯一方法；纯类型，core 零实现、零 IO |
| `compose_turn` | `crates/infra/agent/src/compose.rs` | 修改 | `pub fn compose_turn(stores: &WorkspaceStores, registry: Arc<StopRegistry>, streams: Arc<SessionStreamRegistry>, ask_registry: Arc<agent::AskRegistry>, steer_registry: Arc<agent::SteerRegistry>, store: Arc<Store>, agent: Option<i64>, workspace_root: &Path, ask_port: Option<Arc<dyn orchestration::port::AskPort>>, artifact_writer: Option<Arc<dyn archi::ArtifactWriter>>) -> Result<ComposedTurn, String>` | 末位增参（组合根注入缝）；两产品调用点传实现，既有测试调用点传 `None` |
| `EngineFacade::with_artifact_writer` | `crates/infra/agent/src/lib.rs` | 新增 | `pub fn with_artifact_writer(self, writer: Option<Arc<dyn archi::ArtifactWriter>>) -> Self` | builder 缝，与 `with_ask_port` 同型 |
| `ArchiArtifactWriter::write` | `src-tauri/src/commands/archi/mod.rs` | 新增 | `fn write(&self, root: &Path, files: &[archi::GeneratedFile]) -> Result<archi::ArchiApiWriteOutcome, String>` | `archi::ArtifactWriter` 的唯一实现；函数体只调 `write_artifacts` 一次 |

> 内部缝（`pub(crate)`，不计入公共 API）：`SdkRunner::with_artifact_writer`、`ArchiContext::{with_writer, with_generate}`、`generate_payload`、`contract_lock_template`、`mint_archi_context` 的八名判据。

### 类型定义

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|---|---|---|---|
| `ArtifactWriter` | `crates/core/archi/src/api/mod.rs` | 新增 | `pub trait ArtifactWriter: Send + Sync`；一方法 `write`；零新 DTO、零 specta 出线 |
| `ArchiArtifactWriter` | `src-tauri/src/commands/archi/mod.rs` | 新增 | `pub struct ArchiArtifactWriter;`（零字段）；`impl archi::ArtifactWriter` |
| `ArchiContext` | `crates/infra/agent/src/sdk/archi_tool.rs` | 修改 | 增 `writer: Option<Arc<dyn archi::ArtifactWriter>>`（生成物写盘端口）与 `generate: bool`（`remedy` 判据位）；既有 `root` / `protect` 语义不变 |
| `SdkRunner` / `PumpPayload` / `EngineFacade` | `sdk/runner.rs` / `infra/agent/src/lib.rs` | 修改 | 各增 `artifact_writer: Option<Arc<dyn archi::ArtifactWriter>>` 字段（注入链载荷） |
| `ArchiApiWriteOutcome` / `ArchiApiCheck` / `SessionInjections` / `SessionProtectRule` / `ToolMount` / `ToolTier` / `ToolExec` | `core/archi/src/dto.rs` / `agent` / `sdk/mount.rs` | 零改动 | 形态零变化：响应三字段与 `remedy` 只在执行体拼 JSON；锁面第二规则复用既有 `SessionProtectRule`；档位 / 执行体枚举零新变体 |

### 配置

| 配置键 | 所在文件 | 新增/修改 | 值类型 | 默认值 | 说明 |
|---|---|---|---|---|---|
| `version` | `packages/desktop/package.json` | 修改 | `string` | 落地时 `HEAD` 现值（预期 `0.7.21`） | patch +1（`AGENT.md` 版本纪律）；**零配置键变化**——架构域 `config.json` 的 `ignore` / `exclude`、工作区 `write_protection` 面逐字不变，本变更不新增任何配置项 |

## 数据模型 (Data Model)

### 磁盘资产（形态零变化）

| 资产 | 落点 | 本变更的角色 |
|---|---|---|
| `INDEX.toml` | `<archi_graph_dir>/`（全局唯一一份） | **生成物**（写盘对象，形态 / 落点 / 孤儿口径零变化） |
| `<入口主干>.i.rs` / `.i.ts` | 元素实现入口同级 | **生成物**（同上；`.i.*` 的渲染与落点逻辑一行不改） |
| `<叶段>.schema.json` | `<archi_models_dir>/<分层…>/` | **手写真相**（不是生成物；本变更不写它，`archi_generate` 射程外） |
| `<叶段>.md` | 同上 | 文档（理由；本变更不写它） |

### 写盘语义（`write_artifacts` 唯一实现，零改动）

- **只增只改、从不删**：孤儿生成物只由检查面报告，生成器不清理。
- **字节相同即 `unchanged`**：不重写、`mtime` 不动（幂等的唯一承载）。
- **非原子**：单份失败即 `Err` 且已写部分不回滚（工具面在到达 `write` 之前已整批过保护闸，故失败面只剩 IO）。
- **越根复核**：第二层拦截（第一层在 `archi::api::paths` 的形态校验），逐字沿用。

### 会话级锁面（内存纯数据，不持久化）

`SessionProtectRule { glob, reason }` 两条（挂锁格）：`**/<模型声明目录>/**` + `**/<契约资产目录>/**`；两串模板各自单点（`session_lock_template()` / `contract_lock_template()`）。不挂锁格仍 `None`。`load` 的签名与「唯一 IO 点」语义不变。

### 工具响应（agent 面，不持久化、不出线 IPC）

| 响应 | 形状 | 位置 |
|---|---|---|
| `archi_generate` | `{ written: string[], unchanged: string[], conflicts: [{ path: string, reason: string }] }` | 执行体 `json!` 拼装（`ArchiApiWriteOutcome` 只提供 `written` / `unchanged` 两字段，`conflicts` 是工具层追加） |
| `archi_api(check)` | 既有 `{ artifacts, findings }` + 顶层 `remedy: string \| null` | 同上 |

### 持久化

零新增：无新文件格式、无新 schema、无 DB 表 / 字段、无生成物新增类别。版本字段唯一变化是 `package.json` 的 `version`。

## 路由 / API 设计 (Route / API Design)

### IPC 命令面（零新增、零改动）

| 命令 | 变化 | 说明 |
|---|---|---|
| `archi_api_generate(root: String)` | 零改动 | 签名与 `ArchiApiWriteOutcome` 出参逐字不变；仍是**人类可提交入口**（仅 `incremental` 面板调用） |
| `archi_api_check` 等其余八条 | 零改动 | 九命令清单与签名零变化；`remedy` 不进出线面 |

### agent 工具面（新增一条、改一条响应）

| 工具名 | 入参 | 响应 | tier | exec | 下发面 |
|---|---|---|---|---|---|
| `archi_generate` | 无（`parameters` 零属性；root 取 per-turn 会话 cwd） | `{ written, unchanged, conflicts }` | `Write` | `Archi` | `(proposal, executor)` / `(dev-design, executor)` / archi 来源臂 |
| `archi_api` | 不变（`action` / `element` / `from` / `to` / `name`） | 既有两轴 + 顶层 `remedy`（三态） | `Read`（不变） | `Archi` | 不变（proposal / dev-design / implement 的执行侧 + archi 臂） |

### 下发矩阵（archi 工具面，单源 `archi_matrix`）

| (phase, role) | `archi_module` | `archi_search` | `archi_api` | `archi_drift` | `archi_scaffold` | `archi_element` | `archi_relation` | `archi_generate` |
|---|---|---|---|---|---|---|---|---|
| `(proposal, executor)` | ✅ | ✅ | ✅ | ❌ | ✅ | ✅ | ✅ | **✅** |
| `(dev-design, executor)` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | **✅** |
| `(dev-design, evaluator)` | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| `(test-design, executor)` | **✅（新增）** | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| `(implement, executor)` | ✅ | ❌ | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |
| `(code-review, evaluator)` | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| `(acceptance, evaluator)` | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| 其余相位 / 畸形 `source_ref` | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| `source = "archi"` 臂 | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | **✅** |

### 相位锁面（会话规则，按 `(source, phase, role)`）

| 目标 | 挂锁格 | 拒串 |
|---|---|---|
| `<archi_dir>/graph/model/**` | 除 archi 臂与 `(proposal\|dev-design, executor)` 外全部 | `session_lock_template()`（逐字不变） |
| `<archi_dir>/models/**`（`.schema.json` + `.md`） | 同上（本次新增的第二条规则） | `contract_lock_template()`（新增，见附录 A） |
| `<archi_graph_dir>/INDEX.toml`、`.i.*`、`<archi_dir>/config.json` | 不锁 | 生成物 / 看护配置不在锁面（既知情边界） |

## 依赖 (Dependencies)

### 运行时依赖

| 依赖 | 变化 | 用途 |
|---|---|---|
| `agent-runtime → archi` | 零新增（既有边） | 工具执行体消费 `archi::dto` 投影 + 本次新增的 `archi::ArtifactWriter` 类型 |
| `src-tauri → archi-runtime` | 零新增（既有边） | `ArchiArtifactWriter` 委托 `api_artifacts::write_artifacts`（写盘单点） |
| `agent-runtime ↛ archi-runtime` | **逐字不变** | 写盘单点对 agent 面仍不可达；agent 面只持类型 |
| `std::sync::Arc` + trait object | 标准库 | 端口注入（`Arc<dyn archi::ArtifactWriter>`），零新 crate |

结论：**零新增依赖边、零新增 crate、零 Cargo 清单改动**；`write_artifacts` 仍只 `infra/archi` 一处实现。

### 构建 / 测试依赖

零新增。测试侧复用既有 `tempfile`（临时根对拍）与既有 `loop` / `compose` 夹具；不引入新 dev-dependency。

## 决策留痕

| # | 决策 | 备选与否决理由 |
|---|---|---|
| D-1 | `archi_generate` 是**新工具**，不是 `archi_api` 的第 4 动作 | 档位是**挂载项级**（`ToolMount { definition, tier, exec }`）：`archi_api` 含写动作即整只升 `Write`，在只读下发的相位（`dev-design/evaluator` / `code-review` / `acceptance`）被 `default` 档一并拒，`check` / `bootstrap` / `link` 陪葬 |
| D-2 | 端口落 `core/archi`、实现落壳层注入 | ① 让 `agent-runtime` 直接依赖 `archi-runtime` 撞 `desktop-crate-layout` 明文；② 在 `infra/agent` 再造「渲染 + 写盘」是第二份写盘实现；③ `write_artifacts` 上移 core 破坏「core 全程零写盘」 |
| D-3 | `implement` **不给** `archi_generate` | 生成物的渲染输入只有 schema，代码变化不影响生成物 ⇒ 契约冻结的 implement 期生成物恒新鲜；给了只会让「代码变了 ⇒ 生成物旧了」的假象进门禁 |
| D-4 | L3 硬闸（锁 `models/**`）+ **第二份**拒串 | 复用 `SESSION_LOCK_TEMPLATE` 三重误导（说错对象 / 动作 / 出路）；只改 prompt 不加锁 = 把纪律交给概率（implement 是低档模型，会选「改 schema 迎合代码」） |
| D-5 | `remedy` 走**工具响应层**，不进 DTO | `ArchiApiCheck` 是 specta 出线类型：加字段要连带改 `bindings.ts` / `bindings.zod.ts` / 前端 / `mod_test` 键集断言——为一个给模型看的字符串付三处代价；agent 面与人类面本就不同源 |
| D-6 | `conflicts` 非空 ⇒ 整批零写盘、工具仍 `Ok` | 与 IPC 的「逐份写、失败点之前已写」语义**刻意不同**：工具面比 IPC 更严（起草期 agent 无回滚能力）；冲突是「要转告用户处置」的事实，不是 run 的错误 |
| D-7 | `remedy` 第二 / 三态的判据位落 `ArchiContext.generate` | mint 时按名字清单一次置位（与八名铸造判据同处），执行体零回读注入面、零新 DTO 字段、零全局 |
| D-8 | 注入走**单点 builder 链**（五跳） | 漏一处则生产路径 writer 恒 `None`；与 `with_ask_port` 同型可机械核对；端口缺席由执行体 `is_error` 兜住（R9） |
| D-9 | **偏离提案**：`sdk/mod.rs` 零改动 | 提案列的「执行体模块头注释（五只读 + 两写）」实体在 `archi_tool.rs:1`；`sdk/mod.rs` 只有模块声明，改它等于造无用 diff |
| D-10 | **偏离提案**：`sdk/tools.rs` 增 2 行 + doc | 使 `desktop-archi-agent` 的「`tools::input_path` 对其恒 `None`」与「沙箱改写面原样返回」在**入参夹带 `path`** 时也成立（否则幻觉入参会走通用臂并被沙箱改写 / 拒绝），并防该文件「七个架构工具」doc 失真。代价：AC-13 的文件集 +1。回退方案（若归档面坚持逐字对拍）：`archi_tool.rs` 直引 `mount::ARCHI_GENERATE_TOOL`，接受该 doc 计数失真 |
| D-11 | 知识换句取全串「…、MUST NOT 手改**生成物**」 | delta spec 的断言 needle 逐字含「手改生成物」；字节增量如实记账（骨架 1808 → ~1823 B、全文 1997 → ~2012 B），**门限不抬**（1831 / 2031），实测值在测试相位重冻 |
| D-12 | `compose_turn` 增**末位可选参数**（`Option<Arc<dyn ArtifactWriter>>`） | 端口不得不穿此缝（`archi-runtime` 对 agent 面不可达）；`Option` 与 `with_ask_port` 同型，测试调用点只需补 `None`；代价：约 58 处既有测试调用点机械补参（零断言变化，R9 的兜底之一） |
| D-13 | 端口缺席 / `protect` 缺席一律 `Err`（`is_error`） | 与既有 `commit` 的「生产不可达」同款；MUST NOT panic、MUST NOT 静默成功（否则「写保护未生效」与「没有规则」不可辨） |
| D-14 | `reason.rs` 的「五类文案形态」计数**不变** | 契约串仍是 `ReasonKind::Session(template)` 的一个模板实例，不新增 `ReasonKind` 变体；顺手加第六类会让判定面与文案面同时漂移 |
| D-15 | 生成不新增锁机制 | 单用户单会话，与 IPC 生成同款语义；并发安全由「同一进程内写同一批文件」的既有形态承担，幂等由字节比对承担（AC-15） |
| D-16 | `archi_knowledge.rs` 不动数字门限 | 该文件本就零数字（门限冻结在测试侧）；提案的「门限表述更新」在此只落为「两写工具 → 三写工具」一行措辞 |

## 验收标准回应

| AC | 落点（设计面） |
|---|---|
| AC-1 工具在册且档位正确 | C2（常量 / 定义 / `ToolTier::Write` / `ToolExec::Archi` / 19 项）；五只读与两模型写的档位行零改动 |
| AC-2 矩阵逐格与收窄后不变量 | C4（`archi_matrix` 两格 + `test-design` 加名 + archi 臂八名）；写权对拍集合恰 `{archi_element, archi_relation}`（`archi_generate` 不入对拍，两方向各有断言位） |
| AC-3 生成链三态 | C3（三步执行链）+ C5（`root` = 会话 cwd ⇒ change 会话写 worktree）+ D-6（冲突整批零写盘） |
| AC-4 端口分层与写盘单点 | C1（纯类型）+ C6（唯一实现只调 `write_artifacts` 一次）+ 依赖节（`agent-runtime ↛ archi-runtime` 零变化） |
| AC-5 零产出与孤儿如实 | C2 描述第 ②③④ 句（附录 B）+ C3（零产出由 `render_all` 的既有跳过分支承担——`path` 空 / 无 `<叶段>.schema.json` / 接口面留白 / 入口未解析；工具不报错、不删文件） |
| AC-6 `remedy` 三态与零出线面 | C3（附录 C）+ D-5（不进 DTO / bindings） |
| AC-7 L3 锁面与拒串 | C4（第二条规则 + `CONTRACT_LOCK_TEMPLATE`，附录 A）+ 相位锁面表 |
| AC-8 生成物与模型锁面分离 | C4（只加 `models/**` 一行）+ 零改动面（内置清单仍恰 `config.json` 一条） |
| AC-9 悬空指代零残留 | C2（`archi_api` 描述 + 常量 doc）+ C8（知识换指）+ C7（prompt 四处）+ C9（面板文案） |
| AC-10 契约作者纪律四处生效 | C7（四份 md 的三要素 + 两处删除项 + 4b 禁令保留） |
| AC-11 测试面必改断言全绿 | **由测试相位承接**；设计只保证被断言的名字 / 计数就位于单点（`mount.rs` / `compose.rs` / `reason.rs` / `archi-knowledge.md`） |
| AC-12 零回归读数 | 零改动面（`api_artifacts` / 渲染 / 比对 / 模型零碰）；`archi_generate` 只调用既有单点 ⇒ 本仓生成物零 diff、`.dev-team/architecture/**` 零字节 |
| AC-13 版本与改动集 | C10 + 「修改文件」表；AC-13 的「改动集 ⊆ 提案清单」按该表读——相对提案清单**少一项**（`sdk/mod.rs`，D-9）、**多一项**（`sdk/tools.rs`，D-10，取舍见 Q-3） |
| AC-14 spec delta 形态 | 本 change `specs/` 九份 delta 已定稿；设计不改 spec 文本，落地时逐条对齐 |
| AC-15 幂等与并发安全 | 数据模型「写盘语义」（字节比对承担幂等）+ D-15（不新增锁机制） |
| AC-16 画布仍只读 | C9（文案一行）+ 零改动面（`full` 分支零 `archiApiGenerate` 断言逐字不变） |

## 待决问题 (Open Questions)

| # | 问题 | 现状 / 建议 |
|---|---|---|
| Q-1 | `Backend.Core.Archi.schema.json` 是否为本轮新增的 `ArtifactWriter` 补 `x-class` | 提案裁决为**不补**（R8）：`apiUndeclared` 档 `off`（不产判据、不进 `status`、不影响 `passed`），补它会把 dev-design 的模型写面拖进本轮。若 dev-design 后续裁定必须补，走该相位的模型写面并在 `design.md` 留痕（MUST NOT 在 implement 相位顺手补） |
| Q-2 | proposal R12 / dev-design D11 两条「md 责任」checklist 判据何时上 | 本轮**缓议**：判据只判产物、判不到「谁改的」；先观察指标——`apiFindings` 本次只读实测 **37 条、全 `docMissing`**（提案记 38，AC-12 已声明「落地时实测为准」）；若「新 change 的 `docMissing` 只增不减」则 prompt 未被执行 |
| Q-3 | D-10 是否接受（`sdk/tools.rs` 进改动集，AC-13 文件集 +1） | 建议接受（理由见 D-10）；若拒绝，回退方案与残留（doc 计数失真 + 幻觉 `path` 走沙箱）已写明，需在同一处留痕 |

## 附录 A · `CONTRACT_LOCK_TEMPLATE` 草案（`protect/reason.rs`）

```
该路径受相位锁保护：%s（经 %t 工具）。
接口定义（.schema.json）与模块文档（.md）只在 proposal / dev-design 相位的执行侧
（以及架构页会话）写入；实现相位只实现契约、不改契约。
若代码与契约不符：先确认哪一方是对的——契约对就改代码追上它；
契约确实不可实现或写错，就停下来说明冲突，由用户决定走 backtrack 回到 dev-design 修订；
或请用户改用架构页会话直接维护。
不要改用其他命令或工具尝试绕过（所有写入路径均被拦截）。
```

四要素齐备（事实 + 工具名 / 政策 / 三条出路 / 不得绕过），`%s` / `%t` 全量替换；与 `SESSION_LOCK_TEMPLATE` 逐字不同。

## 附录 B · `archi_generate` 工具描述草案（五句，`mount.rs`）

| # | 语义 | 草案 |
|---|---|---|
| ① | 用法与响应 | 按当前模型的契约渲染并写入三类生成物（架构域 `INDEX.toml`、入口同级 `.i.rs` / `.i.ts`）：幂等（字节相同即不重写、`mtime` 不动），无参数（root 取本会话 cwd），返回 `{ written, unchanged, conflicts }` |
| ② | 零产出 | 入口未解析 / 接口面留白（`$defs` 与 `properties` 皆空）/ 未写 `.schema.json` 的元素不产生生成物——零产出是正常，不是失败 |
| ③ | 孤儿 | 生成器从不删除孤儿生成物，`orphaned` 非空属常态；MUST NOT 为了清空它去手改生成物 |
| ④ | 覆盖风险 | 生成会覆盖同名 `<入口主干>.i.*`（含手写文件）：调用前先 `git status` 确认工作树干净，以便意外可 `git restore` 回滚 |
| ⑤ | 冲突处置 | `conflicts` 非空即被写入保护拒绝、整批零写盘；把冲突（路径 + reason）报告给用户处置，MUST NOT 换工具重试、MUST NOT 手改生成物绕过 |

约束：零本仓命令名（`api:export` 一类零命中）；MUST NOT 把 `.schema.json` 说成生成物。

## 附录 C · `archi_api(check)` 的 `remedy` 三态草案

| 条件 | 值 |
|---|---|
| `mismatched` / `orphaned` / `missing` 三列全空 | `null` |
| 有差异 ∧ `ArchiContext.generate == true` | 「生成物有差异：调 `archi_generate` 重生成（幂等、只写三类生成物）；MUST NOT 手改生成物」 |
| 有差异 ∧ `ArchiContext.generate == false` | 「生成物有差异：本相位无 `archi_generate`——请用户在架构页会话生成；MUST NOT 手改生成物」 |

键恒在场（`null` 也出键）；`artifacts` 与 `findings` 两轴内容不受影响；短串，不改 `cap_result` 上限。

## 附录 D · 知识骨架换句（`archi-knowledge.md`，单句）

- 现文：生成物过期用既有命令重生成、`MUST NOT 手改`；
- 新文：生成物过期调 `archi_generate` 重生成、`MUST NOT 手改生成物`；

字节账：骨架 ≈ 1808 + 15 = 1823 B（≤ 1831）、全文 ≈ 1997 + 15 = 2012 B（≤ 2031）；门限不抬，实测值由测试相位重冻。零其它字节改动（「读五件 / 写两件」行不动——`archi_generate` 已在同句点名）。

## 附录 E · `generate_payload` 形态（`archi_tool.rs`，示意）

```rust
/// `archi_generate`：渲染（纯读）→ 逐份过同一 `ProtectRules` → 端口委托写盘。
///
/// 端口 / `protect` 缺席即 `Err`（生产不可达——装配侧恒接线）；任一份命中保护即整批
/// 零写盘、冲突进 `conflicts`（工具仍 `Ok`：冲突要 agent 转告用户）。
fn generate_payload(&self) -> Result<Value, String> {
    let writer = self.writer.as_deref().ok_or_else(|| { /* 装配缺失文案 */ })?;
    let files = archi::workspace_render(&self.root).map_err(|error| error.message())?;
    let rules = self.protect.as_ref().ok_or_else(|| { /* 保护规则未装配文案 */ })?;
    let conflicts: Vec<Value> = files.iter()
        .filter_map(|file| rules.evaluate_target(tools::ARCHI_GENERATE_TOOL, &file.path)
            .map(|reason| json!({ "path": archi::display(&self.root, &file.path), "reason": reason })))
        .collect();
    if !conflicts.is_empty() {
        return Ok(json!({ "written": [], "unchanged": [], "conflicts": conflicts }));
    }
    let outcome = writer.write(&self.root, &files)?;
    Ok(json!({ "written": outcome.written, "unchanged": outcome.unchanged, "conflicts": [] }))
}
```
