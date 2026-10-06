# 测试设计: desktop-workflow-db-state

> **日期**: 2026-10-06

---

## 验收范围

<!-- 逐条映射 proposal.md 的 10 条 AC。被测文件或模块为承载用例的测试文件（单值）；
     跨半边 AC 以「（同上——…半边）」行展开到各自测试文件；纯静态 / 流程性半边落
     「—（见不可测试项 N）」。 -->

| AC ID | 验收条件 | 被测文件或模块 |
|--------|---------|----------|
| AC-1 | store 四模型与 change 域操作面：回环（建档 → phase_log 含 checklist 多条 → 重开 db 逐字段一致）；打包键同相位内自然序即 evaluator 输出序；重复落账返回 StoreError；native_model id 分配不冲突；`list_models` / `scan` 零改动覆盖四新模型 | packages/desktop/src-tauri/crates/infra/store/src/model_test.rs（回环与查重 / 信封覆盖半边见 AC-1 同上行 → store_test.rs） |
| AC-1 | （同上——change 域操作面半边：建档查重 / attempt 事务内推导 / `(change, phase, attempt)` 写事务查重 / max+1 主键 / workspace 组注册 4→8 信封零改动覆盖） | packages/desktop/src-tauri/crates/infra/store/src/store_test.rs |
| AC-2 | 写面 db 化与 port 缝：七操作全部经 workspace 库落库；phase_log 中途构造失败后 db 零残留；`crates/core/workflow/Cargo.toml` 零 infra/store 依赖；写面保持同步签名 | packages/desktop/src-tauri/crates/infra/store/src/store_test.rs（单事务原子与零残留半边；Cargo.toml 依赖方向静态审查见不可测试项 3） |
| AC-2 | （同上——port 缝消费半边：七写操作经 `dyn ChangeStateStore` 假件注入落库，路由 / 白名单 / 长度 / 表位 / stale 闭包等校验前置逐操作对照保持，七节各自挂 AC-2，本行为重操作代表） | packages/desktop/src-tauri/crates/core/workflow/src/write/phase_log_test.rs |
| AC-2 | （同上——port 适配器半边：`impl ChangeStateStore for Store` 记录 ↔ 中性类型映射委托、StoreError → StoreFault 三分支翻译、Send + Sync 边界锚定） | packages/desktop/src-tauri/crates/infra/store/src/change_port_test.rs |
| AC-3 | 双向墙与 parse 退役：`parse/` 目录不存在；全源码无 workflow.json 读 / 写触点（测试夹具除外）；存量 CLI change（有 workflow.json 无 db 记录）清单以文档形态可见、详情可读产物 | packages/desktop/src-tauri/crates/core/workflow/src/queries/detail_test.rs（目录删除与触点扫描静态半边见不可测试项 1 / 2；列表半边经 list_test.rs 同条随动） |
| AC-4 | queries db 读与发现语义：列表 = db 记录 ∪ 磁盘目录去重并集、同名共存以 db 为准 | packages/desktop/src-tauri/crates/core/workflow/src/queries/list_test.rs |
| AC-4 | （同上——db 缺记录条目无状态面（空流水线 + 产物清单）；detail 时间出线 ISO 串 + null（golden 守卫绿）） | packages/desktop/src-tauri/crates/core/workflow/src/queries/detail_test.rs |
| AC-4 | （同上——产物发现输入面演进半边：`discover_artifacts` / `read_artifact` 改 `&[PhaseStateRecord]` 切片入参、eval-checklist 候选锚定自 `Workflow.eval` 下标平移到 PhaseRecord 序列；空切片 = 文档形态仅文件候选） | packages/desktop/src-tauri/crates/core/workflow/src/artifacts/registry_test.rs |
| AC-4 | （同上——`change_flow_start` 对无建档 change 显式拒绝半边） | packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs |
| AC-5 | orchestration 全链走通：假引擎驱动 run 的 phase_next / phase_start / phase_log / backtrack 全部落库；StepRecord 按 run 可枚举完整步骤历史 | packages/desktop/src-tauri/crates/core/orchestration/src/steps_test.rs（`list_change_steps` 按 run 枚举半边经 store_test.rs 同条随动） |
| AC-5 | （同上——节点状态由 db 状态 × provenance 派生 + 已 pass 相位重启后不重头执行（active_phase 自 db 续走、锚点基线平移 PhaseRecord 行数）半边；快照 db 读源经 snapshot_test.rs 同条随动） | packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next_test.rs |
| AC-6 | create 建档：目录 + explore.md（goal 原文）+ ChangeRecord 三合一，无 workflow.json 产出；成功立即可见可发起；同名 active 冲突拒绝且 db 零建档 | packages/desktop/src-tauri/crates/core/workflow/src/write/create_test.rs（命令接线半边见 AC-6 同上行） |
| AC-6 | （同上——`create_change` 命令接线半边：建档写面 + 成功后清单 / 详情立即可见） | packages/desktop/src-tauri/src/commands/changes/mod_test.rs |
| AC-7 | 归档双写：db status=archived 且目录改名带日期前缀；主键 name 不变；无建档目录显式拒绝 | packages/desktop/src-tauri/crates/core/workflow/src/write/archive_test.rs |
| AC-7 | （同上——归档条目按月分组可达半边：`locate_change` archive 树日期前缀后缀匹配） | packages/desktop/src-tauri/crates/core/workflow/src/queries/mod_test.rs |
| AC-7 | （同上——`archive_change` IPC 薄命令半边（D11 无 UI 入口）） | packages/desktop/src-tauri/src/commands/changes/mod_test.rs |
| AC-8 | 前端退役面与全管线：清单 / 详情无 V0/V1/V2 徽章与「workflow.json 无法解析」文案；抽屉无文件表节、无 workflow 独立面板；建档两态分流 | packages/desktop/src/views/changes/change-detail-view.test.tsx（清单半边经 change-list-view.test.tsx、抽屉二分节半边经 detail-drawer.test.tsx、挂载分支半边经 attachments.test.ts 各节承载；client:check / knip 工具链门见不可测试项 4） |
| AC-8 | （同上——bindings 出线半边：`archive_change` 新命令与 `ArchiveOutcome` / `ChangeStatus` 新类型清单补录，退役类型消失经重导幂等 + golden diff 守卫） | packages/desktop/src-tauri/src/bindings/mod_test.rs |
| AC-9 | golden 显式重写：detail 线面三字段删除经 `DESKTOP_GOLDEN_REWRITE=1` 显式再生成；db 种子语料覆盖多 attempt / backtrack stale / 槽位全缺 / 文档形态 | packages/desktop/src-tauri/crates/core/workflow/tests/corpus_golden_test.rs（diff 人工确认留痕为流程性动作见不可测试项 5） |
| AC-10 | 版本交付：`packages/desktop/package.json` version 0.4.12 → 0.4.13（归档时执行；Cargo.toml 不随动） | —（见不可测试项 6） |

---

## 单元测试

<!-- 覆盖所有可在进程内验证的场景（Rust #[test]、前端 vite-plus 纯函数/组件 mock 测试）。
     测试框架识别结果：src-tauri 树全部模块落 packages/desktop/src-tauri rust 套件（cargo
     test --workspace 自动覆盖），测试文件为共置 *_test.rs 模块（沿 workflow / store /
     orchestration / commands 既有先例）与 tests/ 语料黄金测试；前端模块落 packages/desktop
     vite-plus 套件，共置 .test.ts(x)（vite-plus/test describe/it/expect）。
     db 为进程边界：种子经真实 store change 域操作面 + tempfile 真实组合（workflow 经
     dev-dependency 引 store，design 依赖节明定）；core 写面测试以进程内假件实现
     ChangeStateStore trait（design D1 明定的 fake port 先例，注入依赖入参）。
     门面 / 纯类型 / 生成物（workflow lib.rs 与 write|model 的 mod.rs、model/domain.rs、
     state.rs 类型词汇面、infra/store lib.rs、flow/types.ts、bindings.ts、fixtures/README.md、
     Cargo.toml）不建独立测试文件，统一落「不可测试项」声明（含行为去向）；lib.rs 仅作
     corpus 组合用例章节的挂靠门面（沿 checks-domain 先例）。既有 *_test.rs 扩展节仅声明
     新增用例行，既有用例零改动持衡；walker_test.rs / port_test.rs 既有假件随签名改写为
     design 实现期注意，行为断言面由 steps / snapshot / phase_next 各节承载。 -->

### packages/desktop/src-tauri/crates/infra/store/src/model.rs -> packages/desktop/src-tauri/crates/infra/store/src/model_test.rs

