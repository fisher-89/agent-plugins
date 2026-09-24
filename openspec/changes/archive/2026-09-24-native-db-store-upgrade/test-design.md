# 测试设计: native-db-store-upgrade

> **日期**: 2026-09-23

---

## 验收范围

<!-- 覆盖 proposal.md 全部 8 个 AC。测试命令不写入本文档（用户约束）；
native_db / native_model / redb / serde 库自带语义不逐项测试，只测自研
提取/组装层（用户约束）。 -->

| AC ID | 验收条件 | 测试类型 | 被测文件或模块 |
|--------|---------|---------|----------|
| AC-1 | store 引擎升级 native_db + 模型层：src-tauri workspace 全量测试通过；注册表与 agent run 落库/重放的存量场景行为不变 | 单元测试 + 集成测试 | `packages/desktop/src-tauri/crates/infra/store/src/store_test.rs`、`packages/desktop/src-tauri/crates/infra/store/src/model_test.rs`（存量回归 + 轮账用例废弃）；集成关系「legacy 旧库格式 → Store::open 探测 → migrate 一次性迁移 → native_db 新库」「exec 事件 tee → store 类型化事件表 → 命令面重放」 |
| AC-2 | 旧 redb 格式 fixture 库首启自动迁移：各表记录数一致、解不出的历史事件行落 `Raw`、旧文件改名 `.bak` 存在 | 集成测试 | 集成关系「legacy 旧库格式 → Store::open 探测 → migrate 一次性迁移 → native_db 新库」→ `packages/desktop/src-tauri/crates/infra/store/src/migrate_test.rs` |
| AC-3 | `schema_version` 退役：store 源码无 META 轮账代码；模型 shape 演进治理经 native_model 版本机制 | 单元测试 + 集成测试 | `packages/desktop/src-tauri/crates/infra/store/src/store_test.rs`（轮账用例废弃）；集成关系 R1 场景「user_meta 不迁移」（源码级无轮账代码为静态审查项，见不可测试项） |
| AC-4 | `AgentEventRecord` 嵌装建模：`crates/core/agent` 零 derive 改动；store 公共 API / desktop 命令签名 / 前端 DTO 无任何 native_db 类型 | 单元测试 + 集成测试 | `packages/desktop/src-tauri/crates/infra/store/src/model_test.rs`（键打包 + 嵌装往返）；集成关系「exec 事件 tee → store 类型化事件表 → 命令面重放」（五变体端到端）；git diff 为空与类型不泄漏的静态面见不可测试项 |
| AC-5 | 记录信封 API：`list_models` / `scan` 落地并经 `db_models` / `db_records` 命令可达；分页扫描返回 `RecordEnvelope{key, value}`（JSON Value） | 单元测试 + 集成测试 | `packages/desktop/src-tauri/crates/infra/store/src/envelope_test.rs`、`packages/desktop/src-tauri/crates/infra/store/src/store_test.rs`；集成关系「store 信封 API → db 命令轨道」→ `packages/desktop/src-tauri/src/commands/db/mod_test.rs` |
| AC-6 | 系统工具组 + DB 查看页：壳态侧栏渲染「系统工具」组（[Agent 调试] [DB 查看]），「页面」组仅 [变更]；欢迎态无该组 DOM；点击切换本地 state 无路由 | 单元测试 + 集成测试 | `packages/desktop/src/components/AppSidebar.test.tsx`、`packages/desktop/src/App.test.tsx`；集成关系「侧栏系统工具组 → App 顶层切换 → DbInspectorView 挂载」→ `packages/desktop/src/__tests__/db_inspector_nav.test.tsx` |
| AC-7 | 查看器只读四件套：模型清单 + 计数、分页扫描、单条 JSON 查看可用；查看器与 db 轨道命令无任何写操作入口 | 单元测试 + 集成测试 | `packages/desktop/src/views/db/DbInspectorView.test.tsx`、`packages/desktop/src/hooks/useDbInspector.test.ts`；集成关系「DbInspectorView → useDbInspector → db_models/db_records IPC」→ `packages/desktop/src/__tests__/db_inspector_pipeline.test.tsx` |
| AC-8 | 依赖规则修订落档：store `Cargo.toml` workspace 内依赖仅新增 `agent`；desktop-crate-layout spec 修订可考 | 不可自动化（静态审查） | 见不可测试项（manifest 与 spec 文档无运行时行为可断言） |

框架识别结论（`test_detect_frameworks`）：TS/TSX 侧 vite-plus（jsdom + testing-library，断言经 `vite-plus/test`）；Rust 侧 rust 套件（注册于 `packages/desktop/src-tauri`，workspace 全 crate 覆盖）。仓库既有模式：Rust 集成用例与 `*_test.rs` 模块共置（binary crate 无库目标，沿 `commands/workspaces/mod_test.rs` 惯例）；前端跨模块集成收 `src/__tests__/`。

---

## 单元测试

<!-- 覆盖进程内可验证场景。存储层不 mock（tempfile 真开库，沿 store_test.rs
既有「存储层不 mock」哲学）；IPC 边界在前端以 invoke mock 替身。 -->

### packages/desktop/src-tauri/crates/infra/store/src/model.rs -> packages/desktop/src-tauri/crates/infra/store/src/model_test.rs

#### 待测功能

<!-- design.md 公共函数表无 model.rs 行；以下条目来自 design.md「类型定义」表
与「数据模型」节的 AgentEventRecord 声明。 -->

