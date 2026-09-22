# 任务: desktop-backend-architecture

> **变更**: desktop-backend-architecture
> **日期**: 2026-09-22（attempt 2 修订）

按依赖排序：三份 spec delta 与 proposal.md 已随提案阶段同步完成，不设任务；唯一实现落点是 `commands/mod.rs` 模块注释。attempt 1 已写入纪律段但 spec 指针含被禁字面量（test-execution 回溯根因），本轮 Phase A 将其修正为相对域根定式（design D2/D5），其后是零行为约束的收口自检（Phase B，新增 B3 命名隔离复核）。本变更无新测试面，既有套件回归（含 layout_test 用例）由测试阶段承接，本清单无测试任务。

## Phase A：spec 指针修正（唯一代码落点）

- [x] A1 修改 `packages/desktop/src-tauri/src/commands/mod.rs`：模块文档（`//!`）纪律段四要素（三件事枚举 / 两条禁令 / `*_inner` 微形态保留声明 / 决策出处）文字保持不变，仅将决策出处指针由全路径 `openspec/specs/desktop-app-shell/spec.md` 替换为相对域根定式 `` `specs/desktop-app-shell/spec.md` `` +「路径相对域根」限定语；替换后注释全段（含中文行文与代码 span）不含字面量 `openspec`；纪律段维持 9 行（6~10 行约束内），文件其余内容（`pub mod exec; pub mod queries; pub mod workspaces;` 三行与既有两行注释）零改动（AC-4）
- [x] A2 只读复核修正后注释与其余三要素文字仍与本变更 desktop-app-shell 能力 delta 的 ADDED requirement 对齐（三件事 / 两禁令 / 微形态 / 翻转信号指针），未引入 spec 未载的新纪律；中文 `//!` 风格不变（AC-3/AC-4）

## Phase B：零行为约束收口自检

- [x] B1 只读复核治理基线未被实施过程破坏：`crates/infra/` 仅 store 一个成员且无空目录（无 `infra/graph` / `infra/api` / `infra/fs`）；`commands/exec/` 仍为空轨道（无实现、无空壳 trait）；无新增 crate / 目录（AC-2/AC-5）
- [x] B2 只读复核依赖与既有面未回退：`packages/desktop/src-tauri/Cargo.toml` 的 members 与 `[workspace.dependencies]` 逐行未变（无新增依赖，「core 与 infra 均禁 Tauri」注释原样）；7 条命令注册与 `commands/workspaces` 的 `*_inner` 模式、`crates/infra/store` 公共 API / 表结构 / 单进程约束注释均与变更前一致；core 两 crate 零 Tauri 依赖（AC-5）
- [x] B3 只读复核命名隔离（design D5）：grep `packages/desktop/src-tauri/src/` 与 `crates/*/src` 下产品源码（`*_test.rs` 之外）无字面量 `openspec`，重点确认 `commands/mod.rs` 替换后为零命中；该约束的机械保障为既有 layout_test 用例「全包源码不含_openspec_字样_layout_为唯一例外」，回归由测试阶段承接，不在本变更新增测试（AC-4/AC-5）
