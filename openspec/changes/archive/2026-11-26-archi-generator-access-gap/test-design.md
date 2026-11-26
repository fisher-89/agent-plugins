# 测试设计: archi-generator-access-gap

> **日期**: 2026-11
> **依据**: `proposal.md`（AC-1 – AC-16、风险 R1 – R10）+ `design.md`（C1 – C10、变更清单「新增 0 / 修改 20」、公共函数 / API 四条、类型定义、附录 A – E）+ 本目录九份 delta spec。
> **范围**: design.md「修改文件」表全量（新增文件为 0，删除文件为 0）；design.md「零改动面」表不在被测范围（其中 `crates/infra/archi/src/api_artifacts.rs` 作为写盘单点由 `commands/archi` 的等价性用例**间接**承载，见交叉承载注记）。
> **性质**: 本条只设计单元测试范围与断言口径，不写测试运行命令、不产出评估结论。

---

## 测试框架与路径约定（框架识别结论）

- **Rust（`src-tauri` 各 crate + shell 命令层）**：cargo 内建测试框架；测试文件与源文件同目录、同前缀的 `*_test.rs` 模块，以源文件或模块根内的 `#[cfg(test)] mod *_test;` 声明（如 `sdk/mod.rs` 声明 `mod archi_tool_test;`、`compose.rs` 声明 `mod compose_test;`、`api/mod.rs` 声明 `mod mod_test;`）。断言面为 `assert!` / `assert_eq!` / `assert_ne!` / `matches!` + `serde_json::Value` 线面对拍；异步用 `#[tokio::test]`。`crates/core/archi` 与 `crates/core/workflow` 无 `tempfile` dev-dep 的模块以 std 自建 `TempDir` RAII（既有装置）。
- **前端（`packages/desktop/src`）**：`vite-plus/test`（`describe` / `it` / `expect` / `vi` 自 `vite-plus/test` 导入）+ `@testing-library/react`；测试文件与源文件同目录 `*.test.tsx`；查询锚点一律 `data-testid`；进程边界 mock 统一走 `vi.mock('@tauri-apps/api/core')`。
- **Mock 纪律（沿仓库既有）**：Rust 侧零框架级 mock——盘面一律 tempdir 真件、`ProtectRules::load` / `archi::workspace_render` / `write_artifacts` 真实组合；**唯一允许的替身是本变更新增端口 `archi::ArtifactWriter` 的 test-local 实现**（它是被测 API 的显式注入依赖，`ArchiContext::with_writer` 直接入参），且替身只记录调用与回传构造值，不复制渲染 / 比对 / 写盘任一步。前端仅 mock `invoke` 进程边界。
- **源码结构锚**：仓库既有的大量「零第二份实现 / 单点 / 计数」不变量以 `include_str!("<源文件>")` + 字面计数断言承载（`mount_test` / `compose_test` / `change_flow::mod_test` / `api::mod_test` 皆有先例），本节沿该形态标注为「源码锚」。
- 以下路径均相对项目根；Rust 侧前缀 `packages/desktop/src-tauri/`，前端前缀 `packages/desktop/`。

---

## 验收范围

> 「被测文件或模块」列填 design.md 变更清单里的源文件（模块路径相对项目根），对应测试文件见「单元测试」各节 `->` 标题；纯再导出 / 静态配置 / 管线守线项落「不可测试项」。

