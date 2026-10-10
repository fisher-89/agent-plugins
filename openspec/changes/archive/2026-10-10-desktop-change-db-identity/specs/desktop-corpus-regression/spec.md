# desktop-corpus-regression Specification (Delta)

## MODIFIED Requirements

### Requirement: 三代代表性 fixture 语料

workflow crate SHALL 在 `tests/fixtures/` 维护语料夹具，载体 SHALL 为 **db 种子夹具**（经 store change 域操作面写入 `ChangeRecord` / `PhaseRecord` / `ChecklistItemRecord` / `StepRecord` / `RunRecord` / `RunStepRecord` 的种子构造器——种子携带铸出 id 归键，`ChangeRecord` 主键 id、name 为属性）+ 磁盘产物树（markdown 产物；含归档目录 `YYYY-MM-DD-<name>` 前缀样本与 worktree 目录样本）。语料 SHALL 覆盖：完整执行史（多 attempt、fail→retry、backtrack 与 stale 标记、skipped 条目）、会话槽位全/缺两态、坏行样本（解不出为合法记录的种子，用于显式错误路径）、**归档 change 样本**（db 记录 name 恒裸名 + 磁盘目录带日期前缀 + status=archived，钉死 id 寻址下全状态面出线——原错配修复锚点）、以及 worktree 维度两态——带 `worktree` / `base_commit` 的建档样本与 `worktree=None` legacy 样本各至少一个（覆盖 detail 线面 `worktree` 字段 null / 非 null 两投影，见 desktop-change-queries / desktop-change-worktree）。原「db 缺记录文档形态」样本 SHALL 随文档形态能力退役改造（见下）；原 workflow.json 语料（v1-b / v1-c / v2-a 等三代 fixtures 与损坏样本）随 parse 退役失去解析主体，处置形态（删除 vs 转标 legacy 子目录保留）由 design 定稿，MUST NOT 静默遗留在默认语料路径参与回归。

#### Scenario: 语料覆盖状态面形态

- **WHEN** 检查 `tests/fixtures/` 语料清单与种子构造器
- **THEN** 含完整执行史（含 backtrack / stale / skipped）、槽位全缺两态、归档前缀样本与坏行样本各至少一个，含 worktree 两态建档样本（带 `worktree` / `base_commit` 与 `worktree=None`）各至少一个，全部经 db 种子构造、id 归键

#### Scenario: 磁盘目录零发现断言（文档形态退役改造）

- **WHEN** 语料在磁盘保留 markdown 产物目录（无 db 记录，原文档形态样本）后运行列表聚合
- **THEN** list golden 零呈现该条目（db 单源）；detail 以任意 name / 前缀目录名寻址不可达（恒「未找到」），磁盘目录字节零变化——原样本改作零发现反例断言常驻

#### Scenario: 归档前缀样本全状态面入 golden

- **WHEN** 对归档样本（db name 裸名 + 磁盘目录 `YYYY-MM-DD-<name>` 前缀）运行列表与详情聚合
- **THEN** list golden 该条目 name 为裸名、status=archived、月分组取自 `archived_at`；detail golden status=archived、pipeline 与 runs 全量出线（MUST NOT 为空流水线文档形态）；差异经 `DESKTOP_GOLDEN_REWRITE=1` 显式重写并人工确认留痕

#### Scenario: worktree 两态投影入 golden

- **WHEN** 对含 worktree 两态样本的语料运行全量聚合快照回归
- **THEN** detail 线面 `worktree` 出线值与库内记录逐字一致（非 null 样本出绝对路径、None 样本出 null），golden 差异经 `DESKTOP_GOLDEN_REWRITE=1` 显式重写并人工确认留痕，非静默接受

#### Scenario: 历史夹具处置显式

- **WHEN** 检查 parse 退役后原 workflow.json fixtures 的去留
- **THEN** 按 design 定稿处置（删除或转标 legacy 子目录），默认语料路径无 workflow.json 解析残留，处置结论留痕

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/tests/fixtures/`（种子构造器 id 化） | db 种子语料 | 种子构造器返回 / 携铸出 id（`seed_record` / `create` / `run_phase` / `open_only` / `run_start` / `run_finish` 等按 id 归键）；确定性 id 注入形态（固定 uuid 种子）由 design 定稿 |
| `crates/core/workflow/tests/golden/` | 快照对账 | list golden 改 db 单源（`corpus-list-mixed` 去磁盘-only 条目、`corpus-document-form` 改造为零发现反例）；list / detail 增 `id` 投影；`DESKTOP_GOLDEN_REWRITE=1` 显式再生成 → diff 人工确认留痕 → 复核一致 |
| `crates/core/workflow/tests/corpus_golden_test.rs` | 回归防线 | 归档前缀样本入列；文档形态样本断言改零发现；显式行为约束不豁免 |
