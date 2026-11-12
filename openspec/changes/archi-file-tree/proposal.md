# 提案: archi-file-tree

> **变更**: archi-file-tree
> **日期**: 2026-11-11
> **状态**: draft
> **来源**: `openspec/changes/archi-file-tree/explore.md`（requirement 类型，结论稿 + §12 启动前复核 + §13/§14 两项裁决）
> **性质**: `/architecture` 三段布局最左栏的导航面重构——FQN 元素树改为**仓库文件树**（磁盘枚举 + 元素根归属标记），点击仍走同一次 `onFocus`。画布、抽屉、漂移标记、门禁、写入面零改动。另含两项判定面修正：生成物归属判据下沉 `OwnerIndex`（A2″ 并集）与架构域配置新增 `exclude` 一条。

---

## 问题

### 现状链路（读过的真实代码）

```
src/views/architecture/architecture-view.tsx
  └─ ArchitectureCanvas(root, mode)            components/architecture-canvas.tsx
       ├─ useArchiGraph(root)                  hooks/use-archi-graph.ts       → archi_graph
       ├─ useArchiDrift(root, mode)            hooks/use-archi-drift.ts       → archi_drift
       └─ CanvasBody  ── 三段布局
            ├─ ElementTree(elements, focused, onFocus)   components/element-tree.tsx（w-72）
            ├─ ReactFlow 1-hop 邻域
            └─ ElementDrawer
```

左栏今天的唯一形态是**元素 FQN 前缀树**：层级由 FQN 派生、节点只呈短名、点击 = `setFocusOverride(fqn)`。它回答「模型里有哪些元素」，但回答不了「这段代码归哪个模块 / 哪些目录不是模块」。

### 五个具体症状

1. **导航面与判定面不对齐**：树是模型投影，磁盘上的真实目录结构不在其中；想知道「`crates/infra/archi/src/scan` 归谁」必须逐个元素看抽屉的 `path`。
2. **元素根不可见**：42 个 `path` 非空元素（45 条 `path` 条目）在树里只是 FQN 叶子，无法回答「模块边界落在哪个目录 / 文件上」。
3. **容器元素必然占据树位却零文件**：9 个容器元素（8 `domain` + 1 `package`）`path` 恒空（`decisions/0001-model-granularity.md` §3 的直接后果），在文件语义上无从落地。
4. **生成物归属是错的（判定面缺陷，不是树的问题）**：`scan/owner.rs::strip_generated` 只做「替换扩展名」（`archi.i.rs` → `archi.rs`），而生成器 `api/paths.rs::generated_path` 产出的是**元素根同级**（`crates/infra/archi.i.rs`）——两种形态互不相交，45 条 `path` 条目各自的 `<root>.i.<ext>` 实际是 **0 归位 / 45 无主**。`owner_test.rs:437` 还把这条 bug 写成了正例契约（`src/dir.i.ts` 期望 `Owner::None`），`desktop-archi-api` 的「四种剥离形态各归其元素」Scenario 按现行判据四条全为 `None`（假金句）。
5. **架构域配置无 `exclude` 落点，`openspec/` 三个原型 tsx 会进任何源码枚举面**：`git ls-files` ∩ 五扩展名 ∩ `openspec/` 实测 3 个未被 gitignore 的 `.tsx`（`openspec/changes/archive/2026-09-28-agent-component-ai-sdk/prototypes/*.tsx`），它们既非隐藏也未被 gitignore，`walk_source_files` 照收。

### 关键探明事实（压缩自 explore，已按 §12 复核订正）

- **命令面现状是 5 条**（`all_commands!` 的 archi 段实测）：`archi_graph` / `archi_module` / `archi_change_summary` / `archi_set_gate` / `archi_drift`。本变更是 **5 → 6**。
- **`desktop-archi-model` 对同一事实有两种自称**：标题与 Module Contract 写「四命令」、自己的 Scenario 又写「命令总数仍为五个」——本次统一校正为六。
- **枚举面 = 漂移扫描面**（用户裁决 D5′）：树套用完整排除矩阵（默认 13 条 + 架构域配置 `exclude`），`ExcludeSet::build(extra)` 既有入口直接可用，`scan/exclude.rs` 零改动。代价是 306 个测试文件在树上不可见（本仓源文件 667 → 进树 358）。
- **元素根计数须分两种口径**：元素 51 = 42（`path` 非空）+ 9（`path` 空，容器）；`path` 条目 45 = 29 目录形 + 16 文件形；`elementRoot` **节点** 45（`App.Frontend.Shell` 一个元素占 4 个文件节点）、`elementRoot` **元素** 42。
- **`ambiguous` 本仓恒 0**（结构上不存在同长重复前缀），但必须实现并测——不实现就退化成「任选其一」，与判定单源纪律冲突。
- **`exists = false` 无真实样本**：45 条 `path` 磁盘全部存在，故该分支只能经合成 tempdir 测，spec 须写明它是前向防御分支。

---

## 提案

### 一、左栏：文件树（默认）+ 元素树（分段切换）

```
CanvasBody（同一栏位，两种视图二选一）
[ 左栏 w-80（文件树）/ w-72（元素树）]  [ xyflow 画布 flex-1 ]  [ 抽屉 w-72 ]
   ↑ 栏内顶部一个分段控件（data-testid="archi-tree-mode"，两钮 data-mode="file" | "element"）
     缺省 file；文件树栏宽 80（文件名比 FQN 末段长），元素树沿用 72
```

- 文件树节点来源 = **磁盘枚举 + 元素根标记**，不是模型投影；元素树组件 `element-tree.tsx` 原样保留（MUST NOT 删除），它在容器元素可见性与「谁依赖谁」的场景仍更顺。
- 点击语义**完全沿用**现有一层：`elementRoot` / `member` 行点击 = `setFocusOverride(owners[0])`；`ambiguous` / `containsElements` / `none` 行点击**只切展开、零 `onFocus`**（本仓 `ambiguous` 恒 0，为一个只可能出现在合成语料里的分支新增 popover 原语不划算；`owners` 已在 DTO 里存全量候选，将来放开只改点击处理）。
- 画布 / 抽屉 → 树：`focus` 变化时以「条目 `owners` 是否含该 FQN」定位（多 `path` 元素会有多条命中），展开祖先链、滚到第一条、多节点同时高亮；**零命中**时树顶部一行弱提示（文案不断言成因——容器 `path` 恒空与「源文件全在判定面之外」两种情况都表现为零命中），不滚动。

### 二、数据面：一条新只读命令 + 扁平条目 DTO

```
archi_file_tree(root: String) -> Result<ArchiFileTree, String>
  └─ archi_file_tree_inner(root: &Path)          ← 离 Tauri 可测（沿既有五个 *_inner 形态）

ArchiFileTree  { entries: Vec<ArchiFileEntry> }   // 扁平，path 字节序，确定性；不产 "" 根条目
ArchiFileEntry { path, name, kind: FileEntryKind, mark: FileTreeMark,
                 owners: Vec<String>, contains: u32, exists: bool }
```

