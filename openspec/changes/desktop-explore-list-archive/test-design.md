# 测试设计: desktop-explore-list-archive

> **变更**: desktop-explore-list-archive
> **日期**: 2026-10-19
> **基于**: proposal.md（验收标准 AC-1 ~ AC-9）、design.md（新增/修改文件表、公共函数 / API 表、D1 ~ D6）、specs/desktop-explore-page/spec.md（ADDED「清单分组与时间倒序」+ Module Contract）

## 测试框架与布局（既有模式盘点）

- **前端（`vp test`，vite-plus/test 装载 vitest + jsdom）**：`packages/desktop/vite.config.ts` 的 `test` 节——`include: ['src/**/*.test.ts?(x)']`、`environment: 'jsdom'`、`globals: true`、`setupFiles: ['src/test-setup.ts']`（`ResizeObserver` / Web Animations / `asyncUtilTimeout` 兜底）。测试文件与源文件 **colocated 同目录**（`<name>.test.ts` / `<name>.test.tsx`）。
- **断言与装置**：`import { describe, expect, it, vi } from 'vite-plus/test'`（vitest 风格 `expect`）；组件用 `@testing-library/react`（`render` / `fireEvent` / `waitFor` / `findBy*` / `cleanup`）；进程边界 mock 统一 `vi.mock('@tauri-apps/api/core')`（`vi.hoisted` 的 `invokeMock` 按命令名分派对答 + `ChannelMock` 可编程类）；路由用真实 `MemoryRouter` + `Routes` + 位置探针（`location-probe` / `nav-type-probe`）。
- **纯函数测试先例**：`src/lib/explore-title.test.ts` / `explore-workflow-type.test.ts` / `explore-stance.test.ts`——零 mock 直测、按正向 / 异常 / 边界分 `it`、边界以「参数化连续 `expect`」形态承载。
- **本变更测试面**：新增 `packages/desktop/src/lib/explore-groups.test.ts`（纯函数直测），扩写既有 `packages/desktop/src/views/explores/explore-view.test.tsx`（两处既有顺序断言按倒序更新 + 新增分组 describe）。两个 per-file 章节正对应 design.md 的全部新增 / 修改文件；零改动面（`explore-detail-view.tsx`、`src-tauri/**`、`package.json`）不设 per-file 章节，登记见「不可测试项」。

---

## 验收范围

<!-- 逐条映射 proposal.md「验收标准」AC-1 ~ AC-9：
  - `AC ID`：proposal 验收标准编号
  - `验收条件`：原文摘录
  - `被测文件或模块`：承载用例的测试文件路径（相对项目根；源文件 → 测试文件的映射见下方「单元测试」各 `### <源文件> -> <测试文件>` heading，每行对应 design.md 的一个新增 / 修改文件；零测试路径的交付面与零改动面见「不可测试项」） -->

