# 测试设计: desktop-workspace-config

> **日期**: 2026-09-29

---

## 验收范围

<!-- proposal.md 九条 AC 全量映射；同一 AC 的用例落在多个测试文件时逐行展开。
     模块清单取自 design.md「变更清单」新增 + 修改文件（本变更无删除文件），
     单元测试路径由 test_resolve_paths 派生；routes.tsx 不在测试套件范围
     （excludes），其路由行为以链路入口 app.tsx 的组合用例承载，见「不可测试项」。 -->

| AC ID | 验收条件 | 被测文件或模块 |
|--------|---------|----------|
| AC-1 | layout.rs 顶部常量组覆盖域目录名、`changes` / `archive` / `explores` 子目录名、`config.json` 文件名；`resolve` 与 `config_path` 全量引用常量；扩展后的隔离扫描对产品 `.rs` 源码（排除 `*_test.rs`）执行 `openspec` 与 `config.json` 双禁令，唯一例外 layout.rs | packages/desktop/src-tauri/crates/core/foundation/src/layout_test.rs |
| AC-2 | crate 依赖仅 `foundation` + `serde` / `serde_json` / `specta`，无 Tauri、无 workflow / agent / infra 依赖；`load` 对合法配置原样返回且 diagnostics 为空；对非法字段（枚举外框架名、越界阈值、含通配符的 suite root、`schema` 字面量不符）产出默认值填充的合法 config + 对应 diagnostics；passthrough 未知字段保留；文件缺失以报告内标记表达、不视为失败 | packages/desktop/src-tauri/crates/core/config/src/lib_test.rs |
| AC-3 | `workspace_config` 经 `collect_commands!` 注册；重导出后 `bindings.ts` 含 `workspaceConfig` 包装与 `WorkspaceConfigReport` 系 camelCase 类型；前端经生成绑定 typed 调用，无裸 `invoke('workspace_config')`；命令无 State / 无缓存 / 不落库 | packages/desktop/src-tauri/src/bindings_test.rs |
| AC-3 | 命令薄包装结构证明（无 State 直调、serde_json 对照 inner）与不落库无缓存行为化核对 | packages/desktop/src-tauri/src/commands/config/mod_test.rs |
| AC-4 | 壳态下「页面」组出现 [配置]（`data-testid="nav-config"`），点击后 URL 为 `/config` 并渲染配置页；欢迎态（root 为 null）无壳无该入口 | packages/desktop/src/components/app-sidebar.test.tsx |
| AC-4 | `/config` 路由可达（点击 / 深链）、未知路径兜底不回归、欢迎态隔离 | packages/desktop/src/app.test.tsx |
| AC-5 | 配置页分区呈现基础字段 / tests 面板（逐 suite：root / framework / cwd / config / includes / excludes / coverage / mutation）/ write_protection / 未知字段；未设阈值标注「未设（默认 N）」；diagnostics 警示区单列呈现非法项与吃默认项 | packages/desktop/src/views/config/config-view.test.tsx |
| AC-5 | 基础配置分区 + 「未设（默认 N）」标注 | packages/desktop/src/views/config/components/basic-config-section.test.tsx |
| AC-5 | tests 面板逐 suite 卡片 + 阈值标注对位 | packages/desktop/src/views/config/components/tests-section.test.tsx |
| AC-5 | write_protection 面板与空态占位 | packages/desktop/src/views/config/components/write-protection-section.test.tsx |
| AC-5 | 未知字段 passthrough JSON 预览 | packages/desktop/src/views/config/components/extra-fields-section.test.tsx |
| AC-5 | diagnostics 警示区单列与 kind 视觉分级 | packages/desktop/src/views/config/components/diagnostics-section.test.tsx |
| AC-6 | 进入页面发起一次解析；显式刷新入口（`disabled={loading}`）重取；无轮询 / watch / 事件订阅 / 任何缓存层；切换 workspace 以新根重取 | packages/desktop/src/views/config/hooks/use-workspace-config.test.ts |
| AC-7 | config.json 不存在呈现空态而非错误；坏 JSON / 命令 reject 呈现 inline 持久错误（testid 承载、无 toast 顶替） | packages/desktop/src/views/config/config-view.test.tsx |
| AC-7 | 无效 root `Err` 通道（缺失 / 不可读 / 非目录 / 空白）且进程无 panic；缺文件空态标记在报告内（数据面） | packages/desktop/src-tauri/src/commands/config/mod_test.rs |
| AC-8 | 全包除 layout.rs 外无 `openspec` 与 `config.json` 字面量（双禁令扫描机械半边） | packages/desktop/src-tauri/crates/core/foundation/src/layout_test.rs |
| AC-8 | 页面无任何写回 config.json 的操作入口（只读边界，全链无写命令） | packages/desktop/src/views/config/config-view.test.tsx |
| AC-9 | 静态检查 / 前端测试 / workspace 测试全绿、knip / lint 无新增豁免条目、新增 Rust 测试无 serde / JSON 库自身语义断言（管线判定归 test-execution；stats 常量消费点语义零变化由既有套件回归承载） | packages/desktop/src-tauri/src/commands/stats/mod_test.rs |

---

## 单元测试

<!-- 覆盖所有可在进程内验证的场景。测试框架识别结果（test_detect_frameworks）：
     - packages/desktop 前端 `*.ts` / `*.tsx` → vite-plus（`vite-plus/test` 的
       describe / it / expect / vi，jsdom 环境，colocated `*.test.ts(x)`）；
     - packages/desktop/src-tauri Rust → rust（cargo workspace，colocated
       `*_test.rs` / `mod_test.rs` + `#[cfg(test)] mod` 声明）。
     Rust 测试不启动 Tauri runtime（`#[tauri::command]` 保留原函数可直调先例）；
     前端进程边界统一收敛于 `@tauri-apps/api/core` invoke mock。
     测试边界（spec「测试纪律」requirement）：只覆盖自研层（校验端口 / diagnostics
     组装 / 命令薄包装 / 页面交互），MUST NOT 逐项断言 serde / serde_json /
     Path::join 等库自身语义。 -->

