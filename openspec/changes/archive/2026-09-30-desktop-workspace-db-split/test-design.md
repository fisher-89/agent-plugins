# 测试设计: desktop-workspace-db-split

> **日期**: 2026-09-29

---

## 验收范围

| AC ID | 验收条件 | 被测文件或模块 |
|--------|---------|----------|
| AC-1 | 全局库仅注册 `WorkspaceRecord`；workspace 库注册 `AgentRunRecord` / `AgentEventRecord` / `ExploreRecord`；全局库与 workspace 库（`home_dir()/.dev-team/workspaces/` 子树）文件面分离，测试断言同 root 跨重开派生同一路径 | packages/desktop/src-tauri/crates/infra/store/src/store_test.rs |
| AC-2 | 集成测试：`add_workspace` 后全局库含注册记录；对某 workspace 执行 `create_explore_record` 与 `begin_agent_run` 后，记录落在该 workspace 库，全局库无混入 | packages/desktop/src-tauri/crates/infra/store/src/store_test.rs |
| AC-3 | 同 root 多次 `for_root` 复用同一打开实例（不触发 redb 文件锁冲突）；不同 root 各自独立实例；路径派生为单点纯函数，消费侧零派生逻辑 | packages/desktop/src-tauri/crates/infra/store/src/store_test.rs |
| AC-4 | 预置旧四模型单库文件与旧路径：新布局启动照常成功、不读旧文件、不改名不删除；全局库与 workspace 库从空开始按新布局写入；无任何迁移 / 格式探测代码 | packages/desktop/src-tauri/crates/infra/store/src/store_test.rs |
| AC-5 | `agent_runs` / `agent_run_events` / `agent_run_chain` / `agent_stop` 携 root 路由至对应 workspace 库；workspace A 与 B 各有 runs（含同 id 并行）时互不可见、`agent_stop` 不跨库误停；调试页运行清单仅呈现当前 workspace 历史 | packages/desktop/src-tauri/src/commands/exec/mod_test.rs |
| AC-6 | `db_models` / `db_records` 按 scope 返回对应库的模型清单与记录，两库互不混列；信封 API 与查看器零模型特定代码（git diff 无 per-model 分支） | packages/desktop/src-tauri/src/commands/db/mod_test.rs |
| AC-7 | remove 后注册记录消失、workspace db 文件仍在；重新 add 同 root 后 explore 清单与会话链历史完整可读 | packages/desktop/src-tauri/src/commands/workspaces/mod_test.rs |
| AC-8 | 审查确认四模型 native_model id / version 与字段面不变；前端 DTO 除新增 root / scope 参数外不变；`tests/golden` 契约未触碰 | packages/desktop/src-tauri/crates/infra/store/src/model_test.rs |
| AC-9 | desktop-data-dimensions 拆分后语义自洽：workspace 维度落全局目录 per-workspace db、不进 repo；workflow 过程数据维持 user 维度裁定不变 | —（见不可测试项 1） |

路由说明（双向对齐补充）：AC-2 / AC-5 / AC-6 / AC-7 的跨模块组合用例挂靠链路入口模块——持久化链挂 `store.rs`（`WorkspaceStores` 为两库入口）、命令链挂各命令轨道 `mod.rs`；explores 轨道章节回溯 AC-2 / AC-5（root 路由半边），前端 hook / 视图 / App 壳层章节回溯 AC-5 / AC-6 的 invoke 入参面半边。测试框架：Rust 侧为 `cargo test`（workspace 级注册于 `src-tauri` 根，共置 `*_test.rs` 模块），TS 侧为 `vite-plus`（vp test，共置 `*.test.ts(x)`）。

---

## 单元测试

### packages/desktop/src-tauri/crates/infra/store/src/store.rs -> packages/desktop/src-tauri/crates/infra/store/src/store_test.rs

<!-- AC-1 / AC-2 / AC-3 / AC-4 承载章节。组合用例（WorkspaceStores 为两库持久化链入口）挂靠本章节：注册表 → workspace 库分流、两库隔离均经 WorkspaceStores 公共面真实组合 Store + envelope，不设独立组合测试区。
     路径派生单点（workspace_db_file_name / workspace_db_path，pub(crate) 收口点，design 未列入公共 API 表）经 for_root 与 workspaces/ 子树文件面间接断言，不虚构导出条目。 -->

#### 待测功能

