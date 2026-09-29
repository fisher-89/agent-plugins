# 测试设计: desktop-workspace-code-stats

> **日期**: 2026-09-29

---

## 验收范围

| AC ID | 验收条件 | 被测文件或模块 |
|--------|---------|----------|
| AC-1 | 壳态下「页面」组出现「基础信息」项（`data-testid="nav-info"`），点击后 URL 为 `/info` 并渲染基础信息页；欢迎态（root 为 null）无壳无该入口 | `packages/desktop/src/components/app-sidebar.test.tsx`；`packages/desktop/src/app.test.tsx` |
| AC-2 | `code_stats` 经 `collect_commands!` 注册；重导出后 `bindings.ts` 含 `codeStats` 包装与 `CodeStatsReport` 系类型；前端经 `commands.codeStats` typed 调用，无裸 `invoke('code_stats')` | `packages/desktop/src-tauri/src/bindings_test.rs`；`packages/desktop/src/views/info/hooks/use-code-stats.test.ts` |
| AC-3 | 多语言 tempdir fixture 解析后，页面呈现汇总（文件 / 代码 / 注释 / 空行）与语言表（按代码行降序、占比以 Progress 承载、无内联 `style width`） | `packages/desktop/src-tauri/src/commands/stats/mod_test.rs`；`packages/desktop/src/views/info/info-view.test.tsx`；`packages/desktop/src/views/info/components/language-table.test.tsx`；`packages/desktop/src/views/info/components/stats-summary.test.tsx` |
| AC-4 | 深度默认 5；改为 N 后恰以 `depth=N` 重新发起一次解析，目录树呈现深度随之收缩 | `packages/desktop/src/views/info/hooks/use-code-stats.test.ts`；`packages/desktop/src/views/info/info-view.test.tsx` |
| AC-5 | 深度内目录节点可逐级展开 / 折叠；折叠节点子树不渲染 DOM（测试以子行存在性断言） | `packages/desktop/src/views/info/components/dir-tree.test.tsx` |
| AC-6 | 无 store 模型注册、无内存 / 持久缓存、无 watch、无轮询；每次进入页面、点刷新、调深度均发起新的 invoke | `packages/desktop/src/views/info/hooks/use-code-stats.test.ts`；`packages/desktop/src-tauri/src/commands/stats/mod_test.rs` |
| AC-7 | `code_stats` reject 时页面 inline 持久呈现错误（testid 承载）且无 toast 顶替；解析结果为空时呈现空态而非错误 | `packages/desktop/src/views/info/info-view.test.tsx`；`packages/desktop/src/views/info/hooks/use-code-stats.test.ts` |
| AC-8 | 前端检查 / 前端测试 / Rust 测试套件全绿；新增 Rust 测试无 tokei 自身计数语义断言；knip / lint 无新增豁免条目 | —（见不可测试项 1） |

---

## 单元测试

<!-- Rust 侧为 cargo 套件（#[test] 直调原函数，不启动 Tauri runtime）；前端为 vite-plus/test
  （@testing-library/react + jsdom，IPC 收敛于 @tauri-apps/api/core invoke mock）。
  每个源文件对应一个独立的 `### <源文件> -> <测试文件>` 章节；跨模块组合用例挂靠链路入口模块章节。
  纪律：新增 Rust 用例不含 tokei 自身计数 / 识别语义断言，仅断言自研组装层（同源自洽求和、排序键、
  深度截断、前缀聚合、Err 通道）。 -->

### packages/desktop/src-tauri/src/commands/stats/mod.rs -> packages/desktop/src-tauri/src/commands/stats/mod_test.rs

#### 待测功能

