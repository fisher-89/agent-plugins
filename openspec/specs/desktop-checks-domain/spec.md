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

### Requirement: 检查域 spawn 统一经 exec 执行体

`crates/infra/checks` 的四类外部进程 spawn（`static_check.rs` 的 static-check、`testexec/runner.rs` 的 suite 命令、`mutation/cargo_mutants.rs` 的 cargo-mutants 段、`mutation/stryker.rs` 的 Stryker 段）SHALL 一律经 `exec::run_shell`（见 desktop-exec-runtime「统一执行体 run_shell 契约」）执行；MUST NOT 各调用点自持 spawn + 超时 + 管道排水实现（三份私有 `read_all` 助手退役），MUST NOT 出现裸 `tokio::time::timeout` + `start_kill` 组合。

四调用点的能力面 SHALL 齐平为「超时 / 进程树击杀 / 双管道 / 取消」四项：

- **超时**：static-check 由**零超时**改为 1200 s（此前 cargo 卡在文件锁等待时永久悬挂）；test-execution suite 1200 s（见 desktop-test-execution）；两 mutation 段 1800 s（不变）。
- **进程树击杀**：超时与取消两臂经 `exec::kill_tree` 杀整树并收尸（Windows `taskkill /T /F`），不再只杀壳。
- **双管道**：执行体并发排水两路；调用方按各自既有口径消费——static-check 仍以命令内 `2>&1` 把 stderr 并入 stdout（`stderr` 恒空、诊断与 record 单流语义逐字不变）；test-execution 与 mutation 仍以「stdout 前缀 + stderr 后缀」拼接合并流。
- **取消**：`cancel: CancelFlag` 由调用方透传（见 desktop-change-orchestration「停止贯通到进程执行」）。

取消在检查域的收敛口径 SHALL 为执行面归因（本变更改判，取代既有过渡形态）：`RunOutcome::Cancelled` → runner 置 `fault = Some(RunFault::Stopped)` 后正常返回产出（MUST NOT 返回 `Err("执行已取消")`）→ walker 以 `Terminal::stopped` 收敛 run `stopped`（见 desktop-change-orchestration「执行面归因 RunFault 与门禁步分派」）。`Fatal` / `Interrupted` 等既有失败分流语义 MUST NOT 因迁移而改变。

#### Scenario: 四调用点审查

- **WHEN** 审查 `static_check.rs` / `testexec/runner.rs` / `mutation/cargo_mutants.rs` / `mutation/stryker.rs` 的进程执行面
- **THEN** 四者均调用 `exec::run_shell` 且各传超时预算与 `cancel`；四文件内无 `tokio::time::timeout`、无 `start_kill`、无私有 `read_all`

#### Scenario: static-check 首获超时

- **WHEN** static-check 命令永不退出（如 cargo 阻塞于 `Blocking waiting for file lock`）
- **THEN** 1200 s 到期后执行体收敛 `TimedOut` 并回收进程树，static-check runner 以 `fault = Some(Environment(超时文案))` 正常返回（MUST NOT 让 run 永久停在 running，MUST NOT 以 `Err` 收场）

#### Scenario: static-check 交错流语义不变

- **WHEN** static-check 命令交错写 stdout 与 stderr（命令串仍带 `2>&1`）
- **THEN** `StaticCheckOutcome.diagnostics` 与 `<report_dir>/exec-outputs/<exec_id>.log` 逐字等于该交错流，`stderr` 恒空（MUST NOT 变成「stdout 全量 + stderr 全量」分离拼接）

#### Scenario: 合并流顺序零变

- **WHEN** test-execution suite 或 mutation 段命令分别向 stdout 与 stderr 写入
- **THEN** 落 record / 失败消息的合并流仍是「stdout 前缀 + stderr 后缀」，与迁移前逐字一致

#### Scenario: 取消即杀不残留

- **WHEN** 用户停止正在运行的检查步（cancel 置位）
- **THEN** 执行体终止进程树、收尸并返回 `Cancelled`，runner 以 `fault = Some(Stopped)` 返回；子进程（含 rustc 类子孙）不再继续写 `target/`

#### Scenario: 既有失败分流不变

- **WHEN** 程序不可达、配置缺失、段报告缺失等既有失败面被触发
- **THEN** static-check / test-execution / mutation 各自维持原有 `Err` / `Fatal` / `Interrupted` 语义与文案骨架（迁移只换执行形态，不改分流）

