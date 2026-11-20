# 测试设计: archi-file-tree-lazy-load

> **变更**: archi-file-tree-lazy-load（架构页 · 命令不冻结 + 文件树不参与首屏）
> **角色**: test-design-planner（executor）—— 从 `design.md` 推导单元测试范围与公共 API 签名，只设计测试，不写测试命令细节
> **输入**: `proposal.md`（两层根因 / 提案一~四 / 变更范围 4 实现文件 + 测试文件面 5 行 / AC-1…AC-11 / R-1…R-13（AC-11 已由提案修订改写为「验收清单落盘 + 环境不具说明 + 归档后人工验收承接」））、`design.md`（D1–D10 / 八则事实校准 / 架构组件 7 行 / 起点实况核实表（16 行） / **新增文件 0 + 修改文件 4**（零删除）/ 公共函数·API 表 6 行 / 类型定义 0 行 / 配置 1 行 / 测试文件面表 5 行 / 路由·API 表 4 行 / 验收标准回应 11 行 / 待决问题 1..4）、`tasks.md`（三阶段 + 移交清单）、五份 delta spec（`desktop-app-shell` ADDED ×1 含 5 scenario；`desktop-archi-file-tree` MODIFIED ×1 含 11 scenario；`desktop-archi-canvas` MODIFIED ×1 含 6 scenario；`desktop-archi-drift` MODIFIED ×1 含 4 scenario；`desktop-workspace-code-stats` MODIFIED ×1 含 6 scenario），以及工作树源码与既有测试盘点
> **被测范围**: design「变更清单 - 修改文件」**全 4 行**（`packages/desktop/src-tauri/src/commands/archi/mod.rs`、`packages/desktop/src/views/architecture/components/architecture-canvas.tsx`、`packages/desktop/src/views/architecture/components/file-tree.tsx`（仅 `:13` 一行注释）、`packages/desktop/package.json`）。本变更**零新增文件、零删除文件**；无测试映射对的 1 项（`package.json`）与全部零 diff 面登记于「不可测试项」。另设 `packages/desktop/src/app.tsx -> app.test.tsx` 一节，**仅作 AC-5 三入口跨模块组合用例的挂靠点**（design 测试文件面表已列 `app.test.tsx`；`app.tsx` 源码零 diff），按「链路最上层调用方」定入口（design 待决问题 2 的裁决见下）。
> **契约基准**: 用例一律按 `design.md` 的**定稿契约**设计（D1 命令形态 / D2 `*_inner` 逐字不动 / D3 `JoinError` 文案四值与半角冒号 / D5 `#[tokio::test]` 宿主 / D6 源码面计数拆三条 + 四闭包字面量 / D7 缺省值单点 / D8 声明序单点 / D9 缺省外零改动 / D10 版本一行），并沿事实校准 1（双层 `Result` 不是笔误）、2（`JoinError` 臂实际类型 `tauri::Error`）、3（`ProcessArchiveVcs` 装配留 async 体）、4（模块头禁用「四命令」字面量）、5（`16` 计数必改）、6（裸 `#[test]` 可跑 offload 判据）、8（`openspec/specs/**` 零 diff），不按工作树当前形态回填用例。
> **工作树实况核实（2026-11-20，本相位对 design 起点核实的复核）**: `commands/archi/mod.rs` 八命令全 `pub fn`、文件零 `spawn_blocking`、八条 `*_inner` + `check_root` 在位；`mod_test.rs` 1896 行 / **50** `#[test]` / **0** `#[tokio::test]` / 四条 wrapper 调用点 **29** 处分布于 **19** 个测试函数（其中 **9** 个已带 `#[allow(non_snake_case)]`）/ `:916` 的 `block_on` 只服务 runner future / `命令层零领域规则`（`:1164`）的 `pub fn archi_` 计数 **16** / 模块头 stale 禁表（`:1597`）在位；`architecture-canvas.tsx` 的 `'file'` 三处（`options` 文件项 / `useState` 初值 / `TreeMode` 联合）与 `options` 声明序 文件 / 元素 / 契约 在位；`architecture-canvas.test.tsx` **52** `it()`、`switchTo()` 辅助（`:341`）、`callsOf()`（`:231`）在位、`serveGraphAndDrift` 已分派 `archi_file_tree`；`file-tree.test.tsx` **20** `it()` 且直测 `FileTree`；`app.test.tsx` 的 invoke mock **未桩** `archi_file_tree`（缺省 `Promise.resolve(null)`）、`countOf()`（`:333`）在位、archi 章节 **5** 条用例；`crates/core/archi/**` 与 `file-tree.tsx` 全量零改动面；`package.json` 现值 `0.7.4`。
> **口径修订（回溯修复）**: D7 / D8 两条源码自证原写成整表达式逐字邻接（`useState<TreeMode>('element')` 恰 1 处、`mode:` 字面量 `indexOf` 递增），而本变更仅 2 份前端源码在突变范围——Stryker 沙箱把它们插桩成 `useState<TreeMode>(stryMutAct_… ? "" : (…, 'element'))`，邻接消失致 dry run 红（普通 `vp test` 全绿、功能无回归）。现改为**声明锚 + 同行字面量**正则、声明序用显示名 `indexOf`，语义（单点 / 否定 / 声明序）逐条不变；新形态已在**工作树源码与插桩沙箱双侧**逐条验证通过（28 项断言双绿）。详见下文「源码自证口径」节与 `architecture-canvas.tsx` / `file-tree.tsx` 两节的用例行。
> **AC-11 随动（回溯修复 · 2026-11-20）**: AC-11 原写作「六项真机观测门」，在门禁上**不可履行**——63k 工作区在机但无 release 产物，窗口拖动 / 不白屏 / 三命令起止时刻均不落 agent 可读产物（GUI 交互与时刻读数无读取面），重做验收同因再败（先例 `desktop-agent-execution` 边界第 16 条 / 场景「相邻 assistant 消息不设实现期核验门」）。现随 `proposal.md` R-13 / `design.md`「验收标准回应 · AC-11」改写为「`reports/acceptance.md` 落验收清单 + 环境不具说明 + 归档后人工验收承接」，本文件两处落点（`## 验收范围` 的 AC-11 行 / `## 不可测试项` 的 AC-11 行）随之收敛，「design 待决问题」裁决 1 与「AC-2 的时长面」登记行同步改口径。**AC-1 ~ AC-10、被测范围与 `## 单元测试` 三节全部用例零改动**——真机观测从未进入单测面，故测试范围与用例集不变；「不冻结」的唯一稳定机械判据仍是 AC-2 的首次 poll 断言（时长非承诺面，R-10）。

**测试框架与既有约定（盘点结论）**

| 面 | 框架 / 运行器 | 文件形态 | 断言与替身 |
|---|---|---|---|
| Rust（`src-tauri` app crate） | `cargo nextest run --workspace`（`server:check` = `cargo fmt --check` + `clippy --all-targets` 作编译闭合）；`tokio` dev-dependency `features = ["rt","macros"]` 已在案（第 54 行） | 与被测源文件共置的 `<模块名>_test.rs`，由源文件 `#[cfg(test)] mod mod_test;` 声明（`commands/archi/mod.rs` `:43`–`:44` 既有） | `#[test]` / `#[tokio::test]` + 中文长句用例名（含 `archi_` 前缀名配 `#[allow(non_snake_case)]`）+ `assert_eq!` / `assert!` / `expect_err` + 文案子串断言；Tauri 运行时用 `tauri::test::mock_app()`（进程边界装置）；`serde_json::to_value` 逐字段对拍；`snapshot(root) -> Vec<(PathBuf, Vec<u8>)>` 断言零写盘 |
| Rust 既有 async 先例 | 同上 | `commands/changes/mod_test.rs` **23** 个 `#[tokio::test]` 已 await `create_change`（`spawn_blocking` 形态） | 本相位照抄其宿主形态（D5），不新建装置 |
| 前端（`packages/desktop/src`） | `vite-plus` 的 test 面（vitest API，`vp test`）；`vite.config.ts`：`environment: 'jsdom'`、`include: ['src/**/*.test.ts?(x)']`、`setupFiles: ['src/test-setup.ts']` | 与被测源文件同目录共置的 `<模块名>.test.tsx` | `import { describe, expect, it, vi } from 'vite-plus/test'` + `@testing-library/react`（`render` / `screen` / `within` / `fireEvent` / `waitFor` / `act`）；查询一律 `data-testid` 与 `role`；进程边界 mock `@tauri-apps/api/core` 的 `invoke`（`vi.hoisted` + `vi.mock`）；源码自证经 `node:fs` 的 `readFileSync(resolve(process.cwd(), '<相对路径>'), 'utf8')`（沿 `contract-panel.test.tsx:577` / `model-info.test.tsx:912` 定式） |

**用例表列语义**

