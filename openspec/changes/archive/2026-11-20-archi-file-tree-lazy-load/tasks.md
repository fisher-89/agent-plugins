# 任务: archi-file-tree-lazy-load

> 覆盖 proposal「变更范围」两张表：**实现文件 4 行全量**（`commands/archi/mod.rs` / `architecture-canvas.tsx` / `file-tree.tsx`（仅一行注释）/ `packages/desktop/package.json`，源码零新增文件）+ **验收留痕 1 行**（`reports/acceptance.md`，阶段四落盘，非源码、机器不消费）。测试文件面（`commands/archi/mod_test.rs` 的 offload 判据（AC-2）+ `命令层零领域规则` 的源码面计数拆条（design D6）+ 断言口径调整 + 任何测试执行记录，`architecture-canvas.test.tsx` 的 4 条语义用例改锚 + 12 条插桩 + 2 条新增）**归后续相位**；本相位对测试文件只做**编译闭合**——为 `cargo clippy --all-targets` 能编译测试目标而补 `.await` 与 async 宿主形态（19 个用例 `#[test]` → `#[tokio::test]` + `async fn`），**不改断言口径、不记录测试执行**；AC-11 的真机观测并入归档后人工验收、**MUST NOT 登记为待办项或阻塞项**——本相位不写用例、不跑 `cargo test` / `pnpm test`、不跑 `pnpm check` 四段链（均由桌面端 static-check 与测试相位承接）。
>
> 依赖序：**阶段一**（后端命令形态）与**阶段二**（前端缺省视图换面）互不依赖、可交替；**阶段三**（版本行 + 收尾核对）必须在两者之后——AC-10 的改动集判据要一次看全 4 份文件；**阶段四**（AC-11 验收留痕落盘）与前三阶段无数据依赖，作为收尾放在最后（清单描述的是最终交付形态，含 R-1 的连点刷新基线）。
>
> 四条不可动摇的机械锚点（改错即红）：
> ① 八条 `*_inner` 与 `check_root` **逐字不动**（AC-1 / AC-3 的等价面）；
> ② 模块头仍含 `八命令` 与 `写命令两条`，且 MUST NOT 出现 `五命令` / `四命令` / `六命令` / `其余五条`（既有 stale 断言）；
> ③ `#[tauri::command]` 与 `#[specta::specta]` 各仍 **8** 处；`use` 块零改动（`Path` / `ProcessArchiveVcs` 已在案）；
> ④ 四条命令的 `.await` 与 `map_err` 之间**保留双层 Result**（闭包返回 `Result<T, String>`，`?` 出 `Result<T, String>`）——MUST NOT 改成 `.map_err(|e| e)` 一类「消双层」写法。
>
> 自查 cwd：`packages/desktop/src-tauri`（Rust 侧）；`packages/desktop`（前端 + `package.json`）；仓根（`git` 核对）。

## 阶段一：后端四条取数命令的执行契约翻面（`commands/archi/mod.rs`）

- [x] 改 `packages/desktop/src-tauri/src/commands/archi/mod.rs` 的 `archi_graph`（`:61`–`:65`）：`pub fn archi_graph(root: String) -> Result<ArchiGraph, String>` → `pub async fn archi_graph(root: String) -> Result<ArchiGraph, String>`，函数体改为
  ```rust
  tauri::async_runtime::spawn_blocking(move || archi_graph_inner(Path::new(&root)))
      .await
      .map_err(|error| format!("架构图任务失败: {error}"))?
  ```
  （`#[tauri::command]` / `#[specta::specta]` 两行注解逐字保留；`archi_graph_inner` 在 `:68` 逐字不动）
- [x] 改同文件 `archi_drift`（`:139`–`:145`）：`pub async fn`，函数体改为
  ```rust
  let vcs = ProcessArchiveVcs::new();
  tauri::async_runtime::spawn_blocking(move || archi_drift_inner(Path::new(&root), mode, &vcs))
      .await
      .map_err(|error| format!("漂移检查任务失败: {error}"))?
  ```
  （`ProcessArchiveVcs` 是 unit struct 的零 IO 装配，留 async 体后移入闭包——design 事实校准 3；`archi_drift_inner` 在 `:156` 逐字不动，`CancelFlag::never()` / 模式分流 / 错误文案全在它体内）