- code_stats(): `#[tauri::command]` 三件事薄包装——参数转换（`String → &Path`）→ 调用 `code_stats_inner` → `Err(String)` 透传；sync 形态、无 State 入参
- code_stats_inner(): 领域组装纯函数——root 有效性检查（`fs::metadata`，缺失 / 不可读 / 非目录 → `Err`）→ `Languages::get_statistics` 单次解析 → 汇总 / 语言（代码行降序、tie 语言名字典序、share 百分点）/ 目录树（≤depth 前缀聚合、POSIX path）三面组装

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| code_stats：薄包装结构证明（serde_json 对照 inner） | 正向 | `code_stats(root, depth)` 与 `code_stats_inner(&root, depth)` 对同一 tempdir 根的结果经 `serde_json` 序列化逐字段一致（命令层除参数转换与 Err 透传外零加工，queries mod_test 先例） | 新增 |
| code_stats：薄包装结构证明（serde_json 对照 inner） | 边界 | 以 `(String, u32)` 原函数签名直调即编译运行（`#[tauri::command]` 保留原函数可直调、无 State 入参——无状态薄包装的结构性证明） | 新增 |
| code_stats_inner：root 有效性检查（Err 通道唯一来源） | 异常 | 不存在的 root 路径 → `Err(String)` 且不 panic（PoC 结论：tokei 对缺失根返回空 Languages，Err 通道由前置 metadata 检查补齐） | 新增 |
| code_stats_inner：root 有效性检查（Err 通道唯一来源） | 异常 | root 指向普通文件（非目录）→ `Err(String)` | 新增 |
| code_stats_inner：root 有效性检查（Err 通道唯一来源） | 异常 | 不可读 root → `Err(String)`（fixture 由实现期按平台定夺，Windows 下以目录 ACL 收紧近似） | 新增 |
| code_stats_inner：root 有效性检查（Err 通道唯一来源） | 边界 | 空 root（空串）→ `Err(String)`（blank root 与无效 root 同走 Err，design 定夺） | 新增 |
| code_stats_inner：root 有效性检查（Err 通道唯一来源） | 边界 | 超长（>1000 字符）不存在路径 root → `Err(String)` 不 panic | 新增 |
| code_stats_inner：汇总面与语言面组装（多语言 tempdir fixture） | 正向 | 多语言 fixture（`.rs` / `.ts` / `.md` 各置不同行规模文件）解析 → `totals` 四项 = 各语言行合计（同源自洽不变量），`languages` 非空且按 code 降序 | 新增 |
| code_stats_inner：汇总面与语言面组装（多语言 tempdir fixture） | 正向 | depth=1 与 depth=10 两次解析 → `totals` 与 `languages` 完全相等（深度是聚合截断参数非遍历限制，PoC 结论的行为留档） | 新增 |
| code_stats_inner：汇总面与语言面组装（多语言 tempdir fixture） | 边界 | 两语言 code 相等的 tie fixture（各置一个无注释无空行的单语句文件）→ 行序按语言名字典序（自研 tie 键，断言仅及顺序不及计数） | 新增 |
| code_stats_inner：汇总面与语言面组装（多语言 tempdir fixture） | 边界 | 仅注释与空行的 fixture 文件 → 该语言行在场、code=0、share=0.0（Σcode=0 时 share 口径为 0.0） | 新增 |
| code_stats_inner：汇总面与语言面组装（多语言 tempdir fixture） | 边界 | 目录存在但无被识别文件（空目录 / 仅未知扩展名文件）→ `Ok`：totals 全 0、languages 空数组、tree 空数组（前端空态输入，非 Err） | 新增 |
| code_stats_inner：汇总面与语言面组装（多语言 tempdir fixture） | 边界 | 每行 `share` ∈ [0,100] 且与 code 降序单调一致；`files` / `code` / `comments` / `blanks` 均为非负整数 | 新增 |
| code_stats_inner：目录树组装（≤depth 前缀聚合、POSIX path） | 正向 | 嵌套 fixture（`src/deep/` 多层）→ 节点 `path` 为相对 root 的 `/` 分隔路径、`name` 为末段目录名、`children` 按名字典序 | 新增 |
| code_stats_inner：目录树组装（≤depth 前缀聚合、POSIX path） | 边界 | depth=1 → 树仅顶层目录一层，深层文件行计入其祖先顶层节点聚合（子树全量口径：父含子） | 新增 |
| code_stats_inner：目录树组装（≤depth 前缀聚合、POSIX path） | 边界 | depth=0 → `tree` 空数组、totals 与 languages 不变（截断到无目录层，不丢数） | 新增 |
| code_stats_inner：目录树组装（≤depth 前缀聚合、POSIX path） | 边界 | 根层直属文件不产生目录节点、不计入任何节点聚合（无虚拟根，design 定夺）；子目录节点聚合恰含其全部后代文件 | 新增 |
| code_stats_inner：目录树组装（≤depth 前缀聚合、POSIX path） | 边界 | 50 个顶层子目录（数字 / 大小写 / 中文混合命名）→ `children` 字典序稳定全量呈现 | 新增 |
| code_stats_inner：目录树组装（≤depth 前缀聚合、POSIX path） | 边界 | 含空格与中文名的目录 → `path` 出线保持原名（归一化仅统一分隔符，不改名） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 文件系统（进程边界） | 不 mock：以真实 tempdir fixture 承载（`std::env::temp_dir` + RAII 清理，queries mod_test `TempWs` 先例），多语言 / 嵌套 / 空目录 / 无效 root 按用例搭建 | 全部用例 |
| tokei 库 | 不 mock、不注入：作为被测函数内部真实依赖消费；其计数 / 识别 / ignore 语义不在断言范围（仅组装不变量、排序键、截断、聚合与 Err 面断言） | 全部用例 |

