# 测试设计: archi-rule-agent-prompt-sync

> **变更**: archi-rule-agent-prompt-sync
> **来源**: `design.md`（新增 2 + 修改 21，其中实现面 19 处 + 生成物 / 版本 3 处）、`proposal.md`（AC-1…AC-12）、五份 delta spec
> **被测面口径**: 只覆盖 design「新增文件」+「修改文件」子表（**删除文件不纳入**）；生成物（`bindings.ts` / `bindings.zod.ts`）、版本行（`package.json`）、模块注册（`sdk/mod.rs`）与前端夹具随动记入 `## 不可测试项`——它们是编译 / `tsc` / 静态链强制面，零新增用例语义。
> **测试框架（实测盘点）**: Rust 侧为语言内建 `#[test]` / `#[tokio::test]` + `assert!` / `assert_eq!` / `assert_ne!`（**无第三方断言库**），测试文件与源文件同级、命名 `<模块>_test.rs`，经父模块 `#[cfg(test)] mod <模块>_test;` 注册；临时盘面用 `tempfile`。前端为 `vite-plus/test`（`describe` / `it` / `expect`，vi.mock 只替 Tauri IPC）。本 change 不引新测试依赖。
> **设计约束对齐**: `#### 待测功能` 条目只取 design 的声明面——「公共函数 / API」表有该文件行时逐行映射；无该行时取 design「修改文件」子表 / 「类型定义」表为该文件声明的标识符（私有面契约由决策表 D# 承载），并在条目下方留 HTML 注释标出处，不 grep 源文件导出声明虚构条目；跨模块组合用例挂靠链路发起方 / 最上层调用方所在章节。

---

## 验收范围

