# 探索笔记:native_db 升级 × DB 查看页 × store 未来架构

- 日期:2026-09-23
- 状态:探索收敛,待 phase-proposal 收口
- 拟议 change 名:`native-db-store-upgrade`

## 背景与三个诉求

1. **依赖升级**:store 从 redb 直接驱动升级为 native_db;
2. **DB 查看页**:新增查看 db 的页面,与调试 agent 一并收入「系统工具」侧栏组;
3. **store 架构规划**:crates/infra/store 适应未来扩展更多数据结构。

触发升级的未来数据:**workflow 过程数据,包括每次 agent 执行明细**(phase 执行、评估结论背后的完整事件流、成本、时长——repo 的 workflow.json 只存 eval 结论与 file_log,执行明细 repo 里没有)。

## 现状地图

```
Desktop App (Tauri)
┌─────────────────────────────────────────────────────────┐
│  前端 (React)                     后端 (src-tauri)      │
│  侧栏「页面」组                    main.rs               │
│  ├─ 变更 (changes)                └─ Store::open(db)    │
│  └─ Agent 调试 (agent)            commands/             │
│    (本地 state 切换,无路由)        ├─ queries/workspaces/│
│                                  └─ exec (agent 三命令) │
└──────────────────────────────┬──────────────────────────┘
                               │
                    ┌──────────▼──────────┐
                    │  crates/infra/store │  354 行,redb 直接驱动
                    │  schema_version 手工 │  (user_meta 表)
                    │  user_workspaces        (&str → JSON)   │
                    │  user_agent_runs        (i64 → JSON)    │
                    │  user_agent_run_events  ((run_id,seq) → JSON)│
                    └─────────────────────┘
```

## 关键事实澄清:native_db 不是"换引擎"

native_db(0.8.2)构建在 redb 之上,事务/ACID/文件格式仍是 redb。升级实质是加模型层:

```
现在:  dev-team ─▶ store ─▶ redb        (手写表定义/事务/编解码/schema_version)
升级后:dev-team ─▶ store ─▶ native_db ─▶ redb
                             └─ native_model(类型版本/迁移治理)
```

买到的能力:模型宏注册(新数据 = 定义 struct + 注册)、二级索引、内建迁移(schema_version 手工轮账退役)、实时订阅(未来 UI 刷新通道)。

### 记录家族契合度(升级前评估)

| 记录家族 | 契合度 | 备注 |
|---|---|---|
| `WorkspaceRecord` | ★★★ | PK=root,教科书案例 |
| `AgentRunRecord` | ★★★ | 自增 id 自分配(现状也是 max+1);白赚二级索引 |
| `user_agent_run_events` | ★☆☆→已解决 | 复合键 + 不透明 JSON 与模型范式摩擦最大;类型化建模裁定后见下文 |
| `user_meta`(schema_version) | 净删除 | 被 native_db 迁移系统整体取代 |

## 已做决定(探索两轮收敛)

1. **未来数据**:workflow 过程数据 + 每次 agent 执行明细 → 模型版图见下;
2. **事件流类型化建模**(不走 raw KV 保留通道);
3. **欢迎态不露系统工具**:欢迎屏维持完全独立页面,不渲染壳;DB 查看页只在壳态(workspace 已选)可达;
4. **查看器第一版只读**:模型清单 + 计数 + 分页扫描 + 单条 JSON 查看;写操作等真有 debug 需求再说。

## 三个发现(改变讨论格局)

### 发现 1:core/agent 已有类型化事件模型,自带逃生舱

`core/agent/src/event.rs` 的 `AgentEvent` 是五变体类型化模型(`RunStarted` / `Message` / `SystemNotice` / `RunResult` / **`Raw`**),文档注释明说"serde camelCase 线格式同时是**落库形态**与前端 DTO 镜像基准",Raw 变体"未识别事件透传,**永不丢事件、永不炸解析**"。

推论:类型化建模的类型已存在;store 存 opaque JSON 只是回避依赖方向问题,不是没有类型。旧数据迁移的丢失风险被 Raw 变体兜住——旧 JSON 解不出的行包成 Raw 存,零丢失有构造性保证。

### 发现 2:infra → core 依赖有先例,store 是唯一特例

`infra/agent`(agent-cli)正常依赖 `core/agent` 直接用 `AgentEventKind`。"store 禁依赖 core 契约 crate(自含模型)"是局部特例而非全局层规,其成立前提(模型平凡)已被过程数据打破。

### 发现 3:workflow 过程数据的维度由现有 spec 裁定为 user

`desktop-data-dimensions` spec 给 workspace 维度定硬约束:"SHALL 作为派生数据可重建"(三笔账之三)。过程数据是不可重建的活动历史,也不该进 repo(转录进 git = 噪音 + 体积 + 隐私)。

```
裁定:过程数据 → user 维度
  记录携带 workspace_root + change_name 字段
  查询靠二级索引,不靠分库
  红利:三笔账(redb 双开锁/gitignore 分型/克隆重建)全部不触发
  workspace 维度库(change 缓存/索引/图谱)仍是独立未来话题
```

## 类型所有权裁定:包装建模(store 依赖 core 纯类型作嵌装载荷)

