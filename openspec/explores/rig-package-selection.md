# rig 包选型探索(rig-core 直连 vs rig 门面包)

> 状态:探索收敛(结论 = 维持 rig-core 直连),留痕供当前 change 与后续 change(MCP / memory / 上下文窗管理)引用
> 二轮(2026-09-30):原则修正后结论不变,rig vs rig-core 讨论对修正后三腿开放(见「原则修正」节)
> 日期:2026-09-30
> 关联 change:`agent-sdk-rig-tenant`(implement 26/35 期间触发)
> 探索触发:对比引入 `rig_core` 与 `rig` 包

## 事实核查(全部经 crates.io API + docs.rs 实测,非记忆)

0.42 线两包同版本号、同日(2026-08-17)lockstep 发版、同 monorepo(0xPlaygrounds/rig):

- **`rig` = 门面包**:lib.rs ≈ 265 行;唯一恒依赖 `rig-core ^0.42`(default-features = false);21 个 optional 家族 crate 全 feature-gated(rig-agent / rig-memory / rig-bedrock / rig-vertexai / rig-gemini-grpc / rig-candle / rig-fastembed / 各向量库 rig-lancedb / rig-qdrant 等 + rmcp)。`rig` 名下 0.36→0.42 共 10 版全在 2026 年(2019 年 `rig 0.1.0` 为无关注入包,已 yank);2–4 周一版,churn 不低。
- **`rig-core` = 实现体**:25 个恒依赖(reqwest 0.13 非 optional 等);providers(openai 在内)恒编译、无 per-provider feature;`memory` 模块只有 ConversationMemory trait + InMemoryConversationMemory(容器契约);**无 agent / multi_turn 模块**(0.42 已拆给 rig-agent)。
- **feature 转发核实**(rig 0.42 Cargo.toml):`reqwest = ["rig-core/reqwest"]` 纯转发;`native-tls` / `rustls` 转发 + 弱依赖(`?`)条件启用家族 crate;**`rig` 的 `default` = `rig-core/default` + `agent`(拉 rig-agent)+ derive + rustls——比 rig-core 的 default 更重**。
- **`rig-agent`**(facade 的 `agent` feature):「builder + run 状态机 + typed hook 系统(AgentHook / HookContext)+ 托管工具注册 + memory 编排」;**观察模型是 hook 回调不是事件流**,根级公共面未文档化:工具执行 hook 事件清单、文本/推理增量粒度、取消句柄。与本项目 core 的 `AgentRunner` trait 同名不同物。
- **`rig-memory`**(独立 crate,仅依赖 rig-core + tracing,可独立 `=` pin,不绑架 facade):`SlidingWindowMemory` / `TokenWindowMemory` / `HeuristicTokenCounter`(UTF-8 字节启发式)/ `DemotingPolicyMemory` / `CompactingMemory` + `TemplateCompactor`(文字 rollup,**明确不调 LLM**)/ 孤儿 tool-result 配对清理(裁掉配对 assistant tool_call 时丢孤儿 tool_result——provider 拒绝不成对消息,该坑已踩);`Compactor` trait 开放,**LLM 摘要 compaction rig 不提供**。

## 结论:维持 `rig-core` 直连(已实现形状,零变更)

1. **消费面零增量**:当前 15 处 `rig_core::` import(loop / normalize / resume / runner / tools 五文件)全落在 completion / message / streaming / client / providers::openai——100% 住 rig-core 本体,facade 只 re-export 到 `rig::` 经典路径。
2. **pin 落点更准**:`=` 纪律(0.x 无 semver 承诺)直接钉在实现体;走 facade 则 core 在 `^0.42` 内浮动(0.x caret 只许 0.42.x),要么 lockfile 兜底、要么维护 rig + rig-core 两条目。
3. **default 陷阱更少**:facade default 多拉 rig-agent + spike 已否决的 rustls 栈;现 pin(`default-features = false` + `["reqwest", "native-tls"]`)在 core 上不易配错。

**逃生门不欠债**:将来 MCP change 若吃 facade 的 `rmcp` feature(rmcp 2.x),或直连 rig-memory,均与 rig-core `=0.42.0` 直连共存(cargo 统一,0.42.0 满足 ^0.42),届时加 workspace 条目即可。

## 分层原则(拍板)

### P1:multi_turn——core 只持标记,infra 实现循环

契约层只持有「随时恢复原会话」的标记(session_id),循环实现归 infra。该原则已长在契约里:`AgentRunParams.resume_session_id`(crates/core/agent/src/runner.rs:71,注释原话「由实现方翻译为传输层形态」);cli 引擎解释为 `--resume` flag,sdk 引擎解释为 store 转录重建。**手搓 loop 不是权宜而是该原则的必然**;rig 0.42 的拆分(rig-core 持契约 / rig-agent 持运行时)与我们的分层同构——rig-agent 与 `sdk/loop.rs` 同海拔,不是「错过的依赖」。

### P2:memory——agent 特性、不强制,非协议义务

