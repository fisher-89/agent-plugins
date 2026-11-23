# 测试设计: session-preamble-snapshot

> **输入**: `proposal.md`（13 条验收标准）、`design.md`（DD1–DD10 + 变更清单 + 公共函数 / API 表）、`tasks.md`（十阶段）、五份 delta spec。
> **本次被测面**: design「修改文件」表 15 行 / 16 个文件（`bindings.ts` + `bindings.zod.ts` 同行）；「新增文件」子表整段省略（零新增源文件）、「删除文件」为零。
> **测试框架盘点**: Rust = `cargo test --workspace`（nextest 执行，`#[test]` / `#[tokio::test]`，断言用 `assert_eq!` / `assert!`，fixture 走 `tempfile` 真盘真库，假件为端口 trait 的显式注入替身）；前端 = vite-plus `vp test`（`vite-plus/test` 的 `describe` / `it` / `expect` / `vi`，`@testing-library/react` + jsdom，文件后缀 `*.test.ts(x)`）。
> **只设计测试**：不写测试运行命令细节、不执行静态检查与测试、不产出评估结论。
> **回退修红口径**：test-execution 的 8 项红全为测试侧期望 / 断言未随本变更更新（无生产码缺陷），逐条修正登记见「验收范围」的「既有用例连锁（回退修红登记）」表；覆盖率缺失为测试红连带，不另设覆盖率用例。

---

## 验收范围

| AC ID | 验收条件 | 被测文件或模块 |
|---|---|---|
| AC-1 | 首轮（`New` / `is_unstarted` 的 `Continue`）拍存快照：正文 = 知识 + 空行 + `AGENT.md` 段 + 空行 + 指引常量，空段跳过 | `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/preamble.rs`；测试落点：`compose_test.rs`、`sdk/preamble_test.rs` |
| AC-2 | 已开轮 `Continue` 读库覆盖注入位（不读文件 / 不算知识）；`sdk/loop.rs` 零 `preamble::load` / `AGENT.md` 字样；两轮 System 逐字相等 | `crates/infra/agent/src/compose.rs`、`crates/infra/agent/src/sdk/loop.rs`、`crates/core/agent/src/runner.rs`（注入位 doc，零结构改动）；测试落点：`compose_test.rs`、`sdk/loop_test.rs`、`core/agent/src/runner_test.rs`（本变更零新增用例，见该章留白） |
| AC-3 | 会话中途改写 `AGENT.md` → 下一轮 System 与上一轮逐字相同 | 同 AC-2；测试落点：`compose_test.rs`、`sdk/loop_test.rs` |
| AC-4 | 存量会话（已开轮无快照行）首次 `Continue` 补拍一次，此后冻结 | `crates/infra/agent/src/compose.rs`、`crates/infra/store/src/store.rs`；测试落点：`compose_test.rs`、`store_test.rs` |
| AC-5 | 写一次冻结：二次 `stamp_session_preamble` 返回未写入且内容不变；无重载 / 刷新写面（命令面零新增） | `crates/infra/store/src/store.rs`、`crates/infra/agent/src/compose.rs`、`packages/desktop/src/types/generated/bindings.ts`；测试落点：`store_test.rs`、`compose_test.rs`、`src-tauri/src/bindings/mod_test.rs` |
| AC-6 | 存储面自动覆盖：workspace 模型组注册 + 信封注册表一行 ⇒ `list_models` / `scan` / `describe` / `describe_table` 全自动 | `crates/infra/store/src/model.rs`、`crates/infra/store/src/store.rs`、`crates/infra/store/src/envelope.rs`、`crates/infra/store/src/lib.rs`；测试落点：`model_test.rs`、`describe_test.rs`、`store_test.rs`、`envelope_test.rs`、`crates/infra/agent/src/sdk/db_tool_test.rs`、`src-tauri/src/commands/db/mod_test.rs`（命令面消费方连锁） |
| AC-7 | 空快照与缺席：全空 → 空串 + 零 System；`AGENT.md` 缺席而知识在场 → 知识 + 常量；文件后出现不改本会话 | `crates/infra/agent/src/sdk/preamble.rs`、`crates/infra/agent/src/compose.rs`、`crates/infra/agent/src/sdk/loop.rs`；测试落点：`sdk/preamble_test.rs`、`compose_test.rs`、`sdk/loop_test.rs` |
| AC-8 | 读取口径不变：根 = `SessionCtx.workspace_root`、严格单文件、32KB char 边界截断只作用于 `AGENT.md` 段（`truncate` 单点、零复制）、`load` 去 `async` 后函数体逐字保留 | `crates/infra/agent/src/sdk/preamble.rs`、`crates/infra/agent/src/compose.rs`；测试落点：`sdk/preamble_test.rs`、`compose_test.rs` |
| AC-9 | 出口与载荷纪律：`session_detail` 携 `preamble { savedAt, content }`；四项会话清单 `preamble` 恒 `null` 且不读快照表；bindings / zod 只经再生成 | `crates/core/agent/src/session.rs`、`crates/core/agent/src/lib.rs`、`crates/infra/store/src/store.rs`、`packages/desktop/src/types/generated/bindings.ts`、`packages/desktop/src/types/generated/bindings.zod.ts`；测试落点：`core/agent/src/session_test.rs`、`core/agent/src/lib_test.rs`、`store_test.rs`、`src-tauri/src/bindings/mod_test.rs` |
| AC-10 | 前端只读可见面：元信息行入口（`preamble !== null` 才渲染）+ `StandardDrawer` 抽屉（拍存时刻 / 体量 / 静态指引 / 全文）；零取数、零新命令、零订阅、不进对话流 | `packages/desktop/src/components/agent/agent-session-view.tsx`、`packages/desktop/src/hooks/use-agent-session.ts`；测试落点：`src/components/agent/agent-session-view.test.tsx`、`src/hooks/use-agent-session.test.ts` |
| AC-11 | 语料与文档一致：`db_tables` 计数 workspace 12 / 合计 16；`store.rs` 三处 doc 计数准确；`desktop-change-chat` 措辞；`desktop-agent-execution` 的 `sdk/preamble.rs` 行；`AgentSessionView` 契约资产 prose | `crates/infra/store/src/envelope.rs`、`crates/infra/store/src/store.rs`、`crates/infra/store/src/model.rs`、`.dev-team/architecture/models/Desktop/Components/Agent/Desktop.Components.Agent.AgentSessionView.md`、`openspec/changes/session-preamble-snapshot/specs/**`；测试落点：`crates/infra/agent/src/sdk/db_tool_test.rs`、`envelope_test.rs`、`describe_test.rs`、`model_test.rs`、`src-tauri/src/commands/db/mod_test.rs`（命令面消费方连锁）；文档 / 契约资产面见「不可测试项」 |
| AC-12 | 全管线回归：`run check` 四段、`vp test`、`cargo test --workspace` 全绿，`api:check` 零 diff | 上述全部源文件 + 既有测试套件（含 8 个前端 `SessionSummary` 夹具连锁文件）；测试落点：既有套件（桌面端 static-check / test-execution 相位执行）；夹具连锁与既有用例连锁见下注 |
| AC-13 | 版本交付（归档时）：`package.json` 0.7.7 → 0.7.8；`tauri.conf.json` 跟随；`src-tauri/Cargo.toml` 不随动 | `packages/desktop/package.json`；测试落点：无单测路径（见「不可测试项」） |

> **夹具连锁（不新增用例）**：`SessionSummary` 为 zod `strictObject`，且 `lib/ipc.ts` 的结果面 soft-report 会把缺键计入 `driftCount()`。以下测试文件的 `SessionSummary` fixture 需补 `preamble: null`（源文件本变更未修改，故不建独立被测章节）：`src/components/agent/session-panel.test.tsx`、`src/components/agent/index.test.tsx`、`src/lib/agent-transport.test.ts`、`src/views/agent/agent-debug-view.test.tsx`、`src/views/architecture/agent/archi-chat-panel.test.tsx`、`src/views/changes/agent/change-chat-panel.test.tsx`（另 `src/lib/ipc-contract.test.ts` 为纯类型面，字段加宽不破坏既有 `expectTypeOf` 判据）。

