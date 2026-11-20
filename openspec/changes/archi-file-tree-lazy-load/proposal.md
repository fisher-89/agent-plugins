# 提案: archi-file-tree-lazy-load

> **变更**: archi-file-tree-lazy-load（架构页 · 命令不冻结 + 文件树不参与首屏）
> **日期**: 2026-11-20
> **状态**: draft
> **修订**: 2026-11-20 · AC-11 由「真机观测门」改写为「验收清单落盘 + 归档后人工验收」（理由见 R-13；先例 `desktop-agent-execution` 边界第 16 条 / 场景「相邻 assistant 消息不设实现期核验门」）。AC-1 ~ AC-10 与实现面随此修订**零改动**
> **来源**: 本 change 目录 `explore.md`（§1 问题陈述 / §2 实测证据 / §3 改动方案 / §4 A–F 判据 / §5 R1–R10 / §7 排除方案与错误假设澄清 / §8 范围外）
> **性质**: **执行契约翻面 + 取数策略换缺省**——零新命令、零新 DTO、零生成物变化、零判定语义变化；改动 = 四条命令的形态 + 一个缺省值 + 五处 spec 纪律/口径成文 + 一份验收留痕（`reports/acceptance.md`，手写、机器不消费）
> **不含**: `archi_drift` 的取消 / 去重 / 节流与延迟优化；文件树逐层懒加载或深度截断；`code_stats` 的 async 化；`archi_api_generate` 的 async 化

### 命名与口径（先钉死，防评审跑偏）

change 目录名含 `lazy-load`，**但本次没有逐层懒加载**：文件树的 DTO、取数入口与组件语义逐字不动，只把「它是不是首屏必经之路」从「必经」改为「用户点了才取」。命名是历史残留（explore §3.2 之前的候选方案），本提案不以名为准、以本节口径为准。

| 不要写 / 不要读成 | 因为 |
|---|---|
| 「文件树性能优化」 | 焦点会被引向组件与 DTO，掩盖「所有同步命令都这样」这个大前提（`code_stats` 即同病） |
| 「首屏加速 X%」 | 本次不承诺时长：大工程的首屏时长由 `archi_drift` 决定，本次只把它从「冻结主线程」改为「后台跑」 |
| 「加了懒加载」 | 无逐层取数、无 `hasChildren`、无遍历根收窄；只有「缺省不取」 |
| 「修复冻结 bug」 | 冻结是执行契约缺口的必然结果，写成 bug 会掩盖纪律缺席 |

---

## 问题

### 症状与两层根因

63k 文件工作区上进入 `/architecture`，窗口不可拖动、不可切换、不可重绘，持续数十秒；左栏恰好停在「解析中…」，于是症状被读成「文件树解析慢」。实际是两层根因叠加，**只治一层都不成立**：

| # | 根因层 | 机制 |
|---|---|---|
| ① | **执行契约层** | archi 的四条取数命令是同步命令（Tauri `ExecutionContext::Blocking`），命令体跑在 IPC 线程（Windows = 主线程）上。任何「批量读子系统」的工作都会冻结 webview——**与具体是文件树还是漂移无关**。这解释了为什么症状总落在最后一个还没返回的命令上：图 → 漂移 → 文件树三条命令在同一线程排队 |
| ② | **取数策略层** | 左栏缺省即文件树，`FileTree` 挂载即 invoke 一次全仓投影（实测 42,847 文件 → 59,771 节点 → 约 10.4 MB → 3.1 s walk），且每次从「元素 / 契约」切回「文件」都重来一遍；而它只是第二个排队者——真正的量在 `archi_drift`（16 s IO + 42k 文件 AST 解析） |

**只做一半的后果**：只修 ① → 不冻结，但首屏仍付 3.1 s + 10.4 MB；只修 ② → 首屏省一次 walk，但 drift 照样冻结主线程数十秒。

### 实测证据（explore §2，环境：16 逻辑核 / 63,023 个 git 跟踪文件）

| 指标 | 实测 |
|---|---|
| 过默认排除矩阵 + 架构域 `exclude` 后进树文件 | 42,847 |
| 文件树节点总数（文件 + 合成目录） | 59,771 |
| 全量 payload（JSON 未压缩） | 约 10.4 MB |
| 目录遍历耗时（同等剪枝） | 3,146 ms |
| 全量读源文件耗时（约等于 drift 的 IO 段） | 16,336 ms（492 MB / 10.2M 行） |
| 默认展开（路径段数 ≤ 3）即渲染的行数 | 684（⇒ DOM 非主因） |

