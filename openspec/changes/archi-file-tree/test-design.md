# 测试设计: archi-file-tree

> **变更**: archi-file-tree
> **角色**: test-design-planner（executor）—— 从 `design.md` 推导单元测试范围与公共 API 签名
> **输入**: `proposal.md`（五症状 / 提案一~六 / 17 条决策 / 变更范围 14 实现项 + 11 测试文件行 / AC-1…AC-17 / R-1…R-14）、`design.md`（D1–D17 / 八则事实校准 / 架构组件 13 行 / 新增文件 4 项 + 修改文件 10 项 / 公共函数·API 表 12 行 / 类型定义 9 行 / `tree.rs` 内部拆分与主流程伪码 / 清单守卫随动表 8 行 / AC 落点 / 待决问题 1..6）、七份 delta spec（`desktop-archi-file-tree` 6 ADDED 21 scenario / `desktop-archi-canvas` 3 MODIFIED / `desktop-archi-model` 1 RENAMED / `desktop-archi-api` 1 MODIFIED / `desktop-archi-drift` 1 MODIFIED / `desktop-ipc-type-bindings` 1 MODIFIED / `repo-archi-model` 1 MODIFIED + 1 ADDED）、`tasks.md`（十阶段），以及工作树源码与既有测试盘点
> **被测范围**: design「变更清单」的**新增文件 4 项 + 修改文件 10 项全列**（`crates/core/archi/src/tree.rs`、`.dev-team/architecture/config.json`、`use-archi-file-tree.ts`、`file-tree.tsx`；`scan/owner.rs`、`scan/mod.rs`、`dto.rs`、`archi/lib.rs`、`commands/archi/mod.rs`、`commands/mod.rs`、`architecture-canvas.tsx`、`package.json`、两份生成物）。本变更**零删除文件**。无测试映射对的 6 项（配置资产 / 出口面 / 版本行 / 两份生成物 / 测试文件本体）登记于「不可测试项」。
> **契约基准**: 用例一律按 `design.md` 的**定稿契约**设计（含 D1 候选集合并定档、D2 `module_roots` / `contains_count` 落 `OwnerIndex`、D3 模型读取面唯一入口 `dto::graph_view`、D8 `kind` / `exists` 判定、D13 三态挂载层级、D14 逐字文案、D15 `forcedOpen` 派生 + 可选 `scrollIntoView`），不按工作树当前形态回填用例。
> **契约与工作树的已知差异（用例按契约侧写、差异由实现相位补齐）**: ①`tree.rs` / `tree_test.rs` 尚不存在（`tasks.md` 阶段三建空模块 `tree_test.rs`，仅头注释、零用例）；②`strip_generated` 仍是「只替换扩展名」单候选形态，`strip_candidates` / `longest_prefix_hits` / `module_roots` / `contains_count` 四件未落；③`scan::walk_source_files` / `is_source_file` / `posix` 仍私有；④`dto.rs` 无四个新类型；⑤`commands/archi/mod.rs` 仍是五命令、`commands/mod.rs` 的 archi 段仍五条、`bindings/mod_test.rs` 仍是 48/97/4/19；⑥`use-archi-file-tree.ts` / `file-tree.tsx` 不存在；⑦`architecture-canvas.tsx` 左栏仍是裸 `<ElementTree>`；⑧`.dev-team/architecture/config.json` 不存在。测试相位不因差异删用例，也不把用例改写成旧形态。

**测试框架与既有约定（盘点结论）**

| 面 | 框架 / 运行器 | 文件形态 | 断言与替身 |
|---|---|---|---|
| Rust（`crates/core/archi`） | `cargo nextest run --workspace`（`server:check` = `cargo fmt --check` + `clippy --all-targets` 作编译闭合）；**无 dev-dependency**，临时目录用 std 自建 | 与被测源文件**同 crate 共置**的 `<模块名>_test.rs`，由源文件尾或 `lib.rs` 的 `#[cfg(test)] mod <模块名>_test;` 声明（`scan/owner_test.rs` / `scan/mod_test.rs` 由 `scan/mod.rs` 声明，`dto_test.rs` / `tree_test.rs` 由 `lib.rs` / `tree.rs` 声明）；crate 级集成测试在 `tests/` | `#[test]` + 中文长句用例名 + `assert_eq!` / `assert!` / `matches!`；`TempDir` 自建装置（`std::env::temp_dir()` + `AtomicU64` 计数 + `Drop` 清理）；错误面用 `expect_err` + 文案子串断言 |
| Rust（app 壳 `src-tauri`） | 同上（app crate 有 `tauri` test 面） | `commands/archi/mod_test.rs`（`#[cfg(test)] mod mod_test;`）、`bindings/mod_test.rs` | Tauri 运行时替身用 `tauri::test::mock_app()`（进程边界装置，非仓内模块 mock）；`serde_json::to_value` 逐字段对拍；`snapshot(root) -> Vec<(PathBuf, Vec<u8>)>` 断言零写盘 |
| 前端（`packages/desktop/src`） | `vite-plus` 的 test 面（vitest API，`vp test`；`vp check` 的 `typeCheck` 作编译闭合）；`vite.config.ts` 的 `test` 节：`environment: 'jsdom'`、`include: ['src/**/*.test.ts?(x)']`、`setupFiles: ['src/test-setup.ts']` | 与被测源文件**同目录共置**的 `<模块名>.test.ts`（`.test.tsx` 仅组件渲染面） | `import { describe, expect, it, vi } from 'vite-plus/test'` + `@testing-library/react`（`render` / `rerender` / `renderHook` / `screen` / `act` / `fireEvent` / `waitFor`）；查询一律 `data-testid` / `data-mark` / `data-path` / `data-mode` 钩子；hook 用例 mock `@tauri-apps/api/core` 的 `invoke`（进程边界） |

**用例表列语义**

- **路径类型**：`正向`（契约内输入 / 预期输出）/ `异常`（契约外输入、失败路径、被禁止的写法、反向判据）/ `边界`（空、空串、`null`、极值、恰好阈值、生命周期交错点、越签名脏值）。
- **迭代类型**：`单次`（一次驱动即判）/ `参数化迭代`（同一断言逻辑对表驱动数据集逐项执行）/ `组合迭代`（两个以上维度交叉）。
- **`测试条件` 内的「既有…」前缀**：该臂已被既有用例锁定，本设计**只做回归登记、不新增断言**；未带前缀的为**新增**或**按新形状改写**（改写只动「取哪条断言路径」，语义断言逐条保留——R-7 / R-8）。
- **参数类型边界口径（贯穿各章节，按 Phase 规定的 Type → Edge Case 映射）**：`root: &Path` / `root: String` 取不存在路径、指向文件、空 `Path` / `""`、空白串 + 常规（`str` 侧另加超长 >1000 chars 与含空格 / 非 ASCII / emoji）；`rel_path: &str` / `path: &str` 取 `""`、超长（>1000 chars）、含 `\n` `\0` emoji、`None` 不适用（Rust `&str` 非空型，以空串代「无值」）；`entries: Vec<…>` 取 `[]` / 单元素 / 超大列表 / `undefined`（前端越签名脏值）；`focused: string | null` 取 `null` / `''` / 命中 FQN / 零命中 FQN；枚举 `FileTreeMark`（5 值）与 `FileEntryKind`（2 值）取**每值 + 非法值**（N+1）；`u32`（`contains`）取 `0` / `u32::MAX`；`bool`（`exists`）取 `true` / `false`。

**待测功能条目口径**

`#### 待测功能` 只列 design「公共函数 / API」表中「所在文件」为本源文件的条目（`tree.rs` 1 行、`scan/owner.rs` 3 行、`scan/mod.rs` 3 行、`commands/archi/mod.rs` 2 行、`use-archi-file-tree.ts` 1 行、`file-tree.tsx` 2 行；`dto.rs` / `commands/mod.rs` / `architecture-canvas.tsx` 在该表**零行**，条目容器以 HTML 注释登记来源表并留空或改列「修改文件 / 类型定义」表）。design 明示的私有实现件（`tree.rs` 的 `candidate_files` / `node_set` / `ancestors` / `name_of` / `mark_of` / `exists_of`、`strip_candidates` / `longest_prefix_hits`、`file-tree.tsx` 的 `buildForest` / `FileRow` / `TreeNodeList` / `isExpanded`）**不列本表**，其覆盖经所在文件的公共 API 用例承接——不 grep 源码导出声明、不虚构 API 行。design「类型定义」表的 interface / type 不是「公共函数 / API」行，形状覆盖并入对应函数用例的入参 / 出参断言，类型错由 `cargo clippy` / `vp check` 强制（见「不可测试项」）。

**测试布局决策（源文件 → 测试文件的映射依据）**

1. **新建 3 个测试文件**：`crates/core/archi/src/tree_test.rs`（design「编译闭合随动」已建空模块，用例本相位设计）、`packages/desktop/src/views/architecture/hooks/use-archi-file-tree.test.ts`、`packages/desktop/src/views/architecture/components/file-tree.test.tsx`——前两沿同目录共置定式，第三个沿 `element-tree.test.tsx` / `dir-tree.test.tsx` 的渲染面定式。
2. **复用 6 个既有测试文件**：`scan/owner_test.rs`（剥离判据改写 + 并集定档新增）、`scan/mod_test.rs`（三 helper 提权后的行为回归 + 可见性自证）、`dto_test.rs`（四新类型形状与词表）、`commands/archi/mod_test.rs`（新命令两函数 + root 校验扩到六命令）、`src/bindings/mod_test.rs`（清单守卫随动）、`architecture-canvas.test.tsx`（左栏替换后的断言改写）。`tests/vocabulary_test.rs` 与 `element-tree.test.tsx` **零改动**（前者 `FileTreeMark` 不进四面词汇对拍，后者元素树组件零改动）。
3. **`commands/mod.rs` 的登记面断言落 `src/bindings/mod_test.rs`**：该文件无独立 sidecar（`commands/mod.rs` 只有 `all_commands!` 宏体），登记顺序与计数只在 `bindings/mod_test.rs` 的清单守卫可判（AC-1 / AC-14 的验证方法亦点名该文件）。
4. **组合用例挂靠按链路发起方**：①「磁盘 → 树条目 → 左栏行 → 点击聚焦 → 抽屉取数」的入口是 `architecture-canvas.tsx`（它持有 `useArchiFileTree` 与 `CanvasBody`），故跨模块组合行挂 `architecture-canvas.tsx` 章节；②「模型 → 归属 → mark / owners / contains / exists」的入口是 `tree::file_tree`，组合行挂 `tree.rs` 章节；③hook 自身的触发生命周期挂 hook 章节。不新建独立集成测试章节或组合测试区。
5. **不 mock 被测件与被测链路内部**：`FileTree` / `useArchiFileTree` / `ArchitectureCanvas` 之间，以及与 `ElementTree` / `ElementDrawer` / `structure-error-panel` / `lib/marks` / `drift-marks` / `fqn` / `ui/scroll-area` 全部真实组合；Rust 侧 `tree.rs` 与 `dto` / `scan` / `drift::config` / `OwnerIndex` 全部真实调用——替身化等于让「判定单源」「枚举面一致」自证（AGENT.md「No test-only exports」）。
6. **jsdom 环境桩只在实例级装**：`element.scrollIntoView?.()` 是**可选调用**（D15），jsdom 无该 API 时静默跳过，故「滚到第一条」的断言改为「命中行集合 + 首条尝试调用」的实例级最小桩（`scrollIntoView = vi.fn()`），**不进** `test-setup.ts` 原型；真实滚动视觉效果归手工验证（见「不可测试项」）。

---

## 验收范围

<!-- 测试区域：Rust 面 `packages/desktop/src-tauri/crates/core/archi/src/**` + `packages/desktop/src-tauri/src/**`（`cargo nextest`，各源文件共置 `_test.rs`）；前端面 `packages/desktop/src/views/architecture/**`（`vp test`，jsdom，`setupFiles: src/test-setup.ts`）。本变更新建 3 个测试文件、改写 6 个既有测试文件、`tests/vocabulary_test.rs` 与 `element-tree.test.tsx` 零改动。 -->
<!-- 零 diff / 门禁 / 生成物 / 版本行 / 配置资产不进入 `## 单元测试`，其验收方式逐条列于「不可测试项」。 -->

