# 测试设计: sdk-engine-gap-closure

> **日期**: 2026-10-04

---

## 验收范围

<!-- 被测文件或模块列（单值）指向承载用例的测试文件；拆行（同上——XX 半边）为同一 AC 的次要承载面。
     纯静态约束 / 生成物 / 一次性配置半边填 —（见不可测试项 N）。 -->

| AC ID | 验收条件 | 被测文件或模块 |
|--------|---------|----------|
| AC-1 | workspace 依赖 rig facade 切换：`cargo test --workspace` 全绿；`crates/infra/agent` 源码 `rig_core::` grep 零残留；facade 条目锁 `=0.42.x`、`default-features = false` + 显式 features | packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop_test.rs |
| AC-1（同上——normalize 路径迁移随动半边） | — | packages/desktop/src-tauri/crates/infra/agent/src/sdk/normalize_test.rs |
| AC-1（同上——resume 路径迁移随动半边） | — | packages/desktop/src-tauri/crates/infra/agent/src/sdk/resume_test.rs |
| AC-1（同上——runner 路径迁移随动半边） | — | packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner_test.rs |
| AC-2 | AGENT.md preamble 注入：fixture root 有 AGENT.md → 请求 preamble 逐字含其内容；缺席 → `None`；会话中改文件 → 下一轮请求生效；全程 `crates/core/agent` 与 compose 面零 diff | packages/desktop/src-tauri/crates/infra/agent/src/sdk/preamble_test.rs |
| AC-2（同上——loop 每轮重读接线半边） | — | packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop_test.rs |
| AC-3 | 工具质量（read / grep）：read 超限截断 + 尾部留痕 + offset 翻页取回后续窗口；grep 正则语义匹配 + 上下文行参数；边界用例（恰阈值过 / 超 1 截断） | packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools_test.rs |
| AC-4 | bash 工具：policy 三档矩阵逐格断言（Default 拒 / AcceptEdits 放 / Bypass 放）；进程执行 stdout/stderr 合并回灌 ToolResult；超时收口；终止/超时时进程树被清理；Windows 无 git-bash 环境走 `cmd /C` 兜底；`RunStarted.tools` 恰七工具 | packages/desktop/src-tauri/crates/infra/agent/src/sdk/bash_test.rs |
| AC-4（同上——policy 三档矩阵半边） | — | packages/desktop/src-tauri/crates/infra/agent/src/sdk/policy_test.rs |
| AC-4（同上——七工具定义面半边） | — | packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools_test.rs |
| AC-5 | L1 单结果上限：全工具单结果字节上限边界测试；bash 输出同归此层截断留痕 | packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools_test.rs |
| AC-6 | L2 确定性剪裁：prune 后老 tool_result 为占位符且 tool_use/tool_result 配对完整、最近窗口保护、首条 user 在场；不够再丢最老完整轮对；resume 重建后 + 每请求前两调用点可考；store 转录仍全量 | packages/desktop/src-tauri/crates/infra/agent/src/sdk/context_test.rs |
| AC-6（同上——runner resume 重建后第一调用点半边） | — | packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner_test.rs |
| AC-6（同上——loop 每请求前第二调用点半边） | — | packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop_test.rs |
| AC-7 | L3 compaction：水位触发 → 新史 = [摘要, 首条 user, 最近轮对]；摘要调用失败 → 降级 L2 硬裁 + SystemNotice 留痕，run 不失败收敛 | packages/desktop/src-tauri/crates/infra/agent/src/sdk/compact_test.rs |
| AC-7（同上——loop 请求前编排与降级收敛半边） | — | packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop_test.rs |
| AC-8 | 防线可观测：`SystemNotice{subtype:"context_pruned"\|"context_compacted", payload:{before, after, layer}}` 流出并按密封事件落库；delta 零落库纪律不破 | packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop_test.rs |
| AC-8（同上——DefenseNotice 载荷形状 context 半边） | — | packages/desktop/src-tauri/crates/infra/agent/src/sdk/context_test.rs |
| AC-8（同上——降级 notice 载荷 compact 半边） | — | packages/desktop/src-tauri/crates/infra/agent/src/sdk/compact_test.rs |
| AC-9 | provider `context_length` 列：store 可空列读写（旧记录缺列读兼容）；管理页编辑（留空 = 未配置）；sdk 窗管理消费该值，缺席走 128K 启发式；bindings 再生成 | packages/desktop/src-tauri/crates/infra/store/src/model_test.rs |
| AC-9（同上——sdk 窗管理消费缺省 128K 半边） | — | packages/desktop/src-tauri/crates/infra/agent/src/sdk/context_test.rs |
| AC-9（同上——facade builder 缝半边） | — | packages/desktop/src-tauri/crates/infra/agent/src/lib_test.rs |
| AC-9（同上——compose 组合根接线半边） | — | packages/desktop/src-tauri/crates/infra/agent/src/compose_test.rs |
| AC-9（同上——命令面 CRUD 半边） | — | packages/desktop/src-tauri/src/commands/agents/mod_test.rs |
| AC-9（同上——hook 透传半边） | — | packages/desktop/src/views/agents/hooks/use-agent-providers.test.ts |
| AC-9（同上——管理页表单半边） | — | packages/desktop/src/views/agents/components/provider-panel.test.tsx |
| AC-10 | spec 边界留痕：desktop-agent-execution「SDK 租户 MVP 边界」bash 条款改写为「bash 无沙箱」知情边界；policy_test / tools_test 两条锁定断言翻转归位 | packages/desktop/src-tauri/crates/infra/agent/src/sdk/policy_test.rs |
| AC-10（同上——tools_test bash 缺席断言翻转半边） | — | packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools_test.rs |
| AC-11 | 版本交付：归档时 `packages/desktop/package.json` version 0.4.7 → 0.4.8 | —（见不可测试项 4） |

