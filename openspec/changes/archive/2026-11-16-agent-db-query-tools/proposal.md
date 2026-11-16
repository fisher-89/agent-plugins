# 提案: agent-db-query-tools

> **变更**: agent-db-query-tools
> **日期**: 2026-11-15
> **状态**: draft
> **类型**: requirement
> **来源**: `openspec/changes/agent-db-query-tools/explore.md`（含实测：真跑 store crate 打开 dev 库副本 + 自造 5k / 50k 行库，specta 类型图真跑；分叉 A/B/C/D 与四个实测坑已收敛）
> **范围接缝**: 本变更只做**只读数据面的两只手**：① store 侧新增两个只读原语 `describe()`（表 / 字段说明）与 `query(spec)`（条件过滤 + 排序 + 投影 + 分页），② agent 侧新增两个挂载清单工具 `db_tables` / `db_query`（`ToolExec::Db`，`ToolTier::Read`），③ 会话可见性限定 **explore + change 两臂**。MUST NOT 做：SQL 文本解析（无递归下降解析器）、聚合（`count` / `group by`）、JOIN、数组下表下钻、二级索引下推、写面、MCP、DB 查看页 UI 改造、`StoreMetaRecord` 入注册表、会话级 DB 访问黑名单。
> **性质**: 纯增量读面。事件词汇零新增、IPC 零新增、前端零改动、16 份相位提示词零改动、`crates/core/**` 零改动（工具以**名字**进既有名字清单通道）。

---

## 问题

### 现象一 · agent 对桌面端数据零可见性

工作区数据是 native_db(redb) 二进制文件（`~/.dev-team/workspaces/*.redb`，生产库实测 1.82 GB），单进程独占 + 单写锁。agent 的现有手（`read` / `grep` / `glob` / `ls` / `bash`）对它**一律零命中**：`bash` 打不开二进制库，`grep` 只能匹配到乱码字节。于是任何「上次那个里程碑为什么失败」「这个 change 的历史 verdict 分布」「某会话消耗了多少 token」类问题，agent 只能反过来问用户。

### 现象二 · 唯一读面是给「人」的，不是给 agent 的

```
infra/store/src/envelope.rs
  MODEL_ENTRIES: 11 行（user 3 + workspace 8）
  MAX_SCAN_LIMIT = 500
  list_models() -> Vec<ModelInfo{ name, count }>        ← 说明文字零来源
  scan(model, offset, limit) -> RecordsPage              ← 无过滤 / 无排序 / 无投影
shell/src/commands/db/{db_models,db_records}             ← Tauri IPC 面，agent 无通道
```

三个硬缺口同时成立：**（a）** 取数原语只有「主键自然序 skip/take」，模型想按 `changeId` 取一个 change 的全部 phase 只能整表翻页；**（b）** 说明文字零来源（`ModelInfo` 只有 `name` / `count`），即便能取数，模型也不知道 `verdict` / `stale` / `event.kind` 是什么意思；**（c）** 注册表**漏了 3 张表**——`RunRecord` / `RunNodeRecord` / `RunEdgeRecord` 已注册进 workspace 库（`workspace_models()` 十二模型）但未进 `MODEL_ENTRIES`，`store/src/lib.rs` 连这三个类型都没导出，agent 要看 run 运行史无门。

### 现象三 · 「上限 1000 行」撞穿单结果上限

`tools::cap_result` 是 L1 单结果上限（`MAX_RESULT_BYTES = 50_000`）。explore 实测：一条 `session_event` 整行 JSON 约 1.8 KB，**50 行 = 89.8 KB（1.8 倍上限）、1000 行 ≈ 1.6 MB（32 倍上限）**。行数上限本身不构成护栏——预算闸（40 KB）才是唯一硬拦；缺了它，`cap_result` 会在 JSON 中间截断，模型拿到半截 JSON 而不是「拿多了」这个事实。

### 现象四 · 双库身份锚与会话句柄

数据分两个文件：`desktop-global.redb`（user 维度 3 表）+ `workspaces/<identity>.redb`（workspace 维度 12 表）。`WorkspaceStores.global` 今天是 `Store` **值类型**（非 `Arc`），redb 单进程单句柄——要把全局库交给会话必须让它可共享；workspace 半边 `for_root` 已返回 `Arc<Store>`，现成。change 会话的 cwd 是 worktree，但库身份锚恒 workspace root（注入句柄而非按 cwd 解析，天然正确）。

### 症结

一句话：**agent 缺的是「知道自己该查什么」+「一次把要的取完」两件事**——前者是 schema 文档，后者是声明式查询。本仓 14 张表字段面全定死、无 JOIN / 无子查询 / 无聚合，正是「受限 DSL + schema 校验」最容易做好、也最不容易腐化的场景。

---

## 提案

给 agent 两只只读的手，并把「说明从哪来」「查询怎么表达」「谁能用」三个分叉一次性收口：

