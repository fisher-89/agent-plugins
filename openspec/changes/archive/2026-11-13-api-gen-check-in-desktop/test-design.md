# 测试设计: api-gen-check-in-desktop

> **变更**: api-gen-check-in-desktop
> **角色**: test-design-planner（executor）—— 从 `design.md` 推导单元测试范围与公共 API 签名
> **输入**: `proposal.md`（A + B + D 能力取舍 / P1 写盘归属 / ③ 契约在案即纳管 / U1 生成权限 / 硬线二不锁生成物 / 变更范围 28 实现项 + 31 测试文件行 / AC-1…AC-19）；`design.md`（D1–D22 / 十一则事实校准 / 架构组件 16 行 / 新增文件 10 项（5 实现件 + 5 测试空壳）+ 修改文件 27 行 / 公共函数·API 表 13 行 / 类型定义 8 行 / `api/diff.rs` 内部形态 / 数据模型 / 依赖 / 「测试相位承接面」三行）；`tasks.md`（十三阶段与各阶段自查行）；十份 delta spec（`desktop-archi-contract-panel` 6 ADDED；`desktop-archi-model` RENAMED × 1；`desktop-archi-api-check` / `desktop-archi-drift` / `desktop-archi-api` × 2 / `desktop-crate-layout` × 2 / `desktop-archi-canvas` × 2 / `desktop-archi-file-tree` / `desktop-ipc-type-bindings` 各 1 MODIFIED；`desktop-archi-agent` 1 ADDED + 1 RENAMED + 3 MODIFIED）；以及工作树源码与既有测试盘点
> **被测范围**: design「变更清单」的**新增文件 10 项 + 修改文件全列（27 行）**。本变更**零删除文件**。两类测试语料（16 份漂移金样与 `api-mismatch/models/01-app.toml` 注释面）已按事实校准 10 移出实现变更清单、登记为 design「测试相位承接面」：语料机械重写归 **test-execution**，`golden_test.rs` 的 `DRIFT_CASES` 登记面与三处硬编码断言归本设计（test-design / test-gen），故本节只登记其期望值、不把它们当实现件设计单测。11 项文件级无映射对（两处 `lib.rs` / 两份 `Cargo.toml` / `bindings/mod_test.rs` 本体 / 生成绑定两份产物 / `package.json` / `AGENT.md` + `CLAUDE.md` / `vocabulary_test.rs` / 四个既有 hook 测试 / 两类测试语料）与 8 条判据面（静态检索 / 命令留痕 / 机械门禁 / 零 diff 审查 / 手工验证）登记于「不可测试项」。
> **契约基准**: 用例一律按 `design.md` 的**定稿契约**设计（D1 渲染单点、D2 比对单点、D3 walker 经 `scan::walk_source_files` + 契约目录有界枚举、D5 投影单点、D6 纳管筛选单点、D7 `findings` 装配单点、D9 `derive_status` 第五入参、D10 写盘单点不删不回滚、D11 CLI 收窄、D13 工具响应形状、D14 工具接入点、D15 hook 三触发、D16 两轴零合并、D17 生成权限与二次确认、D18 五态与 testid 全集（含事实校准 9 的订正：`statusText` 判定序 `root === null` → `error !== null` → loading；检查失败时 `-status` 不渲、不谎称「解析中…」）、D19 抽屉契约段、D20 左栏三视图），不按工作树当前形态回填用例。
> **契约与工作树的已知差异（用例按契约侧写、差异由实现相位补齐）**: 实现相位已把五件新件（`api/{render,diff}.rs` / `infra/archi/src/api_artifacts.rs` / `use-archi-api-check.ts` / `contract-panel.tsx`）与五个测试空壳落地，`main.rs` 已收窄为编排 + `diff_json`（零本地 walker / 零本地写盘），`REPORT_VERSION = 3`，`mount.rs` 静态清单 11 项，`TreeMode` 三值，`commands/archi/mod.rs` 八命令。**尚未对齐的恰一处**是 `contract-panel.tsx` 的 `statusText` 判定序：现实现仍以 `loading || check === null` 渲「解析中…」，与 D18（事实校准 9）要求的 `error !== null` 先短路不一致。本设计的「检查失败」用例组按 D18 写（失败态不渲 `-status`、不谎称「解析中…」）——契约与实现的出入由 test-execution 回灌实现相位；测试相位不因差异删用例或用旧形态回填。

**测试框架与既有约定（盘点结论）**

| 面 | 框架 / 运行器 | 文件形态 | 断言与替身 |
|---|---|---|---|
| Rust（`crates/core/archi`、`crates/infra/archi`、`crates/tools/archi-api-gen`） | `cargo nextest run --workspace`（`server:check` 的 `fmt --check` + `clippy --all-targets` 作编译闭合） | 与被测源文件**同 crate 共置**的 `<模块名>_test.rs`，由源文件尾 `#[cfg(test)] mod <模块名>_test;` 声明；crate 级集成测试在 `tests/`（`golden_test.rs` / `vocabulary_test.rs` / `perf.rs` / `pilot_test.rs`） | `#[test]` + 中文长句用例名 + `assert_eq!` / `assert!` / `matches!` + `expect_err` 文案子串；`core/archi` **无 dev-dependency**（`TempDir` 自建：`std::env::temp_dir()` + `AtomicU64` + `Drop` 清理），`infra/archi` 与 `infra/agent` 用 `tempfile`；源码面判据用 `include_str!` 自证 |
| Rust（`crates/infra/agent`） | 同上 | `sdk/*_test.rs` + `compose_test.rs` + `archi_knowledge_test.rs` | 真实 `archi::dto` / `archi::api` / `tools::cap_result` 组合；tempdir 落模型与源文件；矩阵与清单断言用常量表对拍 |
| Rust（app 壳 `src-tauri`） | 同上（`tauri::test::mock_app()`） | `commands/archi/mod_test.rs`（`#[cfg(test)] mod mod_test;`）、`bindings/mod_test.rs` | 真实 `WorkspaceStores`（tempdir 数据根）；`serde_json::to_value` 逐字段对拍；`snapshot(root) -> Vec<(PathBuf, Vec<u8>)>` 断言零写盘 |
| 前端（`packages/desktop/src`） | `vite-plus` 的 test 面（vitest API，`vp test`；`vp check` 的 `typeCheck` 作编译闭合） | 与被测源文件**同目录共置**的 `<模块名>.test.ts` / `.test.tsx` | `import { describe, expect, it, vi } from 'vite-plus/test'` + `@testing-library/react`（`render` / `renderHook` / `rerender` / `act` / `fireEvent` / `waitFor` / `screen` / `within`）；查询一律 `data-testid` / `data-mode` / `data-axis` / `data-kind` / `data-category` / `data-source` 钩子；`invoke` 用 `vi.hoisted` + `vi.mock('@tauri-apps/api/core')` 作进程边界替身，按命令名分发 fixture |

**用例表列语义**

- **路径类型**：`正向`（契约内输入 / 预期输出）/ `异常`（契约外输入、失败路径、被禁止的写法、反向判据）/ `边界`（空、空串、`null`、`[]`、`{}`、极值、恰好阈值、生命周期交错点、越签名脏值）。
- **迭代类型**：`单次`（一次驱动即判）/ `参数化迭代`（同一断言逻辑对表驱动数据集逐项执行）/ `组合迭代`（两个以上维度交叉，或跨模块链路）。
- **`测试条件` 内的「既有…」前缀**：该臂已被既有用例锁定，本设计**只做回归登记、不新增断言**；未带前缀的为**新增**或**按新形状改写**（改写只动「取哪条断言路径」，语义断言逐条保留）。
- **参数类型边界口径（贯穿各章节，按 Phase 的 Type → Edge Case 映射）**：`root: &Path` / `root: String` 取空 `Path` / `""` / 空白串、不存在路径、指向文件、超长（>1000 chars）、含空格 / 非 ASCII / emoji，各章另加常规形态；`files: &[GeneratedFile]` 取 `[]` / 单元素 / 超大列表（1000 条）；`elements: &[ArchiElement]` / `findings: &[ArchiApiFinding]` 取 `[]` / 单元素 / 超大列表，`findings[].element` 另取 `None`；`enabled` / `generating` 等 `bool` 取 `true` / `false`；三列 / 两列 `Vec<String>` 取 `[]` / 单元素 / 超大列表；`TreeMode`（3 值）、`CanvasMode`（2 值）、`ApiStatus`（3 值）、`category` 六值取**每值 + 非法值**（N+1）；前端 `root: string | null` 另取 `null` 与 `undefined`（越签名脏值）。
- **前端「两轴计数」判据口径**：凡断言计数的行，`data-testid="archi-contract-axis-artifacts"` 与 `-axis-findings` 各自容器内的文本**分别**取值，MUST NOT 以全局面文本计数（跨轴合并总数是 spec 明令禁止形态，故以「分轴取值」为断言手段）。

**待测功能条目口径**

`#### 待测功能` 只列 design「公共函数 / API」表中「所在文件」为本源文件的条目（`api/render.rs` 1 行、`api/diff.rs` 1 行、`api/mod.rs` 3 行、`dto.rs` 1 行、`api_artifacts.rs` 1 行、`commands/archi/mod.rs` 4 行、`use-archi-api-check.ts` 1 行、`contract-panel.tsx` 1 行；`drift/mod.rs` / `drift/report.rs` / `tests/golden_test.rs`（含语料期望登记） / `main.rs` / `commands/mod.rs` / `mount.rs` / `archi_tool.rs` / `tools.rs` / `runner.rs` / `compose.rs` / 两份 md / `file-tree.tsx` / `architecture-canvas.tsx` / `element-drawer.tsx` / 生成绑定在该表**零行**，条目容器以 HTML 注释登记来源并留空）。design 明示的私有实现件（`diff.rs` 的 `generated_files_on_disk` / `api_dir_artifacts` / `is_generated_module_file`、`api_artifacts.rs` 的越根复核与比对内部块、`archi_tool.rs` 的 `api_payload`、`contract-panel.tsx` 的 `ContractSection` 类私有件）**不列本表**，其覆盖经所在文件公共 API 的用例承接——不 grep 源码导出声明、不虚构 API 行。design「类型定义」表的类型不是「公共函数 / API」行，形状覆盖并入对应函数用例的入参 / 出参断言。

**测试布局决策（源文件 → 测试文件的映射依据）**

1. **新建 5 个测试文件**：`api/render_test.rs`、`api/diff_test.rs`、`crates/infra/archi/src/api_artifacts_test.rs`（三件由 `tasks.md` 阶段二 / 四建空壳，本设计给用例本体）、`hooks/use-archi-api-check.test.ts`、`components/contract-panel.test.tsx`（阶段十 / 十一建空壳）——全部沿「同目录共置 + `<模块名>_test` / `<模块名>.test.ts(x)`」定式。
2. **复用并改写 18 个既有测试文件**（另含 `src/lib/ipc-contract.test.ts` 一个核对型映射，预期零手改）：`api/mod_test.rs`（三新出口）、`dto_test.rs`（三新 DTO + `graph_diagnostics_error` 提权）、`drift/mod_test.rs`（版本与纳管）、`drift/report_test.rs`（版本线形）、`tests/golden_test.rs`（16 份金样期望与 `DRIFT_CASES` 登记面）、`tools/archi-api-gen/src/main_test.rs`（纪律翻转）、`commands/archi/mod_test.rs`（两命令）、`src/bindings/mod_test.rs`（清单守卫）、`sdk/mount_test.rs` / `sdk/archi_tool_test.rs` / `sdk/tools_test.rs` / `sdk/runner_test.rs` / `compose_test.rs` / `archi_knowledge_test.rs`（工具面与知识面）、`crates/core/workflow/src/write/phase_table_test.rs`（prompt 矩阵）、`file-tree.test.tsx` / `architecture-canvas.test.tsx` / `element-drawer.test.tsx`（渲染面）。
3. **`commands/mod.rs` 的登记面断言落 `src/bindings/mod_test.rs`**：该文件无独立 sidecar（只有 `all_commands!` 宏体），登记顺序与计数只在该清单守卫可判（AC-1 / AC-17 的验证方法亦点名该文件）。
4. **两类测试语料（16 份 `expected/*.json` + `api-mismatch/models/01-app.toml` 注释面）不是实现变更**（design 事实校准 10 / 「测试相位承接面」）：它们的消费方是 `tests/golden_test.rs` 的 `DRIFT_CASES` 登记面与三处硬编码断言。本设计只把**断言侧改写**（`2` → `3`、`clean` → `warnings`）登记为 `golden_test.rs` 的用例（映射对写作「语料期望 → `golden_test.rs`」，用例形如「登记面改写 + 快照对拍」）；语料机械重写（`DESKTOP_GOLDEN_REWRITE=1` + 逐份 `git diff`）归 **test-execution**，不在本设计产出。
5. **`src-tauri/src/bindings/mod_test.rs` / `main_test.rs` / `element-drawer.test.tsx` 的编译闭合面归实现相位、用例本体归测试相位**（design 事实校准 11）：前者是清单守卫本体（无被测源文件可映射，落「不可测试项」，其内容随动在 `commands/mod.rs` 章节登记）；后两者的用例本体在本设计设计，实现相位只做「让既有用例编译通过」的机械改动（`main_test.rs` 四处调用点改调、`element-drawer.test.tsx` 补必填 prop 与 mock 字段）。
6. **`src/types/generated/bindings.ts` + `bindings.zod.ts` 的核对面落 `src/lib/ipc-contract.test.ts`**：产物本身禁手改（归「不可测试项」），其键集与 schema 表由该文件的全量断言自动覆盖（design「测试文件」表标为「核对」）。
7. **组合用例挂靠按链路发起方**：①「drift 报告 → `apiFindings` → 画布左栏 / 抽屉」的入口是 `architecture-canvas.tsx`；②「左侧契约视图 → 检查取数 → 面板两轴」的入口是 `contract-panel.tsx`（它持有 `useArchiApiCheck`）；③「面板 / 抽屉 → `FileViewer` → `read_file`」的入口分别是面板与抽屉自身；④「模型 → 应生成集 → 三类差异 → 写盘 → 复检」的 Rust 侧链路入口是 `api/mod.rs`（`artifact_check` 与命令层共用）与 `commands/archi/mod.rs`（IPC 链路）。不新建独立集成测试章节或组合测试区。
8. **不 mock 被测件与被测链路内部**：`workspace_render` / `compare` / `write_artifacts` / `api::findings` / `gating_findings` 之间以及与 `dto` / `scan` / `contract` / `check` 全部真实组合；前端 `ContractPanel` / `useArchiApiCheck` / `FileViewer` / `StandardDialog` / `ArchitectureCanvas` / `ElementDrawer` 之间全部真实组合——替身化等于让「判定单源」「写盘单点」自证。

---

## 验收范围

<!-- 测试区域：Rust 面 `packages/desktop/src-tauri/**`（`cargo nextest`，各源文件共置 `_test.rs` + `tests/` 集成测）；前端面 `packages/desktop/src/**`（`vp test`，jsdom，`setupFiles: src/test-setup.ts`）。本变更新建 5 个测试文件、复用并改写 18 个既有测试文件、核对型 `ipc-contract.test.ts` 预期零手改、`tests/vocabulary_test.rs` 与 4 个既有 hook 测试零改动。 -->
<!-- 零 diff / 门禁 / 生成物 / 版本行 / 文档口径不进入 `## 单元测试`，其验收方式逐条列于「不可测试项」。 -->