<!-- 挂 AC-1（四模型持久化形状半边：打包键 / id 分配 / serde 定制出线）。
     design.md 公共函数 / API 表未声明该文件模块级导出函数（纯记录模型 + 构造器）；
     待测面为类型定义表四记录模型（ChangeRecord id=9 / PhaseRecord id=10 /
     ChecklistItemRecord id=11 / StepRecord id=12 / ChangeActivePhase 嵌套）与 D4 打包键
     serde 定制。native_model 编解码自身语义不逐项验证（库语义），只测自研打包键与注册面。 -->

#### 待测功能

<!-- design.md 未声明该文件模块级导出函数；被测面为类型定义表行：ChangeRecord /
     PhaseRecord / ChecklistItemRecord / StepRecord / ChangeActivePhase（D3 id 9–12、
     D4 打包键十六进制 serde 定制、构造器）。 -->

- ChecklistItemRecord 打包键: `(phase_id as u128) << 64 | item_index` 构造与十六进制 serde 出线（D4，自研 serde 定制）
- 四记录模型构造器: ChangeRecord / PhaseRecord / ChecklistItemRecord / StepRecord 字段面（类型定义表）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| model_test · ChecklistItemRecord 打包键 | 正向 | 同一 phase_id 下 item_index 0..n 构造主键序列严格递增（主键自然序 = item_index 升序 = evaluator 输出序——AC-1 打包键序半边） | 新增 |
| model_test · ChecklistItemRecord 打包键 | 边界 | item_index=0 首条与高位 phase_id × 低位 item_index 组合（互不串位）：打包 / 还原往返 (phase_id, item_index) 逐字段一致 | 新增 |
| model_test · ChecklistItemRecord 打包键 | 边界 | 打包键十六进制字符串 serde 定制出线（信封 API JSON 可表达——serde_json 无 u128 数字面的自研绕法）与反向解码往返无损 | 新增 |
| model_test · 四记录模型注册 | 正向 | workspace 组注册 4→8 后 id 9 / 10 / 11 / 12 全部注册成功，与既有 id（1/2/4/5/6/7/8）无一冲突；id=3 历史退役空缺不复用（D3） | 新增 |
| model_test · 四记录模型注册 | 边界 | `list_models` 零改动覆盖四新模型：既有信封注册面不写一行即可浏览四新模型（AC-1 零改动覆盖半边，`scan` 行为经 store_test 信封行随动） | 新增 |
| model_test · 构造器字段面 | 边界 | PhaseRecord 三会话槽位 / start_at=None / backtrack 字段 None 的缺省构造合法可落（native_model 平直字段无 flatten、默认 bincode——D3 附加约束的编译锚定） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 纯内存构造 + tempfile 真实 workspace db 注册（Env 装置先例，进程边界用真实临时实例不 mock） | 本节全部用例 |

### packages/desktop/src-tauri/crates/infra/store/src/store.rs -> packages/desktop/src-tauri/crates/infra/store/src/store_test.rs

<!-- 挂 AC-1（回环 / 查重 / 信封覆盖半边）、AC-2（log_change_phase 单事务原子与中途
     失败零残留半边）、AC-5（list_change_steps 按 run 枚举半边）。既有 store_test.rs
     Env / StoresEnv tempfile 装置沿用，本节为扩展节。 -->

#### 待测功能

- Store::create_change_record(record: ChangeStateRecord) -> Result<ChangeStateRecord, StoreError>: 建档 + 同名 active 查重
- Store::find_change_record(name) -> Result<Option<ChangeStateRecord>, StoreError>: 主键直查
- Store::list_change_records() -> Result<Vec<ChangeStateRecord>, StoreError>: 主键自然序
- Store::start_change_phase(change, phase, now) -> Result<PhaseStartState, StoreError>: 单事务 active_phase 写入（attempt 事务内推导）
- Store::log_change_phase(command: &PhaseLogCommand) -> Result<u32, StoreError>: 单事务原子 + `(change, phase, attempt)` 查重，返回 attempt
- Store::apply_change_backtrack(command: &BacktrackCommand) -> Result<(), StoreError>: 单事务回跳 + stale 传播
- Store::amend_change_decision_session(change, phase, session_id) -> Result<(), StoreError>: 最新条目定点改写
- Store::set_change_archived(name, archived_at) -> Result<(), StoreError>: status 翻转（miss `Err`）
- Store::append_change_step(command: &StepCommand) -> Result<(), StoreError>: 步骤行追加（id max+1）
- Store::list_change_steps(change, run_id: Option<&str>) -> Result<Vec<StepStateRecord>, StoreError>: 按 change（可选 run）枚举
- Store::list_phase_records(change) -> Result<Vec<PhaseStateRecord>, StoreError>: 相位行 + checklist 子行内联重组

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| store_test · create_change_record + 回环 | 正向 | 建档 → find_change_record 逐字段一致（name 主键 / workflow_type / created_at / status / active_phase）→ open 重开 db 再读仍逐字段一致（AC-1 回环建档半边） | 新增 |
| store_test · create_change_record | 异常 | 同名 active 已在场再建档 → StoreError 记因且记录表零重复行（AC-1 查重半边；D5 冲突双检查的 db 半边） | 新增 |
| store_test · create_change_record | 边界 | 同名 archived 在场（已归档）再建档 active → 成功（查重仅限 status=active，「同名 active 冲突」口径） | 新增 |
| store_test · list_change_records | 边界 | 空 db → 空 vec；多建档后按主键自然序返回 | 新增 |
| store_test · start_change_phase | 正向 | 开相写 active_phase：phase / attempt=1 / start_at=now 逐字段落库；find_change_record 读出 active_phase 一致 | 新增 |
| store_test · start_change_phase | 边界 | start → log_phase 清位 → 同相位再 start：attempt 事务内推导为 2（重开 attempt 递增，无应用层计数器） | 新增 |
| store_test · start_change_phase | 异常 | 无建档 change 开相 → StoreError 显式（不做隐式建档） | 新增 |
| store_test · log_change_phase 单事务原子 | 正向 | start 后 log（含多条 checklist）→ 单次调用同落 PhaseRecord 行 + ChecklistItemRecord 子行（打包键序）+ active_phase 清位，返回 attempt；重开 db 读出逐字段一致（AC-1 / AC-2 回环与原子半边） | 新增 |
| store_test · log_change_phase | 异常 | `(change, phase, attempt)` 重复落账（同相位同轮二次 log）→ StoreError 记因（AC-1 重复落账半边） | 新增 |
| store_test · log_change_phase | 异常 | 中途构造失败注入（checklist 行非法形态触发写事务回滚）→ 重开 db 无半截 PhaseRecord / 无孤立 ChecklistItem 行（AC-2 零残留半边） | 新增 |
| store_test · log_change_phase | 边界 | checklist 空 vec → 相位行落库、子行集为空；start_at=None / 三槽位 None 的 PhaseLogCommand 合法落库透出 null 面 | 新增 |
| store_test · apply_change_backtrack | 正向 | 回跳命令（backtrack_to / reason + stale_dependents 闭包多条）→ 目标行回跳字段落库且闭包内全部相位条目 stale=true 翻转（AC-5 backtrack 落库半边） | 新增 |
| store_test · apply_change_backtrack | 边界 | stale_dependents 空 vec → 仅回跳行落库，无 stale 误伤 | 新增 |
| store_test · amend_change_decision_session | 正向 | 该相位最新条目 decision 槽位定点改写；重复 amend 幂等覆写同值（D9） | 新增 |
| store_test · amend_change_decision_session | 异常 | 该相位无任何条目 → StoreError 显式（NotFound 面） | 新增 |
| store_test · set_change_archived | 正向 | status → archived + archived_at 落库，主键 name 不变（AC-7 db 半边） | 新增 |
| store_test · set_change_archived | 异常 | name miss（未建档）→ StoreError 显式 | 新增 |
| store_test · append_change_step | 正向 | 连续追加 → id 严格 max+1 递增；run_id / step_kind / status / timestamp / reference 逐字段回读一致（AC-5 StepRecord 半边） | 新增 |
| store_test · append_change_step | 边界 | summary 超 500 字符 → `chars().count()` 口径截断并追加「…（截断，共 N 字符）」留痕（D10）；恰 500 字符不截断 | 新增 |
| store_test · list_change_steps | 正向 | 多 run 多步骤追加 → 按 change 全量时间序枚举；run_id 传 Some 圈定单 run 序列（AC-5 按 run 可枚举半边） | 新增 |
| store_test · list_change_steps | 边界 | 无任何步骤 → 空 vec（不 Err） | 新增 |
| store_test · list_phase_records | 正向 | 多相位多 attempt 交错落账 → 相位行序列 + checklist 内联重组按打包键序（与直查 ChecklistItemRecord 序一致） | 新增 |
| store_test · 信封零改动覆盖 | 边界 | workspace 组 4→8 后既有信封浏览路径（`scan` / 记录查询）零改动可读四新模型记录（AC-1 `list_models` / `scan` 半边；命令面经 commands/db 既有用例族持衡） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | tempfile Env / StoresEnv 真实 redb workspace db（进程边界真实组合，既有装置沿用）；中途失败注入经写命令入参构造非法 checklist 行触发事务回滚，不经 mock | 本节全部用例 |

