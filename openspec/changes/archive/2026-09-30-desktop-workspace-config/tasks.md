# 任务: desktop-workspace-config

> 依赖排序：阶段一（foundation 常量组收口 + 唯一消费点随改）→ 阶段二（core/config crate：读 + 校验 + 默认值）→ 阶段三（命令轨道 + 绑定出线）→ 阶段四（前端取数 hook 与配置页呈现面、路由入口）→ 阶段五（版本收尾与守线，静态）。
> 无 PoC 前置门：本变更零新增外部依赖（serde / serde_json / specta 均为既有 workspace 依赖），无版本选型与行为存疑点；passthrough 出线形态已由 design 以既有出线先例裁定（`Vec<ConfigExtraField>`，非 flatten 非裸 map）。
> 测试编写与测试执行不在本列表：proposal「测试文件」节所列八组（config crate 校验矩阵 / 命令 mod_test / `layout_test.rs` 常量组断言与 `config.json` 扫描扩展 / `bindings_test.rs` 23→24 扩面 / use-workspace-config.test / config-view.test / app-sidebar.test 扩面 / app.test 扩面）归 test-design / test-gen / test-execution 阶段承接，验收判据中的测试执行由 test-execution 阶段满足；守线阶段仅静态检查。

## 阶段一：foundation 常量组与消费点（Rust 半，先收口字面量）

- [x] `packages/desktop/src-tauri/crates/core/foundation/src/layout.rs`：模块顶部设常量组五员——`pub const DOMAIN_DIR_NAME: &str` / `pub const CHANGES_DIR_NAME: &str` / `pub const ARCHIVE_DIR_NAME: &str` / `pub const EXPLORES_DIR_NAME: &str` / `pub const CONFIG_FILE_NAME: &str`（字面量仅此一处；常量名不含 `openspec` 字样；`ARCHIVE_DIR_NAME` doc 注明相对 `changes` 锚定；`DOMAIN_DIR_NAME` doc 迁入原 fn 的「全包唯一磁盘字面量触点 + 裸名消费通道」不变量）
- [x] `layout.rs`：`resolve()` 体内 `changes` / `archive` / `explores` 行内字面量全量改常量引用（`changes_root` / `explores_root` 经 `CHANGES_DIR_NAME` / `EXPLORES_DIR_NAME`，`archive_root` 经 `changes_root.join(ARCHIVE_DIR_NAME)`），签名与行为零变化
- [x] `layout.rs`：新增 `pub fn config_path(root: &Path) -> PathBuf`——`root.join(DOMAIN_DIR_NAME).join(CONFIG_FILE_NAME)` 纯拼接、无文件系统访问；模块 doc 更新为「常量组 + resolve / config_path」口径
- [x] `layout.rs`：删除 `pub fn domain_dir_name()`（fn 形态收敛为 `DOMAIN_DIR_NAME` 常量，不留 fn 包装；`Layout` 结构体零改动）
- [x] `packages/desktop/src-tauri/src/commands/stats/mod.rs`：唯一消费点随改——`use foundation::layout::domain_dir_name;` → `use foundation::layout::DOMAIN_DIR_NAME;`，`get_statistics(&[root], &[domain_dir_name()], ...)` → `&[DOMAIN_DIR_NAME]`；doc 注释内 `foundation::layout::domain_dir_name` 指称同步；统计语义零变化
- [x] 阶段核对（静态）：`cargo build --manifest-path packages/desktop/src-tauri/Cargo.toml` 通过——全包除 `layout.rs` 外无 `openspec` 与 `config.json` 字面量（产品 `.rs` 口径），既有 `layout_test` 扫描不因常量组改动而炸

## 阶段二：core/config crate（读 + 校验 + 默认值）