| AC ID | 验收条件 | 被测文件或模块 |
|---|---|---|
| AC-1 | `archi_module` 无参概览逐条含 `hasPath`（有 `path` 真 / 无 `path` 假、字段不省略）；单元素响应含 `resolvedEntry` 三态可辨（显式命中 / 约定默认命中 / `Absent`→`problem`），且判据只由 `api::paths::entry_state` 产出（源码复核无第二份入口解析）。验证：`dto_test` 新增矩阵 + 源码复核 | `packages/desktop/src-tauri/crates/core/archi/src/dto.rs`、`packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs` |
| AC-2 | `archi_scaffold` 返回草稿：过 `TomlParser` 零结构诊断、含字段序与内联 `metadata`、`path` 必填提示在场；`kind` 值集外 `Err` 并点名四值；调用前后盘面字节完全相同。验证：`archi_tool_test` + `mount_test`（`kind` enum 对齐） | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs` |
| AC-3 | `archi_drift` 恒全量、`scope` 三值逐级追加（`summary` ⊂ `entries` ⊂ `files`）、值集外 `Err` 点名合法值、`cap_result` 截断后计数仍在、坏模型 `is_error` 不 panic、零跨调用缓存（连调两次各自计时）；描述含「`gate` 全 `off` 时 `unmatchedFiles` 是常态」。验证：`archi_tool_test` + `archi_drift_view_test` + `drift/mod_test` + `mount_test` 描述断言 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_drift_view.rs`（新）、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs`、`packages/desktop/src-tauri/crates/core/archi/src/drift/mod.rs` |
| AC-4 | 收口前自检只在自然收敛支执行一次：`archi_preclose` notice **先于** `TurnDone`、请求数不变量（零额外请求）、不回灌模型（历史无新增消息）；T1 / T2 皆假时零 notice 且引擎日志有 skip 行；`CheckError` ⇒ `archi_preclose_skipped`（带成因）+ `TurnDone` 照常成功。验证：`preclose_test`（新）+ `loop_test` | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/preclose.rs`（新）、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs` |
| AC-5 | 写权矩阵逐格成立：`source = "archi"` 与会话级 `proposal` / `dev-design` 执行侧写 `<archi_dir>/graph/model/*.toml` 成功；`dev-design` 评估 / `implement` / `test-design` / `code-review` / `acceptance` / `explore` / `debug` 与畸形 `source_ref` 一律被拒（拒绝串含指路与「不要改用其他命令或工具尝试绕过」）；`models/**` / `INDEX.toml` / 看护 `config.json` 在所有相位可写。验证：`compose_test` + `protect/**` 测试 + `sdk/runner_test` + `core/agent/runner_test` | `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/mod.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/rules.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/reason.rs`、`packages/desktop/src-tauri/crates/core/agent/src/runner.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs` |
| AC-6 | `SessionInjections` 恰五位（`preamble` / `tools` / `finish` / `ask` / `protect`）；旧线 JSON 缺 `protect` 读出 `None`；`ProtectRules::load` 签名与既有调用点零改动；会话规则位次在内置之后、用户之前（首个命中即定）。验证：`core/agent/runner_test` + `protect` 测试 + `sdk/runner_test` + 源码复核 | `packages/desktop/src-tauri/crates/core/agent/src/runner.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/mod.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/rules.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs` |
| AC-7 | 工具面矩阵逐格成立：proposal = `module` / `search` / `scaffold`；dev-design 执行 = `module` / `search` / `drift` / `scaffold`；dev-design 评估 = `module` / `search`；test-design = `search`；implement = `module` / `api` / `drift`；code-review / acceptance = `module`；archi 臂 = 七内建 + 五 archi 名；矩阵外零 archi 工具。验证：`compose_test` + `mount_test` + `sdk/runner_test` | `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs` |
| AC-8 | 知识文本按分层收口：零示例围栏、零字段语法教学、零 `entry` 三条；含 `kind` 四值语义 / 计数单源 / 看护配置三块；A 类句子逐句携带工具指路且该工具面确有对应字段（机械表）；派生段两处目录；骨架 ≤ 2252 B、全文 ≤ 2867 B（实测值冻结）。验证：`archi_knowledge_test` 重写锚点 | `packages/desktop/src-tauri/crates/infra/agent/src/archi-knowledge.md`、`packages/desktop/src-tauri/crates/infra/agent/src/archi_knowledge.rs` |
| AC-9 | 相位 prompt：两处写权相位含写面与自查（含「只为已确定元素落笔」），锁定侧其余 12 份各含「架构模型已锁」一句（该句零 ASCII `archi` 字样；未命中矩阵的档案整篇零 archi 工具名）；evaluator md 零 `backtrack` / 零落账指令；14 份零占位零 MCP；`include_str!` 装配面逐字不变。验证：`phase_table_test` | `packages/desktop/src-tauri/crates/core/workflow/src/prompts/*.md`（14 份） |
| AC-10 | 绑定生成物零手改：`bindings.ts` / `bindings.zod.ts` 的 `resolvedEntry`（+ `ArchiResolvedEntry` / `EntryVia`）只来自 `bindings:export`；前端夹具随动后 `tsc` 绿。验证：静态链（`bindings:export` → `client:check`）+ `git diff` 复核；桌面端执行（见 `## 不可测试项`） | `packages/desktop/src/types/generated/bindings.ts`、`packages/desktop/src/types/generated/bindings.zod.ts`、前端夹具（`src/views/architecture/**` 与 `src/lib/ipc-contract.test.ts`） |
| AC-11 | 全管线绿：`pnpm -C packages/desktop run check`（`server:check` → `bindings:export` → `api:check` → `client:check`）与 `run test` 全通过；`api:check` 零 diff（架构域资产零改动）。验证：静态检查链 + 全量测试；桌面端执行 | 全仓 |
| AC-12 | 版本纪律：`package.json` = `0.7.3`，`src-tauri/tauri.conf.json` 跟随、`src-tauri/Cargo.toml` 版本不随动。验证：`git diff` 复核；桌面端执行（见 `## 不可测试项`） | `packages/desktop/package.json` |

---

## 单元测试

### packages/desktop/src-tauri/crates/core/archi/src/dto.rs -> packages/desktop/src-tauri/crates/core/archi/src/dto_test.rs

#### 待测功能

- `module_overview(root): 全元素薄概览逐条含 hasPath（纯投影不补默认值，字段不省略）`
- `module_detail(root, fqn): 单元素详情含 resolvedEntry 三态投影（判据单点 api::paths::entry_state）`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `module_overview` 的 `hasPath` | 正向 | 元素声明 `path = ["./src/auth/"]` → 该条目 `hasPath = true` | 单例 |
| `module_overview` 的 `hasPath` | 边界 | 元素 `path = []`（空数组）→ `hasPath = false` 且键在场（不省略、不落 `null`） | 单例 |
| `module_overview` 的 `hasPath` | 边界 | 混合模型（部分元素有 `path`、容器元素恒空）→ 逐条独立判定，条目序仍为 FQN 字典序 | 表驱动 |
| `module_overview` 的 `hasPath` | 异常 | 坏模型（结构诊断非空）→ `Err` 诊断文案，**不返回部分概览** | 单例 |
| `module_overview` 的 `hasPath` | 边界 | 空模型（`model/` 下零 `.toml`）→ `Ok(vec![])`，零 panic | 单例 |
| `module_overview` 的 `hasPath` | 组合 | 与 `search` 两消费面同源：同一盘面下 `search` 结果元素的 `hasPath` 与 `module_overview` 逐条相等（`to_brief` 单点） | 表驱动 |
| `module_detail` 的 `resolvedEntry` | 正向 | 显式 `entry` 声明且在盘 → `path = Some(入口)`、`via = explicit`、`problem = null` | 单例 |
| `module_detail` 的 `resolvedEntry` | 正向 | 无 `entry` 声明、约定默认候选在盘（crate 根 `src/lib.rs` / 目录型根 `index.ts`）→ `via = convention` | 表驱动 |
| `module_detail` 的 `resolvedEntry` | 异常 | 显式 `entry` 不在盘 → `path`/`via` 皆 `null`、`problem = NotOnDisk` 文案（含入口串） | 表驱动（四 `EntryProblem` 各一行） |
| `module_detail` 的 `resolvedEntry` | 异常 | `entry` 落在该元素 `path` 之外 → `problem` 为 `OutsidePath` 文案 | 表驱动 |
| `module_detail` 的 `resolvedEntry` | 异常 | `entry` 形态非法（绝对路径 / 反斜杠 / 上级穿透）→ `problem` 为 `Malformed` 文案 | 表驱动 |
| `module_detail` 的 `resolvedEntry` | 异常 | 两元素声明同一入口 → `problem` 为 `Duplicated` 文案且含另一元素 FQN | 单例 |
| `module_detail` 的 `resolvedEntry` | 边界 | `Absent`（未声明且约定候选皆不在盘）→ `path`/`via` 皆 `null`、`problem` 为固定「未声明 `entry`，且约定默认候选皆不在盘」 | 单例 |
| `module_detail` 的 `resolvedEntry` | 边界 | 元素不存在 → `Err`（既有「元素不存在」文案，含元素总数），零 panic | 单例 |
| `module_detail` 的 `resolvedEntry` | 边界 | 坏模型 → `Err` 结构诊断文案（不返回半成品详情） | 单例 |
| `module_detail` 的 `resolvedEntry` | 组合 | `problem` 与 `crate::api::entry_problem_reason` 直调输出**逐字相等**（同一四值措辞、零第二份） | 表驱动（四变体 + `Absent`） |
| `ArchiResolvedEntry` / `EntryVia` 线形 | 正向 | serde 序列化：三字段皆在场（`null` 显式出现），`via` 取 `explicit` / `convention` 两端 | 表驱动（两枚举值） |
| `ArchiResolvedEntry` / `EntryVia` 线形 | 边界 | 反序列化穷举：`path`/`via`/`problem` 全 `null` 形态可读回；`via` 非法串 → `Err`（枚举封闭，零第三值） | 表驱动 |
| `ArchiResolvedEntry` / `EntryVia` 线形 | 异常 | 未知字段忽略（`deny_unknown_fields` 缺省）不破坏旧线反序列化 | 单例 |
| 入口解析单点（源码自证） | 边界 | `dto.rs` 内 `entry_state(` 恰一处、零 `is_file()` / `entry_candidates` 第二份解析 | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock） | 真实 `tempfile` 盘面 + 真实 `parse_dir` / `ModelGraph` / `api::paths::entry_state`；投影只裁剪，替身会引入第二事实源 | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/archi/src/api/mod.rs -> packages/desktop/src-tauri/crates/core/archi/src/api/mod_test.rs

#### 待测功能

- `entry_problem_reason(problem): 四个 EntryProblem 变体各一句话成因（可见性放宽到 pub(crate)，措辞零改动）`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `entry_problem_reason` | 正向 | 四变体逐值 → 文案含对应关键片段（入口串 / 另一 FQN / 「形态非法」「不在盘」「之外」） | 表驱动（4 行） |
| `entry_problem_reason` | 边界 | 同值两次调用文案逐字相等（纯函数确定性） | 单例 |
| `entry_problem_reason` | 边界 | 入口串含特殊字符（`\n` / emoji / `：`）→ 原样插入、不转义不 panic | 表驱动（3 行） |
| `entry_problem_reason` | 异常 | 入口串为空串 → 文案仍完整（`` `` `` 包裹形态），零 panic | 单例 |
| `entry_problem_reason` | 组合 | 与 `check::entry_missing_message` 同源：同一 `EntryProblem` 两处文案的关键片段一致（既有措辞面零漂移） | 表驱动（4 行） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock） | `EntryProblem` 值直构（纯函数入参），不落盘不建图中 | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/archi/src/drift/mod.rs -> packages/desktop/src-tauri/crates/core/archi/src/drift/mod_test.rs

#### 待测功能

- `assets_hash(root): 四类架构资产聚合哈希（sha256 前缀、稳定序、缺失目录不参与、读失败按空字节参与）`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `assets_hash` | 正向 | 同一盘面连续两次调用 → 值逐字相等；`sha256:` 前缀 + 64 位十六进制 | 单例 |
| `assets_hash` | 正向 | 四类资产各改一处即变：`graph/model/*.toml` / `models/**/*.md` / `models/**/*.schema.json` / `graph/INDEX.toml` | 表驱动（4 行，各一次写入） |
| `assets_hash` | 边界 | 全空盘（三处资产根皆不存在）→ 不 panic、返回固定稳定值，两次相等；与另一全空盘同值 | 单例 |
| `assets_hash` | 边界 | 资产根存在但零条目、或仅混入非目标扩展名文件（目录 / `.txt`）→ 与全空盘同值（过滤判据） | 表驱动 |
| `assets_hash` | 边界 | 空文件 vs 单字节文件 → 值不同（字节参与累加）；同名不同内容 → 值不同 | 表驱动 |
| `assets_hash` | 边界 | 内容相同、文件名不同 → 值不同（文件名参与累加） | 单例 |
| `assets_hash` | 边界 | 乱序创建（先建 `z` 后建 `a`）与顺序创建同盘面 → 同值（相对路径排序） | 单例 |
| `assets_hash` | 边界 | 索引文件缺失 / 存在各一态 → 值可辨且不 panic（单文件缺失即不参与） | 表驱动 |
| `assets_hash` | 异常 | 目录内混入不可读子项（子目录不被当文件读）→ 跳过、不 panic、哈希稳定 | 单例 |
| `assets_hash` 与 `model_hash` 分工 | 组合 | 只改契约 md → `assets_hash` 变而 `check` 报告 `modelHash` 逐字不变（两者共享 `hash_files` 但覆盖面不同） | 表驱动（4 类资产各一行） |
| `model_hash` 口径回归 | 正向 | 改造后 `check` 报告 `modelHash` 与改造前算法（文件名字节 + 内容 + 空字节分隔）手工复算相等 | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock） | 真实 `tempfile` 三处资产根 + 真实 `foundation::layout` 派生 + 真实 `sha2` | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_drift_view.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_drift_view_test.rs

#### 待测功能

- `drift_summary(report, duration_ms): 缺省 scope 投影（`status`/`passed`/`modelHash`/`model`/`counts`/`errors`/`warnings`/`modelWarnings`/`apiFindings`/`durationMs`），零路径串`
- `drift_entries(report, duration_ms): summary 之上追加 observations 与判据条目全量`
- `drift_files(report, duration_ms): entries 之上追加 files 四类清单全量`
- `DRIFT_SCOPES: 三值词表单源（mount enum 与 Err 文案共用）`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `drift_summary` | 正向 | 健康盘面 → 键集逐项在场（`mode`/`status`/`passed`/`modelHash`/`model`/`counts`/`errors`/`warnings`/`modelWarnings`/`apiFindings`/`durationMs`），`counts` 五个长度位 + `apiFindings` 三档 + `scan` 六项计数齐全 | 单例 |
| `drift_summary` | 正向 | `duration_ms` 逐值透传 → `durationMs` 逐字相等 | 参数化（`0` / `1` / `74` / `u64::MAX`） |
| `drift_summary` | 边界 | 空报告（零 errors/warnings/observations/apiFindings，`scan` 各清单空）→ 各数组为空数组、各计数为 `0`，**键不缺席** | 单例 |
| `drift_summary` | 边界 | `gate` 全 `off` 的常态盘面（`unmatchedFiles` 数百条）→ `counts.scan.unmatched` 计数正确且投影 JSON 文本**不含任何 unmatched 路径串** | 单例 |
| `drift_summary` | 异常 | `errors` 非空 → `status = "errors"` 且 `passed = false`（与 `report.passed()` 同值） | 单例 |
| `drift_summary` | 异常 | 坏模型（`model.structuralErrors` 非空）→ `structuralErrors` 明细 + `structuralErrorCount` 在场，`errors` 数组仍空 | 单例 |
| `drift_entries` | 正向 | `summary` 键集全含 ∧ 追加 `observations` / `apiFindings` 条目明细（`byKind` 仍只在 summary 侧） | 单例 |
| `drift_entries` | 组合 | 与 `drift_summary` 同值盘面下计数位逐字段相等（增补不重算） | 参数化（三份盘面） |
| `drift_files` | 正向 | `entries` 键集全含 ∧ 追加 `files` 的 `unmatched` / `matched` / `excluded` / `unresolved` 四类清单 | 单例 |
| 三 scope 包含关系 | 边界 | `summary ⊂ entries ⊂ files`（键集严格包含，逐级只增不减） | 表驱动（三组对照） |
| `DRIFT_SCOPES` | 正向 | 常量逐值 `["summary", "entries", "files"]`、长度恰 3（值集封闭） | 单例 |
| 纯投影纪律（源码自证） | 边界 | 本模块零判定 / 零 IO：三个投影函数均从 `drift_summary` 产物增补（`drift_files` 内零第二次计数装配） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock） | `Report` 由真实 `drift::check(Full, [], None, never)` 在 `tempfile` 盘面产出（不直构 30+ 字段的替身——直构会掩盖真实计数形态） | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/preclose.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/preclose_test.rs

#### 待测功能

- `baseline(turn): archi 会话取起点 assets_hash；preamble 缺席即 None（零 IO）`
- `touched(name, input, cwd): write / edit / bash 写面命中 <archi>/graph/** ∨ <archi>/models/**`
- `run(sender, turn, touched, baseline): T1 ∪ T2 触发、notice / 引擎日志、fail-open 三面`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `baseline` | 正向 | `preamble = Some(知识文本)` → `Some(hash)` 且与直调 `archi::drift::assets_hash(cwd)` 逐字相等 | 单例 |
| `baseline` | 边界 | `preamble = None` → `None`（零 IO：`cwd` 指向不存在目录亦 `None`、零 panic） | 表驱动（2 行） |
| `baseline` | 边界 | `preamble = Some("")`（空串）→ 仍 `Some`（判据是 `is_some()`，不按内容判会话） | 单例 |
| `baseline` | 异常 | `cwd` 下三处资产根皆不存在 → 返回全空盘稳定值（不抛错） | 单例 |
| `touched`（`write` / `edit`） | 正向 | `write` 的 `path` 为模型声明目录内某 `.toml` 的绝对路径 → `true` | 表驱动（`graph/model/*.toml` / `models/**/*.md` 两行） |
| `touched`（`write` / `edit`） | 边界 | `path` 恰为 `<archi>` 根 / `graph` 目录本身（不带尾随文件名）→ `false`（判据是目录前缀） | 表驱动（3 行） |
| `touched`（`write` / `edit`） | 边界 | `path` 缺失 / 非字符串（`null` / 数字）→ `false`，零 panic | 表驱动（3 行） |
| `touched`（`write` / `edit`） | 边界 | `path` 落在项目其他目录（`src/` / `openspec/`）→ `false` | 表驱动（2 行） |
| `touched`（`bash`） | 正向 | 写面命令（`cat > <model 路径>` / `tee <model 路径>` / `>> <model 路径>`）→ `true` | 表驱动（3 行） |
| `touched`（`bash`） | 边界 | 只读命令（`ls` / `cat x` / `grep -n x y`）→ `false` | 表驱动（3 行） |
| `touched`（`bash`） | 边界 | 空命令串 / 空白串 → `false`（`written_paths` 空数组） | 表驱动（2 行） |
| `touched` | 边界 | 命名工具非三者（`read` / `archi_module` / `finish_step`）→ 恒 `false`，即便入参带 `path` | 表驱动（3 行） |
| `touched` | 异常 | 大小写 / 盘符与 `cwd` 前缀失配的绝对路径 → `false`（假阴，由 T2 兜底；不引入路径折叠） | 表驱动（2 行） |
| `touched` | 异常 | `bash` 写目标的不可解析形态（含 `~` / 空 target）→ 该 op 跳过、其余照判，零 panic | 单例 |
| `run` | 正向 | T1 命中（`touched = true`）且盘面未变 → 发出 `SystemNotice{subtype: "archi_preclose"}`，payload 含 `status` / `passed` / `counts` / `durationMs` / `changedAssets = false` | 单例 |
| `run` | 正向 | T2 命中（`touched = false`、起点 hash 与收口值不等）→ 同样发 notice 且 `changedAssets = true` | 单例 |
| `run` | 边界 | T1、T2 皆假（`touched = false` 且盘面未变）→ 接收端零事件（零 notice，纯问答零转录噪声） | 单例 |
| `run` | 边界 | `baseline = None`（非 archi 会话）→ 早退：零 notice、零事件 | 单例 |
| `run` | 异常 | `drift::check` 返回 `CheckError`（坏模型 / 架构域配置非法）→ 发 `SystemNotice{subtype: "archi_preclose_skipped"}`，`payload.reason` 含成因；函数正常返回（不抛错） | 表驱动（2 行：坏模型 / 配置非法） |
| `run` | 异常 | `spawn_blocking` 任务失败 → 发 `archi_preclose_skipped` 且 `reason` 含「自检任务失败」 | 单例 |
| `run` | 边界 | 零请求增量 / 零历史追加：函数发出的事件种类集合 ⊆ `{SystemNotice}`（不回灌模型、不催轮次） | 单例 |
| `run` | 边界 | 空架构域（无模型）→ notice `status = "skipped"` 照常发出，`passed` 与 `report.passed()` 一致 | 单例 |
| `run` | 边界 | 常态大报告（302 unmatched）→ payload 只在计数字段出现 `302`，JSON 文本零 unmatched 路径串 | 单例 |
| `run` | 组合 | notice payload = `drift_summary(report, duration_ms)` 逐字段 + 追加 `changedAssets`（投影单点，零第二份装配） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `Sender<AgentEventKind>` | 真实 `tokio::sync::mpsc::channel`（收端在用例侧 drain，不用替身——「发了什么、发了几条」即被测面） | `run` 全部用例 |
| `LoopTurn` / `cwd` | 真实结构直构 + 真实 `tempfile` 盘面（`LoopTurn` 是本 crate 内被测 API 的显式入参） | 全部用例 |
| 无（其余零 mock） | `spawn_blocking` / `assets_hash` / `drift::check` / `protect::written_paths` 全部真实 | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount_test.rs

#### 待测功能

- `build_registry(): 挂载清单 13 → 15（两新项 ToolTier::Read / ToolExec::Archi，注释与 Read 归组同步）`
- `archi_drift_definition(): 描述点名「gate 全 off 时 unmatchedFiles 是常态不是错误」与「files 只在排查归属问题时用」；scope enum 取 DRIFT_SCOPES、无 mode / root 参数`
- `archi_scaffold_definition(): kind enum 由 ElementKind::ALL 派生（省略取 domain）；描述含字段序 / 内联 metadata / path 必填提示 / 零写盘`
- `archi_module_definition(): 描述改述——补「缺 path 由 hasPath 一次判出」「入口解析结果随响应给出」「字段形态用 archi_scaffold」，删与工具面重复的语法教学`

<!-- 出处：「公共函数 / API」表未列该文件（`build_registry` / `archi_*_definition` 为私有面，契约由 D5 / D7 与 AC-2 / AC-3 / AC-7 承载）；上述条目取 design「修改文件」子表为该文件声明的标识符，断言面为既有出口的可观测产物（`registry()` / `lookup` / `definitions` 的静态清单与定义形状），不新增导出。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `registry()` 清单 | 正向 | 恰 15 项、名字唯一且有序；前七恰内建，`archi_drift` / `archi_scaffold` 插在 `archi_api` 之后、两 DB 工具之前 | 单例（期望序常量逐项断言） |
| `registry()` 的 tier / exec 归属 | 正向 | 两新项 `ToolTier::Read` + `ToolExec::Archi`；其余项归属与改造前逐项一致 | 表驱动（15 行） |
| `lookup` | 正向 | `lookup("archi_drift")` / `lookup("archi_scaffold")` 命中且与 `registry()` 同名项逐字段相等（同源单点） | 表驱动（2 行） |
| `lookup` | 异常 | 非法名 / 退役名 → 既有 `None` 语义不变 | 表驱动（3 行） |
| `archi_drift` 定义面 | 正向 | 描述含三要素：漂移读数语义、**「`gate` 全 `off` 时 `unmatchedFiles` 是常态不是错误」**、`files` 只在排查归属问题时用 | 单例 |
| `archi_drift` 定义面 | 边界 | `parameters.properties.scope.enum` 逐值等于 `DRIFT_SCOPES`、`required` 为空数组（零 `mode` / 零 `root` 参数） | 表驱动（逐值 + 键集） |
| `archi_scaffold` 定义面 | 正向 | 描述含「字段序 / 内联 `metadata` / `path` 必填提示 / 关系模板」「`kind` 省略取 `domain`」「零写盘、写面仍是 `write` / `edit`」 | 单例 |
| `archi_scaffold` 定义面 | 边界 | `parameters.properties.kind.enum` 与 `ElementKind::ALL.map(as_str)` **逐值相等**（长度 4、序一致） | 表驱动 |
| `archi_module` 描述改述 | 正向 | 含「缺 `path` 由 `hasPath` 一次判出」「入口解析结果随响应给出（显式 / 约定默认 / 不可用成因）」「新元素字段形态用 `archi_scaffold`」三句 | 表驱动（3 行） |
| `archi_module` 描述改述 | 边界 | 删除字段形态教学：零代码围栏、零 `entry` 三档表（关键词在描述中缺场）；`modelFile` + `modelLine` 可 `edit` 句保留 | 表驱动（2 行） |
| `definitions(names)` | 正向 | 名字清单含两新名 → 按 `registry` 序返回 15 项内对应子集，定义与 `registry` 逐字段相等 | 表驱动（2 行） |
| `definitions(names)` | 边界 | 空清单 / 重复名 / 空串元素 → 既有语义不变（零追加 / 去重语义 / `Err`） | 表驱动（3 行） |
| `injected_finish` 夹具 | 边界 | 夹具构造体的 `SessionInjections` 含 `protect: None`（第五位机械闭合，编译期 + 断言双守） | 单例 |
| 惰性单点 | 边界 | 两次 `registry()` 返回同一指针且内容相等（`OnceLock` 语义不变） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock） | `registry()` / `definitions` / `lookup` 全为进程内纯函数（`OnceLock` 静态表，零 IO / 零 spawn）；`ElementKind::ALL` 取真实常量 | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/archi_tool_test.rs

#### 待测功能

- `ArchiContext::execute(name, input): 分派面新增 archi_drift / archi_scaffold 两支（其余三支语义不变）`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `archi_scaffold` 草稿 | 正向 | 省略 `kind` → `{ "draft": <TOML> }`，草稿含 `kind = "domain"` 实值行、字段序注释（`gate`/`api`/`entry`/`path`/`doc`/`summary`/`metadata`）、内联 `metadata` 纪律行、`path` 必填提示 | 单例 |
| `archi_scaffold` 草稿 | 正向 | 四种 `kind` 逐值 → 草稿实值行逐值对应（`package` / `domain` / `module` / `component`） | 表驱动（4 行） |
| `archi_scaffold` 草稿 | 边界 | 草稿经真实 `TomlParser` 零结构诊断，且图构建零诊断（单段占位 FQN、关系块全注释） | 单例 |
| `archi_scaffold` 草稿 | 边界 | 关系部分全为注释行（零裸 `[[relation]]` 表，避免 `missingField`） | 单例 |
| `archi_scaffold` 草稿 | 边界 | 零写盘：调用前后 `tempdir` 盘面字节快照逐一相同 | 单例 |
| `archi_scaffold` 草稿 | 异常 | `kind` 值集外（`"packagex"` / `""` / `null` / 数字）→ `Err` 文案点名四值（`domain` / `module` / `component` / `package`） | 表驱动（4 行） |
| `archi_drift` scope | 正向 | 省略 `scope` → `summary` 投影（含计数与 `durationMs`，无 `observations` / 判据条目 / `files` 清单） | 单例 |
| `archi_drift` scope | 正向 | `scope = "entries"` → summary 键 + `observations` + 判据条目全量 | 单例 |
| `archi_drift` scope | 正向 | `scope = "files"` → entries 键 + `files` 四类清单 | 单例 |
| `archi_drift` scope | 边界 | 三 scope 逐级包含（`summary` ⊂ `entries` ⊂ `files`） | 表驱动（三组对照） |
| `archi_drift` scope | 异常 | `scope` 值集外（`"full"` / `""` / `null` / `"Summary"` 大小写变体）→ `Err` 点名合法值（`summary / entries / files`），零静默退化 | 表驱动（4 行） |
| `archi_drift` 恒全量 | 正向 | 入参仅 `scope`（零 `mode` / 零 `changed` / 零 `change`）→ 报告 `mode = "full"`、`change = null`、`base = null` | 单例 |
| `archi_drift` 错误面 | 异常 | 坏模型 / 架构域配置非法 → `Err` 文案含成因（经 `CheckError::message()` 回灌），零 panic | 表驱动（2 行） |
| `archi_drift` 常态量 | 边界 | `gate` 全 `off` 盘面 → 返回文本只在计数位含 `unmatched` 数，零 unmatched 路径串 | 单例 |
| `archi_drift` 截断 | 边界 | `files` scope 超大清单（超 30 KB）→ 尾部截断留痕，计数位仍在（`cap_result` 共用） | 单例 |
| `archi_drift` 零缓存 | 边界 | 连调两次：各自 `durationMs` 在场（每次独立计时）；两次之间改盘面 → 第二次读到新状态 | 表驱动（2 行） |
| `archi_drift` 空架构域 | 边界 | 无模型仓库 → `status = "skipped"` 正常出（不报错） | 单例 |
| `execute` 分派 | 异常 | 未知工具名 → 既有 `Err`「未知架构工具: …」不变 | 单例 |
| `execute` 分派 | 正向 | 既有三支（`archi_module` 两形态 / `archi_search` 三 scope / `archi_api` 两动作）语义与既有用例逐条保持绿 | 表驱动（回归） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock） | 真实 `tempfile` 模型盘面 + 真实 `archi::drift::check` / `dto` 投影 + 真实 `TomlParser` + 真实 `tools::cap_result`（30 KB 上限即被测契约） | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop_test.rs

#### 待测功能

- `run(turn, …): 自然收敛支 preclose::run(sender, turn, archi_touched, archi_baseline) 先于 finish_success（archi_baseline 由 preclose::baseline(turn) 在起手捕获，作 T2 的前值；archi 会话之外为 None）；其余四个出口零调用`
- `run 内局部量 archi_touched: 沙箱通过 ∧ 保护闸放行之后置位、跨轮累积（'continuation 循环之外声明，零 LoopTurn 新字段）`

