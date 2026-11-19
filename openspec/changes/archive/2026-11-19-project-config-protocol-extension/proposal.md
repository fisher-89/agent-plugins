# 提案: project-config-protocol-extension

> **变更**: project-config-protocol-extension
> **日期**: 2026-11-19
> **状态**: 待评估（explore 的协议形状与排期已锁定；本提案落定 explore §3.5 四项待定小口径，见「决策表」D10–D13 与「风险 R5」）
> **来源**: `explore.md`（requirement 类型；**只承载切片 1**——todo 原文「扩充 project 配置协议，可以指定编码语言，依赖 project（仅列出 project path 即可），配置页面支持 Agent 检测生成配置。（未来基于 project 划分架构模块、编码/测试按语言使用不同 prompt、编码/测试并行）」的协议字段与拓扑序安装部分）

---

## 问题

`projects` 元素今天是**单字段协议**（`{ root: string }`），且唯一的活消费者按**配置声明序**逐个安装——协议表达力与消费语义同时不足：

1. **无法标注编码语言**：`ProjectConfig` 只有 `root`，一个多语言仓库（TS 前端 + Rust 后端）在配置里无法表达「这个项目是 rust」。已有的语言线索是间接的（`tests[].framework` 八值隐含四个语言族，`mutation_tool(framework)` 已按框架分派），但那是测试维度的事实，不是项目维度的标注，且不在 `projects` 元素上。
2. **无法声明项目间依赖**：`projects` 之间没有任何边，安装顺序纯声明序（`core/workflow/src/write/create.rs:314-326` 的 `bootstrap` 循环）。依赖方排在依赖方之前时，其构建 / 安装会在依赖产物缺失的窗口里执行，失败只落一句警告。
3. **多处 spec 硬承诺与本需求直接冲突**（三份能力、六处条款，必须在同一变更内显式修订，否则实现即违约）：
   - `desktop-change-worktree`「确定性依赖 bootstrap」：「项目根仅承载扫描 / 安装锚点，**MUST NOT 承载依赖引导之外的语义**」——`language` 就是「依赖引导之外」的语义；
   - 同 requirement：「**按配置顺序**逐个解析 `<worktree>/<root>`」+ scenario「多项目逐个扫描安装」——拓扑序会改变顺序；
   - `desktop-workspace-config`「配置解析 core crate」：「`projects` SHALL 为顶层数组（元素 `{ root: string }`…）」——元素形状要扩；
   - 同 spec「`config.schema.json` 静态镜像与实现一致性」：「镜像内容 SHALL 为**迁入前的逐字副本**」——镜像必须随字段面变更；
   - `desktop-change-create` 告警 scenario 的括注「`projects` 键只承载依赖引导项目根」——承载面即将变宽。
4. **`assemble_project` 的静默忽略是新增字段的最坏落点**：`crates/core/config/src/lib.rs:697-737` 对合法元素的未知子键**逐字忽略、零诊断**。新增字段若不同批更新这里，用户配了等于没配，且没有任何提示——`context` / `rules` 已经演示过这个结局（两个字段在案、零消费），本次是第三次机会。

## 提案

以「**两个软规则协议字段 + 一个真实消费者（拓扑序安装） + 配置页呈现 + spec 语义改判**」收口，一次落定切片 1：

