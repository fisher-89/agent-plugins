# 探索笔记：Desktop 后端架构分层（DDD 落位、数据维度、边界分类）

- 日期：2026-09-21
- 状态：长期架构方向探索；与 `desktop-redb-store.md`（本次 MVP 变更）配套——
  本文管长期形状，那边管本次落地
- 由来：redb-store 探索延伸出的分层之问："按领域驱动架构，store 也应属外层，
  业务核心在哪里？" 加上四条已明确的规划特性（见下）。

## 〇、输入：四条规划特性

1. 系统持久化数据（文件/db）分为 **user / workspace 两个维度**
2. workspace 未来会有**写文件**，甚至**读写数据库**操作
3. 领域边界主要有四类：**后端 api、db、文件、shell**
4. 未来会引入一款**图数据库**

## 一、领域核心在哪（继承自上轮讨论）

核心洞察：**主数据库是文件系统（openspec 树本身），redb 只是 side-car**。
领域核心 = change 域的读模型与解释规则：

- `foundation::resolve`：布局策略（领域的遍历语言）
- `workflow`：model（概念定义）、parse 吃字符串的部分（解码器）、
  queries 组装（读模型投影）、artifacts（产物语义解释）

workspace 清单是"最近打开的文件"式 shell 记忆，不是领域聚合。
但特性 #2 之后，"workspace"裂成两个概念，见下节。

## 二、数据两维度：user / workspace

| 维度 | 内容 | 生命周期 | 代表 |
|---|---|---|---|
| **user** | 跨 workspace 的 app 状态 | 跟随用户（app data dir） | workspace 注册表；未来设置、窗口状态 |
| **workspace** | 单 workspace 域内数据（含派生缓存/索引/图数据） | 跟随 workspace | 未来 change 缓存、关系图谱 |

workspace 维度落盘两个候选（留给 dev-design 的轴）：

- **a) home_dir/.dev-team 下按 workspace hash 分目录**（VS Code workspaceStorage 模式）：
  不污染 repo、天然私有；代价是 repo 移动后数据不跟随、hash 寻址依赖稳定
  canonical key
- **b) workspace 内落盘（.openspec/ 下）**：数据跟随 repo、可选择 gitignore
  或提交；代价是写用户 repo 的污染感与多工具并发风险

**约束**：维度必须从第一天在 store 层显式化（表命名/文件布局带维度语义），
哪怕 MVP 只有 user 维度一张表。

同时"workspace"概念一分为二：

- **workspace 注册表**（user 维度）：本次 MVP，shell 状态
- **workspace 内操作**（未来写文件/读写 db）：有行为的执行域，
  即 `commands/exec` 空轨道预留的语义归宿

## 三、边界分类学：四类 port

```
┌─ shell 边界：Tauri IPC、dialog、通知、单实例 ────────────┐
│ ┌─ db 边界 ─────┐ ┌─ 文件边界 ──────┐ ┌─ api 边界 ────┐ │
│ │ redb (kv)     │ │ workspace 读→写 │ │ 后端服务       │ │
│ │ 图 db (未来)  │ │ app 自有文件    │ │ (未来, 用途未定)│ │
│ └───────────────┘ └─────────────────┘ └───────────────┘ │
│        领域核心（解释权）+ 应用编排（现为 command）         │
└──────────────────────────────────────────────────────────┘
```

文件边界将从"领域原生存储（只读）"升级为**可写边界**：
workspace 写文件 = 对用户 repo 的变更，需 write protection /
路径包含性检查 / 审计——`exec` 轨道 trait 形状等第一条真实执行命令
落地时再定形（既有决策不变）。

## 四、目录树落位

```
crates/
  core/            领域 + 应用纯域：foundation, workflow
  infra/
    store/         ★ redb（db 边界第一成员，本次落地）
    graph/         （未来）图数据库
    api/           （未来）后端 api
    fs/            （文件边界长出独立适配时再立）
  (desktop-app)    shell adapter；command 即应用服务
```

依赖方向规则（由 crate 图机械保证，不靠注释纪律）：

```
desktop-app ──▶ infra/store, core/*   （编排）
core/*       ──▶ 纯依赖（serde 等），不依赖 infra，不依赖彼此的上层
infra/store  ──▶ redb + serde 系，不依赖 core（自含模型）
```

- `workflow` **依赖不到 store**：有状态持久化进不了纯读域
- 将来若出现"infra 实现 core 定义的接口"（真正引入 port），依赖方向
  反转为 core 定义 → infra 实现，那是抽接口的时机信号

**对"一个孩子不设学校"的修正**：上轮结论建立在"单成员"假设上；
四类边界 + 图数据库已是明确规划而非投机，目录表达计划零成本，
故 `infra/` 即刻立。但**代码层面仍不预抽象**：不建 port trait、
不建泛型仓储接口——crate 边界 + store 内部纯/杂分模块（model.rs 纯 /
redb.rs 杂）就是将来抽接口的现成缝。

规则演进：根 Cargo.toml 注释"core 两个 crate 禁用任何 Tauri 依赖"
维持不变（core 仍是两个）；infra 同样禁 Tauri（Tauri 只属于 desktop-app）。

## 五、未来租户归位表

| 特性 | 边界 | 落位 | 备注 |
|---|---|---|---|
| workspace 写文件 | 文件 | exec 轨道（模块已预留） | 第一条真实执行命令落地时定 trait；写 repo 需保护与审计 |
| workspace 读写 db | db | infra/store 的 workspace 维度 | 落盘候选 a/b 见第二节，dev-design 定 |
| 图数据库 | db | infra/graph | 选型标准：embedded 优先（与 redb 同哲学）、Rust 亲和、Windows 支持；大概率装 change 关系图谱（change↔spec↔task↔phase、archive 谱系）；数据维度以 workspace 为主、跨 workspace 知识网为辅（开放） |
| 后端 api | api | infra/api | 用途未定（同步/遥测/账号？），先占位 |

## 六、对本次 store 层（MVP）的直接约束

1. **redb 类型不泄漏**：store 公共 API 只暴露自有类型（`WorkspaceRecord`
   等），`redb::Database` / `Table` 不出现在命令签名与前端契约——
   图数据库加入时是 db 边界新成员，而非 store 层重构
2. **数据维度显式化**：表命名从第一天带维度语义（如 user 前缀），
   为 workspace 维度留格
3. **读写分离**：未来 workspace 可写，但写轨道独立成 exec 域，
   不往 workflow（读模型）里塞写代码；workflow "纯读库"定位保持到
   exec 轨道落地那天

## 七、开放问题

1. workspace 维度落盘选 a) home_dir hash 目录 还是 b) repo 内？
2. 图数据库产品选型与引入时机（等关系查询的真实痛点出现再进？）
3. api 边界的用途与形态
4. 应用编排长大后是否需要独立 app 层 crate（现在 command 即应用服务够用）
