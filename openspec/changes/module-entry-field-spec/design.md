# 设计: module-entry-field-spec

> **变更**: module-entry-field-spec（主线变更 A · 模块入口字段与接口文件落点）
> **日期**: 2026-11-15
> **基于**: `proposal.md`（模型 `entry` 字段 + 解析链三态 + 落点改入口同级 + 归属第三候选 + AC-1…AC-9 + R-1…R-9）、`explore.md`（A.1 / A.2 / A.3 / A.5 / A.7 事实面与 A.11 三处签字）、以及七份 spec delta（`desktop-archi-api` / `desktop-archi-api-check` / `desktop-archi-model` / `desktop-archi-agent` / `desktop-archi-drift` / `desktop-archi-file-tree` / `repo-archi-model`）
> **性质**: 「入口」从盘上探测升为模型字段，生成物落点由「元素根同级」改为「实现入口同级」，并让归属判据跟上新落点。零新增判据种类（口径仍十值）、零新增 IPC、零契约文本改动；改动集中在 `crates/core/archi` 的模型 / 路径 / 判据 / 归属四面，加一份生成物搬迁、**两处模型数据的 `api = none` 留白**（三处显式 `entry` 归宿主重建后的后续变更，见 proposal §六 / R-10）、一份知识文本、两份仓库根文档的一处措辞。

---

## 起点实况核实（工作树实测；Rust 前缀省略为 `packages/desktop/src-tauri/`）

| 面 | 实测 |
|---|---|
| `crates/core/archi/src/api/paths.rs`（364 行） | `entry_form` `:242–278`（七候选：Rust `<dir>/src/lib.rs` / `<dir>/mod.rs` / `<dir>.rs`，前端 `<dir>/index.ts` / `<dir>/index.tsx` / `<dir>.ts` / `<dir>.tsx`，逐个 `is_file()` 探测）；`mod_path_attr` `:280`（三形态 `../../` / `../` / 裸文件名）；`generated_path` `:229`（**入参是 `<root>`**，产 `<root>.i.<ext>`）；`ts_import_specifier` `:290`；`EntryForm` `:30`（`Crate` / `SubdirModule` / `SingleFile`）；`checked_root` `:324` + `is_repo_relative` `:338`（路径形态闸）；`generated_kind` `:196`（`Unknown` 归 Rust）；`module_name` `:96` / `rust_module_path` `:165`（透传命名面，D40 单点）；模块 doc 首行「零 IO（`entry_form` 的存在性探测是唯一例外）」 |
| `crates/core/archi/src/api/check.rs`（647 行） | ① 判定在 `:78–86`：`entry_form(...)` → `has_generated`（对 `Rust` / `TypeScript` 两个 kind 各调一次 `generated_path`，`.is_file()` 任一即真）→ `entry_missing = entry.is_none() && !has_generated`；命中即 push `ApiEntryMissing`（文案「既无生成物也无实现入口」）；`category_of` `:45`；`crate_root` `:577`（`path.first()` 取包根三段，与入口无关，零改动） |
| `crates/core/archi/src/api/mod.rs`（956 行） | `element_symbols` `:408`（`entry_form` → `extract_element`）；`render_all` `:423`（`:462–465` 用 `path.first()` 判语言与 `generated_path`，`:470` 用 `entry_form` 取 `form`）；`index_entries` `:489`（`:517` `generated_rel` 走私有 `generated_path_of`）；`bootstrap` `:598`（`:612` `entry_form`）；`api_ref` `:735` / `api_ref_of` `:750`（第三参是 `element_path: &[String]`，`:753` 走 `generated_path_of`）；`module_refs` `:792`（透传 `module_name` / `rust_module_path`，**仍取 `path.first()`**，本变更零改动）；`generated_path_of` `:836`（私有，入参 `element_path`） |
| `crates/core/archi/src/scan/owner.rs`（217 行） | `OwnerEntry` `:50`（`fqn` / `prefix` / `gate`）；`build` `:68`；`owner_of` `:92`（对每个剥离候选各跑 `longest_prefix_hits` 后合并定档，**非短路**）；`longest_prefix_hits` `:154`；`strip_candidates` `:185`（双候选：整组剥离 `<name>.i.<ext> → <name>`、替换扩展名 `<stem>.i.<ext> → <stem>.<ext>`，白名单外单候选）；`prefix_matches` `:212` |
| `crates/core/archi/src/model.rs` / `parse/toml.rs` / `dto.rs` | `Element` `model.rs:44`（`fqn` / `kind` / `gate` / `api` / `path` / `docs` / `summary` / `metadata` / `file` / `line`，**无 `entry`**）；`RawElement` `toml.rs:30`（`deny_unknown_fields`）；`to_model` `toml.rs:82`；`ArchiElement` `dto.rs:458–479`（末尾字段是 `api`）；`to_element` `dto.rs:644`（唯一投影点）；`module_detail` `dto.rs:763`（`:810` 调 `api_ref_of(root, fqn, &element.path)`） |
| 生成物与 `#[path]` | `crates/infra/archi.i.rs` 在盘（17 行）；`crates/infra/archi/src/lib.rs:18` 为 `#[path = "../../archi.i.rs"]`，`:10–17` 注释写「与 `mod_path_attr(EntryForm::Crate, "archi")` 输出等值」；全仓 `.i.rs` 仅此一份、`.i.ts` 零份 |
| `.dev-team/architecture/api/INDEX.toml` | 42 条 `[[entry]]`；`App.Backend.Infra.ArchiRuntime`（`:78–84`）`generated = "packages/desktop/src-tauri/crates/infra/archi.i.rs"`，其余 41 条 `generated = ""` 或空行 |
| 模型数据 | `models/04-backend-shell.toml:4–8` 壳元素（`path = ["./packages/desktop/src-tauri/src/"]`，无 `entry`）；`models/05-backend-tools.toml:7–18` 两个工具 crate（只有 `path`，无 `entry` / `api`）；`06-frontend.toml:8` `App.Frontend.Shell` 四文件 `path`（首条 `app.tsx`）；`06-frontend.toml` 12 个目录根（`views/*` 8 + 视图内子域 3 + `components/ui`） |
| 知识文本与根文档 | `crates/infra/agent/src/archi-knowledge.md:13` 「元素字段面：`kind`（必填）、`gate`、`api`、`path`、`docs`、`summary`、`metadata`」（**漏 `entry`**）；`AGENT.md:27` / `CLAUDE.md:27` 第②条写「元素根同级的 `<root>.i.rs` / `<root>.i.ts`」；两文件全文 `diff` 恰 2 处（`:1` 标题、`:3` 首行说明） |
| 前端线面 | `knip.json` 的 `ignore` 已含 `src/**/*.i.ts`；`ArchiElement` 字面量只出现在 `packages/desktop/src/**/*.test.ts(x)` 与两处生成文件；`views/architecture/components/element-drawer.tsx:45` 只读 `modelFile` / `modelLine` |
| 版本 | `packages/desktop/package.json:3` `"version": "0.6.43"`；`tauri.conf.json` 经 `"version": "../package.json"` 跟随 |

