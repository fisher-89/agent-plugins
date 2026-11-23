# 0001 · 架构模型的粒度与取数口径

> 记录基线模型（`.dev-team/architecture/graph/model/` 六文件）的判据：为什么这些元素存在、为什么这些边存在、什么一律不写进模型。
> 取数口径固定为两类机械事实：**`Cargo.toml` 的 `[dependencies]` 声明**（后端）与 **`import` 语句**（前端）。
> 判据之外的元素与边一律不写——「觉得应该连一条」不是判据。

## 1. 结论

模型的每个元素都能归属到一条覆盖判据（crate / 视图 / 视图内子域 / 共享组件 / 原语层 / 生成物中继 / 壳 / 容器），每条关系都能指到一处具体声明（`Cargo.toml` 段落或 `import` 行）。基线为 **55 元素 / 129 关系**：后端 52 条（10 + 25 + 14 + 3，拆在 `02`–`05` 四个文件）、前端 77 条（全在 `06`）。任一诊断（成环 / 依赖祖先 / 未定义引用 / 孤儿层级）非零即整图停摆，故本记录同时也是「哪些写法会立刻停摆」的清单。

## 2. 粒度判据与元素类型

| 档 | 判据 | 反例（什么不进模型） |
|---|---|---|
| 后端到 crate | `crates/{core,infra,tools}/<dir>/` 每个成员 crate 一元素（裸名 CamelCase），壳 `Backend.Shell` 覆盖 `src-tauri/src/` | crate 内部的 `parse` / `graph` / `dto` / `runner` 等模块文件一律不建元素——内部模块划分不是架构声明，且随重构高频漂移；它们只出现在对应 crate 的 `summary` 里，如 `Backend.Core.Archi` |
| 前端到视图 | `views/<view>/` 每目录一元素、`views/welcome-view.tsx` 单文件一元素；视图内子域 = 除 `components` / `hooks` 外、**递归**非测试源文件 ≥ 3 的子目录 | `views/changes/components/`（2 文件）不构成子域，不建元素；视图内的 `hooks/` / `lib/` 不建元素（其 import 仍计入所属视图） |
| 前端到组件 | `components/agent/*.{ts,tsx}` 与 `components/standard/*.tsx` 每文件一元素（排除测试与 barrel `index.tsx`）；`components/ui/` **收敛为 1 元素** | `components/ui/` 的 41 个文件不逐文件建元素：原语层恒被依赖、不承担「谁能依赖谁」的判定，逐文件建元素只增噪声；`Desktop.Components.Ui` 一个元素承载整层 |

子域计数口径示例：`Desktop.Views.Changes.Flow` 的目录顶层与子目录合计 16 个非测试源文件、`renderers` 8 个、`agent` 4 个（含其 `hooks/`），恰三处达标。

元素类型与粒度档对应：crate（`crates/{core,infra,tools}/<dir>/`）与两个包根（`Backend` / `Desktop`）`kind = "package"`；`core` / `infra` / `tools` 分层容器同为 `package`（其子元素是包）；业务聚合容器（`Desktop.Views` / `Desktop.Components` / `Desktop.Components.Standard`）`kind = "domain"`（`path` 选填）；目录 / 视图 / 单文件组件（含 barrel 与生成物中继）`kind = "module"`。`composite` 是聚合类型，本仓当前无实例——它不承担层级，只用于把过多的同级元素分类；`path` 选填，填了即持文件并可作端点，不填则纯分组。

## 3. 容器 `path` 选填；包根兜底文件归属；barrel 是目录模块的入口

包含关系由 FQN 前缀表达，不由关系表达。`domain` / `composite` 容器（`Backend.Core` / `Backend.Infra` / `Backend.Tools` / `Desktop.Views` / `Desktop.Components` / `Desktop.Components.Standard`）的 `path` 是**选填**字段：不填即纯分组（本基线六个容器全部留空）；填了即「它自己持有文件」，该元素因而可以成为关系端点。容器持 `path` 时须留意：子元素依赖它会写出「子 → 父」，`dependsOnAncestor` 立即停摆——本基线因此让六个容器保持空。