- [x] `packages/desktop/src-tauri/Cargo.toml`：`[workspace.members]` 增 `crates/core/config`；`[workspace.dependencies]` 增 `config = { path = "crates/core/config" }`；根包 `[dependencies]` 增 `config = { workspace = true }`（既有 crate 零新增 `→ config` 边；`Cargo.lock` 机械伴生入库）
- [x] `packages/desktop/src-tauri/crates/core/config/Cargo.toml`（新）：裸名 `config`；依赖仅 `foundation` + `serde` / `serde_json` / `specta`（全走 `{ workspace = true }`）——无 Tauri、无 workflow / agent / infra 依赖
- [x] `packages/desktop/src-tauri/crates/core/config/src/lib.rs`（新）：DTO 全家按 design「类型定义」落地——`WorkspaceConfig`（含 `schema_ref` 的 `#[serde(rename = "$schema")]`、`extra: Vec<ConfigExtraField>`）+ `TestSuite` / `TestFramework`（八变体逐个显式 `#[serde(rename)]`：`vite-plus` / `node-test` 等）/ `CoverageThresholds` / `MutationConfig` / `RulesConfig` / `WriteProtection` / `WriteProtectionFile` / `ConfigExtraField`，derive `Debug, Clone, Serialize, specta::Type` + `#[serde(rename_all = "camelCase")]`（只序列化方向，无 `Deserialize`）
- [x] `config/src/lib.rs`：`DiagnosticKind`（五值：`FileMissing` / `ReadFailed` / `JsonInvalid` / `InvalidValue` / `DefaultApplied`，camelCase 出线）+ `ConfigDiagnostic { kind, path, message }`（path 点路径口径：文件级 `"$"`，suite 内形如 `tests[0].coverage.lines`；message 中文含违例原值与所落默认值）+ `ConfigReport { config, diagnostics }`
- [x] `config/src/lib.rs`：`WorkspaceConfig::default()` 全默认基线——`schema` 为 `"spec-driven"`、`tests` 为 `[]`、coverage 80 / 70 / 75、mutation score 70、suite `cwd` 为 `"."`（对齐 CLI `defaults.ts`；供文件级四分支复用）
- [x] `config/src/lib.rs`：`assemble` 组装校验纯函数（`&serde_json::Map<String, Value> -> (WorkspaceConfig, Vec<ConfigDiagnostic>)`）——design「配置语义移植对照」表逐行落地：合法字段原样保留、违例字段按行处置（有默认者吃默认 + `InvalidValue`、suite 必需字段（root / framework）非法剔除整个 suite、无默认可选字段类型非法视为未设）、缺失 prefault 字段逐字段 `DefaultApplied`、顶层未知键收进 `extra`（key + 原文 `Value`）、自洽不变量成立（产出 config 重序列化回 `Value` 再 `assemble` 零诊断）
- [x] `config/src/lib.rs`：`pub fn load(root: &Path) -> ConfigReport` 入口——路径经 `foundation::layout::config_path` 取得（crate 内零路径拼接、零配置文件名字面量）；四条文件级分支：缺失 → 全默认 + `FileMissing`、读取失败 → 全默认 + `ReadFailed`、语法非法 → 全默认 + `JsonInvalid`、顶层非对象 → 全默认 + `InvalidValue`（path `"$"`）；任意输入不失败收场；不校验 root 有效性（命令层职责）；crate doc 注明能力指针 `specs/desktop-workspace-config/spec.md`（路径相对域根，措辞遵循命名隔离纪律）
- [x] 阶段核对（静态）：`cargo build --manifest-path packages/desktop/src-tauri/Cargo.toml` 通过；`config` crate 依赖面白名单复核（`Cargo.toml` 仅四项）

## 阶段三：命令轨道与绑定出线

- [x] `packages/desktop/src-tauri/src/commands/config/mod.rs`（新轨道）：线面 DTO `WorkspaceConfigReport { config, diagnostics }`（`specta::Type` + camelCase；缺文件标记 = diagnostics 内 `FileMissing` 条目，不设布尔字段）
- [x] `commands/config/mod.rs`：`pub fn workspace_config_inner(root: &Path) -> Result<WorkspaceConfigReport, String>`——`fs::metadata` 有效性检查为 Err 通道唯一来源（缺失 / 不可读 / 非目录 / 空白 → `Err`，对齐 code_stats 裁定；MUST NOT panic、MUST NOT 静默空报告）→ `config::load(root)` → `ConfigReport` → `WorkspaceConfigReport` 字段平移；config.json 缺失不是 `Err`
- [x] `commands/config/mod.rs`：`#[tauri::command] #[specta::specta] pub fn workspace_config(root: String) -> Result<WorkspaceConfigReport, String>` 三件事薄包装（`String → &Path` 转换 → `workspace_config_inner` 调用 → `Err` 透传；sync 形态，零 async 声明、零 State、零缓存、不落库）；轨道 doc 注明能力指针（不出现配置文件名字面量）
- [x] `packages/desktop/src-tauri/src/commands/mod.rs`：`pub mod config;` + 轨道清单 doc 注释六轨 → 七轨
- [x] `packages/desktop/src-tauri/src/bindings.rs`：`collect_commands!` 增 `crate::commands::config::workspace_config`（第 24 条），builder doc 注释命令总数 23 → 24；错误通道（`ErrorHandlingMode::Throw`）、`semantic_types`、bigint cast 不动（命令清单实际落于 `commands/mod.rs` `all_commands!` 宏——运行时 `generate_handler` 与 `collect_commands` 同源注入面）
- [x] 执行 `pnpm -C packages/desktop run bindings:export` 重导出，`packages/desktop/src/types/generated/bindings.ts` 生成物入库（不手改）：核对含 `workspaceConfig` 包装与 `WorkspaceConfigReport` / `WorkspaceConfig` / `TestSuite` / `TestFramework` / `CoverageThresholds` / `MutationConfig` / `RulesConfig` / `WriteProtection` / `WriteProtectionFile` / `ConfigExtraField` / `ConfigDiagnostic` / `DiagnosticKind` camelCase 镜像
- [x] 阶段核对（静态）：`pnpm -C packages/desktop run bindings:check` 零 diff（生成物确定性 / 幂等）——连续两次导出逐字节一致；脚本的 `git diff --exit-code` 形态在生成物入库（commit）前必然非零，属变更期的预期状态

## 阶段四：前端取数 hook 与配置页呈现面、路由入口

