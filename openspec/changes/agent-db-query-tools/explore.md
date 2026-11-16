# 向 agent 提供 DB 查询工具（两件套）

workflow_type: requirement

## 一句话

agent（SDK 引擎里的模型）今天对桌面端数据**零可见性**：库是 native_db(redb) 二进制文件，bash 打不开、grep 读不到。要给它两只只读的手：`列出表 + 表/字段说明`、`按条件查询（≤1000 行）`。

## 现状地图（已核实的底座）

```
infra/store  ── native_db 0.8.2 (redb 2.6) + native_model 0.4.20（bincode 落库）
   │  model.rs   14 个 Record（3 user 维度 + 12 workspace 维度，native_model id 1/2/4..16）
   │  store.rs   Store{ db, dimension }；WorkspaceStores{ global: Store, workspaces_dir, cache: Mutex<HashMap<root, Arc<Store>>> }
   └  envelope.rs 记录信封 API（唯一通用读面）：
        MODEL_ENTRIES: &[ModelEntry{ name, dimension, count, scan, key_of }]  ← 11 行
        list_models() -> Vec<ModelInfo{ name, count }>
        scan(model, offset, limit) -> RecordsPage{ records: [{key,value}], total }   （MAX_SCAN_LIMIT = 500）
        值经 serde_json 出线（native_db 类型不越信封）

shell ── commands/db/mod.rs：db_models / db_records（只读薄包装，State<WorkspaceStores>）
        前端 views/db/*：DB 查看页（模型清单 + 自动表格 + JSON 抽屉）

infra/agent ── sdk/mount.rs：静态挂载清单（10 项 = 七内建 + create_explore_note + archi_module/archi_search）
              mount 项 = { definition, tier(Read/Write/Execute/Protocol), exec(ToolExec) }
              sdk/tools.rs::execute  = 七内建纯函数分发（root, name, input）
              sdk/loop.rs 守卫链 = 成员校验 → policy 档位 → 路径沙箱 → 写入保护闸 → ToolExec 分派
              sdk/{archi_tool,explore_note}.rs = 专用工具执行体范例（context + async execute(&input) -> Result<String,String>）
              sdk/runner.rs   PumpPayload{ explore_store: Option<Arc<Store>>, workspace_root, … } → per-turn 铸造 context
              compose.rs      compose_turn(stores: &WorkspaceStores, store: Arc<Store>, workspace_root: &Path, …)
                              assemble_injections()：explore / archi / change / 其余(debug=tools None) 四臂给「工具名字清单」
```

agent 侧的注入矩阵（今天）：

| 来源 | 工具面 |
|------|--------|
| explore | read, grep, glob, ls, edit, bash, create_explore_note |
| archi | 七内建 + archi_module + archi_search |
| change (per phase/role) | 七内建 + 矩阵勾选的 archi 工具（+ finish_step / ask） |
| 其余（debug 等） | `tools = None` → 内建七件 |

## 两条全链（新工具要走的路）

工具调用链（执行面）：

```
model tool_call ──▶ loop.rs
   ├ 成员校验  mounts（本轮下发面）
   ├ policy::allows(mode, tier)        ← 新工具 = ToolTier::Read
   ├ sandbox_rewrite(cwd, name, input) ← tools::input_path：非七内建返回 None（原样放行）
   ├ protect.evaluate(name, …)         ← 写保护表 `_ => None`，只读工具零介入
   └ match mount.exec { Pure | ExploreNote | Archi | FinishStep | Ask }
        ⇒ 需新增 ToolExec::Db ⇒ turn.db_context.execute(name, &input).await
   结果过 tools::cap_result（L1 = 50_000 字节，超限截断留痕）
```

装配链（上下文与下发面）：

```
compose_turn(stores, store: Arc<Store>, workspace_root)
   ├ assemble_injections(provenance)  → 名字清单（新增 db_tables / db_query 落在哪些臂）
   └ SdkRunner builder 缝（with_explore_note_store 同型）→ PumpPayload
        ⇒ per-turn mint：名字清单含 db 工具 ⇒ DbContext{ global: Arc<Store>, workspace: Arc<Store> }
          （判据单点 = runner.rs::contains_tool，与 archi 的 mint_archi_context 并列）
```

## 需求拆解：两工具的契约草案

### 工具 1 · `db_tables`（发现）

入参：`scope?: "user" | "workspace"`（缺省两库全列）。
出参（JSON）：

