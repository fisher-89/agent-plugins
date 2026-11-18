# desktop-archi-module-docs Specification（delta: module-entry-docs-semantics）

## Purpose

本 delta 只做**排除面的路径实例换段**（E-1 目录迁移）：契约资产由 `<archi_dir>/api/**` 搬进 `<archi_dir>/models/<Domain>/**`，故「契约资产不经 docs 三源」这条封死口径的被排对象改成新路径。

**语义零改动**：来源仍恰三种（`explicit` / `adjacent` / `text`）、`kind` 仍不新增 `api` 值、排除机制仍是「archi 树整棵对 docs 三源不可见」（`is_archi_rel_path`）——迁入 `models/` 后契约 md 落在 archi 树内，天然仍不可见，本变更**不新增任何运行期判据**。`doc` 单值字段、docs 三源退役与判据面十三值**不属本变更**（归后续 `doc` 语义单元），本条只保证「路径串不指空目录」。

## MODIFIED Requirements

### Requirement: 契约资产不经 docs 三源

模块 API 契约资产（`<archi_dir>/models/<Domain>/**`：`<FQN>.md` 手写真相与 `<FQN>.schema.json` 生成物）SHALL NOT 成为模块文档关联的第四种来源，MUST NOT 出现在 `element_docs` 的四字段条目（`{ path, title, kind, source }`）里：

- **来源面仍恰三种**：`explicit` / `adjacent` / `text` 三值封闭不动；契约文件 MUST NOT 被映射为任何第四值（`kind` 亦 MUST NOT 新增 `api` 值）。
- **排除机制是「archi 树整棵不可见」，不是按目录名逐条列举**：契约 md 迁入模型目录后仍落在 `<archi_dir>` 之内，故 `adjacent`（元素 `path` 覆盖范围）与 `explicit`（`docs` 字段）两条通路都已被既有 `is_archi_rel_path` 判据排除在外——本变更 MUST NOT 为此新增第二条排除规则、MUST NOT 在关联层引入「跳过 `models/<Domain>/`」一类目录白名单（那是第二份身份判据，且会与「archi 树整棵不可见」重复）。
- **反向禁写 `docs`**：任何元素的 `docs` MUST NOT 指向 `<archi_dir>/models/<Domain>/**`——与既有「MUST NOT 指向 `openspec/specs/**`」同源理由：模型 MUST NOT 反向依赖派生自它 / 与它并列的资产。该纪律沿既有先例由模型字段审查把守（`repo-archi-model` 的「`docs` 只由包根元素指仓库面文档」），MUST NOT 在关联层新增运行时判据或第四条来源。
- **`archi_module` 的 `api` 字段是并列字段而非 `docs` 承重**：契约状态与路径经 `ArchiModuleDetail.api` 出线（见 `desktop-archi-api` 与 `desktop-archi-agent`），MUST NOT 借道 `docs` 列表承载——否则「关联来源恰三种」的封死口径被破，文档检索会开始命中契约正文。
- **检索面不受影响**：`archi_search` MUST NOT 把 `<archi_dir>/models/<Domain>/**` 纳入文档路径检索结果（元素薄条目与 ADR `text` 源不变）。

#### Scenario: 契约不进入 docs 条目

- **WHEN** 某元素既有 `docs = ["./src/auth/README.md"]`，同时它在 `<archi_dir>/models/<Domain>/` 下有契约文件，并对该元素调用 `archi_module`
- **THEN** `docs` 恰一条（`source = "explicit"`、`kind = "md"`）；契约文件不出现在 `docs` 内、不产生第四条 `source` 值；契约状态只经并列字段 `api` 出线

#### Scenario: 元素 docs 指向契约被字段审查拦下

- **WHEN** 审查模型内任一元素的 `docs` 字段
- **THEN** 无一条指向 `<archi_dir>/models/<Domain>/**`（与既有「不指 `openspec/specs/**`」同口径）；契约资产的消费者是生成器与门禁，不是文档关联

#### Scenario: 检索不命中契约正文

- **WHEN** 以 `query` 命中契约 md 内某个词（该词亦出现在 ADR 中）执行 `archi_search`
- **THEN** 命中项只来自模型元素与 ADR 文档条目；`<archi_dir>/models/<Domain>/**` 下任何路径与内容均不出现

#### Scenario: 契约与模型同目录后相邻扫描仍不误收

- **WHEN** 某元素的 `path` 覆盖 `<archi_dir>` 之外的自有源码目录，而契约 md 在 `<archi_dir>/models/<Domain>/`（不在该元素 `path` 内），跑 `element_docs`
- **THEN** 该契约 md 既不被 `adjacent` 收（不在元素 `path` 覆盖范围），也不被 `explicit` 收（模型字段纪律禁写）；把契约 md 复制到某元素 `path` 下的同名文件场景则按普通邻近文档处理（那是代码旁的真实文档，不是契约）——两种情形的判据不同源，MUST NOT 混用

## Module Contract

| 模块 | 变化 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/docs.rs`（关联来源面） | 语义零改动、串换段 | `element_docs` 仍恰 `explicit` / `adjacent` / `text` 三源；MUST NOT 新增 `<archi_dir>/models/<Domain>/**` 来源或 `api` 类 `kind`；判据仍只有「archi 树整棵不可见」一条 |
| `crates/core/archi/src/docs.rs`（邻近扫描范围） | 串换段 | `<archi_dir>/**` 内的 md 不作为任何元素的邻近文档（既有 `is_archi_rel_path` 判据天然覆盖 `models/<Domain>/**`，本变更不新增规则） |
| `crates/core/archi/src/search.rs`（检索面） | 零改动 | 检索语料不含 `<archi_dir>/models/<Domain>/**`；元素薄条目与 ADR `text` 源不变 |
