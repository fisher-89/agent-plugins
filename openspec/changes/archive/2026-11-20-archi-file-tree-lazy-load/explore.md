# 架构页：命令不冻结 + 文件树不参与首屏

workflow_type: requirement

## 结论

两条动作，各自对应一层根因，互不可替代：

| # | 动作 | 治哪层 |
|---|---|---|
| ① | `archi_graph` / `archi_drift` / `archi_file_tree` / `archi_api_check` 四条命令改 `pub async fn` + `spawn_blocking` | 执行契约：命令体不再占 IPC 线程（Windows = 主线程） |
| ② | 左栏缺省改元素树，分段控件重排为 **元素 / 文件 / 契约** | 取数策略：文件树不再无条件参与首屏 |

**明确不承诺**：不缩短 `archi_drift` 的总时长；不提供取消与去重（重复触发会并行扫描，登记为已知限制）。

## 一、问题陈述

**症状**：在 63k 文件的工作区上进入 `/architecture`，窗口不可拖动、不可切换、不重绘，持续数十秒。表现上像「文件树解析慢」——因为左栏恰好停在「解析中…」。

**根因有两层，缺一不可**：

① **执行契约层**：archi 的四条取数命令是同步命令（Tauri `ExecutionContext::Blocking`），命令体跑在 IPC 线程（Windows = 主线程）上。任何「批量读子系统」的工作都会冻结 webview，**与具体是文件树还是漂移无关**——这解释了为什么症状总落在最后一个还没回的命令上。

② **取数策略层**：左栏缺省即文件树，`FileTree` 挂载即 invoke 一次全仓投影（实测 42,847 文件 → 59,771 节点 → 约 10.4 MB → 3.1 s walk），且每次从「元素 / 契约」切回「文件」都重来一遍；而它只是第二个排队者——真正的量在 `archi_drift`（16 s IO + 42k 文件 AST 解析）。

**只做一半的后果**：只修 ① → 不冻结，但首屏仍付 3.1 s + 10.4 MB；只修 ② → 首屏省一次 walk，但 drift 照样冻结主线程。

**措辞禁忌（避免评审跑偏）**：

| 不要写 | 因为 |
|---|---|
| 「文件树性能优化」 | 会把焦点引向组件与 DTO，掩盖「所有同步命令都这样」（`code_stats` 即同病） |
| 「首屏加速 X%」 | 本次不承诺时长；大工程时长主要取决于 drift |
| 「加了懒加载」 | 本次无逐层懒加载，只有「缺省不取」 |
| 「修复冻结 bug」 | 冻结是契约缺口的必然结果，写成 bug 会掩盖纪律缺席 |

## 二、实测证据

环境：16 逻辑核；工作区 `wpsweb/client/app`（63,023 个 git 跟踪文件）。

| 指标 | 实测 |
|---|---|
| 源扩展名文件（ts/tsx/js/jsx/rs） | 46,352 |
| 过默认排除矩阵 + 架构域 `exclude`（`openspec/**`、`3rdparty/**`）后进树 | **42,847** |
| 文件树节点总数（文件 + 合成目录） | **59,771** |
| 默认展开（路径段数 ≤ 3）即渲染的行数 | **684** |
| 全量 payload（JSON 未压缩，单条约 182 B） | **约 10.4 MB** |
| 目录遍历耗时（同等剪枝） | **3,146 ms** |
| 全量读源文件耗时（约等于 drift 的 IO 段） | **16,336 ms**（492 MB / 10.2M 行） |

**推论**：

- `archi_drift` 是绝对大头：16 s IO + oxc 全量解析 42k 个 TS/JS（`scan/ts.rs` 用 AST，非正则）+ Rust resolver，粗估 25–40 s（release）。全程主线程。
- `archi_file_tree` 排在其后：进页面时 drift 未完成，同步命令同线程排队，于是「文件树解析中…」= 剩余 drift 时长 + 自身 3 s + 10 MB 序列化。
- 前端 DOM 非主因：默认展开只有 684 行。

