# 测试设计: frontend-barrel-entry-migration

> **变更**: frontend-barrel-entry-migration（前端目录模块入口化 · 12 个 barrel + 调用点收窄 21 → 12 + `Components.Agent` 容器翻转）
> **日期**: 2026-11-17
> **输入**: `proposal.md`（提案一–七、AC-1…AC-10、R-1…R-11、「变更范围 - 测试文件」1 新增 / 其余零改动、「不要修改」表）；`design.md`（D1–D11、「新增文件」**13 行**、「修改文件」**9 行**、「公共函数 / API」**24 行**、「类型定义」**3 行** + 45 名未出口清单、「派生读数」表、「待决问题」4 条、「变更清单」新测试文件注）；`tasks.md`（阶段零–四 + 移交清单 T1–T6，其中 **T1** 定义新测试文件的五条判据、**T5** 定义 `vi.mock` 与 barrel 的复核边界）；已在案 delta `specs/desktop-frontend-module-entry/spec.md`（Module Contract 表逐行点名 `module-entries.test.ts` 的判据）
> **性质**: 只设计测试范围与用例，不含评估结论、不复制测试运行脚本细节。静态门禁（`pnpm check` 四段）、全量漂移读数、`knip` 反向实验由桌面端 static-check / test-execution 承接。

**测试框架与既有布局（读盘识别结果，本变更零新增依赖、零新增运行器）**

- **运行器**：`vite-plus/test`（vitest 面）；收集面 `src/**/*.test.ts?(x)`；执行器 `vp test --sequence.shuffle`（`packages/desktop/package.json` 的 `client:test` + `vite.config.ts` 的 `test` 节）。
- **断言库**：`import { describe, expect, it } from 'vite-plus/test'`（jest 兼容谓词：`toEqual` / `toContain` / `toBe` / `toMatch` / `not.toHaveProperty`），零第三方断言库。
- **运行环境**：全仓统一 `jsdom`（`vite.config.ts` 的 `test.environment: 'jsdom'` + `globals: true` + `setupFiles: ['src/test-setup.ts']`）。本变更新增的 `module-entries.test.ts` **沿用默认 `jsdom`，MUST NOT 写 per-file 环境覆盖指令**（`// @vitest-environment node` 一类）：全局 `setupFiles`（`src/test-setup.ts:9`）在文件加载期即读 `Element.prototype`，`node` 环境下模块求值当场抛 `Element is not defined` ⇒ 该环境覆盖是**不可用形态**（整个文件 0 用例、加载即失败，连带 `vp test` 退出非零）。`jsdom` 下 `node:fs` / `node:path` 照常可用，故源文本断言**不需要**换环境。
- **环境口径**（回溯修正，test-gen 须照此落地）：既有 `*.test.tsx` 首行普遍写 `// @vitest-environment jsdom`——与默认值同值、无副作用，本变更零改动它们；新测试文件**一律不写**该指令。新测试面的自我约束是「零运行期 import、零组件渲染、零 mock、零 DOM 断言」，**不**包含「跑在 node 环境」；环境归仓级配置，测试文件不自带口径。上游 `proposal.md`「变更范围 - 测试文件」与 `tasks.md` T1 的「node 环境」措辞**以本条为准**（T1 的五条判据本身不含环境项，逐条照旧）。test-gen 落地时须连带删除该文件首行的环境覆盖指令**与其头部注释中「走 node 环境、无 jsdom」一类表述**——注释若保留旧口径，会把已被实测否证的前提再钉一次。
- **既有源文本断言范式**（本稿沿用的既有模式）：`readFileSync(resolve(process.cwd(), relative), 'utf8')`，cwd = 包根 `packages/desktop`——见 `src/components/agent/index.test.tsx:197-200` 与 `src/views/architecture/components/contract-panel.test.tsx:577`；断言风格为「行集合逐字相等 / 字面量计数 / `not.toContain` 负极形态」。
- **测试文件布局**：colocated `*.test.ts(x)` 与源文件同目录；本变更新增 **1** 个测试文件 `packages/desktop/src/module-entries.test.ts`（design 把它排除在「新增文件」表外，归测试相位），**既有测试零改动**（design 事实校准 9 / D9⑦）。
- **本变更零产品运行期行为变化**（12 个 barrel 是纯 re-export；4 个调用点只改 import 形态），故新测试面全部是**源文本 / 目录面断言**：零组件渲染、零 IPC mock、零 DOM 查询——`jsdom` 只是继承的默认环境（`setupFiles` 需要它），用例不触碰任何 DOM API、不装载任何实现模块。

### 被测模块清点（design「变更清单」两张子表逐项）

| # | 路径（相对项目根） | 来源 | 解析到的测试文件 | 判定 |
|---|---|---|---|---|
| 1 | `packages/desktop/src/views/agent/index.ts` | 新增文件 | `packages/desktop/src/module-entries.test.ts` | 源文本断言，`## 单元测试` 第 1 节 |
| 2 | `packages/desktop/src/views/agents/index.ts` | 新增文件 | 同上 | 第 2 节 |
| 3 | `packages/desktop/src/views/architecture/index.ts` | 新增文件 | 同上 | 第 3 节 |
| 4 | `packages/desktop/src/views/config/index.ts` | 新增文件 | 同上 | 第 4 节 |
| 5 | `packages/desktop/src/views/db/index.ts` | 新增文件 | 同上 | 第 5 节 |
| 6 | `packages/desktop/src/views/info/index.ts` | 新增文件 | 同上 | 第 6 节 |
| 7 | `packages/desktop/src/views/changes/index.ts` | 新增文件 | 同上 | 第 7 节 |
| 8 | `packages/desktop/src/views/changes/agent/index.ts` | 新增文件 | 同上 | 第 8 节 |
| 9 | `packages/desktop/src/views/changes/flow/index.ts` | 新增文件 | 同上 | 第 9 节（压力样本，11 名 / 41 名） |
| 10 | `packages/desktop/src/views/changes/renderers/index.ts` | 新增文件 | 同上 | 第 10 节 |
| 11 | `packages/desktop/src/views/explores/index.ts` | 新增文件 | 同上 | 第 11 节 |
| 12 | `packages/desktop/src/components/ui/index.ts` | 新增文件 | 同上 | 第 12 节（39 条 `export *` / 零重名） |
| 13 | `packages/desktop/src/routes.tsx` | 修改文件 | `packages/desktop/src/app.test.tsx`（既有，零改动） | 只改 import 形态，design 未声明公共 API 变更 ⇒ 第 13 节零新增用例表行 + 不可测试项 4 |
| 14 | `packages/desktop/src/views/changes/change-detail-view.tsx` | 修改文件 | `packages/desktop/src/views/changes/change-detail-view.test.tsx`（既有，零改动） | 同上 ⇒ 第 14 节零新增用例表行 + 不可测试项 4 / 9（T5 mock 复核） |
| 15 | `packages/desktop/src/views/changes/flow/detail-drawer.tsx` | 修改文件 | `packages/desktop/src/views/changes/flow/detail-drawer.test.tsx`（既有，零改动） | 同上 ⇒ 第 15 节零新增用例表行 + 不可测试项 4 |
| 16 | `packages/desktop/src/views/explores/components/explore-preview.tsx` | 修改文件 | `packages/desktop/src/views/explores/components/explore-preview.test.tsx`（既有，零改动） | 同上 ⇒ 第 16 节零新增用例表行 + 不可测试项 4 |
| 17 | `.dev-team/architecture/models/06-frontend.toml` | 修改文件 | — | 模型数据资产 ⇒ 不可测试项 1 |
| 18 | `.dev-team/architecture/decisions/0001-model-granularity.md` | 修改文件 | — | 裁定记录文本 ⇒ 不可测试项 2 |
| 19 | `packages/desktop/package.json` | 修改文件 | — | 版本号单值 ⇒ 不可测试项 7 |
| 20 | `packages/desktop/src-tauri/tauri.conf.json` | 修改文件（**零内容改动**） | — | 引用形态核对项 ⇒ 不可测试项 7 |
| 21 | `packages/desktop/src-tauri/Cargo.toml` | 修改文件（**零改动**） | — | crate 版本行对照 ⇒ 不可测试项 7 |
| 22 | `openspec/changes/frontend-barrel-entry-migration/reports/module-entry-evidence.md` | 新增文件 | — | AC 实测留痕载体（非源文件）⇒ 不可测试项 8 |