```json
[{ "dimension": "workspace", "table": "change", "count": 12,
   "summary": "change 流程状态建档行（换锚后主键 = change id）",
   "fields": [
     { "name": "id", "type": "string", "key": "primary", "note": "change id（身份锚）" },
     { "name": "status", "type": "ChangeStatus", "note": "active|completed|archived" },
     { "name": "worktree", "type": "string | null", "note": "执行 worktree 路径（legacy 为 null）" }
   ] }]
```

### 工具 2 · `db_query`（取数）

入参草案：

```json
{ "table": "phase",
  "scope": "workspace",              // 缺省 workspace
  "where": [ { "field": "change_id", "op": "eq", "value": "2026-11-14-x" },
             { "field": "verdict",  "op": "ne", "value": null } ],   // AND 组合
  "order": { "field": "timestamp", "desc": true },   // 缺省 = 主键自然序
  "select": ["id", "phase", "attempt", "verdict"],   // 字段投影（可选）
  "offset": 0, "limit": 50 }                          // limit 上限 1000
```

出参：`{ table, matched, returned, truncated, records: [{key, value}] }`。

## 调研 · 别人家怎么做（2026-xx 核实）

### ① 主流形态：MCP sqlite 家族 = 三件套，不是两件套

`mcp-server-sqlite`（Anthropic 官方参考实现，被抄得最多）：

```
list_tables          (无参)            → 表名数组
describe_table       (table_name)      → 列定义（名字 + 类型）
read_query           (SELECT SQL)      → 结果对象数组
```

`mcp-sqlalchemy` / Apify `database-mcp-server` 在此基础上加护栏：
- **默认最大行数**（mcp-sqlalchemy 25 行、Apify `limit` 默认 **1000** ← 和我们的口径撞上了）；
- **查询超时**（30s）；
- **只读模式开关**（read-only mode 挡住所有写）；
- 表名列名先对 schema 校验（防幻觉列 + 防注入）。

→ 对我们的启示：用户要的两件套其实是把 `list_tables` + `describe_table` 合并成一只（「列出表名、介绍、字段说明」）。**合并后建议留一个可选 `table` 参数做下钻**：无参 = 表清单 + 一句话介绍（14 行 ≈ 1KB，便宜）；带 `table` = 该表全部字段说明（`PhaseRecord` 15 字段 / `RunNodeRecord` 16 字段）。全量一次性吐出约 7KB（112 字段），不炸但也没必要烧。

### ② 「结构化条件 vs SQL」业界在打一场明确的仗，且 SQL 正在赢

Datadog 在 MCP Night 上的实测结论（他们拿 SQL 重写了 MCP 接口）：同一调查任务，**自由形式的专有 filter 语法**那一侧：猜语法 → 迭代 → 多次调用；**SQL** 那一侧：一条语句直接拿到结果。他们的论据：
- agent 用自由形式工具时会退化成「bash 式」——`head`/`tail`/`jq` 逐段消费，每一步都吃上下文，还要自己管中间状态；
- 声明式查询把「想拿什么」一次表达完，聚合（`GROUP BY`/`count`）在数据侧做完，**省的是上下文而不是 CPU**；
- Supabase 独立得出同一结论。

学术侧的 ReAct-SQL 则给了另一条腿：不是让模型裸写 SQL，而是给**15 个受限关系操作**的 typed DSL —— 每次动作都被 schema 校验、编译执行，模型只走「think → act → observe」。核心动机正是「裸 SQL 难约束」。

→ 对我们的启示：两边其实说的是同一件事——**要有声明式、可校验的查询面**。分歧只在「表面对模型是不是熟悉的语言」。

### ③ native_db 0.8.2 的真实能力边界（读了 crate 源码）

```
RTransaction: get / scan / len        ← 全部能力，就这三个
scan().primary::<T>()  → all / range / start_with
scan().secondary::<T>(KEY)→ all / range / start_with
get().primary(key) / get().secondary(KEY, val)
```

**没有** query 语言、没有 filter 谓词、没有 AND/OR 组合、没有排序、没有聚合。也就是说：
- 只有「主键 / 二级索引的范围与前缀」能下推到引擎；
- 其余条件一律得**在内存里对解码后的记录做**（`scan().all()` → 解码 → 过滤）。

本仓索引清单（14 表里只有 7 个二级索引）：

| 索引 | 字段 | 可下推的条件 |
|------|------|--------------|
| primary | 9 张表是整型 `id`/主键 | `id` 范围 |
| primary | `change.id` / `run.run_id` / `workspace.root` / `session.id` | 字符串前缀（`start_with`）= `LIKE 'x%'` |
| primary | `session_event.event_key` / `checklist.item_key` / `run_node.node_key` / `run_edge.edge_key`（hash64 打包 u128） | 仅精确 |
| secondary | `session_event.session_id` | 等值 / 范围 ✅（转录按会话读 = 现成热路径） |
| secondary | `phase.change_id` / `step.change_id` / `run.change_id` | 等值 / 范围 ✅ |
| secondary | `checklist.phase_id` / `run_node.run_id` / `run_edge.run_id` / `run_edge.from_node_key` | 等值 / 范围 ✅ |

