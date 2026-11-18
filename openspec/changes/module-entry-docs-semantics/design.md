# 设计: module-entry-docs-semantics

> **变更**: module-entry-docs-semantics（E-1 · 契约资产目录迁移 `api/` → `models/` + `graph/`）
> **日期**: 2026-11-18
> **修订**: 2026-11-18 · 回溯后二次定稿——提案层四条事实断言（绑定文件零 diff / 十六份金样逐字不变 / 两处 doc 散文串在改动集之外 / 留痕报告可直接沿用）已全部证伪并改写，本稿按修正后的 `proposal.md`（AC-1 / AC-3 / AC-4 / AC-5 / AC-8 / R-12…R-15）与 10 份 delta 重述实现形态：绑定面按「形状 / doc」两半判、金样给**显式一处例外**、两处 doc 散文串纳入实现文件表、留痕报告的处理口径两次校准（本稿曾定「须在资产搬净后重生成」，第三次修订改为**移交前置**，见下行）。
> **修订（第三次·边界重划）**: 2026-11-18 · 上一稿把「AC-4 的 41 条读数」与「`reports/archi/<timestamp>.json` 的留痕」列为本 change 的交付物，实测不可达成：宿主门禁二进制是变更前构建（`paths` 仍指 `api/` ⇒ `enabled = false`），任何相位都无权重建 / 重启它；而实现步序列末的 `archi_check` 每次都会在未跟踪的 `reports/archi/` 新写一份 `apiFindings: []`。故本稿重划边界（事实校准 11 / D17）：41 条读数与留痕报告改为**移交前置（handoff precondition）**——宿主以换段后源码重建后由门禁 / verify 相位留痕；`reports/archi/` 明确为**门禁运行时产物**（不入库、不计入交付面、不作判红面）。其余交付判据（代码与生成物差异口径、金样一处例外、零扩散白名单、文档纪律节）不变。
> **修订（第四次·交付面纪律）**: 2026-11-18 · 第三次修订把 AC-4 的在线读数划为移交前置后，阶段十前两项仍是「不勾的前置声明」，与 `tasks.md` 的「全部任务勾选」判据互斥；本稿把这两项改写成**本相位可判**的两条（判据面 41 条的离线推导 + `reports/archi/` 的交付面纪律，事实校准 12）。
> **修订（第五次·撤回隐藏 ACL）**: 2026-11-18 · 上一稿把「清空 `reports/archi/` 后关闭该目录的写权限」当作交付面纪律，那是一条**隐藏的本地 ACL**（不入库、不可见、只在 worktree 内生效），MUST NOT 保留：`reports/archi/` 的交付面纪律只有「**不入库即可**」一条——它作运行时产物出现 `??` 属预期，既不判红也不作证据。该 ACL 已从工作树移除、写权限恢复，本稿与 `tasks.md` 同步撤掉该动作；`write_report(...).ok()` 降级面只描述「报告落盘失败不阻断结论」，与是否施加 ACL 无关。
> **修订（第六次·与提案第二次修订逐条对表）**: 2026-11-18 · 本轮按已重划边界的 `proposal.md`（AC-4 / 第七节 ⑤ / R-15）与 `specs/desktop-archi-api-check/spec.md` 的「留痕报告是移交前置」条逐条对表复核本稿与 `tasks.md`，结论：**实现面与文稿面无需改动**，只需把可判性写死。故本轮只做两件事——① 把 AC-4 在 `AC Mapping` 与「落地实况」两处固化成**四面可机械判**（① `api::paths::directory` 体 == `archi_models_dir(root)`；② `api::enabled` 体逐字 `has_markdown(&paths::directory(root), 0)`；③ 离线可推 `path` 非空 44 − `api = "none"` 2 − 契约在案 1 = `apiMissing` 41 ⇒ `0 / 41`；④ 反向判据「`models/` 一级六份 toml 在场而递归零份 `*.md` ⇒ `enabled = false`」），② 记下 `reports/archi/` 的**盘上现状**：该目录当前为空（无任何报告文件），它既不是本 change 的交付物也不参与任何判红面——`apiFindings: []` 的报告、空目录、目录存在性三者 MUST NOT 充当 AC-4 证据，MUST NOT 施加隐藏 ACL / 权限护栏（不入库即可）。十条已通过的验收条目逐字未动。
> **基于**: `proposal.md`（病灶一至四 + 提案一至七 + AC-1…AC-11 + R-1…R-15）、`explore.md`（§E.5.3 / §E.5.5 / §E.5.6 / §E.9）、`specs/` 下 10 份 delta（`workspace-layout-resolution` / `desktop-archi-api` / `desktop-archi-api-check` / `desktop-archi-model` / `desktop-archi-contract-panel` / `desktop-archi-agent` / `desktop-crate-layout` / `desktop-archi-module-docs` / `desktop-archi-file-tree` / `repo-archi-model`）
> **性质**: **行为零变化的机械单元**。两棵树换段 + 一个派生族增删 + 三处路径单点换 base + 两处判据换根 + 索引说明段 / 文档纪律节 / 知识文本串跟随 + 生成物与绑定面的**已知定量随动**。零新增判据、零新增 IPC、零新增 crate、零 DTO **形状**改动、零前端产品代码改动。

---

## 起点实况（`HEAD` 基线）

| 面 | 实测 |
|---|---|
| 资产 | `.dev-team/architecture/api/INDEX.toml`（402 行、44 条 `[[entry]]`）、`api/Infra/App.Backend.Infra.ArchiRuntime.md`、同目录 `.schema.json`；`models/` 一级恰 6 个 `NN-<域>.toml`；`graph/` 不存在 |
| `INDEX.toml` 内嵌路径 | 第 3 行说明段 `契约面：目录 .dev-team/architecture/api …`；第 83 行 `schema = ".dev-team/architecture/api/Infra/App.Backend.Infra.ArchiRuntime.schema.json"`——换段后**恰这两行**变（其余 `.schema.json` 的 `$ref` 是 `./` / `../<Domain>/` 相对形态，零变化） |
| 生产代码触点 | 8 个文件：`foundation/src/layout/mod.rs`（`API_DIR_NAME` / `archi_api_dir` / `archi_api_rel_dir`）、`api/paths.rs`（`use` + `directory`）、`api/schema.rs:317`（`build_index` 说明段）、`api/mod.rs`（`enabled` 经 `paths::directory`，代码零改）、`dto.rs:83` / `dto.rs:361`（doc 方位串）、`infra/agent/src/archi_knowledge.rs`（知识注入）、`api/extract/mod.rs:5` 与 `model.rs:46`（**doc 散文方位串**，doc-only） |
| `paths::directory` 的三个消费点 | `api/mod.rs`（`enabled`）、`api/diff.rs`（孤儿扫描根）、`paths.rs`（`domain_dir` → `contract_path` / `schema_path`）——`index_path` 消费点两处：`api/mod.rs`（`render_all` 末位写盘目标）、`api/diff.rs` |
| 判据面基线 | `apiEntryMissing 0` + `apiMissing 41` = 41、`status = clean`、`passed = true`、`enabled = true`（`path` 非空元素 44 个）——**该读数出自变更前构建的宿主门禁**（`paths` 指 `api/` 时）；换段后本 change 侧只可离线复核（`44 − 2 − 1 = 41`），运行态读数属移交前置（事实校准 11 / D17） |
| 模型读数 | 53 元素 / 120 关系、`diagnostics` 空；收面 = `models/` 一级 + `.toml` 后缀（`parse_dir` 不递归），`drift::model_files` 同收面 ⇒ `modelHash` 与索引 / 契约子树无关 |
| 孤儿扫描机械锚点 | `api/diff.rs` 的 `include_str!` 自证断言（`diff_test.rs`）：`read_dir` 恰 1 处、`depth > super::MAX_API_WALK_DEPTH` 恰 1 处、`walk_source_files(root)` 恰 1 处、`pub fn ` 恰 1 处——换段必须保住这四条 |
| 字面量面 | `.dev-team/architecture/api…` 共 **51 行 / 18 个文件**，全在测试侧（`*_test.rs` 与 `tests/*`）；产品代码零命中；前端 3 个测试文件 4 行 DTO 载荷样例；`AGENT.md` / `CLAUDE.md` 各 2 行含字面量（第 26 / 30 行）+ 第 27 行「契约目录根」无字面量 |
| 出线面绑定 | `bindings.ts` 两行含 doc 方位串（`:633` `ApiStatus`、`:757` `ArchiApiRef.contractPath`）；`bindings.zod.ts` 零 doc 串 ⇒ 换段后恰前者 2 行随动、后者零 diff |
| 漂移金样 | `corpus/drift/expected/*.json` 十六份，其中仅 `api-mismatch.full.json` 的 `occurrences[].file` 内嵌契约路径（`:58`） |
| 版本 | `packages/desktop/package.json` 现值 `0.6.50`；`tauri.conf.json` 为 `"version": "../package.json"`；`src-tauri/Cargo.toml` 的 `[package] version = "0.1.0"` 不随动 |

