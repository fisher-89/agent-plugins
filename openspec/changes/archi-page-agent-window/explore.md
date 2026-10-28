# 架构页通用 agent 窗口：注入什么工具、什么知识

workflow_type: requirement

## 0. 诉求原话与拆解

> 架构页设置通用 agent 窗口，注入生成/维护架构需要的工具和知识。

三个词各自都要落地：

| 词 | 现实含义 | 现状 |
|---|---|---|
| 架构页 | `/architecture` 路由，挂载点 A（`mode = "full"` 全量只读画布） | 只有画布 + 刷新按钮，零会话入口 |
| 通用 agent 窗口 | 自由对话（非相位会话），能开新会话 / 切历史 | 已有两处先例：change 详情左栏 chat、explore 详情页 |
| 工具 + 知识 | ① `archi_*` 专用工具面 ② 模型格式 / 纪律的知识注入 | ① 只在 change 相位会话按矩阵下发 ② 唯一知识通道是 workspace `AGENT.md` |

## 1. 现状全景：会话来源 → 注入面

`SessionProvenance { source, source_ref }` 是自由字符串对，装配单点在
`crates/infra/agent/src/compose.rs:23 assemble_injections`：

```
source             source_ref                      tools 位              toolset 位
──────────────────────────────────────────────────────────────────────────────────
debug（缺省）      任意                             None（七工具）         None
explore            explore 记录 id                  [read,grep,glob,ls,
                                                   edit,bash]            Explore
change（流程相位）  {cid}/{phase}/{role}/{attempt}   None（七工具）         Archi{按 7 行矩阵}
change（详情 chat） <change id>（两段）              None（七工具）         None   ← 解析失败 fail-closed
archi（本次要定义） ?                                ?                     ?
```

要点：
- 矩阵命中判据是 `source_ref` **尾三段**（`change_phase_role`，compose.rs:56）。两段的
  change chat 天然拿不到 archi 工具 —— 架构页若要工具，必须新开一个来源词或新定式。
- archi 两工具已归**只读面**（`sdk/policy.rs:4 READONLY_TOOLS`），三档全放行。
- 工具执行体 `sdk/archi_tool.rs` 的上下文 root = 会话 cwd，所以「cwd 指哪，模型读哪」。
- 清单命令面已有通用 `agent_sessions(root, source, source_ref, offset, limit)`
  （`src/commands/exec/mod.rs:241`），change chat 另加了薄包装
  `list_change_chat_sessions`（同文件 :303，source 词常量收在命令层）。

## 2. 核心张力一：写权落点（架构页=只读铁律 vs 「维护架构」要写）

`desktop-archi-canvas` 把 `full` 钉成只读：「改 gate 档 ❌ 只读视图，呈现『改档需要开一个
change』提示」；`desktop-archi-agent` 把「模型 = 源码、只能经 change worktree 改」也钉死了。
架构页 agent 一旦拿到 `write` / `edit`（全七工具），这两条同时被破。

```
A 只读顾问        cwd=主仓   tools=[read,grep,glob,ls] + Archi{1,1}   零写
                  产出：解释 / 盘点 / 归属判断 / 「该在哪加模块」建议
                  出口：建议文本（可 promote 成 explore → change，沿 explore 既有通路）

B 直写主仓        cwd=主仓   七工具 + Archi{1,1}
                  ⟵ 破两条既有 requirement；共享主仓模型，写坏＝该 workspace 全部架构视图停摆
                  ⟵ 无隔离层、无人工确认回合（归档确认那套护栏全不在场）

C 代理进 change   cwd=<选定 change 的 worktree>  七工具 + Archi{1,1}
                  ⟵ 与 change 详情左栏 chat 重叠；价值只剩「从架构页直接挑 change 进入维护」
```

折中形态也值得考虑：**会话档位由用户显式选**（沿 `agent-input.tsx` 已有的
default / acceptEdits / bypassPermissions 三档选择器）——架构页默认档 = 只读顾问，
切到 acceptEdits 才可写。代价是把「架构纪律」降级成「用户自觉」。

## 3. 核心张力二：知识从哪来（单一事实源纪律）

`desktop-agent-execution:281` 明写：系统提示词唯一来源是 workspace root 的 `AGENT.md`
（每轮重读、32KB、无 fallback），`SessionInjections.preamble` **维持不消费**。

