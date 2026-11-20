# 设计: archi-file-tree-lazy-load

> **变更**: archi-file-tree-lazy-load（架构页 · 命令不冻结 + 文件树不参与首屏）
> **日期**: 2026-11-20
> **修订**: 2026-11-20 · AC-11 由「真机观测门」改写为「验收清单落盘 + 环境不具说明 + 归档后人工验收承接」（理由见 proposal R-13；先例 `desktop-agent-execution` 边界第 16 条 / 场景「相邻 assistant 消息不设实现期核验门」）。本设计的 AC-11 落点与待决问题 1 随动收敛，**AC-1 ~ AC-10 与实现面设计零改动**
> **修订 2**: 2026-11-20 · 测试文件面的移交边界收敛为「实现相位只承担**编译闭合**（`.await` 补丁 + async 宿主形态，`cargo clippy --all-targets` 编译测试目标所必需），**断言口径**（含 D6 计数拆条）、offload 判据与任何**测试执行记录**归测试相位」——D1–D10、变更清单、AC 落点零改动，仅「测试文件面」节补边界说明、`tasks.md` 归属随动
> **基于**: `proposal.md`（问题 / 提案一～四 / AC-1…AC-11 / R-1…R-13）、`explore.md`（§1–§8）、5 份 delta spec（`desktop-app-shell` ADDED ×1；`desktop-archi-file-tree` / `desktop-archi-canvas` / `desktop-archi-drift` / `desktop-workspace-code-stats` 各 MODIFIED ×1）
> **性质**: **执行契约翻面 + 取数策略换缺省**——零新命令、零新 DTO、零生成物变化、零判定语义变化、**源码面零新增文件**；唯一新增是验收留痕 `reports/acceptance.md`（手写、非源码、机器不消费）。实现面 4 份文件（1 份 Rust 命令面 + 2 份前端源码 + 1 行版本），5 份 delta spec 已在提案相位落盘

口径先钉死（沿 proposal「命名与口径」）：本变更**没有逐层懒加载**——文件树的 DTO、取数入口与组件语义逐字不动，只把「它是不是首屏必经之路」从「必经」改为「点了才取」；本变更也**不承诺更快**，只承诺「不冻结」。

---

## 起点实况核实（工作树实测）

| 面 | 实测 |
|---|---|
| `commands/archi/mod.rs` | 八命令全 `pub fn`；四条取数命令在 `:61`（`archi_graph`）/ `:139`（`archi_drift`）/ `:177`（`archi_file_tree`）/ `:193`（`archi_api_check`）；八条 `*_inner` 一律 `pub fn`（`:68` / `:156` / `:187` / `:204` …）；文件现**零** `spawn_blocking` |
| 命令面注解面 | `#[tauri::command]` ×8、`#[specta::specta]` ×8（`mod_test.rs` 有相等断言） |
| `commands/archi/mod_test.rs` | 50 个 `#[test]`、0 个 `#[tokio::test]`；直接调用四条包装 **29 处**，分布在 **19 个测试函数**（`八命令root校验四形态一律err` 一处占 4、`五条读命令零写盘` 占 3、`graph正常态与停摆态` / `archi_file_tree结构诊断非空即err` / `archi_file_tree配置非法err且合法生效` / `archi_file_tree与archi_graph同根一致` / `api_generate写后复检三列全空` 各占 2，其余各 1） |
| 既有 poll 辅助 | `mod_test.rs:916` 的 `block_on`（自建 noop-waker 循环）现只服务 runner future 一处（`:954`），与本次无关，保留 |
| 源码面形态断言 | `mod_test.rs:1216`（用例 `命令层零领域规则` 起于 `:1164`）：`line.starts_with("pub fn archi_")` 计数 **16**（八命令 + 八 inner）⇒ 四条命令改 `pub async fn` 后**必红** |
| 模块头 stale 禁表 | `mod_test.rs:1597` 断言模块头零 `五命令` / `四命令` / `六命令` / `其余五条`；模块头仍须含 `八命令`、`写命令两条`（`:1608` / `:1859`） |
| tauri 侧形态事实 | `tauri::async_runtime::spawn_blocking` 是模块级函数、内部 `RUNTIME.get_or_init(default_runtime)` 自取全局运行时；`JoinHandle<T>: Future<Output = tauri::Result<T>>`（`tauri-2.12.5/src/async_runtime.rs:164` / `:311`）⇒ `.await` 得 `Result<T, tauri::Error>`，`map_err` 臂收到的就是被包的 `JoinError` |
| 既有 async 命令先例 | `commands/changes/mod.rs:268` `create_change`（`pub async fn` + `#[specta::specta]` + `spawn_blocking`，`.await.map_err(\|e\| format!("create 任务失败: {e}"))?`）；`commands/changes/mod_test.rs` 23 个 `#[tokio::test]` |
| 前端 `'file'` 引用 | 恰 3 处：`architecture-canvas.tsx:250`（`options` 的文件项）、`:354`（`useState<TreeMode>('file')`）、`file-tree.tsx:14`（`TreeMode` 类型联合）；另有 3 处「缺省文件树 / 缺省 `file`」注释（`architecture-canvas.tsx:227` / `:336`、`file-tree.tsx:13`） |
| 分段控件现状 | `options` 数组（`:248`–`:266`）声明序为 文件 / 元素 / 契约，同时驱动 `TabsTrigger` 与 `TabsContent`；`Tabs` 受控（`value={mode}`）；`TabsList` 携 `data-testid="archi-tree-mode"` 与显式 `role="group"` |
| base-ui 语义 | `TabsPanel` 的 `keepMounted` 缺省 `false`（`@base-ui/react/tabs/panel/TabsPanel.js:37`）；`TabsTab` 设 `aria-selected={active}`（`tabs/tab/TabsTab.js:180`）⇒「非活动面板不挂载」与「缺省选中可判」均结构性成立 |
| 前端测试 | `architecture-canvas.test.tsx` 已有 `switchTo(label)` 辅助（`:341`，`role="tab"` + 文案）；`app.test.tsx` 的 invoke mock **未桩** `archi_file_tree`，缺省分支 `Promise.resolve(null)`（`:323`） |
| 后端其余面 | 除 `commands/mod.rs`（`all_commands!`）与 `bindings/mod_test.rs`（字符串断言）外，无其他文件引用四条命令 |
| 架构域现状 | `archi_drift`（full）`status = clean`；`apiFindings` 35 条全为 `docMissing`（warn，永不纳管）；`App.Backend.Core.Archi` / `App.Frontend.Views.Architecture` 均 `api = declared` 且无契约资产 |
| 版本 | `packages/desktop/package.json` `0.7.4`；`tauri.conf.json` `"version": "../package.json"`；`src-tauri/Cargo.toml` crate 版本 `0.1.0`（不随动） |
| 主 spec 旧句 | `openspec/specs/desktop-workspace-code-stats/spec.md:37` 仍写「sync 命令不在主线程执行」——由本 change 的 delta 在归档时覆盖；本变更 MUST NOT 手改 `openspec/specs/**`（AC-10） |
| `reports/` 现状 | change 目录下 `reports/` 只有工作流产出（`reports/archi/<timestamp>.json`、`reports/baseline/**`、`reports/test/**`），**无 `acceptance.md`**，也无「人工验收记录」的既有约定 ⇒ AC-11 的落盘路径由提案修订钉死（见 变更清单 · 新增文件） |