推论：`archi_drift` 是绝对大头（IO + oxc 全量解析 + Rust resolver），`archi_file_tree` 次之；前端渲染不是主因。「CPU 不足 5%」是「单线程 + IO 等待」的指纹，不是「机器没那么忙」。

---

## 提案

### 一 · 后端：四条取数命令异步化（执行契约）

```rust
#[tauri::command]
#[specta::specta]
pub async fn archi_file_tree(root: String) -> Result<ArchiFileTree, String> {
    tauri::async_runtime::spawn_blocking(move || archi_file_tree_inner(Path::new(&root)))
        .await
        .map_err(|error| format!("文件树任务失败：{error}"))?
}
```

| # | 面 | 裁定 |
|---|---|---|
| 1 | 改哪四条 | `archi_graph` / `archi_drift` / `archi_file_tree` / `archi_api_check`——archi 命令面的**四条取数命令** |
| 2 | 形态 | `pub async fn` + `tauri::async_runtime::spawn_blocking(move \|\| *_inner(…))`；落 tokio **blocking** 池（为阻塞 IO 设计） |
| 3 | `*_inner` | 签名与语义**零改动**（`&Path` 入参、`Result<T, String>` 出参、错误文案逐字不变），只被移进闭包 |
| 4 | 为何不用 `#[tauri::command(async)]` + 保持 `pub fn` | 该形态落 tokio **worker** 池（= 核数），四条命令并发即占 4 个 worker，与 agent 流式任务抢同一池；blocking 池才是承载阻塞 IO 的那一个 |
| 5 | 绑定与清单 | `specta-macros` 显式处理 asyncness ⇒ 生成仍是 `archiFileTree: (root: string) => __TAURI_INVOKE<ArchiFileTree>(…)`；`bindings:export` 幂等、`all_commands!` 零 diff、`bindings/mod_test.rs` 的逐字签名与相邻序断言照旧通过 |
| 6 | 不改的四条 archi 命令（+ `workspace_config` 同理） | `archi_module`（点击级、单模型解析）、`archi_set_gate`（毫秒级写）、`archi_change_summary`（低频、带 `State<WorkspaceStores>`，改 async 须先 `for_root` 取 `Arc<Store>` 再移入闭包）、`archi_api_generate`（写命令，成本随模型与契约规模而非仓库规模：`workspace_render` 只读模型 + 契约资产，不枚举源码树）、`workspace_config`（单文件读，非批量子系统 IO） |

**形态统一的理由**：只把 `archi_drift` 改 async 也能解冻，但四条取数命令留在同一 IPC 线程上排队时，快命令仍会被慢命令按线程阻塞；命令面形态统一后，「谁在等谁」不再由线程调度决定。

### 二 · 前端：左栏缺省改元素树（取数策略）

```diff
- const [treeMode, setTreeMode] = useState<TreeMode>('file');
+ const [treeMode, setTreeMode] = useState<TreeMode>('element');
```

分段控件重排为 **元素 / 文件 / 契约**（缺省值即首项，控件本体与 testid 不变）。

| 面 | 变更 |
|---|---|
| 分段按钮顺序 | 元素 / 文件 / 契约（`TabsList` 内 `TabsTrigger` 声明序） |
| 首屏 `archi_file_tree` 调用数 | **0**（hash 直入 / 侧栏进入 / 切走再切回三种入口都不取）；点「文件」后才 1 次 |
| 切回「文件」 | 每次重挂各 1 次（`TabsContent` 非活动面板不挂载的既有语义不变） |
| 文件树组件与 DTO | 零改动：条目投影、标记五值、点击与反查、展开态、钩子三触发纪律逐字不变 |
| 失败面 | 文件视图取数失败仍只落 `archi-file-tree-error`，画布与元素信息面板照常 |

收益：省一次 3.1 s 全仓 walk + 约 10.4 MB payload，且文件树取数与 drift 彻底解耦——不点「文件」，这笔成本完全不发生。

**为什么不是逐层懒加载 / 深度截断**（explore §7）：缺省视图切换后首屏已被清零，懒加载的收益只剩「点开文件视图之后」；而每层若仍调全仓 `walk_source_files`，会变成每层 3 s、比现状更慢（真正的分层必须收窄遍历根并保持 gitignore + `ExcludeSet` 语义），DTO 与 spec 大改、本次不值得。

