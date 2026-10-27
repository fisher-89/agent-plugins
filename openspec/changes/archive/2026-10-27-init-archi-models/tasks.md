# 任务: init-archi-models

> 覆盖面 = proposal「变更范围 - 实现文件」7 项（6 个模型文件 + 1 份 ADR），全部为**新增数据 / 文档**，零源码改动。
>
> **本相位不写测试**：不新增 `#[test]`、不新增 `*.test.tsx`、MUST NOT 把本仓模型塞进 `crates/core/archi/tests/corpus/**`（crate 语料须自足可移植，见 design D9）；三条核对（双向覆盖 / 关系证据 / 图自检）以命令 + 报告留痕执行，留痕由报告相位落盘。
>
> 依赖序要点：**阶段一**（判据枚举）解锁全部后续阶段——元素表与关系表必须先由命令枚举出来，不能凭印象落笔；**阶段二**建立目录与包根（`App` 是全部 FQN 的祖先，缺它即 `orphanHierarchy`）；**阶段三～六**（后端四文件）的 `to` 只指后端元素，且都已在 `02` / `03` / `04` 内声明（`05` 的 `to` 指向 `04` 的 `Shell`，故 `04` 不晚于 `05`）；**阶段七**（前端元素）必须先于**阶段八**（前端关系）——关系两端的 FQN 必须已声明，否则 `unknownReference`；**阶段九**（ADR）依赖阶段七（正文要逐字引用前端 FQN）；**阶段十**为交付自查。
>
> 约定：模型内**不写** `metadata` 行、`docs` 行只出现在 `App`；`gate` 一律显式 `"off"`（49 行）；`path` 以 `./` 起始、目录以 `/` 结尾、文件精确到扩展名；每条 `[[relation]]` 写在其 `from` 元素所在文件内。
>
> 自查命令 cwd 除特别标注外均为仓库根；涉及 crate 目录时用 `packages/desktop/src-tauri` 前缀，涉及前端时用 `packages/desktop/src` 前缀。

## 阶段一：判据枚举与基线（落笔前的一次性取数）

- [x] 新建目录 `openspec/architecture/models/` 与 `openspec/architecture/decisions/`（`openspec/architecture/` 当前不存在；`models` / `decisions` 段名由 `foundation::layout::archi_dir` 族派生，落点即此）
- [x] 枚举后端覆盖集合：`ls packages/desktop/src-tauri/crates/core packages/desktop/src-tauri/crates/infra packages/desktop/src-tauri/crates/tools` 得 14 个 crate 目录，另加 `packages/desktop/src-tauri/src/`（壳）；与 spec「后端下钻到 crate 层的元素覆盖」表中的 15 个叶元素 `path`（7 core + 6 infra + `export-bindings` + 壳，另 5 个容器无 `path`）逐项比对，确认无漏无多
- [x] 枚举前端覆盖集合（三条判据分别执行）：① `ls packages/desktop/src/views` 得 9 项（8 目录 + `welcome-view.tsx`）；② 视图内子域 = `views/changes/` 下**目录名不为 `components` / `hooks`** 且**递归**非测试源文件 ≥ 3 的子目录 → 应恰为 `flow`(13) / `renderers`(7) / `agent`(3)（`components`(1) 不达标）；③ `ls packages/desktop/src/components/agent` 与 `.../standard`，按「排除 `*.test.*` 与 barrel `index.tsx`」得 6 + 4 个共享组件文件，`components/ui/` 收敛为 1 元素
- [x] 抽取后端关系事实：逐个读取 14 个 crate 与 `packages/desktop/src-tauri/Cargo.toml` 的 `[dependencies]` 段，取 `{ workspace = true }` 键名与 `{ path = … }` 的 workspace 内条目，按「裸名 CamelCase」映射为元素，得 **43** 条；显式核对 `[dev-dependencies]` 内的 workspace 声明**全部不取**（实测 4 条：`workflow → orchestration`、`orchestration → store`、`workflow → store`、`checks-runtime → workflow`；只按前两条举例排除会漏掉 `workflow → store` 并造成 `cycle`），`export-bindings → dev-team`（`path = "../../../"` → `App.Backend.Shell`）**取**
- [x] 抽取前端关系证据：对 source 侧文件集（`Shell` 的 4 个文件 + 各叶元素 `path` 覆盖范围内文件，**含视图内 `hooks/` / `lib/`**）取静态 `import` / `export … from` 说明符，按 design D4 解析归属、剔除指向 `hooks/` `lib/` `types/`（`types/generated/bindings` 例外 → `App.Backend.Shell`）与 `from == to`，得 **64** 条，并核对「指向 `App.Backend.Shell` 的来源恰 11 个元素」
- [x] 口径对拍（落笔前，一次性）：逐项核对四处数值——proposal AC-1 / AC-3 / AC-5 / AC-6、`specs/repo-archi-model/spec.md` 合计段与场景、design「数值口径闭合」、本任务表的逐文件计数——同为 元素 49 / 后端关系 43 / 前端边 64（= 58 + 6 条 `App.Frontend.Components.Agent` barrel 端点边）/ 关系总 107；任何一处出现第二个数（如把 58 或 101 当成总数）即在本 change 内改 spec delta（`openspec/changes/init-archi-models/specs/repo-archi-model/spec.md`）**或**改本设计，不得带着冲突落笔；若判据枚举实测与四处均不同，以判据枚举结果为准并同步回写 spec delta 与 design，差异说明 + 判据依据 + 实测清单交报告相位