---

## 事实校准（不改提案意图，只定实现形态）

1. **`.await.map_err(...)?` 是「双层 Result」形态，不是笔误**。闭包返回 `Result<T, String>` ⇒ `spawn_blocking` 得 `JoinHandle<Result<T, String>>` ⇒ `.await` 得 `Result<Result<T, String>, tauri::Error>` ⇒ `map_err` 得 `Result<Result<T, String>, String>` ⇒ `?` 得 `Result<T, String>`，正是命令的返回类型。MUST NOT 为了「消双层」把 `*_inner` 调包成 `.map_err(|e| e)` 之类（那会造出第二套错误映射）。
2. **`JoinError` 臂的实际类型是 `tauri::Error`**（内含 `tokio::task::JoinError`）。spec/AC 的「`JoinError` 映射为 `Err`」在源码面就是该 `map_err` 臂；文案取 `{error}` 即含原因。
3. **`archi_drift` 的 `ProcessArchiveVcs` 装配留在 async 体**（`let vcs = ProcessArchiveVcs::new();` 后移入闭包）。它是 unit struct 的零 IO 装配、不是领域组装（`check_root` 与 `archi::drift::check` 仍全在闭包里的 `*_inner`）；这样四条闭包一律单表达式，AC-1 的「`*_inner(…)` 位于闭包内」可写成四条字面量断言（见 D6）。若把装配塞进闭包改写成块，字面量断言就得放宽成「窗口内包含」——不值得。
4. **模块头新句子禁用「四命令」字面量**：stale 禁表含 `四命令`（起点核实「模块头 stale 禁表」行），故措辞用「四条取数命令」。
5. **`命令层零领域规则` 的 16 计数必须随动**（起点核实「源码面形态断言」行）。proposal 的测试文件清单未列这条，本设计补裁为必改，并把它同时当作 AC-1 的源码面判据落点（D6）。
6. **运行时初始化位置不构成风险**：`spawn_blocking` 内的 `RUNTIME.get_or_init(default_runtime)` 是在首次调用处自建全局运行时——在 `#[tokio::test]` 里触发它也已由 `commands/changes/mod_test.rs` 的 23 个用例实证（它们 await 的 `create_change` 同样走 `spawn_blocking`）。故四条 offload 判据用例可用裸 `#[test]`（与 29 处 `.await` 的宿主 `#[tokio::test]` 互不牵连）。
7. **`TabsList` 的 `role="group"` 不动**：`role="tab"` 由 base-ui `Tab` 提供，spec 的寻址口径成立；把 `role="group"` 改成 `tablist` 属语义清理，不在本变更范围（AC-10 的改动集约束）。
8. **`openspec/specs/**` 零 diff**：`desktop-workspace-code-stats` 的旧句由 delta 在归档时覆盖，本变更不预改主 spec；AC-9 的「全仓无残留」判据是**归档后**的读数。
9. **AC-11 的留痕是工作流产物，不是源码**：`reports/acceptance.md` 不进编译 / 测试 / 生成物 / 契约资产，`git status` 里的它属 change 自身产物（AC-10 的改动集不因此扩张）；落盘归实现相位收尾任务，验收相位只读核对、MUST NOT 代写；六项真机观测 MUST NOT 作门禁 / 归档阻塞 / `tasks.md` 待办（写清单文件的收尾任务不在此列）。

