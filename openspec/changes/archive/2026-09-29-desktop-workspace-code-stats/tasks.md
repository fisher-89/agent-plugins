# 任务: desktop-workspace-code-stats

> 依赖排序：阶段一（依赖落位 + 命令轨道与三面组装）→ 阶段二（绑定出线）→ 阶段三（前端取数 hook）→ 阶段四（页面呈现面与路由入口）→ 阶段五（版本收尾与守线，静态）。
> PoC 已于 dev-design 阶段完成并回填 design.md「PoC 前置门」（`Config::depth` 在 12/13 两代均不存在，树面落 Report 自聚合；版本定 `=13.0.0`），实现阶段无需重跑 PoC。
> 测试编写与测试执行不在本列表：proposal「测试文件」节所列各面与既有 `bindings_test.rs` 的 22→23 扩面由 test-design / test-gen 阶段承接，验收判据中的测试执行由 test-execution 阶段满足；守线阶段仅静态检查。

## 阶段一：依赖落位与命令轨道（Rust 半）

- [x] `packages/desktop/src-tauri/Cargo.toml`：`[workspace.dependencies]` 新增 `tokei = { version = "=13.0.0", default-features = false }`；根包 `[dependencies]` 新增 `tokei = { workspace = true }`（MUST NOT 出现在任何 core / infra crate；`crates/` 零改动）
- [x] `packages/desktop/src-tauri/src/commands/stats/mod.rs`（新轨道）：DTO 四型 `CodeStatsReport` / `CodeTotals` / `LanguageStats` / `DirNode`（derive `Debug, Clone, Serialize, specta::Type` + `#[serde(rename_all = "camelCase")]`，字段形状按 design「类型定义」），轨道 doc 注明能力指针 `specs/desktop-workspace-code-stats/spec.md`（路径相对域根，措辞遵循 layout_test 命名隔离纪律）
- [x] `commands/stats/mod.rs`：`code_stats_inner(root: &Path, depth: u32) -> Result<CodeStatsReport, String>` 有效性前置与单次解析——`fs::metadata` 检查（缺失 / 不可读 / 非目录 → `Err`）后以 `Config::default()`（零字段覆写，MUST NOT `from_config_files()`）执行一次 `Languages::get_statistics`，禁止第二次遍历
- [x] `commands/stats/mod.rs`：`code_stats_inner` 汇总面与语言面组装——汇总四项（files / code / comments / blanks）自 per-file 报告合计；语言行按代码行降序（tie 语言名字典序）、`share` 按代码行份额出百分点（Σcode 为 0 时 0.0）
- [x] `commands/stats/mod.rs`：`code_stats_inner` 树面组装——`Language::reports` 逐文件 `strip_prefix(root)` + `components()` 剥离（分隔符归一化为 `/`），目录前缀祖先链逐级累计（子树全量聚合，≤ `depth` 截断），根层直属文件不产生目录节点；`DirNode.children` 按 `name` 字典序
- [x] `commands/stats/mod.rs`：`#[tauri::command] #[specta::specta] pub fn code_stats(root: String, depth: u32) -> Result<CodeStatsReport, String>` 三件事薄包装（`String → &Path` 转换 → `code_stats_inner` 调用 → `Err` 透传；sync 形态，零 async 声明、零 State、零缓存）。注：`#[cfg(test)] mod mod_test;` 声明未随实现加入——`cargo fmt` 无法解析缺失模块会挂 server:check，test-gen 生成 `mod_test.rs` 时须同步补声明
- [x] `packages/desktop/src-tauri/src/commands/mod.rs`：`pub mod stats;` + 轨道清单 doc 注释五轨 → 六轨
- [x] 阶段核对（静态）：`cargo build --manifest-path packages/desktop/src-tauri/Cargo.toml` 通过，全链无 store 模型注册、无 watch、无轮询、无内存 / 持久缓存

## 阶段二：绑定出线

- [x] `packages/desktop/src-tauri/src/bindings.rs`：`collect_commands!` 增 `crate::commands::stats::code_stats`（第 23 条），builder doc 注释命令总数 22 → 23；错误通道（`ErrorHandlingMode::Throw`）、`semantic_types`、bigint cast 不动
- [x] 执行 `pnpm -C packages/desktop run bindings:export` 重导出，`packages/desktop/src/types/generated/bindings.ts` 生成物入库（不手改）：核对含 `codeStats` 包装与 `CodeStatsReport` / `CodeTotals` / `LanguageStats` / `DirNode` camelCase 镜像
- [x] 阶段核对（静态）：`pnpm -C packages/desktop run bindings:check` 零 diff（生成物确定性 / 幂等）——连续两次导出逐字节一致已核实；脚本的 `git diff --exit-code` 形态在生成物入库（commit）前必然非零，属变更期的预期状态