### packages/desktop/src-tauri/crates/core/foundation/src/layout.rs -> packages/desktop/src-tauri/crates/core/foundation/src/layout_test.rs

#### 待测功能

<!-- design.md「公共函数 / API」中所在文件为本源的行。 -->

- `DOMAIN_DIR_NAME`（`pub const &str`）: 域目录名裸名消费通道（fn `domain_dir_name()` 收敛形态，stats 消费点随之改常量引用）
- `CHANGES_DIR_NAME` / `ARCHIVE_DIR_NAME` / `EXPLORES_DIR_NAME` / `CONFIG_FILE_NAME`（`pub const &str`）: 常量组其余四员（`ARCHIVE_DIR_NAME` 相对 `changes` 锚定）
- `resolve(root: &Path) -> Layout`: 体内全量常量引用，行为零变化（签名不变）
- `config_path(root: &Path) -> PathBuf`: `<root>/<DOMAIN_DIR_NAME>/<CONFIG_FILE_NAME>` 纯拼接、无文件系统访问

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| layout 常量组 | 正向 | 五常量齐备且值与既有磁盘名逐字一致（域目录名、`changes`、`archive`、`explores`、`config.json`），doc 注明 `ARCHIVE_DIR_NAME` 的 changes 锚定基准 | 新增 |
| layout 常量组 | 正向 | `resolve` 输出三棵子树与常量组合路径逐字相等（`root/域目录/changes`、`root/域目录/changes/archive`、`root/域目录/explores`）——体内全量常量引用的结构证明，与既有三棵子树拓扑断言并存 | 新增 |
| layout 常量组 | 正向 | `config_path(root)` 等于 `root.join(DOMAIN_DIR_NAME).join(CONFIG_FILE_NAME)`，对不存在的 root 纯推导正常返回（无 IO） | 新增 |
| layout 常量组 | 边界 | `config_path` 空 root（纯相对形式）、带尾分隔符 root、指向普通文件的 root：纯拼接语义保持、不 panic（对齐既有 `resolve` 边界用例形态） | 新增 |
| layout 常量组 | 边界 | `config_path` 对同一输入结果稳定（确定性）；archive 产物的父目录恰为 changes 产物（锚定基准不变量） | 新增 |
| 命名隔离扫描（扩展） | 正向 | 全包产品 `.rs` 源码（排除 `*_test.rs`）双禁令扫描：含 `openspec` 或 `config.json` 任一字面量即 panic 并指认违例文件；扫描根覆盖前端 src、crates 全部 `*/src`、src-tauri/src | 新增 |
| 命名隔离扫描（扩展） | 边界 | layout.rs 自身必须同时含 `openspec` 与 `config.json` 字面量且确实存在（存在性 + 唯一性双向断言）；扫描计数大于零 | 新增 |
| 命名隔离扫描（扩展） | 异常 | `*_test.rs` 排除先例保留：测试文件自身含被扫字面量合法（fixtures 命名目录 / 断言字面值不受禁令误伤） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 真实文件系统（tempdir + 真实源码树） | 沿用既有 `TempDir` RAII 装置（`std::env::temp_dir` + Drop 清理）承载路径拼接用例；隔离扫描直接 `fs::read_to_string` 真实源码树（既有 `collect_rs_files` 扩展），无 mock | 全部 describe |

---

### packages/desktop/src-tauri/crates/core/config/src/lib.rs -> packages/desktop/src-tauri/crates/core/config/src/lib_test.rs

#### 待测功能

<!-- design.md「公共函数 / API」中所在文件为本源的行仅 `load` 一条；
     `assemble` 组装纯函数为 crate 内私有实现（「配置语义移植对照」表载体），
     经 crate 内子模块路径访问，不引入测试专用导出。 -->

- `load(root: &Path) -> ConfigReport`: 配置语义唯一入口——缺失（`FileMissing`）/ 读取失败（`ReadFailed`）/ 语法非法（`JsonInvalid`）/ 顶层非对象四分支全走默认值报告，`assemble` 逐字段组装校验；任意输入不失败收场；不校验 root 有效性（命令层职责）

#### 用例