- `AgentEventRecord`（类型定义）: `event_key: u128` 合成主键（`(run_id as u128) << 64 | seq` 打包）、`run_id: i64` 二级索引、`event: agent::AgentEvent` 嵌装载荷（core 类型 derive-free 原样引用）
- `WorkspaceRecord` / `AgentRunRecord`: 现状平移，仅加 `#[native_model(id = 1/2, version = 1)]` + `#[native_db]`，手写 `encode` / `decode` 删除

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| AgentEventRecord 键打包 | 正向 | 常规 `(run_id, seq)` 组合打包后保序：同 run 内 seq 增大则 `event_key` 严格增大；run_id 增大则整段键区间上移（扫描自然序即重放序的前提） | 新增 |
| AgentEventRecord 键打包 | 边界 | 最小键 `run_id=0, seq=0` 与最大键 `run_id=i64::MAX, seq=u64::MAX` 打包不溢出不回绕 | 新增 |
| AgentEventRecord 键打包 | 边界 | 相邻 run 边界不串键：`run_1` 的 `seq=u64::MAX` 键小于 `run_2` 的 `seq=0` 键（高 64 位隔离） | 新增 |
| AgentEventRecord 嵌装往返 | 正向 | 五变体（`RunStarted` / `Message` / `SystemNotice` / `RunResult` / `Raw`）各构造一条嵌装记录，经模型封装编解码往返后逐字段相等（验证 store 包装层与 core flatten 类型组装兼容，不重复验证 serde 自身语义） | 新增 |
| AgentEventRecord 嵌装往返 | 边界 | `Raw` 变体（`event_type` + `raw_json` 原文含中文/emoji/引号）嵌装往返保真 | 新增 |
| 既有 model_test 存量用例 | 废弃 | 既有 camelCase 键面、全字段往返、None 形态、特殊字符保真、status 受控串、非法 JSON decode 等手写编解码用例随 `encode` / `decode` 删除而废弃（native_model 编解码接管，落库往返由 `store_test.rs` 经 Store 公共 API 承载） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 纯类型层，全部用例为内存构造，无 IO 无进程边界 | 本文件全部用例 |

---

### packages/desktop/src-tauri/crates/infra/store/src/store.rs -> packages/desktop/src-tauri/crates/infra/store/src/store_test.rs

#### 待测功能

<!-- 来自 design.md「公共函数 / API」表中 所在文件 = store.rs 的全部行。 -->

- `Store::open()`: 签名不变，行为升级——native_db 打开 + legacy 格式探测迁移；仍 fail fast、路径完全由入参注入
- `Store::add_workspace()`: upsert 语义不变（`added_at` 保留首添值），载体换 native_db
- `Store::list_workspaces()`: PK（canonical root）自然序不变
- `Store::remove_workspace()`: 语义不变（含消失目录词法回退、miss 幂等）
- `Store::begin_agent_run()`: `last()+1` 自增分配语义不变
- `Store::finish_agent_run()`: 语义不变
- `Store::list_agent_runs()`: `started_at` 降序 + id 降序内存排序维持
- `Store::append_agent_run_events()`: 入参类型化 `&[AgentEvent]`；`seq` 取自事件本体，「缺 seq」错误路径消失
- `Store::list_agent_run_events()`: 返回类型化 `Vec<AgentEvent>`；经 `run_id` 二级索引扫描，seq 升序
- `Store::list_models()`: 注册表驱动返回全部已注册模型清单与计数（计数 0 也列出）
- `Store::scan()`: 主键自然序流式扫描（`skip(offset).take(limit)`），`limit` 上限 500 截断；未知模型名 `Err`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| Store::open | 正向 | 全新路径 open 创建 native_db 新库并建父目录；`list_models` 返回全部注册模型且计数为 0 | 新增 |
| Store::open | 正向 | native 格式已有库重开直通（不再进 legacy 探测分支），此前写入的记录完整读回 | 新增 |
| Store::open | 异常 | 迁移中途失败（旧库读断 / 写新失败 / 改名失败注入点）时 open 返回 `StoreError::Db` 且错误串携带「迁移:」语境前缀，旧文件保持原样或回退 | 新增 |
| Store::open | 废弃 | `schema_version等于当前版本时open幂等成功` / `schema_version高于支持版本时open返回err` / `全新库直读user_meta表schema_version` / `裸redb只读句柄可见user前缀两agent表` 四个存量用例随轮账退役与表名前缀语义消失而废弃 | 废弃 |
| Store::add_workspace / list_workspaces / remove_workspace | 边界 | 既有等价路径去重（大小写、尾分隔符、正反斜杠）、canonical 序、消失目录词法回退、miss 幂等、unicode 保真、重开持久等存量用例全部保留回归（载体更换后行为不变，即 AC-1 的「存量场景行为不变」），不新增用例 | 新增 |
| Store::begin_agent_run / finish_agent_run / list_agent_runs | 边界 | 既有 id 自增分配、入参 id 被覆盖、终态整行替换、`started_at` 降序并列 id 降序、空库空向量等存量用例全部保留回归，不新增用例 | 新增 |
| Store::append_agent_run_events | 正向 | 类型化入参 `&[AgentEvent]` 五变体批量追加成功，经 `list_agent_run_events` 重放 seq 升序、五变体逐字段保真 | 新增 |
| Store::append_agent_run_events | 边界 | 同 run 重复 seq 二次追加后重放恰一条（合成主键覆盖语义，与旧载体行为一致） | 新增 |
| Store::append_agent_run_events | 边界 | 单 run 千级 seq 连续追加后重放序完整不回绕（u128 键打包在大 seq 下的保序性） | 新增 |
| Store::list_agent_run_events | 正向 | 两 run 同 seq 区间互不串扰（经 `run_id` 二级索引扫描隔离，替代旧复合键半开区间实现） | 新增 |
| Store::list_models | 正向 | 写入 workspace / run / event 三模型数据后 `list_models` 计数与各模型实有记录数一致；空库时三模型照列且计数 0 | 新增 |
| Store::scan | 正向 | 按 offset / limit 分页返回 `RecordEnvelope{key, value}`，主键自然序，翻页拼接不重不漏 | 新增 |
| Store::scan | 异常 | 未知模型名（含空串）返回 `Err` 不 panic | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 文件系统（db 文件） | 不 mock——tempfile 真开 native_db 库（沿既有「存储层不 mock」哲学），`Env` 装置提供 db 路径与重开场景 | 本文件全部用例 |
| 旧 redb 手写格式库 fixture | 测试侧以裸 redb 句柄直写 `user_workspaces` / `user_agent_runs` / `user_agent_run_events` 三表构造旧格式库（镜像旧表定义常量；含合法行与损坏行），仅作迁移探测的输入数据，非被 mock 的依赖 | `Store::open` 迁移探测/失败用例 |
| 系统时钟 | 不 mock——`added_at` / `started_at` 由调用方或 now 注入，断言与其无耦合（沿既有口径） | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/store/src/migrate.rs -> packages/desktop/src-tauri/crates/infra/store/src/migrate_test.rs