1. **`projects[].language`（开放字符串，零校验）**：`Option<String>`；缺失 / 显式 null → `None` 零诊断；字符串 → `Some` 原样保留（**不 trim、不大小写 / 别名归一、不白名单、不与 `tests[].framework` 交叉校验**）；非字符串 → `None` + `InvalidValue`（复用既有 `opt_string`）。不做枚举的理由：当前仓库自己的配置就没有 `language`，必填或白名单会立刻制造诊断噪音；`language: rust` + `framework: jest` 不会被拦（软规则）。
2. **`projects[].depends_on`（字符串数组，零引用校验）**：`Option<Vec<String>>`；缺失 / 显式 null → `None` 零诊断；全字符串数组 → `Some`（保序、不去重、零值校验）；非数组 / 混入非字符串 → `None` + `InvalidValue`（复用既有 `opt_string_list`）。四类脏数据（悬空 / 自引用 / 成环 / 重复）**协议层一律不判**，免疫责任转移给消费侧——唯一消费者用 `visited` 天然具备该性质，**一行校验代码都不写**，`assemble` 仍是单遍、无跨元素视野，自洽不变量（`to_value(assemble(x))` 再过一遍诊断恒空）不受影响。
3. **活消费者：拓扑序安装**（`depends_on` 的唯一真实用途）：`bootstrap` 的项目遍历序改由**后序 DFS**（依赖先于依赖方）产出，无依赖或同层者保持**配置声明序**（声明序是唯一 tie-break，MUST NOT 改按字典序）；悬空依赖跳过、自引用 / 成环按 `visited` 收口、同名 `root` 折叠为一次安装（以首次出现为准）——四种脏数据一律零 `Err` 零诊断。既有「每项目首匹配单安装」「失败继续下一个项目」语义逐字保留，退化情形（无 `depends_on`）安装序等于声明序，故既有项目模式用例（多项目 / 首匹配 / 缺失根 / 无 lockfile 注记 / 抑制根扫描 / 失败继续 / 点号根）的断言结构不变。
4. **入参形态收纯数据，不引入新 crate 依赖边**：写面新增 `BootstrapProject { root, depends_on }`（`depends_on` 缺省 `Vec::new()`，写面不区分「未设」与「空」），`create` / `promote_explore` 的项目入参由 `&[String]` 改为 `&[BootstrapProject]`；命令层做 `config::ProjectConfig → BootstrapProject` 映射（`language` 在安装路径不消费）。**替代方案**（直接收 `&[config::ProjectConfig]`，先例 `core/checks` 收 `config::TestSuite`）方向安全但会为一段纯排序逻辑引入 `workflow → config` 的 crate 依赖，除非将来写面还要读配置其它字段，否则不划算。
5. **schema 镜像同步、机械闭环零新增**：`crates/core/config/config.schema.json` 的 `projects.items.properties` 补 `language`（`type: string`）/ `depends_on`（`array<string>`，description 写「值宜为某个 `projects[].root`」作**提示文案**），两条既有一致性断言（八项键集 / `TestFramework` 八值枚举）**零改动**——这是软规则路线的附带收益：无枚举、无白名单就没有可断的机械锚。
6. **配置页呈现面**：`ProjectsGroup` 逐项目呈现 `root` + 已设的 `language` / `depends_on`（未设标注「未设」，呈现面 MUST NOT 隐藏已配置值）；页面仍**只读**，不新增编辑 / 写回入口（Agent 检测生成配置属切片 2，不在本变更）。
7. **spec 改判与立规**：`desktop-change-worktree` 的语义禁令从「只允许一件事」收窄为「**不许变成通用元数据口袋**」（承载扫描 / 安装锚点、编码语言标注、项目间依赖声明三项，其余 MUST NOT），安装顺序与两个 scenario 改写；`desktop-workspace-config` 扩元素形状、改写镜像「逐字副本」条款、扩配置页呈现面；新增一条纪律 requirement「项目协议语义边界与消费点纪律」——把「`depends_on` 是平级边不是层级来源」「`language` 协议不归一、消费侧归一单点」「**新增字段须有消费点或显式标注为声明型**」「字段消费点表」四条钉进 spec，防第三次「已声明、零消费」。

决策表（explore 裁决已锁定项 + 本提案落定的待定小口径）：

