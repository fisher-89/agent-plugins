# 设计: archi-rule-agent-prompt-sync

> **变更**: archi-rule-agent-prompt-sync
> **日期**: 2026-11-19
> **基于**: `proposal.md`（四项改造 / 变更范围 21 行实现文件 / AC-1…AC-12 / R1…R12）、`explore.md`（只读）与五份 delta spec（`desktop-archi-agent` 4 ADDED + 5 MODIFIED；`desktop-archi-drift` 1 ADDED + 1 MODIFIED；`desktop-agent-execution` 1 ADDED + 3 MODIFIED；`desktop-write-protection` 1 ADDED + 3 MODIFIED；`desktop-phase-prompts` 1 MODIFIED）
> **性质**: 四项同批收口——**工具面前置**（`hasPath` / `resolvedEntry` / `archi_scaffold` / `archi_drift`）→ **收口前引擎自检**（`preclose` + `assets_hash`）→ **按相位锁定模型写权**（`SessionInjections.protect` 第五位 + 会话级保护规则）→ **知识分层**（骨架重写 + prompt 分流）。判定域（`drift::check` / `api::paths::entry_state` / 诊断九类）、模型资产（54 元素 / 122 关系）、门禁 runner 与命令面、`AGENT.md` / `CLAUDE.md` 全部零改动。

---

## 事实校准（提案 vs 代码实测）

不改 spec 意图，只把提案行文与代码实况的偏差点摆明并裁定：

1. **`hasPath` 不会进 TS 生成物**：`bindings.ts` 由 `tauri_specta` 按 **IPC 命令签名**收集类型（`src/bindings/mod.rs::builder` 只挂 `all_commands!`）；实测 `ArchiElementBrief` 只出现在 `crates/core/archi/src/dto.rs` 与 `dto_test.rs`，**不是任何命令的返回类型**（`archi_search` 工具面不经 IPC），故它当前根本不在 `bindings.ts` 内。裁定：AC-10 的生成物机械面实际只覆盖 `ArchiModuleDetail.resolvedEntry`（+ 新类型 `ArchiResolvedEntry` / `EntryVia`，经 `archiModule` 命令可达）；`hasPath` 的消费方是**工具响应**（`archi_module` 概览），零生成物 diff，`pnpm run bindings:export` 后应当**零变化**于 `ArchiElementBrief` 词面（若出现即为收集面被改，属异常）。
2. **`EntryProblem` 文案已有一处单点**：`crates/core/archi/src/api/mod.rs` 的私有 `fn entry_problem_reason(&paths::EntryProblem) -> String` 是 `bootstrap` 失败文案的素材（与 `check::entry_missing_message` 同源的四值措辞）。裁定：`resolved_entry.problem` **复用**它（仅把该函数可见性放宽到 `pub(crate)`，零判据改动、零第二份措辞），MUST NOT 在 `dto.rs` 再抄四个 `format!`。提案实现文件表未列该文件，本设计补入（见「修改文件」）。
3. **`SessionInjections` 加第五位不是「additive 且零改动」**：`#[serde(default)]` 只保**线格式**（旧 JSON 缺字段读 `None`）；产品与测试里的**穷尽字面构造**会全部编译红。实测构造点分布：`sdk/runner_test.rs`(16) / `sdk/loop_test.rs`(16) / `core/agent/src/runner_test.rs`(14) / `sdk/tools_test.rs`(7) / `sdk/mount_test.rs`(6) / `sdk/mount.rs`(3，其中 1 处为 `#[cfg(test)]` 夹具) / `compose_test.rs`(3) / `core/agent/src/lib_test.rs`(2) / `compose.rs`(2 处产品臂——恰好是 `assemble_injections` 的两条产品返回) / `core/agent/src/kernel_test.rs`(1)。裁定：产品侧改动面收敛到 `compose.rs` 两处 + `mount.rs` 夹具；测试侧为**机械闭合面**（逐处补 `protect: None` 或 `..Default::default()`，零断言语义改动）。
4. **`ProtectRules::load` 调用点实测 47 处命中（含 `loop_test.rs` 的一处注释；生产调用恒 1 处 = `sdk/runner.rs`）**。裁定：`load` 签名**逐字不变**（`load(workspace_root: Option<&Path>) -> ProtectRules`），会话规则经 `with_session` 追加——47 处调用零改动，这正是选「builder 追加位」而非「load 加形参」的硬理由（提案 D16 的事实支撑）。
5. **`assets_hash` 的遍历不引 `ignore`**（对提案括注的偏离）：`ignore::WalkBuilder` 尊重 `.gitignore` / 全局 gitignore，会让**同一份盘面**因忽略规则状态得到不同哈希——与 spec「同一份盘面必得同一值」直接冲突（扫描面可以按 gitignore 收敛，哈希面不行）。裁定：私有 `collect_assets` 用 `std::fs::read_dir` 递归三处资产根（模型目录 / 契约资产目录 / 索引文件），按**相对路径**排序后累加；零新依赖（`ignore` 在本 crate 已在，但不用于哈希）。
6. **`[[relation]]` 空表会触发 `missingField`**：模板要求「草稿过 `TomlParser` 零结构诊断」（AC-2 的自证锚），而一条只有表头、无 `from` / `to` / `desc` 的 `[[relation]]` 会被解析成缺必填字段的关系。裁定：草稿的**关系部分只能是注释行**（`# [[relation]]` / `# from = …`），元素部分只保留一行实值 `kind`（其余字段注释化）。
7. **草稿的 FQN 必须单段**：多段 FQN（如 `App.Auth`）而祖先未声明会触发 `orphanHierarchy`，与「零结构诊断」冲突。裁定：草稿用单段占位 `[element."NewElement"]` + 一行注释「改成你的完整 FQN；多段时先声明祖先」。
8. **`preclose::run` 需要「前值」入参**：提案签名为 `run(&sender, turn, touched)`，但 T2 的判据是「本轮起点 vs 收口时 `assets_hash` 不相等」，起点值必须在 loop 起手时捕获。裁定：新增 `preclose::baseline(turn) -> Option<String>`（archi 会话才读盘，`spawn_blocking` 内求值），`run` 增 `baseline: Option<&str>` 形参——三函数面见「公共函数 / API」。
9. **T1 的写入面判定点**：loop 已把 `write` / `edit` 的入参改写为 canonical 绝对路径（`sandbox_rewrite` 之后）、并在保护闸判定后才执行。裁定：`archi_touched` 在「沙箱通过 ∧ 保护闸放行 ∧ 执行前」置位（此时 `sandboxed` 在手、零额外 IO），跨轮累积；`bash` 经 `protect::written_paths` 复用同一写面提取面。`touched` 的假阴面（Windows 盘符 / 大小写导致的 `cwd` 前缀失配、`inline` / 重定向等 `written_paths` 未覆盖的写面）**一律由 T2（`assets_hash` 变化）兜底**——不为它扩提取面、不加路径大小写折叠（折叠会引入第二份路径语义）。
10. **知识骨架体量口径**：现值 3071 B（派生段 ≈ 546 B，全文 3617 B）。裁定：骨架门限 **≤ 2252 B**（2.2 KiB）、全文门限 **≤ 2867 B**（2.8 KiB）；实现后按实测值冻结断言（目标区间骨架 2000–2150 B、全文 2550–2750 B），MUST NOT 以「接近门限」冒充达标。

---

## 关键设计决策

