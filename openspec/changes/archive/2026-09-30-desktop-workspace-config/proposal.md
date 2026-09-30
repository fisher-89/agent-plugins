# 提案: desktop-workspace-config

> **变更**: desktop-workspace-config
> **日期**: 2026-09-29
> **状态**: proposed

---

## 问题

desktop 壳内没有任何一处呈现工作区的 `openspec/config.json`：测试阈值（coverage / mutation score）、suite 清单、write_protection 规则、项目 context 等配置，用户只能退出应用手动开文件对照 schema 猜语义。两个原始诉求经探索合为一个变更：

1. **工作区配置页**：解析工作区内 `openspec/config.json` 并只读呈现；
2. **`openspec` 目录名常量化**：将目录名字面量收成 Rust 全局常量，方便未来修改目录。

追加架构约束（用户裁决）：**读取并校验配置属于核心功能，未来其他模块仅允许从核心获取某项配置**——需要完整的 core/infra 归位，不落在 app 层。

现状盘点（构成差距的证据）：

- `foundation::layout::domain_dir_name()` 已是全包唯一 `openspec` 字面量触点，`layout_test.rs` 的**命名隔离扫描**（大小写敏感 `contains("openspec")`，产品源码 `.rs` 全扫，唯一例外 layout.rs）已是被测试强制的架构不变量——诉求 2 的目标「改名只改一处」**已达成**，真实差距是：fn → const 的形式收敛、`resolve()` 体内 `changes` / `archive` / `explores` 子目录名仍是行内字面量、`config.json` 文件名即将成为新触点；
- 配置语义的唯一权威在 CLI 侧：`plugins/dev-team/bin/src/schemas/config/config.schema.ts`（zod/v4：枚举 + 值域 + 通配符禁令 + prefault 默认值 + passthrough）与 `lib/config.ts` 的 `readConfig`（缺失 / 坏 JSON / 校验失败一律**静默回退默认值**）；Rust 侧零实现；
- `embedded-cli` 已于 2026-08-17 退役（`retire-openspec-bundled`），workflow 域全部 native Rust 解析——「shell 出调 CLI 取配置」路线不可行；
- `/info` 代码统计页提供了现成模板轨道：无状态解析命令 + `*_inner` 纯函数 + specta 生成 TS bindings + 显式刷新取数 + inline 错误态。

---

## 提案

新增 **`crates/core/config`** crate 承载配置语义（读 + 校验 + 默认值），foundation 收口全部路径字面量，desktop-app 出一条薄命令轨，前端加一个只读配置页：

```
crates/core/
├── foundation/    ← 常量组（域名 + changes/archive/explores + config.json）
│                    + resolve() + config_path() —— 全包唯一字面量触点
├── workflow/      ← change 域纯读（不动）
├── agent/         ← agent 域契约（不动）
└── config/ (新)   ← 读 + serde 解析 + 校验 + 默认值
      API: load(root) → ConfigReport { config, diagnostics }

src/commands/config/   workspace_config(root) 薄包装（stats 轨道模板）
bindings.rs            collect_commands! 注册 → export_bindings 生成 TS
前端 views/config/     view + use-workspace-config hook + components
routes.tsx / sidebar   /config + 「页面」组 [配置]
```

五个动作：

