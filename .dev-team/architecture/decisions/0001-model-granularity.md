# 0001 · 架构模型的粒度与取数口径

> 记录基线模型（`.dev-team/architecture/models/` 六文件）的判据：为什么这些元素存在、为什么这些边存在、什么一律不写进模型。
> 取数口径固定为两类机械事实：**`Cargo.toml` 的 `[dependencies]` 声明**（后端）与 **`import` 语句**（前端）。
> 判据之外的元素与边一律不写——「觉得应该连一条」不是判据。

## 1. 结论

模型的每个元素都能归属到一条覆盖判据（crate / 视图 / 视图内子域 / 共享组件 / 原语层 / 壳 / 容器），每条关系都能指到一处具体声明（`Cargo.toml` 段落或 `import` 行）。基线为 **50 元素 / 111 关系**：后端 47 条（9 + 24 + 13 + 1，拆在 `02`–`05` 四个文件）、前端 64 条（全在 `06`）。任一诊断（成环 / 依赖祖先 / 未定义引用 / 孤儿层级）非零即整图停摆，故本记录同时也是「哪些写法会立刻停摆」的清单。

## 2. 三档粒度

| 档 | 判据 | 反例（什么不进模型） |
|---|---|---|
| 后端到 crate | `crates/{core,infra,tools}/<dir>/` 每个成员 crate 一元素（裸名 CamelCase），壳 `App.Backend.Shell` 覆盖 `src-tauri/src/` | crate 内部的 `parse` / `graph` / `dto` / `runner` 等模块文件一律不建元素——内部模块划分不是架构声明，且随重构高频漂移；它们只出现在对应 crate 的 `summary` 里，如 `App.Backend.Core.Archi` |
| 前端到视图 | `views/<view>/` 每目录一元素、`views/welcome-view.tsx` 单文件一元素；视图内子域 = 除 `components` / `hooks` 外、**递归**非测试源文件 ≥ 3 的子目录 | `views/changes/components/`（1 文件）不构成子域，不建元素；视图内的 `hooks/` / `lib/` 不建元素（其 import 仍计入所属视图） |
| 前端到组件 | `components/agent/*.{ts,tsx}` 与 `components/standard/*.tsx` 每文件一元素（排除测试与 barrel `index.tsx`）；`components/ui/` **收敛为 1 元素** | `components/ui/` 的 35 个文件不逐文件建元素：原语层恒被依赖、不承担「谁能依赖谁」的判定，逐文件建元素只增噪声；`App.Frontend.Components.Ui` 一个元素承载整层 |

子域计数口径示例：`App.Frontend.Views.Changes.Flow` 的目录顶层与子目录合计 13 个非测试源文件、`renderers` 7 个、`agent` 3 个（含其 `hooks/`），恰三处达标。

## 3. 容器 `path` 恒空，唯一例外是 barrel

包含关系由 FQN 前缀表达，不由关系表达。因此容器元素（`App` / `App.Backend` / `App.Backend.Core` / `App.Frontend` / `App.Frontend.Views` / `App.Frontend.Components` / `App.Frontend.Components.Standard` 等）`path` 恒空，也不参与任何关系——`path` 非空即意味着「它自己持有文件」，而「持有」会立刻引出端点边。

唯一例外是持有 barrel 的 `App.Frontend.Components.Agent`（`path` = `components/agent/index.tsx`）。它合法成为关系端点，端点边恰 **6 条**：2 条出边（barrel 重导出 `AgentInput` / `AgentSessionView`）+ 4 条入边（`Views.AgentDebug` / `Views.Changes.Agent` / `Views.Changes.Flow` / `Views.Explores` import `@/components/agent`）。

两条反例分别是：

- **禁的是「子 → 父」前缀边**：把 `components/standard/hooks/` 归给容器 `App.Frontend.Components.Standard`，就会写出 `Standard.Dailog → Standard` 这类「子依赖父」，`dependsOnAncestor` 立即停摆；故 hooks 目录不入模型。
- **barrel 自持文件不是前缀边**：`App.Frontend.Components.Agent → App.Frontend.Components.Agent.AgentInput` 是「父 import 子」（装配依赖），方向合法，与包含无关。

## 4. dev 边排除