```
今天                                             之后
────────────────────────────────────────────     ────────────────────────────────────────────
store 读面                                        store 读面（同一实例、同一只读纪律）
  list_models()  → {name, count}                   list_models() / scan()            逐字不变
  scan()         → 主键自然序分页                    describe()  → 表清单 / 单表字段说明（doc 派生）
  注册表 11 行（漏 run 三表）                        query(spec) → 条件 / 排序 / 投影 / 分页
                                                  注册表 14 行

agent 工具面                                       agent 工具面（挂载清单 +2 项，ToolTier::Read）
  explore 臂 7 名：六工具 + create_explore_note     explore 臂 9 名：… + db_tables + db_query
  change 臂：七工具 + 矩阵勾选                       change 臂：七工具 + 矩阵勾选 + db_tables + db_query
  archi / debug 臂                                   archi / debug 臂           零变化
                                                   执行体 ToolExec::Db（DbContext 双库句柄）
```

### 一 · 查询形态：结构化 JSON 外壳 + SQL 词表（不用解析器）

```json
{ "table": "phase",
  "scope": "workspace",
  "where": [ { "field": "changeId", "op": "=",    "value": "2026-11-14-x" },
             { "field": "verdict",  "op": "!=",   "value": null },
             { "field": "phase",    "op": "LIKE", "value": "dev-%" },
             { "field": "attempt",  "op": "IN",   "value": [1, 2] } ],
  "order":  { "field": "timestamp", "desc": true },
  "select": ["id", "phase", "attempt", "verdict"],
  "offset": 0, "limit": 20 }
```

- `op` 用 SQL 比较符命名（`=` `!=` `>` `>=` `<` `<=` `LIKE` `IN` `IS NULL` / `IS NOT NULL`），默认全 **AND**；`LIKE` 只支持 `%`（前缀 / 后缀 / 包含，无通配时退化为等值），不做 `_` 通配、不做 `ESCAPE` 子句。
- `field` 支持点号下钻（`event.kind`、`configSnapshot.model`），**不支持数组下标**（`event.blocks.0.name` 不认）。
- `select` 是投影（`key` 恒在、不占名额）；`order` 缺省 = 主键自然序。
- 字段名 = **落库线名（camelCase）**。这是硬约束：不应用 `specta_serde::Format` 就吐 snake_case，模型拿 `change_id` 查询会**恒零命中**——这种「静默零命中」比报错危险一个量级，因此错误面必须反过来做：未知字段一律 `Err` 并**列出候选线名**。

### 二 · 说明来源：代码 doc 即 schema 文档（零手写、零漂移）

14 个 Record 几乎全部 `derive(specta::Type)`，`///` 注释在 derive 时被搬进 `Field.docs` / `NamedDataType.docs`；`serde(rename_all = "camelCase")` 以运行期属性保留，经 `specta_serde::Format.map_types(&types)` 应用后即为落库线名。于是「表介绍 + 字段说明 + 类型 + 线名」全链机械派生：

```
<T as specta::Type>::definition(&mut Types::default())
  → DataType::Reference(Named(dt)) → Format.map_types(&types)
  → NamedDataType { docs = 表介绍, ty: Struct(NamedFields) }
       → 每字段 { 线名, docs = 字段说明, ty = 类型 }
```

配套 **ratchet 测试**（双向）：注册表每个模型的每个字段必须既有 doc、其线名集必须与各模型**真实 serde JSON 键集**逐一对应——未来新增字段不写说明即红，重命名即红。实测缺口只有 12 个字段（`PhaseRecord` ×7 / `ChecklistItemRecord` ×3 / `RunNodeRecord` ×2），补 12 条 `///` 后整份 schema 文档就齐了。

### 三 · 会话可见性：explore + change 两臂

| 臂 | 改动 | 理由 |
|----|------|------|
| explore | 名字清单 7 → **9**（+ `db_tables` / `db_query`） | 探索页就是「分析 / 诊断」场景，最贴题 |
| change | 名字清单 = 七工具 + 矩阵勾选，**再恒 push 两名**（不分 phase / 不分矩阵命中，与 ask 位恒 `Flow` 同构） | 执行 / 评审相位要能查历史 verdict 与运行史 |
| archi | 不动（零 DB 工具） | 架构会话的输入是模型与文档，不是数据行 |
| 其余（debug 等） | `tools = None` 不动（内建七件） | 不把只读数据面塞进调试默认面 |

工具描述**自足**（模型靠 `description` 学语法），16 份静态相位提示词**零改动**。

### 四 · 双档收口：行数 + 字节预算（先到者止）

- `limit` 缺省 **20**、上限 **1000**（越界显式 `Err`，不静默钳位）；`offset` 缺省 0。
- 工具内**字节预算 40 KB**（低于 L1 的 50 KB）：累加到该条会超预算即停，返回已收部分 + `truncated: true` + `truncatedBy: "bytes" | "limit"`，提示「用 select 收窄字段 / 加 where 条件 / 翻 offset」。
- 实测口径：`session_event` 默认 `limit=20` 约 36 KB（预算内），`limit=50` 会被预算截到约 22 行——**上限是名义值，预算是实际生效值**，这一事实必须在描述里讲明，否则模型会把「只回 22 行」误解为「库里只有 22 行」。