### 三 · 纪律与已知限制成文（五处 spec 落笔，零代码）

| # | 落点 | 内容 |
|---|---|---|
| 1 | `desktop-app-shell` **ADDED** 一条纪律 | 批量子系统 IO 命令 MUST 为 `async fn` + `spawn_blocking`，MUST NOT 同步注册；反形态 `#[tauri::command(async)]` + `pub fn` MUST NOT 采用；**判据 = 命令 future 首次 poll 必为 `Pending`**；适用面 = 四条取数命令 + `code_stats` 为登记债务；`JoinError` 为前向防御、不加 test-only 注入缝；**并发度不变**（无取消、无去重、无节流） |
| 2 | `desktop-archi-drift` **MODIFIED** 边界段 | 命令面 `CancelFlag::never()` + 无去重无节流 ⇒ 重复触发产生**并行全仓扫描**；本能力不提供取消与去重，治理归独立变更 |
| 3 | `desktop-workspace-code-stats` **MODIFIED** | 纠正「sync 命令不在主线程执行」为「IPC 线程内联、长任务冻结窗口」，并把 `code_stats` 的 async 化登记为该纪律的待清偿债务（零代码改动） |
| 4 | `desktop-archi-file-tree` **MODIFIED** | 缺省视图由文件树翻为元素树 + 分段序 + 首屏零取数 |
| 5 | `desktop-archi-canvas` **MODIFIED** | 左栏缺省视图段落与「两挂载点缺省」场景随动 |

### 四 · 明确不承诺 / 零扩散边界

| 面 | 为什么不动 |
|---|---|
| `archi_drift` 总时长 | 本次口径是「不冻结」而不是「更快」；延迟 / 增量触发治理「总时长」，单独立项 |
| 取消 / 去重 / 节流 | 涉及命令面 + 领域 + 测试；本次裁决为零代码护栏 + 三处留痕（见风险 R-1） |
| 文件树 DTO / 遍历 / 标记 | 缺省视图切换不改数据面；`tree.rs` / `tree_test.rs` / `file-tree.tsx` 渲染与点击、`use-archi-file-tree.ts` 及测试零改动 |
| 生成物与 IPC 类型 | 四命令签名（IPC 面）不变 ⇒ `bindings:export` 幂等、archi 八命令名单与相邻序零 diff |
| 契约资产与架构模型 | 纪律落 spec；`App.Backend.Core.Archi` / `App.Frontend.Views.Architecture` 无契约资产（`docMissing` 仅 warn），本变更不补（R-8） |

---

## 能力

### 新增能力

无。本变更不产生新能力：执行线程纪律是 `desktop-app-shell` 的新关注点，缺省视图是既有能力的口径翻转。

### 修改的能力

| 能力 | delta | 改什么 |
|---|---|---|
| `desktop-archi-file-tree` | **MODIFIED ×1** | 「左栏文件树呈现、分段切换与双向联动」：缺省「文件树」→ 缺省「**元素树**」；分段控件声明序 元素 / 文件 / 契约（缺省值即首项）；非活动面板不挂载语义保留，但其含义变为「首次挂载 = 用户切到文件视图」⇒ 首屏零 `archi_file_tree`；场景改二、增二。Module Contract 的 `archi_file_tree` 命令行补一句形态指针（IPC 签名 / 参数 / 返回 / 错误文案零变化，纪律归 `desktop-app-shell`） |
| `desktop-archi-canvas` | **MODIFIED ×1** | 「单一画布组件与两处挂载点」：左栏三视图的缺省由文件树改为元素树（两挂载点同）；Module Contract 以「模块增量」表重列受影响行（`architecture-canvas.tsx` 的 `treeMode` 初值与 `TabsTrigger` 声明序、`element-tree.tsx` 成为缺省、`file-tree.tsx` 由「缺省视图」改为第二视图、四个 hook 零改动），「缺省渲它」的要求改为「缺省不渲它」 |
| `desktop-app-shell` | **ADDED ×1** | 新关注点「命令执行线程纪律（批量子系统 IO）」：形态 / 反形态 / 首次 poll 判据 / 适用面四条 / `code_stats` 登记债务 / `JoinError` 前向防御 / 并发度不变 / 出线面零变化 |
| `desktop-archi-drift` | **MODIFIED ×1** | 「判定单源与消费面同源」：取消除外补「无去重、无节流」，新增「并发扫描（已知限制）」条与一条 scenario |
| `desktop-workspace-code-stats` | **MODIFIED ×1** | 「代码统计解析命令」：sync 形态那条改写——「sync 命令不在主线程执行」纠正为「IPC 线程内联、长任务冻结窗口」，并把 async 化登记为该纪律的待清偿债务（零代码改动）；补一条否定 scenario |

