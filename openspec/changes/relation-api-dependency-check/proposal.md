# 提案: relation-api-dependency-check

> **变更**: relation-api-dependency-check
> **日期**: 2026-11-20
> **状态**: 待评估（explore 五项定案全在本变更交付；本提案另修正 explore 的三处事实、spec 基线的五处陈旧读数与本提案旧稿的一处结构性误判（`scan` 三面「逐条相同」）、补两处 explore 未覆盖的连带面，见「事实校正」与风险 R1–R12）
> **来源**: `explore.md`（requirement 类型；需求两条子句 = 模型侧 `[[relation]]` 选填 `apis` 并与代码实际依赖比对 / UI 侧画布连线可点看依赖详情）

---

## 问题

需求的两条子句是同一条链的两端：**声明面**要能说出「这条依赖用了哪几个名字」，**事实面**要能把「实际用了哪几个名字」取上来，**判定面**要把两者对差并按引入方档位出结论，**呈现面**要把差额与命中点给人看。今天这条链在四个位置都断着：

1. **声明面没有粒度**：`[[relation]]` 只有三键（`from` / `to` / `desc`，`RawRelation`）。模型能说「谁依赖谁」，不能说「依赖了什么」。缺的正是「元素对 → 符号」这一级。
2. **事实面在扫描层就丢名字**：`ImportFact` 只有 `specifier` / `line` / `type_only`；`RustResolver::resolve` 只回文件（`use x::Y` 的最长模块前缀被吃掉，尾段 `Y` 丢弃），`scan/ts.rs` 只取 `source.value`。名字在 AST 里明明有，一处没被接住——所以下游全管道（对账 / 报告 / DTO / 画布）都只能停在元素对粒度。
3. **归属缺口使最强名字面判不了**：`packages/desktop/src/types/generated/`（`bindings.ts` 105 KB / **136 个导出名** + `bindings.zod.ts` + `bindings.zod.check.ts`）不属于任何元素，而 `scan` 对「目标不可归属」的边直接丢弃 ⇒ 12 条前端 → 命令面关系今天恒为 `unfulfilledDeclaration`；即使填了 `apis` 也永远判不了。这是 `apis` 的**前置依赖**，不是可选优化。
4. **收数面与呈现面缺席**：报告只承载**结论**（门禁用的 errors / warnings / observations），「这条边实际用了哪些名字、命中在哪几行」无处可取；画布边无 label、无 `onEdgeClick`、无选中态，右栏 `CardHeader` 只有元素态（`ModelInfo`）一种。

### 事实校正（explore 的三处偏离 + spec 基线的五处陈旧读数 + 本提案旧稿的一处结构性误判，逐条给证据）

| # | 既有说法 | 实测 | 处置 |
|---|---|---|---|
| 1 | explore：模型 54 元素 / **122** 关系；`06` 表头「30 元素 / 71 关系」→「31 / 71」 | `graph/model/` 一级实测 **54 元素 / 123 关系**（`06` = 31 / 74，`03` = 8 / 24）；`06` 表头写的是「30 元素 / 72 关系」——**既有陈旧一行**（`Combobox` 元素与其两条边未计入表头） | 本变更同批把 `06` 表头写实为「**32 元素 / 74 关系**」（+1 新元素、关系数不变），并在 `repo-archi-model` 的计数表把「53 / 120」校正为「**55 / 123**（容器 9 + 叶 46）」（既有陈旧 + 本变更 +1 元素） |
| 2 | explore：api 判据基线：分母 44 / `docMissing` 43 → 45 / 44 | `path` 非空元素实测 **45**、`docMissing` 实测 **35**（10 份契约 md 在盘：`Foundation` / `ArchiRuntime` / 8 份 Agent 组件）；spec 里的「44 / 43」是两次变更之前的读数 | 新基线 = **46 / 36**（+1 个 `api = none` 且无 `doc` 的元素）。`desktop-archi-api-check` 的基线表与历史链同批改写，并把「44 / 43 已陈旧」写进留痕（MUST NOT 假装本次只 +1） |
| 3 | explore：「后端 G1 命中 40/49」（可写 `apis` 的覆盖面） | 那是 explore 自己写的只读探针的读数，**不是产品扫描器**：观察根是仓库根而 workspace 清单在 `packages/desktop/src-tauri/Cargo.toml`，`discover_crates` 只读 `root/Cargo.toml` ⇒ 零 crate ⇒ **Rust 事实面在本仓零命中**，49 条后端关系全部落 `unfulfilledDeclaration`（基线报告 62 条 observations = 49 后端 + 13 前端，其中 12 条是命令面边、1 条是既有边面缺口） | **不在本变更修**（独立能力，见「不做清单」与 R1）；但门禁口径、`message` 文案与 UI 空态必须显式承认「`used` 是 under-approx」，否则后端一旦填 `apis` 会被误读成「声明全没用」 |
| 4 | spec 基线：前端边恒等式「**74 = 67 叶→叶 + 7 barrel 端点边**（3 出 + 4 入）」 | `06-frontend.toml` 实测 barrel 端点边 **8 条**——3 出（`AgentInput` / `AgentSessionView` / `SessionPanel`）+ **5 入**（`Views.AgentDebug` / `Views.Changes.Agent` / `Views.Changes.Flow` / **`Views.Config`** / `Views.Explores`；`Views.Config` 的证据在 `views/config/components/config-chat-panel.tsx:1` 的 `@/components/agent` 具名导入）⇒ 恒等式实为 **74 = 66 + 8** | 本变更重写该段时同批写实（`7` → `8`、`67` → `66`、入边清单补 `Views.Config`）；这是**既有陈旧**（该入边早已存在、旧读数未追改），不是本变更新增的边 |
| 5 | spec 基线：「`model.warnings` 为空」 | 本 change 的基线报告（`reports/archi/20261120T1042330799149Z.json`）实测 `model.warnings` **9 条 `noPath`**（9 个容器元素无 `path`，正是「容器 `path` 恒空」纪律的必然结果）、顶层 `warnings[]` 为空 | `repo-archi-model`「图自洽与停摆禁令」重写时把两层分辨写清（顶层 `warnings[]` 空 / `model.warnings[]` 9 条 `noPath` 前后不变）；MUST NOT 继续写成「`model.warnings` 为空」 |
| 6 | spec 基线：清单守卫「首锚 `ActivePhase`、尾锚 `WarningKind`」 | `bindings/mod_test.rs:1311` / `:1313` 实测首锚 `ActivePhase`、**尾锚 `NodeMark`**（`WarningKind` 已随 `docs` 三源退役，尾锚回落到封闭词汇末项） | `desktop-ipc-type-bindings` 的清单表同批写实；计数 `104 → 105` 与插位（`ArchiRelation` 之后、`ArchiScanReport` 之前）实测成立 |
| 7 | spec 基线：`Components.Ui` 入口「39 条 `export *`」 | `packages/desktop/src/components/ui/index.ts` 实测 **41** 条 `export *`（第 2–42 行，逐文件一条） | 本变更重写该 scenario 时按实测写 **41**；结论（`Ui` 零端点边）不变 |
| 8 | spec 基线：8 份前端契约的散文「公共面」表「27 条数据行」 | 10 份契约中 8 份前端契约实测 **28** 条数据行（`Agent.md` 8 + 7 个子组件 6 / 2 / 2 / 3 / 3 / 2 / 2） | `desktop-archi-api-check`「装载期名字清单闸与判据面的分工」重写时按实测写 **28**；结论（留白期零判据）不变 |
| 9 | 本提案旧稿：报告读数「变化恰四处」，`scan` 三面「与本变更前逐条相同」 | **结构性不可达**（不是读数陈旧）：`scan.unmatchedFiles[]` 本就收纳 `gate = off` 归属文件与无主文件（本仓 `gate` 全 `off` ⇒ 412 条全在此），`excluded[]` 收纳既有排除规则命中项 ⇒ 本方新增的 **3 个源文件**必然进 `unmatchedFiles[]`、**11 份测试 / 语料**（1 个 `**/*_test.rs` + 8 个 `**/tests/**` 语料 TS + 2 个前端 `**/*.test.ts(x)`）必然进 `excluded[]`。实测：既有条目零增删改、新增条目全部命中既有规则 ⇒ `scannedFiles` / `unmatchedFiles` **412 → 415**、`excluded` **341 → 352**、候选（`scannedFiles + excluded`）**753 → 767** 自洽（`tasks.md` 记的 341 是 20:25 的采集快照——那 11 份测试 / 语料在 20:44–20:56 才落地，当时新增候选只有 3 个源文件） | AC-8 改写为「**既有条目零增删改 + 新增条目全部命中既有规则**」并写实读数；§7 报告面行与测试表「`scan` 三面」行同批改写；`design.md:326` / `:405`、`test-design.md:32` / `:713` 与 `tasks.md:83` / `:84` 的对应留痕属各相位产物，在重跑时按同一账目校正（本相位不改它们） |