静态半边落不可测试项：AC-1 的 Cargo.toml 条目子句与 `rig_core::` 零残留 grep（不可测试项 1）、AC-2 的零 diff 约束子句（不可测试项 2）、AC-9 的 bindings 再生成半边（不可测试项 3）、AC-10 的 spec markdown 文本半边（不可测试项 5）。

---

## 单元测试

<!-- 测试框架识别（test_detect_frameworks）：Rust 侧 = cargo test --workspace（src-tauri workspace 根注册，cargo 自动覆盖新 crate 与新模块测试）；前端侧 = vite-plus（vp test）。
     单元测试路径（test_resolve_paths）：errors 为空，全部 16 个模块均解析成功，无不可解析项。 -->

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/preamble.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/preamble_test.rs

#### 待测功能

- load(): 读 `<root>/AGENT.md`——存在逐字返回（超 32KB 截前缀、char 边界）、缺席或读取失败返回 `None`；每轮调用即每轮重读，无内部缓存

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| preamble::load 在场逐字 | 正向 | root 写入含中文 / emoji / 换行的 AGENT.md → `load` 返回与文件字节逐字一致（无包装、无前缀后缀） | 新增 |
| preamble::load 缺席 None 且无 CLAUDE.md 兜底 | 异常 | root 无 AGENT.md → `None`；同目录放 CLAUDE.md 仍 `None`（严格无兜底） | 新增 |
| preamble::load 读取失败一律 None | 异常 | AGENT.md 路径实为目录 / root 本身不存在 → `None`（尽力增强不炸，非 NotFound IO 错误同口径） | 新增 |
| preamble::load 恰 32KB 不截断 | 边界 | 文件恰 32 * 1024 字节 → 全文返回、无截断留痕 | 新增 |
| preamble::load 超 32KB 截前缀且多字节字符不劈半 | 边界 | 32KB + 1 起的中文重载文件 → 返回前 32KB 且 char 边界对齐（末尾无残缺多字节序列） | 新增 |
| preamble::load 每轮重读 | 边界 | 两次连续 `load` 调用之间改写 AGENT.md 内容 → 第二次返回改后内容（无缓存语义，改动即生效） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 文件（进程边界依赖白名单） | 不 mock——tempfile tempdir 真实文件系统承载 AGENT.md fixture（写 / 改 / 删均为真实盘上操作） | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/bash.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/bash_test.rs

#### 待测功能

- execute(): bash 执行体——探测 shell 底座 → `tokio::process` 执行 → 超时收口 → 进程树清理 → stdout/stderr 合并返回（stdout 段在前、stderr 段带标记在后）；非零退出码 `Err`
- probe_windows_shell(): PATH 纯探测（`Git\bin\bash.exe` / `Git\usr\bin\bash.exe` 形态，排除 System32 的 WSL bash 同名误中），未命中退 `Cmd`
- ShellBase: `enum ShellBase { GitBash(PathBuf), Cmd, Sh }`——shell 底座探测产物
- kill_process_tree(): 模块私有，经 execute 的超时 / 终止路径间接驱动（`taskkill /PID <pid> /T /F` + `CREATE_NO_WINDOW` 形状，unix `kill -9`）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| bash::execute 命令执行输出合并回灌 | 正向 | git-bash 在场执行 `echo` → `Ok`，stdout 段在前；同命令追加 stderr 输出 → stderr 段带标记在后拼接 | 新增 |
| bash::execute cwd 承接 workspace root | 正向 | 命令以相对路径写文件 → 文件落在 root 下（cwd 语义锁定） | 新增 |
| bash::execute 非零退出码 Err | 异常 | `exit 3` → `Err` 携退出码与已产出输出 | 新增 |
| bash::execute 未知命令 Err | 异常 | 不存在的命令名 → shell 非零退出 → `Err`（不 panic、不悬挂） | 新增 |
| bash::execute 超时收口并触达进程树清理 | 边界 | `timeout_ms=1_000` 执行长驻命令（跨 shell 可用的 `ping -n 30 127.0.0.1`）→ 1 秒级返回 `Err` 携超时记因（活性护栏收口；kill 路径被触达） | 新增 |
| bash::execute timeout_ms 缺省与下限钳位 | 边界 | 缺省（不传 timeout_ms）短命令成功；`timeout_ms=0` 被钳入下限 1_000 → 短命令仍成功（未按 0 生效即证钳位） | 新增 |
| probe_windows_shell 命中 Git 形态条目 | 正向 | fabricated PATH 含 `…\Git\bin\bash.exe` → `ShellBase::GitBash(该路径)`；`…\Git\usr\bin\bash.exe` 同判 | 新增 |
| probe_windows_shell 排除 System32 WSL 同名 | 边界 | fabricated PATH 仅含 `…\System32\bash.exe` → 不误中，退 `ShellBase::Cmd` | 新增 |
| probe_windows_shell 无 bash 退 Cmd | 边界 | fabricated PATH 无任何 bash.exe → `ShellBase::Cmd` | 新增 |
| bash::execute 无 git-bash 环境走 cmd /C 兜底 | 边界 | PATH 替换窗口内剔除 git-bash 条目后执行 `echo` → 经 `Cmd` 底座成功返回（沿 agent crate 级 PATH 串行化锁装置） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 子进程 / 运行环境（进程边界依赖白名单） | 不 mock 进程执行本体——真实 `tokio::process` 起短命子进程；`probe_windows_shell` 以 fabricated PATH 字符串（`OsStr` 入参）纯函数直测，无进程参与；PATH 全局替换窗口沿 lib.rs 既有 crate 级串行化锁互斥 | probe_windows_shell 全部用例、cmd /C 兜底用例 |
| 文件（cwd 语义） | tempfile tempdir 真实目录作 workspace root | cwd 承接用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/policy.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/policy_test.rs