<!-- 出处：「公共函数 / API」表未列该文件（`run` 是 `pub(crate)` 泵本体，本 change 只加挂点与两个局部量，契约由 D8 / D9 与 AC-4 承载）；上述条目取 design「修改文件」子表为该文件声明的标识符，断言面为可观测事件流（notice / `TurnDone` / 请求帧数）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 自然收敛支事件序 | 正向 | 文本收口一轮（T1 命中）→ `SystemNotice{archi_preclose}` 索引 < `TurnDone` 索引，恰一条 notice | 单例 |
| 自然收敛支事件序 | 正向 | `TurnDone` 的 `subtype` / `is_error` / `num_turns` / `usage` 逐字与改造前一致（notice 不改变收口形态） | 表驱动（2 行：成功 / 失败） |
| T1 跨轮累积 | 边界 | 第 1 轮 `write` 写模型目录、第 2 轮纯文本收口 → 仍发 notice（局部量在 `'continuation` 循环之外） | 单例 |
| T1 置位点 | 组合 | 相位锁拒绝的模型写入（`implement` 相位会话）→ 放行路径未达、`archi_touched` 不置位 → 随 T2 判定出结果而非 T1 误命中 | 单例 |
| 纯问答 | 边界 | 零写入且盘面未变 → 零 notice（只留引擎日志） | 单例 |
| 五出口（AC-4） | 异常 | 空 choice 正常收敛出口 → 零 notice | 表驱动（五出口各一行） |
| 五出口（AC-4） | 异常 | 截断终局（`FinishReason` 截断值）→ 零 notice | 表驱动 |
| 五出口（AC-4） | 边界 | `stop_requested` 先置位 → 零 notice（既有「不合成 `turn_done`」语义不变） | 表驱动 |
| 五出口（AC-4） | 边界 | `pause_turn` 到顶 → 零 notice | 表驱动 |
| 五出口（AC-4） | 异常 | 异常终局（`api_error` 重试耗尽）→ 零 notice，`TurnDone` 形态不变 | 表驱动 |
| 零请求增量 | 边界 | 预录流脚本帧数消耗与改造前相同（自检不追加请求）；`chat_history` 无新增消息 | 表驱动（2 行） |
| 会话判定 | 边界 | change 相位会话（`tools` 含 `archi_drift` 但 `preamble = None`）→ 零 notice（不误命中 `turn.archi`） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 模型请求 / 响应流（进程边界） | 既有 `rig::test_utils` 假流缝（`MockScript` + 脚本化 transport + `FakeState` 捕获 `CompletionRequest`）——外层模型 API 是进程边界 | 全部用例 |
| `Sender` / `RunHandle` | 真实 `mpsc` 通道与真实 `RunHandle`（事件序即被测面） | 全部用例 |
| 无（其余零 mock） | 真实 `tempdir` `cwd`、真实 `preclose` / `protect` / `mount` 组合 | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner_test.rs