| AC ID | 验收条件 | 被测文件或模块 |
|--------|---------|----------|
| AC-1 | `groupExploreRecords` 按 `promotedTo` 二分：`null` 全入 `active`、非空全入 `promoted`，两组条目并集 = 入参全集、无丢漏无重复 | `packages/desktop/src/lib/explore-groups.test.ts` |
| AC-2 | 组内按 `createdAt` 倒序；`createdAt` 相等时按 `id` 倒序；输入顺序与后端 `id` 降序均不参与 | `packages/desktop/src/lib/explore-groups.test.ts`、`packages/desktop/src/views/explores/explore-view.test.tsx`（同刻兜底与全局条目序的呈现半边） |
| AC-3 | 纯函数零副作用：入参数组顺序与元素字段不变（不 in-place sort） | `packages/desktop/src/lib/explore-groups.test.ts` |
| AC-4 | 清单页两个组恒渲染（有记录时）：「进行中 (n)」在前、「已转变更 (m)」在后，计数与条目数一致；`explore-items` 全局条目顺序 = 组序拼接 | `packages/desktop/src/views/explores/explore-view.test.tsx` |
| AC-5 | 空组呈现占位文案；清单整体零记录时 `explore-list-empty` 在场且两个组 testid 均不在场 | `packages/desktop/src/views/explores/explore-view.test.tsx` |
| AC-6 | 已转变更组条目点击 → 既有 promoted 态详情，列表内无直达 change 的新入口 | `packages/desktop/src/views/explores/explore-view.test.tsx`（组内点击 → promoted 徽标与 `/changes/<id>` 跳转的端到端用例 + 零新入口用例；详情面零改动不新设计，见「不可测试项」） |
| AC-7 | promote 成功后重取 → 该记录移入「已转变更」组，URL 仍为 `/explores/<name>`（零导航、详情不卸载） | `packages/desktop/src/views/explores/explore-view.test.tsx` |
| AC-8 | 回归锚不变：条目 title 渲染 / 删除动作 / 点击进详情 / 三态互斥 / workspace 根切换落清单 / id 桥派生（既有用例仅四处顺序耦合按倒序更新：title 序、点击索引、title 两态期望、删除目标定位——后两处 proposal 测试文件表未列，见 per-file 章节注释） | `packages/desktop/src/views/explores/explore-view.test.tsx` |
| AC-9 | Rust 侧零改动：`cargo test -p store` 既有 `list_explore_records仅返回入参root的记录且id降序` 等断言原样通过 | 无新增测试文件——`packages/desktop/src-tauri/**` 零改动（既有断言原样通过由桌面端测试执行面承接，见「不可测试项」） |

---

## 单元测试

### packages/desktop/src/lib/explore-groups.ts -> packages/desktop/src/lib/explore-groups.test.ts

<!-- 新增源文件与新建 colocated 测试文件（`src/lib/` 同族先例 `explore-title.test.ts` / `explore-workflow-type.test.ts` 形态：零 mock 直测 + 正向 / 异常 / 边界分组 it）。 -->

#### 待测功能