### 五 · 裁决表

| # | 决策 | 理由 | 被拒备选 |
|---|------|------|----------|
| D1 | 查询形态 = 结构化 JSON + SQL 词表（explore 分叉 A1 变体） | 无解析器即无解析器 bug 面；机制 = 静态词表 + 纯函数便于密集测试；白嫖 SQL 心智模型 | **A5 迷你 SQL**（手写递归下降 300~400 行 + 永不关闭的语法演进面）；**A2 表达式串**；**A3 只给索引便捷查询**（表达力等于没有）；**A4 让模型 bash 自造（redb 二进制 + 单写锁，做不到）** |
| D2 | 说明来源 = specta 运行时类型图派生（B2′） | doc 即代码，无第二份真相；已有的 `bindings.ts` JSDoc 就是产物证据 | **B1 手写 112 条静态表**（白花且必漂移）；**B3 只给字段名 + 类型**（说明质量差一档） |
| D3 | 两工具合并「列表 + 下钻」：`db_tables` 无参出清单、带 `table` 出字段 | 清单 1.4 KB（便宜，每轮可给）；全量字段 11 KB（不炸，但每轮白烧） | 拆成 `list_tables` + `describe_table` 两工具（业界 MCP 三件套形态；本仓表固定 14 张、下钻需求弱） |
| D4 | 可见性 = explore + change 两臂 | 探索是分析场景、change 是复盘场景；archi 读模型不读数据、debug 保持内建七件 | D2 explore + archi；D3 全臂覆盖（把只读数据面塞进 implement 相位是噪声） |
| D5 | `scope` 显式（`user` / `workspace`），缺省 `workspace` | 会话恒在某个 workspace 里跑；与查看页同口径 | 只给 workspace；只给 global |
| D6 | 注册表补齐 `run` / `run_node` / `run_edge`，`store_meta` 仍排除 | run 运行史是复盘主线；`StoreMetaRecord` 是库级格式标记，无查询价值（既有注释口径） | 只补 run 不补 node/edge（运行史断图） |
| D7 | `api_key` **输出打码 `"***"` 且不可作比较面**（`where` / `order` 引用即 `Err`；`select` 允许且值仍打码） | 只打码输出仍留前缀探测 oracle：`LIKE 'sk-a%'` 可逐字符重建明文——数据面泄密不需要模型看到值；投影不构成比较通道 | 只做输出打码（保留 oracle）；`select` 也拒（值与投影打码等价，多一条规则无收益）；整表禁查（该表其余字段无价值，白丢） |
| D8 | 不做二级索引下推 | 表都不大（单 workspace：change 十位 / phase 百位 / session_event 千~万级）；`where` 命中二级索引再下推是优化不是前提 | 一期即下推（多一条执行路径、两份语义要测） |
| D9 | `query` 独立读路径，**不复用** `MAX_SCAN_LIMIT = 500` | 语义不同：查看页页长上限管 IPC 载重，查询面上限是 agent 表达力（用户口径 1000），且自带过滤 / 投影 | 抬 `MAX_SCAN_LIMIT`（查看页单页载重跟着涨，IPC 面白挨） |
| D10 | `limit` 缺省 20（统一，不按表自适应） | 实测最坏表（`session_event` 1.8 KB/行）在 40 KB 预算下容 ~22 行；统一值可预期、可测 | 按表自适应（多一张 per-table 常量表，又是一份要维护的真相）；缺省 50（最坏表必被预算截断，模型先看到 truncated） |
| D11 | 说明文字截断 = 首个句读（`。` / `；` / 换行）→ 兜底 120 字 | `PhaseRecord.change_id` 的 doc 是整段长文，按「首行截」切出半句 | 整段输出（`AgentRunRecord` 的 doc 是整篇小论文，烧上下文）；硬截 120 字（同上半句问题） |
| D12 | store 侧 `describe` / `query` 不 derive `specta::Type` | 零 IPC / 零 bindings / 零前端改动；它们不出线，只被 infra 消费 | derive `Type` 并下发前端（本变更无用，白扩 IPC 面） |
| D13 | 打码与预算闸住在 **agent 执行体**，store 读面保持纯 | store 是数据层（查看页也在用），「记录能不能进模型上下文」是 agent 面政策 | 打码下沉 store（把 agent 政策写进数据层，第二个消费方无从选择） |

---

## 能力

### 新增能力

- **`desktop-agent-db-tools`** — 「agent 的 DB 查询工具两件套」：`db_tables`（表清单 / 单表字段说明）与 `db_query`（条件 / 排序 / 投影 / 分页）的工具契约、查询原语语义（op 全集 / LIKE / 类型比较 / 点号下钻）、说明来源（specta doc 派生 + ratchet）、双档收口（行数 + 40 KB 字节预算）、错误即自纠通道、只读档位与敏感字段（`api_key` 打码且不可作比较面）、两臂可见性与 per-turn 上下文铸造。store 侧新读面（`describe` / `query` / 注册表补齐）归本能力描述——`desktop-workspace-store` 的「记录信封 API」requirement（`list_models` / `scan` 只读、零写面、新模型零改动可浏览）逐字不变。

