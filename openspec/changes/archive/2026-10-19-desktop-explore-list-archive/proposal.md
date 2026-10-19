# 提案: desktop-explore-list-archive

> **变更**: desktop-explore-list-archive
> **日期**: 2026-10-19
> **状态**: 草稿

---

## 问题

`openspec/changes/desktop-explore-list-archive/explore.md` 一行诉求：「桌面端探索清单，区分进行中和已转变更，按时间倒序排列」。现状与该目标三处相悖：

1. **清单无状态区分，单条平铺**：`views/explores/explore-view.tsx:82-91` 是单一 `<ul data-testid="explore-items">`，`list.records.map` 直出全部记录，条目只渲染 `title` + 建档时间 + 删除入口；`promotedTo` 不参与呈现。要判断某条探索是否已转变更，只能点进详情页看详情头的「已转变更」徽标与「查看变更」跳转（`explore-detail-view.tsx:162-180`）——已转变更的探索与仍在进行中的草稿在列表页完全混排，随记录增长越来越难扫。

2. **顺序不是时间序，而是 `id` 序**：后端 `list_explore_records`（`crates/infra/store/src/store.rs:884-892`）仅 `sort_by_key(Reverse(record.id))`，前端 `hooks/use-explore-list.ts:150-197` 直出 `result`、零排序。而 `id` 是「幸存行 max+1」的**可复用计数**（见 desktop-workspace-store「删除级联清会话且 id 复用不错链」）——删掉较新记录后新建的探索会拿到比既有记录更小的 `id`，`id` 降序与建档时间序错位，「按时间倒序」在当前链条上不成立。

3. **时间字段只用于显示**：条目呈现的 `createdAt`（`explore-view.tsx:32`）与排序无关；`updatedAt` 被 title / workflow_type 回填、promote 打标刷新，但列表零消费。

## 提案

只改桌面前端呈现层（`packages/desktop/src`），Rust 侧与 store 排序契约零改动，三面：

1. **按 `promoted_to` 分组**：以记录 `promotedTo` 为唯一判别口径——`null` = 「进行中」（草稿态探索，含 `draft-*` 未定名的占位记录），非空 = 「已转变更」（已 promote）。两组恒按「进行中在前、已转变更在后」呈现，组标题携组名与条目计数（照 change 清单 `进行中 (n)` 既有约定，`change-list-view.tsx:72-83`）；清单有记录时两组恒渲染，组内为空呈现占位文案（「无进行中的探索。」/「暂无已转变更。」），不以组缺席表达空集；清单整体零记录时维持既有 `explore-list-empty` 空态且不渲染分组区（既有三态互斥语义零改）。

2. **组内时间倒序（前端确定性排序）**：新增纯函数 `groupExploreRecords`（`src/lib/explore-groups.ts`，与 `lib/explore-title.ts` / `lib/explore-stance.ts` 同族）——分组 + 各组按 `createdAt` 倒序、同刻以 `id` 倒序兜底。不读后端返回顺序（后端 `id` 降序契约原样保留、零改动），不改动入参数组与记录字段，不发任何 invoke。排序键取 `createdAt` 而非 `updatedAt`：行内显示的时间就是 `createdAt`，且 title / workflow_type 回填与 promote 打标不引起列表重排（避免「看着没动的时间跳位置」）。

3. **容器结构与既有 testid 兼容**：`explore-items` 仍是全局唯一容器（由 `<ul>` 改为分组外层 `<div>`，testid 不变），组容器新增 `explore-group-active` / `explore-group-promoted`，条目仍为 `explore-item` / `explore-item-title` / `explore-item-delete`——全局条目顺序 = 进行中组在前、已转变更组在后。条目的呈现与动作面（点击进详情、删除）零改动：已转变更组条目点击进入既有 promoted 态详情（「已转变更」徽标 + 「查看变更」跳转 `/changes/<promoted_to>`），MUST NOT 在列表内新增直达 change 的入口。promote 成功后经既有 `list.refresh()` 自然移组，详情页停留语义零改（MUST NOT 因移组触发导航或卸载详情）。

## 能力

### 新增的能力

无（纯呈现层增量，落在既有桌面能力内）。

### 修改的能力

