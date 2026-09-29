# 设计: desktop-workspace-code-stats

> **变更**: desktop-workspace-code-stats
> **日期**: 2026-09-29

---

## 提案与规格同步状态

- `proposal.md` 与 `specs/desktop-workspace-code-stats/spec.md`、`specs/desktop-page-routing/spec.md`（均相对域根）已由 proposal 阶段写入并过审（workflow eval pass），本设计不重复其内容，也不把它们列入变更清单与任务。
- proposal「测试文件」节所列五组测试（stats 命令 mod_test / use-code-stats.test / info-view.test / app-sidebar.test / app.test）归 test-design / test-gen / test-execution 阶段承接，不入本清单与任务列表；既有 `bindings_test.rs` 的「22 条命令包装」断言需随之扩为 23 条，同归测试阶段。
- 本 design 为首轮（无回溯差异表）。

## PoC 前置门（结论已回填，先于树面实现）

proposal 裁定：`Config::depth` 的实际目录聚合行为须在 dev-design 前以最小样例 PoC 留档（specta PoC 先例），不符预期退 Report 自聚合。PoC 已于本设计阶段完成（仓库外 scratch 工程，`tokei = "=13.0.0"` + `default-features = false` 编译运行；`Config` 字段面对 12.1.2 与 13.0.0 两版源码双核实），验证对象与实测结论：