## 阶段二：包根与域容器（`01-app.toml`）

- [x] 新建 `openspec/architecture/models/01-app.toml`：写 3 个元素 `App`（`kind = "package"`、`gate = "off"`、`docs = ["./README.md", "./AGENT.md"]`、`summary` 见 design §01）、`App.Backend`、`App.Frontend`（均 `kind = "domain"`、`gate = "off"`、无 `path` / `docs` / `metadata`）；本文件零 `[[relation]]`
- [x] 自查 阶段二：`grep -c '^\[element' openspec/architecture/models/01-app.toml` = 3；`grep -c '^\[\[relation\]\]'` = 0；`grep -n 'metadata'` 零命中；`docs` 仅出现在 `App` 表内且两条路径存在于仓库根

## 阶段三：core 分层（`02-backend-core.toml`）

- [x] 新建 `openspec/architecture/models/02-backend-core.toml`：写 `App.Backend.Core` 容器（`domain`、空 `path`）+ 7 个 crate 元素 `Foundation` / `Config` / `Workflow` / `Archi` / `Orchestration` / `Agent` / `Checks`（`kind = "module"`、`path = "./packages/desktop/src-tauri/crates/core/<dir>/"`、`summary` 取 design §02 表）
- [x] 同文件写 9 条关系（`desc` 一句用途）：`Archi → Foundation`；`Checks → Config`、`Checks → Foundation`；`Config → Foundation`；`Orchestration → Agent`、`Orchestration → Foundation`、`Orchestration → Workflow`；`Workflow → Agent`、`Workflow → Foundation`（`Foundation` / `Agent` 零出边）
- [x] 自查 阶段三：元素 8 / 关系 9；`grep -n 'from = "App.Backend.Core.Workflow"' -A1 openspec/architecture/models/02-backend-core.toml` 无 `to = "App.Backend.Core.Orchestration"`（`[dev-dependencies]` 边，与生产边 `Orchestration → Workflow` 合起来即 `cycle`）；`grep -n 'from = "App.Backend.Core.Workflow"' -A1` 亦无 `to = "App.Backend.Infra.Store"`（`[dev-dependencies]` 边，与生产边 `Store → Workflow` 合起来即 `cycle`）；`grep -n 'from = "App.Backend.Core.Orchestration"' -A1` 无 `to = "App.Backend.Infra.Store"`（同为 dev 边，按段落整体排除）；全文件无 `to = "App.Backend.Core"` 形态的「子 → 父」边

## 阶段四：infra 适配层（`03-backend-infra.toml`）

- [x] 新建 `openspec/architecture/models/03-backend-infra.toml`：写 `App.Backend.Infra` 容器 + 6 个 crate 元素 `AgentRuntime`（`crates/infra/agent/`）/ `ChecksRuntime`（`crates/infra/checks/`）/ `Exec` / `Store` / `VcsRuntime`（`crates/infra/vcs/`）/ `Watch`（`summary` 取 design §03 表）
- [x] 同文件写 21 条关系，按 design §03 的源分组逐条落笔：`AgentRuntime` 8 条、`ChecksRuntime` 5 条、`Exec` 1 条、`Store` 3 条（含跨域 `Store → App.Backend.Core.Workflow`）、`VcsRuntime` 4 条、`Watch` 0 条
- [x] 自查 阶段四：元素 7 / 关系 21；`grep -n 'from = "App.Backend.Infra.Watch"'` 零命中（出边为零）；`grep -n 'from = "App.Backend.Infra.Store"' -A1` 恰 3 条且目标为 `Core.Agent` / `Core.Foundation` / `Core.Workflow`（无 → `Infra.Exec`，亦无 → `Infra.Watch`）