---

## 落地实况（本轮修订时的工作树）

回溯发生在提案层，实现面已按「迁移动作 + 代码换段 + 测试侧换段」落地；下表按**已落地**与**移交前置**（本 change 无权产出、由后续相位闭合的读数面）两态对齐 tasks 的勾选状态——本稿修订后**无未闭合项**。

| 面 | 状态 | 实测 |
|---|---|---|
| 资产迁移（`git mv` 三份 → `models/Infra/` + `graph/`） | 已落地 | `git status` 三条 `R`；`.dev-team/architecture/` 只剩 `config.json decisions graph models`，`api/` 零命中 |
| `layout` 派生族（删 `api` 三符号、增 `graph` 族） | 已落地 | `archi_graph_dir` / `archi_graph_rel_dir` 在案，`archi_api*` 零命中 |
| `api/{paths,mod,diff,schema}.rs` 换段 | 已落地 | `directory` → `archi_models_dir`；`index_path` → `archi_graph_dir`；`INDEX_FILE_NAME` 已 `pub(crate)`；`collect_artifacts` 两处派生；说明段取 `archi_models_rel_dir` |
| `dto.rs` doc 换段 + `bindings.ts` 重导出 | 已落地 | `dto.rs:83` / `:361` 新串；`bindings.ts` diff **恰 2 行**（`:633` / `:757`）、`bindings.zod.ts` 零 diff |
| `archi_knowledge.rs` 两方位注入 | 已落地 | 「契约资产：」+「契约索引：」各 1 处；体量实测 **3646 字节**（≤ 4096） |
| `INDEX.toml` 重生成 | 已落地 | 差异**恰 2 行**（第 3 行说明段、第 83 行 `schema` 列） |
| 金样 | 已落地 | 15 份逐字节不变；`api-mismatch.full.json:58` 恰 1 行换 base |
| 文档纪律节（`AGENT.md` / `CLAUDE.md`） | 已落地 | 该节 3 行换段，两份逐字相同 |
| `package.json` 版本 | 已落地 | `0.6.50` → `0.6.51`（+1 patch） |
| 测试侧落点字面量与断言重写 | 已落地（归测试相位产物） | 18 个测试文件换段、`paths_test` 三处断言重写、`tree_test` 新用例、`archi_knowledge_test` 三处断言 |
| `api/extract/mod.rs:5` doc 散文串 | 已落地 | 第 5 行改指 `<archi_models_dir>/<Domain>/<FQN>.md`；该文件 diff 恰 1 行（`//!`） |
| `model.rs:46` doc 散文串 | 已落地 | 第 45–46 行改指「架构域 `models/<Domain>/` 目录」；该文件 diff 恰 1 行（`///`） |
| 两处 doc 串的机械守卫（`dto_test` 扩面）＋ `pilot_test` 旧位置负向断言 | 已落地（归测试相位产物） | `dto_test` 三份产品源文 `include_str!` + 旧方位串归零；`pilot_test` 有「`<archi_dir>/api` 目录不存在」负向断言 |
| **`reports/archi/<timestamp>.json`（门禁留痕）** | **移交前置（非本 change 交付物）；运行时产物，不入库即可** | 宿主门禁是变更前构建 ⇒ 它只能写出 `apiFindings: []`；该形态既是运行时正常输出、也不作证据（事实校准 10 / 11）。**盘上现状：该目录当前为空（零报告文件）**——空目录与 `apiFindings: []` 报告、目录存在性三者 MUST NOT 充当 AC-4 证据。41 条读数与报告由门禁 / verify 相位在宿主重建后留痕；`reports/archi/` 为运行时产物，不入库、不计入交付面、不作判红面（`??` 属预期，MUST NOT 施加隐藏 ACL / 权限护栏）（事实校准 11 / 12 / D17 / D18） |

---

## 事实校准（不改 spec 意图，只定实现形态）

1. **`api::enabled` 的「换根」是零代码改动**：函数体是 `has_markdown(&paths::directory(root), 0)`，`directory` 改指 `archi_models_dir` 后监测根自动跟随。故 `api/mod.rs` 本变更只改 doc（模块 doc、`enabled` 的 doc、`MAX_API_WALK_DEPTH` 的 doc），**零代码行变化**——spec「`api::enabled` 单点判定，MUST NOT 在命令层另判一套」因此天然成立。
2. **索引侧收面需要一个后缀常量，不能新增字面量**：`paths::INDEX_FILE_NAME`（`"INDEX.toml"`，原为文件私有 `const`）提升为 `pub(crate)`，供 `api/diff.rs` 作索引侧收面后缀。MUST NOT 在 `diff.rs` 写 `"INDEX.toml"`（同一文件名的第二个持有者），MUST NOT 新造 `INDEX_SUFFIX`（同值两名）。
3. **孤儿扫描的「两处派生」必须共用一个递归**：原 `collect_api_dir(directory, index, depth, found)` 的第三参是「索引文件的绝对路径」用于等值比较；改为 `collect_artifacts(directory, suffix, depth, found)`，第三语义位换成**收面后缀**，两处调用只差「根 + 后缀」两个实参。这一形态同时保住 `diff_test` 的四条自证断言。
4. **`starts_with(root)` 越根复核不在 `diff.rs`**：它在写盘单点 `crates/infra/archi/src/api_artifacts.rs`（零改动文件）。spec「三语义照搬」中的这一条本变更零触及，其用例只需夹具换段。
5. **`graph/` 目录由迁移动作显式建出**：`git mv` 要求目标父目录已存在，故迁移步骤为 `mkdir -p .dev-team/architecture/graph` + `git mv …/api/INDEX.toml …/graph/INDEX.toml`；目录搬入索引后即入库，MUST NOT 放 `.gitkeep`、MUST NOT 预建其他住户。写盘单点按 `GeneratedFile.path` 的父目录 `create_dir_all`（零改动）——生成面与本目录无耦合，任何顺序下都能自建。
6. **「DTO 形状零改动」不等于「绑定文件零 diff」（回溯修正 ①）**：specta 把 `dto.rs` 的 Rust doc 注释原样抄进 `bindings.ts`，而本变更恰要改两处 doc 方位串（`ApiStatus`、`ArchiApiRef::contract_path`）⇒ `bindings.ts` **必然**出 diff，且**恰 2 行**（`:633` / `:757`，全在 doc 注释内）。判据拆成两半：形状面（类型名 / 联合成员 / 字段名 / 字段序 / `null` 语义）零变化 + `bindings.zod.ts` 零 diff + `bindings:export` 幂等（重复导出不再产生第三行 diff）。MUST NOT 表述为「零 DTO 形状改动 ⇒ 绑定零再生成」。
7. **两处 doc 散文串纳入实现面（回溯修正 ③）**：`api/extract/mod.rs:5`（`<archi_dir>/api/<Domain>/<FQN>.md`）与 `model.rs:46`（「接口定义写在架构域 `api` 目录的契约资产里」）既不是引号字面量、也不含 `architecture/api` 形态，只按「符号 / 引号字面量」两层扫描会漏；而「旧位置清空」的语义又要求它们不许留。故按 **doc-only 一行**纳入实现文件表（取数器分派 / 签名归一 / `Element` 八字段 / `deny_unknown_fields` / 解析实现逐字不变），并由 AC-5 的第 ③ 层（旧方位串面）机械判。两处都不是「潜规则豁免」——豁免即与 AC-5 互斥。
8. **知识文本体量须实测后写死**：骨架 + 换段后的派生段实测 **3646 字节**（`≤ 4096` 上限），值写进 `archi_knowledge_test` 的体量断言；MUST NOT 预填估算值。
9. **「十六份金样逐字不变」是伪命题（回溯修正 ②）**：`occurrences[].file` 存的是 repo 相对契约路径，契约夹具暂存根由 `archi_models_rel_dir()` 派生 ⇒ 换段必然改 `api-mismatch.full.json` 的 `:58` **一行**。判据 = 十五份逐字节不变 + 该份恰 1 行（`modelHash` / `kind` / `category` / `message` / `line` / `specifier` / `typeOnly` 与其余字段逐字不变）。MUST NOT 因这一行而回退夹具落点，也 MUST NOT 写成「十六份逐字不变」。
10. **`apiFindings: []` 与「契约面未启用」同形（回溯修正 ④ 的事实面）**：`paths::directory` 已指 `models/` 而契约 md 尚未搬入的窗口里 `enabled = false`，门禁写出的是 `apiFindings: []` —— 形态上与「契约面未启用」完全同形，**不是**判过 41 条的干净报告。故它既不能充当 AC-4 证据，也不能被读成「判过了」。这一事实在本稿不产生任何本相位可执行的交付动作（其后果见事实校准 11）。
11. **AC-4 的 41 条读数与留痕报告是「移交前置」，不是本 change 的交付物（第三次修订的边界重划）**：宿主门禁二进制是变更前构建（`paths` 仍指 `api/` ⇒ `archi_api check` 读 `enabled = false`、`missing = [".dev-team/architecture/api/INDEX.toml"]`，`archi_module` 读 `api.contractPath = null`），**任何相位都无权重建 / 重启它**；同时实现步序列末的 `archi_check` 每跑一次就在未跟踪的 `reports/archi/` 新写一份 `apiFindings: []`。两条合起来使「本相位交付 41 条留痕」在结构上不可达成（三次 attempt 同一死因）；「交付面不出现 `[]` 报告」不靠关闭写盘，而靠**不入库**（见事实校准 12）。重划后的口径（提案 `AC-4` 正文按其第二次修订已改：本 change 侧判源码两面 + 离线推导 + 反向判据，运行态读数由**宿主重建后的门禁 / verify 相位**满足）：
    - **41 条读数与 `reports/archi/<timestamp>.json` 的生成** = **移交前置**：由门禁 / verify 相位在宿主以换段后源码重建 / 重启后留痕；本 change 侧的交付判据只到「换根实现正确 + 离线可推的读数」为止（`paths::directory` 指 `<archi_models_dir>`、`enabled` 判据形态零改动、53 元素中 `path` 非空 44 − `none` 2 − 契约在案 1 = `apiMissing` 41 ⇒ `apiEntryMissing 0`）。
    - **`reports/archi/` 是门禁运行时产物**：不入库（MUST NOT `git add`）、不计入交付面、不作判红面；`apiFindings: []` 是该构建的正常输出，不构成「判据漂移」。它作未跟踪目录出现在 `git status`（`?? …/reports/archi/`）属预期——既不判红也不作证据；MUST NOT 为让它「消失」而设隐藏 ACL / 权限护栏（交付面纪律只有「不入库」一条）。