### packages/desktop/src-tauri/src/bindings.rs -> packages/desktop/src-tauri/src/bindings_test.rs

<!-- design.md「公共函数 / API」未声明 bindings.rs 行：本变更对该文件的改动为 collect_commands!
  增第 23 条（code_stats）与 builder doc 注释 22→23，无新公共 API。本章为既有 export_bindings()
  覆盖性套件的 22→23 扩面（design「提案与规格同步状态」节裁定归测试阶段承接），既有用例全部保留。 -->

#### 待测功能

- （design.md 未声明该文件的公共 API 变更；扩面断言挂靠既有 `export_bindings()` 覆盖性套件，见上方注释）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 导出产物覆盖性（22→23 扩面） | 正向 | 命令包装名清单增 `codeStats`、invoke 命令名清单增 `code_stats`（第 23 条）后，既有产物覆盖断言通过 | 新增 |
| 导出产物覆盖性（22→23 扩面） | 正向 | `DTO_TYPES` 增 `CodeStatsReport` / `CodeTotals` / `LanguageStats` / `DirNode` 后 `export type` 断言通过（camelCase TS 镜像出线） | 新增 |
| 导出产物覆盖性（22→23 扩面） | 边界 | 产物含 `codeStats` 包装形态：`(root: string, depth: number)` 入参、返回 `Promise<CodeStatsReport>`、无 `Result` 包装（Throw 模式 reject 语义回归不破） | 新增 |
| 导出幂等（23 条下回归） | 边界 | 连续两次导出逐字节一致、产物无机器路径嵌入的既有断言在 23 条下保持 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 产物文件（共享单一目标） | 不 mock：真实目标路径 + 互斥锁串行访问 + Drop 守卫恢复权威产物（既有 bindings_test.rs 装置照抄） | 全部用例 |

### packages/desktop/src-tauri/src/commands/mod.rs -> packages/desktop/src-tauri/src/commands/mod_test.rs

<!-- design.md 未声明该文件的公共 API 变更：修改内容仅 `pub mod stats;` 与轨道清单 doc 注释
  （五轨 → 六轨），无任何运行时行为面。test_resolve_paths 产出 commands/mod_test.rs 路径，
  但按纯声明模块纪律不建测试文件（见不可测试项 4），本章仅保留框架、不写用例。 -->

#### 待测功能

- （无——该文件无公共 API 变更，不建测试文件）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 不建测试文件，无 mock 需求 | — |

### packages/desktop/src/views/info/hooks/use-code-stats.ts -> packages/desktop/src/views/info/hooks/use-code-stats.test.ts

#### 待测功能