### 一句话结论

难点不是「加个字段」，而是把**实际依赖**从元素对粒度升到**符号**粒度：接住名字（扫描）→ 拿到声明（模型字段）→ 对差并按引入方定级（判定）→ 按需取（第 9 条命令）→ 点开看（右栏两态面板）。同时必须先把 `types/generated/` 归属掉，否则最强的名字面永远判不了。

## 提案

以「**中继元素前置归属 → 模型第四键（只许名字）→ 名字事实进扫描 → 两桶对差（一进档、一观察）→ 按需取数的第 9 条命令 → 右栏两态面板**」六件同批收口，全程守住三条既有承诺：**报告形状零变化（`version` 仍 3 / 16 份金样零变化）**、**判定单源（求差抽成纯函数，drift 与命令都只调它）**、**画布零 watch 零轮询**。

### 1. 前置：中继元素 `App.Frontend.GeneratedBindings`

逻辑关系是「前端 → 后端」，代码关系是「前端 → 生成物 → 后端」。不归属生成目录，`apis` 在这族边上恒为「判不了」。

```toml
# 06-frontend.toml
[element."App.Frontend.GeneratedBindings"]
kind = "module"          # 复用四值之一，不引入第五值 generated
gate = "off"             # 只作 `to`，定级永远看引入方 ⇒ 本档无定级语义
api = "none"             # 生成物是唯一真相：零契约 md / 零 schema / 零 fence
path = ["./packages/desktop/src/types/generated/"]
summary = "IPC 命令面与出线 DTO 的 TypeScript 投影（tauri-specta 生成，勿手改）"
```

- 12 条 `to = "App.Backend.Shell"` 的前端关系改 `to = "App.Frontend.GeneratedBindings"`，`desc` 由「经生成的命令面类型取数与调用」微调为「经生成的 IPC 绑定取数与调用」。
- **不声明 `entry`**：生成目录没有 `index.ts`，它是一组叶子文件而不是模块入口 ⇒ `entry_state = Absent`。`api = none` 已退出入口判据 ⇒ 零 `apiEntryMissing`；`Absent` 只是数据事实（由本变更在 `desktop-archi-api` 的实测场景里如实写为「45 Resolved + 1 Absent」）。
- **上游「生成链」不声明**：`GeneratedBindings → Shell / Core.Archi / …` 不是 import 事实，声明了立刻变永久 `unfulfilledDeclaration`——与本次要消掉的噪声同类。要表达它得先有「生成链对账」判据（`bindings.ts` 的 invoke 串集（snake）× `#[tauri::command]` 名集，零格式转换），属独立能力。
- **被否决的替代**：把生成目录挂成 `App.Backend.Shell` 的第二条 `path`（元素根混语言、与后续导出集校验永久互斥、文件树里 Shell 名下出现 TS 文件）；给 `[[relation]]` 加 `kind = "generated"` 语义位（三处随动且上游仍无判据）；不归属（12 条边永久灰虚线）。

### 2. 模型字段：`[[relation]]` 第四键 `apis`（只许名字）

```toml
[[relation]]
from = "App.Frontend.Views.Architecture"
to   = "App.Frontend.Components.Agent"
desc = "复用会话视图与输入框"
apis = ["AgentInput", "SessionPanel", "SessionPanelState"]   # 选填；省略 = 不声明 API 粒度
```

- **取值域 = `to` 元素的代码导出名**：契约声明面全仓仅 **11** 个名字（`Foundation` 9 个 op + `ArchiRuntime` 1 个 op / 1 个 `$defs`，其余 8 份前端契约 `props = 0`），且真正被跨元素依赖的 `config_path` / `Layout` / `ReportSlot` 全在「契约面留白」名单里 ⇒ 「只许契约声明名」不是略窄，是不可用。模型里写的名字必须与扫描事实同一名字空间，否则就要多一层映射 = 第二份真相。
- **三条护栏只开两条**：① **只许名字**（无签名 / 无参数 / 无类型形状 / 无 `::` / 无空白；值非空、同名重复拒收）；③ **可空引用**（选填；`apis = []` 拒收——与省略同义，静默接受等于让「看起来声明了」与「没声明」两种状态混同）。②「⊆ 导出集机械证成」**不做**：省掉导出集装载，并避开「中继元素 `api = none` 被 `load_contracts` 跳过 ⇒ 导出集为空 ⇒ 12 条前端关系全假阳」这条必然冲突。
- **红线的区分（不是破例）**：既有「协议正文不经模型」的对象是**元素**、理由是「存一份正文会漂移」。`relation.apis` 在两点上都可分：位置在**关系**（消费方的依赖选择，不是提供方的自我描述）、内容**只有名字**（无签名 / 类型形状 / 接口定义）。名字的权威仍在代码导出集与契约 fence，模型只回答「选了哪几个」。正文类字段（含签名、`::` 限定路径、类型形状）仍被拒。
- **`apis` 是开放值集，不进封闭词汇对拍**（同 `doc` / `entry`）：它不落进 `desktop-archi-model`「消费面词汇集一致性对拍」的六组封闭词汇，也不需要 `ALL` 常量。

### 3. 名字事实：把 `specifier` 旁边的名字接住

`ImportFact` / `DependencyEdge` 各加 `names: Vec<String>`（去重、字典序，空 = 取不到名字）。取数口径一次定死：

| 形态 | 产名 | 依据 |
|---|---|---|
| `use a::B` / `use a::{B, C}` | `B` / `B`,`C` | 最长模块前缀被吃掉后剩下的尾段就是符号名 |
| `use a::B as W` | `B`（**被引入名**，不是别名） | `flatten_use_tree` 的 `Rename` 臂已是此语义 |
| `use a::*` / `use a::{self}` | **无名字** | glob 无符号；`self` 即模块本身，最长前缀吃掉全部段 ⇒ 尾段为空 |
| 内联限定路径 `exec::SystemCommand::bare(..)`（未写 `use`） | **本变更不取**（G2） | 尾段可能是模块名（`layout` / `dto` / `port`），需噪声过滤面；属独立增量 |
| `import { A, type B } from 'x'` | `A`,`B` | 具名 specifier；`type` 前缀剥除（与「type-only 算架构依赖」同权） |
| `export { A } from 'x'` | `A` | 与 `import` 同取法 |
| `import X from 'x'` / `import * as ns from 'x'` / `export * from 'x'` | **无名字** | 默认导入绑的是本地变量名（不是导出名）、命名空间与全量重导出没有可枚举的导出名——静默产名会造假名字 |
| 方法调用 `.is_set()` | **写不出来** | Rust 侧真实粒度 = 类型 + 自由函数（`use` 只引类型） |