**关于「CPU 不足 5%」**：16 逻辑核下单线程满载 = 6.25%，而 walk / 读盘是 IO 等待（不计 CPU）。低 CPU 恰是「IO 密集 + 单线程」的指纹，不是「没那么忙」。

## 三、改动方案

### 3.1 后端：四条命令 async 化

Tauri 宏的判定（`tauri-macros-2.7.4/src/command/wrapper.rs`）：

```rust
ExecutionContext::Async if function.sig.asyncness.is_none() => "sync_threadpool",
ExecutionContext::Async => "async",
ExecutionContext::Blocking => "sync",
```

| 形态 | 调度 | 测试改动 |
|---|---|---|
| `#[tauri::command(async)]` + 保持 `pub fn` | tokio **worker** 池 | 零 |
| `pub async fn` + `spawn_blocking`（选此） | tokio **blocking** 池 | 29 处 `.await` |
| 现状 `pub fn` | IPC 线程内联 | — |

选第二种的理由：worker 池 = 核数（本机 16），三条命令并发即占 3 个 worker，且与 agent 流式任务抢同一池；blocking 池才是为阻塞 IO 设计的。形态与 `create_change` / `promote_explore` / `preclose` 一致（`create_change` 已是 `#[tauri::command] #[specta::specta] pub async fn`，in-repo 证明 async + specta 可用）。

```rust
#[tauri::command]
#[specta::specta]
pub async fn archi_file_tree(root: String) -> Result<ArchiFileTree, String> {
    tauri::async_runtime::spawn_blocking(move || archi_file_tree_inner(Path::new(&root)))
        .await
        .map_err(|error| format!("文件树任务失败：{error}"))?
}
```

- `*_inner(&Path)` 签名与语义**零改动**。
- **绑定产物零变化**：`specta-macros` 显式处理 asyncness（`function_asyncness`），生成仍是 `archiFileTree: (root: string) => __TAURI_INVOKE<ArchiFileTree>(...)`；`bindings/mod_test.rs` 的逐字签名断言照旧通过。
- 命令清单（`all_commands!`）零 diff。

**不改的三条**：`archi_module`（点击级，单模型解析）、`archi_set_gate`（唯一写命令，毫秒级）、`archi_change_summary`（带 `State<WorkspaceStores>`，改 async 需先 `for_root` 取 `Arc<Store>` 再移入闭包，且低频）。

### 3.2 前端：缺省元素树

```diff
- const [treeMode, setTreeMode] = useState<TreeMode>('file');
+ const [treeMode, setTreeMode] = useState<TreeMode>('element');
```

分段控件重排为 **元素 / 文件 / 契约**（缺省值即首项；现有测试未断言顺序）。

| 面 | 变更 |
|---|---|
| 分段按钮顺序 | 元素 / 文件 / 契约 |
| 首屏 `archi_file_tree` 调用数 | 0（点「文件」后才 1 次） |
| `architecture-canvas.test.tsx` | 16 个用例触及，见第六节 |
| `app.test.tsx` | 零改动（只断言 `archi_graph`），可新增一条「进页面零 `archi_file_tree`」 |

收益：省 3 s walk + 10 MB payload；文件树取数与 drift 解耦——不点「文件」，这笔成本完全不发生。

### 3.3 新增纪律与已知限制留痕

**纪律**（拟落 `desktop-app-shell`「Tauri command 轨道组织」下）：

> 批量子系统 IO 的命令（整仓枚举 / 全文件读 / 全模型解析）MUST 为 `async fn` + `spawn_blocking`，MUST NOT 以同步命令形态注册。同步 = `ExecutionContext::Blocking` = IPC 线程内联 = 主线程冻结。
> 判据：对命令 future 首次 poll 必为 `Pending`。
> 当前适用面：`archi_graph` / `archi_drift` / `archi_file_tree` / `archi_api_check`。
> 附带：async 化不改变**并发度**语义——无取消的命令在重复触发下会真正并行。

