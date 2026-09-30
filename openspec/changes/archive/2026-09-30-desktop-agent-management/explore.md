# Desktop 全局 Agent 管理(探索记录)

> 2026-09-30 首轮探索。需求原文:
> 【Desktop】增加全局 agent 管理页,不关联工作区;包含两类实例,provider 和 agent;
> 每个 agent 可自定义名称,选择 agent engine、provider;provider 支持配置 url、
> api_key、model(high、medium、low)参数;将某个 agent 标记成默认,workspace 先
> 使用默认,后续改造 workspace 关联 agent。

## 一、现状:三处显式预留的"待决座位"

本需求恰好落在代码里三个写明"后续迭代"的座位上:

| # | 座位 | 位置 | 现状 |
|---|------|------|------|
| 1 | `DEFAULT_ENGINE` 硬编码默认引擎 | `packages/desktop/src-tauri/src/commands/exec/agent.rs:186` | `EngineKind::Sdk` 常量,注释明言"后续与 api key 一起改为配置读取" |
| 2 | `EngineConfig::from_hardcoded_slot()` 硬编码预留位 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/config.rs` | 三字段(api_key/base_url/model)全空,注释明言"配置落盘座位为后续迭代待决问题";现状 = 用户手填代码才能跑 sdk 引擎 |
| 3 | `DbDimension::User` user 维度租户预留 | `packages/desktop/src-tauri/crates/infra/store/src/store.rs:79` | 全局库 `desktop-global.redb` 现仅 `WorkspaceRecord`,注释预留"及未来 user 维度租户" |

相关架构事实:

- **引擎门面**:`EngineFacade::runner_for(kind, engine_cfg)`(`crates/infra/agent/src/lib.rs`)是引擎构造唯一 match 点;`EngineKind = Cli | Sdk` 即需求中的 "agent engine"。
- **消费面零改动承诺**:`EngineConfig` 注释承诺换源时消费面(`runner_for` 签名与编排层)零改动——只要"默认 agent → (EngineKind, EngineConfig)"解析收在命令层,此承诺可兑现;`EngineConfig` 三字段结构体不动,三档 model 在解析层收敛成单 model 塞入。
- **发起面现状**:`agent_start` 收 `engine: Option<EngineKind>`,缺省收敛 `DEFAULT_ENGINE`;调试页 run form 有 engine 下拉(sdk 默认/cli,`src/views/agent/components/agent-run-form.tsx`);explore 页发起不传 engine,走缺省。
- **侧栏**:已有"系统工具"组([Agent 调试][数据库],`src/components/app-sidebar.tsx`),非 workspace 入口语义首次出现即在此,新页面天然落位。
- **store 双库布局**:全局库(user 维度)与 per-workspace 库;provider/agent 属 user 维度数据,住全局库,天然"不关联工作区"。

## 二、概念模型

```
┌─ provider 实例 (user 维度,全局库) ───┐    ┌─ agent 实例 (user 维度,全局库) ─┐
│ name (唯一)                          │    │ name (唯一)                     │
│ base_url                            │◄───│ engine: cli | sdk               │
│ api_key                             │引用│ provider_id (sdk 必填, cli 可空) │
│ models: { high, medium, low }       │    │ ★ 默认标记 (全局至多一个)        │
└─────────────────────────────────────┘    └────────────────────────────────┘
                                                          │
                run 发起(缺省路径)                         ▼
        解析默认 agent ──► engine + provider ──► EngineConfig ──► runner_for