12. **门禁运行时目录的交付面纪律（唯一动作 = 不入库；已撤回隐藏 ACL）**：变更前二进制每跑一次 `archi_check` 就往 `<change>/reports/archi/` 写一份 `apiFindings: []` 的报告（事实校准 10）。处置只有一条：**不入库**（MUST NOT `git add`）、不计入交付面、不作判红面——该目录作未跟踪目录出现（`?? …/reports/archi/`）属预期。上一稿曾「清空该目录后关闭其写权限」，那是一条**隐藏的本地 ACL**（不可入库、不可见、只在 worktree 内生效），已按第五次修订拆除（写权限恢复），MUST NOT 重新施加；runner 的 `write_report(...).ok()` 降级面只描述「报告落盘失败不阻断结论」，与是否施加 ACL 无关，MUST NOT 用它为 ACL 背书。
    - **评估口径（I7 对照须知）**：`reports/archi/` 内的报告出自**宿主运行时**（变更前构建的二进制），其 `enabled` / `apiFindings` / `scan.matched` 读数**不反映工作树源码**。D1…D17 的遵守与否只以工作树内的**源码 / 生成物 / 金样 / 文稿**为据（`paths::directory` 的体、`layout` 的派生族、`diff.rs` 的单递归、`INDEX.toml` 的两行差异、`AGENT.md` / `CLAUDE.md` 纪律节等）；该目录内的任何历史 / 残留报告 MUST NOT 用作决策违例的证据（同理也不能充当 AC-4 证据）。


---

## 关键设计决策

