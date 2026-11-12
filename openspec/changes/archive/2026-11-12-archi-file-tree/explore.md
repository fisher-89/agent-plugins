# 架构页左栏：元素树改为文件树（模块标记 + 范围圈定）

workflow_type: requirement

> 本文是**结论稿**。范围是 `/architecture` 三段布局的最左栏：把「元素」FQN 树换成仓库文件树，
> 在树上标出哪些目录 / 文件是元素根（`path` 根），点击模块即以既有 1-hop 语义圈定它的范围。
> 画布、抽屉、漂移标记、门禁、写入面全部零改动。

---

## 0. 一句话范围

```
左栏  元素树（FQN 前缀派生）  →  文件树（磁盘枚举 + 归属标记）
点击  元素 FQN               →  同一次 onFocus(fqn)，行为不变
新增  archi_file_tree(root) 命令（只读，读 archi 配置的 exclude 一键）+ FileTreeMark 五值词表
改判  归属判据 A2″（整组剥离 ∪ 替换扩展名）——只动 OwnerIndex，树与 drift 同源受益
不动  画布 / 抽屉 / 模型格式 / 任何写命令（drift 判定规则不变，报告本仓零变化）
```

---

## 1. 起点数据

```
元素 51（容器 9 + 叶 42）                            ← §12.1-④ 订正
  path 非空元素 42（01:0 / 02:7 / 03:7 / 04:1 / 05:2 / 06:25）
  path 条目 45 = 29 目录形 + 16 文件形（Shell 四文件单元素 → 4 条）
  容器元素 9 个 path 恒空（8 domain + 1 package）

仓库源文件 667（含 openspec 的 3 个原型 tsx；剔隐藏目录后）
  ├ 测试面 306（*_test.rs 162 / *.test.ts[x] 142 / tests/ 目录 …）——按 D5′ 排除
  └ openspec 3（原型 tsx）——按 B 走 config exclude 排除
⇒ 进树文件 358、合成目录 117、补出根 0 → **树总节点 475**
全量跟踪文件 4288（md 1354 / json 1224 / log 540 …）——按扩展名天然不进树
```

两个直接后果：

- **容器元素在文件树上必然不可见**（`path` 恒空，无持有文件）。这是
  `decisions/0001-model-granularity.md` §3 的直接后果，不是遗漏。
- 只有五种源文件扩展名进树（`.rs` / `.ts` / `.tsx` / `.js` / `.jsx`），与漂移扫描面同集合。
  `.md` / `.json` 按扩展名天然不入树。**但 `openspec/` 会入树**——它下面有 3 个未被
  gitignore 的 `.tsx` 原型（§12.1-③），故按 B 裁决走 archi 配置 `exclude` 排除（§13.4）。
- **枚举面 = 漂移扫描面**（D5′）：树套用完整排除矩阵（默认 13 条 + config），
  故树上看到的 = drift 实际判定的那批文件。「导航面如实」的旧取向已按用户裁决放弃，
  代价见 §10 与 §12.1-⑤。

---

## 2. 定稿决策

| # | 决策 | 理由 |
|---|---|---|
| D1 | 归属在**后端**算，前端零重算 | 「判定单源」纪律（drift / canvas 两条 spec 明令）；且 `OwnerIndex` 已承载 `./` 归一、尾 `/` 剥离、`.i.*` 剥离、同长多命中 `Ambiguous` 四个分支，前端重写必漏 |
| D1′ | 剥离判据**下沉 `OwnerIndex`**、取「整组剥离 ∪ 替换扩展名」并集（A2″） | 树与 drift 必须对同一文件给出同一归属；选树侧另写规则会立刻分叉。纯整组会破 4 条既有断言、并集只翻转 1 条（正是 bug 本身），见 §13.3 / §14 |
| D2 | 命令返回**扁平条目数组**，嵌套由前端派生 | golden 可直接断言；serde / specta 零递归；前端本来就要按 path 做展开态与滚动定位 |
| D3 | 新词表 `FileTreeMark` 五值，**不复用 `NodeMark`** | `NodeMark` 归画布节点、生命周期在漂移报告；同名不同义会让两处语义相互污染 |
| D4 | 归属判据是 **`path` 非空**，不是 `kind` | `App.Frontend.Components.Agent` 是 `domain` 却持有 barrel `index.tsx`，它是元素根 |
| D5′ | 枚举面**套用完整排除矩阵**（默认 13 条 + archi 配置 `exclude`），与 drift 扫描面逐条相同 | 用户裁决：树屏蔽测试文件。收益是「树上看到的 = drift 判定的」，两个消费面对「哪些文件算数」零分叉，且**不需要**新增 `ExcludeSet` 入口（既有 `build(extra)` 直接可用，§13.4-B2 作废）。代价：306 个测试文件在树上不可见（找测试文件请回代码 / grep），须在图例说明 |
| D6 | 声明了但目录不存在的模块根**仍出节点**（`exists = false`） | 「元素在树上找不到」会成为静默失败；顺带暴露 `pathNotFound` |
| D7 | 保留「文件 / 元素」分段切换，默认「文件」 | 容器元素只在 FQN 树上可见；「谁依赖谁」在 FQN 树上更顺。切换只换左栏组件，`focus` 共用 |
| D8 | 文件树**不**消费漂移报告 | 标记只表达归属，不表达违例；违例仍只在画布边 + 抽屉呈现 |
| D9 | `ambiguous` / `containsElements` / `none` 行点击**只切展开、零 onFocus** | 本仓 `ambiguous` 恒 0，为只可能出现在合成语料里的分支新增 popover 原语不划算；`owners` 已在 DTO 里存全量候选，将来放开只改点击处理 |

---

## 3. 数据面

### 3.1 命令

```
archi_file_tree(root: String) -> Result<ArchiFileTree, String>
```

薄包装（三件事纪律），`archi_file_tree_inner(root: &Path)` 便于离 Tauri 测试——形态沿
`src-tauri/src/commands/archi/mod.rs` 既有五个命令。

- 结构诊断非空 → `Err`（与 `archi_module` 同口径）。停摆态下画布整块被
  `StructureErrorPanel` 替换，树本就不会挂载。
- 只读、零写盘；**只读架构域配置的 `exclude` 一个键**（`<archi_dir>/config.json`，
  经 `drift::config::load_config`），不读工作区 `.dev-team/config.json`（§13.4-B）。
- 命令总数：**5 → 6**（现状实测 5：`archi_graph` / `archi_module` /
  `archi_change_summary` / `archi_set_gate` / `archi_drift`）。**顺带校正
  `desktop-archi-model`**：其标题与 Module Contract 写「四命令」、自己的 Scenario 却写
  「命令总数仍为五个」，三处口径不一，统一为六。

### 3.2 DTO（`core/archi/src/dto.rs`）

