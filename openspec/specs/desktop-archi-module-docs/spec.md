# desktop-archi-module-docs Specification

## Purpose

定义模块文档关联能力：三种关联来源（显式 `docs` / 邻近约定 / ADR 文本）与恒四字段条目契约、合并去重与 `docNotFound` 只报不拦、文档位置与邻近扫描范围、「看护与检索不读 `openspec/specs/`」的迁移边界，以及「模块 API 契约资产不经 docs 三源」的封死口径。

## Requirements

### Requirement: 三种文档关联来源与恒四字段契约

模块文档关联 SHALL 只做**关联与检索**，不规定文档形态、不返回正文。关联来源 SHALL 恰为三种：

| 来源 | 机制 | 强度 | 输出 `source` |
|---|---|---|---|
| 显式 | 元素 `docs = ["./src/auth/README.md"]`（与 `path` 同构的字符串数组） | 强——作者亲口指定 | `explicit` |
| 邻近约定 | 元素 `path` 覆盖目录内的 `*.md` | 中——按位置推断 | `adjacent` |
| ADR 文本 | `<archi_dir>/decisions/*.md` 正文出现该元素 FQN | 弱——检索命中，非承诺 | `text` |

关联条目契约 SHALL 恒为 `{ path, title, kind, source }` 四字段：`path` 为 repo 相对路径（`/` 分隔）、`title` 取文档首行标题（无标题则取文件名去扩展名）、`kind` 为形态标记（`md` / `adr`）、`source` 为来源强度词汇。契约 MUST NOT 含摘要、正文、行区间或任何 schema 字段——将来新增「接口说明文档」一类形态，只需新增一条索引来源，工具契约与前端零改动。

关联结果 SHALL 以元素为查询单位返回（`archi_module` 的 `docs` 字段即本契约）；MUST NOT 提供跨元素的文档反向索引（反查「这份文档描述哪些模块」不在本期，避免用户开始写"给 archi 看的文档"）。

#### Scenario: 三源同时命中按强度排序

- **WHEN** 元素 `App.Auth` 显式 `docs` 指向 `src/auth/README.md`，其 `path` 目录下另有 `src/auth/flow.md`，且 `decisions/0007-token-sink.md` 正文提及 `App.Auth`
- **THEN** 返回三条：`explicit` 排首、`adjacent` 次之、`text` 再次；每条严格只含 `path` / `title` / `kind` / `source` 四字段

#### Scenario: 无任何关联文档

- **WHEN** 元素既无 `docs`、其 `path` 目录无 md、也无 ADR 提及
- **THEN** `docs` 为空数组；不产生 warning、不产生诊断

#### Scenario: 不返回正文

- **WHEN** 审查关联结果的字段面与体积
- **THEN** 条目内不含文档正文 / 摘要 / 行区间；正文由调用方（agent 用 `read`、前端用取数命令）按 `path` 自取

### Requirement: 合并去重与 docNotFound 只报不拦

同一文档被多源命中 SHALL 合并为**一条**，`source` 取**最强**来源（`explicit` > `adjacent` > `text`）。排序 SHALL 先按来源强度、同强度按 `path` 字典序（稳定序，便于断言与 diff）。

`docs` 指向不存在的文件 SHALL 产出 `warnings` 的 `docNotFound`（带元素 FQN 与该路径），且该条目 MUST NOT 出现在 `docs` 列表内。`docNotFound` SHALL 只报不拦：它 MUST NOT 使模型进入停摆态、MUST NOT 影响图数据、MUST NOT 阻断归档或门禁——文档缺失不影响代码正确性。

#### Scenario: 同文件双源合并取强

- **WHEN** `src/auth/README.md` 既在元素 `docs` 内、又位于元素 `path` 覆盖目录内
- **THEN** `docs` 中该路径只出现一条，`source = "explicit"`（不重复出现两条）

#### Scenario: docs 指向不存在文件

- **WHEN** 元素 `docs = ["./src/auth/MISSING.md"]` 且该文件不存在
- **THEN** `warnings` 含 `docNotFound`（元素 FQN + 路径），`docs` 不含该条目；画布仍呈正常态（不停摆），图数据完整

#### Scenario: 稳定序

- **WHEN** 同一元素有三条同强度（`adjacent`）文档
- **THEN** 三条按 `path` 字典序排列；重复调用结果逐字相同

### Requirement: 文档位置与邻近扫描范围

模块文档 SHALL 与它描述的代码**同生命周期**：文档 MUST NOT 存放于 `<archi_dir>/` 内。

```
<archi_dir>/                横跨代码的架构资产
├── models/*.toml           模型
├── decisions/*.md          ADR
└── （reports/* 等归 check 侧）

模块文档跟代码走：
  src/auth/README.md        邻近约定（元素 path 覆盖目录内）
  任意位置                   用 docs 显式指向
```

邻近扫描 SHALL 严格限定在元素 `path` 的覆盖范围内（目录递归 / 单文件精确），MUST NOT 越出元素 `path` 去扫全仓。

