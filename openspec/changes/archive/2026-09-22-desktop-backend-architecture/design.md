# 设计: desktop-backend-architecture

> **变更**: desktop-backend-architecture
> **日期**: 2026-09-22（attempt 2 修订）

---

## 变更形态与规格同步状态

本变更为**规格收敛变更**：主体是三份 spec delta（治理未来变更的规格载体），代码侧唯一落点是 `commands/mod.rs` 模块文档，零行为变更。

提案阶段产物已同步完成（proposal 评审 2026-09-22 通过，11/11，见 workflow.json）：

| 产物 | 状态 | 说明 |
|------|------|------|
| `proposal.md` | 已定稿 | 三支柱裁决、5 条 AC、7 项决策均含被拒备选 |
| `specs/desktop-data-dimensions/spec.md` | 已写就（ADDED 3 requirements） | 数据两维度模型；workspace 维度落盘 workspace 内 + foundation 收口；三笔账落地约束 |
| `specs/desktop-crate-layout/spec.md` | 已写就（ADDED 2 requirements） | 四类边界分类学；未来租户归位规则（graph 暂不考虑 / api 不建目录 / 写文件归 exec 轨道） |
| `specs/desktop-app-shell/spec.md` | 已写就（ADDED 2 requirements） | command body 三件事纪律与 `*_inner` 微形态；不建 app crate 裁决与五条翻转信号 |

按约定，上述产物不再进入下方变更清单与任务列表（无任务、无修改文件条目）；archive 由用户手动执行，亦不设任务。

**回溯承接**：attempt 1 实现阶段已按上一版设计在 `commands/mod.rs` 写入 9 行纪律段，但其中 spec 指针含字面量 `openspec`，被既有 layout_test 命名隔离禁令判定失败（test-execution 回溯至 dev-design attempt 2）。本版修正指针定式并在设计层记录该约束；实现阶段的增量收敛为对该注释的一次短语级替换。

---

## 修订记录（上一版 → 本版）

| 位置 | 上一版 | 本版 | 原因 |
|------|--------|------|------|
| 设计决策 D2 | 注释 spec 引用写归档全路径 `openspec/specs/desktop-app-shell/spec.md` | 改为相对域根定式 `` `specs/desktop-app-shell/spec.md` `` + 「路径相对域根」限定语 | 与 layout_test 包源码字面量禁令冲突（回溯根因），禁令详见「命名隔离约束」 |
| 注释内容要素第 4 条 | 指向 `openspec/specs/desktop-app-shell/spec.md` | 指向相对域根定式，注释全段不含禁用字面量 | 同 D2 |
| 设计决策 D5 | （无） | 新增：命名隔离约束成文（禁令来源 / 扫描范围 / 唯一豁免 / 推论） | 回溯要求在设计层记录该约束 |
| 变更清单「修改文件」行 | 修改内容写归档全路径 | 修改内容改写定式并显式声明全段无禁用字面量 | 与 D2 同步 |
| tasks | A1/A2 追加纪律段（已执行） | A1 改为指针短语替换；新增 B3 命名隔离收口自检 | attempt 1 已写入纪律段，本轮为最小修正 |