---

## 关键设计决策

| # | 议题 | 定稿 | 理由 |
|---|---|---|---|
| D1 | 命令形态 | 四条取数命令改 `pub async fn` + `tauri::async_runtime::spawn_blocking(move \|\| *_inner(…))`；MUST NOT 用 `#[tauri::command(async)]` + `pub fn` | 阻塞池为阻塞 IO 设计；worker 池 = 核数（与 agent 流式任务抢池）。与 `create_change` / `promote_explore` 同形 |
| D2 | `*_inner` 与领域规则 | 八条 `*_inner` 与 `check_root` 逐字不动（包括「`check_root` 在闭包内执行」这一点）；命令层零新增规则 | AC-1「diff 只看 wrapper 与模块头」；root 四形态文案逐字不变（AC-3） |
| D3 | `JoinError` 文案 | `format!("<面>任务失败: {error}")`，半角冒号；`<面>` 四值：`架构图` / `漂移检查` / `文件树` / `契约检查` | 与 `create 任务失败: {e}` 同款标点（事实校准 2）；四值各自可定位；该臂不可测（R-2），文案不构成判据 |
| D4 | 不可测面纪律 | 零 test-only 注入缝（不加「测试用 panic 开关」、不导出测试钩子） | spec `desktop-app-shell`「`JoinError` 为前向防御、无注入缝」；与仓库「no test-only exports」同源 |
| D5 | 测试形态 | 19 个触及包装的用例 `#[test]` → `#[tokio::test]`（`commands/changes/mod_test.rs` 23 处先例）；既有 `block_on` 辅助不动（仍只服务 runner future） | 29 处 `.await` 需 async 宿主；`block_on` 的 noop-waker yield 循环不适合真阻塞任务（忙等） |
| D6 | 源码面形态判据 | `命令层零领域规则` 的计数拆成三条：`pub fn archi_` = **12**、`pub async fn archi_` = **4**、`tauri::async_runtime::spawn_blocking` = **4**；另加四条闭包字面量断言 `spawn_blocking(move \|\| archi_*_inner(` | AC-1 要求「源码面可断言」；把 asyncness 本身做成计数判据，任何一条退回同步即红（事实校准 5） |
| D7 | 缺省视图单点 | `treeMode` 初值 `'element'` **只在** `architecture-canvas.tsx` 持有；`file-tree.tsx` 的 `TreeMode` 注释删去「缺省 `file`」 | 同一事实两处持有会漂移（R-12 的一行注释豁免）；`TreeMode` 类型联合、取值与语义零改动 |
| D8 | 分段声明序单点 | 只重排 `options` 数组（元素 → 文件 → 契约）；`TabsTrigger` / `TabsContent` 仍由该数组映射，MUST NOT 在 JSX 里另写顺序 | 声明序唯一持有者；`data-testid` / testid 面零新增 |
| D9 | 缺省外的零改动 | hooks（四个）、`element-tree.tsx`、`contract-panel.tsx`、`mark` 映射、`ResizablePanel` 尺寸、`Tabs` 受控形态、`role="group"`、`all_commands!`、生成绑定、领域 crate、契约资产与模型 | proposal「不要修改」+ AC-8 / AC-10；任一被动即越界 |
| D10 | 版本 | `packages/desktop/package.json` `0.7.4` → `0.7.5`（单行）；`tauri.conf.json` 跟随、`Cargo.toml` crate 行不跟 | 版本纪律（AGENT.md）；R-9 |

---

## 架构组件 (Architecture Components)

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|---|---|---|---|---|
| 四条取数命令的命令面（**改**） | 参数转交 → 阻塞池执行 `*_inner` → `JoinError` 映射；IPC 签名 / 参数 / 返回 / 领域文案零变化 | `packages/desktop/src-tauri/src/commands/archi/mod.rs`（`:61` / `:139` / `:177` / `:193`） | `tauri::async_runtime`、`Path`、`ProcessArchiveVcs`（drift 专用）、四条既有 `*_inner` | Rust async + tokio blocking 池（经 tauri 全局运行时） |
| 命令层执行契约注记（**改**） | 模块头补一段：批量子系统 IO 走阻塞池、纪律的唯一持有者是 `desktop-app-shell` | 同文件模块头（`:1`–`:28`） | 无 | 注释（`//!`） |
| 左栏分段控件与缺省视图（**改**） | `treeMode` 初值 `'element'`；`options` 声明序 元素 / 文件 / 契约；两处「缺省文件树」注释随动 | `packages/desktop/src/views/architecture/components/architecture-canvas.tsx`（`:227` / `:248`–`:266` / `:336` / `:354`） | `TreeMode`（`file-tree.tsx`）、base-ui Tabs、三个视图组件 | React + base-ui Tabs + `ResizablePanel` |
| 文件树的缺省口径注释（**改**） | 删去 `TreeMode` 上方的「缺省 `file`」，避免与单点分叉 | `packages/desktop/src/views/architecture/components/file-tree.tsx`（`:13`） | 无 | 注释 |
| 文件树取数（**零改动**） | 挂载 / `root` 变化 / 上层 `key` 重挂三触发；缺省不挂载 ⇒ 零调用 | `.../hooks/use-archi-file-tree.ts` | 生成绑定 `commands.archiFileTree` | React hook |
| 分段钮（**零改动**） | `role="tab"` + 文案、`aria-selected` 选中态 | `@/components/ui/tabs.tsx` → base-ui | base-ui | 既有原语 |
| 版本行（**改**） | patch +1 | `packages/desktop/package.json` | 无 | JSON |

