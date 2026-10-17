# 测试设计: check-before-flow-start

> **变更**: check-before-flow-start
> **基于**: proposal.md（AC-1 ~ AC-8）、design.md（D1–D10 定稿、架构组件、变更清单、公共函数 / API 表、类型定义表）
> **角色**: test-design-planner 产物（只设计测试范围与用例面，不含测试命令细节；static-check / test-execution 等机械步骤由桌面端承接）

## 框架与布局惯例（盘点结论）

- **Rust 侧**：内联 `#[test]` / `#[tokio::test]` 单元测试，测试文件为源文件同目录 `*_test.rs`（经 `#[cfg(test)] mod` 挂载）；断言风格 `assert_eq!` / `assert!`（中文失败信息）。`infra/vcs` 沿真实 git tempdir 真件形态（`tempfile::TempDir` 隔离 + 直驱 `git` 子命令断言 + `test_path_lock` 串行化）；`infra/checks` 沿真件 shim 形态（`TempWs` + `write_shim` + `write_config`（`openspec/config.json`）+ PATH 前置窗口）；命令层 `mod_test.rs` 沿真件 spawn 先例（`write_static_shim` + config 登记 + `prefix_path` PATH 注入 + `tauri::test::MockRuntime` + commands 级 PATH 锁）；walker 侧沿 port 假件注入形态（FakeWorker / FakeTools / FakeVcs / RecordingHistory + 真实 tempfile `TestDb`）。集成节收 `crates/core/workflow/tests/corpus_golden_test.rs`（db 种子语料 → 读链投影 → golden 快照，`DESKTOP_GOLDEN_REWRITE=1` 显式重写）。
- **前端侧**：vitest（`vite-plus/test` 导入面：`describe` / `it` / `expect` / `vi`），colocated `*.test.ts` / `*.test.tsx`；组件测试 `@testing-library/react` 直渲染（`screen` + `data-testid` 锚点），纯函数内联 fixture 直测。
- **用例表口径**：`路径类型` ∈ 正向 / 异常 / 边界 / 组合（组合 = 跨模块链路用例，挂靠链路发起方 / 最上层调用方所在章节）；`迭代类型` ∈ 单次 / 重试链（幂等重入与二次发起面）/ 枚举遍历（全词汇 / 全槽位穷举）。

## 验收范围

| AC ID | 验收条件 | 被测文件或模块 |
|-------|----------|----------------|
| AC-1 | 新建 change 发起 run：initialize 相位先于 proposal 执行（图上 initialize 列 Clean / StaticCheck / TestExecution / PhaseLog 节点齐备），检查在 worktree（exec_root）内执行，报告与 exec-outputs 落 `openspec/changes/<name>/reports/baseline/` | `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs`（五步序 / slot 构造点）、`packages/desktop/src-tauri/crates/core/orchestration/src/steps.rs`（slot 透传）、`packages/desktop/src-tauri/crates/core/orchestration/src/port.rs`（槽位链路）、`packages/desktop/src-tauri/crates/infra/checks/src/static_check.rs`、`packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod.rs`（baseline 锚点）、`packages/desktop/src-tauri/crates/infra/exec/src/lib.rs`（exec-outputs 槽位锚定）、`packages/desktop/src-tauri/crates/core/foundation/src/layout/mod.rs`（槽位单点）、`packages/desktop/src-tauri/src/commands/change_flow/mod.rs`（真件组合回归）、`packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs`（表头声明） |
| AC-2 | 基线 static-check 失败（真件 shim 红）→ run 以 Terminal 收敛 failed、文案引导基线修复后重新发起；PhaseRecord 零 initialize 条目；再发起 → clean 恢复基线后干净重试 | `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs`（Terminal 分派 / 收敛 / 一次性重试）、`packages/desktop/src-tauri/crates/infra/checks/src/static_check.rs`（fail 载荷 + report_dir 回填）、`packages/desktop/src-tauri/src/commands/change_flow/mod.rs`（基线红停真件） |
| AC-3 | 基线 test-execution `conclusion=Fail` → 停止 + 测试红文案；`conclusion=Error` → 停止 + 环境文案——两态文案可辨（均携 baseline 报告目录） | `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs`（Error / Fail 分文案 Terminal）、`packages/desktop/src-tauri/crates/infra/checks/testexec`（三态 conclusion 载荷供给，零改动回归） |
| AC-4 | initialize 全绿 → PhaseLog 机械 checklist 落账 pass（零 agent、三会话槽位恒 None）；手动档停等照常；再发起（或 auto 推进）不再重跑 initialize（PhaseRecord 持久跳过） | `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs`（机械 checklist 复用 / 一次性跳过 / 停等零特判）、`packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs`（INITIALIZE_STEPS 含 TestExecution 步的声明前提）、`packages/desktop/src-tauri/src/commands/change_flow/mod.rs`（停等计数随动回归） |
| AC-5 | clean 幂等安全：依赖与工具产物树（node_modules / dist / build / target / coverage / .nyc_output）与 dot 目录、`openspec/` 工件树保留；tracked 修改被 reset、其余 untracked 清除 | `packages/desktop/src-tauri/crates/core/foundation/src/prune.rs`（名单单源）、`packages/desktop/src-tauri/crates/infra/vcs/src/git.rs`（reset + clean 排除面真件）、`packages/desktop/src-tauri/crates/infra/vcs/src/lib.rs`（转发缝）、`packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs`（排除面组装 D6） |
| AC-6 | legacy（`worktree=None`）change：initialize 整相位短路——零 spawn、主仓零重置、跳过因可见、后续相位照常 | `packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs`（drive() 短路分支 + step_clean 主仓双保险）、`packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs`（INITIALIZE_PHASE_ID 常量锚） |
| AC-7 | `allowed_backtrack_phases` 不含 initialize（任一后续相位回溯候选白名单排除表头）；stale 传播零波及 | `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs`（backtrackable 属性过滤 + prerequisites 零入边） |
| AC-8 | 前端零结构改动：initialize 相位列（requirement 10 列 / test-only 7 列）与 clean 步词标签（git 分类）呈现，基线报告经节点 outputRef stdout 面板回查；全管线绿（static-check + 前后端测试 + bindings 一致性守卫） | `packages/desktop/src/views/changes/flow/phase-column-node.tsx`、`packages/desktop/src/views/changes/flow/run-step-node.tsx`、`packages/desktop/src/views/changes/flow/run-state.ts`、`packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs`（10/7 列出线 + corpus golden initialize 样本）、`packages/desktop/src-tauri/crates/infra/store/src/model.rs`、`packages/desktop/src-tauri/crates/infra/store/src/store.rs`（词汇落库随动）、`packages/desktop/src/types/generated/bindings.ts`（生成物，见「不可测试项」）、`packages/desktop/package.json`（静态门，见「不可测试项」） |

> 覆盖核对：AC-1 ~ AC-8 八条全量在案；「被测文件或模块」列覆盖 design.md 新增文件子表 1 项（foundation/src/prune.rs）+ 修改文件子表 25 项（其中 `bindings.ts` 与 `package.json` 两生成 / 配置物转「不可测试项」静态门承接，其余 23 项 + 零改动沿用的 `commands/changes/mod.rs`（`read_exec_output` IPC 双锚回查组合半边，经 exec 节与 mod 级注记承接）均有 per-file 章节）；源文件 → 测试文件映射见下文 per-file 章节标题。

## 单元测试

### packages/desktop/src-tauri/crates/core/foundation/src/prune.rs -> packages/desktop/src-tauri/crates/core/foundation/src/prune_test.rs

#### 待测功能