无删除文件 ⇒ 无删除面纳入被测范围。**per-file 映射对共 16 组**（第 1–12 组共享同一新测试文件；第 13–16 组指向零改动的既有测试文件，章节保留完整模板（`#### 待测功能` / `#### 用例` / `#### Mock策略`）并以「零新增用例 + 理由」表行登记）。design「修改文件」表内的 `packages/desktop/src-tauri/tauri.conf.json` / `Cargo.toml` 为零改动对照项，与 `package.json` 同归不可测试项 7。

### 参数类型 → 边界覆盖口径

| 类型 | 本变更的入参 / 载荷 | 边界覆盖（最少条数） | 落地用例 |
|---|---|---|---|
| str / string | barrel 的 specifier（`'./agent-debug-view'` / `'./types'`）、出口名（`AgentDebugView` / `DrawerSelection`）、ui 的模块名段（`'./alert'`） | `''`（空 specifier）、超长（>1000 chars）、特殊字符（emoji / `\n` / `\0` / 引号）、None（裸 `export { X }` 无 specifier ⇒ 非法形态） | 各章节「specifier 形态」行；第 9 / 12 节 |
| list / array | 每个 barrel 的有效语句行集合、出口名集合；ui 的 39 条 `export *`；`readdirSync` 的目录模块清单 | `[]`（空 barrel / 仅注释）、单元素（6 个单文件目录各 1 条）、超大（ui 39 条 / flow 11 名 / ui 242 名） | 「行集合逐字」行；「空文件负样本」行；第 12 节规模用例 |
| dict / object | 「出口名 → specifier」映射（每 barrel 1 张；规模：6 个单文件目录各 1 项、`changes` 2、`changes/agent` 1、`renderers` 2、`explores` 3、`flow` 11、`ui` 39） | `{}`（空映射）、缺必填行（名单少一名）、多余行（多导一名）、None（无内容） | 「行集合逐字」「未出口名零命中」「负样本」 |
| Optional[T] | ui 首行注释（在场 / 缺失）；`*Props` 与子域内部名外露与否 | `None`（注释缺失 / 名字不以任何形态出现） | 第 12 节「首行注释」；第 1 / 5 / 8 / 11 节「`*Props` 零命中」 |
| Enum | 语句形态三值：值出口 `export { … } from` / 类型出口 `export type { … } from` / 通配 `export * from`（仅 ui） | 每值 + 非法形态（`export default`、`export const …`、裸 `export { X }`、`import … from`） | 第 9 节「`export type` 正反样本」；第 12 节「零 `export { … } from`」 |
| int / number | 语句条数（1 / 2 / 3 / 7 / 39）、出口名数（1 / 2 / 3 / 11 / 39 / 242） | `0`（空）、`1`（单文件目录）、`39` / `242`（超大） | 各章节「恰 N 条 / N 名」行 |
| bool | 类型出口是否 `export type`（真 / 假） | `true` / `false`（`None` 不适用：每条语句必属其一类） | 第 9 节 `export type` 双夹具对拍 |
| float | 无 | 不适用 | 本变更零浮点入参 / 载荷 |

---

## 验收范围

<!-- 列口径：`被测文件或模块` = 承载该 AC 的源文件 + 测试文件章节号 + 不可测试项序号。
     12 个 barrel 全部映射到 `packages/desktop/src/module-entries.test.ts`（新增），路径在章节标题中写全、本表内缩写为 `views/…` 与 `components/ui/…`。 -->

| AC ID | 验收条件 | 被测文件或模块 |
|-------|----------|----------------|
| AC-1 | **12 个入口在案且解析面归零**：12 个目录各有且仅有一个 `index.ts`（`components/agent/index.tsx` 不在 12 个之列、逐字不动），每个入口落在其元素 `path` 之内；`entry_state` **`Resolved` 44 / `Absent` 0 / `Invalid` 0**；模型**零显式 `entry` 加行** | 12 个 barrel（第 1–12 节的「在盘 + 落位 + 形态」用例）；`06-frontend.toml`（不可测试项 1）；全量 `entry_state`（不可测试项 3） |
| AC-2 | **边集逐条相等**：同源检查前后前端 `relations` 的 `from` / `to` / `desc` 三列 diff 为空、**71 条 / 24 源**不变、`unresolved` 与 `scan.unmatchedFiles` 零新增；反向判据：任一 barrel 跨子元素 re-export ⇒ 边数 > 71 判红 | 12 个 barrel 的 specifier 纪律（各节「零跨子元素 / 零 `hooks`·`lib`」行）；4 个调用点的目标元素映射（第 13–16 节 + 不可测试项 4）；全量 `relations`（不可测试项 3） |
| AC-3 | **入口 = 公共面**：`vp check`（含 `knip`）零 dead code；每个手挑导出名都能在非 barrel 非测试调用点找到 `import`；反向：多导一名 ⇒ `knip` 报 unused、少导一名 ⇒ typeCheck 红；`components/ui/index.ts` 例外（`knip` `ignore` 内）由源文本断言守 | 11 个手挑 barrel 的名单逐字（第 1–11 节）+ ui 的 39 条 `export *` 与零重名（第 12 节）；`knip` / typeCheck 反向面（不可测试项 5） |
| AC-4 | **调用点收窄实测**：`routes.tsx` 11 → **8**、`change-detail-view.tsx` 8 → **2**、`detail-drawer.tsx` 1 → **1**、`explore-preview.tsx` 1 → **1**（合计 21 → **12**）；12 个目录零残留文件级跨元素 import；`@/components/ui/<mod>` **同命令前后同值**（design D6 改述：132 与 112 / 55 / 34 两组口径各记一遍） | 4 个调用点文件（第 13–16 节的零新增用例表行与既有回归承接面；形态判据见不可测试项 4）；ui 计数（不可测试项 4） |
| AC-5 | **判据面换段而非清零**：`apiFindings` = `apiEntryMissing` **0** + `apiMissing` **41**（净 0、全平移），其余七条 0，总 **41**；`status = clean`、`passed = true`；`api:check` 三列空、`INDEX.toml` 零 diff | 12 个 barrel（入口在案 = 12 条由 ① 面平移的前提）；`06-frontend.toml`（不可测试项 1）；全量 `apiFindings`（不可测试项 3）；`api:check` / 生成物（不可测试项 5） |
| AC-6 | **模型与计数换段无溢出**：`archi_graph(root)` 元素 **53** / 关系 **120** / `06` 段 **30 / 71** / `diagnostics` 与 `warnings` 空；**9 容器 + 44 叶**（`06` 段 3 + 27）；`Components.Agent` 的 `kind = "module"`、`path` 为目录、`entry` 零行、7 条端点边在案；`grep -c "^entry"` **3**、`api = "none"` **2** | `06-frontend.toml`（不可测试项 1）；ADR `§9`（不可测试项 2）；全量 `archi_graph`（不可测试项 3） |
| AC-7 | **子域判据可复算**：`views/` 内递归非测试源 ≥ 3 的子目录恰四条 —— `3 ./architecture/agent`、`4 ./changes/agent`、`16 ./changes/flow`、`8 ./changes/renderers`；`components/` 下零命中 | 目录面实测（不可测试项 4）；ADR `§2` 的三处换值（不可测试项 2） |
| AC-8 | **零扩散**：`git status --porcelain` 改动集 ⊆ 白名单；`packages/desktop/src-tauri/**` 与 `.dev-team/architecture/api/**` 零 diff；`bindings:export` 幂等 | 零 diff 面（不可测试项 6）；生成物 / 绑定幂等（不可测试项 5） |
| AC-9 | **版本纪律**：`package.json` 的 `version` 相对落地时 `HEAD` 现值恰 +1 patch（`0.6.49` → **`0.6.50`**，只加一次）；`tauri.conf.json` 仍为 `"version": "../package.json"`；`Cargo.toml` 的 `[package] version` 逐字不变 | `package.json` / `tauri.conf.json` / `Cargo.toml`（不可测试项 7） |
| AC-10 | **相关能力零漂移**：`openspec/specs/**` 除 `desktop-frontend-module-entry`（新增）、`repo-archi-model`、`desktop-archi-api`、`desktop-archi-api-check`、`desktop-archi-file-tree` 五个能力外零 diff；其余 8 个相邻能力零 diff | `openspec/specs/**` 零 diff 核对（不可测试项 6） |

