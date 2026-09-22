# 提案: desktop-redb-store

> **变更**: desktop-redb-store
> **日期**: 2026-09-21
> **状态**: draft（待评审）

---

## 问题

桌面端（`packages/desktop`）当前将 workspace root 存于 React `useState`（`App.tsx`），**重启即失**：每次打开应用都要重新走一遍文件夹选择器。Rust 侧三个查询命令（`list_changes` / `get_change_detail` / `read_artifact`）均为无状态薄包装，不持有任何状态、无任何持久化。

同时，后续已规划的特性（change 元数据缓存/索引、跨 workspace 搜索、设置项）都需要一个本地持久化层，目前没有承载位。MVP 数据量（几个路径）下 JSON 文件也够用，但后续租户会退化为每次全量重写；rusqlite 引入 C 依赖构建链变重，sled 多年 beta 不建议。选 redb 的价值在"层"——为可预期增长提前铺一层；同时不预抽象 KV trait / 泛型仓储接口（投机性设计，违反 Simplicity first）。

---

## 提案

引入 redb 嵌入式本地库，新增 `crates/infra/store` crate（db 边界第一成员），实现 workspace 注册表持久化，MVP 租户为 workspace 清单：重启自动恢复上次打开的 workspace + Header 下拉切换 + 欢迎屏最近清单。

关键设计：

- **分层落位**：crate 分层为 `core/`（foundation、workflow）+ `infra/`（store）+ `desktop-app`。store 属 db 边界的外层基础设施，不进 core；依赖方向 `desktop-app → infra/store / core`，core 依赖不到 store（crate 图机械保证），infra 与 core 均禁 Tauri。
- **canonical path 作 key**：WORKSPACES 表 key = canonical root path，insert 即 upsert，去重免费、无需 id 分配器；目录移动 = 删旧加新（用户操作），将来需要身份连续性时再迁 u64 id 方案。
- **数据模型**：`WorkspaceRecord { root, name, added_at, last_opened_at }`，`name` 取目录名最后一段（前端悬停展示完整 path）；`list_workspaces` 按 `last_opened_at` 降序，第一名即"上次打开"，不为此单设 API。META 表从第一天存 `schema_version`（redb 无内建迁移），业务表命名携带 user 维度语义，为 workspace 维度租户留格。
- **db 路径注入**：`home_dir()` 依赖 Tauri 上下文，store crate 暴露 `Store::open(path: &Path)`，desktop-app 在 setup 中解析后注入；生命周期挂 Tauri State（`.manage(store)`）。redb `Database` 为 `Send + Sync`（进程内 MVCC，单写多读），同步命令直接用，无需 async。
- **错误约定模板**：命令面 `Result<T, String>` 起步——Tauri 把 Err 变前端 reject，现有 hooks 的 `setError(String(err))` 正好接住；此约定为后续所有可失败命令的模板。db 打开失败 SHALL fail fast 启动报错，不静默降级（静默降级会让"清单丢失"变成无法排查的玄学）。
- **redb 类型不泄漏**：store 公共 API 只暴露自有类型，`redb::Database` / `Table` 不出现在命令签名与前端 DTO——将来图数据库加入时是 db 边界新成员，而非 store 层重构。
- **前端**：欢迎屏从"选文件夹"变为"最近 workspace 清单 + 添加新文件夹"；Header 增加 workspace 下拉（悬停 title 展示完整 path）；启动时自动恢复 list 第一名（无记录则停欢迎屏）。清单取数收口新 hook `useWorkspaces`（启动自动触发一次，其余仍显式触发）。

测试口径：tempdir 集成测试真开 redb 文件跑 add/list/remove/touch 全链路；只测自研层（record 编解码、canonicalize/去重、排序组装、命令映射），redb 自带的事务/持久化语义不逐项验证。新 crate 加入 `[workspace].members` 即被 `cargo test --workspace` 收编，无需套件注册。

---

## 能力

### 新增能力

- **desktop-workspace-store** — `crates/infra/store` redb 本地库：workspace 注册表（user 维度）持久化模型、canonical path 主键与 upsert 语义、清单排序、类型不泄漏与 schema_version、db 路径注入。

### 修改的能力

- **desktop-crate-layout** — crate 分层从 core/desktop-app 两组扩展为 core/infra/desktop-app 三组：新增 `crates/infra/store`，依赖规则扩展（`desktop-app → store`；store 不依赖 core、不依赖 Tauri；core 依赖不到 store）。
- **desktop-app-shell** — 新增 `commands/workspaces` 注册命令轨道（四命令 + `State<Store>` + `Result<T, String>` 错误约定）；workspace 选择升级为"选择与恢复"（启动自动恢复、欢迎屏清单、Header 下拉切换）；hooks 家族加入 `useWorkspaces`。

