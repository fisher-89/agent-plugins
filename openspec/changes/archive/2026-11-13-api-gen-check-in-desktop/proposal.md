# 提案: api-gen-check-in-desktop

> **变更**: api-gen-check-in-desktop
> **日期**: 2026-11-13
> **状态**: draft
> **来源**: `openspec/changes/api-gen-check-in-desktop/explore.md`（requirement 类型，结论稿 + 六项已裁决汇总 + 两条硬线）
> **性质**: 把「契约生成物」的**检查**与**生成**两面从 CLI 单点接进桌面端：判定与比对留在 `core/archi`（纯读盘），写盘单点搬 `infra/archi`（P1），壳层加两条 IPC，前端左栏加第三视图「契约」，并把 `apiFindings` 从「报告里无人看」变成「状态词 + 面板 + agent 自检」三面可见。另含一条 `agent` 只读工具 `archi_api` 与 `apiFindings` 进 `status` 的纳管口径收紧（`passed` 不动）。

---

## 问题

### 现状链路（读过的真实代码）

```
手写真相  <archi_api_dir>/<Domain>/<FQN>.md
   │ core/archi::api::contract 解析（load_contracts → 双向 gate → $defs 规则）
   ▼
内存四类生成物  api::render_all() -> Vec<GeneratedFile>     ← core 纯读盘、零写盘
   │                                  │
   │                    ┌─────────────┴──────────────┐
   ▼                    ▼                            ▼
tools/archi-api-gen::main.rs             api::check()（十条判据，core）
  generate → write()（唯一写盘面）         → drift::check 报告 apiFindings
  --check → differences() + walker       → archi_drift 命令 + LocalArchiCheck 门禁
  bootstrap → api::bootstrap             → 前端：apiFindings 零消费
```

三处真实病灶：

1. **比对面住在工具里**：`differences()` 与 `generated_files_on_disk()`（含手写递归 walker、`SKIPPED_DIRECTORIES`、`MAX_MODULE_WALK_DEPTH`）在 `crates/tools/archi-api-gen/src/main.rs`，**不在 core**。任何第二消费面（IPC / 门禁 / agent 工具）出现前必须上移，否则「判定单源」当场破——本仓硬纪律要求命令面与门禁面同源。
2. **写盘面住在工具里且被禁依赖**：`desktop-crate-layout` 明写 `archi-api-gen` MUST NOT 被 app / core / infra 依赖 ⇒ app 今天**无法**自己生成，只能提示用户跳出去跑 `pnpm api:export`。
3. **`apiFindings` 只进报告不进状态词**：本仓实测 41 条判据（`apiEntryMissing` 28 / `apiMissing` 13）在 `archi_drift` 报告里躺在并列槽位 `apiFindings`，前端一次都没读过，`status` 推导的四个入参里也没有它——「债」只在人主动开报告时才可见。

另有一处易误判的事实：**零 diff 判定其实已经在门禁里**——`config.static_analysis = "pnpm run -C ./packages/desktop check"`（`server:check → bindings:export → api:check → client:check`），随 static-check 步 `passed=false` 硬拦。真正缺的不是「一道门」，而是**分轴与可读**：agent 现在收到「静态检查失败」+ 一段混在 lint 噪音里的 CLI 单行 JSON，无法区分「代码 lint 红了」与「生成物过期了」。

### 五条要解决的具体症状

1. 架构页看不到「生成物是否新鲜」——无清单、无计数、无法在应用内消除。
2. 生成物过期的反馈只有一行「静态检查失败」，与 lint 失败不可辨。
3. 契约判据（41 条）不可见、不进状态词，`api = declared` 的元素「自己承诺的东西没兑现」与「从没承诺」两件事在报告里同权。
4. agent（尤其 implement/executor）改完代码无法自查生成物新鲜度，只能等门禁打回并在噪音里翻。
5. `archi_module` 已给 `api` 三路径（`ArchiApiRef`），前端从未读过——抽屉里没有任何契约入口。

---

## 提案

### 一、能力取舍：A + B + D（C 本轮不做）

| 面 | 内容 | 本轮 |
|---|---|---|
| **A** 判定面进 UI | `apiFindings` 按 `category` 分组呈现在左栏「契约」视图 + 元素抽屉「契约」段；画布**零契约标记**（`desktop-archi-canvas` 明写节点标记不呈现，`drift-marks.ts` 只产边标记） | ✅ |
| **B** 生成 + 检查进 IPC | 比对面上移 `core/archi::api`（纯读盘）、写盘单点搬 `infra/archi`（P1）、壳层加 `archi_api_check` / `archi_api_generate` 两条命令、前端面板三态 + 生成交互 | ✅ |
| **C** 进工作流门禁步 | 把「生成物过期」从 `static_analysis` 里拆成独立 `StepNode`（独立 feedback 计数） | ❌ 但 B/D 的实现形态须让 C 将来能直接复用：**check 判定是纯函数**（`api::compare`），而不是命令内联 |
| **D** agent 工具面 | 新增只读工具 `archi_api`（`check` / `bootstrap` 两动作），生成仍由会话经 `bash` 跑既有命令 | ✅ |

### 二、比对面与写盘面的归属（裁决 P1）

| | 形态 | 结论 |
|---|---|---|
| P1（裁决） | 写盘单点搬 `infra/archi`（与 `report.rs` 报告落盘同型先例）；`archi-api-gen` 加 `archi-runtime` 依赖改调它 | 写盘函数单点，两驱动面（CLI / IPC）同源 |
| P2 | 壳层自写一份 `write` | 出局：写盘是最危险动作（越根复核 + 批量覆盖），两份实现 = 两份安全判据靠人肉同步 |
| P3 | app 只做 check + 「复制 `pnpm api:export`」 | 出局：B 只兑现一半 |

落地形态（逐层，零新增外部包）：

```
core/archi/src/api/render.rs    workspace_render(root) -> Result<Vec<GeneratedFile>, ApiError>
                                收编 CLI 的 collect（graph_view → load_contracts → render_all）
core/archi/src/api/diff.rs      compare(root) -> Result<ArtifactDiff, ApiError>
                                ArtifactDiff { mismatched[], orphaned[], missing[] }（repo 相对串，
                                经 archi::display）；收编 CLI 的 differences + generated_files_on_disk，
                                walker 改用已依赖的 ignore（gitignore 感知）⇒ 消一套手写递归
infra/archi/src/api_artifacts.rs write_artifacts(root, &[GeneratedFile]) -> Result<ArchiApiWriteOutcome, String>
                                收编 CLI 的 write()：starts_with(root) 越根复核（第二层）+ create_dir_all
                                + 先读盘比对再写 ⇒ written / unchanged 两列；**从不删文件**
tools/archi-api-gen/src/main.rs  只剩 argv 解析 + 退出码 + 三子命令编排；零 fs 写调用
```

### 三、两条 IPC（八命令时代的第 7、8 条）

```
archi_api_check(root)    -> ArchiApiCheck   零写盘
  { enabled, meta { toolVersion, commonDigest }, mismatched[], orphaned[], missing[] }
archi_api_generate(root) -> ArchiApiWriteOutcome   写盘
  { written[], unchanged[] }
```

- **check 即生成预览**：三类差异正好是「点生成会写什么」的完整清单 ⇒ UI 次序 = 先看清单 → 再点生成。
- **generate 从不删文件**（CLI 已核实：`Command::Generate` 只写不删，orphan 仅报告）⇒ 写盘危险等级低一档，但**非原子**（逐份 `fs::write`，中途失败 = 部分写入）须如实呈现。
- **停摆安全**：模型坏 ⇒ `graph_view` 结构诊断非空 ⇒ `Err`（与 CLI `collect` 同口径），零写盘；不会因「应生成集为空」误删（根本不删）。
- **`enabled` 只作呈现位**：`enabled = false`（无契约目录 / 零份 md）时 check 仍照跑（应生成集只含 `INDEX.toml` ⇒ `missing` 就那一条），面板 MUST NOT 因 `enabled = false` 渲成空面板。