依赖方向零新增边（`src-tauri/src` 仍只装配 core/infra 端口，不持领域规则）；调用链形态只多一层线程跳转：

```
webview invoke('archi_file_tree')            ← IPC 面逐字不变（typed 包装 Promise<T>）
   └─> pub async fn archi_file_tree(root)    ← 命令 future 首次 poll = Pending
         └─> spawn_blocking(move || archi_file_tree_inner(Path::new(&root)))
               └─> check_root → archi::tree::file_tree(root)   ← 领域规则单点，零改动
```

---

## 变更清单 (Change Inventory)

### 新增文件

<!-- 源码面零新增文件：后端在既有命令面文件内改形态（新开模块会造出第二处命令注册面与第二个 `*_inner` 收口），前端只改既有两个组件的初值 / 声明序 / 注释。唯一新增是验收留痕（非源码，不进编译 / 测试 / 生成物 / 契约资产）。 -->

| 文件 | 说明 | 关联条目 |
|---|---|---|
| `openspec/changes/archi-file-tree-lazy-load/reports/acceptance.md` | 新建（手写、机器不消费）：AC-11 三节——① **验收清单**（explore F 六项逐条给出观测手段 / 预期 / 判据来源：release 进页拖动与「解析中…」可见 / 切「文件」不冻结 / `archi_graph`·`archi_drift`·`archi_file_tree` 三命令起止时刻 / debug 对照一次 / 连点刷新 N 次基线 / 冷热缓存各一次）；② **环境不具说明**（63k 文件工作区 `/d/Projects/wpsweb/client/app` 在机但无 release 产物；窗口拖动 / 不白屏 / 三命令起止时刻均不落 agent 可读产物 ⇒ 六项观测在 agent 相位不可履行）；③ **归档后人工验收承接**（六项观测与结论并入归档后人工验收，结论记文末；不符预期 MUST NOT 回退本 change，按 R-1 / R-10 另立变更） | 验收标准回应 · **AC-11** 行；`tasks.md` 阶段四（落盘 + 边界自查）；`test-design.md` 的 AC-11 两处落点由测试设计相位随动 |

> 该文件是**验收留痕而非源码**：不参与编译 / 测试 / 生成物 / 契约资产，属 change 自身产物（AC-10 的改动集约束不因它扩张）。六项清单的条目数与判据来源由 proposal AC-11 第 1 条给定，落盘时逐项写全、MUST NOT 增删项数；**真机观测本身 MUST NOT 作门禁判据 / 归档阻塞项 / `tasks.md` 待办项**（写清单文件的收尾任务不在此列）。

### 修改文件

