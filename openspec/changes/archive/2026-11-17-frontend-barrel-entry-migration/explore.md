# 变更 B · 前端 barrel 入口化（目录 = 模块）

workflow_type: requirement

> 主线：`openspec/explores/module-entry-declaration.md`。
> 本单元 = 12 个前端目录各补一个 `index.ts(x)` barrel + 调用点改写 + `Components.Agent` 容器翻转。
> 已裁口径：**统一走 i**（补 barrel，粒度不变）；单文件目录同样补，避免同仓两套约定。

## B.0 scope 与依赖

| 项 | 内容 |
|---|---|
| 前置依赖 | 变更 A（`entry` 与默认值口径）；变更 C（否则前端契约一写就红） |
| 被依赖 | 无（末位） |
| 独立可归档 | 是，但开工顺序应在 A 与 C 之后 |
| 不做 | 不给 `hooks/` / `lib/` 建元素；不改后端粒度；不动 `ui` 的调用点（若选 B.5 的折中） |

## B.1 已裁：统一走 i（补 barrel，粒度不变）

### B.1.1 名单：12 个（已裁）

| 组 | 目录 | 顶层非测试源文件 | 补 barrel 后入口 |
|---|---|---|---|
| 多文件（必须补） | `views/changes` | 2（list / detail） | `views/changes/index.ts` |
| | `views/changes/agent` | 2 | `…/agent/index.ts` |
| | `views/changes/flow` | 15 | `…/flow/index.ts` |
| | `views/changes/renderers` | 7 | `…/renderers/index.ts` |
| | `views/explores` | 4 | `…/explores/index.ts` |
| | `components/ui` | 39 | `…/ui/index.ts` |
| 单文件（若也统一） | `views/{agent,agents,architecture,config,db,info}` | 各 1 | 各补 `index.ts` |

**已裁：12 个全补**（含 6 个单文件目录）。两者都曾自洽，但**混用会破功**：多文件目录靠 barrel、单文件目录靠「唯一文件即入口」，等于同一仓库两套约定，而单文件目录多一个 `helper.ts` 就静默换默认入口（虽然会立刻报 `apiEntryMissing`，属「响亮失败」，但仍是漂移源）。若要守住「目录 = 模块」，**12 个都补**才是统一形态；只补 6 个则是「R1 ∪ R2 默认」+「多文件补 barrel」的混合口径。

### B.1.2 硬阻塞：TS 取数面读不出 barrel（必须先修）

`ts::extract_file` 零 `export … from` 分支（`api/extract/ts.rs:119-139`）⇒ barrel 作入口时导出集**恒为空**
⇒ 前端契约一写就红、`bootstrap` 草稿恒空骨架。**修法在变更 C**（`ts-export-closure-extraction.md`），
本单元不重复设计，只把它标为硬前置。

### B.1.3 调用点改写扇出（barrel 必须真被用，否则是死文件）

| 调用点 | 现状 | barrel 化后 |
|---|---|---|
| `routes.tsx`（壳） | 11 条文件级 import 到 9 个视图目录 | 9 条 |
| `views/changes/change-detail-view.tsx` | `./flow/*` 7 处、`./agent/*` 2 处 | 2 条 |
| `views/changes/flow/detail-drawer.tsx` | `../renderers/artifact-tabs` 1 | 1 |
| `views/explores/components/explore-preview.tsx` | `../../changes/renderers/markdown-doc-renderer` 1 | 1 |
| 全仓 `@/components/ui/<file>` | **120 处** | 120 处若全改则等于重写全前端 import 面 |

合计约 **132 处**，其中 `ui` 独占 120。可选的折中：`ui` 的 barrel 只作为「入口声明 + 契约落点」存在、**不改调用点**（`src/components/ui/**` 已在 knip / fmt / lint 三处 ignore，死文件不会被抓）——但那就把「入口」从「公共点」降级为「抽取起点」，与 9.4 的 ii 语义重合，是口径上的自我稀释。

### B.1.4 图与边：barrel 是**边中立**的（前提明确）

`scan/ts.rs` 把 `export … from` 也记为 import fact（不是只记 `import`），但成边循环里有 `if &to == from { continue; }`（`scan/mod.rs:242`）——同元素内自边丢弃。因此：

- barrel 只 re-export **本元素内**文件 → 零新边、零计数变化 ✅
- barrel 若 re-export **跨元素**文件（例如 `views/changes/index.ts` 顺手把 `renderers` 的东西也导出来）→ 真加边，`0001-model-granularity.md` 的「关系只取 import 证据」判据会如实记账，模型必须同步 ⚠️

### B.1.5 静态检查面：knip 会把 barrel 当「公共面账本」

`knip.json` 的 `ignore` 已含 `src/components/ui/**` 与 `src/**/*.i.ts`；`vite.config.ts` 的 `NO_STATIC_CHECK` 同两条。所以：

- `ui/index.ts` 不进 knip / fmt / lint（连格式化都不会被管）——省事但也很容易变成「没人读的目录清单」；
- `views/*/index.ts` 进 knip：**未被任何调用点 import 的 re-export 会被判 unused export** → barrel 必须恰好等于真实公共面。这一条客观上把「入口 = 公共面」变成机械可验的事实，是 i 方案最实在的副产物。

### B.1.6 容器翻转：`App.Frontend.Components.Agent`

它今天是 `kind = domain` + `path = [components/agent/index.tsx]`（ADR 0001 §3 的「容器 path 恒空」唯一例外，合法作为关系端点）。在「barrel 是目录模块入口」的新约定下，它更自然的形态是：

