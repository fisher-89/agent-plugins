# 提案: desktop-workspace-code-stats

> **变更**: desktop-workspace-code-stats
> **日期**: 2026-09-29
> **状态**: proposed

---

## 问题

desktop 壳内的全部页面（变更 / 探索 / Agent 调试 / 数据库）都围绕 openspec 记录或系统内部状态，没有一处面向「工作区本身」的视图：用户想了解当前打开项目的代码规模与语言构成，只能退出应用手动跑 tokei / cloc。诉求具体化为三件：

1. **每个工作区一处「基础信息」页**：代码量、语言占比一目了然；
2. **目录维度可下钻**：目录树可展开 / 折叠，看清代码在目录上的分布；
3. **解析深度可调**：默认解析 5 级目录，可调节，平衡信息量与耗时。

现状盘点（构成空白的证据）：

- Rust 侧对工作区文件系统的既有访问全部是**单层** `fs::read_dir`（change 清单扫描、explore 清单扫描，仅跳过点前缀目录），无任何递归遍历、无 `.gitignore` 语义、无 tree 构建先例；
- tokei 不在 `packages/desktop/src-tauri` 任何依赖与 lock 中；
- 前端无 tree / collapsible 类组件，最接近的只有 db-inspector 的单条记录展开（`openIndex` state）。

---

## 提案

当前工作区根上新增「基础信息」页（路由 `/info`，侧栏「页面」组新入口），后端以 **tokei library**（非 CLI 子进程）解析工作区目录，**一次解析**产出三个呈现面，**不做缓存**（用户裁决）：

```
前端 /info 页                       后端 commands/stats（新轨道）
┌──────────────────────────┐   invoke   ┌─────────────────────────────┐
│ 汇总面 文件/代码/注释/空行  │ ────────▶ │ code_stats(root, depth)     │
│ 语言占比表（降序+占比条）    │ ◀──────── │  └─ code_stats_inner 纯函数  │
│ 目录树（展开/折叠，≤depth） │  三面 DTO  │      └─ tokei library       │
│ 深度控件（默认 5）+ 刷新    │           │  无状态 · 不落库 · 无缓存      │
└──────────────────────────┘           └─────────────────────────────┘
```

四个动作：

1. **解析命令**：`commands/stats/` 新轨道，`code_stats(root, depth) -> Result<CodeStatsReport, String>`——无状态薄包装（三件事纪律），领域组装收在 `code_stats_inner` 纯函数（`*_inner` app 层微形态先例）；tokei 依赖仅落 desktop-app 并精确 pin。DTO（汇总 / 语言行 / 目录树节点）`specta::Type` + camelCase 出线，经既有 bindings 重导出管线供前端 typed 调用。sync 命令形态沿用现状惯例（sync 命令不在主线程执行）；MUST NOT 注册 store 模型、MUST NOT 落库——无缓存承诺的另一半。
2. **基础信息页**：`/info` 路由 + 「页面」组 [基础信息] NavLink（`nav-info`，active 由 URL 派生）；仅壳态可达（欢迎态无壳无入口）；取数收在 `use-code-stats` hook（显式刷新模型，查询失败 inline 持久，沿用错误呈现双轨查询轨）。
3. **呈现面**：汇总面（文件数 / 代码行 / 注释行 / 空行）；语言占比面（语言 | 文件数 | 代码 | 注释 | 空行 | 占比，按代码行降序，占比以 Progress 承载）；目录树面（按 depth 呈现至多 N 级目录节点，节点含聚合统计，可展开 / 折叠，**折叠态子树不渲染 DOM**——不引虚拟化依赖的渲染面收敛手段）。
4. **深度可调 + 显式刷新**：depth 默认 5，页面提供调节控件（值域与控件形态 design 定夺）；调节后以新深度重新发起解析；刷新入口沿用 `disabled={loading}` 语义。MUST NOT 引入轮询 / watch / 内存或持久缓存。

**范围裁定**：

- 语言识别、遍历、`.gitignore` 尊重、hidden 跳过等语义**直接依赖 tokei 既有行为**，MUST NOT 在命令层重写识别规则；`Config::depth` 的实际聚合行为在 dev-design 前以最小样例 **PoC 先行留档**（specta PoC 先例），不符预期则退 Report 自聚合备选。
- 解析域逻辑**暂不下沉 core crate**：单消费者不预建（foundation「第二消费者才下沉」、`infra/fs`「不预建」同款纪律），升级缝由 `*_inner` 微形态保留——第二消费者或 CLI 复用出现时平移函数与 DTO 升 crate。
- 测试只覆盖**自研组装层**（inner 聚合 / DTO 映射 / 排序 / 树构建 / 前端交互），MUST NOT 逐项验证 tokei 自身计数语义。