---

## 变更范围

### 实现文件

- `packages/desktop/src-tauri/crates/infra/store/` — 新 crate：`Cargo.toml`、`src/lib.rs` 及内部分模块（自含模型）
- `packages/desktop/src-tauri/Cargo.toml` — `[workspace].members` 增加 `crates/infra/store`；`[workspace.dependencies]` 收敛 `redb`（锁定具体版本号，目标 4.3.0）、`store` path 依赖及 canonicalize 口径所需依赖（dev-design 定）
- `packages/desktop/src-tauri/src/main.rs` — 解析 db 路径（app_data_dir）、`Store::open` + `.manage()`、注册 workspace 命令
- `packages/desktop/src-tauri/src/commands/mod.rs` — 挂载 `workspaces` 模块
- `packages/desktop/src-tauri/src/commands/workspaces/` — 新命令模块：`list_workspaces` / `add_workspace` / `remove_workspace` / `touch_workspace`
- `packages/desktop/src/types/dto.ts` — `WorkspaceRecord` DTO（与 store 记录字段一一对应）
- `packages/desktop/src/hooks/useWorkspaces.ts` — 新 hook（清单取数收口）
- `packages/desktop/src/App.tsx` 及欢迎屏 / Header 视图 — 启动自动恢复、最近清单、下拉切换、移除项

### 测试文件

- `packages/desktop/src-tauri/crates/infra/store/src/*_test.rs` — tempdir 全链路（add/list/remove/touch + 重开持久化）、等价路径去重、排序组装、schema_version 写入
- `packages/desktop/src-tauri/src/commands/workspaces/mod_test.rs` — 命令映射与 `Err(String)` 错误传递（随 `queries/mod_test.rs` 既有模式）
- `packages/desktop/src/hooks/useWorkspaces.test.ts` — 启动自动触发、错误态
- `packages/desktop/src/App.test.tsx` — 自动恢复第一名 / 无记录停欢迎屏 / 下拉切换 / 移除当前项切换剩余第一名

### 删除文件

- 无

### 不要修改