### Requirement: 瞬态重试与环境错误归因（fault 装配）

检查域三 runner SHALL 把「执行面归因」与「检查结论」分开承载：`StaticCheckOutcome` / `TestExecutionOutcome` / `MutationTestOutcome` 各增 `fault: Option<RunFault>`（`RunFault = Stopped | Environment(String)`，定义见 desktop-change-orchestration「执行面归因 RunFault 与门禁步分派」），`passed` / `conclusion` **MUST 保持既有取值面**（MUST NOT 升三值、MUST NOT 因 fault 改口径）；fault 在位时该值不参与门禁分派（walker 以 fault 先于结论分派）。

- **重试开面**：`static_check.rs` 与 `testexec/runner.rs` SHALL 以 `RetryPolicy::transient()`（见 desktop-exec-runtime「瞬态签名字典与有限重试策略」）执行其 spawn；`mutation/` 三段 SHALL NOT 接 retry（突变长跑是产品语义，段内重试会放大墙钟且复用门已承担断点续跑）。
- **fault 装配表**（static-check / test-execution 两面；逐 `RunOutcome` / 瞬态判定。mutation 的生命周期见下「mutation 专属」——其段超时有意不进 fault）：

| 来源 | `fault` |
|---|---|
| `TimedOut` | `Environment("…超时（N 秒）…")` |
| `Cancelled` | `Stopped` |
| `Completed` + 重试耗尽 + 命中瞬态签名 | `Environment(占用者点名 + 已重试次数 + 重试进展序列 + 末次诊断摘要 + 环境侧处理指引)` |
| `Completed` + 其余非零（真红，无签名） | `None`（走既有反馈边 / 升格 / 基线红文案） |
| 无配置直过（static-check） | `None` |

- **退出面收窄**：环境面（超时 / 瞬态耗尽）MUST NOT 以 `Err` 表达（`Err` 保留给「进程拉不起 / 程序不可达 / 写盘失败 / 配置不支持」等停给用户的基础设施失败——那些面本就没有可重试的执行）；`Cancelled` MUST NOT 再返回 `Err("执行已取消")`。
- **test-execution 专属**：`conclusion = Error` 的成因面 SHALL 收窄为「结果工件解析失败 / harness 崩溃 / 框架探测失败」；超时与瞬态耗尽 MUST 由 fault 承载，且 MUST NOT 落 plan 级 `execution_error`（那会被当作被测代码问题而产生假失败归因）。多 plan 聚合时 fault SHALL 上抛（任一 plan 有 fault 即 outcome 带 fault；`Stopped` 优先于 `Environment`）。
- **诊断与记录**：命中瞬态签名时 SHALL 调 `exec::lockprobe::attribution_text`（仅此刻；常态路径零探针开销）取占用者点名；输出记录仍为**单一 `.log`**（`exec::ExecOutput::Recorded`），内容 SHALL 为全部尝试按序拼接、每段前加分隔头 `===== attempt N/5 · exit=C · transient=<命中签名> =====`（审计不丢上下文）；`diagnostics` / `findings_*` SHALL 反映末次尝试 + fault 归因段（MUST NOT 只记末次而丢掉重试事实）。
- **重试推进可观测（fault 文案补「推进序列」）**：`environment::fault_text` SHALL 收齐尝试面——签名扩为「`attempts` + 末次 `exit_code` + 末次输出字节 + `retried: &[AttemptTrace]`」，并按 `exec::progress_series` 在归因段之后、诊断摘要之前输出一行 `重试进展：待编译 <c1> → <c2> → … → <cN>`（各次尝试的 `Compiling` 行计数，末次在内，单点取数 MUST NOT 在检查域重算计数口径）。`attempts == 1`（未重试）或序列全为 `0`（非 cargo 构建形态）时 MUST NOT 输出该行（无推进可言）；`exec::is_progress_stalled` 为真时该行 SHALL 附「末两次无进展，判定为卡死」——把「重试确实在推进」与「同一批写不进去」变成用户可见证据，并自证卡死早停判据。
- **mutation 专属**：`SegmentFailure::Cancelled` SHALL 映射 `fault = Stopped`（见 desktop-mutation-testing「段执行取消与进程树击杀」）；`Interrupted`（含段超时）与 `Fatal` 的既有分流 MUST NOT 因 fault 面而改变，且 `Interrupted` 面 SHALL 保持 `fault = None`（段超时有意不进 fault）。

