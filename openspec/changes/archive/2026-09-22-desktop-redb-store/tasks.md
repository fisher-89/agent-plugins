# 任务: desktop-redb-store

> **变更**: desktop-redb-store
> **日期**: 2026-09-21

按依赖排序：store crate（被依赖的最底层）→ 命令轨道与启动装配 → 前端 DTO/hook → 视图与壳 → 收口自检。测试文件不在本清单（test-design / test-gen 阶段承接）。

## Phase A：store crate 骨架与 workspace 清单收敛

- [x] A1 新建 `packages/desktop/src-tauri/crates/infra/store/Cargo.toml`：crate 名 `store`、edition 2021；`[dependencies]` 仅 `redb` / `dunce` / `serde` / `serde_json`（均 workspace 引用）、`[dev-dependencies]` `tempfile`；无任何 Tauri 依赖、无 core crate 依赖（AC-1）
- [x] A2 修改 `packages/desktop/src-tauri/Cargo.toml`：`[workspace].members` 增 `crates/infra/store`；`[workspace.dependencies]` 收敛 `redb = "=4.3.0"`、`dunce = "1"`、`tempfile = "3"`、`store = { path = "crates/infra/store" }`；desktop-app `[dependencies]` 增 `store` 并新增 `[dev-dependencies]`（tempfile）；把「core 两个 crate 禁用任何 Tauri 依赖」注释更新为「core 与 infra 均禁 Tauri」（AC-1/AC-10，不动 `openspec/config.json`）
- [x] A3 新建 `packages/desktop/src-tauri/crates/infra/store/src/lib.rs` 最小骨架：crate 文档注释（单进程约束显式声明、db 路径注入约定、schema_version 演进说明），使 `cargo check --workspace` 可通过

## Phase B：store 纯层（model + canonical）

- [x] B1 新建 `packages/desktop/src-tauri/crates/infra/store/src/model.rs`：`WorkspaceRecord`（serde camelCase，`root` / `name` / `added_at: i64` / `last_opened_at: i64`）、`from_root`（name = 目录名最后一段）、JSON 编解码辅助、`now_millis`；在 `lib.rs` 挂 `mod model;` 与 `pub use model::WorkspaceRecord;`
- [x] B2 新建 `packages/desktop/src-tauri/crates/infra/store/src/canonical.rs`：`canonical_key`（`dunce::canonicalize` 主口径，失败返回 Err）与消失目录的词法归一化回退匹配（分隔符统一 `\`、去 `\\?\` 前缀、与存量 key ASCII 大小写不敏感比较）；在 `lib.rs` 挂 `mod canonical;`

## Phase C：Store 持久化与公共面收口

- [x] C1 新建 `packages/desktop/src-tauri/crates/infra/store/src/store.rs`：`StoreError`（`Db(String)` / `Canonicalize(String)`，impl `Display` + `std::error::Error`）、表定义 `user_workspaces`（`TableDefinition<&str, &[u8]>`）与 `user_meta`（`TableDefinition<&str, u64>`）、`Store::open`（`create_dir_all` 父目录 + `Database::create` + `schema_version` 写入/版本校验）、四操作 `add_workspace`（canonicalize + upsert 保留 `added_at` + touch）/ `list_workspaces`（降序 + root 字典序 tie-break）/ `remove_workspace` / `touch_workspace`（两者 miss 幂等 `Ok(false)`，均走 canonical 或回退匹配）；在 `lib.rs` 挂 `mod store;` 并收敛 `pub use store::{Store, StoreError};`

## Phase D：命令轨道与启动装配

- [x] D1 新建 `packages/desktop/src-tauri/src/commands/workspaces/mod.rs`：四命令 `list_workspaces` / `add_workspace` / `remove_workspace` / `touch_workspace`——签名 `(store: State<'_, Store>, ...)`、命令体为参数转换（`String` → `Path`）+ 委托私有 `*_inner(&Store, ...)` + `.map_err(|e| e.to_string())`；模块注释声明「后续可失败命令的 `Result<T, String>` 错误约定模板」（AC-7）
- [x] D2 修改 `packages/desktop/src-tauri/src/commands/mod.rs`：增 `pub mod workspaces;`（三轨注释：queries / workspaces / exec 预留）
- [x] D3 修改 `packages/desktop/src-tauri/src/main.rs`：新增 `.setup()`——`app.path().home_dir()` 拼接 `.dev-team/desktop-store.redb` → `Store::open` → `app.manage(store)`，setup 闭包任一步失败即返回 Err（启动失败报错，fail fast，AC-8）；`invoke_handler` 增注四个 workspace 命令

## Phase E：前端取数层

- [x] E1 修改 `packages/desktop/src/types/dto.ts`：增 `WorkspaceRecord` interface（`root: string` / `name: string` / `addedAt: number` / `lastOpenedAt: number`，对齐 serde camelCase）
- [x] E2 新建 `packages/desktop/src/hooks/useWorkspaces.ts`：`useWorkspaces(): WorkspaceState`——挂载自动 `invoke("list_workspaces")` 一次（唯一自动取数例外）；`add`（invoke `add_workspace`，成功以 canonical 记录刷新清单、失败置 error 态返回 null）、`remove` / `touch`（invoke 对应命令，成功刷新清单、失败返回 false）；无轮询无 watch

## Phase F：视图与壳

- [x] F1 新建 `packages/desktop/src/views/WelcomeView.tsx`：欢迎屏——空清单空态 +「添加新文件夹」入口 + 加载中/error-note 呈现（props 收 `WorkspaceState` 与添加回调）
- [x] F2 修改 `packages/desktop/src/App.tsx` 与 `packages/desktop/src/hooks/useWorkspaces.ts`：恢复收进 hook（`restoredRef` 一次守卫——取得首个非空清单时 fire-and-forget `touch` 第一名后 `setRoot`，防止取数-恢复循环）；root 恒等于清单第一名（`last_opened_at` 降序）；Header 改为 workspace 下拉（清单项 title 完整 path、选中切换时清空 change 选中并 touch、带移除按钮）；移除当前打开项 → 刷新后切换到剩余第一名（清单空时回欢迎屏）；对话框添加流改经 `useWorkspaces().add`，刷新后新记录即第一名打开；workspace 错误态在欢迎屏与 Header 呈现

## Phase G：收口自检

- [x] G1 全量编译自检：`packages/desktop/src-tauri` 下 `cargo check --workspace` 通过（store crate 被 workspace 收编）；`pnpm -C packages/desktop run build`（tsc + vp build）通过；`git status` 确认 `openspec/config.json` 零改动（AC-10）
