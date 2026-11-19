# 设计: project-config-protocol-extension

> **变更**: project-config-protocol-extension
> **状态**: 设计定稿（基于已定稿 proposal D1–D13 + 三份 delta spec）
> **基于**: `proposal.md`（决策表 D1–D13 / 明确不做 / 变更范围 / AC-1…AC-8 / R1–R9）、`specs/{desktop-workspace-config,desktop-change-worktree,desktop-change-create}/spec.md`、`explore.md`（§2 协议形状 / §3.2–3.5 草案与待定小口径）
> **性质**: **加字段 + 换遍历序 + 呈现面增列**——`projects` 元素由单字段扩为三字段（两软规则字段）；`bootstrap` 的项目遍历序由声明序改为 `depends_on` 后序 DFS 拓扑序；配置页 `projects` 行增两列呈现。
>
> **本篇边界**: `crates/core/config`（协议 + 镜像）、`crates/core/workflow`（`BootstrapProject` + 安装序）、`src-tauri/src/commands`（两处映射）、`packages/desktop/src`（配置页呈现）、版本行，另含 4 个既有测试文件的编译闭合适配（形参 / fixture 补键，断言零改，见「变更清单 - 测试文件（编译闭合）」）。**零新增 crate、零新增依赖边、零 db 改动、零 IPC 命令面改动、零契约资产改动**。

---

## 提案与规格同步状态

proposal 的十三条决策、明确不做清单、AC-1…AC-8 与三份 delta spec 已把语义钉到字段名 / 键名 / 顺序口径 / 呈现文案粒度。本设计不重复论证，只做四件事：

1. **把 spec 已钉语义、未钉落点的形状收敛到具体签名与代码形态**（DD1–DD12）：`assemble_project` 的插入位置与诊断顺序、`install_order` 的精确算法与四类脏数据的产出序列、`BootstrapProject` 的 derive 面、配置页呈现的确切文案与分隔符。
2. **对 proposal 的措辞做工作树校准**（见「事实校准」）：前端 fixture 的编译闭合面是 **4 处**（proposal 只列了 1 个测试文件）、`promote_test.rs` 的 `&[]` 无需改动、`bootstrap` 的 doc 注释与新语义冲突必须同批改、`bindings/mod_test.rs` 的 `ProjectConfig` 断言是 `contains` 式故不失败但注释语义过期。
3. **逐条 AC 给落点**（末尾「验收标准落点」表），不写测试策略（用例与门禁由独立相位承接）。
4. **把「非显然正确」写成可实现形态**：`visited` 必须在**递归前**置位（否则自引用 / 成环死循环）；重复 `root` 必须在**外层循环跳过非规范下标**（否则同 root 安装两次，违反 D13）；`Option` 字段线面回写为 `null`，组装层对 null 与缺键同处置，故自洽不变量天然成立（无需新增特例）。

---

## 起点核实（工作树实测；锚点为改动前行号）

- **config 协议面**：`crates/core/config/src/lib.rs:157` `ProjectConfig`（仅 `root: String`）；`:697` `assemble_project`（`root` 缺失 / 非法即整条剔除，合法时 `Some(ProjectConfig { root })`，未知子键静默忽略）；`:289` `opt_string` / `:313` `opt_string_list`（两者处置口径正是两新字段所需，零改造可复用）；`:27` `KNOWN_KEYS` 八项（`projects` 已在列）；`:190` `WorkspaceConfig.projects`。
- **schema 镜像**：`crates/core/config/config.schema.json` 的 `projects.items.properties` 只有 `root`，`required` 为 `["root"]`；一致性断言在 `lib_test.rs` 末段（A1 `KNOWN_KEYS` 八键集 / A2 `TestFramework` 八值枚举 / A3 往返），镜像经 `include_str!("../config.schema.json")` 编译期绑定。
- **写面**：`crates/core/workflow/src/write/create.rs:115` `create(... project_roots: &[String], ...)`（8 参，`#[allow(clippy::too_many_arguments)]`）；`:314` `bootstrap`（doc 写「按配置序」，空 `projects` → `bootstrap_worktree_root`，非空 → `for root in project_roots { bootstrap_project(...) }`）；`:355` `bootstrap_project`（映射表首匹配单安装、失败继续、文案三段）；`BOOTSTRAP_LOCKFILE_COMMANDS` 映射表与 `WARN_*` 常量均在本文件。
- **写面出口**：`crates/core/workflow/src/write/mod.rs:17` `pub use create::{create, CreateOutcome};`（`BootstrapProject` 需同批入列）。
- **promote**：`crates/core/workflow/src/write/promote.rs:31` `promote_explore(... project_roots: &[String], ...)` → `super::create(...)` 纯透传。
- **命令层**：`src-tauri/src/commands/changes/mod.rs:302-314`、`src-tauri/src/commands/explores/mod.rs:329-339` 各一处 `config::load(&main_root).config.projects.map(|p| p.root)` → `Vec<String>`；两文件均以 `use workflow::write::{self, …}` 取写面，故 `write::BootstrapProject` 直接可用。
- **生成物**：`src/types/generated/bindings.ts:2036` `ProjectConfig = { root: string }`、`bindings.zod.ts:1240` 同名 `strictObject`；`WorkspaceConfig.projects: ProjectConfig[]`。
- **前端**：`src/views/config/components/basic-config-section.tsx:62` `ProjectsGroup`（只渲染 `project.root`，`data-testid="config-projects"`，`index` 作 key，注释写明「重复 root 允许」）；同目录 `basic-config-section.test.tsx:54 / :97` 两处项目 fixture。
- **版本 / 配置 / 契约**：`packages/desktop/package.json:3` `0.6.59`；`.dev-team/config.json` 的 `projects` 为两条无新字段元素（合法，本变更不改）；`archi_module` 实测 `App.Backend.Core.Config` / `App.Backend.Core.Workflow` 均 `api.status = "none"`（`contractPath = null`，`models/Backend/` 下只有 `Foundation` 与 `ArchiRuntime` 两份契约），故 `api:check` 预期零 diff。