### 修改的能力

- **`desktop-agent-execution`** — 「会话级工具面注入与 explore 笔记工具」requirement：explore 臂名字清单 7 → 9、change 臂在矩阵之后恒 push 两 DB 工具名（不分 phase / 不分矩阵命中）、执行体装配缝增 `DbContext` per-turn 铸造（判据 = 名字清单含任一 DB 工具名，与 `create_explore_note` / archi 同一类判据）、tier 档位归属增两条 `ToolTier::Read`。其余段落（`core` 词汇封闭、ask 位、`create_explore_note` 契约、debug 臂 `tools = None`）逐字不变。
- **`desktop-archi-agent`** — 「archi 工具面注入与 (phase, role) 矩阵」requirement 内的两处 `source = "explore"` 臂表述：由「六工具收窄 + `create_explore_note` 逐字不变」改为「archi 面恒空 + 完整清单单源指向 desktop-agent-execution / desktop-agent-db-tools」，避免与 explore 臂 9 名的真值互相矛盾。矩阵、archi 臂、fail-closed 语义逐字不变。
- **`desktop-change-chat`** — 「change 环境边界（worktree 为 cwd）」requirement 的两处工具面表述：把写死的「工具面 SHALL 为既有 default 七工具」改为指向 `desktop-agent-execution` 名字清单单源（该句在 archi 工具面注入与本变更之后已失真——change 臂实际含矩阵勾选 + 恒在的 ask / DB 两件）。cwd 派生、双 root 不变量、存在性策略逐字不变。

### 明确不动（并已在 spec 中留痕）

- `desktop-workspace-store` — 记录信封 API 的既有 requirement 逐字不变（新读面由本能力自持契约，边界在「API 只读、零写面」上一致）。
- `desktop-db-inspector` — 查看页零改动；注册表补齐后 `run` / `run_node` / `run_edge` 自动出现在清单与分页扫描面，正是既有「模型自动注册红利与类型壁垒」requirement 预告的行为，无需 delta（命令层测试期望随注册表收敛，见「变更范围 · 测试文件」）。

---

## 变更范围

### 实现文件

以下均相对 `packages/desktop/src-tauri/`（除注明外）：