| 文件 | 具体修改 | 说明 |
|---|---|---|
| `packages/desktop/src-tauri/src/commands/archi/mod.rs` | ① `archi_graph`（`:61`–`:65`）：`pub fn` → `pub async fn`，体改 `tauri::async_runtime::spawn_blocking(move \|\| archi_graph_inner(Path::new(&root))).await.map_err(\|error\| format!("架构图任务失败: {error}"))?`。② `archi_drift`（`:139`–`:145`）：`pub async fn`，先 `let vcs = ProcessArchiveVcs::new();` 再 `spawn_blocking(move \|\| archi_drift_inner(Path::new(&root), mode, &vcs))`，文案 `漂移检查任务失败: {error}`。③ `archi_file_tree`（`:177`–`:181`）：`pub async fn` + `spawn_blocking(move \|\| archi_file_tree_inner(Path::new(&root)))`，文案 `文件树任务失败: {error}`。④ `archi_api_check`（`:193`–`:197`）：`pub async fn` + `spawn_blocking(move \|\| archi_api_check_inner(Path::new(&root)))`，文案 `契约检查任务失败: {error}`。⑤ 模块头（`:1`–`:28`）补一段：四条取数命令走 `tauri::async_runtime::spawn_blocking`（阻塞池），命令体不在 IPC 线程内联；执行线程纪律的唯一持有者是 `desktop-app-shell`「命令执行线程纪律」，本文件不复述判据。⑥ 另四条命令、八条 `*_inner`、`check_root`、`use` 块逐字不动 | AC-1 / AC-2 / AC-3 / AC-4 / AC-9；`#[tauri::command]` 与 `#[specta::specta]` 各仍 8 处；模块头仍含 `八命令` / `写命令两条`，且零 `五命令` / `四命令` / `六命令` / `其余五条` 字面量 |
| `packages/desktop/src/views/architecture/components/architecture-canvas.tsx` | ① `:354` `useState<TreeMode>('file')` → `useState<TreeMode>('element')`。② `:248`–`:266` `options` 数组重排为 元素 → 文件 → 契约（三项的 `mode` / `label` / `content` 逐字不动，只换声明位置）。③ `:227` doc 改「缺省元素树，可切文件 / 契约视图」。④ `:336` doc 改「左栏树——缺省元素树，可切文件 / 契约视图」。其余（`Tabs` 受控、`TabsList` 的 testid 与 `role`、`TabsTrigger` / `TabsContent` 映射、`ResizablePanel` 尺寸、`CanvasBody` 其余逻辑、`defaultFocus`）逐字不动 | AC-5 / AC-6；缺省值单点 + 声明序单点；文件树不挂载 ⇒ 首屏 `archi_file_tree` 零调用 |
| `packages/desktop/src/views/architecture/components/file-tree.tsx` | **仅 `:13` 一行注释**：`/** 左栏树视图（分段控件的三值；缺省 \`file\`）。 */` → 不再陈述缺省（如 `/** 左栏树视图（分段控件的三值之一；缺省值由 architecture-canvas.tsx 的 treeMode 初值持有，不在此处复述）。 */`）。渲染、`TreeMode` 类型联合、标记、点击、反查、展开态与全部导出语义零改动 | AC-8（「零 diff」射程 = 除该行注释外）；R-12 |
| `packages/desktop/package.json` | `"version": "0.7.4"` → `"0.7.5"`（单行） | AC-10；`tauri.conf.json` 经 `"version": "../package.json"` 跟随；`src-tauri/Cargo.toml` 的 crate 版本行 `0.1.0` MUST NOT 随动 |

> **不在本清单的对象**（proposal「不要修改」+ D9）：`crates/core/archi/**`（`tree.rs` / `dto.rs` / `drift/**` / `scan/**` / `api/**`）、`hooks/**`、`element-tree.tsx`、`model-info.tsx`、`contract-panel.tsx`、`lib/**`、`commands/mod.rs`（`all_commands!`）、`src/types/generated/**`、`commands/stats/**`、`commands/config/**`、`.dev-team/architecture/**`、`openspec/specs/**`。

### 公共函数 / API

| 标识符 | 所在文件 | 变化 | 完整签名 | 说明 |
|---|---|---|---|---|
| `archi_graph` | `commands/archi/mod.rs` | 修改（`pub fn` → `pub async fn`） | `pub async fn archi_graph(root: String) -> Result<ArchiGraph, String>` | 体 = `spawn_blocking(move \|\| archi_graph_inner(Path::new(&root))).await.map_err(...)?`；IPC 面（`archi_graph(root: string) => Promise<ArchiGraph>`）与错误文案不变 |
| `archi_drift` | 同上 | 修改（同上） | `pub async fn archi_drift(root: String, mode: ArchiDriftMode) -> Result<ArchiDriftReport, String>` | `ProcessArchiveVcs` 装配留 async 体后移入闭包（事实校准 3）；`CancelFlag::never()` / 零去重零节流在 `*_inner` 内不变 |
| `archi_file_tree` | 同上 | 修改（同上） | `pub async fn archi_file_tree(root: String) -> Result<ArchiFileTree, String>` | `*_inner` 与四条非法 root 文案逐字不变 |
| `archi_api_check` | 同上 | 修改（同上） | `pub async fn archi_api_check(root: String) -> Result<ArchiApiCheck, String>` | 判据输入与报告零改动，只换执行线程 |
| `archi_module` / `archi_change_summary` / `archi_set_gate` / `archi_api_generate` | 同上 | 零改动 | —— | 保持同步形态（spec「明确不在适用面」） |
| `archi_*_inner` ×4 / `check_root` | 同上 | 零改动 | `archi_graph_inner(root: &Path) -> Result<ArchiGraph, String>` 等 | 只被移进闭包；签名 / 语义 / 文案逐字不变 |

> 前端**零公共 API 变化**：`ArchitectureCanvas` / `ElementTree` / `FileTree` / `ContractPanel` 的入参、导出与 testid 逐字不变；`TreeMode` 是类型（见下），缺省值不属 API。

### 类型定义

<!-- 整段省略：本变更零新增 / 零修改类型。`TreeMode = 'file' | 'element' | 'contract'` 三值与语义不动（只停写注释里的缺省陈述）；`ArchiFileTree` / `ArchiFileEntry` / `FileTreeMark` / `ArchiGraph` / `ArchiDriftReport` / `ArchiApiCheck` 及全部 `src/types/generated/**` 逐字不变。 -->

无。

### 配置

