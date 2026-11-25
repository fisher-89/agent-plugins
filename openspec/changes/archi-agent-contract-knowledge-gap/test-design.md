# 测试设计: archi-agent-contract-knowledge-gap

> **日期**: 2026-11-24
> **输入**: 本 change 目录 `proposal.md`（AC-1…AC-17 / R-1…R-15）、`design.md`（C1–C11 组件、「新增文件」2 行 + 「修改文件」21 行、「公共函数 / API」13 行、「内部接线面」6 行、「类型定义」13 行、D-1…D-19、附录 A 骨架草案）、`tasks.md`（阶段 1–6）。
> **性质**: 写面（两 agent 工具）+ 引擎语义（内联出边 / 单文件级联 / 落点四情形 / 反馈面）+ 知识骨架收缩 + 五处立场改写的原子交付。被测面 = **新增 2 个源文件**（`edit/warnings.rs` / `sdk/atomic.rs`）+ **修改 18 个源文件**（`commands/atomic.rs` 与 `crates/infra/agent/src/lib.rs` 为落位迁移面）+ 2 份生成物（`bindings.ts` / `bindings.zod.ts`）；`packages/desktop/package.json` 的一行版本号不纳入被测范围（见不可测试项 1）。
> **本版口径（回溯重做）**: 按 `design.md` D-17 / D-18 / D-19 与 `tasks.md` 阶段 6 同批修订——`ElementSpec` 只扩 `relations` / `create` 两字段（级联唯一承载位 = `plan_element_delete` 的值参）、`DeleteSet` 只留 `of(fqn)` 单构造子（零 `is_empty`）、`archi_tool.rs` 增 `domain_value` 值域查表私有核与 `scaffold_payload` doc 两写工具口径两个被测面。只改断言口径与条目描述，不新增独立测试章节，用例判据不变。
> **路径口径**: `crates/**` / `src/**` 均指 `packages/desktop/src-tauri/` 之下；本文一律写仓库相对全路径。
> **框架识别**: Rust 单元测试（`.dev-team/config.json` 的 `framework = rust`）——`#[test]` / `#[tokio::test]` 属性宏 + std 断言宏（`assert!` / `assert_eq!` / `assert_ne!` / `expect_err`），**零第三方断言库**；测试文件与源文件同目录共置 `*_test.rs`，由 `#[cfg(test)] mod <name>_test;` 在 `lib.rs` / `<dir>/mod.rs` 登记（`commands/atomic.rs` 用 `#[path = "atomic_test.rs"]` 形态）。
> **测试装置**: `crates/core/archi` **零 dev-dependencies**（临时目录由 std 自建或沿 `edit/mod_test.rs` 既有 `TempDir` RAII + `AtomicU64` 计数器先例）；`crates/infra/agent` / 壳层用既有 `tempfile`（dev-dependency，零新增）。异步执行体用例为 `#[tokio::test]`（沿 `archi_tool_test.rs` 先例）；壳层命令用例沿 `tauri::test::mock_app()` + 真实 `WorkspaceStores`（tempdir 数据根）既有装置。测试运行 / 覆盖率 / 变异由桌面端 test-execution 执行，本文不复制命令细节。
> **同批约束（落地顺序）**: ① `edit/mod.rs` 末尾补 `#[cfg(test)] mod warnings_test;`（与既有 `mod mod_test;` / `mod messages_test;` 同形）；② `sdk/mod.rs` 补 `pub(crate) mod atomic;` 与 `#[cfg(test)] mod atomic_test;` 两行（D-13 的机械连带）；③ `sdk/atomic_test.rs` 的用例是既有 `src/commands/atomic_test.rs` 用例的**迁移 + 保留**（内容逐条保留，源侧只留薄委托断言）。测试文件未落地前先补声明会让 `cargo test` 解析失败，故声明与文件同批。
> **用例名口径**: 沿既有「中文用例名 + 小写 ASCII 标识符」写法（如 `plan_element_upsert`）；需要 ASCII 驼峰时补 `#[allow(non_snake_case)]`。用例名不写测试命令、不写断言细节。
> **纪律**: 用例只断言**行为与单点归属**；源码锚（`include_str!`）只钉「名字 / 调用序 / 计数 / 零命中」，MUST NOT 退化成对已被行为用例覆盖的语义做第二份判据（沿 `api/mod_test.rs` / `dto_test.rs` 先例）。不新增 test-only 导出（AGENT.md 第 5 条）——私有件经子模块路径 `super::…` 直调。

---

## 验收范围

<!-- 逐条映射 proposal.md 的 17 条 AC。被测文件或模块 = 承载用例的测试文件；
     仓级读数 / 生成物零 diff / 一行字面量 / spec 文本面落「—（见不可测试项 N）」，并写明替代保证。 -->