- PRUNED_DIR_NAMES: 依赖与工具产物树目录名封闭集（`["node_modules", "dist", "build", "target", "coverage", ".nyc_output"]` 六员；detect 剪枝与 restore_baseline 排除面两消费点同源；名单本体不含 `openspec/`——工件增补属消费点语义）
- is_pruned_dir_name(): dot 前缀或名单命中（剪枝判定纯函数；路径组件迭代语义驻各消费点）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| PRUNED_DIR_NAMES | 正向 | 六员逐字锚定：`node_modules` / `dist` / `build` / `target` / `coverage` / `.nyc_output` 与 explore 决议名单逐字一致（常量组锚——名单扩展一处修改双点生效的前提面） | 枚举遍历 |
| PRUNED_DIR_NAMES | 边界 | 封闭集属性：`len() == 6`、互异无重复、逐员非空串；名单本体不含 `openspec / .`（dot 与工件增补驻消费点——单源边界锚） | 单次 |
| is_pruned_dir_name() | 正向 | 名单逐名命中：六员各一 → `true` | 枚举遍历 |
| is_pruned_dir_name() | 正向 | dot 前缀命中：`.git` / `.idea` / `.vscode` / 单字符 `.` → `true`（dot 剪枝语义） | 单次 |
| is_pruned_dir_name() | 边界 | 空串 → `false`（无前缀无命中）；普通目录名（`src` / `crates` / `openspec`）→ `false`（工件树不进名单本体） | 单次 |
| is_pruned_dir_name() | 边界 | 形态近邻不误伤：`node_module`（少 s）/ `dist-x` / `Build`（大小写折叠）/ 带 `/` 分隔符形态 → `false`（逐字比对零归一；路径切分归消费点） | 单次 |
| is_pruned_dir_name() | 边界 | 超长名（>1000 字符）与特殊字符名（emoji / `\n` 字节串）零 panic 直返 `bool`（纯函数稳健面） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| （无） | 零 mock：纯常量 + 纯函数直测，零进程边界依赖 | 全节 |

### packages/desktop/src-tauri/crates/core/foundation/src/lib.rs -> packages/desktop/src-tauri/crates/core/foundation/src/prune_test.rs

#### 待测功能

<!-- design.md 公共函数 / API 表未列该文件行——变更仅 `pub mod prune;` 模块声明与 lib doc 注记（第二消费者出现才下沉的正触发注记），无独立行为面。 -->

- lib.rs 模块出口: `pub mod prune;` 声明（prune 公共面经 crate 根可达——编译期锚）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| 模块出口编译锚 | 正向 | prune_test 经 `foundation::prune` 路径引用 `PRUNED_DIR_NAMES` / `is_pruned_dir_name` 编译通过（`#[cfg(test)]` 挂载沿 identity_test 先例——挂载即出口存在性证明，零额外导出） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| （无） | 零 mock：编译期出口锚 | 全节 |

### packages/desktop/src-tauri/crates/core/foundation/src/layout/mod.rs -> packages/desktop/src-tauri/crates/core/foundation/src/layout/mod_test.rs

#### 待测功能

- ReportSlot: 报告槽位封闭二值枚举（`Test` 缺省 / `Baseline`；纯进程内零 serde / specta）
- change_reports(): 槽位驱动报告目录单点（后缀经 `report_dir_name(slot)` 单点映射）
- change_reports_in(): change 目录变体（exec-outputs 锚定消费）
- change_baseline_reports(): baseline 变体命名出口（= `change_reports(.., Baseline)`）
- change_baseline_reports_in(): baseline 变体命名出口（change 目录形态）
- change_test_reports(): 修改（签名不变）——改委托 `change_reports(.., Test)`，既有调用点路径逐字节一致
- change_test_reports_in(): 修改（签名不变）——同上 change 目录形态

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| ReportSlot | 正向 | 二值封闭构造互异：`Test != Baseline`（`Debug` / `Clone` / `Copy` / `PartialEq` / `Eq` 派生可用；零 serde 实现面——纯进程内枚举不入 IPC） | 枚举遍历 |
| change_reports() | 正向 | slot=Test → `<root>/openspec/changes/<change>/reports/test`；slot=Baseline → `…/reports/baseline`（两值逐字对拍——槽位单点派生） | 枚举遍历 |
| change_reports_in() | 正向 | change 目录形态同构：`<change_dir>/reports/test` / `<change_dir>/reports/baseline`，且与 root 变体后缀同源（同 slot 两变体仅前缀差异断言） | 枚举遍历 |
| change_baseline_reports() / change_baseline_reports_in() | 正向 | 命名变体委托等值：与 `change_reports(root, change, Baseline)` / `change_reports_in(change_dir, Baseline)` 逐字节相等（委托单点防漂移） | 单次 |
| change_test_reports() / change_test_reports_in() | 正向 | 既有签名回归持衡：返回值与本变更前 `reports/test` 路径逐字节一致（改委托零路径漂移——既有断言面更新而非退役）；与 `change_reports(.., Test)` 等值 | 单次 |
| change_reports() | 边界 | 不存在 root / root 指向文件 / 空 root → 纯推导无 IO 不 panic（沿 `config_path` / `change_test_reports` 先例形态）；同输入重复调用等值（确定性）；尾分隔符 root 不产双分隔符 | 单次 |
| change_reports() | 边界 | 超长 change 名（>1000 字符）与特殊字符名（emoji / `\n`）原样拼接零清洗零 panic（清洗权威在 create 前置——纯拼接负向锚） | 单次 |
| 报告目录常量锚 | 边界 | `BASELINE_REPORTS_DIR_NAME == "baseline"` 逐字锚定 + 既有 `reports` / `test` 常量组锚随动；`report_dir_name(slot)` 两臂穷尽映射（Test→test / Baseline→baseline 无第三后缀） | 枚举遍历 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| （无） | 零 mock：纯路径推导直测（零 IO 断言面沿既有 layout 测试形态） | 全节 |

### packages/desktop/src-tauri/crates/core/workflow/src/state.rs -> packages/desktop/src-tauri/crates/core/workflow/src/state_test.rs

#### 待测功能

- StepNode.as_str(): 补 `Clean => "clean"` 臂（二十值 → 二十一值封闭集词面单点；位序 MutationTest 后）
- StepNode.group(): `Clean` 派生 `StepGroup::Git`（Git 步族 Commit / Rebase / Merge / Cleanup / Finalize / Clean）
- StepNode.role(): `Clean` 恒 `None`（非 agent 步、零 prompt 零角色）
- RunStepKind.as_str(): 补 `Clean => "clean"` 臂（与 StepNode 同词面 1:1）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| StepNode.as_str() | 正向 | `StepNode::Clean` 出线 `"clean"` 逐字；serde wire 出线 `json!("clean")`（snake 线词与 as_str 一致） | 单次 |
| StepNode.as_str() | 边界 | 二十一值全词汇穷举逐字对拍（既有二十值持衡 + 新臂入集；`len() == 21` 无遗漏无重复、Clean 位序在 MutationTest 后） | 枚举遍历 |
| StepNode.as_str() | 异常 | 非法词反序列化拒绝持衡：camel 近似词 `"clear"` / `"clean step"` / 空串 / 越集词仍拒绝（扩词不松动封闭集防线） | 单次 |
| StepNode.group() | 正向 | `StepNode::Clean` → `StepGroup::Git`；与 Commit / Rebase / Merge / Cleanup / Finalize 同组六值断言 | 单次 |
| StepNode.group() | 边界 | 二十一值逐值分组穷举（clean 入 Git、agent 步 / gate 步 / toolStep 归组持衡零漂移） | 枚举遍历 |
| StepNode.role() | 正向 | `StepNode::Clean` → `None`（非 agent 步） | 单次 |
| StepNode.role() | 边界 | 二十一值逐值 role 穷举（clean 与 exec 族 None 面一致、agent 步角色持衡） | 枚举遍历 |
| RunStepKind.as_str() | 正向 | `RunStepKind::Clean` → `"clean"`；wire 词面与 as_str 逐字一致 | 单次 |
| RunStepKind / StepNode 同源 | 边界 | 逐字同源对拍表补 `(Clean, Clean, "clean")` 行（二十一值 1:1 零错位） | 单次 |
| RunStepKind 反序列化 | 异常 | 越集词形拒绝（封闭集外词一律 Err——二十值 → 二十一值后边界外词形仍拒） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| （无） | 零 mock：中性类型 + serde_json 直驱词面断言，零进程边界依赖 | 全节 |

### packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table_test.rs

#### 待测功能