---

## 事实校准（不改 spec 意图，只定实现形态）

1. **`entry_candidates` 不做校验，`entry_state` 才是判据点**。spec 的解析链 ① 是「显式 `entry`（归一后）→ 唯一候选」，若在 `entry_candidates` 里就把形态非法的声明滤空，`Invalid(Malformed)` 与 `Absent` 就不可分辨。故 `entry_candidates` 只做 `normalize_root` + 按序拼串，非法声明**原样返回**；`Malformed` 由 `entry_state` 判。同理 `<root>` 形态非法（`checked_root` 返回 `None`）⇒ 候选表为空 ⇒ `Absent`（与既有 `entry_form` 对非法根返回 `None` 同口径，spec「路径形态非法即拒收」Scenario）。
2. **`EntryProblem` 各变体携带自身数据**（`entry` 原文与 `other_fqn`），`check.rs` 不再回读 `element.entry` 拼文案。理由：`Duplicated` 的冲突方只有 `entry_state` 知道，若只留无载荷三值枚举，`check` 就得为拿冲突方再跑一遍比对（第二份判据）。
3. **`EntryForm` 的形态位派生规则落成 `form_of(entry, element)`**：`entry == root`（`path.first()` 归一）→ `SingleFile`；`entry == "src/lib.rs" || entry.ends_with("/src/lib.rs")` → `Crate`；其余 → `SubdirModule`。`EntryForm` 三值保留、消费面只剩 `EmitTarget.form`（TS 侧待接线形态位），**零新增第四值**。
4. **落位判据的目录分支只对非源文件形态的 `path` 条目生效**：`entry == 某条 path` 或「`path` 条目非源文件形态（`!crate::scan::is_source_file`）且 `entry` 以其 `"{path}/"` 开头」。这样「文件型根声明同目录的另一个文件」自然落 `OutsidePath`（AC-2 第五例），无需第五条判据，也不会出现「`views/welcome-view.tsx/xx.rs` 被前缀匹配放行」。
5. **跨元素唯一判据的实现是「候选交集」**：`duplicate_of` = 其它元素的 `entry_candidates` 里含本元素解析出的同一入口文本。`entry_candidates` 纯文本零 IO，故不必为另一个元素跑完整解析（那要递归 + 读盘）。代价是一处保守近似：同一目录型根下若 `src/lib.rs` 与 `mod.rs` **同时**在盘，另一元素显式声明后者会被判 `Duplicated`（其实前者优先命中）。该近似只多报不瞒报，本仓零实例（AC-4 要求 `Invalid` 0 条）。
6. **`generated_path` 的主干取法**：`<入口目录>` + `<入口主干>.i.<ext>`，`<入口主干>` = 入口文件名最后一次 `.` 之前的部分；文件名无 `.` 时主干 = 整个文件名（`MUST NOT` 丢文件名）；入口无目录段时目录为空（`lib.rs` → `lib.i.rs`，`root.join` 后即仓根，本仓无此形态）。`checked_root` 仍是形态闸（绝对路径 / `..` / 空段 / 空串 → `None`）。
7. **`api_ref` 投影需要元素表**：`api_ref_of` 的第三参由 `element_path: &[String]` 改为 `elements + element`（见 D9）。`dto::module_detail` 手上是模型 AST，故把元素表投影一次（`to_element` 覆盖全部元素，`find` 出当前元素）后传入——**不**为省一次投影而放宽判据射程（投影面与判定面同一入口解析）。
8. **`module_refs`（Rust / TS 透传命名）零改动**：`module_name` / `rust_module_path` 仍取 `path.first()`（D40 的单点），MUST NOT 因入口化改成取入口主干——`archi-runtime` / `archi` / `lib` 三名互不相等（explore A.5）。本变更只钉「入口不参与透传命名」。
9. **`crate_root`（`check.rs:577`）仍取 `path.first()`**：它判「父子元素是否同属一个包」，与入口无关，零改动。
10. **`model::Element` 的 `entry` 插在 `api` 之后、`dto::ArchiElement` 的 `entry` 追加在末尾**：前者按 `desktop-archi-model` 的字段面次序（`fqn` / `kind` / `gate` / `api` / `entry` / `path` / …）；后者按「新增字段追加在末尾」的既有先例（`ArchiDriftReport.api_findings`），使 `bindings.ts` / `bindings.zod.ts` 的 diff 只多一段。
11. **既有机械锚点会波动，改写归测试相位**：`paths_test` 引用 `mod_path_attr` / `entry_form`（`:24–28` 的 `use`）、`check_test:1442–1454`（断言 `check.rs` 源码含 `element.path.first()` 表达容器）、`api/mod_test:2765`（断言 `render_all` 体含 `let scope = element.path.first()`）、`emit/mod_test` 与 `emit/ts_test` 的 `entry_form` 注释断言、`api_ref_of` 四处调用点。产品侧不留旧名，这些断言由测试相位按新形态重写。
12. **`archi_api` 的基线数字只作留痕**：AC-4 的 41 条分桶（15 + 26）由全量 `archi::drift::check` 在新代码上跑出后与 change 报告对拍，**不写进断言**（`desktop-archi-api-check`「基线表本身不写进断言」）；数字的测量口径须带版本注记——门禁跑在宿主进程内，宿主 archi crate 落后一版时量不出新口径（分两批的理由见 proposal §六）。
13. **知识骨架受 3KB 硬上限约束，「五段逐字不动」不可行**：`SKELETON.len() <= 3 * 1024`（`体量切分骨架不超3kb`，test-design「知识面边界」要求既有边界保持绿）时 `HEAD` 骨架已占 3061B，本变更三处增量（八字段面 + `entry` 三纪律、边界「两个指针」句、示例单值 `entry`）约 +316B ⇒ 全量还原将达 3440B。落法：delta spec 的**冻结面**（「自检纪律」「写面与可见性」）逐字还原 `HEAD`（`archi_module` 不带 `element` + 结构诊断 `file:line`、`archi_api`（`action = "check"`）+ 两轴在案），其余非冻结段保持最小语义压缩、各自关键词逐条在场；终值 **3058B ≤ 3072B**。

---

## 关键设计决策