| AC ID | 验收条件 | 被测文件或模块 |
|---|---|---|
| AC-1 | **新命令可数且只读**：`archi_file_tree(root)` 在 `all_commands!` 登记、有 typed 包装，archi 段命令总数 6；调用后磁盘字节零变化 | `packages/desktop/src-tauri/src/commands/archi/mod.rs` → `commands/archi/mod_test.rs`；`packages/desktop/src-tauri/src/commands/mod.rs` → `src/bindings/mod_test.rs` |
| AC-2 | **结构诊断非空即 `Err`，空模型不报错**：任一结构诊断 → `Err`（不返回部分树）；模型目录缺席 / 零元素 → `Ok`（全条目 `mark = none`、`owners` 空） | `commands/archi/mod.rs` → `commands/archi/mod_test.rs`；`crates/core/archi/src/tree.rs` → `tree_test.rs` |
| AC-3 | **DTO 扁平且确定**：`entries` 扁平数组、`path` 字节序；`ArchiFileEntry` 七字段按声明序出线为 camelCase；`ArchiFileTree` 一字段 | `crates/core/archi/src/dto.rs` → `dto_test.rs`；`tree.rs` → `tree_test.rs`；`commands/mod.rs` → `src/bindings/mod_test.rs` |
| AC-4 | **`FileTreeMark` 封闭五值**：`ALL` 长度断言与 `as_str` 逐值对拍；优先级单值；不复用 `NodeMark`、不进四面词汇对拍 | `dto.rs` → `dto_test.rs`；`tree.rs` → `tree_test.rs`（含 `tests/vocabulary_test.rs` 零改动登记） |
| AC-5 | **归属判定正确**：目录型根 → `elementRoot`；内部文件 → `member`；祖先 → `containsElements` 且 `contains` 按 FQN 去重；单文件型根 → 文件节点 `elementRoot`；多 `path` → 多根且不重复计数；嵌套归最深；同路径多元素声明 → `ambiguous`；容器元素零节点 | `tree.rs` → `tree_test.rs`；`crates/core/archi/src/scan/owner.rs` → `scan/owner_test.rs` |
| AC-6 | **判定单源**：`tree.rs` 不含归属匹配实现，标记只经 `OwnerIndex::owner_of` + `module_roots` 查表；`src` 全域无第二份前缀匹配 / 生成物剥离 | `scan/owner.rs` → `scan/owner_test.rs`（`module_roots` / `contains_count` 单点）；`tree.rs` → `tree_test.rs`（源码自证用例）；静态 `grep` 判据见「不可测试项」 |
| AC-7 | **A2″ 并集生效**：根同级形态归元素；四条目录内形态保持；`src/dir.i.ts` 由 `None` 翻 `Element`；`src/dir/` 与 `src/dir.ts` 同场探针归更具体根（禁短路）；白名单外后缀零误剥 | `scan/owner.rs` → `scan/owner_test.rs` |
| AC-8 | **枚举面 = 漂移判定面**：树经 `ExcludeSet::build(config.exclude)`；测试面与配置 `exclude` 命中项不在树；`target` / `node_modules` / `dist` / `build` / `.git` / 隐藏项 / gitignore 命中项不出现；`scan/exclude.rs` 与 `exclude_test.rs` 零 diff | `tree.rs` → `tree_test.rs`；`crates/core/archi/src/scan/mod.rs` → `scan/mod_test.rs`（三 helper 提权语义零变化）；零 diff 见「不可测试项」 |
| AC-9 | **本仓配置落位**：`.dev-team/architecture/config.json` = `{ "exclude": ["openspec/**"] }` 且无 `$schema`；`archi_drift` 与 `archi_file_tree` 均不因此 `Err`；`openspec/` 下 3 个原型 tsx 不在树中 | `.dev-team/architecture/config.json`（配置资产 ⇒ 「不可测试项」）；`tree.rs` → `tree_test.rs`（`exclude` 生效 + 无 `$schema` 等价形态经合成 tempdir）；`commands/archi/mod.rs` → `commands/archi/mod_test.rs` |
| AC-10 | **前向防御分支**：声明但目录不存在的模块根仍出节点且 `exists = false`（合成 tempdir 用例） | `tree.rs` → `tree_test.rs` |
| AC-11 | **左栏替换与分段切换**：缺省渲文件树；切到元素树 testid 不变；归属行点击 → 一次 `setFocusOverride`；非归属行零 `onFocus`；多 `path` 焦点多行高亮 + 滚到首条；零命中顶部提示不滚动 | `packages/desktop/src/views/architecture/components/file-tree.tsx` → `file-tree.test.tsx`；`packages/desktop/src/views/architecture/components/architecture-canvas.tsx` → `architecture-canvas.test.tsx` |
| AC-12 | **树取数独立失败**：失败只渲 `archi-file-tree-error` 行内提示、画布与抽屉照常；loading 呈「解析中…」、树空呈「仓库内无源文件」；停摆态整块面板、树不挂载 | `file-tree.tsx` → `file-tree.test.tsx`；`architecture-canvas.tsx` → `architecture-canvas.test.tsx`；`use-archi-file-tree.ts` → `use-archi-file-tree.test.ts` |
| AC-13 | **零 watch 零轮询**：三触发（挂载 / `root` 变化 / 手动刷新经 `key` 重挂）；源码零 `watchSubscribe` / 零定时器 | `use-archi-file-tree.ts` → `use-archi-file-tree.test.ts`；源码 `grep` 判据见「不可测试项」 |
| AC-14 | **清单守卫随动**：命令 48 → 49、出线 DTO 97 → 101、`ARCHI_COMMANDS` 4 → 6、`ARCHI_DTO_TYPES` 19 → 23；尾部追加序把 `archi_file_tree` 插在 `archi_drift` 之后（`read_file` 仍尾条）；`清单遗漏即产物覆盖断言必红` 对照臂随之改判 | `commands/mod.rs` → `src/bindings/mod_test.rs`；生成物入库零 diff 见「不可测试项」 |
| AC-15 | **本仓实测基线**（非 MUST，登记留痕）：进树文件 358 / 合成目录 117 / 节点 475 / `mark` 分布 / 42 个元素根各 ≥ 1 文件 | 「不可测试项」（一次性统计用例或 ad-hoc 脚本 + 留痕） |
| AC-16 | **门禁**：`pnpm -C packages/desktop run check` 绿；`cargo nextest run --workspace` 全绿；`package.json` `0.6.35 → 0.6.36`，`src-tauri/Cargo.toml` 版本行不变 | 「不可测试项」（机械门禁）；`package.json` 归「不可测试项」 |
| AC-17 | **边界零 diff**：`models/**` / `decisions/**` / `api/**` / `drift/**` / `infra/archi/**` / `infra/agent/**` / `scan/exclude.rs` / `.dev-team/config.json` / `explore.md` 全部零 diff | 「不可测试项」（`git diff --stat` 审查） |

---

## 单元测试

### `packages/desktop/src-tauri/crates/core/archi/src/tree.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/tree_test.rs`

**新建测试文件内容（`tasks.md` 阶段三已建空模块，仅头注释）**——沿 `docs_test.rs` / `scan/mod_test.rs` 的零 mock 定式：真实 tempdir（`<root>/.dev-team/architecture/models/*.toml` 落模型、真实源文件落盘）+ 真实 `ignore::WalkBuilder` + 真实 `drift::config::load_config` + 真实 `ExcludeSet` + 真实 `OwnerIndex`（D9 / D5 即被测语义，任何一层替身都会抹掉「枚举面 = 漂移判定面」）。`TempDir` 装置自建（本 crate 无 dev-dependency），`Drop` 清理并带 `AtomicU64` 唯一后缀。fixture 构造器需三件：`write_model(name, toml)`、`write_file(rel, content)`、`write_config(json)`（后者写 `<root>/.dev-team/architecture/config.json`）。

#### 待测功能

- `file_tree(root: &Path) -> Result<ArchiFileTree, String>`: 唯一公共入口（design「公共函数 / API」表）——读模型（诊断非空 → `Err`；空模型 → 全 `none`）→ 读 `exclude` → 枚举 + 排除 → 模块根表 / 归属 → 目录合成 → `mark` / `owners` / `contains` / `exists` → `path` 字节序。纯读盘、零写盘、零 spawn