- `crates/core/foundation` 与 `crates/core/workflow` — core 不依赖 store；workflow 纯读库定位保持（读写分离，写轨道等 exec 落地）
- `packages/desktop/src-tauri/src/commands/queries/` — 三个既有查询命令语义不变
- `packages/desktop/src-tauri/src/commands/exec/` — 保持预留空轨道，无实现、无空壳 trait
- `openspec/config.json` — rust 套件注册已收敛为 src-tauri 根单条 + `cargo test --workspace`，新 crate 仅需进 members，MUST NOT 新增套件注册
- `desktop-change-queries` / `desktop-change-parse` / `desktop-artifact-plugins` 等既有能力语义

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | 新增 `crates/infra/store` crate | 落位 `crates/infra/store` 并加入 `[workspace].members`；`Cargo.toml` 无 Tauri 依赖、无 core crate 依赖 |
| AC-2 | `Store::open(path)` + add/list/remove/touch 四操作 | tempdir 集成测试覆盖全链路，重开同一 db 文件后记录仍在 |
| AC-3 | canonical path 作 key 的 upsert 去重 | 等价路径（大小写/尾分隔符等书写差异）重复 add 仅一条记录且 `last_opened_at` 刷新；等价路径 touch/remove 命中同一条 |
| AC-4 | 清单排序与 name 字段 | `list_workspaces` 按 `last_opened_at` 降序，第一名即最近打开；`name` 为目录名最后一段 |
| AC-5 | META 表 schema_version + 表维度命名 | 新开 db 即写入 `schema_version`；业务表命名含 user 维度语义 |
| AC-6 | redb 类型不泄漏 | store 公共 API、desktop-app 命令签名、前端 DTO 中均无 redb 类型 |
| AC-7 | workspace 四命令 + `State<Store>` + `Result<T, String>` | 四命令注册于 invoke_handler，经 `State<Store>` 访问 store；失败路径返回 `Err(String)` 并在前端 error 态呈现（测试断言） |
| AC-8 | db 打开失败 fail fast | db 文件打不开时启动失败并报错，无静默空清单降级 |
| AC-9 | 前端自动恢复 + 欢迎屏空态 + Header 下拉 | App 测试：有记录时启动自动恢复 `last_opened_at` 第一名；无记录停欢迎屏；下拉切换清空 change 选中并按新根取数；移除当前打开项切换到剩余第一名（清单空时回欢迎屏） |
| AC-10 | `cargo test --workspace` 收编新 crate | 不修改 `openspec/config.json` 套件注册，新 crate 测试被 `cargo test --workspace` 覆盖 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| Windows canonicalization 口径：`std::fs::canonicalize` 返回 `\\?\` UNC 前缀；路径大小写不敏感（`D:\a\b` 与 `D:\a\B` 同一目录） | 存库 key 与去重比较口径不一致 → 重复记录或 touch/remove 不命中 | 高 | dev-design 必须拍板单一口径（如 `dunce::canonicalize` 去前缀，或存原样 + 归一化比较）并全链路一致；AC-3 用等价路径测试锁死 |
| 首个真正会失败的命令：现有命令从不报错，db 打不开/磁盘写失败是新错误面 | 错误被吞，用户面对空清单玄学 | 中 | `Result<T, String>` 约定 + AC-7/AC-8 错误路径测试；fail fast 不静默降级 |
| redb 单进程模型：双开（dev + 正式版指向同一 app_data_dir）不保证安全 | 数据竞争/损坏 | 低（已知约束） | 约束以代码注释/文档显式声明；tauri-plugin-single-instance 为后手，不进 MVP |
| redb 4.3.0 发布较新（2026-09-15） | API 变动或 Windows 兼容问题 | 低 | 实现时锁定具体版本号；tempdir 集成测试真开文件验证 |
| 自动恢复 + touch 的前端时序 | 首帧闪烁或恢复失败体验差 | 低 | 收口 `useWorkspaces` hook；失败呈现错误态，不阻断手动添加路径 |
| 目录消失（已移除/改名的工作区仍在清单） | 选中后得到空列表，用户困惑 | 低 | 决策：不进 MVP；选中后按既有空列表语义兜底 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| 主键方案 | canonical path 作 key | 去重免费（insert 即 upsert）、无需 id 分配器；目录移动 = 删旧加新可接受 | u64 id + 分配器（需要身份连续性时再迁） |
| 是否自动恢复 | 是；同时 Header 增加下拉切换 | "重启即失、每次重选"是当前最高频痛点 | 仅手动下拉、无自动恢复 |
| name 字段口径 | 目录名最后一段；记录同时返回 path，悬停展示完整路径 | 展示简洁且信息不丢 | 用户自定义别名（投机） |
| 错误约定 | 命令面 `Result<T, String>` 起步 | Tauri Err → 前端 reject，现有 `setError(String(err))` 正好接住；为后续可失败命令立模板 | 自定义错误 enum（MVP 过重） |
| redb 版本 | 最新 4.3.0（2026-09-15 发布），实现时锁定具体版本号 | 含最新修复，锁版本保证可复现构建 | 锁旧版（无收益） |
| 目录消失标记 | 不进 MVP | 选中后既有"空列表"语义已兜底 | 库内定期校验目录存在性 |
| store 落位 | `crates/infra/store`，不进 core | store 属 db 边界外层基础设施；crate 图机械保证 core 纯读域 | 进 `core/`（混淆领域核心与基础设施） |
| 存储选型 | redb | 为后续租户（缓存/索引/设置）打底；rusqlite 引 C 链重、sled 多年 beta、JSON 文件后续租户全量重写 | JSON 文件 / rusqlite / sled |
| 是否预抽象 KV trait | 不抽象（无 port trait、无泛型仓储接口） | 违反 Simplicity first；crate 边界 + store 内纯/杂分模块已是将来抽接口的现成缝 | 预定义 port trait，infra 实现 |
| 测试注册 | 新 crate 无需注册套件 | rust 套件已收敛为 src-tauri 根单条 + `cargo test --workspace`，新 crate 加入 `[workspace].members` 即被收编 | 为 store 单独注册套件（重复收敛成果） |

### 待决问题

- Windows canonicalize 具体口径：`dunce::canonicalize` 去前缀 vs 存原样 + 去重时归一化比较；含大小写归一化策略——dev-design 必须明确单一口径
- db 文件名（占位 `desktop-store.redb`）与 home_dir 下是否分子目录
- 自动恢复路径是否同时 touch（本提案按"选中即 touch"口径写，dev-design 可复核）
- workspace 维度落盘候选（app_data_dir 下按 hash 分目录 vs repo 内落盘）属长期架构探索，不阻塞本变更
- 图数据库选型与 api 边界用途：等真实痛点出现再进，不在本变更范围

---