### packages/desktop/src-tauri/crates/infra/store/src/change_port.rs -> packages/desktop/src-tauri/crates/infra/store/src/change_port_test.rs

<!-- 挂 AC-2（port 缝适配器半边：记录 ↔ 中性类型映射委托、StoreError → StoreFault
     映射；design D2「映射收 store 内单点」的执行证据）。 -->

#### 待测功能

<!-- design.md 公共函数 / API 表未声明该文件具名导出（`impl ChangeStateStore for Store`
     纯 trait 委托胶水，无新命名导出）；被测面为 trait 十一方法在真 Store 上的
     中性类型映射与故障翻译。 -->

- `impl ChangeStateStore for Store`: 读半边（get_change / list_change_records / list_phase_records / list_steps）与写半边（create_change_record / start_phase / log_phase / apply_backtrack / amend_decision_session / set_archived / append_step）的记录 ↔ 中性类型映射委托

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| change_port_test · trait object 全链映射 | 正向 | `&dyn ChangeStateStore` 类型擦除驱动建档 → start_phase → log_phase（含 checklist）→ list_phase_records：中性类型快照（ChangeStateRecord / PhaseStateRecord 含 checklist 内联）与直调 Store 方法结果逐字段一致（映射单点无加工——D2） | 新增 |
| change_port_test · 写命令翻译 | 正向 | PhaseLogCommand / BacktrackCommand / StepCommand 经 trait 入口落库，与 Store 原生方法直调落库行逐字段等值 | 新增 |
| change_port_test · StoreFault 映射 | 异常 | 同名 active 冲突 → `StoreFault::Conflict`；amend 无条目 / set_archived miss → `StoreFault::NotFound`；错误面三分支（Db / Conflict / NotFound）与 StoreError 一一对应不串型 | 新增 |
| change_port_test · trait 边界 | 边界 | `dyn ChangeStateStore` Send + Sync 边界编译锚定（Arc 装入组合根 / LocalToolSteps 可达）；get_change 对未建档名返回 Ok(None)（None = 文档形态契约） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | tempfile 真实 workspace db + 真实 Store（被测主体即真件；类型擦除仅 `&dyn` 强转，零假件） | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next_test.rs

<!-- 挂 AC-2（读源 port 化、路由语义逐项对照保持半边）、AC-5（重启续走 / 锚点基线
     平移 PhaseRecord 行数半边）。既有 phase_next_test.rs 为扩展节：既有 workflow.json
     夹具装置换血为假件 / 真件 store 种子装置，路由断言逐条持衡迁移。 -->

#### 待测功能

- phase_next(store: &dyn ChangeStateStore, change, run_id, anchors: &SessionAnchors) -> Result<PhaseNextOutcome, String>: 只读路由（读源 db；prompt 插值 + 白名单 + 重试上限 + backtrack 检测随行）
- SessionAnchors: 进程内会话锚点（(change, run_id) 键；基线自 eval 条目数平移为 PhaseRecord 行数——D8；类型与位置不变）
- LastResult.timestamp: `Option<i64>`（自 `Option<OffsetDateTime>` 演进）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| phase_next_test · 初始路由 | 正向 | 无建档→显式 `Err`；建档零相位行 → 首相位 proposal、round=1、executor / evaluator prompt 插值与白名单随行（语义对照既有断言迁移） | 新增 |
| phase_next_test · 推进与收口 | 正向 | 最近条目 pass → 下一相位；全相位 pass 走完 → done=true 且 next_phase=None（AC-5 续走判定半边） | 新增 |
| phase_next_test · fail 重试 | 正向 | 同相位连续 fail → round 递增重入同相位；达 `MAX_RETRY_TIMES` → `error=MaxRetriesExceeded{phase, round}`（决策分叉触发点保留） | 新增 |
| phase_next_test · backtrack 路由 | 正向 | 回跳字段落库后（backtrack_to 在场）→ 路由至目标相位重开 | 新增 |
| phase_next_test · 读源 db | 正向 | 真实 tempfile Store 种子（store 操作面落账）驱动路由，全程零 workflow.json 读取（AC-3 双向墙读半边随动） | 新增 |
| phase_next_test · 参数与故障 | 异常 | run_id 空白 → `Err(missing_run_id)`（保留）；假件 store 注入 `StoreFault` → `Err` 串显式失败不静默 | 新增 |
| phase_next_test · 锚点基线平移（D8） | 边界 | 首见锚点 = 该 change PhaseRecord 行数 → 首轮 round=1；条目增长后复见 → round = 窗口条目数 + 1（mid-phase interruption 窗口比对语义等价既往首见） | 新增 |
| phase_next_test · 锚点基线平移（D8） | 边界 | 新 SessionAnchors 实例（模拟重启）铸新锚点：已 pass 相位行在场 → 路由直接推进不重头执行（AC-5 重启不重跑半边） | 新增 |
| phase_next_test · LastResult 演进 | 边界 | last_result.timestamp 出 i64 millis 直透（db 时间戳原样，ISO 转换不在此层）；条目无时间戳形态不再存在（PhaseRecord.timestamp 恒在——`Option<i64>` 收窄为恒值兼容保留） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| ChangeStateStore（注入依赖入参） | 进程内假件实现 trait（design D1 fake port 先例）：可编程记录序列与 `StoreFault` 故障注入 | 路由语义 / 故障传播各 describe |
| workspace db（进程边界） | 组合用例行用真实 tempfile Store + store change 域操作面种子（workflow dev-dep store，不经 mock） | 「读源 db」「锚点基线平移」行 |

### packages/desktop/src-tauri/crates/core/workflow/src/write/phase_start.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/phase_start_test.rs

<!-- 挂 AC-2（开相写操作 port 落库半边：表位校验前置保留、持久化经 port）。既有
     phase_start_test.rs 为扩展节（workflow.json 夹具装置换血）。 -->

#### 待测功能

- phase_start(store: &dyn ChangeStateStore, change, phase) -> Result<PhaseStartOutcome, String>: 开相（表位校验保留在 core；持久化经 `store.start_phase`，attempt 事务内推导）
- PhaseStartOutcome.start_at: `i64`（自 `OffsetDateTime` 演进）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| phase_start_test · 开相落库 | 正向 | 合法表位开相 → `store.start_phase` 落 active_phase；outcome.attempt / start_at（i64）与 db active_phase 一致 | 新增 |
| phase_start_test · 表位校验保留 | 异常 | 相位不在相位表 / 与路由期望不符 → `Err` 且 store 零写入（校验前置——假件记录零写命令） | 新增 |
| phase_start_test · 故障传播 | 异常 | 假件 store 注入 `StoreFault` → `Err` 记因传播不静默 | 新增 |
| phase_start_test · 重开 attempt | 边界 | start → log 清位 → 再 start 同相位 → attempt=2（自 db 推导）；start_at 出 i64 millis 直透 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| ChangeStateStore（注入依赖入参） | 进程内假件（记录写命令与调用序、可编程 `StoreFault`） | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/workflow/src/write/phase_log.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/phase_log_test.rs

<!-- 挂 AC-2（落账写操作 port 落库半边：verdict 推导 / report 长度 / 表位 /
     active_phase 匹配校验前置保留；七操作代表行见验收范围 AC-2 同上行）。既有
     phase_log_test.rs 为扩展节（workflow.json 解析夹具换血为 store 种子）。 -->

#### 待测功能

- phase_log(store: &dyn ChangeStateStore, change, input: &PhaseLogInput) -> Result<PhaseLogOutcome, String>: 落账（`PhaseLogInput` / `PhaseLogOutcome` 形状不变；checklist 信封与会话槽位随行）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| phase_log_test · pass 落账 | 正向 | checklist 全 pass → verdict=pass；PhaseLogCommand 经 `store.log_phase` 落库，outcome.phase / attempt 与命令一致；active_phase 清位随行（既有断言迁移到 db 读侧） | 新增 |
| phase_log_test · fail 落账 | 正向 | 含 fail 项 → verdict=fail（verdict 推导前置保留） | 新增 |
| phase_log_test · 校验前置保留 | 异常 | report 超 2000 字符 → `Err` 且零写命令下发（长度校验保留）；恰 2000 字符 → 通过（边界值） | 新增 |
| phase_log_test · 校验前置保留 | 异常 | active_phase 缺失 / 相位名不匹配（表位）→ `Err` 且零写（校验保留在 core，不在 store） | 新增 |
| phase_log_test · 故障传播 | 异常 | 假件 store 返回 `StoreFault::Conflict`（重复 attempt）→ `Err` 记因传播 | 新增 |
| phase_log_test · 形状随行 | 边界 | checklist 空 vec（verdict 推导与既有语义一致）、skipped=true、三会话槽位 None / Some——`PhaseLogInput` 形状不变逐字段落库透传 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| ChangeStateStore（注入依赖入参） | 进程内假件（捕获 PhaseLogCommand 逐字段比对 + 可编程 `StoreFault`）；零残留原子性证据归 store_test 真件节 | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/workflow/src/write/backtrack.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/backtrack_test.rs