**三处已知限制留痕（纯文字，零代码）**：

1. `desktop-archi-drift` 边界段：「命令面 `CancelFlag::never()`，重复触发（连点刷新 / 反复切页）会产生并发全仓扫描；本能力不提供取消与去重。」
2. `desktop-app-shell` 新纪律内附「并发度」那半句（见上）。
3. 真机验收记录留一行：连点刷新 N 次的可观察现象与耗时。

## 四、验收判据

### A. 形态与产物（机械可判）

| # | 判据 |
|---|---|
| A1 | 四条命令均为 `pub async fn`，体内经 `spawn_blocking` 调 `*_inner`（可加源码面断言：`spawn_blocking` 出现且 `*_inner` 位于闭包内） |
| A2 | `*_inner` 签名 / 语义零变化（diff 只看 wrapper） |
| A3 | `bindings:export` 后 `git diff` 为空；`bindings/mod_test.rs` 四条包装逐字断言照旧通过 |
| A4 | `all_commands!` 零 diff；archi 八命令名单与相邻序断言照旧通过 |

### B. 非阻塞（唯一稳定判据，不用计时）

```rust
fn assert_offloaded<F: Future>(mut fut: F) {
    let mut cx = Context::from_waker(Waker::noop());
    assert!(matches!(Pin::new(&mut fut).poll(&mut cx), Poll::Pending));
}
```

同步实现首次 poll 内跑完返回 `Ready`；`spawn_blocking` 实现必然 `Pending`。加强版（可选）：`#[tokio::test(flavor = "current_thread")]` 内以 interval 计 tick，断言命令完成前 tick ≥ 1。

### C. 结果等价

| # | 判据 |
|---|---|
| C1 | 「包装与 `_inner` 逐字段一致」用例保留（加 `.await`），同参下 deep-equal |
| C2 | 非法 root 四形态（空白 / 缺失 / 不可读 / 非目录）仍 `Err`，文案逐字不变 |
| C3 | 结构诊断非空 → `archi_file_tree` 仍 `Err`；空模型仍 `Ok` 全 `none` |

### D. 缺省视图的时机与交互

| # | 判据 |
|---|---|
| D1 | 进 `/architecture`（hash 直入 / 侧栏进入 / 切走再切回）后 `archi_file_tree` 调用数 = **0**；`archi_graph` 仍 = 1/次挂载 |
| D2 | 点「文件」后 `archi_file_tree` 恰 1 次；元素→文件往返再各 1 次（非活动面板不挂载语义不变） |
| D3 | 分段控件 DOM 序 = 元素 / 文件 / 契约；缺省选中「元素」；三视图共用同一 `ResizablePanel` |
| D4 | 切到「文件」后取数失败：只落 `archi-file-tree-error`，画布与抽屉照常 |

### E. 语义不漂移（本次不动的面）

- `views/architecture/components/file-tree.tsx` 与 `file-tree.test.tsx` 零改动
- `crates/core/archi/src/tree.rs` 与 `tree_test.rs` 零改动
- `hooks/use-archi-file-tree.ts` 与其测试零改动

### F. 真机验收（非自动化，写进验收记录）

在 63k 文件工作区用 **release** 构建（`pnpm build:prod`）：

1. 进 `/architecture` 期间窗口可拖动、可滚动、可切页；「解析中…」可见而非白屏。
2. 切到「文件」时同样不冻结。
3. 记录三命令起止时刻，确认 `archi_drift` 仍是最大项、`archi_file_tree` 次之。
4. 对照 debug 构建（`pnpm build`）记一次，作为「debug 数字不可当基线」的实证。
5. 连点刷新 N 次，记录现象与耗时（已知限制的实证基线）。

冷启动 / 热缓存各测一次：首次走 16 s IO，其后走 page cache，差一个量级。