- WorkspaceStores.open(data_root): 打开全局库（`data_root/desktop-global.redb`）并记录 `workspaces/` 子树根；任一失败 `Err`（setup fail fast）
- WorkspaceStores.global(): 全局库实例（user 维度注册表操作面）
- WorkspaceStores.for_root(root): 按 canonical root 派生路径解析 workspace 库实例；per-root 缓存复用（进程内单开）；坏文件 `Err`
- Store.open_global(path): user 维度组打开（仅 `WorkspaceRecord`）
- Store.open_workspace(path): workspace 维度组打开（run / 事件 / explore 三模型）
- Store.open(path): 退役——拆分至 `open_global` / `open_workspace`（注入式单文件句柄语义由两者继承）
- Store.list_models(): 签名不变；按实例维度只列本库模型
- Store.scan(): 签名不变；跨维度模型名按「未知模型」`Err`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| AC-1 双库布局：模型注册分组 | 正向 | open_global 后 list_models 仅列 workspace 一行（user 组仅 WorkspaceRecord）；open_workspace 后仅列 agent_run / agent_event / explore 三行——两组静态注册无交叉 | 新增 |
| AC-1 双库布局：模型注册分组 | 正向 | WorkspaceStores::open 后经 global() 注册 WorkspaceRecord（add_workspace / list_workspaces 走全局库） | 新增 |
| AC-1 双库布局：模型注册分组 | 边界 | workspace 库实例 scan("workspace") 与 global 实例 scan("agent_run") 均按「未知模型」Err——跨维度模型名不可达（模型分组使混入在打开点不可能） | 新增 |
| AC-1 双库布局：模型注册分组 | 边界 | open_global / open_workspace 于不存在路径创建 db 文件与父目录成功、空库可 list（既有「空文件视同不存在」语义由两构造器继承） | 新增 |
| AC-1/AC-3 workspace 库路径派生单点 | 正向 | 同 root 经两次 WorkspaceStores 生命周期（open → drop → 重开）派生同一 db 文件路径：先写入的 run / explore 数据重开后完整可读（同 root 跨重开派生同一路径） | 新增 |
| AC-1/AC-3 workspace 库路径派生单点 | 边界 | 派生文件名形如「可读段-32位小写hex哈希.redb」，不含路径分隔符与盘符、落 `workspaces/` 子树（哈希成分抗碰撞、文件名不含完整路径） | 新增 |
| AC-1/AC-3 workspace 库路径派生单点 | 边界 | 可读段取 dir_name 清洗：超 24 字符按 char boundary 截断、OS 非法字符置换 `_`、尾部 `.` 与空格去除；可读段为空回退纯哈希名 | 新增 |
| AC-1/AC-3 workspace 库路径派生单点 | 边界 | 同目录大小写不同书写 / 尾分隔符 / 正反斜杠混写经前置归一派生同一文件并命中同一缓存实例（dunce canonical 口径同源） | 新增 |
| AC-2 注册表 → workspace库分流写入（组合） | 正向 | 组合链：WorkspaceStores::open → global().add_workspace 注册 → for_root(root) 上 create_explore_record 与 begin_agent_run → global 库仅含 workspace 模型行、workspace 库含 run / explore 行——全局库无混入 | 新增 |
| AC-2 注册表 → workspace库分流写入（组合） | 边界 | 两 workspace 各自 for_root 写入（各库同 id=1 并行）互不串库：A 库清单与 scan 不含 B 的任何记录 | 新增 |
| AC-3 进程内单开与复用 | 正向 | 同 root 连续两次 for_root 返回同一 Arc 实例（指针等同）且可连续读写（不二次打开文件、无 redb 锁冲突） | 新增 |
| AC-3 进程内单开与复用 | 正向 | 不同 root 各自独立实例：实例不同、写入互不可见、文件各自独立 | 新增 |
| AC-3 进程内单开与复用 | 异常 | 派生路径上的 db 文件损坏时 for_root 返回 Err（坏文件使用时暴露，不静默降级为空库） | 新增 |
| AC-4 全新文件组冷启动（零迁移） | 正向 | 预置旧布局单库文件（desktop-store.redb、四模型数据）于数据根：新布局启动照常成功，全局库与 workspace 库从空开始按新布局写入，旧文件名与字节保持原样（不读、不改名、不删除） | 新增 |
| AC-4 全新文件组冷启动（零迁移） | 异常 | 全局库文件损坏时 WorkspaceStores::open 返回 Err（setup fail fast 口径，不静默降级） | 新增 |
| 单文件打开入口拆分 | 废弃 | 原 Store::open 单库四模型打开用例退役：打开入口断言改写为 open_global / open_workspace 按维度分组（list_models 全量四模型断言随之退役） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 文件系统 / db 文件（进程边界） | 不 mock：tempfile TempDir 真实开库（存储层不 mock，既有惯例）；Env 装置扩出数据根 + workspace 根两级临时目录，旧布局残留文件与损坏文件以真实非法字节预置于磁盘 | 全部 describe（冷启动预置、坏文件异常） |
| 系统时钟（全局变量） | 不 mock：时间戳字段仅记录入库值；排序断言用显式注入的 started_at 与返回值比较 | run / explore 写入与清单排序用例 |

### packages/desktop/src-tauri/crates/infra/store/src/model.rs -> packages/desktop/src-tauri/crates/infra/store/src/model_test.rs

<!-- design.md 未声明该文件的公共 API 变更：本变更仅维度归属文档注释修正（AC-8 强制结论——模型 struct 字段面与 native_model id / version 零变化）。不新增用例、不设断言；既有「事件键打包 + 嵌装往返 + 三枚举线格式」回归用例继续执行，承载 AC-8 的可自动化半边（shape 往返不因拆分漂移）。 -->