- useCodeStats(root): 取数收口 hook，返回 `CodeStatsState`（`report` / `loading` / `error` / `depth` / `setDepth` / `refresh`）；显式刷新模型（root / depth / tick 触发 invoke）、`cancelled` 防串轮、数据归属 root 标记（切换过渡轮抑制）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| useCodeStats：显式刷新取数纪律（AC-4/AC-6） | 正向 | 有效 root 挂载：以 `(root, depth=5)` 发起 codeStats 恰一次，`report` 呈现返回 DTO，loading true→false | 新增 |
| useCodeStats：显式刷新取数纪律（AC-4/AC-6） | 正向 | `refresh()` 显式调用：同 root 同 depth 恰再发一次 | 新增 |
| useCodeStats：显式刷新取数纪律（AC-4/AC-6） | 正向 | `setDepth(N)`：恰以 `depth=N` 再发一次（自默认 5 起），`depth` 态同步更新 | 新增 |
| useCodeStats：显式刷新取数纪律（AC-4/AC-6） | 边界 | `setDepth` 取值域端点 1 与 10：各以对应 depth 恰发起一次 | 新增 |
| useCodeStats：显式刷新取数纪律（AC-4/AC-6） | 边界 | 连续 `setDepth` 5→3→7：三轮各以对应 depth 发起，最终 `report` 为 depth=7 轮结果（在途旧轮取消抑制） | 新增 |
| useCodeStats：显式刷新取数纪律（AC-4/AC-6） | 边界 | 取数完成后静置（推进定时器、无任何交互）：调用数不增长（无轮询的行为化核对） | 新增 |
| useCodeStats：显式刷新取数纪律（AC-4/AC-6） | 异常 | codeStats reject（首取）：`error` 含错误串、loading 复位、`report` 保持 null | 新增 |
| useCodeStats：显式刷新取数纪律（AC-4/AC-6） | 异常 | 成功后 `refresh()` 再 reject：`error` 置位、`report` 保持上次成功值不崩（沿 use-explore-list 不污染惯例） | 新增 |
| useCodeStats：root 切换过渡与归属（AC-6） | 正向 | root A→B：以新 root 重取，B 结果到达后 `report` 归属 B | 新增 |
| useCodeStats：root 切换过渡与归属（AC-6） | 边界 | 切换过渡轮（B 在途）：`report` 为 null，旧根报告不呈现（数据归属 root 标记抑制） | 新增 |
| useCodeStats：root 切换过渡与归属（AC-6） | 边界 | 旧 root 迟到 resolve：不覆盖新态（cancelled 抑制） | 新增 |
| useCodeStats：root 切换过渡与归属（AC-6） | 边界 | 旧 root 迟到 reject：不置错误、不污染新态 | 新增 |
| useCodeStats：生成绑定调用面（AC-2） | 正向 | 经 `commands.codeStats` typed 入口发起：命令名与参数 `(root, depth)` 逐字不变（生成绑定底层同模块 invoke，mock 机制切换后依旧生效），全程无裸 `invoke('code_stats')` | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC（进程边界） | `vi.mock('@tauri-apps/api/core', { invoke: invokeMock })`，按用例 resolve / reject / 手动 pending 三态控制（use-explore-list.test.ts 同型装置） | 全部用例 |
| 内部模块 | 不 mock：hook 为被测对象本体，无注入依赖 | — |

### packages/desktop/src/views/info/info-view.tsx -> packages/desktop/src/views/info/info-view.test.tsx

#### 待测功能

- InfoView({ root }): 页面骨架——标题 + 深度 `<select>`（1–10，默认 5，`data-testid="info-depth-select"`）+ 刷新钮（`disabled={loading}`）+ 三面呈现区 + 空态（`data-testid="info-empty"`）+ inline 持久错误（`data-testid="info-error"`，无 toast 顶替）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| InfoView：三面呈现与深度控件（AC-3/AC-4） | 正向 | codeStats 返回三面 DTO：汇总（info-summary）、语言表（info-language-row）、目录树（info-dir-node）全部呈现，挂载恰发起一次 `(root, depth=5)` | 新增 |
| InfoView：三面呈现与深度控件（AC-3/AC-4） | 正向 | 深度 select 默认值 5；改为 N 后恰以 `depth=N` 重发一次，树面随新数据收缩呈现（AC-4） | 新增 |
| InfoView：三面呈现与深度控件（AC-3/AC-4） | 边界 | loading 期间刷新钮 disabled（pending promise 保持）；loading 解除后可点、点击恰再发一次（AC-6） | 新增 |
| InfoView：错误与空态（AC-7） | 异常 | codeStats reject：info-error 在场且 inline 持久（waitFor 后仍在场）、无 sonner toast 节点、三面呈现区不在场 | 新增 |
| InfoView：错误与空态（AC-7） | 边界 | 空 report（totals 全 0、languages 空数组、tree 空数组）：info-empty 在场、info-error 不在场（空态而非错误） | 新增 |
| InfoView：错误与空态（AC-7） | 边界 | 错误后点刷新且成功：info-error 消失、三面呈现恢复（inline 可恢复语义） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC（进程边界） | `vi.mock('@tauri-apps/api/core')`，invoke 按命令名 codeStats 分发（resolve / reject / 手动 pending 控制 loading 态），返回 camelCase 三面 DTO fixture | 全部用例 |
| 内部模块（useCodeStats / StatsSummary / LanguageTable / DirTree） | 不 mock：真实组合，进程边界收敛于 invoke 一处 | 全部用例 |