两个包根 `Backend` / `Desktop` 不在此列：它们是 `kind = "package"` 的**持有者**，各持根路径（`./packages/desktop/src-tauri/` 与 `./packages/desktop/src/`），把未被更具体元素认领的文件兜在自己名下。这是「包 = 文件归属兜底面」的显式表达；`Backend` 与 `Desktop` 因而可以成为关系端点（如 `Desktop → Desktop.GeneratedBindings`），不属于「容器随手连边」。

barrel 不是容器的特权，而是**目录模块**（`kind = "module"` 的叶）的入口：前端 12 个目录模块（8 个视图目录根 + 3 个视图内子域 + `components/ui`）各持一个 `index.ts`，`Desktop.Components.Agent` 亦为目录型 `module`（`path` = `./packages/desktop/src/components/agent/`，入口由入口解析链的 ③ 命中目录内 `index.tsx`），与这 12 个入口同形。生成物中继 `Desktop.GeneratedBindings` 是唯一**持目录 `path` 而无模块入口**的叶元素（三个生成文件是叶子集合、没有 `index.ts`，见第 7 节），MUST NOT 为它造入口。目录模块因此合法成为关系端点：`Desktop.Components.Agent` 的端点边恰 **8 条**：3 条出边（入口 `index.tsx` 重导出 `AgentInput` / `AgentSessionView` / `SessionPanel`）+ 5 条入边（`Views.AgentDebug` / `Views.Changes.Agent` / `Views.Changes.Flow` / `Views.Config` / `Views.Explores` import `@/components/agent`）。其余 12 个入口只 re-export 本元素直属文件，`from == to` 的自边被 `scan` 丢弃 ⇒ 前端关系数不因入口而变。

两条反例分别是：

- **禁的是「子 → 父」前缀边**：把 `components/standard/hooks/` 归给容器 `Desktop.Components.Standard`，就会写出 `Standard.Dailog → Standard` 这类「子依赖父」，`dependsOnAncestor` 立即停摆；故 hooks 目录不入模型。
- **barrel 自持文件不是前缀边**：`Desktop.Components.Agent → Desktop.Components.Agent.AgentInput` 是「父 import 子」（装配依赖），方向合法，与包含无关。

## 4. dev 边排除

后端关系只取 `[dependencies]` 段内指向 workspace 内包的声明（`{ workspace = true }` 与 `{ path = … }` 同权），`[dev-dependencies]` 与 `[build-dependencies]` 按**段落整体**排除，而不是「挑几个明显成环的排除」。实测 4 条 dev 边：`workflow → orchestration`、`orchestration → store`、`workflow → store`、`checks-runtime → workflow`。前三者与生产边 `orchestration → workflow`、`store → workflow` 合起来即 `cycle`；只按「`workflow ⇄ orchestration`」一条举例排除，仍会漏掉 `workflow → store` 并让整张图停摆。

test-only 互依的事实由 `Cargo.toml` 自身承载，模型 MUST NOT 重复表达；排除后关系总数不受影响（后端仍 52 条）。

## 5. 反向边在案：`Backend.Tools.ExportBindings`

`Backend.Tools.ExportBindings → Backend.Shell`（`path = "../../../"`）是模型内唯一「工具依赖壳」的倒置边，不是笔误：导出命令面 bindings 必须以壳的包为依赖。同理，`Backend.Shell` 出边 14 条、`Backend.Infra.Watch` 与 `Backend.Core.Agent` 出边为零，都是 `Cargo.toml` 的直读结果，不是取舍。

## 6. `gate` 全 `off`

基线 55 个元素 `gate` 全部显式写 `off`（不走缺省），冷启动 ratchet：想收紧 `warn` / `strict` 的模块后续各自独立变更，一次收紧一块，`grep` 即可定位全部待收紧面。收紧的前提是先有价值——本轮只负责让模型第一次存在，不设任何门禁。