1. **foundation 常量组**：layout.rs 顶部设目录名常量组（域目录名 + `changes` / `archive` / `explores` 子目录名 + `config.json` 文件名），`resolve()` 与新增的 `config.json` 路径解析全部引用常量；`domain_dir_name()` fn 形态收敛为常量（消费点 `commands/stats` 随之更新）。
2. **core/config crate**：`load(root) -> ConfigReport { config, diagnostics }` 信封。校验语义**逐条复刻 CLI zod schema**（框架八值枚举 / 0–100 值域 / suite root 通配符禁令 / `schema` 字面量 / 默认值填充 / passthrough 未知字段）；默认值对齐 `defaults.ts`（coverage lines 80 / branches 70 / functions 75、mutation score 70、suite cwd `.`、tests `[]`、schema `spec-driven`）。**复刻校验、不复刻吞错**：CLI `readConfig` 对非法配置静默整体回默认，core 不复刻——非法输入 MUST NOT 使报告失败，config 字段永远合法（默认值填充），违例细节进 diagnostics；未来模块永远拿到合法配置，配置页拿 diagnostics 呈现「哪里非法 / 哪些字段吃了默认」。路径一律经 foundation 取得，crate 内不出现 `config.json` 字面量。
3. **命令轨道**：`commands/config/` 新轨，`workspace_config(root: String) -> Result<WorkspaceConfigReport, String>`——三件事薄包装 + `workspace_config_inner` 纯函数（stats 轨道先例）；无 State、无缓存、不落库；无效 root（缺失 / 不可读 / 非目录 / 空白）统一 `Err`（对齐 code_stats 裁定）；config.json 缺失不是错误（多数 workspace 常态），以报告内标记表达、页面呈现空态。
4. **bindings**：`collect_commands!` 注册 `workspace_config`，DTO `specta::Type` + camelCase 经既有重导出管线出线，前端 typed 调用。
5. **配置页**：`/config` 路由 + 「页面」组 [配置]（`nav-config`）；仅壳态可达；当前工作区根为数据基准（同 info 页，根不进 URL）；**只读**（不提供任何写回 config.json 的操作）；分区呈现基础字段 / tests 面板 / write_protection / 未知字段 passthrough，diagnostics 警示区单列，未设阈值标注「未设（默认 N）」；取数收在 `use-workspace-config` hook，显式刷新模型（无轮询 / 无 watch / 无缓存）。

**范围裁定**：

- **不做漂移防线**（用户裁决）：CLI 短期内会下线，Rust core/config 将成唯一实现，双实现长期维护不成立——`config.schema.ts` 仅作移植参照，不建 golden 对照双跑。
- **只读**：配置编辑会引入写回链路（write protection 自身被改的元问题），诉求仅是「看」。
- **一个 change**：常量组 + config_path + core/config crate + 命令轨 + bindings + 配置页 + spec 修订四处。
- **执法双保险**：依赖规则（`desktop-app → config → foundation`，未来模块仅准经 config 取配置）写入 desktop-crate-layout spec；`layout_test.rs` 命名隔离扫描扩展 `"config.json"` 字面量禁令（唯一例外 layout.rs）——绕开 `config::load` 直接读配置文件的代码连文件名都拼不出。
- 命名纪律：DTO 与标识符沿用「代码命名隔离 openspec 字样」requirement，取 `WorkspaceConfig` / `ConfigReport` 类命名，禁 `openspec_config` 类命名（扫描大小写敏感，snake_case 含 `openspec` 直接炸）。

---

## 能力

### 新增能力

- **desktop-workspace-config** — 工作区配置能力：core/config crate（`load(root) -> ConfigReport { config, diagnostics }`，校验语义复刻 CLI zod、默认值填充、passthrough、不复刻吞错）；`commands/config` 轨道 `workspace_config` 命令（薄包装、不落库无缓存、缺文件空态、无效 root Err）；`/config` 只读配置页（分区呈现 + diagnostics 警示 + 「未设（默认 N）」标注 + 空态 / inline 错误态 + 显式刷新取数）；layout_test 扫描扩展 `"config.json"` 字面量禁令。

### 修改的能力

- **desktop-crate-layout** — crate 三层分组增 `crates/core/config`（裸名 `config`）：依赖规则增 `desktop-app → config → foundation`、未来其他模块仅准经 config 取工作区配置；新增「配置文件名字面量隔离」requirement（产品源码禁 `"config.json"` 字面量，唯一触点 layout.rs）。
- **workspace-layout-resolution** — layout.rs 增目录名常量组（域名 + `changes` / `archive` / `explores` 子目录名 + `config.json` 文件名），`resolve()` 全量引用常量，`domain_dir_name` fn → 常量形态；新增 `config_path()` 路径解析（形态 design 定夺）；唯一字面量触点语义扩展到子目录名与配置文件名。
- **desktop-page-routing** — 路由表增 `/config → 工作区配置页`；「页面」组由 [基础信息] [变更] [探索] 三项扩为四项（增 [配置]）；测试挂钩新增 `nav-config`（既有 testid 不变）。

---