| AC ID | 验收条件 | 被测文件或模块 |
|-------|---------|----------------|
| AC-1 | 工具在册且档位正确：`mount::registry()` 恰 19 项（七内建 + `create_explore_note` + 五 archi 只读 + 两 archi 写 + `archi_generate` + 两 DB + `propose_config`）；`archi_generate` 为 `ToolTier::Write` / `ToolExec::Archi`、`parameters` 零属性、`default` 拒 / `acceptEdits` 放行；五只读与两模型写档位逐字不变；`tools::input_path` 对新工具恒 `None`（入参夹带 `path` 亦然） | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools.rs` |
| AC-2 | 注入矩阵逐格与收窄后的不变量：八行逐行命中（两起草相位含 `archi_generate`、`(test-design, executor)` 恰 `archi_search` + `archi_module`、`(implement, executor)` 零 `archi_generate`、archi 臂八名）；「矩阵与写权锁同集」收窄为两**模型**写工具且双向断言，`archi_generate` 不入对拍集 | `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs` |
| AC-3 | 生成链三态：`written` / `unchanged` 与盘上字节一一对拍（字节相同者在 `unchanged` 且 `mtime` 不变）；二次调用 `written` 空；任一目标命中 `ProtectRules` ⇒ `conflicts` 非空、`written` 与 `unchanged` 皆空、磁盘零变化且工具仍 `Ok`；`root` 取会话 cwd（change 会话写 worktree、主仓零变化）；端口经五跳 builder 链从组合根注入执行体 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/lib.rs`、`packages/desktop/src-tauri/src/commands/archi/mod.rs`、`packages/desktop/src-tauri/src/commands/exec/mod.rs`、`packages/desktop/src-tauri/src/commands/change_flow/mod.rs` |
| AC-4 | 端口分层与写盘单点：`archi::ArtifactWriter` 定义在 `core/archi` 且该 crate 零 `fs::write` / 零 `create_dir_all` / 零 `std::process`；唯一实现在壳层 `commands/archi` 且函数体只调 `write_artifacts` 一次；`agent-runtime` 依赖中零 `archi-runtime`；产品源码零第二份「生成物 `fs::write`」 | `packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs`、`packages/desktop/src-tauri/crates/core/archi/src/lib.rs`、`packages/desktop/src-tauri/src/commands/archi/mod.rs` |
| AC-5 | 零产出与孤儿如实：三种零产出成因各自不报错且零生成物越界（口径校准见下方注记）；工具描述含「零产出不是失败」「孤儿生成物永不删除」「调用前先 `git status`」三句；`orphaned` 照实回显、工具零删除（调用前后盘上文件集相同） | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs` |
| AC-6 | `remedy` 三态与零出线面：`check` 在三场景下 `remedy` 分别为 `null` / 指向 `archi_generate` / 指向架构页会话且含「MUST NOT 手改」；键恒在场；`artifacts` / `findings` 两轴内容不受影响 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool.rs` |
| AC-7 | L3 锁面与拒串：六类非白名单会话写 `models/**` 一律被拒（`write` / `edit` 两路）；三处白名单格放行；拒串取 `CONTRACT_LOCK_TEMPLATE`（路径、`%t`、「只在起草相位执行侧与架构页会话写入」、三条出路、不得绕过）且与模型锁串逐字不同 | `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/reason.rs` |
| AC-8 | 生成物与模型锁面分离：非写权相位写 `<archi_dir>/graph/INDEX.toml`、`<archi_dir>/config.json`、`.i.*` 仍放行；`models/**` 是**会话规则**而非内置规则（内置清单仍恰 `config.json` 一条） | `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs` |
| AC-9 | 悬空指代零残留：产品源码与 prompts 对「本仓的生成命令」「既有命令重生成」两种字面零命中；`contract-panel.tsx` 新串逐字；知识骨架含 `archi_generate` 且骨架 ≤ 1831 B / 全文 ≤ 2031 B（实测值重冻、门限不抬）；`archi_api` 描述第 ③ 句换指 | `packages/desktop/src-tauri/crates/infra/agent/src/archi-knowledge.md`、`packages/desktop/src-tauri/crates/infra/agent/src/archi_knowledge.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs`、`packages/desktop/src-tauri/crates/core/workflow/src/prompts/implementation-generator.md`、`packages/desktop/src-tauri/crates/core/workflow/src/prompts/dev-design-planner.md`、`packages/desktop/src/views/architecture/components/dashboard/contract-panel.tsx` |
| AC-10 | 契约作者纪律四处生效：`implementation-generator.md` 三要素齐备且零「同批同步接口定义」正句；两 planner 含 `archi_generate` 行与零产出正常一句、`dev-design-planner.md` 零「实现相位同一条」；`proposal-planner.md` 含 md 责任句 + `contractPath` 取路径句；`test-design-planner.md` 含 `archi_module` 句与 grep 禁令且 4b 原文逐字仍在 | `packages/desktop/src-tauri/crates/core/workflow/src/prompts/implementation-generator.md`、`packages/desktop/src-tauri/crates/core/workflow/src/prompts/dev-design-planner.md`、`packages/desktop/src-tauri/crates/core/workflow/src/prompts/proposal-planner.md`、`packages/desktop/src-tauri/crates/core/workflow/src/prompts/test-design-planner.md` |
| AC-11 | 测试面必改断言全绿：18 → 19 计数、`compose_test` 的「恰两条」与收窄对拍、`runner_test` 的八名判据、`phase_table_test` 的 prompt 断言逐处改后全绿且不删断言 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/compose.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs`、`packages/desktop/src-tauri/crates/core/workflow/src/prompts/*.md` |
| AC-12 | 零回归读数：`pnpm api:check` 零差异、`.dev-team/architecture/**` 零字节变化、`apiFindings` 计数与构成不变 | 生成物 / 模型零改动面（`crates/core/archi/src/api/{render,check,paths}.rs`、`crates/infra/archi/src/api_artifacts.rs`、`.dev-team/architecture/**`）——静态与管线守线，见「不可测试项」 |
| AC-13 | 版本与改动集：`package.json` 相对落地时 `HEAD` 恰 +1 patch；`src-tauri/Cargo.toml` 与 `tauri.conf.json` 逐字不变；改动集 ⊆ 提案清单（按 design D-9 / D-10 读作少一项 `sdk/mod.rs`、多一项 `sdk/tools.rs`） | `packages/desktop/package.json`（静态核对，见「不可测试项」） |
| AC-14 | spec delta 形态：九份能力目录、`RENAMED` 的 `FROM:` / `TO:` 与基线锚一致、MODIFIED 整段拷贝未丢条目、每条 Requirement 至少一个 Scenario 且标记恰四个 `#` | `openspec/changes/archi-generator-access-gap/specs/**`（openSpec 校验面，见「不可测试项」） |
| AC-15 | 幂等与并发安全：同会话连续两次 `archi_generate` 第二次 `written` 为空且 `mtime` 不变；不新增锁机制；生成失败（`workspace_render` 结构错）⇒ `is_error` 且磁盘零变化 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool.rs`、`packages/desktop/src-tauri/src/commands/archi/mod.rs` |
| AC-16 | 画布仍只读：`full` 模式契约视图零生成按钮、源码零 `archiApiGenerate` 调用路径（既有断言逐字不变）；页内会话生成能力由 `archi_generate` 承载（画布零入口不变） | `packages/desktop/src/views/architecture/components/dashboard/contract-panel.tsx` |

> **交叉承载注记**：
> - 写盘单点的字节语义（只增只改、从不删、非原子、越根复核）归 `crates/infra/archi/src/api_artifacts.rs`（本变更零 diff），由 `commands/archi` 的 `ArchiArtifactWriter` 等价性用例与既有 `archi_api_generate` 用例**间接**承载，不另立被测章节。
> - 沙箱改写面（`sdk/sandbox.rs`）与判定面（`sdk/protect/{rules,shell,inline}.rs`）零改动：`archi_generate` 无 `path` 入参的「原样返回」由 `tools_test.rs` 的 `input_path` 用例覆盖，不在沙箱侧新增断言。
> - `policy_test.rs` / `config_propose_test.rs` 的 `18 → 19` 属既有测试面的计数随动，挂靠在 `sdk/mount.rs` 章节的「静态清单与计数单点」用例组（同一被断言对象 = 挂载清单）。
>
> **AC-5 口径校准注记（须在实现相位落实为断言，而非照抄 AC 字面）**：`render_all` 在逐元素渲染之后**恒追加一份 `INDEX.toml`**（`crates/core/archi/src/api/mod.rs::render_all` 尾部无条件 `files.push`），故「三种零产出成因 ⇒ `written` 与 `unchanged` **皆空**」在盘面可达形态下不可成立。本节把该 AC 的判据收敛为可断言形态：**三种成因下 `written` 与 `unchanged` 中零 `.i.*` 条目、`INDEX.toml` 按字节比对落位、`is_error` 为假、磁盘除 `INDEX.toml` 外零变化**。该收敛不放松 AC 语义（防的是「零产出被当成工具坏了」），但 spec 与 proposal 的字面需在归档前同步（归 dev-design / 提案面）。

---

## 单元测试

### packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/mod_test.rs

#### 待测功能

- ArtifactWriter.write(): 端口唯一方法 `write(&self, root: &Path, files: &[GeneratedFile]) -> Result<ArchiApiWriteOutcome, String>`；纯类型声明，crate 内零实现、零 IO、零 specta 出线

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `ArtifactWriter.write` 签名与对象安全 | 正向 | test-local 桩实现 `impl ArtifactWriter`（记录 `(root, files)` 并回传构造好的 `ArchiApiWriteOutcome`）：以 `Arc<dyn ArtifactWriter>` 调用后收到 `root` 原值与 `files` 逐项（`path` / `content`）相等、回传值（`written` / `unchanged` 两列）原样，证明一方法的签名形态与 trait object 可调用 | 新增 |
| `ArtifactWriter.write` 签名与对象安全 | 边界 | `files = &[]`（空批）：桩实现被调用恰一次、收到零元素；端口不替调用方裁量空集（零产出合法性归调用方 `render_all`，端口 MUST NOT 以空集返 `Err`）；另 `root` 传不存在的路径亦零前置校验（端口不做 root 有效性判定，判定归壳层实现） | 新增 |
| `ArtifactWriter.write` 签名与对象安全 | 异常 | 桩实现返回 `Err("写入失败（…）")`：调用方拿到 `Err` 原文（错误面是 `String`，非 `ApiError`）、零 panic、无部分结果泄漏；`written` / `unchanged` 两列在 `Err` 路径上不可观测 | 新增 |
| 端口零写盘纪律（AC-4 源码锚） | 正向 | `include_str!("mod.rs")` 全文零命中 `fs::write` / `create_dir_all` / `std::process` / `Command::new`（端口是纯类型；crate 五条既有纪律第 5 条「契约面纯读」不因新增 trait 破口） | 新增 |
| 端口零写盘纪律（AC-4 源码锚） | 边界 | 现存 `fs::` 用法仍**只有读面**（`fs::read_to_string` 等），且新增端口段落内零 `fs::` 命中；`ArtifactWriter` 定义紧随 `GeneratedFile`（同文件内相邻，`mod.rs` 中 `struct GeneratedFile` 之后首个 `pub trait` 即它） | 新增 |
| 端口零写盘纪律（AC-4 源码锚） | 异常 | 反向禁字面：文件内零 `write_all` / 零 `OpenOptions` / 零 `remove_file`（防「换一种写盘 API 绕过字面锚」）；`ArchiApiWriteOutcome` 只经 `use crate::dto::…` 引入（不自造第二份结果类型） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `archi::ArtifactWriter`（端口） | test-local 桩实现（`mod_test` 内私有 struct + `impl`）：只记录入参并回传构造好的 `ArchiApiWriteOutcome`，不碰盘、不复制渲染 / 比对 / 写盘 | 签名与对象安全三例；本表余下用例走源码锚，零替身 |
| 无（真实组合） | 桩 `GeneratedFile` / `ArchiApiWriteOutcome` 用真实类型构造；源码锚读 `include_str!("mod.rs")` 同一份编译期字节 | 零写盘纪律三例 |

---

### packages/desktop/src-tauri/crates/core/archi/src/lib.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/mod_test.rs

<!-- design.md 未声明该文件的公共 API 变更（「公共函数 / API」表无 `crates/core/archi/src/lib.rs` 行；该文件只做出口再导出）。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更；出口面由下列用例以「经 crate 根的同一类型身份」承载，不得据源码导出声明补条目。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 出口面单点（`ArtifactWriter` 经 crate 根可达） | 正向 | `crate::ArtifactWriter` 可作 `dyn` 使用：`let _: Arc<dyn crate::ArtifactWriter> = Arc::new(Stub);` 编译通过并可调用 `write`——再导出不是第二处定义 | 新增 |
| 出口面单点（`ArtifactWriter` 经 crate 根可达） | 边界 | 类型同一性：`Arc<dyn crate::ArtifactWriter>` 与 `Arc<dyn crate::api::ArtifactWriter>` 可互赋值（同一 trait 的两种路径），证明根导出来自 `pub use api::{…}` 而非别名新建 | 新增 |
| 出口面单点（`ArtifactWriter` 经 crate 根可达） | 异常 | 反向锚：`lib.rs` 内零 `as ArtifactWriter` / 零二次 `pub trait ArtifactWriter` / 零 `type ArtifactWriter =`（禁别名与影子定义）；既有 `pub use api::{…}` 名单的其余名字逐字不变（出口面只增一名） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | 纯导出面：全部断言为编译期可导入性与源码锚（`include_str!("lib.rs")`），零替身 | 本节三例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount_test.rs

<!-- design.md 未声明该文件的公共 API 变更（`ARCHI_GENERATE_TOOL` 与 `archi_generate_definition()` 为 `pub(crate)` 内部缝，未列入「公共函数 / API」表）。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更；下列用例的被测对象是本变更新增挂载项的行为面（名称 / 档位 / 执行体 / 定义 / 插入位 / 描述），不是凭源码导出臆造的条目。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 静态清单与计数单点（19 项） | 正向 | `registry()` 名字序列逐值等于 `EXPECTED_NAMES`（`[&str; 19]`）：七内建 → `create_explore_note` → 五 archi 只读 → `archi_element` / `archi_relation` → **`archi_generate`** → `db_tables` / `db_query` → `propose_config`；`names.len() == 19`、`unique.len() == 19`；`lookup("archi_generate")` 命中，且对其余 18 名与四类未知名（`archi_elementx` / 空串 / 大写 / 尾空格）行为不变 | 修改 |
| 静态清单与计数单点（19 项） | 边界 | 前七项仍恰七内建（`tools: None` 取前七项的语义不变）；`propose_config` 仍为末项（`config_propose_test` 的「常量在挂载清单末项恰一处」随动为 `names.len() == 19` 且末项名不变） | 修改 |
| 静态清单与计数单点（19 项） | 异常 | 未知名注入面仍显式 `Err` 且文案携非法名（`lookup` 单点不被新项绕过）；`archi_generatex` / `archi_generate `（尾空格）/ `ARCHI_GENERATE` 查空；重复名零登记（`unique.len()` 全等） | 修改 |
| `archi_generate` 档位与执行体归属 | 正向 | 该挂载项 `tier == ToolTier::Write`、`exec == ToolExec::Archi`；与 `write` / `edit` / `archi_element` / `archi_relation` 同档（`policy_test` 的封闭集 `match name` 写档臂加入该名后三档读数为 `[false, true, true]`：`default` 拒 / `acceptEdits` 放 / `bypassPermissions` 放） | 修改 |
| `archi_generate` 档位与执行体归属 | 边界 | 五 archi 只读项档位逐字不变（全 `Read`）、两模型写仍 `Write`；`ToolExec::Archi` 成员由 7 项变 8 项，其余分类（`Pure` / `ExploreNote` / `Db` / `Config`）成员数不变 | 修改 |
| `archi_generate` 档位与执行体归属 | 异常 | 反向：该名不得落 `Read`（`archi_api` 仍 `Read`，两者不同臂——R10「矩阵扩面把只读工具带进写档」的反向锚）；不得落 `Protocol`（收口 / ask 位专属） | 新增 |
| `archi_generate` 定义面 | 正向 | `lookup("archi_generate")` 的 `parameters` 为 `{"type":"object","properties":{}}`（零属性、零 `required` 键）；描述**五句逐句在场**：① 用法与三字段（`written` / `unchanged` / `conflicts`）+ 幂等（字节相同不重写、`mtime` 不动）+ 无参数（root 取会话 cwd）；② 零产出三成因（入口未解析 / 接口面留白 / 未写 `.schema.json`）+「零产出是正常，不是失败」；③ 孤儿永不删除 + `orphaned` 非空属常态 + MUST NOT 为清空它手改生成物；④ 覆盖同名 `<入口主干>.i.*` + 调用前先 `git status` + 可 `git restore` 回滚；⑤ `conflicts` 非空即整批零写盘 + 报给用户 + MUST NOT 换工具重试 | 新增 |
| `archi_generate` 定义面 | 边界 | 描述零本仓命令名（`api:export` / `pnpm` / `bindings:export` 一类零命中）；`.schema.json` 出现处均与「手写 / 未写 / 不是生成物」同句（MUST NOT 说成生成物）；三类生成物点名齐备（`INDEX.toml` / `.i.rs` / `.i.ts`） | 新增 |
| `archi_generate` 定义面 | 异常 | 反向禁句：描述内零「MUST NOT 存在专用写生成物工具」一类旧绝对句、零「本仓」「生成命令」死指代；该名不进 `TOOL_NAMES` 七通用封闭集（长度与逐值不变） | 新增 |
| archi 八项连续成组与插入位 | 正向 | `registry()` 中 archi 八项在序列上连续（无 DB / config 项插入其间）：连续区间长度 8、首项 `archi_module`、末项 `archi_generate` | 新增 |
| archi 八项连续成组与插入位 | 边界 | 插入位 = `archi_relation` 之后、`db_tables` 之前；`definitions(Some(names))` 按清单序过滤时新名随位次出线（序即 `RunStarted.tools` 序） | 新增 |
| archi 八项连续成组与插入位 | 异常 | 反向：不得落在七内建之前（`definitions(tools: None)` 仍取前七项）；不得落在 `propose_config` 之后（末项单一性） | 新增 |
| `archi_api` 描述换指（AC-9） | 正向 | `lookup("archi_api")` 描述第 ③ 句含 `archi_generate`（「过期调 `archi_generate` 重生成」）与「MUST NOT 手改生成物」，三类生成物点名仍在 | 修改 |
| `archi_api` 描述换指（AC-9） | 边界 | 描述内零「既有生成命令」/「本仓的生成命令」字面；`archi_api` 仍 `ToolTier::Read` 且换指后零写盘条款仍在 | 修改 |
| `archi_api` 描述换指（AC-9） | 异常 | `.schema.json` 仍被明写为手写真相（不得因换指变成生成物）；三动作封闭（`check` / `bootstrap` / `link`）enum 与 `required: ["action"]` 逐字不变（零第 4 动作） | 修改 |
| 源码锚：计数与 doc 措辞 | 正向 | `include_str!("mount.rs")`：`Vec::with_capacity(19)` 恰一处、`with_capacity(18)` 零命中；两段 doc 的「十九项静态挂载清单」「十九个字面构造项」在场、含「十八项」者零命中 | 修改 |
| 源码锚：计数与 doc 措辞 | 边界 | `ToolTier::Write` doc 名单含 `archi_generate`（三写：两模型写 + 一生成物写）、`ToolTier::Read` doc 名单逐字不变；`ToolExec::Archi` doc 由「七个 archi 工具（五只读 + 两写）」改「八个（五只读 + 三写）」且计数与成员一致 | 修改 |
| 源码锚：计数与 doc 措辞 | 异常 | 反向：`ARCHI_API_TOOL` 常量 doc 内「重生成仍由会话经既有 `bash` 命令完成」整句零命中（已换指 `archi_generate`）；`mount.rs` 零第二份名字字面（除常量定义处外无 `"archi_generate"` 硬编码） | 修改 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | `registry()` / `lookup` / `definitions` 全为进程内纯函数与 `OnceLock` 静态表，零 IO / 零 spawn；`prompt` 用内存合成串；`orchestration::finish::{description, parameters}` 真实调用（铸造内容即被测面，替身会掩盖 enum 对拍） | 前六组用例 |
| 无（真实组合） | 源码锚读 `include_str!("mount.rs")` 同一份编译期字节（不复制副本） | 末组三例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool_test.rs

<!-- design.md 未声明该文件的公共 API 变更（`ArchiContext::{with_writer, with_generate}`、`generate_payload` 为 `pub(crate)` 内部缝，design「公共函数 / API」表已明列「内部缝，不计入公共 API」）。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更；下列用例的被测对象是本变更新增执行臂与 `remedy` 投影的行为面，不是凭源码导出臆造的条目。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `archi_generate` 执行链（渲染 → 逐份保护 → 端口写盘） | 正向 | 健康根（模型 + `api = declared` 元素的 `<叶段>.schema.json` 声明了 op + 入口源码在盘）：`execute("archi_generate", {})` 后响应三键齐备；替身端口收到的 `root` 逐字等于 `ArchiContext` 的 root、`files` 逐项（`path` / `content`）与 `archi::workspace_render(root)` 的产出**逐字相等**（执行体零重渲染、零改写、零重排）；响应的 `written` / `unchanged` 与端口回传值相等，`conflicts == []` | 新增 |
| `archi_generate` 执行链（渲染 → 逐份保护 → 端口写盘） | 边界 | 应生成集形态两例：① 无 `.schema.json` 的模型根 ⇒ `files` 仅 `INDEX.toml` 一份；② 三元素模型（一元素有 op / 一元素 `path` 空 / 一元素接口留白）⇒ `files` 恰 `1 + N` 份且 `path` 空的元素零条目；两例端口均被调用一次 | 新增 |
| `archi_generate` 执行链（渲染 → 逐份保护 → 端口写盘） | 异常 | 装配缺失两态：① `with_writer(None)` ⇒ `Err` 且文案点名装配缺失（`is_error` 回灌面，MUST NOT panic / MUST NOT 静默成功）；② `protect == None` ⇒ `Err`（fail-closed：无规则集不等于无规则）；两例端口零调用、磁盘零变化 | 新增 |
| 幂等与二次调用（AC-15） | 正向 | 同一根连续两次 `execute`：两次交给端口的 `files` 逐项字节相等（执行体零缓存、零跨调用状态）；替身第一次回 `written = all`、第二次回 `unchanged = all`，两轮响应如实转出——幂等判据单点归 `write_artifacts`，执行体不做第二份字节比对 | 新增 |
| 幂等与二次调用（AC-15） | 边界 | 生成物已被手改成旧内容（盘上字节 ≠ 应生成物）：执行体不预判「已新鲜」、仍整批交端口（端口才有比对权）；`mtime` 断言归壳层实现侧用例（见 `commands/archi`） | 新增 |
| 幂等与二次调用（AC-15） | 异常 | 端口返回 `Err("写入失败（…）不回滚")` ⇒ 工具 `Err`（`is_error`）、响应不出现三键；置换端口为零调用可验证「`Err` 前无第二次写盘尝试」 | 新增 |
| 逐份保护判定与整批拒绝 | 正向 | 用户规则（`write_protection.files`）命中应生成的一份 `.i.rs`：`conflicts` 恰含该项（`path` 为 repo 相对 POSIX、`reason` 取用户规则文案且 `%t` = `archi_generate`），`written` 与 `unchanged` 皆空数组、端口零调用、工具仍 `Ok`、磁盘零变化 | 新增 |
| 逐份保护判定与整批拒绝 | 边界 | 两份命中 ⇒ `conflicts` 两条且序与 `files` 序一致（不排序、不去重）；唯一一份 `INDEX.toml` 命中 ⇒ 同款整批拒绝（零 `.i.*` 写盘）；同 fixture 去规则 ⇒ 零 `conflicts`、端口被调用（射程不误伤） | 新增 |
| 逐份保护判定与整批拒绝 | 异常 | 反向锚：`archi_generate` 无 `path` 入参仍须在射程内——拒绝串含工具名与路径（证明判定不靠入参 `path`）；保护判定零第二份规则集（源码锚：执行体内只出现 `evaluate_target`，零自建 glob / 零自拼 reason） | 新增 |
| 零产出三成因（AC-5 校准口径） | 正向 | 三例各自调用（未写 `.schema.json` / 接口面留白 `$defs {}` + `properties {}` / 有 op 但入口不在盘）：三例 `is_error` 均为假，`written` 与 `unchanged` 中零 `.i.*` 条目（`INDEX.toml` 按端口回传落位），磁盘除 `INDEX.toml` 外零变化 | 新增 |
| 零产出三成因（AC-5 校准口径） | 边界 | 三类成因同株并存（三元素各占一因）⇒ 一份 `.i.*` 都不产生、零 `Err`、`conflicts` 空；另一边界：零元素模型根 ⇒ `files` 恰一份 `INDEX.toml` | 新增 |
| 零产出三成因（AC-5 校准口径） | 异常 | 坏模型（TOML 语法错）⇒ `is_error` 且诊断带 `file:line`（与「零产出」严格可分）、端口零调用、磁盘零变化 | 新增 |
| 响应形状（三字段 / 相对路径 / 恒在场） | 正向 | 三键恒在场（`conflicts` 非空时 `written` / `unchanged` 仍在且为 `[]`）；`written` / `unchanged` 串均 repo 相对 POSIX（无 root 前缀、无反斜杠、无 `./` 前缀） | 新增 |
| 响应形状（三字段 / 相对路径 / 恒在场） | 边界 | 空结果序列化为 `[]` 而非 `null`；`conflicts` 每项恒含 `path` + `reason` 两键（零第三键、零嵌套 DTO） | 新增 |
| 响应形状（三字段 / 相对路径 / 恒在场） | 异常 | 端口返回越根错（`Err`）⇒ 工具 `Err` 原文透传（执行体不吞错、不改写为 `Ok` 冲突）；输出仍过 `tools::cap_result` 单点（零第二份截断实现） | 新增 |
| `archi_api(check)` 的 `remedy` 三态（AC-6） | 正向 | 三列全空（空差异根）⇒ 响应含顶层 `remedy` 键且值为 `null`；`artifacts` / `findings` 两轴键与值逐字不受影响 | 新增 |
| `archi_api(check)` 的 `remedy` 三态（AC-6） | 边界 | 有差异 ∧ `generate == true`（`with_generate(true)`）⇒ `remedy` 为短串，含 `archi_generate` 与「MUST NOT 手改生成物」；同一根换 `with_generate(false)` ⇒ 短串指向架构页会话且含「MUST NOT 手改生成物」、**零 `archi_generate` 字样**（不许诺未下发工具）；两例 `artifacts` 三列内容相同 | 新增 |
| `archi_api(check)` 的 `remedy` 三态（AC-6） | 异常 | 三态之外的输入面：`artifacts.enabled = false`（接口面未启用）时 `remedy` 仍按「三列是否全空」判定（不新造第四态）；响应体长度不因 `remedy` 触发截断（短串，`MAX_RESULT_BYTES` 断言不变） | 新增 |
| 写作范围封闭与零第二份实现 | 正向 | 源码锚：`archi_tool.rs` 内零 `fs::write` / 零 `create_dir_all` / 零 `std::process`；生成臂只串接 `archi::workspace_render` → `ProtectRules::evaluate_target` → `writer.write` 三个既有单点（零 TOML 拼接、零模型写回、零契约写入） | 新增 |
| 写作范围封闭与零第二份实现 | 边界 | 模块头与 `execute` doc 的臂清单计数由「七支 / 五只读 + 两写」改「八支 / 五只读 + 三写（两模型写 + 一生成物写）」，且与 `execute` match 的实际臂数一致（八臂 + `other` 兜底） | 新增 |
| 写作范围封闭与零第二份实现 | 异常 | 未知名仍 `Err`（第八臂不吞 `archi_generate_x` / 空串）；`archi_generate` 不写 `<叶段>.schema.json` / `<叶段>.md`（写前写后两文件字节与 `mtime` 逐字不变） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `archi::ArtifactWriter`（端口） | test-local 替身 `RecordingWriter`：`Mutex` 记录每次 `(root, files)`、可编程返回 `Ok(written/unchanged)` 或 `Err`、可断言调用次数为零（冲突 / 装配缺失路径）。**作为 `ArchiContext::with_writer` 的显式注入依赖**，只记录与回传构造值，不复制渲染 / 比对 / 写盘 | 执行链、幂等、冲突、响应形状、写作范围各组 |
| `ProtectRules` | 真实 `ProtectRules::load(Some(root))`（真实工作区 `config.json` 用户规则）+ 真实 `with_session` 两条会话规则；零替身 | 逐份保护判定三例、冲突整批拒绝 |
| `archi::workspace_render` / `archi::dto` / `archi::api::{artifact_check, findings}` | 真实调用（渲染、比对、判据即被测链路的输入，替身会引入第二事实源） | 全部用例 |
| 盘面 | 真实 tempdir（`tempfile` dev-dep 既有）+ 真实模型 TOML / `<叶段>.schema.json` / 入口源码文件；`mtime` 以 `fs::metadata` 真读 | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner_test.rs

<!-- design.md 未声明该文件的公共 API 变更（`SdkRunner::with_artifact_writer`、`PumpPayload.artifact_writer`、`mint_archi_context` 八名判据均为 `pub(crate)` 内部缝）。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更；下列用例的被测对象是铸造判据与注入链第 3/4 跳的行为面。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `mint_archi_context` 铸造判据（七名 → 八名） | 正向 | 既有表驱动用例补一格：**仅** `archi_generate` 在清单内 ⇒ `Some`；八名逐个单独在场各铸一份（既有七个 case 零改动） | 修改 |
| `mint_archi_context` 铸造判据（七名 → 八名） | 边界 | 空清单 / `tools: None` 语义载体（内建七件）⇒ `None`；清单含重复名 ⇒ 仍 `Some` 一份；workspace root 不存在 ⇒ 不 panic（惰性铸造） | 修改 |
| `mint_archi_context` 铸造判据（七名 → 八名） | 异常 | 近名（`archi_generate_x` / `archi_generate ` 尾空格 / `ARCHI_GENERATE` / `archi_generat`）⇒ `None`（精确名比较，零前缀嗅探）；不得因新名把判据放宽为「含 `archi_` 前缀」 | 修改 |
| `generate` 判据位（`ArchiContext.generate`） | 正向 | 含 `archi_generate` 铸造的上下文 `generate == true`；含 `archi_api` 而清单无 `archi_generate` ⇒ `generate == false`（`implement/executor` 形态） | 新增 |
| `generate` 判据位（`ArchiContext.generate`） | 边界 | 同时含八名 ⇒ `generate == true`；仅 `archi_generate` ⇒ 仍铸上下文（不得因「只含生成物写工具」而不铸，否则 `remedy` 第二态与生成臂同时不可达） | 新增 |
| `generate` 判据位（`ArchiContext.generate`） | 异常 | 反向：`generate` 位与 writer 位**解耦**——`Some(上下文)` 且未注入 writer 时 `generate` 仍为 `true`（执行体自行 `Err`，不得以「无 writer」静默降为 false） | 新增 |
| `with_artifact_writer` builder 缝与载荷透传 | 正向 | `SdkRunner::new(…).with_artifact_writer(Some(writer))` 构造后 `open()` 成功；泵内铸出的 `ArchiContext` 携带该 writer（以替身计数「泵起播后 `archi_generate` 交端口的 files 数 > 0」确认接线，形态沿 `with_ask_port(Some(fake_port))` 先例） | 新增 |
| `with_artifact_writer` builder 缝与载荷透传 | 边界 | `None` 形态可构造且 `open()` 成功（不 panic、不静默成功到执行体之外）；与 `with_ask_port` / `with_max_tokens` 双向续链后两载荷互不覆盖 | 新增 |
| `with_artifact_writer` builder 缝与载荷透传 | 异常 | 缺省（未调 builder）⇒ 上下文 writer 缺席，执行体 `Err`（本层只断言 open 不 panic、不把 `None` 变成「保护关闭 / 写盘关闭」语义） | 新增 |
| 注入链源码锚（第 3/4 跳） | 正向 | `include_str!("runner.rs")`：`PumpPayload` 含 `artifact_writer` 字段、`open_session` 载荷字面量填 `self.artifact_writer.clone()`、`session_pump` 链 `.with_writer(…)` 各恰一处 | 新增 |
| 注入链源码锚（第 3/4 跳） | 边界 | 铸造调用的 root 取 `open.ctx.workspace_root`（会话 cwd；change 会话 = worktree），**不取** `PumpPayload.workspace_root`（主仓根，保护规则装配根）——生成落点与会话 cwd 同源，主仓零变化的判据由此成立 | 新增 |
| 注入链源码锚（第 3/4 跳） | 边界 | 字段与其载荷类型为 `Option<Arc<dyn archi::ArtifactWriter>>`（零具体实现类型出现在 runner；`ArchiArtifactWriter` 在本文件零命中） | 新增 |
| 注入链源码锚（第 3/4 跳） | 异常 | 反向：该字段路径零 `unwrap()` / 零 `expect(` / 零 `#[cfg(test)]` 分支（absent 走 `Option` 语义，不 panic） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `archi::ArtifactWriter`（端口） | test-local 替身（计数 + 回传固定 `ArchiApiWriteOutcome`），经 `with_artifact_writer(Some(...))` 显式注入 | builder 缝与载荷透传三例、`generate` 位端到端一例 |
| provider / 引擎 | 既有装置（`anthropic_provider` + `build_model` + `rig` test-utils 假 provider）真实走 `open()`；零自造替身 | 全部 `open()` 相关用例 |
| 无（真实组合） | `mint_archi_context` 为纯函数（只读名字清单，零 IO / 零盘面） | 铸造判据与 `generate` 位前三例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/lib.rs -> packages/desktop/src-tauri/crates/infra/agent/src/lib_test.rs

#### 待测功能

- EngineFacade.with_artifact_writer(): builder 缝 `pub fn with_artifact_writer(self, writer: Option<Arc<dyn archi::ArtifactWriter>>) -> Self`；与 `with_ask_port` 同型（注入链第 2 跳）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `EngineFacade::with_artifact_writer` 链式构造 | 正向 | `.with_artifact_writer(Some(替身))` 后 `runner_for(EngineKind::Sdk, cfg)` 仍产出 SDK 引擎，行为与「直构 `SdkRunner` 加同载荷」一致（沿 `with_ask_port` / `with_max_tokens` 的等价性先例） | 新增 |
| `EngineFacade::with_artifact_writer` 链式构造 | 边界 | `Some` / `None` 两形态均可构造；与 `with_context_window` / `with_ask_port` / `with_workspace_root` / `with_efforts` 双向续链（两种顺序）均可达且两载荷互不覆盖 | 新增 |
| `EngineFacade::with_artifact_writer` 链式构造 | 异常 | 重复调用该 builder ⇒ 后者覆盖（载荷原样透传、零 panic、零合并语义）；空配置 + 该载荷仍 `config_missing` 显式失败（新字段不得把启动校验变成通过） | 新增 |
| 既有构造路径零回归 | 正向 | `EngineFacade::new()` 与 `with_resume_transcript(…)` 的 `artifact_writer` 缺省 `None`，行为与落地前一致；既有 builder 链（10 处穷尽 `Self { … }` 字面量）全部编译通过且字段透传（`self.artifact_writer` 原样带出） | 回归 |
| 既有构造路径零回归 | 边界 | 缺省 `None` 的语义是「未注入」而非「写盘关闭」：`runner_for` 的 sdk 臂不因缺省而短路、不产生第二份 writer 构造（源码锚：`with_artifact_writer` 在 `runner_for` 的 sdk 臂恰一处） | 回归 |
| 既有构造路径零回归 | 异常 | `Cli` 臂仍以逐字退役文案显式拒绝且零 `Ok` 变体混入（新字段不得改变 `runner_for` 的两值穷举与拒收正文）；装载缝调用计数恒零 | 回归 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `archi::ArtifactWriter`（端口） | test-local 替身（零字段 / 计数）作为 `Option<Arc<dyn …>>` 显式入参；断言只看「是否可构造 / 是否透传」，不触盘 | 链式构造三例、既有构造路径正向 |
| `store::Store` | 真实 `tempfile` 真件库（既有 `temp_store` 装置） | 需要 `runner_for` 的用例 |
| 无（真实组合） | 门面为构造面纯数据装配，无进程边界 | 源码锚与缺省 `None` 断言 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/compose.rs -> packages/desktop/src-tauri/crates/infra/agent/src/compose_test.rs

#### 待测功能

- compose_turn(): 末位增参 `artifact_writer: Option<Arc<dyn archi::ArtifactWriter>>` 并链到门面（组合根注入缝，注入链第 1 跳）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `compose_turn` 末位新增参数 | 正向 | 末位传 `Some(替身)` 组装成功，`ComposedTurn` 四件（`kernel` / `runner` / `query` / `model` 双档）与既有九参语义不变；`begin` 后本轮工具面与写权位不受新参数影响 | 修改 |
| `compose_turn` 末位新增参数 | 边界 | 末位传 `None` 亦组装成功（既有测试调用点形态）；既有测试调用点机械补 `None` 后全部既有断言零变化（约 58 处，回归面由既有用例集合全绿承载） | 修改 |
| `compose_turn` 末位新增参数 | 异常 | 源码锚：签名末位类型逐字为 `Option<Arc<dyn archi::ArtifactWriter>>`；门面链上 `.with_artifact_writer(artifact_writer)` 恰一处；零 `Some(Arc::new(` 具体实现出现在 `compose.rs`（实现归壳层组合根） | 新增 |
| `archi_matrix` 八行命中（AC-2） | 正向 | 表驱动八行逐行比对名字清单：`(proposal, executor)` 与 `(dev-design, executor)` 各含 `archi_generate`（proposal 六名 + generate、dev-design 七名 + generate）、`(test-design, executor)` 恰 `[archi_search, archi_module]`、`(implement, executor)` 恰 `[archi_module, archi_api, archi_drift]`（零 generate）、`(dev-design, evaluator)` 与 `(code-review \| acceptance, evaluator)` 逐名不变；archi 来源臂为七内建 + **八** archi 名（`preamble` / `before_done` / `ask` 三位逐字不变） | 修改 |
| `archi_matrix` 八行命中（AC-2） | 边界 | 矩阵外相位（initialize / test-gen / test-execution / code-analyze / archive 的 SpecSync）⇒ `None`（零 archi 工具，但 DB 两名仍在 change 臂）；畸形 / 短段 / 末段非数字 `source_ref` ⇒ `None`（fail-closed）；explore 臂与 debug 等非 change 来源 ⇒ 零 archi 名 | 修改 |
| `archi_matrix` 八行命中（AC-2） | 异常 | 反向：generate 不得落只读格（`dev-design/evaluator` / `code-review/evaluator` / `acceptance/evaluator` 零命中）；test-design 格不得追加 `archi_api` / `archi_drift` / `archi_scaffold` 或任何写工具 | 修改 |
| 矩阵与写权锁同集（收窄后） | 正向 | 全组合穷举：矩阵含**两模型写工具**（任一）⟺ `model_write_lock == None`（双向断言）；比对集合恰 `{archi_element, archi_relation}` | 修改 |
| 矩阵与写权锁同集（收窄后） | 边界 | `archi_generate` 不在对拍集：显式断言比对集合与该名无交集，且两起草相位「有 generate」与「不挂锁」互不作为对方的判据（不因 generate 凑同集） | 修改 |
| 矩阵与写权锁同集（收窄后） | 异常 | 反向两方向各设断言：① 无「矩阵给模型写工具但挂锁」；② 无「不挂锁但零模型写工具」。MUST NOT 通过把 generate 下发给 implement 或从起草相位撤掉来凑对拍 | 修改 |
| `model_write_lock` 恰两条会话规则 | 正向 | 挂锁格返回 `Some(vec![…])` 且长度恰 **2**：`glob` 分别由 `archi_graph_model_rel_dir()` / `archi_models_rel_dir()` 派生（`**/<dir>/**`），`reason` 分别取 `session_lock_template()` / `contract_lock_template()` | 修改 |
| `model_write_lock` 恰两条会话规则 | 边界 | 不挂锁格仍 `None`（archi 臂 + `(proposal\|dev-design, executor)`）；畸形 / 短段 / 未知 phase / 非 change 且非 archi 来源 ⇒ 两条规则同时在位（含 `explore` / `debug`） | 修改 |
| `model_write_lock` 恰两条会话规则 | 异常 | 反向：`models/**` 与 `graph/model/**` MUST NOT 进内置清单（`rules::load` 仍恰工作区 `config.json` 一条，AC-8）；两条 glob 零新路径字面量（`.dev-team` / `models` 字面在本文件零命中，全经 `foundation::layout`） | 修改 |
| 写面判定行为（相位锁端到端） | 正向 | `<id>/implement/executor/1` 会话以 `write` / `edit` 落 `<archi_dir>/models/<分层…>/<叶段>.schema.json` 与同名 `.md` ⇒ 一律拒，拒串取**契约串**（含路径、`%t`、「实现相位只实现契约、不改契约」、三条出路、不得绕过）；archi 臂与两起草相位 ⇒ `evaluate` 返回 `None` 放行 | 修改 |
| 写面判定行为（相位锁端到端） | 边界 | AC-8 分离面：同轮写 `<archi_dir>/graph/INDEX.toml`、`<archi_dir>/config.json`、`.i.rs` / `.i.ts` ⇒ 全部放行；模型目录同轮仍被拒且取**模型串**（两串不互换、不拼接） | 修改 |
| 写面判定行为（相位锁端到端） | 异常 | 两串彻底不同：契约串 ≠ 模型串（逐字 `assert_ne!`），且 `%s` / `%t` 全量替换后无占位符残留；系统通知载荷与 `is_error` 内容为同一串 | 修改 |
| 源码锚：成因注释与 doc 改写 | 正向 | `model_write_lock` 成因注释含「判据在 `implement` 内不再可自愈 / 唯一出路是 backtrack」的代价句；旧句「契约资产 md 不锁（`implement/executor` 持有 `archi_api`，改代码相位必须能同步契约）」零命中 | 新增 |
| 源码锚：成因注释与 doc 改写 | 边界 | 模块头 doc 的 archi 臂「七 archi 名」→「八」、`protect` 位「`Some(一条)`」→「`Some(两条)`」与实现一致；矩阵 doc 与向量序一致 | 新增 |
| 源码锚：成因注释与 doc 改写 | 异常 | 反向：`model_write_lock` 函数体仍只有一份 `SessionProtectRule` 构造表达式形态（`reason: <template>()` 两处），零第二份文案字面（拒串全取模板出口） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `archi::ArtifactWriter`（端口） | test-local 替身作为 `compose_turn` 末位显式入参（只验可组装与透传，不触盘） | 末位参数三例 |
| `WorkspaceStores` / `store::Store` | 真实 `tempfile` 真件库（既有 compose 装置） | `compose_turn` 端到端组装 |
| 无（真实组合） | `archi_matrix` / `model_write_lock` / `assemble_injections` / `ProtectRules` 全为纯函数或内存装配，真实组合；相位锁端到端用例用真实 `ProtectRules::load` + 真实 `SessionProtectRule` 判定 | 矩阵、写权、锁面、源码锚各例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/reason.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/reason_test.rs

<!-- design.md 未声明该文件的公共 API 变更（`CONTRACT_LOCK_TEMPLATE` 与 `contract_lock_template()` 为 `pub(crate)` 内部缝）。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更；被测对象为第二份会话拒串的文案面与出口单点。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `contract_lock_template()` 四要素齐备（AC-7） | 正向 | 以中性路径与工具名展开后四要素逐个在场：① 事实串（「该路径受相位锁保护」+ 路径原文 + `%t` 替换后的工具名）；② 政策（「接口定义（.schema.json）与模块文档（.md）」+「只在 proposal / dev-design 相位的执行侧（以及架构页会话）写入」+「实现相位只实现契约、不改契约」）；③ **三条出路**（「改代码追上它」/「停下来说明冲突…backtrack 回 dev-design 修订」/「或请用户改用架构页会话直接维护」）；④「不要改用其他命令或工具尝试绕过」 | 新增 |
| `contract_lock_template()` 四要素齐备（AC-7） | 边界 | `%s` / `%t` **全量**替换：模板内 `%s` / `%t` 占位计数与展开后的替换次数一致（路径含 `%s` 字面时按替换次数断言）；展开后串内零残留 `%s` / `%t`；空路径 / 空工具名两边界仍产出完整四要素串（零 panic） | 新增 |
| `contract_lock_template()` 四要素齐备（AC-7） | 异常 | 与模型串逐字不同：`assert_ne!(contract_lock_template(), session_lock_template())`；契约串内零「架构模型 / 元素与关系 / 写进 design.md 或 tasks.md」措辞（复用了就是三重误导），模型串内零「.schema.json / 契约」措辞 | 新增 |
| 既有四串零改动与出口单点 | 正向 | 内置串 / 会话模型串 / 用户带 reason / 用户无 reason 回落四态逐字不变（既有 `deny_reason` 三态用例全绿）；`contract_lock_template()` 返回值与 `CONTRACT_LOCK_TEMPLATE` 常量同源（单一出口，装配层不再抄一份文案） | 回归 |
| 既有四串零改动与出口单点 | 边界 | `ReasonKind::Session(template)` 对两份模板同款处理（模型串与契约串各展开一次，行为同构）；`substitute` 对两份模板的替换语义一致 | 回归 |
| 既有四串零改动与出口单点 | 异常 | 源码锚：模块头「五类文案形态」计数句仍在（`D-14`：契约串是会话类模板的第二个实例，不新增 `ReasonKind` 变体）；`contract_lock_template` 函数体只返常量（零格式化、零参数） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | 文案模板为编译期常量、函数纯字符串运算（零 IO 零依赖）；全部用例以字符串字面量入参直驱，不建目录、不读配置 | 本节全部用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/archi-knowledge.md -> packages/desktop/src-tauri/crates/infra/agent/src/archi_knowledge_test.rs

<!-- design.md 未声明该文件的公共 API 变更（静态知识骨架，非代码模块）。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更；被测对象为单句换指与体量锚。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 生成物指路换句（AC-9） | 正向 | `include_str!("archi-knowledge.md")` 含新串「生成物过期调 `archi_generate` 重生成」与「MUST NOT 手改生成物」；该句与「读五件 / 写两件」行同段（位次不变） | 修改 |
| 生成物指路换句（AC-9） | 边界 | 旧串零残留：needle「生成物过期用既有命令重生成」与「既有命令」（产品源码 / 骨架面）零命中；句内 `archi_generate` 是 `mount::registry()` 在册名（指路不指向不存在工具，既有断言方向不变） | 修改 |
| 生成物指路换句（AC-9） | 异常 | 反向：骨架零「本仓的生成命令」死指代、零路径模板；`.schema.json` 仍被写作「手写真相 / 不是生成物」（换句不得把契约说成生成物） | 修改 |
| 体量冻结重冻（AC-9） | 正向 | 骨架字节数按 LF 归一后 `<= 1831`、全文（骨架 + 派生段）`<= 2031`；两处**精确断言**按实测值重冻（门限**不抬**），实测值与 `design.md 附录 D` 的字节账（≈1823 / ≈2012）一致 | 修改 |
| 体量冻结重冻（AC-9） | 边界 | 派生段仍为追加（`full[..head.len()] == head.trim_end()` 且 `full.len() > head.len()`）；CRLF 检出不得改变冻结值（`lf_bytes` 归一） | 回归 |
| 体量冻结重冻（AC-9） | 异常 | 超限时断言文案如实报「超过门限 1831 B / 2031 B」（不是重冻失败后的静默放宽）；零其它字节改动的可观测面：五段小节标题与三资产分工三句逐字仍在 | 修改 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | `include_str!("archi-knowledge.md")` 取编译期内嵌同一份字节（禁测试内复制副本）；`knowledge()` 走真实 `build` + 真实 `foundation::layout` 派生；`mount::registry()` 是工具名真值 | 本节全部用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/archi_knowledge.rs -> packages/desktop/src-tauri/crates/infra/agent/src/archi_knowledge_test.rs

<!-- design.md 未声明该文件的公共 API 变更（模块头 doc 措辞随动，拼装面与降级语义零改动）。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更；被测对象为模块头措辞与既有拼装 / 降级语义的零回归。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 模块头措辞随动 | 正向 | 源码锚 `include_str!("archi_knowledge.rs")`：含「三写工具（两模型写 + 一生成物写）」措辞；「两写工具」旧措辞零命中 | 修改 |
| 模块头措辞随动 | 边界 | 该文件内零数字门限（门限只冻结在测试侧，`D-16`）——`1831` / `2031` 字面在本文件零命中 | 修改 |
| 模块头措辞随动 | 异常 | 反向：知识「指路不指向不存在工具」的机制句不被改写为承诺（仍表述为「工具面单源在 `mount::registry()`」一类可核对形态），零新增能力宣称 | 修改 |
| 拼装与降级语义零回归 | 正向 | `knowledge()` 返回 `Some(骨架 + 派生段)`，派生段含 `foundation::layout` 两处派生目录；`include_str!` 编译期内嵌（零运行期文件 IO） | 回归 |
| 拼装与降级语义零回归 | 边界 | 降级语义不变：拼装失败只丢 `preamble` 位（不炸 run），`knowledge()` 在异常路径返 `None` 而非 panic（既有断言沿用） | 回归 |
| 拼装与降级语义零回归 | 异常 | 反向：`knowledge()` 不读环境变量 / 不读配置 / 零 `std::fs` 调用（源码锚，尽力增强项不得变成 IO 面） | 回归 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | 真实 `build` + 真实 `include_str!` 内嵌 + 真实 `foundation::layout` 派生；源码锚读同一份字节 | 本节全部用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools_test.rs

<!-- design.md 未声明该文件的公共 API 变更（`D-10` 偏离项：再导出名单 + `input_path` 显式 `None` 臂 + doc 计数）。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更；被测对象为 `tools::input_path` 对新工具的恒 `None` 与再导出单点。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `input_path` 对 `archi_generate` 恒 `None` | 正向 | `input_path(ARCHI_GENERATE_TOOL, &json!({}))` ⇒ `None`；对照 `input_path("read", &json!({"path":"a.md"}))` ⇒ `Some("a.md")`（基准臂不被改写） | 修改 |
| `input_path` 对 `archi_generate` 恒 `None` | 边界 | 幻觉入参四形态均 `None`：`{"path": "…"}` / `{"path": 123}` / `{"file": "…"}` / `{"targeT": …}`+空串——显式 `None` 臂先于通用 `path` 提取（否则入参会走沙箱改写 / 被拒） | 修改 |
| `input_path` 对 `archi_generate` 恒 `None` | 异常 | 近名不享豁免：`archi_generate_x` / `archi_generate `（尾空格）/ `ARCHI_GENERATE` 仍走通用臂（含 `path` 时返回 `Some`，即精确名比较、零前缀嗅探） | 修改 |
| 再导出单点与 doc 计数 | 正向 | `crate::sdk::tools::ARCHI_GENERATE_TOOL` 与 `crate::sdk::mount::ARCHI_GENERATE_TOOL` 同值同源（常量单点，零第二份字面） | 新增 |
| 再导出单点与 doc 计数 | 边界 | 五名参数化用例（三 archi 只读 + 两 DB）扩为六名（+ `archi_generate`）后全 `None`，对照臂两条仍 `Some` | 修改 |
| 再导出单点与 doc 计数 | 异常 | 源码锚：`tools.rs` doc 由「七个架构工具（五只读 + 两写）」改「八个 archi 工具（五只读 + 三写：两模型写 + 一生成物写）」，「七个架构工具」零命中；再导出名单含该名且与 `mount` 名单逐字一致 | 修改 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | `input_path` 为纯函数（入参 `&Value` → `Option<&str>`）；`execute` 用例用真实 tempdir 盘面；源码锚读 `include_str!("tools.rs")` | 本节全部用例 |

---

### packages/desktop/src-tauri/src/commands/archi/mod.rs -> packages/desktop/src-tauri/src/commands/archi/mod_test.rs

#### 待测功能

- ArchiArtifactWriter.write(): `archi::ArtifactWriter` 的唯一实现 `write(&self, root: &Path, files: &[archi::GeneratedFile]) -> Result<archi::ArchiApiWriteOutcome, String>`；函数体只调 `archi_runtime::api_artifacts::write_artifacts` 一次

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `ArchiArtifactWriter::write` 薄委托 | 正向 | 同一模型盘面的两份 tempdir 副本上，`ArchiArtifactWriter::write(root, &archi::workspace_render(root)?)` 与既有 `archi_api_generate_inner(root)` 的 `written` / `unchanged` 两列**逐字相同**（证明委托同一写盘单点，零自有语义分叉） | 新增 |
| `ArchiArtifactWriter::write` 薄委托 | 边界 | `files = &[]` ⇒ 两列空数组、盘上零变化、不建目录；单份已新鲜（先经 IPC 生成一次再调端口）⇒ `written` 空、`unchanged` 恰含该份 | 新增 |
| `ArchiArtifactWriter::write` 薄委托 | 异常 | 越根目标（`files` 里塞一份 root 之外的 `GeneratedFile`）⇒ `Err` 且文案含「越出观察根」、零写盘；父路径被同名文件占位 ⇒ `Err` 且文案含「不回滚」（非原子语义如实透传，不包装成 Ok） | 新增 |
| 写盘语义穿透（AC-3 / AC-15） | 正向 | 首次调用：`written` 与盘上实际变化一一对应（逐份 `read_to_string` 等于 `files.content`）；第二次调用：`written` 为空、全部目标进 `unchanged`，两次之间未变生成物的 `mtime` 逐字不变（幂等由字节比对承担） | 新增 |
| 写盘语义穿透（AC-3 / AC-15） | 边界 | 孤儿生成物（盘上预置一份无人引用的 `<入口主干>.i.ts` 与一份 `.i.rs`）：调用前后文件集相同（工具零删除，`orphaned` 只由检查面报告）；另「已新鲜 + 需更新」混合批：一份进 `unchanged`、一份进 `written`，两列互斥 | 新增 |
| 写盘语义穿透（AC-3 / AC-15） | 异常 | 单份写失败（父目录被文件占位）⇒ `Err` 且此前已写部分仍留在盘上（不回滚，文案「此前已写入 N 份，不回滚」逐字）；`Err` 路径不返回 `ArchiApiWriteOutcome` | 新增 |
| 唯一实现与零自有写盘（AC-4 源码锚） | 正向 | `include_str!("mod.rs")`：`impl archi::ArtifactWriter for ArchiArtifactWriter` 恰一处；该实现函数体内 `api_artifacts::write_artifacts(` 恰一次、零其它语句分支 | 新增 |
| 唯一实现与零自有写盘（AC-4 源码锚） | 边界 | 该实现体内零 `fs::write` / `fs::create_dir_all` / `fs::remove_file`（顶层的 `fs` 导入仍服务 `check_root` 读面）；`ArchiArtifactWriter` 为零字段单元结构（`pub struct ArchiArtifactWriter;`） | 新增 |
| 唯一实现与零自有写盘（AC-4 源码锚） | 异常 | 反向：该实现体零 `workspace_render` 调用（渲染归执行体与既有 IPC 命令，一处不重复）、零 `read_to_string` 比对（比对归写盘单点）；`commands/archi` 全文件零第二份 `impl … ArtifactWriter` | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | 真实 tempdir 根 + 真实 `archi::workspace_render` + 真实 `archi_runtime::api_artifacts::write_artifacts`（写盘单点即被测链路，替身会掩盖「唯一实现」语义）；`mtime` 以 `fs::metadata` 真读；两命令面对拍用两份独立 tempdir 副本 | 薄委托与写盘语义六例 |
| 无（真实组合） | 源码锚读 `include_str!("mod.rs")` 同一份编译期字节 | 源码锚三例 |

---

### packages/desktop/src-tauri/src/commands/exec/mod.rs -> packages/desktop/src-tauri/src/commands/exec/mod_test.rs

<!-- design.md 未声明该文件的公共 API 变更（`D-10`/`C6`：既有 `compose_turn` 调用点补构造并传参，零新公共函数）。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更；被测对象为组合根注入点（注入链第 0 跳）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 组合根注入点（`compose_turn` 末位传实现） | 正向 | 源码锚 `include_str!("mod.rs")`：`compose_turn(` 调用末位实参为 `Some(Arc::new(ArchiArtifactWriter) as Arc<dyn archi::ArtifactWriter>)`（通用会话 = `/architecture` 页内会话的唯一注入点） | 新增 |
| 组合根注入点（`compose_turn` 末位传实现） | 边界 | `compose_turn(` 在本文件恰一处调用；`use crate::commands::archi::ArchiArtifactWriter;` import 恰一处（单点，零 `archi_runtime` 直引） | 新增 |
| 组合根注入点（`compose_turn` 末位传实现） | 异常 | 反向：该位零 `None` 实参（生产必经注入，`None` 只允许出现在测试调用点）；本文件零 `impl archi::ArtifactWriter`（第二份实现即违约） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | 源码锚读 `include_str!("mod.rs")`；端到端「页内会话生成落主仓」由既有 `mock_app()` + 真实 `WorkspaceStores` 装置与 `commands/archi` 的写盘用例共同承载，不在本文件造替身 | 本节三例 |

---

### packages/desktop/src-tauri/src/commands/change_flow/mod.rs -> packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs

<!-- design.md 未声明该文件的公共 API 变更（`C6`：既有 `compose_turn` 调用点补构造并传参，零新公共函数）。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更；被测对象为 change 流程相位会话的注入点（注入链第 0 跳，同型一处）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| change 会话注入点（`compose_turn` 末位传实现） | 正向 | 源码锚 `include_str!("mod.rs")`：`compose_turn(` 调用末位实参为 `Some(Arc::new(ArchiArtifactWriter) as Arc<dyn archi::ArtifactWriter>)`；`ask_port` 位仍为 `Some(Arc::clone(&ask_port))`（两位互不挤占） | 新增 |
| change 会话注入点（`compose_turn` 末位传实现） | 边界 | `compose_turn(` 在本文件恰一处；写入保护装配根仍为**主仓 root**（`Path::new(&root)`），与会话执行根（worktree）分离——端口写盘面（会话 cwd）与保护判定的根语义各自保持既有口径 | 新增 |
| change 会话注入点（`compose_turn` 末位传实现） | 异常 | 反向：该位零 `None` 实参；本文件零第二份 `impl archi::ArtifactWriter`，零 `archi_runtime::` 直引（写盘单点只经端口） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | 源码锚读 `include_str!("mod.rs")`；既有 `组合根零判定_源码只装配不推导` 一类的装配锚形态直接沿用 | 本节三例 |

---

### packages/desktop/src-tauri/crates/core/workflow/src/prompts/implementation-generator.md -> packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table_test.rs

<!-- design.md 未声明该文件的公共 API 变更（prompt 静态资产，非代码模块）。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更；被测对象为契约纪律句的反转与旧正句零残留。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 契约纪律三要素（AC-10） | 正向 | 该 md 含三要素句：① **不改契约**——`.schema.json` 与 `.md` 在本相位受相位锁保护、改了会被拒；② **生成物新鲜度自查**——改完代码用 `archi_api` 的 `check` 看两轴，本相位无 `archi_generate`、据 `remedy` 转告用户；③ **两条出路**——契约对就改代码追上它；契约不可实现或写错就停下说明冲突、由用户决定 `backtrack` 回 `dev-design`（或改用架构页会话） | 修改 |
| 契约纪律三要素（AC-10） | 边界 | 自查句仍点名 `archi_api` 的 `check`（不自造第二动作名）；三条出路以列表 / 分句形态共现于同一段（不散落到互不相关的段落）；句长门限按新句实测重冻（旧「≤ 100 字符」冻结值随句改写更新，不得直接删断言） | 修改 |
| 契约纪律三要素（AC-10） | 异常 | 反向：旧正句零命中——「同批同步接口定义」「改了导出面就同批同步」「生成物过期用既有命令重生成」「本仓的生成命令」；零「必须能同步契约」类表述（把改契约写成纪律的源头） | 修改 |
| 工具集合与无越权 | 正向 | 该 md 的 archi 工具名集合仍恰 `archi_module` / `archi_api` / `archi_drift` 三名；零 `archi_generate`（本相位不给生成工具） | 回归 |
| 工具集合与无越权 | 边界 | 零两模型写工具名（`archi_element` / `archi_relation`）、零「架构模型可写」声明；模型锁定表述仍为中文措辞（句内零 ASCII `archi` 字样，既有断言沿用） | 回归 |
| 工具集合与无越权 | 异常 | 反向：零 `__MCP:*` / 零 `__DEV_TEAM_ROOT__` 类插件时代片段；零字段形态长表与零架构域路径模板（不得借这次改句把知识段抄进 prompt） | 回归 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | `prompt_text("implementation-generator")` 真实读 `CARGO_MANIFEST_DIR/src/prompts/` 下的同一份 md（缺失即红）；零替身 | 本节全部用例 |

---

### packages/desktop/src-tauri/crates/core/workflow/src/prompts/dev-design-planner.md -> packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table_test.rs

<!-- design.md 未声明该文件的公共 API 变更（prompt 静态资产）。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更；被测对象为 `archi_generate` 工具行与错纪律括注的删除。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `archi_generate` 工具行（AC-10） | 正向 | 该 md 的工具行含 `archi_generate`，并表述「生成物过期调它重生成（幂等、只增只改）」与「入口不在盘 / 接口面留白的新模块零产出正常」两句；同句含「MUST NOT 手改生成物」 | 修改 |
| `archi_generate` 工具行（AC-10） | 边界 | 工具行读五写两 + generate 一名（`archi_module` / `archi_search` / `archi_api` / `archi_drift` / `archi_scaffold` + `archi_element` / `archi_relation` + `archi_generate`）；零新增第四个只读 archi 名；零字段形态长表 | 修改 |
| `archi_generate` 工具行（AC-10） | 异常 | 反向：零「本仓的生成命令」/「既有命令重生成」死指代；零路径模板（不以架构域字面路径教模型定位生成物） | 修改 |
| 错纪律括注删除 | 正向 | 「（实现相位同一条纪律）」整串零命中；接口面纪律句仅保留本相位口径（本相位是契约作者，改完导出面同批改接口定义） | 修改 |
| 错纪律括注删除 | 边界 | 写面与自查句仍齐备（`archi_element` / `archi_relation` 为写面、「工具表达不了的才手改」、`archi_drift` 与 `archi_module` 自查）；「只为已确定元素落笔」句属 `proposal-planner`，本份不要求 | 回归 |
| 错纪律括注删除 | 异常 | 反向：零「无专用写工具」一类已失效绝对表述、零「重排文件」类把整份文件排序推给 agent 的表述（既有禁令沿用） | 回归 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | `prompt_text("dev-design-planner")` 真实读盘同一份 md；零替身 | 本节全部用例 |

---

### packages/desktop/src-tauri/crates/core/workflow/src/prompts/proposal-planner.md -> packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table_test.rs

<!-- design.md 未声明该文件的公共 API 变更（prompt 静态资产）。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更；被测对象为 `archi_generate` 行与 md 责任句（`contractPath` 取路径）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `archi_generate` 行与零产出正常（AC-10） | 正向 | 该 md 的工具行含 `archi_generate` 且表述「零产出正常」与「MUST NOT 手改生成物」；同句零「既有命令」死指代 | 修改 |
| `archi_generate` 行与零产出正常（AC-10） | 边界 | 工具行读四写两 + generate（`archi_module` / `archi_search` / `archi_api` / `archi_scaffold` + 两写 + `archi_generate`）；本相位无 `archi_drift`，故零 `archi_drift` 名（矩阵一致性） | 修改 |
| `archi_generate` 行与零产出正常（AC-10） | 异常 | 反向：零「本仓的生成命令」类死指代、零字段形态长表（与 `dev-design-planner` 同口径） | 修改 |
| md 责任句（`contractPath`） | 正向 | 该 md 含「本 change 涉及元素的 `<叶段>.md`（理由）由本相位写」与「路径经 `archi_module` 的 `contractPath` 取（勿手拼）」两句；同句含「文档 MUST NOT 列接口（导出名归 `.schema.json`）」 | 新增 |
| md 责任句（`contractPath`） | 边界 | 该句零手拼路径模板（`models/` 等架构域字面零命中）；`contractPath` 拼写与 `desktop-archi-agent` 的 `api.contractPath` 键名逐字一致（拼错即指向不存在的字段） | 新增 |
| md 责任句（`contractPath`） | 异常 | 反向：不得把 md 责任写成实现相位的动作（该句只属于本相位，零「implement 同步文档」类表述）；「（实现相位同一条纪律）」零命中 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | `prompt_text("proposal-planner")` 真实读盘同一份 md；零替身 | 本节全部用例 |

---

### packages/desktop/src-tauri/crates/core/workflow/src/prompts/test-design-planner.md -> packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table_test.rs

<!-- design.md 未声明该文件的公共 API 变更（prompt 静态资产）。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更；被测对象为 `archi_module` 用法句与接口定义优先于 grep 的边界句。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| `archi_module` 用法句（AC-10） | 正向 | 该 md 含 `archi_module` 的用法一句：取 `assets.interface` 的 `ops` / `types` 键名，用于核对 `design.md` 的「公共函数 / API」是否与接口定义一致；点名 `archi_module` 为只读工具 | 新增 |
| `archi_module` 用法句（AC-10） | 边界 | 该 md 的 archi 工具名集合恰 `archi_search` + `archi_module` 两名（矩阵该格逐名一致）；零 `archi_api` / `archi_drift` / 写工具 / `archi_generate` 字样 | 新增 |
| `archi_module` 用法句（AC-10） | 异常 | 反向：零路径模板（不以架构域字面路径教模型读 schema）；零字段形态长表（键名归 `assets.interface` 返回值，不进 prompt） | 新增 |
| grep 禁令与接口定义优先（AC-10） | 正向 | 「接口定义是手写真相，MUST NOT 用 grep 源码导出的方式替代它」边界句在场，且与 `archi_module` 用法句相邻成句 | 修改 |
| grep 禁令与接口定义优先（AC-10） | 边界 | 过程 4b 原文禁令「不得 grep 源文件导出声明或虚构条目」**逐字仍在**（两条禁令并存不冲突：前者读 schema、后者扫源码）——逐字比对 `不得 grep 源文件导出声明或虚构条目` | 修改 |
| grep 禁令与接口定义优先（AC-10） | 异常 | 反向：零「接口还没写也可以从源码导出反推」类表述（把侧门写正）；模型锁定表述仍在（既有断言沿用） | 修改 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真实组合） | `prompt_text("test-design-planner")` 真实读盘同一份 md；零替身 | 本节全部用例 |

---

### packages/desktop/src/views/architecture/components/dashboard/contract-panel.tsx -> packages/desktop/src/views/architecture/components/dashboard/contract-panel.test.tsx

<!-- design.md 未声明该文件的公共 API 变更（纯文案一行；`ContractPanelProps` 与 `data-testid` 面零改动）。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更；被测对象为 full 模式提示文案与「零生成入口」的既有只读断言。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| full 模式提示文案（AC-9） | 正向 | `mode = 'full'` 渲染后 `archi-contract-readonly` 的 `textContent` **逐字**等于 / 含「生成需开 change，或让左侧会话生成」（新串，与 `desktop-archi-contract-panel` / `desktop-archi-canvas` delta 引述一致） | 修改 |
| full 模式提示文案（AC-9） | 边界 | 旧串片段零命中：`本仓的生成命令` 与 `执行本仓` 在整页文本零命中；提示元素仍为同一 `data-testid`、同一 `text-[11px] text-muted-foreground` 位次（类名 / 位次零改动） | 修改 |
| full 模式提示文案（AC-9） | 异常 | 反向：文案不得承诺本相位没有的动作（零「执行生成命令」/零命令名字面）；`incremental` 分支零只读提示（该串只在 full 出现） | 修改 |
| full 模式零生成入口（AC-16） | 正向 | `mode = 'full'`：`queryByTestId('archi-contract-generate')` 为 `null`、`archi-contract-check` 仍在；`archi_api_generate` 调用次数恒 0（既有断言逐字不变） | 回归 |
| full 模式零生成入口（AC-16） | 边界 | `mode = 'incremental'`：生成按钮可用、零只读提示、二次确认与「复检一次」序不变（既有用例全绿，证明本变更只动文案） | 回归 |
| full 模式零生成入口（AC-16） | 异常 | 反向：`contract-panel.tsx` 源码零 `archiApiGenerate(` 调用出现在 full 分支（既有源码锚 / 行为锚沿用）；画布仍零生成入口（本组件不含画布按钮） | 回归 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `@tauri-apps/api/core` 的 `invoke` | `vi.hoisted` + `vi.mock('@tauri-apps/api/core', …)`，按命令名分发 `archi_api_check` / `archi_api_generate` / `read_file`，并记录调用序与次数（进程边界，唯一 mock） | 全部用例 |
| 无（真实组合） | `useArchiApiCheck` / `FileViewer` / `StandardDialog` / `sonner` 的 `<Toaster />` 真实渲染（确认文案与 toast 内容是被测面）；查询锚点一律 `data-testid` | 全部用例 |

---


## 不可测试项

- **`packages/desktop/package.json` 的 `version` patch +1（AC-13）** — 静态配置核对项：无同名测试文件，也没有进程内可断言的语义（版本纪律归 `AGENT.md`，由实现阶段守线 + 验收阶段对 `HEAD` 比对承载）；`src-tauri/Cargo.toml` / `tauri.conf.json` 的「逐字不变」同为静态核对项。
- **`bindings.ts` / `bindings.zod.ts` 相对落地前零 diff（AC-6）** — 生成物一致性守卫的职责（`bindings:export` 后 `git diff` 为空），非单测面；前端 DTO 键集断言（`commands/archi` 既有 `mod_test` 键集用例）在本变更零改动，属「零改动面」证据。
- **`pnpm api:check` 零差异与 `.dev-team/architecture/**` 零字节变化（AC-12）** — 管线 / 磁盘快照守线：`INDEX.toml`、`.i.*`、`.schema.json` 三类生成物与模型 / 契约资产均不在本变更的写面（`archi_generate` 只被调用、本仓无模型改动），差异由管线命令拦截；`apiFindings` 计数与构成同理（落地时实测为准）。
- **`AC-4` 的依赖边审计（`agent-runtime` 的 `Cargo.toml` 零 `archi-runtime`；`src-tauri → archi-runtime` 既有边不变）** — Cargo 清单审查面：仓库无「以测试断言 Cargo.toml 依赖」的先例，且依赖边由编译期（能否 `use` 到符号）间接自证；`core/archi` 零 `fs::write` / 零 `create_dir_all` / 零 `std::process` 的行为面已由 `api/mod.rs` 章节的源码锚覆盖。
- **「产品源码对『本仓的生成命令』『既有命令重生成』两种字面零命中」（AC-9 的全仓 grep 面）** — 全仓静态审查（含 `openspec/specs/**` 基线与本 change 目录的排除规则）；落点文件的行为面已由 `mount.rs` / `archi-knowledge.md` / 四份 prompt 章节的 needle 与反向 needle 覆盖。
- **`git status --porcelain` 改动集 ⊆ 提案清单（AC-13）** — 改动集核对是提交面静态检查，进程内不可断言；本设计已按 design D-9 / D-10 把 `sdk/mod.rs`（零 diff）排除、把 `sdk/tools.rs`（偏离项）纳入被测范围。
- **`specs/**` 九份 delta 的形态约束（AC-14：`RENAMED` 的 `FROM:` / `TO:` 锚、MODIFIED 整段拷贝、Requirement / Scenario 标记层级）** — openSpec 校验与人工评审面（`openspec validate` + 归档合并），非单测面；本变更的 `test-design.md` 不复制该判据。
- **零改动面（`crates/core/archi/src/api/{render.rs,check/**,paths.rs}`、`crates/core/archi/src/{parse,edit,drift}/**`、`crates/infra/archi/src/api_artifacts.rs`、`crates/infra/agent/src/sdk/mod.rs`、`crates/infra/agent/src/sdk/protect/{mod,rules,shell,inline}.rs`、`crates/tools/archi-api-gen/**`、`openspec/specs/**` 基线、`src-tauri/Cargo.toml`、`tauri.conf.json`、`src/types/generated/**`）** — design.md 明列零 diff，故无被测映射对；其中 `api_artifacts.rs` 的写盘语义（只增只改 / 从不删 / 非原子 / 越根复核）由 `commands/archi` 章节的等价性与语义穿透用例**间接**承载（该文件零改动即本变更的判据之一）。
- **前端画布其余产品代码（画布组件 / 面板结构 / 生成按钮 / 路由）** — 零 UI 变化：`full` 模式仍无生成入口、`incremental` 交互逐字不变；行为面已由 `contract-panel.tsx` 章节的「full 零生成入口 / incremental 不变」两组回归用例承载，画布侧无源文件改动可映射。
- **接口资产面的显式留白（R8 / Q-1：`Backend.Core.Archi.schema.json` 不给 `ArtifactWriter` 补 `x-class`）** — 该缺口使 `apiUndeclared` 判据恒 `off`（不产判据、不进 `status`、不影响 `passed`），故既无单测断言目标、也不构成门禁风险；`api:export` / `api:check` 的零 diff 属管线守线（见上条）。
- **AC-5 字面口径的实现侧同步** — 本节已在「验收范围」注记把「三种零产出成因 ⇒ `written` 与 `unchanged` 皆空」校准为「零 `.i.*` 条目 + `INDEX.toml` 按字节比对落位」；该口径需在归档前同步进 proposal / spec 字面（dev-design 或提案面动作），本文件只留痕，不代替该修订。
