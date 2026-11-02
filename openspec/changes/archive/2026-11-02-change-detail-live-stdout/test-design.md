# 测试设计: change-detail-live-stdout

> **变更**: change-detail-live-stdout
> **角色**: test-design-planner（executor）—— 从 `design.md` 推导单元测试范围与公共 API 签名
> **输入**: `proposal.md`（变更范围 / 测试文件表 / AC-1..AC-17 / R-1..R-11）、`design.md`（D1..D19-a（含回溯补正的 **D10-a** 单飞守卫执行细则）/ 架构组件 / 新增与修改文件表 / 公共函数·API 表 26 行 / 类型定义 / 数据模型不变量 / AC 落点 / 待决问题 1..**15**）、`tasks.md`（九阶段与编译面涟漪计数）、七份 delta spec、工作树源码与既有测试盘点
> **被测范围**: design「变更清单 - 新增文件」（2 项）与「修改文件」（19 项）两张子表的全部模块；**删除文件零项**（design 明示「不涉及」——`StepSettle` / `emit_node` / `now_millis` 是搬迁而非删除，`CodeBlockCopyButton` 亦是搬迁而非删除）。另按 design **D17** 追加一处**组合用例落点**（`packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs`：该文件不是本变更的实现文件、源码零 diff，只因「真实 `ProcessStaticCheck` × 真实 `RunGuard` 上抛链」的端到端用例必须落在壳层才可用，故作为 AC-11 的执行落点登记；见「测试布局决策」第 5 条）
> **本变更性质**: 后端形成一条 live 记录链（`begin → append → commit` 同 id）+ 一条上抛链（runner → `OutputRefSink` → `StepEmitter` → `accumulate_node`）+ watch 失败面两分；前端形成一条「watch 信号 → offset 增量读」跟随链与面板三段三态

**测试框架与既有约定（盘点结论）**

| 面 | 框架 / 运行器 | 文件形态 | 断言与替身 |
|---|---|---|---|
| Rust（`packages/desktop/src-tauri`） | `cargo nextest`（`cargo clippy --all-targets` 门禁）；async 用例 `#[tokio::test]`（默认单线程 runtime），同步用例 `#[test]`；用例名中文长句 | 与被测源文件同目录的 `<模块名>_test.rs`，经源文件尾部 `#[cfg(test)] #[path = "…_test.rs"] mod …_test;` 挂载（`exec/src/lib.rs` / `run.rs`、`checks/src/static_check.rs`、`testexec/{mod,runner}.rs`、`core/orchestration/{port,steps,control,walker}.rs`、`infra/watch/src/lib.rs`、`src-tauri/src/commands/changes/mod.rs`、`src-tauri/src/commands/change_flow/mod.rs` 均已如此） | 断言用 `assert!` / `assert_eq!` / `matches!`；**无 mock 库**——进程边界以真实资产组合替代（tempdir、平台 shim 脚本、`tauri::test::mock_app()`、真实 `WorkspaceStores`、`mpsc` + `recv_timeout` 超时上限）；PATH / 进程级环境替换经 `crate::lock_test_env()` 与 `PathPrefix` / `EnvWindow` RAII 串行化 |
| 前端（`packages/desktop/src`） | `vite-plus` 的 test 面（vitest API，`vp test`）：`environment: jsdom`、`include: src/**/*.test.ts?(x)`；覆盖率 lines 90 / branches 85 / functions 90 | 与被测源文件同目录的 `<模块名>.test.ts`（`.test.tsx` 仅组件） | `import { describe, expect, it, vi } from 'vite-plus/test'` + `@testing-library/react`（`render` / `renderHook` / `act` / `waitFor` / `findByTestId`）；`vi.hoisted` + `vi.mock('@tauri-apps/api/core')` 打**进程边界**替身（`invokeMock` 按命令名分发 + `ChannelMock` 可编程 class）；500 ms 防抖窗口用 `vi.useFakeTimers()` + `advanceTimersByTime` |

**用例表列语义**

- **路径类型**：`正向`（契约内输入 / 预期输出）/ `异常`（契约外输入、reject、失败路径、静默面）/ `边界`（空、空串、`null`、超大、恰好阈值、越过阈值、生命周期交错点）。
- **迭代类型**：`单次`（一次驱动即判）/ `参数化迭代`（同一断言逻辑对表驱动数据集逐项执行）/ `组合迭代`（两个以上维度交叉）。
- **`测试条件` 内的「既有…」前缀**：该臂已被既有用例锁定，本设计**只做回归登记、不新增用例**（AC 的「既有用例全绿」判据）；「**新增**」为要在本变更追加的用例。

**待测功能条目口径**：`#### 待测功能` 只列 design「公共函数 / API」表中「所在文件」为本源文件的条目。design 未在该表声明的模块（`run.rs` 的 `drain`、`static_check.rs` 的 `execute`、`testexec/mod.rs` 的 `outcome`、`testexec/runner.rs` 的 `execute_plan`、`steps.rs` 的 `execute`、`walker.rs` 的 `run_tool`、`commands/mod.rs` 的 `all_commands!`、`stdout-panel.tsx` / `detail-drawer.tsx` 两个前端呈现件、`mutation/cargo_mutants.rs` 与 `mutation/stryker.rs`（各自独立章节）、`bindings.ts` 与 `bindings.zod.ts`（各自独立章节）、`change_flow/mod_test.rs` 落点）以 HTML 注释说明「design.md 未声明该文件的公共 API 变更」，并按 archive 先例（`2026-10-27-ipc-zod-runtime-schema`）用一条**显式标注的**行为面条目承接（取自 design「修改文件」表 / 「类型定义」表 / Module Contract / AC 落点），不虚构 design 未有的 API 行；其模块私有件覆盖路径记入「不可测试项」。design「公共函数 / API」表的 26 行逐行落在对应源文件的 `#### 待测功能` 中：`infra/exec/src/lib.rs` 10 行、`orchestration/src/port.rs` 4 行（`OutputRefSink::new` / `OutputRefSink::emit` / `StaticCheckRunner::run` / `TestExecutionRunner::run`）、`orchestration/src/control.rs` 4 行、`infra/watch/src/lib.rs` 1 行（`subscribe`）、`commands/changes/mod.rs` 3 行、`use-exec-output.ts` 1 行、`stdout-text.ts` 2 行、`code-block-copy-button.tsx` 1 行（`CodeBlockCopyButton`）；合计 26 行。

**上抛覆盖的可写性（design D16 / D17 / D18 落地口径，贯穿全篇）**：

- `OutputRefSink::new` 为 **`pub`**（D16）⇒ `crates/infra/checks` 的记录链用例**可以直构回调**，「上抛恰 1 次」「上抛时刻 `.log` 与父目录已在」（AC-11）在 `static_check_test.rs` / `testexec/mod_test.rs` 侧**必须写成用例**，MUST NOT 再以「整体传 `None`」兜底。
- AC-11 的「真实双侧」端到端组合用例落**壳层** `packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs`（D17）；`crates/core/orchestration/Cargo.toml` 零 diff（MUST NOT 加 `checks-runtime` dev-dependency——core → infra 反向边），`walker_test.rs` 的上抛覆盖一律用**可编程 mock runner**（在 `run` 内 `emit`）。
- `LiveSink::new` 保持 **`pub(crate)`**（D18）：`run_test.rs` 是同 crate 子模块可直构；`checks` 侧只能经公共路径 `LiveExec::sink()` 取实例——该不对称是设计裁定，不是缺口。
- `testexec/runner.rs` 的 `Reject` 臂可达性（design 待决问题 13）：`line_budget()` 在 `#[cfg(not(windows))]` 返回 `None` ⇒ unix 上 `execute_plan` 恒 `Keep`，故「`Reject` 臂 `on_command` 零调用」的**端到端**断言只在 Windows 上成立；非 Windows 归宿为 `decide_argv` 直测（同 crate 可见、`budget` 是显式入参，传 `Some(0)` 即可逼出 `Reject`）。`plan.id` 由 `derive_plan_id(root, framework)` 派生，「含 emoji 的 plan id」须以手工构造 `TestPlan` 提供（沿 `runner_test.rs` 既有 `unsafe_plan` / `filler` fixture 同型先例）。

**同臂单飞守卫的可写性（design D10-a / 待决问题 15 落地口径，贯穿前端章节）**：

- design 的回溯补正把 `useExecOutput` 的 `readDelta` 从「只裁决 `chunk === null` / `text === ''`」扩为**三枚状态位的单飞守卫**（`pendingRef` 在飞标记 + `queuedRef` 排队标记 + 结算后清位 + 至多一拍尾读）。这不是可选的健壮性增量：无守卫时同臂两次在飞读共享**同一** `offsetRef`（后端 `read_exec_output_delta_in` 是整文件 `std::fs::read`，500 ms trailing 防抖只合并**信号**、不覆盖在飞时长）⇒ 同段文本被 `setText(prev => prev + chunk.text)` 追加两遍（破 AC-4「连续增量拼接 == 全量」）、两次 `nextOffset` 同值（破 AC-6「offset 推进」的可断言性）。
- 因此 `use-exec-output.test.ts` 章节的用例表**必须**含 design 待决问题 15 点名的**三条「同臂双读」臂**（用例名统一带「同臂双读」前缀，便于回溯追到 D10-a）：① 在飞折叠为至多一拍尾读；② 尾读有界（队列长度恒 ≤ 1）；③ reject 后守卫不卡死。见该章节用例表的末尾三行。
- 三臂**全部可写**，不属竞态不可测：既有装置已具备所需的两件——`makeGate(times)` / `gateAnswer(..)` 把指定次数的 `read_exec_output_delta` 应答悬停在「在飞」（`use-exec-output.test.ts:88-120`，现用于四条「迟到应答」用例），`vi.useFakeTimers()` + `advanceSignalWindow(500)`（`:174`）推进防抖窗口。合起来即可精确构造「一拍在飞 + n 次信号到达」，无需真实 IPC 时序、无需新增装置。
- MUST NOT 以「同臂叠读属竞态、不可稳定构造」为由省略任一臂（tasks.md 的裁定 4 逐字禁止）；若某臂判定不可写，MUST 在本文件留痕并说明替代判据，MUST NOT 静默删臂。当前裁定：三臂全部可写，无替代判据。

**覆盖口径（design D19 / D19-a 落地口径，只影响前端用例来源）**：

- `components/ui/code-block-copy-button.tsx` 是 `CodeBlockCopyButton` 的**逐字搬迁**（零行为变更），唯一生产消费方为 `stdout-panel.tsx` ⇒ 其测试路径是 `stdout-panel.test.tsx` 的命令头行用例组，**不新建测试文件**（见「测试布局决策」第 7 条）。
- `stdout-panel.tsx` 的 import 源 MUST 指向窄模块，MUST NOT 指向 `@/components/ui/code-block` barrel——后者顶部两条静态再导出会把 shiki 高亮树拉回 import 图与覆盖分母（D19 的判据）。`code-block.tsx` 本变更**零测试目标**，理由见「不可测试项」。
- 覆盖达标是 test-execution 相位读 `coverage-summary.json` 的机械判定：本节**不引入** `coverage.exclude` / `coverage.include` / 阈值改动（那些文件 MUST 零 diff），也不为 shiki 高亮链补测试、不删 `code-block.tsx` 的既有再导出。

**Mock 纪律**（贯穿全篇）：只 mock **进程边界**——Tauri IPC（`invoke` / `Channel`）、jsdom 的运行环境能力（滚动三指标、`navigator.clipboard`）、真实文件系统与子进程（后者用真实 shim 脚本而非替身）、外部时钟（不 mock：`now_millis` 无时钟注入点，见不可测试项）。`crates/infra/exec` / `checks` / `orchestration` / `watch` 之间的 crate 边界**一律真实组合**（core 与 infra 互不 mock）；前端 `src/lib/ipc.ts`、`ipc-drift.ts`、`src/hooks/use-file-watch-signal.ts`、生成物 `bindings` / `bindings.zod`、`components/ui/{button,code-block-copy-button}`、以及被测件自身一律真实组合——mock 被测件等于让断言自证。不为测试新增导出（AGENT.md 规则 5 与 `knip` 的 `Unused exports` 判据）。

**测试布局决策（源文件 → 测试文件的映射依据）**：

1. **Rust 新增/修改模块沿用既有同目录 `*_test.rs`**（无一处新建测试文件）：`lib.rs` / `run.rs` / `static_check.rs` / `testexec/{mod,runner}.rs` / `port.rs` / `steps.rs` / `control.rs` / `walker.rs` / `watch/lib.rs` / `commands/changes/mod.rs` 的测试文件全部已在盘上并由源文件尾部 `#[path]` 挂载。
2. **唯一新建前端测试文件**是 `flow/stdout-text.test.ts`：`stdout-text.ts` 是 design 新增的纯判据模块（D9，待决问题 3），既无既有测试可追加，也不能靠组件内部私有函数直测（knip 判 unused）。co-located 命名即仓库惯例（`flow/metrics.test.ts` / `flow/run-state.test.ts` / `flow/layout.test.ts` 同型）。另一个新增源文件 `components/ui/code-block-copy-button.tsx` **不新建测试文件**（见第 7 条）。
3. **`detail-drawer.tsx` 的 AC-5 组合用例**挂在 `detail-drawer.test.tsx` 章节，但**执行文件是页面级 `src/views/changes/change-detail-view.test.tsx`**（该链路的发起方是页面装配 + 图节点点击，两者均不在被测模块表内——`change-flow-graph.tsx` 与 `change-detail-view.tsx` 本变更零 diff）。章节内已标注该行的执行落点。
4. **`packages/desktop/src-tauri/src/commands/mod.rs`（`all_commands!` 登记）** 无自己的 `mod_test.rs`，其登记面判据一直在 `packages/desktop/src-tauri/src/bindings/mod_test.rs`（`COMMAND_NAMES` / `COMMAND_WRAPPERS` 列表 + 总数锚 45 → 48），故映射到该文件。
5. **AC-11 的「真实双侧」组合用例落壳层 `packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs`**（design D17）：该 crate 同时依赖 `orchestration` 与 `checks-runtime`，且已有同型先例 `组合根装配_staticcheck步默认recorded落单一log并回填引用`。**入口模块判定**：该链路的发起方是壳层组合根（`ChangeFlowControl::begin_run` → `guard.emitter()` → `OutputRefSink::new(..)` → 真实 `ProcessStaticCheck::run(.., Some(sink))`），既不是 walker（`walk_run` 需要假 runner 注入）也不是 checks（checks 侧看不到 `RunGuard`），故用例按「链路最上层调用方」挂在该文件的 per-file 章节。该文件**源码零 diff**——只有测试文件增例，故在「验收范围」的 AC-11 行与「不可测试项」中双向留痕。
6. **生成物路径以盘上实况为准**：`bindings.ts` / `bindings.zod.ts` 位于 `packages/desktop/src/types/generated/`（不是 `src-tauri/src/types/generated/`），其前端契约等价面测试为 `packages/desktop/src/lib/ipc-contract.test.ts`；`src-tauri/src/bindings/mod_test.rs` 承担逐字面锚（命令名 / 包装名 / 总数）。
7. **`components/ui/code-block-copy-button.tsx` 不新建测试文件**（design D19 / 待决问题 14）：该窄模块是 `CodeBlockCopyButton` 从 `code-block.tsx` 的**逐字搬迁**（零行为变更），唯一生产消费方是 `stdout-panel.tsx`（`import { CodeBlockCopyButton } from '@/components/ui/code-block-copy-button'`）。因此它的测试路径就是 `stdout-panel.test.tsx` 的命令头行用例组——复制按钮的点击 → `navigator.clipboard.writeText` 全文、`clipboard` reject 不炸图两条断言**照原样保留**，无需为搬迁新增用例（那会是为分母写测试）。`code-block.tsx` 本变更零测试覆盖目标：它**离开 import 图**正是 D19 的判据，为它（及其 shim 树）补测试被 design 明令否决。
8. **覆盖口径不在测试设计里调参**（design D19 / tasks.md 阶段七自查）：`vite.config.ts`、`knip.json`、`stryker.config.json`、`openspec/config.json` 阈值四文件 MUST 零 diff。覆盖率是否达标是 test-execution 相位读 `coverage-summary.json` 的机械判定，本节只声明用例来源，MUST NOT 在测试设计里引入 `coverage.exclude` / `coverage.include` 之类口径手段。

---

## 验收范围

<!-- 测试区域：Rust 侧 `packages/desktop/src-tauri/crates/**/src/*_test.rs` 与 `packages/desktop/src-tauri/src/**/mod_test.rs`（与被测源文件同目录，经 `#[path]` 挂载；运行器 `cargo nextest`）；
     前端侧 `packages/desktop/src/**/*.test.ts(x)`（与源文件同目录共置；运行器 `vp test`，jsdom）。本变更新建测试文件仅 1 个：`packages/desktop/src/views/changes/flow/stdout-text.test.ts`；其余全部为既有测试文件的用例追加 / 断言改写（`code-block-copy-button.tsx` 无独立测试文件——唯一生产消费方 `stdout-panel.tsx` 的用例即在真实渲染路径上覆盖它）。**AC-11 的组合用例落 `packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs`**（design D17；该文件源码零 diff，仅测试文件增例）。 -->

