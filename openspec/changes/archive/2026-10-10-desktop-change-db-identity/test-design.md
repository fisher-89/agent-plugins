# 测试设计: desktop-change-db-identity

> **变更**: desktop-change-db-identity
> **日期**: 2026-10-10
> **依据**: proposal.md（AC-1..AC-11）+ design.md（D1–D14 决策编号）；tasks.md 六阶段内的「编译面机械随动」只保编译不写断言，本文件展开全部新增 / 重写 / 适配锚（test-gen / test-execution 阶段承接）

---

## 测试边界与框架识别

<!-- 逐文件推导自 design.md 变更清单（实现文件 + 测试文件两节）；test_resolve_paths 解析出
     38 对 source → test 映射（errors 一条：routes.tsx 不在测试配置 scope → 见不可测试项 6）。
     本变更零新增测试文件、零新增 dev / 测试依赖——全部为既有 *_test.rs / *.test.ts(x) 扩展节
     （design 删除文件节：无整文件删除；list 磁盘扫描段 / detail 文档形态分支 / ChangeRecordV1
     解码链 / flow-empty 分支为段级删除）。 -->

- **Rust 面**：src-tauri cargo workspace 套件（rust 框架；src-tauri 根自带 workspace 自动覆盖），测试文件为共置 `*_test.rs` 模块与 `tests/` 语料 golden（corpus_golden_test.rs + tests/golden/*.json）。store / workflow / orchestration 与命令面共 26 个既有测试文件扩展节（另 bindings/mod_test.rs 与语料 golden 各一节，合计 28 节）。
- **前端面**：packages/desktop vite-plus 套件，共置 `.test.ts(x)`（vite-plus/test describe/it/expect），12 个既有测试文件扩展节。`routes.tsx` 无共置测试路径（test_resolve_paths 报「Not in test config scope」），路由行为断言驻 `src/app.test.tsx`（proposal 测试文件清单行）。
- **进程边界策略**：db 为进程边界——store 面真实 tempfile redb workspace db（旧形态库与坏行经裸 redb 直写字节预置）；core 写面 / 查询面以进程内假件实现 `ChangeStateStore`（注入依赖入参）+ 真实 tempdir fs 组合（workflow-db-state 先例沿用）；编排面假引擎 / 假 vcs / 假 worker 装置沿用；前端仅 mock IPC 进程边界（`@tauri-apps/api/core` 的 invoke / Channel），被测页面 / 面板 import 的内部 hooks 真实组合，fixture 经 mock IPC 流入后断言渲染输出。
- **迭代类型词表**：**新增**（新用例）/ **重写**（既有用例语义演进改写——本变更主形态：name 键断言改 id 键）/ **适配**（机械传参或 fixture 补齐，既有断言面随动）/ **持衡（沿用）**（零触点断言面）。**废弃**仅用于整条断言随架构死亡的面（`ChangeRecordV1` 解码链 / list 磁盘扫描 / detail 文档形态 / `flow-empty` 占位）。
- **golden 面**：`DESKTOP_GOLDEN_REWRITE=1` 显式重写流程启用（id 键入 golden + 列表 db 单源 + 文档形态样本退役改造 + 归档前缀样本入 golden——R6 / D13）；diff 人工确认留痕为流程性动作（见不可测试项 5）。
- **测试面 id / name 纪律**：一切身份寻址断言以固定 id 字面量为键（语料种子直携——D2）；name 仅作展示属性与磁盘 / git 面供给值断言。铸出路径断言 id 非空 / 同名单次重铸相异 / `CreateOutcome.id` 与库内逐字一致，不做固定值对拍；不逐项验证 uuid 库语义（库自带语法语义不入测——只测自研铸出 / 组装层）。

---

## 验收范围

<!-- 逐条映射 proposal.md 的 11 条 AC。被测文件或模块为承载用例的测试文件（单值）；
     跨半边 AC 以「（同上——…半边）」行展开到各自测试文件；纯静态 / 流程性半边落
     「—（见不可测试项 N）」。 -->

| AC ID | 验收条件 | 被测文件或模块 |
|--------|---------|----------|
| AC-1 | 建档 change 铸出 uuid 形态 id（主键）；name 为无唯一约束的可变属性；重开库回环读写逐字段一致 | packages/desktop/src-tauri/crates/infra/store/src/store_test.rs（id 回环主半边）；packages/desktop/src-tauri/crates/infra/store/src/model_test.rs（v3 换锚半边）；packages/desktop/src-tauri/crates/infra/store/src/change_port_test.rs（port 映射半边） |
| AC-1 | （同上——相位 / checklist / 步骤 / run 史行 change_id 归属半边；五写命令载荷 id 化） | packages/desktop/src-tauri/crates/core/workflow/src/state_test.rs；packages/desktop/src-tauri/crates/core/workflow/src/write/phase_start_test.rs / phase_log_test.rs / backtrack_test.rs / decision_log_test.rs / phase_next_test.rs / run_test.rs；packages/desktop/src-tauri/crates/core/orchestration/src/steps_test.rs（审计归键半边）；packages/desktop/src-tauri/crates/core/orchestration/src/run_history_test.rs |
| AC-1 | （同上——铸出点与信封 key 投影半边） | packages/desktop/src-tauri/crates/core/workflow/src/write/create_test.rs；packages/desktop/src-tauri/crates/infra/store/src/envelope_test.rs |
| AC-2 | 归档 change 详情（经 id 寻址）：status=archived、9 站 pipeline 与 runs 全量出线，MUST NOT 落文档形态空面；列表归档条目 name 裸名、id 恒在案 | packages/desktop/src-tauri/crates/core/workflow/src/queries/detail_test.rs（详情全状态面半边）；packages/desktop/src-tauri/crates/core/workflow/src/queries/list_test.rs（列表归档条目半边）；packages/desktop/src-tauri/crates/core/workflow/src/queries/mod_test.rs（归档目录后缀扫描半边）；packages/desktop/src-tauri/crates/core/workflow/tests/corpus_golden_test.rs（归档前缀样本 golden 半边） |
| AC-2 | （同上——写面 id → record.name 目录改名半边；命令面归档条目名断言（含 :262 段）改写半边） | packages/desktop/src-tauri/crates/core/workflow/src/write/archive_test.rs；packages/desktop/src-tauri/src/commands/changes/mod_test.rs |
| AC-3 | list_changes 源码零磁盘扫描触点；磁盘-only（CLI 建、无 db 记录）目录零呈现（不入列、不报错、目录字节零变化）；归档月分组以 archived_at 为权威 | packages/desktop/src-tauri/crates/core/workflow/src/queries/list_test.rs（零发现与月分组主半边）；packages/desktop/src-tauri/crates/core/workflow/tests/corpus_golden_test.rs（列表 golden 半边）；packages/desktop/src-tauri/src/commands/changes/mod_test.rs（命令面半边）；packages/desktop/src/views/changes/change-list-view.test.tsx（空态文案 UI 半边）；零触点 grep → 不可测试项 1 |
| AC-4 | db 无该 id 记录时详情返回「未找到」降级，MUST NOT 回退磁盘目录解析、MUST NOT 返回空流水线文档形态；flow-empty 占位分支删除（前端零「文档形态」文案消费） | packages/desktop/src-tauri/crates/core/workflow/src/queries/detail_test.rs（后端半边）；packages/desktop/src/views/changes/change-detail-view.test.tsx（前端退役半边）；packages/desktop/src-tauri/crates/core/orchestration/src/snapshot_test.rs（消费面随动半边） |
| AC-5 | `get_change_detail` / `read_artifact` / `archive_change` / change_flow 五命令 / archive_flow 五命令的定位参数均为 change id；id → 记录 → name 分辨率单点；bindings 再生成一致性守卫绿 | packages/desktop/src-tauri/src/commands/changes/mod_test.rs；packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs；packages/desktop/src-tauri/src/commands/archive_flow/mod_test.rs；packages/desktop/src-tauri/src/bindings/mod_test.rs（签面随动半边）；packages/desktop/src-tauri/crates/core/orchestration/src/port_test.rs（载荷随动半边） |
| AC-6 | run / archive 注册表键为 `(workspace root, change id)`（12 处 name 键清零）；`source_ref` 定式 `<id>/<phase>/<role>/<attempt>` 与 `<id>/archive/*`；前端反查与服务端写侧逐字一致 | packages/desktop/src-tauri/crates/core/orchestration/src/control_test.rs（run 注册表半边）；packages/desktop/src-tauri/crates/core/orchestration/src/archive_flow_test.rs（archive 注册表 + 归档两会话 provenance 半边）；packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs（run provenance 半边）；packages/desktop/src/views/changes/flow/detail-drawer.test.tsx（前端组装半边）；packages/desktop/src/views/changes/hooks/use-session-transcript.test.ts（透传持衡半边） |
| AC-7 | 路由 `/changes/:id`；列表行键 / navigate 均为 id；useChangeDetail 等 hooks 参数 id；页内单一身份源（URL id），无 `detail.name` 作寻址残留 | packages/desktop/src/app.test.tsx（路由半边）；packages/desktop/src/views/changes/change-list-view.test.tsx；packages/desktop/src/views/changes/change-detail-view.test.tsx；packages/desktop/src/views/changes/hooks/use-change-detail.test.ts；packages/desktop/src/views/changes/hooks/use-change-flow-run.test.ts；packages/desktop/src/views/changes/hooks/use-archive-flow.test.ts；packages/desktop/src/views/changes/hooks/use-change-list.test.ts；packages/desktop/src/views/changes/flow/run-control-panel.test.tsx；packages/desktop/src/views/changes/flow/archive-panel.test.tsx；packages/desktop/src/views/changes/components/change-create-dialog.test.tsx |
| AC-8 | 打开含 name 主键形态数据 / 缺版本标记的存量 workspace 库：打开成功、旧数据零可达（重建为空库）、零迁移代码路径、零报错；全新库版本标记就位且重开不触发作废；全局库 workspace 注册表不受波及 | packages/desktop/src-tauri/crates/infra/store/src/store_test.rs（作废重建与表名版本段主半边）；静态半边（v1 解码链删除 / open_global 零触碰）→ 不可测试项 2 |
| AC-9 | 语料种子 id 归键、文档形态样本退役；list / detail golden 经 `DESKTOP_GOLDEN_REWRITE=1` 显式重写（含 id 字段与 db 单源列表）；bindings:check 绿 | packages/desktop/src-tauri/crates/core/workflow/tests/corpus_golden_test.rs（主半边）；packages/desktop/src-tauri/src/bindings/mod_test.rs（bindings 守卫半边）；重写 diff 人工确认 → 不可测试项 5 |
| AC-10 | 全管线回归：rust workspace 套件 / 前端工程检查与 client:check（含 knip）/ bindings 一致性守卫 / 前端套件全绿，无新增豁免条目 | —（见不可测试项 3） |
| AC-11 | 归档时 `packages/desktop/package.json` version 0.4.30 → 0.4.31；`plugins/dev-team` 零改动 | —（见不可测试项 4） |

---

## 单元测试

<!-- 覆盖所有可在进程内验证的场景（Rust #[test] / #[tokio::test]、前端 vite-plus 纯函数 / 组件 mock 测试）。
     每个源文件对应一个独立的 `### <源文件> -> <测试文件>` 章节（均为既有测试文件的扩展节，
     本变更零新增测试文件）；跨模块组合用例（语料全链、create 组合、命令回环）colocate 到链路
     入口模块（corpus_golden_test / create_test / mod_test）的 `#### 用例` 表，describe 标题写链路方向。
     门面 / 纯类型 / 生成物 / 配置文件（bindings.ts、flow/types.ts、Cargo.toml ×2、package.json、
     routes.tsx 的解析错误面）不建独立测试文件，统一落「不可测试项」声明（含行为去向）。
     迭代类型词表见上节：新增 / 重写 / 适配 / 持衡（沿用）/ 废弃。 -->

### packages/desktop/src-tauri/crates/infra/store/src/model.rs -> packages/desktop/src-tauri/crates/infra/store/src/model_test.rs

<!-- 挂 AC-1（v3 换锚半边）、AC-8（表名版本段半边）。design.md 公共函数 / API 表未声明该文件
     模块级导出函数（纯记录模型 + 构造器 + 常量）；待测面为类型定义表行（ChangeRecord v3 /
     StoreMetaRecord / PhaseRecord·StepRecord·RunRecord v2）与 WORKSPACE_STORE_FORMAT_VERSION。
     native_model 编解码自身语义不逐项验证（库语义），只测自研字段面 / 注册面 / 版本段。 -->

#### 待测功能

<!-- design.md 未声明该文件模块级导出函数；被测面为类型定义表行与常量。 -->

- ChangeRecord v3: `id: String` 主键（身份锚）+ `name: String` 普通属性（无唯一约束，恒裸名）；`new` 构造携 id 入参；其余字段面（workflow_type / created_at / status / archived_at / active_phase / worktree / base_commit）逐字不变
- StoreMetaRecord: `key: String` 主键（恒 `"format"`）+ `format_version: u32`；`WORKSPACE_STORE_FORMAT_VERSION = 2` 常量（探测判定源——D3）
- PhaseRecord / StepRecord / RunRecord v2: `change_id: String` 二级索引列（指向 `ChangeRecord.id`）
- `ChangeRecordV1` 与双向 `From` 删除（v1 解码链退役面）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| model_test · ChangeRecord v3 回环 | 正向 | id 主键 + name 属性 + worktree / base_commit 双态构造 → encode / decode 往返逐字段相等（版本头 = 3；v2 回环行重写） | 重写 |
| model_test · ChangeRecord 身份面 | 边界 | id 与 name 双字段独立可辨：同 name 不同 id 两条记录构造合法且不相等（name 非身份键——D11 模型面锚） | 新增 |
| model_test · new 构造携 id | 边界 | `new(id, name, workflow_type, created_at, worktree, base_commit)` 构造逐字段对位；status 恒 active 起步 / archived_at / active_phase 空起步持衡 | 重写 |
| model_test · 表名版本段 | 正向 | native_model id / version 断言：ChangeRecord 9:v3、PhaseRecord 10:v2、StepRecord 12:v2、RunRecord 13:v2、StoreMetaRecord 15:v1（`{id}_{version}_{key}` 表名公式锚——与 store_test 常量同源） | 新增 |
| model_test · v2 记录回环适配 | 正向 | PhaseRecord（change_id）/ StepRecord（change_id）/ RunRecord（change_id）往返逐字段保真；ChecklistItemRecord 11 / RunStepRecord 14 维持 v1 零改动 | 适配 |
| model_test · StoreMetaRecord 往返 | 正向 | key="format" + format_version=WORKSPACE_STORE_FORMAT_VERSION 编解码往返相等（单键固定形态） | 新增 |
| model_test · 十一模型注册 | 边界 | workspace 组注册 10 → 11：id 9 / 10 / 11 / 12 / 13 / 14 / 15 与既有 id（1/2/4/5/6/7/8）无冲突；`list_models` 零改动覆盖 | 重写 |
| model_test · v1 解码链退役 | 废弃 | `change_record存量v1行经版本机制升级读出且两字段置none`、`change_record_v1_from双向upgrade补none与downgrade丢新字段` 随 `ChangeRecordV1` 与双向 From 删除整体废弃（旧形态数据去向 = 不可达或整体丢弃，零 decode 路径——D5） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 纯内存构造 + native_model 编解码往返（零 IO 零进程边界） | 本节全部用例 |

### packages/desktop/src-tauri/crates/infra/store/src/store.rs -> packages/desktop/src-tauri/crates/infra/store/src/store_test.rs

<!-- 挂 AC-1（id 回环 / change_id 操作面）、AC-8（作废重建 / 标记幂等 / 表名版本段）。
     既有 Env / StoresEnv tempfile 装置沿用；旧形态库与坏行经裸 redb 直写字节预置
     （inject_* 先例同式）。既有 change / run 域用例为适配主体（形参 name → id），
     作废重建与表名版本段为新增断言面。 -->

#### 待测功能

- Store::create_change_record(record: ChangeStateRecord) -> Result<ChangeStateRecord, StoreError>: 同 id 防御拒绝（Conflict，不静默覆写）；零同名检查（D11——同名 active 拒绝归写面前置）
- Store::find_change_record(id) / delete_change_record(id) / set_change_archived(id, archived_at): id 形参
- Store::start_change_phase(change_id, phase, now) / amend_change_decision_session(change_id, phase, session_id): id 形参（attempt 推导扫描过滤 change_id）
- Store::list_phase_records(change_id) / list_change_steps(change_id, run_id) / list_change_runs(change_id): 归属键 id
- Store::open_workspace(path) -> Result<Self, StoreError>: 格式版本探测与旧库作废重建——文件缺失 / 空文件 → 建新库 + 写标记；存量 → 读标记：== 2 就绪 / 缺失或 < 2 → 删除重建 + 写标记；删除失败 → StoreError::Db
- Store::calibrate_interrupted_runs(now): 清位按 change_id 定位记录
- 既有 log_change_phase / start_change_run / finish_change_run / append_change_step 随 change_id 定位（签名不变）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| store_test · create 铸 id 回环 | 正向 | 建档（携 id）→ find_change_record(id) 逐字段一致 → 重开 db 再读一致（AC-1 回环主锚；id 主键、name 属性） | 重写 |
| store_test · create 同 id 防御拒绝 | 异常 | 同 id 再建档 → Conflict 且记录表零重复行（防御拒绝不静默覆写——D11 store 侧唯一检查） | 重写 |
| store_test · 同名不同 id 并存 | 边界 | 同名 active + 同名 archived（异 id）各建档成功各一行（name 无唯一约束——D11 归档同名合法化；`create_change_record同名active冲突_归档同名亦拒绝` 旧断言反转） | 重写 |
| store_test · name 非寻址键 | 边界 | `find_change_record(<name 串>)` → None（name 作 id 查询 miss；旧 name 主键寻址面退役结构性锚） | 新增 |
| store_test · list_change_records 主键序 | 边界 | 空库空 vec；多建档（固定 id 字面量）按主键 id 自然序与建档顺序无关 | 适配 |
| store_test · 旧形态库作废重建 | 正向 | 预置缺标记旧形态库（旧表 `9_2_name` 写入 name 主键行、无 `15_1_format` 标记）→ open_workspace 成功零报错；list 空 / find（旧 id 与旧 name）None（旧数据零可达）；重开后标记 format_version == 2 就位（AC-8 主锚） | 新增 |
| store_test · 作废重建后照常读写 | 正向 | 重建后建档 / 开相 / 落账全链落库可读（空库重建零半残态） | 新增 |
| store_test · 标记幂等 | 边界 | 全新库 open → 标记 2；写入数据 → 重开 → 数据保留零作废（标记在场幂等路径——AC-8 scenario） | 新增 |
| store_test · 标记低于当前版本 | 边界 | 预置标记 format_version = 1（< 2，语义编号 1 = name 主键形态时代）→ 作废重建（探测规则「缺失或低于当前」的 < 分支锚） | 新增 |
| store_test · 空文件 / 父目录缺失 | 边界 | 空文件视同缺失 → 建新库 + 写标记；父目录不存在补齐（既有行重证 + 标记断言） | 适配 |
| store_test · 删除失败显式 Err | 异常 | 作废重建删除段失败（环境性文件锁）→ StoreError::Db（恢复性故障语境——旧形态库探测 / 作废路径本身零报错） | 新增 |
| store_test · 表名版本段映射 | 边界 | `CHANGE_RECORD_TABLE` 常量 == `9_3_id`（公式与 native_model id / version / 主键字段名同源断言，v2 的 `9_2_name` 反转）；`10_2_id` + `10_2_change_id` / `12_2_*` / `13_2_*` / `15_1_format` 映射锚 | 重写 |
| store_test · 旧表行对新读面零可达 | 边界 | 预置**标记在场**（format_version=2）但旧表 `9_2_name` 持行的库 → open 零作废（标记幂等）且 find / list 零呈现旧表行（表命名机制第二道防线——D6） | 新增 |
| store_test · 坏行直写注入 | 异常 | 截断 payload 注入 `9_3_id`（裸 redb 直写先例）→ list / find 显式 StoreError 记因不静默（行随表名常量与 new 携 id 重写） | 重写 |
| store_test · change 域操作面 id 形参 | 正向 | start_change_phase / log_change_phase / amend / append_step / list_phase_records / list_change_steps 形参改 id 后既有断言面（attempt 事务内推导 / 单事务原子 / checklist 打包键序 / 重复落账 Conflict）零改动重证 | 适配 |
| store_test · set_change_archived 按 id | 正向 | status 翻转 + archived_at 落库，id / name 均不变；miss → not_found（旧断言仅形参置换） | 适配 |
| store_test · run 域按 change_id | 正向 | start_change_run / finish_change_run / list_change_runs / list_run_steps / calibrate_interrupted_runs 按 change_id 定位（每 run 两写 / 整包原子 / 标定清位语义零改动） | 适配 |
| store_test · open_workspace 内嵌标定 | 正向 | 残留 running + active_phase → 重开即标定（新库标记路径下挂点不变） | 适配 |
| store_test · additive 跨形态语义退役 | 边界 | `存量库additive打开_重开新模型表读写正常` 重写为「标记在场库重开读写正常」——workspace 库跨形态 additive 概念退役（旧形态一律作废重建） | 重写 |
| store_test · v1 行 additive 升级退役 | 废弃 | `存量v1行库additive打开_读出升级两字段none` 与 `inject_v1_change_row` 装置随 v1 解码链退役整体废弃（decode-only 作用面不复存在） | 废弃 |
| store_test · 全局库不受波及 | 边界 | 全局库注册表写入后：workspace 库作废重建发生 → 全局库文件与记录零变化（open_global 零触碰——D5 全局库持衡；`存量库仅workspace注册时新代码additive打开成功` 行持衡） | 新增 |
| store_test · 双库布局既有行 | 边界 | 两级库派生 / 缓存实例复用 / 文件损坏 Err / 旧布局文件惰性废弃（`预设旧布局单库文件后新布局冷启动` 行）——零触点断言面 | 持衡 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | tempfile 真实 redb workspace db（既有 Env / StoresEnv 装置）；旧形态库与坏行经裸 redb `TableDefinition` 直写字节预置（inject_* 先例同式），不经 mock | 本节全部用例 |

### packages/desktop/src-tauri/crates/infra/store/src/change_port.rs -> packages/desktop/src-tauri/crates/infra/store/src/change_port_test.rs

<!-- 挂 AC-1（port 适配半边）。design.md 公共函数 / API 表未声明该文件具名导出
     （`impl ChangeStateStore for Store` 纯 trait 委托胶水）；被测面为 trait 全方法
     id 形参在真 Store 上的中性类型映射与故障翻译。 -->

#### 待测功能

<!-- design.md 未声明该文件具名导出；被测面为 trait 方法在真 Store 上的映射委托。 -->

- `impl ChangeStateStore for Store`: 读半边（get_change(id) / list_phase_records(change_id) / list_steps(change_id, …) / list_runs(change_id)）与写半边（create_change_record / start_phase / log_phase / apply_backtrack / amend_decision_session / set_archived / append_step）的 id 形参委托与记录 ↔ 中性类型映射

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| change_port_test · trait object 全链映射 | 正向 | `&dyn ChangeStateStore` 类型擦除驱动：建档（携 id）→ start_phase → log_phase → list_phase_records，中性类型快照与直调 Store 结果逐字段一致（映射单点无加工） | 重写 |
| change_port_test · 写命令翻译 | 正向 | PhaseLogCommand / BacktrackCommand / StepCommand（change_id 载荷）经 trait 入口落库，与 Store 原生方法直调落库行逐字段等值 | 适配 |
| change_port_test · StoreFault 映射 | 异常 | 同 id 再建档 → `StoreFault::Conflict`；amend 无条目 / set_archived miss → `StoreFault::NotFound`；三分支与 StoreError 一一对应不串型（原「同名 active 冲突」面随 store 语义改「同 id 冲突」） | 适配 |
| change_port_test · trait 边界与 None 语义 | 边界 | `dyn ChangeStateStore` Send + Sync 编译锚定；get_change(未知 id) → Ok(None)（None = 未建档语义——不含文档形态） | 重写 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | tempfile 真实 workspace db + 真实 Store（被测主体即真件，零假件） | 本节全部用例 |

### packages/desktop/src-tauri/crates/infra/store/src/envelope.rs -> packages/desktop/src-tauri/crates/infra/store/src/envelope_test.rs

<!-- 挂 AC-1（信封 key 投影半边）。既有 envelope_test 扩展节：change 模型信封 key
     断言自 name 面翻转 id 面；StoreMetaRecord 不入注册表为新增负断言；分页 / 维度 /
     可读 JSON 面持衡。 -->

#### 待测功能

- `scan` change 模型: `RecordEnvelope.key` = `value["id"]`（信封主键 JSON 投影随主键改 id）
- `list_models` workspace 维度: 注册行集合零变化（StoreMetaRecord 不入 MODEL_ENTRIES——无查看面需求）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| envelope_test · change 信封 key 投影 | 正向 | 建档（id 与 name 双值可辨）→ scan("change") → envelope.key == 记录 id（自 name 面翻转）；value 内 name 仍为属性在场 | 重写 |
| envelope_test · 注册表零新增行 | 边界 | workspace 维度 list_models 行集合与计数（8 行）零变化——`format` 模型名不可见、scan("format") → 未知模型 Err（StoreMetaRecord 不入信封注册表负断言） | 新增 |
| envelope_test · 维度过滤与分页 | 边界 | 跨维度模型名 err / offset-limit 翻页 / 500 截断 / 可读 JSON / 主键自然序——零触点断言面 | 持衡 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | tempfile 真实 workspace db 经公共操作面种子；分页与信封字节面真实读写 | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/workflow/src/state.rs -> packages/desktop/src-tauri/crates/core/workflow/src/state_test.rs

<!-- 挂 AC-1（载荷与记录字段面半边）。既有 state_test 扩展节：ChangeStateRecord 增 id
     首字段；Phase / Step / Run 记录与五写载荷改 change_id；trait 全方法 id 形参编译面；
     线词（RunStatus / RunStepKind / RunStepStatus）持衡。 -->

#### 待测功能

- ChangeStateRecord: 增 `id: String`（首字段，身份锚）；`name` 降为普通属性
- PhaseStateRecord.change / StepStateRecord.change / RunStateRecord.change → `change_id: String`
- 五写命令载荷（PhaseLogCommand / BacktrackCommand / StepCommand / RunStartCommand / RunFinishCommand）`change → change_id`
- ChangeStateStore trait 全方法 id 形参（get_change(id) / start_phase(change_id, …) / list_runs(change_id) 等——编译面最小假件实现）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| state_test · 五写载荷 change_id | 正向 | RunStartCommand / RunFinishCommand 等值面随字段改名重证（PartialEq 逐字段可辨）；其余三载荷同式 | 重写 |
| state_test · ChangeStateRecord 身份面 | 边界 | id + name 双字段构造：id 为身份锚、name 为属性；同 name 不同 id 两记录不等价 | 新增 |
| state_test · trait id 形参编译面 | 边界 | 最小假件实现 trait 全方法 id 形参后可调（编译面锚——行为断言挂 store_test / change_port_test / 写面各节） | 适配 |
| state_test · 线词持衡 | 边界 | RunStatus 五值 / RunStepKind 五值 / RunStepStatus 四值与封闭集防线——零触点 | 持衡 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 纯内存构造与等值面（零 IO 零进程边界） | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/workflow/src/write/create.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/create_test.rs

<!-- 挂 AC-1（铸出点与 CreateOutcome.id 半边，D1 / D11）。既有 create_test 扩展节：
     同名 active 拒绝改写面前置 name 扫描（仅拒 active——归档同名不拒）；前置七道
     全过后铸 uuid v7 形态 id 入载荷。铸出路径断言 id 非空 / 同名单次重铸相异 /
     与库内逐字一致（零固定值对拍、零 uuid 库语义验证——D2）。 -->

#### 待测功能

- create(main_root: &Path, worktree_root: &Path, store: &dyn ChangeStateStore, vcs: &dyn WorktreePort, name: &str, goal: &str) -> Result<CreateOutcome, String>: 签名不变；前置④ 改 name 扫描查重（仅拒同名 active）；前置通过后铸 uuid v7 形态 id 入载荷（D1）；补偿链按 id 删记录
- CreateOutcome: 增 `id: String`（首字段，本次铸出）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| create_test · 建域四件套 + 铸 id | 正向 | 成功 → 目录 + explore.md（goal 原文）+ 建档：outcome.id 非空；find_change_record(outcome.id) 命中且与出线逐字一致；record.name 为裸名属性（铸出路径行为锚） | 重写 |
| create_test · 同名单次重铸相异 | 边界 | 同名（已归档）再建档 → 新 id ≠ 旧 id，两行并存（归档同名合法化——D11） | 新增 |
| create_test · 同名 active 拒绝前置 | 异常 | db 已有同名 active 记录（id 各异）→ Err 且零目录零建档（查重改 name 扫描——「同名 active 冲突」归写面单点） | 重写 |
| create_test · 目录冲突拒绝持衡 | 异常 | 主仓目录已存在 → Err 零副作用（前置①持衡） | 持衡 |
| create_test · 补偿链按 id | 异常 | fs 失败补偿删除按 id（本次自插行零残留——list 空 / find(outcome.id) None；补偿再失败呈现残留对象指引） | 适配 |
| create_test · CreateOutcome 线形 | 正向 | serde 线形恰五键（id 首键 / name / created / worktree / warnings） | 重写 |
| create_test · 既有校验与 vcs 装置 | 边界 | 非法名 / 空白 goal / 超长名 / git 三态 / branch 冲突 / 脏仓警告 / bootstrap / 安装失败——断言面零改动（唯 find 侧断言改 id） | 持衡 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| ChangeStateStore（注入依赖入参） | 假件仅用于补偿链双故障注入行（可编程删除 Err）；其余行真实 tempfile Store（fs 与 db 均真实组合） | 补偿再失败行用假件；其余全节真件 |
| WorktreePort（注入依赖入参） | 既有 vcs 假件装置沿用（调用捕获 + git 三态注入） | 建域 / 冲突 / 补偿各行 |

### packages/desktop/src-tauri/crates/core/workflow/src/write/archive.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/archive_test.rs

<!-- 挂 AC-2（写面 id → record.name 目录改名半边）、AC-5（分辨率单点）。既有
     archive_test 扩展节：archive(layout, store, id)；判定点（后缀扫描 + 单分量
     校验）施于解析出的 name；错误文案随 id 语境修订（呈现记录名——R8）。 -->

#### 待测功能

- archive(layout: &Layout, store: &dyn ChangeStateStore, id: &str) -> Result<ArchiveOutcome, String>: get_change(id) 读记录（None → 未建档拒绝面沿袭）；record.name 供给 active 目录改名与 archive 树定位；id / name 均不随改名变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| archive_test · 双写成功按 id | 正向 | 建档（携 id）+ 磁盘目录在场 → archive(layout, store, id) → 目录改名入 archive 树（日期前缀）+ db 翻转；记录 id / name 双不变；ArchiveOutcome.archived_date 为 UTC 当日 | 重写 |
| archive_test · 未建档 id 拒绝 | 异常 | 未知 id → 显式 Err 零 fs 零 db 变更（拒绝先于一切变更）；错误文案呈现 id 语境 | 适配 |
| archive_test · 续半边与定位判定 | 正向 | 半完成态（目录已在 archive 树 + db 仍 active）重试仅补翻转；双树同名 active 精确名优先；archive 树无前缀同名目录命中补翻转——判定点语义零改动，name 自记录供给 | 适配 |
| archive_test · worktree 记录行 | 边界 | worktree 记录 archive 树命中续半边 / 未 merge 引导 / legacy 未命中泛化——断言面随形参 id 置换（worktree / name 自记录供给） | 适配 |
| archive_test · 翻转失败半完成态 | 异常 | 假件 store set_archived Err → Err 呈现半完成态（目录已改名事实读侧呈现） | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| ChangeStateStore（注入依赖入参） | 假件仅用于翻转失败故障注入行（可编程 set_archived Err）；其余行真实 tempfile Store + 真实 tempdir Layout（双载体全真实组合） | 翻转失败行用假件；其余全节真件 |

### packages/desktop/src-tauri/crates/core/workflow/src/write/phase_start.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/phase_start_test.rs

<!-- 挂 AC-1（id 形参随动半边）。既有 phase_start_test 扩展节——校验 / 表位语义零改动。 -->

#### 待测功能

- phase_start(store: &dyn ChangeStateStore, change_id: &str, phase: &str) -> Result<PhaseStartOutcome, String>: 形参 id；内部 get_change(change_id)（workflow_type / 表位校验零改动）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| phase_start_test · 开相落库按 id | 正向 | 合法表位开相 → store.start_phase(change_id, …) 落 active_phase；outcome.attempt / start_at 与 db 一致 | 重写 |
| phase_start_test · 校验前置保留 | 异常 | 非法相位 / workflow_type 非 requirement / 未建档 id → Err 零写入（校验语义零改动） | 适配 |
| phase_start_test · 重开 attempt | 边界 | start → log 清位 → 再 start attempt=2（id 形参下自既有条目数推导不变） | 适配 |
| phase_start_test · store 故障传播 | 异常 | 假件 StoreFault → Err 记因不静默 | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| ChangeStateStore（注入依赖入参） | 进程内假件（记录写命令与调用序、可编程 StoreFault） | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/workflow/src/write/phase_log.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/phase_log_test.rs

<!-- 挂 AC-1（id 形参随动半边）。既有 phase_log_test 扩展节——verdict 推导 / report
     长度 / 表位 / active_phase 匹配校验零改动。 -->

#### 待测功能

- phase_log(store: &dyn ChangeStateStore, change_id: &str, input: &PhaseLogInput) -> Result<PhaseLogOutcome, String>: 形参 id（PhaseLogCommand 载荷 change_id 随行）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| phase_log_test · pass / fail 落账按 id | 正向 | checklist 全 pass → verdict=pass；PhaseLogCommand 经 store.log_phase 落库，outcome 与命令一致；active_phase 清位随行（既有断言自 name 形参置换 id） | 重写 |
| phase_log_test · 校验前置保留 | 异常 | report 2001 拒绝 / 恰 2000 通过；active_phase 缺失 / 表外相位 → Err 零写（校验前置零改动） | 适配 |
| phase_log_test · 故障传播与形状随行 | 异常 | 假件 StoreFault::Conflict 传播；checklist 空 / skipped / 三槽位 None 两态透传 | 适配 |
| phase_log_test · 未建档 id 显式 err | 异常 | 未建档 id → Err 零写入（get_change miss 面） | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| ChangeStateStore（注入依赖入参） | 进程内假件（捕获 PhaseLogCommand 逐字段比对 + 可编程 StoreFault） | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/workflow/src/write/backtrack.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/backtrack_test.rs

<!-- 挂 AC-1（id 形参随动半边）。既有 backtrack_test 扩展节——白名单二次校验 /
     表位 / reason ≤500 / stale 闭包计算零改动。 -->

#### 待测功能

- backtrack(store: &dyn ChangeStateStore, change_id: &str, input: &BacktrackInput) -> Result<BacktrackOutcome, String>: 形参 id（BacktrackCommand 载荷 change_id 随行）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| backtrack_test · 回跳落库按 id | 正向 | 白名单内目标回跳 → BacktrackCommand（to / reason + stale_dependents）经 store.apply_backtrack 落库；outcome 与命令一致 | 重写 |
| backtrack_test · stale 闭包计算 | 正向 | dependents BFS 多支逐支核对 / 末端空闭包（闭包语义零改动） | 适配 |
| backtrack_test · 校验前置保留 | 异常 | 越权目标 / reason 501 / 非法与未来目标 / 发起相位无条目 → Err 零写 | 适配 |
| backtrack_test · 未建档 id 与故障 | 异常 | 未建档 id → Err 零写；假件 StoreFault 传播 | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| ChangeStateStore（注入依赖入参） | 进程内假件（捕获 BacktrackCommand 与 stale_dependents 闭包逐项比对 + 可编程故障）；phase_table 真实组合 | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/workflow/src/write/decision_log.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/decision_log_test.rs

<!-- 挂 AC-1（id 形参随动半边）。既有 decision_log_test 扩展节——amend 定点改写 /
     幂等 / 无条目 Err / 不做表位校验语义零改动。 -->

#### 待测功能

- decision_log(store: &dyn ChangeStateStore, change_id: &str, phase: &str, session_id: &str) -> Result<DecisionLogOutcome, String>: 形参 id

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| decision_log_test · amend 定点与幂等 | 正向 | 多 attempt 仅最新条目被改；重复挂账同值幂等覆写不追加（形参 id 置换） | 重写 |
| decision_log_test · 无条目与故障 | 异常 | 该相位无条目 → Err 显式；假件 StoreFault 传播；不做表位校验语义零改动 | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| ChangeStateStore（注入依赖入参） | 进程内假件（可编程条目序列与故障） | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next_test.rs

<!-- 挂 AC-1（id 形参随动半边——D7 分辨率单点：prompt 插值改 record.name）。
     既有 phase_next_test 扩展节：路由 / 白名单 / 重试上限 / backtrack 检测语义零改动。 -->

#### 待测功能

- phase_next(store: &dyn ChangeStateStore, change_id: &str, run_id: &str, anchors: &SessionAnchors) -> Result<PhaseNextOutcome, String>: 形参 id；prompt 插值改用 `record.name`
- SessionAnchors: 进程内会话锚点（键 `(change_id, run_id)`；基线自 PhaseRecord 行数）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| phase_next_test · prompt 插值改 record.name | 正向 | 建档 id 与 name 双值可辨 → 首相位路由：executor / evaluator prompt 内插值为 name（id 串不误入 prompt——D7 插值改点行为锚） | 重写 |
| phase_next_test · 锚点键 id 隔离 | 边界 | SessionAnchors 键 (change_id, run_id)：同 run_id 异 id 锚点不串台；已 pass 相位行在场 → 重启新锚点直接推进不重头 | 重写 |
| phase_next_test · 无建档 id 显式 err | 异常 | 未建档 id → Err 不静默空产出；run_id 空白 Err 保留 | 适配 |
| phase_next_test · 路由族既有行 | 边界 | pass 推进 / fail 重试 attempt 递增 / 上限 MaxRetriesExceeded / 全 pass done / backtrack 路由 / 只读零写面——断言面自 name 形参置换 id | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| ChangeStateStore（注入依赖入参） | 进程内假件（记录序列 + StoreFault 注入）；「读源 db」组合行用真实 tempfile Store 种子 | 路由族行假件；组合行真件 |

### packages/desktop/src-tauri/crates/core/workflow/src/write/run.rs -> packages/desktop/src-tauri/crates/core/workflow/src/write/run_test.rs

<!-- 挂 AC-1（run 史载荷 change_id 归属半边）。既有 run_test 扩展节——每 run 两写 /
     终态三值拦截 / 整包委派语义零改动。 -->

#### 待测功能

- run_start / run_finish: 校验读记录面随动（载荷 `change_id`）；未建档 id 显式 err

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| run_test · 写面委派按 change_id | 正向 | run_finish 空步整包委派 / 命令逐字段透传——载荷 change_id 置换后断言面零改动 | 适配 |
| run_test · 未建档 id 显式 err | 异常 | run_start 未建档 id → Err（读记录面随动） | 适配 |
| run_test · 既有拦截行 | 异常 | 空白 run_id 拒绝 / run_finish 非终态三值拒绝（interrupted 记因定式）——零触点 | 持衡 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| ChangeStateStore（注入依赖入参） | 进程内假件（命令捕获 + 可编程故障） | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/workflow/src/queries/list.rs -> packages/desktop/src-tauri/crates/core/workflow/src/queries/list_test.rs

<!-- 挂 AC-3（db 单源主半边）、AC-2（归档条目裸名半边）。既有 list_test 为**重写主体**：
     磁盘扫描 / 文档形态 / 并集去重 / 磁盘回退月分组断言整体退役，改 db 全量记录单源 +
     零发现反例 + 裸名与 id。 -->

#### 待测功能

- list_changes(store: &dyn ChangeStateStore) -> ChangeList: 签名收敛（去 Layout）；db 单源（active / archive 两树扫描与文档形态条目退役）；零 fs 触点
- ChangeSummary: 增 `id: String`（首字段）；name 恒裸名；月分组 archived_at 唯一权威（缺失归「未知时间」置尾）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| list_test · db 单源全量 | 正向 | db 多条（active / archived 异 id）→ 列表 = db 记录全量（active 组 + 归档月组）；条目 id 恒在案、name 恒裸名（AC-2 列表半边） | 重写 |
| list_test · 磁盘目录零发现 | 异常 | 预置磁盘-only active 目录 + archive 树前缀 / 无前缀目录（含惰性字节样本）→ 清单零呈现该等条目、零报错、目录字节零变化（AC-3 主锚——原「并集与去重」「文档形态照常入列」断言反转） | 重写 |
| list_test · 月分组 archived_at 权威 | 边界 | db archived 记录按 archived_at 分组；archived_at 缺失 → 未知时间组置尾——目录前缀不再参与（原「磁盘回退目录前缀」断言退役） | 重写 |
| list_test · 空输入 | 边界 | 空 db（磁盘有目录同断言）→ 空列表零组 | 重写 |
| list_test · 状态面透出 | 正向 | status / active_phase / created 字段面保留；worktree 记录条目照常归组（记录态演绎零改动） | 适配 |
| list_test · 磁盘扫描面退役 | 废弃 | `磁盘归档组_同月内新名在前` / `既有守卫_active扫描跳过archive目录与混入文件` / `d11归组_db_status权威且不回写store` / `目录名取位三态_主仓命中建档名_archived前缀名_未命中建档名` / `无日期前缀入未知时间组置尾`（磁盘半边）随扫描段删除整体废弃 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 真实 tempfile Store 种子（固定 id 字面量）+ 真实 tempdir 磁盘树（零发现反例要求目录真实在场且字节零变化） | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs -> packages/desktop/src-tauri/crates/core/workflow/src/queries/detail_test.rs

<!-- 挂 AC-2（详情全状态面）、AC-4（文档形态退役）。既有 detail_test 重写主体：
     id 寻址、ChangeDetail.id、created 恒自 created_at；文档形态 / 磁盘前缀回退断言退役。 -->

#### 待测功能

- change_detail(layout: &Layout, store: &dyn ChangeStateStore, id: &str) -> Option<ChangeDetail>: get_change(id) → 记录 → locate_change(layout, record.worktree, record.name)；record None → 恒 None（不回退目录解析）；ChangeDetail 增 id、name 自记录直读

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| detail_test · 归档 change 全状态面 | 正向 | 建档 archived（记录 id / 裸名）+ 磁盘目录带 `YYYY-MM-DD-` 前缀 → change_detail(layout, store, id) → status=archived、9 站 pipeline、runs 全量出线（AC-2 主锚——错配链结构性退役） | 重写 |
| detail_test · 未知 id 未找到 | 异常 | db 无该 id（磁盘任意目录在场——裸名 / 前缀名形态）→ None；MUST NOT 回退目录解析、MUST NOT 空流水线文档形态（AC-4 主锚） | 重写 |
| detail_test · ChangeDetail.id 与裸名 | 正向 | id 首字段出线；name 恒裸名（自记录直读——归档前缀形态不出现在 name） | 新增 |
| detail_test · created 单源 | 正向 | created 恒自 created_at（原 archive 树文档形态 created 回退行随磁盘前缀回退删除退役） | 重写 |
| detail_test · 文档形态分支退役 | 废弃 | `文档形态_空流水线与产物清单且字节零进投影`、`archive树文档形态_created回退查询名前缀`、`详情run史出线_文档形态恒空与reason_none`（文档形态半边）随分支删除整体废弃 | 废弃 |
| detail_test · 流水线重组与出线 | 正向 | 9 站重组 / attempt 升序 / checklist 内联 / 时间 ISO 与 null 留位 / 槽位两态 / skipped 与 stale / worktree 出线 / run 史投影与并列稳定序——断言面自 name 形参置换 id（记录供给语义重证） | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 真实 tempfile Store 种子 + 真实 tempdir 磁盘树（产物文件与惰性字节样本） | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/workflow/src/queries/mod.rs -> packages/desktop/src-tauri/crates/core/workflow/src/queries/mod_test.rs

<!-- 挂 AC-2（归档目录定位半边）。locate_change 名义签名不变、消费语义修订：
     name / worktree 恒自记录供给（id 为不透明串不做目录名语义校验）；archive 后缀
     扫描与单分量名校验保留（施于解析出的 name——D7）。 -->

#### 待测功能

- locate_change(layout: &Layout, worktree: Option<&str>, name: &str) -> Option<ChangeLocation>: 签名不变；active 精确名 / archive 日期前缀后缀匹配 / 单分量校验保留

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| mod_test · 定位判定族持衡 | 边界 | active 精确 / archive 前缀后缀 / 多分量与穿越与空名 None / 非日期形态不误匹配 / 双树同名优先 / 多日期前缀——判定点语义零改动（校验施于解析出 name） | 持衡 |
| mod_test · worktree / name 记录供给面 | 边界 | worktree 回退命中 / 优先级链 / worktree 目录被删回退 miss / 穿越校验保留——worktree / name 恒自记录供给（消费语义修订锚，D7） | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 真实 tempdir Layout 目录树（纯路径推导，零进程边界依赖） | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/orchestration/src/control.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/control_test.rs

<!-- 挂 AC-6（run 注册表半边）。既有 control_test 扩展节：复合键 (root, change) →
     (root, change_id)（全方法签名与键构造点随动，proposal 口径 12 处）；name 非键后
     同 root 同名不同 id 可并行。 -->

#### 待测功能

- ChangeFlowControl.begin_run(self: &Arc<Self>, root: &str, change_id: &str, run_id: String, started_at: i64) -> Result<RunGuard, String>: 复合键 `(root, change_id)`
- subscribe / request_stop / current_session / publish / set_session / answer / confirm / snapshot / take_pending: 全方法同式随动

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| control_test · 复合键 (root, id) 登记与并行冲突 | 异常 | begin_run(root, id) 登记；同 root 同 id 二次 begin → Err；RunGuard 除名后重入可达（键构造点逐处置换） | 重写 |
| control_test · 同名不同 id 可并行 | 正向 | 同 root 下两条同名（异 id）change 各自 begin → 均受理互不误拒（name 非键语义锚——原「异 workspace 同名不误拒」行扩写） | 重写 |
| control_test · 订阅与会话槽隔离 | 边界 | 复合键订阅寻址不串台 / RunGuard 仅本键除名 / stop miss 幂等 / confirm-answer 单发 / publish 状态机——断言面随键 id 置换 | 适配 |
| control_test · DTO 线面零渗出 | 边界 | RunNotice kind-only 零载荷 / 复合键不出线 / begin_run started_at 入档 / 终态除名零累积——零触点 | 持衡 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | `#[tokio::test]` 真实 tokio 广播 / 原语 + 内存注册表（零外部替身；session / worker 捕捉装置沿既有） | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/orchestration/src/archive_flow.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/archive_flow_test.rs

<!-- 挂 AC-6（archive 注册表 + 归档两会话 provenance 半边）、AC-2（archive 链路改名单点半边）。
     既有 archive_flow_test 扩展节：ArchiveControl / ArchiveRequest / ArchiveGuard 键 id；
     drive 前置段一次解析 record.name 供 branch / prompt / pathspec / 目录名 / ArchiveSummary.name；
     两 source_ref 身份段 id。 -->

#### 待测功能

- ArchiveControl.begin(self: &Arc<Self>, root: &str, change_id: &str) -> Result<ArchiveGuard, String>: 键 id 化；is_active / subscribe / request_stop / current_session / set_session / publish / snapshot 全方法同式随动
- preflight(main_root: &Path, store: &dyn ChangeStateStore, id: &str, vcs: &dyn ArchiveVcsPort, run_active: bool) -> Option<ArchivePreflight>: id 寻址（worktree 自记录读；name 出线取 record.name）
- drive 前置段一次解析 `record.name`（branch `change/<name>` / prompt 内文 / pathspec / archived_dir 组装 / `ArchiveSummary.name` 全用 name）
- 两会话 provenance: `source_ref = <id>/archive/spec-sync` 与 `<id>/archive/merge-conflict`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| archive_flow_test · 控制面键 id | 正向 | ArchiveControl.begin(root, id) 登记 / 重入防护 / 订阅与会话槽按 (root, id) 隔离——键构造点逐处置换 | 重写 |
| archive_flow_test · 两会话 provenance 身份段 id | 正向 | spec-sync 会话 `source_ref == <固定 id>/archive/spec-sync`；merge-conflict 会话 `<固定 id>/archive/merge-conflict`（原 `<change>/archive/*` 断言反转——AC-6 字面） | 重写 |
| archive_flow_test · drive 分辨率单点 | 正向 | branch `change/<name>` / merge_conflict_prompt 内文 / pathspec 圈定 / archived_dir 组装 / ArchiveSummary.name 全用 record.name（id ≠ name 形态下逐点断言——磁盘 / git 面仍 name 化，D7） | 适配 |
| archive_flow_test · preflight 读面 | 正向 | preflight(…, id, …) 聚合全字段 / None 三态 / 完成度核算与 phase_next 对拍 / worktree 自记录读——断言面随 id 置换 | 适配 |
| archive_flow_test · 冲突分支族 / lean / seal / finalize / legacy | 边界 | 阶段机语义与收口单点零改动（唯请求键与 provenance 值演进）；verify_resolution / residual_markers 纯函数行零触点 | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| ArchiveVcsPort / WorkerAgentPort / ChangeStateStore（注入依赖入参） | 既有 FakeVcs（冲突编程面 + 快照队列）/ 假 worker（outcomes 队列 + 请求捕获）/ 假 store 装置沿用——键与 provenance 断言面随 id 值演进 | 本节全部链级行 |
| 文件系统 | 真实 tempdir 主仓树 + worktree 树（产物与冲突文件按需布置） | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/orchestration/src/walker.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/walker_test.rs

<!-- 挂 AC-6（run provenance 半边）。既有 walker_test 扩展节：RunRequest.change → change_id；
     `source_ref = <id>/<phase>/<role>/<attempt>`（run_worker 组装点）；ToolCommand 构造点与
     snapshot.detail 调用点随动；每 run 两写时序零改动。 -->

#### 待测功能

- RunRequest.change_id: walker 全构造点随动
- provenance `source_ref` 定式 `<id>/<phase>/<role>/<attempt>`（executor / evaluator / decision 与反馈边修复会话同式）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| walker_test · provenance 定式身份段 id | 正向 | 中断相位承接重试 / 反馈边修复新会话等各行：source_ref 逐字 == `<固定 id>/<phase>/<role>/<attempt>`（原 name 段断言重写——AC-6 字面） | 重写 |
| walker_test · 每 run 两写与时序 | 边界 | run_start / run_finish 载荷 change_id；零额外 run 域写 / 三死亡路径同一出口——时序语义零改动 | 适配 |
| walker_test · 真实写面组合行 | 正向 | 多相位 run 落库回验 / 决策挂账 / exec_root 透传 / 续走不重头 / 相位循环工具调用序——种子经 id 建档（断言面随动） | 适配 |
| walker_test · snapshot.detail 调用点 | 边界 | detail(root, id) 形参随动（快照读源 id 寻址） | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 假引擎 / 假 runner / 假 worker 装置 | 既有进程边界假件沿用（请求捕获 + 产出注入）；种子改携固定 id | 本节全部用例 |
| ChangeStateStore | 组合行真实 tempfile Store（真件落库回验）；纯路由行沿既有注入面 | 组合行真件 |

### packages/desktop/src-tauri/crates/core/orchestration/src/port.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/port_test.rs

<!-- 挂 AC-5（命令载荷 id 化随动半边，D12）。既有 port_test 扩展节。 -->

#### 待测功能

- ToolCommand 六变体（PhaseNext / PhaseStart / PhaseLog / Backtrack / DecisionLog / TestExecution）`change → change_id`；`StaticCheck` 增 `change_id: String`（D12）
- WorkflowSnapshotPort::detail(root, id)；TestExecutionRunner::run(root, name)（形参改名——磁盘面 port 收 name，解析在 steps 消费点）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| port_test · ToolCommand 载荷 change_id | 边界 | 六变体 + StaticCheck 新字段构造与 match 穷尽分发（封闭集持衡——字段改名置换） | 适配 |
| port_test · StaticCheck change_id 归键 | 边界 | StaticCheck 携真实 change_id 构造面（非空串占位——D12 载荷锚；行为断言归 steps_test 审计行） | 新增 |
| port_test · 假实现注入面 | 边界 | static_check / diff_context 假实现经 arc 注入 / box 别名——零触点 | 持衡 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 注入依赖（入参） | 既有假实现（调用捕获 + 产出注入） | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/orchestration/src/steps.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/steps_test.rs

<!-- 挂 AC-1（步骤 change_id 归键半边——D12：StaticCheck 审计行空串占位退役）。
     既有 steps_test 扩展节：审计行 change_id 归键；test-execution 臂经
     get_change(change_id) 解析 record.name 后调 runner（未建档 → Err）。 -->

#### 待测功能

- 七臂命令包络 StepRecord 审计落库：`change_id` 归键（含 StaticCheck 臂）
- TestExecution 臂: 调用前经 `store.get_change(change_id)` 解析 `record.name`（未建档 → Err）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| steps_test · 审计行 change_id 归键 | 正向 | 假 runner 驱动七臂 → 每臂 StepRecord change_id == 真实 id；static_check 行可经 list_steps(change_id, …) 枚举（空串占位退役——D12 修正锚） | 重写 |
| steps_test · test-execution name 解析 | 正向 | TestExecution 臂经记录解析 record.name 后调 runner（捕获 runner 入参为 name）；未建档 id → Err 不发 runner（D7 解析单点） | 新增 |
| steps_test · 全链落库组合 | 正向 | 真实 Store + 真实写面：phase_log 臂驱动后 PhaseRecord 与 StepRecord 同库可查（链路入口组合行——id 种子） | 适配 |
| steps_test · 既有臂族行 | 边界 | 摘要截断 / reference 随行 / store 故障行 / 五参构造与 runner 注入——change_id 置换后断言面零改动 | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| StaticCheckRunner / TestExecutionRunner（注入依赖入参） | 既有 FakeRunner / FakeTestExecutionRunner 沿用（记录调用 + 可编程产出） | 臂驱动行 |
| ChangeStateStore | 组合行真实 tempfile Store（落库证据真件）；故障行进程内假件注入 StoreFault | 组合行真件；故障行假件 |

### packages/desktop/src-tauri/crates/core/orchestration/src/run_history.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/run_history_test.rs

<!-- 挂 AC-1（run 载荷 change_id 归属半边）。既有 run_history_test 扩展节——
     过滤单点语义零改动。 -->

#### 待测功能

- `finish_command` 载荷 `change_id`（RunRequest.change_id 随行）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| run_history_test · 载荷 change_id 透传 | 正向 | 全词汇十步恰五落 / 身份与终态等值 / 委派真件回环——字段改名置换，断言面零改动 | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 纯函数直驱 + 真件委派回环（既有先例，零假件） | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/orchestration/src/snapshot.rs -> packages/desktop/src-tauri/crates/core/orchestration/src/snapshot_test.rs

<!-- 挂 AC-4（详情读链消费面随动半边）。既有 snapshot_test 扩展节：detail(root, id)；
     文档形态走通行与损坏字节行退役（未知 id → Err）。 -->

#### 待测功能

- WorkflowSnapshotPort::detail(root: &str, id: &str) -> Result<ChangeDetail, String>: 第二参改 id（经 queries id 寻址）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| snapshot_test · db 读源 detail 按 id | 正向 | store 种子 → detail(root, id) 与 queries::change_detail 直调结果 serde 等值（换血不加工） | 重写 |
| snapshot_test · 未知 id 显式 Err | 异常 | 未建档 id → Err 记因（原「文档形态 db 缺记录磁盘在场走通不 err」行反转——未找到降级） | 重写 |
| snapshot_test · 文档形态与损坏字节行退役 | 废弃 | `文档形态db缺记录磁盘在场走通不err`、`损坏workflowjson字节样本零读取照常出detail` 随文档形态面退役整体废弃 | 废弃 |
| snapshot_test · port 契约持衡 | 边界 | trait object 装配可达 / root 失配与未知 change 显式 Err——零触点 | 持衡 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 真实 tempfile Store（种子经 store 操作面）；trait object 仅 `Arc<dyn ChangeStateStore>` 类型擦除 | 本节全部用例 |

### packages/desktop/src-tauri/src/commands/changes/mod.rs -> packages/desktop/src-tauri/src/commands/changes/mod_test.rs

<!-- 挂 AC-2（:262 归档条目名断言改写半边）、AC-3（命令面 db 单源半边）、
     AC-4（未知 id 未找到半边）、AC-5（id 参数半边）。既有 changes/mod_test.rs
     扩展节：夹具改 id 种子，读 / 写命令参数面 id 置换。 -->

#### 待测功能

- get_change_detail(stores, control, root, id) / read_artifact(stores, root, id, kind, source) / archive_change(app, root, id): 定位参数 id（read_artifact 经 find_change_record(id) 供给 worktree / name；blank id 显式 Err）
- list_changes(stores, root): 签名不变；行为 db 单源
- create_change(app, root, name, goal): 签名不变；返回 CreateOutcome（增 id）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| changes_test · 读命令 id 透传与归档条目断言（:262 段） | 正向 | list / detail / read_artifact 命令与 core 查询 serde 等值（参数 id 形态）；归档条目 name 恒裸名（`2026-02-02-seeded-archived` 前缀名断言改 `seeded-archived`）且 id 在场；月分组 2026-02 自 archived_at | 重写 |
| changes_test · 磁盘-only 零呈现（命令面） | 异常 | 主仓磁盘-only 目录在场 → list 命令零呈现该条目（db 单源命令半边——原 `docs-only` 条目入列断言反转） | 重写 |
| changes_test · 未知 id 未找到 | 异常 | detail 命令未知 id → None；统一查询装配行自「文档形态与未知 change」改未知 id 未找到（早退口径保留） | 适配 |
| changes_test · archive_change 薄命令 | 正向 | 双写成功返回 outcome / 无建档与目标冲突错误映射 / blank id Err——参数 id 置换（三件事薄包装不吞错不加工） | 适配 |
| changes_test · create 接线 | 正向 | create_change → 写面；返回 DTO 增 id（成功后 detail 按 id 可见可发起） | 适配 |
| changes_test · 统一查询装配族 | 边界 | runs 全史 / waitingConfirm 停等 / worktree 感知 / 归档引导 err 透传——断言面随 id 置换 | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 真实 tempdir root + `WorkspaceStores` 真实组合（fs 与 db 双真实）；`*_with` 泛型测试缝直调；MockRuntime 既有装置 | 本节全部用例 |

### packages/desktop/src-tauri/src/commands/change_flow/mod.rs -> packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs

<!-- 挂 AC-5（五命令 id 半边）、AC-6（命令面键 id 半边）。既有 change_flow/mod_test.rs
     扩展节：装配与互斥校验经 id 读记录；blank 文案随 id 语境。 -->

#### 待测功能

- change_flow_start(app, on_event, root, id, auto_next_phase) / change_flow_stop(app, root, id) / change_flow_answer(app, root, id, answer) / change_flow_confirm(app, root, id, proceed) / change_flow_watch(app, on_event, root, id): 定位参数 id
- 装配（建档校验 / workflow_type / exec root / 归档互斥）经 id 读记录；ChangeFlowSink / begin_run / 订阅键 id

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| change_flow_test · 五命令 id 定位与守卫 | 边界 | start / stop / answer / confirm / watch 参数 id；blank_id 各命令模板保留（原 blank_change 六缝置换） | 适配 |
| change_flow_test · 前置校验经 id 读记录 | 边界 | 建档校验 / 相位表校验 / exec root 解析 / worktree 目录缺失拒绝 / 归档互斥快照——经 get_change(id) 解析（错误面 id 语境呈现记录名） | 适配 |
| change_flow_test · 并行冲突键 id | 异常 | `start同change并行run冲突err`：同 root 同 id 二次 start → Err；同名不同 id 各自受理互不误拒（原「复合键命令面_异root同名并行互不误拒」扩写——name 非键） | 重写 |
| change_flow_test · sink 转译与通知面 | 边界 | 通道五 kind 零载荷 / session_event 记会话锚 / 归档进行中拒绝键 id——断言面随 id 置换 | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock（进程边界真件） | 真实 tempdir root + WorkspaceStores 真实组合；假引擎 / capturing_channel / PATH 隔离窗口沿既有装置 | 本节全部用例 |

### packages/desktop/src-tauri/src/commands/archive_flow/mod.rs -> packages/desktop/src-tauri/src/commands/archive_flow/mod_test.rs

<!-- 挂 AC-5（五命令 id 半边）。既有 archive_flow/mod_test.rs 扩展节：前置校验
     经 id 读记录；ArchiveSink / ArchiveControl 调用键 id；merge_target 分支名
     仍 name 化（记录供给）。 -->

#### 待测功能

- archive_flow_preflight(app, root, id) / archive_flow_start(app, on_event, root, id, sync_specs) / archive_flow_stop(app, root, id) / archive_flow_state(app, root, id) / archive_flow_watch(app, on_event, root, id): 定位参数 id
- 前置校验（建档 / status=active / run 互斥）经 id 读记录；ArchiveSink / ArchiveControl 调用键 id

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| archive_flow_test · 五命令 id 定位与拒绝面 | 异常 | 五命令参数 id；blank / 未建档 / 已归档 / run 在案各显式 err（id 语境） | 适配 |
| archive_flow_test · preflight merge_target 仍 name | 正向 | merge_target 分支名 `change/<name>` 自记录供给（id 参数 → 记录 → name 分辨率单点）；worktree 自记录读 | 适配 |
| archive_flow_test · 回环与转译行 | 正向 | start 受理与事件流回环（legacy）/ 重入防护 / agent 失败收敛（CliMissing）——键 id 置换后断言面零改动 | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock（进程边界真件） | MockRuntime 四态托管装置 + capturing_channel / PATH 隔离窗口既有（零扩展） | 本节全部用例 |

### packages/desktop/src-tauri/src/commands/mod.rs -> packages/desktop/src-tauri/src/bindings/mod_test.rs

<!-- 挂 AC-5（bindings 签面随动半边）、AC-9（bindings:check 守卫绿半边）。本变更零新增
     命令零新增 DTO 类型名（id 为既有 DTO 的字段）——三清单零补录；签面断言
     （archive_change 入参 / 归档五命令 Channel 参型 / CreateOutcome 字段面）随 id 置换
     重写。design.md 未声明该源文件自有可测函数（invoke_handler 登记单点为声明式宏）。 -->

#### 待测功能

<!-- design.md 未声明该源文件公共 API；被测面为生成物签面断言（commands 包装签名 / DTO 类型段字段面）。 -->

- 生成物签面断言: 13 命令参数名 id / CreateOutcome·ChangeDetail·ChangeSummary 增 id 字段出线（重导幂等 + 全量 contains 守卫）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| bindings 签面 · archive_change 入参 id | 正向 | 产物含 `archiveChange: (root: string, id: string) => __TAURI_INVOKE<ArchiveOutcome>("archive_change", { root, id })`（原 change 参名断言重写） | 重写 |
| bindings 签面 · 归档五命令 Channel 参型 | 正向 | archiveFlowStart / archiveFlowWatch 签名参名 `change` → `id`（`(onEvent: Channel<ArchiveUpdate>, root: string, id: string, …)` 逐字） | 重写 |
| bindings 签面 · CreateOutcome 恰五字段 | 正向 | CreateOutcome 类型段含 id 首字段（id / name / created / worktree / warnings 五字段面——原恰四字段断言语义演进） | 重写 |
| bindings 签面 · ChangeDetail / ChangeSummary 含 id | 正向 | 两类型段含 `id: string` 字段（出线增量最小化——D8） | 新增 |
| bindings 清单 · 零补录核实 | 边界 | COMMAND_WRAPPERS / COMMAND_NAMES / DTO_TYPES 三清单零补录（无新命令 / 无新类型名）；全量 contains 守卫与首尾锚持衡 | 持衡 |
| bindings 签面 · create_change 持衡 | 边界 | createChange 三参（root / name / goal）与 invoke 形态不变 | 持衡 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| bindings.ts 产物文件（进程边界） | 真实组合不 mock：`export_bindings()` 真实重导 + 产物真实读取 + 产物互斥锁串行（既有装置沿用） | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/workflow/src/lib.rs -> packages/desktop/src-tauri/crates/core/workflow/tests/corpus_golden_test.rs

<!-- 挂 AC-9（语料与 golden 主半边）、AC-3（列表 golden 半边）、AC-2（归档前缀样本
     golden 半边）。design.md 未声明 lib.rs 自有公共 API 变更（crate 门面）；本节为
     db 种子语料全链组合用例章节（链路入口 = crate 公共 API 门面，组合用例挂靠规则）。
     语料 = db 种子构造器（经真实 store 域操作面，固定 id 直携——D2）+ 运行时合成磁盘
     产物树 + workflow.json 惰性字节样本；文档形态样本退役改造为零发现反例；归档前缀
     样本入 golden——D13 范围声明。fixtures/README.md 语料矩阵行随动（文档形态行改
     「零发现反例」语义、归档前缀样本行新增）。 -->

#### 待测功能

<!-- design.md 未声明 lib.rs 自有公共 API；本节承载语料矩阵 golden 对拍与显式重写流程（D13）。 -->

- 语料种子 id 归键: 全部构造器（seed_record / run_phase / open_only / run_start / run_finish 与组合行建档点）经 `ChangeStateRecord.id` 与载荷 `change_id` 携固定 id 字面量（逐字入 golden）；「uuid 形态」断言归 create 铸出路径（零固定值对拍——D2）
- 文档形态样本退役改造: build_document_form 磁盘目录树与惰性字节样本保留（字节零变化），语料职能改「零发现反例」——list 零呈现、change_detail（任意裸名 / 前缀名寻址）恒 None；corpus-document-form.json golden 出局删除（无 golden 对拍）
- 归档前缀样本入 golden: db archived 记录（裸名 + archived_at）+ 磁盘目录带 `YYYY-MM-DD-` 前缀 → list golden 归档组（裸名 + id + 月分组）与 detail golden（status=archived、9 站与 runs 全量——AC-2 golden 面）
- list golden db 单源: 磁盘-only 条目（active / archive 两树）自列表投影出局（零呈现）；月分组 archived_at 权威
- 全部 detail / list golden（`tests/golden/*.json` 全量）增 id 键；显式重写流程（`DESKTOP_GOLDEN_REWRITE=1`）后复核轮绿 + 人工确认留痕（D13）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| corpus_golden_test · 多 attempt / backtrack stale / 槽位全缺语料 | 正向 | 种子携固定 id → detail golden 重组逐字段 + id 键新增（现值面零漂移——显式重写重录） | 适配 |
| corpus_golden_test · worktree 建档 / run 全史 / run 中断语料 | 正向 | 固定 id 归键后 golden 增 id 键；worktree 归一占位 / 两 run 全史 / 标定语义零改动 | 适配 |
| corpus_golden_test · list 混合语料 db 单源 | 正向 | 磁盘-only（含前缀 / 无前缀）条目自列表投影出局；归档记录条目裸名 + id + archived_at 月分组；磁盘目录字节零变化（零呈现反例断言面） | 重写 |
| corpus_golden_test · 文档形态零发现反例 | 异常 | 磁盘目录树保留 + db 零记录 → list 零呈现该语料、change_detail（裸名 / 前缀名寻址）恒 None；惰性字节零进投影；该 golden 出局删除 | 重写 |
| corpus_golden_test · 归档前缀样本入 golden | 正向 | db archived 裸名 + 磁盘 `YYYY-MM-DD-` 前缀目录 → list 归档组（裸名 + id + archived_at 月）与 detail 全状态面 golden（AC-2 golden 半边） | 新增 |
| corpus_golden_test · 显式重写流程与键集断言 | 边界 | 重写后复核轮绿；diff 键集断言随动：detail 恒含 id 键 / list 条目含 id / 零退役键 / 归档前缀样本行在场（原「重写后 diff 范围键集」断言改写） | 重写 |
| corpus_golden_test · create 组合行铸 id | 正向 | 真件 create 建域 / 冲突双检查（同名 active 拒绝）/ fs 补偿按 id / 成功立即可见可发起——寻址断言自 name 改 id（outcome.id 非空 + 与库内逐字一致） | 重写 |
| corpus_golden_test · archive 双写组合行 | 正向 | archive(layout, store, id) 目录改名 + db 翻转 + 按月分组可达（id 种子） | 适配 |
| corpus_golden_test · 坏行语料行 | 边界 | fixtures/README 矩阵「坏行」行（store_test 直写注入承载）零改动 | 持衡 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 真实 tempfile workspace db（种子经 store 域操作面真件——固定 id）+ 真实磁盘产物树 + golden 快照文件真实读写；`DESKTOP_GOLDEN_REWRITE` 环境开关仅测试装置 | 本节全部用例 |

### packages/desktop/src/routes.tsx -> packages/desktop/src/app.test.tsx

<!-- 挂 AC-7（路由半边）。routes.tsx 不在测试配置 scope（test_resolve_paths 报
     「Not in test config scope」——见不可测试项 6），路由与选中 URL 行为断言驻既有
     src/app.test.tsx（proposal 测试文件清单行）；`/changes/:name` → `/changes/:id`。 -->

#### 待测功能

<!-- design.md 未声明 routes.tsx 的公共函数变更（AppRoutes 组件内部路由表行）；被测面为
     App 壳路由行为：URL 段 id 化与选中重置语义。 -->

- 路由表 `/changes/:id`: 清单行点击 → hash 落 `#/changes/<id>`；切页往返 / 根切换 → `#/changes`（无 id 段）
- fixture 与 invoke 载荷: ChangeList / ChangeDetail fixture 增 id；get_change_detail 载荷 `{ root, id }`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| app_test · 路由段 id 化 | 正向 | 详情进入后 hash == `#/changes/<fixture id>`；既有「无 :name 段」重置断言改「无 :id 段」（选中重置语义保留） | 重写 |
| app_test · fixture 与 invoke 载荷 | 边界 | 清单 / 详情 fixture 增 id；get_change_detail invoke 载荷键 `id`（段名置换） | 适配 |
| app_test · 既有路由族行 | 边界 | 启动重定向 / 三页切换 / 清单重挂重发 / 根切换抑制 / HashRouter 自含挂载——零触点 | 持衡 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC（进程边界） | 既有 vi.hoisted + `vi.mock('@tauri-apps/api/core')` invoke 装置沿用；updater / dialog mock 同理 | 本节全部用例 |

### packages/desktop/src/views/changes/change-list-view.tsx -> packages/desktop/src/views/changes/change-list-view.test.tsx

<!-- 挂 AC-7（行键 / navigate 半边）、AC-3（空态文案与零呈现 UI 半边）。既有测试
     扩展节：fixture 增 id；行键与 navigate 取 summary.id；归档行裸名；创建流转
     onCreated(id) → 导航 `/changes/<id>`。 -->

#### 待测功能

<!-- design.md 公共函数 / API 表未声明该视图导出函数（React 组件）；被测面为组件渲染与导航行为。 -->

- 行键 / navigate: 取 `summary.id`；归档行显示裸名；创建成功 `onCreated(id)` → 导航 `/changes/<id>`
- 空态文案随 db 单源修订（零磁盘语义）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| list-view · 行导航 id 化 | 正向 | 点击条目 → 显式 navigate 落 `/changes/<summary.id>`（原携带 change 名断言反转） | 重写 |
| list-view · 归档行裸名 | 正向 | 归档条目（DTO name 裸名 + id）→ 行展示裸名、行键 id（零日期前缀形态） | 新增 |
| list-view · 创建流转 | 正向 | create_change resolve（携 outcome.id）→ refresh + navigate `/changes/<id>`；失败不停流转留窗（原 pathname :name 断言改写） | 重写 |
| list-view · 空态文案修订 | 边界 | 空清单文案随 db 单源修订（零「未发现任何 change 目录」旧句） | 重写 |
| list-view · 状态面消费与分组行 | 正向 | active 运行中徽标 / 月分组（month null 置尾）/ created null 分支——fixture 增 id 后断言面零改动 | 适配 |
| list-view · 文档形态行退役 | 废弃 | `文档形态条目（status 缺席）照常入列且无状态标注` 随 DTO 面与 db 单源语义退役整体废弃 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC（进程边界） | `vi.mock('@tauri-apps/api/core')` 仅 mock invoke 返回清单 DTO 夹具（既有 vi.hoisted 装置）；视图内部 hooks（use-change-list）真实组合渲染 | 本节全部用例 |

### packages/desktop/src/views/changes/change-detail-view.tsx -> packages/desktop/src/views/changes/change-detail-view.test.tsx

<!-- 挂 AC-7（页内身份源半边）、AC-4（flow-empty 退役半边）。既有测试扩展节：
     useParams<'id'>；flow-empty 分支删除；面板传参 changeId + name。 -->

#### 待测功能

<!-- design.md 公共函数 / API 表未声明该视图导出函数（React 组件）；被测面为组件渲染行为与取数参数链。 -->

- useParams<'id'> 取数与深链: 页内身份源 = URL id（无 `detail.name` 作寻址残留）
- `flow-empty` 文档形态占位分支删除（未找到降级沿用——「文档形态」文案零消费）
- DetailLoaded 面板传参收敛: RunControlPanel / ArchivePanel / DetailDrawer 传 `changeId`（URL id）与 `name`（detail.name 展示面）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| detail-view · 路由参数 id 取数 | 正向 | 详情路由 `/changes/:id` → get_change_detail 以 (root, id) 调用恰一次（原 :name 断言重写） | 重写 |
| detail-view · flow-empty 退役 | 异常 | 任意 detail 形态 → 恒走图区；`flow-empty` testid 与「（文档形态：未建档，仅产物清单）」文案零出现（退役负断言——AC-4） | 重写 |
| detail-view · 未知 id 未找到降级 | 异常 | 深链未知 id → null 应答 → 「未找到该 change。」降级页且不崩 | 适配 |
| detail-view · 面板传参 changeId + name | 正向 | 头部区面板 / 抽屉接线：命令面 prop changeId == URL id、展示面 name == detail.name（单一身份源断言——AC-7） | 新增 |
| detail-view · 归档入口两态 | 正向 | status=active → 触发位；archived → 不渲染；链终态 refresh 回落已归档形态——断言面零改动 | 适配 |
| detail-view · run 控制与全史行 | 正向 | 发起 / 停止 / 重挂恢复 / 两 run 全史叠加 / 通知重查 / 转录链路——fixture 增 id 后断言面零改动 | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC（进程边界） | `vi.mock('@tauri-apps/api/core')`（invoke + Channel——既有 vi.hoisted 装置）；内部 hooks（useChangeDetail / useChangeFlowRun / useArchiveFlow）真实组合，fixture 经 mock IPC 流入后断言渲染输出 | 本节全部用例 |

### packages/desktop/src/views/changes/hooks/use-change-detail.ts -> packages/desktop/src/views/changes/hooks/use-change-detail.test.ts

<!-- 挂 AC-7（hooks 参数 id 半边）。既有测试扩展节：`useChangeDetail(root, id)`；
     invoke 载荷 `{ root, id }`；去抖 / 竞态 / 产物同周期逻辑零改动。 -->

#### 待测功能

- useChangeDetail(root: string | null, id: string | null) -> ChangeDetailState: 参数 id 化（取数 / 产物逐读与通知面语义不变）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| use-change-detail_test · 参数与载荷 id 化 | 正向 | refresh → invoke("get_change_detail", { root, id })；read_artifact / 统一视图载荷同式（原 change 键置换） | 适配 |
| use-change-detail_test · 空 id 早退 | 边界 | id 为 null → 零 invoke / 状态重置（原 change null 行置换） | 适配 |
| use-change-detail_test · 去抖与竞态行 | 边界 | notifyRefresh 去抖 / 尾随语义 / 旧响应丢弃 / 错误路径 / notifyTranscript 分流——零触点 | 持衡 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC（进程边界） | `vi.mock('@tauri-apps/api/core')` invoke mock（既有 vi.hoisted 装置） | 本节全部用例 |

### packages/desktop/src/views/changes/hooks/use-change-flow-run.ts -> packages/desktop/src/views/changes/hooks/use-change-flow-run.test.ts

<!-- 挂 AC-7（hooks 参数 id 半边）。既有测试扩展节：参数 `{ root, id, … }`；
     五命令调用载荷键 id；订阅生命周期零改动。 -->

#### 待测功能

- useChangeFlowRun(params: { root; id; activeRunPresent; onNotice }): 参数 id 化（四命令 + 订阅生命周期不变）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| use-change-flow-run_test · 载荷键 id | 正向 | start / stop / confirm / answer 载荷 `{ root, id }`；watch 补订 `{ root, id }`（原 change 键置换——含 autoNextPhase 显式出线行） | 适配 |
| use-change-flow-run_test · 守卫与分流行 | 边界 | id null no-op 零 invoke / 零 Channel；onNotice 五 kind 分流；终态释放——断言面零改动 | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC（进程边界） | invoke + Channel mock（既有 vi.hoisted 装置） | 本节全部用例 |

### packages/desktop/src/views/changes/hooks/use-archive-flow.ts -> packages/desktop/src/views/changes/hooks/use-archive-flow.test.ts

<!-- 挂 AC-7（hooks 参数 id 半边）。既有测试扩展节：参数 `{ root, id, onFinish }`；
     五命令 / 快照 / 补订载荷键 id。 -->

#### 待测功能

- useArchiveFlow(params: { root; id; onFinish }): 参数 id 化（五命令 + 重挂恢复不变）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| use-archive-flow_test · 载荷键 id | 正向 | preflight / start / stop / state / watch 载荷 `{ root, id }`（原 change 键置换——参数序含 channel 首参行） | 适配 |
| use-archive-flow_test · 归并与恢复行 | 边界 | reducer 归并 / onFinish 恰一次 / 挂载快照恢复与补订 / stop 与 error 面——零触点 | 持衡 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC（进程边界） | invoke + Channel mock（既有 vi.hoisted 装置） | 本节全部用例 |

### packages/desktop/src/views/changes/hooks/use-change-list.ts -> packages/desktop/src/views/changes/hooks/use-change-list.test.ts

<!-- 挂 AC-7（DTO id 消费适配半边）。design 行注明本文件改动为注释面、签名零变化——
     测试面仅 fixture 增 id 适配（DTO 类型随 bindings 自动跟随）。 -->

#### 待测功能

<!-- design.md 未声明该文件签名变更（注释面随动）；被测面为 DTO fixture 消费适配。 -->

- useChangeList(root): DTO `ChangeList`（条目增 id）消费——签名零变化

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| use-change-list_test · fixture 增 id 适配 | 边界 | 清单 DTO fixture 条目携 id → 状态面就位与既有断言面一致（零行为变化） | 适配 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC（进程边界） | invoke mock（既有 vi.hoisted 装置） | 本节全部用例 |

### packages/desktop/src/views/changes/hooks/use-session-transcript.ts -> packages/desktop/src/views/changes/hooks/use-session-transcript.test.ts

<!-- 挂 AC-6（透传持衡半边）。design 行注明注释随动、查询逻辑零改动——sourceRef 为
     不透明串精确透传，组装面（定式值替换）归 detail-drawer 章节；本节为持衡声明节
     （无断言改动）。 -->

#### 待测功能

<!-- design.md 未声明该文件查询逻辑变更（仅注释措辞随动）；被测面为 sourceRef 不透明透传语义。 -->

- useSessionTranscript: sourceRef 精确匹配反查（值形态不解析——身份段 id 替换零感知）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| use-session-transcript_test · 持衡声明 | 边界 | sourceRef 反查定式（source 恒 change + sourceRef 原样透传）/ 直查优先 / 实时归并 / refreshKey——断言面零改动（id 段为不透明串，查询逻辑不涉组装） | 持衡 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC（进程边界） | invoke mock（既有 vi.hoisted 装置） | 本节全部用例 |

### packages/desktop/src/views/changes/flow/detail-drawer.tsx -> packages/desktop/src/views/changes/flow/detail-drawer.test.tsx

<!-- 挂 AC-6（前端组装半边）。既有测试扩展节：prop `change → changeId`；
     `selectionRoleRefs` 三处 sourceRef 定式组装身份段改 changeId。 -->

#### 待测功能

- DetailDrawer props `change → changeId`: 三处 sourceRef 定式组装身份段 = changeId（`<id>/<phase>/<role>/<attempt>` 与 active 三 role 反查同式）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| detail-drawer_test · sourceRef 定式身份段 id | 正向 | active 节点选中 → 反查 sourceRef == `<固定 id>/<phase>/<executor 或 evaluator 或 decision>/<attempt>`（fixture 键改 id 形态——原 `test-change/...` 断言重写；与服务端写侧逐字一致——AC-6） | 重写 |
| detail-drawer_test · 槽位直查优先行 | 正向 | sessionId 直查 / 槽位缺席反查 / 三 tab 组装 / 决策槽双 null——断言面随 changeId prop 置换 | 适配 |
| detail-drawer_test · 二分节与壳布局行 | 边界 | 二分节 / 双列壳 / 空态占位 / refreshKey 重查——零触点 | 持衡 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC（进程边界） | `vi.mock('@tauri-apps/api/core')` invoke mock（既有 vi.hoisted 装置）；use-session-transcript 内部 hook 真实组合，fixture 经 mock IPC 投递 | 本节全部用例 |

### packages/desktop/src/views/changes/flow/run-control-panel.tsx -> packages/desktop/src/views/changes/flow/run-control-panel.test.tsx

<!-- 挂 AC-7（props 收敛半边）。既有测试扩展节：props 拆 `changeId`（命令面）+
     `name`（展示面）。 -->

#### 待测功能

- RunControlPanel props: `changeId`（命令面透传）+ `name`（展示面：aria-label / 文案取展示值）；面板自身不寻址

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| run-control-panel_test · props 收敛面 | 正向 | aria-label 取 name（展示可辨）；start / stop / confirm / answer 经注入动作透传（changeId 命令面归属——显名收敛后无 name / id 静默歧义） | 适配 |
| run-control-panel_test · 生命周期与卡片行 | 边界 | 主操作矩阵 / autoNextPhase 开关 / waitingConfirm / waitingAsk / error 与尾行记因——零触点 | 持衡 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 mock | 纯 props 注入直驱渲染（动作回调为入参——入参例外；零 IPC） | 本节全部用例 |

### packages/desktop/src/views/changes/flow/archive-panel.tsx -> packages/desktop/src/views/changes/flow/archive-panel.test.tsx

<!-- 挂 AC-7（props 收敛半边）。既有测试扩展节：props 拆 `changeId` + `name`
     （确认对话 / 进行面标题取展示值）；命令面 id 透传。 -->

#### 待测功能

- ArchivePanel props: `changeId`（命令面）+ `name`（展示面：确认对话 / 标题）；阶段清单与转录区呈现零改动

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| archive-panel_test · props 收敛面 | 正向 | 确认对话与进行面标题取 name；start / stop / 补订经注入动作透传（changeId 命令面） | 适配 |
| archive-panel_test · 阶段清单 / 摘要 / 错误面行 | 边界 | 六阶段序 / Merge 单行演进 / lean 咨询面 / 结果摘要三态 / warnings——零触点 | 持衡 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC（进程边界） | `vi.mock('@tauri-apps/api/core')`（转录区经 mock IPC 投递 session fixture——use-session-transcript 真实组合；内部 hook 替身装置不沿用） | 本节全部用例 |

### packages/desktop/src/views/changes/components/change-create-dialog.tsx -> packages/desktop/src/views/changes/components/change-create-dialog.test.tsx

<!-- 挂 AC-7（导航链路半边——CreateOutcome.id 消费）。既有测试扩展节：成功回调改携
     `outcome.id`。 -->

#### 待测功能

- ChangeCreateDialog 回调面: `onCreated: (id: string) => void`（成功回调携 `outcome.id`）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| change-create-dialog_test · 成功回调携 id | 正向 | 合法提交 → invoke("create_change") 恰一次（载荷 root / name / goal 不变）；成功即关窗并触发 onCreated(outcome.id) 恰一次（原 onCreated(name) 断言重写） | 重写 |
| change-create-dialog_test · trim 与透传行 | 边界 | 提交前 trim / free-form goal 透传 / 失败留窗行内错误——零触点 | 持衡 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC（进程边界） | invoke mock（既有 vi.hoisted 装置） | 本节全部用例 |

---

## 不可测试项

1. **AC-3 静态半边**：`crates/core/workflow/src/queries/list.rs` 源码零磁盘扫描触点（`scan_dir_names` / `scan_archive_dirs` / `locate_prefixed_archive_name` 段删除） — **原因**: 删除性静态约束，由 crate 编译（段删除后引用残留即编译失败）与守线 grep 承载；其行为半边（磁盘-only 零发现 / 目录字节零变化 / 月分组 archived_at 权威）已落 list_test 与 corpus 各节。
2. **AC-8 静态半边**：零迁移代码路径（`ChangeRecordV1` 与双向 `From` 整体删除、v3 零 `from` 链、零 `migrate` 调用）+ `open_global` 与主基线「旧文件惰性废弃」条款零触碰 — **原因**: 删除性 / 「不作为」类约束无进程内行为断言面；行为半边由 store_test 作废重建组（打开成功零报错 / 旧数据零可达 / 标记幂等 / 全局库不受波及）承载。
3. **AC-10 工具链门**：rust workspace 全量套件（src-tauri 根）/ 前端工程检查与 client:check（含 knip 零新增豁免）/ bindings 一致性守卫 / 前端 vite-plus 全量套件——全绿零新增豁免 — **原因**: 工具链级套件门，非单一测试文件可承载，归 test-execution 阶段承接验证（design AC-10 对齐行同口径）。
4. **AC-11 版本交付**：`packages/desktop/package.json` version 0.4.30 → 0.4.31（归档时执行；`tauri.conf.json` 自动跟随、`src-tauri/Cargo.toml` 不随动）+ `plugins/dev-team` 零改动 — **原因**: 静态版本声明与只读红线，无行为面可自动化；由 tasks 阶段六对账与红线自查 grep 承载。
5. **AC-9 流程半边**：golden 重写 diff 的人工确认与留痕 + `bindings:check` 守线 — **原因**: 流程性动作（人工审阅 diff 范围并留痕）不可自动化；其测试半边（重写后复核轮绿 + 键集断言 + 产物签面断言）已落 corpus_golden_test 与 bindings/mod_test 各节。
6. **routes.tsx 解析错误面**：`test_resolve_paths` 报 `packages/desktop/src/routes.tsx`「Not in test config scope」——无共置测试路径 — **原因**: 路由表无独立测试文件（沿既有布局）；路由行为断言驻 `packages/desktop/src/app.test.tsx`（该章节承载 URL 段 id 化与选中重置语义）。
7. **不建测试文件的模块**：`packages/desktop/src/types/generated/bindings.ts`（生成物——`bindings.test.ts` 解析路径不建文件；签面断言归 bindings/mod_test.rs 章节）、`packages/desktop/src/views/changes/flow/types.ts`（纯类型 / 注释面——`types.test.ts` 解析路径不建文件；`sourceRef` 定式描述语义经 detail-drawer 章节断言承载）、`packages/desktop/src-tauri/crates/core/workflow/Cargo.toml` 与 `packages/desktop/src-tauri/Cargo.toml`（`uuid` 依赖声明——编译期承载）、`packages/desktop/package.json`（版本行为见本项 4） — **原因**: 生成物 / 纯类型 / 依赖声明无自有行为，空框架章节自相矛盾且诱发空套件；行为去向见括号。
8. **磁盘与 git name 化面零扰动红线**（不变组件 + R8 文案）：change 目录名 / worktree 落位 / branch `change/<name>` / 归档目录前缀与后缀扫描判定零 diff；错误文案 id 语境呈现记录名 — **原因**: 零 diff 属守线 grep（tasks 阶段六）；文案属展示性断言，其行为半边经 archive_test（record.name 供给改名与 branch）与 commands 各节（错误映射 id 语境）适配置换承载，不设独立断言面。
9. **会话反查端到端半边**：source_ref 写入侧（walker / archive_flow 逐字断言）与读出侧（detail-drawer 组装断言）双侧一致已可测；真实 agent 会话产生与转录落库的端到端回放 — **原因**: 需真实 agent 引擎（非确定），假引擎不产真实会话行；双侧格式逐字断言构成「逐字一致」（AC-6 字面）的可测化。
10. **同名并存可读性（R7 / D9 留痕）**：active + archived 同名两行的 title 展示演进 — **原因**: 本变更不新增展示面（D9 不新增行内归档日期辅助信息），留后续变更以 id 主键为前提偿还；无断言面。
11. **作废数据损失面（R1 / D3）**：旧库历史 phase / run / 会话转录数据的真实损失 + 磁盘-only CLI change 在 desktop 不可见（deliberate 能力回退） — **原因**: 损失与回退语义即接受面（用户拍板），断言面为「旧数据零可达」与「磁盘-only 零呈现」（store_test / list_test 承载），非数据保真面。