> **覆盖核对**：proposal AC-1…AC-10 十条逐条在表内（10 行，编号独立、无合并、无遗漏）。清点表 22 项逐项落表 —— ①–⑫ 12 个 barrel（AC-1/2/3/5）、⑬–⑯ 4 个调用点（AC-2/4）、⑰ 模型（AC-1/5/6）、⑱ ADR（AC-6/7）、⑲–㉑ 版本面（AC-9）、㉒ 留痕（AC-5/8）。
> **被测范围边界**：12 个目录内 39 + 40 个既有实现文件、`components/agent/**`（既有 barrel + 实现 + 其 `index.test.tsx`）、`components/ui/**` 既有文件、`src-tauri/**`、`src/types/generated/**`、`.dev-team/architecture/api/**`、`openspec/specs/**`、`knip.json` / `vite.config.ts` / `tsconfig.json` 均为 design D9 / D10 界定的**零触碰面**，不纳入被测范围；其不变性由 AC-8 / AC-10 的零 diff 核对与既有套件全绿承接。

---

## 单元测试

### packages/desktop/src/views/agent/index.ts -> packages/desktop/src/module-entries.test.ts

#### 待测功能

- `AgentDebugView`: 新增出口 `export { AgentDebugView } from './agent-debug-view';`（实现零改动，签名 `function AgentDebugView({ root }: AgentDebugViewProps): React.JSX.Element` 逐字不变）；`AgentDebugViewProps`（`agent-debug-view.tsx:13` 的 interface）**MUST NOT** 出现在出口面

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 入口在盘与落位 | 正向 | `resolve(process.cwd(), 'src/views/agent/index.ts')` 可读（读失败即红），且该路径落在元素 `path`（`./packages/desktop/src/views/agent/`）之内 | 单例 |
| 有效语句行集合 | 正向 | 去掉空行后逐字等于 `["export { AgentDebugView } from './agent-debug-view';"]`（恰 1 条语句、恰 1 名、单引号 + 分号 + 单行形态） | 单例 |
| `AgentDebugView` 出口在场 | 正向 | 抽取的出口名集合 == `{'AgentDebugView'}`（不是 `export *`、不是 `export default`） | 单例 |
| `AgentDebugViewProps` 不外露 | 异常 | 源文本零命中 `AgentDebugViewProps`（`*Props` 不进公共面；出现即判红） | 单例 |
| 零实现语句与零注释 | 边界 | `^(const\|let\|var\|function\|class\|interface\|type\|enum)\s`、`^import `、`export default` 与注释行（`//` / `/*`）四类全部零命中（design D1「零实现、零注释」，ui 是唯一注释例外） | 表驱动（4 行） |
| specifier 形态 | 边界 | 每条 `from '…'` 值满足 `^\./[a-z0-9-]+$`：零 `../`、零 `./<子目录>/`、零 `./hooks`、零 `./lib`、零 `@/` 别名、零扩展名（`./x.tsx`）、零自指（`./index`） | 逐值迭代（合法 1 + 非法 7 值） |
| 抽取器判红面自证 | 边界 | 空源与「仅一行 `// x`」两份合成样本 ⇒ 抽取结果为空集合（与期望不等 ⇒ 判红）；证明「行集合逐字相等」不是恒真断言 | 逐值迭代（2 值） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `node:fs`（`readFileSync`） | **不 mock**：真实读盘，路径 = `resolve(process.cwd(), 'src/views/agent/index.ts')`（vitest cwd = 包根 `packages/desktop`，沿既有 `sourceOf` 范式） | 全部用例 |
| 实现模块 `./agent-debug-view` 及其依赖（`react` / `@tauri-apps/api/core` …） | **不 import、不 mock**：本测试文件零运行期 import（第一条判据即「纯源文本」）⇒ 组件不装载、无需替身 | 全部用例 |
| 进程边界（网络 / DB / 子进程 / 环境变量 / IPC） | 不存在：只有读盘与纯字符串断言；环境为继承的默认 `jsdom`（本文件不写覆盖指令） | — |

---

### packages/desktop/src/views/agents/index.ts -> packages/desktop/src/module-entries.test.ts

#### 待测功能

- `AgentsView`: 新增出口 `export { AgentsView } from './agents-view';`（签名 `function AgentsView(): React.JSX.Element` 零改动）；该目录是**单文件目录**（1 个非测试源文件）形态的代表

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 单文件目录入口在盘 | 正向 | `src/views/agents/index.ts` 可读，且目录内非测试源文件恰 **1** 个（`agents-view.tsx`）——单文件目录同样持 barrel（design 提案一「统一形态」） | 单例 |
| 有效语句行集合 | 正向 | 逐字等于 `["export { AgentsView } from './agents-view';"]` | 单例 |
| 出口名集合 | 正向 | == `{'AgentsView'}`；零 `Props`（该组件无形参、零 props interface） | 单例 |
| 多余一行负样本 | 异常 | 在合成文本追加 `export { AgentPanel } from './components/agent-panel';`（跨 `components/` 子目录 + 未在名单）⇒ 行集合不等且 specifier 形态判红（双判据同时命中） | 单例 |
| 边界形态 | 边界 | 空源 ⇒ 0 条；仅 `export {};` ⇒ 0 名（与 1 条 1 名的期望不等） | 逐值迭代（2 值） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `node:fs` | **不 mock**：真实读 barrel 与目录清单 | 全部用例 |
| 实现模块与三方依赖 | **不 import、不 mock**（零运行期装载） | 全部用例 |
| 进程边界 | 不存在 | — |

---

### packages/desktop/src/views/architecture/index.ts -> packages/desktop/src/module-entries.test.ts

#### 待测功能

- `ArchitectureView`: 新增出口 `export { ArchitectureView } from './architecture-view';`（签名 `function ArchitectureView({ root }: { root: string | null }): React.JSX.Element` 零改动）；`ArchitectureCanvas` **不在此公共面**（design D6 的两处深路径留白：`views/architecture/components/architecture-canvas` 改写属独立变更）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 有效语句行集合 | 正向 | 逐字等于 `["export { ArchitectureView } from './architecture-view';"]` | 单例 |
| 出口名集合 | 正向 | == `{'ArchitectureView'}` | 单例 |
| `ArchitectureCanvas` 零命中 | 异常 | 源文本零命中 `ArchitectureCanvas`（且零 `./components/architecture-canvas` 一类跨子目录 specifier）——公共面不外露画布组件，两处 `architecture-canvas` 深路径 import 的留白由 design D6 登记，本测试**不得**因判红而反向要求扩公共面 | 单例 |
| specifier 形态 | 边界 | 每条满足 `^\./[a-z0-9-]+$`；非法样本含 `'./components/architecture-canvas'`（子目录）、`'../architecture-view'`（父目录） | 逐值迭代（2 值） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `node:fs` | **不 mock**：真实读盘 | 全部用例 |
| 实现模块与依赖 | **不 import、不 mock** | 全部用例 |
| 进程边界 | 不存在 | — |

---

### packages/desktop/src/views/config/index.ts -> packages/desktop/src/module-entries.test.ts