| AC ID | 验收条件 | 被测文件或模块 |
|-------|------------------|----------------|
| AC-1 | 运行中即可读：进程未结束时 `<report_dir>/exec-outputs/<exec_id>.log` 已存在且内容随写入单调增长（后读 ⊇ 前读；收口整体替换为权威内容） | `packages/desktop/src-tauri/crates/infra/exec/src/lib.rs` → `packages/desktop/src-tauri/crates/infra/exec/src/lib_test.rs`；`packages/desktop/src-tauri/crates/infra/exec/src/run.rs` → `packages/desktop/src-tauri/crates/infra/exec/src/run_test.rs`；`packages/desktop/src-tauri/crates/infra/checks/src/static_check.rs` → `packages/desktop/src-tauri/crates/infra/checks/src/static_check_test.rs`；`packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod.rs` → `packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod_test.rs` |
| AC-2 | 同 id 收口：running 拍 `output_ref` 与终态拍相等；`commit` 后 `.log` == 权威内容（live 交错流被覆盖） | `packages/desktop/src-tauri/crates/infra/exec/src/lib.rs` → `packages/desktop/src-tauri/crates/infra/exec/src/lib_test.rs`；`packages/desktop/src-tauri/crates/infra/checks/src/static_check.rs` → `packages/desktop/src-tauri/crates/infra/checks/src/static_check_test.rs`；`packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod.rs` → `packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod_test.rs`；`packages/desktop/src-tauri/crates/core/orchestration/src/control.rs` → `packages/desktop/src-tauri/crates/core/orchestration/src/control_test.rs`；`packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` → `packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs` |
| AC-3 | 命令可查：`<exec_id>.cmd` 与 `.log` 同目录；static-check 逐字 `{static_analysis} 2>&1`；test-exec 每 plan 一段 `=== plan <id> · <framework> ===` + 展开命令；老记录无 sidecar → `None` | `packages/desktop/src-tauri/crates/infra/exec/src/lib.rs` → `packages/desktop/src-tauri/crates/infra/exec/src/lib_test.rs`；`packages/desktop/src-tauri/crates/infra/checks/src/static_check.rs` → `packages/desktop/src-tauri/crates/infra/checks/src/static_check_test.rs`；`packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod.rs` → `packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod_test.rs`；`packages/desktop/src-tauri/crates/infra/checks/src/testexec/runner.rs` → `packages/desktop/src-tauri/crates/infra/checks/src/testexec/runner_test.rs`；`packages/desktop/src-tauri/src/commands/changes/mod.rs` → `packages/desktop/src-tauri/src/commands/changes/mod_test.rs` |
| AC-4 | 增量读口径：`from_offset = 0` 与整读逐字相等；连续增量拼接 == 全量；`next_offset` 单调不减且不超长度；不产出半 UTF-8 字符。**前端侧的「拼接 == 全量」由 D10-a 单飞守卫保证**（无它则同段追加两遍），对应 `use-exec-output.test.ts` 的「同臂双读」两行 | `packages/desktop/src-tauri/crates/infra/exec/src/lib.rs` → `packages/desktop/src-tauri/crates/infra/exec/src/lib_test.rs`；`packages/desktop/src-tauri/src/commands/changes/mod.rs` → `packages/desktop/src-tauri/src/commands/changes/mod_test.rs`；`packages/desktop/src/views/changes/hooks/use-exec-output.ts` → `packages/desktop/src/views/changes/hooks/use-exec-output.test.ts`（同臂双读） |
| AC-5 | 点击判据零改而可用：运行中（`status=running`）且携 `output_ref` 的脚本步节点可点击并呈 stdout 面板；`change-flow-graph.tsx` 零 diff | `packages/desktop/src-tauri/crates/core/orchestration/src/port.rs` → `packages/desktop/src-tauri/crates/core/orchestration/src/port_test.rs`；`packages/desktop/src-tauri/crates/core/orchestration/src/steps.rs` → `packages/desktop/src-tauri/crates/core/orchestration/src/steps_test.rs`；`packages/desktop/src-tauri/crates/core/orchestration/src/control.rs` → `packages/desktop/src-tauri/crates/core/orchestration/src/control_test.rs`；`packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` → `packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs`；`packages/desktop/src/views/changes/flow/detail-drawer.tsx` → `packages/desktop/src/views/changes/flow/detail-drawer.test.tsx`（AC-5 组合行的执行落点为页面级 change-detail-view 测试文件，用例登记在本行 detail-drawer 映射的 per-file 章节内） |
| AC-6 | 运行中跟随：`status === 'running'` 时经 `ipc.readExecOutputDelta` 取增量、信号到达后再取且 offset 推进；非运行态零 `watch_subscribe`。**offset 推进的可断言性以 D10-a 单飞守卫为前提**（无守卫则两拍 `nextOffset` 同值），对应「同臂双读 · 在飞折叠」与「尾读有界」两行 | `packages/desktop/src/views/changes/hooks/use-exec-output.ts` → `packages/desktop/src/views/changes/hooks/use-exec-output.test.ts`；`packages/desktop/src-tauri/src/commands/changes/mod.rs` → `packages/desktop/src-tauri/src/commands/changes/mod_test.rs` |
| AC-7 | 终态收敛：`status` 转非 running ⇒ `watch_unsubscribe` 恰 1 次 + offset 归零整读恰 1 次；权威内容重排后文本与整读一致 | `packages/desktop/src/views/changes/hooks/use-exec-output.ts` → `packages/desktop/src/views/changes/hooks/use-exec-output.test.ts` |
| AC-8 | 三态与 `\r`：运行中 0 字节呈「运行中，等待输出…」；非运行且无记录呈「（无输出记录）」；invoke reject 呈错误态；含 `\r` 的输出不堆残行 | `packages/desktop/src/views/changes/flow/stdout-panel.tsx` → `packages/desktop/src/views/changes/flow/stdout-panel.test.tsx`；`packages/desktop/src/views/changes/flow/stdout-text.ts` → `packages/desktop/src/views/changes/flow/stdout-text.test.ts` |
| AC-9 | 命令头行与复制：面板最上方多行原样显示命令；老记录呈「（命令未记录）」；复制按钮可用且不阻断内容面 | `packages/desktop/src/views/changes/flow/stdout-panel.tsx` → `packages/desktop/src/views/changes/flow/stdout-panel.test.tsx`；`packages/desktop/src/views/changes/hooks/use-exec-output.ts` → `packages/desktop/src/views/changes/hooks/use-exec-output.test.ts`；`packages/desktop/src/components/ui/code-block-copy-button.tsx` → `packages/desktop/src/views/changes/flow/stdout-panel.test.tsx`（窄模块的唯一生产消费方即 `stdout-panel.tsx`，其用例在面板渲染路径上直测复制按钮——**不新建测试文件**，理由见「测试布局决策」第 7 条）；`packages/desktop/src/components/ui/code-block.tsx`（逐字搬迁出 `CodeBlockCopyButton` + 增一条再导出，**无独立测试文件**——见「不可测试项」） |
| AC-10 | watch 显式失败：父目录缺失 ⇒ `Err(WatchError::ParentUnavailable)`；文件缺失而父目录在 ⇒ `Ok` 且创建后出信号；无退避重试代码 | `packages/desktop/src-tauri/crates/infra/watch/src/lib.rs` → `packages/desktop/src-tauri/crates/infra/watch/src/lib_test.rs` |
| AC-11 | 顺序不变量：`on_output_ref` 触发时刻 live `.log` 与父目录已在（回调内断言）+ 上抛恰 1 次 + 缺省 / 零 spawn 两负面臂 | `packages/desktop/src-tauri/crates/infra/checks/src/static_check.rs` → `packages/desktop/src-tauri/crates/infra/checks/src/static_check_test.rs`；`packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod.rs` → `packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod_test.rs`；`packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` → `packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs`（mock runner 面）；`packages/desktop/src-tauri/src/commands/change_flow/mod.rs` → `packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs`（组合用例落点，D17——源码零 diff，仅测试文件增例） |
| AC-12 | 执行体语义零变：`live: None` 路径逐字节等价（既有 `run_test.rs` 全绿）；tee 不引入无界等待；`exec/Cargo.toml` 零 diff、无 tokio `fs` | `packages/desktop/src-tauri/crates/infra/exec/src/run.rs` → `packages/desktop/src-tauri/crates/infra/exec/src/run_test.rs`；`packages/desktop/src-tauri/crates/infra/exec/src/lib.rs` → `packages/desktop/src-tauri/crates/infra/exec/src/lib_test.rs`；`packages/desktop/src-tauri/crates/infra/checks/src/mutation/cargo_mutants.rs` → `packages/desktop/src-tauri/crates/infra/checks/src/mutation/cargo_mutants_test.rs`（回归登记）；`packages/desktop/src-tauri/crates/infra/checks/src/mutation/stryker.rs` → `packages/desktop/src-tauri/crates/infra/checks/src/mutation/stryker_test.rs`（回归登记）；`packages/desktop/src-tauri/crates/infra/exec/Cargo.toml`（零 diff 面，见不可测试项） |
| AC-13 | 上收零回归：`emit_node` / `StepSettle` / `now_millis` 迁入 `control.rs` 后 56 处调用点源码零改，时间戳 / token 断言全绿 | `packages/desktop/src-tauri/crates/core/orchestration/src/control.rs` → `packages/desktop/src-tauri/crates/core/orchestration/src/control_test.rs`；`packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` → `packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs` |
| AC-14 | bindings 与门禁：三 typed 包装与 `ExecOutputChunk` 在案入库（随前置重导出更新，过期由提交前人工核对）；全量门禁全绿 | `packages/desktop/src-tauri/src/commands/mod.rs` → `packages/desktop/src-tauri/src/bindings/mod_test.rs`；`packages/desktop/src-tauri/src/commands/changes/mod.rs` → `packages/desktop/src-tauri/src/commands/changes/mod_test.rs`；`packages/desktop/src/types/generated/bindings.ts` + `packages/desktop/src/types/generated/bindings.zod.ts` → `packages/desktop/src/lib/ipc-contract.test.ts`（生成物逐字面锚由本表 AC-14 的 commands/mod.rs 映射章节承接） |
| AC-15 | 边界纪律：`exec/Cargo.toml` / `use-file-watch-signal.ts` / `use-change-detail.ts` / `change-flow-graph.tsx` / `core/workflow/**` / `infra/store/**` 零 diff；`mutation/**` 仅两行 `live: None` | `packages/desktop/src-tauri/crates/infra/checks/src/mutation/cargo_mutants.rs` → `packages/desktop/src-tauri/crates/infra/checks/src/mutation/cargo_mutants_test.rs`；`packages/desktop/src-tauri/crates/infra/checks/src/mutation/stryker.rs` → `packages/desktop/src-tauri/crates/infra/checks/src/mutation/stryker_test.rs`（只登记 run 期零 live 受既有用例锁定；零 diff 判据见不可测试项） |
| AC-16 | 版本纪律：`package.json` 0.6.10 → 0.6.11（一次 patch）；`src-tauri/Cargo.toml` 不跟 | `packages/desktop/package.json`（配置面，见不可测试项） |
| AC-17 | 手工冒烟（Windows 桌面端）：运行中节点可点、命令在顶部、输出逐段涌入且粘底跟随、上滑停止跟随、收口重排一次、`cargo` 进度行不堆残行 | 全部前端被测模块的运行期前置置信：`packages/desktop/src/views/changes/hooks/use-exec-output.ts` → `packages/desktop/src/views/changes/hooks/use-exec-output.test.ts`；`packages/desktop/src/views/changes/flow/stdout-panel.tsx` → `packages/desktop/src/views/changes/flow/stdout-panel.test.tsx`；`packages/desktop/src/views/changes/flow/stdout-text.ts` → `packages/desktop/src/views/changes/flow/stdout-text.test.ts`；`packages/desktop/src/views/changes/flow/detail-drawer.tsx` → `packages/desktop/src/views/changes/flow/detail-drawer.test.tsx` |

---

## 单元测试

### `packages/desktop/src-tauri/crates/infra/exec/src/lib.rs` -> `packages/desktop/src-tauri/crates/infra/exec/src/lib_test.rs`

既有测试文件**追加独立用例组**（模块内注释分节，建议标题 `运行中记录与增量回查（change-detail-live-stdout）`）；既有 `record_exec_output*` / `read_exec_output*` 站点的断言行零改动（AC-12 的「薄包装语义零变」判据）。装置沿用既有 `TempRoot` RAII + 真实 tempdir（fs 即被测组合面）。

#### 待测功能

- `LiveExec::begin(root: &Path, change: &str, slot: ReportSlot) -> Result<Self, String>`: 前置铸造——`create_dir_all` 输出目录 + 建空 `<exec_id>.log` 并以追加模式持句柄；失败 `Err(String)` 带语境串；**文件与父目录必须先于 `exec_id` 对外可见**。
- `LiveExec::exec_id(&self) -> &str`: 铸造值只读，`commit` 返回同值（同 id 收口的物理载体）。
- `LiveExec::log_path(&self) -> &Path`: 布局派生 `.log` 绝对路径，与 `exec_output_log_path_in` 同值。
- `LiveExec::sink(&self) -> LiveSink`: 追加口（交 `RunOptions.live`）；内部 `Mutex` 支撑两路 drain 并发；每次 `write` 后 `flush`；写失败置句柄 `None`（此后恒空操作、不上抛、不 panic）。
- `LiveExec::write_command(&self, command: &str) -> Result<(), String>`: `<exec_id>.cmd` 的**原样追加**（零换行 / 零格式化 / 零改写），自开自关句柄（不共用 `writer` 状态位）。
- `LiveExec::commit(&self, output: &[u8]) -> Result<String, String>`: 先 take 句柄（关文件）再覆盖写权威内容，返回**同一** `exec_id`；此后追加恒空操作。
- `record_exec_output(root: &Path, change: &str, slot: ReportSlot, output: &[u8]) -> Result<String, String>`: 降为 `LiveExec::begin(..)?.commit(..)` 薄包装——签名 / 语义 / 落点 / 零截断零改写逐条不变。
- `read_exec_command_in(change_dir: &Path, exec_id: &str) -> Option<String>`: sidecar 回查（`Test` 槽先 / `Baseline` 槽后，首中即返）；非法 `exec_id` 或无 sidecar 的老记录 → `None`。
- `exec_output_log_path_in(change_dir: &Path, exec_id: &str) -> Option<PathBuf>`: 布局派生 `.log` 绝对路径（双锚探测、首中即返）；非法 / 两槽 miss → `None`。
- `read_exec_output_delta_in(change_dir: &Path, exec_id: &str, from_offset: u64) -> Option<ExecOutputDelta>`: 增量读——字符边界收口、`from_offset > 长度` 夹 0、空文件 `Some("")`、非法 / 两槽 miss → `None`；`ExecOutputDelta { text, next_offset }` 零 derive。

<!-- design「类型定义」表另声明 `LiveSink`（`Clone` + 手写 `Debug`）与 `ExecOutputDelta`（零 derive）的形态约束；其 derive / Debug 形态由下方「类型面形态」用例行锁定（该表不属「公共函数 / API」表，故此处置为补充覆盖而非新增 API 条目）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `LiveExec::begin` | 正向 | 指定 `(root, change, slot=Test)` 铸造 → `exec_id` 非空且形如 `exec-<millis>-<seq>`，`reports/test/exec-outputs/<exec_id>.log` 已存在且为 **0 字节**，父目录 `exec-outputs/` 已创建（AC-1 的前置） | 单次 |
| `LiveExec::begin` | 正向 | `slot=Baseline` → 落 `reports/baseline/exec-outputs/`，`reports/test/` 树零新增（槽位同锚派生） | 单次 |
| `LiveExec::begin` | 异常 | 报告树路径被文件占位（`reports/test` 位置写普通文件）→ `Err(String)` 且串带「目录」/「失败」语境，不 panic、不留下半成品 `.log` | 单次 |
| `LiveExec::begin` | 边界 | `change` 取四种形态：普通名 / 超长名（>200 chars）/ 含 `-` 与中文 / 空串 → 前三种正常铸造且目录落位；空串按 layout 派生（不断言业务语义，仅断言不 panic 且返回 `Result`） | 参数化迭代（4 形态） |
| `LiveExec::exec_id` / `log_path` | 正向 | `commit` 前后 `exec_id()` 恒同值；`log_path()` == `exec_output_log_path_in(change_dir, exec_id)` 且为绝对路径、末尾为 `<exec_id>.log` | 单次 |
| `LiveExec::exec_id` / `log_path` | 异常 | 两次 `begin` → 两个 `exec_id` 互异、两 `log_path` 互异（`mint_exec_id` 序号面零改动） | 单次 |
| `LiveExec::exec_id` / `log_path` | 边界 | `log_path()` 反复调用返回同一 `PathBuf` 值、零 fs 副作用（父目录文件数不变） | 单次 |
| `LiveExec::sink` | 正向 | `begin` → `sink().absorb(b"line-1\n")` → `absorb(b"line-2\n")` → 以 `read_exec_output_delta_in(dir, id, 0)` 读回 `"line-1\nline-2\n"`、`next_offset == 14`（追加可见、顺序保序） | 单次 |
| `LiveExec::sink` | 正向 | 同一 `LiveExec` 取两个 `sink()`（模拟两路 drain 并发）→ 交替 `absorb` 各 100 次 → 文件字节数 == 全部片段长度之和、零丢失（`Mutex` 支撑并发写） | 单次 |
| `LiveExec::sink` | 异常 | 写失败静默：crate 内直构 `LiveExec`，`writer` 置为**只读句柄**（`OpenOptions::read(true)` 打开 `.log`）→ `absorb` 不 panic 不上抛、句柄被置 `None`；随后 `absorb` 恒空操作、`commit` 仍覆盖为权威内容 | 单次 |
| `LiveExec::sink` | 边界 | `absorb(&[])`（0 字节）→ 文件长度不变、零 panic；`absorb` 一段 8 KiB 恰满块与其前后各 1 字节 → 三者全部落盘（chunk 边界不丢字节） | 参数化迭代（0 / 8191 / 8192 / 8193 字节） |
| `LiveExec::sink` | 边界 | `commit` 之后再 `absorb(b"late")` → 文件内容仍等于权威内容（追加恒空操作，不需要第二状态位，D1） | 单次 |
| `LiveExec::write_command` | 正向 | `write_command("pnpm run check 2>&1")` → `read_exec_command_in` 逐字返回该串（**无尾随换行**、含 `2>&1`）；文件位于 `.log` 同目录、名 `<exec_id>.cmd` | 单次 |
| `LiveExec::write_command` | 正向 | 连续两次 `write_command`（`"\n=== plan a · rust ===\ncmd-a\n"` 后 `"\n=== plan b · vp ===\ncmd-b\n"`）→ 回查为两段原样拼接（原样追加、无二次格式化） | 单次 |
| `LiveExec::write_command` | 异常 | crate 内直构 `LiveExec`（子模块可见父模块私有字段）并把 `.cmd` 目标位置预置为**同名目录** → `write_command` 返回 `Err(String)`、`.log` 面零影响、`commit` 仍成功（两路互不共用状态位） | 单次 |
| `LiveExec::write_command` | 边界 | `command` 取四种形态：`""`（空串）/ 超长（>1000 chars）/ 含 `\n` 与 `\0` 与 emoji / 含 `\r\n` → 全部**逐字节原样**写入与回查（MUST NOT 追加换行、MUST NOT 转义） | 参数化迭代（4 形态） |
| `LiveExec::commit` | 正向 | 运行中追加 `"interleaved"` 后 `commit(b"authoritative\n")` → 返回 id == `begin` 的 id；`.log` 逐字节等于 `authoritative\n`（live 交错流被整体覆盖，AC-2） | 单次 |
| `LiveExec::commit` | 异常 | crate 内直构 `LiveExec { log_path: <预建目录路径>, .. }` → `commit` 返回 `Err(String)` 不 panic；再次 `commit` 仍返回 `Err`（幂等失败面，句柄已 take） | 单次 |
| `LiveExec::commit` | 边界 | `commit(&[])` → `.log` 被截断为 0 字节且返回 id 在案（test-execution 复用门的空记录形态）；`commit` 超长内容（>1 MiB）与非法 UTF-8 字节 → 逐字节保真（零截断零改写） | 参数化迭代（空 / 超长 / 非法字节） |
| `record_exec_output` | 正向 | 既有站点零改动全绿：落盘 → `read_exec_output` 逐字节一致、仅一个 `.log`、`exec_id` 定式、两次互异、change 隔离（**既有** 6 例） | 参数化迭代（既有站点全集） |
| `record_exec_output` | 异常 | 既有目录不可写臂仍 `Err`：`reports/test` 位置被普通文件占位 → `Err` 串含 `exec` / `目录` / `落盘` 语境（薄包装后 `Err` 可能来自 `begin`（建目录 / 建文件）或 `commit`（覆盖落盘），两种文案都带同一语境簇——用例只断言语境与 `Err` 面，不断言失败位点，避免把单点实现细节钉进断言） | 单次 |
| `record_exec_output` | 边界 | 薄包装等价：同一 `(root, change, slot, bytes)` 依次调 `record_exec_output` 与 `LiveExec::begin(..)?.commit(..)` → 两者落点目录名与文件形态一致（仅 `exec_id` 值不同）、内容逐字节相等 | 组合迭代（两 API × 4 种字节形态：空 / 超长 / 非法字节 / 含 emoji） |
| `read_exec_command_in` | 正向 | `Test` 槽 sidecar 命中；`Baseline` 槽 sidecar 命中（双锚各自可达，AC-3） | 参数化迭代（2 槽） |
| `read_exec_command_in` | 异常 | 老记录（只有 `.log`、无 `.cmd`）→ `None`（非错误）；两槽皆无 → `None` | 单次 |
| `read_exec_command_in` | 边界 | 非法 `exec_id` 七形态：`""` / `".."` / `"../secret"` / `"a/b"` / `"a\b"` / `"C:evil"` / `"exec-1.cmd"` → 全部 `None`（路径逃逸零命中） | 参数化迭代（7 形态） |
| `exec_output_log_path_in` | 正向 | `begin` 后 → 返回绝对路径且 `is_file()`；与 `LiveExec::log_path()` 等值；`commit` 后仍命中 | 单次 |
| `exec_output_log_path_in` | 异常 | 未 `begin` 过的 change 目录 / 未知 `exec_id` → `None`；非法 `exec_id`（同上七形态）→ `None` | 参数化迭代（7 + 2 形态） |
| `exec_output_log_path_in` | 边界 | 同 id 在 `Test` 与 `Baseline` 两槽并存（人为双写）→ 返回 `Test` 槽路径（槽序 `Test` 先，首中即返；零串树） | 单次 |
| `read_exec_output_delta_in` | 正向 | `from_offset = 0` → `text` 与 `read_exec_output` 逐字相等（AC-4）；`next_offset` == 该文本字节长度 | 单次 |
| `read_exec_output_delta_in` | 正向 | 三次连续读（每次传上次 `next_offset`）：`text` 拼接逐字等于全量、每次 `next_offset` 单调不减、第三次之后读取返回 `text == ""` 且 `next_offset` 不变 | 单次 |
| `read_exec_output_delta_in` | 异常 | `from_offset > 文件长度`（先读到 100，再 `commit` 覆盖为 20 字节，再以 100 读）→ 夹 0 重读、返回全文且 `next_offset == 20`（MUST NOT panic / MUST NOT 返回空串） | 单次 |
| `read_exec_output_delta_in` | 异常 | 未 `begin` 的 `exec_id` / 非法 `exec_id`（七形态）→ `None`（miss 非错误） | 参数化迭代（7 + 1 形态） |
| `read_exec_output_delta_in` | 边界 | 块末半字符：追加 `"中".as_bytes()[..2]`（3 字节字符的前 2 字节）→ 本次 `text` 不含该字符任何碎片、`next_offset` 停在该字符起点；补齐第 3 字节后下一次读取完整返回 `"中"` | 单次 |
| `read_exec_output_delta_in` | 边界 | 非法字节推进：追加 `b"head\xff\xfe"` 后读 → `text` 以替换符呈现且 `next_offset` 严格越过非法字节（不形成永不推进循环）；继续追加合法文本后下一次读到其后的合法文本 | 单次 |
| `read_exec_output_delta_in` | 边界 | 空文件：`begin` 后立即读（`from_offset = 0`）→ `Some { text: "", next_offset: 0 }`（与 miss 可分辨）；`from_offset` 取 `u64::MAX` 与 `u64::MAX - 1` → 夹 0 重读、零 panic | 参数化迭代（0 / 长度 / 长度+1 / `u64::MAX-1` / `u64::MAX`） |
| `read_exec_output_delta_in` | 边界 | 4 字节字符（emoji `🚀`）跨块边界截断（前 1 / 2 / 3 字节三形态）→ 三形态均不产出碎片、补齐后完整返回；`next_offset` 恒落字符边界 | 参数化迭代（1 / 2 / 3 字节截断） |
| 类型面形态（`LiveSink` / `ExecOutputDelta`） | 正向 | `LiveSink` 可 `Clone`、`format!("{:?}")` 为 `"LiveSink(..)"`（手写 `Debug`，`Arc<dyn Fn>` 无 `Debug`）；`LiveSink::new(..)` 直构后多次 `absorb` 以到达序投递（crate 内可直构，D2） | 单次 |
| 类型面形态（`LiveSink` / `ExecOutputDelta`） | 异常 | `LiveSink` 的闭包在 `absorb` 内部不改变投递语义（空 `&[]` 投递 1 次、零 panic）；`RunOptions` 的 `Debug` 派生在位（`format!("{:?}")` 不 panic，红线 ②） | 单次 |
| 类型面形态（`LiveSink` / `ExecOutputDelta`） | 边界 | `ExecOutputDelta` 零 derive 的形态事实：仅字段可读（`text` / `next_offset`），构造与解构零额外约束；`next_offset` 取 `0` / `u64::MAX` 可赋值（纯结构） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 不 mock：文件系统 | 真实 tempdir（既有 `TempRoot` RAII）+ 真实 `reports/<slot>/exec-outputs/` 目录树往返；fs 是被测组合面 | 全章节 |
| 不 mock：`foundation::layout` | 真实 `ReportSlot` / `change_reports_in`（「与 layout 同锚」这一断言的前提） | 全章节 |
| 内部构造入口（非 mock） | crate 内直构 `LiveSink::new(..)`（tee 单测）与 `LiveExec` 的只读句柄形态（写失败臂）——两者是 design D2 明示的 `pub(crate)` 面，用于制造公开 API 不可触达的失败臂，不是模块替身 | `LiveSink` 直测、`sink` 写失败静默 |
| 不 mock：时钟 / 环境变量 / 子进程 | 本节用例零 spawn、零时钟注入（`mint_exec_id` 只用真实毫秒，形态断言不比较具体值） | 全章节 |

### `packages/desktop/src-tauri/crates/infra/exec/src/run.rs` -> `packages/desktop/src-tauri/crates/infra/exec/src/run_test.rs`

#### 待测功能

<!-- design.md「公共函数 / API」表未声明该文件的公共 API 变更（`drain` / `attempt_once` / `await_exit` / `wait_cancel` / `DRAIN_CHUNK_BYTES` 均为模块私有）。行为面取自 design D7 /「类型定义」表与 Module Contract：`RunOptions` 增 `live: Option<LiveSink>`、`drain` 改 8 KiB chunk 循环 + tee、`attempt_once` / `await_exit` 透传。 -->

- `run_shell` 的 `live` 观察面（经公共入口 `run_shell(command, RunOptions { live: .. })` 观测；`drain` 为私有件，直测说明见末两行与「不可测试项」）: `live: None` 时排水与本变更前逐字节等价；`live: Some(sink)` 时每读一块先入内存缓冲再 tee 给 sink（到达序、零改写）；重试面同一 sink 收全部尝试字节；被杀两臂仍以 `FINISH_GRACE` 截断，sink 不得引入新的无界等待。