| # | 议题 | 定稿 | 理由 |
|---|---|---|---|
| D1 | 实现序 | 四批依次：① 工具面前置（`hasPath` / `resolvedEntry` / `archi_scaffold` / `archi_drift`）→ ② 收口自检（`assets_hash` + `preclose`）→ ③ 相位锁（`protect` 第五位 + 会话规则）→ ④ 知识分层（骨架重写 + prompt 分流） | 提案 D1 / R2：散文先删而工具面没补齐即「知识缺失」，比现状更差；④ 的每一句都指向 ①/② 的产出的字段 |
| D2 | `hasPath` | `ArchiElementBrief.has_path: bool`（线格式 `hasPath`，**字段不省略**），由 `to_brief` 单点填 `!element.path.is_empty()`；`module_overview` / `search` 两消费面自动继承 | 提案 D6；纯投影不补默认值（既有投影纪律） |
| D3 | `resolvedEntry` | 新增 `ArchiResolvedEntry { path: Option<String>, via: Option<EntryVia>, problem: Option<String> }` + `EntryVia { Explicit, Convention }`（serde camelCase：`explicit` / `convention`）；`ArchiModuleDetail.resolved_entry` 必填三字段；判据单点 `api::paths::entry_state`：`Resolved` → `path` + `via`（由 `element.entry.is_some()` 派生）+ `problem: null`；`Invalid(problem)` → `path`/`via` 皆 `null` + `problem` 取 `api::entry_problem_reason`；`Absent` → 同形 + 固定文案「未声明 `entry`，且约定默认候选皆不在盘」。`EntryForm`（crate / subdir / single-file）**不出线**——本字段只回答「入口在哪、凭什么」 | 提案 D7；三态可辨是 `entry` 纪律从散文移出的前置；`form` 无消费方（引入即多一个要维护的出线值） |
| D4 | 资产哈希 | `archi::drift::assets_hash(root: &Path) -> String`：四类资产（`graph/model/*.toml` + `models/**/*.md` + `models/**/*.schema.json` + `graph/INDEX.toml`），路径经 `foundation::layout` 派生；累加 helper 与 `model_hash` 共用（私有 `hash_files(files: &[PathBuf]) -> String`：相对路径 + 字节，稳定序，`sha256:` 前缀）；缺失目录不参与、单文件读失败按空字节；遍历用 `read_dir` 递归（事实校准 5） | 提案 D11 + 新调用面的前提；`modelHash` 口径逐字不变（仍只覆盖 model toml） |
| D5 | `archi_drift` 工具 | 恒 `mode: Full` + `changed: &[]` + `change: None` + `CancelFlag::never()`；参数面只有 `scope`；值集单点 `DRIFT_SCOPES: [&str; 3] = ["summary", "entries", "files"]`（落 `archi_drift_view.rs`），`mount` 的 JSON `enum` 与执行体 `Err` 文案都取它；`Instant` 计时后进 `drift_summary`；`cap_result` 收口；描述点名「`gate` 全 `off` 时 `unmatchedFiles` 是常态不是错误」与「`files` 只在排查归属问题时用」 | 提案 D9 / D18；本仓基线 302 条 unmatched 是常态——报告直出即 15–25 KB 噪声 |
| D6 | 投影单点 | `sdk/archi_drift_view.rs`：`drift_summary(&Report, duration_ms: u64) -> Value`、`drift_entries(...)`、`drift_files(...)`（逐级追加，`entries` = `summary` + 观察项 + 判据条目；`files` = `entries` + 扫描四类清单）；收口 notice 复用 `drift_summary` 并追加 `changedAssets` | 提案 D10；与 `api::findings` 的「装配单点」同款纪律 |
| D7 | `archi_scaffold` 工具 | 参数 `kind`（可选，enum 值集由 `ElementKind::ALL` 派生，省略取 `domain`；值集外 `Err` 点名四值）；响应 `{ "draft": "<TOML>" }`：单段占位 FQN + 一行实值 `kind` + 其余字段**注释行**（各附一行纪律：`path` 叶必填 / `./` 前缀 / 目录尾 `/` / 容器恒空；`metadata` 必须内联表；`entry` 只在约定默认指不到时写；`doc` 仅 `api = none` 可写）+ 关系注释块；零写盘；过 `cap_result` | 提案 D8 + 事实校准 6/7；「模板可解析」是分层的自证锚（示例围栏删除后由它兜住 `R1`） |
| D8 | 收口自检三函数 | `preclose::baseline(turn) -> Option<String>`（`preamble` 缺席即 `None`，否则 `spawn_blocking(assets_hash(cwd))`）；`preclose::touched(name, input, cwd) -> bool`（`write` / `edit` 取 `path`、`bash` 取 `written_paths`；前缀由 `foundation::layout::archi_graph_rel_dir()` / `archi_models_rel_dir()` 派生，命中 `<archi>/graph/**` ∨ `<archi>/models/**`）；`preclose::run(sender, turn, touched, baseline)`（判 `preamble.is_some()` → 判 T1 ∪ T2 → `spawn_blocking(drift::check(Full, [], None, never))` → notice / 日志，fail-open） | 提案 D12–D14 + 事实校准 8；`preamble.is_some()` 判会话（**不用** `turn.archi.is_some()`——change 相位也下发 archi 工具） |
| D9 | 挂点与事件序 | `loop.rs` 自然收敛支：`preclose::run(...)` **先于** `finish_success`；五个出口的其余四个零调用；`archi_touched` 是 `run` 的局部 `bool`（`'continuation` 循环**之外**声明、跨轮累积；零 `LoopTurn` 新字段——`loop_test.rs` 的 `LoopTurn` 构造面因此零改动） | 提案 D13 + `desktop-agent-execution`「事件序」；notice 先于 `TurnDone` 是可视面唯一保证；跨轮累积是 T1 的正确性前提（写入常发生在收口前的另一轮） |
| D10 | 会话锁载体 | `core/agent`：`SessionProtectRule { glob: String, reason: String }` + `SessionInjections.protect: Option<Vec<SessionProtectRule>>`（`#[serde(default)]`、无 `specta::Type`、`Clone + Eq`）；`ProtectRules::with_session(self, Option<Vec<SessionProtectRule>>) -> ProtectRules` 纯数据合并；`load` 签名不动（事实校准 4） | 提案 D16；四条注入位都是「本会话能做什么」，第五位同性质；`specta` 缺席即零 TS 影响 |
| D11 | 规则序 | `rules::with_session_rules(rules: Vec<Rule>, session: Option<&[SessionProtectRule]>) -> Vec<Rule>`：把会话规则**插在**内置段之后、用户段之前（插入位由 `builtin_rules().len()` 派生，MUST NOT 硬编码 `1`）；`Rule` 的会话形态为 `ReasonKind::Session(template)` | 提案 D16；位次即优先级、首个命中即定（判定侧零优先级分支）；会话串带指路，不能被用户自选 reason 盖掉 |
| D12 | 拒绝文案 | `reason.rs` 新增 `SESSION_LOCK_TEMPLATE`（四要素：路径 + 工具名 `%t` / 「架构模型只在 `proposal` / `dev-design` 相位的执行侧（及架构页会话）可写」/ 补救路径（写进 `design.md` / `tasks.md` 交 `dev-design`，或改用架构页会话）/「不要改用其他命令或工具尝试绕过」）+ `pub(crate) fn session_lock_template() -> &'static str`；`deny_reason` 增 `ReasonKind::Session` 分支（`%s` / `%t` 全量替换） | 提案 D17 + `desktop-write-protection`「拒绝指令文案」；模板单点在 `reason.rs`，`compose` 只取串构造规则（零第二份文案） |
| D13 | 装配单点 | `compose::model_write_lock(provenance) -> Option<Vec<agent::SessionProtectRule>>`：`source == "archi"` → `None`；`source == "change"` ∧ `change_phase_role(source_ref) = Some((phase, "executor"))` ∧ `phase ∈ {proposal, dev-design}` → `None`；其余（含畸形 `source_ref` / 未知 phase / `explore` / `debug`）→ `Some([一条])`；glob = `format!("**/{}/**", foundation::layout::archi_graph_model_rel_dir())`，reason = `protect::session_lock_template()` | 提案 D15 / D17；fail-closed 方向与工具面「零 archi 工具」同向；锁面只 `graph/model/**`（契约 md / `INDEX.toml` / 看护 `config.json` 不锁） |
| D14 | 写面提取复用 | `protect::written_paths(command: &str, cwd: &Path) -> Vec<String>`：`\\`→`/` 归一 → `shell::extract_file_ops` 的 **`OpKind::Write` + `OpTarget::Path`** → `rules::resolve` 归一；`Revert` / `Delete` 不入（删除经 T2 兜底）；`shell.rs` 零改动 | 提案实现文件表；「同一写面提取面」是 `preclose` 的前提，MUST NOT 第二份解析 |
| D15 | 工具面矩阵 | `archi_matrix` 由 7 行改 **8 行表**（`dev-design` 执行 / 评估拆行 + `drift` / `scaffold` 两维）：`(proposal, executor)` = `search` + `module` + `scaffold`；`(dev-design, executor)` = `module` + `search` + `drift` + `scaffold`；`(dev-design, evaluator)` = `module` + `search`；`(test-design, executor)` = `search`；`(implement, executor)` = `module` + `api` + `drift`；`(code-review, evaluator)` / `(acceptance, evaluator)` = `module`；`_ => None`。archi 臂名字清单 = 七内建 + 五 archi 名；`mint_archi_context` 的 any 判据扩为五名 | 提案 D18；矩阵单源仍在 `archi_matrix` 一处（未命中即零 archi 工具）；`drift` 只给两处直接产出漂移的相位（R6 保守案，`code-review` / `acceptance` 不勾） |
| D16 | 知识分层与骨架 | `archi-knowledge.md` 重写为 B/C 两档内容（A 档全部改为工具指路、D 档归 `archi_scaffold`）：删示例 TOML 围栏 / 字段面语法清单 / `entry` 三档长表 / `doc` 判据细节；`path` 压成「叶必填 + `./` 前缀 + 目录尾 `/` + 容器恒空」一段 + `hasPath` 指路；新增 `kind` 四值语义、计数复述单源、架构域看护配置三块散文；`archi_knowledge.rs` 派生段删契约索引（保留四面词表 + 模型目录 + 契约资产 = 两处方位） | 提案 D2–D5 / R4；四面词表保留是 `desktop-archi-model`「消费面词汇集一致性对拍」的既有承诺；契约资产目录保留是「新契约 md 的唯一写入定位」 |
| D17 | A 档指路表（机械断言素材） | 纪律句关键词 → 工具名 → 工具面字段：`path` 缺不缺 → `archi_module` → `hasPath`；`entry` 解析 → `archi_module` → `resolvedEntry`；`doc` 仅 `api = none` → `archi_drift` / `archi_module` → `apiFindings` 的 `apiUnexpected`；祖先先声明 → `archi_module` / `archi_drift` → 结构诊断 `orphanHierarchy`；字段形态 → `archi_scaffold` → `draft`；漂移读数 → `archi_drift` → `counts` / `errors` / `warnings`；契约两轴 → `archi_api` → `artifacts` / `findings`；计数单源 → `archi_drift` → `model.elements` / `model.relations`；看护配置非法 → `archi_drift` → `Err` 文案 | 提案「知识分层与工具面同批移动」；这张表是 `archi_knowledge_test` 的断言面（实现相位只落文本，断言归测试相位） |
| D18 | prompt 分流 | 两处写权相位（`proposal-planner` / `dev-design-planner`）：补工具名 + 写面句（可经 `write` / `edit` 维护模型、无专用写工具、只为已确定元素落笔、写完用 `archi_drift` / `archi_module` 自查）；其余 12 份各补一句「架构模型已锁」指路（中文措辞、零 ASCII `archi`、evaluator 零 `backtrack`）；`implementation-generator` 另补 `archi_drift` 的「改完代码自查未声明依赖」 | `desktop-phase-prompts`「模型写面按相位分流」；两半对称（写权侧 / 锁定侧按同一 `(phase, role)` 白名单） |
| D19 | 生成物 | `pnpm -C packages/desktop run bindings:export` 重生成（非手改）：diff 预期 = `ArchiModuleDetail.resolvedEntry` + `ArchiResolvedEntry` + `EntryVia`；`ArchiElementBrief` 零出现（事实校准 1） | AC-10；零手改是硬线 |
| D20 | 版本 | `packages/desktop/package.json` `0.7.2` → `0.7.3`；`src-tauri/tauri.conf.json` 经 `"version": "../package.json"` 跟随；`src-tauri/Cargo.toml` 的 `version = "0.1.0"` **不动** | 仓库版本纪律 |
| D21 | 明确不做（出 range） | ① 判定域（`drift::check` / `entry_state` / 诊断九类 / 扫描定级）零改动；② 模型与契约资产零改动（`api:check` 零 diff）；③ `AGENT.md` / `CLAUDE.md` 零改动（含「按来源裁剪」与 `doc` / `api` 双写去重）；④ 门禁 runner 与 `archi_drift` IPC 命令零改动；⑤ 画布不自动刷新（知识句保留）；⑥ `archi_drift` 不给 `code-review` / `acceptance`、无增量模式、不注入 `vcs`；⑦ `BulkGlobs` 不加相位条件（R8 如实留痕）；⑧ 无新 crate / 新跨 crate 依赖 / 新 npm 依赖 / 新 DB 表 | 逐条对应提案「明确不做」；每条都是另一件事的边界 |

