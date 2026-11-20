# 测试设计: config-agent-session

> **变更**: config-agent-session
> **来源**: `design.md`（DD1–DD15 / 事实校准 8 条 / 变更清单三子表 / 公共函数与 API / 类型定义 / 数据模型 / 路由与 API 设计）、`proposal.md`（D1–D16、AC-1…AC-10、R1–R11）、`specs/{desktop-config-agent,desktop-workspace-config,desktop-agent-execution,desktop-archi-agent,desktop-change-chat}/spec.md`
> **性质**: 一条新会话臂 + 一个提案工具 + 一次用户触发的原子写回 + 一份文件面写入器 + 一处面板与卡片装配。被测范围取 design「新增文件」（7 项）与「修改文件」（20 行，含一对生成物 `bindings.ts` / `bindings.zod.ts`）两子表，删除文件零涉及（本变更无删除项）。
> **验收面口径**（与 proposal / design 一致）：「检测出什么」**不可断言**——MUST NOT 以「该仓库应被识别为 N 个项目 / 依赖是 A ← B」作验收条；验收只锚四类事实：注入臂装配、`propose_config` 契约矩阵、写面（门 + 失败零写入 + 写入器 P1–P4）、卡片五态。

## 测试框架与布局

| 域 | 框架 / 断言 | 测试文件形态 | 运行入口 |
|---|---|---|---|
| Rust（`src-tauri/crates/core/**`、`crates/infra/**`） | 标准 `#[test]` / `#[tokio::test]` + `assert!` / `assert_eq!`；无外部断言库 | 同目录兄弟文件 `<模块>_test.rs`，经 `#[cfg(test)] mod <模块>_test;` 挂载（`crates/core/config/src/lib.rs:990`、`crates/infra/agent/src/lib.rs:282-288`、`crates/infra/agent/src/sdk/mod.rs` 尾段） | `cargo nextest run --workspace`（桌面端相位执行，本文档不写命令细节） |
| Rust（`src-tauri/src/**` shell 层） | 同上 + `tauri::test::mock_app()`（需 State 的命令）+ `#[tauri::command]` 原函数直调（不启 runtime） | 同目录 `<模块>_test.rs`（`src/commands/config/mod_test.rs`、`src/commands/archi/mod_test.rs`、`src/commands/exec/mod_test.rs`）与 `src-tauri/src/bindings/mod_test.rs`（生成物文本面） | 同上 |
| 前端（`packages/desktop/src/**`） | `vite-plus/test`（`describe` / `it` / `expect`）+ `@testing-library/react`，jsdom 环境（`vite.config.ts` `test.environment`，`src/test-setup.ts` 全局 setup） | 与被测模块同目录的 `<模块>.test.ts(x)`（`include: ['src/**/*.test.ts?(x)']`） | `pnpm -C packages/desktop run test` |

扩展名与断言风格沿既有文件：Rust 侧测试名用中文短语、断言直比产出（诊断集合 / 名字清单 / 键集 / 键名大小写 / 字节数 / mtime）；前端侧用 `screen.getByTestId` / `getByText` + `toEqual` 整组比对，进程边界一律 mock `@tauri-apps/api/core`（`invoke` 按命令名分派 + 可编程 `Channel`），内部模块真实组合。本变更新增 6 个测试文件（`config_knowledge_test.rs` / `config_propose_test.rs` / `commands/atomic_test.rs` / `use-config-chat.test.ts` / `config-chat-panel.test.tsx` / `config-apply-scope.test.tsx`）并扩容 14 个既有测试文件，零新增测试专用导出（AGENT.md「no test-only exports」）。

## 验收范围

| AC ID | 验收条件 | 被测文件或模块 |
|---|---|---|
| AC-1 | config 臂装配逐名正确：`read` / `grep` / `glob` / `ls` / `bash` / `propose_config` 六名 + `ask = Generic` + `finish = None` + `preamble` 非空；**无** `write` / `edit` / db 工具；`RunStarted.tools` 与实际下发面一致；未知名装配层 `Err` | `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/mod.rs`（`config_propose` 模块登记）、`packages/desktop/src-tauri/crates/infra/agent/src/lib.rs`（`config_knowledge` 模块登记） |
| AC-2 | `propose_config` 三面行为：只读模式返回盘面（`candidate: null`、`changes: []`）；脏候选 `Err` 逐条诊断且**零副作用**；干净候选 `Ok` 且 `current` / `candidate` 均为 file face（无 `staticAnalysis` / `writeProtection` / `dependsOn` / `extra:[{key,value}]` 形态）；满配 fixture 输出 < 50 000 B | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/config_propose.rs`、`packages/desktop/src-tauri/crates/core/config/src/lib.rs`（`validate` / `load_file_face` / `to_file_value` 单源）、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools.rs`（无路径白名单与 `cap_result` 面） |
| AC-3 | `ConfigContext.root` 取主仓 root：以「`payload.workspace_root` ≠ `ctx.workspace_root`」的构造断言取前者；铸造判据为名字清单含 `propose_config`；上下文缺席以 `is_error` 回灌且 run 不中断 | `packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs` |
| AC-4 | 知识 preamble 零入参、零 IO、词表派生（八键 / 八值）、零占位符、体量有上限、拼装失败只丢 `preamble` 位 | `packages/desktop/src-tauri/crates/infra/agent/src/config_knowledge.rs`、`packages/desktop/src-tauri/crates/infra/agent/src/config-knowledge.md` |
| AC-5 | 写面门：`workspace_config_apply` 在任一 `InvalidValue` 时 `Err` 且**文件零变化**（mtime + 内容双证）；成功路径经 `to_file_value` → pretty + 尾换行 → `write_atomic`，返回重读报告；入参为 file-face `Value`（typed `WorkspaceConfig` 不得出现在签名上）；`write_atomic` 为命令层唯一落盘点 | `packages/desktop/src-tauri/src/commands/config/mod.rs`、`packages/desktop/src-tauri/src/commands/atomic.rs`、`packages/desktop/src-tauri/src/commands/archi/mod.rs`（搬移后既有写面回归）、`packages/desktop/src-tauri/crates/core/config/src/lib.rs`（`plan_apply_config`） |
| AC-6 | 写入器四条机械锚全绿：P1 键名无大写（`extra` 值内除外）、P2 满配顶层键集 == `known_keys()`、P3 `assemble ∘ to_file_value` 幂等于线面值、P4 全字段字面量往返逐字相等；整数化与 `extra` 三坑各有断言 | `packages/desktop/src-tauri/crates/core/config/src/lib.rs` |
| AC-7 | 卡片五态齐备（进行中 / 失败 / 可应用 / `changes == []` 禁用 +「与当前配置一致」/ 只读模式 / 解析失败原文降级）；diff 基准取 payload `current`；多次 propose 各自独立可应用、无失效标记；应用成功后页面重取一次；面板骨架复用（`config-chat-*`）、归属三件（`source = "config"` / `sourceRef = null` / `cwd = null`）、禁发矩阵单源、收起零取数 | `packages/desktop/src/views/config/config-view.tsx`、`packages/desktop/src/views/config/components/config-chat-panel.tsx`、`packages/desktop/src/views/config/hooks/use-config-chat.ts`、`packages/desktop/src/components/agent/message-cards/tool-cards.tsx`、`packages/desktop/src/components/agent/message-cards/config-apply-scope.tsx`、`packages/desktop/src/components/agent/index.tsx`（barrel 第 8 名）、`packages/desktop/src-tauri/src/commands/exec/mod.rs`（`list_config_chat_sessions` 薄命令） |
| AC-8 | spec 改判零残留冲突：`desktop-workspace-config` 不再有「只读 / 无写路径」活条款；`desktop-agent-execution` 三臂表述全部转四臂；`desktop-archi-agent` 兜底枚举已收窄；`desktop-change-chat` 骨架消费方为三处 | 五份 delta spec 文本（无代码落点，见「不可测试项」） |
| AC-9 | 全管线绿：`check` 链（`server:check` → `bindings:export` → `api:check` → `client:check`）与全量测试通过；生成物零手改；`package.json` version 为 `0.7.3`、`src-tauri/Cargo.toml` 不随动；两新命令在册 | `packages/desktop/src/types/generated/bindings.ts`、`packages/desktop/src/types/generated/bindings.zod.ts`、`packages/desktop/src-tauri/src/commands/mod.rs`（`all_commands!` 登记面）、`packages/desktop/package.json`（见「不可测试项」） |
| AC-10 | 架构模型同步：`06-frontend.toml` 含 `Views.Config → Components.Agent` 一条新边且能在 `views/config/**` 找到 `import` 证据；模型可解析（零结构诊断）；表头注释与 ADR §9 与模型本体同值 | `.dev-team/architecture/graph/model/06-frontend.toml`、`.dev-team/architecture/decisions/0001-model-granularity.md`、`.dev-team/architecture/models/Frontend/Components/App.Frontend.Components.Agent.md`（见「不可测试项」） |

> **对位说明**：上表只列 AC 与承载模块；每个模块的测试文件路径、覆盖方式与 Mock 策略见下方对应 `###` 节（节标题即「源文件 -> 测试文件」映射对）。同一源文件挂两个测试文件时（`mount.rs` → `mount_test.rs` / `policy_test.rs`，生成物 → `bindings/mod_test.rs`）各自成节，判据互不重叠。
>
> **随动与非新增用例（编译 / 装置闭合面，断言判据零改）**：`crates/infra/agent/src/sdk/mount_test.rs` 的 `EXPECTED_NAMES` 13 → 14、`registry` 计数与 `with_capacity` 锚、`policy_test.rs` 三档等价表把 `propose_config` 归 `[true, true, true]`、`compose_test.rs` 两处结构断言（判据零改、config 臂纳入遍历）、`src/commands/archi/mod_test.rs` 的「`fs::write` 恰一处」落点由 `archi/mod.rs` 改指 `commands/atomic.rs`、`bindings/mod_test.rs` 两清单计数 54 → 56、`config-view.test.tsx` 因分栏（`ResizablePanelGroup`）需装置包裹。上述各项均属**同一判据的期望随动**，验收语义不变。

## 单元测试

### packages/desktop/src-tauri/crates/core/config/src/lib.rs -> packages/desktop/src-tauri/crates/core/config/src/lib_test.rs

#### 待测功能