→ 结论：一个**聪明的查询实现**可以在 `where` 命中二级索引时走 `scan().secondary().range(v..=v)`，其余落全表过滤。但这是优化，不是前提——表都不大（单 workspace 量级：change 十位数、phase 百位数、session_event 数千~万级）。

### ④ 与我们处境的关键差异（决定不能直接抄）

- sqlite MCP 那一套底下是**真 SQL 引擎**，`read_query` 白嫖了 where / order / group / limit 全套语义；我们底下是 KV 表 + serde JSON，SQL 得自己写解析器**和**执行器。
- 但我们的表是 **14 张定死的、字段面已知的**——恰恰是 ReAct-SQL 「受限 DSL + schema 校验」最容易做好的场景（不需要 JOIN、不需要子查询、不需要聚合）。

## 分叉点 A · 「按条件查询」怎么表达（最关键）

| 方案 | 形态 | 代价 | 风险 |
|------|------|------|------|
| A1 结构化条件数组 | `where: [{field, op, value}]`，AND | 无解析器，纯函数可密集测试；校验错误能指名候选字段 | 表达力有界（无 OR / 聚合）；模型要现学一套 JSON 形状 |
| A2 迷你表达式串 | `status = "running" AND createdAt > 1700…` | 要词法 + 语法 + 位置化错误文案 | 解析器 bug 面；仍要自己的执行器 |
| A3 只给主键/二级索引便捷查询 | `by: {sessionId: …}` | 最省 | 表达力太窄，等于没有「条件」 |
| A4 让模型用 bash + 自造脚本读库 | — | 零开发 | redb 是二进制 + 单写锁，实际做不到 |
| **A5 SQL 子集** | `SELECT a,b FROM phase WHERE change_id = 'x' AND verdict != null ORDER BY timestamp DESC LIMIT 20` | 手写递归下降解析器 + 执行器（单表 / AND / 6 个比较符 / LIKE 前缀 / IN / IS NULL / ORDER BY / LIMIT），估 300~400 行 + 测试 | 解析器自身 bug 面；但**模型对 SQL 的先天熟悉度最高**（Datadog 实测口径），且错误文案能做成「第 N 列：未知字段 status2（可用：id,name,status）」 |

倾向：**A1 换 SQL 词表**（结构化 JSON 外壳，`op` 用 SQL 比较符命名，`eq→=`…；`startsWith → LIKE 'x%'` 语义；字段路径支持点号下钻 `event.kind`）。理由：
- 本仓一贯「机制 = 静态注册表 + 纯函数」，A1 的校验面天然可密集测试（无条件反射解析）；
- 白嫖 SQL 的心智模型（`=`, `!=`, `>`, `LIKE`, `IN`, `IS NULL`, `ORDER BY`, `LIMIT`），但不背解析器；
- 校验错误可以做到 A5 同样好：未知字段 → 列候选（`db_tables` 的效果），未知 op → 列全套 op。

A5 的价值命题是「一句话表达复杂查询 + 模型零学习成本」，代价是解析器 + 一个永不关闭的语法演进面。若选 A5，建议先做只读子集并把语法写进工具描述（模型靠描述学语法，不靠猜）。

实现口径（A1/A5 共用）：全表 scan（`r.scan().primary::<T>()`）→ 解码 JSON → 条件过滤 → 可选排序 → 切页。**可选优化**：`where` 里存在等值条件命中二级索引（`session_id` / `change_id` / `run_id` / `phase_id`）时改走 `scan().secondary().range(v..=v)` 下推，其余全表。表都不大，先不做也行。

## 分叉点 B · 表说明 / 字段说明从哪来（**已找到近零成本答案**）

现状 `ModelInfo` 只有 `{name, count}`，**没有任何说明文字**。但 14 个 Record 全都 `derive(specta::Type)`，而 specta 的运行时类型图**带 doc 注释**：

```rust
// specta 2.0.0-rc.26
pub struct Field { pub docs: Cow<'static, str>, pub ty: Option<DataType>, … }
pub struct NamedDataType { pub name: Cow<'static, str>, pub docs: Cow<'static, str>, pub ty: DataType, … }
pub struct NamedFields { pub fields: Vec<(Cow<'static, str>, Field)> }
```

