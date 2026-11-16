# 测试设计: agent-db-query-tools

> **变更**: agent-db-query-tools
> **角色**: test-design-planner（executor）—— 从 `design.md` 推导单元测试范围与公共 API 签名，不写测试命令细节
> **输入**: `proposal.md`（四现象 / D1–D13 / 变更范围实现文件 14 行 + 测试文件 7 行（14 个测试文件 = 3 新建 + 9 既有追加 + 2 既有期望随动）/ AC-1…AC-13 / 风险表）、`design.md`（DD1–DD14 / 架构组件 11 行 / 新增文件 **3 行** + 修改文件 **13 行** / 编译闭合批 5 行 / 公共函数·API **17 行** / 类型定义 17 行 / 数据模型 / AC 落点表 / 待决问题 5 条）、四份 delta spec（`desktop-agent-db-tools` ADDED 7 requirements / `desktop-agent-execution` 1 MODIFIED / `desktop-archi-agent` 1 MODIFIED / `desktop-change-chat` 1 MODIFIED）、`tasks.md` 四阶段交接备忘（含 `:257` 的「3 新建 + 9 既有 sidecar 追加 + 2 既有期望随动 = 14」三档口径，本设计同批登记）
> **被测范围**: design「新增文件」3 项（`crates/infra/store/src/describe.rs` / `crates/infra/store/src/query.rs` / `crates/infra/agent/src/sdk/db_tool.rs`）+「修改文件」13 项，其中 **12 项有测试落点**；**4 项无运行期行为**（`crates/infra/store/Cargo.toml` / `crates/infra/store/src/lib.rs` / `crates/infra/agent/src/sdk/mod.rs` / `packages/desktop/package.json`）——登记于「不可测试项」并以编译锚 / diff 审查替代。**删除文件零项**。
> **触及的测试文件 = 14 个（三档口径，与 proposal「测试文件」表 14 行 / design「待决问题 1」/ `tasks.md:264` 一一对应）**：**3 新建**（`describe_test.rs` / `query_test.rs` / `sdk/db_tool_test.rs`）+ **9 既有 sidecar 追加**（`envelope_test` / `model_test` / `store_test` / `mount_test` / `compose_test` / `runner_test` / `tools_test` / `loop_test` / `lib_test`）+ **2 既有期望随动**（`crates/infra/agent/src/sdk/policy_test.rs` 的三档等价表 11 → 13 名、`src/commands/db/mod_test.rs` 的 workspace 清单 8 → 11 行）。后两者的性质是「行数期望随 `MODEL_ENTRIES` 11 → 14 行 / 挂载清单 11 → 13 项收敛」——**不新增用例、不改断言口径、不触生产命令面**，故在本文中以独立 per-file 章节单列（见「单元测试」末两节），MUST NOT 被表述成「追加用例」，也 MUST NOT 因此把 `src-tauri/src/commands/**` 或 `sdk/*_test.rs` 整目录声明为「零 diff」。
> **本变更性质**: 纯增量**只读数据面**——① store 侧 schema 文档派生（`describe`：specta 类型图 + `specta_serde::Format` 线名 + doc 截断）与查询原语（`query`：10 词 op 词表 / 点号下钻 / `LIKE` / `IN` / `IS NULL` / 排序 / 投影 / 分页 / 精确 `matched`），注册表 11 → 14 行，双库 `global: Arc<Store>` + `global_shared()`；② agent 侧两挂载工具（`db_tables` / `db_query`，`ToolExec::Db` / `ToolTier::Read`）+ 执行体政策（40 KB 预算闸 / `apiKey` 打码与比较面禁用 / 错误文案即自纠）+ per-turn `DbContext` 铸造 + explore 臂 7 → 9 名 / change 臂恒 push 两名。事件词汇零新增、IPC 零新增、前端零改动、`crates/core/**` 零改动。

**测试框架与既有布局（盘点结论）**

| 面 | 运行器 | 文件形态 | 断言与装置 |
|---|---|---|---|
| Rust（`packages/desktop/src-tauri/crates/**`） | 仓库既有 Rust 测试链（`packages/desktop` 的 `server:test`，cargo nextest 承载；本变更只需 `-p store` / `-p agent-runtime` 定向） | **sidecar 同目录同前缀** `*_test.rs`，源文件尾 / `lib.rs` / `mod.rs` 以 `#[cfg(test)] mod <name>_test;` 挂载；**本变更新建 3 个测试文件**（`describe_test.rs` / `query_test.rs` / `db_tool_test.rs`）+ **9 个既有 sidecar 追加** + **2 个既有测试文件的期望值随动**（`sdk/policy_test.rs` / `src/commands/db/mod_test.rs`，行数期望随注册表 / 挂载清单补齐收敛，零新增用例） | 仅 `assert!` / `assert_eq!` / `assert_ne!` / `matches!`，零第三方断言库；测试函数名为**中文短语**；装置 = `tempfile` 真 redb 库（`store_test.rs` / `db_tool_test.rs`）或 `native_db::Builder::create_in_memory` 内存库（`describe_test.rs` / `query_test.rs`，`model_test.rs` 先例）+ rig `test_utils` 流缝（`loop_test.rs`）+ 手写假件（`compose_test.rs` 的 `RecordingRunner` / `RecordingSink`） |
| 生成物 / 版本行 | — | `packages/desktop/src/types/generated/**`、`packages/desktop/package.json` **零测试路径**（新类型不 derive `specta::Type`；版本行走 diff 审查） | 登记于「不可测试项」 |

**测试文件面清单（3 新建 + 9 既有 sidecar 追加 + 2 既有期望随动 = 触及 14 个测试文件）**

| 面 | 新建 | 改造 / 追加 | 期望随动（零新增用例） |
|---|---|---|---|
| store | `src/describe_test.rs`、`src/query_test.rs`（`lib.rs` 的 `#[cfg(test)]` 区同批加声明，字母序落 `canonical_test` 之后 / `envelope_test` 之前） | `src/envelope_test.rs`、`src/model_test.rs`、`src/store_test.rs` | — |
| agent-runtime | `src/sdk/db_tool_test.rs`（`sdk/mod.rs` 的 `#[cfg(test)]` 区同批加声明） | `src/sdk/mount_test.rs`、`src/sdk/tools_test.rs`、`src/sdk/loop_test.rs`、`src/sdk/runner_test.rs`、`src/compose_test.rs`、`src/lib_test.rs` | `src/sdk/policy_test.rs`（逐工具三档等价表 11 → 13 名，两 DB 工具归 `Read` 三档全放） |
| 查看页命令层（`src-tauri/src/commands`） | — | — | `db/mod_test.rs`（workspace 维度 `db_models` 清单 8 → 11 行、计数向量与用例名同步；`db/mod.rs` 生产命令面零 diff） |

**用例表列语义**

- **路径类型**：`正向`（契约内输入 / 预期输出）/ `异常`（契约外输入、失败路径、被禁止的写法、反向判据）/ `边界`（空、空串、极值、恰好阈值、下标与嵌套深度边界）。
- **迭代类型**：`单次`（一次驱动即判）/ `参数化迭代`（同一断言逻辑对表驱动数据集逐项执行）/ `组合迭代`（两个以上维度交叉）。
- **「既有…」前缀**：该断言由既有用例锁定，本设计**只做回归登记**（不新增断言、不改断言口径）；未带前缀者为新增或按新形状改写。
- **「↔ 编译闭合」**：design「编译闭合批」的机械点位（`LoopTurn { …, db }` / `PumpPayload` 两字段 / `EngineFacade` 七处字面构造 / `mount_test` 的 11 → 13 名期望表 / `policy_test` 的三档等价表 11 → 13 名 / `src/commands/db/mod_test.rs` 的清单 8 → 11 行）；**MUST NOT 新增用例或改断言语义**，行内只登记点位。
- **「期望随动」**：既有测试里「行数 / 名集」字面期望随 `MODEL_ENTRIES` 11 → 14 行或挂载清单 11 → 13 项同步，只改期望值与用例名，**不新增用例、不改断言口径**；本文为这两处单列 per-file 章节（`sdk/policy_test.rs` / `src/commands/db/mod_test.rs`），其性质区别于「既有 sidecar 追加」。
- **「夹具随动」**：构造点补字段 / 补实参，断言口径零改动（`E0063` / `E0061` 由编译器兜住）。
- **「组合行」**：跨模块链路用例，挂靠该链路**发起方 / 最上层调用方**所在模块章节（见下「跨模块组合用例挂靠」），不另立集成测试章节。
- **「待测功能」条目来源**：优先取 design「公共函数 / API」表行；`model.rs` / `mount.rs` / `loop.rs` / `compose.rs` 四文件在该表**无行**，其条目按 design「修改文件」/「类型定义」表登记的改动面逐项映射（字段 doc / 常量 / 变体 / 名字清单 / 装配原料），不 grep 源文件导出声明。

**参数类型 → 边界覆盖口径（本变更加载点）**

| 类型 | 覆盖要求与本变更加载点 |
|---|---|
| `u32`（`limit` / `offset`） | `0`（`limit: 0` → `returned == 0` 不 Err；`offset: 0` 首页）、`limit = 20` 缺省、`limit = 1000` 恰上限、`limit = 1001` 越界 `Err`、`offset > matched`（空页 + 三元照常）、负数字面（serde `Err`）；共 6 形态 |
| `String`（`table` / `field` / `op` / `scope`） | `""`（空串）、`"   "`（纯空白）、大小写漂移（`LIKE` → `like`、`Phase` → `phase`）、蛇形名（`change_id`）、超长串（`"字".repeat(1200)`）、特殊字符（`"a\nb"` / `"a\0b🚀"`）、正常值；无 `None`（非 `Option`） |
| `Vec<QueryCond>`（`where`） | `[]`（缺失 / 空数组 → 全量）、单条件、多条件 AND、超大列表（5 条以上）、`where` 非数组 → serde `Err` |
| `Option<Vec<String>>`（`select`） | `None`（全字段）、`Some([])`（仅 `key`）、单元素、多元素含点号路径、含未知字段 → `Err` 携候选线名、`select` 非数组 → `Err` |
| `Option<QueryOrder>`（`order`） | `None`（主键自然序）、`desc: false`、`desc: true`、缺 `desc`（缺省 `false`）、`order` 非对象 → `Err` |
| `serde_json::Value`（`where[].value`） | `null`、number、string、bool、array（`IN` 用）、object、缺失（= 视同 `null`）、与字段类型不符（→ `Err` 携字段渲染类型） |
| `DbDimension`（Enum） | 两值逐值（`user` / `workspace`）+ 非法线值（`"global"` / 空串 → serde `Err`） |
| `bool`（`desc` / `truncated`） | `true` / `false` / 缺省（`desc` 缺省 `false`；`truncated` 未触发恒 `false` 且 `truncatedBy: null`） |
| `Vec<String>`（名字清单 / `entries()`） | 空 `Some([])`、单元素、9 名（explore 臂）、13 名（全清单）、近似名（`"db_query2"` / `"DB_QUERY"`）；`tools = None` 不在本清单语义内 |
| `Option<Arc<Store>>`（双库句柄） | 两位皆在场（`Some(DbContext)`）、global 缺席、workspace 缺席、两位皆缺席 |
| `QueryError`（Enum） | `Invalid`（`Display` 前缀 `invalid: `）/ `Db`（`db: `）两值逐值；不进 `StoreError` 四变体 |
| `Result<String, String>`（`execute` 出口） | 成功串（合法完整 JSON）/ `Err` 串（候选表名 / 候选线名 / op 词表 / 字段类型 / 下标指引 / 预算提示）——各文案两两不等且含成因关键词 |
| 字节计数（预算闸） | 未到预算、`used + n` 恰等预算、恰超预算 1 字节、单条即超预算（`returned == 0`）、整体序列化后仍 < `MAX_RESULT_BYTES` |

**跨模块组合用例挂靠**