### 事实校准（proposal 措辞 vs 工作树实测；不改需求意图，只定实现形态）

1. **前端 fixture 编译闭合面是 4 处，不是 1 处**（proposal「测试文件」表只列 `basic-config-section.test.tsx`）。生成物把 `language` / `dependsOn` 出成**必填键**（`string | null` / `string[] | null`，同 `context` / `writeProtection` 既有口径），故凡是字面造 `ProjectConfig` 的 fixture 都需补两键才过 TS：`basic-config-section.test.tsx:54 / :97`、`app.test.tsx:131`、`config-view.test.tsx:37`（`projects: []` 三处不受影响）。这是**编译闭合**，不是新增用例。四个文件已逐条登记在「变更清单 - 测试文件（编译闭合）」（该子表是 tasks.md 实际改动面的双向映射落点）。
2. **`bindings/mod_test.rs:841-846` 的 `ProjectConfig` 断言不会失败**：它用 `type_section(...).contains("root: string,")` 正向断言，字段增加不破坏 `contains`；但注释「项目根单字段」语义过期。实现相位不动它（测试文件归下游相位），仅在交接清单注明该注释待测试相位随动。
3. **`create_test.rs` 的形参随动面 = 夹具定义 1 处 + 项目 roots 传参 8 处**（`:1445 / :1478 / :1506 / :1525 / :1547 / :1591 / :1620 / :1639`）；`create_full`（`:57`）与 `env.create`（`:52`）传 `&[]`，类型推断下零改动。**该文件已入「变更清单 - 测试文件（编译闭合）」子表**（夹具形参 + 私有 `proj` 助手 + 8 处 roots 声明改型，断言零改）。
4. **`promote_test.rs:85` 的 `&[]` 零改动**：`&[]` 对 `&[BootstrapProject]` 同样可推断（空切片），该文件无需触碰。
5. **`bootstrap` 的 doc 注释必须同批改**：`:310-313` 现写「配置了项目根 → 按配置序逐个项目根扫描」，与拓扑序语义直接冲突；`create.rs` 内其余文案（`bootstrap_project` 三段警告、`WARN_*` 常量）逐字不动。
6. **`install_order` 的写法要避开两个 clippy 陷阱**：外层用 `projects.iter().enumerate()`（`needless_range_loop`）、内层依赖查表用 `if let Some(&index) = map.get(...)`（不 `unwrap`）。

---

## 关键设计决策（DD1–DD12）

> 决策 D1–D13 见 proposal「决策表」（已定稿，本篇不重复）。下表是本设计在 spec 已钉语义之外**新增的形状裁决**——每条都含「为什么另一种写法会坏」。