## 阶段五：Tauri 壳（`04-backend-shell.toml`）

- [x] 新建 `openspec/architecture/models/04-backend-shell.toml`：写 `App.Backend.Shell`（`kind = "module"`、`path = "./packages/desktop/src-tauri/src/"`、`summary` = 「Tauri 壳：IPC 命令面、端口装配与绑定导出」）
- [x] 同文件写 12 条出边：`Core.{Agent,Archi,Config,Foundation,Orchestration,Workflow}` + `Infra.{AgentRuntime,ChecksRuntime,Exec,Store,VcsRuntime,Watch}`；每条 `desc` 写「为哪一面装配哪个端口」，MUST NOT 写 `Cargo.toml` 段落位置；`to` 中 MUST NOT 出现 `App.Backend.Core.Checks`（壳不直依赖 core `checks`）
- [x] 自查 阶段五：元素 1 / 关系 12；`grep -c 'from = "App.Backend.Shell"'` = 12

## 阶段六：构建工具 crate（`05-backend-tools.toml`）

- [x] 新建 `openspec/architecture/models/05-backend-tools.toml`：写 `App.Backend.Tools` 容器 + `App.Backend.Tools.ExportBindings`（`path = "./packages/desktop/src-tauri/crates/tools/export-bindings/"`、`summary` = 「把 IPC 类型导出为前端 TypeScript 绑定」）
- [x] 同文件写 1 条反向边 `App.Backend.Tools.ExportBindings → App.Backend.Shell`，`desc` 写明「为导出命令面 bindings 而依赖壳包」
- [x] 自查 阶段六：元素 2 / 关系 1；该反向边是模型内唯一「工具/壳 方向倒置」的边（与 design D3 一致）

## 阶段七：前端元素（`06-frontend.toml` 前半）

- [x] 新建 `openspec/architecture/models/06-frontend.toml`，写 4 个容器：`App.Frontend.Views`、`App.Frontend.Components`（均 `domain`、空 `path`）、`App.Frontend.Components.Agent`（`domain`、`path = ["./packages/desktop/src/components/agent/index.tsx"]`——**唯一非空 `path` 的容器**）、`App.Frontend.Components.Standard`（`domain`、空 `path`）
- [x] 同文件写 `App.Frontend.Shell`（`module`、`path` 为 4 项数组：`app.tsx` / `routes.tsx` / `main.tsx` / `components/app-sidebar.tsx`，全部 `./packages/desktop/src/` 前缀）
- [x] 同文件写 9 个视图元素：`Views.AgentDebug`（`views/agent/`）、`Views.Agents`、`Views.Architecture`、`Views.Changes`、`Views.Config`、`Views.DbInspector`（`views/db/`）、`Views.Explores`、`Views.Info`（均 `module`、目录 `path`）与 `Views.Welcome`（`module`、`path` 指向 `views/welcome-view.tsx` 单文件）
- [x] 同文件写 3 个子域元素：`Views.Changes.Flow`（`views/changes/flow/`）、`Views.Changes.Renderers`（`views/changes/renderers/`）、`Views.Changes.Agent`（`views/changes/agent/`），均 `module`
- [x] 同文件写 10 个共享组件元素（`kind = "component"`、逐文件 `path`）：`Components.Agent.{AgentBlocks,AgentError,AgentInput,AgentMessages,AgentSessionView,ToolCards}`（`agent-blocks.tsx` / `agent-error.ts` / `agent-input.tsx` / `agent-messages.tsx` / `agent-session-view.tsx` / `tool-cards.tsx`）+ `Components.Standard.{Dailog,Drawer,Pagination,Select}`（`dailog.tsx` / `drawer.tsx` / `pagination.tsx` / `select.tsx`）
- [x] 同文件写 `App.Frontend.Components.Ui`（`kind = "module"`、`path = ["./packages/desktop/src/components/ui/"]`、`summary` = 「shadcn 原语层，恒为被依赖方」）
- [x] 自查 阶段七：本文件元素计数 28（4 容器 + 24 叶）；`grep -c '\[element."App.Frontend"\]' openspec/architecture/models/06-frontend.toml` = 0（`App.Frontend` 本体只在 `01-app.toml`，重复即 `duplicateElement`）；`grep -n 'index.tsx'` 仅出现在 `Components.Agent` 容器的 `path`；`grep -n 'metadata\|docs'` 零命中

