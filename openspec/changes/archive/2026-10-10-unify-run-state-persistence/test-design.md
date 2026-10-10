# 测试设计: unify-run-state-persistence

> **日期**: 2026-10-09
> **依据**: proposal.md（AC-1–AC-11）+ design.md（D1–D13、数据模型、读写时序定形）

---

## 测试框架与路径约定（框架识别结论）

- **Rust（src-tauri 三 crate + shell 命令层）**：cargo 内建测试框架，测试文件与源文件同目录同名的 `*_test.rs` 模块（既有惯例，如 `write/phase_log_test.rs`、`control_test.rs`）；跨 crate 语料集成测试驻 `crates/core/workflow/tests/`（`corpus_golden_test.rs`）。断言面为标准 `assert_eq!` / `assert!` + `serde_json::json!` 线面比对。
- **前端（packages/desktop）**：vitest（vite-plus）+ @testing-library/react，测试文件与源文件同目录 `*.test.ts(x)`；`data-testid` 为唯一查询挂钩（spec 红线：MUST NOT 样式类名查询）；进程边界 mock 统一走 `vi.mock('@tauri-apps/api/core')`（invoke 分发 + 可编程 Channel 类，既有装置沿用）。
- **Mock 纪律**（沿仓库既有）：Rust 侧零框架级 mock——db 边界一律 tempfile 真件库真实组合，进程内假件仅限「作为被测 API 显式入参 / 注入依赖」的 trait 假件（入参例外）；前端仅 mock 进程边界（invoke / Channel / 计时器），内部模块（run-state 纯函数等）真实组合。
- 以下源文件路径均相对项目根；Rust 路径前缀 `packages/desktop/src-tauri/` 简写时以节标题全称为准。

---

## 验收范围

| AC ID | 验收条件 | 被测文件或模块 |
|--------|---------|----------|
| AC-1 | 两表与回环：run 走完多相位后重开库查询逐字段一致；存量库 additive 打开零迁移；native_model 版本治理在案 | packages/desktop/src-tauri/crates/infra/store/src/model_test.rs |
| AC-2 | 每 run 两写：假引擎驱动一相位 run 库写事务恰两次（start 建 running 行 / finish 终态更新 + 步整包单事务），无每步写事务；整包原子（任一环节失败整体回滚） | packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs |
| AC-3 | 步词汇口径一致：一次 run 覆盖全部 10 步词汇后查 run_steps 恰 5 落，流程面与三门零行；过滤词汇单点定义，walker / control 零过滤逻辑 | packages/desktop/src-tauri/crates/core/orchestration/src/run_history_test.rs |
| AC-4 | 启动标定：构造库中残留 running 行后重启该行标 interrupted，详情不再呈运行中；live run 写入路径永不产生 interrupted | packages/desktop/src-tauri/crates/infra/store/src/store_test.rs |
| AC-5 | 悬挂杀除与续走：run 中途停止后详情头部不呈「运行中 · phase · attempt」；重新发起自续走锚点推进，已 pass 相位不重头执行 | packages/desktop/src-tauri/crates/infra/store/src/store_test.rs |
| AC-6 | 出线与 golden：ChangeDetail 出线 runs + steps 全史；时间戳 ISO 串 + null 口径不变、纯 derive；diff 经显式重写流程人工确认留痕；bindings 一致性守卫绿 | packages/desktop/src-tauri/crates/core/workflow/src/queries/detail_test.rs |
| AC-7 | 合并查询统一视图：run 运行中调用 get_change_detail 一次返回库读史 ∪ 在飞 run（状态 / 停等与 ask 载荷 / RunEntry 步表）；前端无双命令拼接 | packages/desktop/src-tauri/src/commands/changes/mod_test.rs |
| AC-8 | 通知降位与前端解散：RunUpdate 以变更通知流出（无载荷）；applyRunUpdate / liveEvents / seq 去重删除（knip 无残留）；stop / confirm / answer 请求-应答不变 | packages/desktop/src/views/changes/hooks/use-change-flow-run.test.ts |
| AC-9 | 图常驻渲染：run 收口后刷新 / 重开详情步节点常驻不回落，与运行中同一转换函数派生；两次 run 全史叠加，键 `phase:attempt:step` 无冲突 | packages/desktop/src/views/changes/flow/run-state.test.ts |
| AC-10 | 重挂恢复与转录统一：run 运行中重挂视图步表非空且与 emit 序一致；抽屉实时转录经转录库重查呈现；重放路径行为不变 | packages/desktop/src/views/changes/change-detail-view.test.tsx |
| AC-11 | 语料与管线交付：语料种子含 run 维度样本（≥2 run 全史、interrupted、步词汇与 sessionId 两态），golden 覆盖 runs/steps 投影；管线全绿；desktop 0.4.28；dev-team 零改动 | packages/desktop/src-tauri/crates/core/workflow/tests/corpus_golden_test.rs |

> AC-2 的整包原子回滚断言、AC-5 的续走不重头断言分别由 `store_test.rs`（finish 单事务回滚）与 `walker_test.rs`（两轮 run 相位执行序列）交叉承载，见对应节用例行内 AC 标注；AC-8 的 Rust 侧（RunNotice 线面 / 命令面）由 `orchestration/src/state_test.rs` 与 `change_flow/mod_test.rs` 承载。

---

## 单元测试

### packages/desktop/src-tauri/crates/core/workflow/src/state.rs -> packages/desktop/src-tauri/crates/core/workflow/src/state_test.rs

#### 待测功能

- RunStatus(): run 库侧状态五值封闭集（serde lowercase：running / completed / stopped / failed / interrupted）
- RunStepKind(): 落库步词汇五值封闭集（serde snake_case：executor / evaluator / decision / static_check / test_execution）
- RunStepStatus(): 步状态四值（serde camelCase：running / passed / failed / stopped）
- RunStateRecord: run 运行史读记录（run_id / change / status / reason / started_at / finished_at）
- RunStepStateRecord: run 步史读记录（seq / run_id / phase / attempt / step / status / session_id / detail / timestamp）
- RunStartCommand: run 起始写命令（run_id / change / started_at）
- RunStepEntry / RunFinishCommand: run 收口写命令（终态三值约束 + 步整包）
- ChangeStateStore.list_runs() / list_run_steps() / run_start() / run_finish(): run 域 port 四方法（签名与语义契约面）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| run 中性类型线面词汇 | 正向 | RunStatus 五值序列化出线恰为 lowercase 词（`"running"`/`"completed"`/`"stopped"`/`"failed"`/`"interrupted"`，AC-1/AC-6 线面前置） | 新增 |
| run 中性类型线面词汇 | 正向 | RunStepKind 五值出线恰为 snake_case 词（`"executor"`/`"evaluator"`/`"decision"`/`"static_check"`/`"test_execution"`，AC-3 词汇单点前置） | 新增 |
| run 中性类型线面词汇 | 正向 | RunStepStatus 四值出线恰为 camelCase 词（`"running"`/`"passed"`/`"failed"`/`"stopped"`） | 新增 |
| run 中性类型线面词汇 | 异常 | 非法词反序列化拒绝：`"phase_start"` / `"verdictGate"` / `"WAITING"` 等越集词反序列化为各自枚举均 Err（封闭集防线，AC-3） | 新增 |
| run 命令与记录结构 | 边界 | RunStartCommand / RunFinishCommand / RunStateRecord / RunStepStateRecord 字段全集等值断言：reason / session_id / detail / finished_at 各 None 与 Some 两态构造、PartialEq 等值与不等值面成立 | 新增 |
| run 命令与记录结构 | 边界 | RunFinishCommand.status 携 Running / Interrupted 可构造（类型层不拦）——写面拦截断言挂 write/run_test.rs 节（交叉注记，AC-4） | 新增 |
| ChangeStateStore run 域四方法 | 正向 | trait 四方法签名编译面经实现承载：run_test.rs 假件与 store 真件各自实现后 list_runs / list_run_steps / run_start / run_finish 可调（本节不重复行为断言，注记指向对应节） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | 纯类型直测，零依赖零 mock；trait 行为由实现侧测试承载（write/run_test.rs 假件、infra/store 真件） | 本节全部 describe |