- **扁平而非嵌套**（D2）：golden 可直接断言、serde / specta 零递归、前端本来就要按 `path` 做展开态与滚动定位。
- **结构诊断非空 → `Err`**（与 `archi_module` 同口径）。停摆态下画布整块被 `StructureErrorPanel` 替换，树本就不挂载。**空模型不属此列**：模型目录缺席或零元素时 SHALL 正常返回（全条目 `mark = none`）——冷启动空模型是合法起点，与「模型有结构错」不同类。
- **新词表 `FileTreeMark` 五值，不复用 `NodeMark`**（D3）：`NodeMark` 归画布节点、生命周期在漂移报告；同名不同义会让两处语义相互污染。`FileTreeMark` 与 `FileEntryKind` 均带 `ALL` + `as_str` 逐值对拍；**不进「四面词汇对拍」**（那四面锚定模型 / 报告词汇，另一条生命周期）。

### 三、归属判定：单源下沉 `OwnerIndex`，树只消费

判定链（逐节点）：

```
① module_roots: HashMap<norm_path, Vec<fqn>>   由所有 path 非空元素展开（归一 = OwnerIndex::normalize）
② module_roots 命中：恰 1 个 FQN → elementRoot, owners = [该 FQN]
                     ≥ 2 个 FQN（同路径多元素声明）→ ambiguous, owners = 全部 FQN
   else owner_of(p):
       Element { fqn }         → member,      owners = [fqn]
       Ambiguous(fqns)         → ambiguous,   owners = fqns
       None                    → contains > 0 ? containsElements : none
③ contains(p) = module_roots 中 path 严格以 "p/" 开头的元素数（按 FQN 去重，不含自身）
```

- **归属在算后端、前端零重算**（D1）：「判定单源」纪律（drift / canvas 两条 spec 明令），且 `OwnerIndex` 已承载 `./` 归一、尾 `/` 剥离、生成物剥离、同长多命中 `Ambiguous` 四个分支，前端重写必漏。
- **`elementRoot` 判据是 `path` 非空而非 `kind`**（D4）：`App.Frontend.Components.Agent` 是 `domain` 却持有 barrel `index.tsx`，它是元素根。
- **`contains` 与 `mark` 正交**：所以「属于 X · 含 1 个模块」可同时呈现（嵌套元素的父目录）。

### 四、判定面修正 A2″：`strip_generated` 改为「整组剥离 ∪ 替换扩展名」

两种生成物落点形态**必须并集**，单条规则覆盖不了：

```
① 根同级（generated_path 的实形态）  <root>.i.<ext>            → 需「整组剥离」  archi.i.rs → archi
② 目录内（历史 / 假设形态）          <dir>/<name>.i.<ext>      → 需「替换扩展名」 archi.i.rs → archi.rs
```

- 实现形态：`strip_generated` 改为返回 1~2 个候选（`strip_candidates`），`owner_of` **对每个候选各跑一遍最长前缀、合并命中集再按长度定档**（同长仍归 `Ambiguous`）。**MUST NOT 写成「候选 1 命中就短路」**——那会让 `src/dir.i.ts`（整组剥离得 `src/dir`、替换扩展名得 `src/dir.ts`）只走一条路径，与并集语义不符；用例以「更具体的单文件根胜出」钉住。
- 回归面实测：纯整组剥离会打翻 4 条既有断言（同扩展名单文件根 `a.ts` / `my.mod.rs` / 反斜杠形态），并集只翻转 1 条——恰是把 bug 写成契约的 `src/dir.i.ts`（`None` → `Element`），翻转即修复。
- **对漂移报告本仓零变化**（**保持排除配置不变**为前提——`exclude` 是同一 change 内的第二个独立变量）：唯一生成物 `crates/infra/archi.i.rs` 所属元素 `gate = off`，`Owner::None` 与 `Owner::Element{off}` 同落 `unmatchedFiles`，`matched[]` 只在 `gate != off` 时写入；报告逐字段、逐序不变。推广面（`gate != off` 的已声明元素，其生成物由 `unmatchedFiles` 移入 `matched[]`）属判定面语义扩张，须在 `desktop-archi-api` 与 `desktop-archi-drift` 两处同时写清，MUST NOT 只在树侧实现。
- **`owner_of` 的目标侧同样受益**（`scan/mod.rs` 对 `target_rel` 求归属）：生成物被 import 时也能作为合法 `to`；本仓生成物无人 import，故同样零变化。

### 五、枚举面：完整排除矩阵 + 架构域配置新增 `exclude`

```
archi_file_tree_inner(root)
  ① 读模型（结构诊断非空 → Err）
  ② drift::config::load_config(root, known)     ← 只取 exclude，ignore 不参与
  ③ ExcludeSet::build(&config.exclude)          ← 既有入口，默认 13 条 + config（D5′，零新增入口）
  ④ scan::walk_source_files(root) + is_source_file(rust …)   ← 两函数提为 pub(crate)
  ⑤ 对每个候选文件：matched(rel).is_some() → 丢弃
  ⑥ 每个文件与每个 module_root 逐级上溯合成目录；module_root 无文件也出节点（exists=false）
```

- **`ExcludeSet` / `scan/exclude.rs` 零改动**：默认 13 条在前 + config 追加在后、`matched()` 最小下标优先，正是要的语义。原计划的 `build_config_only(extra)` 作废。
- **本仓新增 `.dev-team/architecture/config.json`**：`{ "exclude": ["openspec/**"] }`，**`$schema` 一律不得添加**——`drift/config.rs` 的 `RawConfig` 带 `#[serde(deny_unknown_fields)]` 且只登记 `ignore` / `exclude` 两键，写 `$schema` 会让 `load_config` 直接 `Err`，同时打断 drift 命令、门禁 runner 与本次新增的树三条消费面。与工作区 `.dev-team/config.json`（带 `$schema`）形态不同，两者不可类比，故本仓该文件不带 `$schema`。
- **连带效应如实登记**：drift 扫描面随之变化——`openspec/changes/archive/…/prototypes/{index,agent-input,agent-messages}.tsx` 三个文件由 `unmatchedFiles +3` 变为 `excluded +3 / unmatchedFiles −3`。已核：无既有测试对真仓 drift 计数断言（golden 走合成语料、perf 走合成夹具、pilot 只读 api 资产）。
- **配置非法 → 树 `Err` 进错误态、不阻断画布**（与漂移取数失败同口径）。

### 六、规模与呈现

