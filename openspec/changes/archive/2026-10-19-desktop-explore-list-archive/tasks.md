# 任务: desktop-explore-list-archive

> 测试文件的编写与执行由 test-design / test-gen / test-execution 阶段承接（`src/lib/explore-groups.test.ts` 新增；`src/views/explores/explore-view.test.tsx` 两处既有顺序断言按倒序更新 + 新增分组 describe），本列表只覆盖实现文件。
>
> 依赖序要点：分组纯函数（阶段一）是分区呈现（阶段二）的唯一前置——`explore-view.tsx` 的 import 与派生调用必须等 `groupExploreRecords` 签名定稿；版本 bump 与语义自查（阶段三）殿后。本变更零 Rust 改动、零 bindings 重生成、零新增 IPC 命令，故无代码生成类前置任务。

## 阶段一：分组与排序纯函数

- [x] 新增 `packages/desktop/src/lib/explore-groups.ts`：文件头 doc 注明四件事——① 分组判据取记录 `promotedTo`（`null` = 进行中，含 `draft-*` 占位记录；非空 = 已转变更）；② 排序键取 `createdAt`（行内展示的同一字段）而非 `updatedAt`（title / workflow_type 回填与 promote 打标会刷新它，取它会「看着没动的时间跳位置」）；③ 显式按 `createdAt` 排而不读后端返回顺序（后端契约为 `id` 降序，而 `id` 是可复用 max+1 计数——删记录后新建的探索可拿到更小 `id`，`id` 序与建档时间序错位）；④ 纯函数纪律（入参只读、零 IO、零 invoke、零 React 依赖；与 `lib/explore-title.ts` / `lib/explore-stance.ts` 同族）
- [x] 修改 `packages/desktop/src/lib/explore-groups.ts`：增 `import type { ExploreRecord } from '../types/dto';` 并实现唯一导出 `export function groupExploreRecords(records: ExploreRecord[]): { active: ExploreRecord[]; promoted: ExploreRecord[] }`——`records.filter((record) => record.promotedTo === null)` 与 `records.filter((record) => record.promotedTo !== null)` 各得**新数组**后分别 `sort((a, b) => b.createdAt - a.createdAt || b.id - a.id)`；判别用 `=== null` / `!== null`（不用 falsy 判断）；不在入参数组上 `sort`、不引入额外拷贝、不导出其它符号、不新增导出类型（返回类型行内标注，与 spec Module Contract 一致）

## 阶段二：清单分区呈现

- [x] 修改 `packages/desktop/src/views/explores/explore-view.tsx`：import 区增 `import { groupExploreRecords } from '../../lib/explore-groups';`（置于既有 `../../types/dto` 与本地相对 import 之间，随既有序）
- [x] 修改 `packages/desktop/src/views/explores/explore-view.tsx`：在 `ExploreListItem` 之后新增**模块私有**组件 `function ExploreGroup({ groupTestId, label, records, emptyText, onOpen, onRemove }: { groupTestId: string; label: string; records: ExploreRecord[]; emptyText: string; onOpen: (name: string) => void; onRemove: (name: string) => void }): React.JSX.Element`——根为 `<section className="mb-3 last:mb-0" data-testid={groupTestId}>`，内含 `<h3 className="m-0 mb-2 text-[13px]">{label} <span className="text-muted-foreground">({records.length})</span></h3>`（层级较外层「探索清单」`<h2>` 降一级，计数形态照 `change-list-view.tsx` 的「进行中 (n)」），`records.length === 0` 时渲染 `<div className="text-muted-foreground">{emptyText}</div>`，否则渲染 `<ul className="m-0 list-none p-0">` 内 `records.map` 的 `ExploreListItem`（`key={record.id}`；`onOpen` / `onRemove` 原样透传）
- [x] 修改 `packages/desktop/src/views/explores/explore-view.tsx`：`ExploreListView` 渲染体顶部增 `const groups = groupExploreRecords(list.records);`（不包 `useMemo`、不进 hook——渲染期纯派生），并把原 `<ul className="m-0 list-none p-0" data-testid="explore-items">…</ul>` 段整体替换为 `{list.records.length > 0 && (<div data-testid="explore-items">…</div>)}`——容器内**顺序固定**：先 `<ExploreGroup groupTestId="explore-group-active" label="进行中" emptyText="无进行中的探索。" records={groups.active} … />`、后 `<ExploreGroup groupTestId="explore-group-promoted" label="已转变更" emptyText="暂无已转变更。" records={groups.promoted} … />`（`onOpen` / `onRemove={list.remove}` 与既有接线同源）
- [x] 零改自查 `packages/desktop/src/views/explores/explore-view.tsx`：`ExploreListItem` 组件本体（`<li>` 根、`explore-item` / `explore-item-title` / `explore-item-delete` testid、`data-name`、建档时间展示、删除按钮文案）逐字未动；三态块 `explore-list-error` / `explore-list-loading` / `explore-list-empty` 的文本、条件与相对位置逐字未动；`ExploreView` 主体（`useParams` 派生、`lastRecordRef` id 桥三级链、`prevRoot` / `resetPending` 收敛、`openExplore` / `onCreated` 导航）逐字未动

## 阶段三：版本与语义自查

- [x] 修改 `packages/desktop/package.json`：`version` 0.5.17 → 0.5.18（`tauri.conf.json` 经 `../package.json` 自动跟随；`src-tauri/Cargo.toml` 不随动）
- [x] 语义自查（grep / diff 核对，非脚本执行）：清单页条目全局顺序恒为「进行中组 → 已转变更组」（两 `<ExploreGroup>` 字面量顺序确认）；组内排序不依赖 `list.records` 原序（`groupExploreRecords` 内对 `filter` 产物全量 `sort`）；`promotedTo` 判别仅 `=== null` / `!== null` 两处；未新增任何 `data-testid`（除 spec 已定的 `explore-group-active` / `explore-group-promoted`，且 `explore-items` 名字未变）；未新增 invoke 调用、未触碰 `list.refresh()` 之外的取数路径；`git status` 只含 `src/lib/explore-groups.ts`（新）、`src/views/explores/explore-view.tsx`、`package.json` 三项——`src-tauri/**`、`src/types/generated/bindings.ts`、`src/views/explores/hooks/use-explore-list.ts`、详情页与 `components/` 全部零改动