#### 待测功能

- allows(): bash 行为变更——Default 拒、AcceptEdits 放、BypassPermissions 放（签名不变）；既有六工具决策行不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| allows bash 三档矩阵逐格 | 正向 | `Default`+`"bash"` → 拒；`AcceptEdits`+`"bash"` → 放；`BypassPermissions`+`"bash"` → 放（AC-4 矩阵逐格断言） | 新增 |
| allows 既有六工具决策行回归 | 正向 | 只读四工具 Default 放 / write+edit Default 拒 / AcceptEdits 六工具全放——bash 行加入后既有格零漂移 | 新增 |
| bash工具名三档均拒绝_缺席断言锁定 | 异常 | 「三档均拒绝」旧语义断言**翻转删除**：bash 进入执行面清单，Default 拒、另两档放（由矩阵行替代） | 废弃 |
| bypass_permissions档六工具全放行且为六工具清单逐一命中 | 边界 | `READONLY+WRITE == TOOL_NAMES.len()` 等价断言随七工具失效——翻转：Bypass 档对七工具清单逐一放行（清单长度等价断言改写） | 废弃 |
| 清单外工具名与大小写变体与空串一律拒绝 | 边界 | `"Bash"`（大小写变体）仍拒、`""` 仍拒、清单外名仍拒——bash 入册不放宽封闭清单匹配 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| （无） | 纯决策表无进程边界依赖，不需要 Mock（沿既有文件口径） | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/tools_test.rs

#### 待测功能

- TOOL_NAMES(): `pub const TOOL_NAMES: [&str; 7]`——追加 `"bash"`
- definitions(): read schema 增 offset/limit 描述与截断语义描述；grep 增 `context` 参数、pattern 描述改正则；新增 bash 条目（`command` 必填、`timeout_ms` 可选）
- execute(): 分发增 `"bash"` 臂；结果统一经 L1 单结果 30KB 字节上限收口（Ok/Err 双路截断留痕）；read 2000 行截断 + 尾部留痕 + offset 翻页；grep 正则匹配 + `context` 上下文行（窗口合并去重、组间 `--` 分隔、缺省 0、200 命中上限维持）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 定义清单恰为七工具且名称_描述_schema齐全 | 正向 | `definitions()` 恰 7 条、与 `TOOL_NAMES` 同源同序；bash 条目 `command` 在 required、`timeout_ms` 可选；read schema 含 offset/limit 与截断语义描述；grep schema 含 `context` 参数、pattern 描述改正则口径 | 新增 |
| 定义清单恰为六工具且名称_描述_schema齐全 | 边界 | 恰六工具断言随七工具面失效（被上行替代） | 废弃 |
| bash在工具面且清单封闭性维持 | 边界 | `TOOL_NAMES` 含 `"bash"`、定义面含 bash 条目；清单仍封闭（无第八条目）——「bash 不在工具面」缺席断言翻转 | 废弃 |
| read 超 2000 行截断尾部留痕可翻页 | 边界 | 2001 行文件 → 输出含前 2000 行 + 尾部「已截断，可用 offset 翻页」留痕；`offset=2000` 翻页取回第 2001 行窗口 | 新增 |
| read 恰 2000 行不截断 | 边界 | 恰 2000 行 → 无截断留痕（恰阈值过半边） | 新增 |
| read limit 缺省与显式同上限 | 边界 | 不传 limit 与 `limit=2000` 行为一致（缺省即上限口径） | 新增 |
| grep 正则语义匹配替换子串字面 | 正向 | pattern `f.n ` 以正则语义命中（`.` 通配）；中文与 emoji 行内容保真不变 | 新增 |
| grep 子串字面语义断言翻转 | 边界 | 旧「正则元字符按子串字面语义」断言翻转：元字符 pattern 按正则解释（不再字面命中） | 废弃 |
| grep 非法正则显式 Err | 异常 | pattern `"["` → `Err`（非法正则显式失败，不静默空匹配） | 新增 |
| grep context 上下文行与窗口合并 | 正向 | `context=1` → 匹配行 ± 1 行；相邻命中窗口合并去重；不连续组间以 `--` 分隔；`context` 缺省 0 维持逐行口径 | 新增 |
| grep 200 命中上限与二进制跳过维持 | 边界 | 205 行全命中 + 二进制在场 → 仍 200 命中 + 1 截断留痕、二进制静默跳过（regex 语义下既有边界不漂移） | 新增 |
| execute L1 字节上限 Ok 路截断 | 边界 | 单行超 30KB 的文件 read → 输出钳在 30KB 内 + 截断留痕 | 新增 |
| execute L1 字节上限 Err 路截断 | 边界 | 产出超 30KB 错误内容的工具调用（edit 大文件多处未命中等）→ `Err` 内容同样钳在 30KB 内 + 留痕（Ok/Err 双路收口） | 新增 |
| bash 臂分发接入 | 正向 | `execute(root, "bash", {command: "echo ok"})` → `Ok`；`execute(root, "bash", {})`（缺 command）→ `Err` 指明 command 必填（非「未知工具」） | 新增 |
| 未知工具名执行分发显式拒绝 | 异常 | 旧用例以 `"bash"` 作清单外反例断言「未知工具」——翻转：bash 已入册，反例改用真实清单外名（如 `"rm_rf"`） | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 文件（进程边界依赖白名单） | 不 mock——tempfile tempdir 真实目录落盘 fixture（大文件 / 多行 / 二进制均为真实盘上内容） | read / grep / L1 上限全部用例 |
| 子进程 | 不 mock——bash 臂经 `sdk::bash::execute` 真实组合（短命 echo 类命令） | bash 臂分发用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/context.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/context_test.rs