```text
kind = "module"            ← domain → module
path = ["./packages/desktop/src/components/agent/"]   ← 文件 → 目录
entry = "./packages/desktop/src/components/agent/index.tsx"
```

连带：`repo-archi-model` 的前端构成**现值 30 元素**（4 容器 + 26 叶）→ 翻转后 **3 容器 + 27 叶（总数不变）**，元素侧全仓 53 不变；ADR 0001 §3 的「唯一例外」段落要改写为「barrel = 目录模块的入口，不再是容器例外」；它的 **7 条端点边逐条不变**（出 3：重导出 `AgentInput` / `AgentSessionView` / `SessionPanel`；入 4）。

> 现值更正（2026-11-17）：本单元起草时前端是 28 元素 / 6 条端点边；`session-panel.tsx` 与 `standard/tooltip.tsx` 补建元素（`2026-11-17-frontend-component-element-coverage`）后为 30 元素 / 7 条端点边。翻转的**结论不变**（容器 -1、叶 +1），但数字须按现值写。

### B.1.7 二阶代价：TS 侧的实现绑定点会变没

Rust 侧的绑定靠入口里的 `#[path = "lib.i.rs"] mod api_gen;` + 生成物尾部的签名见证（`const _: fn(..) = super::X::method;`）——**入口自己就是实现文件**。TS 侧的绑定机制是「实现侧 `import type` 引用生成类型并标注返回类型」，同样要求**标注发生在入口**。

barrel 入口里只剩 `export { X } from './x'`，不带类型标注 → 绑定点消失或被迫散到 15 个实现文件里。可选形态：

- 在 barrel 里做「带类型的再导出」（`export const f: typeof import('./index.i').f = realF`）——机械但啰嗦；
- 承认 TS 侧绑定弱于 Rust（当前 TS 侧零契约，这条不会立刻痛，但会决定前端契约将来是否「能验」）。

### B.1.8 一处必须写死的边界

barrel **MUST NOT** re-export `hooks/` 与 `lib/`（视图内这两类不建元素、其 import 计入所属视图）。理由不是边（自边已丢），而是语义：把「非公共面的内部实现」写进入口，等于让「入口 = 公共面」这条判据从第一天起就失真。

### B.1.9 这一裁的完整扇出（按提交序）

1. `extract/ts.rs` 支持 `export … from`（含 `export *`）→ 否则后面全是空转；
2. `entry` 字段 + 解析链（9.3）+ 四条判据（9.1）→ 模型/解析/dto/bindings；
3. 接口文件 `<入口>.i.<后缀>` 同级落点 → `paths` / `emit` / `owner` / 迁移唯一那份生成物 + `INDEX.toml`；
4. 12（或 6）个 barrel + 调用点改写（10.3）+ knip 口径复核；
5. `App.Frontend.Components.Agent` 的 kind/path 翻转（10.6）+ `repo-archi-model` / ADR 0001 同步；
6. 模型 16 个后端元素补 `entry`（壳必须显式；14 个 crate 走默认可省）+ 27 条 `apiEntryMissing` 的逐条收敛（写入口 / 写 `api = none`）。

---

## B.2 公共面：barrel 的 7 个名字 vs 目录内 10 个导出

```text
components/agent/index.tsx（barrel，7 个名字）
  export { AgentInput }              from './agent-input'
  export type { AskAnswerActions }   from './agent-messages'
  export { AgentSessionView }        from './agent-session-view'
  export type { AgentSessionActions, AgentSessionViewProps } from './agent-session-view'
  export { SessionPanel }            from './session-panel'
  export type { SessionPanelState }  from './session-panel'

目录内实际 `export` 的还有（**不在公共面**）：
  agent-input.tsx      AgentInputProps
  agent-messages.tsx   AgentMessagesProps
  agent-blocks.tsx     isToolPart
  session-panel.tsx    SessionPanelProps（未导出 interface）
  tool-cards.tsx       AgentToolCard
```

这是 11.1 #1 的**活样本**：近似做法（拉被 re-export 文件的全部导出）会把 `AgentInputProps` / `AgentMessagesProps` / `isToolPart` 一并算进 `Components.Agent` 的导出集 → 与「入口 = 公共面」直接矛盾；精确名单才能把这 3 个名字挡在外面。`apiUndeclared` 现在是 off，虚增**不会报**——正是「靠撞运气」的形态。

## B.3 抽出来会是什么

| 符号 | 形态 | 抽取结果 | 可表达性 |
|---|---|---|---|
| `AgentInput` | 组件函数 | `Function`，params 是**解构对象** → `Unknown`，returns 无注解 → `Unknown` | ✗ 写不出有意义的 op |
| `AgentSessionView` | 组件函数 | 同上（签名 `(props: AgentSessionViewProps): React.JSX.Element`，返回类型 `Unknown`） | ✗ |
| `SessionPanel` | 组件函数 | 同上 | ✗ |
| `AskAnswerActions` | interface | `Type`；`onAnswer` 是函数字段、`pendingToolCallId` 可选标量 | ✓ 可写（函数字段有 `x-function` 落点） |
| `AgentSessionActions` | interface | `Type`；3 个布尔 + `send`/`stop`/`steer` 三个函数字段 | ✓ 可写 |
| `AgentSessionViewProps` | interface | `Type`；含 `session?: UseAgentSessionState`、`composer?: (…)=>React.ReactNode`、`askAnswer?: AskAnswerActions`、`'data-testid'?: string` | ⚠ 两处外来类型无着落 |
| `SessionPanelState` | interface | `Type`；含 `session: UseAgentSessionState`、`sessions: SessionSummary[]` | ⚠ 同上 |