<!-- 校验矩阵逐行对齐 design.md「配置语义移植对照」表；自洽不变量为矩阵核心判据。 -->

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| load：文件级分支 | 正向 | 有效 root 无配置文件 → `Ok` + 全默认 config + `FileMissing`（path `$`）诊断，不失败收场（空态来源） | 新增 |
| load：文件级分支 | 异常 | 配置路径被同名目录占据（存在但不可读）→ `Ok` + 全默认 config + `ReadFailed` 诊断（inline 错误来源） | 新增 |
| load：文件级分支 | 异常 | 坏 JSON 语法 → `Ok` + 全默认 config + `JsonInvalid` 诊断（inline 错误来源） | 新增 |
| load：文件级分支 | 边界 | 空文件 / 仅空白字符 → 落 `JsonInvalid` 分支，不 panic（文件内容字符串边界） | 新增 |
| load：顶层与基础字段 | 正向 | 全字段合法完整配置（`$schema` / `schema` / `context` / `static_analysis` / `rules` 全设合法值）→ config 逐字段等于文件内容、diagnostics 为空 | 新增 |
| load：顶层与基础字段 | 异常 | 顶层非对象三形态（数组 / 标量 / null）→ 全默认 config + `InvalidValue`（path `$`），不失败 | 新增 |
| load：顶层与基础字段 | 异常 | `$schema` / `context` / `static_analysis` 非字符串（含显式 null）→ 字段 `None` + 逐字段 `InvalidValue`（path 对位） | 新增 |
| load：顶层与基础字段 | 异常 | `schema` 值 ≠ `spec-driven`（空串 / 大小写变体 / 任意串）→ 默认 `"spec-driven"` + `InvalidValue`（literal 端口） | 新增 |
| load：顶层与基础字段 | 异常 | `rules` 非对象（数组 / 字符串）→ `rules` `None` + `InvalidValue`；`rules.proposal` / `rules.tasks` 非字符串数组（元素混入数字）→ 该子字段 `None` + `InvalidValue`（path 形如 `rules.proposal`） | 新增 |
| load：顶层与基础字段 | 边界 | `rules.proposal` / `rules.tasks` 空数组与多元素列表合法原样保留、无诊断（list 边界内侧） | 新增 |
| load：tests suite | 正向 | 合法多 suite（八框架枚举 `jest` / `vitest` / `vite-plus` / `bun` / `rust` / `node-test` / `go` / `pytest` 逐值各一）→ 全部原样保留、diagnostics 为空（枚举 N 侧全覆盖） | 新增 |
| load：tests suite | 边界 | `tests` 显式 `[]` → 保留空数组、无 `DefaultApplied`（与缺省 `[]` + `DefaultApplied`（path `tests`）形态可区分） | 新增 |
| load：tests suite | 异常 | `tests` 非数组（对象 / 字符串 / 数字）→ `[]` + `InvalidValue`（path `tests`） | 新增 |
| load：tests suite | 异常 | suite 元素非对象（字符串 / 数字 / 数组 / null）→ 该 suite 剔除 + `InvalidValue`（path `tests[i]`） | 新增 |
| load：tests suite | 异常 | suite 必需字段缺失：`root` 缺失或 `framework` 缺失 → 剔除整个 suite + `InvalidValue`（path `tests[i].root` / `tests[i].framework`） | 新增 |
| load：tests suite | 异常 | suite `root` 空串，或含通配符（`*` `?` `{` `[` 四字符逐个 + 组合形态）→ 剔除整个 suite + `InvalidValue`（nonempty + 通配符禁令端口） | 新增 |
| load：tests suite | 边界 | suite `root` 超长（>1000 字符）/ 含换行 / emoji / 中文路径段 → 无通配符即合法原样保留（str 边界合法侧） | 新增 |
| load：tests suite | 异常 | suite `framework` 枚举外值（未知串 / 大小写变体 `Jest` / 空串 / 数字）→ 剔除整个 suite + `InvalidValue`（枚举 +1 侧） | 新增 |
| load：tests suite | 异常 | suite `cwd` 缺失 → `"."` + `DefaultApplied`（path `tests[i].cwd`）；`cwd` 非字符串 → `"."` + `InvalidValue` | 新增 |
| load：tests suite | 异常 | suite `config` 空串或非字符串 → `None` + `InvalidValue`；`config` 合法非空串保留；缺失 → `None` 无诊断（nonempty 边界） | 新增 |
| load：tests suite | 异常 | `includes` / `excludes` 非数组或元素非字符串 → `None` + `InvalidValue`；空数组合法保留、缺失无诊断（list 边界） | 新增 |
| load：coverage 阈值 | 边界 | suite `coverage` 缺失 → 三阈值 80 / 70 / 75 + 逐字段 `DefaultApplied`（path 形如 `tests[i].coverage.lines`） | 新增 |
| load：coverage 阈值 | 异常 | `coverage` 非对象（数组 / 标量）→ 三阈值全默认 + `InvalidValue`（path `tests[i].coverage`） | 新增 |
| load：coverage 阈值 | 边界 | 三阈值值域端点 0 与 100、浮点 80.5 → 合法原样保留（number 边界内侧）；浮点忠实出线 | 新增 |
| load：coverage 阈值 | 异常 | 阈值越界（-1 / -0.5 / 100.1 / 101 / 200）→ 该字段默认 + 逐字段 `InvalidValue`（number 边界外侧，lines / branches / functions 三字段各覆盖） | 新增 |
| load：coverage 阈值 | 异常 | 阈值非数值（字符串 `"80"` / null / bool）→ 默认 + 逐字段 `InvalidValue`（number 类型边界） | 新增 |
| load：mutation 阈值 | 边界 | suite `mutation` 缺失 → `cwd` `None` 无诊断 + `score` 70 + `DefaultApplied`；`mutation` 非对象 → 同缺失处置 + `InvalidValue`（path `tests[i].mutation`） | 新增 |
| load：mutation 阈值 | 异常 | `mutation.score` 越界（-1 / 200）或非数值 → 默认 70 + `InvalidValue`；`mutation.cwd` 非字符串 → `None` + `InvalidValue` | 新增 |
| load：write_protection 与 passthrough | 正向 | `write_protection` 合法（files 逐条 glob + reason）→ 原样保留、diagnostics 空 | 新增 |
| load：write_protection 与 passthrough | 边界 | `write_protection` 缺失 → `None` 无诊断；非对象 → `None` + `InvalidValue`；`files` 非数组 → `None` + `InvalidValue`（path `write_protection.files`） | 新增 |
| load：write_protection 与 passthrough | 异常 | fileRule `glob` 空串或非字符串 → 剔除该条规则 + `InvalidValue`（path `write_protection.files[i].glob`），同数组其余合法条目保留；`reason` 非字符串 → `None` + `InvalidValue` | 新增 |
| load：write_protection 与 passthrough | 正向 | 顶层未知字段 passthrough → `extra` 逐条 key + 原文 `Value`（嵌套对象 / 数组 / 标量 / null 多形态）保留、无诊断；已知字段不进 `extra` | 新增 |
| load：write_protection 与 passthrough | 边界 | 多个未知字段保序保留；未知字段值为超长字符串 / 深嵌套对象 → 原样无损保留（str 边界） | 新增 |
| load：自洽不变量与默认基线 | 正向 | 违例矩阵抽样（顶层非对象 / 坏 suite / 越界阈值 / 坏 JSON / 文件缺失）各自产出的 config 重序列化回 `Value` 再过同一 `assemble` → diagnostics 恒为空（「config 输出永远合法」机械形态） | 新增 |
| load：自洽不变量与默认基线 | 边界 | 默认基线逐字段断言（`schema` `"spec-driven"`、`tests` `[]`、coverage 80 / 70 / 75、`mutation.score` 70、其余 `None`、`extra` `[]`），且默认基线过 `assemble` 零诊断 | 新增 |
| load：自洽不变量与默认基线 | 边界 | 两个 suite 一违一合：违例 suite 剔除并记诊断，合法 suite 保序保留不殃及（剔除处置不扩散） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 真实文件系统（tempdir） | 既有 `TempWs` 式 RAII tempdir 装置（stats mod_test 先例）：合法 / 坏 JSON / 空文件 / 同名目录占据（ReadFailed）/ 缺失各一 fixture，测试结束自动清理；无 mock | 全部 describe |
| crate 内私有组装函数 | `assemble` 经 crate 内子模块路径访问（Rust 子模块可见父模块私有项），自洽不变量直接调用；不新增测试专用导出（CLAUDE.md no-test-only-exports） | load：自洽不变量与默认基线 |