### packages/desktop/src/views/info/components/stats-summary.tsx -> packages/desktop/src/views/info/components/stats-summary.test.tsx

#### 待测功能

- StatsSummary({ totals }): 汇总面四项总量（`files` / `code` / `comments` / `blanks`）呈现（`data-testid="info-summary"`）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| StatsSummary：汇总面呈现（AC-3） | 正向 | 四项数值与入参逐项一致，容器带 info-summary | 新增 |
| StatsSummary：汇总面呈现（AC-3） | 边界 | 全 0 totals：四个 0 渲染不崩 | 新增 |
| StatsSummary：汇总面呈现（AC-3） | 边界 | 大数值（> 2^32，如 5000000000）：完整数值呈现（u64 → number bigint cast 出线口径） | 新增 |
| StatsSummary：汇总面呈现（AC-3） | 边界 | 仅呈现四项、无额外合计 / 派生行（`lines` 不预建口径） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 纯 props 驱动组件，jsdom 真实渲染断言 DOM，零 mock | 全部用例 |

### packages/desktop/src/views/info/components/language-table.tsx -> packages/desktop/src/views/info/components/language-table.test.tsx

#### 待测功能

- LanguageTable({ languages }): 语言占比表（列：语言 | 文件数 | 代码 | 注释 | 空行 | 占比；行序即入参序；占比以 Progress 承载 `value={share}`；行挂钩 `data-testid="info-language-row"`）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| LanguageTable：语言占比表呈现（AC-3） | 正向 | 多语言行按入参序渲染（组件不重排——排序职责在 inner），每行 info-language-row 六列内容与入参一致 | 新增 |
| LanguageTable：语言占比表呈现（AC-3） | 边界 | 空数组：表头在场、零数据行、不崩（页面级空态由 InfoView 承载，组件自身不报错） | 新增 |
| LanguageTable：语言占比表呈现（AC-3） | 边界 | 单语言行：单行渲染正常 | 新增 |
| LanguageTable：语言占比表呈现（AC-3） | 边界 | share=0.0 与 share=100.0：Progress value 端点呈现 | 新增 |
| LanguageTable：语言占比表呈现（AC-3） | 边界 | 50 行语言全量渲染、行数一致无丢失 | 新增 |
| LanguageTable：语言占比表呈现（AC-3） | 边界 | 无内联 `style width`：任一 info-language-row 子树无 `style` 含 width 的内联节点（占比以 ui/progress transform 承载，AC-3 硬断言） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 纯 props 驱动组件，jsdom 真实渲染断言 DOM，零 mock | 全部用例 |

### packages/desktop/src/views/info/components/dir-tree.tsx -> packages/desktop/src/views/info/components/dir-tree.test.tsx

#### 待测功能

- DirTree({ nodes }): 目录树递归行组件——目录名 + 聚合统计（文件 / 代码）+ 缩进层级；per-path 本地 state 展开 / 折叠；折叠节点子树不渲染 DOM；默认展开前 2 级；节点行 `data-testid="info-dir-node"` + `data-path`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| DirTree：展开折叠与 DOM 收敛（AC-5） | 正向 | 默认展开前 2 级（路径段数 ≤2 的节点子行在场，第 3 级子行不在 DOM），节点行 info-dir-node 与 data-path 正确 | 新增 |
| DirTree：展开折叠与 DOM 收敛（AC-5） | 正向 | 逐级点击折叠节点 → 子树行出现；再点击 → 子树行从 DOM 消失（按 data-path 查询为零——折叠态子树不渲染断言） | 新增 |
| DirTree：展开折叠与 DOM 收敛（AC-5） | 边界 | 空数组：无行渲染不崩 | 新增 |
| DirTree：展开折叠与 DOM 收敛（AC-5） | 边界 | 叶子节点（children 空）：无子行可展开，展开开关缺席或点击不产生子行 | 新增 |
| DirTree：展开折叠与 DOM 收敛（AC-5） | 边界 | 大树（50 节点）默认态：DOM 行数收敛于已展开子树（默认折叠收敛断言） | 新增 |
| DirTree：展开折叠与 DOM 收敛（AC-5） | 边界 | 深树（十段路径，对应 depth 上限 10）逐级展开后最深行可达 | 新增 |
| DirTree：展开折叠与 DOM 收敛（AC-5） | 边界 | 同名目录不同路径（`a/x` 与 `b/x`）：折叠 / 展开互不串扰（per-path 键） | 新增 |
| DirTree：展开折叠与 DOM 收敛（AC-5） | 边界 | rerender 同 path 集合、新统计值：展开态保持（键稳定，不随重解析重置） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 纯 props 驱动组件（展开态为组件内部 state），jsdom 真实渲染 + fireEvent 断言 DOM，零 mock | 全部用例 |

