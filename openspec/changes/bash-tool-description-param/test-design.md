# 测试设计: bash-tool-description-param

> **依据**: `design.md`「变更清单 - 修改文件」4 项（3 处源码 + 1 行版本号）、「公共函数 / API」子表（唯一条目 `bash.rs::execute`）、「文案台账」逐字文案、"验收标准落点"表与「影响传导」表的既有夹具点位。
> **框架识别**: Rust 侧为 `#[tokio::test]` / `#[test]` 行内单测，测试文件与产品文件同目录同主干（`bash.rs` ↔ `bash_test.rs`），断言用 `assert!` / `assert_eq!` / `expect_err`（既有 `cargo nextest run -p agent-runtime` 运行器）；前端为 `vp test`（vitest 语义）+ `@testing-library/react`，测试文件同目录 `*.test.tsx`，断言用 `expect` / `toBe` / `textContent`（`// @vitest-environment jsdom` 头注）。
> **范围**: 只做测试设计（路径映射 / 待测功能 / 用例 / Mock 策略），不含测试运行命令细节与评估结论。

## 验收范围

| AC ID | 验收条件 | 被测文件或模块 |
|---|---|---|
| AC-1 | 定义面：`bash` 的 `required` 逐值等于 `["command","intent"]`；`properties.intent` 为 `{"type":"string"}` 且**无 `maxLength`**；工具级描述与字段描述分工合力点出「折叠态只读这一行 / 建议 ≤30 字 / 写目的不复述命令」；其余六定义逐字不变 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs` |
| AC-2 | 意图闸（反向）：`{}`、只带 `command`、`intent` 为数字 / 对象 / `null`、`intent` 为 `"   "` 均 `Err` 且文案含 `intent`；`{}` 的 `Err` 文案含 `command`（校验序） | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/bash.rs` |
| AC-3 | 意图闸（正向 + 软约束）：合规 `intent` 下命令正常执行、输出与合并口径不变；200 字长 `intent` 零 `Err` | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/bash.rs` |
| AC-4 | 执行面零回归：补 `intent` 后既有 bash 用例断言逐条不变（stdout/stderr 合并、非零退出码、超时与 `[1s,30min]` 越界、`command` 空白、git-bash 探测与 `cmd /C` 兜底） | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/bash.rs` |
| AC-5 | 折叠态正向：未展开时 `trace-title` 文本为 `Bash · 跑单测`；折叠态文本不含命令原文 | `packages/desktop/src/components/agent/message-cards/tool-cards.tsx` |
| AC-6 | 折叠态回落：`intent` 缺席 / 空串 / 纯空白 / 非字符串 → 标题逐字 `Bash`，无 `·` 残留、无占位符、无 `undefined`/`null` 文本、不抛错 | `packages/desktop/src/components/agent/message-cards/tool-cards.tsx` |
| AC-7 | 单条省略链：`trace-title` 在场且为唯一文本承载（零 `trace-description`），超长 `intent` 原文完整保留（JS 零截断），零 tooltip / `title` 属性 | `packages/desktop/src/components/agent/message-cards/tool-cards.tsx` |
| AC-8 | 展开态与状态零回归：展开后 `code-block-header` 仍为 `command` 原文、`block-tool-result` 仍为 `output.content`、三态 `data-status` 与图标互斥、卡根钩子、`BUILD_IN_TOOL_CARDS` 键面零变化 | `packages/desktop/src/components/agent/message-cards/tool-cards.tsx` |
| AC-9 | 转录自足（重放同形）：`intent` 只经 `ToolUseBlock.input` 进转录，回放块 `input` 原样含该键；前端重放拿到同一 input → 折叠态同形 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/bash.rs`（转录块形状面）、`packages/desktop/src/components/agent/message-cards/tool-cards.tsx`（重放渲染面） |
| AC-10 | 零外溢：`protect/**`、`sandbox.rs`、`tools.rs::input_path`、`context.rs`/`compact.rs`、`crates/core/agent/**`、`crates/infra/store/**`、`trace.tsx`、`blocks.tsx`、16 份 phase 提示词、`bindings:export` 产物零 diff；版本行相对 `HEAD` 恰好 +1 patch | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs`、`sdk/bash.rs`、`components/agent/message-cards/tool-cards.tsx`、`packages/desktop/package.json`（无单元测试路径，`git diff` 门禁复核，见 `## 不可测试项`） |

## 单元测试

### `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs` -> `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount_test.rs`

#### 待测功能

<!-- design.md「公共函数 / API」子表未声明该文件的公共 API 变更：generic_definition(name) 为模块私有函数（design 明列「私有函数不列」），definitions(...) / TOOL_NAMES 签名与实现零改动。故本映射对无 design 声明的公共 API 条目；下列用例经既有 definitions(...) / injected(...) 的产出面（工具定义载荷）断言本变更的定义形状。 -->

- definitions(): bash 定义产出形状——`required` 逐值 `["command","intent"]`、`properties.intent` 为 `{type:"string"}` 且无 `maxLength` 等长度键、工具级描述前缀不动而末尾追加强制句
- definitions(): 七工具清单规模（`defs.len() == 7`、名序等于 `TOOL_NAMES`）与其余六定义（read / grep / glob / ls / write / edit）零漂移

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `definitions(...)` 的 bash 定义 `parameters.required` | 正向 | 逐值（有序、长度 2）为 `["command","intent"]` | 单例 |
| 同上 `parameters.properties.intent.type` | 正向 | 等于 `"string"` | 单例 |
| 同上 `parameters.properties.intent` 的键集 | 异常 | 恰为 `{type, description}`——`maxLength` / `minLength` / `pattern` / `enum` 四键均缺席（长度不可升为约束） | 表驱动（for 循环 4 键） |
| 同上 `properties.intent.description` 的取值要素 | 正向 | 四要素齐全：「一句话」（写什么）、「折叠状态你只读这一行」（可见性）、「30 字」（软长度）、「复述命令」（写目的不写命令） | 表驱动（for 循环 4 片段） |
| 同上工具级 `description` 前缀 | 边界 | `starts_with("执行 shell 命令并返回合并输出")`（既有三段逐字未动） | 单例 |
| 同上工具级 `description` 末尾追加强制句 | 正向 | `ends_with("；每次调用必须给 intent（缺失或空白即拒绝执行）")` | 单例 |
| 同上工具级 `description` 的文案单点 | 边界 | 不含「30 字」/「复述命令」（取值纪律只在字段描述出现，工具级只讲强制与后果） | 表驱动（for 循环 2 片段） |
| 同上 `required` 的封闭性 | 边界 | 不含 `timeout_ms`（仍可选）；长度恰 2（不多不少） | 单例 |
| 同上 `properties` 的键集 | 边界 | 恰为 `{command, intent, timeout_ms}`——无缺失、无多余属性 | 单例 |
| 其余六定义（read / grep / glob / ls / write / edit）零漂移 | 正向 | 既有断言（名称序 / `required` / 描述 / 关键属性缺口）零改动即绿 | 沿用既有（零改动） |

> 属性**顺序**（`command` → `intent` → `timeout_ms`）不作用例判据：`serde_json::Map` 缺省按键排序，对 `json!` 字面量断言键序无判别力；位置要求由实现相位的读码复核承担（design「文案台账」已钉）。

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock） | `registry()` / `definitions(...)` / `injected(&[...])` 全为进程内纯函数与内存夹具，不替身 `serde_json` 字面量（铸造出的定义载荷本身即被测面，替身会掩盖形状） | 全部用例 |
| 产品源文件读取（`source_of("src/sdk/mount.rs")`，既有夹具） | 真实读盘（`CARGO_MANIFEST_DIR` 相对），不 mock | 若需补「文案单点」的源码级命中数断言时复用既有夹具（只在 schema 面不可判时使用，优先走上表 schema 断言） |

### `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs` -> `packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools_test.rs`

#### 待测功能

<!-- design.md「公共函数 / API」子表未声明该文件的公共 API 变更：本变更在 tools_test.rs 的落点是既有「七工具定义面快照」断言的扩位与执行夹具的必填补位，被测源文件（mount.rs 的定义铸造）无公共 API 签名变化。 -->

- definitions(): 七工具定义面快照的 bash 行扩位（`required` 两值有序、描述前缀锚零改、`properties.intent` 在场且为 `string`），其余六行逐字不变
- definitions(): 非期望形态判据——`maxLength` / `minLength` 任一在场、或 `required` 含 `timeout_ms` / 长度偏离 2 即判红

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 快照表 bash 行的 `required` | 正向 | 既有 `expected` 数组中 bash 项由 `&["command"]` 扩为 `&["command","intent"]`，逐值有序匹配 | 表驱动（既有 `expected` 数组扩位） |
| 快照表 bash 行的描述前缀锚 | 边界 | `starts_with("执行 shell 命令并返回合并输出")` 零改即成立（追加句落末尾，插前缀即红） | 沿用既有（零改动） |
| 快照表其余六行 | 正向 | `read` / `grep` / `glob` / `ls` / `write` / `edit` 的 `required` 与描述锚逐字不变 | 沿用既有（零改动） |
| 定义清单规模与同序 | 边界 | `defs.len() == 7` 且名序等于 `TOOL_NAMES`（既有断言零改） | 沿用既有（零改动） |
| `properties.intent` 在场性 | 正向 | 既有「定义清单恰为七工具且名称/描述/schema 齐全」用例内补 `properties.intent.is_object()` 与 `properties.intent.type == "string"` | 单例 |
| `properties.intent` 的长度约束缺席 | 异常 | `maxLength` / `minLength` 任一在场即为非期望形态（长度升级为协议约束）→ 断言失败；`required` 含 `timeout_ms` 或长度偏离 2 亦同 | 表驱动（for 循环 3 组反形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（零 mock） | 定义面为进程内纯函数（`defs(...)` + `SessionInjections::default()` 内存夹具），不 mock | 上表五个定义面用例 |
| 子进程 / 文件系统（本文件内的执行夹具，供 AC-4 传导） | 真实 spawn + `tempfile::TempDir` 合成执行根，不 mock 壳与进程（真实执行为断言对象） | `bash.rs -> tools_test.rs` 章节所列夹具（本节不含执行面） |

### `packages/desktop/src-tauri/crates/infra/agent/src/sdk/bash.rs` -> `packages/desktop/src-tauri/crates/infra/agent/src/sdk/bash_test.rs`

#### 待测功能

- execute(): bash 执行体——`command` 闸之后、`resolve_timeout_ms` 之前新增意图闸（缺失 / 非字符串 → `Err("入参缺失或非字符串字段: intent")`，`trim` 后为空 → `Err("intent 不得为空")`），不设长度分支，`Ok` 分支的合并输出与 `Err` 分支的超时 / 非零退出码文案逐字不变（`pub(crate) async fn execute(root: &Path, input: &Value) -> Result<String, String>`，签名与可见性不变）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `execute()` 合规意图 | 正向 | `{command:"echo sdk-intent-ok", intent:"跑一条回声命令"}` → `Ok` 且输出含 `sdk-intent-ok` | 单例 |
| `execute()` 长度软约束 | 正向 | `intent` 为 200 字中英混排长串 → 命令照常执行、零 `Err`、输出含 marker | 单例 |
| `execute()` 超长 `intent` | 边界 | `intent` 为 1200 字符（>1000）→ 仍 `Ok`（无长度分支） | 单例 |
| `execute()` 首尾空白 | 边界 | `intent = "  跑单测  "` → `trim` 后非空 → `Ok` | 单例 |
| `execute()` 特殊字符 | 边界 | `intent` 含 `\n` / `\t` / emoji / `#`（`"跑\n单测 🚀\t#1"`）→ `Ok`（内容不参与判定） | 单例 |
| `execute()` 单字符意图 | 边界 | `intent = "读"` → `Ok` | 单例 |
| `execute()` 校验序（command 先报） | 异常 | `{}` → `Err` 含 `command`、不含 `intent` | 单例 |
| `execute()` 校验序（`command` 空白先报） | 异常 | `{command:"   "}` → `Err` 含 `command`（既有断言零改） | 单例 |
| `execute()` 意图缺失 | 异常 | `{command:"echo x"}`（只带 `command`）→ `Err` 含 `intent` 与「入参缺失或非字符串字段」 | 单例 |
| `execute()` 意图非字符串 | 异常 | `intent` ∈ `1` / `1.5` / `{}` / `[]` / `null` / `true` → 四类均 `Err` 且文案含 `intent`（`as_str` 一致返 `None`） | 表驱动（for 循环 6 组） |
| `execute()` 意图空白串 | 异常 | `intent` ∈ `""` / `"   "` / `"\t\n"` → `Err` 含 `intent` 且含「不得为空」 | 表驱动（for 循环 3 组） |
| `execute()` 意图闸先于超时闸 | 边界 | ① `{command:"echo x", timeout_ms:0}` 缺 `intent` → `Err` 含 `intent`（非 `timeout_ms`）；② `{command:"echo x", intent:"说个理由", timeout_ms:0}` → `Err` 含 `timeout_ms` | 表驱动（2 组） |
| `execute()` 意图闸先于 spawn | 边界 | 缺 `intent` 的命令带留产物副作用（`echo x > leak-from-intent.txt`）→ `Err` 后文件不存在 | 单例 |
| `resolve_timeout_ms()`（既有直测） | 边界 | 三条既有入参闸用例零改即绿（缺省 120s / 上下界原样放行 / 越界与非整数 `Err`）——证明意图闸插点未扰动超时面 | 沿用既有（零改动） |
| 既有执行夹具扩位（输出合并面） | 正向 | stdout 回灌 / stderr 合并（`[stderr]` 居中）/ cwd 承接 workspace root / 未知命令名 `Err` 四夹具补 `intent` 后断言逐条不变 | 参数化（既有夹具扩位 4 处） |
| 既有执行夹具扩位（异常与边界面） | 异常 | 非零退出码 `Err` 携退出码与全部输出 / 超时收口（1s 护栏 + 进程树清理文案）/ `timeout_ms` 越界不执行（零产物泄漏）/ git-bash 探测与 `cmd /C` 兜底 五夹具补 `intent` 后断言逐条不变 | 参数化（既有夹具扩位 5 处） |
| 纯保护判定面（`protect/mod_test.rs`、`protect/reason_test.rs`、`policy_test.rs`） | 边界 | 零改动即绿——只读 `command` 做拦截判定、不入执行体（出现 diff 即说明意图闸错放进保护面） | 沿用既有（零改动） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 子进程（shell 壳与命令） | **不 mock**：真实 spawn（`exec::resolve_shell` → git-bash / `cmd /C` / `sh -c`）是本文件断言对象（合并口径、退出码、超时收口、底座兜底） | 全部 `execute()` 用例 |
| 文件系统 | 真实 `tempfile::Builder` 合成临时执行根，仅作 workspace root 载体；产物存在性为断言面 | cwd 承接、零产物泄漏、兜底执行 |
| 进程环境变量 `PATH`（全局变量） | 真实替换 + 既有 crate 级 `TEST_PATH_LOCK` 串行锁（`std::env::set_var("PATH","")` 窗口内互斥），不替身；仅 `#[cfg(windows)]` 用例使用 | git-bash 探测退 `cmd /C` 兜底、超时收口（`ping` 经 PATH 解析） |
| `resolve_timeout_ms` 常量真值 | **不从产品侧导入**：测试侧独立声明 `MIN/MAX_TIMEOUT_MS`（既有做法，导入即与被测方同源，常量搬错时两侧一起错） | 入参闸边界用例 |

### `packages/desktop/src-tauri/crates/infra/agent/src/sdk/bash.rs` -> `packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools_test.rs`

> 组合用例（链路：`sdk/tools.rs::execute` 分发 → `sdk/bash.rs::execute` 意图闸 → 子进程）。入口模块 = `sdk/tools.rs`（分发层，`input` 原样透传、零字段感知，零改动），测试文件沿用其既有 `tools_test.rs`；被测闸在新提交的必填面上。

#### 待测功能

- execute(): 意图闸经 `tools::execute(root, "bash", input)` 分发进入后的可观测面（两处既有执行夹具 + 缺参用例；所在文件 `bash.rs`，分发层 `tools::execute` 签名与实现零改动故不列）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 分发正向：`tools::execute(root,"bash",…)` | 正向 | `{command:"echo bash-arm-ok", intent:"验证分发链路"}` → `Ok` 且输出含 `bash-arm-ok` | 单例 |
| 分发缺参不落「未知工具」 | 异常 | `{}` → `Err` 含 `command`（既有断言零改，`bash` 已入册） | 单例 |
| 分发缺意图不落「未知工具」 | 异常 | `{command:"echo x"}` → `Err` 含 `intent` 且不含「未知工具」 | 单例 |
| L1 截断收口口径 | 边界 | 60KB stdout 非零退出夹具补 `intent` 后仍 `Err` 携全部输出（分发层 L1 截断面零变化） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 子进程 / 文件系统 | 真实 spawn + `tempdir` 合成根，**不 mock**（L1 截断与分发路由均由真实执行驱动） | 全部四个用例 |
| 无其它 mock | 不替身 `tools::execute` 或 `bash::execute`（跨模块真实组合是本组用例的判据来源） | 全部四个用例 |

### `packages/desktop/src-tauri/crates/infra/agent/src/sdk/bash.rs` -> `packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop_test.rs`

> 组合用例（链路：`sdk/loop.rs` 守卫链 → 写保护闸 → 执行臂路由 → `tools::execute` → `bash::execute` 意图闸）。入口模块 = `sdk/loop.rs`（守卫链与路由，零改动），测试文件沿用其既有 `loop_test.rs`。判据：写保护闸在 `loop.rs:658` 先于执行臂路由（`:668`），**拒绝臂不触 `bash::execute`**，故拒绝臂对意图闸零敏感。

#### 待测功能

- execute(): 意图闸在会话级工具循环中的可观测面（五处走真实执行的 bash 夹具与转录自足断言；所在文件 `bash.rs`，守卫链 `loop.rs` 零改动故不列）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 六工具直驱 bash 臂 | 正向 | `explore轮六工具下发面内调用逐一正常执行` 的 bash case 补 `intent`（`{command:"echo loop-six-ok", intent:"验证直驱路径"}`）→ 零拒绝记因、`ToolResult` 非 `is_error`、输出含 `loop-six-ok` | 表驱动（既有 `cases` 数组扩位） |
| 命令分发臂 | 正向 | `写入保护闸_bash三路写法各自命中` 的放行样本补 `intent` → 既有断言逐条不变 | 表驱动（既有 `cases`） |
| 写保护放行臂 | 正向 | `写入保护闸_未命中照常执行且文件真实生成` 的 `echo foo > notes.txt` 补 `intent` → 文件真实生成（`intent` 未进判别链、闸未误判） | 单例 |
| 写保护拒绝臂 | 边界 | `find . -delete` / `echo x > <config>` / `cp /tmp/x <config>` → 仍以写保护记因拒绝，先于执行体（补或不补 `intent` 都不改变结论） | 表驱动（3 处） |
| 同批结果配对 | 正向 | `finish_step同批结果进恰一条user消息` 的 `bash` 调用补 `intent` → 恰一条 user 消息、`ToolResult` 按调用序配对（R5 假绿风险的自查点） | 单例 |
| 意图闸在会话链路中的失败面（R5 假绿防护） | 异常 | 故意留一处 bash 调用**不带** `intent` → 该 `ToolResult` 的 `is_error == true` 且内容含 `intent`（断言不止于「结果条数 == 1」，硬闸不得被静默吞掉） | 单例 |
| 转录自足（AC-9） | 边界 | ① 带 `intent` 的 `ToolUse` 块 `input` 原样含 `intent` 键（`serde_json::Value` 不丢字段、零新列）；② 旧转录（`input` 无 `intent`）块形状不变、`ToolResult` 正常收口 | 表驱动（2 组） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 模型接口（进程边界的外部流） | 沿用既有 `FakeModel::with_turns(...)` 替身注入模型输出（本变更新增字段只改夹具的 `json!` 载荷，不新增 mock 面） | 全部会话级用例 |
| 子进程 / 文件系统 | 真实 spawn + `tempdir` 合成 workspace root；写保护放行断言依赖真实产物，**不 mock** | 放行臂、命令分发臂、六工具直驱臂 |
| 写保护闸 | **不 mock**：真实 `turn.protect.evaluate(...)` 参与判定（拒绝臂的判据正是闸先于执行体） | 拒绝臂三处 |

### `packages/desktop/src/components/agent/message-cards/tool-cards.tsx` -> `packages/desktop/src/components/agent/message-cards/tool-cards.test.tsx`

#### 待测功能

<!-- design.md「公共函数 / API」子表未声明该文件的公共 API 变更：BashCard 在本变更前后均为模块私有组件（DD5 零新导出 / 零新 prop / 不改 AgentToolCardProps），BUILD_IN_TOOL_CARDS 键面与引用不变；下列用例经注册表 BUILD_IN_TOOL_CARDS['bash'] 的渲染面（既有 renderToolCard 夹具）驱动。 -->

- BashCard(): 折叠态标题——`intent` 经 `textOf(...)?.trim() ?? ''` 归一后在场时逐字 `Bash · <intent>`（单文本节点、零 JS 截断、唯一文本承载），缺席 / 空串 / 纯空白 / 非字符串时逐字回落 `Bash`
- BashCard(): 展开态与状态零回归——`code-block-header` 仍为 `command` 原文、`block-tool-result` 仍为 `output.content`、三态 `data-status` 与 Spinner ↔ TerminalIcon 互斥、卡根钩子与 `BUILD_IN_TOOL_CARDS` 键面不变、实时路与重放路同形

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `BashCard` 折叠态标题（AC-5） | 正向 | `{command:"pnpm -C packages/desktop exec vp test run", intent:"跑单测"}` 未展开 → `[data-slot="trace-title"]`.textContent 逐字 `Bash · 跑单测`（空格 + `·` + 空格） | 单例 |
| 折叠态不含命令原文（AC-5） | 正向 | 同一夹具未展开 → 卡根 `textContent` 不含 `pnpm -C packages/desktop exec vp test run` | 单例 |
| `BashCard` 折叠态回落（AC-6） | 异常 | `input` ∈ `{command:"ls -la"}`（缺席）/ `{command:"ls -la", intent:""}` / `{command:"ls -la", intent:"   "}` → title 逐字 `Bash`、不含 `·`、不含占位符 | 表驱动（`it.each` 3 组） |
| 折叠态回落：非字符串意图（AC-6） | 异常 | `intent` ∈ `1` / `{}` / `[]` / `null` / `true` → title 逐字 `Bash`、无 `undefined` / `null` 文本、渲染不抛错 | 表驱动（`it.each` 5 组） |
| `intent` 首尾空白归一（DD2） | 边界 | `intent = "  跑单测  "` → title 逐字 `Bash · 跑单测`（只 trim 首尾） | 单例 |
| `intent` 内部空白不折叠（DD2） | 边界 | `intent = "跑\n单测"` → 文本节点保留 `\n`（不改写、不折行、不合并空白） | 单例 |
| 超长 `intent`（AC-7） | 边界 | `intent` 为 1200 字符 → title 以 `Bash · ` 起、`textContent` 含完整原文（JS 零截断，省略交 CSS `truncate`） | 单例 |
| 特殊字符 `intent` | 边界 | `intent = "跑单测 🚀\t#1"` → 原文逐字进文本节点（emoji / 制表符零变形） | 单例 |
| 单条省略链（AC-7） | 边界 | 带 `intent` 夹具下：`[data-slot="trace-title"]` 恰 1 个、`[data-slot="trace-description"]` 0 个、标题节点无 `title` 属性（零 tooltip） | 单例 |
| 既有 `getByText('Bash')` 精确断言（AC-8 / R6） | 边界 | 既有 `BashCard：output 三态` 夹具 `{command:'ls -la'}` **不加** `intent` → 回落路标题只有一个文本节点，`within(cardRoot()).getByText('Bash')` 零改命中（唯一允许的既有断言改写点保持不动） | 沿用既有（零改动） |
| 展开态命令栏零回归（AC-8） | 正向 | 带 `intent` 夹具展开 → `[data-slot="code-block-header"]`.textContent 逐字等于 `command` 原文（命令不上提折叠态） | 单例 |
| output 三态零回归（AC-8） | 异常 | `running`（`output` 为 `null` + `turnRunning`）/ `error`（`isError` 真）/ 无线三态 `data-status` 与 Spinner ↔ TerminalIcon 互斥判据逐条不变 | 沿用既有（零改动） |
| 异常载荷零回归（AC-8） | 边界 | `input` ∈ `{}` / `null` / 裸字符串 → title 逐字 `Bash`、展开后命令栏空串（既有四形态 `it.each` 零改即绿） | 表驱动（既有 `it.each`） |
| 卡根钩子与注册表（AC-8） | 正向 | `data-testid="block-tool-use"` + `data-tool-id` 恰 1 个；`BUILD_IN_TOOL_CARDS` 键面与顺序不变（既有 `cardAttrs` / 注册表用例零改） | 沿用既有（零改动） |
| 重放同形（AC-9） | 正向 | 同一 `{command, intent}` 载荷经「实时路」（`output: null`）与「重放路」（`output` 在场）各渲染一次 → 两次 `trace-title` 文本逐字相同 | 表驱动（2 组） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@/lib/highlighter`（进程边界：shiki 语法包的动态 import 与重型 grammar 装载） | 沿用既有 `vi.hoisted` + `vi.mock` 替身（`loadHighlighterMock`），每例 `beforeEach` 重置 | 全部渲染用例（`CodeBlock` 展开面） |
| 无新增 mock | `BashCard` 本变更零新 I/O、零新依赖、零新 hook：注册表 / `lib/parse`（`asRecord` / `textOf`）/ `Trace*` / `CodeBlock` 全部真实组合，不为测试新增导出 | 全部渲染用例 |

## 不可测试项

| 文件或模块 | 原因 |
|---|---|
| `packages/desktop/package.json`（仅 `version` 行） | 版本号是交付纪律项，非行为单元：无公共函数 / API、无输入输出面，无法解析测试路径。判据由桌面端 `git diff HEAD -- packages/desktop/package.json` 的「版本行恰好 +1 patch」静态复核承担（AGENT.md 版本纪律 + design DD7）；`src-tauri/tauri.conf.json` 经 `"version": "../package.json"` 跟随、`src-tauri/Cargo.toml` crate 版本行不随动，同为 `git diff` 判据。 |
| AC-10 的零 diff 边界面（`sdk/protect/**`、`sdk/sandbox.rs`、`sdk/tools.rs::input_path`、`sdk/{context,compact}.rs`、`crates/core/agent/**`、`crates/infra/store/**`、`components/ui/trace.tsx`、`message-cards/blocks.tsx`、16 份 `crates/core/workflow/src/prompts/*.md`、`AGENT.md`、`src/lib/{agent-adapter,parse}.ts`、`src/types/generated/**`） | 判据是「本变更对这些文件**零改动**」，属 `git diff --name-only` 的静态门禁，不是可被单测断言的运行时行为（写测试断言「文件未修改」会与既有断言面重复且无判别力）。相关模块的既有测试（`protect/**` 三文件、`policy_test.rs` 等）以「零改动即绿」的方式间接承证：若意图闸错放进保护面，那些夹具会因缺 `intent` 而红。 |
| `bindings:export` 产物（`packages/desktop/src/types/generated/**` 再生成结果） | 「零 diff」由既有生成器再跑一次后比对，属静态生成物校验，无测试路径（design「路由 / API 设计」：本变更不出线新 IPC 类型）。 |