#### Scenario: 重试耗尽判环境错误

- **WHEN** static-check 命令每次都以 `failed to rename archive file` + `os error 32` 退出 1，且输出不含 `Compiling ` 行（待编译计数恒 `0`，不触发卡死早停，走满 5 次）
- **THEN** `StaticCheckOutcome.passed == false` 且 `fault == Some(Environment(…))`；fault 文本含占用者点名（或「未定位占用者」降级说明）、被锁文件路径、「已自动重试 4 次」与环境侧处理指引；`.log` 内含 5 段带 attempt 分隔头的输出

#### Scenario: 真红不产 fault

- **WHEN** 静态检查因真实 lint 错误退出 1（输出不含任何瞬态签名）
- **THEN** `passed == false`、`fault == None`、`attempts == 1`；既有定向反馈边 / 基线红分支照旧

#### Scenario: 无配置直过无 fault

- **WHEN** workspace 未配置 static-check 命令
- **THEN** `passed == true`、`fault == None`、零 spawn、零探针

#### Scenario: 超时与取消走 fault

- **WHEN** static-check 或 test-execution 分别以 `TimedOut` / `Cancelled` 收场
- **THEN** 前者 `fault == Some(Environment(超时文案))`、后者 `fault == Some(Stopped)`；两者均不返回 `Err`；test-execution 的 summary 不记 plan 级 `execution_error`

#### Scenario: 环境文案含重试推进序列

- **WHEN** static-check 各次尝试的合并输出分别含 264 / 5 / 5 行 `Compiling <crate> v…` 且均命中锁签名，重试面以卡死早停收口（`attempts == 3`）
- **THEN** `fault` 文本含 `重试进展：待编译 264 → 5 → 5`，并附「末两次无进展，判定为卡死」；同文仍含占用者点名、被锁文件路径、「已自动重试 2 次」与环境侧处理指引

#### Scenario: 未重试无推进序列行

- **WHEN** 环境文案的 `attempts == 1`（首次尝试即耗尽预算 / 无重试面），或序列各次计数全为 `0`（非 cargo 构建形态，如 Node / 链接器报错）
- **THEN** `fault` 文本 MUST NOT 含「重试进展」行；其余归因段（占用者 / 已重试次数 / 诊断摘要 / 指引）照旧装配

#### Scenario: 瞬态耗尽不落 execution_error

- **WHEN** test-execution 某 suite 命令连续 5 次命中瞬态签名并耗尽重试
- **THEN** `TestExecutionOutcome.fault == Some(Environment(…))`；该 plan 的 `execution_error` 为空（不把环境面写成「被测代码问题」），`conclusion` 值不构成门禁结论

#### Scenario: 解析失败仍为 Error 且无 fault

- **WHEN** suite 命令退出 0 但结果工件缺失 / 不可解析
- **THEN** `conclusion == Error`、`fault == None`（真环境面：工件解析 / harness），既有 `execution_error` 归因链不变

#### Scenario: mutation 不接重试且只映射取消

- **WHEN** 审查 `crates/infra/checks/src/mutation/` 的 spawn 调用与 `SegmentFailure` 映射
- **THEN** 三段 spawn 均以 `RetryPolicy::none()` 语义执行（零重试面）；`Cancelled → fault = Stopped`、`Interrupted`（含段超时）→ `fault = None` + `conclusion = Error`、`Fatal` → `Err`

### Requirement: test-execution 跨 plan 总预算

test-execution runner 的 `execute()` 层 SHALL 设**跨 plan 总预算**（代码常量 `TESTEXEC_TOTAL_BUDGET = 2400 s`，MUST NOT 进 config schema）：起点 SHALL 为本次执行开始时刻（与 `started_at` 同一次取时 / 同量级），**进入每个 plan 之前**判定一次——`elapsed >= TESTEXEC_TOTAL_BUDGET` 时余下 plan MUST NOT spawn（含首个 plan 的边界：`elapsed ≈ 0` 恒不触发）。

超预算的收口 SHALL 沿既有 fault 路径（MUST NOT 新写分支）：置 `fault = Some(RunFault::Environment("测试执行总时长超预算（2400 秒）…"))` 并提前 break，整体以 `fault_outcome` 返回——不写 summary、不跑诊断、不落报告文件（半套报告不进复用门）、`report_dir` 与 `exec_id` 照旧回填；已完成 plan 的结果随 fault 收口一并丢弃（与既有 fault 收口同口径，MUST NOT 引入「部分结果落盘」的第三条路径）。既有 fault 聚合规则 MUST NOT 因预算面改变：当前已有 fault 时 SHALL 保留既有 fault（`Stopped` 优先于 `Environment`），预算不覆盖它。