---

### packages/desktop/src-tauri/crates/core/workflow/src/write/run.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/run_test.rs

#### 待测功能

- run_start(): run 起始写面（run_id 非空 + change 建档在案校验 → 委派 `store.run_start` 落 running 行）
- run_finish(): run 收口写面（change 建档在案 + status 为终态三值且非 interrupted 校验 → 委派 `store.run_finish`）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| run 写面校验 | 正向 | run_start 对建档 change 的合法命令委派成功：真件 tempfile 库 `list_runs` 回读 running 行（status=running、finished_at=None、started_at 原值，AC-1/AC-2 第一写） | 新增 |
| run 写面校验 | 正向 | run_finish 以 completed / stopped / failed 三终态各自委派成功，回读终态与 reason / finished_at 原值（AC-1） | 新增 |
| run 写面校验 | 异常 | run_start run_id 空串 → Err 且零库写（假件捕获面断言未触 store） | 新增 |
| run 写面校验 | 异常 | run_start / run_finish 对未建档 change → Err（NotFound 语义记因，AC-1） | 新增 |
| run 写面校验 | 异常 | run_finish status=Running → Err；status=Interrupted → Err 且记因含「运行期写路径不产生 interrupted」（AC-4 live 写路径防线） | 新增 |
| run 写面校验 | 边界 | run_finish steps 空包（零步 run 收口形态）→ Ok 委派（AC-2 边界：整包含空集合法） | 新增 |
| run 写面校验-委派透传 | 边界 | 假件捕获：run_start / run_finish 收到的命令与入参逐字段等值（写面零改写透传，D2 词汇本体 = 命令类型本身） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| ChangeStateStore（入参例外） | 进程内假件实现 trait（Mutex 捕获 run_start / run_finish 命令、可编程 Err、越权读面 panic），作为被测函数显式入参注入 | 「委派透传」「run_id 空串零库写」「未建档 Err」 |
| db 文件（进程边界） | tempfile 真件 workspace 库（沿 walker_test TestDb 装置先例），真实 start/list 回环 | 「running 行回读」「三终态回读」 |

---

### packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs -> packages/desktop/src-tauri/crates/core/workflow/src/queries/detail_test.rs

#### 待测功能

- ChangeRunStepRecord / ChangeRunEntry: run 史线面 DTO（纯 derive 零字段属性、时间戳 ISO 串 + null）
- ChangeDetail.runs: run 运行史全量出线字段
- change_detail(): 详情聚合扩展（runs 按 started_at 升序、steps 按 seq 升序组装；文档形态恒空 runs）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 详情 run 史出线 | 正向 | 建档 change 含两 run（各自 steps 覆盖五词汇样本）→ runs 两条全史出线，runId / status / reason / startedAt / finishedAt 逐项投影、时间戳 ISO 串口径与既有字段一致（AC-6） | 新增 |
| 详情 run 史出线 | 正向 | 每 run steps 按 seq 升序出线；session_id / detail 两态（Some 透传、None → null）（AC-6/AC-11） | 新增 |
| 详情 run 史出线 | 正向 | runs 按 started_at 升序排列（AC-9 前端分层序的前置语义） | 新增 |
| 详情 run 史出线 | 正向 | 在飞 run 起始行出线：status=running、finishedAt=null、steps 恒空数组（D11——run 清单面运行中可见） | 新增 |
| 详情 run 史出线 | 边界 | 同 started_at 并列两 run → run_id 稳定并列序（排序全确定性） | 新增 |
| 详情 run 史出线 | 边界 | 文档形态（db 缺记录磁盘目录在场）→ runs 恒空数组、聚合不报错（AC-6） | 新增 |
| 详情 run 史出线 | 边界 | 终态 run reason=None（无记因收口）→ null 出线；running 行 finished_at=None → null（口径不变面） | 新增 |
| 详情 run 史出线 | 异常 | store.list_runs / list_run_steps 返回 Err → runs 降级空数组、详情其余面不受阻断（沿 list_phase_records unwrap_or_default 同口径） | 新增 |
| 详情 run 史出线（既有随动） | 边界 | 既有 ChangeDetail 断言基设随 runs 键演进（构造器补 runs 字段；golden 显式重写后线面钉死，见 corpus 节） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| ChangeStateStore（入参例外） | 既有 DetailStore 假件扩展 run 半边：list_runs / list_run_steps 可编程序列（含 Err 可编程），写半边 unimplemented panic（纯读面越权即暴露）；既有读半边行为不变 | 本节全部 describe |
| 磁盘目录树（进程边界） | 既有 Env tempdir 真实目录（产物文件 + 定位面） | 「文档形态恒空 runs」 |

---

### packages/desktop/src-tauri/crates/infra/store/src/model.rs -> packages/desktop/src-tauri/crates/infra/store/src/model_test.rs

#### 待测功能

- RunRecord: run 运行史主行（native_model id=13 v1；run_id 主键、change 二级索引、status 五值、reason / started_at / finished_at）
- RunStepRecord: run 步史行（native_model id=14 v1；step_key u128 打包主键、run_id 二级索引、五词汇封闭集、session_id / detail 有界）
- pack_run_step_key(): `(hash64(run_id) << 64) | seq` 打包键组装（session_key_hash 家族复用）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| run 两模型回环 | 正向 | RunRecord 写读回环逐字段一致：status 五值各一轮、reason / finished_at None 与 Some 两态（AC-1） | 新增 |
| run 两模型回环 | 正向 | RunStepRecord 写读回环逐字段一致：step 五词汇、status 四值、session_id / detail 两态（AC-1） | 新增 |
| run 两模型回环 | 正向 | change 二级索引圈定：两 change 各自 run 混写后按 change 圈定互不串台、run_id 主键全库唯一（AC-1） | 新增 |
| run 两模型回环 | 异常 | 同 run_id 二次插入 → StoreError 冲突面（主键唯一防线，AC-2 start 冲突的前置） | 新增 |
| 打包键形态 | 正向 | pack_run_step_key 数值序即分层序：同 run_id 下 seq 递增键严格递增、异 run_id 键域不相交（AC-9 全史叠加库面前置） | 新增 |
| 打包键形态 | 边界 | seq 空洞合法（seq=0,2,5 写入后全量枚举仍升序无缺行，D7） | 新增 |
| 打包键形态 | 边界 | step_key u128 经 event_key_serde 十六进制线面编码回环无损（SessionEventRecord 先例同式，AC-1） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | tempfile 真件 workspace 库（既有 store 装置），写读真实往返 | 本节全部 describe |