| AC ID | 验收条件 | 被测文件或模块 |
|---|---|---|
| AC-1 | **八命令可数且语义分明**：archi 段 8 条命令、写命令恰两条（`archi_set_gate` / `archi_api_generate`）、其余六条零写盘；`all_commands!` 登记与 typed 包装齐备 | `packages/desktop/src-tauri/src/commands/archi/mod.rs` → `commands/archi/mod_test.rs`；`packages/desktop/src-tauri/src/commands/mod.rs` → `src/bindings/mod_test.rs` |
| AC-2 | **check 零写盘、generate 只增改不删**：`archi_api_check` 前后盘上字节相同；`archi_api_generate` 后三类差异清空且孤儿仍在盘、`written + unchanged` = 应生成份数 | `commands/archi/mod.rs` → `commands/archi/mod_test.rs`；`packages/desktop/src-tauri/crates/infra/archi/src/api_artifacts.rs` → `api_artifacts_test.rs` |
| AC-3 | **DTO 形状**：三新类型字段序 / 声明序 / camelCase；三列路径串 repo 相对 POSIX，与 `archi::display` 同源零第二份归一 | `packages/desktop/src-tauri/crates/core/archi/src/dto.rs` → `dto_test.rs`；`crates/core/archi/src/api/diff.rs` → `api/diff_test.rs`；`src/bindings/mod_test.rs`；`src/lib/ipc-contract.test.ts`（核对） |
| AC-4 | **比对面上移且判定单源**：`core/archi/src/api/**` 提供 `workspace_render` / `compare` 两纯函数；`main.rs` 零 `fs::write` / `create_dir_all` / 零本地 walker；命令面与 CLI 面调同一函数 | `crates/core/archi/src/api/render.rs` → `api/render_test.rs`；`api/diff.rs` → `api/diff_test.rs`；`crates/tools/archi-api-gen/src/main.rs` → `main_test.rs` |
| AC-5 | **写盘单点驻 `infra/archi`**：`fs::write` / `create_dir_all` 在 `api_artifacts.rs` 恰一处、`starts_with(root)` 越根复核在位、零删除分支；CLI 侧零本地写调用 | `crates/infra/archi/src/api_artifacts.rs` → `api_artifacts_test.rs`；`crates/tools/archi-api-gen/src/main.rs` → `main_test.rs`；`crates/tools/archi-api-gen/Cargo.toml`（「不可测试项」） |
| AC-6 | **`status` 纳管口径**：纳管子集 = 契约文件在案元素的判据；子集非空且 `errors` 空 ⇒ `warnings`；`errors` 非空仍 `errors`；子集空 ⇒ `status` 逐字不变；`passed` 不受影响；筛选单点在 `core/archi` | `crates/core/archi/src/api/mod.rs` → `api/mod_test.rs`（`gating_findings`）；`crates/core/archi/src/drift/mod.rs` → `drift/mod_test.rs` |
| AC-7 | **报告版本与金样**：`REPORT_VERSION = 3`；16 份金样除 `version` 外逐字不变，唯一例外 `api-mismatch.full.json` 的 `status` 由 `clean` 变 `warnings` | `crates/core/archi/src/drift/mod.rs` → `drift/mod_test.rs`；`crates/core/archi/src/drift/report.rs` → `drift/report_test.rs`；`crates/core/archi/tests/golden_test.rs`（design「测试相位承接面」：`DRIFT_CASES` 登记面与三处硬编码断言；语料机械重写归 test-execution） |
| AC-8 | **本仓实测零变化**（登记留痕、非 MUST）：`apiFindings` 仍 41 条、契约在案元素判据 0 条 ⇒ `status` / `passed` 除 `version` 外不变 | 「不可测试项」（命令 + 留痕） |
| AC-9 | **面板两轴并列**：生成物差异三分组 + 判据按 `category` chip，两轴各自计数、零跨轴合计；条目点击一次 `onFocus`；路径行经 `FileViewer` + `read_file` | `packages/desktop/src/views/architecture/components/contract-panel.tsx` → `contract-panel.test.tsx` |
| AC-10 | **状态矩阵五态**：`loading` / `enabled = false`（仍列 `missing`）/ 零差异零判据 / 检查失败行内（`statusText` 判定序订正：失败态 MUST NOT 渲「解析中…」）/ 生成失败顶部回显 | `contract-panel.tsx` → `contract-panel.test.tsx` |
| AC-11 | **生成交互与模式权限**：仅 `incremental` 有生成按钮；二次确认文案含「写入 N / 保留 M 孤儿 / 非原子」；确认后 `archiApiGenerate` 恰一次 → toast → `archiApiCheck` 重跑一次；`full` 零入口 + 提示 | `contract-panel.tsx` → `contract-panel.test.tsx`；`architecture-canvas.tsx` → `architecture-canvas.test.tsx` |
| AC-12 | **取数与成本纪律**：三触发（挂载 / `root` 变化 / 手动刷新）、`root === null` 零 invoke、零 watch 零轮询；切 `contract` 视图不重取三既有命令；`workspace_render` 不随画布刷新自动跑 | `packages/desktop/src/views/architecture/hooks/use-archi-api-check.ts` → `use-archi-api-check.test.ts`；`architecture-canvas.tsx` → `architecture-canvas.test.tsx` |
| AC-13 | **抽屉契约段**：概览与文档之间呈现 `api.status` + 三路径行（`FileViewer`）+ 本元素判据（`element === fqn`）；零判据态一句「本元素无漂移」；零新增 invoke | `packages/desktop/src/views/architecture/components/element-drawer.tsx` → `element-drawer.test.tsx` |
| AC-14 | **分段控件三视图**：`data-mode` 三值 `file` / `element` / `contract`、缺省 `file`、`focus` 共用；文件 / 元素两视图既有断言零变化 | `packages/desktop/src/views/architecture/components/file-tree.tsx` → `file-tree.test.tsx`；`architecture-canvas.tsx` → `architecture-canvas.test.tsx` |
| AC-15 | **`archi_api` 只读工具**：静态清单 11 项、`ToolTier::Read` / `ToolExec::Archi`、`check` / `bootstrap` 行为如约、未知 `action` → `is_error`、输出过 `cap_result`、零写盘；矩阵仅 `(implement, executor)` 增 | `crates/infra/agent/src/sdk/mount.rs` → `sdk/mount_test.rs`；`sdk/archi_tool.rs` → `sdk/archi_tool_test.rs`；`sdk/tools.rs` → `sdk/tools_test.rs`；`sdk/runner.rs` → `sdk/runner_test.rs`；`crates/infra/agent/src/compose.rs` → `compose_test.rs` |
| AC-16 | **知识 / prompt 随动**：`archi-knowledge.md` 含新自检句、零 `<>` 占位符、体量上限内；`implementation-generator.md` 只读工具行含 `archi_api`；其余 prompt 零 diff | `crates/infra/agent/src/archi-knowledge.md` → `archi_knowledge_test.rs`；`crates/core/workflow/src/prompts/implementation-generator.md` → `crates/core/workflow/src/write/phase_table_test.rs` |
| AC-17 | **清单守卫随动**：命令 51 → 53、出线 DTO 101 → 104、`ARCHI_COMMANDS` 6 → 8、`ARCHI_DTO_TYPES` 23 → 26；尾部追加序与对照臂同步 | `src/commands/mod.rs` → `src/bindings/mod_test.rs`；`src/types/generated/bindings.ts` + `bindings.zod.ts` → `src/lib/ipc-contract.test.ts`（核对） |
| AC-18 | **门禁**：`pnpm -C packages/desktop run check` 四段绿、`cargo nextest run --workspace` 全绿、`package.json` 0.6.39 → 0.6.40、`src-tauri/Cargo.toml` crate 版本行不变 | 「不可测试项」（机械门禁 + 版本行） |
| AC-19 | **边界零 diff**：契约格式 / 判据 / 扫描 / 门禁执行面 / 写保护面 / `.dev-team/**` / `explore.md` 全部零 diff；画布渲染面与既有 hooks 零改动 | 「不可测试项」（`git diff --stat` 审查） |

---

## 单元测试

### `packages/desktop/src-tauri/crates/core/archi/src/api/render.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/api/render_test.rs`

**新建测试文件**（`tasks.md` 阶段二建空壳：仅头注释、零用例）——沿 `api/mod_test.rs` 的零 mock 定式：自建 `TempDir` RAII（本 crate 无 dev-dependency）+ `AtomicU64` 唯一后缀，真实落模型 / 契约 md / 源文件，真实 `elements_of` → `load_contracts` → `render_all` 组合（渲染序与 fail-closed 即被测语义，替身会引入第二事实源）。fixture 需三件：`write_model(name, toml)`、`write_contract(fqn, markdown)`（落 `<archi_api_dir>/<Domain>/<FQN>.md`）、`write_file(relative, content)`；语料形态与 `api/mod_test.rs` 既有 `render_all` 用例同源。

#### 待测功能

- `workspace_render(root: &Path) -> Result<Vec<GeneratedFile>, ApiError>`: 应生成物渲染单点——`elements_of` → `load_contracts` → `render_all`；纯读盘、零写盘、零 spawn；结构诊断非空经 `elements_of` 映射为 `ApiError::Element`（fail-closed，坏模型零写盘）

<!-- design.md「公共函数 / API」表对本文件恰 1 行（上列）。`elements_of` 是既有私有件（事实校准 5：不提权、只由子模块 `super::` 调用），不入本表；其行为经 `workspace_render` 的 `Err` 与应生成集观测。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `workspace_render` 四类生成物齐备（AC-4 / D1 / spec「判定单源与写盘单点」） | 正向 | 合成 tempdir：目录型元素根 `App.Core.Archi`（`path = ["./crates/core/archi/"]`）+ 前端元素 `App.Frontend.Welcome`（`.ts` 根）+ 在案契约 md → 返回集含 `<FQN>.schema.json` / `<root>.i.rs` / `<root>.i.ts` / `INDEX.toml` 四类；每份 `GeneratedFile.path` 落在 `root` 之下、`content` 非空 | 单次 |
| `workspace_render` 输出序确定（AC-4 / D4 前提） | 正向 | 同一语料 → 输出序为「契约 FQN 序 → 各元素 `.i.*` → `INDEX.toml` 末位」；两次调用 `Vec` 的 `(path, content)` 序列逐字节相等（幂等，无 HashMap 序泄漏） | 单次 |
| `workspace_render` 结构诊断非空即 `Err`（AC-4 / D1 fail-closed / spec「判定单源与写盘单点」） | 异常 | 模型含①环②孤儿层级③未知引用任一 → `Err(ApiError::Element)`，文案含「模型结构错」与诊断条数；MUST NOT 返回部分生成物集 | 参数化迭代（3 诊断类） |
| `workspace_render` 坏模型零写盘（AC-4 / AC-2 的行为侧证） | 异常 | 结构错模型 → `Err` 前后对 `root` 递归快照字节与 mtime 相等；零临时文件残留（`.tmp` 形态零出现） | 单次 |
| `workspace_render` 空模型的冷启动应生成集（AC-4 / D1 / spec「`enabled` 只作呈现位」的应生成集口径） | 边界 | ①模型目录缺席②`models/` 存在但零 `*.toml` → 均 `Ok`；返回集**恰一份** `INDEX.toml`，零 `.i.rs` / 零 `.i.ts` / 零 `.schema.json` | 参数化迭代（2 形态） |
| `workspace_render` 零契约（有元素、零 md）（AC-4 / D1） | 边界 | 模型非空但 `<archi_api_dir>` 缺席或零 md → `Ok`；零 `.i.*` 与零 `.schema.json`，仅 `INDEX.toml`；不 `Err`（判据缺口归 `check`，归集行为归 `compare`） | 单次 |
| `workspace_render` `root` 字符串与路径形态边界（边界 / `&Path` 映射） | 边界 | `root` 为空 `Path` / 不存在路径 / 指向文件 → 返回值落在 `Result` 内且不 panic（各形态在用例内钉死 `Ok` 或 `Err` 之一）；超长（>1000 chars）/ 含空格 / 非 ASCII / emoji 的 `root` → 不 panic | 参数化迭代（3 非法 + 4 字符串形态） |
| `workspace_render` 超大语料（边界 / list 映射） | 边界 | 合成 60 元素各带契约 md → 返回集规模 = `schema.json` 份数 + `.i.*` 份数 + 1；两次调用逐字节相同；不抛错 | 单次 |
| `workspace_render` 纯读盘零 spawn（AC-4 / 纪律判据） | 异常 | `include_str!("render.rs")` 自证：零 `fs::write` / `create_dir` / `create_dir_all` / `File::create` / `std::process` / `tokio::process` / `WalkBuilder` / `read_dir`；`pub fn ` 计数恰 1；`super::elements_of` / `super::load_contracts` / `super::render_all` 各恰一处（零第二份 collect） | 单次（源码自证） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 文件系统 | **不 mock**：真实 tempdir + 真实落盘模型 / 契约 md / 源文件（渲染面读的就是盘上事实） | 全部用例 |
| `super::elements_of` / `load_contracts` / `render_all` | **不 mock**：真实组合（三者的顺序与 fail-closed 语义即被测对象；替身会让「结构错即 `Err`」自证） | 全部用例 |
| `api::display` / 时钟 / 网络 / 子进程 | **零依赖**：路径归一归 `diff.rs` 章节；本文件不触时钟、不 spawn | —— |

---

### `packages/desktop/src-tauri/crates/core/archi/src/api/diff.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/api/diff_test.rs`

**新建测试文件**（`tasks.md` 阶段二建空壳）——沿 `api/mod_test.rs` 零 mock 定式（自建 `TempDir` + 真实 `ignore` 遍历）。夹具构造器：`write_model` / `write_contract` / `write_file` 之外增 `write_generated(rel, content)`（落 `.i.rs` / `.i.ts` / `.schema.json` / `INDEX.toml` 形态）与 `write_gitignore(lines)`；期望集通过先调一次 `workspace_render` 取「应生成集」再按需破坏（内容改写 / 删除 / 多落孤儿），使三类差异各成一份可判语料。

#### 待测功能

- `compare(root: &Path) -> Result<ArtifactDiff, ApiError>`: 三类差异的比对单点——`workspace_render` 的应生成集 vs 盘上实况；`mismatched` / `missing` 按 `workspace_render` 输出序、`orphaned` 按路径字节序；三列串一律经 `super::display`（repo 相对 POSIX）；零写盘

