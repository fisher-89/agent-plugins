# desktop-archi-file-tree Specification（delta: archi-file-tree）

## Purpose

定义 `/architecture` 三段布局最左栏的**文件树**：以仓库磁盘枚举为节点来源、在树上标出哪些目录 / 文件是元素根（元素 `path` 条目），点击模块即以既有 1-hop 语义圈定范围。本能力持有文件树的命令与 DTO、归属标记词表、归属判定与判定单源、枚举面与漂移判定面一致、元素根缺失的前向防御、左栏呈现（分段切换 / 双向联动）与取数状态面。画布、抽屉、漂移标记、门禁与写入面不在本能力。

## ADDED Requirements

### Requirement: 文件树命令与扁平条目投影

后端 SHALL 提供一条读取命令 `archi_file_tree(root: String) -> Result<ArchiFileTree, String>`，把仓库磁盘上的源文件与模块根投影为**扁平条目数组**：

- **薄包装形态**：命令 SHALL 为薄包装（`check_root` → 调领域函数 → 映射错误），领域装配在 `archi_file_tree_inner(root: &Path)` 内以便离 Tauri 测试（沿 `commands/archi/` 既有五个 `*_inner` 形态）；命令层 MUST NOT 承载归属判定、目录合成或标记推导。
- **只读**：命令 MUST NOT 写盘（唯一写命令仍是 `archi_set_gate`），MUST NOT spawn 子进程，MUST NOT 引入 watch 或轮询。
- **结构诊断非空即 `Err`**：模型含任一结构诊断时命令 SHALL 返回 `Err`（与 `archi_module` 同口径），MUST NOT 返回部分树——停摆态下画布整块被结构错面板替换，树本就不挂载。
- **只读一个配置键**：命令读取的配置 SHALL 仅为架构域配置（`<archi_dir>/config.json`，经 `foundation::layout::archi_config_path` 派生）的 `exclude` 一个键——读取走 `desktop-archi-drift` 既有 `config::load_config(root, known)` 入口（`known` = 模型元素 FQN 集），取其 `exclude`，`ignore` 不参与树；配置文件缺失即空配置（不报错）。MUST NOT 读工作区 `.dev-team/config.json`（配置隔离纪律零触碰），MUST NOT 为树另写第二个配置解析器。
- **DTO 扁平且确定**：`ArchiFileTree { entries: Vec<ArchiFileEntry> }`，条目 SHALL 为扁平数组（MUST NOT 嵌套），按 `path` 字节序排列（同输入同输出，确定性）；嵌套关系由前端按 `path` 前缀派生。
- **条目七字段**：`ArchiFileEntry` SHALL 恰为

  | 字段 | 类型 | 语义 |
  |---|---|---|
  | `path` | `String` | repo 相对 POSIX 路径（无前导 `./`、无尾 `/`）；顶层条目的 `path` 即其单段名（不单独产出 `""` 根节点条目） |
  | `name` | `String` | 末段名 |
  | `kind` | `FileEntryKind` | `Dir` \| `File`（封闭二值 + `ALL` + `as_str`） |
  | `mark` | `FileTreeMark` | 归属标记五值（见「归属标记封闭五值词表」） |
  | `owners` | `Vec<String>` | 归属 FQN（字典序去重）：`elementRoot` **恰 1**（同一归一路径被 ≥ 2 个元素声明时按 `ambiguous` 处置，见「归属判定」）、`ambiguous` ≥ 2、其余空数组 |
  | `contains` | `u32` | 后代模块数（按 FQN 去重，不含自身）；`kind = File` 恒 `0` |
  | `exists` | `bool` | `Dir` 且为模块根时该目录是否在盘（其余恒 `true`） |

- **出线形状**：两者 SHALL 带 `specta::Type` 与 `serde(rename_all = "camelCase")`，经 `bindings:export` 生成到 `bindings.ts` / `bindings.zod.ts`；字段序即声明序，投影 MUST NOT 重算、MUST NOT 补默认值。
- **空模型不是错误**：模型目录缺席或零元素时，命令 SHALL 正常返回（全部条目 `mark = none`、`owners` 空）——冷启动的空模型是合法起点，MUST NOT 报错、MUST NOT 与「模型有结构错」混为一类。
- **命令总数**：archi 段命令由 5 条增至 **6** 条（`archi_graph` / `archi_module` / `archi_change_summary` / `archi_set_gate` / `archi_drift` / `archi_file_tree`），登记进 `all_commands!` 单一登记面。

#### Scenario: 扁平条目按 path 字节序且幂等

- **WHEN** 对同一 root 连续两次调用 `archi_file_tree`
- **THEN** 两次返回的 `entries` 逐字节相同；数组为扁平结构、`path` 单调不减；同一文件与同一目录各恰一条（目录由文件与模块根上溯合成，去重）

#### Scenario: 结构诊断非空即 Err