---

### packages/desktop/src-tauri/crates/infra/store/src/store.rs -> packages/desktop/src-tauri/crates/infra/store/src/store_test.rs

#### 待测功能

- start_change_run(): running 行插入（同 run_id 冲突 → StoreError::Conflict）
- finish_change_run(): 单事务收口（终态 + reason + finished_at + RunStepRecord 整包 + 该 change active_phase 清位；任一环节失败整体回滚）
- calibrate_interrupted_runs(): 启动标定（残留 running → interrupted 附记因 + 全量 active_phase 清位；幂等、零残留零写事务）
- list_change_runs() / list_run_steps(): run 史读面（started_at / seq 升序）
- open_workspace(): 打开尾内嵌启动标定（D12）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| run 域操作面-start | 正向 | start_change_run 建 running 行回读一致（status=running、finished_at=None、started_at 命令携带原值；时间戳命令携带零钟面，AC-1/AC-2 第一写） | 新增 |
| run 域操作面-start | 异常 | 同 run_id 二次 start_change_run → Conflict（AC-1） | 新增 |
| run 域操作面-finish | 正向 | finish_change_run 单事务整包：多步（五词汇混合、session_id/detail 两态）一次落、seq 保序、步 timestamp 全包等于 finished_at 同刻（corpus 确定性口径，AC-1/AC-2/AC-10） | 新增 |
| run 域操作面-finish | 正向 | finish 同事务 active_phase 清位：预置 active_phase=Some 的 change 收口后读回 None（D1 悬挂杀除库面，AC-5） | 新增 |
| run 域操作面-finish | 正向 | 多 run 全史：同 change 两 run 先后 start/finish → list_change_runs 两行全史、同相位 attempt 跨 run max+1 递增不撞号（AC-9/AC-11 样本语义） | 新增 |
| run 域操作面-finish | 异常 | finish 目标 run 不在案 → NotFound；run 已终态再 finish → Err（非 running 不可再收口） | 新增 |
| run 域操作面-finish | 异常 | 步整包撞键注入：finish 命令 steps 内两条同 seq（打包主键冲突）→ 整事务回滚——run 行仍 running、RunStepRecord 零行、active_phase 原值保留（AC-2 整包原子回滚） | 新增 |
| 启动标定 | 正向 | 残留 running 行 + 该 change active_phase=Some → calibrate 后行翻 interrupted（finished_at=标定时刻、reason 含「重启标定」与「中断于 phase X attempt N」语境）+ active_phase 清位（AC-4/AC-5） | 新增 |
| 启动标定 | 正向 | 残留 running 行但 active_phase=None → reason 不附相位语境（标定记因定式两态） | 新增 |
| 启动标定 | 边界 | 幂等：连续两次 calibrate 第二次零变化（无 running 残留可标、active_phase 已清，AC-4） | 新增 |
| 启动标定 | 边界 | 零残留库 calibrate → 零副作用（既有记录全部字段含时间戳原值不变——零写事务的可观测面） | 新增 |
| 启动标定-挂点 | 正向 | open_workspace 内嵌标定：首轮打开后种残留（start running + 置 active_phase）→ drop 句柄重开同一 db 文件 → 残留已标定（AC-4 D12 挂点） | 新增 |
| 启动标定-挂点 | 正向 | 存量库 additive 打开：既有八模型形态库文件（零 run 写入）重开成功、新模型表首轮读写正常（AC-1 十模型 additive 零迁移） | 新增 |
| run 史读面 | 正向 | list_change_runs 按 started_at 升序（含并列 run_id 稳定并列序）；list_run_steps 按 seq 升序（seq 空洞序合法）（AC-6/AC-10 前置） | 新增 |
| run 史读面 | 边界 | 无 run 的 change → list_change_runs 空数组；未知 run_id → list_run_steps 空数组（miss 非错误） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | tempfile 真件库真实事务；「中途死亡」形态以构造残留（start 后不 finish）模拟——spec scenario 同式，零 mock | 本节全部 describe |

---

### packages/desktop/src-tauri/crates/infra/store/src/change_port.rs -> packages/desktop/src-tauri/crates/infra/store/src/change_port_test.rs

#### 待测功能

- ChangeStateStore for Store（list_runs / list_run_steps / run_start / run_finish）: trait 委托胶水 + StoreError → StoreFault 映射

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| run 域 port 委托 | 正向 | 种子（start + finish）后经 port 面读回与直调操作面等值（list_runs / list_run_steps 逐字段，AC-1 port 面） | 新增 |
| run 域 port 委托 | 异常 | Conflict（同 run_id 重复 start）与 NotFound（未建档 finish）经 port 面映射为 StoreFault::Conflict / NotFound，Display 记因语境保留 | 新增 |
| run 域 port 委托 | 边界 | 空库形态：list_runs 空数组、list_run_steps 空数组（miss 非错误经 port 面不变） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | tempfile 真件库，port 面与操作面同库对照 | 本节全部 describe |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/state.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/state_test.rs

#### 待测功能

- RunNotice: 五 kind-only 变体（tag `ipc` camelCase：step / sessionEvent / ask / confirmWait / finished，零载荷）
- ChangeRunSnapshot: 扩展 started_at（毫秒）与 steps（累积器快照，全词汇 emit 序）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| RunNotice 线面 | 正向 | 五变体序列化出线恰为 `{"ipc":"step"}` / `{"ipc":"sessionEvent"}` / `{"ipc":"ask"}` / `{"ipc":"confirmWait"}` / `{"ipc":"finished"}`——零载荷键（AC-8 通知降位形态） | 新增 |
| RunNotice 线面 | 边界 | 非法 ipc 词反序列化 Err（封闭集防线） | 新增 |
| ChangeRunSnapshot 扩面 | 正向 | 含 startedAt / steps 的快照序列化出线（steps 逐条 ChangeStepState camelCase 全词汇形态，AC-10 重挂步表面） | 新增 |
| ChangeRunSnapshot 扩面 | 边界 | steps 空数组与 ask=null 两态出线；attempt / phase null 形态 | 新增 |
| RunUpdate seam（既有回归） | 正向 | 既有 RunUpdate 五变体载荷出线用例保绿（内部 seam 未动——归档链 / worker.rs 零触点锚，AC-8） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | 纯类型 serde 直测 | 本节全部 describe |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/control.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/control_test.rs

