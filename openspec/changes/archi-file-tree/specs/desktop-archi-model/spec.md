# desktop-archi-model Specification（delta: archi-file-tree）

## RENAMED Requirements

### RENAMED: IPC 命令面契约（四命令） → IPC 命令面契约（六命令）

- **FROM**: `### Requirement: IPC 命令面契约（四命令）` —— 命令层提供四个命令（`archi_graph` / `archi_module` / `archi_change_summary` / `archi_set_gate`），全部为薄包装；同一 requirement 的 Scenario「archi_module 出参带 api 并列字段」末句又写「命令总数仍为五个」，标题与 Scenario 口径不一。
- **TO**: `### Requirement: IPC 命令面契约（六命令）` —— 内容如下：

命令层 SHALL 提供六个命令，全部为薄包装（沿「三件事纪律」：解析入参 → 调 `archi` / store → 映射错误），MUST NOT 承载领域规则（投影、诊断、diff 全在 `archi`）：

| 命令 | 入参 | 出参 / 语义 |
|---|---|---|
| `archi_graph` | `root` | `ArchiGraph`：正常态（元素 / 关系 / 标记）或停摆态（诊断清单、零图数据） |
| `archi_module` | `root` + `element` | `ArchiModuleDetail`（与 agent 工具同一投影）：元素 + 邻接 + 子元素 + `docs` + **`api`（并列字段）** |
| `archi_change_summary` | `root` + `change_id` | `Option<ArchiChangeSummary>`：以 change 记录解析 worktree（归档 change 回落主仓根），两侧模型结构 diff；模型目录缺席或两侧等价 → `null` |
| `archi_set_gate` | `root` + `element` + `gate` | 写前校验（元素存在 / 值合法 / 渲染自证可解析）通过后原子写回该模型文件；失败零写入并显式 `Err` |
| `archi_drift` | `root` + `mode` | `ArchiDriftReport`：全量 / 增量两模式同源判定（`mode` = `full` \| `incremental`）；命令层零判定规则 |
| `archi_file_tree` | `root` | `ArchiFileTree`：仓库源文件与模块根的扁平条目投影（左栏文件树，契约归 `desktop-archi-file-tree`）；结构诊断非空 → `Err` |

文档正文读取 SHALL 由通用 `commands::fs::read_file`（见 `desktop-file-read`）按绝对路径承担：原 `archi_read_doc`（关联集成员校验 + 读盘）已退役，架构域不再持有专用文档读取命令。

- 全部命令 SHALL 带显式 `root`（MUST NOT 依赖「当前选中 change」一类隐式上下文）；错误面 SHALL 为 `Result<T, String>`（沿既有命令面口径）。
- `archi_graph` / `archi_module` / `archi_change_summary` / `archi_drift` / `archi_file_tree` MUST NOT 写盘；唯一写命令 SHALL 为 `archi_set_gate`（且只改 `gate` 单字段）。
- `archi_set_gate` MUST NOT 出现在 `mode = full` 的前端调用路径（只读视图约束由前端承载、由命令可用性背书——命令本身不区分模式）。
- 六个命令 SHALL 登记进命令清单并随 `bindings:export` 生成到 `bindings.ts`（再生成后入库，过期由提交前 `git status` / `git diff` 人工核对——零 diff 守卫已退役，`check` 链只前置重导出、不做生成物比对）。命令清单与出线类型清单的机械守卫（`bindings/mod_test.rs`）SHALL 在同一 change 内随动（条数断言只作清单长度连带核对，MUST NOT 作为语义判据）。
- **`archi_module` 的 `api` 并列字段**：本变更为 `ArchiModuleDetail` 增加并列字段 `api`（`ArchiElement` 与薄概览条目同步带 `api` 状态），语义为「该元素的契约是否在案」——按 `<archi_api_dir>/<Domain>/<FQN>.md` 约定定位，文件不存在即 `status = none`（零配置，且「有契约 / 无契约」成为可枚举事实）。字段结构与定位规则定义归 `desktop-archi-api`，本能力只登记「`archi_module` 出参形状扩展、命令层仍零领域规则」。该字段 MUST NOT 承载协议正文（契约正文与模块文档正文同一通路：由调用方经 `read_file` 自取）。

#### Scenario: 读命令零写盘

