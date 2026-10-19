# 设计: desktop-explore-list-archive

> **变更**: desktop-explore-list-archive
> **日期**: 2026-10-19
> **基于**: proposal.md、explore.md（一行诉求）、specs/desktop-explore-page/spec.md（ADDED 一条 requirement + Module Contract 两行）

---

## 提案与规格同步状态

`proposal.md` 与 `specs/desktop-explore-page/spec.md` 已由提案阶段定稿，本设计只覆盖 proposal「变更范围 - 实现文件」两文件，并把提案未定形的四处落点收敛：**分区容器 DOM 形态**（`explore-items` 由 `<ul>` 改 `<div>` 后条目如何不脱离列表上下文，D3）、**组标题层级与计数形态**（照 `change-list-view` 的「进行中 (n)」但降一级，D4）、**分组派生落位**（渲染体内直调纯函数还是进 hook，D1）、**零记录与三态互斥的接线方式**（分组区条件渲染判据，D5）。

工作树实况（本变更起点，`packages/desktop` 0.5.17）：`views/explores/explore-view.tsx:82-91` 为单一 `<ul data-testid="explore-items">` + `list.records.map` 平铺，条目 `ExploreListItem` 为 `<li>`（title / 建档时间 / 删除入口，testid `explore-item` / `explore-item-title` / `explore-item-delete`）；三态块 `explore-list-error` / `explore-list-loading` / `explore-list-empty` 与 `<ul>` **平铺并列**（空态未互斥隐藏 `<ul>`——`<ul>` 恒渲染，空清单时为空节点）；`hooks/use-explore-list.ts` 直出 `list_explore_records` 结果、零排序，`recordsRoot === root` 之外返回空数组；后端 `list_explore_records` 契约为 `id` 降序（`id` 为可复用 max+1 计数）。`lib/` 下已有同族纯函数先例 `explore-title.ts`（`extractMarkdownTitle`）/ `explore-stance.ts`（`buildExplorePrompt`）/ `explore-workflow-type.ts`（`extractWorkflowType`），均零 IO、零 React、`import type` 自 `../types/dto`。本设计描述**本变更完成后的目标形态**：`groupExploreRecords` 纯函数 + 清单页两段分区呈现；后端、hook、详情页、新建对话框、bindings 全部零改动。

---

## 关键设计决策

