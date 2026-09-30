# 测试设计: desktop-agent-management

> **日期**: 2026-09-30

---

## 验收范围

| AC ID | 验收条件 | 被测文件或模块 |
|--------|---------|----------|
| AC-1 | URL `/agents` 渲染管理页两栏（Providers / Agents）；侧栏「系统工具」组出现 [Agent 管理] 且 `/agents` 时 active（`nav-agents` testid 可查） | packages/desktop/src/views/agents/agents-view.test.tsx |
| AC-2 | 管理页可新建 / 编辑 / 删除 provider；重名保存报 `Err`；api_key 编辑框留空提交后原值不变 | packages/desktop/src-tauri/src/commands/agents/mod_test.rs |
| AC-3 | 可新建 / 编辑 / 删除 agent；`sdk` 引擎未选 provider 时保存报错；`cli` 引擎 provider 可空 | packages/desktop/src-tauri/src/commands/agents/mod_test.rs |
| AC-4 | 标记默认后全局恰一个；切换默认时旧标记自动清除 | packages/desktop/src-tauri/crates/infra/store/src/store_test.rs |
| AC-5 | 删被 agent 引用的 provider 返回 `Err` 且两类记录均不消失；删默认 agent 后记录删除且默认标记消失 | packages/desktop/src-tauri/crates/infra/store/src/store_test.rs |
| AC-6 | provider 记录含 `models {high, medium, low}`；sdk 运行解析的 `EngineConfig.model` 取 high 档 | packages/desktop/src-tauri/src/commands/exec/agent_test.rs |
| AC-7 | 不带显式 agent 的发起（explore 缺省路径）解析默认 agent 组装 runner；无默认 agent 返回 `Err` 引导管理页；`crates/core/agent` 无 agent / engine 字样，`runner_for` 签名与编排层零改动 | packages/desktop/src-tauri/src/commands/exec/agent_test.rs |
| AC-8 | 调试页 run form 出现 agent 选择器且默认选中默认 agent；选择 sdk agent 发起走 SDK 引擎（时间线 / 落库 / 重放组件零改动复用） | packages/desktop/src/views/agent/components/agent-run-form.test.tsx |
| AC-9 | `AgentProviderRecord` 无 `derive(Debug)` 且手写遮蔽 impl；统一标记注释可 grep；前端列表 / 详情呈现 `sk-***abc` 形态；错误串无 api_key 明文 | packages/desktop/src-tauri/crates/infra/store/src/model_test.rs |
| AC-10 | 既有 `desktop-global.redb` 打开成功，`WorkspaceRecord` 原样可读，两新模型可写入读出；store 无迁移代码 | packages/desktop/src-tauri/crates/infra/store/src/store_test.rs |
| AC-11 | db-inspector 全局库清单出现两新模型并可分页扫描（查看器零改动） | packages/desktop/src-tauri/crates/infra/store/src/envelope_test.rs |
| AC-12 | `cargo test --workspace` 全绿；`pnpm -C packages/desktop run client:check` 与 `run test` 全绿 | —（见不可测试项 1） |

路由说明（双向对齐补充）：AC-2 / AC-3 的命令链组合用例挂靠 `commands/agents/mod_test.rs`（管理命令为全局库链路入口，真实组合 store crate）；AC-4 / AC-5 / AC-10 的持久化语义挂靠 `store_test.rs`（`Store` 管理操作面为唯一写口单点）；AC-6 / AC-7 的解析链挂靠 `agent_test.rs`（`resolve_agent_engine` 为解析单点，读全局库 `default_agent_instance`）；AC-1 / AC-8 / AC-9 的前端呈现与取数半边分别由 `agents-view.test.tsx`、`agent-run-form.test.tsx`、`masked-api-key.test.tsx` 承载，各 hook / transport / 章节回溯 invoke 入参面。测试框架：Rust 侧为 `cargo test`（workspace 级注册于 `src-tauri` 根，共置 `*_test.rs` 模块），TS 侧为 `vite-plus`（vp test，共置 `*.test.ts(x)`）。

---

## 单元测试

### packages/desktop/src-tauri/crates/infra/store/src/model.rs -> packages/desktop/src-tauri/crates/infra/store/src/model_test.rs

<!-- AC-9 承载章节（遮蔽 Debug 结构保证半边）；AC-6 的三档模型存储半边回溯本章节（嵌装往返）。
     「无 derive(Debug)」为编译期形态：可自动化断言经手写 impl 输出承载——format!("{:?}")
     全文扫描含 sk-*** 形态与末 3 字符、不含明文 api_key；记录比较一律走 PartialEq / 字段面
     （EngineConfig 不派生 Debug 的既有口径）。嵌套类型 AgentModelTiers / AgentEngineKind
     为纯类型行（design 类型定义表），随记录往返与线格式用例一并锚定。 -->

#### 待测功能

- AgentProviderRecord.new(): 新建语义构造：id 置 0（写事务 max+1 覆盖）；api_key 传空即空（无原值可保）
- AgentInstanceRecord.new(): 新建语义构造：id 置 0、`is_default = false`（默认标记唯一写口为 set_default）；provider_id 两态透传

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| AgentProviderRecord 构造与遮蔽 Debug（AC-9） | 正向 | new 三字段载荷（name / base_url / api_key / models 三档）逐字段保真、id=0；比较走 PartialEq 与字段面 | 新增 |
| AgentProviderRecord 构造与遮蔽 Debug（AC-9） | 边界 | 手写 Debug 输出 api_key 位为末 3 字符遮蔽形态：format!("{:?}") 全文含「sk-***」与末 3 字符、不含明文 key 全文（结构保证明文不进日志） | 新增 |
| AgentProviderRecord 构造与遮蔽 Debug（AC-9） | 边界 | api_key 空串 / 长度不足 3 字符时 Debug 输出恒 `sk-***`（全遮蔽兜底，不回显原值） | 新增 |
| AgentProviderRecord 构造 | 边界 | name / base_url / api_key 含空格、中文、emoji、引号、换行时构造与 Clone / PartialEq 保真；models 三档全空串构造合法（模型层不做字段校验） | 新增 |
| AgentInstanceRecord 构造 | 正向 | new 恒 id=0、is_default=false（构造器不产默认标记）；engine Cli / Sdk 两变体与 provider_id None / Some 两态逐字段保真 | 新增 |
| 嵌装往返（native_model serde_json codec） | 正向 | provider 记录（含三档 models）encode / decode 内存往返逐字段相等、native_model 版本封装 version 1（id 5 新登记不与既有 1–4 冲突由打开成功锚定） | 新增 |
| 嵌装往返 | 正向 | instance 记录往返保真：engine 两变体 × provider_id 两态（Option 语义经编解码不漂移） | 新增 |
| serde 线格式 | 正向 | engine 出线 `"cli"` / `"sdk"`（serde camelCase 串值域，与 EngineKind 同线值）；记录键名 camelCase（baseUrl / apiKey / models.high / providerId / isDefault） | 新增 |
| serde 线格式 | 异常 | 非法 engine 串（"yolo"）反序列化 Err（受控值域拒绝，与既有三枚举线格式口径同型） | 新增 |