#### 待测功能

- ChangeFlowControl.begin_run(): 发起登记（签名增 started_at；并行冲突检测不变）
- ChangeFlowControl.publish(): 快照面步累积（RunEntry.steps emit 序追加、状态迁移照旧）+ 广播 RunNotice（载荷剥离单点）
- ChangeFlowControl.snapshot(): 含 started_at / steps（克隆累积器）
- RunGuard.steps(): 累积器快照读取
- RunGuard.finish(): 终态广播 + 除名（唯一出口不变量回归）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 步累积器与通知降位 | 正向 | publish(Step) → snapshot().steps 按到达序追加（phase/attempt 随行推进）+ 订阅端收到 RunNotice::Step（快照面与广播面同拍，AC-8/AC-10） | 新增 |
| 步累积器与通知降位 | 正向 | publish(SessionEvent) → 快照面零变化（steps / status / phase 不动）+ 广播 RunNotice::SessionEvent（转录重查分流键，D8） | 新增 |
| 步累积器与通知降位 | 正向 | publish(Ask) / publish(ConfirmWait) / publish(Finished) → 状态迁移与 ask 载荷照旧 + 对应 Notice kind 各自广播（AC-8） | 新增 |
| 步累积器与通知降位 | 边界 | 终态除名后 publish 零累积零广播（既有语义回归——run_finish 后注册表无条目） | 新增 |
| begin_run 扩参 | 正向 | begin_run 携 started_at → snapshot().started_at 等值入档（与 RunRequest 同值锚，D5） | 新增 |
| begin_run 扩参 | 异常 | 同 (root, change) 二次 begin → Err 并行冲突且首个 run 快照面不受扰动（既有断言随签名随动） | 新增 |
| 快照独立性 | 边界 | snapshot() 返回 steps 为克隆：取快照后继续 emit，已取快照不变、新快照含新步（AC-10 重挂读一致性） | 新增 |
| RunGuard.steps() | 正向 | guard.steps() 与 control.snapshot().steps 等值（walker 第二写取累积器的读面，AC-2 组装输入） | 新增 |
| wire(RunUpdate) 广播断言（既有） | 废弃 | broadcast 端 RunUpdate 载荷 JSON 断言用例废弃 → 改 RunNotice kind-only（信封降位，AC-8） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | tokio 原语（broadcast / oneshot / watch / Mutex）进程内真实组合，沿既有装置（yield_now 轮询握手） | 本节全部 describe |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/port.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/port_test.rs

#### 待测功能

- RunHistoryPort: run 落库 port 缝（run_started / run_finished，sync 零 tokio、Err 串语义）
- RunEventSink（既有）: worker.rs → ChangeFlowSink / ArchiveSink 透传 seam（零改动核对锚）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| RunEventSink seam 回归 | 正向 | 既有 RecordingSink 断言保绿：RunUpdate（含 SessionEvent 载荷）透传面零改动（AC-8 归档链零触点锚） | 新增 |
| RunHistoryPort 签名面 | 边界 | trait 声明面经消费侧承载：进程内假件（计数 / 可编程 Err）在 walker_test.rs、真件 StoreRunHistory 在 run_history_test.rs 断言（本文件不重复行为用例，注记） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | 既有 sink 直调装置；新 trait 行为断言挂消费侧测试文件 | 本节全部 describe |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/run_history.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/run_history_test.rs

#### 待测功能

- persisted_step(): 10→5 步词汇过滤单点（executor / evaluator / decision / static_check / test_execution 落；phase_start / phase_log / verdict_gate / retry_gate / whitelist_gate 弃）
- finish_command(): finish 组装 helper（steps → RunStepEntry：seq = 全词汇 emit 序盖戳、status / session_id / detail 透传、timestamp = finished_at）
- StoreRunHistory: RunHistoryPort 适配器（持 `Arc<dyn ChangeStateStore>` 委派 workflow::write 两函数）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| persisted_step 单点 | 正向 | 五落词汇逐一映射到对应 RunStepKind（Some 等值，AC-3） | 新增 |
| persisted_step 单点 | 正向 | 五弃词汇（PhaseStart / PhaseLog / VerdictGate / RetryGate / WhitelistGate）逐一 None（AC-3 忽略集） | 新增 |
| persisted_step 单点 | 边界 | 十词汇全集逐一断言恰五 Some 五 None（映射表全量钉死——词汇漂移即败，AC-3） | 新增 |
| finish_command 组装 | 正向 | 全词汇 10 步 emit 序列（含被过滤步穿插）输入 → RunFinishCommand.steps 恰 5 条且 seq 保留全词汇 emit 序产生空洞（如 0,1,3,6,9 形态）、每条 timestamp = finished_at 同刻（AC-2/AC-3/AC-10 seq=emit 序） | 新增 |
| finish_command 组装 | 正向 | run_id / change / status / reason 自 RunRequest 与收口参数透传等值 | 新增 |
| finish_command 组装 | 边界 | 空步序列 → steps 空包；session_id / detail None 与 Some 两态透传（AC-11 两态样本语义前置） | 新增 |
| StoreRunHistory 委派 | 正向 | 真件组合：run_started 落 running 行、run_finished 整包落库——`list_runs` / `list_run_steps` 回读与命令逐字段一致（AC-1 链路半边 / AC-2 第二写实装） | 新增 |
| StoreRunHistory 委派 | 异常 | 委派 Err（未建档 change / 重复 run_id 冲突）→ `Err(String)` 串语义透传且记因语境保留（walker fail-fast / best-effort 分流的输入面） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | 纯函数直测（persisted_step / finish_command）+ tempfile 真件库（沿 walker_test TestDb 装置）承载 StoreRunHistory 委派 | 本节全部 describe |

---

### packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs

#### 待测功能