#### 待测功能

- `ConfigView`: 新增出口 `export { ConfigView } from './config-view';`（签名 `function ConfigView({ root }: { root: string }): React.JSX.Element` 零改动）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 有效语句行集合 | 正向 | 逐字等于 `["export { ConfigView } from './config-view';"]` | 单例 |
| 出口名集合 | 正向 | == `{'ConfigView'}`（该目录 `components/` 的 5 个 section 组件与 `hooks/` 均不外露） | 单例 |
| 子目录名零命中 | 异常 | 零 `./components/` 与零 `./hooks/` specifier；零 `./hooks/use-workspace-config` 形态（`hooks/` 的 owner 就是所属视图，机械判据拦不住 ⇒ 由本断言拦） | 表驱动（2 行） |
| 行数边界 | 边界 | 恰 1 条非空行、零注释行、零 `export default` | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `node:fs` | **不 mock**：真实读盘 | 全部用例 |
| 实现模块与依赖 | **不 import、不 mock** | 全部用例 |
| 进程边界 | 不存在 | — |

---

### packages/desktop/src/views/db/index.ts -> packages/desktop/src/module-entries.test.ts

#### 待测功能

- `DbInspectorView`: 新增出口 `export { DbInspectorView } from './db-inspector-view';`（签名 `function DbInspectorView({ root }: DbInspectorViewProps): React.JSX.Element` 零改动）；`DbInspectorViewProps`（`db-inspector-view.tsx:274`）**MUST NOT** 外露

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 有效语句行集合 | 正向 | 逐字等于 `["export { DbInspectorView } from './db-inspector-view';"]` | 单例 |
| 出口名集合 | 正向 | == `{'DbInspectorView'}` | 单例 |
| `DbInspectorViewProps` 不外露 | 异常 | 源文本零命中 `DbInspectorViewProps` | 单例 |
| specifier 形态 | 边界 | `^\./[a-z0-9-]+$`；非法样本 `'./db-inspector-view.tsx'`（带扩展名）、`'./hooks/use-db-inspector'` | 逐值迭代（2 值） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `node:fs` | **不 mock**：真实读盘 | 全部用例 |
| 实现模块与依赖 | **不 import、不 mock** | 全部用例 |
| 进程边界 | 不存在 | — |

---

### packages/desktop/src/views/info/index.ts -> packages/desktop/src/module-entries.test.ts

#### 待测功能

- `InfoView`: 新增出口 `export { InfoView } from './info-view';`（签名 `function InfoView({ root }: { root: string }): React.JSX.Element` 零改动）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 有效语句行集合 | 正向 | 逐字等于 `["export { InfoView } from './info-view';"]` | 单例 |
| 出口名集合 | 正向 | == `{'InfoView'}`（`components/` 的 `dir-tree` / `language-table` / `stats-summary` 与 `hooks/use-code-stats` 不外露） | 单例 |
| 子目录与 hooks 零命中 | 异常 | 零 `./components/`、零 `./hooks/`、零 `./lib/` specifier | 表驱动（3 行） |
| 六个单文件目录的一致性 | 边界 | 第 1–6 节的六份 barrel 与第 2 节同构：各 1 条语句 / 1 名 / 零注释 / 零实现语句（跨节同一断言器，表驱动 6 个目录） | 表驱动（6 行，每行一个目录） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `node:fs` | **不 mock**：真实读盘（含 `readdirSync` 目录文件数） | 全部用例 |
| 实现模块与依赖 | **不 import、不 mock** | 全部用例 |
| 进程边界 | 不存在 | — |

---

### packages/desktop/src/views/changes/index.ts -> packages/desktop/src/module-entries.test.ts

#### 待测功能

- `ChangeDetailView`: 新增出口 `export { ChangeDetailView } from './change-detail-view';`（签名 `function ChangeDetailView({ root }: { root: string | null })` 零改动）
- `ChangeListView`: 新增出口 `export { ChangeListView } from './change-list-view';`（签名同上形态，零改动）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 有效语句行集合 | 正向 | 逐字等于 `["export { ChangeDetailView } from './change-detail-view';", "export { ChangeListView } from './change-list-view';"]`（按来源模块名字典序） | 单例 |
| `ChangeDetailView` 出口在场 | 正向 | 出口名集合含该名且来自 `./change-detail-view`（值形态，零 `type` 修饰） | 表驱动（2 行，两个名字各一行） |
| `ChangeListView` 出口在场 | 正向 | 同上，来源 `./change-list-view` | 同上（同一表 2 行） |
| 零跨子元素 re-export（D1 / R-9） | 异常 | 零 `./flow/…`、`./agent/…`、`./renderers/…`、`./components/…`、`./hooks/…` specifier；子域公共面名（`buildFlowGraph` / `ChangeFlowGraph` / `DetailDrawer` / `RunControlPanel` / `mountMaterials` / `runStepNodes` / `unifiedRunEdges` / `unifiedRunNodes` / `ChangeChatPanel` / `ArtifactTabs` / `MarkdownDocRenderer`）逐个零命中（跨子元素 re-export 会在 `scan` 里真加边 ⇒ 前端关系由 71 变 72） | 表驱动（11 行） |
| 行数与形态 | 边界 | 恰 2 条语句、2 名、零注释、零实现语句、零 `export default` | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `node:fs` | **不 mock**：真实读盘 | 全部用例 |
| 实现模块与依赖 | **不 import、不 mock** | 全部用例 |
| 进程边界 | 不存在 | — |

---

### packages/desktop/src/views/changes/agent/index.ts -> packages/desktop/src/module-entries.test.ts

#### 待测功能

- `ChangeChatPanel`: 新增出口 `export { ChangeChatPanel } from './change-chat-panel';`（签名 `function ChangeChatPanel(props: ChangeChatPanelProps): React.JSX.Element` 零改动）；`ChangeChatComposer` / `ChangeChatPanelProps` / `ChangeChatComposerProps` **均不外露**（目录内 4 名只出 1 名）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 有效语句行集合 | 正向 | 逐字等于 `["export { ChangeChatPanel } from './change-chat-panel';"]` | 单例 |
| `ChangeChatPanel` 出口在场 | 正向 | 出口名集合 == `{'ChangeChatPanel'}` | 单例 |
| 未出口三名零命中 | 异常 | 源文本零命中 `ChangeChatComposer`、`ChangeChatPanelProps`、`ChangeChatComposerProps`（`ChangeChatComposer` 是 `change-chat-composer.tsx` 的组件、与面板同元素但非公共面） | 表驱动（3 行） |
| 目录 `hooks/` 不外露 | 异常 | 零 `./hooks/…` specifier（该目录有 `hooks/use-change-chat.ts`：`hooks/` 的 owner 就是本视图，机械判据拦不住） | 单例 |
| specifier 与行数 | 边界 | 恰 1 条语句、specifier 满足 `^\./[a-z0-9-]+$`；非法样本 `'./change-chat-composer'`（同元素但非公共面 ⇒ 行集合不等） | 逐值迭代（2 值） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `node:fs` | **不 mock**：真实读盘 | 全部用例 |
| 实现模块与依赖 | **不 import、不 mock** | 全部用例 |
| 进程边界 | 不存在 | — |

---

### packages/desktop/src/views/changes/flow/index.ts -> packages/desktop/src/module-entries.test.ts

#### 待测功能