<!-- design.md「公共函数 / API」表对本文件恰 1 行（`tree::file_tree`）；四个私有 helper（candidate_files / node_set / mark_of / exists_of）与 ancestors / name_of 不入表，其行为经 file_tree 的条目集合与 kind / mark / owners / contains / exists / path 逐字段观测（禁为测试放宽可见性）。tree_test 是 tree 的子模块，私有件技术上可达——但沿 owner_test 先例一律只经公共面观测，MUST NOT 直调私有 helper（否则「判据顺序 ①②③」可被绕过断言）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `file_tree` 目录型元素根与其内部文件（AC-5 / spec Scenario 1） | 正向 | 元素 `App.Core.Archi` 的 `path = ["./crates/core/archi/"]`，盘含 `crates/core/archi/src/lib.rs` 与 `src/dto.rs` → `crates/core/archi` 条目 `kind = "dir"` / `mark = "elementRoot"` / `owners = ["App.Core.Archi"]` / `exists = true`；`crates/core/archi/src` 与两个 `.rs` 文件均为 `mark = "member"` / `owners = ["App.Core.Archi"]`、文件条目 `kind = "file"` | 单次 |
| `file_tree` 单文件型元素根（AC-5 / spec Scenario 2） | 正向 | 元素 `App.Frontend.Views.Welcome` 的 `path = ["./packages/desktop/src/views/welcome-view.tsx"]` → 该文件条目 `kind = "file"` / `mark = "elementRoot"` / `owners = ["App.Frontend.Views.Welcome"]` / `exists = true` | 单次 |
| `file_tree` 祖先目录 `containsElements` 与 `contains`（AC-5 / D6③） | 正向 | 三个元素根分别落在 `src/a/` / `src/b/` / `src/c/` → `src` 条目 `mark = "containsElements"` / `owners = []` / `contains = 3`；`src/a` 为 `elementRoot` 且 `contains = 0` | 单次 |
| `file_tree` `contains` 与 `mark` 正交并存（AC-5 / spec Scenario「正交计数与标记并存」） | 正向 | 父元素 `path = ["./crates/core/archi/"]`、子元素 `path = ["./crates/core/archi/src/scan/"]`，两目录都在盘 → `crates/core/archi/src` 条目 `mark = "member"`（归父元素）+ `contains = 1`（子元素根在严格前缀内）+ `owners = ["App.Core.Archi"]` | 单次 |
| `file_tree` 多 `path` 元素与 `contains` 去重（AC-5 / spec Scenario「多 path 元素与 contains 去重」） | 正向 | 元素 `App.Frontend.Shell` 声明四条文件形 `path`（`src/shell/{index.ts,app.ts,routes.ts,store.ts}`）→ 四条各出 `elementRoot` 节点且 `owners` 均为该 FQN；祖先目录 `src/shell` 的 `contains` 计 **1**（按 FQN 去重）而非 4 | 单次 |
| `file_tree` 嵌套父子各 `elementRoot` 且内层文件归最深（AC-5 / spec Scenario「嵌套父子各出根」） | 正向 | 父 `path = ["./crates/core/archi/"]`、子 `path = ["./crates/core/archi/src/scan/"]`，盘上 `.../src/scan/owner.rs` 在场 → 两目录各 `elementRoot` 且 `owners` 分别为父 / 子；`owner.rs` 归子元素（最深前缀）；父目录 `contains = 1` | 单次 |
| `file_tree` 逐节点判据②的 `ambiguous` 分支（AC-5 / spec Scenario「同长多命中判 ambiguous 不任选」/ R-10） | 异常 | 合成语料：两元素声明**同一个目录** `path`（`./src/x/` 与 `src/x`，归一后同前缀），探针取该目录**内部**的文件 `src/x/f.ts`（该节点**不是**模块根，故不走 `module_roots` 分支，而走 `owner_of` 的两条等长命中）→ 该文件条目 `mark = "ambiguous"` / `owners = [A, B]`（字典序）/ MUST NOT 只给其一。注：等长的**不同**前缀在 `prefix_matches`（全等或目录前缀）下结构上不可能同时命中，故本仓库该分支的唯一可达来源是「同一归一路径重复声明」——与下一行构成同一分支的两条用例 | 单次 |
| `file_tree` 同一归一路径被多元素声明 → `ambiguous`（AC-5 / spec Scenario「同一归一路径被多元素声明」） | 异常 | 两元素分别写 `./src/shared/` 与 `src/shared`（归一后相同）→ `src/shared` 条目 `mark = "ambiguous"` / `owners = [A, B]`（字典序）/ MUST NOT 判 `elementRoot` | 单次 |
| `file_tree` 容器元素零节点（AC-5 / spec Scenario「容器元素零节点」） | 异常 | 模型含 `path = []` 的 `domain` 与 `package` 容器元素、盘上有若干源文件 → 两元素在任何条目的 `owners` 中**零出现**、不产任何节点（只在元素树上可见） | 单次 |
| `file_tree` 元素根判据是 `path` 非空而非 `kind`（AC-5 / D4） | 正向 | `kind = "domain"` 但 `path = ["./src/components/agent/index.tsx"]` → 该文件条目 `mark = "elementRoot"`（不因 `kind` 漏判 barrel 型 domain） | 单次 |
| `file_tree` 生成物按并集判据归元素（AC-7 / spec Scenario「生成物按并集判据归元素」） | 正向 | 元素 `App.Backend.Infra.ArchiRuntime` 的 `path = ["./crates/infra/archi/"]`，盘上存在 `crates/infra/archi.i.rs`（**根同级**形态）→ 该文件条目 `mark = "member"` / `owners = ["App.Backend.Infra.ArchiRuntime"]`（不落 `none`） | 单次 |
| `file_tree` 结构诊断非空即 `Err`（AC-2 / spec Scenario「结构诊断非空即 Err」） | 异常 | 模型含一处环 / 孤儿层级 / 未知引用任一 → `Err`，文案含「模型结构错」与诊断条数；MUST NOT 返回部分树（`expect_err` 后无 `ArchiFileTree` 可读） | 参数化迭代（3 诊断类） |
| `file_tree` 空模型返回全 `none` 而非错误（AC-2 / spec Scenario「空模型返回全 none」） | 边界 | ①模型目录缺席（`<root>/.dev-team/architecture/models` 不存在）②`models/` 存在但零 `*.toml` → 均 `Ok`；条目集合为仓库源文件与合成目录，**全部** `mark = "none"` / `owners = []` / `contains = 0`；MUST NOT `Err` | 参数化迭代（2 形态） |
| `file_tree` 声明但目录不存在的根仍出节点（AC-10 / spec Scenario「声明但目录不存在仍出节点」/ R-11） | 边界 | 合成 tempdir：元素 `path = ["./src/ghost/"]` 而 `src/ghost/` 不在盘（同目录其余文件正常）→ 树中仍有 `src/ghost` 条目：`kind = "dir"` / `mark = "elementRoot"` / `owners` 含该 FQN / **`exists = false`**；父目录 `src` 出现且 `contains = 1` | 单次 |
| `file_tree` 在盘目录与非根节点 `exists` 恒真（AC-10 / spec Scenario「在盘目录 exists 为真」） | 正向 | 元素 `path = ["./crates/core/archi/"]` 且目录在盘 → `exists = true`；合成目录条目与非目录条目一律 `exists = true`（`exists = false` 只属「`Dir` 且为模块根且不在盘」） | 单次 |
| `file_tree` 路径形态与 `name` 派生（AC-3） | 边界 | 顶层条目 `path` 即其单段名（`src` → `path = "src"` / `name = "src"`）；嵌套条目 `name` 为末段（`crates/core/archi` → `archi`）；全部条目 `path` **无前导 `./`、无尾 `/`**；MUST NOT 产出 `path = ""` 的根条目 | 单次 |
| `file_tree` 扁平数组与 `path` 字节序（AC-3 / spec Scenario「扁平条目按 path 字节序且幂等」） | 正向 | 同一 root 连调两次 → 两次 `serde_json::to_value` 输出逐字节相同；`entries` 的 `path` 序列单调不减（字节序）；同一文件与同一目录各恰一条（去重） | 单次 |
| `file_tree` 测试面不在树中（AC-8 / spec Scenario「测试面不出现」） | 异常 | 盘上造 `src/a.test.tsx` / `crates/core/archi/src/tree_test.rs` / `src/b.spec.ts` / `src/__tests__/c.ts` / `crates/core/archi/tests/d.rs` / `src/fixtures/e.ts` / `src/__mocks__/f.ts` → 七条均不在树中；同目录非测试源文件（`file-tree.tsx` / `tree.rs`）照常出现 | 参数化迭代（7 排除约定） |
| `file_tree` 配置 `exclude` 命中项不出现（AC-8 / AC-9 / spec Scenario「配置 exclude 命中项不出现」） | 异常 | 架构域配置 `{ "exclude": ["openspec/**"] }`，盘上 `openspec/changes/x/prototypes/index.tsx` 未被 gitignore → 该文件与其各级目录均不在树中（该目录若无其余源文件则目录亦不出现）；对照：同一 config 下 `src/a.ts` 照常出现 | 单次 |
| `file_tree` 构建 / 依赖产物树与隐藏项不出现（AC-8 / spec Scenario「构建 / 依赖产物树不出现」） | 异常 | 盘上造 `target/debug/x.rs` / `node_modules/a/index.js` / `dist/y.ts` / `build/z.ts` / `.git/hooks/y.ts` / `.hidden/h.ts` → 六类均不在树中；同层正常源文件照常出现 | 参数化迭代（6 剪枝形态） |
| `file_tree` gitignore 命中项不出现（AC-8） | 异常 | `<root>/.gitignore` 写 `ignored/**` 且盘上有 `ignored/a.ts` → 不在树中（`walk_source_files` 的 gitignore 感知即被测语义） | 单次 |
| `file_tree` 非源文件扩展名不出现（AC-8） | 异常 | 盘上 `src/a.py` / `src/b.md` / `src/c.json` / `src/d.toml` / `src/e`（无扩展名）→ 均不在树中（`is_source_file` 五扩展名之外的判据）；`src/a.rs` 照常出现 | 参数化迭代（5 非源形态） |
| `file_tree` 非法配置使命令 `Err`（AC-8 / spec Scenario「非法配置使树进错误态」） | 异常 | 三种非法形态：①`exclude = ["src/legacy"]`（无通配符）②`exclude = ["  "]`（空白条目）③`exclude` 条目 `Glob::new` 失败（如 `"["`）→ 均 `Err`，文案沿用 `drift/config.rs` / `ExcludeSet::build` 的显式文案（含非法条目并提示 `/**` 形态） | 参数化迭代（3 非法形态） |
| `file_tree` 配置含 `$schema` 键 → `Err`（AC-9 / R-3） | 异常 | `<root>/.dev-team/architecture/config.json` 写 `{ "$schema": "…", "exclude": [] }` → `Err`（`RawConfig` 的 `deny_unknown_fields`）；反例：`{ "exclude": ["openspec/**"] }`（**无** `$schema`）→ `Ok` 且排除生效（真实 `.dev-team/architecture/config.json` 属配置资产，见「不可测试项」，此处为等价形态） | 参数化迭代（2 形态对拍） |
| `file_tree` `ignore` 键不参与树（AC-8 / D9） | 边界 | 配置含 `ignore = [{ from: 模型内元素, to: 模型内元素, reason: "…" }]` 与 `exclude = []` → 树条目集合与 `mark` 不因该键改变；配置本身合法（不 `Err`） | 单次 |
| `file_tree` 零进树源文件的空树（AC-12 的数据侧） | 边界 | 模型非空但仓库内零符合扩展名的源文件（只有 `.py`）→ `Ok` 且 `entries` 仅含模块根合成目录（不在盘则 `mark = "elementRoot"` / `exists = false`）；不 `Err` | 单次 |
| `file_tree` 只读零写盘与零 spawn（AC-1 / AC-8 / spec Scenario「只读零写盘」） | 边界 | 调用前后对 `<root>` 递归快照字节与 mtime 相等（沿 `commands/archi/mod_test.rs` 的 `snapshot()` 形态）；`include_str!("tree.rs")` 自证零 `fs::write` / `fs::remove` / `Command::new` / `std::process` 字面量 | 单次 |
| `file_tree` 判定单源（AC-6） | 异常 | `include_str!("tree.rs")` 自证：零 `starts_with` / 零 `replace(` / 零 `.i.` 字面量 / 零 `WalkBuilder` / 零 `read_dir` / 零 `fn is_source_file` 定义；`pub fn ` 计数恰 1（唯一公共入口）——反例臂：若把前缀匹配或归一写进本文件，该断言必红 | 单次 |
| `file_tree` `FileTreeMark` 五值在树上逐值可达（AC-4 / spec Scenario「五值各渲其标记」） | 正向 | 一份合成语料同时含元素根目录 / 同路径多元素声明 / 元素内文件 / 含模块的祖先目录 / 无主文件 → 五条分别取 `elementRoot` / `ambiguous` / `member` / `containsElements` / `none`；`owners` 依次 1 条 / ≥ 2 条 / 1 条 / 空 / 空 | 组合迭代（5 mark × owners 两维） |
| `file_tree` 优先级单值（AC-4 / spec Scenario「优先级单值」） | 边界 | 某节点既是元素根（`path` 命中）又位于父元素 `path` 内部 → `mark` 取 `elementRoot`（不取 `member`）、`owners` 为该元素 FQN；父元素不因此多出该节点的 `member` 表达 | 单次 |
| `file_tree` `root` 非法与字符串形态不 panic（边界） | 边界 | `root` 为空 `Path` / 不存在路径 / 指向文件 → 返回值落在 `Result` 内且不 panic（空 `Path` 的模型目录派生即空模型路径，用例内钉死实际落 `Ok` 或 `Err` 之一）；超长（>1000 chars）/ 含空格 / 非 ASCII / emoji 的 `root` → 不 panic | 参数化迭代（3 非法 + 4 字符串形态） |
| `file_tree` 超大 fixture 的规模与确定性（AC-15 的可判半边） | 边界 | 合成 300 文件 + 20 元素根 → 条目集合含全部文件与合成目录、`path` 单调不减、两次调用逐字节相同、不抛错（真实基线的 358 / 117 / 475 属「不可测试项」的一次性统计） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 文件系统 | **不 mock**：真实 tempdir（`std::env::temp_dir()` 自建，`AtomicU64` 唯一化、`Drop` 清理）+ 真实落盘模型 / 源文件 / 配置 / `.gitignore`——遍历、排除、归属全部读真盘（沿 `docs_test` / `scan/mod_test` 先例） | 全部用例 |
| `crate::dto::graph_view` / `crate::drift::config::load_config` / `crate::scan::exclude::ExcludeSet` / `crate::scan::{walk_source_files, is_source_file, posix}` / `OwnerIndex` | **不 mock、不替身**：全部真实调用。「枚举面 = 漂移判定面」「判定单源」即被测语义（AC-6 / AC-8），任一层替身都会让断言自证 | 全部用例 |
| `ignore::WalkBuilder`（经 `scan::walk_source_files`） | **不 mock**：真实遍历器；gitignore 感知与五剪枝是行为本体（spec 明写 MUST NOT 自建 `read_dir`） | 枚举 / 剪枝 / gitignore 行 |
| `foundation::layout::archi_models_dir` / `archi_config_path` | **不 mock**：真实派生（路径单点纪律；`tree.rs` 零文件名字面量的判据依赖它） | 全部用例 |
| 时钟 / 网络 / 子进程 | **零依赖**：本文件零 spawn、零网络、零时钟（排序不依赖时间） | —— |
| 源码自证（`include_str!`） | **不 mock**：读自身源码作判据（AC-6 的机械面，沿 `owner_test` 的常量单源先例） | 判定单源 / 只读零写盘两行 |

---

### `packages/desktop/src-tauri/crates/core/archi/src/scan/owner.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/scan/owner_test.rs`

**改写既有测试文件**（651 行，19 条 `strip_generated` 相关断言）——沿既有零 mock 定式（`ArchiElement` 直接构造、零 IO 纯字符串匹配）。改写口径：①根 `src/dir/` 下的探针 `src/dir.i.ts` 期望由 `Owner::None` **翻转为** `Owner::Element`；②`生成物剥离使单文件元素根同级生成物归元素` 的用例名 / doc / 断言意图随并集语义改写（原文「目录形态元素根的根同级生成物仍不命中」正是 bug 契约）；③`生成物剥离四种形态各归其元素` 保留四条目录内形态并**另加**四条根同级实形态正例；④白名单外后缀 4 条反例与常量单源 / 五扩展名用例**保持绿、零改动**；⑤新增并集定档用例（`src/dir/` 与 `src/dir.ts` 同场 → 探针归更具体的单文件根，钉住「禁止候选短路」）；⑥新增 `module_roots` / `contains_count` 两访问器的用例组。

#### 待测功能

- `OwnerIndex::owner_of(&self, rel_path: &str) -> Owner`: 签名不变；体内改为「候选集合并定档」——对每个候选各跑一遍最长前缀、按 `(fqn, 前缀长度)` 合并去重后按长度定档（同长仍 `Ambiguous`）；**MUST NOT 候选短路**
- `OwnerIndex::module_roots(&self) -> BTreeMap<&str, Vec<&str>>`: `pub(crate)` 新增——归一路径 → FQN 列表（每表项字典序去重）；`len == 1` 判 `elementRoot`、`len >= 2` 判 `ambiguous`；树侧唯一来源
- `OwnerIndex::contains_count(&self, path: &str) -> u32`: `pub(crate)` 新增——严格前缀 `"{path}/"` 的模块根元素数（按 FQN 去重、不含自身）；`path` 非归一形态时由 `normalize` 先行归一

