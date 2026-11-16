# desktop-archi-agent Specification（delta: module-entry-field-spec）

## Purpose

本 delta 只改架构会话知识文本的**元素字段面**：由六项补齐为八项（补上本已漏记的 `api` 与新字段 `entry`），并写明 `entry` 的三条纪律（单值 / 只在约定默认指不到时写 / 单文件根 MUST NOT 写）与「`entry` 不进派生词表」（开放值集）。自检纪律、写面与可见性、降级与体量约束逐字不动；`AGENT.md` / `CLAUDE.md` 的契约纪律节在本 delta 内**条目面与节结构冻结**（条目数恒 5、两份副本逐字相同的条款保留），该节第②条的方位词与文件名形态随落点迁移同批同步——这条措辞同步归 `desktop-archi-api`「仓库根文档的契约纪律节」，本 delta 不改该节正文。

## MODIFIED Requirements

### Requirement: 架构会话的系统知识注入

`source = "archi"` 的会话 SHALL 注入一段架构会话知识到 `SessionInjections.preamble`（系统消息合成见 `desktop-agent-execution`），内容 SHALL 覆盖以下关注点：

- **模型位置与聚合**：模型文件位于架构域 `models/` 下、按文件名字典序聚合；目录路径 SHALL 为**派生所得**的 repo 相对路径（见 `workspace-layout-resolution`），MUST NOT 在产品源码或知识文本中手写目录字面量。
- **格式约定**：一元素一表头、键为带引号的完整 FQN 单键（禁点分嵌套写法）；元素字段面为 `kind`（必填）/ `gate` / `api` / `entry` / `path` / `docs` / `summary` / `metadata`；`entry` 是**单值**实现入口指针（repo 相对路径，与 `path` 同口径），只在约定默认指不到时显式写（目录型壳与二进制 crate），单文件根元素 MUST NOT 写（根文件即入口）；关系为 `[[relation]]` 条目（`from` / `to` / `desc`）；`metadata` MUST 用内联表，MUST NOT 用子表（子表会被解析为另一个元素）。
- **层级与缺省**：层级由 FQN 最长前缀推导（子元素须先声明祖先）；新元素 `gate` 缺省 `off`（ratchet 逐块收紧）。
- **边界**：模型里 MUST NOT 写接口 / 导出符号（契约权威在契约资产，只经 `docs` 引用）；`api` 与 `entry` 是两个**指针**（有无公共 API / 入口在哪），MUST NOT 由它们扩张出协议正文。
- **自检纪律**：改完 SHALL 立即复查——调用 `archi_module`（不带 `element`）取得薄概览或结构诊断清单（诊断带 `file:line`），按行修复；契约面另可调 `archi_api`（`action = "check"`）查两轴（生成物三类差异 + 契约判据）；生成物过期 SHALL 用既有命令（`pnpm api:export` / `cargo run -p archi-api-gen -- generate`）重生成，MUST NOT 手改生成物。该纪律 SHALL 为纯提示词约束，MUST NOT 依赖写盘校验或引擎自动校验回灌。
- **写面与可见性**：直改主仓（无 worktree、无归档确认），改动随 `git diff` 评审；只改架构域模型与 `docs` 关联文档，改源码须用户明确要求；写盘后画布不自动更新，须提示用户点「刷新」。
- **历史会话续接**：续接历史会话时 SHALL 先取当下模型现状（`archi_module` 无参概览或指定元素）再回答「现在是什么样」一类问题，MUST NOT 依赖会话早前的描述。

词表类内容 SHALL 从代码单点派生，MUST NOT 手抄：

- `kind` 四值 ← `ElementKind::ALL`；`gate` 三值 + 缺省档 ← `GateTier::ALL` / `GateTier::default()`；`api` 状态二值 + 缺省档 ← `ApiStatus::ALL` / `ApiStatus::default()`；结构诊断九类词 ← `DiagnosticKind::ALL`；目录相对路径 ← `foundation::layout` 的架构域相对路径派生族。
- `entry` SHALL **不进**派生词表：它的值域是任意 repo 相对路径（开放集），不是封闭词汇；知识文本只给「单值 / 只在约定默认指不到时写 / 单文件根 MUST NOT 写」三条纪律，四条判据的定义归 `desktop-archi-api`，不在知识文本内复制判据长表。
- 知识文本 SHALL 为**静态骨架 md（`include_str!`）+ 上述派生片段**在装配侧拼成的单条文本；新增 / 删除任一封闭词汇而知识未同步时，派生消费面测试 MUST 变红（词汇四面一致性见 `desktop-archi-model`）。