## 变更范围

### 实现文件

- `packages/desktop/src-tauri/crates/core/foundation/src/layout.rs`：常量组 + `config_path()`（Layout 加字段 vs 独立函数 design 定夺，倾向独立函数）+ `domain_dir_name` fn → 常量形态
- `packages/desktop/src-tauri/crates/core/config/`（新 crate）：`Cargo.toml`（仅 foundation + serde / serde_json / specta）+ `src/lib.rs`（`WorkspaceConfig` 系 DTO + `ConfigReport` + diagnostics + 校验 + 默认值填充 + `load`）
- `packages/desktop/src-tauri/Cargo.toml`：`[workspace].members` 增 `crates/core/config`；`[workspace.dependencies]` 增 `config = { path = ... }`；壳 `[dependencies]` 增 `config`
- `packages/desktop/src-tauri/src/commands/mod.rs`：挂载 `config` 轨道
- `packages/desktop/src-tauri/src/commands/config/mod.rs`（新轨道）：`workspace_config` 命令 + `workspace_config_inner` 纯函数
- `packages/desktop/src-tauri/src/bindings.rs`：`collect_commands!` 注册 `workspace_config`
- `packages/desktop/src/types/generated/bindings.ts`：重导出生成物（生成物不手改）
- `packages/desktop/src-tauri/src/commands/stats/mod.rs`：`domain_dir_name` 消费点改常量引用（fn → const 波及，语义零变化）
- `packages/desktop/src/routes.tsx`：`/config` 路由项
- `packages/desktop/src/components/app-sidebar.tsx`：「页面」组新增 [配置] NavLink（`data-testid="nav-config"`）
- `packages/desktop/src/views/config/`（新）：`config-view.tsx`、`components/`（分区呈现 / diagnostics 警示 / 空态 / 错误态）、`hooks/use-workspace-config.ts`
- `packages/desktop/package.json`：archive 时 bump 0.3.8 → 0.3.9（配置页为用户可见变更）

### 测试文件

- `packages/desktop/src-tauri/crates/core/foundation/src/layout_test.rs`：常量组断言（resolve 全量引用、config_path 拼接）+ 命名隔离扫描扩展（产品源码含 `"config.json"` 字面量即 panic，唯一例外 layout.rs）
- `packages/desktop/src-tauri/crates/core/config/src/`（新测试）：校验端口矩阵（框架枚举 / 0–100 值域 / 通配符禁令 / `schema` 字面量 / 默认值填充 80·70·75·70·`.`·`[]` / passthrough）、缺失文件与坏 JSON 的 diagnostics 组装、DTO serde 序列化
- `packages/desktop/src-tauri/src/commands/config/mod_test.rs`（新）：thin-wrapper 结构证明（serde_json 对照 inner）；无效 root Err 路径；缺文件空态标记
- `packages/desktop/src-tauri/src/bindings_test.rs`：命令清单 23 → 24、`workspaceConfig` 包装与新 DTO 类型名断言
- `packages/desktop/src/views/config/hooks/use-workspace-config.test.ts`（新）：invoke mock；取数 / 刷新 / 失败 inline
- `packages/desktop/src/views/config/config-view.test.tsx`（新）：分区渲染 / 「未设（默认 N）」标注 / diagnostics 警示 / passthrough 呈现 / 空态 / inline 错误，data-testid 挂钩
- `packages/desktop/src/components/app-sidebar.test.tsx`：`nav-config` 渲染与 active 态
- `packages/desktop/src/app.test.tsx`：`/config` 路由可达与欢迎态隔离用例

### 删除文件

- 无

### 不要修改