- `known_keys()`: `KNOWN_KEYS` 八键的只读出口（声明序即输出序），供知识派生段与 P2 锚共用
- `TestFramework::ALL`: 八值枚举清单（声明序 = wire 序），`&'static` 零堆分配
- `TestFramework::as_str()`: 值 → 文件面字面量（`"jest"` … `"pytest"`），与 `parse_framework` 双向互证
- `validate()`: 任意 JSON 值 → 同一套 `assemble` 校验的 `ConfigReport`；顶层非对象落 `InvalidValue($)` + 全默认（与 `load` 的非对象分支同口径）
- `load_file_face()`: 盘面 file face 投影（`to_file_value(assemble(盘上值).config, 盘上值)`）+ 同一次装配的诊断；文件缺失 / 不可读 / 语法错 / 顶层非对象 → `(json!({}), 该条诊断)`；`extra` 键冲突（不可达）→ `Err`
- `to_file_value()`: 模板驱动的 file face 投影（`to_value` → 三次递归重命名 → 整数化 → `extra` 展开 → 模板裁剪，DD10 / DD11）；`extra` 键冲突 → `Err`（附冲突键）
- `plan_apply_config()`: 纯函数写面规划——`validate` 零 `InvalidValue` 门 → `to_file_value` → `to_string_pretty` + 尾换行 → `ConfigWrite { file, content }`；零 IO

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `known_keys()` | 正向 | 返回恰 8 项，且逐项等于 `["$schema", "schema", "context", "rules", "static_analysis", "projects", "tests", "write_protection"]`（声明序） | 单点 |
| `known_keys()` | 正向 | 返回值键集与 `config.schema.json` 的 `properties` 键集相等（P2 锚对镜像既有断言的一致性继承；镜像经 `include_str!` 读入，零运行时 IO） | 单点 |
| `known_keys()` | 边界 | 返回切片形态为 `&'static [&'static str]`：跨两次调用指针同一（`OnceLock` / 常量面无二次构造） | 单点 |
| `known_keys()` | 异常 | 内存副本从镜像 `properties` 删一键（`tests`）→ 比对断言失败且文案指认缺键（不改仓内文件） | 单点 |
| `TestFramework::ALL` | 正向 | 八值逐值 `as_str()` 与 `serde_json::to_value(&value)` 的 wire 串逐字相同（`#[serde(rename)]` 与 `as_str` 双向对拍） | 矩阵迭代（8 值） |
| `TestFramework::as_str()` | 正向 | 八值双向互证：`parse_framework(v.as_str()) == Some(v)` | 矩阵迭代（8 值） |
| `TestFramework::ALL` | 边界 | 长度恰 8、无重复、声明序与 `as_str` 序一致；`ALL` 为 `&'static`（零 clone 访问） | 单点 |
| `TestFramework::as_str()` | 异常 | 枚举外字面量 `parse_framework` 全 miss：`""` / `"Jest"` / `"vite_plus"`（下划线）/ `"node_test"` / `" rust"` / `"rust "` / `"🙂"` / 1200 字符长串 → 均 `None`（大小写、分隔符、空白与 emoji 都不归一） | 矩阵迭代（8 形态） |
| `validate()` | 正向 | 满配合法 object → `diagnostics` 为空、`config` 字段逐项与期望相等 | 单点 |
| `validate()` | 正向 | 与 `load` 同源：同一份字节经 `validate(from_str)` 与经真实落盘 `load(root)` 的 `(config, diagnostics)` 线面值逐字段相等（两入口零分叉） | 单点 |
| `validate()` | 异常 | 顶层非对象四形态（`null` / `[]` / `"str"` / `1`）→ 恰一条 `InvalidValue($)` + 全默认，**不产生** `defaultApplied` 噪声（与 `load` 非对象分支同口径） | 矩阵迭代（4 形态） |
| `validate()` | 异常 | 空对象 `{}`（缺全部字段）→ 吃默认并逐条 `defaultApplied`，诊断集合与「空文件 `load`」形态一致（不是 `fileMissing`） | 单点 |
| `validate()` | 边界 | 多余顶层字段（`{"custom": 1}`）与已知键混杂 → 未知键进 `extra`、零诊断；`extra` 中嵌套数组 / 对象 / `null` 原样保留 | 矩阵迭代（4 形态） |
| `validate()` | 边界 | 数值边界：`coverage.lines` 取 `0` / `-1` / `101` / `null` → 合法值原样、越界与缺省吃默认 + `InvalidValue`；`90.0` 与 `-0.0` 走浮点判定（JSON 字面无法表达 NaN / Inf，`serde_json` 解析期即拒，故不以 NaN / Inf 作 fixture） | 矩阵迭代（6 形态） |
| `load_file_face()` | 正向 | 盘上满配（三处多词键 + 一条 `extra` 条目）→ 返回值键形全 snake_case（`static_analysis` / `write_protection` / `projects[0].depends_on`）、`extra` 条目展开在顶层、零 camelCase 残留；`diagnostics` 为空 | 单点 |
| `load_file_face()` | 正向 | 与 `propose_config` 的 `current` 公式同源：`load_file_face` 的输出 == `to_file_value(assemble(盘上值).config, 盘上值)`（逐值相等；MUST NOT 等于 `to_value(&WorkspaceConfig)`） | 单点 |
| `load_file_face()` | 边界 | 文件缺失 → `Ok(({}, [FileMissing]))`（不 `Err`，与 `load` 分层口径一致） | 单点 |
| `load_file_face()` | 边界 | 空对象 `{}` → `({}, defaultApplied 若干)`，file face 为空对象（不写默认键，模板裁剪的作用面） | 单点 |
| `load_file_face()` | 异常 | 坏 JSON → `Ok(({}, [JsonInvalid]))`；顶层非对象四形态 → `Ok(({}, [InvalidValue($)]))` 恰一条 | 矩阵迭代（5 形态） |
| `load_file_face()` | 异常 | 配置文件路径被同名目录占据（read 失败且非 `NotFound`）→ `Ok(({}, [ReadFailed]))` | 单点 |
| `load_file_face()` | 异常 | `extra` 条目键与结构体字段同名（盘面 `{"extra": [{"key": "schema", "value": 1}]}`）→ `Err` 且文案含冲突键（不可达分支的显式失败面） | 单点 |
| `to_file_value()` | 正向 | **P1**：满配 fixture 输出中 `extra` 值内部以外任何键名零大写字母 | 单点 |
| `to_file_value()` | 正向 | **P2**：满配 fixture 顶层键集 == `known_keys()` 且不含 `extra` 键 | 单点 |
| `to_file_value()` | 正向 | **P3**：`assemble(to_file_value(cfg, 模板))` 与 `cfg` 逐字段相等（线面值比较，`WorkspaceConfig` 无 `PartialEq`） | 单点 |
| `to_file_value()` | 正向 | **P4**：对「全字段显式」字面量 `L`（字母序、数值写整数）`to_file_value(assemble(L).config, L) == L` | 单点 |
| `to_file_value()` | 正向 | 递归模板裁剪：候选仅 `{schema, tests: [{root, framework}]}` → 顶层只两键、suite 只两键（不写默认字段——「模板驱动」不是只裁顶层） | 单点 |
| `to_file_value()` | 正向 | 三处重命名递归全层级：`projects[i].dependsOn` → `depends_on`、`staticAnalysis` → `static_analysis`、`writeProtection` → `write_protection` | 单点 |
| `to_file_value()` | 边界 | 模板非对象四形态：`{}` → 输出 `{}`；`null` / 数组 / 字符串 → 取 normalized 侧整值（`trim` 的「其余」分支） | 矩阵迭代（4 形态） |
| `to_file_value()` | 边界 | 数组按下标配对：模板短于值（`tests` 一条 vs 值两条）→ 越界项整值保留；模板长于值 → 只输出值侧项 | 矩阵迭代（2 形态） |
| `to_file_value()` | 边界 | 整数化四形态：`90` → `90`（`as_i64()` 命中，非 `90.0`）；`90.5` 保浮点；`0.0` / `-0.0` → `0`（`fract() == 0.0`）；`2^53 + 1` 保浮点（超界不入 `i64`） | 矩阵迭代（5 形态） |
| `to_file_value()` | 边界 | `extra` 三坑：① null 值条目保留（重读条目数不减）② 键序字母序重排（接受，不作保真断言）③ `extra` 值不被二次整数化（`1.0` 仍为 `1.0`） | 矩阵迭代（3 形态） |
| `to_file_value()` | 异常 | `extra` 条目键与结构体派生字段同名（组装期已过滤已知键，故以 `extra` 中 `"schema"` 直接构造）→ `Err` 且文案含冲突键；零静默覆盖 | 单点 |
| `to_file_value()` | 异常 | `extra` 条目值为超深嵌套（1000 层数组）→ 递归下不 panic（保真优先，不做深度裁剪） | 单点 |
| `plan_apply_config()` | 正向 | 合法候选 → `ConfigWrite { file: layout::config_path(root), content }`；`content` 可 `from_str` 回 `Value` 且等于 `to_file_value(&report.config, candidate)`；尾字符为 `"\n"`、无紧凑分隔符（pretty） | 单点 |
| `plan_apply_config()` | 正向 | 零 IO：调用前后目标路径「不存在 / 存在且内容与 mtime 不变」（两种起点各一次） | 矩阵迭代（2 形态） |
| `plan_apply_config()` | 异常 | 任一 `InvalidValue`（覆盖阈值越界 / 枚举外 `framework` / 含 `*` 的 `tests[].root`）→ `Err` 且文案逐条含 `path`；**不产出** `ConfigWrite`（失败零写入在纯函数面即成立） | 矩阵迭代（3 形态） |
| `plan_apply_config()` | 边界 | 仅 `defaultApplied` 诊断（候选 `{}`）→ `Ok`（defaultApplied 不拦截写回），`content` 为 `{}` pretty 形态 + 尾换行 | 单点 |
| `plan_apply_config()` | 边界 | 顶层非对象候选（`null` / `[]`）→ `Err`（`InvalidValue($)`）；候选含未知顶层键 → `Ok` 且该键在 `content` 顶层（`extra` 展开） | 矩阵迭代（3 形态） |
| `plan_apply_config()` | 边界 | root 为空路径 / 深层不存在路径 → `file` 仍按 `layout::config_path` 派生（本函数零 root 校验，root 有效性归命令层；crate 内零 `config.json` 字面量） | 矩阵迭代（2 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| —（文件系统） | 不 mock：沿用既有 `TempWs`（真实 tempdir + RAII 清理 + 真实 `config.json` 落盘），文件边界即真实进程边界 | `load_file_face` / `plan_apply_config` 全簇（含「零写入」的内容 + mtime 双证） |
| —（`config.schema.json`） | 不 mock：`include_str!` 编译期绑定入仓镜像，零运行时 IO、零 cwd 依赖 | `known_keys` × 镜像键集比对 |
| —（`serde_json`） | 不 mock、不逐项断言库语义：值比较用线面 `Value` 对照（`assert_eq!` 直比），只断言本 crate 自己的判定与投影 | 全部用例 |
| 私有助手（`read_document` / `rename_in_place` / `integerize` / `expand_extra` / `trim`） | 不为测试新增导出：经公共入口（`load_file_face` / `to_file_value` / `plan_apply_config`）行为面观察；异常形态用内存 `Value` 副本驱动 | 全部用例 |

### packages/desktop/src-tauri/crates/infra/agent/src/config_knowledge.rs -> packages/desktop/src-tauri/crates/infra/agent/src/config_knowledge_test.rs

#### 待测功能

