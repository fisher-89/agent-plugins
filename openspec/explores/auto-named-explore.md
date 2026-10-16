# 探索免命名创建：需求直入、占位名与 Agent 反写

workflow_type: requirement

## 主题与背景

用户诉求：桌面端新建探索时不再要求用户输入 kebab-case 名称，直接输入需求；建档后
立即开始会话（自动发出首轮）；以 `<日期时间>-分析中` 作临时标题；Agent 在会话中
生成最终 kebab 名与人类可读标题后**反写**（改名 + 改标题）。

本次会话本身就是该流程的活样本：记录名为占位名 `auto-named-explore`，stance 前导
按 `record.name` 钉死笔记文件名（`explore-stance.ts` 的 `stancePreamble(topic)`），
所以本笔记写到 `auto-named-explore.md`。当前仓库无任何 `auto-named` 字面量——
占位名机制尚未编码，`rename` 能力处于"已建成未接线"状态。

## 现状机制（读码结论）

### 创建链路（现状：用户必须起 kebab 名）

```
[新建对话框] TopicEntry: 用户输入 kebab 名（本地 isKebabCase 校验，非法禁提交）
             ImportEntry: scan_explores 列未绑定 kebab 文件，点击绑定
      │ create_explore_record(root, name)   ← 仅写 DB，不落盘文件
      ▼
store 同事务: ExploreRecord{title=name, workflow_type=requirement}
             + 预建空会话（session_id 回写，0.5.6 起建档即有会话归属）
      ▼
onCreated → list.refresh() + navigate(/explores/:name)
      ▼
详情页: useExploreSession（record.sessionId 直供，useAgentChat 重放）
用户在 composer 手动敲首轮 → buildExplorePrompt(需求, name)
             = stance 前导（仅首轮注入，0.4.36）+ 需求
```

### 回填闭环（已存在，title / workflow_type 两路先例）

```
agent 落盘 openspec/explores/<name>.md
   │  watch 单文件订阅（explore_doc_path 派生路径）
   │  → 500ms trailing 防抖 → read_explore 稳定内容
   │  （run 终态 running true→false 另有定点重读兜底）
   ▼
useTitleBackfill        首行 `# <标题>` ≠ record.title → update_explore_title
useWorkflowTypeBackfill `workflow_type:` 行 ≠ record → update_explore_workflow_type
（解析失败/无差异 → 零 invoke；回填失败静默，下次内容变化再试）
```

### 关键存量资产

| 资产 | 状态 | 对本需求的意义 |
|------|------|----------------|
| `rename_explore_record`（store+命令+bindings+`list.rename`） | **UI 零接线，休眠** | name 反写的 DB 半边已就绪；in-place 改名保主键=保会话链 |
| title / workflow_type 回填模式 | 在产 | name 反写照抄此模式即可（第三路回填） |
| 建档预建空会话 | 在产 | "创建后立即开始会话"无需新会话机制，只差 auto-send |
| `rename_explore_record` 契约注释 | "文件改名后的记录重关联入口" | 现契约假设**文件已被外部改名**；本需求需决定谁改文件 |
| `extractMarkdownTitle` / `extractWorkflowType` | 在产 | marker 行解析先例（`name:` 行解析可仿写） |
| stance 前导 | 在产（首轮注入） | 需扩"生成 kebab 名并写标记行"指令；与 SKILL.md 双源同步 |

## 目标流程（提议）

```
[新建] 用户直接输入需求（自然语言，textarea）
   ① 生成唯一占位名（如 draft-20260102-153012）
   ② create_explore_record(root, 占位名)
   ③ 设临时标题 "<日期时间>-分析中"（update_explore_title 或 create 加可选 title 参）
   ④ navigate(/explores/<占位名>, state={autoPrompt: 需求})
详情页: 重放装载完成 && chain.length===0 → 自动 send(stance + 需求)
agent:  首轮即知需求 → 写笔记骨架，含三行约定：
        `# <标题>` / `workflow_type: <t>` / `name: <kebab>`（新增第三标记行）
   │ watch 防抖重读 → 三路回填
   ▼
title   → update_explore_title（既有，"分析中"被真标题覆盖）
wtype   → update_explore_workflow_type（既有）
name    → ★新 finalize 命令：fs::rename 笔记文件（存在时）+ store in-place 改记录名
        → 前端 navigate replace(/explores/<kebab>) + watch 自动重订阅新路径
              （name 变 → useExploreDoc effect 重挂 → 订阅新路径，天然成立）