#### 待测功能

- ContextDefense::resolve(): 窗长解析——`context_window` 缺席走 `DEFAULT_CONTEXT_WINDOW`（128 * 1024）；装配 L2 触发比 0.75 / L3 触发比 0.90 / 保护窗 40k / prune 单体门槛 20k
- estimate_history(): 各 rig Message 的 serde_json 序列化 UTF-8 字节数 ÷ 4 向上取整求和
- prune(): L2——老 tool_result（保护窗外且单体 > 20k 门槛）占位符替换（配对结构完整）→ 不够再丢最老完整轮对；保首条 user；产出 `DefenseNotice{subtype:"context_pruned"}`；未越水位零改动零 notice；孤儿配对清理
- hard_prune(): L3 失败降级用硬裁——保首条 user + 保护窗，中间整段丢弃，产出单条 DefenseNotice
- DefenseNotice: `{subtype: String, payload: Value}`——防线产出的 SystemNotice 前体

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| resolve 显式窗长承接 | 正向 | `resolve(Some(200_000))` → 窗长 200_000，L2/L3 触发阈值按比例推导（0.75 / 0.90） | 新增 |
| resolve 缺席走 128K 缺省 | 边界 | `resolve(None)` → 窗长 128 * 1024（缺省启发式，AC-9 消费半边） | 新增 |
| resolve 退化窗长不产生空史 | 边界 | `resolve(Some(0))` 等退化窗长 → prune/hard_prune 仍保首条 user + 保护窗（防线下界不塌缩为空史） | 新增 |
| estimate_history 空史为零 | 边界 | `&[]` → 0 | 新增 |
| estimate_history 字节启发式与手工口径一致 | 正向 | 单条已知消息 → serde_json 字节数 ÷ 4 向上取整；中文 / emoji 多字节计入（高估方向，宁早裁不触 L4） | 新增 |
| estimate_history 多条线性求和 | 边界 | 多条消息估算 = 各条之和（无跨条压缩或常数底） | 新增 |
| prune 老工具结果占位符替换且配对完整 | 正向 | 含超门槛老 tool_result 的超水位史 → 该 tool_result 替换为占位符文本，tool_use/tool_result 按 id 配对完整（无孤儿 tool_use / tool_result） | 新增 |
| prune 单体门槛恰边界 | 边界 | 老工具结果恰 20k tokens 估算 → 不裁；超 1 → 裁（恰阈值过 / 超 1 截断） | 新增 |
| prune 保护窗内不裁 | 边界 | 超门槛工具结果位于最近 40k 保护窗内 → 不裁（只裁窗外） | 新增 |
| prune 占位不够丢最老完整轮对 | 正向 | 占位替换后估算仍超水位 → 最老完整轮对（user+assistant+其工具对）整段移除；次老轮对保留 | 新增 |
| prune 首条 user 恒在场 | 边界 | 任意剪裁强度下 history 首条恒为原首条 user（phase agent 任务书保全） | 新增 |
| prune 孤儿配对清理 | 边界 | 轮对裁剪产生孤儿 tool_use / tool_result 半边 → 配对清理，无悬空 id | 新增 |
| prune 未越水位零改动零 notice | 边界 | 估算低于 L2 水位的史 → 原史原样返回、DefenseNotice 为空（每请求前调用点不扰动） | 新增 |
| prune 空史与仅首条 user | 边界 | 空史 / 仅一条 user → 零裁剪、零 notice、不 panic | 新增 |
| prune 产出 context_pruned notice 载荷形状 | 正向 | 触发剪裁 → `DefenseNotice{subtype:"context_pruned", payload:{before, after, layer:"l2"}}`，before/after 为估算 tokens 且 after < before | 新增 |
| hard_prune 中间整段丢弃保两头 | 正向 | 超水位长史 → 产物 = [首条 user, 保护窗内容]（中间整段移除）、配对完整 | 新增 |
| hard_prune 单条 notice | 边界 | 硬裁产出恰一条 DefenseNotice，payload 含 before/after/layer（降级路径由 loop 组合为 context_compacted+fallback 口径，见 loop 章节） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| （无） | 纯函数模块，无进程边界依赖；rig Message 以内存构造（serde_json / rig 构造器组装 user / assistant / tool 对），不需要 Mock | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/compact.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/compact_test.rs

#### 待测功能