#### Mock策略

<!-- 无 mock：内存构造 + native_model 封装内存往返，无 IO 无进程边界（既有 model_test 口径）；不重复验证 serde 自身语义。 -->

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 不 mock：内存构造与编解码往返无进程边界依赖 | 全部 describe |

### packages/desktop/src-tauri/crates/infra/store/src/store.rs -> packages/desktop/src-tauri/crates/infra/store/src/store_test.rs

<!-- AC-4 / AC-5 / AC-10 承载章节；AC-3 的 sdk 必填校验单点与 AC-2 的重名查重半边回溯本章节。
     组合用例（全局库单库贯通）挂靠本章节：两新记录与 WorkspaceRecord 同住全局库，真实组合
     Store + 三模型（不经 mock），不设独立组合测试区。既有断言更新面：全局组注册清单从
     ["workspace"] 扩为三模型——open_global 注册分组断言、组合链分流断言、list_models 分维度
     计数断言三处期望随 global_models() 注册扩展改写（废弃行落点）。 -->

#### 待测功能

- Store.upsert_agent_provider(): id=0 新建（事务内 name 查重 + max+1 分配）/ id>0 整行替换（查重排除自身）；name 空白 `Err`；返回落库记录
- Store.remove_agent_provider(): 被 agent 引用 → `Err`（含引用方 name 提示，不级联不删除）；miss 幂等 `Ok(false)`
- Store.list_agent_providers(): 主键 id 升序自然序（稳定可复现）
- Store.find_agent_provider(): 主键直查（解析单点消费 + save 回填原值）
- Store.upsert_agent_instance(): 新建 / 整行替换 + name 查重；`engine == Sdk` 时 provider_id 必填且引用的 provider 必须存在（缺失 / 悬空均 `Err`）；新建臂强制 `is_default=false`、更新臂保留存量标记
- Store.remove_agent_instance(): 默认 agent 同事务清标记后删（无顺延）；miss 幂等 `Ok(false)`
- Store.list_agent_instances(): 主键 id 升序自然序
- Store.find_agent_instance(): 主键直查（显式路径解析）
- Store.default_agent_instance(): 缺省路径解析入口（清单扫 `is_default`，恒零或一）
- Store.set_default_agent_instance(): 标记即切换：单写事务内清全部既有默认 → 置目标；miss `Err`；返回更新后记录

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| provider upsert：新建与查重（AC-2） | 正向 | id=0 新建：写事务内 max+1 分配 id 从 1 起、返回落库记录、list_agent_providers 可读同记录 | 新增 |
| provider upsert：新建与查重（AC-2） | 异常 | 同 name 重复新建 Err（单写事务内查重，无 TOCTOU）且清单不产生第二条（失败无副作用） | 新增 |
| provider upsert：新建与查重（AC-2） | 异常 | name 空串 / 纯空白 Err（空白校验拒绝）；api_key 空串允许落库（新建无原值可保） | 新增 |
| provider upsert：更新整行替换 | 正向 | id>0 更新 base_url / api_key / models 后 id 不变、整行替换生效；查重排除自身（保留原名更新其他字段不误报重名） | 新增 |
| provider upsert：更新整行替换 | 异常 | 更新为他人已占 name Err 且两条记录均原样（不产生半更新） | 新增 |
| provider id 分配 | 边界 | 连续新建 id 严格递增；删除最大 id 记录后新建复用该槽位（max+1 口径、无永久计数器，与 begin_agent_run 同型） | 新增 |
| remove_agent_provider：引用阻止（AC-5） | 异常 | 被 agent 引用时 Err（错误串含引用方 name 提示）且 provider 与 agent 两类记录均原样保留（不级联不静默删除） | 新增 |
| remove_agent_provider | 正向 | 未被引用命中删除 Ok(true)、清单不再含该项 | 新增 |
| remove_agent_provider | 边界 | miss 幂等 Ok(false)、库内容不变 | 新增 |
| provider 清单与直查 | 正向 | list_agent_providers 主键 id 升序自然序（与添加顺序无关、两次调用序稳定确定） | 新增 |
| provider 清单与直查 | 边界 | 空库 list 返回空向量；find_agent_provider 对不存在 / 0 / 负数 / i64::MAX id 均返回 None 不报错 | 新增 |
| agent upsert：engine 约束（AC-3） | 正向 | cli 引擎 provider 可空（provider_id=None 保存成功）；sdk 引擎选存量 provider 保存成功 | 新增 |
| agent upsert：engine 约束（AC-3） | 异常 | sdk 引擎 provider_id=None Err；sdk 悬空引用（provider_id 指向不存在的 provider id）Err | 新增 |
| agent upsert：name 查重 | 异常 | 同 name 重复新建 / 更新撞名 Err；name 空白 Err | 新增 |
| agent upsert：默认标记写口（定夺 5） | 边界 | 新建臂入参 is_default=true 被强制落 false（入参标记不参与写，不变式免受前端入参影响）；更新臂保留存量标记（默认 agent 改名 / 换 provider 后仍为默认） | 新增 |
| remove_agent_instance：删默认清标记（AC-5） | 正向 | 删默认 agent Ok(true) 后 default_agent_instance 返回 None、清单少一行（同事务清标记，无顺延） | 新增 |
| remove_agent_instance | 边界 | miss 幂等 Ok(false)；删非默认 agent 后既有默认不受影响 | 新增 |
| default_agent_instance | 正向 | 有默认返回 Some、无默认返回 None（清单扫 is_default，恒零或一） | 新增 |
| set_default_agent_instance：标记即切换（AC-4） | 正向 | A→B 切换后全局恰一默认为 B、A 标记自动清除；返回更新后记录（is_default=true） | 新增 |
| set_default_agent_instance：标记即切换（AC-4） | 边界 | 重复标记同一 agent 幂等（仍恰一默认）；切换后全清单 is_default 恰一为 true | 新增 |
| set_default_agent_instance | 异常 | miss id Err 且既有默认标记不变 | 新增 |
| 存量库 additive 打开（AC-10） | 正向 | 预置仅注册 WorkspaceRecord 并写入一条记录的存量全局库（native_db 裸构造，不经 Store）：新代码 open_global 成功、原记录原样可读、两新模型可写入读出；无任何迁移代码路径 | 新增 |
| 全局组模型注册 | 正向 | open_global 后 list_models 出现 agent_provider / agent_instance / workspace 三行（计数 0 也列出）；open_workspace 组零变化（两组无交叉注册） | 新增 |
| 全局组模型注册 | 边界 | 跨维度不可达：workspace 库 scan("agent_provider") 与全局库 scan("agent_run") 均「未知模型」Err（模型分组使混入在打开点不可能） | 新增 |
| 全局库单库贯通（组合） | 正向 | 组合链：provider 新建 → sdk agent 新建（引用）→ set_default → default_agent_instance 解析命中 → drop 重开同一库文件后全部状态一致（三模型同库共存互不干扰） | 新增 |
| 全局组注册清单旧断言退役 | 废弃 | 既有「open_global 仅列 workspace 一行」「组合链全局库仅 ("workspace", 1)」「list_models 分维度计数全局组单行」三处断言随 global_models() 追加两注册退役改写为三模型组期望 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 文件系统 / db 文件（进程边界） | 不 mock：tempfile TempDir 真实开库（存储层不 mock 既有惯例）；存量库 additive 预置以 native_db 裸 Models 构造仅 WorkspaceRecord 的库文件写盘（fixture 构造，非 mock） | 全部 describe |
| 系统时钟（全局变量） | 不 mock：两管理记录不设时间戳字段，清单排序与钟面无关 | 全部 describe |