| 文件 | 改动 |
|------|------|
| `Cargo.toml`（workspace） | **零改动**：`specta = "=2.0.0-rc.26"` / `specta-serde = "=0.0.13"` 条目已在案（store 只需在 crate 侧引用） |
| `crates/infra/store/Cargo.toml` | + `specta-serde = { workspace = true }`（单 crate 构建下 serde 重命名才被保留——不指望壳层 feature 统一；`Cargo.lock` 随之多一行 depends 关系，版本零变动） |
| `crates/infra/store/src/model.rs` | 补 12 条字段 `///`（`PhaseRecord` ×7 / `ChecklistItemRecord` ×3 / `RunNodeRecord` ×2）；4 个 `#[serde(with = "event_key_serde")]` 字段（`ChecklistItemRecord.item_key` / `SessionEventRecord.event_key` / `RunNodeRecord.node_key` / `RunEdgeRecord.edge_key`）加 `#[specta(type = String)]`（否则 `Format.map_types` 直接 `Err`：`#[serde(with)] changes the wire type`）；模型版本号、`native_model` 头、字段面**零改动** |
| `crates/infra/store/src/envelope.rs` | 注册表补 `run` / `run_node` / `run_edge` 三行 + 三个 `key_of`（`run_id` / `node_key` / `edge_key`）；`ModelEntry` 增 `schema` / `walk` 两字段与 `entries()` / `entry()` / `dimension_of` 访问器；`op` 词表（`QUERY_OPS`）单点导出（供工具描述拼接，照 `mount.rs::vocabulary()` 先例）；`list_models` / `scan` / `MAX_SCAN_LIMIT = 500` 逐字不动 |
| `crates/infra/store/src/describe.rs`（新） | 表 / 字段说明的机械派生：`Types` + `specta_serde::Format` 取线名（camelCase）、按注册表过滤内嵌命名类型、类型渲染薄 formatter、doc 句读截断；`SessionEventRecord` 单独供给 3 条说明（该表不做 IPC 出线，不强制 derive） |
| `crates/infra/store/src/query.rs`（新） | 只读查询原语：`QuerySpec` / `QueryCond` / `QueryOrder` / `QueryPage` / `QueryError`、op 词表校验、点号下钻、`LIKE` / `IN` / `IS NULL` 语义、过滤 → 排序 → 投影 → 分页（`limit` 上限 1000，**不复用** `MAX_SCAN_LIMIT`；读事务单快照） |
| `crates/infra/store/src/store.rs` | `Store::describe()` / `describe_table(&str)` / `query(spec)` 薄包装；`WorkspaceStores.global: Store` → `Arc<Store>` + 新增 `global_shared() -> Arc<Store>`（`global()` 语义不变，`&Store` 由 Arc 解引用）；`for_root` 缓存与单开纪律零改动 |
| `crates/infra/store/src/lib.rs` | `mod describe;` / `mod query;` 声明；导出 `ModelDoc` / `FieldDoc` / `TableDoc` / `QuerySpec` / `QueryCond` / `QueryOrder` / `QueryPage` / `QueryError` / `DEFAULT_QUERY_LIMIT` / `MAX_QUERY_LIMIT` / `QUERY_OPS`（**不 derive `Type`** → 零 IPC / 零前端改动）；补导出 `RunRecord` / `RunNodeRecord` / `RunEdgeRecord`（现未导出，工具覆盖 run 史必须） |
| `crates/infra/agent/src/sdk/db_tool.rs`（新，含 `sdk/mod.rs` 一条 `mod` 声明） | `DbContext { global: Arc<Store>, workspace: Arc<Store> }` + `async fn execute(&self, name, &input) -> Result<String, String>`；scope 解析、40 KB 字节预算与截断提示、`api_key` 打码、错误文案装配；`MUST NOT` 在此实现第二份查询语义（原语在 store） |
| `crates/infra/agent/src/sdk/mount.rs` | + `DB_TABLES_TOOL` / `DB_QUERY_TOOL` 常量 + 两个定义（description 覆盖 op 词表 / scope / 预算 / 自读警示）+ `ToolExec::Db` + tier `Read`；`TOOL_NAMES` 七件**不动**；挂载清单 11 → 13 项 |
| `crates/infra/agent/src/sdk/tools.rs` | `input_path` 的两 DB 工具名显式 None 臂（照 archi 三工具先例，防未来入参改名被沙箱误改写）；七工具 `execute` 签名零改动 |
| `crates/infra/agent/src/sdk/loop.rs` | `ToolExec::Db` 分派臂（上下文缺席以 `is_error` 回灌，run 不中断）；`LoopTurn` 增 `db: Option<DbContext>`（与 `explore_note` / `archi` / `ask` 同列） |
| `crates/infra/agent/src/sdk/runner.rs` | `PumpPayload` 增双库句柄；`session_pump` per-turn 铸造（判据复用 `contains_tool`，与 `mint_archi_context` 并列） |
| `crates/infra/agent/src/lib.rs`（`EngineFacade`） | builder 缝 `with_db_stores(...)`（与 `with_explore_note_store` 同型），`compose_turn` 透传 |
| `crates/infra/agent/src/compose.rs` | explore 臂 + 两 DB 工具名；change 臂矩阵之后恒 push 两名；`compose_turn` 从 `stores.global_shared()` 与既有 `store: Arc<Store>` 组装 `DbContext` 原料 |
| `packages/desktop/package.json` | version `0.6.43` → `0.6.44`（**归档时执行**，见 AC-13） |

### 测试文件

| 文件 | 覆盖 |
|------|------|
| `crates/infra/store/src/describe_test.rs`（新） | `describe`：表清单 14 行（user 3 / workspace 11、`store_meta` 不出现）、内嵌命名类型不入清单、`table` 下钻字段线名/类型/doc、doc 句读截断、ratchet（每字段 doc 非空；线名集 ↔ 真实 serde JSON 键集双向一致）、`SessionEventRecord` 3 字段 |
| `crates/infra/store/src/query_test.rs`（新） | `query`：op 全集逐格、AND 组合、点号下钻（`event.kind`）与数组下标 `Err`、`LIKE` 四形 + `\%` `\_` 转义、`IN`、`IS NULL` / `IS NOT NULL`、类型不符 `Err` 携类型、排序（升/降 + `null` 末位 + 缺省主键序）、投影（`key` 恒在）、分页（`limit` 上限 1000 / 越界 `Err`）、`total` / `matched` / `returned` 口径、未知表/字段/op 的候选文案、只读（零写面） |
| `crates/infra/store/src/envelope_test.rs` | 注册表面：注册表 14 行、`dimension_of`、三新表 `key_of`（describe / query 行为面已分居两新 sidecar） |
| `crates/infra/store/src/model_test.rs` | 补 doc 后 decode/encode 逐字节不变（`///` 不进线格式）；`specta(type = String)` 字段与真实 serde 十六进制串键集一致 |
| `crates/infra/store/src/store_test.rs` | `global_shared()` 不二次 open（同一 `Arc` 指针 / 单开纪律），`global()` 语义不变 |
| `crates/infra/agent/src/sdk/db_tool_test.rs`（新） | 预算闸（到界即停 / `truncatedBy` 两因 / 不出现半截 JSON）、`api_key` 打码（含 `select` 投影）与 `where` / `order` 引用即 `Err`、错误文案（未知字段携候选线名）、`scope` 三态、`db_tables` 两种入参形态 |
| `crates/infra/agent/src/sdk/mount_test.rs` | 两挂载项在清单内（挂载清单 11 → 13 名）、tier = `Read`、定义 schema 形状（逐工具三档等价表见 `policy_test.rs`） |
| `crates/infra/agent/src/compose_test.rs` | explore 臂 9 名（含两 DB 工具）、change 臂含矩阵未命中行仍含两名、archi 臂 10 名不含、debug 臂 `tools = None`；非法名拒绝语义不变 |
| `crates/infra/agent/src/sdk/runner_test.rs` | per-turn 铸造判据：名单含 DB 工具 → `Some(DbContext)`；不含 → `None` |
| `crates/infra/agent/src/sdk/tools_test.rs` | `input_path` 对两 DB 工具名返回 `None` |
| `crates/infra/agent/src/sdk/loop_test.rs` | `ToolExec::Db` 分派两态（上下文在场走执行体 / 缺席 `is_error` 回灌）；`LoopTurn.db` 夹具随动 |
| `crates/infra/agent/src/lib_test.rs` | `EngineFacade::with_db_stores` builder 缝与 `runner_for` 透传 |
| `crates/infra/agent/src/sdk/policy_test.rs` | 逐工具三档等价表随挂载名数补齐（11 → 13 名，两 DB 工具归 `Read` 三档全放）；档位归属抽样与幂等面口径不变 |
| `src/commands/db/mod_test.rs` | 注册表补齐的期望随动：workspace 维度清单 8 → 11 行（`db_models` 计数向量 + 两库互不混列名集）与用例名同步；断言口径零改动 |