```rust
pub struct ArchiFileTree {
    pub entries: Vec<ArchiFileEntry>,   // path 字节序，确定性
}

pub struct ArchiFileEntry {
    pub path: String,          // repo 相对 POSIX；根目录为 ""
    pub name: String,          // 末段名
    pub kind: FileEntryKind,   // Dir | File
    pub mark: FileTreeMark,    // 五值，见 3.3
    pub owners: Vec<String>,   // 归属 FQN：elementRoot 恰 1；ambiguous ≥2；其余空
    pub contains: u32,         // 后代模块数（按 FQN 去重，不含自身）
    pub exists: bool,          // Dir 且为模块根：目录是否在盘
}
```

`specta::Type` + `#[serde(rename_all = "camelCase")]`，经 `bindings:export` 入
`bindings.ts` / `bindings.zod.ts`。

### 3.3 封闭词表 `FileTreeMark`

```
elementRoot       本节点是某个元素的 path 根        「模块 X」
ambiguous         同长多命中（≥2 候选）              「多归属（2）」
member            位于某元素 path 严格内部            「属于 X」
containsElements  自身无归属，但后代含元素根          「含 N 个模块」
none              以上都不成立                       无标记
```

- 优先级单值：`elementRoot > ambiguous > member > containsElements > none`。
- `contains` 是正交计数，所以「属于 X · 含 1 个模块」可同时呈现（嵌套元素的父目录）。
- 配套 `ALL: [Self; 5]` + `as_str` 逐值对拍 + 长度断言。
- **不进「四面词汇对拍」**（那四面锚定模型 / 报告词汇，另一条生命周期），单独对拍。

---

## 4. 归属判定（逐节点）

```
输入：graph.elements（含 path）+ 磁盘源文件清单

① module_roots: HashMap<norm_path, Vec<fqn>>
     由所有 path 非空元素展开；归一 = OwnerIndex::normalize 同一实现（不另写一份）
② 对每个待出节点 p：
     if module_roots.get(p) is Some(fqns)  → elementRoot, owners = fqns
     else match owner_of(p):
       Owner::Element { fqn }              → member, owners = [fqn]
       Owner::Ambiguous(fqns)              → ambiguous, owners = fqns
       Owner::None                         → contains>0 ? containsElements : none
③ contains(p) = distinct FQN of module_roots 中 path 严格以 "p/" 开头的元素数
```

三个要点：

- **多 `path` 元素**（`App.Frontend.Shell` 4 条）产生 4 个 `elementRoot` 节点；`contains`
  按 FQN 去重，同一元素不重复计数。
- **生成物不是「天然归位」，需先修 `strip_generated`**（§12.1-② 订正）：现规则只做
  「替换扩展名」（`archi.i.rs` → `archi.rs`），而生成物落在**元素根同级**、不在目录内，
  故 `crates/infra/archi.i.rs` 现行判为 `None`。本变更按 **A2″**（§14）把判据改成
  「整组剥离 ∪ 替换扩展名」，改完才归 `member of App.Backend.Infra.ArchiRuntime`。
- **`ambiguous` 本仓预期为 0**（不存在同长重复前缀），但必须实现并测——它是判定单源的
  一部分，不实现就退化成「任选其一」。

本仓基线（A2″ + B + D5′ 定稿后实测，**475 节点 / 84.2 KB**）：

```
elementRoot         45   节点（29 目录形 + 16 文件形）  ← 对应 42 个元素，Shell 一元素占 4 个文件节点
member             373   文件 305 + 目录 68
containsElements    12   祖先目录（packages / …/src-tauri / …/crates/core 等）
none                45   无主文件 37 + 无标目录 8
ambiguous            0   结构上不存在同长重复前缀
文件 358 / 合成目录 117 / 补出根 0
```
42 个元素根**全部**仍有可见文件（最少 1 个，如 `Core.Config` / `Infra.Watch` / `Agent.*` 的单文件元素），
即「屏蔽测试面」不会让任何模块在树上变成空壳。

---

## 5. 树构建

```
① 枚举   walk_source_files（scan::walk_source_files 由私有提为 pub(crate)）
         + is_source_file 过滤（五扩展名）
         剪枝：hidden + gitignore + target / node_modules / dist / build / .git
         + **完整排除矩阵**（D5′：`ExcludeSet::build(extra)` 直接复用——
           默认 13 条测试面 + archi 配置 exclude，零新增入口）
② 合成   每个文件与每个 module_root 逐级上溯到根，去重成目录节点
③ 出节点 module_root 无文件也出（exists=false）
④ 排序   path 字节序；前端渲染「目录先于文件，各自按 name」
```
实测（D5′ + B 后）：文件 358 / 合成目录 117 / 补出根 0 → 节点 475（746 → 84.2 KB）。

---

## 6. UI 设计

### 6.1 布局

```
CanvasBody（现状）
[ 文件树 w-80 ] [ xyflow 画布 flex-1 ] [ 抽屉 w-72 ]
```

`ElementTree` 换成 `FileTree`，栏宽 72 → 80（文件名比 FQN 末段长）。画布与抽屉不动。

### 6.2 四级标记呈现

| mark | 图标 / 色 | 名称 | tooltip |
|---|---|---|---|
| `elementRoot` | 实心圆点 + 主色 | 加粗 | `模块 App.Backend.Core.Archi · gate off` |
| `ambiguous` | 警示色圆点 | 加粗 | `多归属：A、B` |
| `member` | 左侧竖线 + 次级色 | 常规（弱化） | `属于 App.Backend.Core.Archi` |
| `containsElements` | 空心圆 + 计数 | 常规 | `含 7 个模块`（尾随小字 `·7`） |
| `none` | 无 | 常规 | — |

树底部常驻一行图例（一行字，可折叠）：**仅显示判定面源文件**（测试文件与架构配置
`exclude` 命中项不在树中；`none` 行 = 无元素认领）。

### 6.3 点击与双向联动

- 树 → 画布：`elementRoot` / `member` 行点击 = `setFocusOverride(owners[0])`（现状）；
  `ambiguous` / `containsElements` / `none` **只切展开、零 onFocus**（已裁决：不做候选弹层——
  本仓 `ambiguous` 恒 0，为一个只可能出现在合成语料里的分支新增 popover 原语不划算；
  将来真要放开，DTO 里 `owners` 已是全量候选，只改点击处理即可）。
- 画布 / 抽屉 → 树：`focus` 变化时按该元素 `path`（可多条）展开祖先链、滚动到第一条、
  多节点同时高亮（圆点 + `bg-muted` 行底）。
- `focus` 为容器元素（`path` 空）：树顶部一行弱提示「该元素为容器，不持有文件」，不滚动。

### 6.4 状态