<!-- 挂 AC-2（回溯写操作 port 落库半边：白名单二次校验 / 表位 / reason ≤500 保留；
     stale 闭包计算自 persist 迁入本文件——迁移后计算断言在本节承载）。既有
     backtrack_test.rs 为扩展节。 -->

#### 待测功能

- backtrack(store: &dyn ChangeStateStore, change, input: &BacktrackInput) -> Result<BacktrackOutcome, String>: 回跳（`BacktrackInput` / `BacktrackOutcome` 形状不变；stale 闭包 = 目标最新 pass + `dependents` BFS 全条目，随 `BacktrackCommand.stale_dependents` 下发）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| backtrack_test · 回跳落库 | 正向 | 白名单内目标回跳 → BacktrackCommand（to / reason + stale_dependents）经 `store.apply_backtrack` 落库；outcome 与命令一致 | 新增 |
| backtrack_test · stale 闭包计算（迁入） | 正向 | `dependents` BFS 闭包：目标相位之后全部依赖相位条目收齐且仅收依赖闭包（表依赖链多支时逐支核对）；目标为末端相位 → 空闭包 | 新增 |
| backtrack_test · 校验前置保留 | 异常 | 目标不在白名单 → `Err` 零写；reason 超 500 → `Err` 零写（校验保留） | 新增 |
| backtrack_test · 故障传播 | 异常 | 假件 store 注入 `StoreFault` → `Err` 传播（stale 翻转与回跳同事务由 store 节承载） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| ChangeStateStore（注入依赖入参） | 进程内假件（捕获 BacktrackCommand 与 stale_dependents 闭包逐项比对 + 可编程故障）；`phase_table` 真实组合（不变组件零 mock） | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/workflow/src/write/decision_log.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/decision_log_test.rs

<!-- 挂 AC-2（决策挂账写操作 port 落库半边；D9 amend 幂等语义不变）。既有
     decision_log_test.rs 为扩展节（持久化面断言自 workflow.json 读侧迁 db 读侧）。 -->

#### 待测功能

- decision_log(store: &dyn ChangeStateStore, change, phase, session_id) -> Result<DecisionLogOutcome, String>: 决策槽位 amend 定点改写该相位最新条目（幂等覆写；无条目 fault）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| decision_log_test · amend 定点改写 | 正向 | 该相位最新 PhaseRecord decision 槽位改写为 session_id（多 attempt 在场时仅最新条目被改——定点锚定语义保持） | 新增 |
| decision_log_test · 幂等（D9） | 边界 | 重复挂账同 session_id → 幂等覆写不报错、值不重复追加 | 新增 |
| decision_log_test · 无条目 | 异常 | 该相位无任何条目 → `Err` 显式（`StoreFault::NotFound` 面） | 新增 |
| decision_log_test · 故障传播 | 异常 | 假件 store 注入 `StoreFault` → `Err` 传播；不做表位校验、不新增条目语义保持 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| ChangeStateStore（注入依赖入参） | 进程内假件（可编程条目序列与故障）；定点语义证据归 store_test 真件节 | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/workflow/src/write/create.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/create_test.rs

<!-- 挂 AC-6（create 三合一双写半边：D5 建档先行 + 补偿；kebab-case / goal 校验与
     Layout 路径纪律保留）。既有 create_test.rs 为扩展节（workflow.json 初始文档断言
     废弃、建档断言新增）。 -->

#### 待测功能

- create(layout: &Layout, store: &dyn ChangeStateStore, name, goal) -> Result<CreateOutcome, String>: 目录 + explore.md（goal 原文）+ db 建档三合一（D5：冲突双检查全 IO 前置 → 建档先行 → fs 写出 → fs 失败补偿删本次建档；补偿亦失败 Err 呈现残留名）
- CreateOutcome.created: 改取 db `created_at` 日期

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| create_test · 三合一成功 | 正向 | 成功 → 目录创建 + explore.md 含 goal 原文 + ChangeRecord 建档（workflow_type 随表）；目录树内零 workflow.json 产出（双向墙写半边——AC-6） | 新增 |
| create_test · 冲突双检查前置 | 异常 | 目录已存在 → 显式 `Err` 且 db 零建档（零副作用）；db 已有同名 active → 显式 `Err` 且零目录创建（检查全 IO 前置——D5） | 新增 |
| create_test · 校验保留 | 异常 | 非法名（kebab-case 违例 / 非单分量）与空白 goal → `Err` 且零目录零建档（既有校验持衡） | 新增 |
| create_test · fs 失败补偿（D5） | 异常 | 预置 explore.md 位置为目录（真实 fs 注入写失败）→ 补偿删除本次新插建档记录（独立写事务仅删自插行）、Err 记因；重开 db 零本次残留 | 新增 |
| create_test · 补偿再失败 | 边界 | 假件 store 补偿删除返回 `Err` → `Err` 呈现残留记录名（不静默自愈——D5 双故障角落显式面） | 新增 |
| create_test · created 出线 | 边界 | CreateOutcome.created 取 db created_at 日期（UTC 日界口径）；成功后立即经 list / detail 可见可发起（AC-6「成功即可见可发起」半边，组合行） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| ChangeStateStore（注入依赖入参） | 假件仅用于「补偿再失败」双故障注入行；其余行真实 tempfile Store（fs 与 db 均真实组合） | 补偿再失败行用假件；其余全节真件 |
| 文件系统（进程边界） | 真实 tempdir + 预置目录占位注入写失败（不经 mock） | fs 失败补偿行 |

### packages/desktop/src-tauri/crates/core/workflow/src/write/archive.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/archive_test.rs

<!-- 挂 AC-7（归档双写半边：D6 顺序与续半边分支；白名单外的第三条 fs+db 混合路径）。
     新建测试文件（沿 write 面 *_test.rs 共置先例）。 -->

#### 待测功能

- archive(layout: &Layout, store: &dyn ChangeStateStore, change) -> Result<ArchiveOutcome, String>: 先目录改名（active → archive 树 `YYYY-MM-DD-<name>`，目标已存在先查拒绝）后 db 翻转（status=archived + archived_at）；改名成功翻转失败 → Err 半完成态，重试经「archive 树命中 + db 仍 active」仅补 db 翻转（D6）
- ArchiveOutcome { name, archived_date }: UTC `YYYY-MM-DD`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| archive_test · 双写成功 | 正向 | 建档 + 磁盘目录在场 → 目录改名入 archive 树（日期前缀）+ db status=archived / archived_at 落库；主键 name 不变；ArchiveOutcome.archived_date 为 UTC 当日（AC-7） | 新增 |
| archive_test · 续半边重试（D6） | 正向 | 预置半完成态（目录已在 archive 树 + db 仍 active）→ 重试仅补 db 翻转，不重复改名（archive 树源目录不被二次挪动） | 新增 |
| archive_test · 无建档拒绝 | 异常 | active 树目录存在但 db 无记录 → 显式 `Err` 且零 fs 零 db 变更（拒绝先于一切变更——AC-7；双向墙下存量 CLI 目录不可经桌面归档的显式面） | 新增 |
| archive_test · 目标冲突 | 异常 | archive 树已存在 `YYYY-MM-DD-<name>` → 先查拒绝，db 零变更 | 新增 |
| archive_test · 翻转失败半完成态 | 异常 | 假件 store `set_archived` 注入 `Err` → `Err` 呈现半完成态；目录已改名事实由读侧呈现（重试路径可达） | 新增 |
| archive_test · 定位与边界 | 边界 | 同名目录同时存在于 active 与 archive 树 → active 精确名优先；archive 树无日期前缀同名目录（外部手工挪入）不识别为续半边对象 → 常规 `Err` 人工处置（design 实现期注意行） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| ChangeStateStore（注入依赖入参） | 假件仅用于「翻转失败」故障注入行（可编程 `set_archived` Err）；其余行真实 tempfile Store + 真实 tempdir fs（双载体全真实组合） | 翻转失败行用假件；其余全节真件 |
| 文件系统（进程边界） | 真实 tempdir Layout（active / archive 树真实改名） | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/workflow/src/queries/mod.rs -> packages/desktop/src-tauri/crates/core/workflow/src/queries/mod_test.rs