```

- CLI 引擎不消费 provider(无 url/key/model),agent 的 provider 引用对 cli 可空——避免"必须选 provider 但引擎不用"的别扭 UI。
- UI 页面草图:单页两栏(Providers 列表 + Agents 列表),路由 `/agents` 一条,侧栏系统工具组加一项,零路由结构变更。

## 三、已拍板决策

### D1 — model 三档本期"只存不选"(原 Q1,选 a)

- provider 的 `models: { high, medium, low }` 存储完整;
- 消费侧固定取一档(暂定 **high**,具体档位 design 阶段可调)解析进 `EngineConfig.model`;
- `AgentRunParams` 加 effort 参数 + 前端档位选择器 = **后续迭代**,不进本期范围。

### D2 — api_key 遮蔽责任在前端展示层(原 Q2 选 b;2026-09-30 修订:遮蔽层从读 DTO 后移至前端)

- **写路径**:明文过 IPC 入库(全局 redb 在 app data 目录,非 git 追踪,与"机密不入 git 追踪文件"纪律不冲突);
- **读路径**:后端全链路明文(IPC 读回明文——单用户桌面应用,全链路明文读取风险可控),**遮蔽只发生在前端展示层**(列表/详情显示 `sk-***abc` 形态);
- **编辑语义**:key 输入框展示遮蔽占位,留空 = 保持原值不变;
- 简化收益:后端不做遮蔽 DTO、不做信封遮蔽臂,读写单 DTO;db-inspector 明文展示为该决策的自然推论,接受;
- 纪律延续:明文不进日志、不进 `Debug` 输出(D7 手写遮蔽 Debug)、不进 git 追踪文件。

### D3 — 调试页发起面同步改(原 Q5,选 b)

- 调试页 run form 的 engine 下拉 → **agent 选择器**(默认选中默认 agent);
- `agent_start` 的 IPC 参数面随演进:`engine: Option<EngineKind>` 直选改为 agent 选择(具体参数形态 design 定),缺省 = 解析默认 agent;
- explore 页等缺省路径自动获得默认 agent 语义,零改动。

### D4 — 稳定 id 主键 + name 唯一二级索引(原 Q3,已拍板)

- `AgentProvider` / `AgentInstance` 均以稳定 id 为主键,name 走唯一二级索引;
- 后续 workspace→agent 关联引用 id,实体改名不破坏引用。

### D5 — 删除语义(原 Q4,已拍板)

- 删被 agent 引用的 provider:**阻止 + 报错**(提示引用方,不级联);
- 删默认 agent:**清空默认标记**(不顺延),下次 run 显式报错引导去管理页。

### D6 — 实体命名(原 Q7,已拍板)

- 概念命名:**`AgentProvider`**(provider 实体)/ **`AgentInstance`**(agent 实体),UI 文案仍叫"Provider / Agent";
- 落库类型按 store 既有 `*Record` 惯例:`AgentProviderRecord` / `AgentInstanceRecord`(具体类型名 design 阶段可微调);
- "AgentInstance" 呼应需求原文"两类实例"的表述,同时回避 `agent` 契约 crate / `AgentRunRecord` 的重载撞名。

### D7 — `AgentProviderRecord` 手写遮蔽 Debug(原风险 R2 撞点 A 处置)

- `model.rs` 既有 `*Record` 惯例含 `derive(Debug)`,`AgentProviderRecord` **破例不 derive**,手写遮蔽 Debug impl(api_key 位输出 `sk-***abc` 形态);
- 理由:全 app 现无日志框架,"release 无 debug 日志"靠的是日志不存在(过程保证)而非机制保证;遮蔽 Debug(~5 行)把"明文不进日志"变成结构保证,不依赖未来日志配置;
- 开发体验无损:`assert_eq!` 失败仍打印除 key 外的一切;key 的测试比较走 `PartialEq`(沿 `EngineConfig` 既有惯例);
- 边界表附加条款:未来引入日志框架时 release 禁 debug 级(附加约束,不作为放宽 Debug 的依据)。

### D8 — 机密边界表 + 统一注释标记(原风险 R2 处置框架)

- **锚点 = 能力 spec**(`specs/desktop-agent-management/spec.md` 的机密边界表),非 design.md(design 随 change 归档,spec 长存);
- 边界表内容——明文允许出现在:写向 IPC body、读向 IPC body、全局库文件、进程内;禁止出现在:`Debug`/日志输出、git 追踪文件、错误串;
- **统一注释标记**(固定 token 可 grep):`// 机密面有意放宽:…,边界表见 specs/desktop-agent-management`;放置点:`EngineConfig` 注释更新、`AgentProviderRecord` 类型与 api_key 字段、读写 DTO 的 api_key 字段、前端遮蔽展示组件;
- 分工:标记解决"放宽被误判误修"(方向一),遮蔽 Debug 的结构决定解决"无意识扩散"(方向二),互补不可互替。

## 四、风险与验证点

- **R1 native_db 加模型的存量库兼容**:全局库已有存量文件,`global_models()` 追加两个新模型属 schema 加法——需验证 native_db 对"旧库文件 + 新增模型定义"的打开路径无迁移负担(或读文档确认 additive 语义)。记忆约定"不做旧库兼容、勿重新引入迁移层",此处需确认是"加法无感"而非"需要迁移"。验证手段:手上有存量 `desktop-global.redb` 的机器打开测试。
- **R2 机密面放宽的意识化 → 已解(D2 修订 + D7 + D8)**:`EngineConfig` 现有"不进 serde/specta 线面"纪律有意放宽为"后端全链路明文 + 前端展示遮蔽"(D2 修订);扩散防线 = 手写遮蔽 Debug(D7,结构保证);意识化载体 = spec 锚定的机密边界表 + 统一注释标记(D8)。曾评估的替代方案:信封遮蔽臂(随 D2 修订不再需要)、DPAPI 加密落盘(见范围外)、序列化层遮蔽(不可行——serde Serialize 即持久格式,遮蔽即丢真值,已否决)。

## 五、本期范围外(后续路线)

- **workspace 关联 agent**(需求原文明言"后续改造"):`WorkspaceRecord` 或 workspace 配置加 agent 引用;run 解析顺序演变为 workspace 关联 → 全局默认。本期数据模型不需为此预留(D4 的稳定 id 已足够支撑)。
- **effort 进 run 参数**(D1 的后续半边)。
- **api_key 静态加密落盘**(Windows DPAPI):单用户同机威胁模型下收益边际,给读写加密密缝,缓(未来有合规要求再启)。
- provider 连通性测试按钮(未要求,不进范围)。

## 六、拟议 change 名

`add-desktop-agent-management`(与 `desktop-agent-execution` / `desktop-workspace-config` 等 spec 命名对齐)。