| 态 | 处理 |
|---|---|
| 加载中 | 「解析中…」（与画布同形） |
| 取数失败 | 行内 `archi-file-tree-error`，不阻断画布（树与图独立失败） |
| 树空（有模型、零源文件） | 「仓库内无源文件」 |
| 模型停摆 | 上层整块换 `StructureErrorPanel`，树不挂载（现状不变） |
| 展开态 | per-path 翻转集合（沿用 `info/components/dir-tree.tsx` 模式），默认展开前 2 段，刷新后按 path 保持 |
| 刷新 | 由 `ArchitectureView` 的 `refreshTick` → `key` 重挂覆盖，与 graph / drift 同步；零 watch 零轮询纪律不变 |

### 6.5 组件

`views/architecture/components/file-tree.tsx` 独立实现，**不与 `info/components/dir-tree.tsx`
抽公共件**：数据面不同（marks / owners vs 行数聚合），抽象需要泛型参数化，收益为零。

870 节点用「折叠不渲染 DOM」（dir-tree 既有做法）足够；实测卡顿再按可见行扁平化走
`@tanstack/react-virtual`（已在依赖内）。

---

## 7. 影响面

| 能力 | 影响 |
|---|---|
| `desktop-archi-canvas` | 「三段布局（元素树 / 画布 / 抽屉）」表述改为「文件树 / 画布 / 抽屉」；边标记、停摆、零 watch 零轮询全不变 |
| `desktop-archi-api` | **改写「归属规则剥离特例」+ 其 Scenario**：现 Scenario 里 `crates/core/archi.i.rs` 配目录形前缀按现行判据为 `None`（假金句），A2″ 后用并集判据才成立（§14.3） |
| `desktop-archi-drift` | 「文件归属」段补一条：生成物按并集判据归属（本仓 `gate` 全 `off` ⇒ 报告零变化；`gate != off` 的已声明元素其生成物从 `unmatchedFiles` 移入 `matched[]`） |
| `desktop-archi-ipc-type-bindings` | 新命令 1 条 + 新出线 DTO 3 个（`ArchiFileTree` / `ArchiFileEntry` / `FileTreeMark`、含 `FileEntryKind`）⇒ `bindings/mod_test.rs` 五处硬编码计数与 archi 族清单随动（§12.2） |
| `desktop-archi-model` | 模型格式零改动；**「四命令」段与「命令总数仍为五个」校正为六命令**（现状 5 → 6，§12.1-①） |
| `desktop-archi-module-docs` | 零改动；`FileTreeMark` 不进 `docs` 四源 |
| `desktop-archi-agent` | 零改动；不给 agent 加工具（`read` / `glob` 已够） |
| 写入面 | 零新增写命令，`archi_set_gate` 仍是唯一写 |

新增能力规格：`openspec/specs/desktop-archi-file-tree/spec.md`。

（能力章节口径：新增 1 个 = `desktop-archi-file-tree`；修改 5 个 = `desktop-archi-canvas` /
`desktop-archi-model` / `desktop-archi-api` / `desktop-archi-drift` / `desktop-archi-ipc-type-bindings`。）

---

## 8. 测试与验收

**Rust**（`tree_test.rs`，真实 tempdir + 真实 walker，沿 `docs_test` / `scan` 零 mock 先例）