| # | 议题 | 定稿 | 理由 |
|---|---|---|---|
| DD1 | `ProjectConfig` 两新字段的形态与注释 | `pub language: Option<String>`、`pub depends_on: Option<Vec<String>>`，追加在 `root` 之后；derive 面**零改**（`Debug, Clone, Serialize, Type` + `#[serde(rename_all = "camelCase")]`）；字段 doc 写明「开放字符串 / 零校验」「值宜为某个 `projects[].root`、零引用校验」 | `Option` 是「未设 vs 已设」的唯一可表达形态（空串 / 空数组不能区分「用户写了空」与「没写」）；不 derive `Deserialize`（`WorkspaceConfig` 系只出线不解析，沿既有纪律）。**不新增 `Default`**：spec 的缺省语义是字段级 `None`（由 `assemble` 产出），不是类型级默认 |
| DD2 | `assemble_project` 的插入位置与诊断顺序 | 在 `root` 判定成功之后、`Some(ProjectConfig { … })` 之前依次 `opt_string(map, "language", &format!("{project_path}.language"), diagnostics)` → `opt_string_list(map, "depends_on", &format!("{project_path}.depends_on"), diagnostics)`；两字段**不参与** `root` 的去留分支 | 放在 `root` 分支之后 = 元素已确定在案，两字段天然「不影响去留」（AC-2 的结构性保证，不靠一条 if）；复用两个既有助手 = 缺失 / null 零诊断、类型违例 `InvalidValue`、文案统一（零新写校验分支）。顺序固定为 language → depends_on，使 diagnostics 序列可预期 |
| DD3 | schema 镜像两条属性的 exact 形状 | `projects.items.properties` 在 `root` 之后追加：`language` = `{"type": "string", "description": "编码语言标注（自由文本，不做归一 / 校验；仅作声明，后续按语言分派时由消费侧归一）"}`；`depends_on` = `{"type": "array", "items": {"type": "string"}, "description": "依赖的项目根（值宜为某个 projects[].root；不做引用校验，安装时依赖先于依赖方）"}`；`required` 仍为 `["root"]`；两断言（A1 键集 / A2 枚举）与 A3 逐字不动 | 软规则路线下无机械锚可断（spec 明令 MUST NOT 新增第三条断言），同步纪律只能靠 description 自述边界 + 本节落点；`depends_on` 的 description 用「值宜为」提示语气（spec MUST NOT 写成强制约束）。**不加 `minLength` / `uniqueItems` / `$ref`**：镜像若比实现严，编辑器会拦下合法配置（比漏提示更坏） |
| DD4 | `BootstrapProject` 的形态与 derive 面 | `create.rs` 内 `pub struct BootstrapProject { pub root: String, pub depends_on: Vec<String> }`，`#[derive(Debug, Clone)]`；**不** derive serde / specta / Default，**不**新增构造器方法；`depends_on` 缺省值由调用方给 `Vec::new()`（命令层 `unwrap_or_default()`、测试夹具私有助手） | 它是**写面内部纯数据**（不进 IPC、不落盘、不出线），derive `Serialize/Type` 只会造出「它可以是 IPC DTO」的错误读法；`Vec` 而非 `Option<Vec>` 是 D6/D12 裁决（写面不区分「未设」与「空」）。不写 `new()`：唯一需要简短构造的是测试夹具，而测试内私有助手即可覆盖（避免为测试新增公共导出） |
| DD5 | `install_order` 的精确算法（含四类脏数据的产出序列） | 私有纯函数：`fn install_order(projects: &[BootstrapProject]) -> Vec<&str>`。①先建 `HashMap<&str, usize>`，同 root 只保留**首次**下标（`entry().or_insert(index)`）；②外层 `for (index, project) in projects.iter().enumerate()`，**跳过非规范下标**（`map[root] != index`）——这是重复 root 折叠的唯一落点；③`visit(index)`：`if visited[index] { return; }` → `visited[index] = true`（**先标后递归**）→ 逐个 `depends_on` 查表命中即递归、未命中（悬空）直接跳过 → `order.push(root)`（后序）；④返回根串序列 | 「先标后递归」是自引用与成环的**唯一**防死锁手段（标在递归后则 A→B→A 无限互递归）；外层跳过非规范下标是 D13「重复 root 安装恰一次」的唯一实现点（只靠 `visited` 不够：第二个同 root 元素下标未被访问过，仍会 push 出第二次）。$O(V+E)$，递归深度 = 项目数（配置量级几十，不上显式栈）。**不引入 SCC / Kahn**：spec 的 Module Contract 已钉「后序 DFS」，且拓扑序之外再加一层稳定排序只会让「同层 tie-break = 声明序」变成两个判定点 |
| DD6 | 四类脏数据与退化情形的**确定产出序列** | 声明序 `[A(dep→B), B]` → `[B, A]`；`[A, B, C]` 均无依赖 → `[A, B, C]`；`A(dep→ghost)` → `[A]`；`A(dep→A)` → `[A]`；声明序 `[A(dep→B), B(dep→A)]`（成环）→ `[B, A]`；`[A, B]` 同 root → `[A]`（首次为准，B 的 `depends_on` 一并丢弃） | 这些序列是 AC-4「产出确定序列」的可判定形态，测试与实现共用同一份表即不可能漂移。成环序列为 `[B, A]` 是后序 DFS 的必然结果（A 的依赖先落），spec scenario 标题「按声明序落定」按「遍历起点与 tie-break 只来自声明序、产出确定」读（见「待决问题」1） |
| DD7 | `bootstrap` 的改造边界 | 签名 `fn bootstrap(worktree: &Path, projects: &[BootstrapProject], vcs: &dyn WorktreePort, warnings: &mut Vec<String>)`；`projects.is_empty()` → `bootstrap_worktree_root`（legacy 分支逐字保留）；否则 `for root in install_order(projects) { bootstrap_project(worktree, root, vcs, warnings); }`；`bootstrap_project` **零改动** | 拓扑序只换遍历输入，`bootstrap_project` 的映射表探测 / 三段文案 / 「每项目至多一次安装」/「失败继续」全在原处 ⇒ 既有九例的断言结构与文案逐字不变（AC-4 后半）。空 `projects` 仍走 legacy 是 spec 明令（`install_order` 不介入） |
| DD8 | 命令层的映射落点 | 两处均改为：`let projects: Vec<BootstrapProject> = config::load(&main_root).config.projects.into_iter().map(|project| BootstrapProject { root: project.root, depends_on: project.depends_on.unwrap_or_default() }).collect();`，随后 `write::create(…, &projects, …)` / `write::promote_explore(…, &projects, …)`；`language` **不读取** | `BootstrapProject` 收 `Vec<String>`（D12）⇒ 映射必须在此一次性消除 `Option`；`language` 不消费是 spec「安装路径不消费 language」的实现落点（`grep language src/commands` 零命中即判据）。不在此处加 trim / 去重（协议与消费面均零智能） |
| DD9 | 配置页 `ProjectsGroup` 的 exact 呈现形态 | 容器 `data-testid="config-projects"` 与 `（空列表）` 空态不变；每个 `<li key={index} className="break-all">` 内三个同层 `<span>`：`{project.root}` → `language：{project.language ?? '未设'}` → `depends_on：{depsText(project.dependsOn)}`，后两者加 `ml-3 text-xs text-muted-foreground` 弱化；`function depsText(deps: string[] \| null): string`（`null → '未设'`、`length === 0 → '（空列表）'`、否则 `deps.join('、')`）；无编辑控件、无 onChange | 单文本节点「`language：typescript`」让呈现断言用整串查询即可（不必依赖 DOM 结构）；分隔符用全角「：」与既有中文 UI（「未设（默认 N）」「（空列表）」）同域。`未设` 是 spec 措辞（MUST 标注），与 `FieldRow` 的「未配置」（顶层字段缺失）语义不同，故不统一（见「待决问题」2）。**不隐藏已配置值**是 spec 硬要求，故 `null` 以外的任何值都直接呈现 |
| DD10 | 生成物与版本纪律 | `packages/desktop/package.json` `version` `0.6.59` → `0.6.60`；`src/types/generated/bindings.ts` / `bindings.zod.ts` 只经 `pnpm -C packages/desktop run bindings:export` 产出（`ProjectConfig` 出线为 `root: string; language: string \| null; dependsOn: string[] \| null`）；`src-tauri/tauri.conf.json` 经 `"version": "../package.json"` 跟随；`src-tauri/Cargo.toml` crate 版本行不跟 | 版本纪律是 AGENT.md 硬规；生成物手改会被下次生成静默覆盖且零 diff 门失败 |
| DD11 | 契约资产 / api 面预期 | `.dev-team/architecture/models/**`、`.dev-team/architecture/graph/**`、`crates/core/archi` 零改动；`App.Backend.Core.Config` / `App.Backend.Core.Workflow` 两元素 `api.status = none`（无契约 md），故 `api:check` 预期零 diff。若实现期 `api:check` 报 `apiFindings`，按十三值判据就位处置（不回退协议裁决） | 本变更不动 FQN 层级语义、不引入 `depends_on` 作层级源（D3）、不新增可导出符号面（`install_order` 私有、`BootstrapProject` 是 crate 内数据型） |
| DD12 | 自洽不变量的成立方式 | `None` 字段线面序列化为 `null`；`assemble` 对两字段位的 `null` 与键缺失同处置（`opt_string` / `opt_string_list` 的 `None \| Some(Value::Null)` 分支）⇒ 含两字段的合法 config 回写再组装零诊断，**无需为两字段加特例** | 自洽不变量是既有机械锚（AC-3），复用助手的 null 口径即天然满足；若两字段走自定义解析（如 trim 后空串转 None），回写形态就会与协议不同构，自洽立刻破 |

