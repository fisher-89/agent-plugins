# desktop-archi-agent Specification (Delta)

## MODIFIED Requirements

### Requirement: 架构会话的系统知识注入

`source = "archi"` 的会话 SHALL 注入一段架构会话知识到 `SessionInjections.preamble`（系统消息合成见 `desktop-agent-execution`），内容 SHALL 覆盖以下关注点（**按 A/B/C/D 分层后**的清单，分层判据见「知识分层与工具面同批移动」）：

- **模型位置与聚合（B）**：模型文件位于架构域 `graph/model/` 下（契约资产在 `models/<分层目录…>/`，两目录分家）、按文件名字典序聚合；两处目录路径 SHALL 为**派生所得**的 repo 相对路径（见 `workspace-layout-resolution`），MUST NOT 在产品源码或知识文本中手写目录字面量。契约索引 `INDEX.toml` 的方位 MUST NOT 进知识文本（生成物，零手写面；需要时经 `archi_api` 的 `generatedPath` 取得）。
- **字段纪律（B + C）**：
  - `path`（B，一行 + 工具指路）：叶元素必填、`./` 前缀、repo 相对、目录以 `/` 结尾、文件精确到扩展名、容器元素恒空（零例外）；「这个元素缺不缺 `path`」由 `archi_module` 的 `hasPath` 一次判出。
  - `entry`（B，一行 + 工具指路）：只在约定默认指不到时声明（单文件根 = 根文件自身 / 目录型根 = `<root>/index.ts(x)` / crate = `src/lib.rs`）；解析结果与不可用成因由 `archi_module` 的 `resolvedEntry` 给出，MUST NOT 在知识文本里复述三档长表。
  - `doc`（B，一行 + 判据指路）：单值、repo 相对，只有 `api = none` 的元素可写；`declared` 的入口文档恒为派生落点（按 FQN 段链去根与叶分目录、名为 `<FQN>.md`），写了即 `apiUnexpected`。判据长表归 `desktop-archi-api` / `desktop-archi-api-check`。
  - **字段形态与键形（C，散文必留）**：一元素一表头、键为带引号的完整 FQN 单键（禁点分嵌套写法——`[element.App.Auth]` 会被读成**另一个元素**）；`metadata` MUST 用内联表（子表同样会被读成另一个元素）；关系写 `[[relation]]`（`from` / `to` / `desc` / **`apis`**）。**字段序与各字段形态的完整模板不入散文**——用 `archi_scaffold` 取（D 档）。
  - **关系 `apis`（B，一行 + 判据指路，本变更新增）**：选填，**只写名字**——`to` 元素的代码导出名（禁签名 / 类型形状 / `::` 限定路径 / 空白），可留空即不声明 API 粒度，`apis = []` 与同名重复被拒；生成物面 MUST 留空（生成物是唯一真相，手写清单必然漂）。两桶结论（用了没声明 / 声明没用）由 `archi_drift` 的 `errors` / `warnings` / `observations` 给出，MUST NOT 在知识文本里复述判据长表，也 MUST NOT 让模型把它当协议正文写（正文仍在契约资产）。
  - **`kind` 四值语义（C）**：`package` = 包根（无 `path`）/ `domain` = 容器（`path` 恒空）/ `module` = crate、目录型模块或视图 / `component` = 组件文件。四值词汇集由派生段给出，语义只在本条散文。