- walk_run(): run 落库缝（起点第一写 run_started + 终态出口单点第二写 run_finished；每 run 恰两写；start 失败 fail-fast、finish 失败 best-effort）
- RunRequest.started_at: 发起时刻命令携带（与 begin_run 同值）
- RunGuard.steps(): 累积器快照（guard 侧读面）
- 组合链路（挂靠本节，链路发起方 = walk_run）: walk_run → RunHistoryPort → StoreRunHistory → 真件 store 全链落库

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 每 run 两写时序 | 正向 | 假引擎一相位绿跑（executor + static_check + evaluator 多步 emit）→ 计数假件断言恰两次调用（run_started 1 次 + run_finished 1 次）、run 中零额外 run 域写（AC-2） | 新增 |
| 每 run 两写时序 | 正向 | 真件组合（StoreRunHistory + TestDb）：多相位 run 走完 → 库读史回验 RunRecord 终态 / reason / 起止 + RunStepRecord 五词汇整包、seq 与 emit 序逐条一致（AC-1/AC-10 重挂步表与 emit 序一致） | 新增 |
| 每 run 两写时序 | 正向 | 全词汇 run（emit 覆盖十类步词汇，含三门与相位机步）→ 落库 run_steps 恰五词汇、流程面与三门零行（AC-3 缺席断言） | 新增 |
| 每 run 两写时序 | 正向 | 三死亡路径同一出口：completed（全相位 pass）/ stopped（停等拒绝或用户停止）/ failed（会话失败 / verdict 解析失败）三形态各自完成第二写且终态与 reason 各異（AC-2 全死亡路径汇聚） | 新增 |
| 每 run 两写时序 | 正向 | 中途停止（phase_log 未达、active_phase 在位）收口后 → 真件库 active_phase 读回 None（finish 事务清位，AC-5 悬挂杀除链路面） | 新增 |
| 每 run 两写时序 | 正向 | 续走不重头：同 change 第二轮 run（假引擎记录执行相位序列）→ 已 pass 相位零重执行、attempt 序延续递增（AC-5 续走锚点） | 新增 |
| 每 run 两写时序 | 异常 | run_started Err（可编程假件注入）→ fail-fast：工具步 / 会话零调用（零相位执行）、run 以 failed 收口且注册表除名（guard.finish 仍达，D5） | 新增 |
| 每 run 两写时序 | 异常 | run_finished Err（可编程假件注入）→ best-effort 静默：run 照常返回终态、订阅释放与除名不受阻、不重试不报错（D6） | 新增 |
| 每 run 两写时序 | 边界 | RunRequest.started_at 与真件库 RunRecord.started_at 同值（命令铸造面锚，D5） | 新增 |
| 每 run 两写时序 | 边界 | guard.steps() 快照与 control.snapshot().steps 等值（第二写组装输入一致性） | 新增 |
| 既有用例基设（签名随动） | 边界 | 既有 walk_run 用例机械随动：加 history 参（真件走 StoreRunHistory / 计数走假件）、RunRequest 补 started_at 定值字段（断言不弱化） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| RunHistoryPort（入参例外） | 进程内假件：Mutex 记录 start / finish 调用序与命令、可编程 Err 注入（start 失败 / finish 失败两形态）——作为 walk_run 显式入参注入 | 「恰两次调用」「fail-fast」「best-effort」 |
| db 文件（进程边界） | 真件 TestDb（tempfile workspace 库）+ StoreRunHistory 真实委派（既有装置沿用，组合轮不注入假件） | 「真件组合回验」「active_phase 清位」「续走」「started_at 同值」 |
| 假引擎（既有装置） | 沿用既有 scripted WorkerAgentPort / ToolStepPort 假件（被测入参，既有先例） | 本节全部 describe |

---

### packages/desktop/src-tauri/src/commands/changes/mod.rs -> packages/desktop/src-tauri/src/commands/changes/mod_test.rs

#### 待测功能

- ChangeDetailUnified / ActiveRunView: shell 统一视图 DTO（specta；startedAt 毫秒 → ISO 串收命令层单点）
- get_change_detail(): 统一查询装配（core `change_detail` ∪ `ChangeFlowControl::snapshot` 活面投影；blank root / 开库失败 None 语义不变）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 统一查询装配 | 正向 | 无运行 run：返回 detail.runs 全史 + active_run=null——单命令一次返回（AC-7） | 新增 |
| 统一查询装配 | 正向 | 运行中 run（已 emit 多步、含 waitingAsk 载荷形态 / waitingConfirm 停等形态各一轮）→ activeRun 六值状态 + steps 全词汇 emit 序 + ask 载荷 + phase / attempt（AC-7） | 新增 |
| 统一查询装配 | 正向 | 终态收口后（注册表除名）→ activeRun 退 null、runs 尾行（status / reason / 起止）可达（AC-7/D9 收口后状态面） | 新增 |
| 统一查询装配 | 正向 | ActiveRunView.startedAt 毫秒 → ISO 串（与 detail.runs 时间口径同式，转换单点在本命令层） | 新增 |
| 统一查询装配 | 边界 | blank root / 开库失败 → None（既有早退语义回归，零改写） | 新增 |
| 统一查询装配 | 边界 | 文档形态 change（db 缺记录）→ detail 文档形态 + active_run=null（不虚构活面） | 新增 |
| 统一查询装配 | 边界 | 未知 change 名（record 与定位双缺）→ None（既有语义不变） | 新增 |
| 既有读命令回归 | 正向 | list_changes / read_artifact / create_change / archive_change 既有用例保绿（本组零语义改动锚）；get_change_detail 返回型随动解包 | 新增 |
| change_flow_state 消费面（废弃） | 废弃 | get_change_detail 旧 ChangeDetail 返回形态断言废弃 → 统一视图解包形态 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| tauri 运行环境（进程边界） | 既有装置：`tauri::test::mock_app()` + 真实 WorkspaceStores（tempdir 数据根）托管，`app.state::<…>()` 直调（沿本文件既有 Env / app_with 先例） | 本节全部 describe |
| 注册表活面 | 真实 `Arc<ChangeFlowControl>` 托管进 mock app；活面种子经 begin_run + publish（进程内真实组合，零 mock） | 「运行中 run」「终态退场」 |

---

### packages/desktop/src-tauri/src/commands/change_flow/mod.rs -> packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs

#### 待测功能

- change_flow_start_with(): started_at 铸造（now_millis）+ begin_run + RunRequest 装配 + StoreRunHistory 注入 walk_run + Channel<RunNotice> 转发
- change_flow_watch_with(): 运行中 run 的 RunNotice 补订（无运行 run → Ok 零订阅）
- change_flow_state(_with): 命令 + handler 注册整体退役（D4）
- change_flow_stop / confirm / answer: 三控制命令零改动（请求-应答回归锚）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 发起与通知桥 | 正向 | start 受理 → 真件库 RunRecord 起始行落库且 started_at 与返回 summary 同 run 会话（walker 起点第一写经注入 StoreRunHistory 真实落库，AC-2/D5 命令面装配） | 新增 |
| 发起与通知桥 | 正向 | Channel 面收 RunNotice kind-only：step / sessionEvent / ask / confirmWait / finished 各自一拍出线且零载荷键（AC-8） | 新增 |
| 发起与通知桥 | 正向 | watch 重挂补订 → 与 start 订阅同流 Notice（补订命中运行中 run） | 新增 |
| 发起与通知桥 | 边界 | 无运行 run 时 watch → Ok 且零转发任务订阅（既有语义随 RunNotice 化） | 新增 |
| 发起与通知桥 | 边界 | 既有前置校验回归保绿：无建档 / 相位表缺失 / 归档互斥 / worktree 缺失 / 复合键并行冲突（零语义改动锚） | 新增 |
| 控制面回归 | 正向 | stop（幂等）/ confirm / answer 三命令既有用例保绿（请求-应答零改动，AC-8） | 新增 |
| change_flow_state 用例 | 废弃 | 快照命令用例整体删除（命令 + `_with` 缝 + 注册出列；重挂恢复归统一查询——AC-8/D4） | 废弃 |
| Channel(RunUpdate) 载荷断言 | 废弃 | 既有 Channel 面载荷信封断言废弃 → RunNotice kind-only 断言（信封降位） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| tauri 运行环境（进程边界） | 既有装置：mock_app + 真实 WorkspaceStores / Arc<ChangeFlowControl> / Arc<StopRegistry> 托管（沿本文件 Env / app_with 先例） | 本节全部 describe |
| Channel（进程边界） | 沿既有 Channel 捕获装置（on_message InvokeResponseBody 收集）驱动假引擎背景 run | 「RunNotice 各拍」「watch 补订」 |