#### 待测功能

- `mint_archi_context(names, workspace_root): any 判据扩为五 archi 名（module / search / api / drift / scaffold），零命中即 None`
- `ProtectRules::load(workspace_root).with_session(open.injections.protect): 会话规则由注入面传导至 LoopTurn.protect（每 run 一次装配，load 签名不动）`

<!-- 出处：「公共函数 / API」表未列该文件（`mint_archi_context` 为 `pub(crate)` 且本 change 只扩判据；`with_session` 的接线在本章观测，契约由 D10 / D15 与 AC-6 / AC-7 承载）；上述条目取 design「修改文件」子表为该文件声明的标识符。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `mint_archi_context` 判据 | 正向 | 五名任一在场（`archi_module` / `archi_search` / `archi_api` / `archi_drift` / `archi_scaffold`）→ `Some(context)` | 表驱动（5 行） |
| `mint_archi_context` 判据 | 边界 | 零 archi 名 / 空清单 → `None` | 表驱动（2 行） |
| `with_session` 传导 | 组合 | `open.injections.protect = Some([模型目录一条])` → 泵内写入模型路径被拒（拒绝串含模板文案），证明规则从 `open.injections.protect` 传导至 `LoopTurn.protect` | 单例 |
| `with_session` 传导 | 边界 | `protect = None` → 模型路径不被会话规则拦（既有内置规则行为不变） | 单例 |
| `with_session` 传导 | 边界 | `protect = Some([])` 与 `None` 对判定同效 | 表驱动（2 行） |
| `with_session` 传导 | 异常 | 会话规则 glob 为空串 / 畸形 → 不命中、不 panic（fail-open 同用户规则面） | 表驱动（3 行） |
| 双面同串 | 组合 | 拒绝时事件面 notice 文案与工具返回串**同一串**（零第二份文案） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| store / provider（进程边界） | 既有 `store` fixture + 本地假 provider（`http://127.0.0.1:9/v1`，零外网） | 传导端到端用例 |
| 无（其余零 mock） | `ProtectRules::load().with_session()` 真实调用、真实 `tempdir` 模型路径、真实 loop 泵 | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/compose.rs -> packages/desktop/src-tauri/crates/infra/agent/src/compose_test.rs