#### 待测功能

<!-- design.md 公共函数/类型定义表未声明 migrate.rs 的公共 API（crate 私有，
经 Store::open 调用）；职责描述来自 design.md「新增文件」表。迁移全链路的
用例设计收敛于集成关系「legacy 旧库格式 → Store::open 探测 → migrate 一次性
迁移 → native_db 新库」（测试文件即本文件），此处不重复列表。 -->

- 格式探测（native_db 打开失败 → redb read_only 探测）、三表读出与转换（事件行解不出包 `AgentEventKind::Raw`、`seq` 取自旧表复合键、`timestamp_ms` 取 JSON `timestampMs` 或 0）、临时文件建库 + 两段改名 + 失败回退：经集成关系覆盖

#### 用例

<!-- 见集成关系 R1 的场景用例表（同文件承载），此处不重复。 -->

#### Mock策略

<!-- 见集成关系 R1 的 Mock策略。 -->

---

### packages/desktop/src-tauri/crates/infra/store/src/envelope.rs -> packages/desktop/src-tauri/crates/infra/store/src/envelope_test.rs

#### 待测功能

<!-- design.md「公共函数 / API」表中 `Store::list_models` / `Store::scan`
的实现体落在本文件（Store 方法委托至此）；类型来自「类型定义」表。 -->

- `Store::list_models()`（实现体）: 静态模型注册表驱动，返回 `Vec<ModelInfo>`（name + count，计数 0 也列出）
- `Store::scan()`（实现体）: 主键自然序流式扫描、limit 500 上限截断、key/value 均 `serde_json::Value`、native_db 类型不越信封
- `ModelInfo` / `RecordEnvelope`: serde camelCase 信封类型

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 注册表 | 正向 | 注册表覆盖三个已注册模型（workspaces / agent_runs / agent_run_events），`list_models` 全列且计数与写入量一致 | 新增 |
| scan 分页 | 边界 | `offset=0` 首页、`offset` 恰等于记录总数、`offset` 超过记录总数（含 `u32::MAX`）三种形态返回页内容正确（末者为空数组不报错） | 新增 |
| scan 分页 | 边界 | `limit=0` 返回空数组；`limit` 大于剩余记录数返回剩余全部；`limit` 超过 500 截断为 500（上限语义） | 新增 |
| scan 信封 | 正向 | 三模型各自的 key 信封口径：`WorkspaceRecord` → root 字符串、`AgentRunRecord` → id 数值、`AgentEventRecord` → `event_key`（超 u64 范围的 u128 键仍序列化为合法 JSON 形态，不出现序列化失败或二进制乱码） | 新增 |
| scan 信封 | 正向 | value 信封为结构化 JSON（camelCase 字段面与落库线格式一致），人可读、无 bincode 二进制字节串泄漏（desktop-db-inspector「人可读呈现」的 store 侧证词） | 新增 |
| scan 异常 | 异常 | 未知模型名 / 空串模型名返回 `Err`；错误串含模型名便于排查 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 文件系统（db 文件） | 不 mock——tempfile 真开库并经 Store 写入构造数据（存储层不 mock） | 全部用例 |

---

### packages/desktop/src-tauri/src/commands/db/mod.rs -> packages/desktop/src-tauri/src/commands/db/mod_test.rs

#### 待测功能

- `db_models()`: 只读薄包装（`State<Store>` → `list_models` → DTO），错误约定 `Result<T, String>`
- `db_records()`: 只读薄包装（`State<Store>` → `scan` → DTO），`Err` 由 Tauri 转前端 reject

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| db_models | 正向 | 写入数据后命令面返回与直连 `store.list_models()` 的 serde 值一致（薄包装不加工） | 新增 |
| db_models | 边界 | 空库命令面返回全模型清单且计数 0（空态可呈现的命令面前提） | 新增 |
| db_records | 正向 | 命令面返回与直连 `store.scan()` 的 serde 值一致；key/value 均为 JSON 值 | 新增 |
| db_records | 正向 | offset / limit 透传不加工：连续翻页（0..limit、limit..2·limit、…）拼接覆盖全部记录不重不漏 | 新增 |
| db_records | 异常 | 未知模型名返回 `Err(String)`，错误串前缀与 store 层一致（不静默吞错） | 新增 |
| db_records | 边界 | 大 offset（越界）返回空数组而非错误 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| Tauri runtime | `tauri::test::mock_app()`（MockRuntime，无窗口无事件循环）manage 真实 Store（tempfile 真库）后经 `app.state::<Store>()` 取 State，不启动真实进程 | 全部用例（沿 `commands/workspaces/mod_test.rs` 惯例） |

---

### packages/desktop/src/components/AppSidebar.tsx -> packages/desktop/src/components/AppSidebar.test.tsx

#### 待测功能

<!-- design.md「类型定义」表声明 TopPage 扩展；组件内部组（PageNavGroup /
SystemToolsGroup）为非导出实现，用例依据「修改文件」表的修改内容声明设计。 -->