- **desktop-explore-page** — 追加一条 requirement「清单分组与时间倒序」（ADDED：既有「探索清单页与新建流程」只定条目来源与条目面，未定清单排列，两不相冲）；Module Contract 增 `lib/explore-groups.ts`（Function）与 `explore-view.tsx` 分区呈现（Component）两行，并标注零改动面。既有六条 requirement（新建流程、双栏详情与实时预览、对话区呈现、会话链与 stance 前导、免命名定名与改名感知、workspace 切换回列表、title / workflow_type 回填、promote 入口）逐字保留。

### 零改动的能力

- **desktop-explore-queries** — explore 读面与薄命令面零改；`read_explore` / 导入扫描 / promote 语义不变。
- **desktop-workspace-store** — `list_explore_records` 的 `id` 降序契约与 `ExploreRecord` 字段面零改（本变更不把排序下推 store，故 Rust 侧测试与既有「id 降序」断言原样通过）。
- **desktop-change-queries / desktop-change-list 等 change 域能力** — 零触点（分组判据取 explore 记录自身 `promotedTo`，不查 change 记录、不新增 IPC）。

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `packages/desktop/src/lib/explore-groups.ts`（新） | 纯函数 `groupExploreRecords(records)` → `{ active, promoted }`：按 `promotedTo` 分组；组内按 `createdAt` 倒序、同刻 `id` 倒序（拷贝后排序，入参数组只读）；零 IO、零 invoke |
| `packages/desktop/src/views/explores/explore-view.tsx` | 清单区改分区呈现：`explore-items` 容器（`<ul>` → 分组外层 `<div>`）内渲染两个组 `section`（`explore-group-active` / `explore-group-promoted`），组标题「进行中 (n)」/「已转变更 (n)」，组内空呈现占位文案；零记录时维持既有 `explore-list-empty` 且不渲染分组区；`ExploreListItem` / 新建对话框 / 详情派生 / `onCreated` 导航全部零改 |

### 测试文件

| 用例 | 位置 | 处置 |
|---|---|---|
| 分组 + 组内时间倒序 + 同刻 id 兜底 + 入参只读 | `src/lib/explore-groups.test.ts`（新） | 纯函数直测：`createdAt` 与 `id` 序不一致的乱序 fixture 断言按时间倒序、同刻按 id 倒序 |
| 条目 title / name 顺序断言（`['接口重试策略', '布局设计']`） | `src/views/explores/explore-view.test.tsx:142-153` | 期望数组翻转（按 `createdAt` 倒序，fixture `createdAt = base + id` ⇒ 记录 2 在前）；断言强度不变 |
| 条目点击取 `getAllByTestId('explore-item')[1]` 期望 `layout-design` | `src/views/explores/explore-view.test.tsx:360-372` | 索引 `[1]` → `[0]`（同一倒序理由），其余零改 |
| 新增：两组合并呈现 / 组标题计数 / 空组占位 / 零记录时空态与分组区缺席 | `src/views/explores/explore-view.test.tsx` | 新增 describe（promoted fixture 与非 promoted fixture 混排） |
| 新增：promote 成功后刷新 → 记录自进行中组移入已转变更组、详情页不离开 | `src/views/explores/explore-view.test.tsx` | 复用既有 `promote_explore` mock 应答，断言移组 + `location-probe` 不离开 `/explores/<name>` |

### 不要修改

- `packages/desktop/src-tauri/**`（含 `crates/infra/store/src/store.rs` 的 `list_explore_records` `id` 降序与 `store_test.rs` 既有断言）：排序不在 store 侧下推，Rust 零改动、零新增命令、零 bindings 重生成。
- `src/types/generated/bindings.ts`：`ExploreRecord.promotedTo` / `createdAt` / `id` 已在 DTO，无新 IPC 面。
- `src/views/explores/hooks/use-explore-list.ts`：取数与动作面零改（不新增分组字段、不改 `ExploreListState` 形状）。
- `src/views/explores/explore-detail-view.tsx`、`components/explore-create-dialog.tsx`、`components/explore-preview.tsx`：既有 promoted 徽标 / 跳转 / 建档 / 预览语义零改。
- `openspec/specs/` 下其它能力 spec 与 `openspec/changes/desktop-explore-list-archive/explore.md`（只读）。
- `plugins/**` 与插件交付产物（`claude-plugins` / `cursor-plugins` / `cursor-home-image`）：零改动、零版本 bump。

## 验收标准

