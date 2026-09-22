# 测试设计: desktop-backend-architecture

> **日期**: 2026-09-22（attempt 2 修订）

> **修订说明（上一版 → 本版）**: 上一版对 AC-4 的断言「注释决策出处指向 `openspec/specs/desktop-app-shell/spec.md` 全路径」**废弃**——该全路径含字面量 `openspec`，被既有 layout_test 命名隔离用例「全包源码不含_openspec_字样_layout_为唯一例外」拦截（test-execution 回溯根因，design D2/D5）；本版目标态改为相对域根定式 `` `specs/desktop-app-shell/spec.md` `` +「路径相对域根」限定语。其余路由维持不变：AC-1~AC-4 不可测试（静态评审承接）、AC-5 单元测试（既有套件回归路由）。本版仍不新设任何用例，用例表无「新增/废弃」行可记，废弃断言由本说明与不可测试项 4 承载，不静默删除。

---

## 验收范围

<!-- 测试框架口径（由 test_detect_frameworks 识别）：
  - Rust 侧：测试套注册于 packages/desktop/src-tauri 根（cargo workspace），colocated `*_test.rs` + `#[test]` 既有模式（commands/queries/mod_test.rs、commands/workspaces/mod_test.rs、crates/core/foundation/src/layout_test.rs、crates/infra/store/src/store_test.rs 等），workspace 成员自动收编
  - 前端侧：vite-plus（packages/desktop）
  本变更为规格收敛变更：proposal「变更范围 - 测试文件」明确为无——零行为变更、无新测试面，本设计不新设任何用例。对上一版「亦无既有用例受影响」的断言做一次更正：既有 layout_test 命名隔离用例因 attempt 1 注释字面量当前为红，A1 指针替换后应转绿——该用例是既有套件成员，属 AC-5 回归证据面，不因「由红转绿」改判为本变更的用例或集成关系（design D5：不在本变更新增任何测试）。AC-1~AC-4 为规格文本与静态结构约束（无运行时断言形态），验收证据由 acceptance 静态评审承接（见不可测试项）；AC-5 的可自动化半边即「既有套件保持全绿（含 layout_test 用例转绿）」，作为「未破坏现状」的回归证据。 -->

| AC ID | 验收条件 | 测试类型 | 被测文件或模块 |
|--------|---------|---------|----------|
| AC-1 | 变更 `specs/desktop-data-dimensions/spec.md` 存在：含 user / workspace 两维度定义（内容、生命周期、代表）；workspace 维度 SHALL 落盘 workspace 内（`.openspec/` 下、路径经 foundation 收口）；三笔账（redb 独占锁错误语义 MUST 显式、gitignore 按数据类型分型 MUST NOT 一刀切、数据 SHALL 可重建）写为落地 MUST/SHALL 约束 | 不可测试 | —（静态规格文本评审承接，见不可测试项 1） |
| AC-2 | 变更 `specs/desktop-crate-layout/spec.md` 含 ADDED requirement：四类边界定义；租户归位规则含「图数据库暂不考虑」「不建 `infra/api` 目录」「workspace 写文件归 exec 轨道」；infra 现有与未来成员禁 Tauri；磁盘上 `crates/infra/` 仅 store 一个成员，无空目录 | 不可测试 | —（静态规格文本与目录结构约束，见不可测试项 2） |
| AC-3 | 变更 `specs/desktop-app-shell/spec.md` 含 ADDED requirement：command body 三件事纪律（MUST NOT 在 command 层做领域解释或跨边界协调）；`*_inner` app 层微形态确认；五条翻转信号成文且可机械判定 | 不可测试 | —（静态规格文本评审承接，见不可测试项 3） |
| AC-4 | `commands/mod.rs` 模块文档声明 command body 三件事纪律，并指向 spec 决策记录；无其他代码改动 | 不可测试 | —（注释文本内容无运行时断言形态，见不可测试项 4；其命名隔离半边由既有 layout_test 用例机械保障，随 AC-5 回归路由） |
| AC-5 | 无新增 crate / 目录 / 依赖；`cargo test --workspace` 与前端测试套件全绿；`commands/exec/` 仍为空轨道 | 单元测试 | `packages/desktop/src-tauri/src/commands/mod_test.rs`（唯一修改文件 `commands/mod.rs` 的解析测试对——该文件当前不存在，本变更亦不创建，见单元测试章节）；回归面为整个既有 Rust workspace 套（含 foundation/layout_test.rs 命名隔离用例，A1 修正后应转绿）与前端 vite-plus 套（均为既有用例，全绿即证据，无新增用例——静态子句见不可测试项 5） |

