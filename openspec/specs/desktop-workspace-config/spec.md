# desktop-workspace-config Specification

## Purpose

定义 desktop 工作区配置能力：core/config crate 的配置读取与校验契约（校验语义复刻 CLI zod schema、默认值填充、passthrough、不复刻吞错的 `ConfigReport` 信封）、`workspace_config` 命令轨道、`/config` 只读配置页的呈现面与显式刷新取数模型，以及 `config.json` 文件名字面量隔离执法与测试纪律。

## Requirements

### Requirement: 配置解析 core crate

Rust 侧 SHALL 新增 crate `crates/core/config`（裸名 `config`），承载工作区 `config.json` 的读取、解析、校验与默认值填充：

- 依赖 SHALL 仅 `foundation`（workspace 内，取 config.json 路径）+ `serde` / `serde_json` / `specta`；MUST NOT 依赖 `workflow` / `agent` / 任何 infra crate，MUST NOT 依赖 Tauri；
- 入口 API SHALL 为 `load(root) -> ConfigReport`（函数签名形态 design 定夺），`ConfigReport` SHALL 为 `{ config, diagnostics }` 信封：`config` 为**永远合法**的完整配置（违例字段以默认值填充），`diagnostics` 记录违例细节（哪项非法、哪些字段吃了默认、文件缺失等）；
- 校验规则 SHALL 逐条复刻 CLI `plugins/dev-team/bin/src/schemas/config/config.schema.ts`（zod/v4）语义：`tests[].framework` 八值枚举（`jest` / `vitest` / `vite-plus` / `bun` / `rust` / `node-test` / `go` / `pytest`）；数值阈值 0–100 值域（coverage lines / branches / functions、mutation score）；`tests[].root` 非空且 MUST NOT 含 glob 通配符（`*` `?` `{` `[`）；`schema` 字段字面量 `spec-driven`；顶层未知字段 passthrough 保留；
- 默认值 SHALL 对齐 CLI `defaults.ts`：coverage lines 80 / branches 70 / functions 75、mutation score 70、suite `cwd` `.`、`tests` `[]`、`schema` `spec-driven`；
- CLI `readConfig` 的「非法即静默整体回默认」吞错语义 SHALL NOT 复刻：任意输入（缺失文件、坏 JSON、字段非法）MUST NOT 使 `load` 失败收场——产出合法 config + diagnostics；非法输入产出的 config SHALL 能通过同一套校验（自洽不变量）；
- config.json 路径 SHALL 经 `foundation::layout` 的路径解析取得，crate 内 MUST NOT 出现 `config.json` 文件名字面量（隔离执法见 desktop-crate-layout 增量）。

#### Scenario: 合法配置原样呈现

- **WHEN** 对含合法 config.json 的工作区调用 `load`
- **THEN** 返回的 config 逐字段等于文件内容（passthrough 未知字段保留），diagnostics 为空

#### Scenario: 非法字段默认值填充并记 diagnostics

- **WHEN** config.json 含非法值（如 framework 为枚举外值、mutation score 为 200、suite root 含 `*`、`schema` 为非 `spec-driven` 字面量）
- **THEN** `load` 不失败：对应字段以默认值填充，diagnostics 逐项记录违例位置与所落默认值，其余合法字段原样保留

#### Scenario: 缺省字段默认值填充

- **WHEN** config.json 未设 coverage 阈值、未设 `tests`、未设 suite `cwd`
- **THEN** config 中 coverage 为 80 / 70 / 75、`tests` 为 `[]`、suite `cwd` 为 `.`，diagnostics 注明这些字段来自默认值而非文件

#### Scenario: 缺失与坏 JSON 不失败

- **WHEN** 工作区无 config.json，或 config.json 为非法 JSON 语法
- **THEN** `load` 均返回合法报告：config 为默认值填充，diagnostics 区分「文件缺失」与「JSON 语法非法」两种情形供上层呈现

#### Scenario: 路径经 foundation 取得

- **WHEN** 审查 `crates/core/config` 源码
- **THEN** config.json 路径来自 `foundation::layout` 的解析函数，crate 内无 `config.json` 字面量、无第二处路径拼接

### Requirement: workspace_config 命令轨道

Rust 侧 SHALL 新增 `commands/config/` 命令轨道，提供 `workspace_config(root: String) -> Result<WorkspaceConfigReport, String>`：