---

### packages/desktop/src-tauri/src/commands/config/mod.rs -> packages/desktop/src-tauri/src/commands/config/mod_test.rs

#### 待测功能

- `workspace_config(root: String) -> Result<WorkspaceConfigReport, String>`: 三件事薄包装（参数转换 → 调用 → 错误映射），sync 形态、零 State、零缓存、不落库
- `workspace_config_inner(root: &Path) -> Result<WorkspaceConfigReport, String>`: `fs::metadata` root 有效性检查为 Err 通道唯一来源（缺失 / 不可读 / 非目录 / 空白 → `Err`）→ `config::load` → 信封字段平移；配置文件缺失不是 `Err`（报告内 `FileMissing` 标记）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| workspace_config 薄包装 | 正向 | 命令与 inner 对同一 tempdir 根结果经 serde_json 对照逐字段一致（命令层零加工，stats 先例） | 新增 |
| workspace_config 薄包装 | 正向 | `(String,)` 原签名直调无 State 入参（薄包装结构证明，不启动 Tauri runtime）；无效 root 的 `Err` 经命令层透传且错误串与 inner 一致 | 新增 |
| workspace_config_inner：root 有效性 | 异常 | 不存在的 root → `Err`（含 root 无效语义）不 panic | 新增 |
| workspace_config_inner：root 有效性 | 异常 | root 指向普通文件 → `Err`（指明非目录） | 新增 |
| workspace_config_inner：root 有效性 | 异常 | 空白 root（`""`）→ `Err`（对齐 code_stats 裁定：blank 只能来自调用 bug，不静默空报告） | 新增 |
| workspace_config_inner：root 有效性 | 边界 | 超长不存在路径 → `Err` 不 panic；Windows 下悬空 junction 不可读 root → `Err`（cfg(windows)，stats 先例 fixture） | 新增 |
| workspace_config_inner：文件态分层 | 正向 | 有效 root 无配置文件 → `Ok` + `FileMissing` 诊断 + 全默认 config（空态标记，非 `Err`） | 新增 |
| workspace_config_inner：文件态分层 | 异常 | 坏 JSON → `Ok` + `JsonInvalid` 诊断 + 默认 config（报告内 fatal 走 `Ok`，仅无效 root 走 `Err`——两通道分层裁定） | 新增 |
| workspace_config_inner：文件态分层 | 正向 | 合法配置文件 → `Ok` + diagnostics 空，`ConfigReport` → `WorkspaceConfigReport` 字段平移一致 | 新增 |
| workspace_config_inner：不落库无缓存 | 边界 | 连续两次调用各自完整读取：第一次后改写文件内容，第二次结果随文件变化（无缓存行为化核对；store 零触碰） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 真实文件系统（tempdir） | RAII tempdir 承载有效 / 缺失 / 坏 JSON / 非目录 / 不可读（悬空 junction，cfg(windows)）root 与配置文件改写序列；文件系统不 mock | 全部 describe |

---

### packages/desktop/src-tauri/src/bindings.rs -> packages/desktop/src-tauri/src/bindings_test.rs

#### 待测功能

<!-- design.md「公共函数 / API」无所在文件为本源的行；`collect_commands!` 增第
     24 条为「修改文件」表声明。API 表行 `commands.workspaceConfig`（所在文件为
     生成物 bindings.ts）的断言载体是本测试对 `export_bindings()` 导出产物的
     快照比对，故挂靠本节（链路：collect_commands! 注册 → 重导出 → 生成物）。 -->