> **既有用例连锁（回退修红登记）**：下列 8 条既有用例的期望 / 断言须按 design 口径随动——全部为**测试侧期望未随本变更更新**，无生产码缺陷；第 9 行覆盖率缺失为测试红连带（llvm-cov 退出码 100），修红即解，不另立覆盖率专用夹具或新用例。各条修正的完整描述见对应源文件章节的「既有用例连锁」段。
>
> | 失败用例（nextest 名） | 测试文件 | 现判据 / 红因 | 按 design 口径的修正 |
> |---|---|---|---|
> | `compose::compose_test::archi臂调用点链路捕获与直调全等` | `crates/infra/agent/src/compose_test.rs` | 期望 `captured[0] == injections_of("archi", None)`（`assemble_injections` 直调值），未计 `begin` 的快照覆盖 | 期望 = 「装配面 + 快照覆盖」：`SessionInjections { preamble: Some(freeze(装配面 preamble, preamble::load(root))), ..injections_of("archi", None) }`，其余位逐字段不变（用例环境无 `AGENT.md` 时即 `知识 + "\n\n" + GUIDANCE`） |
> | `sdk::loop_test::db分派两工具到执行体且结果非错误` | `crates/infra/agent/src/sdk/loop_test.rs` | `DB_TABLES_TOOL` 分支断言 `rows.len() == 15` | 16（user 4 + workspace 12，`session_preamble` 登记行生效） |
> | `sdk::preamble_test::freeze不trim_段内首尾空白逐字保留` | `crates/infra/agent/src/sdk/preamble_test.rs` | 期望串手写时把 `AGENT.md` 段首 `\n` 压进连接符，段界少计 1 个换行 | 期望 = `format!("{knowledge}\n\n{agent_md}\n\n{GUIDANCE}")`，两段**原样**带入各自首尾 `\n`（`"\n  A  \n"` / `"\n<agent_md/>\n"` ⇒ 段界连续 4 个换行） |
> | `sdk::preamble_test::preamble源文件自证_async退役与截断单点恰一处` | `crates/infra/agent/src/sdk/preamble_test.rs` | 断言 `MAX_PREAMBLE_BYTES` 全文恰 2 次；实际为 `const` 定义 1 处 + `truncate` 体内 2 处 = 3 次，且 design 明写常量与 `truncate` 逐字不动 | 改为四判据（见 preamble 章节用例表末行）：`const` 定义恰一处；消费命中全部落在 `fn truncate` 函数体切片内；`fn freeze` 切片内 `MAX_PREAMBLE_BYTES` / `truncate(` 零命中；`truncate(` 正文调用唯一（`load` 内恰一处） |
> | `commands::db::mod_test::db_models_scope_workspace返回十一行且与直连serde一致` | `src-tauri/src/commands/db/mod_test.rs` | `counts` 期望 11 项，缺 `session_preamble` | 12 项：`("run_edge", 0)` 后追加 `("session_preamble", 0)`；标题与断言文案「十一行」→「十二行」 |
> | `commands::db::mod_test::两库清单互不混列_跨维度模型名扫描err` | `src-tauri/src/commands/db/mod_test.rs` | `ws_names` 期望 11 项，缺 `session_preamble` | 12 项：`"run_edge"` 后追加 `"session_preamble"`；断言文案「仅 workspace 维度十一行」→「十二行」 |
> | `store::store_test::find_session_summary与清单面等值两形态_none全等与some仅快照位不同` | `crates/infra/store/src/store_test.rs` | 末端等价断言的期望值写成 `SessionSummary { preamble: None, ..listed.clone() }`，与同用例已断言的 `found.preamble == Some` 自相矛盾 | 对**副本**置空再比：`let mut trimmed = found.clone(); trimmed.preamble = None; assert_eq!(trimmed, listed)`；`found.preamble.is_some()` / `listed.preamble.is_none()` 与 `row` / `stats` / `turns` 三组逐字段断言保留 |
> | `store::store_test::信封零改动覆盖四新模型_list_models计数与scan信封可读` | `crates/infra/store/src/store_test.rs` | `list_models()` 期望 vec 11 项，缺 `session_preamble` | 12 项：`("run_edge", 0)` 后追加 `("session_preamble", 0)`（注册序末项，`store_meta` 不入清单）；文案补「会话系统提示词快照」 |
> | `packages_desktop_src-tauri_rust` 覆盖率采集缺失（退出码 100） | — | 测试红导致覆盖率工件缺失 / 不可测 | 非用例：上 8 条修红后即解 |

---

## 单元测试

### packages/desktop/src-tauri/crates/core/agent/src/session.rs -> packages/desktop/src-tauri/crates/core/agent/src/session_test.rs

**框架与风格**：`#[test]` + `serde_json::to_value` / `from_value` 逐字段断言（既有文件同风格：驼峰线名字面断言 + 往返等值 + 源码自证读 `session.rs`）。

#### 待测功能

- `SessionPreambleInfo`（design「类型定义」表新增）：出线只读 DTO `{ saved_at, content }`，驼峰线名 `savedAt` / `content`，两字段各带 doc
- `SessionSummary.preamble`（design「类型定义」表修改）：追加 `Option<SessionPreambleInfo>`，**不 skip 序列化** ⇒ 线面恒出键（`null` 值）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `SessionSummary.preamble` | 正向 | 三字段 `row` / `stats` / `turns` 齐备 + `preamble: Some(SessionPreambleInfo { saved_at: 1727000000000, content: "快照正文\n段二" })` → `to_value` 后 `preamble.savedAt == 1727000000000`、`preamble.content` 逐字；反向 `from_value` 往返逐字段相等 | 单点 |
| `SessionSummary.preamble` | 正向 | `preamble: None` → 线面 `value["preamble"] == Value::Null` 且 `value.get("preamble").is_some()`（键恒在场，不省略） | 单点 |
| `SessionSummary.preamble` | 异常 | 线面缺 `preamble` 键 → 反序列化为 `None`（`Option` 隐式缺省，不产半成品）；对照：缺 `row` / 缺 `stats` / 缺 `turns` → 逐个 `Err` | 参数化（4 形态） |
| `SessionPreambleInfo` | 异常 | 越界形态：`preamble` 为字符串 / 对象缺 `content` / `savedAt` 为字符串 → 均 `Err`（不静默折成 `None`） | 参数化（3 形态） |
| `SessionPreambleInfo` | 正向 | 独立构造 `{ saved_at, content }` 序列化 = `{"savedAt": …, "content": …}`，无多余键；往返等值 | 单点 |
| `SessionPreambleInfo` | 边界 | `content` 三形态：空串（快照成立但无正文）、含 `\n` / `\0` / emoji 的特殊字符串、超 32KB（`"中".repeat(20000)`）超长串 —— 三者逐字往返、零截断 | 参数化（3 形态） |
| `SessionPreambleInfo` | 边界 | `saved_at` 四值：`0` / `-1` / `i64::MAX` / `1_727_000_000_000` → 线面为 JSON 数字且往返保真（无范围校验） | 参数化（4 值） |
| `SessionPreambleInfo` | 边界 | 派生链可用：`Clone` 后 `assert_eq!`；`PartialEq` 能区分 `Some(SessionPreambleInfo { content: String::new(), .. })` 与 `None`（空正文 ≠ 无快照） | 单点 |
| `SessionSummary`（三字段零改动） | 边界 | 既有 `SessionRow` / `SessionStats` / `TurnSummary` 线面逐字不变（沿用既有用例断言，本变更新增字段不渗入三字段）：`row` 键集 = `id` / `remoteSessionId` / `configSnapshot` / `provenance` / `createdAt` / `updatedAt` | 单点 |
| `SessionSummary`（源码自证） | 边界 | 读 `session.rs` 文本：`pub preamble:` 字段块内零 `skip_serializing_if` 命中（线面恒有键的机械判据，沿 `turn_summary_status_doc取值清单含interrupted` 的读源先例） | 单点 |

**既有用例连锁**：`session_summary组装往返_turns空数组与多轮两形态` 两处 `SessionSummary { … }` 字面构造需补 `preamble` 字段（空轮形态 `None`、多轮形态 `Some`）。

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock） | 纯数据 + `serde_json` 内存编解码；源码自证读 `session.rs`（同 crate 文件，非进程边界替身） | 全章节 |

---

### packages/desktop/src-tauri/crates/core/agent/src/lib.rs -> packages/desktop/src-tauri/crates/core/agent/src/lib_test.rs

**框架与风格**：`#[test]` + 类型元组「逐名可达」快照（既有 `既有再导出面快照逐名可达_增补不挤掉既有词汇` 同型：任一名字丢失即编译失败）。

#### 待测功能

- `SessionPreambleInfo` 根导出登记（design 修改文件表：`pub use session::{…}` 名单内按字母序插入，位于 `SessionPage` 与 `SessionProvenance` 之间）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `agent::SessionPreambleInfo` | 正向 | 根路径可构造 `SessionPreambleInfo { saved_at: 1, content: "x".into() }`，字段可读（非「仅被 use」的假锚） | 单点 |
| `agent::SessionPreambleInfo` | 异常 | 类型同一性：把根路径值传给形参为 `crate::session::SessionPreambleInfo` 的函数可编译（根导出是 re-export 而非包装类型）；`native_model` 无关面的 id / 版本身份不因导出而变（同一类型断言） | 单点 |
| `agent::SessionPreambleInfo` | 边界 | 既有 `_session_word_shapes` 元组补入新类型后逐名可达（`NewSessionRow` / `SessionPage` / `SessionProvenance` / `SessionRow` / `SessionStats` / `SessionSummary` / `TurnSummary` / `SessionPreambleInfo` 零丢失） | 全量遍历（元组逐名） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock） | 编译期可达性 + 构造形态断言 | 全章节 |

---

### packages/desktop/src-tauri/crates/core/agent/src/runner.rs -> packages/desktop/src-tauri/crates/core/agent/src/runner_test.rs

<!-- design.md 未声明该文件的公共 API 变更：修改文件表只把 `SessionInjections.preamble` 的语义 doc 反转为「会话系统提示词快照正文」，字段面 / serde 形态 / `PartialEq`、`Eq` 派生链逐字不动 ⇒ 本章节无新增用例。行为面由 `preamble_test.rs` / `compose_test.rs` / `loop_test.rs` 覆盖。 -->