---

## 架构组件 (Architecture Components)

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|---|---|---|---|---|
| 投影单点（**改**） | `ArchiElementBrief.has_path`；`ArchiResolvedEntry` + `EntryVia` + `ArchiModuleDetail.resolved_entry`；两投影函数填值 | `crates/core/archi/src/dto.rs` | `crate::api::paths::entry_state` / `api::entry_problem_reason` | 纯投影（`serde` + `specta::Type`） |
| 入口文案单点（**改·可见性**） | `entry_problem_reason` 由私有放宽为 `pub(crate)`，供 `dto` 复用同一四值措辞 | `crates/core/archi/src/api/mod.rs` | `paths::EntryProblem` | 零逻辑改动 |
| 资产哈希（**改**） | `assets_hash`（四类资产聚合）+ 与 `model_hash` 共用的累加 helper | `crates/core/archi/src/drift/mod.rs` | `foundation::layout`、`sha2`、`std::fs` | 纯读盘、零写盘、零 spawn |
| 定义面（**改**） | 两新工具定义（`archi_drift` 三 scope enum + 常态句；`archi_scaffold` 的 `kind` enum ← `ElementKind::ALL`）+ 挂载清单 13 → 15 | `crates/infra/agent/src/sdk/mount.rs` | `archi::dto::{ElementKind, GateTier}`、`rig::completion::ToolDefinition` | 编译期静态清单（`OnceLock`） |
| 执行体（**改**） | `drift_payload`（scope 解析 → 计时 → `drift::check(Full, [], None, never)` → 投影 → `cap_result`）、`scaffold_payload`（kind 解析 → 模板） | `crates/infra/agent/src/sdk/archi_tool.rs` | `archi::drift`、`archi_drift_view`、`tools::cap_result` | 同步执行体（工具面读面，零写盘） |
| 引擎侧投影单点（**新增**） | `drift_summary` / `drift_entries` / `drift_files` + `DRIFT_SCOPES`；工具响应与收口 notice 共用 | `crates/infra/agent/src/sdk/archi_drift_view.rs`（新） | `archi::drift::Report`、`serde_json` | 纯投影（零判定） |
| 收口自检执行体（**新增**） | `baseline` / `touched` / `run`：T1 ∪ T2 触发、`spawn_blocking` 全量判定、notice / 引擎日志、fail-open | `crates/infra/agent/src/sdk/preclose.rs`（新） | `archi::drift::{assets_hash, check}`、`foundation::{cancel::CancelFlag, layout}`、`protect::written_paths` | `tokio::task::spawn_blocking`（`rt` 已在） |
| 收敛挂点（**改**） | 自然收敛支 `preclose::run` 先于 `finish_success`；循环内维护 `archi_touched`（沙箱 + 保护闸之后、执行之前） | `crates/infra/agent/src/sdk/loop.rs` | 同上 + `preclose` | 既有 async 泵 |
| 装配挂载（**改**） | `mint_archi_context` 五名判据；`ProtectRules::load(...).with_session(injections.protect)` | `crates/infra/agent/src/sdk/runner.rs` | `mount`、`protect` | 每 run 一次装配 |
| 保护规则装配（**改**） | `with_session`（纯数据合并）+ `with_session_rules`（插入位）+ `written_paths`（写面访问器） | `crates/infra/agent/src/sdk/protect/{mod,rules}.rs` | `agent::SessionProtectRule`、`checks::globmatch`、`shell::extract_file_ops` | 唯一 IO 点仍是 `load` |
| 拒绝文案（**改**） | `SESSION_LOCK_TEMPLATE` + `session_lock_template()` + `ReasonKind::Session` 分支 | `crates/infra/agent/src/sdk/protect/reason.rs` | — | 纯字符串 |
| 组合根装配（**改**） | archi 臂五 archi 名；`archi_matrix` 8 行；`model_write_lock(provenance)` 单点；三臂注入面携 `protect` | `crates/infra/agent/src/compose.rs` | `mount`、`protect`、`foundation::layout` | 装配单点（零 IO） |
| 会话契约（**改**） | `SessionProtectRule` + `SessionInjections.protect`（第五位，serde additive、无 `specta::Type`） | `crates/core/agent/src/runner.rs` | `serde` | 纯数据（core 不解释路径语义） |
| 知识拼装（**改**） | 派生段收缩为四面词表 + 两处目录（删契约索引）；骨架 doc 纪律改述（示例自证归 `archi_scaffold`） | `crates/infra/agent/src/archi_knowledge.rs` | `archi::dto`、`foundation::layout` | `include_str!` 静态骨架 |
| 知识骨架（**改**） | 按 B/C 分层重写；A/D 档全部改为工具指路 | `crates/infra/agent/src/archi-knowledge.md` | — | Markdown（编译期内嵌） |
| 相位 prompt（**改**） | 两处写权相位的工具行与写面句；其余 12 份「架构模型已锁」一句 | `crates/core/workflow/src/prompts/*.md`（14 份中 14 份动，7 份命中矩阵 + 12 份锁定侧，`implementation-generator` 两侧各一处，互不重叠） | — | 静态 md（`include_str!` 装配面零改动） |