- `groupExploreRecords(records)`: 按 `promotedTo === null` / `!== null` 二分并把两组各自按 `createdAt` 倒序、同刻 `id` 倒序，返回 `{ active, promoted }` 两个新数组；入参数组与元素字段零触碰，零 IO、零 invoke、零 React 依赖

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `groupExploreRecords` 二分判据 | 正向 | 全 active fixture（3 条 `promotedTo: null`，`createdAt` 乱序）→ `promoted` 为空数组、`active` 长度 3 且按 `createdAt` 倒序 | 单次 |
| `groupExploreRecords` 二分判据 | 正向 | 全 promoted fixture（3 条 `promotedTo` 非空，`createdAt` 乱序）→ `active` 为空数组、`promoted` 按 `createdAt` 倒序 | 单次 |
| `groupExploreRecords` 二分完备性 | 正向 | 混合 fixture（3 active + 2 promoted，对应 spec 场景）→ `active.length + promoted.length === records.length`、两组 `id` 集合求交为空、并集与入参 `id` 集合相等（无丢漏无重复，AC-1） | 单次 |
| `groupExploreRecords` 组内时间倒序 | 正向 | 乱序 fixture：`createdAt` 序与 `id` 序完全相反（最新建档者 `id` 最小）→ 组内按 `createdAt` 倒序（输出 `id` 序列与入参序、`id` 降序均不同，AC-2） | 单次 |
| `groupExploreRecords` 同刻 `id` 兜底 | 边界 | 组内 3 条 `createdAt` 全等、`id` 注入为乱序 → 输出按 `id` 倒序 | 单次 |
| `groupExploreRecords` 同刻与异刻交错 | 边界 | 5 条：每组各含「同刻对 + 异刻单条」，`createdAt` 跨组交错 → 首键倒序优先、仅同刻区间内按 `id` 倒序 | 单次 |
| `groupExploreRecords` 空数组 | 边界 | `records = []` → `{ active: [], promoted: [] }`（两数组均在场且长度 0，非 `undefined`） | 单次 |
| `groupExploreRecords` 单元素 | 边界 | 单条 active / 单条 promoted 两形态 → 对应组长度 1、另一组长度 0 | 参数化迭代 |
| `groupExploreRecords` `createdAt` 数值边界 | 边界 | `createdAt = 0` / 负值（纪元前口径）/ `Number.MAX_SAFE_INTEGER` 混合 → 按数值大小倒序（不经 `Date` 解析、不做字符串比较） | 单次 |
| `groupExploreRecords` `promotedTo` 空串 | 边界 | `promotedTo: ''` 与 `null`、`'chg-1'` 混排 → 空串归 `promoted` 组（`=== null` 判据，非 truthiness 判别；同表达式经全 active/promoted 两向锁定） | 单次 |
| `groupExploreRecords` 元素字段缺失 | 边界 | 元素 `promotedTo` 缺席（`undefined`，类型签名域外形态）→ 归 `promoted` 组（`undefined !== null` 的当前口径，不新增补缺分支） | 单次 |
| `groupExploreRecords` 元素多余字段 | 边界 | 元素携带未在 `ExploreRecord` 声明的多余字段 → 分组与排序只依赖 `promotedTo` / `createdAt` / `id`，多余字段零影响且返回元素为该对象本体 | 单次 |
| `groupExploreRecords` 大列表 | 边界 | 1000 条生成式 fixture（混合两组 + `createdAt` 含重复值）→ 两组并集完备、组内 `createdAt` 单调不增、同刻区间 `id` 严格递减 | 单次 |
| `groupExploreRecords` 域外入参 | 异常 | `records` 为 `undefined`（类型断言越过签名）→ 抛 `TypeError` 而非静默返回空分组（签名即契约，不引入防御分支） | 单次 |
| `groupExploreRecords` 入参只读（顺序 + 字段） | 异常 | 调用前深拷贝入参，调用后逐元素全字段 + 数组顺序深比较相等（不 in-place `sort`，AC-3） | 单次 |
| `groupExploreRecords` 返回数组零共享 | 边界 | 返回的 `active` / `promoted` 均非入参数组本体；对返回数组 `push` / 重排后入参长度与顺序不变 | 单次 |
| `groupExploreRecords` 幂等确定性 | 边界 | 同一入参连续调用两次 → 两次两组 `id` 序列逐元素相同（无时间源 / 随机 / IO） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 | 纯函数直测：`import type { ExploreRecord } from '../types/dto'` 为纯类型（零运行时耦合），fixture 以内联 `ExploreRecord` 字面量构造，不引入任何 `vi.mock` | 全部用例 |
| 时间源 | 不 mock：模块零时间读取，`createdAt` 全部由 fixture 显式给定（用例不含真实时钟依赖） | 全部用例 |

### packages/desktop/src/views/explores/explore-view.tsx -> packages/desktop/src/views/explores/explore-view.test.tsx

<!-- 公共函数 / API 表仅列 `ExploreView`（导出签名不变）；模块私有 `ExploreGroup`（新增）与 `ExploreListView`（内部改）不进 API 表，其行为经 `ExploreView` 渲染输出断言（组 `section` testid / 标题 `textContent` / 占位文案 / 条目 `data-name` 序列）间接覆盖。

  既有文件顺序耦合盘点（fixture `createdAt = 1727000000000 + id` ⇒ 倒序后 id 大者在前）：proposal「测试文件」表列两处（title 序 `:146`、点击索引 `:364`），实测另有**两处**同样耦合于 `createdAt` 序，一并纳入更新（断言强度不变，纯期望 / 定位调整）：
  - `:164` title 两态用例期望 `['same-name', '显式标题']` → `['显式标题', 'same-name']`；
  - `:378` 删除用例 `explore-item-delete[0]` 倒序后指向 `layout-design`，须改以 `data-name` 定位（或索引 `[1]`）锁定 `api-retry`，否则既有「删除携名」断言失义。
  - `:493` 用 `findByTestId('explore-item')` 但该用例清单仅一条记录（单元素形态）→ 顺序无关，零改动。 -->