### packages/desktop/src-tauri/crates/infra/store/src/lib.rs -> packages/desktop/src-tauri/crates/infra/store/src/lib_test.rs

<!-- design.md 未声明该文件的公共 API 变更：导出面为既有 pub use 追加四类型
     （AgentProviderRecord / AgentInstanceRecord / AgentModelTiers / AgentEngineKind）。
     纯再导出面零运行时行为，不应创建 lib_test.rs（零用例测试文件）；导出正确性由消费方
     模块用例编译期承载（store_test / agent_test / 各命令测试均经 crate 根路径取用新类型）。
     详见不可测试项 7。 -->

#### 待测功能

<!-- design.md 公共函数 / API 表对 lib.rs 零声明（纯再导出面）；不设条目。 -->

#### 用例

<!-- 不设用例；不应创建 lib_test.rs（防空套件），见不可测试项 7。 -->

#### Mock策略

<!-- 不适用：零运行时行为，无进程边界。 -->

### packages/desktop/src-tauri/crates/infra/store/src/envelope.rs -> packages/desktop/src-tauri/crates/infra/store/src/envelope_test.rs

<!-- AC-11 承载章节。design.md 未声明该文件的公共函数（MODEL_ENTRIES 为 fn-pointer
     静态注册表，新增 scan_* / *_key 为私有数据行函数）：登记行为断言经 Store::list_models /
     Store::scan 公共面在真实组合下承载（信封实现体经公共 API 委托触达的既有惯例），
     待测功能不虚构条目。既有「注册表按维度分组列出」用例的全局组期望随两登记行更新。 -->

#### 待测功能

<!-- design.md 公共函数 / API 表对 envelope.rs 零声明（注册表数据行与私有 fn 不列）；断言经 store.rs 公共面用例承载，不虚构条目。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 新模型登记行（AC-11） | 正向 | open_global 后 list_models 出现 agent_provider / agent_instance 两行（user 维度信封注册）；写入后 list_models 计数与实有记录数一致 | 新增 |
| 新模型登记行（AC-11） | 正向 | scan("agent_provider") / scan("agent_instance") 分页扫描：主键自然序、offset/limit 翻页拼接不重不漏；key 为数值 id 信封、value 为 camelCase JSON（db-inspector 查看器零改动触达前提） | 新增 |
| 新模型登记行（AC-11） | 边界 | 空库计数 0 也列出；信封 API 签名零变化（list_models / scan 形参面与返回形态不变，仅注册表数据行追加） | 新增 |
| 新模型登记行（AC-11） | 异常 | workspace 库实例 scan 新模型名 Err（维度过滤——两新模型仅注册全局组） | 新增 |
| 全局组清单旧断言退役 | 废弃 | 既有「注册表按维度分组列出模型且 list_models 计数与写入量一致」用例的全局组期望 [("workspace", 1)] 随两登记行更新为三模型组 | 废弃 |

#### Mock策略

<!-- 不 mock：tempfile 真开库并经 Store 公共 API 写入构造数据（既有口径）；存储层不 mock。 -->

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 文件系统 / db 文件（进程边界） | 不 mock：tempfile 真开全局库 / workspace 库，记录经真实写入构造扫描库存与计数 | 全部 describe |

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/config.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/config_test.rs

<!-- 换源零改动承诺的构造面章节：from_hardcoded_slot() 退役与 EngineConfig::empty() 更替。
     既有三处 from_hardcoded_slot 引用用例（硬编码位保真、空缺省 → ConfigMissing、重复调用
     等值）随构造删除退役，断言改写为 empty() 构造源；字段面与 is_complete 零变化由改写后
     用例继续锚定。 -->

#### 待测功能

- EngineConfig.empty(): 三字段全空占位（CLI 臂不消费）；替代退役的 from_hardcoded_slot()
- EngineConfig.from_hardcoded_slot(): 退役（删除）：硬编码预留位由管理数据解析取代

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| EngineConfig::empty 构造 | 正向 | empty() 返回 api_key / base_url / model 三字段全空串（空缺省形状锁定，防实现引入隐式缺省） | 新增 |
| EngineConfig::empty 构造 | 边界 | 重复调用返回等值结构（纯构造无隐藏状态）；字段含空格 / 中文 / emoji / 换行的手填构造与 Clone / PartialEq 保真（既有断言换构造源后保留） | 新增 |
| 空配置 → ConfigMissing 前提 | 异常 | empty() 经 is_complete 判定缺失，SdkRunner start 以 ConfigMissing 显式失败且文案区分缺失字段（api_key / base_url / model 三字段名在场）——sdk 配置不齐错误口径不因换源漂移 | 新增 |
| 手填齐备形态 | 正向 | 解析单点产物形态（三字段齐备，model 取 provider high 档）is_complete 判定翻正 | 新增 |
| from_hardcoded_slot 退役 | 废弃 | 「from_hardcoded_slot 返回三字段结构体且手填值逐字段保真」「未手填缺省形态经 is_complete 判定为缺失且 sdk 启动以 config_missing 失败」「重复调用返回等值结构且特殊字符字段构造保真」三处引用用例随构造删除退役，断言源改写为 empty() | 废弃 |

#### Mock策略

<!-- 无 mock：纯结构体构造 + 门面分发既有口径（SdkRunner 空配置 start 为进程内校验失败）。 -->

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 不 mock：构造与 is_complete 判定为纯函数；SdkRunner 空配置 start 为进程内校验失败（既有用例形态） | 全部 describe |

### packages/desktop/src-tauri/src/commands/agents/mod.rs -> packages/desktop/src-tauri/src/commands/agents/mod_test.rs

<!-- AC-2 / AC-3 承载章节（命令链组合挂靠：管理命令为全局库链路入口）；AC-4 / AC-5 / AC-9 的
     命令面半边回溯本章节。装置沿 workspaces/mod_test.rs 既有先例：tauri::test::mock_app()
     manage 真实 WorkspaceStores（tempdir 真开全局库），#[tauri::command] 保留原函数可直调，
     不启动真实 Tauri runtime。组合链（save → list → set_default → default 解析 → 重开一致）
     在本章节承载，不设独立组合测试区。 -->

#### 待测功能