依赖方向零新增边：`core/archi` 只多一处可见性放宽（crate 内）；`core/agent` 只加 serde 结构（零新依赖）；`infra/agent` 已依赖 `archi` / `foundation` / `checks` / `agent`（本变更只用既有依赖的名字）；前端零 npm 依赖变化。

---

## 变更清单 (Change Inventory)

### 新增文件

| 文件 | 说明 |
|---|---|
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_drift_view.rs` | 引擎侧漂移投影单点：`DRIFT_SCOPES`（三值词表单源）、`drift_summary`、`drift_entries`、`drift_files`。零判定、零 IO；被 `archi_tool.rs` 与 `preclose.rs` 共用（子表「公共函数 / API」逐条列出） |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/preclose.rs` | 收口前自检执行体：`baseline` / `touched` / `run`（触发判定、`spawn_blocking` 全量判定、notice + 引擎日志、fail-open）。唯一调用点 = `loop.rs` 自然收敛支（子表逐条列出） |

### 修改文件

| 文件 | 具体修改 | 说明 |
|---|---|---|
| `crates/core/archi/src/dto.rs` | `ArchiElementBrief` 增 `has_path: bool`（`to_brief` 填 `!element.path.is_empty()`）；新增 `EntryVia`（`Explicit` / `Convention`，serde `explicit` / `convention`）与 `ArchiResolvedEntry { path, via, problem }`；`ArchiModuleDetail` 增 `resolved_entry: ArchiResolvedEntry`；`module_detail` 用 `api::paths::entry_state(root, &projected, current)` 投影三态（`Resolved` → `path` + `via`（`element.entry.is_some()`）/ `Invalid` → `api::entry_problem_reason(&problem)` / `Absent` → 固定文案）；DTO 其余字段与派生零改动 | D2 / D3 / AC-1 |
| `crates/core/archi/src/api/mod.rs` | 仅可见性：`fn entry_problem_reason` → `pub(crate) fn entry_problem_reason`（文案与四值 match 逐字不动），供 `dto` 复用 | 事实校准 2 / AC-1（零第二份入口措辞） |
| `crates/core/archi/src/drift/mod.rs` | 新增 `pub fn assets_hash(root: &Path) -> String` + 私有 `collect_assets(root) -> Vec<PathBuf>`（三处资产根递归、相对路径排序）+ 私有 `hash_files(files: &[PathBuf], base: &Path) -> String`（`model_hash` 改为复用它，口径逐字等价）；`check` / `derive_status` / `Report` 零改动 | D4 / AC-3（模型面无改动） |
| `crates/infra/agent/src/sdk/mount.rs` | 常量 `ARCHI_DRIFT_TOOL` / `ARCHI_SCAFFOLD_TOOL`；`archi_drift_definition()`（`scope` enum 取 `archi_drift_view::DRIFT_SCOPES` + 「`gate` 全 `off` 时 `unmatchedFiles` 是常态」+「`files` 只在排查归属问题时用」两句）；`archi_scaffold_definition()`（`kind` enum 取 `ElementKind::ALL`，描述点明「省略取 `domain`」与「零写盘」）；`archi_module_definition()` 描述改述（补「缺 `path` 由 `hasPath` 一次判出」「入口解析结果随响应给出（显式 / 约定默认 / 不可用成因）」「新元素字段形态用 `archi_scaffold` 取模板」三句，删与工具面重复的语法教学句）；`build_registry` 13 → 15（`Vec::with_capacity(15)`，两新项均 `ToolTier::Read` / `ToolExec::Archi`，注释与 `ToolTier::Read` 归组清单同步）；`#[cfg(test)] fn injected_finish` 夹具补 `protect: None` | D5 / D7 / D15；`archi_module` 描述改述是「散文删语法、工具补读数」的另一半 |
| `crates/infra/agent/src/sdk/archi_tool.rs` | `execute` 分派加两支：`ARCHI_DRIFT_TOOL` → `drift_payload`、`ARCHI_SCAFFOLD_TOOL` → `scaffold_payload`；`drift_payload`（`scope` 解析 → `Instant::now` → `archi::drift::check(CheckInput { root, mode: Full, changed: &[], change: None, cancel: CancelFlag::never() })` → `drift_summary/entries/files` 逐级投影，错误面经 `CheckError::message()` + `is_error` 回灌）；`scaffold_payload`（`kind` 解析 ← `ElementKind::ALL`，值集外 `Err` 点名四值 → 模板文本）；两支均过 `tools::cap_result` | D5 / D7 / AC-2 / AC-3 |
| `crates/infra/agent/src/sdk/archi_drift_view.rs`（新） | 见「新增文件」；子表列签名 | D6 |
| `crates/infra/agent/src/sdk/preclose.rs`（新） | 见「新增文件」；子表列签名 | D8 |
| `crates/infra/agent/src/sdk/mod.rs` | 两处模块注册（按既有字母序）：`pub(crate) mod archi_drift_view;`（插在 `archi_tool` 之前）与 `pub(crate) mod preclose;`（插在 `preamble` 之前）；产品模块之外零改动（`#[cfg(test)]` 段的 `archi_drift_view_test` / `preclose_test` 挂载归测试相位） | D6 / D8 / AC-3 / AC-4；两个新文件的**唯一挂载点**——缺注册即「盘上有文件、编译无模块」；与各自文件同批落地（`archi_drift_view` 随阶段二、`preclose` 随阶段四） |
| `crates/infra/agent/src/sdk/loop.rs` | `run` 起手：`let archi_baseline = preclose::baseline(turn).await;`；`'continuation` 循环之外声明 `let mut archi_touched = false;`（跨轮累积），在「沙箱通过 ∧ 保护闸放行」之后、`mount.exec` 分派之前 `archi_touched |= preclose::touched(&name, &sandboxed, &turn.cwd);`；自然收敛支 `preclose::run(&sender, turn, archi_touched, archi_baseline.as_deref()).await;` 先于 `finish_success`；其余四个出口零调用；守卫链四段顺序与拒绝形态逐字不动 | D8 / D9 / AC-4 |
| `crates/infra/agent/src/sdk/runner.rs` | `mint_archi_context` 的 any 判据加 `ARCHI_DRIFT_TOOL` / `ARCHI_SCAFFOLD_TOOL`（五名逐一在场）；`let protect = ProtectRules::load(payload.workspace_root.as_deref()).with_session(open.injections.protect.clone());` | D10 / D15 / AC-6 / AC-7 |
| `crates/infra/agent/src/sdk/protect/rules.rs` | `ReasonKind::Session(String)`；`pub(super) fn with_session_rules(rules: Vec<Rule>, session: Option<&[agent::SessionProtectRule]>) -> Vec<Rule>`（插入位 = `builtin_rules().len()`，规则 glob 经即有 `normalize_user_glob` 归一、reason 原样携 `%s` / `%t`）；`load` 签名与内置 / 用户两段逐字不动 | D11 / AC-6 |
| `crates/infra/agent/src/sdk/protect/mod.rs` | `pub(crate) fn with_session(self, session: Option<Vec<SessionProtectRule>>) -> ProtectRules`（纯数据，零 IO）；`pub(crate) fn written_paths(command: &str, cwd: &Path) -> Vec<String>`（归一 → `shell::extract_file_ops` 的 `Write` + `Path` → `rules::resolve`）；`load` / `evaluate` / `match_path` / `dir_rule_hit` 逐字不动 | D10 / D14 / AC-5 / AC-6 |
| `crates/infra/agent/src/sdk/protect/reason.rs` | `SESSION_LOCK_TEMPLATE` + `pub(crate) fn session_lock_template() -> &'static str`；`deny_reason` 增 `ReasonKind::Session(template)` 分支（`substitute` 全量替换）；三类既有串逐字不变 | D12 / AC-5 |
| `crates/infra/agent/src/compose.rs` | archi 臂名字清单加 `mount::ARCHI_DRIFT_TOOL` / `mount::ARCHI_SCAFFOLD_TOOL`；`archi_matrix` 改 8 行表（D15）；新增 `model_write_lock(provenance) -> Option<Vec<agent::SessionProtectRule>>` 单点；`injected(...)` 增 `protect` 形参、三臂（explore / archi / change）与「其余来源」显式构造逐臂置位 | D13 / D15 / AC-5 / AC-7 |
| `crates/core/agent/src/runner.rs` | 新增 `SessionProtectRule { glob: String, reason: String }`（`Debug + Clone + PartialEq + Eq + Serialize + Deserialize`，**无 `specta::Type`**）；`SessionInjections` 加第五位 `#[serde(default)] pub protect: Option<Vec<SessionProtectRule>>`；类型 doc 词面「四位」→「五位」并补 `protect` 语义一句（None = 不追加，路径语义由装配层解释） | D10 / AC-6 |
| `crates/infra/agent/src/archi-knowledge.md` | 按 D16 / D17 重写：删「示例」围栏整段、字段面语法清单、`entry` 三档长表、`doc` 判据细节；`path` 段压成一段 + `hasPath` 指路；新增 `kind` 四值语义（C）、计数复述单源（B 一行）、架构域看护配置（B 一行）三块；「自检纪律」降为「可提前自查」并改为工具指路（`archi_drift` / `archi_module` / `archi_api` + 生成物重生成命令）；保留「写面与可见性」三条（含画布不自动刷新）与「历史会话续接」 | D16 / D17 / AC-8 |
| `crates/infra/agent/src/archi_knowledge.rs` | 派生段删「契约索引：`<archi>/graph/INDEX.toml`」一处（保留模型目录 + 契约资产两处方位与四面词表）；模块 doc 的两条纪律改述（示例自证改由 `archi_scaffold` 承担、体量门限随实测更新）；`build` / `knowledge` 的降级语义与 `<>` 拒绝逐字不动 | D16 / AC-8 |
| `crates/core/workflow/src/prompts/proposal-planner.md` | 只读工具行补 `archi_module`（定位：`modelFile` + `modelLine` 可直接 `edit`）与 `archi_scaffold`（取新元素模板）；新增写面句：可经既有 `write` / `edit` 维护架构模型（无专用写工具）、只为已确定元素落笔（设计未定的留到 `dev-design`）、写完用 `archi_drift` / `archi_module` 自查 | D18 / AC-9 |
| `crates/core/workflow/src/prompts/dev-design-planner.md` | 工具行补 `archi_drift`（读覆盖 / 归属 / 契约读数）与 `archi_scaffold`（取模板）；写面自查句与既有 `archi_module` / `archi_search` 用法行并列（零字段形态长表、零路径模板） | D18 / AC-9 |
| `crates/core/workflow/src/prompts/implementation-generator.md` | 工具行补 `archi_drift`（改完代码自查未声明依赖）；另补「架构模型已锁」一句（`implement` 在锁定侧） | D18 / AC-9 |
| `crates/core/workflow/src/prompts/*.md`（锁定侧其余 11 份：`test-design-planner` / `test-gen-generator` / `code-analyze-planner` + 8 份 evaluator） | 各补一句「架构模型已锁」指路（中文措辞、零 ASCII `archi` 字样、零路由 / 落账工具指示；evaluator md 仍零 `backtrack`）；既有工具行逐字不动 | D18 / AC-9 |
| `packages/desktop/src/types/generated/bindings.ts` / `bindings.zod.ts` | `pnpm -C packages/desktop run bindings:export` 重生成（非手改）：`ArchiModuleDetail.resolvedEntry` + `ArchiResolvedEntry` + `EntryVia` | D19 / AC-10 |
| `packages/desktop/package.json` | `"version": "0.7.2"` → `"0.7.3"`（单行） | D20 / AC-12 |