### 四、`apiFindings` 进 `status`（裁决 ③ 「契约在案即纳管」）

| 口径 | 后果 | 结论 |
|---|---|---|
| ① 完全不参与（现状） | 41 条判据纯装饰 | 出局 |
| ② severity 折算进 status | 28 + 13 ⇒ status 恒 `warnings`，15 份金样全改，warnings 语义被常规化稀释 | 出局 |
| ③ 只取「契约文件在案元素」的判据参与 status | 本仓仅 1 个试点元素且 findings 0 ⇒ **本仓 status / 门禁零变化**；契约逐份落地即自激活，无白名单无开关 | **采纳** |

- 纳管子集 SHALL 由 `core/archi` 的单一函数派生（「契约文件在案」判据与 `ArchiApiRef` 同源：`paths::contract_path(...).is_file()`），`drift` 侧 MUST NOT 自己判 `is_file`。
- `status` 推导序**只增加一个条件**：`warnings` 分支由「`warnings[]` 非空」扩为「`warnings[]` 非空 **或** 纳管子集非空」；其余四值语义与先后序逐字不变 ⇒ `status == errors ⟺ passed == false` 恒成立。
- **`passed` 本轮不动**：`apiFindings` 永不产生 `errors` 槽 ⇒ 门禁布尔零变化；收紧路径（某类判据改 error 档 + 改 `passed` 判据两处同批）留独立变更。
- **`REPORT_VERSION` 2 → 3**：字段零改但**推导输入改了**，`status` 语义随版本分界 ⇒ 递增并留痕（消费方按 `version` 分支；`version = 2` 的报告 `status` 不读契约判据）。

### 五、UI：左栏分段控件第三模式「契约」

```
CanvasBody
  ├─ 左栏（分段控件 文件 | 元素 | 契约）  → FileTree / ElementTree / ContractPanel
  ├─ ReactFlow 画布（三种左栏视图下都保留）
  └─ ElementDrawer（右抽屉；「契约」段插在概览与文档之间）
```

- **两轴并列、各自计数，MUST NOT 混成一个「问题总数」**：上轴 = 生成物新鲜度（可机器修，来源 `archi_api_check`）；下轴 = 契约 vs 代码（要人或 agent 修，来源既有 `archi_drift` 报告的 `apiFindings`，父级已取数、零新 invoke）。
- **`category` 六值做筛选 chip**（entry / contract / signature / reexport / nesting / constructor）——词汇已在报告里，前端零新词表。
- **条目点击 = 聚焦**（复用既有 `onFocus`：元素树 / 文件树都这么干）；「打开」走既有 `FileViewer` + 通用 `read_file`。
- **状态矩阵全画**：`loading` / `enabled=false`（冷启动说明）/ 零差异零判据 / invoke reject 行内错误 / 生成失败错误回显。
- **抽屉契约段零新取数**：`ArchiModuleDetail.api`（`ArchiApiRef` 三路径）早已在返回里；本元素判据从父级同一份 `apiFindings` 按 `element === fqn` 过滤。

### 六、生成权限：U1 + D（既有只读纪律零反转）

| | 形态 | 失 |
|---|---|---|
| **U1（裁决）** | 生成按钮仅 `incremental`（change 详情）可点；`full` 只显示差异 + 「生成需开 change，或让左侧会话代劳」 | 架构页仍是观察窗 |
| U2 | 两处都可生成 | 给画布开写入口，反转 `desktop-archi-canvas` 四处带论证的文本 |
| U3 | U2 + 同批放宽 `archi_set_gate` | 反转四处 spec + 三条场景 + 一个 testid |

`full` 模式的写盘走**页内 agent 会话**（正是「只读约束」段原本给出的路径：会话在主仓根直改，画布保持只读）；D 面把这条路做得更好用，而不是给画布开写入口。

### 七、D 面：`archi_api` 只读工具

一条硬约束先撞上：`desktop-crate-layout` 明写 **`archi-runtime` MUST NOT 被 `agent-runtime` 依赖** ⇒ P1 把写盘函数放进 `archi-runtime` 后，agent **拿不到**写盘面。故工具只能是只读档：

```
archi_api(action = "check")                        → 两轴结构化结果（三类差异 + 十条判据摘要）
archi_api(action = "bootstrap", element = "<FQN>") → 契约草稿（只出 stdout 语义，零写盘）
```

生成仍由会话经 `bash` 跑既有命令（`pnpm api:export` / `cargo run -p archi-api-gen -- generate`）——这条路今天就通，零依赖变更。接入点全是既有范式（当前形态 = `mount.rs` 静态清单 + `compose.rs` 的**工具名字清单**注入，`toolset` 枚举已退役、`core/agent` 零改动）：`mount.rs` 静态清单 10 → 11 项（`ToolTier::Read`，`ToolExec::Archi` 沿用既有 `ArchiContext` 路由，不新增变体）、`archi_tool.rs` 的 `execute` 加一臂、`runner.rs::mint_archi_context` 的 present 判据加名、`compose.rs` 的 `archi_matrix` 给 `(implement, executor)` 加名且 archi 来源臂名单 +1、`archi-knowledge.md` 自检纪律补一条、`implementation-generator.md` 提示行补一句。

### 八、本轮不锁生成物（硬线二裁决，留痕）

生成物**不**进写保护面（不加用户规则、不内置化）。理由：① 零 diff 门已随 `static_analysis` 在 static-check 步硬拦，「手改生成物」不会静默腐烂；② 集成后 UI 面板直接呈现三类差异，再叠事前拦截等于同一事实三处提示；③ 保护面只挡常见路径（`write` / `edit` / shell 路径 token），子进程写盘天然穿过。**重开条件**（任一成立即重新评估）：契约面从本仓试点扩到多工作区复用；零 diff 门从 static-check 链路被摘掉；实际观察到 agent 手改生成物的案例。P1 已顺带留门：`.i.rs` / `.i.ts` 命名判据在本 crate 内，将来内置化只需 `protect::rules` 侧派生，不必造第三份判据。

---

## 能力

### 新增能力

**`desktop-archi-contract-panel`**（新 spec 目录）——契约面板与两条新 IPC 的完整契约：`archi_api_check` / `archi_api_generate` 的命令语义与 DTO、判定单源与写盘单点、两轴并列呈现与状态矩阵、生成交互（二次确认 / 孤儿如实 / 非原子如实）、按模式的生成权限、面板取数与成本纪律、抽屉契约段。

### 修改的能力

（口径：新增 **1** 个能力；修改 **9** 个 = `desktop-archi-model`（RENAMED × 1）/ `desktop-archi-api-check`（MODIFIED × 1）/ `desktop-archi-drift`（MODIFIED × 1）/ `desktop-archi-api`（MODIFIED × 2）/ `desktop-crate-layout`（MODIFIED × 2）/ `desktop-archi-agent`（ADDED × 1 + RENAMED × 1 + MODIFIED × 3）/ `desktop-archi-canvas`（MODIFIED × 2）/ `desktop-archi-file-tree`（MODIFIED × 1）/ `desktop-ipc-type-bindings`（MODIFIED × 1）。与 `specs/` 下十个 delta 目录（1 新增 + 9 修改）一一对应。）

- **desktop-archi-model**（`RENAMED` × 1）
  - `RENAMED`「IPC 命令面契约（六命令）」→「IPC 命令面契约（八命令）」：命令表补 `archi_api_check` / `archi_api_generate` 两行（语义归 `desktop-archi-contract-panel`）；「唯一写命令是 `archi_set_gate`」改为「写命令两条（`archi_set_gate` / `archi_api_generate`）」，六个命令 / 五个只读命令的计数统一改为八 / 六；Scenario「archi_module 出参带 api 并列字段」末句「命令总数仍为六个」改八个；`## Purpose` 的「六命令」同步（归档合并时）。其余命令行、`archi_set_gate` 只读约束、`api` 并列字段段逐字保留。