**比对逐字符**，禁大小写折叠与驼峰归一：判定侧常驻第二处映射规则正是要避免的东西；跨语言转换在本仓几乎不发生（前端 import 生成物拿的是 Pascal 类型名 + 一个 `commands` 桶，camel 化的命令 key 只在成员访问里，低于 `apis` 射程）。

### 4. 判定：两桶，档位由取数精度决定

```
Declared（模型 apis）             Used（扫描事实，under-approx）
   ├── used ∖ declared  → 有证据（import 是硬事实）
   │                      按 from.gate：strict → errors / warn → warnings / off → 不出现
   │                      词汇：ArchiDriftKind::UndeclaredApi（ALL 4 → 5）
   └── declared ∖ used  → 无证据（≠ 反证）⇒ observations[]（unusedDeclaration）
                          永不进 status / passed
```

- **不对称的理由是取数精度**，不是审美：方法调用、内联限定路径、glob、默认导入都能让真依赖「看不见」；「没看见」不能判红。`declared ∖ used` 天生混两种成因（a 真没用；b 写错或看不见），与「同一个值 MUST NOT 承载两种缺口」正面冲突 ⇒ **显式豁免**并写明理由（成因不可机械分辨，因为取数面是 under-approx、下限不得靠猜）；报告文案与 UI 文案 **MUST NOT 只写「没用」**，必须并列两种可能。
- **求差单点**：`api_buckets(declared, used)` 纯函数（`archi::drift`），drift 对账与第 9 条命令都只调它；命令层零领域规则。
- **零噪声**：关系省略 `apis` ⇒ 两桶皆空 ⇒ 报告与既有 16 份金样逐字段相同（冷启动纪律）。新增语料 case 只加不改。

### 5. 收数面：第 9 条命令 `archi_relation`

```
archi_relation(root, from, to) -> Result<ArchiRelationDetail, String>
  ├ from / to / description / mark / reason     ← 与 ArchiRelation 同源
  ├ modelFile / modelLine                       ← 模型声明行（点开定位）
  ├ declared          : 模型 apis（空 = 未声明）
  ├ used              : 扫描事实名（去重、字典序）
  ├ usedNotDeclared / declaredNotUsed            ← 与 drift 判据同一纯函数求差
  └ occurrences       : 复用 ArchiDriftOccurrence 形状（file / line / specifier / typeOnly）
```

- **`ArchiRelation` 只加 `apis: Vec<String>`**（模型拷贝、字段不省略）：画布拿图即可显示声明清单，不必为新面板预取；`modelFile` / `modelLine` 只在新 DTO 出线（不为 123 条边整体加两个字段）。
- **定向扫描**：只扫 `from` 元素 `path` 下的文件，机制借 `ScanInput.mode = Incremental` 的 `changed` 白名单（字面语义就是「只扫其中的源文件」）；目标归属走既有 `OwnerIndex`（不解析目标）。零写盘、不进报告。
- **为什么不进报告 facts 段**：报告 `version` 3 → 4 + 16 份金样机械重写 + 绑定 + 前端载荷语料全动，换来的只是「全量边的审计价值」；点开即取、按需付费才是本需求的形状。报告只承载结论这条分工不破。

### 6. 呈现面：右栏面板两态 + 边可点

```
CardHeader：元素态 ModelInfo（现状）  ⇄  关系态 RelationInfo（新增）
                  ▲ 点节点                        ▲ 点边（同一块面板位，点节点回元素态）

RelationInfo
  关系   Views.Architecture → Components.Agent      [标记徽标：unfulfilled / exempt]
  说明   复用会话视图与输入框
  声明 API (3)  AgentInput · SessionPanel · SessionPanelState    ← ArchiRelation.apis（零取数）
  实际使用 (3)  …                                               ← 点击后取
  用了没声明 / 声明没用                                          ← 两桶（按 gate / 观察项语义）
  命中点  archi-chat-panel.tsx:12  import { SessionPanel } from '@/components/agent'
  模型    graph/model/06-frontend.toml:512  [查看]
```

四种空态必须彼此可辨：① 未声明 `apis`（「未声明依赖 API（选填）」，**不发起取数**）；② 扫描面无该关系的 import（= `unfulfilledDeclaration` 的事实面）；③ 边被豁免（`via = config / inline` + reason）；④ 取数失败（行内错误 + 重试，图与元素面板照常渲染）。

**画布三触发口径的开窗**：`desktop-archi-canvas` 规定取数只许三种触发（挂载 / `root` 变化 / 显式刷新），禁 watch 与轮询。点边取详情是**显式用户动作发起的按需查询**，既非 watch 也非轮询 ⇒ 合规，但必须写进 spec，否则会被当成第四个触发点。

### 7. 数据面连带（不做这些，交付面自相矛盾）

| 面 | 变化 |
|---|---|
| 模型 | +1 元素、12 关系改指、`06` 表头「30 / 72」→「32 / 74」（同批修正既有陈旧） |
| 裁定记录 ADR | §1 / §9 计数写实（53 / 121 → **55 / 123**（容器 9 + 叶 46），`06` 30 / 72 → **32 / 74**，前端恒等式 65 + 7 → **66 + 8**）；§8 的「命令面边落 `observations`」段改写为「已归属生成物元素、边即兑现（12 条命令面边离开观察项）」 |
| 生成物 | `pnpm api:export` 重导：`graph/INDEX.toml` **45 → 46 条**（新元素 `status = absent`）、`bindings.ts` / `bindings.zod.ts` 随新命令与新 DTO 变化——**全部为生成物，零手改** |
| api 基线 | 分母 45 → **46**、`docMissing` 35 → **36**（其余十二值仍 0、`status = clean`、`passed = true`）；spec 里「44 / 43」的陈旧读数在本变更同批改写并留痕 |
| 知识文本 | 关系四键（`from` / `to` / `desc` / `apis`）+ 一句「`apis` 只写名字、可留空」；`apis` 与 `doc` / `entry` 同列（开放值集，不进派生词表） |
| 报告面 | **形状零变化**：字段集与 `version` 不变；本仓读数变化 = 语义面四处（`model.elements` 54 → **55**（`byGate.off` 同）、`observations` 62 → **50**（12 条命令面边转兑现）、`apiFindings` 的 `docMissing` 35 → **36**（+1 个 `api = none` 且无 `doc` 的元素）、`modelHash`（模型字节变））+ **扫描面按条目级账目**（`scannedFiles` / `unmatchedFiles` 412 → **415**、`excluded` 341 → **352**、候选 753 → **767**；既有条目零增删改、新增条目全部命中既有规则，逐条见 AC-8）；其余逐字不变（`warnings` / `errors` 空、`model.warnings` 9 条 `noPath`、`matched` 0、`unresolved` 0、`status = clean`、`passed = true`） |