| # | 议题 | 定稿 | 理由 |
|---|---|---|---|
| D1 | 索引落点 | `INDEX.toml` 落 `<archi_dir>/graph/`（`archi_graph_dir`），MUST NOT 落 `models/` 根；`index_path` 由 `archi_graph_dir(root).join(INDEX_FILE_NAME)` 派生，**MUST NOT** 写成 `directory(root).join(…)` | `models/` 根收面是「一级 + `.toml` 后缀」，索引落根会被 `parse_dir` 当模型解析（`deny_unknown_fields` 收 `[[entry]]` ⇒ `unknownField` ⇒ 模型停摆），且 `drift::model_files` 同收面 ⇒ 全部金样换段。换家使冲突在目录层消失，模型收面判据零改动 |
| D2 | 契约目录的派生函数 | 不新增：直接复用既有 `archi_models_dir` / `archi_models_rel_dir`；MUST NOT 造 `archi_contract_dir` 一类改指别名 | 「一个目录两个名字」让函数名与语义永久错位；`archi_api_dir` 退役即退役，不留兼容壳 |
| D3 | `foundation::layout` 增删面 | 删 `API_DIR_NAME` / `archi_api_dir` / `archi_api_rel_dir`；增 `GRAPH_DIR_NAME`（私有 `const`，与既有常量组同形）/ `archi_graph_dir(root: &Path) -> PathBuf` / `archi_graph_rel_dir() -> String`；三者全从 `archi_dir` / `archi_rel_dir` 派生 | `graph` 段名的唯一触点集中在 layout（与 `models` / `decisions` / `config.json` 同一条隔离纪律）；`Layout` / `resolve` / `is_archi_rel_path` 零变化 |
| D4 | `paths` 三路径形态 | `directory(root)` 单行转发 `archi_models_dir(root)`；`index_path(root)` 直接从 `archi_graph_dir` 派生；`contract_path` / `schema_path` / `domain_dir` 零形态改动（只经 `directory` 的 base 换段） | 「索引与契约目录首次分家」必须在派生点分家；若 `index_path` 仍由 `directory` join 得出，将来把索引挪回契约目录只需删一层 `.parent()`——独立派生点是 R-2 的结构性防线 |
| D5 | 孤儿扫描形态（两处派生 / 一份递归） | `api_dir_artifacts(root)` 派生两处：`collect_artifacts(&paths::directory(root), SCHEMA_FILE_SUFFIX, 0, &mut found)` + `collect_artifacts(&archi_graph_dir(root), paths::INDEX_FILE_NAME, 0, &mut found)`；三语义（深度上限复用 `MAX_API_WALK_DEPTH`、`symlink_metadata` 不跟随、`read_dir` 只此一处）逐条照搬；两处唯一差异 = 根实参 + 后缀实参 | spec「MUST NOT 第二份手写递归」的机械形态就是「同一函数、两次调用」；`diff_test` 的三条计数断言因此逐字不变 |
| D6 | 旧位置残留的判红面（**机制诚实版**） | 孤儿枚举严格覆盖 `models/**` 与 `graph/` **两处派生** ⇒ `api/` 下的残留**不在 `api:check` 的枚举面内**。旧位置缺席由三条机械面守：① `pilot_test` 的「`<archi_dir>/api` 目录不存在」负向断言；② 目录树读数（`ls .dev-team/architecture` 零命中）+ `models/` 一级 `find -maxdepth 1` 读数；③ 迁移动作本身（`git mv` 与代码改动**同一提交**内完成）。MUST NOT 写「`api:check` 报应删未删」 | 为覆盖旧位置而把枚举根放宽到 `<archi_dir>` 会偏离「两处派生 / 根不同后缀不同」的 spec 形态，并让 `models/` 之外的任意 `.schema.json` 变成潜在判红面。spec delta 与本稿同批为「机制诚实版」，措辞不再与实现互斥 |
| D7 | `api::enabled` 语义显式化 | 判据形态零变化（递归含 ≥1 份 `*.md`、深度上限、符号链接不跟随）；doc 明写「被监测目录 = `<archi_models_dir>`；一级 `NN-<域>.toml` **不是**启用证据」；MUST NOT 收紧为 `<Domain>/<FQN>.md` 形态 | 收紧是行为变化，与判据面十三值同批（E-2）；本变更只承认「目录换根」 |
| D8 | 生成物差异口径 | `INDEX.toml` 差异**恰 2 行**（说明段目录串 + 已声明元素的 `schema` 列）；`.schema.json` / `.i.rs` / `.i.ts` 零字节变化。MUST NOT 表述为「零 diff」 | 说明段取 `<archi_models_rel_dir>`（索引与契约已分家，写成索引目录即指错地方），`schema` 列是 repo 相对路径、必然含 `models/<Domain>/` |
| D9 | 落地顺序 | ① `git mv` 三份资产（同提交）→ ② 改 layout 派生族 + 生产代码（同批，含两处 doc-only）→ ③ 测试侧字面量与断言语料 → ④ `api:export` 重生成 → ⑤ `api:check` 三列空。①② 必须同批：paths 已指新位而资产未搬会在新位**新建**文件；资产已搬而 paths 未改会把旧位当「应生成未生成」。**全量读数 41 条与留痕报告不在本序列内**——它们是宿主重建后的移交前置（D17） | 中间态允许不绿，提交前必须绿（R-6）；把「宿主二进制重建」写进本序列会让序列永远停在 ⑤（无相位有权重建宿主进程） |
| D10 | `graph/` 语义边界 | 本变更内 `graph/` **恰住 `INDEX.toml` 一份**；函数名对「契约索引的家」与「一切图派生资产的家」两种语义都成立，但扩住属独立变更，MUST NOT 预建第二住户、MUST NOT 为将来预留分支 | R-10；若最终改名 `archi_index_path` 属独立变更（一次符号替换） |
| D11 | 知识文本注入形态 | 派生段末句由「契约目录：`<archi_api_rel_dir>`」改为两个带冒号的标签方位：`契约资产：<archi_models_rel_dir>/<Domain>/<FQN>.md（手写真相）；契约索引：<archi_graph_rel_dir>/INDEX.toml` | spec Requirement 的「契约资产：…（索引 …）」括注形态与 `desktop-archi-agent` 的「『契约资产：』+『契约索引：』两条断言」逐字对齐；两种措辞取标签式一处落地，MUST NOT 同时保留括注句（同一方位两个写法必然漂移）。四面词表注入、纯函数零 IO、降级 `None` 语义逐字不变；注入串 MUST NOT 出现 `<>` 相邻两字符 |
| D12 | 仓库根文档纪律节口径 | `## 模块 API 契约` 节恰改 3 行（现第 26 / 27 / 30 行 = 第 ① / ② / ⑤ 条），要点数仍 5，`AGENT.md` 与 `CLAUDE.md` 该节正文逐字相同：① 手写真相 `models/<Domain>/<FQN>.md`；② `INDEX.toml` 方位 `.dev-team/architecture/graph/`；⑤ 索引与计数 `.dev-team/architecture/graph/INDEX.toml` | 第 27 行不含 `architecture/api` 字面量（写的是「契约目录根」），纯 grep 驱动的替换必然漏；验收按「三条要点逐条对拍」而非「字面量归零」（AC-11 / R-8） |
| D13 | 测试侧换段口径（归测试相位，本设计只钉判据） | 落点常量一律改经 `archi_models_dir` / `archi_graph_dir` 派生或改字面串；`paths_test` 的三条断言按新结构重写：「索引层级 = 契约目录本身」→「索引在 `graph/` 根、与契约目录分家」、「`directory` 与 `archi_api_dir` 同值」→「与 `archi_models_dir` 同值」、「来源文件零 `"api"` 字面量」保留并扩为「零 `"api"` / `"models"` / `"graph"` 段字面量 + 两个派生函数在案」；`dto_test` 的 doc 方位串守卫扩到 **三份产品源文**（`dto.rs` + `api/extract/mod.rs` + `model.rs`）；`pilot_test` 新增旧位置负向断言 | 这几条是旧落点的机械见证，不重写即与新结构矛盾；其余 51 行字面量为落点串，机械替换 |
| D14 | 零扩散白名单（**按修正后口径**） | 允许出 diff 的只有：本 change 目录的**文稿与规格面**、三份资产迁移与 `api/` 删除、`layout/mod.rs`、`api/{paths,mod,diff,schema}.rs`、`api/extract/mod.rs`（**doc-only 一行**）、`model.rs`（**doc-only 一行**）、`dto.rs`（doc-only）、`archi_knowledge.rs`、proposal「测试文件」表所列测试文件、`types/generated/bindings.ts`（**恰 2 行 doc 注释**）、`AGENT.md` / `CLAUDE.md`、`package.json`。零 diff 面：`api/check.rs`、`api/{contract,emit,render}.rs`、`docs/search/drift/scan/tree.rs`、`infra/archi/src/api_artifacts.rs`、`tools/archi-api-gen/src/main.rs`、`packages/desktop/src/**` 产品代码、`bindings.zod.ts`、`models/0*.toml`、`decisions/*` | 「白名单之外即误动」是 R-9 / R-12 的判红面；把 `types/generated/**` 整棵写成零 diff 会与 specta 抄 doc 的事实互斥。**`reports/archi/` 不在交付面内**（门禁运行时产物，见 D17）：它在 `git status` 里以 `??` 出现属预期，既不加分也不判红，「本 change 目录」这条白名单不为其背书 |
| D15 | 非契约产物随动的口径 | ① `bindings.ts` 恰 2 行 doc 注释（**形状零变化**，zod 零 diff，`bindings:export` 幂等）；② `api-mismatch.full.json` 恰 1 行 `occurrences[].file`（其余十五份逐字节不变）。两条都 MUST NOT 被读成「零 diff」，也 MUST NOT 被读成判据漂移 | 两者分别是 specta 抄 doc、`occurrences[].file` 存 repo 相对路径两件事实的必然后果；口径写错即让落地者误判（R-12 / R-13） |
| D16 | 两处 doc 散文串的判据落点 | 实现侧只改 `api/extract/mod.rs:5` 与 `model.rs:46` 两行 doc；机械守卫由测试相位在 `dto_test` 扩面（三份产品源文 `include_str!` + 旧方位串 `matches` 计数归零）。实现侧 MUST NOT 为它们造新符号或保留旧串做「历史说明」 | AC-5 的第 ③ 层（旧方位串面）必须有点名的机械面，否则「旧位置清空」在 doc 面永远只能靠 review 记忆 |
| D17 | 交付面与门禁运行时产物的分界（**第三次修订的核心**） | `reports/archi/` 是门禁运行时产物：**不入库**（MUST NOT `git add`）、**不计入交付面**、**不作判红面**；`git status` 出现 `?? openspec/changes/<change>/reports/archi/` 属预期。AC-4 的 41 条读数与留痕报告 = **移交前置**：宿主二进制是变更前构建（`paths` 仍指 `api/`），任何相位无权重建 / 重启，故读数由门禁 / verify 相位在宿主重建后留痕；本 change 侧判据止于「换根实现 + 离线可推读数」（事实校准 11）。`reports/archi/` 内的 `apiFindings: []` 是该构建的正常输出，既不作 AC-4 证据、也不构成判据漂移 | 上一稿把宿主进程的产出写成本相位的勾选项，使任务在结构上不可完成（三个 attempt 同一死因：既取不到 41 条，也无法从门禁手里拿走 `[]` 报告的书写）。边界重划后，交付面只由工作树内的可判面构成（代码 / 生成物 / 金样 / 文稿），跨进程读数归移交；「不让 `[]` 报告进交付面」由「不入库」达成，MUST NOT 用隐藏 ACL |
| D18 | 门禁运行时目录的交付面纪律（第四次修订的边界、第五次撤回 ACL） | 唯一动作是**不入库**：`reports/archi/` 作运行时产物留在工作树（`??` 属预期），不计入交付面、不作判红面、不作证据。MUST NOT 对该目录施加隐藏 ACL / 权限护栏（上一稿的写拒绝已拆除、写权限已恢复）；runner 的 `write_report(...).ok()` 降级面只描述「报告落盘失败不阻断结论」，与 ACL 无关，MUST NOT 用作 ACL 的依据 | 变更前二进制无法重建 ⇒ 它只能写 `apiFindings: []`；该形态已被 D17 排除在交付面外，剩下的纪律只有「别把它 `git add` 进来」。隐藏 ACL 既不可入库也不可见，等于让交付面「看起来干净」而状态藏在 worktree 里——正是本条要禁的形态 |

---