`///` 注释在 derive 时被搬进 `docs` 字段（这也是 `bindings.ts` 里每个字段都带 JSDoc 的原因——已经生成好的产物就是证据）。serde 的 `rename_all = "camelCase"` 则以**运行期属性**形式保留，经 `specta_serde::Format.map_types(&types)` 应用后即为**落库线名**。

于是「表介绍 + 字段说明」全链变成机械派生，零手写、零漂移：

```
<T as specta::Type>::definition(&mut Types::default())
   → DataType::Reference(Named(dt))
   → Format.map_types(&types)        ← 应用 serde 重命名（camelCase 线名）+ 展平
   → NamedDataType { docs = 表介绍, ty: Struct(NamedFields) }
        → 每字段 { 线名, docs = 字段说明, ty = 类型 }
```

量化成本：**112 个字段里只有 12 个没有 `///`**（`PhaseRecord.phase/attempt/skipped/stale/backtrack_reason/evaluator_session_id/decision_session_id`、`ChecklistItemRecord.item/pass/evidence`、`RunNodeRecord.phase/attempt`）——补 12 条注释，整份 schema 文档就齐了。再加一条 ratchet 测试（「每个字段必须有 doc，否则红」），未来新增字段**被迫**写说明，文档永不腐化。

| 方案 | 说明来源 | 漂移防线 | 结论 |
|------|----------|----------|------|
| B1 手写静态注册表 | `&'static [ModelDoc]` 全手写 112 条 note | 双向一致性测试（config.schema.json 先例） | 白花 112 条 |
| **B2′ specta 派生（本仓可行）** | struct/field doc 注释即说明 | doc 即代码，无第二份 | ✅ 推荐：补 12 条注释即可 |
| B3 只给字段名 + 类型 | specta 派生，无文字 | 无 | 说明质量差一档 |

实现注意：
- store 需显式 `specta-serde` 依赖（`specta-serde` 会开启 `specta-macros/serde`，重命名才会被保留——**单 crate 构建也要成立**，不能指望壳层的 feature 统一）；
- 表介绍取注释**首行**并按 ~120 字截断（`AgentRunRecord` 的注释是整篇小论文）；
- 类型渲染用一层薄 formatter（`DataType::Primitive/Reference/List/Map/Nullable` → `string | null` / `ChangeStatus` / `number`），约 60 行。

已探明的坑：**注册表少 4 张表**——`RunRecord` / `RunNodeRecord` / `RunEdgeRecord` / `StoreMetaRecord` 已注册进库（`workspace_models` 12 个）但没进 `MODEL_ENTRIES`（11 行，注释写「StoreMetaRecord 为内部治理记录，不入注册表」）。agent 要查 run 运行史就得补，顺便也补上 DB 查看页的缺口。

## 分叉点 C · 双库 scope

数据分两个文件：`desktop-global.redb`（user 维度 3 表）+ `workspaces/<identity>.redb`（workspace 维度 12 表）。

- 取哪个库：`scope` 入参（与查看页同口径）；缺省 workspace（会话就是在某个 workspace 里跑）。
- 关键实现约束：**redb 单句柄单进程**，同一个库文件不能二次 open。`WorkspaceStores.global` 现在是 `Store` 值类型（非 Arc），要把全局库交给会话必须让它可共享（`global: Arc<Store>` + `global_shared()`；`for_root` 已返回 `Arc<Store>`，workspace 半边现成）。compose_turn 手上正好两样都有（`stores` 与 `store: Arc<Store>`），不用改命令层。
- worktree 语义：change 会话 cwd 是 worktree，但库身份锚恒 workspace root —— 注入句柄（而非按 cwd 解析）天然正确。

## 分叉点 D · 哪些会话能拿到这两只工具

| 选项 | 覆盖 | 说明 |
|------|------|------|
| D1 仅 explore | explore 臂 +2 名字 | 探索页是「分析/诊断」场景，最贴题 |
| D2 explore + archi | 两臂 | 架构会话也能看数据 |
| D3 全部（含 change 各相位 + debug） | 四臂，debug 臂要从 `tools=None` 改成显式七件+2 | 覆盖面最大，但把只读数据面塞进 implement 相位是噪声 |

**✅ 已定：explore + change 两臂（不含 archi / debug）**

| 臂 | 改动 |
|----|------|
| explore | `EXPLORE_TOOLS` 七项常量 → 九项（+`db_tables`、`db_query`） |
| change | `names = builtin 七件 + 矩阵 archi 工具` 后再 push 两只（**不分 phase / 不分矩阵命中**，与 `ask` 位恒 Flow 同构） |
| archi | 不动 |
| 其余（debug 等） | `tools = None` 不动（内建七件） |