### packages/desktop/src/routes.tsx -> packages/desktop/src/app.test.tsx

<!-- design.md 未声明该文件的公共 API 变更（修改内容仅路由表增 /info 项，AppRoutes 导出签名不变）。
  test_resolve_paths 对该文件报 Not in test config scope（无 colocated 测试路径，见不可测试项 6）；
  路由可达性属跨模块组合语义，按链路入口挂靠规则以 app.test.tsx 组合用例承载
  （App 自含 HashRouter 真实挂载，routes → InfoView 真实组合，非独立集成章节）。 -->

#### 待测功能

- （无新导出 API；组合用例经 App 路由链路驱动 AppRoutes，覆盖 `/info` 路由项与 `*` 兜底回归）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| App：/info 路由可达与欢迎态隔离（AC-1） | 正向 | 壳态点击 nav-info → hash 落 `#/info`、基础信息页呈现（info-summary 等挂钩在场）、变更页内容卸载、nav-info 激活 | 新增 |
| App：/info 路由可达与欢迎态隔离（AC-1） | 正向 | 启动前 hash 已为 `#/info`：深链直出基础信息页且 nav-info 激活（对齐既有深链用例形态） | 新增 |
| App：/info 路由可达与欢迎态隔离（AC-1） | 边界 | 未知路径兜底回归：`#/bogus` 仍兜底落 `#/changes`（`/info` 插入不破 `*` 兜底语义） | 新增 |
| App：/info 路由可达与欢迎态隔离（AC-1） | 边界 | `#/info/xyz`（路由表无 /info 子段）：兜底落 `#/changes`，不空白不崩 | 新增 |
| App：/info 路由可达与欢迎态隔离（AC-1） | 边界 | 欢迎态（root=null）：无壳无 nav-info，`#/info` 不渲染基础信息页、仅欢迎屏（无 Router 挂壳的结构既有断言扩员） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC（进程边界） | 既有 mockIpc 按命令名分发装置扩展 codeStats 分支（返回三面 DTO fixture）；`getVersion` / `open` / `check` 沿用既有 mock，`window.location.hash` 每用例重置 | 本 describe 全部用例 |
| 内部模块（AppRoutes / InfoView / useCodeStats） | 不 mock：真实组合，进程边界收敛于 invoke | 本 describe 全部用例 |

### packages/desktop/src/components/app-sidebar.tsx -> packages/desktop/src/components/app-sidebar.test.tsx

<!-- design.md 未声明该文件的公共 API 变更：改动为 PageNavGroup 增 [基础信息] NavLink
  （data-testid="nav-info"，lucide Info 图标，组内首位，active 由 URL 派生）。
  本章承接 nav-info 渲染 / active 用例与既有 nav-* 回归核对，既有用例全部保留。 -->

#### 待测功能