```
目录型元素根 → elementRoot；其内文件 → member；祖先目录 → containsElements 且计数正确
单文件型元素根（welcome-view.tsx）→ 文件节点为 elementRoot
多 path 元素 → 多个 elementRoot，contains 按 FQN 去重
嵌套：子 path 是父 path 前缀 → 父子各 elementRoot，内层文件归最深层（最长前缀）
.i.rs 生成物 → member（A2″ 后：根同级整组剥离命中；含「同扩展名单文件根」不回归的对照）
A2″ 并集定档：根 src/dir/ 与 src/dir.ts 同时在场、探针 src/dir.i.ts → 归更具体者（单文件根胜出）
A2″ 零误剥：.i.py / .d.rs / 大写 .I.rs / 无扩展名 .i → 归属与改动前逐值相同
容器元素（path 空）→ 零节点
target / node_modules / .git / gitignore 命中项不出现
D5′：测试面（`*.test.tsx` / `*_test.rs` / `tests/` 目录）不出现，与 drift 扫描面逐条一致
D5′：config `exclude` 命中项不出现（`openspec/**` 本仓实测）
声明但目录不存在的模块根仍出节点（exists=false）
幂等：两次调用逐字相同
FileTreeMark::ALL 与 as_str 逐值对拍 + 长度断言
```

**前端**（`file-tree.test.tsx`）

```
目录先于文件、name 序
折叠默认前 2 段；展开不触发 invoke
五种 mark 各渲染对应 testid（archi-file-mark 的 data-mark）
点 elementRoot / member → onFocus(owner)；点 ambiguous / containsElements / none → 零 onFocus
focused 为多 path 元素 → 多行同时高亮
测试文件不在树中：注入含 `*.test.tsx` / `*_test.rs` 的图数据后零对应行（D5′）
config exclude 生效：注入 `.dev-team/architecture/config.json` 后该路径不在树中
空态 / 错误态（树取数失败不阻断画布）
```

**既有前端测试改写面**（§12.2 漏项，必须列入）
```
architecture-canvas.test.tsx：4 处 getByTestId('archi-element-tree')（385 / 817 / 837 / 931）
                              5 处 archi-tree-node 交互与停摆断言（373 / 583 / 587 / 774 / 833）
  ⇒ D7 切「文件」缺省后逐处改：元素树断言改用「元素」分段视图的 testid，
    文件树断言用新 testid（archi-file-tree / archi-file-node / archi-file-mark）
```

**门禁**：`bindings:export` 后提交两份生成物；`knip` 零新增 ignore；
`packages/desktop/package.json` patch +1。

---

## 9. 实施顺序（四个可独立提交的切片）

```
段 1 归属判据     strip_generated 改并集（A2″）+ owner_test 期望同步 + api/drift 两处 spec 口径
                  ↑ 独立可提交：纯判定面修正，本仓报告零变化（§14）
段 2 后端查询面  tree.rs + DTO + archi_file_tree + 单测 + bindings 重导出
                  + .dev-team/architecture/config.json（{"exclude":["openspec/**"]}，无 $schema）
                  + ExcludeSet 复用（零改动；D5′ 后不需要 build_config_only）
                  ↑ 此时架构页不变、无用户可见变更
段 3 前端替换    FileTree + use-archi-file-tree + CanvasBody 换栏 + 聚焦联动 + 单测
                  ↑ 用户可见，bump 版本
段 4 规格收口    新增 desktop-archi-file-tree spec
                  改 desktop-archi-canvas 布局表述
                  校正 desktop-archi-model「四命令 / 五个」→ 六命令
                  改 desktop-archi-api 的剥离特例 + Scenario（段 1 已改口径，此处落 spec）
                  改 desktop-archi-drift「文件归属」补并集判据
                  bindings 两份生成物入库（段 2 的 DTO 已出线）
```

---

## 10. 风险与备选

| 风险 | 处置 |
|---|---|
| 475 条 DTO（实测 **84.2 KB** 紧凑 JSON）与解析时间 | 已实测：节点 475（358 文件 + 117 目录）、`mark` 分布 `elementRoot 45 / member 373 / containsElements 12 / none 45`。体量已降到「无需懒加载」量级；仍超阈值则改 `archi_file_tree(root, dir)` 单层 + 前端缓存（DTO 形状不变） |
| graph 与 tree 两次独立读模型，极端并发编辑下可能不一致 | 同一 `refreshTick` 重挂内两次读；join 失败时 tooltip 退化为裸 FQN，不崩 |
| 用户把树上的灰点误读为违例 | 标记图例常驻一行；违例仍只在画布边与抽屉呈现 |
| 文件树隐藏 `openspec/` 等非源码面 | 已裁决走 archi 配置 `exclude`（§13.4）；本仓新增 `<archi_dir>/config.json`，连带 drift 的 `excluded` 面 +3 / `unmatchedFiles` −3 |
| 306 个测试文件在树上不可见，用户「在树里找测试文件」会落空 | D5′ 的显式代价（换「树面 = 判定面」零分叉）。图例常驻一句「仅显示判定面源文件（测试文件与 `exclude` 项不在树中）」；`ExcludeSet` 的命中留痕已在 drift 报告的 `excluded[]` 里，需要精确清单时看那里 |
| 两个元素根可能**只剩测试文件**（本仓实测无，最少 1 个可见文件） | 前向防御：元素根本身仍出节点（D6 的 `exists` 判据只看目录在不在盘），若某元素只剩测试文件，树上是「空目录节点」而非消失；`none` 计数会给 0，不误报 `pathNotFound` |
| `strip_generated` 判据改动会动到 drift 判定面 | 本仓 `gate` 全 `off` ⇒ 报告逐字段零变化（唯一生成物所在元素 `gate = off`，两分支同落 `unmatchedFiles`）；`gate != off` 时生成物移入 `matched[]`，须在两处 spec 写清（§14.2） |
| `walk_source_files` 提为 `pub(crate)` 扩大了内部可见面 | 它是既有单点、无第二份实现；提权限比复制一份遍历便宜 |

---

## 11. 明确不做

```
文件内容预览（右栏已有 archi_read_doc 通路，且它只认文档关联集内路径）
在树上标记漂移违例（违例归画布边与抽屉）
给 agent 新增 archi_file_tree 工具（read / glob 已够）
gate / 契约状态徽标（本轮只表达「哪里是模块」）
删除 ElementTree 组件（作为「元素」视图保留）
全量文件开关（含「显示测试文件」的反向开关——D5′ 已裁决屏蔽，将来要给的是
  「看判定面 / 看全量」两态开关，加开关即改，DTO 不变）
树里的测试文件行（D5′ 已裁决屏蔽；找测试文件走 grep 或 IDE）
```

---

## 12. 启动前复核（追加 · 对 §0–§11 的事实订正）

> 复算口径：读模型 6 份 toml 的 51 条元素（45 条 path）+ `git ls-files` 五扩展名源文件，
> 以 `scan/owner.rs` 的 `normalize` / `strip_generated` / `prefix_matches` 原样重放。
> 下列条目是**与代码相反**或**未登记**的，proposal 落地前须先按此订正，否则会写进 spec 成 MUST。

### 12.1 硬错误（5 条）

**① 命令数：现状 5，不是 6**

```
all_commands!  archi 段 = 5      archi_graph / archi_module / archi_change_summary / archi_set_gate / archi_drift
grep -c '^#\[tauri::command\]' src/commands/archi/mod.rs = 5
grep -c 'commands::archi::' src/commands/mod.rs        = 5
```
⇒ 本变更是 **5 → 6**，不是 §0/§3.1 写的 6 → 7。
且 `desktop-archi-model` 对同一事实有**两种**自称：标题与 Module Contract 写「四命令」，
自己的 Scenario 又写「命令总数仍为五个」。该项 delta 的正确写法是
「四命令 → 六命令 / 五个 → 六个」统一校正，MUST NOT 只改一处数字。

**② 「生成物天然归位」是反事实——`archi.i.rs` 恰是归不上的那一类**

```
prefix  crates/infra/archi/      normalize → "…/crates/infra/archi"        （尾 / 剥掉）
rel     crates/infra/archi.i.rs  strip_generated → "…/crates/infra/archi.rs"
prefix_matches:  rel == prefix ?           否
                 rel.starts_with(prefix+"/") ?  否（下一字符是 '.'）
⇒ Owner::None
```
非推断：`crates/core/archi/src/scan/owner_test.rs:437` 已把这条写成正例断言
（「目录形态元素根的根同级生成物剥后不在目录内 → 如实无主」）。
⇒ §4「`archi.i.rs` → member」与 §8「`.i.rs` 生成物 → member」两句不成立，见 12.3-A 待裁决。

**③ `openspec/` 会进树（三处断言均不成立）**

```
git ls-files ∩ 五扩展名 ∩ openspec/
  openspec/changes/archive/2026-09-28-agent-component-ai-sdk/prototypes/{index,agent-input,agent-messages}.tsx
```
三者非隐藏、未被 gitignore ⇒ `walk_source_files` 照收。
§5 的枚举面只有「walker + 扩展名过滤」，没有路径前缀排除 ⇒
§1「openspec/ 不入树」/ §10 风险行 / §11 明确不做 三处均落空。

**④ 计数混了两种口径**

| 口径 | 实测 | 笔记写的 |
|---|---|---|
| 元素总数 | 51 = 42（path 非空）+ **9**（path 空） | 51 = 41 + 10 |
| 容器元素 | 8 `domain` + 1 `package`（App / App.Backend / App.Frontend / Core / Infra / Tools / Views / Components / Components.Standard） | 10 |
| `path` 条目 | 45 = **29 目录形 + 16 文件形** | 未区分 |
| `elementRoot` 节点 | **45**（节点数） | 42 |
| `elementRoot` 元素 | **42**（Shell 一元素占 4 个文件节点） | 42（与上混用） |

§4 尾句「42 个 elementRoot（45 条 path，Shell 占 4）」把节点数与元素数说成一句，spec 须分开写。

**⑤ D5（不套测试排除矩阵）已作废 → D5′（套用完整矩阵）**

```
源文件 667（含 openspec 3）  其中测试面 306 = 45.9%
  *_test.rs 162  *.test.ts[x] 142  mod_test 1  perf 1
```
原取向（不套矩阵）会让树里近半节点是测试文件；**用户裁决改为屏蔽测试面**（D5′），
故本条从「后果未登记」升级为「已关闭」。随之变化的数字：

```
进树文件 667 → 358（−306 测试面 −3 openspec）      目录 194 → 117（消失 77，多为 corpus 语料链）
树节点   861 → 475                                紧凑 JSON 159.8 KB → 84.2 KB
无主文件  72 → 37（`src/lib` 22 / `src/components` 剩余 8 / `src/hooks` 12 中的非测试部分 …）
42 个元素根全部仍有可见文件，无空壳（最少 1 个）
```
`none` 行仍需图例一句解释（无主目录 8 个：`src/lib` / `src/hooks` / `src/types` /
`src/types/generated` / `src/components/file` / `src/components/standard/hooks` /
`crates/tools/specta-zod-patched`(+`/src`)，这些是 `decisions/0001` §7 的**封闭排除面**，
本来就不建元素——树如实呈现「这里没有模块」，不是缺陷）。

### 12.2 影响面漏项（不改会在实施期卡住）

| 漏项 | 证据 | 处置 |
|---|---|---|
| **`desktop-archi-ipc-type-bindings` 未列入影响面** | `src/bindings/mod_test.rs` 硬编码 `COMMAND_NAMES.len() == 48`（3 处：1047 / 1951 / 2423）、`DTO_TYPES.len() == 97`（1107）、`ARCHI_COMMANDS[..; 4]`（1652）、`ARCHI_DTO_TYPES[..; 19]`（1660）、尾部追加 vec 断言（1714–1726） | 新命令 +2~3 个 DTO 会让上述断言全红 ⇒ 该能力 MUST 进「修改的能力」，§7 的「零改动」表述作废 |
| 前端既有测试未列 | `architecture-canvas.test.tsx` 4 处 `getByTestId('archi-element-tree')`（385 / 817 / 837 / 931）、5 处 `archi-tree-node` 交互与停摆断言（373 / 583 / 587 / 774 / 833） | D7 默认切「文件」后这些断言必改；§8 只列了新 `file-tree.test.tsx` |
| `ambiguous` 点击候选弹层无测试行 | §6.3 写了「弹候选选择」，§8 前端用例表无对应 | ✅ 已裁决降级「只切展开」：§6.3 / §8 已同步，不新增 popover 原语 |
| D6 `exists = false` 无真实证据 | 45 条 path 磁盘**全部存在**（`os.path.exists` 逐条 0 miss） | 只能经合成 tempdir 测，spec 须写明它是前向防御分支 |
| 容量现值可钉死 | D5′ 前 861 节点 159.8 KB；**D5′ 后 475 节点 84.2 KB** | §4 / §10 已换成定稿值（84.2 KB），体量降到无需懒加载的量级 |
| `desktop-archi-canvas` 改动面比 §7 一行更大 | 同 spec 另有三处「元素树零节点」（46 / 55 / 60 行语境）、Module Contract 表 `element-tree.tsx` 行 | D7 保留 ElementTree 作「元素」视图 ⇒ 须逐处点明「改 / 不改」，不能只改 §20 那一行 |

### 12.3 待裁决（唯一会动到判据/归属面的两点）

```
A  生成物归位                                              ← 【已裁决：A2″（下沉 OwnerIndex，双形态并集）】
   ├ 接受无主（A1）：archi.i.rs 显示 none（与漂移判定完全同源，D1 纪律零妥协）
   ├ 下沉 OwnerIndex 双形态并集（A2″）                                  ✅ 取此
   └ 树侧加规则（A3）：与 drift 归属分叉 → 撞 D1，否
      落地形态 / 断言翻转 / spec 改写面：见 §14

B  openspec/ 排除方式                                        ← 【已裁决：采用配置方式】
   ├ 路径前缀硬排除（openspec/ 一条）
   └ 复用 archi 排除配置（<archi_dir>/config.json 的 exclude 追加一条）  ✅ 取此
      差别：前者单点写死，后者把「树里有什么」变成可配置面
      落地形态与连带后果：见 §13.4
```

### 12.4 启动前 checklist

```
[x] §0 / §3.1 命令数改 5 → 6；desktop-archi-model 的「四命令 / 五个」统一为六
[x] §4 / §8 删「.i.rs → member」，按 12.3-A 定稿后改写
[x] §5 枚举面补一条 openspec/ 排除判据（按 12.3-B 定稿）
[x] §1 计数按 12.1-④ 拆成「元素 / 条目 / 节点」三行
[x] §10 风险补「树近半是测试文件」「72 个 none 行」→ 后由 D5′ 改为「306 个测试文件不可见」+「37 个 none」
[x] §7 影响面加 desktop-archi-ipc-type-bindings（五处硬编码计数随动）
[x] §8 测试表补 architecture-canvas.test.tsx 改写项（ambiguous 弹层用例 → 已按 D9 改为零 onFocus 断言）
[x] §7 能力章节拆「新增能力 / 修改的能力」两栏，与计划落的 spec 一一对应
[x] §13.2-A 已裁决 A2″：落地清单见 §14（§4 / §7 / §8 已按此改写）
[x] §13.4-B 已裁决配置方式：落地清单见 §13.4-B1~B5
[x] 用户裁决 ① `ambiguous` 点击降级「只切展开」→ D9（§6.3 / §8 已同步）
[x] 用户裁决 ② 树屏蔽测试文件 → D5′（§1 / §4 / §5 / §8 / §10 / §13.4 已同步；
    `ExcludeSet` 零改动、`build_config_only` 计划作废、节点数 861 → 475）
```

---

## 13. 12.3-A 详解（生成物归位）+ 12.3-B 落地（配置方式）

### 13.1 A：`<root>.i.<ext>` 为什么一定归不上（机制层）

**A-1 现规则做了什么**

`scan/owner.rs::strip_generated` 的判据是**替换扩展名**，不是剥离后缀：

```
末段 <stem>.i.<ext>   →   <stem>.<ext>          "archi.i.rs" → "archi.rs"
（ext ∈ {rs, ts, tsx, js, jsx}，marker 恰为 "i"，module 非空）
```
然后交给 `prefix_matches(rel, prefix)`，判据只有两条：`rel == prefix` 或 `rel` 以 `prefix + "/"` 开头。

**A-2 生成器产出的是「根同级」，两种形态都不满足上述判据**

`api/paths.rs::generated_path` = `root.join(format!("{element_root}.i.{ext}"))`，其中 `<root>` 按
`normalize_root` 归一（剥 `./` 与尾 `/`，**保留文件扩展名**）。逐形态复算：

```
① 目录形根（29 条）   根 crates/infra/archi      生成物 crates/infra/archi.i.rs
   现规则 →  crates/infra/archi.rs
   == prefix ?            否
   starts_with "archi/" ? 否（下一字符是 '.'）        ⇒ Owner::None

② 单文件形根（16 条） 根 views/welcome-view.tsx  生成物 views/welcome-view.tsx.i.ts
   现规则 →  views/welcome-view.tsx.ts
   == prefix ?            否
   starts_with "…tsx/" ?  否                          ⇒ Owner::None

③ 唯一能命中的形态   根 .../views/architecture（无扩展名文件形）
   生成物  views/architecture.i.ts → views/architecture.ts ≠ views/architecture（目录形）
   只有在「根 = views/architecture.ts 且生成物为 .i.ts」时才全等——本仓没有任何元素是这个形状
```
**全量复算：45 条 path 条目 × 各自的 `<root>.i.<ext>` = 0 归位 / 45 无主。**
本仓磁盘上真实存在的生成物只有 1 个（`crates/infra/archi.i.rs`），归 `App.Backend.Infra.ArchiRuntime`，
按现规则 = **无主**。

**A-3 为什么既有测试没抓到**

`owner_test.rs:354 生成物剥离四种形态各归其元素` 的四条正例探针是**目录内双层形态**：

```
views/architecture/architecture.i.ts  →  views/architecture/architecture.ts   ← 在目录内，前缀命中
crates/core/archi/archi.i.rs          →  crates/core/archi/archi.rs           ← 在目录内，前缀命中
```
即它测的是「生成物落在元素目录**里**」——而 `generated_path` 的落点是「元素根**同级**，不在目录内」
（`paths_test` 自己的断言：`path.parent() == root.join(element.path[0]).parent()`）。
**测试与生成器覆盖的是两个互不相交的形状。**

同一文件的 `:437` 则把真实形态写成了**负例**：

```
directory.owner_of("src/dir.i.ts")  根 src/dir/  →  期望 Owner::None  "如实无主"
```
这条断言在「目录形根 + 根同级生成物」上把 bug 固化成契约。

**A-4 `desktop-archi-api` spec 的对应 Scenario 现为假金句**

spec 第 219 行 Scenario 声称四个探针「各归其元素」，其中
`crates/core/archi.i.rs` 配前缀 `crates/core/archi`（目录形）—— 按 A-2 复算为 `None`。
该 Scenario 只在「根为**同扩展名单文件**且生成语言一致」时成立，而本仓 45 条 path 无一条是那个形状
（29 目录形 + 16 个 `.tsx` 单文件，`.tsx` 根生成 `…tsx.i.ts` 剥成 `…tsx.ts` 仍不等）。
⇒ 这条 Scenario 不是本变更引入的问题，但它决定了文件树上那一个点长什么样，**必须在本次一并定稿**。

### 13.2 A 的四个选项（含改动面与代价）

```
                         改动点             本仓归属差异  既有断言翻转   与 drift 的关系
A1  接受无主             零代码              —            0 条          完全同源
A2  下沉 OwnerIndex      改 strip_generated   1 个文件     5 条（13.3）  完全同源（同一函数）
    纯整组剥离 .i.<ext>   （替换 → 剥离）     592→593 有主 1 修复 4 回归
A2″ 下沉 OwnerIndex      加一条并集分支       1 个文件     1 条（13.3）  完全同源（同一函数）
    双形态并集            （旧新各试一次）     592→593 有主 恰是 bug 本身
A3  树侧加规则           只在 tree.rs 里写     1 个文件     0 条          分叉（D1 禁忌）
A4  树里过滤 .i.*        枚举面加一条         不显示       0 条          不涉及（树不再完整）
```

**A2 的具体形态（剥离候选：整组剥离 vs 替换扩展名）**

```
末段 …<name>.i.<ext>  →  …<name>            archi.i.rs  → archi
                                            welcome-view.tsx.i.ts → welcome-view.tsx
```
复算（D5′/B 之前的全量 664 个非隐藏源文件；规则差异与排除面无关）：

| 规则 | 有主 | 无主 | 归属结果变化的文件 |
|---|---|---|---|
| 现行（替换扩展名） | 592 | 72 | — |
| A2（整组剥离） | 593 | 71 | `crates/infra/archi.i.rs`（唯一一个） |
| A2″（双形态并集） | 593 | 71 | 同上（本仓无可区分样本） |

（进树枚举面按 D5′ 缩到 358 文件后，同一差异仍只是那一个文件：
`archi.i.rs` 是仓内唯一生成物，且不在测试面内。）

- 目录形根：`crates/infra/archi.i.rs` 剥成 `crates/infra/archi` ⇒ `rel == prefix` **命中**。
- 单文件形根：`views/welcome-view.tsx.i.ts` 剥成 `views/welcome-view.tsx` ⇒ 全等 **命中**。
- 目录内双层形态（既有四条正例）：剥成 `…/archi/archi`，仍在目录内 ⇒ **照旧命中**（零回归）。
- 既有负例 `src/dir.i.ts`（根 `src/dir/`）：剥成 `src/dir` ⇒ 全等 **从 None 翻成 Element**
  ——这条断言正是「把 bug 写成契约」的那一条，翻转是修复本身，不是回归。

### 13.3 A2 的回归面（逐条复算，别只信「翻转是修复」）

把 `owner_test.rs` 里全部 19 条 `strip_generated` 相关断言逐条按三种规则重放：

```
规则                  翻转条数   翻转的是哪几条
现行                  0 条       —
A2 纯整组              5 条      views/welcome.i.ts(根 views/welcome.ts)      期望 True → 变 False  ❌ 回归
                                 src/a.i.ts(根 src/a.ts)                      期望 True → 变 False  ❌ 回归
                                 src/my.mod.i.rs(根 src/my.mod.rs)            期望 True → 变 False  ❌ 回归
                                 src/dir.i.ts(根 src/dir/)                    期望 False → 变 True  ✅ 修复
                                 反斜杠形态 src\my.mod.i.rs                    期望 True → 变 False  ❌ 回归
A2″ 双形态并集（新规则 or 旧规则任一命中即算）
                      1 条      src/dir.i.ts(根 src/dir/)                  期望 False → 变 True  ✅ 修复
```

**为什么纯整组会破 4 条**：`.ts` / `.rs` 这类「根与生成物扩展名同族」的元素，旧规则
（`a.i.ts` → `a.ts`）恰好是**对的**；整组剥离（`a.i.ts` → `a`）会把那个 `.ts` 一起吃掉，
于是与元素根 `a.ts` 不再全等。换句话说：

```
生成物的两个落点形态是**不同**的：
  ① 根同级（generated_path）：<root>.i.<ext>     → 需要「整组剥离」
  ② 目录内（历史上/假设形态）：<dir>/<name>.i.<ext> → 需要「替换扩展名」
旧规则只处理 ②，新规则只处理 ①，**单条规则覆盖不了两个形态**。
```

⇒ 若选 A2，正确形态是 **A2″：两条判据取并集**（先试整组剥离，再试替换扩展名，任一命中即归属）。
它的代价只有 1 条断言翻转，而那条恰好是把 bug 写成契约的 `src/dir.i.ts`。
若坚持「只留一条判据」的洁癖，代价是 4 条既有断言从绿变红、必须逐条改写期望
（并要说服评审「这些不是回归」）——**成本高于收益，故 A2 应取并集形态**。

**A2 对漂移报告的影响（关键，且本仓恰好为零）**

```
本仓唯一生成物的元素 App.Backend.Infra.ArchiRuntime  gate = off
  Owner::None            → unmatched_files.push
  Owner::Element{off}    → unmatched_files.push     ← 同一分支结果
  生成物内容零 use 语句（grep 零命中）⇒ 抽取零事实 ⇒ 零边
  matched_files 只在 gate != off 时写入 ⇒ 不变
⇒ 本仓 drift 报告逐字段、逐序不变（unmatched_files 集合与序都不动）
```
推广到「gate = warn 的已声明元素」时**会变**：其生成物从 `unmatchedFiles` 移入 `matched[]`。
这是判定面的语义扩张，须在 `desktop-archi-api` 的「归属规则剥离特例」与
`desktop-archi-drift` 的「文件归属」两处同时写清，MUST NOT 只在树侧实现。

**A3 为什么是禁忌**：树与 drift 对同一文件给出不同归属（一个说 member、一个说 unmatched），
正是 D1「判定单源」要防的那种分叉；且前端/后端各有一套归属规则时，`ambiguous` 分支必然只在一侧实现。

**A4 为什么不推荐**：与 D5′ 的取向冲突（D5′ 已把「进树判据」统一为漂移排除矩阵），
再为生成物单开一条隐藏规则会让「什么进树」变成两条互不相干的判据。

**决策驱动量（不是审美）**

```
现状：契约已声明 1 个元素（INDEX.toml：declared 1 / absent 41）
无主生成物节点 = 1 个  → A1 的可接受度极高

若 42 个元素全部 declared（本变更的邻接目标）：
  无主生成物节点 = 42 个，每个都紧挨着自己的模块目录/文件出现一个灰点
  → A1 从「一个可忽略的边角」变成「系统性的视觉噪声」
```
⇒ **若本轮只交付 1 个试点元素，A1（接受无主 + 记为已知缺口）成本最低；
若同期或近期要推契约到全仓，A2 应一并做掉**——它是判定面的一次性修正，
越晚做则「树上有 42 个灰点」会先被当成树的功能缺陷来报。

**裁决结果（用户已定）**：取 **A2″** —— 把 `strip_generated` 改成「整组剥离 ∪ 替换扩展名」
两条判据取并集，同步翻转 `owner_test` 的 `src/dir.i.ts` 一条期望，并改写 `desktop-archi-api`
那条 Scenario。落地定稿见 **§14**。

### 13.4 B 已裁决：走配置方式——落地形态与必须登记的连带后果

**B-1 数据流**

```
archi_file_tree_inner(root)
  ① 读模型（同 archi_module 口径：结构诊断非空 → Err）
  ② config::load_config(root, known)                 ← 只取 exclude，ignore 不参与
  ③ ExcludeSet::build(&config.exclude)               ← 既有入口，默认 13 条 + config（D5′）
  ④ walk_source_files + is_source_file 过滤
  ⑤ 对每个候选文件：matched(rel).is_some() → 丢弃
```
**B-2 无需新增入口**（D5′ 裁决后的简化）

原计划要加 `build_config_only(extra)`，理由是「树不吞测试文件」。**D5′ 已改为吞**
（树面 = 判定面），故 `scan/exclude.rs` **零改动**——既有 `build(extra)` 的语义
（默认 13 条在前 + config 追加在后、`matched()` 最小下标优先）正是要的。
`exclude_test` 亦零改动。

**B-3 本仓要新建一个此前不存在的文件——且它不能带 `$schema` 键**

```
.dev-team/architecture/config.json    （现在不存在，只有 api / decisions / models 三个子目录）
{ "exclude": ["openspec/**"] }
```
**`$schema` 一律不得添加**：`drift/config.rs` 的 `RawConfig` 带 `#[serde(deny_unknown_fields)]`，
且只登记 `ignore` / `exclude` 两键（`$schema` 无 `rename` 也无容忍分支）。写 `"$schema": …`
会让 `load_config` 直接 `Err("架构域配置解析失败…unknown field")`，而该 `Err` 会同时打断
**drift 命令、门禁 runner 与本次新增的树**三条消费面。
⇒ 与工作区 `.dev-team/config.json`（带 `$schema`）形态不同，两者不可类比。

连带效应（必须在 proposal 里如实登记，不能说「零改动」）：

```
drift 的扫描面随之变化：
  openspec/changes/archive/…/prototypes/{index,agent-input,agent-messages}.tsx  3 个文件
  现在：unmatchedFiles +3
  之后：excluded +3、unmatchedFiles −3
  已核：无既有测试对真仓计数断言
        golden 走合成语料（corpus/drift/*）、perf 走合成夹具（1000 文件）、pilot 只读 api 资产（不调 drift / scan）
  ⇒ 不破测试，但「新增配置文件」本身是一条要写进 spec 的动作
```

**B-4 语义边界（写 spec 时必须点明）**

- 树读的是 `<archi_dir>/config.json` 的 `exclude`，**不是**工作区 `.dev-team/config.json`（D1/配置隔离纪律零触碰）；
- `archi_file_tree` 因此成为「读一个配置键」的只读命令。§3.1 原句「不读工作区 config.json」仍成立，但要改写成「只读架构域配置的 `exclude` 一个键」；
- 配置非法（无通配符 / glob 编译失败）→ 树 `Err` 行进错误态，**不阻断画布**（§6.4 既有处置）；
- **树上可见 = 参与判定**（D5′）：测试文件与 `exclude` 命中项**都不在树中**，故不再存在「看得见却不参与判定」的错位；图例只需说明「仅显示判定面源文件」。

**B-5 `openspec/**` 的实测效果**

```
D5′ + B 合计效果（两次排除叠在一起才是最终枚举面）：
  文件   667 → 358   （−306 测试面 −3 openspec）
  目录   194 → 117   （消失 77：openspec 链 5 + corpus 语料链 60+ + crates/*/tests 3）
  节点   861 → 475
  紧凑 JSON 159.8 KB → 84.2 KB