- `knowledge()`: 静态骨架（`include_str!`）+ 代码派生词表（`config::known_keys` 八键 / `TestFramework::ALL` 八值）+ 使用指引拼成单条文本；**零入参、零 IO**；拼装失败 → `None`（只丢 `preamble` 位）
- `build(skeleton)`: 拼装单点——骨架 `trim()` 为空 / 含 `<>` 占位符 → `Err`；否则 `skeleton.trim_end()` + 派生段（降级路径可测的唯一入口）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `knowledge()` | 正向 | 返回 `Some` 且文本以 `SKELETON.trim_end()`（同一 `include_str!` 字节）起手，其后接派生段（骨架零改写、零截断） | 单点 |
| `knowledge()` | 正向 | 派生段八键逐键在场（语料取 `config::known_keys()` 实值，不用字面量） | 矩阵迭代（8 键） |
| `knowledge()` | 正向 | 派生段八值逐值在场（`TestFramework::ALL` + `as_str()` 实值） | 矩阵迭代（8 值） |
| `knowledge()` | 正向 | 词表连接形态为单点分隔「 / 」：整段连接串逐字在场（与 `archi_knowledge` 同形） | 单点 |
| `knowledge()` | 正向 | 使用指引三句在场：两模式（不给候选 = 盘面 / 给候选 = 校验回环）、候选必须是**完整文件面**、`is_error` 后「修正后重交完整候选」 | 单点 |
| `knowledge()` | 边界 | 体量上限：`len() < 8_000` 字节（与 `AGENT.md` 合体后不挤压上下文） | 单点 |
| `knowledge()` | 边界 | 零 `<>` 占位符（全文本 `matches("<>")` 为 0） | 单点 |
| `knowledge()` | 边界 | 零盘面真值：文本不含本仓项目根（`packages/desktop`）、不含阈值实际值形态（如 `lines: 80`）、不含写保护 glob 字面量 | 单点 |
| `knowledge()` | 边界 | 零写保护规则复述：不含内置锁定规则的键名表（只保留「`config.json` 是 agent 不可写的文件」事实句） | 单点 |
| `build()` | 异常 | `build("")` / `build("   \n\t ")` → `Err`（骨架空），文案含「骨架为空」语义 | 矩阵迭代（2 形态） |
| `build()` | 异常 | `build("a <> b")` → `Err`，文案含「`<>` 占位符」成因（占位符即「路径 / 词表未派生」形态） | 单点 |
| `build()` | 边界 | 超长骨架（100_000 字符）→ `Ok` 且派生段仍追加在尾部（无截断实现、无 panic） | 单点 |
| `build()` | 边界 | 特殊字符骨架（emoji / `\n` / `\0` / 引号）→ 原样保留在输出中（零转义加工） | 矩阵迭代（4 形态） |
| `build()` | 异常 | 坏骨架的降级传导：以 `build` 的 `Err` 面 + 装配侧 `preamble == None`（`compose_test` 用例）双证「拼装失败只丢 preamble 位、发起点不 `Err`」 | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| —（无 mock） | 词表取 `config::known_keys()` / `TestFramework::ALL` 真实函数；骨架取 `include_str!("config-knowledge.md")` 编译期同一份字节（禁测试内复制副本，替身会抹掉「词表漂移 / 骨架写坏」的判据） | 全部用例 |
| 坏骨架形态 | 不注入生产常量：以 `build(内存字符串)` 驱动异常 / 边界（生产 `knowledge()` 恒喂仓内骨架，零 IO） | `build` 四类用例 |

### packages/desktop/src-tauri/crates/infra/agent/src/config-knowledge.md -> packages/desktop/src-tauri/crates/infra/agent/src/config_knowledge_test.rs

#### 待测功能

- `knowledge()`: 输出文本的起手段即本文件字节（`SKELETON.trim_end()` 逐字保留、派生段接其后）——静态骨架作为资产的可观察入口，测试面见上一节
- `build(skeleton)`: 骨架合法性单点（`trim()` 为空 / 含 `<>` 占位符 → `Err`）；本文件作为生产骨架恒经 `include_str!` 喂入，异常形态由内存副本驱动

> **注记**：design.md 未声明该文件的公共 API 变更（「公共函数 / API」子表无本文件行，静态骨架 md 是资产、非代码模块）；上述两条取的是本文件被消费处的既有内部入口名，用于定位测试面，不构成新增公共面。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 骨架八键语义段 | 正向 | 八键名逐键在骨架内以「字段名 + 语义」形态出现（键名以 `known_keys()` 遍历断言，骨架侧零手抄） | 矩阵迭代（8 键） |
| 骨架使用指引 | 正向 | `propose_config` 两模式与「完整候选」指引在骨架内（与派生段不重复持有：派生段只放词表） | 单点 |
| 骨架 `defaultApplied` 口径 | 正向 | 「`defaultApplied` 是正常形态 / 清零噪音 = 给有默认值的字段写真值，不是给 Option 写 `null`」表述在案 | 单点 |
| 骨架软规则 | 正向 | `projects[].language`（开放字符串、归一只在消费侧）与 `depends_on`（平级边 / 安装顺序、非层级来源）语义在案；`context` / `rules` 零工程消费在案 | 单点 |
| 骨架写权事实 | 边界 | 仅事实陈述「`config.json` 是 agent 不可写的文件」；零内置锁定规则的键名 / glob 复述（不构成第二真相源） | 单点 |
| 骨架自证 | 边界 | 骨架内零 `<>` 占位符、整体体量 < 8_000 字节（与 `knowledge()` 上限同源断言） | 单点 |
| 骨架自证 | 异常 | 内存副本注入 `<>`（如 `"路径：<>"`）→ `build(副本)` `Err`（负向判据由内存副本驱动，不改仓内文件） | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| —（骨架字节） | 不 mock：`include_str!("config-knowledge.md")` 编译期同一份文件；异常形态只在内存字符串副本上就地改写 | 全部用例 |

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/config_propose.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/config_propose_test.rs

#### 待测功能

- `PROPOSE_CONFIG_TOOL`: 工具名常量 `"propose_config"`（装配 / `mount` 再导出 / `input_path` 白名单三处同一常量）
- `definition()`: `{ candidate?: object }` + `required: []` 的参数面；描述逐句写明两模式、「候选是整份文件面（`static_analysis` / `write_protection` / `depends_on` 用 snake_case）」、并点明**无路径参数**
- `ConfigContext::new()`: per-turn 铸造产物构造（root 原样持有）
- `ConfigContext::execute()`: 两模式执行体——只读返回盘面 / 候选校验回环；输出恒单条 file-face JSON 字符串（过 `cap_result`）；`changes` / `summary` 后端算；零落盘、零落库、零会话状态改动

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `PROPOSE_CONFIG_TOOL` | 正向 | 常量值恰 `"propose_config"`、与 `definition().name` 逐字相同、且 `mount::lookup(常量)` 命中（单源三面） | 单点 |
| `PROPOSE_CONFIG_TOOL` | 边界 | 常量在 `mount::registry()` 末项（第 14 项）出现恰一次（名字清单无重复登记） | 单点 |
| `definition()` | 正向 | 参数面 `properties.candidate.type == "object"`、`required == []`、顶层无 `path` / `root` 一类路径属性 | 单点 |
| `definition()` | 正向 | 描述含两模式语义（「不给候选 = 返回当前盘面」/「给候选 = 校验回环」）与「修正后重交完整候选」指引 | 单点 |
| `definition()` | 边界 | 描述点明「无路径参数，盘面由服务端按仓根解析」与三处 snake_case 键形（防模型自造 `path` / 用 camelCase） | 单点 |
| `definition()` | 异常 | 描述不含可选片段候选的措辞（不得暗示「可只传片段」）——以文本锚断言无「片段 / partial」类词 | 单点 |
| `ConfigContext::new()` | 正向 | 构造后 `execute` 读到的盘面来自该 root（落盘 fixture 在 A 根，B 根为另一份配置 → 断言读到 A） | 单点 |
| `ConfigContext::new()` | 边界 | root 为不存在路径 / 空路径 → 构造不 panic，`execute` 返回盘面 `{}` + `FileMissing` 摘要 | 矩阵迭代（2 形态） |
| `ConfigContext::execute()`（只读模式） | 正向 | 入参 `{}` → `Ok`；JSON 解析后顶层恰四键（`current` / `candidate` / `changes` / `summary`，无第五键）；`candidate == null`、`changes == []`、`current` 为 file face（含 `static_analysis`、无 `staticAnalysis`） | 单点 |
| `ConfigContext::execute()`（只读模式） | 正向 | `current` 与盘上原文同内容（逐值等于 `load_file_face` 输出；`extra` 条目在顶层展开、`projects[0].depends_on` 键形在案） | 单点 |
| `ConfigContext::execute()`（只读模式） | 边界 | `{"candidate": null}` 与 `{}` 等价（null 视同缺省 → 只读模式）；`{"candidate": {}}` 走校验模式（空对象 = 合法候选，非缺省） | 矩阵迭代（2 形态） |
| `ConfigContext::execute()`（只读模式） | 边界 | 诊断折进 `summary`：含 `FileMissing` 时 `summary` 携「诊断 1 条」类摘要；无诊断时呈「无诊断」形态（诊断不新增键，D3 四键不变） | 矩阵迭代（2 形态） |
| `ConfigContext::execute()`（校验模式） | 正向 | 干净 file-face 候选（含 `projects[0].language` / `depends_on` 与 `write_protection.files`）→ `Ok`；`candidate` 为 file face（无 camelCase 多词键、`extra` 展开在顶层）、`changes` 逐项形态齐、`summary` 为一句中文 | 单点 |
| `ConfigContext::execute()`（校验模式） | 正向 | `changes` 三 `kind` 覆盖：新增顶层未知键 → `added`（`before` 为 null）；候选删去盘面某键（`context`）→ `removed`（`after` 为 null）；改顶层标量（`schema`）→ `changed` | 矩阵迭代（3 形态） |
| `ConfigContext::execute()`（校验模式） | 正向 | 数组作叶子：`projects[0].language` 变化 → **一条** `changed` path `projects`（不逐下标）；`tests` 同上 | 矩阵迭代（2 形态） |
| `ConfigContext::execute()`（校验模式） | 边界 | `changes` 序 = 字母序（`serde_json::Map` 直接序，零额外排序）；`path` 为点分 file-face 键路径（顶层即键名，无 `$` 前缀） | 单点 |
| `ConfigContext::execute()`（校验模式） | 边界 | 候选 == 盘面 → `changes == []` 且 `summary` 为「候选合法，与当前配置一致」 | 单点 |
| `ConfigContext::execute()`（校验模式） | 边界 | 候选用 wire 面键名：`projects[i].dependsOn`（元素级未知子键）在输出中被丢弃、`staticAnalysis`（顶层未知键）落 `extra` 并在顶层展开原文键名——两形态都不产生第二个 snake_case 字段 | 矩阵迭代（2 形态） |
| `ConfigContext::execute()`（校验模式） | 异常 | 脏候选（枚举外 `framework` + 越界覆盖阈值 + 含 `*` 的 `tests[].root`）→ `Err`；文案逐条 `path: message`，尾行含「修正后重交完整候选（一次全量，不接受片段）」；零落盘 | 单点 |
| `ConfigContext::execute()`（校验模式） | 异常 | 候选非对象三形态（`[]` / `"s"` / `1`）→ `Err`（`InvalidValue($)` 逐条），零 panic | 矩阵迭代（3 形态） |
| `ConfigContext::execute()` | 边界 | 满配 fixture（八键全字段 + 多 suite + 多项目 + 多写保护规则）两模式输出均 `< 50_000` 字节且为可解析 JSON（无「已截断」留痕） | 矩阵迭代（2 模式） |
| `ConfigContext::execute()` | 边界 | 超大候选（500 项目 × 20 suite）→ 输出经 `tools::cap_result` 截断留痕（尾部含「已截断」文案）、不 panic（截断形态可辨识，MUST NOT 冒充合法 JSON） | 单点 |
| `ConfigContext::execute()` | 边界 | 入参非对象四形态（`[]` / `"str"` / `1` / `null`）→ 只读模式（不 panic、四键输出） | 矩阵迭代（4 形态） |
| `ConfigContext::execute()` | 异常 | 零副作用双证：两模式各调用一次前后，`config.json` 字节 + mtime 逐字不变；记录目录条目零新增（无临时文件残留） | 矩阵迭代（2 模式） |
| `ConfigContext::execute()` | 边界 | 候选含顶层未知键 `path` → 输出中该键原文保留、内容零改写（与 `tools_test` 的 `input_path` 用例互为双证） | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| —（盘面读取） | 不 mock：真实 tempdir + 真实 `config::load_file_face`（唯一读入口）；MUST NOT 在测试里复制 file face 投影逻辑 | 全部用例（含零副作用双证） |
| —（校验 / 归一化） | 不 mock：真实 `config::validate` / `config::to_file_value`（单源即被测契约的一部分；替身会让「wire face 出线」回潮不被发现） | 校验模式全簇 |
| —（`tools::cap_result`） | 不 mock：真实 50 KB 上限与留痕文案即被测契约（禁第二份上限实现） | 满配 / 超大候选两档 |
| 时钟（mtime） | 不 mock：读真实 `fs::metadata`（零副作用双证的基线） | 零副作用用例 |