#### 待测功能

- `archi_matrix(phase, role): 7 行改 8 行（dev-design 执行 / 评估拆行 + drift / scaffold 两维），矩阵外兜底零 archi 工具`
- `model_write_lock(provenance): archi 臂与两处写权相位 → None；其余（含畸形 source_ref / explore / debug）→ Some(一条 graph/model glob 规则)`
- `injected(...) / assemble_injections(provenance): 三臂（explore / archi / change）与其余来源逐臂显式置位第五位 protect`

<!-- 出处：「公共函数 / API」表把 `archi_matrix` / `model_write_lock` / `assemble_injections` 明列私有面（「私有面按本表口径不列」，契约由 D13 / D15 与 AC-5 / AC-7 承载）；上述条目取 design「修改文件」子表为该文件声明的标识符，断言面为可观测产物（`SessionInjections` 的 `tools` / `protect`）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `archi_matrix` 工具面 | 正向 | 8 行逐格 × 5 工具维度：`proposal/executor` = module+search+scaffold；`dev-design/executor` = module+search+drift+scaffold；`dev-design/evaluator` = module+search；`test-design/executor` = search；`implement/executor` = module+api+drift；`code-review/evaluator` = module；`acceptance/evaluator` = module；兜底行 = 零 archi 名 | 表驱动（8 行） |
| `archi_matrix` 工具面 | 边界 | 矩阵外（`initialize` / `test-gen` / 未知 phase / `decision` 角色）→ `tools` 为变更臂基座清单，零 archi 名 | 表驱动（4 行） |
| archi 臂工具面 | 正向 | archi 臂 = 七内建 + 五 archi 名（12 名，序 = module / search / api / drift / scaffold）；不含 `create_explore_note` / 两 DB 工具（挂载清单 15 项另含） | 表驱动（2 行） |
| `model_write_lock` 逐格 | 正向 | 无锁格：`source = "archi"`（任意 `source_ref`）、`change` + `proposal/executor`、`change` + `dev-design/executor` → `protect = None` | 表驱动（4 行） |
| `model_write_lock` 逐格 | 异常 | 挂锁格：`dev-design/evaluator`、`implement/executor`、`test-design/executor`、`code-review/evaluator`、`acceptance/evaluator`、`explore`、`debug`、未知 source → `Some(1 条)` | 表驱动（8 行） |
| `model_write_lock` 逐格 | 异常 | 畸形 / 缺失 `source_ref`（`""` / `"a/b"` / `"a/implement"` / `"a/weird/executor/1"` / `None`）→ fail-closed 一律 `Some(1 条)` | 表驱动（5 行） |
| 锁规则内容 | 正向 | `glob == format!("**/{}/**", foundation::layout::archi_graph_model_rel_dir())`（经 layout 复算）、`reason == protect::session_lock_template()`（逐字） | 单例 |
| 锁面范围（AC-5） | 边界 | 该 glob 对 `models/**/*.md` / `graph/INDEX.toml` / 架构域 `config.json` 三条路径**不命中**（经 `checks::globmatch` 真实匹配面判定） | 表驱动（3 行） |
| 三臂置位 | 正向 | `explore` → `Some`；`archi` → `None`；`change` 白名单 → `None`；未命中来源 → `Some`（逐臂构造显式置位） | 表驱动（4 行） |
| 位间解耦 | 边界 | 矩阵未命中但锁在位（`implement/evaluator` → 零 archi 名 ∧ `protect = Some`）；archi 臂零锁但五工具全在 | 表驱动（2 行） |
| 调用点链路 | 组合 | `composed.begin` 捕获的 `injections` 与直调 `assemble_injections` 逐字段全等（含 `protect` 位）——archi 臂与 change 白名单两路各一次 | 表驱动（2 行） |
| 装配面失败 | 异常 | 非法工具名 → `Err` 且不返回部分注入面（既有语义，`protect` 位不引入新旁路） | 单例 |
| 端到端写权（AC-5） | 组合 | `assemble_injections` 产出 → `ProtectRules::load().with_session()` → 写入模型目录被拒，拒绝串含「proposal / dev-design」「不要改用其他命令或工具尝试绕过」 | 单例 |
| 源码自证 | 边界 | `archi_graph_model_rel_dir` 在 `compose.rs` 恰一处、`"proposal"` / `"dev-design"` 只在矩阵与 `model_write_lock` 两处、`Some(vec![` 只在 `model_write_lock` 内 | 表驱动（3 行） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| store / provider（进程边界） | 既有 `native_db` 内存库 fixture + 本地假 provider（沿用 `compose_test` 现有装置） | 调用点链路用例 |
| 无（其余零 mock） | `assemble_injections` / `archi_matrix` / `model_write_lock` 纯内存；`ProtectRules` 真实装配；glob 匹配走真实 `checks::globmatch` | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/agent/src/runner.rs -> packages/desktop/src-tauri/crates/core/agent/src/runner_test.rs