## 阶段三：前端取数 hook

- [x] `packages/desktop/src/views/info/hooks/use-code-stats.ts`（新）：`useCodeStats(root: string): CodeStatsState`——effect 依赖 `[root, depth, tick]` 显式刷新模型，经 `commands.codeStats` typed 调用（禁裸 invoke）；`cancelled` 防串轮 + 数据归属 root 标记（切换工作区过渡轮不呈现旧根报告）；失败落 `error` inline 持久；无轮询、无 watch、无缓存
- [x] `use-code-stats.ts`：`depth` 默认 5、`setDepth` / `refresh` 触发重取；`loading` 贯穿请求全程（供刷新钮 `disabled={loading}`）

## 阶段四：页面呈现面与路由入口

- [x] `packages/desktop/src/views/info/components/stats-summary.tsx`（新）：汇总面四项总量，`data-testid="info-summary"`
- [x] `packages/desktop/src/views/info/components/language-table.tsx`（新）：`ui/table` 语言占比表（语言 \| 文件数 \| 代码 \| 注释 \| 空行 \| 占比），行序即入参序；占比以 `ui/progress` 承载（`value={share}`，无内联 `style width`），行挂钩 `data-testid="info-language-row"`
- [x] `packages/desktop/src/views/info/components/dir-tree.tsx`（新）：递归行组件，目录名 + 聚合统计 + 缩进层级；per-path `Set<string>` 展开 / 折叠本地 state，默认展开前 2 级，**折叠节点子树不渲染 DOM**；展开 / 折叠零新增 invoke；节点行 `data-testid="info-dir-node"` + `data-path`
- [x] `packages/desktop/src/views/info/info-view.tsx`（新）：`InfoView({ root }: { root: string })` 页面骨架——深度控件（原生 `<select>` `data-testid="info-depth-select"`，选项 1–10，默认 5，`onChange` 走 `setDepth`）+ 刷新钮（`data-testid="info-refresh"`，`disabled={loading}`）+ 三面呈现区 + 空态（`data-testid="info-empty"`，`totals.files === 0` 时呈现）+ inline 持久错误（`data-testid="info-error"`，无 toast 顶替）；函数体拆分子组件守 `max-lines-per-function: 50`
- [x] `packages/desktop/src/routes.tsx`：`<Route path="/info" element={<InfoView root={root} />} />`（位于 `*` 兜底前）；既有路由项与未知路径兜底不动
- [x] `packages/desktop/src/components/app-sidebar.tsx`：`PageNavGroup` 组内首位增 [基础信息] NavLink（lucide `Info`，`to="/info"`，`data-testid="nav-info"`，`isActive={pathname === "/info"}`）；既有 `nav-changes` / `nav-agent` / `nav-explores` / `nav-db` 挂钩与语义不动

## 阶段五：版本收尾与守线（静态，不含测试执行）

- [x] `packages/desktop/package.json`：`version` 0.3.6 → 0.3.7
- [x] 变更清单复核：design.md 变更清单各条目与实际落地文件一致；已知机械伴生 `packages/desktop/src-tauri/Cargo.lock` 随依赖 pin 入库；测试文件（proposal「测试文件」节各组 + 既有 `bindings_test.rs` 的 22→23 扩面）确认未在本变更实现面落地，归 test-design / test-gen 阶段
- [x] `pnpm -C packages/desktop run client:check` 全绿（tsc + knip，零新增豁免条目）
- [x] `pnpm -C packages/desktop run server:check` 全绿（cargo fmt + clippy）
- [x] 全链复审（静态）：无 store 模型注册、无 db 触碰、无内存 / 持久缓存、无文件 watch、无轮询、无事件订阅；`crates/` 与 `tests/golden/**` 零 diff；Rust 侧产品代码无 layout_test 禁用字面量
- [x] 测试执行归 test-execution 阶段承接（本阶段仅静态检查，不含任何测试运行）