| # | 议题 | 定稿 | 理由 |
|---|---|---|---|
| D1 | 入口解析链的落点 | `paths` 新增 `entry_candidates(element) -> Vec<String>`（纯文本、零 IO、按优先序：① 显式 `entry` 归一后 ② 文件型根 = 根文件自身 ③ 目录型根四条约定形态 `src/lib.rs` → `mod.rs` → `index.ts` → `index.tsx` ④ 其余空表）+ `entry_state(root, elements, element) -> EntryState`（首个落盘候选 + 四条判据，读盘只在此） | 候选派生与判据分离，AC-1「候选派生零 IO」可机械断言；③ 不做语言判定（目录型根 `language_of` 恒 `Unknown`，按语言分派会让变更 B 的 barrel 全部失效） |
| D2 | 旧七候选的裁剪 | 删 Rust `<dir>.rs`、前端 `<dir>.ts` / `<dir>.tsx` 三条；`<dir>/lib.rs` 不入列；新增文件型根 ② | 三条对目录型根只能指到 `path` 之外（必违落位判据），实测零命中（R-5）；文件型根由 ② 覆盖，`<dir>/lib.rs` 恒不成立（crate 的 `lib.rs` 必在 `src/` 下） |
| D3 | 三态与判据载荷 | `EntryState::{Resolved { form, entry }, Invalid(EntryProblem), Absent}`；`EntryProblem::{Malformed { entry }, OutsidePath { entry }, NotOnDisk { entry }, Duplicated { entry, other_fqn }}` | 事实校准 1/2：判据结论自带数据，`check` 只负责措辞（R-3 要求文案点名越界目标 / 缺失文件 / 冲突方 FQN） |
| D4 | 「无入口」的语义 | `Absent` = 未声明且无约定默认；`Invalid(_)` = 四条判据任一违反。两者同报一条 `apiEntryMissing`（`category = entry` / `warn`），**不进** `status`、**不改** `passed`；`api = none` 不豁免 | spec `desktop-archi-api-check` 的十值封闭与「无可用入口」口径；九类结构化诊断不扩（会让整图停摆） |
| D5 | `has_generated` 退场 | `check.rs` ① 改为 `match entry_state(...)`：`Resolved` → 继续 ②③④；其余 → 一条 `apiEntryMissing`。删 `has_generated` 与配套的双 `generated_path` 探测 | 生成物路径是**入口的函数**，无入口即无路径可派生（因果倒置的替代证据必须删）；AC-3「`.i` 在盘仍报 ①」由测试相位钉住 |
| D6 | `generated_path` 入参 | `pub fn generated_path(root: &Path, entry: &str, kind: GeneratedKind) -> Option<PathBuf>`，产 `<入口目录>/<入口主干>.i.<ext>`；`#[path]` 值退化为 `generated_path(...).file_name()` | 落点唯一函数；`#[path]` 属性值只能是字面量，函数已无产出余地 |
| D7 | 两条函数删除 | 删 `paths::mod_path_attr` 与 `paths::entry_form`；`ts_import_specifier` 与 `EntryForm` 保留为**待接线形态**（doc 标注「本期未接线，MUST NOT 当既有真值」） | `mod_path_attr` 的三种 `../` 层数在入口同级下恒退化为文件名，再留一层只是间接（与 D40/D41「零二次派生」同族，explore A.3 / A.11 签字）；`entry_form` 的职责由 `entry_state` 整体接管 |
| D8 | 语言分派输入 | `generated_kind(language_of(<入口>))`：函数本身零改动，**调用方改喂入口**；`Unknown` 归 Rust 的既有判据保留 | 入口恒带扩展名（目录型根的四个候选全是 `*.rs` / `*.ts(x)`），语言面因此不再靠「无扩展名即 Rust」兜底 |
| D9 | 投影面签名 | `pub fn api_ref_of(root: &Path, elements: &[ArchiElement], element: &ArchiElement) -> ArchiApiRef`；私有 `fn ref_projection(root: &Path, fqn: &str, entry: Option<&str>) -> ArchiApiRef` 承接「元素缺席 / 模型错」这一形态；`api_ref(root, fqn)` 取全表后委派 | 投影与判定走同一入口解析（事实校准 7）；「元素缺席仍报契约事实」的既有语义由 `ref_projection` 保住，MUST NOT 为凑参数构造占位元素 |
| D10 | 归属第三候选的表达 | `strip_candidates` 由 `Vec<String>` 改为 `Vec<StrippedCandidate>`，`StrippedCandidate::{Prefix(String), Stem(String)}`：`Prefix` 比对元素 `path` 归一前缀（候选 1/2），`Stem` 比对**文件型 `path` 的去扩展名形态**（候选 3）；`longest_prefix_hits` 收该枚举，长度取命中目标的字符数 | 第三候选的**文本**与候选 1 相同，差异只在比对目标——用「候选 + 比对目标」二元组表达才不产生第二份剥离函数；并集去重、最长档定档、`Ambiguous` 语义逐字沿用（`src/dir.i.ts` 仍由候选 2 的更长前缀胜出，`src/a.ts` + `src/a.tsx` 仍 `Ambiguous`） |
| D11 | 「文件型 `path`」的判据 | `OwnerIndex::build` 为每条前缀算 `stem: Option<String>`，判据 = `crate::scan::is_source_file(prefix)`（五扩展名白名单，与枚举面同源） | MUST NOT 另写扩展名清单（`scan` 的既有纪律）；纯文本判据，`build` 仍零读盘 |
| D12 | `INDEX.toml` 的 `generated` | `index_entries` 走 `entry_state` → `generated_path`；`Absent` / `Invalid` 留空（沿用既有 `filter(|_| present)`） | spec：「生成物路径是入口的函数，无入口即无路径可写」 |
| D13 | `bootstrap` 的失败文案 | 无可用入口时 `ApiError::Element` 文案改为「未声明 `entry`，且约定候选 …皆不在盘 / 声明值 <x> 被判 <理由>」，并列出候选 | 与 `apiEntryMissing` 同一套理由（同一解析链），避免两处各写一份「为什么没有入口」 |
| D14 | 纪律节措辞同步 | `AGENT.md` / `CLAUDE.md` 第②条：`元素根同级的 <root>.i.rs / <root>.i.ts` → `实现入口同级的 <入口主干>.i.rs / <入口主干>.i.ts`；**条目数恒 5**、其余四条要点与节内其余条款逐字不动、两文件成对同一提交 | 该节是 agent 每轮注入的系统提示词，落点一迁措辞即失真（proposal §六 #7 / AC-9）；字段知识只进 `archi-knowledge.md`，MUST NOT 加第六条 |
| D15 | 明确不做 | ① 前端 12 个目录根补 barrel（变更 B）；② TS 侧入口到入口的相对路径真算（变更 C，本变更只定规则 + 显式留白）；③ 契约格式补丁（变更 D）；④ 第 11 个 finding kind；⑤ 结构化诊断扩类；⑥ 「文件型根声明另一个文件」的放宽（记入 proposal R-4，需新增判据，MUST NOT 顺手支持）；⑦ `scan/mod.rs` / `tree.rs` / `drift/**` / `tools/archi-api-gen/**` / `foundation::layout` / `crates/core/config` 的任何改动 | 逐条对应 proposal「明确不做」与七份 delta 的留白条款；⑦ 中 `scan::is_source_file` 只是被 `owner.rs` 与 `paths.rs` **只读复用**（零签名变化） |

---