- INITIALIZE_PHASE_ID: `pub const INITIALIZE_PHASE_ID: &str = "initialize"`（walker legacy 短路判定消费锚）
- allowed_backtrack_phases(): 迭代增 `.filter(|phase| phase.backtrackable)`——initialize 恒不入任何相位的回溯白名单（无位置推导排除；`idx == 0` 早退保留）
- phase_table(): requirement → 10 相位（initialize 表头）、test-only → 7 相位（initialize 表头），其余 `None` 不退化

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| phase_table() | 正向 | requirement 表 10 相位表序：initialize → proposal → dev-design → test-design → implement → test-gen → test-execution → code-review → acceptance → archive（`REQUIREMENT_PHASE_ORDER` 夹具 9 → 10 扩位、initialize 首列） | 单次 |
| phase_table() | 正向 | test-only 表 7 相位表序：initialize → proposal → code-analyze → test-design → test-gen → test-execution → archive（`TEST_ONLY_PHASE_ORDER` 夹具 6 → 7 扩位） | 单次 |
| phase_table() initialize 表项 | 正向 | 两表 initialize `PhaseDefinition` 形态：`description == "基线检查"`、steps 五步序 PhaseStart → Clean → StaticCheck → TestExecution → PhaseLog（PhaseStart / Clean / PhaseLog 均 `on_fail=None`；StaticCheck / TestExecution 均 `on_fail=Some(OnFail::Terminal)`；全 `prompt_ref=None`）、`artifact_manifest` 空、`prerequisites=[]`、`executor=None`、`evaluator=None`、`backtrackable=false`（确定性相位声明全字段锚） | 单次 |
| phase_table() initialize 表项 | 边界 | 两表共用同一 static 单源：requirement 与 test-only 的 initialize steps 逐位等值（`INITIALIZE_STEPS` 两表同源——沿 PROPOSAL_STEPS 复用先例断言形态）；步序首 PhaseStart / 末 PhaseLog / Clean 相位内独占 | 单次 |
| phase_table() backtrackable 字段 | 边界 | 既有 15 条目全部补 `backtrackable: true`（两表逐条断言）；initialize 为唯一 `false` 条目（表内自描述属性全量对拍） | 枚举遍历 |
| phase_table() | 异常 | 未知 workflow_type → `None` 持衡（含超长 / 特殊字符 / 空白 + 大小写归一命中既有面回归——扩表不改变分派拒绝面） | 单次 |
| INITIALIZE_PHASE_ID | 正向 | 常量逐字 `"initialize"`（与两表表头相位 id 相等——单源同值断言） | 单次 |
| allowed_backtrack_phases() | 正向 | 任意后续相位白名单不含 initialize：两表各相位（proposal … archive）遍历断言 `!contains("initialize")`；全表覆盖场景（requirement archive / test-only archive）白名单 = 除 initialize 外全部前序相位按表序 | 枚举遍历 |
| allowed_backtrack_phases() | 正向 | 属性过滤语义持衡：requirement acceptance 白名单 = proposal … code-review 八相位（表序前置集语义不因表头插入漂移——排除仅由 `backtrackable` 承载非位置推导）；test-only test-design 白名单含 proposal + code-analyze | 单次 |
| allowed_backtrack_phases() | 边界 | initialize 自身作 current_phase → 空白名单；proposal 仍空白名单（表首「自身无前序」语义持衡——idx==0 早退非排除机制） | 单次 |
| allowed_backtrack_phases() | 边界 | 空表 / 表外相位 / 空串 → 空 Vec 不 panic（既有边界持衡） | 单次 |
| prerequisites stale 面 | 边界 | 两表 initialize `prerequisites == []`（无依赖入边）且任何相位 prerequisites 不含 initialize（`dependents` 下游遍历空——stale 传播 BFS 零波及的结构前提，AC-7 半边） | 枚举遍历 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| （无） | 零 mock：编译期 const static 表直读断言，零 IO | 全节 |

### packages/desktop/src-tauri/crates/core/workflow/src/write/mod.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table_test.rs

#### 待测功能

<!-- design.md 公共函数 / API 表未列该文件行——变更仅再导出清单增 INITIALIZE_PHASE_ID（walker 消费出口），无独立行为面。 -->

- write/mod.rs 再导出面: `workflow::write::INITIALIZE_PHASE_ID` 可达（经 crate 公共出口消费）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| 再导出编译锚 | 正向 | phase_table_test 经 `workflow::write::INITIALIZE_PHASE_ID` 路径引用并与 `phase_table::INITIALIZE_PHASE_ID` 同值（再导出零漂移——walker import 面的出口存在性证明） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| （无） | 零 mock：编译期出口锚 | 全节 |

### packages/desktop/src-tauri/crates/core/orchestration/src/state.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/state_test.rs

#### 待测功能

- ChangeStepKind.as_str(): 补 `Clean => "clean"` 臂（二十值 → 二十一值；serde camelCase 线词 `clean` 与 snake 线词同形）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| ChangeStepKind.as_str() | 正向 | `ChangeStepKind::Clean` → `"clean"`；serde wire 出线 `json!("clean")`（camel 与 snake 同形逐字） | 单次 |
| ChangeStepKind 词汇 | 边界 | 二十一值穷举互异无碰撞（`len() == 21`、wire 集合互异——既有二十值断言扩位；Clean 位序 MutationTest 后） | 枚举遍历 |
| ChangeStepKind 反序列化 | 异常 | 越集词拒绝：`"Clean"` / `"clean_up"` / `"cleanUp"` / 空串等封闭集外词形一律 Err | 单次 |
| ChangeStepKind 词面同源 | 边界 | `ChangeStepKind::Clean` 与 `workflow::state::RunStepKind::Clean` 的 `as_str()` 逐字同源（身份串词面对拍表补行） | 单次 |
| ChangeStepKind 往返 | 正向 | `"clean"` 序列化 / 反序列化双向派生往返等值（rename_all camelCase 线格式持衡） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| （无） | 零 mock：纯词面 / 派生断言（serde_json 直驱），零进程边界依赖 | 全节 |

### packages/desktop/src-tauri/crates/core/orchestration/src/run_history.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/run_history_test.rs

#### 待测功能

<!-- design.md 公共函数 / API 表显式注记：mapped_step 为私有映射函数，不入公共函数表——条目为 design.md 变更清单声明；finish_command 为本文件既有公共落包入口（本次零签名改动）。 -->

- mapped_step(): 补 `ChangeStepKind::Clean => RunStepKind::Clean` 臂（二十一值 1:1 直映射零过滤）
- finish_command(): 落包入口——initialize run 节点（含 clean）零过滤入 command.nodes、串行 forward 边入 `RunEdgeRecord`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| mapped_step | 正向 | `ChangeStepKind::Clean` → `RunStepKind::Clean` 直映射（新臂逐对断言——同词面逐字） | 单次 |
| mapped_step | 边界 | 二十一值全枚举 1:1 同词面零过滤（`len() == 21`；既有二十值断言扩位） | 枚举遍历 |
| finish_command 落包 | 边界 | initialize run 五节点（phase_start / clean / static_check / test_execution / phase_log）零过滤入 command.nodes（无静默丢弃）；串行 forward 边序逐位断言 | 单次 |
| finish_command 落包 | 异常 | 悬空边 / 空包既有透传语义持衡（扩词不改变落包边界行为——回归锚） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| （无） | 零 mock：纯映射函数 + 进程内 history 构造直测 | 全节 |

### packages/desktop/src-tauri/crates/core/orchestration/src/port.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/port_test.rs

#### 待测功能