- **发现链**（`db_tables` 入参 → `DbContext::execute` → `Store::describe` / `describe_table` → `describe::tables` / `table`）最上层调用方 = agent 执行体 ⇒ 两形态 + `scope` 三态 + 候选文案的组合行写 `crates/infra/agent/src/sdk/db_tool.rs -> sdk/db_tool_test.rs` 章节；纯派生半边（14 行 / 字段线名 / ratchet）留 `describe_test.rs`。
- **取数链**（`db_query` 入参 → 形状解析 → `Store::query` → 预算闸 → 打码 → 结果成形）最上层调用方 = agent 执行体 ⇒ 预算闸 / 打码 / 三元口径 / 错误文案组合行写 `db_tool_test.rs`；纯查询语义（op 逐格 / `LIKE` / 排序 / 投影）留 `query_test.rs`。
- **可见性链**（组合根 `assemble_injections` / `compose_turn` → `EngineFacade::with_db_stores` → `SdkRunner` builder → 泵起播 `mint_db_context` → `LoopTurn.db` → `ToolExec::Db` 分派）发起方 = 组合根 ⇒ 「两臂名字清单 + 双库句柄在场」组合行写 `crates/infra/agent/src/compose.rs -> compose_test.rs` 章节；铸造判据留 `runner_test.rs`；分派两态留 `loop_test.rs`；builder 位面留 `lib_test.rs`。
- **双库身份锚链**（`WorkspaceStores::open` → `global_shared()` / `for_root()` → `DbContext`）发起方 = 组合根 ⇒ 单开纪律与 `Arc` 同实例组合行写 `store_test.rs`（实例面）与 `compose_test.rs`（装配面各一次）。
- **两处期望随动不构成链路**（`sdk/policy_test.rs` / `src/commands/db/mod_test.rs`）：它们是「注册表 / 挂载清单行数」在既有断言上的投影，无跨模块调用序，故各占一个 per-file 章节、用例行只登记期望值（见「用例表列语义 · 期望随动」），不并入任何链路章节。

---

## 验收范围

<!-- 测试区域：Rust 侧 `packages/desktop/src-tauri/crates/infra/{store,agent}/src/**/*_test.rs`（sidecar）
     + 查看页命令层 `packages/desktop/src-tauri/src/commands/db/mod_test.rs`。
     本变更新建 3 个测试文件（`describe_test.rs` / `query_test.rs` / `sdk/db_tool_test.rs`）、追加 9 个既有 sidecar、
     另有 2 个既有测试文件的期望值随动（`sdk/policy_test.rs` / `src/commands/db/mod_test.rs`）
     → 共 14 个测试文件 / 14 个 per-file 章节。不可测面（两个清单与声明导出文件 / 版本行 / 结构零 diff 主张）见文末「不可测试项」。 -->

| AC ID | 验收条件 | 被测文件或模块 |
|---|---|---|
| AC-1 | **`db_tables` 发现面**：无参 14 行（`dimension` / `table` / `count` / `summary`，user 3 + workspace 11，`store_meta` 不出现、不展开字段）；带 `table` 返回全字段（`name` = 落库线名 camelCase、`type` 渲染文本、`doc`）；`scope` 收窄生效；两形态字段集互不重叠 | `crates/infra/store/src/describe.rs` -> `describe_test.rs`；`crates/infra/agent/src/sdk/db_tool.rs` -> `sdk/db_tool_test.rs`；`crates/infra/store/src/store.rs` -> `store_test.rs` |
| AC-2 | **说明来源机械派生 + ratchet**：说明全部来自 `///` doc 与 specta 类型图（`Format` 已应用 → camelCase 线名）；每模型每字段 doc 非空、线名集 ↔ 真实 serde JSON 键集双向一致；内嵌命名类型不入表清单；`SessionEventRecord` 3 字段同形态；4 个 `event_key_serde` 字段线格式仍为十六进制串 | `crates/infra/store/src/describe.rs` -> `describe_test.rs`；`crates/infra/store/src/model.rs` -> `model_test.rs`；`crates/infra/store/Cargo.toml`（依赖项，见「不可测试项」）；`crates/infra/store/src/lib.rs`（导出面，见「不可测试项」） |
| AC-3 | **线名可查**：`where: [{field:"changeId", op:"=", …}]` 命中；写 `change_id` → `Err` 且文案列候选线名（不静默零命中） | `crates/infra/store/src/describe.rs` -> `describe_test.rs`（线名集）；`crates/infra/store/src/query.rs` -> `query_test.rs`（顶层校验 + 候选文案）；`crates/infra/agent/src/sdk/db_tool.rs` -> `sdk/db_tool_test.rs`（文案透出） |
| AC-4 | **查询原语语义**：op 全集逐格断言；AND 组合；`event.kind` 下钻命中（数组下标不认）；`LIKE` 四形 + `\%` `\_` 字面转义 + `_` 不作通配；`IN`；`IS NULL` 两态；`null` 比较语义；跨类型比较 `Err` 携字段类型 | `crates/infra/store/src/query.rs` -> `query_test.rs` |
| AC-5 | **排序 / 投影 / 分页**：`order` 升降与 `null` 恒末位、缺省主键自然序；`select` 投影后 `key` 恒在、未选字段不出现、点号路径重建；`offset` / `limit` 生效、上限 1000、越界 `Err`（不钳位）；`total` / `matched` / `returned` 三元可区分 | `crates/infra/store/src/query.rs` -> `query_test.rs`；`crates/infra/store/src/store.rs` -> `store_test.rs`（不复用 `MAX_SCAN_LIMIT`） |
| AC-6 | **双档收口**：40 KB 预算先到者止（`truncated: true` + `truncatedBy: "bytes"` + 无半截 JSON）；行数到 `limit` → `"limit"`；两因文案含三条收缩指引；`session_event` 缺省 `limit=20` 落预算内、`limit=50` 被预算截断且如实回报 `returned < matched`；结果体恒低于 L1 `MAX_RESULT_BYTES`（50 KB） | `crates/infra/agent/src/sdk/db_tool.rs` -> `sdk/db_tool_test.rs`；`crates/infra/agent/src/sdk/mount.rs` -> `sdk/mount_test.rs`（描述声明「预算是实际生效值」） |
| AC-7 | **敏感面**：`agent_provider` 记录 `apiKey` 输出恒 `"***"`（含 `select` 显式投影）；`where` / `order` 引用 `apiKey` → `Err`（前缀与等值探测通道都不成立） | `crates/infra/agent/src/sdk/db_tool.rs` -> `sdk/db_tool_test.rs` |
| AC-8 | **只读与零介入**：store 新增面全为读（无写 API / 无写事务）；两工具 tier = `Read` → 三档全放行；`sandbox_rewrite` 原样返回；写保护闸零介入；`crates/core/**`、`policy.rs`、`protect/**` 零 diff | `crates/infra/agent/src/sdk/mount.rs` -> `sdk/mount_test.rs`（tier + 三档）；`crates/infra/agent/src/sdk/policy.rs` -> `sdk/policy_test.rs`（逐工具三档等价表 11 → 13 名，两 DB 工具归 `Read` 档；既有期望随动，见该 per-file 章节）；`crates/infra/agent/src/sdk/tools.rs` -> `sdk/tools_test.rs`（`input_path` 五名 `None` 臂）；`crates/infra/agent/src/sdk/loop.rs` -> `sdk/loop_test.rs`（零 notice）；`crates/infra/store/src/{describe,query,store}.rs`（只读行为面：调用后计数不变） |
| AC-9 | **会话可见性两臂**：explore 臂 9 名（六工具 + `create_explore_note` + 两 DB）；change 臂不分 phase / 不分矩阵命中恒含两名（矩阵未命中行、畸形 `source_ref` 同样含）；archi 臂 10 名（七内建含 `write` + 三 archi；proposal AC-9 与「测试文件」表的「archi 臂 9 名」为笔误，按 `compose.rs:62-66` 实测与 design 校准为 10）与 debug 臂 `tools = None` 均不含；`RunStarted.tools` 与实际下发面逐名一致 | `crates/infra/agent/src/compose.rs` -> `compose_test.rs`；`crates/infra/agent/src/sdk/mount.rs` -> `sdk/mount_test.rs`（名字进清单 + `definitions` 过滤） |
| AC-10 | **双库 scope 与身份锚**：`scope="user"` 走全局库、`workspace` 走会话 workspace 库、缺省 workspace；两库清单互不混列；`global_shared()` 交出 `Arc` 且不二次 open；change 会话 cwd 为 worktree 时库身份锚仍为主仓 root | `crates/infra/store/src/store.rs` -> `store_test.rs`（`global_shared` 单开纪律）；`crates/infra/store/src/envelope.rs` -> `envelope_test.rs`（`dimension_of` 派库）；`crates/infra/agent/src/sdk/db_tool.rs` -> `sdk/db_tool_test.rs`（scope 三态 + 维度自纠）；`crates/infra/agent/src/compose.rs` -> `compose_test.rs`（装配句柄对）；`crates/infra/agent/src/lib.rs` -> `lib_test.rs`（builder 缝）；`src-tauri/src/commands/db/mod.rs` -> `src/commands/db/mod_test.rs`（查看页两库清单互不混列的名集期望随注册表补齐 8 → 11 行；既有期望随动，见该 per-file 章节） |
| AC-11 | **成员校验与幻觉调用**：未下发两工具的会话（archi / debug）调用 `db_query` → `SystemNotice{subtype:"tool_unavailable"}` + `is_error` 回灌、run 不中断、不触执行体；`DbContext` 缺席时执行体以 `is_error` 回灌 | `crates/infra/agent/src/compose.rs` -> `compose_test.rs`（两臂不含名）；`crates/infra/agent/src/sdk/loop.rs` -> `sdk/loop_test.rs`（成员校验回归 + `ToolExec::Db` 缺席臂）；`crates/infra/agent/src/sdk/runner.rs` -> `sdk/runner_test.rs`（铸造判据） |
| AC-12 | **事件 / IPC / 前端零改动**：`AgentEvent` 六变体与块词汇逐字不变、零新 subtype；`src/bindings/` 与 `packages/desktop/src/**` git diff 为空；`src-tauri/src/commands/**` 的**生产命令面**（`db/mod.rs` / 各命令实现）零 diff（唯一例外 = `db/mod_test.rs` 的注册表行数期望随动，登记于本文 per-file 章节与「不可测试项」） | 结构事实 → 见「不可测试项」；行为侧锚 = `crates/infra/agent/src/sdk/loop_test.rs` 与事件面既有用例零改动全绿；`crates/infra/agent/src/sdk/mod.rs`（声明面，见「不可测试项」）；`src-tauri/src/commands/db/mod.rs` -> `src/commands/db/mod_test.rs`（生产命令面零 diff + 期望随动单点） |
| AC-13 | **版本交付**：归档时 `packages/desktop/package.json` version `0.6.43` → `0.6.44`（`tauri.conf.json` 跟随、`src-tauri/Cargo.toml` crate 版本行不跟随） | `packages/desktop/package.json`（元数据行，无运行期行为 → 见「不可测试项」） |

> **口径说明**：AC-8 / AC-12 的「零 diff」与 AC-13 的版本递增是**结构 / 元数据事实**，运行期无法断言（见「不可测试项」）；本表只登记行为侧可断言半边。两处「零 diff」都按**收窄口径**表述：`src-tauri/src/commands/**` 只声明生产命令面零 diff（唯一例外 `db/mod_test.rs` 的注册表行数 / 名集期望随动），`sdk/*_test.rs` 同理（例外 = `policy_test.rs` 的三档等价表期望随动）——MUST NOT 用整目录通配把这两处登记项误判成越界改动。全量测试回归、`check` 整链（`server:check` → `bindings:export` → `api:check` → `client:check`）与版本递增由桌面端 static-check / test-execution 相位承接。

---

## 单元测试

### `packages/desktop/src-tauri/crates/infra/store/src/describe.rs` -> `packages/desktop/src-tauri/crates/infra/store/src/describe_test.rs`

**新建 sidecar**（`lib.rs` 的 `#[cfg(test)]` 区同批加 `mod describe_test;`，字母序落 `canonical` 之后 / `envelope_test` 之前）。装置：`native_db::Builder::new().create_in_memory(crate::store::workspace_models() / global_models())` 内存库（`model_test.rs:291` 先例）+ 逐记录 `rw.insert(record)` 播种；`Types` / `Format` / `render_type` / `truncate_doc` 全真实调用（被测面即派生链，替身会掩盖线名 / 类型文本差异）。

#### 待测功能