只算 B（config exclude 单独）：文件 −3 / 目录 −5 / ≈−1.3 KB
```

---

## 14. A2″ 落地定稿（已裁决）

> 裁决：**A2″——把剥离判据下沉 `OwnerIndex::strip_generated`，改成
> 「整组剥离 ∪ 替换扩展名」两条判据取并集**。理由是树与 drift 必须对同一文件给出同一归属
> （D1 判定单源）；树侧另写规则会立刻分叉（A3 已否）。
> 本节把落地形态、断言翻转面、spec 改写面逐条写死，供 proposal 直接引用。

### 14.1 判据形态（单点，唯一实现）

```
现状（只处理「生成物在元素目录内」这一形态）：
    <stem>.i.<ext>          →  <stem>.<ext>          "archi.i.rs" → "archi.rs"
A2″（补上「生成物在元素根同级」这一形态）：
    候选1  整组剥离         <name>.i.<ext>  →  <name>       "archi.i.rs" → "archi"
    候选2  替换扩展名        <stem>.i.<ext>  →  <stem>.<ext> "archi.i.rs" → "archi.rs"
    命中判据 = 任一候选满足 prefix_matches（rel == prefix 或 rel.startsWith(prefix + "/")）
```
三条实现纪律：

- **仍只此一处**：`normalize` / `prefix_matches` / `OwnerIndex::build` 语义逐字不动；
  改动集中在 `strip_generated`（或其调用点：需要一个「返回多个候选」的形态）。
- **形态二选一由测试钉住**：`fn strip_candidates(rel) -> Vec<String>` 返回 1~2 个候选，
  `owner_of` 对每个候选各跑一遍最长前缀、合并命中集再按长度定档（同长仍归 `Ambiguous`）。
  MUST NOT 实现成「候选1 命中就用候选1」——那会让 `src/dir.i.ts`（候选1 全等 `src/dir`、
  候选2 得 `src/dir.ts`）只走一条路径，与并集语义不符。
- **候选顺序不影响结果**：命中集按 `(fqn, prefix 长度)` 合并去重后定档，
  与候选遍历顺序无关（确定性由排序保证，不由顺序保证）。
- 末段不是 `.i.<ext>`、或白名单外扩展名（`.i.py` / `.d.rs` / 大写 `.I.rs`）→ 返回单个
  原路径（零误剥，既有反例照旧）。

### 14.2 对 drift 的实际影响（本仓零，须在两处 spec 写清）

```
本仓唯一生成物：crates/infra/archi.i.rs  所属元素 gate = off
  A2″ 前  Owner::None            → unmatchedFiles.push
  A2″ 后  Owner::Element{off}    → unmatchedFiles.push          （同一分支，结果不变）
  matched 只在 gate != off 时写入                                 （不变）
  生成物内容零 use 语句（实测 18 行、`use` 零命中）⇒ 抽取零事实 ⇒ 零边