- ToolCommand::StaticCheck / ToolCommand::TestExecution（变体形态）: 各增 `slot: ReportSlot` 载荷字段（`MutationTest` 不增——基线检查明确不含 mutation）
- StaticCheckRunner.run(): trait 方法签名增 `slot: ReportSlot` 第三参（槽位透传）
- TestExecutionRunner.run(): trait 方法签名增 `slot: ReportSlot` 第三参
- ArchiveVcsPort.restore_baseline(): 新增 trait 方法 `fn restore_baseline(&self, worktree: &Path, exclusions: &[&str]) -> Result<String, String>`（worktree 在场判定驻调用方、port 零探测方法）
- StaticCheckOutcome.report_dir: 增 `report_dir: String` 字段（infra 按槽位派生回填，pass / fail 均携——与 `TestExecutionOutcome.report_dir` 对称）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| ToolCommand 变体形态 | 正向 | `StaticCheck { change_id, slot }` / `TestExecution { change_id, slot }` 构造与载荷承接：slot=Test / Baseline 两值原样承载、change_id 归键承接（既有封闭集 match 穷尽分发锚扩 slot 断言面）；`MutationTest` 变体零 slot 字段（编译面——不含 mutation 声明锚） | 枚举遍历 |
| ToolCommand slot 载荷 | 边界 | slot 经 `Copy` 传递不耗损（ReportSlot 纯进程内枚举零 serde——不进 IPC 线面的类型锚） | 单次 |
| StaticCheckOutcome.report_dir | 正向 | 字段构造 / 读取：pass / fail 两态各携非空 report_dir 串（与 `TestExecutionOutcome.report_dir` 字段对称面对拍） | 单次 |
| StaticCheckOutcome.report_dir | 边界 | 字段缺省 / 空串构造面不 panic（回填语义由 infra 真件节断言——port 面只锚字段在场可构造可读取） | 单次 |
| StaticCheckRunner.run / TestExecutionRunner.run | 正向 | 三参 `(root, name, slot)` 透传断言：假件 runner 记录调用三元组，slot=Test / Baseline 两值原样到达（trait 为被测 API 显式注入依赖） | 枚举遍历 |
| StaticCheckRunner.run / TestExecutionRunner.run | 异常 | 假件可编程 `Err(String)` 面：Err 串原样透传（签名扩展不吞错误面——回归锚） | 单次 |
| ArchiveVcsPort.restore_baseline() | 正向 | trait 面可注入：测试假件实现该面（Ok 恢复摘要串 / Err 可编程 + worktree 与 exclusions 载荷捕获），walker 消费面经 `Arc<dyn ArchiveVcsPort>` 注入编译通过并按可编程结果分派 | 单次 |
| ArchiveVcsPort.restore_baseline() | 边界 | `exclusions: &[&str]` 载荷承接：多元素切片（名单 + `".*"` + 域目录名形态）与空切片两态原样到达假件（D6 排除面透传契约） | 单次 |
| ArchiveVcsPort.restore_baseline() | 异常 | `Err` 面契约：`Result<String, String>`（错误串组装内容归 infra/vcs 真件节——port 面锚签名与透传，不重复断言文案） | 单次 |
| ArchiveVcsPort.restore_baseline() | 边界 | port 零探测方法：trait 无 exists / probe 新面（worktree 在场判定驻调用方——假件仅实现既有方法族 + restore_baseline 即编译通过，编译面锚定） | 单次 |
| trait 契约 | 边界 | trait 对象 `Send + Sync` 契约随新方法持衡（既有断言面扩展不破） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| StaticCheckRunner / TestExecutionRunner / ArchiveVcsPort 测试假件 | 沿既有 port_test 假件形态扩展：可编程产出（Ok / Err / conclusion 三态）+ 调用三元组与载荷捕获（trait 为被测 API 显式注入依赖） | 签名透传、slot 载荷承接、restore_baseline 缝契约 |

### packages/desktop/src-tauri/crates/core/orchestration/src/steps.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/steps_test.rs

#### 待测功能

<!-- design.md 公共函数 / API 表未列该文件行——execute 的 StaticCheck / TestExecution 两分支 slot 透传为变更清单声明（LocalToolSteps 组合不变、id → name 解析面零改动），经既有 execute 公共入口驱动。 -->

- LocalToolSteps.execute(): `StaticCheck` / `TestExecution` 两分支命令解构增 slot 并透传 `static_check.run(&step.root, &name, slot)` / `test_execution.run(&step.root, &name, slot)`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| execute slot 透传 | 正向 | `ToolCommand::StaticCheck { slot: Baseline }` → FakeRunner 收 `(root, name, Baseline)` 三元组原样（slot=Test 同构）——TestExecution 分支同式经 FakeTestExecutionRunner 断言 | 枚举遍历 |
| execute slot 透传 | 边界 | slot=Test 缺省相位行为与本变更前逐字节一致（既有工具步回归持衡——签名扩展零语义漂移）；id ≠ name 形态下 name 解析单点照常（解析面零改动锚） | 单次 |
| execute 审计行 | 正向 | baseline 槽位 static_check / test_execution 步照常落 `StepRecord` 审计行（reference 携 baseline report_dir——审计面零改动回归） | 单次 |
| execute | 异常 | id → 记录 → name 解析 miss 既有 Err 语义持衡（slot 透传不改变解析失败面——回归锚） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| FakeRunner / FakeTestExecutionRunner（沿既有 steps_test 假件扩展 slot 记录） | 可编程产出 + `(root, name, slot)` 调用捕获（runner trait 为注入依赖） | slot 透传断言 |
| FakeStore（沿既有内存假件 store） | 内存记录 + 审计行捕获（DB 进程边界以注入 trait 假件替代） | 审计行断言 |

### packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs

#### 待测功能

<!-- design.md 公共函数 / API 表显式注记：step_clean / step_static_check_terminal / step_test_execution_terminal / legacy 短路落账助手为 walker.rs 私有 async 执行体、Clean 路由臂为 run_step 内臂，均不入公共函数表——initialize 五面行为经既有 walk_run 公共入口驱动。 -->