#### 待测功能

<!-- 无公共函数 / API 行，留白。 -->

#### 用例

<!-- 无新增用例；`runner_test.rs` 既有用例应保持全绿（六位字段面零结构改动的回归面）。 -->

#### Mock策略

<!-- 无。 -->

---

### packages/desktop/src-tauri/crates/infra/store/src/model.rs -> packages/desktop/src-tauri/crates/infra/store/src/model_test.rs

**框架与风格**：`#[test]` + `<T as native_model::Model>::native_model_id()/native_model_version()` 逐模型断言 + `native_model::encode/decode` 内存往返 + tempfile 真库打开（既有 `十一模型注册workspace组全覆盖…` 同型）。

#### 待测功能

- `SessionPreambleRecord`（design「类型定义」表新增）：`native_model(id = 19, version = 1)`、主键 `session_id`、字段 `content` / `saved_at`、无版本链、无二级索引

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `SessionPreambleRecord`（模型身份） | 正向 | `native_model_id() == 19` 且 `native_model_version() == 1` | 单点 |
| `SessionPreambleRecord`（模型身份） | 异常 | 全册 id 冲突面：workspace 组 13 模型 + user 组 4 模型的 id 去重后长度不变；且 id 集合含 19、不含退役空洞 3 / 14 | 全量遍历（17 模型） |
| `SessionPreambleRecord`（模型身份） | 边界 | 无版本链：`model.rs` 文本内 `SessionPreambleRecordV` 零命中、`native_model(id = 19` 恰一处、其属性内无 `from =`（新增模型而非字段演进 ⇒ 零 `from` 链、零迁移路径） | 单点（源码自证） |
| `SessionPreambleRecord`（编解码） | 正向 | `native_model::encode` → `decode::<SessionPreambleRecord>` 逐字段相等，返回 `version == 1` | 单点 |
| `SessionPreambleRecord`（编解码） | 异常 | 以错误类型解码：`SessionPreambleRecord` 的载荷用 `SessionRecord` 解 → `Err`（编解码不跨界吞错） | 单点 |
| `SessionPreambleRecord`（编解码） | 边界 | `content` 三形态（空串 / 含 `\n\0` 与 emoji / >32KB 中文）与 `saved_at` 四值（`0` / `-1` / `i64::MAX` / 正值）组合 → 编解码往返逐字；`session_id` 边界（空串 / 超长 1100 char / 含 `\n` 与 `\0`）同样保真（主键不校验形态） | 组合遍历（3×4 + 3） |
| `SessionPreambleRecord`（线名） | 正向 | `serde_json::to_value` 键集恰 `{"content", "savedAt", "sessionId"}`，`sessionId` 为主键线名；反向 `from_value` 往返等值 | 单点 |
| `SessionPreambleRecord`（注册面） | 边界 | 真 workspace 库 `Store::open_workspace` 成功（13 模型注册无 id 冲突即 `Models::define` 不 panic）；`list_models()` 出现 `session_preamble` 行（空库计数 0 也列出） | 单点 |
| `SessionPreambleRecord`（与 `SessionRecord` 独立） | 边界 | 三字段结构字面构造即可编译（字段面仅 `session_id` / `content` / `saved_at`）：`SessionRecord` 加列不影响本模型、本模型加列不需要 `SessionRecord` 版本 bump | 单点 |

**既有用例连锁**：`十一模型注册workspace组全覆盖…` 的期望 id 序列由 `[2,7,8,4,9,10,11,12,13,16,17,15]` 变为 `[2,7,8,4,9,10,11,12,13,16,17,19,15]`（新模型注册在 `StoreMetaRecord` 之前），`list_models` 名单补 `session_preamble`，用例标题与「十一 / 十二模型」措辞同步为 13。

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock） | `native_model` 内存编解码 + tempfile 真库打开（进程边界依赖走真实文件系统与真实 redb） | 全章节 |

---

### packages/desktop/src-tauri/crates/infra/store/src/model.rs -> packages/desktop/src-tauri/crates/infra/store/src/describe_test.rs

**框架与风格**：`#[test]` + `describe::table(s)` / `schema_of` 派生面断言（既有 `ratchet…` / `候选与维度名为自纠文案单点` 同型）；样本记录表 `sample_records()` 逐表一条。

#### 待测功能

- `SessionPreambleRecord` 的 `///` doc 与字段说明进入 `describe` 派生面（design 修改文件表：表 / 字段说明的唯一派生源 ⇒ 自动纳入 ratchet 与下钻）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `describe` 表清单 | 正向 | workspace 维度表清单含 `session_preamble`（12 张），空库计数 0 也列出；user 维度 4 张不受影响 | 单点 |
| `describe` 表清单 | 异常 | 表名拼写错误（`session_preambles`）→ `Err` 且候选文案含 `session_preamble`（自纠通道）；合法名撞维度（以 user 维度查 `session_preamble`）→ `Err` | 参数化（2 形态） |
| `describe` 表清单 | 边界 | `sample_records()` 补第 16 条后 `records.len() == entries().len()`；`all_candidates()` 以 `workspace` 起、以 `session_preamble` 止，分隔符计数 = 表数 − 1 | 全量遍历 |
| `session_preamble` 表介绍 | 正向 | `table(Workspace, "session_preamble")` 的 `summary` 非空、按首个句读（`。` / `；` / 换行）截断、≤ 120 字且不截出半句（结构体 doc 首句判据） | 单点 |
| `session_preamble` 字段面 | 正向 | 下钻三字段：`sessionId`（`string`）/ `content`（`string`）/ `savedAt`（`number`），顺序为结构体字段序，`doc` 均非空 | 单点 |
| `session_preamble` 字段面 | 异常 | ratchet 双向一致：线名集 `{sessionId, content, savedAt}` == 真实 serde 键集（方向一：线名 ⊆ 键集；方向二：键集 ⊆ 线名；字段数 == 键数）；蛇形名 `session_id` / `saved_at` 全线缺席 | 单点 |
| `session_preamble` 字段面 | 边界 | `schema_of::<SessionPreambleRecord>` 返回可解析根引用（`json api-types` 的 schema 入口）；三次连调结果逐字相等（类型图每次重建） | 单点 |

**既有用例连锁**：`sample_records()` 的 15 条扩到 16 条（`assert_eq!(records.len(), entries().len())`）；`候选与维度名为自纠文案单点` 的 `ends_with("run_edge")` 改 `session_preamble`、`entries().len() - 1` 随动；`ratchet十五表…` 重命名为十六表（其函数体按 `entries()` 长度遍历，仅标题 / 文案需同步）。

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock） | tempfile 真 workspace 库 + `specta::Type` 运行时类型图真实派生（说明文本正是被测对象） | 全章节 |

---

### packages/desktop/src-tauri/crates/infra/store/src/store.rs -> packages/desktop/src-tauri/crates/infra/store/src/store_test.rs

**框架与风格**：`#[test]` + tempfile 真 redb 库（`Env::db_path` / `open_workspace_ok`）+ `seed_session` / `begin_agent_turn` / `finish_agent_turn` 既有 fixture；新增断言沿用「主键直查逐字段相等」与「源码自证读 `store.rs`」两种形态。

#### 待测功能