## 架构组件 (Architecture Components)

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|---|---|---|---|---|
| 布局派生族（**改**） | `api` 段退役、`graph` 段落地；契约目录复用 `archi_models_dir` | `crates/core/foundation/src/layout/mod.rs` | 纯 `std::path`，零 IO | 常量组 + 纯函数 |
| 契约路径单点（**改**） | `directory` → models；`index_path` → graph；`INDEX_FILE_NAME` 提升 `pub(crate)` | `crates/core/archi/src/api/paths.rs` | `foundation::layout` | 纯路径运算 |
| 启用判据（**doc 改，代码零改**） | `enabled` 经 `paths::directory` 自动换根；`has_markdown` 语义逐字不动 | `crates/core/archi/src/api/mod.rs` | `paths` | 有界 `read_dir` 递归 |
| 孤儿扫描（**改**） | 两个根共用一份有界递归（`models/**` 的 `.schema.json` + `graph/**` 的 `INDEX.toml`） | `crates/core/archi/src/api/diff.rs` | `paths`、`foundation::layout`、`scan::walk_source_files`（既有） | 参数化后缀的有界递归 |
| 索引说明段（**改**） | `build_index` 说明段取 `archi_models_rel_dir()` | `crates/core/archi/src/api/schema.rs` | `foundation::layout` | `toml_edit` |
| 契约面 doc 方位串（**doc-only 改**） | `dto.rs:83` / `:361`、`api/extract/mod.rs:5`、`model.rs:46` 四处方位串换段；三份文件的行为面零改动 | `crates/core/archi/src/dto.rs` / `api/extract/mod.rs` / `model.rs` | — | Rust doc（specta 抄进 `bindings.ts`） |
| 架构会话知识（**改**） | 注入「契约资产 + 契约索引」两个方位 | `crates/infra/agent/src/archi_knowledge.rs` | `foundation::layout`、`archi::dto` 词表 | 纯函数字符串拼装 |
| 契约资产与索引（**迁移**） | md + schema 进 `models/Infra/`；索引进 `graph/` | `.dev-team/architecture/{models/Infra,graph}/` | — | 数据资产（`git mv`） |
| 类型绑定（**生成物随动**） | `bindings.ts` 恰 2 行 doc 注释随 `dto.rs` 换段 | `packages/desktop/src/types/generated/bindings.ts` | `bindings:export`（specta） | 生成物（入库） |
| 写盘单点（**零改动**） | 按 `GeneratedFile.path` 父目录创建、越根复核 | `crates/infra/archi/src/api_artifacts.rs` | — | 文件边界 |
| 仓库根文档纪律节（**改**） | `## 模块 API 契约` 三条要点换段 | `AGENT.md` / `CLAUDE.md` | — | md（两份同源） |

依赖方向零新增边：`archi` 仍只依赖 `foundation`（既有边）；`infra/agent` → `archi` / `foundation` 既有边不变；`infra/archi` 与 `tools/archi-api-gen` 零改动。`core` 仍零写盘、零 infra 依赖。

### 数据流（改后）

```text
.dev-team/architecture/
├── models/                                   ← 双身份目录
│   ├── 0[1-6]-*.toml ──parse_dir（一级 + .toml，不递归）──> Model（53 元素 / 120 关系）
│   └── <Domain>/<FQN>.md ──contract::parse──> Contract ──> schema 装配
│                                   │
│                                   ├──> <Domain>/<FQN>.schema.json（同目录生成物）
│                                   └──> graph/INDEX.toml（说明段取 models_rel）
└── graph/
    └── INDEX.toml                            ← 索引的唯一落点（archi_graph_dir）

api::enabled   := <models> 递归含 ≥1 份 *.md（一级 toml 不算证据）
api::diff      := 盘上枚举（models/** 的 .schema.json + graph/** 的 INDEX.toml） − 应生成集
archi_knowledge:= 「契约资产：<models_rel>/<Domain>/<FQN>.md；契约索引：<graph_rel>/INDEX.toml」
dto → specta   := ApiStatus / ArchiApiRef 的 doc 方位串抄进 bindings.ts（恰 2 行）
```

---

## 变更清单 (Change Inventory)

### 新增文件

<!-- 本变更零新增源码文件：proposal「变更范围 - 实现文件」全为修改 / 迁移 / 删除。新增的四个磁盘对象（models/Infra/ 两份资产、graph/INDEX.toml）是 git mv 的落点，逐条登记在「修改文件」表的迁移行，不重复列为新增文件；graph/ 由 mkdir -p 建出后搬入索引（git mv 要求目标父目录已存在，见事实校准 5）。 -->

无。

### 修改文件

| 文件 | 具体修改 | 说明 |
|---|---|---|
| `.dev-team/architecture/models/Infra/App.Backend.Infra.ArchiRuntime.md` | `git mv` 自 `api/Infra/`，内容逐字不变 | 迁移 D1 / D2；`desktop-archi-api`「契约资产形态与唯一手写真相」的新落位 |
| `.dev-team/architecture/models/Infra/App.Backend.Infra.ArchiRuntime.schema.json` | 同上（字节不变） | `$ref` 是相对路径、`$id` 取文件名 ⇒ 零字节变化（D8） |
| `.dev-team/architecture/graph/INDEX.toml` | `mkdir -p graph/` 后 `git mv` 自 `api/INDEX.toml`，随后 `api:export` 重生成（差异恰 2 行：第 3 行说明段目录串、第 83 行 `schema` 列） | 索引换家（D1）；目录因搬入索引而入库，不放 `.gitkeep`（事实校准 5） |
| `.dev-team/architecture/api/**` | 删除（迁移后不存在；`git mv` 天然完成，MUST NOT 留空目录） | `api` 段退役；缺席判据见 D6 |
| `crates/core/foundation/src/layout/mod.rs` | ① 删 `const API_DIR_NAME: &str = "api";`，原位新增 `const GRAPH_DIR_NAME: &str = "graph";`（doc 改述为「契约索引子目录名」）；② 删 `pub fn archi_api_dir(root: &Path) -> PathBuf`，原位新增 `pub fn archi_graph_dir(root: &Path) -> PathBuf`（`archi_dir(root).join(GRAPH_DIR_NAME)`；doc 写明「契约索引唯一合法定位函数；消费者 MUST NOT 行内拼 `graph` 段；本变更内该目录恰住索引一份」）；③ 删 `pub fn archi_api_rel_dir() -> String`，原位新增 `pub fn archi_graph_rel_dir() -> String`（`format!("{}/{}", archi_rel_dir(), GRAPH_DIR_NAME)`；doc 写「`/` 分隔、零前导 `./`」） | D3；`archi_models_dir` / `archi_models_rel_dir` / `archi_decisions_*` / `archi_config_path` / `is_archi_rel_path` / `Layout` / `resolve` 逐字不动 |
| `crates/core/archi/src/api/paths.rs` | ① 模块 doc 的「契约目录一律经 `archi_api_dir` 派生」改为「契约目录经 `archi_models_dir`、契约索引经 `archi_graph_dir` 派生，MUST NOT 行内拼 `models` / `graph` 段」；② `use foundation::layout::archi_api_dir;` → `use foundation::layout::{archi_graph_dir, archi_models_dir};`；③ `const INDEX_FILE_NAME` 提升为 `pub(crate) const INDEX_FILE_NAME`（doc 改述为「写盘目标与孤儿扫描收面共用」）；④ `directory` 体改 `archi_models_dir(root)`；⑤ `index_path` 体改 `archi_graph_dir(root).join(INDEX_FILE_NAME)`（doc 写明 MUST NOT 由 `directory(root).join(…)` 派生）；⑥ `contract_path` / `schema_path` / `domain_dir` 的 doc 方位串换段（体不动） | D2 / D4 / D13；`domain_of` / `ref_relative` / `entry_*` / `EntryForm` / `generated_path` / `normalize_root` / `checked_root` / `is_fqn` / `is_repo_relative` 逐字不动 |
| `crates/core/archi/src/api/mod.rs` | 仅 doc：① 模块 doc 的启用条件段把「契约目录缺席」改为「`<archi_models_dir>` 缺席或其中递归零份 `*.md`」，并补「一级 `NN-<域>.toml` 不是启用证据」；② `enabled` 的 doc 改述被监测目录；③ `MAX_API_WALK_DEPTH` 的 doc 由「契约目录递归扫描」改为「契约资产与索引两处枚举共用的深度上限」 | D7；**代码行零变化**（`enabled` 体 `has_markdown(&paths::directory(root), 0)` 逐字保留） |
| `crates/core/archi/src/api/diff.rs` | ① 模块 doc「契约目录为有界本地枚举」改为「契约资产目录与索引目录为两处同形的有界本地枚举（同一递归、根与后缀两个实参不同）」；② `generated_files_on_disk` 的 doc 改述两个根；③ `api_dir_artifacts` 体改为两处派生（D5）；④ 私有 `collect_api_dir(directory, index, depth, found)` 改为 `collect_artifacts(directory: &Path, suffix: &str, depth: usize, found: &mut Vec<PathBuf>)`——收面判据由「文件名等于索引路径」换成「文件名以 `suffix` 结尾」，其余三语义逐字照搬 | D5；`compare` 与三列语义零改动；`read_dir` 仍恰 1 处、深度上限仍恰 1 处 |
| `crates/core/archi/src/api/schema.rs` | `build_index` 内 `let contract_dir = foundation::layout::archi_api_rel_dir();` → `archi_models_rel_dir();`（局部变量名保留，doc 补「说明段取契约资产所在目录，MUST NOT 取索引自身目录」） | D8；`$defs` 规则 / 装配 / 债表扫描 / `IndexEntry` 字段逐字不动 |
| `crates/core/archi/src/dto.rs` | doc-only：① `ApiStatus` 的 doc（`:83`）「架构域 `api` 目录」→「架构域 `models/<Domain>/`」；② `ArchiApiRef::contract_path` 的 doc（`:361`）`<archi_dir>/api/<Domain>/<FQN>.md` → `<archi_dir>/models/<Domain>/<FQN>.md` | 事实校准 6 / D15；字段名 / 顺序 / serde / specta 零变化；该 doc 经 specta 抄进 `bindings.ts` ⇒ 绑定恰 2 行随动 |
| `crates/core/archi/src/api/extract/mod.rs` | **doc-only 一行**：模块 doc 第 5 行 `<archi_dir>/api/<Domain>/<FQN>.md` → `<archi_models_dir>/<Domain>/<FQN>.md` | 事实校准 7 / D16 / AC-5 ③；取数器分派、签名归一、`extract` / `extract_element` 两个入口的能力面逐字不变 |
| `crates/core/archi/src/model.rs` | **doc-only 一行**：第 46 行 `api` 字段 doc 的「接口定义写在架构域 `api` 目录的契约资产里」→「架构域 `models/<Domain>/` 目录」 | 事实校准 7 / D16 / AC-5 ③；字段面（八项）、`deny_unknown_fields`、解析实现逐字不变 |
| `crates/infra/agent/src/archi_knowledge.rs` | `build` 的格式串末句由 `…；契约目录：{}` 改为 `…；契约资产：{}/<Domain>/<FQN>.md（手写真相）；契约索引：{}/INDEX.toml`，实参序由 `[models_rel, decisions_rel, api_rel]` 改为 `[models_rel, decisions_rel, models_rel, graph_rel]`；`build` 与模块 doc 的「契约目录相对路径」改述为「契约资产与契约索引两个方位」 | D11；四面词表注入、`join` 单点、降级 `None`、`<>` 拒绝逐字不动；体量实测 3646 字节（事实校准 8） |
| `packages/desktop/src/types/generated/bindings.ts` | `bindings:export` 重导出后入库：恰 **2 行 doc 注释**（`:633` `ApiStatus`、`:757` `ArchiApiRef.contractPath`）换段 | 事实校准 6 / D15；类型 / 联合成员 / 字段名 / 字段序零变化；`bindings:export` 幂等（重复导出不再产生第三行 diff） |
| `AGENT.md` | `## 模块 API 契约` 节恰 3 行换段（第 26 / 27 / 30 行 = 第 ① / ② / ⑤ 条）：① 手写真相 → `.dev-team/architecture/models/<Domain>/<FQN>.md`；② `INDEX.toml` 方位「契约目录根」→ `.dev-team/architecture/graph/`；⑤ 索引串 → `.dev-team/architecture/graph/INDEX.toml`；要点数仍 5，第 ③ / ④ 条逐字不变 | D12 / AC-11 |
| `CLAUDE.md` | 同上（该节正文与 `AGENT.md` 逐字相同） | 两份副本不分化（节内零 diff） |
| `packages/desktop/package.json` | `"version"` 相对落地时 `HEAD` 现值 +1 patch（`0.6.50` → `0.6.51`；落地时若 HEAD 已滚动则按新现值 +1） | 桌面源码改动，无例外；`tauri.conf.json` 经 `"version": "../package.json"` 跟随，`src-tauri/Cargo.toml` 的 `[package] version` 不随动 |