- `mountMaterials()`: 新增出口 ← `./attachments`（签名 `function mountMaterials(detail: ChangeDetail, envelopes: ArtifactEnvelope[]): FlowMaterials` 零改动）
- `ChangeFlowGraph()`: 新增出口 ← `./change-flow-graph`（`function ChangeFlowGraph(props: ChangeFlowGraphProps): React.JSX.Element`）
- `DetailDrawer()`: 新增出口 ← `./detail-drawer`（`function DetailDrawer({ … }: DetailDrawerProps): React.JSX.Element | null`）
- `buildFlowGraph()`: 新增出口 ← `./graph`（`function buildFlowGraph(detail: ChangeDetail, runNodes?: RuntimeFlowNode[], runEdges?: readonly UnifiedRunEdge[]): FlowGraph`）
- `RunControlPanel()`: 新增出口 ← `./run-control-panel`（`function RunControlPanel({ … }: RunControlPanelProps): React.JSX.Element`）
- `runStepNodes()` / `unifiedRunEdges()` / `unifiedRunNodes()`: 新增出口 ← `./run-state`（单条语句三名，签名各零改动）
- `DrawerSelection` / `FlowGraph` / `FlowMaterials`（类型）: 新增**类型**出口 ← `./types`，`isolatedModules` ⇒ 必须以 `export type { … }` 形态

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 有效语句行集合（压力样本） | 正向 | 逐字等于 7 条语句，按来源模块名字典序：`./attachments` → `./change-flow-graph` → `./detail-drawer` → `./graph` → `./run-control-panel` → `./run-state` → `./types`（11 名 / 目录 41 个具名导出只出这 11 个） | 单例 |
| 8 个值出口逐条在场 | 正向 | `mountMaterials` / `ChangeFlowGraph` / `DetailDrawer` / `buildFlowGraph` / `RunControlPanel` / `runStepNodes` / `unifiedRunEdges` / `unifiedRunNodes` 各在值出口行内；`runStepNodes` / `unifiedRunEdges` / `unifiedRunNodes` 三名同属一条 `./run-state` 语句 | 表驱动（8 行） |
| 3 个类型出口必须 `export type` | 正向 | `DrawerSelection` / `FlowGraph` / `FlowMaterials` 全部落在 `export type { DrawerSelection, FlowGraph, FlowMaterials } from './types';` 一条语句内 | 单例 |
| 类型出口的值形态负样本（R-5） | 异常 | `export { DrawerSelection, FlowGraph, FlowMaterials } from './types';`（值形态）⇒ 判红：转译期被当值导出、运行期 import 未定义，且与本仓 `isolatedModules` 纪律冲突 | 双夹具对拍 |
| 未出口名零命中 | 异常 | 目录内其余 **30** 个具名导出逐个零命中（例：`nodeIdentityKey` / `runNodeKey` / `UnifiedRunStep` / `UnifiedRunEdge` / `LiveRunFace` / `FlowNode` / `FlowEdge` / `FlowColumn` / `RuntimeFlowNode` / `RoleSessionRef` / `FlowEdgeKind` / `RunStepGroup` / `FlowRoleLabel` / `graphTranslateExtent` / `columnLayout` / `nodeLayout` / `edgeHandle` / `settledDurationLabel` / `runningDurationLabel` / `formatTokens` / `LabeledEdge` / `PhaseColumnNode` / `RunStepNode` / `SessionTranscriptPanel` / `StdoutPanel` / `normalizeCarriageReturns` / `isStuckToBottom` / `COLUMN_HEADER_H` / `ColumnFlowNode` / `RunStepFlowNode`）——「多导一名即虚增公共面」（R-10） | 表驱动（30 行） |
| specifier 形态与零跨子元素 | 边界 | 每条满足 `^\./[a-z0-9-]+$`：零 `../`、零 `./<子目录>/`、零 `./hooks`、零 `./lib`、零 `@/`、零 `./types.ts`（带扩展名） | 逐值迭代（合法 7 + 非法 6 值） |
| 零实现语句与零注释 | 边界 | `^(const\|let\|var\|function\|class\|interface\|type\|enum)\s`、`^import `、`export default`、注释行四类零命中（38 个公开符号只经实现文件维护，入口不承载实现） | 表驱动（4 行） |
| 少一条语句的负样本 | 边界 | 删 `runStepNodes` 所在名 ⇒ 行集合与名单不等（对应 AC-3 反向判据「少导一名即断调用点」，由 typeCheck 与本节双守） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `node:fs` | **不 mock**：真实读 barrel 与目录清单（41 个具名导出的分母来自实测，不写成常量） | 全部用例 |
| 实现模块（`attachments.ts` / `graph.ts` / `run-state.ts` / `types.ts` / 6 个组件） | **不 import、不 mock**：零运行期装载（组件会拉 `@xyflow/react` / `@tauri-apps/api`）；「未出口名零命中」也以源文本为准 | 全部用例 |
| 进程边界 | 不存在 | — |

---

### packages/desktop/src/views/changes/renderers/index.ts -> packages/desktop/src/module-entries.test.ts

#### 待测功能

- `ArtifactTabs()`: 新增出口 ← `./artifact-tabs`（`function ArtifactTabs({ artifacts }: { artifacts: ArtifactEnvelope[] }): React.JSX.Element`）
- `MarkdownDocRenderer`: 新增出口 ← `./markdown-doc-renderer`（`const MarkdownDocRenderer = memo(MarkdownDocRendererBase)`，`MemoExoticComponent` 形态零改动）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 有效语句行集合 | 正向 | 逐字等于 `["export { ArtifactTabs } from './artifact-tabs';", "export { MarkdownDocRenderer } from './markdown-doc-renderer';"]` | 单例 |
| `ArtifactTabs` 出口在场 | 正向 | 出口名集合含该名、来源 `./artifact-tabs` | 单例 |
| `MarkdownDocRenderer` 出口在场 | 正向 | 出口名集合含该名、来源 `./markdown-doc-renderer`（`memo(...)` 形态不影响「导出名」断言） | 单例 |
| 其余 6 个导出零命中 | 异常 | 源文本零命中 `RendererComponent` / `resolveRenderer` / `ArtifactView` / `EvalChecklistRenderer` / `Fallback` / `TasksProgressRenderer`（registry 与 renderer 实现不进公共面；`registry.ts` 只被同元素文件消费） | 表驱动（6 行） |
| 行数边界 | 边界 | 恰 2 条语句、2 名、零注释；非法样本：把 `./registry` 的 `resolveRenderer` 加进来 ⇒ 行集合不等 + 零命中判据同时红 | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `node:fs` | **不 mock**：真实读盘 | 全部用例 |
| 实现模块与依赖（`react-markdown` / `shiki` …） | **不 import、不 mock**（零运行期装载；渲染行为由既有 `markdown-doc-renderer.test.tsx` 覆盖、本变更零改动） | 全部用例 |
| 进程边界 | 不存在 | — |

---

### packages/desktop/src/views/explores/index.ts -> packages/desktop/src/module-entries.test.ts

#### 待测功能

- `ExploreDetailView()`: 新增出口 ← `./explore-detail-view`（`function ExploreDetailView({ root }: { root: string }): React.JSX.Element`）
- `ExploreLayout()`: 新增出口 ← `./explore-layout`（`function ExploreLayout({ root }: { root: string }): React.JSX.Element`）
- `ExploreListView()`: 新增出口 ← `./explore-list-view`（`function ExploreListView({ root }: { root: string }): React.JSX.Element`）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 有效语句行集合 | 正向 | 逐字等于 3 条语句：`./explore-detail-view` → `./explore-layout` → `./explore-list-view` | 单例 |
| 三名出口逐条在场 | 正向 | `ExploreDetailView` / `ExploreLayout` / `ExploreListView` 各在对应来源语句内（值形态） | 表驱动（3 行） |
| 未出口名零命中 | 异常 | 源文本零命中 `ExploreDetailPane` / `ExploreDetailPaneProps` / `ExploreRouteContext` / `useExploreRouteContext`（目录内 7 名只出 3 名：路由上下文与详情面板只被同元素文件与测试直取） | 表驱动（4 行） |
| 目录 `hooks/` 不外露 | 异常 | 零 `./hooks/…` specifier（`use-explore-list` / `use-explore-doc` / `use-explore-session` 三个 hook 非公共面） | 单例 |
| specifier 形态 | 边界 | `^\./[a-z0-9-]+$`；非法样本 `'./components/explore-composer'`（子目录）、`'../../changes/renderers'`（父级跨元素） | 逐值迭代（2 值） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `node:fs` | **不 mock**：真实读盘（含目录清单：顶层 4 个非测试源文件 / 7 名导出） | 全部用例 |
| 实现模块与依赖 | **不 import、不 mock** | 全部用例 |
| 进程边界 | 不存在 | — |