#### 待测功能

- `SessionProtectRule { glob, reason }: 线格式 glob / reason（reason 携 %s / %t 原样）；Debug + Clone + PartialEq + Eq，无 specta::Type（不进 TS bindings）`
- `SessionInjections.protect（第五位）: #[serde(default)] additive——旧线 JSON 缺字段读 None；四字段语义逐字不变`

<!-- 出处：「公共函数 / API」表只列函数出口，上述两处属 design「类型定义」表行（纯数据，契约由 D10 与 AC-6 承载）；无新增函数出口，故条目取类型 / 字段名。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `SessionInjections` 键面 | 正向 | 五键恰为 `preamble` / `tools` / `finish` / `ask` / `protect`（序列化键集逐项断言） | 单例 |
| `SessionInjections` 键面 | 正向 | 全字段在位 round-trip：序列化 → 反序列化逐字段相等（含 `protect: Some([1 条])`） | 表驱动（2 行：`Some` / `None`） |
| `SessionProtectRule` 线形 | 正向 | 线名为 `glob` / `reason`（类型无 `rename_all` 改写面）；`reason` 中的 `%s` / `%t` 原样往返（不做替换） | 单例 |
| 旧线兼容（serde additive） | 边界 | 旧线 JSON 缺 `protect` 键 → 读出 `None`（表驱动 4 形态：缺键 / `null` / 空对象 `{}` / 仅 `preamble`） | 表驱动（4 行） |
| 旧线兼容（serde additive） | 边界 | 未知字段忽略（additive 演进不破线格式） | 单例 |
| `default()` | 边界 | 五键全 `None` | 单例 |
| `Some([])` 与 `None` | 边界 | 两者序列化形态不同（`[]` vs `null`）且各自可读回；类型语义「装配层不产空数组」由取值面如实表达 | 表驱动（2 行） |
| 畸形 `protect` | 异常 | 非数组 / 元素缺 `reason` / `glob` 非字符串 → 反序列化 `Err`，零 panic | 表驱动（3 行） |
| `SessionProtectRule` 派生面 | 边界 | `Clone` + `PartialEq` + `Eq`：同值相等、差一字段不等（配置快照比较语义） | 表驱动（3 行） |
| 零 `specta` 面 | 异常 | 源码自证：`SessionProtectRule` 邻域零 `specta::Type`（不进 TS bindings） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock） | 纯 serde 值（`serde_json::to_value` / `from_value`）；不落盘、不建会话 | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/mod.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/mod_test.rs

#### 待测功能

