# desktop-archi-module-docs Specification（delta: module-api-contract-format）

## ADDED Requirements

### Requirement: 契约资产不经 docs 三源

模块 API 契约资产（`<archi_dir>/api/**`）SHALL NOT 成为模块文档关联的第四种来源，MUST NOT 出现在 `element_docs` 的四字段条目（`{ path, title, kind, source }`）里：

- **来源面仍恰三种**：`explicit` / `adjacent` / `text` 三值封闭不动；契约文件 MUST NOT 被映射为任何第四值（`kind` 亦 MUST NOT 新增 `api` 值）。
- **反向禁写 `docs`**：任何元素的 `docs` MUST NOT 指向 `<archi_dir>/api/**`——与既有「MUST NOT 指向 `openspec/specs/**`」同源理由：模型 MUST NOT 反向依赖派生自它 / 与它并列的资产。该纪律沿既有先例由模型字段审查把守（`repo-archi-model` 的「`docs` 只由包根元素指仓库面文档」），MUST NOT 在关联层新增运行时判据或第四条来源。
- **`archi_module` 的 `api` 字段是并列字段而非 `docs` 承重**：契约状态与路径经 `ArchiModuleDetail.api` 出线（见 `desktop-archi-api` 与 `desktop-archi-agent`），MUST NOT 借道 `docs` 列表承载——否则「关联来源恰三种」的封死口径被破，文档检索会开始命中契约正文。
- **检索面不受影响**：`archi_search` MUST NOT 把 `<archi_dir>/api/**` 纳入文档路径检索结果（元素薄条目与 ADR `text` 源不变）。

#### Scenario: 契约不进入 docs 条目

- **WHEN** 某元素既有 `docs = ["./src/auth/README.md"]`，同时它在 `<archi_dir>/api/` 下有契约文件，并对该元素调用 `archi_module`
- **THEN** `docs` 恰一条（`source = "explicit"`、`kind = "md"`）；契约文件不出现在 `docs` 内、不产生第四条 `source` 值；契约状态只经并列字段 `api` 出线

#### Scenario: 元素 docs 指向契约被字段审查拦下

- **WHEN** 审查模型内任一元素的 `docs` 字段
- **THEN** 无一条指向 `<archi_dir>/api/**`（与既有「不指 `openspec/specs/**`」同口径）；契约资产的消费者是生成器与门禁，不是文档关联

#### Scenario: 检索不命中契约正文

- **WHEN** 以 `query` 命中契约 md 内某个词（该词亦出现在 ADR 中）执行 `archi_search`
- **THEN** 命中项只来自模型元素与 ADR 文档条目；`<archi_dir>/api/**` 下任何路径与内容均不出现

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/docs.rs`（关联来源面） | 来源封闭（零改动） | `element_docs` 仍恰 `explicit` / `adjacent` / `text` 三源；MUST NOT 新增 `<archi_dir>/api/**` 来源或 `api` 类 `kind` |
| `crates/core/archi/src/docs.rs`（邻近扫描范围） | 架构域目录排除（零改动） | `<archi_dir>/**` 内的 md 不作为任何元素的邻近文档（既有判据天然覆盖 `api/**`，本变更不新增规则） |
| `crates/core/archi/src/search.rs`（检索面） | 零改动 | 检索语料不含 `<archi_dir>/api/**`；元素薄条目与 ADR `text` 源不变 |
| `crates/core/archi/src/dto.rs`（`ArchiApiRef`，见 desktop-archi-api） | 契约状态的并列承载 | `ArchiModuleDetail.api` 为并列字段；MUST NOT 复用 `DocRef` 四字段形状或混入 `docs` 列表 |