邻近扫描的排除集合 SHALL 为**内置谓词**（测试目录、构建产物目录、VCS 目录，并对 `ignore` 的 gitignore 语义生效——`node_modules` / `target` 等恒不参与）。用户可配排除键集归 `desktop-archi-drift`「架构域看护配置」（落地形态为 `<archi_dir>/config.json`，路径由 `foundation` 派生）；本能力 MUST NOT 预建该配置面（不建文件读取、不建键集、不建 schema）。

`docs` / `path` 内的非 md 文件不参与邻近扫描；`decisions/` 内非 md 文件不参与 ADR 检索。

#### Scenario: 测试目录内 md 不算模块文档

- **WHEN** 元素 `path = ["./src/auth/"]`，目录内 `src/auth/README.md` 与 `src/auth/__tests__/fixture.md` 并存
- **THEN** `docs` 只含 `src/auth/README.md`；`fixture.md` 不出现（内置排除谓词命中测试目录）

#### Scenario: 扫描不越出 path 覆盖范围

- **WHEN** 元素 `path = ["./src/auth/"]`，而 `src/order/README.md` 也在盘
- **THEN** `src/order/README.md` 不出现在该元素的 `docs` 内（不扫全仓）

#### Scenario: 架构域目录内的 md 不算模块文档

- **WHEN** `<archi_dir>/models/notes.md` 在盘
- **THEN** 任何元素的邻近关联都不含该文件（架构域目录不是文档落点）

### Requirement: 看护与检索不读 specs（迁移边界）

本能力 SHALL 守住四条边界，使将来的「模块文档逐步替代 `openspec/specs/`」迁移不被本期实现反向锁死：

| # | 边界 | 理由 |
|---|---|---|
| 1 | 看护（结构校验 / 图诊断）不读 `specs` | 否则替代完成那天，看护逻辑要整体改写 |
| 2 | 检索不读 `specs` | 一旦纳入，用户会开始写「给 archi 看的 spec」，迁移被反向锁死 |
| 3 | 文档关联只认三种来源 | 不新增 spec 来源 |
| 4 | 门禁不依赖 `specs` ↔ element 映射 | 该映射将来由「模块文档 ↔ element」承担 |

本变更 MUST NOT 引入任何 `openspec/specs/` 读取路径、MUST NOT 引入 spec ↔ element 映射、MUST NOT 让 `specs` 参与检索语料。「模块文档替代 specs」的迁移（含 acceptance 可断言结构与归档 `SpecSync` 增量合并语义的对应物缺失问题）需独立变更裁决，本期零预铺。

#### Scenario: 检索不命中 specs 内容

- **WHEN** `query = "archi"` 且 `openspec/specs/` 内某文件路径含该词、某 ADR 亦提及
- **THEN** 命中项只来自模型元素与他人文档路径；`openspec/specs/` 下任何路径与内容均不出现

#### Scenario: 关联来源封闭

- **WHEN** 审查文档关联来源清单与 `docs` 输出
- **THEN** `source` 只取 `explicit` / `adjacent` / `text` 三值；无第四条来源、无 `specs` 来源，新增来源须改本 requirement

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
| `crates/core/archi/src/docs.rs` | 三源文档关联 | `element_docs(model, graph, root, fqn) -> DocsOutcome`：`explicit` / `adjacent` / `text` 三源、强度排序（同强度按 `path` 字典序）、同文件合并取强；条目恒 `{ path, title, kind, source }`；正文不返回 |
| `crates/core/archi/src/docs.rs`（排除谓词） | 邻近扫描范围 | 内置排除（测试 / 构建 / VCS 目录 + `ignore` 的 gitignore 语义）；扫描严格限于元素 `path` 覆盖范围；用户可配排除键集不在本期 |
| `crates/core/archi/src/docs.rs`（warnings） | 文档缺失 | `docNotFound`（元素 FQN + 路径）只报不拦：不进停摆态、不影响图数据、不阻断归档 |
| `crates/core/archi/src/docs.rs`（ADR 检索） | `decisions/*.md` 正文命中 | 正文出现元素 FQN 即命中，`kind = "adr"`、`source = "text"`；不读 `openspec/specs/` |
| `crates/core/archi/src/docs.rs`（关联来源面） | 来源封闭（零改动） | `element_docs` 仍恰 `explicit` / `adjacent` / `text` 三源；MUST NOT 新增 `<archi_dir>/api/**` 来源或 `api` 类 `kind` |
| `crates/core/archi/src/docs.rs`（邻近扫描范围） | 架构域目录排除（零改动） | `<archi_dir>/**` 内的 md 不作为任何元素的邻近文档（既有判据天然覆盖 `api/**`，本变更不新增规则） |
| `crates/core/archi/src/search.rs`（检索面） | 零改动 | 检索语料不含 `<archi_dir>/api/**`；元素薄条目与 ADR `text` 源不变 |
| `crates/core/archi/src/dto.rs`（`ArchiApiRef`，见 desktop-archi-api） | 契约状态的并列承载 | `ArchiModuleDetail.api` 为并列字段；MUST NOT 复用 `DocRef` 四字段形状或混入 `docs` 列表 |
| `crates/core/archi/src/dto.rs`（`DocRef`） | 线面契约 | `path` / `title` / `kind` / `source` 四字段；`specta::Type`；三面（工具 / IPC / 进程内）同一投影 |