- `TopPage`: `'changes' | 'agent' | 'db'`（增 `db` 变体）
- 页面导航组收敛为仅 [变更]；新增「系统工具」组（[Agent 调试] 平移 + [DB 查看]，`data-testid="nav-db"`）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 系统工具组 | 正向 | 壳态渲染「系统工具」组标签，组内含 nav-agent 与 nav-db 两项，nav-db 携带 lucide 图标（svg 结构断言，沿既有口径） | 新增 |
| 系统工具组 | 正向 | 点击 nav-db → `onPageChange("db")` 恰一次；点击 nav-agent → `onPageChange("agent")` 恰一次 | 新增 |
| 系统工具组 | 正向 | `page="db"` 时 nav-db 呈激活态，nav-changes / nav-agent 不激活 | 新增 |
| 页面导航组收敛 | 正向 | 「页面」组仅含 nav-changes（nav-agent 从「页面」组消失、归属「系统工具」组，组归属以 DOM 结构断言） | 新增 |
| 页面导航组收敛 | 废弃 | 既有「页面导航组」describe 中「页面」组含 [变更][Agent 调试] 两项、`page='agent'` 时 nav-agent 在页面组等断言随组重组废弃/改写 | 废弃 |
| 语义不串扰 | 边界 | `page="db"` 时 workspace 清单组照常渲染、激活态与 onOpen/onAdd/onRemove 回调不受切页影响（既有语义回归） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| jsdom 环境缺口 | 沿既有 `stubEnvironment()`：matchMedia stub、ResizeObserver stub、innerWidth 固定；组件纯回调驱动（`onPageChange` 等以 `vi.fn()` 注入） | 全部用例 |

---

### packages/desktop/src/App.tsx -> packages/desktop/src/App.test.tsx

#### 待测功能

<!-- design.md 公共函数/类型定义表未声明 App 的 API 行；用例依据「修改文件」
表修改内容声明（顶层视图切换支持 db，三分支渲染）设计。 -->

- 顶层视图切换支持 `db`：三分支渲染 `ChangeView` / `AgentDebugView` / `DbInspectorView`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| db 分支渲染 | 正向 | 壳态 `page="db"` 时主内容区渲染 DbInspectorView（其余两分支不挂载） | 新增 |
| 欢迎态不可达 | 边界 | `root=null`（欢迎态）不挂载壳与侧栏：DOM 无「系统工具」组，DB 查看页不可达（欢迎屏独立页面语义） | 新增 |
| 切页不触发取数 | 边界 | 切至 db 页再切回，`list_changes` 等命令调用次数不增长（useChangeList 留 App 层、不随 page 重取） | 新增 |
| 既有切换用例 | 废弃 | 既有「changes/agent 两分支」断言若与三分支结构耦合（如「仅两分支」计数类断言）随改造废弃/改写，其余保留回归 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC 边界 | `vi.mock('@tauri-apps/api/core')` invoke 按命令名分发 fixture 并记录调用序列（次数断言依赖记录），沿 App.test.tsx 既有装置；matchMedia / 视口 stub 同源 | 全部用例 |

---

### packages/desktop/src/views/db/DbInspectorView.tsx -> packages/desktop/src/views/db/DbInspectorView.test.tsx

#### 待测功能

- `DbInspectorView()`: 只读查看页组件——模型清单（含计数，0 计数空态）、选中模型后分页扫描、单条记录 JSON 查看（key 与 value 信封完整呈现）、翻页控件、inline 持久错误区

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 模型清单 | 正向 | `models` 数据渲染清单：每行模型名 + 计数；loading 态呈现进行中形态 | 新增 |
| 模型清单 | 边界 | 计数为 0 的模型呈现空态而非错误（desktop-db-inspector「清单与计数」场景） | 新增 |
| 分页扫描 | 正向 | 选中模型后记录区按 offset/limit 呈现信封行（key + value 摘要）；nextPage/prevPage 控件按 `hasMore` / `offset` 边界启用或禁用 | 新增 |
| 分页扫描 | 边界 | 空记录页呈现空态（非错误、非白屏） | 新增 |
| 单条查看 | 正向 | 点开单条记录：以 `JSON.stringify(value, null, 2)` 形态完整呈现 key 与 value，字段名可读、无二进制乱码（「人可读呈现」场景） | 新增 |
| 错误态 | 异常 | `recordsError` 置位时 inline 持久错误区呈现、页面不白屏、已有清单不丢失；无任何 toast 调用（查询轨「错误呈现双轨」语义） | 新增 |
| 只读边界 | 边界 | 渲染树不存在任何写操作入口（无新增/修改/删除记录类按钮；可交互控件仅选择、翻页、刷新类） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `useDbInspector` hook | `vi.mock` 返回受控 fixture state（组件纯 state 驱动，沿 `AgentDebugView.test.tsx` 模式），不含 IPC 边界 | 全部用例 |

---

### packages/desktop/src/hooks/useDbInspector.ts -> packages/desktop/src/hooks/useDbInspector.test.ts

#### 待测功能

- `useDbInspector()`: 挂载取模型清单一次；`selectModel` 重置 offset 取第一页；`nextPage` / `prevPage` 翻页；`refresh` 重取当前页；返回 `DbInspectorState`（`models` / `loading` / `error` / `selected` / `records` / `recordsLoading` / `recordsError` / `offset` / `hasMore` + 动作）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 挂载取数 | 正向 | 挂载即 `invoke('db_models')` 恰一次（进入页面即用户显式动作语义），`loading` 置位后随成功清除 | 新增 |
| 挂载取数 | 边界 | 取数链稳定：挂载后调用数不再增长（无轮询、无恢复循环，沿 useWorkspaces.test.ts 稳定判据模式） | 新增 |
| selectModel | 正向 | `selectModel(name)` 重置 offset=0 并以 `{model, offset: 0, limit}` 取第一页；连续两次 selectModel 时两次动作均触发取数、最终状态与最后选择一致 | 新增 |
| 翻页 | 正向 | `nextPage` / `prevPage` 以 offset 步进取对应页；`refresh` 以当前 offset 重取 | 新增 |
| 翻页 | 边界 | 末页（`hasMore=false`）时 nextPage 不再触发取数；首页 prevPage 不触发负 offset 取数；记录数恰为整页倍数时 hasMore 判定使翻页拼接不重不漏 | 新增 |
| 错误态 | 异常 | `db_models` reject → `error` 置位含错误串、models 保持空数组、无 toast 调用（hook 不引 sonner，错误 inline 由视图承载） | 新增 |
| 错误态 | 异常 | `db_records` reject → `recordsError` 置位、已选模型与清单不丢失、无未捕获异常 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC 边界（`@tauri-apps/api/core`） | `vi.mock` invoke 按命令名（`db_models` / `db_records`）分发固定 fixture 并记录调用序列；reject 用例以 `mockRejectedValue` 注入 | 全部用例（沿 useWorkspaces.test.ts 替身模式；查询轨无 toast，无需 sonner mock） |

