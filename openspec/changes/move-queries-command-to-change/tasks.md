# 任务: move-queries-command-to-change

> 测试文件的编写与执行由 test-design / test-gen / test-execution 阶段承接，本列表只覆盖 design.md 变更清单的实现文件。`queries/mod_test.rs` 的删除是轨道消解的结构性组成（随模块删，非测试编写），其 7 个用例的随迁落盘归测试轨道（原文基准经 git 历史核对，design D5）。顺序按依赖排列：命令平移 → 登记面与轨道消解 → 注释清扫 → 守线。版本不 bump（AC-5，design D7），零 `packages/desktop/package.json` 触点。

## 阶段一：三读命令平移与组 doc 二分重写

- [ ] 修改 `packages/desktop/src-tauri/src/commands/changes/mod.rs`：自 `commands/queries/mod.rs` 逐字平移 `is_blank_root` 助手（`fn is_blank_root(root: &str) -> bool` 及其 doc）与三读命令 `list_changes` / `get_change_detail` / `read_artifact`——函数名、签名、`#[tauri::command]` + `#[specta::specta]` 属性、命令体逻辑与逐函数 doc 注释零改动（design D2），置于 `create_change` 之前（读先写后，沿 explores 组内次序，design D1）；import 块机械合并去重（`std::path::Path` 与 `foundation::layout::resolve` 单一见，新增 `workflow::artifacts` / `workflow::model` / `workflow::parse` / `workflow::queries` 四组 use）；`create_change` 命令体与既有 `#[cfg(test)] mod mod_test;` 声明位零改动——内联 blank root 判定不顺手改用 `is_blank_root`（写面零改动红线，design D2）
- [ ] 同文件模块 doc 重写（design D1）：「三分职责」改「二分」——本组（change 域读 + 记录面，沿 explores 组同组先例：三读命令 + `create_change`）/ `change_flow`（run 编排控制）；组内 blank root 双口径并存且 MUST NOT 互换入 doc（读命令空结果语义 / `create_change` 显式 `Err`）；`queries/mod.rs` 既有模块 doc 的「IPC 字符串入参的显式格式/包含性检查口径」段（root / change / source / kind 四条与「无 State、不持有或缓存 workspace 状态、无直接文件系统访问」薄包装声明）随读命令并入组 doc；能力 spec 指针增补 `specs/desktop-app-shell/spec.md`（与既有 `specs/desktop-change-create/spec.md` 并列，路径相对域根定式，不含 layout 命名隔离扫描的双禁令字面量）

## 阶段二：登记面 re-path 与 queries 轨道消解

- [ ] 修改 `packages/desktop/src-tauri/src/commands/mod.rs`：移除 `pub mod queries;`（其余 `pub mod` 声明仍保持字母序）；`all_commands!` 宏清单内三条目（`list_changes` / `get_change_detail` / `read_artifact`）in-place re-path——路径段 `$crate::commands::queries` → `$crate::commands::changes`，宏清单内位置零位移、不聚拢到 `changes::create_change` 相邻处（design D3：登记序与 specta 出线序耦合性未验证，保守零位移保 bindings 零 diff）；`main.rs` 的 `generate_handler` 与 `src/bindings` 的 specta builder 经宏同源自动跟随，零额外触点
- [ ] 删除 `packages/desktop/src-tauri/src/commands/queries/mod.rs` 与 `packages/desktop/src-tauri/src/commands/queries/mod_test.rs`（目录整体消解：无空目录、无空模块声明、无 re-export shim，design D5）；删除后 `src/commands/` 目录树无 `queries` 条目

## 阶段三：源码 doc 注释残留清扫（注释级，无逻辑）

- [ ] 修改 `packages/desktop/src-tauri/src/commands/explores/mod.rs`：模块 doc「三件事纪律沿 `commands/queries` 模板」与 `is_blank_root` doc「（同 `commands::queries` 口径）」两处引用 re-point 为 `commands/changes`（前者为 proposal 清单外残留吸收、后者为 proposal 实现文件条目，design D4）
- [ ] 修改 `packages/desktop/src-tauri/src/commands/config/mod.rs` 与 `packages/desktop/src-tauri/src/commands/stats/mod.rs`：各一处模块 doc「三件事纪律沿 `commands/queries` 模板」re-point 为 `commands/changes`（proposal 清单外注释级残留吸收，提案评估留痕「design 相位可吸收」，design D4）；两组命令逻辑与其余 doc 零改动

## 阶段四：守线（静态，不含测试执行）

- [ ] 静态检查全绿：`pnpm -C packages/desktop run server:check`（cargo fmt + clippy，即 AC-1 的 `cargo check` 编译面）与 `pnpm -C packages/desktop run client:check`（前端零触点，作零 diff 边界确认，无新增豁免条目）；自动化套件的执行验证由 test-execution 阶段承接
- [ ] `pnpm -C packages/desktop run bindings:check` 零 diff（重导出幂等性：`packages/desktop/src/types/generated/bindings.ts` 零 diff 即 AC-2 验收面，design 变更清单该行为预期零 diff 证明物）
- [ ] 静态自查：`commands/queries` / `commands::queries` 在 `packages/desktop/src-tauri/src/` 源码与 doc 全域零命中（golden fixtures 与 openspec 历史叙述除外，design D6 边界）；`src/commands/` 目录树无 `queries` 目录
- [ ] 静态自查：禁改面零 diff——三命令函数名 / 签名 / 命令体与 DTO 逐字核对（git diff 仅模块落位与 doc 变化）、`crates/core/workflow/**`、前端 `src/**`、`plugins/dev-team/**`、`crates/core/workflow/tests/golden/**` 与 `tests/fixtures/**`、其余命令组逻辑（`explores` / `config` / `stats` 仅 doc 注释行，design D4）
- [ ] 变更清单核对：design.md 变更清单与实际触达文件双向一致（清单外零改动、清单内零遗漏）；`packages/desktop/package.json` 零触点（version 保持 0.4.2，AC-5 / design D7）