- `describe::tables(db, dimension)`: 该维度表清单（`table` / `dimension` / `count` / `summary`），不展开字段；注册表按维度过滤（`store_meta` 天然缺席）。
- `describe::table(dimension, name)`: 单表全量字段（线名 / 渲染类型 / doc）；未知表 `Err` + 本维度候选表名。（落定签名不含 `db` 入参——计数只在清单侧 `tables` 需要；design「公共函数 / API」表的草案签名含 `db`，按落地形态取两侧一致的行为面。）
- `describe::field_index(table)`: 该表线名 / 渲染类型 / `FieldKind` 三元组集合（`query` 顶层校验的同一来源）。
- `describe::schema_of::<T>(types)`: derive 型 schema 供给（`T::definition`），即注册表 `schema` 字段的实现。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `describe::tables` | 正向 | workspace 维度恰 11 行（`agent_run` / `session` / `session_event` / `explore` / `change` / `phase` / `checklist_item` / `step` / `run` / `run_node` / `run_edge`，序 = 注册表登记序）、user 维度恰 3 行（`workspace` / `agent_provider` / `agent_instance`）；每行 `summary` 非空、`count` 与播种量一致 | 参数化迭代（2 维度） |
| `describe::tables` | 正向 | 清单形态不展开字段：`ModelDoc` 四字段（`table` / `dimension` / `count` / `summary`），字段说明只经 `TableDoc.fields` 供给（形态侧证 —— 序列化面无 `fields` 键的断言落在 `db_tool_test.rs` 的输出形态行） | 单次 |
| `describe::tables` | 异常 | `store_meta` 不出现在任一维度清单；内嵌命名类型（`AgentModelTiers` / `SessionConfigSnapshot` / `ChangeActivePhase`）不出现在表名集（按注册表名单过滤） | 参数化迭代（2 维度 × 3 内嵌名） |
| `describe::tables` | 边界 | 空库（零记录）→ 两维度仍全列（11 / 3 行），`count` 全 0（与 `list_models` 的「计数 0 也列出」同口径） | 单次 |
| `describe::tables` | 边界 | 维度入参决定输出而非库探测：以 `DbDimension::User` 查 workspace 模型组的内存库 → 仍返回 user 3 行、`count` 全 0、不 panic | 单次 |
| `describe::table` | 正向 | `("phase")` → 15 字段逐线名（`id` / `changeId` / `phase` / `attempt` / `verdict` / `report` / `skipped` / `stale` / `backtrackTo` / `backtrackReason` / `executorSessionId` / `evaluatorSessionId` / `decisionSessionId` / `startAt` / `timestamp`）；`type` 渲染（`number` / `string` / `boolean` / `Verdict` / `string \| null`）；每字段 `doc` 非空 | 单次 |
| `describe::table` | 正向 | `("session_event")` → 恰 3 字段：`eventKey` = `string`、`sessionId` = `string`、`event` = `object`（手写 schema：opaque 引用渲染为 `object`），三条 `doc` 与 `model.rs` 的 `///` 同源（非空） | 单次 |
| `describe::table` | 异常 | 未知表 `"phases"`（拼写错误）→ `Err`，文案含候选表名（至少含 `phase`），不返回空字段清单 | 单次 |
| `describe::table` | 异常 | 跨维度表名：user 维度库查 `"phase"` → `Err` + 本维度候选（`workspace` / `agent_provider` / `agent_instance`），不静默成功 | 单次 |
| `describe::table` | 边界 | 表名 `""` / `"   "` / `"Phase"` / `"phase "` / `"字".repeat(1200)` / `"a\nb"` / `"a\0b🚀"` → 逐形态 `Err` + 候选（零大小写归一、零 trim） | 参数化迭代（7 形态） |
| `describe::tables` / `describe::table` | 边界 | doc 截断（`truncate_doc` 单点）：长文 doc（`PhaseRecord.change_id` 类整段）截在首个 `。` / `；` / `\n`（含该字符）；无句读或句读位 > 120 字 → 截到 120 `char` 边界并补 `…`；恰 120 字不补省略号；尾部空白剥离；表介绍与字段 `doc` 走同一函数（同一句读规则） | 参数化迭代（5 形态） |
| `describe::schema_of::<PhaseRecord>` | 正向 | 渲染分类器（DD14）表驱动：`Primitive` 各位宽 → `number` / `string` / `boolean`；`Nullable(inner)` → `"<inner> \| null"`；`List` → `"<T>[]"`；`Map` → `"{ [key: <K>]: <V> }"`；`Enum` → `string`；`Struct` → `object`；`Reference(Named)` → 命名类型名（`Verdict` / `ChangeStatus`）；`Reference(Opaque)` → `object` | 参数化迭代（9 形态） |
| `describe::schema_of::<T>`（具名引用的内联形态） | 边界 | 容器型（`String` / `Vec<T>` / `Map`）在 specta 类型图里以「具名引用 + `NamedReferenceType::Inline { dt }`」出现（具名侧 `ty = None`）：`render_type` / `field_kind` 须递归取内联 `dt` 再分类——真实字段侧证 `phase.changeId` → `string` / `FieldKind::Text`、`phase.backtrackTo` → `string \| null`；命名枚举（`Verdict`）与命名结构仍渲染类型名。漏解内联形态会让字符串字段全落 `Unknown`（`query` 的字符串值类型校验随之整体失效），故单列此边界 | 参数化迭代（3 形态） |
| `describe::field_index` | 正向 | `("phase")` → 含 `changeId`，且 `kind_of` / `render_of` 与 `render_type` / `field_kind` 同源一致（同名字段的 kind 与文本不互相矛盾：`attempt` = Number/number、`phase` = Text/string、`skipped` = Bool/boolean、`verdict` = Text/Verdict） | 单次 |
| `describe::field_index` | 异常 | 未知表 → `Err` + 候选；`kind_of("event.blocks")`（嵌套路径）→ `Unknown`；`names()` 不含蛇形名（`change_id` 缺席、`changeId` 在场） | 组合迭代（3 观测面） |
| `describe::field_index` | 边界 | `("session_event")` 三字段索引与手写 schema 同源：`eventKey` / `sessionId` / `event`；`names()` 恰 3 项 | 单次 |
| ratchet（`tables` + `table` + `serde_json::to_value`） | 正向 | 14 张表 × 每字段：`doc` 非空（空 doc 即指名模型与字段失败）；线名集 == 真实 serde JSON 键集（逐表断言集合相等，两方向各一次：线名 ⊆ 键集、键集 ⊆ 线名） | 组合迭代（14 表 × 双向） |
| ratchet（三 `specta(type = String)` 字段） | 正向 | `checklist_item.itemKey` / `run_node.nodeKey` / `run_edge.edgeKey`：`describe` 线名为 `string`，且 `serde_json::to_value` 得 `0x` 前缀十六进制串（落库与线格式未改） | 参数化迭代（3 字段） |
| ratchet（反向判据） | 异常 | 键集差集为空的两个方向各断言一次（差集非空即红）；`change_id` 一类蛇形名在任一表线名集中缺席（防「未应用 `Format`」回归） | 组合迭代（14 表 × 2 方向） |
| `describe::tables` | 边界 | 每次调用重建 `Types`：连调两次结果逐字相等（无跨调用污染）；同一表两次 `table()` 输出相等 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 零 mock | 被测面为进程内纯派生链（注册表 + specta 类型图 + `specta_serde::Format` + 纯 formatter），无 IO、无配置、无子进程、无网络 | 全章节 |
| 不 mock：`native_db` | `create_in_memory(crate::store::workspace_models() / global_models())` 真内存库 + `rw.insert` 真落库（计数与解码面都需真实模型登记；替身会让 `count` 断言空转） | 全部用例 |
| 不 mock：`specta` / `specta_serde` | 真实 `T::definition` + 真实 `Format::map_types`（被测面就是「线名与 doc 从哪来」，替身化等于把断言改成自证） | 全章节 |

### `packages/desktop/src-tauri/crates/infra/store/src/query.rs` -> `packages/desktop/src-tauri/crates/infra/store/src/query_test.rs`

**新建 sidecar**（`lib.rs` 同批加 `mod query_test;`）。装置同 `describe_test.rs`（内存库 + 逐条播种），查询执行体 `query::run` 直调（该函数即被测面）；播种数据为可读的人工语料（`phase` 15 行 / `session_event` 若干长行 / `agent_provider` 2 行 / `run` + `run_node` + `run_edge` 各若干），使每格断言可人工核算。

#### 待测功能

- `query::run(db, dimension, spec)`: 过滤 → 排序 → 投影 → 分页执行体（DD4 双路径：`where` 空且无 `order` 走惰性 `skip/take`，其余全表 `walk` 且 `total` / `matched` 精确）；`limit` 缺省 20 / 上限 1000（**不复用** `MAX_SCAN_LIMIT`）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `query::run` `op` | 正向 | 词表逐格（`=` / `!=` / `>` / `>=` / `<` / `<=` / `LIKE` / `IN` / `IS NULL` / `IS NOT NULL`）各查一次，命中集与人工核算相等（`>=` / `<=` 含边界值；`<` / `>` 不含） | 参数化迭代（10 词） |
| `QUERY_OPS` | 正向 | 10 词逐值有序（`["=", "!=", ">", ">=", "<", "<=", "LIKE", "IN", "IS NULL", "IS NOT NULL"]`），且每词经 `query::run` 均不 `Err`（词表与解析单点同源） | 参数化迭代（10 词） |
| `query::run` `op` | 异常 | 未知 op：`"contains"` / `"like"`（小写）/ `""` / `"   "` / `"=="` → `Err` 且文案含全词表（10 词逐词出现），MUST NOT 退化为空结果集 | 参数化迭代（5 形态） |
| `query::run` `where` | 正向 | AND 组合：两条件（`changeId` + `verdict != null`）= 交集；三条件 = 三重收窄；`where` 缺失 / `[]` = 全量（`matched == total`） | 组合迭代（4 形态） |
| `query::run` 点号下钻 | 正向 | `event.kind == "message"` 命中该会话中 kind 为 message 的行；`configSnapshot.model` 嵌套命中（`session` 表） | 参数化迭代（2 路径） |
| `query::run` 点号下钻 | 异常 | `event.blocks.0.name` → `Err`，文案含「不支持数组下标」指引（改看整条记录 / 用点号取对象字段），且不命中任何行 | 单次 |
| `query::run` 点号下钻 | 边界 | 不存在的中段路径（`event.nope`）→ 运行期视作 `null`：`IS NULL` 命中、`=` 值不命中、不 `Err`（DD11 静态校验只到顶层） | 组合迭代（3 op） |
| `query::run` `LIKE` | 正向 | 四形：前缀 `dev-%`、后缀 `%-executor`、包含 `%sign%`、无通配 `dev-design`（退化为等值）；`%` 匹配空串（`dev-%` 命中 `dev-`）；区分大小写（`Dev-%` 不命中 `dev-design`） | 参数化迭代（5 形态） |
| `query::run` `LIKE` | 异常 | `_` 不作单字符通配（`dev_design` 不命中 `dev-design`）；`\%` / `\_` 为字面转义（值含字面 `%` / `_` 的行被命中，非转义形态按通配处理）；`null` 不匹配任何 `LIKE`（含 `%` / `%%`） | 组合迭代（4 形态） |
| `query::run` `IN` | 正向 | `attempt IN [1,2]` 逐元素按 `=` 语义命中；单元素数组；超大列表（20 元素）结果集正确 | 参数化迭代（3 形态） |
| `query::run` `IN` | 异常 | `value` 非数组（字符串 / 数字 / 对象 / 缺失）→ `Err` 携期望形状（数组）；不返回空集 | 参数化迭代（4 形态） |
| `query::run` `IN` | 边界 | `IN []` → `matched == 0`、`records` 空、`total` 照常；`IN` 数组含 `null` 元素（按 `=` 语义恒不命中） | 组合迭代（2 形态） |
| `query::run` `IS NULL` / `IS NOT NULL` | 正向 | 两态互补（命中集不交、并集 == 全量）；`backtrackReason` 空值行在 `IS NULL` 侧 | 单次 |
| `query::run` `IS NULL` / `IS NOT NULL` | 异常 | 带非 null `value` → `Err` 携「不接 value」形状提示；带显式 `null` 等价于省略（正向通过） | 参数化迭代（2 形态） |
| `query::run` `null` 比较语义 | 边界 | `= null` → false（不命中）；`!= null` → true（命中非空行）；`>` / `>=` / `<` / `<=` 任一侧 `null` → 不命中（不 `Err`、不 panic） | 组合迭代（6 格） |
| `query::run` 类型校验 | 异常 | 值类型与字段类型不符：`attempt = "1"`（字符串查 number）→ `Err` 携字段渲染类型 `number`；`skipped = "true"` → `Err` 携 `boolean`；`phase = 1` → `Err` 携 `string`；不返回空集 | 参数化迭代（3 组合） |
| `query::run` 类型校验 | 边界 | 嵌套路径（`event.kind`）类型不可知 → 不做值类型校验（三类值均不 `Err`，按运行期比较）；`verdict`（Named Enum，kind = Text）接受字符串、拒绝数字 | 参数化迭代（2 形态） |
| `query::run` 排序 | 正向 | 升 / 降序各一次（`timestamp` 严格单调）；缺 `desc`（缺省 false）= 升序；无 `order` = 主键自然序（与 `scan` 的序一致） | 参数化迭代（3 形态） |
| `query::run` 排序 | 边界 | `null` 恒末位（升序降序皆末位，`path 缺失` 同 null）；跨类型定序 `number < string < bool < object < array < null`（同一字段混插各类值的语料）；相等值行集两页一致（不锁相对序，只断言集合与页切无重无漏） | 组合迭代（2 方向 × 3 形态） |
| `query::run` 投影 | 正向 | `select: ["id","phase","attempt"]` → `value` 恰含三键、`key` 恒在；`select` 缺省 = 全字段（逐键与表线名集一致） | 参数化迭代（2 形态） |
| `query::run` 投影 | 正向 | 点号路径重建：`select: ["sessionId","event.kind","event.role"]` → `value.event` 为只含 `kind` / `role` 的对象（`event` 其余键不出现） | 单次 |
| `query::run` 投影 | 边界 | `select: []` → `value` 为空对象、`key` 仍在；`select` 项重复（`["id","id"]`）→ 不重复输出；`select` 含未知字段 → `Err` + 候选线名；`select` 非数组 → `Err` 携期望形状 | 组合迭代（4 形态） |
| `query::run` 分页 | 正向 | `offset` / `limit` 切页：两页并集 == 全量命中集（无重无漏）；`offset = 0` 首页；`limit` 缺失 → 20 条（语料 > 20 时 `returned == 20`） | 组合迭代（3 形态） |
| `query::run` 分页 | 异常 | `limit = 1001` → `Err` 文案含上限 1000（**不钳位**、不返回 1000 行装成功）；`limit = u32::MAX` 同臂 | 参数化迭代（2 形态） |
| `query::run` 分页 | 边界 | `limit = 1000`（恰上限）→ 不 `Err`、`returned == min(matched, 1000)`（**证明未复用 500 上限**，语料 ≥ 520 行）；`limit = 0` → `returned == 0`、`matched` / `total` 仍精确；`offset > matched` → 空页 + 三元照常 | 组合迭代（3 形态） |
| `query::run` 三元口径 | 正向 | `total`（全量）/ `matched`（过滤后）/ `records.len()`（本页）逐场景可区分：无过滤小 limit、有过滤大 limit、有过滤 + offset 三场景各一次；空表 → 0 / 0 / 0；恒有 `returned ≤ matched ≤ total` | 组合迭代（4 场景） |
| `query::run` `spec` 形态 | 异常 | 未知表（`"phases"` / `""` / `"   "` / `"Phase"`）→ `Err` + 候选表名；跨维度表名（user 库查 `phase`）→ `Err`（本维度候选）；`store_meta` → `Err`（未注册） | 参数化迭代（5 形态） |
| `query::run` `spec` 形态 | 异常 | 未知字段（`change_id` / `ChangeId` / `changeIdd`）→ `Err` 携候选线名（至少含 `changeId`），MUST NOT 返回空集；`order.field` 同形 | 参数化迭代（3 形态） |
| `QuerySpec` 反序列化 | 边界 | 缺省语义：缺 `where` / `order` / `select` / `offset` → `[]` / `None` / `None` / `0`；缺 `limit` → **20**；未知键（`order_by` / `scope`）→ `Err`（`deny_unknown_fields`）；`limit: -1` / `limit: 2^32` / `offset: -1` → serde `Err` | 参数化迭代（7 形态） |
| `QueryError` | 边界 | `Display` 两臂前缀：`Invalid(..)` → `invalid: …`、`Db(..)` → `db: …`（`Db` 臂以构造变体直测）；`Store::{describe,describe_table,query}` 的返回类型恒为 `QueryError`（签名锚，非 `StoreError` 变体），`StoreError` 四变体与 `change_port` 映射臂零改动由既有用例回归 | 参数化迭代（2 臂） |
| `query::run`（只读） | 边界 | 任一次查询前后各表计数不变（`total` 逐表对拍）、`store_meta` 未被写入、无新记录产生（只读零写面行为侧证） | 组合迭代（3 查询 × 2 观测面） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 零 mock | 被测面为进程内纯函数 + 单读事务扫描（无 IO、无配置、无子进程、无网络、无时间依赖） | 全章节 |
| 不 mock：`native_db` | `create_in_memory` 真内存库 + 真 `rw.insert` 播种（跨类型值、`null` 列、> 520 行语料都需要真实编码；替身无法承载 `walk` / `scan` 语义） | 全部用例 |
| 不 mock：`crate::describe` | 真实 `field_index` / `field_kind`（同一分类器是 AC-4「类型不符 `Err` 携类型」的机械前提，替身化会让两处口径漂移不可见） | 类型校验与投影行 |