- （design.md 未声明该文件的公共 API 变更；用例针对既有 `AppSidebar` 导出组件的 nav-info 扩员，见上方注释）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| AppSidebar：页面导航组 nav-info（AC-1） | 正向 | nav-info 在场且居「页面」组首位（组顺序 基础信息 → 变更 → 探索，DOM 结构断言）、tagName 为 A、href=/info、带 svg 图标 | 新增 |
| AppSidebar：页面导航组 nav-info（AC-1） | 正向 | pathname=/info → nav-info `data-active="true"` 且其余 nav 为 false；/changes 下 nav-info 为 false（active 由 URL 派生） | 新增 |
| AppSidebar：页面导航组 nav-info（AC-1） | 正向 | 点击 nav-info → location-probe 呈 /info | 新增 |
| AppSidebar：页面导航组 nav-info（AC-1） | 边界 | pathname 无匹配前缀（/bogus）→ nav-info `data-active="false"`、渲染不崩（对齐既有降级用例） | 新增 |
| AppSidebar：页面导航组 nav-info（AC-1） | 边界 | 既有挂钩回归：nav-changes / nav-agent / nav-explores / nav-db 的组归属与激活语义既有断言不破（页面组扩员不改变他项） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| Router context（运行环境） | `MemoryRouter` initialEntries 包裹（NavLink 需 Router context）+ LocationProbe 组件投影 pathname（既有 mountNav 装置复用） | 本 describe 全部用例 |
| ResizeObserver 缺口（jsdom 运行环境） | `vi.stubGlobal('ResizeObserver', ResizeObserverStub)`（既有 stubEnvironment 装置复用） | 全部用例 |
| workspace 回调（入参例外） | `onOpen` / `onAdd` / `onRemove` 以 `vi.fn()` 注入 | 既有用例回归 |

### packages/desktop/src/types/generated/bindings.ts -> packages/desktop/src/types/generated/bindings.test.ts

<!-- 生成物不手改（design 变更清单口径）。test_resolve_paths 产出 bindings.test.ts 路径，
  但按纯类型产物纪律不建测试文件（见不可测试项 5），本章仅保留框架。
  design.md 未声明该文件的公共 API 变更——内容权威性由 Rust 侧 bindings_test.rs 权威快照比对
  承载（AC-2），前端 typed 调用由 use-code-stats.test.ts 生成绑定调用面用例回归。 -->

#### 待测功能

- （无——生成物，不建测试文件；覆盖方式见上方注释与不可测试项 5）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 不建测试文件，无 mock 需求 | — |

---

## 不可测试项

- AC-8 管线面（前端检查 / 前端测试 / Rust 测试套件全绿、knip / lint 无新增豁免条目） — **原因**: 管线绿与豁免零增是执行期与静态检查的结论性状态，非进程内可用例断言的行为；其中「新增 Rust 测试无 tokei 自身计数语义断言」已转译为本设计 stats/mod_test.rs 章节的用例纪律（仅组装不变量 / 排序键 / 截断 / 聚合 / Err 面断言），执行验证归 test-execution 阶段
- tokei 自身语义（语言识别、行计数口径、hidden 目录跳过、`.gitignore` 尊重、非 git 目录 ignored 计入边界） — **原因**: proposal 裁定 MUST NOT 逐项验证库自带语法 / 匹配 / 计数语义；本设计仅经自研组装不变量（同源自洽求和、排序键、深度截断、前缀聚合、Err 通道）间接覆盖，库行为留档已由 design 阶段 PoC 完成
- AC-6 前半的全局负向断言（无 store 模型注册、无内存 / 持久缓存、无 watch 订阅） — **原因**: 「不存在某物」类负向存在性无法以正向用例证明，归静态检查与审查；可行为化部分（进入页面 / 点刷新 / 调深度各发起新 invoke、静置后调用数不增长）已落入 use-code-stats.test.ts 与 stats/mod_test.rs 用例
- `packages/desktop/src-tauri/src/commands/mod.rs` 不建测试文件（test_resolve_paths 产出 commands/mod_test.rs 路径） — **原因**: 纯模块声明文件（`pub mod stats;` + doc 注释五轨 → 六轨），无任何运行时行为面；按纯类型模块纪律不建空测试套件，防空套件红灯
- `packages/desktop/src/types/generated/bindings.ts` 不建测试文件（test_resolve_paths 产出 bindings.test.ts 路径） — **原因**: 生成物不手改、无独立行为面；内容覆盖由 bindings_test.rs（Rust 侧权威快照比对，AC-2）承载，前端消费经 use-code-stats.test.ts 生成绑定调用面用例回归
- `packages/desktop/src/routes.tsx` 独立测试路径解析失败（test_resolve_paths 报 Not in test config scope） — **原因**: 路由表为组合声明文件、test config 无 colocated 映射；非不可测试——其可达性行为属跨模块组合语义，已按链路入口挂靠规则以 app.test.tsx 组合用例承载（见对应章节）