#### Scenario: 多 plan 总时长有上界

- **WHEN** tests[] 声明三个 suite，前两个各耗时 900 s 且均通过（含各自的瞬态重试），进入第三个 plan 时累计已超 2400 s
- **THEN** 第三个 plan 零 spawn，整体以 `fault == Some(Environment(总时长超预算文案))` 收口；`reports/test` 树无本次 summary（半套报告不进复用门），`report_dir` / `exec_id` 仍回填

#### Scenario: 预算内零行为变化

- **WHEN** 本次执行的各 plan 累计耗时未触及 2400 s（常规 1 rust + 1 vite 形态）
- **THEN** 逐 plan 照常执行、聚合与报告写盘零变化；`fault == None`、结论与 count 与既有逐字节一致

#### Scenario: 预算不覆盖既有 fault

- **WHEN** 某 plan 已产出 `fault = Stopped`（用户停止）后执行循环 break（或已记 `Environment` fault）后累计耗时亦超预算
- **THEN** 最终 fault 仍为既有那个（`Stopped` 优先），MUST NOT 被总预算文案覆盖

### Requirement: 覆盖率采集完整性（缺测不假通过）

汇总报告组装 SHALL 判定**覆盖率采集完整性**：任一子报告「自身结论非 pass」（`exit_code != 0` 或 `summary.failed > 0` 或 `coverage.pass == false`）且 `coverage == null`（覆盖率采集缺失）在场时——

- 该 suite MUST NOT 被静默跳过加权：SHALL 为其追加一条 `Problem { problem_type: ExecutionError, framework: <该 suite 框架> }`（消息说明该 suite 覆盖率采集缺失），每 suite 至多一条，且与该 suite 已有问题**去重**（同 suite 已因非零退出落 `execution_error` 时不重复追加）；`determine_conclusion` 既有映射（`has_execution_error → Error`）零改动；
- 聚合 `coverage` 块 MUST NOT 为 `pass == true`：采集不完整时 SHALL 收敛为 `None`（与「无任何实测数据 → None」同源的不可测语义）——部分 suite 的达标数值 MUST NOT 被呈现为整体通过。

「全部非 pass 子报告均有覆盖率」或「无任何非 pass 子报告」或「全部子报告均无覆盖率（本就 `None`）」三形态 SHALL 维持既有加权与判定口径零变。覆盖率数值口径（加权算法 / 阈值 / override）与 `problems` 的其余归并规则 MUST NOT 变化。

#### Scenario: 缺测 suite 不再报覆盖率通过

- **WHEN** rust suite 以非零退出收场且 `coverage == null`（llvm-cov 未产工件），同时 vite-plus suite 覆盖率达标（lines 96.21）
- **THEN** `summary.conclusion == Error`（execution_error 归因在位）、`summary.coverage == None`（MUST NOT 为 `Some(pass: true)`）；`problems` 含该 rust suite 的覆盖率缺失归因

#### Scenario: 去重不重复归因

- **WHEN** 某 suite 非零退出且 `summary.failed == 0`、同时 `coverage == null`（既有 `collect_problems` 已为该 suite 落一条 `execution_error`）
- **THEN** 该 suite 的 `execution_error` 条目仍只有一条（覆盖率缺失归因不追加第二条），消息面不改写既有条目

#### Scenario: 全测齐全时判定零变

- **WHEN** 全部 suite 均产出覆盖率且达标（真件回归形态）
- **THEN** 聚合 `coverage` 为 `Some(pass: true)`、`conclusion` 与既有实现逐字节一致，`problems` MUST NOT 新增条目

#### Scenario: 全无覆盖率时口径不变

