# desktop-checks-domain Specification

## Purpose

Desktop 后端检查域（checks 边界，边界分类学第六类）：外部进程 + pass/fail 结论门禁（static-check / test-execution）的领域组织——纯层落位 `crates/core/checks`（报告 schema 模型 / 解析器 / 聚合判定 / 诊断树 / 复用门，零 spawn 零 Tauri），进程执行落位 `crates/infra/checks`（crate 裸名 `checks-runtime`）；runner port 归属消费者留在 `core/orchestration::port`，spawn 不进 core。

## Requirements

### Requirement: checks 边界定义与分流法则

Desktop 后端 SHALL 以 checks 边界（边界分类学第六类）承接检查域。walker 直接调用的执行步按执行性质分流：外部进程 + pass/fail 结论门禁（static-check、test-execution）SHALL 落 checks 边界——契约与纯层落位 `crates/core/checks`，进程执行落位 `crates/infra/checks`（crate 裸名 `checks-runtime`）；外部进程 + agent 会话（executor / evaluator / decision）SHALL 留 agent 边界；外部进程 + 纯上下文文本（git diff）。

#### Scenario: 检查步归 checks 边界

- **WHEN** 评审 static-check 与 test-execution 执行步的落位
- **THEN** 二者声明 checks 边界（core/checks 纯层 + infra/checks 执行），infra/agent 内无检查域成员寄居

#### Scenario: 会话步留 agent 边界

- **WHEN** 评审 executor / evaluator / decision 会话步的落位
- **THEN** 留 agent 边界（compose_turn 会话租户），checks 边界不承载任何 agent 会话

### Requirement: core/checks 纯层零进程边界

`crates/core/checks` SHALL 承载检查域纯层：报告 schema 模型（对齐 CLI zod 权威 schema）、coverage 解析器（istanbul / llvm-cov / coverage 文本）、阈值与 override 聚合判定（conclusion 判定）、完整性校验与诊断树确定性分支、复用门判定（mtime 新鲜度比较纯函数）。core/checks MUST NOT 进程 spawn、MUST NOT 依赖 tokio process、MUST NOT 依赖 Tauri；解析与聚合 SHALL 为纯函数，以 fixtures 直驱可测（corpus 黄金语料先行，文件系统扫描等 IO 侧落 infra）。

#### Scenario: 纯函数 fixtures 直驱

- **WHEN** 以真实 summary.json / report.json fixtures 驱动 core/checks 的解析器与聚合判定
- **THEN** 无需进程、无需文件系统即可得出结论与诊断分支，测试全程进程内完成

#### Scenario: 零 spawn 审查

- **WHEN** 审查 core/checks 源码的依赖声明与进程创建调用
- **THEN** 无 tokio process 依赖、无进程 spawn、无 Tauri 依赖

### Requirement: infra/checks 进程执行层与 static_check 平移

`crates/infra/checks` SHALL 承载检查域进程执行：`static_check.rs`（ProcessStaticCheck）自 infra/agent 平移，本轮**启用 recorded 输出记录并改为 `2>&1` 交错合并**；`testexec/`（test-execution runner：框架探测、命令模板展开、spawn / 超时 / 输出捕获、报告写盘编排）。Windows shell shim（cmd /C）与程序解析前置检查（PATH / cwd 探测、可执行后缀候选——防 shell 吞命令缺失烧反馈边预算）SHALL 复用 static_check 既有经验。输出捕获 SHALL 经 infra/exec 的 `ExecOutput` 记录模式（见「输出记录收敛到 infra/exec」），checks 层 MUST NOT 保留私有 stdout 捕获 / 落盘分支。

static-check 平移后的语义变更（其余平移等价项不变——无配置 / 空命令直过、配置在位而程序不可达显式 `Err`、退出码映射）：