### 公共函数 / API

> 口径：只列**跨模块可见**的出口（`pub` / `pub(crate)`）。本变更不新增任何 IPC 命令、不新增 CLI 子命令、不新增 HTTP 端点；`core/archi` 的 crate 外公开面**只增** `assets_hash` 一处。

| 标识符 | 所在文件 | 变化 | 完整签名 | 说明 |
|---|---|---|---|---|
| `archi::drift::assets_hash` | `crates/core/archi/src/drift/mod.rs` | 新增 | `pub fn assets_hash(root: &Path) -> String` | 四类架构资产聚合哈希（`sha256:<hex>`）；缺失目录不参与、读失败按空字节；零写盘、零 spawn |
| `archi::api::entry_problem_reason` | `crates/core/archi/src/api/mod.rs` | 修改（可见性） | `pub(crate) fn entry_problem_reason(problem: &paths::EntryProblem) -> String` | 四个 `EntryProblem` 变体的一句话成因；`dto` 复用同一措辞（零第二份） |
| `archi::dto::module_detail` | `crates/core/archi/src/dto.rs` | 修改（返回形态） | `pub fn module_detail(root: &Path, fqn: &str) -> Result<ArchiModuleDetail, String>` | 返回体新增 `resolved_entry`（判据经 `api::paths::entry_state`） |
| `archi::dto::module_overview` | `crates/core/archi/src/dto.rs` | 修改（返回形态） | `pub fn module_overview(root: &Path) -> Result<Vec<ArchiElementBrief>, String>` | 每条薄概览新增 `hasPath` |
| `sdk::archi_drift_view::drift_summary` | `crates/infra/agent/src/sdk/archi_drift_view.rs` | 新增 | `pub(crate) fn drift_summary(report: &archi::drift::Report, duration_ms: u64) -> serde_json::Value` | 缺省 scope 的投影（含 `status` / `passed` / `modelHash` / `model` / `counts` / `errors` / `warnings` / `modelWarnings` 明细 / 判据 `byKind` / `durationMs`）；工具响应与收口 notice 共用 |
| `sdk::archi_drift_view::drift_entries` | 同上 | 新增 | `pub(crate) fn drift_entries(report: &archi::drift::Report, duration_ms: u64) -> serde_json::Value` | `summary` 之上追加 `observations` 与判据条目全量 |
| `sdk::archi_drift_view::drift_files` | 同上 | 新增 | `pub(crate) fn drift_files(report: &archi::drift::Report, duration_ms: u64) -> serde_json::Value` | `entries` 之上追加 `files`（`unmatched` / `matched` / `excluded` / `unresolved`） |
| `sdk::archi_drift_view::DRIFT_SCOPES` | 同上 | 新增 | `pub(crate) const DRIFT_SCOPES: [&str; 3]` | `["summary", "entries", "files"]`——`mount` 的 JSON `enum` 与执行体 `Err` 文案的唯一词表源 |
| `sdk::preclose::baseline` | `crates/infra/agent/src/sdk/preclose.rs` | 新增 | `pub(crate) async fn baseline(turn: &LoopTurn) -> Option<String>` | `turn.injections.preamble` 缺席即 `None`（零 IO）；否则 `spawn_blocking(assets_hash(turn.cwd))` |
| `sdk::preclose::touched` | 同上 | 新增 | `pub(crate) fn touched(name: &str, input: &serde_json::Value, cwd: &Path) -> bool` | `write` / `edit` 取 `path`（沙箱后的绝对路径）、`bash` 取 `protect::written_paths`；命中 `<archi>/graph/**` ∨ `<archi>/models/**`（前缀经 `foundation::layout` 派生） |
| `sdk::preclose::run` | 同上 | 新增 | `pub(crate) async fn run(sender: &tokio::sync::mpsc::Sender<AgentEventKind>, turn: &LoopTurn, touched: bool, baseline: Option<&str>)` | 触发 = T1 ∪ T2；命中即 `spawn_blocking(drift::check(..))` → `SystemNotice{"archi_preclose"}`；`Err` / `JoinError` → `SystemNotice{"archi_preclose_skipped"}`；两条件皆假只写引擎日志 |
| `sdk::protect::ProtectRules::with_session` | `crates/infra/agent/src/sdk/protect/mod.rs` | 新增 | `pub(crate) fn with_session(self, session: Option<Vec<agent::SessionProtectRule>>) -> ProtectRules` | 纯数据合并：会话规则插在内置之后、用户之前（`load` 签名不变） |
| `sdk::protect::written_paths` | 同上 | 新增 | `pub(crate) fn written_paths(command: &str, cwd: &Path) -> Vec<String>` | 复用 `shell::extract_file_ops` 的 `Write` + `Path` 态，经 `rules::resolve` 归一；零第二份写面解析 |
| `sdk::protect::session_lock_template` | `crates/infra/agent/src/sdk/protect/reason.rs`（经 `sdk::protect` 再导出） | 新增 | `pub(crate) fn session_lock_template() -> &'static str` | 相位锁文案模板（四要素、携 `%s` / `%t`）；`compose` 取它构造规则串 |
| `ArchiContext::execute` | `crates/infra/agent/src/sdk/archi_tool.rs` | 修改（分派面） | `pub(crate) async fn execute(&self, name: &str, input: &serde_json::Value) -> Result<String, String>` | 新增 `archi_drift` / `archi_scaffold` 两支；签名不变（分派内部加 match 臂） |