既有装置：`ShimWs`（平台 shim 脚本 + `put` / `put_bytes`）与 `options(timeout)` 辅助函数（补 `live: None` 属编译面涟漪，行为断言零改）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `run_shell` 的 `live` 面 | 正向 | `live: Some(sink)` + 单管道（脚本 `2>&1` 形态）写 3 行 → sink 累计缓冲逐字节等于 `RunReport.stdout`，且 `stdout` 仍逐字节等于进程写入内容（零改写） | 单次 |
| `run_shell` 的 `live` 面 | 正向 | 双管道形态（stdout 与 stderr 各写若干行、不施加 `2>&1`）→ 共享同一 sink：累计缓冲的多集合包含两路全部字节、总字节数相等（交错序只作包含关系断言，R-11） | 单次 |
| `run_shell` 的 `live` 面 | 正向 | 重试面（`RetryPolicy::transient()`，前两次失败第三次成功）→ 同一 sink 按尝试序收到全部尝试字节（总长度 ≥ 末次 `stdout` 长度，末次字节出现在末尾） | 单次 |
| `run_shell` 的 `live` 面 | 异常 | `spawn` 失败（程序不可达）→ sink 零调用、`Err(RunError::Spawn)` 语义与文案零变 | 单次 |
| `run_shell` 的 `live` 面 | 异常 | 入口短路（`cancel` 已置位）→ 零 spawn、sink 零调用、`Cancelled` + `exit_code == -1` | 单次 |
| `run_shell` 的 `live` 面 | 边界 | 输出恰好 8192 字节（一块满）/ 8193 字节（跨块）/ 1 字节 → sink 总量 == `stdout` 长度、字节序一致（chunk 边界不丢不重） | 参数化迭代（1 / 8192 / 8193 字节） |
| `run_shell` 的 `live` 面 | 边界 | 空输出（脚本零 echo）→ 首次 `Ok(0)` 即 break、sink 零调用、`stdout` 为空 | 单次 |
| `run_shell` 的 `live` 面 | 边界 | 超大输出（>1 MiB）+ 非 UTF-8 原始字节 → sink 累计逐字节等于 `stdout`（sink 面是字节不是文本） | 单次 |
| `run_shell` 的 `live` 面 | 边界 | 被杀两臂（超时 / 取消）且 `live: Some`，孤儿孙进程持写端（既有 `自然退出_孤儿孙进程持写端时收尸后有界收口` 装置 + sink 在位）→ 仍以 `FINISH_GRACE` 截断返回、墙钟上界与 `live: None` 相同、sink 收到截断前字节（红线 ③ / AC-12） | 组合迭代（超时 × 取消） |
| `run_shell` 的 `live` 面 | 正向 | **既有回归**：`live: None` 路径全组零改动全绿（单流 / 退出码谱 / 双缓冲不合流 / cwd / env / 超时 / 取消 / 重试 / 卡死早停 / 末次输出 / `单次等价回归_逐字节同改造前`）——AC-12 的逐字节等价判据 | 参数化迭代（既有站点全集） |
| `drain`（私有件，直测） | 正向 | 自造 `AsyncRead` 按「8192 字节块 → 1 字节块 → EOF」产出 → `buffer` 与 sink 均收到全部字节且顺序一致；`pipe: None` → 立即返回且 buffer / sink 零变化 | 单次 |
| `drain`（私有件，直测） | 边界 | 空流（首读即 EOF）→ `buffer` 与 sink 均零字节、立即返回；0 字节块（`Ok(0)` 但非 EOF 的合法形态）→ 不视为 EOF 之前的死循环（按 EOF 语义收口）、零 panic；`DRAIN_CHUNK_BYTES` 边界三点（8191 / 8192 / 8193 字节）→ 全部字节落盘、零丢零重 | 参数化迭代（空流 / 0 字节块 / 8191 / 8192 / 8193） |
| `drain`（私有件，直测） | 异常 | 自造 `AsyncRead` 产出 3 字节后返回 `Err` → break：`buffer` 保留已读 3 字节、sink 收到同样 3 字节、零 panic（「已读即输出」语义零变） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 子进程边界 | 真实平台 shim 脚本（既有 `ShimWs`：Windows `.cmd` / unix `sh` + 执行位），不 mock 进程、不 mock `tokio::process` | 全部 `run_shell` 用例 |
| `sink` 观察面 | 真实 `LiveSink`（crate 内直构 `LiveSink::new` 指向 `Arc<Mutex<Vec<u8>>>` 记录器）或真实 `LiveExec::sink()`——sink 是被测契约的一部分，替身化会让 tee 断言自证 | 全部 `live: Some` 用例 |
| `tokio::io::AsyncRead` 替身 | 仅用于 `drain` 的 chunk 边界与读错误两臂（`run_test` 是 `run` 的同 crate 子模块，可直调私有 `drain`；该替身是被测函数的**显式入参**，不是内部模块替身） | `drain` 两行 |
| 计时 | 被杀两臂沿用既有虚拟时钟（`tokio::time::pause` + `advance(FINISH_GRACE)`）与真实时钟两套装置，不新增计时替身 | 超时 / 取消 / 孤儿孙进程 |
| 进程级环境 | 裸名用例沿用 `crate::lock_test_env()` 串行化的 `PathPrefix`；新增用例优先用脚本绝对路径（零 PATH 依赖） | 全章节 |

### `packages/desktop/src-tauri/crates/infra/checks/src/static_check.rs` -> `packages/desktop/src-tauri/crates/infra/checks/src/static_check_test.rs`

#### 待测功能

<!-- design.md「公共函数 / API」表未声明该文件的公共 API 变更（`ProcessStaticCheck` / `execute` / `ensure_program_resolvable` 形态不变；`StaticCheckRunner::run` 的扩参记在 `port.rs` 行）。行为面取自 design D4 / D5 的上游与「修改文件」表：`execute` 增 `on_output_ref` 形参并插入 `begin → write_command → 上抛 → run_shell(live) → commit` 五步链。 -->

- `ProcessStaticCheck::run`（记录链观测面）: `begin`（建目录建空 `.log`）→ `write_command("{command} 2>&1")`（失败静默 `.ok()`）→ `on_output_ref(exec_id)`（触发时文件与父目录已在）→ `run_shell(RunOptions { live: Some(sink), .. })` → `commit(attempts_log(&run))`（同 id、失败降级 `None`）；无配置 / 空命令直过分支**零 `begin`**；被杀两臂仍 `commit` 已收字节。

既有装置（`TempWs` + `write_config` + `write_shim` + `write_staged_shim` + `PathPrefix` + `env_window` + `live_logs` / `wait_for_non_empty_log` / `change_dir`）全部沿用；`check()` / `check_test()` 辅助的末位 `None` 属编译面涟漪、其 5 处调用点的断言零改。**新增一个辅助** `check_with_ref(root, name, slot, sink: OutputRefSink) -> Result<StaticCheckOutcome, String>`（体内把 `Some(sink)` 传给 `ProcessStaticCheck::run`）供上抛臂使用——这不是为测试开的生产导出，而是测试文件内的装置（design D16 的 `pub fn OutputRefSink::new` 是其可写性的前提，MUST NOT 再以整体 `None` 兜底）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ProcessStaticCheck::run` 记录链 | 正向 | 分阶段写输出的 shim（先写 1 行 → 等 marker → 再写 1 行）→ 进程未结束时 `read_exec_output_delta_in` 读到第一行；随后再读后读 ⊇ 前读（单调增长，AC-1 / R-11 用包含关系不断言精确长度） | 单次 |
| `ProcessStaticCheck::run` 记录链 | 正向 | `on_output_ref` 回调内立即以 `exec_id` + change 目录调 `exec_output_log_path_in` 并 `is_file()` → 文件与父目录均在案（AC-11 顺序不变量） | 单次 |
| `ProcessStaticCheck::run` 记录链 | 正向 | 终态 `output_ref` == 回调收到的 `exec_id`（同 id 收口，AC-2）；`.log` 逐字节 == `attempts_log(&run)`（live 交错被覆盖；多尝试面含 attempt 分隔头） | 单次 |
| `ProcessStaticCheck::run` 记录链 | 正向 | `.cmd` 逐字 == `"{static_analysis} 2>&1"`（无尾随换行）、与 `.log` 同目录同 id 前缀、`read_exec_command` 回查同值（AC-3） | 单次 |
| `ProcessStaticCheck::run` 记录链 | 异常 | 无配置键 / 空命令 / 全空白命令三形态 → `passed == true`、`output_ref == None`、`reports/<slot>/exec-outputs/` **零新增文件**（零 `begin`，AC-1 的负面判据） | 参数化迭代（3 形态） |
| `ProcessStaticCheck::run` 记录链 | 异常 | 取消（入口已置位）与超时（虚拟时钟推进）两臂 → 仍 `commit` 已收字节：`output_ref` 在位、`.log` 可读（可为 0 字节）、`fault` 文案与 `passed` 口径零变 | 组合迭代（取消 × 超时） |
| `ProcessStaticCheck::run` 记录链 | 异常 | `on_output_ref: None` → 零行为差异：`output_ref` 仍按 `commit` 回填、`.log` / `.cmd` 形态与 `Some` 臂一致（仅无回调），既有 5 处 `.run(..)` 断言行零改 | 单次 |
| `ProcessStaticCheck::run` 记录链 | 异常 | 记录面降级（**既有回归 + 扩展**）：`reports/test` 位置被普通文件占位使 `LiveExec::begin` 失败 → `output_ref == None`、检查主流程照常（`passed` 与 `diagnostics` 口径零变）、回调**零**调用（无 id 可上抛）。既有用例 `落盘失败降级none不阻断主流程` 补一条回调零调用断言 | 单次 |
| `ProcessStaticCheck::run` 记录链 | 异常 | `on_output_ref` 恰被调用 **1** 次（计数器）；命令不可达 `Err` 臂与 `archi` 槽位拒收臂 → 回调 **零** 调用（零 spawn ⇒ 零上抛） | 组合迭代（1 次 × 两零次臂） |
| `ProcessStaticCheck::run` 记录链 | 边界 | 输出为空（零 echo 命令）→ `.log` 0 字节且 `output_ref` 在位；`read_exec_output_delta_in(dir, id, 0)` → `Some { text: "", next_offset: 0 }`（空文件 ≠ miss） | 单次 |
| `ProcessStaticCheck::run` 记录链 | 边界 | 超长输出（>1000 chars）与非 UTF-8 字节 → `.log` 逐字节保真、`diagnostics` 口径零变；超长命令串的 `.cmd` 原样不截断 | 参数化迭代（超长 / 非法字节） |
| `ProcessStaticCheck::run` 记录链 | 边界 | 两次 `run` → 两 `exec_id` 互异、各自 `.log` 与 `.cmd` 独立互不覆写（既有 `重复run_各自exec_id落独立log且前次不被覆写` 扩 `.cmd` 断言） | 单次 |
| `ProcessStaticCheck::run` 记录链 | 边界 | `slot=Baseline` → `.log` / `.cmd` 均落 `reports/baseline/exec-outputs/`，`reports/test/` 零混居（既有槽位回归 + sidecar 断言） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 不 mock：文件系统 / config | 真实 `TempWs` tempdir + 真实 `openspec/config.json` 写入（`write_config`）+ 真实报告树落盘 | 全章节 |
| 不 mock：子进程 | 真实平台 shim（`write_shim` / `write_spin_shim`）；分阶段输出用 sleep + marker 文件时序；`PathPrefix` 提供 PATH 前缀 | 运行中读取、上抛时刻、超时 / 取消 |
| `PathPrefix` / `EnvWindow` | 既有 RAII 装置（`crate::lock_test_env()` 串行化进程级 PATH / env 替换），不属模块替身 | 全部 spawn 用例 |
| `OutputRefSink` | 真实 `OutputRefSink::new(..)`（checks 侧只能经此构造；回调体做三件事：计数 +1、断言文件存在、把 `exec_id` 记入 `Arc<Mutex<Vec<String>>>`） | 上抛序列与次数用例 |
| 计时 | 超时臂沿用虚拟时钟推进（`tokio::time::advance`）；分阶段读取用真实短 sleep + marker | 超时臂、运行中读取 |

### `packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod.rs` -> `packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod_test.rs`

#### 待测功能

<!-- design.md「公共函数 / API」表未声明该文件的公共 API 变更（`ProcessTestExecution` / `execute` / `outcome` / `fault_outcome` / `push_plan_head` 形态见「修改文件」表：`outcome` / `fault_outcome` 首三形参改为 `&LiveExec`）。行为面取自 design D4 / D5 与 delta spec「输出记录为脚本真实输出」的扩写条。 -->

- `ProcessTestExecution::run`（记录链观测面）: `detect` 成功之后、复用门判定之前 `begin` + 上抛 `exec_id`（D5 位点：`detect` 的 `Err` 不留孤儿空记录）→ 逐 plan spawn 前 `write_command("\n=== plan {id} · {framework} ===\n{command}\n")` → 三分支（复用门 / fault / 正常）各以同 id `commit(&merged_output)`；`outcome` / `fault_outcome` 吃 `&LiveExec`。

既有装置（`PathWindowGuard` + `set_shim_env` + `outcome_test` 包装 + 手工 `SummaryReport` fixture）沿用；`outcome_test` 体内改为先 `LiveExec::begin(..)` 再调 `outcome(&live, ..)`，其 6 处调用点签名不变、断言面不动（编译面涟漪）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ProcessTestExecution::run` 记录链 | 正向 | 两 plan 均 spawn（rust + vp shim）→ `.cmd` 含两段、各以 `=== plan <id> · <framework> ===` 起手并紧随展开后的命令（段序 == plan 序，AC-3） | 单次 |
| `ProcessTestExecution::run` 记录链 | 正向 | 同上 → `.log` **不含**命令文本（内容仍是脚本真实输出 + plan 分隔头）；去掉分隔头与 `[argv]` 行后与脚本输出逐字节一致 | 单次 |
| `ProcessTestExecution::run` 记录链 | 正向 | `on_output_ref` 回调内断言 `.log` 与父目录已在（AC-11）；终态 `TestExecutionOutcome.exec_id` == 回调收到值（AC-2） | 单次 |
| `ProcessTestExecution::run` 记录链 | 正向 | 运行中读取：第一个 plan 输出后、第二个 plan 未开始时读 `.log` → 已含第一段输出；执行结束后再读 → 覆盖为 `merged_output` 且 id 不变（AC-1 / AC-2） | 单次 |
| `ProcessTestExecution::run` 记录链 | 异常 | 复用门命中（既有 summary 齐整 + 输入未变）→ 零 spawn 仍产出一条 `.log`（空内容）并回填 `exec_id`；`.cmd` 缺省（无命令可记）⇒ `read_exec_command` 为 `None` | 单次 |
| `ProcessTestExecution::run` 记录链 | 异常 | fault 分支（取消 / 超时 / 预算耗尽）→ `fault_outcome` 仍以**同 id** `commit` 已执行 plan 的合并流；`conclusion == Fail` 占位与四项计数 0 零变 | 组合迭代（取消 × 超时 × 超预算） |
| `ProcessTestExecution::run` 记录链 | 异常 | `detect` 显式 `Err`（不支持的框架 / 空清单）→ **零** `LiveExec::begin`：`reports/<slot>/exec-outputs/` 无新增文件、回调零调用（D5 相对 proposal 字面的细化判据） | 单次 |
| `ProcessTestExecution::run` 记录链 | 异常 | `on_command` 回调面：`Reject` 臂（命令行超预算不可降级）→ 该 plan **零新段**且零 spawn（未 spawn 的 plan 不写 sidecar）；`on_output_ref: None` → `.log` / `.cmd` 形态与 `Some` 臂一致 | 组合迭代（Reject × 无回调） |
| `ProcessTestExecution::run` 记录链 | 边界 | 零 plan（配置 `tests` 为空）→ `detect` 返回空清单的成功路径：`begin` 建 0 字节记录 + 回填 id、`.cmd` 缺省 | 单次 |
| `ProcessTestExecution::run` 记录链 | 边界 | 单 plan / 三 plan 两形态 → `.cmd` 段数 == plan 数、段间以 `\n` 分隔且段首无多余空行（格式单点在 `mod.rs`，D2 / D4） | 参数化迭代（1 / 3 plan） |
| `ProcessTestExecution::run` 记录链 | 边界 | plan 命令串超长（`WithFiles` 展开后 >1000 chars）与含 emoji 的 plan id → `.cmd` 原样逐字（零改写、零截断）。**构造口径**（design 待决问题 13）：`plan.id` 由 `derive_plan_id(root, framework)`（`\` / `/` → `_`）派生，`detect` 无法产出 emoji 形态 ⇒ 该臂以**手工构造 `TestPlan`** 提供（沿 `runner_test.rs` 既有 `unsafe_plan` / `filler` fixture 同型先例） | 参数化迭代（超长 / 特殊字符） |
| `outcome` / `fault_outcome` | 正向 | `&LiveExec` 到位 → `commit(merged_output)` 同 id 覆盖；既有 6 处 `outcome_test` 调用点的 `exec_id` 非空与内容断言零改 | 参数化迭代（既有站点全集） |
| `outcome` / `fault_outcome` | 异常 | `commit` 失败 / `live == None`（`LiveExec::begin` 失败）→ `exec_id` 降级为空串（`commit_exec_id` 的 `unwrap_or_default` 口径零变）、不 panic；既有用例 `记录失败降级空串不阻断门禁结论` 与 `outcome_test` 的 `live == None` 分支零改动仍绿 | 单次 |
| `outcome` / `fault_outcome` | 边界 | `commit(&[])`（复用门空记录）→ `.log` 0 字节、`exec_id` 非空 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 不 mock：文件系统 / config | 真实 tempdir + 真实 `openspec/config.json`（`tests` 清单）+ 真实报告树 | 全章节 |
| 不 mock：子进程 | 真实平台 shim（`set_shim_env` / `set_shim_dest_2` 既有装配：以 env 注入 shim 路径与工件目的路径） | 全部 spawn 用例 |
| `PathWindowGuard` | 既有 RAII（`crate::lock_test_env()` 串行化 PATH 全局替换窗口） | 全部 spawn 用例 |
| `OutputRefSink` / `on_command` | 真实 `OutputRefSink::new(..)`；`on_command` 是 `execute_plan` 的**显式入参**（`&dyn Fn(&str) + Sync`），本节经公共入口 `run` 间接驱动（`mod.rs` 侧闭包即被测代码，不替身化） | 上抛序列、sidecar 分段 |
| 不 mock：`testexec/runner.rs` | 真实组合——`mod.rs` 与 `runner.rs` 的职责分界（谁写 `.cmd`、谁只透传）本身就是被断言项 | 全章节 |

### `packages/desktop/src-tauri/crates/infra/checks/src/testexec/runner.rs` -> `packages/desktop/src-tauri/crates/infra/checks/src/testexec/runner_test.rs`

#### 待测功能

<!-- design.md「公共函数 / API」表未声明该文件的公共 API 变更（design 明示 `execute_plan` 为 `pub(crate)`、`expand_template` / `decide_argv` 为模块私有）。行为面取自 design D4 与「修改文件」表：`execute_plan` 增 `live: Option<exec::LiveSink>` 与 `on_command: Option<&(dyn Fn(&str) + Sync)>` 两个末位形参，只透传、不自持记录生命周期。 -->

- `execute_plan(plan, report_dir, cancel, live, on_command)`（`pub(crate)`，经 `runner_test` 直调）: `decide_argv` 得最终命令串后、`ensure_program_resolvable` 与 spawn 之前，`on_command` 被调用**恰一次**（`Keep` / `Omit` 两臂；`Reject` 臂零调用）；`live` 原样交给 `run_shell`；记录生命周期（`begin` / `write_command` / `commit`）零职责。

既有装置沿用；46 处 `execute_plan(..)` 调用（`runner_test.rs`）与 1 处（`detect_test.rs`）补末位 `None, None`（编译面涟漪，断言零改）。**平台可达性**（design 待决问题 13）：`line_budget()` 在 `#[cfg(not(windows))]` 返回 `None` ⇒ unix 上 `execute_plan` 恒走 `Keep`，故 `Reject` 臂的端到端断言只在 Windows 上成立；跨平台等值判据由 `decide_argv` 直测承担（同 crate 可见、`budget` 是显式入参）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `execute_plan` 的透传面 | 正向 | `Keep` 臂（未超预算）→ `on_command` 恰调用 1 次，入参逐字 == spawn 命令串（与 `PlanExecution.argv_len` 同源）；`live: Some(sink)` → sink 收到该 plan 的输出字节 | 单次 |
| `execute_plan` 的透传面 | 正向 | `Omit` 臂（超预算但可安全降级）→ `on_command` 入参为降级后的命令串（含框架自发现、无 `{files}`），且 `argv_note` 在位 | 单次 |
| `execute_plan` 的透传面 | 异常 | `Reject` 臂（不可安全降级 / 降级后仍超预算）→ `on_command` **零** 调用、零 spawn、`fault == Some(Environment(..))`（不返回 `Err`）、`live` 零写入。**平台门控**（design 待决问题 13）：`line_budget()` 在 non-windows 返回 `None` ⇒ 端到端臂须 `#[cfg(windows)]`；非 Windows 上的等值判据由下一行 `decide_argv` 直测承担 | 组合迭代（Windows 端到端 × 跨平台 `decide_argv`） |
| `decide_argv`（`Reject` 臂的跨平台直测） | 异常 | 同 crate 直调 `decide_argv(plan, .., expanded, Some(0))`（`budget` 是**显式入参**，与平台无关）：`omit_files_safe = false` → `ArgvDecision::Reject { .. }`；`omit_files_safe = true` 且降级后仍超 `Some(0)` → 亦 `Reject`；两臂均**不产出命令串** ⇒ 与「`on_command` 零调用」同源（零命令即零回吐） | 参数化迭代（2 臂） |
| `decide_argv`（`Reject` 臂的跨平台直测） | 边界 | 预算边界三点：`Some(展开长度 - 1)` → `Reject`（差 1 字节即越界）；`Some(展开长度)` → `Keep`（恰等不越界）；`Some(u64::MAX)` → `Keep`（上限侧零溢出）；展开命令为空串 `""` 时 `Some(0)` → `Keep`（空命令长度 0 恰在预算内） | 参数化迭代（4 形态） |
| `decide_argv`（`Keep` / `Omit` 对照臂） | 正向 | `Some(plan_len)` 恰等于展开长度 → `Keep`（**原串回填**，命令与改造前逐字符一致）；`Some(0)` 且 `omit_files_safe = true` 且降级后为空 → `Omit { note: .. }`——两臂各产命令串 ⇒ 与「`on_command` 恰 1 次」同源 | 参数化迭代（2 臂） |
| `decide_argv`（`Keep` / `Omit` 对照臂） | 异常 | 不可安全降级的降级请求：`omit_files_safe = false` 且超预算 → 不产出命令串（`Reject`，零回吐）；`omit_files_safe = true` 但降级后仍超预算 → 亦不产出（降级不保证收敛时不静默放行） | 参数化迭代（2 形态） |
| `decide_argv`（`Keep` / `Omit` 对照臂） | 边界 | 空与极端输入：`expanded` 为空串 + `Some(0)` → `Keep`；`budget = None`（平台无预算）→ 恒 `Keep`（`line_budget()` 在 non-windows 的 `None` 出口）；`omit_files_safe = true` 且降级后为空串 → `Omit`（空命令合法） | 参数化迭代（3 形态） |
| `execute_plan` 的透传面 | 异常 | `ensure_program_resolvable` 失败（程序不可达）→ `on_command` 已调用 1 次（命令串已裁决），返回 `Err` 且 `live` 面零写入（spawn 未发生） | 单次 |
| `execute_plan` 的透传面 | 异常 | `live: None` 与 `on_command: None` → 与今日逐字节等价（既有 46 处调用站点全绿，`PlanExecution` 各字段断言零改） | 参数化迭代（既有站点全集） |
| `execute_plan` 的透传面 | 边界 | 报告目录清理失败（`report_dir` 被文件占位）→ 在命令裁决**之前**早返 `Err`，`on_command` 零调用、`live` 零写入 | 单次 |
| `execute_plan` 的透传面 | 边界 | `on_command` 被调用时刻早于 spawn：回调内探测「工件文件尚不存在」（`results_output` 不在案）→ 断言成立（顺序：裁决 → 回调 → 可达性 → spawn） | 单次 |
| `execute_plan` 的透传面 | 边界 | `cancel` 已置位 → spawn 入口短路：`on_command` 已调用 1 次（命令已裁决）、`live` 零写入、`fault == Some(Stopped)` | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 不 mock：文件系统 / config | 真实 tempdir 报告目录 + 真实 `TestPlan` fixture（既有 `detect` 产物或手工构造） | 全章节 |
| 不 mock：子进程 | 真实平台 shim（既有装配），不 mock `tokio::process` | 全部 spawn 用例 |
| `on_command` 观察面 | `Arc<Mutex<Vec<String>>>` 记录器 + 计数（该回调是被测函数的**显式入参**，不是内部模块替身） | 全部 `on_command` 用例 |
| `sink` 观察面 | 真实 `LiveSink`：`checks` 侧**只能经公共路径** `LiveExec::sink()` 取得（`LiveSink::new` 为 exec crate 内 `pub(crate)`，D18），落盘字节经 `std::fs::read(live.log_path())` 读回（既有 `sink_recorder` / `live_tee_bytes` 装置同型） | `live: Some` 用例 |
| 不 mock：`mod.rs` 的记录生命周期 | 真实组合（`runner_test` 直调 `execute_plan` 并断言其**不**产生 `.cmd` / 不调用 `commit`——记录职责边界即被测契约） | 全章节 |