**读数写实边界（沿用既有惯例，不是漏项）**：本变更只把**数据纪律 owner 与直接判据面**的读数写实——模型表头、ADR `§1` / `§9` / `§8`、`repo-archi-model`、`desktop-archi-api-check`（基线表 + 读数零漂移条）、`desktop-archi-api`（入口面实测）。其余 capability spec 内的**历史读数**（如 `desktop-frontend-module-entry` 的「71 条 / 24 个源 / 44 个 `path` 非空」、「53 元素 / 44 叶」、`desktop-archi-contract-panel` 的「43 条存量」）**不逐份追改**：它们是各自变更交付时的读数，且既有「53 / 120 vs 54 / 123」的陈旧早已在案（同一条处置）。任一 spec 若要按当前模型重算全库读数，属独立的数据面清理变更。**反面口径**：本变更**重写到的段落**不适用本条豁免——凡落在 `specs/` 四份受影响段落（`repo-archi-model` 计数与前端边、`desktop-archi-api` 入口实测、`desktop-archi-api-check` 基线、`desktop-ipc-type-bindings` 清单）内的读数，一律按盘上实测复述（含「事实校正」表九处）。

### 决策表

| 编号 | 决策 | 值 |
|---|---|---|
| D1 | 承载面 | 六件同一 change；实现序 = 中继元素 → 模型字段 → 名字事实 → 两桶判定 → 命令面 → UI → 数据面连带（explore §9 的 T0–T6） |
| D2 | 中继元素 | `App.Frontend.GeneratedBindings`（`module` / `off` / `api = none` / `path` = 生成目录 / 无 `entry` / 无 `doc`）；12 条关系改指 |
| D3 | 上游生成链 | 不声明（非 import 事实；声明即永久 unfulfilled） |
| D4 | `apis` 取值域 | `to` 元素的代码导出名（`ApiSymbol.name` 名字空间）；逐字符比对，禁大小写折叠 / 驼峰归一 |
| D5 | 护栏 | 只开 ①「只许名字」③「可空引用」；②「⊆ 导出集机械证成」不做 |
| D6 | 值面 | 非空 / 无空白 / 无 `::` / 同名重复拒收 / `apis = []` 拒收；开放值集，不进封闭词汇对拍 |
| D7 | 名字取数 | Rust 尾段（别名取原名；`self` / glob 不产名）；TS 具名（`import` + `export … from`，`type` 剥除；default / namespace / `export *` 不产名）；内联限定路径（G2）不做 |
| D8 | 门禁口径 | `used ∖ declared` → `ArchiDriftKind::UndeclaredApi`（`ALL` 4 → 5，按 `from.gate` 三档）；`declared ∖ used` → `ArchiObservationKind::UnusedDeclaration`（observation，永不进门禁） |
| D9 | 报告 | 形状零变化、`version` 仍 3、16 份既有金样零变化；新 case 只加不改 |
| D10 | 求差单点 | `api_buckets(declared, used)`；drift 与 `archi_relation` 共用，命令层零领域规则 |
| D11 | 收数面 | 第 9 条命令 `archi_relation`；新 DTO `ArchiRelationDetail`；`ArchiRelation.apis`（不加 `modelFile` / `modelLine`）；`ArchiDriftOccurrence` 复用 |
| D12 | 定向扫描 | `from` 侧文件白名单 + `Incremental`；零写盘、不进报告、不缓存 |
| D13 | UI | `CardHeader` 两态（`ModelInfo` ⇄ `RelationInfo`）；四空态；边失配徽标（`EdgeMark` 第五值）**不做** |
| D14 | 三触发口径 | 边点击的按需查询由显式用户动作发起，不计入三触发；仍 MUST NOT 自动重取（watch / 轮询） |
| D15 | 数据面连带 | 模型 / ADR / `INDEX.toml` / `bindings.ts` / api 基线 / 知识文本同批（§7） |
| D16 | 知识文本 | 关系四键 + 一句 `apis` 纪律；体量锚与派生段零扩张（不新增词表） |
| D17 | 版本纪律 | `packages/desktop/package.json` patch +1（`0.7.4` → `0.7.5`）；`src-tauri/Cargo.toml` 不跟 |
| D18 | Rust 事实面缺口 | 不在本变更修（workspace 清单发现属独立能力）；只在门禁文案、空态与 spec 里如实登记 |

### 明确不做（知情边界，非遗漏）

| 不做 | 理由 | 何时再做 |
|---|---|---|
| **Rust 工作区清单发现**（观察根无 `Cargo.toml` 时的 crate 发现） | 与 `apis` 判据正交：修它会让 49 条后端声明转兑现并暴露新的后端未声明依赖，是独立的行为变更 | 后端关系要填 `apis` 之前 |
| ⊆ 导出集机械证成（护栏②） | 导出集装载成本 + 与中继元素 `entry` 冲突（`api = none` 被 `load_contracts` 跳过 ⇒ 12 条前端关系全假阳） | 出现「错字被当观察项忽略」的真实事故 |
| 内联限定路径取数（G2） | 尾段含模块名噪声，需过滤面 | 后端 9 条「只有内联路径」的边被认为必须可写 `apis` |
| 标识符集旁证（`declared ∖ used` 的廉价升级） | 引入「文本出现」这一弱事实族 | 观察项长期无人看 |
| 生成链对账（`bindings.ts` invoke 串 ↔ `#[tauri::command]`） | 独立能力，不影响本需求判据完整性 | 出现「后端加命令忘重导」事故 |
| 边失配徽标入词汇（`EdgeMark` 第五值） | 封闭四值 + 三处随动；只作 label 文案又与「画布只认标记不认判定」摩擦 | 需要画布上一眼扫出失配 |
| 模型结构 diff 覆盖 `apis` | 关系身份仍是 `(from, to)` | 归档确认面要呈现「依赖面变更」 |
| `apis` 在生成物面填写 | 生成物是唯一真相，手写清单必然漂 | 永不（生成物面留空） |
| 报告 facts 段 / 报告 `version` 4 | 16 份金样 + 绑定 + 前端语料全动，换不来点开体验 | 需要全量边审计时 |

## 能力

### 新增的能力

无。五项定案分别落在既有能力的既有语义面上：字段与命令面归 `desktop-archi-model`，名字事实与两桶判定归 `desktop-archi-drift`，面板两态与按需取数归 `desktop-archi-canvas`，基线与入口实测归 `desktop-archi-api-check` / `desktop-archi-api`，模型数据与计数纪律归 `repo-archi-model`，知识文本归 `desktop-archi-agent`，清单守卫随动归 `desktop-ipc-type-bindings`。单拆 `desktop-archi-relation-api` 之类会与这八份的既有分工全面重叠。

### 修改的能力

- **desktop-archi-model**（2 条）
  - **MODIFIED**「架构模型文件格式与字段封闭」：关系字段面三键 → **四键**（`from` / `to` / `desc` / `apis`）；`apis` 值面四条（只许名字 / 可空 / 非空去重 / `apis = []` 拒收）；补「关系 `apis` 不是协议正文」的区分句与两条护栏；写明 `apis` 是开放值集、不进封闭词汇对拍。`deny_unknown_fields` 语义不变（`api = […]` 仍报 `unknownField`）。新增三 scenario（四键解析与缺省 / `apis` 形状五类拒收（含显式空数组）/ 与元素协议正文的边界区分），既有十 scenario 逐字保留。
  - **MODIFIED / RENAMED**「IPC 命令面契约（八命令）」→ **九命令**（标题随动，delta 里以 `RENAMED` 记 FROM/TO + `MODIFIED` 携新标题内容）：表格加 `archi_relation`（入参 `root` + `from` + `to`，出参 `ArchiRelationDetail`）；`ArchiRelation` 加 `apis`（模型拷贝、字段不省略）；计数句、`all_commands!` archi 段 8 → 9、绑定清单随动一并写明；「写命令恰两条」不变。