<!-- design.md「公共函数 / API」表对本文件恰 1 行（上列）。类型 `ArtifactDiff` 在「类型定义」表（进程内类型、无 `specta`），三列形状覆盖并入本节用例；私有件 `generated_files_on_disk` / `api_dir_artifacts` / `is_generated_module_file` 不列本表，其判据经 `compare` 的三列与「源码自证」行承接。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `compare` 内容不符计入 `mismatched`（AC-3 / AC-4 / spec「检查命令契约」） | 正向 | 应生成物在盘但内容改写一个字节 → `mismatched` 恰含该份 repo 相对串；`missing` / `orphaned` 不含它 | 单次 |
| `compare` 应生成未生成计入 `missing`（AC-3 / AC-4） | 正向 | 删掉一份应生成物（如 `.schema.json`）→ `missing` 恰含该串；其余两列不含它 | 单次 |
| `compare` 孤儿生成物计入 `orphaned`（AC-3 / AC-4 / spec「生成从不删文件」的报告侧） | 正向 | 多落一份 `crates/core/archi.i.rs`（命名符合判据、不在应生成集）→ `orphaned` 恰含该串；`mismatched` / `missing` 不含它 | 单次 |
| `compare` 三类差异同场互不混淆（AC-3 / D4） | 组合迭代 | 同一 tempdir 同时造「一份不符 + 一份缺 + 一份孤儿」→ 三列各恰一条且互不交叠；两列（`mismatched` / `missing`）序与 `workspace_render` 输出序一致、`orphaned` 按路径字节序 | 组合迭代（3 差异类 × 2 序判据） |
| `compare` 零差异 → 三列全空（AC-2 / AC-11 的「生成后清空」前提） | 边界 | 按应生成集逐份落盘且内容逐字相同 → `ArtifactDiff { mismatched: [], orphaned: [], missing: [] }`；不 `Err` | 单次 |
| `compare` 盘上无关同名文件不误报（AC-3 / 反向判据） | 异常 | 盘上造 `src/app.ts` / `src/app.tsx` / `src/app.js` / `src/app.schema.json.bak` / `src/dir.i.tsx` / `src/notes.i.js` → 均不入 `orphaned`（命名判据为 `ends_with(".i.rs") \|\| ends_with(".i.ts")`，与落点无关）；对照臂：`src/x.i.ts` 入 `orphaned` | 参数化迭代（6 非命中形态 + 1 命中形态） |
| `compare` 构建 / 依赖产物树与 VCS 目录不扫（AC-4 / D3） | 异常 | 盘上造 `target/debug/x.i.rs` / `node_modules/a/index.i.js` / `dist/y.i.ts` / `build/z.i.ts` / `.git/hooks/h.i.ts` → 五类均不入 `orphaned`；同层正常孤儿 `src/loose.i.rs` 入列 | 参数化迭代（5 剪枝形态） |
| `compare` `.gitignore` 命中项不扫（AC-4 / D3 的显式留痕 / R-1） | 异常 | `<root>/.gitignore` 写 `generated/**`，盘上 `generated/old.i.rs` → 不入 `orphaned`；对照臂：删掉该 `.gitignore` 行后同一文件入 `orphaned`（遍历经 `scan::walk_source_files` 的 gitignore 感知即被测语义） | 参数化迭代（2 形态对拍） |
| `compare` 契约目录的 schema 与 `INDEX.toml` 孤儿（AC-4 / D3 有界枚举） | 正向 | `<archi_api_dir>` 内多落一份 `App/Ghost.schema.json` 与旧 `INDEX.toml`（内容不符）→ 前者入 `orphaned`、后者入 `mismatched`；契约目录**外**同名的 `App/Ghost.schema.json`（如 `src/App/Ghost.schema.json`）不入 `orphaned`（schema 判据只在契约目录内成立） | 单次 |
| `compare` 契约目录有界枚举的深度上限（AC-4 / D3 / 零新常量） | 边界 | 契约目录内造 `a/b/c/d/e/f/Ghost.schema.json`（超过 `MAX_API_WALK_DEPTH`）→ 不入 `orphaned`、不 panic；同目录浅层 `Ghost2.schema.json` 照常入列 | 单次 |
| `compare` 空仓库 / 契约面未启用（AC-3 / 边界） | 边界 | ①模型缺席②零 `*.toml`③有元素但 api 目录缺席 → 均 `Ok`；`missing` 恰为 `INDEX.toml` 一条、另两列空；`enabled = false` 时**照常**给出该条（不因未启用而空列） | 参数化迭代（3 形态） |
| `compare` 路径串与 `archi::display` 同源（AC-3 / D2 / spec「DTO 形状」） | 正向 | 三列每个串满足：无 `\`、无前导 `./`、无前导 `/`、不含 `root` 前缀、分隔符恒 `/`；含中文 / 空格 / emoji 的目录名同样归一；反向判据：`include_str!("diff.rs")` 零本地归一（零 `replace("\\\\", "/")` / 零 `strip_prefix`） | 组合迭代（4 形态 × 三列） |
| `compare` 零写盘（AC-2 / AC-4） | 边界 | 调用前后对 `root` 递归快照（字节 + mtime）相等；盘上文件数不变（不删孤儿、不写缺口）；`include_str!("diff.rs")` 零 `fs::write` / `create_dir` / `File::create` | 单次 |
| `compare` 结构错模型即 `Err`（AC-4 / D1 / D2） | 异常 | 模型含环 → `Err`（`workspace_render` 同口径，命令面不返回部分差异）；前后零写盘 | 单次 |
| `compare` 遍历面零第二份实现（AC-4 / D3） | 异常 | `include_str!("diff.rs")` 自证：`walk_source_files` 恰一处；`WalkBuilder` 零命中；`read_dir` 只出现在 `api_dir_artifacts`（1 处）；`MAX_API_WALK_DEPTH` 恰一处（复用零新常量）；`SKIPPED_DIRECTORY` / `MAX_MODULE_WALK_DEPTH` 零命中；`pub fn ` 计数恰 1 | 单次（源码自证） |
| `compare` `root` 非法与超大孤儿列表（边界） | 边界 | `root` 为空 `Path` / 不存在 / 指向文件 → 落 `Result` 内不 panic；盘上 1000 份孤儿 → `orphaned.len() == 1000`、序为路径字节序、不抛错 | 参数化迭代（3 非法 + 1 超大） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 文件系统 | **不 mock**：真实 tempdir + 真实落盘（三类差异即盘上事实的投影） | 全部用例 |
| `ignore`（经 `scan::walk_source_files`） | **不 mock**：真实遍历器（剪枝与 `.gitignore` 感知即被测语义；`scan/exclude_test.rs` 已锁其行为，本节只经 `compare` 观测） | 剪枝 / gitignore 两行 |
| `super::workspace_render` / `super::display` / `super::paths` | **不 mock**：真实调用（应生成集与归一单点是判定单源的两个支点） | 全部用例 |
| 时钟 / 网络 / 子进程 | **零依赖** | —— |

---

### `packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/api/mod_test.rs`

**改写既有测试文件**（本变更新增三出口：`artifact_check` / `gating_findings` / `findings`）——沿既有零 mock 定式（自建 `TempDir` + 真实 `paths` / `contract` / `check` / `diff` / `render` 组合）。既有 `enabled` / `meta` / `render_all` / `check` / `bootstrap` / `api_ref` 用例**逐字保留**（回归登记行）。

#### 待测功能

- `artifact_check(root: &Path) -> Result<ArchiApiCheck, ApiError>`: `compare` + `enabled` + `meta()` 的 DTO 投影**单点**（命令层零投影）；`common_digest` 出全量 `sha256:<64hex>`
- `gating_findings(root: &Path, elements: &[ArchiElement], findings: &[ArchiApiFinding]) -> Vec<ArchiApiFinding>`: 纳管子集筛选单点（`api == Declared` 且契约文件在案；`element = None` 落选）；`drift` 侧零 `is_file`
- `findings(root: &Path, elements: &[ArchiElement]) -> Result<Vec<ArchiApiFinding>, ApiError>`: 契约判据装配单点（`enabled` 短路 → `load_contracts` → `check`）；`drift` 与 `archi_api` 工具共用

<!-- design.md「公共函数 / API」表对本文件恰 3 行（上列）+ 事实校准 8 的 `dto::graph_diagnostics_error` 提权（归 dto.rs 章节）。两子模块声明 `pub mod diff;` / `pub mod render;` 与头注释订正不是 API 行（其行为归各自章节）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `artifact_check` 三列与 `compare` 逐列相等（AC-3 / D5） | 正向 | 一份「不符 + 缺 + 孤儿」语料 → `artifact_check().mismatched / orphaned / missing` 与 `diff::compare` 的三列 `assert_eq!` 相等；`enabled == api::enabled(root)` | 单次 |
| `artifact_check` 的 `meta` 投影（AC-3 / D5 / spec「DTO 形状」） | 正向 | `toolVersion == env!("CARGO_PKG_VERSION")`；`commonDigest` 形如 `sha256:` + 64 位小写 hex（**全量不截断**）；与 `meta().common_digest` 逐字相等；`schemaUri` / `generatedNotice` / `common` 三件**不出线**（JSON 键集恰 `["enabled","meta","mismatched","orphaned","missing"]` 与 `["toolVersion","commonDigest"]`） | 单次 |
| `artifact_check` 未启用契约面仍 `Ok`（AC-10 的数据侧 / spec「`enabled` 只作呈现位」） | 边界 | 无 `<archi_api_dir>` / 目录存在但零 md → `Ok`；`enabled == false` 且 `missing` 恰 `INDEX.toml` 一条；MUST NOT `Err`、MUST NOT 三列全空（空面板的成因不得来自数据面） | 参数化迭代（2 形态） |
| `artifact_check` 结构错模型即 `Err`（AC-3 / D5） | 异常 | 模型含环 → `Err`，无 DTO 产出（`expect_err` 后无三列可读）；前后零写盘 | 单次 |
| `artifact_check` 零差异的成功态（AC-10 / 边界） | 边界 | 应生成集逐份落盘且一致 → 三列全空且 `enabled == true`；`Ok`（「生成物最新」态的判据来源） | 单次 |
| `gating_findings` 契约在案的 `declared` 元素判据入选（AC-6 / D6） | 正向 | 元素 `api = "declared"` 且 `<archi_api_dir>/<Domain>/<FQN>.md` 在盘；传入两条判据（`element = Some(该 FQN)`）→ 两条均入选；返回序与入参序一致（零重排） | 单次 |
| `gating_findings` 无契约文件落选（AC-6 / D6） | 异常 | 元素 `api = "declared"` 但契约 md 缺席；其判据 → 落选（返回空）；对照臂：补上 md 后同一条判据入选 | 参数化迭代（2 形态对拍） |
| `gating_findings` 非 `declared` 元素即使契约在案也落选（AC-6 / D6 的闭合面） | 异常 | `api = "none"` 与 `api = "undeclared"` 元素各带 1 条判据、且人为落一份同名契约 md → 两条均落选（口径 = 契约即承诺，只筛 `Declared`） | 参数化迭代（2 词值） |
| `gating_findings` `element = None` 落选（AC-6 / D6 / spec「纳管子集定义」） | 异常 | 传入 `element = None` 的判据（如 `apiMissing` 无归属形态）→ 落选；同一调用里带归属且契约在案的另一条入选（同场不互相带过） | 单次 |
| `gating_findings` 空列表与零元素（边界 / list 映射） | 边界 | ①`findings = []`②`elements = []`③契约面未启用（`<archi_api_dir>` 缺席）→ 三种均返回 `[]` 且不 panic、零 IO 报错 | 参数化迭代（3 形态） |
| `gating_findings` 超大列表与去重稳定（边界） | 边界 | 1000 条判据（半数命中）→ 命中条数与序稳定；连调两次结果逐条相等（`BTreeSet` 集合判据零泄漏） | 单次 |
| `findings` 正常装配产出十条判据结果（AC-6 / D7） | 正向 | 契约与代码签名不符的语料 → 返回非空、`category` / `kind` / `occurrences` 与 `check(ApiCheckInput)` 直出逐字段相等；与 `drift::check` 报告的 `api_findings` 逐字段相等（同一函数） | 单次 |
| `findings` 未启用短路（AC-6 / D7） | 边界 | `<archi_api_dir>` 缺席 / 零 md → `Ok(vec![])`；`include_str!("mod.rs")` 自证 `if !enabled(root)` 早退分支在契约装载之前 | 参数化迭代（2 形态） |
| `findings` 契约非法即 `Err`（AC-6 / D7 错误映射） | 异常 | 非法 JSON 块 / `$defs` 冲突 / 非法 UTF-8 字节任一 → `Err(ApiError)`；`drift` 侧映射为 `CheckError::Config`（错误文案逐字等价，见 `drift` 章节） | 参数化迭代（3 非法形态） |
| `findings` 装配单点（AC-6 / D7） | 异常 | `include_str!("mod.rs")` 自证：`pub fn findings` 恰一处定义；`load_contracts` 出现在 `findings` 与既有 `check` / `render_all` 之内、`drift/mod.rs` **零**内联装配（归 `drift` 章节）；`is_file` 只出现在 `gating_findings`（1 处） | 单次（源码自证） |
| 既有出口零变化（AC-19 / 回归登记） | 边界 | `enabled` / `meta` / `render_all` / `check` / `bootstrap` / `api_ref_of` 的既有断言**逐条保持绿、零改动**；`pub mod diff;` 置 `contract` 与 `emit` 之间、`pub mod render;` 置 `paths` 与 `schema` 之间（字母序） | 单次（回归登记） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 文件系统 | **不 mock**：真实 tempdir（契约 md / 模型 / 源文件 / 应生成物） | 全部用例 |
| `diff::compare` / `contract` / `check` / `paths` / `render` | **不 mock**：真实组合（投影单点与筛选单点的语义即「不加工」；替身会让逐字段相等自证） | 全部用例 |
| 时钟 / 网络 / 子进程 | **零依赖** | —— |

---

### `packages/desktop/src-tauri/crates/core/archi/src/dto.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/dto_test.rs`

**改写既有测试文件**——只**追加**三新类型（`ArchiApiCheck` / `ArchiApiMeta` / `ArchiApiWriteOutcome`）的字段序 / 线形 / 边界断言与 `graph_diagnostics_error` 提权后的行为断言；既有九组封闭词汇与全部出线 DTO 的断言**零改**。沿既有零 mock 定式（真实 `serde_json` 序列化 / 反序列化）。

#### 待测功能

- `graph_diagnostics_error(diagnostics: &[ArchiDiagnostic]) -> String`: 结构诊断文案单点（`pub(crate)` → `pub`，事实校准 8；agent 工具复用，零第二份 `file:line` 拼装）

<!-- design.md「公共函数 / API」表对本文件恰 1 行（上列）。三新 DTO 列在「类型定义」表而非 API 表，其形状 / 线形 / 边界覆盖由本节用例表承载（不虚构 API 条目）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ArchiApiCheck` 五字段声明序出线（AC-3 / spec「DTO 形状」） | 正向 | 全字段实例 `serde_json::to_value` → 键序列恰 `["enabled","meta","mismatched","orphaned","missing"]`；反序列化往返逐字段相等；`specta` 类型名 `ArchiApiCheck` 与 `bindings/mod_test.rs` 的 `ARCHI_DTO_TYPES` 登记一致（本行只作形状侧等价登记） | 单次 |
| `ArchiApiMeta` 二字段（AC-3） | 正向 | 键序列恰 `["toolVersion","commonDigest"]`；`toolVersion` 为版本串形态、`commonDigest` 为 `sha256:<hex>` 形态（DTO 不截断，长度 ≥ 71）；往返相等 | 单次 |
| `ArchiApiWriteOutcome` 二字段（AC-3） | 正向 | 键序列恰 `["written","unchanged"]`；两列不交叠（同串不同时出现）；往返相等 | 单次 |
| 三类型的字段值边界（边界 / str 与 list 映射） | 边界 | `enabled` 取 `true` / `false`；`toolVersion` / `commonDigest` 取 `""`（越签名脏值，序列化不 panic）/ 超长 >1000 chars / 含 `\n\0` 与 emoji；三列与两列取 `[]` / 单元素 / 1000 条 / **非字典序输入（DTO 零重排）**；往返后逐字段相等 | 组合迭代（`enabled` 2 × 列容器 4 × 字符串 4 抽样） |
| 三类型词汇外 / 缺字段反序列化（异常 / 对象映射） | 异常 | `ArchiApiCheck` 缺 `meta` 键 → `Err`；`meta` 为 `null` → `Err`；三列之一为 `null` → `Err`；多余字段 `{... , "extra": 1}` → 按既有 DTO 口径（结构体无 `deny_unknown_fields` 时忽略，用例内钉死实际行为并逐字注释）；`enabled` 传字符串 → `Err` | 参数化迭代（5 形态） |
| `graph_diagnostics_error` 文案形态（AC-15 的错误面 / 事实校准 8） | 正向 | `[]` → 固定文案（零「结构错」字样时也非空、无 `file:line` 拼接）；单诊断 → 文案含诊断条数「1」与 `kind` / 位置；三条诊断 → 条数「3」；两处调用方（`dto::graph_view` 的 `api` 与 agent 工具）产出**逐字相同**（零第二份拼装） | 参数化迭代（0 / 1 / 3 诊断） |
| `graph_diagnostics_error` 提权后的可达性与纯度（边界） | 边界 | 从 crate 外（`archi::dto::graph_diagnostics_error`）可调用（编译期 + 一次直调）；纯函数：同入参两次调用逐字相同、零 IO 副作用（调用前后 tempdir 快照不变）；诊断含超长 `message`（>1000 chars）/ 含 `\n` 与 emoji → 不 panic | 参数化迭代（3 形态） |
| 既有九组封闭词汇与既有出线 DTO 零变化（AC-19 / 回归登记） | 边界 | 九组词汇的词集 / 词序 / `ALL` 长度逐字不变；`ArchiGraph` / `ArchiElement` / `ArchiRelation` / `ArchiDiagnostic` / `ArchiModuleDetail` / `ArchiApiRef` / `ArchiChangeSummary` / `ArchiSearchResult` / `ArchiFileTree` 的键序列逐字不变（新类型为**并列追加**）；三新类型不进四面词汇对拍（`tests/vocabulary_test.rs` 零 diff，见「不可测试项」） | 单次（回归登记 + 反向判据） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| serde / serde_json | **不 mock**：真实序列化 / 反序列化（声明序与线形即被测对象） | 全部用例 |
| `specta::Type` | **不 mock**：derive 是编译期事实；产物面由 `src/bindings/mod_test.rs` 与 `src/lib/ipc-contract.test.ts` 章节断言 | 出线形状行 |
| 时钟 / 网络 / 子进程 / 文件系统 | **零依赖**（全内存构造，零 IO） | —— |

---

### `packages/desktop/src-tauri/crates/core/archi/src/drift/mod.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/drift/mod_test.rs`

**改写既有测试文件**——`REPORT_VERSION` 2 → 3、契约判据装配改调 `api::findings`、新增纳管子集入参与 `warnings` 分支的或条件。装置沿既有定式（自建 `TempDir` + 真实模型 / 源文件 / 契约 md / `config.json`）。三处既有断言**必须改写**（口径随纳管变化）：`契约面缺席时_api_findings为空` 的 `version == 2`、`试点元素零命中且不改status` 的「契约面不参与推导」表述、`契约不一致产出非空api_findings` 的 `status` 仍 `clean` 断言。

#### 待测功能

<!-- design.md「公共函数 / API」表对本文件零行：`REPORT_VERSION` 是常量、`derive_status` 与 `check` 是该 crate 的公共面但未列 API 表（本变更只改其入参与分支条件）。条目改列 design「修改文件」表 drift/mod.rs 行与 D7 / D8 / D9；断言语义按契约侧写明。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `REPORT_VERSION == 3` 与报告 `version` 字段（AC-7 / D8） | 正向 | `run_full` 产出的 `report.version == 3`；`skipped` 早退臂（无模型）同样 `version == 3`；`include_str!("mod.rs")` 自证 `REPORT_VERSION: u32 = 3` 恰一处、零 `= 2` | 参数化迭代（2 模式 + 源码自证） |
| 纳管子集非空且无 `errors` → `warnings`（AC-6 / D9 / spec「契约在案判据把 status 拉到 warnings」） | 正向 | 契约在案的 `declared` 元素 + 签名不符契约（工具方法返回位声明为 `integer`）→ `api_findings` 非空、`status == warnings`；`errors` 空、`warnings` 空 | 单次 |
| `errors` 非空时仍 `errors`（AC-6 / D9 先后序不变） | 异常 | 同场造未声明依赖 `errors` + 契约在案判据 → `status == errors`（纳管不得抢 `errors` 档）；`errors` 槽内容与非纳管语料逐字相同 | 单次 |
| 纳管子集空 ⇒ `status` 与改动前逐字相同（AC-6 / D9 / AC-8 的口径侧） | 边界 | ①`clean` 语料②`noChanges`（零可判文件）③`skipped`（无模型）④`warnings`（既有 warnings 非空、契约面缺席）→ 四值逐条与改动前一致；`api_findings` 空 | 参数化迭代（4 语料形态） |
| 契约不在案 + 判据存在 ⇒ 不改 `status`（AC-6 / D6 的对照臂） | 异常 | 元素 `api = declared` 但契约 md 缺席、代码导出缺口（`apiEntryMissing`）→ `api_findings` 非空、纳管子集空 ⇒ `status == clean`（与无契约面时相同）；补上契约 md 后同语料转 `warnings`（自激活，无白名单） | 参数化迭代（2 形态对拍） |
| 契约在案但判据 0 条 ⇒ `status` 不变（AC-6 / AC-8 的形态侧） | 边界 | 契约与代码签名一致（十判据零命中）→ `api_findings` 空、纳管子集空 ⇒ `status == clean`；对照臂：删掉契约目录 → 同 `status`（改写既有 `试点元素零命中且不改status` 的对照表述） | 单次 |
| `passed` 不因判据变假（AC-6 / spec「`passed` 本轮不动」） | 正向 | `status == warnings` 且 `errors` / `structural_errors` 双空 → 门禁派生面（`errors.is_empty() && structural_errors.is_empty()`）判 `passed == true`；对照臂：造一条 `errors` → `passed == false` 且 `status == errors`（恒等式 `status == errors ⟺ passed == false` 不破） | 参数化迭代（2 形态对拍） |
| `warnings` 与纳管并存不叠加升级（AC-6 / D9） | 边界 | 既有 `warnings` 非空（如 unfulfilled 声明）+ 纳管子集非空 → 仍 `warnings`（或条件不产出新档）；`warnings` 槽内容零增删 | 单次 |
| 判据装配改走 `api::findings`（AC-6 / D7） | 异常 | `include_str!("mod.rs")` 自证：`api::findings` 恰一处、`api::gating_findings` 恰一处；零 `load_contracts` 内联调用（契约装载只在 `core/archi::api`）；`is_file` 零命中（AC-6 的机械判据）；零「首期 MUST NOT」字样 | 单次（源码自证） |
| 三处既有断言按纳管口径改写（AC-7 / R-7） | 边界 | `契约面缺席时_api_findings为空` 的 `version` 断言 2 → 3；`试点元素零命中且不改status` 的对照表述改「契约面判据 0 条 ⇒ 不改 status」（断言体不变）；`契约不一致产出非空api_findings` 末段 `status` 断言 `clean` → `warnings` 并补「`passed` 不受影响」；其余 20+ 条既有用例**逐字保留** | 单次（改写登记） |
| `skipped` 早退臂的判据槽（AC-7 / 边界） | 边界 | 无模型 → `status == skipped`、`api_findings == []`、`version == 3`；零 `is_file` 调用（早退不触契约面） | 单次 |
| 报告其余 14 字段与 `occurrences` 聚合零变化（AC-19 / 回归登记） | 边界 | `observations` / `suppressed` / `modelHash` / `scan` / `model` / `errors` / `warnings` 的既有断言**全绿零改动**；`api_findings` 的 `occurrences` 逐项形态不变 | 单次（回归登记） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 文件系统 | **不 mock**：真实 tempdir 落模型 / 源文件 / 契约 md / `config.json` | 全部用例 |
| `dto::graph_view` / `scan` / `api::findings` / `api::gating_findings` / `api::check` | **不 mock**：真实组合（纳管链路的入参来源即被测语义；替身会让「契约在案才纳管」自证） | 全部用例 |
| 时钟 / 摘要 | **不 mock**：断言形态与变化性（沿既有用例口径） | 版本 / 报告装配行 |
| 网络 / 子进程 | **零依赖** | —— |

---

### `packages/desktop/src-tauri/crates/core/archi/src/drift/report.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/drift/report_test.rs`

**改写既有测试文件**——字段 / 类型 / 序号**零改动**（14 字段 + `api_findings` 并列槽位），只随 `REPORT_VERSION` 3 与 `status` 五值语义扩一条口径做线形回归。沿既有零 mock 定式（真实 `serde_json`，零 IO）。

#### 待测功能