副作用可控：change 臂的 DB 工具不看 `source_ref` 解析结果，所以畸形 `source_ref` 的既有 fail-closed 语义零变化；16 份静态相位提示词**零改动**（工具描述自足，模型靠 `description` 学用法）。

## 硬冲突 · 1000 行 vs 50KB

L1 单结果上限 50_000 字节（`tools::cap_result`）。一条 `session_event` 是好几个 KB —— **1000 条 × 整行 JSON 必然撞穿 L1**，`cap_result` 会在 JSON 中间截断（模型拿到半截 JSON）。

可行的收口（建议同时上）：
1. `limit` 缺省小（50），上限 1000（用户口径的「上限」）；
2. 工具自带**字节预算**（~40KB）先到者止，返回 `returned < matched` + `truncated: true` + 提示「用 select 收窄字段 / 加条件」；
3. `select` 字段投影（大表 event 载荷只取 `event.kind` 之类）。

## 安全与副作用

- `AgentProviderRecord.api_key` 是**明文**字段：一旦被查询出来，就会进 transcript（`session_event` 落库）+ UI 渲染 + 下一轮请求上下文。需要字段级脱敏（`api_key` → `"***"`），或者干脆把这张表标成不可查字段。
- 自读反馈环：查 `session` / `session_event` 能读到**本会话自己的转录**（包括刚写入的工具结果）——不是漏洞但会灌爆上下文，说明文字里要警示。
- 只读保证：store 侧只加读 API；loop 侧 `ToolTier::Read`（Default 档也放行）；`input_path` 显式 None 臂（照 archi 先例，防未来入参改名被沙箱误改写）。

## 实测（真跑，非推演）

方法：在 `/tmp` 建临时 crate 依赖本仓 `store` crate，用 `Store::open_workspace` 打开**真实 dev 库副本** + 自造 5k / 50k 行库（`append_session_events` 真实写路径，每行 ~1.8KB 文本）。零改动仓库。

### 1. 现有 `scan` 的真实成本（session_event，行均 1795 B）

| 库规模 | `scan(0, 50)` | `scan(50%, 50)` | `scan(末页, 50)` | `scan(0, 1000)` |
|--------|---------------|-----------------|------------------|-----------------|
| 5,000 行（16 MB） | 0.68 ms | 10.0 ms | 11.7 ms | 2.46 ms → **只回 500 行** |
| 50,000 行（129 MB） | 0.67 ms | 93 ms | 120 ms | 2.90 ms → **只回 500 行** |

- **首页 O(1)**（`.all()` 返回的是 lazy iterator，`skip/take` 真早停）；
- **深翻 O(offset)**：skip 逐行 decode，50k 行时 50% 处 93 ms；
- 生产库实测 **1.82 GB**（`~/.dev-team/workspaces/*.redb`，正在被运行中的应用独占，无法复制）→ 按 2.58 KB/行 ≈ **70 万行** → 外推**深翻一页 ≈ 1.3~1.7 秒**。这是分页语义的固有成本，agent 会习惯性翻页，必须让它几乎不需要翻。

### 2. 「上限 1000 条」被既有常量卡死

`envelope.rs::MAX_SCAN_LIMIT = 500`。`scan(_, _, 1000)` 静默截到 500 —— **需求要 1000 就得改常量或另开读路径**（本文档倾向后者：query 自带过滤/排序/投影，本就不该复用查看页的页长上限）。

### 3. 体积才是真瓶颈，不是耗时

| 量 | 字节 | 对照 L1 (`MAX_RESULT_BYTES = 50_000`) |
|----|------|----------------------------------------|
| session_event 50 行 | **89.8 KB** | 1.8 倍 ❌ |
| session_event 1000 行 | **≈ 1.6 MB** | **32 倍** ❌❌ |
| 全量 50k 行 | 79 MB | 1580 倍 |
| 1000 行序列化耗时 | 1.0 ms | 耗时完全不是问题 |

结论：**40 KB 字节预算闸是唯一护栏**；默认 `limit=50` 对 `session_event` 会被截到约 **22 行**。默认值建议按表自适应（或干脆统一给 20）。

### 4. `describe` 输出体积实测（specta 类型图真跑）

| 范围 | 字节 |
|------|------|
| 表清单（10 行：name + summary + count） | 1,175 B |
| 全量字段说明（10 表 / 87 字段） | 9,217 B |
| 最大单表 `PhaseRecord`（15 字段） | 1,270 B |
| 最小单表 `WorkspaceRecord`（3 字段） | 404 B |