### `packages/desktop/src-tauri/crates/infra/store/src/model.rs` -> `packages/desktop/src-tauri/crates/infra/store/src/model_test.rs`

**既有 sidecar 追加（回归为主）**。design「公共函数 / API」表无本文件行：本文件改动面为 12 条字段 `///` 与 3 处 `#[specta(type = String)]`（design「修改文件」表 ①②），测试落点由 design「测试文件」表登记为「补 doc 后 decode/encode 逐字节不变」与「`event_key_serde` 字段的 `specta(type = String)` 与真实 serde 十六进制串键集一致」。同批加注：`SessionEventRecord.event_key` **不加**该属性（该校验 1），其线格式由 `describe.rs::schema_session_event` 手写 schema 表达，行为侧归 `describe_test.rs`。

#### 待测功能

- `PhaseRecord` / `ChecklistItemRecord` / `RunNodeRecord`（`native_model::encode()` → `native_model::decode::<T>()` 往返）: 补 12 条字段 `///` 后编码字节、解码字段值与本变更前逐字节相等（注释不进线格式，字段序与枚举下标不变）。
- `ChecklistItemRecord.item_key` / `RunNodeRecord.node_key` / `RunEdgeRecord.edge_key`（`#[specta(type = String)]`）: `serde_json::to_value` 线格式仍为 `0x` 前缀十六进制串，且 JSON 键集与该模型字段面逐一对应（属性只改类型图，不改 wire）。
- `SessionEventRecord.event_key`（**不加** `specta(type = String)`）: 该模型不 derive `Type`，落库与 JSON 线格式仍为十六进制串（反向锚 —— 线名说明由 `describe::schema_session_event` 手写 schema 供给）。
- 既有 `model_test` 全量编解码 / 版本升级链 / 键打包 / serde 往返用例: 断言口径零改动全绿（回归判据）。
- `specta` 属性语法与 `specta-serde` 依赖面: 编译闭合（无运行期断言；依赖项登记见「不可测试项」）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 既有 `model_test` 全量 | 正向 | 「既有…」12 条 doc 与 3 处属性落地后，既有 decode/encode 逐字节对拍、版本升级链、键打包、serde 线格式往返用例**零改动全绿**（`///` 不进线格式） | 单次 |
| `PhaseRecord` / `ChecklistItemRecord` / `RunNodeRecord` | 正向 | 补 doc 后 `native_model::encode` → `decode` → 再 `encode` 三态字节相等（注释不入编码，字段序 / 枚举下标不变） | 参数化迭代（3 模型） |
| `ChecklistItemRecord.itemKey` / `RunNodeRecord.nodeKey` / `RunEdgeRecord.edgeKey` | 正向 | `#[specta(type = String)]` 落地后 `serde_json::to_value` 仍给 `0x` 前缀十六进制串，且 JSON 键集与该模型全字段集逐一对应（属性只影响类型图，不改 wire） | 参数化迭代（3 字段） |
| `SessionEventRecord.event_key` | 异常 | 未加 `specta(type = String)`：该模型不 derive `Type`，属性在场即编译错误；落库与 JSON 线格式仍为十六进制串（回归断言，与 `describe_test` 的手写 schema 三字段互为反向锚） | 单次 |
| ↔ 编译闭合 | — | `specta` 属性语法与 `specta-serde` feature 面编译通过（无运行期断言；`store/Cargo.toml` 依赖项见「不可测试项」） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 零 mock | 与既有 `model_test.rs` 同装置：`native_model::encode/decode` 内存往返 + `native_db` 内存库严格字节判等 + `serde_json` 真序列化 | 全章节 |
| 不 mock：既有装置 | 既有 fixture 构造器（`event` / `run_started` / `roundtrip`）原样复用，新用例不改装置形态 | 全章节 |

### `packages/desktop/src-tauri/crates/infra/store/src/envelope.rs` -> `packages/desktop/src-tauri/crates/infra/store/src/envelope_test.rs`

**既有 sidecar 改造 + 追加**：workspace 维度期望清单 8 → **11** 行（尾部补 `run` / `run_node` / `run_edge`）、计数向量同步；注册表访问器与 `dimension_of` 新增用例；`list_models` / `scan` / `MAX_SCAN_LIMIT` 的既有断言**口径零改动**（回归判据）。describe / query 的行为面按 design 待决问题 1 落 `describe_test.rs` / `query_test.rs`，本文件只保留注册表面。

#### 待测功能

- `entries()`: 注册表只读出口（14 行全表；`describe` / `query` 的过滤源）。
- `dimension_of(table)`: 表名 → 维度（跨维度唯一由注册表保证）；未注册名返回 `None`。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `entries()` | 正向 | 长度恰 14、名字唯一、序 = 登记序（user 3 在前 / workspace 11 在后，逐名对照常量清单）；`dimension` 分组与名字集一致 | 单次 |
| `entries()` | 边界 | 与 `list_models` 的两个维度清单并集逐名相等（同一真源）：首项 `workspace`、末项 `run_edge`（登记序首尾锚） | 单次 |
| `dimension_of` | 正向 | `"run"` / `"run_node"` / `"run_edge"` / `"phase"` → `Workspace`；`"workspace"` / `"agent_provider"` / `"agent_instance"` → `User` | 参数化迭代（6 表） |
| `dimension_of` | 异常 | `"store_meta"` → `None`（内部治理记录不入注册表）；`"phases"` / `""` / `"   "` / `"Phase"` / `"a\nb"` / `"a\0b🚀"` / `"字".repeat(1200)` → `None`（零归一、零 trim） | 参数化迭代（7 形态） |
| `Store::scan`（三新表 `key_of`） | 正向 | `run` → `runId` 字符串；`run_node` → `{runId, nodeKey}`；`run_edge` → `{runId, edgeKey}`（十六进制串形态，与 `checklist_item` / `session_event` 同形）；三表经 `scan` 可读出播种行、空表 `records` 空 + `total` 0 | 参数化迭代（3 表） |
| `Store::list_models` | 正向 | workspace 维度 11 行（序 = 登记序）、user 维度 3 行；三新表计数与播种量一致（`run` 1 / `run_node` n / `run_edge` m） | 单次 |
| `Store::scan` | 异常 | 未知模型名 `Err`（文案逐字不变）；跨维度模型名（user 库 `scan("phase")`）→ `Err` 未知模型（维度锁定） | 参数化迭代（2 形态） |
| `Store::scan` | 边界 | 「既有…」`MAX_SCAN_LIMIT = 500` 口径零改动：`limit = 1000` 仍截到 500 行、`limit = 0` 空页、`offset` 越过全量 → 空页但 `total` 照常 | 参数化迭代（3 形态） |
| ↔ 编译闭合 | — | `ModelEntry` 增 `schema` / `walk` 两字段后 14 行各补两行、`Vec::with_capacity` 与 `entry()` / `entries()` 访问器编译通过（无运行期断言） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 数据根 / 库文件 | 既有 `Env` 装置（`tempfile` 双临时目录 + 真 `Store::open_workspace` / `open_global`），存储层不 mock | 全章节 |
| 不 mock：注册表与分页扫描 | 真实 `OnceLock`-free 常量表 + 真 `native_db` 读事务（被测面即「新模型登记一行即可浏览」，替身化会让登记行为断言空转） | 全章节 |
| 播种 | 真写面 API（`create_session` / `append_session_events` / `start_change_run` / `finish_change_run` 等），不直插 native_db | 三新表与计数行 |

### `packages/desktop/src-tauri/crates/infra/store/src/store.rs` -> `packages/desktop/src-tauri/crates/infra/store/src/store_test.rs`

**既有 sidecar 改造 + 追加**：`WorkspaceStores.global` 由值类型改 `Arc<Store>` 后 `global()` 语义不变（既有 40+ 调用点零改动即回归判据）；新增 `describe` / `describe_table` / `query` 三薄包装与 `global_shared()` 用例。

#### 待测功能