- **WHEN** 模型含一处环（或未定义引用 / 孤儿层级等任一结构诊断）时调用 `archi_file_tree`
- **THEN** 返回 `Err`（文案含诊断条数或首条诊断），零树条目返回；前端整块呈现结构错面板、树不挂载

#### Scenario: 只读零写盘且只读一个配置键

- **WHEN** 调用 `archi_file_tree` 前后比对仓库与配置目录的字节与 mtime，并审查取数路径
- **THEN** 磁盘零变化；命令只经 `foundation::layout::archi_config_path` 读架构域配置的 `exclude`，源码零工作区 `.dev-team/config.json` 读取

#### Scenario: 空模型返回全 none 而非错误

- **WHEN** 模型目录缺席或 `models/` 为空时调用 `archi_file_tree`
- **THEN** 返回 `Ok`：条目集合为仓库源文件与合成目录，全部 `mark = "none"`、`owners` 空、`contains = 0`；MUST NOT 返回 `Err`（空模型是冷启动合法起点，与结构错不同类）

#### Scenario: 条目七字段按声明序出线

- **WHEN** 审查 `bindings.ts` 中 `ArchiFileEntry` 与 `ArchiFileTree` 的类型段
- **THEN** 前者恰七字段 `path` / `name` / `kind` / `mark` / `owners` / `contains` / `exists`（camelCase、声明序），后者恰一字段 `entries`；`FileEntryKind` 为 `"dir" | "file"` 判别

### Requirement: 归属标记封闭五值词表

文件树节点的归属标记 SHALL 由**新封闭词表** `FileTreeMark` 承载，恰五值：

```
elementRoot        本节点恰是一个元素的 path 根（owners 恰 1）
ambiguous          同长多命中 ≥ 2 个元素（含同一归一路径被多元素声明、以及同长不同前缀两种来源）
member             位于某元素 path 的严格内部
containsElements   自身无归属，但后代含元素根
none               以上都不成立
```

- **优先级单值**：判据 SHALL 按 `elementRoot > ambiguous > member > containsElements > none` 次序取首个成立者，`mark` MUST NOT 同时取多值。
- **`contains` 正交**：`contains`（后代模块数）SHALL 是与 `mark` 正交的计数，故「`member` 且含 N 个模块」（嵌套元素的父目录）可同时呈现。
- **不复用 `NodeMark`**：`FileTreeMark` MUST NOT 复用画布节点标记 `NodeMark`（后者归画布节点、生命周期在漂移报告）——同名不同义会让两处语义相互污染；两者各自带 `ALL` 与 `as_str`。
- **封闭性机械可验**：`FileTreeMark::ALL` SHALL 为五值常量并与 `as_str` 逐值对拍（长度断言防新增变体漏登）；`FileEntryKind::ALL` 同。
- **不进四面词汇对拍**：`FileTreeMark` MUST NOT 纳入 `desktop-archi-model`「消费面词汇集一致性对拍」的四面（那四面锚定模型 / 报告词汇，另一条生命周期）；本词表只由本能力自己的 `ALL` 对拍守护。
- **不承载漂移违例**：标记 SHALL 只表达归属，MUST NOT 表达违例 / 门禁 / 契约状态；违例仍只经画布边与抽屉呈现。

#### Scenario: 五值各渲其标记

- **WHEN** 树条目依次为某元素根、同长多命中项、元素内文件、含模块的祖先目录、无主文件
- **THEN** 五条分别取 `elementRoot` / `ambiguous` / `member` / `containsElements` / `none`；`owners` 依次为 1 条 / ≥ 2 条 / 1 条 / 空 / 空

#### Scenario: 优先级单值

- **WHEN** 某节点既是元素根（`path` 命中）又位于父元素 `path` 内部
- **THEN** 其 `mark` 取 `elementRoot`（不取 `member`），`owners` 为该元素 FQN；父元素不因此多出 `contains` 计数外的任何表达

#### Scenario: 正交计数与标记并存

- **WHEN** 某目录位于父元素 `path` 内部，且其后代含 1 个嵌套元素根
- **THEN** 该目录 `mark = member`、`owners = [父元素]`、`contains = 1`；前端同时呈现「属于父元素」与「含 1 个模块」

#### Scenario: ALL 与 as_str 逐值对拍

- **WHEN** 遍历 `FileTreeMark::ALL` 与 `FileEntryKind::ALL`
- **THEN** 每项 `as_str` 产出唯一且与语料词一致（`elementRoot` / `ambiguous` / `member` / `containsElements` / `none`、`dir` / `file`）；新增变体未补入 `ALL` 时长度断言失败

### Requirement: 归属判定与判定单源

文件树的每个节点 SHALL 按**与架构漂移同一实现**的归属判据定档，树侧 MUST NOT 自建第二套归属 / 前缀匹配 / 生成物剥离：