- [x] `packages/desktop/src/views/config/hooks/use-workspace-config.ts`（新）：`useWorkspaceConfig(root: string): WorkspaceConfigState`——effect 依赖 `[root, tick]` 显式刷新模型，经 `commands.workspaceConfig` typed 调用（禁裸 invoke）；`refresh` 触发重取、`loading` 贯穿请求全程（供刷新钮 `disabled={loading}`）；`cancelled` 防串轮 + 数据归属 root 标记（切换工作区过渡轮不呈现旧根报告，`use-code-stats` 抑制哲学）；失败落 `error` inline 持久；无轮询、无 watch、无事件订阅、无缓存
- [x] `packages/desktop/src/views/config/components/diagnostics-section.tsx`（新）：警示区单列，逐条 kind 视觉分级（`InvalidValue` 强调 / `DefaultApplied` 弱化），展示 path + message；`data-testid="config-diagnostics"` + 条目级 `data-testid="config-diagnostic-item"`；diagnostics 为空整区不渲染
- [x] `packages/desktop/src/views/config/components/basic-config-section.tsx`（新）：`$schema` / `schema` / `context` / `static_analysis` / `rules` 分区；`schema` 等无文件值字段按 `defaultedPaths` 标注「未设（默认 N）」；`data-testid="config-basic"`
- [x] `packages/desktop/src/views/config/components/tests-section.tsx`（新）：逐 suite 卡片呈现 root / framework / cwd / config / includes / excludes / coverage（lines / branches / functions）/ mutation（cwd / score）；未设阈值按 `defaultedPaths`（path 对位 `tests[i].coverage.lines` 等）标注「未设（默认 N）」，与文件显式设值形态可区分；`tests` 空数组呈现弱化空行；`data-testid="config-tests"` + suite 级 `data-testid="config-suite"`
- [x] `packages/desktop/src/views/config/components/write-protection-section.tsx`（新）：逐条 glob + reason；未配置呈现弱化「未配置」占位；`data-testid="config-write-protection"`
- [x] `packages/desktop/src/views/config/components/extra-fields-section.tsx`（新）：`extra` 逐条 key + JSON 预览（`JSON.stringify(value, null, 2)` 等宽块）；`data-testid="config-extra"`；`extra` 为空整区不渲染
- [x] `packages/desktop/src/views/config/config-view.tsx`（新）：`ConfigView({ root }: { root: string })` 页面骨架——页头（标题 + 刷新钮 `data-testid="config-refresh"`，`disabled={loading}`）+ 状态面（inline 持久错误 `data-testid="config-error"`：命令 reject 与文件级 `ReadFailed` / `JsonInvalid` 双来源，无 toast 顶替；loading 行 `data-testid="config-loading"`；空态 `data-testid="config-empty"`：diagnostics 含 `FileMissing` 时呈现、文案说明默认值行为）+ `defaultedPaths` 派生（`kind === 'defaultApplied'` 的 path `Set`）+ 五分区编排（警示区置顶 → 基础配置 → tests → write_protection → 未知字段）；只读（无任何编辑 / 保存 / 删除入口）；页面根 `data-testid="config-view"`；函数体拆分子组件守 `max-lines-per-function: 50`
- [x] `packages/desktop/src/routes.tsx`：`<Route path="/config" element={<ConfigView root={root} />} />`（位于 `/info` 之后、`*` 兜底前）；import `ConfigView`；既有路由项与未知路径兜底不动
- [x] `packages/desktop/src/components/app-sidebar.tsx`：「页面」组末位增 [配置] NavLink（lucide `Settings` 图标，`to="/config"`，`data-testid="nav-config"`，`isActive={pathname === "/config"}`）；组件 doc 注释三项 → 四项；既有 `nav-info` / `nav-changes` / `nav-explores` / `nav-agent` / `nav-db` 挂钩与语义不动

## 阶段五：版本收尾与守线（静态，不含测试执行）

- [x] `packages/desktop/package.json`：`version` 0.3.8 → 0.3.9
- [x] 变更清单复核：design.md 变更清单各条目与实际落地文件一致；机械伴生 `packages/desktop/src-tauri/Cargo.lock` 入库；测试文件（proposal「测试文件」节八组 + 既有 `bindings_test.rs` 的 23→24 扩面）确认未在本变更实现面落地，归 test-design / test-gen 阶段
- [x] `pnpm -C packages/desktop run client:check` 全绿（tsc + knip，零新增豁免条目）
- [x] `pnpm -C packages/desktop run server:check` 全绿（cargo fmt + clippy）
- [x] 全链复审（静态）：全包产品 `.rs` 除 `layout.rs` 外无 `openspec` 与 `config.json` 字面量；crate 图机械保证 `config` 无 Tauri、不依赖 workflow / agent / infra、既有 crate 零 `→ config` 边；无 store 模型注册、无 db 触碰、无内存 / 持久缓存、无文件 watch、无轮询、无事件订阅；`crates/core/workflow` / `crates/core/agent` / `crates/infra/**` 与 `tests/golden/**` 零 diff；前端全链无写回 config.json 的操作入口、无裸 `invoke('workspace_config')`
- [x] 测试执行归 test-execution 阶段承接（本阶段仅静态检查，不含任何测试运行）