- summarize(): L3——以当前 model 自摘要老历史（中文要点 prompt、禁工具、失败一次重试）；成功返回 `[摘要, 首条 user, 保护窗]` 新史（摘要为带 `[历史摘要]` 头的 user 消息置顶）；失败 `Err` 交 loop 降级

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| summarize 成功新史形态 | 正向 | 假模型返回摘要文本 → 新史 = [带 `[历史摘要]` 头的摘要 user 消息, 原首条 user, 最近轮对]（三段形态逐位断言） | 新增 |
| summarize 摘要请求禁工具 | 正向 | 捕获的摘要 CompletionRequest 不携工具定义面（禁工具口径） | 新增 |
| summarize 摘要 prompt 组装要点集 | 正向 | 捕获请求的 prompt 含「这是历史摘要，请勿重复已完成的工作」前缀形态与要点集关键词（已完成工作 / 决策及其原因 / 下一步——自研组装层断言，不逐字全量比对） | 新增 |
| summarize 失败一次重试 | 异常 | 假模型首次 `completion` Err、次次成功 → 恰发出 2 次摘要请求且返回成功新史 | 新增 |
| summarize 两次失败交降级 | 异常 | 假模型两次均 Err → `Err` 返回（run 不失败收敛的降级编排便归 loop 章节承载） | 新增 |
| summarize 空史输入 | 边界 | 空史（无首条 user 可保）→ 显式 `Err`，不 panic 不产空新史 | 新增 |
| summarize 空摘要文本 | 边界 | 假模型返回空文本摘要 → 空文本摘要消息按形态置顶不炸（或显式 Err，以实现定稿为准锁定其一） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| CompletionModel（入参例外——被测 API 显式入参） | 沿 loop_test FakeModel 同型内存假模型：捕获 `completion` 请求（prompt 组装 / 禁工具断言缝）、按预编程序列返回摘要文本或 `CompletionError`（重试 / 降级驱动） | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop_test.rs

<!-- design.md 公共函数 / API 表无 loop.rs 函数行（run() 为既有内部主体）；类型定义表声明 LoopTurn 增 defense 字段。
     修改文件行声明的编排职责（preamble 接入 / 每请求前 L2+L3 编排 / notice 转发）以下列用例承载。
     既有假流套件（一轮文本 / 两轮工具 / 重建史注入 / api_error / 停止 / 容量洪峰 / policy 拒绝 / 沙箱拦截）为 AC-1 路径迁移回归面：rig:: 路径下原样全绿，不逐条重列。 -->

#### 待测功能

- LoopTurn: 增 `defense: ContextDefense` 字段（引擎内部轮参数投影；runner 侧已解析缺省）
- 请求构造编排（修改文件行职责）：`preamble::load(&turn.cwd)` 每轮重读注入请求 preamble；每次发请求前 L2 prune + L3 触发/摘要/降级编排；防线 `DefenseNotice` 转发为 `SystemNotice` 密封事件

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| preamble 每轮重读接入请求（AGENT.md 文件 → preamble::load → loop 请求构造） | 正向 | tempdir root 写 AGENT.md + 两轮假流 → 首轮捕获请求 preamble 逐字含文件内容；轮间改写文件 → 次轮捕获请求 preamble 为改后内容（跨轮生效） | 新增 |
| preamble 缺席时请求 preamble 为 None | 边界 | root 无 AGENT.md → 捕获请求 preamble 为 `None`（不注入空串） | 新增 |
| L2 每请求前剪裁生效（loop 每请求前调用点） | 正向 | 小窗 defense + 超水位假史（工具轮回灌后）→ 捕获请求 chat_history 中老工具结果为占位符 + 事件流出现 `SystemNotice{context_pruned}` | 新增 |
| L3 水位触发摘要编排 | 正向 | 小窗 defense + L2 后仍超水位 → 假模型收到摘要请求，下一请求 chat_history = [摘要, 首条 user, 最近轮对] 形态 | 新增 |
| L3 失败降级硬裁且 run 不失败收敛 | 异常 | 假模型摘要请求两次 Err → 请求史按 hard_prune 硬裁 + 事件流出现 `SystemNotice{subtype:"context_compacted", payload:{..., layer:"l3", fallback:true}}` + 本轮仍以正常 `TurnDone` 收敛（不 api_error 不中断） | 新增 |
| 防线 notice 载荷形状与密封词汇 | 边界 | `context_pruned` / `context_compacted` 事件 payload 均含 before/after/layer 键；SystemNotice 为密封变体（delta 零落库纪律不破） | 新增 |
| 未触水位时请求前防线零扰动 | 边界 | 宽窗 defense + 低水位史 → 捕获请求史原样、零防线 notice（与既有用例事件序兼容） | 新增 |
| LoopTurn defense 字段装配 | 边界 | `LoopTurn` 完整构造锚定扩展：`defense` 字段必填（既有 loop_turn 装置扩字段）——构造点适配 | 新增 |
| loop_turn字段面完整构造锚定 | 边界 | 既有构造锚定断言随新字段失效——翻转扩 `defense` 字段面 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| CompletionModel（入参例外——被测 API 显式入参） | 沿既有 FakeModel 内存假流缝：逐轮预录 `RawStreamingChoice` 流项 + 捕获 `CompletionRequest`（preamble / chat_history 断言缝）+ 可编程 stream Err；摘要路径扩展 `completion` 臂预编程序列（成功文本 / 两次 Err 驱动降级） | 全部用例 |
| 文件（AGENT.md） | 不 mock——tempdir tempdir 真实文件，轮间真实改写 | preamble 每轮重读用例 |
| 内部模块（preamble / context / compact） | 不 mock——真实组合（最小 mock 原则：非进程边界、非注入依赖的内部模块必须真实组合） | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner_test.rs