- run_step(): 增 `StepNode::Clean` 路由臂（initialize 步族区段分派至 step_clean）
- step_clean(): 主仓双保险（root == main_root → passed + 跳过因，正常不可达）→ 排除面组装（PRUNED_DIR_NAMES + `".*"` + `domain_dir_name()`）→ `ctx.vcs.restore_baseline`：Ok → passed + detail 摘要；Err → failed 节点 + `Terminal::failed`
- step_static_check_terminal(): 单发执行体——单节点 running → spawn（slot=Baseline）→ pass / fail 收口；static 红 → failed 节点 + Terminal（D7 文案 + baseline 报告目录，`{dir}` 经 `StaticCheckOutcome.report_dir` 供给）
- step_test_execution_terminal(): 单发执行体——Error / Fail 分文案 Terminal；pass 挂 `cursor.test_outcome` 后 Proceed
- drive(): legacy 短路分支（`INITIALIZE_PHASE_ID && request.root == request.main_root` → PhaseStart 开相 + PhaseLog `skipped=true` + 空 checklist + 跳过因落账；三检查步零执行；停等照常）
- initialize PhaseLog: 机械 checklist 三条目零改动复用（steps 含 TestExecution 步 → `cursor.test_outcome` 自动挂载 → 三分支天然分派）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| initialize 五步序（经 walk_run 驱动） | 正向 | 全绿链：phase_start → clean → static_check → test_execution → phase_log 节点 emit 序逐位一致（五节点 phase=initialize、串行 forward 边）；PhaseRecord 落 pass 条目（skipped=false、verdict=pass、三会话槽位恒 None——AC-1 / AC-4） | 单次 |
| step_clean 排除面组装 | 正向 | restore Ok → clean 节点 passed + detail 恢复摘要；FakeVcs 捕获 exclusions 逐项断言 = `PRUNED_DIR_NAMES` 六名 + `".*"` + `openspec`（`domain_dir_name()` 派生——D6 组装面，排除决策驻 walker） | 单次 |
| step_clean | 异常 | restore Err → clean 节点 failed + run 以 `Terminal::failed` 收敛（Err 串附 git 语境）；PhaseRecord 零 initialize 条目；后续检查步零执行（AC-5 Err 半边） | 单次 |
| step_clean 主仓双保险 | 边界 | `request.root == request.main_root` 直达 → 步 passed + legacy 跳过因、零 vcs 调用（FakeVcs 调用记录空——正常不可达防御分支，相位级短路先行；AC-6 主仓红线护栏半边） | 单次 |
| Terminal 前置分派 | 正向 | 步声明 `on_fail=Some(OnFail::Terminal)` → 走单发 terminal 执行体（单节点 running → 一次 spawn → 收口；无反馈边、零 agent 会话注入）；缺省相位（implement 等工具步）照旧走 `feedback_limit` 分支（缺省无 Terminal 声明仍视为计划漂移拒绝——分派不混流，D4 半边） | 单次 |
| step_static_check_terminal | 异常 | static 红（FakeRunner passed=false）→ static_check 节点 failed 携 output_ref + run `Terminal::failed`；终态文案 = D7 static 红文案（含「基线静态检查未通过」与 baseline report_dir——AC-2） | 单次 |
| step_test_execution_terminal | 异常 | `conclusion=Error` → Terminal + 环境文案（「基线测试执行出错（环境 / 配置问题）」+ 报告目录）；`conclusion=Fail` → Terminal + 测试红文案（「基线测试未通过」+ 报告目录）——三态文案互异可辨（与 static 红文案亦互异，AC-3） | 单次 |
| step_test_execution_terminal | 正向 | test pass → `cursor.test_outcome` 挂载（outcome 携 baseline report_dir）→ Proceed 至 PhaseLog（机械 checklist 消费前提） | 单次 |
| Terminal 收敛与一次性 | 边界 | 任一 Terminal 失败 → run failed、PhaseRecord 零 initialize 条目、零 eval 条目、TestExecution 步不执行（static 红先停——步序短路）；再发起（二次 walk_run）→ `phase_next` 仍路由 initialize、clean 清残留后全链干净重试 | 重试链 |
| drive() legacy 短路 | 正向 | `request.root == request.main_root` 的 initialize → phase_start + phase_log 两枚节点留痕、PhaseLog 以 `skipped=true` + 空 checklist + 跳过因（TXT_SKIP_LEGACY_INITIALIZE）落账；Clean / StaticCheck / TestExecution 三步零执行（FakeVcs 与 FakeTools 调用记录空——零 vcs / 零检查 spawn、主仓零重置，AC-6） | 单次 |
| drive() legacy 短路 | 边界 | 短路后停等照常（手动档 ConfirmWait 零豁免特判）；后续相位照常路由（skipped 条目一次性持久同 pass 语义——`has_phase_passed` 两态皆认） | 单次 |
| 一次性 pass 跳过 | 边界 | initialize pass 落账后二次发起 → `phase_next` 路由 proposal（initialize 不重跑——PhaseRecord 持久跳过）；auto 推进面同构（AC-4） | 重试链 |
| initialize PhaseLog 机械 checklist | 正向 | steps 含 TestExecution → `cursor.test_outcome` 自动挂载 → `step_phase_log` 三分支走机械 checklist：三条目落账（mutation 恒 None → 第 3 条 evidence「null（未跑）」如实留痕）、report 携 conclusion + 计数 + baseline 报告目录、三会话槽位恒 None（D8 零新分支复用断言——AC-4） | 单次 |
| ToolCommand 构造点 | 边界 | initialize 两 terminal 执行体构造 `ToolCommand` 携 `slot=Baseline`；缺省相位（implement static-check 门禁步等）照旧 `slot=Test`（FakeTools 命令捕获断言——构造点分派单源） | 单次 |
| 落库组合（链路：walker → run_history → RunNodeRecord） | 组合 | initialize 五节点零过滤落 `RunNodeRecord`（step 线词 phase_start / clean / static_check / test_execution / phase_log）+ 串行 forward 边入 `RunEdgeRecord`（AC-8 落库半边） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `ArchiveVcsPort` 假件 FakeVcs | 沿既有 walker_test 假件扩展 `restore_baseline`：Ok 摘要 / Err 可编程 + worktree 与 exclusions 载荷捕获（git 子进程边界以注入 port 假件替代——被测 API 显式注入依赖） | step_clean 全路径、排除面组装、legacy 零 vcs 断言 |
| `ToolStepPort` 假件 FakeTools | 沿既有装置：写面预录产出 + `ToolCommand`（含 slot）捕获 | Terminal 分派、slot 构造点断言、机械 checklist 驱动 |
| `StaticCheckRunner` / `TestExecutionRunner` 假件 | 沿既有假件族扩展 slot 记录：可编程 passed / conclusion（Pass / Fail / Error 三态）/ report_dir | 三态文案、test_outcome 挂载、静态红收敛 |
| `WorkerAgentPort` / `WorkflowSnapshotPort` 假件 | 沿既有 FakeWorker / StubSnapshot（initialize 无 agent 面——调用记录空即「零 agent 注入」断言半边） | 全链驱动、确定性相位零 agent 断言 |
| store（`TestDb` 真件） | 真实 tempfile `Store::open_workspace`（沿既有 TestDb 装置——db 真件组合，不 mock） | PhaseRecord pass / skipped / 零条目断言、节点落库断言 |

### packages/desktop/src-tauri/crates/infra/checks/src/static_check.rs -> packages/desktop/src-tauri/crates/infra/checks/src/static_check_test.rs

#### 待测功能

- ProcessStaticCheck.run(): 签名增 `slot: ReportSlot`（同 trait）——报告锚点 / 输出记录按槽位派生、`StaticCheckOutcome.report_dir` 回填（pass / fail / 无配置直过三分支均携）；执行语义（退出码映射 / 程序不可达显式 Err）零分叉

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| run slot=Baseline | 正向 | pass 场景：`StaticCheckOutcome.report_dir` = baseline 报告树锚点、`output_ref` 落盘；`.log` 输出记录落 `<change>/reports/baseline/exec-outputs/<exec_id>.log`（经 `record_exec_output` slot 透传——AC-1 报告分离半边） | 单次 |
| run slot=Baseline | 异常 | fail 场景（shim 非零退出）：passed=false + diagnostics；report_dir 仍回填 baseline 锚（fail 分支亦携——walker static 红文案供给面） | 单次 |
| run slot=Test | 正向 | 缺省槽位回归：报告路径与 exec-outputs 落 `reports/test`，与本变更前逐字节一致（既有断言面持衡——改签名零路径漂移） | 单次 |
| run 无配置直过 | 边界 | `openspec/config.json` 无 static_analysis 配置 → 直过 passed、零 spawn；report_dir 亦回填本槽位锚点（语义 = 本槽位报告树锚点——「无配置分支 report_dir 亦回填」锚） | 单次 |
| run | 异常 | 程序不可达显式 Err 持衡（裸名不在 PATH / 相对路径不可达——spawn 前解析前置；slot 参数化不改变 Err 面，回归锚） | 单次 |
| run 槽位等价性 | 边界 | slot=Test / Baseline 两值行为等价断言：同 shim 同退出码下 passed / diagnostics / output_ref 语义仅锚点不同（执行语义零分叉锚） | 枚举遍历 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| 平台 shim 脚本 + `openspec/config.json` | 真件形态沿既有 static_check_test 装置（`TempWs` + `write_shim`（stdout / stderr 交错 + 可选退出码）+ `write_config` + PATH 前置窗口 RAII）——spawn 为被测本体不 mock | 全节 |
| 报告树 / 文件系统 | 真实 tempdir 落盘 + 回读断言（fs 不 mock） | report_dir / exec-outputs 锚点断言 |

### packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod.rs -> packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod_test.rs

#### 待测功能

- ProcessTestExecution.run(): 签名增 `slot: ReportSlot`——`reports_dir` 经 `foundation::layout::change_reports(root, change, slot)` 派生；复用门（`read_existing_summary` / `prune_roots`）读本槽位树 summary（跨树零复用 + 树内 mtime 复用判定零改动）；`outcome` 内 `record_exec_output` 增 slot 透传；聚合 / 诊断 / 框架矩阵零改动

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| run slot=Baseline | 正向 | 报告落 baseline 树：`summary.json` + 逐 suite 子报告 + `exec-outputs/<exec_id>.log` 落 `reports/baseline`（与 `reports/test` 零混居——目录树逐文件断言，AC-1） | 单次 |
| run slot=Test | 正向 | 缺省槽位回归：`reports/test` 路径与本变更前逐字节一致（既有 reports_dir 断言面持衡） | 单次 |
| 复用门树内复用 | 正向 | baseline 树内 mtime 复用：输入未变（mtime 持衡）→ 二次 baseline run 复用本树 summary 零重 spawn、`exec_id` 引用既有记录；clean 触碰输入 mtime 推进 → 复用失效重跑（判定逻辑零改动——树内语义持衡） | 重试链 |
| 复用门跨树隔离 | 异常 | 跨树零复用：`reports/test` 树存在可复用 summary 时 slot=Baseline run 不复用（反向同构：baseline 树 summary 不被 slot=Test 相位步复用）——reports_dir 槽位隔离天然成立断言 | 单次 |
| run 三态载荷 | 异常 | `conclusion=Error`（shim 环境坏）→ outcome conclusion=Error + `report_dir`=baseline 锚；`conclusion=Fail` → 测试红 + baseline 锚（walker 分文案消费的载荷供给面——字段级断言，AC-3 半边） | 单次 |
| run 槽位等价性 | 边界 | slot 两值行为等价：同 fixture 下 conclusion / 四计数 / findings 装配仅报告锚点不同（聚合 / 诊断 / 框架矩阵零分叉锚） | 枚举遍历 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| 测试框架 shim（`set_shim_env` / `path_window`） | 真件形态沿既有 testexec mod_test 装置（shim 脚本产出 summary / 退出码可编程 + PATH 窗口）——spawn 为被测本体不 mock | 全节 |
| 报告树 / 文件系统 | 真实 tempdir（mtime 经 `set_mtime` 夹具操控——fs 不 mock） | 复用门树内 / 跨树断言 |