---

### packages/desktop/src-tauri/crates/core/workflow/tests/fixtures（run 维度种子构造器） -> packages/desktop/src-tauri/crates/core/workflow/tests/corpus_golden_test.rs

#### 待测功能

- run 维度种子构造器: 经 store run 域操作面（run_start / run_finish 整包）+ calibrate_interrupted_runs 构造中断样本（禁裸表插桩）
- golden 聚合快照: detail 线面 runs / steps 投影守卫（显式重写流程后钉死）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| run 维度语料 golden | 正向 | 多 run 全史样本（≥2 run、五词汇步、session_id / detail 有无两态、attempt 跨 run 递增）→ golden 含 runs 键全史投影且与库内记录逐字一致（AC-11/AC-6） | 新增 |
| run 维度语料 golden | 正向 | interrupted 标定样本（run_start 后标定）→ golden 含 status=interrupted、reason 定式与 active_phase 清位面（AC-11/AC-4） | 新增 |
| run 维度语料 golden | 正向 | steps 投影恰五词汇封闭集断言（RunStepKind 类型封闭使流程面词汇结构不可表达——词汇集钉死随快照常驻，AC-3/AC-11 缺席断言） | 新增 |
| run 维度语料 golden | 边界 | 既有六 detail golden 全量增 runs 键（存量零 run 样本 → 空数组）——显式重写流程（覆写模式 + 人工确认 diff 预期范围留痕）后快照回归钉死（AC-6） | 新增 |
| 语料完整性守卫（既有） | 正向 | 「golden 目录与语料集合一致」复核用例随新语料样本扩展保绿（AC-11） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | 既有语料装置（真件库 + fixtures 构造器）扩展 run 维度；种子一律经 store run 域操作面（与真实写路径同构） | 本节全部 describe |

---

### packages/desktop/src/views/changes/hooks/use-change-detail.ts -> packages/desktop/src/views/changes/hooks/use-change-detail.test.ts

#### 待测功能

- useChangeDetail(): 统一视图取数（detail + activeRun 态 + 产物信封组装既有面）
- notifyRefresh(): 统一视图失效重取（300ms 尾随去抖）
- notifyTranscript(): 转录刷新信号（150ms 尾随去抖）
- refresh(): 显式刷新即时语义（保留）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 统一视图取数 | 正向 | getChangeDetail 返回 ChangeDetailUnified → detail 与 activeRun 双态就位、产物信封组装照常（AC-7 前端半边） | 新增 |
| 统一视图取数 | 边界 | root / change null → 空态零取数；getChangeDetail 返回 null → detail=null 空态（既有回归） | 新增 |
| 统一视图取数 | 异常 | getChangeDetail reject → error 态呈现、loading 复位（既有回归） | 新增 |
| notifyRefresh 去抖 | 正向 | 去抖窗内连续多次 notifyRefresh → 恰一次重查 invoke（fake timers 前进 300ms 后一拍，AC-8/R2 节流） | 新增 |
| notifyRefresh 去抖 | 边界 | 窗内新到达重置计时（trailing 语义：最后一拍必达——重查发生在最后一次通知后 300ms） | 新增 |
| notifyTranscript 去抖 | 正向 | notifyTranscript 独立 150ms 节流且不触发统一视图重查；notifyRefresh 不触发转录信号（kind 分流互不串台，D8） | 新增 |
| refresh 即时语义 | 正向 | refresh() 绕过去抖即刻重查（运行外显式刷新唯一更新途径语义保持） | 新增 |
| 卸载面 | 边界 | 卸载弃挂起计时器与在途重查（零泄漏重查 / cancelled 面既有） | 新增 |
| 旧返回形态断言 | 废弃 | 旧 ChangeDetail 单体返回形态断言废弃 → activeRun 解包形态 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| invoke（进程边界） | `vi.mock('@tauri-apps/api/core')`：invoke 按命令名分发（getChangeDetail / readArtifact 可编程 resolve / reject + 调用计数）——沿本文件既有 mock 装置 | 本节全部 describe |
| 计时器（运行环境） | vi.useFakeTimers + advanceTimersByTime 驱动 300/150ms 窗口 | 「去抖」全部用例 |

---

### packages/desktop/src/views/changes/hooks/use-change-flow-run.ts -> packages/desktop/src/views/changes/hooks/use-change-flow-run.test.ts

#### 待测功能

- useChangeFlowRun(): 缩位重写——控制动作（start / stop / confirm / answer，invoke 面零改动）+ 通知订阅生命周期（activeRunPresent 才订阅、发起先行订阅、finished 后自然断开、卸载弃投递；onNotice 按 kind 分流）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 控制动作面 | 正向 | start(autoNextPhase) → 先构造 Channel 订阅再 invoke changeFlowStart、入参序列零改动（AC-8 控制面不变） | 新增 |
| 控制动作面 | 正向 | stop / confirm / answer 各自动作 invoke 入参与既有完全一致（请求-应答回归） | 新增 |
| 控制动作面 | 异常 | 动作 reject → error 态呈现（start 先行清错既有语义） | 新增 |
| 通知订阅生命周期 | 正向 | activeRunPresent=true → changeFlowWatch 补订（Channel 挂 onNotice）；activeRunPresent=false → 零 watch 调用零 Channel 构造 | 新增 |
| 通知订阅生命周期 | 正向 | onNotice 分流：投递 step / sessionEvent / ask / confirmWait / finished 五种 RunNotice 各自回调一拍（AC-8 kind 位） | 新增 |
| 通知订阅生命周期 | 边界 | finished 通知后通道自然断开：后续投递零回调（订阅释放面） | 新增 |
| 通知订阅生命周期 | 边界 | 卸载弃投递；root / change 切换重挂（channel 引用复位既有纪律） | 新增 |
| changeFlowState 恢复用例 | 废弃 | 快照恢复（initialRunState 种子 + isTerminalStatus 补订分支）用例整体删除（D4 重挂归统一查询） | 废弃 |
| RunUpdate 归并用例 | 废弃 | applyRunUpdate / seedRunState 状态机镜像与 Channel RunUpdate 载荷投递用例整体删除（AC-8 前端解散） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| invoke + Channel（进程边界） | 沿既有 `vi.hoisted` 装置：ChannelMock 类捕获 onmessage（测试直接投递 RunNotice 信封）、invokeMock 按命令名分发并记录入参 | 本节全部 describe |