#### 待测功能

- SdkRunner::new(): 增第三参 `context_window: Option<u64>`（承接组合根旁路的窗长载荷）；泵内装配 `ContextDefense` 并在 resume 重建后执行 L2 prune（notice 先于 `RunStarted` 转发）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| new 三参构造缺省 None | 边界 | `SdkRunner::new(config, resume, None)` → open/ask 行为与既有拒连端点套件完全一致（缺省防线装配不改变既有事件序） | 新增 |
| new 三参构造显式小窗 | 正向 | `SdkRunner::new(config, resume, Some(小窗))` → Continue + 携超门槛工具结果的转录 → 事件序为 `SystemNotice{context_pruned}` 先于 `RunStarted`（重建史防线留痕口径） | 新增 |
| store 转录全量不变半边 | 边界 | 携防线 Continue 时装载缝返回的全史事件仍全量进入重建（防线只作用请求史，装载缝无写通道、无反写） | 新增 |
| RunStarted.tools 恰七工具 | 边界 | ask 泵观察回流用例中 `RunStarted{tools}` 恰为七工具清单（与 TOOL_NAMES 同源） | 新增 |
| 既有 open 段与泵与门面一致性套件构造点适配 | 边界 | 既有 `SdkRunner::new` 两参构造点全部扩为三参（`None` 缺省）——断言语义零变化 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 转录装载缝（入参例外——注入依赖） | 沿既有 loader 闭包替身（内存三形态 Ok(Some 全史) / Ok(None) / Err + 调用计数），不 mock store | Continue 全部用例 |
| 网络 / provider 端点 | 不 mock——既有拒连端点 `http://127.0.0.1:9/v1` 装置（无网络触达） | ask 泵用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/normalize.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/normalize_test.rs

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更：本变更为纯 `rig_core::` → `rig::` use 路径迁移，流项归一化逻辑零改动（design 修改文件行）。
     章节仅承载路径迁移回归锚（来源：proposal 测试文件清单「loop_test.rs / normalize_test.rs / resume_test.rs：facade 路径迁移回归」，回溯 AC-1）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| stream_item 路径迁移回归 | 正向 | rig:: 路径下既有归一化断言集（text/reasoning 增量、完整块不出第二密封通道、tool_call/final 簿记不出事件、unknown Raw 透传、幂等）零改动全绿 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| （无） | 纯函数测试，rig 流项内存构造，无进程边界依赖（沿既有文件口径） | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/resume.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/resume_test.rs

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更：本变更为纯 `rig_core::` → `rig::` use 路径迁移，转录重建逻辑零改动（design 修改文件行；「resume 重建后 L2 剪裁调用点」落 runner.rs 泵内，见 runner 章节）。
     章节仅承载路径迁移回归锚（来源：proposal 测试文件清单同上行，回溯 AC-1）。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 转录重建路径迁移回归 | 正向 | rig:: 路径下既有重建断言集（三轮全史重建 / 链式续会话 / 多块单消息归并 / tool_result 成对回灌 / 非对话事件剔除 / 空史四形态 Err / 千级长转录）零改动全绿 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|----------|
| （无） | 纯函数测试，AgentEvent 内存 fixture 构造，无进程边界依赖（沿既有文件口径） | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/lib.rs -> packages/desktop/src-tauri/crates/infra/agent/src/lib_test.rs

#### 待测功能

- with_context_window(): builder 缝——`EngineFacade` 增 `context_window: Option<u64>` 载荷字段与链式 builder；`new()` / `with_resume_transcript` 缺省 `None` 不变；`runner_for` 签名不动
- EngineFacade: 增载荷字段后的门面（`runner_for` Sdk 臂传参 `SdkRunner::new` 三参）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| with_context_window builder 链式构造 | 正向 | `EngineFacade::new().with_context_window(Some(200_000))` → 构造成功且门面分发行为与直构 `SdkRunner::new(cfg, resume, Some(200_000))` 一致（open 校验错误面等价断言，沿既有门面分发一致性口径） | 新增 |
| 既有构造路径缺省 None 不变 | 边界 | `new()` / `with_resume_transcript(loader)` 产物 → `runner_for(Sdk)` 行为与现状一致（不携窗长即缺省防线，既有断言零漂移） | 新增 |
| crate根导出面锚定 | 边界 | 既有 crate 根导出锚定用例适配：`with_context_window` 经门面可达（不新增顶层 re-export 面） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 转录装载缝（入参例外——注入依赖） | 沿既有 counting_loader 闭包替身（调用计数） | builder / 分发一致性用例 |
| 网络 / provider 端点 | 不 mock——拒连端点装置 | Sdk 臂行为等价用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/compose.rs -> packages/desktop/src-tauri/crates/infra/agent/src/compose_test.rs

<!-- design.md 公共函数 / API 表无 compose.rs 函数行；类型定义表声明 ResolvedEngine 增 context_window: Option<u64>（组合根内部结构）。
     修改文件行职责：resolve_agent_engine 的 Sdk 臂读 provider.context_length → facade 构造表达式串接 with_context_window（AC-9 接线唯一 compose 触点）。 -->

#### 待测功能