> 私有面（`archi_matrix` / `model_write_lock` / `assemble_injections` / `drift_payload` / `scaffold_payload` / `deny_for` / `collect_assets` / `hash_files`）按本表口径不列——其契约由五份 delta spec 与决策表承载。

### 类型定义

| 类型名 | 所在文件 | 变化 | 说明 |
|---|---|---|---|
| `ArchiElementBrief` | `crates/core/archi/src/dto.rs` | 修改 | 增 `has_path: bool`（线格式 `hasPath`，必填不省略）；其余五字段不变 |
| `ArchiResolvedEntry` | 同上 | 新增 | `{ path: Option<String>, via: Option<EntryVia>, problem: Option<String> }`，`serde(rename_all = "camelCase")` + `specta::Type`（→ `bindings.ts` 的 `path` / `via` / `problem`，三字段皆不省略） |
| `EntryVia` | 同上 | 新增 | `enum EntryVia { Explicit, Convention }`（serde `explicit` / `convention`）+ `specta::Type`；MUST NOT 增设第三值（约定默认与显式命中之外无第三来源） |
| `ArchiModuleDetail` | 同上 | 修改 | 增 `resolved_entry: ArchiResolvedEntry`（必填） |
| `SessionProtectRule` | `crates/core/agent/src/runner.rs` | 新增 | `{ glob: String, reason: String }`；`Debug + Clone + PartialEq + Eq + Serialize + Deserialize`，**无 `specta::Type`**（不进 TS bindings）；`reason` 可携 `%s` / `%t` |
| `SessionInjections` | 同上 | 修改 | 增第五位 `#[serde(default)] protect: Option<Vec<SessionProtectRule>>`（`None` = 不追加）；四字段语义逐字不变 |
| `Rule` / `ReasonKind` | `crates/infra/agent/src/sdk/protect/rules.rs` | 修改 | `ReasonKind` 增 `Session(String)` 变体（持模板串）；`Rule` 字段面不变 |
| `ToolTier` / `ToolExec` / `ToolMount` | `crates/infra/agent/src/sdk/mount.rs` | 零改动 | 三档 / 六执行体 / 三元组形态逐字不变；两新项复用 `Read` / `Archi` |
| `LoopTurn` | `crates/infra/agent/src/sdk/loop.rs` | 零改动 | 不新增字段（`archi_touched` 与 `baseline` 是 `run` 的局部量）——`loop_test.rs` 的 `LoopTurn` 构造面因此零改动 |