---

## 架构组件

### 1. 配置协议（`crates/core/config`）

- **职责**：把 `config.json` 的 `projects` 元素从单字段扩为「锚点 + 语言标注 + 依赖声明」三段，且两新字段只做类型判定、零内容智能。
- **文件位置**：`crates/core/config/src/lib.rs`（类型 + 组装）、`crates/core/config/config.schema.json`（编辑器镜像）。
- **依赖**：既有 `foundation`（路径）+ `serde` / `serde_json` / `specta`，**零新增**；复用 crate 内私有助手 `opt_string` / `opt_string_list`。
- **技术**：单遍组装（`assemble` → `assemble_project`）、诊断信封（`ConfigDiagnostic` + 点路径 `projects[i].language`）、`include_str!` 编译期镜像。

### 2. 依赖引导安装序（`crates/core/workflow`）

- **职责**：把项目模式的遍历序由声明序改为 `depends_on` 后序 DFS 拓扑序，四类脏数据零 `Err` 零诊断地收口；安装动作与文案面不变。
- **文件位置**：`crates/core/workflow/src/write/create.rs`（`BootstrapProject` / `install_order` / `bootstrap` / `create`）、`promote.rs`（透传）、`mod.rs`（出口）。
- **依赖**：现有 `vcs` port 缝与 `ChangeStateStore` port；**不新增 `config` 依赖边**（`BootstrapProject` 收纯数据的理由，D6）。
- **技术**：纯内存后序 DFS（`HashMap` 索引 + `Vec<bool>` visited）、同步无 IO、`Vec<&str>` 返回（零克隆）。