- **`2>&1` 交错合并**：spawn 前把脚本改为 `{command} 2>&1`，stderr 并入 stdout 单一管道，两 fd 指向同一管道、写入顺序即真实顺序；`diagnostics = output.stdout`（交错，更还原终端），`output.stderr` 为空。
- **默认 recorded**：`ProcessStaticCheck` 去 `record_anchor` 构造，`StaticCheckRunner::run(root, name)` 收 name，`execute(root, name)` 直接以 `record_exec_output(root, name, output.stdout)` 落单一 `.log` 并回填 `output_ref`（不再有 `recorded()` 装配分支）。

#### Scenario: 平移零逻辑改动

- **WHEN** ProcessStaticCheck 及其测试自 infra/agent 迁入 infra/checks
- **THEN** 执行语义逐行等价（无配置直过 / 程序不可达显式 Err / 退出码映射不变），平移后测试全绿，infra/agent 无 static_check 残留

#### Scenario: Windows .cmd shim 可达

- **WHEN** runner 在 Windows 上执行 jest / vitest / npx 类 `.cmd` shim 命令
- **THEN** spawn 前完成程序解析前置检查（含可执行后缀候选），程序缺失显式 `Err` 停给用户而非落 passed=false 反馈边空转

#### Scenario: static-check 2>&1 交错合并

- **WHEN** static-check 命令同时向 stdout 与 stderr 交错写入（如先 stdout 一行、再 stderr 一行、再 stdout 一行）
- **THEN** spawn 后的 `output.stdout` 为三行按真实顺序的单一交错流、`output.stderr` 为空；`diagnostics` 逐字等于该交错流，MUST NOT 为「stdout 全量 + stderr 全量」的分离拼接

#### Scenario: static-check 默认 recorded

- **WHEN** 组合根以 `ProcessStaticCheck::new()` 装配并调用 `run(root, name)`
- **THEN** `StaticCheckOutcome.output_ref` 为 `Some(exec_id)`（落盘失败降级 `None`，不阻断检查主流程）；`<report_dir>/exec-outputs/<exec_id>.log` 在案且内容等于交错 stdout，无 `<exec_id>.err` 文件

### Requirement: checks 依赖规则与 port 归属

依赖方向 SHALL 为：`checks`（core）→ `config` + `foundation`（core→core 消费 typed tests[] 模型，先例：config 唯一出口纪律）；`checks-runtime`（infra）→ `orchestration`（port 类型消费，agent-runtime 同款先例）+ `checks` + `config` + `foundation`。runner port（`TestExecutionRunner` / `StaticCheckRunner` / `ToolCommand` / `ToolStepOutput`）SHALL 留 `orchestration::port` 原地（port 属于消费者；移 port 牵动 ToolStepOutput 封闭集引用链）；`ToolStepOutput::TestExecution` 最小载荷类型定义于 orchestration::port，core/orchestration MUST NOT 依赖 core/checks（组合根装配时由 checks 域产出映射为 port 载荷）。spawn 不进 core 红线延续：core/checks 零 spawn，进程执行全部落 infra/checks。

#### Scenario: Cargo.toml 依赖审查

- **WHEN** 检查 checks 两 crate 的 `Cargo.toml`
- **THEN** `checks` 的 workspace 内依赖仅 `config` + `foundation` 且零 Tauri；`checks-runtime` 依赖 `orchestration` + `checks` + `config` + `foundation` 且零 Tauri

#### Scenario: orchestration 零依赖 checks

- **WHEN** 检查 core/orchestration 的依赖声明
- **THEN** 无 `checks` 边（最小载荷类型定义于 orchestration::port 本地）；port.rs 三件套（TestExecution 命令 / 产出 / Runner port）原地扩展

### Requirement: 输出记录收敛到 infra/exec

stdout / stderr 记录职责 SHALL 从 checks 层收敛到 `crates/infra/exec`：`SystemCommand` 之外 SHALL 提供两种输出记录模式 `ExecOutput`：

