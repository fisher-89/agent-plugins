# desktop-workspace-code-stats Specification

## Purpose

定义 desktop 工作区基础信息页与代码统计命令轨道的契约：`/info` 入口可达性（以当前工作区根为数据基准）、Rust `code_stats` 命令薄包装形态（tokei 单次遍历产出汇总 / 语言行 / 目录树三面，不落库无缓存）、统计呈现面与目录树展开折叠、深度可调与显式刷新取数模型，以及解析语义边界与测试纪律。

## Requirements

### Requirement: 基础信息页入口与可达性

前端 SHALL 在侧栏「页面」组新增 [基础信息] 入口（`data-testid="nav-info"`），经路由 `/info` 承载（路由表与 NavLink 契约见 desktop-page-routing）。基础信息页 SHALL 以**当前工作区根**为数据基准（workspace 根为 sidebar 本地态，页面 MUST NOT 以 URL 参数承载根）；仅壳态可达：欢迎态（root 为 null）MUST NOT 渲染壳与该入口。切换工作区 SHALL 以新根重新发起解析并清空旧根数据；切入基础信息页 MUST NOT 触发 change / explore 取数，既有页面互切语义不变。

#### Scenario: 入口与渲染

- **WHEN** 壳态下用户点击「页面」组的 [基础信息]
- **THEN** URL 变为 `/info`，基础信息页渲染并以当前根与默认深度发起 `code_stats` 解析，workspace 清单组仍在

#### Scenario: 欢迎态不可达

- **WHEN** `root` 为 null（欢迎态）
- **THEN** 页面无壳 DOM 与 `nav-info` 入口，基础信息页不可达

#### Scenario: 切换工作区重取

- **WHEN** 用户在 `/info` 页点击 sidebar 另一 workspace 清单项
- **THEN** 页面以新根重新发起解析，旧根统计数据不再呈现，无 change / explore 取数被触发

### Requirement: 代码统计解析命令

Rust 侧 SHALL 新增 `commands/stats/` 命令轨道，提供 `code_stats(root: String, depth: u32) -> Result<CodeStatsReport, String>`：

- 命令体 SHALL 为三件事薄包装（参数转换 / 调用 / 错误映射），领域组装收在 `code_stats_inner` 纯函数（`*_inner` app 层微形态先例，可不经 Tauri 运行时直接测试）；
- 解析 SHALL 经 tokei library（依赖仅落 desktop-app，版本精确 pin），MUST NOT spawn tokei CLI 子进程；
- 一次调用 SHALL 完成一次遍历并产出三面数据：汇总（文件数 / 代码行 / 注释行 / 空行）、语言行（按代码行降序的逐语言统计）、目录树（至多 `depth` 级目录节点的聚合统计 + 父目录在深度内的文件叶，见目录树面 Requirement）——树面 MUST NOT 触发第二次独立遍历；
- 命令 MUST NOT 注册 store 模型、MUST NOT 落库、MUST NOT 持有任何缓存（内存或磁盘）——每次调用都完整重新解析；
- DTO SHALL `specta::Type` + `serde(rename_all = "camelCase")` 出线，经既有 bindings 重导出管线生成 typed 包装，前端 MUST NOT 以裸 `invoke('code_stats')` 调用；
- 命令形态沿用 sync 命令惯例（sync 命令不在主线程执行）；如需 async + `spawn_blocking` 由 design 定夺并记录理由；
- root 不存在或不可读 SHALL 返回 `Err(String)` 走前端 reject，MUST NOT panic、MUST NOT 静默返回空。

#### Scenario: 薄包装结构证明

- **WHEN** 审查 `commands/stats` 实现与测试
- **THEN** 命令体为参数转换 + `code_stats_inner` 调用 + 错误映射三段，测试以 serde_json 对照证明命令与 inner 结果一致（不经 Tauri 运行时）

#### Scenario: 单次遍历产出三面

- **WHEN** 对同一 fixture 工作区调用一次 `code_stats`
- **THEN** 返回的汇总、语言行、目录树来自同一次解析（树目录聚合与语言行总数自洽），无第二次遍历命令或调用

#### Scenario: 绑定出线与 typed 调用

- **WHEN** 执行 bindings 重导出并审查前端调用点
- **THEN** `bindings.ts` 含 `codeStats` 包装与 `CodeStatsReport` 系 camelCase 类型，前端经生成绑定调用，无裸字符串 invoke