- list_agent_providers(): 全局库实例（global()）薄包装：清单 id 升序
- save_agent_provider(): id None 新建 / Some 更新；参数转换段：id 存在且 api_key 为空 → 读存量记录回填原值（留空 = 保持原值），store 恒收全字段
- delete_agent_provider(): 引用阻止 `Err` / miss 幂等 `Ok(false)`
- list_agent_instances(): 同 list 模板
- save_agent_instance(): sdk 缺 provider 校验在 store 单点透传
- delete_agent_instance(): 默认 agent 删后清标记（store 同事务）
- set_default_agent_instance(): 标记即切换，返回更新后记录

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 管理命令 → 全局库持久化（AC-2 组合） | 正向 | 组合链：save_agent_provider(None) 新建 → list_agent_providers 清单呈现 → save_agent_provider(Some) 更新 → delete_agent_provider 删除 → WorkspaceStores 重开后状态一致（命令面经 global() 真实组合 store crate） | 新增 |
| save_agent_provider 留空 key 回填（AC-2） | 正向 | id 存在且入参 api_key 空串 → 读存量记录回填原值：落库 key 与存量逐字段一致（留空 = 保持原值） | 新增 |
| save_agent_provider 留空 key 回填（AC-2） | 边界 | id 存在且 api_key 非空 → 直接落新值（显式换 key 不误回填）；id=None 新建传空 key → 空值落库（无原值可保） | 新增 |
| provider 命令校验与删除语义（AC-2/AC-5） | 异常 | 重名保存 reject `Err` 且清单不变；更新 miss id `Err`；name 空白 `Err` | 新增 |
| provider 命令校验与删除语义（AC-2/AC-5） | 边界 | delete_agent_provider 命中 Ok(true) / 被引用 Err 透传 / miss Ok(false)；list_agent_providers 空库空数组 | 新增 |
| agent 命令 engine 约束（AC-3） | 正向 | save_agent_instance cli 新建 provider 可空 Ok；sdk 选存量 provider Ok | 新增 |
| agent 命令 engine 约束（AC-3） | 异常 | sdk 缺 provider / 悬空引用 reject（store 校验单点透传，命令层不加重复校验） | 新增 |
| agent 命令删除与默认（AC-4/AC-5） | 正向 | delete_agent_instance 删默认 agent → default_agent_instance 返回 None；set_default_agent_instance 标记即切换、返回更新后记录，list 复核全局恰一默认 | 新增 |
| agent 命令删除与默认（AC-4/AC-5） | 异常 | set_default_agent_instance miss id Err；delete_agent_instance miss Ok(false) | 新增 |
| 错误串机密边界（AC-9） | 边界 | 全部 Err(String) 错误串（重名 / 引用阻止 / 回填 miss / sdk 校验）不含 api_key 明文——错误只含 name / 计数语境 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| Tauri 运行环境 | tauri::test::mock_app()（MockRuntime，无窗口无事件循环）manage 真实 WorkspaceStores（tempdir 真开全局库）后经 app.state 取 State；沿用 workspaces/mod_test.rs 既有装置模式 | 全部 describe |
| 文件系统 / db 文件（进程边界） | 不 mock：tempfile 真开库；引用阻止场景以真实 agent 记录引用构造 | 全部 describe |

### packages/desktop/src-tauri/src/commands/mod.rs -> packages/desktop/src-tauri/src/commands/mod_test.rs

<!-- design.md 未声明该文件的公共 API 变更：`pub mod agents;` + `all_commands!` 宏清单追加
     七命令路径（24 → 31 条）。纯注册导线，不应创建 commands/mod_test.rs：注册正确性由
     bindings/mod_test.rs 快照（export-bindings 两侧自动同步）与七命令各自用例间接承载。
     详见不可测试项 8。 -->

#### 待测功能

<!-- design.md 公共函数 / API 表对 commands/mod.rs 零声明（宏清单注册面）；不虚构条目。 -->

#### 用例

<!-- 不设用例；不应创建 commands/mod_test.rs，注册面由 bindings/mod_test.rs 快照与各命令用例间接承载，见不可测试项 8。 -->

#### Mock策略

<!-- 不适用：零运行时行为。 -->

### packages/desktop/src-tauri/src/commands/exec/mod.rs -> packages/desktop/src-tauri/src/commands/exec/mod_test.rs

<!-- AC-7 的命令入口半边。agent_start 尾参 engine: Option<EngineKind> → agent: Option<i64>；
     解析 Err（无默认 / agent 不存在）抵达前端 reject。既有 engine 收敛与 serde 面两组用例随
     DEFAULT_ENGINE 退役删除（废弃行落点）；blank root、for_root 落库、提前 resolve 等既有
     回归零变化继续执行（不新增用例行，装置复用 agent_test.rs 假 runner）。 -->

#### 待测功能

- agent_start(): 尾参 `engine: Option<EngineKind>` → `agent: Option<i64>`；缺省（None）= 解析默认 agent，解析 `Err`（无默认 / agent 不存在）reject 前端；`Result<T, String>` 模板不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| agent_start 缺省解析（AC-7） | 正向 | 不传 agent（explore 形态）且全局库存在默认 agent → 命令体经 resolve_agent_engine 解析默认并按解析产物发起（经编排缝以假 runner 观测引擎形态） | 新增 |
| agent_start 缺省解析（AC-7） | 异常 | 无默认 agent → Err 引导管理页文案 reject 前端：零落库零推送，不静默回退硬编码引擎（MUST NOT 回退口径） | 新增 |
| agent_start 显式 agent（AC-8） | 正向 | agent=存量 id → 按该 agent 引擎形态发起（cli / sdk 两形态各验一轮） | 新增 |
| agent_start 显式 agent（AC-8） | 异常 | agent=不存在 id → Err reject，零落库零推送 | 新增 |
| agent 尾参 serde 面 | 正向 | Option\<i64\> 缺席承接 None、数值承接 Some、非数值 / 浮点拒绝（守卫先于分发，既有 serde 面用例形态平移到 i64 值域） | 新增 |
| engine 尾参面退役 | 废弃 | 「agent_start_engine 缺省收敛 sdk（DEFAULT_ENGINE unwrap_or 断言）」与「engine 尾部可选参数 serde 面」两组用例随 DEFAULT_ENGINE 退役与尾参语义演进删除 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| Tauri 运行环境 | mock_app() manage WorkspaceStores + RunStopRegistry（须先于 start 托管，沿用既有装置） | 全部 describe |
| agent 引擎子进程（进程边界） | FakeRunner 假 runner 经 start_agent_run_with 泛型缝注入（装置复用 agent_test.rs），captured_params 观测引擎形态 | agent_start 组合用例 |
| Channel（IPC 边界） | capturing_channel 捕获流出消息；异常口径断言零推送 | 缺省解析 / 显式 agent 用例 |
| 文件系统 / db 文件 | 不 mock：tempdir 真开全局库，默认 agent 经全局管理操作面真实写入构造 | 缺省解析用例 |

### packages/desktop/src-tauri/src/commands/exec/agent.rs -> packages/desktop/src-tauri/src/commands/exec/agent_test.rs