---

## 能力

### 新增能力

- **desktop-workspace-code-stats** — 工作区基础信息页：`/info` 路由与「页面」组入口（仅壳态可达）；`commands/stats` 轨道 `code_stats` 解析命令（tokei library、depth 默认 5、一次解析出汇总 / 语言占比 / 目录树三面 DTO）；页面呈现面（汇总、语言占比表、可展开折叠目录树、深度调节、显式刷新）；无缓存、无轮询、不落库。

### 修改的能力

- **desktop-page-routing** — 路由表增 `/info → 工作区基础信息页`；「页面」组组成由 [变更] [探索] 两项扩为 [基础信息] [变更] [探索] 三项；测试挂钩新增 `nav-info`（既有 `nav-changes` / `nav-agent` / `nav-explores` 不变）。

---

## 变更范围

### 实现文件

- `packages/desktop/src-tauri/src/commands/stats/`（新轨道）：`mod.rs`（`code_stats` 命令薄包装 + `code_stats_inner` 纯函数 + DTO `CodeStatsReport` / `CodeTotals` / `LanguageStats` / `DirNode`，`specta::Type` + `serde(rename_all = "camelCase")`）
- `packages/desktop/src-tauri/src/bindings.rs`：`collect_commands!` 注册 `code_stats`
- `packages/desktop/src-tauri/Cargo.toml`：tokei 依赖精确 pin（workspace.dependencies 收敛）
- `packages/desktop/src/types/generated/bindings.ts`：重导出生成物（生成物不手改）
- `packages/desktop/src/routes.tsx`：`/info` 路由项
- `packages/desktop/src/components/app-sidebar.tsx`：「页面」组新增 [基础信息] NavLink（`data-testid="nav-info"`）
- `packages/desktop/src/views/info/`（新）：`info-view.tsx`、`components/`（汇总面 / 语言占比表 / 目录树）、`hooks/use-code-stats.ts`
- `packages/desktop/package.json`：desktop 版本 bump

### 测试文件

- `packages/desktop/src-tauri/src/commands/stats/mod_test.rs`（新）：thin-wrapper 结构证明（serde_json 对照 inner）；tempdir fixture 的组装断言（深度截断、语言排序、树聚合、Err 路径）——不含 tokei 自身计数语义断言
- `packages/desktop/src/views/info/hooks/use-code-stats.test.ts`（新）：invoke mock；取数 / 调深重取 / 失败 inline
- `packages/desktop/src/views/info/info-view.test.tsx`（新）：渲染面 / 深度控件 / 树展开折叠（折叠态子树不渲染）/ 空态 / inline 错误，data-testid 挂钩
- `packages/desktop/src/components/app-sidebar.test.tsx`：`nav-info` 渲染与 active 态
- `packages/desktop/src/app.test.tsx`：`/info` 路由可达与欢迎态隔离用例

### 删除文件

- 无

### 不要修改

