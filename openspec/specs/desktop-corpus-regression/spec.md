# desktop-corpus-regression Specification

## Purpose

定义 workflow crate 的语料回归防线：db 种子夹具（经 store change 域操作面构造 change 状态四模型）+ 磁盘产物树入仓管理，以全量聚合 + golden 快照对比的回归测试捕获 db 聚合与 detail 线面投影的静默劣化。

## Requirements

### Requirement: 三代代表性 fixture 语料

workflow crate SHALL 在 `tests/fixtures/` 维护语料夹具，载体 SHALL 为 **db 种子夹具**（经 store change 域操作面写入 `ChangeRecord` / `PhaseRecord` / `ChecklistItemRecord` / `StepRecord` / `RunRecord` / `RunNodeRecord` / `RunEdgeRecord` 的种子构造器——种子携带铸出 id 归键）+ 磁盘产物树。语料 SHALL 覆盖：完整执行史（多 attempt、fail→retry、backtrack 与 stale 标记、skipped 条目）、会话槽位全/缺两态、坏行样本、**归档 change 样本**（db 记录 name 恒裸名 + 磁盘目录带日期前缀 + status=archived + **archive 列空 checklist PhaseRecord 与 10 步节点样本**）、worktree 维度两态、title 维度两态。原 workflow.json 语料随 parse 退役失去解析主体，处置形态由 design 定稿，MUST NOT 静默遗留在默认语料路径参与回归。

#### Scenario: 语料覆盖状态面形态

- **WHEN** 检查 `tests/fixtures/` 语料清单与种子构造器
- **THEN** 含完整执行史（含 backtrack / stale / skipped）、槽位全缺两态、归档前缀样本（含 archive 列空 checklist 与 10 步节点）、坏行样本、worktree 两态建档样本、title 两态样本各至少一个，全部经 db 种子构造、id 归键

#### Scenario: 归档样本全状态面入 golden

- **WHEN** 对归档样本（db name 裸名 + 磁盘目录 `YYYY-MM-DD-<name>` 前缀 + archive 列在案）运行列表与详情聚合
- **THEN** list golden 该条目 name 为裸名、status=archived、月分组取自 `archived_at`；detail golden status=archived、10 站 pipeline 与 runs 全量出线（archive 列 PhaseRecord 与 step 节点在案）；差异经 `DESKTOP_GOLDEN_REWRITE=1` 显式重写并人工确认留痕

#### Scenario: 磁盘目录零发现断言（文档形态退役改造）

- **WHEN** 语料在磁盘保留 markdown 产物目录（无 db 记录，原文档形态样本）后运行列表聚合
- **THEN** list golden 零呈现该条目（db 单源）；detail 以任意 name / 前缀目录名寻址不可达（恒「未找到」），磁盘目录字节零变化——原样本改作零发现反例断言常驻

#### Scenario: worktree 两态投影入 golden

- **WHEN** 对含 worktree 两态样本的语料运行全量聚合快照回归
- **THEN** detail 线面 `worktree` 出线值与库内记录逐字一致（非 null 样本出绝对路径、None 样本出 null），golden 差异经 `DESKTOP_GOLDEN_REWRITE=1` 显式重写并人工确认留痕，非静默接受

#### Scenario: title 两态投影入 golden

- **WHEN** 对含 title 两态样本的语料运行全量聚合快照回归
- **THEN** list / detail 线面 `title` 出线与库内记录逐字一致（title=name 缺省样本出 name、显式样本出人类标题），golden 差异经 `DESKTOP_GOLDEN_REWRITE=1` 显式重写并人工确认留痕，非静默接受

#### Scenario: 历史夹具处置显式

- **WHEN** 检查 parse 退役后原 workflow.json fixtures 的去留
- **THEN** 按 design 定稿处置（删除或转标 legacy 子目录），默认语料路径无 workflow.json 解析残留，处置结论留痕

### Requirement: 全量聚合快照回归

workflow crate SHALL 提供**全量聚合 + golden 快照对比**的回归测试：对 db 种子语料逐样本执行建档 → queries 聚合（列表 + 详情），输出序列化结果与 golden 文件比对。该测试 SHALL 防止 db 聚合与 detail 线面投影的静默劣化（原「TS schema 演进导致 Rust 解析静默劣化」防线随 parse 退役由本防线接管聚合面）；golden 更新 SHALL 是显式行为（`DESKTOP_GOLDEN_REWRITE=1` 覆写 → 人工确认 diff 预期范围并留痕 → 复核一致），MUST NOT 静默自动重写。

