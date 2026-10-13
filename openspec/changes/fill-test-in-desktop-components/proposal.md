# 提案: fill-test-in-desktop-components

> **变更**: fill-test-in-desktop-components
> **日期**: 2026-10-13
> **状态**: draft
> **探索**: `openspec/changes/fill-test-in-desktop-components/explore.md`（一句话目标：补充 `packages/desktop/src/components/standard` 目录下的单元测试）

---

## 问题

`packages/desktop/src/components/standard/` 是视图域标准交互组件层——`dailog.tsx` 的 `StandardDialog`、`drawer.tsx` 的 `StandardDrawer`、`select.tsx` 的 `StandardSelect`、`hooks/use-open.ts` 的 `useOpen`——被 change-create-dialog（desktop-change-create）、detail-drawer（desktop-change-flow-view）、agent-run-form / agent-run-history（desktop-agent-execution）四个消费面复用，但**该目录零专属单元测试**（四个源文件、零 `*.test.*`）。缺口有三：

1. **契约无直接回归防线**：受控 / 非受控开合归一（useOpen 的 prop 同步三分支与回调缺省）、trigger 可选双态、StandardDialog footer 双态（「取消/提交」vs「关闭」）与 `onSubmit(close)` 显式关窗语义（点击提交本身不关窗）、StandardSelect 值双态（null → placeholder 占位且 `data-value` 属性缺席）等行为只被消费方测试间接路过，分支语义无断言锚；任一消费方测试重构都可能静默丢掉这些行为锁定。
2. **覆盖数据证实间接覆盖不锁分支**：最近一次 change 语料回归的 desktop 覆盖汇总（`openspec/changes/archive/2026-10-12-desktop-test-only-flow/reports/test/packages_desktop_vite-plus/coverage-summary.json`）里 `use-open.ts` lines 88.88% / branches 83.33%，其余三个组件仅靠消费路径路过达 lines 100%——行覆盖不等于契约锁定。
3. **mutation 面无专属断言**：`openspec/config.json` desktop 套件 includes `src/**/*.{ts,tsx}`（仅排除 `src/components/ui/**` 与 `src/routes.tsx`），`components/standard/**` 既在 coverage 阈值（lines 90 / branches 85 / functions 90）达标面内、也在 mutation score 80 达标面内，间接断言对本层 mutant 的扑杀不可控。

## 提案

为标准交互组件层补 colocated 单元测试，并把该层契约立档为新能力：

1. **colocated 单测四文件**：`dailog.test.tsx` / `drawer.test.tsx` / `select.test.tsx` / `hooks/use-open.test.ts` 与源文件同目录落位，直接锁定既有行为契约（细则见能力节与 specs delta），产品源码零改动。
2. **测试风格沿既有先例**：`vite-plus/test` + `@testing-library/react`（沿 `change-create-dialog.test.tsx` / `agent-run-form.test.tsx` / `use-workspaces.test.ts` 先例）；查询挂钩为 role / 可及名 / 文本 / `data-testid`，MUST NOT 样式类名查询；base-ui 弹层开合异步收敛（Select 以 `fireEvent.mouseDown` 打开、option 经 `pointerDown` + `click` 选中、`findByRole('listbox')` / `waitFor` 收敛）；本层零进程边界，MUST NOT mock 被测组件或 `ui/**` 底座。
3. **新能力 `desktop-standard-components` 立档**：grep 全 `openspec/specs/`（`StandardDialog` / `StandardDrawer` / `StandardSelect` / `useOpen` / `components/standard` 零命中）确认该层此前无任何能力覆盖，本次以 `## ADDED Requirements` 立档行为契约 + 测试纪律，「单测直达契约分支、不依赖消费方间接覆盖」随同立为 requirement。
4. **test-only 纪律**：四个被测源文件与消费方既有测试一律不动；若单测揭示既有行为瑕疵（见 R1），按现状锁定、留待独立变更处理，MUST NOT 顺手改产品代码。
5. **版本交付**：`packages/desktop` 0.5.0 → 0.5.1（归档时执行）；`plugins/dev-team` 零改动（保持 2.10.44）。

## 关键决策