| AC ID | 验收条件 | 被测文件或模块 |
|---|---|---|
| AC-1 | 挂载清单恰 **18** 项（七内建 + `create_explore_note` + 五 archi 只读 + 两 archi 写 + 两 DB + `propose_config`）；两写工具 `ToolTier::Write` / `ToolExec::Archi`；`default` 档拒绝、`acceptEdits` / `bypassPermissions` 放行；五只读档位逐字不变 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount_test.rs`（18 项名字序 + 档位 + 定义面）、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/policy_test.rs`（三档等价表封闭集随动，机械必改） |
| AC-2 | 矩阵下发两写工具的 `(phase, role)` 集合 == `model_write_lock` 判 `None` 的集合，逐格相等；archi 臂七名；未命中相位零下发 | `packages/desktop/src-tauri/crates/infra/agent/src/compose_test.rs`（矩阵 7 行 + archi 臂 + 同集断言；loop 侧成员校验由既有 `sdk/loop_test.rs` 机制背书，不改） |
| AC-3 | 落点四情形：既有元素落其文件（`file` 不同值 ⇒ `Err`）；新建省略 `file` 落父元素文件；单段 FQN 省略 `file` ⇒ `Err` 点名需给 `file`；`file` 不存在且无 `create` ⇒ `Err` 提示 `create = true`，给了 `create` ⇒ 新文件带 `# <名>（1 元素 / 0 关系）` 表头 | `packages/desktop/src-tauri/crates/core/archi/src/edit/mod_test.rs` |
| AC-4 | 内联出边四条约束：同批重复 `to` ⇒ `Err`（1 基下标 + `action` + `to` 同行）；`delete` 项带 `desc` / `apis` ⇒ `Err`；内联边已声明在别的模型文件 ⇒ `Err` 点名该文件；未列出的既有边零改动；四计数守恒 | `packages/desktop/src-tauri/crates/core/archi/src/edit/mod_test.rs`（apply 面四情形 + plan 面四条前置 + 守恒） |
| AC-5 | `cascade = false` 时 P3 两拒绝臂逐字保留；`cascade = true` 时同文件闭包删净（子树 + 出入边 + 表头计数随动），跨文件 ⇒ `Err` 点名文件且零落盘；删除纯函数恰一个 | `packages/desktop/src-tauri/crates/core/archi/src/edit/mod_test.rs`（行为 + 源码锚）；仓级「第二个删除实现零命中」见不可测试项 6 |
| AC-6 | `warnings` 恒在场（空数组 = 无异常）；`changed` 空 = 幂等（盘上字节不变）、非空 ⊆ 参数面字段名；`Err` 满足「第一行自足」（含 `重试：` / `下一步：` / `改用 <工具名>` 之一或可复制 JSON 片段） | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool_test.rs`（E1 / E2 / plan Err 三面 + 幂等）、`packages/desktop/src-tauri/crates/core/archi/src/edit/mod_test.rs`（`changed` / `created` 求值 + `EditError::message()` 形态）、`packages/desktop/src-tauri/crates/core/archi/src/edit/messages_test.rs`（文案族封闭） |
| AC-7 | 七条 warning 逐条红绿；`warnings[].kind` ∈ 两枚举 `as_str` 并集；severity 与 `ArchiApiFindingKind::severity()` 逐值一致、drift 两值恒 `warn`；降噪不变式成立；源码零 `drift::check` 引用 | `packages/desktop/src-tauri/crates/core/archi/src/edit/warnings_test.rs`（七条 + 降噪 + 单点 + 源码锚）、`packages/desktop/src-tauri/crates/core/archi/src/edit/mod_test.rs`（`WriteOutcome.warnings` 接线：元素写回调 / 删除 / relation 恒空） |
| AC-8 | 成功响应的 `counts` 与写后该文件重解析计数相等、与表头括号数字相等；`model` 与写后 `archi_drift` 报告的 `model.elements` / `model.relations` 逐值相等；成功响应序列化长度 < `MAX_RESULT_BYTES` | `packages/desktop/src-tauri/crates/core/archi/src/edit/mod_test.rs`（`counts` 对 `TomlParser::parse`、`model` 对 `parse_dir`——drift 报告同源，数值同一）、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool_test.rs`（响应体量上界） |
| AC-9 | `archi_module` 单元素响应的 `fileHash` 与 `drift::file_hash(声明文件)` 逐字相等；`archi_element { expect: fileHash }` 成功；改动文件后旧 `expect` ⇒ `Err` + 零落盘；响应 `hash` 可作下一次 `expect` | `packages/desktop/src-tauri/crates/core/archi/src/dto_test.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool_test.rs`（往返组合）、`packages/desktop/src-tauri/src/commands/archi/mod_test.rs`（出线面） |
| AC-10 | 两工具 schema 含八行上限与全部 `enum`（由 `ALL` 派生）；限额常量单点；执行体 E1 对 `action` / `element` / 值域 / 上限逐项拒绝；值域查表单源（`domain_value` 私有核为 `optional_domain` 与 `archi_scaffold` 共用，判据句一条） | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount_test.rs`（schema 与 `WRITE_LIMITS` 对拍 + 单点锚）、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool_test.rs`（E1 逐项硬闸 + `domain_value` 两入口同判据句与单源锚） |
| AC-11 | 骨架 ≤ 1500 B、全文 ≤ 1700 B 且实测值冻结；四段在场（含手改出口一句）；零示例 TOML 围栏 / 零键形示例 / 零字段值域行 / 零结构诊断词；两处派生目录仍在行程 | `packages/desktop/src-tauri/crates/infra/agent/src/archi_knowledge_test.rs` |
| AC-12 | 旧立场零残留：(a) 产品源码面 `.rs` 三句字面零命中，且 `scaffold_payload` doc / 模块头 / `SCAFFOLD_TEMPLATE` 首行三处同口径（零「写盘仍走既有 `write` / `edit`」）；(b) 两份 phase prompt 零命中且含两写工具名 + 「工具表达不了的才手改」+ 自查句；(c) delta 结构面五处承接块 | (a) 单测窄面：`packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount_test.rs`（scaffold 常量 doc）、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool_test.rs`（`SCAFFOLD_TEMPLATE` 首行 + `scaffold_payload` doc 三处口径）；(b) `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table_test.rs`；(c) 见不可测试项 4 |
| AC-13 | `archi_set_gate` 与 `archi_element(gate)` 产逐字节相同文本（同实现）；签名 / 错误面 / 失败零写入不变；模型写盘走 `write_atomic` 且恰一份实现（infra 侧；壳层薄委托） | `packages/desktop/src-tauri/src/commands/archi/mod_test.rs`（命令落盘文本 == `plan_element_upsert` 的 `content`）、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool_test.rs`（工具落盘文本 == 同一 `content`，两半合取即等价）、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/atomic_test.rs` + `packages/desktop/src-tauri/src/commands/atomic_test.rs`（唯一实现与薄委托） |
| AC-14 | 两写工具的目标文件命中用户 `write_protection.files` 规则时被拒（拒串沿用既有族、`%s` / `%t` 替换正确）；未命中放行；零第二份规则集 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/mod_test.rs`（`evaluate_target` 三档）、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool_test.rs`（拒写零落盘） |
| AC-15 | `bindings.ts` / `bindings.zod.ts` 的 diff 均恰为 `ArchiModuleDetail` 增 `fileHash` 一字段；`api:check` 零差异；`.dev-team/architecture/**` 零字节变化；`apiFindings` 计数不变 | `packages/desktop/src-tauri/src/bindings/mod_test.rs`（字段序八字段 + zod schema 同字段）；零 diff / 仓级读数见不可测试项 2 |
| AC-16 | `package.json` `version` 相对 `HEAD` 恰 +1 patch；`src-tauri/Cargo.toml` 逐字不变；改动集 ⊆ 提案白名单 | —（见不可测试项 1 / 5） |
| AC-17 | `specs/` 恰七个能力目录；RENAMED 的 `FROM:` / `TO:` 逐字一致；MODIFIED 为基线整段拷贝后改；每条 Requirement 至少一个 Scenario 且 WHEN/THEN 成对；各 delta 含 `## Module Contract` 段 | —（见不可测试项 4） |

**源文件 → 测试文件映射**（design「新增文件」2 行 + 「修改文件」21 行，逐行给出；删除文件不存在于本变更）：

| 源文件（design 新增 / 修改） | 测试文件 | 映射依据 |
|---|---|---|
| `packages/desktop/src-tauri/crates/core/archi/src/edit/warnings.rs`（新增） | `packages/desktop/src-tauri/crates/core/archi/src/edit/warnings_test.rs`（新增） | 目录化模块共置测试；由 `edit/mod.rs` 末尾补 `#[cfg(test)] mod warnings_test;` 登记（与 `mod messages_test;` 同形） |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/atomic.rs`（新增） | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/atomic_test.rs`（新增） | 同目录共置测试；由 `sdk/mod.rs` 补 `#[cfg(test)] mod atomic_test;` 登记；用例集 = 壳层既有原子写用例迁入（内容逐条保留） |
| `packages/desktop/src-tauri/crates/core/archi/src/edit/mod.rs`（修改） | `packages/desktop/src-tauri/crates/core/archi/src/edit/mod_test.rs`（既有，扩写） | 既有 `#[cfg(test)] mod mod_test;` 已登记；既有 12 组用例逐条保留 + 新增语义用例 |
| `packages/desktop/src-tauri/crates/core/archi/src/edit/messages.rs`（修改） | `packages/desktop/src-tauri/crates/core/archi/src/edit/messages_test.rs`（既有，扩写） | 既有登记；`ALL` ↔ `criterion()` 穷尽对拍随 19 → 23 扩 |
| `packages/desktop/src-tauri/crates/core/archi/src/lib.rs`（修改） | `packages/desktop/src-tauri/crates/core/archi/src/edit/mod_test.rs` | 纯出口整理无函数面；本 crate 无 `lib_test.rs` 惯例（`dto_test.rs` doc 明写），出口面锚经 `include_str!("../lib.rs")`，沿归档 `archi-edit-engine-primitives` 先例 |
| `packages/desktop/src-tauri/crates/core/archi/src/dto.rs`（修改） | `packages/desktop/src-tauri/crates/core/archi/src/dto_test.rs`（既有，扩写） | 既有共置测试，`module_detail` 行为面已在该文件 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs`（修改） | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount_test.rs`（既有，扩写） | 既有登记；另含两处机械随动：`sdk/policy_test.rs`（准入档封闭集）与 `sdk/config_propose_test.rs`（末项计数）——均在 mount.rs 节以「落 <文件>」标注，不新开章节 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool.rs`（修改） | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool_test.rs`（既有，扩写） | 既有 `#[tokio::test]` 装置（`healthy` / `execute_json` / `snapshot`）直接复用 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs`（修改） | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner_test.rs`（既有，扩写） | 既有 `mint_archi_context` 铸造判据用例族已在该文件 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools.rs`（修改） | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools_test.rs`（既有，扩写） | 既有 `input_path` 用例族已在该文件 |
| `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs`（修改） | `packages/desktop/src-tauri/crates/infra/agent/src/compose_test.rs`（既有，扩写） | 既有矩阵表驱动 + `model_write_lock` 用例族已在该文件 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/mod.rs`（修改） | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/mod_test.rs`（既有，扩写） | 既有 `ProtectRules` 加载 / 匹配 / 拒串用例族已在该文件 |
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mod.rs`（修改，D-13 一行） | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/atomic_test.rs` | 一行模块声明无行为；断言面 = 源码锚 + 编译期可达（`atomic_test` 能编译即登记成立），沿 `lib.rs → edit/mod_test.rs` 先例 |
| `packages/desktop/src-tauri/crates/infra/agent/src/lib.rs`（修改） | `packages/desktop/src-tauri/crates/infra/agent/src/lib_test.rs`（既有，扩写） | 既有 `crate根导出面锚定…` 用例族正是导出面归属地 |
| `packages/desktop/src-tauri/crates/infra/agent/src/archi-knowledge.md`（修改） | `packages/desktop/src-tauri/crates/infra/agent/src/archi_knowledge_test.rs`（既有，改判 + 扩写） | 语料测试文件以 `include_str!("archi-knowledge.md")` 直读骨架；门限锚已在该文件 |
| `packages/desktop/src-tauri/crates/infra/agent/src/archi_knowledge.rs`（修改） | `packages/desktop/src-tauri/crates/infra/agent/src/archi_knowledge_test.rs`（既有） | 同上（`build` / `knowledge` 拼装与降级用例已在该文件） |
| `packages/desktop/src-tauri/src/commands/atomic.rs`（修改） | `packages/desktop/src-tauri/src/commands/atomic_test.rs`（既有，收窄） | 既有 `#[cfg(test)] #[path = "atomic_test.rs"]` 登记；行为用例迁 infra 侧，本文件留薄委托断言 |
| `packages/desktop/src-tauri/src/commands/archi/mod.rs`（修改） | `packages/desktop/src-tauri/src/commands/archi/mod_test.rs`（既有，改 needle + 扩写） | 既有 `archi_set_gate` / `archi_module` 用例族已在该文件 |
| `packages/desktop/src-tauri/crates/core/workflow/src/prompts/proposal-planner.md`（修改） | `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table_test.rs`（既有，改判） | 该文件是 `include_str!` 编译期语料断言 prompt 正文的唯一测试归属地（`prompt_text` / `prompt_names` 私有 helper） |
| `packages/desktop/src-tauri/crates/core/workflow/src/prompts/dev-design-planner.md`（修改） | `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table_test.rs`（既有，改判） | 同上 |
| `packages/desktop/src-tauri/src/types/generated/bindings.ts`（修改，生成物） | `packages/desktop/src-tauri/src/bindings/mod_test.rs`（既有，改判） | 该文件经 `export_bindings()` 重导产物并逐段解析；`ArchiModuleDetail` 七字段序断言在此（必红改八字段） |
| `packages/desktop/src-tauri/src/types/generated/bindings.zod.ts`（修改，生成物） | `packages/desktop/src-tauri/src/bindings/mod_test.rs`（既有，扩写） | 同文件的 `zod_generated_path()` 已覆盖该产物（导出稳定性 / 同批恢复用例）；补 `ArchiModuleDetailSchema` 字段面断言 |
| `packages/desktop/package.json`（修改） | —（无测试文件） | 一行版本号，无行为可断言 ⇒ 见不可测试项 1 |

---

## 单元测试

<!-- 框架与装置：Rust `#[test]` / `#[tokio::test]` + std 断言宏，零第三方断言库；
     `crates/core/archi` 零 dev-dependencies（临时目录 std 自建或沿 `edit/mod_test.rs`
     既有 TempDir RAII），`crates/infra/agent` / 壳层用既有 `tempfile`（零新增）。

     跨模块组合用例一律挂链路发起方 / 最上层调用方，不新建集成章节：
     - `plan_* → apply_* → 自证 / 表头 / counts / model` 挂 `edit/mod.rs` 节；
     - `plan_* → upsert_warnings / delete_warnings` 挂 `edit/mod.rs` 节（warnings 模块
       自身判据挂 `warnings.rs` 节）；
     - `plan_* → ProtectRules::evaluate_target → write_atomic → 响应` 挂 `archi_tool.rs` 节；
     - `archi_module(fileHash) → archi_element(expect) → 响应 hash` 挂 `archi_tool.rs` 节；
     - `mint_archi_context → with_protect → execute` 挂 `runner.rs` 节（装配侧是发起方）；
     - `命令 archi_set_gate → plan_element_upsert` 挂 `commands/archi/mod.rs` 节；
       「两通路同文本」的另一半（写工具侧）挂 `archi_tool.rs` 节，两半合取即 AC-13。

     源码锚纪律：只断言「名字 / 调用序 / 计数 / 零命中」，用 `include_str!("<同目录文件>")`
     取编译期内嵌字节（禁测试内复制副本、禁相对路径读盘）；被行为用例覆盖的语义不写第二份。 -->

### packages/desktop/src-tauri/crates/core/archi/src/edit/warnings.rs -> packages/desktop/src-tauri/crates/core/archi/src/edit/warnings_test.rs

<!-- 挂 AC-7（七条逐条红绿 / kind 与 severity 单点 / 降噪不变式 / 源码零 drift::check）、
     AC-6（warnings 恒在场、message 形态）。
     条目取自 design「公共函数 / API」子表中「所在文件 = edit/warnings.rs」的 2 行
     （`upsert_warnings` / `delete_warnings`）与「类型定义」子表的 `EditWarning` 一行；
     未做源文件导出扫描。
     装置：真实 `api::paths::{contract_path, doc_path}` 与真实盘上文件（`TempDir`）——
     路径派生与 `Path::exists` 即被测判据，替身会抹掉「契约 md 在 / 不在盘」的语义；
     `Element` 由 `TomlParser::parse` 真实解析合成源取得（不手搓残形）。 -->

#### 待测功能

- `upsert_warnings()`: 六条 upsert 判据（`noPath` / `pathNotFound` / `docMissing`×2 / `docNotFound` / `apiUnexpected`）+ 降噪过滤；`element` 取写后文件重解析结果；`kind` / `severity` 取既有枚举 `as_str` / `severity()` 返回值，路径取 `api::paths`
- `delete_warnings()`: 仅第 7 条（删除后孤儿契约 md ⇒ `apiUnexpected` / `warn`），恒进场；不读模型、不跑 `drift::check`
- `EditWarning`（类型）: `{ kind: &'static str, severity: &'static str, message: String }`——kind / severity 存既有单点的 `as_str()` 返回值（零新词表、零新档位）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `upsert_warnings()` · 第 1 条 `noPath` | 正向 | `package` 且 `path` 空 + `changed ∋ path` ⇒ 恰一条 `kind = "noPath"` / `severity = "warn"`；对照臂：`composite` 且 `path` 空 ⇒ 零条；`module` 且 `path` 空 ⇒ 一条——`纯分组容器外的空path报noPath` | 矩阵迭代（3 形态） |
| `upsert_warnings()` · 第 1 条 的 `kind` 归因 | 边界 | `changed` 只含 `api` / `doc` 时（`package` + `path` 空 + `created = false`）⇒ `noPath` 零条；`changed ∋ kind` 与 `changed ∋ path` 两臂各出一条（归因集合含 `kind`）——`noPath只由path或kind诱发` | 矩阵迭代（2 形态） |
| `upsert_warnings()` · 第 2 条 `pathNotFound` | 异常 | `path` 项指向不存在的目录 / 文件 + `changed ∋ path` ⇒ 每条缺席项一条 `pathNotFound` / `warn`；全部在盘 ⇒ 零条；多项缺席 ⇒ 多条、逐条 message 含该路径——`path缺席逐条报pathNotFound` | 矩阵迭代（3 形态） |
| `upsert_warnings()` · 第 3 条 `docMissing`（declared） | 正向 | `api = declared` 且派生契约 md 缺席 + `changed ∋ api` ⇒ `docMissing` / `warn`，message 尾句指向 `archi_api` 的 `bootstrap`；契约 md 在盘 ⇒ 零条——`declared缺契约报docMissing` | 矩阵迭代（2 形态） |
| `upsert_warnings()` · 第 4 条 `docMissing`（none 未声明 doc） | 异常 | `api = none` 且 `doc` 空 + `changed ∋ api` ⇒ `docMissing` / `warn`，message 尾句指向经 `doc` 声明；`doc` 有值 ⇒ 该条零命中（转由第 5 条判）——`none未声明doc报docMissing` | 矩阵迭代（2 形态） |
| `upsert_warnings()` · 第 5 条 `docNotFound` | 异常 | `api = none` + `doc` 指不到盘 ⇒ `docNotFound` / **`error`**（七条中唯一 `error` 档）；`doc` 指到盘 ⇒ 零条——`doc指不到盘报docNotFound且档位为error` | 矩阵迭代（2 形态） |
| `upsert_warnings()` · 第 6 条 `apiUnexpected` | 异常 | `api = none` 且派生契约 md 在盘 + `changed ∋ api` ⇒ `apiUnexpected` / `warn`，message 尾句给「删契约 md / 改 declared」两条出路；`api = declared` 且在盘 ⇒ 零条——`none有契约报apiUnexpected` | 矩阵迭代（2 形态） |
| `upsert_warnings()` · 降噪：非诱发字段 | 边界 | `changed` 只含 `summary` / `gate` / `metadata`（三形态，事实面同时具备可触发 `noPath` / `docMissing` 的条件）⇒ `warnings` 恒空；`changed = []`（幂等重复写）⇒ 恒空——`非诱发字段零warning` | 矩阵迭代（4 形态） |
| `upsert_warnings()` · 降噪：`created` 视为全字段 | 边界 | `created = true` 且 `changed = ["kind"]` ⇒ 六条判据均按「全字段」求值（该报的都在）；`created = false` + 同条件 ⇒ 按 `changed` 收窄——`created放行全集` | 矩阵迭代（2 形态） |
| `upsert_warnings()` · 降噪：契约三条归因 | 边界 | `changed = ["path"]` 且 `api` 条件成立 ⇒ 契约三条（`docMissing` / `docNotFound` / `apiUnexpected`）零命中；`changed = ["doc"]` 且 `path` 条件成立 ⇒ `noPath` / `pathNotFound` 零命中——`两类warning归因互不串门` | 矩阵迭代（2 形态） |
| `upsert_warnings()` · 词汇与档位单点 | 异常 | (`noPath` / `pathNotFound` / `docMissing` / `docNotFound` / `apiUnexpected`) 逐值 ∈ (`ArchiDriftKind::ALL` ∪ `ArchiApiFindingKind::ALL`) 的 `as_str` 并集；`severity` 逐条 == `ArchiApiFindingKind::severity().as_str()`（`docNotFound` ⇒ `"error"`，`docMissing` / `apiUnexpected` ⇒ `"warn"`），drift 两值恒 `"warn"`——`kind与severity逐值取自既有单点` | 穷举迭代（5 kind） |
| `upsert_warnings()` · `message` 形态 | 边界 | 每条 message 非空、≤ 200 B、**不含自身 kind 字面量**、含判据句 + 事实 + 下一步；七条齐发时 `warnings.len()` 与判据命中数相等（不重不漏）——`message只写判据句事实与下一步` | 矩阵迭代（2 形态） |
| `delete_warnings()` · 第 7 条 | 正向 | 元素派生契约 md 在盘 ⇒ 恰一条 `apiUnexpected` / `warn`，message 尾句为「一并清理」；md 不在盘 ⇒ 零条——`删除后孤儿契约报一条` | 矩阵迭代（2 形态） |
| `delete_warnings()` · 恒进场 | 异常 | 元素已从模型消失（只传 fqn）仍返回（不查模型、不 panic）；`root` 不存在 / 空模型目录 ⇒ 零条不 panic——`不读模型恒可调用` | 矩阵迭代（2 形态） |
| `delete_warnings()` · 只此一条 | 边界 | 同一 `root` 下同时具备 `path` 空 / `doc` 缺席等的元素被删除 ⇒ 仍只出第 7 条（upsert 六条零命中）；`api = none` + 契约 md 在盘 ⇒ 仍一条（不叠加第 6 条）——`删除动作只允许孤儿契约一条` | 矩阵迭代（2 形态） |
| `EditWarning`（类型） · 值来源 `'static` | 边界 | 六条 upsert + 第 7 条产出的 `kind` / `severity` 与两枚举 `as_str` / `severity().as_str()` 返回值**逐值相等**（同一字面量、非新拼串）；结构无第四字段（`message` 之外的字段面封闭）——`EditWarning零新词表` | 单点 |
| 源码锚 · 零 `drift::check` 与零写盘 | 异常 | `include_str!("warnings.rs")` 内 `drift::check` / `ArchiDriftMode` / `scan::` / `parse_dir` 零命中，`fs::write` / `fs::rename` 零命中（写面 warning 只由零成本事实推导）——`warnings零drift调用零写盘` | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | 真实 `TempDir` + 真实 `api::paths::{contract_path, doc_path}` + 真实 `std::fs`（「在盘 / 不在盘」是被测判据，替身会抹掉它）；`Element` 由真实 `TomlParser::parse` 取得 | 本节全部用例 |
| 两词汇枚举（`ArchiDriftKind` / `ArchiApiFindingKind`） | 不 mock——它们是**被测单点**（kind / severity 的持有者）；用例直接取 `ALL` / `severity()` 与产出对拍 | `kind与severity逐值取自既有单点` |

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/atomic.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/atomic_test.rs

<!-- 挂 AC-13（`write_atomic` 恰一份实现、`.archi-tmp` 后缀与错误文案逐字沿用）。
     条目取自 design「公共函数 / API」子表中「所在文件 = infra/agent/src/sdk/atomic.rs」的 1 行。
     迁移面：`packages/desktop/src-tauri/src/commands/atomic_test.rs` 的既有 7 条用例
     （`写新文件内容逐字且零残留` / `覆盖既有文件为整体替换` / `空内容与超长内容两形态` /
     `无父目录目标err文案可辨` / `特殊目标名临时名不撞目标` / `父目录不存在err且目标未创建` /
     `只读目录err或可写穿零残留`）**逐条迁入本文件**，内容零删减；结构锚用例按新落点改写。
     装置：`tempfile::TempDir`（infra/agent 既有 dev-dependency）+ 真实 `std::fs`。 -->

#### 待测功能

- `write_atomic()`: 唯一落盘实现——同目录 `<name>.archi-tmp` 临时文件 + `fs::rename`；`fs::write` 失败与 `fs::rename` 失败均清临时文件后 `Err`；父目录缺失即 `Err`（不 `create_dir_all`）；两条错误文案逐字沿用壳层原实现

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `write_atomic()` · 写新文件 | 正向 | 目标不存在 + 内容串 ⇒ `Ok`、读回逐字等于入参、同目录零 `*.archi-tmp` 残留——`写新文件内容逐字且零残留` | 既有（迁移） |
| `write_atomic()` · 覆盖既有 | 正向 | 覆盖长内容为短内容 ⇒ 读回逐字等于新内容（整体替换、无半截）——`覆盖既有文件为整体替换` | 既有（迁移） |
| `write_atomic()` · 空与超长内容 | 边界 | 空串 ⇒ `Ok` 且文件为空；1 MB 内容 ⇒ `Ok` 且读回逐字相等——`空内容与超长内容两形态` | 既有（矩阵 2 形态，迁移） |
| `write_atomic()` · 无父目录 | 异常 | 空路径形态（`Path::new("")`）⇒ `Err` 文案含「无父目录」；判据边界按既有注释口径（相对文件名 `parent()` 为 `Some("")`，不造伪失败）——`无父目录目标err文案可辨` | 既有（迁移） |
| `write_atomic()` · 特殊目标名 | 边界 | 目标名含空格 / emoji / 名字本身以 `.archi-tmp` 结尾 ⇒ 临时名不撞目标、写入成功、内容逐字——`特殊目标名临时名不撞目标` | 既有（矩阵 3 形态，迁移） |
| `write_atomic()` · 父目录不存在 | 异常 | 父目录缺失 ⇒ `Err` 文案含「写入临时文件失败」、零残留、目标未被创建（零 `create_dir_all`）——`父目录不存在err且目标未创建` | 既有（迁移） |
| `write_atomic()` · 只读目录 | 边界 | `cfg(unix)` 目录 0o555 ⇒ `Err`；以 root 运行可写穿时只断言零残留 + 目标可读（不伪造失败）——`只读目录err或可写穿零残留` | 既有（迁移） |
| `write_atomic()` · 结构锚 | 异常 | `include_str!("atomic.rs")` 内 `fs::write(&temporary` 恰一处、`fs::rename(&temporary, path)` 恰一处、`create_dir_all` 零命中、`.archi-tmp` 恰一处（后缀单点）——`结构锚唯一落盘点与零建目录` | 单点（改写自壳层原锚） |
| `write_atomic()` · 签名锚 | 边界 | 以函数指针锚 `let f: fn(&Path, &str) -> Result<(), String> = write_atomic;` ⇒ 签名与可见性与壳层委托面逐字一致（改签名即编译失败）——`签名与壳层委托面一致` | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | 真实 `TempDir` + 真实 `std::fs`：原子性（临时文件 + rename）、失败清临时文件、错误文案都是被测语义，替身会抹掉判定对象 | 本节全部用例 |
| 文件权限（只读目录） | 不 mock——`cfg(unix)` 下真实 `0o555` + 既有「root 可写穿则只断言零残留」兜底口径 | `只读目录err或可写穿零残留` |

### packages/desktop/src-tauri/crates/core/archi/src/edit/mod.rs -> packages/desktop/src-tauri/crates/core/archi/src/edit/mod_test.rs

<!-- 挂 AC-3（落点四情形）、AC-4（内联出边四条 + 守恒 + 未列出不动）、AC-5（cascade 单文件
     闭包与跨文件拒 + 删除纯函数唯一）、AC-6（changed / created 求值 + message 形态）、
     AC-7（warnings 接线）、AC-8（counts / model 对拍）、AC-9（hash 可作下一次 expect）。
     条目取自 design「公共函数 / API」子表中「所在文件 = edit/mod.rs」的 8 行（4 `plan_*` +
     4 `apply_*`）与「类型定义」子表的 7 行（`WriteAction` / `RelationSpec` / `DeleteSet` /
     `WriteCounts` / `RelationTally` / `WriteOutcome` / `ElementSpec`）；未做源文件导出扫描。
     既有 12 组用例（三态 / 保序 decor / FQN 七形态 / 非表 / 坏源 / 越界自证 / 幂等零差异 /
     落点四臂 / 父元素缺席 / expect 四形态 / 关系三态 / 结构诊断四拒）**逐条保留**，判据与
     `ElementSpec` 字段名不变者只随返回类型 `ModelWrite → WriteOutcome` 取 `.write`。
     装置：零 mock——真实 `toml_edit`（排版 / decor 保留即被测能力）+ 真实 `TomlParser`
     （自证与回读判据同源）+ 真实 `ModelGraph`（层级 / 邻接单点）+ 真实 `parse_dir`
     （`model` 计数对拍）；临时根沿本文件既有 `TempDir` RAII 先例。 -->

#### 待测功能

- `plan_element_upsert()`: 返回 `WriteOutcome`；增批量四条（同批重复 `to` / `delete` 带字段 / 跨文件内联边 / 端点缺席）、端点存在性前置、落点四情形（既有 / 省略落父 / 单段 FQN 拒 / `file` + `create` 新文件表头）、新文件表头 `# <名>（N 元素 / M 关系）`
- `plan_element_delete()`: 签名 `(root, fqn, cascade: bool, expect: Option<&str>)`——`cascade` 是级联的**唯一承载位**（`ElementSpec` 无同名 / 任何级联字段，D-17）；`cascade = false` 两拒绝臂逐字保留；`cascade = true` 先算单文件闭包（子树 ∪ 出入边）——全在目标文件即删、跨文件 ⇒ `CascadeCrossFile` 点名文件与阻塞项且零文本产出
- `plan_relation_upsert()`: 参数面不变，返回形状扩为 `WriteOutcome`（`warnings` 恒空、`relations` 计一条）
- `plan_relation_delete()`: 同上（`deleted = 1`）
- `apply_element_spec()`: 同签名的行为扩展——元素字段写回后按 `relations` 逐条 upsert / delete 并逐对回读（同一自证链）；`delete` 项带 `desc` / `apis` 与同批重复 `to` 在此面拒绝
- `apply_element_delete()`: 删除纯函数**唯一**形态 `(source, &DeleteSet)`——按元素集删 `[element."…"]` 表、按关系对集删 `[[relation]]` 条目；关系缺席 ⇒ `RelationNotFound`（列该文件现有对）
- `apply_relation_upsert()`: 签名与语义不变（契约面保留）；内联删边与独立删边共用同一判据与文案族
- `apply_relation_delete()`: 同上
- `WriteAction`（类型）: `enum { Upsert, Delete }`、`Default = Upsert`、`ALL: [WriteAction; 2]`、`as_str()`（`"upsert"` / `"delete"`）、`parse(raw) -> Option<Self>`——内联边 `action` 与两工具顶层 `action` 共用一份二值词表
- `RelationSpec`（类型）: `{ to, desc: Option<Option<String>>, apis: Option<Vec<String>>, action: WriteAction }`——三态 + 动作
- `DeleteSet`（类型）: `{ elements: BTreeSet<String>, relations: BTreeSet<(String, String)> }` + `of(fqn)` **单构造子**（零 `is_empty`，D-18）；集合判空由调用方按公开字段 `elements` / `relations` 直接判
- `WriteCounts`（类型）: `{ elements: usize, relations: usize }`（`counts` 与 `model` 共用一形）
- `RelationTally`（类型）: `{ created, updated, deleted, unchanged: usize }`
- `WriteOutcome`（类型）: `{ write: ModelWrite, warnings, changed, created, relations, counts, model }` 七字段
- `ElementSpec`（类型）: 既有字段 + `relations` / `create` 两字段（`Default` 仍成立）；**不带 `cascade`**——级联只作用于删除，承载位是 `plan_element_delete` 的值参（D-17）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `apply_element_spec()` · 内联边更新 | 正向 | 元素既有 + `relations = [{ to: X, desc: Some(Some("新说明")) }]`（该对已在同文件）⇒ 该条 `desc` 变、关系条数不变、其余条目与元素逐字保留、回读命中——`内联边命中走更新` | 单点 |
| `apply_element_spec()` · 内联边新建 | 正向 | `relations = [{ to: 在案元素 }]` 且该对缺席 ⇒ 追加一条 `[[relation]]`（键序 `from` / `to` / `desc` / `apis`）、条数 +1、回读该对存在——`内联边缺席即建` | 单点 |
| `apply_element_spec()` · 内联删边 | 正向 | `relations = [{ to: X, action: Delete }]`（该对在源内）⇒ 条目离场、条数 −1、回读该对缺席、其余条目逐字——`内联删边只动命中条` | 单点 |
| `apply_element_spec()` · 未列出的既有边 | 正向 | 源含 3 条关系而 `relations` 只列 1 条 ⇒ 另 2 条逐字保留在产出（零全量替换、零静默删）——`未列出的边保持不动` | 单点 |
| `apply_element_spec()` · 新建元素带 relations | 正向 | 缺席 FQN + `kind` + `relations` 2 条 ⇒ 同一次产出里元素表与 2 条关系并现、逐对回读、`finish` 自证零诊断——`新建与出边同批写出` | 单点 |
| `apply_element_spec()` · `delete` 项带字段 | 异常 | `[{ to, action: Delete, desc: Some(_) }]` 与 `[{ to, action: Delete, apis: Some(_) }]` 两形态 ⇒ 各 `RelationDeleteHasFields`、零文本产出（返回类型即保证）——`delete项带字段即拒` | 矩阵迭代（2 形态） |
| `apply_element_spec()` · 同批重复 `to` | 异常 | `[X, X]` 与 `[X(delete), X(upsert)]` 两形态 ⇒ `RelationDuplicateInBatch`；facts 含 **1 基下标**、`action`、`to`（如 `relations[2]（action = delete, to = "Backend.Legacy"）`）——`同批重复to即拒且定位到项` | 矩阵迭代（2 形态） |
| `apply_element_spec()` · 坏源与非表形态 | 异常 | 源不可解析 ⇒ `SourceParseFailed` 零文本；`element."App" = "x"` ⇒ `ElementNotTable`（既有用例保留）——`坏源与非表形态零文本` | 既有（维持） |
| `apply_element_spec()` · 既有语义零回归 | 边界 | 三态六位、封闭枚举位、保序 decor、FQN 七形态、越界借自证、`file` / `expect` 非入参旋钮、幂等零差异——既有 12 组用例逐条保留（只把 `ModelWrite` 取值改为 `outcome.write` 形态）——`既有元素写回语义逐条维持` | 既有（维持） |
| `apply_element_delete()` · 元素集 + 关系对集 | 正向 | `DeleteSet { elements: {父, 子1, 子2}, relations: {(a, b)} }` ⇒ 三张元素表与一条关系同批离场、其余内容与注释逐字、表头计数随动下降——`集合删除一把落净且计数随动` | 单点 |
| `apply_element_delete()` · 关系对缺席 | 异常 | `relations` 含文件中不存在的对 ⇒ `RelationNotFound`，facts 列该文件现有 `from → to` 对、零文本产出——`关系对缺席列现有对` | 单点 |
| `apply_element_delete()` · 元素缺席 | 异常 | `elements` 含不在案的 FQN ⇒ `ElementNotFound`，facts 列该文件现有元素；缺 `element` 表 ⇒ `ElementTableMissing`——`元素缺席列现有元素` | 矩阵迭代（2 形态） |
| `apply_element_delete()` · `DeleteSet` 退化形态 | 边界 | `DeleteSet::of(fqn)` ≡ 单元素 + 零关系（单元素删除的唯一表达）；公开字段判空两臂：`DeleteSet::default()` 两集合皆空 / `of(fqn).elements.len() == 1` 且 `of(fqn).relations.is_empty()`（类型面零 `is_empty` 方法）；行为面 `apply_element_delete(source, &DeleteSet::of(fqn))` 与既有单元素删除产出逐字节相同——`单元素删除是集合退化形` | 矩阵迭代（2 形态） |
| `apply_relation_upsert()` / `apply_relation_delete()` · 契约面零回归 | 边界 | 既有关系三态 / 同源重复 `RelationDuplicated` / 关系缺席 `RelationNotFound` 用例逐条保留；与内联路径同一 `relation_duplicated_in_source` 文案族（同一 `EditErrorKind`、同一 facts 形态）——`内联与独立关系写共用判据` | 既有（维持） |
| `plan_element_upsert()` · 落点四情形 | 正向 | ① 既有元素 ⇒ `write.file == root.join(声明文件)`；② 新建省略 `file` ⇒ 落**父元素**声明文件；③ 新建 + `file` 缺席 + 无 `create` ⇒ `Err` 提示 `create = true`；④ 新建 + `file` + `create = true`（文件不存在）⇒ 产出的 `content` 首行为 `# <file 去 .toml>（1 元素 / 0 关系）`——`落点四情形逐条成立` | 矩阵迭代（4 形态） |
| `plan_element_upsert()` · 单段 FQN 省略 `file` | 异常 | 新建单段 `App`（无父）省略 `file` ⇒ `TargetInvalid` 并点名需给 `file`（新分层落点是人判断）——`单段fqn省略file即拒` | 单点 |
| `plan_element_upsert()` · 既有元素换 `file` | 异常 | 既有元素 + 不同 `file` ⇒ `TargetInvalid`（facts 点名现声明文件与给定值，「迁文件属独立语义」）；同值 `file` ⇒ `Ok`——`既有元素不接受改落点` | 矩阵迭代（2 形态） |
| `plan_element_upsert()` · 批量四条 | 异常 | 同批重复 `to` ⇒ `RelationDuplicateInBatch`（1 基下标 + `action` + `to`）；`delete` 项带 `desc` / `apis` ⇒ `RelationDeleteHasFields`；内联边已声明在**别的模型文件** ⇒ `RelationOwnedElsewhere` 点名该文件并指向 `archi_relation`；内联 `to` 端点不在案 ⇒ `RelationEndpointMissing`——`批量四条各点名去向` | 矩阵迭代（4 形态） |
| `plan_element_upsert()` · 四计数守恒 | 正向 | `created + updated + deleted + unchanged == relations.len()`（混合批次：1 新建 + 1 改 + 1 删 + 1 同值）；不带 `relations`（与 `relations: []` 两形态）⇒ 四项皆 `0`（不是省略）——`四计数守恒且空批次归零` | 矩阵迭代（3 形态） |
| `plan_element_upsert()` · `changed` / `created` 幂等 | 正向 | 同值重复 upsert（第二次）⇒ `created = false`、`changed = []`、`content` 与盘上逐字节相等；首次新建 ⇒ `created = true`、`changed` ⊆ 实际给出的字段名；只改 `summary` ⇒ `changed == ["summary"]`——`changed空即幂等` | 矩阵迭代（3 形态） |
| `plan_element_upsert()` · `counts` 与 `model` 对拍 | 正向 | `outcome.counts` == 写后 `TomlParser::parse(&content)` 的 `elements` / `relations` == 产出文本表头括号内两数字；`outcome.model` == 写后 `parse::parse_dir(root)` 的 `model.elements` / `model.relations`（前值 ± 增量；drift 报告同源，数值同一）——`counts与表头与model三处同值` | 单点 |
| `plan_element_upsert()` · `warnings` 接线 | 正向 | 元素写回后 `outcome.warnings` 与 `upsert_warnings(root, 写后元素, &outcome.changed, outcome.created)` 逐条相等（`kind` / `severity` / `message` 三维）；plan 层零自拼 warning——`warnings由模块单点产出` | 单点 |
| `plan_element_upsert()` · 既有前置次序 | 边界 | 结构诊断 / 缺 `kind` / 在案 + `kind` / `expect` 四形态（含文件缺席 + `Some(_)`）/ 父元素缺席 / 源读失败 ——既有判据次序逐条保留，新增内联检查不抢先（同批多个违例时先后固定）——`既有判据次序不被新检查打乱` | 既有（维持 + 扩） |
| `plan_element_delete()` · `cascade = false` 两拒绝臂 | 异常 | 有直接子元素 ⇒ `DeleteHasChildren` 点名各子 FQN；被任一关系引用（出边 / 入边）⇒ `DeleteHasRelations` 点名关系对；facts 与 P3 逐字一致——`cascade缺省保留两拒绝臂` | 矩阵迭代（2 形态） |
| `plan_element_delete()` · `cascade = true` 同文件闭包 | 正向 | 目标元素有 2 层子树 + 出入边全在同一文件 ⇒ 闭包内元素与关系全删净、`content` 无闭包内 FQN、表头计数随动；`changed = []` / `created = false` / `relations` 四项皆 `0`（D-7）——`单文件闭包删净` | 单点 |
| `plan_element_delete()` · `cascade = true` 跨文件闭包 | 异常 | 闭包含另一模型文件声明的子元素或某条边声明在别的文件 ⇒ `CascadeCrossFile` 点名各文件与阻塞项、零文本产出、盘上零变化——`跨文件闭包拒并点名` | 单点 |
| `plan_element_delete()` · 闭包边界 | 边界 | 叶元素（无子无边）`cascade = true` 与 `false` 的 `content` 逐字节相同；`cascade = true` 而元素缺席 ⇒ `ElementNotFound`（闭包不绕过存在性）——`无子无边时cascade与缺省同文本` | 矩阵迭代（2 形态） |
| `plan_relation_upsert()` / `plan_relation_delete()` · 薄形 `WriteOutcome` | 正向 | 两入口返回 `WriteOutcome`：`warnings` 恒空、`relations` 计一条（upsert 新对 ⇒ `created = 1`；同值 ⇒ `unchanged = 1`；delete ⇒ `deleted = 1`）；`changed` ⊆ {`desc`, `apis`}（delete 为 `[]`）；`write.file` 与既有落点规则逐字一致——`关系写薄形四计数取一` | 矩阵迭代（3 形态） |
| `plan_*` · `hash` 与 `expect` 闭环 | 正向 | `outcome.write.hash` == `drift::file_hash_of(&file, &content)` == 落盘后 `drift::file_hash(&file)`；把该值作下一次 `expect` 再调 ⇒ `Ok`；改文件后旧 `expect` ⇒ `ExpectMismatch` 且零文本——`写后哈希可作下一次expect` | 单点 |
| `WriteAction`（类型） | 边界 | `ALL.len() == 2`；`as_str` 逐值 `"upsert"` / `"delete"`；`parse` 往返（含大小写 / 空白不认 ⇒ `None`）；`Default::default() == Upsert`；非法值 ⇒ `None`（E1 据此拒）——`动作二值词表闭合` | 表驱动 |
| `RelationSpec` / `DeleteSet` / `WriteCounts` / `RelationTally`（类型） | 边界 | `RelationSpec { desc: None / Some(None) / Some(Some(_)) }` 三态可构造；`DeleteSet` 两集合按 `BTree` 序；`of(fqn)` 单构造子 + 公开字段判空两臂（`default()` 两集合空 / `of(fqn)` 非空）；`WriteCounts` 两字段可构造（`counts` 与 `model` 共用一形）；`RelationTally` 全 0 可构造——`新增类型形状齐备` | 表驱动 |
| `WriteOutcome`（类型） | 边界 | 七字段可构造；内嵌 `write: ModelWrite` 的三字段与 P3 逐字同义（`file` / `content` / `hash`）；`PartialEq` / `Debug` 可对拍（零 IPC 出线）——`WriteOutcome内嵌ModelWrite不变` | 单点 |
| `ElementSpec`（类型） | 边界 | 既有字段 + `relations` / `create` 两字段，`#[derive(Default)]` 使 `ElementSpec { fqn, ..Default::default() }` 仍成立；两新字段缺省分别 `[]` / `false`；字段面封闭——全字面构造（无 `..Default`）不带 `cascade` 即编译通过，若把级联开关加回 spec 即红——`ElementSpec缺省扩展不破既有简写` | 单点 |
| 源码锚 · 删除纯函数唯一 | 异常 | `include_str!("mod.rs")` 中 `fn apply_element_delete` 恰一处；删元素表的 `remove` 调用只在该函数内（零 `apply_element_delete_one` / 零第二份单元素删除路径）；`fn is_empty` 零命中（`DeleteSet` 零测试专用方法，D-18）；`Sha256` / `sha2` / `fs::write` 零命中（零哈希拼装、零写盘）——`删除实现恰一个且零写盘` | 单点 |
| 源码锚 · 零 `drift::check` | 异常 | `include_str!("mod.rs")` 内 `drift::check` / `ArchiDriftMode` / `scan::` 零命中（`file_hash_of` 与 `report` 的词汇 / `severity` 消费是唯一 drift 依赖面）——`edit零drift判定调用` | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | 真实 `toml_edit` + 真实 `TomlParser` / `parse_dir` + 真实 `ModelGraph` + 真实 `TempDir` 落文件；渲染 / 自证 / 回读 / 表头回填是被测语义，替身会抹掉「被自证的文本 == 交付文本」这一判据 | 本节全部用例 |
| 注入 `expect`（显式入参） | 不 mock——`expect` 是被测 API 的显式入参；取真实 `drift::file_hash_of` / `file_hash` 值 | `写后哈希可作下一次expect` |
| `upsert_warnings` / `delete_warnings`（同 crate 兄弟模块） | 不 mock——同一 crate 内的真实组合（接线本身即被测：plan 层零自拼 warning） | `warnings由模块单点产出` |

### packages/desktop/src-tauri/crates/core/archi/src/edit/messages.rs -> packages/desktop/src-tauri/crates/core/archi/src/edit/messages_test.rs

<!-- 挂 AC-6（文案族封闭 / message 形态）、AC-4 与 AC-5 的判据落地（四个新变体的判据句）。
     条目取自 design「类型定义」子表的 `EditErrorKind` 一行（19 → 23）；「公共函数 / API」
     子表无本文件行（`EditError::message` 是既有面，语义不变）——故本节以类型行为被测面，
     不新造函数条目。
     既有 19 值对拍与逐值互异用例保留，计数与 needle 表随 23 扩。 -->

#### 待测功能

- `EditErrorKind`（类型）: 19 → **23** 值（+ `RelationDeleteHasFields` / `RelationDuplicateInBatch` / `CascadeCrossFile` / `RelationOwnedElsewhere`）；`ALL: [EditErrorKind; 23]` 与 `criterion()` 穷尽分支同批扩；既有 19 条判据句逐字不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `EditErrorKind` · `ALL` ↔ `criterion()` 对拍 | 正向 | `ALL.len() == 23`；遍历 `ALL` 逐值调用 `criterion()` 非空；`criterion()` 的 `match` 分支集合与 `ALL` 逐值相等（新增变体未同批补 `ALL` 即红）——`二十三值穷尽对拍` | 穷举迭代（23 值） |
| `EditErrorKind` · 四个新变体在场 | 正向 | `RelationDeleteHasFields` / `RelationDuplicateInBatch` / `CascadeCrossFile` / `RelationOwnedElsewhere` 逐值在 `ALL` 中且 `criterion()` 非空——`四新变体在册` | 穷举迭代（4 值） |
| `EditErrorKind` · 逐值互异与操作中立 | 异常 | 23 条判据句两两互异；零「改档」/「本操作」/「本工具」一类操作专属词；零中文译名（`orphanHierarchy` 等诊断词不译）——`判据句互异且操作中立` | 矩阵迭代（2 面） |
| `EditErrorKind` · 既有 19 条逐字不变 | 异常 | 既有 19 句 needle 表逐句在场（`模型存在结构诊断，拒绝写回` … `回读不一致`）；零旧句被改写——`既有判据句零改写` | 穷举迭代（19 句） |
| `EditErrorKind` · 文案族出口唯一 | 边界 | `EditError::message()` 恒为「判据句：事实」（恰一个全角 `：` 分隔）；`include_str!("messages.rs")` 内 `impl Display for EditError` / `impl std::error::Error for EditError` 零命中（第二条渲染路径会破坏「变体集合 == 文案集合」）——`渲染出口唯一` | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | 纯数据枚举与字符串比对，零 IO、零临时目录 | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/archi/src/lib.rs -> packages/desktop/src-tauri/crates/core/archi/src/edit/mod_test.rs

<!-- design.md 未声明该文件的公共函数 / API 变更（「公共函数 / API」子表无本文件行）；
     本文件是纯出口整理（design「修改文件」表第 3 行：`pub mod edit` 出口补 6 个类型 +
     新增 `warnings` 子模块出口 3 名；crate 根 doc 五条纪律零改动）。故「待测功能」只登记
     **文件级出口面**（无函数定义），MUST NOT 以 grep 导出声明虚构函数条目；
     测试文件归属沿归档 `archi-edit-engine-primitives` 先例（本 crate 无 `lib_test.rs` 惯例，
     出口面锚经 `include_str!("../lib.rs")` 落在 `edit/mod_test.rs`）。 -->

#### 待测功能

- `lib.rs 出口面`（文件级被测面，无函数定义）: `pub use edit::{…}` 含 6 个新类型（`WriteAction` / `RelationSpec` / `DeleteSet` / `WriteOutcome` / `WriteCounts` / `RelationTally`）；`warnings` 子模块出口含 3 名（`EditWarning` / `upsert_warnings` / `delete_warnings`）；既有 12 名（8 函数 + 4 类型）未丢
- `lib.rs 头注释与纪律`（文件级被测面）: crate 根 doc 的五条纪律零改动（本变更不触组织性约束）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| crate 根出口面 · `edit` 组新增 6 名 | 正向 | `include_str!("../lib.rs")` 的 `pub use edit::{` 分组块含 `WriteAction` / `RelationSpec` / `DeleteSet` / `WriteOutcome` / `WriteCounts` / `RelationTally` 六名逐个在场——`crate根出口含六个写面类型` | 穷举迭代（6 名） |
| crate 根出口面 · `warnings` 三名 | 正向 | `EditWarning` / `upsert_warnings` / `delete_warnings` 三名经 crate 根可达（`use crate::{EditWarning, upsert_warnings, delete_warnings}` 编译通过）——`warnings三名经crate根可达` | 穷举迭代（3 名） |
| crate 根出口面 · 既有 12 名未丢 | 异常 | 8 函数（4 `apply_*` + 4 `plan_*`）与 4 类型（`EditError` / `EditErrorKind` / `ElementSpec` / `ModelWrite`）逐名仍在；`#[cfg(test)] mod warnings_test;` 登记恰一处（幽灵声明或漏声明即红）——`既有出口零丢且新测试注册在册` | 穷举迭代（12 名 + 1 声明） |
| crate 根头注释 · 五条纪律零改动 | 边界 | crate 根 doc 五条纪律逐句在场（本变更不引入新的组织性约束）；零「无专用写工具」旧立场句（AC-12a 的窄面）——`头注释纪律零改动` | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | `include_str!("../lib.rs")` 取编译期内嵌字节（禁测试内复制副本、禁读盘相对路径）；名字清单以 `use` 面为准，不手抄第二份出口表 | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/archi/src/dto.rs -> packages/desktop/src-tauri/crates/core/archi/src/dto_test.rs

<!-- 挂 AC-9（`fileHash` 与 `drift::file_hash` 逐字相等 / 与 `modelHash` 不可比）、AC-15 的
     类型形状面（`ArchiModuleDetail` 末尾一字段）。
     条目取自 design「公共函数 / API」子表的 `module_detail` 一行（修改，同签名、出参增
     `file_hash`）与「类型定义」子表的 `ArchiModuleDetail` 一行。
     装置：真实合成模型 + 真实 `drift::file_hash`（`modelFile` 已知 ⇒ 零额外读盘，故
     直接用真实盘面），沿本文件既有 `module_detail*` 用例族装置。 -->

#### 待测功能

- `module_detail()`: 同签名；出参增 `file_hash`——以 `modelFile` 定位后调 `drift::file_hash`（同一函数同一 `sha256:` 前缀，零第二份哈希实现、零额外读盘）
- `ArchiModuleDetail`（类型）: 末尾追加 `file_hash: String`（`sha256:<hex>`）；其余七字段声明序不变；薄概览（`ArchiElementBrief`）不带

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `module_detail()` · `fileHash` 取值单点 | 正向 | 单元素响应的 `file_hash` == `drift::file_hash(&root.join(元素声明文件))` 逐字相等；形态为 `sha256:` + 64 位小写 hex——`fileHash等于声明文件的file_hash` | 单点 |
| `module_detail()` · 跨元素同异文件 | 正向 | 同文件声明的两元素 ⇒ 两响应 `file_hash` 相等；异文件两元素 ⇒ 不等（哈希按文件，不按元素）——`fileHash按文件不按元素` | 矩阵迭代（2 形态） |
| `module_detail()` · 内容敏感 | 边界 | 对该文件写回一次（gate 改档）后重取 ⇒ `file_hash` 与改前不等；文件字节复原后与改前相等（同内容同值）——`fileHash随内容变化` | 矩阵迭代（2 形态） |
| `module_detail()` · 与 `modelHash` 不可比 | 异常 | `detail.file_hash != report.model_hash`（`archi_drift` 全量报告的聚合值）；反向断言：MUST NOT 写等值断言（同名不同义 ⇒ 恒不匹配的假冲突陷阱）；两者只共享 `sha256:` 前缀与长度——`不可与聚合modelHash互比` | 单点 |
| `module_detail()` · 异常两臂不带该字段 | 异常 | 元素缺席 ⇒ `Err` 文案沿用既有（含「元素不存在」与元素总数）；结构诊断非空 ⇒ `Err`（诊断全量形）——两臂都零响应体，`file_hash` 无产出面——`异常面零响应体` | 矩阵迭代（2 形态） |
| `module_detail()` · 字段声明序与薄面 | 边界 | `ArchiModuleDetail` 的 `file_hash` 追加在 `resolved_entry` 之后（声明序末位，D-9）；`ArchiElementBrief` 不带该字段（薄概览零扩宽）——`新字段追加表尾且薄面不带` | 单点 |
| `module_detail()` · 既有投影零回归 | 边界 | 既有 `module_detail` 七字段投影用例（邻接 / `doc` 单值 / `api` 并列 / `resolvedEntry` 三字段）逐条保留且逐字段相等——`既有投影面零变化` | 既有（维持） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | 真实 `TempDir` 模型文件 + 真实 `drift::file_hash`（哈希函数是被测单点，替身会抹掉「同一函数同值」判据） | 本节全部用例 |
| `archi_drift` 报告（`modelHash` 对照臂） | 不 mock——用真实 `drift::check`（Full）取聚合值一次；该值只作「不等」的反向对照，不作算术 | `不可与聚合modelHash互比` |

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount_test.rs

<!-- 挂 AC-1（18 项清单 / 两写档位 / 五只读不变）、AC-10（八行上限与 `enum` 派生 / 限额单点）、
     AC-12a 窄面（`archi_scaffold` 常量 doc 的旧立场句改写）。
     条目取自 design「类型定义」子表的 `WriteLimits` 一行（新增）+「修改文件」表第 5 行的
     ①–⑤ 五条改动面（两常量 / 两份定义 / 清单 16 → 18 / doc 项数措辞 / scaffold doc）；
     「公共函数 / API」子表无本文件行（两 `*_definition()` 与两常量为 `pub(crate)`，
     非公共 API），故以**文件级挂载面**登记，不虚构 `pub fn` 条目。
     机械随动两处（不新开章节，落本节的用例行内标注）：
     - `packages/desktop/src-tauri/crates/infra/agent/src/sdk/policy_test.rs`（准入档封闭集）；
     - `packages/desktop/src-tauri/crates/infra/agent/src/sdk/config_propose_test.rs`（末项计数）。
     装置：既有 `registry()` / `lookup()` / `definitions()` 真值 + `source_of("src/sdk/mount.rs")`
     源码锚 helper（既有）。 -->

#### 待测功能

- `WriteLimits` / `WRITE_LIMITS`（类型 + 常量）: 十二字段上限（`fqn` 256 / `path_items` 32 / `path_item` 1024 / `metadata_items` 32 / `metadata_key` 64 / `metadata_value` 512 / `relations` 64 / `relation_apis` 32 / `relation_api` 128 / `text` 512 / `pointer` 1024 / `file` 256）——schema 键值与执行体硬闸共用同一组常量（数字单点）
- 挂载面（文件级，无函数定义）: 两常量 `ARCHI_ELEMENT_TOOL = "archi_element"` / `ARCHI_RELATION_TOOL = "archi_relation"`；静态清单 16 → **18**（两写项 `ToolTier::Write` / `ToolExec::Archi`，插在 `archi_scaffold` 之后、`db_tables` 之前）；两份平铺 schema 定义（逐字段 `description`、`enum` 由 `ALL` 派生、上限取自 `WRITE_LIMITS`）；`Vec::with_capacity(18)` 与两段 doc 的项数措辞；`archi_scaffold` 常量 doc 的写面句改写

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 挂载面 · 18 项名字与序 | 正向 | `EXPECTED_NAMES: [&str; 18]` 与 `registry()` 逐值相等；序 = 七内建 → `create_explore_note` → 五 archi 只读 → **两 archi 写** → 两 DB → `propose_config`；`names.len() == 18` 且 `unique.len() == 18`——`十八项名字唯一且有序` | 既有（改计数 + 插两值） |
| 挂载面 · 两写工具档位 | 正向 | `lookup("archi_element")` / `lookup("archi_relation")` 命中且 `tier == ToolTier::Write`、`exec == ToolExec::Archi`；`definitions(&[两名])` 按清单序出两份定义——`两写工具写档archi执行体` | 矩阵迭代（2 名） |
| 挂载面 · 五只读档位零变化 | 异常 | 五 archi 只读项 `tier == ToolTier::Read` / `exec == ToolExec::Archi` 逐项逐字不变；`create_explore_note` / 两 DB / `propose_config` 的档位与执行体零变化——`只读与提案档位零变化` | 穷举迭代（5 + 4 项） |
| 挂载面 · 三档准入等价（落 `sdk/policy_test.rs`） | 异常 | 封闭集 `match name` 的写档臂加 `"archi_element"` 与 `"archi_relation"`（与 `write` / `edit` / `create_explore_note` 同臂 `[false, true, true]`）；只读臂 `[true, true, true]` **不得**添这两名；两处 `assert_eq!(…, 16)` → 18——`准入封闭集随清单扩` | 穷举迭代（18 项） |
| 挂载面 · 末项仍提案工具（落 `sdk/config_propose_test.rs`） | 边界 | `names.len() == 18`（断言串「十八项」随动）；`names.last() == Some(PROPOSE_CONFIG_TOOL)` 逐字不动；`PROPOSE_CONFIG_TOOL` 恰一处——`末项仍为提案工具` | 既有（改计数） |
| `WriteLimits` · schema 上限对拍 | 正向 | 两份定义 schema 的 `maxLength` / `maxItems` 逐字段 == `WRITE_LIMITS` 对应字段（12 项）；`enum` 逐字段 == `ElementKind::ALL`（4）/ `GateTier::ALL`（3）/ `ApiStatus::ALL`（2）/ `WriteAction::ALL`（2）的 `as_str` 序——`schema上限与enum全由常量派生` | 穷举迭代（12 上限 + 4 枚举） |
| `WriteLimits` · 数字单点 | 边界 | `include_str!("mount.rs")` 内十二个限额数字各只出现在 `WRITE_LIMITS` 构造处（定义面描述串零裸数字）；`WRITE_LIMITS` 声明恰一处——`限额数字单点` | 单点 |
| 挂载面 · 描述面零形态长表 | 边界 | 两份 `description` 零字段形态教学（`kind = ` / `[[relation]]` / `path = [` / `metadata = {` 一类示例零命中）；各含「原子写 / 坏文本不可产出」「`expect` ↔ `hash` 闭环」「`cascade` 只同一模型文件内」三事——`描述不复述字段形态长表` | 矩阵迭代（2 份定义） |
| 挂载面 · 结构锚随动 | 异常 | `source_of("src/sdk/mount.rs")` 内 `Vec::with_capacity(18)` 恰一处、「十八项静态挂载清单」恰一处、「十八个字面构造项按序」恰一处；`ask_user_question 不在十八项静态清单` 措辞随动——`源码结构锚按字面计数随动` | 单点 |
| 挂载面 · scaffold doc 旧立场句 | 异常 | `ARCHI_SCAFFOLD_TOOL` 的常量 doc 由「不在专用写工具之列」改写为「模型写面是两个写工具，本工具只出模板」；「无专用写工具」/「不设专用写工具」/「MUST NOT 存在专用写」三串在 `mount.rs` 零命中——`scaffold文档句改写` | 单点 |
| 挂载面 · 未知名与重复登记 | 边界 | `lookup("archi_elementx")` / `lookup("")` ⇒ `None`；18 项名字两两不同（零重复登记）——`未知名查空且零重复` | 矩阵迭代（2 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | `registry()` / `lookup()` / `definitions()` 为纯构造面（`OnceLock` 惰性初始化），零 IO；源码锚经 `include_str!` | 本节全部用例 |
| `ToolTier` / `ToolExec` 的准入判定 | 不 mock——`policy_test.rs` 的封闭集等价表是**被测机械守卫本体**（`policy.rs` 实现零改动，测试随清单随动） | `准入封闭集随清单扩` |

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool_test.rs

<!-- 挂 AC-6（三通道 / 幂等 / Err 第一行自足）、AC-8（响应体量）、AC-9（expect 往返组合）、
     AC-10（E1 硬闸 + 值域查表单源）、AC-12a 窄面（`SCAFFOLD_TEMPLATE` 首行 + `scaffold_payload`
     doc 口径）、AC-13 的工具半边、AC-14（保护闸拒写）。
     条目取自 design「内部接线面」子表的四行（`ArchiContext::new` 不变 / `with_protect` 新增 /
     `execute` 修改 / `domain_value` 新增）与「类型定义」子表的三行（`ElementWriteInput` /
     `RelationWriteInput` / `ArchiContext` 增 `protect`）；「公共函数 / API」子表无本文件行（两工具
     是 agent 工具，不是 IPC 命令；`execute` 是 `pub(crate)`）。
     `scaffold_payload` 的 doc 口径与 `SCAFFOLD_TEMPLATE` 首行同属 D-19 ② 的被测面（不是函数条目，
     归本节的文件级被测面）。
     装置：既有 `tempfile` 合成根 + `healthy()` / `write_model()` / `write_file()` /
     `execute_json()` / `snapshot()` helper；`ProtectRules` 由真实用户配置
     （`.dev-team/config.json` 的 `write_protection.files`）经 `ProtectRules::load` 构造后
     `with_protect` 注入（不 mock 规则集）。 -->

#### 待测功能

- `ArchiContext::execute()`: 增第六、七臂——`archi_element` / `archi_relation`；落盘链固定「E1 解析与硬闸 → E2 诊断前置 → `plan_*` → `ProtectRules::evaluate_target` → `atomic::write_atomic` → 响应装配 → `cap_result`」
- `ArchiContext::with_protect()`: 装配侧注入写保护规则集（`session_pump` 一处接线）；`new()` 不变（`protect = None`）
- `domain_value()`（文件内私有，design「内部接线面」表一行）: `fn domain_value<T: Copy>(raw: &str, name: &str, all: &[T], as_str: impl Fn(T) -> &'static str) -> Result<T, String>`——值域查表唯一核：E1 的 `optional_domain`（`kind` / `gate` / `api` 三处）与 `archi_scaffold` 的 `kind` 同调；判据句唯一形态 `{name} 非法值 "{raw}"（合法值：…）`，合法值清单由 `ALL` 现算；零第二份 `ElementKind::ALL` 查表、零「非法 kind: …」旧文案（D-19 ①）
- `ElementWriteInput` / `RelationWriteInput`（类型）: 两工具的 E1 解析产物（私有）；字段面见 design 类型定义表（`ElementWriteInput.cascade` 保留，承载 `plan_element_delete` 的值参直供，D-17）
- `ArchiContext`（类型）: 增私有字段 `protect: Option<ProtectRules>`
- `scaffold_payload` 的写面口径（文件级被测面）: 函数 doc 与模块头、`SCAFFOLD_TEMPLATE` 首行同口径——落盘走 `archi_element` / `archi_relation`（工具表达不了的才手改 `write` / `edit`）；旧句「写盘仍走既有 `write` / `edit`」零残留（D-19 ②）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `execute()` · 七臂分派 | 正向 | 五只读臂响应逐字不变（`archi_module` / `archi_search` / `archi_api` / `archi_drift` / `archi_scaffold` 既有用例保留）；两写臂返回 JSON 串；未知名 ⇒ `Err`（既有文案）——`七臂分派且未知仍err` | 既有（维持）+ 新两臂 |
| `execute()` · `archi_element` 成功响应面 | 正向 | upsert 新建 ⇒ JSON 含 `warnings`（数组）/ `action = "upsert"` / `element` / `file`（repo 相对 POSIX）/ `hash`（`sha256:`）/ `created = true` / `changed`（数组）/ `relations` 四计数 / `counts` / `model` 两计数；盘上文件字节 == `plan_element_upsert` 的 `content`——`元素写响应三通道齐备且落盘同文本` | 单点 |
| `execute()` · `archi_relation` 薄形 | 正向 | upsert ⇒ JSON 含 `warnings`（**恒空数组**）/ `action` / `from` / `to` / `file` / `hash` / `created` / `changed` ⊆ {`desc`, `apis`}；零 `counts` / `model` / `relations` 键——`关系写薄形无计数` | 单点 |
| `execute()` · E1 值域硬闸 | 异常 | `action` 非 `upsert`/`delete`；`kind` / `gate` / `api` 值集外；`element` 空 / 空白 / 超 256；`relations[].action` 越界 ⇒ 各 `Err` 点名参数与合法值（`ALL` 派生）；多处违例 ⇒ **一次报全**（`；` 分隔）——`E1值域一次报全` | 矩阵迭代（6 形态） |
| `execute()` · E1 上限硬闸 | 异常 | `path` 33 项 / 单项 1025 B；`metadata` 33 项 / 键 65 B / 值 513 B；`relations` 65 条 / `apis` 33 项 / 单项 129 B；`summary` 513 B；`entry` 或 `doc` 1025 B；`file` 257 B ⇒ 各 `Err` 点名字段与上限（数字取自 `WRITE_LIMITS`）——`E1上限逐字段拒` | 穷举迭代（9 上限） |
| `ElementWriteInput` / `RelationWriteInput`（类型） · E1 解析与跨字段互斥 | 异常 | 经 `execute` 可达的解析产物字段面：`path` 非数组 / 元素非字符串；`metadata` 非对象 / 值非字符串；`relations` 非对象数组 / 项缺 `to`；`cascade = true` 配 `action = "upsert"` ⇒ 各 `Err`；`action = "delete"` 时字段面参数不生效（`properties` 描述明写）——`E1类型与跨字段互斥` | 矩阵迭代（5 形态） |
| `execute()` · E2 诊断前置 | 异常 | 模型含 `cycle` / `orphanHierarchy` 诊断 ⇒ 两写臂各 `Err`，文案 == `dto::graph_diagnostics_error` 全量形（含种类与 `file:line`）、零落盘（`snapshot` 前后全等）——`结构诊断前置零落盘` | 矩阵迭代（2 入口） |
| `execute()` · `plan_*` 错误映射 | 异常 | 元素缺席 / 落点非法 / `expect` 不匹配 / 批量四条违例 ⇒ `Err` 串 == 对应 `EditError::message()`（判据句 + 全角 `：` + 事实），零自造文案；首行含 `重试：` / `下一步：` / `改用 archi_relation` 之一或可复制参数片段（第一行自足）——`写面err即message且第一行自足` | 矩阵迭代（4 形态） |
| `execute()` · 保护闸拒写 | 异常 | 用户规则 glob 命中**解析后的目标文件** ⇒ `Err` 为既有族拒串（`%s` / `%t` 已替换为路径与工具名）、零落盘、零临时文件残留；未命中 ⇒ 正常写入——`命中用户规则即拒写` | 矩阵迭代（2 形态） |
| `execute()` · `protect` 未注入 | 异常 | `ArchiContext::new(root)` 直接调写臂 ⇒ `Err` 并声明「生产不可达」（不静默放行）——`未注入规则集不静默放行` | 单点 |
| `execute()` · `warnings` 恒在场与幂等 | 边界 | 干净写（无待报事实）⇒ `warnings == []`（键在场、零省略）；同值重复写 ⇒ `changed == []`、`created == false`、盘上字节与首次后逐字节相等、两次 `hash` 相同——`warnings恒在场且幂等零字节变化` | 矩阵迭代（2 形态） |
| `execute()` · 响应体量上界与 `cap_result` | 边界 | 七条 warning 齐发 + 最大字段面入参的成功响应 ⇒ 序列化长度 < `tools::MAX_RESULT_BYTES`（出口仍过 `tools::cap_result`，未触达截断）；不设键序断言（结构上界 ≈ 2 KB，截断不可能命中）——`成功响应永不触达截断` | 单点 |
| `execute()` · 脏入参不 panic | 边界 | `null` / 数字 / 嵌套对象 / 超长键的入参 ⇒ `Err` 或成功，零 panic；`archi_element` 的 `relations: null` 与省略同义——`脏入参不panic` | 矩阵迭代（4 形态） |
| `execute()` · 乐观锁往返（组合：`archi_module` → `archi_element` → 响应 `hash`） | 正向 | `archi_module(element)` 取 `fileHash` → `archi_element { expect: fileHash }` ⇒ 成功 → 响应 `hash` 作下一次 `expect` ⇒ 再次成功（连续两次写同一文件零额外读）；两次响应 `hash` 与盘面 `drift::file_hash` 同值——`fileHash到expect到hash闭环` | 单点 |
| `execute()` · 旧 `expect` 与落点后置 | 异常 | 先写一次再以**旧** `expect` 写 ⇒ `ExpectMismatch` + 盘上零变化；写臂在 `plan_*` 之后才判保护闸（落点未知前不判）⇒ 非法落点的 `Err` 来自 `plan_*` 而非保护串——`旧expect拒且保护闸后置` | 矩阵迭代（2 形态） |
| `execute()` · `SCAFFOLD_TEMPLATE` 首行改写 | 异常 | `include_str!("archi_tool.rs")` 的模板**首行**指向两写工具（零「写盘仍用既有 write / edit」旧句）；`archi_scaffold` 的 `draft` 字段面与既有用例逐条不变——`草稿模板改指两写工具` | 单点 |
| `scaffold_payload` · 三处口径一致（D-19 ②） | 异常 | `include_str!("archi_tool.rs")` 内「写盘仍走既有」/「写盘仍用既有」/「无专用写工具」零命中；模块头、`scaffold_payload` 函数 doc、`SCAFFOLD_TEMPLATE` 首行**三处**各含 `archi_element` 与 `archi_relation`（同一口径；`write` / `edit` 只在「工具表达不了的才手改」句中出现）——`写面口径三处同源` | 单点 |
| `domain_value()` · 两入口同判据句（D-19 ①） | 正向 | 同一非法 `kind` 值经 `archi_element`（E1）与 `archi_scaffold`（草稿）两入口 ⇒ `Err` 串**逐字相同**（`kind 非法值 "<raw>"（合法值：…）`，合法值序由 `ElementKind::ALL` 现算）；`gate` / `api` 两字段在 E1 面同句式；`archi_scaffold非法kind报错点名四值` 判据不变（四值仍逐个在串内）——`值域判据句单条且两入口同文` | 矩阵迭代（3 字段 + 2 入口） |
| `domain_value()` · 源码单源锚 | 异常 | `include_str!("archi_tool.rs")` 内 `fn domain_value` 恰一处；`ElementKind::ALL` 的查表只在该私有核内（`scaffold_payload` 与 `optional_domain` 零直接查表）；「非法 kind:」零命中（旧文案不留）——`值域查表一处` | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock（主面） | 真实 `TempDir` 合成模型 + 真实 `archi::edit` + 真实 `write_atomic` + 真实 `TomlParser` / `drift::file_hash`：落盘链与字节相等是被测语义 | 本节除下列两行外全部用例 |
| `ProtectRules`（注入依赖） | 不 mock——真实写 `.dev-team/config.json` 的用户规则后经 `ProtectRules::load` + `with_protect` 注入（规则加载与三段序同属被测面）；这是被测 API 的显式注入依赖，非进程边界替身 | `命中用户规则即拒写` / `未注入规则集不静默放行` / `旧expect拒且保护闸后置` |
| 文件系统（`snapshot` 前后对比） | 不 mock——用既有 `snapshot(root)` 取真实字节快照判「零落盘」 | `结构诊断前置零落盘` / `命中用户规则即拒写` |

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner_test.rs

<!-- 挂 AC-2 的装配侧（archi 来源臂七名 + 铸造判据）、AC-14 的接线（`with_protect` 一处）。
     「公共函数 / API」与「内部接线面」两张子表都无本文件行；本文件的两处改动的唯一登记处是
     design「修改文件」表第 7 行（`mint_archi_context` 判据五 → 七、`session_pump` 接线
     `with_protect`）。故「待测功能」按该行登记**文件级被测面**两项，不虚构 `pub fn` 条目。
     组合用例挂本节的理由：链路发起方是装配（mint → with_protect → execute 自上而下）。 -->

#### 待测功能

- `mint_archi_context()`（文件级被测面，`pub(crate)`，非公共 API 表条目）: 铸造判据由五名扩为**七名**（任一在场即铸；名字清单包含形态与调用点单点不变）
- `session_pump` 的保护规则接线（文件级被测面）: 在 `ProtectRules` 装配之后以 `archi.map(|context| context.with_protect(protect.clone()))` 接线——写臂与 `write` / `edit` 共用同一规则集

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `mint_archi_context()` · 七名任一在场即铸 | 正向 | 七名（`archi_module` / `archi_search` / `archi_api` / `archi_drift` / `archi_scaffold` / `archi_element` / `archi_relation`）逐一单独在场 ⇒ 各 `Some(context)`；混合清单（只含两写名）⇒ `Some`——`七名任一在场即铸` | 穷举迭代（7 名 + 1 混合） |
| `mint_archi_context()` · 非 archi 名单不铸 | 异常 | 空清单 / 只七内建 / 只 `create_explore_note` / 只两 DB 名 / 形似名（`archi_elementx`）⇒ 全 `None`——`非archi名单零铸造` | 矩阵迭代（5 形态） |
| `mint_archi_context()` · 入参边界 | 边界 | `root` 不存在 / 空白仍铸（读面错误归执行体，铸造不判盘）；`&[]` ⇒ `None`；两铸造判据互不污染（explore-only ⇒ `None`，archi-only ⇒ `Some`）——`铸造不判盘且两判据互不污染` | 矩阵迭代（3 形态） |
| `with_protect` 接线（组合：`session_pump` → mint → with_protect → execute） | 正向 | 用户规则命中目标文件时，装配出的 context 调写臂 ⇒ 拒串（与 `ProtectRules::evaluate_target` 直接调用同串）；未命中 ⇒ 写入成功；`with_protect` 链式调用不改 `root`（读臂响应与未注入时逐字相同）——`装配侧同一规则集接线` | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | 真实名字切片 + 真实 `ProtectRules::load`（真实配置文件）+ 真实 `TempDir` 模型 | 本节全部用例 |
| `session_pump` 的整轮启动 | 不在单测内驱动（泵需要 provider / store 全链）——接线判据经 `with_protect` 后再 `execute` 的组合面覆盖；`session_pump` 的接线形态由 `include_str!("runner.rs")` 的 `with_protect(` 恰一处锚定 | `装配侧同一规则集接线` |

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools_test.rs

<!-- 挂 AC-10 的沙箱面（两写工具无 `path` 入参 ⇒ 恒 `None`，沙箱改写面零扩宽）。
     「公共函数 / API」与「内部接线面」两张子表都无本文件行；唯一登记处是 design
     「修改文件」表第 8 行（`input_path` 名单加两名 + 常量再导出 + doc 措辞随动）。
     故按该行登记文件级被测面两项。 -->

#### 待测功能

- `input_path()`（文件级被测面）: archi 名单加两写名 ⇒ 恒返回 `None`（两工具无 `path` 入参）；`input_path` doc 的「五个架构只读工具」措辞随动
- 常量再导出（文件级被测面）: `pub(crate) use crate::sdk::mount::{…}` 增 `ARCHI_ELEMENT_TOOL` / `ARCHI_RELATION_TOOL`（执行体与装配面同源取常量）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `input_path()` · 七 archi 名恒 `None` | 正向 | 七名逐一 + 入参含 `{"path": "src/a.rs"}` ⇒ 全 `None`（不进路径沙箱）；两写名的 `file` 入参也不被当作 `path` 提取——`七archi名恒不进沙箱` | 穷举迭代（7 名） |
| `input_path()` · 既有分派零变化 | 异常 | `read` / `glob` / `write` 等非 archi 名仍按原分派返回 `Some(_)` / `None`（缺 `path` 时 `None`）；删两写名之外零语义变化——`非archi分派零变化` | 矩阵迭代（3 形态） |
| 两常量再导出与 doc | 边界 | `crate::sdk::tools::{ARCHI_ELEMENT_TOOL, ARCHI_RELATION_TOOL}` 可达且逐字等于 mount 常量；`include_str!("tools.rs")` 的 `input_path` doc 零「五个架构只读工具」旧措辞——`常量同源且doc随动` | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | 纯函数 + `serde_json::Value` 入参，零 IO | 本节全部用例 |

### packages/desktop/src-tauri/crates/infra/agent/src/compose.rs -> packages/desktop/src-tauri/crates/infra/agent/src/compose_test.rs

<!-- 挂 AC-2（矩阵与写权锁同集 / archi 臂七名 / 未命中相位零下发）、AC-12 的「写工具 canonical
     + 手改 second path」表述在装配面的落点（矩阵两行）。
     「公共函数 / API」与「内部接线面」两张子表都无本文件行；唯一登记处是 design
     「修改文件」表第 9 行（`archi_matrix` 两行追加两写工具 + archi 臂五 → 七 +
     模块头 doc 随动；`model_write_lock` 零改动）。故按该行登记文件级被测面。
     装置：既有 `injections_of(source, source_ref)` + `seven()` + 表驱动行；成员校验机制
     （loop 侧 `tool_unavailable`）由既有 `sdk/loop_test.rs` 用例背书，本变更不改。 -->

#### 待测功能

- `archi_matrix`（文件级被测面）: `(proposal, executor)` 与 `(dev-design, executor)` 两行追加 `archi_element` / `archi_relation`；其余五行逐字不变
- archi 来源臂（文件级被测面）: 名字清单五 → **七**（序随 `mount::registry()` 实现序）；模块头 doc 的臂描述随动
- 同集不变式（文件级被测面）: 矩阵含两写工具的 `(phase, role)` 集合 == `model_write_lock` 判 `None` 的集合，逐格相等（`model_write_lock` 实现零改动）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `archi_matrix` · 七行逐行 | 正向 | `proposal/executor` 与 `dev-design/executor` 两行的 archi 面各含两写工具（序尾随 `mount` 清单序）；其余五行（`dev-design/evaluator` / `test-design/executor` / `implement/executor` / `code-review/evaluator` / `acceptance/evaluator`）零写名；每行名字清单 = 七内建 + 该行 archi 名 + 两 DB 名——`矩阵两行加写工具其余零变化` | 表驱动（7 行） |
| archi 来源臂 · 七名 | 正向 | `source = "archi"`（`source_ref` 三形态：`None` / 空串 / 带 change 形态）⇒ 名字清单 = 七内建 + 七 archi 名（`archi_module` / `archi_search` / `archi_api` / `archi_drift` / `archi_scaffold` / `archi_element` / `archi_relation`）；不含 `create_explore_note` 与两 DB——`archi臂七名三形态` | 表驱动（3 形态） |
| 同集不变式 · 逐格相等 | 异常 | 遍历全部 `(phase, role)` 组合：`matrix` 含 `archi_element` ⟺ `model_write_lock == None`；反向 ⟺ 挂锁；两写工具与 `write` / `edit` 的相位面一致（`source = "archi"` 恒无锁且有写名）——`矩阵与写权锁同集` | 穷举迭代（全组合） |
| 未命中相位零下发 | 异常 | `test-design/executor` 只 `archi_search`、`code-review/evaluator` 只 `archi_module`、`explore` / `debug` 来源零 archi 名（除臂规则）；调用被成员校验拒绝（既有 loop 机制，测试不改，只断言「未下发」）——`未命中相位零下发` | 矩阵迭代（3 形态） |
| 装配面源码锚 | 边界 | `include_str!("compose.rs")` 内两处写权相位字面量只落 `archi_matrix` 与 `model_write_lock` 两区；`archi_matrix` 签名逐字不变（`fn archi_matrix(phase: &str, role: &str) -> Option<Vec<&'static str>>`）；`model_write_lock` 函数体的锁 glob 与文案零改动（`SessionProtectRule` 构造仍只在该函数内）——`矩阵形状不变且锁面零改动` | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | `injections_of` 走真实 `compose` 装配（store / vcs / provider 由既有测试夹具注入的闭包替身，非本节新增）；矩阵与锁面判定是纯函数 | 本节全部用例 |
| 既有装配夹具（store / provider 闭包替身） | 沿既有装置、不新增——它们服务整个文件的全部用例，不在本节引入新的替身面 | 全节（沿用） |

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/mod.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/mod_test.rs

<!-- 挂 AC-14（保护闸射程：写工具目标文件过同一 `ProtectRules`、同一规则序与拒串族、
     零第二份规则集）。
     条目取自 design「内部接线面」子表的 `ProtectRules::evaluate_target` 一行（新增）。
     装置：既有 `write_config` / `user_rule` / `with_rules` / `protected_path` / `Rule` helper；
     真实 `.dev-team/config.json` 落盘后 `ProtectRules::load`。 -->

#### 待测功能

- `ProtectRules::evaluate_target()`: `pub(crate) fn evaluate_target(&self, tool: &str, path: &Path) -> Option<String>`——与 `write` / `edit` 路径判定同一 `normalize` → `match_path` → `deny_for` 链（抽私有 `deny_normalized` 供 `evaluate_path` 复用）；零新规则、零新拒串

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `evaluate_target()` · 用户规则命中 | 正向 | 用户规则 glob 命中目标路径 ⇒ `Some(拒串)`，且与 `evaluate("write", json!({"path": 同路径}), cwd)` 的串**逐字相等**（同一规则序、同一 `deny_for`）；`%s` / `%t` 已替换为路径与 `tool` 实参——`与write路径判定同串` | 矩阵迭代（2 入口） |
| `evaluate_target()` · 内置规则命中 | 正向 | 内置规则命中 ⇒ 内置拒串（内置在前、首命中即拒）；用户规则与内置同时命中 ⇒ 返回内置串（规则序不变）——`内置优先且首命中即定` | 矩阵迭代（2 形态） |
| `evaluate_target()` · 未命中放行 | 异常 | 路径不在任何规则内 ⇒ `None`；空规则集（`with_rules(vec![])`）⇒ `None`——`未命中放行` | 矩阵迭代（2 形态） |
| `evaluate_target()` · 归一形态一致 | 边界 | 同一路径的反斜杠形态 / `./` 前缀 / 尾随形式经 `rules::normalize` 后与 `evaluate_path` 同判定（同 `match_path` 命中集、同 `deny_for` 拒串；两入口共用同一 `deny_normalized` 链）——`两入口同归一链` | 矩阵迭代（3 形态） |
| `evaluate_target()` · 判定零 IO | 边界 | 目标路径不存在于盘 ⇒ 判定照常（不查盘、不 panic、零副作用）；同一 rules 连续两次调用同结果——`判定不查盘` | 矩阵迭代（2 形态） |
| `evaluate()` · 三路分派零变化 | 异常 | `evaluate` 对 `write` / `edit` 走路径、`bash` 走命令、其余（含七个 archi 工具名）恒 `None`——射程遍历用例保留；`_ => None` 分支逐字不变（写模型工具**不经** `evaluate`）——`evaluate分派逐字不变` | 既有（维持） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | 真实规则构造（`Rule` 值 + 真实 `.dev-team/config.json` 经 `ProtectRules::load`）+ 真实 `checks::globmatch`（glob 方言单点即被测） | 本节全部用例 |
| 文件系统 | 不 mock——「判定零 IO / 不查盘」正是被测判据（用例中路径可不存在） | `判定不查盘` |

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/mod.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/atomic_test.rs

<!-- design.md 未声明该文件的公共函数 / API 变更（「公共函数 / API」与「内部接线面」两表都无
     本文件行）；本文件仅有 D-13 的一行机械连带 `pub(crate) mod atomic;`。故「待测功能」只登记
     文件级登记面，MUST NOT 虚构函数条目；测试归属 = `sdk/atomic_test.rs`（该文件能编译即
     登记成立，沿 `lib.rs → edit/mod_test.rs` 先例）。 -->

#### 待测功能

- `sdk/mod.rs 模块登记面`（文件级被测面，无函数定义）: 补 `pub(crate) mod atomic;`（新模块声明）与 `#[cfg(test)] mod atomic_test;`（测试登记）；模块可见性保持 `pub(crate)`（`sdk` 模块本身仍私有）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 模块登记面 · 声明与登记 | 正向 | `include_str!("mod.rs")` 内 `pub(crate) mod atomic;` 恰一处、`#[cfg(test)] mod atomic_test;` 恰一处；`use super::atomic::write_atomic` 编译通过（登记成立的可执行证据）——`atomic模块登记在册` | 单点 |
| 模块登记面 · 可见性 | 异常 | `pub mod atomic`（提升为公开模块）零命中；`mod atomic;`（私有到 `sdk`）零命中——`模块可见性维持pub(crate)` | 矩阵迭代（2 形态） |
| 模块登记面 · 零第二份声明 | 边界 | `atomic` 模块在 `sdk/mod.rs` 之外零声明（无第二处 `mod atomic`）；既有 `mod` 声明块序不被扰动（新增行插在声明区而非测试登记尾部）——`声明单点` | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | `include_str!("mod.rs")` 编译期内嵌字节 + 编译期可达性 | 本节全部用例 |

### packages/desktop/src-tauri/crates/infra/agent/src/lib.rs -> packages/desktop/src-tauri/crates/infra/agent/src/lib_test.rs

<!-- design.md 未声明该文件的公共函数 / API 变更（两张子表都无本文件行）；唯一改动是
     design「修改文件」表第 12 行的门面导出行 `pub use sdk::atomic::write_atomic;`。
     故「待测功能」登记文件级门面面一项，测试归属 = 既有 `lib_test.rs`
     （其「crate 根导出面锚定…」用例族正是导出面归属地）。 -->

#### 待测功能

- `crate 根门面`（文件级被测面，无函数定义）: `pub use sdk::atomic::write_atomic;`——壳层经 `agent_runtime::write_atomic` 引用；`sdk` 模块本身仍私有（零 `pub mod sdk`）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| crate 根门面 · 名字可达与签名锚 | 正向 | `let f: fn(&Path, &str) -> Result<(), String> = agent_runtime::write_atomic;` 编译通过（名字经 crate 根可达且签名逐字一致）——`write_atomic经门面可达` | 单点 |
| crate 根门面 · sdk 仍私有 | 异常 | `include_str!("lib.rs")` 内 `pub mod sdk` 零命中；`pub use sdk::atomic::write_atomic` 恰一处——`sdk模块保持私有` | 矩阵迭代（2 形态） |
| crate 根门面 · 既有导出零丢 | 边界 | 既有 `crate根导出面锚定_…` 用例逐条保留（`compose_turn` / `ComposedTurn` / `session_query` / `EngineFacade` / `EngineKind` / `EngineConfig` / `ResumeTranscript` 等）；新增一行不改变既有名字面——`既有门面零丢` | 既有（维持） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | 签名锚为零调用编译期锚；`include_str!` 源码锚 | 本节全部用例 |

### packages/desktop/src-tauri/crates/infra/agent/src/archi-knowledge.md -> packages/desktop/src-tauri/crates/infra/agent/src/archi_knowledge_test.rs

<!-- 挂 AC-11（四段 / 手改出口一句 / 零语法教学 / 零结构诊断词 / 新门限冻结 / 两处派生目录）、
     AC-12 的知识面（零旧立场句）。
     「公共函数 / API」子表无本文件行（静态资产）；唯一登记处是 design「修改文件」表第 13 行
     与 C8 / D-10（附录 A 草案）。故按该行登记文件级被测面。
     改判面（既有语料用例随知识面收窄）：`骨架零围栏零示例零三档表`、`骨架零字段形态教学`、
     `骨架含kind四值语义`、`骨架含计数单源一行`、`骨架_指路表逐句在场`（10 行 → 收窄后行集）、
     `骨架形态纪律四条在场`、`骨架含键形真因果句`、`骨架含path基数三口径`、
     `骨架规范语义在体量压缩下不被删`——语法段离场的断言必须**同批删除或改判**，
     MUST NOT 留恒真空断言（AC-12b 的同一纪律）。 -->

#### 待测功能

- 骨架文本（文件级被测面，静态资产）: 四段结构（职责 / 工具面 / 留给判断 + 手改出口一句；派生路径段不在骨架内）；门限 **骨架 ≤ 1500 B / 全文 ≤ 1700 B**（`lf_bytes(trim_end)` 口径，实测值冻结）；零示例 TOML 围栏、零键形示例、零字段值域行、零结构诊断词

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 骨架 · 四段与手改出口 | 正向 | 四段小节标题在场（`## 职责` / `## 工具面` / `## 留给判断`，加 `# 架构模型须知` 起手）；手改出口一句含「工具表达不了的」「手改」「`archi_module` 自查」「`file:line`」四要素；职责段含「只改架构目录」「画布不自动刷新」「续接历史会话」三句；工具面含五只读名 + 两写工具名——`四段与手改出口逐句在场` | 矩阵迭代（4 组锚） |
| 骨架 · 零语法教学 | 异常 | 零可解析 TOML 代码围栏（```）；零键形示例（`[element."` / `[[relation]]` 形态的示例行；行内反引号引述不计）；零字段值域行（`gate = "off" / "warn" / "strict"` 式、`api = "declared"` / `"none"` 式、`kind` 四值 bullet 全离场）；零 `metadata = { … }` 示例——`语法教学段全离场` | 矩阵迭代（4 面） |
| 骨架 · 零结构诊断词 | 异常 | `orphanHierarchy` / `dependsOnAncestor` 零命中；其余结构诊断词（`cycle` / `duplicateElement` / `unknownField` 等）零命中（诊断词只由工具 `Err` 与 drift 读数承载）——`零结构诊断可执行词` | 穷举迭代（诊断词表） |
| 骨架 · 体量冻结 | 边界 | `lf_bytes(skeleton().trim_end())` == 实现后实测值且 ≤ 1500；`lf_bytes(&knowledge())` == 实测值且 ≤ 1700；两值同批冻结（内容变更须同批更新，MUST NOT 抬锚）——`骨架与全文双门限冻结` | 矩阵迭代（2 数） |
| 骨架 · 两处派生目录仍在行程 | 边界 | `knowledge()` 全文含模型声明目录与契约资产目录两段（`模型目录：` 前缀逐字保留）；`knowledge()` 以 `skeleton().trim_end()` 起手（拼装前缀不变）——`派生段零改动` | 矩阵迭代（2 形态） |
| 骨架 · 工具点名指向真实工具 | 边界 | 骨架出现的每个 `archi_*` 词都在 `mount::registry()` 命中（含新增两写工具名）；零「指向不存在工具」的指路——`指路不指向不存在工具` | 既有（维持 + 扩两写名） |
| 骨架 · A 档指路表收窄 | 异常 | 收窄后的行集逐句在场（`path` / `entry` / `api` / 字段序 / 覆盖 / 两轴 / `config.json` / 结构诊断 / 生成物过期）；`orphanHierarchy` 行离场（对应断言同批删除，零恒真空）；`file:line` 指路仍由 `dto::graph_diagnostics_error` 真实投影背书——`指令表收窄且新行集可判` | 表驱动（9 行） |
| 骨架 · 旧立场零残留 | 异常 | 「无专用写工具」/「不设专用写工具」/「MUST NOT 存在专用写」三串零命中；「写盘仍用既有 write / edit」零命中——`知识面零旧立场` | 矩阵迭代（4 串） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | `include_str!("archi-knowledge.md")` 编译期内嵌字节（禁测试内复制副本）；`knowledge()` 走真实 `build` + 真实 `foundation::layout` 派生 | 本节全部用例 |
| `foundation::layout`（派生目录） | 不 mock——两处目录的派生是被测内容（「仍在行程」即断言其在场）；`layout` 为纯路径函数，零 IO | `派生段零改动` |

### packages/desktop/src-tauri/crates/infra/agent/src/archi_knowledge.rs -> packages/desktop/src-tauri/crates/infra/agent/src/archi_knowledge_test.rs

<!-- 挂 AC-11 的拼装面（模块头 doc 由六段改四段；`build` / `knowledge` 拼装与降级语义逐字不变）。
     「公共函数 / API」子表无本文件行；唯一登记处是 design「修改文件」表第 14 行。
     故按该行登记文件级被测面；`build` / `knowledge` 的既有用例（拼装 / 降级 / 零尖括号占位）
     只作「零回归」保留，不重写。 -->

#### 待测功能

- `archi_knowledge` 模块头 doc（文件级被测面，无函数定义）: 骨架描述由**六段**改为**四段**；`build` / `knowledge` 的拼装与降级语义逐字不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 模块头 doc · 段数改判 | 正向 | `include_str!("archi_knowledge.rs")` 模块头含「四段」表述（职责 / 工具面 / 留给判断 / 派生路径段）；「六段」零命中——`头注释四段随动` | 单点 |
| 拼装语义 · 零回归 | 异常 | `build` 对空白骨架 / 缺目录形态的 `Err` 与降级路径逐字不变；`knowledge()` 恒含派生两段；`骨架零尖括号占位` 既有用例保留——`拼装与降级语义不变` | 既有（维持） |
| 拼装语义 · 前缀与追加 | 边界 | `knowledge()` 以 `skeleton.trim_end()` 为前缀、派生段确为追加（`full.len() > head.len()`）——既有用例维持——`拼装前缀不变` | 既有（维持） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | `include_str!` + 真实 `build` 调用（纯字符串拼装，零 IO） | 本节全部用例 |

### packages/desktop/src-tauri/src/commands/atomic.rs -> packages/desktop/src-tauri/src/commands/atomic_test.rs

<!-- 挂 AC-13 的壳层半边（薄委托、签名 / 错误文案 / 临时后缀逐字不变、命令层零 `fs::write`）。
     条目取自 design「内部接线面」子表的「`write_atomic`（壳层）」一行（修改，薄委托）。
     收窄面：本文件既有 7 条行为用例迁 `sdk/atomic_test.rs`（内容逐条保留），本文件只留
     委托等价（正向 + 边界两臂）、两条错误串透传臂与结构锚一条。 -->

#### 待测功能

- 壳层 `write_atomic()`: `pub(crate) fn write_atomic(path: &Path, content: &str) -> Result<(), String>`——函数体一行转调 `agent_runtime::write_atomic`；签名 / 可见性 / 错误文案 / `.archi-tmp` 后缀 / 失败清临时文件 / 父目录缺失即 `Err` 逐字不变；函数体零 `fs::write` / 零 `fs::rename` / 零 `create_dir_all`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 壳层 `write_atomic()` · 委托等价 | 正向 | 写新文件 / 覆盖既有文件两形态 ⇒ 读回逐字等于入参、同目录零 `*.archi-tmp` 残留（与 infra 实现同行为；用例体只保留两条最小臂，其余已迁 infra 侧）——`委托后行为等价` | 矩阵迭代（2 形态） |
| 壳层 `write_atomic()` · 错误串透传 | 异常 | 父目录缺失 ⇒ `Err` 文案与 `agent_runtime::write_atomic` 直调**逐字相等**（含「写入临时文件失败」与路径）；空路径 ⇒ 「无父目录」——`错误文案原样透传` | 矩阵迭代（2 形态） |
| 壳层 `write_atomic()` · 结构锚 | 异常 | `include_str!("atomic.rs")` 内 `fs::write` / `fs::rename` / `create_dir_all` 零命中；`agent_runtime::write_atomic` 恰一处（委托单点）；签名行 `pub(crate) fn write_atomic(path: &Path, content: &str) -> Result<(), String>` 逐字在场——`命令层零落盘实现` | 单点 |
| 壳层 `write_atomic()` · 边界形态委托等价 | 边界 | 空内容 / 1 MB 超长内容 / 目标名以 `.archi-tmp` 结尾三形态 ⇒ 委托后与 `agent_runtime::write_atomic` 直调同行为（读回内容相等、零临时残留、`.archi-tmp` 后缀不变）——`边界形态与infra同行为` | 矩阵迭代（3 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | 真实 `TempDir` + 真实 infra `write_atomic`（委托目标即被测对象，替身会抹掉「同一实现」判据） | 本节全部用例 |
| `agent_runtime` 门面 | 不 mock——经 crate 根 `pub use` 的真实路径（AC-13「两写面共用同一函数」的机械前提） | `错误文案原样透传` |

### packages/desktop/src-tauri/src/commands/archi/mod.rs -> packages/desktop/src-tauri/src/commands/archi/mod_test.rs

<!-- 挂 AC-9（`archi_module` 出参 `fileHash`）、AC-13（命令落盘文本 == 同一 `plan_element_upsert`
     的 `content`；签名 / 错误面 / 失败零写入不变）。
     条目取自 design「公共函数 / API」子表的 `archi_set_gate_inner` 一行（修改，同签名）。
     `archi_module` 命令体零代码改动（`fileHash` 随投影出线），其用例是既有 `archi_module*`
     族的一行断言扩写，非新函数条目。
     既有 needle 随动：`archi_file_tree与唯一写命令的边界` 的 `plan_set_gate` needle 已在
     P3 改为 `plan_element_upsert`；本变更不动该用例。 -->

#### 待测功能

- `archi_set_gate_inner()`: 同签名 `(root: &Path, element: &str, gate: GateTier) -> Result<(), String>`——内部取 `plan_element_upsert(...)?.write` 再 `write_atomic`；命令面不暴露 `expect`
- `archi_module` 出参（既有命令，投影随动）: `ArchiModuleDetail` 带 `fileHash`（命令体零新增逻辑）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `archi_set_gate` · 逐字节与单行 diff | 正向 | `healthy` 合成模型改档 ⇒ 落盘文件与「只把该行 `gate` 值换掉」的期望文本**逐字节相等**、按行 diff 恰 1 行（既有用例维持）——`改档落盘与期望文本逐字节相等` | 既有（维持） |
| `archi_set_gate` · 文本单源（组合：命令 → `plan_element_upsert`） | 正向 | 命令落盘文本 == 直接调 `plan_element_upsert(root, &ElementSpec { fqn, gate: Some(g), ..Default::default() })` 的 `outcome.write.content` 逐字节相等（AC-13 的命令半边；工具半边在 `archi_tool_test.rs`，两半合取即「两通路同文本」）——`命令落盘文本即plan的content` | 单点 |
| `archi_module` · 出参含 `fileHash` | 正向 | `ArchiModuleDetail` 单元素响应八字段，`fileHash` 在末位且 == `drift::file_hash(root.join(元素声明文件))`；形态 `sha256:` + 64 hex——`module出参带fileHash` | 单点 |
| `archi_set_gate` · 失败零写入与错误串形态 | 异常 | 元素缺席 / `root` 非法 ⇒ `Err` 且目标文件字节零变化；`Err` 串 == `EditError::message()` 形态（判据句起始 + 恰一个全角 `：`）；结构错模型 ⇒ 文案含「模型存在结构诊断」与「拒绝写回」——`失败零写入且错误串即message` | 既有（维持 + 改 needle） |
| `archi_set_gate` · 三档往返与跨文件 | 边界 | 三档逐值往返读回一致；目标元素在第二文件时第一文件零变化；无显式 `gate` 行 ⇒ 插入获得该行（既有用例维持）——`三档往返与跨文件` | 既有（维持） |
| `archi_set_gate_inner` · 命令面纪律源码锚 | 边界 | `include_str!("mod.rs")` 的 `archi_set_gate_inner` 函数体内四 needle 在场（`plan_element_upsert` / `ElementSpec` / `.write` / `write_atomic`）；零 `expect`（命令面不暴露乐观锁）；模块头 doc 含「写工具与命令面共用同一 `plan_element_upsert`」与「命令面不暴露 `expect`」；命令清单仍九条、写命令仍两条——`命令面零领域规则且不暴露expect` | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | 真实 `TempDir` 模型 + 真实 `archi::edit` + 真实 `write_atomic`（落盘原子性与零残留即被测语义） | `archi_set_gate` 全部用例 |
| Tauri 运行时（既有装置） | `tauri::test::mock_app()` + 真实 `WorkspaceStores`（tempdir 数据根）沿用既有；本节新增用例不经 State（两入口都吃 `&Path`） | 沿用，不新增 |

### packages/desktop/src-tauri/crates/core/workflow/src/prompts/proposal-planner.md -> packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table_test.rs

<!-- 挂 AC-12b（prompts 面旧立场零命中 + 两写工具名 + 「工具表达不了的才手改」+ 自查句）、
     AC-12 的五处立场改写第 4 条（两个 phase prompt 与 `desktop-phase-prompts` 的同名断言）。
     「公共函数 / API」子表无本文件行（静态资产）；唯一登记处是 design「修改文件」表第 17 行
     与 C9。故按该行登记文件级被测面。
     改判面：既有用例 `写权相位两处含写面与自查` 的 `text.contains("无专用写工具")` 断言
     **必红**——改为两写工具名 + 「工具表达不了的才手改」，MUST NOT 留一条对旧句的恒真空断言。 -->

#### 待测功能

- `proposal-planner.md` 写面句与工具行（文件级被测面，静态资产）: 工具行改为「额外可用 archi 工具：读三件 + 写两件」（含 `archi_element` / `archi_relation`）；写面句改为「用两写工具写 + 工具表达不了的才手改」；保留「只为已确定的元素落笔」与 `archi_module` 自查句；仍含「架构模型可写」写权声明

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 写面句 · 两写工具与手改出口 | 正向 | `prompt_text("proposal-planner")` 含 `archi_element` 与 `archi_relation`；含「工具表达不了的才手改」；含「只为已确定的元素」与「写完」自查句；含「架构模型可写」——`写面句指两写工具与手改出口` | 矩阵迭代（5 锚） |
| 写面句 · 工具行读三写两 | 正向 | 工具行含三只读名（`archi_search` / `archi_module` / `archi_scaffold`）与两写名；零 `write` / `edit` 作为**唯一**写面表述（旧「经既有 write / edit 维护」句式离场）——`工具行读三写两` | 单点 |
| 写面句 · 旧立场零命中 | 异常 | 「无专用写工具」/「不设专用写工具」/「MUST NOT 存在专用写」三串在 `prompt_text("proposal-planner")` 零命中；`phase_table_test.rs` 内的断言串本身也零命中（AC-12a 的测试夹具面）——`旧立场句零命中` | 矩阵迭代（4 面） |
| 写面句 · 其余 12 份零改动 | 边界 | 锁定侧 12 份（14 − 2 写权相位）各含「架构模型已锁」且不含两写工具名（写权相位专属）；既有 `锁定侧十二份各含锁定句` 用例保留——`其余prompt零改动` | 既有（维持） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | `prompt_text(name)` 走 `include_str!` 编译期语料（既有 helper）；纯字符串断言 | 本节全部用例 |

### packages/desktop/src-tauri/crates/core/workflow/src/prompts/dev-design-planner.md -> packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table_test.rs

<!-- 挂 AC-12b（同上）与 design 修改文件表第 18 行 / C9。
     dev-design 侧差异：工具行由「四个只读工具」改为「读四件 + 写两件」；自查句仍
     `archi_drift` + `archi_module`（两份 prompt 的自查面不同，断言分别写）。 -->

#### 待测功能

- `dev-design-planner.md` 写面句与工具行（文件级被测面，静态资产）: 工具行改为「读四件 + 写两件」；写面句改为两写工具 + 「工具表达不了的才手改」；自查句仍 `archi_drift` + `archi_module`；仍含「架构模型可写」

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 写面句 · 两写工具与手改出口 | 正向 | `prompt_text("dev-design-planner")` 含 `archi_element` / `archi_relation` / 「工具表达不了的才手改」/「只为已确定的元素」/「架构模型可写」——`写面句指两写工具与手改出口` | 矩阵迭代（5 锚） |
| 写面句 · 工具行读四写两 | 正向 | 工具行含四只读名（`archi_module` / `archi_search` / `archi_drift` / `archi_scaffold`）与两写名；零「四个只读工具」旧措辞——`工具行读四写两` | 单点 |
| 写面句 · 自查句双工具 | 异常 | 自查句同时含 `archi_drift` 与 `archi_module`；「无专用写工具」三串零命中（含测试断言串面）——`自查双工具且旧立场零命中` | 矩阵迭代（2 面） |
| 写面句 · 与 proposal 侧不串味 | 边界 | `dev-design-planner` 的只读面含 `archi_drift` 而 `proposal-planner` 不含（两侧工具行各自成立，不互相复制）——`两侧工具行互不复制` | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | `prompt_text(name)` 的 `include_str!` 编译期语料 | 本节全部用例 |

### packages/desktop/src-tauri/src/types/generated/bindings.ts -> packages/desktop/src-tauri/src/bindings/mod_test.rs

<!-- 挂 AC-15 的字段面（`ArchiModuleDetail` 增 `fileHash` 一字段）。
     本文件是生成物（`bindings:export` 重导，MUST NOT 手改）；「公共函数 / API」子表无本文件行，
     唯一登记处是 design「修改文件」表第 19 行与 C11。故登记文件级产物面。
     必改断言：`field_names(&detail) == ["element","dependsOn","dependedBy","children","doc",
     "api","resolvedEntry"]`（七字段）在增 `fileHash` 后即失败 ⇒ 改八字段、`fileHash` 追加表尾
     （与 D-9「追加声明末尾」同向）。MUST NOT 以「删断言换绿」收场。 -->

#### 待测功能

- `bindings.ts` 的 `ArchiModuleDetail` 出线面（文件级被测面，生成物）: 八字段按声明序（`fileHash` 追加表尾，含 JSDoc）；其余类型段零新增 / 零删减

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ArchiModuleDetail` · 八字段序 | 正向 | `field_names(&type_section(&content, "ArchiModuleDetail"))` == `["element", "dependsOn", "dependedBy", "children", "doc", "api", "resolvedEntry", "fileHash"]`（`fileHash` 表尾）——`字段面按声明序出线且新键追加表尾` | 单点（必改） |
| `ArchiModuleDetail` · 字段类型与 JSDoc | 正向 | `fileHash` 行类型为 `string`；其上方 JSDoc 在场（出线注释随 `#[doc]`/字段注释）；零 `null` 或可选修饰（`String` 非 `Option`）——`新字段为必填string带注释` | 单点 |
| 产物其它类型面零变化 | 异常 | 既有负向断言逐条保留（退役类型零 `export type … =` 声明；`ArchiElementBrief` 零出线；`ArchiGraph` 三字段序 / `ArchiResolvedEntry` 三字段序 / `ArchiElement` 字段序逐字不变）——`其余类型面零新增` | 既有（维持） |
| 导出稳定性 | 边界 | 连续两次 `export_bindings()` 后 `bindings.ts` 内容相等（含新字段零抖动）；篡改 — 重导出 — 比对既有机制不变——`导出幂等` | 既有（维持） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock（含产物改写） | 真实 `export_bindings()` + 既有 OS 文件锁（跨进程串行）改写产物后比对；失败不恢复即红（既有装置） | 本节全部用例 |
| `specta` / `specta-zod` | 不 mock——导出链是被测单点（手改生成物会被下次生成覆盖） | 全节 |

### packages/desktop/src-tauri/src/types/generated/bindings.zod.ts -> packages/desktop/src-tauri/src/bindings/mod_test.rs

<!-- 挂 AC-15 的第二半（`ArchiModuleDetailSchema` 同字段、`bindings.zod.ts` 无其它 diff）。
     唯一登记处是 design「修改文件」表第 20 行与 C11；「公共函数 / API」子表无本文件行。 -->

#### 待测功能

- `bindings.zod.ts` 的 `ArchiModuleDetailSchema`（文件级被测面，生成物）: 含 `fileHash` 字段且类型为 `z.string()`；其余 schema 面零新增键

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ArchiModuleDetailSchema` · 新字段在场 | 正向 | `bindings.zod.ts` 内 `ArchiModuleDetailSchema` 段的 `fileHash:` 在场且值形态 `z.string()`（零 `optional` / `nullable`）——`zod同字段为必填string` | 单点 |
| schema 面零新增键 | 异常 | 该 schema 的其余七键逐字不变（`element` / `dependsOn` / `dependedBy` / `children` / `doc` / `api` / `resolvedEntry`）；`allowed_object_keys` 基座判型机制零改动——`其余schema键零变化` | 既有（维持） |
| zod 产物稳定性 | 边界 | 连续两次导出 `bindings.zod.ts` 内容相等（既有 `first_zod == second_zod` 用例保留）；目录重建后同批恢复用例保留——`zod导出幂等` | 既有（维持） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 mock | 真实 `export_bindings()` + 既有产物文件锁 | 本节全部用例 |

---

## 不可测试项

| # | 模块 | 原因 | 替代保证 |
|---|---|---|---|
| 1 · 版本行 | `packages/desktop/package.json`（`version` 0.7.14 → 0.7.15）、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml` 的版本行 | 一行字面量，无函数 / 分支 / 输出可断言；版本纪律属交付面机械判据（AC-16） | 桌面端 `git diff` 核对：`package.json` 相对落地时 `HEAD` 恰 +1 patch、`tauri.conf.json` 仍 `"version": "../package.json"`、`src-tauri/Cargo.toml` 逐字不变 |
| 2 · 生成物零 diff 与仓级读数 | AC-15 的「`bindings.ts` / `bindings.zod.ts` 的 diff **恰为**一字段、无其它 diff」、`api:check` 零差异、`.dev-team/architecture/**` 零字节变化（55 / 129 / `modelHash` / 六表头）、`apiFindings` 计数不变 | 派生生成物不许手改（会被下次生成覆盖），仓级读数与 diff 面不是可执行单元 | `bindings:export` 后 `git diff -- packages/desktop/src/types/generated/` 恰增 `fileHash` 两处；`api:check` 非零即红；`archi_drift`（Full）读数复核 55 / 129 / `clean` / `apiFindings` 构成；单测面由 `src/bindings/mod_test.rs` 的八字段序 + zod 同字段 + 既有负向断言兜住 |
| 3 · 仓级 grep 零命中（产品源码面，AC-12a） | 「无专用写工具」/「不设专用写工具」/「MUST NOT 存在专用写」在 `packages/desktop/src-tauri/**` 的 `.rs`（含注释 / doc / 测试夹具）零命中；「写盘仍走既有」/「非法 kind:」同面零命中 | 仓级扫描不是可执行单元：单测读全仓会越过测试边界并把别的 `*_test.rs` 防线拖进判定面 | 单测窄面兜四处：`mount_test.rs`（scaffold doc）、`archi_tool_test.rs`（`SCAFFOLD_TEMPLATE` 首行 + `scaffold_payload` doc 三处口径 + `domain_value` 旧文案）、`phase_table_test.rs`（两份 prompt 与断言串面）；仓级零命中由桌面端 static-check 执行 |
| 4 · spec delta 文本面（AC-12c / AC-17） | 七份 delta 的 RENAMED `FROM:` / `TO:` 逐字一致、MODIFIED 为基线整段拷贝后改、每条 Requirement 至少一个 Scenario 且 WHEN/THEN 成对、各含 `## Module Contract` 段；五处立场承接块新文本不把旧命题写成肯定命题 | spec 是文本资产，不是可执行单元；本变更不改写 delta 之外的能力 | 桌面端按 AC-12c / AC-17 逐条核对七份 delta 的标题链、Scenario 计数与承接块措辞；`openspec/specs/**` 零改动由改动集核对兜住（AC-12 的基线豁免正是其相容点） |
| 5 · 改动集与白名单（AC-16） | `git status --porcelain` 的改动集 ⊆ 提案「变更范围」所列文件（含 D-13 的 `sdk/mod.rs` 一行 + 本 change 目录） | 工作树状态不是测试面 | 桌面端 `git status --porcelain` 与提案文件表逐条核对；`openspec/specs/**` 零改动、`src-tauri/Cargo.toml` 逐字不变两条同批核 |
| 6 · 仓级唯一性 grep（AC-5 / AC-10 / AC-13） | 「第二个只删一个元素的删除实现」零命中、`DeleteSet` 零 `is_empty`（D-18）、「写面限额数字只在 `WRITE_LIMITS` 一处」仓级零命中、第二份「临时文件 + rename」零命中 | 四条都是跨文件仓级扫描，单测只锚自己文件的窄面 | 单测窄面各自兜一处：`edit/mod_test.rs`（`fn apply_element_delete` 恰一处 + 零第二删除路径 + `fn is_empty` 零命中）、`mount_test.rs`（十二数字只在 `WRITE_LIMITS`）、`atomic_test.rs`（`fs::write` / `fs::rename` 各恰一处）+ `commands/atomic_test.rs`（壳层零 `fs::*` 落盘）；跨 crate 面由 static-check 按 tasks.md 阶段 5 / 阶段 6 的 grep 执行 |
| 7 · 本仓真实模型面（`.dev-team/architecture/**`） | 「55 元素 / 129 关系 / 六文件表头逐字不变」「写工具在真实模型上落点正确」 | 单元测试不读仓内模型目录（`archi` / `agent` 既有测试全用合成 fixture，读真实仓面会让测试与仓数据耦合） | 合成 fixture 复刻形态（多文件模型 + 父元素声明 + 跨文件边）；仓面零变化由 `git diff -- .dev-team/architecture/` 零输出 + `archi_drift` 读数复核兜住 |
| 8 · AC-2 的 loop 侧成员校验拒绝 | 未命中相位的会话调 `archi_element` 被 `loop` 以 `tool_unavailable` 拒绝（`is_error` 回灌） | 需要整轮泵启动（provider / store / 异步流），超出单元测试边界；且该机制本变更零改动 | 既有 `sdk/loop_test.rs` 的成员校验用例（`explore轮幻觉write调用被成员校验拒绝且run不中断`）背书机制不变；单测面由 `compose_test.rs` 的「未命中相位零下发」断言兜住（不下发 ⇒ loop 必拒，同一判据的两半） |