---

## 单元测试

<!-- Rust 侧为 colocated `*_test.rs` 模块文件模式，前端侧为 vite-plus 测试；本变更零行为变更，故无任何新增测试文件与新增用例。
  唯一被测文件对来自 test_resolve_paths 解析结果（errors 为空）：唯一修改文件 `commands/mod.rs` -> `commands/mod_test.rs`（该测试文件当前不存在，本变更也不创建它——
  章节保留仅为了声明「此文件对无新增测试面」这一事实，并作为 AC-5 回归路由的落点）。既有全套用例（含 commands/workspaces/mod_test.rs 的
  「workspace命令面 → Store持久化」共置集成用例、foundation/layout_test.rs 的命名隔离用例与 store 单元用例）保持全绿，即本变更「未破坏现状」的全部自动化证据；
  其中命名隔离用例在 attempt 1 现状下为红（commands/mod.rs 注释含禁用字面量），A1 指针替换后应转绿。 -->

### packages/desktop/src-tauri/src/commands/mod.rs -> packages/desktop/src-tauri/src/commands/mod_test.rs

<!-- 本变更对该文件的唯一修改是模块文档（//!）纪律段的指针短语替换（AC-4，design D1/D2：除该注释外零改动——attempt 1 已写入 9 行纪律段，
  本轮仅将决策出处指针由全路径 `openspec/specs/desktop-app-shell/spec.md` 替换为相对域根定式 `specs/desktop-app-shell/spec.md`（路径相对域根），
  替换后注释全段不含字面量 `openspec`；上一版的「追加纪律段且指针写全路径」断言已废弃，见文首修订说明）。注释为源码文本，无运行时行为、
  无签名/导出/命令注册变更；design.md「公共函数 / API」为「无」。因此本章节不设用例：不新设读源码字符串比对的注释断言（测试反模式，
  且 proposal 明确「测试文件：无」）——注释不含禁用字面量这一命名隔离半边的机械化断言已由既有 layout_test 用例承载（扫描范围含 src-tauri/src，
  commands/mod.rs 在其中，唯一豁免为 foundation/layout.rs），本变更不重复建设；注释语法合法性由既有套件编译通过间接覆盖；
  既有三轨挂载（exec/queries/workspaces）与其余模块的行为不变性由既有用例保持全绿给出（AC-5 回归证据）。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（design.md「公共函数 / API」：无——仅模块文档注释，无签名、导出、命令注册变更） -->

#### 用例

<!-- 无独立用例：见章节注释。本变更无任何迭代类型的用例（proposal「测试文件：无——零行为变更，无新测试面」）；
  attempt 1 → attempt 2 的断言更替（全路径指针 → 相对域根指针）不落用例行，由文首修订说明与不可测试项 4 承载。 -->

#### Mock策略

<!-- 无：无新设用例即无 Mock 需求；既有用例的 Mock 装置（commands/workspaces/mod_test.rs 以 tauri::test::mock_app() 的 MockRuntime manage 真实 Store、
  tempdir 真开 redb 文件）不因本变更改变。 -->

---

## 集成测试