| ID | 决策 | 理由 | 被拒绝的备选方案 |
|----|------|------|------------------|
| D1 | **测试 colocated 落位**（`dailog.test.tsx` / `drawer.test.tsx` / `select.test.tsx` / `hooks/use-open.test.ts` 与源文件同目录） | 沿消费方测试先例（`change-create-dialog.test.tsx` / `agent-run-form.test.tsx` / `use-workspaces.test.ts` 均与源文件同目录），契约断言与被测文件零距离、重构时同目录同步演进 | **备选：集中到独立 tests 目录**。被拒绝，偏离本仓库 colocated 既有约定，契约断言与被测文件分离，目录调整时易失联。**备选：仅强化消费方集成测试补齐覆盖**。被拒绝，间接覆盖不锁本层分支（见问题 1 / 2），消费方测试重构会静默丢契约锚 |
| D2 | **现状锁定、产品源码零改动**（test-only 纪律） | 本变更目标是补测试缺口而非改行为；全部断言可经 DOM 观察面（role / 可及名 / 文本 / data-*）承载，无需动源码 | **备选：为可测性改组件签名 / 抽内部结构 / 加测试专用导出**。被拒绝，污染产品代码，且违反 AGENT.md「No test-only exports」约定（knip 死代码报告被污染）。**备选：顺手修 R1 的 useOpen 回调闭包陈旧**。被拒绝，混入行为变更会破坏 test-only 审查口径，该语义若确认为缺陷留待独立 change 修 |
| D3 | **新能力 `desktop-standard-components` 立档**（`## ADDED Requirements`） | grep 全 `openspec/specs/`（`StandardDialog` / `StandardDrawer` / `StandardSelect` / `useOpen` / `components/standard`）零命中，该层此前无任何能力覆盖，须以 ADDED 立档行为契约 + 测试纪律 | **备选：并入既有消费方能力**（desktop-change-create / desktop-change-flow-view / desktop-agent-execution 追加 requirement）。被拒绝，本层是被四个消费面复用的独立组件层，归入任一消费面造成能力边界错位。**备选：并入 desktop-app-shell**。被拒绝，desktop-app-shell 只承载测试挂钩与 vendored 底座纪律，不承载本层组件契约 |
| D4 | **真实渲染 + 异步收敛，MUST NOT mock `ui/**` 底座** | 本层零进程边界，base-ui 弹层开合 / 选中语义只有经真实组件渲染才锁得住；`findByRole('listbox')` / `waitFor` 收敛消除 frame 异步 flaky（R2） | **备选：mock `src/components/ui/**` 加速**。被拒绝，mock 把 base-ui 弹层行为剔出被测面，契约断言失真。**备选：同步断言 + fake timers**。被拒绝，与 base-ui frame 异步动作竞态，历史先例（`agent-run-form.test.tsx`）已确立异步收敛写法 |

## 能力

### 新增能力

- **desktop-standard-components** — `components/standard` 标准交互组件层：`useOpen` 受控 / 非受控开合归一（prop 同步三分支 + 回调缺省安全）；`StandardDialog` trigger 可选 + footer 双态 + `onSubmit(close)` 显式关窗；`StandardDrawer` 恒「关闭」footer + `containerClassName` 透传 + 右侧滑出；`StandardSelect` 泛型值语义（`T | null`）+ `data-value` / `data-testid` 透出 + placeholder 双态 + disabled 透传；colocated 单测纪律（直达契约分支、无样式类名查询、弹层异步收敛、零 mock 底座）。

### 修改的能力

- 无（既有能力全部零 delta）。

### 引用沿用（零 delta）

- **desktop-change-create** / **desktop-change-flow-view** / **desktop-agent-execution** — 四个消费面（change-create-dialog / detail-drawer / agent-run-form / agent-run-history）行为零变化，消费契约随新能力收口。
- **desktop-app-shell** — 「data-testid 测试挂钩」与「shadcn 控件替换与 vendored 内部化」纪律沿用（类名仅作透传断言的被检属性、不作查询挂钩；`ui/**` 底座照常全管线无豁免）。

## 变更范围

### 实现文件

- `packages/desktop/package.json` — `version` 0.5.0 → 0.5.1（归档时执行；产品源码其余零改动——本变更为 test-only）

### 测试文件

- `packages/desktop/src/components/standard/hooks/use-open.test.ts` — useOpen 契约：非受控初始态与开合回调、受控 `outerOpen` 变化同步（undefined / 同值不干预分支）、回调缺省安全（`renderHook` 承载）
- `packages/desktop/src/components/standard/dailog.test.tsx` — StandardDialog 契约：trigger 有无双态、footer 双态（「取消/提交」vs「关闭」）、`onSubmit(close)` 恰一次且提交不自动关窗（回调内 `close()` 后才关）、取消 / 关闭零 `onSubmit`
- `packages/desktop/src/components/standard/drawer.test.tsx` — StandardDrawer 契约：trigger 开合 + 「关闭」关窗、footer 恒「关闭」单按钮、`containerClassName` 透传（类名作被检属性）、受控 `open` / `onOpenChange` 随开合回报
- `packages/desktop/src/components/standard/select.test.tsx` — StandardSelect 契约：弹层选项 = items labels、选中 `onChange` 收选中值、value null / 非 null 呈现与 `data-value` 双态、placeholder 默认「请选择」/ 自定义、disabled 禁开

### 不要修改