### 3. 命令层映射（`src-tauri/src/commands`）

- **职责**：把配置面的 `config::ProjectConfig` 投影为写面的 `BootstrapProject`，`language` 在安装路径不消费。
- **文件位置**：`src-tauri/src/commands/changes/mod.rs`、`src-tauri/src/commands/explores/mod.rs`。
- **依赖**：`config` crate（读配置）+ `workflow::write`（写面入参）。
- **技术**：`spawn_blocking` 内构造 `Vec<BootstrapProject>`（节奏与现有一致），闭包捕获后按引用传入。

### 4. 配置页呈现（`packages/desktop/src/views/config`）

- **职责**：`projects` 区逐项目呈现 `root` + 已设的 `language` / `depends_on`，保持只读。
- **文件位置**：`packages/desktop/src/views/config/components/basic-config-section.tsx`（`ProjectsGroup`）。
- **依赖**：生成物 `types/generated/bindings` 的 `ProjectConfig`（`language` / `dependsOn`）。
- **技术**：React 函数组件 + 纯函数 `depsText`，无状态、无 IO、无表单。

---

## 变更清单

> **覆盖口径**：三个子表合起来覆盖本 change 的全部改动文件——「修改文件」11 项 = proposal「变更范围 - 实现文件」11 行逐项对应（含两份生成物）；「测试文件（编译闭合）」4 项 = tasks.md 实际触碰的测试文件（proposal「测试文件」表漏列的 `app.test.tsx` / `config-view.test.tsx` / `create_test.rs` 在此补全）；「新增文件」空。实现期新增路径即越界，需回本表登记。

### 新增文件

<!-- 本变更零新增文件：两新函数落在既有 create.rs，两新字段落在既有类型，镜像与呈现面均为既有文件增列。 -->

### 修改文件