### `packages/desktop/src-tauri/crates/core/orchestration/src/port.rs` -> `packages/desktop/src-tauri/crates/core/orchestration/src/port_test.rs`

#### 待测功能

<!-- design「公共函数 / API」表在本文件的条目共四行：`OutputRefSink::new`（见下方第 1 条）、`OutputRefSink::emit`（第 2 条）、`StaticCheckRunner::run`（第 4 条）、`TestExecutionRunner::run`（第 5 条）。下方第 3 条 `ToolStepRequest.on_output_ref` 来自 design「类型定义」表——同为本文件的变动契约面，故显式标注来源一并列入待测功能（不是虚构的 API 行）。 -->

- `OutputRefSink::new(run: impl Fn(&str) + Send + Sync + 'static') -> Self`（design「公共函数 / API」表）: 上抛回调的**唯一构造面**，可见性为 **`pub`**（design D16——生产唯一路径是 `walker::run_tool`，第二消费者是 `crates/infra/checks` 的记录链用例，跨 crate 须可见；若落 `pub(crate)` 则 AC-11 在 checks 侧结构性不可写，用例只能整体传 `None`）。盘上实况核对：`crates/core/orchestration/src/port.rs:154` 为 `pub fn new(..)`（非 `pub(crate)`）。形态约束见 design「类型定义」表：`Clone` 派生 + 手写 `Debug` / `PartialEq` / `Eq`。
- `OutputRefSink::emit(&self, exec_id: &str)`（design「公共函数 / API」表）: 投递一次 `exec_id`——原样透传、零改写、零校验；`crates/infra/checks` 两 runner 在 live 文件建好后调它**恰一次**。`PartialEq` / `Eq` 为 `Arc::ptr_eq` **身份语义**（沿 `foundation::cancel::CancelFlag` 的「同一底层对象」先例）；`Debug` 手写为 `"OutputRefSink(..)"`（`Arc<dyn Fn>` 无 `Debug`，而 `ToolStepRequest` 派生 `Debug`）。
- `ToolStepRequest.on_output_ref: Option<OutputRefSink>`（来源：design「类型定义」表）: 新增末位字段，缺省 `None` = 老行为；结构体仍 `derive(Debug, Clone, PartialEq, Eq)`。
- `StaticCheckRunner::run(&self, root: &str, name: &str, slot: ReportSlot, cancel: CancelFlag, on_output_ref: Option<OutputRefSink>) -> BoxToolFuture`: 末位为上抛回调；未提供或不调用 = 老行为；实现须在 live 记录文件**已创建之后**调用一次。
- `TestExecutionRunner::run(..)`（同型）: 同签名面与同调用时机契约。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `OutputRefSink::new` / `OutputRefSink::emit` | 正向 | `new(回调)` 后 `emit("exec-x")` → 回调收到 `"exec-x"`（原样、零改写）；`emit("")` 亦原样投递（空串是合法 exec_id 形态的边界由消费方判定） | 单次 |
| `OutputRefSink::new` / `OutputRefSink::emit` | 正向 | `Clone` 后两份副本 `emit` → 同一次回调（`Arc` 共享）各收到一条，计数与调用次数一致 | 单次 |
| `OutputRefSink::new` / `OutputRefSink::emit` | 异常 | 回调注入面：回调体内部 panic 时 `emit` 的调用点不上抛（按既有 `BoxToolFuture` 边界收口、不静默继续）；`emit` 空串 "" 与含控制字符 / emoji 的超长 `exec_id`（>1000 chars）→ 原样投递、零转义零截断 | 参数化迭代（4 形态） |
| `OutputRefSink::new` / `OutputRefSink::emit` | 边界 | 上抛次数边界：单实例连续 `emit` 0 次 / 1 次 / 1000 次 → 回调计数分别为 0 / 1 / 1000（恰等，零合并零丢弃）；`Clone` 后两副本交错 `emit` 100 次 → 计数和为 200（无丢失、无重复） | 参数化迭代（0 / 1 / 1000） |
| `OutputRefSink` | 异常 | 跨 crate 可构造性（D16）：`pub fn new(..)` 在 checks 侧可见——本用例以「同 crate 内可构造」为基线，跨 crate 面由 `static_check_test.rs` / `testexec/mod_test.rs` 的上抛臂实际调用锁定；`Debug` 输出恒为 `"OutputRefSink(..)"`（不含闭包地址、不 panic，红线 ②）；`ToolStepRequest` 的 `Debug` 在 `on_output_ref: Some(..)` 与 `None` 两形态均不 panic | 参数化迭代（`Some` / `None`） |
| `OutputRefSink` | 边界 | 身份相等语义：同一实例的两份 clone → `==` 且 `Eq` 自反；两个不同 `new(..)` 实例（回调体相同）→ `!=`（`Arc::ptr_eq` 为判据，不是行为等价）；`Option<OutputRefSink>` 的 `None == None` 成立 | 组合迭代（同实例 × 异实例 × `None`） |
| `ToolStepRequest.on_output_ref` | 正向 | 既有 33 处字面量补 `on_output_ref: None` 后编译通过；`PartialEq` 在两 `None` 形态下相等、`Some(同一实例)` 两形态下相等（`ToolStepRequest` 的 `Eq` 语义不变） | 参数化迭代（`None` / `Some` 同实例） |
| `ToolStepRequest.on_output_ref` | 异常 | `Some(不同实例)` 的 `ToolStepRequest` → `!=`（身份语义，非回调行为比较）；`port_test.rs:3075` 的穷尽解构补字段后 `on_output_ref.is_none()` 成立（形态适应而非口径改写） | 单次 |
| `ToolStepRequest.on_output_ref` | 边界 | 缺省语义：`on_output_ref: None` 的请求经 `steps.rs` 透传后 runner 侧收到 `None`（零补发拍、零额外通知）；既有 mock runner 仅补未使用形参即可编译 | 单次 |
| `StaticCheckRunner::run` / `TestExecutionRunner::run` | 正向 | 既有 9 处 `port_test` 调用面补末位 `None` 后断言零改（`ProgrammableRunner` / `ProgrammableTestExecutionRunner` 行为不变） | 参数化迭代（既有站点全集） |
| `StaticCheckRunner::run` / `TestExecutionRunner::run` | 异常 | 传入 `Some(sink)` 且 mock runner 从不调用 → 与 `None` 逐字段等价（`ToolStepOutput` 全字段相等，零补发拍） | 单次 |
| `StaticCheckRunner::run` / `TestExecutionRunner::run` | 边界 | 同一 `OutputRefSink` 实例传给两 trait 的 `run` → 两处 `emit` 均抵达同一回调（类型面不吞实例、跨 trait 复用不失效） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 不 mock：`OutputRefSink` | 真实 `OutputRefSink::new(..)`（`port_test` 与 `port.rs` 同 crate；构造面为 `pub`（D16），同 crate 与跨 crate 两面均可见）；回调体写 `Arc<Mutex<Vec<String>>>` | 全部用例 |
| 不 mock：`foundation::cancel::CancelFlag` / `ReportSlot` | 真实类型（身份语义与槽位是断言面） | 全部用例 |
| 进程边界 | 本节零 IPC / 零 spawn / 零文件系统（纯类型与契约面） | 全章节 |

### `packages/desktop/src-tauri/crates/core/orchestration/src/steps.rs` -> `packages/desktop/src-tauri/crates/core/orchestration/src/steps_test.rs`

#### 待测功能

<!-- design.md「公共函数 / API」表未声明该文件的公共 API 变更（`LocalToolSteps::new` / `execute` 形参面不变；`steps.rs` 的改动是两分支原样透传 `step.on_output_ref`，记在 Module Contract）。行为面取自 design「修改文件」表与 delta spec Module Contract 的 `steps.rs` 行。 -->

- `LocalToolSteps::execute`（工具步分派面）: static-check 与 test-execution 两分支把 `step.on_output_ref` **原样**透传给对应 runner（`Option` 未消费即丢弃，其余分支行为零变）；`step.cancel` 已在统一点取出，不受影响。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `LocalToolSteps::execute` 透传 | 正向 | static-check 分支：请求携 `on_output_ref: Some(sink)` → `FakeRunner` 收到**同一实例**（`Arc::ptr_eq` 判等）；`None` → runner 收到 `None` | 参数化迭代（`Some` / `None`） |
| `LocalToolSteps::execute` 透传 | 正向 | test-execution 分支同上（`FakeTestExecutionRunner` 收到同一实例） | 参数化迭代（`Some` / `None`） |
| `LocalToolSteps::execute` 透传 | 异常 | 其余九类分支（phaseNext / phaseStart / phaseLog / backtrack / decisionLog / mutationTest / archiCheck / archivePreflight / archiveSeal）→ `on_output_ref` 不被消费、不传给任何 runner（各自 `ToolStepPort` 签名未扩）；`Option` 丢弃零副作用 | 参数化迭代（9 分支） |
| `LocalToolSteps::execute` 透传 | 边界 | 23 处既有 `ToolStepRequest` 字面量补 `on_output_ref: None` 后断言零改（编译面涟漪的机械判据） | 参数化迭代（既有站点全集） |
| `LocalToolSteps::execute` 透传 | 边界 | runner 返回 `Err` / `fault` 两臂 → 上抛通道不改变错误面与产出面（`Terminal` 归因逻辑零变） | 组合迭代（`Err` × `fault`） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `FakeRunner` / `FakeTestExecutionRunner` | 既有测试装置（记录收到的 `root` / `name` / `slot` / `cancel` / `on_output_ref` 与调用序）；它们是被测 `execute` 的**注入依赖**（`Arc<dyn StaticCheckRunner>`），不是内部模块替身 | 全部用例 |
| 不 mock：`steps.rs` 自身 | 真实组合——透传即被测契约 | 全章节 |
| 进程边界 | 本节零 spawn / 零 IPC；文件系统只在既有用例中经 tempdir 触达 | 全章节 |

### `packages/desktop/src-tauri/crates/core/orchestration/src/control.rs` -> `packages/desktop/src-tauri/crates/core/orchestration/src/control_test.rs`

#### 待测功能

<!-- design「类型定义」表另声明 `StepEmitter` 形态（`#[derive(Clone)]`、字段私有、唯一构造面 `RunGuard::emitter()`）与 `StepSettle` 的搬迁可见性；`StepSettle` 的六方法不属「公共函数 / API」表，其行为由下方铸造行与既有 control_test 站点共同锁定。 -->

- `RunGuard::emitter(&self) -> StepEmitter`: `'static` 发射器（`(root, change_id)` 复合键 + `Arc<ChangeFlowControl>`），供跨 port 边界回调持有。
- `StepEmitter::attach_output_ref(&self, phase: &str, attempt: u32, step: ChangeStepKind, instance_no: u32, exec_id: &str)`: 补发一拍 `running + output_ref` 的步状态；经同一 `emit_node` 铸造单点；`accumulate_node` 就地合并（不新增节点、不动 `started_at`）。
- `now_millis() -> i64`（搬迁为 `pub(crate)`）: 时钟缝单点——时钟早于 epoch 取 0，不 panic；`emit_node` 与 walker 的 run 收口共用。
- `emit_node(guard: &RunGuard, phase: &str, attempt: u32, step: ChangeStepKind, instance_no: u32, settle: StepSettle)`（搬迁为 `pub(crate)`）: 步状态行铸造单点；`started_at` / `ended_at` 由本函数铸造（running → `(now, None)`、终态 → `(now, Some(now))`）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `RunGuard::emitter` | 正向 | 已 `begin_run` 的 guard → `emitter()` 产出的发射器在 guard **被 drop 后**仍可 `attach_output_ref`（`'static` 语义：复合键 + `Arc` 自带所有权），节点被写出 | 单次 |
| `RunGuard::emitter` | 正向 | 同一 guard 两次 `emitter()` → 两发射器对同一 `(root, change_id)` 键写入，节点累积在同一 run 上（零串台） | 单次 |
| `RunGuard::emitter` | 异常 | run 已 `finish`（终态除名）后调用 `emitter()` 再 `attach_output_ref` → 静默无副作用（注册表 miss 分支），零 panic、零节点新增 | 单次 |
| `RunGuard::emitter` | 边界 | 同 root 异 change id 两 guard 的发射器交叉写入 → 各自 run 的节点互不可见（复合键隔离） | 组合迭代（同 root × 异 id） |
| `StepEmitter::attach_output_ref` | 正向 | 先 `emit_node(running)`（无 `output_ref`），再 `attach_output_ref("implement", 2, StaticCheck, 0, "exec-x")` → 同键节点被就地合并：`status == running`、`output_ref == Some("exec-x")`、节点**总数仍为 1**、`started_at` 为首次 emit 值（AC-5 / AC-13 的合并语义） | 单次 |
| `StepEmitter::attach_output_ref` | 正向 | 补发后再 `emit_node(passed, with_output_ref(Some("exec-x")))` → 终态拍 `output_ref` 与补发拍同值（同 id 收口，AC-2） | 单次 |
| `StepEmitter::attach_output_ref` | 异常 | 目标节点不存在（未先 emit running）→ 补发拍按 emit 序**追加**为一条 `running + output_ref` 新节点（`accumulate_node` 的无命中分支，非契约外崩溃）；此臂只在 walker 漏 emit 时可达，用例只锁「不 panic、不静默丢弃」 | 单次 |
| `StepEmitter::attach_output_ref` | 边界 | `instance_no` 不同（0 与 1）→ 两独立节点各携各自 `output_ref`（同键多实例不串扰）；`exec_id` 为空串 `""` 与超长串（>1000 chars）→ 原样落 `output_ref`（零截断、零改写） | 参数化迭代（`instance_no` 2 × `exec_id` 3 形态） |
| `StepEmitter::attach_output_ref` | 边界 | 补发拍不覆盖在位值：节点已有 `detail` / `session_id` / `token_input` → 补发后三者保持原值（`accumulate_node` 的「some 才覆写」+ 补发只携 `output_ref` 与 `status`） | 单次 |
| `now_millis` | 正向 | 返回值 > 0（epoch 之后），两次连续调用单调不减 | 单次 |
| `now_millis` | 异常 | 时钟早于 epoch 取 0 不 panic——该臂无注入点（时钟缝不开放），记入「不可测试项」的代码审查项；本节只锁「不 panic 且非负」 | 单次 |
| `now_millis` | 边界 | `emit_node(running)` 后 `started_at` 为 `now_millis` 量级（与调用前取样比较：`started_at >= before && started_at <= after`）；终态拍 `ended_at` 非 `None` 且 `>= started_at` | 组合迭代（running × 终态） |
| `emit_node`（搬迁后） | 正向 | 既有 `control_test` 铸造站点零改动全绿（`publish快照面状态机镜像` / `emit经broadcast流出kind通知` / `sample_step` fixture 相关组）——铸造口零行为变化 | 参数化迭代（既有站点全集） |
| `emit_node`（搬迁后） | 异常 | `guard` 对应 run 已除名 → 零 panic、零可写面（既有 `finish单次收口_除名后无残留可改写面` 语义零变） | 单次 |
| `emit_node`（搬迁后） | 边界 | `StepSettle` 六构造 / builder 全组合（`running` / `passed` / `failed` / `stopped` × `with_session` / `with_output_ref` / `with_tokens`）→ 各字段落位正确、`started_at` 恒有值、`ended_at` 仅终态有值 | 组合迭代（4 状态 × 3 builder） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 不 mock：`ChangeFlowControl` | 真实类型 + `#[tokio::test]` 进程内 tokio 原语（broadcast / watch / Mutex）真实组合（既有装置：`begin_run` 登记、`subscribe` 取订阅、`yield_now` 轮询握手） | 全章节 |
| 不 mock：`now_millis` | 真实 `SystemTime`（无时钟注入点；见「不可测试项」的时钟缝说明）——时间断言只用区间（`before <= x <= after`）与单调性，不断言具体值 | 时间戳相关行 |
| 不 mock：`state.rs` 类型 | 真实 `ChangeStepState` / `ChangeStepKind` / `RunUpdate`（线面形态是断言对象） | 全章节 |
| 进程边界 | 本节零 spawn / 零 IPC / 零文件系统 | 全章节 |

### `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` -> `packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs`

#### 待测功能

<!-- design.md「公共函数 / API」表未声明该文件的公共 API 变更（`run_tool` / `StepIdentity` / `node_key` / `begin_node` 均模块私有；`walk_run` 签名不变）。行为面取自 design D6 与「修改文件」表：`run_tool` 增 `identity: Option<StepIdentity>`、14 处调用点 4 `Some` / 10 `None`、`emit_node` / `StepSettle` / `now_millis` 定义移出。 -->

- `walk_run`（脚本步上抛链的端到端观测面）: 四位脚本步（static-check 反馈边 / `on_fail=Terminal`，test-execution 反馈边 / `on_fail=Terminal`）经 `run_tool(identity: Some(..))` 组装 `OutputRefSink`，回调内补发 `running + output_ref` 拍；其余十处工具步传 `None`（零补发拍、零额外通知）；emit 面收口后 56 处调用点行为零变。

既有装置：`NullRunner` / `FailingRunner` / `NullTestExecutionRunner` / `CancelableStaticRunner` / `AlwaysFailStaticRunner` / `HangingIgnoreCancelRunner`（各补末位 `_on_output_ref` 形参）+ 假 store fixture；全部既有站点零改动。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `walk_run` 脚本步上抛 | 正向 | 假 runner 在 `run` 内先 `on_output_ref.emit("exec-x")` 再返回 `output_ref: Some("exec-x")` → 图上该节点先出 `running + output_ref = Some("exec-x")` 拍、终态拍同值，**节点总数 1**（补发拍被就地合并，AC-5 / AC-2） | 单次 |
| `walk_run` 脚本步上抛 | 正向 | static-check 反馈边臂与 `on_fail=Terminal` 臂各一遍：四脚本步全部传 `Some(StepIdentity)`（以「回调收到 `exec_id` 且补发拍 `phase` / `attempt` / `step` / `instance_no` 与该步一致」为判据） | 参数化迭代（4 脚本步） |
| `walk_run` 脚本步上抛 | 正向 | `on_output_ref: None` 的 mock runner（既有 `NullRunner` 等）→ running emit 的 `output_ref` 仍为 `None`、终态按产出回填、零补发拍、零额外通知（「未提供回调即老行为」scenario） | 参数化迭代（`None` 臂 × 既有 mock 全集） |
| `walk_run` 脚本步上抛 | 异常 | runner 在回调**之外**返回 `Err`（`fail_on("static-check")`）→ 补发拍已在、终态按 `Terminal` 收敛，节点 `output_ref` 保持补发值（不因失败被清空） | 组合迭代（补发 × `Err`） |
| `walk_run` 脚本步上抛 | 异常 | 假 runner 调用 `on_output_ref` **两次**（不同 id）→ `accumulate_node` 的「some 才覆写」使 `output_ref` 为第二次值、节点仍 1 条（幂等合并不新增节点；该臂是防御性观测，spec 未声明多次调用） | 单次 |
| `walk_run` 脚本步上抛 | 边界 | 十处非脚本步（archive preflight / phaseNext / phaseStart / mutation-test / archi / phaseLog / preflight / seal / backtrack / decisionLog）→ 传 `None`：零补发拍、`output_ref` 恒 `None`、emit 序与既有断言逐条一致 | 参数化迭代（10 调用点） |
| `walk_run` 脚本步上抛 | 边界 | 上抛时刻文件已在（**跨 crate 组合行**，落点见下）：以真实 `ProcessStaticCheck` + 真实 tempdir 驱动，回调内断言 `exec_output_log_path_in(..).is_file()` 成立（AC-11 的链路端到端判据）。**执行文件为 `packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs`**（design D17：`crates/core/orchestration/Cargo.toml` MUST 零 diff，MUST NOT 加 `checks-runtime` dev-dependency——core → infra 反向边），本行的入口模块判定与用例正文见该文件章节 | 单次 |
| `walk_run` 脚本步上抛 | 边界 | `instance_no` 递增形态（反馈边重跑）：同 `(phase, attempt, step)` 两次执行 → 两节点、各自 `output_ref` 独立、`started_at` 各自首见；「补发拍不扰动既有合并语义」scenario | 组合迭代（两次执行 × `instance_no` 0/1） |
| `walk_run` emit 收口 | 正向 | **既有回归**：`emit_node` / `StepSettle` / `now_millis` 迁出本文件后，既有铸造用例组（时间戳 / token / 会话槽 / 边累积）零改动全绿（AC-13 的「56 处调用点源码零改」判据） | 参数化迭代（既有站点全集） |
| `walk_run` emit 收口 | 异常 | `run_tool` 的 14 处调用点在 `identity: Option<StepIdentity>` 为必填形参后全部编译通过（4 `Some` + 10 `None`）；漏传任一处即编译失败——本行以「全量用例可编译并运行」为判据（编译期契约） | 单次 |
| `walk_run` emit 收口 | 边界 | run 收口时刻的 `finish_command(.., now_millis(), ..)` 仍在位：终态 `ended_at` 非 `None`、与 `now_millis` 同量级（收口时间戳口径零变） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 假 runner 集（`NullRunner` / `FailingRunner` / `NullTestExecutionRunner` / `CancelableStaticRunner` / `AlwaysFailStaticRunner` / `HangingIgnoreCancelRunner`） | 既有测试装置，各自补末位 `_on_output_ref` 形参；新增上抛用例在此之上加**可编程** runner（`Arc<Mutex<Vec<String>>>` 记录 `emit` 与返回 `output_ref`）——它们是 `Arc<dyn …Runner>` 的注入依赖，不是内部模块替身 | 上抛链全部用例 |
| 假 store / 假写面 | 既有 fixture（`ChangeStateStore` 的进程内实现），零改动 | 全章节 |
| `OutputRefSink` / `StepEmitter` | 真实类型（`run_tool` 内组装，测试侧只观测回调效果，不给替身） | 上抛链全部用例 |
| 组合行（walker × infra/checks） | 真实 `ProcessStaticCheck` + 真实 tempdir + 真实 shim（不 mock 跨 crate 边界的任一侧）；**执行位置是壳层 `change_flow/mod_test.rs`**（D17），本节不留置该用例的替身——本节只保留 mock runner 面上的补发拍断言 | 「上抛时刻文件已在」一行（跨节执行） |
| 进程边界 | 仅组合行涉真实 spawn（PATH 经 `crate::lock_test_env()` 串行化）；其余用例零 spawn | 全章节 |