- **desktop-archi-api-check**（`MODIFIED` × 1）
  - `MODIFIED`「报告并列槽位 apiFindings」：把「首期 MUST NOT 影响 `status` / `passed`」拆成两半——`passed` 仍不受影响，`status` 改由「契约在案元素」的判据纳管（口径 ③，纳管子集由 `core/archi` 单点派生）；`REPORT_VERSION` 2 → 3；Scenario「既有字段与金样零改动」补口径（无契约在案语料逐字不变、`api-mismatch` 金样 `status` 由 `clean` 变 `warnings`）；Scenario「首期不影响 status 与 passed」改写为「契约在案判据进 status、passed 不动」。其余 bullet（既有 14 字段零改、稳定序、词汇四组、判定单源）与另两条 Scenario 逐字保留。
- **desktop-archi-drift**（`MODIFIED` × 1）
  - `MODIFIED`「报告结构与 status 五值」：jsonc 与 `version` bullet 的 2 → 3；`status` 五值 bullet 的 `warnings` 定义扩一个条件（「或契约在案元素的判据非空」）；`apiFindings` bullet 的「首期 MUST NOT 影响 status / passed」改写为「不影响 `passed`，经纳管子集影响 `status`，纳管口径归 `desktop-archi-api-check`」；Scenario「apiFindings 并列而不混装」与「版本递增与旧报告兼容」随之改写，新增 Scenario「契约在案判据把 status 拉到 warnings」。其余 bullet（observations 分界、occurrences 聚合、suppressed 留痕、modelHash、词汇四组）与另三条 Scenario 逐字保留。
- **desktop-archi-api**（`MODIFIED` × 2）
  - `MODIFIED`「生成物入仓与零 diff 门挂载」：追加一条 bullet——应用内两条 IPC（检查 / 生成）与 CLI 两驱动面**消费同一实现**（`api::workspace_render` / `api::compare` / `write_artifacts`），脚本面两条命令原文不变。其余 bullet（四类生成物表、`knip` 边界、类型绑定再生成、TS 形态前提、版本纪律）与 3 条 Scenario 逐字保留。
  - `MODIFIED`「仓库根文档的契约纪律节」：第 ⑤ 条由「`apiFindings` 首期不进 `status` / `passed`」改为「契约在案元素的判据进 `status`（`warnings` 档），不进 `passed`」——`AGENT.md` / `CLAUDE.md` 两处副本同批改、逐字相同。其余四条要点与三条 MUST NOT 逐字保留。
- **desktop-crate-layout**（`MODIFIED` × 2）
  - `MODIFIED`「工具 crate `archi-api-gen` 落位与契约面依赖扩展」：落位与职责改为「CLI 三子命令 + 三子命令编排，**写盘实现不在本 crate**」；workspace 内依赖 `archi` + `foundation` → 加 `archi-runtime`（带用途注释）；「允许写盘」改述为「写盘经 `archi-runtime` 单点，本 crate 自身零 `fs::write` / `create_dir_all`」；「零依赖面」的「MUST NOT 依赖任何 infra crate / `orchestration`」改为「MUST NOT **直接**依赖 `orchestration` / `store` / `config` / `workflow` / Tauri（`archi-runtime` 的传递依赖不可避免，须留痕）」；「新增入边：无」改为「新增入边 `archi-api-gen → archi-runtime`（tools → infra 单向）」。其余三条 Scenario 中的「写盘仅限工具边界」改写为「写盘单点在 `infra/archi`」，另两条逐字保留。
  - `MODIFIED`「infra/archi crate 落地登记与 archi 依赖扩展」：职责句追加「与报告落盘并列的**生成物唯一写盘函数** `write_artifacts`」；「新增入边」追加 `archi-api-gen → archi-runtime` 一条（原 `desktop-app → archi-runtime` 保留），MUST NOT 被依赖清单（`workflow` / `archi` / `agent-runtime` / `checks-runtime`）逐字不变；依赖面与零 `spawn` / 零 Tauri 结论零变化。其余 bullet 与三条 Scenario 逐字保留。
- **desktop-archi-agent**（`ADDED` × 1 + `MODIFIED` × 3）
  - `ADDED`「archi_api 只读工具的自检与草稿契约」：`check` / `bootstrap` 两动作、响应形状（复用 `ArchiApiCheck` 与 `ArchiApiFinding`，摘要按 `category` 计数）、`cap_result` 上限单点、只读零写盘、零第二份判定。
  - `RENAMED`「SessionToolset::Archi 与 (phase, role) 注入矩阵」→「archi 工具面注入与 (phase, role) 矩阵」：标题与正文的 `SessionToolset::Archi { module, search }` 变体表述**已是退役机制的残留**（代码侧 `toolset` / `SessionToolset` 零命中，注入面实际是 `compose.rs` 产出的工具名字清单 + `mount::lookup` 成员校验）——本次借改矩阵一并校正为实存形态，并给 `archi_api` 在 `archi_matrix` 里加一臂（仅 implement/executor）与 archi 来源臂三工具全开；`fail-closed` 与 `source_ref` 尾三段解析纪律逐字保留，两条 Scenario 的「两工具」表述随实。
  - `MODIFIED`「只读档位归属与成员校验不变」：只读清单加 `archi_api`（三档全放行）；四条 MUST NOT 与工具上下文条款逐字保留；Scenario 标题与内容随实（三个 archi 工具）。
  - `MODIFIED`「模型 = 源码（写纪律与不设专用写工具）」：仅把「`toolset` 词表内只有两个只读工具」改为「三个只读工具」（新工具仍非写工具，两条写通路与零第三通路裁定零反转）。
  - `MODIFIED`「架构会话的系统知识注入」：自检纪律 bullet 补一句「可调 `archi_api` 查两轴；生成物过期用既有命令重生成，MUST NOT 手改生成物」；Scenario「自检纪律在知识中可指认」的清单加一条。
- **desktop-archi-canvas**（`MODIFIED` × 2）
  - `MODIFIED`「单一画布组件与两处挂载点」：左栏由「文件树 / 元素树两视图」改为「文件树 / 元素树 / 契约视图三视图」（缺省仍文件树；契约视图完整契约归 `desktop-archi-contract-panel`）。两条 Scenario 的「三段齐备」表述随实。其余 bullet（观察根、显式取数、1-hop 缺省、testid 纪律）与另两条 Scenario 逐字保留。
  - `MODIFIED`「能力差异（全量只读 / 增量可改档）」：能力差异表加一行「生成生成物：`full` ❌（只显示差异 + 提示）/ `incremental` ✅ 二次确认后写 worktree」；只读约束段补一句「本裁定同样覆盖契约面板的生成按钮」。其余 bullet 与四条 Scenario 逐字保留（含「MUST NOT 以『会话能写』为理由为画布新增任何写入口」——U1 正是这条的延伸）。
- **desktop-archi-file-tree**（`MODIFIED` × 1）
  - `MODIFIED`「左栏文件树呈现、分段切换与双向联动」：分段控件由两钮改为三钮（`data-mode="file"` / `"element"` / `"contract"`，缺省仍 `file`），并声明「文件 / 元素两视图的分段语义与联动逐字不变，契约视图归 `desktop-archi-contract-panel`」；Scenario「缺省渲文件树且可切回元素树」补一句「切到契约视图不触发 `archi_graph` / `archi_module` / `archi_drift` 重取数」。其余 bullet 与 5 条 Scenario 逐字保留。
- **desktop-ipc-type-bindings**（`MODIFIED` × 1）
  - `MODIFIED`「tauri-specta 接入与命令绑定生成」：清单守卫条款随动——命令 51 → 53、出线 DTO 101 → 104、`ARCHI_COMMANDS` 6 → 8、`ARCHI_DTO_TYPES` 23 → 26；尾部追加序断言把两条新命令插在 `archi_file_tree` 之后、`read_file` 之前（`read_file` 仍为尾条）。其余正文、两条 bullet 与四条 Scenario 逐字保留。

