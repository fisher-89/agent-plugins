# 测试设计: project-config-protocol-extension

> **变更**: project-config-protocol-extension
> **来源**: `design.md`（DD1–DD12 / 变更清单三子表 / 公共函数与 API / 类型定义 / 数据模型）、`proposal.md`（AC-1…AC-8）
> **性质**: 加字段 + 换遍历序 + 呈现面增列。被测范围只取 design「新增文件」（空）与「修改文件」两子表（11 项），删除文件零涉及。

## 测试框架与布局

| 域 | 框架 / 断言 | 测试文件形态 | 运行入口 |
|---|---|---|---|
| Rust（`src-tauri/crates/core/**`） | 标准 `#[test]` + `assert!` / `assert_eq!`；无外部断言库 | 同目录兄弟文件 `<模块>_test.rs`，经 `#[cfg(test)] mod <模块>_test;` 挂载（`crates/core/config/src/lib.rs:963`、`crates/core/workflow/src/write/mod.rs:53-74`） | `cargo test`（桌面端相位执行，本文档不写命令细节） |
| Rust（`src-tauri/src/**` shell 层） | 同上 + `tauri::test::MockRuntime` | 同目录 `<模块>_test.rs`（`src/commands/changes/mod_test.rs`、`src/commands/explores/mod_test.rs`、`src/bindings/mod_test.rs`） | 同上 |
| 前端（`packages/desktop/src/**`） | `vite-plus/test`（`describe` / `it` / `expect`）+ `@testing-library/react`，jsdom 环境（文件首行 `// @vitest-environment jsdom`） | 与被测组件同目录的 `<组件>.test.tsx` | `pnpm -C packages/desktop run test` |

扩展名与断言风格沿用既有文件：Rust 侧测试名用中文短语、断言用 `assert!` / `assert_eq!` 直比产出（诊断集合、安装序、线面值）；前端侧用 `screen.getByText` / `getByTestId` + `toEqual` 整数组比对。本变更零新增测试文件（`create_test.rs` 内新增私有助手 `proj()`，不导出）。

## 验收范围

| AC ID | 验收条件 | 被测文件或模块 |
|---|---|---|
| AC-1 | `ProjectConfig` 携 `language` / `depends_on` 两可空字段；缺失 / 显式 null → `None` 且 diagnostics 零新增；合法值原样保留（`language` 不 trim / 不归一；`depends_on` 保序、不去重、悬空项在案） | `packages/desktop/src-tauri/crates/core/config/src/lib.rs`、`packages/desktop/src/types/generated/bindings.ts` |
| AC-2 | 类型违例逐字段处置：`language` 非字符串 / `depends_on` 非数组或混入非字符串 → 字段落 `None` + `InvalidValue`（path `projects[i].language` / `projects[i].depends_on`），元素本身仍在案；同元素其余合法字段不受牵连 | `packages/desktop/src-tauri/crates/core/config/src/lib.rs` |
| AC-3 | 自洽不变量不破：含两字段的合法 config 重序列化回 `Value` 再经同一组装，诊断恒空 | `packages/desktop/src-tauri/crates/core/config/src/lib.rs` |
| AC-4 | 安装序为拓扑序（依赖先于依赖方；无依赖 / 同层保持声明序）；悬空 / 自引用 / 成环 / 重复 `root` 四类脏数据零 `Err`、零新增警告、产出确定序列；既有九例断言不变 | `packages/desktop/src-tauri/crates/core/workflow/src/write/create.rs`、`packages/desktop/src-tauri/crates/core/workflow/src/write/promote.rs` |
| AC-5 | 写面入参为 `BootstrapProject`，`core/workflow` 依赖零新增（无 `config` 边）；两命令层映射到位（`language` 在安装路径不消费） | `packages/desktop/src-tauri/crates/core/workflow/src/write/create.rs`、`packages/desktop/src-tauri/crates/core/workflow/src/write/mod.rs`、`packages/desktop/src-tauri/src/commands/changes/mod.rs`、`packages/desktop/src-tauri/src/commands/explores/mod.rs` |
| AC-6 | schema 镜像 `projects.items.properties` 含 `root` / `language` / `depends_on` 三项且 description 为提示文案；两条一致性断言（八项键集 / 八值枚举）零改动通过 | `packages/desktop/src-tauri/crates/core/config/config.schema.json` |
| AC-7 | 配置页 `projects` 区逐项目呈现已设的 `language` / `depends_on`（未设标注「未设」），页面仍只读（无编辑 / 保存入口） | `packages/desktop/src/views/config/components/basic-config-section.tsx` |
| AC-8 | 全管线绿（`check` 链 + 全量测试）；绑定生成物零手改；`package.json` version `0.6.60`、`src-tauri/Cargo.toml` 版本不随动 | `packages/desktop/src/types/generated/bindings.ts`、`packages/desktop/src/types/generated/bindings.zod.ts`、`packages/desktop/package.json`（机械门禁，见「不可测试项」） |