```

层边界注记：store 层纯 DB（create 明确"只写 DB 不触磁盘"）；文件操作在
commands/workflow 层（promote 的 move 半边在 `workflow::write`）。finalize 命令
应在 commands 层编排两步（改文件 + 改记录），非原子——按 promote 先例：失败显式
Err 带残留物与恢复指引。

## 待决问题（按影响排序）

1. **反写协议**：name 标记行（推荐，desktop 拥有改名三态一致）vs agent 自行 `mv`
   文件（watch 只盯旧路径，mv 后桌面端无从得知新名，仍需标记行 → 文件移动不如一并
   收归桌面端）。
2. **占位名唯一性**：固定 `auto-named-explore` 第二个草稿即撞"记录已存在"。候选：
   `draft-<yyyyMMdd-HHmmss>`（同秒双开仍可能撞 → 重试/随机短后缀）；占位名前缀
   同时是"未定名"态的判别依据（stance 指令、promote 门禁都要用）。
3. **finalize 新命令 vs 扩展 rename_explore_record**：现契约是"重关联已改名文件"，
   扩展会改语义；新命令（file mv if exists + record rename）契约更干净。
4. **auto-send 通道**：router location.state 传 `autoPrompt`。F5 时 state 随 history
   存留 → 需闸门：装载完成 && chain.length===0（发出即有 running 轮行）&& 一次性
   消费 ref/sessionStorage。
5. **URL 替换竞态**：finalize 成功 → list.refresh + navigate replace 之间详情页按旧
   name 找记录会 miss（现 MissingRecord 占位可兜底，但最好 invoke 完成后再导航）。
6. **未定名即 promote**：占位名合法 kebab → 会以 `draft-*` 建 change。候选：promote
   门禁拦"占位前缀未定名"并提示；或允许（用户自负）。
7. **agent 生成名撞已有记录**：rename 返回"目标名已存在" → 呈现策略（行内错误 +
   请 agent 换名？自动加后缀？）。
8. **临时标题设置方式**：create 后补一发 update_explore_title（两连 invoke，无原子
   窗口问题——失败只是标题暂缺）vs create_explore_record 加可选 initial title。
9. **stance 前导改造**：占位名记录的前导加"生成 kebab 名 + 写 name: 行"指令；非
   占位记录不加。注意前导仅首轮注入，agent 首轮已知需求，够用。
10. **「分析中」的消失时机**：title 回填发生在 agent 写出 `# 标题` 行的防抖重读后；
    若 agent 迟迟不写标题，标题保持"分析中"（可接受）。

## 涉及面清单（落地时）

- 前端：`explore-create-dialog.tsx`（TopicEntry → 需求输入）、`explore-view.tsx`
  （onCreated 带_state 导航）、`use-explore-session.ts`（auto-send 闸门）、
  `explore-detail-view.tsx`（useRecordBackfill 加 name 路 + 导航协调）、
  `lib/explore-stance.ts`（前导加命名指令 + SKILL.md 同步）、新 `lib/explore-name.ts`
  （占位名生成 + name 行解析）、`bindings.ts`（重新生成）
- 后端：`commands/explores/mod.rs`（finalize 命令）、store 复用 rename；测试随迁
- spec：`desktop-explore-page`（新建流程 Requirement 改写 + 新增 name 回填
  Requirement + auto-send Scenario）
- 既有测试改写：`explore-create-dialog.test.tsx`（kebab 校验用例转向占位名流程）、
  `use-explore-session.test.ts`

## 结论倾向（探索期，未定稿）

占位名 + 三标记行 + desktop-owned finalize 是与现有架构咬合最紧的形状：回填模式、
watch 重订阅、in-place rename、预建会话四件存量资产全部直接复用；唯一真正的新
机制是"文件+记录"联动改名命令与 auto-send 闸门。

## 决策记录（2026-01-27 会话推进）

用户裁定两条（对应上文待决 1 / 2）：

1. **反写协议 = 标记行**：agent 在笔记中写 `name: <kebab>` 约定行，桌面端解析并
   执行 finalize（改文件 + 改记录）。agent 不自行 mv 文件。