| 验证对象 | 通过判据 | PoC 实测结论（已回填） |
|----------|----------|----------|
| `Config::depth` 存在性与目录聚合 | 库侧提供 depth 参数并产出目录聚合报告 | **否定（proposal 前提失实）**：tokei **12.1.2 与 13.0.0 的 `Config` 均无 `depth` 字段**（两版源码 grep `depth` 零命中）。`Config::depth` 路线在 12/13 两代都不存在，树面按 spec 预留退路落**基于 `Language::reports` 的目录前缀自聚合**（`code_stats_inner` 纯函数承接），无备选悬念 |
| 版本选型与依赖面 | 精确 pin 可编译、库 API（`Languages::get_statistics` / `Language::reports` / `Report` / `CodeStats`）可用 | **`13.0.0` + `default-features = false`**：PoC 以该 pin 编译运行通过；`cli` feature（默认开启）剥离后 clap 4 / colored / env_logger / num-format 全部不入树，纯库依赖面为 ignore / rayon / regex 等现代版本。被拒：12.1.2（2021 年线且 clap 2 为**非可选**依赖，库用途也拖整条 CLI 栈）；14.0.0 / 15.0.0（同 API 面，无必要追新） |
| `Language::reports` 逐文件报告 | 每文件一条 `Report { name, stats }`，语言总量与 reports 求和一致 | **通过**：`get_statistics` 恒填充 per-file reports；`Language::total()` 仅对 `reports` 求和（源码核实）→ 汇总面 / 语言面 / 树面**同源自洽**（同一批 per-file 报告的三种投影） |
| `Report.name` 路径形态 | 可剥离 root 前缀得相对路径 | **通过（需归一化）**：name 为绝对路径且分隔符混排（Windows 下 `/` 与 `\` 并存，随入参形态）；`strip_prefix(root)` + `components()` 剥离后统一以 `/` 拼接相对路径。剥离 miss 的文件不计入树面目录节点（实际不可达：name 由同一 root 的遍历派生；文件作 root 的输入被有效性检查前置拦截，见下） |
| hidden / ignore 语义 | 依赖 tokei 既有行为 | **通过（带边界，照单全收）**：hidden 目录默认跳过（`.hiddendir` 未出现）；`.gitignore` 尊重以 git 仓库存在为前提（`ignore` crate `require_git` 默认），**非 git 目录中 ignored 目录 / target 等一并计入**——spec 已裁定不在命令层发明过滤规则，此边界留档为已知行为，真实工作区多为 git 仓库 |
| 非法 root 行为 | 缺失根不 panic | **通过（需前置有效性检查）**：缺失根 → 空 `Languages`（不 panic、不报错）——与 spec「不存在或不可读 root SHALL 返回 `Err(String)`」相悖，故 `code_stats_inner` 前置 `std::fs::metadata` 有效性检查作为 **Err 通道唯一来源**（缺失 / 不可读 / 非目录 → `Err`；tokei 自身解析无 Err 面）；目录存在但无被识别文件 → 空 report 走前端空态 |

深度语义结论（承接上表第 1 行）：**深度是 `code_stats_inner` 的聚合截断参数，不是遍历限制**——tokei 遍历恒全量，解析耗时与 depth 无关，汇总面与语言面数字不随 depth 变化（AC-4 只要求树面收缩，成立）。proposal「平衡信息量与耗时」中的耗时面随之失效，深度控件价值收敛为**树面信息密度调节**；「缓存 / 流式留待真实痛点」裁定不受影响（耗时恒定且无缓存承诺）。

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| `commands/stats` 命令轨道 | `code_stats` 三件事薄包装 + `code_stats_inner` 领域组装（root 有效性检查 → tokei 单次解析 → 汇总 / 语言 / 树三面组装）+ 四个出线 DTO | `packages/desktop/src-tauri/src/commands/stats/mod.rs`（新） | tokei、serde、specta | tokei 13 library（`Config::default()`，零字段覆写） |
| bindings 注册面 | `collect_commands!` 增 `code_stats`（第 23 条），既有重导出管线（`bindings:export` + diff 守卫）不变 | `packages/desktop/src-tauri/src/bindings.rs` | tauri-specta、specta-typescript | 既有管线零改型 |
| `/info` 路由与入口 | 路由表增 `/info` 项；「页面」组增 [基础信息] NavLink（首位，active 由 URL 派生） | `packages/desktop/src/routes.tsx`、`src/components/app-sidebar.tsx` | react-router | 既有路由 / NavLink 模式 |
| 基础信息页视图域 | 三面呈现（汇总 / 语言占比表 / 目录树）+ 深度控件 + 显式刷新 + 空态 / inline 持久错误 | `packages/desktop/src/views/info/`（新：`info-view.tsx` + `components/` 三件） | 生成绑定、`ui/table`、`ui/progress`、`ui/button` | React + Tailwind（无新 UI 依赖） |
| `useCodeStats` 取数 hook | 显式刷新模型：root/depth/tick 触发 invoke，loading / error / 数据 root 归属标记（切换过渡轮抑制） | `packages/desktop/src/views/info/hooks/use-code-stats.ts`（新） | 生成绑定 `commands.codeStats` | 既有 hooks 模式（`use-explore-list` 同型） |
| 依赖收敛面 | tokei 精确 pin 落 `[workspace.dependencies]`，仅根包（desktop-app 壳）引用 | `packages/desktop/src-tauri/Cargo.toml` | — | workspace 依赖收敛惯例 |

## 设计定夺（proposal 待决问题闭环）

| 待决问题 | 定夺 | 理由 / 被拒备选 |
|----|------|------|
| tokei 精确版本 | `tokei = { version = "=13.0.0", default-features = false }`（workspace.dependencies 收敛，根包 `workspace = true` 引用） | 13.0.0 为 2025-11 稳定版、`=` pin 沿 specta / native_db 先例；`default-features = false` 剥离 CLI 栈（clap 4 等）是纯库消费的最低依赖面（PoC 编译验证）。被拒：12.1.2（clap 2 非可选、2021 停更线）；14/15（同 API 面追新无益） |
| 深度控件形态与值域 | 原生 `<select>`（Tailwind 样式化，`data-testid="info-depth-select"`），值域 **1–10**，默认 **5**，变更即以新 depth 重取 | 不新增依赖的前提（spec 明令）：ui 库无 select / slider 组件，原生控件可测可样式化零成本；上限 10 防近全量退化（proposal 风险表），且深度不再影响耗时（PoC 结论），上限纯为树面 DOM 规模收敛。被拒：shadcn Select / radix Slider（新组件依赖，弃） |
| 目录树默认展开层级 | **前 2 级默认展开**（相对路径段数 ≤ 2 的节点默认展开，更深默认折叠），展开 / 折叠为 per-path 本地 state（`Set<string>`），不随重解析重置 | 保守初始 DOM（风险表「渲染卡顿」缓解；折叠态子树不渲染 DOM 是主收敛手段，默认展开是辅助）；per-path 键稳定，调深 / 刷新后同名目录保持展开态，无需重置 effect。被拒：全展开（大树初始 DOM 失控）；全折叠（默认视图零信息量） |
| 占比口径 | **代码行份额** = 该语言 code / Σ全部语言 code，以百分点 0–100 出线（`share: f64`，Σcode 为 0 时为 0.0） | 与「按代码行降序」同轴（排序键与占比键一致，呈现自洽）；Progress `value` 直用。被拒：总行份额（稀释排序轴，两键不一致） |
| 空白 root 防御 | blank root 与不存在 / 不可读 / 非目录 root 同走 `Err`（`code_stats_inner` 单点 `metadata` 有效性检查） | spec 明令无效 root「MUST NOT 静默返回空」，Err 暴露调用方 bug；壳态 root 恒有值，blank 只能来自调用 bug。被拒：空 report（`list_changes` 先例的 blank → 空结果适用于非 Result 命令的空 workspace 呈现语义，本命令 spec 反向裁定） |
| sync vs async 命令形态 | sync `fn`（沿既有 22 条命令惯例，`#[tauri::command]` + `#[specta::specta]`，零 async 声明） | 现状惯例：sync 命令不在 UI 主线程执行（proposal 裁定引用），大仓库解析的等待感由前端 loading 态 + 刷新钮 `disabled={loading}` 承接；`agent_start` 的 async 为后台任务 handoff，本命令一次性完整解析无 handoff 需求。被拒：async + `spawn_blocking`（需为单命令引入 tokio runtime 面——当前 tokio 仅 dev-dependencies 且 features=`["sync"]`，不成比例） |
| `DirNode` 聚合口径 | 子树全量聚合：节点统计 = 该目录子树内**全部**被解析文件合计（含更深文件，祖先链逐级累计）；**根层直属文件不产生目录节点**（不设虚拟根节点，其行数由汇总面 / 语言面承载） | 与 PoC 验证的前缀聚合语义一致（`src` 含 `src/deep` 的行）；树面职责为「代码在目录上的分布」下钻，根层文件无目录归属。被拒：虚拟根节点（投机复杂度）；仅直属文件计数（祖先不含后代，聚合失真） |
| tokei `Config` 形态 | `Config::default()`，**零字段覆写**，MUST NOT `from_config_files()` | spec 场景「不重写识别规则」：识别 / hidden / ignore 全部委托 tokei 默认行为；`from_config_files()` 会读用户全局 tokei 配置引入跨机器非确定性，同一工作区任何机器必须同口径 |