- 实测（D5′ + B 定稿后）：进树文件 **358**、合成目录 **117**、补出根 **0** → 树节点 **475**、紧凑 JSON **84.2 KB**；`mark` 分布 `elementRoot 45 / member 373 / containsElements 12 / none 45 / ambiguous 0`。体量降到「无需懒加载」量级；仍超阈值则改 `archi_file_tree(root, dir)` 单层 + 前端缓存（DTO 形状不变）。
- 42 个元素根**全部**仍有可见文件（最少 1 个），「屏蔽测试面」不会让任何模块在树上变成空壳。
- 呈现：`elementRoot` 实心圆点 + 主色 + 加粗；`ambiguous` 警示色圆点；`member` 左侧竖线 + 次级色 + 弱化；`containsElements` 空心圆 + 尾随小字 `·N`；`none` 无标记。树底部常驻一行可折叠图例：**仅显示判定面源文件**（测试文件与架构域配置 `exclude` 命中项不在树中；`none` 行 = 无元素认领）。
- 展开态沿 `views/info/components/dir-tree.tsx` 既有模式（per-path 翻转集合，默认展开前 2 段，刷新后按 path 保持）；**折叠不渲染 DOM**。MUST NOT 与该件抽公共抽象（数据面不同：marks / owners vs 行数聚合）。

### 关键决策与被拒备选

| # | 决策 | 理由 | 被拒备选（为何拒） |
|---|---|---|---|
| D1 | 归属在后端算，前端零重算 | 「判定单源」纪律；`OwnerIndex` 四分支已实现 | **前端按 `graph.elements[].path` 自算**：必漏 `./` 归一 / 尾 `/` / 生成物剥离 / `Ambiguous`，与 drift 立刻分叉 |
| D1′ | 剥离判据下沉 `OwnerIndex`，取并集（A2″） | 树与 drift 必须对同一文件给出同一归属 | **A1 接受无主**（本轮可接受，但契约推到全仓后树上有 42 个灰点）；**A2 纯整组**（打翻 4 条既有断言）；**A3 树侧加规则**（撞 D1）；**A4 树里过滤 `.i.*`**（枚举面变成两条互不相干的判据） |
| D2 | 命令返回扁平条目数组 | golden 可断言、serde / specta 零递归、前端本就要按 path 展开 | 嵌套 DTO（递归 + 前端仍要拍平） |
| D3 | 新词表 `FileTreeMark` 五值 | 与 `NodeMark` 生命周期不同，同名不同义会互相污染 | 复用 `NodeMark`（五值 vs 两值，语义打架） |
| D4 | `elementRoot` 判据是 `path` 非空，不是 `kind` | `Components.Agent` 是 `domain` 却持 barrel，是元素根 | 以 `kind` 判（漏掉 barrel 型 domain） |
| D5′ | 枚举面套完整排除矩阵 | 树上看到的 = drift 实际判定的，两消费面零分叉；`ExcludeSet::build` 直接复用 | **原 D5（不套矩阵）**：树里近半是测试文件（306/667），用户裁决改为屏蔽 |
| D6 | 声明但目录不存在的模块根仍出节点（`exists = false`） | 「元素在树上找不到」会成为静默失败；顺带暴露 `pathNotFound` | 跳过不存在的根（树会静默漏模块） |
| D7 | 保留「文件 / 元素」分段切换，缺省「文件」 | 容器元素只在 FQN 树上可见；切换只换左栏组件，`focus` 共用 | 直接删除 `ElementTree`（丢了容器视图，且既有 9 处断言全废） |
| D8 | 文件树不消费漂移报告 | 标记只表达归属，不表达违例；违例仍只在画布边 + 抽屉 | 在树上标违例（等于给节点加第二种语义，且需第二套标记派生） |
| D9 | `ambiguous` / `containsElements` / `none` 行点击只切展开 | 本仓 `ambiguous` 恒 0，为合成语料分支新增 popover 原语不划算 | 候选弹层（新原语 + 新测试面，收益 0） |

---

## 能力

### 新增能力

**`desktop-archi-file-tree`**（新 spec 目录）——左栏文件树的完整契约：命令与扁平 DTO、`FileTreeMark` 五值词表、归属判定与判定单源、枚举面与漂移判定面一致、`exists` 前向防御、左栏呈现与分段切换 / 双向联动、树取数独立失败。

### 修改的能力

（口径：新增 **1** 个能力 = `desktop-archi-file-tree`；修改 **6** 个 = `desktop-archi-canvas`（MODIFIED × 3）/ `desktop-archi-model`（RENAMED × 1）/ `desktop-archi-api`（MODIFIED × 1）/ `desktop-archi-drift`（MODIFIED × 1）/ `desktop-ipc-type-bindings`（MODIFIED × 1）/ `repo-archi-model`（MODIFIED × 1 + ADDED × 1）。与 `specs/` 下七个 delta 目录一一对应。）

- **desktop-archi-canvas**（`MODIFIED` × 3）
  - `MODIFIED`「单一画布组件与两处挂载点」：三段布局的第一段由「左元素树（层级由 FQN 前缀派生）」改为「左栏树——缺省文件树（磁盘枚举 + 元素根归属标记），可切到元素树（FQN 前缀派生）」；同 requirement 的 Scenario「两处挂载点共用同一组件」的「元素树 / 画布 / 抽屉三段齐备」改为「左栏树 / 画布 / 抽屉三段齐备」。其余 bullet（两挂载点表、观察根、显式取数、1-hop、data-testid 纪律）与另两条 Scenario **逐字保留**。
  - `MODIFIED`「结构错的停摆态（全或无）」：仅改三处表述——正文「不渲染元素树节点」→「不渲染左栏树（文件树与元素树均不挂载）」；Scenario「有结构错即停摆」的「元素树零节点」→「左栏树零节点（文件树与元素树均不挂载）」；Scenario「无结构错渲染完整图」的「元素树与画布正常渲染」→「左栏树与画布正常渲染」。其余正文与 Scenario **逐字保留**。
  - `MODIFIED`「漂移标记呈现契约（只认标记，不认判定）」：仅在一处补边界——「画布节点与元素树节点只呈短名，无状态点」之后补「左栏文件树的**归属标记**（`FileTreeMark`）不属本能力的节点标记词汇，定义归 `desktop-archi-file-tree`，MUST NOT 承载漂移违例」。其余正文与 5 条 Scenario **逐字保留**。
- **desktop-archi-model**（`RENAMED` × 1）
  - `RENAMED`「IPC 命令面契约（四命令）」→「IPC 命令面契约（六命令）」：命令表补 `archi_file_tree` 行（只读、结构诊断非空 → `Err`），正文的「四个命令 / 唯一写命令 / 五个 → 六个」与 Scenario「`archi_module` 出参带 api 并列字段」末句「命令总数仍为五个」统一改为**六个**；`## Purpose` 中的「四命令」同步为「六命令」（归档合并时）。其余命令行、`archi_set_gate` 只读约束、`api` 并列字段段**逐字保留**。
- **desktop-archi-api**（`MODIFIED` × 1）
  - `MODIFIED`「生成物落点与入口查找」：bullet「归属规则 SHALL 加一条剥离特例」由「按去掉 `.i` 后的路径」改写为**两条判据取并集**（整组剥离 ∪ 替换扩展名，命中集按最长前缀定档，候选短路禁止）；Scenario「四种剥离形态各归其元素」改写为「两种落点形态各归其元素」（探针改为根同级实形态 + 目录内形态，并新增白名单外后缀不误归的反例）。其余 bullet（`<name>` 取路径末段、`<root>` 无后缀、入口查找顺序、`#[path]` 等值、E0255、路径形态非法即拒收）与 5 条 Scenario **逐字保留**。