<!-- design.md「公共函数 / API」表对本文件零行：本变更只改 `api_findings` 字段 doc 的纳管口径表述，字段定义、排序与类型零改动（D8）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 完整报告 JSON 往返与 15 键线形（AC-7 / AC-19） | 正向 | `ArchiDriftReport` 全字段构造 → 序列化键序列与改动前逐字相同（`version` + 14 字段，`api_findings` 并列槽位形态不变）；反序列化往返逐字段相等 | 单次 |
| `version = 3` 的线形（AC-7 / D8） | 正向 | `version == 3` 的报告往返后仍 3；`version` 为 `number` 类型（非字符串）；旧值 `2` 反序列化仍可读（字段零改 ⇒ 向前兼容形态不被破坏） | 参数化迭代（3 / 2 / 1 三值） |
| `api_findings` 槽位形状（AC-6 / AC-7） | 正向 | 数组元素六字段（`kind` / `category` / `element` / `target` / `message` / `occurrences`）camelCase 出线；`element` / `target` 可为 `null`；空数组形态恰 `[]`（非 `null`） | 参数化迭代（空 / 单条 / 多条） |
| `status` 五值线形零变化（AC-6 / 回归登记） | 边界 | `ArchiCheckStatus::ALL` 长度 5、`as_str` 逐值不变（`skipped` / `errors` / `noChanges` / `warnings` / `clean`）；词汇外反序列化仍 `Err` | 参数化迭代（5 值 + 1 非法值） |
| 字段值边界（边界 / dict 映射） | 边界 | ①`api_findings` 缺键 → 反序列化 `Err`（不补默认值，design 明写「MUST NOT 补默认值」）②多余字段 → 按既有口径（钉死实际行为）③`occurrences` 元素含空 `specifier` / `null` → 往返相等；`timestamp` / `modelHash` 空串不 panic | 参数化迭代（3 形态） |
| 未来版本号与词汇外 `status`（异常 / 消费方按 `version` 分支） | 异常 | `version = 4` 的报告反序列化（按既有口径钉死：字段零改故可读，但消费方 MUST NOT 补默认值——用例内断言实际行为）；`version = 0` / `version` 为字符串 → 行为钉死；`status = "warning"`（词外单数形态）→ `Err`（零宽松归一） | 参数化迭代（4 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| serde / serde_json | **不 mock**：真实序列化 / 反序列化（线形即被测对象） | 全部用例 |
| 时钟 / 文件系统 / 网络 / 子进程 | **零依赖**（全内存构造） | —— |

---

### `packages/desktop/src-tauri/crates/core/archi/tests/corpus/drift/**` -> `packages/desktop/src-tauri/crates/core/archi/tests/golden_test.rs`

**测试相位承接面**（design「测试相位承接面」第 1 / 2 行；事实校准 10）——16 份 `expected/*.json` 与 `api-mismatch/models/01-app.toml` 的注释面是**语料期望值资产**（非实现件、非本设计的产出），消费方是 `golden_test.rs` 的 `DRIFT_CASES` 登记面与三处硬编码断言。本设计只**登记期望与断言侧改写**（`2` → `3`、`clean` → `warnings`，断言语义逐条保留）；机械重写（`DESKTOP_GOLDEN_REWRITE=1`）与 `git diff` 逐份确认由桌面端 **test-execution** 承接，实现相位对 `tests/**` 零读写。装置沿既有定式（真实 `copy_tree` 到 tempdir + 真实 `drift::check` 双模式驱动）。

#### 待测功能

<!-- design.md「公共函数 / API」表对本文件零行（语料 JSON 与模型注释不是 API 行）。本节登记 design「测试相位承接面」前两行的期望值 + D8 / AC-7，断言对象是 golden_test.rs 的 `DRIFT_CASES` 登记面与三处硬编码断言；语料本体重写归 test-execution，不进本设计的被测文件清单。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `DRIFT_CASES` 的 `api-mismatch` full 行 `status` 改写（AC-7 / spec「契约在案判据把 status 拉到 warnings」） | 正向 | 登记面该行 `status: "clean"` → `"warnings"`；`DriftExpectation` 的字段说明注释由「契约面不进 `status` 推导」改为纳管口径（与 design D8 一致，零测试内第二份口径）；重写轮断言（`report.status.as_str() == expectation.status`）随之通过；其余 15 行 `status` 逐字不变 | 单次 |
| 16 份快照 `version` 2 → 3（AC-7 / D8） | 正向 | 16 份 `expected/*.json` 的 `version` 均 3；除 `version` 外 15 份与改动前**逐字相同**（快照对拍）；`api-mismatch.full.json` 另有 `status` 一处差异 | 参数化迭代（16 份） |
| 三处硬编码版本断言改写（AC-7 / R-7） | 边界 | `判定语料快照均含apiFindings键且版本为二` 的 `json!(2)` → `json!(3)` 且用例名改「版本为三」；`既有金样重写面键集一致` 的 `version == 2` → `3`（键集断言体零改）；`非空apiFindings金样在案` 的 `status` 断言 `"clean"` → `"warnings"` 且注释口径改纳管 | 单次（改写登记） |
| `api-mismatch` 判据条数与 `category` 不变（AC-7 / 边界） | 边界 | 登记面 `api_findings == 1`；快照内恰 1 条 `apiSignatureMismatch`、`category == "signature"`、`occurrences` 非空——**纳管只改 `status`，不改判据条数 / kind / 顺序** | 单次 |
| `api-mismatch/models/01-app.toml` 注释与快照自洽（AC-7 / R-2） | 异常 | 文件头注释口径为「契约在案 ⇒ 判据纳管 ⇒ `status = warnings`；`passed` 不受影响」，与金样 `status` 无自相矛盾；模型元素 / 关系 / 元数据条目**逐字不变**（`git diff` 只含注释行） | 单次 |
| 重写闸只读语义不变（AC-7 / 回归登记） | 异常 | 未置位 `DESKTOP_GOLDEN_REWRITE=1` 时快照字节零变化（既有断言零改动）；置位时仍按登记面断言（快照不参与判定）；golden 缺失 / 非法 JSON 的两条文案断言不变 | 单次（回归登记） |
| 语料目录与登记面互为镜像、case 数 14（AC-7 / 回归登记） | 边界 | 既有 `判定语料目录与登记面互为镜像` 与 `判定语料 14 个 case` 断言零改动（本变更零新增 / 零删除 case 目录） | 单次（回归登记） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 语料盘面 | **不 mock**：真实 `copy_tree` 到 tempdir 后驱动真实 `drift::check`（语料即期望值本体） | 全部用例 |
| `DESKTOP_GOLDEN_REWRITE` | **不 mock**：取真实环境变量（重写闸语义即被测对象）；本设计不发起重写 | 重写闸行 |
| 时钟 / 网络 / 子进程 | **零依赖** | —— |

---

### `packages/desktop/src-tauri/crates/infra/archi/src/api_artifacts.rs` -> `packages/desktop/src-tauri/crates/infra/archi/src/api_artifacts_test.rs`

**新建测试文件**（`tasks.md` 阶段四建空壳）——沿 `crates/infra/archi/src/report_test.rs` 定式：真实 `tempfile::TempDir`（本 crate dev-dep 已声明）+ 真实 `archi::display`；写盘失败以「父路径被普通文件占位」注入（不改权限、不依赖平台）。夹具：`file(path, content) -> GeneratedFile`。

#### 待测功能

- `write_artifacts(root: &Path, files: &[GeneratedFile]) -> Result<ArchiApiWriteOutcome, String>`: 逐份「`starts_with(root)` 越根复核 → 读盘比对 → `create_dir_all` + `fs::write`」；字节相同计入 `unchanged` 且不重写（mtime 不动）；**从不删文件**、单份失败即 `Err` 且**不回滚**已写部分；两列串经 `archi::display`

<!-- design.md「公共函数 / API」表对本文件恰 1 行（上列）。`ArchiApiWriteOutcome` 在「类型定义」表，两列形状覆盖并入本节用例。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `write_artifacts` 首轮全 `written`（AC-2 / AC-5 / spec「生成命令契约」） | 正向 | tempdir 空盘 + 三份应生成物（含跨层新目录）→ `Ok`；`written.len() == 3` 且逐串为 repo 相对；`unchanged` 空；三份内容与入参逐字节相同 | 单次 |
| `write_artifacts` 次轮全 `unchanged` 且不重写（AC-2 / D10 / spec「幂等且不动 mtime」） | 正向 | 同一批再调一次 → `unchanged.len() == 3` / `written` 空；三份文件 mtime **不变**（先落盘、`sleep` 后重调，比对 `modified()`）；内容零变化 | 单次 |
| `write_artifacts` 内容变化只重写变化份（AC-2 / spec「只增改」） | 正向 | 三份已一致中的一份入参内容改一字 → `written` 恰该一份、`unchanged` 含另两份；盘上该份内容更新、mtime 变化；另两份 mtime 不动 | 单次 |
| `write_artifacts` 父目录自动创建（AC-2 / D10） | 正向 | 目标 `<root>/a/b/c/x.schema.json`（`a/b/c` 全不存在）→ `Ok`、文件在盘、内容正确、`written` 含该串 | 单次 |
| `write_artifacts` 越根目标即 `Err` 且该份零写盘（AC-5 / D10 / spec「越根复核」） | 异常 | 入参含 `<tempdir 外>/evil.i.ts`（`starts_with(root)` 为假）→ `Err`，文案含「越出观察根」与两侧路径；该目标未创建、父目录未被建；同批合规份按序处理（用例内钉死「越根在前即整批未写」与「越根在后即前序已写」两种次序的实际行为并逐字注释——不声称全或无） | 参数化迭代（2 次序） |
| `write_artifacts` 单份失败不回滚（AC-5 / D10 / spec「非原子如实」） | 异常 | 目标父路径被**普通文件**占位（`<root>/a` 是文件，目标是 `<root>/a/b/x.i.rs`）→ `Err`（`create_dir_all` 失败），文案不含「全或无」字样；前序已写的份**保留在盘**（快照可见） | 单次 |
| `write_artifacts` 从不删文件（AC-2 / AC-5 / 硬线二） | 异常 | 盘上预置孤儿 `crates/core/archi.i.rs`（不在 `files` 内）→ 调用后该文件字节与 mtime 均不变、仍在盘；`include_str!("api_artifacts.rs")` 零 `remove_file` / `remove_dir` / `remove_dir_all`（零删除分支） | 单次（行为 + 源码自证） |
| `write_artifacts` 两列路径串 repo 相对 POSIX（AC-3 / AC-5） | 正向 | `root` 分别取绝对路径与相对路径（`set_current_dir` 后构造）→ 两列串均无 `\`、无前导 `./`、无前导 `/`、不含 `root` 前缀；含中文 / 空格 / emoji 目录名同样归一；与 `archi::display(root, &path)` 逐字相等 | 参数化迭代（2 root 形态 × 4 路径名形态） |
| `write_artifacts` 空列表（边界 / list 映射） | 边界 | `files = []` → `Ok(ArchiApiWriteOutcome { written: [], unchanged: [] })`；零目录创建（快照目录集合不变）、零写盘 | 单次 |
| `write_artifacts` `root` 非法与超大列表（边界） | 边界 | `root` 为空 `Path` / 不存在目录 / 指向文件 → 分别落 `Ok` 或 `Err`（用例内钉死），不 panic；1000 份应生成物 → `written.len() == 1000`、不抛错、两列不交叠 | 参数化迭代（3 非法 root + 1 超大列表） |
| `write_artifacts` 写盘实现单点（AC-5 / 纪律判据） | 异常 | `include_str!("api_artifacts.rs")` 自证：`fs::write` 恰 1、`create_dir_all` 恰 1、`starts_with(root)` 恰 1、`remove_*` 零命中；`pub fn ` 计数恰 1；零 `WalkBuilder` / 零 `std::process` / 零 `tokio::process`（dumb IO，零判定零渲染） | 单次（源码自证） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 文件系统 | **不 mock**：真实 `tempfile::TempDir` + 真实 `fs` 读写（写盘语义即被测对象）；失败面用「父路径文件占位」注入 | 全部用例 |
| `archi::display` / `archi::GeneratedFile` / `ArchiApiWriteOutcome` | **不 mock**：真实调用 / 真实类型（路径归一与线形是契约的一部分） | 全部用例 |
| 时钟（mtime 判据） | **不 mock**：真实 `std::fs::metadata().modified()`；两轮之间以极短 `sleep` 拉开可辨区间（沿 `report_test.rs` 时钟口径，不引入假时钟） | 幂等 / 只重写变化份两行 |
| 网络 / 子进程 | **零依赖** | —— |

---

### `packages/desktop/src-tauri/crates/tools/archi-api-gen/src/main.rs` -> `packages/desktop/src-tauri/crates/tools/archi-api-gen/src/main_test.rs`

**改写既有测试文件**——纪律断言**翻转**（本地写盘 1 → 0、本地 walker 三件迁出改「零本地副本」断言），三子命令行为改走真实 `api::*` + `write_artifacts`（tempdir）。design 事实校准 11 把本文件登记为 `cargo check --all-targets` 的**编译闭合面**：实现相位只做四处调用点机械改调（`super::collect` → `archi::api::workspace_render`、`super::differences` → `archi::api::compare`、`super::write` → `archi_runtime::api_artifacts::write_artifacts`（单份改 `std::slice::from_ref`）、`Report::to_json()` → `super::diff_json(&report)`）与 doc 表述订正，**零用例新增、零断言删除、零断言语义改写**；下列用例本体（含纪律断言翻转）归测试相位。装置沿既有定式（自建 `TempDir` + `snapshot()` + 真实 `stage_crate` / `stage_ts` 语料）。

#### 待测功能

<!-- design.md「公共函数 / API」表对本文件零行：`run` 是 `pub(crate)` 的离 `main` 实现体（既有测试经 `super::run` 驱动），未进 API 表；本变更只做「去掉本地实现 + 改调 core/infra」的编排收窄（D11）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 本地写盘面翻转（AC-4 / AC-5 / D11） | 异常 | `include_str!("main.rs")` 自证：`fs::write` 计数 **0**（原 1）、`create_dir_all` **0**（原 1）、`File::create` 0；`archi_runtime::api_artifacts::write_artifacts` 恰 1 处调用 | 单次（源码自证） |
| 本地 walker / 比对面迁出（AC-4 / D11） | 异常 | `include_str!("main.rs")` 自证：`fn collect` / `fn write` / `fn differences` / `generated_files_on_disk` / `is_generated_module_file` / `walk_modules` / `fn walk` / `SKIPPED_DIRECTORIES` / `MAX_MODULE_WALK_DEPTH` / `model_elements` / `struct Report` **全部零命中**（改为「零本地副本」断言，判据名与 core 侧新落点对齐）；`api::workspace_render` / `api::compare` / `write_artifacts` / `api::bootstrap` 各恰一处 | 单次（源码自证） |
| `generate` 写四类生成物（AC-4 / AC-2） | 正向 | tempdir 语料（crate 元素 + ts 元素 + 契约）→ `run(Generate, root)` 返回 `Ok(0)`；四类产物在盘、内容与 `workspace_render` 逐份相同；`written + unchanged` = 应生成份数（首轮 `unchanged = 0`） | 单次 |
| `generate` 幂等（AC-4 / AC-2） | 正向 | 连调两次 → 两次均 `Ok(0)`；第二次全 `unchanged`、盘上 mtime 不变（先比对的语义经单点继承） | 单次 |
| `--check` 无差异 → 退出码 0（AC-4 / D11） | 正向 | 先 `Generate` 再 `Check` → `Ok(0)`；stdout 零 JSON 行 | 单次 |
| `--check` 三类差异 → 退出码 1 + 单行 JSON 三键（AC-4 / D11 / 脚本面契约） | 异常 | 造「不符 + 孤儿 + 缺失」→ `Ok(1)`；stdout 恰一行可解析 JSON、键序 `["mismatched","orphaned","missing"]`、逐值与 `api::compare` 三列相同（两驱动面同源） | 单次 |
| `check` 零写盘总断言（AC-4 / AC-2） | 边界 | `Check` 前后 `snapshot()` 逐字节相等；mtime 不变；零临时文件残留 | 单次 |
| `generate` 结构错模型即 `Err` 且零写盘（AC-4 / D1） | 异常 | 坏模型 → `run(Generate)` 产 `Err`（退出码面 1）；快照前后相等（fail-closed：坏模型不得固化成入库生成物） | 单次 |
| `bootstrap` 只出 stdout（AC-4 / 既有回归登记） | 边界 | 既有 `bootstrap只出stdout` / `bootstrap不读knip` / `bootstrap元素不存在` 三用例**逐字保留**（子命令面零改）；调用前后盘上快照不变 | 单次（回归登记） |
| 退出码三态互不混用（AC-4 / D11） | 边界 | 无差异 `0` / 有差异或失败 `1` / 用法错 `2`——既有 `退出码面三分互不混用` 断言保留；`Usage` 文案逐字不变 | 单次（回归登记） |
| 孤儿不删（AC-2 / 硬线二） | 异常 | `Check` 报 `orphaned` 非空后执行 `Generate` → 该孤儿仍在盘、字节不变；后续 `Check` 仍报同一条 `orphaned`（生成不清理） | 单次 |
| 零子进程与 `repository_root` 不变量（AC-4 / 既有回归登记） | 边界 | 既有 `零spawn双零命中`、`repository_root源码面机械断言`、`repository_root保持私有`、`repository_root退化为cwd的不变量` 四用例**逐字保留**；`use` 区不再需要 `fs` / `BTreeSet` 时不出现死导入（`cargo clippy` 编译闭合） | 单次（回归登记） |
| `root` 非法与超大差异清单（边界） | 边界 | `root` 为空 `Path` / 不存在 / 指向文件 → `run` 落 `Result` 内不 panic；1000 条差异 → 单行 JSON 可解析、三数组长度正确、不抛错 | 参数化迭代（3 非法 root + 1 超大） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 文件系统 | **不 mock**：真实自建 tempdir + 真实落盘 + `snapshot()` 对拍 | 全部用例 |
| `archi::api` / `archi_runtime::api_artifacts` | **不 mock**：真实调用（「两驱动面同一实现」正是被测语义；替身会让同源断言自证） | 全部用例 |
| 子进程 | **零依赖**：CLI 本层零 spawn（纪律断言自证） | —— |

---

### `packages/desktop/src-tauri/src/commands/archi/mod.rs` -> `packages/desktop/src-tauri/src/commands/archi/mod_test.rs`

**改写既有测试文件**——追加两命令与两 `*_inner` 的逐字段一致、root 校验扩到八命令（既有 `六命令root校验四形态一律err` 的用例名与调用面扩为符实，**不新增第二条同义用例**）、check 零写盘、generate 后复检清空、结构错模型双 `Err`。装置沿既有定式：`tauri::test::mock_app()` + 真实 `WorkspaceStores`（tempdir 数据根）；无 State 的七命令直调原签名。

#### 待测功能

- `archi_api_check(root: String) -> Result<ArchiApiCheck, String>`: `#[tauri::command] #[specta::specta]`；薄包装 = `archi_api_check_inner(Path::new(&root))`；零写盘、零 spawn
- `archi_api_check_inner(root: &Path) -> Result<ArchiApiCheck, String>`: `check_root` → `archi::api::artifact_check` → `ApiError::message()`
- `archi_api_generate(root: String) -> Result<ArchiApiWriteOutcome, String>`: 第二条写命令；只增只改不删
- `archi_api_generate_inner(root: &Path) -> Result<ArchiApiWriteOutcome, String>`: `check_root` → `archi::api::workspace_render` → `archi_runtime::api_artifacts::write_artifacts`