#### 待测功能

<!-- design.md 公共函数 / API 表对 model.rs 零声明（仅注释修正）；既有回归用例继续承载，不虚构条目。 -->

#### 用例

<!-- 不设新增用例：AC-8 可自动化半边由既有回归用例（键打包、嵌装往返、三枚举线格式）继续执行承载；静态审查子句见不可测试项 2。 -->

#### Mock策略

<!-- 无 mock：既有用例为内存构造，无 IO 无进程边界；编解码经 native_model 封装内存往返，不重复验证 serde 自身语义（既有口径）。 -->

### packages/desktop/src-tauri/crates/infra/store/src/lib.rs -> packages/desktop/src-tauri/crates/infra/store/src/lib_test.rs

<!-- design.md 未声明该文件的公共 API 变更：导出面为 WorkspaceStores / DbDimension 的 pub use 再导出 + crate 文档由「单库四模型」改写为双库布局。纯导出面零运行时行为，不应创建 lib_test.rs（零用例测试文件）；导出正确性由消费方模块用例编译期承载。详见不可测试项 5。 -->

#### 待测功能

<!-- design.md 公共函数 / API 表对 lib.rs 零声明（纯再导出面）；不设条目。 -->

#### 用例

<!-- 不设用例；不应创建 lib_test.rs（防空套件），见不可测试项 5。 -->

#### Mock策略

<!-- 不适用：零运行时行为，无进程边界。 -->

### packages/desktop/src-tauri/crates/infra/store/src/envelope.rs -> packages/desktop/src-tauri/crates/infra/store/src/envelope_test.rs

<!-- design.md 未声明该文件的公共函数 API（信封实现体为 pub(crate)）：本变更为 ModelEntry 注册表行增 dimension 标签 + list_models / scan 按实例维度过滤（AC-6 的信封半边，分维度是数据行非分支代码）。维度过滤的行为断言经 Store::list_models / Store::scan 公共面在 store_test.rs 承载（信封实现体经公共 API 委托触达的既有惯例不变）；本文件既有「注册表结构 / 分页边界矩阵 / 未知模型名异常面」回归用例继续执行，不新增用例。 -->

#### 待测功能

<!-- design.md 公共函数 / API 表对 envelope.rs 零声明（pub(crate) 实现体不列）；维度过滤经 store.rs 公共面用例覆盖，不虚构条目。 -->

#### 用例

<!-- 不设新增用例：维度标签过滤由 store_test.rs 的 list_models / scan 分维度断言经真实组合覆盖；既有分页边界矩阵与未知模型名回归继续执行（AC-6 混列防线）。 -->

#### Mock策略

<!-- 不 mock：既有用例 tempfile 真开库并经 Store 公共 API 写入构造数据；存储层不 mock 口径不变。 -->

### packages/desktop/src-tauri/src/main.rs -> packages/desktop/src-tauri/src/main_test.rs

<!-- design.md 未声明该文件的公共 API（setup 装配：数据根注入 → WorkspaceStores::open → app.manage；DB_FILE_NAME 常量演进为 DATA_DIR）。Tauri 进程入口装配需真实 Wry runtime 与 home_dir 环境，进程内单测不可行（见不可测试项 4）；fail fast 口径由 store_test.rs 的 WorkspaceStores::open 异常用例承载。不应创建 main_test.rs。 -->

#### 待测功能

<!-- design.md 公共函数 / API 表对 main.rs 零声明（进程入口装配，非可单测 API）；不设条目。 -->

#### 用例

<!-- 不设用例；不应创建 main_test.rs，见不可测试项 4。 -->

#### Mock策略

<!-- 不适用：进程入口装配需真实 runtime 与环境路径，mock 装配无验证价值。 -->

### packages/desktop/src-tauri/src/commands/workspaces/mod.rs -> packages/desktop/src-tauri/src/commands/workspaces/mod_test.rs

<!-- AC-7 承载章节。组合用例（remove_workspace 命令为链路入口）挂靠本章节：命令面 → WorkspaceStores 缓存复用 → workspace 库历史可读，真实组合 store crate，不设独立组合测试区。 -->

#### 待测功能