> **本修订的 spec 影响：零 delta**——AC-11 改写只动验收留痕与风险 / 范围口径，五份 delta 逐字不变（验收清单属工作流产物，不进 spec 语料）。

### 零 delta 的能力（出现 spec diff 即误改）

| 能力 | 理由 |
|---|---|
| `desktop-archi-model` | 其命令面契约只说「八命令薄包装 + `*_inner` 可测」，不说线程形态；async wrapper 仍是薄包装、仍各配一个 `*_inner` ⇒ 陈述成立。线程形态的唯一持有者是 `desktop-app-shell` 新纪律（不在此处复述，避免同一事实两处持有） |
| `desktop-ipc-type-bindings` | 生成绑定逐字节零 diff（specta 显式处理 asyncness）；命令清单 / 相邻序 / 类型清单零变化 |
| `desktop-archi-api-check` / `desktop-archi-contract-panel` | 判据面与契约视图零变化；`archi_api_check` 只换执行线程，不改判据输入与报告 |
| `desktop-archi-api` | 契约资产形态与生成链零改动 |
| `desktop-workspace-config` | 其 spec 只写「沿用 sync 惯例，如需 async 由 design 定夺」，未陈述错误事实；`workspace_config` 是单文件读，不在本纪律的适用面内 |
| `desktop-archi-agent` | 知识文本与 agent 工具面零改动（工具面恒全量调用的执行面不在本变更） |

---

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `packages/desktop/src-tauri/src/commands/archi/mod.rs` | 四条取数命令 `pub fn` → `pub async fn` + `tauri::async_runtime::spawn_blocking(move \|\| *_inner(…))`，`JoinError` 映射为 `Err`（文案含原因）；`*_inner` 与另四条命令逐字不动；模块头注释补一句执行契约（批量子系统 IO 走阻塞池，纪律见 `desktop-app-shell`） |
| `packages/desktop/src/views/architecture/components/architecture-canvas.tsx` | `treeMode` 缺省值 `'file'` → `'element'`；`options` 数组序 元素 / 文件 / 契约；两处「缺省文件树」注释随动 |
| `packages/desktop/src/views/architecture/components/file-tree.tsx` | **仅一行注释**：`TreeMode` 上方的「缺省 `file`」改为不陈述缺省（缺省值只在 `architecture-canvas.tsx` 一处持有）——渲染、标记、点击、反查、展开态与全部导出语义零改动 |
| `packages/desktop/package.json` | `version` `0.7.4` → `0.7.5`（桌面源码改动，一次 patch） |

### 测试文件

| 对象 | 改动 |
|---|---|
| `packages/desktop/src-tauri/src/commands/archi/mod_test.rs` | ① 直接调用四命令的 **29 处**补 `.await`；② 触及 wrapper 的用例由 `#[test]` 改 `#[tokio::test]`（`commands/changes/mod_test.rs` 有 23 处可照抄）；③ 新增 offload 判据 helper（`std::task::Waker::noop()` + `Pin::new(&mut fut).poll(&mut cx)`，断言首个 poll 为 `Pending`）并为四命令各一条；④ 「包装与 `_inner` 逐字段一致」用例保留（加 `.await`，同参 deep-equal）；⑤ 非法 root 四形态（空白 / 缺失 / 不可读 / 非目录）文案逐字不变、结构诊断非空 `Err`、空模型 `Ok` 全部照旧 |
| `packages/desktop/src/views/architecture/components/architecture-canvas.test.tsx` | ① 语义改用例 4 条（~438 缺省布局 / ~1073 缺省视图与换面 / ~1283 三视图共用栏位 / ~1371·1385 契约视图往返）：缺省锚点由 `archi-file-tree` 改 `archi-element-tree`，触及文件树处插 `switchTo('文件')`；② 12 条用例（~700/933/962/1063/1089/1119/1177/1204/1225/1244/1273 等）插 `switchTo('文件')` 或改等待锚点；③ 新增两条：进页面 `archi_file_tree` 调用数 = 0（三种入口形态）、分段控件 `role="tab"` 声明序 = 元素 / 文件 / 契约且缺省选中「元素」 |
| `packages/desktop/src/views/architecture/components/file-tree.test.tsx` | 零改动（直测 `FileTree`，不经缺省视图） |
| `packages/desktop/src/app.test.tsx` | 零改动（其 mock 未桩 `archi_file_tree`，只断言 `archi_graph`）；可选新增一条「进架构页零 `archi_file_tree`」——D1 的主判据落在 `architecture-canvas.test.tsx`，此处不加也可 |
| `packages/desktop/src-tauri/src/bindings/mod_test.rs` | **零改动**（逐字签名、zod 表项、archi 族清单与相邻序断言应照旧通过——这是一条反证断言：改了就说明 async 化动了出线面） |