> **不在本清单的对象**（proposal「不要修改」+ D14）：`crates/core/archi/src/parse/**`、`api/check.rs` 及其语料语义、`api/{contract,emit,render}.rs` 的判据与渲染逻辑、`crates/core/archi/src/{docs,search,drift,scan,tree}.rs`、`crates/infra/archi/src/api_artifacts.rs`、`crates/tools/archi-api-gen/src/main.rs`、`packages/desktop/src/**` 产品代码、`packages/desktop/src/types/generated/bindings.zod.ts` 与其余生成面、`.dev-team/architecture/models/0*.toml`、`decisions/0001-model-granularity.md`、`openspec/specs/**`（10 个 delta 能力之外）。
>
> **`model.rs` / `api/extract/mod.rs` 的边界说准**：两文件各只允许**一行 doc 散文串** diff；字段面、解析实现、取数器分派与签名归一出现任何 diff 即误动（D16）。
>
> **测试文件面**（proposal「测试文件」表 18 个文件）：用例本体、断言值与新增用例归测试相位（test-design / test-gen / test-execution），本设计只在 D13 里钉住必须重写的断言口径、`dto_test` 守卫扩面与 `pilot_test` 负向断言两处机械面。

### 公共函数 / API

| 标识符 | 所在文件 | 变化 | 完整签名 | 说明 |
|---|---|---|---|---|
| `archi_graph_dir` | `crates/core/foundation/src/layout/mod.rs` | 新增 | `pub fn archi_graph_dir(root: &Path) -> PathBuf` | `<archi_dir>/graph`；契约索引 `INDEX.toml` 的唯一合法定位函数；纯推导零 IO |
| `archi_graph_rel_dir` | 同上 | 新增 | `pub fn archi_graph_rel_dir() -> String` | `<archi_rel_dir>/graph`；`/` 分隔、零前导 `./`；消费者限于知识文本与段名纪律登记 |
| `archi_api_dir` | 同上 | **删除** | `pub fn archi_api_dir(root: &Path) -> PathBuf`（原签名） | `api` 段退役；不保留改指别名（D2） |
| `archi_api_rel_dir` | 同上 | **删除** | `pub fn archi_api_rel_dir() -> String`（原签名） | 同上 |
| `directory` | `crates/core/archi/src/api/paths.rs` | 修改 | `pub fn directory(root: &Path) -> PathBuf` | 转发 `foundation::layout::archi_models_dir(root)`；契约目录 == 模型目录 |
| `index_path` | 同上 | 修改 | `pub fn index_path(root: &Path) -> PathBuf` | `foundation::layout::archi_graph_dir(root).join(INDEX_FILE_NAME)`；MUST NOT 由 `directory(root).join(…)` 派生（D4） |
| `contract_path` | 同上 | 修改（只换 base） | `pub fn contract_path(root: &Path, fqn: &str) -> Option<PathBuf>` | 结果变 `<archi_models_dir>/<Domain>/<FQN>.md`；FQN 形态判据与 `None` 语义零变化 |
| `schema_path` | 同上 | 修改（只换 base） | `pub fn schema_path(root: &Path, fqn: &str) -> Option<PathBuf>` | 结果变 `<archi_models_dir>/<Domain>/<FQN>.schema.json` |
| `INDEX_FILE_NAME` | 同上 | 修改（可见性） | `pub(crate) const INDEX_FILE_NAME: &str = "INDEX.toml"` | 由文件私有提升为 crate 可见：写盘目标（`index_path`）与孤儿扫描收面（`diff.rs` 第二处调用）共用同一持有者（事实校准 2） |
| `enabled` | `crates/core/archi/src/api/mod.rs` | 修改（语义换根，**代码零改**） | `pub fn enabled(root: &Path) -> bool` | 判据 = `<archi_models_dir>` 递归含 ≥1 份 `*.md`（经 `paths::directory` 自动换根）；一级 `*.toml` 不构成启用证据 |
| `build_index` | `crates/core/archi/src/api/schema.rs` | 修改（说明段串） | `pub fn build_index(entries: &[IndexEntry], meta: &ApiMeta) -> String` | 说明段「契约面：目录 …」取 `archi_models_rel_dir()`；条目装配与序零变化 |
| `build` | `crates/infra/agent/src/archi_knowledge.rs` | 修改（仅格式串与实参） | `pub(crate) fn build(skeleton: &str) -> Result<String, String>` | 派生段增「契约资产 / 契约索引」两个方位；骨架校验、四面词表、`Err` 文案零变化 |

> 私有函数不进本表：`api::diff::api_dir_artifacts` / `collect_artifacts`（D5）、`api::paths::domain_dir`（体不动）、`archi_knowledge::join`。CLI 面零新增子命令，IPC 面零新增命令。
>
> **无签名变化的两处 doc-only 文件**：`api/extract/mod.rs` 的 `extract_element` / `extract` 与 `model.rs` 的 `Element` 字段面均无签名 / 字段变化，故不进本表——它们只在「修改文件」表占**一行 doc 散文串**（D16）。

### 类型定义