## 变更清单

<!-- 以文件为入口逐层展开，实现阶段以此清单为边界。覆盖 proposal「变更范围-实现文件」全部条目；测试文件不在本清单（归测试阶段）。 -->

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/src-tauri/src/commands/stats/mod.rs` | 代码统计命令轨道（单文件模块）：`code_stats` 命令薄包装 + `code_stats_inner` 领域组装纯函数 + DTO 四型（`CodeStatsReport` / `CodeTotals` / `LanguageStats` / `DirNode`，`specta::Type` + `serde(rename_all = "camelCase")`）；轨道 doc 注明能力指针 `specs/desktop-workspace-code-stats/spec.md`（路径相对域根），指针措辞遵循 layout_test 命名隔离纪律 |
| `packages/desktop/src/views/info/info-view.tsx` | 基础信息页骨架：标题 + 深度控件（原生 `<select>` 1–10，默认 5）+ 刷新钮（`disabled={loading}`）+ 三面呈现区 + 空态（`data-testid="info-empty"`）+ inline 持久错误（`data-testid="info-error"`，`bg-fail-bg` 错误呈现双轨查询轨，无 toast 顶替） |
| `packages/desktop/src/views/info/components/stats-summary.tsx` | 汇总面：文件数 / 代码行 / 注释行 / 空行四项总量（`data-testid="info-summary"`） |
| `packages/desktop/src/views/info/components/language-table.tsx` | 语言占比表（`ui/table`）：语言 \| 文件数 \| 代码 \| 注释 \| 空行 \| 占比，行序即 `languages` 入参序（已按代码行降序）；占比以 `ui/progress` 承载（`value={share}`，无内联 `style width`——该组件内部以 transform 实现），行挂钩 `data-testid="info-language-row"` |
| `packages/desktop/src/views/info/components/dir-tree.tsx` | 目录树（递归行组件）：目录名 + 聚合统计（文件 / 代码）+ 缩进层级；展开 / 折叠 per-path 本地 state，**折叠节点子树不渲染 DOM**；默认展开前 2 级；节点行 `data-testid="info-dir-node"` + `data-path` |
| `packages/desktop/src/views/info/hooks/use-code-stats.ts` | 取数收口 hook：显式刷新模型（root / depth / tick 触发），`cancelled` 防串轮 + 数据归属 root 标记（切换工作区过渡轮不呈现旧根报告，`use-explore-list` 抑制哲学）；无轮询、无缓存 |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src-tauri/Cargo.toml` | `[workspace.dependencies]` 新增 `tokei = { version = "=13.0.0", default-features = false }`；根包 `[dependencies]` 新增 `tokei = { workspace = true }` | `=` 精确 pin（specta / native_db 先例）；`default-features = false` 剥离 `cli` feature（clap 4 / colored / env_logger / num-format 不入树，PoC 验证）；tokei 仅落 desktop-app 壳，MUST NOT 出现在任何 core / infra crate（`crates/` 零改动） |
| `packages/desktop/src-tauri/src/commands/mod.rs` | `pub mod stats;`；轨道清单 doc 注释五轨 → 六轨 | 既有轨道语义与 doc 纪律（三件事 / `*_inner` 微形态）照抄引用 |
| `packages/desktop/src-tauri/src/bindings.rs` | `collect_commands!` 增 `crate::commands::stats::code_stats`（第 23 条）；builder doc 注释命令总数 22 → 23 | 错误通道（`ErrorHandlingMode::Throw` → Promise reject）、`semantic_types`、bigint cast 全部不动 |
| `packages/desktop/src/types/generated/bindings.ts` | 重导出生成物：`codeStats` 包装 + `CodeStatsReport` / `CodeTotals` / `LanguageStats` / `DirNode` camelCase TS 镜像 | 生成物不手改：`pnpm -C packages/desktop run bindings:export` 产出入库 |
| `packages/desktop/src/routes.tsx` | 路由表增 `<Route path="/info" element={<InfoView root={root} />} />`（位于 `*` 兜底之前）；import `InfoView` | 既有路由项与未知路径兜底（`*` → `/changes`）不动 |
| `packages/desktop/src/components/app-sidebar.tsx` | `PageNavGroup` 增 [基础信息] NavLink（lucide `Info` 图标，`to="/info"`，`data-testid="nav-info"`，`isActive={pathname === "/info"}`），置于组内首位（组顺序：基础信息 → 变更 → 探索） | 既有 `nav-changes` / `nav-agent` / `nav-explores` / `nav-db` 挂钩与语义不动 |
| `packages/desktop/package.json` | `version` 0.3.6 → 0.3.7 | 版本收尾 bump |