- ResolvedEngine: 增 `context_window: Option<u64>` 字段——`resolve_agent_engine` Sdk 臂自 provider 记录透传（`None` 原样透传，不在组合根落 128K 缺省字面——缺省启发式归 `ContextDefense::resolve` 承载）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| resolve Sdk 臂透传 context_length | 正向 | seed provider 记录 `context_length = Some(200_000)` + 默认 sdk 实例 → `resolve_agent_engine` 产物 `context_window == Some(200_000)`（其余装配面不变） | 新增 |
| resolve provider 缺列透传 None | 边界 | provider 记录 `context_length = None`（含 V1 升级读入的旧记录）→ 产物 `context_window == None`（不落 128_000 字面） | 新增 |
| 既有解析套件回归 | 边界 | 既有快照承接 / 错误面 / 引用完整性 / continue 校验断言集零漂移（组合根其余行为不变式） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 文件 / DB（进程边界依赖白名单） | 不 mock——tempdir 真开 `WorkspaceStores` 全局库 seed 记录（沿既有 open_stores / seed_provider 装置） | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/store/src/model.rs -> packages/desktop/src-tauri/crates/infra/store/src/model_test.rs

#### 待测功能

- AgentProviderRecord::new(): 五参构造（增 `context_length: Option<u64>`；`None` = 未配置语义入存储层）
- AgentProviderRecord: 增可空 `context_length` 列；native_model `(id = 5, version = 2)` envelope 演进；手写遮蔽 `Debug` 字段清单补新字段
- AgentProviderRecordV1: `pub(crate)` legacy 结构（五字段，`version = 1`），`From` 双向 upgrade/downgrade 承接旧库读（升级读入 `context_length = None`）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| provider new 五参构造载荷保真 | 正向 | `new(name, base_url, api_key, models, Some(200_000))` → 五字段 + context_length 逐字段保真、id 置 0 | 新增 |
| provider构造new三字段载荷与三档models逐字段保真且id置0比较走partial_eq与字段面 | 边界 | 既有四参构造用例随五参签名失效——翻转：四参调用编译失败、五参 `None` 版承接字段面 / PartialEq 断言 | 废弃 |
| provider 嵌装往返 v2 含 context_length 双态 | 正向 | envelope encode/decode 往返：`context_length = Some(200_000)` 与 `None` 两形态逐字段相等 | 新增 |
| provider 记录编解码版本断言 2 | 边界 | 解码产物 native_model version 为 2（envelope 版本演进落位） | 新增 |
| 存量 V1 行经版本机制升级读入 | 正向 | 以 V1 legacy 形态编码的 payload → 库读升级为 v2 记录且 `context_length = None`（旧记录缺列读兼容，v3→v4 先例同型） | 新增 |
| provider记录嵌装往返含三档models逐字段相等 | 边界 | 既有往返用例随 version 演进适配 v2 口径（断言语义保持，版本断言位更新） | 废弃 |
| V1 From 双向 upgrade/downgrade | 边界 | `From<AgentProviderRecordV1>` 升级补 `context_length = None`；反向 downgrade 丢新字段无损（五字段原形还原） | 新增 |
| provider 遮蔽 Debug 补 context_length 位 | 边界 | 手写 Debug 输出含 context_length 字段位且 api_key 位仍为遮蔽形态（字段清单断言对齐，明文不进 Debug） | 新增 |
| provider serde 线格式 contextLength 键 | 边界 | serde camelCase 线格式含 `contextLength` 键；`null` 与缺席均解为 `None`（缺列读兼容的 serde 半边） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| （无） | 纯记录编解码测试：native_model envelope encode/decode 内存往返 + serde 线格式断言，无进程边界依赖（沿既有文件口径） | 全部用例 |

---

### packages/desktop/src-tauri/src/commands/agents/mod.rs -> packages/desktop/src-tauri/src/commands/agents/mod_test.rs

#### 待测功能

- save_agent_provider(): 增 `context_length: Option<u64>` 平参并传入记录构造（留空提交 = `None`，MUST NOT 落 0/128000 字面；api_key 留空保持原值语义不变）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| save 携 context_length 落库且重开一致 | 正向 | `save_agent_provider(..., Some(200_000))` 新建 → list / 读回 `context_length == Some(200_000)`；重开 stores 后一致（组合链扩字段） | 新增 |
| save 留空 None 不落字面 | 边界 | `save_agent_provider(..., None)` → 读回 `None`（库中无 0 / 128_000 字面） | 新增 |
| save 编辑态 context_length 双向翻转 | 边界 | 存量记录 `None → Some(N)` 与 `Some(N) → None` 两次编辑均按入参落库 | 新增 |
| save api_key 回填语义与新参正交 | 边界 | 编辑时 api_key 传空 + context_length 传 `Some(N)` → key 回填原值且 context_length 落 `Some(N)`（无回填语义，与 api_key 区分） | 新增 |
| 既有管理命令套件构造点适配 | 边界 | 既有 save 直调装置扩 `context_length` 实参；组合链 / 重名 / 删除 / 错误串遮蔽断言集零漂移 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 文件 / DB（进程边界依赖白名单） | 不 mock——`tauri::test::mock_app()` manage 真实 `WorkspaceStores`（tempdir 真开全局库），命令体经真实组合链执行（沿既有文件装置） | 全部用例 |

---

### packages/desktop/src/views/agents/hooks/use-agent-providers.ts -> packages/desktop/src/views/agents/hooks/use-agent-providers.test.ts

#### 待测功能

