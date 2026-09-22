# 提案: desktop-backend-architecture

> **变更**: desktop-backend-architecture
> **日期**: 2026-09-22
> **状态**: draft（待评审）

---

## 问题

redb store MVP（`crates/infra/store`，workspace 注册表）已落地，db 边界有了第一成员。但它同时把一组长期架构问题摆上了台面，此前只散落在探索笔记里，未进入任何可执行的约束载体：

1. **数据维度未成文**：系统持久化数据客观上分 user（跟随用户）/ workspace（跟随仓库）两个维度，user 维度已由表命名（`user_workspaces` / `user_meta`）兑现，但 workspace 维度数据（未来的 change 缓存、索引、关系图谱）落在哪里、遵循什么规则，没有任何定论。
2. **边界分类学未成文**：领域边界客观上有四类（shell / db / 文件 / api），四条已规划特性（workspace 写文件、workspace 读写 db、图数据库、后端 api）各自归位到哪、哪些"现在就做"、哪些"明确不做"，没有一致口径。不收敛的后果是每个未来变更重新辩论一次落位，或走向投机性预抽象（预建空目录、空壳 trait）。
3. **app 层去留未裁决**：command 越长越多（现 7 条），"要不要抽独立 app 层 crate"悬而未决；不立纪律与翻转信号，command 会在无人触发决策的情况下悄悄长大。
4. **收敛结论只在笔记里**：2026-09-22 第二轮探索已将上述问题全部裁决（见 `explore.md` 第八节），但 explore 笔记是一次性的——不落 spec 就不可考、不可执行、约束不到未来变更。

本变更是一次**规格收敛变更**：把裁决写进 spec 体系，治理未来变更；代码零行为变更。

---

## 提案

三条 codification 支柱，全部落 spec，代码侧仅一处模块注释：

1. **数据维度模型 → 新能力 `desktop-data-dimensions`**：user / workspace 两维度定义（内容、生命周期、代表）；workspace 维度落盘裁决为 **workspace 内落盘**（`.openspec/` 下，具体目录层留未来变更的 dev-design），否决 home_dir hash 目录方案；落盘路径 MUST 经 foundation layout 解析器收口；未来 workspace 维度持久化落地时 SHALL 偿还三笔账——redb 独占锁错误语义、gitignore 按数据类型分型、克隆可重建语义。
2. **边界分类学与租户归位 → `desktop-crate-layout` 增补**：四类边界（shell / db / 文件 / api）分类学；未来租户归位规则——workspace 写文件归 exec 轨道（文件边界，trait 等第一条真实执行命令定形）、workspace 读写 db 归 store 的 workspace 维度、图数据库**暂不考虑**（痛点驱动再启，届时落 `infra/graph`）、后端 api **先占位留痕**（不建 `infra/api` 目录）；infra 全体现有与未来成员禁 Tauri。本变更**不建任何新目录、新 crate**。
3. **command 纪律与翻转信号 → `desktop-app-shell` 增补**：不建独立 app 层 crate（7 条命令全是 3-5 行薄包装，app 层现为空收益）；command body SHALL 只做三件事——参数转换、调用、错误映射；`*_inner(&Store)` 纯函数模式确认为 app 层微形态（将来抽 crate 是函数边界升 crate 边界的零重写平移）；五条 app crate 翻转信号成文（Rust 侧 phase lifecycle 命令、exec 第一条真实命令、跨 store + fs 协调、CLI/headless 复用、机械判据兜底），任一出现即重新决策。

**代码落点（最小）**：`commands/mod.rs` 模块文档补充 command body 三件事纪律声明——沿 store 单进程约束"以代码注释显式声明"的既有先例，让纪律出现在写新命令的人会看的位置。

---

## 能力

### 新增能力

- **desktop-data-dimensions** — 数据两维度模型（user / workspace）与 workspace 维度落盘策略：维度定义与生命周期、workspace 内落盘决策与 gitignore 分型、redb 独占锁错误语义与克隆重建约束、落盘路径经 foundation 收口。

### 修改的能力