跨会话持久上下文(claude-code 式,如 CLAUDE.md / auto-memory)住在 agent 产品里,不住在协议里:CLI 租户的 memory 是 claude cli 自己的事,SDK 租户的 memory 将来是 sdk/ 引擎内部特性——同一特性两引擎各自实现,core 永远只认 session_id 标记。store 仍是唯一会话状态,记忆是「上下文注入」不是第二份会话。
**双义拆解**:rig 的 memory(rig-core ConversationMemory / rig-memory 历史整形)= 会话内窗管理 ≠ claude-code 式 memory(跨会话注入);前者归 loop 自管(chat_history 本就逐轮自组装),后者是将来引擎特性(loop 起播前读持久记忆注入 preamble,收敛后可选提炼写回)。

## rig-agent 适配评估(能≠值)

「AgentEvent 在 infra 层适配 rig-agent」机械可行(core 零污染守得住),但适配**性质**变化:

- 现路线:normalize 是无状态纯函数,翻译距离 ≈ 0(rig-core 流项 → 事件枚举),已实现。
- rig-agent 路线:变成「他们的 agent 状态机 → 我们的 claude-code 信封」的有状态翻译器——三块补丁(policy 拒绝的 `SystemNotice` 合成 / `RunResult` 字段对齐 / 取消句柄)贴在别人状态机外沿,观察粒度受制 hook 面(未实测、根文档不透明,本身是信号)。
- 经济账:loop.rs / runner.rs 已写完,替换是重写,收益 ≈ 0。

**结论:适配可行性从来不是问题,适配距离才是;手搓 loop 让它恒为零。**(hook 面细节若将来翻案需 spike,不凭记忆断言。)

## memory 成本账本(自建 vs rig 原生)

| 成本项 | 自建 | rig 原生(rig-memory 直连) | 判词 |
|---|---|---|---|
| 滑窗 / token 窗裁剪 | 纯函数 ~百行级,密集可测(贴测试纪律) | 现成,已处理孤儿配对 | rig 略省 |
| token 计数 | 字节启发式同款(真 tokenizer 在 provider 侧拿不到) | HeuristicTokenCounter 现成 | 平 |
| LLM 摘要 compaction | 自写(中途补 completion + 触发时机 / 注入格式 / 事件面语义) | **同样自写**(trait 开放,无现成) | 两边同价,rig 不省 |
| 接线 | loop 的 history 组装点直接改 | 插 ConversationMemory 契约(loop 不用该容器) | 自建略省 |
| 依赖足迹 | 0 | +1 轻 crate(仅依赖 rig-core + tracing) | ≈平 |

倾向:确定性裁剪将来直连 rig-memory(或抄算法含孤儿配对处理);LLM compaction 自写;跨会话注入自建薄层。三层均不需要 rig facade。

## 已知缺口:SDK 引擎无上下文窗管理(新发现,已裁 A 档)

手搓 loop 意味着窗管理是自己的账单:MVP 的 chat_history 逐轮追加(resume 转录 + 本轮)**无任何裁剪,长会话必然撞 provider 窗口上限**。与 memory 功能无关(不做跨会话 memory 也存在);claude CLI 内部有 auto-compaction,MVP 没有。explore 链(长会话)比调试页(短会话)先疼。

三档裁决:**A 纯留痕(采纳)**——缺口是缺席不是偏差,proposal AC 从未承诺,AC-9 真机验证为短会话不撞窗;记为已知限制 + 后续 change 首项素材(候选手段:rig-memory 直连裁剪 / 工具结果大小上限 / LLM compaction)。B 轻量护栏进 MVP(工具结果上限)被否——范围增项需回溯周期,与 26/35 只剩机械 v2 任务的现状不成比。C 完整窗口管理出 MVP,留独立 change。

## 对当前 change 的影响:零任务影响

剩余 9 项(阶段八 2 + 阶段九 4 + 阶段十 3)全是 v2 缺省语义修订,不碰依赖形状 / loop 内部 / memory:①包选型对比 = 对既成事实的理由升维;②memory 本就不在 MVP 任何阶段;③窗管理缺口走 A 档纯留痕。

design「rig-core spike 结论」一处文本微修留痕:multi_turn 托管 0.42 实际住 rig-agent(facade 的 agent feature)而非 rig-core——裁定结论(手搓)不受影响,任务不碰,按既有「文本差异留痕」模式处理(design 文本修不修待裁定)。

## 原则修正(二轮):适配目标恒为 core 协议,claude-code 非中转层

> 触发:上节「rig-agent 适配评估」中「他们的 agent 状态机 → **我们的 claude-code 信封**」的表述把适配目标错认成 claude-code 模拟,自我加重了适配距离。本节修正归因;结论(维持 rig-core 直连)经重估后不变,但后续 rig vs rig-core 讨论只应押在修正后的腿上。

### 三层模型

- **claude-code 线格式**:CLI 租户私有源格式,在 `cli/jsonl.rs` 被翻译为 core 协议(system/init→RunStarted、total_cost_usd→cost_usd、subtype 原样透传)
- **core 协议**:AgentEvent 五变体 + subtype 词典,自家所有、开放铸造;spec 已双约束——core「MUST NOT 认识 claude 与引擎」(specs/desktop-agent-execution/spec.md:13)、subtype「不做封闭枚举」(spec.md:45,174)
- **引擎原生观察面**:rig-core 流项(normalize 无状态直译,已实现)或假想 rig-agent hook 面