### 零改动的能力及理由

| 能力 | 理由 |
|---|---|
| **desktop-write-protection** | 硬线二裁决「本轮不锁生成物」——既不加用户规则、也不内置化；`ReasonKind` 与内置 glob 派生态零改动，`rule` / 文案 / 射程均不变 |
| **desktop-archi-gate** | 门禁结论仍只消费漂移报告；`passed` 判据零改动（`apiFindings` 不产 `errors` 槽），故 `LocalArchiCheck` 零改动 |
| **desktop-archi-chat** | 会话面板入参与插槽零变化；`archi` 来源臂的工具面扩展归 `desktop-archi-agent` |
| **desktop-archi-drift 的判定面** | 扫描 / 归属 / 定级 / 豁免 / `occurrences` 聚合零改动（只改报告 version 与 `status` 推导的入参） |
| **desktop-archi-api 的契约格式与判据** | `contract` / `schema` / `emit` / `extract` / `paths` / `check` 六面零改动——本变更只搬「比对面」与「写盘面」 |
| **workspace-layout-resolution** | 复用既有 `archi_api_dir` / `archi_config_path` 派生，零新增 layout 函数 |
| **desktop-workspace-config** | 工作区 `.dev-team/config.json` 与 `KNOWN_KEYS` 七项零变化（架构域配置与之互不读取） |
| **desktop-phase-prompts** | 只在既有文件 `implementation-generator.md` 的「额外可用只读工具」行补一句，文件清单 / 静态零占位 / `include_str!` 字面路径三条纪律零改动 |
| **desktop-checks-domain / desktop-test-execution** | `static_analysis` 段序与 `api:check` 命令形态零改动（C 面本轮不做） |