- **desktop-crate-layout** — 增补（不改动既有 requirement）：四类边界分类学（shell / db / 文件 / api）；未来租户归位规则（graph 暂不考虑、api 不建目录、文件边界写轨道归 exec、db 边界新成员归 infra）；infra 层 Tauri 禁令泛化到未来成员。
- **desktop-app-shell** — 增补（不改动既有 requirement）：command body 三件事纪律与 app 层微形态（`*_inner` 缝）；不建 app crate 的裁决与五条翻转信号。

---

## 变更范围

### 实现文件

- `packages/desktop/src-tauri/src/commands/mod.rs` — 模块文档补充 command body 纪律（参数转换 / 调用 / 错误映射三件事；编排下推 core 或触发 app crate 决策；详见变更内 `specs/desktop-app-shell`）
- 变更内三份 spec delta（`specs/desktop-data-dimensions/`、`specs/desktop-crate-layout/`、`specs/desktop-app-shell/`）为本变更主体产物，随 archive 落入 `openspec/specs/`

### 测试文件

- 无——零行为变更，无新测试面。既有 `cargo test --workspace`（含 `commands/workspaces/mod_test.rs`）与前端套件须保持全绿，作为"未破坏现状"的回归证据。

### 删除文件

- 无

### 不要修改

- **不新建目录 / crate**：不建 `infra/graph`（图数据库暂不考虑）、不建 `infra/api`（api 先占位留痕）、不建 `infra/fs`（文件边界长出独立适配再立）
- **不抽 app 层 crate**：`commands/` 结构与 `workspaces/mod.rs` 的 `*_inner` 模式保持原样（平移缝保留）
- **store crate**：公共 API、表结构（`user_workspaces` / `user_meta`）、单进程约束注释均不变
- **core 两 crate**：零 Tauri 依赖维持；根 `Cargo.toml` 既有注释（"core 与 infra 均禁 Tauri"）维持
- **exec 空轨道**：不实现命令、不建空壳 trait（既有决策不变）
- **前端与既有命令行为**：7 条命令、hooks、视图零变更
- **既有能力语义**：`desktop-change-queries` / `desktop-change-parse` / `desktop-artifact-plugins` / `desktop-workspace-store` / `desktop-corpus-regression` 等不受影响

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | 新增 `desktop-data-dimensions` 能力 spec | 变更 `specs/desktop-data-dimensions/spec.md` 存在：含 user / workspace 两维度定义（内容、生命周期、代表）；workspace 维度 SHALL 落盘 workspace 内（`.openspec/` 下、路径经 foundation 收口）；三笔账（redb 独占锁错误语义 MUST 显式、gitignore 按数据类型分型 MUST NOT 一刀切、数据 SHALL 可重建）写为落地 MUST/SHALL 约束 |
| AC-2 | `desktop-crate-layout` 增补边界与租户规则 | 变更 `specs/desktop-crate-layout/spec.md` 含 ADDED requirement：四类边界定义；租户归位规则含"图数据库暂不考虑""不建 `infra/api` 目录""workspace 写文件归 exec 轨道"；infra 现有与未来成员禁 Tauri；磁盘上 `crates/infra/` 仅 store 一个成员，无空目录 |
| AC-3 | `desktop-app-shell` 增补纪律与翻转信号 | 变更 `specs/desktop-app-shell/spec.md` 含 ADDED requirement：command body 三件事纪律（MUST NOT 在 command 层做领域解释或跨边界协调）；`*_inner` app 层微形态确认；五条翻转信号成文且可机械判定 |
| AC-4 | `commands/mod.rs` 纪律注释 | 模块文档声明 command body 三件事纪律，并指向 spec 决策记录；无其他代码改动 |
| AC-5 | 零行为变更 | 无新增 crate / 目录 / 依赖；`cargo test --workspace` 与前端测试套件全绿；`commands/exec/` 仍为空轨道 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 长期方向约束"对未来说话"，软约束易被未来变更忽视 | 归位规则失效，回到逐变更重新辩论 | 中 | 约束全部写成可机械判定的形式：目录存在性可 ls、Tauri 依赖可查 Cargo.toml、命令行数可数；裁决留 spec + proposal 双存档，archive 后可考 |
| 三笔账（独占锁 / gitignore 分型 / 克隆重建）在未来 workspace 维度变更中遗失 | 落地时踩坑：双开失败无错误语义、派生缓存被误提交、换机器后数据丢失不可重建 | 中 | 三笔账写为 `desktop-data-dimensions` spec 的 MUST/SHALL 落地约束；未来 workspace 维度 change 的 proposal/dev-design 必须引用该 spec |
| 纯文档变更被误读为"没有实现内容" | 验收困惑、变更被误判未完成 | 低 | AC-1~AC-4 明确验收物（三份 spec + 一处模块注释），AC-5 明确"零行为变更 + 回归全绿"即完成态 |
| command 逐渐长大而翻转信号无人查看 | app 层膨胀后被迫大改 | 低 | 机械判据兜底（单命令非样板逻辑 >30 行、`commands/` 非样板逻辑持续增长）入 spec，代码评审可引用；`*_inner` 平移缝保证推迟决定成本极低 |
| `.openspec/` 目录未来更名（"该名称未来要改"为已知规划）与 workspace 维度落盘路径冲突 | 落盘路径与命名隔离约束打架 | 低 | spec 只定"workspace 内落盘"方向；路径解析 SHALL 经 foundation layout 解析器收口（沿用既有路径收口约束），具体目录层留未来变更 dev-design |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| 本变更形态 | 规格收敛变更：零行为变更，主体为三份 spec delta + 一处模块注释 | explore 裁决只在一次性笔记里无法约束未来变更；spec 体系是可执行、可考的治理载体 | 只留 explore 不立 spec（不可考、不可执行）；顺带实现某项裁决（无一条属于本变更） |
| workspace 维度落盘 | b) workspace 内落盘（`.openspec/` 下，具体层留 dev-design） | 数据跟随 repo，克隆重建语义恰是优点（换机器缓存重建）；gitignore 可选择分享 | a) home_dir 下按 workspace hash 分目录（不污染 repo，但 repo 移动数据不跟随、hash 寻址依赖稳定 canonical key） |
| 图数据库 | 暂不考虑；spec 保留 `infra/graph` 归位记录 | 关系查询真实痛点未现，现在选型是投机 | 现在选型引入（违背 Simplicity first） |
| api 边界 | 先占位：spec 留痕，不建 `infra/api` 目录，不动代码 | 用途未定（同步/遥测/账号？）；空目录零收益 | 建占位目录（目录表达计划仅对已有 crate 图有机械收益，此处无） |
| 独立 app 层 | 中间道路：不建 app crate；command body 三件事纪律 + 五条翻转信号 | 7 条命令全为 3-5 行薄包装，app 层现为空收益纯 pass-through；`*_inner(&Store)` 已是 app 层微形态，将来抽 crate 是函数边界升 crate 边界的零重写平移，推迟成本极低 | 立即抽 app crate（壳体量不足，分量不够）；不加纪律放任生长（漂移风险） |
| 纪律的代码落点 | `commands/mod.rs` 模块注释声明三件事纪律 | 沿 store 单进程约束"以代码注释或 crate 文档显式声明"先例；出现在写新命令的人会看的位置 | 仅 spec 留痕（纪律与代码落点分离，易被绕过） |
| infra 禁 Tauri 的表达 | 维持根 `Cargo.toml` 既有注释 + spec 增补为未来成员规则 | 注释已存在（"core 与 infra 均禁 Tauri"），无需改代码；spec 化使其约束到 graph / api 等未来成员 | 改注释措辞（零信息增量） |

### 待决问题

- workspace 维度落盘具体目录层（`.openspec/` 下哪一层）——未来 workspace 维度 change 的 dev-design 定
- redb 独占锁冲突的错误语义（重试 / 只读降级）——同上，届时定
- gitignore 按数据类型分的目录表——同上；若出现值得随 repo 分享的状态再分目录
- 图数据库产品选型与引入时机——关系查询真实痛点出现时重启
- api 边界的用途与形态——真实需求出现时重启

---