## B.4 验收判据（机械可复算）

1. 12 个目录各有且仅有一个入口（`index.ts(x)`），入口文件在 `path` 内；
2. `scan` 的边集与改动前**逐条相等**（barrel 只 re-export 本元素文件 ⇒ 自边丢弃）；
3. knip 零 unused export（barrel 恰等于真实公共面）；
4. `archi_module` 对 12 个元素的 `docs`/`api` 投影与入口一致；`App.Frontend.Components.Agent` 的 6 条端点边逐条在案；
5. `repo-archi-model` 的计数复算：~~28 前端元素 = 3 容器 + 25 叶，关系 64 条不变~~ → **已过期**，现测 30 元素（4 容器 + 26 叶）/ 71 关系 / 7 条 barrel 端点边；翻转后应 3 容器 + 27 叶 / 71 关系不变（详见 B.7.3）。

## B.5 未决（本单元内）

- `ui` 是否也改调用点（B.1.3 写 120 处，**实测已 132 处 / 67 文件 / 35 模块**，见 B.6.3）还是只放一个「不改调用点的入口」（后者与 B.3 的 ii 语义重合，需明示取舍）；
- `views/*/index.ts` 的 `export type` / `export` 分割形态（受 `verbatimModuleSyntax` 约束）；
- 是否顺势把 `repo-archi-model` 里「容器 `path` 唯一例外」整段重写为「barrel 即目录模块入口」。

## B.6 启动就绪度评估（本次实测）