#### Scenario: 不落库无缓存

- **WHEN** 审查 `commands/stats`、store 注册模型与全链代码
- **THEN** store 无新注册模型，命令无 memo / 静态缓存 / 磁盘缓存，连续两次调用各自完整解析

#### Scenario: 失败走 reject

- **WHEN** 以不存在的 root 调用 `code_stats`
- **THEN** 命令返回 `Err(String)`，前端收到错误字符串，进程无 panic、无空报告假成功

### Requirement: 统计呈现面

基础信息页 SHALL 呈现两个统计面：

- **汇总面**：文件数、代码行、注释行、空行四项总量；
- **语言占比面**：逐语言一行（语言名 | 文件数 | 代码 | 注释 | 空行 | 占比），按代码行降序排列；占比 SHALL 以 Progress 组件承载（`value` 传百分比，MUST NOT 内联 `style width`），占比口径（代码行份额）由 design 定夺。

解析结果为空（无任何被识别的代码文件）SHALL 呈现空态而非错误；查询失败 SHALL 按「错误呈现双轨」查询轨 inline 持久呈现（无 toast 顶替）。

#### Scenario: 汇总与语言表渲染

- **WHEN** 对多语言 fixture 工作区完成解析
- **THEN** 汇总面四项总量正确呈现，语言表按代码行降序排列且每行含五项统计与占比条

#### Scenario: 空态

- **WHEN** 工作区无任何被 tokei 识别的代码文件
- **THEN** 页面呈现空态文案，无 error-note、无崩溃

#### Scenario: 查询失败 inline 持久

- **WHEN** `code_stats` reject
- **THEN** 页面 inline 持久呈现错误（testid 承载、不自动消失），无 toast 顶替

### Requirement: 目录树展开折叠

目录树面 SHALL 以树形行呈现至多 `depth` 级目录节点，并支持展开到文件层：目录行含目录名与该目录聚合统计（文件数 / 代码行，口径 design 定夺），父目录在深度内的直接被解析文件 SHALL 以文件叶行（文件名 + 单文件代码行）呈现——根层直属文件为顶层文件叶，不设虚拟根目录节点；深度外的文件不出叶、仅并入最深可达祖先的聚合。同父条目 SHALL 目录先于文件（各自按名字典序）。目录节点 SHALL 可展开 / 折叠（本地 state），文件叶行 MUST NOT 有展开开关；**折叠节点的子树 MUST NOT 渲染 DOM**（按需渲染，作为不引虚拟化依赖的渲染面收敛手段）；默认展开层级由 design 定夺。树上统计数据 SHALL 为 `code_stats` 单次解析结果的聚合投影，MUST NOT 为树的展开交互发起增量解析调用。

#### Scenario: 深度内节点与文件叶呈现

- **WHEN** 以 `depth=2` 解析含三级目录与多级文件的工作区
- **THEN** 树仅呈现至第二级目录节点，第三级目录不出现在任何展开态中；父目录在深度内的文件以文件叶行呈现，深度外的文件不出叶

#### Scenario: 展开到文件层

- **WHEN** 用户展开一个含直接被解析文件的目录节点
- **THEN** 其直属文件叶行（文件名 + 代码行）随子目录行呈现且目录行在前、文件行在后，文件叶行无展开开关，期间无新的 `code_stats` invoke 发生

#### Scenario: 展开折叠交互

- **WHEN** 用户折叠一个已展开的目录节点后再展开
- **THEN** 折叠期间其子树行从 DOM 消失，重新展开后子树行恢复且统计不变，期间无新的 `code_stats` invoke 发生

#### Scenario: 大树按需渲染

- **WHEN** 解析结果含大量深层目录节点且全部保持折叠
- **THEN** 未展开节点的子树行不在 DOM 中（以子行存在性断言），页面渲染节点数与展开量而非全量树规模成正比

### Requirement: 深度可调与显式刷新取数

`depth` SHALL 默认为 5；页面 SHALL 提供深度调节控件（值域与控件形态 design 定夺，MUST NOT 以新增重型依赖为前提）。取数 SHALL 遵循显式刷新模型：进入页面发起一次解析；调节深度 SHALL 以新深度重新发起解析并更新树面；页面 SHALL 提供显式刷新入口（沿用 `disabled={loading}` 语义）。MUST NOT 引入轮询、文件 watch、事件订阅或任何缓存层；重复进入页面 SHALL 重新解析而非复用上一次结果。