- `ExecOutput::Transient`：spawn 即得、反馈边消费、即弃（无 id，全量不进库 / 不落盘引用）；
- `ExecOutput::Recorded { exec_id }`：stdout 落库 / 落盘、可回查（有 id 可查；`exec_id` 供 `RunNodeRecord.output_ref` 等引用，见 desktop-change-state-store「run 执行图落库」）。

`record_exec_output` SHALL 收敛为**单一 `.log`**（`.err` 退役）：单一交错流（static-check 的 `2>&1` 交错、test-execution 的 stdout+stderr 合并流）即完整还原终端，MUST NOT 再分文件落 stdout / stderr。`read_exec_output(root, change, exec_id)` SHALL 读同一 `.log`（未知 / 非法 `exec_id` → `None`，miss 非错误）。

static-check SHALL 由「默认 `Transient`」改为**默认 `Recorded`**（`run(root, name)` 收 name，`output_ref` 恒回填；反馈边仍消费 `diagnostics` 文本，全量经 `output_ref` 可回查）；test-execution 的 `report_dir` 与 `exec_id` SHALL 统一口径（报告全量仍落 change 报告目录，`exec_id` 为回查引用，record 内容为脚本真实输出——见 desktop-test-execution）。新增 desktop-app 命令 `read_exec_output(root, change_name, exec_id) -> Option<String>`（UTF-8 lossy；miss=None）暴露 `infra/exec::read_exec_output` 给前端 stdout 面板。agent 会话记录 SHALL 继续走 `SessionEventRecord`（`session_id` 重放唯一来源），MUST NOT 内嵌进 `ExecOutput`（不与转录单表不变量冲突）。`ExecOutput::Recorded` 的落库载体（独立 exec 输出表 vs 落盘文件 + 引用）由 design 定稿，但 MUST NOT 全文进 `RunNodeRecord.detail`（引用式与截断式口径不变）。

#### Scenario: 两模式各司其职

- **WHEN** 某检查 spawn 产出仅需反馈边消费的 stdout，与某需回查的 spawn stdout
- **THEN** 前者可经 `ExecOutput::Transient` 即弃；后者以 `ExecOutput::Recorded { exec_id }` 可经引用回查全量；checks 层无私有 stdout 捕获分支

#### Scenario: 记录单点

- **WHEN** 审查 infra/exec 与 infra/checks 源码
- **THEN** stdout 捕获 / 落库 / 落盘逻辑单点驻 infra/exec；static-check / test-execution 经 `ExecOutput` 消费，无各自私有的捕获或落盘路径

#### Scenario: 会话记录不内嵌

- **WHEN** 审查 WorkerAgent 步的输出记录
- **THEN** 会话记录仍经 `session_id` 引用 `SessionEventRecord` 重放（转录单表不变量不变），MUST NOT 经 `ExecOutput` 内嵌全量会话

#### Scenario: 单一 .log 记录与回查

- **WHEN** static-check（2>&1 交错）或 test-execution（stdout+stderr 合并）产出一段输出并经 `record_exec_output` 落盘
- **THEN** 仅写 `<report_dir>/exec-outputs/<exec_id>.log`（无 `.err`）；`read_exec_output` 读回字节与该单一流逐字一致

#### Scenario: read_exec_output 命令 miss 与 lossy

- **WHEN** 前端以未知 / 非法 `exec_id` 调 `read_exec_output` 命令，或以已知 `exec_id` 调取含非 UTF-8 字节的输出
- **THEN** 前者返回 `None`（miss 非错误）；后者返回 UTF-8 lossy 字符串（非法字节以替换符呈现），不 panic、不报错

### Requirement: mutation 执行面与 MutationRunner port 归属

