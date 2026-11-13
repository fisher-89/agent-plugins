# desktop-archi-model Specification（delta: api-gen-check-in-desktop）

## RENAMED Requirements

### RENAMED: IPC 命令面契约（六命令） → IPC 命令面契约（八命令）

- **FROM**: `### Requirement: IPC 命令面契约（六命令）` —— 命令层提供六个命令（`archi_graph` / `archi_module` / `archi_change_summary` / `archi_set_gate` / `archi_drift` / `archi_file_tree`），「唯一写命令 SHALL 为 `archi_set_gate`」；同一 requirement 的 Scenario「archi_module 出参带 api 并列字段」末句写「命令总数仍为六个」。本变更新增检查 / 生成两条命令，六命令口径失效。
- **TO**: `### Requirement: IPC 命令面契约（八命令）` —— 内容如下：

命令层 SHALL 提供八个命令，全部为薄包装（沿「三件事纪律」：解析入参 → 调 `archi` / store → 映射错误），MUST NOT 承载领域规则（投影、诊断、diff 全在 `archi`）：

| 命令 | 入参 | 出参 / 语义 |
|---|---|---|
| `archi_graph` | `root` | `ArchiGraph`：正常态（元素 / 关系 / 标记）或停摆态（诊断清单、零图数据） |
| `archi_module` | `root` + `element` | `ArchiModuleDetail`（与 agent 工具同一投影）：元素 + 邻接 + 子元素 + `docs` + **`api`（并列字段）** |
| `archi_change_summary` | `root` + `change_id` | `Option<ArchiChangeSummary>`：以 change 记录解析 worktree（归档 change 回落主仓根），两侧模型结构 diff；模型目录缺席或两侧等价 → `null` |
| `archi_set_gate` | `root` + `element` + `gate` | 写前校验（元素存在 / 值合法 / 渲染自证可解析）通过后原子写回该模型文件；失败零写入并显式 `Err` |
| `archi_drift` | `root` + `mode` | `ArchiDriftReport`：全量 / 增量两模式同源判定（`mode` = `full` \| `incremental`）；命令层零判定规则 |
| `archi_file_tree` | `root` | `ArchiFileTree`：仓库源文件与模块根的扁平条目投影（左栏文件树，契约归 `desktop-archi-file-tree`）；结构诊断非空 → `Err` |
| `archi_api_check` | `root` | `ArchiApiCheck`：四类生成物与契约渲染结果的**三类差异**（`mismatched` / `orphaned` / `missing`）+ `enabled` + `meta`；零写盘（契约归 `desktop-archi-contract-panel`） |
| `archi_api_generate` | `root` | `ArchiApiWriteOutcome`：把应生成物写盘（`written` / `unchanged` 两列）；**只增只改、从不删文件**；结构诊断非空 → `Err` 且零写盘 |

文档正文读取 SHALL 由通用 `commands::fs::read_file`（见 `desktop-file-read`）按绝对路径承担：原 `archi_read_doc`（关联集成员校验 + 读盘）已退役，架构域不再持有专用文档读取命令。

- 全部命令 SHALL 带显式 `root`（MUST NOT 依赖「当前选中 change」一类隐式上下文）；错误面 SHALL 为 `Result<T, String>`（沿既有命令面口径）。
- `archi_graph` / `archi_module` / `archi_change_summary` / `archi_drift` / `archi_file_tree` / `archi_api_check` MUST NOT 写盘；写命令 SHALL 恰为两条——`archi_set_gate`（只改 `gate` 单字段）与 `archi_api_generate`（只写四类生成物，零删除），其余六条 MUST NOT 触碰磁盘。
- `archi_set_gate` MUST NOT 出现在 `mode = full` 的前端调用路径（只读视图约束由前端承载、由命令可用性背书——命令本身不区分模式）；`archi_api_generate` 同理只在 `mode = incremental` 的前端调用路径出现（生成权限归 `desktop-archi-contract-panel`）。
- 八个命令 SHALL 登记进命令清单并随 `bindings:export` 生成到 `bindings.ts`（再生成后入库，过期由提交前 `git status` / `git diff` 人工核对——零 diff 守卫已退役，`check` 链只前置重导出、不做生成物比对）。命令清单与出线类型清单的机械守卫（`bindings/mod_test.rs`）SHALL 在同一 change 内随动（条数断言只作清单长度连带核对，MUST NOT 作为语义判据）。
- **`archi_module` 的 `api` 并列字段**：本变更为 `ArchiModuleDetail` 增加并列字段 `api`（`ArchiElement` 与薄概览条目同步带 `api` 状态），语义为「该元素的契约是否在案」——按 `<archi_api_dir>/<Domain>/<FQN>.md` 约定定位，文件不存在即 `status = none`（零配置，且「有契约 / 无契约」成为可枚举事实）。字段结构与定位规则定义归 `desktop-archi-api`，本能力只登记「`archi_module` 出参形状扩展、命令层仍零领域规则」。该字段 MUST NOT 承载协议正文（契约正文与模块文档正文同一通路：由调用方经 `read_file` 自取）。