<!-- design.md「公共函数 / API」表对本文件恰 3 行（上列）。strip_candidates / longest_prefix_hits / normalize / prefix_matches / OwnerIndex::build / Owner 三值 / GENERATED_MARKER / GENERATED_EXTENSIONS 全部零改动或私有，不入表。strip_candidates 的行为经 owner_of 公共面观测（owner_test 是 owner 的兄弟模块——由 scan/mod.rs 声明——私有件不可直调；MUST NOT 为测试放宽可见性）。module_roots / contains_count 是 pub(crate)，owner_test 同 crate 内可直调。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `OwnerIndex::owner_of` 根同级生成物归元素（AC-7 / D1 整组剥离候选） | 正向 | 元素 `App.Backend.Infra.ArchiRuntime`（`path = ["./crates/infra/archi/"]`）+ 探针 `crates/infra/archi.i.rs` → `Owner::Element { fqn: "…ArchiRuntime" }`（整组剥离得 `crates/infra/archi`，被目录前缀命中）；同形态另举三例（`views/architecture/` ← `views/architecture/architecture.i.tsx`、`src/lib/` ← `src/lib/lib.i.ts`、`core/x/` ← `core/x/x.i.rs`） | 参数化迭代（4 根同级实形态） |
| `OwnerIndex::owner_of` 目录内四形态保持（AC-7 既有回归） | 正向 | 既有四条目录内正例（`views/architecture/architecture.i.ts` / `views/welcome-view/welcome-view.i.ts` / `crates/core/archi/archi.i.rs` / `crates/core/archi/src/scan/scan.i.rs`）→ 各归其元素（替换扩展名候选与目录前缀候选均命中，不得因新增整组候选而改变） | 参数化迭代（4 既有正例） |
| `OwnerIndex::owner_of` `src/dir.i.ts` 由 `None` 翻 `Element`（AC-7 翻转臂） | 正向 | 元素 `App.Core.Dir`（`path = ["./src/dir/"]`）+ 探针 `src/dir.i.ts` → `Owner::Element`（整组剥离得 `src/dir` 命中前缀；修复前为 `None`）；该断言与改写后的用例名 / doc 一致（MUST NOT 保留旧期望） | 单次 |
| `OwnerIndex::owner_of` 并集定档：更具体的单文件根胜出（AC-7 / R-1 钉住「禁止候选短路」） | 异常 | 同场两元素：`App.Core.Dir`（`path = ["./src/dir/"]`）与 `App.Core.DirFile`（`path = ["./src/dir.ts"]`），探针 `src/dir.i.ts` → 归 `App.Core.DirFile`（替换扩展名候选 `src/dir.ts` 更长）。**反例臂**：若实现成「候选 1 命中就短路」，整组候选 `src/dir` 会先命中并给出 `App.Core.Dir` → 该断言必红 | 单次 |
| `OwnerIndex::owner_of` 并集合并后同长仍 `Ambiguous`（AC-7 / 既有语义保持） | 异常 | 两元素 `path` 同长且各被一条候选命中同一生成物 → `Owner::Ambiguous`，候选全量、字典序去重（合并两候选的命中集后不得退化为单值） | 单次 |
| `OwnerIndex::owner_of` 白名单外后缀零误剥（AC-7 / 既有 4 条反例保持绿） | 异常 | 文件级根 `src/my.mod.rs` 下探针 `src/my.mod.i.py` / `src/my.mod.d.rs` / `src/my.mod.i` / `src/my.mod.I.rs` → 全 `Owner::None`（零误剥）；对照：`src/my.mod.i.rs` → 归元素 | 参数化迭代（4 反例 + 1 对照） |
| `OwnerIndex::owner_of` 非生成物路径归属零变化（AC-7 / AC-17 回归） | 边界 | `views/architecture.tsx` / `views/welcome-view/index.ts` / `crates/core/archi/src/scan/rust.rs` 三条既有点位逐值相等；文件名含 `.i` 但非生成物（`a.i.txt` / `a.i`）→ 判据零变化 | 参数化迭代（5 形态） |
| `OwnerIndex::owner_of` 五扩展名逐值可剥（AC-7 / 既有常量单源用例保持） | 边界 | `.i.rs` / `.i.ts` / `.i.tsx` / `.i.js` / `.i.jsx` 各配等价元素根 → 五者全 `Element`（缺 `js` / `jsx` 会落 `None`）；`GENERATED_EXTENSIONS` 长度 5、`GENERATED_MARKER == "i"`、两常量各只定义一处 | 参数化迭代（5 扩展名） |
| `OwnerIndex::owner_of` 边界形态不 panic（边界） | 边界 | `rel_path = ""` / `".i.rs"`（空 stem）/ `"src/my.mod.i.rs/"`（尾 `/`）/ 超长（>1000 chars）/ 含 emoji 与 `\n` → 返回值落在封闭三值内且不 panic；反斜杠分隔与 `./` 前缀的生成物仍剥 | 参数化迭代（6 形态） |
| `OwnerIndex::module_roots` 归一路径 → FQN 列表（AC-5 / AC-6 / D2） | 正向 | 元素 A（`path = ["./src/auth/", "src/auth"]`，两写法归一同路径）+ 元素 B（`path = ["./src/billing/"]`）→ 键集合为 `{"src/auth", "src/billing"}`（**归一形态**：无 `./`、无尾 `/`）；`src/auth` 表项为 `["A"]`（同元素重复声明去重后恰 1 ⇒ 该节点判 `elementRoot` 而非 `ambiguous`） | 单次 |
| `OwnerIndex::module_roots` 同路径多元素声明 → 表项 ≥ 2（AC-5 / spec Scenario「同一归一路径被多元素声明」） | 异常 | 元素 A（`./src/shared/`）与元素 B（`src/shared`）→ 键 `src/shared` 的表项为 `["A", "B"]`（字典序）；树侧据此判 `ambiguous` | 单次 |
| `OwnerIndex::module_roots` 容器元素零表项（AC-5 / spec Scenario「容器元素零节点」） | 异常 | `path = []` 的元素 → `module_roots` 无该 FQN 任何表项、零键产出 | 单次 |
| `OwnerIndex::module_roots` 空路径条目过滤与空模型（边界） | 边界 | 元素 `path = [""]` / `path = ["./"]`（归一后空串）→ 不产表项（沿 `build` 既有 `prefix.is_empty()` 丢弃）；`elements = []` → 返回空 `BTreeMap`、不 panic | 参数化迭代（3 形态） |
| `OwnerIndex::contains_count` 严格前缀计数（AC-5 / D6③） | 正向 | 元素根 `src/`（自身）+ `src/a/` + `src/b/` + `src/a/deep/` → `contains_count("src")` = 3（不含自身）；`contains_count("src/a")` = 1（`src/a/deep`）；`contains_count("src/b")` = 0 | 参数化迭代（3 探针） |
| `OwnerIndex::contains_count` 按 FQN 去重（AC-5 / spec Scenario「多 path 元素与 contains 去重」） | 正向 | 元素 `App.Frontend.Shell` 声明四条文件形 `path` 全在 `src/shell/` 下 → `contains_count("src/shell")` = **1**（按 FQN 去重）而非 4；`contains_count("src")` = 1 | 单次 |
| `OwnerIndex::contains_count` 不计数非严格前缀（AC-5 / spec 判据③） | 异常 | 元素根 `src/ab/` → `contains_count("src/a")` = **0**（`src/ab` 不以 `src/a/` 开头——MUST NOT 用裸 `starts_with` 前缀）；`contains_count("src")` = 1 | 单次 |
| `OwnerIndex::contains_count` 非归一入参先归一（AC-5 / D2） | 边界 | 入参 `./src/` / `src\\` / `src/` 三写法 → 与 `contains_count("src")` 结果相等（`normalize` 先行）；入参 `""` / `"."` → 返回 0、不 panic | 参数化迭代（5 形态） |
| `OwnerIndex::contains_count` 上界与空模型（边界） | 边界 | 单元素根自身 → `contains_count(自身)` = 0；1000 个互不为前缀的元素根（`src/k0/` … `src/k999/`）→ `contains_count("src")` = 1000（`u32` 容量内不溢出）；`elements = []` → 任意 path 返回 0 | 参数化迭代（3 形态） |
| 两候选来源分离可达（AC-7 / D1） | 边界 | 仅声明 `src/archi/`（目录根）时 `src/archi.i.rs` 归元素（整组候选生效）；仅声明 `src/archi.rs`（文件根）时 `src/archi.i.rs` 归元素（替换候选生效）——两形态各证明一条候选路径独立可达 | 参数化迭代（2 形态） |
| 既有 `strip_generated` 断言的改写面（R-1 / R-7 回归登记） | 边界 | `生成物剥离四种形态各归其元素` / `生成物剥离只去末段标记` / `生成物边界形态不panic` / `生成物形态下同长多命中仍歧义` / `非生成物路径归属零变化` / `生成物常量单源与五扩展名齐备` 六条用例的语义断言逐条保留（只改翻转臂的期望与 `生成物剥离使单文件元素根同级生成物归元素` 的用例名 / doc）；`生成物白名单外后缀不剥离` 零改动 | 单次（回归登记） |
| 判据面单源（AC-6 / `tasks.md` 阶段一自查明令） | 异常 | `include_str!("owner.rs")` 自证：`strip_generated` **零命中**；`strip_candidates` 恰一处定义 + 恰一处调用；`fn module_roots` / `fn contains_count` / `fn longest_prefix_hits` 各恰一处；`fn normalize` / `fn prefix_matches` / `pub fn build` 三处签名与函数体逐字未变（源码串对拍） | 单次（源码自证） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 模型 / 图 | **不 mock**：`ArchiElement` 直接构造（纯值，不经解析——沿既有 `element()` 构造器） | 全部用例 |
| 文件系统 | **零依赖**：纯字符串前缀匹配，零 IO（`module_roots` / `contains_count` 同为内存表运算） | 全部用例 |
| 时钟 / 网络 / 子进程 | **零依赖** | —— |
| 源码自证（`include_str!("owner.rs")`） | **不 mock**：常量单源与「零 `strip_generated` 残留 / 候选不短路结构」的判据（沿既有 `生成物常量单源与五扩展名齐备` 先例） | 判据面单源行 |

---

### `packages/desktop/src-tauri/crates/core/archi/src/scan/mod.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/scan/mod_test.rs`

**改写既有测试文件**（902 行）——只**追加**可见性提权后的行为回归与单源自证；`scan()` 六段、`SOURCE_EXTENSIONS`、`ImportFact` / `DependencyEdge` / `ScanInput` / `ScanOutcome` 的既有断言**逐字零改**（`tasks.md` 阶段一自查明令）。沿既有零 mock 定式（真实 tempdir + 真实 `WalkBuilder` + 真实 `ExcludeSet` / `OwnerIndex` / `TsResolver` / `RustResolver`）。

#### 待测功能