| 文件 | 具体修改内容 | 说明 |
|---|---|---|
| `packages/desktop/src-tauri/crates/core/config/src/lib.rs` | ①`ProjectConfig` 追加 `language: Option<String>` / `depends_on: Option<Vec<String>>` 两字段 + doc（DD1）；②`assemble_project` 在 `root` 判定成功后、构造 `Some(ProjectConfig { root, language, depends_on })` 前追加 `opt_string` / `opt_string_list` 两行（DD2）；③`WorkspaceConfig.projects` 的 doc 由「依赖引导项目根列表」改为「依赖引导项目锚点列表（root + 可选 language / depends_on）」 | 不新增校验代码；`KNOWN_KEYS`、两助手、`assemble` 单遍结构零改动 |
| `packages/desktop/src-tauri/crates/core/config/config.schema.json` | `projects.items.properties` 在 `root` 后追加 `language` / `depends_on` 两条（形状与 description 文案见 DD3）；`required` 保持 `["root"]`；其余字节零改 | 手工镜像同 commit 同步（spec「`config.schema.json` 静态镜像与实现一致性」） |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/create.rs` | ①新增 `pub struct BootstrapProject { pub root: String, pub depends_on: Vec<String> }`（DD4）；②新增私有 `fn install_order(projects: &[BootstrapProject]) -> Vec<&str>` 与私有递归 `fn visit_order(...)`（DD5，含 `use std::collections::HashMap`）；③`create` 第 4 参 `project_roots: &[String]` → `projects: &[BootstrapProject]`，末尾 `bootstrap(&worktree, projects, vcs, &mut warnings)`；④`bootstrap` 同形改造（DD7）+ doc 注释改「按 `depends_on` 拓扑序」；⑤`bootstrap_project` / `bootstrap_worktree_root` / 映射表 / `WARN_*` 常量**逐字不动** | 校验时序、补偿链、警告语义零变化 |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/promote.rs` | `promote_explore` 第 4 参 `project_roots: &[String]` → `projects: &[BootstrapProject]`，doc 与透传实参随动 | 纯透传，语义零变化 |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/mod.rs` | 出口面增 `pub use create::BootstrapProject;`（与 `pub use create::{create, CreateOutcome};` 相邻） | 命令层构造入参所需 |
| `packages/desktop/src-tauri/src/commands/changes/mod.rs` | `create_change_with` 内 `project_roots: Vec<String>` → `projects: Vec<BootstrapProject>`（DD8 映射），`write::create(...)` 实参由 `&project_roots` → `&projects`；注释由「依赖引导项目根经 config crate 读取」改为「项目锚点（root + depends_on）经 config crate 读取，language 在安装路径不消费」 | 命令签名 / 参数面零变化 |
| `packages/desktop/src-tauri/src/commands/explores/mod.rs` | 同上映射，`write::promote_explore(...)` 实参随动 | 同上 |
| `packages/desktop/src/types/generated/bindings.ts` | `ProjectConfig` 增 `language: string \| null` / `dependsOn: string[] \| null`（含 doc 注释出线）——**只经 `bindings:export` 产出** | 生成物，禁手改 |
| `packages/desktop/src/types/generated/bindings.zod.ts` | `ProjectConfigSchema` 增 `language`（`z.string().nullable()`）/ `dependsOn`（`z.array(z.string()).nullable()`）——**只经 `bindings:export` 产出** | 生成物，禁手改 |
| `packages/desktop/src/views/config/components/basic-config-section.tsx` | `ProjectsGroup` 逐项目呈现三要素（DD9）：`root` + `language：…` + `depends_on：…`，空态 / `（空列表）` / `未设` 三态；新增模块私有 `depsText`；文件头 `ProjectsGroup` doc 注释同步「语言与依赖并列呈现、未设标注」；`data-testid="config-projects"` 与只读形态保持 | 无编辑控件、无写回通道 |
| `packages/desktop/package.json` | `version` `0.6.59` → `0.6.60` | 版本纪律（D9）；`tauri.conf.json` 跟随、`Cargo.toml` 不跟 |

### 测试文件（编译闭合）

<!-- 本子表只登记「因入参形态 / 出线字段面变化而必须补齐形参与字面量」的既有测试文件，使工作区编译通过；MUST NOT 新增用例、MUST NOT 改断言与期望值——用例与门禁归 test-design / test-gen / test-execution 相位。 -->

| 文件 | 具体修改内容 | 说明 |
|---|---|---|
| `packages/desktop/src-tauri/crates/core/workflow/src/write/create_test.rs` | ①夹具 `create_with_projects` 第 3 参由 `project_roots: &[String]` 改为 `projects: &[BootstrapProject]`（`:87-97`，转手给 `create(...)`）；②测试模块内新增私有助手 `fn proj(root: &str) -> BootstrapProject`（`depends_on: Vec::new()`，不导出）；③8 处项目 roots 声明改型（`:1445` / `:1478` / `:1506` / `:1525` / `:1547` / `:1591` / `:1620` / `:1639`，即 `web` / `service` / `api` / `missing` / `plain` / `rs` / `.` 各处 `["x".to_owned()]` → `[proj("x")]`） | 仅形参面与构造面适配（R7）：既有九例的断言结构、安装序期望与警告文案逐字保留——无 `depends_on` 时安装序 = 声明序，故断言不变。`:1570` 的 `&[]` 与 `create_full`（`:57`）/ `env.create`（`:52`）的 `&[]` 零改动（空切片对 `&[BootstrapProject]` 类型推断成立） |
| `packages/desktop/src/views/config/components/basic-config-section.test.tsx` | 两处字面量 `ProjectConfig` fixture（`:54` 与 `:97`）补 `language: null, dependsOn: null` | 生成物把两新字段出成必填键（`string \| null` / `string[] \| null`，同 `context` / `writeProtection` 既有口径），不补即 TS 编译失败；断言与呈现期望零改（`projects: []` 处不受影响） |
| `packages/desktop/src/app.test.tsx` | `:131` 的 `projects: [{ root: 'packages/desktop' }]` 补两键 `language: null, dependsOn: null` | 同上，纯编译闭合；该文件其余 `root` 字面量属 `WorkspaceRecord` / `invoke` 载荷，与本变更无关 |
| `packages/desktop/src/views/config/config-view.test.tsx` | `:37` 的两条项目 fixture 各补 `language: null, dependsOn: null` | 同上，纯编译闭合 |

> **零改动项**：`packages/desktop/src-tauri/crates/core/workflow/src/write/promote_test.rs:85` 的 `&[]` 无需触碰（空切片类型推断成立）；`use-workspace-config.test.ts` 的 `projects: []` 与三处 `projects: []` 同样零改动。

### 公共函数 / API

| 标识符 | 所在文件 | 新增/修改 | 完整签名 | 说明 |
|---|---|---|---|---|
| `create` | `crates/core/workflow/src/write/create.rs` | 修改 | `pub fn create(main_root: &Path, store: &dyn ChangeStateStore, vcs: &dyn WorktreePort, projects: &[BootstrapProject], name: &str, goal: &str, title: &str, workflow_type: &str) -> Result<CreateOutcome, String>` | 第 4 参类型由 `&[String]` 改为 `&[BootstrapProject]`；九道前置校验、四段执行、三处补偿、警告清单语义逐字不变 |
| `promote_explore` | `crates/core/workflow/src/write/promote.rs` | 修改 | `pub fn promote_explore(main_root: &Path, store: &dyn ChangeStateStore, vcs: &dyn WorktreePort, projects: &[BootstrapProject], name: &str, note: &str, title: &str, workflow_type: &str) -> Result<PromoteOutcome, String>` | 第 4 参形态随动、纯透传给 `create`；搬运失败文案零变化 |
| `write::BootstrapProject`（出口再导出） | `crates/core/workflow/src/write/mod.rs` | 新增 | `pub use create::BootstrapProject;` | 命令层构造入参的唯一取用路径 |

> 私有函数不入本表（非公共 API）：`install_order` 的签名 `fn install_order(projects: &[BootstrapProject]) -> Vec<&str>` 与算法见 DD5 与「修改文件」表 `create.rs` 行。

### 类型定义

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|---|---|---|---|
| `ProjectConfig` | `crates/core/config/src/lib.rs` | 修改 | 元素形状由 `{ root: String }` 扩为 `{ root: String, language: Option<String>, depends_on: Option<Vec<String>> }`；`Option` 出线为 `null`（键恒在），camelCase → `dependsOn`；derive 面零改 |
| `BootstrapProject` | `crates/core/workflow/src/write/create.rs` | 新增 | `pub struct BootstrapProject { pub root: String, pub depends_on: Vec<String> }`，`#[derive(Debug, Clone)]`；写面内部纯数据（无 serde / specta / Default，DD4） |
| `ProjectConfig`（TS 生成物） | `packages/desktop/src/types/generated/bindings.ts` / `bindings.zod.ts` | 修改 | `{ root: string; language: string \| null; dependsOn: string[] \| null }`；`strictObject` 同步（生成物，禁手改） |