### `packages/desktop/src-tauri/crates/infra/watch/src/lib.rs` -> `packages/desktop/src-tauri/crates/infra/watch/src/lib_test.rs`

#### 待测功能

- `subscribe(path: PathBuf, sender: Sender<FileWatchSignal>) -> Result<Watcher, WatchError>`: 签名不变、导出臂语义收紧——文件缺失（父目录在）仍 `Ok`（监听休眠于父目录，目标出现即生效）；父目录缺失 → `Err(WatchError::ParentUnavailable(缺失父目录))`，MUST NOT 返回永不产信号的 `Ok`。

<!-- design「类型定义」表另声明 `WatchError` 由元组结构体改为 `pub enum WatchError { Notify(notify::Error), ParentUnavailable(PathBuf) }`（`Display` + `Error`）；该枚举的 `Display` 文案由下方「失败面两分」与「文案」两行锁定，`watch_target_of` / `is_target_change` / `parent_of` 为模块私有（见「不可测试项」）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `subscribe` 失败面两分 | 正向 | 文件缺失而父目录在（既有 `目标不存在时订阅返回ok且文件后出现再修改信号生效送达`）→ `Ok`，随后创建文件并修改 → 超时窗口内收到信号（AC-10 的 `Ok` 臂，既有用例零改动仍绿） | 单次 |
| `subscribe` 失败面两分 | 正向 | 父目录缺失（临时目录内取 `missing-dir/note.md`）→ `Err(WatchError::ParentUnavailable(p))` 且 `p` 逐字等于缺失的父目录路径 | 单次 |
| `subscribe` 失败面两分 | 异常 | `WatchError::Notify` 臂仍可构造且 `Display` 文案逐字 `"watch: {notify 错误}"`（既有文案零变）；`std::error::Error` impl 在位（可转 `Box<dyn Error>`） | 单次 |
| `subscribe` 失败面两分 | 异常 | 空路径 `PathBuf::new()` / `"."` 两形态 → `subscribe` 返回 `Err(ParentUnavailable(_))` 而非 panic（`parent_of` 的「根路径无父回退目标自身」分支） | 参数化迭代（空路径 / 当前目录） |
| `subscribe` 失败面两分 | 边界 | 父目录补齐后重订即生效：先 `Err(ParentUnavailable(..))`，随后 `create_dir_all` 该父目录并再次 `subscribe` 同一路径 → 返回 `Ok`；写入目标文件后信号正常到达（无需 refresh、无退避等待） | 单次 |
| `subscribe` 失败面两分 | 边界 | 父目录存在但目标路径的**祖父**链完整嵌套（`a/b/c/note.md`，仅 `c` 存在而其下为空）→ `Ok`；中间层缺失（`a` 在、`a/b` 不在）→ `Err(ParentUnavailable("a/b"))` | 组合迭代（两嵌套形态） |
| `subscribe` 失败面两分 | 边界 | 目标本身是目录（父目录在）→ 返回 `Ok` 且零 panic（本 crate 不做路径类型校验；既有判定本体零改动，此臂只锁「不 panic、不新造语义」） | 单次 |
| `subscribe` 既有语义 | 正向 | **既有回归**：rename-replace 收信号 / 信号仅含 `path` 键 / `Watcher` drop 后零信号 / 删除重建后仍送达 / 短窗口多次修改合并到 1–3 帧（5 例零改动全绿） | 参数化迭代（既有站点全集） |
| `subscribe` 既有语义 | 异常 | 既有失败面零变：目标被删除后**再订阅**（同一路径、父目录仍在）→ 仍返回 `Ok`（不新造 `Err` 变体、不退化为 panic）；退订后（`Watcher` drop）再写文件 → 零信号且零 panic（既有 5 例的负面判据）。**「路径权限不可读」臂按下面「不可测试项」表登记**（Windows 上需 ACL 级构造，跨进程权限不可控） | 参数化迭代（2 形态） |
| `subscribe` 既有语义 | 边界 | 零 fs 副作用与零重试：`subscribe` 前后目标父目录条目集合不变（无 `create_dir`）；源码面无退避 / 重挂状态机（后者属静态审查，见「不可测试项」） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 文件系统 | 真实 tempdir（既有 `TempFile` RAII）+ 真实事件触发（`fs::write` / `rename`）；被监视文件不 mock | 全章节 |
| `notify` 库 | 真实 `recommended_watcher`（不 mock notify）——本 crate 只测订阅组装与失败面两分，不测 notify 自身事件语义（既有约定） | 全章节 |
| `mpsc` | 真实 `std::sync::mpsc` + `recv_timeout(SIGNAL_TIMEOUT = 5 s)` 防悬挂 | 全部信号臂 |
| `ParentUnavailable` 的可构造性 | 真实 `WatchError::ParentUnavailable(PathBuf)` 由 `subscribe` 返回，不经替身构造（除 `Display` 单独用例需要直构枚举变体——那是纯值构造，不是模块替身） | 失败面两分、文案 |

### `packages/desktop/src-tauri/src/commands/changes/mod.rs` -> `packages/desktop/src-tauri/src/commands/changes/mod_test.rs`

#### 待测功能

- `exec_output_path(stores: State<'_, WorkspaceStores>, root: String, change_name: String, exec_id: String) -> Option<String>`: 布局派生 `.log` 绝对路径（照 `explore_doc_path` 同构）；blank root / 开库失败 / miss → `None`。
- `read_exec_command(stores, root, change_name, exec_id) -> Option<String>`: 命令 sidecar 读取；老记录无 sidecar → `None`（面板降级「命令未记录」）。
- `read_exec_output_delta(stores, root, change_name, exec_id, from_offset: u64) -> Option<ExecOutputChunk>`: 增量读（`from_offset = 0` 即全量）；运行中零字节 → `Some { text: "", nextOffset: 0 }`。

<!-- design「类型定义」表另声明出线 DTO `ExecOutputChunk { text: String, next_offset: u64 }`（`Debug, Clone, serde::Serialize, specta::Type` + `rename_all = "camelCase"`）与模块私有 `change_dirs`（D8 的单点抽取）；`read_exec_output` 的接口面零改动由下方「定位单点抽取」行锁定。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `exec_output_path` | 正向 | 落一条真实记录后调用 → `Some(绝对路径)`，路径以 `<exec_id>.log` 结尾且 `is_file()`；与 `exec::exec_output_log_path_in` 同值 | 单次 |
| `exec_output_path` | 正向 | `slot=Baseline` 记录 → 返回 baseline 树路径（双锚探测由 exec 侧承载，命令面零重复逻辑） | 单次 |
| `exec_output_path` | 异常 | blank root 三形态（`""` / 空白串 / `\t`）→ `None` 且零 invoke 副作用；未知 `exec_id` / 非法 `exec_id`（`..` / `a/b` / `C:evil`）→ `None` | 参数化迭代（3 blank + 3 非法） |
| `exec_output_path` | 边界 | worktree-backed change（记录落 worktree 内）→ 经候选序定位后命中；archived（日期前缀目录）→ 亦命中 | 组合迭代（worktree × archived） |
| `read_exec_command` | 正向 | static-check 形态单条命令 / test-exec 形态两段命令 → 逐字返回（多段之间保留 `\n`） | 参数化迭代（1 段 / 2 段） |
| `read_exec_command` | 异常 | 老记录（只有 `.log`、无 `.cmd`）→ `None`（非错误）；blank root / 未知 `exec_id` / 非法 `exec_id` → `None` | 参数化迭代（4 形态） |
| `read_exec_command` | 边界 | 空 sidecar（写入空串）→ `Some("")`（命中但空，与 miss 可分辨）；超长命令（>1000 chars）与含 emoji / `\n` → 逐字返回不截断 | 参数化迭代（空 / 超长 / 特殊字符） |
| `read_exec_output_delta` | 正向 | 全量读：真实记录 → `text` 与 `read_exec_output` 逐字相等、`nextOffset` == 字节长度（AC-4 的命令面判据） | 单次 |
| `read_exec_output_delta` | 正向 | 连续增量：`fromOffset = 0` → 取 `nextOffset` → 再以它调 → 拼接等于全量、`nextOffset` 单调不减 | 单次 |
| `read_exec_output_delta` | 异常 | 未知 / 非法 `exec_id` / blank root → `None`；两次槽位皆 miss → `None` | 参数化迭代（5 形态） |
| `read_exec_output_delta` | 边界 | 运行中零字节（`LiveExec::begin` 后未追加）→ `Some { text: "", nextOffset: 0 }`（非 `null`，与无记录可分辨）；`fromOffset` 取 `文件长度 + 1` 与 `u64::MAX` → 夹 0 重读、`nextOffset == 长度`（零 panic） | 参数化迭代（0 / 长度 / 长度+1 / `u64::MAX`） |
| `read_exec_output_delta` | 边界 | 非 UTF-8 字节 → `text` 以替换符呈现、`nextOffset` 越过该字节（严格推进）；半字符截断 → `nextOffset` 停在字符起点 | 组合迭代（非法字节 × 半字符） |
| `ExecOutputChunk` 出线形态 | 正向 | 序列化为 JSON → 键名为 `text` / `nextOffset`（camelCase），值为字符串与数字；无 `next_offset` 蛇形键 | 单次 |
| `ExecOutputChunk` 出线形态 | 异常 | 序列化不吞非法内容：`text` 含非 UTF-8 来源经 lossy 后的替换符 / 含控制字符 → 序列化为合法 JSON 字符串（逐字保真、零转义丢字节）；`next_offset` 为 `u64::MAX` → 按 `dangerously_cast_bigints_to_number` 口径出线为 `number` 零点丢失（与 `2^53-1` 同口径）；字段缺失不出现（无 `skip_serializing_if`） | 参数化迭代（3 形态） |
| `ExecOutputChunk` 出线形态 | 边界 | `next_offset` 取 `0` / `2^53 - 1` → 序列化为 `0` / 对应 `number`（`dangerously_cast_bigints_to_number` 口径，零 `bigint` 面） | 参数化迭代（0 / 2^53-1） |
| `read_exec_output`（定位单点抽取 D8） | 正向 | **既有回归**：既有 8 例（命中 / 非 UTF-8 lossy / name 定位 / 未命中与非法 id / blank root / 空与超长 / worktree / 归档前缀 / 同名共存）零改动全绿——签名、参数键、返回类型、三条 `None` 语义逐条不变（AC-14 的「既有命令零变」判据） | 参数化迭代（既有站点全集） |
| `read_exec_output`（定位单点抽取 D8） | 异常 | blank root 三形态 / 未知 `exec_id` / 非法 `exec_id`（`..` / `a/b` / `C:evil`）→ `None`（三条 `None` 语义逐条不变，AC-14 的既有失败面判据）；开库失败 → `None` 不 panic | 参数化迭代（6 形态） |
| `read_exec_output`（定位单点抽取 D8） | 边界 | 空记录（`.log` 为 0 字节）→ `Some("")` 与 miss 的 `None` 可分辨；超长（>1 MiB）与非 UTF-8 字节 → lossy 文本逐字等于 `read_exec_output_delta` 的全量 `text`（两命令同口径） | 组合迭代（空 × 超长 × 非法字节） |
| `change_dirs`（D8 抽取的等价性） | 异常 | 抽取后失败面同锚：四命令对同一非法 `exec_id` / blank root 全部返回 `None`（无一命令因走不同定位路径而给出不同失败语义）；未知 change → 四命令一致 `None` | 参数化迭代（3 形态） |
| `change_dirs`（D8 抽取的等价性） | 边界 | 同名 change 共存（测试树与 baseline 树同时存在）与归档前缀（日期目录）两形态 → 四命令仍定位到同一 change 目录；change 名为空串 / 超长 → 一致 `None`（布局派生的边界不外溢为 panic） | 组合迭代（同名 × 归档 × 空名） |
| `change_dirs`（D8 抽取的等价性） | 正向 | 四命令同锚：同一 change 同时存在 test 与 baseline 记录 → `exec_output_path` / `read_exec_command` / `read_exec_output_delta` / `read_exec_output` 四者定位到**同一** change 目录（人工比对四者命中的父目录前缀相等） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `WorkspaceStores` / db | 真实 `WorkspaceStores::open(tempdir)` + `tauri::test::mock_app()` + `app.state::<…>()`（既有装置）；种子经 store change 域操作面建档 | 全章节 |
| 文件系统 | 真实 tempdir + 真实 `exec::record_exec_output` / `LiveExec` 落盘（不手写 `.log` / `.cmd` 内容，避免与 exec 侧口径分裂） | 全章节 |
| 不 mock：`infra/exec` 三件 | 真实 `read_exec_command_in` / `exec_output_log_path_in` / `read_exec_output_delta_in`（命令做 1:1 映射，替身化会让映射断言自证） | 全章节 |
| 不 mock：`workflow::queries::locate_change` | 真实调用（候选序定位是被断言的契约，D8 的抽取等价性判据在此） | worktree / archived / 同名共存 |
| 进程边界 | 本节零 spawn、零 IPC（命令直调，不经 `invoke`） | 全章节 |

### `packages/desktop/src-tauri/src/commands/change_flow/mod.rs` -> `packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs`

<!-- design.md「公共函数 / API」表未声明该文件的公共 API 变更（`change_flow/mod.rs` 本变更**源码零 diff**）。本章节是 design **D17** 的落点登记：AC-11 的「真实双侧」端到端组合用例（真实 `RunGuard` + 真实 `ProcessStaticCheck`）只能落在壳层——该 crate 同时依赖 `orchestration` 与 `checks-runtime`，而 `crates/core/orchestration` 加 `checks-runtime` dev-dependency 即 core → infra 反向边（MUST 零 diff）。被测行为取自 design D6 / D16 / D17 与 AC-11、AC-2、AC-5。 -->

#### 待测功能

- `组合根装配` 的上抛链（入口模块 = 壳层组合根；源码零 diff，仅测试文件增例）: `ChangeFlowControl::begin_run(root, change_id, ..)` 取 `RunGuard` → `RunGuard::emitter()` 得 `StepEmitter` → `OutputRefSink::new(move |exec_id| …)` 交真实 `ProcessStaticCheck::run(.., Some(sink))`；回调内以 `exec_output_log_path_in(change_dir, exec_id).is_file()` 断言「上抛时刻 `.log` 与父目录已在」（AC-11），并以「终态 `output_ref` == 回调收到值」断言同 id 收口（AC-2）。既有同型先例：`组合根装配_staticcheck步默认recorded落单一log并回填引用`（本用例以其骨架扩展，不新建装置）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `组合根装配` 上抛链 | 正向 | `begin_run` 取 guard → `emitter()` → `OutputRefSink::new(..)`（回调体做三件事：计数 +1、断言 `exec_output_log_path_in(..).is_file()`、把 `exec_id` 记入 `Arc<Mutex<Vec<String>>>`） → 真实 `ProcessStaticCheck::run(root, name, slot, cancel, Some(sink))` + 真实平台 shim → 回调恰 1 次且回调内文件存在（AC-11 的链路端到端判据） | 单次 |
| `组合根装配` 上抛链 | 正向 | 同链路 → 终态 `StaticCheckOutcome.output_ref == 回调收到的 exec_id`；`control.snapshot(root, change_id)` 里该步节点 `output_ref` 与之同值且 `status == running` 拍与终态拍引用相等（AC-2 / AC-5 的运行中可点前提） | 单次 |
| `组合根装配` 上抛链 | 异常 | `on_output_ref: None`（老调用形态）→ 链路零行为差异：`output_ref` 仍按 `commit` 回填、`.log` / `.cmd` 形态与 `Some` 臂一致（「未提供回调即老行为」） | 单次 |
| `组合根装配` 上抛链 | 异常 | 命令不可达 / `archi` 槽位拒收两臂 → 回调**零**调用（零 spawn ⇒ 零上抛）、`output_ref == None`、`reports/<slot>/exec-outputs/` 零新增文件 | 组合迭代（不可达 × 槽位拒收） |
| `组合根装配` 上抛链 | 边界 | `crates/core/orchestration/Cargo.toml` 零 diff 的机械判据：`grep -n "checks-runtime"` 零命中（组合落点留在壳层的前提）；本用例的 `#[tokio::test]` 与既有 `组合根装配_*` 三例同文件、同 PATH 锁窗口串行化 | 单次 |
| `组合根装配` 上抛链 | 边界 | 上抛时刻早于 spawn：回调内探测 shim 的首行输出**尚未**出现在 `.log` 中（`read_exec_output_delta_in(.., 0)` 为 `Some { text: "", next_offset: 0 }`），证明顺序为「`begin` → `write_command` → 上抛 → spawn」 | 单次 |
| `组合根装配` 上抛链 | 边界 | `slot = Baseline` → `.log` / `.cmd` 落 `reports/baseline/exec-outputs/`、回调内 `exec_output_log_path_in` 命中的是 baseline 树路径（槽位锚点不串树） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 不 mock：`ChangeFlowControl` / `RunGuard` / `StepEmitter` | 真实类型（`begin_run` 登记 + `Arc` 共享；`#[tokio::test]` 进程内广播原语真实组合） | 全章节 |
| 不 mock：`ProcessStaticCheck` | 真实 `checks_runtime::ProcessStaticCheck::new()`（零参构造）——跨 crate 边界任一侧替身化都会让 AC-11 自证 | 全章节 |
| 不 mock：文件系统 / config / 子进程 | 真实 `Env` tempdir + 真实 `openspec/config.json`（`static_analysis` 登记 shim 裸名）+ 真实平台 shim（`write_static_shim`） | 全章节 |
| `OutputRefSink` | 真实 `OutputRefSink::new(..)`（D16 的 `pub` 构造面是本落点可写的前提）；回调体写 `Arc<Mutex<Vec<String>>>` + 计数 | 上抛链全部用例 |
| 进程级 PATH | 既有 `prefix_path(..)` RAII 窗口（`PATH_LOCK` 串行化），不属模块替身 | 全部 spawn 用例 |
| 不 mock：`orchestration` / `checks-runtime` 的 crate 边界 | 真实组合（壳层是仓库唯一同时依赖双侧的位置；D17 的机械判据是 `crates/core/orchestration/Cargo.toml` 零 diff） | 全章节 |

### `packages/desktop/src-tauri/src/commands/mod.rs` -> `packages/desktop/src-tauri/src/bindings/mod_test.rs`

#### 待测功能

<!-- design.md「公共函数 / API」表未声明该文件的公共 API 变更（`all_commands!` 是宏登记面，非函数）；变更取自 design「修改文件」表：`all_commands!` 在 `read_exec_output` 之后增三行（45 → 48 条）。该文件的登记面判据一直落在 `src-tauri/src/bindings/mod_test.rs`（`COMMAND_NAMES` / `COMMAND_WRAPPERS` 两份清单 + 总数锚 + 尾锚），本文件无独立 `mod_test.rs`。 -->

- `all_commands!` 登记面（经 `bindings/mod_test.rs` 的两份清单与生成物逐字断言观测）: 在 `commands::changes::read_exec_output` 之后新增 `exec_output_path` / `read_exec_command` / `read_exec_output_delta` 三行；builder 注册与 `generate_handler!` 同源；总数 45 → 48。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `all_commands!` 登记 | 正向 | `COMMAND_NAMES` / `COMMAND_WRAPPERS` 各含三个新名字（`exec_output_path` / `read_exec_command` / `read_exec_output_delta` 与 `execOutputPath` / `readExecCommand` / `readExecOutputDelta`），生成物逐条命中 | 参数化迭代（3 命令） |
| `all_commands!` 登记 | 正向 | 总数锚 45 → 48（两份清单持衡、一一对应）；尾锚仍为 `archi_drift`（三行插入在 changes 组内，尾锚零位移） | 单次 |
| `all_commands!` 登记 | 异常 | 唯一登记：三新名字在两清单与生成物中各恰 1 次（无重复登记）；`read_exec_output` / `readExecOutput` 的后接形态未被破坏（既有条目零位移） | 参数化迭代（3 命令） |
| `all_commands!` 登记 | 边界 | 登记序：三行紧跟 `read_exec_output`（生成物内 `readExecOutput` 之后即为 `execOutputPath` / `readExecCommand` / `readExecOutputDelta`，与 `all_commands!` 同序） | 单次 |
| `all_commands!` 登记 | 边界 | 三命令 typed 包装逐字：`execOutputPath: (root: string, changeName: string, execId: string) => __TAURI_INVOKE<string \| null>("exec_output_path", { root, changeName, execId })` 等三行形态（参数键 lowerCamelCase、返回类型 `Promise<…>`/`null` 面） | 参数化迭代（3 包装） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 生成物 | 真实 `bindings.ts` / `bindings.zod.ts`（`authoritative_snapshot()` 读盘快照 + 既有跨进程文件锁 `lock()` 串行化）；不手写产物、不 mock 产物 | 全章节 |
| 不 mock：`all_commands!` | 真实宏（清单来源即被测登记面） | 全章节 |
| 进程边界 | 本节零 IPC / 零 spawn；文件系统只读生成物与 `dto.ts` shim | 全章节 |