```
通道                              现状                              架构页可用性
AGENT.md（cwd 每轮读）            唯一系统提示词来源                全局一份，非架构专用
SessionInjections.preamble        不消费（spec 两次强调）           要首次激活＝契约变更
core/workflow prompts/*.md        16 份 include_str! 静态角色知识    挂在相位 step 上，非相位会话无入口
config.json 的 context / rules    已解析、已出线到 config 页，未被消费  现成可用但语义泛（"全局规则"）
仓内领域文档（如 <archi_dir>/AGENTS.md）  不存在                   人可编辑、随 change 演进、git 可追溯
工具 description 自带知识         最小                               容量小
```

若选「产品内置静态知识」（`include_str!`，与相位 prompt 同风格），最大风险是**与
`desktop-archi-model` / `dto.rs` 词表漂移**。仓库已有的对冲手法：`phase_table_test` 里
「16 条 include_str! 与盘面 md 逐字对拍」+ 从 `dto` 常量反向断言（kind 四值 / gate 三值 /
诊断九类）。这条测试风格可以原样搬。

若选「仓内领域文档」，冷启动（模型为空、文档为空）无知识可注入，需要静态兜底 —— 于是
大概率落在「静态骨架 + 仓内约定叠加」的合成形态。

## 4. 工具缺口：现在这两个工具够「生成」吗？

| 能力 | 工具现状 | 缺口 |
|---|---|---|
| 定位（已知模块） | `archi_module(element)` ✅ | — |
| 发现（不知归谁管） | `archi_search(query, scope)` ✅ | — |
| 全量盘点 | `archi_module()` 薄概览（fqn/kind/gate/summary） | 概览**不含诊断**：agent 无法确认「我改完模型是否仍合法」 |
| 结构校验 | 无 | 缺 `archi_check`（全量结构诊断九类 + 文档警告）＝画布停摆态的数据同源 | 
| 我改了什么 | `archi::diff` 纯函数在，命令面只给归档确认 `archi_change_summary` | agent 无 diff 工具（bash + git 勉强可代） |
| 骨架怎么写 | 无 | 无示例 / 无模板，靠知识注入里的 TOML 片段 |
| 源码 → 元素归属 | read/grep/glob/ls | 够用（架构工具刻意不吐正文，见 explore §5.1） |

一个值得警惕的反向问题：`archi_check` 会不会变成「第二个校验插口」？现有纪律是「写入不
校验、读取报错、三处校验（读取 / 门禁 / 归档）」——`archi_check` 属"读取面"，同源不破纪律。
真正要防的是「让 agent 写盘前先校验」的诱惑。

## 5. 前端形态：复用度与新增面

```
views/changes/agent/change-chat-panel.tsx (216 行)   头区动作｜状态提示｜历史清单｜composer
        │  AgentSessionView（注入形态）+ useAgentSession
        │  归属 source="change", source_ref=<change id>
        │
        └── 架构页若照抄一份 → 第二套 chat 面板（与 desktop-change-chat 的「单一实现」纪律擦边）
            更可能是：抽公共「会话面板」（头区/历史/状态行）→ change 与 archi 各传归属参数
```

架构页布局当前是「标题 + 刷新 + 画布」，放面板的形态也有分支：
- 左栏 `ResizablePanelGroup`（与 change 详情 / explore 详情同构）→ 画布变窄
- 右侧可折叠抽屉 → 与详情页 `DetailDrawer` 同构，画布不被长期挤压
- 与画布联动的候选增强：**选中元素即上下文**（抽屉选中 `App.Auth` → 发问时带上 FQN，
  甚至工具默认 `element`），这是架构页独有、change chat 给不了的价值

## 6. 待定问题（本探索要收敛的）

1. 写权：A / B / C，还是「档位自选」？（最影响 spec 真伪的一条）
2. 来源词与归属：新 `source = "archi"` + `source_ref = null`（一个 workspace 一份历史）？
   还是 `source_ref = <root>` / `<archi_dir>`？
3. 装配落点：`assemble_injections` 加一支 archi 臂（与 explore 臂同形，最省），还是把
   「来源 → 工具面」矩阵数据化（因为已经三支特权来源了）？