<!-- design.md「公共函数 / API」表对本文件恰 4 行（上列）；既有六命令与六个 *_inner 签名 / 语义零变化。`check_root` / `write_atomic` 是既有私有件，不入表。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `archi_api_check` ↔ `_inner` 逐字段一致（AC-1 / AC-3） | 正向 | 健康语料 + 一份不符 + 一份孤儿 → 包装与 `_inner` 各自 `Ok`，两者 `serde_json::to_value` 逐字段相等（命令层零投影，沿既有「包装与 inner 一致」形态） | 单次 |
| `archi_api_generate` ↔ `_inner` 逐字段一致（AC-1 / AC-3） | 正向 | 同一语料 → 两列 `written` / `unchanged` 逐字段相等；两份调用各自幂等（第二次全 `unchanged`） | 单次 |
| `archi_api_generate_inner` 的编排与写盘面（AC-2 / D12） | 正向 | `_inner(root: &Path)` 直调 → 先 `workspace_render` 取应生成集、再经 `write_artifacts` 落盘（`include_str!("mod.rs")` 自证零第二份渲染 / 零 `fs::write`）；返回两列与 `write_artifacts` 直调逐字段相等；盘上与应生成集逐份一致 | 单次 |
| root 校验扩到八命令（AC-1 / spec「root 无效」） | 异常 | `root` 为 `""` / `"   "` / 指向文件 / 不存在目录 → `archi_api_check` 与 `archi_api_generate`（含各自 `_inner`）均 `Err`，文案沿用 `check_root` 口径（`root 无效` / `root 不是目录`）；并入既有同一用例、用例名改「八命令」 | 参数化迭代（4 root 形态 × 2 函数） |
| `archi_api_check` 零写盘（AC-2 / spec「检查命令零写盘」） | 边界 | 调用前后 `snapshot(root)`（递归 `(PathBuf, Vec<u8>)`）相等；模型目录与契约目录 mtime 不变；零 `.tmp` 残留；并入既有 `四条读命令零写盘` 的快照段（用例名随实） | 单次 |
| `archi_api_generate` 写后复检三列全空（AC-2） | 正向 | 首轮 `check` 三列非空 → `generate` → 再 `check`：三列全空、`enabled` 保持；`written + unchanged` = 应生成份数 | 单次 |
| `archi_api_generate` 孤儿仍在盘（AC-2 / 硬线二） | 异常 | 盘上预置孤儿 `crates/core/archi.i.rs` → `generate` 后该文件字节与 mtime 不变；复检 `orphaned` 仍报同一条 | 单次 |
| 结构错模型 → 两命令均 `Err` 且零写盘（AC-2 / D12 / D1） | 异常 | 模型含环 → `archi_api_check` 与 `archi_api_generate` 均 `Err`（文案含「模型结构错」）；两次调用前后快照相等 | 参数化迭代（2 命令) |
| `enabled = false` 时 check 仍 `Ok`（AC-10 的数据侧） | 边界 | 无 `<archi_api_dir>` → `archi_api_check` 返回 `Ok`、`enabled == false`、`missing` 恰 `INDEX.toml` 一条；`archi_api_generate` 仍可写该份（应生成集只此一份）→ `written` 含它 | 单次 |
| 写失败文案透传不称全或无（AC-2 / spec「非原子如实」） | 异常 | 目标是普通文件占位的父路径 → `archi_api_generate` 返回 `Err`；文案原样透传 `write_artifacts` 的错误、零包装零补默认值；不含「全部成功」类措辞 | 单次 |
| 命令层零领域规则与头注释口径（AC-1 / D12 / 事实校准 4） | 异常 | `include_str!("mod.rs")` 自证：两新函数体内零 `workspace_render` 以外的渲染 / 比对 / 归一（`archi_api_check_inner` 只调 `artifact_check`）；`fs::write` / `create_dir_all` / `write_atomic` 仍只属 `archi_set_gate` 面（新命令零命中）；文件头「八命令」与「写命令两条」在位，零「六命令」/「唯一写命令」/「其余五条」 | 单次（源码自证） |
| 既有六命令零变化（AC-19 / 回归登记） | 边界 | `archi_graph` / `archi_module` / `archi_change_summary` / `archi_set_gate` / `archi_drift` / `archi_file_tree` 的签名 / 出参 / 错误映射逐字不变；`archi_drift` 的 VCS 缝、`set_gate` 原子写回、`archi_module` 的 `api` 三路径等既有 20+ 条用例**保持绿、零改动** | 单次（回归登记） |
| `root` 字符串边界（边界 / str 映射） | 边界 | `root` 超长（>1000 chars）/ 含空格 / 非 ASCII / emoji → 落 `Result` 内不 panic（`Ok` 或 `Err` 在用例内钉死）；空串形态已由 root 校验行覆盖 | 参数化迭代（4 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| Tauri 运行时 | **进程边界装置，非仓内模块 mock**：`tauri::test::mock_app()`（沿既有装置；两新命令无 State，直调原签名即可） | 全部用例 |
| `archi::api::artifact_check` / `workspace_render` / `archi_runtime::api_artifacts::write_artifacts` | **不 mock**：真实调用（薄包装语义即「零加工」；替身会让逐字段一致与「写后清空」自证） | 全部用例 |
| 文件系统 / store | **不 mock**：真实 tempdir 落模型 / 契约 / 源文件；`snapshot()` 判零写盘 | 全部用例 |
| 网络 / 子进程 | **零依赖**（两命令零 spawn） | —— |

---

### `packages/desktop/src-tauri/src/commands/mod.rs` -> `packages/desktop/src-tauri/src/bindings/mod_test.rs`

`commands/mod.rs` 无独立 sidecar（只有 `all_commands!` 宏体与 `TEST_PATH_LOCK`），登记面断言的唯一可测落点是 `bindings/mod_test.rs` 的清单守卫（AC-1 / AC-17 的验证方法点名该文件）。本节承载**登记顺序**与**清单随动**两侧；`bindings/mod_test.rs` 的用例本体随动在此登记，其文件本身归「不可测试项」。

#### 待测功能

<!-- design.md「公共函数 / API」表对本文件零行（`all_commands!` 追加两条是宏体改动，不是 API 行）。条目改列 design「修改文件」表 commands/mod.rs 行与 `bindings/mod_test.rs` 行；断言对象是五个清单常量与三组计数断言。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `COMMAND_NAMES` / `COMMAND_WRAPPERS` 随动（AC-1 / AC-17） | 正向 | 两清单各含 `"archi_api_check"` / `"archiApiCheck"` 与 `"archi_api_generate"` / `"archiApiGenerate"`，插在 `archi_file_tree` / `archiFileTree` 之后、`read_file` / `readFile` 之前；`read_file` / `readFile` 仍为**尾条** | 单次 |
| 两组计数断言随动（AC-17 / R-8） | 正向 | 三处 `COMMAND_NAMES.len()` 与三处 `COMMAND_WRAPPERS.len()` 断言同值更新为 **53**（六处；断言语义不变，只随清单长度）；`DTO_TYPES.len()` 断言 101 → **104**；相关说明串的「51 → 53」类表述同步 | 参数化迭代（7 处计数） |
| `DTO_TYPES` 随动与字典序插入（AC-3 / AC-17） | 正向 | 含 `ArchiApiCheck` / `ArchiApiMeta` / `ArchiApiWriteOutcome`（archi 段内按字典序）；首锚 `ActivePhase` / 尾锚 `WarningKind` 不动 | 单次 |
| `ARCHI_COMMANDS` 族清单 6 → 8（AC-1 / AC-17） | 正向 | 数组类型为 `[(&str, &str); 8]`，序为 `archi_graph` / `archi_module` / `archi_change_summary` / `archi_set_gate` / `archi_drift` / `archi_file_tree` / **`archi_api_check`** / **`archi_api_generate`** | 单次 |
| `ARCHI_DTO_TYPES` 族清单 23 → 26（AC-3 / AC-17） | 正向 | 数组类型 `[&str; 26]`；三新类型按字典序插入；逐名命中产物 `export type <Name> =` | 参数化迭代（26 名） |
| 尾部追加序期望 vec 随动（AC-17 / 易漏项） | 正向 | 既有期望 vec 增两条（`"archi_api_check"` / `"archi_api_generate"`，位于 `"archi_file_tree"` 之后、`"read_file"` 之前）；整条 `assert_eq!` 逐字相等 | 单次 |
| 族末条对照臂改判（AC-17 / 清单遗漏即产物覆盖断言必红） | 异常 | 族末条由 `archi_file_tree` 变 `archi_api_generate` → `copy.pop()` 取到 `archi_api_generate`；「原清单条目在产物内」与反向两臂（副本无缺失 / 确已删去该条）**断言体零改**即自动改判为绿 | 单次 |
| 两新命令的 args / result schema 形状（AC-3 / AC-17 / spec Module Contract 生成物行） | 正向 | `bindings.zod.ts` 含 `archiApiCheck` / `archiApiGenerate` 的 `argsSchemas` 表项（`root: z.string()`）与 `resultSchemas` 表项（`ArchiApiCheck` / `ArchiApiWriteOutcome`）；两份产物各含三个新类型的具名导出一处；签名为 `(root: string) => __TAURI_INVOKE<T>("archi_api_*", { root })` | 参数化迭代（2 命令 + 3 类型） |
| 既有条目零位移（AC-17 / 尾锚判据） | 边界 | `app_environment` 之前的全部条目与改动前逐字同序；新增项只插在 archi 段尾部（`archi_file_tree` 之后）；其余条目零位移 | 单次 |
| 重导出幂等（AC-17 / 既有零 diff 门） | 边界 | 连续两次 `export_bindings()` 产物逐字节一致（既有幂等用例**零改动**）；本行只作回归登记 | 单次（回归登记） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 绑定的真实导出面 | **不 mock**：`authoritative_snapshot()` 真实调用 `export_bindings()` 产出两份产物（清单 ↔ 产物强耦合即被测语义）；变更类用例以 `RestoreOnDrop` 兜底恢复，串行靠既有 `file_lock()` | 全部用例 |
| `all_commands!` 宏 | **不 mock**：清单常量是手写副本，与宏体的对拍即被测语义（宏不反射，故以「产物含命令名」间接见证） | 全部用例 |
| 时钟 / 网络 / 子进程 | **零依赖**（`export_bindings` 为进程内调用） | —— |

---

### `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs` -> `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount_test.rs`

**改写既有测试文件**（静态清单 10 → 11 项）——沿既有零 mock 定式（纯函数 / `OnceLock` 静态表，无 IO）。

#### 待测功能

<!-- design.md「公共函数 / API」表对本文件零行：本变更新增 `ARCHI_API_TOOL` 常量（`pub(crate)`）与一份 `ToolMount` 静态项、私有 `archi_api_definition()`，未进 API 表；行为契约归 D14 / `desktop-archi-agent` ADDED 要求。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 静态清单 11 项与期望序（AC-15 / D14） | 正向 | `EXPECTED_NAMES` 扩为 11 项、`archi_api` 位于 `archi_search` 之后；`TOOL_NAMES.len() == 11`；`builtin()` / `definitions()` 的长度与名字投影均对齐 | 单次 |
| `archi_api` 的档位与执行路由（AC-15 / spec「只读档位归属」） | 正向 | `lookup("archi_api")` → 该 `ToolMount` 的 `tier == ToolTier::Read` 且 `exec == ToolExec::Archi`；`ToolExec::Archi` 的臂名清单含三工具（源码自证） | 单次 |
| `policy::allows` 对 `archi_api` 三档恒放行（AC-15） | 正向 | `ToolTier::Read` 在 `Ask` / `Auto` / `Flow` 三档均 `allows == true`（沿既有 policy 判据；零放行名单变化） | 参数化迭代（3 档） |
| `archi_api` 的入参 schema（AC-15 / D13） | 正向 | `definitions()` 内该定义：`action` 为封闭二值（`check` / `bootstrap`）、`element` 可选（string）、`required == ["action"]`；描述含「生成物过期用既有命令重生成，MUST NOT 手改生成物」要点 | 单次 |
| 非法名与既有查表语义零变化（AC-15 / 回归登记） | 异常 | 未知名 `"archi_write"` / `"archi_api "`（尾空格）/ `""` → `lookup` 均 `None`（成员校验 fail-closed）；既有十项 `lookup` 全 `Some` | 参数化迭代（3 非法名） |
| 既有十项零位移（AC-19 / 回归登记） | 边界 | 前七内建 + `create_explore_note` + 两 archi 工具的名字 / tier / exec 逐字不变；`finish` 位置位铸造断言不变 | 单次（回归登记） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 静态注册表 | **不 mock**：真实 `registry()` / `definitions()` / `lookup`（`OnceLock` 一次性表，纯函数零 IO） | 全部用例 |
| `policy::allows` | **不 mock**：真实调用（Read 档三档放行是判定单点） | 三档行 |
| 时钟 / 网络 / 子进程 / 文件系统 | **零依赖** | —— |

---

### `packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool.rs` -> `packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool_test.rs`

**改写既有测试文件**（`ArchiContext::execute` 加 `archi_api` 一臂）——沿既有零 mock 定式（真实 tempdir 模型 / 契约 / 源文件 + 真实 `archi::dto` / `archi::api` + 真实 `tools::cap_result`）。

#### 待测功能

<!-- design.md「公共函数 / API」表对本文件零行：执行体是 `ArchiContext`（`pub(crate) struct`）的私有方法，按 design 口述「不单列」；行为契约由 D13 与 `desktop-archi-agent` ADDED 要求承载。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `check` 动作：结构诊断非空 → `is_error`（AC-15 / D13） | 异常 | 模型含环 → 结果 `is_error == true`，文案含「模型结构错」与诊断条数（经 `graph_diagnostics_error` 单点，零第二份 `file:line` 拼装）；不返回部分两轴 | 单次 |
| `check` 动作：两轴结构化摘要（AC-15 / D13） | 正向 | 健康语料 + 一份不符 + 一份孤儿 + 两条判据 → 输出 JSON 含 `artifacts`（`enabled` / `meta` / 三列）与 `findings`（`total` / `byCategory` / `items`）；`findings.total == items.len()`；`byCategory` 各计数与 `items` 按 `category` 分组计数逐项相等（零新词表） | 组合迭代（2 轴 × 3 判据类别） |
| `check` 动作：判据 0 条的成功态（AC-15 / 边界） | 边界 | 契约面缺席 → `findings.total == 0`、`byCategory == {}`、`items == []`；`artifacts.enabled == false` 且 `missing` 恰 `INDEX.toml`；`is_error == false` | 单次 |
| `bootstrap` 动作：出草稿（AC-15 / D13） | 正向 | `{ action: "bootstrap", element: "<在模型内的 FQN>" }` → 输出含 `draft` 文本且非空；与 `archi::api::bootstrap` 直出逐字相等 | 单次 |
| `bootstrap` 动作：缺参 / 空白参 → `is_error`（AC-15 / spec「未知 action 与缺参」） | 异常 | ①`element` 缺失②`element: ""`③`element: "   "` → 均 `is_error == true`，文案点名缺参；不写盘 | 参数化迭代（3 形态） |
| 未知 / 缺失 `action` → `is_error`（AC-15 / spec「未知 action」） | 异常 | `{ action: "write" }` / `{ action: "CHECK" }` / `{}`（`action` 缺失）/ `{ action: null }` → 均 `is_error == true`，文案含合法值 `check / bootstrap` | 参数化迭代（4 形态） |
| 输出过 `cap_result`（AC-15 / D13） | 边界 | 造 3000 条判据（`items` 超大）→ 输出长度 ≤ L1 上限、含截断留痕文案；`total` 与 `byCategory` 全量保留（摘要优先）；不 panic | 单次 |
| 两动作零写盘（AC-15 / spec「零写盘」） | 边界 | `check` 与 `bootstrap` 调用前后对 `root` 快照相等；`include_str!("archi_tool.rs")` 零 `fs::write` / `create_dir` / `create_dir_all` | 参数化迭代（2 动作） |
| 判据来源单点（AC-15 / D7） | 异常 | `include_str!("archi_tool.rs")` 自证：`archi::api::findings` 恰一处、`archi::api::artifact_check` 恰一处、零 `load_contracts` 直接调用（零第二份装配） | 单次（源码自证） |
| 既有 `archi_module` / `archi_search` 用例零变化（AC-19 / 回归登记） | 边界 | 两项的既有断言（两形态 / 三 scope / 坏模型 / 参数缺失 / 30KB 收口）**逐字保留**；`execute` 的既有两臂行为不变 | 单次（回归登记） |
| 参数脏值不 panic（边界 / str 映射） | 边界 | `element` 超长（>1000 chars）/ 含 emoji / 含 `/` → 由 `api::bootstrap` 判定为 `Err`/`Ok` 之一，`is_error` 与之同向、不 panic；`input` 为 `null`（越签名脏值）→ `is_error` | 参数化迭代（4 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 文件系统 | **不 mock**：真实 tempdir 落模型 / 契约 md / 源文件 | 全部用例 |
| `archi::dto::graph_view` / `archi::api::{artifact_check, findings, bootstrap}` / `tools::cap_result` | **不 mock**：真实调用（判定单源与 L1 上限即被测契约；替身会让「零第二份装配」自证） | 全部用例 |
| 时钟 / 网络 / 子进程 | **零依赖** | —— |

---

### `packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools.rs` -> `packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools_test.rs`

**改写既有测试文件**（两行随动：名字再导出名单 + `input_path` 零路径分支，事实校准 2）——沿既有零 mock 定式。

#### 待测功能