- `Store::stamp_session_preamble(session_id, content) -> Result<bool, StoreError>`（design「公共函数 / API」表新增）：单写事务写一次——行已存在 → `Ok(false)` 不改写不 commit；否则插入 + `commit()` + `Ok(true)`，`saved_at` 取 `now_millis()`
- `Store::find_session_preamble(session_id) -> Result<Option<SessionPreambleRecord>, StoreError>`（新增）：主键直查，miss → `Ok(None)`
- `Store::find_session_summary(session_id) -> Result<Option<SessionSummary>, String>`（修改，签名零变更）：装配前读快照并映射 `SessionPreambleInfo`
- `assemble_summary` 增第四参 / `query_sessions` 传 `None`（design 私有件）：清单路径 `preamble` 恒 `None` 且**不调用**快照读

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `stamp_session_preamble` | 正向 | 空库首次写入 `(ses-p, "知识\n\n<agent_md …>…</agent_md>\n\n指引")` → `Ok(true)`；随后 `find_session_preamble` 回读 `session_id` / `content` 逐字；`saved_at` ∈ `[before, after]`（store 侧时钟包夹） | 单点 |
| `stamp_session_preamble` | 异常 | 二次写入（改内容为 `"改写后的正文"`）→ `Ok(false)`；回读 `content` 仍为首次值、`saved_at` 不变（写一次冻结、无更新 / 重置语义） | 参数化（同内容 / 异内容两形态） |
| `stamp_session_preamble` | 异常 | 维度错配：user 维度 store（`Store::open_global`）上调用 → `Err`（native_db 拒绝 workspace 模型），且不产生行、不吞错 | 单点 |
| `stamp_session_preamble` | 边界 | 空串正文 `""` → `Ok(true)` 且回读 `content == ""`（「快照成立但无正文」）；超长正文（`"中".repeat(20000)`）→ 回读长度守恒（无 32KB 二次截断，截断只在 `preamble::truncate`） | 参数化（2 形态） |
| `stamp_session_preamble` | 边界 | 并发 / 重复三次连调（首 `Ok(true)` + 后两次 `Ok(false)`）；跨库重开（drop 后 `open_workspace` 同路径）快照行仍在且逐字（与会话同落一个 workspace 库文件） | 单点 |
| `find_session_preamble` | 正向 | 已 stamp 的 id → `Some(record)` 三字段逐字段相等；`saved_at > 0` | 单点 |
| `find_session_preamble` | 异常 | 未 stamp 的 id（会话在场）→ `Ok(None)`；空串 id → `Ok(None)` 不 panic；user 维度 store 上调用 → `Err` | 参数化（3 形态） |
| `find_session_preamble` | 边界 | 空库（零会话零快照）→ `Ok(None)`；超长 `session_id`（1100 char）→ `Ok(None)`（主键不校验形态） | 参数化（2 形态） |
| `find_session_summary` | 正向 | 有快照：`preamble == Some(SessionPreambleInfo { saved_at, content })` 逐字段相等；`row` / `stats` / `turns` 与既有聚合口径逐字段全等（快照步骤不改其余面） | 单点 |
| `find_session_summary` | 异常 | 查无此 id / 空 id → `Ok(None)`（既有语义不变，快照读不改变分层）；有快照但会话已被移除（仅快照行在）→ `Ok(None)`（不虚构会话） | 参数化（3 形态） |
| `find_session_summary` | 边界 | 无快照行的会话 → `preamble == None`（存量会话形态，MUST NOT 以空正文伪造） | 单点 |
| 清单路径（`list_sessions` / `list_sessions_page`） | 正向 | 先 stamp 一份快照行 → 清单项 `preamble == None`（四项清单同源，`agent_sessions` 等命令皆走此面） | 单点 |
| 清单路径（`list_sessions` / `list_sessions_page`） | 异常 | 快照体量放大面：写入 37KB 级正文后清单每项 `preamble == None`，且清单 JSON 体积不随快照膨胀（结构判据，非计时） | 单点 |
| 清单路径（`list_sessions` / `list_sessions_page`） | 边界 | 源码自证（「不读」而非「读了不给」）：读 `store.rs` 文本，`fn query_sessions` 函数体内零 `find_session_preamble` / `SessionPreambleRecord` 命中，且其 `assemble_summary(...)` 第四实参为 `None`；对照 `fn find_session_summary` 函数体内 `find_session_preamble` 恰一处 | 单点（源码自证） |
| `find_session_summary` 与清单等值 | 边界 | 既有 `find_session_summary与清单面等值两形态_none全等与some仅快照位不同` 更新：形态一（无快照行）`found == listed` 逐字段全等（`preamble` 双 `None`）；形态二（有快照行）先断言 `found.preamble.is_some()` / `listed.preamble.is_none()`，再用**副本**做等价——`let mut trimmed = found.clone(); trimmed.preamble = None; assert_eq!(trimmed, listed)`（置 `None` 只作用于副本，MUST NOT 回写 `found`，否则与上一断言自相矛盾）；`row` / `stats` / `turns` 三组逐字段断言保留 | 参数化（2 形态） |

**既有用例连锁（回退修红）**：

- `find_session_summary与清单面等值两形态_none全等与some仅快照位不同`：末端等价断言的期望值现写成 `SessionSummary { preamble: None, ..listed.clone() }`，而同一用例上一行已断言 `found.preamble == Some` ⇒ 期望值 `preamble: None` 与 `found` 的 `Some` 自相矛盾，必红。按上表末行改「副本置 `None` 后与 `listed` 全等」。
- `信封零改动覆盖四新模型_list_models计数与scan信封可读`：`list_models()` 期望 vec 11 项 → 12 项，在 `("run_edge", 0)` 之后追加 `("session_preamble", 0)`（workspace 注册序末项；`StoreMetaRecord` 不入清单），断言文案补「会话系统提示词快照」。

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock） | tempfile 真 redb 库承载 fixture 与断言（读写均为真实盘上操作）；维度错配用例以真实 `Store::open_global` 实例作失败注入，不伪造 `StoreError` | 全章节 |

---

### packages/desktop/src-tauri/crates/infra/store/src/envelope.rs -> packages/desktop/src-tauri/crates/infra/store/src/envelope_test.rs

**框架与风格**：`#[test]` + 真库 `list_models` / `scan` / `entries()` 断言 + tempfile 真库（既有 `注册表按维度分组列出…` / `entries长度与登记序与维度分组` 同型）。

#### 待测功能

- `MODEL_ENTRIES` 增 `session_preamble` 一行（design 修改文件表：`count` / `scan` / `key_of` / `schema` / `walk` 五件同型）——由此派生出 workspace 维度 11 → 12 行、名字唯一集 15 → 16
- 私有三包装（`scan_session_preambles` / `walk_session_preambles` / `session_preamble_key`）经 `Store::scan` 公共面触达（不虚构私有条目断言）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `entries()` | 正向 | 长度恰 16、名字唯一集 16、序 = 登记序（user 四行在前；workspace 段末项由 `run_edge` 变为 `session_preamble`）；workspace 维度恰 12 行、首项 `agent_run` | 单点 |
| `entries()` | 异常 | `scan("session_preambles")`（拼写错误）→ `Err`（Db 变体，文案携候选清单）；user 维度库 `scan("session_preamble")` → `Err`（跨维度不可达） | 参数化（2 形态） |
| `entries()` | 边界 | `entries()` 与两维度 `list_models` 并集逐名相等（同一真源）；空库 `scan("session_preamble")` → 空 `records` + `total == 0` | 全量遍历 |
| `Store::scan("session_preamble")` | 正向 | 播种 2 行后 `scan` 分页：`total == 2`、`records` 每条 `key == {…sessionId 字符串}`（`session_preamble_key` 取 `value["sessionId"]`）、`value` 三键 camelCase | 单点 |
| `Store::scan("session_preamble")` | 异常 | 越界 offset → 空 `records` 且 `total` 恒全量（分页信封语义）；limit 超上限截断 | 参数化（2 形态） |
| `Store::scan("session_preamble")` | 边界 | `limit = 0` / 恰等于总行数 / 分页跨越 `walk` 通路（`walk` 直通：一次读事务内逐条解码且计数一致） | 参数化（3 形态） |
| `entries()` / `list_models`（同源） | 正向 | workspace 维度 12 行（`session_preamble` 计数与 `stamp` 次数一致、`store_meta` 不出现）；全局库仍 4 行 | 单点 |

**既有用例连锁**：三处计数断言 11 → 12、`entries().len()` 15 → 16、`workspace.last()` 锚点 `run_edge` → `session_preamble`、`全部注册模型空库扫描均回空分页信封` 的模型名数组补一行（文件名内「十五 / 十一」措辞同步）。

**命令面连锁（源文件零 diff，仅测试期望随动）**：`src-tauri/src/commands/db/mod_test.rs` 两例的 `list_models` 期望行数 11 → 12——`db_models_scope_workspace返回十一行且与直连serde一致` 的 `counts` 在 `("run_edge", 0)` 后追加 `("session_preamble", 0)`；`两库清单互不混列_跨维度模型名扫描err` 的 `ws_names` 在 `"run_edge"` 后追加 `"session_preamble"`；两例标题与断言文案的「十一行」→「十二行」。`commands/**` 源文件按 design 红线零改动，连锁只落测试文件。

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock） | tempfile 真库 + 注册表真实分发（故障面用非法模型名 / 跨维度名触发真实 `Err`） | 全章节 |

---

### packages/desktop/src-tauri/crates/infra/store/src/envelope.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/db_tool_test.rs

**跨模块组合用例挂靠说明**：链路 = `envelope::MODEL_ENTRIES`（表清单唯一真源）→ `db_tables` 工具（最上层消费方，`crates/infra/agent/src/sdk/db_tool.rs` 零改动）。发起方 / 最上层调用方是 `db_tables`，故组合用例落在此章节，不新建独立集成区。

**框架与风格**：`#[tokio::test]` + 真双库 `DbContext` + `tables_of` / JSON 载荷断言（既有 `db_tables无参返回十五行清单…` 同型）。

#### 待测功能

- `db_tables` 无参清单随注册表 +1（design 修改文件表 / `desktop-agent-db-tools` delta：workspace 11 → 12、合计 15 → 16）
- `db_tables` 下钻 `session_preamble`（三字段线名 + 渲染类型 + doc）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `db_tables`（无参） | 正向 | 两库全列恰 16 行，含 `{"dimension":"workspace","table":"session_preamble"}` 行；其 `count` 与播种快照行数一致、`summary` 非空；每行仍只含四键（清单不展开 `fields`） | 单点 |
| `db_tables`（无参） | 异常 | `store_meta` 不出现在任一维度清单（标记模型不可见）；`AgentModelTiers` / `SessionConfigSnapshot` 等内嵌命名类型不入表清单 | 参数化（2 形态） |
| `db_tables`（scope 三态） | 正向 | `scope = "workspace"` → 12 行且末行为 `session_preamble`；`scope = "user"` → 4 行；缺省 / `null` → 16 行（两库全列） | 参数化（4 形态） |
| `db_tables`（scope 三态） | 异常 | 非法 scope（`"global"` / `""` / `1` / `{}`）→ `Err` 且文案列两合法值 | 参数化（4 形态） |
| `db_tables`（下钻） | 正向 | `{"table":"session_preamble"}` → 三字段按结构体序：`sessionId` / `content` / `savedAt`，类型 `string` / `string` / `number`，`doc` 全非空且为 camelCase 线名 | 单点 |
| `db_tables`（下钻） | 异常 | `{"table":"session_preambles"}`（拼写错误）→ 显式 `Err` 且候选清单含 `session_preamble`；`{"table":"session_preamble","scope":"user"}` → `Err`（维度与 scope 不符） | 参数化（2 形态） |
| `db_tables`（下钻） | 边界 | 空表（零快照行）下钻仍返回三字段（字段面与行数无关）；连续两次调用结果逐字相等（无跨调用状态） | 单点 |