### packages/desktop/src-tauri/crates/infra/agent/src/compose.rs -> packages/desktop/src-tauri/crates/infra/agent/src/compose_test.rs

#### 待测功能

- `assemble_injections()`: 模块私有纯函数（签名不变），新增第四臂——`provenance.source == CONFIG_SOURCE` → 六名工具 / `ask = Some(Generic)` / `finish = None` / `preamble` 取 `config_knowledge::knowledge()`（DD1）
- `injected()`: 模块私有装配助手（成员校验 + 名字清单落地），config 臂复用同一实现、签名与判据零改
- `CONFIG_SOURCE`: 新增来源常量 `"config"`（与命令层 `CONFIG_CHAT_SOURCE`、前端 `use-config-chat` 同字面量，三处注释互指）

> **注记**：design.md 未声明该文件的公共 API 变更（「公共函数 / API」子表无本文件行）；上述三条均为既有模块私有入口 / 本文件新增私有常量，测试经它们观察 DD1 的第四臂，不构成公共面。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `assemble_injections` 的 config 臂 | 正向 | `source = "config"` → `tools` 恰 6 名且名字与序为 `["read", "grep", "glob", "ls", "bash", "propose_config"]` | 单点 |
| `assemble_injections` 的 config 臂 | 正向 | 六名内零 `write` / `edit` / `db_tables` / `db_query`；`ask == Some(Generic)`；`finish == None`；`preamble.is_some()` 且与 `config_knowledge::knowledge()` 逐字相等（装配面零二次改写） | 单点 |
| `assemble_injections` 的 config 臂 | 边界 | `source_ref` 任意形态（`None` / `""` / 四段 change 形态）→ 该臂逐字段不变（config 臂不消费 `source_ref`） | 矩阵迭代（3 形态） |
| `assemble_injections` 的 config 臂 | 边界 | 近似来源名（`"Config"` / `"config "` / `"configx"` / `"config-chat"`）→ 落兜底臂（七内建 + 两 DB 名，无 `propose_config`）：判据是精确相等，不是前缀 / 大小写嗅探 | 矩阵迭代（4 形态） |
| `assemble_injections` 的 config 臂 | 异常 | 装配面成员校验仍单源：以含非法名的名字清单驱动 → `Err` 前缀「注入面非法工具名」、文案携非法名（判据零改，config 臂复用既有 `injected`） | 单点 |
| `CONFIG_SOURCE` | 正向 | 源码结构锚（`include_str!("compose.rs")`）：`const CONFIG_SOURCE: &str = "config"` 恰一处；同字面量与命令层 `CONFIG_CHAT_SOURCE`、前端 `use-config-chat` 的 `source` 三处逐字相同 | 单点 |
| `assemble_injections` 的结构锚 | 边界 | `include_str!("compose.rs")`：`fn assemble_injections` 恰一处、函数体内零 `SessionInjections::default()` 命中（逐臂显式置位） | 矩阵迭代（2 断言） |
| `assemble_injections` 的结构锚 | 正向 | 「逐来源下发面与装配单源一致」断言的四来源遍历纳入 `config`（`RunStarted.tools` 上报面 == `assemble_injections` 产出面；判据零改、只多一臂） | 单点 |
| `assemble_injections` 的 config 臂 | 异常 | `preamble` 降级传导：以 `config_knowledge::build("<>")` 的 `Err` 面（`config_knowledge_test`）＋本臂「`knowledge()` 为 `Some` 时逐字相等」构成双向判据——装配层不消费 `build` 的 `Err`，只取 `Option` | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| —（无 mock） | `assemble_injections` 为进程内纯函数；既有 `injections_of(source, source_ref)` 装置直读；`preamble` 取真实 `config_knowledge::knowledge()`（替身会掩盖「词表漂移 / 骨架写坏」在装配面的传导） | 全部用例 |
| 结构断言语料 | 不 mock：`include_str!("compose.rs")` 编译期绑定源文件（既有两处结构断言同形） | 结构锚两行 |

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount_test.rs

#### 待测功能

- `registry()`: 挂载清单数据面（`pub(crate)`），第 14 项 `propose_config`（`ToolTier::Read` / `ToolExec::Config`）与 `with_capacity(14)` 随动
- `definitions()`: `tools: Some(...)` 面解析第 14 名，`tools: None` 仍只取七内建（`ask` / `finish` 两条追加分支零改）
- `lookup()`: 精确名命中第 14 项且定义与 `config_propose::definition()` 同源
- `TOOL_NAMES`: 七件逐字不变（`tools: None` 面契约零位移）

> **注记**：design.md 未声明该文件的公共 API 变更（「公共函数 / API」子表无本文件行；挂载清单是 `pub(crate)` 数据面，`PROPOSE_CONFIG_TOOL` 的「所在文件」记为 `sdk/config_propose.rs`）；上述条目取既有 `pub(crate)` 入口名，用于定位第 14 项挂载、`with_capacity(14)` 与 `ToolExec::Config` 变体的测试面。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `registry()` | 正向 | 恰 14 项、名字逐值有序（`EXPECTED_NAMES` 末项 `propose_config`）、零重复名；前七项仍恰七内建 | 单点 |
| `registry()` 第 14 项 | 正向 | `tier == ToolTier::Read`、`exec == ToolExec::Config`、`definition.name` 与 `config_propose::definition()` 同名同参数面（定义单源在 `config_propose`） | 单点 |
| `registry()` | 边界 | 源码结构锚（`include_str!("mount.rs")`）：`Vec::with_capacity(14)` 恰一处、两段 doc 的「十四项」在场、`build_registry` 构造项数 = 14 | 单点 |
| `registry()` | 边界 | 构造函数项序稳定：`registry()` 两次调用字符串序相同（`OnceLock` 单次构建） | 单点 |
| `definitions()` | 正向 | `tools: Some(config 六名)` → 恰 6 项，逐项 tier / exec 对应（`read`…`bash` 为 `Pure`/Read-Execute 既有档，`propose_config` 为 Read/Config），序与清单序一致 | 单点 |
| `definitions()` | 边界 | `tools: None` → 仍只取前七内建（`propose_config` 不进 `tools: None` 语义）；`ask = Generic` 时末尾追加 `AskUserQuestion`、`finish = None` 不追加 `finish_step` | 单点 |
| `lookup()` | 正向 | `lookup("propose_config")` 命中且定义名逐字相同 | 单点 |
| `lookup()` | 异常 | 近似名 `"propose-config"` / `"ProposeConfig"` / `"propose_config "` / `" propose_config"` → 全 `None`（精确名判据） | 矩阵迭代（4 形态） |
| `definitions()` | 异常 | 清单含未知名 → `Err` 文案前缀「注入面非法工具名（挂载清单外）」逐字不变（第 14 项不改变该判据） | 单点 |
| `TOOL_NAMES` | 边界 | 仍七件且逐字不变（`tools: None` 面契约零位移） | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| —（无 mock） | `registry()` / `lookup` / `definitions` 全为进程内纯函数（`OnceLock` 一次性静态表，无 IO / spawn / 网络）；`prompt` 用内存合成字符串（既有装置） | 全部用例 |
| 定义来源 | 不 mock：第 14 项定义取真实 `config_propose::definition()`（替身会掩盖「名字在 mount、描述在 exec」的分叉） | 第 14 项用例 |

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/policy_test.rs

#### 待测功能

- `allows()`: 准入纯函数（`sdk/policy.rs` 既有公共面），第 14 项在三档下均须放行（`Read` 档 `[true, true, true]`），其余 13 项期望逐字不变
- `tier()`: `policy_test.rs` 内实测助手（从 `registry()` 按名取 tier），用于观察 `mount.rs` 挂载数据的 tier 归属

> **注记**：design.md 未声明该文件的公共 API 变更（「公共函数 / API」子表无本文件行）；本映射是 `mount.rs` 挂载数据在三档准入面上的等价表随动，判据零改，上述两条为既有函数与测试内助手名。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 十四项三档等价表 | 正向 | 遍历 `registry()` 逐工具对拍：`propose_config` 归 `[true, true, true]`（Read 档三档全放），其余 13 项期望逐字不变 | 单点 |
| 十四项三档等价表 | 边界 | 名两两不同 / 条目数恒等于 `registry().len()`（14）；表内 `other` 分支零命中（封闭集） | 单点 |
| 十四项三档等价表 | 异常 | 封闭集外名不落遍历面：把 `propose_config` 之外的未登记名喂入时 `unreachable!` 语义不可达（以注册表长度断言作替代判据，不伪造遍历外名） | 单点 |
| `tier()` 抽样 | 正向 | `tier("propose_config") == ToolTier::Read`（与 `db_*` / `archi_*` 同族归档） | 单点 |
| `allows()` | 正向 | `Read` × 三档恒 `true`（`propose_config` 归档前提）；`Write` / `Execute` 档决策逐字不变（既有 12 格矩阵零随动） | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| —（无 mock） | `allows(mode, tier)` 为纯函数，`registry()` 真实数据；判据是「等价表与实际 tier 对拍」本身 | 全部用例 |

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools_test.rs

#### 待测功能

- `input_path()`: 既有公共面的无路径白名单补 `PROPOSE_CONFIG_TOOL` → `propose_config` 入参恒 `None`（不进路径沙箱改写面）
- `cap_result()`: `propose_config` 输出复用同一 50 000 B 收口与截断留痕（禁第二份上限实现）
- `MAX_RESULT_BYTES`: 上限常量口径不变（`50_000`），本文件不因第 6 名白名单而改值

