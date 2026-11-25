# 测试设计: archi-agent-context-review

> **日期**: 2026-11-25
> **输入**: 本 change 目录 `proposal.md`（16 条 AC、R1–R16、P1–P11）、`design.md`（C1–C14、「新增文件」0 行 + 「修改文件」28 行 + 「条件性修改」4 行、「公共函数 / API」27 行、「内部接线面」12 行、「类型定义」15 行、D-1…D-23、附录 A / B）、`specs/` 11 份 delta。
> **性质**: 真相搬移（读面换 `<FQN>.schema.json`）+ 写面补闸（`validate_schema`）+ 判据被动收缩（13 → 10）+ 工具面三资产 + Agent 上下文重写 + 生成面让位与规则面同步的原子交付。零模型字节、零 `.i.rs` / `.i.ts` 字节、零 `.schema.json` 迁移（11 份与 fence 逐项等价）⇒ 本变更**无新增源文件**，被测范围 = design「修改文件」表的 28 行。
> **重做收口（本轮回溯）**: `is_pure_ref_alias` 的消费者是**三处**——`check` 的 `declared_types`、`contract::Contract::from_schema` 的 `types` / `reexports` 分流、`dto` 的 `interface_asset` 三数组二分——三处**共用同一份判据**（`check.rs` 节「单点」用例据此改为三份语料拼接计数：`is_pure_ref_alias(` 合计恰 4 = 定义 1 + 调用 3），并同批修正三处与源码不符的形态断言：`from_schema` 对本地 alias 的去路（既不入 `types` 也不入 `reexports`）、`required ⊆ properties` 的射程（`$defs.<名>.required[]` ⊆ 同条目 `properties`）、`is_pure_ref_alias` 自身的判据边界（只问「`$ref` 是唯一键」）。
> **路径口径**: `crates/**` / `src/**` / `views/**` 一律指 `packages/desktop/` 之下；本文写仓库相对全路径。
> **框架识别**: 后端 = Rust 内置单元测试——`#[test]` / `#[tokio::test]` 属性宏 + std 断言宏（`assert!` / `assert_eq!` / `expect_err` / `panic!`），**零第三方断言库**；测试文件与源文件同目录共置 `<name>_test.rs`，由 `#[cfg(test)] mod <name>_test;` 在 `src/api/mod.rs` / `src/lib.rs` / `sdk/mod.rs` / 各目录 `mod.rs` 登记（`src/api/mod.rs` 尾部 1128–1141 行为该形态）。前端 = `vite-plus/test`（vitest 兼容 API）+ `@testing-library/react` + jsdom（`vite.config.ts` 的 test 节与 `vp test`），断言 `expect(...).toEqual(...)`；测试与组件同目录 `*.test.tsx` / `*.test.ts`。
> **测试装置**: `crates/core/archi` **零 dev-dependencies**（临时目录用 std 自建 `TempDir` + `AtomicU64` 计数器，沿 `api/check_test.rs` / `edit/warnings_test.rs` 既有先例）；`crates/infra/agent` 用既有 `tempfile`，异步执行体用 `#[tokio::test]`（沿 `archi_tool_test.rs` 先例）；`crates/tools/archi-api-gen` 用自建 `TempDir`（既有）。前端 IPC 边界经 `vi.mock('@tauri-apps/api/core')` 注入 `invoke`（既有装置）。测试运行 / 覆盖率 / 变异由桌面端 test-execution 执行，本文不复制命令细节。
> **同批约束（落地顺序）**: ① `Contract::from_schema` / `validate_schema` 与夹具迁移**同一提交**——`api/{mod,check,schema,contract,diff}_test.rs` 的输入源从「md 源文 → `contract::parse` → `build_schema`」改为手写 schema JSON，否则测试无法编译；② `edit/mod_test.rs` 只改调用点入参（`created` 语义），断言面挂 `warnings_test.rs`；③ 前端 `use-archi-module.test.ts` 的 `detailOf()` 与 `architecture-panel.test.tsx` 的 `detailOf()` 是 `ArchiModuleDetail` 唯一夹具处——`assets` / `reading` 两必填字段落地即红，属机械随动。
> **条件性修改不入被测范围**: design「条件性修改」表 4 行是 Q-1 / Q-6 / Q-7 / Q-8 的修正落点（文本 / 注释面，获裁决才执行）；本文按「未裁决即零动作」设计，其文本面归不可测试项 6。
> **用例名口径**: 沿既有「中文用例名 + 小写 ASCII 标识符」写法（如 `load_contracts全链`）；需要 ASCII 驼峰时补 `#[allow(non_snake_case)]`。用例名不写测试命令、不写断言细节。
> **纪律**: 用例只断言**行为与单点归属**；源码锚只钉「名字 / 调用序 / 计数 / 零命中」，MUST NOT 对已被行为用例覆盖的语义写第二份判据（沿 `api/mod_test.rs` / `dto_test.rs` 先例）。不新增 test-only 导出（AGENT.md 第 5 条）——私有件经 `super::…` 直调。

---

## 验收范围

<!-- 「被测文件或模块」列 = 承载用例的测试文件（沿本仓既有口径）；「—」即该 AC 无单测面，
     替代保证写进不可测试项。每个源文件到测试文件的映射见本段末表（design「修改文件」28 行
     逐行覆盖，无遗漏、无删除文件）。 -->