补上 `session_event`（手写 3 条）+ run 三表（33 字段）后约 **11 KB** 全量 / **1.4 KB** 清单。→ 「无参给清单、`table` 下钻」是对的；全量 11 KB 一次吐出也不炸，但每轮白烧。

### 5. specta 派生路径的 4 个真实坑（已逐个撞到）

| 坑 | 现象 | 解法 |
|----|------|------|
| `#[serde(with = "event_key_serde")]` 的 4 个字段 | `specta_serde::Format.map_types` **直接 Err**：`Unsupported serde attribute at 'ChecklistItemRecord.item_key': #[serde(with)] changes the wire type` | 4 字段加 `#[specta(type = String)]`（线格式本来就是十六进制串） |
| `SessionEventRecord` 无 `Type` derive | 14 张表里唯一不 implement `specta::Type`（不做 IPC 出线，`event` 是 `Box<RawValue>`） | 手写 3 条字段说明，或给它补 derive + `specta(type = ...)` |
| `NamedReference` 无 `Display` | 类型渲染 `?named` | 自实现：`types.get(&r).name` → `ChangeStatus` 等 |
| 内嵌 struct 也进类型图 | `AgentModelTiers` / `SessionConfigSnapshot` / `ChangeActivePhase` 出现在 `into_sorted_iter` 里 | 按 `MODEL_ENTRIES` 名单过滤（表 ≠ 所有命名类型） |
| `run`/`run_node`/`run_edge`/`store_meta` | **store lib.rs 未导出这四个类型** | 工具要覆盖 run 运行史必须加 `pub use` |

**Format 应用后的线名（实测，与 `bindings.ts` 完全一致）**：

```
ChangeRecord: id, name, title, workflowType, createdAt, status, archivedAt,
              activePhase, worktree, baseCommit          ← camelCase，不是 change_id
PhaseRecord:  id, changeId, phase, attempt, verdict, report, skipped, stale,
              backtrackTo, backtrackReason, executorSessionId, evaluatorSessionId,
              decisionSessionId, startAt, timestamp
```

不应用 Format 就吐 snake_case（`change_id`）—— agent 拿它查询会**恒零命中**。这条是硬约束。

### 6. 真实 `session_event` 落库结构（决定下钻路径）

```json
{ "eventKey": "0x073f691f…", "sessionId": "ses-0-…",
  "event": { "kind": "message", "role": "user", "seq": 1, "turnId": 4,
             "timestampMs": 1794555505557,
             "blocks": [{ "kind": "text", "text": "随便问我一个问题", "citations": null }],
             "parentToolUseId": null } }
```

`kind` 是 **flatten 到 event 顶层**的（不存在 `event.eventKind`，也不是 `event.type`）。所以下钻必须支持点号：`event.kind = "message"` + `event.role = "tool"` 是「找工具调用」的唯一路径。数组下标（`event.blocks.0.name`）**建议不支持**——过滤后整条看即可。

### 7. 12 个缺 doc 的字段（实测 undoc 计数与静态统计完全一致）

`PhaseRecord`: `phase` / `attempt` / `skipped` / `stale` / `backtrack_reason` / `evaluator_session_id` / `decision_session_id`（7）
`ChecklistItemRecord`: `item` / `pass` / `evidence`（3）
`RunNodeRecord`: `phase` / `attempt`（2）

另：`PhaseRecord.change_id` 的 doc 是整段长文，按「首行截 120 字」切出的是半句（`…归属键换锚`）——建议按「首个句号 / 分号 / 换行」截断，再兜底 120 字。

## 已定方案（本轮探索的收敛）

| 分叉 | 定案 |
|------|------|
| 查询形态 | **A1 变体**：结构化 JSON 外壳 + SQL 词表（`= != > >= < <= LIKE IN IS NULL / IS NOT NULL`），无解析器 |
| 说明来源 | **B2′**：specta 运行时类型图派生（表 doc + 字段 doc + 类型 + camelCase 线名），补 12 条缺失字段注释 + ratchet 测试 |
| 会话可见性 | **explore + change 两臂**（archi / debug 不动） |
| 上限收口 | 行数 `limit` 缺省 50 / 上限 1000 + **工具内字节预算 ~40KB**（先到者止 → `truncated: true` + 提示用 `select`） |
| 未定小项（我的建议） | 表范围补 `run/run_node/run_edge`（`store_meta` 仍排除：库级格式标记，无查询价值）；`api_key` 字段级打码为 `"***"`；工具 1 留可选 `table` 做下钻 |

## 两工具的最终契约

### `db_tables`（发现）