<!-- 本变更零类型改动：`ArchiApiRef` / `ApiStatus` / `ArchiApiCheck` / `ArchiApiMeta` / `IndexEntry` / `Element` 的字段名、类型、顺序、serde / specta 派生逐字不变（仅 doc 注释串换段）；无新增 struct / enum / type alias。注意：doc 注释经 specta 出线到 `bindings.ts`（2 行），故「类型零变化」不蕴含「绑定文件零 diff」（事实校准 6）。 -->

无。

### 配置

| 配置键 | 所在文件 | 变化 | 值类型 | 默认值 / 现值 | 说明 |
|---|---|---|---|---|---|
| `version` | `packages/desktop/package.json` | 修改 | `string` | `HEAD` 现值 `0.6.50` → `0.6.51`（+1 patch；落地时按 HEAD 现值递增，MUST NOT 照抄本表数字） | `tauri.conf.json` 经 `"version": "../package.json"` 跟随；`src-tauri/Cargo.toml` 的 `[package] version = "0.1.0"` MUST NOT 随动 |

> 配置项零新增：`package.json` 的 `api:export` / `api:check` / `check` 三段命令串逐字不变；`.dev-team/config.json`、`.dev-team/architecture/config.json`、`crates/core/config/**`（含 `config.schema.json` 镜像）、`tauri.conf.json`、`capabilities/default.json` 全部零 diff。

---

## 数据模型 (Data Model)

本变更零新增持久化结构，只改**磁盘落点**与派生规则。

### 架构域目录树（迁移后）

| 路径 | 形态 | 持久化 / 生成 | 本变更的作用 |
|---|---|---|---|
| `<archi_dir>/config.json` | 文件 | 手写 | 零改动 |
| `<archi_dir>/decisions/*.md` | 文件 | 手写 | 零改动 |
| `<archi_dir>/models/0[1-6]-<域>.toml` | 一级文件 | 手写（模型声明） | 零内容改动；仍是 `parse_dir` / `drift::model_files` 收面的全部（53 / 120 与 `modelHash` 不变） |
| `<archi_dir>/models/<Domain>/<FQN>.md` | 二级文件 | 手写（唯一真相） | 新增落点（`git mv`）；对模型收面不可见（子目录不递归） |
| `<archi_dir>/models/<Domain>/<FQN>.schema.json` | 二级文件 | 生成物（入库） | 新增落点（`git mv`）；字节不变 |
| `<archi_dir>/graph/INDEX.toml` | 文件 | 生成物（入库，全局唯一） | 新增落点（`git mv` + 重生成，恰 2 行差异） |
| `<archi_dir>/api/**` | — | — | **不存在**（AC-1 正读判据；判红面见 D6） |

### 契约资产的身份判据（零改动，本变更只承认）

| 面 | 判据 | 归属 |
|---|---|---|
| 模型文件 | `models/` **一级** + `.toml` 后缀（`read_dir` 逐条 `is_file()`，不递归、不按 `NN-` 前缀筛） | `parse::parse_dir`（零改动） |
| 契约 md / schema | `<Domain>/` 子目录内的 `<FQN>.md` / `<FQN>.schema.json` | `api::paths`（base 换段） |
| 契约索引 | `<archi_dir>/graph/INDEX.toml`（`archi_graph_dir`） | `api::paths::index_path`（换派生点） |
| 启用证据 | `<archi_models_dir>` 递归 ≥1 份 `*.md` | `api::enabled`（根随 `directory` 换段） |

### 生成物字节差异（D8）

| 生成物 | 落点 | 差异 |
|---|---|---|
| `INDEX.toml` | `<archi_dir>/graph/` | **恰 2 行**：第 3 行说明段「契约面：目录 `<models_rel>`」、第 83 行 `ArchiRuntime` 的 `schema` 列（`models/Infra/…`） |
| `<FQN>.schema.json` | `<archi_models_dir>/<Domain>/` | 0 字节（`$id` / `$ref` 均不嵌目录） |
| `<入口主干>.i.rs` / `.i.ts` | 实现入口同级 | 0 字节（落点是入口的函数） |

### 非契约产物两处随动（D15）

| 面 | 随动 | 不变量（判红面） |
|---|---|---|
| `packages/desktop/src/types/generated/bindings.ts` | 恰 **2 行 doc 注释**（`:633` `ApiStatus`、`:757` `ArchiApiRef.contractPath`）——specta 抄 `dto.rs` 的 doc | 类型 / 联合成员 / 字段名 / 字段序零变化；`bindings.zod.ts` 零 diff；`bindings:export` 幂等 |
| `crates/core/archi/tests/corpus/drift/expected/api-mismatch.full.json` | 恰 **1 行**（`:58` `occurrences[0].file` 换 base） | 该份 `modelHash` / `kind` / `category` / `message` / `line` / `specifier` / `typeOnly` 逐字不变；其余 15 份逐字节不变 |

### 门禁运行时产物（**不属交付面**，D17）

| 面 | 形态 | 口径 |
|---|---|---|
| `<change>/reports/archi/<timestamp>.json` | 门禁每跑一次就覆盖 / 新增一份（未跟踪）；**现盘上该目录为空（零报告文件）** | **不入库、不计入交付面、不作判红面**；宿主是变更前构建 ⇒ 一旦运行就只写出 `apiFindings: []`（与「契约面未启用」同形，不作 AC-4 证据；空目录同理不作证据）。41 条读数与干净报告的留痕是**移交前置**：宿主以换段后源码重建后由门禁 / verify 相位产出 |

### DTO 面（形状零变化，doc 出线 2 行）

`ArchiApiRef` 的 `contractPath` / `schemaPath` / `generatedPath` 取值随路径派生自然换段；字段集、顺序、`null` 语义零变化。`bindings.zod.ts` 零 diff；`bindings.ts` 的 diff 全在 doc 注释内（2 行）。

---

<!-- 路由 / API 设计 (Route / API Design) 整段省略：本变更零新增 IPC 命令（八命令清单与出参形状逐字不变）、零新增 HTTP 端点、零新增 CLI 子命令（`archi-api-gen` 的 `generate` / `--check` / `bootstrap` 三子命令形态与退出码不变）、零新增数据库表。变化的只是已被这些入口消费的后端路径派生值。 -->

---

## 依赖 (Dependencies)

| 类别 | 依赖 | 用途 | 变化 |
|---|---|---|---|
| 运行时（Rust） | `std::path` / `std::fs` | 路径派生与有界 `read_dir` 枚举 | 零新增外部 crate |
| 运行时（Rust） | `foundation`（workspace 内） | `archi_models_dir` / `archi_graph_dir` 派生 | 既有边，消费者扩容（`api::diff` 新增对 `archi_graph_dir` 的调用） |
| 运行时（Rust） | `toml_edit`（`api/schema.rs` 既有） | `INDEX.toml` 装配 | 零变化 |
| 运行时（Rust） | `serde` / `serde_json` / `sha2` / `specta`（既有） | 索引装配 / 摘要 / DTO 出线（doc 抄进 `bindings.ts`） | 零变化 |
| 构建 / 测试 | `cargo fmt` / `clippy` / `cargo test`（既有段） | 门禁四段 | 零新增工具；`package.json` 脚本面零变化 |
| 构建 / 测试 | `bindings:export`（specta 生成） | `bindings.ts` 重导出（2 行 doc 随动） | 命令串零变化，仅执行一次 |
| 构建 / 测试 | `git mv`（vcs 工具） | 资产迁移（保留历史） | 一次性动作，非依赖 |

`Cargo.toml` / `Cargo.lock` 零新增依赖、零新增 workspace 成员；`archi` 的零 `workflow` 依赖与 `workflow` ⇄ `archi` 互不依赖两条红线零变化。

---

## 验收标准回应 (AC Mapping)