<!-- `src-tauri/Cargo.lock` 为依赖 pin 的机械伴生（随 `tokei` 引入的 lock 条目），入库但不单列条目。proposal「测试文件」节各组与既有 `bindings_test.rs` 的 22→23 扩面归测试阶段，不在本清单。 -->

### 公共函数 / API

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `code_stats` | `packages/desktop/src-tauri/src/commands/stats/mod.rs` | 新增 | `#[tauri::command] #[specta::specta] pub fn code_stats(root: String, depth: u32) -> Result<CodeStatsReport, String>` | 三件事薄包装：参数转换（`String → &Path`）→ 调用 `code_stats_inner` → 错误映射（Err 透传 `String`）；sync 形态沿既有惯例 |
| `code_stats_inner` | `packages/desktop/src-tauri/src/commands/stats/mod.rs` | 新增 | `pub fn code_stats_inner(root: &Path, depth: u32) -> Result<CodeStatsReport, String>` | 领域组装纯函数（无 Tauri State，离 Tauri 运行时可测，`*_inner` app 层微形态）：root 有效性检查（`fs::metadata`，缺失 / 不可读 / 非目录 → `Err`）→ `Languages::get_statistics(&[root], &[], &Config::default())` **单次遍历** → 汇总 / 语言（代码行降序、tie 语言名字典序、share 百分点）/ 目录树（≤depth 前缀聚合）三面组装 |
| `InfoView` | `packages/desktop/src/views/info/info-view.tsx` | 新增 | `export function InfoView({ root }: { root: string }): React.JSX.Element` | 页面唯一出口组件；壳态 root 非空直用（不接 `string \| null`）；函数体拆分子组件，守 `max-lines-per-function: 50` |
| `StatsSummary` | `packages/desktop/src/views/info/components/stats-summary.tsx` | 新增 | `export function StatsSummary({ totals }: { totals: CodeTotals }): React.JSX.Element` | 汇总面 |
| `LanguageTable` | `packages/desktop/src/views/info/components/language-table.tsx` | 新增 | `export function LanguageTable({ languages }: { languages: LanguageStats[] }): React.JSX.Element` | 语言占比表 |
| `DirTree` | `packages/desktop/src/views/info/components/dir-tree.tsx` | 新增 | `export function DirTree({ nodes }: { nodes: DirNode[] }): React.JSX.Element` | 目录树（展开 / 折叠本地 state；内部递归 `DirNodeRow` 私有组件不导出） |
| `useCodeStats` | `packages/desktop/src/views/info/hooks/use-code-stats.ts` | 新增 | `export function useCodeStats(root: string): CodeStatsState` | 取数收口：组件不直接 invoke；`setDepth` / `refresh` 触发重取，失败落 `error` inline 持久 |
| `commands.codeStats` | `packages/desktop/src/types/generated/bindings.ts` | 新增 | `codeStats: (root: string, depth: number) => Promise<CodeStatsReport>` | 生成包装（reject 语义）；前端唯一调用面，禁裸 `invoke('code_stats')` |

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `CodeStatsReport` | `packages/desktop/src-tauri/src/commands/stats/mod.rs` | 新增 | 三面聚合根：`totals: CodeTotals` + `languages: Vec<LanguageStats>` + `tree: Vec<DirNode>`；derive `Debug, Clone, Serialize, specta::Type` + `#[serde(rename_all = "camelCase")]`（只序列化方向，无 Deserialize 需求） |
| `CodeTotals` | 同上 | 新增 | `files` / `code` / `comments` / `blanks` 四字段，`u64`（既有 bigint cast 口径出线 `number`）；`lines` 不设字段（派生值不预建，`code + comments + blanks` 前端可算） |
| `LanguageStats` | 同上 | 新增 | `name: String`（`LanguageType::name()`）、`files` / `code` / `comments` / `blanks: u64`、`share: f64`（百分点 0–100，代码行份额） |
| `DirNode` | 同上 | 新增 | `name: String`（末段目录名）、`path: String`（相对 root 的 POSIX 路径，`/` 分隔）、`files` / `code` / `comments` / `blanks: u64`（子树全量聚合）、`children: Vec<DirNode>`（≤depth，名字典序）；递归类型经 specta 出线 |
| `CodeStatsState` | `packages/desktop/src/views/info/hooks/use-code-stats.ts` | 新增（TS interface） | `report: CodeStatsReport \| null`、`loading: boolean`、`error: string \| null`、`depth: number`、`setDepth: (depth: number) => void`、`refresh: () => void` |
| 生成 TS 镜像四型 | `packages/desktop/src/types/generated/bindings.ts` | 新增 | `CodeStatsReport` / `CodeTotals` / `LanguageStats` / `DirNode` 的 camelCase 生成物（不手改） |