⇒ 本仓 drift 报告逐字段、逐序不变（unmatchedFiles 集合与序都不动，matched[] 不变）

推广（写进 spec 的 MUST）：
  gate != off 的已声明元素，其生成物 A2″ 后由 unmatchedFiles 移入 matched[]
  ⇒ 这不是「树的特权」，而是判定面的一次语义修正：生成物本就属于该元素
  ⇒ desktop-archi-api「归属规则剥离特例」与 desktop-archi-drift「文件归属」两处同改
```
另：`owner_of` 的**目标侧**同样受益（`scan/mod.rs:227` 对 `target_rel` 求归属），
即生成物在被 import 时也能作为合法 `to`。本仓生成物无人 import，故同样零变化。

### 14.3 `desktop-archi-api` 的改写点（现为假金句）

```
现 spec「归属规则 SHALL 加一条剥离特例」(第 212 行)：
  按「去掉 `.i` 后的路径」判定归属 —— 描述与实际实现（替换扩展名）不一致，须改成并集判据

现 spec Scenario「四种剥离形态各归其元素」(第 219 行)：
  探针 views/architecture.i.ts / views/welcome-view.i.ts / crates/core/archi.i.rs /
       crates/core/archi/src/scan.i.rs  × 目录形前缀
  按现行判据四条**全部为 None**（§13.1 复算），A2″ 后四条全命中
  ⇒ 改后这条 Scenario 才成立；同时**新增一条反例**：生成物不该因为 A2″ 而误归
     （`src/mod.i.py` 一类白名单外后缀仍不剥）