- 命令体 SHALL 为三件事薄包装（参数转换 / 调用 / 错误映射），领域组装收在 `workspace_config_inner` 纯函数（`*_inner` app 层微形态先例，可不经 Tauri 运行时直接测试）；
- 语义委托 core/config crate 的 `load`，命令层零自有校验 / 默认值规则；
- 无效 root（缺失 / 不可读 / 非目录 / 空白）SHALL 返回 `Err(String)`（对齐 code_stats 裁定），MUST NOT panic、MUST NOT 静默空报告；config.json 缺失 SHALL NOT Err——报告内以标记表达（多数 workspace 常态，空态由前端呈现）；
- 命令 MUST NOT 注册 store 模型、MUST NOT 落库、MUST NOT 持有任何缓存（内存或磁盘）——每次调用完整重读重校验；
- DTO SHALL `specta::Type` + `serde(rename_all = "camelCase")` 出线，经既有 bindings 重导出管线生成 typed 包装，前端 MUST NOT 以裸 `invoke('workspace_config')` 调用；
- 命令形态沿用 sync 命令惯例；如需 async + `spawn_blocking` 由 design 定夺并记录理由。

#### Scenario: 薄包装结构证明

- **WHEN** 审查 `commands/config` 实现与测试
- **THEN** 命令体为参数转换 + `workspace_config_inner` 调用 + 错误映射三段，测试以 serde_json 对照证明命令与 inner 结果一致（不经 Tauri 运行时）

#### Scenario: 无效 root 走 reject

- **WHEN** 以不存在的 root 调用 `workspace_config`
- **THEN** 命令返回 `Err(String)`，前端收到错误字符串，进程无 panic

#### Scenario: 缺文件返回空态报告

- **WHEN** 以有效但无 config.json 的工作区根调用 `workspace_config`
- **THEN** 命令返回 `Ok` 报告且文件缺失标记在案（非错误），前端据此呈现空态

#### Scenario: 绑定出线与 typed 调用

- **WHEN** 执行 bindings 重导出并审查前端调用点
- **THEN** `bindings.ts` 含 `workspaceConfig` 包装与 `WorkspaceConfigReport` 系 camelCase 类型，前端经生成绑定调用，无裸字符串 invoke

#### Scenario: 不落库无缓存

- **WHEN** 审查 `commands/config`、store 注册模型与全链代码
- **THEN** store 无新注册模型，命令无 memo / 静态缓存 / 磁盘缓存，连续两次调用各自完整读取解析

### Requirement: 配置页呈现面

前端 SHALL 在侧栏「页面」组新增 [配置] 入口（`data-testid="nav-config"`），经路由 `/config` 承载（路由与 NavLink 契约见 desktop-page-routing）。配置页 SHALL 以**当前工作区根**为数据基准（同基础信息页，根为 sidebar 本地态不进 URL）；仅壳态可达：欢迎态（root 为 null）MUST NOT 渲染壳与该入口。页面 SHALL 为**只读**：MUST NOT 提供任何写回 config.json 的操作入口。

呈现面 SHALL 分区：

- **基础配置**：`$schema` / `schema` / `context` / `static_analysis` / `rules`；
- **tests 面板**：逐 suite 呈现 root / framework / cwd / config / includes / excludes / coverage 阈值（lines / branches / functions）/ mutation（cwd / score）；未设阈值 SHALL 标注「未设（默认 N）」以区分文件值与默认值；
- **write_protection 面板**：逐条 glob 规则与 reason；
- **未知字段**：passthrough 字段可见（呈现形态 design 定夺）；
- **diagnostics 警示区**：单列呈现违例项（哪项非法、哪些字段吃了默认），与正常配置区视觉可分。

config.json 不存在 SHALL 呈现空态而非错误；坏 JSON 与命令 reject SHALL 按「错误呈现双轨」inline 持久呈现（testid 承载、无 toast 顶替）。

#### Scenario: 分区渲染

- **WHEN** 对含 tests 两个 suite 与 write_protection 规则的 fixture 工作区完成解析
- **THEN** 基础配置、tests 面板（逐 suite 全字段）、write_protection 面板各自呈现对应内容，字段值与文件一致

#### Scenario: 未设阈值标注默认

- **WHEN** 某 suite 的 coverage 未设 lines
- **THEN** 该阈值呈现为「未设（默认 80）」类标注，与文件显式设值的呈现形态可区分

#### Scenario: diagnostics 警示区

- **WHEN** config.json 含非法字段（吃默认）或未知字段
- **THEN** 警示区逐项列出违例与默认值来源，passthrough 未知字段仍在配置区可见，页面不崩溃

#### Scenario: 空态

- **WHEN** 工作区无 config.json
- **THEN** 页面呈现空态文案（非错误、无 error-note），说明该工作区未配置

#### Scenario: 错误态 inline 持久

- **WHEN** `workspace_config` reject 或报告携 JSON 语法非法情形
- **THEN** 页面 inline 持久呈现错误（testid 承载、不自动消失），无 toast 顶替

#### Scenario: 只读边界

- **WHEN** 审查配置页组件与命令轨道
- **THEN** 无编辑 / 保存 / 删除入口，命令轨道无任何写命令，全链无对 config.json 的写路径

#### Scenario: 切换工作区重取

- **WHEN** 用户在 `/config` 页点击 sidebar 另一 workspace 清单项
- **THEN** 页面以新根重新发起解析，旧根配置不再呈现

