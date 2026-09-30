# 设计: desktop-workspace-config

> **变更**: desktop-workspace-config
> **日期**: 2026-09-29

---

## 提案与规格同步状态

- `proposal.md` 与四份 spec delta（`specs/desktop-workspace-config/spec.md` 新增 + `desktop-crate-layout` / `workspace-layout-resolution` / `desktop-page-routing` 修改，均相对域根）已由 proposal 阶段写入并过审（workflow eval pass），本设计不重复其内容，也不把它们列入变更清单与任务。
- proposal「测试文件」节所列八组测试（config crate 校验矩阵 / 命令 mod_test / `layout_test.rs` 扫描扩展与常量组断言 / `bindings_test.rs` 23→24 扩面 / use-workspace-config.test / config-view.test / app-sidebar.test 扩面 / app.test 扩面）归 test-design / test-gen / test-execution 阶段承接，不入本清单与任务列表。
- 本 design 为首轮（无回溯差异表）；无 PoC 前置门——本变更零新增外部依赖（serde / serde_json / specta 均为既有 workspace 依赖），无版本选型与行为存疑点；唯一管线边角（passthrough 出线形态）以「既有出线先例」裁定，见设计定夺 D8。

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| `foundation::layout` 常量组 | 全包唯一磁盘字面量触点：域目录名 + `changes` / `archive` / `explores` 子目录名 + 配置文件名五常量；`resolve()` 全量引用；新增 `config_path()`；`domain_dir_name()` fn 收敛为 `DOMAIN_DIR_NAME` 常量 | `packages/desktop/src-tauri/crates/core/foundation/src/layout.rs`（修改） | 无（刻意零依赖不变） | `std::path` 纯路径推导 |
| `config` core crate（新） | 配置读取 + serde 解析 + 校验 + 默认值填充：`load(root) -> ConfigReport { config, diagnostics }` 信封；校验语义逐条复刻 CLI zod、吞错不复刻（config 输出永远合法）；未来模块唯一配置出口 | `packages/desktop/src-tauri/crates/core/config/`（新：`Cargo.toml` + `src/lib.rs`） | `foundation`（取路径）+ `serde` / `serde_json` / `specta`；MUST NOT 依赖 workflow / agent / infra / Tauri | 纯 Rust + serde，无 IO 之外的任何运行时面 |
| `commands/config` 命令轨道（新） | `workspace_config` 三件事薄包装 + `workspace_config_inner` 纯函数（root 有效性 Err 通道唯一来源 → `config::load` → 信封转换）；无 State / 无缓存 / 不落库 | `packages/desktop/src-tauri/src/commands/config/mod.rs`（新）+ `src/commands/mod.rs`（挂载） | `config` crate、`foundation`（仅路径类型）、serde、specta | 既有 `*_inner` app 层微形态先例（stats 轨道同型） |
| bindings 注册面 | `collect_commands!` 增 `workspace_config`（第 24 条），既有重导出管线（`bindings:export` + `bindings:check` diff 守卫）不变 | `packages/desktop/src-tauri/src/bindings.rs` | tauri-specta、specta-typescript | 既有管线零改型 |
| `/config` 路由与入口 | 路由表增 `/config` 项；「页面」组末位增 [配置] NavLink（`data-testid="nav-config"`，active 由 URL 派生） | `packages/desktop/src/routes.tsx`、`src/components/app-sidebar.tsx` | react-router、lucide-react | 既有路由 / NavLink 模式 |
| 配置页视图域（新） | 只读分区呈现：基础配置 / tests 面板 / write_protection / 未知字段（passthrough）/ diagnostics 警示区；空态 / inline 持久错误 /「未设（默认 N）」标注 | `packages/desktop/src/views/config/`（新：`config-view.tsx` + `components/` 五件） | 生成绑定、`ui/button` | React + Tailwind（无新 UI 依赖） |
| `useWorkspaceConfig` 取数 hook | 显式刷新模型：`[root, tick]` 触发一次 `commands.workspaceConfig` typed 调用，loading / error / 数据 root 归属标记（切换过渡轮抑制） | `packages/desktop/src/views/config/hooks/use-workspace-config.ts`（新） | 生成绑定 `commands.workspaceConfig` | 既有 hooks 模式（`use-code-stats` 同型） |
| 依赖收敛面 | workspace members 增 `crates/core/config`；`[workspace.dependencies]` 增 `config` path 依赖；壳 `[dependencies]` 增 `config` | `packages/desktop/src-tauri/Cargo.toml` | — | workspace 依赖收敛惯例（零新增外部 crate） |

## 设计定夺（proposal 待决问题闭环）