- **desktop-archi-drift**（3 条）
  - **MODIFIED**「三类漂移与定级只看引入方」：补两条**名字级对偶**——② 的 API 粒度（`used ∖ declared` 按 `from.gate` 三档出 `undeclaredApi`）与 ① 的 API 粒度（`declared ∖ used` → 观察项，永不进门禁）；「定级只看引入方」逐字保留。新增四 scenario（名字级对偶的三档定级 / 声明多余只作观察 / 关系缺失时不出 `undeclaredApi` / 同一元素对的两条观察并存），既有五 scenario 逐字保留。
  - **MODIFIED**「报告结构与 status 五值」：漂移条目封闭词汇 **4 → 5**（+ `undeclaredApi`）；观察项 `unusedDeclaration` 由「保留未产」改为「`declared ∖ used` 时产出」；补「两桶成因不可机械分辨」的显式豁免（S2）与 `message` 必须并列两种成因一句；`version` 仍 3、字段集逐字不变。
  - **ADDED**「依赖 API 两桶对账与求差单点」：`api_buckets(declared, used)` 纯函数（drift 与命令共用、零第二套判据）；`used` 是 under-approx 的三条已知上限（方法调用 / 内联限定路径 / 观察根无 workspace 清单时 Rust 事实为零）；`occurrences` 复用既有聚合形状、名字进 `message`（报告零新字段）。
- **desktop-archi-canvas**（2 条）
  - **MODIFIED**「单一画布组件与两处挂载点」：三触发纪律保留原样，补一条显式开窗——**边点击的按需查询由显式用户动作发起，不计入三触发；仍 MUST NOT 缓存失效以外的自动重取**（S3）。
  - **ADDED**「关系信息面板（`RelationInfo`）与边点击」：`CardHeader` 两态与切换语义（点节点回元素态）；面板七段（关系 / 说明 / 声明 API / 实际使用 / 两桶 / 命中点 / 模型行号）；四空态彼此可辨；取数失败只落行内错误；`EdgeMark` 词汇零扩张、节点标记仍不呈现。
- **desktop-archi-api-check**（2 条）
  - **MODIFIED**「契约判据的验证语料与报告数基线」：分母 45 → **46**、`docMissing` 35 → **36**（33 declared + 3 none）；把「44 / 43 已陈旧」写进留痕（既有 `Combobox` 元素与 10 份契约落地使读数早于 spec）；其余十二值仍 0、`status = clean`、`passed = true`；语料守护与 `api/**` sidecar 13 份零变化。
  - **MODIFIED**「装载期名字清单闸与判据面的分工」：读数随仓演进、口径不变——`docMissing 36`（33 + 3）/ `apiEntryMissing 0` / `apiMissing 0` / 其余十值 0，并把历史链的「第六段 43（41 + 2）」改写为**当前段读数**（「更早一段」的说法保留成因，MUST NOT 让旧读数与新读数并排而无解释）；语料守护追加**新 case 登记**（既有 16 份金样逐字节不变、新增 `api-*` case 是新增文件）；**零改动面表重写**——「判据面零改动」的范围收窄为 `api/**`（`check.rs` / `severity()` 三档 / `gating_findings` 入选判据 / 纳管子集 / `ArchiApiFindingKind` 十三值），`drift/report.rs` 与 `drift/mod.rs` **不在**该零改动面内（它们的 `ArchiDriftKind` +1 值与两桶装配由 `desktop-archi-drift` 登记）；`bindings:export` 的幂等范围同步收窄到判据面（`ArchiDriftKind` 类型随生成物更新）。
- **desktop-archi-api**（1 条）
  - **MODIFIED**「生成物落点与入口查找」：本仓入口面实测由「44 / 0 / 0」改写为「**46 个 `path` 非空元素 / `Resolved` 45 / `Absent` 1 / `Invalid` 0**」——新增的 `Absent` 恰为中继元素（生成目录无 `index.ts`，且 `api = none` 已退出入口判据 ⇒ 零 `apiEntryMissing`）；三处显式 `entry` 仍是唯一实例。
- **repo-archi-model**（4 条）
  - **MODIFIED**「基线落位与模型文件划分」：全基线 **55 元素 / 123 关系**（容器 9 + 叶 46）、`06` 段 **32 元素 / 74 关系**；新增「生成物中继元素」一类（`kind = module` / `api = none` / `path` = 生成目录 / 无 `entry`）；`api = none` 两处 → **三处**；计数复述三处（表头 / 计数表 / ADR）同批写实并留痕「53 / 120 → 55 / 123 含既有陈旧校正」。
  - **MODIFIED**「前端关系只取 import 证据」：`types/generated/bindings` 的命令面映射由 `App.Backend.Shell` 改指 `App.Frontend.GeneratedBindings`；12 条命令面边由「声明式约定、扫描面无证据 ⇒ 落 `observations`」改为「**有主 ⇒ 兑现**」；计数恒等式写实（74 = **66** 叶→叶 + **8** barrel 端点边，3 出 + 5 入）。
  - **MODIFIED**「图自洽与停摆禁令」：入口文档债读数 **35 → 36**（spec 旧读数 43 是两次变更之前的数，同批留痕）、`observations` 62 → **50**（12 条命令面边离开观察项）；其余（`diagnostics` 空、顶层 `warnings` 空、`model.warnings` 恰 9 条 `noPath` 前后不变、`status = clean`、`passed = true`）逐字保留。
  - **MODIFIED**「覆盖判据的机械核对与漂移纪律」：判据表新增第 4 行「生成物中继元素」（判据 = `path` 恰为生成目录、`api = none`、零 `entry`、零关系出边），使「判据之外零元素」在新元素上仍成立；已知缺口清单零变化。
- **desktop-archi-agent**（1 条）
  - **MODIFIED**「架构会话的系统知识注入」：关系键面由三键改四键 + 一句「`apis` 只写名字、可留空（生成物面留空）」；`apis` 与 `doc` / `entry` 同列（开放值集、不进派生词表）；体量锚与派生段两处目录零扩张。
- **desktop-ipc-type-bindings**（1 条）
  - **ADDED**「archi 族第九条命令与 `ArchiRelationDetail` 的清单随动」：`COMMAND_NAMES` / `COMMAND_WRAPPERS` **56 → 57**、`DTO_TYPES` **104 → 105**、`ARCHI_COMMANDS` **8 → 9**、`ARCHI_DTO_TYPES` **21 → 22**；尾部追加序（`archi_relation` 插在 `archi_api_generate` 之后、`read_file` 之前）、DTO 插位（`ArchiRelation` 之后、`ArchiScanReport` 之前）、首尾锚（`ActivePhase` / **`NodeMark`**——`WarningKind` 已退役，尾锚随封闭词汇末项）与「清单只作连带核对、不作语义判据」口径逐字保留。

其余能力零改动，理由：