- **路径类型**：`正向`（契约内输入 / 预期输出）/ `异常`（契约外输入、失败路径、被禁止的写法、反向判据）/ `边界`（空、空串、`null`、极值、恰好阈值、生命周期交错点、越签名脏值）。
- **迭代类型**：`单次`（一次驱动即判）/ `参数化迭代（N …）`（同一断言逻辑对表驱动数据集逐项执行）/ `组合迭代（链路 …）`（两个以上维度交叉）/ `遍历枚举（N 值）` / `表驱动（N 行）`。
- **`测试条件` 内的「既有…」前缀**：该臂已被既有用例锁定，本设计**只做回归登记、不新增断言**；未带前缀的为**新增**或**按新形状改写**（改写只动「取哪条断言路径 / 插哪次 `switchTo`」，语义断言逐条保留——AC-7 / AC-8 / R-3）。
- **参数类型边界口径（贯穿各章节，按 Type → Edge Case 映射）**：`root: String` / `root: &Path` 取空串 `""` / 全空白 `"   "` / 不存在路径 / 指向文件 / 非目录 + 超长（>1000 chars）/ 含空格 / 非 ASCII / emoji；`mode: ArchiDriftMode` 两合法值逐个 + 非法值（枚举 N+1，`+1` 半边不可达，见「不可测试项」）；`App` 返回的 `Result<T, String>` 取 `Ok` / `Err` 双形态；前端 `root: string | null` 取 `null` / `''` / 常规路径；React 侧无 int / float / list / dict 入参（`options` 与 `elements` 是内部常量或既有 props，不属本变更入参面）。
- **offload 判据不接受计时**（AC-2 / spec `desktop-app-shell`）：凡「首次 poll 为 `Pending`」相关行 MUST NOT 出现 `Instant::now` / `elapsed` / `sleep` / `timeout` 一类判据。
- **源码自证 MUST 插桩不敏感**（Stryker 沙箱 / 本次回溯修复）：本变更只有 `architecture-canvas.tsx` / `file-tree.tsx` 两份前端源码在突变范围；沙箱把被突变的表达式就地包成 `stryMutAct_…("N") ? "" : (stryCov_…("N"), <原值>)`，整表达式逐字邻接随之消失 ⇒ 源码自证只许用「**声明锚 + 同行字面量**」正则、声明序用显示名 `indexOf`，MUST NOT 断言 `useState<TreeMode>('element')` / `mode: 'element'` 一类整段逐字邻接（详见下节「源码自证口径」，先例 `use-config-chat.test.ts:601`）。

**源码自证口径（Stryker 沙箱插桩不敏感；本次回溯修复）**

事实（实测，非假设）：D7 / D8 的两处源码自证原写成整表达式逐字邻接，而本变更恰有 2 份前端源码在突变范围（Stryker 日志 `Found 2 of 35257 file(s) to be mutated` / `Instrumented 2 source file(s) with 446 mutant(s)`）。沙箱（`reports/test/…/mutation/_stryker-tmp/sandbox-*/`）把被突变的表达式就地包成三元，逐字邻接消失，dry run 因此只红一条：用例 `源码自证（缺省值单点 / 声明序单点） 应把缺省视图与分段声明序各自单点持有（architecture-canvas.tsx）`，失败文案 `expected [] to have a length of 1 but got +0`（与之对照，普通 `vp test` 10040 条全绿、功能无回归）。

沙箱内实测形态：

| 位置 | 工作树原文 | 沙箱（插桩后） |
|---|---|---|
| `treeMode` 初值 | `const [treeMode, setTreeMode] = useState<TreeMode>('element');` | `… = useState<TreeMode>(stryMutAct_…("191") ? "" : (stryCov_…("191"), 'element'));` |
| `options` 的 `mode` | `mode: 'element',` | `mode: stryMutAct_…("148") ? "" : (stryCov_…("148"), 'element'),` |
| `options` 的 `label` | `label: '元素',` | `label: stryMutAct_…("149") ? "" : (stryCov_…("149"), '元素'),` |
| JSX 派生 / 属性面 | `options.map(` / `<TabsTrigger value={option.mode}>` / `data-testid="archi-tree-mode"` / `role="group"` | **逐字在位**（JSX 骨架与属性名不被包） |

三条锚法（语义不变，只换锚点）：

1. **单点 → 声明锚计数**：`useState<TreeMode>(` 这类「被调用方 + 左括号」插桩后不动，计数判据锚它，不锚整个初始化式。
2. **取值 → 同行字面量**：`/useState<TreeMode>\([^\n]*'element'/` 断言「该声明行内出现 `'element'`」（`treeMode` 初值在沙箱内仍是单行）；否定臂用 `/useState<TreeMode>\([^\n]*'(file|contract)'/` 零命中。
3. **声明序 → 显示名 `indexOf`**：对三钮显示名 `'元素'` / `'文件'` / `'契约'` 取 `indexOf` 判严格递增（三者均 > -1），另加 `/label: [^\n]*'元素'/` 三条同行锚；不锚 `mode: '…'` 的整段邻接。

旧 → 新对照（判据语义逐条不变，单点 / 否定断言一个不少）：

| 判据 | 旧断言（沙箱内红 / 脆） | 新断言（插桩不敏感） | 不变的语义 |
|---|---|---|---|
| D7 缺省值单点 | `useState<TreeMode>('element')` 全源恰 1 处 | `/useState<TreeMode>\(/` 计数 **1** **且** `/useState<TreeMode>\([^\n]*'element'/` 命中 | 全文件仅一处 `TreeMode` 初值声明，初值为 `'element'` |
| D7 否定臂 | `useState<TreeMode>('file')` 零命中 | `/useState<TreeMode>\([^\n]*'(file\|contract)'/` 零命中 | `'file'` / `'contract'` 不作初值（缺省不是文件树） |
| D8 声明序 | 三个 `mode: '…'` 字面量 `indexOf` 递增 | 三个显示名字面量 `indexOf` 递增 + 三条 `label:` 同行锚 | `options` 声明序 = 元素 → 文件 → 契约（缺省值即首项） |
| D7 文件树侧 | `export type TreeMode = 'file' \| 'element' \| 'contract';` 整句逐字 | 声明锚 `export type TreeMode =` 计数 **1** + 三取值字面量 `indexOf` 均大于锚位置 | 三值联合齐备，`TreeMode` 声明仍在 `file-tree.tsx` |

边界：Rust 侧 `include_str!("mod.rs")` 的形态计数**不适用**本条（突变范围只有 2 份前端源码在案），保持逐字断言；前端源码自证凡新增，一律按上式书写。

**待测功能条目口径**

`#### 待测功能` 只列 design「公共函数 / API」表中「所在文件」为本源文件的条目（`commands/archi/mod.rs` **6 行**；`architecture-canvas.tsx` / `file-tree.tsx` **零行**，前端整段标注「零公共 API 变化」，故其条目容器以 HTML 注释登记来源表后留空）。禁 grep 源码导出声明、禁虚构条目。`*_inner` ×8 / `check_root` 在表中占「零改动」那一行（与四条命令的「零改动」行并列），其**行为面**只经 wrapper 的等价用例观测（`*_inner` 是逐字不动面）。

**测试布局决策（源文件 → 测试文件的映射依据）**

1. **零新建测试文件**：`commands/archi/mod_test.rs`（改写既有 1896 行）、`architecture-canvas.test.tsx`（改写既有 52 条）、`app.test.tsx`（追加 3 条）、`file-tree.test.tsx`（**零改动**，回归登记）——四条映射对全部落在既有兄弟测试文件上，沿「同目录共置 + `#[cfg(test)] mod`」定式。
2. **组合用例挂靠按链路发起方**：①「进 `/architecture` → 挂载 → 缺省视图 → 取数计数」的入口是**外壳路由**，故 AC-5 的三入口组合行挂 `app.tsx -> app.test.tsx`；②「左栏分段 → 视图挂载 / 卸载 → `archi_file_tree` 计数」的入口是 `architecture-canvas.tsx`（它持有 `TreeColumn` 与 `treeMode`），故文件树接线 / 失败面 / 栏位 / 声明序组合行全部挂该节；③「命令 future → 阻塞池 → `*_inner`」的入口是命令面，挂 `mod.rs` 节。不新建独立集成测试章节或组合测试区。
3. **不 mock 被测件与被测链路内部**：`ArchitectureCanvas` / `TreeColumn` / `FileTree` / `ElementTree` / `ContractPanel` / base-ui `Tabs` 之间全部真实组合（`TabsPanel` 的 `keepMounted = false` 就是「非活动面板不挂载 ⇒ 首屏零取数」的判据本体，替身化会让 AC-5 自证）；Rust 侧 `spawn_blocking` / `*_inner` / `check_root` / `archi::dto` / `archi::drift::check` / `archi::tree` / `archi::api` 全部真实调用（替身会让「包装 ↔ inner 逐字段一致」自证）。
4. **源码自证是既有手法，不是新发明**：`include_str!("mod.rs")`（Rust）与 `readFileSync`（前端）两种源码文本断言在两部门各有先例；D6 的形态计数、D7 / D8 的单点反证、AC-8 的「否定断言清零」均落在这一手法上（运行期无可观测差异的形态约束，只能源码自证）。前端两处一律按「源码自证口径」的**插桩不敏感**形态书写（声明锚 + 同行字面量 / 显示名 `indexOf`）。
5. **jsdom 环境桩沿用实例级**：`scrollIntoView` 仍是实例级最小桩（`file-tree.test.tsx` 既有，`afterEach` 复原），**不进** `test-setup.ts`；`ResizeObserver` / `offsetWidth` / `DOMMatrixReadOnly` / `getBBox` 度量垫片照抄 `architecture-canvas.test.tsx` 既有形态（环境缺口而非业务 mock）。

**design 待决问题在本相位的裁决**

| # | 待决问题 | 本相位裁决 |
|---|---|---|
| 1 | AC-11 验收留痕的落盘路径（design 待决问题 1） | 非本相位产物；路径已由 proposal AC-11（R-13 修订）**钉死**为 `openspec/changes/archi-file-tree-lazy-load/reports/acceptance.md`（手写、机器不消费），design 待决问题 1 随之收敛、不再走裁决。本相位只登记两处落点：`## 验收范围` 的 AC-11 行 = 「文件在案 + 三节齐备 + 清单六项可操作」；`## 不可测试项` 说明六项真机观测为何不可自动化且 **MUST NOT 作门**（观测并入归档后人工验收、不阻塞归档） |
| 2 | `app.test.tsx` 三入口用例是否必做 | **做**：AC-5 明文列举三入口，而 canvas 层只能以「卸载 + 重挂」等价模拟第三入口（真实 hash 与侧栏进入只有外壳可驱动）；该文件 mock 未桩 `archi_file_tree`，零桩成本 ⇒ 3 条落 `app.test.tsx` |
| 3 | `JoinError` 臂永久不可测 | 按 design 处置：不加注入缝；可测半边（四条文案字面量 + 双层 `Result` 形态）落 `mod.rs` 节用例，运行期触发面登记「不可测试项」 |
| 4 | `archi_graph` 多一次调度带来的时序差异 | 新增 / 改写用例一律以 `waitFor` 收敛（不依赖「首轮 invoke 在同一微任务内 resolve」），MUST NOT 以单次 `await Promise.resolve()` 后紧接同步断言的形式判定落定 |