### `packages/desktop/src/types/generated/bindings.ts` -> `packages/desktop/src/lib/ipc-contract.test.ts`

#### 待测功能

<!-- design.md「公共函数 / API」表未声明该文件的公共 API 变更（它是 `bindings:export` 生成物，MUST NOT 手写；design「修改文件」表记其重导出三命令 typed 包装与 `ExecOutputChunk` 类型）。既有前端契约等价面测试 `packages/desktop/src/lib/ipc-contract.test.ts` 经命令键遍历自动覆盖新命令，故不新增测试文件；本文件是等价断言的**类型主权端**（schema 端见 `bindings.zod.ts` 章节），产物逐字面锚（命令名 / 包装名 / 总数 45 → 48 / 尾锚）另由 `packages/desktop/src-tauri/src/bindings/mod_test.rs` 承接。 -->

- `commands`（生成物类型表）: 三新命令（`execOutputPath` / `readExecCommand` / `readExecOutputDelta`）的 typed 包装签名与 `ExecOutputChunk` 类型在案；`readExecOutput` 逐字不变。
- 类型对位面（等价断言另一端）: `Parameters<typeof commands[k]>` / `Awaited<ReturnType<typeof commands[k]>>` 是 schema 推导面的唯一对位参照（类型主权不转移）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `commands` 新命令面 | 正向 | `Object.keys(commands)` 含三新命令且各恰 1 次；`commands.execOutputPath` / `commands.readExecCommand` 形参元组为 `[string, string, string]`，`commands.readExecOutputDelta` 为 `[string, string, string, number]` | 参数化迭代（3 命令） |
| `commands` 新命令面 | 正向 | `Awaited<ReturnType<typeof commands.readExecOutputDelta>>` 与 `ExecOutputChunk` 互相可赋值（`MutualAssignable` 类型断言，`vp check` 的 typeCheck 执行） | 单次 |
| `commands` 新命令面 | 异常 | 反例校准：断言 `readExecOutputDelta` 的返回类型**不**等于窄化的 `{ text: string; nextOffset: number }` 之外形态（如 `text: number`）——`MutualAssignable` 为 `false`，证明等价断言非恒真、`any` 不静默通过 | 参数化迭代（2 反例） |
| `commands` 新命令面 | 边界 | `Option` 出线为 nullable：`readExecOutputDelta` / `readExecCommand` / `execOutputPath` 的返回类型均含 `\| null`；`nextOffset` 为 `number`（非 `bigint`） | 参数化迭代（3 命令） |
| `commands` 新命令面 | 边界 | `ExecOutputChunk` 的空面类型：`text` 接受空串 `""`、`nextOffset` 接受 `0`（类型层零窄化，空记录形态可表达） | 单次 |
| 既有命令零变 | 正向 | `Parameters<typeof commands.readExecOutput>` / `Awaited<ReturnType<typeof commands.readExecOutput>>` 与既有断言逐字相等；重导出后 `readExecOutput` 生成行一行未动（既有前端调用点除 `use-exec-output.ts` 外零改动） | 单次 |
| 既有命令零变 | 异常 | 反例校准：若把 `readExecOutput` 的返回类型写成 `string`（丢 `\| null`），`MutualAssignable` 断言即判 `false`——证明「逐字不变」断言非恒真 | 单次 |
| 既有命令零变 | 边界 | 空态与 miss 形态：`readExecOutput` 返回 `null` 与返回 `""` 两形态在类型层可分辨（`string \| null` 未被压成 `string`）；`nextOffset` 类数值字段保持 `number` 而非 `bigint`（既有命令与三新命令同口径） | 组合迭代（`null` × 空串） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 不 mock：生成物与 zod | 真实 `import` `bindings.ts` / `bindings.zod.ts` / `zod`（真实单实例是 `.meta()` / `instanceof` 可观测的前提） | 全章节 |
| 不 mock：`ipc.ts` | 真实代理层（形参元组与 `ipc` 面等价是本节判据之一，替身化即失去判据） | 形参元组行 |
| 进程边界 | 本节零真实 IPC / 零 spawn；只做静态类型与键集断言 | 全章节 |

### `packages/desktop/src/types/generated/bindings.zod.ts` -> `packages/desktop/src/lib/ipc-contract.test.ts`

#### 待测功能

<!-- design.md「公共函数 / API」表未声明该文件的公共 API 变更（同一生成物对，见 `bindings.ts` 章节）；本文件承担等价断言的**schema 端**：三表（`argsSchemas` / `resultSchemas` / `channelArgs`）自动纳入三新命令与 `ExecOutputChunk` schema。 -->

- `argsSchemas` / `resultSchemas` / `channelArgs`（生成物三表）: 三新命令自动入册，键集与 `Object.keys(commands)` 一致；`ExecOutputChunk` schema 接受 `null`（miss）与空文本形态。
- 结果面校验分级: `readExecOutputDelta` 的脏结果只 soft-report 漂移、原值透传不 throw（既有分级策略零变）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 生成物三表等价 | 正向 | `Object.keys(argsSchemas / resultSchemas / channelArgs)` 与 `Object.keys(commands)` 完全一致（含三新命令；既有 `it.each(commandKeys)` 自动扩展，零用例改动即覆盖） | 参数化迭代（全部命令） |
| 生成物三表等价 | 正向 | 三新命令的 `argsSchemas` 形参个数 == `commands[k].length`（`execOutputPath` / `readExecCommand` 三参，`readExecOutputDelta` 四参）；形参名为唯一 lowerCamelCase（`root` / `changeName` / `execId` / `fromOffset`） | 参数化迭代（3 命令） |
| 生成物三表等价 | 异常 | `readExecOutputDelta` 结果面脏（缺 `nextOffset` / `text` 给数字）→ 结果面 soft-report 记 1 条漂移、原值透传不 throw（既有分级策略零变） | 参数化迭代（2 脏形态） |
| 生成物三表等价 | 边界 | `readExecOutputDelta` 返回 `null`（miss）→ 结果面零漂移（`Option` 出线为 nullable，schema 接受 `null`）；`{ text: "", nextOffset: 0 }` → 零漂移（空记录合法形态） | 组合迭代（miss × 空增量） |
| `ExecOutputChunk` schema 面 | 正向 | `z.infer<typeof resultSchemas.readExecOutputDelta>` 与 `Awaited<ReturnType<typeof commands.readExecOutputDelta>>` 互相可赋值（`MutualAssignable` 类型断言，`vp check` 的 typeCheck 执行） | 单次 |
| `ExecOutputChunk` schema 面 | 异常 | 反例校准：脏对象 `{ text: 42, nextOffset: "7" }` 被 `resultSchemas.readExecOutputDelta` 判为失败 → `ipc` 层 soft-report 记 1 条漂移、原值透传不 throw（schema 不静默接受异型）；`{ text: "a" }`（缺 `nextOffset`）同样记 1 条 | 参数化迭代（2 脏形态） |
| `ExecOutputChunk` schema 面 | 边界 | `nextOffset` 为 `number`（非 `bigint`）：`expectTypeOf(...).toEqualTypeOf<number>()` 面等价；`fromOffset` 入线 schema 为必填 `number`（零 `optional` 放宽），缺失即被 strict 拒 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 不 mock：生成物与 zod | 真实 `import` `bindings.zod.ts` / `bindings.ts` / `zod`（schema 实例与 `.meta()` 可回读是被测面） | 全章节 |
| 不 mock：`ipc.ts` / `ipc-drift.ts` | 真实代理与真实漂移统计（soft-report 分级是既有实现，替身化会让「零漂移 / 恰 1 条」自证） | 三表等价与脏结果行 |
| 进程边界 | 本节零真实 IPC / 零 spawn；断言只读 schema 与键集 | 全章节 |

### `packages/desktop/src-tauri/crates/infra/checks/src/mutation/cargo_mutants.rs` -> `packages/desktop/src-tauri/crates/infra/checks/src/mutation/cargo_mutants_test.rs`

**既有测试文件**（`cargo_mutants_test.rs` 已在盘上并被源文件尾部 `#[cfg(test)] #[path]` 挂载），本章节为**纯回归登记**：不得新增 `/tmp`-级别的一次性脚本，也不改既有用例的断言行。

#### 待测功能

<!-- design.md「公共函数 / API」表未声明该文件的公共 API 变更（`pub(crate) run_segment(..)` 签名不变、`MutationRunner` 不在本轮扩参面）；变更面只有 `exec::RunOptions` 构造点各补一行 `live: None`（编译面涟漪，design D14 与待决问题 7）。故本条目按 design「修改文件」表显式标注为**行为面回归登记**，不是新增 API。 -->

- `run_segment()`（回归登记）: 突变段**零 live 接入**——`exec::RunOptions { .. }` 构造补 `live: None` 单行、argv 常量面与报告清场、`SEGMENT_SPAWN_TIMEOUT_MS` / `PER_MUTANT_TIMEOUT_SECS`、零重试「`retry: exec::RetryPolicy::none()`」与段归因（`SegmentFailure` → `Fatal` / `Interrupted` / `Cancelled`）逐字不变（Q2 裁定：突变段不纳入 live 记录面）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `run_segment` 零 live | 正向 | **既有回归**：`cargo_mutants_test.rs` 全绿且断言行零改动——`argv模板常量面锚定` / `jobs计算_半cpu关系式抽样口径` / `spawn模板展开_argv逐参等值与报告权威落位` / `cargo_target_dir多段共享_env断言同一不重置` / `退出零无报告_零突变体段合成空计数面` / `陈旧报告清场_上一段残留不误读` / `补丁入参边界_空串超大与特殊字符不破坏` / `段中断错误面_双段装配接线` | 参数化迭代（既有站点全集） |
| `run_segment` 零 live | 异常 | spawn 失败 / 超时中断（`强制终止且 pid 消失不挂死`）/ 运行中 cancel（`段取消变体与文案且秒级收敛`）/ 已置位 cancel（`入口短路零 spawn`）四臂的归因（`Fatal` `Err` / `Interrupted` → `conclusion = Error` / `Cancelled` → `fault = Stopped`）与本变更前逐字一致；`live: None` 不改变任何一条错误串 | 参数化迭代（4 臂） |
| `run_segment` 零 live | 边界 | 非零退出中断（`携退出码与输出尾部捕获`）的输出尾部仍有界（既有有界截断判据零变）——`live: None` 未把 stdout 改道，故尾部捕获路径逐字等价 | 单次 |
| `run_segment` 零 live | 边界 | 突变段执行前后对报告树做文件快照对比 → `reports/<slot>/mutation/` 下**零** `<exec_id>.log` / `<exec_id>.cmd` 新增、零 `exec-outputs/` 目录创建（Q2 裁定的运行期判据） | 单次 |
| `run_segment` 零 live | 边界 | 写操作幂等：同一 `patch_path` 连续两次段执行 → `mutants.out` 被前次清场后重建（不叠加残留）、`outcomes.json` 每次重读为当次权威、报告树零累积 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 不 mock：文件系统 / 子进程 | 既有装置（真实 tempdir + 平台 shim 脚本 + `PathWindowGuard`），argv 经 `SpawnRecord` 记录真值 | 全章节 |
| 不 mock：`exec::run_shell` | 真实调用（`live: None` 的透传语义正是被测面，替身化会让「零 live 记录」自证） | 全章节 |
| 进程边界：真实 `cargo` | 平台 shim 脚本替代真实 `cargo` 可执行（运行环境替身），断言的是 argv 与退出面而非真实突变分析 | spawn 系用例 |

### `packages/desktop/src-tauri/crates/infra/checks/src/mutation/stryker.rs` -> `packages/desktop/src-tauri/crates/infra/checks/src/mutation/stryker_test.rs`

**既有测试文件**（`stryker_test.rs` 已在盘上并被源文件尾部 `#[cfg(test)] #[path]` 挂载），本章节同为**纯回归登记**。

#### 待测功能

<!-- design.md「公共函数 / API」表未声明该文件的公共 API 变更（`pub(crate) run_segment(..)` 签名不变）；变更面只有 `exec::RunOptions` 构造点补一行 `live: None`（design D14 与待决问题 7）。故按 design「修改文件」表显式标注为行为面回归登记。 -->

- `run_segment()`（回归登记）: Stryker 段**零 live 接入**——`exec::RunOptions { .. }` 构造补 `live: None` 单行；临时 config 组装（`mutate` / `tempDirName` / `concurrency` 半 CPU）、`npx --prefix <suite cwd> stryker run <config>` 模板展开、`ensure_on_path("npx")` 前置、合并流大块逐字节与「瞬态签名不触发段重试」逐字不变（Q2 裁定）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `run_segment` 零 live | 正向 | **既有回归**：`stryker_test.rs` 全绿且断言行零改动——`临时config组装_共享sandbox与半cpu与json直写` / `runner与插件包映射_三框架封闭` / `清单相对化_cwd内外与特殊目录名` / `清单边界_空单超大与suite_config块` / `ensure_on_path_不可达显式err可达ok` / `spawn模板展开_argv逐参等值与cwd捕获` / `合并流与大输出_双管道大块逐字节` | 参数化迭代（既有站点全集） |
| `run_segment` 零 live | 异常 | `ensure_on_path("npx")` 不可达 → `SegmentFailure::Fatal` 显式 `Err`（零 spawn）；非零退出中断与超时中断（`强制终止且 pid 消失不挂死`）的输出尾部捕获与本变更前逐字一致 | 参数化迭代（3 臂） |
| `run_segment` 零 live | 异常 | `输出含瞬态签名不触发段重试`：段内零重试（`retry: exec::RetryPolicy::none()`）下同一段输出含瞬态签名仍只执行一次；`live: None` 未引入第二条 drain 路径，故判定臂逐字等价 | 单次 |
| `run_segment` 零 live | 边界 | 运行中 cancel（`段取消变体且临时 config 保留`）/ 已置位 cancel（`入口短路零 spawn 且临时 config 在位`）两臂：`live: None` 不改变临时 config 的保留与短路语义（`fault = Stopped`） | 参数化迭代（2 臂） |
| `run_segment` 零 live | 边界 | 段执行前后对报告树做文件快照对比 → `reports/<slot>/mutation/` 下**零** `<exec_id>.log` / `<exec_id>.cmd` 新增、零 `exec-outputs/` 目录创建（同 cargo-mutants 段口径的运行期判据） | 单次 |
| `run_segment` 零 live | 边界 | 写操作幂等：同一 `mutation_dir` / 同一段序两次段执行 → 临时 config 按同一路径覆写为等价内容（零残留累积、不产生第二份 config）、报告路径逐次重写为当次权威 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 不 mock：文件系统 / 子进程 | 既有装置（真实 tempdir + 平台 shim 脚本充当 `npx` + `PathWindowGuard`） | 全章节 |
| 不 mock：`exec::run_shell` | 真实调用（`live: None` 与零重试的透传语义是被测面） | 全章节 |
| 进程边界：真实 `npx` / `stryker` | 平台 shim 脚本替代（运行环境替身），断言 argv 模板展开与退出面，不执行真实突变分析 | spawn 系用例 |
| `UnusedSeamRunner`（`mutation/mod_test.rs` 的既有 mock） | 既有装置，`MutationRunner` 两 trait 各补末位 `_on_output_ref` 形参（编译面涟漪，行为断言零改） | 装配面既有站点 |

### `packages/desktop/src/views/changes/hooks/use-exec-output.ts` -> `packages/desktop/src/views/changes/hooks/use-exec-output.test.ts`

#### 待测功能

- `useExecOutput(params: { root: string | null; changeName: string | null; execId: string | null; status: ChangeStepStatus | null }): UseExecOutputResult`: 命令头行一次读（`ipc.readExecCommand`，失败不阻断 `text`）+ 内容面 offset 增量（`ipc.readExecOutputDelta`，`fromOffset` 仅取自后端 `nextOffset`）+ `status === 'running'` 时才挂 `useFileWatchSignal`（`armKey = root + "\u0000" + execId`，否则 `null` ⇒ 零订阅）+ `status` 转非 running ⇒ offset 归零整读一次；内容面 effect 每次臂起点先 `offsetRef = 0` + `setText(null)` + `setError(null)` 再发起装载首拍 ⇒ **臂内 reject 时 `text` 恒为 `null`**（D10：「每臂起点 `offsetRef = 0` 装载首拍」，MUST NOT 写成「保留上一臂 / 运行中累积文本」）；全部经 `ipc` 代理，MUST NOT 裸 `commands.*`。**同臂单飞守卫**（D10-a，AC-4 / AC-6 的必要条件）：`readDelta` 入口在 `pendingRef` 为真时**不叠读、不动 `offsetRef`**，只置 `queuedRef` 即返回；结算（`then` / `catch`）**先**清 `pendingRef` **再**推 `offsetRef` 与追加文本；`then` / `catch` 末尾发现 `queuedRef` 为真则清零并补**恰一拍**尾读（队列长度恒 ≤ 1，不随信号数增长）；臂切换与 cleanup 三枚 ref 一律清零（在飞响应由 `disposed` 丢弃）。