```
入参  {}                        → 表清单（14 或 13 行）：{ dimension, table, count, summary }
入参  { table: "phase" }        → 该表全字段：{ table, summary, fields: [{ name, type, doc }] }
      { scope?: "user"|"workspace" }
```
`summary` = struct doc 首行截 120 字；`name` = **落库线名**（camelCase，直接可喂 `db_query`）；`type` = `string` / `number` / `boolean` / `ChangeStatus` / `string | null`。

### `db_query`（取数）

```json
{ "table": "phase",
  "scope": "workspace",
  "where": [ { "field": "change_id", "op": "=",    "value": "2026-11-14-x" },
             { "field": "verdict",   "op": "!=",   "value": null },
             { "field": "phase",     "op": "LIKE", "value": "dev-%" },
             { "field": "attempt",   "op": "IN",   "value": [1, 2] } ],   // 默认全 AND
  "order": { "field": "timestamp", "desc": true },        // 缺省 = 主键自然序
  "select": ["id", "phase", "attempt", "verdict"],        // 可选投影（value 只留这些键；key 恒在）
  "offset": 0,
  "limit": 50 }                                            // 缺省 50，上限 1000
```

返回：

```json
{ "table": "phase", "total": 137, "matched": 12, "returned": 12,
  "truncated": false, "records": [ { "key": 41, "value": { … } } ] }
```

- `truncated: true` 的两因：行数到 `limit` **或** 字节预算先到；对齐文案提示「用 select 收窄字段 / 加 where 条件 / 翻 offset」。
- 字段路径支持点号下钻（`event.kind`、`configSnapshot.model`）；未知字段 / 未知 op / 未知表 → `Err` 且**列候选**（`可用字段: id, name, status…`）——错误文案即自纠通道。
- 比较语义：数字按数值、字符串按字典序、`null` 恒排末位、跨类型按类型序；`LIKE` 只支持 `%`（前后缀/包含），下划线通配不做。
- 只读：store 侧无写面；`ToolTier::Read`。

## 语义规格（实测后收紧）

### `LIKE`

```
模式串 'x%'   → 前缀      '%x'   → 后缀      '%x%'  → 包含      'x'    → 退化为等值（与 SQL 同）
'%' 匹配任意长度（含空串）；'\%' / '\_' 表示字面 % 与 _
区分大小写（不做 SQLite 的 NOCASE）；不支持的：'_' 单字符通配、ESCAPE 子句、正则
null 不匹配任何 LIKE
```

理由：数据是 `ses-…` / `dev-design` / `ChangeStatus` 这类标识符，大小写敏感更可预测；`_` 通配在标识符里（`change_id`）极易误伤，直接不做。

### 比较与类型

```
= !=        ：同类型比较；null 与任何值 → = 为 false、!= 为 true
> >= < <=   ：number 按数值 / string 按 Unicode 码点序 / bool false<true；任一侧 null → 不匹配
IN          ：value 必须是数组，逐元素按 = 语义
IS NULL / IS NOT NULL ：无 value 参数
类型不符（如 string 查 number 字段）→ Err 并指明字段类型（静默 false 会让 agent 误以为没数据）
```

### `select` 投影

```
["id","phase","event.kind"] → 输出 { "id":…, "phase":…, "event": { "kind": "message" } }
key 恒在（不占 select 名额）；未知字段 → Err + 列候选
```

### 分页与预算

```
limit 缺省 20 / 上限 1000；offset 缺省 0
字节预算 40 KB：累加到该条会超预算时停止，返回已收部分 + truncated=true + truncatedBy: "bytes" | "limit"
提示文案：用 select 收窄字段 / 加 where 条件 / 翻 offset（注意深翻在十万行级约 1 秒）
```

### 排序

`order.field` 缺省不存在（= 主键自然序，即 `event_key` / `id` 升序）。
带 `order` 时**必须全表 decode**（70 万行库 ≈ 300 ms），此时不早停；无 `order` + 无 `where` 可早停（凑够 offset+limit 即停）。

## 落点清单（按最终方案）