---

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `packages/desktop/src-tauri/crates/core/archi/src/api/render.rs`（新增） | `workspace_render(root) -> Result<Vec<GeneratedFile>, ApiError>`：收编 CLI 的 `collect`（`dto::graph_view` → `load_contracts` → `render_all`）；结构诊断非空 → `Err(ApiError::Element)`（fail-closed，坏模型不得被固化成入库生成物）；纯读盘、零写盘、零 spawn |
| `packages/desktop/src-tauri/crates/core/archi/src/api/diff.rs`（新增） | `compare(root) -> Result<ArtifactDiff, ApiError>`：收编 CLI 的 `differences` + `generated_files_on_disk`；`ArtifactDiff { mismatched, orphaned, missing }` 三列均为 repo 相对 POSIX 串（经 `archi::display`，MUST NOT 本地另写归一）；`orphaned` 的落点枚举由手写递归 walker 改为已依赖的 `ignore`（gitignore 感知 + 构建产物剪枝），消掉 `SKIPPED_DIRECTORIES` / `MAX_MODULE_WALK_DEPTH` / `walk_modules` 三处手写；零写盘 |
| `packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs`（改） | `pub mod diff;` `pub mod render;` + 出口函数 `workspace_render` / `compare` / `artifact_check`（`ArtifactDiff` + `api::meta()` → `ArchiApiCheck` 投影，单一落点）+ `gating_findings(root, &elements, &findings) -> Vec<ArchiApiFinding>`（「契约文件在案元素」筛选单点，判据与 `api_ref` 同源） |
| `packages/desktop/src-tauri/crates/core/archi/src/dto.rs`（改） | 新增三个出线 DTO：`ArchiApiCheck { enabled, meta, mismatched, orphaned, missing }`、`ArchiApiMeta { toolVersion, commonDigest }`、`ArchiApiWriteOutcome { written, unchanged }`（`specta::Type` + `rename_all = "camelCase"`，字段序即声明序）；既有 DTO 与九组封闭词汇零变化 |
| `packages/desktop/src-tauri/crates/core/archi/src/lib.rs`（改） | 再导出 `workspace_render` / `compare` / `artifact_check` / `gating_findings` / `ArtifactDiff` 与三个新 DTO（沿既有逐名 `pub use` 段） |
| `packages/desktop/src-tauri/crates/core/archi/src/drift/mod.rs`（改） | `check()` 内：`api_findings` 装配后调 `api::gating_findings` 得纳管子集 → `derive_status` 增该入参；`warnings` 分支条件由「`warnings[]` 非空」扩为「`warnings[]` 非空 **或** 纳管子集非空」，其余四值先后序逐字不变；`passed` 派生面零改动 |
| `packages/desktop/src-tauri/crates/core/archi/src/drift/report.rs`（改） | `REPORT_VERSION` 2 → 3 与其兼容说明（`version = 2` 的报告 `status` 不读契约判据、`version = 1` 无 `apiFindings` 字段）；报告 14 字段形状零改动 |
| `packages/desktop/src-tauri/crates/infra/archi/src/api_artifacts.rs`（新增） | `write_artifacts(root, &[GeneratedFile]) -> Result<ArchiApiWriteOutcome, String>`：先 `starts_with(root)` 越根复核（第二层）→ 逐份「读盘比对，异才写」→ `create_dir_all` + `fs::write`；从不删文件；`written` / `unchanged` 两列 repo 相对串；单份失败即 `Err`（已写部分不回滚，文案如实） |
| `packages/desktop/src-tauri/crates/infra/archi/src/lib.rs`（改） | `pub mod api_artifacts;`（与 `report` 并列；文件头注释补「本 crate 承载两处写盘单点：报告落盘与生成物写盘」） |
| `packages/desktop/src-tauri/crates/tools/archi-api-gen/src/main.rs`（改） | 去掉 `collect` / `write` / `differences` / `Report` / `walk` / `walk_modules` / `generated_files_on_disk` / `is_generated_module_file` / `SKIPPED_DIRECTORIES` / `MAX_MODULE_WALK_DEPTH` / `model_elements`；三子命令改调 `api::workspace_render` + `archi_runtime::write_artifacts` / `api::compare` / `api::bootstrap`；`--check` 差异清单序列化保留（单行 JSON + 三类键，退出码语义不变：无差异 0 / 有差异或失败 1 / 用法错 2）；`generate` 输出文案改报 `written` 份数 |
| `packages/desktop/src-tauri/crates/tools/archi-api-gen/Cargo.toml`（改） | `[dependencies]` 加 `archi-runtime = { workspace = true }` 带用途注释（「生成物写盘单点；本 crate 只做 CLI 编排」）；文件头注释的「零依赖面」段改写（写盘实现已上移，CLI 仍零子进程） |
| `packages/desktop/src-tauri/src/commands/archi/mod.rs`（改） | 新增 `archi_api_check(root) -> Result<ArchiApiCheck, String>` + `archi_api_check_inner(root: &Path)`（`check_root` → `archi::api::artifact_check`）与 `archi_api_generate(root) -> Result<ArchiApiWriteOutcome, String>` + `archi_api_generate_inner`（`check_root` → `api::workspace_render` → `write_artifacts`）；文件头「六命令」→「八命令」、「唯一写命令是 `archi_set_gate`」→「写命令两条」 |
| `packages/desktop/src-tauri/src/commands/mod.rs`（改） | `all_commands!` 的 archi 段追加两条（位置在 `archi_file_tree` 之后、`read_file` 之前；`read_file` 仍为尾条） |
| `packages/desktop/src-tauri/src/bindings/mod_test.rs`（改） | 清单随动：`COMMAND_NAMES` / `COMMAND_WRAPPERS` 51 → 53、`DTO_TYPES` 101 → 104、`ARCHI_COMMANDS` 6 → 8、`ARCHI_DTO_TYPES` 23 → 26；尾部追加序期望 vec 与「取 `ARCHI_COMMANDS` 末条做副本删一条」对照臂同步（族末条由 `archi_file_tree` 变 `archi_api_generate`）；`archiFileTree` 类 args/result schema 断言补两条 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs`（改） | 静态清单 10 → 11 项：新增 `ARCHI_API_TOOL = "archi_api"`（`ToolTier::Read`、`ToolExec::Archi`）；`ToolTier::Read` 注释与 `ToolExec::Archi` 臂名清单同步 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool.rs`（改） | `ArchiContext::execute` 的 match 加 `archi_api` 一臂（`action` = `check` / `bootstrap`），输出 `serde_json` 后过 `tools::cap_result`（L1 上限单点）；`check` = `dto::graph_view`（结构诊断 → `is_error`）→ `api::load_contracts` → `api::check` + `api::compare`，按 `category` 计数摘要；`bootstrap` = `api::bootstrap` 文本；零写盘 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs`（改） | `mint_archi_context` 的 present 判据（名字清单含任一 archi 工具）加 `ARCHI_API_TOOL` |
| `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs`（改） | `archi` 来源臂名字清单 +1（三工具全开）；`archi_matrix` 给 `(implement, executor)` 加 `archi_api` 名；其余相位零变化；零 `toolset` 概念引入 |
| `packages/desktop/src-tauri/crates/infra/agent/src/archi-knowledge.md`（改） | 「自检纪律」bullet 补一句：可调 `archi_api` 查「生成物新鲜度」与「契约 vs 代码」两轴；生成物过期用 `pnpm api:export` 重生成，MUST NOT 手改生成物；零 `<>` 占位符、体量受既有上限约束 |
| `packages/desktop/src-tauri/crates/core/workflow/src/prompts/implementation-generator.md`（改） | 「额外可用只读工具」行补 `archi_api`（改完代码自查生成物新鲜度与契约判据）；其余相位 prompt 零改动 |
| `packages/desktop/src/views/architecture/hooks/use-archi-api-check.ts`（新增） | 显式取数 `commands.archiApiCheck(root)`：挂载 / `root` 变化 / 手动 `refresh()` 三触发；`root === null` 零 invoke；零 watch 零轮询；返回 `check` / `loading` / `error` / `refresh`；失败只落面板行内错误，MUST NOT 阻断画布 |
| `packages/desktop/src/views/architecture/components/contract-panel.tsx`（新增） | 左栏「契约」视图：两轴分区（生成物差异三类分组 + 判据按 `category` chip 过滤）、计数各自独立、条目点击 `onFocus(element)`、路径行走 `FileViewer`、「打开」走 `read_file`、`[检查]` 与按模式的 `[生成]`、状态矩阵五态、生成前 `StandardDialog` 二次确认（写明「写入 N 份 / 保留 M 份孤儿不做处理 / 非原子」）、生成成功后调父级 `refresh` 重跑 check；testid `archi-contract-panel` / `archi-contract-axis-artifacts` / `archi-contract-axis-findings` / `archi-contract-chip` / `archi-contract-item` / `archi-contract-generate` / `archi-contract-check` / `archi-contract-status` / `archi-contract-error` |
| `packages/desktop/src/views/architecture/components/file-tree.tsx`（改） | `TreeMode` 扩 `'file' \| 'element' \| 'contract'`；`TreeModeSwitch` 选项表加第三钮（`data-mode="contract"`），控件本体与另两钮零变化 |
| `packages/desktop/src/views/architecture/components/architecture-canvas.tsx`（改） | 左栏分支加 `contract` 臂（渲 `ContractPanel`，栏宽 w-80）；`report.apiFindings` 与 `focused` 传给面板与抽屉；`incremental` / `full` 决定生成按钮可用性；`toNodes` / `toEdges` / `neighborhoodPositions` / 停摆分支零改动 |
| `packages/desktop/src/views/architecture/components/element-drawer.tsx`（改） | 概览与文档之间插「契约」段：`api.status` + 三路径行（`FileViewer` 打开）+ 本元素判据清单（`element === fqn` 过滤，空态一句「本元素无漂移」）；数据全部来自既有 `ArchiModuleDetail.api` 与父级同一份 `apiFindings`（零新 invoke） |
| `packages/desktop/package.json`（改） | `version` 0.6.39 → 0.6.40（一次 patch）；`src-tauri/Cargo.toml` 的 crate 版本行不跟 |
| `AGENT.md` + `CLAUDE.md`（改） | 「模块 API 契约」节第 ⑤ 条改为「契约在案元素的判据进 `status`（`warnings` 档），不进 `passed`」；两文件该节正文逐字相同 |
| `packages/desktop/src/types/generated/bindings.ts` + `bindings.zod.ts`（再生成入库） | 两条新命令的 typed 包装 + `ArchiApiCheck` / `ArchiApiMeta` / `ArchiApiWriteOutcome` 三个 `export type` 与 `argsSchemas` / `resultSchemas` 表项；随 `bindings:export` 刷新，入库前经 `git diff` 人工核对 |

### 测试文件

| 文件 | 处理 |
|---|---|
| `crates/core/archi/src/api/render_test.rs`（新增） | 真实 tempdir（沿 `api/**` 零 mock 先例）：应生成集含四类产物；结构诊断非空 → `Err`；空模型 → 仅 `INDEX.toml`（冷启动应生成集）；零契约 → 零 `.i.*` 与零 `.schema.json`；两次调用逐字相同（幂等） |
| `crates/core/archi/src/api/diff_test.rs`（新增） | 三类差异各一正例（缺口 / 内容不符 / 孤儿）；盘上无关同名文件不误报；`target` / `node_modules` / `dist` / `build` / `.git` 内不扫；**`.gitignore` 命中项不扫**（walker 收紧的显式留痕用例）；零差异 → 三列全空；路径串与 `archi::display` 同源（反斜杠归一）；零写盘（调用前后盘上字节相同） |
| `crates/core/archi/src/api/mod_test.rs`（改） | `gating_findings` 用例：契约文件在案的 `declared` 元素判据入选；无契约文件的 `declared` 元素判据落选；`element = None` 的条目落选；契约面未启用 ⇒ 空 |
| `crates/core/archi/src/drift/mod_test.rs`（改） | `status` 推导：纳管子集非空且无 errors → `warnings`；`errors` 非空时仍 `errors`（先后序不变）；纳管子集空 → `status` 与改动前逐字相同；`passed` 不因判据变假 |
| `crates/core/archi/src/drift/report_test.rs`（改） | `REPORT_VERSION == 3`；四组词汇对拍零改动 |
| `crates/core/archi/tests/corpus/drift/expected/*.json`（16 份重写） | 全部 `version` 2 → 3；`api-mismatch.full.json` 的 `status` 由 `clean` 变 `warnings`（该元素 `api = declared` 且契约在案 ⇒ 判据纳管），其余 15 份除 `version` 外逐字不变；沿既有 `DESKTOP_GOLDEN_REWRITE=1` 显式重写并人工确认 |
| `crates/core/archi/tests/corpus/drift/api-mismatch/models/01-app.toml`（改） | 文件头注释里「契约面 MUST NOT 进 `status` 推导 …… `status = clean`」改为契约在案纳管口径（`status = warnings`），避免语料自述与金样矛盾 |
| `crates/core/archi/src/dto_test.rs`（改） | 三个新 DTO 字段名与声明序断言（`ArchiApiCheck` 五字段 / `ArchiApiMeta` 二字段 / `ArchiApiWriteOutcome` 二字段）；既有断言零改 |
| `crates/core/archi/tests/vocabulary_test.rs`（核对） | 四面词汇对拍零改动（本变更不新增封闭词汇） |
| `crates/infra/archi/src/api_artifacts_test.rs`（新增） | 首轮全 `written`、次轮全 `unchanged`；越根目标 → `Err` 且零写盘；父目录自动创建；内容相同不重写（mtime 不变）；单份失败返回 `Err` 且已写部分保留（文案如实，不回滚）；路径串 repo 相对 |
| `crates/tools/archi-api-gen/src/main_test.rs`（改） | 纪律断言翻转：`main.rs` 的 `fs::write` / `create_dir_all` 计数 1 → **0**；`SKIPPED_DIRECTORIES` / `MAX_MODULE_WALK_DEPTH` / `is_generated_module_file` / `walk_modules` 的单点断言迁出（改为「零本地副本」断言，判据名与 core 侧新落点对齐）；三子命令行为用例改走真实 `api::*` + `write_artifacts`（tempdir）；退出码三态保持不变 |
| `src-tauri/src/commands/archi/mod_test.rs`（改） | 追加 `archi_api_check` / `archi_api_generate` 与 `_inner` 逐字段一致断言；空白 / 非目录 root → `Err`；`archi_api_check` 零写盘；`archi_api_generate` 写入后 `check` 由「三类差异非空」变「三列全空」；结构错模型 → 两条均 `Err` 且零写盘；root 校验用例扩到八命令（名字里的「六命令」改符实） |
| `src-tauri/src/bindings/mod_test.rs`（改） | 见「实现文件」行（五处计数 + 两组 archi 清单 + 尾部序 + 对照臂） |
| `crates/infra/agent/src/sdk/mount_test.rs`（改） | 静态清单 11 项；`archi_api` 的 `ToolTier::Read` / `ToolExec::Archi` 断言 |
| `crates/infra/agent/src/sdk/archi_tool_test.rs`（改） | `check` 动作：结构诊断非空 → `is_error`；正常模型 → 三类差异 + 判据计数摘要；`bootstrap` 动作：出草稿且盘上零变化；未知 `action` / 缺 `element` → `is_error`；大输出被 `cap_result` 截断；两个动作均零写盘 |
| `crates/infra/agent/src/sdk/runner_test.rs` + `compose` 相关用例（改） | `archi_api` 进 present 判据；矩阵 `(implement, executor)` 含三工具；其余相位仍只两 / 一 / 零工具；archi 来源臂三工具全开 |
| `crates/infra/agent/src/archi_knowledge*`（改） | 知识文本含新自检句、仍零 `<>` 占位符、体量不超上限 |
| `packages/desktop/src/views/architecture/components/contract-panel.test.tsx`（新增） | 两轴分区与各自计数独立（MUST NOT 出现合并总数）；`category` chip 过滤；条目点击 → `onFocus`（一次）；`enabled = false` 呈冷启动说明且仍显示 `missing` 一条；零差异零判据态；invoke reject 行内错误；`incremental` 有生成按钮 / `full` 只有检查 + 提示；生成链路全栈断言（点生成 → dialog 文案含「写入 N / 保留 M 孤儿 / 非原子」→ 确认 → `archiApiGenerate` 一次 → toast → `archiApiCheck` 重跑一次） |
| `packages/desktop/src/views/architecture/hooks/use-archi-api-check.test.ts`（新增） | 三触发（挂载 / `root` 变化 / `refresh()`）、`root === null` 零 invoke、零轮询零 watch、失败清旧数据（旧差异 MUST NOT 冒充本次结果） |
| `packages/desktop/src/views/architecture/components/architecture-canvas.test.tsx`（改） | 新增 `archiApiCheck` / `archiApiGenerate` 桩位；第三钮 `data-mode="contract"` 切到面板且不触发 `archi_graph` / `archi_module` / `archi_drift` 重取数；`full` 模式无生成入口；既有画布 / 抽屉 / 标记断言逐字保留 |
| `packages/desktop/src/views/architecture/components/element-drawer.test.tsx`（改） | 契约段：`declared` 三路径行可打开 / 本元素判据逐条列出 / 零判据态一句「本元素无漂移」；零新增 invoke |
| `packages/desktop/src/views/architecture/components/file-tree.test.tsx`（改） | 分段控件三钮、缺省仍 `file`、第三钮回调 `onChange('contract')`；文件 / 元素两视图既有断言逐字保留 |
| `packages/desktop/src/lib/ipc-contract.test.ts`（核对） | 键集全量断言自动覆盖两条新命令；`argsSchemas` / `resultSchemas` / `channelArgs` 三表逐键相等（零手改，核对随动即通过） |

### 不要修改

- **契约判据与契约格式**：`crates/core/archi/src/api/check.rs` / `check_test.rs` / `contract.rs` / `schema.rs` / `paths.rs` / `emit/**` / `extract/**` 零改动（十条判据的语义、`$defs` 规则、生成物命名与落点判据一律不动）。
- **漂移判定面**：`crates/core/archi/src/scan/**`（含 `owner.rs` / `exclude.rs`）、`drift/report.rs` 的字段定义与排序、`observations` / `suppressed` 装配零改动；`derive_status` 只加一个入参与一个分支条件。
- **门禁执行面**：`crates/infra/archi/src/report.rs` / `runner.rs` 零改动（门禁 `passed` 判据不变）；`LocalArchiCheck` 的 outcome 派生零改动。
- **写保护面**：`crates/infra/agent/src/sdk/protect/**` 零改动（生成物本轮不入锁）；`.dev-team/config.json` 零改动。
- **画布渲染面**：`architecture-canvas.tsx` 的 `toNodes` / `toEdges` / `neighborhoodPositions` / `defaultFocus` / `ArchiNodeView` / `StructureErrorPanel` 分支、`structure-error-panel.tsx`、`lib/marks.ts`、`lib/drift-marks.ts`、`lib/fqn.ts`、`element-tree.tsx` 零改动。
- **既有 hooks**：`use-archi-graph.ts` / `use-archi-drift.ts` / `use-archi-module.ts` / `use-archi-file-tree.ts` 零改动（新面板复用它们已取的数据，不自取第二份）。
- **脚本面**：`package.json` 的 `api:export` / `api:check` / `check` 三段命令形态零改动（C 面不做）。
- **`crates/core/agent/**`**：注入面本就是工具名字清单（`SessionInjections.tools`），无枚举字段需新增——「`SessionToolset` / `toolset`」在代码侧已退役，本次只在 spec 侧校正该残留表述；`SessionInjections` / `RunRequest` / `SessionProvenance` 三面零改动。
- **工作区配置与模型资产**：`.dev-team/config.json`、`crates/core/config/**`（含 `config.schema.json` 镜像）、`.dev-team/architecture/models/**`、`decisions/**`、`api/**`（契约资产与四类生成物本轮零改动——本变更只搬检查 / 生成通道，不改契约内容）。
- **规格基线**：`openspec/specs/**` 只在 change 目录出 delta（归档时合入）。
- **`openspec/changes/api-gen-check-in-desktop/explore.md`**：只读。
- **`packages/desktop/src-tauri/Cargo.toml`**：crate 版本行不随前端 patch 变动（`[workspace.dependencies]` 与 `members` 也零改动——无新 crate）。

---

## 验收标准

| ID | 验收条件 | 验证方法 |
|---|---|---|
| AC-1 | **两条新命令可数且语义分明**：archi 段命令 **8** 条（`archi_graph` / `archi_module` / `archi_change_summary` / `archi_set_gate` / `archi_drift` / `archi_file_tree` / `archi_api_check` / `archi_api_generate`）；写命令恰两条（`archi_set_gate` / `archi_api_generate`），其余六条零写盘 | `commands/archi/mod_test.rs` + `bindings/mod_test.rs` + 源码审查 |
| AC-2 | **check 零写盘、generate 只增改不删**：`archi_api_check` 调用前后盘上字节完全相同；`archi_api_generate` 写后三类差异清空，且**孤儿文件仍在盘**（`written` / `unchanged` 两列之和等于应生成份数） | `commands/archi/mod_test.rs` + `api_artifacts_test.rs` |
| AC-3 | **DTO 形状**：`ArchiApiCheck` 五字段（`enabled` / `meta` / `mismatched` / `orphaned` / `missing`）、`ArchiApiMeta` 二字段、`ArchiApiWriteOutcome` 二字段，按声明序出线为 camelCase；三列路径串为 repo 相对 POSIX（反斜杠归一、无前导 `./`），与 `archi::display` 同源零第二份归一 | `dto_test.rs` + `bindings/mod_test.rs` + `api/diff_test.rs` |
| AC-4 | **比对面上移且判定单源**：`core/archi/src/api/**` 提供 `workspace_render` / `compare` 两纯函数；`tools/archi-api-gen/src/main.rs` **零** `fs::write` / `create_dir_all` / 零本地 walker；命令面（IPC）与 CLI 面调同一函数（源码断言 + 行为逐字段一致） | `main_test.rs` 翻转断言 + `api/diff_test.rs` + 源码审查 |
| AC-5 | **写盘单点驻 `infra/archi`**：契约生成面的 `fs::write` / `create_dir_all` 在 `crates/infra/archi/src/api_artifacts.rs` **恰一处**（`starts_with(root)` 越根复核在位）；`archi-api-gen` 的 workspace 内依赖为 `archi` + `foundation` + `archi-runtime`（带用途注释）；`archi-api-gen` 仍不被 app / core / infra 依赖 | `api_artifacts_test.rs` + `Cargo.toml` 审查 + `main_test.rs` |
| AC-6 | **`status` 纳管口径**：纳管子集 = 契约文件在案元素的判据；子集非空且 `errors` 空 ⇒ `status = warnings`；`errors` 非空仍 `errors`；子集空 ⇒ `status` 与改动前逐字相同；`passed` **不因** `apiFindings` 变假；纳管筛选在 `core/archi` 单点（`drift` 侧零 `is_file` 调用） | `api/mod_test.rs` + `drift/mod_test.rs` + 源码审查 |
| AC-7 | **报告版本与金样**：`REPORT_VERSION = 3`；16 份金样除 `version` 外逐字不变，唯一例外是 `api-mismatch.full.json` 的 `status` 由 `clean` 变 `warnings`（契约在案的 `apiSignatureMismatch` 纳管）；重写经 `DESKTOP_GOLDEN_REWRITE=1` 并人工确认 | `cargo nextest` + `git diff` 逐份核对 |
| AC-8 | **本仓实测零变化**（登记留痕，不作为 MUST）：本仓 `apiFindings` 仍 41 条（`apiEntryMissing` 28 / `apiMissing` 13），契约在案元素仅 `App.Backend.Infra.ArchiRuntime` 且判据 0 条 ⇒ `archi_drift` 的 `status` / 门禁 `passed` 与改动前逐字段相同（除 `version`） | 手工跑一次 `archi_drift` 与门禁步 + 报告留痕 |
| AC-9 | **面板两轴并列**：契约视图分「生成物差异（三类分组 + 计数）」与「判据（按 `category` 六值 chip + 计数）」两区，MUST NOT 出现跨轴合并的问题总数；条目点击触发一次 `onFocus`（元素圈定）；路径行经 `FileViewer` + `read_file` 打开 | `contract-panel.test.tsx` |
| AC-10 | **状态矩阵五态**：`loading` / `enabled = false`（冷启动说明 + 仍显示 `INDEX.toml` missing 一条）/ 零差异零判据 / invoke reject（行内错误、画布不受影响）/ 生成失败（面板顶部错误回显，文案不声称「全或无」） | `contract-panel.test.tsx` |
| AC-11 | **生成交互**：仅 `incremental` 有生成按钮；点击后 `StandardDialog` 二次确认，文案含「写入 N 份」「保留 M 份孤儿不做处理」「非原子」；确认后 `archiApiGenerate` 恰一次 → toast 报 written / unchanged → 自动重跑 `archiApiCheck` 一次；`full` 模式零生成入口 + 一行提示（形态沿 `archi-gate-readonly`） | `contract-panel.test.tsx` + `architecture-canvas.test.tsx` |
| AC-12 | **取数与成本纪律**：`use-archi-api-check` 只在契约视图挂载 / `root` 变化 / 手动刷新三触发 invoke；切到 `contract` 视图不触发 `archi_graph` / `archi_module` / `archi_drift` 重取数；`root === null` 零 invoke；源码零 `watchSubscribe` 零定时器；`workspace_render` 不随画布刷新自动跑 | `use-archi-api-check.test.ts` + `architecture-canvas.test.tsx` + 源码审查 |
| AC-13 | **抽屉契约段**：概览与文档之间呈现 `api.status` + 契约 / schema / 生成物三路径行（`FileViewer` 打开）+ 本元素判据（`element === fqn` 过滤，零判据态一句「本元素无漂移」）；零新增 invoke（复用父级同一份 `apiFindings` 与既有 `ArchiModuleDetail.api`） | `element-drawer.test.tsx` + `architecture-canvas.test.tsx` |
| AC-14 | **分段控件三视图**：`data-mode` 三值 `file` / `element` / `contract`，缺省 `file`；三视图各自渲染、`focus` 共用；文件 / 元素两视图既有断言与 testid 零变化 | `file-tree.test.tsx` + `architecture-canvas.test.tsx` |
| AC-15 | **`archi_api` 只读工具**：静态清单 11 项；`ToolTier::Read`（`policy::allows` 三档恒放行）、`ToolExec::Archi`（沿用既有路由，零新变体、零 `toolset` 概念）；`check` / `bootstrap` 两动作行为如约，未知 `action` → `is_error`；输出过 `cap_result`；**零写盘**；`archi_matrix` 仅 `(implement, executor)` 增、其余相位零增；archi 来源臂三工具全开 | `mount_test.rs` + `archi_tool_test.rs` + `runner_test.rs` + 矩阵用例 |
| AC-16 | **知识 / prompt 随动**：`archi-knowledge.md` 含新自检句且零 `<>` 占位符、体量在上限内；`implementation-generator.md` 的只读工具行含 `archi_api`；其余相位 prompt 零 diff | `archi_knowledge` 用例 + `git diff` |
| AC-17 | **清单守卫随动**：命令 51 → 53、出线 DTO 101 → 104、`ARCHI_COMMANDS` 6 → 8、`ARCHI_DTO_TYPES` 23 → 26；尾部追加序断言与对照组臂同步；`bindings:export` 后两份生成物入库、`git diff` 人工核对通过 | `cargo nextest` + `git diff` |
| AC-18 | **门禁**：`pnpm -C packages/desktop run check` 绿（`server:check` → `bindings:export` → `api:check` 零 diff → `client:check` 含 `knip` 零新增 ignore）；`cargo nextest run --workspace` 全绿；`package.json` 0.6.39 → 0.6.40，`src-tauri/Cargo.toml` crate 版本行不变 | 门禁命令 + `git diff --stat` |
| AC-19 | **边界零 diff**：`crates/core/archi/src/api/check.rs` / `contract.rs` / `schema.rs` / `paths.rs` / `emit/**` / `extract/**`、`crates/core/archi/src/scan/**`、`crates/infra/archi/src/report.rs` / `runner.rs`、`crates/infra/agent/src/sdk/protect/**`、`.dev-team/config.json`、`.dev-team/architecture/api/**`、`openspec/changes/api-gen-check-in-desktop/explore.md` 全部零 diff | `git diff --stat` 审查 |

**AC ↔ spec requirement 对应**：

| AC | spec requirement / scenario |
|---|---|
| AC-1 / AC-2 / AC-3 | `desktop-archi-contract-panel` ADDED「检查命令契约」「生成命令契约」；`desktop-archi-model` RENAMED「IPC 命令面契约（八命令）」 |
| AC-4 / AC-5 | `desktop-archi-contract-panel` ADDED「判定单源与写盘单点」；`desktop-crate-layout` MODIFIED「工具 crate `archi-api-gen` 落位与契约面依赖扩展」+「infra/archi crate 落地登记与 archi 依赖扩展」 |
| AC-6 / AC-7 / AC-8 | `desktop-archi-api-check` MODIFIED「报告并列槽位 apiFindings」；`desktop-archi-drift` MODIFIED「报告结构与 status 五值」 |
| AC-9 / AC-10 / AC-11 / AC-12 | `desktop-archi-contract-panel` ADDED「两轴并列呈现与状态矩阵」「生成交互与模式权限」「面板取数与成本纪律」 |
| AC-13 | `desktop-archi-contract-panel` ADDED「抽屉契约段」 |
| AC-14 | `desktop-archi-file-tree` MODIFIED「左栏文件树呈现、分段切换与双向联动」+ `desktop-archi-canvas` MODIFIED「单一画布组件与两处挂载点」 |
| AC-11（模式权限部分） | `desktop-archi-canvas` MODIFIED「能力差异（全量只读 / 增量可改档）」 |
| AC-15 / AC-16 | `desktop-archi-agent` ADDED「archi_api 只读工具的自检与草稿契约」+ RENAMED「archi 工具面注入与 (phase, role) 矩阵」+ MODIFIED「只读档位归属与成员校验不变」「模型 = 源码（写纪律与不设专用写工具）」「架构会话的系统知识注入」 |
| AC-17 | `desktop-ipc-type-bindings` MODIFIED「tauri-specta 接入与命令绑定生成」 |
| AC-4 / AC-5（脚本面同源） | `desktop-archi-api` MODIFIED「生成物入仓与零 diff 门挂载」 |
| AC-6（AGENT.md 口径） | `desktop-archi-api` MODIFIED「仓库根文档的契约纪律节」 |
| AC-18 / AC-19 | 工程验收面（无 requirement 对应；以门禁与 `git diff` 为判据） |

---

## 风险

| ID | 风险 | 影响 | 概率 | 缓解措施 |
|---|---|---|---|---|
| R-1 | **比对面 walker 换 `ignore` 后语义收紧**（`.gitignore` 命中路径不再被扫） | 若某工作区把生成物 gitignore（违反 `desktop-archi-api`「必须入仓」纪律），`orphaned` 将漏报；反之构建产物目录剪枝更彻底 | 中 | 语义变化在 spec 与测试双处留痕（`api/diff_test.rs` 专设「`.gitignore` 命中不扫」用例）；「必须入仓」是既有 MUST，收紧与之一致；若将来出现合法 gitignore 形态，回退序为「`ignore` 增 `overrides` 白名单生成物命名」 |
| R-2 | **`api-mismatch` 金样 `status` 由 `clean` 变 `warnings`**（纳管口径的直接后果） | 被误读为回归；语料文件头注释与金样自相矛盾 | 高 | AC-7 / AC-8 显式点名这是**预期变化**；同批改 `api-mismatch/models/01-app.toml` 的注释；其余 15 份金样断言的措辞改为「除 `version` 外逐字不变」 |
| R-3 | **`REPORT_VERSION` 递增**让历史报告与新报告不可混读 | 消费方若按版本分支写错，会把 `version = 2` 的 `status` 当成「已含契约纳管」 | 中 | 递增 + 显式兼容说明（`version = 1` 无 `apiFindings`、`version = 2` 的 `status` 不读契约判据）；`drift/report.rs` 注释与 spec 两处同源；无持久化消费方按版本分支的代码在案（前端只读当下报告） |
| R-4 | **写盘面搬 `infra/archi` 后 `archi-api-gen` 传递依赖 `orchestration`**（`archi-runtime` 既有依赖） | 破「tools 零 orchestration」措辞；`tauri build` 是否被拖带需复核（工具仍不入壳依赖 ⇒ 不拖带） | 中 | `desktop-crate-layout` 的该条 MUST NOT 改为「MUST NOT **直接**依赖」并留痕；`archi-api-gen` 仍不被 app / core / infra 依赖（AC-5 机械断言）；`tauri build` 产物不含该 bin 的既有 Scenario 保留 |
| R-5 | **写盘非原子**：逐份 `fs::write` 中途失败 = 部分写入 | 用户误以为「点生成 = 全或无」 | 高 | `desktop-archi-contract-panel` 明写「非原子、失败点之前已写」，dialog 与错误文案 MUST NOT 声称全或无；`api_artifacts_test.rs` 用只读目录构造失败并断言已写部分保留 |
| R-6 | **生成不删孤儿**被误读为「生成后即干净」 | 用户反复点生成以为能清掉 orphan | 高 | dialog 必须写「保留 M 份孤儿不做处理」；面板两轴分区里 orphan 属「需人处理」的一类（可打开定位后手工删除）；spec 写成硬条款 |
| R-7 | **`workspace_render` 成本**（模型元素 × 契约 × 声明抽取全量） | 面板拖慢架构页 | 中 | 面板按需触发（切到契约视图 / 点检查），MUST NOT 随 `refresh` 自动跑（AC-12）；`use-archi-api-check` 零轮询；将来超阈值可换「按元素分片 + 前端缓存」而不改 DTO |
| R-8 | **`bindings/mod_test.rs` 五处硬编码计数与两组清单** | 漏改一处即门禁红；两处非计数断言（尾部序期望 vec、族末条对照臂）易漏 | 高 | AC-17 逐项点名（51 → 53 / 101 → 104 / 6 → 8 / 23 → 26 + 尾序 + 对照臂）；spec 的清单守卫条款已含「同 change 随动」 |
| R-9 | **前端既有断言成批变红**（分段控件由两钮变三钮；抽屉新增段落） | 改写时顺手删掉真断言 | 中 | 改写口径写死：文件 / 元素两视图断言逐字保留，只新增第三钮与契约段断言；`file-tree.test.tsx` 与 `element-drawer.test.tsx` 的既有用例零删除 |
| R-10 | **agent 工具输出体量**（41 条判据 + 三类差异） | 挤压会话上下文 | 中 | 输出过 `tools::cap_result`（L1 上限单点）；`check` 动作给「按 `category` 计数摘要 + 逐条」两级，超限时摘要优先保留 |
| R-11 | **冷启动态被渲成空面板**（`enabled = false` 时差集只有 `INDEX.toml`） | 用户以为「契约面没东西」而非「还没启用」 | 中 | AC-10 的 `enabled = false` 用例断言「冷启动说明 + missing 一条」；spec 写成硬条款 |
| R-12 | **`FileViewer` 路径口径**（`archi::display` 只 strip 前缀不做规范化，反斜杠归一为 `/`） | 「打开」按钮取数失败或开错文件 | 低 | AC-3 断言路径串形态与 `archi::display` 同源；`read_file` 已是通用绝对路径读面，前端以 `root + '/' + 相对路径` 组装（沿 `file-tree` / `element-drawer` 既有打开路径做法） |
| R-13 | **生成物仍可被 agent 手改**（本轮不锁写保护） | 手改后零 diff 门会红，但发生在事后 | 低 | 硬线二裁决留痕 + 三条重开条件写入 proposal；面板直接呈现「内容不符」，可见性前置；`archi-knowledge.md` 明写「MUST NOT 手改生成物，用既有命令重生成」 |
| R-14 | **knip 把新 hook 报为未使用** | `client:check` 红 | 中 | 新 hook 由 `ContractPanel` 真实消费（MUST NOT 只被测试引用）；`ArchiApiCheck` 等类型经 `bindings.ts` 出线即用；不新增 ignore 条目 |
| R-15 | **U1 口径被误读为「架构页仍不能生成 = B 没做完」** | 期望落空 | 中 | `full` 模式一行提示写明两条现实通路（开 change / 页内会话执行 `pnpm api:export`）；proposal 的 U1/U2/U3 三表留痕；D 面让「会话代劳」这条路可自查 |
| R-16 | **`apiFindings` 纳管后本仓永远 `warnings`** 的担忧 | 与口径 ② 的病灶混淆 | 低 | 本仓实测纳管子集 = 0（AC-8）；契约逐份落地时该元素的判据才纳管，是「自己刚承诺的东西没兑现」而非存量债；「无白名单、无开关、无部分豁免」的禁令逐字保留 |