- `commands.workspaceConfig`（生成包装，经产物断言）: `workspaceConfig: (root: string) => Promise<WorkspaceConfigReport>`（Throw 模式 reject 语义）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 导出产物覆盖性（24 条扩面） | 正向 | 产物含全部 24 条命令包装名与 invoke 命令名：清单 23 → 24（`workspaceConfig` / `workspace_config` 在册），既有 23 条逐字回归 | 新增 |
| 导出产物覆盖性（24 条扩面） | 正向 | `workspaceConfig` 包装形态逐字：`(root: string) => __TAURI_INVOKE<WorkspaceConfigReport>("workspace_config", { root })`（Promise reject 语义，无 Result 包装） | 新增 |
| 导出产物覆盖性（24 条扩面） | 正向 | 出线 DTO 十二型齐备：`WorkspaceConfigReport` / `WorkspaceConfig` / `TestSuite` / `TestFramework` / `CoverageThresholds` / `MutationConfig` / `RulesConfig` / `WriteProtection` / `WriteProtectionFile` / `ConfigExtraField` / `ConfigDiagnostic` / `DiagnosticKind` | 新增 |
| 新 DTO 出线形态 | 正向 | `TestFramework` 出线八字符串字面量联合（`jest` / `vitest` / `vite-plus` / `bun` / `rust` / `node-test` / `go` / `pytest`——非 rename_all 可表达的 `vite-plus` / `node-test` 在册） | 新增 |
| 新 DTO 出线形态 | 正向 | `DiagnosticKind` 出线五值 camelCase 字面量联合（`fileMissing` / `readFailed` / `jsonInvalid` / `invalidValue` / `defaultApplied`）；`ConfigExtraField.value` 出线 `unknown`（serde_json::Value 语义规则既有口径回归） | 新增 |
| 新 DTO 出线形态 | 边界 | 24 条注册后导出仍幂等确定：连续两次导出产物逐字节一致、无机器路径嵌入（确定性输出不回归） | 新增 |
| 既有断言回归 | 边界 | 纂改恢复 / 目录缺失重建 / 错误面 Promise reject / 无 `_Deserialize` 相位伴生等既有用例全保留、零废弃 | 新增 |

<!-- 迭代类型列语义：本表仅列本变更新增的断言行（新增）；既有用例一行不删
     （零废弃），回归范围如上行所列。 -->

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 产物文件（共享单一目标） | 沿用既有互斥锁串行化 + 权威快照装置（`export_bindings` 重导出后读字节、篡改类用例挂 `RestoreOnDrop` 守卫），文件系统不 mock | 全部 describe |

---

### packages/desktop/src-tauri/src/commands/stats/mod.rs -> packages/desktop/src-tauri/src/commands/stats/mod_test.rs

#### 待测功能

<!-- design.md「公共函数 / API」无所在文件为本源的行（无公共 API 变更声明）。
     本文件修改为 fn → 常量消费点形态更新（`use foundation::layout::domain_dir_name`
     → `use foundation::layout::DOMAIN_DIR_NAME`，`get_statistics` 实参
     `&[DOMAIN_DIR_NAME]`），统计语义零变化（AC 与 spec 明令）：
     既有 mod_test.rs 全部用例保留回归、零废弃，不新增用例——排除语义行为由
     既有「根层 / 任意层级目录整棵排除」用例在常量引用下继续承载，常量值本体
     由 layout_test.rs 常量组断言承载。 -->

#### 用例

<!-- 无新增用例行（见上注）。 -->

#### Mock策略

<!-- 无新增用例即无新增 mock：既有装置（RAII tempdir 真实文件系统、tokei 真实
     消费）全量保留回归，无任何 mock 面的新增或调整。 -->

---

### packages/desktop/src/views/config/hooks/use-workspace-config.ts -> packages/desktop/src/views/config/hooks/use-workspace-config.test.ts

#### 待测功能

- `useWorkspaceConfig(root: string): WorkspaceConfigState`: 取数收口 hook——effect 依赖 `[root, tick]` 显式刷新模型，经 `commands.workspaceConfig` typed 调用；`{ data, loading, error, refresh }` 形态；`cancelled` 防串轮 + 数据归属 root 标记（切换过渡轮抑制旧根报告）；无轮询、无 watch、无事件订阅、无缓存

#### 用例

<!-- use-code-stats.test.ts 同型矩阵（同型 hook 先例），命令与参数换为 workspace_config。 -->

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| useWorkspaceConfig：显式刷新纪律 | 正向 | 挂载以 `{ root }` 发起 `workspace_config` 恰一次，`data` 呈现返回报告，loading true→false | 新增 |
| useWorkspaceConfig：显式刷新纪律 | 正向 | `refresh()` 同 root 恰再发一次 | 新增 |
| useWorkspaceConfig：显式刷新纪律 | 边界 | 取数完成后静置（推进定时器、无任何交互）：调用数不增长（无轮询的行为化核对） | 新增 |
| useWorkspaceConfig：失败面 | 异常 | 首取 reject → `error` 含错误串、loading 复位、`data` 保持 null | 新增 |
| useWorkspaceConfig：失败面 | 异常 | 成功后 `refresh()` 再 reject → `error` 置位、`data` 保持上次成功值不崩 | 新增 |
| useWorkspaceConfig：root 切换与归属 | 边界 | root A→B：以新根重取，B 结果到达后 `data` 归属 B | 新增 |
| useWorkspaceConfig：root 切换与归属 | 边界 | 切换过渡轮（B 在途）：`data` 为 null，旧根报告不呈现（归属 root 标记抑制） | 新增 |
| useWorkspaceConfig：root 切换与归属 | 边界 | 旧根迟到 resolve / reject：不覆盖新态、不置错误（cancelled 抑制） | 新增 |
| useWorkspaceConfig：生成绑定调用面 | 正向 | 经 `commands.workspaceConfig` typed 入口发起：命令名与参数 `{ root }` 逐字不变（生成绑定底层同模块 invoke，mock 机制切换后依旧生效；无裸 invoke 字符串） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC 进程边界 | `vi.hoisted` invokeMock + `vi.mock('@tauri-apps/api/core')`：按命令名分发 resolve / reject / 手动 pending 三态控制；返回 camelCase `WorkspaceConfigReport` fixture 深拷贝防用例间残留（use-code-stats.test 先例） | 全部 describe |

---