<!-- design.md「公共函数 / API」表对本文件零行：本变更只改 `pub(crate) use` 名单与 `input_path` 的一个条件分支（零行为变化，缺了功能上仍 None）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ARCHI_API_TOOL` 名字再导出可见（AC-15 / 事实校准 2） | 正向 | `tools::ARCHI_API_TOOL == "archi_api"`；与 `mount::ARCHI_API_TOOL` 同一常量（值相等） | 单次 |
| `input_path` 对三 archi 工具均 `None`（AC-15） | 边界 | `input_path("archi_module", …)` / `("archi_search", …)` / `("archi_api", …)` → 三种均 `None`（archi 工具无路径入参）；对照臂：`read` / `write` / `edit` / `grep` 的 `input_path` 行为零变化 | 参数化迭代（3 archi + 4 内建） |
| `input_path` 脏入参不 panic（边界 / object 映射） | 边界 | `archi_api` 的 `input` 为 `{}` / `null` / 含 `file_path: 123` 形态 → 仍 `None`、不 panic | 参数化迭代（3 形态） |
| `archi_api` 不得成为路径型工具（AC-15 / 反向判据） | 异常 | `builtin()` 名单**不含** `archi_api`（它不是七内建之一）；`input_path("archi_api", { element: '/etc/passwd' })` 仍 `None`（声称的元素名不得被当作路径入参外泄）；`mount::lookup("archi_api")` 存在但 tier 非 `Write` | 单次 |
| `cap_result` 上限语义零变化（AC-15 / 回归登记） | 边界 | 既有 L1 上限与截断留痕文案断言**逐字保留**（本变更零改上限实现） | 单次（回归登记） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `sdk::mount` 常量与 `input_path` | **不 mock**：真实调用（纯函数，零 IO） | 全部用例 |
| 时钟 / 网络 / 子进程 / 文件系统 | **零依赖** | —— |

---

### `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs` -> `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner_test.rs`

**改写既有测试文件**（`mint_archi_context` 的 present 判据加 `ARCHI_API_TOOL`）——沿既有零 mock 定式。

#### 待测功能

<!-- design.md「公共函数 / API」表对本文件零行：`mint_archi_context` 是私有件，本变更只扩 present 判据的名字清单（D14）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| present 判据含 `archi_api`（AC-15 / D14） | 正向 | 注入名单只含 `archi_api` → `mint_archi_context` 产 `Some`（per-turn 上下文在场）；三工具各自单独在场均 `Some` | 参数化迭代（3 工具单点） |
| 零 archi 工具 → `None`（AC-15 / 回归登记） | 边界 | 注入名单只含内建工具 / `tools = None` / `tools = Some(vec![])` → `None`（不铸上下文） | 参数化迭代（3 形态） |
| present 判据为精确名字匹配（AC-15 / 反向判据） | 异常 | 注入名单含 `"archi_api_typo"` / `"archi_api "`（尾空格）/ `"ARCHI_API"`（大写）→ 均 `None`（零前缀 / 零大小写宽松匹配）；对照臂：`"archi_api"` 逐字即 `Some` | 参数化迭代（3 脏名 + 1 对照） |
| 既有 runner 断言零变化（AC-19 / 回归登记） | 边界 | 上下文路由、失败回灌、日志与事件面的既有用例**逐字保留** | 单次（回归登记） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `mint_archi_context` 与注入名单 | **不 mock**：真实调用（判据为纯字符串匹配） | 全部用例 |
| 时钟 / 网络 / 子进程 / 文件系统 | **零依赖** | —— |

---

### `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs` -> `packages/desktop/src-tauri/crates/infra/agent/src/compose_test.rs`

**改写既有测试文件**（archi 来源臂三工具全开 + `archi_matrix` 的 `(implement, executor)` 加一名字）——沿既有定式（真实 store / provider fixture 装配链路）。

#### 待测功能

<!-- design.md「公共函数 / API」表对本文件零行：`archi_matrix` / `injected` / `compose_turn` 不在该表；本变更只加一个来源臂名字与一行矩阵项（D14）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| archi 来源臂三工具全开（AC-15 / D14） | 正向 | `provenance.source = "archi"` 时注入名单含 `archi_module` / `archi_search` / `archi_api` 三名（与矩阵结果并集，按既有去重序） | 单次 |
| `(implement, executor)` 矩阵含两名（AC-15 / D14 / AC-16 的矩阵侧） | 正向 | `source_ref = "{change}/implement/executor/1"` → 注入名单含 `archi_module` 与 `archi_api`、**不含** `archi_search` | 单次 |
| 其余矩阵行零变化（AC-15 / 回归登记） | 边界 | `proposal/executor` 仅 `archi_search`；`dev-design` 两角色两名；`test-design/executor` 仅 `archi_search`；`code-review/evaluator` 与 `acceptance/evaluator` 仅 `archi_module`；其余相位 `None`（零 archi 工具） | 参数化迭代（7 行） |
| 未知名 fail-closed 与 `source_ref` 解析纪律（AC-15 / 回归登记） | 异常 | 既有「注入面非法工具名 → `Err`」与尾三段解析（畸形 / 短段 / 末段非数字 → `None`）断言**逐字保留** | 单次（回归登记） |
| 零 `toolset` 概念（AC-15 / spec「零 toolset 概念」） | 异常 | `include_str!("compose.rs")` 零 `SessionToolset` / 零 `toolset` 字段；矩阵来源仍是工具名字清单（三名字常量直接 `push`） | 单次（源码自证） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| store（`native_db`）/ provider fixture | **不 mock**：真实 `WorkspaceStores`（tempdir 数据根）+ 既有 seed 装置（`compose_turn` 的装配链路即被测语义） | 全部用例 |
| `mount::lookup` / `injected` / `archi_matrix` | **不 mock**：真实调用（成员校验与矩阵是判定单点） | 全部用例 |
| 网络 / 子进程 | **零依赖**（引擎连接配置在装配期构造，不发起请求） | —— |

---

### `packages/desktop/src-tauri/crates/infra/agent/src/archi-knowledge.md` -> `packages/desktop/src-tauri/crates/infra/agent/src/archi_knowledge_test.rs`

**改写既有测试文件**——自检纪律补一条（`archi_api` 两轴 + 生成物用既有命令重生成、MUST NOT 手改）；零 `<>` 占位符与体量上限的既有断言保留。沿既有零 mock 定式（`include_str!` 同一份字节 + 真实 `archi` 解析面）。

#### 待测功能

<!-- design.md「公共函数 / API」表对本文件零行（静态 md，非 API）。条目改列 design「修改文件」表 archi-knowledge.md 行与 D21 / AC-16。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 自检纪律含新句（AC-16 / D21） | 正向 | `knowledge()` 产物含 `archi_api` 与「MUST NOT 手改生成物」要点；点名两轴（生成物三类差异 / 契约判据）与重生成命令（`pnpm api:export` 或 `cargo run -p archi-api-gen -- generate`）；与 `include_str!("archi-knowledge.md")` 同源（零测试内副本） | 单次 |
| 零占位符（AC-16 / spec「零 `<>` 占位符」） | 异常 | 全文零 `<>`、零 `{{`、零 `__INCLUDE:`；`trim()` 非空 | 参数化迭代（3 片段） |
| 体量上限内（AC-16 / spec「体量受既有上限约束」） | 边界 | 拼装产物字节数 ≤ 既有上限；较改动前增量有界（新增恰一条 bullet，零整段重写）；1000 行级异常语料不适用（静态文件） | 单次 |
| 既有词表三面与目录派生在场（AC-19 / 回归登记） | 边界 | kind / gate / 结构诊断三面词表在场、目录路径派生在场、唯一示例 TOML 双零诊断的既有断言**逐字保留** | 单次（回归登记） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `archi_knowledge::build` / `knowledge` | **不 mock**：真实调用（词表派生 + 骨架拼装即被测面） | 全部用例 |
| 骨架字节 | **不 mock**：`include_str!` 编译期内嵌同一份文件（禁测试内复制副本） | 全部用例 |
| 文件系统 / 网络 / 子进程 | **零依赖**（零 IO） | —— |

---

### `packages/desktop/src-tauri/crates/core/workflow/src/prompts/implementation-generator.md` -> `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table_test.rs`

**改写既有测试文件**——`ARCHI_PROMPT_MATRIX` 扩为「模块 / 发现 / 契约自检」三维登记面、越权扫描名单加 `archi_api`（现有断言「无第三工具」的表述随实为「三名字各自按矩阵勾选」）。装置沿既有定式（真实读盘 prompt md + 真实 `phase_table` 装配面）。

#### 待测功能

<!-- design.md「公共函数 / API」表对本文件零行（静态 prompt md）。条目改列 design「修改文件」表 implementation-generator.md 行与 D21 / AC-16；断言对象是 phase_table_test.rs 的 prompt 矩阵与占位扫描。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `implementation-generator` 含 `archi_api`（AC-16 / D21） | 正向 | 该 md 的「额外可用只读工具」行含 `archi_api`，且说明「改完代码自查生成物新鲜度与契约判据」；仍含既有 `archi_module`、**不含** `archi_search` | 单次 |
| 其余 13 份 md 零 `archi_api`（AC-16 / spec「其余相位 prompt 零 diff」） | 异常 | 逐份 `prompt_text(name)` 断言 `!contains("archi_api")`；矩阵登记面 `api` 维度仅 `implementation-generator` 为 `true`（反向臂：其余六份命中 md 与七份非命中 md 全 `false`） | 参数化迭代（13 份） |
| 矩阵三维逐文件对拍（AC-16 / 改动面） | 正向 | `ARCHI_PROMPT_MATRIX` 由 `(name, module, search)` 扩为 `(name, module, search, api)`；七行勾选状态与正文 `contains` 逐项一致；`两工具名并集恰七份且无第三工具` 改名为「三名字并集与矩阵一致」并把 `archi_api` 纳入越权扫描白名单 | 参数化迭代（7 行 × 3 名字） |
| 14 份 md 零占位与装配面逐字（AC-16 / 回归登记） | 边界 | 既有 `十四份md零占位与零mcp`、`装配面include_str逐字不变且工具面不落prompt`、`命中md零路径模板` 三断言**逐字保留**；`prompt_names().len() == 14` 不变（零新增 / 删除 md） | 单次（回归登记） |
| 非命中 md 零 archi 字样（AC-16 / 回归登记） | 异常 | 七份非命中 md（`proposal-evaluator` / `test-design-evaluator` / `implementation-evaluator` / `test-gen-generator` / `test-gen-evaluator` / `code-analyze-planner` / `code-analyze-evaluator`）仍零 `archi` 字样（含 `archi_api`） | 参数化迭代（7 份） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| prompt md 盘面 | **不 mock**：真实 `std::fs::read_to_string(prompts_dir()/…md)`（文本即被测对象） | 全部用例 |
| `phase_table` 装配面 | **不 mock**：真实调用（`include_str!` 逐字与 md 盘面对拍） | 回归登记行 |
| 网络 / 子进程 | **零依赖** | —— |

---

### `packages/desktop/src/views/architecture/hooks/use-archi-api-check.ts` -> `packages/desktop/src/views/architecture/hooks/use-archi-api-check.test.ts`

**新建测试文件**（`tasks.md` 阶段十建空壳：仅头注释）——沿 `use-archi-drift.test.ts` 定式：`renderHook` + `rerender` + `act` + `waitFor`；进程边界 mock `@tauri-apps/api/core` 的 `invoke`（生成的 `commands.archiApiCheck` 最终经此调用），按命令名分发 fixture 并记录调用序列与载荷。夹具：`apiCheck(overrides)` 构造五字段 DTO（三列缺省空）。

#### 待测功能

- `useArchiApiCheck(root: string | null): ArchiApiCheckState`: 显式取数 `commands.archiApiCheck(root)`；三触发（挂载 / `root` 变化 / 手动 `refresh()`）；`root === null` 零 invoke；失败清旧数据（旧差异 MUST NOT 冒充本次结果）、只落 `error`；零 watch 零轮询；返回 `{ check, loading, error, refresh }`

<!-- design.md「公共函数 / API」表对本文件恰 1 行（上列）；`ArchiApiCheckState` 在「类型定义」表，形状覆盖并入返回值的四成员断言。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `useArchiApiCheck` 挂载触发（AC-12 / spec「三触发与零轮询」） | 正向 | `renderHook(() => useArchiApiCheck('/ws/alpha'))` → 恰一次 `archi_api_check` invoke、载荷 `{ root: '/ws/alpha' }`；落定后 `check` 等于 fixture、`loading === false`、`error === null` | 单次 |
| `useArchiApiCheck` `root` 变化触发（AC-12） | 正向 | `rerender({ root: '/ws/beta' })` → 共两次 invoke、末次载荷 `{ root: '/ws/beta' }`；落定后 `check` 为 beta 的 fixture | 单次 |
| `useArchiApiCheck` `refresh()` 触发（AC-12 / spec「手动刷新」） | 正向 | 落定后 `act(() => result.current.refresh())` → 第三次 invoke、载荷同 `root`；`loading` 期间为 `true`、落定后 `false` | 单次 |
| `useArchiApiCheck` `root === null` 零 invoke（AC-12 / spec「`root === null` 零 invoke」） | 边界 | `renderHook(() => useArchiApiCheck(null))` → invoke 零调用；`check === null` / `loading === false` / `error === null`（三者复位）；随后 `rerender({ root: '/ws' })` → 一次 invoke | 单次 |
| `useArchiApiCheck` `root` 由值转 `null` 复位（边界 / 生命周期交错点） | 边界 | root A 已落定 → `rerender({ root: null })` → `check === null`（旧差异 MUST NOT 残留）、`loading === false`、零新 invoke | 单次 |
| `useArchiApiCheck` 失败清旧数据（AC-12 / spec「失败只落行内错误」） | 异常 | root A 成功落定 → `refresh()` 且本次 reject(`new Error('模型结构错')`) → `check === null`（旧 A 的三列 MUST NOT 冒充本次结论）、`error` 含「模型结构错」、`loading === false` | 单次 |
| `useArchiApiCheck` 失败后成功清 `error`（AC-12） | 正向 | 首次 reject → `error` 非 `null`；再 `refresh()` 且 resolve → `check` 非 `null`、`error === null`（等待信号取「成功后才有」的 `check`，避开 effect 起始同步清 `error` 的竞态） | 单次 |
| `useArchiApiCheck` 新 `root` 未回时旧结果不冒充（AC-12 / `loadedRoot` 判据） | 边界 | root A 已落定；`rerender({ root: '/ws/beta' })` 且 B 挂起 → `check === null`；B 落定后 `check` 为 B fixture | 单次 |
| `useArchiApiCheck` 竞态：旧 root 迟到应答不回写（AC-12 / 交接点） | 异常 | 两次 invoke 挂起（A、B）；先 resolve B 再 resolve A（迟到）→ `check` 仍为 B fixture（`cancelled` 旗标拦旧应答） | 单次 |
| `useArchiApiCheck` 卸载后不写状态（AC-12） | 边界 | `unmount()` 后 resolve 挂起应答 → 零 React 警告（无「state update on unmounted」）、invoke 恰一次 | 单次 |
| `useArchiApiCheck` 零轮询零 watch（AC-12 / spec「零 watch 零轮询」） | 异常 | `vi.useFakeTimers()` 下 `advanceTimersByTime(60_000)` → invoke 计数不变（`finally` 复原）；源码零 `setInterval` / `setTimeout` / `watchSubscribe` / 裸 `invoke(` 归「不可测试项」静态检索 | 单次 |
| `useArchiApiCheck` 返回键集恰四成员（AC-12 / D15） | 异常 | `Object.keys(result.current).sort()` 恰 `['check','error','loading','refresh']`；反向判据：多出成员即红 | 单次 |
| `useArchiApiCheck` 空差异与超大差异（边界 / list 映射） | 边界 | ①fixture 三列全空且 `enabled = true` → `check` 非 `null`（空差异是成功态，与未载入可辨）②三列各 1000 条 → `check.mismatched.length === 1000`、零裁剪（hook 纯透传） | 参数化迭代（2 形态） |
| `useArchiApiCheck` `root` 脏值（边界 / str 映射） | 边界 | `root = ''`（空串）/ 超长（>1000 chars）/ 含 emoji → 仍 invoke 一次且载荷逐字透传（hook 零前置校验，是否 `Err` 由后端裁）；`undefined`（越签名）→ 与 `null` 同向（零 invoke，用例内钉死实际行为） | 参数化迭代（4 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core` 的 `invoke` | **进程边界 mock**：`vi.hoisted` + `vi.mock` 替身，按命令名 `archi_api_check` 分发 fixture；`beforeEach` 重置并装配默认实现（沿 `use-archi-drift.test.ts` 同款） | 全部用例 |
| `commands.archiApiCheck`（生成绑定） | **不 mock**：真实生成包装经替身 `invoke` 触达（断言载荷即验证「包装 → invoke」映射） | 全部用例 |
| React 调度（`renderHook` / `act` / `waitFor`） | **不 mock**：真实 React 19 渲染与调度（hooks 生命周期即被测对象） | 全部用例 |
| 时钟 | **只 `vi.useFakeTimers()` 做零轮询臂**，`finally` 复原；其余用真时钟 | 零轮询行 |
| 文件系统 / 网络 / 子进程 | **零依赖** | —— |

---

### `packages/desktop/src/views/architecture/components/contract-panel.tsx` -> `packages/desktop/src/views/architecture/components/contract-panel.test.tsx`

**新建测试文件**（`tasks.md` 阶段十一建空壳）——渲染面套件（`environment: 'jsdom'`）：`render` / `fireEvent` / `screen` / `within` / `waitFor`；进程边界 mock `invoke`（`archi_api_check` / `archi_api_generate` / `read_file` 三命令分发）。查询锚点一律 design D18 的 testid 全集与 `data-axis` / `data-kind` / `data-category` / `data-source` 钩子。夹具：`apiCheck(overrides)` 构造五字段 DTO、`finding(overrides)` 构造六字段判据。

#### 待测功能

- `ContractPanel(props: ContractPanelProps): React.JSX.Element`: 两轴分区（生成物差异三分组 + 判据按 `category` chip，各自计数、零跨轴合计）；条目点击 `onFocus`（元素圈定）；路径行走 `FileViewer` + `read_file`；`[检查]` 与按模式的 `[生成]`；五态（`loading` / `enabled = false` / 零差异零判据 / 检查失败行内 / 生成失败顶部；状态词判定序 `root === null` → `error !== null` → loading → `enabled` → 零差异零判据，检查失败时 `-status` 不渲、不谎称「解析中…」）；生成前 `StandardDialog` 二次确认（写入 N / 保留 M 孤儿 / 非原子），成功后 `refresh()` 重跑 check

<!-- design.md「公共函数 / API」表对本文件恰 1 行（上列）；`ContractPanelProps` 在「类型定义」表，形状覆盖并入入参断言。私有件（分组渲染、chip 派生、状态行）不列本表，其行为经本节用例承接。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ContractPanel` 两轴分区与各自计数（AC-9 / spec「两轴并列」） | 正向 | `check` 三列各 1 条 + `findings` 3 条 → `-axis-artifacts` 文本含「生成物差异（3）」、`-axis-findings` 文本含「判据（3）」；两组 `-item` 的 `data-axis` 分别为 `artifacts` / `findings`；**全局**零「6」类合计（分轴取值断言，跨轴求和在任一容器内都不得出现） | 单次 |
| `ContractPanel` 三分组与组内计数（AC-9） | 正向 | `mismatched` 2 条 / `orphaned` 1 条 / `missing` 3 条 → 组标题分别含「不符 2」/「孤儿 1」/「缺失 3」；三组条目 `data-kind` 逐值命中；某列空则组标题计数为 0 且零 `-item` | 组合迭代（3 类 × 有/无两态） |
| `ContractPanel` 判据 chip 由数据派生（AC-9 / D16 / spec「零前端词表」） | 正向 | `findings` 的 `category` 取 `entry` / `signature` / `signature` / `nesting` → chip 恰三枚（去重后排序）、文案 `entry 1` / `signature 2` / `nesting 1`；零计数类别（如 `contract`）**不渲** chip；源码零类别常量表（自证行见下） | 单次 |
| `ContractPanel` chip 过滤与取消（AC-9） | 正向 | 点 `signature` chip → 列表只余该类别 2 条、chip `aria-pressed`（或等价选中态）为真；再点同 chip → 恢复全量 4 条；切到另一 chip → 只余该类别 | 参数化迭代（2 类别 × 切换 3 步） |
| `ContractPanel` 判据条目点击 → 一次 `onFocus`（AC-9 / spec「条目点击 = 聚焦」） | 正向 | `finding({ element: 'App.Auth' })` 条目点击 → `onFocus` 恰一次、实参 `App.Auth`；连点两条不同元素 → 各自一次（无去重缓存） | 单次 |
| `ContractPanel` `element === null` 条目不可点（AC-9 / 边界） | 异常 | `finding({ element: null })` 条目 → 渲出文本但点击 `onFocus` **零调用**；DOM 无 `button` / `role="button"` 包装（只渲文本） | 单次 |
| `ContractPanel` 生成物条目「打开」→ `read_file`（AC-9 / spec「路径行经 `FileViewer`」） | 正向 | `check.mismatched = ['crates/core/archi/src/lib.i.rs']`、`root = '/ws'` → 点该条目的「打开」→ `read_file` 恰一次、载荷 `{ root: '/ws', path: 'crates/core/archi/src/lib.i.rs' }` 形态（拼接口径为 `root + '/' + rel`，断言实际载荷键与值） | 单次 |
| `ContractPanel` `loading` 态（AC-10） | 边界 | invoke 挂起未回 → `-status` 文案「解析中…」；两轴 `-item` 零节点；`-check` / `-generate` 处于禁用（`loading` 期间） | 单次 |
| `ContractPanel` `enabled = false` 冷启动（AC-10 / spec「`enabled=false` 不得渲成空面板」） | 边界 | `check.enabled = false`、`missing = ['INDEX.toml']` → `-status` 文案含「契约面未启用」与 `INDEX.toml` 提示；`-axis-artifacts` **仍**列出该一条 `-item`（`data-kind="missing"`）；不渲成空面板 | 单次 |
| `ContractPanel` 零差异零判据（AC-10） | 边界 | 三列全空且 `findings = []` → `-status` 文案「生成物最新，判据 0 条」；零 chip、零 `-item`；不抛错 | 单次 |
| `ContractPanel` 检查失败行内错误与重试（AC-10 / D18 / spec「invoke reject → 行内错误」） | 异常 | `archi_api_check` reject(`new Error('root 无效')`) → `-error` 在场、`data-source="check"`、文案含原因；`-status` **不在场**（`statusText` 在 `error !== null` 处短路返回 `null`，MUST NOT 渲出「解析中…」——失败时 `check === null` 且 `loading === false`，判定序若先看 loading 就会自相矛盾）；点 `-check` → 第二次 invoke（可重试）且成功后错误消失 | 单次 |
| `ContractPanel` 生成失败顶部回显（AC-10 / spec「文案不声称全或无」） | 异常 | `archi_api_check` 正常、`archi_api_generate` reject(`new Error('写入失败（已写 2 份，不回滚）')`) → `-error` 在场、`data-source="generate"`、渲染在面板顶部（在 `-axis-artifacts` 之前）；文案透传原因、不含「全部失败」类措辞 | 单次 |
| `ContractPanel` 首帧与「未载入」态的解析中判据（AC-10 / D18 的判定序） | 边界 | `root !== null`、`check === null`、`loading === false`、`error === null`（effect 未跑 / 尚未回包）→ `-status` 仍为「解析中…」（`loading` 判据含 `check === null`），不渲成空面板；对照臂：`error !== null` 时同一组合下 `-status` 不渲（错误块单独承载成因） | 参数化迭代（2 形态对拍） |
| `ContractPanel` 生成失败不改写状态词（AC-10 / D18「两成因不可混说」） | 异常 | `check` 已成功落定（零差异）且 `archi_api_generate` reject → `-error data-source="generate"` 在顶部、`-status` 仍为 check 派生的「生成物最新，判据 0 条」（MUST NOT 因写盘失败改写成「解析中…」或声称「全部失败」）；两成因分挂不同 `data-source` | 单次 |
| `ContractPanel` `incremental` 有生成按钮（AC-11 / U1） | 正向 | `mode = 'incremental'` → `-generate` 在场且可用；`-check` 同时在场；零 `-readonly` | 单次 |
| `ContractPanel` `full` 零生成入口 + 一行提示（AC-11 / U1 / spec「形态沿 `archi-gate-readonly`」） | 异常 | `mode = 'full'` → `-generate` **不在场**；`-readonly` 在场、文案含「生成需开 change」与 `pnpm api:export`；点全局面板零 `archi_api_generate` 调用 | 单次 |
| `ContractPanel` 生成链路全栈（AC-11 / D17） | 组合迭代 | `check = {mismatched:2, missing:1, orphaned:4}` → 点 `-generate` → `StandardDialog` 正文含「将写入 3 份（不符 2 + 缺失 1）」「保留 4 份孤儿生成物不做处理」「写盘非原子」；点「提交」→ `archi_api_generate` 恰一次 → toast 文案含 `written` / `unchanged` 计数 → `archi_api_check` 再跑恰一次（载荷同 `root`）；dialog 关闭、`generating` 复位 | 组合迭代（对话框文案 3 断言 × 调用序 3 断言） |
| `ContractPanel` 生成期间禁用与零二次 invoke（AC-11 / 边界） | 边界 | `archi_api_generate` 挂起未回 → `-generate` 与 `-check` 均禁用；连点两次 → `archi_api_generate` 仍恰一次 | 单次 |
| `ContractPanel` 生成成功后差异刷新（AC-11 / AC-2 的界面侧） | 正向 | 生成 resolve 后 `archi_api_check` 返回三列全空 → 面板转「生成物最新，判据 0 条」；`-generate` 重新可用 | 单次 |
| `ContractPanel` 头部 meta 呈现（AC-9 / D18） | 正向 | `meta = { toolVersion: '0.6.40', commonDigest: 'sha256:' + 'a'.repeat(64) }` → 头部文本含 `0.6.40` 与 `commonDigest` 前 12 位 hex（**不渲 64 位全量**）；`meta` 缺席时头部不抛错 | 单次 |
| `ContractPanel` `root = null`（边界） | 边界 | `root = null` → `-status` 文案含「无工作区根」、零 invoke、零 `-item`、`-generate` 不可用 | 单次 |
| `ContractPanel` 越签名脏值（边界 / list 与 str 映射） | 边界 | `findings = []` 正常；`findings` 1000 条 → 渲出 1000 条 `data-axis="findings"` 条目、不抛错；`check.mismatched` 含超长（>1000 chars）/ 含 emoji 的路径串 → 条目文本逐字保留、`FileViewer` 载荷不被截断 | 参数化迭代（4 形态） |
| `ContractPanel` 零第二份取数（AC-12 / spec「切视图不重取三命令」） | 异常 | 挂载后只出现 `archi_api_check`（+ 按需 `read_file`，+ 确认时的 `archi_api_generate`）三类命令；`archi_graph` / `archi_module` / `archi_drift` **零调用**；`include_str!("contract-panel.tsx")` 零 `useArchiGraph` / `useArchiDrift` / `useArchiModule` / 零 `setInterval` / `watchSubscribe` / 零类别常量表 | 单次（行为 + 源码自证） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core` 的 `invoke` | **进程边界 mock**：`vi.hoisted` + `vi.mock` 替身，按命令名分发 `archi_api_check` / `archi_api_generate` / `read_file` 三类 fixture；记录调用序列与载荷 | 全部用例 |
| `useArchiApiCheck` | **不 mock**：真实 hook（三触发与失败清旧即被测链路的一环；与 hook 章节的分工是「hook 断言生命周期、面板断言渲染与调用序」） | 全部用例 |
| `StandardDialog` / `ui/button` / `FileViewer` / `sonner` 的 `toast` | **不 mock**：真实组件渲染与真实 `toast` 调用（确认文案与 toast 内容是被测面）；`read_file` 经替身 `invoke` 观测（`FileViewer` 真实挂载、按需触发） | 生成链路 / 打开行 |
| `React` 调度（`render` / `act` / `waitFor`） | **不 mock**：真实 React 19 | 全部用例 |
| 时钟 / 子进程 / 网络 | **零依赖**（面板零定时器、零 spawn） | —— |

---

### `packages/desktop/src/views/architecture/components/file-tree.tsx` -> `packages/desktop/src/views/architecture/components/file-tree.test.tsx`

**改写既有测试文件**——`TreeMode` 扩第三值 + `TreeModeSwitch` 第三钮；文件 / 元素两视图的既有断言**逐字保留**。沿既有零 mock 定式（纯 props 驱动，断言真实 DOM）。

#### 待测功能

<!-- design.md「公共函数 / API」表对本文件零行：本变更只把 `TreeMode` 联合类型扩一值、`TreeModeSwitch` 的选项表加一行（D20）；`FileTree` 与 `TreeModeSwitch` 的签名与语义零变化。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `TreeModeSwitch` 三钮齐备（AC-14 / spec「分段控件三钮」） | 正向 | `mode = 'file'` → 容器 `archi-tree-mode` 内恰三钮、`data-mode` 为 `file` / `element` / `contract`；文案分别为「文件」/「元素」/「契约」；`file` 钮 `aria-pressed = true`、另两钮 `false` | 单次 |
| `TreeModeSwitch` 第三钮回调（AC-14 / spec「第三钮回调」） | 正向 | 点 `data-mode="contract"` → `onChange('contract')` 恰一次；点 `element` / `file` 各一次 `onChange` 逐值命中 | 参数化迭代（3 钮) |
| `TreeModeSwitch` 选中态随 `mode` 变化（AC-14） | 正向 | `rerender({ mode: 'contract' })` → 三钮 `aria-pressed` 按值翻转；`onChange` 零调用（受控组件零内部 state） | 参数化迭代（3 模式） |
| `TreeModeSwitch` 缺省仍 `file`（AC-14 / spec「缺省仍 `file`」） | 边界 | 不传 `mode` 的等价形态（上层缺省值）下缺省态为 `file`（由画布章节承载默认值断言，此处断言受控组件对 `mode = 'file'` 的呈现）；`data-mode` 三值各恰一钮（零重复、零额外 `data-mode`） | 单次 |
| `TreeModeSwitch` 越签名 `mode` 脏值（异常 / Enum N+1） | 异常 | `mode = 'tree'`（越签名）/ `''` / `undefined` → 三钮 `aria-pressed` 全 `false`（受控组件不猜模式）、渲染不抛错、点任一钮仍 `onChange` 逐值命中；反向判据：MUST NOT 把脏值兜底成 `file`（兜底会把「未知视图」伪装成「文件视图」） | 参数化迭代（3 脏值） |
| 文件 / 元素两视图既有断言逐字保留（AC-14 / 回归登记） | 边界 | 六值标记 / 展开态 / 聚焦反查 / 空树 / 图例 / 归属行点击等既有断言**逐字保留、零改动**；`TreeModeSwitch` 两钮的既有断言语义不变（只补第三钮断言） | 单次（回归登记） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| props 数据 | **零 mock**：纯 props 驱动（组件即被测对象） | 全部用例 |
| `ui/scroll-area` / `ui/button` / `lucide-react` | **不 mock**：真实渲染（控件可达性属性是被测面） | 全部用例 |
| `Element.prototype.scrollIntoView` | **实例级最小桩**（既有滚动断言用，`afterEach` 复原；不进 `test-setup.ts` 原型） | 既有聚焦反查行 |
| 网络 / 子进程 / 时钟 | **零依赖**（组件零 invoke） | —— |

---

### `packages/desktop/src/views/architecture/components/architecture-canvas.tsx` -> `packages/desktop/src/views/architecture/components/architecture-canvas.test.tsx`

**改写既有测试文件**——左栏加 `contract` 臂、`apiFindings` 下传、`mode` 传入面板；既有画布 / 抽屉 / 标记 / 停摆断言**逐字保留**。`invokeMock` 需补 `archi_api_check` / `archi_api_generate` 分支。本节同时承载跨模块组合用例（链路入口是本文件）。

#### 待测功能

<!-- design.md「公共函数 / API」表对本文件零行：`CanvasBody` 左栏加第三臂 + `apiFindings` 透传，列在「修改文件」表与「架构组件」表（D20），不是 API 行。被测面是该组件的左栏分支、`apiFindings` 下传与既有渲染分支的零回归。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 第三钮切到契约视图（AC-14 / spec「三视图各自渲染」） | 正向 | 健康图 + `archi_drift` 报告 → 点 `data-mode="contract"` → `archi-contract-panel` 在场、`archi-file-tree` / `archi-element-tree` 均不在场；画布与抽屉仍同屏（三段布局齐备） | 单次 |
| 切到契约视图不重取三既有命令（AC-12 / spec「切到契约视图不触发重取数」） | 异常 | 在 file / element / contract 间来回切 6 次 → `archi_graph` / `archi_module` / `archi_drift` / `archi_file_tree` 调用计数**全部不变**；`archi_api_check` 只在首次进 contract 时一次、再进不重复（面板卸载重挂的等价形态在用例内钉死：以实际调用计数为准并逐字注释） | 单次 |
| `apiFindings` 下传到面板与抽屉（AC-9 / AC-13 / D20） | 正向 | `archi_drift` 报告含 2 条 `apiFindings`（`category = signature` / `entry`）→ 切到契约视图：`-axis-findings` 计数为 2、两枚 chip 在场；抽屉契约段列出属于当前元素的判据（`element === fqn` 过滤，零判据元素渲「本元素无漂移」） | 组合迭代（面板 + 抽屉两目标） |
| 组合：判据条目聚焦 → 画布圈定 + 抽屉取数（AC-9 / AC-13 / 双向联动） | 正向 | 判据 `element = 'App.Auth'` 条目点击 → 抽屉 `archi-drawer-fqn` 变 `App.Auth`、`archi_module` 恰一次且载荷 `{ root, element: 'App.Auth' }`；`ReactFlow` 节点集合切到该元素 1-hop 邻域 | 组合迭代（链路：panel → focus → canvas + drawer） |
| 组合：契约段路径行 → `read_file`（AC-13） | 正向 | 抽屉契约段点「查看」（契约路径）→ `read_file` 恰一次、载荷键为 `{ root, path }` 且 `path` 为该 repo 相对串 | 组合迭代（链路：drawer → FileViewer → read_file） |
| `full` 模式无生成入口（AC-11 / spec「能力差异表」） | 异常 | `mode = 'full'` → 进契约视图：`-generate` 不在场、`-readonly` 在场；全会话 `archi_api_generate` 零调用 | 单次 |
| `incremental` 模式生成可用（AC-11） | 正向 | `mode = 'incremental'` → `-generate` 在场且可用（具体交互归面板章节） | 单次 |
| 结构错停摆态左栏零契约（AC-10 / 既有停摆分支零改动） | 异常 | `diagnostics` 非空 → `archi-error-panel` 在场；`archi-contract-panel` / `archi-file-tree` / `archi-element-tree` **均零**；`archi-tree-mode` 不挂载；`archi_api_check` 零调用 | 单次 |
| 空模型分支的左栏（AC-10 边界 / 既有早返回） | 边界 | `graph.elements` 为空且零诊断 → `archi-canvas-empty` 在场；`archi-contract-panel` 不挂载；`archi_api_check` 零调用（沿用既有早返回分支，零改动） | 单次 |
| 报告缺席时 `apiFindings` 为空数组（边界） | 边界 | `archi_drift` reject（`report = null`）→ 契约视图面板判据区计数 0、零 chip、不抛错；`archi-contract-error`（检查失败）与 `archi-drift-error` 两行内提示并存且互不掩盖 | 单次 |
| 既有画布 / 抽屉 / 标记断言零变化（AC-19 / 回归登记） | 边界 | `toNodes` / `toEdges` / `neighborhoodPositions` / `defaultFocus` / `ArchiNodeView` / `StructureErrorPanel` / 抽屉接线 / `driftMarks` 合并的既有断言逐字保留（节点集合、边 id 与 style、节点坐标、`ReactFlow` 选项、`archi-canvas-loading` / `-error` / `-empty` 三态、`mode` 差异与改档重取）；文件 / 元素两视图断言不变 | 单次（回归登记） |
| 左栏宽度随模式（AC-14 / D20 的呈现契约） | 边界 | `contract` 模式栏容器含 `w-80`；`element` 模式含 `w-72`；模式切换后类名随之变化（三模式各断言一次） | 参数化迭代（3 模式） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core` 的 `invoke` | **进程边界 mock**：按命令名分发 `archi_graph` / `archi_module` / `archi_set_gate` / `archi_drift` / `archi_file_tree` / **`archi_api_check`** / **`archi_api_generate`** / `read_file`（既有装置扩三条分发臂） | 全部用例 |
| `@xyflow/react` | **观测垫片**（环境缺口而非业务 mock）：记录 `ReactFlow` 入参后交真实组件渲染（既有 `flowProps` 手法 + 既有 `ResizeObserver` / `offsetWidth` / `DOMMatrixReadOnly` / `getBBox` 垫片） | 画布断言行 |
| `ContractPanel` / `ElementDrawer` / `FileTree` / `ElementTree` / `useArchiApiCheck` / `useArchiDrift` | **不 mock**：真实组件 + 真实 hook 组合（三段布局与下传链路即被测语义；替身会让「下传 `apiFindings`」自证） | 全部用例 |
| `lib/marks` / `lib/drift-marks` / `lib/fqn` / `structure-error-panel` | **不 mock**：真实调用（零改动面，回归登记依赖真实标记派生） | 画布 / 标记行 |
| 时钟 / 网络 / 子进程 | **零依赖**（组件层零 spawn；`vi.useFakeTimers` 仅既有零轮询臂使用） | —— |

---

### `packages/desktop/src/views/architecture/components/element-drawer.tsx` -> `packages/desktop/src/views/architecture/components/element-drawer.test.tsx`

**改写既有测试文件**——`ElementDrawerProps` 加 `findings`，概览与文档之间插契约段（`api.status` + 三路径行 + 本元素判据 + 空态）；既有抽屉断言**逐字保留**。design 事实校准 11 把本文件登记为 `tsc --noEmit` 的**编译闭合面**：实现相位只做机械改动（每个 `<ElementDrawer>` 调用补 `findings={[]}`、共享 `archi_module` mock 补 `api` 投影），**零用例新增、零断言删除、零断言语义改写**；下述契约段用例本体归测试相位。沿既有零 mock 定式（纯 props + `invoke` 进程边界替身）。

#### 待测功能

<!-- design.md「公共函数 / API」表对本文件零行：`ContractSection` 是私有组件，`ElementDrawerProps` 在「类型定义」表（追加 `findings`）；本变更的呈现契约归 D19 / AC-13。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 契约段插入位置（AC-13 / D19） | 正向 | `module.detail.api.status = 'declared'` 且 `root !== null` → 抽屉内段落序为「概览 → 契约 → 文档」（以段标题与既有段 testid 的 DOM 序断言）；契约段标题为 `h3`（与文档段同形） | 单次 |
| `declared` 三路径行可打开（AC-13 / spec「三路径行」） | 正向 | `api = { status: 'declared', contractPath: '.dev-team/architecture/api/Core/App.Auth.md', schemaPath: '.../App.Auth.schema.json', generatedPath: 'src/auth.i.ts' }` → 三行各带 `data-testid="archi-contract-path"`；点「查看」→ `read_file` 恰一次、载荷 `path` 为对应 repo 相对串（三次点击共三次） | 参数化迭代（3 路径） |
| 路径为 `null` 的行禁用（AC-13 / 边界） | 边界 | `schemaPath = null`（其余非空）→ 该行「查看」`disabled`、点击零 `read_file`；另两行照常可点 | 单次 |
| 本元素判据逐条列出（AC-13 / spec「`element === fqn` 过滤」） | 正向 | `findings` 含 `element = 'App.Auth'` 两条与 `element = 'App.Billing'` 一条 → 抽屉只列前者两条（`kind` + `message` 文本逐字）；后者不出现 | 单次 |
| 零判据态（AC-13 / spec「本元素无漂移」） | 边界 | `findings` 全属其他元素 → `archi-contract-drawer-empty` 在场、文案「本元素无漂移」；零判据条目 | 单次 |
| `api.status = 'none'` 只渲一行（AC-13 / D19） | 边界 | `status = 'none'`、三路径全 `null` → 渲一行 `archi-contract-drawer-empty`（文案含「无契约」与 `api = none` / 契约文件不在案两成因）；三路径行**均不渲** | 单次 |
| 零新增 invoke（AC-13 / spec「零新增 invoke」） | 异常 | 打开抽屉并交互契约段 → invoke 序列只含 `archi_module`（既有）与 `read_file`（点击触发）；`archi_api_check` / `archi_api_generate` / `archi_drift` **零调用**；`include_str!("element-drawer.tsx")` 零 `archiApiCheck` 等新命令引用 | 单次（行为 + 源码自证） |
| 边界：`findings` 空 / 超大 / `element` 脏值（边界 / list 与 str 映射） | 边界 | ①`findings = []` → 「本元素无漂移」②1000 条本元素判据 → 全量列出、不抛错③`element = ''` 与 `element` 含 emoji 的判据不误配到本元素（严格 `===` 判据） | 参数化迭代（3 形态） |
| 契约段在 `root = null` / `detail = null` 时不渲（边界） | 边界 | `root = null` 或 `module.detail = null` → 契约段零节点（沿 D19「`root !== null && detail !== null` 才渲」），既有空态照常 | 参数化迭代（2 形态） |
| 既有抽屉断言零变化（AC-19 / 回归登记） | 边界 | 概览字段 / 文档四字段条目 / ADR 按需取 / `incremental` 改档三档 / `full` 只读提示 / 零「相邻报告」区块的既有断言**逐字保留** | 单次（回归登记） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core` 的 `invoke` | **进程边界 mock**：按命令名分发 `archi_module` / `read_file` / `archi_set_gate`（既有装置零扩；新命令**不得**出现在分发臂内——出现即说明新增了 invoke） | 全部用例 |
| `findings` / `module.detail.api` | **不 mock**：纯 props 数据（父级同一份报告，零新取数） | 全部用例 |
| `FileViewer` / `ui/button` / `StandardDialog` | **不 mock**：真实组件（「查看」按钮与对话框是被测面） | 路径行 / 改档行 |
| 时钟 / 网络 / 子进程 | **零依赖** | —— |

---

### `packages/desktop/src/types/generated/bindings.ts` + `bindings.zod.ts` -> `packages/desktop/src/lib/ipc-contract.test.ts`

**核对型映射**（产物禁手改，design「测试文件」表标为「核对」）——该文件的全量键集断言与三表逐键相等断言**随 `bindings:export` 自动覆盖**两条新命令与三个新 DTO，预期**零手改即通过**；本节只登记「确需随动」的核对面。

#### 待测功能

<!-- design.md「公共函数 / API」表对本文件零行（生成物）。条目改列 design 修改文件表的「再生成入库」行与 AC-17；断言对象是 ipc-contract.test.ts 的三表全量相等与类型推断面。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 三表键集与 `commands` 完全一致（AC-17 / 自动覆盖） | 正向 | `Object.keys(argsSchemas)` / `resultSchemas` / `channelArgs` 与 `Object.keys(commands)` 逐键相等 —— 两条新命令随再生成自动入表，**零手改**；任一表缺一条即红 | 单次 |
| 两新命令的 args schema 与函数元数一致（AC-3 / AC-17） | 正向 | `argsSchemas.archiApiCheck` / `.archiApiGenerate` 的 `shape` 键恰 `['root']`、值类型为 `z.string()`；长度与 `commands.archiApiCheck.length` / `.archiApiGenerate.length` 相等 | 参数化迭代（2 命令） |
| 两新命令的 result schema 类型推断（AC-3 / AC-17） | 正向 | `z.infer<resultSchemas.archiApiCheck>` 与 `ArchiApiCheck` 双向可赋值（编译期）；`archiApiGenerate` 同 `ArchiApiWriteOutcome`；`channelArgs` 对两命令为空表（非 Channel 命令） | 参数化迭代（2 命令） |
| 键集差一即红的可判性（AC-17 / 反向判据） | 异常 | 以「副本删一条键」的手法外证断言强度：`Object.keys({ ...argsSchemas, archiApiCheck: undefined })` 与 `Object.keys(commands)` 不再相等 ⇒ 该断言族能捕获「产物漏生成一条」；反向臂：三表各自删不同键均被判出（三表非冗余） | 参数化迭代（3 表） |
| 既有命令零漂移（AC-17 / 回归登记） | 边界 | 既有 51 条命令的三表键与既有 shape 断言**逐字保留**；`channelArgs` 对既有 Channel 命令的键名断言不变 | 单次（回归登记） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core` | **进程边界替身**：既有 `invoke` / `Channel` 双导出替身（`bindings.ts` 模块装载即解构，故两者都必须导出） | 全部用例 |
| 生成产物 | **不 mock**：真实 import `../types/generated/bindings` 与 `.zod`（产物形状即被测对象） | 全部用例 |
| `ipc` / `ipc-drift` | **不 mock**：真实代理走一遍 `invoke` 往返（零漂移判据依赖真实链路） | 回归登记行 |

---

## 不可测试项

以下条目属 design「变更清单」的新增 / 修改文件、design「测试相位承接面」的语料，或属 AC 的判据面，但**无单元测试路径**（成因逐条说明）。它们不进入 `## 单元测试`，进入验收的方式落在「静态检索 / 机械门禁 / 命令 + 留痕 / 编译期 / 零 diff 审查」五类，与 proposal「测试文件」表及 `tasks.md` 的自查行一致。

| 文件 / 模块 / 判据 | 类别 | 不可测试原因 | 验收方式 |
|---|---|---|---|
| `packages/desktop/src-tauri/crates/core/archi/src/lib.rs` | crate 根（出口面） | 无 `lib_test.rs` sidecar；内容为 `pub mod` 与三处 `pub use` 追加，零独立行为面（出口可达性是编译期事实） | AC-4 / AC-6：`cargo check --all-targets` 通过；`grep -n "pub use api::{…}"` 段含 `artifact_check` / `compare` / `findings` / `gating_findings` / `workspace_render` / `ArtifactDiff` 各恰一处；`pub use dto::{…}` 含三 DTO 与 `graph_diagnostics_error` 各恰一处；crate doc 第 5 条无「唯一写盘面在工具 crate」表述 |
| `packages/desktop/src-tauri/crates/infra/archi/src/lib.rs` | crate 根（模块声明 + 头注释） | 无 sidecar；只有 `pub mod api_artifacts;` 一行与头注释订正（`report` / `runner` 仍私有） | AC-5：`grep -n "pub mod api_artifacts"` 恰一处；`grep -n "mod report;\|mod runner;"` 仍为私有形态；crate 头注释含「两处写盘单点」；`cargo check -p archi-runtime --all-targets` 通过 |
| `packages/desktop/src-tauri/crates/tools/archi-api-gen/Cargo.toml` | 配置面（依赖） | 依赖声明与用途注释是构建元数据，无运行时行为；「不被 app / core / infra 依赖」是入边判据（负向图性质） | AC-5：`grep -n "archi-runtime" Cargo.toml` 恰一处且带用途注释；`cargo metadata` 的该 crate `dependencies` 恰含 `archi` / `foundation` / `archi-runtime`；`grep -rn "archi-api-gen" crates/ src/ --include=Cargo.toml` 零入边 |
| `packages/desktop/src-tauri/Cargo.toml` | 配置面（注释行） | 只改 `archi-runtime` 用途注释，依赖条目与 crate 版本行零改动 | AC-5 / AC-18：`git diff --stat` 只含注释行、不含 `version` 与依赖条；`grep -n "api_artifacts::write_artifacts" Cargo.toml` 命中注释一处 |
| `packages/desktop/src-tauri/src/bindings/mod_test.rs`（文件本体） | 测试文件本体（清单随动） | 它本身就是清单守卫（无被测源文件可映射）；其**内容随动**已在 `commands/mod.rs -> bindings/mod_test.rs` 章节逐条登记，此处只登记「文件即测试」 | AC-17：`git diff` 的五处计数与两组清单改动与本章节用例逐条对应；`cargo nextest run -p desktop` 该文件全绿（由 test-execution 承接） |
| `packages/desktop/src/types/generated/bindings.ts` + `bindings.zod.ts`（产物本体） | 生成物 | `bindings:export` 机械产出，零手写面；其**形状**在 `ipc-contract.test.ts` 章节与 `bindings/mod_test.rs` 章节以产物断言覆盖 | AC-17：`pnpm run bindings:export` 后 `git diff --stat src/types/generated/` 只含两条包装 + 三类型 + 两表项；再跑一次幂等零 diff；`pnpm run bindings:zod-check` 通过 |
| `packages/desktop/package.json` | 配置面（版本行） | `version` 一行改动，无逻辑面；脚本内容零变化 | AC-18：`version` = `0.6.40`（相对 `0.6.39` +1 patch）；`scripts` 的 `api:export` / `api:check` / `check` 三段命令串逐字不变；`src-tauri/Cargo.toml` crate 版本行不变 |
| `AGENT.md` + `CLAUDE.md` | 文档口径 | 纯 markdown 散文，零代码路径；两副本「逐字相同」是文本一致性判据 | AC-19：两文件 `## 模块 API 契约` 第 ⑤ 条**逐字相同**且为纳管口径（`grep -n "契约在案元素的判据进 \`status\`、不进 \`passed\`"` 各命中）；`grep -n "首期不影响\|不进 status"` 零命中；`git diff` 差异行全在该节内 |
| `packages/desktop/src-tauri/crates/core/archi/tests/vocabulary_test.rs` | 既有测试文件（零改动面） | 本变更零新增封闭词汇，四面词汇对拍应零 diff；「零改动」不是可断言的行为 | AC-19：`git diff --stat crates/core/archi/tests/vocabulary_test.rs` 为空；`grep -c "ArchiApiCheck\|ArchiApiMeta\|ArchiApiWriteOutcome"` 该文件零命中 |
| `packages/desktop/src/views/architecture/hooks/{use-archi-graph,use-archi-drift,use-archi-module,use-archi-file-tree}.test.ts` 与四个对应 hook | 既有测试文件 + 零改动面 | 四个 hook 零改动（新面板不自取第二份数据）；「切视图不重取」的**跨组件**判据在 `architecture-canvas.test.tsx` 章节，hook 自身无新增行为 | AC-12 / AC-19：`git diff --stat` 八个文件为空；`architecture-canvas.test.tsx` 的调用计数断言全绿 |
| `packages/desktop/src-tauri/crates/core/archi/tests/corpus/drift/expected/*.json`（16 份）与 `.../api-mismatch/models/01-app.toml`（注释面） | 测试语料（design「测试相位承接面」；已移出实现变更清单，事实校准 10） | 期望值资产而非可执行测试：其取值由 `golden_test.rs` 的 `DRIFT_CASES` 登记面与三处硬编码断言承载（断言侧已登记于 `golden_test.rs` 章节）；语料本体的重写是机械步骤，不是单测可驱动的新行为 | AC-7：**test-execution** 以 `DESKTOP_GOLDEN_REWRITE=1` 重写 + `git diff` 逐份确认；16 份 `version` 均 3，`api-mismatch.full.json` 的 `status` 为 `warnings`，其余 15 份除 `version` 外零差异，`01-app.toml` 差异只含注释行；实现相位对 `tests/**` 零读写 |
| AC-2 / AC-4 / AC-5 / AC-6 / AC-15 / AC-19 的跨文件 `grep` 判据（core 零写盘、`main.rs` 零 walker、`infra/archi` 写盘恰两处、`drift` 零 `is_file`、`archi_api` 名字面、`SessionToolset` 零命中） | 静态检索 | 断言对象是跨文件文本出现次数（全局否定式判据），单测内读自身源码只能覆盖单文件；部分已作单文件自证行下沉到各章节 | AC-4 / AC-5 / AC-6 / AC-15：`grep -rn "fs::write\|create_dir_all\|File::create" crates/core/archi/src crates/tools/archi-api-gen/src src/commands/archi/` 零命中；`grep -rn "fs::write\|create_dir_all" crates/infra/archi/src/` 恰命中 `report.rs` 与 `api_artifacts.rs`；`grep -n "is_file" crates/core/archi/src/drift/mod.rs` 零命中；`grep -rn "generated_files_on_disk\|is_generated_module_file" crates/ src/` 只命中 `core/archi/src/api/diff.rs`；`grep -rn "SessionToolset\|toolset" crates/infra/agent/src crates/core/agent/src` 零命中 |
| AC-8 的本仓实测基线（41 条 `apiFindings`：`apiEntryMissing` 28 / `apiMissing` 13；契约在案元素仅 `App.Backend.Infra.ArchiRuntime` 且判据 0 条） | 命令 + 留痕 | 依赖**真仓**真实模型与契约资产，不是可复现的合成夹具；spec 明写「不作为 MUST，登记留痕」 | AC-8：仓库根手工跑一次 `archi_drift`（与 `archi_api_check`）核对 `status` / `passed` 除 `version` 外与改动前逐字段相同，结论写入本 change 的实施记录 |
| AC-18 的门禁命令面（`pnpm -C packages/desktop run check` 四段链 / `cargo nextest run --workspace` 全绿 / 版本双行） | 机械门禁 | 属静态脚本机械步骤（`package.json` 脚本驱动），agent 内不重复执行；本设计不复制测试运行脚本细节 | AC-18：`pnpm -C packages/desktop run check` 退 0（`server:check` → `bindings:export` 零 diff → `api:check` 零 diff → `client:check` 含 `knip` 零新增 ignore）；`cargo nextest run --workspace` 全绿；`git diff --stat` 只含本清单文件集 |
| AC-19 的边界零 diff 面（`api/{check,contract,schema,paths}.rs` / `api/{emit,extract}/**` / `scan/**` / `infra/archi/src/{report,runner}.rs` / `infra/agent/src/sdk/protect/**` / `.dev-team/config.json` / `.dev-team/architecture/{models,decisions,api}/**` / `explore.md` / 画布渲染面 / `lib/**` / `element-tree.tsx` / `structure-error-panel.tsx`） | 零 diff 面 | 无新增测试面（「零改动」不是可断言的运行时行为）；部分路径的行为侧证已在各章节的回归登记行 | AC-19：`git diff --stat` 对上述路径逐条为空（单条命令一次核对）；配置隔离静态面另加 `grep -n "config.json" crates/core/archi/src/api/*.rs src-tauri/src/commands/archi/mod.rs` 零命中（既有 `foundation/src/layout/mod.rs` 为唯一例外） |
| `api-mismatch/models/01-app.toml` 之外的 14 份 case 模型与 `CASES`（解析语料） | 零改动面（测试语料侧） | 本变更的口径只触及 `api-mismatch` 的模型注释（归 test-execution）；其余模型与解析语料登记面零改动，不属实现变更 | AC-7 / AC-19：`git diff --stat crates/core/archi/tests/corpus/**` 只含 `expected/*.json` 16 份与 `api-mismatch/models/01-app.toml`（由 test-execution 重写后核对）；`golden_test.rs` 的解析语料断言全绿 |
| 契约面板 / 抽屉契约段 / 分段控件的真实浏览器视觉（栏宽 `w-80` / `w-72` 的实际像素、chip 换行、`FileViewer` 抽屉层级、`StandardDialog` 焦点陷阱） | 手工验证 | jsdom 无布局引擎与焦点管理：断言只能到「类名 / DOM 结构 / 组件挂载」一层 | AC-9 / AC-11 / AC-13 / AC-14：手工在 `pnpm run dev` 下打开架构页逐项核对（三视图切换、两轴计数、生成二次确认、抽屉契约段） |
| `enabled = false` 状态下面板「不渲成空面板」的**语义**判据（spec 用 MUST NOT 表达） | 半可测 | 呈现面已由 `contract-panel.test.tsx` 章节断言（`-status` 文案 + `missing` 条目在场）；但「空面板」的视觉判定（无区块 / 无区块内空态）属人工判断 | AC-10：断言侧已覆盖文案与条目；MUST 由评审在 dev 环境确认「冷启动不呈空白区块」 |
| 前端 `contract-panel.tsx` 的「零类别常量」与「chip 由数据派生」的**结构**判据 | 半可测 | 行为侧已断言「零计数类别不渲 chip」；「源码零类别常量表」需人工看代码（无前端静态分析器约束） | AC-9：`grep -n "'entry'\|'signature'\|'reexport'\|'nesting'\|'constructor'" src/views/architecture/components/contract-panel.tsx` 零命中；`grep -n "Set(findings.map"` 命中派生单点 |