## 7. 封闭排除面

以下位置不建独立元素：`src/hooks/**`、`src/lib/**`、`src/types/**`（`types/generated/` 已由生成物中继元素归属）、`src/components/standard/hooks/**`；`*.test.{ts,tsx}` 与 `src/test-setup.ts` 不参与扫描。

- 顶层 `hooks` / `lib` 与 `types/` 的其余文件归属落到包根 `Desktop`（最长前缀兜底），其 import 计入 `Desktop` 的端点——这是 `Desktop → Desktop.GeneratedBindings` / `Desktop → Desktop.Components.Ui` 两条关系的来源；
- `components/standard/hooks` 不入模型是为了避免把 hooks 归给容器（本基线容器不持 `path`；若归给容器再让子元素依赖它，就会写出「子 → 父」边）；
- **粒度 ≠ 解析面**：视图目录内的 `hooks/` / `lib/` 文件不建元素，但其 `import` 计入所属视图元素——否则「指向 `Desktop.GeneratedBindings` 的 13 条命令面边」无法全部成立（`Views.DbInspector` 与 `Views.Changes.Agent` 两处只由视图 `hooks/` 文件证成）。

唯一的映射例外是生成物：`types/generated/bindings` 的 import 不丢弃，而是映射为命令面依赖 → `Desktop.GeneratedBindings`（`desc` 写明「经生成的 IPC 绑定」）。这是前端依赖命令面的唯一合法边来源（前端 → 生成物；生成物 → 后端那一段不声明，理由见下）。

该映射现在**有主 ⇒ 兑现**：`src/types/generated/` 由 `Desktop.GeneratedBindings` 持有（`api = none`、零 `entry`、零关系出边），扫描解析出目标文件后按 `Owner::Element` 归属，13 条命令面边不再落 `observations[].unfulfilledDeclaration`。上游生成链（`GeneratedBindings → Shell`）不是 `import` 事实、**不声明**——声明即永久 `unfulfilledDeclaration`，要表达它须先有「生成链对账」判据（`bindings.ts` 的 invoke 串集 ↔ `#[tauri::command]` 名集），属独立能力。

## 8. 已知缺口与修改纪律