- **WHEN** 依次调用 `archi_graph` / `archi_module` / `archi_change_summary` / `archi_drift` / `archi_file_tree`
- **THEN** 磁盘字节零变化（模型、源码与文档均只读）；取数结果与 `archi` 投影逐字段一致（命令层零改写）

#### Scenario: 文档关联集不含契约正文

- **WHEN** 审查 `archi_module` 返回的 `docs` 条目集合
- **THEN** 集合内不含 `<archi_dir>/api/**` 路径（关联集零契约正文通路）；契约正文的存在与否只经 `api` 字段表达

#### Scenario: gate 写回原子与零残留

- **WHEN** 调用 `archi_set_gate` 改合法元素的 `gate`，以及以非法值 / 不存在元素调用
- **THEN** 前者目标行变为新档位且其余内容与注释逐字保留；后者显式 `Err` 且该文件字节零变化（校验先于任何写动作）

#### Scenario: archi_module 出参带 api 并列字段

- **WHEN** 调用 `archi_module(root, element = "App.Backend.Infra.ArchiRuntime")`（该元素契约在案）与另一无契约元素
- **THEN** 前者 `api.status = "declared"` 且 `contractPath` / `schemaPath` 指向实际文件；后者 `api.status = "none"` 且三个路径字段为 `null`；两者其余字段（元素 / 邻接 / 子元素 / `docs`）语义零变化，命令总数仍为六个

#### Scenario: 六命令在册且只一个写面

- **WHEN** 审查 `all_commands!` 的 archi 段与 `bindings.ts` 的 typed 包装
- **THEN** archi 段恰六条（`archi_graph` / `archi_module` / `archi_change_summary` / `archi_set_gate` / `archi_drift` / `archi_file_tree`），每条各有对应包装；源码中含写盘调用的命令恰为 `archi_set_gate` 一条

## Module Contract

主基线既有行逐字沿用（含 `crates/core/archi/**` 各模块、`src-tauri/src/commands/archi/mod.rs` 与 `crates/core/archi/tests/**`）；本 delta 受影响模块如下：

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `src-tauri/src/commands/archi/mod.rs` | **六命令**薄包装 | `archi_graph` / `archi_module` / `archi_change_summary` / `archi_set_gate` / `archi_drift` / `archi_file_tree`，各配一个 `*_inner` 领域组装纯函数（离 Tauri 可测）；`Result<T, String>` 错误面；唯一写命令为 `archi_set_gate`（写前校验 + 原子写回）；命令层零领域规则（文档正文经通用 `read_file`） |
| `src-tauri/src/commands/mod.rs` | 命令登记面 | `all_commands!` 的 archi 段 5 → 6 条（追加 `commands::archi::archi_file_tree`，位于 `archi_drift` 之后、`read_file` 之前；builder 与 `generate_handler!` 同源） |
| `crates/core/archi/src/tree.rs`（新增，见 `desktop-archi-file-tree`） | 文件树投影 | `file_tree(root) -> Result<ArchiFileTree, String>`：枚举 + 排除 + 归属 + 目录合成；纯读盘、零写盘、零 spawn；归属与剥离一律经 `OwnerIndex` |
| `crates/core/archi/src/dto.rs`（扩展，见 `desktop-archi-file-tree`） | 出线投影 | 新增 `ArchiFileTree` / `ArchiFileEntry` / `FileEntryKind` / `FileTreeMark`；既有九组封闭词汇与全部出线 DTO 字段名 / 顺序 / 语义零变化 |
| `src-tauri/src/bindings/mod_test.rs`（扩展，见 `desktop-ipc-type-bindings`） | 清单守卫随动 | 命令名 / 包装名清单 48 → 49、出线 DTO 清单 97 → 101、archi 族两组清单 4 → 6 与 19 → 23；尾部追加序断言把 `archi_file_tree` 插在 `archi_drift` 之后 |
| 本能力 `## Purpose`（归档合并时同步） | 口径订正 | 现文本写「与四命令 IPC 契约」，归档合并 SHALL 同步为「与六命令 IPC 契约」——delta 只承载 requirement 变更，Purpose 属合并时的同批订正，MUST NOT 让标题已改而 Purpose 仍写四（该类「同一事实两种自称」正是本变更要清的病灶） |