- AgentProviderSaveInput: 增 `contextLength: number | null`——save 调用透传至 `invoke('save_agent_provider', ...)` 入参

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| save 透传 contextLength 数字 | 正向 | `save({...contextLength: 200000})` → invoke 入参含 `contextLength: 200000` | 新增 |
| save 透传 contextLength null | 边界 | `save({...contextLength: null})` → invoke 入参 `contextLength: null`（不落 0） | 新增 |
| provider fixture 与库存回放适配 | 边界 | `provider()` fixture 补 `contextLength` 字段（bindings 类型演进后必填）；mockDispatch 库存往返保留该字段（list 回读保真） | 新增 |
| 既有取数 / 错误双轨套件回归 | 边界 | 挂载一次 / 动作轨刷新 / 错误双轨断言集零漂移（新字段不改变刷新与 toast 口径） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| IPC（进程边界依赖白名单） | 沿既有 `vi.mock('@tauri-apps/api/core')` invoke 按命令名分发替身（可切换 resolve/reject + 调用序列记录） | 全部用例 |
| toast（进程边界呈现面） | 沿既有 `sonner` toast.error `vi.fn` 替身 | 错误双轨回归用例 |

---

### packages/desktop/src/views/agents/components/provider-panel.tsx -> packages/desktop/src/views/agents/components/provider-panel.test.tsx

#### 待测功能

- ProviderFormState: 增 `contextLength: string`（空串 = 未配置，string 承载数字输入）；`PROVIDER_FIELDS` 增行；`toFormState` 映射（null → 空串）；保存边界 parse（空串 → null、非正整数拒绝提交）；「跟随缺省 128K」语义提示

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 新建表单填窗长提交 | 正向 | `provider-context-length` 输入 `200000` → `state.save` 入参 `contextLength: 200000` | 新增 |
| 留空提交为 null | 边界 | 字段留空 → `state.save` 入参 `contextLength: null`（空串 = 未配置） | 新增 |
| 编辑态回填 null 与数字 | 边界 | 存量记录 `contextLength: null` → 输入框空串；`200000` → 输入框 `"200000"`（toFormState 映射） | 新增 |
| 非正整数提交被拒 | 异常 | 输入 `0` / `-1` / `1.5` / `abc` → 保存被拦截（`state.save` 不被调用，按实现形态为禁用或校验拦截） | 新增 |
| 缺省语义提示在场 | 边界 | 表单渲染含「留空跟随缺省 128K」语义提示文案（PROVIDER_FIELDS 增行的提示位） | 新增 |
| 既有表单校验与呈现套件适配 | 边界 | 既有 name/base_url/api_key 校验、清单呈现、删除与错误呈现断言集零漂移（fixture 补 contextLength） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| state 动作（入参例外——被测组件显式入参） | 沿既有 `fakeState` 装置：`AgentProvidersState` fixture + `save` / `remove` `vi.fn` 可编程注入 | 全部用例 |
| 内部协作者（MaskedApiKey） | 不 mock——真实组合（沿既有口径） | 清单呈现用例 |

---

## 不可测试项

- AC-1 的 Cargo.toml 静态子句（facade 条目锁 `=0.42.x`、`default-features = false` + 显式 features `reqwest`/`native-tls`/`memory`、`regex = "1"` 条目、rig-core 解析钉子保留、agent-runtime crate 声明）与 `rig_core::` grep 零残留 — **原因**: 静态配置与源码文本守线，design 定口径「本设计守线仅静态、工作区全绿过关线由 test-execution 阶段核验」，非进程内断言；不为本守线立测试文件。
- AC-2 的零 diff 约束子句（`crates/core/agent` 与 `cli/` 零改动；preamble 机制不触 compose 面）— **原因**: 负向 diff 范围约束，由实现阶段 file_log / diff 静态核验；preamble 的可自动化行为半边已由 preamble_test / loop_test 章节承载。
- AC-9 的 bindings 再生成半边（`packages/desktop/src/types/generated/bindings.ts` 与 `src/bindings/mod_test.rs`）— **原因**: export-bindings 生成物（`pnpm bindings:export` 再生成），非手改非测试工作产物；本变更无新增 IPC 命令（`save_agent_provider` 已在 COMMAND_NAMES、`AgentProviderRecord` 已在 DTO_TYPES 在册），按 bindings 清单维护口径核实无需补录。
- AC-11 版本交付（`packages/desktop/package.json` version 0.4.7 → 0.4.8）— **原因**: 归档时一次性配置变更（任务列表交付阶段承载），无进程内可断言行为。
- AC-10 的 spec markdown 文本半边（desktop-agent-execution「SDK 租户 MVP 边界」bash 条款改写为「bash 无沙箱」知情边界）— **原因**: 提案阶段已写入 specs delta 的纯规格文本；可自动化的断言翻转半边已由 policy_test / tools_test 章节承载。
- bash 进程树清理的级联终止效果（`taskkill /T /F` 对孙进程的实际杀伤、unix `kill -9` 等价面）— **原因**: 真实跨平台进程树存活状态断言不可移植（孙进程孵化与存活检测环境依赖强、时序 flaky）；形状复制自 `cli/runner.rs` 既有实践，超时 Err 收口路径已由 bash_test 超时用例驱动（kill 调用被触达）。
- L4 极限场景兜底（首条 user 单块超窗 / 单轮即超窗；`prompt_too_large` 不细分、维持泛化 `api_error` 记因）— **原因**: 设计目标即不抵达的兜底线，需真实 provider 拒绝超大请求方可见；api_error 失败收敛路径由既有 loop_test 假流 Err 用例承载，「不细分」为不作为类约束无新增行为断言。