- **desktop-archi-drift**（`MODIFIED` × 1）
  - `MODIFIED`「文件归属与两套解析器一套归属逻辑」：补一条生成物归属 bullet（`.i.<ext>` 按并集判据归属；`gate != off` 的已声明元素其生成物进入 `scan.matched[]` 而非 `unmatchedFiles[]`；本仓 `gate` 全 `off` ⇒ 报告零变化，前提为排除配置不变）+ 两条 Scenario（`gate != off` 时生成物进 `matched[]`；`gate` 全 `off` 时字段不移动）。其余四条归属 bullet、两套解析器三段与 5 条 Scenario **逐字保留**。
- **desktop-ipc-type-bindings**（`MODIFIED` × 1）
  - `MODIFIED`「tauri-specta 接入与命令绑定生成」：补一条清单守卫条款——新增命令 / 出线类型时 `bindings/mod_test.rs` 的清单守卫（命令名清单、包装名清单、出线 DTO 清单、archi 族两组清单、尾部追加序）SHALL 同 change 随动更新，条数断言只作清单长度的连带核对、MUST NOT 作为语义判据；本次实测随动面 = 命令 48 → 49、出线 DTO 97 → 101、`ARCHI_COMMANDS` 4 → 6（补齐既有漏登的 `archi_drift`）、`ARCHI_DTO_TYPES` 19 → 23。其余三条 bullet 与 3 条 Scenario **逐字保留**。
- **repo-archi-model**（`MODIFIED` × 1 + `ADDED` × 1）
  - `MODIFIED`「图自洽与停摆禁令」：仅改一处表述与一条 Scenario——「`/architecture` 页面呈现元素树 + 画布 + 抽屉三段」→「左栏树 + 画布 + 抽屉三段」；其余三条 bullet 与另两条 Scenario（成环禁边、孤儿层级零命中）**逐字保留**。
  - `ADDED`「本仓架构域看护配置与排除留痕」：`<archi_dir>/config.json` 存在、内容为 `{ "exclude": ["openspec/**"] }`、不带 `$schema` 键；效果为 `openspec/` 下 3 个原型 tsx 由 drift 的 `unmatchedFiles` 移入 `excluded[]`，模型元素 / 关系 / 计数零变化。

### 零改动的能力及理由

| 能力 | 理由 |
|---|---|
| **desktop-archi-agent** | 不给 agent 新增 `archi_file_tree` 工具——`read` / `glob` 已够；注入矩阵、知识文本与 `SessionToolset` 面零变化 |
| **desktop-archi-module-docs** | `FileTreeMark` 不进 docs 三源；文档关联只按元素 `path` 范围扫描，文件树不改扫描面 |
| **desktop-archi-gate** | 门禁结论只消费漂移报告；本变更零命令面写入、零档位语义变化 |
| **desktop-archi-api-check** | 十条契约判据与本变更无交集（`apiEntryMissing` 走 `paths` 的入口查找，不读 `OwnerIndex`） |
| **desktop-archi-chat / desktop-change-chat** | 画布外挂的会话面板入参与插槽零变化 |
| **workspace-layout-resolution** | 复用既有 `archi_config_path` / `archi_models_dir` 派生，零新增 layout 函数 |
| **desktop-workspace-config** | 架构域配置与工作区 `.dev-team/config.json` 互不读取（`KNOWN_KEYS` 七项零变化） |
| **repo-archi-model 的元素 / 关系 / 计数** | `views/architecture/**` 已是单元素 `App.Frontend.Views.Architecture`（目录内 `components` / `hooks` / `lib` 不建元素），新增文件零模型漂移；51 元素 / 113 关系、`06` 段 28 / 64 不变（`repo-archi-model` 本 delta 只改布局表述 + 新增配置 requirement，不动元素 / 关系 / 计数） |