```
选项 1(采纳):包装建模                 选项 2(否决):镜像类型
#[native_db]                            store 复制一份形状
struct AgentEventRecord {
  #[primary_key]                        代价:双份维护、形状漂移、
  key: (run_id, seq)?   ← spike ①      app 层映射代码;core 类型
  #[secondary_key]                      演进需人肉同步
  run_id: i64,
  event: agent::AgentEvent  ── core 类型作嵌装载荷
}
```

- 关键技巧:native_db derive 打在 store 的包装 struct 上,core 类型保持 derive-free,native_model 版本治理留在 infra 侧;
- 规则修订:「store 禁依赖 core」→「**store 禁依赖 core 行为,可依赖 core 纯类型作嵌装载荷**」;`PhaseLog` / `ChecklistItem` 等 workflow 领域形状同理引用;
- 选项 3(抽 contract crate)等第二个消费方出现再谈,现在做是过度设计。

## 模型版图(升级后全景)

```
user db(app data dir,native_db)
├─ WorkspaceRecord      PK: root                        [现状平移]
├─ AgentRunRecord       PK: id                          [现状扩展]
│    + 二级索引: change_name / status
│    + 可选联动字段: source(manual|workflow), workflow_run_id?, phase?
├─ AgentEventRecord     PK: (run_id, seq)? ← spike ①    [类型化新建]
│    二级索引: run_id;嵌装 agent::AgentEvent(含 Raw 逃生舱)
├─ WorkflowRunRecord    PK: id                          [新建]
│    字段: workspace_root, change_name, workflow_type, status, started_at
│    二级索引: (workspace_root, change_name)
├─ PhaseExecutionRecord PK: ?                           [新建]
│    字段: workflow_run_id, phase, attempt, agent_run_id → AgentRunRecord
│    二级索引: workflow_run_id
└─ (未来租户) Setting / WindowState ...
```

二级索引回本点:change X 完整执行史 = `WorkflowRun → PhaseExecution → AgentRun → Events` 一条索引扫描链;现状"全表读 + 内存排序"撑不起该查询形态。

### 待裁(有倾向):AgentRunRecord 联动设计

一张表加可选 workflow 上下文字段(手动调试 = `source=manual` 特例),还是 workflow 驱动 run 单独成表?
**倾向一张表加字段**:事件流、重放、查看器全部复用,调试页成为 workflow 执行史的免费子集。

## DB 查看页设计

### 侧栏结构(已收敛)

```
「页面」组                「系统工具」组
└─ 变更                   ├─ Agent 调试(平移)
                          └─ DB 查看(新增)
```

`TopPage` 加变体;欢迎态(root===null)维持完全独立页面不渲染壳。

### 序列化选型与查看器耦合

native_model 默认 bincode(查看器看到二进制);若可按模型配 serde_json 后端,查看器几乎白拿(spike ③)。选型为"人可读"加权。

### 信封 API(穿透 store 类型壁垒)

现有纪律"redb 类型不越 crate 公共面"平移为 native_db 类型不越面。查看器经 store 暴露的通用记录信封:

```
list_models() → [{name, count}]          ← 模型清单
scan(model, offset, limit) → Vec<RecordEnvelope{ key: Value, value: Value }>
```

新模型注册即自动可被查看器浏览,零额外代码——依赖升级与查看页最漂亮的咬合点。

## 数据迁移账(躲不掉)

`desktop-data-dimensions` spec:agent 运行事件转录"不可重建、非派生缓存"——丢旧数据不成立,须真实迁移。native_db 迁移系统管同模型 shape 演进,不管跨表布局导入,须写一次性 legacy 导入:

```
启动时探测旧文件
  旧 user.db(redb 手写表)──read_only 打开──▶ 逐表读出
  事件解不出的行 → AgentEventKind::Raw 兜底(零丢失)
  写入新 user.db(native_db,事务内)→ 旧文件改名留档(.bak)
```

- 底层同为 redb:旧文件只读、新文件经 native_db 写,无锁冲突;
- store 短期同时保留 redb 直依赖(读旧)与 native_db(写新),迁移稳定一个版本后收掉 redb。

## Spike 清单(进 change 前必须验,按优先级)

1. **复合主键**:`(run_id, seq)` 能否直接做 native_db PK?不行退路:合成键(u128 打包或字符串)——事件模型地基;
2. **redb 版本收敛**:native_db 0.8.x 自带 redb 与仓库 pin 的 `=4.3.0` 能否统一,还是 redb 退为纯传递依赖;
3. **序列化后端**:native_model 能否按模型选 serde_json——决定查看器白拿 JSON 还是自己解 bincode。

## 变更拆分倾向

**一个 change 打包**(依赖升级 + 迁移 + 架构 + 查看页 + 系统工具组),tasks 分轨:查看页吃升级红利(信封 API + 模型注册),分两个 change 会导致信封 API 做两遍。

## 遗留开放问题

- AgentRunRecord 联动设计最终裁(倾向已述);
- native_db 0.8.x pre-1.0 的 API 稳定性心智账(spike 2 一并看)。

## 参考资料

- [native_db on crates.io](https://crates.io/crates/native_db)
- [native_db 0.8.2 README (docs.rs)](https://docs.rs/crate/native_db/0.8.2/source/README.md)
- [native_db GitHub](https://github.com/vincent-herlemont/native_db)
- 仓库内:`openspec/specs/desktop-data-dimensions/spec.md`(维度裁定依据)、`crates/core/agent/src/event.rs`(类型化事件模型)、`crates/core/workflow/src/model/workflow.rs`(workflow 领域形状)