| 编号 | 决策 | 值 |
|---|---|---|
| D1 | `language` 类型 | `Option<String>`，开放字符串、零内容校验、不影响元素去留 |
| D2 | `depends_on` 类型 | `Option<Vec<String>>`，零引用校验、保序、不去重、不影响元素去留 |
| D3 | `depends_on` 语义 | 平级边（安装顺序 + 将来影响面传播）；**不**充当架构模块层级来源（层级唯一真相源 = FQN 最长前缀） |
| D4 | 安装序算法 | 后序 DFS + `visited`；同层 tie-break = 配置声明序（非字典序） |
| D5 | 脏依赖处置 | 悬空跳过 / 自引用与成环按 visited 收口 / 重复 root 折叠为首次出现；零 `Err`、零诊断 |
| D6 | 写面入参 | 新增 `BootstrapProject`，`create` / `promote_explore` 签名随动；不引入 `workflow → config` 依赖 |
| D7 | schema 镜像 | 补两属性 + description；两断言零改动、不新增第三条断言 |
| D8 | 配置页 | 只呈现（含「未设」标注），**不**新增编辑 / 写回 |
| D9 | 版本纪律 | `packages/desktop/package.json` patch +1（现值 `0.6.59` → `0.6.60`）；`src-tauri/Cargo.toml` 不跟 |
| D10 | `language` 是否 trim | **不 trim**（协议零智能；归一只在消费侧） |
| D11 | `depends_on` 落盘是否去重 | **不去重**（写盘保真；消费侧 Set 化） |
| D12 | `BootstrapProject.depends_on` 形态 | `Vec<String>`（写面不需要区分「未设」与「空」） |
| D13 | 重复 `root` 的语义 | 以**首次出现**为准，后续重复项折叠（安装恰一次）——**知情的行为变更**，见 R5 |

**明确不做**（均为知情边界，非遗漏）：

- `language` 白名单 / 大小写归一 / trim / 与 `tests[].framework` 交叉校验（软规则裁决；交叉校验留作将来的可选 warn 级诊断）。
- `language_key(&str)` 归一函数的实现（切片 3 前置，本变更只立「消费侧归一单点」纪律）。
- `depends_on` 的引用存在性 / 自引用 / 成环 / 重复项校验（协议层不做，消费侧免疫）。
- 按语言分派 prompt 附录、按语言静态检查 / 测试分派、编码 / 测试并行、基于 project 划分架构模块（**切片 3**，见 explore §4 路标）。
- 配置页的「Agent 检测生成配置」与任何 config **写回**（**切片 2**，见 `openspec/explores/config-agent-session.md`；本变更页面仍只读）。
- `projects` 落库 / 增删命令（`projects` 只来自 config.json 读取，db 面零改动）。
- `KNOWN_KEYS` 变更（`projects` 已在列八项之内）。
- `tests[].root` 的基准口径与校验口径对齐、`AGENT.md` 的 prompt 份数漂移（14 vs 16）——explore §5 顺手发现的相邻漂移，属独立议题，本变更不混入。

### 范围与切片边界

| 切片 | 内容 | 本变更 |
|---|---|---|
| 1 | `projects[].language` + `projects[].depends_on` 协议字段；依赖引导改拓扑序安装 | **在范围内** |
| 2 | 配置页 Agent 检测生成配置（会话式写回） | 不在范围内（独立议题，`openspec/explores/config-agent-session.md`） |
| 3 | 语言归一单点 + 按语言 prompt 附录；架构模块划分边界裁决；影响面传播；编码 / 测试并行 | 不在范围内（路标） |

## 能力

### 新增的能力

无。两个新字段是 `projects` 元素的字段语义（归 `desktop-workspace-config` 的配置协议与呈现面），拓扑序安装是 `create` 依赖引导的顺序语义（归 `desktop-change-worktree`）——单拆一个 `desktop-project-protocol` 之类的能力只会造出横跨 config / workflow / 前端三处的悬空契约，且与既有「配置契约在 desktop-workspace-config、bootstrap 语义在 desktop-change-worktree」的分工冲突。

### 修改的能力

- **desktop-workspace-config** —
  - **MODIFIED**「配置解析 core crate」：`projects` 元素形状由 `{ root: string }` 扩为 `{ root, language?, depends_on? }`（字段集封闭，其余子键静默忽略）；逐字段处置（缺失 / null 零诊断；合法原样；类型违例落 `None` + `InvalidValue`；均不影响元素去留）；校验规则清单补「两字段只校验类型、内容零校验」；新增三个 scenario（原样保留 / 类型违例 / 自洽不变量）。
  - **MODIFIED**「`config.schema.json` 静态镜像与实现一致性」：镜像内容条款由「迁入前逐字副本」改为「与实现字段面一致（迁入时逐字承接，此后手工维护 + 同 commit 同步）」；新增「软规则字段只入 `type` + `description`、MUST NOT 新增第三条断言」条；对齐与同步两 scenario 相应改写。
  - **MODIFIED**「配置页呈现面」：`projects` 行由「逐 project 的 root 列表」改为「逐 project 呈现 root + 已设的 language / depends_on（未设标注「未设」）」，新增呈现 scenario。
  - **ADDED**「项目协议语义边界与消费点纪律」：`depends_on` 平级边 / 非层级来源；`language` 协议不归一、消费侧归单单点；新增字段须有消费点或显式标注声明型；字段消费点表（含 `context` / `rules` 零消费欠账在案）。