## 阶段八：前端关系（`06-frontend.toml` 后半）

- [x] 按 design §06 的 22 个源分组写 64 条 `[[relation]]`：`Shell` 10 条（`Components.Ui` + 9 个视图）、`Components.Agent` 2 条（`AgentInput` / `AgentSessionView`）、`AgentBlocks` 1、`AgentMessages` 3、`AgentSessionView` 4（含 → `App.Backend.Shell`）、`ToolCards` 1、4 个 `Standard.*` 各 1、`Views.AgentDebug` 6、`Views.Agents` 3、`Views.Architecture` 2、`Views.Changes` 6、`Views.Changes.Agent` 3、`Views.Changes.Flow` 6、`Views.Changes.Renderers` 1、`Views.Config` 2、`Views.DbInspector` 3、`Views.Explores` 4、`Views.Info` 2、`Views.Welcome` 1
- [x] 逐条写 `desc`（一句「为什么依赖」）：指向 `App.Backend.Shell` 的一律写「经生成的命令面类型取数与调用」；指向 `Components.Ui` 的写「使用按钮 / 表格等原语呈现」；跨视图组件复用（`Views.Explores → Views.Changes.Renderers`）写「复用 markdown 产物渲染」；MUST NOT 写 import 文件清单
- [x] 硬性排除复核：零 `from == to` 自环；零指向 `hooks` / `lib` / `types` 目标的关系（`types/generated/bindings` 的 import 全部映射到 `App.Backend.Shell`，不建「→ 前端某元素」的边）；零「子 → 父」前缀边（`App.Frontend.Views.Changes.Flow → App.Frontend.Views.Changes` 一类 MUST NOT 出现，指向 `Views.Changes` 的 import 只经 hooks，已按 D4 剔除）
- [x] 自查 阶段八：本文件关系计数 64；`grep -c 'to = "App.Backend.Shell"'` 中前端侧 = 11；出边为零的叶恰为 `Components.Agent.AgentError` / `Components.Agent.AgentInput` / `Components.Ui`

## 阶段九：粒度裁定记录（`decisions/0001-model-granularity.md`）

- [x] 新建 `openspec/architecture/decisions/0001-model-granularity.md`，按 design「ADR 结构」写 8 节：结论 / 三档粒度 / 容器 `path` 恒空与 barrel 例外 / dev 边排除 / 反向边在案 / `gate` 全 `off` / 封闭排除面 / 已知缺口与修改纪律；每节带判据与至少一个「什么不写进模型」的反例
- [x] 正文逐字出现 `App.Backend.Core.Archi`、`App.Backend.Tools.ExportBindings`、`App.Backend.Shell`、`App.Frontend.Components.Agent`、`App.Frontend.Components.Ui`、`App.Frontend.Views.Changes.Flow` 六个 FQN（`docs.rs` 以 `contains(fqn)` 关联，措辞可自然）；同时写明「取数口径 = `Cargo.toml` `[dependencies]` 声明 + `import` 证据」与「新增 crate / 视图 / 子域 / 共享组件 / 跨元素 import 须在同一 change 内补声明」
- [x] 已知缺口节写明：顶层 `src/hooks` 无元素 ⇒ `App.Frontend.Shell → App.Backend.Shell` 缺载体；补齐须新增元素（改 capability），不在本变更
- [x] 自查 阶段九：`grep -c 'App\.Backend\.Core\.Archi'` ≥ 1；ADR 路径**未**出现在任何模型文件的 `docs` 中（`grep -rn 'decisions' openspec/architecture/models` 零命中）；文件名恰为 `0001-model-granularity.md`

## 阶段十：交付自查（命令 + 留痕，非测试）