| 文件 | 改动 |
|------|------|
| `infra/store/Cargo.toml` | + `specta-serde`（确保 serde 重命名在单 crate 构建下也生效） |
| `infra/store/src/model.rs` | 补 12 条字段 `///`（`PhaseRecord` ×7 / `ChecklistItemRecord` ×3 / `RunNodeRecord` ×2）；4 个 `event_key_serde` 字段加 `#[specta(type = String)]`（否则 `Format` Err） |
| `infra/store/src/envelope.rs` | `describe()`（走 `Types` + `specta_serde::Format`，按 `MODEL_ENTRIES` 过滤内嵌类型；`SessionEventRecord` 手写 3 条说明）；`query(spec)`（独立读路径，**不复用 `MAX_SCAN_LIMIT`**）；补 `run`/`run_node`/`run_edge` 三行 entry + `key_of` |
| `infra/store/src/store.rs` | `Store::describe()/query()`；`WorkspaceStores.global: Arc<Store>` + `global_shared()` |
| `infra/store/src/lib.rs` | 导出 `ModelDoc`/`FieldDoc`/`QuerySpec`/`QueryPage`（**不 derive `Type`** → 零 IPC / 零前端改动）；补导出 `RunRecord`/`RunNodeRecord`/`RunEdgeRecord`（现未导出，工具覆盖 run 史必须） |
| 类型渲染 | 自实现 `DataType → String`（`NamedReference` 无 `Display`，需 `types.get(&r).name`）；`DataType::Primitive` 按位宽收成 `number` / `string` / `boolean` |
| `infra/agent/src/sdk/db_tool.rs`（新） | `DbContext{ global: Arc<Store>, workspace: Arc<Store> }` + `execute(name, &input)`；字节预算与截断提示在此 |
| `infra/agent/src/sdk/mount.rs` | + `DB_TABLES_TOOL` / `DB_QUERY_TOOL` 常量 + 两定义 + `ToolExec::Db` + tier Read；`TOOL_NAMES` 七件不动 |
| `infra/agent/src/sdk/tools.rs` | `input_path` 的显式 None 臂 +2 名（照 archi 先例） |
| `infra/agent/src/sdk/loop.rs` | + `ToolExec::Db` 分派臂 |
| `infra/agent/src/sdk/runner.rs` | `PumpPayload` + `stores: Option<…>`，per-turn mint（判据 = 名字清单含 db 工具，`contains_tool` 复用） |
| `infra/agent/src/compose.rs` | builder 加 `with_db_stores`；explore 臂 +2 名；change 臂 +2 名 |
| `infra/agent/src/sdk/policy.rs` | `Read` 档天然放行，零改动（新工具不得进 Write/Execute 档） |
| 前端 / IPC | **零改动** |

## 测试与 spec 面

- store：`describe` 的字段集 vs 各模型**真实 serde JSON 键集**双向断言（ratchet：新字段无 doc 即红）；`query` 的 op 全覆盖 / 路径下钻 / 排序 / 分页 / 字节预算截断 / 未知字段列候选 / `api_key` 打码 / 只读。
- infra/agent：工具定义 schema（`mount` 挂载清单成员校验 + tier）；执行体错误串；`compose_test` 断言 explore/change 两臂名单含两新工具、archi/debug 两臂不含。
- spec：新增 capability（暂名 `desktop-agent-db-tools`）+ 改写 `desktop-agent-execution`「会话级工具面注入」requirement 的两臂名单；`desktop-db-inspector` 仅当补 3 张表进查看页时需要补一句（可见模型变多）。

## 风险与后续

- **上下文污染**：本轮/本会话自家转录可被自读（`session` / `session_event`），工具描述须警示；预算闸是唯一硬拦。
- **全表 decode 延迟**：`scan().all()` 对 `session_event` 全解码，万级行可能数百 ms；先不做索引下推，若实测卡顿再按「`where` 等值命中二级索引 → `scan().secondary().range()`」优化。
- **不做索引下推的代价**：`ORDER BY` 与任意字段过滤必全表；表大时靠 `limit` 早停（无条件时可就地停，带 `order` 必须全扫）。
- **后续可加**（不在本期）：聚合（`count`/`group by`）、按轮/按 run 的专用查询、把 `describe` 接到 DB 查看页显示字段说明。

## 复现实测（临时件，未落仓库）

```
/tmp/dbprobe/probe        # 临时 crate：依赖本仓 store/agent crate
  gen <path> <rows> <payload>   造 N 行真实 session_event
  probe <path>                  各模型算 scan 首页/中段/末页/1000 行耗时
  cost <path>                   decode vs 序列化成本拆分
  describe                      specta 类型图 → 表/字段说明（含 undoc 计数）
  describe2                     应用 specta_serde::Format 后验证 camelCase 线名
  show <path>                   打印真实 session_event 行（含 key 与 event 结构）

构建：CARGO_TARGET_DIR=<repo>/src-tauri/target cargo build --release   # 复用已编译产物，46s
```

生产库 `~/.dev-team/workspaces/*.redb`（1.82 GB）复制失败 = `Device or resource busy`（运行中的应用持有 redb 锁），故实测用 dev 库副本（1.5 MB / 53 行）+ 自造 5k / 50k 行库。