4. 知识通道：preamble 首激活 / 静态 md 走第二条 System 消息 / 仓内领域文档 / 组合？
5. 工具面是否补 `archi_check`（+ 是否补 diff）？
6. 面板是否抽公共组件（动 change chat 会牵 `desktop-change-chat` spec）？
7. 与 change 详情「架构」页签 + 左栏 chat 的关系：架构页 agent 会不会成为更弱的重复品？
   它必须回答「哪些问题只能在架构页问」——目前答案偏向：全量视角的盘点 / 归属判断 /
   跨 change 的架构决策讨论。

---

# 第二轮：三条裁定落地后的影响面

用户裁定（2026-xx-xx）：

1. **写权 = B**：架构页 agent 有写权，直改主仓模型；prompt 里提示「改完检查语法」。
2. **知识**：初版注入必要 prompt，尽量简短；担心漂移的片段从**代码同源动态拼接**。
3. **用途三条**：① 初始工程补架构（冷启动）② 架构演进预修改（查代码偏差）③ 直接修复架构图 / 文档错误。

## 8. B 方案的 spec 冲突面（必须改的既有 requirement）

| spec | 现有约束（逐字） | B 方案下的处置 |
|---|---|---|
| `desktop-archi-agent`「模型 = 源码（写纪律与不设专用写工具）」 | 「**冷启动同样走 change**」+「MUST NOT 存在 workspace 级直接改模型的通路」+「三类使用场景同源（全走 change dev-design）」 | 改写为**两条通路**：change worktree（原路，人工确认回合）／架构页通用会话（新路，直改主仓，护栏 = prompt 纪律 + git diff 可回滚）。「不设专用写工具」保留（仍用 `write` / `edit`） |
| `desktop-archi-agent`「SessionToolset::Archi 与 (phase, role) 注入矩阵」 | 矩阵 7 行按 `source_ref` 尾三段解析；畸形 fail-closed 零 archi 工具 | 新增 **archi 来源臂**（不走 phase 解析）：`source = "archi"` → `Archi { module: true, search: true }` + `tools = None`（七工具全量）。explore 臂 / change 臂逐字不动 |
| `desktop-agent-execution`「会话级工具面注入与 explore 笔记工具」 | 装配消费面 = explore 臂 + change 臂 + 其余 default | 消费面扩一支；`preamble` 位由「维持不消费」改为**首激活**（见 §9） |
| `desktop-agent-execution`「SDK 引擎系统提示词（AGENT.md 注入）」 | 「`SessionInjections.preamble` 位维持不消费」两处强调 | 改为：系统消息 = 产品知识（preamble 位）＋ `AGENT.md`（逐字）合并为**一条** System 消息 |
| `desktop-archi-canvas`「能力差异（全量只读 / 增量可改档）」 | `full` 模式「MUST NOT 呈现任何可提交的 gate 编辑入口」 | **保留**（画布 UI 仍然零写入口），但补一句消歧：只读的是**人类入口**；agent 写面在另一种会话里，不是画布能力 |
| `desktop-write-protection` | 「`<archi_dir>/models/` 与架构域配置 MUST NOT 进入内置锁面」 | **零改动**——这条正好是 B 方案的前提（否则 agent 改不了模型） |
| 新增 `desktop-archi-chat`（建议名） | — | 架构页会话面板：骨架 / 归属 `source = "archi"` / 历史清单 / 轮收口刷新画布 / 权限档位 |

一句话：**画布仍然只读，写权只在会话里**。这是 B 方案能同时满足「`full` 只读」与「能维护」的唯一措辞。

## 9. 知识注入：通道选型与同源拼接

### 9.1 通道选型

```
候选                                     现状                          判定
AGENT.md（cwd 每轮读，唯一 System 源）    已在用                        不动（它是用户工作区约定）
SessionInjections.preamble               字段全链路透传、sdk 零消费      ◀ 首激活（改动≈一行 + spec 措辞）
core/workflow prompts/*.md               相位 step 专用                不适用于非相位会话
include_str! 新静态 md                    可行但纯静态 → 会与 dto 漂移    作为「必静态骨架」载体
config.json 的 context / rules           已解析已出线但未被消费          不用（语义泛，且会引入第二知识源）
仓内 <archi_dir>/AGENTS.md                不存在                        不推荐做初版（冷启动无内容，需静态兜底→两条路）
```