<!-- AC-6 / AC-7 承载章节（解析单点）。既有编排时序回归（tee 双 sink、EOF 三分支收敛、dyn 缝、
     引擎中立、信封线格式）零变化继续执行，构成「start_agent_run_with / drive_agent_run 零改动」
     的编译期 + 行为期证据（定夺 9）——不新增用例行；波及面仅两处：DEFAULT_ENGINE 常量用例退役、
     薄入口 PATH 隔离用例尾参形态更新。解析单点新增用例读全局库（WorkspaceStores 装置扩容），
     store 不 mock。 -->

#### 待测功能

- resolve_agent_engine(): 解析单点：None → default_agent_instance（无默认 `Err` 引导管理页）/ Some(id) → find_agent_instance；sdk 由引用 provider 组装 config（model 取 high 档），cli 臂 empty()；AgentEngineKind → EngineKind 两臂映射
- start_agent_run(): 尾参 `engine: EngineKind` → `resolved: ResolvedEngine`；runner_for(resolved.kind, resolved.config)；形参数仍 6 个
- DEFAULT_ENGINE: 退役（删除）：缺省收敛改由解析单点承载

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| resolve 缺省路径（AC-7） | 正向 | None + 全局库存在默认 sdk agent → Ok(ResolvedEngine)：kind=Sdk 且 config 三字段自引用 provider 组装、model 恰取 models.high（AC-6 消费半；medium / low 不入 config） | 新增 |
| resolve 缺省路径（AC-7） | 异常 | None + 无默认 agent → Err 含引导管理页语境文案（不落库不推流不回退硬编码引擎） | 新增 |
| resolve 显式路径（AC-8） | 正向 | Some(id) 命中 cli agent → kind=Cli 且 config=EngineConfig::empty()（CLI 臂不消费占位）；Some(id) 命中 sdk agent → 同缺省路径组装形态 | 新增 |
| resolve 显式路径（AC-8） | 异常 | Some(id) 不存在 → Err（agent 不存在语境） | 新增 |
| resolve 边界与映射 | 边界 | sdk agent 引用的 provider 缺失（绕过 store 校验的悬空数据）→ Err 不 panic；AgentEngineKind::{Cli, Sdk} → EngineKind 两臂同线值（"cli" / "sdk"） | 新增 |
| start_agent_run 尾参换源（定夺 9） | 正向 | resolved（Cli 形态 / Sdk 形态）各经薄入口走全链：runner_for 收解析产物、提前 resolve running、tee 双 sink 与终态流出与既有用例语义一致（装置入参形态同步更新） | 新增 |
| DEFAULT_ENGINE 退役 | 废弃 | 「default_engine 硬编码位恒等 sdk 且与 engine_config 硬编码位同点成对」用例随常量删除退役；「start_agent_run 隔离 PATH 薄入口」用例尾参 engine: EngineKind::Cli 形态更新为 resolved 尾参（断言保留） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| Tauri 运行环境 | mock_app()（MockRuntime）manage 真实 WorkspaceStores 与 RunStopRegistry（既有装置；全局库由 workspace 库直开扩为 WorkspaceStores——解析单点读 global()） | resolve 与薄入口 describe |
| agent 引擎子进程（进程边界） | FakeRunner 假 runner（预录事件 / 可控启动失败）经泛型缝 / dyn 缝注入；captured_params 观测引擎入参 | 薄入口与编排回归 describe |
| Channel（IPC 边界） | capturing_channel / failing_channel 捕获与失败注入（页面已关） | tee 双 sink 与终态流出断言 |
| 文件系统 / db 文件 | 不 mock：tempdir 真开全局库，provider / agent 记录经管理操作面真实写入构造 | resolve 全部 describe |
| PATH 环境变量（运行环境） | 隔离 PATH 触发薄入口失败分支；修改以共享互斥锁（PATH_LOCK）串行化 | PATH 隔离用例 |

### packages/desktop/src/views/agents/agents-view.tsx -> packages/desktop/src/views/agents/agents-view.test.tsx

<!-- AC-1 承载章节（页面两栏主体）。AgentsView 为无 props 组合根：真实组合两取数 hook 与两
     panel（hook 内部协作者不 mock，经 invoke 面驱动）；路由表注册半边（routes.tsx）不在测试
     配置 scope（见不可测试项 4），URL→视图映射由本章节页面渲染断言与 app-sidebar.test.tsx
     的 nav-agents active 断言两端夹持。 -->

#### 待测功能

- AgentsView(): 无 props（全局语义，不依赖 root）；组合根：挂两取数 hook 后分别下发两 panel

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| AgentsView 两栏骨架（AC-1） | 正向 | 渲染 Providers / Agents 两栏；两 hook 挂载取数回填后清单呈现记录（ProviderPanel / AgentPanel 真实组合） | 新增 |
| AgentsView 两栏骨架（AC-1） | 边界 | 两清单均空 → 两栏空态渲染不崩；页面不触发任何 workspace 命令（invoke 面仅 list_agent_providers / list_agent_instances） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC 进程边界（@tauri-apps/api/core） | invoke 按命令名分发替身（list_agent_providers / list_agent_instances 固定 fixture；数组形态防 null 崩溃），记录调用序列断言命令面 | 全部 describe |
| sonner toast（第三方 spy） | vi.mock sonner 以 vi.fn 替身注入 toast.error（动作轨错误呈现断言） | 动作失败呈现用例 |

### packages/desktop/src/views/agents/components/provider-panel.tsx -> packages/desktop/src/views/agents/components/provider-panel.test.tsx

<!-- 回溯 AC-2 / AC-5 / AC-9 的前端呈现半边。state（AgentProvidersState）为被测组件显式入参，
     动作轨道以 vi.fn() 注入（入参例外）；MaskedApiKey 内部协作者真实组合；组件不触 invoke
     （取数收口在 hook 层）。 -->

#### 待测功能

- ProviderPanel(props: { state: AgentProvidersState }): Providers 栏：清单（name / base_url / 三档 model / 遮蔽展示）+ 新建 / 编辑表单（api_key 编辑态恒空 + 遮蔽占位留空保持原值）+ 删除（引用阻止 `Err` 呈现，清单不变）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| ProviderPanel 清单呈现（AC-9 前端半） | 正向 | 清单渲染 name / base_url / models 三档 / 遮蔽 api_key（MaskedApiKey 真实组合，呈现 sk-***abc 形态） | 新增 |
| ProviderPanel 表单与提交（AC-2 前端半） | 正向 | 新建表单提交 → state.save 以 (None, name, base_url, api_key, models 三档) 恰调用一次；编辑态 api_key 输入框恒空 + 遮蔽占位提示留空保持原值，提交 api_key 空串（回填语义由命令层承载） | 新增 |
| ProviderPanel 删除与错误呈现（AC-5 前端半） | 异常 | 删除被引用 provider → state.remove reject 的 Err 呈现且清单不变 | 新增 |
| ProviderPanel 表单校验 | 边界 | name 空白提交被阻；超长（>1000 字符）name / base_url 与特殊字符原样入参；models 三档全空可提交（模型层无校验，前端不抢做） | 新增 |
| ProviderPanel 空态 | 边界 | 空清单空态渲染；loading / error 态呈现不变形 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| state 动作轨道（入参例外） | AgentProvidersState 以 fixture + vi.fn() 动作（save / remove 可编程 resolve / reject）显式入参注入 | 全部 describe |
| 无 IPC | 不触 @tauri-apps/api/core（取数收口在 use-agent-providers，由其章节承载） | 全部 describe |

