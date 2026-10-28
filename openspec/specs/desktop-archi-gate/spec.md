# desktop-archi-gate Specification

## Purpose

定义架构漂移的**门禁挂载**：只挂 implement 相位（acceptance / test-only / initialize 一律不挂，且无运行时开关）、结论语义与反馈边（`passed` 只看 `errors` 与模型结构错，`skipped` / `noChanges` 显式「未检查」非通过，反馈文本有界且含固定处置提示，超限升格相位 fail，不自动改代码）、runner port 与报告落盘（port 落 `orchestration`、实现落 `infra/archi`、零进程故 `fault` 恒 `None`、写盘失败降级 `report_ref = None` 而不阻断结论）、输入口径与观察根（模型与源码同根、变更集经 `archive_vcs` 唯一入口且取数失败即步失败、零读工作区配置）。判定语义本身不在本能力：扫描 / 定级 / 报告结构见 `desktop-archi-drift`，本能力只消费其报告并派生步结论。

## Requirements

### Requirement: 门禁只挂 implement 相位

架构漂移门禁 SHALL 只作为 requirement 工作流 implement 相位的计划步出现（`StepNode::ArchiCheck`，相位序列见 desktop-workflow-write-face「相位定义有序 step 序列与 artifact 清单」），且 MUST NOT 出现在其它任何相位：

- **acceptance 不挂**：模型一致性在 implement 已收敛；acceptance 关注验收行为，架构问题转新 change 验收——在此重复门禁只会得到一个「修也修不动」的阻断点。
- **test-only 表不挂**：该表无 implement 相位（代码改动落在 code-analyze，其步序列为 P→E 无工具步）；且测试文件被扫描全排除，即便挂上也必然 `clean` / `skipped`——挂上只会得到一个永远空转的步骤。
- **initialize 不挂**：基线相位只做基线恢复与基线绿门（static-check / test-execution），基线里的架构债不属于本次变更。
- 门禁的启用状态 MUST NOT 依赖任何运行时开关（无配置项、无环境变量、无相位名判断）：挂载点唯一由相位表声明承载。

#### Scenario: implement 相位必经

- **WHEN** 假引擎驱动一次 requirement 工作流 run 走完 implement 相位（executor → static-check 通过）
- **THEN** 执行图出现 `archi_check` 步节点（位于 static-check 与 evaluator 之间），其结论参与相位推进

#### Scenario: 其它相位零触点

- **WHEN** 分别驱动 initialize / dev-design / test-gen / test-execution / code-review / acceptance / archive 相位与 test-only 工作流全相位
- **THEN** 执行图零 `archi_check` 节点、零 `ArchiCheckRunner` 调用、零报告落盘

#### Scenario: 无运行时开关

- **WHEN** 审查 `phase_table.rs`、`steps.rs`、`walker.rs` 与工作区 `config.json`
- **THEN** 门禁的启用与否只由 implement 相位表的步声明决定；无 `archi` 相关配置键、无环境变量分支、无按相位名硬编码判断

### Requirement: ArchiCheck 步结论语义与反馈边

ArchiCheck 步 SHALL 按报告派生结论并分派（执行体见 desktop-change-orchestration「ArchiCheck 步接线与反馈边」）：

- `passed` SHALL 为 `errors.is_empty() && model.structuralErrors.is_empty()`；`warnings` / `observations` / `suppressed` 一律 MUST NOT 影响 `passed`（观察项与提示项不是拦截项）。
- `status = clean` / `warnings` → 步 `passed`；`status = errors` → 步 `failed` 并进定向反馈边（上限 3 次，超限升格相位 fail）。
- `status = skipped`（无模型）/ `noChanges`（无可判文件）→ 步 SHALL `passed`，但 detail MUST 显式写「未检查（无模型）」/「未检查（无可判文件）」，MUST NOT 写成「检查通过」——否则「没检查」被误读成「检查过了没问题」（与报告 `status` 五值同一纪律）。
- 反馈诊断文本 SHALL 由本 runner 侧装配并**有界**：`errors` / `structuralErrors` 逐条渲染（元素对 + 档位 + 出现次数 + 至多两处 `file:line`），整体不超 `MAX_REPORT_CHARS` 量级；全量明细走报告文件，MUST NOT 塞进反馈文本；文本 SHALL 另含一条固定处置提示——「先按模型改代码；若真实设计如此，应回到 dev-design 补模型声明，MUST NOT 在此处硬改代码消违例」（提示来自反馈模板常量，MUST NOT 改动 `prompts/` 静态角色 md）。
- 步失败 SHALL NOT 触发代码自动改写（无自动修复、无文件写入）：改代码是 executor 的活，改模型要回到 dev-design。