> 后两行（`sdk/policy_test.rs` / `src/commands/db/mod_test.rs`）是**注册表与挂载清单补齐的期望随动**，不新增用例、不改断言口径——它们是「`MODEL_ENTRIES` 11 → 14 行」与「挂载清单 11 → 13 项」在两处既有测试里的必然投影，随本变更同批收敛，不是越界改动。前 12 行 = 新建 3 + 既有 sidecar 9（与 design / test-design 同一份清单）。

### 不要修改

- `packages/desktop/src/**`（前端）、`src/bindings/` 与 `src/commands/**` 的**生产命令面** — **零 diff**：新读面不 derive `Type`、无新 IPC 命令，查看页 UI 无需改动。**除外**（两处期望随动，重述于「测试文件」）：① `src/commands/db/mod_test.rs` 的注册表行数期望（workspace 维度清单 8 → 11 行、`db_models` 计数向量与用例名同步）；② `crates/infra/agent/src/sdk/policy_test.rs` 的 tier 表期望（逐工具三档等价表 11 → 13 名，两 DB 工具归 `Read`）。两者分别是「`MODEL_ENTRIES` 11 → 14 行」与「挂载清单 11 → 13 项」在既有测试里的必然投影（注册表补齐正是既有「新模型零改动可浏览」红利），随本变更同批收敛、断言口径零改动，不是越界改动。
- `crates/core/**` — **零 diff**：`SessionInjections` 词汇不动（工具以名字进清单），`AgentEvent` 词汇、`EngineConfig` 字段面、`AgentStartError` 变体集均不动。
- `crates/infra/agent/src/sdk/policy.rs` / `sandbox.rs` / `protect/**` — 零 diff（`Read` 档三档放行天然覆盖；`input_path` 显式臂在 `tools.rs`；挂载名数补齐的期望随动在 `policy_test.rs`，见「测试文件」）。
- `crates/infra/agent/src/sdk/{bash,explore_note,archi_tool,context,compact,resume,log}.rs` 与七工具执行体 — 语义零改动。
- `crates/core/workflow/src/prompts/**` 16 份相位提示词 — 零改动（工具描述自足）。
- store 写面 / `MODEL_ENTRIES` 之外的模型版本头 / `StoreMetaRecord` 注册 — 零改动；`MAX_SCAN_LIMIT = 500` 与查看页分页口径 — 零改动。
- `openspec/specs/desktop-workspace-store/**`、`openspec/specs/desktop-db-inspector/**` 的既有 requirement 文本 — 零改动。
- `.dev-team/architecture/api/**` 契约面 — 本期不新增契约资产（`App.Backend.Infra.Store` 元素保持既留白态；如后续要把该公共读面纳入契约面，另立变更）。

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | `db_tables` 发现面 | 无参返回 14 行表清单（`dimension` / `table` / `count` / `summary`），user 3 + workspace 11，`store_meta` 不出现；带 `table` 返回该表全字段（`name` = 落库线名 camelCase、`type` 渲染文本、`doc`）；`scope` 收窄生效；两种形态均不吐另一形态的字段 |
| AC-2 | 说明来源机械派生 + ratchet | 说明全部来自 `///` doc 与 specta 类型图（`Format` 已应用 → 线名为 camelCase）；注册表每模型每字段 doc 非空、线名集与真实 serde JSON 键集双向一致；故意删一条 doc 或改一个 `rename` → 测试红；`SessionEventRecord` 3 字段同形态可读 |
| AC-3 | 线名可查（camelCase 唯一真值） | `where: [{field:"changeId", op:"=", ...}]` 正常命中；`changeId` 之外写 `change_id` → `Err` 且文案列出候选线名（**不静默零命中**） |
| AC-4 | 查询原语语义 | op 全集（`=` `!=` `>` `>=` `<` `<=` `LIKE` `IN` `IS NULL` / `IS NOT NULL`）逐格断言；AND 组合；`event.kind` 点号下钻命中（数组下标不认）；`LIKE` 前缀 / 后缀 / 包含 / 无通配退化等值 + `\%` `\_` 字面转义；`null` 的比较语义（`=` false / `!=` true / 大小比较不匹配）；跨类型比较 `Err` 携字段类型 |
| AC-5 | 排序 / 投影 / 分页 | `order` 升降序正确、缺省主键自然序；`select` 投影后 `key` 恒在、未选字段不出现；`offset` / `limit` 生效，`limit` 上限 1000、越界 `Err`（不静默钳位）；`total`（全量）/ `matched`（过滤后）/ `returned`（本页）三者口径可区分 |
| AC-6 | 双档收口 | 40 KB 预算先到者止：返回体 `truncated: true` + `truncatedBy: "bytes"` 且**无半截 JSON**；行数到 `limit` → `truncatedBy: "limit"`；两因文案含「用 select 收窄 / 加 where / 翻 offset」；`session_event` 缺省 `limit=20` 落在预算内、`limit=50` 被预算截断且如实回报 `returned < matched`；结果体恒低于 L1 `MAX_RESULT_BYTES`（50 KB） |
| AC-7 | 敏感面 | `agent_provider` 记录输出中 `api_key` 恒为 `"***"`（含 `select` 显式投影）；`where` / `order` 引用 `api_key` → `Err`（前缀与等值探测通道都不成立） |
| AC-8 | 只读与零介入 | store 侧新增面全为读（无写 API、无写事务）；两工具 tier = `Read` → 三档（default / acceptEdits / bypassPermissions）全放行；`sandbox_rewrite` 对两工具原样返回；写保护闸零介入；`crates/core/**`、`policy.rs`、`protect/**` git diff 为空 |
| AC-9 | 会话可见性两臂 | explore 臂名字清单 9 名（六工具 + `create_explore_note` + `db_tables` + `db_query`）；change 臂不分 phase / 不分矩阵命中恒含两名（矩阵未命中行、畸形 `source_ref` 同样含）；archi 臂 10 名（七内建 + 三 archi）与 debug 臂 `tools = None` 均**不含**两名；`RunStarted.tools` 与实际下发面逐名一致；spec 语料自洽（`desktop-archi-agent` / `desktop-change-chat` 的 explore / change 臂表述与本变更一处对一处，无第二份清单） |
| AC-10 | 双库 scope 与身份锚 | `scope="user"` 走全局库、`workspace` 走会话 workspace 库、缺省 workspace；两库清单互不混列；`WorkspaceStores::global_shared()` 交出 `Arc` 且不二次 open（进程内单开纪律不破）；change 会话 cwd 为 worktree 时库身份锚仍为主仓 root |
| AC-11 | 成员校验与幻觉调用 | 未下发 DB 工具的会话（archi / debug）调用 `db_query` → `SystemNotice{subtype:"tool_unavailable"}` + `is_error` ToolResult 回灌，run 不中断、不触执行体；`DbContext` 缺席时执行体以 `is_error` 回灌（生产不可达） |
| AC-12 | 事件 / IPC / 前端零改动 | `AgentEvent` 六变体与块词汇逐字不变、零新 subtype 需求；`src/bindings/` git diff 为空；前端 `packages/desktop/src/**` git diff 为空 |
| AC-13 | 版本交付 | 归档时 `packages/desktop/package.json` version `0.6.43` → `0.6.44`（`src-tauri/tauri.conf.json` 经 `"version": "../package.json"` 跟随；`src-tauri/Cargo.toml` crate 版本行不跟随） |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| **上下文污染 / 自读反馈环**：查 `session` / `session_event` 能读到本会话自己的转录（含刚写入的工具结果），一次 `limit=20` 就灌回 ~36 KB | 上下文被自家转录挤占，长会话可能进 L2 剪裁 | 高（知情接受） | 工具描述明示「这两张表是本会话自身转录，慎查」；40 KB 预算闸是唯一硬拦；不做会话级屏蔽（会造第二套规则，且诊断场景确有需要） |
| **全表 decode 延迟**：无 `order` + 无 `where` 可早停；带 `order` 或任意字段过滤必须全表 decode（50k 行实测 93 ms，70 万行外推 1~1.7 s） | agent 深翻页时单次工具调用变慢 | 中 | 缺省 `limit` 小 + 描述提示「大表用 where / select」；二期按「`where` 等值命中二级索引 → `scan().secondary().range()`」下推 |
| **名义上限与实际行数落差**：`limit` 上限 1000 但预算会把大表截到 ~22 行 | 模型误判「库里就这么多」 | 中 | 返回体恒报 `total` / `matched` / `returned` 三元 + `truncatedBy`；描述显式声明「预算先到者止」 |
| **`specta_serde::Format` 派生坑**（已逐个实测）：4 个 `serde(with)` 字段直接 `Err`、`NamedReference` 无 `Display`、内嵌 struct（`AgentModelTiers` / `SessionConfigSnapshot` / `ChangeActivePhase`）也进类型图、`SessionEventRecord` 无 `Type` derive | `describe` 编译不过或输出错线名 | 高（已定位） | 4 字段加 `#[specta(type = String)]`；类型渲染自实现（`types.get(&r).name`）；按注册表名单过滤内嵌类型；`SessionEventRecord` 单独供给说明并由 ratchet 覆盖 |
| **说明文字质量**：`///` 是给人看的散文，部分 doc 是整段/整篇（`PhaseRecord.change_id`、`AgentRunRecord`） | 说明字段臃肿、挤占上下文 | 中 | 首句读截断（`。` / `；` / 换行）→ 120 字兜底；ratchet 只要求「有 doc」不要求长度（长度是作者的判断） |
| **敏感字段只打码不过滤 = 前缀探测 oracle** | 模型可经 `LIKE 'sk-a%'` 逐字符重建明文 | 中（若漏做） | D7：输出打码 **且** 堵死比较通道（`where` / `order` 引用即 `Err`），两半都在 spec 与测试里 |
| **注册表补齐改变查看页可见面**（`run` / `run_node` / `run_edge` 三张新表出现，`key` 为十六进制打包串） | 查看页清单变长、key 人读性略降 | 低（预期内） | 属既有「模型自动注册红利」预告行为，非缺陷；`key` 十六进制串与 `checklist_item` / `session_event` 既有投影同形；`src/commands/db/mod_test.rs` 的清单期望 8 → 11 行同批随动（已登记在「测试文件」），不属越界改动 |
| **双口径混淆**：查询面 `limit` 上限 1000 vs 查看页 `MAX_SCAN_LIMIT` 500 | 后续维护者误以为该抬常量或共用读路径 | 低 | spec 明写「`query` 独立读路径、MUST NOT 复用 `MAX_SCAN_LIMIT`」；`scan` 与查看页口径零改动 |
| **`WorkspaceStores.global` 由值类型改 `Arc`** | 误用二次 open（redb 单写锁冲突） | 低 | 只经 `global_shared()` 交出 clone；`Store::open_global` 仍单点；单开纪律测试覆盖 |
| **第二份查询语义**（工具侧再实现一遍过滤/排序） | 两套语义必然漂移 | 中（若实现走偏） | 原语单源在 store；`db_tool.rs` 只做 scope 解析 / 打码 / 预算 / 文案；op 词表由 store 单点导出供 description 拼接（照 `vocabulary()` 先例） |