## 架构组件 (Architecture Components)

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|---|---|---|---|---|
| 入口解析与路径派生单点（**改**） | `entry_candidates`（纯文本）/ `entry_state`（唯一读盘点）/ `generated_path`（入口 → 落点）/ `form_of`（形态位派生）；删 `entry_form` / `mod_path_attr` | `crates/core/archi/src/api/paths.rs` | `crate::scan::is_source_file`（文件型根判据，只读复用）、`std::path`、`foundation::layout::archi_api_dir`（既有） | 纯函数 + `is_file()` 探测（探测只在本组件） |
| 契约判据面（**改**） | ① 改经 `entry_state`；`Absent` / `Invalid` ⇒ 一条带理由的 `apiEntryMissing`；删 `has_generated`；②③④ 与 ⑤–⑩ 逐字不动 | `crates/core/archi/src/api/check.rs` | `paths`、`contract`、`extract`、`schema`（既有） | 纯函数出 `Vec<ArchiApiFinding>` |
| 契约面装配与投影（**改**） | `element_symbols` / `render_all` / `index_entries` / `bootstrap` / `api_ref` / `api_ref_of` 取 `Resolved` 入口；`generated_path_of` 改收 `(elements, element)`；`ref_projection` 承接无元素形态 | `crates/core/archi/src/api/mod.rs` | 同上一层 | 纯读盘、零写盘、零 spawn（既有边界） |
| 归属索引（**改**） | 剥离候选三值化（整组剥离 ∪ 替换扩展名 ∪ 主干对齐）、`OwnerEntry.stem`、`longest_prefix_hits` 按比对目标长度定档；`normalize` / `prefix_matches` / `module_roots` / `contains_count` 逐字不动 | `crates/core/archi/src/scan/owner.rs` | `crate::scan::is_source_file`、`crate::dto::{ArchiElement, GateTier}` | 纯函数，构建一次复用 |
| 模型字段面（**改**） | `Element.entry` / `RawElement.entry` / `ArchiElement.entry` + 投影搬运 | `crates/core/archi/src/{model.rs,parse/toml.rs,dto.rs}` | `serde`、`toml`（既有） | 结构体 + `#[serde(default)]` |
| 生成物搬迁（**改**） | `crates/infra/archi/src/lib.i.rs` 新增（内容逐字）、旧 `crates/infra/archi.i.rs` 删除、`#[path]` 字面量与注释同步 | `crates/infra/archi/{src/lib.rs,src/lib.i.rs,archi.i.rs}` | 无（纯文件搬迁） | 静态文件 |
| 知识文本（**改**） | 元素字段面补 `entry`、`entry` 三条纪律、示例 TOML 加单值 `entry` | `crates/infra/agent/src/archi-knowledge.md` | 无（`include_str!` 静态骨架） | markdown |
| 仓库根文档（**改**） | `## 模块 API 契约` 第②条方位词与文件名形态 | `AGENT.md` / `CLAUDE.md` | 无 | markdown |
| 模型数据（**改**） | 两个工具 crate 各一行 `api = "none"`（HEAD 字段面内）；`04` 零改动，全库零 `entry`（三处显式值归宿主重建后的后续变更） | `.dev-team/architecture/models/05-backend-tools.toml` | 解析面 | TOML |
| 生成物与线面（**再生成**） | `INDEX.toml` 的 `ArchiRuntime.generated` 改值；`bindings.ts` / `bindings.zod.ts` 加 `entry` | `.dev-team/architecture/api/INDEX.toml`、`packages/desktop/src/types/generated/**` | 生成器 | 机械再生成（禁手改） |

### 数据流（改后）

```
models/*.toml ──TomlParser──> Element{entry, path} ──dto::graph_view──> ArchiElement{entry, path}
                                                         │
                        paths::entry_candidates(element) │ 纯文本（① → ② → ③ → ④）
                                                         ▼
                        paths::entry_state(root, elements, element)   ← 唯一读盘点（is_file）
                          ├─ Absent ─────────────┐
                          ├─ Invalid(problem) ───┤→ check ①：一条 apiEntryMissing（文案分列理由）
                          └─ Resolved{form, entry}
                                        │
        ┌───────────────────────────────┼────────────────────────────────┐
        ▼                               ▼                                ▼
generated_path(root, entry, kind)   extract::extract_element(entry)   EmitTarget.form（TS 形态位）
        │                               │
        ├─ render_all 产 .i.*           └─ check ②③④ / load_contracts.symbols
        ├─ index_entries 的 generated
        └─ api_ref / api_ref_of 的 generatedPath
                                        │
scan/owner.rs：.i.<ext> 归属 = 整组剥离 ∪ 替换扩展名 ∪ 主干对齐（∪ 后按命中长度定档）
```

---

## 变更清单 (Change Inventory)

### 新增文件

| 文件 | 说明 |
|---|---|
| `packages/desktop/src-tauri/crates/infra/archi/src/lib.i.rs` | `crates/infra/archi.i.rs` 的**逐字搬迁**（17 行，含 `LocalArchiCheck` 与签名见证 `const _`）；落点由「元素根同级」改为「入口同级」，内容零改动（类型定义见下表 `LocalArchiCheck` 一行） |

### 修改文件