<!-- 挂 AC-7（archive 树日期前缀后缀定位半边——归档条目按月分组可达的路径推导根基）。
     新建测试文件（queries 定位面此前无独立共置测试；沿 queries 共置先例）。 -->

#### 待测功能

- locate_change(layout: &Layout, name) -> Option<ChangeLocation>: active 树精确名命中 + archive 树日期前缀后缀匹配（db 名 `foo` ↔ `YYYY-MM-DD-foo`）；单分量名校验保留

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| mod_test · active 精确名 | 正向 | active 树同名目录 → 命中 ChangeLocation（source=active 口径不变） | 新增 |
| mod_test · archive 前缀后缀匹配 | 正向 | archive 树 `2026-10-06-foo` → db 名 `foo` 命中（归档后按月分组可达的推导半边——AC-7）；多日期前缀同后名取最新 | 新增 |
| mod_test · 名校验保留 | 异常 | 多分量名（含 `/` / `..` 路径穿越形态）与空白名 → None（单分量校验持衡） | 新增 |
| mod_test · 双树同名 | 边界 | 同名同时在 active 与 archive 树 → active 精确命中优先；两树均未命中 → None | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 真实 tempdir Layout 目录树（纯路径推导，零进程边界依赖） | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/workflow/src/queries/list.rs -> packages/desktop/src-tauri/crates/core/workflow/src/queries/list_test.rs

<!-- 挂 AC-4（db ∪ 磁盘并集 / 同名 db 优先 / 按月分组半边）、AC-3（文档形态入列半边）、
     AC-7（D7 读时以磁盘事实归组半边）。既有 list_test.rs 为扩展节：workflow.json 代际
     夹具装置换血为 db 种子 + 磁盘目录装置，inventory / unparsable 断言废弃。 -->

#### 待测功能

- list_changes(layout: &Layout, store: &dyn ChangeStateStore) -> ChangeList: db 记录 ∪ 磁盘目录去重并集（同名 db 优先）；`ChangeSummary` 删 `inventory` / `unparsable`、增 `status: Option<ChangeStatus>` / `active_phase`；按月分组（db 取 `archived_at`，磁盘回退目录前缀，无前缀入未知时间组）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| list_test · 并集与去重 | 正向 | db 建档条目 + 磁盘-only 条目同时入列，同名只出现一次且取 db 形态（status / active_phase 状态面在场——AC-4） | 新增 |
| list_test · 文档形态入列 | 正向 | 磁盘目录在场 db 缺记录（存量 CLI change）→ 照常入列，status=None / active_phase=None（AC-3 / AC-4 文档形态半边） | 新增 |
| list_test · 按月分组 | 正向 | db 归档条目按 archived_at 分组；磁盘 archive 条目按目录日期前缀分组；组间新月份在前、组内新名在前（既有断言持衡迁移） | 新增 |
| list_test · D7 读时对账 | 边界 | 外部 CLI 归档（目录已改名）而 db 仍 active → 以磁盘事实归入 archive 月组，查询路径不回写 db（读后重查 db status 仍 active——纯读纪律）；反向（db archived、目录仍在 active 树）→ 按 active 归组 | 新增 |
| list_test · 未知时间组 | 边界 | archive 目录无日期前缀 → 入未知时间组不丢弃；空 db + 空磁盘 → 空列表零组 | 新增 |
| list_test · 既有守卫持衡 | 边界 | active 扫描不把 archive 目录本身误当名为 archive 的 active change；name 语义 = 磁盘目录名（归档条目含日期前缀） | 新增 |
| list_test · 退役断言 | 废弃 | 条目 `inventory` 代际标注断言（V0/V1/V2）与 `unparsable` 字段断言随字段删除废弃（AC-8 后端半边） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 真实 tempdir 磁盘目录树 + 真实 tempfile Store（种子经 store change 域操作面：建档 / 归档翻转），fs 与 db 双真实组合 | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs -> packages/desktop/src-tauri/crates/core/workflow/src/queries/detail_test.rs

<!-- 挂 AC-4（9 站流水线重组 / 文档形态空流水线 / 时间出线 ISO+null golden 守卫半边）、
     AC-3（存量 CLI change 详情可读产物、零 workflow.json 读取半边）。既有 detail_test.rs
     为扩展节：workflow.json 解析夹具换血为 db 种子，inventory / unparsable / fileLog
     断言废弃。 -->

#### 待测功能

- change_detail(layout: &Layout, store: &dyn ChangeStateStore, name) -> Option<ChangeDetail>: PhaseRecord / ChecklistItemRecord 重组 9 站流水线；`ChangeDetail` 删 `inventory` / `unparsable` / `file_log` 三字段；`AttemptRecord` 形状不变（槽位三列直读透出 None → null）；时间出线 ISO 串 + null 收本层单点（millis → RFC3339）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| detail_test · db 重组流水线 | 正向 | store 种子（多相位多 attempt + checklist）→ 9 站流水线重组：attempt 升序、checklist 随行、槽位三列直读、stale / backtrack 字段透出（AttemptRecord 形状不变——AC-4） | 新增 |
| detail_test · 时间出线单点 | 正向 | created_at / start_at / timestamp 的 i64 millis → RFC3339 ISO 串；None → null（冻结契约半边，golden 守卫绿的前提断言——AC-4） | 新增 |
| detail_test · 文档形态 | 正向 | 磁盘目录在场（含 workflow.json 惰性字节样本）db 缺记录 → 空流水线 + 产物清单，字节零进投影；全程无 workflow.json 读取（AC-3 详情半边） | 新增 |
| detail_test · 槽位全缺 | 边界 | PhaseRecord 三槽位 None → AttemptRecord 三值 null 不报错（既有语义持衡）；start_at=None → null（AC-9 槽位全缺语料面单点随动） | 新增 |
| detail_test · 边界换算 | 边界 | millis → RFC3339 边界（epoch 0 → `1970-01-01T00:00:00Z` 口径）；转换收 queries 单点（写面 / store 层无 ISO 出线） | 新增 |
| detail_test · 未找到 | 异常 | 两树均无目录且 db 无记录 → `None`（不 Err 不虚构） | 新增 |
| detail_test · 退役断言 | 废弃 | `inventory` / `unparsable` / `fileLog` 三字段断言与「workflow.json 无法解析」分支断言随字段删除废弃（AC-9 golden diff 删除范围的进程内对应面） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 真实 tempdir 磁盘树（产物文件 + 惰性 workflow.json 字节样本）+ 真实 tempfile Store 种子；golden 快照文件真实读写（DESKTOP_GOLDEN_REWRITE 开关仅测试装置） | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/workflow/src/artifacts/registry.rs -> packages/desktop/src-tauri/crates/core/workflow/src/artifacts/registry_test.rs

<!-- 挂 AC-4（产物发现输入面演进的 detail 产物清单半边）、AC-3（文档形态产物发现
     半边）。既有 registry_test.rs 为扩展节：`(Inventory, Option<&Workflow>)` 入参装置
     换血为 `&[PhaseStateRecord]` 切片装置，matcher 注册表与信封机制断言持衡。 -->

#### 待测功能

- discover_artifacts(change_dir: &Path, phases: &[PhaseStateRecord]) -> Vec<ArtifactDescriptor>: 候选枚举（文件树遍历 + 相位条目序列），输入面自 `(Inventory, Option<&Workflow>)` 演进
- read_artifact(change_dir: &Path, phases: &[PhaseStateRecord], kind, source) -> Option<ArtifactEnvelope>: 信封读出（eval-checklist 候选锚定从 `Workflow.eval` 下标平移到 PhaseRecord 序列）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| registry_test · 相位序列候选 | 正向 | 多相位多 attempt PhaseStateRecord 切片 → eval-checklist 候选 source 序号串与序列下标一一对应（序列序 = 行序，非 attempt 号——锚定平移断言）；read_artifact 按序号读出对应 checklist 信封 | 新增 |
| registry_test · 文件候选持衡 | 正向 | 磁盘 markdown / tasks 文件遍历候选、跳过点前缀、特化 kind 先于 markdown-doc 的输出序——既有断言零改动持衡 | 新增 |
| registry_test · 空切片 | 边界 | phases 空（文档形态 db 零条目）→ 仅文件候选，不 panic 不 Err（AC-3 文档形态产物清单半边） | 新增 |
| registry_test · 越界与非法 source | 异常 | 序号串越界（≥ phases.len()）→ None；路径逃逸 source 串 → None（既有守卫持衡） | 新增 |
| registry_test · 入参面退役 | 废弃 | `Inventory` / `Workflow` 入参形态断言随签名演进废弃（编译期移除的进程内对应面） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 真实 tempdir change 目录树 + 内存构造 PhaseStateRecord 切片（纯函数输入面，零进程边界） | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/orchestration/src/steps.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/steps_test.rs