### packages/desktop/src/views/config/config-view.tsx -> packages/desktop/src/views/config/config-view.test.tsx

#### 待测功能

- `ConfigView({ root }: { root: string }): React.JSX.Element`: 页面唯一出口组件——页头（标题 + 刷新钮 `config-refresh`，`disabled={loading}`）+ inline 持久错误（`config-error`，命令 reject 与文件级 fatal 诊断双来源）+ loading 行（`config-loading`）+ 空态（`config-empty`，`FileMissing` 时呈现）+ 五分区编排 + diagnostics → `defaultedPaths` 派生（`kind === 'defaultApplied'` 的 path 集合）

#### 用例

<!-- 本节兼作跨模块组合用例挂靠入口（链路：useWorkspaceConfig 取数 → 五分区
     渲染），内部模块不 mock 真实组合。 -->

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| ConfigView：页面编排 | 正向 | 完整报告：五分区依序呈现（diagnostics 警示区置顶 → 基础配置 → tests → write_protection → 未知字段），页面根 `config-view` 在场 | 新增 |
| ConfigView：页面编排 | 正向 | 挂载以当前 root 发起一次解析（`workspace_config` `{ root }`），loading 行 `config-loading` 在场、数据到达后卸载 | 新增 |
| ConfigView：页面编排 | 正向 | `defaultedPaths` 派生对位：`DefaultApplied` 诊断 path（如 `tests[0].coverage.lines`）驱动对应分区呈现「未设（默认 80）」，文件显式设值字段不标注 | 新增 |
| ConfigView：状态面 | 异常 | 命令 reject → `config-error` inline 持久呈现（testid 承载、无 toast 顶替），分区不渲染 | 新增 |
| ConfigView：状态面 | 异常 | 文件级 fatal 诊断（`ReadFailed` / `JsonInvalid`）→ `config-error` inline 呈现（双来源之文件侧） | 新增 |
| ConfigView：状态面 | 边界 | `FileMissing` 诊断 → `config-empty` 空态（非错误、无 error 语义），文案说明默认值行为，与 diagnostics 警示区分 | 新增 |
| ConfigView：刷新交互 | 正向 | 点击 `config-refresh` 恰再发一次解析；loading 期间刷新钮 disabled | 新增 |
| ConfigView：切换工作区 | 边界 | root prop 变化 → 以新根重取，旧根报告不呈现（hook 归属抑制经页面可见，AC-6 壳层核对） | 新增 |
| ConfigView：只读边界 | 边界 | 页面无任何写回入口：唯一可交互控件为刷新钮，无文本输入 / 编辑 / 保存 / 删除控件，全程无写命令发起（AC-8 前端半边） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC 进程边界 | invokeMock 按命令名 `workspace_config` 分发（resolve / reject / 手动 pending 控制 loading 态）；返回 camelCase `WorkspaceConfigReport` fixture（含合法 / `FileMissing` / `ReadFailed` / `JsonInvalid` / 违例 + `DefaultApplied` 组合各一），深拷贝防残留（info-view.test 先例） | 全部 describe |
| 内部模块 | `useWorkspaceConfig` 与五分区组件不 mock、真实组合（组合用例挂靠本入口模块） | 全部 describe |

---

### packages/desktop/src/views/config/components/diagnostics-section.tsx -> packages/desktop/src/views/config/components/diagnostics-section.test.tsx

#### 待测功能

- `DiagnosticsSection({ diagnostics }: { diagnostics: ConfigDiagnostic[] }): React.JSX.Element`: diagnostics 警示区单列置顶，kind 视觉分级（`InvalidValue` 强调色 / `DefaultApplied` 弱化），条目展示 path + message；diagnostics 为空时整区不渲染

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| DiagnosticsSection | 正向 | `InvalidValue` 条目呈现 path + message 且强调色分级（`config-diagnostics` 区 + `config-diagnostic-item` 条目） | 新增 |
| DiagnosticsSection | 正向 | `DefaultApplied` 条目弱化呈现，与 `InvalidValue` 视觉可分，path 与 message 逐字承载（含违例原值与所落默认值的中文文案） | 新增 |
| DiagnosticsSection | 边界 | 空数组 → 整区不渲染（`config-diagnostics` 不在场） | 新增 |
| DiagnosticsSection | 边界 | 混合 kind 列表逐条渲染、条数一致；20+ 条全量渲染无丢失 | 新增 |
| DiagnosticsSection | 边界 | path 两形态原样呈现：文件级 `$` 与点路径 `tests[0].coverage.lines` | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（纯组件） | fixture `ConfigDiagnostic[]` 直传入参渲染断言，无进程边界、无 mock | 全部 describe |

---

### packages/desktop/src/views/config/components/basic-config-section.tsx -> packages/desktop/src/views/config/components/basic-config-section.test.tsx

#### 待测功能

- `BasicConfigSection({ config, defaultedPaths }: { config: WorkspaceConfig; defaultedPaths: ReadonlySet<string> }): React.JSX.Element`: 基础配置分区（`$schema` / `schema` / `context` / `static_analysis` / `rules`）+ 按 `defaultedPaths` 对位标注「未设（默认 N）」

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| BasicConfigSection | 正向 | 五字段呈现：`$schema` / `schema` / `context` / `static_analysis` / `rules`（proposal / tasks 列表逐项） | 新增 |
| BasicConfigSection | 正向 | `schema` path 在 `defaultedPaths` → 呈现「未设（默认 spec-driven）」，与文件显式设值形态可区分 | 新增 |
| BasicConfigSection | 边界 | 可选字段 `None`（context / static_analysis / rules）→ 弱化「未配置」占位行（面板在、内容空） | 新增 |
| BasicConfigSection | 边界 | `defaultedPaths` 为空集 → 任何字段不出现「未设（默认 N）」标注 | 新增 |
| BasicConfigSection | 边界 | `rules.proposal` / `rules.tasks` 空数组与多元素列表两形态渲染；path 不对位的 `defaultedPaths` 条目不污染渲染 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（纯组件） | fixture `WorkspaceConfig` + `ReadonlySet<string>` 直传入参渲染断言，无 mock | 全部 describe |