选定形态：**静态骨架（`include_str!` 少量文字）＋ 代码派生片段（`archi::dto` / `foundation::layout`）在装配侧拼成一条 preamble 文本** → 塞 `SessionInjections.preamble` → sdk loop 与 `AGENT.md` 合并成一条 System 消息。

```
compose_turn（infra/agent/src/compose.rs）
  │  source == "archi" ?
  │    └─ archi_knowledge() : String      ← 纯函数，可单测
  │         ├ include_str!("archi-knowledge.md")   静态骨架（纪律 / 排版约定 / 写面边界）
  │         ├ ElementKind::ALL → vocabulary(..)     动态：kind 四值
  │         ├ GateTier::ALL + default() → vocabulary 动态：gate 三值 + 缺省
  │         ├ DiagnosticKind::ALL → vocabulary（需补 ALL）  动态：九类诊断词
  │         └ layout 相对路径（需补访问器）          动态：models / decisions 目录
  ▼
SessionInjections { preamble: Some(知识), tools: None, toolset: Some(Archi{true,true}) }
  ▼
sdk/loop.rs:195
  let mut history_system = merge(preamble::load(cwd), injections.preamble)
  chat_history.push(Message::system(history_system))   ← 单条（rig 侧只发一条，避免多 System 语义不明）
```

### 9.2 同源拼接的原料清单（可派生 / 半派生 / 必静态）

```
可派生（零漂移）
  kind 四值            ElementKind::ALL                       ✅ 已有（tools.rs 描述就在用）
  gate 三值 + 缺省      GateTier::ALL / GateTier::default()     ✅ 已有
  诊断九类词            DiagnosticKind                         ⚠ 枚举有，ALL 需新增
  架构目录相对路径       foundation::layout                     ⚠ 只有绝对 PathBuf 访问器（archi_dir / archi_models_dir /
                                                              archi_decisions_dir），prompt 要相对段 → 需补相对形态派生
  工具名 / 入参          ARCHI_MODULE_TOOL / ARCHI_SEARCH_TOOL   ✅ 已有

半派生（先导出私有 serde 字段名）
  element / relation 字段名  kind / gate / path / docs / summary / metadata / from / to / desc
  └ 权威在 parse/toml.rs 的私有 RawDocument / RawElement / RawRelation（#[serde(deny_unknown_fields)]）
    但一行都取不到 → 需新增 pub const 字段名清单（格式骨架靠它拼，漂移即编译期红）

必静态（代码里没有单点，只能写文字）
  「一元素一表头；键 = 带引号的完整 FQN 单键（[element."App.Auth"]），禁点分」   ← TOML 键形约定
  「metadata 用内联表，禁子表（子表会被读成元素 X.metadata）」                ← 与子元素表头同构的陷阱
  「层级由 FQN 最长前缀推导，不用 TOML 表嵌套」
  「新元素 gate 缺省 off（ratchet：逐块收紧）」
  「不写接口 / 导出符号，契约权威在 openspec/specs，只经 docs 引用」           ← 知情边界
  「改完立即复查；只改模型与关联文档；保留注释与既有排版」                        ← 纪律
```

最后一个必静态项里有一条**只能靠文字**的理由：`#[serde(deny_unknown_fields)]` 能拒绝 `gatte`，但拒绝不了「用错了形态」（比如 `metadata` 写成子表——它合法 TOML，只是被读成另一个元素）。

### 9.3 首版知识草案（约 12 行，`<>` 处为拼装位）