- `crates/core/workflow` / `crates/core/agent` / `crates/infra/**` 全部 crate 零改动（本变更无既有 crate 消费 config，「未来模块 → config」依赖留待真实消费者出现，不预铺）
- store：不注册新模型、不触碰 db 文件（无缓存即无新落库维度）
- `plugins/dev-team/bin/src/schemas/config/**` 与 `lib/config.ts`：只读语义参照，不改（漂移防线已裁定不做）
- 既有命令轨道（queries / exec / explores / watch / workspaces / db）语义；stats 轨道仅常量消费点形态更新、统计语义零变化
- desktop-app-shell 取数模型：不引入文件 watch、轮询、事件订阅（配置页无任何推送例外）
- desktop-page-routing 既有路由项与未知路径兜底语义（`*` → `/changes`）
- `tests/golden/**` 与 workflow 域 golden 契约（本变更不触 workflow crate）

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | `foundation/src/layout.rs` + `layout_test.rs` | layout.rs 顶部常量组覆盖域目录名、`changes` / `archive` / `explores` 子目录名、`config.json` 文件名；`resolve` 与 `config_path` 全量引用常量；扩展后的隔离扫描对产品 `.rs` 源码（排除 `*_test.rs`）执行 `openspec` 与 `config.json` 双禁令，唯一例外 layout.rs |
| AC-2 | `crates/core/config/` | crate 依赖仅 `foundation` + `serde` / `serde_json` / `specta`，无 Tauri、无 workflow / agent / infra 依赖；`load` 对合法配置原样返回且 diagnostics 为空；对非法字段（枚举外框架名、越界阈值、含通配符的 suite root、`schema` 字面量不符）产出默认值填充的合法 config + 对应 diagnostics；passthrough 未知字段保留；文件缺失以报告内标记表达、不视为失败 |
| AC-3 | `commands/config/` + `bindings.rs` | `workspace_config` 经 `collect_commands!` 注册；重导出后 `bindings.ts` 含 `workspaceConfig` 包装与 `WorkspaceConfigReport` 系 camelCase 类型；前端经生成绑定 typed 调用，无裸 `invoke('workspace_config')`；命令无 State / 无缓存 / 不落库 |
| AC-4 | `routes.tsx` + `app-sidebar.tsx` | 壳态下「页面」组出现 [配置]（`data-testid="nav-config"`），点击后 URL 为 `/config` 并渲染配置页；欢迎态（root 为 null）无壳无该入口 |
| AC-5 | `config-view.tsx` + `workspace_config_inner` | 配置页分区呈现基础字段 / tests 面板（逐 suite：root / framework / cwd / config / includes / excludes / coverage / mutation）/ write_protection / 未知字段；未设阈值标注「未设（默认 N）」；diagnostics 警示区单列呈现非法项与吃默认项 |
| AC-6 | `use-workspace-config` | 进入页面发起一次解析；显式刷新入口（`disabled={loading}`）重取；无轮询 / watch / 事件订阅 / 任何缓存层；切换 workspace 以新根重取 |
| AC-7 | 状态面 | config.json 不存在呈现空态而非错误；坏 JSON / 命令 reject 呈现 inline 持久错误（testid 承载、无 toast 顶替）；无效 root 返回 `Err` 且进程无 panic |
| AC-8 | 全链审查 | 全包除 layout.rs 外无 `openspec` 与 `config.json` 字面量；crate 图机械保证 `config` 无 Tauri、不依赖 workflow / agent / infra；页面无任何写回 config.json 的操作入口 |
| AC-9 | 管线 | `pnpm -C packages/desktop run client:check`、`run test`、`cargo test --workspace` 全绿；knip / lint 无新增豁免条目；新增 Rust 测试无 serde / JSON 库自身语义断言 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| zod 语义移植偏差（prefault 与 optional 的默认时机、refine 顺序、literal 失败路径） | 校验行为与 CLI 不一致，页面呈现失真 | 中 | 逐条对照 `config.schema.ts` 移植并建单测矩阵（AC-2）；CLI 下线前以 CLI 行为为参照逐案核对 |
| serde 反序列化与 zod 的失败粒度天然不同（字段级 vs 整体） | 「永远合法 config」承诺实现走样 | 中 | 校验实现方式（`deserialize_with` 逐字段 vs 事后校验 pass）design 定夺并留档；不变量以测试钉死：任意输入产出通过同一套校验的 config |
| `"config.json"` 字面量禁令误伤（doc 注释、日志文案、测试 fixture） | 扫描炸到合法代码 | 低 | 扫描沿用 `*_test.rs` 排除先例；注释是否纳入禁令范围 design 一并裁定（当前 openspec 扫描按全文匹配，注释同样禁） |
| core/config 与 CLI 双实现短期漂移 | 同一配置两边解读不同 | 低 | 漂移防线已裁定不做：CLI 短期下线、Rust 成唯一实现；移植期以 schema.ts 为准 |
| workspace 普遍无 config.json | 页面多数时候空态，显得「坏」 | 低 | 空态为裁定语义（非错误）；空态文案说明默认值行为，与 diagnostics 警示区分 |
| DTO 命名踩 openspec 隔离不变量 | 扫描炸、返工 | 低 | 命名纪律入 spec（`WorkspaceConfig` 类 CamelCase）；扫描本身即守门 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| 配置读取落位 | 新 `crates/core/config` crate（裸名 `config`） | 用户裁决：读取并校验配置属核心功能，未来其他模块仅准从核心取配置；「第二消费者预期在先」使 foundation 单消费者纪律不构成反例 | app 层 `*_inner` 微形态（code-stats 先例）——未来模块无法依赖 app 层，弃 |
| CLI 吞错语义是否复刻 | 复刻校验、不复刻吞错：`ConfigReport { config, diagnostics }` 信封 | CLI 静默整体回默认让非法配置不可见；核心承诺「永远拿到合法配置」+ 页面拿到 diagnostics 可归因 | 逐字复刻吞错（非法配置对用户不可见，弃）；缺失 / 坏 JSON 直接 Err（把常态当错误，弃） |
| 常量化范围 | 完整版常量组：域名 + `changes` / `archive` / `explores` + `config.json`，`resolve` / `config_path` 全量引用 | 「改名只改 layout.rs 一处」目标一步到位；config.json 即将成为新触点，一次收口免二次改 | 仅加 config.json 常量（resolve 体内子目录字面量残留，弃） |
| 「仅准从核心取配置」执法 | 依赖规则入 spec + `"config.json"` 字面量扫描禁令双保险 | 依赖规则靠评审、字面量禁令靠测试机械保证；绕开 core/config 者连文件名都拼不出 | 仅依赖规则（可静默绕过，弃） |
| spec 承载 | 四处 delta：desktop-workspace-config（新）+ desktop-crate-layout + workspace-layout-resolution + desktop-page-routing | 路由表枚举与「页面」组 SHALL 组成都在 page-routing（desktop-workspace-code-stats 先例）；crate 规则与字面量隔离在 crate-layout；常量组与 config_path 契约在 layout-resolution。explore 笔记记三处，本提案补正第四处（page-routing），否则旧 SHALL 直接冲突 | 只写新能力 spec（page-routing 旧 SHALL 冲突，弃） |
| 页面形态 | 只读 + 显式刷新（复刻 use-code-stats 取数模型） | 诉求仅是「看」；编辑会引入写回链路与 write protection 自治元问题 | 可编辑配置页（范围失控，弃）；轮询 / watch 自动刷新（无推送例外先例，弃） |
| 与 CLI 的漂移防线 | 不做 | CLI 短期内下线，Rust core/config 将成唯一实现，双实现长期维护不成立（用户裁决） | golden 对照双跑（维护成本沉没，弃） |
| 无效 root 语义 | 统一 `Err`（缺失 / 不可读 / 非目录 / 空白） | 对齐 code_stats 裁定：壳态 root 恒有值，blank 只能来自调用 bug；与 config.json 缺失（常态、空态）分层 | blank root 返回默认配置报告（掩盖调用 bug，弃） |

### 待决问题

- `config_path()` 形态：Layout 加第四字段 vs 独立函数（倾向独立函数——config.json 是文件不是目录树）
- `domain_dir_name` fn → 常量的公开形态（导出 `pub const` vs 保留 fn 包装）与 stats 消费点更新方式
- DTO 字段形状与 diagnostics 粒度（逐字段 vs 逐条消息）；校验失败的 suite 处置（剔除并记 diagnostics vs 降级保留可呈现形态）
- 校验实现方式（serde `deserialize_with` 逐字段 vs 事后校验 pass）
- 坏 JSON 的命令通道（`Err` reject vs 报告内 fatal diagnostics 由页面判定；两通道下核心均产出合法默认配置）
- 配置页分区组件拆分与「页面」组内排序

---