| 待决问题 | 定夺 | 理由 / 被拒备选 |
|----|------|------|
| `config_path()` 形态 | **独立函数** `pub fn config_path(root: &Path) -> PathBuf`，`Layout` 结构零改动（维持三棵目录树字段） | config.json 是单文件而非目录树，`Layout` 的结构语义（三棵子树 + archive 物理嵌套于 changes）不容纳第四字段；独立函数与 `resolve` 同源引用常量组，调用面 `layout::config_path(&root)` 语义自明。被拒：`Layout` 加第四字段（破坏结构语义，既有消费点与 `PartialEq` 比较面无谓扩散） |
| `domain_dir_name` 公开形态 | **删除 fn，导出 `pub const DOMAIN_DIR_NAME: &str`**，不留 fn 包装；唯一消费点 `commands/stats` 改 `use foundation::layout::DOMAIN_DIR_NAME`（`get_statistics` 的 `ignored_directories` 实参 `&[DOMAIN_DIR_NAME]`），语义零变化 | 「收敛为常量」的直接形态；fn 包装是在常量之上的冗余间接层（CLAUDE.md 简单性优先）。常量命名 SCREAMING_SNAKE_CASE（Rust 惯例）且不含 `openspec` 字样，不触隔离扫描。被拒：保留 fn 包装常量（双形态并存，消费点无所适从） |
| 常量组构成 | 五常量全部 `pub const &str`：`DOMAIN_DIR_NAME` / `CHANGES_DIR_NAME` / `ARCHIVE_DIR_NAME` / `EXPLORES_DIR_NAME` / `CONFIG_FILE_NAME`；`resolve` 与 `config_path` 全量引用，体内零行内字面量 | proposal 裁定的完整版常量化一步到位：「改名只改 layout.rs 一处」覆盖域目录、三个子目录与配置文件名；`ARCHIVE_DIR_NAME` 语义为相对 `changes` 的子段（`archive_root = changes_root.join(ARCHIVE_DIR_NAME)`），常量组注释写明锚定基准 |
| DTO 字段形状与 diagnostics 粒度 | **逐字段逐条**：`ConfigDiagnostic { kind, path, message }`，`path` 为点路径（文件级 `"$"`，suite 内形如 `tests[0].coverage.lines`），`kind` 五值枚举 `FileMissing` / `ReadFailed` / `JsonInvalid` / `InvalidValue` / `DefaultApplied`，`message` 为中文人读文案（含违例原值与所落默认值） | 逐字段粒度使页面「未设（默认 N）」标注可直接消费：`DefaultApplied` 条目的 `path` 与分区组件渲染的字段路径精确对位，无需区间推断；五 kind 让前端状态面映射零歧义（`FileMissing` → 空态；`ReadFailed` / `JsonInvalid` → inline 错误；`InvalidValue` / `DefaultApplied` → 警示区条目）。被拒：逐条纯消息串（无 path 无 kind，页面无法对位标注）；`file_present` 布尔顶置字段（与 `FileMissing` 诊断冗余，spec 口径为「报告内标记 = 诊断条目」） |
| 校验失败的 suite 处置 | **剔除整个 suite 并记 diagnostics**（`InvalidValue` 落在 `tests[i].root` / `tests[i].framework`）：必需字段（root / framework）缺失或非法即整 suite 剔除，`tests` 中其余合法 suite 原样保留 | zod 语义下 suite 必需字段失败即该 suite 整体失败，且二者均无默认值可填（发明 root / framework 即偏离「复刻校验」）；「永远合法」承诺由剩余 suite 保序保留 + 逐条诊断归因达成。被拒：降级保留可呈现形态（需要发明必需字段的占位值，校验语义失真） |
| 校验实现方式 | **事后校验组装 pass**：`serde_json` 先解出 `Value`，纯函数 `assemble(&Map<String, Value>) -> (WorkspaceConfig, Vec<ConfigDiagnostic>)` 逐字段判定处置；`WorkspaceConfig` 只序列化出线、从不直接反序列化 | serde 反序列化 fail-fast 且字段级回调（`deserialize_with`）无法自然收集逐字段诊断；CLI zod 语义本身就是「读原文 → 逐项判定 → 逐项处置」，组装 pass 一一对应；passthrough 即 map 剩余键；自洽不变量（任意输入产出的 config 重序列化回 `Value` 再 `assemble` 零诊断）可机械测试。被拒：`deserialize_with` 逐字段（诊断收集面碎片化，错误路径拼装重复劳动） |
| 坏 JSON 的命令通道 | **报告内 fatal diagnostics，命令 `Ok`**：`JsonInvalid` / `ReadFailed` 随默认值填充的合法 config 一并返回，仅无效 root（缺失 / 不可读 / 非目录 / 空白）走 `Err` | spec 场景「缺失与坏 JSON 不失败」与「错误呈现双轨」共同裁定：坏 JSON 是页面 inline 错误态的数据来源而非命令失败；命令 Err 通道与 `code_stats` 裁定对齐（root 有效性 = 调用 bug，文件内容问题 = 报告语义）。两通道下核心均产出合法默认配置，承诺不分叉 |
| passthrough 出线形态 | **命名字段** `extra: Vec<ConfigExtraField { key: String, value: serde_json::Value }>`（非 `#[serde(flatten)]`、非裸 map） | bindings 管线从未出线过 map / 索引签名类型（生成物无 `[key: string]` 先例），代码库唯一 flatten 先例是 enum 内部 tag（`AgentEvent.kind`）；`serde_json::Value` 出线 `unknown` 有语义规则先例、`Vec<T>` 有 `TreeEntry` 先例，`Vec<ConfigExtraField>` 是零管线悬念的组合。只读页面以 JSON 预览呈现，嵌套形态无损保存语义（key → 原文值不变）。被拒：`#[serde(flatten)] BTreeMap<String, Value>`（specta map-flatten 出线形态未经验证，bindings 是严格断言的生成物，不引入边角风险） |
| 命令 sync vs async | **sync `fn`**（沿既有命令惯例，`#[tauri::command]` + `#[specta::specta]`，零 async 声明） | 单个小文件读取 + 纯内存校验，无阻塞风险面；stats 轨道先例（sync 命令不在 UI 主线程执行）；`spawn_blocking` 需为单命令引入 tokio runtime 面（当前 tokio 仅 dev-dependencies 且 features=`["sync"]`），不成比例 |
| `WorkspaceConfigReport` 与 core `ConfigReport` 关系 | core 定义域类型 `ConfigReport { config, diagnostics }`；命令线面 DTO `WorkspaceConfigReport { config, diagnostics }` 定义在 `commands/config`，字段平移转换一行 | 出线类型名守 AC-3（`bindings.ts` 含 `WorkspaceConfigReport` 系类型，specta 只导出命令签名可达类型）；core 域类型与 app 线面分离沿 stats 轨道先例（core 不出线、命令自有 DTO）。缺文件标记 = diagnostics 内 `FileMissing` 条目（spec 口径），不另设布尔 |
| 「页面」组排序与图标 | [基础信息] [变更] [探索] [配置]——[配置] 追加组内末位；lucide `Settings` 图标 | 与 spec 列举顺序一致（spec 行文顺序即基础信息 → 变更 → 探索 → 配置）；追加尾部不扰动既有三项的视觉记忆与测试锚点。被拒：插入 [基础信息] 之后（无功能收益，纯扰动） |
| 分区组件拆分与页内顺序 | 五分区组件：`diagnostics-section`（警示区，单列置顶）→ `basic-config-section`（$schema / schema / context / static_analysis / rules）→ `tests-section`（逐 suite 卡片）→ `write-protection-section` → `extra-fields-section`；页头（标题 + 刷新钮）之下依次为 inline 错误 → loading → 空态 → 分区序列 | 警示区置顶让违例与吃默认项第一屏可见（attention first，与 info 页错误横幅同位逻辑）；五分区与 spec 呈现面清单一一对应，`max-lines-per-function: 50` 由拆分自然满足。各分区数据缺省时以弱化「未配置」占位行呈现（面板在、内容空），与 info 页「空树区省略」反向——配置页的面板在位性本身是信息 |