<!-- 挂 AC-5（七臂命令包络 StepRecord 审计落库 / run 级 store 缝注入 / 按 run 串链半边；
     全链落库组合行）。既有 steps_test.rs 为扩展节：TempRoot workflow.json 夹具装置换血
     为 store 种子装置；FakeRunner / FakeTestExecutionRunner 假件装置沿用。 -->

#### 待测功能

- LocalToolSteps::new(anchors: Arc<SessionAnchors>, static_check: Arc<dyn StaticCheckRunner>, test_execution: Arc<dyn TestExecutionRunner>, store: Arc<dyn ChangeStateStore>, run_id: String) -> Self: run 作用域注入（组合根一次）
- 七臂命令包络 StepRecord 审计落库（成功 / 失败皆落；summary ≤500 截断留痕；reference 携 checks 报告目录 / 会话 id）；ToolStepPort 直调面不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| steps_test · 七臂审计落库 | 正向 | 假 runner 驱动七臂各一命令 → 每臂一条 StepRecord（step_kind 封闭集七值、run_id 串链、timestamp / status / summary 齐），成功臂与失败臂皆落行（AC-5 审计半边） | 新增 |
| steps_test · 全链落库组合（命令 → 写面 → db） | 正向 | 真实 Store + 真实写面：phase_log 臂驱动后 PhaseRecord 与 StepRecord 同库可查、phase_next / phase_start / backtrack 臂落库可达（AC-5「全部落库」进程内证据——链路入口组合用例） | 新增 |
| steps_test · reference 随行 | 正向 | static_check / test_execution 臂 reference 携报告目录、phase_log 臂携会话 id；无引用臂 reference=None | 新增 |
| steps_test · 摘要截断（D10） | 边界 | 臂产出摘要超 500 字符 → 落库行截断留痕；假 runner 注入超长 diagnostics 驱动 | 新增 |
| steps_test · store 故障 | 异常 | 假件 store 注入 `StoreFault` → 臂命令 `Err` 记因上抛不静默（审计失败不吞业务结果） | 新增 |
| steps_test · 直调面持衡 | 边界 | ToolStepPort 直调语义不变——既有分发臂用例族（假 runner 记录调用、Err 传播断言）零改动持衡；walker 对载体无感知（walker_test / port_test 假件随签名改写归 test-gen 承接，见节首注释） | 新增 |
| steps_test · 构造签名演进 | 废弃 | `LocalToolSteps::new` 双参构造断言随五参签名（store + run_id）废弃重写 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| StaticCheckRunner / TestExecutionRunner（注入依赖入参） | 既有 FakeRunner / FakeTestExecutionRunner 假件沿用（进程边界假件：记录调用 + 可编程产出） | 全部臂驱动用例 |
| ChangeStateStore | 组合行用真实 tempfile Store（落库证据真件）；store 故障行用进程内假件注入 `StoreFault` | 全链落库行真件；故障行假件 |

### packages/desktop/src-tauri/crates/core/orchestration/src/snapshot.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/snapshot_test.rs

<!-- 挂 AC-5（快照读源换血半边：FsSnapshot → StoreSnapshot、detail 经 queries db 读、
     WorkflowSnapshotPort 契约不变）。既有 snapshot_test.rs 为扩展节。 -->

#### 待测功能

- StoreSnapshot::new(root: String, store: Arc<dyn ChangeStateStore>) -> Self: db 读源快照（替换 `FsSnapshot::new`）
- StoreSnapshot::detail(root, change) -> Result<ChangeDetail, String>: WorkflowSnapshotPort 实现换血（经 `change_detail`；「unparsable 显式 Err」分支删除）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| snapshot_test · db 读源 detail | 正向 | store 种子（建档 + 相位行）→ detail(root, change) 与 queries::change_detail 直调结果 serde 等值（换血不加工）；文档形态（db 缺记录磁盘在场）走通不 Err | 新增 |
| snapshot_test · port 契约持衡 | 边界 | WorkflowSnapshotPort trait object 装配（walker 消费面）可达；root 不匹配 / 未知 change → `Err` 记因（port 契约不变） | 新增 |
| snapshot_test · unparsable 分支退役 | 异常 | 磁盘 workflow.json 损坏字节样本在场 → 不再显式 `Err`、零读取照常出文档形态 detail（退役回归行——AC-3 随动） | 新增 |
| snapshot_test · FsSnapshot 退役 | 废弃 | FsSnapshot 构造与 workflow.json 读源断言随实现替换废弃 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 真实 tempfile Store + 真实 tempdir 磁盘树（含惰性字节样本）；trait object 仅 `Arc<dyn ChangeStateStore>` 类型擦除 | 本节全部用例 |

### packages/desktop/src-tauri/src/commands/change_flow/mod.rs -> packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs

<!-- 挂 AC-4（`change_flow_start` 前置校验改 db 建档校验半边）、AC-5（组合根注入
     store 缝半边：LocalToolSteps / StoreSnapshot for_root 装配可达）。既有
     change_flow/mod_test.rs 为扩展节：workflow.json 夹具改 db 种子。 -->

#### 待测功能

- change_flow_start 前置校验: 校验 1（change 存在且可编排）与校验 2（workflow_type 有相位表）改读 db——`find_change_record` 命中 + `phase_table` 非 None；校验 3（无并行 run）与空白 root / change 校验保留
- 组合根: 向 `LocalToolSteps` / `StoreSnapshot` 注入 `for_root` store（run 发起链装配缝）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| change_flow_test · 建档校验通过 | 正向 | db 种子建档（requirement）→ 校验通过进入 run 发起（组合根 store 缝装配后驱动链可达——AC-5 半边） | 新增 |
| change_flow_test · 无建档拒绝 | 异常 | 存量 CLI change（workflow.json 在场、db 零记录）→ 显式 `Err` 记因拒绝（AC-4 半边；双向墙用户路径留痕） | 新增 |
| change_flow_test · 相位表校验保留 | 异常 | db 有档但 workflow_type 无相位表（phase_table None）→ 显式 `Err`（校验 2 语义平移） | 新增 |
| change_flow_test · 参数校验持衡 | 异常 | 空白 root / 空白 change → `Err`（既有断言持衡） | 新增 |
| change_flow_test · 并行 run 与其余命令 | 边界 | 校验 3（begin_run 冲突检测）持衡；stop / 应答 / 确认 / watch 命令既有用例族零改动持衡 | 新增 |
| change_flow_test · workflow.json 解析校验退役 | 废弃 | 「workflow.json 无法解析」前置校验断言随建档校验替换废弃 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 真实 tempdir root + `WorkspaceStores::open` 真实组合（种子经 store change 域操作面）；run 链假引擎 / 假 runner 沿既有进程边界假件装置 | 本节全部用例 |

### packages/desktop/src-tauri/src/commands/changes/mod.rs -> packages/desktop/src-tauri/src/commands/changes/mod_test.rs

<!-- 挂 AC-6（create 命令接线半边）、AC-7（`archive_change` 薄命令半边）、AC-4（list /
     detail 接线 db 读面半边）。既有 changes/mod_test.rs 为扩展节：夹具改 db 种子 +
     WorkspaceStores 组合，`archive_change` / `archive_change_with` 为新增命令面。 -->

#### 待测功能

- list_changes / get_change_detail / read_artifact / create_change: 接线 db 读面与建档写面（`for_root` store 入参；读命令 IPC 签名不变）
- archive_change(app, root, change) -> Result<ArchiveOutcome, String>: 新增 IPC 薄命令（三件事：参数校验 → 写面 archive → 错误映射；`archive_change_with<R: Runtime>` 测试缝）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| changes_test · 读命令透传持衡 | 正向 | list / detail 命令与 core 查询结果 serde 等值对比（既有透传无加工口径，夹具改 db 种子后重证）；read_artifact 信封出线持衡 | 新增 |
| changes_test · create 接线 | 正向 | create_change 命令 → 写面三合一；返回 DTO name + created；成功后 list / detail 立即可见（AC-6「可见可发起」命令半边） | 新增 |
| changes_test · archive 接线 | 正向 | archive_change 命令 → 写面双写成功，返回 ArchiveOutcome；随后 list 命令按月分组可达（AC-7 命令半边 + 分组可达组合行） | 新增 |
| changes_test · archive 错误映射 | 异常 | 无建档 change / 目标冲突 → `Err` 记因映射（三件事薄包装不吞错不加工）；空白 root / change 参数校验持衡 | 新增 |
| changes_test · create 错误映射 | 异常 | 同名 active 冲突 → `Err` 映射（db 零建档零目录——D5 前置检查经命令链可达） | 新增 |
| changes_test · blank root 双口径 | 边界 | 读命令空 / 空白 root 早退空结果、create 显式 `Err`——既有双口径断言持衡 | 新增 |
| changes_test · 状态面接线 | 废弃 | 命令 DTO 内 `inventory` / `unparsable` / `fileLog` 断言随字段删除废弃 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 真实 tempdir root + `WorkspaceStores` 真实组合（fs 与 db 双真实）；`*_with` 泛型测试缝直调（无 State / AppHandle 依赖的面直测，沿既有无 runtime 直调先例） | 本节全部用例 |