---

### packages/desktop/src-tauri/crates/infra/store/src/lib.rs -> packages/desktop/src-tauri/crates/infra/store/src/lib_test.rs

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（纯接线与文档：模块声明、导出增
AgentEventRecord / ModelInfo / RecordEnvelope、crate 文档重写）。导出正确性
由编译与其余用例的导入路径承载，不设独立用例。 -->

#### 用例

<!-- 无用例：接线与文档变更，无运行时行为。 -->

#### Mock策略

<!-- 无。 -->

---

### packages/desktop/src-tauri/crates/infra/store/src/canonical.rs -> packages/desktop/src-tauri/crates/infra/store/src/canonical_test.rs

#### 待测功能

<!-- design.md 明确本文件无修改（「canonical 口径与引擎无关，原样复用」），
未声明任何公共 API 变更，不新增用例；既有行为由 `store_test.rs` 的
add/remove 等价路径用例间接覆盖。 -->

#### 用例

<!-- 无用例：design.md 声明无修改。 -->

#### Mock策略

<!-- 无。 -->

---

### packages/desktop/src-tauri/src/commands/exec/agent.rs -> packages/desktop/src-tauri/src/commands/exec/agent_test.rs

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（类型化涟漪：tee 循环删除
`serde_json::to_value` 分支，直接传 `&[AgentEvent]`）。签名与收敛语义不变，
既有 `run_agent_with` 假 runner 用例全部保留回归（AC-1 存量场景行为不变）；
链路级断言见集成关系「exec 事件 tee → store 类型化事件表 → 命令面重放」。 -->

#### 用例

<!-- 无新增用例：涟漪简化由存量用例回归与集成关系 R3 承载。 -->

#### Mock策略

<!-- 无新增（既有假 runner / PATH_LOCK 装置沿用）。 -->

---

### packages/desktop/src-tauri/src/commands/exec/mod.rs -> packages/desktop/src-tauri/src/commands/exec/mod_test.rs

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（`agent_run_events` 删除
Value → AgentEvent 反序列化步骤，签名与返回类型不变）。既有查询用例保留
回归，seeding 装置由 Value 构造改类型化构造；链路级断言见集成关系 R3。 -->

#### 用例

<!-- 无新增用例：既有用例经装置适配后回归，链路级新断言在集成关系 R3。 -->

#### Mock策略

<!-- 无新增（沿既有 mock_app + 真实 Store 装置）。 -->

---

### packages/desktop/src-tauri/src/commands/mod.rs -> packages/desktop/src-tauri/src/commands/mod_test.rs

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（增 `pub mod db;` 轨道接线）。
接线正确性由编译与 `commands/db/mod_test.rs` 用例承载，不设独立用例。 -->

#### 用例

<!-- 无用例：纯接线。 -->

#### Mock策略

<!-- 无。 -->

---

### packages/desktop/src-tauri/src/main.rs -> packages/desktop/src-tauri/src/main_test.rs

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（invoke_handler 注册两条 db 命令）。
注册可达性由集成关系「store 信封 API → db 命令轨道」在命令函数层面验证；
完整 invoke_handler 装配需真实 runtime，不设独立用例（见不可测试项）。 -->

#### 用例

<!-- 无用例：装配接线。 -->

#### Mock策略

<!-- 无。 -->

---

### packages/desktop/src/types/dto.ts -> packages/desktop/src/types/dto.test.ts

#### 待测功能

<!-- design.md 声明本文件新增 `ModelInfo` / `RecordEnvelope` 类型镜像（纯
类型，无运行时行为）。类型正确性由类型检查与消费方（useDbInspector /
DbInspectorView）用例的 fixture 编译承载，不设独立运行时用例。 -->

#### 用例

<!-- 无用例：类型镜像无运行时行为。 -->

#### Mock策略

<!-- 无。 -->

---

## 集成测试

<!-- 聚焦模块组合时才暴露的行为。Rust 侧集成用例与 `*_test.rs` 模块共置
（binary crate 无库目标，沿 `commands/workspaces/mod_test.rs` 既有模式）；
前端跨模块集成收 `src/__tests__/`（沿 agent_page_nav / agent_run_pipeline
既有模式）。 -->

### legacy 旧库格式 → Store::open 探测 → migrate 一次性迁移 → native_db 新库 → `packages/desktop/src-tauri/crates/infra/store/src/migrate_test.rs`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | 触发方（`Store::open` 打开流程：native 打开失败 → legacy 探测 → 调迁移 → 重开） |
| `packages/desktop/src-tauri/crates/infra/store/src/migrate.rs` | 迁移执行方（read_only 读旧、行转换、临时文件建新库、两段改名 + 回退） |
| `packages/desktop/src-tauri/crates/infra/store/src/model.rs` | 转换目标（`WorkspaceRecord` / `AgentRunRecord` / `AgentEventRecord`，native_model 封装与 `Raw` 兜底构造） |

**关联AC**: AC-1, AC-2, AC-3

