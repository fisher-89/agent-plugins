# 验收留痕 · module-doc-field-docs-retirement（实现相位）

> 本文件按 tasks 阶段十一的记录面记录实测读数。取数面分两类：**实现面** = 本相位（七工具 +
> `bash`）可达的静态事实与既有命令读数；**移交面** = 需门禁 runner / 测试相位的读数（由桌面端
> static-check / test-execution 回填）。本相位不写用例、不跑测试门禁。

## §1 第六段读数（`apiFindings` 逐 `kind`）

取数通道：`cargo run -p archi-api-gen` 的一次性探针（临时加 `probe` 子命令读
`archi::check(CheckInput { mode: full, .. })`，取数后立即 `git restore` 该文件、净零改动）。

| 项 | 实测 |
|---|---|
| `path` 非空元素（分母） | **44** |
| `apiFindings` 总数 | **43** |
| `docMissing` | **43** = 41（`api = declared` 且派生契约 md 不在盘，**改名**自 `apiMissing` 形态①）+ 2（`api = none` 且未声明 `doc`，**首次入面**） |
| `apiEntryMissing` | **0** |
| `apiMissing` | **0**（形态① 移出后只剩「有约定无兑现」，本仓零实现漂移） |
| 其余十值 | 各 **0** |
| `status` | **clean** |
| `passed` | **true**（43 条全 `warn` 且 `docMissing` 两侧都不纳管） |
| `errors` / `warnings` / `structuralErrors` | 0 / 0 / 0 |

**成因必须与原话一起读**：这不是「新增 43 条债」，而是同一批 41 份契约换了个桶名（`apiMissing`
形态①→`docMissing`）加上 2 个 `api = none` 元素首次进入判据面，分母 44 不变。`apiMissing` 归零
是「形态①移出」，不是「实现漂移清零」。MUST NOT 把 41 与 43 并排写成债务增长。

## §2 模型读数

| 项 | 实测 |
|---|---|
| 元素 / 关系 | **53 / 120**（与基线一致） |
| `diagnostics` | **空**（`01-app.toml` 删 `docs` 行与白名单同批落地 ⇒ 无 `unknownField`） |
| `modelHash` | `sha256:8dca725d…`（`HEAD`）→ **`sha256:339b8d6d…`**：删一行字节即变，**必然变化**（MUST NOT 记成不变） |
| `path` 非空 / `api = none` / 契约在案 | 44 / 2 / 1（与基线一致） |

## §3 生成物与租约面

| 项 | 实测 |
|---|---|
| `pnpm api:export` | `0 份生成物已写入（3 份未变）`；`git status` 在 `.dev-team/` 下仅 `models/01-app.toml` 一行删除 ⇒ `graph/INDEX.toml` / `models/Infra/*.schema.json` / `.i.*` 逐字节零变化（`doc` 不是生成物） |
| `pnpm api:check` | 退出 0、三列（`mismatched` / `orphaned` / `missing`）皆空 |
| `pnpm bindings:export` | 连续两次导出后 `git diff packages/desktop/src/types/generated/` 稳定（幂等）；`DocRef` / `DocKind` / `DocSource` / `WarningKind` / `ArchiWarning` 离场，`ArchiElement.doc` / `ArchiModuleDetail.doc` / `ArchiSearchResult.doc` 进场，`SearchScope` 无残留（`docs` 词面已换） |

## §4 语料守护

| 项 | 实测 |
|---|---|
| `crates/core/archi/src/api/**` sidecar | 仍恰 **13** 份（新判据用例须落既有文件） |
| `crates/core/archi/tests/corpus/api/` | **零存在** |
| `crates/core/archi/src/docs*.rs` | **已删除**（随 `lib.rs` 的 `pub mod docs;` / `mod docs_test;`） |
| `corpus/drift/expected/*.json` | 16 份；逐份预测见下（重录归测试相位，`DESKTOP_GOLDEN_REWRITE=1` 显式执行 + 人工确认） |

十六份金样的变化逐份留痕（**预测值**，重录后以实测为准）：

- `api-mismatch.full.json`：`apiFindings` 条目不变（同一条 `apiSignatureMismatch`，首错序下它仍是
  该元素唯一命中：契约在盘含 fence → 入口 `Resolved` → 契约声明的 `file_stem` 已在代码导出集中
  ⇒ `apiMissing` 不命中）；**变化点是 `status`：`warnings` → `errors`**——`apiSignatureMismatch`
  是 `error` 档且该元素纳管（契约在盘），`derive_status` 新分支把 `status` 抬到 `errors`，
  门禁 `passed` 同步 `false`（报告 json 不承载 `passed`，故仅 `status` 出 diff）。
- 其余 **15 份**：用例目录下无 `contracts/` ⇒ `enabled = false` ⇒ `apiFindings = []`，与既有
  `status` 一致，**零变化**。
- 首错短路未改变任何既有金样的条目数（无「同元素多报」的 case 落在语料内）；重录时若出现第二条
  变化，须按「哪一条因首错被短路」逐条追因。

## §5 零扩散与纪律核对

