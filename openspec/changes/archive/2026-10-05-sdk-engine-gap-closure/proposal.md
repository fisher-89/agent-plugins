# 提案: sdk-engine-gap-closure

> **变更**: sdk-engine-gap-closure
> **日期**: 2026-10-04
> **状态**: draft

---

## 问题

SDK 引擎（rig 进程内租户）与 CLI 引擎在 core 协议层已完全对齐（`AgentEvent` 六变体 / 状态机 / write-through / 一轮一命泵契约，编排层零引擎分支），但 CLI 引擎整个白嫖 claude CLI 产品层（系统提示词、全工具面、MCP、auto-compaction、memory），SDK 引擎没有这层可白嫖——MVP 只建了裸 loop + 6 个文件工具（`preamble: None` 的裸模型，`sdk/loop.rs:92`）。**差距不在协议层，在「agent 产品层」**，具体缺口：

1. **裸模型**：系统提示词恒 `None`，无任何项目约定注入；CLI 引擎免费获得完整 coding-agent prompt + CLAUDE.md。
2. **工具面窄且质量低**：六工具（read/grep/glob/ls/write/edit）中 grep 为子串字面匹配（`sdk/tools.rs:180-197`）、read 无大小上限；**bash 缺席**（自 agent-sdk-rig-tenant 立项留痕、经 agent-core-session-kernel spec 快照延续至今的「最大刀刃移除」）。
3. **上下文无管理**：history 每轮全量 `clone()` 无限增长，`MAX_TURNS=50` 唯一熔断；重型 phase agent（phase prompt 数万 token + spec/design 全文 + repo 阅读）单轮可逼近 128K 窗，触顶即 api_error 失败收敛。CLI 引擎的 auto-compaction + prompt caching SDK 全无。
4. **窗长不可配**：provider 记录（`AgentProviderRecord`）无 `context_length` 字段，引擎无从得知真实窗长。
5. **依赖面**：rig-core 直连，rig 家族能力（rig-memory 等）将来需重新开洞；「维持 rig-core 直连」的旧探索结论已被拍板翻案。

---

## 提案

**一个 change 打包六项补全**（拍板 #5/#14），**facade 切换为第一阶段先行**（机械、可独立验证，后续全部工作落在新依赖上）：

1. **依赖切换**：`rig-core` 直连 → `rig` facade（rig-core 的 re-export 门面，唯一恒依赖 rig-core，家族 crate 全 feature-gated）+ 引入 rig-memory 算法层。锁 `=0.42.x` 平移、不升版本（切换与升级两变量分离）。
2. **AGENT.md preamble 注入**：起播前读 workspace root `AGENT.md`（严格无 CLAUDE.md 兜底，本仓库已复制 CLAUDE.md → AGENT.md），存在 → 逐字注入、缺席 → `None`；**每轮重读，改动下一轮生效**。引擎内部特性（P2 原则），`crates/core/agent` 零改动。
3. **工具质量**：read 截断 + offset 翻页 + 留痕；grep 正则匹配 + 上下文行（替换子串字面匹配）；全工具统一单结果字节上限（防线 L1 层）。
4. **bash 工具（第七工具）**：Default 档拒绝、AcceptEdits / Bypass 放行（AcceptEdits 与 Bypass **首次真实分化**）；`tokio::process` 执行，Windows git-bash 优先探测 + `cmd /C` 兜底、unix `sh -c`；`kill_on_drop` + `taskkill /T /F` 形状进程树清理；超时为活性护栏。**授权/沙箱/安全机制显式不进本期**（知情留痕：bypass 默认档下护栏为零）。
5. **L1-L3 上下文防线**（L4 = 现状 provider 报错兜底，设计目标永不抵达）：
   - **L1 单结果上限**：归工具质量项（read 截断 / 全工具字节上限 / bash 输出截断）。
   - **L2 请求前确定性剪裁**：prune 老工具结果（占位符替换，配对结构完整、保护最近窗口）→ 不够再丢最老完整轮对（rig-memory 算法 + 孤儿配对清理）；**保首条 user**（phase agent 任务书）；两个调用点：resume 转录重建之后 + loop 每次发请求前。
   - **L3 LLM compaction**：水位仍升 → 当前 model 自摘要老历史，新史 = `[摘要, 首条 user, 最近轮对]`（摘要要点含决策及其原因，参考 opencode）；**失败降级 L2 硬裁 + SystemNotice 留痕，不打断 run**。
   - **不变量**：store 转录**永远全量**，剪裁/压缩只作用于喂 provider 的请求史；剪裁/压缩发 `SystemNotice{subtype:"context_pruned"|"context_compacted"}`（开放词典）可观测。
   - **窗长来源**：provider 记录可空 `context_length` 列 + 缺省 128K 启发式；token 估算用 UTF-8 字节启发式（rig-memory `HeuristicTokenCounter` 同款）。