---

## 验收范围

<!-- 测试区域：Rust 面 `packages/desktop/src-tauri/src/commands/archi/mod_test.rs`（cargo nextest，同 crate 共置）；前端面 `packages/desktop/src/views/architecture/components/architecture-canvas.test.tsx` + `packages/desktop/src/app.test.tsx`（vp test，jsdom）。本变更零新建测试文件、改写 3 个既有测试文件、`file-tree.test.tsx` 与 `src/bindings/mod_test.rs` 零改动。 -->
<!-- 零 diff / 门禁 / 生成物 / 版本行 / spec 成文 / 验收留痕（AC-11：清单在案 + 三节齐备，真机观测不作门）不进入 `## 单元测试`，其验收方式逐条列于「不可测试项」。 -->

| AC ID | 验收条件 | 被测文件或模块 |
|---|---|---|
| AC-1 | **形态**：四条取数命令均 `pub async fn`，命令体内 `spawn_blocking` 且 `*_inner(…)` 位于闭包内；`*_inner` 签名 / 语义 / 错误文案零变化（diff 只看 wrapper 与模块头） | `packages/desktop/src-tauri/src/commands/archi/mod.rs` -> `packages/desktop/src-tauri/src/commands/archi/mod_test.rs`（源码自证：计数拆三条 + 四闭包字面量 + 双层 `Result` 形态）；`*_inner` 逐字不动由同节源码 + 运行期等价用例双承 |
| AC-2 | **非阻塞判据**：四命令 future 的**首次 poll 为 `Pending`**（`Waker::noop()` + `poll`）；MUST NOT 以计时断言代替 | `commands/archi/mod.rs` -> `commands/archi/mod_test.rs`（`assert_offloaded` 四命令各一条 + helper 反向自证一条） |
| AC-3 | **结果等价**：①包装 ↔ `_inner` 同参 deep-equal；②非法 root 四形态文案逐字不变；③结构诊断非空 ⇒ `archi_file_tree` `Err`、空模型 ⇒ `Ok` | `commands/archi/mod.rs` -> `commands/archi/mod_test.rs`（4 条 parity + root 四形态 + 结构诊断 / 空模型 / 配置两端） |
| AC-4 | **出线面零 diff**：`src/types/generated/**` 零 diff、`all_commands!` 零 diff、`bindings/mod_test.rs` **零改动**且逐字签名 / zod 表项 / archi 八命令名单与相邻序断言照旧通过 | `commands/archi/mod.rs` -> `commands/archi/mod_test.rs`（`#[tauri::command]` / `#[specta::specta]` 各 8 计数）；`packages/desktop/src-tauri/src/bindings/mod_test.rs`（零改动反证，见「不可测试项」）；生成物零 diff 见「不可测试项」 |
| AC-5 | **缺省视图的时机**：三入口 `archi_file_tree` 调用数 = 0；点「文件」后恰 1 次；往返各再 1 次；`archi_graph` 每次挂载 1 次、`archi_module` / `archi_drift` 计数不因切换增加 | `packages/desktop/src/views/architecture/components/architecture-canvas.tsx` -> `.../architecture-canvas.test.tsx`（计数 + 缺省正锚）；`packages/desktop/src/app.tsx` -> `packages/desktop/src/app.test.tsx`（三入口组合，`app.tsx` 源码零 diff，仅作挂靠点） |
| AC-6 | **分段控件与栏位**：`role="tab"` 声明序 = 元素 / 文件 / 契约、缺省选中「元素」；三视图共用同一 `ResizablePanel`，切换不改变栏宽、不引发画布 / 元素信息面板重取数 | `architecture-canvas.tsx` -> `architecture-canvas.test.tsx`（声明序 + 选中态 + 同一栏位引用 + 切换零重取） |
| AC-7 | **失败面不回归**：切到「文件」后取数失败只落 `archi-file-tree-error`，画布与元素信息面板照常；`root` 变化的旧树不冒充；加载中与空树文案照旧 | `architecture-canvas.tsx` -> `architecture-canvas.test.tsx`（插桩后的失败 / 加载 / 空树 / 换 root 四行）；`file-tree.tsx` 组件面零改动见「不可测试项」（`file-tree.test.tsx` 零 diff 且 20 条全绿） |
| AC-8 | **语义不漂移 + 测试锚点不退化**：`tree.rs` / `tree_test.rs` / `file-tree.tsx`（除一行注释）/ `file-tree.test.tsx` / `use-archi-file-tree.ts` 及其测试零 diff；缺省判据 MUST NOT 只用 `queryByTestId('archi-file-tree') === null` 单一否定断言 | `packages/desktop/src/views/architecture/components/file-tree.tsx` -> `.../file-tree.test.tsx`（注释零陈述缺省 + 导出 / testid 零变动 + 20 条回归登记）；`architecture-canvas.tsx` -> `architecture-canvas.test.tsx`（否定断言清零 + 正锚强制 + 源码单点反证〔插桩不敏感口径〕）；领域 crate 零 diff 见「不可测试项」 |
| AC-9 | **纪律与已知限制成文**：`desktop-app-shell` 新纪律含形态 / 反形态 / 首次 poll 判据 / 适用面四条 + `code_stats` 债务 / `JoinError` 前向防御 / 并发度不变 / 出线零变化，每条至少一个 scenario；`desktop-archi-drift` 明写并行全仓扫描且无取消无去重；`desktop-workspace-code-stats` 旧句已纠正；反向判据无残留 | **无单元测试面**（spec 文本；代码侧可测半边 = 模块头执行契约注记，落 `commands/archi/mod.rs` -> `mod_test.rs`）；其余见「不可测试项」 |
| AC-10 | **范围与版本纪律**：`git status --porcelain` 改动集 ⊆ 4 份实现文件；`package.json` `version` 恰 +1 patch（`0.7.4` → `0.7.5`）；`tauri.conf.json` 仍 `"../package.json"`；`src-tauri/Cargo.toml` crate 版本行零 diff；`openspec/specs/**` 除 5 份 delta 外零 diff | `packages/desktop/package.json`（配置面，无测试路径 ⇒「不可测试项」）；零 diff 面（生成物 / 主 spec / 架构模型 / 领域 crate / 另四命令）见「不可测试项」 |
| AC-11 | **验收留痕 + 归档后人工验收承接**（真机观测不作门）：`openspec/changes/archi-file-tree-lazy-load/reports/acceptance.md` 在案且三节齐备——① **验收清单**六项（逐条给出观测手段 / 预期 / 判据来源）；② **环境不具说明**（63k 工作区在机但无 release 产物，窗口拖动 / 不白屏 / 三命令起止时刻无 agent 可读面 ⇒ 六项观测在 agent 相位不可履行）；③ **归档后人工验收承接**（观测与结论并入归档后人工验收）。判据止于「文件在案 + 三节齐备 + 清单六项可操作」；真机观测 MUST NOT 作门禁判据 / 归档阻塞项 / `tasks.md` 待办项 | **无单元测试面**（六项观测依赖 release 构建与 GUI 交互读数，时长随机器 / 缓存漂移）⇒「不可测试项」，验收相位只读核对「文件在案 + 三节齐备」；「不冻结」由 AC-2 首次 poll 判据机械承接（`mod.rs` 节第 1 行） |

> **被测范围边界**：`packages/desktop/src-tauri/crates/core/archi/**`（`tree.rs` / `tree_test.rs` / `dto.rs` / `drift/**` / `scan/**` / `api/**` / `lib.rs`）、`hooks/**`（含 `use-archi-file-tree.ts` 与其测试）、`element-tree.tsx`、`model-info.tsx`、`contract-panel.tsx`、`lib/**`、`commands/mod.rs`（`all_commands!`）、`src/types/generated/**`、`commands/stats/**`、`commands/config/**`、`src/bindings/mod_test.rs`、`file-tree.test.tsx`、`.dev-team/architecture/**`、`openspec/specs/**` 均为只读红线（design「不在本清单的对象」+ D9），不纳入被测范围；其「零 diff」本身不构成可断言行为，逐条登记于「不可测试项」。

---

## 单元测试

### `packages/desktop/src-tauri/src/commands/archi/mod.rs` -> `packages/desktop/src-tauri/src/commands/archi/mod_test.rs`

**改写既有测试文件**（1896 行 / 50 `#[test]`）——机械约束六条（design 测试文件面表第 1 行 + D5 / D6，逐条落位）：