保持项（与上一版一致，不回退）：D1（注释唯一落点）、D3（措辞与 delta 对齐）、D4（提案产物不入清单）；架构组件表、数据模型、路由/API、依赖、AC-1/AC-2/AC-3/AC-5 落点均不变。

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| desktop-data-dimensions 能力 spec（新） | 数据两维度模型的治理载体：user / workspace 维度定义与生命周期、workspace 内落盘裁决、三笔账 MUST/SHALL 落地约束 | `openspec/changes/desktop-backend-architecture/specs/desktop-data-dimensions/spec.md`（archive 后 → `openspec/specs/desktop-data-dimensions/spec.md`） | proposal 决策表、explore.md 第八节裁决；被未来 workspace 维度 change 的 proposal/dev-design 强制引用 | OpenSpec spec delta（RFC 2119 MUST/SHALL + Scenario） |
| desktop-crate-layout 能力 spec（增补） | 四类边界分类学（shell / db / 文件 / api）与未来租户归位表；infra 禁 Tauri 由现有成员泛化到未来成员 | `openspec/changes/desktop-backend-architecture/specs/desktop-crate-layout/spec.md`（archive 后原位合并 `openspec/specs/desktop-crate-layout/spec.md`） | 既有 desktop-crate-layout spec（不改既有 requirement）；根 `packages/desktop/src-tauri/Cargo.toml` 依赖图（既有「core 与 infra 均禁 Tauri」注释，L36） | OpenSpec spec delta |
| desktop-app-shell 能力 spec（增补） | command body 三件事纪律、`*_inner` app 层微形态确认、不建 app crate 裁决与五条翻转信号 | `openspec/changes/desktop-backend-architecture/specs/desktop-app-shell/spec.md`（archive 后原位合并 `openspec/specs/desktop-app-shell/spec.md`） | 既有 desktop-app-shell spec；`commands/workspaces` 的 `*_inner(&Store)` 既有模式（被点名为微形态，不改代码） | OpenSpec spec delta |
| `commands/mod.rs` 模块文档（唯一代码落点） | 把三件事纪律声明写进「写新命令的人会看的位置」，并以**不含禁用字面量的相对域根指针**指向 spec 决策记录 | `packages/desktop/src-tauri/src/commands/mod.rs` | desktop-app-shell spec delta（措辞对齐，见设计决策 D3）；命名隔离约束（见设计决策 D5，限定指针形态） | Rust `//!` 模块级 doc comment |
| 受治理对象（零改动基线） | 约束的适用对象，本变更 MUST NOT 触碰：desktop-app 壳（7 命令 + setup）、`crates/infra/store`（公共 API / 表结构 / 单进程约束注释不变）、core 两 crate（零 Tauri）、`commands/exec/` 空轨道 | `packages/desktop/src-tauri/src/`、`packages/desktop/src-tauri/crates/` | cargo workspace（根 `packages/desktop/src-tauri/Cargo.toml`，成员恰为 foundation / workflow / store） | Rust 2021 / redb 4.3 / Tauri 2 |

---

## 设计决策

| # | 决策 | 理由 |
|---|------|------|
| D1 | 纪律注释只写 `commands/mod.rs` 模块文档；不动 crate 文档、不加每命令注释、不加任何属性标注 | AC-4 限定「无其他代码改动」；模块文档是该文件内新增命令的必经阅读位，一个落点即可约束三轨全体命令 |
| D2 | 注释中 spec 引用采用**相对域根定式**：能力名（desktop-app-shell 能力 spec）+ 路径 `` `specs/desktop-app-shell/spec.md` `` + 限定语「路径相对域根」；MUST NOT 写含字面量 `openspec` 的全路径 | 上一版全路径被 layout_test 命名隔离禁令拦截（回溯根因）；相对域根形式同时满足两点——不含禁用字面量、域根未来更名时指针不失效（与 proposal 风险表「`.openspec/` 目录未来更名」的既有规划一致，rename-proof）。可溯源性不损失：域根即 foundation `layout::resolve` 的 domain_root（唯一字面量触点 `crates/core/foundation/src/layout.rs`），全路径可拼接复原为 `openspec/specs/desktop-app-shell/spec.md`；该归档基线已存在（desktop-app-shell 为「修改的能力」），archive 原位合并、路径长期稳定；变更内 delta 路径 archive 后失效，仍不指向 |
| D3 | 注释内容四要素固定（见下节），与本变更 desktop-app-shell delta 的 ADDED requirement 措辞对齐，不引入 spec 未载的新纪律 | 注释与 spec 是同一裁决的两个载体，措辞漂移会形成两套口径；delta 本身未钉任何路径，指针形态由 D2 单方约束，无对齐冲突 |
| D4 | 三份 spec delta 与 proposal.md 不复述为任务或修改文件——已随提案阶段完成 | 规格同步属提案产物；实现阶段的增量只有一处模块注释修正，列入清单/任务会制造伪待办 |
| D5 | 命名隔离约束成文：产品 Rust 源码 MUST NOT 含字面量 `openspec`（磁盘域根目录名），唯一豁免 `crates/core/foundation/src/layout.rs`；注释指针因此 MUST 采用 D2 相对域根定式 | 禁令来源为既有机械测试（见「命名隔离约束」），attempt 1 实测触发 panic；把约束写进 design 使指针形态有据可查，未来任何写注释 / 文档字符串的人不再踩同一坑 |

### 命名隔离约束（D5 详述）