- **desktop-change-worktree** —
  - **MODIFIED**「确定性依赖 bootstrap」：项目模式安装序改「`depends_on` 拓扑序（依赖先于依赖方），无依赖 / 同层保持声明序」；语义禁令由「MUST NOT 承载依赖引导之外的语义」收窄为「承载三项、三项之外 MUST NOT」；「多项目逐个扫描安装」改写为「多项目按拓扑序安装」，新增「无依赖保持声明序」「悬空与自引用不阻断」「成环依赖按声明序落定」「重复 root 折叠安装一次」四 scenario；「警告按配置序累积」改为按安装序。
- **desktop-change-create** —
  - **MODIFIED**「写面 create 操作进程内创建 change」：仅订正告警 scenario 内「`projects` 键只承载依赖引导项目根」的括注为「只承载项目锚点声明（`root` / `language` / `depends_on` 三项）」——上限 5 / 门槛 7 与前置八 / 九道校验零改动（零实现触点）。

其余能力零改动，理由：

- **desktop-ipc-type-bindings**：生成机制零改；`ProjectConfig` 出线变宽由 `bindings:export` 自动带出（`bindings.ts` + `bindings.zod.ts` 两个生成物）。
- **desktop-explore-queries**：`promote_explore` 的 spec 只约束命令面语义与「复用 change create」，`workflow::write::promote_explore` 的**内部**项目入参形态不在 spec 承诺面内，纯透传随动。
- **desktop-crate-layout / repo-archi-model**：不新增 crate、不新增跨 crate 依赖边（`workflow` 仍不依赖 `config`）；契约资产零改动（`App.Backend.Core.Config` / `Workflow` 两个元素本无 `.md` 契约文档，`api:check` 预期零 diff）。
- **desktop-write-protection**：`config.json` 内置锁不变。
- **desktop-change-state-store**：`projects` 不落库，store 模型零改动。
- **desktop-standard-components**：呈现复用既有分区组件形态，不新增标准组件。

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `packages/desktop/src-tauri/crates/core/config/src/lib.rs` | `ProjectConfig` 增 `language: Option<String>` / `depends_on: Option<Vec<String>>`（字段注释写明「开放字符串 / 零校验」「值宜为某个 `projects[].root`、零引用校验」）；`assemble_project` 在两字段就位后追加 `opt_string(map, "language", &format!("{project_path}.language"), diagnostics)` 与 `opt_string_list(map, "depends_on", &format!("{project_path}.depends_on"), diagnostics)`；两字段**不参与**元素去留判定；`KNOWN_KEYS` 零改动 |
| `packages/desktop/src-tauri/crates/core/config/config.schema.json` | `projects.items.properties` 增 `language`（`"type": "string"` + description「编码语言标注（自由文本，不做归一 / 校验）」）/ `depends_on`（`"type": "array", "items": {"type": "string"}` + description「依赖的项目根（值宜为某个 `projects[].root`；不做引用校验）」）；`required` 仍只有 `root` |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/create.rs` | 新增 `pub struct BootstrapProject { pub root: String, pub depends_on: Vec<String> }`；新增私有纯函数 `install_order(projects: &[BootstrapProject]) -> Vec<&str>`（后序 DFS + `visited` + 声明序 tie-break，零 `Err`、零 IO）；`create` / `bootstrap` 的项目入参由 `project_roots: &[String]` 改为 `projects: &[BootstrapProject]`，`bootstrap` 遍历序改由 `install_order` 产出；`bootstrap_project` 的探测 / 警告文案 / 单安装 / 失败继续语义逐字不动 |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/promote.rs` | `promote_explore` 项目入参随 `create` 同形态（纯透传） |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/mod.rs` | 出口面增 `pub use create::BootstrapProject;`（命令层构造入参） |
| `packages/desktop/src-tauri/src/commands/changes/mod.rs` | `project_roots: Vec<String>` 改为 `projects: Vec<BootstrapProject>`（`config::load(&main_root).config.projects` 映射：`root` 原样 + `depends_on.unwrap_or_default()`；`language` 不消费），`write::create(...)` 实参随动 |
| `packages/desktop/src-tauri/src/commands/explores/mod.rs` | 同上映射，`write::promote_explore(...)` 实参随动 |
| `packages/desktop/src/types/generated/bindings.ts` / `bindings.zod.ts` | `pnpm run bindings:export` 再生成（非手改）：`ProjectConfig` 携 `language` / `dependsOn` 可空 |
| `packages/desktop/src/views/config/components/basic-config-section.tsx` | `ProjectsGroup` 逐项目呈现 `root` + `language` / `depends_on`（已设即列出，未设标注「未设」；依赖项并列展示；保持既有 `data-testid="config-projects"` 与只读形态） |
| `packages/desktop/package.json` | `version` patch +1（`0.6.59` → `0.6.60`）；`src-tauri/tauri.conf.json` 经 `../package.json` 跟随、`src-tauri/Cargo.toml` 不跟 |

### 测试文件

| 用例 / 断言 | 位置 | 处置 |
|---|---|---|
| 两字段缺失 / 显式 null → `None` 且零诊断 | `crates/core/config/src/lib_test.rs` | 新增 |
| 合法值原样保留（`language` 不 trim / 不归一；`depends_on` 保序不去重，含悬空与重复项） | 同上 | 新增 |
| 类型违例（`language` 非字符串；`depends_on` 非数组 / 混入非字符串）→ 字段落 `None` + `InvalidValue` 且元素仍在案 | 同上 | 新增 |
| 自洽不变量：含两字段的 config 回写再 `assemble` 诊断恒空 | 同上（既有自洽用例扩一条 fixture） | 扩展 |
| 镜像 `projects.items.properties` 含 `root` / `language` / `depends_on` 三项（两断言锚点零改动） | 同上 | 新增一条断言 |
| 夹具 `create_with_projects` 收 `&[BootstrapProject]`；既有九例（多项目 / 首匹配 / 缺失根 / 无 lockfile 注记 / 抑制根扫描 / 空 projects / 失败继续 / 脏仓警告先于项目警告 / 点号根）断言与文案不变 | `crates/core/workflow/src/write/create_test.rs` | 夹具扩展 + 调用点随动 |
| 拓扑序：声明序 [src-tauri(dep→desktop), desktop] → 安装序 desktop 先 | 同上 | 新增 |
| 无依赖退化：三项目无 `depends_on` → 安装序 = 声明序 | 同上 | 新增 |
| 悬空 / 自引用：零 `Err`、其余项目照常安装、无新增警告 | 同上 | 新增 |
| 成环 A→B→A：各安装恰一次、顺序确定、零 `Err` | 同上 | 新增 |
| 重复 `root`：安装恰一次（首次出现为准） | 同上 | 新增 |
| `promote_explore` 空项目实参形态随动 | `crates/core/workflow/src/write/promote_test.rs` | 实参形态调整（断言零变化） |
| `ProjectsGroup` 呈现 `language` / `depends_on` / 未设标注 / 只读无编辑控件 | `packages/desktop/src/views/config/components/basic-config-section.test.tsx` | 新增用例 |

### 不要修改

- **`explore.md`**（本 change 的）：只读。
- **`crates/core/config` 的 `KNOWN_KEYS`**、`TestFramework` 八值枚举、`config.schema.json` 的两条一致性断言：本次不新增键、不改枚举、不加第三条断言。
- **`assemble` 的单遍结构与既有 `opt_string` / `opt_string_list` 助手**：直接复用，不为其新增能力或参数。
- **`bootstrap_project` / `bootstrap_worktree_root` 的映射表、警告文案与文案常量**：拓扑序只换遍历顺序，不改任何文案与「每项目单安装」「失败继续」语义。
- **`crates/core/workflow/Cargo.toml`**：不新增依赖（尤其不新增 `config`：`BootstrapProject` 收纯数据的理由）。
- **`crates/core/archi`**、`.dev-team/architecture/models/`、`.dev-team/architecture/graph/**`：架构契约资产零改动（本次不动 FQN 层级语义、不引入 `depends_on` 作层级源）。
- **store / db 面**（`SessionRecord` / `ChangeRecord` / 迁移链 / 注册模型）：`projects` 不落库，零改动。
- **`packages/desktop/src/views/config/` 的只读边界**：不新增任何写回 config.json 的命令、hook 或表单。
- **`.dev-team/config.json`**：本仓库自身的配置是否需要加 `language` / `depends_on` 属落地时的可选项，MUST NOT 为演示需求强改；若改，走值级替换。
- **`openspec/todo.md`**：待办勾选与拆分由流程面管理，实现期不改。

## 验收标准

| ID | 验收条件 | 验证方法 |
|----|---------|---------|
| AC-1 | `ProjectConfig` 携 `language` / `depends_on` 两可空字段；缺失 / 显式 null → `None` 且 diagnostics **零新增**；合法值**原样保留**（`language` 不 trim / 不归一；`depends_on` 保序、不去重、悬空项在案） | `lib_test` 新增矩阵 |
| AC-2 | 类型违例逐字段处置：`language` 非字符串 / `depends_on` 非数组或混入非字符串 → 字段落 `None` + `InvalidValue`（path `projects[i].language` / `projects[i].depends_on`），**元素本身仍在案**（仅 `root` 决定去留）；同元素其余合法字段不受牵连 | `lib_test` 新增矩阵 |
| AC-3 | 自洽不变量不破：含两字段的合法 config 重序列化回 `Value` 再经同一组装，诊断恒空 | `lib_test` 自洽锚 |
| AC-4 | 安装序为拓扑序：依赖先于依赖方；无依赖或同层保持声明序（退化情形安装序 = 声明序）；悬空 / 自引用 / 成环 / 重复 `root` 四类脏数据零 `Err`、零新增警告、产出确定序列（重复 `root` 安装恰一次） | `create_test` 新增五例 + 既有九例断言不变 |
| AC-5 | 写面入参为 `BootstrapProject`，`core/workflow` 的 `Cargo.toml` 依赖零新增（无 `config` 边）；两命令层映射到位（`language` 在安装路径不消费） | 源码复核 + `cargo tree -p workflow` 依赖面 |
| AC-6 | schema 镜像 `projects.items.properties` 含 `root` / `language` / `depends_on` 三项且 description 为提示文案；两条一致性断言（八项键集 / 八值枚举）**零改动通过** | `lib_test` + 镜像 JSON 复核 |
| AC-7 | 配置页 `projects` 区逐项目呈现已设的 `language` / `depends_on`（未设标注「未设」），页面仍只读（无编辑 / 保存入口） | `basic-config-section.test.tsx` 新增用例 |
| AC-8 | 全管线绿：`pnpm -C packages/desktop run check`（`server:check` → `bindings:export` → `api:check` → `client:check`）与 `run test` 全通过；绑定生成物零手改（diff 只来自 `bindings:export`）；`package.json` version 为 `0.6.60`、`src-tauri/Cargo.toml` 版本不随动 | 静态检查链 + 全量测试 + `git diff` 复核 |

> **验收面口径**：验收止于**单元测试 + 静态检查链**。`language` 在切片 1 内**没有**运行时消费者（唯一活消费者是 `depends_on` → 拓扑序安装），因此 MUST NOT 以「按语言跑通某条流程」作为验收条——那属切片 3。

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|---|---|---|---|
| **R1 `language` 是声明型字段，可能重演「已声明、零消费」**：本期除「配了能看见」外无工程消费者（真实消费者在切片 3：按语言 prompt 附录 / 按语言检查分派） | 与 `context` / `rules` 同一结局：字段在案但无人读，价值被质疑 | 高 | 不掩饰：本提案与 spec 明确标注其为声明型字段并列出计划消费者；新增 discipline requirement 把「新增字段须有消费点或显式标注声明型」立成规则；`depends_on` 同批给出真实消费者，避免两个新字段一起悬空 |
| **R2 软规则长期漂移**：无归一 ⇒ `TypeScript` / `ts` / `typescript` 三分；切片 3 若直接按原文 match 必漏 | 语言分派漏判，且是静默漏判 | 高（切片 3 引入消费时） | spec 立「消费侧归一单点（`language_key` 类函数，落 `core/config`）」纪律；消费点表标注 language 为声明型；切片 3 开工前置条件写入 explore 路标 |
| **R3 脏依赖静默**：悬空 / 成环不报诊断，用户可能看不懂「安装顺序为什么是这样」 | 配置错误无反馈 | 中 | 裁决即「零校验 + 消费面免疫」（保 `assemble` 单遍与自洽不变量）；配置页呈现 `depends_on` 使配置可见；错误后果温和（顺序退化 = 现状），不产生 `Err` / 不阻断 create；将来如要反馈可加 warn 级诊断（非本次） |
| **R4 拓扑序改变既有安装顺序**：现有多项目配置若隐式依赖声明序，顺序调整后会不同 | 少数用户安装序变化（多为改善） | 中 | 退化情形（无 `depends_on`）安装序逐字等于声明序，AC-4 有锚；顺序仅在显式声明依赖时改变，且方向是「依赖先装」 |
| **R5 重复 `root` 语义变化（知情行为变更）**：现状两条同 `root` 元素会安装两次，本变更折叠为一次（以首次出现为准） | 依赖「重复声明 = 重复安装」的特例用户行为变化 | 低 | D13 显式裁决并在 spec scenario 留痕；前端既有注释即「重复 root 允许」，折叠语义更符直觉；如需重复安装属另一诉求，另立变更 |
| **R6 spec 改判漏改即实现违约**：`desktop-change-worktree` 两条（语义禁令 / 按配置顺序 + scenario）与 `desktop-workspace-config` 三条（元素形状 / 镜像逐字副本 / 呈现面）都是硬承诺 | 实现完成仍被判违约 | 中 | 本变更的 delta spec 已把这六处条款全部 MODIFIED（三份能力：workspace-config 3 处 + ADDED 1 条、change-worktree 3 处、change-create 1 处）；AC-4 / AC-6 / AC-7 分别对位；评审以「基线全文是否有残留冲突句」为复核项 |
| **R7 签名变更面扩散**：`create` / `promote_explore` / `bootstrap` / 两命令层 / 测试夹具（`create_with_projects` 9 处调用点、`create_full` 与 `create_with_projects` 两处直连 `create(...)`、`promote_test` 1 处） | 漏改 → 编译错（安全），但改动面噪音大 | 中 | 编译期强制全覆盖；夹具统一收 `&[BootstrapProject]` 使绝大多数用例（48 处 `.create(...)` 封装调用）零改动；`install_order` 为私有纯函数，单测与集成测皆可覆盖 |
| **R8 镜像与实现漂移无机械拦截**：新增两字段不入一致性断言（软规则路线），忘记同步 `config.schema.json` 无人报 | 编辑器补全与实际字段不一致（不崩、不失败） | 中 | AC-6 显式核对镜像三项；spec 立「同 commit 同步」纪律；实现时以 `ProjectConfig` 字段集与 `projects.items.properties` 键集逐项对照 |
| **R9 api 契约面预期为 no-op，但需实证**：`App.Backend.Core.Config` / `Workflow` 两元素无 `.md` 契约文档，增字段预期不改 `apiFindings` | 若 `api:check` 反而报新增 finding，实现期需裁决归属（补契约文档 or 判定为既有 `docMissing` warn） | 低 | AC-8 以 `pnpm run check` 全绿为门；若报 finding，实现相位按 `api-check` 判据裁决，属该相位处置面，不回退本提案的协议裁决 |