| 配置键 | 所在文件 | 变化 | 值类型 | 默认值 / 现值 | 说明 |
|---|---|---|---|---|---|
| `version` | `packages/desktop/package.json` | 修改 | `string` | `0.7.5`（现值 `0.7.4`，+1 patch） | 桌面源码改动的版本纪律；`tauri.conf.json` 经 `"version": "../package.json"` 跟随；`src-tauri/Cargo.toml` crate 版本行不跟；发布标签由 `.github/workflows/desktop-release.yml` 产出 |

> 零配置项新增 / 删除：无新 feature flag、无新 blocking 池配置（R-7：默认上限 512，本次最多 3–4 并发）、无新环境变量；`pnpm check` 四段命令串逐字不变。

### 测试文件面（断言口径与用例归测试相位；实现相位只承担编译闭合）

> **移交边界**：`commands/archi/mod_test.rs` 的 `.await` 补丁与 async 宿主形态（`#[test]` → `#[tokio::test]` + `async fn`）是**编译闭合的必需品**——`server:check` 的 `cargo clippy --all-targets` 会编译测试目标，四条包装改 async 后测试模块的同步调用形态即编译失败——故随 static-check 反馈由实现相位落地，且**不改断言口径**。下表第 ③ 项（offload 判据）与第 ④ 项（D6 计数拆条）属断言口径本身，连同任何测试执行记录一并归测试相位：实现相位不认领，`tasks.md` 不记录（见该文件移交清单）。

| 文件 | 必改点（机械约束，供测试相位执行） |
|---|---|
| `packages/desktop/src-tauri/src/commands/archi/mod_test.rs` | ① 19 个触及包装的测试函数 `#[test]` → `#[tokio::test]`（其中 9 个已带 `#[allow(non_snake_case)]`，改形态时属性并存保留）——**编译闭合项**，已由实现相位落地，本相位复核；② 29 处直接调用补 `.await`（同步命令 `archi_module` / `archi_set_gate` / `archi_api_generate` / `archi_change_summary` 不补）——同属**编译闭合项**；③ 新增 `fn assert_offloaded(future: impl std::future::Future)`（`Waker::noop()` + `Pin::pin!` + 单次 poll 断言 `Pending`），四命令各一条裸 `#[test]`（可传空 root，后台任务以 `Err` 收尾、结果丢弃）；④ `命令层零领域规则` 的形态计数按 D6 拆三条 + 四条闭包字面量；⑤ 「包装 ↔ `_inner` 逐字段一致」四处（`三命令与inner结果逐字段一致` / `archi_drift包装与inner一致` / `archi_file_tree包装与inner逐字段一致` / `api_check包装与inner逐字段一致`）保留同参 deep-equal；⑥ 非法 root 四形态、结构诊断非空、空模型 `Ok`、配置非法 / 合法两端用例的断言文案逐字保留 |
| `packages/desktop/src/views/architecture/components/architecture-canvas.test.tsx` | ① 4 条语义用例（缺省三段布局 / 缺省视图换面 / 三视图共用栏位 / 契约视图往返）把「图就绪」信号由 `archi-file-tree` 改为 `archi-tree-mode` 或 `archi-element-tree`，触及文件树处插 `switchTo('文件')`；② 12 条依赖文件树在场或点击的用例插 `switchTo('文件')`（`switchTo` 辅助已存在）；③ 新增「进页面 `archi_file_tree` 调用数 = 0，点「文件」后恰 1 次，往返各再 1 次」与「分段钮 `role="tab"` 声明序 = 元素 / 文件 / 契约且缺省 `aria-selected` 为真」两条；④ **MUST NOT** 留 `queryByTestId('archi-file-tree') === null` 这类否定断言作为缺省判据（缺省换面后永真） |
| `packages/desktop/src/app.test.tsx` | 建议新增一条三入口用例（hash 直入 / 侧栏进入 / 切走再切回 ⇒ `countOf('archi_file_tree') === 0`、`archi_graph` 各 1）；该文件 mock 未桩该命令（缺省 `resolve(null)`），零桩成本 |
| `packages/desktop/src/views/architecture/components/file-tree.test.tsx` | **零改动**（直测 `FileTree`，不经缺省视图） |
| `packages/desktop/src-tauri/src/bindings/mod_test.rs` | **零改动且必须照旧通过**——逐字签名 / zod 表项 / archi 八命令名单与相邻序断言是 AC-4 的反证：需要改它就说明 async 化动了出线面 |

---

## 数据模型 (Data Model)

零新增实体、零字段变化、零持久化面变化——本变更只改「取数发生在哪个线程」与「哪个视图先挂载」。下列三项的形态在本变更后逐字不变：