- `ProtectRules::with_session(self, session): 纯数据合并——会话规则插在内置之后、用户之前`
- `protect::written_paths(command, cwd): 复用 shell 写面提取面（Write + Path），返回归一后路径`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `with_session` | 正向 | `Some([模型目录一条])` → 命中该 glob 的 `write` 返回模板替换后的完整文案（含路径 + 工具名） | 单例 |
| `with_session` | 边界 | `None` → 规则面与 `load` 直出逐条相等（零追加） | 单例 |
| `with_session` | 边界 | `Some([])` 与 `None` 判定结果同效 | 表驱动（2 行） |
| `with_session` | 边界 | 规则序（内置 → 会话 → 用户）：同一路径同时命中内置 ∧ 会话 ∧ 用户 → 返回**内置**文案 | 表驱动（3 组对照） |
| `with_session` | 边界 | 会话 ∧ 用户重叠（内置不覆盖该路径）→ 返回**会话**文案 | 单例 |
| `with_session` | 正向 | bash 写入面同样命中（`write` / `edit` 之外的 `bash` 分支）：`cat > <model>` / `tee <model>` / 重定向 → 拒绝 | 表驱动（3 行） |
| `with_session` | 边界 | 只读工具零介入（`read` / `grep` / `archi_module` / `ls`）→ `None` | 表驱动（4 行） |
| `with_session` | 边界 | 相对形态 glob（`**/<archi_graph_model_rel_dir>/**`）与绝对形态 glob 均命中 | 表驱动（2 行） |
| `with_session` | 异常 | 畸形 glob（空串 / `**` 单独 / 非法括号）→ 不命中、不 panic | 表驱动（3 行） |
| `with_session` | 边界 | `load` 签名不变：既有 `load(None)` / `load(Some(root))` 调用形态零改动（源码自证 + 编译期） | 单例 |
| `written_paths` | 正向 | `cat > a.toml` → 一条归一后绝对路径（`\` 归一为 `/`） | 单例 |
| `written_paths` | 正向 | 多写面（`tee a b` / 多 op 命令）→ 多条且序稳定 | 表驱动（2 行） |
| `written_paths` | 边界 | 空串 / 空白串 → 空数组 | 表驱动（2 行） |
| `written_paths` | 边界 | 只读命令（`ls` / `cat x` / `grep -n x y`）→ 空数组 | 表驱动（3 行） |
| `written_paths` | 边界 | 删除 / 还原形态（`rm x` / `git checkout -- x`）→ 空数组（`Delete` / `Revert` 不入，由 T2 兜底） | 表驱动（2 行） |
| `written_paths` | 边界 | 相对路径目标 → 按 `cwd` 解析为绝对形态 | 表驱动（2 行） |
| `written_paths` | 异常 | 不可解析目标（`~` / 空 target / 变量）→ 跳过该项、其余照返回，零 panic | 表驱动（3 行） |
| `written_paths` | 组合 | 与保护闸同一写面提取面：同一命令写入模型目录 → `written_paths` 命中 ∧ `evaluate` 拒绝（同一 `extract_file_ops` 面，零第二份解析） | 单例 |
| `evaluate` 回归 | 正向 | 既有三路判定（write/edit `path`、bash 三态、目录规则）逐条语义不变 | 表驱动（回归） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock） | `ProtectRules` 直构 + 真实 `rules::load(None)` 内置快照；真实 `shell::extract_file_ops` 与 `rules::resolve`；配置文件面用真实 `tempfile` + `layout::config_path` | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/rules.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/rules_test.rs

#### 待测功能

- `with_session_rules(rules, session): 会话规则插在内置段之后、用户段之前（插入位由 builtin_rules().len() 派生，零硬编码 1）`
- `ReasonKind::Session(template): 命中后由 deny_reason 做 %s / %t 全量替换（模板串单点在 reason.rs）`
- `load(workspace_root): 签名与内置 / 用户两段逐字不动（既有 ~47 处调用零改动）`

<!-- 出处：`with_session_rules` 为 `pub(super)`、`ReasonKind::Session` 属 design「类型定义」表行，均不在「公共函数 / API」表（契约由 D11 / D12 与 AC-6 承载）；上述条目取 design「修改文件」子表为该文件声明的标识符。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 会话规则装配 | 正向 | 会话规则插在**内置段之后、用户段之前**：规则向量位次 = `builtin_rules().len()..` 起 | 单例 |
| 会话规则装配 | 边界 | `session = None` → 与 `load` 直出规则向量逐条相等（零改动） | 单例 |
| 会话规则装配 | 边界 | `session = Some(&[])` → 同上（空与 `None` 同效） | 表驱动（2 行） |
| 会话规则装配 | 边界 | 多条会话规则保序（输入序即装配序） | 表驱动（2 组） |
| 会话规则装配 | 边界 | 插入位由 `builtin_rules().len()` 派生：源码自证零硬编码 `1` | 单例 |
| 会话规则装配 | 异常 | 畸形 glob（空串 / 无法归一）→ 不命中且不 panic（与用户规则同一失败面） | 表驱动（2 行） |
| 会话规则装配 | 正向 | `ReasonKind::Session(template)` 进入判定后由 `deny_reason` 做 `%s` / `%t` 替换（与规则构造面同一实例） | 单例 |
| 既有装配回归 | 正向 | 内置恒一条 + 用户规则合并语义、`bulk_globs`、四类归一函数逐条不变 | 表驱动（回归） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock） | 真实 `tempfile` 目录 + `layout::config_path` 派生的真实 JSON 配置文件（`config::load` 真实读取） | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/reason.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/reason_test.rs

#### 待测功能

- `session_lock_template(): 相位锁文案模板出口（四要素 + %s / %t 占位符）`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `session_lock_template` | 正向 | 四要素逐条在场：`%s`（路径位）+ `%t`（工具位）+ 「proposal / dev-design 相位的执行侧（以及架构页会话）」+ 补救指路（写进 `design.md` / `tasks.md` 交 `dev-design`，或改用架构页会话）+ 「不要改用其他命令或工具尝试绕过」 | 表驱动（5 断言） |
| `session_lock_template` | 边界 | 模板零架构域路径字面量（路径只经 `%s` 携入） | 单例 |
| `session_lock_template` | 边界 | 同值两次调用返回同一 `&'static str`（单点出口） | 单例 |
| `deny_reason(Session)` | 正向 | `%s` / `%t` 全量替换（占位符零残留），输出含实际路径与实际工具名 | 表驱动（3 组：`write` / `edit` / `bash`） |
| `deny_reason(Session)` | 边界 | 路径含特殊字符（空格 / `：` / emoji）→ 原样插入、不转义零 panic | 表驱动（3 行） |
| `deny_reason(Session)` | 边界 | 空 path / 空 tool → 占位替换为空串，文案结构完整 | 表驱动（2 行） |
| `deny_reason(Session)` | 异常 | 模板无占位符（构造 `ReasonKind::Session("固定串")`）→ 原样返回，不 panic | 单例 |
| 既有文案回归 | 正向 | 三类既有串（内置 / 用户回落 / 批量）与 `substitute` / `bulk_revert_reason` 逐字不变 | 表驱动（回归） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock） | 纯字符串函数；`ReasonKind` 值直构 | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/archi-knowledge.md -> packages/desktop/src-tauri/crates/infra/agent/src/archi_knowledge_test.rs

#### 待测功能

- `骨架文本（archi-knowledge.md）: B/C 分层内容面——零示例围栏 / 零字段语法清单 / 零 entry 三档长表 / 零 doc 判据细节，含 kind 四值语义、计数单源一行、看护配置一行`
- `A 档指路句: 每句携带工具名 + 该工具面确有的字段（D17 表逐行，含 path→hasPath、entry→resolvedEntry、字段形态→archi_scaffold）`
- `体量锚: 骨架 ≤ 2252 B、全文 ≤ 2867 B（实测值冻结），降级语义不变`