> **注记**：design.md 未声明该文件的公共 API 变更（「公共函数 / API」子表无本文件行；`input_path` 签名为既有公共面、本次只补白名单）；上述三条为既有函数 / 常量名，用于定位「无路径参数」白名单加 `propose_config` 的测试面。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `input_path("propose_config")` | 正向 | 入参含 `path: "src/a.ts"` 与 `candidate: {...}` → `None`（不进路径沙箱改写面） | 单点 |
| `input_path` 六名统一面 | 正向 | 三 archi + 两 DB + `propose_config` 共六名同一入参全 `None`；基准对照 `read` → `Some(path)`、`glob` → `Some(pattern)` 不变 | 单点 |
| `input_path("propose_config")` | 异常 | 脏入参七形态（`{}` / `null` / `[]` / `"str"` / `1` / `{"path": 1}` / `{"path": ""}`）→ 恒 `None`、零 panic | 矩阵迭代（7 形态） |
| `input_path` 近似名 | 边界 | `"propose_config "` / `"PROPOSE_CONFIG"` / `"propose-config"` / `"propose_config_x"` → 落默认 `path` 分支（精确名判据，非前缀 / 大小写嗅探） | 矩阵迭代（4 形态） |
| 沙箱面 | 正向 | 候选含顶层未知键 `path` 的 `propose_config` 入参经 `sandbox` 面零改写、零拒绝（既有 `sandbox_test` 参数化纳入第 6 名） | 单点 |
| `cap_result` | 边界 | `MAX_RESULT_BYTES == 50_000` 口径不变，`propose_config` 输出复用同一收口（禁第二份上限实现） | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| —（无 mock） | `input_path` 为纯函数；`sandbox` 面取真实 `check` / 改写链（替身会让「候选被当路径改写」不被发现） | 全部用例 |

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner_test.rs

#### 待测功能

- `mint_config_context()`: 判据 = 名字清单含 `PROPOSE_CONFIG_TOOL`（精确名，非枚举比较 / 非来源嗅探）；`root` = `payload.workspace_root`（主仓 root），MUST NOT 取 `SessionCtx.workspace_root`（exec root / worktree）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `mint_config_context()` | 正向 | 名字清单含 `propose_config` + `workspace_root = Some(A)` → `Some(ctx)`；`ctx` 的 root 经 `execute` 读到 A 根盘面 | 单点 |
| `mint_config_context()` | 正向 | **主仓 root 非 exec root**：构造双根（`payload.workspace_root = A`、`ctx.workspace_root = B`，两处各落一份内容不同的 `config.json`）→ 读到的盘面来自 A；把该断言钉在 `mint_config_context(names, Some(A))` 与 `open.ctx.workspace_root = B` 同场构造上（配置页会话 `cwd = null` 时二者同值，此用例提前钉住「将来被赋予 worktree 执行根」的形态） | 单点 |
| `mint_config_context()` | 边界 | 名字清单为空 / 仅七内建 / 仅 archi 名 / 仅 DB 名 → `None`（四形态均不铸） | 矩阵迭代（4 形态） |
| `mint_config_context()` | 边界 | `workspace_root = None` → `None`（即便名字在场；闸在「主仓 root 在场才铸」） | 单点 |
| `mint_config_context()` | 边界 | `workspace_root = Some("")` / `Some(不存在路径)` / `Some(超长路径)` → `Some`（铸造不做有效性校验，root 校验归命令层与读盘分支） | 矩阵迭代（3 形态） |
| `mint_config_context()` | 异常 | 近似名（`"propose_config "` / `"ProposeConfig"`）→ `None`（精确名判据） | 矩阵迭代（2 形态） |
| per-turn 装配点 | 正向 | 以 config 六名注入面 + 双根 payload 驱动一轮 → `LoopTurn.config` 在场且 root 为主仓 root（装配点取 `open.injections.tools` + `payload.workspace_root`，不取 `ctx.workspace_root`） | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 提供者 IO | 沿用既有 scripted provider 桩（`ask_scripted_provider`，进程边界替身） | per-turn 装配点用例 |
| `PumpPayload` / `SessionCtx` | 不 mock：以真实结构构造双根形态（若 mock 掉 payload，root 来源断言就空转） | 主仓 root 用例 |

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop_test.rs

#### 待测功能

- `LoopTurn.config`: 新增 `pub(crate)` 字段（per-turn 铸造随轮携带，缺席时以 `is_error` 回灌的判据来源）
- `ToolExec::Config` 分派臂: `match mount.exec` 内新增分支——`Some(ctx) => ctx.execute(&input)`，`None` → `("专用工具执行上下文未装配", true)`（与 `Archi` / `Db` 同形）

> **注记**：design.md 未声明该文件的公共 API 变更（「公共函数 / API」子表无本文件行；`LoopTurn.config` 为 `pub(crate)` 字段、分派臂是内部 `match` 分支）；上述两条取本文件的字段名与分派臂名，用于定位测试面。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ToolExec::Config` 分派臂 | 正向 | `LoopTurn.config = Some(ctx)` + 模型调 `propose_config({})` → 恰一条 tool_result、`is_error == false`、内容为合法 JSON 四键（`current` / `candidate` / `changes` / `summary`） | 单点 |
| `ToolExec::Config` 分派臂 | 正向 | 校验模式一条：模型调 `propose_config({candidate})` → 结果非 error 且 `candidate` 非 null、`changes` 非空（执行体承接，不被 loop 二次加工） | 单点 |
| `ToolExec::Config` 分派臂 | 边界 | 三档 permission mode 逐档（`Default` / `AcceptEdits` / `BypassPermissions`）→ 零 `permission_denied`、零 `sandbox_denied`、结果非 error（Read 档 + 无路径参数双证） | 矩阵迭代（3 档） |
| `ToolExec::Config` 分派臂 | 边界 | 候选含顶层未知键 `path` → 沙箱不改写、不拒绝（与 `tools_test` / `config_propose_test` 三面同证） | 单点 |
| `ToolExec::Config` 分派臂 | 异常 | `LoopTurn.config = None` + 名单含 `propose_config` → tool_result `is_error == true`、内容逐字「专用工具执行上下文未装配」、同 id 回灌；run 不中断（续轮照常收口） | 单点 |
| `ToolExec::Config` 分派臂 | 异常 | 执行体 `Err`（脏候选）→ `is_error == true`、内容为诊断原文（含「修正后重交完整候选」）；零 `SystemNotice`、run 不中断 | 单点 |
| `ToolExec::Config` 分派臂 | 边界 | 上下文缺席但工具名不在名字清单 → 落成员校验拒绝（既有面），不进 Config 臂 | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 模型侧 | 沿用既有 `FakeModel`（脚本化 tool_call / 文本），进程边界替身 | 全部用例 |
| 盘面 / 上下文 | 不 mock：真实 tempdir root + 真实 `ConfigContext::new`（上下文缺席形态以 `None` 直构） | 全部用例 |

### packages/desktop/src-tauri/src/commands/config/mod.rs -> packages/desktop/src-tauri/src/commands/config/mod_test.rs

#### 待测功能

- `workspace_config_apply()`: `#[tauri::command]` + `#[specta::specta]` 薄包装；入参 `root: String` + `candidate: serde_json::Value`（file face，出线 TS `unknown`）
- `workspace_config_apply_inner()`: 三段实现（config 轨道私有 `check_root` → `config::plan_apply_config` → `commands::atomic::write_atomic` → `config::load` 重读 → 报告字段平移）；失败零写入；成功返回**写后重读**报告

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `workspace_config_apply` / `_inner` | 正向 | 命令与 inner 结果逐字段一致（`root: String` 直调 + `Path` 直调，不启 runtime——既有 `workspace_config` 先例） | 单点 |
| `workspace_config_apply_inner` | 正向 | 合法候选 → `Ok` 报告；文件被整体替换为规范化内容（pretty + 尾换行、键字母序、`static_analysis` / `write_protection` / `depends_on` 三处 snake_case）；文件无临时残留 | 单点 |
| `workspace_config_apply_inner` | 正向 | 写后重读证明：写前盘面空 / 值为默认，候选设 `context` 与 `tests[0].framework` → 返回报告的 `config` 为新值、`diagnostics` 无 `fileMissing`（不是写前快照） | 单点 |
| `workspace_config_apply_inner` | 异常 | 脏候选（越界阈值 / 枚举外 `framework` / 含 `*` 的 `tests[].root`）→ `Err` 且文案逐条含 `path`；文件内容 **与 mtime** 双证不变（零写入） | 矩阵迭代（3 形态） |
| `workspace_config_apply_inner` | 异常 | root 四形态（`""` 空白 / 不存在 / 普通文件 / 超长不存在路径）→ `Err` 不 panic、任何文件零改动（four 文案可辨） | 矩阵迭代（4 形态） |
| `workspace_config_apply_inner` | 边界 | 候选 `{}` → `Ok`（`defaultApplied` 不拦写回），文件内容为 `{}` pretty 形态 + 尾换行 | 单点 |
| `workspace_config_apply_inner` | 边界 | 幂等：候选 == 盘面现值 → 连续两次 apply 后文件字节与 mtime 语义（内容逐字相同；重复 apply 不改变内容） | 单点 |
| `workspace_config_apply_inner` | 边界 | 候选含未知顶层键 → `Ok` 且该键保留在文件顶层（`extra` 展开），重读报告 `config.extra` 含该条目 | 单点 |
| 签名面 | 正向 | 结构锚（`include_str!("mod.rs")`）：两函数签名第二参为 `serde_json::Value` / `&serde_json::Value`，源码零 `WorkspaceConfig` typed 入参；`workspace_config` / `workspace_config_inner` 既有面逐字未动 | 单点 |
| 写面门结构 | 边界 | `plan_apply_config` 失败时 `write_atomic` 不在调用链上（以「脏候选 → 文件零变」结构性证明：失败路径无 `fs` 写入） | 单点 |
| 写失败面 | 异常 | 目标目录只读（`cfg(unix)` 0o555）→ `Err` 或（root 运行时可写穿）零临时残留 + 目标可读（沿既有 `write_atomic` 兜底口径），不 panic | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| —（Tauri 运行时） | 不 mock：`#[tauri::command]` 保留原函数可直调（`commands/config/mod_test.rs` 既有先例），`*_inner` 为离运行时的可测面 | 全部用例 |
| —（文件系统） | 不 mock：既有 `TempWs` 真实 tempdir + 真实 `config.json`（mtime 与内容双证依赖真实 FS） | 全部用例 |
| —（`plan_apply_config` / `write_atomic` / `load`） | 不 mock：三段真实组合——替身会让「失败零写入」与「写后重读」两条契约空转 | 成功 / 失败全簇 |

### packages/desktop/src-tauri/src/commands/atomic.rs -> packages/desktop/src-tauri/src/commands/atomic_test.rs

#### 待测功能