- **归属兜底（本变更）**：顶层 `src/hooks` / `src/lib` / `src/types` 与 `components/file/**` 仍不建独立元素，但文件不再悬空——归属落到包根 `Desktop`，其 import 计入 `Desktop` 端点（`Desktop → Desktop.GeneratedBindings` / `Desktop → Desktop.Components.Ui`）。要为某一目录建独立元素，仍须按第 2 节判据新增档位并连带换段全部计数，属改本能力。
- **已知缺口（先于本变更存在）**：`crates/tools/specta-zod-patched` 是 vendored 上游 exporter（`[package] name = "specta-zod"`，补丁面与跟版核对登记在 `desktop-ipc-type-bindings`），尚未建元素；其文件现由包根 `Backend` 兜底归属。补齐须新增 crate 元素并连带换段全部计数，或等上游合入对应能力后整目录删除（届时判据自然归零）——两条路都属独立变更，故本期只登记。
- **已知缺口（判据无档）**：`components/file/file-viewer.tsx` 没有独立元素（归属落 `Desktop`）——第 2 节三层表只有 `components/agent/*` 与 `components/standard/*.tsx` 两档，没有 `components/<其它目录>` 档。**补它须改本能力：新增 `components/<其它目录>` 粒度档**（连带决定该档是否排除 barrel / hooks），故本期只登记。
- **已知缺口（判据命中未建元素）**：`views/architecture/agent/` 的递归非测试源文件 3 个（`archi-chat-composer.tsx` / `archi-chat-panel.tsx` / `hooks/use-archi-chat.ts`）已命中第 2 节的视图内子域判据，但模型未建 `Desktop.Views.Architecture.Agent`。补齐属独立变更（元素面与全部计数同批换段），不由本期顺手加。
- **已知缺口（边面）**：11 条已建元素之间的跨元素 `import` 尚未声明：`Desktop.Views.Architecture → Desktop.Components.Agent`、`…Views.Architecture → …Components.Standard.Dailog`、`…Views.Changes → …Components.Agent`、`…Views.Changes → …Components.Standard.Drawer`、`…Views.Changes → …Views.Changes.Renderers`、`…Views.Explores → …Components.Standard.Dailog`、`…Components.Agent.AgentInput → …Components.Standard.Select`、`…Components.Agent.AgentInput → …Components.Ui`、`…Components.Agent.ToolCards → …Components.Ui`、`…Components.Agent → …Components.Agent.AgentMessages`、`…Views.Changes.Flow → …Views.Changes`（`…` 承接同一条前缀 `Desktop.`）。其中末条是「子 → 父」形态，声明即 `dependsOnAncestor` 停摆，**补齐须先改代码分层**；其余十条补声明即可，但只补边不补元素会让「加边前后的差」失去可对拍性，故本期不补。同条附一处反向缺口：`…Views.Changes.Agent → …Components.Ui` 已声明而扫描面无证据（该目录只有测试文件 import `@/components/ui/resizable`），清除或保留须与代码分层一并定。
- **覆盖判据第 4 档（生成物中继）**：`Desktop.GeneratedBindings` 的判据 = `path` 恰为生成目录 `./packages/desktop/src/types/generated/`、`api = none`、零 `entry`、零关系出边（只作 `to`）。生成物是唯一真相：手写契约、模块入口与生成链都会漂，故它只持有归属、不承担判定——`gate = off` 只关定级、不关事实，13 条命令面边以 `Owner::Element` 命中即兑现。
- **上游生成链不声明**：`GeneratedBindings → Shell` 不是 `import` 事实，声明即永久 `unfulfilledDeclaration`；要表达它须先有「生成链对账」判据（`bindings.ts` 的 invoke 串集 ↔ `#[tauri::command]` 名集，零格式转换），属独立能力，不在基线内顺手加。
- **漂移纪律**：新增 / 删除 crate、视图、视图内子域、共享组件，或新增跨元素 `import` 依赖时，同一 change 内必须同步模型（元素 / 关系 / `path`），MUST NOT 留待「下次整理」。
- **修改纪律**：偏离判据（例如想给 crate 内部模块建元素、想让容器随手连边、想按「语义上该有」补边）必须改本能力的 requirement，不得就地加元素或加边；模型是数据资产，判据是它的唯一合法性来源。

## 9. 计数推导（可逐项复算）

```
元素 = 01(2) + 02(9) + 03(8) + 04(1) + 05(3) + 06(32) = 55（纯容器 6 + 其余 49）
关系 = 01(0) + 02(10) + 03(25) + 04(14) + 05(3) + 06(77) = 129
     后端 52（Cargo [dependencies] 的 workspace 内声明）+ 前端 77（import 证据）
前端 77 = 69（元素 → 元素）+ 8（`Desktop.Components.Agent` barrel 端点边）
```

若某次落笔得到前端 66，说明 8 条 barrel 端点边被漏掉（判据自相矛盾），MUST NOT 以「容器不入边」为由删减——要回到 66 只能撤销第 3 节的目录模块入口或改第 7 节的映射规则，两者都是改本能力。

**留痕（既有陈旧，本变更同批校正）**：`53 / 121`、`06(30) / 72`、前端 `65 + 7` 与第 2 节的「39 个文件」、第 3 节的「端点边 7 条（4 条入边）」都是旧读数——`Combobox` 元素与其两条关系、`Views.Config → Components.Agent` 的入边早已在盘上存在而读数未追改。故 `53 → 55` 是本变更 **+1（生成物中继元素、零新关系）与既有陈旧 +1** 的合计，MUST NOT 把它读成「一次 +1」；处置是计数复述三处（本节 / 第 1 节 / `06` 表头）与两处构成读数同批换段。