### packages/desktop/src/views/agents/components/agent-panel.tsx -> packages/desktop/src/views/agents/components/agent-panel.test.tsx

<!-- 回溯 AC-3 / AC-4 / AC-5 的前端呈现半边。state 与 providers 均为显式入参注入；
     sdk 必填校验单点在 store（前端不抢做校验，仅呈现后端 Err）。 -->

#### 待测功能

- AgentPanel(props: { state: AgentInstancesState; providers: AgentProviderRecord[] }): Agents 栏：清单（name / engine / 引用 provider 名 / 默认标记）+ 新建 / 编辑表单（engine 二值 + provider 选择，sdk 必填）+ 默认标记切换 + 删除

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| AgentPanel 清单呈现 | 正向 | 清单渲染 name / engine 二值 / 引用 provider 名（经 providers 清单按 id 解析）/ 默认标记 | 新增 |
| AgentPanel 表单与提交（AC-3 前端半） | 正向 | cli agent provider 可空提交 → state.save 以 (None, name, "cli", null) 恰调用一次；sdk agent 选 provider 提交 → save 携 providerId | 新增 |
| AgentPanel 表单与提交（AC-3 前端半） | 异常 | sdk 未选 provider 提交 → 后端 Err 呈现且清单不变（校验单点在 store，前端仅呈现） | 新增 |
| AgentPanel 默认标记切换（AC-4 前端半） | 正向 | 切换默认 → state.setDefault 以目标 id 恰调用一次；成功后清单刷新呈现唯一默认标记 | 新增 |
| AgentPanel 删除 | 边界 | 删除 agent → state.remove 调用；默认 agent 删除后清单刷新无默认标记 | 新增 |
| AgentPanel 边界 | 边界 | providers 清单为空时 provider 下拉空态、sdk 提交走后端报错呈现；engine 下拉仅 cli / sdk 二值 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| state 动作轨道（入参例外） | AgentInstancesState 以 fixture + vi.fn() 动作（save / remove / setDefault 可编程 resolve / reject）显式入参注入 | 全部 describe |
| providers fixture（入参例外） | AgentProviderRecord[] 显式入参注入（含命中 / 缺失引用两形态） | 清单 provider 名解析用例 |
| 无 IPC | 不触 @tauri-apps/api/core | 全部 describe |

### packages/desktop/src/views/agents/components/masked-api-key.tsx -> packages/desktop/src/views/agents/components/masked-api-key.test.tsx

<!-- 回溯 AC-9 的前端遮蔽展示半边。纯展示组件，无进程边界无 mock；遮蔽口径（末 3 字符、
     空与过短恒 sk-***）为 design 语义约束（形态已钉死，图标级微调不涉本组件）。 -->

#### 待测功能

- MaskedApiKey(props: { apiKey: string }): 遮蔽展示：末 3 字符 + `sk-***` 前缀；空 / 过短恒 `sk-***`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| MaskedApiKey 遮蔽形态（AC-9） | 正向 | 常规 key → `sk-***` 前缀 + 末 3 字符（sk-***abc 形态）；渲染输出不含明文全文 | 新增 |
| MaskedApiKey 遮蔽形态（AC-9） | 边界 | 空串 / 长度 ≤3（含恰 3 字符）→ 恒 `sk-***`（不回显原值，防 `sk-***abc` 恰为原文泄露） | 新增 |
| MaskedApiKey 遮蔽形态（AC-9） | 边界 | 超长（>1000 字符）/ emoji / 多字节 key 末 3 字符按 char 粒度截取不悬挂 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 不 mock：纯展示组件，无进程边界 | 全部 describe |

### packages/desktop/src/views/agents/hooks/use-agent-providers.ts -> packages/desktop/src/views/agents/hooks/use-agent-providers.test.ts

<!-- 取数收口 hook（挂载一次 + 动作轨道 + 双轨错误）。错误双轨口径沿 use-workspaces 既有
     惯例：清单加载失败置 inline error 态（查询轨）；save / remove 失败 toast 固定前缀且
     不置 error 态（动作轨）。 -->

#### 待测功能

- useAgentProviders(): AgentProvidersState { providers, loading, error, save, remove }：挂载 invoke list_agent_providers 一次 + 动作轨道（显式动作刷新，无轮询）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| useAgentProviders 取数收口 | 正向 | 挂载 invoke list_agent_providers 恰一次，providers 态回填、loading 收敛 | 新增 |
| useAgentProviders 取数收口 | 边界 | 无轮询：取数链稳定判据（连续轮询调用数不再增长） | 新增 |
| useAgentProviders 动作轨道 | 正向 | save → invoke save_agent_provider 后清单刷新；remove → invoke delete_agent_provider 后清单刷新 | 新增 |
| useAgentProviders 动作轨道 | 异常 | save / remove reject → toast.error 固定前缀调用且不置 error 态（动作轨 toast 呈现） | 新增 |
| useAgentProviders 加载失败 | 异常 | 清单加载 reject → error 态置位（查询轨 inline 呈现，无 toast） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC 进程边界（@tauri-apps/api/core） | invoke 按命令名分发替身（可切换 resolve / reject 并记录调用序列），沿 use-workspaces.test.ts 既有替身模式 | 全部 describe |
| sonner toast（第三方 spy） | vi.mock sonner 以 vi.fn 替身注入 toast.error | 动作轨错误断言 |

### packages/desktop/src/views/agents/hooks/use-agent-instances.ts -> packages/desktop/src/views/agents/hooks/use-agent-instances.test.ts

<!-- 取数收口 hook（同 provider 侧 + setDefault 动作）。setDefault 成功后刷新清单（标记即
     切换的呈现面）。 -->

#### 待测功能

- useAgentInstances(): AgentInstancesState { instances, loading, error, save, remove, setDefault }：挂载 invoke list_agent_instances 一次 + 动作轨道（含 setDefault）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| useAgentInstances 取数收口 | 正向 | 挂载 invoke list_agent_instances 恰一次，instances 态回填；无轮询（调用数稳定） | 新增 |
| useAgentInstances setDefault（AC-4 前端半） | 正向 | setDefault → invoke set_default_agent_instance 成功后清单刷新（唯一默认标记呈现） | 新增 |
| useAgentInstances 动作轨道 | 正向 | save / remove → 对应 invoke 后清单刷新 | 新增 |
| useAgentInstances 动作轨道 | 异常 | save / remove / setDefault reject → toast.error 固定前缀且不置 error 态 | 新增 |
| useAgentInstances 加载失败 | 异常 | 清单加载 reject → error 态置位（查询轨 inline 呈现） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC 进程边界（@tauri-apps/api/core） | invoke 按命令名分发替身（可切换 resolve / reject 并记录调用序列），沿既有替身模式 | 全部 describe |
| sonner toast（第三方 spy） | vi.mock sonner 以 vi.fn 替身注入 toast.error | 动作轨错误断言 |