1. **宿主形态**：19 个触及四条 wrapper 的测试函数 `#[test]` → `#[tokio::test]` + `async fn`（含 `八命令root校验四形态一律err` / `五条读命令零写盘` / 二条 drift 包装用例 / 七条 tree 用例 / 六条 api 用例）；9 个已带 `#[allow(non_snake_case)]` 的属性与 `#[tokio::test]` **并存保留**（属性序 `#[tokio::test]` → `#[allow(…)]` → `fn`）。改写后计数收敛为 **19 `#[tokio::test]` + 36 `#[test]`**（50 − 19 + 4 offload + 1 helper 自证 = 55 用例）。
2. **调用点补 `.await`**：**29 处**直接调用补 `.await`（分布在 19 个函数：`八命令root校验四形态一律err` 占 4、`五条读命令零写盘` 占 3、`graph正常态与停摆态` / `archi_file_tree结构诊断非空即err` / `archi_file_tree配置非法err且合法生效` / `archi_file_tree与archi_graph同根一致` / `api_generate写后复检三列全空` 各占 2，其余 12 个函数各 1）；同步命令 `archi_module` / `archi_set_gate` / `archi_api_generate` / `archi_change_summary` **不补**（零改动面）。
3. **新增 offload 装置**：`fn assert_offloaded<F: std::future::Future>(future: F)` —— `std::task::Waker::noop()`（Rust 1.95 工具链，`Waker::noop` 自 1.85 稳定）+ `std::pin::pin!` + `Context::from_waker` + **单次** `poll`，断言 `Poll::Pending`（失败文案带中文判据句）；**不加**任何 test-only 注入缝（D4）。
4. **源码面形态计数拆三条**（D6，替代既有 `pub fn archi_` == 16 单条）：`pub fn archi_` = **12** / `pub async fn archi_` = **4** / `tauri::async_runtime::spawn_blocking` = **4**；另四条闭包字面量断言。
5. **既有等价用例保留**：四条「包装 ↔ `_inner` 逐字段一致」（`三命令与inner结果逐字段一致` / `archi_drift包装与inner一致` / `archi_file_tree包装与inner逐字段一致` / `api_check包装与inner逐字段一致`）只加 `.await`，同参 deep-equal 与 fixture **逐字保留**。
6. **既有断言不删**：root 四形态文案、结构诊断非空、空模型 `Ok` 全 `none`、配置非法 / 合法两端、零写盘快照、模块头 stale 禁表、命令层零领域规则 —— 断言体逐条保留，仅按 async 形态补 `.await` 与随动命名。

#### 待测功能

- `archi_graph(root: String) -> Result<ArchiGraph, String>`: 修改（`pub fn` → `pub async fn`）—— 体内 `spawn_blocking(move || archi_graph_inner(Path::new(&root)))` + `.await` + `map_err(|error| format!("架构图任务失败: {error}"))?`；IPC 面（参数 / 返回 / 错误文案）零变化，命令 future 首次 poll = `Pending`
- `archi_drift(root: String, mode: ArchiDriftMode) -> Result<ArchiDriftReport, String>`: 修改（同上）——`ProcessArchiveVcs::new()` 装配留 async 体后移入闭包（单表达式闭包，事实校准 3）；`CancelFlag::never()` / 模式分流 / 零去重零节流全在 `*_inner` 内不变；文案 `漂移检查任务失败: {error}`
- `archi_file_tree(root: String) -> Result<ArchiFileTree, String>`: 修改（同上）—— 四条非法 root 文案逐字不变；结果与 `*_inner` 逐字段一致；文案 `文件树任务失败: {error}`
- `archi_api_check(root: String) -> Result<ArchiApiCheck, String>`: 修改（同上）—— 判据输入与报告零改动，只换执行线程；文案 `契约检查任务失败: {error}`
- `archi_module()` / `archi_change_summary()` / `archi_set_gate()` / `archi_api_generate()`: 零改动（保持同步形态）——`archi_change_summary` 仍持 `State<WorkspaceStores>` 且不 await；`archi_set_gate` 仍是唯一写命令
- `archi_*_inner()` ×4 / `check_root()`: 零改动 —— 签名 / 语义 / 错误文案逐字不变，只被移进闭包；`check_root` 仍在闭包内执行（D2：领域组装 MUST NOT 被搬进 async 体的同步段）