---

### packages/desktop/src/views/config/components/tests-section.tsx -> packages/desktop/src/views/config/components/tests-section.test.tsx

#### 待测功能

- `TestsSection({ suites, defaultedPaths }: { suites: TestSuite[]; defaultedPaths: ReadonlySet<string> }): React.JSX.Element`: tests 面板逐 suite 卡片（root / framework / cwd / config / includes / excludes / coverage 三阈值 / mutation cwd + score）+ 阈值按 path 对位标注「未设（默认 N）」；空数组呈弱化空行

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| TestsSection | 正向 | 多 suite 逐卡片呈现全字段（`config-suite` 计数与 fixture 一致，字段值逐字承载） | 新增 |
| TestsSection | 正向 | 阈值标注索引对位：`defaultedPaths` 含 `tests[0].coverage.lines` → 第一卡片该阈值「未设（默认 80）」，`tests[1]` 同字段不受波及（`mutation.score` 对位 `tests[i].mutation.score` 同理） | 新增 |
| TestsSection | 边界 | suites 空数组 → `config-tests` 在场呈弱化空行、无 `config-suite` | 新增 |
| TestsSection | 边界 | `includes` / `excludes` / `config` / `mutation.cwd` 为 `None` → 对位占位不崩 | 新增 |
| TestsSection | 边界 | 阈值 `number \| null` 出线口径的 null 值 → `??` 防御呈现不崩（specta 裸 f64 口径先例，`LanguageStats.share` 同型） | 新增 |
| TestsSection | 边界 | 30 suite 全量渲染无丢失、`config-suite` 计数一致 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（纯组件） | fixture `TestSuite[]` + `ReadonlySet<string>` 直传入参渲染断言，无 mock | 全部 describe |

---

### packages/desktop/src/views/config/components/write-protection-section.tsx -> packages/desktop/src/views/config/components/write-protection-section.test.tsx

#### 待测功能

- `WriteProtectionSection({ protection }: { protection: WriteProtection \| null }): React.JSX.Element`: write_protection 面板逐条 glob 规则 + reason；未配置呈现弱化「未配置」占位

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| WriteProtectionSection | 正向 | files 逐条 glob + reason 呈现（`config-write-protection` 在场） | 新增 |
| WriteProtectionSection | 边界 | `protection` 为 null → 弱化「未配置」占位、不崩 | 新增 |
| WriteProtectionSection | 边界 | `protection` 在而 `files` 为 null / 空数组 → 弱化空态呈现不崩 | 新增 |
| WriteProtectionSection | 边界 | `reason` 为 null → 该条 reason 位占位；多条规则全量渲染 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（纯组件） | fixture `WriteProtection \| null` 直传入参渲染断言，无 mock | 全部 describe |

---

### packages/desktop/src/views/config/components/extra-fields-section.tsx -> packages/desktop/src/views/config/components/extra-fields-section.test.tsx

#### 待测功能

- `ExtraFieldsSection({ extra }: { extra: ConfigExtraField[] }): React.JSX.Element`: 未知字段分区逐条 key + JSON 预览（`JSON.stringify(value, null, 2)` 于等宽块）；`extra` 为空时整区不渲染

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| ExtraFieldsSection | 正向 | 逐条 key + JSON 预览呈现（`config-extra` 在场），对象 / 数组 / 标量 / null 多形态 value 均可预览 | 新增 |
| ExtraFieldsSection | 边界 | 空数组 → 整区不渲染（`config-extra` 不在场） | 新增 |
| ExtraFieldsSection | 边界 | 深嵌套 / 超长 JSON 值预览完整无损（key → 原文值不变，passthrough 可见性即需求本体） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（纯组件） | fixture `ConfigExtraField[]` 直传入参渲染断言，无 mock | 全部 describe |

---

### packages/desktop/src/components/app-sidebar.tsx -> packages/desktop/src/components/app-sidebar.test.tsx

#### 待测功能

<!-- design.md「公共函数 / API」无所在文件为本源的行（AppSidebar 组件出口无
     公共 API 表行变更）；本文件修改为「修改文件」表声明：PageNavGroup 末位增
     [配置] NavLink（lucide `Settings` 图标、`to="/config"`、
     `data-testid="nav-config"`、active 由 URL 派生）。用例直接对组件渲染产物
     断言（data-testid 挂钩）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| AppSidebar：页面导航组 nav-config 扩员 | 正向 | `nav-config` 在场且居「页面」组末位（组序 nav-info → nav-changes → nav-explores → nav-config）、tagName 为 A、href=/config、带 svg 图标、文案「配置」 | 新增 |
| AppSidebar：页面导航组 nav-config 扩员 | 正向 | pathname=/config → `nav-config` data-active=true 且其余 nav 全 false；/changes 下 `nav-config` 为 false（active 由 URL 派生） | 新增 |
| AppSidebar：页面导航组 nav-config 扩员 | 正向 | 点击 `nav-config` → location-probe 呈 /config | 新增 |
| AppSidebar：页面导航组 nav-config 扩员 | 边界 | pathname 无匹配前缀（/bogus）→ `nav-config` data-active=false、渲染不崩（对齐既有降级用例） | 新增 |
| AppSidebar：既有挂钩回归 | 边界 | 页面组三项与系统工具组（nav-agent / nav-db）的组归属与激活语义既有断言全保留、零废弃（扩员不改变他项） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 运行环境（jsdom 环境缺口） | 既有 `ResizeObserverStub` + `vi.stubGlobal` 兜底；`MemoryRouter` 包裹 + LocationProbe 承载 URL 断言（既有装置沿用），业务依赖无 mock | 全部 describe |