- `Store::describe()`: 本库维度表清单（`table` / `dimension` / `count` / `summary`），三行委托 `describe::tables`。
- `Store::describe_table(table)`: 单表全字段；未知表 `Err` + 本维度候选表名。
- `Store::query(&spec)`: 过滤 → 排序 → 投影 → 分页（`limit` 缺省 20 / 上限 1000，**不复用** `MAX_SCAN_LIMIT`）。
- `WorkspaceStores::global_shared()`: 全局库 `Arc` 出口（redb 单进程单句柄 → MUST NOT 二次 open；`global()` 语义不变）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `Store::describe` | 正向 | workspace 库 11 行 / 全局库 3 行，表名集与 `list_models` 逐名一致（同一注册表，两读面同源） | 参数化迭代（2 库） |
| `Store::describe_table` | 正向 | `("phase")` 下钻字段集与 `describe_test` 直调结果相等；`("nope")` → `QueryError::Invalid` 且文案含候选表名 | 参数化迭代（2 形态） |
| `Store::query` | 正向 | 委托正确：`where` + `order` + `select` + `offset` / `limit` 全链一次跑通，`total` / `matched` / `records.len()` 三元与 `scan` 的全量计数对拍 | 单次 |
| `Store::query` | 边界 | `limit = 1000` 不复用 500：播种 ≥ 520 行（`phase` 或 `session_event`）→ `returned > 500` 且 `== min(matched, 1000)`（钳到 500 即本行红） | 单次 |
| `Store::query` | 异常 | `limit = 1001` → `QueryError::Invalid`（文案含 1000），非 `StoreError::Db`；未知表 / 未知字段 → `Invalid` 携候选 | 参数化迭代（3 形态） |
| `WorkspaceStores::global_shared` | 正向 | 两次调用 `Arc::ptr_eq` 为真（同一实例、无二次 open）；经 `global()` 写入 provider → 经返回的 `Arc` 读到同一记录（同一句柄双向可见） | 组合迭代（2 观测面） |
| `WorkspaceStores::global_shared` | 正向 | `global()` 返回 `&Store` 语义零改动：`&*global_shared() == global()` 观测同一实例（同一 provider 计数 / 同一 workspace 清单） | 单次 |
| `WorkspaceStores::global_shared` | 边界 | 单开纪律：连调 3 次 + 其间 `for_root` 开 workspace 库 → 无 panic、无锁冲突；`for_root` 同 root 仍返回同一 `Arc`（既有缓存语义零改动） | 组合迭代（2 缓存面） |
| `Store::{describe,describe_table,query}` | 边界 | 只读：三调用前后 `list_models` 计数逐表不变、`store_meta` 未改、库文件无新记录（行为侧零写面证） | 组合迭代（3 调用 × 2 观测面） |
| 既有用例 | 正向 | 「既有…」`open` / `open_global` / `open_workspace` / `for_root` / `list_models` / `scan` / 全部写面用例零改动全绿（`global` 改 `Arc` 零波及） | 单次 |
| ↔ 编译闭合 | — | `open()` 构造点 `Arc::new(...)`、`global()` 改 `self.global.as_ref()` 编译通过；`WorkspaceStores` 无字面构造点（私有字段） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 数据根 / 库文件 | 既有 `Env` 装置（`tempfile` 真目录 + 真 `WorkspaceStores::open`）；不 mock 文件系统 | 全章节 |
| 不 mock：`WorkspaceStores` / `Store` | 真实实例（被测面就是「同一句柄共享」与「单开纪律」，替身化会让 `Arc::ptr_eq` 与锁行为空转） | 全部用例 |
| 不 mock：`describe` / `query` 原语 | 真实委托链（薄包装的价值在于「不复制语义」，替身化无法证明这一点） | 三包装行 |

### `packages/desktop/src-tauri/crates/infra/agent/src/sdk/db_tool.rs` -> `packages/desktop/src-tauri/crates/infra/agent/src/sdk/db_tool_test.rs`

**新建 sidecar**（`sdk/mod.rs` 的 `#[cfg(test)]` 区同批加 `mod db_tool_test;`，字母序落 `context_test` 之后 / `explore_note_test` 之前）。装置：`tempfile` 目录 + 真 `Store::open_global` / `Store::open_workspace`（红宝单句柄，进程内各开一次）+ 真写面播种；`DbContext::new` 显式持两 `Arc<Store>`（不经 `WorkspaceStores`——装配面归 `compose_test.rs`）。

#### 待测功能

- `DbContext::new(global, workspace)`: 双库句柄构造（均为既有实例，MUST NOT 二次 open）。
- `DbContext::execute(name, input)`: 两工具执行体——`db_tables` 两形态 / `db_query` 全链（scope 解析 → 形状解析 → 敏感字段守卫 → `store` 原语 → 预算闸 → 打码 → 结果成形），未知名 `Err`，出口过 `tools::cap_result`。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `DbContext::execute`（`db_tables` 无参） | 正向 | 结果为 JSON **数组** 14 行，逐行含 `dimension` / `table` / `count` / `summary`，不含 `fields` 键；`store_meta` 缺席；两库计数与播种量一致 | 单次 |
| `DbContext::execute`（`db_tables` 下钻） | 正向 | `{"table":"phase"}` → JSON **对象** `{table, dimension, summary, fields:[{name,type,doc}]}`；不含 `count` 键；`name` 为 camelCase 线名（`changeId` 在场、`change_id` 缺席） | 单次 |
| `DbContext::execute`（`db_tables` + `scope`） | 正向 | `scope = "user"` → 3 行；`"workspace"` → 11 行；缺省 → 14 行（两库全列）；下钻形态带 `scope` 时按该库解析 | 参数化迭代（3 形态） |
| `DbContext::execute`（`db_tables` 错误面） | 异常 | 未知表 `"phases"` → `Err` 含候选表名；入参非对象 / `table` 非字符串 / `table` 空白 / `scope` 非法值（`"global"` / `""`）→ `Err` 引「列合法值」（`user` / `workspace`） | 参数化迭代（5 形态） |
| `DbContext::execute`（`db_query` 全链） | 正向 | `where` + `order` + `select` + `offset` / `limit` → 返回 `{table,total,matched,returned,truncated,truncatedBy,records}`；`records` 逐条 `{key, value}`、`key` 恒在；三元自洽（`returned ≤ matched ≤ total`）；`truncatedBy` 未触发为 `null` | 单次 |
| `DbContext::execute`（`db_query` `scope`） | 正向 | 缺省 `workspace` 走 workspace 句柄；`scope: "user"` 走全局句柄（`agent_provider` 可查）；两库清单互不混列（同表名跨维度不可达） | 参数化迭代（2 形态） |
| `DbContext::execute`（`db_query` `scope`） | 异常 | 表维度与 `scope` 不符（`scope: "workspace"` 查 `agent_provider`）→ `Err` 携「该表属 user 维度，请加 `scope: \"user\"`」；`scope: "global"` / `""` / `1` → `Err` 列两合法值；不静默空结果、不自动路由 | 参数化迭代（4 形态） |
| `DbContext::execute`（`db_query` 入参透传） | 边界 | 入参含 `scope` 时 `QuerySpec`（`deny_unknown_fields`）**不得** `Err`（执行体先摘 `scope`）；`where` / `order` / `select` 显式 `null` 与服务端缺省等价 | 参数化迭代（3 形态） |
| `DbContext::execute`（预算闸） | 正向 | `session_event` 大行语料（行均 ≥ 1.5 KB）× `limit: 50` → `truncated: true` + `truncatedBy: "bytes"` + `returned < matched` + `hint` 含 `select` / `where` / `offset` 三指引；结果串可被 `serde_json::from_str` 完整解析（**无半截 JSON**）、串长 < 50_000 且全文无「已截断」留痕（`cap_result` 未触发） | 单次 |
| `DbContext::execute`（行数档） | 正向 | `phase` 语料 `matched = 30`、`limit: 20` → `truncated: true` + `truncatedBy: "limit"` + `returned == 20` + `returned < matched`；同一语料 `limit: 50` → `truncated: false` + `truncatedBy: null` | 组合迭代（2 档） |
| `DbContext::execute`（预算闸边界） | 边界 | 单条记录序列化即 > 40 KB → `returned == 0` + `truncatedBy: "bytes"` + 无半截 JSON；`used + n == RESULT_BUDGET_BYTES`（恰到界）→ 该条**入**结果，下一条即停（先到者止口径）；`limit: 1000` 对任一表 → 结果仍 < 50 KB（`cap_result` 对该工具不可达） | 组合迭代（3 形态） |
| `DbContext::execute`（`apiKey` 打码） | 正向 | `agent_provider` 无投影查询 → 每条 `value.apiKey == "***"`；`select: ["id","apiKey"]` 投影 → `apiKey` 仍 `"***"`；对结果串全文扫描：**不含**播种的明文 key（含其前缀段 `sk-…` 前 4 字符） | 参数化迭代（2 形态） |
| `DbContext::execute`（`apiKey` 比较面） | 异常 | `where: [{field:"apiKey", op:"LIKE", value:"sk-a%"}]`、`where: [{field:"apiKey", op:"=", value:"<明文>"}]`、`order: {field:"apiKey"}` 三形态 → 均 `Err` 且文案含「不可用于过滤 / 排序」；不返回任何记录（前缀与等值探测通道都不成立） | 参数化迭代（3 形态） |
| `DbContext::execute`（错误文案自纠） | 异常 | 未知字段 `change_id` → `Err` 含 `change_id` 与候选线名（至少 `changeId`）；未知 op `contains` → `Err` 含 10 词全表；类型不符 `attempt = "1"` → `Err` 含 `number`；数组下标 `event.blocks.0.name` → `Err` 含下标指引；`IN` 非数组 / `IS NULL` 带值 → `Err` 含期望形状 | 参数化迭代（6 形态） |
| `DbContext::execute`（点号路径 hint） | 边界 | `where` 含点号路径且 `matched == 0` → 返回体 `hint` 提示「嵌套路径无法静态校验，用 `db_tables` 下钻确认字段名」；`matched == 0` 且无点号路径 → 无该 hint（不误导） | 组合迭代（2 形态） |
| `DbContext::execute`（未知名 / 出口） | 异常 | `name` 为 `"db_tables2"` / `""` → `Err` 含未知名；成功路径出口过 `cap_result`（结果串 ≤ 50 KB 恒成立，无二次截断） | 参数化迭代（2 形态） |
| `DbContext::execute`（组合行：执行体 → store 原语） | 正向 | 真实双库：workspace 句柄取「主仓 root」对应库、global 句柄取全局库（两 `Arc` 由测试显式构造，与 change 会话 worktree cwd 无关——身份锚判据由 `compose_test.rs` 承接）；`db_tables` 与 `db_query` 在同一 `DbContext` 上连续调用互不污染（无跨调用状态） | 组合迭代（2 工具 × 2 库） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 数据根 / 双库文件 | `tempfile` 真目录 + 真 `Store::open_global` / `open_workspace`（进程内各一次、`Arc` 共享；不 mock 存储层——预算闸与打码的输入必须是真 serde JSON 值） | 全章节 |
| 不 mock：`store::{describe*, query}` | 真实原语（design「MUST NOT 在 `db_tool` 实现第二份查询语义」的机械判据：文案与词表必须来自同一来源） | 全章节 |
| 不 mock：`serde_json` | 真序列化 / 反序列化（无半截 JSON 的判据只能由真解析器给出） | 预算闸与打码行 |
| 不 mock：`tools::cap_result` | 真兜底（断言「未触发」比替身更严格：全文无截断留痕） | 预算闸 / 出口行 |

### `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs` -> `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount_test.rs`

**既有 sidecar 改造**：`EXPECTED_NAMES` 11 → **13**（尾部 + `db_tables` / `db_query`）、tier 计数 Read 7 → **9**、`tier/exec` 归属表逐项补齐。design「公共函数 / API」表无本文件行（改动为常量 / 定义 / `ToolExec::Db` 变体 / tier，均登记在「类型定义」与「修改文件」表），测试落点由 design「测试文件」表登记为「两挂载项在清单内 / tier = `Read` / `policy::allows` 三档放行 / 定义 schema 形状」。

#### 待测功能