- **层级与缺省（B + C）**：层级由 FQN 最长前缀推导（子元素须先声明祖先——违反时结构诊断 `orphanHierarchy` 由 `archi_module` / `archi_drift` 的 `is_error` 与读数给出）；新元素 `gate` 缺省 `off`（ratchet 逐块收紧）。
- **计数复述单源（B，一行）**：模型文件表头注释里的元素 / 关系数量 SHALL 以工具读数（`archi_drift` 的 `model.elements` / `model.relations`）为准，改模型 SHALL 在同一批内更新表头；MUST NOT 让表头与模型本体各说各话。spec 的计数表与 ADR 的复算表属仓库文档面，知识文本 MUST NOT 复述任何具体数字。
- **架构域看护配置（B，一行）**：`<archi_dir>/config.json` 存在且管 `ignore`（元素对豁免）/ `exclude`（扫描排除），未知键与缺 `reason` 即配置非法——非法时 `archi_drift` 以 `is_error` 点名具体条目；MUST NOT 在知识文本里复述字段长表。
- **边界（C）**：模型里 MUST NOT 写接口 / 导出符号（契约权威在契约资产）；`api` / `entry` / `doc` 是三个**指针**（有无公共 API / 入口在哪 / 入口文档读哪份），MUST NOT 由它们扩张出协议正文；关系 `apis` 是**消费侧的依赖名字清单**（不是提供方的协议正文）——位置在关系、内容只有名字、权威仍在该元素的代码导出面。
- **自检纪律（A，指路）**：改完可**提前**自查——`archi_drift` 取覆盖 / 归属 / 契约读数（`errors` / `warnings` 通常为空，非空即逐条修）、`archi_module` 取结构诊断（带 `file:line`）、契约面 `archi_api`（`action = "check"`）查两轴；生成物过期用既有命令（`pnpm api:export` / `cargo run -p archi-api-gen -- generate`）重生成，MUST NOT 手改生成物。该纪律 SHALL 为纯提示词约束加工具指路；引擎在收口时另有一次兜底自检（见「收口前引擎自检（架构判据读数）」），故自检 SHALL NOT 被表述为「不调工具就不算完」的义务。
- **写面与可见性（C）**：直改主仓（无 worktree、无归档确认），改动随 `git diff` 评审；**禁止修改架构目录之外的文件**（源码 / 配置 / 脚本 / 域外文档一律不改，用户要求亦不越界；生成物重生成除外）；写盘后画布不自动更新，须提示用户点「刷新」。
- **历史会话续接（C）**：续接历史会话时 SHALL 先取当下模型现状（`archi_module` 无参概览或指定元素）再回答「现在是什么样」一类问题，MUST NOT 依赖会话早前的描述。

词表类内容 SHALL 从代码单点派生，MUST NOT 手抄：

- `kind` 四值 ← `ElementKind::ALL`；`gate` 三值 + 缺省档 ← `GateTier::ALL` / `GateTier::default()`；`api` 状态二值 + 缺省档 ← `ApiStatus::ALL` / `ApiStatus::default()`；结构诊断九类词 ← `DiagnosticKind::ALL`；模型声明目录与契约资产目录 ← `foundation::layout` 的架构域相对路径派生族。
- `entry` / `doc` / **`apis`** SHALL **不进**派生词表：三者的值域是任意 repo 相对路径 / 任意代码标识符（开放集），不是封闭词汇；知识文本只给纪律句 + 工具指路，判据定义归 `desktop-archi-api` / `desktop-archi-api-check` / `desktop-archi-drift`，不在知识文本内复制判据长表。
- **派生的目录方位 SHALL 为两处**：模型声明目录（`archi_graph_model_rel_dir`）、契约资产（`<archi_models_rel_dir>/<分层目录…>/<FQN>.md`）。**契约索引方位句 SHALL 删除**（生成物，零手写面）；**结构诊断九词与四面词表 SHALL 保留**（`desktop-archi-model`「消费面词汇集一致性对拍」把知识文本列为四面之一，删即破该机械闭环）。
- 知识文本 SHALL 为**静态骨架 md（`include_str!`）+ 上述派生片段**在装配侧拼成的单条文本；新增 / 删除任一封闭词汇而知识未同步时，派生消费面测试 MUST 变红。

两条工程约束：

- **自证可解析**：字段形态的机械锚 SHALL 由 `archi_scaffold` 的草稿承担（`TomlParser` 零诊断，见「archi_scaffold 只读工具的新元素模板契约」）；知识文本本身 MUST NOT 再承载示例 TOML（示例是重复的第二事实源）。
- **尽力增强、不炸 run**：知识拼装失败（派生异常 / 读取失败）SHALL 降级为 `preamble = None`，MUST NOT 让发起点 `Err`、MUST NOT 中断 run；知识文本 MUST NOT 含 `<>` 一类占位符（路径由派生注入），体量 SHALL 受上限约束（**骨架 ≤ 2.2 KB、全文 ≤ 2.8 KB，实测值冻结**，防与 `AGENT.md` 合体后挤压上下文）。**本变更的增量 SHALL 在既有上限内**（关系键面补一个 `apis` + 一句 `apis` 纪律，约 +60 B；超限时 SHALL 压缩同段冗余表述而不是抬高锚），收口自检的装配与本知识解耦（`before_done` 档位按来源置位），知识降级 MUST NOT 影响自检。
- **单条 System**：该知识 MUST NOT 以独立第二条 System 消息下发（合并规则见 `desktop-agent-execution`「SDK 引擎系统提示词（AGENT.md 注入）」）。

#### Scenario: 词表由代码派生