## 五、风险与已知限制

| # | 风险 | 影响 | 处置 |
|---|---|---|---|
| R1 | **漂移任务堆积**（已移出本次范围） | 改后并发扫描真正并行（见下） | 登记为已知限制，零代码护栏 |
| R2 | `spawn_blocking` 的 `JoinError` 臂只在任务 panic 时出现 | 该分支无测试覆盖 | spec 登记「前向防御、不可测」；不加 test-only 注入点 |
| R3 | 缺省视图替换不彻底时，`queryByTestId(...)===null` 断言退化成永真 | 测试变绿但失判据 | 统一改锚 `archi-tree-mode` 或 `archi-canvas`，逐用例复核语义 |
| R4 | `archi_graph` 本就毫秒级，async 化多一次调度 | 可能一次多余渲染 | 无需处理 |
| R5 | UX 变更：文件视图首次打开要等 3 s（虽不冻结） | 需适应 | spec 明示；`focus` 与双向联动语义不变 |
| R6 | 跨能力 spec 漂移：`desktop-workspace-code-stats` 写着「sync 命令不在主线程执行」（错误）；`desktop-app-shell` 无执行线程纪律 | 后人误判「同步命令安全」 | 本次新增纪律 Requirement，并把 code-stats 错误句登记待纠正（代码不改） |
| R7 | tokio blocking 池默认上限 512 | 本次最多 3–4 并发，无实际风险 | 无需处理 |
| R8 | `App.Backend.Core.Archi` / `App.Frontend.Views.Architecture` 无契约资产（`docMissing` 仅 warn） | 新纪律无契约面承载 | 纪律落 spec + 命令层用例；是否补契约另议 |
| R9 | 版本纪律 | — | `package.json` patch +1；`tauri.conf.json` 跟随；`src-tauri/Cargo.toml` crate 版本行不跟 |
| R10 | 漂移总时长未改善 | 仍「慢」，只是不再「卡」 | 口径写明「不承诺更快，只承诺不冻结」；drift 延迟 / 增量单独立项 |

### R1 的连带影响（必须显式接受）

```
改动前：drift 同步跑主线程 → 窗口冻死 → 点不动刷新 → 天然限流（最多 1 条）
改动后：drift 跑 blocking 池 → 窗口可交互 → 连点刷新 → 每次重挂各起一条全仓扫描
```

已确认的触发路径：

- `refreshTick` 一变 → `<ArchitectureCanvas key={refreshTick}>` 整块重挂 → 三个 hook 全部重取；
- 刷新按钮恒可点（`ArchitectureView` 未消费任何 `loading`，`useArchiGraph` 暴露了但没人用）；
- 命令面 `CancelFlag::never()`，扫描循环的取消轮询永不命中。

量级：单次 = 16 s IO + 42k 文件解析；连点 5 次约等于 5 条并发扫同一批文件，blocking 池 512 **不构成保护**，page cache 被反复冲刷，反而更慢。

**裁决：零代码护栏**——不做取消、不做去重、不做节流，仅在 3.3 的三处留痕。交付形态 = 「窗口不冻结，但重复触发会并行扫描」。验收时若见内存 / IO 抖动，先查是否连点了刷新，不要误判成 async 化回归。

## 六、影响面清单

### 后端

```
commands/archi/mod.rs          四条 wrapper 改 async（*_inner 不动）
commands/mod.rs                all_commands! 零改动
src/bindings/mod_test.rs       零改动（断言照旧通过）
```

**直接调用点计数**（`commands/archi/mod_test.rs`）：file_tree 11 / api_check 9 / graph 6 / drift 3 = **29 处需 `.await`**。宿主测试按是否触及 wrapper 决定是否改 `#[tokio::test]`（该文件现有 50 个 `#[test]`、0 个 `#[tokio::test]`；`commands/changes/mod_test.rs` 有 23 个 `#[tokio::test]` 可照抄）。