| # | 问题 | 定稿 | 理由 | 备选方案 |
|---|------|------|------|----------|
| D1 | 分组派生落位 | `ExploreListView` 渲染体内直调 `const groups = groupExploreRecords(list.records);`——不进 `useExploreList`、不包 `useMemo` | 纯函数 O(n log n) 且清单规模小，渲染期派生无副作用（`explore-view.tsx` 的 id 桥三级派生同在渲染体，先例在场）；`ExploreListState` 形状零改（spec「零改动面」明列）；`list.records` 引用每次取数才变，`useMemo` 依赖与调用成本同量级，收益为零 | 派生进 hook（污染取数面，spec 明令）；`useMemo` 缓存（多一层记忆化、无收益） |
| D2 | 纯函数实现形态与比较器 | `export function groupExploreRecords(records: ExploreRecord[]): { active: ExploreRecord[]; promoted: ExploreRecord[] }`——两次 `filter`（`promotedTo === null` / `!== null`）各自返回**新数组**后原地 `sort`；比较器 `(a, b) => b.createdAt - a.createdAt \|\| b.id - a.id` | `filter` 产物即拷贝，`sort` 只动新数组 → 入参数组零触碰且无需显式 `[...records]`（AC-3 天然成立，实现面可读）；判别用 `=== null` 而非 truthiness（`promotedTo` 类型即 `string | null`，判空即判组）；`b - a` 对 `createdAt` / `id` 皆为 number，零 `Date` 解析 | `records.reduce` 单趟分区（等价但多一个可选累加器形态，可读性差）；`[...records].sort` 后 `filter`（一次多余拷贝） |
| D3 | `explore-items` 的 DOM 形态 | `explore-items` 容器（`<div>`，testid 与全局唯一性不变）内两个 `<section>`，每组内仍是 `<ul>` + `<li>` 条目——**`ExploreListItem` 的 `<li>` 根零改** | proposal 要求 `ExploreListItem` 呈现面零改；`<li>` 必须挂在列表上下文（直接置于 `<div>` 下为非法 HTML 且伤无障碍语义），故每组各带一个 `<ul>`——分组容器与条目容器各守其职 | 条目根改 `<div>`（触碰 `ExploreListItem`，违背 proposal 零改约束）；去掉 `<ul>`/`<li>` 只用 `<div>`（既有列表语义与样式 `list-none` 上下文丢失） |
| D4 | 组标题与空组占位形态 | 组标题 `<h3>`：组名 + muted `<span>({n})</span>`（视觉与文案照 `change-list-view.tsx:72-83` 的 `进行中 (n)`，层级降一级因外层已有「探索清单」`<h2>`）；空组渲染 `<div className="text-muted-foreground">` 占位文案；**不新增标题 / 空态 testid** | 组 testid 已由 spec 定为 `explore-group-active` / `explore-group-promoted`，计数与占位文案经组容器 `textContent` 断言即可，避免自造新的 testid 面（AC-4 / AC-5 的断言口径已足够）；标题层级跟随既有 `<h2>探索清单</h2>` 的嵌套关系 | 标题复用 `<h2>`（与卡片标题同级，视觉与语义层级混乱）；空态给独立 testid（新增面，spec 未要求） |
| D5 | 零记录与三态互斥接线 | 分组区以 `{list.records.length > 0 && (…)}` 条件渲染——**只保留「空清单不渲染分组区」一条新判据**；三态块（error / loading / empty）文本、顺序、条件逐字零改 | 既有三态互斥语义（AC-8）零改；`useExploreList` 在首次取数与跨根过渡期间返回空数组 → 加载中 / 过渡轮天然不渲染分组区（不抢跑、不呈现旧根记录）；`explore-items` 由「恒在场空 `<ul>`」变为「有记录才在场」，现有用例只在该元素有记录时 `findByTestId`，语义收窄无冲突 | 保留恒渲染的空分组区（AC-5 明令零记录时两组 testid 均不在场）；新增第四条互斥状态（与既有三态语义重叠） |
| D6 | 版本号 bump 归属 | `packages/desktop/package.json` `version` 0.5.17 → 0.5.18，收在任务末阶段单行 | proposal「变更范围」未列此文件，但既有 desktop 变更惯例均在变更内 bump（`0.5.10→0.5.11`、`0.5.16→0.5.17` 等先例）；`src-tauri/Cargo.toml` 不随动 | 待归档提交统一 bump（与既有历史不一致） |

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 分组与排序纯函数（新增） | `groupExploreRecords`：按 `promotedTo` 二分 + 组内 `createdAt` 倒序（同刻 `id` 倒序）；入参只读、零 IO、零 invoke、零 React | `packages/desktop/src/lib/explore-groups.ts`（新） | `type ExploreRecord`（`../types/dto`；与 `lib/explore-title.ts` / `lib/explore-stance.ts` 同族） | TypeScript 纯函数 |
| 清单分区呈现（修改） | `ExploreListView` 派生两组并分区渲染：`explore-items`（唯一容器）内两个 `<section>`（`explore-group-active` / `explore-group-promoted`），组标题携组名与计数，空组渲染占位文案，零记录不渲染分组区；私有 `ExploreGroup` 承担单组渲染（标题 + 条目列表 / 占位），复用未改的 `ExploreListItem` | `packages/desktop/src/views/explores/explore-view.tsx` | react、react-router、`../../types/dto`、`../../lib/explore-groups`、`./hooks/use-explore-list`、`./explore-detail-view`、`./components/explore-create-dialog` | TypeScript、React（TSX） |
| 清单取数与动作面（零改动，被依赖） | `useExploreList` 继续直出后端记录（`id` 降序原样保留）、动作薄封装与 root 归属标记不变；分组不在 hook 内派生 | `packages/desktop/src/views/explores/hooks/use-explore-list.ts` | bindings、react | TypeScript、React hooks |
| 后端清单排序契约（零改动，被依赖） | `list_explore_records` 维持 `id` 降序，不承接时间排序（排序纯在前端） | `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | native_db | Rust |
| 详情与新建面（零改动，复用） | promoted 态徽标 / 「查看变更」跳转、建档、预览、id 桥派生、`onCreated` 导航、workspace 切换 `resetPending` 全部原样；分区后条目点击仍进既有详情 | `views/explores/explore-detail-view.tsx`、`views/explores/components/*` | — | TypeScript、React |

---

## 变更清单

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/src/lib/explore-groups.ts` | 纯函数模块：模块 doc（分组判据取 `promotedTo`、排序键取 `createdAt` 的理由与「不读后端返回顺序」的口径、入参只读与零 invoke 纪律）+ `import type { ExploreRecord } from '../types/dto';` + `export function groupExploreRecords`（见「公共函数 / API」表）；不引入其它导出、不新增导出类型 |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src/views/explores/explore-view.tsx` | ① 增 `import { groupExploreRecords } from '../../lib/explore-groups';`；② 新增**模块私有**组件 `ExploreGroup({ groupTestId, label, records, emptyText, onOpen, onRemove })`——渲染 `<section data-testid={groupTestId}>` + `<h3>`（`{label} <span className="text-muted-foreground">({records.length})</span>`）+ 有记录时 `<ul>` 内 `ExploreListItem` 列表、无记录时 `<div className="text-muted-foreground">{emptyText}</div>`；③ `ExploreListView` 渲染体增 `const groups = groupExploreRecords(list.records);`；④ 原 `<ul className="m-0 list-none p-0" data-testid="explore-items">` 段替换为 `{list.records.length > 0 && (<div data-testid="explore-items">…</div>)}`，容器内顺序固定为两个 `ExploreGroup`——前组 `explore-group-active` / label「进行中」/ `emptyText`「无进行中的探索。」、后组 `explore-group-promoted` / label「已转变更」/ `emptyText`「暂无已转变更。」；⑤ 三态块（`explore-list-error` / `explore-list-loading` / `explore-list-empty`）逐字零改；⑥ `ExploreListItem` 组件本体、`ExploreView` 主体（路由参数派生 / id 桥三级链 / `resetPending` / `onCreated` 导航）逐字零改 | 分区呈现在清单页单点落地；`ExploreListItem` 的 `<li>` 根与全部 testid 零改（D3）；条目全局顺序 = 进行中组在前、已转变更组在后（AC-4） |
| `packages/desktop/package.json` | `version` 0.5.17 → 0.5.18 | 版本纪律（D6）；`tauri.conf.json` 经 `../package.json` 自动跟随，`src-tauri/Cargo.toml` 不随动 |

### 删除文件

<!-- 不涉及：本变更零删除——既有 `<ul>` 容器就地改为分组外层 `<div>`，条目组件与三态块全部保留。 -->

### 公共函数 / API

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|-----------|------|------|
| `groupExploreRecords` | `packages/desktop/src/lib/explore-groups.ts` | 新增 | `(records: ExploreRecord[]) => { active: ExploreRecord[]; promoted: ExploreRecord[] }` | 按 `promotedTo === null` / `!== null` 二分（互斥且完备）；两组各按 `createdAt` 降序、同刻 `id` 降序；返回新数组、入参数组与元素字段零触碰；零 IO、零 invoke、零 React 依赖 |
| `ExploreView` | `packages/desktop/src/views/explores/explore-view.tsx` | 修改（实现） | `(props: { root: string }) => React.JSX.Element` | 导出签名与路由接线不变；仅清单页分支（`ExploreListView`）内部改分区呈现，详情分支与派生链逐字不变 |

> 模块私有（不列，零改或新增但不属模块级导出面）：`ExploreListItem`（零改）、`ExploreListView`（内部改）、新增 `ExploreGroup`。

### 类型定义

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|--------|----------|-----------|------|
| `ExploreRecord` | `packages/desktop/src/types/generated/bindings.ts` | 不变 | 唯一消费面为既有字段 `id: number`（同刻兜底键）、`promotedTo: string \| null`（分组判据）、`createdAt: number`（排序键）；本变更零字段增删、零 bindings 重生成 |
| `ExploreListState` | `packages/desktop/src/views/explores/hooks/use-explore-list.ts` | 不变 | 分组结果不进 state（D1），形状与动作面零改 |

> 无新增导出类型：`groupExploreRecords` 返回类型以行内对象字面量标注（与 spec Module Contract 的写法一致），不引入仅一次性使用的具名 interface。

### 配置

| 配置键 | 所在文件 | 新增/修改 | 值类型 | 默认值 | 说明 |
|--------|----------|-----------|--------|--------|------|
| `version` | `packages/desktop/package.json` | 修改 | `string` | `0.5.18`（原 `0.5.17`） | 版本纪律（D6） |

> 无新增运行时配置键 / 环境变量 / feature flag：分组与排序规则是编译期常量语义，不参数化。

---

## 数据模型

持久化面**零改动**（`ExploreRecord` 表结构、`list_explore_records` 的 `id` 降序契约、bindings 均不变）。本变更只引入一层**渲染期派生视图模型**：

```
list.records（后端返回，id 降序，原样不重排）
└── groupExploreRecords(records)                      ← 纯函数，渲染期调用
      ├── active   = records.filter(promotedTo === null)   .sort(createdAt ↓, id ↓)
      └── promoted = records.filter(promotedTo !== null)   .sort(createdAt ↓, id ↓)

呈现序（explore-items 容器内条目全局序）
= active[0..n]            ← 「进行中」(n)
  ⊕ promoted[0..m]        ← 「已转变更」(m)
```

- **判据**：`promotedTo`（`ExploreRecord.promotedTo: string | null`）——`null` = 进行中（含 `draft-*` 未定名占位记录），非空 = 已转变更。打标即事实：不查 change 记录、不校验目标 change 是否仍存在（proposal 已接受）。
- **排序键**：`createdAt`（写入时 stamp 的建档毫秒，行内展示的同一字段）；同刻以 `id` 倒序兜底（`id` 唯一 → 全序 → 输出确定）。`updatedAt` 不参与（title / workflow_type 回填与 promote 打标会刷新它，取它会导致「看着没动的时间跳位置」）。
- **关系**：`active ∩ promoted = ∅` 且 `active ∪ promoted = records`（二分完备，条目不丢不重——AC-1）；一条记录恰属一组。
- **不变量**：入参数组顺序与元素字段调用前后完全相同（`filter` 产出新数组后才 `sort`，不 in-place 触碰入参——AC-3）；同输入两次调用输出序列一致（无时间源、无随机、无 IO）。
- **持久化**：无。分组结果是 React 渲染期临时值，不写库、不进 URL、不进 `ExploreListState`（刷新即按最新记录重算）。

---

## 路由 / API 设计

零新增 / 零改动——本变更不新增 HTTP 路由、不新增 Tauri IPC 命令、不重生成 bindings。既有面保持：

| 面 | 说明 |
|----|------|
| 路由 `/explores` | 清单页（分区呈现的落点）；`explore-items` 容器内条目顺序 = 组序拼接 |
| 路由 `/explores/:name` | 详情页；两组的条目点击都走既有 `onOpen` → `navigate(/explores/<name>)`，已转变更组条目落既有 promoted 态（`explore-promoted-badge` + `explore-promoted-jump` → `/changes/<promotedTo>`） |
| 路由 `/changes/:id` | 仅由详情页「查看变更」跳转；清单内**不新增**直达 change 的入口（AC-6） |
| IPC `list_explore_records` / `promote_explore` / `delete_explore_record` | 签名与语义零改；promote 成功后既有 `list.refresh()` 触发重取 → 记录 `promotedTo` 非空 → 重算分组即自然移组（AC-7，前端零新增逻辑） |

---

## 依赖

运行时依赖：**无新增**（纯函数与分区渲染只用既有 `react` / `react-router` / `types/dto` 类型）。

构建 / 测试依赖：**无新增**（沿用 `vp check` / `knip` / `vitest` / `cargo test`；本变更零 Rust 改动，`cargo test -p store` 既有 `id` 降序断言原样通过）。

| 依赖 | 变更 | 用途 |
|------|------|------|
| `react` / `react-router` | 不变 | 清单页既有渲染与导航接线 |
| `../types/dto`（`ExploreRecord` 类型 re-export） | 不变 | 纯函数入参类型（`import type`，零运行时耦合） |
| `vite-plus/test`（测试面，测试相位承接） | 不变 | 纯函数直测与视图用例 |

---

## 验收标准落点

| ID | 设计落点 |
|----|----------|
| AC-1 | D2 的两次 `filter` 二分 + 完备性不变量（`active ∪ promoted = records`，无丢漏无重复） |
| AC-2 | D2 比较器 `b.createdAt - a.createdAt \|\| b.id - a.id`；分组不读后端返回顺序（`filter` 后全量 `sort`） |
| AC-3 | D2：`filter` 新数组上 `sort`，入参数组零触碰（AC 由测试相位断言，设计面已保证实现形态） |
| AC-4 | 修改文件表 ③④：两 `<section>` 恒序（active 前、promoted 后）+ 标题计数（`records.length`）+ `explore-items` 内条目全局序 = 组序拼接 |
| AC-5 | D4（空组占位文案）+ D5（零记录 `explore-list-empty` 在场、分组区不渲染 → 两组 testid 不在场） |
| AC-6 | 路由表：条目点击走既有 `onOpen`，已转变更组条目落既有 promoted 态详情；清单内零新增 change 入口（`ExploreListItem` 零改即保证） |
| AC-7 | 路由 / API 表末行：promote 成功 → 既有 `list.refresh()` 重取 → 重算分组移组；`explore-detail-view.tsx` 零改 → 零导航、详情不卸载 |
| AC-8 | D3（`ExploreListItem` 与 testid 零改）+ D5（三态互斥零改）+ 修改文件表 ⑥（派生链 / `resetPending` / `onCreated` 零改） |
| AC-9 | 零改动面：`src-tauri/**` 零文件、零命令、零 bindings 重生成（架构组件表「清单取数与动作面」「后端清单排序契约」两行标注被依赖且零改） |

---

## 待决问题

1. **排序口径的可换性（proposal 风险 1）**：本设计固定 `createdAt`。若产品后续要「最近活动优先」，改动面为 `groupExploreRecords` 比较器首个键（`updatedAt`）一处，分组与呈现零改；本期不预留开关参数（避免为未定需求加配置面）。
2. **「已转变更」组无限增长**（proposal 风险 4）：本期不做折叠 / 分页 / 归档（诉求未含，store 无归档语义）；组计数是唯一规模提示。若后续要折叠，落点为 `ExploreGroup` 的标题行（现值域内不做）。
3. **占位记录（`draft-*`）归「进行中」组**（proposal 风险 5，已接受）：与详情页「分析中」临时标题同窗，不另设第三组。
4. **目标 change 已删 / 已归档仍显示「已转变更」**（proposal 风险 6）：打标即事实，列表不查 change 记录（避免跨 crate 读面与额外 IPC）；详情页跳转失败面既有。
5. **版本 bump 归属（D6）**：proposal「变更范围」未列 `packages/desktop/package.json`，本设计按既有「变更内 bump」惯例纳入末阶段；若归档流程改为统一 bump，则该行可整条删除（零语义影响）。