```
【架构模型】<archi_dir>/models/*.toml，按文件名序聚合；每元素一个表头。
【格式】[element."App.Auth"] kind=<package|domain|module|component>
        gate=<off|warn|strict>（缺省 off） path=["..."] docs=["..."]
        summary="..." metadata={ owner = "..." }   ← 内联表，禁 [element."X".metadata] 子表
        [[relation]] from="A" to="B" desc="..."
【层级】由 FQN 最长前缀推导（App.Auth.Token ⊂ App.Auth）；子元素须先声明祖先，否则 orphanHierarchy。
【自检】改完立即调用 archi_module（不带 element）：模型结构坏时它返回诊断清单
        （<syntaxError|missingField|invalidValue|unknownField|duplicateElement|cycle|dependsOnAncestor|unknownReference|orphanHierarchy>），
        好时返回薄概览。诊断文案自带 file:line，按行修。
【边界】模型里不写接口 / 导出符号（契约在 openspec/specs，只经 docs 引用）；
        新增元素 gate 缺省 off；改档只改 gate 一行，保留注释与其余排版。
【写面】直接改主仓文件（无 worktree、无归档确认）；改动随 git diff 评审。
        只改 <archi_dir>/ 下模型与 docs 关联的文档；改源码须用户明确要求。
```

拼装位检查：kind / gate / 诊断九类 / 目录路径可派生；其余为必静态骨架文字。

## 10. 「检查语法」的三条实现路径

```
a 纯 prompt + 复查 archi_module()       零工具变更
     模型坏 → 三投影（module_detail / module_overview / search）全 Err，带
              「模型结构错（共 N 条）：kind @ file:line」逐行清单
     模型好 → 静默成功（无「已校验 ✓」信号，靠 agent 自觉）
   ⟵ 与既有纪律「写入不校验、读取报错」完全同源；变更面 = 一段 prompt 文字

b 新增 archi_check 工具                  词汇变更
     成功返回 []（明确的"已校验"信号），失败返回诊断数组（与画布停摆态同源）
   ⟵ 代价：SessionToolset::Archi 是两位布尔（module / search），加第三位须
     同改 core 词汇 + infra definitions + 全量对拍语料；spec 已明写「无第三位」

c 写盘后引擎自动校验并回灌               ❌ 不推荐
   ⟵ 在 loop 里新增第三个校验插口，破坏「校验只发生在读取 / 门禁 / 归档三处」
```

首版倾向 a（用户已说「prompt 提示需要检查语法」），b 作为「agent 老是不自检」时的升级项。

## 11. 三类用途 → 缺口清单

| 用途 | 现有工具够不够 | 缺口 |
|---|---|---|
| ① 初始工程补架构（空模型冷启动） | `write` 有 `create_dir_all(parent)`（tools.rs:551）→ 可直接建 `<archi_dir>/models/`；`archi_module()` 空模型返回空概览；`read`/`grep`/`glob`/`ls` 可摸源码 | 无「源码结构一览」类工具（可让 agent 用 `ls`/`glob` 逐层；工作区代码量统计已有命令面 `desktop-workspace-code-stats`，但不在 agent 工具面）。建议 prompt 加一句「先 ls 顶层，再逐域细化，一次一个文件」 |
| ② 演进预修改（查代码偏差） | **本轮不做**（用户裁定：只是场景举例，不实现） | 漂移判定归 `archi-check-and-gate`（未落地）。本轮 agent 仍可用 `read`/`grep` 自行抽查，但无专用工具、也无 spec 义务 |
| ③ 直接修图 / 文档错误 | `archi_module` 给 `modelFile` + `modelLine` → `edit` 直达；`docs` 关联路径是仓内 md，可 `write`/`edit` | 无（模型坏时 `archi_module` 的 Err 已带 `file:line`，正是"修图错误"的入口） |

## 12. 归属、清单与刷新接线

```
归属     source = "archi"，source_ref = null（一个 workspace 一份历史）
         cwd = null → 后端回落 root（= store root，双 root 不变量天然满足）
清单     前端已有先例直调 agentSessions(root, 'archi', null, ...)
         （views/agent/hooks/use-agent-run-history.ts:107）
         └ 若坚持 change chat 那条「前端零来源词汇」纪律 → 加 list_archi_chat_sessions 薄包装
刷新     零 watch 纪律 ⇒ agent 写盘后画布不会自己变；**手动刷新按钮是必需项**（用户裁定）
         /architecture 已有 `archi-refresh`（architecture-view.tsx:23）：setRefreshTick →
         <ArchitectureCanvas key={tick} …> 重挂 → useArchiGraph 的 useEffect 重新 invoke
         archi_graph。链路完整，零新代码。
         ⟵ 会话收口自动 refresh 属**可选增强，本轮不做**（页面必须有人工刷新入口即可）。
         spec 需落一句：agent 写盘后画布不自动更新，须用户点刷新才可见（零 watch 的必然
         推论，避免被当成 bug 报）。prompt 亦应提醒 agent「改完请用户点刷新」。
```