| AC | 设计落点 | 判据形态 / 观测 |
|---|---|---|
| AC-1 目录树换段干净 | D1 / D2 / D6 + 修改文件表四行迁移 | `api/` 零命中；`find models -maxdepth 1 -name '*.toml'` = **6**（无 `INDEX.toml`）；`models/Infra/` 1 md + 1 schema；`graph/` 恰 1 索引。**旧位置缺席的判据面（机制诚实版）**：`api:check` 的孤儿枚举严格两处派生，`api/` 残留**不可见** ⇒ 由 ① `pilot_test`「`<archi_dir>/api` 目录不存在」负向断言、② 目录树读数、③ 同提交 `git mv` 三条守；MUST NOT 写成「`api:check` 报应删未删」 |
| AC-2 生成物差异恰两行 | D8（`schema.rs` 说明段取 `archi_models_rel_dir`；`paths` 换 base） | `INDEX.toml` 第 3 行 + 第 83 行差异，其余 400 行与全部 `.schema.json` / `.i.rs` 零字节；`api:check` 三列皆空、退出码 0（桌面端执行）。**不是「零 diff」** |
| AC-3 模型读数零变化（金样显式一处例外） | 零 `parse` / `drift` 改动 + `graph/` 在 `models/` 之外 + `<Domain>/` 子目录不递归 + 事实校准 9 | 53 / 120、`diagnostics` 空、`modelHash` 逐字不变；**15 份金样逐字节不变 + `api-mismatch.full.json:58` 恰 1 行换 base**（该份其余字段逐字不变）。反向判据：索引复制到 `models/` 根 ⇒ `unknownField` + `modelHash` 变化 |
| AC-4 契约面换根零语义变化（**本 change 判据 = 源码两面 + 离线可推 + 反向判据；读数面为移交前置**） | D7（`enabled` 零代码改动）+ D17（边界重划）+ D18（交付面纪律 = 不入库）+ 事实校准 11 / 12 | **本 change 侧判据（四面，逐面可机械判）**：① `api::paths::directory` 实现体 == `archi_models_dir(root)`（契约目录与模型目录同址）；② `api::enabled` 实现体逐字 `has_markdown(&paths::directory(root), 0)`——判据形态零改动、换根经 `directory` 单点传导、命令层无第二处判断；③ 离线可推读数 `path` 非空 44 − `api = "none"` 2 − 契约在案 1 = `apiMissing` **41**，配 `apiEntryMissing` 0 ⇒ `0 / 41`、`status = clean`，与基线逐项一致；④ 反向判据「`models/` 一级六份 `NN-<域>.toml` 在场而 `models/**` 递归零份 `*.md` ⇒ `enabled = false`、`apiFindings = []`」（用例 `api/mod_test.rs::enabled模型文件不是启用证据` 在案），放入一份契约 md ⇒ 重新启用，两例共用同一判定函数。**移交前置**：`enabled === true`、`apiFindings` 长度 41、`passed = true` 与 `reports/archi/<timestamp>.json` 的干净留痕须**宿主以换段后源码重建 / 重启后**由门禁 / verify 相位产出——宿主二进制是变更前构建（`paths` 仍指 `api/`），本 change 任何相位无权重建，故该读数**不是本 change 的勾选项**；`reports/archi/` 为运行时产物（盘上当前为空目录；不入库、不作证据、不作判红面；`[]` 报告与空目录 MUST NOT 充证据；MUST NOT 施加隐藏 ACL） |
| AC-5 段名纪律（四层，各层判什么说准） | D3 / D4 / 事实校准 2 / D16 | **① 符号面**：产品 `.rs`（`*_test.rs` 除）内三符号零命中。**② 引号字面量面**：`"api"` 作为架构域路径段的字面量零命中；本层**不覆盖 Rust 模块名**（`archi::api` / `src/api/` 仍在）。**③ 旧方位串面**：产品 `.rs` 内 `architecture/api` 与 `<archi_dir>/api/` 零命中，含两处 doc 散文（`api/extract/mod.rs:5`、`model.rs:46`）；测试侧判据是 `.dev-team/architecture/api` **夹具落点串**归零，而 `architecture/api` 作为**否定断言的被检串**允许留存（现 8 行，全在断言 / 注释），MUST NOT 用「全仓零命中」当判据。**④ 段拼接面**：`models` / `graph` 段行内拼接零命中（唯一触点 `layout/mod.rs`；`infra/store` 的 `.field("models", …)` 是 Debug 标签、不属判据面）。反向判据：任一层被破坏即红 |
| AC-6 知识文本换段 | D11 + 事实校准 8 | 输出含 `<models_rel>/<Domain>/<FQN>.md` 与 `<graph_rel>/INDEX.toml` 两方位、零 `architecture/api`；四面词表与 4096 上限不变（实测 3646 字节）；`archi_knowledge_test` 的引导语 / 派生串 / 体量三处断言同批更新 |
| AC-7 两处判据换根不换语义 | D5（同一 `collect_artifacts` 两次调用，根 + 后缀两实参不同） | `models/<Domain>/Ghost.schema.json` → `orphaned`；`graph/INDEX.toml` 内容不符 → `mismatched`；深度上限 / 符号链接不跟随各一条用例；`starts_with(root)` 复核驻写盘单点（零改动，事实校准 4）；`read_dir` 仍恰 1 处 ⇒ 零第二份递归 |
| AC-8 零扩散（绑定面按形状 / doc 分开判） | D14 + D15 ① + 事实校准 6 | 改动集 ⊆ 白名单（`reports/archi/` 不在白名单语义内：运行时产物、未跟踪 `??` 属预期、不入库、不作判红面——事实校准 12 / D18）；`src/**` 产品代码零 diff；`bindings.zod.ts` 零 diff；`bindings.ts` 的 diff **恰 2 行且全在 doc 注释内**、类型 / 联合成员 / 字段名 / 字段序零变化；`bindings:export` 幂等。**MUST NOT 写成「`types/generated/**` 零 diff」** |
| AC-9 版本纪律 | 配置表 `version` 行 | `package.json` +1 patch（相对落地时 HEAD 的 `0.6.50`）、`tauri.conf.json` 保持 `"version": "../package.json"`、`src-tauri/Cargo.toml` 的 `[package] version` 逐字不变 |
| AC-10 相关能力零漂移 | 修改文件表的「不在本清单的对象」段 + D14 | `openspec/specs/**` 除 10 个 delta 能力外零 diff；`check.rs`、`api/{contract,emit,render}.rs`、`tools/archi-api-gen/src/main.rs`、`infra/archi/src/api_artifacts.rs` 零 diff |
| AC-11 文档纪律节三条要点换段且两份不分化 | D12 | 两文件该节各改 **3 行**（26 / 27 / 30 = ① / ② / ⑤）；`grep 'architecture/api'` 与节内 `grep '契约目录根'` 均零命中；要点数仍 5；两份该节正文逐字相同（节内零 diff、全文 diff 仍恰 2 处且全在节外）。反向判据：只改 `AGENT.md` ⇒ 该节 diff 出线即红 |

---

## 待决问题 (Open Questions)

| # | 事项 | 现状 | 处置 |
|---|---|---|---|
| OQ-1 | 旧位置 `<archi_dir>/api` 缺席的机械面 | **已闭合**：孤儿枚举严格两处派生，故缺席改由 `pilot_test` 负向断言 + 目录树读数 + 同提交 `git mv` 守（D6）；spec delta 已同批改为「机制诚实版」 | 负向断言本身是测试相位产物（tasks 移交说明列明）；若日后确需机械守旧段，单开变更把枚举根放宽到 `<archi_dir>` |
| OQ-2 | `graph/` 的长期语义（契约索引的家 vs 一切图派生资产的家） | **开放（不影响本变更）**：本变更按「契约索引的家」落地，目录内恰住 `INDEX.toml` 一份（D10） | 扩住属独立变更；若最终定名 `archi_index_path`，改名 = 一次符号替换 |
| OQ-3 | `api/extract/mod.rs:5` / `model.rs:46` 的旧 doc 散文串 | **已闭合**：回溯后已纳入实现文件表（doc-only 一行各一处），并由 AC-5 ③ 与 `dto_test` 守卫扩面机械判（D16 / 事实校准 7） | 无遗留；实现侧 MUST NOT 以「不在字面量扫描面内」为由豁免 |
| OQ-4 | `enabled` 对 `models/**` 任意 `*.md` 的误启用 | **已知留白**：判据形态保持「递归任意 `*.md`」（D7 / R-3） | 收紧为 `<Domain>/<FQN>.md` 形态属行为变化，与判据面十三值同批（E-2）；本变更只钉「模型文件不是启用证据」这一半 |
| OQ-5 | 知识文本体量（4096 字节上限） | **已闭合**：实测 3646 字节（骨架 + 换段后派生段），值已写进体量断言 | 若后续措辞调整逼近上限，先删修饰语，MUST NOT 删方位串 |
| OQ-6 | 留痕报告的生成与入库时机 | **已闭合（边界重划，D17）**：`reports/archi/` 是门禁运行时产物——不入库、不计入交付面、不作判红面；`git status` 的 `?? …/reports/archi/` 属预期。41 条读数与干净报告是**移交前置**，由门禁 / verify 相位在宿主以换段后源码重建后留痕 | 本 change 不再产出、不再等待该文件；`apiFindings: []` 是变更前构建的正常输出，既不判红也不作 AC-4 证据。REQ「留痕报告须非空」在 spec 面保留，其验证落点在宿主重建后的门禁相位 |