- `walk_source_files(root: &Path) -> Vec<PathBuf>`: 私有 → `pub(crate)`；gitignore 感知 + 五剪枝语义零变化，供文件树复用
- `is_source_file(rel: &str) -> bool`: 私有 → `pub(crate)`；五扩展名判据零变化，供文件树复用
- `posix(path: &str) -> String`: 私有 → `pub(crate)`；`\` → `/` + 剥前导 `./` 零变化，供文件树复用（design 事实校准 1）

<!-- design.md「公共函数 / API」表对本文件恰 3 行（上列，均为「修改（可见性）」）;scan() / SOURCE_EXTENSIONS / inline_ignore / resolve 零改动或不入表。三函数是 pub(crate)，mod_test 是 scan 的子模块，可直调——这也是三条提权的可测前提（不提权则行为只能经 scan() 间接观测，而 scan() 的六段带排除与解析噪声）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `walk_source_files` 枚举面与五剪枝（AC-8 / 提权后行为回归） | 正向 | 真实 tempdir 造 `src/a.rs` / `src/b.ts` / `src/c.tsx` / `src/d.js` / `src/e.jsx` / `src/f.md`（非源）/ `target/x.rs` / `node_modules/y.js` / `dist/z.ts` / `build/w.ts` / `.git/v.ts` / `.hidden/u.ts` → 返回路径集合为前五条（绝对路径、`root` 前缀）；六类剪枝项零命中 | 参数化迭代（5 收 + 6 剪枝） |
| `walk_source_files` gitignore 感知（AC-8 / 提权后行为回归） | 异常 | `<root>/.gitignore` 写 `ignored/**` 且盘上有 `ignored/a.ts` / `src/b.ts` → 返回集合只含 `src/b.ts`（gitignore 命中项由遍历器跳过，MUST NOT 由本函数二次过滤） | 单次 |
| `walk_source_files` 路径形态与空目录（边界） | 边界 | `root` 为不存在路径 → 返回空 `Vec`、不 panic；`root` 为空目录 → 空 `Vec`；`root` 为相对路径 `"a/b"` → 返回路径以该相对根起头（无 `canonicalize`，沿既有语义） | 参数化迭代（3 形态） |
| `is_source_file` 五扩展名与非法值（AC-8 / 提权后行为回归） | 正向 | `a.rs` / `a.ts` / `a.tsx` / `a.js` / `a.jsx` → 全 `true`；`a.py` / `a.md` / `a.json` / `a`（无扩展名）/ `a.RS`（大写）→ 全 `false`；路径含目录前缀（`src/deep/a.rs`）→ `true` | 参数化迭代（5 收 + 5 拒 + 1 深路径） |
| `is_source_file` 边界字符串（边界） | 边界 | `""` / `"."` / `"a."`（空扩展名）/ `"a.rs.bak"`（末段非源）/ 超长（>1000 chars）/ 含 emoji 文件名 → 均按末段扩展名判、不 panic；`"a\n.rs"` → 不 panic | 参数化迭代（6 形态） |
| `is_source_file` 反向判据：判据只看末段扩展名（异常） | 异常 | `"rs"`（无点，裸词）/ `"src/a.rs/"`（尾 `/`，末段为空）/ `"a.rs\0"` / `".rs"`（空 stem，扩展名 `rs`）→ 前两条 `false`（末段非五扩展名）、后两条不得 panic 且结果与 `rsplit_once('.')` 语义一致（用例内钉死）；MUST NOT 因路径形态宽松而误收非源文件 | 参数化迭代（4 形态） |
| `posix` 归一形态（AC-8 / 事实校准 1 / 提权后行为回归） | 正向 | `"a\\b\\c"` → `"a/b/c"`；`"./a/b"` → `"a/b"`；`"././a/b"` → `"a/b"`（循环剥）；`"a/b"` → `"a/b"`（幂等） | 参数化迭代（4 形态） |
| `posix` 反向判据：不做过量归一（异常） | 异常 | `".hidden/a"` → 原样（`./` 只剥**前导**，`.hidden` 不是 `./`）；`"a/./b"` → `"a/./b"`（中间段不归一，归一职责只在首尾）；`"//a/b"` → `"//a/b"`（不折叠重复分隔符）；`"/abs/path"` → 原样（不剥前导 `/`）——四形态钉死「本函数只做两件事」 | 参数化迭代（4 形态） |
| `posix` 边界形态（边界） | 边界 | `""` → `""`；`"."` → `"."`（非 `./` 形态不剥）；`"a/b/"` → `"a/b/"`（**不**去尾 `/`——尾 `/` 剥离归 `OwnerIndex::normalize`，两函数职责不同）；超长与含 emoji → 不 panic | 参数化迭代（5 形态） |
| 三 helper 提权后的单源零复制（AC-6 / 事实校准 1） | 异常 | `include_str!("mod.rs")` 自证三处签名均为 `pub(crate) fn`；`read_dir` 在 `walk_source_files` 之外零出现（零第二份遍历）；`fn is_source_file` 恰一处定义 | 单次（源码自证） |
| `scan()` 六段与既有语义零变化（AC-17 / R-7 回归登记） | 边界 | 既有用例（`scannedFiles` / `changedFiles` 计数不变量、取消、`inline_ignore` 行号与理由边界、排除 / 归属 / 抽取 / 聚合）**逐条保持绿、零改动**——本行只作回归登记，不新增断言 | 单次（回归登记） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 文件系统 | **不 mock**：真实 tempdir + 真实落盘 + 真实 `.gitignore`（gitignore 语义是行为本体） | 全部用例 |
| `ignore::WalkBuilder` | **不 mock**：真实遍历器（五剪枝与 gitignore 感知即被测语义） | `walk_source_files` 全部行 |
| `ExcludeSet` / `OwnerIndex` / 两套解析器 | **不 mock**：真实组合（沿既有 `scan/mod_test.rs` 定式；本文件新增行只测 helper，不下探编排） | 回归登记行 |
| 时钟 / 网络 / 子进程 | **零依赖** | —— |

---

### `packages/desktop/src-tauri/crates/core/archi/src/dto.rs` -> `packages/desktop/src-tauri/crates/core/archi/src/dto_test.rs`

**改写既有测试文件**（1644 行）——只**追加**四新类型的形状 / 词表 / 线形断言；既有九组封闭词汇与全部出线 DTO 的字段名 / 顺序 / 语义断言**零改**（`tasks.md` 阶段二自查明令 `NodeMark` 命中面逐字一致）。沿既有零 mock 定式（真实 `serde_json` 序列化 / 反序列化）。

#### 待测功能

<!-- design.md「公共函数 / API」表对本文件零行：FileEntryKind / FileTreeMark / ArchiFileEntry / ArchiFileTree 列在「类型定义」表（D4），不是「公共函数 / API」行。四处新增类型的形状与线形覆盖并入本节的用例表（词表的 ALL / as_str 是类型关联常量与方法，属「类型定义」表的覆盖面，故以用例表承载、不虚构 API 条目）。既有 graph_view / module_detail / module_overview / search / change_summary / linked_doc_paths 六投影入口与本变更零交集，不入表。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `FileEntryKind::ALL` / `as_str` 二值封闭（AC-4 / spec Scenario「ALL 与 as_str 逐值对拍」） | 正向 | `ALL.len() == 2`；`Dir` → `"dir"`、`File` → `"file"`；`serde_json::to_value` 与 `as_str` 逐值一致；`from_value` 往返逐值相等；`ALL` 去重后长度不变 | 参数化迭代（2 值） |
| `FileTreeMark::ALL` / `as_str` 五值封闭（AC-4 / spec Scenario「ALL 与 as_str 逐值对拍」） | 正向 | `ALL.len() == 5`；`ElementRoot` → `"elementRoot"`、`Ambiguous` → `"ambiguous"`、`Member` → `"member"`、`ContainsElements` → `"containsElements"`、`None` → `"none"`；serde 线形与 `as_str` 逐值一致；往返恒等；`ALL` 去重后长度不变、序与 `as_str` 分支序一致 | 参数化迭代（5 值） |
| `FileEntryKind` / `FileTreeMark` 词汇外反序列化（异常 / Enum N+1） | 异常 | `FileEntryKind`：`"Dir"` / `"DIR"` / `"file "` / `""` / `"directory"` → 全 `Err`（`rename_all = "lowercase"` 严格）；`FileTreeMark`：`"ElementRoot"` / `"elementroot"` / `"none "` / `""` / `"unfulfilled"`（`NodeMark` 的词）/ `"crossKind"` → 全 `Err`（`rename_all = "camelCase"` 严格，零宽松归一） | 参数化迭代（5 + 6 形态） |
| `ArchiFileEntry` 七字段声明序出线（AC-3 / spec Scenario「条目七字段按声明序出线」） | 正向 | 全字段条目序列化 → JSON 键序列恰为 `["path", "name", "kind", "mark", "owners", "contains", "exists"]`（camelCase + 声明序；`serde_json` 默认保序即 spec 的「字段序即声明序」判据）；反序列化往返逐字段相等 | 单次 |
| `ArchiFileEntry` 字段值边界（边界 / 数值与容器映射） | 边界 | `path` 取 `""`（越签名脏值，序列化不 panic）/ 超长 >1000 chars / 含 `\n\0` emoji；`name` 同；`owners` 取 `[]` / 单元素 / 超大列表（1000 条）/ 非字典序输入（**序列化不重排**——投影侧已排序，DTO 零加工）；`contains` 取 `0` / `u32::MAX`；`exists` 取 `true` / `false`；`kind` / `mark` 取每枚举值——往返后逐字段相等 | 组合迭代（`owners` 4 × `contains` 2 × `exists` 2 × `kind` 2 × `mark` 5 抽样） |
| `ArchiFileTree` 单字段扁平（AC-3） | 正向 | `ArchiFileTree { entries: [] }` → 序列化文本恰 `{"entries":[]}`（**单键**，零额外字段）；`entries` 含三条 → 扁平数组（**无嵌套 `children` 键**）、顺序即入参顺序（DTO 零重排）；往返相等 | 参数化迭代（2 形态） |
| 四新类型与 `specta::Type` 的出线形状（AC-3 / AC-14） | 正向 | `specta` 类型名与字段名与 serde 一致（`ArchiFileTree` / `ArchiFileEntry` / `FileEntryKind` / `FileTreeMark`）；`FileTreeMark` 五线值的 `export type` 段由 `src/bindings/mod_test.rs` 侧断言（本行只作形状侧的等价登记，不读产物） | 单次 |
| 既有九组封闭词汇与全部出线 DTO 零变化（AC-17 / 类型单点纪律） | 边界 | `NodeMark` / `EdgeMark` / `ElementKind` / `GateTier` / `DiagnosticKind` / `WarningKind` / `DocKind` / `DocSource` / `SearchScope` 的词集与词序逐字不变；`ArchiGraph` / `ArchiElement` / `ArchiRelation` / `ArchiDiagnostic` / `ArchiModuleDetail` / `ArchiChangeSummary` / `ArchiSearchResult` 的键序列逐字不变（新增类型为**并列追加**，不入既有 DTO）；`dto.rs` 内 `NodeMark` 的出现面与改动前一致（零交叉引用 `FileTreeMark`） | 单次（回归登记 + 反向判据） |
| `FileTreeMark` 不进四面词汇对拍（AC-4 / spec「不进四面词汇对拍」） | 异常 | `tests/vocabulary_test.rs` **零 diff**（本变更不动该文件）；反向判据：四面集合不含 `FileTreeMark` / `FileEntryKind` 任何词——若把新词表并入四面，四面词汇集的大小与内容断言必红 | 单次（零 diff 登记见「不可测试项」） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| serde / serde_json | **不 mock**：真实序列化 / 反序列化（线形与声明序即被测对象） | 全部用例 |
| `specta::Type` | **不 mock**：derive 是编译期事实；产物面（`bindings.ts`）由 `src/bindings/mod_test.rs` 章节断言 | 出线形状行 |
| `tests/vocabulary_test.rs` | **零依赖**：不读该文件，判据是「零 diff + 反向集合不含」（文本比对归 `git diff`） | 四面零改动行 |
| 时钟 / 网络 / 子进程 | **零依赖** | —— |

---

### `packages/desktop/src-tauri/src/commands/archi/mod.rs` -> `packages/desktop/src-tauri/src/commands/archi/mod_test.rs`

**改写既有测试文件**（1279 行）——追加 `archi_file_tree` 与 `_inner` 的逐字段一致、root 校验扩到六命令、只读零写盘（扩到新命令）、结构诊断非空 → `Err`、空模型 → `Ok`；把既有 `四命令root校验四形态一律err` 扩到六命令并把用例名里的「四命令」改为符实（**不新增第二条同义用例**）。装置沿既有定式：`tauri::test::mock_app()` + 真实 `WorkspaceStores`（tempdir 数据根）托管进 app，无 State 的命令直调原签名。

#### 待测功能

- `archi_file_tree(root: String) -> Result<ArchiFileTree, String>`: `#[tauri::command] #[specta::specta]`；薄包装 = `archi_file_tree_inner(Path::new(&root))`；只读、零写盘、零 spawn
- `archi_file_tree_inner(root: &Path) -> Result<ArchiFileTree, String>`: `check_root` → `archi::tree::file_tree`；命令层零领域规则

<!-- design.md「公共函数 / API」表对本文件恰 2 行（上列）；既有五命令与五个 *_inner 签名 / 语义零变化。check_root / write_atomic 是既有私有件，不入表。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `archi_file_tree` ↔ `archi_file_tree_inner` 逐字段一致（AC-1 / AC-3） | 正向 | 健康模型 + 若干源文件的 tempdir → 包装与 `_inner` 各自 `Ok`，两者 `serde_json::to_value` 逐字段相等（命令层零加工，沿既有 `三命令与inner结果逐字段一致` 形态） | 单次 |
| root 校验四形态一律 `Err`（AC-1 / spec「空白 / 缺失 / 非目录 → Err」） | 异常 | `root` 为 `""` / `"   "` / 指向文件 / 不存在目录 → `archi_file_tree` 与 `archi_file_tree_inner` 均 `Err`，文案含 `root 无效` 或 `root 不是目录`（既有 `check_root` 口径）；在既有 `四命令root校验四形态一律err` 的**同一用例**内扩充两类调用，用例名改符实为「六命令」 | 参数化迭代（4 root 形态 × 2 函数） |
| `archi_file_tree` 只读零写盘（AC-1 / spec Scenario「只读零写盘」） | 边界 | 调用前后 `snapshot(root)`（递归 `(PathBuf, Vec<u8>)`）相等；`<root>/.dev-team/architecture/models` 的 mtime 不变；零临时文件残留（`write_atomic` 的 `.tmp` 形态零出现）；该命令并入既有 `三条读命令零写盘` 的快照段（用例名随之改符实） | 单次 |
| `archi_file_tree` 反向判据：与唯一写命令的边界（AC-1 / spec「唯一写命令仍是 archi_set_gate」） | 异常 | `archi_file_tree` 不接收 `gate` 参数、不触 `edit::plan_set_gate`（`include_str!("mod.rs")` 自证新函数体内零 `plan_set_gate` / 零 `write_atomic` 调用）；对照臂：同盘调 `archi_set_gate` 后模型字节变化，而 `archi_file_tree` 调用后保持不变 | 单次 |
| `archi_file_tree` 结构诊断非空即 `Err`（AC-2 / spec Scenario「结构诊断非空即 Err」） | 异常 | 模型含一处环 → `Err`，文案含「模型结构错」与诊断条数；MUST NOT 返回部分树；对照臂：同盘换健康模型 → `Ok` | 参数化迭代（2 形态对拍） |
| `archi_file_tree` 空模型返回 `Ok`（AC-2 / spec Scenario「空模型返回全 none」） | 边界 | ①模型目录缺席 ②`models/` 存在零 `*.toml` → 均 `Ok`；全部条目 `mark = "none"` / `owners` 空；MUST NOT `Err` | 参数化迭代（2 形态） |
| `archi_file_tree` 配置非法 → `Err` 且文案沿用既有显式文案（AC-9 / AC-8） | 异常 | `<root>/.dev-team/architecture/config.json` 写 `{ "exclude": ["src/legacy"] }`（无通配符）→ `Err`，文案含非法条目与 `/**` 形态提示（与 `archi_drift配置非法Err` 的文案同源）；对照：`{ "exclude": ["openspec/**"] }` → `Ok` | 参数化迭代（2 形态对拍） |
| `archi_file_tree` 与 `archi_graph` 同根同模型的一致性（AC-2 / AC-8 / R-6） | 正向 | 同一 tempdir 上分别调两命令 → 前者 `diagnostics` 空 ⇔ 后者 `Ok`；前者的 `elements[].path` 非空元素 FQN 集合是后者 `owners` 并集的子集（同一模型两投影，判据不交叉） | 单次 |
| 既有五命令与五个 `*_inner` 零变化（AC-17 / R-7 回归登记） | 边界 | `archi_graph` / `archi_module` / `archi_change_summary` / `archi_set_gate` / `archi_drift` 的签名 / 出参 / 错误映射逐字不变；`archi_set_gate` 仍是唯一写命令（`fs::write` / `fs::rename` 只出现在 `write_atomic`）；既有 20+ 条用例**保持绿、零改动** | 单次（回归登记） |
| 命令层零领域规则（AC-6 / D11） | 异常 | `include_str!("mod.rs")` 自证：`archi_file_tree_inner` 体内零归属判定 / 零目录合成 / 零标记推导（零 `OwnerIndex` / 零 `starts_with` / 零 `FileTreeMark` 构造）；文件头与 `:19` 注释口径为「六命令」（零残留「五命令」/「四命令」）；`use` 区含 `ArchiFileTree` | 单次（源码自证） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| Tauri 运行时 | **进程边界装置，非仓内模块 mock**：`tauri::test::mock_app()`（沿既有装置；`archi_file_tree` 无 State，直调原签名即可，装置只为同文件其余用例的 `WorkspaceStores`） | 全部用例 |
| `archi::tree::file_tree` / `archi::dto::graph_view` / `drift::config::load_config` | **不 mock**：真实调用（薄包装语义即「零加工」；替身会让逐字段一致断言自证） | 全部用例 |
| 文件系统 / store | **不 mock**：真实 tempdir 落模型 / 源文件 / 配置；真实 `WorkspaceStores`（数据根指向 tempdir）；零写盘判据用递归快照 | 全部用例 |
| 时钟 / 网络 / 子进程 | **零依赖**：本层零 spawn（`archi_file_tree` 不触 vcs / 不触 shell） | —— |

---

### `packages/desktop/src-tauri/src/commands/mod.rs` -> `packages/desktop/src-tauri/src/bindings/mod_test.rs`

`commands/mod.rs` 无独立 sidecar（只有 `all_commands!` 宏体与 `TEST_PATH_LOCK`），登记面断言的唯一可测落点是 `bindings/mod_test.rs` 的清单守卫（AC-1 / AC-14 的验证方法点名该文件）。本节承载**登记顺序**与**清单随动**两侧。

#### 待测功能

<!-- design.md「公共函数 / API」表对本文件零行（all_commands! 追加一行是宏体改动，不是 API 行）。条目改列 design「修改文件」表 commands/mod.rs 行与「清单守卫随动」表八行；断言对象是 bindings/mod_test.rs 的五个清单常量与三组计数断言。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `COMMAND_NAMES` / `COMMAND_WRAPPERS` 随动（AC-1 / AC-14） | 正向 | 两清单各含 `"archi_file_tree"` / `"archiFileTree"`，且插在 `"archi_drift"` / `"archiDrift"` 之后、`"read_file"` / `"readFile"` 之前；`read_file` / `readFile` 仍为**尾条**（尾锚零位移） | 单次 |
| `COMMAND_NAMES.len()` / `COMMAND_WRAPPERS.len()` 计数随动（AC-14 / R-8） | 正向 | 三处 `COMMAND_NAMES.len()` 断言与三处 `COMMAND_WRAPPERS.len()` 断言同值更新为 **49**（六处；断言**语义不变**，只随清单长度）；用例名 `登记面_总数四十八且尾锚零位移` 改「四十九」（只改名字） | 参数化迭代（6 处计数） |
| `DTO_TYPES` 随动与字典序插入（AC-3 / AC-14） | 正向 | 含 `ArchiFileEntry` / `ArchiFileTree`（插在 `ArchiExcludedFile` 与 `ArchiGateChange` 之间）、`FileEntryKind` / `FileTreeMark`（插在 `EdgeMark` 与 `GateTier` 之间）；首锚 `ActivePhase` / 尾锚 `WarningKind` 不动；`.len()` 断言 97 → **101** | 单次 |
| `ARCHI_COMMANDS` 族清单 4 → 6（AC-1 / AC-14 / R-8 的非计数随动） | 正向 | 数组类型为 `[(&str, &str); 6]`，序为 `archi_graph` / `archi_module` / `archi_change_summary` / `archi_set_gate` / `archi_drift` / **`archi_file_tree`**（既有漏登 `archi_drift` 一并补齐）；doc 由「架构域四命令」改「架构域命令族」 | 单次 |
| `ARCHI_DTO_TYPES` 族清单 19 → 23（AC-3 / AC-14） | 正向 | 数组类型 `[&str; 23]`；`ArchiFileEntry` / `ArchiFileTree` 插在 `ArchiDependsOn` 与 `ArchiDiagnostic` 之间、`FileEntryKind` / `FileTreeMark` 插在 `EdgeMark` 与 `GateTier` 之间；逐名命中产物 `export type <Name> =` | 参数化迭代（23 名） |
| 尾部追加序期望 vec 随动（AC-14 / R-8 易漏项） | 正向 | 既有期望 vec 增一条 `"archi_file_tree"`（位于 `"archi_drift"` 之后、`"read_file"` 之前）；整条 `assert_eq!` 逐字相等 | 单次 |
| `清单遗漏即产物覆盖断言必红` 对照臂改判（AC-14 / design 待决问题 5） | 异常 | 族末条由 `archi_set_gate` 变 `archi_file_tree` → `copy.pop()` 取到 `archi_file_tree`；对照臂「原清单条目在产物内」与反向臂「副本无缺失」「副本确已删去该条」三条**断言体零改**即自动改判为绿；用例名 `archi四命令在册且既有条目零位移` → `archi命令族在册且既有条目零位移` | 单次 |
| `archiFileTree` 的 args schema 形状与导出类型名逐名命中（AC-3 / AC-14 / spec Module Contract 生成物行） | 正向 | `bindings.zod.ts` 含 `archiFileTree` 的 `argsSchemas` 表项（`root: z.string()` 形态）与 `resultSchemas` 表项（`ArchiFileTree`）；两份产物各含四个新类型的具名导出一处；签名为 `(root: string) => __TAURI_INVOKE<ArchiFileTree>("archi_file_tree", { root })`（沿既有产物形状） | 参数化迭代（4 类型 + 1 命令） |
| 既有清单条目零位移（AC-14 / 尾锚判据） | 边界 | `app_environment` 之前的全部条目与改动前逐字同序；`read_file` 仍为 `COMMAND_NAMES` 尾条、`readFile` 仍为 `COMMAND_WRAPPERS` 尾条（新增项只插在 archi 段尾部） | 单次 |
| 重导出幂等（AC-14 / 既有零 diff 门） | 边界 | 连续两次 `export_bindings()` 产物逐字节一致（既有 `archi章节重导出幂等零diff` 用例**零改动**）；本行只作回归登记 | 单次（回归登记） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 绑定的真实导出面 | **不 mock**：`authoritative_snapshot()` 真实调用 `export_bindings()` 产出两份产物（清单守卫的本体是「清单 ↔ 产物强耦合」）；变更类用例以 `RestoreOnDrop` 兜底恢复，串行靠既有 `file_lock()` | 全部用例 |
| `all_commands!` 宏 | **不 mock**：清单常量是手写副本，与宏体的对拍即被测语义（宏体不反射，故以「产物含命令名」间接见证） | 全部用例 |
| 时钟 / 网络 / 子进程 | **零依赖**（`export_bindings` 是进程内调用，零 spawn） | —— |

---

### `packages/desktop/src/views/architecture/hooks/use-archi-file-tree.ts` -> `packages/desktop/src/views/architecture/hooks/use-archi-file-tree.test.ts`

**新建测试文件**（纯 hook 套件）——沿 `use-archi-graph.test.ts` / `use-archi-drift.test.ts` 定式：`renderHook` + `rerender` + `act` + `waitFor`；进程边界 mock `@tauri-apps/api/core` 的 `invoke`（生成的 `commands.archiFileTree` 最终经此调用），按命令名分发 fixture 并记录调用序列与载荷；`beforeEach` 重置 `invokeMock` 并按命令名装配默认实现。

#### 待测功能

- `useArchiFileTree(root: string | null): ArchiFileTreeState`: `commands.archiFileTree(root)`；三触发（挂载 / `root` 变化 / 上层 `key` 重挂，**无 `refresh` 成员**）；失败与换 root 都清空 `tree`；`root === null` 零 invoke；返回 `{ tree, loading, error }`

<!-- design.md「公共函数 / API」表对本文件恰 1 行（上列）；ArchiFileTreeState 在「类型定义」表，形状覆盖并入返回值的三成员断言。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `useArchiFileTree` 挂载触发（AC-13 / spec Scenario「三触发与零轮询」） | 正向 | `renderHook(() => useArchiFileTree('C:\\demo\\alpha'))` → 恰一次 `archi_file_tree` invoke，载荷 `{ root: 'C:\\demo\\alpha' }`；落定后 `tree` 等于 fixture、`loading === false`、`error === null` | 单次 |
| `useArchiFileTree` `root` 变化触发（AC-13） | 正向 | `rerender({ root: 'B' })` → 共两次 invoke、末次载荷 `{ root: 'B' }`；落定后 `tree` 为 B 的 fixture | 单次 |
| `useArchiFileTree` 上层 `key` 重挂触发（AC-13） | 正向 | 卸载后重新 `renderHook`（同一 `root`）→ 再次 invoke（`key` 重挂即新实例，零缓存）；两次载荷相同 | 单次 |
| `useArchiFileTree` `root = null` 零 invoke（AC-13） | 边界 | `renderHook(() => useArchiFileTree(null))` → invoke 零调用；`tree === null` / `loading === false` / `error === null`（三者复位）；随后 `rerender({ root: 'A' })` → 一次 invoke | 单次 |
| `useArchiFileTree` 无 `refresh` 成员（AC-13 / D12） | 异常 | 返回值的键集合恰为 `['error', 'loading', 'tree']`（`Object.keys(...).sort()`，沿 `agent-adapter.test.ts` 先例）；`result.current.refresh` 为 `undefined`——反向判据：若补 `refresh` 成员，该断言必红 | 单次 |
| `useArchiFileTree` 失败清空旧树（AC-12 / spec Scenario「换 root 不与旧树混淆」） | 异常 | root A 成功落定 → `rerender({ root: 'B' })` 且 B 的 invoke reject(`new Error('root 无效')`) → `tree === null`（**旧 A 的树 MUST NOT 冒充**）、`error` 含 `root 无效`、`loading === false` | 单次 |
| `useArchiFileTree` 失败后成功清空 error（AC-12） | 正向 | 首次 reject → `error` 非 `null`；随后重挂（`key` 语义）并 resolve → `tree` 非 `null`、`error === null`（等待信号取「成功后才有」的 `tree`，避开 effect 起始同步清 `error` 的竞态——沿 `use-archi-graph.test.ts` 的注释口径） | 单次 |
| `useArchiFileTree` 新 root 未回时旧树不冒充（AC-12 / D12 的 `loadedRoot` 判据） | 边界 | root A 已落定；`rerender({ root: 'B' })` 且 B 挂起未 resolve → `tree === null`（`loadedRoot !== root`）；B 落定后 `tree` 为 B 的 fixture | 单次 |
| `useArchiFileTree` 竞态：旧 root 迟到应答不回写（AC-12 / R-6） | 异常 | 两次 invoke 挂起（A 与 B）；先 resolve B（落定）→ 再 resolve A（迟到）→ `tree` 仍为 B 的 fixture（`cancelled` 旗标拦旧应答） | 单次 |
| `useArchiFileTree` 卸载后不写状态（AC-12） | 边界 | `unmount()` 后 resolve 挂起的应答 → 零 React 警告（无「state update on unmounted」）、invoke 恰一次 | 单次 |
| `useArchiFileTree` 零轮询零 watch（AC-13 / spec Scenario「三触发与零轮询」） | 异常 | `vi.useFakeTimers()` 下 `advanceTimersByTime(60_000)` → invoke 计数不变；源码 `grep` 零 `setInterval` / `setTimeout` / `watchSubscribe` 归「不可测试项」的静态检索 | 单次 |
| `useArchiFileTree` 空树与加载中三态（AC-12 / spec Scenario「空树与加载中」） | 边界 | ①响应 `{ entries: [] }` → `tree.entries` 为 `[]`（**不是 `null`**——空树是成功态，与未载入可辨）②挂起未回 → `tree === null` 且 `loading === true`（两者组合即「解析中…」的派生前提） | 参数化迭代（2 形态） |
| `useArchiFileTree` fixture 规模与载荷边界（边界 / list 映射） | 边界 | 响应含超大条目列表（1000 条）→ `tree.entries.length === 1000`、不抛错、零裁剪（hook 是纯透传）；`root = ''`（空串越签名脏值）→ 仍 invoke 一次且载荷 `{ root: '' }`（是否 `Err` 由后端裁，hook 零前置校验） | 参数化迭代（2 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core` 的 `invoke` | **进程边界 mock**：`vi.hoisted` + `vi.mock` 替身，按命令名 `archi_file_tree` 分发 fixture；`beforeEach` 重置并装配默认实现（沿既有 `use-archi-graph.test.ts` 同款） | 全部用例 |
| `commands.archiFileTree`（生成绑定） | **不 mock**：真实生成包装经替身 `invoke` 触达——断言载荷形状即验证「生成包装到 invoke 的映射」 | 全部用例 |
| React 调度（`renderHook` / `act` / `waitFor`） | **不 mock**：真实 React 19 渲染与调度（hooks 生命周期即被测对象） | 全部用例 |
| 时钟 | **只 `vi.useFakeTimers()` 做零轮询臂**，`finally` 复原（沿既有用例）；其余用例用真时钟 | 零轮询行 |
| 文件系统 / 网络 / 子进程 | **零依赖** | —— |

---

### `packages/desktop/src/views/architecture/components/file-tree.tsx` -> `packages/desktop/src/views/architecture/components/file-tree.test.tsx`

**新建测试文件**（渲染面套件，`environment: 'jsdom'`）——沿 `element-tree.test.tsx` / `dir-tree.test.tsx` 定式：纯 props 驱动（展开态为组件内部 state）+ `render` / `fireEvent` / `screen` / `queryAllByTestId` 断言真实 DOM，零 mock。查询锚点一律 `data-testid`（`archi-file-tree` / `archi-file-node` / `archi-file-mark` / `archi-file-tree-empty` / `archi-file-tree-focus-miss` / `archi-file-tree-legend` / `archi-tree-mode`）与 `data-mark` / `data-path` / `data-mode`。fixture 构造器 `entry(path, overrides: Partial<ArchiFileEntry>)` 收全七字段。

#### 待测功能

- `FileTree(props: FileTreeProps) -> React.JSX.Element`: Props 恰三参 `{ entries, focused, onFocus }`；扁平 → 森林（目录先于文件、各自 `name` 序）→ 递归行；per-path 翻转展开（默认前 2 段）；折叠不渲子树；五值标记 + tooltip；`elementRoot` / `member` 行点击 → `onFocus(owners[0])`，其余三值只切展开；`focused` 命中条目全量高亮 + 强制展开祖先链 + 滚到首条；零命中顶部弱提示；底部可折叠图例
- `TreeModeSwitch(props: TreeModeSwitchProps) -> React.JSX.Element`: 分段控件 `data-testid="archi-tree-mode"` + 两钮 `data-mode="file"` / `data-mode="element"`（`aria-pressed` 表达选中态）

<!-- design.md「公共函数 / API」表对本文件恰 2 行（上列）；FileTreeProps / TreeModeSwitchProps / TreeMode / FileTreeNode 在「类型定义」表（FileTreeNode 文件内不导出），形状覆盖并入两函数的入参断言。buildForest / FileRow / TreeNodeList / isExpanded 私有件不入表。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `FileTree` 目录先于文件、各自 `name` 序（AC-11 / spec「渲染顺序」） | 正向 | `entries` 乱序传入（后端只给 `path` 字节序）→ 渲染序为同层目录先于文件、同 kind 按 `name` 的 `localeCompare` 序；断言 `queryAllByTestId('archi-file-node')` 的 `data-path` 序列 | 单次 |
| `FileTree` 缺省展开前 2 段（AC-11 / spec「展开态」） | 正向 | 三层路径 `a/b/c/d.ts` → 挂载后可见 `a` / `a/b`（段数 ≤ 2 默认展开），`a/b/c` 与文件行**不在 DOM**（折叠不渲染子树）；点 `a/b` 行的 chevron → `a/b/c` 出现 | 单次 |
| `FileTree` 展开不触发取数（AC-11 / AC-13） | 正向 | 点 chevron 展开 / 收起若干次 → `onFocus` 零调用（展开是纯本地 state）；`flipped` 集合按 `path` 键稳定 | 单次 |
| `FileTree` 五种 mark 各渲对应 `data-mark` 与 tooltip（AC-4 / AC-11 / spec「标记呈现」表） | 正向 | 五条 fixture（`elementRoot` / `ambiguous` / `member` / `containsElements` / `none`）→ `archi-file-mark` 元素的 `data-mark` 逐值命中；tooltip 逐值：`模块 App.A` / `多归属：App.A、App.B` / `属于 App.A` / `含 2 个模块`（尾随小字 `·2`）；`none` 行**零** `archi-file-mark` 节点 | 参数化迭代（5 mark） |
| `FileTree` `elementRoot` / `member` 行点击 → 一次 `onFocus(owners[0])`（AC-11 / spec Scenario「点模块行圈定范围」） | 正向 | 点 `elementRoot` 行（`owners = ["App.Core.Archi"]`）→ `onFocus` 恰一次、实参 `App.Core.Archi`；点 `member` 行（同 `owners`）→ 同上；两次点击各自独立一次（无去重缓存） | 参数化迭代（2 mark） |
| `FileTree` 非归属三值只切展开、零 `onFocus`（AC-11 / spec Scenario「非归属行只切展开」/ D9） | 异常 | 点 `ambiguous`（`owners = ["A","B"]`）/ `containsElements` / `none` 行 → `onFocus` **零调用**；若该行有子目录则展开态翻转（子节点 DOM 出现 / 消失）；MUST NOT 出现候选弹层（DOM 中零新增 `dialog` / `popover` 角色节点） | 参数化迭代（3 mark） |
| `FileTree` 多 `path` 元素焦点多行同时高亮（AC-11 / spec Scenario「聚焦多 path 元素多行高亮」） | 正向 | `entries` 含四条 `owners = ["App.Frontend.Shell"]` 的 `elementRoot` 行，`focused = "App.Frontend.Shell"` → 四行同时高亮（高亮态以行元素类名断言）、标记圆点保留；`onFocus` 零调用（聚焦是入参不是事件） | 单次 |
| `FileTree` 焦点命中时强制展开祖先链（AC-11 / D15） | 正向 | 深层条目 `a/b/c/d.ts`（`owners = ["App.Deep"]`）且 `focused = "App.Deep"`（默认只展开前 2 段 → 该行本不可见）→ 挂载后该行出现在 DOM（`forcedOpen` 派生集合强制展开祖先链，**零 setState**） | 单次 |
| `FileTree` 焦点命中的滚到首条（AC-11 / D15） | 正向 | 实例级给命中行装 `scrollIntoView = vi.fn()` 桩（`afterEach` 复原）→ 挂载后**首条**命中行被调用一次（`{ block: 'nearest' }`）；非命中行零调用 | 单次 |
| `FileTree` 零命中顶部弱提示且不滚动（AC-11 / spec Scenario「容器元素聚焦呈提示不滚动」/ D14） | 异常 | `focused = "App.Container"`（`entries` 中零条 `owners` 含该 FQN）→ `archi-file-tree-focus-miss` 在场，文案逐字含「该元素在树上无可定位节点（容器元素不持有文件，或其源文件均在判定面之外）」；`scrollIntoView` 桩零调用；MUST NOT 断言「该元素是容器」（文案并列两成因） | 单次 |
| `FileTree` 换焦点后的提示与高亮切换（AC-11 / 生命周期边界） | 边界 | `focused` 由零命中 FQN 改为命中 FQN（`rerender`）→ 提示节点消失、命中行高亮；再改为 `null` → 提示消失、零高亮、`onFocus` 与 `scrollIntoView` 全程零调用 | 单次 |
| `FileTree` 折叠不渲子树且展开态按 `path` 保持（AC-11 / spec Scenario「折叠不渲染子树且展开态按 path 保持」） | 边界 | 展开深层目录 → `rerender` 同一批 `entries`（模拟刷新重挂的同批 `path`）→ 该目录仍展开（per-path 翻转集合保持）；折叠态子树的 DOM **零节点**（`queryAllByTestId` 计数为 0，不是 `display:none`） | 单次 |
| `FileTree` 空树态（AC-12 / spec Scenario「空树与加载中」） | 边界 | `entries = []` → `archi-file-tree-empty` 在场、文案 `仓库内无源文件`；`archi-file-node` 零节点；不抛错 | 单次 |
| `FileTree` 图例常驻与可折叠（AC-8 呈现侧 / spec「图例」） | 正向 | 底部 `archi-file-tree-legend` 常驻；折叠态只渲一行入口、展开后文案逐字含「仅显示判定面源文件（测试文件与架构配置 exclude 命中项不在树中；none 行 = 无元素认领）」；图例文本不影响树行渲染 | 单次 |
| `FileTree` D5′ 呈现侧：前端不做二次过滤（AC-8 / R-2） | 异常 | 注入含 `src/a.test.tsx` / `src/b.rs_test` 形态的 `entries`（越过后端过滤的脏输入）→ 前端**不做二次过滤**（行照常渲出、`data-path` 命中），反向钉住「过滤面单点在后端」 | 单次 |
| `FileTree` `entries` 越签名脏值（边界 / list 映射） | 边界 | `entries = undefined`（`as` 断言越签名）→ 抛错或渲 `archi-file-tree-empty` 二者择一，MUST NOT 渲出非空树（不虚构条目）；用例内钉死一种行为 | 单次 |
| `FileTree` 超大条目列表（边界） | 边界 | 1000 条扁平 `entries` → 渲染不抛错、展开态与标记逻辑照常（默认展开前 2 段使可见行数有界）；断言行集合非空且 `data-path` 唯一 | 单次 |
| `FileTree` `path` / `name` 字符串边界（边界 / str 映射） | 边界 | `path = ""`（脏值）→ 不抛错；超长 `path`（>1000 chars）/ 含 emoji 与空格的 `path` → 行渲出、`data-path` 逐字保留；单段 `path`（无 `/`）→ 无 chevron（无子目录） | 参数化迭代（4 形态） |
| `TreeModeSwitch` 两钮与 `onChange`（AC-11 / spec「缺省文件树」） | 正向 | `mode = 'file'` → 容器 `archi-tree-mode` 在场，`data-mode="file"` 钮 `aria-pressed = true`、`data-mode="element"` 钮 `false`；点后者 → `onChange('element')` 恰一次；点前者 → `onChange('file')` | 参数化迭代（2 钮） |
| `TreeModeSwitch` 选中态随 `mode` 变化（AC-11） | 正向 | `rerender({ mode: 'element' })` → 两钮 `aria-pressed` 翻转；`onChange` 零调用（受控组件零内部 state） | 单次 |
| `TreeModeSwitch` 与 `FileTree` 的独立性（AC-11 / AC-13） | 边界 | 同屏渲两件 → 切 `TreeModeSwitch` 不改变 `FileTree` 的展开态与 `onFocus` 调用数（切换只换左栏组件，触达取数归 `architecture-canvas` 章节） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `ArchiFileEntry` fixture | **不 mock、不自建替身**：纯 props 数据（`entry()` 构造器收全七字段），组件本身是被测对象 | 全部用例 |
| `ui/scroll-area`（`@base-ui/react`） | **不 mock**：真实组合（滚动容器是被测渲染面的一部分；jsdom 下渲染为 DOM 容器，`scrollIntoView` 由实例级桩补） | 树渲染行 |
| `ui/button` / `lucide-react` | **不 mock**：真实渲染（chevron 与图标的可达性属性是被测面） | 全部用例 |
| `views/info/components/dir-tree` | **不 import、不参考实现**：spec 明令 MUST NOT 抽公共件；反向判据是 `file-tree.tsx` 源码零 `dir-tree` 引用（归「不可测试项」的静态检索） | —— |
| `Element.prototype.scrollIntoView` | **实例级最小桩**：只给命中行元素装 `scrollIntoView = vi.fn()`，`afterEach` 复原（组件侧是可选调用 `?.`，jsdom 无该 API 时静默跳过）；**不进** `test-setup.ts` 原型 | 滚到首条 / 零命中两行 |
| 时钟 / 网络 / 子进程 | **零依赖**（`FileTree` 与 `TreeModeSwitch` 零 invoke：取数在 hook 层，组件是纯呈现） | —— |

---

### `packages/desktop/src/views/architecture/components/architecture-canvas.tsx` -> `packages/desktop/src/views/architecture/components/architecture-canvas.test.tsx`

**改写既有测试文件**（4 处 `getByTestId('archi-element-tree')`、5 处 `archi-tree-node` 交互与停摆断言）——改写口径写死（R-7）：①元素树断言一律**先点分段控件 `data-mode="element"`** 再断言；②文件树断言用新 testid（`archi-file-tree` / `archi-file-node` / `archi-file-mark`）；③停摆态断言改为「左栏树零节点（两视图均不挂载）」；④画布 / 抽屉 / 标记断言**逐字保留**。`invokeMock` 需补 `archi_file_tree` 分支（既有 `serveGraph` 类装置扩一条命令分发）。本节同时承载跨模块组合用例（链路入口是本文件）。

#### 待测功能

<!-- design.md「公共函数 / API」表对本文件零行：CanvasBody 左栏改为「分段控件 + 条件渲 FileTree / ElementTree」+ useArchiFileTree 接线，列在「修改文件」表与「架构组件」表（D13 / D16），不是 API 行。被测面是该组件的左栏渲染分支与三段布局。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ArchitectureCanvas` 缺省渲文件树（AC-11 / spec Scenario「缺省渲文件树且可切回元素树」） | 正向 | 健康图 + `archi_file_tree` 返回三条 fixture → 挂载后 `archi-file-tree` 在场、`archi-element-tree` **不在场**；`archi-canvas` 与 `archi-drawer` 同屏（三段布局齐备） | 单次 |
| `ArchitectureCanvas` 分段切到元素树（AC-11 / D7） | 正向 | 点 `archi-tree-mode` 的 `data-mode="element"` 钮 → `archi-element-tree` 在场、`archi-file-tree` 不在场；元素树的 `archi-tree-node` 与切换前既有断言一致（入参与 testid 零变化） | 单次 |
| `ArchitectureCanvas` 切换不触发取数（AC-11 / spec Scenario「缺省渲文件树且可切回元素树」末句） | 异常 | 在文件树 / 元素树间来回切 4 次 → `archi_graph` / `archi_module` / `archi_drift` / `archi_file_tree` 的调用计数**全部不变**（切换只换左栏组件） | 单次 |
| 组合：点文件树模块行 → 画布圈定范围 + 抽屉取数（AC-11 / spec Scenario「点模块行圈定范围」） | 正向 | 文件树渲 `elementRoot` 行（`owners = ["App.Auth"]`）→ 点该行 → `archi-drawer-fqn` 变 `App.Auth`；`archi_module` 恰一次且载荷 `{ root, element: 'App.Auth' }`；`ReactFlow` 节点集合切到该元素 1-hop 邻域 | 组合迭代（链路：tree → focus → canvas + drawer） |
| 组合：画布节点点击 → 文件树反查高亮（AC-11 / 双向联动） | 正向 | `entries` 含两条同 `owners`（多 `path` 元素）的 `elementRoot` 行 → 点画布 `rf__node-Shell` → 文件树两条行同时高亮、`archi_file_tree` 调用计数不变（反查是纯派生，零重取数） | 组合迭代（链路：canvas → focus → tree） |
| `ArchitectureCanvas` 树取数失败不阻断画布（AC-12 / spec Scenario「树失败画布照常」） | 异常 | `archi_file_tree` reject、`archi_graph` / `archi_drift` 正常 → 左栏渲 `archi-file-tree-error`（文案含「文件树取数失败」与原因、含「画布照常渲染」）；画布节点 / 边与抽屉照常渲染、漂移标记照常呈现（`flow().nodes` 非空、`archi-drawer` 在场） | 组合迭代（两取数面独立失败） |
| `ArchitectureCanvas` 树加载中（AC-12 / spec Scenario「空树与加载中」/ D13） | 边界 | `archi_file_tree` 挂起未回 → 左栏渲 `archi-file-tree-loading`（文案「解析中…」）、`archi-file-tree` **不在场**；画布与抽屉照常（图取数已回） | 单次 |
| `ArchitectureCanvas` 树空态（AC-12 / spec Scenario「空树与加载中」） | 边界 | `archi_file_tree` 返回 `{ entries: [] }` → 左栏渲 `archi-file-tree-empty`（文案「仓库内无源文件」）、`archi-file-tree-loading` 不在场；画布照常 | 单次 |
| `ArchitectureCanvas` 树失败后换 root 恢复（AC-12 / spec Scenario「换 root 不与旧树混淆」） | 边界 | root A 树取数失败 → 错误行；`rerender({ root: 'B' })` 且 B 成功 → 错误行消失、`archi-file-tree` 在场且条目为 B 的 fixture（A 的树不冒充） | 单次 |
| `ArchitectureCanvas` 停摆态左栏零节点（AC-12 / `desktop-archi-canvas` MODIFIED「结构错的停摆态」） | 异常 | `diagnostics` 非空 → `archi-error-panel` 在场；`archi-file-node` / `archi-tree-node` **均零**（两视图均不挂载）；`archi-tree-mode` 控件不挂载；`ReactFlow` 一次都不挂（`flowProps` 长度 0）；`archi-drawer` 不在场 | 单次 |
| `ArchitectureCanvas` 空模型分支的左栏（AC-12 边界） | 边界 | `graph.elements` 为空且零诊断 → `archi-canvas-empty` 在场；`archi-file-tree` / `archi-element-tree` 均不挂载（既有 `CanvasBody` 早返回分支零改动）；用例内钉死该分支下 `archi_file_tree` 是否被调用（以 `CanvasBody` 是否挂载为准，避免把 hook 挂载层级写反） | 单次 |
| 左栏宽度随模式（AC-11 / D16） | 边界 | `treeMode === 'file'` → 栏容器含 `w-80`；切到 `element` → 含 `w-72` 且不含 `w-80`（design D16 的呈现契约） | 参数化迭代（2 模式） |
| 既有画布 / 抽屉 / 标记断言零变化（AC-17 / R-7 回归登记） | 边界 | 4 处元素树断言按「先点 `data-mode="element"`」改写后语义逐条保留；`toNodes` / `toEdges` / `neighborhoodPositions` / `defaultFocus` / `ArchiNodeView` / `StructureErrorPanel` / 抽屉接线 / `driftMarks` 合并的既有断言**逐字保留**（节点集合、边 id 与 style、节点坐标、`ReactFlow` 选项、`archi-canvas-loading` / `archi-canvas-error` / `archi-canvas-empty` 三态、`archi-drift-error` 行内提示、`mode` 差异与改档重取） | 单次（回归登记） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core` 的 `invoke` | **进程边界 mock**：`vi.hoisted` + `vi.mock` 替身，按命令名分发 `archi_graph` / `archi_module` / `archi_set_gate` / `archi_drift` / **`archi_file_tree`** 五类 fixture（既有装置扩一条分发臂） | 全部用例 |
| `@xyflow/react` | **观测垫片**（环境缺口而非业务 mock）：记录 `ReactFlow` 入参后交真实组件渲染（既有 `flowProps` 手法，度量垫片 ResizeObserver / offsetWidth / DOMMatrixReadOnly / getBBox 照抄 `change-flow-graph.test.tsx`） | 画布断言行 |
| `useArchiFileTree` / `FileTree` / `TreeModeSwitch` / `ElementTree` / `ElementDrawer` | **不 mock**：真实 hook + 真实组件组合（左栏分支与三段布局即被测语义；替身会让「缺省渲文件树」自证） | 全部用例 |
| `lib/marks` / `lib/drift-marks` / `lib/fqn` / `structure-error-panel` | **不 mock**：真实调用（零改动面，回归登记的判据依赖真实标记派生） | 画布 / 标记 / 停摆行 |
| `Element.prototype.scrollIntoView` | **实例级最小桩**（只在需要断言「滚到首条」的组合行装；本文件组合行以高亮集合为主判据，滚动细节归 `file-tree.test.tsx` 章节） | 反查高亮行 |
| 时钟 / 网络 / 子进程 | **零依赖**（组件层零 spawn；`vi.useFakeTimers` 仅既有零轮询臂使用） | —— |

---

## 不可测试项

以下条目属 design「变更清单」的新增 / 修改文件，或属 AC 的判据面，但**无单元测试路径**（成因逐条说明）。它们不进入 `## 单元测试`，进入验收的方式落在「静态检索 / 机械门禁 / 命令 + 留痕 / 编译期」四类，与 proposal「测试文件」表及 `tasks.md` 的自查行一致。

| 文件 / 模块 / 判据 | 类别 | 不可测试原因 | 验收方式 |
|---|---|---|---|
| `.dev-team/architecture/config.json` | 配置资产（新增） | 纯静态 JSON 单键单条，零代码路径；其**语义**（`exclude` 生效、无 `$schema` 时 `load_config` 不 `Err`、有 `$schema` 时 `Err`）已在 `tree.rs -> tree_test.rs` 章节以**等价合成 tempdir 形态**全覆盖（真仓配置 MUST NOT 作为单测夹具——测试不得依赖仓库自身资产） | AC-9：`grep -n "schema" .dev-team/architecture/config.json` 零命中；`python -c`/`node -e` 解析输出 `{'exclude': ['openspec/**']}`；`git status --short .dev-team/` 只多该一个文件（`models/` / `decisions/` / `api/` 零 diff）；手工跑一次 `archi_drift` 比对 `excluded[] +3 / unmatchedFiles −3` |
| `packages/desktop/src-tauri/crates/core/archi/src/lib.rs` | crate 根（模块声明 + 出口） | 无 `lib_test.rs` sidecar；内容为 `pub mod tree;` 与 `pub use` 四行，无独立行为面（出口可达性是编译期事实） | AC-3 / AC-6：出口符号可被下游 import（`cargo check --all-targets` + `commands/archi` / `infra/archi` 两消费方编译通过）；`grep -n "pub mod tree;\|pub use tree::file_tree;"` 各恰一处；`grep -c` 断言四新类型在 `pub use dto::{…}` 段内；`#[cfg(test)] mod` 段零改动（`tree_test` 由 `tree.rs` 自身声明） |
| `packages/desktop/package.json` | 配置面 | `version` 一行改动，无逻辑面；脚本内容零变化 | AC-16：`version` = `0.6.36`（相对 `0.6.35` 递增一个 patch）；`src-tauri/Cargo.toml` 的 crate 版本行不变（`git diff` 审查）；`scripts` 四条（`bindings:export` / `api:export` / `api:check` / `check`）逐字不变 |
| `packages/desktop/src/types/generated/bindings.ts` + `bindings.zod.ts` | 生成物 | `bindings:export` 机械产出，零手写面；其**形状**已在 `commands/mod.rs -> bindings/mod_test.rs` 章节以产物断言覆盖（`archiFileTree` 包装 + 四类型 + `argsSchemas` / `resultSchemas`） | AC-14：`pnpm run bindings:export` 后 `git diff` 零行；`pnpm run bindings:zod-check` 通过；再跑一次幂等零 diff |
| `packages/desktop/src-tauri/crates/core/archi/src/tree_test.rs`（本相位之前的**空文件**形态） | 测试文件本体（`tasks.md` 阶段三产物） | 空模块（仅头注释、零用例、零断言）本身无被测对象；它的存在价值是让 `#[cfg(test)] mod tree_test;` 可解析（编译闭合） | `cargo clippy --all-targets` 通过（零 `failed to resolve mod`）；本相位已把用例本体设计于 `tree.rs -> tree_test.rs` 章节，空文件由 test-gen 相位填充 |
| `tests/vocabulary_test.rs` | 既有测试文件（**零改动面**） | 本变更零改动该文件（`FileTreeMark` 不进四面词汇对拍），无非被测新增面 | AC-4 / AC-17：`git diff --stat crates/core/archi/tests/vocabulary_test.rs` 为空；`grep -c "FileTreeMark\|FileEntryKind"` 该文件零命中 |
| `packages/desktop/src/views/architecture/components/element-tree.test.tsx` | 既有测试文件（**零改动面**） | 元素树组件保留为「元素」分段视图，入参 / testid / 渲染零改动 | AC-17：`git diff --stat` 为空；既有断言全绿即证「元素树未因左栏改造而回归」 |
| AC-6 的静态 `grep` 判据（`src` 全域无第二份前缀匹配 / 生成物剥离） | 静态检索 | 需跨文件枚举（`tree.rs` 之外的全部产品 `.rs`），且判据含「除 `scan/owner.rs` 之外零命中」这种全局否定式——单测内读自身源码只能覆盖单文件，跨文件枚举属静态门形态 | AC-6：`grep -rn "starts_with(prefix\|strip_generated" packages/desktop/src-tauri/crates/core/archi/src packages/desktop/src-tauri/src \| grep -v "scan/owner.rs"` **零命中**；`grep -n "\.i\."` / `grep -n "replace("` 于 `tree.rs` 零命中；`grep -c "pub fn "` 于 `tree.rs` 恰 1 |
| AC-13 的源码 `grep` 判据（零 watch / 零定时器 / 零裸 `invoke`） | 静态检索 | 断言对象是源码字符串的出现次数（跨文件枚举），`use-archi-file-tree.test.ts` 的运行期面只能证「假计时器推进下零重发」 | AC-13：`grep -n "setInterval\|setTimeout\|watchSubscribe\|watch_subscribe" src/views/architecture/hooks/use-archi-file-tree.ts` 零命中；`grep -n "invoke("` 零命中（只走生成包装）；`grep -n "refresh"` 零命中 |
| AC-8 的 `scan/exclude.rs` / `exclude_test.rs` 零 diff | 零 diff 面 | 无新增测试面（本变更明令零改动该两文件，「零改动」本身不是可断言的行为） | AC-8 / AC-17：`git diff --stat crates/core/archi/src/scan/exclude.rs crates/core/archi/src/scan/exclude_test.rs` 为空 |
| AC-9 的「`openspec/` 下 3 个原型 tsx 不在树中 + drift 报告 +3 / −3」 | 命令 + 留痕 | 依赖**真仓**真实资产（`openspec/changes/archive/**/prototypes/*.tsx` 三个未被 gitignore 的 `.tsx`）与真仓 drift 报告计数；把本仓真实资产塞进 crate 语料会违反「测试夹具不依赖仓库自身资产」 | AC-9：手工跑一次 `archi_drift` 比对 `excluded[]` 与 `unmatchedFiles[]` 的迁移（+3 / −3），结果留痕于 `openspec/changes/archi-file-tree/reports/`；`archi_file_tree` 对真仓跑一次并核对 `openspec/` 零条目 |
| AC-15 的本仓实测基线（358 / 117 / 475 / `mark` 分布 / 42 个元素根各 ≥ 1 文件） | 命令 + 留痕 | 依赖全仓真实模型与真实源文件（667 源文件，其中 306 个测试文件被排除），不是可复现的合成夹具；spec 明写「不作为 MUST，登记留痕」 | AC-15：`tree.rs` 一次性统计用例（`#[ignore]` 或 ad-hoc 脚本）输出条目数 / `mark` 分布 / 每元素根的可见文件数，结果落 `reports/`；任一项与 proposal 实测不等即缺陷（MUST NOT 改基线表放行） |
| AC-16 的门禁命令面（`check` 四段链 / `cargo nextest` 全绿 / 版本双行） | 机械门禁 | 属静态脚本机械步骤（`package.json` 脚本驱动），agent 内不重复执行；本设计不复制测试运行脚本细节 | AC-16：`pnpm -C packages/desktop run check` 退 0（`server:check` → `bindings:export` 零 diff → `api:check` 零 diff → `client:check` 含 `knip` 零新增 ignore）；`cargo nextest run --workspace` 全绿；`git diff --stat` 只含本清单文件集 |
| AC-17 的边界零 diff 面（`models/**` / `decisions/**` / `api/**` / `drift/**` / `infra/archi/**` / `infra/agent/**` / `.dev-team/config.json` / `explore.md` / `Cargo.toml` / `element-tree.tsx` / 抽屉 / 面板 / `lib/**`） | 零 diff 面 | 无新增测试面（「零改动」不是可断言的运行时行为）；部分路径的行为侧证已在各章节的回归登记行（既有断言保持绿） | AC-17：`git diff --stat` 对上述路径逐一为空（单条命令一次核对）；配置隔离的静态面另加 `grep -rn "config.json" crates/core/archi/src/tree.rs crates/core/archi/src/scan/ src-tauri/src/commands/archi/mod.rs` 零命中 |
| `file-tree.tsx` 的「MUST NOT 抽公共件」反向判据 | 静态检索 | 断言对象是源码中的 `dir-tree` / `DirTree` 零引用，属跨文件文本检索；运行期无可观测差异 | AC-17 / spec「不抽公共件」：`grep -n "dir-tree\|DirTree" src/views/architecture/components/file-tree.tsx` 零命中；`git diff --stat src/views/info/` 为空 |
| 真实浏览器下的滚动视觉（`scrollIntoView({ block: 'nearest' })` 的实际滚动、滚动容器的视觉贴边、`data-testid` 行的横向溢出） | 手工验证 | jsdom 无布局引擎：`scrollIntoView` 是**可选调用**（D15），断言只能到「被调用 + 入参」一层；真实滚动偏移与视觉贴合不可机械观测 | AC-11：手工在 `pnpm run dev` 下打开 `/architecture`，验证焦点定位后视口滚到首条命中行且多 `path` 元素多行可见；滚动容器在深层目录下不横向溢出 |
| 大树（475 节点 / 84.2 KB）的首屏解析时间与交互流畅度 | 手工验证 | 性能体感与渲染帧率无单测判据（既有 `perf.rs` 只覆盖漂移扫描的合成夹具）；spec 明确本期不做懒加载 | AC-15 / R-5：手工在真仓打开 `/architecture` 并记录首屏「解析中…」到树渲出的时间；若超阈值则按 proposal 的退路（`archi_file_tree(root, dir)` 单层 + 前端缓存）另立变更 |
| `architecture-canvas.tsx` 的「切换不重取数」在**真实 Tauri runtime** 下的行为 | 手工验证 | 单测以 `invoke` 替身计数为判据（已覆盖调用次数）；真实 IPC 层的重发 / 缓存行为不属本层契约 | AC-11 / AC-13：手工在 `pnpm run dev` 下反复切换分段钮，观察后端日志零新增 `archi_*` 调用 |
| `repo-archi-model` ADDED「本仓架构域看护配置与排除留痕」的模型侧零变化 | 数据资产（架构模型） | 模型资产零改动是本变更的**前提**而非产物；其判据是 `git diff` 而非断言 | AC-17 / `repo-archi-model`：`git diff --stat .dev-team/architecture/models/ .dev-team/architecture/decisions/` 为空；`archi_graph` 对真仓零诊断（手工跑一次留痕） |