#### 待测功能

- `ExploreView(props)`: 清单分支（`/explores`）经 `groupExploreRecords` 派生后分区呈现——`explore-items` 内两个 `section` 恒序（`explore-group-active` 前、`explore-group-promoted` 后）、组标题携组名与计数、空组渲染占位文案、零记录不渲染分组区；条目点击进详情 / 删除 / promote 刷新移组 / 三态互斥 / id 桥派生照既有语义

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 分组区两 `section` 恒序（链路：`explore-view` → `lib/explore-groups`） | 正向 | 混合 fixture（2 active + 2 promoted，`createdAt = base + id`，promoted 记录 `id` 较小）→ `explore-items` 内 `section` 的 `data-testid` 序列 = `['explore-group-active', 'explore-group-promoted']`（AC-4） | 单次 |
| 组标题与计数（链路同上传入） | 正向 | 同一 fixture → 进行中组 `textContent` = `进行中 (2)`、已转变更组 = `已转变更 (2)`；各组内 `explore-item` 节点数 = 对应计数（AC-4） | 单次 |
| 条目全局序（链路：`explore-view` → `lib/explore-groups`） | 正向 | 乱序 fixture（组内 `createdAt` 与 `id` 序相反 + 组间混排）→ `explore-items` 内 `explore-item` 的 `data-name` 序列 = active 时间倒序 ⊕ promoted 时间倒序（AC-4 / AC-2 呈现半边） | 单次 |
| 既有顺序断言更新①（title 序） | 正向 | 既有 fixture（`createdAt = 1727000000000 + id`）→ `explore-item-title` 文本序列翻转为 `['布局设计', '接口重试策略']`、`data-name` 翻转为 `['layout-design', 'api-retry']`（AC-8：仅期望倒序，断言强度不变） | 单次 |
| 既有顺序断言更新②（点击索引） | 正向 | 同一 fixture → 点击 `getAllByTestId('explore-item')[0]` → `location-probe` = `/explores/layout-design`、详情 title = 「布局设计」（索引 `[1]` → `[0]`，AC-8） | 单次 |
| 既有顺序断言更新③（title 两态用例） | 正向 | `records = [record(1, 'same-name', 'same-name'), record(2, 'explicit', '显式标题')]` → title 序列翻转为 `['显式标题', 'same-name']`（原期望 `['same-name', '显式标题']`，`createdAt = base + id` ⇒ 记录 2 在前；proposal 测试文件表未列、本设计补入，AC-8） | 单次 |
| 既有顺序耦合更新④（删除目标定位） | 正向 | 既有 fixture → 倒序后 `explore-item-delete[0]` 已指向 `layout-design`；改以 `data-name` 定位（或索引 `[1]`）取 `api-retry` 的删除入口 → `delete_explore_record` 携 `{ root, name: 'api-retry' }` 并触发一次重取（断言强度不变，仅定位方式调整，AC-8） | 单次 |
| 已转变更组条目点击进既有 promoted 态详情 | 正向 | promoted fixture（`promotedTo: 'chg-1'`）→ 点击已转变更组条目 → `location-probe` = `/explores/<name>`、`explore-promoted-badge` 文本「已转变更」；再点 `explore-promoted-jump` → `/changes/chg-1`（AC-6） | 单次 |
| 清单内零直达 change 入口 | 异常 | 清单页（含已转变更组）→ `queryByTestId('explore-promoted-jump')` 为 `null`，组内条目仅 `explore-item` / `explore-item-title` / `explore-item-delete` 三 testid（MUST NOT 新增入口，AC-6） | 单次 |
| promote 成功后移组（链路：详情页 → `list.refresh` → 重取 → 分组重算） | 正向 | 详情页 `/explores/api-retry` → 点击 `explore-promote-trigger`（`promote_explore` 应答成功）；第二次 `list_explore_records` 返回该记录 `promotedTo` 非空 → 回清单后该 `data-name` 位于已转变更组、进行中组计数减一（AC-7） | 单次 |
| promote 后零导航 / 详情不卸载 | 正向 | 与上例同一窗口 → `location-probe` 恒为 `/explores/api-retry`、`nav-type-probe` 无新 PUSH / REPLACE 迁移、`explore-detail-title` 持续在场（移组只在下次回清单时可见，AC-7） | 单次 |
| 进行中组空态占位 | 边界 | 清单仅含 promoted 记录 → 进行中组在场且 `textContent` 含「无进行中的探索。」+ `(0)`、组内 `explore-item` 数为 0；已转变更组条目在场（AC-5） | 单次 |
| 已转变更组空态占位 | 边界 | 清单仅含 active 记录 → 已转变更组在场且含「暂无已转变更。」+ `(0)`；组序仍 active 前、promoted 后（AC-5） | 单次 |
| 零记录空态与分组区缺席 | 边界 | `list_explore_records` 应答 `[]` → `explore-list-empty` 在场；`explore-group-active` / `explore-group-promoted` / `explore-items` 均不在场（AC-5 / D5） | 单次 |
| 单条记录 | 边界 | 单条 active 记录（`createdAt = 0`）→ 进行中组计数 `(1)`、已转变更组占位在场；行内时间渲染不抛错（排序断言不以显示文本为据） | 单次 |
| 同刻 `id` 兜底（视图侧） | 边界 | 同组两条 `createdAt` 全等、注入 `id` 与期望逆序 → DOM `data-name` 序列按 `id` 倒序（兜底键经视图端到端确认） | 单次 |
| 大清单呈现 | 边界 | 120 条混合 fixture（`createdAt` 与 `id` 序相反）→ 各组计数与条目数一致、组序恒 `active → promoted`（顺序断言取 `data-name`，不取本地化时间文本） | 单次 |
| 加载中态不抢跑分组区 | 边界 | `list_explore_records` 挂起未决 → `explore-list-loading` 在场；两个组 testid 与 `explore-items` 均不在场（D5 过渡轮空数组形态） | 单次 |
| 取数失败态 | 异常 | `list_explore_records` reject → `explore-list-error` 在场且含原始错误文本；`explore-list-empty`、两个组 testid、`explore-items` 均不在场（既有三态互斥零改，AC-8） | 单次 |
| 删除动作接线回归 | 正向 | 混合 fixture → 点击已转变更组条目的 `explore-item-delete` → `delete_explore_record` 携 `{ root, name }` 并触发一次重取（组内动作面零改，AC-8） | 单次 |
| workspace 根切换后分组呈现 | 边界 | 带旧选中的详情页 → `rerender` 新 root → 落 `/explores`、新根记录按分组呈现（进行中组在场、旧根 `data-name` 不在场，AC-8 落清单半边） | 单次 |
| id 桥派生回归 | 边界 | 旧名 URL + 改名后清单（`id` 不变）→ 详情照旧派生并 `replace` 收敛至新名（分组派生不改三级派生链，AC-8） | 单次 |
| `draft-*` 占位记录归进行中组 | 边界 | 占位记录（`promotedTo: null`，`sessionId` 在场）与非占位 active 记录混排 → 同处「进行中」组、不另设第三组（design 待决 3） | 单次 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `@tauri-apps/api/core` 的 `invoke` / `Channel`（进程边界 IPC） | `vi.mock('@tauri-apps/api/core')` + `vi.hoisted` 导出的 `invokeMock` / `ChannelMock`；`invokeMock.mockImplementation` 按命令名分派应答（`list_explore_records` 返回 fixture、`promote_explore` 返回 `{ changeId: 'chg-1', changeName: 'api-retry' }`，`read_explore` / `watch_subscribe` / `session_detail` 等空态应答） | 全部用例（清单取数、promote、删除、详情派生） |
| `list_explore_records` 应答序列（IPC 应答注入） | 队列式应答（`shift()` 出队：首次取数、`refresh` 后第二次取数各出一次），用例内按需重置 | promote 移组、onCreated 刷新链、id 桥回归 |
| `react-router` | 不 mock：真实 `MemoryRouter` + `Routes`（`/explores`、`/explores/:name`、`/changes/:id`）+ `LocationProbe`（`useLocation` / `useNavigationType` 探针 span） | 移组零导航、点击进详情、`/changes/<id>` 跳转断言 |
| `hooks/use-explore-list` / `explore-detail-view` / `components/explore-create-dialog` / `lib/explore-groups` | 真实组合，不 mock（分组派生与详情派生即被测行为；组合用例挂靠本文件即最上层调用方） | 全部用例 |
| 时间显示（`new Date(createdAt).toLocaleString()`） | 不 mock（`Date` / 时区不入断言）：排序与组序断言一律取 `data-name` / 组内节点序，不取本地化时间文本 | 全部用例 |
| jsdom 环境缺件（`ResizeObserver` / Web Animations / `BASE_UI_ANIMATIONS_DISABLED`） | 沿用既有 `src/test-setup.ts` 兜底实现，不新增 mock | 详情页双栏与 `@base-ui` 组件挂载 |

