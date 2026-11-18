# desktop-archi-file-tree Specification（delta: module-entry-docs-semantics）

## Purpose

本 delta 让文件树的「不进树」事实面跟上 E-1 目录迁移：契约资产由 `<archi_dir>/api/**` 搬进 `<archi_dir>/models/<Domain>/**`、索引搬进 `<archi_dir>/graph/`。**判据零改动**——文件树只枚举五种源扩展名，md / `.schema.json` / `INDEX.toml` 本来就不在枚举面内；本 delta 把这条事实写成机械断言（新增一条 bullet 与一条 Scenario），并同步 Module Contract 的「零改动面」路径实例。

## MODIFIED Requirements

### Requirement: 枚举面与漂移判定面逐条一致

文件树的源文件枚举面 SHALL 与架构漂移的扫描面**逐条相同**——树上可见的 = 漂移实际判定的那批文件：

- **遍历**：SHALL 复用扫描单点的 gitignore 感知遍历（`scan::walk_source_files`，提为 `pub(crate)` 供本能力调用）与扩展名判据（`scan::is_source_file`，五种 `.rs` / `.ts` / `.tsx` / `.js` / `.jsx`）；MUST NOT 自建 `read_dir` 遍历、MUST NOT 另写扩展名清单。
- **剪枝**：隐藏项、gitignore 命中项、`target` / `node_modules` / `dist` / `build` / `.git` MUST NOT 进树（沿既有遍历器的 `filter_entry` 语义）。
- **排除矩阵**：SHALL 复用 `scan::exclude::ExcludeSet::build(archi_config.exclude)` 的完整矩阵（默认 13 条测试面约定 + 架构域配置追加），逐文件以 `matched(rel).is_some()` 丢弃；MUST NOT 新增「只配置不默认」的第二入口（`ExcludeSet` 语义即「默认在前、配置追加、最小下标优先」，正是所需）。
- **测试面不在树中**：`**/*.test.{ts,tsx,js,jsx}` / `**/*.spec.{ts,tsx,js,jsx}` / `**/__tests__/**` / `**/*_test.rs` / `**/tests/**` / `**/fixtures/**` / `**/__mocks__/**` 命中项 MUST NOT 出现（本仓实测源文件 667 → 进树 358，其中 306 个测试文件被排除）。
- **契约资产与契约索引不在树中（本变更后的显式事实）**：`<archi_dir>/models/<Domain>/**`（契约 `<FQN>.md` 与 `<FQN>.schema.json`）与 `<archi_dir>/graph/INDEX.toml` MUST NOT 出现在树里——它们不是五种源扩展名之一，故被**扩展名判据**排除，与 `ExcludeSet` 的排除矩阵无关。本能力 MUST NOT 为此新增目录白名单、MUST NOT 为契约文件新增条目类型或标记值；`models/` 一级的六个 `NN-<域>.toml` 同样不进树（`.toml` 非源扩展名）——「模型目录里只有源文件才进树」与「archi 树整棵对文档面不可见」两条既有口径同时成立。
- **配置 `exclude` 生效**：架构域配置 `exclude` 命中项 MUST NOT 出现；非法配置（空白条目 / 无通配符 / glob 编译失败）SHALL 使命令 `Err`（文案沿用 `drift/config.rs` 与 `ExcludeSet::build` 的既有显式文案），树进错误态、MUST NOT 阻断画布。
- **目录合成**：每个进树文件与其所属模块根 SHALL 逐级上溯合成目录节点（去重）；**合成目录不需要在盘存在**（元素根可能声明了不存在或已被排除的目录）。

#### Scenario: 测试面不出现

- **WHEN** 盘上存在 `src/views/architecture/components/file-tree.test.tsx` 与 `crates/core/archi/src/tree_test.rs`
- **THEN** 两条均不在树中；同目录下的非测试源文件（`file-tree.tsx` / `tree.rs`）照常出现

#### Scenario: 配置 exclude 命中项不出现

- **WHEN** 架构域配置 `exclude = ["openspec/**"]` 且仓内 `openspec/changes/archive/**/prototypes/index.tsx` 未被 gitignore
- **THEN** 该文件与其各级目录均不在树中（若某目录其余源文件全被排除，则目录也不出现）

#### Scenario: 构建 / 依赖产物树不出现

- **WHEN** 盘上存在 `packages/desktop/src-tauri/target/debug/x.rs`、`packages/desktop/node_modules/a/index.js`、`.git/hooks/y.ts` 或某隐藏目录下的源文件
- **THEN** 均不在树中；同层正常源文件照常出现

#### Scenario: 非法配置使树进错误态而不阻断画布

- **WHEN** 架构域配置 `exclude` 写一条无通配符的条目（如 `src/legacy`）
- **THEN** `archi_file_tree` 返回 `Err`（文案含非法条目并提示 `/ **` 形态）；前端只渲 `archi-file-tree-error` 行内提示，画布与抽屉照常渲染

#### Scenario: 契约资产与模型文件都不进树

- **WHEN** 盘上 `<archi_dir>/models/` 一级有六个 `NN-<域>.toml`、`<archi_dir>/models/Infra/` 有 `<FQN>.md` 与 `<FQN>.schema.json`、`<archi_dir>/graph/INDEX.toml` 存在，跑一次 `archi_file_tree`
- **THEN** 上述文件与其合成目录节点均不出现在树里（三者的扩展名都不是五种源扩展名，判据是扩展名而非目录名）；树节点集合与迁移前逐字相同——反向判据：若把某契约 md 改名为 `.ts` 形态放进元素 `path` 内，它会立刻作为普通源文件进树（说明排除面不是「archi 目录豁免」而是「非源文件不收」）

## Module Contract

| 模块 | 变化 | 说明 |
|------|------|------|
| `crates/core/archi/src/tree.rs` / `tree_test.rs` | 零实现改动；新增一条不进树断言 | 枚举面、归属与剥离判据、`ArchiFileTree` 出线形状逐字不变；新增「契约资产与模型文件都不进树」用例 |
| `crates/core/archi/src/api/**`、`crates/tools/archi-api-gen/**`、`<archi_dir>/models/<Domain>/**`、`<archi_dir>/graph/**` | 零改动（路径实例换段） | 契约生成面零改动（并集判据只动归属，不动落点与 `#[path]` 等值判据）；`<archi_dir>/api/**` 已随 E-1 迁移退役，本行只是把路径实例换到新落点 |
| `packages/desktop/src/views/architecture/components/file-tree.tsx` + 测试 | 零改动 | 前端只消费 `ArchiFileTree` 投影；树节点集合不因契约资产搬进 `models/` 而变化 |