2. **占位名 = 拼接时间**：占位名带时间戳后缀保证唯一。

### 决策推演出的定案形状

**笔记三行约定**（现有两行 + 新第三行，解析器仿 `extractWorkflowType` 逐行扫描
首个命中行）：

```
# <标题>                        ← extractMarkdownTitle（既有）
workflow_type: <requirement|test-only>   ← extractWorkflowType（既有）
name: <kebab>                   ← ★新 extractExploreName（仿 wtype 解析器）
```

幂等性白捡：改名后笔记里 `name:` 行指向的就是新名 → 解析无差异 → 零 invoke；
后续 agent 重写笔记若丢标记行 → 解析 null → 保持记录名，不会误改回。

**占位名格式**（kebab 合法性已验：`draft-20260102-153012` 三段——首段字母开头，
后段数字开头，均合法；且 promote 的 is_kebab_case 门槛拦不住它 → 见下方余留 3）：

```
占位名:   draft-<yyyyMMdd-HHmmss>      （kebab 合法）
临时标题: <yyyy-MM-dd HH:mm:ss>-分析中  （同一时间戳派生，展示面纯装饰）
```

同秒双开仍可撞（create 返回"记录已存在"）：生成侧在前端可直接换下一秒重试；
生成侧若在后端（draft 专用命令内 retry loop）则更净——两案见余留 1。

**auto-send 补充勘定**（查证 change-flow 无 auto-start 先例，此机制全新）：
- 路由 = HashRouter（app.tsx:79）→ `navigate(..., { state: { autoPrompt } })` 的
  state 会随浏览器 back/forward **复现**——纯 ref 一次性消费不够（back 后 forward
  重现 state，ref 已消费 → 依赖 chain.length===0 兜底，此组合恰好正确：重放后
  chain>0 自然拒发；若首轮 send 失败 chain 仍 0，back/forward 反而构成一次免费
  重试）。闸门三条件：装载完成（!loading）&& chain.length===0 && state.autoPrompt
  在场。`use-explore-session` 的 send 已内建 loading/未就绪守卫，auto-send 只是
  多一个 effect 触发点。

### 余留问题（原 10 条裁掉 2 条，重排）

1. **占位名生成侧**：前端生成（复用 create_explore_record，撞名即显式错误/本地
   重试）vs 后端新 draft 命令（create + 占位名生成 + 可选 initial title 同事务，
   retry loop 内聚）。倾向后者——临时标题也顺带一次 invoke 落定，免两连发；但
   属新命令成本，proposal 期定。
2. **finalize 命令形状**：`finalize_explore_name(root, old, new)` = fs::rename 笔记
   文件（存在时；不存在则跳过——name 行必然出自笔记，常态文件在场）+ store
   rename_explore_record。两步非原子，按 promote 先例：失败 Err 带残留物与恢复
   指引（文件已改名而记录未改 → 记录名 miss 文件，预览空态 + scan 出现未绑定
   新名文件，恢复 = 重跑 finalize 或手动改回）。
3. **draft 名可 promote**：`draft-*` 合法 kebab → 未定名也能启动变更。候选：a)
   promote 门禁拦占位前缀（需"占位"判别知识进命令层——前缀常量共享）；b) 不拦
   （用户自负，change 名 = draft-* 难看但无害）。倾向 a，成本一个前缀比对。
4. **agent 生成名撞已有记录**：rename Err"目标名已存在" → 回填路不静默（区别于
   title/wtype 静默失败），详情头行内呈现，用户让 agent 换名 → 改标记行 → watch
   重读 → 重试 rename，自愈闭环。
5. **URL 替换时序**：finalize 成功 → list.refresh + navigate(replace, 新名)；
   刷新完成前详情页按旧名 find miss → MissingRecord 闪现。缓解：await rename 完
   成后再 refresh+navigate（回填 effect 本就在内容稳定后触发，天然异步，无阻塞）。
6. **stance 前导改造**：占位名记录（前缀判别）的前导追加"生成 kebab 名并写
   `name:` 第三行"指令；非占位记录维持现状（不加，避免存量流程噪声）。
   SKILL.md 双源同步照旧。

## 决策记录·二（同会话续推）

用户再裁两条（对应上文待决 1 与新增的工具面议题）：