**关系描述**: 这是本变更风险最高的跨模块交互：迁移模块以「读旧格式（redb 手写表）+ 写新格式（native_db 模型）」两种引擎口径同时工作，且迁移由 `Store::open` 隐式触发——用户无感，失败即丢运行历史的入口。值得测试的原因有三：一是旧表复合键 `(run_id, seq)` 与新模型 `event_key` 打包、value JSON 与 `AgentEvent` 嵌装之间的转换口径只在两模块组合时才可端到端验证；二是「解不出即 `Raw` 兜底」的零丢失构造性保证必须以真实损坏行验证，任何一行丢失都是不可重建数据损失（desktop-data-dimensions 裁定）；三是两段改名的原子性（`.bak` 留档、失败回退）依赖真实文件系统状态机。可能的出错模式：探测分支误判（native 库被误当 legacy）、迁移后未直通（每次 open 重复探测）、`seq` 误取自 value 而非旧表键、`user_meta` 被误迁移导致轮账概念复活。

#### 场景: 旧格式库首启自动迁移且各表记录一致

验证：以裸 redb 句柄构造旧格式 fixture 库（`user_workspaces` 2 行、`user_agent_runs` 2 行含 completed 终态与汇总字段、`user_agent_run_events` 两 run 各多条乱序 seq），对同一路径 `Store::open`；预期打开成功且 `list_workspaces` / `list_agent_runs` / `list_agent_run_events` 与 fixture 逐行一致（事件按 seq 升序、run 清单 `started_at` 降序不变）。随后 drop 重开同一路径：结果与首次一致且不再触发探测（native 格式即迁移完成标记，AC-2「记录数一致」+ AC-1「重开直通」）。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 三表 fixture 全量迁移后经 Store 公共 API 读回，workspace / run / 事件三读面与旧库内容逐行一致（root、id、字段面、事件 seq 序） | 新增 |
| 正向 | 迁移后重开同一路径直通（不再探测），重读结果与首次一致 | 新增 |
| 边界 | 空 legacy 库（三表存在但零行）迁移后为空新库，`list_models` 计数全 0 不报错 | 新增 |
| 边界 | legacy 库含 `user_meta` 表（schema_version）时迁移成功且该表内容不出现在新库任何可读面（轮账概念随迁移退役，AC-3） | 新增 |

#### 场景: 损坏事件行 Raw 兜底零丢失

验证：fixture 的 `user_agent_run_events` 混入三类损坏行——非法 JSON 字节串、合法 JSON 但缺 `kind` 字段、`kind` 为未知判别值；`seq` 只存在于旧表复合键中。预期：`Store::open` 不因损坏行失败；重放后行数与旧行数一致（零丢失）；损坏行落 `AgentEventKind::Raw`（`event_type` 取 JSON `"kind"` 字段或 `"unknown"`，`raw_json` 为原文字节串），`seq` 取自旧表复合键、`timestamp_ms` 取 JSON `timestampMs` 或 0；合法行不受损坏行影响。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 混入损坏行后迁移成功，重放行数与旧行数相等（无一行丢失，构造性保证成立） | 新增 |
| 边界 | 非法 JSON / 缺 `kind` / 未知 `kind` 三类损坏行各自落 `Raw`，`event_type` / `raw_json` / `seq` / `timestamp_ms` 按既定兜底口径取值 | 新增 |
| 边界 | 事件 value 含中文/emoji/深嵌套的合法行迁移后五变体逐字段保真（嵌装载荷端到端无损） | 新增 |

#### 场景: .bak 留档与失败回退原子性

验证：迁移成功路径上旧文件改名 `.bak` 存在且新库落原路径（AC-2「`.bak` 存在」）；失败路径上（以旧库被占用、目标改名受阻等注入点模拟）`Store::open` 返回 `Err`、错误串携带「迁移:」前缀，且旧文件保持原样或被回退——任何时点旧库不丢。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 迁移成功后原路径为 native 新库、`.bak` 文件存在且内容为完整旧库（可经只读 redb 句柄打开复核） | 新增 |
| 异常 | 迁移中途失败时 open 返回 `StoreError::Db` 且错误串带「迁移:」语境前缀 | 新增 |
| 异常 | 失败后旧文件保持原样（可再次以 legacy 口径读出全部记录）或改名被回退，`.bak` 不覆盖未成功写入的新库 | 新增 |
| 边界 | 同路径 `.bak` 已存在（上次迁移残留）时成功迁移的覆盖/先移除行为符合设计（先移除再改名） | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| 旧 redb 手写格式库 | 测试侧以裸 redb 句柄（`Database::create` / 只读复核用 `ReadOnlyDatabase`）直写三表 fixture，镜像旧表定义常量；属测试数据构造而非依赖 mock | 全部场景 |
| 文件系统改名/占用 | 不 mock——tempfile 真实路径上断言 `.bak` 存在性与回退；失败注入优先用「路径被普通文件占据」「句柄未释放」等真实可达形态 | `.bak` 留档与回退场景 |

---

### store 信封 API → db 命令轨道 → `packages/desktop/src-tauri/src/commands/db/mod_test.rs`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `packages/desktop/src-tauri/crates/infra/store/src/envelope.rs` | 信封实现方（注册表分发、scan 分页与 key/value JSON 化） |
| `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | 中间层（`Store::list_models` / `Store::scan` 公共方法，委托信封实现体） |
| `packages/desktop/src-tauri/src/commands/db/mod.rs` | IPC 薄包装（`db_models` / `db_records` 只读命令，`State<Store>` → 信封 API → DTO） |

**关联AC**: AC-5, AC-7

**关系描述**: 信封 API 是「依赖升级」与「查看页」两个诉求的咬合点：store 侧的注册表机制必须经命令轨道原样可达，查看器的「零模型特定代码」才成立。值得测试的原因：命令层是 native_db 类型不越公共面的最后一道闸——若薄包装意外加工或泄漏了非 JSON 形态，前端信封契约（`RecordEnvelope{key, value}` 均为 JSON 值）即被破坏；同时 `Err(String)` 约定决定前端 inline 错误能否呈现。可能的出错模式：命令面对未知模型静默返回空数组（吞错）、serde 形态与直连 store 不一致（薄包装偷偷加工）、offset/limit 语义在透传中被二次解释。

#### 场景: 命令面可达与薄包装保真

验证：mock_app manage 真实 Store（tempfile 真库），经 Store 写入三模型数据后直调 `db_models` / `db_records`；预期与直连 `store.list_models()` / `store.scan()` 的 serde 值一致，未知模型名 reject 携带 store 层错误串。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 写入数据后 `db_models` / `db_records` 与直连 store 公共 API 的 serde 值一致（薄包装不加工） | 新增 |
| 正向 | 经命令面连续翻页（offset 0 → limit → 2·limit → …）拼接结果覆盖全部记录不重不漏（AC-5 分页扫描 + 「分页扫描与单条查看」场景的命令面前提） | 新增 |
| 异常 | `db_records` 未知模型名返回 `Err(String)`，错误串与 store 层一致（不静默吞错） | 新增 |
| 边界 | 空库 `db_models` 返回全模型计数 0、`db_records` 返回空数组（空态链路命令面可达） | 新增 |
| 边界 | 返回信封的 key / value 序列化产物均为 JSON 值，无 bincode 二进制形态泄漏（「类型不越信封」的值层面证词） | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| Tauri runtime | `tauri::test::mock_app()` manage 真实 Store，`#[tauri::command]` 保留原函数直调，不启动真实 IPC | 全部用例（沿 workspaces / exec 轨道既有惯例） |