- **WHEN** 检查知识文本的 `kind` / `gate` / `api` / 诊断词清单，并新增一个结构诊断变体（同时补 `DiagnosticKind::ALL`）
- **THEN** 知识文本随即含该新词（无需改静态骨架）；若只加变体而未补 `ALL`，`ALL` 与 `as_str` 对拍测试红；`entry` / `doc` / `apis` 不出现在任何派生词表行（开放值集，零词表）

#### Scenario: 字段纪律走工具指路而非长表

- **WHEN** 审查知识文本的字段纪律段
- **THEN** `path` / `entry` / `doc` 各为一行纪律 + 工具指路（`hasPath` / `resolvedEntry` / `apiUnexpected`）；零 `entry` 三档长表、零字段序清单、零示例 TOML 围栏；`kind` 四值语义、引号 FQN 单键、内联 `metadata`、层级先声明四条散文在场

#### Scenario: 关系键面与 apis 纪律在场

- **WHEN** 审查知识文本里关系相关的那一段
- **THEN** `[[relation]]` 的键面为四键（`from` / `to` / `desc` / `apis`）；`apis` 的纪律句含「只写名字」「可留空」「生成物面留空」三条关键词，且不含任何判据长表（两桶 / 档位 / 白名单长句零命中）；同样的句子里零示例 TOML 围栏（键名只作行内引述）

#### Scenario: 拼装失败降级

- **WHEN** 知识拼装路径出现异常（派生产物不可用）
- **THEN** 装配结果为 `preamble = None`，session 照常发起、run 正常推进（七工具面 + 五 archi 工具不变），MUST NOT 抛装配错误；收口前自检照常装配（`before_done` 档位与知识解耦；T1 / T2 皆假时仍零 notice）

#### Scenario: 自检纪律与可见性在知识中可指认

- **WHEN** 检查知识文本内容
- **THEN** 同时含「改完可调 `archi_drift` 取读数并按条修」「结构诊断经 `archi_module` 带 `file:line`」「契约面可调 `archi_api` 查两轴」「生成物过期用既有命令重生成、MUST NOT 手改生成物」「写盘后画布不自动更新、提示用户刷新」「续接历史会话先取当下模型」六条；且知识文本内零 `<>` 占位符

#### Scenario: 体量与目录方位实测

- **WHEN** 取 `knowledge()` 输出与骨架字节
- **THEN** 骨架 ≤ 2.2 KB、全文 ≤ 2.8 KB 且实测值被冻结断言（本变更的 `apis` 增量仍在此上限内）；派生段含模型声明目录与契约资产两处方位，**零契约索引串**、**四面词表（含结构诊断九词）在场**

## Module Contract

主基线既有行逐字沿用；本变更受影响模块如下：

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/agent/src/archi-knowledge.md` | 静态骨架（`include_str!`） | 「位置与键形」段的关系键面由三键改四键（`from` / `to` / `desc` / `apis`）；「字段纪律」段新增一行 `apis` 纪律（只写名字 / 可留空 / 生成物面留空 / 结论看 `archi_drift`）；零示例围栏、零判据长表、零 `<>` 占位符 |
| `crates/infra/agent/src/archi_knowledge.rs` | 派生段装配（零改动） | 四面词表与两处目录派生逐字不变；`apis` 不进派生的任何词表（与 `doc` / `entry` 同列） |
| `crates/infra/agent/src/archi_knowledge_test.rs` | 知识面测试 | 新增锚：关系四键在场、`apis` 三关键词在场、`apis` 不出现在派生词表行、骨架字节仍 ≤ 2.2 KB；既有锚（零围栏 / 零字段形态教学 / 指路表 / 体量）逐字保留 |
| `crates/infra/agent/src/sdk/mount.rs` / `compose.rs` | 工具面（零改动） | 工具清单与 `(phase, role)` 矩阵不变（`archi_relation` 只走 IPC 命令面，不进 agent 工具面） |

### Function: `crates/infra/agent/src/archi_knowledge.rs`

| 项 | 变化 | 描述 |
|---|---|---|
| `knowledge()` | 零改动（内容随骨架） | 骨架 + 派生段拼装；失败降级 `None`；体量锚（骨架 ≤ 2.2 KB / 全文 ≤ 2.8 KB）仍由测试断言 |
| 派生词表 | 零改动 | `ElementKind::ALL` / `GateTier::ALL` / `ApiStatus::ALL` / `DiagnosticKind::ALL` 与两处目录派生逐字不变；`apis` 不进词表 |