- `packages/desktop/src/components/standard/` 四个源文件（`dailog.tsx` / `drawer.tsx` / `select.tsx` / `hooks/use-open.ts`）——被测对象零改动（MUST NOT 为可测性改签名、抽内部结构或加导出）
- `packages/desktop/src/components/ui/**`（base-ui / shadcn 底座，测试经真实组件渲染，MUST NOT mock）
- 消费方视图与其既有测试（`change-create-dialog` / `detail-drawer` / `agent-run-form` / `agent-run-history` 的源文件与 `*.test.tsx` 一律不动）
- `openspec/config.json` 测试矩阵（desktop 套件 includes 已覆盖 `components/standard/**`，MUST NOT 加 excludes / 豁免）
- `plugins/dev-team` 全部（保持 2.10.44 零改动）

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | useOpen 单测 | 非受控初始 `false`、开合回调收新值、受控 `outerOpen` 变化同步（同值注入不重设）、`onOuterOpenChange` 缺省不报错，四面断言在场 |
| AC-2 | StandardDialog 单测 | trigger 有无双态；footer 双态（onSubmit 在场 →「取消」+「提交」，缺席 →「关闭」）；`onSubmit` 恰一次且入参为 `close` 函数；点击「提交」不自动关窗、回调内 `close()` 后内容退场；「取消」/「关闭」关窗且 `onSubmit` 零调用 |
| AC-3 | StandardDrawer 单测 | trigger 开合 + 「关闭」关窗（`onOpenChange` 随开合回报）；footer 恒「关闭」单按钮；`containerClassName` 透传内容容器；无 trigger 时受控 `open` 驱动呈现 |
| AC-4 | StandardSelect 单测 | 弹层选项 = items labels；选中 `onChange` 收选中 value；`value=null` 呈 placeholder 且 `data-value` 缺席 / 非 null 呈 label 且 `data-value` 在场；默认「请选择」与自定义 placeholder；disabled 禁开零回调 |
| AC-5 | 测试纪律 | 四个 colocated 测试文件在场且契约分支直达；零样式类名查询挂钩（`querySelector` / `getElementsByClassName` 零命中，类名仅作被检属性）；不 mock 被测组件与 `ui/**` 底座；Select 弹层以 `findByRole('listbox')` 异步收敛 |
| AC-6 | 全管线通过 | `pnpm -C packages/desktop run client:check` 与 `pnpm -C packages/desktop run test` 全绿；desktop 套件覆盖率达阈值（lines 90 / branches 85 / functions 90）、mutation score 达 80；`openspec/config.json` 无新增 excludes / 豁免 |
| AC-7 | 版本交付（归档时执行） | `packages/desktop/package.json` `version` 0.5.0 → 0.5.1；`tauri.conf.json` 维持 `../package.json` 引用，`src-tauri/Cargo.toml` 版本不随动；`plugins/dev-team` 保持 2.10.44 零改动 |

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| R1 useOpen 回调闭包陈旧 | `handleOpenChange` 以 `useCallback(..., [])` 闭包捕获首渲染的 `onOuterOpenChange`，若以「中途替换回调后新回调被调」为断言会失败于现状 | 中 | test-only 纪律按现状锁定（断言以稳定回调 `vi.fn()` 单实例承载）；该语义若确认为缺陷留待独立变更修，本变更 MUST NOT 顺手改源码 |
| R2 base-ui 弹层 jsdom 异步开合 | Dialog / Select 开启动作为 frame 异步执行，同步断言易 flaky | 中 | 沿 `agent-run-form.test.tsx` 先例：`fireEvent.mouseDown` 打开 Select、`findByRole('listbox')` / `waitFor` 收敛最终态；AC-5 显式断言收敛方式 |
| R3 类名透传断言与 testid 纪律的边界 | `containerClassName` / `className` 透传只能经类名属性断言，易被误读为违反「无样式类名查询」 | 低 | spec 显式区分：类名只作**被检属性**（对 role / testid 定位元素断言 className），不作**查询挂钩**（AC-5 含该口径） |
| R4 测试可测性倒逼产品改动 | 组件难以直接断言时若改源码，test-only 纪律被破坏 | 低 | 「不要修改」锁定四个源文件；断言全部经 DOM 角色 / 可及名 / 文本 / data-* 观察面承载，不为测试加导出或 props |
| R5 间接覆盖与直接断言重叠 | 消费方既有测试与新单测断言重叠，维护面变宽、重构需双处改 | 低 | 消费方测试不动（锁消费路径契约）；standard 单测只锁本层自有契约分支，不复刻消费方集成断言 |
| R6 覆盖 / mutation 达标口径 | `use-open.ts` branches 83.33% 待补平；`components/standard/**` 在 mutate glob 内（config 仅排除 `ui/**`），间接断言扑杀不可控 | 低 | AC-6 显式要求阈值达标；四文件直接断言负责扑杀本层 mutant，无新增豁免口径 |