既有测试文件**全文件重写**（`UseExecOutputResult` 去 `loading`，design 待决问题 4 明示既有断言需改写）；装置沿用 `vi.hoisted` 的 `invokeMock` + `ChannelMock`（`deliver` / `instances`）+ `vi.useFakeTimers()`。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `useExecOutput` 装载首拍 | 正向 | 挂载（`status = 'running'`）→ `read_exec_output_delta` 恰 1 次且 `fromOffset` 为 `0`；`text` 为返回 `text`、零额外任务 | 单次 |
| `useExecOutput` 装载首拍 | 正向 | 挂载（`status = 'passed'`，终态打开）→ 同样首拍 `fromOffset = 0` 全量读 1 次；**零** `watch_subscribe`（`armKey` 为 null） | 单次 |
| `useExecOutput` 装载首拍 | 正向 | 命令头行：`read_exec_command` 恰 1 次（入参 `root` / `changeName` / `execId` 逐字）；返回串原样进 `command`、多行与 emoji 不截断 | 单次 |
| `useExecOutput` 装载首拍 | 异常 | `read_exec_command` reject → `command` 为 `null`、`text` 照常渲染、`error` 保持 `null`（命令失败不阻断内容面，AC-9） | 单次 |
| `useExecOutput` 装载首拍 | 异常 | `read_exec_output_delta` reject → `error` 为拒绝串（`String(cause)` 口径）、`text` 为 `null`（D10 的每臂起手 `offsetRef = 0` + `setText(null)` 已执行，首拍无前值可保留、实现亦不做跨臂文本继承），面板不炸图（AC-8） | 单次 |
| `useExecOutput` 装载首拍 | 边界 | 任一入参 `null`（三形态：`root` / `changeName` / `execId`）→ 零 `invoke`（三命令均零调用）、`command` / `text` / `error` 全 `null`、`offsetRef` 归 0 | 参数化迭代（3 形态） |
| `useExecOutput` 装载首拍 | 边界 | 返回 `null`（miss：未知 / 非法 `exec_id`）→ `text` 保持 `null`（空态语义，不写 `""`）；返回 `{ text: "", nextOffset: 0 }` → `text` 保持 `null` 且 `offsetRef` 为 0（运行中 0 字节不触发重渲染） | 组合迭代（miss × 空增量） |
| `useExecOutput` 运行中跟随 | 正向 | 信号到达（`ChannelMock.deliver`）→ 500 ms 防抖后 `read_exec_output_delta` 第 2 次，`fromOffset` == 第 1 次的 `nextOffset`；`text` 追加（AC-6） | 单次 |
| `useExecOutput` 运行中跟随 | 正向 | 两次信号序列（`nextOffset` 10 → 25 → 40）→ 三次增量拼接 == 全量、`fromOffset` 序列为 `0 / 10 / 25`（offset 只取自后端，前端零字节计数） | 单次 |
| `useExecOutput` 运行中跟随 | 正向 | `armKey` 断言：`status = 'running'` 时 `watch_subscribe` 恰 1 次、首参为 `ChannelMock` 实例、次参为 `exec_output_path` 返回的派生路径（`execOutputPath` 被调用 1 次） | 单次 |
| `useExecOutput` 运行中跟随 | 异常 | `exec_output_path` 返回 `null` → 零 `watch_subscribe`（`resolvePath` 判空），内容面照常（装载首拍已有数据） | 单次 |
| `useExecOutput` 运行中跟随 | 异常 | `watch_subscribe` reject → 静默：`error` 仍 `null`、内容面可用、后续信号零增量（订阅未建立）；零重试 | 单次 |
| `useExecOutput` 运行中跟随 | 边界 | 非 running 三形态（`passed` / `failed` / `stopped`）与 `status = null` → `armKey` 为 `null`：零 `Channel` 构造（`ChannelMock.instances.length === 0`）、零 `watch_subscribe`、零 `watch_unsubscribe`（AC-6 的负面判据 / R-7） | 参数化迭代（4 形态） |
| `useExecOutput` 运行中跟随 | 边界 | 同一 `status` 重渲染（父组件重渲，`resolvePath` / `onSignal` 每次新身份）→ 零重订（`watch_subscribe` 恒 1、`watch_unsubscribe` 恒 0），信号仍由最新 `readDelta` 触发（共享件「不因回调身份变化重订」语义在消费侧的判据） | 单次 |
| `useExecOutput` 终态收敛 | 正向 | `status` 由 `running` 转 `passed` → `watch_unsubscribe` 恰 1 次；`read_exec_output_delta` 再 1 次且 `fromOffset` 为 `0`（offset 归零整读，AC-7）；`text` 被替换为权威内容（非追加） | 单次 |
| `useExecOutput` 终态收敛 | 正向 | 权威内容重排（运行中累积 `"a\nb\n"`，终态整读返回 `"b\na\n"`）→ `text` 逐字等于整读结果，不残留 live 交错序 | 单次 |
| `useExecOutput` 终态收敛 | 异常 | 转终态的整读 reject → `error` 为拒绝串、`text` 为 `null`（D10：臂起点 `offsetRef = 0` 装载首拍前先 `setText(null)`，跨臂不继承上一臂累积值）、`watch_unsubscribe` 仍恰 1 次（spec 只要求 reject 呈错误态，未声明保留文本） | 单次 |
| `useExecOutput` 终态收敛 | 边界 | 生命周期交错：信号在飞（防抖窗口内）时 `status` 转终态 → 旧臂计时被取消（推进 500 ms 后零增量调用），重读只来自新臂的装载首拍（`read_exec_output_delta` 总计 2 次） | 组合迭代（信号在飞 × 状态切换） |
| `useExecOutput` 终态收敛 | 边界 | 卸载（`unmount`）→ 订阅以 `subscription_id` 退订 1 次、零残留计时（推进 1000 ms 零增量），`disposed` 后迟到响应不回写 `text` | 单次 |
| `useExecOutput` 取数纪律 | 异常 | `ipc` 面零漂移：合法入参下 `driftCount()` 为 0（参数面 strict 通过、结果面 schema 通过）；返回脏结果（`nextOffset` 缺键）→ 漂移恰 1 条、原值透传不 throw | 组合迭代（干净 × 脏） |
| `useExecOutput` 取数纪律 | 边界 | 只经 `ipc`：`invokeMock` 收到的命令名集合 ⊆ {`read_exec_command`, `read_exec_output_delta`, `exec_output_path`, `watch_subscribe`, `watch_unsubscribe`}（零裸 `commands.*` 旁路） | 单次 |
| `useExecOutput` 同臂双读（D10-a） | 正向 | **同臂双读 · 在飞折叠为至多一拍尾读**：`deltaGate = makeGate(1)` 挂起第一拍（不 resolve）→ 在飞期间触发**两次** watch 信号（各 `advanceSignalWindow(500)`）→ 断言 `read_exec_output_delta` **仍只调用 1 次**、无第二次调用的 `fromOffset`；放行第一拍后推进 500 ms → 总调用 **2 次**（两信号折叠成一拍尾读）且第二拍 `fromOffset === 第一拍 nextOffset`；最终 `text` 与全量**逐字相等**（MUST NOT 出现同段重复）、`nextOffset` **严格递增**。该臂同时是 AC-4「连续增量拼接 == 全量」与 AC-6「offset 推进」在前端侧的判据 | 单次 |
| `useExecOutput` 同臂双读（D10-a） | 边界 | **同臂双读 · 尾读有界**：在飞期间触发 **5 次**信号（而非 2 次）→ 尾读仍只补 **1 拍**（`read_exec_output_delta` 总调用 2 次）——`queuedRef` 的「布尔队列」语义由「2 次与 5 次信号的调用次数相同」间接锁定（ref 内部状态不可直读，见「不可测试项」）；放行后 `text` 仍与全量逐字相等。与上一行成对：队列长度恒 ≤ 1 | 单次 |
| `useExecOutput` 同臂双读（D10-a） | 异常 | **同臂双读 · reject 后守卫不卡死**：第一拍返回 reject → `error` 为拒绝串；随后一次信号触发的新读**正常发出并成功追加** → 断言 `read_exec_output_delta` 调用次数增至 2、`text` 等于新读内容（`pendingRef` 已清位由「新读能发出」间接锁定——**MUST NOT 卡成永久在飞**，这是守卫最危险的失效形态：标记未清则此后永不再取数） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（`invoke`） | `vi.hoisted` 的 `invokeMock` 按命令名分发（`read_exec_command` / `read_exec_output_delta` / `exec_output_path` / `watch_subscribe` / `watch_unsubscribe`），应答支持「字符串 = 命中、`null` = miss、`Error` = reject」三形态 + 入参记录；**在飞闸门**沿用既有 `makeGate(times)` / `deltaGate`（指定次数的增量应答悬停在 promise 上，`release()` 放行后交付）——同臂双读三臂唯一的时序构造装置，不是模块替身 | 全章节（闸门用于迟到应答与同臂双读三臂） |
| `@tauri-apps/api/core`（`Channel`） | `vi.hoisted` 的 `ChannelMock`（原型 accessor + `deliver(event)` + `instances` 静态表）：信号唯一注入口；`instances` 用于断言「零 Channel 构造」 | 运行中跟随、非 running 边界 |
| 计时器 | `vi.useFakeTimers()` + `vi.advanceTimersByTime(500)`（共享件 `SIGNAL_DEBOUNCE_MS`）；`afterEach` 复位 `vi.useRealTimers()`；`flush()` 辅助（`await act(async () => {})`）冲刷微任务 | 防抖窗口、跨状态切换、卸载 |
| `console` | `vi.spyOn(console, 'warn' / 'error')` 只观测（漂移告警与静默面不污染输出） | 取数纪律行 |
| 不 mock：`src/hooks/use-file-watch-signal.ts` | **必须真实组合**——`armKey` / 退订 / 防抖正是本节被测契约（Change A 零改动）；替身化会让 AC-6 / AC-7 的断言自证 | 全章节 |
| 不 mock：`src/lib/ipc.ts` / `ipc-drift.ts` / 生成物 | 真实组合（`ipc` 代理面是「只经 ipc」判据所在；`driftCount()` 断言依赖真实校验面） | 全章节 |

### `packages/desktop/src/views/changes/flow/stdout-panel.tsx` -> `packages/desktop/src/views/changes/flow/stdout-panel.test.tsx`

#### 待测功能

<!-- design.md「公共函数 / API」表未声明该文件的公共 API 变更（`StdoutPanelProps` 仍不导出；design「类型定义」表记其新增 `status` 字段）。行为面取自 design D11 与「修改文件」表：命令头行 / 三态 / 粘底 / 渲染前 `\r` 归一 / 移除 `loading`。 -->

- `StdoutPanel({ root, changeName, execId, status })`（不导出 props 类型，`status` 新增）: 三段结构（命令头行在最上方 `<pre data-testid="stdout-command">` + `CodeBlockCopyButton`；状态提示；文本区 `<pre data-testid="stdout-text">`）；三态按 `status` 与空文本分派；粘底跟随（`stickRef` + `onScroll` + 文本变化 effect）；渲染前 `normalizeCarriageReturns`；**复制按钮的 import 源为 `@/components/ui/code-block-copy-button`**（design D19-a：MUST NOT 从 `@/components/ui/code-block` barrel import——该 barrel 顶部的 `highlightCode` / `useHighlightedCode` 两条静态再导出会把 shiki 高亮树带入 import 图与覆盖率分母）。

既有测试文件**全文件重写**（`loading` 相关断言与 `stdout-loading` testid 退场）；装置沿用 `vi.hoisted` 的 `invokeMock`（三命令分发）+ `navigator.clipboard` `Object.defineProperty` 打桩（既有装置）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `StdoutPanel` 命令头行 | 正向 | `read_exec_command` 返回两段命令（含 `\n`）→ `stdout-command` 位于面板**最上方**（DOM 首个子元素），文本逐字含两段、`whitespace-pre-wrap` 类在位（多行原样，AC-9） | 单次 |
| `StdoutPanel` 命令头行 | 正向 | `command !== null` → `stdout-command-copy` 在位（`CodeBlockCopyButton`），点击后 `navigator.clipboard.writeText` 收到全文（多行内容原样） | 单次 |
| `StdoutPanel` 命令头行 | 异常 | `read_exec_command` 返回 `null`（老记录）→ `stdout-command` 文本为「（命令未记录）」、复制按钮**零渲染**、内容面照常渲染 | 单次 |
| `StdoutPanel` 命令头行 | 异常 | 复制按钮点击但 `navigator.clipboard` reject → 不 throw、不炸图、内容面不受影响 | 单次 |
| `StdoutPanel` 命令头行 | 边界 | `command` 为空串 `""` → 命令头行渲染空 `<pre>`（命中但空）且复制按钮在位（`null` 与 `""` 可分辨） | 单次 |
| `StdoutPanel` 命令头行 | 边界 | import 源判据（design D19-a）的运行期侧证：面板渲染出复制按钮即证明 `CodeBlockCopyButton` 已从窄模块解析（`data-slot="code-block-copy-button"` 在位）；`code-block.tsx` 的 shiki 树不参与任何断言，其不在 import 图由 `coverage-summary.json` 无该文件键与 `grep` 零命中作为机械判据（见「不可测试项」） | 单次 |
| `StdoutPanel` 三态 | 正向 | `status='running'` 且 `read_exec_output_delta` 返回 `{ text: "", nextOffset: 0 }` → `stdout-running` 文本为「运行中，等待输出…」、零 `stdout-text`、零 `stdout-empty`（AC-8） | 单次 |
| `StdoutPanel` 三态 | 正向 | `status='passed'` 且返回 `null`（miss）→ `stdout-empty` 为「（无输出记录）」、零 `stdout-running`（两文案不混同） | 单次 |
| `StdoutPanel` 三态 | 异常 | `read_exec_output_delta` reject → `stdout-error` 呈现拒绝串、面板骨架与命令头行仍在（不炸图、不阻断抽屉其余面） | 单次 |
| `StdoutPanel` 三态 | 边界 | `status = null` 且返回 `null` → 走「（无输出记录）」而非「运行中」分支（非运行判别）；`status='running'` 且文本非空 → 渲染 `stdout-text`、零 `stdout-running` | 组合迭代（`status` × 文本空/非空四形态） |
| `StdoutPanel` 三态 | 边界 | `root === null` → 零 `invoke`、呈「（无输出记录）」空态（既有语义零变，去 `loading` 后仍成立） | 单次 |
| `StdoutPanel` `\r` 归一 | 正向 | 输出含 `"Compiling a\rCompiling b\rCompiling c\nnext line"` → `stdout-text` 文本为 `"Compiling c\nnext line"`（只留最后一个 `\r` 之后内容，AC-8） | 单次 |
| `StdoutPanel` `\r` 归一 | 异常 | 归一不触碰其它字符：文本含替换符 \uFFFD / \0 / 制表符 / emoji 且无 \r → 渲染结果逐字等于入参（归一零副作用、零转义；AC-8 的「不堆残行」不改动任何其它字节） | 单次 |
| `StdoutPanel` `\r` 归一 | 边界 | `"a\rb"`（行内单 `\r`）/ `"\r"`（仅 CR）/ `""` 三形态 → 分别为 `"b"` / `""` / `""`；`"a\nb"`（无 CR）原样不变（归一纯函数行为经面板观测一遍，直测见 `stdout-text.test.ts`） | 参数化迭代（4 形态） |
| `StdoutPanel` 粘底跟随 | 正向 | 文本增长且 `stickRef` 为 `true`（初始）→ 文本变化 effect 把容器 `scrollTop` 置为 `scrollHeight`（jsdom 下以 stub 的只读属性 + setter 观测） | 单次 |
| `StdoutPanel` 粘底跟随 | 正向 | 用户上滑（触发 `scroll` 使 `isStuckToBottom` 为 `false`）→ 后续文本增长**不**改 `scrollTop`（停止跟随）；再回到底部（`scroll` 后判据为 `true`）→ 恢复跟随 | 组合迭代（跟随 × 停止 × 恢复） |
| `StdoutPanel` 粘底跟随 | 异常 | 容器 ref 为 `null`（未渲染 `<pre>` 时）→ effect 早返、零异常 | 单次 |
| `StdoutPanel` 粘底跟随 | 边界 | 恰好容差边界：`scrollHeight - scrollTop - clientHeight` 为 `1`（跟随）与 `2`（不跟随）两形态 → 判据在 1 px 容差上分界（AC-8 的判据边界，组件侧经 `scroll` 事件观测） | 参数化迭代（1 / 2 px） |
| `StdoutPanel` 既有回归 | 异常 | 错误态优先于空态：`read_exec_output_delta` reject 且文本面为空 → 呈 `stdout-error` 而非「（无输出记录）」（既有降级语义零变，去 `loading` 后仍成立） | 单次 |
| `StdoutPanel` 既有回归 | 边界 | 三态 × 文本面的正交矩阵：`status` 四形态（`running` / `passed` / `failed` / `null`）× 增量四形态（`null` / `""` / 单字符 / 超长）→ 仅「`running` 且空文本」落 `stdout-running`、「非 running 且 `null` 或 `""`」落 `stdout-empty`、其余落 `stdout-text`（16 组合判据） | 组合迭代（4 × 4） |
| `StdoutPanel` 既有回归 | 正向 | 超长输出（>1000 chars）完整渲染不截断；含 emoji 与替换符逐字呈现；原「空串渲染空 `<pre>`」臂改写为三态语义（`status='running'` → `stdout-running`、`status='passed'` → `stdout-empty`） | 参数化迭代（超长 / 特殊字符 / 空串 × 两状态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（`invoke` / `Channel`） | `vi.hoisted` 的 `invokeMock`（三命令分发）+ `ChannelMock`（`status='running'` 时用于推信号验证跟随） | 全章节 |
| `navigator.clipboard.writeText` | `vi.spyOn` / `Object.defineProperty` 打桩（jsdom 无 clipboard）——**运行环境能力替身**，不是内部模块替身 | 命令头行复制两行 |
| 滚动指标（`scrollTop` / `scrollHeight` / `clientHeight`） | jsdom 无布局引擎：经 `Object.defineProperty` 在 `<pre>` 元素实例上打桩三指标（运行环境替身）；`scrollTop` 赋值为可观测 setter | 粘底跟随四行 |
| 不 mock：`use-exec-output.ts` / `stdout-text.ts` | 真实组合——hook 取数规则与纯判据都是被测链路（AC-8 / AC-9 的判据在真实组合上成立才有意义） | 全章节 |
| 不 mock：`@/components/ui/code-block-copy-button` 的 `CodeBlockCopyButton` | 真实组件（复制按钮的形态与可点击性是 AC-9 的断言对象）；**import 源判据**（design D19-a）：`stdout-panel.tsx` 的 import 行 MUST 指向 `@/components/ui/code-block-copy-button`，MUST NOT 指向 `@/components/ui/code-block` barrel（后者顶部的 `highlightCode` / `useHighlightedCode` 两条静态再导出会把 shiki 高亮树带进 import 图与覆盖率分母）；该判据的运行期侧证是本文件用例全绿而 `coverage-summary.json` 里不含 `components/ui/code-block.tsx` | 命令头行两行 |

### `packages/desktop/src/views/changes/flow/stdout-text.ts` -> `packages/desktop/src/views/changes/flow/stdout-text.test.ts`

#### 待测功能

- `normalizeCarriageReturns(text: string): string`: 逐行只保留最后一个 `\r` 之后的内容（`split('\n') → lastIndexOf('\r') → join('\n')`）；纯函数、零副作用。
- `isStuckToBottom(metrics: { scrollTop: number; scrollHeight: number; clientHeight: number }): boolean`: 容差 1 px（`scrollHeight - scrollTop - clientHeight <= 1`）；纯函数、零 DOM 依赖。

**新建测试文件**（依据见篇首「测试布局决策」第 2 条）：`stdout-text.ts` 是本变更两个新增源文件之一（另一个 `code-block-copy-button.tsx` 不新建测试文件，见第 7 条），它无既有测试可追加；co-located 命名沿 `flow/metrics.test.ts` / `flow/run-state.test.ts` / `flow/layout.test.ts` 同型。**本变更新建测试文件因此恰 1 个**。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `normalizeCarriageReturns` | 正向 | 多行 cargo 进度序列 `"Compiling a\rCompiling b\rCompiling c\nCompiling d\rCompiling e"` → `"Compiling c\nCompiling e"`（逐行取最后一段） | 单次 |
| `normalizeCarriageReturns` | 正向 | 无 `\r` 的多行文本 → 逐字返回（恒等变换，含 emoji / 中文 / 替换符） | 单次 |
| `normalizeCarriageReturns` | 异常 | 含替换符 `\uFFFD` / `\0` / 制表符的文本 → 原样保留（归一不触碰其它字符），逐字节等于入参 | 单次 |
| `normalizeCarriageReturns` | 边界 | 空串 `""` → `""`；单 `"\r"` → `""`；`"\r\n"` → `"\n"`（按 `\n` 拆行后逐行归一，换行符本身不被吞）；`"a\r"` → `""` | 参数化迭代（4 形态） |
| `normalizeCarriageReturns` | 边界 | 行首 CR `"\ra\n\rb"` → `"a\nb"`；连续 CR `"x\r\r y"` → `" y"`（只留最后一个之后） | 参数化迭代（2 形态） |
| `normalizeCarriageReturns` | 边界 | 超长文本：单行 >10000 chars（含 20 个 CR）、100 行 × 每行 1 个 CR 两形态 → 输出逐行正确、零截断 | 参数化迭代（2 形态） |
| `normalizeCarriageReturns` | 边界 | 幂等性 `f(f(x)) === f(x)`（对上述各形态抽样） | 组合迭代（幂等 × 4 形态） |
| `isStuckToBottom` | 正向 | `{ scrollTop: 900, scrollHeight: 1000, clientHeight: 100 }` → `true`（差值 0，恰在底部） | 单次 |
| `isStuckToBottom` | 正向 | 顶部 `{ scrollTop: 0, scrollHeight: 1000, clientHeight: 100 }` → `false`；中途 `scrollTop: 450` → `false` | 参数化迭代（2 位置） |
| `isStuckToBottom` | 异常 | 负差值（`scrollTop` 越界大于 `scrollHeight - clientHeight`，如 `{1000, 1000, 100}`）→ `true`（`<= 1` 自然成立，零 panic、零钳制）；`scrollTop` 为负数（`{ -50, 1000, 100 }` ⇒ 差值 950）→ `false`（判据符号自然成立，不加取值域约束） | 参数化迭代（2 形态） |
| `isStuckToBottom` | 边界 | 容差 1 px 分界：差值恰 `1` → `true`；差值 `1.0001` → `false`；差值 `0` → `true` | 参数化迭代（3 形态） |
| `isStuckToBottom` | 边界 | 零与极端尺寸：`{0,0,0}` → `true`；`clientHeight = 0` 且 `scrollHeight` 巨大 → `false`；任一维为 `NaN` → `false`（`NaN <= 1` 为 false，零 throw） | 参数化迭代（3 形态） |
| `isStuckToBottom` | 边界 | 超大数值：`scrollHeight = Number.MAX_SAFE_INTEGER` 且 `scrollTop` 同量级 → 按差值判定、无精度炸裂、不四舍五入 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 不 mock：任何东西 | 两个纯函数零依赖（零 DOM、零 IPC、零 I/O）；用例直接以字面量入参断言返回值 | 全章节 |
| 不 mock：模块导出面 | 真实 import `normalizeCarriageReturns` / `isStuckToBottom`（跨模块生产消费点由 `stdout-panel.tsx` 承担，`knip` 判据因此不违规） | 全章节 |

### `packages/desktop/src/views/changes/flow/detail-drawer.tsx` -> `packages/desktop/src/views/changes/flow/detail-drawer.test.tsx`

#### 待测功能

<!-- design.md「公共函数 / API」表未声明该文件的公共 API 变更（`DetailDrawer` / `DrawerContent` 的 props 面不变）。变更取自 design「修改文件」表与 D11：stdout 分支给 `StdoutPanel` 增传 `status={node.status}`，分派判据与其余分支逐字不动。 -->

- `DetailDrawer`（stdout 分派面）: `node.outputRef !== null` 的分派判据零改动；`StdoutPanel` 收到 `root` / `changeName`（`detail.name`，非 id）/ `execId`（`node.outputRef`）/ **新增 `status`（`node.status`）**；会话转录与列头分派零改动。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `DetailDrawer` stdout 分派 | 正向 | `outputRef !== null` 且 `status = 'running'` 的节点选中 → `StdoutPanel` 挂载并以 `status='running'` 渲染：`stdout-running` 呈「运行中，等待输出…」（`status` 下传接线，AC-5） | 单次 |
| `DetailDrawer` stdout 分派 | 正向 | `status = 'passed'` 且增量返回文本 → `stdout-text` 渲染该文本、零 `stdout-running`（同一节点两状态的分派差异由 `status` 一参决定） | 单次 |
| `DetailDrawer` stdout 分派 | 异常 | `read_exec_output_delta` reject → 抽屉内 `stdout-error` 不炸图、关闭按钮仍可用（既有降级语义零变，仅取数命令由整读换为增量） | 单次 |
| `DetailDrawer` stdout 分派 | 边界 | 分派判据零改动：双 null 节点 → 零 `stdout-panel`、零 `read_exec_output_delta`；`role !== null` → 会话转录、零 stdout 取数（既有四例改写为增量命令面，断言语义不变） | 参数化迭代（4 分派形态） |
| `DetailDrawer` stdout 分派 | 边界 | 取数定式：`read_exec_output_delta` 入参为 `{ root, changeName: NAME, execId: node.outputRef, fromOffset: 0 }`——`changeName` 为展示名（非 change id），逐字断言 | 单次 |
| `DetailDrawer` stdout 分派 | 边界 | **AC-5 组合行（执行落点：`packages/desktop/src/views/changes/change-detail-view.test.tsx`）**：链路 `change-detail-view → change-flow-graph（判据零改）→ detail-drawer → stdout-panel → use-exec-output → ipc 三命令`。以 `status = 'running'` 且携 `outputRef` 的 static-check 节点点击 → 抽屉打开并呈 stdout 面板（命令头行 + 增量输出），推进 500 ms 后信号到达再取增量；`change-flow-graph.tsx` 源码零 diff（静态判据） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（`invoke` / `Channel`） | 既有 `invokeMock`（命令分发改指向三新命令）+ `ChannelMock` | 全章节 |
| 不 mock：`StdoutPanel` / `use-exec-output.ts` / `stdout-text.ts` | 真实组合（design 明示「`SessionTranscriptPanel` / `ArtifactTabs` / `StdoutPanel` 真实组合不 mock」） | 全章节 |
| 不 mock：`change-flow-graph.tsx` | 真实组合（点击判据是被断言项，替身化会让 AC-5 自证） | AC-5 组合行 |
| `data-testid` 钩子 | 真实（`stdout-panel` / `stdout-text` / `stdout-empty` / `stdout-error` / `stdout-command` / `stdout-command-copy` / `stdout-running`） | 全章节 |

### `packages/desktop/src/components/ui/code-block-copy-button.tsx` -> `packages/desktop/src/views/changes/flow/stdout-panel.test.tsx`

**新增源文件 → 既有测试文件**（design D19 / D19-a / 待决问题 14）：该模块是 `CodeBlockCopyButton` 从 `components/ui/code-block.tsx` 的**逐字搬迁**（函数体、样式类名、`data-slot` / `data-copied` / `aria-label` / `sr-only` 播报行、2 s 复位 effect、`clipboard.writeText(..).then(..).catch(() => {})` 全部零行为变更），唯一生产消费方为 `stdout-panel.tsx`。故其测试路径就是面板的命令头行用例组——**不新建测试文件**（理由见「测试布局决策」第 7 条；`knip.json` 的 `ignore` 面同时覆盖 `src/components/ui/**`，不会产生 `Unused exports` 报告）。