适配方向恒为「引擎原生 → core 协议」;claude-code 是词汇捐助者 + CLI 源格式,任何引擎不经它中转。协议不够用扩协议(自家的),不逼引擎演 claude-code 的戏。

### 结构已达标,残留只在词汇层

SDK 引擎无任何 claude-code 中间形态:normalize 直译、编排/状态机/落库/前端全租户共享。残留是 loop 合成事件借 CLI 词汇且注释框成「对齐」(sdk/loop.rs:43-44 `SUBTYPE_SUCCESS`「CLI 线格式口径」、:69「与 CLI init 对齐」、:325-327「CLI 合成收敛事件的命名口径」)。三处细节证词典实际已按 core 所有在运作:

1. `sandbox_denied` 为 SDK 自铸(CLI 无沙箱概念)——开放铸造已在发生
2. 借词未抄齐:event.rs 文档举 `permission_denial`(claude-code 拼法),loop 实铸 `permission_denied`
3. 归档 change explore 原话:信封「本就照 agent loop 形状设计」——出发点是通用 agent loop,claude-code 只是恰好也是

### 词典权威方向(待将来拍板,现状影响 ≈ 0)

现状方向倒置:subtype 值域的事实源头 = claude-code 发什么(jsonl 原样透传),core 词典「恰好」包含借词。正道:core 定义正典词典,CLI 翻译折叠进正典(claude-code 新词经透传容忍,不重定义正典)。语义重合处字符串本就相同,差别只在明天谁跟谁变。纯文档层善后:loop.rs 三处「CLI 口径」注释下次碰文件顺手改为「core 协议 subtype」(内部注释,不 bump)。

### 「三块补丁」归因重估

| 原判 | 重估 |
|---|---|
| SystemNotice 合成(policy 拒绝) | 拒绝出事件是协议层需求;claude-code 的只是命名习惯 |
| RunResult 字段对齐 | 普通协议映射,与 CLI 侧 total_cost_usd→cost_usd 同性质,非补丁 |
| 取消句柄 | 运行时管制,与协议/观察面均无关 |

三块全是 rig-agent **托管运行时**(占有工具执行/loop/取消)的成本,非 claude-code 模拟成本。观察面上真实存在的距离只剩:callback hook 模型 → 流式事件协议的有状态转换——这是 rig-agent 观察模型的成本,量到 core 协议为止。

### 对 rig vs rig-core 讨论的净影响(本节唯一目的)

修正后反对 rig-agent 路线的腿只剩三条,全部与 claude-code 无关:

1. **经济账**:loop/runner 已写完,替换是重写,收益 ≈ 0(不变)
2. **hook 面不透明**:根级公共面未文档化,事件粒度/取消句柄未实测(不变;翻案需 spike)
3. **托管工具执行 vs policy/sandbox 插值**:rig-agent 管工具执行,我方 policy 插值(拒绝 + 合成 ToolResult)需挂进他们的执行路径(不变)

若将来 spike 证实 hook 面粒度足够 + 托管执行可插 policy,观察面适配本身不再是障碍,争论收敛为纯经济账与托管管制贴合度。另注意两个正交性:①**选包(rig facade vs rig-core)与选运行时(rig-agent vs 手搓 loop)是两个正交问题**——facade 唯一增量是 feature 转发 + 家族 crate 门面,rig-agent 可不经 facade 直依赖(同 rig-memory 直连模式);②memory / 窗管理(候选 rig-memory 直连)与 rig-agent 无关,不构成 facade 加分项。

## 对话纪要

1. 触发:对比引入 rig_core 与 rig 包;核实两包形状(facade vs 实现体、feature 转发、版本史)→ 维持 rig-core(三条理由 + 逃生门)。
2. 拍板两原则:multi_turn core 持标记 / infra 实现循环(契约已长成,runner.rs:71);memory agent 特性不强制(含 rig 会话内整形 vs claude-code 跨会话注入的双义拆解)。
3. 实测 rig-agent(hook 观察模型、事件面不透明)与 rig-memory(无 LLM 摘要、孤儿配对已处理)→ 适配评估「能≠值」+ memory 账本;发现 MVP 无窗管理缺口。
4. 裁三档:A 纯留痕(零任务影响),剩余 9 项原样执行;留痕落独立 explore 文档(本文件)。
5. 原则修正(二轮):适配目标恒为 core 协议、claude-code 非中转层(词汇捐助者 + CLI 源格式);结构已达标(normalize 直译 + spec 双约束),残留仅 loop 注释「CLI 口径」措辞与词典权威方向(待拍板,影响 ≈ 0)。「三块补丁」重估为 rig-agent 托管运行时成本;rig vs rig-core 后续讨论只押三腿:经济账 / hook 面不透明 / 托管执行 vs policy 插值,另注意选包与选运行时正交。