### 配置

| 配置键 | 所在文件 | 新增/修改 | 值类型 | 默认值 | 说明 |
|---|---|---|---|---|---|
| `projects[].language` | `crates/core/config/config.schema.json`（镜像） / `crates/core/config/src/lib.rs`（实现） | 新增 | `string`（开放文本，零白名单 / 零归一 / 不 trim） | 未设 = `null`（`None`）且零诊断 | 编码语言标注；声明型字段（本期无运行时消费者，计划消费者见 spec「字段消费点表」） |
| `projects[].depends_on` | 同上 | 新增 | `string[]`（全字符串元素；保序、不去重、零引用校验） | 未设 = `null`（`None`）且零诊断 | 平级依赖边（不是层级来源）；唯一消费者 = bootstrap 拓扑序安装 |

<!-- 路由 / API 设计：本变更零 IPC 命令面改动（`create_change` / `promote_explore` 命令名与参数面不变，仅内部映射随动），无新增端点，故整段省略。 -->

---

## 数据模型

### `ProjectConfig`（`config.json` 的 `projects[]` 元素）

| 字段 | 类型 | 必填 | 处置（缺失 / null / 合法 / 违例） | 约束 |
|---|---|---|---|---|
| `root` | `String` | 是 | 缺失或非法 → 整条剔除 + `InvalidValue`（`projects[i]` / `projects[i].root`） | 非空、非绝对、无 `..` 段 / `:` / glob 通配符（现状不动） |
| `language` | `Option<String>` | 否 | 缺失 / null → `None` 零诊断；字符串 → `Some` **原样**；非字符串 → `None` + `InvalidValue`（`projects[i].language`） | 零内容校验；不影响元素去留 |
| `depends_on` | `Option<Vec<String>>` | 否 | 缺失 / null → `None` 零诊断；全字符串数组 → `Some`（保序、不去重）；非数组 / 混入非字符串 → `None` + `InvalidValue`（`projects[i].depends_on`） | 零引用校验；脏数据免疫责任在消费侧；不影响元素去留 |