| 对象 | 字段 / 结构 | 关系 | 持久化 |
|---|---|---|---|
| `ArchiFileTree` / `ArchiFileEntry` / `FileTreeMark` | 条目七字段（`path` / `kind` / `mark` / `owners` / `contains` / `exists` …）、标记五值（`elementRoot` / `ambiguous` / `member` / `containsElements` / `none`） | 扁平条目 → 前端派生森林（父缺席挂根、目录先于文件、按 `name` 排序） | 无（每次命令调用现算，零缓存零落库） |
| `ArchiGraph` / `ArchiModuleDetail` / `ArchiDriftReport` / `ArchiApiCheck` | 结构与 `REPORT_VERSION` / `status` 五值 / `apiFindings` 纳管口径逐字不变 | 漂移报告与门禁面 / 引擎面同源（五消费面共用同一结构） | 命令面零写盘；落盘者仍恒为门禁 runner（`reports/archi/<timestamp>.json`） |
| 生成绑定（`src/types/generated/**`） | typed 包装 / zod 表项 / DTO 类型 | `all_commands!` 命令清单与相邻序 | 生成物（`bindings:export` 幂等，本变更后零 diff） |

**「命令形态」不是数据面**：asyncness 不进 DTO、不进绑定、不进报告；它只决定命令 future 的首次 poll 结果（`Pending`）与命令体所在线程（阻塞池）。

**前端状态面零新增**：`treeMode`（`'element'` 初值）是既有 `useState` 的值变化，不产生新的持久化 / 序列化形态；文件树的展开态仍是 per-path 翻转集合（按 `path` 在重挂间保持）。

**验收留痕不是数据面**：`reports/acceptance.md` 是手写 markdown——无 schema、无消费方（机器不消费）、不进 DTO / 生成物 / 门禁输入，它的新增不改变任何数据形态与持久化面。

---

## 路由 / API 设计 (Route / API Design)

本变更零新增端点；触及四条既有 IPC 命令的**执行线程**，路由 / 参数 / 返回 / 错误面逐字不变。

| 命令（IPC 名） | 生成包装 | 输入 | 输出 | 错误 | 执行线程（本变更后） | auth |
|---|---|---|---|---|---|---|
| `archi_graph` | `archiGraph(root: string)` | `{ root: string }` | `Promise<ArchiGraph>` | `Err(String)`（root 四形态文案不变） | 阻塞池（`spawn_blocking`），首次 poll `Pending` | 无（本地 IPC，无鉴权面） |
| `archi_drift` | `archiDrift(root: string, mode: ArchiDriftMode)` | `{ root, mode }` | `Promise<ArchiDriftReport>` | 同上 + 增量 vcs 硬失败文案不变 | 同上 | 无 |
| `archi_file_tree` | `archiFileTree(root: string)` | `{ root: string }` | `Promise<ArchiFileTree>` | 同上 + 结构诊断非空 | 同上 | 无 |
| `archi_api_check` | `archiApiCheck(root: string)` | `{ root: string }` | `Promise<ArchiApiCheck>` | 同上 + 契约资产非法 | 同上 | 无 |

**前置 / 并发语义（明写，避免被读成护栏）**：四条命令零取消（`CancelFlag::never()` 不变）、零 in-flight 去重、零节流；重复触发（连点刷新 / 反复切页 / `key` 重挂）会**真正并行**执行每次任务，后到者覆盖呈现。该形态登记在 `desktop-archi-drift`「并发扫描（已知限制）」与 `desktop-app-shell`「并发度不变」两处，本设计不追加代码护栏（R-1）。

**前端取数时机**（与命令面正交）：进 `/architecture`（hash 直入 / 侧栏 / 切走再切回）⇒ `archi_graph` 1 次、`archi_drift` 1 次、`archi_module` 1 次（聚焦派生）、`archi_file_tree` **0 次**；点「文件」⇒ `archi_file_tree` 1 次；两视图往返 ⇒ 每次重挂各 1 次，其余命令计数不增。

---

## 依赖 (Dependencies)

运行时依赖：**零新增**。

| 依赖 | 变化 | 用途 |
|---|---|---|
| `tauri`（`async_runtime`） | 零新增（命令面已依赖） | `spawn_blocking` 与 `JoinHandle` 的 `Future` 实现 |
| `tokio`（crate 依赖） | 零新增 | blocking 池实现（经 tauri 全局运行时，无直接调用） |
| `vcs-runtime`（`ProcessArchiveVcs`） | 零新增 | `archi_drift` 闭包内的 unit struct 装配（不变） |

构建 / 测试依赖：**零新增**。

| 依赖 | 变化 | 用途 |
|---|---|---|
| `tokio`（dev，`features = ["rt", "macros"]`，已在案） | 零新增 | `#[tokio::test]` 的 19 个用例宿主（`commands/changes/mod_test.rs` 已用同款） |
| `std::task::{Waker, Context, Poll}` / `std::pin::pin!` | 零新增（std） | offload 判据（Rust 1.95 工具链，`Waker::noop` 自 1.85 稳定） |
| 前端 npm 依赖 | 零新增 | 只改初值 / 数组序 / 注释；`bindings:export` 应幂等（`src/types/generated/**` 零 diff） |

`Cargo.lock` 与 `pnpm-lock.yaml` 零净增包；`[workspace.dependencies]` / `members` 零改动。

---

## 验收标准回应