- list_workspaces(): State 切 `WorkspaceStores`，注册表操作走全局库；IPC 面不变
- add_workspace(): 注册成功后 `for_root` 预开对应 workspace 库（fail fast，坏文件注册时以 `Err` 暴露）；IPC 面不变
- remove_workspace(): 仅删注册记录，db 文件与缓存实例保留；IPC 面不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| workspace命令面 → workspace库历史保留（AC-7 组合） | 正向 | 组合链：add_workspace 注册并预开 → for_root 库种 explore 记录与会话链 runs → remove_workspace 后注册记录消失且 workspace db 文件仍在磁盘 → 重新 add 同 root → list_explore_records 与会话链历史完整可读 | 新增 |
| workspace命令面 → workspace库历史保留（AC-7 组合） | 边界 | remove_workspace 后派生路径上 db 文件名与字节保持原样（不自动清理）；重加同 root 经缓存实例即读即得（无二次文件打开） | 新增 |
| 注册表操作 → 全局库 | 正向 | add_workspace 后 global() 清单含注册记录，且对应 workspace 库文件已创建（预开校验落位） | 新增 |
| 注册表操作 → 全局库 | 异常 | workspace 库文件损坏时 add_workspace 返回 Err 且注册记录保留（先注册后预开；重加同 root upsert 幂等并再次校验） | 新增 |
| State 切换回归 | 正向 | 三命令经命令面与直连 WorkspaceStores 公共 API 结果 serde 一致（薄包装不加工；IPC 面不变） | 新增 |
| State 切换回归 | 边界 | remove 未注册 root 返回 false 幂等；空 root add 为 Err、remove 为 false（blank root 语义回归） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| Tauri 运行环境 | tauri::test::mock_app()（MockRuntime，无窗口无事件循环）manage 真实 WorkspaceStores（tempdir 真开全局库）后经 app.state 取 State；沿用既有装置模式 | 全部 describe |
| 文件系统 / db 文件（进程边界） | 不 mock：tempfile 真开库；预开校验用的损坏 workspace 库文件以真实非法字节写盘于派生路径 | 预开校验、文件保留类用例 |

### packages/desktop/src-tauri/src/commands/explores/mod.rs -> packages/desktop/src-tauri/src/commands/explores/mod_test.rs

<!-- 回溯 AC-2 / AC-5（explore 写读经 for_root 路由至 workspace 库的半边）；命令签名与线格式零变化。 -->

#### 待测功能

- scan_explores(): State 切 `WorkspaceStores`，绑定过滤读 `for_root(&root)` 库；IPC 面不变
- list_explore_records(): 经 `for_root` 路由；blank root 空结果
- create_explore_record(): 经 `for_root` 路由；blank root `Err`
- rename_explore_record(): 经 `for_root` 路由；blank root `Err`
- delete_explore_record(): 经 `for_root` 路由，级联删除收敛同一 workspace 库内；blank root `Err`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| explore命令面 → workspace库路由 | 正向 | create_explore_record 携 root 经 for_root 落该 workspace 库：命令写后经 WorkspaceStores::for_root(root) 读回同记录；scan_explores 绑定过滤读同库 | 新增 |
| explore命令面 → workspace库路由 | 正向 | rename 保主键、delete 级联名下 runs+events 均在同一 workspace 库内收敛（级联语义回归 + 路由核对） | 新增 |
| explore命令面 → workspace库路由 | 边界 | 两 workspace 同名建档各自独立：A 库建档后 B 库清单为空、同名互不冲突（分库天然隔离，原「同 root 过滤」断言演进为跨库隔离断言） | 新增 |
| explore命令面 → workspace库路由 | 边界 | blank root：list / scan 返回空结果；create / rename / delete 返回 Err（blank root 纪律矩阵） | 新增 |
| explore命令面 → workspace库路由 | 异常 | 同 (root, name) 重复建档 Err；非法记录名（空串 / 含路径分隔符）Err（存量回归） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| Tauri 运行环境 | mock_app() manage 真实 WorkspaceStores（tempdir 真开全局库与各 workspace 库） | 全部 describe |
| workflow 查询层 | 不 mock：scan_explores 真实组合 foundation layout resolve 于 tempdir workspace 根，笔记文件真实写盘（导入绑定过滤求差） | scan_explores 用例 |

### packages/desktop/src-tauri/src/commands/exec/mod.rs -> packages/desktop/src-tauri/src/commands/exec/mod_test.rs

<!-- AC-5 承载章节。组合用例挂靠本章节：agent_stop 复合键寻址（RunStopRegistry 为 agent.rs 的类型定义修改，经命令面入口承载）、agent_start → for_root 落库（经 start_agent_run_with 泛型缝，装置复用 agent_test.rs 假 runner）。 -->

#### 待测功能

