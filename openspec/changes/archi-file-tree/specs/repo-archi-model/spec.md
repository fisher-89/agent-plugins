# repo-archi-model Specification（delta: archi-file-tree）

## MODIFIED Requirements

### Requirement: 图自洽与停摆禁令

基线模型 SHALL 使四类结构诊断**全部零命中**：`cycle` / `dependsOnAncestor` / `unknownReference` / `orphanHierarchy`；`archi_graph(root)` SHALL 返回正常态（元素与关系完整、`diagnostics` 为空），MUST NOT 停摆。

- **祖先显式声明**：所有容器元素 SHALL 在模型内声明（层级由 FQN 前缀推导，孤儿层级即 `orphanHierarchy`）；MUST NOT 依赖「隐式祖先自动补建」（本能力体系的既有前提）。
- **停摆零容忍**：交付面 MUST NOT 以「结构错 + 停摆面板」形态收尾——停摆是坏模型的处置，不是基线的可接受状态。
- **`docNotFound` 零告警**：`docs` 只指现存根文档、`path` 指向现存目录 / 文件，故 `warnings` SHALL 为空。

#### Scenario: 正常态而非停摆态

- **WHEN** 调用 `archi_graph(root)`
- **THEN** 返回正常态：`elements` 51、`relations` 113（后端 49 + 前端 64）、`diagnostics` 空、`warnings` 空；`/architecture` 页面呈现左栏树 + 画布 + 抽屉三段，无结构错面板、零「修好后再看」文案

#### Scenario: 成环与祖先禁边被机械拦住

- **WHEN** 假设模型被写成含 `orchestration ⇄ workflow`（纳入 dev 边）或 `Standard.Dailog → Standard`（把 hooks 归给容器）一类错误
- **THEN** 结构诊断立即返回 `cycle` / `dependsOnAncestor` 且画布停摆——该写法在交付面上不可通过（基线自身 MUST 为这两种写法的反例）

#### Scenario: 孤儿层级零命中

- **WHEN** 检查每个叶元素的 FQN 前缀链
- **THEN** 每一级祖先（`App` → `App.Backend` → `App.Backend.Core` → crate；`App` → `App.Frontend` → `Views.Components` → 叶）均在模型内显式声明，`orphanHierarchy` 零命中（含本变更新增的 `App.Backend.Tools.ArchiApiGen` 链：`App` → `App.Backend` → `App.Backend.Tools`）

## ADDED Requirements

### Requirement: 本仓架构域看护配置与排除留痕

本仓库 SHALL 落位一份**架构域看护配置** `<archi_dir>/config.json`（本期落点 `.dev-team/architecture/config.json`），内容 SHALL 恰为扫描排除一条：

```json
{ "exclude": ["openspec/**"] }
```