### 配置

| 配置键 | 所在文件 | 类型 | 默认值 | 说明 |
|--------|----------|------|--------|------|
| `tokei` | `packages/desktop/src-tauri/Cargo.toml` `[workspace.dependencies]` | 新增 | `{ version = "=13.0.0", default-features = false }` | crates.io 核实回填：13.0.0 稳定版（2025-11-25 发布，MSRV 1.71）；`default-features = false` 剥离 `cli` feature |
| `tokei` | `packages/desktop/src-tauri/Cargo.toml` 根包 `[dependencies]` | 新增 | `tokei = { workspace = true }` | 仅 desktop-app 壳引用 |
| `version` | `packages/desktop/package.json` | 修改 | `"0.3.7"` | 版本收尾 bump |

---

## 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `CodeStatsReport`（出线 DTO） | 见「类型定义」 | `totals` 为全部 per-file 报告合计；`languages` 逐语言行（排序后投影）；`tree` 为同批 per-file 报告的目录前缀投影——三面同源一次 `get_statistics`，树聚合与语言行自洽 | **无**（进程内出线面，MUST NOT 落库 / 无缓存） |
| store 模型 | 无新注册模型 | — | store / db 文件零触碰 |

演进纪律：`CodeStatsReport` 系为新增出线面，不触碰既有线格式与 golden 契约（workflow 域 `tests/golden/**` 不在本变更范围）；无落库模型即无版本演进面。