> **对位说明**：上表只列 AC 与承载模块；每个模块的测试文件路径、覆盖方式与 mock 策略见下方对应 `###` 节（节标题即「源文件 -> 测试文件」映射对）。
>
> **编译闭合面（非新增用例）**：`create_test.rs` 的夹具形参与 8 处 roots 声明、`promote_test.rs:85` 的 `&[]`、`app.test.tsx:131` / `config-view.test.tsx:37` 的项目 fixture 补两键，均为形参 / 字面量适配（design「测试文件（编译闭合）」子表），断言与期望值零改。

## 单元测试

### packages/desktop/src-tauri/crates/core/config/src/lib.rs -> packages/desktop/src-tauri/crates/core/config/src/lib_test.rs

#### 待测功能

<!-- design.md 的「公共函数 / API」子表无本文件行；下列条目取自同篇「类型定义」（`ProjectConfig` 修改）与「配置」（两新键）子表，测试面为公共 `load()` 的产出——`assemble_project` / `opt_string` / `opt_string_list` 均为私有，不直连测试、不为此新增导出。 -->

- `ProjectConfig.language`: 新增 `Option<String>` 字段——缺失 / 显式 null → `None` 零诊断；字符串原样保留（不 trim / 不归一 / 零内容校验）；非字符串 → `None` + `InvalidValue`（path `projects[i].language`）；不参与元素去留
- `ProjectConfig.depends_on`: 新增 `Option<Vec<String>>` 字段——缺失 / 显式 null → `None` 零诊断；全字符串数组 → `Some`（保序、不去重、零引用校验）；非数组或混入非字符串 → `None` + `InvalidValue`（path `projects[i].depends_on`）；不参与元素去留
- `load()`: 经单遍组装消费两字段——诊断追加序固定为 `language` 先于 `depends_on`（DD2）、线面 `None` 出 `null` 且 camelCase 出 `dependsOn`、自洽不变量（回写再组装零诊断）不破

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ProjectConfig.language` | 正向 | 键缺失与显式 `null` 两形态 → `language == None` 且 `diagnostics` 零新增（与既有「projects 静默空列表」口径一致） | 矩阵迭代（2 形态） |
| `ProjectConfig.language` | 正向 | 合法字符串原样保留：`"typescript"` / `"  rust  "`（不 trim）/ `"TypeScript"`（不归一）/ `"C++"` / `"🙂"` / 1200 字符长串 → `Some` 与原文逐字相等 | 矩阵迭代（6 形态） |
| `ProjectConfig.language` | 边界 | 空串 `""` → `Some("")` 且零诊断（零内容校验，空串不得退化为 `None`） | 单点 |
| `ProjectConfig.language` | 异常 | 非字符串（数字 / 布尔 / 对象 / 数组）→ `language == None` + `InvalidValue` path `projects[0].language`，元素仍在 `config.projects` 中 | 矩阵迭代（4 形态） |
| `ProjectConfig.depends_on` | 正向 | 键缺失与显式 `null` 两形态 → `depends_on == None` 且零诊断 | 矩阵迭代（2 形态） |
| `ProjectConfig.depends_on` | 正向 | `["b", "a", "b", "ghost"]` → `Some` 逐项相等：保序、不去重、悬空项在案 | 单点 |
| `ProjectConfig.depends_on` | 边界 | `[]` → `Some(vec![])`（与 `None` 可区分）；`["single"]` 单元素；1000 元素大列表 → 原样保序 | 矩阵迭代（3 形态） |
| `ProjectConfig.depends_on` | 边界 | 元素为空串 / 含前后空格 / emoji / 超长字符串 → 原样保留（零内容校验，空串项不得触发诊断） | 矩阵迭代（4 形态） |
| `ProjectConfig.depends_on` | 异常 | 混入非字符串元素（数字 / `null` / 布尔 / 嵌套数组）→ 整字段 `None` + `InvalidValue` path `projects[0].depends_on`，元素仍在案 | 矩阵迭代（4 形态） |
| `ProjectConfig.depends_on` | 异常 | 非数组（对象 / 字符串 / 数字 / 布尔）→ `None` + `InvalidValue`（消息含原值类型），元素仍在案 | 矩阵迭代（4 形态） |
| `load()` | 异常 | 同一元素两字段同时违例 → 元素仍在案（仅 `root` 决定去留）、两条 `InvalidValue` 且 `language` 诊断序号在 `depends_on` 之前（DD2 顺序钉点） | 单点 |
| `load()` | 边界 | `root` 缺失或非法 + 两字段均合法 → 整条剔除，且诊断只有 `projects[0].root` 一条（两字段不参与去留、不追加诊断） | 矩阵迭代（2 形态） |
| `load()` | 边界 | 三元素混合（合法 / `language` 违例 / `depends_on` 违例）→ 三条全部保留、诊断 path 下标对位、处置互不殃及 | 单点 |
| `load()` | 边界 | 合法两字段 + 未知子键（如 `"custom": 1`）→ 零诊断（未知子键静默忽略语义不变；两新字段不断言 `KNOWN_KEYS`） | 单点 |
| `load()` | 正向 | 自洽不变量：含两字段（`None` / `Some("")` / `Some(vec![])` / 多元素混合）的合法 config 经 `serde_json::to_value` 回写再过 `assemble` → 诊断恒空（既有 `assert_self_consistent` 矩阵追加 fixture） | 矩阵迭代（4 fixture） |
| `load()` | 正向 | 线面形态：`None` 序列化为 `null` 且键恒在、`depends_on` 出线名为 `dependsOn`、`language` 原样（`value_of` 断言） | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| —（无 mock） | 沿用既有 `TempWs`：真实 tempdir + 真实 `config.json` 落盘读取，文件边界即真实进程边界 | 全部用例（AC-1 / AC-2 / AC-3 均经 `load()` 端到端观察） |
| 诊断 / 配置值对照 | 不使用替身：`has_diag()` / `value_of()` 为既有测试内私有助手，直读真实产出 | 诊断 path、顺序与线面形态断言 |

### packages/desktop/src-tauri/crates/core/config/config.schema.json -> packages/desktop/src-tauri/crates/core/config/src/lib_test.rs

#### 待测功能

<!-- design.md 的「公共函数 / API」子表无本文件行（它是镜像资产，非代码模块）；下列条目取自「配置」子表两新键与 DD3 / AC-6，测试面为 `include_str!("../config.schema.json")` 的编译期绑定值。 -->

- `config.schema.json`（`projects.items.properties`）: 在 `root` 之后增 `language`（`type: string`）与 `depends_on`（`type: array` / `items.type: string`）两属性，`required` 仍只 `["root"]`；两属性只带 `type` + `description`，MUST NOT 引入 `minLength` / `uniqueItems` / `enum` 等严于实现的约束
- 既有三条一致性断言（A1 `KNOWN_KEYS` 八键集 / A2 `TestFramework` 八值枚举 / A3 往返）: 逐字不动仍通过，且不新增机械锚

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `projects.items.properties` | 正向 | 键集排序后恰为 `["depends_on", "language", "root"]`；`required` 等于 `["root"]`（两新键不入 `required`） | 单点 |
| `projects.items.properties.language` | 正向 | `type == "string"` 且不含 `minLength` / `maxLength` / `enum` / `pattern` / `default` 等约束键（镜像不得比实现严，DD3） | 单点 |
| `projects.items.properties.depends_on` | 正向 | `type == "array"`、`items.type == "string"`、不含 `uniqueItems` / `minItems` / `maxItems` / `default`；`description` 为非空且含「值宜为」提示语（非强制约束措辞） | 单点 |
| 三条既有断言 | 正向 | A1 八键集 / A2 八值枚举 / A3 往返照旧通过（`assert_key_set` / `assert_framework_enum` / 往返用例零改动），确认本变更不新增第三条机械锚 | 矩阵迭代（3 断言） |
| `projects.items.properties` | 异常 | 内存副本分别删除 `language`、删除 `depends_on`、把两新键写进 `required` → 新断言失败且失败文案指认具体缺失 / 越界项 | 矩阵迭代（3 副本） |
| `projects.items.properties` | 异常 | 内存副本把 `language` 写成对象、把 `depends_on.items` 取走 → 断言给出可指认文案，不越界 `unwrap` | 矩阵迭代（2 副本） |
| `projects.items.properties` | 边界 | 属性顺序打乱（`depends_on` 前置）→ 排序后比较仍通过（顺序无关）；`language` 为 `null` → 归类为类型违例文案 | 矩阵迭代（2 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| —（无 mock） | `include_str!` 编译期绑定入仓文件，零运行时 IO；异常 / 边界形态用既有 `mirror_copy()` 在内存 `Value` 副本上就地改写，不使用 mock 框架 | 全部用例（真实入仓镜像正向 + 内存副本反向） |

### packages/desktop/src-tauri/crates/core/workflow/src/write/create.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/create_test.rs

#### 待测功能

- `create()`: 第 4 参由 `project_roots: &[String]` 改为 `projects: &[BootstrapProject]`，安装序改由 `install_order` 的后序 DFS 拓扑序产出（依赖先于依赖方；无依赖 / 同层保持配置声明序）；九道前置校验、补偿链、警告文案、每项目单安装、失败继续语义逐字不变
- `BootstrapProject`: 新增 `pub struct { root: String, depends_on: Vec<String> }`，写面不区分「未设」与「空」（`depends_on` 缺省 `Vec::new()`）

<!-- `install_order` 为私有纯函数（`fn`，非 `pub`），兄弟测试模块不可达——其产出序列经 `create()` → `bootstrap` → 假件 vcs 的 `installs()` 顺序观察；期望序取自 DD6 序列表，不为测试新增 `pub` 导出。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `create()` | 正向 | 拓扑序：声明序 `[src-tauri(dep→desktop), desktop]`（两项目均有 lockfile）→ `installs()` 序为 `desktop` → `src-tauri`，cwd 各为 `<worktree>/<root>`，零警告 | 单点 |
| `create()` | 正向 | 无依赖退化：三项目 `depends_on: vec![]` → 安装序逐字等于声明序（D4 tie-break = 声明序，非字典序） | 单点 |
| `create()` | 正向 | 多层链：`A(dep→B)` / `B(dep→C)` / `C` → 安装序 `C, B, A`（后序 DFS 逐层下沉） | 单点 |
| `BootstrapProject` | 正向 | 夹具私有助手 `proj(root)` 构造 `depends_on: Vec::new()` 入参，与 `create()` 第 4 参形态吻合并通过（空 `Vec` 与显式空数组行为一致，写面不区分「未设 / 空」） | 单点 |
| `create()` | 边界 | 重复 `root`：`[proj("web"), {root:"web", depends_on:["api"]}, proj("api")]` → `web` 恰安装一次（首次出现为准，第二条元素的 `depends_on` 一并丢弃），安装序 `web, api` | 单点 |
| `create()` | 边界 | 依赖项重复 / 空串：`A(dep→["B","B",""])` → visited 幂等收口，`B` 恰一次、空串按悬空跳过，零 `Err` | 单点 |
| `create()` | 边界 | 超大输入：200 个无依赖项目 + 20 层依赖链 → 线性完成、序确定、无栈溢出 / 死循环、安装次数 = 项目数 | 单点 |
| `create()` | 边界 | `projects: &[]` → legacy worktree 根扫描分支（既有 `空projects回退worktree根扫描` 断言逐字不变） | 单点 |
| `create()` | 边界 | 点号根 `"."` + `depends_on` 指向另一根 → `.` 解析为 worktree 根，安装序仍按拓扑序（保持既有「点号根」语义） | 单点 |
| `create()` | 异常 | 悬空依赖：`A(dep→ghost)` + `B` → 零 `Err`，两项照常安装，警告集合与无 `depends_on` 时同形（无「未知依赖」类新增警告） | 单点 |
| `create()` | 异常 | 自引用：`A(dep→A)` → 零 `Err`、`A` 恰安装一次、无死循环 | 单点 |
| `create()` | 异常 | 成环：声明序 `[A(dep→B), B(dep→A)]` → 零 `Err`、各安装恰一次、产出序 `[B, A]`（DD6 确定序） | 单点 |
| `create()` | 异常 | 拓扑序 + 安装失败：`A(dep→B)` 两项均失败 → 警告按**安装序**累积（`B` 的失败警告先于 `A`）、仍返回 `Ok`、建档与 worktree 均保留 | 单点 |
| `create()` | 异常 | 拓扑序 + 项目根不存在：`A(dep→B)`、`B` 根缺失 → 警告按安装序出现两条「在 worktree 内不存在」，零安装 | 单点 |
| `create()` | 正向 | 既有项目模式九例回归（多项目 / 首匹配单安装 / 缺失根 / 无 lockfile 注记 / 抑制根扫描 / 空 projects / 失败继续 / 脏仓警告先于项目警告 / 点号根）：安装序期望与警告文案逐字不变 | 矩阵迭代（9 例） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `vcs: &dyn WorktreePort` | 沿用既有 `FakeVcs`（进程替身：`probe` / `add_worktree` / `run_install` 全捕获，`installs()` 返回 `(cwd, command)` 有序列表）；子进程安装不真实执行 | 全部拓扑序 / 脏数据用例（安装序即唯一断言面） |
| `store: &dyn ChangeStateStore` | 沿用既有 `CreateStore` 内存假件（作为被测 API 显式入参注入） | 建档保留 / 回滚断言 |
| 文件系统 | 真实 tempdir 双根（主仓根 + worktree 落位父锚），`seed_main_path` 落真实 lockfile——不 mock fs | 项目根探测（`is_dir` / lockfile `exists`） |

### packages/desktop/src-tauri/crates/core/workflow/src/write/promote.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/promote_test.rs

#### 待测功能

- `promote_explore()`: 第 4 参形态随 `create` 改为 `projects: &[BootstrapProject]`，纯透传给 `create`；搬运失败文案、笔记删除半边语义零变化

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `promote_explore()` | 正向 | 空项目实参 `&[]`（对 `&[BootstrapProject]` 类型推断成立）→ legacy 路径、笔记移走、`PromoteOutcome` 双字段照常（既有断言零改） | 单点 |
| `promote_explore()` | 边界 | 项目模式 + 拓扑序透传：`[A(dep→B), B]` → change 内安装序 `B, A`，笔记同时移走（`create` 侧顺序能力经 promote 路径同样生效） | 单点 |
| `promote_explore()` | 异常 | 空白 `name` / 非 kebab `name` → 显式 `Err` 且零安装、零笔记删除（前置校验先于 bootstrap，既有断言不变） | 矩阵迭代（2 形态） |
| `promote_explore()` | 异常 | `create` 侧补偿失败路径（既有 `add` 失败用例）→ `Err` 文案不变，项目入参形态不影响补偿链 | 单点 |

> **跨模块组合用例挂靠**：`promote_explore` → `create` 是本变更唯一的跨模块运行链路（`promote_explore` 为发起方 / 最上层调用方），故「拓扑序透传」一例挂靠本文件，不另设集成测试区。`config::ProjectConfig → BootstrapProject` 的投影链路无运行时可测行为，见「不可测试项」与 `commands/*` 两节。

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `vcs` / `store` | 沿用既有 `FakeVcs` / `PromoteStore` 假件（被测 API 显式入参） | 安装序与建档断言 |
| 文件系统 | 真实 tempdir（主仓笔记 + worktree 树真实落盘），不 mock | 笔记移走 / 删除失败面 |

### packages/desktop/src-tauri/crates/core/workflow/src/write/mod.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/create_test.rs

#### 待测功能

- `write::BootstrapProject`（出口再导出）: 命令层构造入参的唯一取用路径——`pub use create::BootstrapProject;` 与 `pub use create::{create, CreateOutcome};` 相邻

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `write::BootstrapProject` | 正向 | 测试模块经 `super::BootstrapProject`（即 `write::` 出口，不经 `create::` 私有路径）取型构造入参并传入 `create()` → 编译通过且安装序生效（再导出可见性锚点） | 单点 |

<!-- 纯再导出无运行时分支：异常 / 边界路径不适用（可见性失效即编译失败，属编译期强制而非用例断言）；不为此新增构造器方法或 `Default`（DD4）。 -->

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| —（无 mock） | 复用 `create_test.rs` 既有装置（`FakeVcs` / `CreateStore` / 真实 tempdir） | 唯一用例 |

### packages/desktop/src-tauri/src/commands/changes/mod.rs -> packages/desktop/src-tauri/src/commands/changes/mod_test.rs

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更：`create_change` / `create_change_with` 的命令签名与参数面零变化，改动仅为 `spawn_blocking` 内的 `config::ProjectConfig → BootstrapProject` 投影（`root` 原样 + `depends_on.unwrap_or_default()`，`language` 不消费，DD8）。本文件零新增用例；下列条目 = 映射后行为面 + 实读 `mod_test.rs` 登记的既有用例回归。 -->

- `create_change_with()`: `config::ProjectConfig → BootstrapProject` 投影后透传 `write::create`——投影为纯字段搬运，正确性由编译期类型强制（`&[BootstrapProject]` 形参）+ 源码复核（`grep language src/commands` 零命中）承担；运行面由既有用例「未配置 `config.json` → 空 `projects` → 写面 legacy worktree 根扫描」回归
- `create接线_建域落位_返回dto增id且按id立即可见可发起()`: 既有用例回归——映射后空 `projects` 走 legacy 分支，建档 / 落位 / dto / 立即可达断言逐字不变
- `create_title透传_显式值落库且默认name形态()`: 既有用例回归——`title` 透传与默认 `name` 形态不受第 4 参形态变化影响
- `create_change四参签面_workflow_type扩ipc入参()`: 既有用例回归——命令签名面断言（`create_change` 四参 / `create_change_with` 六参）零变化
- `create_workflow_type透传test_only且dto五键不变()`: 既有用例回归——`workflow_type` 透传与 `CreateOutcome` dto 五键形态不变
- `create非法workflow_type拒绝_零建档零建域()`: 既有用例回归——写面前置校验仍先于任何安装动作
- `create错误映射_同名active冲突err且零目录零建档()`: 既有用例回归——`Err` 逐字透传且零副作用
- `create拒绝映射_非git仓err透传且零产生()`: 既有用例回归——拒绝路径不经安装序
- `blank_root双口径_读命令早退空结果_create显式err()`: 既有用例回归——blank root 早退发生在映射之前，不触 `config::load`
- `create_change组合_4条active第5个创建成功()`: 既有用例回归——命令面 → 写面前置⑧ 上限道的组合链路放行面
- `create_change组合_第6个请求上限err逐字透传()`: 既有用例回归——上限 `Err` 串零包装透传

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `create_change_with()` | 正向 | 未配置 `config.json` 的 workspace → `config::ProjectConfig` 投影产出空 `Vec<BootstrapProject>` → `write::create` legacy 分支；投影不引入额外诊断 / 警告（映射无返回值可断言，故经建档结果与警告清单间接观察） | 单点 |
| `create接线_建域落位_返回dto增id且按id立即可见可发起` | 正向 | 既有断言逐字回归（映射后空 `projects` 路径） | 单点 |
| `create_title透传_显式值落库且默认name形态` | 正向 | 既有断言逐字回归 | 单点 |
| `create_change四参签面_workflow_type扩ipc入参` | 边界 | 命令签名面（四参 IPC / 六参泛型缝）零变化——映射只在 `spawn_blocking` 闭包内，不出线到 IPC 参数 | 单点 |
| `create_workflow_type透传test_only且dto五键不变` | 正向 | 既有断言逐字回归（`CreateOutcome` 形态不受第 4 参类型影响） | 单点 |
| `create非法workflow_type拒绝_零建档零建域` | 异常 | 既有断言逐字回归（前置校验时序先于 `bootstrap`） | 单点 |
| `create错误映射_同名active冲突err且零目录零建档` | 异常 | 既有断言逐字回归（`Err` 文案零改写） | 单点 |
| `create拒绝映射_非git仓err透传且零产生` | 异常 | 既有断言逐字回归（拒绝路径零安装） | 单点 |
| `blank_root双口径_读命令早退空结果_create显式err` | 边界 | 既有断言逐字回归（blank root 在映射前显式 `Err`） | 单点 |
| `create_change组合_4条active第5个创建成功` | 正向 | 既有断言逐字回归（命令面 → 写面组合链路） | 单点 |
| `create_change组合_第6个请求上限err逐字透传` | 异常 | 既有断言逐字回归（上限 `Err` 零包装） | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| —（无 mock） | 既有装置为真实 tempdir + 真实 `Store` + `tauri::test::MockRuntime` + 真实 git 仓夹具，本次零改动；`config.json` 由 tempdir 真实缺省（空 `projects`） | 全部既有回归用例 |
| 映射正确性 | 不使用替身：编译期类型强制 + 源码复核（`grep language src/commands` 零命中），非运行时断言面 | AC-5 |

### packages/desktop/src-tauri/src/commands/explores/mod.rs -> packages/desktop/src-tauri/src/commands/explores/mod_test.rs

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更：`promote_explore` / `promote_explore_with` 签名零变化，改动仅为同一 `config::ProjectConfig → BootstrapProject` 投影后透传给 `write::promote_explore`（`language` 不消费，DD8）。本文件零新增用例；下列条目 = 透传行为面 + 实读 `mod_test.rs` 登记的既有用例回归。 -->

- `promote_explore_with()`: 同一 `config::ProjectConfig → BootstrapProject` 投影后透传 `write::promote_explore`——投影行为由编译期类型强制 + 源码复核承担，运行面由既有 `promote_explore_with` 用例（未配置 `config.json` → 空 `projects` → legacy 分支）回归
- `promote_explore四步成功_笔记移走记录保留打标()`: 既有用例回归——空 `projects` 下四步（写面建 change / 笔记移走 / 记录保留 / `promoted_to` 打标）形态不变
- `promote_explore透传test_only且move照常()`: 既有用例回归——`workflow_type` 透传与笔记移动半边不受第 4 参形态影响
- `promote_explore_workflow_type大小写空白canonical()`: 既有用例回归——canonical 归一发生在写面，命令面投影不介入
- `promote_explore_blank_root与非kebab显式err零读写()`: 既有用例回归——前置校验先于映射与透传，零读写
- `promote_explore笔记未落盘或空白拒绝()`: 既有用例回归——笔记读取步序不受投影影响
- `promote_explore已promoted拒绝()`: 既有用例回归——`promoted_to` 已设即拒绝
- `promote_explore非法workflow_type拒绝零铸出()`: 既有用例回归——零 change 铸出
- `promote占位门禁先于笔记读取步序()`: 既有用例回归——占位名门禁时序先于笔记读取与映射
- `用户自建draft前缀名promote同被门禁拦截()`: 既有用例回归——门禁对用户自建 `draft-` 前缀名同样生效

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `promote_explore_with()` | 正向 | 未配置 `config.json` 的 workspace → 投影产出空 `Vec<BootstrapProject>` → `write::promote_explore` legacy 分支；笔记移走 + 记录保留 + 打标链路完整（投影无返回值可断言，经 `PromoteOutcome` 与文件面间接观察） | 单点 |
| `promote_explore四步成功_笔记移走记录保留打标` | 正向 | 既有断言逐字回归（空 `projects` 路径） | 单点 |
| `promote_explore透传test_only且move照常` | 正向 | 既有断言逐字回归（透传形态与 move 半边） | 单点 |
| `promote_explore_workflow_type大小写空白canonical` | 边界 | 既有断言逐字回归（canonical 落库在写面，投影零归一） | 单点 |
| `promote_explore_blank_root与非kebab显式err零读写` | 异常 | 既有断言逐字回归（映射前显式 `Err`，零文件读写） | 单点 |
| `promote_explore笔记未落盘或空白拒绝` | 异常 | 既有断言逐字回归（笔记读取步序不变） | 单点 |
| `promote_explore已promoted拒绝` | 异常 | 既有断言逐字回归（`promoted_to` 已在即拒绝） | 单点 |
| `promote_explore非法workflow_type拒绝零铸出` | 异常 | 既有断言逐字回归（零 change 铸出） | 单点 |
| `promote占位门禁先于笔记读取步序` | 边界 | 既有断言逐字回归（门禁步序先于读取与映射） | 单点 |
| `用户自建draft前缀名promote同被门禁拦截` | 边界 | 既有断言逐字回归（前缀名门禁一致） | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| —（无 mock） | 既有装置为真实 tempdir + 真实 `Store` + `tauri::test::MockRuntime` + 真实 git 仓夹具，本次零改动；`config.json` 由 tempdir 真实缺省（空 `projects`） | 全部既有回归用例 |
| 映射正确性 | 不使用替身：编译期类型强制 + 源码复核（`grep language src/commands` 零命中），非运行时断言面 | AC-5 |

### packages/desktop/src/types/generated/bindings.ts -> packages/desktop/src-tauri/src/bindings/mod_test.rs

#### 待测功能

<!-- design.md 的「公共函数 / API」子表无本文件行（生成物）；下列条目取自「类型定义」子表 `ProjectConfig`（TS 生成物）行。生成物禁手改，测试只读产物文本断言，不 mock 生成流程。 -->

- `ProjectConfig`（`bindings.ts` 出线）: `{ root: string; language: string | null; dependsOn: string[] | null }`——两新键为必填键、`Option` 出线为 `null`、camelCase 出线

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ProjectConfig` | 正向 | `type_section("ProjectConfig")` 依次含 `root: string,` / `language: string \| null,` / `dependsOn: string[] \| null,`（既有 `root` 断言扩展两键，字段注释「项目根单字段」随动更新） | 单点 |
| `ProjectConfig` | 异常 | 对产物文本副本删除 `language` 行 / 删除 `dependsOn` 行 → 断言失败且失败文案指认缺键（防生成面漏字段；副本改写不外写文件） | 矩阵迭代（2 副本） |
| `ProjectConfig` | 边界 | `WorkspaceConfig.projects` 仍为 `projects: ProjectConfig[],`（容器形态不变，元素形状变宽） | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 生成物文件 | 真实读取 `src/types/generated/bindings.ts`（`std::fs::read_to_string`），无 mock；异常形态用内存字符串副本 | 全部用例 |
| 生成流程 | 不 mock：生成幂等由既有「连续两次 `bindings:export` 零 diff」用例覆盖（本变更零改动） | 回归面 |

### packages/desktop/src/types/generated/bindings.zod.ts -> packages/desktop/src-tauri/src/bindings/mod_test.rs

#### 待测功能

<!-- design.md 的「公共函数 / API」子表无本文件行（生成物）；下列条目取自「类型定义」子表 `ProjectConfig`（TS 生成物）行与 DD10。 -->

- `ProjectConfigSchema`（`bindings.zod.ts`）: `strictObject` 增两 getter——`language` 为 `z.string().nullable()`、`dependsOn` 为 `z.array(z.string()).nullable()`，与 `bindings.ts` 类型面同构

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ProjectConfigSchema` | 正向 | `ProjectConfigSchema` 段含 `get language() { return z.string().nullable(); }` 与 `get dependsOn() { return z.array(z.string()).nullable(); }`，且 `WorkspaceConfigSchema.projects` 仍为 `z.array(ProjectConfigSchema)` | 单点 |
| `ProjectConfigSchema` | 边界 | `strictObject` 包裹不变（两 getter 落同一对象内，不裂成独立 schema）；连续两次导出零 diff（既有幂等用例回归） | 单点 |
| `ProjectConfigSchema` | 异常 | 产物文本副本删除 `dependsOn` getter → 断言失败并指认缺键（zod 面漏字段即前端解析面漏字段） | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 生成物文件 | 真实读取 `src/types/generated/bindings.zod.ts`，无 mock；异常形态用内存字符串副本 | 全部用例 |

### packages/desktop/src/views/config/components/basic-config-section.tsx -> packages/desktop/src/views/config/components/basic-config-section.test.tsx

#### 待测功能

<!-- design.md 的「公共函数 / API」子表无本文件行；下列条目取自 DD9（`ProjectsGroup` 三要素呈现 + 私有 `depsText` 三态）与 AC-7，测试面为公共组件 `BasicConfigSection`（`ProjectsGroup` / `depsText` 为模块私有，经渲染观察，不新增导出）。 -->

- `ProjectsGroup`（经 `BasicConfigSection` 呈现）: 逐项目呈现 `root` + `language：<值>` + `depends_on：<值>`；已设值一律呈现（MUST NOT 隐藏），未设标注「未设」；`data-testid="config-projects"` 与只读形态保持
- `depsText()`: `null` → `未设`、`length === 0` → `（空列表）`、否则 `join('、')`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ProjectsGroup` | 正向 | 单项目 `{root:'packages/desktop', language:'typescript', dependsOn:['packages/desktop/src-tauri']}` → 该 `<li>` 文本依次含 `packages/desktop` / `language：typescript` / `depends_on：packages/desktop/src-tauri`；`config-projects` 与 `li` 结构保留 | 单点 |
| `ProjectsGroup` | 正向 | 多项目不同值并列（`typescript` + 依赖 / `rust` 无依赖）→ 逐项呈现不串行，顺序 = 配置序 | 单点 |
| `ProjectsGroup` | 边界 | `language: null` + `dependsOn: null` → 两处「未设」，`root` 仍呈现（未设不吞行、已配置值不隐藏） | 单点 |
| `depsText()` | 边界 | `dependsOn: []` → `depends_on：（空列表）`；单元素；三元素 → 「、」连接（与全角冒号同域） | 矩阵迭代（3 形态） |
| `depsText()` | 边界 | `language: ''`（已设空串）→ 呈现 `language：` 空值形态，不落「未设」（与 `null` 可区分，协议零校验允许空串） | 单点 |
| `ProjectsGroup` | 边界 | `projects: []` → 「（空列表）」且零 `<li>`（既有空态不变） | 单点 |
| `ProjectsGroup` | 异常 | 超长值：`language` 1200 字符 + `dependsOn` 20 项 → 单文本节点完整呈现（不截断、不抛错） | 单点 |
| `ProjectsGroup` | 异常 | 特殊字符：`language: 'Type\nScript🙂'` → 值原样进入呈现面（正则文本查询容忍空白折叠，值不被吞 / 不转义错乱） | 单点 |
| `ProjectsGroup` | 异常 | 只读边界：含两字段的非空 `projects` 区经 `querySelectorAll('input, textarea, select, button')` 取到 0 个编辑控件；组件入参只有 `config` / `defaultedPaths`（无回调 / 无 onChange） | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| —（无 mock） | 纯展示组件，`WorkspaceConfig` fixture 直传入参渲染；进程边界（IPC / 配置读取）不在本组件内 | 全部用例 |
| 生成物类型 | 不 mock：fixture 直接以生成物 `ProjectConfig` 类型字面量构造（编译期即校验出线形状） | fixture 构造 |

## 不可测试项

| 模块 | 原因 | 替代保证 |
|---|---|---|
| `packages/desktop/package.json`（`version` `0.6.59` → `0.6.60`） | 版本行为一行字面量，无函数 / 分支 / 输出可断言；既有测试零引用该文件 | 桌面端 `run check` 链 + `git diff` 复核（AC-8）；`src-tauri/tauri.conf.json` 经 `"version": "../package.json"` 跟随、`src-tauri/Cargo.toml` 不随动，同属机械核对 |
| `install_order`（`write/create.rs` 私有纯函数） | 私有 `fn`，兄弟测试模块不可达；为其加 `pub` 只服务测试，污染导出面（AGENT.md「no test-only exports」） | 产出序列经 `create()` → `bootstrap` → `FakeVcs.installs()` 行为面覆盖；期望序取自 DD6 确定序列表（测试与实现共用同一份表） |
| `crates/core/workflow/Cargo.toml` 依赖面（AC-5「零新增 `config` 边」） | 清单文件无运行时行为，无单元测试路径 | `cargo tree -p workflow` + `git diff` 复核；`BootstrapProject` 收纯数据（DD4）即其实现落点 |
| `packages/desktop/src/app.test.tsx`、`packages/desktop/src/views/config/config-view.test.tsx` 的两处项目 fixture | 非被测源文件，仅为生成物出线变宽的编译闭合（补 `language: null, dependsOn: null`） | 断言与期望值零改，随全量测试回归；若漏补则 TS 编译即失败（编译期强制） |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/promote_test.rs:85` 的 `&[]` | 空切片对 `&[BootstrapProject]` 类型推断成立，文件无需触碰 | 类型推断由编译期强制；promote 行为面由本文件既有用例 + 新增拓扑序一例覆盖 |