### 验收留痕（非源码，本 change 目录内）

| 对象 | 改动 |
|---|---|
| `openspec/changes/archi-file-tree-lazy-load/reports/acceptance.md` | 新建（手写、机器不消费）：验收清单六项 + 环境不具说明 + 归档后人工验收承接（AC-11）。由实现相位的收尾任务落盘（路径由此钉死，design 待决问题 1 的落盘路径随之收敛），验收相位只读核对、MUST NOT 代写；不属源码改动 |

### 不要修改

| 对象 | 理由 |
|---|---|
| `packages/desktop/src-tauri/crates/core/archi/**`（`tree.rs` / `tree_test.rs` / `dto.rs` / `drift/**` / `scan/**` / `api/**` / `lib.rs`） | 判定、投影、枚举、归属与标记语义零改动；async 化只动命令层 wrapper |
| `packages/desktop/src/views/architecture/components/file-tree.tsx`（除那一行注释） | 渲染 / 标记 / 点击 / 反查 / 展开态逐字不变 |
| `packages/desktop/src/views/architecture/hooks/**`（含 `use-archi-file-tree.ts`）、`components/element-tree.tsx`、`components/model-info.tsx`、`components/contract-panel.tsx`、`lib/**` | 取数纪律与呈现面零改动 |
| `packages/desktop/src-tauri/src/commands/mod.rs`（`all_commands!`）、`src/types/generated/**` | 命令清单与生成绑定零 diff |
| 另四条 archi 命令与 `code_stats` / `workspace_config` | 不在本纪律的本次适用面内（`code_stats` 为登记债务，归独立变更） |
| `.dev-team/architecture/**`（模型 / 契约 / INDEX） | 元素、关系与契约资产零改动（新纪律由 spec 承载，不新增模型元素） |
| `openspec/specs/**` | 除本 change 的 5 份 delta 外零 diff |

---

## 验收标准

### AC-1 · 形态（继承 explore A1 / A2）

四条取数命令均为 `pub async fn`，命令体内经 `spawn_blocking` 调 `*_inner`（源码面可断言：`spawn_blocking` 出现且 `*_inner(...)` 位于闭包内）；`*_inner` 的签名、语义与错误文案零变化（diff 只看 wrapper 与模块头注释）。

### AC-2 · 非阻塞判据（继承 explore B）

对四命令各一条用例：命令 future 的**首次 poll 为 `Pending`**（`Waker::noop()` + `poll`）。同步实现（命令体内直接调 `*_inner`）会在首次 poll 内返回 `Ready`，该用例即红。MUST NOT 以计时断言代替（时长随机器与缓存漂移）。

### AC-3 · 结果等价（继承 explore C1–C3）

① 「包装与 `_inner` 逐字段一致」用例保留（加 `.await`，同参 deep-equal）；② 非法 root 四形态（空白 / 缺失 / 不可读 / 非目录）仍 `Err` 且文案逐字不变；③ 结构诊断非空 ⇒ `archi_file_tree` 仍 `Err`、空模型仍 `Ok` 全 `none`。

### AC-4 · 出线面零 diff（继承 explore A3 / A4）

`bindings:export` 后 `src/types/generated/**` 的 `git diff` 为空；`bindings/mod_test.rs` **零改动**且四条包装的逐字签名断言、zod 表项断言、archi 八命令名单与相邻序断言全部通过；`all_commands!` 零 diff。

### AC-5 · 缺省视图的时机（继承 explore D1 / D2）

进 `/architecture`（hash 直入 / 侧栏进入 / 切走再切回）后 `archi_file_tree` 调用数 = **0**；点「文件」后恰 1 次；元素 → 文件 → 元素 → 文件往返后各再 1 次；`archi_graph` 仍每次挂载 1 次、`archi_module` / `archi_drift` 计数不因切换增加。

