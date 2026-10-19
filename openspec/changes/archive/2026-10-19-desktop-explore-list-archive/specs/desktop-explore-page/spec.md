# desktop-explore-page Specification (Delta)

## ADDED Requirements

### Requirement: 清单分组与时间倒序

探索清单 SHALL 以记录 `promoted_to` 为唯一分组判据呈现：`promoted_to` 为空归「进行中」组（草稿态探索，含 `draft-*` 未定名占位记录），非空归「已转变更」组（已 promote；记录与笔记的既有孤儿 / 保留语义不变，MUST NOT 因分组变化改动记录）。两组 SHALL 恒按「进行中在前、已转变更在后」呈现，组标题 SHALL 携组名与组内条目计数（照 change 清单「进行中 (n)」既有约定）；清单有记录时两组 `section` SHALL 恒渲染，组内为空 SHALL 呈现占位文案（「无进行中的探索。」/「暂无已转变更。」），MUST NOT 以组缺席表达空集。清单整体零记录时 SHALL 维持既有空态（`explore-list-empty`）且 MUST NOT 渲染分组区（既有 loading / error / empty 三态互斥语义零改）。

组内排序 SHALL 由前端纯函数（`lib/explore-groups.ts` 的 `groupExploreRecords`）确定性完成：按记录 `created_at` 倒序，`created_at` 相等时以 `id` 倒序兜底。排序 MUST NOT 依赖后端 `list_explore_records` 的返回顺序（其后端契约为 `id` 降序，而 `id` 为可复用计数——删记录后新建的探索可拿到更小 `id`，`id` 序会与建档时间序错位）。分组与排序 SHALL 纯读：不改动入参数组、不改动记录字段、不发任何 invoke、不引入轮询。

条目呈现 SHALL 沿用既有契约（`title`、建档时间、删除入口、点击进详情），容器 testid SHALL 保持 `explore-items`（全局唯一，条目顺序 = 组序拼接）且新增分组 testid `explore-group-active` / `explore-group-promoted`。已转变更组条目点击 SHALL 进入既有 promoted 态详情（「已转变更」徽标 + 「查看变更」跳转 `/changes/<promoted_to>`），MUST NOT 在清单内新增直达 change 的入口。promote 成功后经既有清单刷新，该记录 SHALL 自「进行中」组移入「已转变更」组，详情页停留语义 SHALL 零改（MUST NOT 因移组触发导航或卸载详情）。

#### Scenario: 按 promoted_to 分组与组序

- **WHEN** 当前 workspace 的清单含 3 条 `promoted_to = null` 与 2 条 `promoted_to` 非空的 `ExploreRecord`
- **THEN** 「进行中 (3)」组在前、「已转变更 (2)」组在后；两组条目互不串组、并集为清单全集；计数与实际条目数一致

#### Scenario: 组内按建档时间倒序

- **WHEN** 同一组内记录的 `created_at` 与 `id` 顺序相反（乱序 fixture，如 `created_at` 最新者 `id` 最小）
- **THEN** 条目按 `created_at` 倒序呈现（最新建档在前），`created_at` 完全相等时按 `id` 倒序；不按后端返回顺序、不按 `id` 序

#### Scenario: 分组纯读零副作用

- **WHEN** 以同一组记录连续调用两次 `groupExploreRecords`
- **THEN** 两次返回序列一致；入参数组顺序与元素字段在调用前后逐字段相同（不 in-place 排序）；全程无 invoke 调用

#### Scenario: 空组占位与整体空态

- **WHEN** 清单仅有进行中记录时打开 `/explores`；或清单零记录时打开 `/explores`
- **THEN** 前者「已转变更」组照常渲染标题与计数 0 及占位文案「暂无已转变更。」；后者仅呈现 `explore-list-empty` 空态，两个分组 testid 均不在场

#### Scenario: 已转变更条目进既有 promoted 详情

- **WHEN** 用户点击「已转变更」组中某条目
- **THEN** 导航至 `/explores/<name>` 并呈现既有 promoted 态（「已转变更」徽标 + 「查看变更」跳转 `/changes/<promoted_to>`）；清单内无直达 change 的入口

#### Scenario: promote 成功后移组

- **WHEN** 用户在详情页以「启动变更」promote 成功，清单经既有刷新重取且该记录 `promoted_to` 非空
- **THEN** 返回清单时该记录位于「已转变更」组、「进行中」组计数减一；promote 当下 URL 仍停留 `/explores/<name>`（零导航、详情未卸载）

## Module Contract

### Function

| 模块 | 关键契约 |
|------|----------|
| `packages/desktop/src/lib/explore-groups.ts`（新） | `groupExploreRecords(records: ExploreRecord[]) => { active: ExploreRecord[]; promoted: ExploreRecord[] }`：按 `promotedTo` 二分；组内 `createdAt` 倒序、同刻 `id` 倒序；拷贝后排序（入参只读）；零 IO、零 invoke、零 React 依赖（与 `lib/explore-title.ts` / `lib/explore-stance.ts` 同族纯函数） |

### Component

| 模块 | 关键契约 |
|------|----------|
| `packages/desktop/src/views/explores/explore-view.tsx`（改） | 清单数据区经 `groupExploreRecords` 派生两组后分区渲染：`explore-items`（全局唯一容器，由 `<ul>` 改为分组外层 `<div>`，条目仍 `explore-item`）内两个 `section`——`explore-group-active`（「进行中 (n)」）、`explore-group-promoted`（「已转变更 (n)」），组内空渲染占位文案；零记录维持 `explore-list-empty` 且不渲染分组区；`ExploreListItem` 呈现面（title / 建档时间 / 删除入口 / testid）与 `onCreated` 导航、id 桥派生、root 切换 resetPending 全部零改 |

### 零改动面

| 模块 | 说明 |
|------|------|
| `src/views/explores/hooks/use-explore-list.ts` | 取数与动作面零改（`ExploreListState` 形状不变，分组不在 hook 内派生） |
| `packages/desktop/src-tauri/**`（`store::list_explore_records` 及 `store_test`） | `id` 降序契约原样保留，本变更不下推排序；零新增命令、零 bindings 重生成 |
| `views/explores/explore-detail-view.tsx` / `components/`（composer、预览、新建对话框） | 既有 promoted 徽标 / 查看变更 / 建档 / 预览 / 会话呈现语义零改 |
| 既有 requirement（新建流程、双栏详情与实时预览、对话区呈现、会话链与 stance 前导、免命名定名与改名感知、workspace 切换回列表、title / workflow_type 回填、promote 入口） | 逐字保留，本变更只追加清单排列关注点 |