| ID | 验收条件 | 验证方法 |
|----|---------|---------|
| AC-1 | `groupExploreRecords` 按 `promotedTo` 二分：`null` 全入 `active`、非空全入 `promoted`，两组条目并集 = 入参全集、无丢漏无重复 | `lib/explore-groups.test.ts` 断言（含全 active / 全 promoted / 混合三例） |
| AC-2 | 组内按 `createdAt` 倒序；`createdAt` 相等时按 `id` 倒序；输入顺序与后端 `id` 降序均不参与 | `lib/explore-groups.test.ts` 乱序 fixture（`createdAt` 序与 `id` 序相反）断言输出序列 |
| AC-3 | 纯函数零副作用：入参数组顺序与元素字段不变（不 in-place sort） | `lib/explore-groups.test.ts` 调用前后深比较入参 |
| AC-4 | 清单页两个组恒渲染（有记录时）：「进行中 (n)」在前、「已转变更 (m)」在后，计数与条目数一致；`explore-items` 全局条目顺序 = 组序拼接 | `explore-view.test.tsx` 混合 fixture 断言组 testid 顺序 + 计数文案 + 条目 `data-name` 序列 |
| AC-5 | 空组呈现占位文案（「无进行中的探索。」/「暂无已转变更。」）；清单整体零记录时既有 `explore-list-empty` 在场且两个组 testid 均不在场 | `explore-view.test.tsx` 两组各空一例 + 零记录一例 |
| AC-6 | 已转变更组条目点击 → 既有 promoted 态详情（`explore-detail-*` 徽标 / 查看变更跳转 `/changes/<id>`），列表内无直达 change 的新入口 | `explore-view.test.tsx` 点击断言 + `explore-detail-view.test.tsx` 既有 promoted 用例全绿 |
| AC-7 | promote 成功后 `list_explore_records` 重取 → 该记录移入「已转变更」组，URL 仍为 `/explores/<name>`（零导航、详情不卸载） | `explore-view.test.tsx` 移组用例（断言 `location-probe` 不变 + 移组） |
| AC-8 | 回归锚不变：条目 title 渲染 / 删除动作 / 点击进详情 / 三态互斥（loading、error、empty）/ workspace 根切换落清单 / id 桥派生 | `explore-view.test.tsx` 既有用例全绿（仅两处顺序断言按倒序更新） |
| AC-9 | Rust 侧零改动：`cargo test -p store` 既有 `list_explore_records仅返回入参root的记录且id降序` 等断言原样通过 | 桌面端静态检查 / 测试执行脚本（本变更不改 Rust 文件） |

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|---|---|---|---|
| explore 笔记未定排序时间口径（`created_at` 建档时间 vs `updated_at` 最近活动） | 排序结果与产品预期不符 | 中 | 本提案定 `created_at`（与行内显示时间一致、回填不引起重排）；若产品要「最近活动优先」，仅换排序键一处（分组与呈现零改），改动代价已知 |
| `id` 可复用导致「时间序」在极端情形下仍显跳变 | 用户困惑 | 低 | 显式按 `createdAt` 排序（正是本变更要修的错位源）；同刻以 `id` 兜底保证确定性 |
| 既有顺序断言翻转掩盖真实回归 | 列表行为回归被削平 | 中 | 两处期望数组只为倒序更新，断言强度不变；新增用例使用 `createdAt` 与 `id` 序相反的乱序 fixture，确保测的是时间序而非 mock 顺序 |
| 「已转变更」组无限增长（历史探索全留） | 长列表扫读成本上升 | 中 | 本期不做折叠 / 分页 / 归档（未在诉求内，且 store 无归档语义）；组计数提醒规模，留待后续变更 |
| 占位记录（`draft-*` 未定名）归入「进行中」组 | 组内混有「分析中」条目 | 低（已接受） | 占位记录本就是草稿态探索（`promoted_to` 必为空），与详情页「分析中」临时标题同窗；promote 门禁已有显式 `Err` 拦截 |
| 分组判据取 `promotedTo` 而不校验目标 change 是否仍存在 | 目标 change 被删/归档后仍显示「已转变更」 | 低 | 打标即事实（desktop-workspace-store `mark_explore_promoted` 语义），列表不查 change 记录（避免跨 crate 读面与额外 IPC）；详情页跳转失败面既有 |
| 本变更与未归档的 `desktop-mutation-filter-observability` 无重合面 | 归档顺序冲突 | 低 | 两者不触同一 spec 文件与同一源码文件，无合并冲突 |