### 配置

| 配置键 | 所在文件 | 变化 | 值类型 | 默认值 / 现值 | 说明 |
|---|---|---|---|---|---|
| `version` | `packages/desktop/package.json` | 修改 | `string` | `0.7.3`（现值 `0.7.2`，+1 patch） | `src-tauri/tauri.conf.json` 经 `"version": "../package.json"` 跟随（零手改）；`src-tauri/Cargo.toml` 的 `version = "0.1.0"` 不动 |

> 配置项零新增：`.dev-team/config.json`、`crates/core/config/**`（含 `config.schema.json` 镜像与 `KNOWN_KEYS`）、`tauri.conf.json`、`capabilities/default.json` 全部零 diff。相位锁**不是**配置（无配置键、无开关、无环境变量），其置位只由 provenance 决定。

### 机械闭合面（编译 / 类型随动，零用例语义改动）

> 以下**不是**用例编写：第五位字段与 DTO 新字段是编译期与 `tsc` 的硬约束，必须逐处补齐（R9）。断言级改写（新锚点、新矩阵维度、新用例）归测试相位。
>
> 顺序约束：`SessionInjections` 第五位的穷尽字面构造（下表 Rust 侧全部行）与契约字段的引入**同批**落地——契约批次结束时 `cargo check -p agent -p agent-runtime --all-targets` 即须转绿，MUST NOT 推迟到生成物 / 版本批次（否则该批次的自查条件恒红）；前端夹具随动只需在 `bindings:export` 之后的 `client:check` 前完成。

| 文件 | 机械修改 |
|---|---|
| `crates/infra/agent/src/sdk/mount.rs`（`#[cfg(test)]` 夹具） | `injected_finish` 补 `protect: None` |
| `crates/infra/agent/src/sdk/runner_test.rs`（16 处） / `sdk/loop_test.rs`（16 处） / `sdk/tools_test.rs`（7 处） / `sdk/mount_test.rs`（6 处） / `crates/infra/agent/src/compose_test.rs`（3 处） / `crates/core/agent/src/runner_test.rs`（14 处） / `crates/core/agent/src/lib_test.rs`（2 处） / `crates/core/agent/src/kernel_test.rs`（1 处） | `SessionInjections` 穷尽字面构造补 `protect: None`（或改 `..Default::default()`，以最小 diff 为准）；零断言语义改动 |
| `packages/desktop/src/views/architecture/hooks/use-archi-module.test.ts` | `detailOf` 夹具补 `resolvedEntry`（三字段齐备，如 `{ path: null, via: null, problem: null }`） |
| `packages/desktop/src/views/architecture/components/architecture-canvas.test.tsx` / `architecture-view.test.tsx` / `components/model-info.test.tsx` | 按 `tsc` 报错逐处补 `resolvedEntry`（匿名对象字面量按 `ArchiModuleDetail` 语境收敛） |

<!-- 路由 / API 设计：不涉及——本变更零新 IPC 命令、零 HTTP 端点、零 CLI 子命令。新增的两个 archi 工具是模型侧工具面（定义在 infra 挂载清单、经注入名字清单下发），不含网络面；`SessionProtectRule` 无 `specta::Type` 故 `bindings.ts` 不出现该类型。 -->

---

## 数据模型 (Data Model)

### 出线 DTO（`core/archi`，IPC + 工具响应共用）

| 类型 | 字段 | 取值 | 持久化 |
|---|---|---|---|
| `ArchiElementBrief` | `hasPath` | `bool`（`path` 数组非空）；字段不省略 | 否（纯投影） |
| `ArchiResolvedEntry` | `path` | `string \| null`——repo 相对 POSIX 入口路径 | 否 |
| | `via` | `"explicit" \| "convention" \| null`——模型显式 `entry` / 约定默认命中 | 否 |
| | `problem` | `string \| null`——入口不可用或不存在的人读成因（`Malformed` / `OutsidePath` / `NotOnDisk` / `Duplicated` 四值之一，或「未声明且候选皆不在盘」） | 否 |

三态映射（判据单点 `api::paths::entry_state`）：

| `EntryState` | `path` | `via` | `problem` |
|---|---|---|---|
| `Resolved { form, entry }` | `entry` | `element.entry.is_some()` → `explicit`，否则 `convention` | `null` |
| `Invalid(problem)` | `null` | `null` | `entry_problem_reason(&problem)` |
| `Absent` | `null` | `null` | 「未声明 `entry`，且约定默认候选皆不在盘」 |

`EntryForm`（crate / subdir / single-file）**不出线**：`resolvedEntry` 只回答「入口在哪、凭什么判定」，形态位无消费方。

### 会话契约（`core/agent`，serde-only）

| 类型 | 字段 | 语义 | 持久化 |
|---|---|---|---|
| `SessionProtectRule` | `glob: String` | 装配层已归一（相对形态带 `**/` 前缀、绝对形态原样） | 否（随 `TurnRequest.injections` 进程内流转；**不进** `SessionConfigSnapshot`、不进 TS bindings） |
| | `reason: String` | 携 `%s` / `%t` 占位符的模板串（相位锁取 `session_lock_template()`） | 否 |
| `SessionInjections.protect` | `Option<Vec<SessionProtectRule>>` | `None` = 不追加会话规则；`Some([])` 与 `None` 同效（装配层永不产出空数组） | 否（`#[serde(default)]`：旧线 JSON 缺字段读 `None`） |

规则三来源与位次（`ProtectRules.rules` 的装配序即优先级）：

```
内置（`**/<workspace config path>`，恒 1 条） → 会话（相位锁，0 或 1 条） → 用户（`write_protection.files[]`）
```

### 收口自检载荷（`SystemNotice` 开放词典，非持久化结构）

`archi_preclose`（触发命中）：

```jsonc
{ "subtype": "archi_preclose",
  "payload": {
    "mode": "full",
    "status": "clean | warnings | errors | noChanges | skipped",
    "passed": true,
    "modelHash": "sha256:…",
    "model": { "elements": 54, "relations": 122,
               "byGate": { "strict": 0, "warn": 0, "off": 54 },
               "structuralErrors": [], "structuralErrorCount": 0 },
    "counts": { "errors": 0, "warnings": 43, "suppressed": 0, "observations": 12,
                "modelWarnings": 0,
                "apiFindings": { "total": 43, "bySeverity": { "error": 0, "warn": 43, "off": 0 } },
                "scan": { "changedFiles": 0, "scannedFiles": 358, "excluded": 3,
                          "unmatched": 302, "matched": 53, "unresolved": 0 } },
    "errors": [], "warnings": [ /* 全量明细：kind / from / to / gate / message / occurrences */ ],
    "modelWarnings": [],
    "apiFindings": { "byKind": { "docMissing": 43 } },
    "durationMs": 74,
    "changedAssets": true
  } }
```

`archi_preclose_skipped`（`CheckError` / 任务失败）：`{ "subtype": "archi_preclose_skipped", "payload": { "reason": "<成因>" } }`。