**既有用例连锁**：`db_tables无参返回十五行清单且不展开字段`（15 → 16）、`db_tables_scope三态收窄且下钻按维度派库`（11 → 12）、`同一上下文两工具连续调用互不污染`（15 → 16）。同一 16 行基线在 loop 分派面另有锚点——`sdk/loop_test.rs` 的 `db分派两工具到执行体且结果非错误`（`rows.len() == 15` → 16），见 loop 章节连锁。

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock） | 真双库句柄（global + workspace，tempfile）+ 真实 `DbContext`；被 mock 的只有网络类依赖（本工具不触） | 全章节 |

---

### packages/desktop/src-tauri/crates/infra/store/src/lib.rs -> packages/desktop/src-tauri/crates/infra/store/src/model_test.rs

**框架与风格**：`#[test]` + 类型同一性 / 根路径可达断言（沿核心 crate `lib_test.rs` 的「同一实现」判据）。

#### 待测功能

- `pub use model::{…, SessionPreambleRecord}`（design 修改文件表）：出 crate 根供 `compose.rs` 读 `content` / `saved_at`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `store::SessionPreambleRecord` | 正向 | 根路径可构造三字段字面值，`<store::SessionPreambleRecord as native_model::Model>::native_model_id() == 19`（根路径真值而非仿造类型） | 单点 |
| `store::SessionPreambleRecord` | 异常 | 同一性否证：把根路径值传给形参为 `crate::model::SessionPreambleRecord` 的函数可编译；若根面另立仿造 struct，则 model id 断言与注册面（`open_workspace` 不 panic）互证会红 | 单点 |
| `store::SessionPreambleRecord` | 边界 | 根路径实例参与 `native_model::encode` / `decode` 往返逐字段相等（根导出不改编解码面） | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock） | 编译期可达性 + 真实编解码 | 全章节 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/preamble.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/preamble_test.rs

**框架与风格**：`#[test]`（去 `async` 后由 `#[tokio::test]` 降级为普通测试）+ `tempfile` 真实盘上 `AGENT.md` 读写 + 纯函数直调（既有文件已是「文件为进程边界依赖白名单、tempdir 真盘承载 fixture」口径）。

#### 待测功能

- `preamble::load(root: &Path) -> Option<String>`（design「公共函数 / API」表修改）：`async` 退役为同步；读取根 / 严格单文件 / 32KB char 边界截断 / `<agent_md path="…">` 包装 / 失败即 `None` 逐条不变；调用方由 `loop` 改为 `compose`
- `preamble::freeze(knowledge: Option<&str>, agent_md: Option<&str>) -> String`（新增）：非空段按「知识 → `AGENT.md` 段 → `GUIDANCE`」以 `"\n\n"` 连接；知识与 `AGENT.md` 皆空 → 空串；`GUIDANCE` MUST NOT 单独成文
- `preamble::GUIDANCE`（新增常量）：≤ 512 B、零路径字面量、零 `<>` 占位符，语义三点（冻结 / 真身 `AGENT.md` / `read` 重读）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `load` | 正向 | 在场：写 `"# 工作区约定 🎉\n\n- 提交信息用中文\n"` → 返回值 == `<agent_md path="{绝对路径}">\n{正文}\n</agent_md>` 逐字节相等（含中文 / emoji / 换行） | 单点 |
| `load` | 异常 | 缺席 → `None`；同目录仅 `CLAUDE.md` / `AGENTS.md` → 仍 `None`（严格单文件、零兜底回退） | 参数化（3 形态） |
| `load` | 异常 | 读取失败：`AGENT.md` 为目录 / root 路径不存在 / 文件非 UTF-8 字节 → 一律 `None`（读取侧永不报错） | 参数化（3 形态） |
| `load` | 边界 | 恰 32KB（`"a".repeat(32*1024)`）→ 全文返回、无截断留痕；超 1 字节 → 截到 32768 | 参数化（2 形态） |
| `load` | 边界 | 多字节边界：`"中".repeat(...)` 跨 32KB / emoji（4 字节）跨 32KB → 回退至最长 char 边界前缀，产物恒为原文前缀（不劈半、不包装改写） | 参数化（2 形态） |
| `load` | 边界 | 单文件语义：同目录并列 `CLAUDE.md` / `AGENTS.md` 内容不进入返回值（三文件全在时只读 `AGENT.md`） | 单点 |
| `freeze` | 正向 | 双段在场：`freeze(Some("知识"), Some("<agent_md …>…</agent_md>")) == "知识\n\n<agent_md …>…</agent_md>\n\n{GUIDANCE}"`（段序 = 知识 → `AGENT.md` → 常量，段间恰一个空行） | 单点 |
| `freeze` | 正向 | 单侧在场：仅 `AGENT.md` → `"<agent_md …>\n\n{GUIDANCE}"`；仅知识 → `"知识\n\n{GUIDANCE}"`（末段恒为常量，空前导不留） | 参数化（2 形态） |
| `freeze` | 异常 | 全空四形态：`(None, None)` / `(Some(""), None)` / `(None, Some(""))` / `(Some(""), Some(""))` → 均返回 `String::new()`，`GUIDANCE` 不单独成文 | 参数化（4 形态） |
| `freeze` | 边界 | 不 trim（段界换行不得少计）：`freeze(Some("\n  A  \n"), Some("\n<agent_md/>\n"))` 的期望串 = `format!("{knowledge}\n\n{agent_md}\n\n{GUIDANCE}")`——两入参**原样**代入（含各自首尾 `\n`）⇒ 段界处连续 4 个换行（上段尾 1 + 连接 2 + 下段首 1）；并断言 `body.contains("\n  A  \n")`（知识段首尾空白未被剥离）。MUST NOT 手写压缩后的字面量 | 单点 |
| `freeze` | 边界 | 不二次截断：入参 `AGENT.md` 段为 40KB 超长串 → 拼接结果长度 = 知识 + 2 + 段长 + 2 + 常量长（逐字节守恒，32KB 只在 `load` 内发生） | 单点 |
| `freeze` | 边界 | 知识段不截断：知识 > 32KB 原样在首位（截断只作用于 `AGENT.md` 段） | 单点 |
| `freeze` | 边界 | 纯函数性：同入参两次调用逐字相等、无内部缓存；任一入参变化即结果变化 | 单点 |
| `GUIDANCE` | 正向 | 体量判据：`chars().count() <= 512` 且 `len() <= 512`；非空且不以空白结尾 | 单点 |
| `GUIDANCE` | 异常 | 零路径字面量：不含 `"/"`、`"\\"`、`"C:"`、`"~"`；零占位符：不含 `<` / `>` / `{` / `}` / `{{`（装配层常量无注入位） | 参数化（2 形态） |
| `GUIDANCE` | 边界 | 三点语义在场：含 `AGENT.md`、`read`、冻结语义词（`冻结` 或等价表述）；`GUIDANCE` 在 `freeze` 产物中恰出现一次（`matches(GUIDANCE).count() == 1`） | 单点 |
| `load` / `freeze`（源码自证） | 边界 | `preamble.rs` 文本四判据（函数切片口径 = 自 `fn <name>(` 起至首个 `\n}\n` 收口，不含 `///` doc 段）：① `async fn load` / `tokio::fs` 零命中（`async` 退役）；② `const MAX_PREAMBLE_BYTES` 定义恰一处；③ `MAX_PREAMBLE_BYTES` 的消费命中**全部落在 `fn truncate` 函数体切片内**（定义行以外全文零命中、切片外零命中；不锁总命中次数——`truncate` 体内本就有 len 比较与回退初值两处，design 的「常量逐字不动」即此形态）；④ `fn freeze` 切片内 `MAX_PREAMBLE_BYTES` / `truncate(` 均零命中（零二次截断），`fn load` 切片内 `truncate(` 恰一处、`fn truncate(` 定义唯 1 | 单点（源码自证） |

**既有用例连锁（回退修红）**：