### AC-6 · 分段控件与栏位（继承 explore D3）

分段钮 `role="tab"` 的 DOM 声明序 = 元素 / 文件 / 契约；缺省选中「元素」（`aria-selected` / `data-active` 之一成立）；三视图共用同一 `ResizablePanel`，切换不改变栏宽、不引发画布 / 元素信息面板重取数。

### AC-7 · 失败面不回归（继承 explore D4）

切到「文件」后取数失败：只落 `archi-file-tree-error`，画布节点 / 边与元素信息面板照常渲染；`root` 变化的旧树不冒充；加载中与空树文案照旧。

### AC-8 · 语义不漂移（继承 explore E）+ 测试锚点不退化（explore R3）

`crates/core/archi/src/tree.rs` / `tree_test.rs`、`file-tree.tsx`（除一行注释）、`file-tree.test.tsx`、`use-archi-file-tree.ts` 与其测试**零 diff**。同时：凡以「缺省视图」为判据的用例 MUST NOT 只用 `queryByTestId('archi-file-tree') === null` 这类否定断言（缺省换面后它会退化成永真），必须锚 `archi-tree-mode` / `archi-element-tree` 在场或 `archi_file_tree` 调用计数，逐用例复核语义。

### AC-9 · 纪律与已知限制成文（继承 explore §3.3 三处留痕）

`desktop-app-shell` 的新纪律含：形态 / 反形态 / 首次 poll 判据 / 适用面四条 + `code_stats` 债务 / `JoinError` 前向防御 / 并发度不变 / 出线零变化，每条至少一个 scenario；`desktop-archi-drift` 明写「重复触发产生并行全仓扫描，本能力不提供取消与去重」并有 scenario；`desktop-workspace-code-stats` 的「sync 命令不在主线程执行」已纠正、零代码改动。反向判据：全仓 spec 内无「sync 命令不在主线程执行」一类陈述残留。

### AC-10 · 范围与版本纪律

`git status --porcelain` 的改动集 ⊆ 变更范围所列文件（注意只有 `commands/archi/mod.rs` 一份 Rust 源码、`architecture-canvas.tsx` 与 `file-tree.tsx`（仅一行注释）两份前端源码、一份 `package.json`）；`packages/desktop/package.json` 的 `version` 相对落地时 `HEAD` 恰 +1 patch；`src-tauri/tauri.conf.json` 仍 `"version": "../package.json"`；`src-tauri/Cargo.toml` 的 crate 版本行逐字不变；`openspec/specs/` 下除 5 份 delta 外零 diff（`desktop-archi-model` / `desktop-ipc-type-bindings` 零 diff 是判据之一）。

### AC-11 · 验收清单落盘 + 归档后人工验收（继承 explore F 的六项观测；观测本身不设门）

`openspec/changes/archi-file-tree-lazy-load/reports/acceptance.md` 在案且含下列三节，缺一节、清单缺一项即不成立（只读核对可判）：

1. **验收清单**——explore F 的六项逐条给出观测手段 / 预期 / 判据来源：① **release** 构建（`pnpm build:prod`）下进 `/architecture`：窗口可拖动 / 可切页 / 不白屏，「解析中…」可见；② 切到「文件」同样不冻结；③ 记录 `archi_graph` / `archi_drift` / `archi_file_tree` 三命令起止时刻，确认 `archi_drift` 仍是最大项、`archi_file_tree` 次之；④ debug 构建（`pnpm build`）对照一次（debug 数字不当基线）；⑤ 连点刷新 N 次的观察现象与耗时（R-1 已知限制的实证基线）；⑥ 冷 / 热缓存各一次（首次走 16 s IO，其后走 page cache）。
2. **环境不具说明**——本 change 的验收面不具备履行上述六项的观测条件：机上有 63k 文件工作区（`/d/Projects/wpsweb/client/app`）但无 release 产物；窗口拖动 / 不白屏 / 三命令起止时刻均不落任何 agent 可读产物（GUI 交互与时刻读数无读取面）。故六项观测在 agent 相位不可履行，验收评估者（只读、无 GUI）写成门必败且重做同因再败。
3. **归档后人工验收承接**——六项的观测与结论 SHALL 并入**归档后人工验收**（真实大仓库 + release 构建，人工执行），结论记入本文件末尾；若与预期不符，MUST NOT 回退本 change，按 R-1 / R-10 的口径另立变更（时长 / 并发治理）。