1. **create 命令嵌入，名称选填**：`create_explore_record` 的 `name` 改
   `Option<String>`——`Some` 走现状（导入绑定显式名）；`None` 时后端同事务生成
   占位名与临时标题（"<日期时间>-分析中"文案内聚后端）。前端两连 invoke 问题
   消失；不新开 draft 命令。
2. **explore 会话注入专用工具，禁用 write（写面），文件名固定**：写笔记不再靠
   通用 write/edit + stance 文本纪律，而是注入 `write_explore_note` 一类专用
   工具——工具无路径参数，固定写当前记录的笔记文件；通用 write/edit 从 explore
   会话的工具面移除。

### 引擎侧现状勘定（读码结论）

- **core 契约已有接口位**：`core/agent/src/runner.rs` 的 `SessionInjections` 含
  `preamble: Option<String>` 与 `tools: Option<Vec<String>>`（注释明言"为切片③
  预留，MVP 引擎忽略"）——但 `Vec<String>` 只能表达**七工具子集（收窄）**，表达
  不了"追加参数化上下文的专用工具"。
- **生产装配恒空**：`infra/agent/compose.rs:157` `injections:
  SessionInjections::default()`——契约位完全未用，激活它不算破坏性变更。
- **SDK 引擎工具面全局静态**：`sdk/tools.rs` `TOOL_NAMES: [&str; 7]` +
  `definitions()` 全量七件；`sdk/loop.rs:104` 每轮全量组装
  `CompletionRequest.tools`；执行分发 `tools::execute(root, name, input)` 按名
  match，未知工具 Err"未知工具"。
- **policy 是档位×工具纯决策表**（`sdk/policy.rs`），与会话来源无关；收窄
  definitions 本身即最强约束（模型面看不到 write 就不会调），policy 只需兜底
  拒绝幻觉调用。

### 决策 2 的落地通路（提议形状）

```
SessionInjections 扩展（core 契约）
  tools: Option<Vec<String>>          ← 既有：七工具收窄（explore 用：read/grep/glob/ls + bash?）
  + toolset: Option<SessionToolset>   ← 新增受控枚举（如 ExploreNote），引擎侧
                                        解释为 write_explore_note 定义+执行体
  （受控枚举而非自由字符串——沿"工具面封闭清单"哲学，模型幻觉工具名不进执行）

compose 装配（生产路径激活契约位）
  explore 会话（provenance.source='explore' 判别）→ 注入收窄 + ExploreNote

loop 执行（LoopTurn 增 note 上下文）
  definitions 组装：收窄子集 + write_explore_note(content: string, mode?: append|overwrite)
  执行路由：write_note → 写 turn 解析的笔记路径（复用 write 落盘 + 沙箱天然过：
           固定路径本身就在 workspace 内）
```

### 决策 2 牵出的三个新关键点

1. **note 路径必须按轮解析，不可会话级快照**：记录名会被 finalize 改（占位→真
   名）。若 LoopTurn 建立时快照路径，改名后 agent 续写会落回旧占位名文件——
   幽灵文件 + scan 冒未绑定条目。正确形状：每轮 compose 时按 store 现值
   （record.name）解析笔记路径。watch 侧同理已天然成立（name 变→重订阅）。
2. **stance 前导改写**：「笔记落盘」段的文件名知识（"MUST 写入
   openspec/explores/<topic>.md"）退位给工具描述；stance 改为指令"用
   write_explore_note 落盘笔记 + name: 标记行表达命名意图"。决策 1 的标记行
   反写**不受影响**（命名意图走标记行，桌面端 finalize 执行改名）——本解读
   待用户确认："固定名字"= 工具文件名固定于当前记录名，而非"取消改名机制"。
3. **append 语义**：stance 现要求"默认追加；用户明确要求才整文件重写"。专用
   工具若只有覆写语义，agent 每轮需 read→拼→全量写（token 贵且易丢内容）；
   建议工具带 `mode: append|overwrite`（或拆两工具）。bash 是否保留给 explore
   会话待定（现状 bypassPermissions 放行 bash；探索立场允许 bash 调研？git log
   / wc 等命令对调研有用，但 bash 也能写文件——若保留，"禁写"形同虚设；若
   移除，调研能力受损。倾向移除 write/edit/bash 三件，探索纯只读 + 专用工具）。

### create 签名变化影响面（决策 1）