- `registry()`: 静态挂载清单 13 项（七内建 + `create_explore_note` + 三 archi + `db_tables` / `db_query`），名字唯一、序 = 登记序，`Vec::with_capacity` 与 doc 的「十三项」同批收敛。
- `policy::allows(mode, tier)`: 两 DB 工具 `tier = ToolTier::Read` → 三档（`Default` / `AcceptEdits` / `BypassPermissions`）全放行（无单独放行名单、无 `ToolTier::Protocol` 借用）。
- `db_tables_definition()`: `name == "db_tables"`、`parameters` 两属性（`table` / `scope`）与 `required = []`、description 覆盖 `scope` 缺省口径与「带 `table` 即下钻字段」的两形态语义。
- `db_query_definition()`: `name == "db_query"`、`parameters` 七属性（`table` / `scope` / `where` / `order` / `select` / `offset` / `limit`）与 `required == ["table"]`、description 覆盖 `QUERY_OPS` 全 10 词、`limit` 缺省 20 / 上限 1000、「预算是实际生效值」、`session` / `session_event` 自读警示。
- `definitions(injections, question)`: 名字清单 → 下发清单（`tools = None` 仍前七项、`Some([])` 零下发、两 DB 名按 registry 序下发、未知名 `Err`、`finish` 收口工具尾追加），`ToolExec::Db` 归属两 DB 项。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `registry()` | 正向 | 13 项名字唯一且有序：前七恰 `TOOL_NAMES`（`tools = None` 语义载体），第 8 项 `create_explore_note`，第 9~11 项三 archi，末两项 `db_tables` / `db_query` | 单次 |
| `registry()` tier | 正向 | `ToolTier::Read` 恰 9 项（含两 DB 工具）、`Write` 3 项、`Execute` 1 项、`Protocol` 0 项（静态表内）；两 DB 项 `exec == ToolExec::Db` | 单次 |
| `policy::allows` | 正向 | 三档（`Default` / `AcceptEdits` / `BypassPermissions`）× 两工具 → 全 `true`（无单独放行名单） | 组合迭代（3 档 × 2 工具） |
| `db_tables_definition` / `db_query_definition` | 正向 | `name` 恰两常量；description 含 `QUERY_OPS` 全部 10 词逐词、`scope` 两值（`user` / `workspace`）与缺省口径、`limit` 缺省 20 / 上限 1000、「预算是实际生效值」表述、`session` 与 `session_event` 自读警示、先 `db_tables` 再 `db_query` 的引导 | 组合迭代（2 定义 × 6 断言组） |
| `db_tables_definition` / `db_query_definition` | 正向 | `parameters` 形状：均 `type = object`；`db_tables` 两属性（`table` / `scope`，`required` 为空数组）；`db_query` 含 `table` / `scope` / `where` / `order` / `select` / `offset` / `limit`，`required == ["table"]`；`scope` 为两值 enum | 组合迭代（2 定义 × 3 形状面） |
| `definitions` | 正向 | 两 DB 名进清单即下发：`injected(&["read","db_tables"])` → 两项按 registry 序（`read` 在前）；`injected(&["db_query","read"])` 同序（清单序不决定下发序） | 参数化迭代（2 形态） |
| `definitions` | 异常 | 未知名仍 `Err`（`"db_query2"` / `"DB_QUERY"` / `"db tables"` 各一次），文案携非法名，不返回部分清单 | 参数化迭代（3 形态） |
| `definitions`（`tools = None`） | 边界 | 仍恰前七项（不含 DB 两名、不含 archi、不含 `create_explore_note`）——`tools: None` 语义零改动 | 单次 |
| `definitions` | 边界 | `Some([])` → 零下发；两 DB 名 + `finish` 位 → 收口工具仍尾追加（两分支互不依赖） | 组合迭代（2 形态） |
| ↔ 编译闭合 | — | `Vec::with_capacity(11)` → `13`、`registry()` / `build_registry()` doc 的「十一项」→「十三项」、`ToolExec` 增变体后 loop 分派 `match` 完备（无运行期断言） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 零 mock | `registry()` / `definitions` / `lookup` / `policy::allows` 全为进程内纯函数（`OnceLock` 静态表，无 IO / 无 spawn / 无网络）；`orchestration::finish` 真实调用（既有先例） | 全章节 |
| 不 mock：`store::QUERY_OPS` | 真实常量（description 与 `Op::parse` 同源正是被断言的性质，替身化会把「两处词表漂移」藏起来） | 定义形状行 |

### `packages/desktop/src-tauri/crates/infra/agent/src/sdk/policy.rs` -> `packages/desktop/src-tauri/crates/infra/agent/src/sdk/policy_test.rs`

**既有期望随动（`policy.rs` 生产源零 diff）**：本变更只动**对拍表的期望值**——「逐工具三档等价于原三张按名常量表」用例的 `Read` 档名单行尾部补 `db_tables` / `db_query`（原七名 → **九名**，两 DB 工具归 `Read` 三档全放），用例名与 doc 的「十一项」→「十三项」同步；`Write` 档三名（`write` / `edit` / `create_explore_note`）、`Execute` 档一名（`bash`）与「三档 × 四 tier」矩阵**零改动**。性质登记：这是「挂载清单 11 → 13 项」在本文件的必然投影（design「编译闭合批」倒数第二行），**零新增用例、零断言语义变更**；`policy.rs` 的 `allows(mode, tier)` 签名轴与常量表退役自证口径不变。

#### 待测功能

- `mount::registry()` × `policy::allows(mode, tier)`（对拍表）: 逐工具三档等价表的 `Read` 档名单扩到九名（含 `db_tables` / `db_query`），`Write` / `Execute` 档名单与三档判定矩阵不变（既有期望随动；`policy.rs` 无公共 API 变更）。
<!-- design.md 未声明本文件的公共 API 变更：policy.rs 零 diff，本节的测试对象是既有对拍表的期望值随挂载清单补齐 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 「期望随动」十三项挂载逐工具三档等价于原三张按名常量表 | 正向 | `Read` 档名单行尾部补 `db_tables` / `db_query`：两工具 × 三档（`Default` / `AcceptEdits` / `BypassPermissions`）全 `true`；`Write` / `Execute` 档七工具三档判定与原常量表逐格相等；`unreachable!` 封闭集扩到 13 名后不再触发 | 参数化迭代（13 名 × 3 档） |
| 「期望随动」用例名与 doc | 正向 | 用例名 / 注释的「十一项」→「十三项」；`挂载清单内十一项名字两两不同` 同步为「十三项」（断言只判名字两两不同，口径零改动） | 单次 |
| 三档 × 四 tier 矩阵与幂等面 | 边界 | 「既有…」三档 × 四 tier 的 12 格全覆盖 + 连续三次同判、`档位归属抽样`（`read` / `archi_module` / `write` / `create_explore_note` / `bash`）、`accept_edits` / `bypass` 与 `default` 分化断言**零改动**全绿（两 DB 工具走 `Read` 档，不进抽样名单，无需扩断言） | 单次 |
| 常量表退役自证 | 边界 | 「既有…」`policy.rs` 代码行内 `READONLY_TOOLS` / `WRITE_TOOLS` / `EXECUTE_TOOLS` / `listed` 零命中 + `fn allows` 恰 1 处（签名轴 `(AgentPermissionMode, ToolTier)`）零改动全绿 | 单次 |
| 清单外工具名 | 异常 | 「既有…」`"" / Write / rm_rf / web_search / archi_write / READ / create_Explore_Note` 七名仍不在清单内（名单零改动；两 DB 工具已在清单内，不进该名单） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 零 mock | 被测面为 `mount::registry()` 静态表 × `policy::allows` 纯函数对拍（无 IO / 无网络 / 无子进程），既有装置原样复用 | 全章节 |
| 不 mock：`mount::registry()` | 真实 13 项静态清单（对拍表的意义正是「清单补项后档位不漂移」，替身化会让该性质空转） | 全部用例 |

### `packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools.rs` -> `packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools_test.rs`

**既有 sidecar 追加**：`input_path` 显式 `None` 臂由三名扩到五名；`execute` 七工具与 `cap_result` 面**零改动**（既有用例零改动通过即回归判据）。

#### 待测功能

- `tools::input_path(name, input)`: 五名显式 `None` 臂（三 archi + 两 DB），路径沙箱改写面对两工具恒原样返回。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `tools::input_path` | 正向 | `"db_tables"` / `"db_query"` → `None`（即便入参含 `path` / `file` / `pattern` 字段），两工具各一形态 | 参数化迭代（2 工具 × 2 入参形态） |
| `tools::input_path` | 正向 | 五名统一参数化（`archi_module` / `archi_search` / `archi_api` / `db_tables` / `db_query`）→ 全 `None`；基准对照 `read` → `Some(path)`、`glob` → `Some(pattern)` 不变 | 参数化迭代（7 名） |
| `tools::input_path` | 边界 | 空入参 `{}` / 非对象入参（`null` / `[]` / `"str"`）/ `path` 非字符串（数字 / 对象 / 数组）/ `path` 为空串 → 两 DB 名恒 `None`、不 panic | 参数化迭代（6 形态） |
| `tools::input_path` | 边界 | 极端名形态：`"db_query "` / `"DB_TABLES"` / `"db_tables_db_query"` → 落到默认 `path` 分支（`Some` / `None` 取决于入参），证明判据是精确名而非前缀嗅探 | 参数化迭代（3 形态） |
| 既有 `execute` / `cap_result` | 正向 | 「既有…」七工具 `execute(root, name, input)` 纯函数语义与 L1 收口用例零改动全绿（签名零改动） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 零 mock | 被测面为纯函数（名字 → `Option<&str>` 投影），无 IO | 全章节 |
| 文件系统 | 既有用例用 `tempfile` 真目录（新用例不触文件系统：`input_path` 纯解析） | 全章节 |

### `packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs` -> `packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop_test.rs`

**既有 sidecar 改造 + 追加**：`LoopTurn` 加 `db` 字段（既有 ~40 处字面构造补 `db: None`，夹具随动、断言口径零改）；`ToolExec::Db` 分派臂新增用例。design「公共函数 / API」表无本文件行（改动为「类型定义」表的 `LoopTurn.db` 加位与守卫链第 6 段分派臂），本文件的用例对象是分派臂的可观测行为（AC-11 后半 / AC-8 零介入），落点由 design「测试文件」表之外的 `tasks.md` 交接备忘登记（`sdk/loop_test.rs`：`ToolExec::Db` 两态）。

#### 待测功能