#### Scenario: 默认深度与调节重取

- **WHEN** 进入基础信息页后将深度从 5 调至 3
- **THEN** 恰以 `depth=3` 重新发起一次 `code_stats`，树面收缩至两级目录，汇总面与语言面随之更新为新解析结果

#### Scenario: 显式刷新

- **WHEN** 用户点击刷新按钮
- **THEN** 重新发起一次完整解析，loading 期间刷新钮 disabled，无任何自动重取

#### Scenario: 无轮询无缓存

- **WHEN** 停留在基础信息页不操作
- **THEN** 无定时器重取、无 watch 订阅、无后台解析；离开再进入页面才产生下一次解析调用

### Requirement: 解析语义边界与测试纪律

遍历、语言识别、`.gitignore` 尊重、hidden 跳过等语义 SHALL 直接依赖 tokei 既有行为，MUST NOT 在命令层重写识别或过滤规则。`Config::depth` 的实际目录聚合行为 SHALL 在 dev-design 前以最小样例 PoC 验证并留档 design（specta PoC 先例）；不符预期时退路 SHALL 为基于 tokei Report 列表按目录前缀自聚合。测试 SHALL 只覆盖自研组装层（inner 聚合、DTO 映射、排序、树构建、深度截断、文件叶归属与排序、前端交互），MUST NOT 逐项断言 tokei 自身的计数 / 匹配语义。desktop 前端 SHALL 维持全管线通过（`vp check --fix` / knip / `vp test`，`max-lines-per-function: 50` 与 data-testid 纪律不变），Rust 侧 `cargo test --workspace` 全绿。

#### Scenario: 不重写识别规则

- **WHEN** 审查 `commands/stats` 源码
- **THEN** 无手写扩展名映射表、无自研 gitignore 匹配、无目录排除白名单，识别与过滤全部委托 tokei 配置

#### Scenario: PoC 先行留档

- **WHEN** 审查 design 与实现顺序
- **THEN** `Config::depth` 聚合行为的最小样例验证记录在案，且先于树面实现任务执行

#### Scenario: 测试边界

- **WHEN** 检索新增 Rust 与前端测试
- **THEN** 断言集中在自研组装层与交互行为，无「tokei 对某语言计 N 行」类库语义断言

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src-tauri/src/commands/stats/`（新轨道） | 代码统计解析命令轨道 | `code_stats(root, depth) -> Result<CodeStatsReport, String>` 薄包装（三件事）；`code_stats_inner` 纯函数微形态（可离 Tauri 测试）；单次遍历出三面；不落库、无缓存、无第二次遍历 |
| `CodeStatsReport` / `CodeTotals` / `LanguageStats` / `DirNode` / `FileNode` / `TreeEntry`（DTO） | 出线数据面 | `specta::Type` + camelCase；汇总 / 语言行（代码行降序）/ 树条目信封（`TreeEntry` tag `kind` 目录 / 文件叶双变体，目录名 + 聚合统计 + 子条目 / 文件名 + 单文件行统计）；字段级形状 design 定夺 |
| `packages/desktop/src-tauri/Cargo.toml` + workspace.dependencies | tokei 依赖落位 | tokei 仅落 desktop-app；版本精确 pin；不进 core / infra crate |
| `packages/desktop/src-tauri/src/bindings/mod.rs` | 命令注册 | `collect_commands!` 增 `code_stats`；bindings 重导出管线（入库 + diff 守卫）不变 |
| `packages/desktop/src/views/info/`（新） | 基础信息页视图域 | `info-view.tsx` + 汇总面 / 语言占比表 / 目录树组件；树折叠态按需渲染；Progress 承载占比；空态与 inline 错误 |
| `packages/desktop/src/views/info/hooks/use-code-stats.ts`（新） | 取数收口 hook | `{ data, loading, error, refresh }` 形态；深度参数化 invoke；显式刷新模型；查询失败 inline |
| `packages/desktop/src/routes.tsx` + `src/components/app-sidebar.tsx` | 路由与入口 | `/info` 路由项；「页面」组 [基础信息] NavLink，`data-testid="nav-info"`，active 由 URL 派生 |
| `*.test.ts(x)` / `mod_test.rs`（配套测试） | 自研组装层验证 | data-testid 挂钩、invoke mock、tempdir fixture；折叠态子树不渲染断言；无 tokei 库语义断言 |