- `freeze不trim_段内首尾空白逐字保留`：期望串按 `format!("{knowledge}\n\n{agent_md}\n\n{GUIDANCE}")` 逐字构造（两段原样带入，含各自首尾 `\n`）。现手写字面量把 `AGENT.md` 段首 `\n` 压进连接符 → 段界少计 1 个换行，与 `freeze` 的 `segments.join("\n\n")` 实现必然不等。
- `preamble源文件自证_async退役与截断单点恰一处`：`MAX_PREAMBLE_BYTES` 的「全文恰 2 次」判据**不可满足**——`const` 定义 1 处 + `truncate` 体内 2 处消费 = 3 次命中，而 design 明写「`truncate` / 常量逐字不动」。改为上表末行的四判据：单一定义（`const` 恰一处）+ `truncate` 唯一消费函数（消费命中全部在 `fn truncate` 切片内）+ `freeze` 零截断（`fn freeze` 切片内两个字样零命中）+ `truncate(` 定义唯 1 / `load` 内调用唯一。
- 已落地的随动（保持）：全部 `#[tokio::test] async fn` 去 `async` 并改 `load(dir.path())`（去 `.await`）；`agent_md两次load之间改写即反映_读取侧无缓存` 只断言「`load` 逐次读盘、无缓存」，「每轮生效」语义的证明迁到 `compose_test` / `loop_test`。

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 文件系统（进程边界依赖） | 不 mock：`tempfile::TempDir` 真实承载 `AGENT.md`（写 / 改 / 删均为真实盘上操作；「AGENT.md 为目录」等失败面用真实目录占位） | `load` 全部用例 |
| 无其他 | `freeze` / `GUIDANCE` 为纯函数与常量，零依赖 | 合成与常量用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/compose.rs -> packages/desktop/src-tauri/crates/infra/agent/src/compose_test.rs

**框架与风格**：`#[tokio::test]` + `DualEnv`（tempfile 数据根 + workspace 根 + `WorkspaceStores`）+ `composed_with_fakes`（`ComposedTurn` 私有字段的测试侧装配缝：`RecordingRunner` 捕获 `SessionInjections` / `RecordingSink`）+ 真 redb 库的 `find_session_preamble` 回读。

#### 待测功能

- `ComposedTurn::begin(session, question, ctx, provenance) -> Result<RunningTurn, String>`（design「公共函数 / API」表修改，签名零变更）：首轮（`New` / `Continue` + `is_unstarted`）拍存 + 本轮用内存体；续轮已开轮读库覆盖注入位；存量会话补拍一次；落库在 `begin_turn` 之后且失败不中断（私有助手 `freeze_preamble` / `injectable` 经 `begin` 间接覆盖）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `begin`（首轮拍存 · `New`） | 正向 | root 写 `AGENT.md = "项目约定 🎉"`、`provenance.source = "archi"` → `begin` 后：① `runner.captured_injections()[0].preamble == Some(freeze(知识, load 产物))`（本轮内存体）；② `store.find_session_preamble(running.session_id).content` 与 ① 逐字相等；③ `saved_at > 0` | 单点 |
| `begin`（首轮拍存 · `New`） | 正向 | 对照臂：`provenance.source = "debug"`（无知识段）→ 正文 = `AGENT.md` 段 + `\n\n` + `GUIDANCE`，首位无空行 | 参数化（来源 4 臂：`archi` / `config` / `change` / `debug`） |
| `begin`（首轮拍存 · `New`） | 异常 | 落库失败不中断：注入 user 维度 `Store`（`Store::open_global`）使 `stamp_session_preamble` 走 `Err` → `begin` 仍 `Ok`、本轮注入位仍为内存体、`running.session_id` 可用（尽力语义 + 日志留痕，不上抛） | 单点 |
| `begin`（首轮拍存 · `New`） | 边界 | 落库顺序判据：`compose.rs` 文本中 `stamp_session_preamble(` 的出现位置晚于 `begin_turn(`；且库行主键 == `running.session_id`（`New` 路径 id 由 kernel 铸造 ⇒ 落库不可能更早） | 单点（源码自证 + 载荷断言） |
| `begin`（首轮拍存 · `Continue` + `is_unstarted`） | 正向 | 预建空会话（`create_session`，无句柄 / 无轮行 / 无转录）+ `Continue` → 快照行落库且正文与 `New` 同口径（逐字）；同时既有 `restamp_session_config` 仍生效 | 单点 |
| `begin`（首轮拍存 · `Continue` + `is_unstarted`） | 异常 | 判据只查一次：`is_unstarted_session` 在 `begin` 内恰调用一次（源码自证 `fn begin` 函数体命中数 == 1），且 `begin_turn` 之后不再查询（轮行已落，二次查询恒假） | 单点（源码自证） |
| `begin`（首轮拍存 · `Continue` + `is_unstarted`） | 边界 | 畸形库形态（有转录无轮行 ⇒ `is_unstarted` 为假）：走续轮读回分支，不重复补拍（判据单一来源，不另立第二套） | 参数化（2 形态：有轮行 / 有转录） |
| `begin`（续轮读库） | 正向 | 先 `begin(New)` 得会话 id，再 `stamp` 不参与（首轮已写）；随后改 root 的 `AGENT.md` 为另一内容 → `begin(Continue{id})` 的注入位 == 首轮快照正文（逐字），库行不变 | 单点 |
| `begin`（续轮读库） | 正向 | 手工 seed 快照行 `"冻结体 v1"` 于已开轮会话 → `Continue` 注入位 == `"冻结体 v1"`（覆盖 `assemble_injections` 产出的知识段） | 单点 |
| `begin`（续轮读库） | 异常 | 跨引擎续会话守卫不被快照分支绕过：已开轮会话的 `config_snapshot.engine` 与当前 store 引擎不符 → 既有 `Err("会话非当前引擎产出（跨引擎续会话不支持）: {id}")` 文案逐字不变 | 单点 |
| `begin`（续轮读库） | 异常 | `Continue` 未知 id → `Err("会话不存在: {id}")`（既有契约文案，MUST NOT 被 `is_unstarted_session` 的内部 `Err` 顶替） | 单点 |
| `begin`（续轮读库） | 边界 | 空正文快照（`stamp(id, "")`）→ 注入位 `None`（`injectable` 把空串归零）；此后 `AGENT.md` 出现也不改本会话（仍 `None`） | 单点 |
| `begin`（存量补拍） | 正向 | 已开轮会话（`create_session` + `begin_agent_turn`）无快照行 → 首次 `Continue` 注入位 = 现算体、库中新增行（逐字相等） | 单点 |
| `begin`（存量补拍） | 正向 | 补拍一次性：首次补拍后改 `AGENT.md`，第二次 `Continue` → 注入位仍为首次补拍体、库行 `content` 与 `saved_at` 均不变 | 单点 |
| `begin`（存量补拍） | 异常 | 补拍落库失败不中断：`New` 路径注入 user 维度 store 使 `stamp` 走 `Err` → `begin` 仍 `Ok`、本轮注入位为内存体（`Continue` 路径的失败注入会先在 `find_session` 处 `Err`，故补拍失败降级以本行 + 下条结构判据共同锚定） | 单点 |
| `begin`（存量补拍） | 边界 | 重试口唯一：`begin` 内 `stamp_session_preamble` 调用点恰一处（落库后置），补拍分支入口唯一（`find_session_preamble == None`）⇒「落库失败的会话下次 `Continue` 再补一次」由存量补拍正向用例结构性覆盖（源码自证调用点计数 + 补拍用例） | 单点（源码自证） |
| `begin`（存量补拍） | 边界 | 补拍取「补拍当轮」的知识：先移除 root 的 `AGENT.md` 再补拍 → 快照仅知识 + 常量；补拍后再写文件不改本会话 | 单点 |
| `begin`（读取口径） | 边界 | 截断只作用于 `AGENT.md` 段：root 写 40KB `AGENT.md` → 库中正文的 `<agent_md>` 段为 32KB char 边界前缀、知识段完整、常量完整（总长 < 40KB + 知识 + 常量） | 单点 |
| `begin`（读取口径） | 边界 | 读取根 = `ctx.workspace_root`（worktree 即分支版）：以与 `compose_turn` 的 `workspace_root` 入参不同的 `SessionCtx.workspace_root` 调用 → 快照取 `SessionCtx` 那只根的 `AGENT.md`（读取根与实例/主仓根解耦） | 单点 |
| `begin`（快照体量） | 边界 | 总量 > 32KB 的正文原样落库（>32KB 不报错、不二次截断）：断言回读长度守恒 | 单点 |

**既有用例连锁（回退修红）**：

- `archi臂调用点链路捕获与直调全等`：判据从「捕获 == `injections_of` 直调值」改为「捕获 == 装配面 + 快照覆盖」——`begin` 已把 `preamble` 位覆盖为冻结体，期望 `SessionInjections { preamble: Some(preamble::freeze(装配面 preamble, preamble::load(root))), ..injections_of("archi", None) }`，其余位逐字段不变；用例环境无 `AGENT.md` 时即 `知识 + "\n\n" + GUIDANCE`。此为「组合根零二次改写」判据在新语义下的正确形态（覆盖恰一处、其余位恒等）。
- 本章新增用例经 `composed_with_fakes`（既有测试侧装配缝：`RecordingRunner` / `RecordingSink`）观测注入位与落库面；`compose_turn` 真 Sdk runner 的既有用例沿用 dummy 端点（`http://127.0.0.1:9/v1`）不触网，仅作全链回归。
- `assemble_injections` 各臂用例（直调 `injections_of`，断言 `preamble` 取知识或 `None`）保持绿——该函数零改动，快照覆盖只发生在 `begin`。

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `AGENT.md`（文件系统） | 不 mock：tempfile 真实写入 / 改写 / 删除 | 首轮、续轮「改文件不影响」、补拍、截断用例 |
| `Store`（持久面） | 不 mock：tempfile 真 redb 库；失败注入用真实 user 维度实例（`Store::open_global`）而非伪造 `StoreError` | 落库、回读、失败降级用例 |
| `AgentRunner`（端口，显式注入依赖） | `RecordingRunner` 假件：捕获 `SessionOpen.injections` / `prior_handle`（既有装置） | 注入位逐字断言 |
| `SessionSink`（端口，显式注入依赖） | `RecordingSink` 假件：`create_session` 载荷与 `begin_turn` 计数（既有装置） | 落库顺序与轮行计数 |
| 引擎网络（进程边界） | 不 mock 真实 Sdk runner 的用例沿用 dummy 端点 `http://127.0.0.1:9/v1`（不触网） | `compose_turn` 全链回归用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop_test.rs