### packages/desktop/src-tauri/crates/infra/checks/src/testexec/detect.rs -> packages/desktop/src-tauri/crates/infra/checks/src/testexec/detect_test.rs

#### 待测功能

<!-- design.md 公共函数 / API 表未列该文件行——is_pruned_path 为模块私有、私有 PRUNED_DIR_NAMES 常量退役改引 foundation 单源（design.md 变更清单声明）。 -->

- is_pruned_path(): 组件名判定委托 `foundation::prune::is_pruned_dir_name`（私有名单退役；路径组件迭代语义驻消费点）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| is_pruned_path 单源化 | 正向 | 既有剪枝行为持衡：fixture 树内 node_modules / dist / build / target / coverage / .nyc_output 与 dot 目录不入 plan / mtime 面（既有 detect 断言面保持——私有名单退役零行为漂移） | 枚举遍历 |
| 单源对拍 | 边界 | 同一批路径组件两侧对拍：detect 剪枝结果与 `foundation::prune::is_pruned_dir_name` 逐组件等值（无第二份名单的结构证明） | 单次 |
| 剪枝面 mtime | 边界 | 既有「剪枝面目录内改动不计入 mtime」断言随单源化持衡（回归锚） | 单次 |
| 两名单不串味 | 边界 | 复用门扫描 `EXCLUDED_DIR_NAMES` 为独立集（CLI `REUSE_SCAN_EXCLUDED_DIRS` 同源）不入上移面：与 `PRUNED_DIR_NAMES` 集合互异断言（detect 内两名单边界锚） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| （无） | 零 mock：tempdir fixture 树真实 fs 直测（沿既有 detect_test 装置） | 全节 |

### packages/desktop/src-tauri/crates/infra/exec/src/lib.rs -> packages/desktop/src-tauri/crates/infra/exec/src/lib_test.rs

#### 待测功能

- record_exec_output(): 签名增 `slot: ReportSlot` 第三参（输出记录按槽位锚定 `<report_dir>/exec-outputs/<exec_id>.log`；exec_id 铸造与路径校验零改动）
- read_exec_output_in(): 同 change 目录双锚探测（`reports/test/exec-outputs/` 先、`reports/baseline/exec-outputs/` 后），exec_id 跨树唯一首中即返、miss 仍 `None`（签名不变，行为双锚）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| record_exec_output slot=Baseline | 正向 | 输出落 `<change>/reports/baseline/exec-outputs/<exec_id>.log`（单一 `.log` 零 `.err` 残留）；exec_id 非空且重复调用互异（铸造零改动锚） | 单次 |
| record_exec_output slot=Test | 正向 | 缺省槽位回归：落 `reports/test` 树，与本变更前逐字节一致（既有断言面持衡） | 单次 |
| read_exec_output_in 双锚 | 正向 | baseline 树 exec_id → `Some` 内容逐字节（首锚 test miss → 次锚 baseline 命中）；test 树 exec_id → `Some`（首锚命中即返）；同一 change 两树并存输出各自回查可达 | 单次 |
| read_exec_output_in 跨树唯一 | 边界 | 两树各落一份输出 → 各自 id 回查得各自内容、无串树（exec_id 跨树唯一——首中即返不歧义）；两树均 miss 的未知 id → `None` | 单次 |
| read_exec_output_in | 异常 | 非法 exec_id（空串 / `..` / 路径分隔符 / 盘符 / `.log` 后缀）→ `None` 持衡（双锚探测不松动路径逃逸防线——回归锚） | 单次 |
| read_exec_output_in | 边界 | 空输出（`Some(vec![])`）与超长（>1000 字节）+ 特殊字节（`\n` / `\0` / emoji）保真（两树各验一形态——引用式零截断沿既有边界） | 单次 |
| read_exec_output 顶层 | 组合 | 顶层函数签名零改动：经 root 变体回查两树 id 均可达（委托 `read_in` 天然双锚——`read_exec_output` IPC 命令零改动回查覆盖面的组合半边，AC-1 / AC-8） | 单次 |
| exec_output_dir 槽位版 | 正向 | 槽位参数版派生对拍：slot 两值目录与 `foundation::layout::change_reports_in(change_dir, slot)` 等值（锚点单点委托——`exec_output_dir_in` 无第二份后缀拼接） | 枚举遍历 |
| record_exec_output | 异常 | 落盘目录不可建（报告树路径被文件占位）→ `Err(String)` 串语义持衡（回归锚） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| （无） | 零 mock：真实 tempdir 报告树落盘 + 回查往返（沿既有 lib_test 装置——fs 为被测组合面） | 全节 |

### packages/desktop/src-tauri/crates/infra/vcs/src/git.rs -> packages/desktop/src-tauri/crates/infra/vcs/src/git_test.rs

#### 待测功能

<!-- design.md 公共函数 / API 表未列该文件行——restore_baseline 为 pub(crate) spawn 单点（公共消费面经 ProcessArchiveVcs），条目为 design.md 变更清单声明。 -->

- git::restore_baseline(): `git -C <worktree> reset --hard` → `git -C <worktree> clean -fd -e p1 … -e pn`（逐项排除、不带 `-x`）；成功返回恢复摘要串、失败 Err 沿既有 `git()` 语境

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| restore_baseline tracked 重置 | 正向 | 已跟踪文件改动（编辑）+ staged 改动 → 恢复后 `git status` 干净、内容回 HEAD 版本（`reset --hard` 半边，AC-5） | 单次 |
| restore_baseline untracked 清除 | 正向 | 未跟踪新文件被清除；排除面六目录（node_modules / dist / build / target / coverage / .nyc_output 逐目录夹具）原样保留（`clean -fd -e` 半边，AC-5） | 枚举遍历 |
| restore_baseline dot 与工件树 | 正向 | 未跟踪 dot 目录（`.idea` 等）经 `-e ".*"` 保留；未跟踪 `openspec/` 工件树经显式 `-e` 保留（bootstrap 依赖与过程文档不丢，AC-5） | 单次 |
| restore_baseline gitignore 免疫层 | 边界 | 不带 `-x`：被 `.gitignore` 规则覆盖的目录（依赖树常态）保留；无 ignore 规则仓的同名目录经 `-e` 双保险保留（两种仓形态各一——`-x` 缺席语义锚） | 单次 |
| restore_baseline 幂等 | 边界 | 干净仓（零改动零 untracked）二次调用 Ok 且仓态不变；exclusions 空切片 → 纯 reset + clean -fd 照常 Ok（幂等安全，AC-5「幂等安全」半边） | 重试链 |
| restore_baseline 摘要 | 正向 | 成功返回恢复摘要串非空（step detail 消费面——产出形态锚） | 单次 |
| restore_baseline | 异常 | 非 git 目录 / 空仓异常面 → `Err` 沿既有 `git()` 语境（命令串 + 退出码 + stderr——不吞不静默） | 单次 |
| restore_baseline 排除面边界 | 边界 | 嵌套排除名（排除目录内再建同名子目录）与 Windows 路径分隔语义（`git clean -e ".*"` 覆盖面——design 待决注记的真件护栏验证面） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| （无） | 零 mock：真实 git tempdir 仓真件直驱（沿既有 init_repo / `test_path_lock` 装置；git 子进程为被测本体不 mock） | 全节 |

### packages/desktop/src-tauri/crates/infra/vcs/src/lib.rs -> packages/desktop/src-tauri/crates/infra/vcs/src/lib_test.rs

#### 待测功能