#### Scenario: 快照回归通过

- **WHEN** 运行快照回归测试
- **THEN** 全部 db 种子样本的聚合输出与 golden 一致

#### Scenario: 聚合漂移被捕获

- **WHEN** 某聚合或线面投影形状变化导致输出改变
- **THEN** 回归测试失败并呈现差异，需显式确认后才能更新 golden

### Requirement: run 运行史语料维度

workflow crate 的 db 种子语料 SHALL 覆盖 run 执行图维度（RunRecord / RunNodeRecord / RunEdgeRecord）：同 change 多 run 全史样本（≥2 run，attempt 跨 run 递增不撞号）、interrupted 标定样本、**节点全词汇样本**（二十二类步词汇在场，含 phase_start / phase_log / 三门 gate、mutation_test、archi_check、initialize 基线步 clean 与 9 个归档步（含 cleanup））、**initialize 相位样本**（5 步节点（phase_start / clean / static_check / test_execution / phase_log）+ 机械 checklist PhaseRecord + 串行 forward 边，含 baseline 槽位报告树工件样本）、**archive 相位样本**（11 步节点 + 空 checklist PhaseRecord + 串行 forward 边）、**feedback 边样本**、**边 kind 四值样本**（forward / feedback / retry / backtrack 各至少一条）、`session_id` / `detail` / `output_ref` 有无两态。聚合 golden SHALL 覆盖 detail 线面 `runs` / `nodes` / `edges` 投影（全史出线、稳定身份键、二十二值全词汇、边 kind 四值、时间戳 ISO 串口径）；golden 更新 SHALL 沿显式重写流程。种子构造 SHALL 经 store run 域操作面写入，MUST NOT 以裸表插桩绕过操作面。

#### Scenario: run 维度种子在案

- **WHEN** 检查 `tests/fixtures/` 种子构造器与语料清单
- **THEN** 含多 run 全史、interrupted 标定、节点二十二值全词汇（含 `archi_check`）、initialize 相位、archive 相位、feedback 边、边 kind 四值、sessionId / detail / outputRef 两态样本各至少一个，全部经 store run 域操作面构造

#### Scenario: golden 覆盖 nodes / edges 投影

- **WHEN** 对含 run 维度样本的语料运行全量聚合快照回归
- **THEN** detail 线面 `runs` / `nodes` / `edges` 出线与库内记录逐字一致（全史不截、稳定身份键、边 kind 四值、零 seq / 零过滤），golden 差异经 `DESKTOP_GOLDEN_REWRITE=1` 显式重写并人工确认留痕

#### Scenario: 全词汇与 archive 断言防漂移

- **WHEN** 语料样本经含流程面步骤（phase_start / phase_log / 三门）、mutation_test、archi_check、initialize 基线步（clean / static_check / test_execution）、9 个归档步（含 cleanup）与 feedback 重跑的构造序列落库后快照
- **THEN** run_nodes 投影二十二类词汇全在案（零过滤）、initialize 相位 5 步节点以 forward 边串行、archive 相位 11 步节点以 forward 边串行、feedback 重跑为 `instanceNo` 递增独立节点、run_edges 投影含 feedback 边；断言随快照回归常驻

### Requirement: 步级时间与 token 语料维度

run 运行史 db 种子语料 SHALL 覆盖步级时间与 token 维度（承接「run 运行史语料维度」，样本字段随 `ChangeStepState` 四字段扩展）：种子构造器构造 `ChangeStepState` 字面量时 SHALL 以**定值注入** `started_at` / `ended_at` / `token_input` / `token_output`（零 wall-clock 进 corpus——emit 生产时钟不进种子路径，沿「corpus 确定性由测试注入承载」口径）；样本 SHALL 含**时间有无两态**（正常起止双值步 / 瞬步 `started_at == ended_at` / running 收口在途 `ended_at=None`）与**token 有无两态**（WorkerAgent 步带 token 值 / 工具步 None）各至少一个。聚合 golden SHALL 覆盖 detail 线面 nodes 四字段投影（毫秒数口径，见 desktop-change-queries delta）；golden 更新 SHALL 沿显式重写流程（`DESKTOP_GOLDEN_REWRITE=1` → 人工确认 diff 预期范围留痕 → 复核一致），MUST NOT 静默自动重写。种子构造 SHALL 经 store run 域操作面写入（`finish_command` 组装路径），MUST NOT 以裸表插桩绕过操作面。