---

### packages/desktop/src/components/ui/index.ts -> packages/desktop/src/module-entries.test.ts

#### 待测功能

- 39 条 `export *`: 新增出口面 —— `export * from './<mod>';` 逐模块一条（目录内 39 个非测试源文件各一条，按模块名字典序）；**唯一例外**：`components/ui` 在 `knip.json` 的 `ignore` 与 `vite.config.ts` 的 `NO_STATIC_CHECK` 内 ⇒ 本文件是该目录**唯一**的机械判据（不手维护「被引用的 34 个模块」白名单）
- 首行外部约束注释: 说明「本文件是目录模块的入口声明 / 视图内直引模块名是刻意选择 / 勿据此判死代码」

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 39 条 `export *` 与形态 | 正向 | 非空非注释行恰 **39** 条，每条形如 `export * from './<mod>';`（单引号 + 分号 + 单行），且按 `<mod>` 字典序排列 | 规模迭代（39 行） |
| specifier ↔ 目录模块一一对应 | 正向 | `readdirSync('src/components/ui')` 的 **39** 个非测试源文件（全 `.tsx`，零顶层 `.ts`）与 39 个 specifier 双向一一对应：每条指向现存文件、specifier 零重复 | 规模迭代（39 项） |
| 首行外部约束注释 | 正向 | 首行是注释且同时含「入口」与「勿据此判死代码」两处语义（design D2 的唯一注释例外）；首行之后零第二条注释 | 单例 |
| 零 `export { … } from` / 零 `export default` | 异常 | `export {`（非 `*` 形态）零命中、`export default` 零命中 —— 手维护白名单与默认导出两形态均判红 | 表驱动（2 行） |
| 零实现语句 | 异常 | `^(const\|let\|var\|function\|class\|interface\|type\|enum)\s` 与 `^import ` 零命中（入口不承载实现 / 不做二次装载） | 表驱动（2 行） |
| 39 模块具名导出零重名 | 正向 | 逐模块抽取具名导出（`export { … }` 块含跨行块、`export type { … }`、`export (function\|const\|class\|interface\|type\|enum) <名>`），合计恰 **242** 名、**零重名**（`export *` 的零遮蔽前提）；`resizable.tsx` 的 `export { usePanelRef } from 'react-resizable-panels'` 作为外来再导出计入具名面 | 规模迭代（39 模块 / 242 名） |
| 5 个已知零引用模块在面内 | 边界 | `button-group` / `context-menu` / `input-group` / `marker` / `sheet` 五条 specifier 均在 39 条内（MUST NOT 在本变更内删除；清理属独立变更，R-2） | 逐值迭代（5 值） |
| specifier 形态 | 边界 | 每条满足 `^\./[a-z0-9][a-z0-9-]*$`：零 `../`、零 `./<子目录>/`、零 `@/`、零扩展名（`./badge.tsx`）、零自指（`./index`）、零大写（`./Badge`） | 逐值迭代（6 类非法样本） |
| 抽取器判红面自证 | 边界 | 合成两份各含 `export { A }` 的源文本 ⇒ 抽取器报 1 个重名；合成跨行块 `export {\n A,\n B,\n};` ⇒ 抽取器给出 2 名（证明能识别块形态、判据不是恒真） | 双夹具对拍 |
| 行数下界 | 边界 | 空源 ⇒ 0 条；单条 `export * from './alert';` ⇒ 1 条（与 39 条规模用例互证抽取器两端） | 逐值迭代（2 值） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `node:fs`（`readdirSync` / `readFileSync`） | **不 mock**：真实读目录与 39 个模块源文本 | 一一对应 / 零重名 / 5 个零引用模块用例 |
| 39 个 ui 模块及其依赖（`react` / `@base-ui/react` / `react-resizable-panels` / `cn` …） | **不 import、不 mock**：本文件零运行期装载 ⇒ 无需替身，也不会把 39 个组件装进 jsdom（design 注「纯源文本读取，无运行期 import」） | 全部用例 |
| `process.cwd()` | **不 mock**：vitest 以包根 `packages/desktop` 为 cwd，路径以 `resolve(process.cwd(), 'src/components/ui/…')` 拼接（沿 `index.test.tsx:199` 范式） | 全部用例 |
| 进程边界（网络 / DB / 子进程 / 环境变量 / IPC） | 不存在：仅用 `node:fs` 读源文本，零网络 / DB / 子进程 / IPC；运行环境沿用仓默认 `jsdom`（**不写** per-file 覆盖指令——`setupFiles` 读 `Element.prototype`，`node` 环境会在加载期抛错），本文件零 DOM 调用 | — |

---

### packages/desktop/src/routes.tsx -> packages/desktop/src/app.test.tsx

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更：「公共函数 / API」表无本文件行（`AppRoutes({ root })` 的签名与路由表内容——路径 / 参数 / 重定向——逐字不变）；本变更对该文件只改 import 形态（`:3–13` 的 11 条文件级 → 8 条目录级）。按纪律不 grep 源文件导出声明、不虚构条目。 -->