### Requirement: 显式刷新取数模型

取数 SHALL 收口在 `use-workspace-config` hook（`{ data, loading, error, refresh }` 形态），遵循显式刷新模型：进入页面发起一次解析；页面 SHALL 提供显式刷新入口（沿用 `disabled={loading}` 语义）。MUST NOT 引入轮询、文件 watch、事件订阅或任何缓存层；重复进入页面 SHALL 重新解析而非复用上一次结果。

#### Scenario: 显式刷新

- **WHEN** 用户点击刷新按钮
- **THEN** 重新发起一次完整解析，loading 期间刷新钮 disabled，无任何自动重取

#### Scenario: 无轮询无缓存

- **WHEN** 停留在配置页不操作
- **THEN** 无定时器重取、无 watch 订阅、无后台解析；离开再进入页面才产生下一次解析调用

### Requirement: 测试纪律与字面量隔离执法

测试 SHALL 只覆盖自研层：校验规则端口（枚举 / 值域 / 通配符禁令 / 字面量 / 默认值填充 / passthrough）、diagnostics 组装、命令薄包装、前端呈现与交互；MUST NOT 逐项断言 serde / serde_json 库自身的解析语义。

命名隔离执法 SHALL 扩展 `mod_test.rs` 扫描：产品源码 `.rs` 文件（排除 `*_test.rs`）含 `config.json` 文件名字面量即 panic，唯一例外 `foundation/src/layout/mod.rs`——与既有 `openspec` 目录名禁令并列执行（存在性 + 唯一性断言）。标识符与 DTO 命名沿用 desktop-crate-layout「代码命名隔离 openspec 字样」requirement（`WorkspaceConfig` 类命名，禁 `openspec_config` 类命名）。

desktop 前端 SHALL 维持全管线通过（`vp check --fix` / knip / `vp test`，`max-lines-per-function: 50` 与 data-testid 纪律不变），Rust 侧 `cargo test --workspace` 全绿。

#### Scenario: 扫描禁令生效

- **WHEN** 任一非 layout/mod.rs 的产品 `.rs` 源码引入 `config.json` 字面量并运行 Rust 测试
- **THEN** `mod_test` 隔离扫描 panic 指认违例文件；仅 layout/mod.rs 持有该字面量且断言其在场

#### Scenario: 测试边界

- **WHEN** 检索新增 Rust 与前端测试
- **THEN** 断言集中在自研校验端口、diagnostics 组装、命令包装与页面交互，无「serde 对某语法解析为某值」类库语义断言

#### Scenario: 管线通过

- **WHEN** 运行 `pnpm -C packages/desktop run client:check`、`run test` 与 `cargo test --workspace`
- **THEN** fmt / lint / knip / 测试全部通过，无新增豁免条目

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/config/`（新 crate，裸名 `config`） | 配置读取 + 校验 + 默认值（核心功能，未来模块唯一配置出口） | `load(root) -> ConfigReport { config, diagnostics }`；依赖仅 foundation + serde / serde_json / specta；禁 Tauri；校验语义复刻 CLI zod、吞错不复刻；config 输出永远合法 |
| `WorkspaceConfigReport` / `WorkspaceConfig` / diagnostics（DTO） | 出线数据面 | `specta::Type` + camelCase；信封 `config + diagnostics`；字段级形状与 diagnostics 粒度 design 定夺 |
| `foundation::layout`（常量组 + config_path） | config.json 路径唯一触点 | 常量组与 `config_path` 契约见 workspace-layout-resolution 增量 |
| `packages/desktop/src-tauri/src/commands/config/`（新轨道） | 配置解析命令轨道 | `workspace_config(root) -> Result<WorkspaceConfigReport, String>` 薄包装（三件事）；`workspace_config_inner` 纯函数；无效 root Err、缺文件空态标记；不落库无缓存 |
| `packages/desktop/src-tauri/src/bindings.rs` | 命令注册 | `collect_commands!` 增 `workspace_config`；bindings 重导出管线不变 |
| `packages/desktop/src/views/config/`（新） | 配置页视图域 | `config-view.tsx` + 分区组件（基础 / tests / write_protection / 未知字段 / diagnostics 警示）+ 空态与 inline 错误；只读 |
| `packages/desktop/src/views/config/hooks/use-workspace-config.ts`（新） | 取数收口 hook | `{ data, loading, error, refresh }` 形态；显式刷新模型；失败 inline |
| `packages/desktop/src/routes.tsx` + `src/components/app-sidebar.tsx` | 路由与入口 | `/config` 路由项；「页面」组 [配置] NavLink，`data-testid="nav-config"`，active 由 URL 派生 |
| `mod_test.rs` 隔离扫描（扩展） | 字面量执法 | `openspec` 与 `config.json` 双禁令；产品源码全扫（排除 `*_test.rs`）；唯一触点 layout/mod.rs |