| 文件 | 具体修改 | 说明 |
|---|---|---|
| `packages/desktop/src-tauri/crates/core/archi/src/model.rs` | `Element` 在 `api` 之后、`path` 之前加 `pub entry: Option<String>`（附语义 doc：单值、repo 相对、四条判据归 `api::paths`） | 事实校准 10；字段面次序按 `desktop-archi-model` |
| `packages/desktop/src-tauri/crates/core/archi/src/parse/toml.rs` | `RawElement` 加 `#[serde(default)] entry: Option<String>`；`to_model` 搬运 `entry: raw_element.entry` | `entry = ["a"]` / `entry = {…}` 由 serde `invalid type` 归 `invalidValue`（`classify` 既有分支），拼错的键仍 `unknownField`（`deny_unknown_fields`） |
| `packages/desktop/src-tauri/crates/core/archi/src/dto.rs` | `ArchiElement` 末尾（`api` 之后）加 `pub entry: Option<String>`；`to_element` 搬运 `entry: element.entry.clone()`；`module_detail` 把元素表投影一次（`to_element` 覆盖全部元素、`find` 出当前元素）后调 `api_ref_of(root, &projected, current)` | 显式字段不省略（缺省出 `null`）；投影只裁剪零重算（入口解析仍在 `api::paths`） |
| `packages/desktop/src-tauri/crates/core/archi/src/api/paths.rs` | ① 新增 `EntryState` 与 `EntryProblem` 两型；② 新增 `entry_candidates` / `entry_state` + 私有 `within_path` / `duplicate_of` / `form_of`；③ `generated_path` 入参由 `element_root` 改 `entry`（产 `<入口目录>/<入口主干>.i.<ext>`）；④ **删** `entry_form` 与 `mod_path_attr`；⑤ `ts_import_specifier` 的 doc 标注「待接线留白，当前输出 MUST NOT 当既有真值」；⑥ 模块 doc 的「零 IO（`entry_form` 是唯一例外）」改为「零 IO（`entry_state` 的存在性探测是唯一例外）」，并补「落点 = 入口同级」与 `module_name` 只服务透传命名的说明 | D1–D8；`checked_root` / `is_repo_relative` / `normalize_root` / `module_name` / `rust_module_path` / `generated_kind` / `language_of` / `ref_relative` / `domain_of` / 四类契约路径逐字不动 |
| `packages/desktop/src-tauri/crates/core/archi/src/api/check.rs` | ① 段改为 `match paths::entry_state(input.root, input.elements, element)`：`Resolved` → 继续 ②③④，`Absent` / `Invalid(problem)` → 一条 `ApiEntryMissing`；删 `has_generated` 及其两 kind 探测；新增私有 `fn entry_missing_message(element: &ArchiElement, state: &EntryState) -> String`（`Absent` 列候选、四种 `EntryProblem` 各点名越界目标 / 缺失文件 / 冲突方 FQN / 形态非法值） | AC-2 / AC-4；`category_of`、②–⑩ 判据与互斥序逐字不动 |
| `packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs` | ① `element_symbols(root, elements, element)`：`entry_state` → `Resolved` 取入口，否则空集；② `render_all`：删 `path.first()` 的 `scope`，改 `entry_state` → `Resolved{form, entry}` → `generated_kind(language_of(entry))` → `generated_path(root, &entry, kind)`；③ `index_entries`：`generated_rel` 改走 `generated_path_of(root, elements, element)`；④ `bootstrap`：`entry_state` → 失败即 `ApiError::Element` 并列出候选（D13）；⑤ `api_ref` / `api_ref_of` / 新增私有 `ref_projection`（D9）；⑥ 私有 `generated_path_of(endpoint…)` 入参由 `element_path: &[String]` 改 `(elements: &[ArchiElement], element: &ArchiElement)`；⑦ `module_refs` 与 `display` / `ref_name` / `reexport_target` 逐字不动 | `path.first()` 退出落点面；容器元素「`path` 空」的早退保留（`entry_candidates` 对空 `path` 返空表，二者同义） |
| `packages/desktop/src-tauri/crates/core/archi/src/api/diff.rs` | `generated_files_on_disk` 的 doc：旧落点样例 `crates/infra/archi.i.rs` / `views/welcome-view.tsx.i.ts` 改为入口同级形态（`crates/infra/archi/src/lib.i.rs` / `views/welcome-view.i.ts`） | 零逻辑改动（`orphaned` 按命名判据 `*.i.rs` / `*.i.ts`，与落点深度无关） |
| `packages/desktop/src-tauri/crates/core/archi/src/api/emit/mod.rs` | 模块 doc 与 `EmitTarget.form` doc 的「来自 `paths::entry_form`（盘上命中哪个入口文件）」改为「来自 `paths::entry_state` 的 `Resolved` 形态位」 | 零逻辑改动（渲染器零形态推导不变） |
| `packages/desktop/src-tauri/crates/core/archi/src/api/emit/ts.rs` | `render` / `render_scope` 的 doc 内 `paths::entry_form` 引用同改；补一句「`ts_import_specifier` 的入口同级真值归前端契约变更（待接线）」 | 零逻辑改动（`EntryForm` 三值与 `ts_import_specifier` 调用面不扩不改） |
| `packages/desktop/src-tauri/crates/core/archi/src/scan/owner.rs` | ① `OwnerEntry` 加 `stem: Option<String>`（`is_source_file(prefix)` 为真时 = 去最后一次扩展名的前缀）；② 新增私有 `enum StrippedCandidate { Prefix(String), Stem(String) }`，`strip_candidates` 返回三候选（`Prefix` 整组剥离、`Prefix` 替换扩展名、`Stem` 主干对齐）；③ `longest_prefix_hits` 收 `&StrippedCandidate`，`Stem` 只比对 `entry.stem`、`Prefix` 比 `entry.prefix`，长度取命中目标字符数；④ `owner_of` 的合并去重 / 最长档 / `Ambiguous` 逻辑逐字保留；⑤ 模块 doc 的「双形态并集」段改「三候选并集 + 入口同级」 | D10 / D11 / AC-5；`normalize` / `prefix_matches` / `module_roots` / `contains_count` 逐字不动 |
| `packages/desktop/src-tauri/crates/infra/archi/src/lib.rs` | `#[path = "../../archi.i.rs"]` → `#[path = "lib.i.rs"]`；`:10–17` 注释重写：生成物**落入口同级**、值恒为文件名（零 `../`）、等值性由 `paths_test` 断言（删「与 `mod_path_attr` 输出等值」「`mod_path_attr` 零产品调用者」两句） | proposal §六 #2 |
| `packages/desktop/src-tauri/crates/infra/archi.i.rs` | **删除** | 否则 `api:check` 的 `orphaned` 非空（`desktop-archi-api`「落点迁移后旧位置必须清空」） |
| `packages/desktop/src-tauri/crates/infra/agent/src/archi-knowledge.md` | ①「格式约定」的元素字段面补 `entry`（八项：`kind` / `gate` / `api` / `entry` / `path` / `docs` / `summary` / `metadata`），并补 `entry` 三条纪律（单值 / 只在约定默认指不到时写 / 单文件根 MUST NOT 写）；②「边界」段补一句：`api` 与 `entry` 是两个指针，MUST NOT 由它们扩张出协议正文；③ 示例 TOML 给 `App.Auth` 加一行单值 `entry = "packages/desktop/src/auth/token.ts"` | AC-6；四条判据的长表 MUST NOT 进知识文本（只指路） |
| `AGENT.md` / `CLAUDE.md` | `## 模块 API 契约` 第②条：「元素根同级的 `<root>.i.rs` / `<root>.i.ts`」→「实现入口同级的 `<入口主干>.i.rs` / `<入口主干>.i.ts`」；**条目数恒 5**、其余四条要点与节内其余条款逐字不动；成对同一提交 | D14 / AC-9（R-9） |
| `.dev-team/architecture/models/04-backend-shell.toml` | **零改动**（`git diff` 为空）：`entry` 归后续变更 | 13 条关系与元素计数逐字不变；本变更写入即触发旧宿主 `unknownField` |
| `.dev-team/architecture/models/05-backend-tools.toml` | 两个工具 crate 各加**一行** `api = "none"`（HEAD 字段面内）；`entry` 归后续变更 | 表头「3 元素 / 3 关系」与 3 条关系逐字不变 |
| `.dev-team/architecture/api/INDEX.toml` | 机械再生成：`App.Backend.Infra.ArchiRuntime.generated` 由 `packages/desktop/src-tauri/crates/infra/archi.i.rs` 改为 `packages/desktop/src-tauri/crates/infra/archi/src/lib.i.rs`；其余 41 条与文件头注释逐字不变 | 禁手改；`pnpm api:export` 产出 |
| `packages/desktop/src/types/generated/bindings.ts` / `bindings.zod.ts` | 机械再生成：`ArchiElement` 加 `entry: string \| null`（zod 侧 `entry: z.string().nullable()`） | 禁手改；`pnpm bindings:export` 产出（AC-7） |
| `packages/desktop/package.json` | `"version"` 相对落地时 `HEAD` 现值 +1 patch（实测起点 `0.6.43` ⇒ `0.6.44`；`tauri.conf.json` 经 `"version": "../package.json"` 跟随，`src-tauri/Cargo.toml` 的 crate 版本行 MUST NOT 随动） | 版本纪律 |