- [x] 改同文件 `archi_file_tree`（`:177`–`:181`）：`pub async fn`，函数体改为
  ```rust
  tauri::async_runtime::spawn_blocking(move || archi_file_tree_inner(Path::new(&root)))
      .await
      .map_err(|error| format!("文件树任务失败: {error}"))?
  ```
  （`archi_file_tree_inner` 在 `:187` 逐字不动）
- [x] 改同文件 `archi_api_check`（`:193`–`:197`）：`pub async fn`，函数体改为
  ```rust
  tauri::async_runtime::spawn_blocking(move || archi_api_check_inner(Path::new(&root)))
      .await
      .map_err(|error| format!("契约检查任务失败: {error}"))?
  ```
  （`archi_api_check_inner` 在 `:204` 逐字不动）
- [x] 同文件模块头（`:1`–`:28`）补一段执行契约注记：四条取数命令（`archi_graph` / `archi_drift` / `archi_file_tree` / `archi_api_check`）经 `tauri::async_runtime::spawn_blocking` 落阻塞池、命令体不在 IPC 线程内联；执行线程纪律与判据的唯一持有者是 `desktop-app-shell`「命令执行线程纪律」，本文件不复述判据。措辞 MUST NOT 含 `五命令` / `四命令` / `六命令` / `其余五条` 字面量（锚点 ②），故用「四条取数命令」而非「四命令」
- [x] 自查 阶段一：`grep -c "pub async fn archi_" src/commands/archi/mod.rs` = **4**；`grep -c "^pub fn archi_" src/commands/archi/mod.rs` = **12**（8 inner + 4 未动命令）；`grep -c "tauri::async_runtime::spawn_blocking" src/commands/archi/mod.rs` = **4**；四条 `spawn_blocking(move || archi_*_inner(` 各恰 1 处；`grep -c "#\[tauri::command\]"` 与 `grep -c "#\[specta::specta\]"` 仍各 **8**；`grep -n "spawn_blocking" src/commands/archi/mod.rs` 恰 4 处且全在四条 wrapper 体内（另四条命令与全部 `*_inner` 体内零命中）
- [x] 自查 阶段一：`git diff -U0 packages/desktop/src-tauri/src/commands/archi/mod.rs` 的 hunk 只覆盖四条 wrapper 与模块头（`*_inner` / `check_root` / `use` 块零 hunk）；`git diff --stat` 该文件之外的**产品 Rust 源码**零改动（`commands/mod.rs` / `bindings/mod_test.rs` / `crates/**` 全零）；`commands/archi/mod_test.rs` 只含编译闭合所需的 `.await` 与 async 宿主形态（见阶段一末条），用例与断言口径不在此列
- [x] static-check 反馈修复 · **编译闭合**：`server:check` 的 `cargo clippy --all-targets` 会编译测试目标，四条包装改 async 后测试模块的同步调用形态即编译失败 ⇒ `commands/archi/mod_test.rs` **只补编译所必需的两类改动**：① 19 个触及四条包装的用例 `#[test]` → `#[tokio::test]` + `async fn`（async 宿主形态；9 个已带 `#[allow(non_snake_case)]` 的属性并存保留）；② 直接调用处逐个补 `.await`（四个同步命令 `archi_module` / `archi_set_gate` / `archi_api_generate` / `archi_change_summary` 不补；`archi_drift` 的跨行调用一处按行长折两行）。**不改断言口径**（`命令层零领域规则` 的形态计数取值与拆条归测试相位，见移交清单）、**不记录测试执行**（`cargo test` 的用例计数与红绿由测试相位产出，本相位不记）

## 阶段二：前端缺省视图换面（`architecture-canvas.tsx` + `file-tree.tsx`）