### packages/desktop/src-tauri/src/commands/mod.rs -> packages/desktop/src-tauri/src/bindings/mod_test.rs

<!-- 挂 AC-8（bindings 出线半边：新命令与新 DTO 类型清单补录、退役类型消失承载）。
     design.md 公共函数 / API 表未声明该源文件自有可测函数（`tauri::generate_handler`
     声明式登记单点）；本章节承载登记面清单核实的结论（沿 mod_test 三清单现行维护
     口径）：读 `src/bindings/mod_test.rs` 现状——① `COMMAND_WRAPPERS` / `COMMAND_NAMES`
     尚未收录 `archive_change` → 按现行口径补录自身新命令两条（wrapper `archiveChange` +
     invoke `archive_change`）；② `DTO_TYPES` 已在册 `ActivePhase` / `AttemptRecord` /
     `ChangeDetail` / `ChangeList` / `ChangeSource` / `ChangeSummary` / `ChecklistItem`
     （无需重复补录），本变更自有出线新类型 `ArchiveOutcome` / `ChangeStatus` 需补录；
     ③ `change_flow_*` / `create_change` 等历史缺录为既有滞后债，contains 语义不致红、
     不在本变更范围。 -->

#### 待测功能

<!-- design.md 未声明该源文件公共 API（invoke_handler 登记单点为声明式宏）；本节被测面
     为 bindings 出线断言三清单的补录行（COMMAND_WRAPPERS / COMMAND_NAMES / DTO_TYPES）。 -->

- all_commands 登记面 `archive_change` 条目: invoke_handler 与 specta builder 同源出线（登记单点）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| bindings 清单补录 · archive_change | 正向 | `COMMAND_WRAPPERS` 增 `"archiveChange"`、`COMMAND_NAMES` 增 `"archive_change"` 后，既有覆盖性用例（产物含全部命令包装名 / invoke 名）自动涵盖新命令：bindings 重导后产物含 `archiveChange` 包装条目与 `"archive_change"` invoke 名（AC-8 出线半边） | 新增 |
| bindings 清单补录 · 新 DTO | 边界 | `DTO_TYPES` 增 `"ArchiveOutcome"` / `"ChangeStatus"`（本变更自有出线新类型）→ 产物含两类型名；`ChangeStatus` 小写线值（active / archived）出线由生成物呈现 | 新增 |
| bindings 清单补录 · 在册不重录 | 边界 | `ChangeDetail` / `ChangeSummary` / `ActivePhase` / `AttemptRecord` 等已在册类型不重复补录：字段面演进（三字段删除 + 状态面 / 槽位类型跟随）由生成物幂等重导 + golden diff 守卫承载（核实结论行） | 新增 |
| bindings 清单补录 · 退役类型 | 废弃 | `Inventory` / `FileLogEntry` 镜像类型相关出线断言随生成物收缩废弃（contains 语义无缺失断言，退役由 golden diff 与前端编译双守卫） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| bindings.ts 产物文件（进程边界） | 真实组合不 mock：`export_bindings()` 真实重导 + 产物真实读取 + 产物互斥锁串行（既有装置沿用） | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/workflow/src/lib.rs -> packages/desktop/src-tauri/crates/core/workflow/tests/corpus_golden_test.rs

<!-- 挂 AC-9（db 种子语料矩阵 golden 对拍 + detail 线面字段演进显式重写流程半边）、
     AC-1（回环语义的语料级随动）。design.md 未声明 lib.rs 自有公共 API（crate 门面：
     parse 移除、state 加入）；本节承载 db 种子语料全链组合用例，链路入口 = crate 公共
     API 门面（组合用例挂靠规则；来源：proposal 测试文件清单行 + AC-9）。既有 13 个
     workflow.json 语料夹具与 golden 快照删除（D12），新语料 = db 种子构造器（经真实
     store change 域操作面，workflow dev-dep store）+ 运行时合成磁盘产物树 +
     workflow.json 惰性字节样本（desktop 不解析、字节零进投影）；golden 快照经
     `DESKTOP_GOLDEN_REWRITE=1` 显式再生成。 -->

#### 待测功能

<!-- design.md 未声明 lib.rs 自有公共 API 变更（纯门面：模块声明收敛）；本节为跨模块
     组合用例章节，被测面为 list / detail / locate 全读链 + store 操作面种子构造，
     golden 期望以显式重写流程录制。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| corpus_golden_test · 多 attempt 语料 | 正向 | 同相位两轮落账 + 重开 attempt 种子 → detail golden 重组逐字段一致（attempt 升序、checklist 打包键序、created_at / start_at ISO 出线） | 新增 |
| corpus_golden_test · backtrack stale 语料 | 正向 | 回跳 + dependents 闭包翻转种子 → stale 位与 backtrack_to / backtrack_reason 字段 golden 一致（AC-9 矩阵） | 新增 |
| corpus_golden_test · 槽位全缺语料 | 正向 | 三会话槽位全 None + start_at None 种子 → 槽位三列 null / 时间 null 的 golden 一致（冻结契约「null 留位」半边） | 新增 |
| corpus_golden_test · 文档形态语料 | 正向 | 磁盘产物树（proposal / design / tasks / reports）+ workflow.json 惰性字节样本、db 零记录 → 空流水线 + 产物清单 golden，字节零进投影（AC-9 / AC-3 矩阵） | 新增 |
| corpus_golden_test · db 坏行语料 | 异常 | 底层直写注入 native_model 解码失败行 → 读侧 StoreError 显式记因不静默（fixtures/README 矩阵「坏行」覆盖面；unparsable 警示对象不复存在的现役错误面） | 新增 |
| corpus_golden_test · 显式重写流程 | 边界 | `DESKTOP_GOLDEN_REWRITE=1` 再生成后重跑绿；快照 diff 仅限 `inventory` / `fileLog` / `unparsable` 三键删除与 `status` / `active_phase` 状态面键新增（AC-9 diff 范围的进程内断言；人工确认留痕见不可测试项 5） | 新增 |
| corpus_golden_test · 旧语料退役 | 废弃 | v0-a…v3-a / corrupt-* 13 目录 workflow.json 语料断言与对应 golden 快照随 D12 处置整体废弃删除 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 真实 tempfile workspace db（种子经 store change 域操作面真件）+ 真实磁盘产物树 + golden 快照文件真实读写；`DESKTOP_GOLDEN_REWRITE` 环境开关仅测试装置 | 本节全部用例 |

### packages/desktop/src/views/changes/change-list-view.tsx -> packages/desktop/src/views/changes/change-list-view.test.tsx

<!-- 挂 AC-8（清单退役面半边：InventoryBadge / unparsable 标注删除、status /
     active_phase 消费、文档形态条目照常入列）。既有测试为扩展节：invoke 夹具改 db
     读面 DTO 形态。 -->

#### 待测功能

<!-- design.md 公共函数 / API 表未声明该视图导出函数（React 视图组件）；被测面为
     组件渲染行为：退役元素不再渲染、状态面两态消费。 -->

- 清单视图退役面: `InventoryBadge` 与 unparsable「无法解析」标注删除；条目状态面改 `status` / `active_phase` 消费；文档形态条目照常入列

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| list-view · 徽章退役 | 异常 | invoke 返回无 `inventory` 字段的清单 DTO → V0/V1/V2 代际徽章不再渲染任何形态（退役负断言）；月分组与归档条目渲染持衡（AC-8） | 新增 |
| list-view · unparsable 标注退役 | 异常 | DTO 无 `unparsable` 字段 →「无法解析」标注与警示位不再出现（含历史损坏样本形态夹具） | 新增 |
| list-view · 状态面消费 | 正向 | `status="active"` 且 `activePhase` 在场 → 进行中相位呈现；`status="archived"` → 归档呈现；均 null（文档形态）→ 无状态位照常入列 | 新增 |
| list-view · 退役断言清理 | 废弃 | 既有代际徽章映射与 unparsable 标注断言随字段退役废弃 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC（进程边界） | `vi.mock('@tauri-apps/api/core')` 仅 mock invoke 返回 db 读面 DTO 夹具（既有 vi.hoisted 装置沿用）；视图内部 hooks（use-change-list）真实组合渲染不 mock | 本节全部用例 |

### packages/desktop/src/views/changes/change-detail-view.tsx -> packages/desktop/src/views/changes/change-detail-view.test.tsx

<!-- 挂 AC-8（详情退役面半边：代际徽章映射 / UnparsableNote / WorkflowPanel 删除、
     建档两态分流）。既有测试为扩展节。 -->