### 12.1 手动刷新恰好补上「检查语法」的失败兜底

```
agent 写模型  ──▶ ① agent 侧自检：archi_module() 无参 → 坏则 Err「模型结构错（共 N 条）：kind @ file:line」→ 自修
                   ② 用户侧校验：点刷新 → archi_graph 若仍有诊断 → StructureErrorPanel 逐条 file:line 红字
                     （diagnostics 非空即全或无停摆，零降级样式）
```

于是 §10 的 a 路径（纯 prompt 自检）安全性不依赖 agent 自觉：**最坏情况用户一刷新就暴露**，且暴露形态与 agent 看到的是同一份诊断数据（`archi::dto` 单点投影）。这也让「新增 `archi_check` 工具」的紧迫性进一步下降。


## 13. 面板复用：抽公共骨架，两处换一处不换

### 13.1 三个"面板"其实不是同类

```
explore 详情左栏                      change 详情左栏                    /architecture（新）
──────────────────────────           ────────────────────────         ──────────────────
AgentSessionView                     会话管理骨架                     会话管理骨架
  + ExploreComposer                   头行：新会话 / 历史 ▾            （同 change）
                                      状态行：禁用理由 / 草案文案
一记录 ↔ 一会话（幂等）               历史清单：分页 / 加载更多         一 workspace ↔ 多会话
无新会话、无历史、无折叠               折叠窄条（分栏产物）              无折叠（或抽屉）
```

explore 的"面板"**没有会话管理维度**——会话由 `ensure_explore_session` 幂等绑定到记录，
本身就只有 `AgentSessionView` + composer 薄适配，早已在公共层。抽出来的骨架对它没有
可复用件，硬套只会带进「恒空的清单 / 恒禁的新会话」哑参数。

### 13.2 裁定：抽骨架，change chat 这次就换，explore 不动

新组件 `components/agent/session-panel.tsx`（经 `components/agent/index.tsx` 桶导出）：

```
入参
  state: SessionPanelState     ← 结构化状态（下）
  labels: { title, draftNote, promptLabel, placeholder }
  testIdPrefix: string         ← 保住 change-chat-* / archi-chat-* 两套 testid
  composer: (actions) => ReactNode   ← 沿 AgentSessionView 的插槽范式透传
  collapsed?: boolean                ← 缺省 false（塌陷分支 = 窄条）
  onToggleCollapsed?: () => void     ← 缺省无（不呈展开入口）
渲染
  collapsed ? 窄条（竖向标题 + 展开入口）
            : 头行动作（标题 + 新会话 + 历史 ▾） → 状态行（note） → 历史清单（分页） → AgentSessionView（注入形态）

SessionPanelState（骨架只认这个接口 = ChangeChatState 的公共子集）
  session: UseAgentSessionState        sessions / total
  listOpen / listLoading / listError   toggleList / loadMore
  newSession / selectSession           canStart
  note: string | null                  ← 状态行文案（null = 不占位）
```

**`CollapsedRail` 改为纳入骨架**（修正上一轮判断）：架构页左栏也做可折叠（画布是横向
内容，要能全宽查看），于是折叠窄条出现**第二个消费点**（change 详情 + 架构页）——
按「不为单用例抽抽象」的反向逻辑，两处都用即该抽。`collapsed` / `onToggleCollapsed`
做成可选入参：change 详情传入（其真相来自分栏 `onResize` 回传），架构页同样传入，
explore 不传（它本就不可折叠，行为零变化）。

两侧的差异全部收敛到「各自 hook 供一个 `note`」：

```
change：note = disabledReason ??（currentSessionId === null ? DRAFT_NOTE : null）
        （这段现就在 ExpandedBody 里，平移进 useChangeChat 即可）
archi ：note = currentSessionId === null ? '发送首条消息即开启新会话' : null
        （或再加一句「改完模型请点刷新」——§12 的提醒可以落在状态行，不占 prompt）
```

**为什么这次就换 change chat（而不是下轮）：**