<!-- 本变更为规格收敛变更，代码侧唯一落点是 `commands/mod.rs` 模块文档注释的指针短语替换，零行为变更（design「路由/API 设计」：Tauri IPC 命令面零变更，
  7 条命令签名与行为不动，`commands/exec/` 仍为空轨道）。故本变更不引入任何跨模块交互，无集成关系章节。两点边界说明：
  ① 受治理对象（desktop-app 壳、crates/infra/store、core 两 crate）的「零改动基线」行为不变性，不作为本变更的测试对象新设覆盖——
  其由既有集成用例（commands/workspaces/mod_test.rs 共置的「workspace命令面 → Store持久化」关系等）保持全绿作为回归证据承接（AC-5 路由）；
  若为基线模块另设关系章节，即超出 proposal 变更范围（测试设计须与 proposal 范围一致）。
  ② foundation layout_test 的命名隔离扫描虽横跨 src-tauri/src、crates/*/src 与前端树多个模块边界，但该关系是既有测试套件的既有关系，非本变更引入的交互；
  本变更与其的交集仅是「既有用例由红转绿」这一回归事实，按 AC-5 回归路由处理，不新设集成关系章节、不为其虚构关联AC（design D5：不在本变更新增任何测试）。 -->

---

## 不可测试项

<!-- 列出 proposal 范围内但无法通过自动化测试验证的条目，每项说明原因。 -->

- AC-1 desktop-data-dimensions spec delta 文本内容（user / workspace 两维度定义与生命周期、workspace 维度落盘 workspace 内 + foundation 路径收口、三笔账 MUST/SHALL 落地约束条款） — **原因**: 规格文本无运行时断言形态；且该 spec 的约束兑现对象是未来 workspace 维度持久化变更（本变更零代码兑现），文本齐备性由 acceptance 静态评审逐条比对 proposal AC-1 承接。
- AC-2 desktop-crate-layout spec 增补文本与磁盘基线（四类边界定义、租户归位规则；`crates/infra/` 仅 store 一个成员、无空目录、不建 `infra/graph` / `infra/api` / `infra/fs`） — **原因**: 前半为规格文本，同上由静态评审承接；后半为目录/成员存在性的静态结构约束与「不作为」类约束（不建目录不建 crate），无运行时行为可断言，由实现收口自检复核（tasks B1；design「验收标准对齐」AC-2 行已声明磁盘基线核实）。
- AC-3 desktop-app-shell spec 增补文本（command body 三件事纪律、`*_inner` 微形态确认、五条翻转信号成文且可机械判定） — **原因**: 规格文本与对既有代码模式的确认，代码侧零改动（`commands/workspaces` 的 `*_inner` 模式保持原样），无行为变更面可测；「可机械判定」是给未来评审用的判据成文要求，其文本质量由静态评审承接。
- AC-4 `packages/desktop/src-tauri/src/commands/mod.rs` 模块注释内容（三件事枚举、两条禁令、`*_inner` 微形态声明、决策出处四要素） — **原因**: 注释为源码文本约束，无运行时行为；为断言注释内容而读源码做字符串比对属测试反模式，且 proposal 明确「测试文件：无」。决策出处要素的目标态断言已在 attempt 2 更替：上一版的「指向 `openspec/specs/desktop-app-shell/spec.md` 全路径」**废弃**（被既有 layout_test 命名隔离用例拦截，即回溯根因），本版按 design D2 为相对域根定式 `` `specs/desktop-app-shell/spec.md` `` +「路径相对域根」、全段不含字面量 `openspec`。两个可自动化残余均由既有套件承接、不新设测试：① 注释语法合法性（`//!` 文档注释写错会编译失败）由既有套件编译通过间接覆盖（随 AC-5 回归）；② 命名隔离（注释全段不含 `openspec` 子串）由既有 layout_test 用例「全包源码不含_openspec_字样_layout_为唯一例外」机械断言（子串包含判定、区分大小写，扫描范围含 src-tauri/src，唯一豁免 foundation/layout.rs），attempt 1 现状为红、A1 替换后应转绿（随 AC-5 回归路由，不在本项重复计数）；措辞与 spec 的对齐由 acceptance 静态评审按 design D3 逐句比对承接。
- AC-5 静态子句（无新增 crate / 目录 / 依赖；`commands/exec/` 仍为空轨道） — **原因**: 结构不变式无测试断言形态，由实现收口自检承接（tasks B1/B2）；「套件全绿」半边可自动化，已路由为单元测试（见验收范围 AC-5 行），不在本项重复。
- 三笔账兑现（redb 独占锁错误语义显式化、gitignore 按数据类型分型、数据克隆可重建） — **原因**: desktop-data-dimensions spec 将三笔账写为未来 workspace 维度持久化变更落地时 MUST 逐项引用交代的约束，本变更零代码兑现，不存在本变更的测试面；其验证设计属未来变更 test-design 的职责，本变更不预写用例（避免为未落地行为虚构断言）。