---

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `packages/desktop/src-tauri/crates/core/archi/src/scan/owner.rs`（改） | ① `strip_generated` → `strip_candidates`（返回 1~2 个候选：整组剥离 `<name>.i.<ext> → <name>`、替换扩展名 `<stem>.i.<ext> → <stem>.<ext>`）；② `owner_of` 对每个候选各跑一遍最长前缀、按 `(fqn, 前缀长度)` 合并去重定档（同长仍 `Ambiguous`），**MUST NOT 候选短路**；③ `normalize` / `prefix_matches` / `OwnerIndex::build` 语义逐字不动；④ 白名单（`rs` / `ts` / `tsx` / `js` / `jsx`）与 `.i.py` / `.d.rs` / 大写 `.I.rs` / 无扩展名 `.i` 的零误剥保持 |
| `packages/desktop/src-tauri/crates/core/archi/src/scan/mod.rs`（改） | `walk_source_files` 与 `is_source_file` 由私有提为 `pub(crate)`（既有单点，无第二份实现；提权限比复制一份遍历便宜），扫描语义零变化 |
| `packages/desktop/src-tauri/crates/core/archi/src/tree.rs`（新增） | `file_tree(root) -> Result<ArchiFileTree, String>`：读模型（诊断非空 → `Err`；空模型 → 全 `none`）→ 由 `graph.elements` 建 `known` FQN 集后调 `drift::config::load_config(root, &known)` 取 `exclude`（`ignore` 不参与）→ `ExcludeSet::build` → 枚举（`walk_source_files` + `is_source_file`）+ 过滤 → 合成目录 + `module_roots` 出节点 → 逐条定 `mark` / `owners` / `contains` / `exists` → `path` 字节序；纯读盘、零写盘、零 spawn；自带归属实现禁止（一律经 `OwnerIndex`） |
| `packages/desktop/src-tauri/crates/core/archi/src/dto.rs`（改） | 新增 `ArchiFileTree` / `ArchiFileEntry` / `FileEntryKind`（`Dir` \| `File`）/ `FileTreeMark`（五值 + `ALL` + `as_str`）；`specta::Type` + `serde(rename_all = "camelCase")`；既有 DTO 与九组封闭词汇零变化 |
| `packages/desktop/src-tauri/crates/core/archi/src/lib.rs`（改） | `pub mod tree;` + 再导出 `file_tree` 与四个新出线类型（跨 crate 唯一出口形态沿既有 `pub use` 段） |
| `packages/desktop/src-tauri/src/commands/archi/mod.rs`（改） | 新增 `archi_file_tree(root) -> Result<ArchiFileTree, String>` 薄包装 + `archi_file_tree_inner(root: &Path)`；`check_root` 复用；错误面 `Result<T, String>`；同步两处注释口径（文件头「五命令」→「六命令」、第 19 行「IPC 四命令契约」→「六命令」） |
| `packages/desktop/src-tauri/src/commands/mod.rs`（改） | `all_commands!` 的 archi 段追加 `commands::archi::archi_file_tree`（builder 注册与 `generate_handler!` 同源；位置在 `archi_drift` 之后、`read_file` 之前） |
| `packages/desktop/src-tauri/src/bindings/mod_test.rs`（改） | 清单随动：`COMMAND_NAMES` / `COMMAND_WRAPPERS` 48 → 49（`archi_file_tree` / `archiFileTree` 插在 `archi_drift` 之后、`read_file` 之前）、`DTO_TYPES` 97 → 101（尾部两段本身是字典序：`ArchiFileEntry` / `ArchiFileTree` 插在 archi 段的 `ArchiExcludedFile` 与 `ArchiGateChange` 之间，`FileEntryKind` / `FileTreeMark` 插在末段的 `EdgeMark` 与 `GateTier` 之间；首锚 `ActivePhase` / 尾锚 `WarningKind` 不动）、`ARCHI_COMMANDS` 4 → 6（补 `archi_drift` + 新增项）、`ARCHI_DTO_TYPES` 19 → 23（`ArchiFileEntry` / `ArchiFileTree` 插在 `ArchiDependsOn` 与 `ArchiDiagnostic` 之间，`FileEntryKind` / `FileTreeMark` 插在 `EdgeMark` 与 `GateTier` 之间）；3 处 `COMMAND_NAMES.len()` 断言与既有尾部追加序期望 vec 同改；`清单遗漏即产物覆盖断言必红` 用例的「取 `ARCHI_COMMANDS` 末条做副本删一条」因该族末条由 `archi_set_gate` 变 `archi_file_tree` 而需同步；`archi四命令在册且既有条目零位移` 用例名与 `ARCHI_COMMANDS` 的「四命令」注释同步改为符实（`commands/archi/mod_test.rs` 的 `四命令root校验四形态一律err` 亦在测试文件行另计） |
| `.dev-team/architecture/config.json`（新增） | `{ "exclude": ["openspec/**"] }`；**不带 `$schema`**（`deny_unknown_fields` 只认 `ignore` / `exclude` 两键，写 `$schema` 会让三条消费面同时 `Err`） |
| `packages/desktop/src/views/architecture/hooks/use-archi-file-tree.ts`（新增） | 显式取数 `commands.archiFileTree(root)`：挂载 / `root` 变化 / 手动刷新三触发（`refreshTick` → `key` 重挂），零 watch 零轮询；返回 `tree` / `loading` / `error`；错误与图独立（不阻断画布） |
| `packages/desktop/src/views/architecture/components/file-tree.tsx`（新增） | 左栏文件树 + 分段控件组件（`TreeModeSwitch`，`data-testid="archi-tree-mode"`，缺省 `file`；控件由 `architecture-canvas.tsx` 挂载与持有状态，本件只导出并渲它）；per-path 翻转展开集合（默认展开前 2 段）、折叠不渲 DOM、`ScrollArea`；五种 mark 标记与 tooltip（`模块 <FQN>` / `多归属：A、B` / `属于 <FQN>` / `含 N 个模块`）；`elementRoot` / `member` 行点击 → `onFocus(owners[0])`，其余三值只切展开；`focus` 变化 → 按 `owners` 含该 FQN 定位全部命中条目并展开祖先链 + 多行高亮 + 滚到第一条；零命中 → 顶部一行弱提示（不断言成因）；底部常驻一行可折叠图例；testid `archi-file-tree` / `archi-file-node` / `archi-file-mark`（`data-mark`）/ `archi-file-tree-error` / `archi-file-tree-empty` |
| `packages/desktop/src/views/architecture/components/architecture-canvas.tsx`（改） | `CanvasBody` 左栏改为「分段控件 + 条件渲 `FileTree` / `ElementTree`」；`useArchiFileTree(root)` 接线；`ElementTree` 入参与 testid 零变化；画布 / 抽屉 / 标记合并 / 停摆态分支零改动 |
| `packages/desktop/package.json`（改） | `version` `0.6.35 → 0.6.36`（一次 patch）；`src-tauri/Cargo.toml` 的 crate 版本行**不跟** |
| `packages/desktop/src/types/generated/bindings.ts` + `bindings.zod.ts`（再生成入库） | `archiFileTree` typed 包装 + 四个新出线类型与 `argsSchemas` / `resultSchemas` 表项；随 `bindings:export` 刷新，入库前经 `git diff` 人工核对 |

### 测试文件

| 文件 | 现状 | 处理 |
|---|---|---|
| `crates/core/archi/src/tree_test.rs`（新增） | — | 真实 tempdir + 真实 walker（沿 `docs_test` / `scan` 零 mock 先例）：目录型元素根 → `elementRoot`；其内文件 → `member`；祖先目录 → `containsElements` 且 `contains` 正确；单文件型元素根 → 文件节点 `elementRoot`；多 `path` 元素 → 多个 `elementRoot` 且 `contains` 按 FQN 去重；嵌套父子各 `elementRoot`、内层文件归最深；生成物 → `member`；`target` / `node_modules` / `.git` / gitignore 命中项不出现；测试面（`*.test.tsx` / `*_test.rs` / `tests/`）不出现；config `exclude` 命中项不出现（`openspec/**`）；声明但目录不存在的根仍出节点（`exists = false`）；两次调用逐字相同（幂等）；`FileTreeMark::ALL` / `FileEntryKind::ALL` 与 `as_str` 逐值对拍 + 长度断言 |
| `crates/core/archi/src/scan/owner_test.rs`（改） | 19 条 `strip_generated` 相关断言（`生成物剥离四种形态各归其元素` / `生成物剥离使单文件元素根同级生成物归元素` / `生成物白名单外后缀不剥离` / `生成物剥离只去末段标记` 等） | ① `:437` 的 `src/dir.i.ts`（根 `src/dir/`）期望由 `Owner::None` **翻转为 `Owner::Element`**，同步改写 `生成物剥离使单文件元素根同级生成物归元素` 的用例名 / doc 与断言意图（原文写着「目录形态元素根的根同级生成物仍不命中」——那正是 bug 契约；该用例其余「其它扩展名仍 `None`」断言按并集复算仍成立，保持绿）；② `生成物剥离四种形态各归其元素` 保留既有四条目录内形态并**另加**四条根同级实形态正例（含 `crates/infra/archi.i.rs`）；③ 白名单外后缀 4 条反例保持绿；④ 新增并集定档用例（`src/dir/` 与 `src/dir.ts` 同场、探针 `src/dir.i.ts` ⇒ 归更具体的单文件根，钉住「禁止候选短路」） |
| `crates/core/archi/src/tree.rs` 之外：`dto_test.rs` | 既有 DTO 形状断言 | 追加新 DTO 字段名与声明序断言（`ArchiFileEntry` 七字段、`ArchiFileTree` 一字段），既有断言零改 |
| `packages/desktop/src-tauri/src/commands/archi/mod_test.rs`（改） | 1279 行；文件头**已写「六命令薄包装」而实现是 5 条**（现状第三处口径不一，本变更落地后自然对齐）；`四命令root校验四形态一律err` 等用例名沿用「四命令」旧口径 | 追加 `archi_file_tree` 与 `_inner` 逐字段一致断言、空白 / 非目录 root → `Err`、只读命令零写盘、结构诊断非空 → `Err`、空模型 → `Ok`；root 校验用例扩到六命令并把名字里的「四命令」改为符实（不新增第二条同义用例） |
| `packages/desktop/src-tauri/src/bindings/mod_test.rs`（改） | 清单守卫 | 见「实现文件」行；另断言 `archiFileTree` 的 args schema 形状与导出类型名逐名命中 |
| `packages/desktop/src/views/architecture/components/file-tree.test.tsx`（新增） | — | 目录先于文件、`name` 序；缺省展开前 2 段、展开不触发 invoke；五种 mark 各渲对应 `data-mark`；点 `elementRoot` / `member` → `onFocus(owner)`，点 `ambiguous` / `containsElements` / `none` → 零 `onFocus`；`focused` 为多 `path` 元素 → 多行同时高亮（`owners` 含该 FQN 即命中）；零命中 → 顶部提示且不滚动；注入含 `*.test.tsx` / `*_test.rs` 的取数结果后零对应行（D5′）；注入 config `exclude` 后该路径零行；空态 / 错误态（错误不阻断画布） |
| `packages/desktop/src/views/architecture/hooks/use-archi-file-tree.test.ts`（新增） | — | 三触发（挂载 / `root` 变化 / 刷新）、零轮询零 watch、失败态与旧数据清空（旧树 MUST NOT 冒充本次结果） |
| `packages/desktop/src/views/architecture/components/architecture-canvas.test.tsx`（改） | 4 处 `getByTestId('archi-element-tree')`（385 / 817 / 837 / 931）、5 处 `archi-tree-node` 交互与停摆断言（373 / 583 / 587 / 774 / 833） | D7 缺省切「文件」后逐处改：元素树断言改为**先点分段控件 `data-mode="element"`** 再断言；文件树断言用新 testid（`archi-file-tree` / `archi-file-node` / `archi-file-mark`）；停摆态断言改为「左栏树零节点（两视图均不挂载）」；画布 / 抽屉 / 标记断言逐字保留 |
| `packages/desktop/src/views/architecture/components/element-tree.test.tsx` | 现有元素树组件测试 | 零改动（本件不删不改，testid 不变） |