| 项 | 内容 |
|----|------|
| 禁令载体 | 既有测试 `crates/core/foundation/src/layout_test.rs` 用例「全包源码不含_openspec_字样_layout_为唯一例外」（L168 `let forbidden = "openspec"`，子串包含判定，区分大小写） |
| 扫描范围 | 三个源码树下全部 `.rs` 产品源码（文件名以 `_test.rs` 结尾者排除）：① `packages/desktop/src`（前端树，现无 `.rs`）；② `src-tauri/crates/*/src`（core / infra 全体 crate）；③ `src-tauri/src`（desktop-app 根包，含 `commands/mod.rs`） |
| 唯一豁免 | `crates/core/foundation/src/layout.rs`（磁盘目录名的唯一合法触点，`resolve()` 在此拼出域根） |
| 设计动机 | 域根目录名在产品源码中零触点：运行时路径一律经 foundation layout 解析器收口（既有约束），源码引用（注释 / 文档字符串）同理不硬编码——域根未来更名时全仓库无一处源码需改 |
| 对本变更的推论 | `commands/mod.rs` 注释全段（含中文行文与代码 span）MUST NOT 出现 `openspec` 子串；spec 指针用 D2 定式。attempt 1 的 `openspec/specs/desktop-app-shell/spec.md` 即因此被拦 |
| 保障方式 | 既有 layout_test 用例机械保障（回归由测试阶段承接）；实现与收口阶段以只读 grep 复核（tasks B3），不在本变更新增任何测试 |

### 注释内容要素（AC-4 落点，目标态）

`commands/mod.rs` 模块文档 = 既有两行三轨描述 + 纪律段（attempt 1 已写入，共 9 行），四要素：

1. **三件事纪律**：command body 仅允许——参数转换（IPC 入参 → 领域/store 入参）、调用（core 函数或 store 操作）、错误映射（领域/Store 错误 → `Err(String)`）；
2. **两条禁令**：领域解释 SHALL 下推 core；跨边界协调 SHALL 触发 app crate 决策（五条翻转信号，见 spec），MUST NOT 以「先塞进命令里」的方式消化编排增长；
3. **微形态声明**：`commands/workspaces` 的 `*_inner(&Store)` 纯函数模式为 app 层微形态，将来抽 app crate 时平移复用（函数边界升 crate 边界）、不重写、不内联回命令体；
4. **决策出处**（本版唯一修正点）：指向 desktop-app-shell 能力 spec，指针定式钉死为——

   > `` `specs/desktop-app-shell/spec.md` ``，路径相对域根

   即注释行由 attempt 1 的

   > `` //! `openspec/specs/desktop-app-shell/spec.md`）：每条命令的 body 仅允许三件事—— ``

   替换为

   > `` //! `specs/desktop-app-shell/spec.md`，路径相对域根）：每条命令的 body 仅允许三件事—— ``

   域根目录名不出现在注释；读者经 foundation `layout` 模块或仓库 `openspec/` 目录均可定位归档 spec。

体量约束：纪律段维持 9 行（6~10 行约束内）；文件其余内容（`pub mod exec; pub mod queries; pub mod workspaces;` 三行与既有两行注释）零改动。

---

## 变更清单

<!--
  覆盖范围：proposal「变更范围 - 实现文件」中唯一代码落点 commands/mod.rs。
  三份 spec delta 与 proposal.md 为提案阶段已同步产物（见「变更形态与规格同步状态」），按约定不列入本清单、不设任务。
-->

### 新增文件

无——规格收敛变更不新增任何实现文件、目录或 crate。

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src-tauri/src/commands/mod.rs` | 模块文档（`//!`）纪律段保持四要素不变（三件事枚举 + 两条禁令 + `*_inner` 微形态保留声明 + 决策出处），仅将决策出处指针由全路径 `openspec/specs/desktop-app-shell/spec.md` 替换为相对域根定式 `` `specs/desktop-app-shell/spec.md` `` +「路径相对域根」限定语；替换后注释全段不含字面量 `openspec`（命名隔离禁令，见 D5） | AC-4；attempt 1 已按上一版设计写入纪律段，本版收敛为该短语级替换（D2）；除该注释外零改动（设计决策 D1） |

### 删除文件

无。

### 公共函数 / API

<!-- 无公共函数/API 变更：仅模块文档注释，无签名、导出、命令注册变更 -->

无。

### 类型定义

<!-- 无类型变更 -->

无。

### 配置

<!-- 无配置变更：不新增依赖、不改 Cargo.toml members / workspace.dependencies、不改 tauri.conf.json -->

无。

---

## 数据模型

本变更不新增任何持久化模型、字段或表；`crates/infra/store` 表结构（`user_workspaces` / `user_meta`）与公共 API 不变。spec 化的数据维度模型（概念层，作为未来一切落盘设计的强制分类轴）：