- agent_start(): IPC 面不变；blank root `Err`；落库经 `for_root` 至当前 workspace 库
- agent_stop(root, run_id): 复合键 `(root, run_id)` 寻址不跨库误停；miss 幂等 `Ok`
- agent_runs(root): 清单收窄为当前 workspace 历史（started_at 降序）
- agent_run_events(root, run_id): 重放该库 run 事件（seq 升序）
- agent_run_chain(root, source, source_ref): 还原本库链（发起顺序）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| exec查询命令面 → workspace库 root 寻址 | 正向 | agent_runs(root) 仅返回该 workspace 库 runs；两 workspace 各有 runs（各库同 id=1 并行）时互不可见（同 id 并行隔离） | 新增 |
| exec查询命令面 → workspace库 root 寻址 | 正向 | agent_run_events(root, run_id) 重放该库 run 事件；agent_run_chain(root, source, source_ref) 还原本库链且不受他库同定位串干扰 | 新增 |
| exec查询命令面 → workspace库 root 寻址 | 边界 | blank root 时 agent_runs / agent_run_events / agent_run_chain 返回空结果（blank root 纪律矩阵：查询空结果语义） | 新增 |
| agent_stop 复合键寻址（组合） | 正向 | 注册表登记 (rootA, id=1) 与 (rootB, id=1) 两句柄：agent_stop(rootB, 1) 仅 B 命中、A 句柄存活可再 stop（不跨库误停） | 新增 |
| agent_stop 复合键寻址（组合） | 边界 | agent_stop 对未注册 (root, run_id) 幂等 Ok 不报错（miss 语义 + 复合键化回归） | 新增 |
| agent_start → for_root 落库（组合） | 正向 | start_agent_run_with（假 runner）经 WorkspaceStores 预解析 for_root：running 记录提前 resolve 且落该 root 的 workspace 库、事件随运行追加同库、终态经 Channel 流出 | 新增 |
| agent_start → for_root 落库（组合） | 异常 | blank root 的 agent_start 携空 root 经命令委托编排缝期待 Err（无 cwd 无从发起；命令体首参 Wry 句柄 MockRuntime 不可构造，装置复用 for_root 落库组合用例的 FakeRunner）；PATH 隔离触发失败分支装置复用 | 新增 |
| 全局清单语义退役 | 废弃 | 无 root 入参的 agent_runs / agent_run_events / agent_run_chain / agent_stop 用例形态退役（跨库聚合清单与裸 id 寻址随 id 域内化废弃） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| Tauri 运行环境 | mock_app() manage WorkspaceStores + RunStopRegistry（须先于 start 托管，沿用既有装置） | 全部 describe |
| agent CLI 子进程（进程边界） | FakeRunner 假 runner（装置复用 agent_test.rs）替代真实 ClaudeCliRunner；agent_start 正向不直测 Wry 句柄，经 start_agent_run_with 泛型缝承载 | agent_start → for_root 落库 describe |
| PATH 环境变量（运行环境） | 隔离 PATH 触发失败分支；修改以共享互斥锁（PATH_LOCK）串行化 | agent_start 异常用例 |
| Channel（IPC 边界） | capturing_channel 捕获流出消息，断言事件与终态 Record 信封照常流出 | agent_start 组合用例 |
| 文件系统 / db 文件 | 不 mock：tempfile 真开两个 workspace 库（同 id 并行场景） | 全部 describe |

### packages/desktop/src-tauri/src/commands/exec/agent.rs -> packages/desktop/src-tauri/src/commands/exec/agent_test.rs

<!-- design.md 未声明该文件的公共函数 API（start_agent_run / drive_agent_run 为 pub(crate) 编排收口；RunStopRegistry 为类型定义修改——句柄表键演进 (root, run_id)、pub(crate) 方法面增 root 参）。复合键寻址与 for_root 落库路由的行为用例挂靠命令面入口，在 exec/mod_test.rs 的「agent_stop 复合键寻址」「agent_start → for_root 落库」describe 承载（真实组合 WorkspaceStores + RunStopRegistry，不设独立编排用例区）；本文件既有编排时序回归（假 runner 装置、EOF 收敛优先级）继续执行，不新增用例。 -->

#### 待测功能

<!-- design.md 公共函数 / API 表对 agent.rs 零声明（pub(crate) 收口点不列）；RunStopRegistry 复合键行为经 exec/mod.rs 命令面用例覆盖，不虚构条目。 -->

#### 用例

<!-- 不设新增用例：复合键寻址与 for_root 落库路由由 exec/mod_test.rs 组合用例承载；既有编排时序回归继续执行（drive_agent_run 签名不变，仍收 &Store）。 -->

#### Mock策略

<!-- 不新增 mock：既有装置（FakeRunner / capturing_channel / PATH_LOCK）由 exec/mod_test.rs 组合用例复用；本文件不独立设用例。 -->

### packages/desktop/src-tauri/src/commands/db/mod.rs -> packages/desktop/src-tauri/src/commands/db/mod_test.rs

<!-- AC-6 承载章节。DbDimension scope 寻址两库；信封 API 与零模型特定代码约束的行为半边（两库互不混列）在本章节断言。 -->

#### 待测功能