### 不要修改

- **画布与抽屉渲染**：`components/architecture-canvas.tsx` 的 `toNodes` / `toEdges` / `neighborhoodPositions` / `defaultFocus` / `ArchiNodeView` / `StructureErrorPanel` 分支、`components/element-drawer.tsx`、`components/structure-error-panel.tsx`、`lib/marks.ts`、`lib/drift-marks.ts`、`lib/fqn.ts` 零改动。
- **元素树组件**：`components/element-tree.tsx` 保留为「元素」分段视图，入参 / testid / 渲染零改动（MUST NOT 删除）。
- **漂移判定与门禁**：`crates/core/archi/src/drift/**`（树只调用其既有 `config::load_config` 读面，零改动）、`crates/infra/archi/**`（门禁 runner）、`crates/core/archi/src/scan/exclude.rs` / `exclude_test.rs` 零改动（D5′ 后 `ExcludeSet::build(extra)` 语义正是要的，MUST NOT 新增 `build_config_only` 一类入口）。
- **扫描编排**：`scan/mod.rs` 的 `scan()` 函数体（枚举 → 模式过滤 → 排除 → 归属 → 抽取 → 聚合六段）与 `SOURCE_EXTENSIONS` 常量零改动（只把两个私有 helper 提为 `pub(crate)`）。
- **契约生成面**：`crates/core/archi/src/api/**`、`crates/tools/archi-api-gen/**`、`<archi_dir>/api/**` 零改动（A2″ 只改归属判据，不动生成物落点 `paths::generated_path` 与 `#[path]` 等值判据）。
- **写入面**：零新增写命令，`archi_set_gate` 仍是唯一写；`archi_file_tree` 只读、零写盘。
- **工作区配置**：`.dev-team/config.json`、`crates/core/config/**`（含 `config.schema.json` 镜像与一致性测试）零改动——架构域配置不是工作区配置。
- **模型资产**：`.dev-team/architecture/models/**`、`decisions/0001-model-granularity.md`、`api/INDEX.toml` 零改动（`views/architecture/**` 已是单元素，新增文件零模型漂移）。
- **agent 面**：`crates/infra/agent/**`（含 `archi_knowledge.rs` 与注入矩阵）零改动，MUST NOT 为树新增 agent 工具。
- **规格基线**：`openspec/specs/**` 只在 change 目录出 delta（归档时合入）。
- **`openspec/changes/archi-file-tree/explore.md`**：只读。
- **`packages/desktop/src-tauri/Cargo.toml`**：crate 版本行不随前端 patch 变动。

---

## 验收标准