## 路由 / API 设计

<!-- 本变更不涉及 HTTP API。前端路由与 IPC 命令面如下。 -->

| 层 | 方法 / 路径 | 描述 | 输入 | 输出 | 认证 |
|----|------------|------|------|------|------|
| 前端路由 | `GET`（HashRouter）`/info` | 基础信息页（仅壳态可达；欢迎态无壳无入口） | 路由参数：无（root 由壳态 props 传入，MUST NOT 以 URL 参数承载根） | `InfoView` 渲染 | — |
| IPC 命令 | `invoke('code_stats')`（typed：`commands.codeStats`） | 工作区代码统计单次解析，三面 DTO | `{ root: string, depth: number }` | `Promise<CodeStatsReport>`（reject 语义，`Err(String)` 透传） | 本地应用 IPC，无认证面 |

## 依赖

### 运行时依赖

- `tokei`（`=13.0.0`，`default-features = false`）— 工作区遍历、语言识别、行计数、hidden / ignore 语义的唯一提供方；仅落 desktop-app 壳，core / infra crate 零触碰（依赖方向不变，`crates/` 禁 Tauri 同款隔离不破）
- `ignore` / `rayon` / `regex` 等（tokei 传递依赖）— 随 tokei 13.0.0 引入的现代版本线（PoC lock 实测一次性引入约 200 个 lock 条目，含其构建期代码生成依赖，接受一次性编译成本——proposal 风险表已裁定）

### 构建/测试依赖

- 无新增 — tempdir fixture 复用既有 dev-dependencies（`tempfile`、`tokio`、tauri `test` feature）；生成物管线复用既有 `bindings:export`（`cargo run --bin export-bindings`，非测试执行）

---

## 验收标准对齐