> **测试文件面归测试相位**（proposal「测试文件」表全量）：`crates/core/archi/src/api/{paths,check,mod}_test.rs`、`scan/owner_test.rs`、`src/{dto,model,parse/toml,parse/mod,search,graph,edit,tree}_test.rs`、`api/{contract,emit,extract}/**/_test.rs`、`tests/{golden,pilot,vocabulary}_test.rs`、`packages/desktop/src/views/architecture/**/*.test.tsx` 的用例本体不在本设计内。本设计只登记三处**机械锚点波动**（事实校准 11）：`element.path.first()` 在 `check.rs` / `api/mod.rs` 的两处源码断言、`mod_path_attr` / `entry_form` 的 `use` 与调用点、`api_ref_of` 的四处调用点。

> **不在本清单**：`.dev-team/architecture/api/**/*.md`（契约文本零改动，`api:export` 后 `App.Backend.Infra.ArchiRuntime.schema.json` 应零 diff）、`crates/tools/archi-api-gen/**`、`crates/core/archi/src/{lib.rs,scan/mod.rs,tree.rs,drift/**,docs.rs,graph.rs,search.rs,edit.rs}`、`foundation::layout`、`crates/core/config/**`、`packages/desktop/src-tauri/src/**`、`packages/desktop/knip.json`（`src/**/*.i.ts` 已在 `ignore`）、`package.json` 的脚本段（`api:export` / `api:check` / `check` 逐字不变）。`crates/infra/agent/src/archi_knowledge.rs` **零改动**：`entry` 的值域是开放集（任意 repo 相对路径），MUST NOT 进派生词表（`ElementKind::ALL` / `GateTier::ALL` / `ApiStatus::ALL` / `DiagnosticKind::ALL` 四面不变），字段知识只落静态骨架 md。

### 公共函数 / API

| 标识符 | 所在文件 | 变化 | 完整签名 | 说明 |
|---|---|---|---|---|
| `entry_candidates` | `crates/core/archi/src/api/paths.rs` | 新增 | `pub fn entry_candidates(element: &ArchiElement) -> Vec<String>` | 解析链候选的唯一产出点：**纯文本、零 IO、按优先序**（① 显式 `entry` 归一后 ② 文件型根 = 根文件自身 ③ 目录型根四条约定形态 ④ 空表）。不做校验（形态非法由 `entry_state` 判 `Malformed`） |
| `entry_state` | 同上 | 新增 | `pub fn entry_state(root: &Path, elements: &[ArchiElement], element: &ArchiElement) -> EntryState` | 四条判据（基准 / 落位 / 存在 / 跨元素唯一）与「首个落盘候选」的唯一判定点；**读盘只在此**（`is_file`）。`Resolved` 附形态位与入口串 |
| `generated_path` | 同上 | 修改（合并入参语义） | `pub fn generated_path(root: &Path, entry: &str, kind: GeneratedKind) -> Option<PathBuf>` | 入参由元素根改为**入口**；产出 `<入口目录>/<入口主干>.i.<ext>`；形态非法（`checked_root` 不过）→ `None` |
| `entry_form` | 同上 | **删除** | `pub fn entry_form(root: &Path, element: &ArchiElement) -> Option<(EntryForm, String)>`（原签名，退场） | 职责由 `entry_state` 整体接管（形态位进 `Resolved.form`，入口串进 `Resolved.entry`） |
| `mod_path_attr` | 同上 | **删除** | `pub fn mod_path_attr(form: EntryForm, name: &str) -> String`（原签名，退场） | 入口同级后 `#[path]` 值恒为 `generated_path(...).file_name()`，等值性归 `paths_test` 直接断言 |
| `ts_import_specifier` | 同上 | 保留（doc 改述） | `pub fn ts_import_specifier(form: EntryForm, name: &str) -> String` | **待接线形态**：入口同级规则（同目录 `./<入口主干>.i`、跨目录由入口到入口派生）归前端契约变更；当前输出 MUST NOT 当作既有真值 |
| `check`（`api::check::check`） | `crates/core/archi/src/api/check.rs` | 修改（① 段） | `pub fn check(input: ApiCheckInput<'_>) -> Vec<ArchiApiFinding>` | ① 改经 `entry_state`；`Absent` / `Invalid` ⇒ 一条 `apiEntryMissing`（文案按 `EntryProblem` 分列）；删 `has_generated`；②–⑩ 与互斥序逐字不变 |
| `render_all` | `crates/core/archi/src/api/mod.rs` | 修改（落点取入口） | `pub fn render_all(root: &Path, snapshot: &ApiSnapshot) -> Result<Vec<GeneratedFile>, ApiError>` | 生成物路径取 `Resolved` 入口；无可用入口的元素不产 `.i.*`（与「零内容不生成」同口径） |
| `bootstrap` | 同上 | 修改（解析链 + 文案） | `pub fn bootstrap(root: &Path, element: &str) -> Result<String, ApiError>` | 入口经 `entry_state`；失败文案列出候选与判据理由（D13） |
| `api_ref` | 同上 | 修改（内部） | `pub fn api_ref(root: &Path, fqn: &str) -> ArchiApiRef` | 取全元素表后委派 `api_ref_of`；元素缺席 / 模型错仍报契约事实（`ref_projection` 承接） |
| `api_ref_of` | 同上 | 修改（签名） | `pub fn api_ref_of(root: &Path, elements: &[ArchiElement], element: &ArchiElement) -> ArchiApiRef` | 第三参由 `element_path: &[String]` 改为元素表 + 元素本身（入口解析需跨元素唯一判据的射程） |
| `OwnerIndex::owner_of` | `crates/core/archi/src/scan/owner.rs` | 修改（内部判据） | `pub fn owner_of(&self, rel_path: &str) -> Owner` | 候选集扩为三候选（含主干对齐），对每候选各跑一遍最长前缀后合并定档；**签名与 `Ambiguous` 语义不变** |
| `OwnerIndex::build` | 同上 | 修改（内部，签名不变） | `pub fn build(elements: &[ArchiElement]) -> OwnerIndex` | 每条前缀额外算 `stem`（文件型 `path` 的去扩展名形态）；`module_roots` / `contains_count` 的返回语义不变 |

> 私有函数（`paths::{within_path, duplicate_of, form_of}`、`check::entry_missing_message`、`api::{generated_path_of, ref_projection}`、`owner::{strip_candidates, longest_prefix_hits, prefix_matches}`）按本表口径不列。`crates/core/archi/src/lib.rs` 的 `pub use api::{…}` 出口行零改动（`paths` 已是 `pub mod`，`EntryState` / `EntryProblem` 经 `archi::api::paths` 可达）。

### 类型定义