- `loop::run()` 的 `mount.exec` 分派臂 `ToolExec::Db`: `turn.db = Some(ctx)` → `ctx.execute(&name, &input)` 的 `Ok` / `Err` 双态折叠为 `(content, is_error)`；`turn.db = None` → 「专用工具执行上下文未装配」+ `is_error` 回灌，run 不中断。
- `loop::run()` 的成员校验段（`mount::definitions` 缺席 → `SystemNotice{subtype:"tool_unavailable"}`）: 名单不含两 DB 工具的会话发起 `db_query` 走既有拒绝臂、执行体未被触达（回归判据）。
- `sandbox_rewrite(root, name, input)` 与写保护闸 `protect::evaluate`: 两工具名不在改写 / 拦截面 → 入参原样传执行体、零 `sandbox_denied` / `permission_denied`（`tools::input_path` 五名 `None` 臂的上游效应）。
- `LoopTurn.db`: 新字段（`Option<DbContext>`，与 `explore_note` / `archi` / `ask` 同列）—— 全部字面构造点补 `db: None` 后既有轮次语义零改动（编译闭合锚）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ToolExec::Db` 分派臂 | 正向 | 名单含 `db_tables` + `db: Some(DbContext)` → 一条 tool_result、`is_error == false`、内容为合法完整 JSON（含 14 行表清单）、零 `SystemNotice`（成员校验 / policy / sandbox / protect 全放行）；续轮照常、`TurnDone{is_error:false}` | 单次 |
| `ToolExec::Db` 分派臂 | 正向 | `db_query` 同链：`{table:"phase", select:[...]}` → 非 `is_error`、`value` 逐键载荷可选；三档（`Default` / `AcceptEdits` / `BypassPermissions`）逐档零 `permission_denied` | 组合迭代（3 档） |
| `ToolExec::Db` 分派臂 | 异常 | `db: None` + 名单含 `db_query` → tool_result `is_error == true` 且内容为「专用工具执行上下文未装配」；run 不中断（后续轮正常、`TurnDone{is_error:false}`）、零 panic | 单次 |
| 成员校验（既有） | 异常 | 名单不含两 DB 工具（debug 形态 `tools: None` / archi 形态）而模型发起 `db_query` → `SystemNotice{subtype:"tool_unavailable"}` + `is_error` 回灌，run 不中断；结果内容为拒绝文案（非 `db_tables` / `db_query` 的 JSON 形态），可见执行体未被触达；文案与既有未知名臂逐字同形 | 参数化迭代（2 臂形态） |
| `ToolExec::Db` 分派臂 | 边界 | 沙箱零介入：入参含 `path` / `pattern` 字段 → 原样传给执行体（无改写、无 `sandbox_denied`）；保护闸零介入（两工具名不在 `protect::evaluate` 匹配面 → 无 `permission_denied`） | 组合迭代（2 闸 × 2 工具） |
| `ToolExec::Db` 分派臂 | 边界 | 执行体 `Err` 串（未知字段 / 未知 op）→ `is_error == true` 且内容为自纠文案；`content` 长度 ≤ `MAX_RESULT_BYTES`（出口收口不二次触发） | 参数化迭代（2 错误形态） |
| ↔ 编译闭合 | — | `LoopTurn` 全部字面构造点补 `db: None`（生产 1 处 + `loop_test.rs` / `runner_test.rs` 夹具 40+ 处；断言口径零改动） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 模型 / provider 端点 | 既有 rig `test_utils` 流缝装置（`FakeModel::with_turns` + `raw_tool_call` / `raw_text`）；**不触真实网络** | 全部用例 |
| 工作目录 | `tempfile` 真目录作 `cwd`（preamble / protect 读面需真实文件） | 全部用例 |
| 双库句柄 | 真 `Store`（`tempfile` 下 `open_global` / `open_workspace`）+ `DbContext::new`（被测链上游必须真实：打码与预算闸在结果串上断言） | 分派臂用例 |
| 不 mock：`policy` / `sandbox` / `protect` / `mount` / `tools` | 真实组合（AC-8「零介入」正是闸门不介入这一事实，替身化会让断言空转） | 全章节 |
| 事件通道 | 真实 `mpsc::channel(256)`（既有 `drive_loop_turn` 装置） | 全章节 |

### `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs` -> `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner_test.rs`

**既有 sidecar 改造 + 追加**：`PumpPayload` 加两库句柄字段、`SdkRunner` 加两字段与 `with_db_stores` builder（`new` 签名零改动，构造点零波及）；新增 `mint_db_context` 判据用例（与 `mint_archi_context` 用例同族同装置）。

#### 待测功能

- `mint_db_context(injections, global, workspace)`: per-turn 铸造判据单点（名单含任一 DB 工具名且两把句柄皆在场才 `Some`）。
- `SdkRunner::with_db_stores(global, workspace)`: builder 旁路（`new` 签名不动，与 `with_workspace_root` 同构）。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `mint_db_context` | 正向 | 名单含 `db_tables`（仅此一名）→ `Some`；名单含 `db_query`（仅此一名）→ `Some`；两名皆含 → `Some` | 参数化迭代（3 名单） |
| `mint_db_context` | 正向 | 名单为九名 explore 臂完整清单 → `Some`；名单为 change 臂（七工具 + archi + 两 DB）→ `Some` | 参数化迭代（2 臂） |
| `mint_db_context` | 异常 | 名单不含任一（七内建 / archi 十名 / `create_explore_note` 单名）→ `None`；`tools = None`（内建七件底座）→ `None`；`Some([])` → `None` | 参数化迭代（4 形态） |
| `mint_db_context` | 异常 | 句柄缺席：global `None`、workspace `None`、两位皆 `None`（名单含两 DB 名）→ 均 `None`（fail-closed，不铸半截上下文） | 参数化迭代（3 形态） |
| `mint_db_context` | 边界 | 近似名不命中：`"db_query2"` / `"DB_QUERY"` / `"db_query "` / `"db"` / `""` / `"字".repeat(1200)` → `None`（精确名判据，非前缀 / 大小写嗅探） | 参数化迭代（6 形态） |
| `mint_db_context` | 边界 | 超大名单（100 名 + 末位含 `db_tables`）→ `Some`（判据不依赖位置）；两 DB 名各出现多次 → `Some`（去重无关） | 组合迭代（2 形态） |
| `SdkRunner::with_db_stores` | 正向 | builder 后 `open_session` 成功（不 `Err`）、`SessionOpen.injections` 原样透传；与 `with_max_tokens` / `with_web_search` / `with_ask_registry` / `with_workspace_root` 链式组合后仍成功 | 组合迭代（2 builder 序） |
| `SdkRunner::with_db_stores` | 边界 | 未调 builder（两字段 `None`）→ `open_session` 仍成功（缺省形态不 panic / 不 Err）；`new` 签名零改动（既有构造点零改动即回归） | 单次 |
| 既有用例 | 正向 | 「既有…」`mint_archi_context` / `derive_output_tokens` / provider 组装 / 泵装配用例零改动全绿（`PumpPayload` 两字段为纯加位） | 单次 |
| ↔ 编译闭合 | — | `PumpPayload` 字面构造（生产 1 处 + 夹具）补 `db_global` / `db_workspace`；`LoopTurn` 铸造点补 `db` | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 双库句柄 | `tempfile` 真目录 + 真 `Store` 的 `Arc`（`mint_db_context` 只判在场性，不读库；但 `Arc<Store>` 无法以免实现替身替代——句柄类型即被测签名） | 全部用例 |
| provider 配置 / 网络 | 既有装置：`EngineConfig` 指向 `127.0.0.1:9` 不可达端口（`open_session` 不发起请求；断言不依赖网络结果） | builder 行 |
| 不 mock：`contains_tool` 判据 | 真实 `SessionInjections` 形态（被测面就是「名字清单包含」这条判据） | 全部用例 |

### `packages/desktop/src-tauri/crates/infra/agent/src/lib.rs` -> `packages/desktop/src-tauri/crates/infra/agent/src/lib_test.rs`

**既有 sidecar 追加**：`EngineFacade` 加两字段与 `with_db_stores` builder（`new` 与其后逐字段字面构造共七处各补两行）；新增 builder 缝用例。

#### 待测功能

- `EngineFacade::with_db_stores(global, workspace)`: 组合根注入缝（与 `with_explore_note_store` 同型）；`runner_for` 透传。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `EngineFacade::with_db_stores` | 正向 | `EngineFacade::new().with_db_stores(g, w).runner_for(Sdk, cfg)` → `Ok`（不因新增字段缺省而 `Err`）；`EngineKind::Cli` 臂仍返回 `CLI_TENANT_RETIRED` 文案（既有语义零改动） | 参数化迭代（2 引擎臂） |
| `EngineFacade::with_db_stores` | 正向 | 与其它 builder 链式组合任意序（`with_resume_transcript` / `with_context_window` / `with_max_tokens` / `with_web_search` / `with_ask_registry` / `with_ask_port` / `with_explore_note_store` / `with_workspace_root`）→ 每个位面保持设定值（`..self` 形态不误清位），`runner_for` 仍 `Ok` | 组合迭代（2 序 × 9 builder） |
| `EngineFacade::with_db_stores` | 边界 | 不调用该 builder（两字段 `None`）→ `runner_for` 仍 `Ok`（未注入 = 上下文缺席，非错误）；`Default::default()` 与 `new()` 等价 | 参数化迭代（2 构造面） |
| `EngineFacade::with_db_stores` | 边界 | 二次调用后者覆盖前者：`with_db_stores(g1,w1).with_db_stores(g2,w2)` → 不 panic、`runner_for` 成功（覆盖语义与既有 builder 同构） | 单次 |
| ↔ 编译闭合 | — | 七处逐字段字面构造各补 `db_global` / `db_workspace` 两行（`new` / `with_resume_transcript` / `with_context_window` / `with_max_tokens` / `with_web_search` / `with_explore_note_store` / `with_workspace_root`）；`with_ask_registry` / `with_ask_port` 走 `..self` 零改动 | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 双库句柄 | `tempfile` 真目录 + 真 `Store` 的 `Arc`（句柄类型即被测签名） | 全部用例 |
| provider 配置 / 网络 | 既有装置：不可达端点 `127.0.0.1:9`（不建连、不断言网络结果） | 全章节 |
| 不 mock：`runner_for` | 真实构造链（被测面是「字段在 builder 链里不丢」，替身 runner 会让该断言失效） | 全部用例 |

### `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs` -> `packages/desktop/src-tauri/crates/infra/agent/src/compose_test.rs`

**既有 sidecar 改造 + 追加**：`EXPLORE_TOOLS` 7 → **9**、change 臂在矩阵之后恒 push 两名、`compose_turn` 从 `stores.global_shared()` 与既有 `store` 组装 `DbContext` 原料。既有 explore 七名 / archi 十名 / debug `tools = None` 断言的**期望值**随之更新（这是 spec 口径变化，非口径改写）。design「公共函数 / API」表无本文件行（改动面是名字清单与装配原料），测试落点由 design「测试文件」表登记（`compose_test.rs`：explore 9 名 / change 恒 2 / archi / debug 不含）。

#### 待测功能

- `assemble_injections(provenance)`（explore 臂）: `EXPLORE_TOOLS` 7 → 9（六工具 + `create_explore_note` + `db_tables` / `db_query`），`finish = None` / `preamble = None` / `ask = Some(Generic)` 零改动。
- `assemble_injections(provenance)`（archi 臂）: 七内建 + 三 archi = **10 名**、**不含**两 DB 名（`compose.rs:62-66` 实测；proposal 的「9 名」为笔误）；`preamble` 在场判定零改动。
- `assemble_injections(provenance)`（其余来源）: `debug` / 未知 / 空来源 → `tools = None`（内建七件、不含两 DB 名）、`ask = Some(Generic)`。
- `assemble_injections(provenance)`（change 臂）: 七内建 + `archi_matrix` 命中行 + **矩阵之后恒 push** 两 DB 名（不分 phase / 不分矩阵命中 / 畸形 `source_ref` 同样命中），`ask = Some(Flow)` 同构。
- `injected(names, preamble, finish, ask, provenance)`: 名字清单落地 + 成员校验（`mount::lookup` 未知名 `Err` 携名与来源；两 DB 名命中不 `Err`）。
- `compose_turn(...)`: 双库原料装配 —— `stores.global_shared()` 与既有 `Arc<Store>`（move 前 `Arc::clone`）经 `.with_db_stores(...)` 下行；库身份锚恒 workspace root，不随会话 `cwd`（worktree）派生。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `assemble_injections`（explore 臂） | 正向 | 9 名逐名有序：`read` / `grep` / `glob` / `ls` / `edit` / `bash` / `create_explore_note` / `db_tables` / `db_query`（无 `write`）；`finish = None`、`preamble = None`、`ask = Some(Generic)`；名字清单不含 `AskUserQuestion` | 单次 |
| `assemble_injections`（archi 臂） | 正向 | 10 名（七内建含 `write` + 三 archi）；**不含**两 DB 名；`preamble` 在场、`ask = Some(Generic)`（`source_ref` 两形态不改变该臂） | 参数化迭代（2 source_ref） |
| `assemble_injections`（其余来源） | 正向 | `debug` / `unknown-source` / `""` → `tools = None`（内建七件，不含 DB）、`preamble = None`、`finish = None`、`ask = Some(Generic)` | 参数化迭代（3 来源） |
| `assemble_injections`（change 臂 · 矩阵命中） | 正向 | `<id>/dev-design/executor/1` → 七工具 + `archi_module` + `archi_search` + `db_tables` + `db_query`（DB 两名在 archi 名之后）；`finish = None`、`ask = Some(Flow)` | 单次 |
| `assemble_injections`（change 臂 · 矩阵未命中） | 正向 | `<id>/test-gen/executor/1`、`<id>/archive/spec_sync/1`、`<id>/initialize/executor/2` → 七工具 + 两 DB 名（零 archi 工具）；不抛装配错误、不退化为全量 archi 工具 | 参数化迭代（3 行） |
| `assemble_injections`（change 臂 · 畸形 source_ref） | 异常 | `<id>/implement/executor`（段数不足）、`<id>/a/b/x`（attempt 非数字）、`""`、`None` → 七工具 + 两 DB 名 + 零 archi 工具（fail-closed 仍含 DB 两名）；`ask` 仍 `Some(Flow)` | 参数化迭代（4 形态） |
| `assemble_injections`（change 臂 · 全矩阵） | 边界 | 7 行矩阵逐行 + 2 畸形 + 空：每条变更名单都含 `db_tables` / `db_query` 各恰一次（不分 phase / 不分矩阵命中） | 组合迭代（7 行 + 3 畸形） |
| `assemble_injections`（成员校验） | 异常 | 未知名仍 `Err` 携名与来源（如注入面出现 `"fly"`）；两 DB 名经 `mount::lookup` 命中（不 `Err`） | 参数化迭代（2 形态） |
| `compose_turn`（组合行：装配原料） | 正向 | `WorkspaceStores` + workspace `Arc<Store>` 双句柄在场 → `compose_turn(...)` 不 `Err`、`begin` 成功、会话行落 workspace 库（`global_shared()` 与 `for_root` 两条原料齐备，无二次 open / 无锁冲突） | 单次 |
| `compose_turn`（组合行：两臂下发面） | 正向 | explore 与 change 两 provenance 各 `begin` 一轮 → 捕获的 `TurnRequest.injections.tools` 含两 DB 名（与 `assemble_injections` 单源一致，装配链无第二份清单） | 参数化迭代（2 臂） |
| `compose_turn`（组合行：身份锚） | 边界 | 会话 `cwd` 取 worktree（`for_root(root)` 与 `cwd` 不同路径）→ 装配的 workspace 库句柄仍为 root 对应实例（`Arc::ptr_eq(&env.store(), &stores.for_root(&root))`），DB 句柄不经 cwd 派生 | 单次 |
| `compose_turn` | 异常 | 解析失败（无默认 agent / provider 两成因）→ `Err` 文案零改动；失败路径零库写入（会话行 / 事件零新增） | 参数化迭代（2 成因） |
| 既有用例 | 正向 | 「既有…」`resolve_agent_engine` / `archi_matrix` / 快照定型 / prior_handle 透传等用例除名字清单期望值外零改动通过 | 单次 |
| ↔ 编译闭合 | — | `compose_turn` 门面链尾补 `.with_db_stores(...)`、`composed_with_fakes` 门面夹具与 `worker_test.rs` 构造点随动（断言口径零改动） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 数据根 / 库文件 | 既有 `DualEnv` 装置（`tempfile` 双临时目录 + 真 `WorkspaceStores`）；存储层不 mock | 全章节 |
| runner / sink | 既有手写 `RecordingRunner`（捕获 `TurnRequest.injections`）+ `RecordingSink`（进程边界替身：真实引擎需网络与真实 provider）；`compose_turn` 真实构造链行用真 `SdkRunner`（不可达端点） | 名字清单行 / 装配原料行 |
| provider 配置 | fixture 落库（真配置读面），`base_url` 指向 `127.0.0.1:9` 不可达端口 | 全部用例 |
| 不 mock：`assemble_injections` / `archi_matrix` / `injected` | 真实纯函数（被测面即「名字清单单源」） | 名字清单行 |

### `packages/desktop/src-tauri/src/commands/db/mod.rs` -> `packages/desktop/src-tauri/src/commands/db/mod_test.rs`

**既有期望随动（`db/mod.rs` 生产命令面零 diff）**：本变更只动**注册表补齐在查看页命令层的投影**——`db_models` 的 workspace 维度清单期望 8 → **11** 行（计数向量与名集尾部补 `run` / `run_node` / `run_edge`，三者计数按播种量为 0），两处用例名与断言说明文案「八行」→「十一行」同步；user 维度三行、跨维度扫描 `Err`、blank root 空结果、坏文件 `Err`、分页拼接、薄包装「命令面 serde 值与直连 `list_models` 一致」等其余断言**零改动**。性质登记：这是「`MODEL_ENTRIES` 11 → 14 行」在既有「新模型零改动可浏览」红利上的必然投影（design「编译闭合批」末行 / proposal「测试文件」表末行），**零新增用例、零断言口径变更、不触生产命令面**。

#### 待测功能

- `db_models(state, dimension, root)`（生产实现零改动）: workspace 维度清单 11 行的计数向量与名集（含三新表），命令面与直连 `Store::list_models` 的 serde 值一致（薄包装不加工）。
- `db_models` 两库名集互不混列: user 维度三行 / workspace 维度十一行，各自不出另一维度名。
<!-- design.md 未声明本文件的公共 API 变更：db/mod.rs 生产命令面零 diff，本节的测试对象是既有清单期望值随 MODEL_ENTRIES 11 → 14 行收敛 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 「期望随动」`db_models_scope_workspace` | 正向 | 计数向量尾部补 `("run",0)` / `("run_node",0)` / `("run_edge",0)` → 恰 11 项且序 = 注册表登记序；用例名与说明文案「八行」→「十一行」；「命令面 serde 值 == 直连 `list_models`」断言零改动 | 单次 |
| 「期望随动」两库互不混列 | 正向 | workspace 名集补三新表 → 11 名（`agent_run` … `step` + `run` / `run_node` / `run_edge`）；user 名集仍恰三行（`workspace` / `agent_provider` / `agent_instance`），零交集 | 参数化迭代（2 维度） |
| 跨维度 / 边界面 | 异常 | 「既有…」跨维度模型名扫描 `Err`（文案含「未知模型」）、blank root 空结果、坏库文件 `Err`（`NotFound` / 解码失败两成因）三态**零改动**全绿 | 参数化迭代（3 形态） |
| 分页与薄包装 | 边界 | 「既有…」`db_records` 按主键自然序分页拼接不重不漏、`MAX_SCAN_LIMIT` 口径相关断言零改动全绿（注册表补齐不改分页语义） | 单次 |
| 生产命令面零 diff（结构） | — | `db/mod.rs` 的实现（`db_models` / `db_records` 薄包装）零 diff——无运行期断言，归桌面端 `git diff` 审查（见「不可测试项」） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 数据根 / 库文件 | 既有 `Env` 装置（`tempfile` 双临时目录 + 真 `WorkspaceStores::open` + tauri `App` state 注入）；存储层不 mock | 全章节 |
| 播种 | 既有 `seed_session` / `append_events` / `create_explore_record` 真写面 API，不直插 native_db | 计数向量行 |
| 不 mock：`store::WorkspaceStores` / `Store::list_models` | 真实实例与真实读面（期望值随动的意义正是「注册表补三行即出现在查看页」，替身化会让该性质空转） | 全部用例 |

---

## 不可测试项

| 模块 | 原因 | 替代验证 |
|---|---|---|
| `packages/desktop/src-tauri/crates/infra/store/Cargo.toml`（`specta-serde = { workspace = true }` 一行） | 依赖清单无运行期行为可断言 | 编译闭合（`describe_test.rs` / `query_test.rs` 用到 `specta_serde::Format`，缺依赖即编译失败）+ workspace 条目已在案（`specta-serde = "=0.0.13"`，零新外部依赖）+ `git diff` 审查 |
| `packages/desktop/src-tauri/crates/infra/store/src/lib.rs`（`mod describe;` / `mod query;` + 六类 `pub use`） | 纯声明与再导出，无运行期分支 | 编译锚：`describe_test.rs` / `query_test.rs` 以 `crate::describe::…` / `crate::query::…` 导入、`db_tool_test.rs` 以 `store::{ModelDoc, QuerySpec, QUERY_OPS, …}` 导入，任一名丢失即编译失败；另侧锚 = 新类型**不 derive `specta::Type`**（`bindings:export` 零 diff 由桌面端执行复核） |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mod.rs`（`pub(crate) mod db_tool;` 一行） | 模块声明行，无运行期行为 | `db_tool_test.rs` 编译可达即锚（缺声明则测试文件不被编译、`cargo clippy --all-targets` 亦会因 `cfg(test)` 声明缺失暴露） |
| `packages/desktop/package.json`（`version` 一行） | 元数据面，无运行期行为；判据是**相对递增**（落地时 `HEAD` 现值 +1 patch，MUST NOT 写死字面号） | 桌面端 diff 审查：`git diff HEAD -- packages/desktop/package.json` 的 `version` 行恰 +1 patch（`0.6.43` → `0.6.44`）；`src-tauri/tauri.conf.json` 仍为 `"version": "../package.json"`；`src-tauri/Cargo.toml` 的 crate 版本行未动 |
| AC-12 的**结构事实**（`AgentEvent` 六变体与块词汇零新增、零新 subtype、`src-tauri/src/bindings/**` 与 `packages/desktop/src/**` 零 diff、`src-tauri/src/commands/**` 的**生产命令面**零 diff） | 「某词汇 / 某文件未被改」是源码事实，运行期断言只能证行为等价 | 桌面端 `git diff` 逐文件审查；**唯一登记例外** = `src-tauri/src/commands/db/mod_test.rs` 的注册表行数 / 名集期望随动（非生产命令面、非越界，见其 per-file 章节）；行为侧证 = `loop_test.rs` / `mount_test.rs` / store 事件面既有用例零改动全绿（工具结果仍走既有 `Message{role: Tool}` 密封路径） |
| AC-8 的**只读结构事实**（`describe.rs` / `query.rs` / `db_tool.rs` 无写 API、`policy.rs` / `sandbox.rs` / `protect/**` / `crates/core/**` 零 diff） | 同上（缺席事实无运行期投影） | 桌面端 `git diff` 逐文件审查 + 行为侧证（`query_test.rs` / `store_test.rs` 的「调用后表计数不变、`store_meta` 未改」行；`loop_test.rs` 的三档零 `permission_denied` 与沙箱零改写行） |
| 「`total` / `matched` / `records` 取**同一读事务**快照」（DD4 两路径同快照） | 并发写与查询的交错不可做确定性断言（redb 单进程 MVCC 下的竞态窗口无法稳定复现；为它引入 sleep 会让用例抖动） | 实现读码复核（`query::run` 单一 `r_transaction` 收口，DD4 明写「同一读事务」）+ 功能侧证：`total == 播种量`、`matched == 人工核算过滤量`、`returned ≤ matched ≤ total` 恒成立（`query_test.rs` 三元口径行） |
| 「全表 decode 耗时 / 内存上界 = 保留窗 ≤ `offset + limit` 行」（任务书风险表口径，70 万行外推 1.3~1.7 s） | 性能与环境相关属性，硬阈值断言会抖动；属设计已知情接受的代价 | 不做耗时断言；只断言功能性结果（`matched` 精确、页切无重无漏）。性能回归由人工实测承接（design「待决问题 4」留痕） |
| 「`db_tool.rs` 不实现第二份查询语义 / 不出现第二份字段表」（D1 / spec「说明来源与文档防腐」） | 「不存在第二份实现」是结构事实 | 行为侧证：错误文案与 op 词表与 `query.rs` 同源（`mount_test.rs` 断言 description 含 `QUERY_OPS` 逐词；`db_tool_test.rs` 断言错误文案含候选线名 / 词表）+ 桌面端读码复核（`db_tool.rs` 无过滤 / 排序实现） |
| 「`describe` / `query` 不 derive `specta::Type`，零 IPC / 零前端改动；注册表补齐在查看页自动出现三新表」 | 生成链幂等与「无新命令」是工具 / 缺席事实 | 桌面端执行 `bindings:export` / `api:check` 后 `git diff` 为空（`src/types/generated/**` 与 `.dev-team/architecture/api/**`）；行为侧证 = `envelope_test.rs` 的 `list_models` / `scan` 三新表行（查看页复用的正是这两条读面） |
| 16 份相位提示词零改动 + 三份 delta spec 的语料自洽（AC-9 后半：archi / change-chat 的 explore 与 change 臂表述单源指向） | 文档事实，无运行期投影 | 桌面端 `git diff` 审查：`crates/core/workflow/src/prompts/**` 零改动且全文零 `db_tables` / `db_query` 命中；spec 文本以 `desktop-agent-execution` / `desktop-agent-db-tools` 为单源（人工比对） |
| 编译闭合批的纯机械点位（`LoopTurn` 40+ 处 / `PumpPayload` / `EngineFacade` 七处 / `mount_test` 期望表 11 → 13 / `compose_test` 名字清单期望值） | 只补字段 / 只更期望值，只求可编译与 spec 口径同步；design 与 tasks 明令**不新增用例、不改断言语义** | `cargo clippy --workspace --all-targets` 通过（`git diff` 对这些文件显示纯新增行 / 期望值行）+ 各章节「↔ 编译闭合」「夹具随动」行只登记点位与计数 |
| **两处既有期望随动**（`crates/infra/agent/src/sdk/policy_test.rs` 三档等价表 11 → 13 名 / `src-tauri/src/commands/db/mod_test.rs` workspace 清单 8 → 11 行） | 性质是「行数期望随 `MODEL_ENTRIES` 11 → 14 行 / 挂载清单 11 → 13 项收敛」——**零新增用例、零断言语义变更**，故不属「追加用例」，也不属越界改动；二者已在「单元测试」各有独立 per-file 章节 | 桌面端 `git diff` 审查：两文件 diff 只含期望值（名集 / 计数向量）与用例名 / 说明文案行，生产源 `policy.rs` / `db/mod.rs` 零 diff；`cargo clippy --all-targets` + 全量测试回归由 test-execution 相位执行 |
| `src-tauri/src/commands/**` 的「零 diff」表述 | 该目录下 `db/mod_test.rs` 有登记的期望随动 → **不可整目录声明零 diff**（整目录口径会把必然随动误判为越界，C9 回溯根因） | 口径按文件收窄：生产命令面（`db/mod.rs` 等实现）零 diff，`db/mod_test.rs` 单列为期望随动（见其 per-file 章节）；`tasks.md` 自查项（`:216` / `:224` / `:257`）与本表同口径 |
| 机械门禁（`cargo clippy --all-targets` / `pnpm -C packages/desktop run check` 四段链 / 全量测试回归 / `knip` / 版本行相对递增） | 机械门禁步骤，由桌面端 static-check / test-execution 相位执行；在设计里复述脚本细节等于复制运行脚本 | 门禁执行结果 + 本设计各章节对覆盖率的支撑（断言全部落在真实契约面：op 逐格 / 线名 ratchet / 预算闸原子性 / 打码与比较面 / 两臂名字清单 / 铸造三态 / 双库单开纪律） |