---

### packages/desktop/src/views/changes/flow/run-state.ts -> packages/desktop/src/views/changes/flow/run-state.test.ts

#### 待测功能

- unifiedRunSteps(runs, liveSteps): 统一视图步输入纯函数（runs[].steps 展开 + 活步并入；步词汇归一单点 `static_check`→`staticCheck` / `test_execution`→`testExecution`；runs 序 = startedAt 稳定分层序）
- runStepNodes(steps): 槽位配对机制（同键 running→终态配对、重号 `:seq` 后缀堆叠——保留）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| unifiedRunSteps | 正向 | runs[].steps snake 词归一 camel 词（static_check→staticCheck / test_execution→testExecution），executor / evaluator / decision 零改写（AC-9 词汇归一单点） | 新增 |
| unifiedRunSteps | 正向 | 多 run 展开保 startedAt 序 + liveSteps 活步（全词汇 camel）并入尾随——输出即 runStepNodes 单一输入（AC-9 同一转换函数入口） | 新增 |
| unifiedRunSteps | 边界 | runs 空数组 + liveSteps 空数组 → 空输出（零 run 收口后无史形态）；仅 live（首 run 运行中零库史）→ 活步全量出 | 新增 |
| unifiedRunSteps | 边界 | 两 run 全史叠加：同相位 attempt 递增不撞键、`(run 起始, emit 序)` 稳定分层（AC-9 全史叠加） | 新增 |
| runStepNodes 槽位机制 | 正向 | 同键 running→终态配对收敛（终态覆写 open 槽）；重号步 `:seq` 后缀堆叠（attempt 复用撞键例外吸收——design 边界留痕节，AC-9） | 新增 |
| runStepNodes 槽位机制 | 边界 | running 步无终态配对 → open 槽保持（在飞 pulse 面）；非 PIPELINE_PHASES 相位步滤除（既有守卫保留） | 新增 |
| reducer 镜像（既有） | 废弃 | applyRunUpdate / initialRunState / seedRunState / appendEvent（liveEvents + seq 去重）全部用例删除（AC-8 解散） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | 纯函数直测，fixture 以 bindings 类型内存构造（沿本文件既有 fixture 风格） | 本节全部 describe |

---

### packages/desktop/src/views/changes/flow/graph.ts -> packages/desktop/src/views/changes/flow/graph.test.ts

#### 待测功能

- buildFlowGraph(detail, runNodes?): 骨干 + runNodes 恒尾追加归并（签名不变；调用侧单源拼装语义由 change-detail-view 节承载）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 流程图模型派生（既有回归） | 正向 | 骨干 eval 收集 / active 按 startAt 插入 / 边推导（forward / retry / backtrack）既有用例保绿——签名零改动锚 | 新增 |
| 流程图模型派生（既有回归） | 正向 | runNodes 非空时恒尾追加参与同链边推导（既有用例保绿；统一视图派生节点常驻参与全链的形态复断言，AC-9 派生路径不变面） | 新增 |
| 流程图模型派生（既有回归） | 边界 | 空 pipeline → 空图（既有）；runNodes 缺省 → 仅骨干（既有） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | 纯函数直测（既有装置） | 本节全部 describe |

---

### packages/desktop/src/views/changes/change-detail-view.tsx -> packages/desktop/src/views/changes/change-detail-view.test.tsx

#### 待测功能

- DetailLoaded 单源拼装: activeRun 自 detail hook、run hook 供动作 + 订阅（onNotice 分流回调接线）
- useRunViewEffects: 删除（终态 refresh 回落分支 + liveEvents 展平双退场）
- 组合链路（挂靠本节，链路入口 = 视图组装层）: RunNotice → onNotice 分流 → notifyRefresh / notifyTranscript → 统一视图重查 → 图重绘

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 通知驱动重查链路（组合） | 正向 | activeRun 运行中投递 step 通知 → 去抖后统一视图重查、图重绘含新步节点（与 use-change-detail 真实组合，AC-8 取数模型 / AC-7 消费面） | 新增 |
| 通知驱动重查链路（组合） | 正向 | sessionEvent 通知 → 转录重查信号触发（150ms 分流，抽屉转录经转录库刷新，AC-10 转录统一） | 新增 |
| 通知驱动重查链路（组合） | 边界 | finished 通知 → 最后一拍统一视图重查后订阅释放（activeRun 退场、runs 尾行就位——「运行外显式刷新唯一途径」回归） | 新增 |
| 图常驻渲染 | 正向 | run 收口后刷新 / 重开详情（重渲染或重挂载）→ 步节点（executor / evaluator / decision / static-check / test-execution）常驻上图不回落、无 pulse 运行态（data-testid 断言，AC-9） | 新增 |
| 图常驻渲染 | 正向 | 两 run 全史叠加 + 第三 run 活步 pulse 叠加，同列 attempt 递增可辨（AC-9 全史叠加 + 在飞叠加） | 新增 |
| 重挂恢复 | 正向 | run 运行中重挂视图（remount）→ activeRun 经统一查询恢复、步表非空且与 emit 序一致、运行中才补订 watch（AC-10 重挂恢复） | 新增 |
| 重挂恢复 | 边界 | 无运行 run 重挂 → 空闲态 + 历史步节点常驻（AC-9/AC-10 邻接形态） | 新增 |
| useRunViewEffects 回落（既有） | 废弃 | 终态触发显式 refresh 回落用例与 liveEvents prop 链用例删除（AC-8 拆除面） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| invoke + Channel（进程边界） | 沿既有视图级 mock 装置：invoke 分发（getChangeDetail 可编程多拍返回）+ ChannelMock 投递 RunNotice；fake timers 驱动去抖窗 | 本节全部 describe |
| 子组件 | 真实组合不 mock（RunControlPanel / DetailDrawer / 流程图真实渲染），data-testid 查询挂钩（样式类名查询禁止） | 本节全部 describe |

---

### packages/desktop/src/views/changes/flow/run-control-panel.tsx -> packages/desktop/src/views/changes/flow/run-control-panel.test.tsx

#### 待测功能