- db_models(scope, root): Global 走 `global()`（忽略 root）；Workspace 走 `for_root`（blank root 空结果）
- db_records(scope, root, model, offset, limit): scope 寻址两库；信封分页语义不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| db查看命令面 → 双库 scope 寻址 | 正向 | scope=Global 忽略 root：db_models 仅返回 workspace 一行（含计数）；db_records 扫描全局库记录信封 | 新增 |
| db查看命令面 → 双库 scope 寻址 | 正向 | scope=Workspace + root：db_models 返回 agent_run / agent_event / explore 三行；db_records 按主键自然序分页扫描该 root 的 workspace 库（分页语义回归） | 新增 |
| db查看命令面 → 双库 scope 寻址 | 边界 | 两库互不混列：Global scope 不出现 agent_run / agent_event / explore 行、Workspace scope 不出现 workspace 行；跨维度模型名扫描 Err | 新增 |
| db查看命令面 → 双库 scope 寻址 | 边界 | Workspace scope blank root：db_models 与 db_records 同口径返回空结果（blank root 纪律矩阵） | 新增 |
| db查看命令面 → 双库 scope 寻址 | 边界 | 未知模型名 reject；limit 超 500 截断（信封分页语义回归） | 新增 |
| 无 scope 入参形态退役 | 废弃 | 无 scope / root 入参的 db_models / db_records 用例形态随命令入参面演进退役 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| Tauri 运行环境 | mock_app() manage 真实 WorkspaceStores（tempdir 真开全局库与两个 workspace 库） | 全部 describe |
| 文件系统 / db 文件 | 不 mock：tempfile 真开库，记录经真实写入构造分页库存与计数 | 分页 / 混列断言用例 |

### packages/desktop/src/hooks/use-agent-chat.ts -> packages/desktop/src/hooks/use-agent-chat.test.ts

<!-- 回溯 AC-5 的前端入参面半边：链还原与停止 invoke 携 root；hook 对外签名与镜像语义不变。 -->

#### 待测功能

- useAgentChat(params): 对外签名不变；链还原经 `agentRunChain(params.root, source, sourceRef)` 与逐 run `agentRunEvents(params.root, run.id)` 取数；停止经 `agentStop(params.root, runId)`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| useAgentChat 链还原与停止携 root | 正向 | 链还原 invoke agentRunChain 携 (root, source, sourceRef) 三参，逐 run invoke agentRunEvents 携 (root, run.id)（入参面更新断言，镜像还原语义回归） | 新增 |
| useAgentChat 链还原与停止携 root | 正向 | stop 调 agentStop 携 (root, runId)；Channel 终态 Record 回流照常收敛 | 新增 |
| useAgentChat 链还原与停止携 root | 废弃 | 无 root 的 agentRunChain(source, sourceRef) / agentStop(runId) 调用形态断言随命令入参面演进而退役 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC 进程边界（@tauri-apps/api/core） | invoke 按命令名分发替身（可切换 resolve/reject 并记录调用序列，断言 root 等入参）；Channel 以可编程 class 捕获 onmessage，测试直接投递 Event / Record 信封 | 全部 describe |
| 适配层与 transport | 不 mock：agent-adapter 与镜像语义真实参与，不 mock ai 的 useChat | 链还原镜像断言 |

### packages/desktop/src/views/agent/hooks/use-agent-run-history.ts -> packages/desktop/src/views/agent/hooks/use-agent-run-history.test.ts

<!-- 回溯 AC-5 的前端入参面半边：运行清单收窄为当前 workspace 历史（root 入参化 + null 空态）。 -->

#### 待测功能

- useAgentRunHistory(root): `root: string | null` 入参透传 `agentRuns(root)` / `agentRunEvents(root, runId)` 两条取数轨道；root 为 null 跳过 invoke 保持空态

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| useAgentRunHistory root 入参 | 正向 | 传 root 挂载即 invoke agentRuns(root)；openRun 后 invoke agentRunEvents(root, runId)（两条取数轨道均透传） | 新增 |
| useAgentRunHistory root 入参 | 边界 | root 为 null 时不发起任何 invoke，runs 保持空态（跳过取数分支） | 新增 |
| useAgentRunHistory root 入参 | 边界 | refresh() 重取仍携同一 root（清单收窄语义在重取后保持） | 新增 |
| useAgentRunHistory root 入参 | 废弃 | 无 root 入参的挂载取数形态（全局运行清单）随收窄语义退役 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC 进程边界（@tauri-apps/api/core） | invoke 按命令名分发替身（固定 fixture 并记录调用序列，断言 root 入参与 null 跳过） | 全部 describe |

### packages/desktop/src/views/agent/agent-debug-view.tsx -> packages/desktop/src/views/agent/agent-debug-view.test.tsx

<!-- design.md 未声明该文件的公共 API 变更：root 透传 useAgentRunHistory(root)，页面已有 root prop、接线即可（design 修改内容行）。既有用例（root prop 渲染与清单挂载）继续回归，不新增用例。 -->

#### 待测功能

<!-- design.md 公共函数 / API 表对 agent-debug-view.tsx 零声明（root prop 已存在，仅 hook 调用接线）；不虚构条目。 -->

#### 用例

<!-- 不设新增用例：root 透传接线的行为由 use-agent-run-history.test.ts（hook 入参面）与本文件既有渲染回归共同覆盖。 -->

#### Mock策略

<!-- 不新增 mock：既有 invoke 替身装置继续用于既有回归用例。 -->

### packages/desktop/src/views/explores/hooks/use-explore-session.ts -> packages/desktop/src/views/explores/hooks/use-explore-session.test.ts