---

## 过程

### 决策

见「提案 · 五 · 裁决表」（D1~D13）。补充两条本变更的**边界性决策**：

| 问题 | 决策 | 理由 |
|------|------|------|
| 新读面契约住哪个能力 | 全部住新能力 `desktop-agent-db-tools`；`desktop-workspace-store` 既有 requirement 零改动 | 避免「同一份查询语义拆进两份 spec」；边界在「只读 / 零写面」上一致，不产生矛盾 |
| 是否给 DB 工具做会话级黑名单 / 表级白名单 | 不做 | 一旦有白名单就必须解释「为什么这张表不能查」，规则会持续膨胀；预算闸 + 描述警示已足够（风险表首行知情接受） |

### 待决问题（归 design）

- `describe` / `query` 是否独立模块文件（`envelope.rs` 已 500+ 行）；类型渲染 formatter 的落点。
- `QuerySpec` 的 JSON 形状定稿（`where` / `order` / `select` 键名与 serde `camelCase`）、`Err` 是字符串还是结构化（含 `candidates` 字段）。
- 字节预算的计数口径（逐条累加序列化长度 vs 整体序列化后切）与「到界即停」的整条原子性实现。
- `LIKE` 转义（`\%` / `\_`）实现与「无 `%` 退化等值」的单点判定。
- 嵌套路径（`event.*`）的类型校验降级策略：顶层按 schema 校验类型，嵌套路径类型不可知时如何报「混合类型」。
- 4 个 `event_key_serde` 字段的 `#[specta(type = String)]` 与 ratchet「线名集 ↔ 真实 serde 键集」断言的共存方式。
- change 臂两名工具的 push 位置与写法（与 ask 位恒 `Flow` 同构的形态）。
- `DbContext` 的铸造点与装配缝命名（`with_db_stores` 收两个 `Arc<Store>` 还是收一个结构体）。
- `db_tables` 的 `table` 与 `scope` 组合解析次序（表名是否跨库唯一、跨库同名时的报错形态）。
- 表介绍（struct doc）与字段说明的输出上限是否需要 per-table 例外（`AgentRunRecord` 类长文）。

---