<!-- design.md「公共函数 / API」表对本文件恰 6 行（上列逐行映射）；类型定义表 0 行，配置表 1 行（`package.json`，不在本文件）。`write_atomic` / `check_root` 之外的私有件不入表。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 四条命令 future 首次 poll 为 `Pending`（AC-2 / spec `desktop-app-shell` Scenario「四条命令首次 poll 为 Pending」） | 正向 | `root` 取健康 tempdir 的 `root()`（`check_root` 可过 ⇒ 命令体真落阻塞池）；`assert_offloaded(archi_graph(root.clone()))` / `assert_offloaded(archi_drift(root.clone(), ArchiDriftMode::Full))` / `assert_offloaded(archi_file_tree(root.clone()))` / `assert_offloaded(archi_api_check(root.clone()))` 四次 `Waker::noop()` 单次 `poll` 均 `Pending`；future 首 poll 后立即丢弃（后台任务以 `Ok` / `Err` 收尾、结果不读、零落地断言）；四类均以裸 `#[test]`（事实校准 6，无 `#[tokio::test]` 宿主） | 参数化迭代（4 命令） |
| offload 判据的反向自证（helper 非永真 / R-4） | 异常 | 对 `async { 42 }`（首 poll 即 `Ready`）调用同一 helper → 必 panic（`#[should_panic]`，期望文案含「首次 poll 必须为 Pending」）；若 helper 写成恒真，该臂红。偶发面（阻塞任务在首 poll 前完成）按 R-4 加强版处置：`#[tokio::test(flavor = "current_thread")]` + interval 计 tick ≥ 1，MUST NOT 退回计时断言 | 单次 |
| 包装 ↔ `_inner` 逐字段一致（AC-1 / AC-3 ①） | 正向 | 四条既有 parity 用例加 `.await` 后保留：同参 deep-equal（`serde_json::to_value` 逐字段相等）、fixture 与断言体逐字不变；`archi_drift` 臂取 `Full` 模式（免 vcs）、`api_check` 臂取 `api_fixture`（零写盘命令） | 表驱动（4 用例） |
| 非法 root 四形态一律 `Err`（AC-3 ② / spec 口径「空白 / 缺失 / 不可读 / 非目录」） | 异常 | 四形态：`""`（空白）/ `"   "`（全空白）/ 指向文件 / 不存在目录 → 四命令 `is_err()`；文案三式逐字（`root 无效：不得为空白` / `root 无效（{path}）：…` / `root 不是目录（{path}）`）；同一 `for` 循环内同步对拍 `archi_*_inner` 与 `check_root`（内外文案同源，闭包内校验未外提）；`#[tokio::test]` 下逐项 `.await` | 参数化迭代（4 root 形态 × 4 命令 + inner 对拍） |
| `root: String` 字符串形态边界（边界 / str 映射） | 边界 | 五形态：超长（路径段 `"x".repeat(1100)`）/ 含空格 / 非 ASCII（`架构-命令-根`）/ emoji（`archi-cmd-🎯`）/ 空串 `""` → 四命令均 `is_err()` 且不 panic（`Result` 内收口）；既有 `api命令root字符串边界` 的四形态扩到四命令（原用例只覆盖 `archi_api_check` / `archi_api_generate`） | 参数化迭代（5 形态 × 4 命令） |
| 结构诊断非空 ⇒ `archi_file_tree` `Err`（AC-3 ③） | 异常 | 环模型 → `Err` 文案含「模型结构错」与诊断条数；MUST NOT 返回部分树；对照臂：同盘换健康模型 → `Ok`（既有 `archi_file_tree结构诊断非空即err` 加 `.await`，断言逐字保留） | 参数化迭代（2 形态对拍） |
| 空模型 ⇒ `Ok` 且全条目无主（AC-3 ③） | 边界 | ①模型目录缺席 ②`models/` 存在零 `*.toml` → 均 `Ok`；全部条目 `mark = none` / `owners` 空；MUST NOT `Err`（既有 `archi_file_tree空模型返回ok且全无主` 加 `.await`） | 参数化迭代（2 空模型形态） |
| 配置非法 `Err` / 合法生效（AC-3 ③ 侧 / 既有文案） | 异常 | `{ "exclude": ["src/legacy"] }`（缺通配符）→ `Err` 文案含非法条目与 `/**` 提示；`{ "exclude": ["openspec/**"] }` → `Ok` 且排除生效（既有 `archi_file_tree配置非法err且合法生效` 加 `.await`） | 参数化迭代（2 形态对拍） |
| 只读命令零写盘（AC-1「只换线程」/ 既有快照口径） | 边界 | `五条读命令零写盘` 三处 wrapper 调用加 `.await` 后 `snapshot(before) == snapshot(after)`；异步化 MUST NOT 引入落盘（`.tmp` 残留零）；快照段与 `file_tree.entries` 非空断言逐字保留 | 单次 |
| `archi_drift` 的 vcs 装配位置与两模式（AC-1 / 事实校准 3 / 边界 Enum 两合法值） | 正向 | 运行期：wrapper `Full` → `Ok` 且零 vcs 调用（健康语料）；`Incremental` → 经真实 `ProcessArchiveVcs` 取变更集，非 git tempdir 上显式 `Err` 且文案点名 git 失败（MUST NOT 降级空清单）；源码自证：`let vcs = ProcessArchiveVcs::new();` 在 `pub async fn archi_drift` 体内且**先于** `spawn_blocking(`，闭包为单表达式 `spawn_blocking(move \|\| archi_drift_inner(Path::new(&root), mode, &vcs))`；四条既有 drift 用例（全量零 vcs / 增量经 vcs 缝 / 变更集失败即 Err / 根路径边界逐字回填）加 `.await` 保持绿 | 组合迭代（源码形态 × Full / Incremental 两臂） |
| 源码面 asyncness 计数（AC-1 / D6 / 事实校准 5） | 异常 | `include_str!("mod.rs")` 自证三条计数：`pub fn archi_` 行 **12** / `pub async fn archi_` 行 **4** / `tauri::async_runtime::spawn_blocking` **4**；反向判据：任一条退回同步即红（既有「16 一件」单条按 D6 拆分，用例名同步改符实） | 参数化迭代（3 计数） |
| 四条闭包为单表达式且 `*_inner(…)` 在闭包内（AC-1 / D6 后半） | 正向 | 逐条字面量断言各**恰 1 处**：`spawn_blocking(move \|\| archi_graph_inner(` / `spawn_blocking(move \|\| archi_drift_inner(` / `spawn_blocking(move \|\| archi_file_tree_inner(` / `spawn_blocking(move \|\| archi_api_check_inner(`；作用域否定：切出四条 wrapper 与八条 `*_inner` 体后，残余源码中 `spawn_blocking` **零命中**（另一条 `*_inner` 或另一条同步命令体内出现即红） | 参数化迭代（4 闭包 + 1 作用域否定） |
| 双层 `Result` 与 `JoinError` 文案在位（AC-1 / D3 / 事实校准 1·2；R-2 的可测半边） | 边界 | 四条 wrapper 体内形态 `.await` + `.map_err(\|error\| format!(` 在位；四值文案逐字（`架构图任务失败: {error}` / `漂移检查任务失败: {error}` / `文件树任务失败: {error}` / `契约检查任务失败: {error}`，**半角冒号**）；反向判据：零 `.map_err(\|e\| e)` / 零第二套错误映射写法；该分支的**运行期**触发不可测（见「不可测试项」） | 参数化迭代（4 命令 × 文案 / 形态两维） |
| 模块头执行契约注记（AC-9 代码侧锚 / 事实校准 4） | 正向 | 模块头（连续 `//!` 前缀段）仍含 `八命令` 与 `写命令两条`；新增句在位：含 `spawn_blocking`、含 `阻塞池`、含 `desktop-app-shell`（纪律指针单点，本文件不复述判据）；`#[tauri::command]` 与 `#[specta::specta]` 各 **8** 处在位（既有断言） | 单次 |
| 模块头旧口径零命中（既有 stale 禁表，随动核对） | 异常 | 模块头零 `五命令` / `四命令` / `六命令` / `其余五条` 四字面量 —— 措辞只能用「四条取数命令」（事实校准 4；写成「四命令」即红）。既有两处 stale 断言逐条保留：`archi_file_tree命令层零领域规则` 的全源四字面量禁表（`:1597`）+ `命令层零领域规则与头注释口径` 的 `六命令` 禁表 | 遍历枚举（4 禁字面量） |
| 命令层零领域规则 + `check_root` 未外提（AC-1 · D2 / 既有断言保持） | 异常 | 既有断言保持：`archi::api::artifact_check(` / `archi::workspace_render(` 各 1、零 `archi::api::compare(` / `archi::api::meta(` / `archi::api::display(` / `archi::api::findings(`、零 `fs::write` / 零 `create_dir_all`、`use crate::commands::atomic::write_atomic;` 恰 1 处；新增两条：`check_root(` 在四条 wrapper 体内**零命中**（校验留在闭包里的 `*_inner`）、`Path::new(&root)` 四条各 1 处 | 单次 |
| 另四条命令与八条 `*_inner` 的同步形态零变化（AC-1「diff 只看 wrapper」/ D9） | 边界 | `archi_module` / `archi_change_summary` / `archi_set_gate` / `archi_api_generate` 仍 `pub fn` 且体内零 `.await`、零 `spawn_blocking`；八条 `*_inner` 仍 `pub fn` 且签名行逐字（`root: &Path` 入参、`Result<T, String>` 出参）；`archi_set_gate` 仍是唯一写命令 | 单次 |
| 同步面用例零改动（AC-3 / D9 的回归登记） | 边界 | 31 条**不触及四条 wrapper** 的既有用例全绿且断言体逐字不改（其中 30 条零改动；`命令层零领域规则` 一条按 D6 拆分，见上文计数行）：`change_summary*` 五条 / `set_gate*` 三条 + `write_atomic只读目录失败且无残留` / `module三形态` + `archi_module*` 六条 / `archi_drift` 的 inner 侧十一条（全量零 vcs / 增量经 vcs 缝 / 变更集失败即 `Err` / root 校验 / 无模型 skipped / 坏模型不 panic / 配置非法 / 全量零写盘 / 与门禁 runner 同源 / 命令层零判定 / 根路径边界逐字回填）/ `archi_file_tree命令层零领域规则` / `命令层零领域规则与头注释口径` / `写失败文案透传不称全或无` | 表驱动（31 条回归登记，30 条零改） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| Tauri 运行时（`App<MockRuntime>` + `WorkspaceStores`） | **进程边界装置，非仓内模块 mock**：既有 `app_with(data_root)`（`tauri::test::mock_app()`）、`store_of` / `seed_record` 装置；`archi_change_summary` 的 `State` 面既有用例零改 | 全部（尤其 `八命令root校验…` 与 `change_summary*`） |
| `tauri::async_runtime` 的全局运行时 | **不 mock**：`spawn_blocking` 首次调用经 `RUNTIME.get_or_init(default_runtime)` 自建全局运行时（`commands/changes/mod_test.rs` 23 例实证）；offload 四行以裸 `#[test]` 直接 poll，其余行由 `#[tokio::test]` 宿主 await | offload 4 行 + 全部 `.await` 行 |
| `archi::dto` / `archi::drift::check` / `archi::tree::file_tree` / `archi::api::artifact_check` 四领域单点 | **不 mock**：真实调用（parity 用例的判据就是「命令层零加工」，替身会自证） | 全部 |
| `ProcessArchiveVcs`（drift 的 vcs 缝） | **包装层不 mock**：`archi_drift` 内 `ProcessArchiveVcs::new()` 真装配；`FakeVcs` 只作 `archi_drift_inner` 的**显式入参**注入（既有形态）；wrapper 用例一律取 `Full` 模式避开子进程 | drift 包装 2 行 + 两模式行 |
| 子进程（git） | **不触达**：wrapper 用例零增量模式；`Incremental` 的真实失败臂只在非 git tempdir 上观测（无替身） | drift 两模式行 |
| 文件系统 / tempdir | **不 mock**：既有四装置 `healthy` / `drift_fixture` / `tree_fixture` / `api_fixture` 真实落盘（模型 / 源文件 / 配置 / 契约资产）；零写盘判据用既有 `snapshot()` 递归快照 | 全部 |
| 时钟 / 计时器 | **不 mock、零计时断言**（AC-2 明令 MUST NOT 计时；判据为单次 poll 结果） | offload 4 行 |
| `std::task::{Waker, Context, Poll}` / `std::pin::pin!` | **不 mock**（std 原语即判据本体）；既有 `block_on` 辅助**零改动**（仍只服务 `archi_drift与门禁runner同源` 的 runner future） | offload 4 行 + helper 自证行 |
| 环境变量 / 网络 / 数据库 | 零依赖 | —— |

---

### `packages/desktop/src/views/architecture/components/architecture-canvas.tsx` -> `packages/desktop/src/views/architecture/components/architecture-canvas.test.tsx`

**改写既有测试文件**（1612 行 / 52 `it()`）——改写口径四类，写死以防「测试变绿但失判据」（R-3 / AC-8）：

| 类 | 处置 | 覆盖用例（本相位逐条对位，行号取改写前） |
|---|---|---|
| **① 语义改锚（4 条）** | 把「页面就绪 / 缺省视图」信号由 `archi-file-tree` 改为 `archi-element-tree`（辅以 `archi-tree-mode`）；触及文件树处插 / 换 `switchTo`；标题中的「左栏缺省为文件树」改符实 | `应渲染三段布局并以首个顶层元素为聚焦（左栏缺省为元素树）`（438）/ `应缺省渲元素树且切到文件树时 testid 换面（两视图不共存）`（1073）/ `应让左栏三视图共用同一可调宽栏位`（1283，`switchTo('元素')` 换为 `switchTo('文件')`）/ `应点「契约」后只渲契约面板` + `应在三视图往返切换时零重取图 / 抽屉 / 漂移三条命令`（1371·1385） |
| **② 只改等待锚点（3 条）** | 把「图已落定」的等待信号由 `archi-file-tree` 换为 `archi-element-tree`（或 `archi-tree-mode`），断言体零改 | `应在首轮未回时呈现加载态`（933·951）/ `应在 archi_graph reject 时呈现错误面板…`（962·972）/ `应不因文档警告停摆`（1063·1066） |
| **③ 插 / 换 `switchTo('文件')`（8 条）** | 仅在被测行为需要文件树**在场或可点击**处插入（或替换为）一次切换，其余断言逐字保留 | `应在元素树点击后按新邻域重算…`（713，删去多余的 `switchTo('元素')`——缺省即元素）/ `应在两视图间来回切换四次后只让文件树随重挂重取`（1089）/ `应在点文件树归属行后圈定范围…`（1119·1133）/ `应在画布节点点击后让文件树反查高亮…`（1149）/ `应在文件树取数失败时只落左栏行内提示…`（1177）/ `应在文件树首轮未回时渲「解析中…」占位`（1204）/ `应在文件树返回空条目时渲「仓库内无源文件」`（1225）/ `应在树失败后换 root 成功时清掉错误行…`（1244） |
| **④ 计数语义改判（2 条，与 ③ 的 1089 重叠）** | `archi_file_tree` 的基准由「挂载即有 1 次」改为「切换后才有 1 次」；往返累计按「每次切回各 +1」重算（原 `+2` 断言随之改判），`archi_graph` / `archi_module` / `archi_drift` 不变式**逐字保留** | 1089 / 1385 |
| **⑤ 否定断言清零（AC-8）** | 只保留两处 `queryByTestId('archi-file-tree')).toBeNull()`：①左栏整块不挂载（空模型 / 停摆态）②已切到别的视图后两视图不共存；**每处同行必须另有正锚**（`archi-element-tree` / `archi-contract-panel` / `archi-canvas-empty` 在场或 `archi_file_tree` 计数）；「缺省视图」判据 MUST NOT 仅由否定断言表达 | 全文 52 条逐条复核（尤其 1073 / 1273 / 1371 / 1561 / 1593） |