- `packages/desktop/src-tauri/crates/`（core / infra 全部 crate 零改动——不新增 crate、不改依赖方向规则）
- store：不注册新模型、不触碰 db 文件（无缓存即无新落库维度）
- 既有命令轨道（queries / exec / explores / watch / workspaces / db）语义与既有测试挂钩（`nav-changes` / `nav-agent` / `nav-explores` / `nav-db`）
- desktop-app-shell 取数模型：不引入文件 watch、轮询、事件订阅（基础信息页无任何推送例外）
- desktop-page-routing 既有路由项与未知路径兜底语义（`*` → `/changes`）
- `tests/golden/**` 与 workflow 域 golden 契约（本变更不触 workflow crate）

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | `routes.tsx` + `app-sidebar.tsx` | 壳态下「页面」组出现「基础信息」项（`data-testid="nav-info"`），点击后 URL 为 `/info` 并渲染基础信息页；欢迎态（root 为 null）无壳无该入口 |
| AC-2 | `commands/stats/` + `bindings.rs` | `code_stats` 经 `collect_commands!` 注册；重导出后 `bindings.ts` 含 `codeStats` 包装与 `CodeStatsReport` 系类型；前端经 `commands.codeStats` typed 调用，无裸 `invoke('code_stats')` |
| AC-3 | `info-view.tsx` + `code_stats_inner` | 多语言 tempdir fixture 解析后，页面呈现汇总（文件 / 代码 / 注释 / 空行）与语言表（按代码行降序、占比以 Progress 承载、无内联 `style width`） |
| AC-4 | 深度控件 + `use-code-stats` | 深度默认 5；改为 N 后恰以 `depth=N` 重新发起一次解析，目录树呈现深度随之收缩 |
| AC-5 | 目录树组件 | 深度内目录节点可逐级展开 / 折叠；折叠节点子树不渲染 DOM（测试以子行存在性断言） |
| AC-6 | 全链审查 | 无 store 模型注册、无内存 / 持久缓存、无 watch、无轮询；每次进入页面、点刷新、调深度均发起新的 invoke |
| AC-7 | 错误与空态 | `code_stats` reject 时页面 inline 持久呈现错误（testid 承载）且无 toast 顶替；解析结果为空时呈现空态而非错误 |
| AC-8 | 管线 | `pnpm -C packages/desktop run client:check`、`run test`、`cargo test --workspace` 全绿；新增 Rust 测试无 tokei 自身计数语义断言；knip / lint 无新增豁免条目 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 大仓库解析秒级耗时且无缓存，每次刷新都全量重来 | 刷新等待感明显 | 中 | sync 命令不在主线程执行（现状惯例），loading 态 + 刷新钮 `disabled={loading}`；缓存 / 流式留待真实痛点出现（本变更已裁定不做） |
| 无 `.gitignore` 的仓库把 `node_modules` / `target` 等计入 | 计数偏大、解析变慢 | 中 | 先依赖 tokei 既有 ignore / hidden 语义；design 阶段 PoC 以极端样本实测；确需补充排除规则时由后续变更裁定，不在本变更发明遍历规则 |
| tokei `Config::depth` 的目录聚合行为与预期不符 | 树面实现返工 | 低 | dev-design 前 PoC 先行留档；备选方案为基于 tokei Report 列表按目录前缀自聚合 |
| tokei 依赖树较重拉长编译时间 | 构建变慢 | 低 | 仅 desktop-app 单点依赖，接受一次性成本 |
| 深层大树行数过多造成渲染卡顿 | 页面卡顿 | 中 | 折叠态子树按需渲染收敛 DOM；默认展开层级由 design 保守取值 |
| 深度被调至过大值退化为近全量解析 | 解析变慢 | 低 | 控件限定值域（上限 design 定夺），默认 5 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| tokei 以 library 还是 CLI 子进程接入 | library（`tokei` crate 直依赖） | 同进程类型化结果、免外部安装、无文本输出口径漂移 | spawn tokei CLI（要求用户装机 + 解析人读输出，弃） |
| 解析逻辑落位 | desktop-app `commands/stats` 内 `code_stats_inner` 纯函数（app 层微形态） | 单消费者不预建 crate（foundation 第二消费者才下沉、`infra/fs` 不预建先例）；`*_inner` 平移先例保留升级缝 | 新 core crate（如 `codestats`）——第二消费者或 CLI 复用出现时再升 |
| 命令轨道 | 新 `commands/stats/` 轨道 | explores / db 先例：域各立轨道；避免稀释 change 域 `queries` 章程 | 并入 `commands/queries`（与 change 域三命令混域，弃） |
| 单命令 vs 多命令 | 单 `code_stats` 一次解析返回三面 DTO | 树面与语言面同源自同一次遍历，分命令即重复解析 | summary / tree 分命令（二次遍历成本，弃） |
| 路由与导航契约承载 | desktop-page-routing 出 MODIFIED delta | 「页面」组 SHALL 组成与路由表枚举都在该能力，只写新能力 spec 会留下冲突 SHALL | 仅新能力 spec 内自述（page-routing 旧 SHALL 冲突，弃） |
| 目录树渲染方案 | 本地 state 展开 / 折叠 + 折叠态按需渲染 | DOM 面收敛，零新增依赖 | react-window 类虚拟化（重依赖，数据量未到，弃） |
| 深度语义实现 | tokei `Config::depth`，PoC 先行验证 | specta PoC 先例：库行为留档 design 再定实现 | Report 按目录前缀自聚合（PoC 不符时的退路） |

### 待决问题

- tokei 精确版本（stable 12.x vs 13 alpha）——design 定夺并精确 pin
- 深度控件形态（不新增重型依赖的前提）与值域上限
- 目录树默认展开层级；占比口径（代码行份额 vs 总行份额）
- 无 `.gitignore` 仓库是否需要额外目录排除规则（先依赖 tokei 默认语义，PoC 结论说话）
- 空白 root 的防御语义（空报告 vs `Err`）——对齐 `list_changes` 先例由 design 定夺

---