后端关系只取 `[dependencies]` 段内指向 workspace 内包的声明（`{ workspace = true }` 与 `{ path = … }` 同权），`[dev-dependencies]` 与 `[build-dependencies]` 按**段落整体**排除，而不是「挑几个明显成环的排除」。实测 4 条 dev 边：`workflow → orchestration`、`orchestration → store`、`workflow → store`、`checks-runtime → workflow`。前三者与生产边 `orchestration → workflow`、`store → workflow` 合起来即 `cycle`；只按「`workflow ⇄ orchestration`」一条举例排除，仍会漏掉 `workflow → store` 并让整张图停摆。

test-only 互依的事实由 `Cargo.toml` 自身承载，模型 MUST NOT 重复表达；排除后关系总数不受影响（后端仍 47 条）。

## 5. 反向边在案：`App.Backend.Tools.ExportBindings`

`App.Backend.Tools.ExportBindings → App.Backend.Shell`（`path = "../../../"`）是模型内唯一「工具依赖壳」的倒置边，不是笔误：导出命令面 bindings 必须以壳的包为依赖。同理，`App.Backend.Shell` 出边 13 条、`App.Backend.Infra.Watch` 与 `App.Backend.Core.Agent` 出边为零，都是 `Cargo.toml` 的直读结果，不是取舍。

## 6. `gate` 全 `off`

基线 50 个元素 `gate` 全部显式写 `off`（不走缺省），冷启动 ratchet：想收紧 `warn` / `strict` 的模块后续各自独立变更，一次收紧一块，`grep` 即可定位全部待收紧面。收紧的前提是先有价值——本轮只负责让模型第一次存在，不设任何门禁。

## 7. 封闭排除面

以下位置不建元素、不参与覆盖、不贡献边：`src/hooks/**`、`src/lib/**`、`src/types/**`（含 `types/generated/bindings.ts`）、`src/components/standard/hooks/**`、`*.test.{ts,tsx}`、`src/test-setup.ts`。

- 顶层 `hooks` / `lib` / `types` 没有归属元素，其 import 零贡献；
- `components/standard/hooks` 不入模型是为了守住第 3 节的「容器 `path` 恒空」；
- **粒度 ≠ 解析面**：视图目录内的 `hooks/` / `lib/` 文件不建元素，但其 `import` 计入所属视图元素——否则「指向 `App.Backend.Shell` 的 11 条命令面边」无法全部成立（`Views.DbInspector` 与 `Views.Changes.Agent` 两处只由视图 `hooks/` 文件证成）。

唯一的映射例外：`types/generated/bindings` 的 import 不丢弃，而是映射为命令面依赖 → `App.Backend.Shell`（`desc` 写明「经生成的命令面类型」）。这是前端指向后端的唯一合法边来源。

## 8. 已知缺口与修改纪律

- **已知缺口**：顶层 `src/hooks` 无元素 ⇒ `App.Frontend.Shell → App.Backend.Shell` 在图上缺载体，前端壳对命令面的依赖本期不可见（`src/hooks` 的 `import` 因无归属元素而零贡献）。补齐须新增元素并重排排除面，属改本能力，不在基线内顺手加。
- **漂移纪律**：新增 / 删除 crate、视图、视图内子域、共享组件，或新增跨元素 `import` 依赖时，同一 change 内必须同步模型（元素 / 关系 / `path`），MUST NOT 留待「下次整理」。
- **修改纪律**：偏离判据（例如想给 crate 内部模块建元素、想让容器随手连边、想按「语义上该有」补边）必须改本能力的 requirement，不得就地加元素或加边；模型是数据资产，判据是它的唯一合法性来源。

## 9. 计数推导（可逐项复算）

```
元素 = 01(3) + 02(8) + 03(8) + 04(1) + 05(2) + 06(28) = 50（容器 10 + 叶 40）
关系 = 01(0) + 02(9) + 03(24) + 04(13) + 05(1) + 06(64) = 111
     后端 47（Cargo [dependencies] 的 workspace 内声明）+ 前端 64（import 证据）
前端 64 = 58（叶 → 叶 / 叶 → 容器）+ 6（`App.Frontend.Components.Agent` barrel 端点边）
```

若某次落笔得到前端 58，说明 6 条 barrel 端点边被漏掉（判据自相矛盾），MUST NOT 以「容器不入边」为由删减——要回到 58 只能撤销第 3 节的 barrel 例外或改第 7 节的映射规则，两者都是改本能力。
