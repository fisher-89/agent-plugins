# desktop-change-queries Specification (Delta)

## MODIFIED Requirements

### Requirement: change 详情聚合

queries SHALL 聚合单个 change 的详情，状态面单源自 workspace 库：9 站流水线（proposal / dev-design / test-design / implement / test-gen / test-execution / code-review / acceptance / code-analyze）逐站展开 PhaseRecord 的 attempt 序列（attempt 序号、verdict、report、checklist 的 item / pass / evidence、timestamp），并暴露 `active_phase`（自 ChangeRecord）与 backtrack 字段（backtrack_to / backtrack_reason）；checklist 自 ChecklistItemRecord 子表按打包键序重组，线面聚合形状不变（存储级拆分，出线不动）。详情 SHALL 出线 `worktree` 路径（自 ChangeRecord.worktree 直读透出；None → null，纯 derive 零派生改写口径），作为用户 review / 手动 commit / merge 的可达锚（见 desktop-change-worktree「worktree 路径 UI 可见性」）。产物发现与信封读取的路径解析 SHALL 感知 worktree：带 worktree 记录的 change，其 `locate_change` SHALL 在主仓 active / archive 两树未命中时回退到记录的 worktree 路径（目录解析单点收口）；`worktree=None` 的 change SHALL 维持主仓两树解析既有语义。db 缺记录的 change SHALL 返回空流水线与产物文档清单（文档形态，主仓两树解析），MUST NOT 报错、MUST NOT 读取其 workflow.json。fileLog 区块与 unparsable 状态 SHALL 保持退役；`worktree` 字段为 detail 线面字段演进，SHALL 走 golden 显式重写流程（wire contract 冻结记忆，`DESKTOP_GOLDEN_REWRITE=1` + diff 人工确认留痕）。

详情 SHALL 同时出线 **run 运行史全量读面**：`runs` 数组（自 RunRecord 全史投影，不截 last_run：runId / status / reason / 起止时刻）与每 run 的 `steps`（自 RunStepRecord 投影：phase / attempt / step / status / seq / sessionId / detail），供前端步节点常驻渲染与运行史审计消费（见 desktop-change-state-store「run 运行史落库」、desktop-change-flow-view）。时间戳 SHALL 维持既有口径（db i64 毫秒落库、出线 ISO 串 + null，转换收 queries 层单点）；DTO SHALL 纯 derive 零字段属性，runs / steps 字段演进 SHALL 走 golden 显式重写流程（`DESKTOP_GOLDEN_REWRITE=1` + diff 人工确认留痕），bindings 再生成同步前端类型。详情标示「运行中」SHALL 与实际在飞 run 一致：active_phase 悬 hanging 杀除后（desktop-change-state-store「启动标定与 active_phase 悬挂处置」），MUST NOT 出现无运行 run 而呈运行中的假象；在飞 run 的并入见 desktop-change-orchestration「运行控制命令面」读路统一。

详情 SHALL 同时给出该 change 目录内可读产物的清单（经 artifact 插件 matcher 发现，见 `desktop-artifact-plugins`），供前端按信封逐个请求；产物发现 SHALL 对 db 有记录与 db 缺记录两类 change 同等可用。

#### Scenario: 流水线含重试与回跳轨迹

- **WHEN** 某 change 的 PhaseRecord 含同 phase 多次 attempt（fail→retry）与 backtrack_to 记录
- **THEN** 详情按 phase 分组展示全部 attempt，且 backtrack 目标与原因可查

#### Scenario: worktree 路径出线与产物经 worktree 解析

- **WHEN** 查询带 worktree 记录的 change 详情并逐个读取其产物信封
- **THEN** detail DTO 的 `worktree` 与库内记录值逐字一致；产物清单与信封读取命中 worktree 内目录（主仓两树未命中不致 detail / read_artifact 落空）

#### Scenario: legacy 与 null 出线

- **WHEN** 查询 `worktree=None` 的 change 详情
- **THEN** `worktree` 出线 null、产物解析走主仓两树既有语义，聚合不报错

#### Scenario: db 缺记录文档形态详情

- **WHEN** 查询无 db 记录的存量 change 详情
- **THEN** 返回空流水线 + 产物文档清单，workflow.json 零读取，不报错

#### Scenario: 运行中状态可见且不撒谎

- **WHEN** 某 change 的 ChangeRecord.`active_phase` 非空（phase 正在运行）
- **THEN** 详情标示该 phase 为运行中（含 attempt 序号与 start_at）；run 死亡后（含中途停止与重启标定）`active_phase` 已处置，详情 MUST NOT 呈运行中假象

#### Scenario: run 运行史全量出线

- **WHEN** 某 change 历经两次 run（第二次自续走推进）后查询详情
- **THEN** `runs` 出线两条全史记录（各自 status / reason / 起止），每 run `steps` 按 emit seq 稳定序出线（恰五词汇，流程面步骤零行），attempt 跨 run 递增不撞号

#### Scenario: golden 显式重写留痕

- **WHEN** detail DTO 增 `runs` / `steps` 字段后运行语料回归测试
- **THEN** diff 经 `DESKTOP_GOLDEN_REWRITE=1` 显式再生成并人工确认预期范围留痕，非静默重写；bindings 一致性守卫绿

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/src/queries/detail.rs` | 详情线面收敛（run 读史扩展） | `runs` / `steps` 全史投影（RunRecord / RunStepRecord 直读，纯 derive）；时间口径 i64 毫秒落库 → ISO 串 + null 出线单点不变；步词汇过滤与落库同一单点消费（desktop-change-state-store） |
| `crates/core/workflow/tests/golden/` + `corpus_golden_test.rs` | wire contract 守卫 | runs / steps 字段演进走 golden 显式重写（`DESKTOP_GOLDEN_REWRITE=1` + 人工确认留痕）；run 维度种子见 desktop-corpus-regression |
| `packages/desktop/src/types/generated/bindings.ts` | 类型跟随 | 经 export-bindings 再生成；一致性守卫拦截漂移 |