> **待测功能条目 0 条**（源文件在 design「公共函数 / API」表零行）；本节只登记「零新增用例 + 理由」与既有回归承接面。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 零新增用例（import 形态收窄 11 → 8） | 边界 | 不新增 `it`：11 → 8 条目录级 import 与「12 个 barrel 覆盖文件零残留文件级跨元素 import」均为 `grep` 计数面（见 `## 不可测试项` 4），不构成运行期行为；新增用例即偏离 design 事实校准 9 / D9⑦「零测试改动」 | 单例 |
| 既有 `app.test.tsx` 回归面 | 正向 | 既有用例全绿即证明路由表零变化：挂载 `App` 走完整路由表、断言路由切换与重定向；8 条目录 specifier 只为既有实现模块换一条解析路径（`<dir>/index.ts` 折叠） | 单例 |
| 目标元素等价 | 异常 | 若目录 specifier 少取 / 取错名（如 `./views/changes` 漏 `ChangeListView`）⇒ 既有用例在挂载或点击期即红；收窄前后目标元素逐条相等由全量 `relations` diff 承接（`## 不可测试项` 3），不另设断言 | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/app` / `@tauri-apps/api/core` / `@tauri-apps/plugin-dialog` / `@tauri-apps/plugin-updater` / `react-router`（既有 `app.test.tsx:23–31` 的五处替身） | **零改动、零新增**：既有 `vi.mock` 逐字保留（本变更零测试改动） | 既有用例全绿 |
| 8 个视图目录的 barrel 及其实现模块 | **不 mock**：路由挂载需真实组件树；替身掉 barrel 会让「入口被真用」这一事实失去观测点 | 同上 |
| 进程边界（IPC） | 由既有 `invokeMock`（`@tauri-apps/api/core`）承接，本变更零新增 | 全部用例 |

### packages/desktop/src/views/changes/change-detail-view.tsx -> packages/desktop/src/views/changes/change-detail-view.test.tsx

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更：「公共函数 / API」表无本文件行（`ChangeDetailView` 的签名与 props / 行为零变化）；本变更对该文件只改 import 形态（`./flow/*` 7 条 + `./agent/*` 1 条 → `./flow` + `./agent` 共 2 条；`:18` 的 `../architecture/components/architecture-canvas` 逐字不动，D6 留白）。按纪律不 grep 源文件导出声明、不虚构条目。 -->

> **待测功能条目 0 条**；本节只登记「零新增用例 + 理由」、既有回归承接面与 T5 的 mock 边界。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 零新增用例（import 收窄 8 → 2） | 边界 | 不新增 `it`：8 → 2 条（`./flow` 为单条跨行、三类型走 inline `type`）属 `grep` 文本面（`## 不可测试项` 4）；`consistent-type-imports` / `no-duplicate-imports` 的写法规约由 lint 承接 | 单例 |
| 既有 `change-detail-view.test.tsx` 回归面 | 正向 | 既有用例（详情页挂载 + 抽屉探针 + `mountMaterials` 的 spy 计数）全绿即证明 props / 渲染 / 行为零变化 | 单例 |
| T5 · mock 经 barrel 的可见性 | 异常 | 若 `vi.mock(import('./flow/attachments'), { spy: true })`（`:46`）与 `vi.mock('@/views/changes/flow/detail-drawer', …)`（`:72`）不再命中被 barrel 中转的模块 ⇒ spy 计数与抽屉替身断言即红；处置只允许改 mock 目标或保持既有直引，**MUST NOT** 回退产品侧 import（细则 `## 不可测试项` 9） | 单例 |
| 目标元素等价 | 异常 | `./flow` 折叠到 `views/changes/flow/index.ts` 后 owner 仍 `Views.Changes.Flow`、`./agent` 仍 `Views.Changes.Agent`；两处 `architecture-canvas` 深路径 import 不在判红面（D6）；边面判据归全量 `relations` diff（`## 不可测试项` 3） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `vi.mock(import('./flow/attachments'), { spy: true })`（`:46`） | **不改 mock、不新增**：目标仍是实现文件的 module id；barrel 的 `export { mountMaterials } from './attachments'` 指向同一 id，是否穿透由既有用例实跑判定（T5）。实证不穿透时才允许把 mock 目标改指 barrel，产品侧 import MUST NOT 退回文件级 | `mountMaterials` spy 计数用例 |
| `vi.mock('@/views/changes/flow/detail-drawer', …)`（`:72`） | 同上（同一 module id 替换语义） | 抽屉替身用例 |
| `@tauri-apps/api/core`（`invokeMock` / `ChannelMock`，`:41`） | **零改动、零新增** | 全部用例 |
| `./flow` / `./agent` 目录 specifier 的折叠解析 | **不 mock**：折叠由产品侧 `scan::ts::TsResolver` 与构建期 resolver 各自承担，测试相位不替身解析器 | — |

### packages/desktop/src/views/changes/flow/detail-drawer.tsx -> packages/desktop/src/views/changes/flow/detail-drawer.test.tsx

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更：「公共函数 / API」表无本文件行（`DetailDrawer` 的签名与 props 面零变化）；本变更对该文件只改一条 import 的 specifier（`:7` `'../renderers/artifact-tabs'` → `'../renderers'`，具名 `ArtifactTabs` 不变）。按纪律不 grep 源文件导出声明、不虚构条目。 -->

> **待测功能条目 0 条**；本节只登记「零新增用例 + 理由」与既有回归承接面。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 零新增用例（specifier 1 → 1） | 边界 | 不新增 `it`：`:7` 的 import 条数 1 → 1、具名不变，只换 specifier 形态，属 `grep` 文本面（`## 不可测试项` 4） | 单例 |
| 既有 `detail-drawer.test.tsx` 回归面 | 正向 | 既有用例（抽屉挂载 + `vi.mock('./session-transcript-panel', …)` 探针替身）全绿即证明 `ArtifactTabs` 经 barrel 取用后渲染面零变化 | 单例 |
| R-4 · 目录 specifier 折叠 | 异常 | 若 `'../renderers'` 未折叠到 `views/changes/renderers/index.ts` ⇒ 既有用例在模块解析期即红；若解析成功而 owner 漂移 ⇒ 全量 `relations` 丢边（71 → 69，`## 不可测试项` 3）。测试相位只回报既有用例结果，不新增断言 | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `vi.mock('./session-transcript-panel', …)`（`:70`） | **不改 mock、不新增**：目标是与本文件同目录的实现文件，与 barrel 无耦合 | 既有用例 |
| `@tauri-apps/api/core`（`:59` 的 `invokeMock` / `ChannelMock`） | **零改动、零新增** | 全部用例 |
| `'../renderers'` 的折叠解析 | **不 mock**：折叠由构建期 resolver 与产品侧解析面承担 | — |

### packages/desktop/src/views/explores/components/explore-preview.tsx -> packages/desktop/src/views/explores/components/explore-preview.test.tsx

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更：「公共函数 / API」表无本文件行（`ExplorePreview({ doc, loading, onRefresh })` 零变化）；本变更对该文件只改一条 import 的 specifier（`:4` `'../../changes/renderers/markdown-doc-renderer'` → `'../../changes/renderers'`，具名 `MarkdownDocRenderer` 不变）。按纪律不 grep 源文件导出声明、不虚构条目。 -->

> **待测功能条目 0 条**；本节只登记「零新增用例 + 理由」与既有回归承接面。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 零新增用例（specifier 1 → 1） | 边界 | 不新增 `it`：`:4` 的 import 条数 1 → 1、具名不变，属 `grep` 文本面（`## 不可测试项` 4） | 单例 |
| 既有 `explore-preview.test.tsx` 回归面 | 正向 | 既有用例（文档渲染 + 刷新回调）全绿即证明 `MarkdownDocRenderer` 经 barrel 取用后渲染面零变化 | 单例 |
| 跨元素边等价 | 异常 | 若 `'../../changes/renderers'` 折叠失败或指错元素 ⇒ 既有用例红，或全量 `relations` 丢 `Views.Explores → Views.Changes.Renderers` 边（`## 不可测试项` 3） | 单例 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `vi.mock(import('react-markdown'), …)`（`:15`） | **不改 mock、不新增**：三方库替身，与 barrel 无耦合 | 既有用例 |
| `'../../changes/renderers'` 的折叠解析 | **不 mock**：折叠由构建期 resolver 与产品侧解析面承担 | — |
| 进程边界（网络 / IPC） | 不存在：既有用例已用三方替身，本变更零新增 | — |

---

## 不可测试项

1. **`.dev-team/architecture/models/06-frontend.toml`（`Components.Agent` 的 `kind` / `path` 翻转 + 表头 `:2` 改数）** —— 判定：**模型数据资产，无专属测试载体**。原因：① 仓内无 `*_test.toml` 形态，也没有任何前端用例以「读本仓模型并断言其内容」为职责；② 该文件的「正确性」由 `archi_graph` 读数与 `grep` 计数定义（AC-1 / AC-6），而基线读数按 `desktop-archi-api-check` 纪律**不写进断言**（易随元素增减漂移）。**既有间接回归锚**（登记事实，不新增断言）：`packages/desktop/src-tauri/crates/core/archi/tests/pilot_test.rs` 的 `投影三路径与单点派生逐字相等` 走 `api::api_ref(&repository_root(), FQN)` → `elements_of(root).unwrap_or_default()` → `dto::graph_view(root)`，而 `elements_of` 对任何 `diagnostics` 非空直接 `Err`（`src/api/mod.rs:924-933`）——模型若被改出非法字段 / 断链 / 祖先缺失，该用例的 `status == Declared` 断言即红。承接通道：AC-1 / AC-6 的 `grep -c '^\[element\.' == 30`、`^\[\[relation\]\] == 71`、`^entry == 3`、`api = "none" == 2`、`kind = "domain"` 合计 8，以及全量 `archi_graph`（见 3）。

2. **`.dev-team/architecture/decisions/0001-model-granularity.md`（`§2` / `§3` / `§9` 改写）** —— 判定：**裁定记录文本，无断言载体**。原因：三处计数同值（`06` 表头 / spec 计数表 / ADR `§9`）是**审核纪律**，为它写「读 markdown 并断言含某句」的测试会把文档措辞钉进套件（文档本来就会随叙述重写而变）。承接通道：AC-6 / AC-7 的静态核对——`grep` 零命中 `35 个文件` / `13 个` / `另有 4 个容器` / `容器 10 + 叶 43`，且 AC-7 的 `find` 循环输出写死四条（见 4）。

3. **全量读数：`entry_state` 44 / 0 / 0、前端 `relations` 71 条 / 24 源三列 diff 空、`apiFindings` `0 + 41 = 41`、`status = clean` / `passed = true`、`diagnostics` 与 `warnings` 空** —— 判定：**属移交面（design D7 / 事实校准 10），无单元测试载体**。原因：取数通道是门禁 runner / IPC，测试相位与实现相位都不持有；把它们写进断言即等于把仓库基线钉进套件（既有明文禁例）。承接通道：tasks 移交清单 **T3**（test-execution 回填 `reports/module-entry-evidence.md` §1–§3）。**红线**：MUST NOT 手写伪读数、MUST NOT 用 `incremental-*.json` 顶替全量、MUST NOT 因读数不符而改模型或 barrel 名单去凑。

4. **4 个调用点文件的形态判据（21 → 12）与 AC-4 / AC-7 的静态面** —— 判定：**无新增用例面，靠静态 `grep` / `find` 核对**。原因：① design D9⑦ 明令零测试改动，四个文件的既有测试是行为回归承接面而非形态判据载体；② 形态判据（`routes.tsx` 11 → 8、`change-detail-view.tsx` 8 → 2、`detail-drawer.tsx` 1 → 1、`explore-preview.tsx` 1 → 1、12 个目录零残留深路径 import、`@/components/ui/<mod>` 两组计数前后同值、AC-7 的 `find` 循环）本身是文本计数，与产品行为无关。承接通道：tasks 阶段零 / 阶段二 / 阶段四的自查 `grep` 与 `find`（实现面）+ AC-7 的命令原文；边面等价由 3 的全量 `relations` diff 覆盖。**边界**：`views/architecture/components/architecture-canvas` 的两处深路径 import（`change-detail-view.tsx:18` / `flow/run-control-panel.tsx:24`）**不在判红面**（design D6：登记为已知留白，其两条模型边已在案）—— 若评估相位要求「所有跨元素 import 都经入口」，正确处置是改能力（把 `ArchitectureCanvas` 加进 `views/architecture` 公共面）而非改这两行 import。**若评估相位要求形态判据机械化**：落点是给 `module-entries.test.ts` 增第 6 条判据（扩展 T1），MUST NOT 反向修改产品侧 import 形态去迁就测试。

5. **生成物与门禁面：`api:check` 三列空、`INDEX.toml` 零 diff、`api:export` 二跑零写入、`bindings:export` 幂等、`vp check`（`knip` + typeCheck）全绿及其反向判据** —— 判定：**无单元测试载体**。原因：① 这些是 CLI / 门禁事实，测试相位不重复执行（本相位纪律：静态脚本由桌面端执行）；② `knip` 的反向实验（给某 barrel 多导一名 ⇒ 报 unused；从 `flow/index.ts` 删 `runStepNodes` ⇒ `change-detail-view.tsx` 编译红）**MUST NOT 留在工作树**。承接通道：tasks **T2**（static-check）/ **T4**（绑定与生成物幂等）。本稿只在第 1–12 节把「名单逐字」做成源文本断言——它是 `knip` 判据的**前置对齐面**，不替代 `knip`。

6. **零 diff 面：`packages/desktop/src-tauri/**`、`packages/desktop/src/components/ui/**`（除新增 `index.ts`）、`.dev-team/architecture/api/**`、`packages/desktop/src/types/generated/**`、`openspec/specs/**`、`knip.json`、`vite.config.ts`、`tsconfig.json`、12 个目录内 39 + 40 个既有实现文件** —— 判定：**无断言载体，靠 `git diff` 核对**。原因：为「零 diff」写测试需读取本仓真实路径并与基线比对，正是「基线不写进断言」禁止的形态。承接通道：AC-8（`git status --porcelain` ⊆ 白名单）/ AC-10（`git diff` 逐目录零输出），实现面 + static-check 执行。

7. **版本面：`packages/desktop/package.json`（`0.6.49` → `0.6.50`）、`packages/desktop/src-tauri/tauri.conf.json`（零内容改动）、`packages/desktop/src-tauri/Cargo.toml`（`0.1.0` 零改动）** —— 判定：**单值配置，无行为可测**。原因：① 为版本纪律写断言会把版本号耦进套件，并使后续每个 change 都要改测试；② `tauri.conf.json` 的判据是「引用形态未被写成字面版本号」，属文本事实。承接通道：AC-9 的 `grep -n '"version"'` 与 `git diff` 零输出核对（static-check）。

8. **`openspec/changes/frontend-barrel-entry-migration/reports/module-entry-evidence.md`（新增留痕）** —— 判定：**不是被测源文件，而是 AC 的实测载体**。原因：其正确性由 T3 读数与 `grep` 实测定义（元素级 / 全量两种口径，§0 必须声明；ui 两组计数口径并列），写进断言即与「基线不写进断言」冲突。承接通道：tasks 阶段零 / 阶段四的填写项 + design D7 的四节结构（§1 入口面 / §2 关系面 / §3 判据面 / §4 零扩散与版本），每节须标 `实现面` / `移交面`。

9. **T5 · `vi.mock` 与 barrel 的相互作用复核（执行期动作，非新用例）** —— 判定：**不设计新用例，但必须实测**。原因：`change-detail-view.test.tsx:46` 的 `vi.mock(import('./flow/attachments'), { spy: true })` 与 `:72` 的 `vi.mock('@/views/changes/flow/detail-drawer', …)` 按 **module id** 替换实现文件；产品侧改走 `./flow` 后，barrel 的 `export { mountMaterials } from './attachments'` / `export { DetailDrawer } from './detail-drawer'` 是否读到被替身化的模块（ESM live binding 经 `export … from` 中转），只能由既有用例实跑判定。承接通道：该测试文件的既有用例全绿（由桌面端 test-execution 执行；零改动即判据）；失败时按 tasks T5 处置（改 mock 目标或保持直引），**MUST NOT** 回退产品侧 import。

10. **5 个零引用 ui 模块与 `export *` 的语义等价性** —— 判定：**已知缺口，不设断言**（design R-1 / R-2）。原因：`button-group` / `context-menu` / `input-group` / `marker` / `sheet` 随 `export *` 进入公共面，「是否应进入」无机械判据；**语义等价性**（模块新增 / 删除是否与入口条目保持意图一致）同样不可断言——本稿第 12 节只断言「39 条 / 一一对应 / 5 个模块仍在面内 / 零重名」，MUST NOT 据此宣称「ui 公共面 = 真实使用面」。清理（删模块或改白名单）属独立变更。

11. **`packages/desktop/src/components/agent/index.tsx` 与其 `index.test.tsx`（本变更零改动）** —— 判定：**不在被测范围，只作回归锚**。原因：① 它不在 12 个 barrel 之列（design D9③：翻转只发生在模型与裁定记录）；② 其 6 条 re-export / 7 名 / `AskAnswerActions` 消费点由既有 `index.test.tsx` 自证，本变更零触碰；③ `App.Frontend.Components.Agent` 的入口由 ③ 命中该 `index.tsx` 属模型面事实（见 1）。承接通道：既有测试零改动、必须仍绿。

> **不做项（明确不登记为测试缺口）**：① 前端契约资产（`.md` / `.schema.json` / `index.i.ts`）与 `paths::ts_import_specifier` 接线 —— proposal 五 / design D9① 裁定不做，故不设计任何 `api-reexport` 契约用例，其「零实例」由 AC-8 的零 diff 面覆盖；② `api = none` 标注 —— 已裁不做（伪造「无公共 API」），不为它设反向用例；③ 5 个零引用 ui 模块的删除 / 132 处 ui 调用点改写 —— 独立变更，本稿只断言「不删不改」的现状（第 12 节 5 值用例 + 不可测试项 4 的同值计数）；④ `ArchitectureCanvas` 进入 `views/architecture` 公共面 —— design 待决问题 1 明写「两条都属改能力，MUST NOT 在本变更内顺手做」，故第 3 节只断言「不外露」不设计正向用例。