判据与边界：验收相位对本 AC 的核对止于「文件在案 + 三节齐备 + 清单六项可操作」；**真机观测本身 MUST NOT 作为验收门禁判据、MUST NOT 作为归档阻塞项、MUST NOT 在 `tasks.md` 登记为待办项或阻塞项**（写清单文件的收尾任务不在此列）。「不冻结」的唯一稳定机械判据是 AC-2（首次 poll 必为 `Pending`），时长不在承诺面（R-10）⇒ 真机观测对通过与否无判据增量，只有证据补强价值。同源先例：`desktop-agent-execution`「SDK 租户 MVP 边界与已知限制」第 16 条与场景「相邻 assistant 消息不设实现期核验门」——不可履行的真机核验写成门只会堵塞，确认动作并入归档后人工验收、不阻塞实现与归档。

下游产物随动：`design.md` 的「验收标准回应 · AC-11」行与待决问题 1（落盘路径）、`test-design.md` 的 AC-11 两处落点（验收范围表 / 不可测试项的验收方式）、`tasks.md` 的验收相位说明 SHALL 随本改写同步；判据 = 三处都不再把「完成六项真机观测」写成 AC-11 的通过条件，只写「清单文件在案 + 三节齐备」与「观测并入归档后人工验收」。

---

## 风险

| # | 风险 | 处置 |
|---|---|---|
| R-1 | **漂移任务堆积**（explore 已移出本变更范围）：改前同步跑主线程 ⇒ 窗口冻死 ⇒ 点不动刷新 ⇒ 天然限流；改后窗口可交互 ⇒ 连点刷新 / 反复切页（`refreshTick` 变 ⇒ `<ArchitectureCanvas key>` 整块重挂 ⇒ 三 hook 全重取；刷新钮恒可点）⇒ 每次各起一条全仓扫描，blocking 池 512 不构成保护，page cache 被反复冲刷反而更慢 | **裁决：零代码护栏**（不做取消 / 去重 / 节流），交付形态明写为「窗口不冻结，但重复触发会并行扫描」；三处留痕：`desktop-archi-drift` 已知限制条 + `desktop-app-shell` 并发度半句 + 归档后人工验收清单一行（`reports/acceptance.md` 验收清单第 ⑤ 项）。验收见内存 / IO 抖动时先查是否连点了刷新，MUST NOT 误判成 async 化回归 |
| R-2 | `spawn_blocking` 的 `JoinError` 臂只在阻塞任务 panic 时出现 ⇒ 无测试覆盖 | spec 登记为**前向防御、不可测**；MUST NOT 加 test-only 注入点（与「no test-only exports」同源） |
| R-3 | 缺省视图替换不彻底时，`queryByTestId('archi-file-tree') === null` 一类断言退化成永真 ⇒ 测试变绿但失判据 | AC-8 要求统一改锚 `archi-tree-mode` / `archi-element-tree` / 调用计数，逐用例复核语义（4 条语义用例 + 12 条插桩用例逐个过一遍） |
| R-4 | 「首次 poll 必为 `Pending`」的判据有理论竞态（阻塞任务在首次 poll 之前就完成） | 实践上不可能（首次 poll 发生在 `spawn_blocking` 返回后的同一段同步代码内），且这是**形态断言**而非性能断言；若真出现偶发，加强版为 `#[tokio::test(flavor = "current_thread")]` + interval 计 tick ≥ 1。MUST NOT 退回计时断言 |
| R-5 | UX 变更：文件视图首次打开要等 3 s（虽不冻结） | spec 与本文档明示；`focus` 与双向联动语义不变；「解析中…」态保留 |
| R-6 | 跨能力 spec 漂移：`desktop-workspace-code-stats` 写着错误事实（「sync 命令不在主线程执行」） | 本变更**同批纠正**（spec-only、零代码改动）并新增纪律；explore 的「登记待纠正」由此清偿。若评审要求留债，删除该 delta 不影响其余四份——但此时新纪律与旧陈述并存，MUST 在同一处显式登记为已知矛盾 |
| R-7 | tokio blocking 池默认上限 512 | 本次最多 3–4 并发，无实际风险；不引入配置项 |
| R-8 | `App.Backend.Core.Archi` / `App.Frontend.Views.Architecture` 无契约资产（`api = declared` 但契约缺席，`docMissing` 仅 warn） ⇒ 新纪律无契约面承载 | 纪律落 spec + 命令层用例（offload 断言即机械判据）；补契约归独立变更，本变更 MUST NOT 新建契约资产 |
| R-9 | 版本纪律 | `package.json` patch +1（`0.7.4` → `0.7.5`）；`tauri.conf.json` 经 `"../package.json"` 跟随；`src-tauri/Cargo.toml` crate 版本行**不跟** |
| R-10 | 漂移总时长未改善 | 口径写明「不承诺更快，只承诺不冻结」；drift 延迟 / 增量单独立项（explore §8） |
| R-11 | explore §3.1 的「四改三不改」只记账了 7 条命令，第 8 条 `archi_api_generate` 未表态 | **本提案补裁**：不纳入（写命令、二次确认后显式触发、低频，且 `workspace_render` 只读模型 + 契约资产，成本随模型规模而非仓库规模）。若评审要求纳入，按同形改 async，成本 +1 命令 + 若干处 `.await` |
| R-12 | `file-tree.tsx` 的「零改动」与一行注释更正冲突（explore E 写「零改动」） | 注释现写「缺省 `file`」，缺省翻面后即为错误信息；**裁决：允许改这一行**（缺省值单点留在 `architecture-canvas.tsx`），并在 AC-8 把「零 diff」的射程写成「除该行注释外」，避免 stale comment 长期误导 |
| R-13 | **AC-11 的真机观测门 agent 不可履行**：63k 工作区在机、但无 release 产物，窗口拖动与三命令起止时刻无 agent 可读面 ⇒ 该门必败，且重做验收同因再败（不是偶发失败，是判据不可履行） | **裁决：AC-11 改写为「验收清单落盘 + 环境不具说明 + 归档后人工验收承接」，真机观测 MUST NOT 作门**（见 AC-11）。先例 `desktop-agent-execution` 边界第 16 条 / 场景「相邻 assistant 消息不设实现期核验门」（`openspec/specs/desktop-agent-execution/spec.md:1015`、`:1040`）：不可履行的真机核验写成门只会堵塞实现与归档。可判半边仍在：「不冻结」由 AC-2 的首次 poll 判据机械承接，时长本就非承诺面（R-10）。AC-1 ~ AC-10 与实现面**零改动**；`reports/acceptance.md` 路径由此钉死（design 待决问题 1 收敛） |