- `commands/explores/mod.rs`：`name: Option<String>`（specta 类型生成随之）
- `store::create_explore_record`：`None` 分支生成占位名 + 临时标题；**同事务内
  撞名重试不能靠重读时钟**（事务内时间可能同值）——毫秒段进名
  （`draft-<epoch-millis>` kebab 合法）或撞名时单调 +1 兜底；proposal 期定
- 前端 `bindings.ts` 重新生成；`TopicEntry` 改需求输入（textarea），isKebabCase
  校验与"kebab 报错"用例退役；ImportEntry 调用改传 `Some`
- spec `desktop-explore-page`「新话题不预建文件且 kebab 校验」scenario 整体改写
  （kebab 校验场景仅在导入路径残留）

### 更新后的待决清单（余留）

1. "固定名字"语义确认（工具文件名固定 vs 取消标记行改名）——本笔记按前者理解。
2. 专用工具参数面：mode append/overwrite；是否给 title 参数（可省——首行 #
   即 title，标记行协议已覆盖）。
3. explore 会话 bash 去留（倾向移除，见上）。
4. finalize 命令形状（前文余留 2，未变）；draft 名 promote 门禁（前文余留 3，
   未变）；URL 替换时序（余留 5）。
5. SessionInjections 扩展形态：受控枚举 vs tools 数组放宽——倾向枚举。

## 决策记录·三（同会话续推）

用户再裁三条（收束工具面全部议题）：

1. **初期 prompt 不含 note 路径；路径由工具调用返回**：agent 不知道文件在哪，
   调用创建工具 → 工具响应携带解析后的文件路径 → agent 以此路径续用 Edit。
2. **创建工具仅限初次写入（文件不存在时）；已存在报错；Edit 不屏蔽**：无
   append/overwrite 参数——初建走专用工具（create-only 语义），后续修改全部
   走通用 Edit。write 仍禁用（前次决策），edit/bash 保留。
3. **bash 保留**；创建工具的使用约束（何时用、优先用它而非 bash 造文件）经
   初始 prompt 软约束。

### 定案后的 explore 会话工具面

```
            硬约束（工具面）                软约束（prompt）
  ┌─────────────────────────────┐   ┌──────────────────────────────┐
  │ read / grep / glob / ls     │   │ 命名意图写 name: 标记行       │
  │ edit（保留，沙箱内）         │   │ note 结构约定（# 标题 /       │
  │ bash（保留，活性护栏）       │   │   workflow_type: 行）         │
  │ create_explore_note(content) │   │ 优先用创建工具而非 bash 造文件│
  │   └ 无路径参数，服务端解析    │   │ 探索立场（不写应用代码…）     │
  │ write —— 从 definitions 移除 │   └──────────────────────────────┘
  └─────────────────────────────┘
```

### 派生性质（三条决策组合出的意外收获）

1. **路径发现闭环 + 撞名即告知**：create 成功 → 返回路径；文件已存在 → 报错
   **也应携带现路径**（导入场景常态命中：既有笔记 → 错误信息即路径告知 →
   agent 转用 Edit）。错误文案是恢复通道，非纯失败。
2. **append/overwrite 议题消解**：create-only + Edit 唯一匹配语义 = 天然安全
   的追加模型，无需 mode 参数。
3. **路径解析天然 per-call**：create 工具无路径参数 → 服务端执行时按 store
   现值解析当前记录名——比 per-turn 快照更强，finalize 改名后下一次 create
   调用自动指向新名。
4. **stance 前导失去 ${topic} 插值**：路径知识退位给工具 →
   `buildExplorePrompt(userInput, topic)` 的 topic 参可能整体退役（命名指令
   只对占位记录下发，判别在前端，不靠 name 插值）。前导与 SKILL.md 双源同步。

### 新坐实的实现刺（读码验证）

**definitions 收窄 ≠ 硬禁用——幻觉调用直通执行**。loop.rs 工具分发链为
`policy → sandbox → execute`，**无 definitions 成员校验**：模型若幻觉出
`write` 调用（已从 definitions 移除），policy 表中 write ∈ WRITE_TOOLS →
放行 → 执行。硬禁用需 loop 增加"调用名 ∈ 本轮实际下发 definitions"校验
（或 policy 感知注入面）。`tools::execute` 的"未知工具"分支只兜未知名，
不兜"已知但未下发"名。RunStarted.tools 上报口径（loop.rs:111 引
`tools::TOOL_NAMES` 全量常量）需随实际下发面走。

