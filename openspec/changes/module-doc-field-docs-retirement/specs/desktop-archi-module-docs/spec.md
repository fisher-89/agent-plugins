# desktop-archi-module-docs Specification（delta: module-doc-field-docs-retirement）

## Purpose

本 delta **整份退役**模块文档关联能力（三源关联、恒四字段条目、合并去重、`docNotFound` 只报不拦、文档位置与邻近扫描范围、契约资产不经 docs 三源）：`docs: Vec<String>` 与整个 `docs.rs` 被 `doc: Option<String>` 单值取代，入口文档改为**并列面**（`api = declared` 走派生落点 `<models>/<Domain>/<FQN>.md`，`api = none` 走 `doc` 声明），检索面改由 `archi_search` 的入口文档语料承担（见 `desktop-archi-agent`）。

> **承接方（删前必须指定，本 delta 同批落地）**：本能力仍有效的纪律不随本能力消失，逐条有新落点——① 「检索不读 `openspec/specs/`」（原「看护与检索不读 specs」第 2 条）与② 「不返回正文」、「不提供跨元素反向索引」（原「三种文档关联来源与恒四字段契约」的两条封死口径）→ `desktop-archi-agent` 的 ADDED requirement「检索面纪律与迁移边界」；③ 「看护（结构校验 / 图诊断）不读 `specs`」与「门禁不依赖 `specs ↔ element` 映射」（原「看护与检索不读 specs」第 1 / 4 条）→ `desktop-archi-drift` 的 ADDED requirement「看护与门禁不读 specs（迁移边界）」。原第 3 条边界（「文档关联只认三种来源」）**随本能力一并作废**——它断言的对象（三源关联）已不存在，不留空壳。

## REMOVED Requirements

### Requirement: 三种文档关联来源与恒四字段契约

**Reason**: 能力整体退役——E-1 把契约资产搬进 `<archi_dir>/models/<Domain>/` 后，入口文档落进架构域树内，而 `docs.rs` 对架构域树整棵硬排除（`is_archi_rel_path`）⇒ 入口文档**不可能**经三源出线；且既有的「模块文档 MUST NOT 存放于 `<archi_dir>/` 内」位置纪律与本变更的前提直接相反。替代形态是**并列面**：`api = declared` 元素的入口文档恒为派生落点 `<models>/<Domain>/<FQN>.md`（= `api.contractPath`，不写也不许写），`api = none` 元素以 `doc: Option<String>` 单值声明。条目形状随之退化为**一条路径字符串**（`ArchiModuleDetail.doc: Option<String>` / `ArchiSearchResult.doc: Vec<String>`），`DocRef` 的 `title` / `kind` / `source` 三字段无消费者（`search.rs::matches_doc` 命中只看 `path` 与 `file_stem`，`title` 本就零作用）。

**Migration**: 模型元素 `docs = [...]` → 删该行；`api = none` 且需要入口文档的元素改声明 `doc = "<repo 相对路径>"`（单值），`api = declared` 元素零声明（入口文档即 `contract_path`）。消费面：`ArchiModuleDetail.docs` → `doc`（单值，仅 `api = none` 有值）、`ArchiSearchResult.docs: Vec<DocRef>` → `doc: Vec<String>`、`SearchScope::Docs` → `Doc`（线格式词 `doc`）。判定面：原 `docNotFound`（读 `docs` 数组、只报不拦）由 `ArchiApiFindingKind::DocNotFound`（读 `doc` 单值、`error` 档、纳管 `status`）承接——**名字复用而非能力延续**，新值不读数组、不停留在「只报不拦」。

### Requirement: 合并去重与 docNotFound 只报不拦

**Reason**: 合并去重的对象（多源命中同一文档）随三源消失而不存在；`docNotFound` 的「只报不拦」语义被本变更**反向改写**——新 `docNotFound`（`doc` 声明指不到）是 `error` 档且纳管 `status`，因为「声明指不到」是纯笔误、无中间态可辩，放任即「把 `doc` 指向不存在文件」的消红后门。文档缺失不再有「不影响门禁」的豁免：它要么是「还没写」（`docMissing`，`warn`，不纳管），要么是「写错了」（`docNotFound`，`error`，纳管）。

**Migration**: 旧 `ArchiModuleDetail.warnings: Vec<ArchiWarning>`（`WarningKind::DocNotFound`）随之退役——`WarningKind` 成为空枚举故整体删除，`ArchiWarning` 零消费者同批退役。替代读数是 `apiFindings` 内的 `docMissing` / `docNotFound`（定义与档位归 `desktop-archi-api-check`「判据级档位与元素 gate 解耦」）。前端与 agent 侧的「文档缺失只报不拦」表述 MUST 同批删除，MUST NOT 保留为兼容分支。

### Requirement: 文档位置与邻近扫描范围

**Reason**: 位置纪律**反转**——入口文档必须住在 `<archi_dir>/models/<Domain>/` 内（与模型同树），故「模块文档 MUST NOT 存放于 `<archi_dir>/` 内」不再是判据；邻近扫描（`path` 覆盖目录递归收 `*.md`）与 ADR 检索（`decisions/*.md` 正文含 FQN）整族随 `docs.rs` 删除。内置排除谓词（测试 / 构建 / VCS 目录）的唯一消费者也是邻近扫描，一并退场——文件树与漂移扫描的自有排除面（`desktop-archi-file-tree` / `desktop-archi-drift`）不属本能力，零改动。

