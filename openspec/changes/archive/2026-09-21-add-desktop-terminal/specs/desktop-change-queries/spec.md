# desktop-change-queries Specification

## ADDED Requirements

### Requirement: change 列表扫描与分组

workflow crate 的 queries SHALL 基于 `Layout` 扫描进行中与归档两个目录树，返回全量 change 列表，每条至少含：名称、代际标注（v2 / v1 / v0）、创建时间（可得时）。归档 change SHALL 按月分组：日期优先取 archive 目录名的日期前缀（`YYYY-MM-DD-<name>`）；无日期前缀的 SHALL 归入"未知时间"组而非被丢弃。缺失目录（如无归档树）SHALL 返回空结果而非报错。

#### Scenario: 全量列表含代际标注

- **WHEN** 对含进行中与归档 change 的 workspace 调用列表查询
- **THEN** 返回进行中 + 归档全量条目，每条带 v2 / v1 / v0 代际标注

#### Scenario: 按月分组与未知时间兜底

- **WHEN** 归档目录中既有 `2026-05-08-xxx` 风格目录名、也有无日期前缀目录名
- **THEN** 有前缀者按月份分组，无前缀者进入"未知时间"组，无一被丢弃

#### Scenario: 缺失目录树降级

- **WHEN** workspace 无归档目录
- **THEN** 归档列表为空，查询不报错

### Requirement: change 详情聚合

queries SHALL 聚合单个 change 的详情：9 站流水线（proposal / dev-design / test-design / implement / test-gen / test-execution / code-review / acceptance / code-analyze）逐站展开 `eval[]` 中的 attempt 序列（attempt 序号、verdict、report、checklist 的 item / pass / evidence、timestamp），并暴露 `active_phase`（运行中状态）与 backtrack 字段（backtrack_to / backtrack_reason）。v1 change SHALL 返回 eval 历史但无 file_log 相关区块；v0 change SHALL 返回空流水线与产物文档清单。

详情 SHALL 同时给出该 change 目录内可读产物的清单（经 artifact 插件 matcher 发现，见 `desktop-artifact-plugins`），供前端按信封逐个请求。

#### Scenario: 流水线含重试与回跳轨迹

- **WHEN** 某 change 的 `eval` 含同 phase 多次 attempt（fail→retry）与 backtrack_to 条目
- **THEN** 详情按 phase 分组展示全部 attempt，且 backtrack 目标与原因可查

#### Scenario: 三代降级详情

- **WHEN** 分别查询 v2 / v1 / v0 的 change 详情
- **THEN** v2 返回完整流水线与运行状态；v1 返回 eval 历史但无 file_log 区块；v0 返回空流水线加文档清单，均不报错

#### Scenario: 运行中状态可见

- **WHEN** 某 change 的 `active_phase` 非空（phase 正在运行）
- **THEN** 详情标示该 phase 为运行中（含 attempt 序号与 start_at）

### Requirement: 查询层纯读且无指令概念

queries SHALL 为纯读操作：只扫描与读取 workspace 文件，MUST NOT 写入、移动或修改任何文件。域 crate（workflow）MUST NOT 出现指令 / 命令概念（exec 属 desktop-app 层的预留轨道）；域 crate MUST NOT 依赖 Tauri。

#### Scenario: 只读保证

- **WHEN** 连续执行列表与详情查询
- **THEN** workspace 目录树的文件内容与结构无任何变化

#### Scenario: 域 crate 无 Tauri 依赖

- **WHEN** 检查 `workflow` crate 的依赖
- **THEN** 无 Tauri 相关依赖，queries 可独立于桌面壳被测试复用

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `workflow::queries` | 扫描与聚合 | 列表查询（active + archive、代际、按月分组）；详情查询（流水线 / attempt / verdict / checklist / active_phase / backtrack）；纯读 |
| `foundation::layout` | 路径输入 | queries 仅经 `Layout` 取目录，不自行拼磁盘路径 |
| `workflow::artifacts` | 产物发现协作 | 详情返回产物清单，发现机制复用 matcher 注册表 |