- [x] 改 `packages/desktop/src/views/architecture/components/architecture-canvas.tsx` `:354`：`const [treeMode, setTreeMode] = useState<TreeMode>('file');` → `useState<TreeMode>('element')`（缺省值单点，MUST NOT 在 `file-tree.tsx` 或别处再声明缺省）
- [x] 同文件 `:248`–`:266` 重排 `options` 数组为 **元素 → 文件 → 契约**：`{ mode: 'element', label: '元素', content: <ElementTree … /> }` 提到首位、`{ mode: 'file', label: '文件', content: <FileTree … /> }` 次之、`contract` 保持末位；三项的 `mode` / `label` / `content` 表达式逐字不动（只换声明位置），`TabsTrigger` / `TabsContent` 仍由该数组映射（MUST NOT 在 JSX 里另写一份顺序）
- [x] 同文件 `:227` 的 `TreeColumn` doc 改为不再陈述「缺省文件树」：`/** 左栏树栏位：分段控件 + 三态行（缺省元素树，可切文件 / 契约视图）。切换只换左栏组件，\`focus\` 共用；取数失败只落本栏行内提示，画布与抽屉照常渲染。 */`
- [x] 同文件 `:336` 的 `CanvasBody` doc 改为：`/** 正常态三段布局（左栏树——缺省元素树，可切文件 / 契约视图；画布；抽屉）。漂移取数失败时零标记照常渲染 + 画布区顶部一行内提示（叠加信息不阻断图）。 */`
- [x] 改 `packages/desktop/src/views/architecture/components/file-tree.tsx` **仅 `:13` 一行注释**：`/** 左栏树视图（分段控件的三值；缺省 \`file\`）。 */` → 不再陈述缺省，例如 `/** 左栏树视图（分段控件的三值之一；缺省值由 architecture-canvas.tsx 的 treeMode 初值持有，不在此处复述）。 */`。该文件其余字节（`TreeMode` 联合、`DEFAULT_EXPAND_SEGMENTS`、渲染 / 标记 / 点击 / 反查 / 展开态、全部导出）零改动
- [x] 自查 阶段二：`grep -rn "'file'" packages/desktop/src/views/architecture/ --include=*.tsx --include=*.ts | grep -v test` 只剩 `file-tree.tsx:14` 的类型联合与 `architecture-canvas.tsx` 的 `options` 文件项（`useState` 初值零命中）；`grep -rn '缺省文件树\|缺省 \`file\`' packages/desktop/src/views/architecture/ --include=*.tsx` 在非测试源码零命中
- [x] 自查 阶段二：`git diff -U0 packages/desktop/src/views/architecture/components/file-tree.tsx` **只有 1 个 hunk、1 行**注释变化；`git diff --stat packages/desktop/src/views/architecture/` 只列 `architecture-canvas.tsx` 与 `file-tree.tsx` 两份（`hooks/**` / `element-tree.tsx` / `model-info.tsx` / `contract-panel.tsx` / `lib/**` 零改动）

## 阶段三：版本行与零扩散收尾核对（只读，不跑生成、不跑门禁）

- [x] 改 `packages/desktop/package.json`：`"version": "0.7.4"` → `"version": "0.7.5"`（单行；桌面源码改动的版本纪律，一次 patch）
- [x] 不改 `packages/desktop/src-tauri/tauri.conf.json`（`"version": "../package.json"` 逐字不变）与 `packages/desktop/src-tauri/Cargo.toml`（crate 版本行 `version = "0.1.0"` 逐字不变）
- [x] `git status --porcelain` 的**实现面**改动集 ⊆ `packages/desktop/src-tauri/src/commands/archi/mod.rs` / `packages/desktop/src/views/architecture/components/architecture-canvas.tsx` / `packages/desktop/src/views/architecture/components/file-tree.tsx` / `packages/desktop/package.json`（+ `openspec/changes/archi-file-tree-lazy-load/` 的本文档与 design / proposal / specs，属本 change 自身产物）；测试文件面只含编译闭合所需的 `commands/archi/mod_test.rs` 形态适配（`.await` 与 async 宿主形态，见阶段一末条），用例与断言口径归测试相位
- [x] `git diff --stat packages/desktop/src-tauri/crates/core/archi/ packages/desktop/src-tauri/crates/infra/ packages/desktop/src-tauri/src/commands/mod.rs packages/desktop/src-tauri/src/bindings/ packages/desktop/src-tauri/src/commands/stats/ packages/desktop/src-tauri/src/commands/config/ packages/desktop/src/types/generated/ .dev-team/ openspec/specs/` **零输出**（AC-8 / AC-10 的核心判据：领域 crate、命令清单、生成绑定、契约资产与主 spec 全零 diff）
- [x] `git diff -U0 packages/desktop/package.json` 只有 `version` 一行变化
- [x] 反向核对 AC-4 的「反证」前提：`packages/desktop/src-tauri/src/bindings/mod_test.rs` 零 diff（若因 async 化需要改它，说明出线面被动，必须回到阶段一查形态而非改测试）

## 阶段四：AC-11 验收留痕落盘（`reports/acceptance.md`，手写、机器不消费）