- ProcessArchiveVcs.restore_baseline(): 委托 `git::restore_baseline` 单点；`Err` 经 `map_err` 附 worktree 路径与「主仓未受影响」注记

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| ProcessArchiveVcs.restore_baseline() | 正向 | 转发对拍：经 `&dyn ArchiveVcsPort` 调用与 `git::restore_baseline` 自由函数同仓同排除面双跑效果逐字等值（沿既有清单 / 补丁转发对拍形态——单点复用防漂移） | 单次 |
| ProcessArchiveVcs.restore_baseline() | 异常 | 非 git 目录 → `Err` 含 worktree 路径字面量与「主仓未受影响」注记（`map_err` 语境面——与 git 侧原文可辨） | 单次 |
| ProcessArchiveVcs.restore_baseline() | 边界 | 主仓双保险真件锚：真实主仓 + worktree 分置 tempdir，主仓预置未提交改动 → restore 调用后主仓未提交改动原样在案（restore 仅作用于 worktree——AC-6 灾难面护栏） | 单次 |
| ProcessArchiveVcs 构造 | 正向 | `new()` / `Default::default()` 等值构造既有断言随新方法持衡（回归锚） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| （无） | 零 mock：真实 git tempdir 仓 + 真实 worktree 分置（沿既有 lib_test 转发对拍与 worktree_slot 装置） | 全节 |

### packages/desktop/src-tauri/crates/infra/store/src/model.rs -> packages/desktop/src-tauri/crates/infra/store/src/model_test.rs

#### 待测功能

<!-- design.md 公共函数 / API 表未列该文件行（RunNodeRecord.step 仅字段 doc 注释随动，表结构零变化零迁移）——条目为 design.md 变更清单 / 类型定义表声明的词汇随动面。 -->

- RunNodeRecord.step: `RunStepKind` 词汇白名单式扩展 + `clean`（二十一值封闭集；旧库可读零迁移）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| RunNodeRecord.step 词汇 | 正向 | `RunStepKind::Clean` 节点构造 / 序列化 / 反序列化往返等值（serde 线词 `"clean"` 白名单式扩展可用） | 单次 |
| RunNodeRecord.step 词汇 | 边界 | 二十一值全词汇落行 / 读回零过滤（既有穷举断言 20 → 21 扩位——零迁移：行内词面扩展旧库可读） | 枚举遍历 |
| RunNodeRecord.step 词汇 | 异常 | 越集词既有拒绝面持衡（回归锚——doc-only 改动零行为漂移） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| （无） | 零 mock：中性类型 + 真实 tempfile store 往返（沿既有 model_test 真件形态） | 全节 |

### packages/desktop/src-tauri/crates/infra/store/src/store.rs -> packages/desktop/src-tauri/crates/infra/store/src/store_test.rs

#### 待测功能

<!-- design.md 公共函数 / API 表未列该文件行（run 节点读面 doc「二十值全词汇零过滤」→「二十一值」注释随动，零逻辑改动）——条目为 design.md 变更清单声明的词汇随动面。 -->

- run 节点读面: 二十一值全词汇零过滤（clean 线词入读面封闭集）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| run 节点读面 | 正向 | `step=clean` 节点写入 → 读回 step 原样（零过滤零改写——doc-only 随动的行为回归锚） | 单次 |
| run 节点读面 | 边界 | 二十一值穷举写入 / 读回往返（既有断言 20 → 21 扩位） | 枚举遍历 |
| run 节点读面 | 异常 | 既有读侧错误语义（坏行显式 Err 不静默）持衡（回归锚） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| （无） | 零 mock：真实 tempfile store 真件（沿既有 store_test 形态） | 全节 |

### packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs -> packages/desktop/src-tauri/crates/core/workflow/src/queries/detail_test.rs

#### 待测功能

<!-- design.md 公共函数 / API 表未列该文件行（节点行线面纯 derive 零字段属性、pipeline 出线随 phase_table 自动跟随零代码改动）——条目为 design.md 变更清单声明的线面随动面；另收语料 golden 组合节。 -->

- detail pipeline 出线: requirement 10 列 / test-only 7 列（initialize 首列）——出线随 `phase_table` 自动跟随
- 详情节点投影 stepKind 线面: 二十一值全词汇出线零过滤（doc 二十值 → 二十一值注释随动）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| pipeline 列数 | 正向 | requirement detail pipeline 10 站（initialize 首列、archive 末列）；test-only 7 站（initialize 首列、archive 末列）——既有 9 / 6 列断言扩位（AC-8） | 单次 |
| pipeline 列数 | 边界 | initialize 站 attempts 空与有条目（pass / skipped 两形态）出线（legacy skipped 条目读侧透出——呈现面零结构改动前提） | 单次 |
| 详情节点投影 stepKind | 正向 | initialize run 五节点 stepKind 全量出线（含 `clean`——二十一值零过滤，既有断言 20 → 21 扩位） | 枚举遍历 |
| 详情节点投影 stepKind | 异常 | db 坏行 / 越集词既有拒绝面持衡（扩词不改变读侧错误语义——回归锚） | 单次 |
| 语料组合（链路：corpus_golden_test 种子 → change_detail 投影 → golden） | 组合 | 语料补 initialize 相位样本（正常 pass 条目 + legacy skipped 条目两形态）与 clean 步词节点样本（二十一类词汇随动）→ golden 全量对比随动；重写经 `DESKTOP_GOLDEN_REWRITE=1` 显式留痕（fixtures/ 矩阵随动，AC-8 语料半边） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| db store | detail_test 沿既有进程内假件 store（id 键记录表 + 可编程序列——注入依赖假件）；corpus 节真实 tempfile Store 真件 | 详情读链投影断言 / 语料 golden 组合 |
| 磁盘产物树 | 真实 tempdir 合成产物树（corpus 节既有构造器） | 语料 golden 组合 |
| golden 文件 | 入仓 golden 快照 + `DESKTOP_GOLDEN_REWRITE` 环境变量开关（进程边界经测试环境注入） | golden 重写留痕 |

### packages/desktop/src-tauri/src/commands/change_flow/mod.rs -> packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs

#### 待测功能

- change_flow_start（IPC）: 签名与四道前置校验零改动（initialize 挂相位表头，发起面零感知）——真件 spawn 组合回归（基线红停 / 基线绿走）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| change_flow_start 组合（链路：命令面 → walk_run） | 组合 | 基线绿走：真实 git 主仓 + worktree 夹具（exec root ≠ 主 root）+ 绿 static shim（`write_static_shim` 退出 0 + `openspec/config.json` 登记 + `prefix_path` PATH 注入，commands 级 PATH 锁串行化）→ initialize 相位先于 proposal 路由、Clean / StaticCheck / TestExecution / PhaseLog 节点齐备、检查在 worktree 内执行、报告与 exec-outputs 落 `openspec/changes/<name>/reports/baseline/`（AC-1 真件全链） | 单次 |
| change_flow_start 组合 | 组合 | 基线红停：shim 配置为红（非零退出）→ run 以 Terminal 收敛 failed、终态文案引导在基线上修复后重新发起；PhaseRecord 零 initialize 条目；shim 转绿后重新发起 → clean 恢复基线后干净重试（AC-2 真件全链） | 重试链 |
| change_flow_start 前置校验 | 边界 | 四道既有前置校验回归持衡（未建档 / workflow_type 未命中 / exec root 缺席 / 并行 run 拒绝——零改动回归锚）；invoke 提前 resolve 不悬挂（事件经 Channel 流出——既有形态持衡） | 单次 |
| 停等计数随动 | 边界 | STOP_BEFORE_ARCHIVE 语义回归：initialize 入前序路由相位计数（requirement 9 个前序 / test-only 6 个前序）后 stop_before 恒 `"archive"` 行为不变——既有停等用例回归持衡（AC-4 停等半边） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| （无） | 零 mock：`tauri::test::MockRuntime` + 真实 `WorkspaceStores`（tempdir 数据根）+ 真实 git 主仓 / worktree 夹具 + 平台 shim 真件 spawn + PATH 前置窗口（沿既有 mod_test 真件先例——spawn 为被测组合本体） | 全节 |

### packages/desktop/src/views/changes/flow/run-state.ts -> packages/desktop/src/views/changes/flow/run-state.test.ts

#### 待测功能