### packages/desktop/src/views/agent/hooks/use-agent-options.ts -> packages/desktop/src/views/agent/hooks/use-agent-options.test.ts

<!-- 回溯 AC-8 前置取数（调试页选择器数据面）。只读取数：挂载一次，无动作轨道、无轮询；
     defaultId 派生自 is_default 记录（无默认回 null）。 -->

#### 待测功能

- useAgentOptions(): AgentOptionsState { instances, loading, defaultId }：调试页 agent 选择器取数（挂载一次，无轮询）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| useAgentOptions 选择器取数（AC-8 前置） | 正向 | 挂载 invoke list_agent_instances 恰一次；defaultId = is_default 记录 id | 新增 |
| useAgentOptions 选择器取数（AC-8 前置） | 边界 | 无默认记录 → defaultId null；空清单 → instances [] + defaultId null；无轮询（调用数稳定） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC 进程边界（@tauri-apps/api/core） | invoke 按命令名分发替身（list_agent_instances 固定 fixture，含 / 不含 is_default 两形态） | 全部 describe |

### packages/desktop/src/views/agent/components/agent-run-form.tsx -> packages/desktop/src/views/agent/components/agent-run-form.test.tsx

<!-- AC-8 承载章节。engine 下拉（ENGINE_OPTIONS 三用例）随引擎直选面退役；agent 选择器
     选项 = agent 实例清单、默认选中默认 agent。既有 permission-mode 与 prompt 必填、
     disabled 禁用、超长 prompt 保真等用例零变化继续执行（mount 装置签名随 agents prop
     演进），不新增用例行。 -->

#### 待测功能

- AgentRunForm(props: { disabled: boolean; agents: AgentInstanceRecord[]; onStart: (input: AgentStartInput) => void }): engine 下拉退役 → agent 选择器（`data-testid="agent-select"`，默认选中默认 agent）；AgentStartInput.engine 字段退役 → agent: number \| null

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| AgentRunForm agent 选择器（AC-8） | 正向 | agents 清单渲染 agent-select 选项（按记录 name 标识）；默认选中默认 agent（is_default 记录） | 新增 |
| AgentRunForm agent 选择器（AC-8） | 正向 | 显式选择 agent 提交 → onStart 以 { prompt, permissionMode, agent: id } 恰调用一次 | 新增 |
| AgentRunForm agent 选择器 | 边界 | agents 空清单 → 选择器空态仍可发起：onStart agent=null（走后端缺省解析，报错口径由后端承载） | 新增 |
| AgentRunForm agent 选择器 | 边界 | 不显式选择提交 → onStart agent=null（explore 同型缺省语义） | 新增 |
| engine 下拉退役 | 废弃 | 「初始 agent-engine 下拉值为 sdk」「切至 cli 后 onStart engine: cli」「注入清单外 option 值（yolo）」三用例随 ENGINE_OPTIONS 与 engine 字段退役删除 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| onStart 回调（入参例外） | vi.fn() 注入捕获 AgentStartInput（既有装置，mount 签名随 agents prop 演进） | 全部 describe |
| 无 IPC | 纯回调组件无进程边界（取数在 use-agent-options，由其章节承载） | 全部 describe |

### packages/desktop/src/views/agent/agent-debug-view.tsx -> packages/desktop/src/views/agent/agent-debug-view.test.tsx

<!-- 回溯 AC-8 接线半边：挂 useAgentOptions() 取清单透传 agents prop 给 AgentRunForm。
     design.md 公共函数表对该文件零声明（页面组件未列）；透传行为以一例锚定，既有时间线 /
     原始流 / 历史区 / 停止入口用例零变化继续执行（不新增用例行）。 -->

#### 待测功能

<!-- design.md 公共函数 / API 表对 agent-debug-view.tsx 零声明（AgentDebugView 未列）；透传接线经下例锚定，不虚构条目。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 调试页 agent 清单接线（回溯 AC-8） | 正向 | 挂载经 useAgentOptions 取清单（真实组合 hook，经 invoke 替身驱动），AgentRunForm 收到 agents prop 并呈现 agent-select | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC 进程边界（@tauri-apps/api/core） | 既有 invoke 按命令名分发替身沿用（list_agent_instances 分支回数组形态） | 接线用例 |
| AgentRunForm | 不 mock：内部协作者真实组合 | 接线用例 |

### packages/desktop/src/lib/agent-transport.ts -> packages/desktop/src/lib/agent-transport.test.ts

<!-- 回溯 AC-7 / AC-8 的 invoke 入参面半边：readEngine → readAgentId（number \| null 运行时
     校验）、invoke 尾参 engine → agent。design.md 公共函数表对该文件零声明（transport 对外
     签名零变化）；既有 engine 键读取四用例退役、agent 键用例平移新增。 -->

#### 待测功能

<!-- design.md 公共函数 / API 表对 agent-transport.ts 零声明（对外签名零变化，仅 body 键与运行时校验内部演进）；经下列用例锚定 invoke 入参面。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| transport agent 尾参翻译（回溯 AC-7/AC-8） | 正向 | body.agent 数值 → invoke 尾参 agent 携该 id（readAgentId 运行时校验通过，AgentStartArgs\[8\] 位同步） | 新增 |
| transport agent 尾参翻译 | 边界 | body.agent 缺席 / 显式 null → agent=null 传递不抛错（缺省语义透传，explore 链零改动） | 新增 |
| transport agent 尾参翻译 | 异常 | body.agent 非数值（"yolo"）→ 「非法 agent」参数校验拒绝且不发起 invoke（对齐 permissionMode 校验口径） | 新增 |
| engine 尾参面退役 | 废弃 | readEngine 的「engine="sdk"/"cli" 读取透传」「缺席承接 null」「显式 null 同口径」「清单外值拒绝」四用例随 agent 尾参演进而退役 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC 进程边界（@tauri-apps/api/core） | invoke 替身记录调用序列（startCallArgs 断言尾参键名与值）；Channel 以可编程 class 捕获（既有装置） | 全部 describe |

### packages/desktop/src/hooks/use-agent-chat.ts -> packages/desktop/src/hooks/use-agent-chat.test.ts

<!-- 回溯 AC-7 的 explore 缺省半边（hook 不传 agent，body.agent=null 缺席透传）与调试链显式
     agent 穿透。design.md 公共函数表对该文件零声明（hook 对外签名零变化）；既有 body.engine
     穿透用例退役、agent 键用例新增；resumeSessionId / parentRunId / 来源三元组组装互不串线
     断言保留（键名断言同步更新）。 -->

#### 待测功能