- **desktop-archi-gate**：门禁 runner 与落盘零改动；本变更只让报告里多一个漂移词与一个观察项产出条件，`passed` 判据零变化。
- **desktop-archi-file-tree**：新元素在文件树里按既有 `elementRoot` 机制标记，零契约改动。
- **desktop-archi-contract-panel / desktop-archi-chat**：契约视图与页内会话零改动。
- **desktop-agent-execution / desktop-checks-domain / desktop-crate-layout**：零新工具、零新 crate、零新依赖（`archi` 既有依赖全部够用）。
- **desktop-workspace-config**：`KNOWN_KEYS` / schema 镜像零改动（架构域看护配置未变）。

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `.dev-team/architecture/graph/model/06-frontend.toml` | +1 元素 `App.Frontend.GeneratedBindings`；12 条关系的 `to` 与 `desc` 微调；表头注释「30 元素 / 72 关系」→「32 元素 / 74 关系」 |
| `.dev-team/architecture/decisions/0001-model-granularity.md` | §1 / §9 计数写实（**55 / 123**（容器 9 + 叶 46）；`06` 32 / 74；§9 前端恒等式 65 + 7 → **66 + 8**）；§8 命令面边段落改为「已归属生成物元素、边即兑现」；新增「生成物中继元素」判据一行 |
| `.dev-team/architecture/graph/INDEX.toml` | `pnpm api:export` 重导（非手改）：45 → 46 条，新元素 `status = absent` |
| `packages/desktop/src-tauri/crates/core/archi/src/parse/toml.rs` | `RawRelation` 加 `apis: Option<Vec<String>>`；值面四查（非空 / 无空白 / 无 `::` / 无重复）与 `Some(vec![])` 拒收 → `invalidValue` 诊断（`deny_unknown_fields` 不变） |
| `packages/desktop/src-tauri/crates/core/archi/src/model.rs` | `Relation` 加 `apis: Vec<String>`（`From` 归一后形态；`file` / `line` / `span` 零变化） |
| `packages/desktop/src-tauri/crates/core/archi/src/scan/mod.rs` | `ImportFact` / `DependencyEdge` 各加 `names: Vec<String>`；原样透传到 `DependencyEdge`（去重 + 字典序单点） |
| `packages/desktop/src-tauri/crates/core/archi/src/scan/rust.rs` | `extract` 产出名字（`flatten_use_tree` 一次产出「路径 + 尾段」两列，`Rename` 取原名、`Glob` / `self` 不产名）；`resolve` 仍只回文件（名字不走前缀匹配） |
| `packages/desktop/src-tauri/crates/core/archi/src/scan/ts.rs` | `ImportDeclaration` 的具名 specifier 与 `ExportFromDeclaration` 的具名清单取名字（`import_kind == Type` 剥除）；default / namespace / `export *` 不产名 |
| `packages/desktop/src-tauri/crates/core/archi/src/drift/mod.rs` | `pub fn api_buckets(declared: &[String], used: &[String]) -> (Vec<String>, Vec<String>)`（求差单点）；对账后按关系 `apis` 非空产两桶（`used ∖ declared` 按 `from.gate` 落 `errors` / `warnings`；`declared ∖ used` 落 `observations`） |
| `packages/desktop/src-tauri/crates/core/archi/src/drift/report.rs` | `ArchiDriftKind` + `UndeclaredApi`（`ALL: [Self; 5]` + `as_str`）；`ArchiObservationKind::UnusedDeclaration` 注记改为「`declared ∖ used` 时产出」；`ArchiDriftReport` 字段集与 `REPORT_VERSION = 3` 零变化 |
| `packages/desktop/src-tauri/crates/core/archi/src/drift/report_test.rs` | `ArchiDriftKind::ALL.len()` 4 → 5；新增观察项产出条件用例（`declared ∖ used` 产一条、`unusedDeclaration` 不进 `status`） |
| `packages/desktop/src-tauri/crates/core/archi/src/dto.rs` | `ArchiRelation` 加 `apis: Vec<String>`；新增 `ArchiRelationDetail { from, to, description, mark, reason, model_file, model_line, declared, used, used_not_declared, declared_not_used, occurrences }`（`specta::Type`）；`to_relation` 拷贝 `apis` |
| `packages/desktop/src-tauri/crates/core/archi/src/relation.rs`（新） | `relation_detail(root, from, to) -> Result<ArchiRelationDetail, String>`：模型面取声明 + `modelFile` / `modelLine`，事实面定向扫描（`ScanInput { mode: Incremental, changed: <from 侧源文件白名单>, .. }`），求差调 `drift::api_buckets`；零写盘、零第二套判定 |
| `packages/desktop/src-tauri/crates/core/archi/src/lib.rs` | 新模块出口 `pub mod relation`（+ `#[cfg(test)] mod relation_test`），产物 `relation_detail` 出口 |
| `packages/desktop/src-tauri/src/commands/archi/mod.rs` | `archi_relation` + `archi_relation_inner`（`root` 校验→调 `archi::relation::relation_detail`→错误映射）；文件头「八命令」→「九命令」；既有八条零改动 |
| `packages/desktop/src-tauri/src/commands/mod.rs` | `all_commands!` archi 段 8 → 9（`archi_relation` 追加在 `archi_api_generate` 之后、`read_file` 之前） |
| `packages/desktop/src-tauri/src/bindings/mod_test.rs` | 四处计数随动（56 → 57 / 104 → 105 / 8 → 9 / 21 → 22）与尾部追加序 |
| `packages/desktop/src/types/generated/bindings.ts` / `bindings.zod.ts` | `bindings:export` 重导（非手改）：`ArchiRelationDetail`、`ArchiRelation.apis`、`commands.archiRelation` 包装 |
| `packages/desktop/src/views/architecture/hooks/use-archi-relation.ts`（新） | 按需取数 hook：`(root, edge)` 变化即取、同一元素对零重复取、失败保留 `error` 供重试；零 watch 零轮询；`edge = null` 时不取数 |
| `packages/desktop/src/views/architecture/components/relation-info.tsx`（新） | `RelationInfo` 面板：七段 + 四空态 + 行内错误与重试 + 模型行号「查看」；`data-testid="archi-relation-info"` |
| `packages/desktop/src/views/architecture/components/architecture-canvas.tsx` | `onEdgeClick` + 选中态（`selectedEdge`）+ `CardHeader` 两态切换（点节点清选中回元素态）；`toEdges` 的 `id` 仍 `from->to`（既有身份，零改动） |
| `packages/desktop/src-tauri/crates/infra/agent/src/archi-knowledge.md` | 关系键面补 `apis` + 一句「只写名字、可留空」（生成物面留空）；零新词表 |
| `packages/desktop/package.json` | `version` patch +1（`0.7.4` → `0.7.5`）；`src-tauri/tauri.conf.json` 经 `../package.json` 跟随、`src-tauri/Cargo.toml` 不跟 |

### 测试文件