> ⚠ **本节写于变更 C 归档之前，已被 [B.7](#b7-c-落地后的遗留问题复测) 取代**（A / C 均已合入 master，两道闸解除）。
> 保留本节的理由只有一条：它记录「依赖序实景」与「四处漂移」的推导过程；数字与结论以 B.7 为准。

> 问题：「现在能不能把 B 开成 change？」结论：**能起草变更包，不能开工实现**——两条硬前置都还没到 master。

### B.6.1 三道闸的实测

| 闸 | 判据 | 实测（本会话） |
|---|---|---|
| A（`entry` 字段） | master 的 archi crate 有 `entry` 才能声明入口 | **无**：master `model.rs` / `dto.rs` 零命中 `entry`；A 的全部工作只存在于 worktree `D:/Projects/.worktrees/.../module-entry-field-spec` 的**未提交改动**（`git status` 全 `M`），分支 tip `bdff431` 是 master 的**祖先**（未合入） |
| C（取数面） | `extract/ts.rs` 认 `export … from` | **无**：`api/extract/ts.rs:68` 只把 `Statement::ExportDeclaration(export)` 的 `export.declaration` 解包（`export function/interface/type/const`），re-export 的 `declaration` 恒 `None` ⇒ 零产出 |
| B 自身前提 | 12 个目标目录零 barrel | **成立**：逐个 `ls` 复核，12 目录 `barrel=0`；唯一既有 barrel 是 `components/agent/index.tsx` |

A 的进度：`tasks.md` 61 项已勾、仅剩 1 项「移交说明（不勾为任务）」；但**未提交、未合入**。即「A 的代码基本写完」≠「A 已落地」。

### B.6.2 依赖序实景

```text
master  ──┬── [A 代码]  worktree 未提交 ──→ 需合并 + 宿主重建（entry 数据才可解析）
          │
          ├── [C 代码]  零行（仅探索文档）
          │
          └── [B]       ← 本单元：等 A 的代码 + 等 C 的代码
                    ├─ 12 个 barrel（纯前端，技术上不依赖 A/C）
                    ├─ 132 处 import 改写（同上）
                    ├─ 模型 12 条 entry + Components.Agent 翻转（依赖 A 的字段 + 宿主重建）
                    └─ 前端契约（依赖 C，否则一写就红）
```

注意一个**先落 B 的副作用**：barrel 一旦存在于盘上，A 的约定候选（`entry_candidates` 对目录型根列 `<root>/index.ts` / `<root>/index.tsx`，零 IO、按序取首个落盘者）就会命中 `index.ts(x)`，那 12 个目录根**立刻退出 ① `apiEntryMissing`**。

> 现值更正（2026-11-17）：原文引的 `paths::entry_form` 探测已随 A 归档退场（改为 `entry_candidates` / `entry_state` 三态）；基线也已由三段变四段：**28/13 → 15/26 → 12/27 → 12/29（当前）**。B 先落的副作用**性质不变**：会让那 12 条 `apiEntryMissing` 提前归零，使 `desktop-archi-api-check` 的「基线表」与「两条切分口径」两处读数提前换段。属「响亮但错序」的噪声，故 B 仍应排在 C 之后、并在同一 change 内更新基线表。

### B.6.3 探索文档的四处漂移（起草前必须重测，不能照抄）

| 探索文档写的 | 实测 | 影响 |
|---|---|---|
| `@/components/ui/<file>` **120 处** | **132 处 / 67 个文件 / 35 个模块**（`button` 35、`badge` 10…） | B.1.3 的扇出表与 B.5 第一条未决的「是否改调用点」都建立在 120 上 |
| `views/changes/flow` **15** 顶层源文件 | 15 ✅（但 `repo-archi-model` spec 写「flow（13 源文件）」） | spec 已失真 |
| `views/changes/agent` **2** | 2 ✅（spec 写「agent（3）」） | 同上 |
| （未提）`components/agent` | 非测试源 **7** + barrel 1；`session-panel.tsx` 在模型里**无元素**（`06-frontend.toml` 只列 6 个 Agent.* 叶） | `repo-archi-model` 的「覆盖判据（双向）」今天已不成立 |

根因：前端元素 `gate = "off"`，且 archi crate 里**没有**文件级覆盖检查（全仓 grep `uncovered` / `coverage` 只剩 `orphan_hierarchy` 与生成物 `orphaned`）——「判据命中的目录 / 文件集合与 path 集合相等」只是 spec 散文，没有机械手段抓它。**这直接决定 B 的 spec delta 不能照抄数字**：写完 barrel 后需要一次 frontend 模型对账（补 `SessionPanel`、补 `labeled-edge` 等），否则是把假话钉进 spec。

### B.6.4 三条启动路径

| 路径 | 形态 | 代价 / 风险 |
|---|---|---|
| **P0 · 先启 C** | C 的探索已到算法草案级（`surface()` 递归 + `TsResolver` 复用 + 6 条判据），先开 `change/ts-export-closure-extraction`，A 合并后实现 | 最贴依赖序；但 B 继续等（A+C 两跳） |
| **P1 · B 只起草** | 现在就建 `openspec/changes/frontend-barrel-entry-migration/`（proposal + design + tasks + spec delta 提纲），实现挂起，标「blocked by A、C」 | 风险：delta 的 `MODIFIED` 文本基于 A/C 合入前的 spec（`desktop-archi-api` 的 `paths.rs` 行、`repo-archi-model` 的容器例外段都会先被 A 改写），届时需重写一次 |
| **P2 · 拆 B** | B1 = 12 barrel + 132 处 import 改写 + knip 复核（零模型面，不依赖 A/C）；B2 = entry 声明 + `Components.Agent` 翻转 + 基线复算 + spec/ADR 同步 | B1 技术上独立，但（a）会提前改判 A 的基线 12 条；（b）knip 要求 barrel 恰等于真实公共面 ⇒ B1 必须把改写做全，`ui/index.ts` 因 `knip.json` ignore 而**零校验**（B.1.5 已记） |

### B.6.5 起草时不可省的收口

- `ui/index.ts` 的定位（B.5 第一条）：改 132 处 / 只放入口不改调用点 / 干脆不给 ui 建 barrel（须改 `Components.Ui` 的 `path` 形态）——三选一必须写进 proposal「明确不做」表；
- `export type` 与 `export` 的分割形态受 `verbatimModuleSyntax` 约束（B.5 第二条）；
- `repo-archi-model`：容器「唯一例外」段重写 + 28 = 3 容器 + 25 叶 + 64 边不变的复算（B.5 第三条），**且要先做一次漂移对账**（B.6.3）；
- 版本行：前端源改动 ⇒ `packages/desktop/package.json` patch +1（AGENT.md 纪律）。

## B.7 C 落地后的遗留问题复测

> 复测时点：master `e65f3d2`（`ts-export-closure-extraction` 已归档）。本段取代 B.6 的数字与结论。

### B.7.1 闸门复测：三道全开

| 项 | 实测（master） | 判定 |
|---|---|---|
| A · `entry` 字段 | `model.rs:52` / `dto.rs:482` `pub entry: Option<String>`；`api/paths.rs::entry_candidates` 三态派生（显式 / 文件型根 / 目录型根四候选）+ `entry_state` 四条判据 | **已落地** |
| A · `entry` 数据 | `04` 1 处 + `05` 2 处（`module-entry-data-backfill` 归档） | **已归位** |
| C · 取数面 | `api/extract/ts.rs:92` `extract_surface` 三面（声明 / named / star）；`extract/mod.rs` 的 `Closure` 递归 + `visited` 环防 + `reexport_ok` 同元素约束 | **已落地** |
| B 自身前提 | 12 个目标目录 `barrel=0`（逐个复核） | **零启动** |
| 基线 | 最新报告 `apiFindings = 41`（`apiMissing 29` + `apiEntryMissing 12`） | 12 条与 B 的 12 个 barrel **一一对应** |

12 条 `apiEntryMissing` 实测名单 = `Components.Ui` + `Views.{AgentDebug, Agents, Architecture, Changes, Changes.Agent, Changes.Flow, Changes.Renderers, Config, DbInspector, Explores, Info}`——与 B.1.1 的 12 个目录完全重合，说明 B 的边界本身就是判据边界。

### B.7.2 新发现 · 补 barrel 不消 finding，只换面（B 最大的口径空白）

`api` 字段缺省 = `declared`（`model.rs:47` 的 doc 明写），故：

```text
今天（41）                              补完 12 个 barrel 后（仍 41）
  apiEntryMissing  12  ← 12 个目录根        apiEntryMissing   0
  apiMissing       29                       apiMissing       41   ← 12 条原样搬过来
```

**这是「缺门」变「缺契约」**，不是清零。三种收口（MUST 在 proposal「明确不做」或设计里择一并写死）：

| 收口 | 结果 | 代价 |
|---|---|---|
| 留空（缺省 `declared`） | 12 条 `apiMissing` 恒报（`warn`，不进 `passed`） | 报告长期带 12 条噪声，但「前端 op 面留白」是 C.2.7 已承认的事实 |
| 标 `api = "none"` | 41 → 29，清零 | 「barrel 就是公共面」变假话，与 C 存在的意义正面冲突；且 `api=False` 断言无公共 API 是**数据说谎** |
| 写 `api-types` 契约（12 份 `index.i.ts`） | 41 → 29 且为真 | 首次触发 `.i.ts` 生成链 ⇒ 见 B.7.4（`ts_import_specifier` 未接线）；且要逐目录人工挑公共面（B.2 的活样本证明不能「拉全部导出」） |

探索文档 B.0–B.6 全篇没有这一条；它是 B 起草时的**第一个必答题**。

> **已裁（见 B.7.10）：留空**（缺省 `declared`），spec 加第五段 `0 + 41 = 41`。三选的成本对比与门禁实测在 B.7.10。

### B.7.3 新发现 · C 的同元素约束是「本元素**或子元素**」，不是「本元素」

`api/extract/mod.rs::reexport_ok` 的判据是 FQN 前缀（`"{fqn}."`），故 `Views.Changes` 的 barrel 若 re-export `./flow/change-flow-graph`（owner = 子元素 `Views.Changes.Flow`），**取数面放行**。两个后果：

- B.1.4「barrel 只 re-export 本元素内文件 ⇒ 零新边」的前提取材过于宽松：子元素也在允许面内；
- `scan/ts.rs` 把 `export … from` 记 import fact ⇒ 跨子元素 re-export 会**真加边**（`Views.Changes → Views.Changes.Flow`，父 → 子合法），`71` 就不是不变量了。

结论：B 需要一条比 B.1.8（禁 hooks/lib）更宽的纪律——**每个 barrel 只 re-export 本元素直属文件**（`views/changes/index.ts` 只导 `change-detail-view` / `change-list-view`；`views/explores/index.ts` 只导那 3 个视图文件）。注意 hooks/lib 的禁令**不是机械兜底**：视图内 `hooks/` 的 owner 就是该视图元素（目录归属），`reexport_ok` 会放行，所以 B.1.8 靠人守。

### B.7.4 新发现 · `.i.ts` 生成链仍是断的（C 明示不含）

C 的 proposal「不含」写着「`.i.ts` 试点与 `ts_import_specifier` 接线」；`api/paths.rs:452` 的 doc 仍标注「**待接线留白**……本仓 `.i.ts` 生成物为零，该规则此刻无实例可验」。

所以 B 若选 B.7.2 的第三条收口，**B 必须自己接线 specifier 派生**（12 份同名主干 `index` 的生成物分落 12 个目录 + 12 处 `import type` 指向），这是 B.1.9 扇出表里缺失的一项。`INDEX.toml` 的 `generated` 记完整 repo 相对路径（`.../archi/src/lib.i.rs`），故 12 个 `index.i.ts` 不会撞车；`knip.json` 与 `NO_STATIC_CHECK` 已 ignore `src/**/*.i.ts`。

### B.7.5 新发现 · 覆盖判据（2026-11-17 新落）与 B 的 barrel 正面相遇

`frontend-component-element-coverage` 给 `repo-archi-model` 加了「覆盖判据（双向）+ §8 已知缺口清单」，而 B 要新增 **12 个源文件**。分档看：

| 目录 | 覆盖判据档 | 加 barrel 的影响 |
|---|---|---|
| `views/*/`（9 个） | 目录级「每目录一元素」 | 不受影响（barrel 在目录内，不产生新元素） |
| `components/ui/` | 「收敛为 1 元素」目录级 | 不受影响 |
| `views/changes/{flow,renderers,agent}` | 「非 `components`/`hooks` 且非测试源 ≥ 3」 | 15→16 / 7→8 / **2→3** |

第三行藏着一件巧事：`agent` 实测只有 **2** 个非测试源文件，今天**不满足「≥3」却已建元素**（spec 的 Scenario 写「`agent`（3）」），B 补 barrel 恰好把它补到 3——把既有的假话变成真话。同类假话还有「`flow`（13 源文件）」实测 **15**（`renderers` 7 正确）。这三处数字 MUST 在 B 的 spec delta 里重测，不能照抄。

### B.7.6 数字换段总表（起草时直接取这行）

| 量 | 探索文档旧值 | 现测 |
|---|---|---|
| `06-frontend` 元素 | 28（4 容器 + 24 叶） | **30（4 容器 + 26 叶）** |
| 前端关系 | 64（58 + 6） | **71（64 + 7）** |
| `Components.Agent` 端点边 | 6（2 出 4 入） | **7（3 出 4 入：`AgentInput` / `AgentSessionView` / `SessionPanel`）** |
| 全库 | 51 / 113 | **53 / 120** |
| `apiFindings` | 12 / 27 = 39 | **12 / 29 = 41** |
| `@/components/ui/*` 调用点 | 120 | **132（67 文件 / 35 模块）** |
| `api:check` 的 path 非空元素 | 41 / 42 | **44**（spec 原文） |

翻转后应为：`Components.Agent` 由 `domain` 变 `module`、`path` 由文件变目录 ⇒ **3 容器 + 27 叶**（总数 30 不变）、关系 **71 不变**、端点边 **7 条不变**（`owner_of` 用最长前缀，6 个子元素的文件型 path 优先于父的目录型 path，barrel `index.tsx` 仍归父）。

### B.7.7 已解除的旧遗留（不必再写进 B）

| B.6 里记的 | 现状 |
|---|---|
| 「A 的代码只在 worktree 未提交」 | 已归档；`entry_form` 探测与 `mod_path_attr` 均已退役（`entry_candidates` + `form_of` 取代） |
| 「B 先落会提前改判 A 的探测基线，属错序噪声」 | **不再成立**：候选派生已是官方解析链 ③，「补 barrel ⇒ ① 转 Resolved」正是设计意图 |
| 「`session-panel.tsx` 在模型中无元素、覆盖判据双向不成立」 | `frontend-component-element-coverage` 已补 `SessionPanel` + `Tooltip` 两元素与 7 条边 |
| B.1.9 第 2 项「`entry` 字段 + 四条判据」 | 已完成（A） |
| B.1.9 第 1 项「`extract/ts.rs` 支持 `export … from`」 | 已完成（C） |

### B.7.8 仍然悬着的（与 C 无关，B 自己的未决）

1. **`ui/index.ts` 的定位**（B.5 第一条，132 处）：改调用点 / 只放入口不改调用点 / 不给 ui 建 barrel（须改 `Components.Ui` 的 `path` 与判据档）——三选一；
2. `export type` 与 `export` 的分割形态（`verbatimModuleSyntax`）；
3. E（`module-entry-docs-semantics`）仍未裁决——但它管「入口文档」，B 管「入口文件」，可并行，不阻塞；
4. `init-check-archi` 只到一行 explore（「变更初始阶段，检查架构」），未起草，与 B 无交集。

### B.7.9 结论

**C 落地后 B 已无外部前置闸：可以启动，且启动前只剩三件必答**（B.7.2 的 `api` 口径、B.7.3 的 barrel 边界纪律、B.7.8 第 1 条的 ui 取舍）。把这三条裁掉即可起草 `openspec/changes/frontend-barrel-entry-migration/`；`repo-archi-model` 的 delta 用 B.7.6 的现测值，不要用探索文档里的旧数字。

### B.7.10 追问：留空（缺省 `declared`）能否过门禁？

**能，而且连 `status` 都不会被抬到 `warnings`**——原因比「apiFindings 不进 passed」更具体：

```text
gating_findings  (api/mod.rs:297)   纳管子集 = 交集
  ├─ element.api == Declared                    ← 前端 12 个元素满足（缺省即 declared）
  └─ paths::contract_path(fqn) 是盘上文件       ← 前端 12 个元素**零契约文件**，落选

⇒ gating = ∅
⇒ derive_status(...)：errors 空 → 有扫描 → warnings[] 空 且 gating 空 → **status = clean**
⇒ passed = errors.is_empty() && structural_errors.is_empty() → **true**
```

且 spec 已经把这条现状写死在 `desktop-archi-api-check:212`：「`status` 仍为 `clean`（纳管子集为空）」——**今天 12+29 条判据在场而 status 是 clean**，正是同一机制。所以「留空」在门禁面是**零代价**：

| 面 | 留空的结果 |
|---|---|
| 门禁 `passed` | `true`（判据永不产 `errors`） |
| 报告 `status` | 仍 `clean`（纳管子集要求契约文件在案） |
| `apiFindings` | `0 + 41 = 41`——12 条从 `apiEntryMissing` 平移到 `apiMissing`，**并列槽位如实呈现** |
| 架构页「契约」视图 | 判据轴 41 条（12→0 消失、29→41），用户可感知但无阻断 |
| 元素抽屉契约段 | `status = declared` + 三路径行（契约 md 缺失 ⇒ 呈「无契约…」文案） |

**代价落在 spec 而非门禁**，三处必改：

1. `desktop-archi-api-check:190` 的基线表（分母仍是 44 个 path 非空元素，但两桶变 **0 / 41**）；
2. 同文件 `:191` 的「四段历史链」——B 落地是**第五段** `0 + 41 = 41`，且这一段的性质是「净 0，全平移」（与 A 数据面那一步「真正收了两条债」正相反，必须在文里点明，否则下一个人读成债务清白）；
3. 同文件 `:212` 的 Scenario 逐条点名（8 视图目录根 + 3 子域 + `Ui` 由 ① 面平移至 ② 面）。

由此 B.7.2 的三选一实际收敛为：**留空是唯一零门禁代价的选项**；`api = none` 会**伪造**「无公共 API」；写契约则要先接 B.7.4 的 `.i.ts` 断链（工程量大一个数量级）。

### B.7.11 ui 的裁决材料（B.7.8 第 1 条）

> **已裁：B —— 建 barrel，只作入口声明 / 契约落点，132 处调用点保持直引**（后果见 B.7.13）。

B.7.10 的裁决（第五段 `0 + 41 = 41`）**隐含了 ui 必须建 barrel**：若 ui 不建，第五段只能写成 `1 + 40 = 41`，「12 个统一走 i」的口径当场破功。所以真正剩下的唯一问题是**改不改那 132 处调用点**。

先把 ui 的真实形状摆出来（实测）：

```text
components/ui/  39 个源文件
  ├─ 34 个被 `@/components/ui/<mod>` 引用（35 个模块名，button 独占 35 处）
  └─ 5 个零引用（连 ui 内部都不引）：button-group / context-menu / input-group / marker / sheet
export 形态：46 行（40 `export` + 6 `export interface`），`export default` 零，
             具名符号无重名 ⇒ `export * from './<mod>'` × N 条即可，无遮蔽风险
```

knip 现状：`ignore` 含 `src/components/ui/**` ⇒ **整个目录不参与死代码扫描**（连 `vp check` 的格式化也不管）。

| 选项 | 动作 | 收益 | 代价 |
|---|---|---|---|
| **A · 建 barrel + 改 132 处** | `ui/index.ts` 34 条 `export *`；132 处 import 改指 `@/components/ui` | barrel 成为**真实公共面**（被使用 ⇒ 不可能腐化）；12 个目录形态统一 | 132 处机械改写（可脚本），diff 大；5 个死模块不进 barrel（否则 barrel 躺 5 条永不生效的 re-export） |
| **B · 建 barrel 不改调用点** | `ui/index.ts` 34~39 条 `export *`，132 处原样 | 0 改写 | barrel **零调用者** + 目录在 knip ignore ⇒ 改名 / 删文件都不会有人报；「入口 = 公共点」降级为「抽取起点」（B.1.5 已预警） |
| **C · 不建 ui barrel** | 保持 1 条 `apiEntryMissing` | 0 改动 | 第五段变 `1 + 40`；「12 个统一」破功；或须给 `Components.Ui` 写 spec 豁免，等于同仓两套约定 |
| **D · A + 顺手清理 5 个死模块** | 删 `button-group` / `context-menu` / `input-group` / `marker` / `sheet` | ui 目录 = 真实公共面，barrel 无需排除项 | 删 5 个 shadcn 预留组件的决定权在 B 之外（可能是给未来留的） |

**A 与 B 的关键分水岭不是 diff 大小，而是「barrel 有没有校验」**：ui 在 knip ignore 内，只有「132 处都走 barrel」能让「barrel 恰等于真实公共面」成为**被使用的事实**而不是纸面规则；不改调用点就等于给 ui 开了一个例外，而这个例外的腐化是静默的。

另一条可选项（未列进表，代价最高）：把 `src/components/ui/**` 从 knip `ignore` 移除，让 barrel 进死代码扫描——那 5 个死模块 + 46 个导出里未被使用的会立刻全红，属独立清理，不适合塞进 B。

### B.7.12 逐目录 barrel 内容清单（knip 闭合的实测结果）

`views/*/index.ts` **在** knip 的扫描面内（`ignore` 只排 `ui` / `.i.ts` / `generated`），所以每个 barrel 的导出面 MUST 恰好等于「跨元素被 import 的名字」——多导一个就红，少一个调用点就断。逐目录实测如下（`explicit` = 文件里 `export` 的名字总数 / `needed` = 跨元素实际取用）：

| barrel | needed（barrel 只导这些） | 来源文件 | 该目录 explicit |
|---|---|---|---|
| `views/agent/index.ts` | `AgentDebugView` | `agent-debug-view` | 2（`AgentDebugViewProps` 不外露） |
| `views/agents/index.ts` | `AgentsView` | `agents-view` | 1 |
| `views/architecture/index.ts` | `ArchitectureView` | `architecture-view` | 1 |
| `views/config/index.ts` | `ConfigView` | `config-view` | 1 |
| `views/db/index.ts` | `DbInspectorView` | `db-inspector-view` | 2（props 不外露） |
| `views/info/index.ts` | `InfoView` | `info-view` | 1 |
| `views/changes/index.ts` | `ChangeDetailView` / `ChangeListView` | 2 文件 | 各 1 |
| `views/changes/agent/index.ts` | `ChangeChatPanel` | `change-chat-panel` | 4（composer 与两 props 不外露） |
| `views/changes/flow/index.ts` | 8 值（`mountMaterials` / `ChangeFlowGraph` / `DetailDrawer` / `buildFlowGraph` / `RunControlPanel` / `runStepNodes` / `unifiedRunEdges` / `unifiedRunNodes`）+ 3 类型（`DrawerSelection` / `FlowGraph` / `FlowMaterials`） | 7 文件 | **46 行 / 约 38 符号** |
| `views/changes/renderers/index.ts` | `ArtifactTabs` / `MarkdownDocRenderer` | 2 文件 | 13 行 |
| `views/explores/index.ts` | `ExploreDetailView` / `ExploreLayout` / `ExploreListView` | 3 文件 | 含 `ExploreDetailPane` / `ExploreRouteContext` / `useExploreRouteContext`（**均不外露**：前两者只被同元素文件直引或测试用） |
| `components/ui/index.ts` | 39 条 `export *`（见 B.7.13 的 34/39 待定） | 39 文件 | 46 行 / 零重名 |

**flow 是这条纪律的压力样本**：目录里 38 个公开符号，barrel 只该出现 11 个。这正好是 B.1.5 说的「knip 把入口 = 公共面变成机械可验的事实」——每次扩公共面都得显式改 barrel，而不是顺手 `export *`。同时也说明 B.1.3 的「`./flow/*` 7 处 → 1 条」实际是「7 处 → 1 条 barrel 导入取 11 个名字」。

调用点改写面（不含 ui 的 132 处）实测：

| 文件 | 现 | 后 |
|---|---|---|
| `src/routes.tsx` | 11 条文件级 import | 9 条（9 个视图目录各 1 条） |
| `views/changes/change-detail-view.tsx` | `./flow/*` 7 条 + `./agent/*` 1 条 | 2 条（`./flow` / `./agent`） |
| `views/changes/flow/detail-drawer.tsx` | `../renderers/artifact-tabs` 1 条 | `../renderers` 1 条 |
| `views/explores/components/explore-preview.tsx` | `../../changes/renderers/markdown-doc-renderer` 1 条 | `../../changes/renderers` 1 条 |
| 合计 | 20 条 | 13 条 |

### B.7.13 ui 选 B 的连带后果（须在 design 里显式写）

用户裁定「建 barrel 但不改调用点」，于是 ui 成为**唯一一个 barrel 零调用者**的目录。三件事必须在 design 里落地，否则将来被人当死文件删掉：

1. **写清这是有意识的例外**：`repo-archi-model` 里「入口 = 公共面 / 调用点经入口」的表述需为 `Components.Ui` 留一句「调用点走模块直引是刻意的，入口只承载声明与契约落点」；否则 spec 与实现互斥。
2. **barrel 自身留一行注释**（外部约束型，符合 AGENT.md 的注释纪律）：说明「本文件是目录模块的入口声明，视图中直引模块名是刻意选择，勿据此判死代码」。
3. **34 vs 39 待定**：`export *` 取「被引用的 34 个模块」还是「目录全部 39 个（含 5 个零引用死模块）」。`export *` 全导更贴「目录 = 模块」；只导 34 则与真实使用一致。两选一在 design 里定，并同步记那 5 个死模块为已知缺口（清理属独立变更）。

正面影响：因为 ui **建了** barrel，`entry_candidates` ③ 会命中它 ⇒ `Components.Ui` 退出 `apiEntryMissing` ⇒ B.7.10 裁定的第五段 `0 + 41 = 41` 成立。若选 C（不建），第五段就只能写 `1 + 40`。

### B.7.14 三件必答题的收敛结果

| # | 问题 | 裁决 | 落点 |
|---|---|---|---|
| 1 | `api` 口径 | **留空**（缺省 `declared`），第五段 `0 + 41 = 41` | B.7.10 |
| 2 | ui 是否改 132 处调用点 | **不改**（建 barrel 作入口声明 + 契约落点） | B.7.11 / B.7.13 |
| 3 | barrel 边界纪律 | **只导「跨元素被 import 的名字」**（knip 闭合），且 MUST NOT 跨子元素 re-export（否则真加边，见 B.7.3） | B.7.3 / B.7.12 |

三条合起来即 B 的完整口径：**12 个 barrel 逐个手挑导出名字（唯一例外 ui 用 `export *`）→ 12 条 `apiEntryMissing` 归零 → 20 处调用点缩成 13 处 → `Components.Agent` 翻转 → 第五段基线 `0 + 41`**。B 可以起草了。

### B.7.15 子域判据的 `components/` 排除面（第四条纪律）

> 复测时点：master `d965ba3`（`architecture view` 重构 + Bash card 两提交已入）。

`repo-archi-model:180-181` 的视图内子域判据是「`views/<视图>/<子目录>` 递归非测试源文件 **≥ 3**」**且**「路径不含 `components` / `hooks`」。前半条是唯一的入选条件，后半条才是把 `components/` 挡在门外的唯一机制——**没有它，本仓当场多出 6 个子域元素**：

| 目录（递归非测试源） | 数 | 若缺排除条 |
|---|---|---|
| `views/architecture/components/`（含 `graph/` 1 个） | **7** | 入选 |
| `views/config/components/` | **5** | 入选 |
| `views/explores/components/` | **4** | 入选 |
| `views/info/components/` / `agents/components/` / `agent/components/` | 各 **3** | 入选 |
| `views/changes/components/` | **2** | 不入选（且 2 已是 spec 写的 1） |

故 B 重写覆盖判据时，这句 MUST 原样保留并**扩到路径段口径**（不是「直接子目录不叫 `components`」）——`views/architecture/components/graph/` 是句子的第二个实例，它自身只有 1 个源文件、碰巧不入选，但「嵌在 `components/` 里的目录是否出局」今天没写清，是靠数量侥幸过关的。

**B 的 12 个 barrel 会移动上表的分母**（barrel 是非测试源）：`architecture/components` 7→8（已出局，仅证排除条承重）、`changes/agent` 2→3（B.7.5 的「巧事」：把既有的假话补成真话）、`changes/flow` 15→16、`changes/renderers` 7→8；`changes/components` 2 不变（无 barrel 计划）。

**可复算的验收判据（B.4 #1 的补强）**：跑下面这条命令，输出 MUST 恰为上表之外的四个候选（`changes/flow` / `changes/renderers` / `changes/agent` 三个建元素子域 + `architecture/agent` 一条 §8 已知缺口），且 `components/` 下任何目录 MUST NOT 出现：

```bash
cd packages/desktop/src/views
for d in $(find . -mindepth 2 -type d \( -path './*/components' -o -path './*/hooks' -o -path './*/lib' \) \
             -prune -o -mindepth 2 -type d -print | sort); do
  n=$(find "$d" -type f \( -name '*.ts' -o -name '*.tsx' \) ! -name '*.test.ts' ! -name '*.test.tsx' | wc -l)
  [ "$n" -ge 3 ] && echo "$n  $d"
done
```

现测输出（`d965ba3`）：`3 ./architecture/agent`、`3 ./changes/agent`、`15 ./changes/flow`、`7 ./changes/renderers` —— 与模型 30 元素 / §8 缺口表逐条对上。

**顺带一条 spec 漂移（B 的 delta 必改）**：同句把 `views/changes/components/` 写成「1 文件」，实测 **2**（`change-create-dialog.tsx` / `change-delete-dialog.tsx`，各伴一测试）；且该 Scenario 的 WHEN 只枚举 `views/changes/` 的子目录、THEN 却断言「各视图的 `components/` 目录不构成子域」——枚举面被写窄了，其余 8 个视图的 `components/` 从未进枚举。B 只须改数、MUST NOT 顺手扩枚举面（扩面即改判据，属独立变更）；但纪律文本 SHALL 明写「排除条对 `views/*/` 全体生效」，使窄枚举不成为将来漏判的依据。