| ID | 验收条件 | 验证方法 |
|---|---|---|
| AC-1 | **新命令可数且只读**：`archi_file_tree(root)` 在 `all_commands!` 登记、有 typed 包装，archi 段命令总数 **6**（`archi_graph` / `archi_module` / `archi_change_summary` / `archi_set_gate` / `archi_drift` / `archi_file_tree`）；调用后磁盘字节零变化 | `commands/archi/mod_test.rs` 新用例 + `bindings/mod_test.rs` 清单断言 |
| AC-2 | **结构诊断非空即 `Err`，空模型不报错**：模型含任一结构错时 `archi_file_tree` 返回 `Err`（与 `archi_module` 同口径）、不返回部分树；模型目录缺席或零元素时返回 `Ok`（全条目 `mark = none`、`owners` 空）——冷启动空模型是合法起点，MUST NOT 与结构错混为一类 | `commands/archi/mod_test.rs` |
| AC-3 | **DTO 扁平且确定**：`ArchiFileTree.entries` 为 `ArchiFileEntry` 扁平数组、按 `path` 字节序；`ArchiFileEntry` 七字段（`path` / `name` / `kind` / `mark` / `owners` / `contains` / `exists`）按声明序出线为 camelCase | `dto_test.rs` + `bindings/mod_test.rs` 类型段断言 |
| AC-4 | **`FileTreeMark` 封闭五值**：`elementRoot` / `ambiguous` / `member` / `containsElements` / `none`，`ALL` 长度断言与 `as_str` 逐值对拍；优先级单值 `elementRoot > ambiguous > member > containsElements > none`；MUST NOT 复用 `NodeMark`，MUST NOT 纳入四面词汇对拍 | `tree_test.rs` + `dto_test.rs` |
| AC-5 | **归属判定正确**：目录型根 → `elementRoot`（`owners` 恰 1）；其内文件 → `member`；祖先目录 → `containsElements` 且 `contains` = 后代元素根 FQN 去重数；单文件型根 → 文件节点 `elementRoot`；多 `path` 元素 → 多 `elementRoot` 且 `contains` 不重复计同一元素；嵌套父子各 `elementRoot`、内层文件归最深前缀；同一归一路径被 ≥ 2 个元素声明 → `ambiguous`（不判 `elementRoot`）；容器元素（`path` 空）零节点 | `tree_test.rs` |
| AC-6 | **判定单源**：`tree.rs` 不含任何归属匹配实现，标记只经 `OwnerIndex::owner_of` + `module_roots` 查表；`src` 全域无第二份 `path` 前缀匹配 / 生成物剥离实现 | 源码审查 + `grep` |
| AC-7 | **A2″ 并集生效**：`crates/infra/archi.i.rs`（根同级形态）归 `App.Backend.Infra.ArchiRuntime`；目录内形态 4 条既有正例保持；`src/dir.i.ts`（根 `src/dir/`）由 `None` 翻为 `Element`；`src/dir/` 与 `src/dir.ts` 同场时探针归更具体的单文件根（禁止候选短路）；`.i.py` / `.d.rs` / `.I.rs` / 无扩展名 `.i` 归属与改动前逐值相同 | `scan/owner_test.rs` |
| AC-8 | **枚举面 = 漂移判定面**：树经 `ExcludeSet::build(config.exclude)`；测试面（`*.test.{ts,tsx,js,jsx}` / `*.spec.*` / `__tests__/` / `*_test.rs` / `tests/` / `fixtures/` / `__mocks__/`）与配置 `exclude` 命中项均不在树中；`target` / `node_modules` / `dist` / `build` / `.git` / 隐藏项 / gitignore 命中项不出现；`scan/exclude.rs` 与 `exclude_test.rs` 零 diff | `tree_test.rs` + `git diff` |
| AC-9 | **本仓配置落位**：`.dev-team/architecture/config.json` = `{ "exclude": ["openspec/**"] }`，**无 `$schema` 键**；`archi_drift` 与 `archi_file_tree` 均不因此 `Err`；`openspec/` 下 3 个原型 tsx 不在树中、drift 报告由 `unmatchedFiles` 移入 `excluded[]`（+3 / −3） | `tree_test.rs` + `commands/archi/mod_test.rs` + 手工跑一次 `archi_drift` 比对 |
| AC-10 | **前向防御分支**：声明了但目录不存在的模块根仍出节点且 `exists = false`（合成 tempdir 用例；本仓 45 条 `path` 全部在盘，无真实样本） | `tree_test.rs` |
| AC-11 | **左栏替换与分段切换**：缺省渲文件树（`archi-file-tree`），分段控件可切到元素树（`archi-element-tree` / `archi-tree-node` 原 testid 不变）；点击 `elementRoot` / `member` → 一次 `setFocusOverride`（画布与抽屉随动）；点击 `ambiguous` / `containsElements` / `none` → 零 `onFocus`（`archi_module` 调用次数不增）；`focus` 命中多条目（多 `path` 元素）时多行同时高亮并滚到第一条；`focus` 零命中时顶部一行弱提示且不滚动（文案不断言成因） | `file-tree.test.tsx` + `architecture-canvas.test.tsx` 改写项 |
| AC-12 | **树取数独立失败**：`archi_file_tree` 失败时只渲 `archi-file-tree-error` 行内提示，画布与抽屉照常渲染（图零降级）；树 `loading` 呈「解析中…」、树空呈「仓库内无源文件」；停摆态整块 `StructureErrorPanel`、树不挂载 | `file-tree.test.tsx` + `architecture-canvas.test.tsx` |
| AC-13 | **零 watch 零轮询**：树取数只在挂载 / `root` 变化 / 手动刷新（`refreshTick` → `key` 重挂）三触发；源码零 `watchSubscribe` / 零定时器 | 源码审查 + `grep` + `use-archi-file-tree.test.ts` |
| AC-14 | **清单守卫随动**：`bindings/mod_test.rs` 五处计数与两组 archi 清单同步（命令 48 → 49、出线 DTO 97 → 101、`ARCHI_COMMANDS` 4 → 6、`ARCHI_DTO_TYPES` 19 → 23），尾部追加序断言把 `archi_file_tree` 插在 `archi_drift` 之后（`read_file` 仍为尾条）；`清单遗漏即产物覆盖断言必红` 的「取 `ARCHI_COMMANDS` 末条」对照臂随之改判；`bindings:export` 后两份生成物入库零 diff 复核 | `cargo nextest` + `git diff` |
| AC-15 | **本仓实测基线**（不作为 MUST，登记留痕）：进树文件 358 / 合成目录 117 / 补出根 0 / 节点 475 / 紧凑 JSON 84.2 KB；`mark` 分布 `elementRoot 45 / member 373 / containsElements 12 / none 45 / ambiguous 0`；42 个元素根全部仍有 ≥ 1 个可见文件 | `tree.rs` 一次性统计用例或 ad-hoc 脚本输出 + proposal 留痕 |
| AC-16 | **门禁**：`pnpm -C packages/desktop run check` 绿（`server:check` → `bindings:export` 零 diff → `api:check` 零 diff → `client:check` 含 `knip` 零新增 ignore）；`cargo nextest run --workspace` 全绿；`package.json` `0.6.35 → 0.6.36`，`src-tauri/Cargo.toml` crate 版本行不变 | 门禁命令 + `git diff --stat` |
| AC-17 | **边界零 diff**：`.dev-team/architecture/models/**`、`decisions/**`、`api/**`、`crates/core/archi/src/drift/**`、`crates/infra/archi/**`、`crates/infra/agent/**`、`crates/core/archi/src/scan/exclude.rs`、`.dev-team/config.json`、`explore.md` 全部零 diff | `git diff --stat` 审查 |

**AC ↔ spec requirement 对应**：

| AC | spec requirement / scenario |
|---|---|
| AC-1 / AC-2 / AC-3 | `desktop-archi-file-tree` ADDED「文件树命令与扁平条目投影」；`desktop-archi-model` RENAMED「IPC 命令面契约（六命令）」 |
| AC-4 | `desktop-archi-file-tree` ADDED「归属标记封闭五值词表」 |
| AC-5 / AC-6 | `desktop-archi-file-tree` ADDED「归属判定与判定单源」 |
| AC-7 | `desktop-archi-api` MODIFIED「生成物落点与入口查找」+ `desktop-archi-drift` MODIFIED「文件归属与两套解析器一套归属逻辑」 |
| AC-8 | `desktop-archi-file-tree` ADDED「枚举面与漂移判定面逐条一致」 |
| AC-9 | `repo-archi-model` ADDED「本仓架构域看护配置与排除留痕」+ `desktop-archi-file-tree` ADDED「枚举面与漂移判定面逐条一致」 |
| AC-1 / AC-11（三段布局表述） | `repo-archi-model` MODIFIED「图自洽与停摆禁令」的 Scenario「正常态而非停摆态」（「元素树 + 画布 + 抽屉三段」→「左栏树 + 画布 + 抽屉三段」），与 `desktop-archi-canvas` 的布局表述同批对齐 |
| AC-10 | `desktop-archi-file-tree` ADDED「元素根缺失不静默」 |
| AC-11 / AC-12 / AC-13 | `desktop-archi-file-tree` ADDED「左栏文件树呈现、分段切换与双向联动」+「树取数的独立失败与状态面」；`desktop-archi-canvas` MODIFIED「单一画布组件与两处挂载点」+「结构错的停摆态（全或无）」+「漂移标记呈现契约（只认标记，不认判定）」 |
| AC-14 | `desktop-ipc-type-bindings` MODIFIED「tauri-specta 接入与命令绑定生成」 |
| AC-15 / AC-16 / AC-17 | 工程验收面（无 requirement 对应；以门禁与 `git diff` 为判据） |