**Migration**: 原「元素 `path` 目录内的 `*.md`」不再被模型关联，改为普通仓库文件（需要时由入口文档链接、由文件树看见、由通用文件搜索命中）。ADR（`decisions/*.md`）**退出 archi 自动检索语料**——这是显式接受的次生后果，三条替代抵达路径为：① 入口文档内的 markdown 链接；② 左栏文件树；③ 通用文件搜索工具。`archi_decisions_dir` / `archi_decisions_rel_dir` / `DECISIONS_DIR_NAME` 三个派生符号随唯一非测试消费者（`docs.rs`）一并删除，`decisions/` 目录与 `0001-model-granularity.md` 文件本体留在盘上——退役的是派生单点，不是资产；将来若需程序化访问须重新引入单点，MUST NOT 手写目录字面量。

### Requirement: 看护与检索不读 specs（迁移边界）

**Reason**: 本 requirement 的四条边界中，第 1（看护不读 `specs`）、第 2（检索不读 `specs`）、第 4（门禁不依赖 `specs ↔ element` 映射）三条**仍然有效**，但它们的承载体（文档关联与检索面）已迁出本能力——留在本能力等于把有效纪律挂在已删除的资产上（AGENT.md：退役资产不得留空壳）。第 3 条（文档关联只认三种来源）随三源退役作废，不留替代。故本 requirement 整体移出，有效三条由承接方以同一 change 内的 ADDED requirement 承接。

**Migration**: 三条有效边界的新落点——「检索不读 `openspec/specs/`」→ `desktop-archi-agent`「检索面纪律与迁移边界」；「看护（结构校验 / 图诊断）与门禁不依赖 `specs ↔ element` 映射」→ `desktop-archi-drift`「看护与门禁不读 specs（迁移边界）」。本变更 MUST NOT 引入任何 `openspec/specs/` 读取路径、MUST NOT 引入 `spec ↔ element` 映射；「模块文档替代 `specs`」的迁移（含 acceptance 可断言结构与归档 `SpecSync` 增量合并语义的对应物缺失问题）仍需独立变更裁决，本变更零预铺。

### Requirement: 契约资产不经 docs 三源

**Reason**: 三源已不存在，本条的被禁止对象（「契约资产成为第四种来源」「`kind` 新增 `api` 值」「检索命中契约正文」）失去意义——契约 md 就是 `api = declared` 元素的**入口文档本身**，它经 `api.contractPath` 与 `doc` 并列出线，不再需要「不经三源」的封死口径。反向纪律（模型 MUST NOT 把契约路径写进文档字段）的形态也随之改写：`api = declared` 元素 MUST NOT 写 `doc`（写了即 `apiUnexpected`），这条新纪律归 `repo-archi-model`「基线落位与模型文件划分」与 `desktop-archi-api`「契约资产形态与唯一手写真相」。

**Migration**: 四条封死口径的去处——① 来源面封闭：随三源退役作废；② 「archi 树整棵不可见」的排除机制：随邻近扫描退役（零替代规则，MUST NOT 新增目录白名单）；③ 反向禁写 `docs`：改写成 `repo-archi-model` 的字段纪律「`doc` 仅 `api = none` 元素可写，且 MUST NOT 指向 `<archi_dir>/models/<Domain>/**` 与 `openspec/specs/**`」；④ 「`archi_module` 的 `api` 是并列字段而非 `docs` 承重」：`api` 并列字段保留，`docs` 侧承重面消失。检索面「MUST NOT 命中契约正文」由 `desktop-archi-agent`「archi_search 只读工具的轻量检索契约」的语料定义承接（语料只含路径串、零正文）。

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/archi/src/docs.rs` | **（退役删除）** | 三源关联（`explicit` / `adjacent` / `text`）、强度排序与合并去重、`DocsOutcome` / `DocWarning`、`normalize_rel` / `is_repo_relative`、`resolved_within`、`title_of`、ADR 读盘与分发、邻近扫描与内置排除谓词——整文件删除；`file_stem` 迁驻 `crates/core/archi/src/search.rs`（检索命中仍在用） |
| `crates/core/archi/src/docs_test.rs` | **（退役删除）** | 与 `docs.rs` 同批删除；其「不读 `specs`」用例的语义面由 `desktop-archi-agent` 的检索面 requirement 重述 |
| `crates/core/archi/src/dto.rs` | **（退役删除部分）** | `DocRef` / `DocKind` / `DocSource` / `WarningKind` / `ArchiWarning` 全数退役；`ArchiModuleDetail.docs` 与 `.warnings` 删、`.doc: Option<String>` 加；`ArchiSearchResult.docs` → `doc: Vec<String>`；`SearchScope::Docs` → `Doc`（线格式词 `doc`）；`dto::linked_doc_paths` 退役 |
| `crates/core/archi/src/search.rs` | 检索面（语义换面） | 语料 = `path` 非空元素的**在盘入口文档载体**（declared 取派生 `contract_path`、none 取 `doc` 声明），条目为路径串；命中判据仍只吃 `path` + `file_stem`（不读正文） |
| `crates/core/foundation/src/layout/mod.rs` | **（退役删除部分）** | `DECISIONS_DIR_NAME` / `archi_decisions_dir` / `archi_decisions_rel_dir` 三符号删除；`decisions/` 目录本体不动 |
| `openspec/specs/desktop-archi-module-docs/` | **（退役删除）** | 本 delta 归档合并时整目录删除（无残留 requirement、无空壳、无 `TBD`）；有效纪律的承接地见 Purpose 段的承接方段 |