<!-- design.md 公共函数 / API 表未列该文件行——normalizeKind / stepWord / groupOf 为模块私有归一函数（design.md 变更清单声明），经既有导出面（unifiedRunNodes / runStepNodes / nodeIdentityKey）与内联 fixture 直测。 -->

- normalizeKind(): `clean` 为 identity 词走 `default` 分支直返（snake / camel 同形零显式 case；词汇注释 20 → 21）
- stepWord(): 反向归一 `clean` 回 snake 词面（round-trip 恒等）
- groupOf(): git 组判定补 `step === 'clean'`（clean 归 git 分类）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| normalizeKind | 正向 | `'clean'` → `'clean'`（identity 词走 default 分支直返——与 commit / rebase / merge / finalize / preflight / seal / cleanup 先例一致，零显式 case） | 单次 |
| normalizeKind / stepWord | 边界 | 二十一值全量对拍表更新（snake ↔ camel 期望表补 clean 行，词汇漂移即断）；round-trip `stepWord(normalizeKind(word))` 恒回 snake 词 | 枚举遍历 |
| groupOf | 正向 | `'clean'` 归 git 分类（git 臂补值——经 runStepNodes / 节点分组出线断言，与 commit / rebase / merge / finalize / cleanup 同组） | 单次 |
| groupOf | 边界 | 二十一值逐词分组穷举（clean 入 git、既有归组零漂移——agent / gate / toolStep 三类持衡）；未知步词 default 回落不崩持衡（回归锚） | 枚举遍历 |
| nodeIdentityKey | 正向 | clean 节点身份键 `{phase}:{attempt}:clean:{instanceNo}`（stepWord 词面——与后端边表端点逐字可比） | 单次 |
| stepFromRecord / unifiedRunNodes | 正向 | 库读史行 step='clean'（RunStepKind 线词）→ 统一视图节点归一出线（双源合成对新词透明） | 单次 |
| stepFromRecord / unifiedRunNodes | 异常 | 既有越集词拒绝 / 载荷缺省边界持衡（回归锚——归一面不因扩词松动） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| （无） | 零 mock：纯函数直测 + 内联 fixture（stepRow / runStepRow 构造器沿既有形态），零 react 零进程边界 | 全节 |

### packages/desktop/src/views/changes/flow/run-step-node.tsx -> packages/desktop/src/views/changes/flow/run-step-node.test.tsx

#### 待测功能

<!-- design.md 公共函数 / API 表未列该文件行——STEP_LABEL 为组件内私有常量表（design.md 变更清单声明），经 RunStepNode 组件渲染面直测。 -->

- STEP_LABEL: `Record<ChangeStepKind, string>` 封闭联合补 `clean: '基线恢复'` 词条
- RunStepNode: clean 节点渲染——词条出线、git 组徽章、基线输出经既有 `outputRef` stdout 面板直达（零渲染改动）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| STEP_LABEL | 正向 | `clean` → 「基线恢复」词条渲染（`data-testid="run-step-kind"` 出线断言——新键值对拍） | 单次 |
| STEP_LABEL | 边界 | `Record<ChangeStepKind, string>` 封闭联合键集恰等二十一词零缺键（编译期穷尽 + 运行时穷举兜底断言 20 → 21 更新） | 枚举遍历 |
| git 分类徽章 | 正向 | clean 节点归 git 组徽章（group='git' 与 commit / rebase / merge / finalize / cleanup 同列可辨——AC-8 观测半边） | 单次 |
| 基线输出回查 | 正向 | 基线检查节点携 outputRef → 既有 stdout 面板零渲染改动直达（回查经后端 `read_exec_output` 双锚覆盖 baseline 树——前端零结构改动的组合半边，沿既有 outputRef 面断言回归，AC-1 / AC-8） | 单次 |
| RunStepNode 渲染 | 边界 | 载荷缺省形态（detail=null / sessionId=null / outputRef=null）既有降级渲染持衡（扩键不改变缺省面——回归锚） | 单次 |
| RunStepNode 渲染 | 异常 | 既有异常载荷回归面持衡（零渲染崩溃，回归锚） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| （无） | 零 mock：组件直渲染（沿既有 run-step-node.test 形态，零 IPC / 零依赖注入面） | 全节 |

### packages/desktop/src/views/changes/flow/phase-column-node.tsx -> packages/desktop/src/views/changes/flow/phase-column-node.test.tsx

#### 待测功能

<!-- design.md 公共函数 / API 表未列该文件行——PHASE_LABEL 部分映射与 phaseLabel 回落函数为组件内私有（design.md 变更清单 / D9 声明），经 PhaseColumnNode 渲染面直测。 -->

- PHASE_LABEL: 部分映射单键 `initialize: '基线检查'`（D9——不自造全相位标签新约定）
- phaseLabel(): 未命中返回 raw id 回落（其余相位沿用 id 原词呈现先例）
- 列头文本: 经 `phaseLabel(data.phase)` 渲染（initialize 相位列头人读标签）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| PHASE_LABEL / phaseLabel | 正向 | phase='initialize' 列头渲染「基线检查」（data-testid 锚——新映射键出线） | 单次 |
| phaseLabel 回落 | 边界 | 未命中回落原词：'proposal' / 'dev-design' / 'archive' 等其余相位列头渲染 raw id（回落语义——既有相位 id 原词呈现先例保持，零全相位标签新约定） | 枚举遍历 |
| phaseLabel 回落 | 边界 | 回落稳健面：空串 / 未知相位串 / 超长串（>1000 字符）/ 特殊字符（emoji）→ 原样返回不 panic 不截断 | 单次 |
| 列头渲染 | 正向 | 10 列（requirement）/ 7 列（test-only）形态下列头经 phaseLabel 渲染零结构改动（initialize 首列入列、既有列容器渲染断言回归持衡——AC-8） | 单次 |
| 列头渲染 | 异常 | 既有缺省字段 / 载荷异常回归面持衡（回归锚） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| （无） | 零 mock：组件直渲染（沿既有 phase-column-node.test 形态，零 IPC 零进程边界） | 全节 |

## 不可测试项

| 模块 / 事项 | 原因 |
|-------------|------|
| `packages/desktop/src/types/generated/bindings.ts` | 生成物（specta 管线自动再生，非手写测试面）：`RunStepKind` / `ChangeStepKind` TS 联合类型 + `"clean"` 由 `pnpm -C packages/desktop run bindings:export` 产出；漂移由 `bindings:check` 静态守卫拦截（静态步骤由桌面端执行，agent 内不重复）。词面正确性的可执行半边由 Rust 侧 state_test / run_history_test 二十一值断言与前端 run-state.test 对拍表承接。 |
| `packages/desktop/package.json`（version 0.5.12 → 0.5.13） | 版本交付面零行为可断言：AC-8「全管线绿」为静态门（static-check + 前后端测试 + bindings 一致性守卫——桌面端执行）；`src-tauri/Cargo.toml` 版本不随动、`plugins/dev-team` 仍 2.10.44 属静态核对。 |
| `phase_table.rs` 私有 static `INITIALIZE_STEPS` | 非导出面（两表共用 static，同 `PROPOSAL_STEPS` 先例，design 公共函数表注记明确不入表）：不以测试为目的新增导出（No test-only exports 纪律）；五步声明经 `phase_table()` 两表 initialize.steps 逐位等值 + 全字段断言覆盖。 |
| `walker.rs` 词汇常量（`TXT_SKIP_LEGACY_INITIALIZE` 与 baseline 三态失败文案） | 私有 const——经 walker emit 面（PhaseLog report 跳过因 / Terminal 终态 reason / 节点 detail）出线断言覆盖，不直接对常量本身断言。 |
| AC-8「全管线绿」（static-check + 前后端测试 + bindings 一致性守卫） | 静态检查与测试执行为机械步骤，由桌面端 test-execution / static-check 承接（agent 内不重复执行、不复制测试运行脚本细节）。 |
| `plugins/dev-team`、`openspec/explores/` 与 CLI 工作流 / workflow.json、突变面（`MutationRunner` / Stryker / cargo-matants 执行链）、`phase_next.rs` 路由、`ChangeRecord` / db 表结构 | 「不要修改」清单项（零改动）：不属被测范围；路由含表头由 phase_table 表驱动声明断言间接锚定（`phase_next.rs` 零改动的前提面）。 |