6. **provider `context_length` 可空列**：`AgentProviderRecord` 加列（store schema 演进沿既有 envelope 惯例）+ 管理命令面与管理页编辑（留空 = 未配置）。

spec delta 覆盖两个既有能力 spec：`desktop-agent-execution`（含「SDK 租户 MVP 边界」bash 条款的知情边界改写）与 `desktop-agent-management`（context_length 列）。

---

## 能力

### 新增能力

- 无——本变更全部落点为既有能力的深化（SDK 引擎内部特性 + 管理数据列），MUST NOT 新立能力 spec。

### 修改的能力

- **desktop-agent-execution** — SDK 引擎产品层补全：rig facade 依赖切换、AGENT.md preamble 注入、工具质量（read 截断翻页 / grep 正则上下文行）、bash 第七工具（三档分化 + 执行体）与「bash 无沙箱」知情边界改写、L1-L3 上下文防线及 `context_pruned` / `context_compacted` 可观测；「SDK 租户 MVP 边界」第 4 条「bash 缺席」偿还。
- **desktop-agent-management** — provider 记录加可空 `context_length` 列：存储模型、CRUD 命令面与管理页编辑字段，供 sdk 引擎窗防线消费（缺席走 128K 缺省启发式）。

---

## 变更范围

### 实现文件

以下均相对 `packages/desktop/src-tauri/`（除注明外）：

- `Cargo.toml`（workspace）：新增 `rig` facade 条目（`=0.42.x` 锁版、`default-features = false`、features `reqwest` + `native-tls` + memory feature）；`rig-core` 直连条目按 spike pin 策略处置（单条目则退役）
- `crates/infra/agent/src/sdk/`：loop.rs / normalize.rs / resume.rs / runner.rs / tools.rs / policy.rs / config.rs 的 `rig_core::` → `rig::` 路径迁移（约 15 处五文件）；loop.rs 三处「CLI 口径」注释顺手改为 core 协议口径（内部注释善后，不单独立项）
- `crates/infra/agent/src/sdk/`（新增）：AGENT.md preamble 读取（每轮重读）；bash 工具（进程执行 + shell 底座探测 + 超时 + 进程树清理，`taskkill /T /F` 形状复用 `cli/runner.rs` `kill_process_tree` 的形状、cli 文件零 diff）；上下文防线模块（L2 剪裁 + L3 compaction + token 估算 + 窗长解析）
- `crates/infra/agent/src/sdk/tools.rs`：read 截断 + offset 翻页、grep 正则 + 上下文行、全工具单结果字节上限、`TOOL_NAMES` 扩至七工具
- `crates/infra/agent/src/sdk/policy.rs`：bash 三档决策表（Default 拒 / AcceptEdits 与 Bypass 放）
- `crates/infra/store/src/model.rs`：`AgentProviderRecord` 加可空 `context_length` 列 + envelope 版本演进
- `src/commands/agents/`：provider CRUD 携带 `context_length`（留空 = None）
- `packages/desktop/src/views/agents/`：管理页 provider 表单增 `context_length` 数字字段（留空 = 未配置）
- `src/bindings/`：经 export-bindings 再生成（管理 DTO 演进），非手改
- `packages/desktop/package.json`：version 0.4.7 → 0.4.8（**归档时执行**，见 AC-11）

### 测试文件

- `sdk/policy_test.rs`：「bash 三档均拒绝」锁定断言翻转为 Default 拒 / AcceptEdits 与 Bypass 放的三档矩阵
- `sdk/tools_test.rs`：「bash 不在工具面」断言翻转；read 截断/翻页、grep 正则/上下文行、单结果字节上限边界用例
- `sdk/loop_test.rs` / `normalize_test.rs` / `resume_test.rs`：facade 路径迁移回归；resume 重建后 L2 剪裁调用点
- 新增：上下文防线模块测试（prune 占位符与配对完整 / 轮对裁剪 / 保首条 user / compaction 失败降级 / SystemNotice 载荷）、bash 执行体测试（档位 / 超时 / 进程树清理 / 兜底探测）、preamble 测试（在场逐字 / 缺席 None / 每轮重读）
- `crates/infra/store/src/model_test.rs`（及 envelope 侧）：`context_length` 列读写与缺省读兼容
- `src/commands/agents/mod_test.rs` 与管理页组件测试：CRUD 携带与表单编辑
- `cargo test --workspace` 全绿为 facade 切换阶段（第一阶段）过关线

### 删除文件

- workspace `Cargo.toml` 的 `rig-core` 直连 pin 条目（**条件性**：spike 裁定单条目形态时删除，保留双条目钉死则不删）
- `policy_test.rs` / `tools_test.rs` 中两条 bash 旧语义锁定断言（翻转为新语义，非删文件）