| 项 | 实测 |
|---|---|
| `git status --porcelain` | 改动集 ⊆ design D18 白名单（23 个修改 / 2 个删除 + 本 change 目录）；零 `api/extract/**`、`api/emit/**`、`api/render.rs`、`api/schema.rs`、`api/diff.rs`、`api_artifacts.rs`、`drift/config.rs`、`tree.rs`、`scan/**`、前端 `contract-panel.tsx` / hooks / `drift-marks.ts` / `architecture-canvas.tsx`、`archi_tool.rs`、`sdk/policy.rs`、`models/0[2-6]-*.toml`、`models/Infra/**`、`decisions/**`、`openspec/specs/**` |
| 新建文件 | 零新增源码；未新建两份工具 crate `README.md`、未新建 41 份契约 md（数据面 2b 不在本 change） |
| 版本纪律 | `packages/desktop/package.json` `0.6.52` → **`0.6.53`**（+1 patch）；`tauri.conf.json` 仍 `"version": "../package.json"`；`src-tauri/Cargo.toml` 的 `[package] version = "0.1.0"` 逐字不变 |
| `AGENT.md` / `CLAUDE.md` | 该节正文逐字相同；要点数 5 → 6 |
| `archi_decisions_*` / `DECISIONS_DIR_NAME` | 产品源码零命中（`*_test.rs` 内的用例由测试相位同批清理） |

## §6 实现面偏差记录（须与 design 一并读）

1. **`ArchiElement.doc` 为 design 类型表的隐含项**：`paths::carrier_path(start, &ArchiElement)`、
   `api::doc_facts(&[ArchiElement])`、`api::gating_findings`（`none` 支取 `element.doc.is_some()`）与
   `check`（⑥ 矛盾判据、③ 声明值文案）四处都以 `ArchiElement` 为入参，故该 DTO 必须携带模型的
   `doc`（与既有 `entry` 指针同型）。出线面因此比「两处进场」多一处 `ArchiElement.doc`；`bindings.ts`
   重导后如实生效（新增字段，零破坏）。
2. **`ApiUnexpected` 在 `check.rs` 的出现次数为 3**：两处构造（`none` 支 ① / `declared` 支 ⑥）+
   一处 `category_of` 分类臂。tasks 的自查项按「构造数 2」记，实际 grep 计数含分类臂。
3. **启用判据单点 = 私有 `facts_enabled(&doc_facts)`**：`api::enabled(root, elements)` 与
   `api::findings`（一次 `load_contracts` 后由 `snapshot.doc_facts` 派生）共用同一判定本体，
   `artifact_check` / 命令层只经公开 `enabled` 抵达——不存在第二套「递归任意 md」判据。
4. **`parse_args` 等 CLI 面零改动**：探针只作一次性取数（`git restore` 归零）。

## §7 静态检查第一轮反馈的修复留痕（实现相位）

`pnpm run server:check` 首轮红在**测试面编译**（`cargo clippy --all-targets` 连带 lib test 与
integration test 一并编译）。修复动作全部为编译级机械随动，零产品语义改动：

| 文件族 | 机械动作 |
|---|---|
| `foundation/src/layout/mod_test.rs` | 删 `archi_decisions_dir` / `archi_decisions_rel_dir` 的用例段与断言，三函数同源用例降为两函数（`archi_dir` / `archi_models_dir`） |
| `archi/src/**/*_test.rs`、`archi/tests/*` | `ArchiElement` 构造器补 `doc`；`ApiCheckInput` / `ApiSnapshot` 字面量补 `doc_facts`；`Element.docs` → `doc`；`SearchScope::Docs` → `Doc`；删 `linked_doc_paths` / `DocRef` / `DocKind` / `DocSource` / `WarningKind` 引用；`ApiFindingSeverity` 穷尽 match 补 `Error` 臂 |
| `api/mod_test.rs` 的 `enabled` 节 | 判据换面（载体在盘且含 fence），旧「递归任意 `*.md`」用例整节重写为四条新语义用例（fence 判定 / 杂项 md 不误启用 / `doc` 载体与越界形态 / 容器零参与）；单点自证改锚 `facts_enabled` / `doc_facts` |
| `commands/archi/mod_test.rs`、`agent/src/archi_knowledge_test.rs` | 出参 `docs` / `warnings` → `doc`；知识文本断言 `docs` → `doc`、决策记录目录串整条删除 |
| 前端 `*.test.ts(x)` | `ArchiElement` fixture 补 `doc: null`；`ArchiModuleDetail.docs` fixture → `doc` |

**并发现两处真实回归（产品侧，已修）**：① `model.rs` 的 `api` 字段 doc 因折行把既有守卫串
「架构域 `models/<Domain>/` 目录」切断（`dto_test::三份产品源文doc方位串换段` 判红），已重排
使该串连续；② `paths_test::形态判据三单点各一处定义` 的 `is_repo_relative` 消费计数随 `doc_path`
新增由 4 改 5。

**行尾纪律**：批量编辑曾把若干 Rust 文件写成 CRLF（工作区态），`include_str!` 断言的 `
}
`
因此失配——已全量归一为 LF（`git ls-files --eol` 零 `w/crlf`），`cargo fmt --all --check` 与
`vp check` 的格式门同时恢复绿。

**当前门禁读数**：`pnpm run check` 四段全绿（`server:check` / `bindings:export` / `api:check` /
`client:check`）；`cargo fmt --all --check` 通过；`vp check` 报 270 文件格式正常、260 文件零
warning 零类型错；`knip` 仅剩既有配置提示。

## §8 移交面（由桌面端回填）

- `pnpm check` 四段（`server:check` / `bindings:export` / `api:check` / `client:check`）与
  `vp check` / `knip` / `cargo nextest` 的结论。
- 十六份 drift 金样的 `DESKTOP_GOLDEN_REWRITE=1` 重录与人工确认（本文件 §4 为预测）。
- 测试相位对 `*_test.rs` / `tests/**` / `app.test.tsx` 的机械随动（`doc_facts`、`doc`、`DocFact`、
  `SearchScope::Doc`、首错断言、`runner_test` 的 `passed` 反转等）。