---

## 不可测试项

| 模块 | 原因 |
|---|---|
| `packages/desktop/package.json` | 版本 bump（交付面）：`version` 0.5.17 → 0.5.18 无单元测试路径；由桌面端静态检查与打包流程（`vp build` / `tauri.conf.json` 经 `../package.json` 跟随）承接（D6） |
| `packages/desktop/src/views/explores/explore-detail-view.tsx`（及其既有 `explore-detail-view.test.tsx`） | 零改动面：design.md「新增文件」/「修改文件」两表均未收录该文件（仅出现在架构组件表「详情与新建面（零改动，复用）」行），「公共函数 / API」表亦无该文件行 → 不设 per-file 章节、不新设计用例。AC-6 的详情半边（promoted 徽标 / 「查看变更」跳转 `/changes/<id>` / promote 失败面）由 `explore-view.test.tsx` 的组内点击用例端到端断言，并叠加既有 `explore-detail-view.test.tsx` 原样重跑作回归锚（分区改动若误触详情面即转红） |
| `packages/desktop/src-tauri/**`（含 `crates/infra/store/src/store.rs` 的 `list_explore_records` 与 `store_test.rs`） | 「零改动面」（AC-9）：本变更不新增 / 不修改任何 Rust 文件、不新增 IPC 命令、零 bindings 重生成；既有 `cargo test -p store` 断言（`list_explore_records仅返回入参root的记录且id降序`）原样通过即证，不为其另设测试 |
| `ExploreGroup` 的标题层级与样式类（`<h3>` 层级、`text-muted-foreground` / `mb-3 last:mb-0`） | 纯视觉与无障碍层级，无行为契约；断言面收敛为组 `section` testid 顺序、组 `textContent`（组名 + 计数）与占位文案（design D4 明示不为标题 / 空态新增 testid），样式面由 `vp check` 静态检查承接 |
| 组标题计数的 DOM 内部分层（`{label} <span>({n})</span>` 的 span 边界） | 计数口径取组容器 `textContent` 文本断言（`进行中 (2)`）；span 分层属呈现细节，单独断言会锁定与 spec 无关的实现形态 |
| 占位文案以外的空组视觉差异（空组 `<div>` 的 className） | 不存在独立 testid（D4 定稿），空组断言以文案 + 计数 `(0)` + 「组内零 `explore-item`」三面表达，样式类不设断言 |