#### 待测功能

<!-- design.md 未声明该视图导出函数（React 视图组件）；被测面为组件渲染行为：
     退役元素不再渲染、建档两态分流（status 在场与否）。 -->

- 详情视图退役面: 代际徽章映射、`UnparsableNote`、`WorkflowPanel` 删除；两态分流改建档判别；error / loading / 未找到降级页沿用

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| detail-view · 退役元素 | 异常 | invoke 返回删三字段后的 detail DTO → 代际徽章、「workflow.json 无法解析」警示条、workflow 独立面板（WorkflowPanel）均不再渲染（退役负断言——AC-8） | 新增 |
| detail-view · 两态分流 | 正向 | `status` 在场 → 完整状态面（9 站流水线 + 抽屉）；status 缺（文档形态）→ 降级为产物清单面，不崩溃不空白 | 新增 |
| detail-view · 降级页持衡 | 边界 | error / loading / 未找到三降级页既有断言零改动持衡；抽屉打开入口（单一交互入口）持衡 | 新增 |
| detail-view · 退役断言清理 | 废弃 | 徽章映射表断言、UnparsableNote 渲染断言、fileLog 面板挂载断言随退役废弃 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC（进程边界） | `vi.mock('@tauri-apps/api/core')` 仅 mock invoke + Channel 返回 detail DTO 夹具（既有装置沿用）；内部 hooks 真实组合 | 本节全部用例 |

### packages/desktop/src/views/changes/flow/detail-drawer.tsx -> packages/desktop/src/views/changes/flow/detail-drawer.test.tsx

<!-- 挂 AC-8（抽屉二分节半边：文件表节删除、右列三分节 → 二分节）。既有测试为
     扩展节：detail 夹具删三字段。 -->

#### 待测功能

<!-- design.md 未声明该组件导出函数（React 组件）；被测面为分节结构：右列仅
     「本站文档 | eval report+checklist」二分节；`FileLogTable` 引用与 `hasFileLog`
     链路删除；左列会话区、双列滚动、单一交互入口不变。 -->

- 抽屉退役面: 文件表节删除、二分节结构、既有左列会话区 / 双列滚动断言持衡

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| detail-drawer · 二分节 | 正向 | 任意 detail 夹具（含历史上曾有 fileLog 的形态）→ 右列恒二分节（本站文档 / eval report+checklist），文件表节与「文件」标题不出现 | 新增 |
| detail-drawer · 持衡面 | 边界 | 左列会话区、双列滚动、单一交互入口、eval checklist 分节渲染——既有断言零改动持衡 | 新增 |
| detail-drawer · hasFileLog 链路退役 | 异常 | 任意输入下不出现文件表挂载条件分支（hasFileLog 恒假的死分支不驻留——退役负断言） | 新增 |
| detail-drawer · 退役断言清理 | 废弃 | file-log-table.test.tsx 断言族随 `flow/file-log-table.tsx` 组件删除整体废弃；本文件既有文件表节断言废弃 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC（进程边界） | `vi.mock('@tauri-apps/api/core')` 仅 mock invoke（既有 vi.hoisted 装置）；attachments / renderers 内部模块真实组合 | 本节全部用例 |

### packages/desktop/src/views/changes/flow/attachments.ts -> packages/desktop/src/views/changes/flow/attachments.test.ts

<!-- 挂 AC-8（挂载分支收缩半边：file_log 挂节点与 scope='workflow' 图外分支删除；
     文档挂列 + eval-checklist 挂节点保持）。既有纯函数测试为扩展节。 -->

#### 待测功能

- mountMaterials(...): 纯函数挂载层——file_log 挂节点分支与 `scope='workflow'` 图外素材分支删除；文档挂列映射表 + eval-checklist 挂节点保持；`nodeFiles` / `outsideFiles` 挂载状态字段退役（`FileLogEntry` 随 bindings 演进删除）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| attachments · 分支收缩 | 异常 | detail 夹具无 `fileLog` 字段（TS 类型面已无该字段）→ mountMaterials 产物零文件挂载节点、零图外素材；产物状态面无 `nodeFiles` / `outsideFiles` 键（退役负断言——编译期与运行时双锚定） | 新增 |
| attachments · 持衡面 | 正向 | 文档挂列映射（proposal / dev-design / test-execution 列）与 eval-checklist 挂节点——既有断言换夹具后重证持衡 | 新增 |
| attachments · 装置退役 | 废弃 | `fileEntry()` 装置与 `fileLog` 夹具字段、file_log 挂载断言族废弃 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 纯函数内存直驱（detail / envelope / descriptor 夹具字面量，零进程边界零 IO） | 本节全部用例 |

---

## 不可测试项

1. AC-3 静态半边：`crates/core/workflow/src/parse/` 目录（detect / workflow_file 及测试）与 `write/persist.rs`、`model/workflow.rs`、`model/inventory.rs`、`tests/generation_parse_test.rs`、13 个 workflow.json 语料夹具及对应 golden 快照、`flow/file-log-table.tsx` 删除 — **原因**: 删除性静态约束，由 crate 编译与模块引用收敛承载（引用残留即编译失败），无进程内行为断言面；其行为半边（文档形态可见 / 详情可读产物 / unparsable 分支退役回归）已落 detail_test / list_test / snapshot_test / corpus 各节。
2. AC-3 静态半边：全 desktop 源码 workflow.json 读 / 写触点扫描（测试夹具除外） — **原因**: 全源码静态守线检查（design.md AC-3 对齐行明定归守线任务），测试夹具豁免使其无法作为单测断言全量自动化。
3. AC-2 静态半边：`crates/core/workflow/Cargo.toml` 零 infra/store 依赖审查 + 写面全函数 sync 签名 — **原因**: 依赖方向是 Cargo 依赖图编译期约束（workflow 不依赖任何 infra 即不可越线，AC-2 硬约束的承载即编译），sync 签名为编译期形态；port 缝行为面经七个写操作节与 change_port_test 承载。
4. AC-8 静态半边：`pnpm -C packages/desktop run client:check`（fmt / lint / knip 零新增豁免）与 `vp test`、`cargo test --workspace` 全绿 — **原因**: 工具链级套件门，非单一测试文件可承载，归 test-execution 阶段承接验证（design.md AC-8 对齐行同口径）；退役行为断言经四个前端节承载。
5. AC-9 流程半边：golden 重写 diff 的人工确认与留痕 — **原因**: 流程性动作（人工审阅 diff 范围并留痕），不可自动化；其测试半边（重写后重跑绿 + diff 范围键集断言）已落 corpus_golden_test.rs 节。
6. AC-10 版本交付：`packages/desktop/package.json` version 0.4.12 → 0.4.13（归档时执行，`src-tauri/Cargo.toml` 不随动） — **原因**: 静态版本声明（proposal 明定归档时执行），无行为面可自动化。
7. 门面 / 纯类型 / 生成物 / 文档模块不建独立测试文件（`crates/core/workflow/src/lib.rs` 的 lib_test 解析路径、`write/mod.rs`、`model/mod.rs`、`model/domain.rs`、`state.rs`、`crates/infra/store/src/lib.rs` 的 lib_test 解析路径、`crates/infra/store/Cargo.toml`、`src/commands/mod.rs` 的 mod_test 解析路径、`flow/types.ts`、`types/generated/bindings.ts`、`tests/fixtures/README.md`） — **原因**: 纯 re-export / 类型声明 / 生成物 / 语料说明无自有行为，空框架章节自相矛盾且诱发空套件；行为去向：state.rs 词汇面（ChangeStateStore / 中性类型 / StoreFault）经 change_port_test 与写面各节假 port 消费承载；model/domain.rs 为形状不变的类型迁移（`Verdict` / `ChecklistItem` 导出面不变），编译期承载；bindings.ts 经 bindings/mod_test.rs 守卫 + 重导幂等 + golden diff 承载；fixtures/README.md 的 db 种子语料矩阵语义经 corpus_golden_test.rs 节承载；`src/commands/mod.rs` 登记单点经 bindings 清单补录节承载；Cargo.toml 仅增 workspace 内路径依赖一行，无行为面。
8. 多窗口 / 跨进程并发共写竞态（redb 写事务并发面） — **原因**: 跨进程并发时序无法在单进程单测中确定性复现；并发安全由 redb 单写者模型库语义保障（不逐项验证库语义——项目纪律），单事务原子性与中途失败零残留（AC-2 核心诉求）已经 store_test 真件节承载。
9. walker_test.rs / port_test.rs 既有假件随 `StoreSnapshot` / 写面签名改写 — **原因**: 纯测试代码适配（design.md 实现期注意行明定归 test-design / test-gen 阶段承接），无新增行为断言；其行为覆盖面由 steps_test（全链落库组合）与 snapshot_test（port 契约）节承载。