- RunControlPanel props 面: activeRun（状态徽章 / 停等卡片 / ask 卡片自活面）+ lastRun（runs 尾行——终态徽章与收口记因）+ 动作与错误面（沿 run hook 供能；data-testid 既有挂钩零改号）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 控制面板活面 | 正向 | activeRun running → run-status 徽章呈现 + 停止按钮可用（AC-7 消费面） | 新增 |
| 控制面板活面 | 正向 | activeRun waitingAsk → run-ask-card 呈 question / options、自由文本提交触发 answer 动作（既有交互回归） | 新增 |
| 控制面板活面 | 正向 | activeRun waitingConfirm → run-confirm-card 双按钮触发 confirm(true/false) 动作（既有交互回归） | 新增 |
| 控制面板收口面 | 正向 | activeRun 退场 + lastRun 尾行 → 终态徽章 + run-finished-reason 读 runs 尾行 reason（D9 收口记因改读库史） | 新增 |
| 控制面板收口面 | 边界 | lastRun interrupted（标定样本形态）→ 徽章呈现 interrupted、记因呈现标定语境 | 新增 |
| 控制面板空闲面 | 边界 | activeRun null + lastRun null（零 run change）→ 发起为主操作、状态徽章缺席（既有空闲形态回归） | 新增 |
| 旧 props 形态（既有） | 废弃 | run.state（ChangeFlowRunState 镜像输入）与 finishedReason 自镜像的用例废弃 → activeRun / lastRun 输入形态（testid 零改号断言保留） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 动作回调（入参例外） | props 注入 vi.fn spy（start / stop / confirm / answer），activeRun / lastRun 以 bindings 类型内存构造（沿本文件既有 runStub 装置改写） | 本节全部 describe |

---

### packages/desktop/src/views/changes/flow/detail-drawer.tsx -> packages/desktop/src/views/changes/flow/detail-drawer.test.tsx

#### 待测功能

- DetailDrawer: liveEvents 参数链退役；转录刷新键（refreshKey）下传 SessionTranscriptPanel

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 抽屉转录联动 | 正向 | eval 节点三转录 tab（executor / evaluator / decision）与槽位缺席空态既有用例保绿（回归锚） | 新增 |
| 抽屉转录联动 | 正向 | 转录面板接收 refreshKey：通知信号经视图下传触发面板重查（与 SessionTranscriptPanel 真实组合断言，AC-10） | 新增 |
| 抽屉转录联动 | 边界 | active 节点反查联动（sessionId 恒 null 走 sourceRef）既有用例保绿 | 新增 |
| liveEvents 参数链（既有） | 废弃 | liveEvents 过滤传递用例删除（参数链退役，AC-8） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| invoke（进程边界） | 沿既有装置：转录 / 详情查询命令 mock 分发 | 本节全部 describe |
| 子组件 | SessionTranscriptPanel 真实组合（不 mock） | 「refreshKey 下传」 |

---

### packages/desktop/src/views/changes/flow/session-transcript-panel.tsx -> packages/desktop/src/views/changes/flow/session-transcript-panel.test.tsx

#### 待测功能

- SessionTranscriptPanel: liveEvents 退役（转录库唯一数据源）；refreshKey 刷新键消费

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 转录面板呈现 | 正向 | 会话转录经库查询呈现：session-meta / drawer-session-* 既有 data-testid 锚保绿（回归） | 新增 |
| 转录面板呈现 | 正向 | refreshKey 变化 → 面板发起重查（与 useSessionTranscript 真实组合，AC-10 转录统一） | 新增 |
| 转录面板呈现 | 边界 | 查无会话 → drawer-session-empty 空态（既有）；running / sealed 徽章两态（既有） | 新增 |
| liveEvents 并入（既有） | 废弃 | liveEvents 实时并入 / 呈现用例删除（AC-8 liveEvents 退役） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| invoke（进程边界） | 沿既有装置：sessionDetail / agentSessionTranscript mock 分发 | 本节全部 describe |

---

### packages/desktop/src/views/changes/hooks/use-session-transcript.ts -> packages/desktop/src/views/changes/hooks/use-session-transcript.test.ts

#### 待测功能

- useSessionTranscript(): 转录库重查（root / sourceRef / sessionId / refreshKey 入参；sessionId 直查优先、sourceRef 反查回退既有面）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 转录寻址查询 | 正向 | sessionId 在场直查优先（sessionDetail + agentSessionTranscript）、缺席回退 sourceRef 反查（既有回归锚，重放路径行为不变——AC-10） | 新增 |
| 转录寻址查询 | 正向 | refreshKey bump → 重新发起同一寻址查询（通知触发重查面，AC-10） | 新增 |
| 转录寻址查询 | 边界 | 双 null（root 或 sessionId+sourceRef 均缺）→ 清空态零查询（既有）；查询 reject → error 态（既有） | 新增 |
| 转录寻址查询 | 边界 | refreshKey 不变时寻址键变化仍即时重查（节点切换联动既有语义不因刷新键引入而弱化） | 新增 |
| assembleTranscript（既有） | 废弃 | live 并入 + seq 去重合并用例删除（转录库唯一数据源，AC-8/AC-10） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| invoke（进程边界） | 沿既有装置：sessionDetail / agentSessions / agentSessionTranscript 可编程 resolve / reject + 调用计数 | 本节全部 describe |

---

## 不可测试项

- **AC-3「walker / control 源码零过滤逻辑」grep 面** — 源码静态审查约束（过滤 match 仅允许出现在 `run_history.rs::persisted_step`），由实现阶段红线自查 grep 与 code-review 承载，非进程内可断言；行为面（恰五落 / 零流程面行）已由 run_history_test / walker_test / corpus 缺席断言覆盖。
- **归档链与 seam 零触点（`archive_flow.rs` / `ArchiveSink` / `use-archive-flow.ts` / `worker.rs` / `RunUpdate` 类型定义零 diff）** — 「零改动」是 diff 面断言而非行为断言；port_test / state_test 的既有用例保绿作为行为侧回归锚，diff 审查归实现阶段自查。
- **AC-6 golden「人工确认留痕」** — 显式重写流程中的人工确认是流程性步骤，不可自动化断言；golden 等值回归本身由 corpus_golden_test 覆盖（覆写模式仅在重写流程启用）。
- **bindings 再生成一致性（`packages/desktop/src/types/generated/bindings.ts`）** — 生成物零手改、由 bindings 一致性守卫（管线命令）拦截漂移，非单测面；前端类型消费正确性由各前端测试以新类型 fixture 承载。
- **AC-8 knip 零未用导出残留 / AC-11 `vp test`、`client:check`、`server:check` 全绿与 desktop version 0.4.28、`plugins/dev-team` 零改动** — 管线守线与静态核对项（版本号 / 插件产物零 diff），归实现阶段守线任务与验收阶段承载。
- **R7 通知-重查竞态「静默期最坏少一拍」端到端形态** — 通知到达与查询落库之间的时序窗口行为不可确定性复现；去抖合并行为（最后一拍必达）已在 use-change-detail.test 断言，端到端竞态由「通知失效信号 + 查询权威」语义兜底，不做单测。
- **进程 kill 崩溃注入（真实 SIGKILL 中断 walker 验证步史丢失）** — 端到端崩溃注入不可确定性断言；以「构造残留 running 行（start 后不 finish）+ 启动标定」近似承载（store_test 已列，spec scenario 同式），崩溃丢步史为 A 骨架既定权衡留痕。
- **broadcast 容量溢出滞后丢事件 / Channel 断流的端到端丢事件形态** — 由重挂统一查询兜底语义承载（R7 语义），不做确定性单测；订阅生命周期边界（finished 断开、卸载弃投递）已在 use-change-flow-run.test 断言。