### 前端测试改动清单（`architecture-canvas.test.tsx`，16 个用例）

```
需改语义（4）
  438  应渲染三段布局并以首个顶层元素为聚焦（左栏缺省为文件树）
  1073 应缺省渲文件树且切到元素树时 testid 换面
  1283 应让左栏三视图共用同一可调宽栏位（以 archi-file-tree 出现作挂载信号）
  1371/1385 契约视图 / 三视图往返（同上）
需插入 switchTo('文件')（12）
  713 933 962 1063 1089 1119 1177 1204 1225 1244 1273
```

### spec 触点

```
desktop-archi-file-tree       「缺省文件树」+ 3 个场景 → 改为「缺省元素树」
desktop-archi-canvas          左栏缺省段与挂载描述一句话
desktop-app-shell             新增执行线程纪律 Requirement（含并发度说明）
desktop-archi-drift           边界段新增并发扫描已知限制
desktop-workspace-code-stats  「sync 命令不在主线程执行」错误陈述待纠正（登记）
```

### 版本

`packages/desktop/package.json` patch +1；`src-tauri/tauri.conf.json` 经 `"../package.json"` 跟随；`src-tauri/Cargo.toml` crate 版本行**不跟**。

## 七、已排除方案与已澄清的错误假设

### 排除的方案

| 方案 | 排除理由 |
|---|---|
| 命令改 `#[tauri::command(async)]` 保持 `pub fn` | 落到 tokio worker 池（= 核数），与 agent 流式任务抢同一池；blocking 池才是为阻塞 IO 设计 |
| 只改缺省视图、不做 async | 首屏省一次 walk，但 drift 仍冻主线程数十秒 |
| 只做 async、不改缺省视图 | 不冻结，但首屏仍付 3.1 s + 10.4 MB，且每次切回「文件」重来 |
| 逐层懒加载（每层一取 + `hasChildren`） | 首屏已被缺省视图清零，收益只在「点开文件视图之后」；且每层若仍调全仓 `walk_source_files` 会变成每层 3 s，比现状更慢（分层必须收窄遍历根）。DTO 与 spec 大改，本次不值得 |
| 深度截断（沿 `code_stats(depth)` 先例） | 同上，且顶层节点在 63k 仓库仍可能上千；同样需要 `hasChildren` 与聚焦反查补取 |
| 刷新按钮 loading 禁用 / hook 层 in-flight 去重 | 均涉护栏语义（点按无响应或刷新被吞），本次裁决为不做 |

### 已澄清的错误假设

| 假设 | 结论 |
|---|---|
| 「瓶颈是 Vite / node 进程」 | dev 专属成本。安装版前端是 `dist/` 静态资源，不存在 node；实测安装版 CPU < 5%，与「单线程 + IO 等待」自洽 |
| 「默认全量渲染导致卡」 | 否。默认只展开 3 段，实测可见行 684；DOM 非主因 |
| 「文件树是主犯」 | 它只是第二个排队者。真正的量在 `archi_drift`（16 s IO + 42k AST 解析） |
| 「`code_stats` 是 async 先例」 | 反了：它同样是同步命令 + tokei 全仓解析，`/info` 页同病；本次不动其代码，仅登记 spec 陈述错误 |

## 八、范围外（另立项）

| 议题 | 说明 |
|---|---|
| `archi_drift` 取消 / 去重 / 节流 | 用户单独分析；基础设施在 `foundation::cancel`，但涉及命令面 + 领域 + 测试 |
| drift 延迟 / 增量触发（首屏不跑全量） | 治「总时长」而非「冻结」；本次口径明确不承诺更快 |
| 文件树逐层懒加载 / 深度截断 | 缺省视图调整后收益变窄；若做，必须收窄遍历根（`WalkBuilder::new(root.join(parent))`）并保持 gitignore + `ExcludeSet` 语义 |
| `code_stats` async 化 | 与本次同病，独立立项更清晰 |