1. 只给 archi 用 = 为单用例抽抽象（本仓 AGENT.md 明令禁止），且抽象未经第二实现检验，
   等将来换时大概率要重抽；
2. change chat 的测试是**行为级**（`change-chat-panel.test.tsx` 675 行全走 panel 的
   testid 与交互断言），抽骨架不改可观察行为 → 测试应原样通过，回归面可控；
3. 抽完两处永久同步演进（新会话 / 历史 / 状态行 / 加载更多 / 折叠窄条只改一处）。

**留在各自侧、不抽的三件：**

| 不抽 | 理由 |
|---|---|
| 判据单源（`canSend` / `disabledReason` / `runActive` / `archived`） | change 的业务判据；archi 侧另一套（几乎恒可发，仅运行中禁发） |
| 来源词与清单命令 | change 走 `list_change_chat_sessions`；archi 见 §13.3 |
| `useAgentSession` 的建立 | 注入形态不变——消费者各自建 hook，骨架不建第二条流（这条纪律零变化） |


**spec 影响面**：`desktop-change-chat` 的「对话体单一实现」requirement 描述的是行为
（对话与轮行经 `AgentSessionView`，MUST NOT 自拼），抽骨架不破；只需在它的 Module Contract
表补一行新组件，并把「面板骨架归一、change 与 archi 共用」写进 purpose/contract。
**新增 spec**（`desktop-archi-chat`）则反向引用同一骨架。

### 13.3 附带决策：archi 的清单命令面

change 走专用薄包装（`source` 词常量收在命令层），前端零来源词汇。archi 两个选择：

```
a) 加 list_archi_chat_sessions(root, offset, limit)      与 change 对称，+20 行命令 + 1 个 mod_test
b) 前端直调 agent_sessions(root, 'archi', null, …)      零后端改动（use-agent-run-history.ts:107 有先例）
```

推荐 **a**：既然骨架都已归一，命令面也对称更一致，且让 `use-archi-chat` 与
`use-change-chat` 结构完全同形（便于将来再提取清单取数 hook）。代价是一个薄包装。

### 13.4 历史清单在架构页到底解决什么（以及它的风险）

先看 change chat 的历史清单**实际做了什么**（代码事实）：

```
打开面板          sessionId = null（空态，不自动恢复最近一条）
                  ⟹ 旧会话没有别的入口 —— 清单是唯一回退路径
点历史 ▾          toggleList → reload() → list_change_chat_sessions(offset 0)
点某条            setSessionId(id) → useAgentSession 以 sessionId **直查**
                  session_detail + agent_session_transcript 重放整段对话
之后发送          Continue 语义：compose 从会话行取 engine_session_id 作
                  prior_handle（compose.rs:224-266），引擎重建全史 ⟹ **上下文不丢**
点「新会话」      reset() 清空本地镜像 + sessionId = null（不可逆）⟹ 清单是后悔药
清单项信息        id 前缀 + updatedAt + 轮数（HistoryItem）
```

所以清单 = **续话入口 + 找回入口 + 「新会话」的后悔药**，三合一。

架构页的差异（这是要判断的关键）：

| 维度 | change chat | 架构页 chat |
|---|---|---|
| 生命周期 | 随 change 结束（归档后只读） | **workspace 级，长期**（跟项目一样长） |
| 会话数量 | 通常个位数 | 会累积到几十上百 → 分页 + 辨识负担 |
| 会话内容 | 「这个 change 怎么改」 | 「这批模块怎么划分 / 这个域该不该拆」 |
| 上下文过期风险 | 低（worktree 生命周期短、改动集中） | **高**（见下） |

**过期风险值得单列**：历史会话续接时携带的是**对话史**（assistant 当时看到 / 说过的内容），
而模型文件是活的。agent 的工具（`archi_module` / `read`）每次实时读盘，所以**工具调用永远
新鲜**，只有 assistant 的「记忆」是旧的。三个月后接着一条老会话问「现在 App.Billing 长啥样」，
agent 可能凭记忆直接答而不调工具。

缓解（零机制成本，落在 §9.3 的知识文案里）：

```
续接历史会话时，先 archi_module()（无参薄概览）或 archi_module(element=…) 取当前模型现状，
再回答「现在是什么样」一类问题——不要依赖会话早前的描述。
```