两条工程约束：

- **自证可解析**：知识文本 SHALL 含一段示例 TOML，测试 SHALL 以 `archi` 的解析面解析该片段并要求零结构诊断（字段名 / 内联 `metadata` / 键形 / `entry` 单值形态任一漂移即红）。
- **尽力增强、不炸 run**：知识拼装失败（派生异常 / 读取失败）SHALL 降级为 `preamble = None`，MUST NOT 让发起点 `Err`、MUST NOT 中断 run；知识文本 MUST NOT 含 `<>` 一类占位符（路径由派生注入），体量 SHALL 受上限约束（测试锚定，防与 `AGENT.md` 合体后挤压上下文）。
- **单条 System**：该知识 MUST NOT 以独立第二条 System 消息下发（合并规则见 `desktop-agent-execution`「SDK 引擎系统提示词（AGENT.md 注入）」）。

#### Scenario: 词表由代码派生

- **WHEN** 检查知识文本的 `kind` / `gate` / `api` / 诊断词清单，并新增一个结构诊断变体（同时补 `DiagnosticKind::ALL`）
- **THEN** 知识文本随即含该新词（无需改静态骨架）；若只加变体而未补 `ALL`，`ALL` 与 `as_str` 对拍测试红；`entry` 不出现在任何派生词表行（开放值集，零词表）

#### Scenario: 示例片段可解析

- **WHEN** 取出知识文本中的示例 TOML 片段并用 `archi` 解析
- **THEN** 零结构诊断；把 `metadata` 改成子表、改掉任一字段名、或把 `entry` 改成数组后该测试红

#### Scenario: 拼装失败降级

- **WHEN** 知识拼装路径出现异常（派生产物不可用）
- **THEN** 装配结果为 `preamble = None`，session 照常发起、run 正常推进（七工具面 + 三 archi 工具不变），MUST NOT 抛装配错误

#### Scenario: 自检纪律在知识中可指认

- **WHEN** 检查知识文本内容
- **THEN** 同时含「改完调用 `archi_module` 复查并按其 `file:line` 修复」「契约面可调 `archi_api` 查两轴」「生成物过期用既有命令重生成、MUST NOT 手改生成物」「写盘后画布不自动更新、提示用户刷新」「续接历史会话先取当下模型」五条；且知识文本内零 `<>` 占位符

#### Scenario: 字段面八项与 entry 纪律在案

- **WHEN** 审查知识文本的「格式约定」段
- **THEN** 字段面逐项列出 `kind` / `gate` / `api` / `entry` / `path` / `docs` / `summary` / `metadata` 八项；`entry` 的三条纪律（单值 / 只在约定默认指不到时写 / 单文件根 MUST NOT 写）在案；段内零判据长表（四条判据的定义不复制，只指路）

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/agent/src/archi-knowledge.md` | 静态知识骨架（`include_str!` 内嵌） | 「格式约定」段的元素字段面为八项（`kind` / `gate` / `api` / `entry` / `path` / `docs` / `summary` / `metadata`），并含 `entry` 三条纪律；示例 TOML 经 `TomlParser` 零诊断；零 `<>` 占位符、体量受上限约束 |
| `crates/infra/agent/src/archi_knowledge.rs` | 骨架 + 派生段拼装 | 派生词表取 `ElementKind::ALL` / `GateTier::ALL` / `ApiStatus::ALL` / `DiagnosticKind::ALL` 与 `foundation::layout` 派生族；`entry` MUST NOT 进派生词表（开放值集）；降级为 `None` 而非 `Err` |
| `crates/infra/agent/src/archi_knowledge_test.rs` | 知识语料 | 示例片段可解析（含 `entry` 单值形态）；四面词表在场；体量上限；纯函数性（三次调用逐字相等） |

### Component: 仓库根文档

| 项 | 变化 | 描述 |
|---|---|---|
| `AGENT.md` / `CLAUDE.md` | 仅第②条措辞同步（条目增删与节结构冻结） | `## 模块 API 契约` 节的冻结面为**条目增删与节结构**：条目数恒 5，本变更 MUST NOT 增删要点、MUST NOT 加 `entry` 第六条（字段知识走知识文本与 `desktop-archi-model`），两份副本该节正文仍逐字相同；第②条的方位词与文件名形态（「实现入口同级」/ `<入口主干>.i.*`）属 `desktop-archi-api`「仓库根文档的契约纪律节」的实现面，本 delta 不写该节正文 |