### 不要修改

- `crates/core/agent/**` — **零 diff**（AGENT.md 注入 / 防线 / bash 全为引擎内部特性；`AgentEvent` 词汇、`EngineConfig` 三字段、`AgentStartError` 变体集均不动）
- `crates/infra/agent/src/cli/**` — CLI 引擎零 diff（`kill_process_tree` 仅复用形状）
- `compose.rs` 的 `SessionInjections` 恒传 default —— injections 收窄 / 交互审批协议（`permission_request` + `agent_respond`）留独立 change（两引擎受益）
- store 转录落库语义 —— 全量不变，剪裁/压缩只作用请求史；「转录无上限增长」既有边界留痕不变
- MCP / 子代理 / 多模态（Image 忽略维持）/ thinking 请求参数 / cost 价格表 —— 均缓行（SDK 反超项：流式增量、路径沙箱、结构化 usage 亦不动）
- golden 线面契约（detail wire 冻结；本变更无 wire 字段演进，`SystemNotice` subtype 走开放词典）
- rig 版本升级（锁 `=0.42.x` 平移，升级走独立 change）
- bash 授权/沙箱/白名单机制（留后续 change）

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | workspace 依赖 rig facade 切换 | `cargo test --workspace` 全绿；`crates/infra/agent` 源码 `rig_core::` grep 零残留；facade 条目锁 `=0.42.x`、`default-features = false` + 显式 features |
| AC-2 | AGENT.md preamble 注入 | fixture：root 有 AGENT.md → 请求 preamble 逐字含其内容；缺席 → `None`；会话中改文件 → 下一轮请求生效；全程 `crates/core/agent` 与 compose 面零 diff |
| AC-3 | 工具质量（read / grep） | read 超限截断 + 尾部留痕 + offset 翻页取回后续窗口；grep 正则语义匹配 + 上下文行参数；边界用例（恰阈值过 / 超 1 截断） |
| AC-4 | bash 工具 | policy 三档矩阵逐格断言（Default 拒 / AcceptEdits 放 / Bypass 放）；进程执行 stdout/stderr 合并回灌 ToolResult；超时收口；终止/超时时进程树被清理；Windows 无 git-bash 环境走 `cmd /C` 兜底；`RunStarted.tools` 恰七工具 |
| AC-5 | L1 单结果上限 | 全工具单结果字节上限边界测试；bash 输出同归此层截断留痕 |
| AC-6 | L2 确定性剪裁 | prune 后老 tool_result 为占位符且 tool_use/tool_result 配对完整、最近窗口保护、首条 user 在场；不够再丢最老完整轮对；resume 重建后 + 每请求前两调用点可考；store 转录仍全量 |
| AC-7 | L3 compaction | 水位触发 → 新史 = [摘要, 首条 user, 最近轮对]；摘要调用失败 → 降级 L2 硬裁 + SystemNotice 留痕，run 不失败收敛 |
| AC-8 | 防线可观测 | `SystemNotice{subtype:"context_pruned"\|"context_compacted", payload:{before, after, layer}}` 流出并按密封事件落库；delta 零落库纪律不破 |
| AC-9 | provider `context_length` 列 | store 可空列读写（旧记录缺列读兼容）；管理页编辑（留空 = 未配置）；sdk 窗管理消费该值，缺席走 128K 启发式；bindings 再生成 |
| AC-10 | spec 边界留痕 | desktop-agent-execution「SDK 租户 MVP 边界」bash 条款改写为「bash 无沙箱」知情边界；policy_test / tools_test 两条锁定断言翻转归位 |
| AC-11 | 版本交付 | 归档时 `packages/desktop/package.json` version 0.4.7 → 0.4.8（用户可见变更：SDK 引擎能力补全） |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| rig facade re-export 面不完整 / memory feature 确切名称与路径未知 | 切换编译失败或 rig-memory 引入受阻 | 中 | design 前置 spike（不凭记忆断言）；拍板 #14 阶段一独立验证（`cargo test --workspace` 过关线）；失败退路 = 维持 rig-core 直连并拆分本 change |
| L3 compaction 行为退化 / 压缩循环（claude code 中途压缩退化、codex 重新引入致循环的教训） | agent 质量下降或 token 反复膨胀 | 中 | 摘要含「勿重复已完成工作」前缀形态；失败降级 L2 硬裁不打断 run；水位线、保护窗与摘要 prompt 归 design 定稿 |
| bash 护栏为零（BypassPermissions 默认档下模型可执行任意命令） | 本机自有 repo 场景误操作面扩大 | 高（知情接受） | 显式知情留痕（spec 边界条款，非安全缺口遗漏）；超时 + 进程树清理为活性护栏；沙箱/审批/白名单留后续 change |
| 窗长失准（中文重载启发式偏差 / 用户填错 context_length） | 过早裁剪丢密度或过晚裁剪触 L4 | 中 | 缺省 128K 启发式；L4 兜底现状 api_error 收敛不炸 run；`prompt_too_large` 错误细分提示填列（design 小项） |
| facade 锁版累积升级债（rig 约 2-4 周一版） | 后续单次升级跨度变大 | 低 | 拍板 #13 切换与升级两变量分离；升级独立 change |
| store 加列旧库读兼容 | 既有全局库 provider 记录读取失败 | 低 | 可空列 + serde 缺省读兼容；沿 envelope 版本演进既有惯例 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| SDK 引擎定位 | 守住「无 CLI 环境的轻量编码 agent」，change 工作流六工具面验证已覆盖各 phase agent 需求 | bash 二轮修订后进范围作 agent 通用能力保底；MCP/子代理仍缓行 | 自建完整 coding-agent 产品层（否，体量失控） |
| 系统提示词 | 自动抓取 AGENT.md（严格无 CLAUDE.md 兜底，本仓库已复制），每轮重读 | P2 原则：引擎特性、起播前读；语义最简可预期；bash 引入后 agent 可改自己的提示词，每轮重读使改动下一轮生效 | 自写完整 prompt / 起播快照 / CLAUDE.md 兜底 |
| bash 是否进范围 | 进（偿还 agent-sdk-rig-tenant 立项留痕）；授权/沙箱/安全显式暂不考虑 | 「bash 缺席」边界延续至今成最大刀刃缺口；知情留痕护栏为零 | 继续缓行（最大刀刃缺口持续暴露） |
| bash 三档归属 | Default 拒绝、AcceptEdits / Bypass 放行 | 现有三档语义对第七工具的自然延伸（只读档不给执行权）；AcceptEdits ≠ Bypass 首次真实分化 | 三档均放 / 均拒 / 新增第四档 |
| bash shell 底座 | Windows git-bash 优先探测 + `cmd /C` 兜底；unix `sh -c` | unix 语法对模型成功率最高；探测失败退 builtin 不损开箱即跑 | 仅 `cmd /C` / PowerShell |
| 窗长来源 | provider 记录可空 `context_length` 列 + 128K 缺省启发式 | 已知模型硬编码表维护必腐化（否决） | 硬编码模型表 / 每次探测 |
| 裁剪保头策略 | 保首条 user | phase agent 首条 user 即任务书（与通用 chat 裁剪本质区别） | 通用 chat 裁剪（可丢任务书） |
| 剪裁与 compaction 取舍 | 两者都要（L2 确定性 + L3 LLM 摘要），L2 用 rig-memory 算法、L3 自写 `Compactor` LLM 版 | 三家参考实现共识：先裁工具输出、LLM 摘要最后手段 | 仅 L2（密度不可恢复）/ 仅 L3（成本与失真） |
| 依赖策略 | rig-core → rig facade，锁 `=0.42.x` 平移不升版 | MCP（rmcp）/rig-memory 家族能力开洞；切换与升级两变量分离 | 维持直连（旧结论翻案）/ 升最新版 |
| 打包与阶段化 | 一个 change 打包六项；facade 切换为第一阶段先行 | 机械、可独立验证，后续工作全落新依赖上 | 拆多 change / 无阶段化 |