#### Scenario: 三态分派

- **WHEN** 分别以 `status = clean` / `warnings` / `errors` 的增量报告驱动本步
- **THEN** 前两次步 `passed`（detail 分携「零违例」「N 条提示」），第三次步 `failed` 并注入一条 executor 修复会话（计数 +1）

#### Scenario: 未检查不等于通过

- **WHEN** 观察根下无架构模型（`status = skipped`），或以 `noChanges` 收场
- **THEN** 步 `passed` 且 detail 逐字为「未检查（无模型）」/「未检查（无可判文件）」；报告 `status` 原样保留，MUST NOT 被呈现为 `clean`

#### Scenario: 反馈文本有界

- **WHEN** 报告含 12 条 `errors`（其中一条聚合了 40 处 `occurrences`）
- **THEN** 注入 executor 的文本按元素对聚合并截断到 `MAX_REPORT_CHARS` 量级（每条至多两处 `file:line`），全量明细可由报告文件回查

#### Scenario: 反馈含处置提示

- **WHEN** 某步以 `errors` 非空收场，反馈注入同一 executor 会话
- **THEN** 文本末含「若真实设计如此，应回到 dev-design 补模型声明」的固定提示；`crates/core/workflow/src/prompts/` 的 16 份角色 md 字节零变化

#### Scenario: 结构错同样拦截且零 panic

- **WHEN** 模型含 `cycle` 结构错（读盘成功、`structuralErrors` 非空）
- **THEN** 步 `failed`、反馈文本含该结构错（file / line / 元素名），检查过程零 panic、零源码写入

### Requirement: ArchiCheck runner port 与报告落盘

门禁执行 SHALL 经 port 缝下沉、实现落 infra 边界：

- **port 归属**：`ArchiCheckRunner` 定义落 `crates/core/orchestration::port`（port 属于消费者，与 `StaticCheckRunner` / `TestExecutionRunner` / `MutationRunner` 同型），产出统一走 `ToolStepOutput::ArchiCheck(ArchiCheckOutcome)`；`ArchiCheckOutcome { passed, diagnostics, fault: Option<RunFault>, report_dir, report_ref: Option<String> }`。
- **实现落位**：实现落 `crates/infra/archi`（crate 裸名 `archi-runtime`）——判定与解析全部复用 `crates/core/archi`，本层只做「取输入 → 调判定 → 落盘 → 派生 outcome」，MUST NOT 含归属 / 定级 / `status` 推导逻辑。
- **零进程**：本 runner MUST NOT 创建任何进程（无 `std::process` / `tokio::process`，零 shell），因此 `fault` 在生产路径恒 `None`（字段保留以与既有三 outcome 同形，见 desktop-change-orchestration「执行面归因 RunFault 与门禁步分派」的表格不受本步影响）；IO 失败（报告写盘 / 目录创建）MUST NOT 返回 `Err`——降级为 `report_ref = None` 并保留结论，主流程不因审计面失败而中断。
- **报告落盘**：增量模式 SHALL 落 `reports/archi/<timestamp>.json`（目录经 `foundation::layout::change_reports(root, name, ReportSlot::Archi)` 派生，MUST NOT 行内字面量），成功时 `report_ref` 为该文件路径、`report_dir` 为目录；MUST NOT 复用 `reports/test` / `reports/baseline`（其三套语义——plans 索引 / 覆盖率复用 / mutation scope——会与漂移报告互相污染）。
- **磁盘面收 name**：port 收 change 磁盘名（`run(root, name, changed, cancel)`——槽位不设形参，恒 `ReportSlot::Archi` 由本 runner 内部取用），id → 记录 → name 的解析在 `steps.rs` 消费点（未建档显式 `Err`，与既有三 runner 同式）。
- **取消**：`cancel` 置位 SHALL 以 `fault = Some(RunFault::Stopped)` 正常返回（MUST NOT 以 `Err` 表达取消）——检查本身毫秒级，取消只在扫描长尾时命中（遍历 / 解析循环内轮询标志）。

#### Scenario: 报告落盘与引用回填

- **WHEN** 以某已建档 change 的 worktree 为根跑一次增量检查（零违例）
- **THEN** `reports/archi/<timestamp>.json` 生成且内容等于报告结构；outcome `passed = true`、`report_dir` 为该目录、`report_ref` 为该文件路径

#### Scenario: 写盘失败不阻断结论