#### Scenario: 读命令零写盘

- **WHEN** 依次调用 `archi_graph` / `archi_module` / `archi_change_summary` / `archi_drift` / `archi_file_tree` / `archi_api_check`
- **THEN** 磁盘字节零变化（模型、源码、文档与四类生成物均只读）；取数结果与 `archi` 投影逐字段一致（命令层零改写）

#### Scenario: 文档关联集不含契约正文

- **WHEN** 审查 `archi_module` 返回的 `docs` 条目集合
- **THEN** 集合内不含 `<archi_dir>/api/**` 路径（关联集零契约正文通路）；契约正文的存在与否只经 `api` 字段表达

#### Scenario: gate 写回原子与零残留

- **WHEN** 调用 `archi_set_gate` 改合法元素的 `gate`，以及以非法值 / 不存在元素调用
- **THEN** 前者目标行变为新档位且其余内容与注释逐字保留；后者显式 `Err` 且该文件字节零变化（校验先于任何写动作）

#### Scenario: archi_module 出参带 api 并列字段

- **WHEN** 调用 `archi_module(root, element = "App.Backend.Infra.ArchiRuntime")`（该元素契约在案）与另一无契约元素
- **THEN** 前者 `api.status = "declared"` 且 `contractPath` / `schemaPath` 指向实际文件；后者 `api.status = "none"` 且三个路径字段为 `null`；两者其余字段（元素 / 邻接 / 子元素 / `docs`）语义零变化，命令总数仍为八个

#### Scenario: 八命令在册且恰两个写面

- **WHEN** 审查 `all_commands!` 的 archi 段与 `bindings.ts` 的 typed 包装
- **THEN** archi 段恰八条（`archi_graph` / `archi_module` / `archi_change_summary` / `archi_set_gate` / `archi_drift` / `archi_file_tree` / `archi_api_check` / `archi_api_generate`），每条各有对应包装；源码中含写盘调用的命令恰为 `archi_set_gate` 与 `archi_api_generate` 两条

## Module Contract

### API: `src-tauri/src/commands/mod.rs` + `src-tauri/src/commands/archi/mod.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `all_commands!` archi 段 | 6 → 8 条 | 追加 `commands::archi::archi_api_check` 与 `commands::archi::archi_api_generate`（位于 `archi_file_tree` 之后、`read_file` 之前；`read_file` 仍为尾条）；builder 注册与 `generate_handler!` 同源 |
| `commands/archi/mod.rs` | 改 | 文件头「六命令」→「八命令」、「唯一写命令是 `archi_set_gate`」→「写命令两条（`archi_set_gate` / `archi_api_generate`）」；新增两条命令 + 两个 `*_inner`；既有六条零改动 |
| `commands/archi/mod_test.rs` | 改 | root 校验用例由六命令扩到八命令并把名字里的旧口径改符实；新两条命令的 `_inner` 一致性与零写盘 / 写后差异清空用例 |

### CLI / Component: 零改动面

| 项 | 描述 |
|---|---|
| `crates/core/archi/src/dto.rs` 既有投影 | `ArchiGraph` / `ArchiModuleDetail` / `ArchiApiRef` / `ArchiFileTree` 等逐字不变（本 delta 只改命令面清单与写面身份） |
| 前端调用路径 | `archi_set_gate` 仍只在 `incremental` 路径出现；新增 `archi_api_generate` 的调用面归 `desktop-archi-contract-panel` |