- **字段面只用 `exclude`**：本次不引入 `ignore`（零豁免条目）——配置缺失与「有配置但 `ignore` 为空」在语义上等价，故 MUST NOT 为凑形态写空 `ignore` 数组。
- **MUST NOT 带 `$schema` 键**：该文件的读取面是 `desktop-archi-drift`「架构域看护配置」的严格解析（`deny_unknown_fields`，只登记 `ignore` / `exclude` 两键，`$schema` 既无 `rename` 也无容忍分支）；写 `$schema` 会让配置解析直接 `Err`，同时打断漂移命令、门禁 runner 与文件树三条消费面。本文件与工作区 `.dev-team/config.json`（带 `$schema`）形态不同，两者 MUST NOT 类比照抄。
- **排除理由**：`openspec/` 下存在**未被 gitignore** 的源文件（实测 `openspec/changes/archive/2026-09-28-agent-component-ai-sdk/prototypes/{index,agent-input,agent-messages}.tsx` 三个原型），它们不在 `decisions/0001-model-granularity.md` §7 的封闭排除面内、也不被默认测试面约定命中，故必须经配置排除，MUST NOT 以「代码里写死一条 `openspec/` 前缀」替代（那会让「什么进扫描面」变成第二套判据）。
- **连带留痕（必须如实登记，不是零改动）**：该文件生效后，上述三个文件在漂移报告中由 `scan.unmatchedFiles[]` 移入 `scan.excluded[]`（`{ file, rule }`，`rule = "openspec/**"`）——`unmatchedFiles` −3 / `excluded` +3；文件树的枚举面（`desktop-archi-file-tree`「枚举面与漂移判定面逐条一致」）随之同源变化（实测进树文件 667 → 358 中 −3 归此、−306 归测试面约定）。判定规则、`status` 推导与报告结构零变化。
- **模型资产零变化**：该配置 MUST NOT 引起任何模型改动——元素 / 关系 / `path` / `gate` / 计数（51 元素 / 113 关系、`06` 段 28 / 64）与裁定记录 `§9` 复算表逐字不变；`views/architecture/**` 已是单元素 `App.Frontend.Views.Architecture`，本变更在该目录内新增的 `components/file-tree.tsx` / `hooks/use-archi-file-tree.ts` 不建元素、不贡献新边（视图内 `lib/` 与 `hooks/` 的依赖计入该视图元素）。
- **工作区配置零触碰**：本文件 MUST NOT 被工作区配置 crate 读取；`.dev-team/config.json` 的 `KNOWN_KEYS`、`config.schema.json` 镜像与前端 config 页零变化（归属面写死：工作区配置管执行，架构域配置管看护策略）。

#### Scenario: 配置文件在案且形态正确

- **WHEN** 读取 `.dev-team/architecture/config.json`
- **THEN** 内容为 `{ "exclude": ["openspec/**"] }`（单键、单条、目录形 glob）；`grep '$schema'` 该文件零命中；`archi_drift` 与 `archi_file_tree` 均正常返回（配置解析零 `Err`）

#### Scenario: 排除生效与报告留痕

- **WHEN** 在真仓执行全量漂移检查与文件树取数
- **THEN** 漂移报告 `scan.excluded[]` 含 `openspec/changes/archive/2026-09-28-agent-component-ai-sdk/prototypes/{index,agent-input,agent-messages}.tsx`（`rule = "openspec/**"`），这三条 MUST NOT 出现在 `scan.unmatchedFiles[]`；文件树中 `openspec/` 前缀下零节点

#### Scenario: 模型与计数零漂移

- **WHEN** 对比配置加入前后的模型资产与计数复述
- **THEN** `models/` 六个文件逐字不变（51 元素 / 113 关系、`06` 段 28 / 64、`gate` 全 `off`）、`decisions/0001-model-granularity.md` 的 `§9` 复算表逐字不变；`archi_graph(root)` 仍为正常态（零诊断、零告警）

#### Scenario: 工作区配置零影响

- **WHEN** 审查工作区 `.dev-team/config.json`、`crates/core/config` 的 `KNOWN_KEYS` 与 `config.schema.json` 镜像
- **THEN** 三者与本变更前逐字一致；架构域配置只经 `foundation::layout::archi_config_path` 派生路径读取，未被工作区配置 crate 读取

## Module Contract

主基线既有行逐字沿用；本 delta 新增 / 影响如下：

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `.dev-team/architecture/config.json`（新增） | 架构域看护配置 | `{ "exclude": ["openspec/**"] }`；**不带 `$schema`**（读取面 `deny_unknown_fields` 只认 `ignore` / `exclude`）；生效后 `openspec/` 下三个原型 tsx 由 `unmatchedFiles[]` 移入 `excluded[]` |
| `.dev-team/architecture/models/**` / `decisions/0001-model-granularity.md` / `api/**` | 模型与契约资产（零改动） | 元素 / 关系 / `path` / `gate` / 计数与 ADR 复算表逐字不变；配置加入 MUST NOT 引起模型改动 |
| `packages/desktop/src/views/architecture/**`（见 `desktop-archi-file-tree`） | 画布（零模型漂移） | 该目录已是单元素 `App.Frontend.Views.Architecture`；新增 `components/file-tree.tsx` 与 `hooks/use-archi-file-tree.ts` 不建元素，其 import 计入该视图元素（计数零变化） |