<!-- 出处：md 骨架无函数出口（经 `include_str!` 内嵌，产物由 `archi_knowledge::knowledge()` 出），「公共函数 / API」表无该文件行，契约由 D16 / D17 与 AC-8 承载；上述条目取 design「修改文件」子表为该文件声明的文本面与 design 事实校准 10 的体量门限。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 分层删除面 | 正向 | 骨架零代码围栏（`` ``` `` 计数 = 0）、零字段语法教学段（`kind`/`gate`/`api`/`entry`/`path`/`doc` 逐项形态教学句缺场）、零 `entry` 三档默认长表 | 表驱动（3 组） |
| 分层新增面 | 正向 | 含 `kind` 四值语义（四值逐值 + 语义短语：包根 / 容器 / crate / 组件文件）、计数复述单源一行、架构域看护配置一行（`ignore` / `exclude`） | 表驱动（3 行） |
| A 档指路表（D17） | 正向 | 逐句断言：`path` 缺不缺 → `archi_module` + `hasPath`；`entry` 解析 → `archi_module` + `resolvedEntry`；`doc` 仅 `api = none` → 判据指路；祖先先声明 → 结构诊断 `orphanHierarchy`；字段形态 → `archi_scaffold` + `draft`；漂移读数 → `archi_drift`；契约两轴 → `archi_api`；看护配置非法 → `archi_drift` 的 `is_error` | 表驱动（8 行） |
| A 档工具面自证 | 组合 | 骨架点名的每个 archi 工具名均在 `mount::registry()` 命中（指路不指向不存在的工具） | 表驱动（逐名） |
| 派生段方位 | 正向 | 派生段含模型目录 + 契约资产两处（`layout` 派生复算），**零**契约索引路径 | 表驱动（3 行） |
| 四面词表 | 正向 | `kind` / `gate` / `api` 状态 / 结构诊断九词逐值在场（既有断言保留，词表由 `ALL` 派生） | 表驱动（4 组） |
| 静默陷阱保留（R1） | 边界 | 带引号 FQN 单键、`metadata` 必须内联表、目录尾 `/`、容器 `path` 恒空四条 C 档纪律逐条在场 | 表驱动（4 行） |
| 保留项 | 边界 | 写面与可见性三句（直改主仓 / 禁止越界 / 画布写盘后不自动刷新）+ 历史会话续接一句在场 | 表驱动（2 行） |
| 体量锚 | 边界 | 骨架字节 ≤ 2252 B（实测值冻结断言） | 单例 |
| 体量锚 | 边界 | 全文（骨架 + 派生段）字节 ≤ 2867 B（实测值冻结断言） | 单例 |
| 占位符纪律 | 边界 | 骨架零 `<>` 占位符（既有断言保留） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock） | `include_str!("archi-knowledge.md")` 取编译期内嵌的同一份字节（禁测试内复制副本）；`layout` / `ElementKind::ALL` 等取真实派生函数 | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/archi_knowledge.rs -> packages/desktop/src-tauri/crates/infra/agent/src/archi_knowledge_test.rs

#### 待测功能

- `build(skeleton): 派生段收缩为四面词表 + 两处目录（删契约索引）；<> 占位拒绝与降级语义不变`
- `knowledge(): 输出 = 骨架 + 派生段（前缀相等）；骨架不可用即 None（既有降级不变）`

<!-- 出处：`build` / `knowledge` 为 `pub(crate)` 且不在「公共函数 / API」表（本 change 只改派生段内容与 doc 措辞，契约由 D16 与 AC-8 承载）；上述条目取 design「修改文件」子表为该文件声明的标识符。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 派生段目录 | 正向 | 输出含 `layout::archi_graph_model_rel_dir()` 与 `archi_models_rel_dir()` 两处（逐字复算），且**不含** `archi_graph_rel_dir()` 的索引串（`INDEX.toml` 词零命中） | 表驱动（3 行） |
| 派生段词表 | 正向 | 四面词表连接串逐字在场（`" / "` 单点连接形态） | 表驱动（4 组） |
| 拼装关系 | 组合 | `knowledge()` 全文 = `skeleton.trim_end()` + 派生段（包含关系 + 前缀相等） | 单例 |
| 降级语义 | 异常 | 骨架为空 / 仅空白 → `Err`；`knowledge()` 不 panic 且装配层只丢 `preamble`（既有断言保留） | 表驱动（2 行） |
| 占位符拒绝 | 异常 | 骨架含 `<>` → `build` 返回 `Err`（文案含「占位符」），`knowledge()` 降级 | 单例 |
| 模块 doc 纪律 | 边界 | 骨架自证纪律改述后仍保留「尽力增强、不炸 run」一条（doc 词面复核） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock） | 真实 `build` / `knowledge` + 真实 `include_str!` 骨架 + 真实 `layout` / 词表常量；零 IO 零 tempdir | 全部用例 |

---

### packages/desktop/src-tauri/crates/core/workflow/src/prompts/*.md -> packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table_test.rs

#### 待测功能

- `proposal-planner.md: 只读工具行补 archi_module / archi_scaffold；写面句（可经 write / edit 维护模型、只为已确定元素落笔、写完自查）`
- `dev-design-planner.md: 工具行补 archi_drift / archi_scaffold；写面自查句与既有 archi_module / archi_search 用法行并列（零字段形态长表、零路径模板）`
- `implementation-generator.md: 工具行补 archi_drift（改完代码自查未声明依赖）；另补「架构模型已锁」一句（implement 在锁定侧）`
- `锁定侧其余 11 份 md（test-design-planner / test-gen-generator / code-analyze-planner + 8 份 evaluator）: 各补「架构模型已锁」一句（中文措辞、零 ASCII archi、evaluator 零 backtrack）`
- `phase_table.rs 装配面（只读参照）: 14 条 include_str! 与角色表逐字不变`

<!-- 出处：14 份 md 是静态文本资产（「公共函数 / API」表无出口，契约由 D18 与 AC-9 承载）；上述条目取 design「修改文件」子表为该目录声明的逐文件写入面，断言面为 `phase_table_test` 的文本 / 矩阵读盘断言。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ARCHI_PROMPT_MATRIX` 扩维 | 正向 | 7 份命中文件 × 5 工具维度逐格：`proposal-planner` = module+search+scaffold；`dev-design-planner` = module+search+drift+scaffold；`dev-design-evaluator` = module+search；`test-design-planner` = search；`implementation-generator` = module+api+drift；`code-review-evaluator` = module；`acceptance-evaluator` = module | 表驱动（7 行 × 5 列） |
| 写权相位（两处） | 正向 | `proposal-planner` / `dev-design-planner` 各含写面句：可经既有 `write` / `edit` 维护模型（无专用写工具）、**只为已确定元素落笔**、写完用 `archi_drift` / `archi_module` 自查 | 表驱动（2 行 × 3 断言） |
| 锁定侧 12 份 | 正向 | 各含「架构模型已锁」一句并携「写进产物交 `dev-design` 落地」指路 | 表驱动（12 行） |
| 锁定侧 12 份 | 边界 | 该句零 ASCII `archi` 字样（逐行过滤该句断言） | 表驱动（12 行） |
| 锁定侧 12 份 | 边界 | 该句不指示路由 / 落账工具调用 | 表驱动（12 行） |
| 未命中矩阵的档案 | 边界 | 7 份整篇零 archi 工具名（既有断言随新矩阵更新后仍成立） | 表驱动（7 行） |
| 命中 md 路径模板 | 边界 | 7 份命中 md 零架构域路径字面量（`.dev-team/architecture` / `architecture/graph/model`） | 表驱动（7 行） |
| 「写入模型」断言 | 异常 | 「写入模型」措辞按相位期望：写权相位允许（仅在写面句内），锁定侧禁（改为「架构模型已锁」措辞） | 表驱动（14 行） |
| evaluator 纪律 | 边界 | 8 份 evaluator md 零 `backtrack` 字样（既有断言保留） | 表驱动（8 行） |
| 占位与 MCP | 边界 | 14 份零 `<change>` / `{{` / `__INCLUDE:` / `__MODEL_` / `__MCP:` 等占位片段，且静态非空 | 表驱动（14 行） |
| `archi_drift` / `archi_scaffold` 分布 | 正向 | 全目录 `archi_drift` 命中恰 2 份（`dev-design-planner` / `implementation-generator`）、`archi_scaffold` 命中恰 2 份（`proposal-planner` / `dev-design-planner`） | 表驱动（2 组） |
| 装配面回归 | 边界 | `crates/core/workflow/src/write/phase_table.rs` 的 14 条 `include_str!` 与角色表逐字不变（源码自证） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock） | 真实文件读取（`prompts` 目录遍历 + `fs::read_to_string`）；不 mock 文件系统 | 全部用例 |

---

## 不可测试项

| 模块 / 面 | 原因 |
|---|---|
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mod.rs` | 本 change 只加两行 `pub(crate) mod …`（`archi_drift_view` / `preclose`）注册，无运行时可断言行为；缺注册即编译 / 模块解析失败，判据由 `cargo check --all-targets` 承担，不设用例。 |
| `packages/desktop/src/types/generated/bindings.ts` / `bindings.zod.ts` | 生成物，禁手改（AC-10）。断言面是「再生成后 diff 恰为 `ArchiModuleDetail.resolvedEntry` + `ArchiResolvedEntry` + `EntryVia`、`ArchiElementBrief` 零出现」，只能由静态链（`server:check` → `bindings:export` → `api:check` → `client:check`）与 `git diff` 复核完成，且由桌面端执行，agent 内不重复执行。 |
| `packages/desktop/package.json` 版本行 / `src-tauri/tauri.conf.json` 跟随 | 数据行（AC-12），无运行时行为；判据为 `git diff` 复核（`0.7.3` / 路径引用未改 / `src-tauri/Cargo.toml` 版本不随动），由桌面端执行。 |
| 前端 DTO 夹具随动（`src/views/architecture/hooks/use-archi-module.test.ts`、`components/architecture-canvas.test.tsx`、`architecture-view.test.tsx`、`components/model-info.test.tsx`、`src/lib/ipc-contract.test.ts`） | 这些文件本身就是测试文件，本 change 对它们的改动只是补齐必填字段 `resolvedEntry`（三字段齐备）——机械闭合、零新增断言与零新增用例语义；缺口由 `tsc`（`client:check`）强制暴露，不构成新测试设计项。 |
| `assets_hash` 的「单文件读失败按空字节参与」分支 | OS 级读失败（权限 / 句柄）无法在 `tempfile` 单测中确定性注入；用例侧以「子目录混入资产根被跳过」「缺失目录不参与」两个相邻形态覆盖稳定性，读失败口径以源码复核兜底（沿 spec 的保守方向，不新增重试）。 |
| `preclose::touched` 的 Windows 盘符 / 大小写失配假阴面 | 平台相关真实盘面差异无法在跨平台单测中稳定注入；用例以「显式大小写失配 → `false`」与「不同盘符前缀 → `false`」两个负向形态近似，实际漏判由 T2（`assets_hash` 变化）兜底——这是设计已登记的判据（不为它扩提取面、不加路径折叠）。 |
| AC-11（全管线绿 / `api:check` 零 diff） | 整链静态与全量测试门禁，由桌面端执行；本设计只保证各文件的用例落点覆盖 AC-1…AC-9 的断言面。 |