### 待决问题（归 design）

- rig memory feature 确切名称与 re-export 路径；现用 API 面在 facade 路径下全可达（spike 前置，不凭记忆断言）
- pin 策略：facade `=` pin 下 rig-core 单条目（cargo 统一解析）vs 保留双条目钉死
- 水位百分比值（L2 约 70-75%、L3 约 85-90% 区间定值）与 prune 保护窗大小（参考 opencode 40k/20k）
- 摘要 prompt 语言与要点集定稿；compaction 失败一次重试 vs 直接降级
- read 截断行数（约 2000 行）、全工具单结果字节上限、bash 超时缺省值（参照 claude code 120s 缺省 / 600s 上限）
- AGENT.md 自身体量上限（32KB 提及）与嵌套不级联确认；preamble 逐字不包装维持确认
- `context_length` 抵达 sdk 引擎的接线路径（`EngineConfig` 三字段不动承诺下：ctx 可扩展位 / 组合根旁路 / 引擎经既有 port 自查 provider 记录）
- L4 错误细分（`prompt_too_large` vs 泛化 `api_error`）；防线不覆盖的极限场景（首条 user 单块超窗 / 单轮即超窗）如实留 L4
- 上下文防线模块文件命名；loop.rs 三处 CLI 口径注释善后落点

---
