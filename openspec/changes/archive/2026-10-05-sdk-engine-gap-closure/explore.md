# SDK 引擎能力差距分析与补全路线(sdk vs cli)

> 状态:探索收敛(定位守住 + 一个 change 打包,待 phase-proposal 提升)
> 日期:2026-10-04
> 关联:吸收并取代 rig-package-selection(2026-09-30,原文件已移除;其「维持 rig-core 直连」结论被拍板 #6 翻案,残留原则见 §9)
> 探索触发:分析 `infra/agent` sdk 引擎对比 cli 引擎的差距,如何补全

## 核心结论

**差距不在协议层,在「agent 产品层」。** core 中立内核 / AgentEvent 六变体 / 状态机 / write-through / 一轮一命泵契约两引擎已完全对齐(编排层零引擎分支)。真正的差距是:CLI 引擎整个白嫖 claude CLI 产品层(系统提示词、全工具面、MCP、auto-compaction、memory、缓存、子代理),SDK 引擎没有这层可白嫖,MVP 只建了裸 loop + 6 个文件工具(`preamble: None` 的裸模型)。

## 1. 架构事实

```
                ┌─────────────────────────────────────────┐
                │        core/agent 中立内核(共享)         │
                │  AgentRunner trait · AgentEvent 六变体    │
                │  状态机 · write-through · 一轮一命泵契约   │
                └───────────────┬─────────────────────────┘
                                │ runner_for(cli|sdk) 唯一 match
                ┌───────────────┴───────────────┐
                ▼                               ▼
        ┌──────────────┐                ┌──────────────┐
        │ cli/ 引擎     │                │ sdk/ 引擎     │
        │ 薄壳:spawn +  │                │ 手搓 loop +   │
        │ jsonl 归一化  │                │ 6 个文件工具   │
        └──────┬───────┘                └──────┬───────┘
               ▼                               ▼
        ┌──────────────────┐          ┌──────────────────┐
        │ claude CLI 产品层 │          │   (无)           │
        │ 全工具面·MCP·     │          │  preamble: None  │
        │ compaction·      │          │  裸模型 + 6 工具  │
        │ system prompt…   │          │                  │
        └──────────────────┘          └──────────────────┘
```

CLI 引擎自身仅 ~500 行(spawn + flag 组装 + jsonl 解析);flag 面极简(`-p --output-format stream-json --verbose [permission档] [--resume <id>] --disallowedTools AskUserQuestion`),模型/MCP/skills/hooks 全部继承用户本机 CLI 配置。

## 2. 差距全景

### 2.1 SDK 落后 CLI 的(agent 产品层缺口)

| 维度 | CLI(产品层继承) | SDK 现状 | 严重度 |
|---|---|---|---|
| 系统提示词 | 完整 coding-agent prompt + CLAUDE.md | `preamble: None` 裸模型(loop.rs:92) | 🔴 |
| 工具面广度 | Bash/Read/Write/Edit/Glob/Grep/WebFetch/WebSearch/Task/TodoWrite/… | 6 个文件工具(read/grep/glob/ls/write/edit) | 🔴(定位守住后可缓) |
| bash | 有 | 无(spec 留痕「最大刀刃移除」) | 🔴 已进本 change(拍板 #8) |
| 工具质量 | ripgrep 正则/上下文行、read 截断、diff 级 edit | grep 子串字面匹配(tools.rs:180-197)、read 无大小上限、edit 精确串替换 | 🟡 必修 |
| MCP | 继承用户 CLI 配置(`system/init` 上报) | 恒空数组(loop.rs:62) | 🟡 缓行 |
| 上下文管理 | auto-compaction + prompt caching | history 每轮全量 `clone()`,无限增长;MAX_TURNS=50 唯一熔断 | 🔴 本 change 主体 |
| 子代理 | Task 工具 | 无,`parent_tool_use_id` 恒 None | 🟢 缓行 |
| memory | CLAUDE.md / auto-memory | 无 | 🟢 AGENT.md 档补 |
| 多模态 | 图片块 | Image 显式忽略(loop.rs:178) | 🟢 缓行 |
| cost | `total_cost_usd` 真实值 | 恒 None(无价格表) | 🟢 缓行 |
| resume 保真 | 原生 `--resume`(全上下文在 claude 侧) | 转录重建:Raw 丢、子代理压平、成本随历史线性涨 | 🟡 窗管理顺带缓解 |
| thinking 配置 | 有 | 无任何请求参数 | 🟢 缓行 |

### 2.2 SDK 反超 CLI 的(不对称,留档)

| 维度 | 说明 |
|---|---|
| 流式增量 | SDK 产 `MessageDelta`;CLI 禁做 `--include-partial-messages`,delta 恒空 |
| 路径沙箱 | CLI bypass 档(`--dangerously-skip-permissions`)完全无护栏;SDK bypass 至少有 canonicalize + root 前缀校验 |
| usage 结构化 | rig `Usage` 七字段(cached_input 等)vs CLI 裸 JSON |
| 引擎配置可控 | provider/model 显式配置管理 vs CLI 继承用户默认 |

### 2.3 两边共同缺的(headless 组装面选择,非引擎差距)

`SessionInjections`(preamble + tools 收窄)core 已预留但 `compose.rs:121` 恒传 default,两引擎都不消费;model、max-turns、env、allowedTools 正向白名单均不进参数面。交互审批协议(`permission_request` + `agent_respond`)spec 留独立 change(两引擎受益)。

**顺带发现**:SDK 的 `AcceptEdits` 与 `BypassPermissions` 行为完全相同(policy.rs:31-36 两臂一致)——今天非 bug(工具面太小无可分化);bash 档位已拍板(#9):Default 拒绝、AcceptEdits/Bypass 放行,两档**首次真实分化**;沙箱/审批类授权机制仍留后续 change。

## 3. 拍板记录(2026-10-04)

| # | 问题 | 拍板 |
|---|---|---|
| 1 | SDK 引擎定位 | **守住**「无 CLI 环境的轻量编码 agent」,能支持当前 change 执行需求即可;MCP/子代理仍缓行。**bash 二轮修订(2026-10-04):移入范围**,作为 agent 通用能力的保底(见拍板 #8) |
| 2 | 系统提示词 | **自动抓取 AGENT.md 即可**(不自写完整 coding-agent prompt):起播前读 `<workspace_root>/AGENT.md`,存在→preamble 逐字注入,缺席→None(现状不变) |
| 3 | 窗长来源 | **(c) provider 模型记录加可空 `context_length` 列 + 缺省 128K 启发式**(已知模型硬编码表否决) |
| 4 | 裁剪保头策略 | **保首条 user**(phase agent 首条 user 即任务书,与通用 chat 裁剪的本质区别) |
| 5 | 节奏 | **一个 change 打包**(AGENT.md + 工具质量 + L1-L3 窗管理 + facade 切换) |
| 6 | rig 依赖 | **rig-core 直连 → rig(facade)**(既有 explore 的逃生门转正门) |
| 7 | memory 机制 | **剪裁和 compaction 都要**,参考 opencode / codex / claude code 实现,在 rig-memory 基础上补充 |
| 8 | bash 工具 | **进范围**(「bash 缺席」边界自 agent-sdk-rig-tenant 立项、经 agent-core-session-kernel spec 快照延续至今,本 change 偿还);**授权/沙箱/安全暂不考虑**(显式知情留痕:bypass 默认档下 bash 即模型可执行任意命令、护栏为零,沙箱/审批/白名单留后续 change) |
| 9 | bash 三档归属(复审 A1) | **Default 档拒绝、AcceptEdits / Bypass 放行**——现有三档语义对第七工具的自然延伸(只读档不给执行权),非新授权机制;AcceptEdits ≠ Bypass 首次真实分化 |
| 10 | bash shell 底座(复审 A2) | **git-bash 优先探测 + `cmd /C` 兜底**(unix 语法对模型成功率最高;探测失败退 builtin 不损「开箱即跑」);unix 侧 `sh -c` |
| 11 | AGENT.md 读取范围(复审 A3) | **严格只读 AGENT.md,无 CLAUDE.md 兜底**;本仓库以 CLAUDE.md 内容复制出 AGENT.md 一份(已执行) |
| 12 | AGENT.md 会话中修改(复审 A4) | **每轮重读,改动下一轮生效**(不做起播快照)——bash 引入后 agent 可改自己的系统提示词,此语义行为最简且可预期 |
| 13 | facade 版本策略(复审 A5) | **锁 `=0.42.x` 平移,不升版本**——切换与升级两个变量分离(rig 2-4 周一版,churn 高) |
| 14 | change 内阶段化(复审 A6) | **维持一个 change;facade 切换为第一阶段先行**(机械、可独立验证,后续全部工作落在新依赖上) |

### 定位守住的验证:change 工作流工具需求

| 阶段 agent | 干的活 | 需要的工具 | 6 工具面覆盖 |
|---|---|---|---|
| implementation-generator | 按 design/tasks 写实现代码 | read/write/edit/glob/grep | ✓ |
| 各 evaluator | 静态清单评估、读代码比对 | read/grep/glob | ✓ |
| test-gen-generator | 写测试文件 | read/write/edit | ✓ |
| test-execution | 跑测试(走 dev-team CLI,非 agent) | — | ✓(无关) |
| code-review | 读 diff/代码 | read/grep/glob | ✓ |

change 工作流结构上不需要 agent 侧 bash(测试/构建走 CLI 或 infra)。真正咬执行质量的缺口收窄为:系统提示词、工具质量、窗管理(phase prompt 数万 token + spec/design 全文 + repo 阅读,重型 executor 单轮可逼近 128K 窗)。

## 4. 参考实现调研(opencode / codex / claude code)

调研日期 2026-10-04,来源见文末。

| 维度 | Claude Code | Codex CLI | OpenCode |
|---|---|---|---|
| 触发 | ~95% 容量(实测偏晚) | token 阈值(180k/244k)+ 95% 有效窗 | `tokens > context − output` |
| 保留策略 | 仅摘要,摘要开新会话 | 摘要 + 最近用户消息 ~20k | 摘要 + 追加 Continue 用户消息 |
| 工具结果 | microcompact:清空老 tool_result 内容、保留 tool_use 块(API 配对完整);四层分级 Snip→Micro→Collapse→Auto | 未述(摘要前缀告知「工具状态可访问」) | prune 独立机制:保护最近 40k,可裁部分 >20k 才裁更早工具输出 |
| 摘要要点 | 完成了什么/文件/下一步 | 进度/决策/约束/待办 + 摘要前缀(「勿重复已完成工作」) | 最完整:含关键技术决策**及其原因** |
| 失败处理 | — | 指数退避重试 | 环境变量可关闭 |
| 教训 | 中途自动压缩致行为退化;多次压缩累积失真 | 重新引入内容可致压缩循环 | — |

**共识模式**:工具结果是上下文大头 → 先裁工具输出、再动对话结构、LLM 摘要是最后手段;保最近、丢中间,任务书要么进摘要要么显式保留。

## 5. 合成设计:四层上下文防线

```
水位 ──────────────────────────────────────────────▶
        L1          L2              L3            L4
     单结果上限   请求前确定性剪裁    LLM compaction  provider 报错
     (恒常)      (~70-75% 水位)     (~85-90% 水位)   (finish_error,
                                                  设计目标:永不走到)
```

- **L1 单结果上限**:read 缺省截断(如 2000 行 + 「已截断,可 offset 翻页」留痕);全工具统一单结果字节上限。grep/glob 已有 200 条上限,真正缺口是 read;bash 引入后 stdout/stderr 合并截断同归此层
- **L2 确定性剪裁**(请求前跑,两个调用点:resume rebuild 之后 + loop 每次发请求前):先 prune 老工具结果(老 tool_result 内容替换为占位符,配对结构完整,保护最近窗口;opencode/claude code 同款);不够再丢最老完整轮对(rig-memory 孤儿配对清理兜底)。保首条 user
- **L3 LLM compaction**:水位仍升 → 当前 model 自摘要老历史,新史 = `[摘要, 首条 user, 最近轮对]`;摘要 prompt 按 opencode 要点集(含决策及原因);**失败降级到 L2 硬裁 + SystemNotice 留痕,不打断 run**(比 codex 指数退避保守:phase agent 场景宁可丢密度不能丢收敛)
- **L4** = 现状 api_error 收敛,防线目标是永不抵达

**防线不覆盖的极限场景(留痕防误解)**:①首条 user 单块自身超窗(巨型 phase prompt + 小窗模型,如 32K)——L1-L3 全失效,归 L4 兜底;错误细分(`prompt_too_large` vs 泛化 `api_error`,提示用户填 provider `context_length`)为 design 小项;②单轮即超窗(无多轮累积)——L2 轮对裁剪不适用,仅剩 L1 + L3 两道。

### 配套裁定

| 项 | 形状 |
|---|---|
| 窗长来源 | provider 模型记录可空 `context_length` 列,缺省 128K 启发式 |
| token 估算 | rig-memory `HeuristicTokenCounter`(UTF-8 字节启发式,中文重载比英文经验值准) |
| L2 容器 | 用 rig-memory `TokenWindowMemory`/`SlidingWindowMemory` 的**算法**(含孤儿配对),不用其容器契约——loop 自己拼 `Vec<Message>`(手搓 loop 是 P1 原则必然) |
| L3 实现位 | 实现 rig-memory 开放的 `Compactor` trait 的 LLM 版(其自带 `TemplateCompactor` 仅文字 rollup 不调 LLM,LLM 版即「在 rig-memory 基础上补充」的缺口) |
| 可观测性 | 剪裁/压缩发 `SystemNotice{subtype:"context_pruned"|"context_compacted", payload:{before,after,layer}}`(subtype 开放词典现成) |
| **不变量** | **store 转录永远全量**;剪裁/压缩只作用于喂 provider 的请求史(重放/审计/resume 重建源不受影响) |
| AGENT.md | 起播前读 root 单份,逐字注入;**严格只读 AGENT.md 无 CLAUDE.md 兜底**(拍板 #11,本仓库已复制 CLAUDE.md→AGENT.md);**每轮重读,改动下一轮生效**(拍板 #12);缺省决策(逐字不包装/自身 32KB 上限/不级联嵌套)design 定 |

## 6. rig-core → rig(facade)切换

facade 事实核查(源自已移除的 rig-package-selection):facade 唯一恒依赖 `rig-core ^0.42`(default-features=false),21 个家族 crate 全 feature-gated(rig-memory、rmcp 均在内)。切换形状:

- workspace 依赖 `rig = "=0.42.x"`、`default-features = false`、features 开 `reqwest` + `native-tls` + memory feature;**锁 0.42.x 平移、不升版本**(拍板 #13,切换与升级两变量分离)
- 代码面:loop/normalize/resume/runner/tools 五文件 ~15 处 `rig_core::` → `rig::`(纯 re-export,经典路径同名)
- **design 前置 spike(不凭记忆断言)**:memory feature 确切名称与 re-export 路径;现用 API 面在 facade 路径下全可达;pin 策略(facade `=` pin 下 rig-core 经 cargo 统一解析,是否保留双条目钉死)
- 红利:将来 MCP 直接开 rmcp feature

## 7. change 范围清单(一个 change)

1. **AGENT.md preamble 注入**(sdk 内部,零 core 改动;座位由 P2 原则锁定:引擎特性、起播前读)
2. **工具质量**:read 截断+翻页、grep 正则+上下文行
3. **L1-L3 上下文防线**(prune / 剪裁 / compaction + SystemNotice 可观测)
4. **provider 模型记录 `context_length` 可空列**(store schema 加列 + 管理页)
5. **依赖切换 rig-core → rig(facade)+ rig-memory 引入**
6. **bash 工具**(工具面第七工具,拍板 #8/#9/#10;授权/沙箱/安全暂不考虑,非安全面设计点归 design):
   - 档位:Default 拒绝、AcceptEdits / Bypass 放行(policy 决策表更新,AcceptEdits≠Bypass 分化落地)
   - 进程执行:`tokio::process`,cwd = workspace root,env 继承;shell 底座:Windows git-bash 优先探测 + `cmd /C` 兜底,unix `sh -c`(拍板 #10)
   - 停止语义:`kill_on_drop(true)` + Windows 进程树清理复用 `cli/runner.rs` `kill_process_tree` 的 `taskkill /T /F` 形状(future drop 不会自动杀子进程)
   - 超时:缺省(参照 claude code 120s 缺省 / 600s 上限)——活性护栏,非安全护栏
   - 输出截断归 L1;`RunStarted.tools` 报七工具
   - spec 修订点:「bash 工具 MUST NOT 进 MVP 工具面」条款、SDK 租户 MVP 边界第 4 条(bash 缺席)偿还;policy bash 三档拒绝的两条锁定测试翻转(policy_test.rs「bash工具名三档均拒绝」、tools_test.rs「bash不在工具面」)

**阶段化(拍板 #14)**:facade 切换为第一阶段先行——机械、可独立验证(`cargo test --workspace` 全绿即过关),后续 AGENT.md / 工具质量 / L1-L3 / bash 全部落在新依赖上。

**spec delta 覆盖两个能力 spec(proposal 勿漏)**:`desktop-agent-execution`(bash 边界条款显式重写——「路径沙箱是唯一护栏、必过关卡」与 MVP 边界「bash 缺席」在 bash 裸奔进范围后自相矛盾,须按既有 MVP 边界留痕模式把「bash 无沙箱、护栏为零」写成知情边界)+ `desktop-agent-management`(provider models 加 `context_length` 可空列)。

design 阶段小决策:水位百分比值、prune 保护窗大小(参考 opencode 40k/20k)、摘要 prompt 语言、compaction 失败一次重试还是直接降级。

## 8. 附:关键代码事实定位

- CLI flag 面:`crates/infra/agent/src/cli/flags.rs:24-49`;禁用 flag 负向断言 `flags_test.rs:338-353`
- CLI jsonl 归一化(六类顶层 type):`cli/jsonl.rs:14-20`;thinking_tokens 丢弃 `:49-51`;cost 取 `total_cost_usd` `:168-185`
- CLI 不产 MessageDelta(`--include-partial-messages` 禁做)
- SDK 裸请求(全参数 None):`sdk/loop.rs:90-102`;MAX_TURNS=50 `loop.rs:15`;MCP 恒空 `loop.rs:62`;Image 忽略 `loop.rs:178`
- SDK 工具面:`sdk/tools.rs:26`(六工具,MAX_GLOB/MAX_GREP_RESULTS=200 `:19,22`);grep 子串匹配 `:180-197`
- SDK 权限三档:`sdk/policy.rs:29-39`(AcceptEdits 与 Bypass 两臂一致)
- SDK 沙箱:`sdk/sandbox.rs:62-81`(canonicalize_lenient + root 前缀校验)
- resume 重建:`sdk/resume.rs:11-48`(仅 Message 进史,子代理压平 `:25-27`)
- injections 恒空:`compose.rs:121`;跨引擎续会话拒绝 `compose.rs:97-99`
- provider 三档模型:`desktop-agent-management` spec L13(models high/medium/low,无窗长字段)
- rig-core pin:`=0.42.0` + `default-features=false` + `["reqwest","native-tls"]`(workspace Cargo.toml:91-93)

## 9. 吸收自 rig-package-selection(2026-10-04 移除原文件)

> 原探索(2026-09-30,二轮)中已被本文件覆盖的部分:核心结论「维持 rig-core 直连」被拍板 #6(facade 切换)翻案;窗管理 A 档留痕被 §5 四层防线偿还;memory 成本账本裁决(裁剪用算法/compaction 自写)被 §5 L2/L3 吸收。以下为仍有效且未在前文重述的部分,原文件据此移除。

### P1/P2 分层原则(维持有效)

- **P1 multi_turn**:core 契约只持「随时恢复原会话」的标记(session_id),循环实现归 infra——CLI 解释为 `--resume` flag,SDK 解释为 store 转录重建(契约原话「由实现方翻译为传输层形态」)。手搓 loop 是该原则的必然,rig-agent 与 `sdk/loop.rs` 同海拔,不是「错过的依赖」。
- **P2 memory**:跨会话持久上下文是 agent 产品特性、非协议义务;同一特性两引擎各自实现,core 永远只认 session_id 标记,store 仍是唯一会话状态。双义拆解:rig 的 memory(会话内窗管理)≠ claude-code 式 memory(跨会话注入);前者归 loop 自管(本文件 §5),后者是引擎特性(本文件 AGENT.md 即其最小实现)。

### rig-agent 适配评估(能≠值,维持有效)

反对 rig-agent 托管运行时的三腿:①经济账(loop/runner 已写完,替换是重写,收益 ≈ 0);②hook 面不透明(观察模型是 hook 回调非事件流,工具执行 hook 事件清单/文本与推理增量粒度/取消句柄未实测,根级公共面未文档化);③托管工具执行 vs 我方 policy/sandbox 插值(拒绝 + 合成 ToolResult 需挂进他们的执行路径)。正交性:选包(facade vs core)与选运行时(rig-agent vs 手搓)是两个正交问题——拍板 #6 只动前者。翻案条件:spike 证实 hook 面粒度足够 + 托管执行可插 policy,则争论收敛为纯经济账。

### 原则修正(二轮,维持有效):适配目标恒为 core 协议,claude-code 非中转层

- 三层模型:claude-code 线格式(CLI 租户私有源格式,`cli/jsonl.rs` 翻译)/ core 协议(`AgentEvent` 五变体 + 开放 subtype 词典,自家所有、开放铸造)/ 引擎原生观察面(rig 流项 normalize 无状态直译)。适配方向恒为「引擎原生 → core 协议」,claude-code 只是词汇捐助者 + CLI 源格式,任何引擎不经它中转。
- 残留仅词汇层:`sdk/loop.rs` 三处「CLI 口径」注释(SUBTYPE_SUCCESS「CLI 线格式口径」/「与 CLI init 对齐」/「CLI 合成收敛事件的命名口径」)待改为「core 协议 subtype」——本 change 大改 loop.rs,顺手善后候选(内部注释,不单独立项)。
- 词典权威方向(core 定义正典词典、CLI 翻译折叠进正典,claude-code 新词经透传容忍)仍待将来拍板,现状影响 ≈ 0。

### facade feature 转发事实(§6 spike 背景)

rig 0.42 的 `reqwest` feature 纯转发 `rig-core/reqwest`;`native-tls`/`rustls` 转发 + 弱依赖(`?`)条件启用家族 crate;facade 的 `default` 比 rig-core 的 default 更重(多拉 rig-agent + derive + rustls)——故切换必须 `default-features = false` + 显式 features。

## 调研来源

- [Context Compaction Research: Claude Code, Codex CLI, OpenCode(badlogic gist)](https://gist.github.com/badlogic/cd2ef65b0697c4dbe2d13fbecb0a0a5f)
- [Context Compaction in Codex, Claude Code, and OpenCode(justin3go)](https://justin3go.com/en/posts/2026/04/09-context-compaction-in-codex-claude-code-and-opencode)
- [OpenCode Compaction 官方文档](https://opencode.ai/v2/docs/compaction)
- [Investigating How Codex Context Compaction Works](https://kangwooklee.com/blogs/codex_context_compaction.html)
- [Codex Experimental Context Management(OpenAI Community)](https://community.openai.com/t/experimental-context-management-compaction-in-codex/1395578)
- [Inside Claude Code: Context Compaction](https://y-agent.github.io)

## 对话纪要

1. 双引擎深读(cli/ 4 文件 + sdk/ 9 文件)→ 差距全景:协议层已对齐,缺口全在 agent 产品层;SDK 反超项(流式增量/路径沙箱/结构化 usage)留档
2. 拍板定位:守住轻量编码 agent(change 工作流 6 工具面验证覆盖);bash/MCP/子代理缓行
3. 拍板系统提示词:自动抓取 AGENT.md(座位 = P2 原则的引擎内部特性)
4. 窗管理三档展开 + 增长解剖(单结果/run 内累积/resume 重建再增长)→ 拍板:窗长来源 (c)、保首条 user、剪裁+compaction 都要
5. 三家参考实现调研(opencode/codex/claude code)→ 四层防线合成(L1 caps / L2 prune+轮对裁剪 / L3 LLM compaction / L4 兜底永不抵达)
6. 拍板 rig facade 切换 + 一个 change 打包;范围清单 5 项定稿
7. 吸收 rig-package-selection 残留价值(P1/P2 原则、rig-agent 评估三腿、原则修正二轮、feature 转发事实)进 §9 后移除原文件——其核心结论已被拍板 #6 翻案,保留会形成两个收敛状态互相矛盾的探索文件
8. bash 二轮修订:进 change 范围作 agent 通用能力保底(偿还 agent-sdk-rig-tenant 立项、经 agent-core-session-kernel spec 快照延续的「bash 缺席」边界);授权/沙箱/安全显式暂不考虑(知情留痕:bypass 默认档下护栏为零),沙箱/审批/白名单留后续 change
9. 复审拍板 A1-A6:bash 档位(Default 拒 / AcceptEdits+Bypass 放,两档首次分化)、shell 底座(git-bash 探测 + cmd /C 兜底)、AGENT.md 严格无兜底(本仓库复制 CLAUDE.md→AGENT.md)、每轮重读改动下轮生效、facade 锁 0.42.x 平移不升版、维持一个 change 且 facade 先行阶段化;防线极限场景(首条 user 单块超窗 / 单轮即超窗)与 spec 自洽性(delta 覆盖 desktop-agent-execution + desktop-agent-management 两 spec、bash 边界条款显式重写)留痕

## 拆分与否的复审(2026-10-04,proposal 过评后)

**结论:维持一个 change**(用户三度确认)。分析要点:依赖图两条真实耦合链(facade→rig-memory→L2/L3 共享水位与测试基建;L1 截断约定↔bash 共享 policy 决策表),自然切缝只切得出「上下文 vs 工具面 + facade 小块」,而 facade 小块单独走 8 阶段的行政开销可能超过实现量。不预先拆的核心理由:①backtrack 是工作流内建的事后缩范围正道;②facade 风险已被 2026-09-30 事实核查压至中低;③拍板 #14 阶段化已获「facade 独立先行 change」方案约七成收益。备选方案②(facade 独立 change)在「看重 git 历史单独留痕 / 对 spike 更不安」时更优,拆分最便宜时点在 dev-design 开启前——已过,不再翻案。

**拆分触发器(design 阶段落 tasks.md 前言)**:

- **T1(facade spike 失败)**:memory feature 名不符 / re-export 面缺现用 API → 当场 backtrack proposal 剔除 facade 项,降级二选一:rig-memory 直连(不走 facade)或内抄算法(含孤儿配对)
- **T2(compaction 深坑)**:test-execution 阶段两段假流编排失控 / 降级语义反复拖住交付 → backtrack 缩范围,compaction 剔除留独立后续 change,L1/L2 先行交付