<!-- design.md 未声明该文件的公共 API 变更——proposal 点名核对任务落点：root 已为入参且经 useAgentChat 承载全部新入参，本文件预期零修改（波及时同步注释）。既有用例继续回归，不新增用例。 -->

#### 待测功能

<!-- design.md 公共函数 / API 表对 use-explore-session.ts 零声明（预期零修改的核对项）；不虚构条目。 -->

#### 用例

<!-- 不设新增用例：既有用例（root 透传、会话链经 useAgentChat）继续回归；若实现波及本文件，由 test-gen 按波及面补充注释同步用例。 -->

#### Mock策略

<!-- 不新增 mock：既有 invoke 替身装置继续用于既有回归用例。 -->

### packages/desktop/src/views/db/hooks/use-db-inspector.ts -> packages/desktop/src/views/db/hooks/use-db-inspector.test.ts

<!-- 回溯 AC-6 的前端入参面半边：scope 态 + root 入参化取数；取数仍全部用户显式动作触发，无轮询。 -->

#### 待测功能

- useDbInspector(root): 增 `root: string` 入参与 scope 态（默认 workspace 库，暴露 `scope` / `setScope`）；invoke 更新为 `dbModels(scope, root)` / `dbRecords(scope, root, model, offset, PAGE_SIZE)`；scope / root 变更重置选中模型与分页并重取

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| useDbInspector scope/root 入参化 | 正向 | 挂载 invoke dbModels 携 (scope, root)，默认 scope 为当前 workspace 库 | 新增 |
| useDbInspector scope/root 入参化 | 正向 | selectModel 后 invoke dbRecords 携 (scope, root, model, offset, PAGE_SIZE) 入参齐备 | 新增 |
| useDbInspector scope/root 入参化 | 边界 | setScope 切换后重置选中模型（复位 null）与分页（offset 回 0）并重取清单；root 变更同口径重置重取 | 新增 |
| useDbInspector scope/root 入参化 | 边界 | 翻页 / 刷新存量语义回归（PAGE_SIZE 页长、hasMore 判据、第 0 页 prevPage 不动） | 新增 |
| useDbInspector scope/root 入参化 | 废弃 | 无入参 useDbInspector() 的 dbModels() / dbRecords(model, offset, limit) 调用形态随入参面演进退役 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC 进程边界（@tauri-apps/api/core） | invoke 按命令名分发替身（db_models 恒回清单、db_records 按 offset/limit 切库存，记录调用序列断言 scope / root 入参）；沿用既有替身模式 | 全部 describe |

### packages/desktop/src/views/db/db-inspector-view.tsx -> packages/desktop/src/views/db/db-inspector-view.test.tsx

<!-- 回溯 AC-6 的前端呈现面半边：scope 双 tab 按钮组（aria-pressed 同 agent 页切换行模式）+ root prop 透传。 -->

#### 待测功能

- DbInspectorView(props: { root }): 接收 root prop；模型区头部 scope 双 tab 切换（全局库 / workspace 库，默认 workspace 库）；空态 / inline 持久错误态不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| DbInspectorView scope 双 tab | 正向 | 渲染「全局库 / workspace 库」双 tab 按钮组，默认 workspace 库 aria-pressed=true（同 agent 页切换行模式） | 新增 |
| DbInspectorView scope 双 tab | 正向 | 点击全局库 tab：aria-pressed 切换、模型清单重取且选中模型复位（经 hook 重置语义联动） | 新增 |
| DbInspectorView scope 双 tab | 边界 | 空态与 inline 持久错误态不随 scope 切换变形（查询轨 error 态回归） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC 进程边界（@tauri-apps/api/core） | invoke 按命令名分发替身 + @testing-library 渲染断言（aria-pressed / 空态 / 错误态）；useDbInspector 内部协作者不 mock，组件真实组合 hook 经 invoke 面驱动（同 agent-debug-view 模式） | 全部 describe |
| sonner toast（第三方 spy） | vi.mock sonner 以 vi.fn 替身注入 toast.error / success / info：查询轨错误呈现断言「无任何 toast 调用」（inline 持久双轨语义） | 错误态与 scope 切换用例 |

### packages/desktop/src/app.tsx -> packages/desktop/src/app.test.tsx

<!-- design.md 未声明 app.tsx 的公共 API 变更（App 根组件为壳层装配：当前 root 经 useChangeList 常驻、页面经 routes.tsx 逐页下传，页面内取数入参已由各自 hook / 视图章节声明）。本章节为 proposal 测试文件清单点名落点：壳层导航用例中 db_models / agent_runs 的 invoke 入参断言随命令入参面（root / scope）演进更新，回溯 AC-5 / AC-6 的前端入参面半边；壳层布局、路由表、workspace 动作链与错误双轨等既有回归继续执行，不新增壳层行为用例。 -->

#### 待测功能

