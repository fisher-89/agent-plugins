# desktop-change-queries Specification

## Purpose

定义 workflow crate 查询层的聚合契约：change 列表为 db 记录 ∪ 磁盘目录的去重并集（状态面自 workspace 库、存量 CLI change 文档形态入列、按月分组），单 change 详情聚合（9 站流水线、PhaseRecord attempt 序列、active_phase、backtrack、会话槽位），查询层保持纯读且无指令概念。

## Requirements

### Requirement: change 列表扫描与分组

workflow crate 的 queries SHALL 返回全量 change 列表，条目集合 SHALL 为 **db 记录 ∪ 磁盘目录**的去重并集（发现语义）：workspace 库的 ChangeRecord 全量 + **主仓** changes / archive 两棵目录树内的目录，同名条目（db 记录与磁盘目录并存）SHALL 以 db 为准合并为一条。db 有记录的条目 SHALL 携状态面：名称、`status`（active | archived）、创建时间（`created_at`）、`active_phase` 运行中标注（可得时）——**状态归组以 db status 为权威**：带 worktree 记录的 active change 在 merge 前主仓两树未命中（目录位于其 worktree 内），SHALL 照常以 db status 归入进行中组，MUST NOT 因主仓目录缺席而被丢弃、MUST NOT 误归「未知时间」组，其工件 / 产物解析 SHALL 经记录的 worktree 路径（见 desktop-change-worktree）。db 缺记录的条目（存量 CLI change）SHALL 以文档形态入列（仅名称与产物存在性，无状态面）。归档 change SHALL 按月分组：db 记录以 `archived_at`（或归档目录名日期前缀，分组时间取值单点由 design 定稿）；磁盘侧无日期前缀的目录 SHALL 归入"未知时间"组而非被丢弃。缺失目录（如无归档树）SHALL 返回空结果而非报错。inventory 代际标注（v0 / v1 / v2）SHALL 退役，MUST NOT 出现在列表 DTO。

#### Scenario: db 建档条目状态面呈现

- **WHEN** 对含已建档 change（active 与 archived 各若干）的 workspace 调用列表查询
- **THEN** 各条目携 status / created_at，归档条目按月分组可达，active 运行中条目标注 active_phase

#### Scenario: worktree change 以 db status 归组

- **WHEN** 某 change 带 worktree 记录（status=active）且主仓 active / archive 两树均无其目录（merge 前）时调用列表查询
- **THEN** 该条目以 db status 归入进行中组且状态面完整呈现，不报错、不丢弃、不误归未知时间组

#### Scenario: 磁盘目录无 db 记录文档形态入列

- **WHEN** 主仓 changes 目录树存在仅有 workflow.json 与 markdown 产物的存量目录（无 db 记录）
- **THEN** 该目录以文档形态入列（无状态面字段），清单不报错、不自动建档

#### Scenario: 同名共存以 db 为准

- **WHEN** db 记录与主仓磁盘目录同名的 change 被列表查询命中
- **THEN** 输出恰一条且状态面取自 db，不产生重复条目

#### Scenario: 按月分组与未知时间兜底

- **WHEN** 归档目录中既有 `2026-05-08-xxx` 风格目录名、也有无日期前缀目录名
- **THEN** 有前缀者按月份分组，无前缀者进入"未知时间"组，无一被丢弃

#### Scenario: 缺失目录树降级

- **WHEN** workspace 无归档目录
- **THEN** 归档列表为空，查询不报错

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

### Requirement: 详情暴露 attempt 会话槽位

change 详情聚合 SHALL 在 attempt 记录（PhaseRecord 的线面投影）中暴露记录在案的会话槽位：executor / evaluator / decision 三槽位的会话 id，自 PhaseRecord 三槽位列直读透出（与 `SessionRecord` 同库，join 为库内一等查询）。槽位列 None 的记录（缺省落账）SHALL 以 null 呈现三值，MUST NOT 报错、MUST NOT 以 sourceRef 反查等派生手段虚构槽位值。DTO SHALL 沿既定口径（纯 derive、零字段属性、时间出线 ISO 串 + null），经 bindings 再生成同步前端类型。查询层 SHALL 保持纯读：槽位为直读透出，MUST NOT 在查询层写入或回填 db。

#### Scenario: 含槽位记录透出会话 id

- **WHEN** 查询某 PhaseRecord 携带 executor / evaluator / decision 会话槽位的 change 详情
- **THEN** 对应 attempt 记录的三个会话 id 与库内记录值逐字一致，无派生改写

#### Scenario: 缺槽位记录降级 null

- **WHEN** 查询槽位列为 None 的 PhaseRecord（缺省落账）
- **THEN** 全部 attempt 记录的三槽位均为 null，聚合不报错、详情其余区块照常

#### Scenario: bindings 同步

- **WHEN** DTO 演进后执行 bindings 重导出
- **THEN** 前端生成物含槽位与线面字段最新形状，一致性守卫（git diff --exit-code）绿

### Requirement: 查询层纯读且无指令概念

queries SHALL 为纯读操作：只读 workspace 文件与 workspace 库（经只读查询面），MUST NOT 写入、移动或修改任何文件，MUST NOT 产生 db 写事务。域 crate（workflow）MUST NOT 出现指令 / 命令概念（exec 属 desktop-app 层的预留轨道）；域 crate MUST NOT 依赖 Tauri。

#### Scenario: 只读保证

- **WHEN** 连续执行列表与详情查询
- **THEN** workspace 目录树的文件内容与结构、workspace 库的记录状态均无任何变化

#### Scenario: 域 crate 无 Tauri 依赖

- **WHEN** 检查 `workflow` crate 的依赖
- **THEN** 无 Tauri 相关依赖，queries 可独立于桌面壳被测试复用

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `workflow::queries` | db 读 + 磁盘产物发现的聚合层 | 列表（db ∪ 主仓磁盘去重并集、db status 权威归组——worktree 条目不因主仓目录缺席丢失）；详情（PhaseRecord / ChecklistItemRecord 重组、active_phase、backtrack、worktree 直读出线 None → null）；纯读 |
| `workflow::queries::locate_change` | change 目录解析单点 | 主仓 active 树精确名 → archive 树精确名 → archive 树日期前缀后缀匹配 → **worktree 路径回退（记录带 worktree 时）**；名称穿越校验不变 |
| `crates/infra/store`（workspace 库实例，经 port 缝消费） | 状态读源 | ChangeRecord / PhaseRecord / ChecklistItemRecord 只读查询；时间转换收 queries 层单点（i64 millis → ISO 串 + null） |
| `foundation::layout` | 路径输入 | 产物发现仅经 `Layout` 取目录，不自行拼磁盘路径 |
| `workflow::artifacts` | 产物发现协作 | 详情返回产物清单，发现机制复用 matcher 注册表；两类 change 同等可用 |
| `crates/core/workflow/src/queries/detail.rs` | 线面收敛 | `inventory` / `fileLog` / `unparsable` 字段删；槽位三列直读透出（None → null）；纯 derive 零字段属性口径不变 |
| `crates/core/workflow/tests/golden/` + `corpus_golden_test.rs` | wire contract 守卫 | `worktree` 字段演进走 golden 显式重写流程（`DESKTOP_GOLDEN_REWRITE=1` + diff 人工确认留痕） |
| `packages/desktop/src/types/generated/bindings.ts` | 类型跟随 | 经 export-bindings 再生成；一致性守卫拦截漂移 |