- **WHEN** 报告目录不可写（权限 / 只读盘注入）
- **THEN** outcome 仍以 `passed` 与完整 `diagnostics` 返回、`report_ref = None`（`report_dir` 照常给出预期目录），步不因审计面失败而失败

#### Scenario: 零进程与零第二套判定

- **WHEN** 审查 `crates/infra/archi` 源码
- **THEN** 无进程创建、无 git 调用、无归属 / 定级 / status 推导实现；判定全部来自 `archi` 的漂移判定纯函数

#### Scenario: 槽位不复用

- **WHEN** 检查步跑完后的 change 报告树
- **THEN** 漂移报告只出现在 `reports/archi/`；`reports/test` 与 `reports/baseline` 字节零变化，其 summary 索引不含漂移报告条目

### Requirement: 门禁输入口径与观察根

门禁的检查输入 SHALL 与 run 的执行根严格同源：

- **root**：SHALL 为 run 的 exec root（有 worktree 时 `detail.worktree`，legacy change 为主仓根）——模型与源码同根读盘，MUST NOT 混根。
- **变更集**：增量模式的变更文件清单 SHALL 经 `ArchiveVcsPort::change_edit_set(root, "HEAD")` 由 `steps.rs` 取数后注入（与 mutation 的清单注入同式，唯一入口）；取数失败 SHALL 显式传播为步失败，MUST NOT 降级为空清单（空清单会静默变成 `noChanges` = 假通过）。
- **模型**：SHALL 读同一 root 下的架构模型（`<archi_dir>/models/*.toml`）；模型不存在即 `skipped`（不阻断），模型结构错即不通过（阻断）。
- **不读工作区配置**：本步 MUST NOT 读 `openspec/config.json`（不含 `tests[]` 扩展名矩阵、不含 `static_analysis`）；扫描排除与豁免只来自架构域配置（见 desktop-archi-drift「架构域看护配置」）。

#### Scenario: worktree 根下模型与源码同根

- **WHEN** 某 change 有 worktree，其模型被本 change 修改过，跑 implement 相位门禁
- **THEN** 检查读到的是 worktree 内的模型与 worktree 内的源码；主仓模型与源码零读取

#### Scenario: 变更集取数失败即步失败

- **WHEN** `change_edit_set` 返回 `Err`（git 不可达 / 仓库损坏）
- **THEN** 步以显式失败收场（零反馈会话、零报告落盘），MUST NOT 以空变更集继续并产出 `noChanges`

#### Scenario: 工作区配置零读取

- **WHEN** 审查 `crates/infra/archi` 与门禁链路的配置读取面
- **THEN** 零 `config` crate 依赖、零 `tests[]` / `static_analysis` 读取；`desktop-workspace-config` 的 `KNOWN_KEYS` 与 schema 镜像零变化

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/orchestration/src/port.rs` | port 与产出封闭集 | `ArchiCheckRunner`（`run(root, name, changed, cancel)`——槽位恒 `Archi`）、`ArchiCheckOutcome`、`ToolCommand::ArchiCheck`、`ToolStepOutput::ArchiCheck`（定义与分派见 desktop-change-orchestration） |
| `crates/infra/archi/Cargo.toml`（新） | crate 落位 | 裸名 `archi-runtime`；workspace 内依赖 `archi` + `orchestration`（port 类型）+ `foundation`（报告路径派生）；外部依赖 `serde_json`；零 Tauri / 零 spawn / 零 vcs |
| `crates/infra/archi/src/lib.rs` | 装配出口 | `pub use runner::LocalArchiCheck;`（结构体名 design 可定，零参构造） |
| `crates/infra/archi/src/runner.rs` | 门禁执行体 | `impl ArchiCheckRunner for LocalArchiCheck`：调 `archi::drift::check` → 落 `reports/archi/<timestamp>.json` → 派生 `passed` / `diagnostics` / `report_dir` / `report_ref`；`fault` 除取消外恒 `None`；写盘失败降级 `None` |
| `crates/infra/archi/src/report.rs` | 落盘单点 | 报告文件名（`<timestamp>.json`）与目录创建；经 `foundation::layout` 派生目录，零行内路径字面量 |
| `src-tauri/src/commands/change_flow/mod.rs` | 装配 | `LocalToolSteps::new` 增第四 runner `Arc::new(LocalArchiCheck::new())`；装配层零判定 |
| `src-tauri/Cargo.toml` + workspace 清单 | 依赖登记 | `[workspace.dependencies]` 增 `archi-runtime = { path = "crates/infra/archi" }`；壳 `[dependencies]` 增 `archi-runtime`（带用途注释）；`members` 增 `crates/infra/archi` |