| AC ID | 验收条件 | 被测文件或模块 |
|---|---|---|
| AC-1 | 零行为回归（读面）：`.dev-team` 根重跑 `archi_drift`，除 `timestamp` / `durationMs` 与 38 条 `docMissing` 的 `message` 文案外逐字段不变（`apiFindings.total` 仍 38、`byCategory` 全 `doc`、`status = clean`、`passed` 不变） | `packages/desktop/src-tauri/crates/core/archi/src/api/mod_test.rs`（schema-only 装载零差 + `docMissing` 单因计数）、`.../api/contract_test.rs`（`from_schema` 解包 ⇒ 签名见证不退化）、`.../api/check_test.rs`（两支链与 `docMissing` 文案）；仓级读数见不可测试项 3 |
| AC-2 | 生成物零漂移：`api:check` 零差异且 `orphaned` **不含**任何 `.schema.json`；`.i.rs` / `.i.ts` 逐字节不变；`INDEX.toml` 差异只允许头部说明段一句 | `.../api/mod_test.rs`（`render_all` 三类产物）、`.../api/diff_test.rs`（孤儿扫描收面）、`packages/desktop/src-tauri/crates/tools/archi-api-gen/src/main_test.rs`（`generate` / `--check` 收面）、`.dev-team/architecture/graph/INDEX.toml`（生成物，见不可测试项 3）；逐字节与仓级面见不可测试项 3 / 4 |
| AC-3 | 绑定面可归因：`bindings:export` 后 `git diff` 只含 `assets` / `reading` + 4 个新 DTO 类型 + `ArchiApiFindingKind` 联合三值离场；命令包装零新增 | `packages/desktop/src-tauri/src/bindings/mod_test.rs`（`DTO_TYPES` / `ARCHI_DTO_TYPES` 补 4 + 联合三值离场 + 计数）、`.../views/architecture/hooks/use-archi-module.test.ts`（`detailOf` 夹具）、`.../views/architecture/components/dashboard/contract-panel.test.tsx`（`ArchiApiCheck` 夹具）；`git diff` 归因见不可测试项 3 |
| AC-4 | 写闸三类坏输入（坏 JSON / `$id` 与文件名不符 / `properties` 与 `$defs` 重名）各一条 `Err` 且盘上字节零变化；同一文件随后合法写入成功并回「接口已校验」 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools_test.rs` |
| AC-5 | 写闸与装载同源：把写闸放行的合法 schema 手工改坏（绕过 `write`）后跑 `archi_drift` ⇒ `ApiError` 停摆而非半真报告（文案点名文件与期望值） | `.../core/archi/src/api/schema_test.rs`（唯一入口同一实现）、`.../api/mod_test.rs`（`load_contracts` / `findings` / `artifact_check` 的 `Err` 传递）、`.../sdk/tools_test.rs`（写闸放行侧）；`drift::check` 映射由既有 `.../core/archi/src/drift/mod_test.rs` 族背书（该文件零改动） |
| AC-6 | 三资产一次给全：`archi_module` 响应含 `assets.topology` / `assets.interface` / `assets.doc` 三段与 `reading`；三段 `present` 各自独立（「只有 schema」「只有 md」「两者都无」三态夹具各验一次） | `.../core/archi/src/dto_test.rs`、`.../infra/agent/src/sdk/archi_tool_test.rs`、`.../views/architecture/components/model-info.test.tsx`、`.../views/architecture/components/dashboard/architecture-node.tsx`（注释面，见不可测试项 7） |
| AC-7 | 双草稿与 `link`：`bootstrap` 返回 `{ schema, doc }`，schema 骨架可直接落盘并过写闸、doc 骨架只有 H1 + 一句（零段名）；`link` 算出的跨元素 `$ref` 被 `apiRefMissing` 判可达 | `.../core/archi/src/api/mod_test.rs`（双草稿）、`.../infra/agent/src/sdk/archi_tool_test.rs`（三动作 + 可达性组合）、`.../crates/tools/archi-api-gen/src/main_test.rs`（CLI 单行 JSON）、`.../core/archi/src/lib.rs`（出口面；用例落 `.../core/archi/src/api/mod_test.rs`） |
| AC-8 | 词汇与档位：`ArchiApiFindingKind::ALL` 长度 10、`as_str` / `severity` 三方对拍；`error` 恰 `apiMissing` / `apiSignatureMismatch` 两项；`category` 七值、无空组 | `.../core/archi/src/drift/report_test.rs`、`packages/desktop/src-tauri/src/bindings/mod_test.rs`（联合三值离场） |
| AC-9 | 纳管口径：`declared` ∧ schema 在盘的元素集合 = 11（与今日读数同）；`docMissing` 永不纳管；`status == errors ⟺ passed == false` 不变量成立 | `.../core/archi/src/api/mod_test.rs`（`gating_findings` / `enabled`）、`.../core/archi/src/drift/report_test.rs`（档位蕴含）、`.../core/archi/src/model.rs`（`api` / `doc` 字段语义注释面）；「= 11」为仓级读数（不可测试项 3） |
| AC-10 | 上下文面：知识骨架与全文体量在新冻结锚（1800 / 2000 B）内；三份 prompt 新句断言在场，其余 11 份零 archi 工具字样；change 臂两写权相位 `preamble` 在场、`implement/executor` 缺席 | `.../infra/agent/src/archi-knowledge.md`（同 `archi_knowledge_test.rs`）、`.../infra/agent/src/archi_knowledge.rs`（同文件）、`.../core/workflow/src/write/phase_table_test.rs`（三份 prompt）、`.../infra/agent/src/compose_test.rs` |
| AC-11 | 规则面：`AGENT.md` 与 `CLAUDE.md` 的「模块 API 契约」节各恰 5 条要点、两份节内正文逐字相同、节内零 `api-*` fence 示例、零 `openspec/specs/` 路径 | `AGENT.md` + `CLAUDE.md`（文本资产，见不可测试项 2） |
| AC-12 | 写面 warnings 收窄：`archi_element` 的五条 conditions 逐条可绿（`noPath` / `pathNotFound` / `docMissing`（仅 `created`）/ `apiUnexpected` / 删除后的 `apiUnexpected`）；只改 `doc` 三次（写值 / 清键 / 指向不存在文件）`warnings` 均为 `[]`；`DocNotFound` 在写面零出现 | `.../core/archi/src/edit/warnings_test.rs` |
| AC-13 | `docMissing` 单因单义：构造「派生 md 在盘、schema 缺失、`path` 非空且在盘、入口已解析」⇒ 判据面零条目；既有元素 upsert 的写面 `warnings` 亦为空 | `.../core/archi/src/api/check_test.rs`、`.../core/archi/src/edit/warnings_test.rs` |
| AC-14 | 版本纪律：`package.json` 相对落地时 `HEAD` +1 patch；`tauri.conf.json` 跟随；`src-tauri/Cargo.toml` 的 crate 版本行零变化 | `packages/desktop/package.json`（一行字面量，见不可测试项 1） |
| AC-15 | `archi_search` 语料换面：四态夹具 × 两值 `api` 逐格 ⇒ 语料恰为「在盘 `<FQN>.md` ∪ 在盘 `<FQN>.schema.json`」；`doc` 字段零参与；`scope` 两半边空数组、`"docs"` 显式 `Err` 并列三合法值、空 `query` 显式 `Err`；`decisions/**` / `openspec/specs/**` / `path` 内散落 md 零命中；源码零 `carrier_path` / `doc_path` 引用 | `.../core/archi/src/search_test.rs`、`.../core/archi/src/dto_test.rs`（检索投影）、`.../core/archi/src/api/paths_test.rs`（退役零命中）；仓级 grep 见不可测试项 5 |
| AC-16 | `archi_api` 两格下发：`(proposal, executor)` 六名 / `(dev-design, executor)` 七名且 definitions 与 `RunStarted.tools` 逐名一致；其余矩阵行与矩阵外相位不含 `archi_api`；「含 `archi_element`」⟺ `model_write_lock == None` 逐格仍相等、`archi_api` 档位仍 `Read`、`READONLY_TOOLS` 三工具名零变化 | `.../infra/agent/src/compose_test.rs`、`.../infra/agent/src/sdk/mount_test.rs`（definitions 逐名来源） |

**源文件 → 测试文件映射**（design「新增文件」0 行 + 「修改文件」28 行逐行给出）：

| 源文件（design 修改文件） | 测试文件 | 映射依据 |
|---|---|---|
| `packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs` | `packages/desktop/src-tauri/crates/core/archi/src/api/mod_test.rs`（既有，改判 + 扩写） | 既有 `#[cfg(test)] mod mod_test;`；`load_contracts` / `render_all` / `bootstrap` / `api_ref` 用例族已在该文件 |
| `packages/desktop/src-tauri/crates/core/archi/src/api/contract.rs` | `packages/desktop/src-tauri/crates/core/archi/src/api/contract_test.rs`（既有，改判） | 既有登记；`parse` / `verify` / `Contract` 用例族在该文件（`parse` 面整体退场） |
| `packages/desktop/src-tauri/crates/core/archi/src/api/schema.rs` | `packages/desktop/src-tauri/crates/core/archi/src/api/schema_test.rs`（既有，改判 + 扩写） | 既有登记；`build_schema` / `verify_defs` / `build_index` 用例族在该文件 |
| `packages/desktop/src-tauri/crates/core/archi/src/api/paths.rs` | `packages/desktop/src-tauri/crates/core/archi/src/api/paths_test.rs`（既有，改判） | 既有登记；`carrier_path` / `doc_path` / `ref_relative` 用例族在该文件 |
| `packages/desktop/src-tauri/crates/core/archi/src/api/diff.rs` | `packages/desktop/src-tauri/crates/core/archi/src/api/diff_test.rs`（既有，扩写） | 既有登记；`compare` 三列用例族在该文件 |
| `packages/desktop/src-tauri/crates/core/archi/src/api/check.rs` | `packages/desktop/src-tauri/crates/core/archi/src/api/check_test.rs`（既有，改判） | 既有登记；十判据求值链用例族在该文件 |
| `packages/desktop/src-tauri/crates/core/archi/src/edit/warnings.rs` | `packages/desktop/src-tauri/crates/core/archi/src/edit/warnings_test.rs`（既有，改判） | 既有 `edit/mod.rs` 尾部登记的 `mod warnings_test;`；七条 conditions 用例族在该文件 |
| `packages/desktop/src-tauri/crates/core/archi/src/model.rs` | `packages/desktop/src-tauri/crates/core/archi/src/model_test.rs`（既有，微扩） | 既有 `#[cfg(test)] mod model_test;`；八字段面解析用例族在该文件 |
| `packages/desktop/src-tauri/crates/core/archi/src/drift/report.rs` | `packages/desktop/src-tauri/crates/core/archi/src/drift/report_test.rs`（既有，改判） | 既有登记；四组 `ALL` 与 `as_str` / `severity` 对拍用例族在该文件 |
| `packages/desktop/src-tauri/crates/core/archi/src/dto.rs` | `packages/desktop/src-tauri/crates/core/archi/src/dto_test.rs`（既有，扩写） | 既有 `#[cfg(test)] mod dto_test;`；`module_detail` / `ArchiApiRef` 用例族在该文件 |
| `packages/desktop/src-tauri/crates/core/archi/src/search.rs` | `packages/desktop/src-tauri/crates/core/archi/src/search_test.rs`（既有，改判） | 既有登记；检索语料用例族在该文件（design 表 `search` 行注「实现落 `search.rs`」） |
| `packages/desktop/src-tauri/crates/core/archi/src/lib.rs` | `packages/desktop/src-tauri/crates/core/archi/src/api/mod_test.rs`（既有，扩写） | 本 crate 无 `lib_test.rs` 惯例（`dto_test.rs` doc 明写）；出口面锚经 `include_str!("../lib.rs")` 落 `api/mod_test.rs`，沿归档 `archi-edit-engine-primitives` 先例 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools.rs` | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools_test.rs`（既有，扩写） | 既有 `write` / `edit` 用例族 + `tempfile` 装置 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool.rs` | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool_test.rs`（既有，扩写） | 既有 `archi_api` / `archi_module` `#[tokio::test]` 装置 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs` | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount_test.rs`（既有，改判） | 既有 `registry十八项` / 定义面用例族 |
| `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs` | `packages/desktop/src-tauri/crates/infra/agent/src/compose_test.rs`（既有，改判 + 扩写） | 既有 `注入矩阵八行逐行命中` / `model_write_lock` 用例族 |
| `packages/desktop/src-tauri/crates/infra/agent/src/archi-knowledge.md` | `packages/desktop/src-tauri/crates/infra/agent/src/archi_knowledge_test.rs`（既有，改判） | 语料测试文件以 `include_str!("archi-knowledge.md")` 直读骨架；体量门限锚在该文件 |
| `packages/desktop/src-tauri/crates/infra/agent/src/archi_knowledge.rs` | `packages/desktop/src-tauri/crates/infra/agent/src/archi_knowledge_test.rs`（既有，扩写） | 同上（`build` / `knowledge` 拼装与降级用例在该文件） |
| `packages/desktop/src-tauri/crates/core/workflow/src/prompts/proposal-planner.md` | `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table_test.rs`（既有，改判） | `include_str!` 编译期语料断言 prompt 正文的唯一归属地（`prompt_text` helper） |
| `packages/desktop/src-tauri/crates/core/workflow/src/prompts/dev-design-planner.md` | `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table_test.rs`（既有，改判） | 同上 |
| `packages/desktop/src-tauri/crates/core/workflow/src/prompts/implementation-generator.md` | `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table_test.rs`（既有，改判） | 同上（既有 `implementation-generator` 断言族已在末段） |
| `packages/desktop/src-tauri/crates/tools/archi-api-gen/src/main.rs` | `packages/desktop/src-tauri/crates/tools/archi-api-gen/src/main_test.rs`（既有，改判） | 既有 `generate落四类生成物` / `bootstrap只出stdout` / `check*` 用例族 |
| `AGENT.md` + `CLAUDE.md` | —（无测试文件） | 纯文本资产，无编译单元 ⇒ 不可测试项 2 |
| `packages/desktop/package.json` | —（无测试文件） | 一行版本号，无行为可断言 ⇒ 不可测试项 1 |
| `.dev-team/architecture/graph/INDEX.toml` | —（生成物，无独立测试） | 派生产物；断言面 = `main_test.rs` 的 `generate` / `--check` 夹具（tempdir 复刻）+ `api:check` ⇒ 不可测试项 3 |
| `packages/desktop/src/views/architecture/components/dashboard/contract-panel.tsx` | `packages/desktop/src/views/architecture/components/dashboard/contract-panel.test.tsx`（既有，改判） | 既有五态 / chip / 零类别常量断言族在该文件 |
| `packages/desktop/src/views/architecture/components/model-info.tsx` | `packages/desktop/src/views/architecture/components/model-info.test.tsx`（既有，扩写） | 既有 `ContractSection` 渲染用例族在该文件 |
| `packages/desktop/src/views/architecture/components/dashboard/architecture-node.tsx` | `packages/desktop/src/views/architecture/components/architecture-panel.test.tsx`（既有，机械随动） | 本文件无 dedicated test（proposal 表列的 `architecture-node.test.tsx` 不存在）；渲染宿主 = `architecture-panel.tsx` → `ArchitectureDashboard` → `ArchitectureNode`，其 `detailOf()` 夹具是 `ArchiModuleDetail` 的驱动面 |

---

## 单元测试

<!-- 框架与装置：Rust `#[test]` + std 断言宏，零第三方断言库；`crates/core/archi` 零
     dev-dependencies（tempdir 用 std 自建 `TempDir` + `AtomicU64`），`crates/infra/agent`
     用既有 `tempfile`，异步执行体 `#[tokio::test]`。前端 = `vite-plus/test` +
     `@testing-library/react` + jsdom，IPC 经 `vi.mock('@tauri-apps/api/core')`（既有装置）。

     夹具迁移总口径（**同批**）：`api/{mod,check,schema,contract,diff}_test.rs` 与
     `dto_test.rs` / `search_test.rs` 里「md 源文 → `contract::parse` → `build_schema`」的
     三件套装置，本变更后一律改为**手写 schema JSON 字符串**（`json!` 或原始文本）——
     `parse` / `build_schema` 退役后旧装置无法编译。`pilot_markdown()` 等 helper 更名为
     `pilot_schema()` 之类的等价物，断言面逐条对拍既有语义。

     跨模块组合用例一律挂链路发起方 / 最上层调用方，不新建集成章节：
     - `bootstrap → validate_schema` 挂 `api/mod.rs` 节（草稿的产出方）；写盘半边挂 `sdk/tools.rs` 节；
     - `link → write → check` 挂 `sdk/archi_tool.rs` 节（发起方）；
     - `write / edit → validate_schema → load_contracts / findings` 挂 `sdk/tools.rs` 节（写闸是发起方）；
     - `upsert_warnings → edit::upsert_element 既有调用点` 挂 `edit/warnings.rs` 节（`edit/mod.rs` 零改动）；
     - DTO 新字段 → 前端夹具编译面挂 `dto.rs` 节（载荷发起方）。 -->

### packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/mod_test.rs

<!-- 挂 AC-1（读面零回归）、AC-2（`render_all` 三类）、AC-5（停摆传递）、AC-7（双草稿）、
     AC-9（纳管口径）、AC-15（语料换面的启用判据面）。
     条目取自 design「公共函数 / API」子表中「所在文件 = api/mod.rs」的 6 行
     （`load_contracts` / `enabled` / `gating_findings` / `render_all` / `bootstrap` /
     `api_ref`·`api_ref_of`）与「类型定义」的 `BootstrapDrafts`（新）/ `ApiSnapshot`（改）/
     `DocFact`（删除）三行；未做源文件导出扫描。
     「内部接线面」的 `doc_presence` / `schema_draft` / `doc_draft` 只经上述公开入口断言
     （私有件由 `super::…` 直调，零新增导出）。
     必改断言：`enabled按载体fence判定` / `enabled杂项md不误启用` / `load_contracts缺md零装载` /
     `render_all四类生成物齐备` / `bootstrap草稿过parse段序恰六段` / `api_ref为none时三路径全空`
     六族随语义换锚改写，MUST NOT 以「删断言换绿」收场。 -->

#### 待测功能

- `load_contracts()`: 同签名；只读 `<FQN>.schema.json` 经 `validate_schema` 装配 `Contract`，不在盘跳过（零 md 回落）、坏文件 `ApiError` 停摆；通过后 `Contract.file` 覆写为 repo 相对路径（`display`）
- `enabled()`: 同签名；任一 `path` 非空 ∧ `api = declared` 元素 schema 在盘（`api = none` 的越界 schema 不构成启用证据）
- `gating_findings()`: 同签名；入选 = `element` 非空 ∧ `api = declared` ∧ schema 在盘（文档在盘与否零参与）
- `render_all()`: 同签名；只产 `.i.rs` / `.i.ts` / `INDEX.toml` 三类（零 `.schema.json`）
- `bootstrap()`: 返回 `BootstrapDrafts { schema, doc }`；schema 骨架可直接过写闸、doc 骨架只有 H1 + 一句；失败面三理由逐字不变
- `api_ref()` / `api_ref_of()`: 同签名；三路径各自按自身在盘事实出线，`status` 改判 schema 在盘
- `BootstrapDrafts`（类型）: `{ schema: String, doc: String }`——两出口（CLI / `archi_api`）共用，零第二份拼装
- `ApiSnapshot`（类型）: `doc_facts` → `doc_present: BTreeMap<String, bool>`（`DocFact` 类型删除）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `load_contracts()` · schema-only 装载 | 正向 | tempdir 落 1 元素 + 同目录 `<FQN>.schema.json`（1 op + 1 类型、零 md）⇒ `contracts.len() == 1`；`contracts[0].ops[0].signature` 与 `schema.properties.<op>.x-function`（去掉 `description`）逐键相等；`contracts[0].file` == repo 相对 schema 路径——`只有schema即装载且签名即解包值` | 单点 |
| `load_contracts()` · 零 md 回落 | 异常 | 只有派生 `<FQN>.md`（含历史 `api-op` fence）而 schema 缺席 ⇒ 该元素零装载、`schemas` 无该键（Q4 零回落）；两者都不在 ⇒ 同样零条目——`md在盘零回落` | 矩阵迭代（2 形态） |
| `load_contracts()` · 坏 schema 停摆 | 异常 | 三形态：JSON 语法错 / `$id` 与文件名不符 / op 名与类型名撞名 ⇒ `Err(ApiError::…)` 且文案含文件名与期望值；零半真 `ApiSnapshot`（`findings` / `artifact_check` 同源返回 `Err`）——`坏schema停摆不产半真报告` | 矩阵迭代（3 形态） |
| `load_contracts()` · `api = none` 与空 `path` | 边界 | `api = none` 元素在盘 schema ⇒ 不装载（既有 `load_contracts跳过api_none` 维持）；`path = []` 元素在盘 schema ⇒ 不装载；空元素表 ⇒ 零 contracts 零 schemas——`留白与空path零参与` | 矩阵迭代（3 形态） |
| `load_contracts()` · 规模与幂等 | 边界 | 100 元素规模面稳定序；同输入两次调用产出逐字段相等（零随机序）——`规模面稳定且幂等` | 单点 |
| `enabled()` · 五形态判据 | 正向 | `declared` + `path` 非空 + schema 在盘 ⇒ `true`；同条件 schema 缺（md 在盘）⇒ `false`；`api = none` + schema 在盘 ⇒ `false`；`path = []` + schema 在盘 ⇒ `false`；空元素表 ⇒ `false`——`启用只认declared且schema在盘` | 矩阵迭代（5 形态） |
| `enabled()` · 单点锚 | 边界 | `include_str!("mod.rs")` 的 `enabled` 函数体含 `schema_path(` 与 `is_file(`、零 `has_api_fence` / `facts_enabled`；`artifact_check` 与 `findings` 各恰一处 `enabled(` 调用（零第二份判定）——`启用判定单点且零第二份` | 单点 |
| `gating_findings()` · 入选口径 | 正向 | `declared` ∧ schema 在盘元素的三条 `docMissing` / `apiMissing` / `apiSignatureMismatch` ⇒ 全入选；`docMissing` 对 `declared` 但 schema 缺席的元素 ⇒ 不入选；`api = none` 的 `apiUnexpected` ⇒ 不入选——`纳管等于declared且schema在盘` | 矩阵迭代（3 形态） |
| `gating_findings()` · 边界 | 边界 | 空 `findings` ⇒ 空 vec；`element = None` 的 finding ⇒ 不入选；`docMissing` 永不入选（构造 38 条 `docMissing` 语料 ⇒ 空集）——`文档债永不纳管` | 矩阵迭代（3 形态） |
| `render_all()` · 三类产物 | 正向 | 快照含后端 + 前端各 1 元素 ⇒ 返回集恰含 `INDEX.toml` + `.i.rs` + `.i.ts` 三类；每类路径与 `paths::index_path` / `generated_path` 派生逐字相等——`只产三类生成物` | 单点 |
| `render_all()` · schema 零生成 | 异常 | 即使 `snapshot.schemas` 有值（含坏形态）⇒ 输出集零 `.schema.json`（`.schema.json` 后缀零命中）；`render_all` 不读 `schemas`（源码锚：零 `SCHEMA_FILE_SUFFIX`）——`schema不再由生成器产出` | 单点 |
| `render_all()` · 空快照与无入口 | 边界 | 空快照 ⇒ 零内容不生成 `.i.*`、INDEX 留空（既有维持）；无可用入口 ⇒ 同上；幂等两次逐字节相等（既有维持）——`空快照与幂等` | 既有（维持） |
| `bootstrap()` · 双草稿形状 | 正向 | `BootstrapDrafts { schema, doc }`：`schema` 过 `validate_schema(fqn, &schema)` 成功、每 op 一项 `properties.<k>.x-function`（`description` 为 `""`）、`$comment` == `SCHEMA_WRITER_NOTICE`；`doc` == `# <FQN>` + 一句、零 `##` 段名、零 ```` ``` ```` fence——`双草稿可直接落盘` | 矩阵迭代（3 锚） |
| `bootstrap()` · 空符号集 | 边界 | 元素无可抽取符号 ⇒ `$defs {}` + `properties {}` 两空对象且仍过写闸；`doc` 仍两行形态（零段名）——`留白写空schema` | 单点 |
| `bootstrap()` · 失败面 | 异常 | 元素不存在 / 无可用入口 / 符号抽取失败 ⇒ `ApiError` 文案逐字不变（既有四理由候选文案维持）；失败零部分草稿——`草稿失败面逐字不变` | 矩阵迭代（3 形态） |
| `api_ref()` / `api_ref_of()` · 三路径独立 | 正向 | 只有 schema ⇒ `schemaPath` 非空、`contractPath` / `generatedPath` 为 `null`（键显式在场）；只有派生 md ⇒ `contractPath` 非空、`schemaPath` 为 `null`；三者都在盘 ⇒ 三条全出——`三路径各按自身在盘事实出线` | 矩阵迭代（3 形态） |
| `api_ref_of()` · `status` 换锚 | 异常 | `declared` ∧ schema 缺（md 在盘）⇒ `status = "none"`；`declared` ∧ schema 在盘 ⇒ `"declared"`；`api = none` ⇒ `"none"` 且三路径仍按各自在盘事实（旧「none 时三者全空」断言必改）——`status只认schema在盘` | 矩阵迭代（3 形态） |
| `api_ref_of()` · 派生单点 | 边界 | 源码锚：三路径各取 `paths::contract_path(` / `schema_path(` / `generated_path(` 恰一处，零手拼 `format!("{}…", fqn)`；`api_ref` 与 `api_ref_of` 同源（`api_ref` 委托）——`三路径派生单点` | 单点 |
| `BootstrapDrafts` / `ApiSnapshot`（类型） | 边界 | `BootstrapDrafts` 序列化键序 `["schema", "doc"]` 两键且均为 `String`；`ApiSnapshot.doc_present` 为 `BTreeMap<String, bool>`；`include_str!("mod.rs")` 零 `DocFact` / 零 `doc_facts`——`载体事实收成单布尔` | 单点 |
| 组合：`bootstrap() → validate_schema()` | 正向 | `bootstrap(root, fqn).schema` 直接喂 `validate_schema(fqn, &schema)` ⇒ `Ok`（写闸与草稿同源；写盘半边在 `sdk/tools.rs` 节）——`草稿即过闸` | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | 真实 tempdir（std 自建 `TempDir`）+ 手写 schema JSON 夹具 + 真实 `validate_schema` / `extract`；路径派生与 `is_file` 即被测语义，替身会抹掉「在盘 / 不在盘」 | 本节全部用例 |
| `serde_json` | 不 mock——文本层重复键与解包语义都由它承载 | `load_contracts` / `bootstrap` 臂 |
| 既有 fixture helper | `pilot_markdown()` → schema 文本 helper（同批改名 + 改写），断言面逐条对拍 | `load_contracts` / `render_all` / `api_ref` 臂 |

### packages/desktop/src-tauri/crates/core/archi/src/api/contract.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/contract_test.rs

<!-- 挂 AC-1（R2 回归锚：`ops[i].signature` == `x-function` 解包值）、AC-8 的复用三闸面。
     条目取自 design「公共函数 / API」子表的 `Contract::from_schema`（新）/ `parse`（删除）/
     `verify`（修改）/ `is_constructor`（不变）四行与「类型定义」的 `Contract`（改）/
     `ContractOp`（不变）/ `ContractSection`（删除）三行。
     必改断言：全文 2890 行的 md 解析器用例族整体退场（`parse` 退役）；保留并改判的是
     `verify` 三闸与 `is_constructor`；`duplicate_keys` / `is_identifier` 的可见性用例改为
     经 `super::contract::{…}` 直调（编译期即判，零新增导出）。 -->

#### 待测功能

- `Contract::from_schema()`: 新增投影（无 `Result`）：解包 `properties.<k>.x-function` 至 `signature`（丢 `description`）、`line = 0`、`types` = 非纯 alias 的 `$defs`、`reexports` = 可反查目标 FQN 的纯 alias
- `verify()`: 同签名，输入改判 schema 投影；保留七族名字闸 / `x-function` 落点闸 / `required ⊆ properties` 三闸
- `is_constructor()`: 不变（三条消费面共用：`check` / `bootstrap` / `emit`）
- `Contract`（类型）: 删 `reexport_prose` / `sections`；`fqn` / `title` / `file` / `ops` / `types` / `reexports` 保留
- `ContractOp`（类型）: 不变；`line` 恒 `0`（schema 无行号）
- 退役面: `parse()` 与 md 解析件（`finish_section` / `parse_object*` / `collect_reexport_prose` / `collect_surface_table` / `file_title` / `FENCE_OP` / `FENCE_TYPES` / `ContractSection`）零命中

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `Contract::from_schema()` · R2 回归锚 | 正向 | 夹具 schema 1 op（`x-function` 带 `owner` / `params` / `returns`）⇒ `contract.ops[0].signature` 与 `schema.properties.<op>.x-function` 逐键相等（**包裹态已解**：`signature["returns"]` 直取）；`contract.ops[0].line == 0`——`签名即x-function解包值` | 单点（回归锚） |
| `Contract::from_schema()` · `description` 丢弃 | 正向 | `x-function.description` 在场 / 缺席两臂 ⇒ `signature` 均无 `description` 键，其余键（`owner` / `params` / `returns` / `async`）逐字保留——`描述不进签名` | 矩阵迭代（2 形态） |
| `Contract::from_schema()` · `types` / `reexports` 二分 | 正向 | `$defs` 三形态：跨元素纯 `$ref` alias（带目标文件主干、`reexport_target` 可反查出目标 FQN）⇒ `reexports` 值 == 目标 FQN；本地 alias `#/$defs/<名>` ⇒ **既不入 `types` 也不入 `reexports`**（纯 alias 已落 `continue`，而 `reexport_target` 对零文件部分的引用反查 `None`；其名字由 `verify` 的 `$defs` 键名闸覆盖）；带 `properties` 的定义 ⇒ `types`——`透传与类型按纯alias二分` | 矩阵迭代（3 形态） |
| `Contract::from_schema()` · 空 schema | 边界 | `$defs {}` + `properties {}` ⇒ `ops` / `types` / `reexports` 全空；`fqn` / `title` / `file` 取入参逐字——`空schema投影为零` | 单点 |
| `Contract::from_schema()` / `ContractOp`（类型） · 名称面边界 | 边界 | `properties` 含非 op 键（无 `x-function` 的纯 `$ref` 镜像）⇒ 不进 `ops`；1000 键规模稳定序；`$defs` 名含 Unicode ⇒ 原样保留（八族闸在 `verify`，不在投影）——`投影零过滤` | 矩阵迭代（3 形态） |
| `verify()` · 七族名字闸 | 异常 | 非法 op 名五形态（中划线 / 空格 / emoji / 空串 / 1000+ 字符）⇒ `Err` 点名该 token；合法 ASCII 标识符 ⇒ `Ok`——`七族名字闸唯一判据` | 矩阵迭代（6 形态） |
| `verify()` · `x-function` 落点闸 | 异常 | `x-function` 作 `properties.<k>` 兄弟键 / `x-function` 值非对象 ⇒ `Err`；`x-function` 恰在值对象内 ⇒ `Ok`——`落点闸` | 矩阵迭代（3 形态） |
| `verify()` · `required ⊆ properties` 闸 | 异常 | `$defs.<名>.required[]` 的元素不在同条目 `properties` 键集内（含非字符串元素，文案点名条目与 `$defs.<名>.required[<i>]` pointer）⇒ `Err`；`required` 全在同条目 `properties` 内 / `required` 缺席或非数组 ⇒ `Ok`（射程只 `$defs` 条目，不判 `x-function.params[]`）——`required子集闸` | 矩阵迭代（3 形态） |
| `verify()` · 死闸零残留 | 边界 | `include_str!("contract.rs")` 的 `fn verify` 体内零 `reexport_prose` / `sections` / 散文表格闸；三闸各一处调用——`三闸保留其余退场` | 单点 |
| `parse()` 退役面 | 异常 | 源码 `include_str!("contract.rs")` 零 `pub fn parse` / `FENCE_OP` / `FENCE_TYPES` / `collect_reexport_prose` / `collect_surface_table` / `struct ContractSection`；行数 ≤ 500（设计要约 400–500）——`md解析器整体退场` | 单点 |
| `is_constructor()` · 语义锁定 | 既有（维持） | 五形态（大写首字母构造函数 / 小写非构造 / 空串 / 带点路径 / 超长）逐值往返；三条消费面（`check` / `bootstrap` / `emit`）共用同一函数（源码锚各一处调用）——`构造函数判据不变` | 既有（维持） |
| `duplicate_keys()` / `is_identifier()` · 可见性 | 边界 | `pub(crate)` / `pub`：经 `super::contract::{duplicate_keys, is_identifier}` 直调成功（编译期即判）；`duplicate_keys` 对重复键文本返回首个重复键与位置、无重复 ⇒ `None`；`is_identifier` 六形态（含中划线 / emoji / 超长）——`可见性供写闸与link复用` | 矩阵迭代（2 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | 手写 schema JSON 值（`json!`）+ 真实 `serde_json` 解析；`reexport_target` / `schema_fqn` 真实调用（`reexports` 的目标 FQN 反查是被测语义） | 本节全部用例 |
| md 解析装置 | 整体删除（`parse` 退役）——MUST NOT 保留「旧 md 夹具仍可解析」的恒真断言 | `parse()` 退役面 |

### packages/desktop/src-tauri/crates/core/archi/src/api/schema.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/schema_test.rs

<!-- 挂 AC-4（三类坏输入里的 `$id` 与键空间两臂）、AC-5（写闸与装载同源）、AC-7（草稿过闸）。
     条目取自 design「公共函数 / API」子表的 `validate_schema`（新）+ 六个 `verify_*`（新）+
     `build_schema`（删除）八行与「修改文件」表 `build_index` 的 `present` 头注释改写；
     `verify_defs` / `degraded_marks` / `build_index` / `ApiFormStatus` / `DEGRADED_KINDS`
     逐字保留，`verify_defs` 用例既有维持。
     闸序（D-2 / design C1）：① 文本层重复键 → ② JSON 可解析 → ③ identity → ④ 键空间 →
     ⑤ 形态白名单 → ⑥ 镜像 → ⑦ 跨元素 `$ref` 前缀 → ⑧ 顶层键封闭 → ⑨ 复用三闸 + `verify_defs`。
     `verify_defs` 归 ⑨ 不另编号 ⇒ 本节不给它编闸号。 -->

#### 待测功能

- `validate_schema()`: 唯一公开入口 `(fqn, text) -> Result<Contract, ApiError>`——九道闸一趟走完并产出 `Contract`（`file` = `<FQN>.schema.json`）
- `verify_identity()`: `$id` == `<FQN>.schema.json`、`title` == FQN、文件主干 == FQN 三者一致
- `verify_keyspace()`: op 键集（`properties` 里带 `x-function` 者）∩ `$defs` 键集 = ∅（D-3）
- `verify_shape()`: `properties.<k>` 恰二选一——`{ "x-function": {…} }`（值须对象）或 `{ "$ref": "#/$defs/<名>" }` 且 `<名> ∈ $defs`
- `verify_mirror()`: `$defs` 每键在 `properties` 有指向自己的 `$ref`
- `verify_ref_prefix()`: 跨元素 `$ref` 前缀逐字等于 `ref_relative(本FQN, 目标FQN)`；文案并列当前串与期望串
- `verify_top_level_keys()`: 顶层键封闭六项（`$schema` / `$id` / `title` / `$comment` / `$defs` / `properties`）；`$id` / `title` / `$defs` / `properties` 必在场
- `verify_defs()`: 零改造复用（五条规则 + 透传纯 `$ref` + `x-class` 带属性判错）
- `build_index()`: `present` 头注释与说明段改判「schema 在盘 = 接口文件在案」；值域与条目字典序不变
- 退役面: `build_schema()` 零命中；`GENERATED_NOTICE` 逐字保留、新增 `SCHEMA_WRITER_NOTICE`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `validate_schema()` · 合法最小 | 正向 | `$schema` / `$id` / `title` / `$comment` / `$defs {}` / `properties {}` ⇒ `Ok`，`Contract` 的 `ops` / `types` / `reexports` 全空、`file` == `<FQN>.schema.json`——`空壳合法` | 单点 |
| `validate_schema()` · 合法带面 | 正向 | 1 op + 1 类型（镜像 `$ref`）+ 1 跨元素透传 alias ⇒ `Ok`；`ops[0].signature` 为解包值、`types` 含 1、`reexports` 含 1——`合法面全过` | 单点 |
| `validate_schema()` · 九闸逐闸反例 | 异常 | ① 文本含重复键（`serde_json` 会折叠 ⇒ 必须 ① 先抓）② 非法 JSON ③ `$id` / `title` / 文件主干三条不符（三臂）④ op 名与类型名撞名 ⑤ `properties.<k>` 兄弟键 / `$ref` 指向 `$defs` 外（两臂）⑥ `$defs` 键无镜像 ⑦ 跨元素 `$ref` 前缀差一字符 ⑧ 未知顶层键 ⑨ `$defs.<名>.required[]` 越界（元素不在同条目 `properties` 键集内）⇒ 各一条 `Err`，文案点名文件与期望值——`九闸各自独立可判` | 矩阵迭代（13 形态） |
| `validate_schema()` · 闸序 | 边界 | ①–② 的坏文本同时违反 ③（重复键 + `$id` 不符）⇒ 文案落在 ①（先发生）；③–⑧ 同时违反多闸 ⇒ 文案按固定闸序取最前——`闸序固定` | 矩阵迭代（2 形态） |
| `validate_schema()` · 文本层边界 | 边界 | 空串 / 纯空白 / 1 MB 超长 / emoji / 含 `\0` / 带 BOM ⇒ 全 `Err` 且不 panic（`\0` 与 BOM 由 ② 兜）；空白合法性由 ② 决定——`文本边界不panic` | 矩阵迭代（6 形态） |
| `verify_identity()` · 三臂 | 异常 | `$id` 缺 / 非字符串 / 不等于 `<FQN>.schema.json`；`title` ≠ FQN；文件主干 ≠ FQN ⇒ 各一条 `Err` 并列期望值——`identity三合一` | 矩阵迭代（5 形态） |
| `verify_identity()` · 正例 | 正向 | 三者一致（含 FQN 四段 / 单段两形态）⇒ `Ok`——`identity正例` | 矩阵迭代（2 形态） |
| `verify_identity()` · 边界 | 边界 | `$id` 大小写差异 / 前后空白 / 全角字符 ⇒ `Err`（逐字相等判据，零归一）——`identity逐字` | 矩阵迭代（3 形态） |
| `verify_keyspace()` · 撞名 | 异常 | 同名既是 op（带 `x-function`）又是 `$defs` 键 ⇒ `Err` 点名两键空间——`同名不成两用` | 单点 |
| `verify_keyspace()` · 正例与边界 | 正向 | `$defs` 键集与 op 键集不相交（今 11 份资产形态）⇒ `Ok`；`$defs` 空 / `properties` 空 ⇒ `Ok`；仅 `properties` 有非 op 的 `$ref` 镜像键（与 `$defs` 同名）⇒ `Ok`（D-3：不是「properties 与 $defs 无重名」）——`键空间只判同名两用` | 矩阵迭代（3 形态） |
| `verify_shape()` · 白名单 | 异常 | `{x-function: 非对象}` / `{x-function, $ref}` 双键 / `{$ref}` 指向 `$defs` 外 / `properties.<k>` 空对象 / `properties.<k>` 为数组 ⇒ 各一条 `Err`——`形态恰一键` | 矩阵迭代（5 形态） |
| `verify_shape()` · 正例与边界 | 正向 | `{x-function: {…含将来未知键}}`（内层自由）⇒ `Ok`；`{$ref}` 指向在册 `$defs` ⇒ `Ok`；`properties {}` 空 ⇒ `Ok`——`内层不校验` | 矩阵迭代（3 形态） |
| `verify_mirror()` · 镜像 | 异常 | `$defs.<名>` 无 `properties.<名>` 的自我 `$ref` ⇒ `Err`；`properties.<名>` 的 `$ref` 指向别的名字 ⇒ `Err`——`defs每键须自我镜像` | 矩阵迭代（2 形态） |
| `verify_mirror()` · 正例与边界 | 正向 | 三类型全镜像 ⇒ `Ok`；`$defs {}` ⇒ `Ok`——`无类型无镜像要求` | 矩阵迭代（2 形态） |
| `verify_ref_prefix()` · 前缀逐字 | 异常 | 同 crate 兄弟 / 跨 crate / 越根 `../../../` 三形态：前缀 == `ref_relative` 输出 ⇒ `Ok`；差一字符 / 少一层 `../` / 多一段目录 ⇒ `Err` 且文案并列当前串与期望串——`前缀由ref_relative单点决定` | 矩阵迭代（5 形态） |
| `verify_ref_prefix()` · 边界 | 边界 | 本模块内 `#/$defs/<名>`（零文件部分）⇒ 不作跨元素判（`Ok`）；目标 FQN 非 FQN 形态 ⇒ `Err`；`$ref` 非字符串 ⇒ `Err`——`本模块不判前缀` | 矩阵迭代（3 形态） |
| `verify_top_level_keys()` · 封闭 | 异常 | 六键外任一（`additionalProperties` / `definitions` / `type`）⇒ `Err`；缺 `$id` / `title` / `$defs` / `properties` 各一臂 ⇒ `Err`——`顶层键封闭` | 矩阵迭代（7 形态） |
| `verify_top_level_keys()` · 边界 | 边界 | `$schema` / `$comment` 可选（在场 / 缺席两臂）⇒ `Ok`；顶层值非对象（数组 / 字符串）⇒ `Err`——`可选键与非法形态` | 矩阵迭代（3 形态） |
| `verify_defs()` · 零改造复用 | 既有（维持） | 五条规则 + 透传纯 `$ref` 校验 + `x-class` 带属性判错，既有用例逐条保留（入参本就是 `&Value`，零改造）——`defs规则逐字保留` | 既有（维持） |
| `build_index()` · `present` 换锚 | 正向 | `present` 判 schema 在盘（不依赖 `contract.is_some()`）；说明段第二行改「review 口径：读 `<FQN>.schema.json`（接口，手写真相）与 `<FQN>.md`（理由）；`declared` = 接口文件在案」；值域仍 `declared` / `absent`、条目字典序不变——`present换锚而字节最小化` | 单点 |
| `build_index()` · 边界 | 边界 | 空条目 ⇒ 说明段 + 零 `[entry]`；`degraded` 段与 `[entry.categories]` 形状分布 / 整段省略逐字不变（既有用例维持）——`index形状零改` | 既有（维持） |
| 退役面 · `build_schema` / 写者注 | 异常 | 源码 `include_str!("schema.rs")` 零 `pub fn build_schema`；`GENERATED_NOTICE` 逐字保留（`.i.*` 字节不变的前提）+ `SCHEMA_WRITER_NOTICE` 在场且含「手写接口真相」与「`archi_api` 的 `check` 自查」——`生成器让位而注记分家` | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | 手写 schema 文本（原始 `&str`）+ 真实 `serde_json` 解析、真实 `ref_relative` / `schema_fqn`；闸序与文案即被测语义 | 本节全部用例 |
| `ApiMeta` | 字面量构造（既有先例，零替身类型） | `build_index` 臂 |
| 文件系统 | 不用磁盘——`validate_schema` 无路径入参（`Contract.file` 恒 `<FQN>.schema.json`），装载期覆写由 `load_contracts` 用例覆盖 | 全节 |

### packages/desktop/src-tauri/crates/core/archi/src/api/paths.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/paths_test.rs

<!-- 挂 AC-15 的「源码零 carrier_path / doc_path」与「零第二份载体派生」面。
     条目取自 design「公共函数 / API」子表的 `carrier_path` / `doc_path`（删除）、
     `schema_fqn`（新）、`is_fqn`（可见性）、`checked_root`（可见性）四行；
     「修改文件」表同批登记 `is_repo_relative` 可见性不变（仍有 `paths.rs` 内读者）。
     必改断言：`carrier_path` / `doc_path` 的 2924 行用例族整族退场；`contract_path` /
     `schema_path` / `index_path` / `entry_state` / `ref_relative` / `generated_path` 的既有
     用例逐条保留（逐字不变是 `link` 与 `verify_ref_prefix` 的共同单点前提）。 -->

#### 待测功能

- `schema_fqn()`: `pub fn schema_fqn(file_name: &str) -> Option<&str>`——`<FQN>.schema.json` 去后缀的**唯一**反查（`reexport_target` 与写闸同调）
- `is_fqn()`: 私有 → `pub`（`link` 的 FQN 形态判据复用，零第二份）
- `checked_root()`: `pub(crate)`（写面 `path_on_disk` 复用「归一 + 越根」判据）
- 退役面: `carrier_path()` / `doc_path()` 零命中（资产① 落点恒为派生 `<FQN>.md`）
- 逐字不变面: `contract_path` / `schema_path` / `index_path` / `entry_state` / `ref_relative` / `generated_path` / `directory` / `normalize_root` / `language_of`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `schema_fqn()` · 正例 | 正向 | `App.Auth.schema.json` ⇒ `Some("App.Auth")`；四段 FQN 同式；`contract_path` / `schema_path` 的落盘名往返一致——`文件名反查唯一` | 单点 |
| `schema_fqn()` · 非法形态 | 异常 | `App.Auth.md` / `schema.json`（零主干）/ `X.schema.JSON`（大写）/ 空串 ⇒ 全 `None`——`非schema名零命中` | 矩阵迭代（4 形态） |
| `schema_fqn()` · 边界 | 边界 | 超长名（1000+）/ 含 emoji / 多重后缀 `.schema.json.json` ⇒ `None` 或按既有后缀规则（不 panic）；单段 FQN ⇒ `Some`——`边界不panic` | 矩阵迭代（3 形态） |
| `is_fqn()` · 可见性 | 边界 | 经 `paths::is_fqn`（`archi::is_fqn` 出口）直调成功；合法四段 / 单段 / 空串 / 带中划线四形态逐值——`link复用同一判据` | 矩阵迭代（4 形态） |
| `checked_root()` · 归一与越根 | 边界 | `""` / `"."` / `"./"` / `"a/./b"` / `"../x"` / 绝对路径 ⇒ 与既有 `entry_state` 同判据（`pub(crate)` 直调：`None` 或归一后串）；两次调用幂等——`写面复用同一归一` | 矩阵迭代（6 形态） |
| `carrier_path()` / `doc_path()` 退役 | 异常 | `include_str!("paths.rs")` 零 `pub fn carrier_path` / `pub fn doc_path`；`grep` 同 crate 源码零 `carrier_path(` / `doc_path(` 引用（仓级面见不可测试项 5）——`载体派生单份` | 单点 |
| 逐字不变面 | 既有（维持） | `ref_relative` 六形态（兄弟 / 跨 crate / 同 crate 深层 / 单段 / 越根 / 自身）、`entry_state` 四情形、`generated_path` 两类、`contract_path` / `schema_path` / `index_path` 派生——既有断言逐条保留（本条是 `link` / `verify_ref_prefix` 的锚）——`派生单点零改动` | 既有（维持） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | 路径派生为纯函数；`is_file` 存在性用 tempdir 真实落盘（沿既有先例） | 全节 |
| `foundation::layout` | 不 mock——`archi_models_dir` 派生是被测前提 | `contract_path` / `schema_path` 臂 |

### packages/desktop/src-tauri/crates/core/archi/src/api/diff.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/diff_test.rs

<!-- 挂 AC-2（`orphaned` 不含 `.schema.json`）。
     design「公共函数 / API」子表**未声明本文件的公共 API 变更**（`compare` / `api_dir_artifacts`
     均不在表内）；本节的被测面来自「修改文件」表第 5 行（`api_dir_artifacts` 收面只留
     `<archi_graph_dir>/INDEX.toml`）与 R1（收面与 `render_all` 让位同一提交）。故本节按
     「文件级被测面」登记，MUST NOT 据源码导出声明虚构函数条目。 -->

#### 待测功能

- 文件级被测面（design 修改文件表第 5 行）: 孤儿扫描收面——`*.schema.json` 退出 `api_dir_artifacts` 枚举，`orphaned` / `mismatched` / `missing` 三列语义与稳定序逐字不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 孤儿扫描收面 | 正向 | tempdir 落 11 份 `<FQN>.schema.json`（均不在应生成集）+ 1 份孤儿 `.i.rs` + 1 份 `INDEX.toml` ⇒ `orphaned` 恰含后两者、`*.schema.json` 零条目（R1 的「全红」症状不出现）——`手写schema不进孤儿` | 矩阵迭代（3 形态） |
| 三列语义零变 | 既有（维持） | `mismatched`（内容不符）/ `orphaned`（应删未删）/ `missing`（应生成未生成）三列定义与稳定序、深度上限、符号链接不跟随、有界 `read_dir` 四条纪律逐条保留——`三列语义逐字不变` | 既有（维持） |
| 收面边界 | 边界 | 空目录 / 只有 `INDEX.toml` / 深 5 层嵌套的 `.i.rs` ⇒ 三列各自为空或如既有；`.schema.json` 出现在应生成集为空时也不报「应生成未生成」——`空态零误报` | 矩阵迭代（3 形态） |
| 异常面 | 异常 | 未知后缀文件（`X.schema.json.bak` / `INDEX.toml.bak`）⇒ 不进三列；目录不可读 / 符号链接环 ⇒ 按既有纪律跳过或 `Err`，不 panic、不半写——`收面零误伤` | 矩阵迭代（2 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | 真实 tempdir + 真实 `walk_source_files` / `workspace_render`（应生成集与盘面比对即被测语义） | 全节 |

### packages/desktop/src-tauri/crates/core/archi/src/api/check.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/check_test.rs

<!-- 挂 AC-1（`docMissing` 文案与条数构成）、AC-8（`category` 七组）、AC-13（单因单义）。
     条目取自 design「公共函数 / API」子表的 `category_of`（修改）与 `is_pure_ref_alias`
     （可见性）两行；两支求值链（`none` 2 步 / `declared` 4 步）与三值删除来自
     「修改文件」表第 6 行（C6），按「文件级被测面」登记（`check::check` 不在公共 API 子表内）。
     必改断言：`api::findings` 侧的 `apiReexportDrift` / `apiDocMissing` / `docNotFound` 三族
     用例整族退场；`docMissing` 的两条文案并为一条（`doc_facts` 三态 → `doc_present` 布尔）。 -->

#### 待测功能

- `category_of()`: 同签名；`doc` 只剩 `docMissing`，七组集合不变（成员收缩、无空组）
- `is_pure_ref_alias()`: 私有 → `pub(crate)`；**三处消费者共用同一份判据、MUST NOT 各写一份**——`check` 的 `declared_types`（`apiMissing` 类型面排除）、`contract::Contract::from_schema` 的 `types` / `reexports` 分流、`dto` 的 `interface.types` / `reexports` 分派
- 文件级被测面（C6 / 修改文件表第 6 行）: `api = none` 支 2 步（`apiUnexpected` → `docMissing`）、`api = declared` 支 4 步（`docMissing` → `apiEntryMissing` → `apiMissing` → 一致性三条）；三值删除是事实源消失而非放宽；「接口未写」（schema 不在盘）零判据

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 求值链 · `none` 支 | 正向 | `api = none` + schema 在盘 ⇒ 恰一条 `apiUnexpected`（② 短路，`docMissing` 不再求）；schema 缺 + 派生 md 缺 ⇒ 恰一条 `docMissing`；两者皆缺 ⇒ 恰一条 `docMissing`——`none支两步` | 矩阵迭代（3 形态） |
| 求值链 · `declared` 支 | 正向 | 派生 md 缺 ⇒ 恰一条 `docMissing` 且 ⑤⑥ 不再求（零 `apiMissing` / `apiSignatureMismatch`）；md 在盘 + schema 在盘 ⇒ 依次求 `apiEntryMissing` → `apiMissing` → 一致性；md 在盘 + schema 缺 ⇒ `apiEntryMissing` 后零声明集（⑤⑥ 无对象）——`declared支四步` | 矩阵迭代（3 形态） |
| 求值链 · 三值零产出 | 异常 | 历史语料（fence 与散文表不符 / md 有结构无 fence / `doc` 指不到盘）⇒ `apiReexportDrift` / `apiDocMissing` / `docNotFound` 三 kind 零产出；源码零 `reexport_drift` / `doc_not_found` / `api_doc_missing` 三函数——`三值随事实源消失` | 矩阵迭代（3 形态） |
| 求值链 · AC-13 反向锁 | 边界 | 构造「派生 md 在盘、schema 缺、`path` 非空且在盘、入口已解析」⇒ `check` 返回零条目（`docMissing` 不成立、`apiMissing` 无声明集）——`接口未写不产判据` | 单点 |
| 求值链 · 短路与互斥 | 边界 | 同一元素同时具备 `apiUnexpected` 与 `docs` 面事实 ⇒ 只出 `apiUnexpected`（① 命中即不再求 ②）；空元素表 / 空 snapshot ⇒ 零条目；3000 判据规模不 panic——`短路互斥与规模` | 矩阵迭代（3 形态） |
| `category_of()` · 七组映射 | 正向 | 十值逐值映射：`doc` 组恰含 `docMissing` 一值、`entry` / `missing` / `signature` / `constructor` / `ref` / `nesting` 其余六组各非空；七组集合 == 设计列出的七值——`成员收缩分组不变` | 矩阵迭代（10 形态） |
| `category_of()` · 穷尽与非法 | 边界 | `ArchiApiFindingKind::ALL` 十值逐值 `category_of` 返回非空且 ∈ 七组；`ALL` 长度 10（与 `report.rs` 同锚）——`十值全可打标` | 矩阵迭代（10 形态） |
| `is_pure_ref_alias()` · 判据 | 边界 | 判据只问「`$ref` 是唯一键」（`as_object` ∧ `contains_key("$ref")` ∧ `len() == 1`），**不看目标形态、不看值类型**——`true` 三臂：跨元素 `$ref`（带文件部分）、本地 `#/$defs/<名>`、单键但值非字符串（`{"$ref": null}`）；`false` 三臂：带兄弟键（`description` / `x-class` / `x-common`）、无 `$ref` 的对象（`{}` / 带 `properties` 的定义）、非对象（字符串 / 数组 / `null`）——`纯alias判据单一` | 矩阵迭代（6 形态） |
| `is_pure_ref_alias()` · 单点 | 边界 | **三消费者一份判据**（源码锚，三份语料拼接计数，沿本文件既有 `include_str!("../drift/report.rs")` 跨文件先例）：`include_str!("check.rs")` + `include_str!("contract.rs")` + `include_str!("../dto.rs")` 三份语料里 `matches("is_pure_ref_alias(")` 合计恰 **4** = 定义 1（`fn is_pure_ref_alias` 只在 `check.rs`）+ 调用 3（`check.rs` 的 `declared_types` 一条 `!is_pure_ref_alias(`、`contract.rs` 的 `Contract::from_schema` 一条 `::is_pure_ref_alias(`、`dto.rs` 的 `interface_asset` 一条 `::is_pure_ref_alias(`；带括号计数以避开 `[`is_pure_ref_alias`]` 形态的 doc 链接）；零第二份 alias 判定（`fn is_pure_ref_alias` 与 `len() == 1` 在 `contract.rs` / `dto.rs` 各恒 0，只在 `check.rs` 各 1）——`三消费者一份判据` | 单点 |
| `occurrences` 聚合 | 既有（维持） | 同名 kind 跨元素聚合的 `occurrences` 形状与稳定序、`Option` 字段形态、空输入——既有用例保留——`报告形状零改` | 既有（维持） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | 手写 schema / 手写 `Contract`（经 `Contract::from_schema`）+ 真实 `extract` 符号集 + 真实 tempdir（`entry_state` / `contract_path` / `schema_path` 的存在性判定即被测语义） | 全节 |
| 旧装置 | `contract::parse` + `schema::build_schema` 产出夹具 → 改手写 schema 值（同批迁移）；`build_schema` 相关断言整体退场 | 全节 |

### packages/desktop/src-tauri/crates/core/archi/src/edit/warnings.rs -> packages/desktop/src-tauri/crates/core/archi/src/edit/warnings_test.rs

<!-- 挂 AC-12（五条 conditions 逐条 / 只改 doc 三次全空 / `DocNotFound` 零出现）、AC-13（单因单义）。
     条目取自 design「公共函数 / API」子表的 `upsert_warnings` / `delete_warnings` 两行与
     「内部接线面」的 `path_on_disk` / `schema_on_disk` / `api_induced` 三件（私有，经公开入口断言）。
     必改断言：`declared缺契约报docMissing` / `none未声明doc报docMissing` / `doc指不到盘报docNotFound`
     三族整体退场（`doc` 零诱发）；`none有契约报apiUnexpected` 的条件从「契约 md 在盘」改为
     「schema 在盘」；`created放行全集` 的期望条数由 6 条改为 5 条。 -->

#### 待测功能

- `upsert_warnings()`: 五条 conditions（`noPath` / `pathNotFound` / `docMissing`（仅 `created`）/ `apiUnexpected`）+ 降噪过滤；`kind` / `severity` 全取既有枚举单点
- `delete_warnings()`: 删除后 `apiUnexpected`（schema 在盘）一条；`doc` 零参与
- `EditWarning`（类型）: `{ kind, severity, message }` 不变——kind / severity 取既有 `as_str()` / `severity()` 返回值（零新词表）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `upsert_warnings()` · 第 1 条 `noPath` | 既有（维持） | `package` 且 `path` 空 + `changed ∋ path` ⇒ 一条 `noPath` / `warn`；`composite` 零条——`纯分组容器外的空path报noPath` | 既有（维持） |
| `upsert_warnings()` · 第 2 条 `pathNotFound` | 既有（维持） | 缺席项逐条报 `pathNotFound`；全部在盘 ⇒ 零条——`path缺席逐条报` | 既有（维持） |
| `upsert_warnings()` · 第 3 条 `docMissing`（仅 `created`） | 正向 | `created = true` + 派生 `<FQN>.md` 缺 ⇒ 一条 `docMissing` / `warn`（与 `api` 无关：`declared` / `none` 两臂都出）；`created = true` + md 在盘 ⇒ 零条——`新建缺派生md报一条` | 矩阵迭代（3 形态） |
| `upsert_warnings()` · 第 3 条 · 既有元素 upsert | 异常 | `created = false`（既有元素 upsert）+ 派生 md 缺 + `changed ∋ api` ⇒ `docMissing` 零条（写面只由 `created` 诱发，避免既有元素噪声复读）——`既有元素不复读文档债` | 单点 |
| `upsert_warnings()` · 第 4 条 `apiUnexpected` | 正向 | `api = none` ∧ schema 在盘 + `changed ∋ api` ⇒ 一条 `apiUnexpected` / `warn`，尾句给「删 schema / 改 declared」两条出路；`declared` ∧ schema 在盘 ⇒ 零条；`none` ∧ schema 缺（md 在盘 ⇒ 旧形态② 语料）⇒ 零条——`留白不该有接口文件` | 矩阵迭代（3 形态） |
| `upsert_warnings()` · `doc` 零诱发 | 边界 | 只改 `doc` 三次（写值 / 清键 / 指向不存在文件），事实面同时具备可触发 `docMissing` / `apiUnexpected` 的条件 ⇒ `warnings == []`（新增降噪不变式一条）——`只改doc恒空` | 矩阵迭代（3 形态） |
| `upsert_warnings()` · `DocNotFound` 零出现 | 异常 | `api = none` + `doc` 指不到盘 + `changed ∋ doc` ⇒ 零条且 `kind = "docNotFound"` 零命中；`include_str!("warnings.rs")` 零 `DocNotFound`——`写面不再有该值` | 单点 |
| `upsert_warnings()` · 降噪与归因 | 既有（维持） | `changed` 只含 `summary` / `gate` / `metadata` ⇒ 恒空；`created = true` 视为全字段；两类 warning 归因互不串门（`path` 面 / `api` 面）——既有四条断言保留 | 既有（维持） |
| `upsert_warnings()` / `EditWarning`（类型） · 单点取值 | 边界 | `kind` / `severity` 逐值 ∈ 两枚举 `as_str()` / `severity()`；`path_on_disk` 走 `checked_root` + `root.join`（源码锚零 `doc_path(` / 零第二份归一）；`api_induced` 恰两条（`none` ∧ schema 在盘 / `declared` 零条目）——`复用既有单点` | 单点 |
| `delete_warnings()` · 删除后越界接口 | 异常 | 删除元素后其 `<FQN>.schema.json` 仍在盘 ⇒ 一条 `apiUnexpected`；schema 不在盘 ⇒ 零条——`删除后接口孤儿` | 矩阵迭代（2 形态） |
| 组合：`edit::upsert_element` 既有调用点 → `upsert_warnings` | 正向 | 既有元素 upsert 且派生 md 缺 ⇒ `WriteOutcome.warnings` 为空（AC-13 的写面半边）；`created = true` 同条件 ⇒ 恰一条 `docMissing`（调用点签名不变，`changed` / `created` 由 `edit/mod.rs` 求值）——`写面与判据面同义` | 矩阵迭代（2 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | 真实 tempdir（派生 md / schema / `path` 目录真实落盘）+ 真实 `api::paths`；存在性判定即被测语义 | 全节 |
| `Element` 取值 | 经真实 TOML 解析合成源取得（既有 `element_of` helper），不手搓残形 | 全节 |

### packages/desktop/src-tauri/crates/core/archi/src/model.rs -> packages/desktop/src-tauri/crates/core/archi/src/model_test.rs

<!-- 挂 AC-9 的字段语义面（`api` = 接口承诺开关、`doc` = 约定 + 零判据读者）。
     design「公共函数 / API」子表**未声明本文件的公共 API 变更**（纯注释 / 字段文档改写，
     零逻辑行、零类型行，见 C9）；本节的被测面来自「修改文件」表第 8 行。故按「文件级被测面」
     登记，MUST NOT 据源码导出声明虚构函数条目。 -->

#### 待测功能

- 文件级被测面（C9 / 修改文件表第 8 行）: `Element.api` 注释改为「接口承诺开关」、`Element.doc` 注释改为「约定：只有 `api = none` 会写；零判据读者」；字段集与八字段顺序零改动

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 字段注释语义 | 正向 | `include_str!("model.rs")` 的 `api` 字段文档含「接口承诺」且零「入口文档载体」；`doc` 字段文档含「约定」与「零判据读者」且保留「单值 / repo 相对 POSIX」——`注释改述不新增判据` | 矩阵迭代（3 锚） |
| 字段面零改动 | 边界 | 既有八字段集合与顺序断言维持（`fqn` / `kind` / `gate` / `api` / `path` / `summary` / `metadata` / `entry` + `doc` / `mark` 的既有形态）；`ApiStatus` 仍二值封闭、`doc` 仍 `Option<String>`——`注释改写零类型行` | 既有（维持） |
| 解析面零回归 | 异常 | `api` 词汇外值 / `doc` 非字符串 / 未知字段 ⇒ 既有 `deny_unknown_fields` 诊断逐字不变；`api = none` + `doc` 写值 ⇒ 零诊断（约定非机械判据，Q10）——`解析不新增禁令` | 矩阵迭代（3 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | 真实 TOML 解析（`parse::toml`）与 `include_str!` 源码锚 | 全节 |

### packages/desktop/src-tauri/crates/core/archi/src/drift/report.rs -> packages/desktop/src-tauri/crates/core/archi/src/drift/report_test.rs

<!-- 挂 AC-8（十值三方对拍 / `error` 恰两项 / `category` 七组）、AC-9（档位蕴含纳管口径）、
     AC-3 的联合三值离场。
     条目取自 design「类型定义」的 `ArchiApiFindingKind`（十三 → 十值）一行与
     「修改文件」表第 9 行；`ArchiApiFinding` / `ApiFindingSeverity` / `REPORT_VERSION`（3）
     逐字不变（类型表「不变」行）。
     必改断言：`ALL: [Self; 13]` → `[Self; 10]`；`severity` 的 `error` 三 → 两项；
     `as_str` 十三词 → 十词。 -->

#### 待测功能

- `ArchiApiFindingKind`（类型）: 十三 → 十值（删 `ApiReexportDrift` / `DocNotFound` / `ApiDocMissing`）
- `ArchiApiFindingKind::ALL`: `[Self; 10]`——长度即红灯
- `ArchiApiFindingKind::as_str()`: 十词（与变体逐位同源）
- `ArchiApiFindingKind::severity()`: `error` 恰 `ApiMissing` / `ApiSignatureMismatch`；`off` 恰 `ApiUndeclared`；其余七 `warn`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ALL` · 长度与成员 | 正向 | `ArchiApiFindingKind::ALL.len() == 10`；十值 `as_str` 逐位与 `ALL` 同源（索引 i 的变体 ↔ 第 i 个词）；零重复——`长度即红灯` | 单点 |
| 三值离场 | 异常 | 十词中 `apiReexportDrift` / `docNotFound` / `apiDocMissing` 零命中；源码零三变体；`include_str!` 零三字面量——`事实源消失的删除` | 矩阵迭代（3 形态） |
| `severity()` · 十值档位 | 正向 | `error` 恰两项（`apiMissing` / `apiSignatureMismatch`）且其余八项非 `error`；`off` 恰一项（`apiUndeclared`）；`warn` 恰七项——`error两项off一项` | 矩阵迭代（10 形态） |
| `severity()` · 非法/穷尽 | 边界 | 十值逐值 ∈ `ApiFindingSeverity::ALL`（三值）；`ApiFindingSeverity::ALL` 长度仍 3；三值 `as_str` 逐字不变——`档位词汇零扩` | 矩阵迭代（10 形态） |
| `category` 七组 | 边界 | 十值经 `api::check::category_of` 映射后集合 == 七组（`doc` / `entry` / `missing` / `signature` / `constructor` / `ref` / `nesting`）且无空组——`分组集合不变` | 单点 |
| 不变量 · `status` ↔ `passed` | 边界 | `status == "errors" ⟺ passed == false`：构造 `error` 档判据 ⇒ `errors` + `passed = false`；全 `warn` ⇒ `clean` + `passed = true`（档位蕴含由 `severity()` 单点保证）——`不变量成立于语料` | 矩阵迭代（2 形态） |
| 报告形状零改 | 既有（维持） | `ArchiApiFinding` 字段面 / `occurrences` / `ApiFindingSeverity` / `ArchiCheckStatus` / `REPORT_VERSION == 3` 逐字不变——既有断言保留——`形状零改` | 既有（维持） |
| 组合：联合类型出线面 | 正向 | `packages/desktop/src-tauri/src/bindings/mod_test.rs`：`bindings.ts` 的 `ArchiApiFindingKind` 联合恰十成员、三值零命中；`bindings.zod.ts` 的 enum 同批收缩——`出线联合随词汇收缩` | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | 纯枚举与纯函数；`category_of` 经 `api::check` 真实调用（单一打标处） | 全节 |
| 生成物 | `bindings/mod_test.rs` 的真实 `export_bindings()` + 既有 OS 文件锁（跨进程串行），不 mock | 组合行 |

### packages/desktop/src-tauri/crates/core/archi/src/dto.rs -> packages/desktop/src-tauri/crates/core/archi/src/dto_test.rs

<!-- 挂 AC-6（三资产各自 present / `reading` / `interface` 键名摘要）、AC-3（四新 DTO 出线）、
     AC-15 的检索投影面、AC-9 的 `ArchiApiRef` 三路径独立。
     条目取自 design「公共函数 / API」的 `module_detail`（修改）、`search`（修改，行内注
     「实现落 `search.rs` ⇒ 语料用例挂 `search.rs` 节」）两行与「类型定义」的
     `ArchiAssetSet` / `ArchiTopologyAsset` / `ArchiInterfaceAsset` / `ArchiDocAsset`
     （新）/ `ArchiModuleDetail`（改）/ `ArchiApiRef`（仅 doc）六行。
     必改断言：`module_detail并列api字段` 的键序八 → 十（`assets` / `reading` 追加在
     `fileHash` 之后）；`archi_api_ref为none时三路径全空` 必改为「各自按在盘事实」；
     `module_detail投影随md存在性切换` 必改为「随 schema 存在性切换」。 -->

#### 待测功能

- `module_detail()`: 同签名；出参增 `assets`（`topology` / `interface` / `doc` 三段各自 `present`）+ `reading`（静态常量）；`doc` 改为原样投影（去 `api = none` 过滤，D-23）；既有八字段名 / 类型 / 顺序不变
- `ArchiAssetSet` / `ArchiTopologyAsset` / `ArchiInterfaceAsset` / `ArchiDocAsset`（类型）: 四新出线类型；`in_degree` 出线 `in`、`abstract_note` 出线 `abstract`（D-7）
- `ArchiModuleDetail`（类型）: 追加 `assets` + `reading`（声明序在 `file_hash` 之后）
- `ArchiApiRef`（类型）: 四字段零改名零改型；`status` = 接口文件在案、`contractPath` = 文档路径（仅 JSDoc）
- 检索投影（`dto::search`，行声明在本文件）: `ArchiSearchResult.elements` 排序与 `doc` 语料串透传；语料规则本体见 `search.rs` 节

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `module_detail()` · 三资产各自 `present` | 正向 | 四态夹具：只有 schema ⇒ `interface.present = true`、`doc.present = false`；只有 md ⇒ 反之；两者都在 ⇒ 双 true；都没有 ⇒ 双 false（`topology.present` 恒 true——元素必在模型里）——`三段present互不牵连` | 矩阵迭代（4 形态） |
| `module_detail()` · `interface` 键名摘要 | 正向 | `ops` = 带 `x-function` 的 `properties` 键（字典序）；`types` = 非纯 alias `$defs` 键；`reexports` = 纯 alias `$defs` 键；空 schema ⇒ 三数组空 + `abstract` = 接口面留白文案——`只给键名不给正文` | 矩阵迭代（2 形态） |
| `module_detail()` · `interface.abstract` 六形态 | 异常 | 优先级 = 不可解析 > 含透传 > 空 > 只有操作 > 只有类型 > 两者都有；坏 schema ⇒ `present = true`、`bytes` 照实、三数组空、`abstract` 提示「用 `archi_api` 的 `check` 看诊断」（D-9 不 `Err`）——`坏文件只降级摘要面` | 矩阵迭代（6 形态） |
| `module_detail()` · `doc` 段名与体量 | 正向 | `headings` = 按序 `##` 段名（`###` / 代码块内 `##` 不进）；`lines` / `bytes` 与盘上一致；零正文；空 md ⇒ `headings` 空、`bytes` 取实值——`文档只给结构` | 矩阵迭代（3 形态） |
| `module_detail()` · `topology` | 正向 | `file` / `line` 与元素声明一致；`out` / `in` = 邻域出入边数；0/0、仅出、仅入三形态逐值——`拓扑零额外读盘` | 矩阵迭代（3 形态） |
| `module_detail()` · `reading` | 边界 | 静态常量：同元素两次调用逐字相等、不同元素逐字相等（零变量插值）；含「read」与「不必整读」两语义、含「已足够」判断句——`静态指引零插值` | 单点 |
| `module_detail()` · `doc` 原样投影（D-23） | 正向 | `api = declared` + `doc` 有值 ⇒ `detail.doc` == 元素原文（旧「declared 恒 null」断言必改）；`api = none` + `doc` 无值 ⇒ `null`；`api = none` + `doc` 有值 ⇒ 原文——`投影零改写` | 矩阵迭代（3 形态） |
| `ArchiModuleDetail`（类型） · 键序与既有字段 | 边界 | 十字段键序 == `[element, dependsOn, dependedBy, children, doc, api, resolvedEntry, fileHash, assets, reading]`；既有八字段类型逐字不变——`新字段追加表尾` | 单点 |
| `module_detail()` · 异常两臂 | 异常 | 元素缺席 ⇒ `Err` 文案含元素总数提示；模型结构错 ⇒ `Err` 附诊断清单（既有两臂维持）——`定位面异常不变` | 既有（维持） |
| `ArchiApiRef` · 三路径独立 | 正向 | 只有 schema ⇒ `schemaPath` 非空 + 其余 `null`；只有 md ⇒ `contractPath` 非空；三者在盘 ⇒ 三条全出；四字段名 / 顺序零改——`三路径各按自身事实` | 矩阵迭代（3 形态） |
| `ArchiApiRef` · `status` 与边界 | 异常 | `declared` ∧ schema 缺 ⇒ `status = "none"`；`api = none` ∧ schema 在盘 ⇒ `"none"` 但 `schemaPath` 非空（越界态仍报事实）；超长 / 特殊字符路径串原样出线——`status换锚而路径照实` | 矩阵迭代（3 形态） |
| 四新类型 · 线形 | 边界 | `serde(rename_all = "camelCase")` 生效：`ArchiTopologyAsset` 键为 `file/line/out/in`；`ArchiInterfaceAsset` 含 `abstract` 键；`ArchiDocAsset` 四键；`ArchiAssetSet` 三键 `topology/interface/doc`——`in与abstract的出线名` | 矩阵迭代（4 形态） |
| 检索投影（`dto::search`） | 边界 | `ArchiSearchResult.elements` 按 FQN 字典序、`doc` 原样透传 `SearchOutcome.doc`（排序 / 去重由 `search.rs` 负责）；`doc` 键注释改「资产路径（文档 / 接口）」——`投影零加工` | 单点 |
| 组合：DTO 载荷 → 前端夹具编译面 | 正向 | `.../views/architecture/hooks/use-archi-module.test.ts` 的 `detailOf()` 补 `assets`（三资产三态各一例）与 `reading`；`.../components/architecture-panel.test.tsx` 的 `detailOf()` 同批补；TS 类型检查通过、面板断言零改（零功能改动，D-22）——`载荷外溢只在夹具` | 单点 |
| 组合：四新类型 → 出线清单 | 正向 | `packages/desktop/src-tauri/src/bindings/mod_test.rs`：`ARCHI_DTO_TYPES` 由 22 → 26（按字典序插入四名）、`DTO_TYPES.len()` 111 → 115、四名在 `bindings.ts` 各有 `export type`；`COMMAND_NAMES` / `COMMAND_WRAPPERS` 零变化——`四类型入两份清单` | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | 真实 tempdir + 手写 schema / md 夹具 + 真实 `paths::{contract_path, schema_path}`；三资产 `present` 即存在性判定，替身会抹掉语义 | 全节 |
| 哈希 | `drift::file_hash` 真实调用（既有） | `module_detail` 臂 |
| 前端 `invoke` | 不属本节——前端夹具随动只补字段值，IPC 面在 `model-info.test.tsx` / `architecture-panel.test.tsx` 既有 `vi.mock('@tauri-apps/api/core')` 装置内 | 组合行 |

### packages/desktop/src-tauri/crates/core/archi/src/search.rs -> packages/desktop/src-tauri/crates/core/archi/src/search_test.rs

<!-- 挂 AC-15（语料换面全表）。
     design「公共函数 / API」把 `search` 行记在 `dto.rs`，但同格说明「实现落 `search.rs`」
     （D-21：`dto::search` 只是投影）⇒ 语料 / 分词 / scope 用例挂本文件（实现单点，也是链路
     发起方）。design「修改文件」表第 11 行同批登记语料两规则与 `carrier_path` 调用删除。
     故本节按该行的实现落位登记，MUST NOT 另造 `search.rs` 之外的条目。 -->

#### 待测功能

- `search()`（`dto.rs` 表行的实现落位，D-21）: 语料两条固定规则——`path` 非空元素的在盘 `<FQN>.md` ∪ 在盘 `<FQN>.schema.json`，与 `api` 状态 / `doc` 字段无关
- `SearchOutcome`（类型）: `doc` 语义改「资产路径（文档 / 接口）」；字段名 / 类型不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `search()` · 四态 × 两值 `api` | 正向 | 逐格：只有 md ⇒ 文档命中、接口不命中；只有 schema ⇒ 接口命中；两者都在 ⇒ 两条都命中（按路径去重）；都没有 ⇒ 零命中；`api = none` 的「只有文档」与 `declared` 的「只有 schema」两格是语义锚——`命中只由在盘事实决定` | 矩阵迭代（8 格） |
| `search()` · `doc` 字段零参与 | 边界 | 同元素 `doc` 写值 / 不写值两例 ⇒ 命中集逐元素相等（`doc` 不进语料，P9）——`doc字段退出语料` | 矩阵迭代（2 形态） |
| `search()` · `scope` 三值 | 正向 | `elements` ⇒ `doc` 空数组；`doc` ⇒ `element_fqns` 空；`all` ⇒ 两者都填（既有语义维持）——`两半边各自为空数组` | 矩阵迭代（3 形态） |
| `search()` · 参数异常 | 异常 | 空 query / 纯空白 query ⇒ `Err`（不返回全量）；`SearchScope` 非法值 `"docs"` ⇒ `Err` 并列三合法值——`参数面逐字不变` | 矩阵迭代（3 形态） |
| `search()` · 语料排除 | 异常 | `decisions/**` / `openspec/specs/**` / 元素 `path` 目录内散落的 `.md`（非派生 `<FQN>.md`）⇒ 零命中；`decisions` 与 `specs` 语义由 `path` 面天然排除（不新增过滤规则）——`非派生资产不参与` | 矩阵迭代（3 形态） |
| `search()` / `SearchOutcome`（类型） · 派生单点 | 边界 | 源码 `include_str!("search.rs")` 零 `carrier_path(` / `doc_path(`；`contract_path(` 与 `schema_path(` 各恰一处；零手拼 `format!("{}.md"` ——`零第二份载体派生` | 单点 |
| `search()` · 既有面零回归 | 既有（维持） | 驼峰分词 / 多词 AND / 大小写等价 / 稳定序 / 空模型 / 超长 query / 仅正文命中不匹配——既有用例逐条保留（语料换面不改匹配语义）——`匹配语义零改` | 既有（维持） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | 真实 tempdir（派生 md / schema 真实落盘）+ 真实 `ModelGraph`（既有 `graph_of` helper） | 全节 |
| 文件系统 | 不 mock——`is_file` 是语料判据本体 | 全节 |

### packages/desktop/src-tauri/crates/core/archi/src/lib.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/mod_test.rs

<!-- 挂 AC-7（两出口同源经出口面）与 AC-15 的「零第二份派生」出口纪律。
     design「公共函数 / API」子表**未声明本文件的函数**；本节的被测面来自「修改文件」表第 12 行
     （出口面追加 `BootstrapDrafts` / `validate_schema` / `schema_fqn` / `is_fqn` / `is_identifier`）。
     本 crate 无 `lib_test.rs` 惯例（`dto_test.rs` 模块头 doc 明写「本 crate 无 lib_test 惯例」），
     出口面锚落 `api/mod_test.rs`（`include_str!("../lib.rs")` 解析到 `src/lib.rs`；该文件已有两处
     同式锚：`pub use api::display` 恰 1 次、`pub use api::{` 分组块在 crate 根），沿归档
     `archi-edit-engine-primitives` 先例。注意 `lib.rs` 是**逐名成组**形态，断言 MUST NOT
     写成「每名一条 `pub use` 行」。 -->

#### 待测功能

- 出口面（文件级被测面）: 五名（`BootstrapDrafts` / `validate_schema` / `schema_fqn` / `is_fqn` / `is_identifier`）进 crate 根的**逐名成组** `pub use api::{…}` 块（既有形态：分组块 + `pub use api::display` 单行，见 `api/mod_test.rs` 既有出口面锚）；四个新 DTO（`ArchiAssetSet` / `ArchiTopologyAsset` / `ArchiInterfaceAsset` / `ArchiDocAsset`）进既有 `pub use dto::{…}` 块；`DocFact` 零出线

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 出口面 · 五名在册 | 正向 | `include_str!("../lib.rs")` 的 `pub use api::{…}` 分组块内五名各恰一次（分组块形态不变，零拆成五行单名 `pub use`）；`pub use dto::{…}` 块内四新类型各恰一次——`出口面按分组块扩面` | 矩阵迭代（9 锚） |
| 出口面 · 退役名零出线 | 异常 | `DocFact` / `carrier_path` / `doc_path` / `build_schema` / `ContractSection` 零出线（零 `pub use`、零分组块成员）；零通配 `pub use api::*`（逐名出口纪律）；零同名重复——`退役名不出线` | 矩阵迭代（5 形态） |
| 出口面 · 可达性 | 正向 | 经 `archi::{BootstrapDrafts, validate_schema, schema_fqn, is_fqn, is_identifier}` 直调成功（编译期即判，零测试专用导出）——`五名可经crate根调用` | 单点 |
| 出口面 · 既有导出零回归 | 边界 | 既有分组块与单行 `pub use` 逐字保留（`pub use api::display` 单行恰 1、`pub use api::{…}` 块仍含既有成员）；`pub mod api;` 等 11 行模块声明零增删——`既有出口零回归` | 既有（维持） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | `include_str!` 编译期语料 + 直接调用（可达性由编译判定） | 全节 |

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools_test.rs

<!-- 挂 AC-4（三类坏输入 + 回执）、AC-5（写闸放行侧 + 与装载同源）。
     条目取自 design「公共函数 / API」的 `tools::execute`（不变）一行与「内部接线面」的
     `write` / `edit`（修改，增 `root: &Path`）/ `schema_target_fqn`（新，文件内私有）三行。
     射程只 `<archi_models_dir>/**/<FQN>.schema.json`；不属保护面、不改档位、不动沙箱。 -->

#### 待测功能

- `tools::execute()`: 对外签名零改动（内部把 `root` 传给 `write` / `edit`）
- `write()`: 增 `root: &Path`；落盘前若命中接口文件即 `validate_schema`，`Err` 零落盘（不建父目录）；成功追加「接口已校验」
- `edit()`: 增 `root: &Path`；校验对象是 `old → new` 替换后的**候选正文**
- `schema_target_fqn()`（内部）: 路径命中 `<root>/<models rel>/**` 且文件名为 `<FQN>.schema.json` ⇒ `Some(FQN)`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `write()` · 三类坏输入 | 异常 | ① 文本层重复键 ② `$id` 与文件名不符 ③ `properties` 与 `$defs` 撞名 ⇒ 各一条 `Err`（文案含文件名 + 期望值 + 出路），盘面字节零变化、父目录未被创建——`坏文本不可产出` | 矩阵迭代（3 形态） |
| `write()` · 合法接口文件 | 正向 | 合法 schema（op + 类型镜像）写入 ⇒ 成功、落盘字节 == 入参 `content`、回执含「接口已校验」——`合法即过闸并回执` | 单点 |
| `write()` · 边界形态 | 边界 | 空内容 / 1 MB 超长 / 已存在文件覆写 / 缺失多层父目录（合法内容）⇒ 过闸行为与既有写盘语义一致（空内容 `Err`、超长合法则落盘、覆写替换）——`写盘语义不变` | 矩阵迭代（4 形态） |
| `write()` · 非接口路径零校验 | 边界 | 同目录 `X.md` / `<models>` 外的 `X.schema.json` / `<root>` 内他处 `schema.json` ⇒ 原样落盘、零校验、回执零「接口已校验」——`射程只一项` | 矩阵迭代（3 形态） |
| `edit()` · 候选正文校验 | 异常 | 替换后正文非法（三形态同上）⇒ `Err` 且原文零变化；替换后正文合法 ⇒ 成功 + 「接口已校验」——`校验替换后正文` | 矩阵迭代（4 形态） |
| `edit()` · 既有错误面 | 既有（维持） | `old_string` 不存在 / 多处出现默认失败 / `replace_all` 全替换 / 中文与 emoji 目标串——既有断言逐条保留（本节只加候选正文闸）——`编辑语义零改` | 既有（维持） |
| `schema_target_fqn()` · 路径判定 | 边界 | `<models rel>/**/X.schema.json` ⇒ `Some("X")`；`../` 越根 / 大写后缀 `X.schema.JSON` / 主干非 FQN 四形态 ⇒ `None`；`<models rel>` 前缀本身不算命中（需在目录内）——`命中判定单点` | 矩阵迭代（5 形态） |
| 沙箱与档位面 | 异常 | `<archi_models>` 外路径写 ⇒ 既有沙箱链拒绝；`default` 档仍拒 `write`、`acceptEdits` 放行（既有）；写闸不参与 `ProtectRules`（源码锚零 `protect::` 调用）——`写闸非保护面` | 矩阵迭代（3 形态） |
| `execute()` · 签名与接线 | 边界 | `include_str!("tools.rs")` 的 `pub async fn execute(root: &Path, name: &str, input: &Value)` 逐字不变；`write(` / `edit(` 调用点各恰一处且传 `root`——`对外签名零改` | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | 真实 tempdir（`tempfile`，既有 `tempdir` / `root_of` helper）+ 真实 `validate_schema`；落盘字节与父目录存在性是判据本体 | 全节 |
| 读类工具 | 不参与本节（`read` / `grep` / `glob` / `ls` 既有用例族保留，仅作零回归对照） | 既有对照 |

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool_test.rs

<!-- 挂 AC-6（`assets` / `reading` 透传）、AC-7（双草稿与 `link` 可达性）。
     条目取自 design「公共函数 / API」的 `archi_api` / `archi_module` 一段（工具非 IPC，执行体即
     全部执行面）与「内部接线面」的 `link_payload`（新，文件内私有）一行、「修改文件」表第 14 行。
     必改断言：`archi_api_bootstrap出草稿` 的 `payload["draft"]` → `payload["schema"]` +
     `payload["doc"]`；`archi_api非法action与缺参` 的合法值文案二值 → 三值；
     `archi_module响应doc单值三组同源` 的「declared 出 null」臂随 D-23 改判。 -->

#### 待测功能

- `api_payload()`（`archi_api` 执行体）: 三动作 `check` / `bootstrap` / `link`；词汇外值 `is_error` 点名三值
- `link_payload()`（内部）: 两形态互斥（`{from, to, name}` / `{element, name}`）+ FQN / 标识符校验；纯计算零读盘零写盘
- `archi_module` 响应: `assets` / `reading` 由 DTO 单点透传（零拼装）；薄概览零携带

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `link_payload()` · 两形态 | 正向 | 跨元素 ⇒ `ref_relative(from, to) + to + ".schema.json#/$defs/<name>"`（前缀 == `paths::ref_relative` 输出，不比 spec 示例字面，D-15）；本模块 ⇒ `"#/$defs/<name>"`；`{from, to, name}` 与 `{element, name}` 同批给 ⇒ `Err`——`两形态互斥且前缀单点` | 矩阵迭代（3 形态） |
| `link_payload()` · 参数校验 | 异常 | `from` / `to` 非 FQN 形态；`name` 非标识符（中划线 / 空串 / emoji / 1000+ 字符 / 带点）；三参数缺任一 ⇒ `Err` 点名该参数——`校验复用path与contract单点` | 矩阵迭代（6 形态） |
| `link_payload()` · 零读盘零写盘 | 边界 | 无模型目录 / 元素不存在也能算出串；调用前后盘面快照逐字节相等——`纯计算` | 单点 |
| 组合：`link → write → check` 可达 | 正向 | `link` 出的跨元素串写进目标 schema 的 `$ref` ⇒ `validate_schema` 过闸、`archi_api` 的 `check` 判 `apiRefMissing` 零命中（AC-7 可达性；写盘经 `write` 工具，本节执 `execute`）——`算出的串必可达` | 单点 |
| `bootstrap` · 双草稿 | 正向 | 回 `{ schema, doc }` 两键（无 `draft` 键）；两段文本与 `archi::api::bootstrap` 的 `drafts.schema` / `drafts.doc` 逐字相等（两出口同源）；`doc` 零 `##` 段名——`双草稿单源` | 矩阵迭代（3 锚） |
| `api_payload()` · 三值词汇面 | 异常 | 未知值 / 大小写变形（`CHECK`）/ 缺失 / `null` ⇒ `Err` 并列三合法值；`bootstrap` 缺 `element` / 空白 `element` ⇒ `Err` 点名——`词汇外点名三值` | 矩阵迭代（6 形态） |
| `archi_module` · `assets` / `reading` 透传 | 正向 | 单元素响应与 `dto::module_detail` 的 `assets`（三段）/ `reading` 逐字段相等（零拼装）；薄概览（省略 `element`）零 `assets` / `reading` 键——`DTO单点投影` | 矩阵迭代（2 形态） |
| `archi_module` · `doc` 投影改判 | 边界 | `api = declared` + `doc` 有值 ⇒ 段内 `doc` == 元素原文（D-23）；`api = none` ⇒ 原文；键面零旧槽位（`docs` / `warnings` 仍零）——`doc照原样投影` | 矩阵迭代（2 形态） |
| `check` 动作与输出收口 | 既有（维持） | 两轴结构化摘要 / 3000 判据过 `cap_result` 截断留痕 / 结构诊断即 `Err` / 零写盘——既有断言保留；`byCategory` 随七组收缩——`既有面维持` | 既有（维持） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | 真实 `tempfile` 模型目录 + 手写 schema / md 夹具 + 真实 `archi::api::*`；`execute_json` / `api_root` 既有 helper 复用 | 全节 |
| 工具注册表 | 不 mock——`ArchiContext::execute` 走真实分发表（工具名 → 执行体即被测边界） | 全节 |

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount_test.rs

<!-- 挂 AC-16（definitions 逐名来源 + 档位零变化）、AC-6 / AC-7 的工具描述面。
     design「公共函数 / API」子表**未声明本文件的公共 API 变更**（三处描述文本 + `archi_api` 参数
     面）；被测面来自「修改文件」表第 15 行与 C12。故按「文件级被测面」登记。
     既有两条禁令保持绿：零字段长表、零项目专用命令。 -->

#### 待测功能

- 文件级被测面（C12 / 修改文件表第 15 行）: `archi_module_definition()` 五句（三资产一次给全 / 接口唯一凭据 / 文档整读 / `resolvedEntry` + `fileHash` / 两路径语义）；`archi_api_definition()` 四句 + `action` enum 三值 + `from` / `to` / `name` 三参数；`archi_scaffold_definition()` 一句（接口草稿指 `archi_api` 的 `bootstrap`）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `archi_module` 描述面 | 正向 | 五句逐句在场：三资产一次给全（含「理解模块先调本工具」）/ 接口唯一凭据 / 文档整读（「接口不必整读」）/ `resolvedEntry` + `fileHash` / 两路径语义（`contractPath` = 文档）；零 `api = none 才取 doc` 的旧措辞——`描述随三资产改写` | 矩阵迭代（5 锚） |
| `archi_api` 描述与参数面 | 正向 | 描述含三动作与「勿手拼 `$ref`」；`action` enum == `["check", "bootstrap", "link"]`；`parameters.properties` 含 `from` / `to` / `name`（类型 string）；`required == ["action"]`——`三动作与三参数` | 矩阵迭代（4 锚） |
| `archi_scaffold` 描述指路 | 正向 | 描述含「接口草稿用 `archi_api` 的 `bootstrap`」；字段词典正文（kind / path / gate / api 各行）逐字不变——`词典正文零改` | 单点 |
| 工具名 / 档位 / exec 归属 | 既有（维持） | `archi_api` 仍 `ToolTier::Read` + `ToolExec::Archi`；registry 十八项名字唯一且有序、前七恰内建；`READONLY_TOOLS` 三工具名零变——`扩面不扩写权` | 既有（维持） |
| 描述面边界 | 边界 | `archi_api` 的 `action` enum 序固定为 `check → bootstrap → link`；`parameters.required` 仍只 `["action"]`；三处描述的本变更增量体量在量级内（零字段长表，R8 的约 1.8 KB 上限）——`描述面不膨胀` | 矩阵迭代（3 形态） |
| 两条禁令 | 异常 | `archi_*` 定义面零字段长表（零 `\n-` 列表 / 零值域长行）、零项目专用命令字面（`pnpm …` / `cargo …`）——既有两条禁令保持绿——`描述面零长表` | 既有（维持） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | 纯常量断言（`ToolDefinition` 由 `registry` / `lookup` 真实取） | 全节 |

### packages/desktop/src-tauri/crates/infra/agent/src/compose.rs -> packages/desktop/src-tauri/crates/infra/agent/src/compose_test.rs

<!-- 挂 AC-10（change 臂 preamble 生产条件）、AC-16（两格下发 + 同集不变式）。
     design「公共函数 / API」子表**未声明本文件的公共 API 变更**（`archi_matrix` 与
     `preamble` 生产条件均为私有装配面）；被测面来自「修改文件」表第 16 行与 C13 / D-16。
     必改断言：`注入矩阵八行逐行命中` 的两行名单各 +1 名（proposal 六名 / dev-design 七名），
     且序按 spec 列出的行序（`archi_api` 在 `search` 之后）。 -->

#### 待测功能

- 文件级被测面（C13 / D-16）: `archi_matrix` 两格补 `ARCHI_API_TOOL` 并对齐 spec 行序；change 臂 `preamble` 生产条件 = `model_write_lock(provenance).is_none()`（单一判据，与两写工具下发同集）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 两格补 `archi_api` | 正向 | `(proposal, executor)` 名单 == `[module, search, api, scaffold, element, relation]`（六名）；`(dev-design, executor)` == `[module, search, api, drift, scaffold, element, relation]`（七名）；definitions 与 `RunStarted.tools` 逐名一致且都含 `archi_api`——`两格名单与序` | 矩阵迭代（2 格） |
| 其余格零下发 | 异常 | `(dev-design, evaluator)` / `(test-design, executor)` / `(code-review, evaluator)` / `(acceptance, evaluator)` / 矩阵外相位 ⇒ 名单不含 `archi_api`；`archi_of()` 与之逐格相等——`扩面不外溢` | 矩阵迭代（5 形态） |
| 同集不变式 | 边界 | 「含 `archi_element`」⟺ `model_write_lock(provenance) == None` 逐格仍相等（8 行）；`archi_api` 档位仍 `Read`；`READONLY_TOOLS` 三工具名零变——`扩面不扩写权` | 单点 |
| change 臂 `preamble` 生产 | 正向 | 两写权相位（`proposal/executor` / `dev-design/executor`）`preamble.is_some()` 且内容 == `archi_knowledge::knowledge()`（同一份知识）；`implement/executor` 与三 evaluator 相位 ⇒ `None`——`写权相位带知识` | 矩阵迭代（5 形态） |
| `preamble` 与写工具同集 | 边界 | 逐格：`preamble.is_some() ⟺ model_write_lock(provenance).is_none()`（8 行 + `archi` 来源臂）——`单一判据` | 矩阵迭代（9 格） |
| 降级语义 | 异常 | 知识拼装失败 ⇒ `preamble = None` 而工具面 / `before_done` / `ask` / `finish` 位不变（「尽力增强」既有语义）——`失败只丢知识段` | 单点 |
| archi 臂与 config 臂零回归 | 既有（维持） | `source = "archi"` 臂七工具 + 知识 preamble 逐字不变；config 臂工具面与协议知识逐字不变——既有断言保留——`他臂零触碰` | 既有（维持） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 既有装置 | `DualEnv` / 真实 `WorkspaceStores`（tempdir 数据根）/ 既有 provider 播种 helper 沿用，不新增 | 全节 |
| 知识函数 | 不 mock——`archi_knowledge::knowledge()` 是装配单点（替身会让「同一份知识」不可见） | `preamble` 臂 |
| 会话 provenance | 字面量构造（既有 `ctx()` / 私有 `provenance` helper） | 矩阵与同集臂 |

### packages/desktop/src-tauri/crates/infra/agent/src/archi-knowledge.md -> packages/desktop/src-tauri/crates/infra/agent/src/archi_knowledge_test.rs

<!-- 挂 AC-10 的第一半（五段骨架 / 体量新锚 / 零围栏零诊断词）。
     条目取自 design「修改文件」表第 17 行与 D-17 / 附录 A（草案实测 1778 B）；「公共函数 / API」
     子表无本文件行（静态资产）⇒ 按「文件级被测面」登记。
     必改断言：既有四段断言 → 五段；双门限 1500 / 1700 → 1800 / 2000 B（实测后冻结，门限不抬）。 -->

#### 待测功能

- 文件级被测面（静态资产）: 五段（三资产分工 / 设计顺序 / 三条不可越界 / 工具面 / 留给判断 + 兼容一句）；骨架 ≤ 1800 B、全文 ≤ 2000 B（`lf_bytes(trim_end)` 口径，实测冻结）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 五段与兼容句 | 正向 | 五段标题逐段在场；三资产分工段含「schema（唯一凭据）」与「md 答理由」；工具面段含读五写二与 `archi_api` 三动作名；兼容句含「历史 fence 读面不读」与「清理中」——`五段逐句在场` | 矩阵迭代（6 锚） |
| 体量双门限 | 边界 | 骨架 ≤ 1800 B、全文 = 骨架 + 派生段 ≤ 2000 B；两实测值同批冻结并写入断言文案（D-17）——`体量锚重定` | 单点 |
| 语法教学零残留 | 异常 | 零 ```` ``` ```` 围栏 / 零值域行（`kind =` / `gate =` 形态）/ 零结构诊断词（`invalidValue` / `unknownField` 等）/ 零 `<>` 占位符 / 零目录字面量（`.dev-team` 与 `openspec`）——`只给纪律不给词典` | 矩阵迭代（5 形态） |
| 旧立场零残留 | 异常 | 「契约 md 是唯一真相」/「四类生成物」/「api-* fence」旧断言句零命中（`stale_stances()` 扩面）；「schema 是唯一凭据」在场——`立场换面` | 矩阵迭代（3 形态） |
| 工具面指路真实 | 边界 | 指路工具名 ⊆ `mount` 工具名常量并集（既有 `指路不指向不存在工具` 用例扩面到 `link` / `bootstrap` / `check` 三动作名）——`指路不指向空` | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | `include_str!("archi-knowledge.md")` 编译期语料 + `mount` 常量真实引用 | 全节 |

### packages/desktop/src-tauri/crates/infra/agent/src/archi_knowledge.rs -> packages/desktop/src-tauri/crates/infra/agent/src/archi_knowledge_test.rs

<!-- 挂 AC-10 的第二半（派生段改写 + 拼装/降级语义不变）。
     条目取自 design「修改文件」表第 18 行与 D-17 后半；「公共函数 / API」子表无本文件行
     （`build` / `knowledge` 均为 crate 内装配面）⇒ 按「文件级被测面」登记。 -->

#### 待测功能

- 文件级被测面（C13）: 派生段改「模型目录 + 契约资产（`<FQN>.md` 理由 + `<FQN>.schema.json` 接口，同目录）」；两处目录派生与降级语义逐字不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 派生段改写 | 正向 | 派生段含 `<FQN>.md`（理由）与 `<FQN>.schema.json`（接口）两条同目录资产的表述；模型目录派生串仍在；零「四类生成物」旧措辞——`两资产同目录` | 矩阵迭代（4 锚） |
| 拼装与降级语义 | 既有（维持） | 占位符替换 / 空骨架校验 / 拼装失败降级 `None` ——既有 `拼装与降级语义不变` 用例逐条保留——`装配零改` | 既有（维持） |
| 占位符与降级 | 异常 | 骨架占位符未全部替换 / 骨架为空 ⇒ `knowledge()` 降级 `None`（不 panic、不产出半拼文本）；未知占位符 ⇒ 原样保留——`拼装失败只降级` | 矩阵迭代（3 形态） |
| 前缀与派生工具 | 边界 | 拼装前缀逐字不变（既有 `拼装前缀不变`）；目录派生仍经 `foundation::layout`（源码锚零字面路径）——`派生单点` | 既有（维持） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | 真实 `knowledge()` / `build()` 拼装（`include_str!` 语料）+ 既有 helper | 全节 |

### packages/desktop/src-tauri/crates/core/workflow/src/prompts/proposal-planner.md -> packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table_test.rs

<!-- 挂 AC-10 的 prompt 面（三份新句断言 + 其余 11 份零 archi 工具字样）。
     「公共函数 / API」子表无本文件行（静态资产）；唯一登记处是「修改文件」表第 19 行与 D-18
     ⇒ 按「文件级被测面」登记。断言经 `phase_table_test.rs` 的 `prompt_text(name)` helper
     （`include_str!` 编译期语料，既有装置）。
     措辞冲突留痕（实现期一次收口）：design D-18 与附录 A 写「先边界、后接触面、最后理由」，
     proposal 正文写「先拓扑、后接触面、最后理由」；断言锚定「后接触面、最后理由」两序，首序
     用「先（边界|拓扑）」二择一，避免测试钉死未统一的字面。 -->

#### 待测功能

- 文件级被测面（静态资产）: 工具行补 `archi_api`（`bootstrap` 取双草稿 / `link` 算 `$ref`）；新增设计顺序句与接口面纪律句（留白写空 schema `$defs {}` + `properties {}`，不编造导出）；「只为已确定元素落笔」与 `archi_module` 自查句保留

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 设计顺序句 | 正向 | 含「后接触面」「最后理由」两序子串；首序匹配「先边界」或「先拓扑」之一；顺序句在同段落内（非散落两处）——`三序句在场` | 矩阵迭代（3 锚） |
| 工具行含 `archi_api` | 正向 | 工具行含 `archi_api`、`bootstrap`、`link` 三串；既有 `archi_module` / `archi_scaffold` 名仍在——`起草相位持有草稿工具` | 矩阵迭代（3 锚） |
| 接口面留白纪律句 | 正向 | 含「`$defs {}`」「`properties {}`」与「不编造」（或同义否定句）；零路径模板（零 `.dev-team/architecture/models` 字面）、零字段形态长表（零以 `- ` 开头的字段值行）——`留白不用编` | 矩阵迭代（4 锚） |
| 既有句保留 | 既有（维持） | 「只为已确定的元素」句与 `archi_module` 自查句逐字保留；「架构模型可写」写权声明仍在——`旧句零删` | 既有（维持） |
| 体量与占位符 | 边界 | 本份全文零 `<>` 角占位符、零 `TBD` / `TODO`、零字段形态长表（零以 `- ` 开头的字段值行）；增量在三句以内（不把知识段复制进 prompt）——`只加句不带知识段` | 单点 |
| 其余 prompt 零 archi 工具字样 | 异常 | 14 份语料中除 `proposal-planner` / `dev-design-planner` / `implementation-generator` 三份外，零 `archi_` 工具名字面（`prompt_names()` 逐份断言；既有锁定侧用例改判）——`只三份带 archi 工具` | 矩阵迭代（11 份） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | `prompt_text(name)` 走 `include_str!` 编译期语料；纯字符串断言 | 全节 |

### packages/desktop/src-tauri/crates/core/workflow/src/prompts/dev-design-planner.md -> packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table_test.rs

<!-- 挂 AC-10（同上）。「修改文件」表第 20 行与 D-18。
     dev-design 侧差异：工具行由「读四件」改「读五件」+ `archi_api`；既有 `archi_drift` /
     `archi_scaffold` 行逐字保留（两份 prompt 的只读面不同，断言分别写）。 -->

#### 待测功能

- 文件级被测面（静态资产）: 工具行「读五件 + `archi_api`」；设计顺序句与留白纪律句同 proposal 侧但工具面多 `archi_drift`；既有 `archi_drift` / `archi_scaffold` 行逐字保留

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 工具行读五件含 `archi_api` | 正向 | 含 `archi_module` / `archi_search` / `archi_drift` / `archi_scaffold` / `archi_api` 五名；零「读四件」旧措辞（若原句保留则须列出五名）——`读五件` | 矩阵迭代（5 锚） |
| 设计顺序与留白句 | 正向 | 同 proposal 侧三序与两空对象断言（两侧各自独立断言，不互相复制）——`两写权相位同纪律` | 矩阵迭代（3 锚） |
| 既有行逐字保留 | 既有（维持） | `archi_drift` 与 `archi_scaffold` 行逐字保留；自查句含 `archi_drift` + `archi_module`——`既有工具行零改` | 既有（维持） |
| 旧立场零命中 | 异常 | 「读四件」/「四个只读工具」/「无专用写工具」三串零命中；零 `<>` 角占位符与零 `TBD`——`旧措辞与占位符零残留` | 矩阵迭代（5 形态） |
| 与 proposal 侧不串味 | 边界 | `dev-design-planner` 含 `archi_drift` 而 `proposal-planner` 不含；`proposal-planner` 含 `archi_relation` 而 `dev-design-planner` 也含（两侧写权一致），但只读面差异逐名成立——`两侧工具面互不复制` | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | `prompt_text("dev-design-planner")` 的 `include_str!` 编译期语料 | 全节 |

### packages/desktop/src-tauri/crates/core/workflow/src/prompts/implementation-generator.md -> packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table_test.rs

<!-- 挂 AC-10（第三份的新句断言）。「修改文件」表第 21 行与 D-18 末句。
     本份只加一句，工具行三名（`archi_module` / `archi_api` / `archi_drift`）逐字不变。 -->

#### 待测功能

- 文件级被测面（静态资产）: 单加「改了导出面（新增 / 删除 / 改签名）就同批同步接口定义文件，改完用 `archi_api` 的 `check` 自查」

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 接口同步句 | 正向 | 含「导出面」+「同批」（或「同步」）+ `.schema.json` + `archi_api` 的 `check` 四要素；句内零路径模板（零 `.dev-team/**` 字面）——`改导出即改接口` | 矩阵迭代（4 锚） |
| 工具行零改 | 既有（维持） | 工具行三名逐字不变（既有用例维持）；本份不新增 `archi_element` / `archi_relation`（无写权）——`无写权相位零扩` | 既有（维持） |
| 无写权相位纪律 | 异常 | 全份零 `archi_element` / `archi_relation` 名；零「架构模型可写」写权声明；零 `<>` 角占位符——`无写权零扩面` | 矩阵迭代（3 形态） |
| 边界 · 与知识段不重复 | 边界 | 该句不复述三资产分工全文（句长 ≤ 80 字；零「唯一凭据」类知识句）——`一句不带知识段` | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | `prompt_text("implementation-generator")` 的 `include_str!` 编译期语料 | 全节 |

### packages/desktop/src-tauri/crates/tools/archi-api-gen/src/main.rs -> packages/desktop/src-tauri/crates/tools/archi-api-gen/src/main_test.rs

<!-- 挂 AC-2（三类产物 / `--check` 收面）、AC-7（CLI 双草稿单行 JSON 与工具出口同源）。
     条目取自 design「CLI」子表四行与「修改文件」表第 22 行；`Command` 枚举、子命令面、
     退出码语义（0 / 1 / 2）逐字不变。
     必改断言：`generate落四类生成物` → 三类（`.schema.json` 断言整体退场）；
     `bootstrap只出stdout` 的 `println!("{text}")` 源码锚 → 单行 JSON 形态。 -->

#### 待测功能

- `generate` 子命令: 写 `INDEX.toml` / `<入口主干>.i.rs` / `<入口主干>.i.ts`；MUST NOT 写 `.schema.json`；幂等语义不变
- `--check` 子命令: 生成到内存后读盘比对、零写入；差异即非零退出并列清单；`.schema.json` 永不出现在三列
- `bootstrap <元素>` 子命令: 打印**单行 JSON** `{ "schema": "…", "doc": "…" }`（与 agent 工具同形同源、零写盘）
- `Usage` 文案: 三类产物与双草稿描述（子命令面与退出码语义不变）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `generate` · 三类产物 | 正向 | 后端 + 前端夹具 ⇒ 盘上恰 `INDEX.toml` / `.i.rs` / `.i.ts` 三类；零 `.schema.json` 新增（旧断言「schema 生成物在盘」整体退场）——`不再产接口文件` | 矩阵迭代（3 形态） |
| `generate` · 幂等与目录派生 | 边界 | 连续两次 `generate` 逐字节相等；目录经 `layout` 派生；缺失目录自动创建（既有）——`幂等与派生` | 既有（维持） |
| `generate` · 坏 schema 不阻断 | 异常 | 盘上手工放坏 `.schema.json` ⇒ `generate` 不读它、不报错（只产三类）；`generate契约错误零写入` 用例的触发面改为模型 / 入口错误——`读面不参与生成` | 单点 |
| `--check` · 收面三列 | 异常 | 盘上手工 `.schema.json`（既非应生成也非 `.i.*`）⇒ `orphaned` / `mismatched` / `missing` 三列零提及；有差异 ⇒ 退出码非零并列清单；零写盘总断言维持——`schema永不进三列` | 矩阵迭代（3 形态） |
| `bootstrap` · 单行 JSON | 正向 | stdout 恰一行、可 `serde_json::from_str` 回读、两键 `schema` / `doc`（键序 schema → doc）、两值均非空字符串；零写盘（盘面快照前后相等）——`与工具出口同形` | 矩阵迭代（3 锚） |
| `bootstrap` · 与工具同源 | 正向 | 两段文本与 `archi::api::bootstrap(root, fqn)` 的 `drafts.schema` / `drafts.doc` 逐字相等（MUST NOT 在 `main.rs` 另造骨架拼装）——`两出口单源` | 单点 |
| `bootstrap` · 失败面 | 异常 | 元素不存在 ⇒ `Err` 文案含 FQN 且零写盘；`schema` 骨架可过 `validate_schema`（正例回读后直接校验）——`草稿即过闸` | 矩阵迭代（2 形态） |
| `Usage` 与子命令面 | 边界 | `argv` 解析三子命令逐字不变；非法参数归 `Usage` 退出码 2；`Usage` 文案含三类产物与双草稿描述、零「四类」「四份」旧措辞——`命令面零改` | 矩阵迭代（3 形态） |
| 源码面纪律 | 边界 | `include_str!("main.rs")` 零 `Command::new`（零子进程）、零 `knip`；JSON 经 `serde_json` 序列化（零手写转义 `\\\"`）——`零手拼` | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | 自建 `TempDir`（std，零 dev-dependency 既有形态）+ 真实 `run(Command, root)`；盘面快照 helper 既有 | 全节 |
| 真实 `archi` crate | 不 mock——CLI 是 `archi::api::bootstrap` 的第二出口，替身会让「同源」不可见 | `bootstrap` 臂 |
| 子进程 | 零使用（源码锚断言） | 全节 |

### packages/desktop/src/views/architecture/components/dashboard/contract-panel.tsx -> packages/desktop/src/views/architecture/components/dashboard/contract-panel.test.tsx

<!-- 挂 AC-3（`ArchiApiCheck` 夹具载荷）/ AC-8（chip 集合随数据收缩）/ D-22（仅冷启动文案）。
     「公共函数 / API」子表无本文件行（React 组件，零功能改动）；被测面来自「修改文件」表第 26 行
     与 `desktop-archi-contract-panel` spec ⇒ 按「文件级被测面」登记。
     既有锚：`data-category` / `data-testid` / `data-axis` / `data-kind`（面板查询面零改）。 -->

#### 待测功能

- 文件级被测面（组件）: 冷启动文案「契约面未启用」→「接口面未启用」；chip 由 `apiFindings` 数据聚合（`category` 成员收缩后集合随动，七组不变）；五态 / 聚焦 / 打开通路零功能改动

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 冷启动文案 | 正向 | `artifacts.enabled = false` ⇒ 面板文案含「接口面未启用」、零「契约面未启用」——`文案随语义` | 单点 |
| `category` chip 集合 | 异常 | 夹具注入七组各 1 条（`doc` 组用 `docMissing`）⇒ 七个 chip 按既有序渲染；注入含已退役 kind 的载荷 ⇒ 面板零新类别常量（既有「零类别常量」断言扩面：`contract-panel.tsx` 内零 `'apiReexportDrift'` / `'docNotFound'` / `'apiDocMissing'` 字面）——`chip由数据聚合` | 矩阵迭代（7 组） |
| 单组与空组边界 | 边界 | `findings` 全为 `docMissing`（38 条）⇒ 恰一个 `doc` chip + 计数 38；`findings = []` ⇒ 零 chip 零 crash；单条 `element = null` ⇒ 归「无归属」——`空组不渲染` | 矩阵迭代（3 形态） |
| 五态与交互零回归 | 既有（维持） | 五态渲染 / 聚焦 `signature` 只列该 kind / 打开通路（`data-testid`）逐条不变——既有断言保留——`零功能改动` | 既有（维持） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（进程边界） | `vi.mock` + `invokeMock`（既有 `vi.hoisted` 装置）按命令名分发；IPC 是进程边界，属允许 mock 面 | 全节 |
| 数据 | `ArchiApiCheck` / `ArchiApiFinding` 夹具字面量（含七组 category）——不入 mock，直接作为 props / mock 返回值 | 全节 |

### packages/desktop/src/views/architecture/components/model-info.tsx -> packages/desktop/src/views/architecture/components/model-info.test.tsx

<!-- 挂 AC-6 的呈现面（`ArchiModuleDetail` 载荷 + `doc` 原样投影）/ D-22（仅 JSDoc）。
     「公共函数 / API」子表无本文件行；「修改文件」表第 27 行只登记 `ContractSection` 的 JSDoc
     改述（三路径面归右栏「文档」分段 ⇒ 只渲非空项）。既有装置：`invoke` mock +
     `element()` / `relation()` / `DECLARED_API` 夹具。 -->

#### 待测功能

- 文件级被测面（组件）: `ContractSection` 的注释口径改「本元素判据」（三路径面不在本段）；渲染结构零改动；`api.status` / `doc` 的呈现规则不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ContractSection` 只列本元素判据 | 正向 | `api.status = declared` + 一张 finding ⇒ 判据列表渲染、零三路径行（三路径面在右栏「文档」分段）——`判据段不列路径` | 单点 |
| `api.status` 三态 | 异常 | `none` / `declared` + 三路径独立（`contractPath` 为 `null` 且 `schemaPath` 非空）⇒ 判据段照渲、零 crash；`declared` + 三路径全 `null` ⇒ 判据段不崩——`路径缺席不影响判据段` | 矩阵迭代（3 形态） |
| `doc` 呈现规则 | 边界 | `api = declared` + `doc` 有值 ⇒ 「入口文档」段不渲（呈现规则不变，D-23 只改 DTO 投影）；`api = none` + `doc` 有值 ⇒ 渲 ——`呈现规则不变` | 矩阵迭代（2 形态） |
| 写权与门限零回归 | 既有（维持） | `archi_set_gate` 通路 / 门限展示 / 既有「文档段」断言逐条不变——既有用例保留——`零功能改动` | 既有（维持） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（进程边界） | `vi.mock` + `invokeMock` 按命令名分发（`archi_module` / `read_file` / `archi_set_gate`），既有装置 | 全节 |
| 夹具 | `element()` / `DECLARED_API` 字面量扩展（`assets` / `reading` 不必进入本组件夹具——`ModelInfo` 不吃 `ArchiModuleDetail`；`detailOf` 面在 `architecture-panel.test.tsx`） | 全节 |

### packages/desktop/src/views/architecture/components/dashboard/architecture-node.tsx -> packages/desktop/src/views/architecture/components/architecture-panel.test.tsx

<!-- 挂 AC-6 的右栏「文档」分段（三路径各自独立后「只渲非空项」天然正确）/ D-22。
     本文件无 dedicated test（proposal 测试文件表列的 `architecture-node.test.tsx` 不存在）；
     渲染宿主 = `architecture-panel.tsx` → `architecture-dashboard.tsx` → `ArchitectureNode`，
     其 `detailOf()` 夹具是 `ArchiModuleDetail` 的唯一驱动面 ⇒ 机械随动落该文件。
     改动本体是一行语义注释（三路径 = 文档 / 接口 / 生成物，各自在盘事实）——注释面见不可测试项 7。 -->

#### 待测功能

- 文件级被测面（组件，零行为改动）: 右栏「文档」分段三条路径各自按在盘事实 push；`contractPath` = 文档、`schemaPath` = 接口、`generatedPath` = 生成物

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 三路径独立渲染 | 正向 | `detailOf()` 夹具三态：只有 schema（`contractPath = null`）⇒ 「文档」段不渲该行、零 crash；只有 md ⇒ 渲该行；三条全在 ⇒ 三行按既有序——`只渲非空项` | 矩阵迭代（3 形态） |
| 缺失与异常形态 | 异常 | 三路径全 `null` ⇒ 段落整体不渲（零空标题）；`status = none` + `schemaPath` 非空（越界态）⇒ 仍渲接口行——`越界态仍报事实` | 矩阵迭代（2 形态） |
| 载荷随动 | 边界 | `detailOf()` 补 `assets`（三资产各一态）与 `reading`；面板既有断言零改（零功能改动）——`夹具随动断言不变` | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core`（进程边界） | `vi.mock` + `invokeMock`（`archi_module` 返回 `detailOf(fqn)`），既有装置 | 全节 |
| 夹具 | `detailOf()` 字面量扩展（`assets` / `reading` 必填）——不入 mock | 全节 |

---

## 不可测试项

| # | 模块 | 原因 | 替代保证 |
|---|---|---|---|
| 1 · 版本行 | `packages/desktop/package.json`（`version` 相对落地时 `HEAD` +1 patch）、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml` 的 crate 版本行 | 一行字面量，无函数 / 分支 / 输出可断言；版本纪律是交付面机械判据（AC-14） | 桌面端 `git diff` 核对：`package.json` 恰 +1 patch、`tauri.conf.json` 仍 `"version": "../package.json"`、`src-tauri/Cargo.toml` 版本行逐字不变 |
| 2 · 仓库根规则面（AC-11） | `AGENT.md` 与 `CLAUDE.md` 的「模块 API 契约」节（各恰 5 条要点 / 两份节内正文逐字相同 / 零 `api-*` fence 示例 / 零 `openspec/specs/` 路径） | 纯文本资产，不进编译单元；仓内无任何测试以 `include_str!` 读这两份文件（`compose_test.rs` 的 `AGENT.md` 只作会话注入夹具，内容自由） | 桌面端 static-check 按 AC-11 四条逐项核对 + `diff` 比对两份节内正文逐字相同；R12（同源副本分化）由「同提交改两份」的交付纪律兜住 |
| 3 · 生成物零 diff 与仓级读数（AC-1 / AC-2 / AC-3 / AC-9） | ① `.dev-team` 根 `archi_drift` 除 `timestamp` / `durationMs` / 38 条 `docMissing` 文案外逐字段不变；② `api:check` 零差异且 `orphaned` 不含 `.schema.json`；③ `.i.rs` / `.i.ts` 逐字节不变；④ `INDEX.toml` 只差头部一句；⑤ `bindings:export` 的 `git diff` 只含 `assets` / `reading` + 4 新类型 + 联合三值离场；⑥ 「`declared` ∧ schema 在盘 = 11」 | 仓级读数、产物 diff 与目录级扫描不是可执行单元；生成物不许手改（会被下次生成覆盖），读真仓面会让测试与仓数据耦合 | 桌面端执行 `pnpm run api:export` / `api:check` / `bindings:export` 后逐项核对 `git diff`；单测窄面兜各自一处：`main_test.rs`（三类产物 / `--check` 三列）、`diff_test.rs`（孤儿收面）、`api/mod_test.rs`（`render_all` 三类 + `gating_findings` 口径）、`bindings/mod_test.rs`（两份清单 + 字段面） |
| 4 · 金样语料重写（R7） | `packages/desktop/src-tauri/crates/core/archi/tests/golden_test.rs` + `tests/corpus/drift/**` 的 `docMissing` 文案与判据词汇重写；`reports/archi/**` 门禁产物 | 金样正文由 `DESKTOP_GOLDEN_REWRITE=1` 重写后人工确认——语料重生成不是单测断言；`reports/` 是门禁运行产物、不入库（design 零改动面） | 桌面端以 `DESKTOP_GOLDEN_REWRITE=1` 重写后逐例复核：`api-mismatch` case 仍产非空 `apiFindings`；单测面由 `api/check_test.rs` 的 `docMissing` 文案 + 三值零产出断言兜住 |
| 5 · 仓级 grep 零命中（AC-15 / 退役面） | 产品源码面零 `carrier_path` / `doc_path` / `DocFact` / `build_schema` / `ContractSection` / `FENCE_OP` / `reexport_prose` / `doc_facts`；`ArchiApiFindingKind` 三值零字面；零「第二份载体派生」 | 仓级扫描不是可执行单元（单测读全仓会越过测试边界并把别的 `*_test.rs` 防线拖进判定面） | 单测窄面各兜一处：`api/paths_test.rs`（`paths.rs` 内零命中）、`api/contract_test.rs`（`parse` 退役面）、`api/schema_test.rs`（`build_schema` 退役面）、`search_test.rs`（语料派生各一处）、`edit/warnings_test.rs`（零 `doc_path(`）；跨文件仓级面由桌面端 static-check 按 tasks.md 的 grep 执行 |
| 6 · spec delta 与条件性修改（Q-1 / Q-6 / Q-7 / Q-8） | 11 份 delta 的 requirement / scenario 结构与承接块措辞；design「条件性修改」表 4 行（`link` 示例串 / `desktop-archi-model` 两处表述 / `ContractType` token / `drift/mod.rs` 两处注释） | spec 与未裁决的修正落点是文本资产，不是可执行单元；未获裁决即零动作 | 桌面端按 delta 结构逐条核对（每条 Requirement 至少一个 Scenario、WHEN / THEN 成对、含 `## Module Contract` 段）；4 行按裁决结果逐行执行并留 diff；`link` 的行为锚已由 `archi_tool_test.rs` 的「前缀 == `ref_relative` 输出」用例钉住（不比 spec 示例字面，Q-1） |
| 7 · 前端注释面 | `.../components/dashboard/architecture-node.tsx` 的一行语义注释、`.../components/model-info.tsx` 的 `ContractSection` JSDoc —— 注释文本本身无行为可断言 | 注释不是运行期事实；为注释写 `?raw` 源码断言会把纯文案变成第二份判据 | 行为面已由「三路径独立渲染」「`ContractSection` 只列本元素判据」两条用例兜住；注释语义由桌面端 static-check / review 核对（D-22 零功能改动） |
| 8 · 仓内模型与 11 份资产正文（P10 / 零改动面） | `.dev-team/architecture/graph/model/*.toml`（55 元素 / 129 关系 / `modelHash`）、`.dev-team/architecture/models/**` 的 11 份 `.md` 与 11 份 `.schema.json` 正文（含 `$comment` 旧文案） | 本变更零模型字节改动、零迁移；测试一律用合成 tempdir 夹具，读真仓面会与本仓数据耦合 | 桌面端 `git diff -- .dev-team/architecture/` 核对（仅 `INDEX.toml` 头部一句）；`archi_drift` 读数复核 55 / 129 / `modelHash` 逐字不变；写闸对本仓 11 份 schema 的实测全过（`validate_schema` 单测的合法面覆盖同一形态） |