- [x] 新建 `openspec/changes/archi-file-tree-lazy-load/reports/acceptance.md` 第 1 节「验收清单」：explore F 六项逐条给出观测手段 / 预期 / 判据来源——① release 构建（`pnpm build:prod`）进 `/architecture`：窗口可拖动 / 可切页 / 不白屏、「解析中…」可见；② 切「文件」同样不冻结；③ `archi_graph` / `archi_drift` / `archi_file_tree` 三命令起止时刻（确认 `archi_drift` 仍最大项、`archi_file_tree` 次之）；④ debug 构建（`pnpm build`）对照一次（debug 数字不当基线）；⑤ 连点刷新 N 次的现象与耗时（R-1 已知限制的实证基线）；⑥ 冷 / 热缓存各一次（首次走 16 s IO，其后走 page cache）。**条目数 MUST NOT 增删**
- [x] 同文件第 2 节「环境不具说明」：本 change 的验收面不具备履行六项的观测条件——63k 文件工作区（`/d/Projects/wpsweb/client/app`）在机但无 release 产物；窗口拖动 / 不白屏 / 三命令起止时刻均不落任何 agent 可读产物（GUI 交互与时刻读数无读取面）⇒ 六项观测在 agent 相位不可履行，写成门必败且重做同因再败
- [x] 同文件第 3 节「归档后人工验收承接」：六项的观测与结论 SHALL 并入归档后人工验收（真实大仓库 + release 构建，人工执行），结论记入本文件末尾；若与预期不符，MUST NOT 回退本 change，按 R-1 / R-10 的口径另立变更（时长 / 并发治理）
- [x] 自查 AC-11 边界（只读）：`reports/acceptance.md` 在案 + 三节齐备 + 清单六项可操作；本 `tasks.md` 全文**零**「完成六项真机观测」一类待办项或阻塞项（真实观测并入归档后人工验收、不阻塞归档）；该文件属 change 自身产物、不进实现面改动集判据（AC-10 仍只认 4 份实现文件 + change 目录产物）

> **移交（不在本相位，不勾为任务）**：
> - **测试相位 · 后端用例面**（`commands/archi/mod_test.rs`；实现相位的范围止于编译闭合——`.await` 与 async 宿主形态，见阶段一 static-check 反馈修复条；**断言口径（含 D6 计数拆条）的复核与判定归本相位**）：① `assert_offloaded`（`Waker::noop()` + `Pin::pin!` + 单次 poll 断言 `Pending`）四命令各一条（AC-2），MUST NOT 以计时断言代替；② `命令层零领域规则` 的形态计数按 design D6 拆三条——`pub fn archi_` = 12 / `pub async fn archi_` = 4 / `tauri::async_runtime::spawn_blocking` = 4——外加四条闭包字面量断言 `spawn_blocking(move || archi_*_inner(`（该计数口径与取值属测试相位的判据，实现相位不认领）；③ 测试执行记录（`cargo test` / `pnpm test` 的用例计数、红绿与失败明细）由测试相位产出并落测试相位产物，本 `tasks.md` 不记录。
> - **测试相位 · 前端用例面**：`architecture-canvas.test.tsx`（4 条语义用例把「图就绪」信号改锚 `archi-tree-mode` / `archi-element-tree`、12 条插 `switchTo('文件')`、新增「首屏 `archi_file_tree` = 0 且点「文件」恰 1 次」与「`role="tab"` 声明序 + 缺省 `aria-selected`」两条，MUST NOT 以 `queryByTestId('archi-file-tree') === null` 否定断言作缺省判据）；`app.test.tsx`（建议补三入口零调用一条，design 待决问题 2）；`file-tree.test.tsx` 与 `bindings/mod_test.rs` 零改动。
> - 验收相位：**只读核对 AC-11**——`reports/acceptance.md` 在案 + 三节齐备 + 清单六项可操作（MUST NOT 代写；MUST NOT 要求真机观测作为通过条件）。六项观测并入**归档后人工验收**（真实大仓库 + release 构建，人工执行），结论记该文件末尾；与预期不符时按 R-1 / R-10 另立变更，不阻塞归档。「不冻结」的机械判据是 AC-2 的首次 poll 断言。
> - 桌面端 static-check：`server:check` → `bindings:export`（应幂等、`git diff` 为空）→ `api:check` → `client:check` 四段链由工作流执行。