- `write_atomic()`: 同目录临时文件 + `rename` 覆盖的原子写（`pub(crate)`，`archi_set_gate` 与 `workspace_config_apply` 共用）；临时名 `{name}.archi-tmp`；失败清临时文件；命令层唯一 `fs::write` 位点
- 注：`src/commands/atomic_test.rs` 需在 `atomic.rs` 内以 `#[cfg(test)] mod atomic_test;` 登记（本相位只登记非测试行，测试行由 test-gen 同批落）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `write_atomic()` | 正向 | 写新内容 → 目标文件内容逐字等于入参、同目录零 `*.archi-tmp` 残留 | 单点 |
| `write_atomic()` | 正向 | 覆盖既有文件 → 内容整体替换（无半截：读回逐字等于新内容） | 单点 |
| `write_atomic()` | 边界 | 空内容 `""` → `Ok` 且文件为空；超长内容（1 MB）→ `Ok` 且读回逐字相等 | 矩阵迭代（2 形态） |
| `write_atomic()` | 边界 | 目标为纯文件名（无父目录，`Path::new("file")`）→ `Err` 文案含「无父目录」 | 单点 |
| `write_atomic()` | 边界 | 目标名含特殊字符（空格 / emoji / 名字本身以 `.archi-tmp` 结尾）→ 临时名不撞目标、写入成功 | 矩阵迭代（3 形态） |
| `write_atomic()` | 异常 | 父目录不存在 → `Err` 文案含「写入临时文件失败」，零残留、目标未被创建 | 单点 |
| `write_atomic()` | 异常 | 目录只读（`cfg(unix)` 0o555）→ `Err` 或可写穿时零残留（沿既有 `archi` 兜底口径，不伪造失败） | 单点 |
| `write_atomic()` | 正向 | 结构锚：`include_str!("atomic.rs")` 中 `fs::write(&temporary` 恰一处（命令层唯一落盘点），`create_dir_all` 零命中 | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| —（文件系统） | 不 mock：真实 tempdir（std 自建 RAII，沿 `commands/archi/mod_test.rs` 装置） | 全部用例 |
| 平台差异 | 不 mock：权限用例 `#[cfg(unix)]` 门控 + 「root 运行时可写穿」兜底分支（既有二形态先例） | 只读目录用例 |

### packages/desktop/src-tauri/src/commands/archi/mod.rs -> packages/desktop/src-tauri/src/commands/archi/mod_test.rs

#### 待测功能

- `archi_set_gate()` / `archi_set_gate_inner()`: 写回行为回归（内部改写改经 `crate::commands::atomic::write_atomic`，三档 gate 往返与错误面零变化）
- `write_atomic()`（本文件删除）: 搬移后行为经 `commands::atomic` 单点回归——`.archi-tmp` 临时后缀与两条失败清理逐字保留
- `check_root()`: 本文件私有 root 校验保持原样（与 config 轨道 `check_root` 分家，两条 root 文案不动）

> **注记**：design.md 未声明该文件的公共 API 变更（「公共函数 / API」子表无本文件行；本文件只删除私有 `write_atomic`、改引用 `crate::commands::atomic::write_atomic`；六命令签名零改）；上述条目为既有函数名 / 被删除函数名，用于定位搬移后的回归测试面。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `archi_set_gate` 写回面 | 正向 | 三档逐值往返（`Strict` / `Warn` / `Off` / `Strict`）逐字回归；非目标文件零变化、无显式 `gate` 行的元素经插入获得该行（搬移后行为零变化） | 矩阵迭代（3 档） |
| 命令层零领域规则结构锚 | 正向 | `include_str!("mod.rs")`：零 `fs::write(&temporary`、`use crate::commands::atomic::write_atomic;` 在场、零 `create_dir_all`；「`fs::write` 恰一处」断言落点改指 `commands/atomic.rs`（判据同值、落点随动） | 单点 |
| `archi_set_gate` 错误面 | 异常 | root 非法 → `Err` 且模型文件零写入（既有断言逐字回归） | 单点 |
| `archi_set_gate` 错误面 | 边界 | 只读目录失败 + 无 `archi-tmp` 残留（既有 `write_atomic只读目录失败且无残留` 用例随落点回归；`.archi-tmp` 后缀原样保留使三处既有断言零随动） | 单点 |
| `check_root()` | 边界 | root 四形态（`""` 空白 / 不存在 / 普通文件 / 不可读）→ `archi_set_gate` 逐条 `Err`，文案与搬移前逐字相同（私有校验未被 config 轨道 `check_root` 顶替） | 矩阵迭代（4 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| —（文件系统 / 模型盘面） | 不 mock：既有 `TempDir` + 真实模型 TOML（`healthy()` 装置） | 全部用例 |
| `write_atomic` | 不 mock：经真实 `commands::atomic`（搬移后的单点即被测对象） | 写回全簇 |

### packages/desktop/src-tauri/src/commands/exec/mod.rs -> packages/desktop/src-tauri/src/commands/exec/mod_test.rs

#### 待测功能

- `list_config_chat_sessions()`: 薄包装 `list_sessions(Some("config"), None, offset, limit)`；`source` 词常量收命令层（`CONFIG_CHAT_SOURCE`）；blank root → 空页信封（不触库、不 `Err`）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `list_config_chat_sessions` | 正向 | 播种 `source = "config"` 三条 + `archi` / `change`（两段 `source_ref`） / change 流程（四段）各一条 → 只命中 config 三条，`updated_at` 降序 | 单点 |
| `list_config_chat_sessions` | 正向 | 与 `agent_sessions` 同参（`Some("config")` + `None`）信封逐字段全等（薄包装等价性） | 单点 |
| `list_config_chat_sessions` | 边界 | blank root 三形态（`""` / `"   "` / `"\t\n "`）→ 空页信封（`sessions: []` / `total: 0`）且零库寻址 | 矩阵迭代（3 形态） |
| `list_config_chat_sessions` | 边界 | 分页五形态（`offset 0 limit 1` / `offset 1 limit 1` / `limit 0` / `offset` 越界 / 双参 `u32::MAX`）→ 页内容与 `total` 全量语义正确 | 矩阵迭代（5 形态） |
| `list_config_chat_sessions` | 边界 | 精确 `source` 过滤：`source = "config"` + `source_ref = Some("")` 的会话仍命中（`None` 不过滤 `source_ref`）；`source = "configx"` 不命中 | 矩阵迭代（2 形态） |
| `list_config_chat_sessions` | 边界 | 并列 `updated_at` → 按 id 降序（与 `list_archi_chat_sessions` 同排序口径） | 单点 |
| `list_config_chat_sessions` | 边界 | 空库 → `Ok` 空页；跨 root 隔离（A 库只见 A 的 config 会话） | 矩阵迭代（2 形态） |
| `list_config_chat_sessions` | 异常 | root 不可寻址（不存在路径）→ `Err` 透传不 panic | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| store | 不 mock：`tempfile` 真库 + `tauri::test::mock_app()` 托管 `WorkspaceStores`（既有 `Env` 装置） | 全部用例 |
| 会话行 fixture | 不 mock：既有 `SessionRecord` fixture 构造器播种（`source` 自由字符串，零注册模型） | 归属矩阵用例 |

### packages/desktop/src-tauri/src/commands/mod.rs -> packages/desktop/src-tauri/src/bindings/mod_test.rs

#### 待测功能

- `all_commands!`: 宏体是命令单一登记面，新增两条登记（`commands::config::workspace_config_apply` / `commands::exec::list_config_chat_sessions`）
- 登记面派生清单（`COMMAND_NAMES` / `COMMAND_WRAPPERS`）: 两清单一一对应且总数随动（54 → 56），宏体是唯一来源、生成物不得手改

> **注记**：design.md 未声明该文件的公共 API 变更（「公共函数 / API」子表无本文件行；`all_commands!` 宏体是登记面，非函数）；上述两条为本文件的宏名与其派生清单名，用于定位两条命令登记与生成物计数随动的测试面。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `all_commands!` 登记面 | 正向 | `COMMAND_NAMES` 含 `workspace_config_apply` / `list_config_chat_sessions`；`COMMAND_WRAPPERS` 含 `workspaceConfigApply` / `listConfigChatSessions`（两清单一一对应） | 单点 |
| `all_commands!` 登记面 | 边界 | 两清单条目数持衡且总数随动（54 → 56）；既有条目零位移 | 单点 |
| `all_commands!` 登记面 | 边界 | 登记位次：`commands::config::workspace_config_apply` 紧跟 `config::workspace_config`、`commands::exec::list_config_chat_sessions` 紧跟 `exec::list_archi_chat_sessions`（同族相邻而非尾部追加） | 单点 |
| `all_commands!` 登记面 | 异常 | 内存副本从产物文本删去 `listConfigChatSessions` 行 → 持衡断言失败并指认缺项（副本驱动，不改产物） | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 生成产物 | 不 mock：真实读取 `src/types/generated/*`（既有 `generated_path()` + 跨进程文件锁装置）；异常形态用内存字符串副本 | 全部用例 |

### packages/desktop/src/types/generated/bindings.ts -> packages/desktop/src-tauri/src/bindings/mod_test.rs

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（「公共函数 / API」子表无本文件行，生成物）；下列条目取自「修改文件」表的 `bindings.ts` 行与路由 / API 设计表的两条 IPC。生成物禁手改，测试只读产物文本断言，不 mock 生成流程。 -->

- `commands.workspaceConfigApply`: `(root: string, candidate: unknown) => __TAURI_INVOKE<WorkspaceConfigReport>("workspace_config_apply", { root, candidate })`
- `commands.listConfigChatSessions`: `(root: string, offset: number, limit: number) => __TAURI_INVOKE<SessionPage>("list_config_chat_sessions", { root, offset, limit })`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `workspaceConfigApply` | 正向 | 包装行逐字在场：入参 `root: string, candidate: unknown`、返回 `WorkspaceConfigReport`、invoke 名 `workspace_config_apply`、载荷 `{ root, candidate }` | 单点 |
| `listConfigChatSessions` | 正向 | 包装行逐字在场：三参 `root: string, offset: number, limit: number`、返回 `SessionPage`、invoke 名 `list_config_chat_sessions` | 单点 |
| `workspaceConfigApply` | 异常 | 产物文本副本把 `candidate: unknown` 改成 `WorkspaceConfig`（typed 入参回潮）→ 断言失败并指认（wire face typed 入参禁令的机械锚） | 单点 |
| 生成幂等 | 边界 | 连续两次 `export_bindings` 零 diff（既有用例回归）；`bindings.ts` 的 diff 只来自生成（不手改） | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 生成物文件 | 不 mock：真实读取 `src/types/generated/bindings.ts`（含跨进程文件锁防并发改写）；异常形态用内存字符串副本 | 全部用例 |

### packages/desktop/src/types/generated/bindings.zod.ts -> packages/desktop/src-tauri/src/bindings/mod_test.rs

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（「公共函数 / API」子表无本文件行，生成物）；条目取自「修改文件」表 `bindings.zod.ts` 行——两新命令的 zod 出线，`candidate` 沿用仓内既有 `unknown` 出线形态（`z.unknown()`，先例 `ConfigExtraFieldSchema.value`）。 -->

- `commands.workspaceConfigApply` 入参 / 返回 schema：`candidate` 为 `z.unknown()`、返回 `WorkspaceConfigReportSchema`
- `commands.listConfigChatSessions` 入参 / 返回 schema：三参 + `SessionPageSchema`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `workspaceConfigApply` schema | 正向 | 入参对象含 `root` / `candidate` 两 getter（`candidate` 为 `z.unknown()`）、返回 `WorkspaceConfigReportSchema` | 单点 |
| `listConfigChatSessions` schema | 正向 | 入参对象含 `root` / `offset` / `limit` 三 getter，返回 `SessionPageSchema` | 单点 |
| `workspaceConfigApply` schema | 异常 | 产物文本副本删 `candidate` getter → 断言失败并指认缺键（zod 面漏字段 = 前端解析面漏字段） | 单点 |
| 两 schema 形态 | 边界 | 各落同一 `strictObject` 块内（不裂成独立对象）；连续两次导出零 diff | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 生成物文件 | 不 mock：真实读取 `src/types/generated/bindings.zod.ts`；异常形态用内存字符串副本 | 全部用例 |