#### 待测功能

- `CodeBlockCopyButton(props: React.ComponentProps<typeof Button> & { value: string }): React.JSX.Element`（design「公共函数 / API」表）: `value` 经 `navigator.clipboard.writeText` 写入；成功后 2 s 内呈 `CheckIcon` 并触发 `sr-only` 播报（`aria-live="polite"`），2 s 后复位 `CopyIcon`；`clipboard` 拒绝时 `.catch(() => {})` **静默**（不 throw、不炸图——AC-9 的错误面）；`data-slot` / `data-copied` / `aria-label` / `variant`（缺省 `ghost`）/ `size`（缺省 `icon-xs`）逐字沿用原实现；`onClick?.(event)` 先于剪贴板写入，`...props` 透传。

<!-- design「类型定义」表另声明 `CodeBlockCopyButtonProps`（仍不导出——内联于形参面，与搬出前形态零变）；其形态由下方「props 相位」一行锁定，不构成新增 API 条目。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `CodeBlockCopyButton` 复制面 | 正向 | `command` 为多行串（含 `\n` 与 emoji）→ 面板渲染后点 `stdout-command-copy` → `navigator.clipboard.writeText` 收到**全文逐字**（零截断、零转义；`value` 直传不加工） | 单次 |
| `CodeBlockCopyButton` 复制面 | 正向 | 点击成功后 → 按钮 `data-copied="true"` 在位、`aria-label` 由 `Copy` 转 `Copied`、`sr-only` 播报文本为 `Copied`（`CheckIcon` 分支；AC-9 的可反馈性） | 单次 |
| `CodeBlockCopyButton` 复制面 | 异常 | `navigator.clipboard.writeText` reject → `.catch(() => {})` 静默：零抛错、面板零 `stdout-error`、`data-copied` 不置位（仍 `undefined`）、内容面照常渲染（AC-9「不阻断内容面」） | 单次 |
| `CodeBlockCopyButton` 复制面 | 异常 | 连续两次点击（第二次在首次成功的 2 s 窗口内）→ `writeText` 调用 **2 次**（每次点击都发起写入，不因已在 `Copied` 态而短路）；`copied` 由 `true` 再置 `true` 时 React 不重跑 effect ⇒ **计时不被重置**：以首次成功时刻为零点，推进至 `1999 ms` 仍为 `Copied`、`2000 ms` 归 `Copy`（第二次点击不延长展示窗口） | 单次 |
| `CodeBlockCopyButton` props 相位 | 边界 | `value` 取三形态：`""`（空串）/ 超长（>1000 chars）/ 含 `\n` 与 `\0` 与 emoji → 均原样写入（空串也发起一次 `writeText`）；`command === null`（老记录）时按钮**零渲染**（该臂由 `stdout-panel.tsx` 的 `command !== null &&` 判据承担，窄模块零内部判空） | 参数化迭代（3 形态） |
| `CodeBlockCopyButton` props 相位 | 边界 | 2 s 复位：点击后推进 `2_000 ms` → `data-copied` 归 `undefined`、`aria-label` 回 `Copy`、`sr-only` 文本清空；推进 `1_999 ms` → 仍在 `Copied`（恰在阈值上分界）；组件在复位前卸载 → 计时被 cleanup 清除、零「卸载后 setState」告警 | 组合迭代（1_999 / 2_000 / 卸载前） |
| `CodeBlockCopyButton` props 相位 | 正向 | 透传面：`data-testid` 与其它 `...props` 落在 `Button` 上（`stdout-command-copy` 可被 `getByTestId` 命中）；`variant` / `size` 未显式传时取缺省 `ghost` / `icon-xs`（`data-slot="code-block-copy-button"` 在位）；显式 `onClick` 与内部剪贴板写入**两次都发生**（内部先调 `onClick?.(event)`） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `navigator.clipboard.writeText` | `Object.defineProperty(navigator, 'clipboard', { value: { writeText: vi.fn() } })`——**运行环境能力替身**（jsdom 不实现 Clipboard API），不是内部模块替身；每例 `beforeEach` 重置 | 全章节 |
| 计时器 | `vi.useFakeTimers()` + `advanceTimersByTime(1999 / 2000)` 观测 2 s 复位（与 `useFileWatchSignal` 的 500 ms 防抖用例共用同一套开关；`afterEach` 须 `vi.useRealTimers()` 复位，否则面板既有粘底用例的 `advanceTimersByTime(500)` 与本节 2 s 计时会互相干扰） | 复位边界行 |
| 不 mock：`components/ui/button` / `cn` / `lucide-react` | 真实组合——`data-slot` / `aria-label` / 图标分支是断言对象，替身化会让形态断言自证 | 全章节 |
| 不 mock：`stdout-panel.tsx` / `use-exec-output.ts` | 真实组合（窄模块经面板的真实渲染路径驱动，与「不新建测试文件」的判定一致） | 全章节 |
| 不 mock：shiki 高亮树 | 本模块**零 shiki 依赖**（D19 的全部意义）：`components/ui/lib/highlighter.tsx` / `hooks/use-highlighted-code.ts` / `code-block.tsx` 三者不在 import 图内，故本节零替身可打 | 全章节 |

---

## 不可测试项

| 模块 | 原因 | 替代验证 |
|---|---|---|
| `packages/desktop/package.json`（`version` 0.6.10 → 0.6.11） | 配置 / 元数据，无运行期行为可断言；版本纪律的判据是「一次 patch、`packages/desktop/src-tauri/Cargo.toml` 不跟」 | AC-16：diff 审查（`packages/desktop/package.json` 单行变更 + `Cargo.toml` crate 版本行未动 + `tauri.conf.json` 的 `"version": "../package.json"` 引用行零改） |
| `packages/desktop/src-tauri/crates/infra/exec/Cargo.toml` | 零 diff 面（AC-12 / AC-15）——「正确性主张」就是一行不变；在测试里读 `Cargo.toml` 扫描会把用例绑死在路径布局上 | `git diff --stat` 审查 + `grep -n "serde\|specta\|tokio"` 与改动前逐字比对 + `grep -rn "tokio::fs"` 零命中 |
| `packages/desktop/src-tauri/crates/infra/exec/src/lib.rs` 的模块私有件 `complete_prefix_len` / `exec_command_path_in` | 非导出，无自己的测试路径；为测试新增导出违反 AGENT.md 规则 5（`knip` 判 `Unused exports`） | `complete_prefix_len` 由 `read_exec_output_delta_in` 的「半字符留待下次 / 非法字节不卡游标 / 4 字节字符截断」三组用例全覆盖；`exec_command_path_in` 由 `.cmd` 同目录同 id 前缀与双锚探测两组用例覆盖 |
| `packages/desktop/src-tauri/crates/infra/exec/src/run.rs` 的私有件 `attempt_once` / `await_exit` / `wait_cancel` / `DRAIN_CHUNK_BYTES` | 非导出（`drain` 例外：`run_test` 是同 crate 子模块可直调，已在本文件 `drain` 两行直测） | `attempt_once` / `await_exit` 的 `live` 透传由 `run_shell` 的自然退出臂 / 被杀两臂 / 重试面用例全覆盖（两处调用点同 sink）；`DRAIN_CHUNK_BYTES = 8192` 由 chunk 边界三点（1 / 8192 / 8193 字节）锁定 |
| `packages/desktop/src-tauri/crates/infra/exec/src/lib.rs` 的 `LiveSink::new` / `absorb` 的**跨 crate** 可见性 | 二者为 `pub(crate)`（design D18 的有意收窄，与 `OutputRefSink::new` 转 `pub` 成不对称：`LiveSink` 的消费侧有 `LiveExec::sink()` 这条公共取得路径，无跨 crate 直构需求），`checks` 侧无法构造 sink——这不是不可测，而是「不为尚未存在的消费者开公开构造」的纪律 | crate 内直测（`lib_test` / `run_test` 同 crate 子模块可直构）；跨 crate 面经 `LiveExec::sink()` 的公共路径覆盖（`static_check_test` / `mod_test` / `runner_test`） |
| `packages/desktop/src-tauri/crates/core/orchestration/Cargo.toml` 的零 diff 判据 | 依赖面判据（MUST NOT 加 `checks-runtime` dev-dependency——core → infra 反向边，design D17），运行期不可观测 | `grep -n "checks-runtime" crates/core/orchestration/Cargo.toml` 零命中（静态审查）；运行期侧证是 AC-11 组合用例落在壳层 `change_flow/mod_test.rs` 而非 `walker_test.rs` |
| `testexec/runner.rs` 的 `Reject` 臂在**非 Windows** 平台不可达（design 待决问题 13） | `line_budget()` 在 `#[cfg(not(windows))]` 返回 `None` ⇒ `decide_argv` 首句即 `Keep`，故「`Reject` 臂 `on_command` 零调用」的端到端断言只在 Windows 上成立；为测试拓宽平台门控会给生产代码加测试专用分支 | 两路并列：① Windows 上 `execute_plan` 的 `Reject` 端到端臂（`cfg(windows)` 门控，`on_command` 零调用 + `fault = Environment` + `live` 零写入）；② 全平台 `decide_argv` 直测（同 crate 可见、`budget` 是显式入参，传 `Some(0)` 逼出 `Reject`），以「裁决结果 + `Keep` / `Omit` 两臂的回吐次数」共同锁定该结论 |
| 含 emoji / 超长 `plan.id` 的 `.cmd` 臂 | `plan.id` 由 `checks::aggregate::derive_plan_id(root, framework)` 派生（`\` / `/` → `_`），`detect` 无法产出该形态 | 用例侧以**手工构造 `TestPlan`** 提供（沿 `runner_test.rs` 既有 `unsafe_plan` / `filler` fixture 同型先例）；`write_command` 零改写（D2）故该形态在 `runner_test` / `mod_test` 直接可用 |
| `packages/desktop/src-tauri/crates/infra/exec/src/lib.rs` 的 `is_valid_exec_id` / `mint_exec_id` / `exec_output_dir` / `exec_output_dir_in` | 既有私有件且本变更零改动（design 明示「复用、零改」） | 非法 id 七形态与槽位派生由三只读函数的用例覆盖；`mint_exec_id` 形态由 `exec_id` 定式断言覆盖 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/control.rs` 的 `now_millis` 的**时钟早于 epoch** 臂 | 零参数函数、无时钟注入点（design 明示「零新时钟」）；要测该臂须给生产代码加时钟缝——属投机抽象 | 代码审查（`.duration_since(UNIX_EPOCH).unwrap_or(0)` 单一表达式，无 panic 路径）；运行期只锁「非负 + 单调」（`control_test` 时间戳行） |
| `packages/desktop/src-tauri/crates/core/orchestration/src/control.rs` 的 `StepEmitter` 的 `'static` 约束 | 类型层判据（回调跨 port 边界时不能借用 `&RunGuard`），运行期观测不到 | 编译期机械保证（`BoxToolFuture` 的 `Send` 界 + `OutputRefSink` 的 `'static` 界）；运行期等价判据是「guard drop 后发射器仍可用」一行用例 |
| `crates/core/orchestration` 的「56 处 `emit_node` 调用点源码零改」与「新增 `on_output_ref: None` 的 33 处字面量」 | 源码文本 / 计数判据属 grep、diff 类静态核对；把源文件读进测试扫描与桌面端静态检查重复且脆 | AC-13 / 阶段二自查：diff 审查（`walker.rs` 仅 `use` 一行 + 三处定义删除）+ `grep -c "fn emit_node"` 恰 1 处且位于 `control.rs` + `grep -c "emit_node("` 在 `walker.rs` 为 56；运行期侧证是既有铸造用例组全绿（本文件 `control.rs` / `walker.rs` 章节的「既有回归」行） |
| `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs` 的模块私有件 `StepIdentity` / `node_key` / `begin_node` / `run_tool` | 非导出（`run_tool` 只被 `walk_run` 内部调用） | `identity: Option<StepIdentity>` 的组装正确性由四脚本步（`Some`）+ 十处（`None`）的上抛用例覆盖；14 处必填形参漏传任一处即编译失败（编译期契约） |
| `packages/desktop/src-tauri/crates/infra/watch/src/lib.rs` 的模块私有件 `watch_target_of` / `is_target_change` / `parent_of` | 非导出；`watch_target_of` 的判定本体本变更零改动（只改其 `None` 出口） | 父目录缺失 / 文件缺失两臂 + 父目录补齐后重订 + 嵌套父目录四组用例覆盖 `None` 出口与 `parent_of`；`is_target_change` 由既有五例（rename-replace / 删除重建 / drop 退订等）覆盖 |
| `watch` 的「目标父目录在但路径**权限不可读**」臂（test-design `subscribe` 既有语义异常行的第一形态） | 该形态只在 OS 权限层可达：Windows 需 ACL 级构造（`icacls` 改写 ACE），unix 需在 `chmod 000` 后仍以非 root 运行；CI 容器常以 root 运行 ⇒ 「不可读」不成立，用例在两种运行环境下都不可稳定构造。且本变更对该路径零改动（`watch_target_of` 判定本体不变，只改 `None` 出口），该臂不是本变更的判据面 | 明文登记为**不可测试项**（不在测试代码中写 `skip`）；替代置信：①既有五例的 `Ok` 面零变（`目标缺失ok与深嵌套末级缺失err` / `目标为目录_返回ok且零panic`）；②本变更新增的 `目标删除后再订阅仍ok且信号可达` 锁定「删除后再订阅仍 `Ok`」；③失败面两分由 `父目录缺失显式err且携带缺失父目录` + `连续err不重试_补齐后重新订阅才ok` 完整锁定 |
| AC-10 的「零退避重试 / 零预建目录 / 零递归挂祖先」源码判据 | grep 类静态核对（`create_dir` / `retry` / `backoff` / 递归挂载零命中）；在测试里扫源码与桌面端静态检查重复 | 桌面端静态检查（grep）；运行期侧证是「父目录缺失 → `Err`，补齐后**重订**才 `Ok`（本 crate 不自己恢复）」用例 |
| `packages/desktop/src-tauri/src/commands/watch/mod.rs` 的 `ParentUnavailable` 错误串出线 | 该文件本变更**零 diff**（`map_err(\|e\| e.to_string())` 原样即出新因），无新增行为可测 | diff 审查（零 diff）+ 运行期由 `watch` crate 的 `Display` 文案用例与既有命令错误串模板用例共同锁定 |
| `packages/desktop/src/types/generated/bindings.ts` / `bindings.zod.ts` 的**生成过程**（`bindings:export`） | 生成物是机械产物，正确性判据是「重导后零 diff」而非运行期断言 | AC-14：前置 `bindings:export` 重导出 + 提交前人工核对；运行期侧证是 `packages/desktop/src/lib/ipc-contract.test.ts` 三表等价 + `packages/desktop/src-tauri/src/bindings/mod_test.rs` 的逐字面锚 |
| `packages/desktop/src/views/changes/flow/change-flow-graph.tsx` | 本变更**零 diff**（点击判据一字不动，design D1 的红利）；其行为已被既有 `change-flow-graph.test.tsx` 全覆盖 | diff 审查（零 diff）+ 既有用例全绿 + AC-5 组合行（运行中节点可点）作为「判据未改但可用」的运行期判据 |
| `packages/desktop/src/hooks/use-file-watch-signal.ts` | 本变更**零 diff**（第二个消费方接入不开新参数） | diff 审查（零 diff）+ 既有 `use-explore-doc.test.ts` 全绿 + `use-exec-output.test.ts` 的 `armKey` / 退订 / 防抖用例作为第二消费方的运行期判据 |
| `packages/desktop/src/views/changes/hooks/use-change-detail.ts` | 本变更**零 diff**（驱动源不同：run 内存通知） | diff 审查 + 既有 `use-change-detail.test.ts` 全绿 |
| `packages/desktop/src-tauri/crates/core/workflow/**` / `packages/desktop/src-tauri/crates/infra/store/**` | 本变更**零 diff**（零 DB / 零线格式 / 零 bincode）——「正确性主张」是一行不变 | AC-15：`git diff --stat` 审查 + 既有 golden 语料用例全绿（出现 golden diff 即越界） |
| `packages/desktop/src-tauri/crates/infra/checks/src/mutation/**` 的**零 diff 判据**本身 | design 待决问题 7 明示：`RunOptions.live` 必填使两处构造各补一行 `live: None`，故该目录不是字面零 diff | diff 审查（只允许两行 `live: None`，其余零 diff）+ `cargo_mutants_test` / `stryker_test` 全绿 + 本文件两个 mutation per-file 章节（`packages/desktop/src-tauri/crates/infra/checks/src/mutation/cargo_mutants.rs` 与 `packages/desktop/src-tauri/crates/infra/checks/src/mutation/stryker.rs`，测试文件为同目录同名 `_test.rs`）各一条「零 `.log` / `.cmd` 新增」用例 |
| `packages/desktop/src-tauri/src/commands/changes/mod.rs` 的模块私有件 `change_dirs` / `push_worktree_candidate` / `is_blank_root` | 非导出（`change_dirs` 为 D8 抽取的私有单点） | 四命令同锚等价性一行用例（同一 change 四命令定位到同一目录）+ 既有 `read_exec_output` 八例全绿（抽取等价性的最强判据） |
| 前端 `knip` 死代码判据（`packages/desktop/src/views/changes/flow/stdout-text.ts` 必须有跨模块生产消费点） | 构建期静态扫描，运行期不可观测（`Object.keys(module)` 只给模块结构快照，易漂移） | 桌面端 `knip` 门禁 + 源码事实（`packages/desktop/src/views/changes/flow/stdout-panel.tsx` import 两个函数，design D9 已钉） |
| `packages/desktop/src/components/ui/code-block.tsx`（本变更从其中**搬出** `CodeBlockCopyButton` 并增一条再导出） | 本变更对该文件**零测试目标**，且这是刻意的（design D19）：它在本变更后**不在任何生产 import 图内**（`stdout-panel.tsx` 改指窄模块，全仓无一处从该 barrel import）⇒ 它不参与 `coverage-summary.json` 的分母。**为它新建测试文件本身即违规**——任何 `import { .. } from '@/components/ui/code-block'`（含测试文件）都会经其顶部两条静态再导出把 `lib/highlighter.tsx` / `hooks/use-highlighted-code.ts` 重新拉回 import 图与覆盖分母，正撞 D19 要消除的缺口；为 shiki 高亮链补测试亦被 design 待决问题 14 明令否决（「为分母写测试」），删其再导出被 D19-a 禁（vendored 树内部契约原地保留）。「搬出后行为零变」由窄模块 `code-block-copy-button.tsx` 经函数体逐字迁移承载 | diff 审查（该文件只有三处机械变化：函数段删除、两条失去引用的 import 删除、顶部一条再导出新增与尾部 `export` 块删一项；`useHighlightedCode` / `highlightCode` 两行原地保留）+ `grep -rn "components/ui/code-block['\"]" packages/desktop/src --include=*.ts --include=*.tsx` 零命中（含测试文件）+ `vp check` / `knip` 全绿 + `coverage-summary.json` 无该文件键 |
| 覆盖口径四文件（`packages/desktop/vite.config.ts` / `knip.json` / `stryker.config.json` / `openspec/config.json` 的 `tests[].coverage`）的「零 diff」判据 | 「正确性主张」是这四个文件一行不变（design D14 / D19 的机械判据），运行期不可观测；在测试里读配置扫描与本仓静态检查重复且脆 | diff 审查（`git diff --stat` 输出为空）+ test-execution 相位读 `coverage-summary.json` 复算（`functions` 分母应落 ~1995 量级而非 2031、三文件键不出现）+ 阈值达标（lines 90 / branches 85 / functions 90） |
| `packages/desktop/src-tauri/src/commands/change_flow/mod.rs`（源码） | 本变更**源码零 diff**——D17 只把 AC-11 的端到端组合**用例**落在同目录的 `mod_test.rs`（该文件从「零 live 相关用例」变为「含 `OutputRefSink` 上抛链组合用例」），实现侧无新增行为可测 | diff 审查（`change_flow/mod.rs` 零 diff，仅 `mod_test.rs` 增例）+ 本文件 `change_flow/mod.rs -> mod_test.rs` 章节的七行用例 |
| `packages/desktop/src/views/changes/hooks/use-exec-output.ts` 的三枚 ref 内部状态（`offsetRef` / `pendingRef` / `queuedRef`）本身 | 三者都是 hook 内的 `useRef`，不经 hook 返回面导出（为断言它们新增导出违反 AGENT.md 规则 5，且 `knip` 判 `Unused exports`）；直接读 ref 需要把 hook 实现细节钉进断言 | 行为面等价判据（design D10-a 的「可观测后果」路线）：`pendingRef` 由「在飞期间 `read_exec_output_delta` 恒 1 次调用」锁定；`queuedRef` 由「2 次与 5 次信号的总调用次数相同（均 2）」锁定其布尔队列语义；`offsetRef` 由「各拍 `fromOffset` 序列严格推进且取自上一拍 `nextOffset`」锁定。三者合起来即三条「同臂双读」用例，见 `use-exec-output.ts` 章节用例表末三行 |
| `useExecOutput` 的 `readDelta` 闭包本身（模块私有，非导出） | 无法直调（hook 内部定义，随 effect 实例生成）；为它开导出即 test-only 导出 | 全部经「信号 → 防抖 → 取数」的公共链路驱动（`ChannelMock.deliver` + `advanceSignalWindow`），闭包的三条规则由「同臂双读」三臂 + 既有「迟到应答不回写」用例共同锁定 |
| AC-17 手工冒烟（运行中输出涌入观感、粘底跟随手感、`cargo` 进度行不堆残行、收口重排一次） | 「涌入」与「跟随手感」是渲染观感 + 真实桌面端时序，jsdom + fake timers 只能给状态面等价证据 | 手工冒烟清单；四条已有运行期前置置信：AC-6 用例（信号驱动增量）、AC-7 用例（转终态整读）、AC-8 的两条归一用例、粘底四行用例 |
| AC-14 的全量门禁（`cargo` 全量测试 / `vp check` / `vp test` 覆盖率 thresholds） | 机械门禁步骤，由桌面端静态检查与测试执行相位承担；在测试设计里复述阈值等于复制运行脚本细节 | 门禁执行结果 + 本文件各章节用例对覆盖率阈值的支撑（lines 90 / branches 85 / functions 90）；D19 的覆盖分母判据见上方「覆盖口径四文件」与「`code-block.tsx`」两行 |