| 用例 / 断言 | 位置 | 处置 |
|---|---|---|
| `apis` 四键解析（含省略 → `[]`）、非空 / 无空白 / 无 `::` / 同名重复 / `apis = []` 五类形状违例各报 `invalidValue`、`api = […]` 仍 `unknownField` | `crates/core/archi/src/parse/toml_test.rs` + `model_test.rs` | 新增 |
| `Relation.apis` 三态（省略 / 单名 / 多名）与字面量夹具随动 | `model_test.rs` / `dto_test.rs` | 新增 / 随动 |
| Rust 四形态产名（`use a::B` / `use a::{B,C}` / `use a::B as D` → `B` / `use a::*` 与 `use a::{self}` → 空） | `crates/core/archi/src/scan/rust_test.rs` | 新增 |
| TS 三形态产名（`import {A, type B}` / `export {A} from` / default / namespace / `export *` → 空） | `crates/core/archi/src/scan/ts_test.rs` | 新增 |
| `api_buckets` 纯函数逐例（双空 / 全匹配 / 少一项 / 多一项 / 乱序与重复） | `crates/core/archi/src/drift/*_test.rs` | 新增 |
| 两桶对账：`used ∖ declared` 三档定级（strict → errors / warn → warnings / off → 不出现）、`declared ∖ used` → 一条 `unusedDeclaration` 且 `status` / `passed` 不变、未填 `apis` 零产 | `crates/core/archi/src/drift/*_test.rs` | 新增 |
| `ArchiDriftKind::ALL.len() == 5`、`ArchiObservationKind::ALL.len() == 2`、`version = 3`、既有 16 份金样零变化 | `drift/report_test.rs` + `tests/corpus/drift/**` + `tests/golden_test.rs` | 改断言 + 只加新 case |
| `relation_detail`：声明 / 事实 / 两桶 / 命中点 / `modelFile:line`；四空态（未声明 `apis` 零取数、无 fact、豁免、取数失败）；`archi_relation` 与 `archi_drift` 对同一 `(from, to)` 给出同一两桶 | `crates/core/archi/src/relation_test.rs` + `src-tauri/src/commands/archi/mod_test.rs` | 新增 |
| 命令清单随动（57 / 105 / 9 / 22）与尾部追加序 | `src-tauri/src/bindings/mod_test.rs` | 扩展 |
| 前端：边点击取数、四空态可辨、点节点回元素态、未声明 `apis` 不发起取数、取数失败零影响图与元素面板 | `views/architecture/components/architecture-canvas.test.tsx` + `relation-info.test.tsx`（新）+ `hooks/use-archi-relation.test.ts`（新） | 新增 |
| 前端载荷语料：`ArchiRelationDetail` 与 `ArchiRelation.apis` 的 DTO 夹具随动（字段集由 `tsc` 报错逐处补齐） | `packages/desktop/src/lib/ipc-contract.test.ts` + `views/architecture/**` 的既有夹具 | 随动 |
| 本仓读数留痕：`observations` 62 → 50、`docMissing` 35 → 36、`model.warnings` 9 条 `noPath` 前后不变、`scan` 按条目级账目（`scannedFiles` / `unmatchedFiles` 412 → 415、`excluded` 341 → 352、候选 753 → 767；既有条目零增删改、新增 3 源 + 11 测试 / 语料全部命中既有规则）、`INDEX.toml` 46 条 | 本 change `reports/`（命令 + 留痕，不入 crate 语料） | 新增 |

### 不要修改

- **`explore.md`**（本 change 的）：只读。
- **报告结构与金样**：`ArchiDriftReport` 字段集、`REPORT_VERSION = 3`、`crates/core/archi/tests/corpus/drift/expected/*.json` 16 份既有金样均零改动（新行为只经**新增 case** 落地）。
- **`EdgeMark` / `NodeMark` 词汇与 `lib/marks.ts` / `lib/drift-marks.ts`**：逐字不变（本变更不做边失配徽标）。
- **`ArchiApiFindingKind`（十三值）/ `severity()` / 纳管子集 / `passed` 判据**：零改动——「判据面零改动」的范围仅限 `api/**` 与纳管子集函数；`drift/report.rs` 的 `ArchiDriftKind` +1 值与 `drift/mod.rs` 的两桶装配**不在此列**（归 `desktop-archi-drift`），`bindings.ts` 的 `ArchiDriftKind` 随之由 `bindings:export` 更新。
- **`scan/owner.rs` 归属判据**：零改动（新元素走既有 `OwnerIndex` 最长前缀）。
- **`infra/agent` 工具面与 `compose` 矩阵**：零新工具（`archi_relation` 只走 IPC 命令面）。
- **`archi-knowledge.md` 的体量锚与派生段**：不新增词表、不改体量上限。
- **`AGENT.md` / `CLAUDE.md`**：零改动。
- **`openspec/todo.md`**：流程面管理。
- **`packages/desktop/src-tauri/crates/core/archi/Cargo.toml`**：零新依赖。

## 验收标准

| ID | 验收条件 | 验证方法 |
|----|---------|---------|
| AC-1 | 未填 `apis` 的仓：`drift` 报告与既有 16 份金样逐字段相同（零噪声）——冷启动纪律（16 份金样是**语料根**的冻结快照，与本仓读数无关） | `tests/golden_test.rs` 全量 + 本仓 `archi_drift(full)` 前后对拍：语义面变化恰四处（`observations` 62 → 50、`model.elements` 54 → 55、`apiFindings.docMissing` 35 → 36、`modelHash`），扫描面按 AC-8 的条目级账目变化，采集元信息 `timestamp` 变；其余逐字不变（`errors` / `warnings` 空、`model.warnings` 9 条 `noPath`、`matched` 0、`unresolved` 0、`status`、`passed`、字段集与 `version`） |
| AC-2 | 填一条 `apis` 且漏一项 ⇒ 恰 1 条 `undeclaredApi`，档位按 `from.gate`（strict → `errors` / warn → `warnings` / off → 不出现） | 新语料 case + `drift` 单测 |
| AC-3 | 填一条 `apis` 且多一项 ⇒ 恰 1 条 `unusedDeclaration`，且 `status` / `passed` 不变 | 新语料 case + `drift` 单测 |
| AC-4 | `ArchiDriftKind::ALL.len() == 5`、`ArchiObservationKind::ALL.len() == 2`（未扩）、`as_str` 逐值对拍 | `drift/report_test.rs` |
| AC-5 | `archi_relation` 与 `archi_drift` 对同一 `(from, to)` 给出同一 `usedNotDeclared` / `declaredNotUsed`（同一纯函数；源码复核零第二套判定） | 命令测试对拍 + 源码复核 |
| AC-6 | 报告 `version` 仍为 3、字段集逐字不变（`errors` / `warnings` / `observations` / `apiFindings` 形状不变） | `report_test` + 本仓报告 diff |
| AC-7 | 点边取数失败：图与元素面板零影响（只落行内错误 + 重试）；未声明 `apis` 的边零取数 | 前端用例 |
| AC-8 | 模型面：55 元素 / 123 关系（容器 9 + 叶 46）、`06` 32 / 74、前端恒等式 **74 = 66 + 8**（barrel 端点边 3 出 + 5 入）；12 条命令面边由 `unfulfilledDeclaration` 转兑现（`observations` 62 → 50）；`model.warnings` 仍恰 9 条 `noPath`。**扫描面按条目级账目验收（MUST NOT 写成「三面逐条相同」）**：**既有条目零增删改**（既有 `excluded[]` 条目的 `file` / `rule` 与 `unmatchedFiles[]` 路径逐条不变；数组排序带来的位序平移不计），**新增条目全部命中既有规则**、且逐条可指到本变更新增文件——`scannedFiles` / `unmatchedFiles` 412 → **415**（+3 = `relation.rs` / `use-archi-relation.ts` / `relation-info.tsx`：`gate` 全 `off` ⇒ 与无主同视落 `unmatchedFiles[]`，与归属无关）、`excluded` 341 → **352**（+11 = 1 个 `**/*_test.rs`（`relation_test.rs`）+ 8 个 `**/tests/**` 语料 TS（4 个新 case 各 `repo/src/api/index.ts` + `repo/src/consumer/x.ts`）+ 2 个前端 `**/*.test.ts(x)`），`matched` 0 / `unresolved` 0 / 排除矩阵（13 条默认 + 1 条配置追加 `openspec/**`）前后不变，候选（`scannedFiles + excluded`）753 → **767** 自洽（+14 = 3 源 + 11 测试 / 语料） | `archi_graph` + `archi_drift(full)` 读数留痕，加两数组的**集合对拍**（两端 = `reports/archi/20261120T1042330799149Z.json`（full、变更前）与变更后 full 报告）：既有条目集合的差集为空（`excluded[]` 连 `rule` 一并比）、新增条目逐条点名并各带命中规则；`gate` 全 `off` 使 `matched` 恒空，故 MUST NOT 用 `scannedFiles` / `unmatchedFiles` 计数相等充当「归属未变」的证据（20:25 采集点记的 `excluded = 341` 是那 11 份测试 / 语料尚未落地时的快照，全量落地后为 352，差额逐条可解释） |
| AC-9 | api 基线：本仓分母 46、`docMissing` 36（33 declared + 3 none）、其余十二值 0、`status = clean`、`passed = true`；`api/**` sidecar 仍 13 份 | 命令读数 + `api:check` |
| AC-10 | 生成物零手改：`graph/INDEX.toml` 46 条、`bindings.ts` / `bindings.zod.ts` 的 `ArchiRelationDetail` / `ArchiRelation.apis` / `archiRelation` 只来自 `bindings:export`；清单守卫四处计数随动 | `pnpm run check` 链 + `git diff` 复核 |
| AC-11 | 知识文本：关系四键在场、`apis` 一句在场、`apis` 不进派生词表、体量锚不破 | `archi_knowledge_test` |
| AC-12 | 全管线绿：`pnpm -C packages/desktop run check`（`server:check` → `bindings:export` → `api:check` → `client:check`）与 `run test` 全通过 | 静态检查链 + 全量测试 |
| AC-13 | 版本纪律：`package.json` = `0.7.5`，`src-tauri/tauri.conf.json` 跟随、`src-tauri/Cargo.toml` 版本不随动 | `git diff` 复核 |