### packages/desktop/src/components/agent/message-cards/tool-cards.tsx -> packages/desktop/src/components/agent/message-cards/tool-cards.test.tsx

#### 待测功能

- `ConfigProposeCard()`: 私有卡组件（五态判据 + 字段级 diff + 「应用」按钮），注册表内部、不导出
- `BUILD_IN_TOOL_CARDS`: 注册表新增第 8 键 `propose_config`（既有 7 键逐字零位移）

> **注记**：design.md 未声明该文件的公共 API 变更（「公共函数 / API」子表无本文件行；`ConfigProposeCard` 是注册表内部卡组件、不导出，见「公共函数」表末的私有函数说明）；上述两条为本文件既有常量名与新增私有组件名，条目面取自「类型定义」表末行（提案卡载荷的宽容解析面）与 DD14 的五态判据，测试经 `BUILD_IN_TOOL_CARDS["propose_config"]` 与 `AgentMessages` 渲染观察。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ConfigProposeCard` | 正向 | 进行中：`output === null` + `turnRunning` 未定 → 运行态（Spinner / running 语义）且零诊断块 | 单点 |
| `ConfigProposeCard` | 正向 | 失败：`output.isError = true`、`content` 为多行 `path: message` 诊断 → 诊断原文 `<pre>` 呈现、零「应用」按钮 | 单点 |
| `ConfigProposeCard` | 正向 | 可应用：`candidate != null` + `changes` 三条（`added` / `removed` / `changed`）→ 逐项 `path` 与 kind 呈现；added / removed 沿用 `EditCard` 的双 `CodeBlockLine`（`variant="removed"` / `"added"`）；「应用」可点 | 单点 |
| `ConfigProposeCard` | 正向 | 无差异支：`candidate != null` + `changes == []` → 按钮 `disabled` + 文案「与当前配置一致」 | 单点 |
| `ConfigProposeCard` | 正向 | 只读模式：`candidate === null` → 仅盘面摘要（`current` 键值呈现），零应用按钮 | 单点 |
| `ConfigProposeCard` | 异常 | 内容不可解析两形态（`"not json"` / `"{"`）→ 原文降级渲染、零抛错、零空白 | 矩阵迭代（2 形态） |
| `ConfigProposeCard` | 异常 | 载荷形态异常：`changes` 非数组 / 项缺 `path` / `kind` 为未知值 / `current` 非对象 → 宽容读（`asRecord` / `textOf`）不抛错，呈降级形态 | 矩阵迭代（4 形态） |
| `ConfigProposeCard` | 边界 | `candidate = {}` + `changes == []` → 可应用态但禁用；`changes` 200 项 → 渲染不 panic（列表限高可滚动） | 矩阵迭代（2 形态） |
| `ConfigProposeCard` | 边界 | provider 缺席（无 `ConfigApplyScope`）→ 按钮禁用 + 文案「应用只在配置页可用」（既有 `tool-cards.test.tsx` 渲染面零包装即可跑） | 单点 |
| `ConfigProposeCard` 应用 | 正向 | provider 在场（`{root, onApplied}`）→ 点击恰一次 `ipc.workspaceConfigApply(scope.root, candidate)`；成功后 `onApplied()` 恰一次 | 单点 |
| `ConfigProposeCard` 应用 | 异常 | invoke reject → inline 错误呈现、按钮回到可点、候选不被丢弃（再次点击触发第二次调用） | 单点 |
| `ConfigProposeCard` | 正向 | 规范化明示在场：「未知子键、键序与格式会丢失 / 变化」类文案 | 单点 |
| `ConfigProposeCard` | 边界 | 多次提案独立：两张卡并渲染 → 点第一张后第二张仍可点且内容未被改写（无失效标记） | 单点 |
| `ConfigProposeCard` | 异常 | 前端零配置语义结构锚：源码零 `KNOWN_KEYS` / 零阈值 / 零 diff 计算（唯一决策 `changes.length === 0` 与 `candidate === null`） | 单点 |
| `BUILD_IN_TOOL_CARDS` | 正向 | 注册表第 8 键 `propose_config` 在场，既有 7 键逐字零位移 | 单点 |
| `BUILD_IN_TOOL_CARDS` 注册面 | 边界 | 未注册名（`propose_configx` / 大小写变体）→ 回退通用卡（既有 `ToolPartView` 兜底），不命中本卡 | 矩阵迭代（2 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core` | mock `invoke` 按命令名分派（恰 `workspace_config_apply`）：成功 / reject / 手动 pending 三态 | 应用调用与应用失败用例 |
| `ConfigApplyScope` / `useConfigApplyActions` | 不 mock：真实 provider 组合（缺席形态以不包 provider 直渲） | 全部用例 |
| 卡片入参 | 不 mock：`AgentToolCardProps` 直构（`output` 为 `{content, isError}` 字面量） | 五态矩阵 |

### packages/desktop/src/components/agent/index.tsx -> packages/desktop/src/components/agent/index.test.tsx

#### 待测功能

- `ConfigApplyScope`（barrel 第 8 个值导出）: 从 `./message-cards/config-apply-scope` 再导出，测试经 `barrel.ConfigApplyScope` 与直导入的引用同一性观察
- 既有三个值导出（`AgentInput` / `AgentSessionView` / `SessionPanel`）: 逐字零位移，出口集合由三元组扩为四元组

> **注记**：design.md 未声明该文件的公共 API 变更（「公共函数 / API」子表无本文件行；`ConfigApplyScope` 的「所在文件」记为 `message-cards/config-apply-scope.tsx`）；本文件改动取自「修改文件」表：barrel 增一条值导出（第 8 名，DD12——供 `views/config` 经入口取用、MUST NOT 深层导入），上述两条为既有导出名与新增导出名。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| barrel 出口集合 | 正向 | `Object.keys(barrel).sort()` 恰 `['AgentInput', 'AgentSessionView', 'ConfigApplyScope', 'SessionPanel']`（既有三元组扩为四元组） | 单点 |
| `ConfigApplyScope` | 正向 | `barrel.ConfigApplyScope` 与 `./message-cards/config-apply-scope` 直导入为**同一引用**（不建第二个 provider 实现） | 单点 |
| barrel 出口集合 | 边界 | 不导出 `ConfigApplyActions` 类型（内层只供本元素内消费）；barrel 源码恰一条 `export { ConfigApplyScope } from './message-cards/config-apply-scope';`（零重复行） | 单点 |
| `ConfigApplyScope` | 异常 | 经 barrel 包裹后子组件读到 `{root, onApplied}`；不包 provider 时读到 `null` 且不抛错（降级语义随出口一致，不是 barrel 侧新包装层） | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core` | mock `invoke` / `Channel` 最小面（barrel 冒烟渲染零取数，既有装置） | 全部用例 |

### packages/desktop/src/views/config/config-view.tsx -> packages/desktop/src/views/config/config-view.test.tsx

#### 待测功能

- `ConfigView()`: 既有导出组件改形态——单列 → 横向分栏（`ResizablePanelGroup orientation="horizontal"`：左 `ConfigChatPanel` → `ResizableHandle` → 右既有页头 + 五分区），`onApplied={state.refresh}` 单路重取
- `ConfigHeader()`: 本文件私有页头，文案与 doc 改判（不再「只读页面」）
- `ConfigStatusFaces()`: 本文件私有状态面，既有错误 / 空态 / loading 三面判据零改（分栏后仍按原判据隐藏）

> **注记**：design.md 未声明该文件的公共 API 变更（「公共函数 / API」子表无本文件行；本文件是既有组件改形态）；改动取自「修改文件」表与 Module Contract 的 `ConfigView` 行，测试面细节见下方用例表。上述三条为本文件既有入口名（一导出 + 两私有），不构成新增公共面。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ConfigView` | 正向 | 分栏结构：`config-chat-*` 骨架实例（`config-chat-actions` 等）与右栏 `config-refresh` + 五分区（`config-diagnostics` / 基础 / tests / 写入保护 / 未知字段）同屏；`config-view` testid 保留 | 单点 |
| `ConfigView`（链路：卡片 → `workspace_config_apply` → `useWorkspaceConfig.refresh`） | 正向 | 应用成功后 `workspace_config` invoke **恰增加一次**（页面重取一次、新值就地呈现），且不采信 apply 返回值作第二真相源（只经 refresh 单路） | 单点 |
| `ConfigView` | 边界 | 缺省收起态：右栏全宽、零会话类 invoke（`list_config_chat_sessions` / `session_detail` / `agent_session_watch` 零调用） | 单点 |
| `ConfigView` | 边界 | root 切换 → 会话面板复位（清单清空、`sessionId` 置 null）且页面报告按新根重取 | 单点 |
| `ConfigView` | 异常 | 报告命令 reject（既有面）→ `config-error` 在案、右侧不崩（会话面板照常挂载） | 单点 |
| `ConfigView` | 边界 | 文件缺失 / fatal 诊断（既有三态）→ 分区隐藏（`config-empty` / 默认值提示）但分栏与会话面板仍在 | 矩阵迭代（2 形态） |
| `ConfigView` | 正向 | 折叠真相单一结构锚：源码零折叠 state、零 `onResize` 像素派生、零面板显隐开关（折叠全在骨架内） | 单点 |
| `ConfigHeader()` | 正向 | 页头文案改判为「会话 + 应用」语义、零「只读页面」字样；`config-*` testid 与既有状态面结构不变 | 单点 |
| `ConfigStatusFaces()` | 边界 | 错误 / 空态 / loading 三面判据零改：报告 reject → `config-error`、文件缺失 → `config-empty`、默认值提示照旧，分栏后仍在右栏内 | 矩阵迭代（3 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core` | mock `invoke` 按命令名分派（`workspace_config` / `workspace_config_apply` / `list_config_chat_sessions` / `agent_start` 等），返回 camelCase fixture（既有装置扩命令面） | 全部用例 |
| 内部模块 | 不 mock：`useWorkspaceConfig` / 五分区 / `ConfigChatPanel` 及其 hook 链真实组合——本文件兼作**跨模块组合用例挂靠入口**（链路方向：页面 → 面板 → hook → 会话基建 → `agent_start`；写回方向：卡片 → `workspace_config_apply` → 页面 `refresh`） | 组合用例 |
| 分栏装置 | 真实 `ResizablePanelGroup`（既有 `index.test.tsx` / `archi-chat-panel.test.tsx` 同形包裹），不 mock 布局库 | 全部用例 |

### packages/desktop/src/views/config/hooks/use-config-chat.ts -> packages/desktop/src/views/config/hooks/use-config-chat.test.ts

#### 待测功能

- `useConfigChat({ root })`: 会话状态面 + 清单分页 + 新会话 / 历史切换 + root 复位 + 运行中禁发判据 + `note` 三态；归属三件（`source = "config"` / `sourceRef = null` / `cwd = null`）由本 hook 承担；清单与写回一律经 `ipc`（MUST NOT 直调 `commands.<name>`）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `useConfigChat` | 正向 | 空态（`sessionId === null`）→ 零重放查询、零订阅、零 `session_detail`；`canSend === true` / `canStart === true` / `disabledReason === null` | 单点 |
| `useConfigChat` | 正向 | 首发：`send({text, permissionMode})` → `agent_start` invoke 入参 `sessionId: null`、`source: "config"`、`sourceRef: null`、`cwd: null`、root 为本 hook 入参 | 单点 |
| `useConfigChat` | 正向 | 清单首屏：打开历史 → `ipc.listConfigChatSessions(root, 0, 20)` 恰一次，`sessions` 替换、`total` 透传 | 单点 |
| `useConfigChat` | 正向 | 分页追加：`loadMore` → 以 `offset = sessions.length` 再取一页并累加；`sessions.length < total` 为「加载更多」判据 | 单点 |
| `useConfigChat` | 正向 | 重取时机：会话 id 由 null → 非 null、`running` 由 true → false 各触发一次首屏重取；挂载本身零清单 invoke（零轮询） | 单点 |
| `useConfigChat` | 正向 | 新会话：`reset()` 被调用且 `sessionId` 置 null（不预建会话行、零铸造 invoke）；历史点选：以该 id 直查 `session_detail` + 转录重放，零 `source` 反查 | 单点 |
| `useConfigChat` | 正向 | 禁发矩阵：`running === true` → `canSend === false`、`canStart === false`、`disabledReason` 为运行中文案、`note === disabledReason` | 单点 |
| `useConfigChat` | 正向 | 状态行三态优先级：空态 → 草案文案；已开会话空闲 → 写回入口提示（「应用」在提案卡上）；运行中 → 运行中理由 | 矩阵迭代（3 形态） |
| `useConfigChat` | 边界 | `root = null` → 零 invoke（清单与发起皆不发），`canSend` 不因此被禁用（禁用判据只有 `running`） | 单点 |
| `useConfigChat` | 边界 | root 切换 → `sessionId` / 会话镜像 / 清单 / 展开态复位；在途清单请求票号作废（旧根页不落新根） | 单点 |
| `useConfigChat` | 边界 | `total === 0` 空清单与 `total > 页宽` 两形态下 `loadMore` 可用性判据正确 | 矩阵迭代（2 形态） |
| `useConfigChat` | 异常 | 清单命令 reject → `listError` 非空、`listLoading === false`、面板不崩；下次打开历史重取成功即清错 | 单点 |
| `useConfigChat` | 异常 | 连点「历史」/ 收口重取交错 → 只有最新一拍落地（旧页不覆盖新页） | 单点 |
| `useConfigChat` | 异常 | 历史选中的会话不存在（`session_detail` reject）→ 会话面 `error` 呈现、`sessionId` 保留（可重试），清单不受污染 | 单点 |
| `useConfigChat` | 边界 | 发送纪律结构锚：源码零 `commands.` 直调（一律 `ipc.`） | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core` | mock `invoke` 按命令名分派（`agent_start` / `agent_stop` / `session_detail` / `agent_session_transcript` / `agent_session_watch` / `list_config_chat_sessions`）；`Channel` 为可编程类（既有 `use-archi-chat.test.ts` 装置） | 全部用例 |
| 内部模块 | 不 mock：`useAgentSession` / `@ai-sdk/react` / `agent-transport` 真实组合——hook 即链路入口，归属参数经真实链路到达 invoke | 归属与发起用例 |
| 时钟 | 不 mock：`updatedAt` / `startedAt` 由 fixture 显式给定（排序与显示断言取原始数值） | 清单排序用例 |