| 模型 | 内容 | 关系 | 持久化 |
|------|------|------|--------|
| user 维度数据 | 跨 workspace 的 app 状态。已落地代表：workspace 注册表（`user_workspaces` / `user_meta`）；未来租户：设置、窗口状态 | 生命周期跟随用户；是 workspace 维度数据的注册入口（声明 workspace root） | app data dir（redb store）；MUST NOT 因维度模型迁移或混入 workspace 维度存储 |
| workspace 维度数据 | 单 workspace 域内数据，均为未来租户：change 缓存、change 索引、关系图谱 | 生命周期跟随 workspace；SHALL 作为派生数据可重建（缓存重算、索引重扫），克隆到新机器缺失不致错误 | workspace 内 `.openspec/` 树（具体目录层留未来 dev-design）；路径 SHALL 经 foundation layout 解析器收口，消费侧无硬编码路径；gitignore 按数据类型分型，MUST NOT 一刀切 |
| 三笔账（落地约束，非存储模型） | ① redb 独占锁错误语义 MUST 显式（重试 / 只读降级，届时定）；② gitignore 类型-目录-策略对照落地时给出；③ 数据 SHALL 可克隆重建 | 未来 workspace 维度持久化变更的 proposal / dev-design MUST 逐项引用交代 | 记录于 desktop-data-dimensions spec，本变更零代码兑现 |

---

## 路由/API 设计

本变更不涉及 HTTP API。Tauri IPC 命令面零变更：7 条命令（queries 3 + workspaces 4）签名与行为均不动，`commands/exec/` 仍为空轨道（无实现、无空壳 trait）。

---

## 依赖

### 运行时依赖

- 无新增。既有依赖图维持现状：workspace 成员恰为 `crates/core/foundation`、`crates/core/workflow`、`crates/infra/store`（desktop-app 根包兼作 workspace 根，tauri-cli 所迫）；根 `Cargo.toml` L36「core 与 infra 均禁 Tauri（Tauri 只属于 desktop-app 壳）」注释维持原样，其向未来成员（graph / api）的泛化由 desktop-crate-layout spec 承载，不改注释措辞（零信息增量）。

### 构建/测试依赖

- 无新增。既有 `cargo`（workspace 根 `packages/desktop/src-tauri`）与 `tauri = { workspace = true, features = ["test"] }` dev-dependency 不因本变更变化；回归证据沿用既有 `cargo test --workspace`（含 layout_test 命名隔离用例，本版修正后应转绿）与前端测试套件（全绿即「未破坏现状」），由测试阶段承接，本变更无新测试面。

---

## 验收标准对齐

| AC | 设计落点 |
|----|----------|
| AC-1 | desktop-data-dimensions spec delta 已在提案阶段写就（ADDED 3 requirements：两维度模型 / workspace 内落盘 + foundation 收口 / 三笔账），见「变更形态与规格同步状态」与「数据模型」；随 archive 落入 `openspec/specs/` |
| AC-2 | desktop-crate-layout spec delta 已写就（ADDED 2 requirements：四类边界分类学 / 未来租户归位表，含 graph 暂不考虑、api 不建目录、写文件归 exec 轨道、infra 未来成员禁 Tauri）；磁盘基线已满足——`crates/infra/` 仅 store 一个成员、无空目录（已核实），实现阶段由收口自检复核 |
| AC-3 | desktop-app-shell spec delta 已写就（ADDED 2 requirements：command body 三件事纪律与 `*_inner` 微形态 / 五条翻转信号可机械判定）；注释措辞按 D3 与其对齐 |
| AC-4 | 「修改文件」表唯一落点 `packages/desktop/src-tauri/src/commands/mod.rs`；注释内容四要素在「注释内容要素」固化，指针定式按 D2 不含禁用字面量（D5 约束成文）；D1 约束无其他代码改动 |
| AC-5 | 「依赖」两节均无新增；「受治理对象（零改动基线）」与 tasks Phase B 收口自检覆盖：不新建 crate / 目录 / 依赖、`commands/exec/` 仍为空轨道、store 与 core 零触碰；B3 复核命名隔离；回归证据由既有套件给出（测试阶段承接） |

---

## 待决问题

本变更设计层无未决问题（注释内容要素、指针定式与命名隔离约束均已定）。以下为 spec 显式留给未来变更的决策轴，不阻塞本变更：

- workspace 维度落盘具体目录层（`.openspec/` 下哪一层）——未来 workspace 维度 change 的 dev-design 定
- redb 独占锁冲突的错误语义（重试 / 只读降级）——同上，届时定
- gitignore 按数据类型分的目录表——同上；若出现值得随 repo 分享的状态再分目录
- 图数据库产品选型与引入时机——关系查询真实痛点出现时重启（届时按 db 边界新成员落 `infra/graph`）
- api 边界的用途与形态——真实需求出现时重启