---

### packages/desktop/src/app.tsx -> packages/desktop/src/app.test.tsx

#### 待测功能

<!-- design.md「公共函数 / API」无所在文件为本源的行（App 壳组件出口无公共
     API 表行变更）；/config 路由项为 routes.tsx「修改文件」表声明，而
     routes.tsx 不在测试套件范围（见不可测试项）。本文件为路由链路入口
     （App 壳 → 路由表 → 页面），组合用例按链路入口挂靠本节（/info 路由套件
     先例），断言经渲染产物 testid / hash / invoke 参数承载。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| App：/config 配置路由可达与欢迎态隔离 | 正向 | 壳态点击 `nav-config` → hash 落 #/config、`config-view` 在场、`nav-config` 激活、变更页内容卸载、`workspace_config` 以 `{ root: 当前根 }` 发起 | 新增 |
| App：/config 配置路由可达与欢迎态隔离 | 正向 | 启动前 hash 已为 #/config：深链直出配置页且 `nav-config` 激活，恰发起一次解析（对齐 /info 深链用例形态） | 新增 |
| App：/config 配置路由可达与欢迎态隔离 | 边界 | 未知路径兜底回归：#/bogus 仍兜底落 #/changes（/config 插入不破 `*` 兜底语义）、不误触 `workspace_config` | 新增 |
| App：/config 配置路由可达与欢迎态隔离 | 边界 | #/config/xyz（路由表无子段）→ 兜底落 #/changes、不空白不崩 | 新增 |
| App：/config 配置路由可达与欢迎态隔离 | 边界 | 欢迎态（root=null）：无壳无 `nav-config`、#/config 不渲染配置页仅欢迎屏、解析零发起（Router 未挂壳的隔离，AC-4 后半） | 新增 |
| App：/config 配置路由可达与欢迎态隔离 | 异常 | `workspace_config` reject（含无效 root `Err` reject）→ `config-error` inline 呈现、无 toast 顶替、壳不崩（AC-7 全链核对） | 新增 |
| App：/config 配置路由可达与欢迎态隔离 | 边界 | 切换 workspace（sidebar 清单项）后停留 /config → 以新根重发 `workspace_config`、旧根配置不呈现（AC-6 切换重取的壳层核对） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC 进程边界 | 既有 `mockIpc` 按命令名分发装置扩 `workspace_config` 分支（resolve / reject / pending 三态 + fixture 深拷贝）；invokeMock 调用次数 / 参数断言沿用（countOf） | 全部 describe |
| 进程外依赖 | 既有 getVersion / dialog open / updater check mock 装置与 sonner toast 残留清理沿用，不新增 mock 面 | 全部 describe |

---

## 不可测试项

<!-- test_resolve_paths errors：packages/desktop/src/routes.tsx — Not in test
     config scope（桌面套件 excludes 显式排除）。其余条目为 proposal / design
     范围内无法以进程内自动化用例验证或不在单测面的声明性 / 管线性内容。 -->

- packages/desktop/src/routes.tsx — **原因**: 桌面测试套件 excludes 显式排除（组合声明文件、无独立测试路径，test_resolve_paths 报 Not in test config scope）；`/config` 路由行为以链路入口 `packages/desktop/src/app.tsx` 的组合用例承载（app.test.tsx「/config 配置路由可达与欢迎态隔离」describe，/info 先例同型）
- packages/desktop/src-tauri/Cargo.toml（`[workspace.members]` / `[workspace.dependencies]` / 根包 `[dependencies]` 增 `config`） — **原因**: 纯构建配置声明，无进程内可验证行为；AC-8 的「crate 图机械保证 `config` 无 Tauri、不依赖 workflow / agent / infra」由 cargo 依赖图在编译期机械保证（未声明的依赖无法 use），随 AC-9 静态检查（server:check）复核，非单测面
- packages/desktop/src-tauri/src/commands/mod.rs（`pub mod config;` 挂载 + 轨道清单 doc 六轨 → 七轨） — **原因**: 纯模块挂载声明，无独立运行时行为；挂载正确性由 bindings_test.rs 命令注册断言与编译期承载
- packages/desktop/src/types/generated/bindings.ts（重导出生成物） — **原因**: 生成物不手改、无独立测试路径；其内容断言（`workspaceConfig` 包装 + 十二型 camelCase 镜像）经 bindings_test.rs 对 `export_bindings()` 产物快照承载（见 bindings.rs 章节）
- packages/desktop/package.json（version 0.3.8 → 0.3.9） — **原因**: 版本收尾元数据（archive 时生效），归 archive 收尾人工核对，非单测面
- AC-9 管线判定（静态检查 / 前端测试 / workspace 测试全绿、knip / lint 无新增豁免） — **原因**: 归 test-execution 阶段执行判定，test-design 阶段不预设执行结论；stats 常量消费点语义零变化以既有 mod_test.rs 全量回归承载（见该章节说明）
- 「无 watch / 事件订阅 / 任何缓存层」的绝对不存在性证明 — **原因**: 不存在性无法穷举证明；以行为化核对为代理断言（hook 静置调用数不增长、命令连续调用各自完整读取、`refresh` 为唯一重取入口），框架与库自身语义不重测（spec「测试纪律」requirement）
- DTO specta 出线形态的运行时正确性（camelCase 字段名、`number | null` 等 TS 镜像形状） — **原因**: 生成物形状由 bindings_test.rs 对产物文本的快照断言承载（AC-3）；specta 生成器对 derive 类型的转换语义属库自身语义，不重测