- **模块根表**：SHALL 由全部 `path` 非空元素展开为 `norm_path → Vec<fqn>`，归一 SHALL 复用 `OwnerIndex::normalize` 同一实现（`./` 前缀剥除、尾 `/` 剥除、`\` → `/`）；MUST NOT 另写一份归一。
- **判据顺序**（逐节点）：
  ```
  ① 节点 path ∈ 模块根表：
       该表项恰 1 个 FQN              → elementRoot，owners = [该 FQN]
       该表项 ≥ 2 个 FQN（同路径多元素声明）→ ambiguous， owners = 全部 FQN（字典序）
  ② 否则 OwnerIndex::owner_of(path)：
       Element { fqn }                → member，      owners = [fqn]
       Ambiguous(fqns)                → ambiguous，   owners = fqns
       None                           → contains > 0 ? containsElements : none
  ③ contains(p) = 模块根表中 path 严格以 "p/" 开头的元素数（按 FQN 去重，不含自身——自身 path 恰为 p，不满足严格前缀）
  ```
- **元素根判据是 `path` 非空而非 `kind`**：`kind = domain` 但持有 barrel 元素根（如 `App.Frontend.Components.Agent` 持 `index.tsx`）SHALL 仍判 `elementRoot`。
- **多 `path` 元素**：一个元素的每条 `path` 条目 SHALL 各出 `elementRoot` 节点（每个节点 `owners` 恰一条该 FQN）；`contains` 按 FQN 去重，同一元素 MUST NOT 被重复计数。
- **`elementRoot` 的 owners 恰 1**：同一归一路径被 ≥ 2 个元素声明时该节点 SHALL 判 `ambiguous`（`owners` ≥ 2），MUST NOT 从中任选一个给出 `elementRoot`——该形态与「同长不同前缀」同为归属不确定，两者都不得任选。
- **嵌套最长前缀**：父子 `path` 前缀重叠时，内层文件 SHALL 归最深前缀者（既有最长前缀语义），父子各出各自 `elementRoot`。
- **容器元素零节点**：`path` 恒空的容器元素 MUST NOT 出现在文件树上（这是 `decisions/0001-model-granularity.md` §3 的直接后果，不是遗漏）；容器只在元素树（FQN 前缀派生）上可见。
- **`ambiguous` 必须实现**：同长多命中 SHALL 记 `ambiguous` 并保留全量候选于 `owners`，MUST NOT 任选其一；本仓预期恒 0，但分支 MUST 实现并由合成用例覆盖（不实现即退化成「任选其一」，与判定单源纪律冲突）。
- **生成物归属沿用并集判据**：`.i.<ext>` 生成物的归属 SHALL 由 `OwnerIndex` 的「整组剥离 ∪ 替换扩展名」并集判据给出（定义与断言面见 `desktop-archi-api`「生成物落点与入口查找」与 `desktop-archi-drift`「文件归属」），树侧 MUST NOT 另加剥离规则（否则树与漂移对同一文件给出不同归属）。

#### Scenario: 目录型元素根与其内部文件

- **WHEN** 元素 `App.Core.Archi` 的 `path = ["./crates/core/archi/"]`，盘上该目录含 `src/lib.rs` 与 `src/dto.rs`
- **THEN** `crates/core/archi` 节点 `mark = elementRoot`、`owners = ["App.Core.Archi"]`；`crates/core/archi/src` 与两个 `.rs` 文件 `mark = member`、`owners = ["App.Core.Archi"]`

#### Scenario: 单文件型元素根

- **WHEN** 元素 `App.Frontend.Views.Welcome` 的 `path = ["./packages/desktop/src/views/welcome-view.tsx"]`
- **THEN** 该文件节点 `mark = elementRoot`、`owners = ["App.Frontend.Views.Welcome"]`；同目录其余文件为 `member` 或按各自归属定档

#### Scenario: 多 path 元素与 contains 去重

- **WHEN** 元素 `App.Frontend.Shell` 声明四条文件形 `path`，且其中一条位于某祖先目录下
- **THEN** 四条各出 `elementRoot` 节点（`owners` 均为该 FQN）；该祖先目录的 `contains` 只计 1（按 FQN 去重），MUST NOT 计 4

#### Scenario: 嵌套父子各出根且内层文件归最深

- **WHEN** 父元素 `path = ["./crates/core/archi/"]`、子元素 `path = ["./crates/core/archi/src/scan/"]`，盘上 `crates/core/archi/src/scan/owner.rs` 在场
- **THEN** 两个目录节点各为 `elementRoot`（各有其 `owners`）；`owner.rs` 归子元素（最深前缀），父元素目录的 `contains = 1`

#### Scenario: 容器元素零节点

- **WHEN** 树构建输入含 `path` 为空数组的容器元素（`domain` / `package`）
- **THEN** 该元素在文件树上零节点、不产生任何 `owners` 引用；它只在元素树上以 FQN 节点呈现

#### Scenario: 同长多命中判 ambiguous 不任选

- **WHEN** 两个不同元素以同长度 `path` 前缀同时覆盖某文件（合成语料）
- **THEN** 该文件 `mark = ambiguous`、`owners` 为两个 FQN（字典序），MUST NOT 只给其一

#### Scenario: 同一归一路径被多元素声明判 ambiguous

- **WHEN** 两个元素声明同一个 `path`（如均写 `./src/shared/`，含 `./` 前缀与尾 `/` 两种拼法归一后相同）
- **THEN** 该路径节点 `mark = ambiguous`、`owners` 为两个 FQN；MUST NOT 判 `elementRoot`（`owners` 恰 1 才是 `elementRoot`）

#### Scenario: 生成物按并集判据归元素

- **WHEN** 盘上存在 `crates/infra/archi.i.rs`（元素根同级的生成物）且元素 `App.Backend.Infra.ArchiRuntime` 的 `path = ["./crates/infra/archi/"]`
- **THEN** 该文件节点 `mark = member`、`owners = ["App.Backend.Infra.ArchiRuntime"]`（不落 `none`）；树侧源码零第二份剥离 / 归一 / 前缀匹配实现

### Requirement: 枚举面与漂移判定面逐条一致

文件树的源文件枚举面 SHALL 与架构漂移的扫描面**逐条相同**——树上可见的 = 漂移实际判定的那批文件：

- **遍历**：SHALL 复用扫描单点的 gitignore 感知遍历（`scan::walk_source_files`，提为 `pub(crate)` 供本能力调用）与扩展名判据（`scan::is_source_file`，五种 `.rs` / `.ts` / `.tsx` / `.js` / `.jsx`）；MUST NOT 自建 `read_dir` 遍历、MUST NOT 另写扩展名清单。
- **剪枝**：隐藏项、gitignore 命中项、`target` / `node_modules` / `dist` / `build` / `.git` MUST NOT 进树（沿既有遍历器的 `filter_entry` 语义）。
- **排除矩阵**：SHALL 复用 `scan::exclude::ExcludeSet::build(archi_config.exclude)` 的完整矩阵（默认 13 条测试面约定 + 架构域配置追加），逐文件以 `matched(rel).is_some()` 丢弃；MUST NOT 新增「只配置不默认」的第二入口（`ExcludeSet` 语义即「默认在前、配置追加、最小下标优先」，正是所需）。
- **测试面不在树中**：`**/*.test.{ts,tsx,js,jsx}` / `**/*.spec.{ts,tsx,js,jsx}` / `**/__tests__/**` / `**/*_test.rs` / `**/tests/**` / `**/fixtures/**` / `**/__mocks__/**` 命中项 MUST NOT 出现（本仓实测源文件 667 → 进树 358，其中 306 个测试文件被排除）。
- **配置 `exclude` 生效**：架构域配置 `exclude` 命中项 MUST NOT 出现；非法配置（空白条目 / 无通配符 / glob 编译失败）SHALL 使命令 `Err`（文案沿用 `drift/config.rs` 与 `ExcludeSet::build` 的既有显式文案），树进错误态、MUST NOT 阻断画布。
- **目录合成**：每个进树文件与其所属模块根 SHALL 逐级上溯合成目录节点（去重）；**合成目录不需要在盘存在**（元素根可能声明了不存在或已被排除的目录）。

#### Scenario: 测试面不出现

- **WHEN** 盘上存在 `src/views/architecture/components/file-tree.test.tsx` 与 `crates/core/archi/src/tree_test.rs`
- **THEN** 两条均不在树中；同目录下的非测试源文件（`file-tree.tsx` / `tree.rs`）照常出现

#### Scenario: 配置 exclude 命中项不出现

- **WHEN** 架构域配置 `exclude = ["openspec/**"]` 且仓内 `openspec/changes/archive/**/prototypes/index.tsx` 未被 gitignore
- **THEN** 该文件与其各级目录均不在树中（若某目录其余源文件全被排除，则目录也不出现）

#### Scenario: 构建 / 依赖产物树不出现

- **WHEN** 盘上存在 `packages/desktop/src-tauri/target/debug/x.rs`、`packages/desktop/node_modules/a/index.js`、`.git/hooks/y.ts` 或某隐藏目录下的源文件
- **THEN** 均不在树中；同层正常源文件照常出现

#### Scenario: 非法配置使树进错误态而不阻断画布

- **WHEN** 架构域配置 `exclude` 写一条无通配符的条目（如 `src/legacy`）
- **THEN** `archi_file_tree` 返回 `Err`（文案含非法条目并提示 `/ **` 形态）；前端只渲 `archi-file-tree-error` 行内提示，画布与抽屉照常渲染

### Requirement: 元素根缺失不静默

声明了但磁盘上不存在的模块根 SHALL **仍然出节点**，以 `exists = false` 标记：

- **出节点**：该节点的 `kind = Dir`、`mark = elementRoot`、`owners` 含对应 FQN，`exists = false`；其祖先目录同样合成。
- **不误报归属**：`exists = false` 的根 MUST NOT 使其 `mark` 退化为 `none`，MUST NOT 使该元素在树上消失（「元素在树上找不到」会是静默失败）。
- **只看盘上存在性**：`exists` 判据 SHALL 仅为「`kind = Dir` 且该目录在盘」，MUST NOT 掺入 `pathNotFound` 一类文档诊断结论。
- **前向防御已登记**：本仓 45 条 `path` 条目磁盘全部存在（真实样本为零），故该分支 SHALL 由合成 tempdir 用例覆盖；spec 明示其为前向防御分支，MUST NOT 因「本仓无样本」而省略实现。

#### Scenario: 声明但目录不存在仍出节点

- **WHEN** 合成 tempdir 中元素 `path = ["./src/ghost/"]` 而 `src/ghost/` 不存在（同目录其余文件正常）
- **THEN** 树中仍有 `src/ghost` 节点：`kind = "dir"`、`mark = "elementRoot"`、`owners` 含该 FQN、`exists = false`；其父目录出现且 `contains` 计入该元素

#### Scenario: 在盘目录 exists 为真

- **WHEN** 元素 `path = ["./crates/core/archi/"]` 且该目录在盘
- **THEN** 该节点 `exists = true`；非模块根的目录与非目录节点恒 `exists = true`

### Requirement: 左栏文件树呈现、分段切换与双向联动

`/architecture`（与 change 详情「架构」页签共用的同一画布组件）的三段布局第一段 SHALL 提供文件树，并与元素树以分段控件切换：

- **缺省文件树**：左栏缺省渲文件树；分段控件（`data-testid="archi-tree-mode"`，两钮 `data-mode="file"` / `data-mode="element"`）SHALL 切换左栏组件；切换只换左栏、`focus` 共用，MUST NOT 触发画布或抽屉重新取数。
- **元素树保留**：元素树 MUST NOT 删除——容器元素（`path` 空）只在 FQN 树上可见，且「谁依赖谁」在 FQN 树上更顺；其入参、渲染与既有 testid（`archi-element-tree` / `archi-tree-node`）零变化。
- **渲染顺序**：SHALL 目录先于文件、各自按 `name` 排序（后端只按 `path` 字节序，呈现序由前端派生）；缩进表达层级。
- **标记呈现**（图标 + 色 + 名称 + tooltip）：

  | `mark` | 图标 / 色 | 名称 | tooltip |
  |---|---|---|---|
  | `elementRoot` | 实心圆点 + 主色 | 加粗 | `模块 <FQN>` |
  | `ambiguous` | 警示色圆点 | 加粗 | `多归属：A、B` |
  | `member` | 左侧竖线 + 次级色 | 常规（弱化） | `属于 <FQN>` |
  | `containsElements` | 空心圆 + 计数 | 常规 | `含 N 个模块`（尾随小字 `·N`） |
  | `none` | 无 | 常规 | — |

- **点击（树 → 画布）**：`elementRoot` / `member` 行点击 SHALL 调 `onFocus(owners[0])`（即既有 `setFocusOverride`，一次调用、与元素树同语义）；`ambiguous` / `containsElements` / `none` 行点击 SHALL **只切该行展开态、零 `onFocus`**（MUST NOT 为候选选择新增弹层原语；`owners` 已是全量候选，将来放开只改点击处理）。
- **反查（画布 / 抽屉 → 树）**：`focus` 变化时 SHALL 以「条目 `owners` 是否含该 FQN」定位（判据 MUST NOT 依赖元素 `path` 在前端重算），对全部命中条目（多 `path` 元素会有多条）展开其祖先链、滚动到第一条、并让其同时高亮；**零命中**时 SHALL 在树顶部呈一行弱提示「该元素在树上无可定位节点（容器元素不持有文件，或该元素源文件均在判定面之外）」，MUST NOT 滚动。提示文案 MUST NOT 断言「该元素是容器」——`path` 非空的元素若其源文件全被排除矩阵排除也会零命中，两种成因不可在同一句里混说。
- **展开态**：SHALL 为 per-path 翻转集合（沿 `views/info/components/dir-tree.tsx` 既有模式），默认展开前 2 段路径，刷新后按 `path` 保持用户操作态；折叠 SHALL 不渲染子树 DOM。
- **图例**：树底部 SHALL 常驻一行可折叠图例，说明「仅显示判定面源文件（测试文件与架构配置 `exclude` 命中项不在树中；`none` 行 = 无元素认领）」。
- **测试挂钩**：SHALL 用 `data-testid`（`archi-file-tree` / `archi-file-node` / `archi-file-mark` 携 `data-mark` / `archi-tree-mode`），MUST NOT 以样式类名作查询挂钩。
- **不抽公共件**：文件树组件 MUST NOT 与 `views/info/components/dir-tree.tsx` 抽公共抽象——数据面不同（marks / owners vs 行数聚合），抽象需泛型参数化而收益为零。

#### Scenario: 缺省渲文件树且可切回元素树

- **WHEN** 打开 `/architecture`（或 change 详情「架构」页签）
- **THEN** 左栏缺省为文件树（`archi-file-tree` 在场、`archi-element-tree` 不在场）；点分段钮 `data-mode="element"` 后元素树在场且其 testid 与切换前既有断言一致；`archi_graph` / `archi_module` / `archi_drift` 调用次数不因切换增加

#### Scenario: 点模块行圈定范围

- **WHEN** 点击某 `mark = elementRoot` 或 `member` 行
- **THEN** 触发一次 `onFocus(owners[0])`：画布切到该元素的 1-hop 邻域、抽屉取该元素详情（`archi_module` 恰一次）

#### Scenario: 非归属行只切展开

- **WHEN** 点击 `mark` 为 `ambiguous` / `containsElements` / `none` 的行
- **THEN** 该行展开态翻转，`onFocus` 零调用（`archi_module` 调用次数不增），MUST NOT 出现候选弹层

#### Scenario: 聚焦多 path 元素多行高亮

- **WHEN** 画布或抽屉把 `focus` 切到声明了多条 `path` 的元素（如 `App.Frontend.Shell`）
- **THEN** 其全部对应节点同时高亮（`data-mark="elementRoot"` 且行底为高亮态）、祖先链展开、视图滚动到第一条

#### Scenario: 容器元素聚焦呈提示不滚动

- **WHEN** `focus` 为 `path` 为空的容器元素
- **THEN** 树顶部呈一行「该元素在树上无可定位节点」类弱提示（成因含「容器不持有文件」）；树不滚动、不尝试定位任何节点，且画布与抽屉的 1-hop 呈现不受影响

#### Scenario: 折叠不渲染子树且展开态按 path 保持

- **WHEN** 展开某深层目录后触发一次刷新重挂（同一批 `path`）
- **THEN** 该目录仍为展开态（per-path 翻转集合保持）；折叠态子树的 DOM 零节点

### Requirement: 树取数的独立失败与状态面

文件树取数 SHALL 与图 / 漂移取数独立失败，且沿用显式取数纪律：

- **三触发零监听**：取数 SHALL 只在组件挂载、`root` 变化、用户显式刷新三处发起；MUST NOT 引入目录 watch、定时轮询或事件订阅（沿 `desktop-archi-canvas`「三触发取数与零监听」同一红线）。刷新由 `ArchitectureView` 的 `refreshTick` 经 `key` 重挂统一覆盖（与 graph / drift 同步）。
- **失败不阻断画布**：取数失败 SHALL 只在左栏呈行内 `archi-file-tree-error`，MUST NOT 阻断或降级画布与抽屉渲染（树与图独立失败）。
- **旧结果不冒充**：失败时 SHALL 清空已载树（旧树 MUST NOT 继续以「本次结果」呈现），`root` 变化时旧 root 的树 MUST NOT 冒充新 root。
- **状态面**：加载中呈「解析中…」；树空（有模型、零进树源文件）呈「仓库内无源文件」；模型停摆时上层整块换结构错面板、树不挂载（与画布同形）。

#### Scenario: 树失败画布照常

- **WHEN** `archi_file_tree` 返回 `Err` 而 `archi_graph` 与 `archi_drift` 正常
- **THEN** 左栏呈 `archi-file-tree-error` 行内提示，画布节点 / 边与抽屉照常渲染、标记照常呈现

#### Scenario: 三触发与零轮询

- **WHEN** 审查 `use-archi-file-tree` 的取数调用点并观察运行期磁盘外改模型
- **THEN** 取数只在挂载 / `root` 变化 / 显式刷新三处发起；源码零 watch 接入、零定时器；磁盘外改模型时树不自动变化（须用户刷新）

#### Scenario: 换 root 不与旧树混淆

- **WHEN** `root` 由 A 改为 B 后 B 的取数失败
- **THEN** 左栏呈 B 的错误态，MUST NOT 继续显示 A 的树条目

#### Scenario: 空树与加载中

- **WHEN** 模型非空但仓库内零进树源文件（或首次取数未回）
- **THEN** 前者呈「仓库内无源文件」、后者呈「解析中…」；两者均不抛错、不影响画布

## Module Contract

### API: `packages/desktop/src-tauri/src/commands/archi/mod.rs`（命令面）

| 项 | 变化 | 描述 |
|------|------|------|
| `archi_file_tree` | 新增 | `(root: String) -> Result<ArchiFileTree, String>`；`check_root` → `archi_file_tree_inner` → 错误映射；只读、零写盘、零 spawn |
| `archi_file_tree_inner` | 新增 | `(root: &Path) -> Result<ArchiFileTree, String>`：读模型（结构诊断非空 → `Err`）→ `load_config` 取 `exclude` → `archi::tree::file_tree`；离 Tauri 可测 |
| 既有五命令与五个 `*_inner` | 零改动 | 签名 / 语义逐字不变；文件头注释的「五命令」更正为「六命令」 |

### API: `packages/desktop/src-tauri/src/commands/mod.rs`（登记面）

| 项 | 变化 | 描述 |
|------|------|------|
| `all_commands!` | 追加一行 | `commands::archi::archi_file_tree`，位于 `archi_drift` 之后、`read_file` 之前（archi 段 5 → 6 条；builder 注册与 `generate_handler!` 同源） |

### Function: `packages/desktop/src-tauri/crates/core/archi/src/tree.rs`（新增）

| 函数 | 签名 | 描述 |
|------|------|------|
| `file_tree` | `(root: &Path) -> Result<ArchiFileTree, String>` | 唯一入口：读模型（诊断非空 → `Err`；空模型 → 全 `none`）→ 由元素表建 `known` 后经 `drift::config::load_config(root, &known)` 取 `exclude`（`ignore` 不参与；文件缺失即空配置）→ 枚举（`scan::walk_source_files` + `is_source_file`）→ 排除（`ExcludeSet::build` 的完整矩阵）→ 模块根表 / `OwnerIndex` 归属 → 目录合成 → `mark` / `owners` / `contains` / `exists` → `path` 字节序。纯读盘、零写盘、零 spawn；MUST NOT 自带归属实现（一律经 `OwnerIndex`），MUST NOT 自带第二个配置解析器 |

### Function: `packages/desktop/src-tauri/crates/core/archi/src/dto.rs`（改）

| 项 | 变化 | 描述 |
|------|------|------|
| `ArchiFileTree` | 新增 | `{ entries: Vec<ArchiFileEntry> }`；`specta::Type` + camelCase |
| `ArchiFileEntry` | 新增 | 七字段 `path` / `name` / `kind` / `mark` / `owners` / `contains` / `exists`；声明序即出线序（`owners` 与 `contains` 的语义见「文件树命令与扁平条目投影」与「归属判定与判定单源」） |
| `FileEntryKind` | 新增 | 封闭二值 `Dir` / `File` + `ALL` + `as_str` |
| `FileTreeMark` | 新增 | 封闭五值 `elementRoot` / `ambiguous` / `member` / `containsElements` / `none` + `ALL` + `as_str`；MUST NOT 复用 `NodeMark`、MUST NOT 进四面词汇对拍 |
| 既有九组封闭词汇与全部出线 DTO | 零改动 | 字段名 / 顺序 / 语义逐字不变 |

### Function: `packages/desktop/src-tauri/crates/core/archi/src/scan/owner.rs`（改，判据面）

| 项 | 变化 | 描述 |
|------|------|------|
| `strip_generated` → 候选集 | 改 | 由「替换扩展名」改为返回 1~2 个候选：整组剥离 `<name>.i.<ext> → <name>`、替换扩展名 `<stem>.i.<ext> → <stem>.<ext>`；白名单（`rs` / `ts` / `tsx` / `js` / `jsx`）与零误剥面不变 |
| `owner_of` | 改 | 对每个候选各跑一遍最长前缀、按 `(fqn, 前缀长度)` 合并去重后定档；同长仍 `Ambiguous`；**MUST NOT 候选短路** |
| `normalize` / `prefix_matches` / `OwnerIndex::build` | 零改动 | 语义逐字不变（判据改造只此一处） |

### Function: `packages/desktop/src-tauri/crates/core/archi/src/scan/mod.rs`（改，可见面）

| 项 | 变化 | 描述 |
|------|------|------|
| `walk_source_files` | 私有 → `pub(crate)` | gitignore 感知遍历单点，供 `tree` 复用；语义零变化 |
| `is_source_file` | 私有 → `pub(crate)` | 五扩展名判据单点，供 `tree` 复用；语义零变化 |
| `scan()` / `SOURCE_EXTENSIONS` / 排除 / 归属 / 抽取 / 聚合六段 | 零改动 | 扫描语义逐字不变 |

### Function: `packages/desktop/src-tauri/crates/core/archi/src/lib.rs`（改）

| 项 | 变化 | 描述 |
|------|------|------|
| `pub mod tree;` | 新增 | 模块登记 |
| `pub use` 段 | 追加 | `tree::file_tree` 与 `ArchiFileTree` / `ArchiFileEntry` / `FileEntryKind` / `FileTreeMark` 跨 crate 唯一出口（沿既有 `pub use` 形态） |

### Component: `packages/desktop/src/views/architecture/components/file-tree.tsx`（新增）

| 项 | Props / 变化 | 描述 |
|------|------|------|
| `FileTree` | `{ entries: ArchiFileEntry[]; focused: string \| null; onFocus: (fqn: string) => void }` | 前端派生嵌套（目录先于文件、各自按 `name`）、per-path 翻转展开集合（默认前 2 段）、折叠不渲 DOM、`ScrollArea`；五值标记呈现与 tooltip；`elementRoot` / `member` 点击 → `onFocus(owners[0])`，其余三值只切展开；`focused` 变化按「条目 `owners` 含该 FQN」定位全部命中条目并展开祖先链 + 多行高亮 + 滚到第一条（零命中 → 顶部一行弱提示，不断言成因）；底部可折叠图例；testid `archi-file-tree` / `archi-file-node` / `archi-file-mark`（`data-mark`）/ `archi-file-tree-error` / `archi-file-tree-empty` |
| `TreeModeSwitch` | `{ mode: 'file' \| 'element'; onChange: (mode) => void }` | 分段控件 `data-testid="archi-tree-mode"`，两钮 `data-mode`；缺省 `file` |

### Component: `packages/desktop/src/views/architecture/components/architecture-canvas.tsx`（改）

| 项 | 变化 | 描述 |
|------|------|------|
| `CanvasBody` 左栏 | 改 | 「分段控件 + 条件渲 `FileTree` / `ElementTree`」；`useArchiFileTree(root)` 接线；`ElementTree` 入参与 testid 零变化 |
| `toNodes` / `toEdges` / `neighborhoodPositions` / `defaultFocus` / `ArchiNodeView` / 停摆分支 / 抽屉接线 / `driftMarks` 合并 | 零改动 | 画布与标记呈现逐字不变 |

### Function: `packages/desktop/src/views/architecture/hooks/use-archi-file-tree.ts`（新增）

| 函数 | 签名 | 描述 |
|------|------|------|
| `useArchiFileTree` | `(root: string \| null) => { tree: ArchiFileTree \| null; loading: boolean; error: string \| null }` | `commands.archiFileTree(root)`；三触发（挂载 / `root` 变化 / 上层 `key` 重挂）、零 watch 零轮询；失败清空 `tree`（旧结果不冒充本次）；`root` 变化后未回包时 `tree = null` |

### CLI: `packages/desktop/package.json`（脚本与版本）

| 项 | 变化 | 描述 |
|------|------|------|
| `version` | `0.6.35` → `0.6.36` | 一次 patch；`src-tauri/tauri.conf.json` 经 `"../package.json"` 跟随；`src-tauri/Cargo.toml` 的 crate 版本行**不跟** |
| `bindings:export` / `check` / `api:check` 等脚本 | 零改动 | 命令形态逐字不变；生成物随 `bindings:export` 刷新后入库 |

### Component: 生成物（入库）

| 项 | 变化 | 描述 |
|------|------|------|
| `packages/desktop/src/types/generated/bindings.ts` | 再生成 | `archiFileTree` typed 包装 + `ArchiFileTree` / `ArchiFileEntry` / `FileEntryKind` / `FileTreeMark` 类型 |
| `packages/desktop/src/types/generated/bindings.zod.ts` | 再生成 | 上列命令的 `argsSchemas` / `resultSchemas` 表项 + 四个具名 schema（纯接受面、零修饰） |

### 零改动面

| 模块 | 说明 |
|------|------|
| `components/element-tree.tsx` / `element-tree.test.tsx` | 元素树保留为「元素」分段视图，入参 / 渲染 / testid 零改动 |
| `components/element-drawer.tsx` / `structure-error-panel.tsx` / `lib/marks.ts` / `lib/drift-marks.ts` / `lib/fqn.ts` | 抽屉、停摆面板、边标记映射与标记派生、短名工具零改动 |
| `crates/core/archi/src/drift/**`、`crates/infra/archi/**` | 漂移判定与门禁 runner 零改动（本能力只消费 `drift::config::load_config`） |
| `crates/core/archi/src/scan/exclude.rs` / `exclude_test.rs` | `ExcludeSet::build(extra)` 语义即所需，零改动、零新增入口 |
| `crates/core/archi/src/api/**`、`crates/tools/archi-api-gen/**`、`<archi_dir>/api/**` | 契约生成面零改动（并集判据只动归属，不动落点与 `#[path]` 等值判据） |
| `crates/infra/agent/**` | 零 agent 工具新增；注入矩阵与知识文本零改动 |
| `.dev-team/architecture/models/**` / `decisions/**` | 模型元素 / 关系 / 计数零改动（`views/architecture/**` 已是单元素，新增文件不建元素） |
| `.dev-team/config.json` / `crates/core/config/**` | 工作区配置零改动（架构域配置与之互不读取） |