**框架与风格**：`#[tokio::test]` + `FakeModel` 假流（`captured_requests()` / `system_instructions()`）+ `drive_loop_turn`；源码自证读 `loop.rs`（既有 `compose_test` 同型先例）。

#### 待测功能

- 系统消息一元消费（design 修改文件表）：`turn.injections.preamble` 在场 → `chat_history` **恰一条** `Message::System` 且逐字；缺席 → 零 System 消息；`system_prompt` 二进制形参退役、引擎执行体零文件读取

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 一元 System 消费 | 正向 | `injections.preamble = Some("知识\n\n<agent_md …>\n\n指引")` + root 写有内容不同的 `AGENT.md` → 首轮请求 `system_instructions() == Some(preamble)` 逐字（不被文件内容覆盖或追写） | 单点 |
| 一元 System 消费 | 正向 | 两轮驱动（首轮工具调用 `read` + 续轮直答）：`requests[0].system_instructions() == requests[1].system_instructions()` 逐字相等（快照成立的结构证明，AC-2） | 单点 |
| 一元 System 消费 | 正向 | 中途改文件不改前缀（AC-3）：`rewrite_file_on_first_stream` 在首轮流后改写 `AGENT.md` → 次轮 System 与首轮逐字相同 | 单点 |
| 一元 System 消费 | 异常 | `preamble = None` → 两轮请求均零 `Message::System`（不注入空串） | 单点 |
| 一元 System 消费 | 异常 | 边界形态 `Some("")`：按 `if let Some` 原样推一条（loop 零空串判定，空串由装配层 `injectable` 拦截）——锁住「loop 不新增第二套归一化」 | 单点 |
| 一元 System 消费 | 边界 | 超长 preamble（>32KB 中文）→ System 长度守恒（loop 零截断） | 单点 |
| 一元 System 消费 | 边界 | 特殊字符（`\n` / `\0` / emoji / 段间多空行）逐字保留 | 单点 |
| 一元 System 消费 | 边界 | 位置契约：`chat_history[0]` 为 System，其后历史逐条与 `history` 逐字相等（不重排、不追加第二条） | 单点 |
| 一元 System 消费 | 边界 | 假流双形态（单轮直答 / 工具续轮）与两权限档（`bypassPermissions` / 默认）组合 → System 数与内容不变 | 组合遍历（2×2） |
| 模块面（源码自证） | 边界 | `loop.rs` 文本：`AGENT.md` / `preamble::load` / `system_prompt` 三字样零命中；`turn.cwd` 的三处既有使用仍在场（沙箱 / 写保护 / 工具执行，`cwd` 字段不删） | 单点（源码自证） |

**既有用例连锁（回退修红）**：

- `db分派两工具到执行体且结果非错误`：`DB_TABLES_TOOL` 分支的 `rows.len() == 15` → 16（user 4 + workspace 12；`session_preamble` 登记行生效）。该用例是 `db_tables` 全量清单在 loop 分派面的锚点，清单行数变即红。
- `system_prompt` 的纯函数用例（双在场 / 仅知识 / 仅 `agent_md` / 皆无 / 空串入参 / 特殊字符 / 超长 / 纯函数性）随函数退役删除，其语义由 `preamble_test.rs` 的 `freeze` 用例承接。
- 系统消息的「改写生效」用例语义反转为上表第 3 行（改写**不**生效，`system_instructions` 两轮逐字相同）；「`preamble` 缺席零 System」用例保留并改由 `injections.preamble = None` 驱动（不再依赖无文件态）。

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 模型 / 网络（进程边界） | `FakeModel` 假流替身（既有装置：脚本化「文本 + 工具调用 + 收口」序列，捕获请求） | 全部运行态用例 |
| 文件系统 | 不 mock：tempfile 真盘写 `AGENT.md`（「改写不影响」正是断言对象） | 一元消费与改写用例 |
| 无其他 | `LoopTurn` 由测试直接构造（`injections` 显式入参） | 全章节 |

---

### packages/desktop/src/types/generated/bindings.ts -> packages/desktop/src-tauri/src/bindings/mod_test.rs

**框架与风格**：`#[test]` + `authoritative_snapshot()`（`export_bindings()` 后读盘）+ `type_section(content, name)` 切片断言 + 跨进程文件锁串行 + `RestoreOnDrop` 恢复（既有双产物守卫同型）。

#### 待测功能

- 生成产物出线面（design 修改文件表：`SessionPreambleInfo` 类型 + `SessionSummary.preamble: SessionPreambleInfo | null`，MUST NOT 手改）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `bindings.ts` 类型段 | 正向 | `type_section("SessionPreambleInfo")` 含 `savedAt: number,` 与 `content: string,`（两字段，无多余键）；`type_section("SessionSummary")` 含 `preamble: SessionPreambleInfo \| null,` 且既有 `row` / `stats` / `turns` 三行逐字不变 | 单点 |
| `bindings.ts` 类型段 | 正向 | `DTO_TYPES` 名单补 `SessionPreambleInfo` 后与产物 `export type` 全集逐一相等（顺序按既有名单约定） | 全量遍历 |
| `bindings.ts` 类型段 | 异常 | 篡改产物（删 `preamble` 行 / 把类型改名为 `SessionPreamble`）→ 断言抓且文案指认缺项；`RestoreOnDrop` 在 unwind 后重导出恢复权威产物（既有篡改用例同型） | 参数化（2 形态） |
| `bindings.ts` 类型段 | 异常 | 命令面零新增：`COMMANDS` 名单零 diff（无 `session_preamble` / `session_preamble_reload` / `session_preamble_status`）；`sessionDetail` 包装逐字不变（`(root: string, sessionId: string) => __TAURI_INVOKE<SessionSummary \| null>("session_detail", { root, sessionId })`）；产物无 `Result<` 包装 | 单点 |
| `bindings.ts` 类型段 | 边界 | 确定性：连续两次 `export_bindings` 产物逐字节相等（零 diff 门）；`SessionRow` / `SessionStats` / `TurnSummary` 三段逐字不变 | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock） | 真实 `export_bindings()` 再生成 + 读盘；产物独占由 OS 文件锁串行（跨进程） | 全章节 |

---

### packages/desktop/src/types/generated/bindings.zod.ts -> packages/desktop/src-tauri/src/bindings/mod_test.rs

**框架与风格**：`#[test]` + `authoritative_zod_snapshot()` + `zod_section(content, name)` 切片断言（既有 zod 快照用例同型）。

#### 待测功能

- zod 镜像随动（design 修改文件表）：`SessionPreambleInfoSchema` 新增 + `SessionSummarySchema.preamble` 为 nullable

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `bindings.zod.ts` schema 段 | 正向 | `zod_section("SessionPreambleInfoSchema")` 为 `z.strictObject` 且两 getter：`get savedAt() { return z.number(); }`、`get content() { return z.string(); }`；`SessionSummarySchema` 含 `get preamble() { return SessionPreambleInfoSchema.nullable(); }` | 单点 |
| `bindings.zod.ts` schema 段 | 异常 | 篡改（删 `get preamble()` / 把 `.nullable()` 去掉 / 把 `content` 改成 `z.number()`）→ 断言抓且指认；unwind 后重导出恢复 | 参数化（3 形态） |
| `bindings.zod.ts` schema 段 | 边界 | 零运行期耦合：zod 产物不 `import` `bindings.ts`；连续两次导出逐字节相等；多命令映射表（`resultSchemas.sessionDetail`）指向 `SessionSummarySchema.nullable()` | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock） | 同 `bindings.ts` 章节：再生成 + 读盘 + 文件锁 | 全章节 |

---

### packages/desktop/src/hooks/use-agent-session.ts -> packages/desktop/src/hooks/use-agent-session.test.ts

**框架与风格**：vite-plus `describe` / `it` / `expect` + `@testing-library/react` 渲染 hook + `invoke` / `Channel` mock + `driftCount()` 零漂移断言（既有文件同型）。

#### 待测功能

