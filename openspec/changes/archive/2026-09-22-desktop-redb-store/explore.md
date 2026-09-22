# 探索笔记：Desktop 引入 redb 本地数据库，实现 store 层（MVP：workspace 清单）

- 日期：2026-09-21
- 状态：探索完成，决策已拍板，待起 change

## 背景与动机

当前 desktop app（Tauri 2）的 workspace root 只存在 React `useState` 里（`App.tsx`），
**重启即失**，每次打开都要重新走文件夹选择器。Rust 侧三个查询命令
（`list_changes` / `get_change_detail` / `read_artifact`）均为无状态薄包装，
不持有任何状态、无持久化。

本次引入 redb 本地数据库、实现 store 层，MVP 租户为 workspace 清单：
记录打开过的 workspace，重启后自动恢复 + Header 下拉切换。

**动机自觉**：MVP 数据量（几个路径）下 JSON 文件 + 原子改名写即够；
选 redb 的价值在"层"——为后续租户（change 元数据缓存/索引、跨 workspace
搜索、设置项）打底。属"为可预期增长提前铺一层"，非 MVP 刚需。
不为未来可能的存储后端替换预抽象 KV trait（投机性设计，违反 Simplicity first）。

对比结论（备查）：rusqlite 引 C 依赖构建链变重；sled 多年 beta 不建议；
JSON 文件是"只有清单"场景下的诚实答案，但会让后续租户每次全量重写。

## 已拍板决策

| # | 问题 | 决策 |
|---|------|------|
| 1 | key 方案 | **A：canonical path 作 key**。去重免费（insert 即 upsert）、无需 id 分配器；目录移动 = 删旧加新，可接受。将来需要身份连续性时再迁 u64 id 方案 |
| 2 | 自动恢复 | **是**。重启自动恢复上次打开的 workspace；同时 Header 增加 workspace 下拉切换 |
| 3 | name 字段 | **目录名最后一段**；记录同时返回 path，前端悬停展示完整路径 |
| 4 | 错误约定 | **`Result<T, String>`** 起步。Tauri 把 Err 变前端 reject，现有 hooks 的 `setError(String(err))` 正好接住。此约定为后续所有可失败命令的模板 |
| 5 | redb 版本 | **最新（4.3.0，2026-09-15 发布）**，实现时锁定具体版本号 |
| 6 | missing 标记 | **不进 MVP**。已消失目录的检查放在选中后（现有"空列表"语义兜底） |
| 7 | 测试注册 | **新 crate 无需注册套件**。rust 套件已收敛为 src-tauri 根单条 + `cargo test --workspace`（openspec/config.json 已改），新 crate 仅需加入 `[workspace].members` |

## 架构落位

> 2026-09-21 修订：分层讨论深化后改落 `crates/infra/store`（store 属 db 边界
> 的外层基础设施，不进 core），详见 `desktop-backend-architecture.md`。

```
crates/core/foundation   (纯路径推导)
crates/core/workflow     (解析/查询/产物)
crates/infra/store       ★ 新增：redb + WorkspaceStore（db 边界第一成员）

  依赖方向：  desktop-app ──▶ infra/store / core 两个 crate
              core 依赖不到 store（crate 图机械保证）
  规则：      core 与 infra 均禁 Tauri 依赖；redb 不是 Tauri，不违规
```

关键设计点：

1. **db 路径注入，不在 core 解析**。Tauri `home_dir()` 离不开 Tauri 上下文。
   store crate 暴露 `Store::open(path: &Path)`，desktop-app 在 setup 里解析
   `%APPDATA%/<ident>/desktop-store.redb`（具体文件名实现时定）后注入。
   store crate 用 tempdir 即可测，保持 core 可测试性。
2. **生命周期挂 Tauri State**：`main.rs` `.manage(store)`，新命令模块
   `commands/workspaces` 引用 `State<Store>`。redb `Database` 是
   `Send + Sync`（进程内 MVCC，单写多读），同步命令直接用，不需要 async。

## 数据模型

```
TableDefinition<&str, &[u8]>  WORKSPACES
  key   = canonical root path
  value = serde_json(WorkspaceRecord)

WorkspaceRecord {
  root: String,                          // canonical
  name: String,                          // 目录名最后一段，展示用
  added_at: OffsetDateTime,
  last_opened_at: Option<OffsetDateTime>,
}
```

无论现在是否用 id 方案，都建 META 表存 `schema_version`（redb 无内建迁移，
从第一天留好这颗牙）。

"上次打开的 workspace"不设单独 API：`list` 按 `last_opened_at` 降序，第一名即是。

## 命令面（MVP）

```
list_workspaces()              -> Result<Vec<WorkspaceRecord>, String>  // last_opened_at 降序
add_workspace(root: String)    -> Result<WorkspaceRecord, String>       // canonicalize + upsert + touch
remove_workspace(root: String) -> Result<(), String>
touch_workspace(root: String)  -> Result<(), String>                    // 打开时刷新 last_opened_at
```

前端影响：欢迎屏从"选文件夹"变为"最近 workspace 清单 + 添加新文件夹"；
Header 增加 workspace 下拉（悬停 title 展示完整 path）；启动时自动恢复
list 第一名（无记录则停欢迎屏）。

## 风险与坑（按痛感排序）

1. **Windows canonicalization**：`std::fs::canonicalize` 返回 `\\?\C:\...`
   UNC 前缀，存库与展示均难看；Windows 路径大小写不敏感，`D:\a\b` 与
   `D:\a\B` 是同一目录。候选：`dunce::canonicalize` 去前缀，或存原样 +
   去重时归一化比较。MVP 最容易踩的语义坑，dev-design 必须明确口径。
2. **第一个真正会失败的命令**：现有命令从不报错（最差返回 None/空列表）；
   db 打不开、磁盘写失败如何呈现，错误约定见决策 #4。
3. **redb 单进程模型**：面向单进程嵌入设计，双开（dev + 已装正式版指向同一
   app_data_dir）场景不保证安全。列为已知约束，tauri-plugin-single-instance
   是后手，不进 MVP。
4. **db 打开失败的启动策略**：MVP 直接启动失败并报错，不静默降级
   （静默降级会让"清单丢失"变成无法排查的玄学）。
5. **目录消失**：不进 MVP（决策 #6），选中后按空列表语义兜底。

## 测试口径

- tempdir 集成测试：真开 redb 文件跑 add/list/remove/touch 全链路
- 只测自研层：record 编解码、canonicalize/去重、排序组装、命令映射；
  redb 自带的事务/持久化语义不逐项验证（与 vitest 不测库语义同原则）
- 新 crate 加入 `[workspace].members` 即被 `cargo test --workspace` 收编，
  无需套件注册；mutants 照常覆盖纯逻辑