---

## explore 未决项的归属

| explore 条目 | 处置 |
|---|---|
| §3.1「不改的三条」漏记 `archi_api_generate` | **本提案补裁为不改**，理由见 R-11；纪律的适用面以「四条取数命令 + `code_stats` 债务」为准，不含写命令 |
| §3.3 第 1 处留痕（`desktop-archi-drift` 边界段） | 采纳，落「判定单源与消费面同源」的新增条与 scenario |
| §3.3 第 2 处留痕（`desktop-app-shell` 并发度半句） | 采纳，落 ADDED 纪律的「并发度不变」条 |
| §3.3 第 3 处留痕（真机验收记录一行） | 采纳为 AC-11：六项观测清单落 `reports/acceptance.md`（R-1 实证基线即清单第 ⑤ 项），观测本身并入归档后人工验收（R-13 改写理由） |
| §五 R6「code-stats 错误句待纠正」 | **本提案同批纠正**（R-6），零代码改动 |
| §五 R8（契约资产缺口） | 不在本 change（R-8）：纪律由 spec 与 offload 用例承载，不新建契约资产 |
| §七「刷新 loading 禁用 / hook 层 in-flight 去重」 | 不采纳：涉护栏语义（点按无响应 / 刷新被吞），与 R-1 的零代码裁决同源 |
| §八 范围外四项（drift 取消去重、drift 延迟增量、文件树逐层懒加载、`code_stats` async 化） | 逐项不在本 change；`code_stats` 在纪律内**登记为债务**（唯一新增的口径留痕），其余三项不预铺 |
| §四 F 的真机验收 | AC-11：不做成自动化（时长与机器 / 缓存强相关），且 MUST NOT 作门——清单落 `reports/acceptance.md`，观测并入归档后人工验收、不阻塞归档（R-13） |
| explore 的「逐层懒加载」候选方案 | 排除，理由见「提案 · 二」末段（缺省切换后收益变窄；真做必须收窄遍历根并保 gitignore + `ExcludeSet` 语义，属独立变更） |