---

## 风险

| ID | 风险 | 影响 | 概率 | 缓解措施 |
|---|---|---|---|---|
| R-1 | **A2″ 并集判据改动动到漂移判定面** | 若实现成「候选 1 命中即短路」，`src/dir.i.ts` 一类会更具体元素判定失真；反之若写成纯整组剥离，4 条既有断言从绿变红且必须逐条说服评审「不是回归」 | 中 | 实现形态写死为「候选集合并定档」（`owner_test` 的并集定档用例钉住）；`drift` 报告本仓零变化经 §14.2 逐字段复算；AC-7 / AC-8 机械判据 |
| R-2 | **306 个测试文件在树上不可见**，用户「在树里找测试文件」落空 | 误解为「树缺文件」的缺陷报告 | 中 | D5′ 的显式代价（换「树面 = 判定面」零分叉）；图例常驻一句「仅显示判定面源文件（测试文件与 `exclude` 项不在树中）」；命中留痕已在 drift 报告的 `excluded[]` 里，需要精确清单时看那里 |
| R-3 | **新增架构域配置文件格式写错**（带 `$schema`） | `deny_unknown_fields` 让 `load_config` 直接 `Err`，同时打断 drift 命令、门禁 runner 与本次新增的树三条消费面 | 中 | AC-9 断言「无 `$schema` 键」；`drift/config.rs` 的严格性零改动；`repo-archi-model` 的新 requirement 明文写死该形态与工作区配置的差别 |
| R-4 | **本仓新增 `exclude` 改变 drift 扫描面**（与 A2″ 是两个独立变量，同一 change 内同时落地） | `openspec/**` 三个原型 tsx 由 `unmatchedFiles` 移入 `excluded[]`（+3 / −3），既有对真仓计数断言会红；且若把两者混在一条「报告零变化」断言里，会得到自相矛盾的期望 | 低 | 已核：golden 走合成语料、perf 走合成夹具、pilot 只读 api 资产，无既有测试对真仓 drift 计数断言；连带效应在 proposal 与 spec 两处如实登记；`desktop-archi-drift` 的零变化 Scenario 显式声明「保持排除配置不变」，两个变量分开断言 |
| R-5 | **475 条 DTO（实测 84.2 KB）与解析时间** | 首屏取数变慢、大仓库更甚 | 低 | 实测定档；仍超阈值则改 `archi_file_tree(root, dir)` 单层 + 前端缓存（DTO 形状不变） |
| R-6 | **`graph` 与 `tree` 两次独立读模型，极端并发编辑下不一致** | 树上某元素根与画布节点集合短暂不匹配 | 低 | 同一 `refreshTick` 重挂内两次读；join 失败时 tooltip 退化为裸 FQN，不崩（前端只读 `owners` 字符串） |
| R-7 | **既有前端断言成批变红**（缺省切「文件」后 4 处 `archi-element-tree` + 5 处 `archi-tree-node`） | 逐处改写时容易顺手删掉真断言（尤其停摆态零节点断言） | 高 | 改写口径写死：元素树断言一律「先点分段 `data-mode="element"` 再断言」；停摆态断言改为「两视图均不挂载、左栏零节点」；画布 / 抽屉 / 标记断言逐字保留；AC-11 / AC-12 反向钉住 |
| R-8 | **`bindings/mod_test.rs` 五处硬编码计数与两组清单** | 新命令 + 四个出线类型会让 5 处断言全红，漏改一处即门禁红 | 高 | AC-14 逐项点名（48 → 49 / 97 → 101 / 4 → 6 / 19 → 23 + 尾序插入）；除计数外另有两处**非计数**断言随动（尾部追加序期望 vec 增加一条、`ARCHI_COMMANDS` 末条对照臂由 `archi_set_gate` 变 `archi_file_tree`），易漏；`desktop-ipc-type-bindings` 的 MODIFIED 把「清单守卫随动」写成条款 |
| R-9 | **用户把树上的标记误读为违例** | 「灰点 = 有问题」的误判 | 中 | 标记只表达归属；图例常驻一行；违例仍只在画布边与抽屉呈现（D8：树不消费漂移报告） |
| R-10 | **`ambiguous` 分支实现后无真实样本可验证**（本仓两种来源——同长不同前缀、同一归一路径被多元素声明——均为 0） | 分支腐化（改坏不会被本仓数据发现） | 中 | `tree_test.rs` 合成两类用例（同长重复前缀 / 同路径多元素声明）；前端 `file-tree.test.tsx` 注入 `mark = ambiguous` + 两 owners 断言「只切展开、零 `onFocus`」 |
| R-11 | **元素根只剩测试文件时在树上变空壳** | 用户以为模块消失 | 低 | 本仓实测无（最少 1 个可见文件）；前向防御：元素根本身仍出节点（D6 的 `exists` 只看目录在不在盘），最坏是「空目录节点」而非消失，`contains` 给 0，不误报 `pathNotFound` |
| R-12 | **`walk_source_files` 提为 `pub(crate)` 扩大内部可见面** | 组内其他模块可能复制其语义 | 低 | 它是既有单点、无第二份实现；提权限比复制一份遍历便宜；`tree.rs` MUST NOT 自建遍历（AC-6 / AC-8 覆盖） |
| R-13 | **规格口径残留**：`desktop-archi-model` 的 `## Purpose` 与 Module Contract 表内「四命令」、`commands/archi/mod.rs` 文件头「五命令」 | 归档后 spec 与代码各说各话（正是本变更要修的病灶） | 中 | delta 用 `RENAMED` 改 requirement 标题、正文与 Scenario 三处数字；`## Purpose` 与 Module Contract 的同步写进 proposal 与 spec 的落地说明；AC-1 / AC-14 以命令清单实测为判据 |
| R-14 | **配置非法（无通配符 / glob 编译失败）让树 `Err`** | 用户看到树错误态而画布正常，误以为树坏了 | 低 | 错误文案沿用 `drift/config.rs` 与 `ExcludeSet::build` 的既有显式文案（含非法条目）；错误只落树、不阻断画布（AC-12） |