- `sessionFromTurn` 的 `SessionSummary` 字面补 `preamble: null`（design 修改文件表）：本地轮行镜像无快照面
- 重放路径 `applyReplay` 原样落 `session_detail.preamble`（design 修改文件表：零其他改动即贯通）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `sessionFromTurn`（early-resolve 镜像） | 正向 | 发送提问 → `RunStarted` 回写本地轮行镜像：`state.session.preamble === null`；`row` / `stats` / `turns` 与既有投影逐字段一致 | 单点 |
| `sessionFromTurn`（early-resolve 镜像） | 异常 | `session_detail` 返回 `null`（无会话）→ `state.session === null`，无 preamble 读取路径、无异常抛出 | 单点 |
| `sessionFromTurn`（early-resolve 镜像） | 边界 | 零 zod 漂移：整条链路（`session_detail` 返回带 `preamble` 的对象）后 `driftCount() === 0`（fixture 补 `preamble` 后严格对象不缺键） | 单点 |
| `applyReplay`（重放） | 正向 | `session_detail` 返回 `preamble: { savedAt: 1727000000000, content: "快照正文" }` → `state.session.preamble` 逐字段相等（不裁剪、不改写） | 单点 |
| `applyReplay`（重放） | 异常 | 返回体 `preamble: null` → 镜像 `preamble === null`（不伪造空对象 / 不填 `{savedAt:0, content:""}`） | 单点 |
| `applyReplay`（重放） | 边界 | 超长正文（>32KB 含中文与换行）→ 镜像逐字保留（长度守恒，零截断）；空串 `content: ""` 形态 → 保留空串（≠ `null`） | 参数化（2 形态） |
| `applyReplay` / `withTurns` | 边界 | 重放后追加轮行（`withTurns`）→ `preamble` 不被 `stats/turns` 现算清空或改写（展开语义保留其余字段） | 单点 |

**既有用例连锁**：`use-agent-session.test.ts` 的 `sessionFixture` / `detail` fixture 需补 `preamble` 键（否则 `driftCount()` 的非零断言用例之外会出现意外漂移计数）；`use-agent-session.throttle.test.tsx` 如复用同一 fixture 工厂则随之贯通。

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| Tauri IPC（`@tauri-apps/api/core` 的 `invoke` / `Channel`，进程边界） | `vi.mock` 按命令名分发的 `invokeMock` + `ChannelMock` 直投信封（既有装置） | 全部用例 |
| 无内部模块 mock | `agent-transport` / `ipc-drift` / `ipc` 真实组合（漂移计数正是真实 collector 读数） | 全部用例 |

---

### packages/desktop/src/components/agent/agent-session-view.tsx -> packages/desktop/src/components/agent/agent-session-view.test.tsx

**框架与风格**：`// @vitest-environment jsdom` + `render` / `screen` / `within` / `fireEvent` / `waitFor`；抽屉断言沿 `standard/drawer.test.tsx` 口径（`getByRole('heading', { name })` + `findByRole('button', { name: '关闭' })`，base-ui 真实渲染）。

#### 待测功能

- `SessionMeta` 入口（design 修改文件表 / `desktop-agent-chat-infra` delta）：`summary.preamble !== null` → 渲染「系统提示词」入口（`data-testid="session-meta-preamble"`，可访问名）；`null` → 不渲染、不占位
- `PreambleDrawer`（design「类型定义」表新增，**私有**）：自持开合态 + `StandardDrawer`（`title="系统提示词快照"`）+ 三段内容（拍存时刻 / 体量 / 静态指引 / 正文）；只读、零取数、不进对话流

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `SessionMeta` 入口 | 正向 | `session_detail` 回 `preamble` 非空 → `screen.getByTestId('session-meta-preamble')` 在场、可访问名「系统提示词」；其余元信息位（id / 状态徽章 / 轮数 / tokens）逐字不变 | 单点 |
| `SessionMeta` 入口 | 正向 | 点击入口 → 抽屉打开：`getByRole('heading', { name: '系统提示词快照' })` + footer 单「关闭」按钮（无「提交」/「取消」） | 单点 |
| `SessionMeta` 入口 | 异常 | `preamble: null`（存量会话 / 首轮未重放）→ `queryByTestId('session-meta-preamble')` 为 `null`、无占位元素；会话其余呈现与既有一致 | 单点 |
| `SessionMeta` 入口 | 边界 | 先 `null` 后重放带回 `preamble` → 入口由缺席转为在场（重放收口后自然出现，无脏态残留） | 单点 |
| `PreambleDrawer` 内容 | 正向 | 拍存时刻呈现 == `new Date(1727000000000).toLocaleString()`；体量呈现 == `new TextEncoder().encode(content).length` 字节（用中文 `content` 区分字节数与 UTF-16 长度，如 `"中文"` → `6 字节`） | 单点 |
| `PreambleDrawer` 内容 | 正向 | 静态指引文案在位：含「冻结」语义、`AGENT.md`、重新读取（`read`）三要点；与抽屉正文区的快照全文互不混排 | 单点 |
| `PreambleDrawer` 内容 | 正向 | 正文全文逐字：抽屉正文容器（`<pre>`，等宽 + `whitespace-pre-wrap`）的 `textContent` 与 `preamble.content` 逐字符相等（保留换行与空行，零改写） | 单点 |
| `PreambleDrawer` 内容 | 异常 | 零写面：抽屉内 `queryAllByRole('textbox')` 为空、无「重载」/「刷新」/「保存」按钮；`invokeMock` 在开合前后调用次数不变（零新增 IPC、零订阅） | 单点 |
| `PreambleDrawer` 内容 | 异常 | 不进对话流：开合抽屉前后消息数与部件数零变化；`content` 不出现在消息列表容器内 | 单点 |
| `PreambleDrawer` 内容 | 边界 | `content` 空串 → 抽屉仍可打开、体量呈现 `0 字节`、正文区空（不塌陷成 `null` 态） | 单点 |
| `PreambleDrawer` 内容 | 边界 | 超长 `content`（>32KB，含多段换行与 emoji）→ 全文逐字（长度守恒）、`<pre>` 承载于抽屉既有 `ScrollArea`（不额外嵌套滚动） | 单点 |
| `PreambleDrawer` 内容 | 边界 | 关闭按钮点一次 → 抽屉收起（heading 缺席），再开一次仍呈现同一份内容（自持开合态无状态丢失） | 单点 |

**既有用例连锁**：本文件 `summary()` fixture 补 `preamble` 参数（缺省 `null`，新增用例传 `{ savedAt, content }`）；既有「元信息行四要素」类用例断言保持绿（入口为追加位，不改既有位）。

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| Tauri IPC（`@tauri-apps/api/core`，进程边界） | `vi.hoisted` 的 `invokeMock`（按命令名分发 `session_detail` / `agent_session_transcript` / `agent_session_watch`）+ `ChannelMock`（既有装置） | 全部渲染用例 |
| 无内部模块 mock | `useAgentSession` / `AgentMessages` / `message-cards` 及 `StandardDrawer`（base-ui）全部真实组合——组件即链路入口 | 全部用例 |

---

## 不可测试项

| 对象 | 原因 | 兜底 |
|---|---|---|
| `packages/desktop/package.json` 版本行（`0.7.7` → `0.7.8`） | 无代码行为面，单测无判据；`tauri.conf.json` 经 `"version": "../package.json"` 跟随、`src-tauri/Cargo.toml` crate 版本行不动 | AC-13 在归档时人工核对；发布由 `.github/workflows/desktop-release.yml` 承担 |
| `.dev-team/architecture/models/Desktop/Components/Agent/Desktop.Components.Agent.AgentSessionView.md`（职能段 +1 句） | 契约资产 prose，无运行期行为面；架构模型已锁，本相位 MUST NOT 触碰模型文件 | 静态门 `pnpm -C packages/desktop run check`（`api:check` 零 diff）与架构页「契约」视图 |
| `openspec/changes/session-preamble-snapshot/specs/**`（5 份 delta 的计数与措辞） | 文档面（`db_tables` 计数已在 `db_tool_test.rs` 机械锚定，其余为语义叙述） | 归档合并时的 spec 审阅；`desktop-change-chat` 措辞无代码面 |
| `packages/desktop/src/types/generated/**` 的「不许手改」判据 | 生成物本身不可手写测试；手改即「零 diff 门」失败 | 由 `bindings/mod_test.rs` 的「连续两次导出零 diff」+ 篡改恢复用例间接覆盖（已在上文两章登记） |
| `crates/core/agent/src/runner.rs`（`SessionInjections.preamble` doc 反转） | design「公共函数 / API」表无该文件行（仅 doc，字段面零结构改动），按规则留章节框架不虚构条目 | 六位字段面回归由既有 `runner_test.rs` 全绿保证 |
| 8 个前端 `SessionSummary` 夹具连锁文件（`session-panel.test.tsx` / `index.test.tsx` / `agent-transport.test.ts` / `agent-debug-view.test.tsx` / `archi-chat-panel.test.tsx` / `change-chat-panel.test.tsx` 等） | 对应源文件（`session-panel.tsx` 等）本变更未修改 ⇒ 无被测面变更；仅需补 `preamble: null` 键 | AC-12 全量回归（`vp test`）与 `driftCount()` 零漂移断言 |
| `packages/desktop/src/lib/ipc.ts` / `ipc-drift.ts` | Proxy 形态自动覆盖 DTO 字段变化，零改动零测试面 | 结果面 soft-report 由 `ipc-contract.test.ts` 的类型面 Equal 断言覆盖 |