> 与 design 测试文件面表的差异（本相位按工作树逐条对位后收敛，只精确化计数、不改意图）：design 记「4 条语义用例 + 12 条插桩」，实际形态为 **4 条语义改锚 + 3 条只改等待锚点 + 8 条插 `switchTo('文件')` + 1 条空模型分支补正锚（1273）**；design 列举的 ~700 与 ~450 经核对**无需改动**（前者只等抽屉文案与节点集合、后者是纯拓扑坐标用例，二者均不触文件树）。

#### 待测功能

<!-- design.md「公共函数 / API」表对本文件**零行**：前端整段写明「零公共 API 变化——ArchitectureCanvas / ElementTree / FileTree / ContractPanel 的入参、导出与 testid 逐字不变；TreeMode 是类型，缺省值不属 API」。故本节不推导任何导出面。被测面取自「修改文件」表第 2 行（`treeMode` 初值、`options` 声明序、两处 doc）与「架构组件」表第 3 行（左栏分段控件与缺省视图），可观测面是左栏渲染分支、分段控件 DOM 序与 `archi_file_tree` 调用计数。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 缺省视图 = 元素树且首屏零 `archi_file_tree`（AC-5 / spec `desktop-archi-file-tree` Scenario「缺省渲元素树且切到文件树」） | 正向 | 健康图挂载 → `archi-element-tree` 在场、`archi-tree-node` 非空、`archi-file-tree` 不在场，且 `callsOf('archi_file_tree')` 长度 **0**；三段布局齐备（`archi-tree-mode` / `archi-canvas` / `archi-drawer`）；正锚 = 元素树在场 + 计数（MUST NOT 只以 `archi-file-tree` 缺席作判据）；既有 ~438 用例按此改锚并改名 | 单次 |
| 点「文件」后恰 1 次且载荷逐字（AC-5 第 2 句） | 正向 | `switchTo('文件')` → `archi_file_tree` 恰 **1** 次、载荷 `{ root: 'C:\\demo\\alpha' }`；`archi-file-tree` 在场（`archi-file-node` 非空）、`archi-element-tree` 不在场；`archi_graph` / `archi_module` / `archi_drift` 计数不增 | 单次 |
| 两视图往返各再 1 次（AC-5 / spec Scenario「切回文件树只重取文件树」） | 正向 | 元素（缺省）→ 文件（第 1 次）→ 元素 → 文件（第 2 次）→ 元素 → 文件（第 3 次）⇒ `archi_file_tree` 累计 **3**；`archi_graph` / `archi_module` / `archi_drift` 全程计数不变；非活动面板不挂载语义不变（元素态下 `archi-file-node` 零节点） | 单次 |
| 分段钮声明序与缺省选中（AC-6 / spec Scenario「分段钮声明序与缺省选中」） | 正向 | `within(getByTestId('archi-tree-mode')).getAllByRole('tab')` 的 `textContent` 序列 === `['元素', '文件', '契约']`；缺省「元素」钮 `aria-selected="true"`、另两钮 `false`（base-ui `TabsTab` 提供；MUST NOT 以自绘 `aria-pressed` / `data-mode` 作判据，二者在本变更是退役口径） | 单次 |
| 切换后选中态随动（AC-6） | 正向 | `switchTo('文件')` / `switchToContract()` 后选中态各自移到目标钮（`aria-selected` 恰一枚为真）；选中态变化 MUST NOT 引发 `archi_graph` / `archi_module` / `archi_drift` 重取 | 参数化迭代（3 视图） |
| 三视图共用同一栏位且栏宽不随视图变化（AC-6 / spec Scenario「分段钮声明序与缺省选中」末句） | 边界 | `columnOf()` / `panelOf()` 的 DOM 引用在 元素 → 文件 → 契约 三次切换后 `toBe` 恒等（同一 `ResizablePanel`，不重建）；`closest('[data-group]')` 恒非空；改锚后的 ~1283 用例补第三视图臂 | 单次 |
| 契约视图往返零重取（AC-5 / AC-6 侧） | 正向 | 两轮「元素 → 契约 → 文件」往返 ⇒ `archi_graph` / `archi_module` / `archi_drift` 计数不变、`archi_api_check` 每次进契约视图各 1（累计 2）；点「契约」后 `archi-contract-panel` 在场、两树均不在场（既有断言保留，且 `switchToContract()` 前补一条「缺省元素树在场」正锚）；~1385 用例的切换序随声明序改写 | 单次 |
| 文件树接线的插桩组（AC-7 / AC-8，③ 类） | 边界 | 上表 ③ 类 8 条用例只在该用例需要文件树在场 / 点击处插（或换）`switchTo('文件')`；断言体（条目集合、`data-mark`、`owners` 高亮、画布节点 / 边、抽屉 fqn、错误行文案、加载 / 空树文案、`root` 换新）逐条保留 | 参数化迭代（8 用例） |
| 计数语义改判组（AC-5 / ③类） | 边界 | ~1089 用例：基准由「挂载即 1 次」改为「切到文件后 1 次」，两轮往返后累计 = 基准 + 2；~1385 用例：三视图往返后 `archi_api_check` = 2 且三条既有命令计数不变 | 参数化迭代（2 用例） |
| 空模型分支左栏零挂载（AC-7 边界 / 既有 ~1273） | 边界 | `archi-canvas-empty` 在场；`archi-file-tree` / `archi-element-tree` / `archi-tree-mode` 三者皆缺席；补正锚 `callsOf('archi_file_tree')` 长度 **0**（`CanvasBody` 早退 ⇒ `TreeColumn` 不挂载 ⇒ 即便缺省换成元素树也无取数） | 单次 |
| 缺省视图与 `mode` 无关（AC-6 / spec `desktop-archi-canvas` Scenario「两处挂载点共用同一组件」的测试半边） | 边界 | 同一组件分别以 `mode="full"`（挂载点 A）与 `mode="incremental"`（挂载点 B 形态）挂载 → 两形态均 `archi-element-tree` 在场、`archi_file_tree` 0 次、`archi_drift` 载荷 `{ root, mode }` 逐值透传；`change-detail-view.test.tsx`（挂载点 B 宿主）零改动 | 组合迭代（2 形态 × 缺省视图 / 计数两维） |
| 文件视图失败面不回归（AC-7 / spec `desktop-archi-file-tree` 失败面条） | 异常 | `switchTo('文件')` 且 `archi_file_tree` reject → 仅 `archi-file-tree-error` 在场（文案含原因与「画布照常渲染」）、`archi-file-tree` 与 `archi-file-node` 不渲染；画布节点 / 边与 `archi-drawer` 照常；切回「元素」后元素树在场（视图卸载 / 重挂语义不变） | 单次 |
| 源码面单点反证：缺省值与声明序（AC-8 / D7 / D8；插桩不敏感口径） | 异常 | `readFileSync('src/views/architecture/components/architecture-canvas.tsx')` 自证（沙箱把表达式包成 `stryMutAct_… ? "" : (…, <原值>)` ⇒ 一律「声明锚 + 同行字面量」，MUST NOT 断言整初始化式逐字）：**①单点** 声明锚 `useState<TreeMode>(` 全源计数 **1**，且该行命中 `/useState<TreeMode>\([^\n]*'element'/`；**②否定** `/useState<TreeMode>\([^\n]*'(file\|contract)'/` **零命中**（`'file'` / `'contract'` 不作初值）；**③声明序** 三钮显示名 `'元素'` / `'文件'` / `'契约'` 的 `indexOf` 三者均 > -1 且严格递增，另加 `/label: [^\n]*'元素'/` 三条同行锚（不锚 `mode: '…'` 的整段邻接）；**④派生单点** `options.map(` 驱动 `<TabsTrigger value={option.mode}>` / `<TabsContent value={option.mode}>`（插桩后 JSX 骨架逐字在位 ⇒ JSX 内零第二份顺序）、`data-testid="archi-tree-mode"` 与 `role="group"` 在位；**⑤旧注释口径零命中** `缺省文件树` / `左栏缺省为文件树` | 单次 |
| 缺省判据的正锚覆盖（AC-8 / R-3） | 边界 | 逐条复核 52 条既有用例：凡以「缺省视图」为判据者一律以 `archi-element-tree` 在场 + `callsOf('archi_file_tree')` 计数表达；`archi-file-tree` 否定断言仅存两处（左栏整块不挂载 / 已切换后不共存）且各自同行另有正锚；删除任一新锚后用例仍绿者视为退化锚点，须重写 | 遍历枚举（52 用例） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core` 的 `invoke` | **进程边界 mock**：`vi.hoisted` + `vi.mock`，按命令名分发表（既有 `serveGraphAndDrift` 分派 `archi_graph` / `archi_drift` / `archi_module` / `archi_file_tree` / `archi_set_gate`），`beforeEach` 重置并装默认实现 | 全部 |
| `@xyflow/react` | **观测垫片**（环境缺口而非业务 mock）：记录 `ReactFlow` 入参后交真实组件渲染（既有 `flowProps` 手法）；`ResizeObserver` / `offsetWidth` / `offsetHeight` / `DOMMatrixReadOnly` / `getBBox` 度量垫片照抄既有 | 画布节点 / 边断言行 |
| base-ui `Tabs`（`TabsList` / `TabsTrigger` / `TabsContent`） | **不 mock**：`TabsPanel` 的 `keepMounted = false` 正是「非活动面板不挂载 ⇒ 首屏零取数 / 往返各 1 次」的判据本体；`role="tab"` 与 `aria-selected` 由 base-ui 提供 | 缺省视图 / 计数 / 声明序 / 栏位行 |
| `FileTree` / `ElementTree` / `ContractPanel` / `TreeColumn` / `CanvasBody` / `useArchiFileTree` | **不 mock**：真实组合（缺省分支与挂载 / 卸载即被测语义；替身会让 AC-5 自证） | 全部 |
| `lib/marks` / `lib/drift-marks` / `lib/fqn` / `structure-error-panel` / `model-info` | **不 mock**：真实调用（零改动面，回归登记的判据依赖真实标记派生） | 画布 / 标记 / 停摆 / 抽屉行 |
| 源码文本 | **不 mock、不 import 源模块**：`node:fs` 的 `readFileSync(resolve(process.cwd(), 'src/…'), 'utf8')`（沿 `contract-panel.test.tsx` / `model-info.test.tsx` 先例）；断言一律「声明锚 + 同行字面量」正则 / 显示名 `indexOf`（插桩不敏感，见「源码自证口径」） | 源码自证两行 |
| 时钟 / 定时器 / 网络 / 子进程 | **零依赖**：全部落定用 `waitFor`（真时钟），MUST NOT 以「单次 `await Promise.resolve()` 后紧接同步断言」判定落定（design 待决问题 4） | —— |

---

### `packages/desktop/src/app.tsx` -> `packages/desktop/src/app.test.tsx`

**追加 3 条用例**（既有文件零结构改动）——本节是 AC-5 三入口的**跨模块组合挂靠点**：`app.tsx` 源码本变更**零 diff**，但它是最上层调用方（真实 hash 路由 + 真实侧栏点击 + 真实卸载重挂），canvas 层只能等价模拟「切走再切回」。既有 `mockIpc()` 未桩 `archi_file_tree`（缺省 `Promise.resolve(null)`）、`countOf()`（`:333`）与 `archi` 章节 5 条用例在位，零桩成本。

#### 待测功能

<!-- design.md「公共函数 / API」表对本文件零行（前端零公共 API 变化），且 app.tsx 不在「修改文件」表内（零 diff）。本节只承载 AC-5 的三入口组合用例（按链路最上层调用方挂靠，见「测试布局决策」第 2 条）与既有 archi 章节用例的回归登记。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| hash 直入 `#/architecture` 零 `archi_file_tree`（AC-5 / spec Scenario「三种缺省入口均零文件树取数」） | 正向 | `window.location.hash = '#/architecture'` + `render(<App />)` → 等 `archi-canvas` 与 `archi-element-tree` 在场后断言 `countOf('archi_file_tree') === 0`、`countOf('archi_graph') === 1`；`archi-file-tree-loading` 零出现（文件树既不挂载也不处加载态）；`nav-architecture` 激活 | 单次 |
| 侧栏进入零 `archi_file_tree`（AC-5 第 2 入口） | 正向 | `restored()` 后点 `nav-architecture` → `archi-canvas` 在场、`archi-element-tree` 在场、`countOf('archi_file_tree') === 0`、`countOf('archi_graph') === 1`；hash 变 `#/architecture` | 单次 |
| 切走再切回零 `archi_file_tree`（AC-5 第 3 入口 / 挂载语义） | 边界 | 进架构页 → 点 `nav-changes` → 再点 `nav-architecture`（第二次挂载）⇒ `countOf('archi_file_tree')` 仍 **0**、`countOf('archi_graph') === 2`（挂载触发不做跨挂载缓存，既有语义）；两次挂载的 `archi-element-tree` 均在场 | 单次 |
| 既有 archi 章节用例零改动（回归登记） | 边界 | `应渲染架构页并激活 nav-architecture` / `应从侧栏进入并在切回变更页时复原` / `应在 archi_graph reject 时呈现错误态` / `应在切走再切回时重发 archi_graph` / `收起态下新增架构 Agent 面板不引入会话取数` 五条断言体逐字保留（其判据只锚 `archi_graph` / 壳层 invoke 序列，不受缺省视图换面影响） | 表驱动（5 条回归登记） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core` 的 `invoke` | **进程边界 mock**：既有 `mockIpc()` 按命令名分发；`archi_file_tree` **未桩**（缺省 `Promise.resolve(null)`）——「0 次」判据不受该缺省影响（0 次即未被调用），但 MUST NOT 把该缺省的返回形态当作任何断言对象 | 全部 |
| 壳层 API（`getVersion` / `open` / `check`） | **既有替身**（`getVersionMock` / `openMock` / `checkMock`），零改动 | 全部 |
| `App` / 路由 / `ArchitectureCanvas` / 侧栏 | **不 mock**：真实组合（三入口分别由真实 hash 改值、真实 nav 点击、真实卸载 + 重挂驱动；这正是本节存在的理由） | 3 条 |
| 时钟 / 网络 / 子进程 | **零依赖**（全部 `waitFor` 收敛） | —— |

---

### `packages/desktop/src/views/architecture/components/file-tree.tsx` -> `packages/desktop/src/views/architecture/components/file-tree.test.tsx`

**测试文件零改动**（20 `it()`，直测 `FileTree`，不经 `TreeColumn`）——本节只登记：①该文件 `:13` 一行注释的源码断言；②既有 20 条用例的回归登记。渲染 / 标记 / 点击 / 反查 / 展开态 / 取数状态面一律零新增断言（AC-8「零 diff」射程 = 除该行注释外）。

#### 待测功能

<!-- design.md「公共函数 / API」表对本文件**零行**（前端零公共 API 变化：FileTree / FileTreeProps 入参、导出与 testid 逐字不变；TreeMode 是类型且「只停写注释里的缺省陈述」）。本行不做任何导出面推导。被测面取自「修改文件」表第 3 行（仅 `:13` 注释：删去「缺省 `file`」）——注释无可执行行为面，故下列用例为「源码文本断言 + 回归登记」两类，无行为新增。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 注释不再自行持有缺省值（AC-8 / D7 / R-12；插桩不敏感口径） | 异常 | `readFileSync('src/views/architecture/components/file-tree.tsx')` 自证：**①旧注释零命中**　旧注释原文形态（「缺省」后接反引号包裹的 `file`，即 `:13` 现状）零命中——改回即红；**②「缺省」只作指针**　含「缺省」的行恰 **1** 行且同行含 `architecture-canvas.tsx`（注释面不被插桩 ⇒ 可逐字判，但只判「指向单点持有者」，不断言整行原文；**不得在本文件重新持有缺省值**，D7 单点）；**③跨文件反证**　本文件 `useState<TreeMode>(` **零命中**（声明锚口径，缺省值单点只在 `architecture-canvas.tsx`）；**④对照臂**　`architecture-canvas.tsx` 内旧口径 `缺省文件树` 零命中 | 单次 |
| `TreeMode` 联合与全部导出零改动（AC-8 / D9；插桩不敏感口径） | 正向 | 源码自证（声明锚 + 同行字面量，不做整句逐字）：声明锚 `export type TreeMode =` 计数 **1**，三取值字面量 `'file'` / `'element'` / `'contract'` 的 `indexOf` 均**大于**该锚位置（三值齐备，`TreeMode` 语义零变动）；`export function FileTree` / `export interface FileTreeProps` 在位；`data-testid` 六面零变动（`archi-file-tree` / `archi-file-node` / `archi-file-mark` / `archi-file-tree-error` / `archi-file-tree-loading` / `archi-file-tree-empty`，插桩后属性面逐字在位）；`buildForest` / `FileRow` / `isExpanded` 等私有件零导出 | 单次 |
| 组件行为与「缺省视图」解耦（AC-8 的行为侧 / 既有 20 条回归） | 边界 | 直测 `<FileTree root focused onFocus/>`（不经 `TreeColumn`）：挂载即 1 次 `archi_file_tree`、无分段控件、无「哪个视图是缺省」的判据 ⇒ 证明「缺省视图换面」只在 `architecture-canvas.tsx` 持有；既有 20 条（层级派生 / 兄弟序 / 缺省展开三段 / 五值标记 / 归属行点击 / 非归属行只切展开 / 多 path 高亮 / 根不下钻 / 滚到首条 / 零命中静默 / 展开态按 path 保持 / 空树 / 挂载取数 / 解析中 / 失败行内 / 零根不 invoke / 换 root 重取 / 上千条规模 / path 字符串边界）零改动全绿 | 表驱动（20 条回归登记） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core` 的 `invoke` | **进程边界 mock**：既有 `vi.hoisted` + `vi.mock`（只分派 `archi_file_tree`），本变更零改 | 取数状态面回归行 |
| `FileTree` / `ui/scroll-area` / `ui/button` / `lucide-react` | **不 mock**：真实组合（渲染面即被测语义） | 全部 |
| `Element.prototype.scrollIntoView` | **实例级最小桩**（既有，`afterEach` 复原；不进 `test-setup.ts` 原型） | 滚到首条回归行 |
| 源码文本 | **不 mock、不 import 源模块**：`readFileSync(resolve(process.cwd(), 'src/views/architecture/components/file-tree.tsx'), 'utf8')`；断言一律「声明锚 + 同行字面量」正则（插桩不敏感，注释面不被插桩） | 源码断言行 |
| 时钟 / 网络 / 子进程 | **零依赖** | —— |

---

## 不可测试项

以下条目属 design「变更清单」的修改文件、或属 AC 的判据面，但**无单元测试路径**（成因逐条说明）。它们不进入 `## 单元测试`，进入验收的方式落在「静态检索 / 机械门禁 / 只读核对留痕 / 归档后人工验收 / 编译期 / 代码评审」六类。

| 文件 / 模块 / 判据 | 类别 | 不可测试原因 | 验收方式 |
|---|---|---|---|
| `packages/desktop/package.json`（`0.7.4` → `0.7.5`） | 配置面（修改） | 单行 JSON 数据，零逻辑面，无兄弟测试文件可挂靠 | AC-10：`git diff -U0 -- packages/desktop/package.json` 只有 `version` 一行且相对落地时 `HEAD` 恰 +1 patch；`tauri.conf.json` 仍 `"version": "../package.json"`；`src-tauri/Cargo.toml` 的 `version = "0.1.0"` 行 `git diff` 为零 |
| AC-9：五份 delta spec 成文与反向判据 | spec 文本 | 「纪律是否写全 / 措辞是否持有唯一事实」无法用 `assert!` 表达；反向判据（全仓无「sync 命令不在主线程执行」残留）是**归档后**读数——主 spec 旧句由 delta 覆盖，本变更 MUST NOT 手改 `openspec/specs/**`（事实校准 8） | 逐份人工复核：`desktop-app-shell` 新纪律的形态 / 反形态 / 首次 poll 判据 / 适用面四条 + `code_stats` 债务 / `JoinError` 前向防御 / 并发度不变 / 出线零变化各至少一个 scenario；`desktop-archi-drift` 的「并行全仓扫描、无取消无去重」条 + scenario；`desktop-workspace-code-stats` 旧句已改写。归档后执行 `grep -rn "sync 命令不在主线程执行" openspec/specs/` 零命中 |
| AC-11：验收清单落盘 + 环境不具说明 + 归档后人工验收承接 | 留痕（只读核对）+ 人工验收 | 六项真机观测（窗口可拖动 / 不白屏 / 三命令起止时刻 / 连点刷新基线 / 冷热缓存）依赖真实大仓库 + release 构建，且不落任何 agent 可读产物（GUI 交互与时刻读数无读取面）⇒ 观测在 agent 相位不可履行，写成门必败且重做同因再败（proposal R-13；先例 `desktop-agent-execution` 边界第 16 条 / 场景「相邻 assistant 消息不设实现期核验门」） | `openspec/changes/archi-file-tree-lazy-load/reports/acceptance.md` **在案 + 三节齐备**（只读核对，缺一节即不成立）：① **验收清单**六项（逐项给出观测手段 / 预期 / 判据来源）：(1) release 构建（`pnpm build:prod`）进 `/architecture`——窗口可拖动 / 可切页 / 不白屏、「解析中…」可见；(2) 切「文件」同样不冻结；(3) `archi_graph` / `archi_drift` / `archi_file_tree` 三命令起止时刻（确认 `archi_drift` 仍最大项、`archi_file_tree` 次之）；(4) debug 构建（`pnpm build`）对照一次（debug 数字不当基线）；(5) 连点刷新 N 次的现象与耗时（R-1 实证基线）；(6) 冷 / 热缓存各一次（首次走 16 s IO，其后走 page cache）。条目数 MUST NOT 增删；② **环境不具说明**（63k 工作区 `/d/Projects/wpsweb/client/app` 在机但无 release 产物、观测无 agent 可读面）；③ **归档后人工验收承接**（六项观测与结论并入归档后人工验收、结论记文末；不符预期按 R-1 / R-10 另立变更，MUST NOT 回退本 change）。**真机观测本身 MUST NOT 作门禁判据 / 归档阻塞项 / `tasks.md` 待办项**（写清单文件的收尾任务不在此列）；「不冻结」的机械判据是 AC-2 首次 poll（`mod.rs` 节第 1 行），时长非承诺面（R-10） |
| AC-2 的「时长」面与首屏提速百分比 | 非判据（MUST NOT） | spec `desktop-app-shell` 与 design R-4 明令 MUST NOT 以计时断言代替首次 poll 判据；本变更口径是「不冻结」而非「更快」（proposal 命名与口径 + R-10） | 判据由首次 poll 用例承接（`mod.rs` 节第 1 行）；时长只在归档后人工验收观测（`reports/acceptance.md` 清单第 (3) / (5) / (6) 项），不设阈值、不作门禁判据、不阻塞归档 |
| `JoinError` 臂的运行期触发（R-2 / D4） | 前向防御 | 只在阻塞任务 panic 时出现；无 test-only 注入缝（spec「零注入点」+ AGENT.md「no test-only exports」同源），构造它需要专门的可 panic 注入面 | 可测半边落 `mod.rs` 节：四值文案与双层 `Result` 形态的源码字面量断言；运行期面登记为前向防御，不新增用例 |
| `packages/desktop/src/types/generated/bindings.ts` + `bindings.zod.ts` | 生成物 | `bindings:export` 机械产出，零手写面；asyncness 不进产物（specta 显式处理） | AC-4：`pnpm run bindings:export` 后 `git diff` 零行（幂等，再跑一次仍零 diff） |
| `packages/desktop/src-tauri/src/bindings/mod_test.rs`（**零改动**） | 既有测试文件零 diff 面 | 本变更明令零改动；它是「出线面未被动」的**反证锚**（需要改它就说明 async 化动了出线面，须回阶段一查形态而非改测试） | AC-4：`git diff --stat` 为空 + 该文件全绿；其中 `ARCHI_COMMANDS` 八命令名单与相邻序、`archiFileTree` 的逐字签名（`archiFileTree: (root: string) => __TAURI_INVOKE<ArchiFileTree>("archi_file_tree", { root })`）、zod 两表断言即运行期证明 |
| `packages/desktop/src/views/architecture/components/file-tree.test.tsx`（**零改动**） | 既有测试文件零 diff 面 | 直测 `FileTree`，不经缺省视图 ⇒ 无新增断言面（回归登记见 `file-tree.tsx` 节第 3 行） | AC-8：`git diff --stat` 为空 + 20 条全绿 |
| `packages/desktop/src/views/changes/change-detail-view.test.tsx`（挂载点 B 宿主，**零改动**） | 既有测试文件零 diff 面 | 其 mock 未桩 `archi_file_tree`（缺省 `resolve(null)`）、既有断言只锚 `archi_graph` ⇒ 缺省换面后无需改动；挂载点 B 的「缺省元素树 + 0 次」由 `architecture-canvas.test.tsx` 的 `mode="incremental"` 组合行等价承接 | `git diff --stat` 为空 + 全绿。若该文件因缺省换面需要修改，即证明缺省值不再是单点（回 `architecture-canvas.tsx` 查，而非改测试） |
| `packages/desktop/src-tauri/crates/core/archi/**`（`tree.rs` / `tree_test.rs` / `dto.rs` / `drift/**` / `scan/**` / `api/**` / `lib.rs`） | 零 diff 面 | 判定 / 投影 / 归属 / 标记语义零改动（AC-8）；async 化只动命令层 wrapper，领域层无可测新增面 | `git diff --stat` 六个路径为空 + 该 crate 既有用例全绿 |
| `packages/desktop/src/views/architecture/hooks/**`（含 `use-archi-file-tree.ts` 与其测试）、`element-tree.tsx`、`model-info.tsx`、`contract-panel.tsx`、`lib/**` | 零 diff 面 | D9 明令零改动；「零改动」本身不是可断言行为（其行为侧已由既有用例锁定） | `git diff --stat` 各路径为空 + 既有用例全绿 |
| `.dev-team/architecture/**`（模型 / 契约资产 / `INDEX.toml`） | 零 diff 面 | 零新增模型元素与契约资产（R-8：`api = declared` 但契约缺席，`docMissing` 仅 warn；纪律由 spec 承载）。`App.Frontend.Views.Architecture` 的 summary 只枚举「文件树 / 元素树 / 契约三视图」、不陈述缺省视图 ⇒ 缺省换面无需模型随动；`App.Backend.Core.Archi` 与其契约资产同属 `api = declared` 无契约面，命令面形态由 `desktop-app-shell` spec 承载而非模型 | `git diff --stat .dev-team/` 为空；架构页「契约」视图不变即为等价观测（`apiFindings` 仍为 `docMissing` warn 面） |
| `openspec/specs/**`（除本 change 5 份 delta 外） | 零 diff 面 | 主 spec 零 diff 是本变更的硬线（AC-10）；`desktop-archi-model` / `desktop-ipc-type-bindings` 的零 delta 是判据之一 | `git diff --stat openspec/specs/` 为空 + 两份零 delta 能力的 `git diff` 逐条为空 |
| `ArchiDriftMode` 的非法枚举值（Enum N+1 的「+1」半边） | 类型面（core，零改动） | 枚举定义在 `crates/core/archi/src/drift/**`（本变更零改动面）；非法值在 IPC 反序列化层被拒，`pub async fn` 层结构上不可构造 | 两合法值的行为面由 `mod.rs` 节 drift 行承接；非法值面由 core 既有 serde 用例（零 diff）承接 |
| 另四条命令与 `code_stats` / `workspace_config` 的线程形态（不在适用面） | 零 diff 面 + 债务登记 | 本变更明令不改（spec「明确不在适用面」）；`code_stats` 的 async 化归独立变更 | `mod.rs` 节源码计数承接（`pub fn archi_` = 12 含四命令与八 `*_inner`、`spawn_blocking` 在它们体内零命中）+ `commands/stats/**` / `commands/config/**` / `commands/mod.rs` 零 diff |
| 注释措辞的「贴切度」（`architecture-canvas.tsx` 两处 doc + `file-tree.tsx` 一行） | 注释 | 注释无可执行行为面；本设计已对「是否仍陈述旧缺省」设源码断言，但「措辞是否表达正确意图」不可机械判定 | code review + 两份 delta spec 的对应条（`desktop-archi-file-tree` 缺省元素树条 / `desktop-archi-canvas` 模块增量表） |
| AC-10 的 `git status --porcelain` 改动集上界 | 机械核对 | 属工作流收尾核对（非单测） | `git status --porcelain` 的改动集 ⊆ 4 份实现文件 + 本 change 目录（design / proposal / tasks / test-design / specs / reports）；`git diff --stat` 对「不在本清单的对象」全零 |