## 配置语义移植对照（zod → core/config 逐条策略）

校验权威为 `plugins/dev-team/bin/src/schemas/config/config.schema.ts`（zod/v4）+ `defaults.ts`；下表是 `assemble` 组装 pass 的完整规则面（实现与校验矩阵测试均以此为准）。共同前提：任意输入 MUST NOT 使 `load` 失败收场；`InvalidValue` / `DefaultApplied` 均记 `path` 与含默认值的中文 `message`。

| zod 规则（config.schema.ts） | 默认值（defaults.ts） | 缺失处置 | 非法处置 |
|------------------------------|----------------------|----------|----------|
| 顶层非对象（数组 / 标量 / null） | 全默认 | — | 全默认 config + `InvalidValue`（path `"$"`，顶层非对象） |
| `$schema: string().optional()` | 无（None） | None，无诊断 | 非字符串 → None + `InvalidValue` |
| `schema: literal('spec-driven').optional().prefault('spec-driven')` | `"spec-driven"` | 默认 + `DefaultApplied` | 值 ≠ `"spec-driven"` → 默认 + `InvalidValue` |
| `context: string().optional()` | 无 | None，无诊断 | 非字符串 → None + `InvalidValue` |
| `rules: { proposal: string[].optional, tasks: string[].optional }.optional()` | 无 | None，无诊断 | 非对象 → None + `InvalidValue`；子字段非 string[] → 该子字段 None + `InvalidValue`（path 形如 `rules.proposal`） |
| `static_analysis: string().optional()` | 无 | None，无诊断 | 非字符串 → None + `InvalidValue` |
| `tests: array(testSuiteSchema).optional().prefault([])` | `[]` | `[]` + `DefaultApplied`（path `tests`） | 非数组 → `[]` + `InvalidValue`；元素非对象 → 剔除该 suite + `InvalidValue`（path `tests[i]`） |
| suite `root: string().nonempty().refine(无 `*?{[` 通配符)`（必需） | 无 | 缺失 → 剔除整个 suite + `InvalidValue` | 空串或含通配符 → 剔除整个 suite + `InvalidValue`（path `tests[i].root`） |
| suite `framework: 八值枚举`（`jest` / `vitest` / `vite-plus` / `bun` / `rust` / `node-test` / `go` / `pytest`，必需） | 无 | 缺失 → 剔除整个 suite + `InvalidValue` | 枚举外值 → 剔除整个 suite + `InvalidValue`（path `tests[i].framework`） |
| suite `cwd: string().optional().prefault('.')` | `"."` | 默认 + `DefaultApplied` | 非字符串 → 默认 + `InvalidValue` |
| suite `config: string().nonempty().optional()` | 无 | None，无诊断 | 空串或非字符串 → None + `InvalidValue` |
| suite `includes` / `excludes: string[].optional()` | 无 | None，无诊断 | 非数组或元素非字符串 → None + `InvalidValue` |
| suite `coverage: coverageSchema.optional().prefault({})`（对象级） | 对象填充 | 三阈值逐字段默认 + 逐字段 `DefaultApplied` | 非对象 → 三阈值全默认 + `InvalidValue`（path `tests[i].coverage`） |
| coverage `lines` / `branches` / `functions: number().min(0).max(100).optional().prefault(80 / 70 / 75)` | 80 / 70 / 75 | 默认 + 逐字段 `DefaultApplied` | 非数值或越界（<0 或 >100）→ 默认 + 逐字段 `InvalidValue` |
| suite `mutation: mutationConfigSchema.optional().prefault({})`（对象级） | 对象填充 | `cwd` None（无诊断）+ `score` 默认 `DefaultApplied` | 非对象 → 同缺失处置 + `InvalidValue`（path `tests[i].mutation`） |
| mutation `cwd: string().optional()`（无 prefault） | 无 | None，无诊断 | 非字符串 → None + `InvalidValue` |
| mutation `score: number().min(0).max(100).optional().prefault(70)` | 70 | 默认 + `DefaultApplied` | 非数值或越界 → 默认 + `InvalidValue` |
| `write_protection: { files: array(fileRule).optional }.optional()` | 无 | None，无诊断 | 非对象 → None + `InvalidValue`；`files` 非数组 → None + `InvalidValue`（path `write_protection.files`） |
| fileRule `glob: string().nonempty().optional()` | 无 | None，无诊断 | 空串或非字符串 → **剔除该条规则** + `InvalidValue`（path `write_protection.files[i].glob`） |
| fileRule `reason: string().optional()` | 无 | None，无诊断 | 非字符串 → None + `InvalidValue` |
| `.passthrough()`（顶层未知字段） | — | — | 原样保留进 `extra`（`Vec<ConfigExtraField>`，key + 原文 `Value`），无诊断 |
| 文件缺失 | 全默认 | 全默认 config + `FileMissing`（path `"$"`）——**空态来源，非错误** | — |
| 文件读取失败（存在但不可读） | 全默认 | — | 全默认 config + `ReadFailed`（inline 错误来源） |
| JSON 语法非法 | 全默认 | — | 全默认 config + `JsonInvalid`（inline 错误来源） |

自洽不变量（校验矩阵测试的核心判据，归测试阶段落地）：任意输入产出的 `WorkspaceConfig` 经 `serde_json::to_value` 回写后再过同一 `assemble`，diagnostics 必为空——「config 输出永远合法」的机械形态。

## 变更清单

<!-- 以文件为入口逐层展开，实现阶段以此清单为边界。覆盖 proposal「变更范围-实现文件」全部 12 条；测试文件不在本清单（归测试阶段）。 -->

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/src-tauri/crates/core/config/Cargo.toml` | 新 crate manifest（裸名 `config`）：依赖仅 `foundation` + `serde` / `serde_json` / `specta`（全走 `{ workspace = true }`）；无 Tauri、无 workflow / agent / infra 依赖（crate 图机械保证） |
| `packages/desktop/src-tauri/crates/core/config/src/lib.rs` | 配置语义唯一实现：DTO 全家（`WorkspaceConfig` / `TestSuite` / `TestFramework` / `CoverageThresholds` / `MutationConfig` / `RulesConfig` / `WriteProtection` / `WriteProtectionFile` / `ConfigExtraField`，derive `Debug, Clone, Serialize, specta::Type` + `#[serde(rename_all = "camelCase")]`）+ `ConfigDiagnostic` / `DiagnosticKind` / `ConfigReport` + `WorkspaceConfig::default()`（默认值填充基线）+ `assemble` 组装校验纯函数（「配置语义移植对照」表逐行落地）+ `load(root) -> ConfigReport` 入口（路径经 `foundation::layout::config_path` 取得，crate 内零配置文件名字面量）；crate doc 注明能力指针 `specs/desktop-workspace-config/spec.md`（路径相对域根）与「未来模块唯一配置出口」契约 |
| `packages/desktop/src-tauri/src/commands/config/mod.rs` | 配置命令轨道（单文件模块）：线面 DTO `WorkspaceConfigReport`（`specta::Type` + camelCase）+ `workspace_config` 命令薄包装 + `workspace_config_inner` 纯函数（`fs::metadata` root 有效性检查为 Err 通道唯一来源 → `config::load` → 信封字段平移）；轨道 doc 注明能力指针（措辞遵循 layout_test 命名隔离纪律，不出现配置文件名字面量） |
| `packages/desktop/src/views/config/config-view.tsx` | 配置页骨架：页头（标题 + 刷新钮 `data-testid="config-refresh"`，`disabled={loading}`）+ inline 持久错误（`data-testid="config-error"`，命令 reject 与文件级 fatal 诊断双来源）+ loading 行（`data-testid="config-loading"`）+ 空态（`data-testid="config-empty"`，diagnostics 含 `FileMissing` 时呈现，文案说明默认值行为）+ 五分区编排 + diagnostics → `defaultedPaths: ReadonlySet<string>` 派生（`kind === 'defaultApplied'` 的 path 集合，供分区做「未设（默认 N）」标注）；页面根 `data-testid="config-view"`；函数体拆分子组件守 `max-lines-per-function: 50` |
| `packages/desktop/src/views/config/components/diagnostics-section.tsx` | diagnostics 警示区（单列置顶）：逐条 `DiagnosticKind` 视觉分级（`InvalidValue` 强调色 / `DefaultApplied` 弱化），展示 path + message（`data-testid="config-diagnostics"`，条目级 `data-testid="config-diagnostic-item"`）；diagnostics 为空时整区不渲染 |
| `packages/desktop/src/views/config/components/basic-config-section.tsx` | 基础配置分区：`$schema` / `schema` / `context` / `static_analysis` / `rules`（proposal / tasks 列表）；`schema` 与无文件值的字段按 `defaultedPaths` 标注「未设（默认 N）」（`data-testid="config-basic"`） |
| `packages/desktop/src/views/config/components/tests-section.tsx` | tests 面板：逐 suite 卡片呈现 root / framework / cwd / config / includes / excludes / coverage（lines / branches / functions）/ mutation（cwd / score）；未设阈值按 `defaultedPaths`（path 对位 `tests[i].coverage.lines` 等）标注「未设（默认 N）」，与文件显式设值形态可区分；`tests` 为空数组呈现弱化空行（`data-testid="config-tests"`，suite 级 `data-testid="config-suite"`） |
| `packages/desktop/src/views/config/components/write-protection-section.tsx` | write_protection 分区：逐条 glob 规则 + reason；未配置呈现弱化「未配置」占位（`data-testid="config-write-protection"`） |
| `packages/desktop/src/views/config/components/extra-fields-section.tsx` | 未知字段分区：`extra` 逐条 key + JSON 预览（`JSON.stringify(value, null, 2)` 于等宽块），passthrough 可见性即需求本体（`data-testid="config-extra"`）；`extra` 为空时整区不渲染 |
| `packages/desktop/src/views/config/hooks/use-workspace-config.ts` | 取数收口 hook：`useWorkspaceConfig(root: string): WorkspaceConfigState`——effect 依赖 `[root, tick]` 显式刷新模型，经 `commands.workspaceConfig` typed 调用（禁裸 invoke）；`cancelled` 防串轮 + 数据归属 root 标记（切换工作区过渡轮不呈现旧根报告，`use-code-stats` 抑制哲学）；失败落 `error` inline 持久；无轮询、无 watch、无事件订阅、无缓存 |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src-tauri/crates/core/foundation/src/layout.rs` | 模块顶部设五常量组：`pub const DOMAIN_DIR_NAME: &str` / `CHANGES_DIR_NAME: &str` / `ARCHIVE_DIR_NAME: &str` / `EXPLORES_DIR_NAME: &str` / `CONFIG_FILE_NAME: &str`（常量名不含 `openspec` 字样）；`resolve()` 体内 `changes` / `archive` / `explores` 行内字面量全量改常量引用；新增 `config_path(root: &Path) -> PathBuf`（`root.join(DOMAIN_DIR_NAME).join(CONFIG_FILE_NAME)`，纯拼接无 IO）；`domain_dir_name()` fn 删除、由 `DOMAIN_DIR_NAME` 常量承接（doc 迁移至常量，注明「全包唯一磁盘字面量触点 + 裸名消费通道」不变量）；模块 doc 更新为常量组 + 两函数口径 | AC-1 前半：常量组覆盖域目录名 / 三子目录名 / 配置文件名，`resolve` 与 `config_path` 全量引用；`Layout` 结构体签名与字段零改动 |
| `packages/desktop/src-tauri/Cargo.toml` | `[workspace.members]` 增 `crates/core/config`；`[workspace.dependencies]` 增 `config = { path = "crates/core/config" }`；根包 `[dependencies]` 增 `config = { workspace = true }` | workspace 依赖收敛惯例；`config` 仅落 desktop-app 壳（既有 crate 零新增 `→ config` 边，首个真实消费者出现才立边） |
| `packages/desktop/src-tauri/src/commands/mod.rs` | `pub mod config;`；轨道清单 doc 注释六轨 → 七轨 | 既有轨道语义与 doc 纪律（三件事 / `*_inner` 微形态）照抄引用 |
| `packages/desktop/src-tauri/src/bindings.rs` | `collect_commands!` 增 `crate::commands::config::workspace_config`（第 24 条）；builder doc 注释命令总数 23 → 24 | 错误通道（`ErrorHandlingMode::Throw` → Promise reject）、`semantic_types`、bigint cast 全部不动 |
| `packages/desktop/src/types/generated/bindings.ts` | 重导出生成物：`workspaceConfig` 包装 + `WorkspaceConfigReport` / `WorkspaceConfig` / `TestSuite` / `TestFramework` / `CoverageThresholds` / `MutationConfig` / `RulesConfig` / `WriteProtection` / `WriteProtectionFile` / `ConfigExtraField` / `ConfigDiagnostic` / `DiagnosticKind` camelCase TS 镜像 | 生成物不手改：`pnpm -C packages/desktop run bindings:export` 产出入库 |
| `packages/desktop/src-tauri/src/commands/stats/mod.rs` | `use foundation::layout::domain_dir_name;` → `use foundation::layout::DOMAIN_DIR_NAME;`；`get_statistics(&[root], &[domain_dir_name()], ...)` → `&[DOMAIN_DIR_NAME]`；doc 注释内 `foundation::layout::domain_dir_name` 指称同步更新 | fn → const 波及的唯一消费点，统计语义零变化（AC 与 spec 明令「语义零变化」） |
| `packages/desktop/src/routes.tsx` | 路由表增 `<Route path="/config" element={<ConfigView root={root} />} />`（位于 `/info` 之后、`*` 兜底之前）；import `ConfigView` | 既有路由项与未知路径兜底（`*` → `/changes`）不动；壳态才挂 Router 的既有结构不破（欢迎态无壳无入口） |
| `packages/desktop/src/components/app-sidebar.tsx` | `PageNavGroup` 末位增 [配置] NavLink（lucide `Settings` 图标，`to="/config"`，`data-testid="nav-config"`，`isActive={pathname === "/config"}`）；组件 doc 注释三项 → 四项 | 既有 `nav-info` / `nav-changes` / `nav-explores` / `nav-agent` / `nav-db` 挂钩与语义不动 |
| `packages/desktop/package.json` | `version` 0.3.8 → 0.3.9 | 配置页为用户可见变更，版本收尾 bump（archive 时生效） |

<!-- `src-tauri/Cargo.lock` 为 workspace 成员新增的机械伴生，入库但不单列条目。proposal「测试文件」节八组与既有 `bindings_test.rs` 的 23→24 扩面归测试阶段，不在本清单。 -->

### 公共函数 / API

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `DOMAIN_DIR_NAME` | `packages/desktop/src-tauri/crates/core/foundation/src/layout.rs` | 修改（fn `domain_dir_name()` → const） | `pub const DOMAIN_DIR_NAME: &str` | 域目录名裸名消费通道（tokei `ignored_directories` 等），全包唯一磁盘字面量触点不变量迁至常量 doc |
| `CHANGES_DIR_NAME` / `ARCHIVE_DIR_NAME` / `EXPLORES_DIR_NAME` / `CONFIG_FILE_NAME` | 同上 | 新增 | `pub const CHANGES_DIR_NAME: &str`、`pub const ARCHIVE_DIR_NAME: &str`（相对 `changes` 锚定）、`pub const EXPLORES_DIR_NAME: &str`、`pub const CONFIG_FILE_NAME: &str` | 常量组其余四员；改名只改 layout.rs 一处的完整版收口 |
| `resolve` | 同上 | 修改 | `pub fn resolve(root: &Path) -> Layout`（签名不变） | 体内全量常量引用，行为零变化 |
| `config_path` | 同上 | 新增 | `pub fn config_path(root: &Path) -> PathBuf` | `<root>/<DOMAIN_DIR_NAME>/<CONFIG_FILE_NAME>` 纯拼接，无文件系统访问；`config` crate 取路径唯一通道 |
| `load` | `packages/desktop/src-tauri/crates/core/config/src/lib.rs` | 新增 | `pub fn load(root: &Path) -> ConfigReport` | 配置语义唯一入口：`config_path` 取路径 → 缺失（`FileMissing`）/ 读取失败（`ReadFailed`）/ 语法非法（`JsonInvalid`）/ 顶层非对象四分支全走默认值报告 → `assemble` 逐字段组装校验；任意输入不失败收场；不校验 root 有效性（命令层职责） |
| `workspace_config` | `packages/desktop/src-tauri/src/commands/config/mod.rs` | 新增 | `#[tauri::command] #[specta::specta] pub fn workspace_config(root: String) -> Result<WorkspaceConfigReport, String>` | 三件事薄包装：参数转换（`String → &Path`）→ 调用 `workspace_config_inner` → 错误映射（Err 透传 `String`）；sync 形态，零 State、零缓存、不落库 |
| `workspace_config_inner` | 同上 | 新增 | `pub fn workspace_config_inner(root: &Path) -> Result<WorkspaceConfigReport, String>` | `*_inner` app 层微形态（离 Tauri 运行时可测）：`fs::metadata` 有效性检查（缺失 / 不可读 / 非目录 / 空白 → `Err`，对齐 code_stats 裁定）→ `config::load(root)` → `ConfigReport` → `WorkspaceConfigReport` 字段平移；config.json 缺失不是 Err（报告内 `FileMissing` 标记） |
| `ConfigView` | `packages/desktop/src/views/config/config-view.tsx` | 新增 | `export function ConfigView({ root }: { root: string }): React.JSX.Element` | 页面唯一出口组件；壳态 root 非空直用；只读（无任何写回入口） |
| `useWorkspaceConfig` | `packages/desktop/src/views/config/hooks/use-workspace-config.ts` | 新增 | `export function useWorkspaceConfig(root: string): WorkspaceConfigState` | 取数收口：组件不直接 invoke；`refresh` 触发重取，失败落 `error` inline 持久 |
| `DiagnosticsSection` | `packages/desktop/src/views/config/components/diagnostics-section.tsx` | 新增 | `export function DiagnosticsSection({ diagnostics }: { diagnostics: ConfigDiagnostic[] }): React.JSX.Element` | 警示区单列，kind 视觉分级 |
| `BasicConfigSection` | `packages/desktop/src/views/config/components/basic-config-section.tsx` | 新增 | `export function BasicConfigSection({ config, defaultedPaths }: { config: WorkspaceConfig; defaultedPaths: ReadonlySet<string> }): React.JSX.Element` | 基础配置分区 + 「未设（默认 N）」标注 |
| `TestsSection` | `packages/desktop/src/views/config/components/tests-section.tsx` | 新增 | `export function TestsSection({ suites, defaultedPaths }: { suites: TestSuite[]; defaultedPaths: ReadonlySet<string> }): React.JSX.Element` | tests 面板逐 suite 卡片 + 阈值标注 |
| `WriteProtectionSection` | `packages/desktop/src/views/config/components/write-protection-section.tsx` | 新增 | `export function WriteProtectionSection({ protection }: { protection: WriteProtection \| null }): React.JSX.Element` | write_protection 面板，null 时弱化占位 |
| `ExtraFieldsSection` | `packages/desktop/src/views/config/components/extra-fields-section.tsx` | 新增 | `export function ExtraFieldsSection({ extra }: { extra: ConfigExtraField[] }): React.JSX.Element` | passthrough JSON 预览，空时整区不渲染 |
| `commands.workspaceConfig` | `packages/desktop/src/types/generated/bindings.ts` | 新增 | `workspaceConfig: (root: string) => Promise<WorkspaceConfigReport>` | 生成包装（reject 语义）；前端唯一调用面，禁裸 `invoke('workspace_config')` |

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `WorkspaceConfig` | `packages/desktop/src-tauri/crates/core/config/src/lib.rs` | 新增 | 永远合法的完整配置：`$schema`（字段 `schema_ref` + `#[serde(rename = "$schema")]`: `Option<String>`）、`schema: String`（字面量默认 `"spec-driven"`）、`context: Option<String>`、`rules: Option<RulesConfig>`、`static_analysis: Option<String>`、`tests: Vec<TestSuite>`、`write_protection: Option<WriteProtection>`、`extra: Vec<ConfigExtraField>`（passthrough）；实现 `Default`（全默认基线，供四条文件级分支复用）；只序列化方向，无 `Deserialize` 需求 |
| `TestSuite` | 同上 | 新增 | `root: String`、`framework: TestFramework`、`cwd: String`（默认 `"."`）、`config: Option<String>`、`includes: Option<Vec<String>>`、`excludes: Option<Vec<String>>`、`coverage: CoverageThresholds`、`mutation: MutationConfig` |
| `TestFramework` | 同上 | 新增 | 八值枚举 `Jest` / `Vitest` / `VitePlus` / `Bun` / `Rust` / `NodeTest` / `Go` / `Pytest`，逐变体显式 `#[serde(rename = "...")]` 对齐 CLI 串（`vite-plus` / `node-test` 非 rename_all 可表达）；specta 出线字符串字面量联合 |
| `CoverageThresholds` | 同上 | 新增 | `lines` / `branches` / `functions: f64`（zod `number` 忠实浮点；沿 `LanguageStats.share` 先例出线 `number \| null`，前端呈现点 `??` 防御） |
| `MutationConfig` | 同上 | 新增 | `cwd: Option<String>`（无默认）、`score: f64`（默认 70） |
| `RulesConfig` | 同上 | 新增 | `proposal: Option<Vec<String>>`、`tasks: Option<Vec<String>>` |
| `WriteProtection` | 同上 | 新增 | `files: Option<Vec<WriteProtectionFile>>` |
| `WriteProtectionFile` | 同上 | 新增 | `glob: Option<String>`、`reason: Option<String>` |
| `ConfigExtraField` | 同上 | 新增 | passthrough 载体：`key: String`、`value: serde_json::Value`（经既有语义规则出线 `unknown`） |
| `DiagnosticKind` | 同上 | 新增 | 五值枚举 `FileMissing` / `ReadFailed` / `JsonInvalid` / `InvalidValue` / `DefaultApplied`，`rename_all = "camelCase"` 出线字符串联合；前端状态面映射：`FileMissing` → 空态、`ReadFailed` / `JsonInvalid` → inline 错误、`InvalidValue` / `DefaultApplied` → 警示区条目 |
| `ConfigDiagnostic` | 同上 | 新增 | `kind: DiagnosticKind`、`path: String`（点路径，文件级 `"$"`，suite 内形如 `tests[0].coverage.lines`）、`message: String`（中文人读文案，含违例原值与所落默认值） |
| `ConfigReport` | 同上 | 新增 | core 域信封：`config: WorkspaceConfig` + `diagnostics: Vec<ConfigDiagnostic>`；不出线（命令签名不可达） |
| `WorkspaceConfigReport` | `packages/desktop/src-tauri/src/commands/config/mod.rs` | 新增 | 命令线面信封：`config: WorkspaceConfig` + `diagnostics: Vec<ConfigDiagnostic>`（`specta::Type` + camelCase）；缺文件标记 = diagnostics 内 `FileMissing` 条目 |
| `WorkspaceConfigState` | `packages/desktop/src/views/config/hooks/use-workspace-config.ts` | 新增（TS interface） | `data: WorkspaceConfigReport \| null`、`loading: boolean`、`error: string \| null`、`refresh: () => void`（spec `{ data, loading, error, refresh }` 形态） |
| 生成 TS 镜像十二型 | `packages/desktop/src/types/generated/bindings.ts` | 新增 | 上列 Rust DTO 的 camelCase 生成物（不手改） |

### 配置

| 配置键 | 所在文件 | 类型 | 默认值 | 说明 |
|--------|----------|------|--------|------|
| `crates/core/config` | `packages/desktop/src-tauri/Cargo.toml` `[workspace.members]` | 新增 | — | workspace 成员注册（cargo test --workspace 自动覆盖新 crate） |
| `config` | `packages/desktop/src-tauri/Cargo.toml` `[workspace.dependencies]` | 新增 | `{ path = "crates/core/config" }` | workspace 依赖收敛；`foundation` 同款 path 依赖形态 |
| `config` | `packages/desktop/src-tauri/Cargo.toml` 根包 `[dependencies]` | 新增 | `config = { workspace = true }` | 仅 desktop-app 壳引用；既有 crate 零新增 `→ config` 边 |
| `version` | `packages/desktop/package.json` | 修改 | `"0.3.9"` | 版本收尾 bump（配置页用户可见） |

---

## 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `ConfigReport`（core 域信封） | `config: WorkspaceConfig` + `diagnostics: Vec<ConfigDiagnostic>` | `config` 由文件内容逐字段组装（合法字段原样、违例字段吃默认、未知字段进 `extra`）；`diagnostics` 与 `config` 同一次 `assemble` 产出、path 可对位 config 字段 | **无**（进程内出线面，每次调用完整重读重校验；MUST NOT 落库 / 无缓存） |
| `WorkspaceConfig`（出线 DTO 树） | 见「类型定义」 | `tests: Vec<TestSuite>`（保序，违例 suite 剔除）；`ConfigExtraField` 收留顶层未知键 | 无 |
| store 模型 | 无新注册模型 | — | store / db 文件零触碰（无缓存即无新落库维度） |

演进纪律：本变更为新增只读出线面，不触碰既有线格式与 golden 契约（workflow 域 `tests/golden/**` 不在范围）；无落库模型即无版本演进面。

## 路由 / API 设计

<!-- 本变更不涉及 HTTP API。前端路由与 IPC 命令面如下。 -->

| 层 | 方法 / 路径 | 描述 | 输入 | 输出 | 认证 |
|----|------------|------|------|------|------|
| 前端路由 | `GET`（HashRouter）`/config` | 工作区配置页（仅壳态可达；欢迎态无壳无入口） | 路由参数：无（root 由壳态 props 传入，MUST NOT 以 URL 参数承载根） | `ConfigView` 渲染 | — |
| IPC 命令 | `invoke('workspace_config')`（typed：`commands.workspaceConfig`） | 工作区配置单次读取解析，`{ config, diagnostics }` 信封 | `{ root: string }` | `Promise<WorkspaceConfigReport>`（reject 语义，无效 root `Err(String)` 透传；config.json 缺失为 `Ok` + `FileMissing` 诊断） | 本地应用 IPC，无认证面 |

## 依赖

### 运行时依赖

- 零新增外部依赖 — `foundation` / `serde` / `serde_json` / `specta` 均为既有 workspace 依赖；`config` crate 与命令轨全部走 `{ workspace = true }` 引用
- `foundation`（workspace path）— `config` crate 唯一 workspace 内依赖：`config_path()` 提供配置文件路径，crate 内零路径拼接、零文件名字面量
- `serde` / `serde_json` — DTO 序列化出线与原文 `Value` 解析（组装 pass 的输入面）
- `specta` — DTO `Type` derive（出线 TS 镜像的传递来源；命令签名可达类型才出线）
- `lucide-react`（前端既有）— [配置] 项 `Settings` 图标

### 构建/测试依赖

- 无新增 — 生成物管线复用既有 `bindings:export` / `bindings:check`；`crates/core/config` 随 workspace 成员注册自动进入 `cargo test --workspace` 覆盖面（无需逐 crate 注册）

---

## 验收标准对齐

| AC | 设计落点 |
|----|----------|
| AC-1（常量组 + 全量引用 + 双禁令扫描） | `layout.rs` 五常量组 + `resolve` / `config_path` 全量引用 + `domain_dir_name` fn → 常量（设计定夺 D 表）；扫描扩展（`config.json` 禁令、唯一例外 layout.rs）归测试阶段（proposal「测试文件」`layout_test.rs` 组） |
| AC-2（crate 依赖白名单 + load 语义） | `crates/core/config/Cargo.toml` 仅 `foundation` + serde 系 + specta；`load` 四条文件级分支 + `assemble` 逐条策略（「配置语义移植对照」表）；passthrough 保留；缺失以 `FileMissing` 诊断表达；依赖方向由 crate 图机械保证 |
| AC-3（collect_commands! 注册 + typed 调用 + 不落库无缓存） | `bindings.rs` 增 `workspace_config`（第 24 条）；`bindings:export` 重导出入库；`WorkspaceConfigReport` 系 camelCase 出线；前端唯一调用面 `commands.workspaceConfig`；命令零 State / 零 memo / 零落库 |
| AC-4（nav-config 入口 + `/config` 渲染 + 欢迎态隔离） | `app-sidebar.tsx`「页面」组末位 [配置] NavLink（`Settings` 图标、`data-testid="nav-config"`、active 由 URL 派生）；`routes.tsx` 增 `/config` 项；欢迎态（root 为 null）不挂 Router / 侧栏的既有结构不破 |
| AC-5（分区呈现 + 「未设（默认 N）」+ diagnostics 警示区） | 五分区组件与页内顺序（设计定夺 D 表末行）；`defaultedPaths` 派生自 `DefaultApplied` 诊断 path，分区按 path 对位标注；警示区单列置顶、kind 视觉分级 |
| AC-6（显式刷新模型） | `useWorkspaceConfig` effect 依赖 `[root, tick]`：进入页面一次解析、`refresh`（`disabled={loading}`）重取、切换 workspace 以新根重取（root 归属标记抑制过渡轮）；无轮询 / watch / 事件订阅 / 缓存层 |
| AC-7（状态面） | 空态：diagnostics 含 `FileMissing` → `config-empty`（非错误）；inline 持久错误：命令 reject（`error` 态）与文件级 `ReadFailed` / `JsonInvalid` 双来源 → `config-error`（testid 承载、无 toast 顶替）；无效 root `Err` 且 `workspace_config_inner` 无 panic 面（metadata 检查 + 纯组装） |
| AC-8（全链审查） | 全包除 `layout.rs` 外零 `openspec` 与 `config.json` 字面量（常量组 + 字面量禁令双保险，扫描归测试阶段）；crate 图机械保证 `config` 无 Tauri、不依赖 workflow / agent / infra；全链无写命令、无编辑 / 保存 / 删除入口（只读边界） |
| AC-9（管线） | 守线阶段静态检查：`bindings:check` 零 diff、`client:check`（tsc + knip，零新增豁免条目）、`server:check`（fmt + clippy）、`max-lines-per-function: 50`（ConfigView 拆分五组件）、data-testid 纪律；AC 中的测试执行由 test-execution 阶段满足 |

## 待决问题

- ~~`config_path()` 形态~~——已闭环：独立函数，`Layout` 零改动（见「设计定夺」）
- ~~`domain_dir_name` 公开形态与 stats 消费点~~——已闭环：删除 fn、导出 `pub const DOMAIN_DIR_NAME`，唯一消费点 `commands/stats` 改常量引用（见「设计定夺」）
- ~~DTO 字段形状与 diagnostics 粒度；校验失败的 suite 处置~~——已闭环：逐字段逐条（`kind` 五值 + `path` 点路径 + 中文 `message`）；必需字段非法的 suite 整体剔除并记诊断（见「设计定夺」与「配置语义移植对照」）
- ~~校验实现方式~~——已闭环：事后校验组装 pass（`Value` → `assemble` 纯函数），`WorkspaceConfig` 只序列化不反序列化（见「设计定夺」）
- ~~坏 JSON 的命令通道~~——已闭环：报告内 fatal diagnostics（`Ok`），仅无效 root 走 `Err`（见「设计定夺」）
- ~~配置页分区组件拆分与「页面」组内排序~~——已闭环：五分区组件、警示区置顶；[配置] 追加组内末位、`Settings` 图标（见「设计定夺」）
- 追加闭环（proposal 未列的管线边角）：passthrough 出线形态定为 `Vec<ConfigExtraField>` 命名字段（非 flatten 非裸 map，bindings 管线零悬念）；命令 sync 形态；`WorkspaceConfigReport` / `ConfigReport` 双类型分工——均见「设计定夺」。
- 无未决项，无停线项。