突变执行 SHALL 落 checks 边界：进程执行落位 `crates/infra/checks/src/mutation/`（testexec/ 旁新模块：分段调度、Stryker / cargo-mutants 模板展开 spawn、outcome 映射、段级与归并报告写盘、sandbox 复用与全段后一次清理），纯层判定（映射表口径、score 公式、段计数归并、阈值判定）落位 `crates/core/checks`（纯函数 fixtures 直驱可测，零 spawn 零 Tauri 红线不变）。`MutationRunner` port SHALL 留 `orchestration::port` 原地（port 归属消费者先例同式，与 `TestExecutionRunner` 并列；`ToolCommand::MutationTest` / `ToolStepOutput::MutationTest` 入封闭集）；`checks-runtime` 依赖面 MUST NOT 扩 vcs——变更文件清单（`change_edit_set`）由 walker 经 vcs 缝取数后以 `MutationRunner::run(root, name, files)` 入参注入，清单获取实现在 `infra/vcs`。突变报告 SHALL 落 change 报告目录 `reports/test/<plan>/mutation/` 子树（段级 `mutation-seg-<i>.json` + 归并块，全程留存），路径经 foundation layout 推导（命名隔离双禁令唯一触点纪律不变）。

#### Scenario: 双层落位与零 spawn 红线

- **WHEN** 审查 mutation 纯层（映射 / score / 归并）与执行模块（spawn / 分段 / 清理）的落位
- **THEN** 纯层在 core/checks（fixtures 直驱、零 spawn 零 Tauri），进程执行在 infra/checks/src/mutation/；core/checks 无 tokio process 依赖

#### Scenario: 清单经入参注入不扩依赖

- **WHEN** MutationRunner 执行需要变更文件清单
- **THEN** 清单由 `change_edit_set`（infra/vcs）产出、walker 经 vcs 缝取数后传入 `run(root, name, files)`；checks-runtime 的 Cargo.toml 无 vcs 依赖

#### Scenario: 报告落 change 报告树

- **WHEN** 突变分段执行完毕
- **THEN** 段级原始报告与归并块落 `reports/test/<plan>/mutation/` 子树（经 layout 推导零路径字面量），全程留存不清理

### Requirement: baseline 报告槽位参数化

`crates/infra/checks` 两 runner（`static_check.rs` 的 `ProcessStaticCheck` 与 `testexec/` 的 test-execution runner）SHALL 支持报告槽位参数化：槽位为封闭二值 `test`（缺省——既有 `reports/test` 锚点，语义零改动）与 `baseline`（initialize 相位基线检查——`reports/baseline` 锚点）。槽位 SHALL 仅参数化报告落点（summary / 子报告 / exec-outputs 同锚派生），执行语义（无配置直过 / 程序不可达显式 Err / 退出码映射 / 框架矩阵 / 复用门判定规则）MUST NOT 因槽位分叉；两槽位的目录派生 SHALL 经 foundation layout 单点（`reports/test` 与 `reports/baseline` 同源变体），runner 内 MUST NOT 出现第二份路径派生。walker 调用面（`StaticCheckRunner` / `TestExecutionRunner` port 签名）的槽位透传形态由 design 定稿；`read_exec_output` 回查面 SHALL 覆盖两槽位的 exec-outputs（exec_id 跨槽位唯一）。

#### Scenario: 缺省槽位零改动

- **WHEN** 以缺省调用形态执行 static-check / test-execution（implement / test-gen / test-execution 相位的门禁步）
- **THEN** 报告与 exec-outputs 落 `reports/test` 锚点，与本变更前逐字节同路径；执行语义零分叉

#### Scenario: baseline 槽位落点与语义不变

- **WHEN** 以 baseline 槽位执行两 runner（initialize 相位基线检查）
- **THEN** 报告与 exec-outputs 落 `reports/baseline` 锚点（layout 单点派生）；static-check 无配置直过 / 程序不可达显式 Err、test-execution 框架矩阵与不支持框架显式 Err 等语义与缺省槽位一致

#### Scenario: 路径派生单点

- **WHEN** 审查 infra/checks 与 infra/exec 源码的报告目录派生
- **THEN** `reports/test` / `reports/baseline` 两变体仅驻 foundation layout 单点；runner 与 exec 层消费该单点，无逐 runner 私有后缀拼接