| AC | 设计落点 |
|----|----------|
| AC-1（`nav-info` 入口 + `/info` 渲染 + 欢迎态隔离） | `app-sidebar.tsx` `PageNavGroup` 增 [基础信息] NavLink（首位、`data-testid="nav-info"`、active 由 URL 派生）；`routes.tsx` 增 `/info` 项；壳态才挂 Router / 侧栏的既有结构不破（欢迎态无壳无入口） |
| AC-2（`collect_commands!` 注册 + typed 调用） | `bindings.rs` 增 `code_stats`（23 条）；`bindings:export` 重导出入库；`bindings.ts` 含 `codeStats` 包装与 `CodeStatsReport` 系 camelCase 类型；前端唯一调用面为生成绑定，无裸 `invoke('code_stats')` |
| AC-3（汇总面 + 语言占比表） | `code_stats_inner` 单次解析产出汇总四项与逐语言行（代码行降序）；`stats-summary.tsx` / `language-table.tsx` 呈现，占比以 `ui/progress` 承载（`value={share}`，无内联 `style width`——该组件以 transform 实现）；多语言 fixture 的呈现断言由测试阶段承接 |
| AC-4（深度默认 5、调 N 恰发一次 `depth=N` 解析、树面收缩） | `useCodeStats` 以 `[root, depth, tick]` 为 effect 依赖，`setDepth(N)` 即以新 depth 发起一次 invoke；深度控件原生 `<select>` 1–10 默认 5；深度为聚合截断参数，树面层数随 depth 收缩（PoC 结论：汇总 / 语言面数字不随 depth 变化，AC 只约束树面） |
| AC-5（树可展开折叠、折叠态子树不渲染 DOM） | `dir-tree.tsx` per-path 本地 state 展开 / 折叠，折叠节点**不渲染子树 DOM**；默认展开前 2 级；树上数据为单次解析结果的聚合投影，展开交互零新增 invoke |
| AC-6（无缓存 / 无 watch / 无轮询，动作均发起新 invoke） | 命令无 State、无 memo、无静态缓存；store 零触碰（无新注册模型）；hook 无轮询 / 订阅，进入页面、点刷新、调深度各自触发一次完整 invoke（显式刷新模型，沿用 `disabled={loading}`） |
| AC-7（reject inline 持久 + 空态） | `Err` 经 Promise reject 落 `error` 态，`info-view.tsx` 以 `data-testid="info-error"` inline 持久呈现（错误呈现双轨查询轨，无 toast 顶替）；解析结果为空（无被识别文件）呈现 `data-testid="info-empty"` 空态而非错误（root 有效性检查在前，空 report 只能来自空目录） |
| AC-8（管线与纪律） | 守线阶段仅静态检查：`bindings:check` 零 diff、`client:check`（tsc + knip，零新增豁免条目）、`server:check`（fmt + clippy）、`max-lines-per-function: 50`（InfoView 拆分四组件）、data-testid 纪律、新增 Rust 测试无 tokei 自身计数语义断言（测试面归 test-design / test-gen 阶段）；proposal AC 中的前端 / Rust 测试执行由 test-execution 阶段满足 |

## 待决问题

- ~~tokei 精确版本~~——已闭环：`=13.0.0` + `default-features = false`（PoC 编译验证，crates.io 发布信息核实回填「配置」表）
- ~~深度控件形态与值域上限~~——已闭环：原生 `<select>`，1–10，默认 5（见「设计定夺」）
- ~~目录树默认展开层级；占比口径~~——已闭环：默认展开前 2 级；占比 = 代码行份额（百分点 0–100）（见「设计定夺」）
- ~~空白 root 的防御语义~~——已闭环：blank / 不存在 / 不可读 / 非目录 root 统一 `Err`（`metadata` 有效性检查单点；`list_changes` blank → 空结果先例不适用本命令，spec 反向裁定）（见「设计定夺」）
- ~~无 `.gitignore` 仓库是否需要额外目录排除规则~~——已闭环：不引入（PoC 实测非 git 目录中 ignored 目录计入，属 tokei 既有语义边界；spec 裁定不在命令层发明过滤规则，边界留档「PoC 前置门」）
- 无未决项；树面路线（Report 自聚合）由 PoC 定案，无停线项。