> **验收面口径**：验收止于**单元测试 + 静态检查链 + 一次真仓全量读数**。MUST NOT 以「agent 填对了 `apis`」这类模型行为质量作验收条——那是 prompt 与数据的长期观测项。

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|---|---|---|---|
| **R1 后端 `used` 面在本仓为零**（观察根无 `Cargo.toml` ⇒ 零 crate ⇒ 49 条后端关系恒 unfulfilled） | 后端关系一旦填 `apis`，全部名字落 `declared ∖ used`，读起来像「声明全没用」 | 高（已实测） | 本变更不修（D18），但三处同时兜：spec 写明 `used` 是 under-approx 的已知上限；`unusedDeclaration` 文案并列两种成因（真没用 / 写错或看不见）；UI 空态②「扫描面无该关系的 import」如实呈现。填后端 `apis` 前先做工作区清单发现（独立变更） |
| **R2 中继元素引发 DTO / 计数面扩散**：+1 元素牵动模型表头、ADR §1/§9、`INDEX.toml`、api 基线（46 / 36）、`desktop-archi-api` 的入口实测（45 Resolved + 1 Absent）、`repo-archi-model` 四处计数 | 漏一处即自称自相矛盾（本仓「同一事实两种自称」正是既有病灶） | 高 | 变更范围表逐处列全并进验收（AC-8 / AC-9 / AC-10）；计数复述三处同批写实；提案「事实校正」表把九处（八处既有陈旧 + 一处本提案旧稿的结构性误判：`scan` 三面「逐条相同」）**逐条留痕**（explore 三处 + spec 基线五处：barrel 8 条端点边、`model.warnings` 9 条 `noPath`、DTO 尾锚 `NodeMark`、`Ui` 入口 41 条 `export *`、前端契约散文 28 条数据行），凡本变更重写到的段落其读数一律按盘上实测复述；**写实边界**（§7 末尾）逐条点名「哪几份 spec 的历史读数不追改」——收敛面明确，避免半个仓库追改与「声称已清」的两种失真 |
| **R3 `apis` 变相成为协议正文** | 模型开始承载签名 / 类型形状，与「单一手写真相」冲突 | 中 | 三条护栏（只许名字 / 可空 / `apis = []` 拒收）+ 值面四查（无空白 / 无 `::` / 无重复）机械拒收；spec 写死「位置在关系、内容只有名字、权威在代码」；不做 ⊆ 导出集是**知情取舍**（后果已写进 spec：错字只能落观察项） |
| **R4 `declared ∖ used` 的两种成因混淆** | 错字被当成「可忽略的没用」，长期无人看 | 中 | 显式豁免「一个值一种成因」的纪律并写明理由；`message` 与 UI 必须并列两种可能；观察项永不进门禁（宁可漏报不制造假红） |
| **R5 两桶与既有对账耦合出新假阳**：同一元素对同时出 `unfulfilledDeclaration` 与 `unusedDeclaration` | 报告噪声翻倍 | 中 | 两条事实不同（「完全没 import」vs「这几个名字没出现」），允许并存，但 `unusedDeclaration` 文案必须点名「本对无 import 时另有 unfulfilledDeclaration 一条」；AC-1 的对拍保证 `apis` 空仓零噪声 |
| **R6 定向扫描的成本与语义漂移**：`archi_relation` 复用 `Incremental` 模式 | 误把它当成「一次增量检查」（报告面差异）或成本失控 | 中 | 命令只取 `ScanOutcome.edges`，MUST NOT 组装报告；`changed` 白名单 = `from` 元素 `path` 下的源文件（十来个，毫秒级）；零写盘、零缓存；spec 写明「这是一次按需事实取样，不是一次检查」 |
| **R7 画布三触发口径被读成第四个触发点** | 后续变更以「既然点边能取，那 watch 也行」滑坡 | 中 | S3 写进 spec（显式用户动作发起、不计入三触发、仍禁 watch / 轮询）；前端 hook 零 `setInterval` / 零订阅，测试断言零定时器 |
| **R8 报告形状被顺手改** | 16 份金样 + 绑定 + 前端载荷语料全动，且 `version` 语义失去可信度 | 低 | D9 写死；验收 AC-6；`used` / 两桶只经命令面出线，`message` 承载名字 |
| **R9 名字取数的假名污染**：默认导入 / 命名空间 / `export *` 若被静默产名 | 假阳（「用了没声明」）指向不存在的导出名 | 中 | 取数口径一次定死（§3 表）并逐形态进语料；比对逐字符、禁折叠 |
| **R10 中继元素 `gate = off` 的语义误读** | 有人以为它是「不参与判定 ⇒ 12 条边仍不兑现」；或反过来用 `scan` 三面计数反推归属 | 中 | `off` 只关定级、不关事实（既有纪律）；AC-8 以 `observations` 62 → 50 机械证明 12 条边兑现；`scan.unmatchedFiles` 仍含 3 个生成文件（`off` 的既有分支）也一并写进验收，防「文件归属变了所以报告变了」的误读。**反面同样封死**：`scannedFiles` / `unmatchedFiles` / `excluded` 的计数必然随本方新增源文件与测试 / 语料而变，AC-8 只按**条目级账目**验收（既有条目零增删改 + 新增条目全部命中既有规则），MUST NOT 用「三面计数相等」充当归属或判定面零变化的证据 |
| **R11 `Absent` 入口事实被读成「入口债」** | 有人给生成目录造 `index.ts` 消 Absent | 低 | `api = none` 已退出入口判据（零 `apiEntryMissing`）；spec 与 ADR 明写「生成目录是叶子文件集合、无模块入口」；`entry` 仍恰三处 |
| **R12 版本纪律漂移**：`package.json` 忘 bump 或 `src-tauri/Cargo.toml` 被顺手跟随 | 发布流水线版本错位 | 低 | AC-13 显式核对两处；`tauri.conf.json` 经 `../package.json` 自动跟随 |