#### Scenario: 种子定值与维度两态在案

- **WHEN** 检查 `tests/fixtures/` 种子构造器对 `ChangeStepState` 的构造面
- **THEN** 四字段均以定值字面量注入（零 wall-clock 调用）；正常双值步、瞬步同刻、running 在途（ended None）、token 有值与 None 两态样本各至少一个，全部经 store run 域操作面落库

#### Scenario: golden 覆盖四字段投影

- **WHEN** 对含步级时间与 token 维度样本的语料运行全量聚合快照回归
- **THEN** detail 线面 `runs[].nodes[]` 四字段出线与库内记录逐字一致（毫秒数、None → null），golden 差异经 `DESKTOP_GOLDEN_REWRITE=1` 显式重写并人工确认留痕

#### Scenario: 确定性回归可复现

- **WHEN** 连续两次运行快照回归测试
- **THEN** 输出与 golden 全等（定值注入下零时间漂移），无 flaky 时间断言

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/tests/fixtures/`（种子构造器 id 化） | db 种子语料 | 种子构造器返回 / 携铸出 id（`seed_record` / `create` / `run_phase` / `open_only` / `run_start` / `run_finish` 等按 id 归键）；确定性 id 注入形态（固定 uuid 种子）由 design 定稿；覆盖执行史形态 + 归档前缀样本 + 坏行；历史 workflow.json 夹具删除或转标 legacy（design 定稿） |
| `crates/core/workflow/tests/fixtures/`（种子构造器扩展） | db 种子语料 worktree 维度 | worktree 两态建档样本（带 `worktree` / `base_commit` 与 legacy `None`）；聚合 golden 覆盖 detail `worktree` null / 非 null 两投影（字段语义见 desktop-change-state-store） |
| `crates/core/workflow/tests/golden/` | 快照对账 | list golden 改 db 单源（`corpus-list-mixed` 去磁盘-only 条目、`corpus-document-form` 改造为零发现反例）；list / detail 增 `id` 投影；`DESKTOP_GOLDEN_REWRITE=1` 显式再生成 → diff 人工确认留痕 → 复核一致 |
| `crates/core/workflow/tests/corpus_golden_test.rs` | 回归防线 | 归档前缀样本入列；文档形态样本断言改零发现；db 种子 → 聚合 → golden 比对；显式行为约束不豁免 |
| `crates/core/workflow/tests/fixtures/`（run 维度种子） | 词汇与 archive 样本 | cleanup 步词节点入全词汇样本；archive 相位样本 11 步节点（SpecSync → Cleanup → Seal 序）+ 串行 forward 边；经 store run 域操作面构造 |
| `crates/core/workflow/tests/corpus_golden_test.rs` | golden 随动 | 二十一值 nodes 投影断言 + initialize 相位样本断言；diff 经 `DESKTOP_GOLDEN_REWRITE=1` 显式重写留痕；既有 archive 断言不回归 |
| `crates/core/workflow/tests/fixtures/`（种子构造器扩展） | 步级维度样本 | `ChangeStepState` 字面量补四字段（定值注入）；时间两态（双值 / 瞬步同刻 / ended None）与 token 两态样本；经 store run 域操作面构造 |
| `crates/core/workflow/tests/corpus_golden_test.rs` + `tests/golden/` | golden 随动 | detail nodes 四字段投影入 golden；diff 经 `DESKTOP_GOLDEN_REWRITE=1` 显式重写留痕；既有二十值词汇与 archive 断言不回归 |
| `crates/core/workflow/tests/golden/` | golden 快照 | initialize 相位 / clean 步词 / baseline 槽位样本入快照；重写留痕人工确认 |
| `crates/core/workflow/tests/corpus_golden_test.rs` + 种子构造器 | 语料维度随动 | 全词汇样本补 `archi_check` 节点（implement 相位，feedback 边样本可复用既有边）；golden 显式重写留痕 |
| `tests/fixtures/`（db 种子） | 构造面不变 | 种子仍全经 store run 域操作面写入，MUST NOT 裸表插桩 |