<!-- design.md 公共函数 / API 表对 use-agent-chat.ts 零声明（对外签名零变化，仅 body 键演进）；经下列用例锚定 body 组装面。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| useAgentChat agent 穿透（回溯 AC-8） | 正向 | sendMessage input.agent=id → body.agent=id（invoke 入参断言；调试链显式选择穿透） | 新增 |
| useAgentChat agent 穿透（AC-7 缺省半） | 边界 | explore 形态不传 agent → body.agent=null 缺席透传不注入（缺省解析语义由后端解析单点承载，前端零回退） | 新增 |
| useAgentChat agent 穿透 | 异常 | 类型面外运行时异常值（"yolo"）原样入 body，由 transport readAgentId 层拒绝（职责分界留痕，error 含「非法 agent」） | 新增 |
| engine 穿透退役 | 废弃 | 「发送组装 engine 穿透」describe 五用例（sdk / cli / 缺席 / yolo / 互不串线键名）随 agent 键演进而退役；互不串线断言以 agent 键名重建保留 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC 进程边界 | invoke / transport 替身记录调用序列（startCallArgs 断言 body 键）；镜像语义（agent-adapter）真实参与，不 mock ai 的 useChat（既有口径） | 全部 describe |

### packages/desktop/src/components/app-sidebar.tsx -> packages/desktop/src/components/app-sidebar.test.tsx

<!-- 回溯 AC-1 的侧栏入口半边：系统工具组新增 [Agent 管理]（nav-agents，active 由
     pathname === '/agents' 派生，lucide 图标为实现可微调项不锁定）。design.md 公共函数表
     对该文件零声明（组件签名不变，仅导航项数据面追加与组内排序）；新增两例锚定入口面，
     既有页面组与 workspace 组回归零变化继续执行。 -->

#### 待测功能

<!-- design.md 公共函数 / API 表对 app-sidebar.tsx 零声明（AppSidebar 签名不变，仅导航项数据面追加）；经下列用例锚定入口面。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 系统工具组 [Agent 管理] 入口（回溯 AC-1） | 正向 | nav-agents 项在「系统工具」组渲染；/agents 路径下 active 态成立、他路径下不成立（active 由 URL 派生） | 新增 |
| 系统工具组组内排序 | 边界 | 组内排序 [Agent 管理] [Agent 调试] [数据库]；「页面」组四项与既有 testid 零变化（回归锚定） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| Router 与布局环境 | MemoryRouter + SidebarProvider 包裹（既有装置）；ResizeObserver stub 兜底 radix 定位 | 全部 describe |
| 无 IPC | 侧栏纯导航组件不触 invoke（既有口径） | 全部 describe |

### packages/desktop/src/types/generated/bindings.ts -> packages/desktop/src/types/generated/bindings.test.ts

<!-- 生成物（随 bindings:export 再生成，零手写）。不应创建 bindings.test.ts：七新命令与
     agent_start 参数面、四新类型出线的类型正确性由消费方 hook / 视图 / 命令测试编译期承载；
     bindings/mod_test.rs 快照随再生成机械更新（design 清单外补入行，非手写用例）。
     详见不可测试项 9。 -->

#### 待测功能

<!-- 纯生成物，design.md 未声明任何运行时行为变更；不设条目。 -->

#### 用例

<!-- 不设用例；不应创建 bindings.test.ts（零用例套件会红整个 vp test 套件），见不可测试项 9。 -->

#### Mock策略

<!-- 不适用：零运行时行为。 -->

---

## 不可测试项

- AC-12 全管线绿（`cargo test --workspace` 全绿、`pnpm -C packages/desktop run client:check` 与 `run test` 全绿） — **原因**: 执行门禁本身非单元测试设计对象，由 test-execution 阶段执行全量套件与静态检查承载；单套件级绿落点已分布本设计各章节。
- AC-7 静态审查子句（`crates/core/agent` 无 agent / engine 字样、`AgentStartError` 变体集不变；`runner_for` 签名、`EngineConfig` 字段面、`start_agent_run_with` / `drive_agent_run` 零 diff；store 无迁移代码路径） — **原因**: 静态代码结构审查约束（零 diff 与字样 grep 判定），无进程内行为断言可写；行为半边已路由 agent_test.rs（解析语义 + 既有编排回归继续执行构成零改动证据，定夺 9）与 store_test.rs（additive 打开、零迁移）承载。
- AC-9 注释 token 落位（统一机密标记注释四处放置、token 可 grep 且无 layout 命名隔离扫描双禁令字面量） — **原因**: 注释文本静态落位检查，非运行时行为；遮蔽 Debug 与 MaskedApiKey 的可自动化半边分别路由 model_test.rs 与 masked-api-key.test.tsx；token grep 核对归 code-review / acceptance 静态审查。
- AC-1 路由表注册半边（`routes.tsx` `/agents` → `<AgentsView />` 不携 root） — **原因**: test_resolve_paths 报告 routes.tsx 不在测试配置 scope（Not in test config scope）；一行路由注册由 agents-view.test.tsx（页面两栏渲染）与 app-sidebar.test.tsx（nav-agents active）两端断言夹持，URL→视图端到端映射归 acceptance 真机走查。
- AC-8 真实 SDK 引擎发起（选 sdk agent 发起走真实 rig 租户、时间线 / 落库 / 重放组件真实复用） — **原因**: 真实引擎发起需模型 API 凭据与外部服务，进程内单测不可行；解析产物经门面分发的可自动化半边由 agent_test.rs（resolve 产物形态 + runner_for 收解析产物）与 agent_test.rs 既有 tee / 落库回归承载，真实发起归 acceptance 真机走查。
- R1 存量 `desktop-global.redb` 真机 additive 验证留痕（design 定夺 6） — **原因**: 需开发者磁盘上真实存量库文件，进程内测试不可复现；可自动化半边（native_db 裸构造仅 WorkspaceRecord 的存量形态库 → 新代码 additive 打开读写）已路由 store_test.rs AC-10 用例承载。
- `packages/desktop/src-tauri/crates/infra/store/src/lib.rs`（解析出 `lib_test.rs`） — **原因**: 纯 `pub use` 再导出面追加四类型，零运行时行为；导出正确性由消费方模块用例编译期承载，不应创建 lib_test.rs（零用例测试文件）。
- `packages/desktop/src-tauri/src/commands/mod.rs`（解析出 `commands/mod_test.rs`） — **原因**: `all_commands!` 宏清单追加为纯注册导线（原生 handler 与 export-bindings 两侧自动同步）；注册正确性由 bindings/mod_test.rs 快照（24 → 31 条）与七命令各自用例间接承载，不应创建独立测试文件。
- `packages/desktop/src/types/generated/bindings.ts`（解析出 `bindings.test.ts`） — **原因**: 构建再生成物（零手写），无自研运行时行为；类型正确性由消费方用例编译期承载，不应创建测试文件。`bindings/mod_test.rs` 快照随 `bindings:export` 机械再生成（design 清单外补入行），非手写用例设计对象。
- `packages/desktop/package.json` 的 version bump（0.3.12 → 0.3.13） — **原因**: 静态配置项（沿用归档先例口径），版本号由发布流程与产物 rebuild 核对，非代码测试对象。