```
`owner_test.rs` 亦须同步（三处）：

| 位置 | 现状 | A2″ 后 |
|---|---|---|
| `:354` 四种形态各归其元素 | 4 条正例（目录内双层形态） | 保持绿；**另加**「根同级」4 条正例（含 `crates/infra/archi.i.rs` 真形态） |
| `:437` `src/dir.i.ts` 期望 `Owner::None`（「如实无主」） | 把 bug 写成契约 | **翻转为 `Owner::Element`**，断言改「根同级生成物归元素」 |
| `:446` 白名单外后缀零误剥 | 4 条反例 | 保持绿（并集不影响白名单） |

### 14.4 实施切片位置

A2″ 是**独立的第一个切片**（§9 段 1），先于树落地，理由：

```
① 它是纯判定面修正、无用户可见变更，可单独过门禁（clippy / test / api:check）
② 树以它为前置：若先做树、再做 A2″，树的第一版会被写进「生成物无主」的期望里，
   随后 A2″ 让该期望翻转——等于同一行为在两次变更里各定稿一次
③ drift 报告本仓零变化（§14.2），故它不阻塞任何既有门禁
```

### 14.5 A2″ 的残余风险

| 风险 | 处置 |
|---|---|
| 并集下「同为合法的两个候选」可能落到不同元素，且**顺序敏感的做法会给出不同答案** | 实测用例：元素根 `src/dir/` 与 `src/dir.ts` 同时在场、探针 `src/dir.i.ts`——整组剥离得 `src/dir`（命中 `App.Dir`）、替换扩展名得 `src/dir.ts`（命中 `App.DirTs`，前缀更长）。**并集按最长前缀定档 ⇒ 恒归 `App.DirTs`；若实现成「候选1 命中就短路」⇒ 归 `App.Dir`**。故用例断言「更具体的单文件根胜出」，用它钉住实现 MUST 走命中集合并而非候选短路 |
| 未来若生成物落点改成「入口同级」（而非元素根同级），并集会静默继续工作、掩盖落点漂移 | 落点由 `paths::generated_path` 单点派生、`paths_test` 三形态断言守着；并集只解决归属，不改落点判据 |
| 并集让 `strip_generated` 从「纯函数一进一出」变成「一进多出」，调用点需改 | 若坚持一进一出，可改为「由 `owner_of` 对两个候选各跑一遍最长前缀、合并命中集」，语义等价；**唯独 MUST NOT 写成候选短路**（上一条用例即判红面）。真实同长歧义（两个元素根字面相同 / 等长）仍归 `Owner::Ambiguous`，与改动前同 |