<!-- design.md 公共函数 / API 表对 app.tsx 零声明（壳层装配组件，非可单测 API 单元），不虚构条目；本章节仅承载既有导航用例的 invoke 入参面断言更新（proposal 测试文件清单 src/app.test.tsx 行）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 壳层导航 db 页入参面（回溯 AC-6） | 正向 | nav-db 切入后 db_models 调用参数断言由无参更新为携 (scope, root)：默认 workspace 库 scope + 当前 workspace root；db-model-list 在场、nav-db 激活、其余两页卸载等壳层路由断言不变 | 新增 |
| 壳层导航 agent 页入参面（回溯 AC-5） | 正向 | nav-agent 切入后 AgentDebugView 挂载发起的 agent_runs invoke 断言携当前 workspace root（返回 [] 数组形态不变），agent-run-form 在场与 hash 落点断言不变 | 新增 |
| 壳层导航欢迎态隔离（root=null） | 边界 | 空清单启动无壳无导航项（nav-agent / nav-db 不在场）：agent_runs / db_models 零调用、root 无从携带（既有欢迎态隔离回归，清单域取数形态不变） | 新增 |
| 无参页面取数形态退役 | 废弃 | toHaveBeenCalledWith('db_models') 无参断言与无 root 的页面挂载取数形态随命令入参面演进而退役（invoke 替身分发同步按新入参面更新） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC 进程边界（@tauri-apps/api/core） | 既有 invoke 按命令名分发替身沿用：db_models / db_records / agent_runs / agent_run_events 分支按新入参面 (scope, root) 更新分发与固定响应（数组形态防 null 崩溃），记录调用序列断言 scope / root 入参；list_workspaces / list_changes / add_workspace / remove_workspace / get_change_detail / code_stats 分支不变 | 全部 describe |

### packages/desktop/src/types/generated/bindings.ts -> packages/desktop/src/types/generated/bindings.test.ts

<!-- 生成物（随 bindings:export 再生成，零手写）。不应创建 bindings.test.ts：生成物类型正确性由消费方 hook / 视图用例编译期承载；详见不可测试项 6。 -->

#### 待测功能

<!-- 纯生成物，design.md 未声明任何运行时行为变更（仅命令入参增 root / scope、DbDimension 类型出线）；不设条目。 -->

#### 用例

<!-- 不设用例；不应创建 bindings.test.ts（零用例套件会红整个 vp test 套件），见不可测试项 6。 -->

#### Mock策略

<!-- 不适用：零运行时行为。 -->

---

## 不可测试项

- AC-9 维度 spec 自洽（workspace 维度落全局目录 per-workspace db、不进 repo；workflow 过程数据维持 user 维度裁定） — **原因**: 纯规格 markdown 文本语义约束，无进程内可断言载体；由 spec 评审与 archive 阶段 specs diff 审查承载，不属于代码测试范围。
- AC-8 静态审查子句（四模型 native_model id / version 与字段面逐项不变、前端 DTO 除 root / scope 外不变、`tests/golden` 契约未触碰） — **原因**: 审查类约束（逐字段比对与 fixture 未触碰判定），无行为断言可写；可自动化半边（模型 shape 往返与三枚举线格式回归不漂移）已路由 model_test.rs 既有用例承载。
- AC-3「消费侧零派生逻辑」与 AC-6「零模型特定代码（git diff 无 per-model 分支）」 — **原因**: 静态代码结构审查子句（消费侧不含派生组装、查看器无 per-model 分支），非进程内行为；行为半边已分别路由 store_test.rs（派生单点确定性）与 db/mod_test.rs（scope 混列隔离）承载。
- packages/desktop/src-tauri/src/main.rs 的 setup 装配（数据根注入 → `WorkspaceStores::open` → `app.manage`、`RunStopRegistry` / `WatchRegistry` 挂载） — **原因**: Tauri 进程入口需真实 Wry runtime 与 `home_dir` 环境方可装配，进程内单测不可行；全局库打开失败 fail fast 口径由 store_test.rs 的 `WorkspaceStores::open` 异常用例覆盖，装配正确性属 code-review / 验收审查面。
- `packages/desktop/src-tauri/crates/infra/store/src/lib.rs`（解析出 `lib_test.rs`） — **原因**: 纯 `pub use` 再导出面与 crate 文档改写，零运行时行为；导出正确性由消费方模块用例编译期承载，不应创建 lib_test.rs（零用例测试文件）。
- `packages/desktop/src/types/generated/bindings.ts`（解析出 `bindings.test.ts`） — **原因**: 构建再生成物（零手写），无自研运行时行为；类型正确性由消费方 hook / 视图用例编译期承载，不应创建测试文件。
- `packages/desktop/package.json` 的 version bump（0.3.8 → 0.3.9） — **原因**: test_detect_frameworks 判定非可测源文件（静态配置项），版本号由发布流程与产物 rebuild 核对，非代码测试对象。
- `packages/desktop/src/routes.tsx` 的 root prop 接线（`/db` 路由向 `DbInspectorView` 传 `root={root}`） — **原因**: test_resolve_paths 报告不在测试配置 scope；一行 prop 透传由消费方 db-inspector-view.test.tsx（root prop 消费）与应用组合渲染覆盖，不单独建文件。