### packages/desktop/src/views/config/components/config-chat-panel.tsx -> packages/desktop/src/views/config/components/config-chat-panel.test.tsx

#### 待测功能

- `ConfigChatPanel({ root, onApplied })`: 面板薄封装——`useConfigChat` 状态面 + `ConfigApplyScope`（`root` 非 null 时提供）+ `SessionPanel`（`testIdPrefix = "config-chat"`）+ composer 插槽（`AgentInput`，`idPrefix = "config-chat"`）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ConfigChatPanel`（链路：面板 → hook → 会话基建 → `agent_start`） | 正向 | 输入并发送 → 恰一次 `agent_start` invoke，入参含 `source: "config"` / `sourceRef: null` / `cwd: null` / root（组合用例挂靠本入口：面板是这条链路的最上层调用方） | 单点 |
| `ConfigChatPanel` | 正向 | 骨架复用：`config-chat-*` testid 集合在案（`-actions` / `-collapse` / `-new` / `-history` / `-note` / `-rail` / `-expand` / `-session`），且 `SessionPanel` 与 `@/components/agent` 直导入为同一实例 | 单点 |
| `ConfigChatPanel` | 正向 | composer 注入形态：空闲时输入 + 发送可用；运行中 disabled + 理由文案 + 停止入口在场（`config-chat` 前缀 id） | 单点 |
| `ConfigChatPanel` | 边界 | 缺省收起 → 展开前后同一骨架实例；收起期间零会话类 invoke | 单点 |
| `ConfigChatPanel` | 边界 | `root = null` → 面板渲染但零 invoke；`ConfigApplyScope` 不提供（卡片呈只读态） | 单点 |
| `ConfigChatPanel` | 正向 | `onApplied` 透传：包裹的 scope 提供 `{root, onApplied}`，卡片点击应用成功 → 回调恰一次 | 单点 |
| `ConfigChatPanel` | 异常 | `agent_start` reject → 面板 error 呈现、不崩、composer 恢复可用 | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core` | mock `invoke`（按命令名分派）+ 可编程 `Channel`（既有 `archi-chat-panel.test.tsx` 装置） | 全部用例 |
| 内部模块（`useConfigChat` / `SessionPanel` / `AgentInput` / `AgentSessionView`） | 不 mock：真实组合——组合用例入口即本组件 | 全部用例 |
| 分栏装置 | 真实 `ResizablePanelGroup` 包裹（骨架自带折叠分栏） | 全部用例 |

### packages/desktop/src/components/agent/message-cards/config-apply-scope.tsx -> packages/desktop/src/components/agent/message-cards/config-apply-scope.test.tsx

#### 待测功能

- `ConfigApplyScope({ value, children })`: 卡片级写回动作面的 Provider 薄封装（`{ root, onApplied }`）
- `useConfigApplyActions()`: 卡片侧读取；`null` = 无写回通道（卡片呈只读态）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `ConfigApplyScope` / `useConfigApplyActions()` | 正向 | 包裹子组件 → 读到的 `{ root, onApplied }` 与传入 value 逐字段相等（同引用） | 单点 |
| `useConfigApplyActions()` | 边界 | 无 provider → 返回 `null`（缺席降级，不抛错） | 单点 |
| `useConfigApplyActions()` | 边界 | value 换根（`root` 变化）→ 消费侧读到新值（不缓存旧引用） | 单点 |
| `ConfigApplyScope` | 边界 | 子节点形态（单元素 / fragment / 零子节点）→ 渲染零崩、context 仍可读 | 矩阵迭代（3 形态） |
| `ConfigApplyScope` | 异常 | 嵌套两层 provider → 内层覆盖外层（最近者优先）、`onApplied` 取内层回调；不 panic | 单点 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| —（无 mock） | 纯 React context：以 `renderHook` / 最小消费组件真实渲染；零 IPC 面（本文件不触 `invoke`） | 全部用例 |

## 不可测试项

| 模块 | 原因 | 替代保证 |
|---|---|---|
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mod.rs`（`pub(crate) mod config_propose;`）、`packages/desktop/src-tauri/crates/infra/agent/src/lib.rs`（`mod config_knowledge;`） | 模块登记行，无函数 / 分支 / 输出可断言 | 编译期强制：漏登记则 `crate::config_propose` / `crate::config_knowledge::knowledge()` 无法解析，`compose.rs` 与 `mount.rs` 立即编译失败（AC-1 / AC-4 的用例同在编译面之内） |
| `packages/desktop/package.json`（`version` `0.7.2` → `0.7.3`） | 一行字面量，无行为可断言 | 桌面端 `run check` 链 + `git diff` 复核（AC-9）；`src-tauri/tauri.conf.json` 经 `"version": "../package.json"` 跟随、`src-tauri/Cargo.toml` 不随动，同属机械核对 |
| `.dev-team/architecture/graph/model/06-frontend.toml`（表头 `72 关系` + `Views.Config → Components.Agent` 一条新边） | 架构模型资产：声明式 TOML，无函数 / 分支；模型可解析性由 `archi` 的既有解析面覆盖，但「表头注释与模型本体同值」无测试面（仓内无对仓模型的 golden 断言） | AC-10：`archi` 读数核对（元素 / 关系计数与零结构诊断）+ `views/config/**` 的 `import … from '@/components/agent'` 证据 + `git diff` 只含自己那一条（DD15 漂移纪律） |
| `.dev-team/architecture/decisions/0001-model-granularity.md`（§9 复算 `72 条` / `06(72) = 121` / 基线 `121 关系`） | 决策记录散文，纯文本资产 | AC-10 手工核对（与模型本体同值）；既有漂移（实读 54 / 122）按 D16 / R8 **不在本变更修复范围** |
| `.dev-team/architecture/models/Frontend/Components/App.Frontend.Components.Agent.md`（职能段 7 条 / 8 名 + 公共面表增 `ConfigApplyScope` 一行） | `api = declared` 元素的契约散文，公共面 7 名既有留白（无 `api-op` / `api-reexport` fence），新增名只动散文；`api:check` 该元素 `gate = "off"` 不纳管门禁 | `api:check` 预期零 diff + 手工核对散文计数；不写 `api-reexport` fence（否则生成物 `.i.ts` 变化，与 AC-9「diff 只来自 `bindings:export`」冲突） |
| AC-8（五份 delta spec 的改判零残留冲突） | 无代码落点：改判面是 spec 文本而非可执行单元 | 桌面端以基线全文检索逐处复核（检索词：只读 / 两处 / 唯一生产者 / 三臂 / 其余非 change）；本相位不代跑 |
| AC-9 的机械门禁部分（`check` 链、`cargo test --workspace`、生成物零手改、版本口径） | 静态脚本与全量管线由桌面端执行，不在单元测试面内 | 桌面端相位执行；本文档只声明覆盖范围与断言判据，不复制运行脚本细节 |
| 私有助手 `read_document` / `rename_in_place` / `integerize` / `expand_extra` / `trim`（`core/config`）、`diff` / `summary` 拼装（`config_propose`）、`check_root`（`commands/config`）、`ConfigProposeCard`（`tool-cards` 注册表内部） | 私有函数 / 组件不外露；为其加 `pub` 只服务测试，污染导出面（AGENT.md「no test-only exports」） | 经各自公共入口行为面覆盖（`load_file_face` / `to_file_value` / `plan_apply_config` / `execute` / `workspace_config_apply_inner` / `BUILD_IN_TOOL_CARDS` 渲染）；异常形态用内存副本或注入入参驱动 |
| 「检测出什么」类断言（项目识别数量 / 依赖方向） | 会话式写回的产物取决于模型，不可断言（proposal「验收面口径」） | 验收只锚注入臂装配、`propose_config` 契约矩阵、写面（门 + 失败零写入 + P1–P4）、卡片五态四类事实 |