两 notice 都**先于** `TurnDone` 流出；`TurnDone` 的 `subtype` / `is_error` / `num_turns` / `usage` 逐字不变。整份 `Report` 不入转录——`counts` 里 `unmatched` 等常态量只出现在**计数位**（明细只在 `scope = "files"` 与 `scope = "entries"` 出现）。

### 哈希（非持久化）

| 面 | 覆盖面 | 消费者 | 是否入报告 |
|---|---|---|---|
| `model_hash(root)`（既有） | `graph/model/*.toml` | 报告字段 `modelHash` | 是（口径逐字不变） |
| `assets_hash(root)`（新增） | 上述 + `models/**/*.md` + `models/**/*.schema.json` + `graph/INDEX.toml` | 收口自检的 T2 判据 | 否（MUST NOT 取代 `modelHash`、MUST NOT 作为缓存失效依据） |

两者共用私有 `hash_files`（相对路径排序后逐文件累加「相对路径 + 字节」+ 空字节分隔，输出 `sha256:<hex>`）。

### 持久化与路径

| 面 | 落点 | 本次变化 |
|---|---|---|
| 模型声明 / 契约资产 / 索引 / 看护配置 | `.dev-team/architecture/**` | **零变化**（模型内容与本变更无关；`api:check` 预期零 diff） |
| 漂移报告 | 无（引擎两侧零写盘） | 零变化（落盘者恒为门禁 runner） |
| DB 表 / 字段 | — | 零变化（无新表、无新列；`SystemNotice` 载荷是 JSON 值） |
| 工具输出 | 无（`archi_drift` / `archi_scaffold` 零写盘） | 零变化 |

---

## 依赖 (Dependencies)

### 运行时依赖

零新增。`core/archi` 只用既有 `sha2` / `std::fs` / `foundation::layout`；`infra/agent` 只用既有 `archi` / `foundation` / `checks` / `agent` / `serde_json` / `tokio`（`spawn_blocking` 属 `rt` feature，`agent-runtime` 的 `tokio` features 已含 `rt`）；`core/agent` 只用既有 `serde`；`core/workflow` 零依赖变化（只改 md 文本）。前端零依赖变化。

### 构建 / 测试依赖

零新增：无 dev-dependency 变化（`archi` / `agent-runtime` 的既有 `tempfile` 够用）；无新工具链、无 `package.json` scripts 变化。

---

## 验收标准回应

| AC | 落点 |
|---|---|
| AC-1 | `dto.rs` 的 `has_path`（`to_brief` 单点）+ `ArchiResolvedEntry` / `resolved_entry`（`entry_state` 单点、`via` 由 `element.entry.is_some()` 派生、`Invalid`/`Absent` 落 `problem`）；源码复核面：`entry_state` 之外零入口解析（`dto.rs` 内只出现一次调用） |
| AC-2 | `mount.rs` 的 `archi_scaffold_definition`（`kind` enum ← `ElementKind::ALL`）+ `archi_tool.rs` 的 `scaffold_payload`（值集外 `Err` 点名四值、零写盘、草稿单段 FQN + 注释化字段 + 注释化关系，过 `TomlParser` 零诊断；事实校准 6/7） |
| AC-3 | `archi_tool.rs` 的 `drift_payload`（恒 `Full`、三 scope 逐级追加、`Instant` 计时、`cap_result`）+ `archi_drift_view.rs`（投影单点、`counts` 常量级摘要）+ `mount.rs` 描述两句（常态句 / `files` 用途）；零跨调用缓存（无缓存容器） |
| AC-4 | `preclose.rs`（`baseline` / `touched` / `run`：五出口只自然收敛支、T1 ∪ T2、`spawn_blocking` + `CancelFlag::never()`、fail-open 三面、notice 先于 `finish_success`）+ `loop.rs` 挂点；零额外请求、零历史追加（`run` 只发 notice、不动 `history`） |
| AC-5 | `compose.rs` 的 `model_write_lock`（逐臂置位）+ `protect/{mod,rules,reason}.rs`（`with_session` / 规则序 / 模板四要素）+ `core/agent` 的第五位；锁面只 `graph/model/**`（`models/**` / `INDEX.toml` / 看护 `config.json` 零命中） |
| AC-6 | `SessionInjections` 恰五位（`preamble` / `tools` / `finish` / `ask` / `protect`）；`#[serde(default)]` 使旧线 JSON 缺字段读 `None`；`ProtectRules::load` 签名不变（47 处既有调用零改动）；规则序由 `with_session_rules` 的插入位表达 |
| AC-7 | `compose.rs` 的 `archi_matrix` 8 行 + archi 臂五 archi 名 + `mount::registry` 15 项 + `mint_archi_context` 五名判据；矩阵外零 archi 工具（`_ => None`） |
| AC-8 | `archi-knowledge.md` 重写（D16 / D17）+ `archi_knowledge.rs` 派生段两处目录、四面词表在场、`None` 降级不变；体量门限 2252 / 2867 B（事实校准 10），实测值由测试相位冻结 |
| AC-9 | 14 份 prompt 的相位分流（D18）：两处写权相位含写面与自查句；其余 12 份含「架构模型已锁」一句（中文、零 ASCII `archi`、evaluator 零 `backtrack`）；`phase_table.rs` 的 `include_str!` 与角色表零改动 |
| AC-10 | `bindings:export` 重生成（D19）；生成物只含 `ArchiModuleDetail.resolvedEntry` / `ArchiResolvedEntry` / `EntryVia`（`hasPath` 不进 bindings——事实校准 1）；前端夹具随动后 `tsc` 绿 |
| AC-11 | 全管线（`server:check` → `bindings:export` → `api:check` → `client:check` + `run test`）由桌面端执行；本设计保证 `api:check` 零 diff 的前提是 `.dev-team/**` 零改动（D21 ②） |
| AC-12 | `package.json` = `0.7.3`；`tauri.conf.json` 跟随；`src-tauri/Cargo.toml` 版本行不随动（D20） |

---

## 待决问题 (Open Questions)

1. **骨架 / 全文体量的精确冻结值**：门限已定（2252 B / 2867 B），但「实测值冻结」要求实现后回填具体数字。若实测骨架落在 2150–2252 B 区间（而非目标 2000–2150），是否接受为达标而不追加删句？本设计裁定：**接受**（门限即达标判据），但 MUST NOT 为压体量删除 C 档四条散文（`metadata` 内联表 / 带引号 FQN 单键 / `kind` 语义 / 层级先声明——R1 的兜底面）。
2. **`archi_drift` 的 `files` scope 在超大仓库的截断体验**（R5 / R3）：`cap_result` 截断后常出现「路径清单只到第 N 条」。本期不加分页参数（值集封闭是三值）；若实测体验差，后续可在 `files` 之外增设「按元素收窄」参数，属独立裁决。
3. **`code-review` / `acceptance` 的 `archi_drift` 勾选**（R6）：本期取保守案（不勾）。决策理由是「时机差不足以推翻『这两相位只读模型面』的既有条款」，放行需独立裁决——本设计不预设裁决结果。
4. **R8 的批量动作缺口**：`git stash` / `git clean` 不按相位判别（`BulkGlobs` 与相位无关）如实留痕；`git checkout <ref> -- <path>` 等单路径形态已被会话规则覆盖。是否给 `BulkGlobs` 加相位条件，留待独立裁决（提案「明确不做」⑦）。
5. **`assets_hash` 的读失败口径**：spec 定「单文件读失败按空字节参与（宁可哈希变化）」。语义上「读失败」会让哈希值与真实盘面不符，但方向是**更保守**（必然触发 T1 之外的 T2，多跑一次判定）。本期按 spec 执行，不新增重试。