| 类型名 | 所在文件 | 变化 | 说明 |
|---|---|---|---|
| `Element.entry` | `crates/core/archi/src/model.rs` | 修改（字段面 +1） | `pub entry: Option<String>`；缺省 `None`；语义 = 单值 repo 相对实现入口指针（四条判据归 `api::paths`），插在 `api` 之后 |
| `RawElement.entry` | `crates/core/archi/src/parse/toml.rs` | 修改（反序列化面 +1） | `#[serde(default)] entry: Option<String>`；数组 / 表形态由 serde 归 `invalidValue`，拼错键由 `deny_unknown_fields` 归 `unknownField` |
| `ArchiElement.entry` | `crates/core/archi/src/dto.rs` | 修改（线面 +1，追加末尾） | `pub entry: Option<String>`；线格式 `entry`，缺省 `null`、**显式字段不省略**（与「投影不补默认值」同款） |
| `EntryState` | `crates/core/archi/src/api/paths.rs` | 新增 | `enum EntryState { Resolved { form: EntryForm, entry: String }, Invalid(EntryProblem), Absent }`；`#[derive(Debug, Clone, PartialEq, Eq)]`。`Resolved` 的 `form` 供 `EmitTarget.form`（TS 形态位），`entry` 是落点与取数的输入；`Absent` 是合法终态（不参与契约面） |
| `EntryProblem` | 同上 | 新增 | `enum EntryProblem { Malformed { entry: String }, OutsidePath { entry: String }, NotOnDisk { entry: String }, Duplicated { entry: String, other_fqn: String } }`；四个变体与 `apiEntryMissing` 的四条理由一一对应，`entry` 为归一后的声明值（文案点名用），`other_fqn` 为冲突方 |
| `StrippedCandidate`（私有） | `crates/core/archi/src/scan/owner.rs` | 新增 | `enum StrippedCandidate { Prefix(String), Stem(String) }`；`Prefix` 比对元素 `path` 归一前缀，`Stem` 比对文件型 `path` 的去扩展名形态。私有类型，不进公共面 |
| `OwnerEntry`（私有） | 同上 | 修改 | 加 `stem: Option<String>`（文件型 `path` 才有值） |
| `LocalArchiCheck` | `crates/infra/archi/src/lib.i.rs` | 位置迁移（内容逐字不变） | 生成类型 + 自动构造器，由 `crates/infra/archi.i.rs` 整体搬到入口同级；`pub fn new() -> Self` 与两行签名见证 `const _` 逐字不动 |

### 配置

| 配置键 | 所在文件 | 变化 | 值类型 | 默认值 / 现值 | 说明 |
|---|---|---|---|---|---|
| `version` | `packages/desktop/package.json` | 修改 | `string` | 落地时 `HEAD` 现值 +1 patch（实测起点 `0.6.43` ⇒ `0.6.44`） | `tauri.conf.json` 经 `"version": "../package.json"` 跟随；`src-tauri/Cargo.toml` 的 crate 版本行 MUST NOT 随动 |
| `entry`（模型元素字段） | 模型 `models/*.toml` | **本变更零处**（字段面新增在代码侧；三处数据归宿主重建后的后续变更） | `string`（可选，缺省 `null`） | 无（目标值：`App.Backend.Shell` → `.../src-tauri/src/lib.rs`；两个工具 crate → `.../crates/tools/<crate>/src/main.rs`） | 值与 `path` 同口径（`./` 前缀由归一剥除）；单文件根与 14 个 crate MUST NOT 声明；写早于宿主重建即被旧宿主判 `unknownField` |
| `api`（模型元素字段） | `.dev-team/architecture/models/05-backend-tools.toml` | 新增两处值 | `"declared" \| "none"` | `"none"`（两个工具 crate） | 二进制 crate 的公共面是 CLI 契约；与「无入口」是两个事实，本基线两类各有实例 |

> 配置项零新增：`.dev-team/config.json`、`crates/core/config/**`（含 `config.schema.json` 镜像）、`KNOWN_KEYS`、`tauri.conf.json`、`capabilities/default.json`、`package.json` 的 `scripts` 段全部零 diff。

---

## 数据模型 (Data Model)

### 模型字段（唯一 `entry` 面）

| 字段 | 类型 | 缺省 | 判据 | 持久化 |
|---|---|---|---|---|
| `element.<FQN>.entry` | 字符串（单值） | `None` / 线面 `null` | 基准（repo 相对 POSIX，`./` 不参与身份）/ 落位（等于某条 `path` 或落在某条目录型 `path` 之内）/ 存在（盘上必须有）/ 跨元素唯一（候选交集为零） | `<archi_dir>/models/NN-<域>.toml`，与 `path` / `docs` 同口径 |

- 违反判据 **不产生结构化诊断**（`DiagnosticKind` 仍九值、模型仍可读、画布不停摆），只产生一条 `apiEntryMissing`（`category = entry` / `warn`）。
- 本变更模型数据零 `entry` 声明（三处目标值见上表「配置」行，归后续变更）；51 元素 / 113 关系计数逐字不变。

### 派生关系（非持久化）

| 事实 | 由什么派生 | 唯一产出点 |
|---|---|---|
| 入口候选（有序表） | `path.first()` + `entry` 文本 | `paths::entry_candidates`（零 IO） |
| 入口（`Resolved`）与不可用理由 | 候选表 + 盘上存在性 + 落位 + 候选交集 | `paths::entry_state`（唯一读盘点） |
| 生成物落点 | 入口 + 语言（入口扩展名） | `paths::generated_path` |
| `#[path]` 字面量 | 生成物落点的文件名 | 手写入口行（字面量）+ `paths_test` 断言（`generated_path().file_name()`） |
| 入口形态位（`EntryForm`） | 入口与元素根的关系 | `paths::form_of`（`Resolved.form`） |
| `.i.<ext>` 的归属 | 三候选并集 + 最长前缀 | `scan::owner::OwnerIndex::owner_of` |
| Rust / TS 透传命名 | `path.first()`（**与入口无关**） | `paths::{module_name, rust_module_path}`（D40 单点） |

### 生成物 `INDEX.toml`（键集 / 类型 / 顺序不变）

| 列 | 类型 | 语义（改后） | 本仓取值 |
|---|---|---|---|
| `generated` | 字符串 | 该元素生成物的**入口同级**路径；无可用入口（`Absent` / `Invalid`）留空 | `App.Backend.Infra.ArchiRuntime` = `packages/desktop/src-tauri/crates/infra/archi/src/lib.i.rs`，其余 41 条不变 |

### 持久化面变化

| 对象 | 变化 |
|---|---|
| `crates/infra/archi.i.rs` | 删除 |
| `crates/infra/archi/src/lib.i.rs` | 新增（逐字搬迁） |
| `crates/infra/archi/src/lib.rs` | `#[path]` 值 + 注释（无持久化形态变化） |
| `.dev-team/architecture/api/INDEX.toml` | 一行值变化（机械再生成） |
| `packages/desktop/src/types/generated/{bindings.ts,bindings.zod.ts}` | `ArchiElement.entry` 一段（机械再生成） |

---