---

### exec 事件 tee → store 类型化事件表 → 命令面重放 → `packages/desktop/src-tauri/src/commands/exec/mod_test.rs`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `packages/desktop/src-tauri/src/commands/exec/agent.rs` | 写入方（tee 循环：类型化事件直接落库 + Channel 实时流） |
| `packages/desktop/src-tauri/src/commands/exec/mod.rs` | 查询命令面（`agent_run_events` 返回类型化事件序列） |
| `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | 存储方（类型化 `append_agent_run_events` / 二级索引 `list_agent_run_events`） |
| `packages/desktop/src-tauri/crates/infra/store/src/model.rs` | 嵌装载荷（`AgentEventRecord` 包装 core `agent::AgentEvent`） |

**关联AC**: AC-1, AC-4

**关系描述**: 事件 API 类型化是本变更的连带涟漪：写入方（tee）、存储方（嵌装模型 + u128 键）、查询命令面三段链路全部换轨，任何一段的 serde 口径漂移都只在组合时暴露。既有 `mod_test.rs` / `agent_test.rs` 的存量用例以 `serde_json::Value` 构造事件种子，装置须随类型化签名改造；改造后的组合断言（五变体逐字段重放保真）是 AC-4「嵌装建模」的端到端证词，也是 AC-1「落库/重放存量场景行为不变」的直接验收面。可能的出错模式：`seq` 来源从事件本体改为键打包后重放序与 tee 产出序不一致、`Raw` 变体在嵌装往返中丢失、Channel 推送与落库的先后关系被破坏。

#### 场景: 类型化链路五变体端到端保真

验证：以假 runner 注入五变体事件流（含 `Raw` 与中文/emoji 载荷）驱动 `run_agent_with`，随后经 `agent_run_events` 命令面重放；预期逐字段相等、seq 升序、run 间隔离，与存量用例（Value 种子路径）改造前行为一致。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 假 runner 五变体事件流经 tee 落库后命令面重放逐字段保真、seq 升序（AC-4 端到端证词） | 新增 |
| 正向 | 两 run 重叠 seq 区间互不串扰（二级索引隔离替代旧复合键半开区间，命令面复核） | 新增 |
| 边界 | 既有存量用例（run 清单排序、终态替换、落库失败收敛 failed、Channel 推送不中断、重开持久）经类型化装置适配后全绿 | 新增 |
| 边界 | 单 run 千级 seq 连续产出后重放序完整（u128 键打包在大 seq 下的保序性，命令面复核） | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| AgentRunner | 假 runner 经 `run_agent_with` 泛型缝注入受控事件流（沿 agent_test.rs 既有装置），不 mock store 与模型层 | 全部用例 |
| PATH 环境变量 | 既有 `PATH_LOCK` 互斥 + 空 PATH 触发启动失败分支（存量用例沿用） | 启动失败 Err 用例（保留回归） |

---

### 侧栏系统工具组 → App 顶层切换 → DbInspectorView 挂载 → `packages/desktop/src/__tests__/db_inspector_nav.test.tsx`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `packages/desktop/src/components/AppSidebar.tsx` | 触发方（「系统工具」组渲染与 `onPageChange('db')` 派发） |
| `packages/desktop/src/App.tsx` | 切换中间件（顶层 `page` state 三分支渲染，无路由） |
| `packages/desktop/src/views/db/DbInspectorView.tsx` | 挂载目标（db 页主内容区） |
| `packages/desktop/src/hooks/useDbInspector.ts` | 挂载后取数发起方（进页取模型清单） |

**关联AC**: AC-6

**关系描述**: 侧栏重组把导航入口拆成两组，「组归属 + 切换语义 + 欢迎态不可达」三件事横跨 Sidebar 与 App 两层：单组件测试断言不了「欢迎态无该组 DOM」（App 层不挂壳）与「切页不触发 change 取数」（useChangeList 留 App 层的共存语义）。值得测试的原因：db 是第三个顶层分支，若三分支渲染意外共享挂载副作用（如重复取数、选中状态重置），只有整树组合才暴露。可能的出错模式：nav-db 点击后 `page` 未切或切了但视图未换、欢迎态壳被误挂载露出系统工具组、db 页与 changes 页取数互相触发。

#### 场景: 壳态入口与切换共存

验证：壳态（root 已选）渲染 App，点击 nav-db；预期主内容区切至 DbInspectorView、workspace 清单组仍在、`list_changes` 调用次数不增长（本地 state 切换、无路由、无取数副作用）。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 壳态点击 nav-db → DbInspectorView 渲染且 `db_models` 取数发起（进页即用户显式动作） | 新增 |
| 正向 | changes → db → agent → changes 往返切换，各域视图与 workspace 清单共存不串扰 | 新增 |
| 边界 | 切至 db 页再切回 changes，`list_changes` / `list_workspaces` 调用次数不增长（切页不触发重取） | 新增 |
| 异常 | db 页挂载时 `db_models` reject，错误仅 inline 于查看页，不影响侧栏与切页（页面级错误不冒泡为全局崩溃） | 新增 |

#### 场景: 欢迎态不可达

验证：`root=null`（欢迎态）渲染 App；预期无壳与侧栏 DOM、「系统工具」组不存在、无任何可达入口进入 DB 查看页（欢迎屏维持完全独立页面）。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 边界 | 欢迎态 DOM 中无「系统工具」组标签与 nav-db（AC-6「欢迎态无该组 DOM」） | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| IPC 边界 | `vi.mock('@tauri-apps/api/core')` invoke 按命令名分发 fixture 并记录调用序列（`db_models` / `db_records` / `list_changes` / `list_workspaces`），沿 agent_page_nav.test.tsx 装置；matchMedia / 视口 stub 同源 | 全部场景 |

---

### DbInspectorView → useDbInspector → db_models/db_records IPC → `packages/desktop/src/__tests__/db_inspector_pipeline.test.tsx`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `packages/desktop/src/views/db/DbInspectorView.tsx` | 呈现方（清单 / 分页 / 单条 JSON / 空态 / inline 错误态渲染） |
| `packages/desktop/src/hooks/useDbInspector.ts` | 取数收口（挂载取一次、动作触发分页、error 态 inline 持久） |
| `packages/desktop/src-tauri/src/commands/db/mod.rs` | IPC 命令提供方（`db_models` / `db_records`，测试中于 IPC 边界以 invoke 替身呈现） |

**关联AC**: AC-5, AC-7

**关系描述**: 查看页的用户可见行为是「hook 状态机 + 视图渲染 + IPC 契约」三者的组合：单测里 hook 被 mock（视图测渲染）或视图被卸载（hook 测取数），只有真 hook + 真 view + mock IPC 的组合才能验证「用户动作 → invoke 参数 → 信封渲染」整链——尤其是翻页 offset 步进与 `hasMore` 判定的拼接不重不漏、错误 inline 持久不顶替。可能的出错模式：翻页参数与展示页错位一页、reject 后错误区被下一次渲染吞掉、隐式 effect 轮询导致取数失控（既有 useWorkspaces 稳定判据模式可直接复用）。

#### 场景: 清单到翻页取数链不重不漏

验证：真 hook + 真 view 挂载，invoke 替身按命令名返回多页信封 fixture；预期挂载取 `db_models` 一次，选中模型取第一页，连续翻页 invoke 参数 offset 严格步进，页面渲染的记录并集等于 fixture 全集（不重不漏）。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 进页取清单 → 选中模型取第一页 → 两次翻页 → 渲染记录并集与 fixture 全集一致、无重复 key | 新增 |
| 正向 | 点开单条记录，呈现的 JSON 文本与该信封 key/value 一致（人可读、可复制语义） | 新增 |
| 边界 | 静置后 invoke 调用数不增长（无轮询、无事件订阅，AC-7 只读取数纪律） | 新增 |
| 异常 | `db_records` reject → inline 持久错误区呈现且不消失（后续渲染不顶替）、无 toast 调用（「查询失败 inline 持久」场景） | 新增 |
| 边界 | 末页 `hasMore=false` 时翻页控件禁用、无多余 invoke | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| IPC 边界（`@tauri-apps/api/core`） | `vi.mock` invoke 按命令名分发多页信封 fixture 并记录调用序列；reject 用例以 `mockRejectedValue` 注入 | 全部用例（组件树与 hook 均真实，仅 mock IPC 进程边界） |

---

## 不可测试项

<!-- proposal 范围内无法以自动化单元/集成测试验证的条目 -->

- AC-8 依赖规则修订落档（store `Cargo.toml` 仅新增 `agent`、workspace pin `native_db` / `native_model`、`redb` 保留注释、desktop-crate-layout spec 修订） — **原因**: manifest 与 spec 文档是声明性产物，无运行时行为可断言；`test_resolve_paths` 亦判定三个 manifest（`packages/desktop/package.json`、`packages/desktop/src-tauri/Cargo.toml`、`crates/infra/store/Cargo.toml`）为「Not a testable source file」。其自动验证面即依赖解析与 workspace 编译，落档正确性属静态审查项。
- AC-4 的「`crates/core/agent` git diff 为空（零 derive 改动）」 — **原因**: 「某目录无改动」是版本库状态断言，属提交前静态 diff 审查，运行时测试只能证明嵌装在 core 类型零改动前提下成立（由 model_test 嵌装往返与集成 R3 覆盖）。
- AC-3 的「store 源码无 META 轮账代码」与 AC-7 的「db 轨道命令无任何写操作入口」「查看器无模型特定分支」 — **原因**: 源码形态级断言（不存在某代码路径、不存在 per-model 分支）属静态审查/代码评审项；自动化只覆盖其行为投影（`user_meta` 不迁移、命令面仅两只读命令可调用、当前三模型经注册表可浏览）。「新模型注册即自动可浏览」的机制性主张同理：无法在不注册真实新模型的前提下自动化验证，机制由注册表结构与 R2 行为用例间接支撑。
- `packages/desktop/src/main.rs` 的 invoke_handler 完整装配 — **原因**: 命令注册矩阵需真实 Tauri runtime 启动才能端到端断言；MockRuntime 直调模式只能验证命令函数本身（集成 R2 覆盖），装配接线属编译期与人工冒烟项。
- 迁移窗口双开竞态（另一进程迁移期间写旧库） — **原因**: 需要真实双进程互斥场景与文件锁时序注入，自动化测试不作承诺；设计以「迁移仅启动时执行一次 + 单进程约束文档强化」缓解，双开风险由既有单进程治理（后手）承载。
- `packages/desktop/package.json` 版本 bump（0.2.1 → 0.3.0） — **原因**: 发布元数据，非可测行为；由发布流程核查。