| AC | 落点 | 归属 |
|---|---|---|
| **AC-1** 形态 | D1 / D2 + 修改文件第 1 行（四条 wrapper 的确切代码形态）+ 事实校准 1（双层 Result） | 阶段一 |
| **AC-2** 非阻塞判据 | D5 / D6 + 事实校准 6：`assert_offloaded`（`Waker::noop()` + 单次 poll 断言 `Pending`），四命令各一条；MUST NOT 计时断言 | 测试相位（辅助函数签名与放置已定） |
| **AC-3** 结果等价 | D2（`*_inner` 与 `check_root` 逐字不动）⇒ 四条非法 root 文案 / 结构诊断非空 / 空模型 `Ok` 天然等价；「包装 ↔ inner 一致」用例保留同参 deep-equal | 阶段一（代码）／测试相位（用例） |
| **AC-4** 出线面零 diff | D1（specta 显式处理 asyncness）+ D9：`all_commands!` / `src/types/generated/**` / `bindings/mod_test.rs` 三面零改动，且后者的逐字签名与相邻序断言照旧通过（反证） | 阶段一（代码）／桌面端 static-check + `bindings:export` diff |
| **AC-5** 缺省视图的时机 | D7 / D8 + 修改文件第 2 行：缺省 `'element'` ⇒ `FileTree` 不挂载 ⇒ 首屏零 `archi_file_tree`；`TabsContent` 非活动面板不挂载（起点核实「base-ui 语义」行：`keepMounted=false`）⇒ 切回各 1 次 | 阶段二（代码）／测试相位（计数器断言） |
| **AC-6** 分段控件与栏位 | D8（`options` 声明序单点）+ D9（`ResizablePanel` / `Tabs` 受控不动）⇒ 三钮声明序 元素 / 文件 / 契约、缺省选中由 `aria-selected` 表达 | 阶段二（代码）／测试相位 |
| **AC-7** 失败面不回归 | D9：`useArchiFileTree` 零改动 ⇒ 失败只落 `archi-file-tree-error`、旧树不冒充、加载 / 空树文案照旧 | 零代码（沿既有实现）／测试相位（用例插 `switchTo('文件')`） |
| **AC-8** 语义不漂移 + 锚点不退化 | D7（`file-tree.tsx` 仅一行注释）+ 测试文件面表第 2 行第 ④ 条（MUST NOT 留否定断言） | 阶段二／收尾核对（`git diff` 零输出四面） |
| **AC-9** 纪律与已知限制成文 | 5 份 delta spec 已由提案相位落盘；本变更实现面**零 spec 改动**。反向判据（无「sync 命令不在主线程执行」残留）是**归档后**读数——主 spec 旧句由 delta 覆盖（事实校准 8） | 零代码／归档时生效 |
| **AC-10** 范围与版本纪律 | 修改文件表恰 4 行 + 版本表；收尾核对项见 tasks 阶段三 | 阶段三 |
| **AC-11** 验收留痕（真机观测不作门） | `reports/acceptance.md` 在案且三节齐备（缺一节、清单缺一项即不成立）：① **验收清单**六项（观测手段 / 预期 / 判据来源，条目数不增删）；② **环境不具说明**（无 release 产物、窗口拖动与三命令起止时刻无 agent 可读面 ⇒ 六项观测在 agent 相位不可履行）；③ **归档后人工验收承接**（观测与结论并入归档后人工验收，不符预期按 R-1 / R-10 另立变更、MUST NOT 回退本 change）。反向判据：真机观测 MUST NOT 作门禁 / 归档阻塞 / `tasks.md` 待办项（写清单文件的收尾任务不在此列）；「不冻结」由 AC-2 首次 poll 判据机械承接，时长非承诺面（R-10）。下游 `test-design.md` 的 AC-11 两处落点由测试设计相位随动 | 实现相位收尾（落盘）／验收相位（只读核对「文件在案 + 三节齐备 + 清单六项可操作」） |

---

## 待决问题 (Open Questions)

1. **（已收敛 · 2026-11-20 提案修订 R-13）AC-11 验收留痕的落盘路径**：原议「记入 `reports/`，无既有约定」→ proposal AC-11 钉死为 `openspec/changes/archi-file-tree-lazy-load/reports/acceptance.md`（手写、机器不消费），且真机观测并入归档后人工验收、不作门禁。本设计随之落「变更清单 · 新增文件」与「验收标准回应 · AC-11」。保留此行仅为追溯，不再走裁决。
2. **`app.test.tsx` 三入口用例是否必做**：proposal 标为「可选」（改动量视角），AC-5 又明文列举三入口。本设计倾向补一条（零桩成本，`countOf` 已有），不过若测试相位判定「切走再切回」已由 `architecture-canvas.test.tsx` 的重挂用例等价覆盖，可只保留 hash 直入 + 侧栏两条。裁决权在测试相位。
3. **`spawn_blocking` 的 `JoinError` 臂永久不可测**（R-2）：本设计按 spec 落「前向防御、零注入缝」。若将来引入「阻塞任务 panic 的可观测面」诉求，须连同「命令层零 test-only 导出」一起重议，属独立变更。
4. **`archi_graph` 多一次调度带来的时序差异**（explore R4）：命令本就毫秒级，异步化只多一次池跳转；若有测试隐式依赖「首轮 invoke 在同一微任务内 resolve」，按测试相位的实测为准调整等待锚点——本设计不为此预留代码缝。