- [x] 落位与文件划分自查：`ls openspec/architecture/models` 恰 6 个 `NN-<域>.toml`（`01-app` / `02-backend-core` / `03-backend-infra` / `04-backend-shell` / `05-backend-tools` / `06-frontend`）；`ls openspec/architecture/decisions` 恰 1 个 `.md`；`git status --porcelain` 中模型文件只出现在 `openspec/architecture/**`（`src/` `docs/` `openspec/specs/` 下零模型文件）
- [x] 计数自查：`grep -c '^\[element' openspec/architecture/models/*.toml` 逐文件 3 / 8 / 7 / 1 / 2 / 28 且合计 **49**；`grep -c '^\[\[relation\]\]'` 逐文件 0 / 9 / 21 / 12 / 1 / 64 且合计 **107**（与 design「计数恒等式」及「数值口径闭合」同值；若实现时判据枚举得出不同值，以判据枚举结果为准，同步回写 spec delta 与 design 并在报告留痕）
- [x] 双向覆盖自查：分别以命令枚举「crate 目录 + `src-tauri/src/`」「视图目录 + `welcome-view.tsx`」「视图内子域」「共享组件文件 + `components/ui/`」与模型叶元素 `path` 集合求双向差集，四个方向差集均空；并确认 `views/changes/components/`、各视图 `components/`/`hooks/`/`lib/`、`src/hooks`、`src/lib`、`src/types`、`components/standard/hooks`、`*.test.*`、`test-setup.ts` 零元素
- [x] crate 内部零元素自查：把模型全部元素 FQN 末段与 `packages/desktop/src-tauri/crates/*/*/src/*.rs` 的模块文件名（`parse` / `graph` / `dto` / `runner` / `walker` 一类）对拍，确认零交集（crate 内部细分只出现在 `summary` 里，不入元素表）
- [x] 后端关系证据自查：逐 crate 比对各 `Cargo.toml` 的 `[dependencies]` workspace 内声明（`{ workspace = true }` 与 `{ path = … }`）与模型 `[[relation]]` 的 `from`/`to`，43 条逐条相等；`[dev-dependencies]` 的四条反例（`workflow → orchestration`、`orchestration → store`、`workflow → store`、`checks-runtime → workflow`）零出现；`App.Backend.Tools.ExportBindings` 出边恰 1 条且指向 `App.Backend.Shell`
- [x] 前端关系证据自查：抽查 design 表中每个源分组各 1 条（至少覆盖 11 条命令面边、`Views.Explores → Views.Changes.Renderers`、`Views.Changes → Views.Changes.Flow`、`Standard.* → Ui`），确认可指到具体文件的 `import` 语句；反向抽查「有跨元素 import、无对应关系」为零；证据清单文本准备随报告落盘
- [x] 字段纪律自查：`grep -c 'gate = "off"'` 合计 **49**，`grep -c 'gate = "warn"\|gate = "strict"'` 为 0；`grep -c 'metadata'` 为 0；`docs` 出现 1 处（`App` 表内 2 条）；叶元素 `path` 全部以 `"./` 起手、目录以 `/` 结尾；`summary` 49 条全非空且不含自身 FQN 字面
- [x] 图自检（agent 只读工具实调）：`archi_module`（省略 element）返回 49 条薄概览且不报结构错；`archi_module(element="App.Backend.Core.Archi")` 返回 `dependsOn` 恰 1 条（`App.Backend.Core.Foundation`）、`dependedBy` 含 `App.Backend.Infra.AgentRuntime` 与 `App.Backend.Shell`、`children` 空、`warnings` 空、`modelFile` = `openspec/architecture/models/02-backend-core.toml` 且 `modelLine` 指向该元素表头
- [x] 检索自查（agent 只读工具实调）：`archi_search(query="archi", scope=all)` 的 `elements` 含 `App.Backend.Core.Archi` 与 `App.Frontend.Views.Architecture`，`docs` 含 `openspec/architecture/decisions/0001-model-granularity.md`（`kind = adr`、`source = text`、四字段）
- [x] 零代码改动自查（桌面端执行）：`git diff --stat` 只含 `openspec/architecture/**` 与本 change 目录；`bindings:check` 零 diff；静态检查 / 测试结果与基线一致（3239 通过、覆盖率不降）
- [x] 版本自查：`packages/desktop/package.json` `version` 保持 `0.6.0`（不 bump，design D8）；`packages/desktop/src-tauri/tauri.conf.json` 与 `packages/desktop/src-tauri/Cargo.toml` 零 diff；若评审裁定需 bump（O-2），改为 `0.6.1` 单行并复查 `tauri.conf.json` 自动跟随
- [x] 越界面自查：`packages/desktop/src-tauri/crates/**`（含 `crates/core/archi/tests/corpus/**`）、`packages/desktop/src-tauri/src/**`、`packages/desktop/src/**`、`openspec/config.json`、`.gitignore`、`openspec/specs/**`、根 `README.md` / `AGENT.md` / `CLAUDE.md` 全部零 diff