### Requirement: 依赖与工具产物树名单共享单源

依赖与工具产物树名单（`node_modules` / `dist` / `build` / `target` / `coverage` / `.nyc_output` + dot 目录剪枝——语义为「依赖与工具产物树，探测与清理均应跳过」）SHALL 自 `crates/infra/checks/src/testexec/detect.rs` 的私有 `PRUNED_DIR_NAMES` 上移 foundation 公开共享单点：testexec detect 剪枝与 infra/vcs `restore_baseline` 的 clean 排除面两消费点 SHALL 同源消费该单点，MUST NOT 保留第二份名单（detect 私有常量退役）。名单本体 SHALL 仅承载目录名封闭集与 dot 剪枝语义；`restore_baseline` 消费点的排除面 SHALL 为名单 + `openspec/`（工件与报告树为未跟踪面，MUST NOT 被 clean 删除——增补属 vcs 消费点语义，不入名单本体）。名单扩展 SHALL 经 foundation 单点一处修改两消费点同时生效。

#### Scenario: 两消费点同源

- **WHEN** 审查 testexec detect 的剪枝实现与 infra/vcs restore_baseline 的排除面实现
- **THEN** 两者均消费 foundation 公开名单单点；`detect.rs` 无私有 `PRUNED_DIR_NAMES` 残留，infra/vcs 无第二份名单

#### Scenario: 名单语义仅目录名与 dot 剪枝

- **WHEN** 以名单驱动的探测 / 清理遇到 `node_modules/`、`target/`、`.venv/`（dot 目录）与普通源码目录
- **THEN** 名单内目录与 dot 目录被剪枝 / 保留（按消费点语义），普通源码目录不受名单影响；名单本体不含 `openspec/` 等工件路径

#### Scenario: 单点扩展双点生效

- **WHEN** 向 foundation 名单新增一个依赖产物目录名
- **THEN** detect 剪枝与 restore_baseline 排除面同步生效，零第二处修改

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/checks`（新，裸名 `checks`） | 检查域纯层 | model / parser / aggregate / diagnose / reuse 纯函数；零 spawn 零 tokio process 零 Tauri；依赖 config + foundation；fixtures 直驱 + corpus 黄金可测 |
| `crates/infra/checks`（新，裸名 `checks-runtime`） | 检查域进程执行 | static_check.rs 平移（零逻辑改动）+ testexec/（detect / runner / report）；依赖 orchestration + checks + config + foundation + tokio process；零 Tauri |
| `orchestration::port`（原地不动） | runner port 归属 | `TestExecutionRunner` 与 `StaticCheckRunner` 并列；`ToolCommand::TestExecution` / `ToolStepOutput::TestExecution`（最小载荷）入封闭集；core/orchestration 零依赖 checks |
| `crates/infra/agent`（裸名 `agent-runtime`，收窄） | 会话租户身份回归 | cli / sdk / worker / compose / store_port；static_check 移出 |
| `crates/core/foundation/src/`（名单单点，新） | 依赖与工具产物树名单 | 封闭目录名集 + dot 剪枝判定纯函数；公开共享；detect 与 vcs 两消费点同源 |
| `crates/infra/checks/src/static_check.rs` | static-check 槽位 | 报告锚点按槽位参数化（test 缺省 / baseline）；执行语义零分叉 |
| `crates/infra/checks/src/testexec/`（runner / detect） | test-execution 槽位与剪枝 | 报告锚点按槽位参数化；detect 剪枝改引 foundation 名单（私有 `PRUNED_DIR_NAMES` 退役） |
| `crates/infra/vcs`（restore_baseline，新） | 基线恢复排除面 | 排除面 = foundation 名单 + `openspec/`；实现驻 infra（core 零 spawn 红线），经 `ArchiveVcsPort` 缝消费 |