**关系**：`depends_on` 是 `projects[]` 之间的**平级有向边**（依赖方 → 依赖方），语义仅为「安装顺序」与（将来的）影响面传播方向；**不是**架构模块的包含 / 父子关系（层级唯一真相源仍是 FQN 最长前缀，D3）。

**派生态（不落盘）**：`install_order(projects)` 由 `projects[]` + 各自 `depends_on` 派生「安装序」（`Vec<&str>`，root 串序列）。派生规则：同 root 折叠为首次出现的元素 → 后序 DFS（依赖先于依赖方）→ 同层 tie-break = 声明序；悬空依赖跳过、自引用 / 成环由 visited 收口。四种脏数据一律零 `Err`、零诊断、产出确定序列（DD6 已列全）。

**持久化**：`projects` 只来自工作区根 `config.json` 读取（路径经 `foundation::layout` 单点），**零 DB 落库、零迁移、零写回通道**；`config.json` 仍在内置写保护清单内。派生安装序是纯内存中间值，不落盘。

---

## 依赖

| 类别 | 依赖 | 用途 |
|---|---|---|
| 运行时 | 无新增 | `install_order` 只用 `std::collections::HashMap` + `Vec`；config / 命令 / 前端均复用现有依赖 |
| 构建 / 测试 | 无新增 | 沿用 `cargo` / `pnpm` / `bindings:export` 既有链；不新增 crate、不新增 npm 包 |

---

## 待决问题

1. **成环安装序的 spec 措辞读法**：`desktop-change-worktree` scenario「成环依赖按声明序落定」与 Module Contract 已钉的「后序 DFS」在字面上会给出两种读法——本设计按后序 DFS 定产 `[B, A]`（声明序 `[A(dep→B), B(dep→A)]`），并把「按声明序落定」读作「遍历起点与 tie-break 只来自声明序、产出确定」。若评审要求字面「安装序 = 声明序」，需回调 spec 措辞或改算法（Kahn + 破环取声明序首项），本设计倾向前者（改措辞、不改实现）。测试相位应以上表 DD6 的序列为准。
2. **「未设」与既有「未配置」两套弱化文案并存**：本设计按 spec 措辞对 `language` / `depends_on` 取「未设」，不动 `FieldRow` 顶层的「未配置」。若评审要求全页统一，属呈现层文案调整，不涉协议与数据面。
3. **`.dev-team/config.json` 是否在本变更内补 `language`**：proposal 明令 MUST NOT 为演示强改；本设计取**不改**（两条现有元素无新字段仍合法）。若落地期要演示呈现面，另按值级替换补 `"language": "typescript"` 等，且属可选动作。

---

## 验收标准落点

| AC | 落点 |
|---|---|
| AC-1 两字段可空 / 缺失零诊断 / 合法原样 | DD1 + DD2（复用 `opt_string` / `opt_string_list` 的「缺失 / null 零诊断、字符串原样」分支；不 trim、不去重、悬空项在案） |
| AC-2 类型违例逐字段落 `None` + `InvalidValue`、元素仍在案 | DD2（两字段调用位于 `root` 判定成功之后，结构性保证「不影响去留」；path 由 `format!("{project_path}.language")` 给出） |
| AC-3 自洽不变量 | DD12（`None` → `null` 回写，组装层 null 与缺键同处置 ⇒ 二次组装零诊断，无特例） |
| AC-4 拓扑序 / 退化 / 四类脏数据零 `Err` | DD5（算法）+ DD6（确定序列表）+ DD7（`bootstrap_project` 零改 ⇒ 既有九例断言语义不变） |
| AC-5 写面收 `BootstrapProject`、无 `config` 依赖边、两命令层映射到位 | DD4（纯数据、无 serde/specta）+ DD8（映射含 `language` 不消费）+ `crates/core/workflow/Cargo.toml` 零改动 |
| AC-6 镜像两属性在案且两断言零改动 | DD3（exact 形状 + `required` 仍只 `root`；A1/A2/A3 逐字不动） |
| AC-7 配置页逐项目呈现两字段、未设标注、仍只读 | DD9（三要素形态 + `depsText` 三态 + 无编辑控件） |
| AC-8 全管线绿 / 生成物零手改 / 版本口径 | DD10（`0.6.60` + 生成物只经 `bindings:export`）+ DD11（契约资产零改动、`api:check` 预期零 diff）；`check` 链与测试回归由桌面端相位执行 |

> **验收面口径**：与 proposal 一致——止于单元测试 + 静态检查链。`language` 在切片 1 内无运行时消费者，MUST NOT 以「按语言跑通某条流程」作为验收条。