- **WHEN** 全部子报告均无覆盖率数据（`coverage == None`，含结论非 pass 的 suite）
- **THEN** 聚合 `coverage` 仍为 `None`（既有口径），`conclusion` 由 `failed` / `execution_error` 定型；既有 `execution_error` 归因条目逐字节不变

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
| `crates/infra/checks/src/static_check.rs` | static-check 执行与装配 | `execute(root, name, slot, cancel)` 经 `run_shell(command, RunOptions { cwd: root, timeout: 1200 s, cancel, retry: RetryPolicy::transient() })`；命令串仍为 `{command} 2>&1`；`diagnostics = report.stdout`、`passed = exit_code == 0`、record 拼全部尝试（attempt 分隔头）；按 fault 装配表填 `StaticCheckOutcome.fault`；命中签名时调 `lockprobe::attribution_text` |
| `crates/infra/checks/src/testexec/runner.rs` | suite 执行与归因 | `SUITE_COMMAND_TIMEOUT` 600 s → **1200 s**；`execute_plan` 经 `run_shell(cwd = plan.cwd, timeout, cancel, retry = RetryPolicy::transient())`；`TimedOut` / 瞬态耗尽 → `fault = Environment(…)`（不产 plan `execution_error`）；`Cancelled` → `fault = Stopped`（不返回 `Err`）；`PlanExecution` 增 `fault` 并上抛；合并流「stdout 前缀 + stderr 后缀」零变；私有 `read_all` 退役 |
| `crates/infra/checks/src/testexec/mod.rs` | 结论与 fault 上传 | 多 plan fault 聚合（`Stopped` 优先）；`conclusion = Error` 成因面收窄为解析 / harness / 框架探测失败 |
| `crates/infra/checks/src/mutation/mod.rs` | 段执行失败面 | 不接 retry；`SegmentFailure::Cancelled` → `fault = Some(Stopped)`；`Interrupted` → `conclusion = Error` + `fault = None`；`Fatal` → `Err` |
| `crates/core/orchestration/src/port.rs` | 载荷 | 三 outcome 各加 `fault: Option<RunFault>`；`passed` / `conclusion` 取值面不变 |
| `crates/infra/exec/src/transient.rs` / `lockprobe.rs` | 消费面 | `RetryPolicy::transient()` / `none()`；`attribution_text(diagnostics) -> Option<String>`（占用者 + 被锁文件 + 降级说明） |
| `crates/infra/checks/src/mutation/cargo_mutants.rs` | 段执行 | `run_segment(patch_path, exec_dir, cancel)` 经 `run_shell(cwd = exec_dir, env = [CARGO_TARGET_DIR], timeout = 1800 s, cancel)`；`TimedOut` → `SegmentFailure::Interrupted`（文案不变）；`Cancelled` → `SegmentFailure::Cancelled`；私有 `read_all` 退役 |
| `crates/infra/checks/src/mutation/stryker.rs` | 段执行 | `run_segment(segment, cancel)` 经 `run_shell(cwd = suite_cwd, timeout = 1800 s, cancel)`；同 `TimedOut` / `Cancelled` 映射；私有 `read_all` 退役 |
| `crates/core/checks` | 纯层 | 零改动（解析 / 聚合 / 复用门 / 映射表均不变） |
| `crates/infra/checks/src/environment.rs` | 环境文案装配（单点） | `fault_text(attempts: u32, exit_code: i32, output: &[u8], retried: &[AttemptTrace]) -> String`——归因段 + 「已自动重试 N 次」+「重试进展：待编译 c1 → … → cN」+ 末次诊断摘要 + 处置指引；`attempts == 1` 或计数全 `0` 时无推进序列行；`is_progress_stalled` 为真时附卡死判定语；`attempts_log` 零改动 |
| `crates/infra/checks/src/static_check.rs` | environment::fault_text 调用点 | `fault_text(run.attempts, run.exit_code, &run.stdout, &run.retried)`（`2>&1` 交错流不变） |
| `crates/infra/checks/src/testexec/runner.rs` | environment::fault_text 调用点 | `fault_text(run.attempts, run.exit_code, &merged, &run.retried)`（合并流「stdout 前缀 + stderr 后缀」不变） |
| `crates/infra/checks/src/testexec/mod.rs` | 跨 plan 总预算 | `TESTEXEC_TOTAL_BUDGET: Duration = 2400 s`（代码常量）；`execute()` 记录起点、进入每 plan 前判定、超预算置 `Environment` fault 并走既有 `fault_outcome`（不写 summary / 不进复用门）；既有 fault 聚合优先级不变 |
| `crates/core/checks/src/aggregate.rs` | 覆盖聚合完整性 | 汇总组装检出「非 pass 且 `coverage == None`」子报告 → 追加去重后的 `ProblemType::ExecutionError`；采集不完整时 `aggregate_coverage` 收敛 `None`（MUST NOT `pass == true`）；`determine_conclusion` / 加权算法 / 阈值 / override 零改动 |