### 残余边界

1. **Edit 陈旧路径自愈环**：finalize 改名后 agent 手里的旧路径失效 → Edit
   报错（文件不存在）→ agent 重试 create → 错误信息携带现路径 → 恢复。依赖
   上文"错误文案带路径"设计；不加新机制。
2. **create 执行体的上下文解析缝**：tools::execute 纯函数（root, name,
   input），create_explore_note 需要当前记录名（store 现值）——经 LoopTurn
   携带解析闭包 / 会话上下文句柄，架构缝 proposal 期定。
3. 未变余留：finalize 命令形状、draft 名 promote 门禁、URL 替换时序、
   SessionInjections 扩展形态（tools 收窄可复用既有 Vec<String> 位——
   [read,grep,glob,ls,edit,bash] 即收窄表达；追加 create_explore_note 仍需
   新增受控枚举位）。

## 决策记录·四（同会话续推：create 原子定名）

用户裁决：「Agent 手里应该没有旧路径，创建文件的工具已经指定了正确的文件名」。
这条否定了上轮的「Edit 陈旧路径自愈环」前提，其成立方式只有一种——**create
工具调用时就携带 agent 定的最终名，文件与记录同时定名**（原子）：

```
轮 1: agent 探索 → 起名 api-retry → create_explore_note(
        name: "api-retry",            ← 最终名，创建时指定
        content: <初稿，两行约定>)
        │ 服务端（原子两步）：
        │   ① 文件落盘 openspec/explores/api-retry.md
        │   ② store rename: draft-xxx → api-retry（in-place 保主键保链）
        ▼
      返回: openspec/explores/api-retry.md
轮 2+: agent Edit 此路径 —— 文件名自诞生即最终名，永不失效
      （占位名从未落过盘 → 世界上不存在"旧路径"）
```

### 对决策链的连锁修正

| 前决策 | 修正后 |
|--------|--------|
| 决策 1：name: 标记行反写 | **退位**——name 走 create 参数；笔记退回两行约定（`# 标题` + `workflow_type:`）；第三路回填（name）取消 |
| finalize 命令（改文件+改记录） | **退役**（会话内不再需要改名）；`rename_explore_record` 维持休眠，仅服务会话外手动改名边缘 |
| Edit 陈旧路径自愈环 / 错误文案带路径 | **消解**；「已有文件报错」仍保留（导入场景：既有文件名=最终名，路径天然稳定——恰好佐证本设计） |
| 占位名 = draft-<时间戳> | 不变，但它的生命周期缩为「仅存在于 store 记录名，从未落盘」 |

### 新的主要工程点：前端感知记录改名

create 执行时记录名变了（draft-xxx → api-retry），而前端 URL 是 /explores/
draft-xxx、watch 盯旧路径文件、详情按 URL name 从清单 find 记录。感知通路
候选（读码验证过基础件）：

- **id 桥**（倾向）：run 终态（running true→false，现成 effect）→ list.refresh()
  → 按 record.id（主键，in-place rename 不变）重寻记录 → 得新 name → navigate
  replace 新 URL → useExploreDoc 以 name 为依赖自动重订阅。与 changes 页
  「一切寻址以 id 为准」哲学呼应。
- **SystemNotice 子类型**：core 事件 SystemNotice{subtype, payload} 子类型开放
  （event.rs:89），create 执行体发 explore_renamed 通知 → 前端事件流捕获 →
  refresh+navigate。实时性更好，但事件面多一路消费。

中间窗口（create 已执行、run 未终态）：前端仍显示占位名 + watch 旧路径
（无文件，空态）——run 终态后收敛，可接受（与「分析中」临时标题同窗）。

### 残余小刺

1. **撞名**：create(name) 撞已有记录/文件 → 报错 → agent 换名重试（自愈）；
   名字非法 kebab → 同路报错。
2. **定名后悔**：create-only 语义下名字一次定死，agent 无改口机制（可接受；
   会话外 rename 休眠能力兜底）。
3. **引擎侧上下文缝不变**：create 执行体要触 store rename——比 per-call 读
   记录名更进一层（写记录），LoopTurn 携闭包/句柄的缝同前。