顺带一个弱替代路径（说明清单不是「必须」而是「顺手」）：会话不会丢——store 是每 workspace
库，系统工具 → Agent 调试页能列全部来源的会话。但该页的来源筛选是封闭三值
`'debug' | 'change' | 'all'`（`use-agent-run-history.ts:6`），archi 只能走 `all` 翻全库，
发现性很差。

**结论**：保留「新会话 + 历史 ▾」。骨架既已归一，增量成本 ≈ 0（`use-archi-chat` 新增清单
取数 + `list_archi_chat_sessions` 薄命令）。**不建议**做「打开即自动恢复最近一条」——与
change chat 的空态裁定分叉，且会让「同一 workspace 长期演进的会话」无声接管输入框。


## 14. 分叉收敛状态

已裁定（本轮）：

| # | 议题 | 裁定 |
|---|---|---|
| 1 | 写权 | **B**：架构页会话直改主仓模型；`write` 越界靠 **prompt 软约束**（sandbox 不加子面白名单） |
| 2 | 语法自检 | **路径 a**：prompt 要求改完调 `archi_module()` 复查；不新增 `archi_check` 工具。安全兜底 = 用户点刷新见 `StructureErrorPanel`（§12.1） |
| 3 | 知识通道 | `preamble` 位首激活：静态骨架 md + `archi::dto` / `layout` 派生片段拼装；**产品知识在前、`AGENT.md` 在后，合并成单条 System 消息** |
| 4 | 刷新 | 手动刷新按钮必需（已有 `archi-refresh`，链路现成）；会话收口自动刷新不做 |
| 5 | 代码偏差判定 | 本轮不做（②只是场景举例，归 `archi-check-and-gate`） |
| 6 | 面板 | 抽 `components/agent/session-panel.tsx` 骨架（含折叠窄条）；**change chat 本轮就换，explore 不换**（§13.2） |
| 7 | archi 清单命令 | 加 `list_archi_chat_sessions` 薄包装，与 change 对称（§13.3） |
| 8 | 面板布局 | **统一左侧 agent chat**（与 change 详情 / explore 详情同构的左栏分栏，可收起）；不做右侧抽屉 |
| 9 | 选中元素即上下文 | **本轮不做**（不动 `ArchitectureCanvas` 的 props 面） |
| 10 | 历史清单 | 保留「新会话 + 历史 ▾」，不自动恢复最近一条（§13.4） |

仍开放（实现期再定，不阻塞提案）：

- 知识拼装失败（`layout` 派生异常等）的降级形态——建议照 `preamble::load` 的「尽力增强、不炸 run」语义，返回 `None` 而非 `Err`。
- 骨架入参的**最终命名**（`SessionPanelState` / `labels` / `testIdPrefix` 等）随抽取时定型。
- 架构页左栏的 `defaultSize` / `minSize` 具体值（沿 change 详情 ~30 / ~22 与 explore 55 / 25 之间取）。



## 15. 代码地图（下一轮要读的具体位置）

| 关注点 | 位置 |
|---|---|
| 注入装配单点 | `packages/desktop/src-tauri/crates/infra/agent/src/compose.rs:23` |
| 词汇（SessionToolset） | `crates/core/agent/src/runner.rs:47` |
| 工具定义 / 执行体 | `crates/infra/agent/src/sdk/tools.rs:57`、`sdk/archi_tool.rs` |
| 权限三档 | `crates/infra/agent/src/sdk/policy.rs:4` |
| 系统提示词通道 | `crates/infra/agent/src/sdk/preamble.rs`、`loop.rs:195` |
| 架构页视图 | `packages/desktop/src/views/architecture/architecture-view.tsx` |
| 可复用 chat 面板 | `packages/desktop/src/views/changes/agent/change-chat-panel.tsx` + `hooks/use-change-chat.ts` |
| 会话清单命令面 | `packages/desktop/src-tauri/src/commands/exec/mod.rs:241 / :303` |
| 权威 spec | `openspec/specs/desktop-archi-agent`、`desktop-archi-canvas`、`desktop-agent-execution:254/279/309` |
| 原设计出处（已归档） | `openspec/changes/archive/2026-10-27-archi-graph-doc-agent/explore.md §5` |