## 路由 / API 设计 (Route / API Design)

**零新增 IPC 命令、零新增 HTTP 端点、零新增 CLI 子命令**（`archi-api-gen` 的 `generate` / `--check` / `bootstrap` 三形态与退出码逐字不变）。唯一线格式变化是既有三条 IPC 响应共享的 `ArchiElement`：

| 面 | 方法 / 通道 | 输入 | 输出变化 | auth |
|---|---|---|---|---|
| `archi_graph` | Tauri IPC | `root: string` | `ArchiElement.entry: string \| null`（追加字段，缺省 `null`） | 无（本地进程内） |
| `archi_module` | Tauri IPC | `root: string`, `element?: string` | 同上（`element` / `children` 面同源） | 无 |
| `archi_search` | Tauri IPC | `root: string`, `query: string`, `scope` | 薄条目 `ArchiElementBrief` **不扩**（搜索面无 `path` / `api`，本变更不扩其字段面） | 无 |

- 契约面两条 IPC（`archi_api` 的 `check` / `generate`）入出参零变化（`orphaned` 的名称即路径，落点变化不改语义）。
- `bindings.ts` / `bindings.zod.ts` 由 `bindings:export` 再生成，MUST NOT 手写前端类型镜像（AC-7）。

---

## 依赖 (Dependencies)

运行时依赖：

| 依赖 | 变化 | 用途 |
|---|---|---|
| `std::path` / `std::fs`（`Path::is_file`） | 零新增（已在案） | 入口存在性探测（`entry_state` 唯一读盘点） |
| `crate::scan::is_source_file` | 新增**内部调用边**（`api/paths.rs` 与 `scan/owner.rs` 只读复用，函数签名不变） | 「文件型根 / 文件型 `path`」判据的扩展名白名单单源（零第二份清单） |
| `serde` / `toml` | 零新增（`entry` 走 `#[serde(default)] Option<String>`） | 模型字段反序列化与 `invalidValue` 分类 |
| `specta` | 零新增（`ArchiElement` 已实现 `Type`） | 线面类型生成 |

构建 / 测试依赖：**零新增**。`Cargo.lock` 零净增包、`[workspace.dependencies]` / `members` 零改动；前端零 npm 依赖变化（只多两处生成文件的一段）。`archi` crate 的模块依赖方向仍单向：`api → scan`（既有 `api/diff.rs` 已依赖 `scan::walk_source_files`，零新增边类型）。

---

## 验收标准回应

| AC | 设计落点 | 归属阶段（tasks） |
|---|---|---|
| AC-1 解析链三态可判、候选派生零 IO | D1（`entry_candidates` 纯文本 + `entry_state` 唯一读盘）、事实校准 1/4、D2（候选裁剪后仍四形态封闭） | 一（字段面）/ 二（解析链） |
| AC-2 四条判据各一反例 | D3（`EntryProblem` 四变体带载荷）、事实校准 4（文件型根声明别的文件 → `OutsidePath`）、事实校准 5（候选交集判 `Duplicated`） | 二（判据）+ 三（文案分列理由） |
| AC-3 落点迁移停在不动点 | D6 / D7（落点取入口、`#[path]` 值 = 文件名、`mod_path_attr` 删）、生成物搬迁三件套（新文件 / 删旧文件 / 入口行改写）、`INDEX.toml` 再生成 | 二 / 三（生成器侧）/ 五（搬迁） |
| AC-4 判据面数字与基线逐项相等（41 = 15 + 26） | D4 / D5（① 语义改为「无可用入口」、`has_generated` 退场）；13 个前端文件型根元素由 ① 面转 ② 面（数据零依赖）；壳与两个工具 crate 仍留 ① 面（显式 `entry` 归后续变更） | 三（判据面）/ 六（模型数据）；实测留痕由「新代码全量判定」跑出（重建前的宿主只能复现旧基线） |
| AC-5 归属判据覆盖新落点 | D10 / D11（三候选并集 + `OwnerEntry.stem`，`MUST NOT` 候选短路） | 四 |
| AC-6 模型面与知识面 | `Element.entry` / `RawElement.entry` / `ArchiElement.entry` + 知识文本八项字段面与示例单值 `entry` | 一（字段面）/ 七（知识文本） |
| AC-7 IPC 面再生成 | D9 + `ArchiElement.entry`（追加末尾）→ `bindings:export`；`knip.json` 已含 `src/**/*.i.ts`（零改动） | 八 |
| AC-8 零回归 | 事实校准 11（金样 `model` 段只含计数 ⇒ 零改写；`api-mismatch` 的 `src/mod/mod.rs` 由 `<root>/mod.rs` 约定候选取代旧探测后仍命中）、D10（非 `.i` 路径归属零变化） | 五 / 九（由桌面端门禁实测） |
| AC-9 纪律节与真实落点一致 | D14（只改第②条方位词与文件名形态；条目数恒 5；两份副本成对改） | 七 |

> **不列为 AC 的流程项**（沿 proposal 末尾声明）：`package.json` 版本纪律、`check` 四段链形态、`api = none` 的显式留白语义各有现成 spec 落点，本 delta 不改这些条款，故本设计不另立验收项；实现完成判据止于变更清单落地 + 阶段九的工作树核对，四段门禁（`server:check` → `bindings:export` → `api:check` → `client:check`）与 `apiFindings` 分桶实测由桌面端 static-check 承接。

---

## 待决问题 (Open Questions)

1. **`Duplicated` 的保守近似**（事实校准 5）：同一目录型根下 `src/lib.rs` 与 `mod.rs` 同时在盘时，跨元素声明 `mod.rs` 会被误判冲突。本仓零实例（AC-4 要求 `Invalid` 0 条）。若要精确化，需要「为其它元素跑完整解析并缓存」——那会把单元素解析变成全表解析，收益仅为消一个理论假阳，建议留待有真实需求时再评估，MUST NOT 在实现期顺手加缓存层。
2. **`entry` 指向目录或非源文件**：本变更只判「盘上存在」（`is_file`）+ 形态（`is_repo_relative`），不判「是不是源文件扩展名」。显式 `entry` 指向 `README.md` 的形态会 `Resolved` 并让 `language_of` 给 `Unknown`（→ Rust 渲染器）。当前无实例（proposal 未列该判据）；若后续要求，须与 `desktop-archi-api` 的落点口径一并裁决，MUST NOT 就地加第五条判据。
3. **`ts_import_specifier` 的接线形态**（R-7）：入口同级后正确值恒为 `./<目标入口主干>.i`（同目录），跨目录需「入口 → 入口」的相对派生。本变更只留规则与 doc 标注，实际接线归前端契约变更（变更 C）；接线时 MUST NOT 把当前 `../<name>.i` 输出当作既有真值。
4. **`ArchiElementBrief` 是否补 `entry`**：搜索 / 概览薄条目当前不含 `path`，本变更不扩其字段面（spec delta 未要求）。若将来模块入口文档（变更 E）需要薄条目展示入口，须在彼时扩面——本变更 MUST NOT 顺手加。
