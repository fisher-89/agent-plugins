# 设计: fill-test-in-desktop-components

> **变更**: fill-test-in-desktop-components
> **日期**: 2026-10-13
> **基于**: proposal.md、explore.md、specs/desktop-standard-components/spec.md（desktop-standard-components ADDED）
> **性质**: test-only —— 本设计只测绘**既有**实现（`packages/desktop/src/components/standard/` 四文件），不提出新实现方案；产品源码零改动（AC-1…AC-5 的断言对象 = 下列契约面；AC-7 版本交付见「配置」）

---

## 提案与规格同步状态

`proposal.md` 与 `specs/desktop-standard-components/spec.md` 已由提案阶段定稿，本设计基于其文本展开，把「测试覆盖范围」落到被测文件的逐条既有契约测绘。工作树实况（本变更起点，`packages/desktop` 0.5.0 基线）：

- `packages/desktop/src/components/standard/` 恰四文件：`dailog.tsx` / `drawer.tsx` / `select.tsx` / `hooks/use-open.ts`，零 `*.test.*`（`ls` / `find` 确认）。
- `grep -r "StandardDialog\|StandardDrawer\|StandardSelect\|useOpen\|components/standard" openspec/specs/` 零命中 —— 该层此前无任何能力覆盖（支撑 proposal D3 的 ADDED 立档）。
- `useOpen` 仅被 `dailog.tsx` / `drawer.tsx`（`./hooks/use-open`）内部消费，目录外零直接引用；四个消费面只 import 三个组件。
- 覆盖数据佐证（proposal 问题 2）：`openspec/changes/archive/2026-10-12-desktop-test-only-flow/reports/test/packages_desktop_vite-plus/coverage-summary.json` 中 `use-open.ts` lines 88.88% / branches 83.33%，余下三组件仅被消费路径路过。

本设计的「变更清单」以被测文件为入口测绘既有契约；proposal「变更范围 - 测试文件」的四个测试文件（`dailog.test.tsx` / `drawer.test.tsx` / `select.test.tsx` / `hooks/use-open.test.ts`）由 test-design / test-gen / test-execution 相位承接，本清单不展开。

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 开合状态归一 hook | `useOpen` 归一受控 / 非受控开合：非受控初始 `outerOpen ?? false`；`outerOpen` prop 变化同步内部态（undefined / 同值不干预）；变更函数先落内部态再回调 `onOuterOpenChange?.(value)`（回调缺省安全） | `packages/desktop/src/components/standard/hooks/use-open.ts` | react `useState` / `useEffect` / `useCallback` | TypeScript、React 19 hook |
| 标准对话框 | `StandardDialog`：title / content 主体；trigger 可选双态（`trigger !== undefined` 才渲染 `DialogTrigger render={trigger}`）；footer 双态（onSubmit 为函数 → 「取消」+「提交」，否则仅「关闭」）；「提交」→ `onSubmit(close)` 显式关窗；开合态唯一持有于 `useOpen` | `packages/desktop/src/components/standard/dailog.tsx` | `@/components/ui/dialog`、`@/components/ui/button`、`./hooks/use-open` | TypeScript、React、base-ui Dialog（经 shadcn 式 `ui/dialog` 装配） |
| 标准抽屉 | `StandardDrawer`：title / content 主体（`swipeDirection="right"` 右侧滑出）；trigger 可选同 Dialog 式；footer 恒「关闭」单按钮；`containerClassName` 与未识别 props 透传 `DrawerContent`；开合态唯一持有于 `useOpen` | `packages/desktop/src/components/standard/drawer.tsx` | `@/components/ui/drawer`、`@/components/ui/button`、`./hooks/use-open` | TypeScript、React、base-ui Drawer（经 `ui/drawer` 装配） |
| 标准选择器 | `StandardSelect<T extends string \| number>`：泛型值语义（`value: T \| null`）单选；items 逐项渲染 `SelectItem`；`SelectValue` 透出 `data-testid` / `data-value` / `placeholder` 与未识别 props；`disabled` 透传底层 `Select` | `packages/desktop/src/components/standard/select.tsx` | `@/components/ui/select` | TypeScript、React 泛型组件、base-ui Select（经 `ui/select` 装配） |
| vendored UI 底座 | `ui/dialog` / `ui/drawer` / `ui/select` / `ui/button`（base-ui 原语 + cn 类名收敛 + lucide 图标）；测试经真实组件渲染，MUST NOT mock（desktop-app-shell 的 vendored 内部化纪律沿用） | `packages/desktop/src/components/ui/{dialog,drawer,select,button}.tsx` | `@base-ui/react`、`cn`、`lucide-react` | TypeScript、React、base-ui ^1.8.0 |
| 消费面（引用沿用，零改动） | change-create-dialog（`StandardDialog` + `onSubmit(close)`）、detail-drawer（`StandardDrawer` + `containerClassName` + `onOpenChange`）、agent-run-form（`StandardSelect` + `data-testid`）、agent-run-history（`StandardDrawer` + `onOpenChange`） | `packages/desktop/src/views/changes/components/change-create-dialog.tsx`、`packages/desktop/src/views/changes/flow/detail-drawer.tsx`、`packages/desktop/src/views/agent/components/agent-run-form.tsx`、`packages/desktop/src/views/agent/components/agent-run-history.tsx` | standard 组件层 | TypeScript、React |

> **零改动沿用**（proposal「不要修改」）：四个被测源文件（被测对象零改动，MUST NOT 为可测性改签名 / 抽内部结构 / 加导出——AGENT.md「No test-only exports」）；`ui/**` 底座（照常全管线无豁免）；四个消费面源文件与其既有测试（`change-create-dialog.test.tsx` / `agent-run-history.test.tsx` / `agent-run-form.test.tsx` 等，锁消费路径契约）；`openspec/config.json` 测试矩阵；`plugins/dev-team`（保持 2.10.44）。

---

## 变更清单

> 本清单测绘**既有**实现（全部为「既有（零改动）」）。本变更为 test-only，唯一实现文件改动是归档时的版本号（见「配置」）。

### 被测文件

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/src/components/standard/hooks/use-open.ts` | 开合状态归一 hook（AC-1 断言对象）：`useState(outerOpen ?? false)` 初始态；`useEffect`（deps `[outerOpen]`）在 `outerOpen !== undefined && outerOpen !== innerOpen` 时 `setInnerOpen(outerOpen)`（undefined / 同值两分支不干预）；`handleOpenChange` 先 `setInnerOpen(value)` 再 `onOuterOpenChange?.(value)` |
| `packages/desktop/src/components/standard/dailog.tsx` | `StandardDialog`（AC-2 断言对象）：`hasTrigger = trigger !== undefined`（:34）、`hasSubmit = typeof onSubmit === 'function'`（:35）；`DialogTrigger render={trigger}`（:42）；`DialogContent {...rest}` 透传未识别 props（:43）；footer：`DialogClose render={<Button variant="outline" />}` 文本「取消」/「关闭」双态（:49-51）、`<Button onClick={() => onSubmit?.(close)}>提交</Button>`（:52-56）；`close = useCallback(() => setOpen(false), [])`（:38）——点击「提交」本身不关窗，关窗时机由回调内显式 `close()` 决定 |
| `packages/desktop/src/components/standard/drawer.tsx` | `StandardDrawer`（AC-3 断言对象）：`Drawer swipeDirection="right"` 右侧滑出（:36）；`DrawerTrigger render={trigger}` 可选（:37）；`DrawerContent className={containerClassName} {...rest}` 透传（:38）；footer 恒「关闭」单按钮（:44）；开合经 `useOpen(outerOpen, onOuterOpenChange)`（:33），开合变更随 `onOpenChange` 回报 |
| `packages/desktop/src/components/standard/select.tsx` | `StandardSelect<T extends string \| number>`（AC-4 断言对象）：`placeholder = '请选择'`（:24）、`disabled = false`（:25）缺省；`Select value={value} items={items} disabled={disabled} onValueChange={onChange}`（:30）；`SelectValue data-testid={testId} data-value={value} placeholder={placeholder} {...rest}`（:32）——`value` 为 null 时 `data-value` 属性缺席（React 不渲染 `data-*={null}`），`id` / `className` 等经 `...rest` 透传 `SelectValue`；`items.map` 逐项 `<SelectItem value={item.value}>{item.label}</SelectItem>`（:34-40） |

### 公共函数 / API

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|----------|------|------|
| `useOpen` | `packages/desktop/src/components/standard/hooks/use-open.ts` | 既有（零改动） | `export function useOpen(outerOpen?: boolean, onOuterOpenChange?: (value: boolean) => void): OpenPropState`（`OpenPropState = [boolean, (value: boolean) => void]`） | 受控 / 非受控开合归一（AC-1）：非受控初始 `outerOpen ?? false`；`outerOpen` 变化同步内部态（undefined / 同值不干预）；返回的变更函数先落内部态再回调 `onOuterOpenChange?.(value)`，回调缺省（undefined）不报错。`handleOpenChange` 为 `useCallback(..., [])`（闭包捕获首渲染 `onOuterOpenChange`，见待决问题 R1） |
| `StandardDialog` | `packages/desktop/src/components/standard/dailog.tsx` | 既有（零改动） | `export function StandardDialog({ title, trigger, content, onSubmit, open: outerOpen, onOpenChange: onOuterOpenChange, ...rest }: StandardDialogProps): React.JSX.Element`（返回类型为推断） | 标准对话框（AC-2）：trigger 可选双态、footer 双态、`onSubmit(close)` 显式关窗（`close: () => void` 关窗并经 `onOpenChange` 回报 `false`）；`open` / `onOpenChange` 经 `useOpen` 归一 |
| `StandardDrawer` | `packages/desktop/src/components/standard/drawer.tsx` | 既有（零改动） | `export function StandardDrawer({ title, content, containerClassName, trigger, open: outerOpen, onOpenChange: onOuterOpenChange, ...rest }: StandardDrawerProps): React.JSX.Element`（返回类型为推断） | 标准抽屉（AC-3）：右侧滑出、trigger 可选同 Dialog 式、footer 恒「关闭」、`containerClassName` 透传 `DrawerContent.className`、未识别 props 透传内容容器；开合经 `useOpen` |
| `StandardSelect` | `packages/desktop/src/components/standard/select.tsx` | 既有（零改动） | `export function StandardSelect<T extends string \| number>({ value, onChange, items, placeholder = '请选择', disabled = false, 'data-testid': testId, ...rest }: StandardSelectProps<T>): React.JSX.Element`（返回类型为推断） | 标准选择器（AC-4）：泛型值语义、items 逐项渲染、选中经 `onChange(value: T \| null)`、`data-value` / `data-testid` 透出、placeholder 双态（缺省「请选择」）、`disabled` 透传底层 `Select` |

### 类型定义

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|--------|----------|----------|------|
| `OpenPropState` | `packages/desktop/src/components/standard/hooks/use-open.ts` | 既有（零改动） | 模块私有别名 `[boolean, (value: boolean) => void]` —— `useOpen` 返回二元组：`[open, setOpen]` |
| `StandardDialogProps` | `packages/desktop/src/components/standard/dailog.tsx` | 既有（零改动） | 模块私有 type：`title: string`；`content: React.JSX.Element`；`trigger?: React.JSX.Element`；`onSubmit?: (close: () => void) => void`；`open?: boolean`；`onOpenChange?: (open: boolean) => void` |
| `StandardDrawerProps` | `packages/desktop/src/components/standard/drawer.tsx` | 既有（零改动） | 模块私有 type：`title: string`；`content: React.JSX.Element`；`containerClassName?: string`；`trigger?: React.JSX.Element`；`open?: boolean`；`onOpenChange?: (open: boolean) => void` |
| `StandardSelectProps<T extends string \| number>` | `packages/desktop/src/components/standard/select.tsx` | 既有（零改动） | 模块私有 interface：`value: T \| null`；`onChange: (value: T \| null) => void`；`items: { value: string \| number; label: string }[]`；`placeholder?: string`；`disabled?: boolean`；`'data-testid'?: string`；`id?: string`；`className?: string` |

> 三个 Props 类型与 `OpenPropState` 均无 `export`（模块私有，契约经组件 / hook 签名对外暴露）；MUST NOT 为测试加导出（AGENT.md「No test-only exports」，knip 死代码报告）。

### 配置

| 配置键 | 所在文件 | 新增/修改 | 值类型 | 默认值 | 说明 |
|--------|----------|----------|--------|--------|------|
| `version` | `packages/desktop/package.json` | 修改（归档时执行） | string | `0.5.1`（现行 `0.5.0`） | 版本交付（AC-7）：0.5.0 → 0.5.1；`src-tauri/tauri.conf.json` `"version": "../package.json"` 引用维持、自动跟随，`src-tauri/Cargo.toml` 版本不随动；`plugins/dev-team` 保持 2.10.44 零改动 |
| `tests[]`（desktop 套件） | `openspec/config.json` | 既有（零改动） | object | root `packages/desktop`、framework `vite-plus`、config `vite.config.ts`、includes `["src/**/*.{ts,tsx}"]`、excludes `["src/components/ui/**/*","src/routes.tsx"]`、coverage `{lines 90, branches 85, functions 90}`、mutation `{score 80}` | AC-6 达标口径来源：`components/standard/**` 既在覆盖阈值面内、也在 mutation 达标面内；MUST NOT 加 excludes / 豁免（现状 excludes 仅 `ui/**` 与 `routes.tsx`） |
| `mutate` | `packages/desktop/stryker.config.json` | 既有（零改动） | string[] | `["src/**/*.ts","src/**/*.tsx","!src/**/*.test.ts","!src/**/*.test.tsx","!src/components/ui/**"]`（thresholds high 80 / low 60 / break 50） | mutation 面含 `components/standard/**`（仅排 `ui/**`）；四文件直接断言负责扑杀本层 mutant（AC-6） |
| `test` | `packages/desktop/vite.config.ts` | 既有（零改动） | object | include `['src/**/*.test.ts?(x)']`、environment `jsdom`、globals `true`、setupFiles `['src/test-setup.ts']` | `vp test` 面：colocated 四测试文件随 include 自动入列；jsdom 环境 + `test-setup.ts` 兜底（`BASE_UI_ANIMATIONS_DISABLED`、`getAnimations` / `ResizeObserver` stub） |
| `lint.overrides`（`*.test.ts` / `*.test.tsx`） | `packages/desktop/vite.config.ts` | 既有（零改动） | object | 关闭 `max-lines-per-function`、放开 `no-non-null-assertion` / `no-unsafe-type-assertion` | 既有测试文件 lint 口径，四测试文件沿用（AC-5 的 `client:check` 通过面） |

---

## 数据模型

无持久化、无库表、无 DTO —— 全部为 React 组件内存态与 DOM 观察面：

- **开合状态**：`open: boolean`，由 `useOpen` 单点归一（`StandardDialog` / `StandardDrawer` 各自唯一持有，MUST NOT 另持第二份开合状态）。非受控初始 `outerOpen ?? false`；受控时 `outerOpen` prop 变化同步内部态（undefined / 与内部态同值不干预）。状态变更路径：返回的 `setOpen(value)` → 内部态 + `onOuterOpenChange?.(value)`；Dialog 的 `close()` = `setOpen(false)`。
- **StandardSelect 值语义**：`value: T | null`（`T extends string | number`）。`null` → 触发器呈 `placeholder` 文本（缺省「请选择」）且 `data-value` 属性缺席；非 null → 呈该项 label 且 `data-value` = 选中值（DOM 属性字符串化）。选中经 `onChange(value: T | null)` 抛出（base-ui `onValueChange` 直连）。
- **items 清单**：`{ value: string | number; label: string }[]` —— `value` 作 `SelectItem` 的 key 与 value，`label` 作呈现文本；清单整体同时传 base-ui `Select` 的 `items` 入参与逐项 `SelectItem` 渲染。
- **DOM 观察面**（测试断言承载面，查询挂钩 = role / 可及名 / 文本 / `data-testid` 及语义 data-*）：`data-testid`（`SelectValue`）、`data-value`（`SelectValue`，null 缺席）、role（dialog / listbox / option / combobox）、可及名 / 按钮文本（「取消」「提交」「关闭」）、占位文本（「请选择」/ 自定义）、`containerClassName` / `className`（类名仅作透传断言的被检属性，MUST NOT 作查询挂钩——desktop-app-shell 纪律沿用）。
- **关系**：`useOpen` ← `StandardDialog` / `StandardDrawer`（开合态归一唯一入口）；`StandardSelect` 无 hook、无内部状态（值完全受控）；四消费面 → 三组件（import 关系，行为零变化）。

---

## 路由 / API 设计

<!-- 整段省略：本层为纯展示组件层（React 组件 + hook），无 HTTP 端点 / IPC 命令 / CLI 子命令；被测四文件不触进程边界（proposal D4「零进程边界」）。 -->

---

## 依赖

### 运行时依赖

- 零新增 npm 依赖（test-only）。
- `react` ^19（`useState` / `useEffect` / `useCallback` / JSX）：被测四文件的框架基座。
- `@base-ui/react` ^1.8.0（dialog / drawer / select 原语）：经 `@/components/ui/{dialog,drawer,select}` 装配后被 standard 层消费；测试经真实渲染锁弹层开合 / 选中语义（MUST NOT mock，proposal D4）。
- `cn`、`lucide-react`（`ui/dialog` 的 XIcon 等）：vendored 底座既有依赖，随真实渲染入测面。
- `@/components/ui/button`：Dialog / Drawer footer 按钮载体（`DialogClose render={<Button variant="outline" />}` 同式）。

### 构建 / 测试依赖

- 零新增；全部沿既有先例（proposal「测试风格沿既有先例」）。
- `vite-plus`（`vp test --sequence.shuffle` / `vp check --fix`）：测试执行与 lint（`client:check` = `vp check --fix && knip`，AC-6 全绿面）；测试先例 `change-create-dialog.test.tsx` / `agent-run-form.test.tsx` / `use-workspaces.test.ts`（colocated、`@testing-library/react`）。
- `@testing-library/react` ^16.3.0：`render` / `renderHook` / `fireEvent` / `screen` / `within` / `waitFor` / `findByRole`——查询挂钩 role / 可及名 / 文本 / `data-testid`（AC-5）。
- `jsdom` ^26 + `src/test-setup.ts`：test 环境兜底（`BASE_UI_ANIMATIONS_DISABLED` 置位、`getAnimations` / `ResizeObserver` stub）——base-ui 弹层 jsdom 兼容面。
- `@vitest/coverage-v8`：覆盖率（AC-6 阈值 lines 90 / branches 85 / functions 90，`use-open.ts` branches 83.33% 待补平）。
- `@stryker-mutator/core` + `@stryker-mutator/vitest-runner`：mutation（AC-6 score 80；mutate glob 含 `components/standard/**`）。
- `knip`：死代码守卫（约束「No test-only exports」——本层零新增导出）。
- 静态脚本（static-check / test-execution 等机械步骤）由桌面端执行，agent 内不重复执行。

---

## 待决问题

- **R1 useOpen 回调闭包陈旧的语义定性（非本次阻塞）**：`handleOpenChange` 以 `useCallback(..., [])` 闭包捕获首渲染的 `onOuterOpenChange`（`use-open.ts:18-21`），「中途替换回调后新回调被调」按现状不成立；本变更按现状锁定（断言以稳定 `vi.fn()` 单实例承载，proposal 风险 R1 / D2），该语义是否为缺陷、是否修（如补 deps 或 ref 转发）留待独立 change 判定，本变更 MUST NOT 顺手改源码。
- 无其余未决事项：四被测文件契约已逐行测绘，「公共函数 / API」与「类型定义」签名均核自源码，无 `(待确定)` 项；消费面行为零变化（引用沿用），覆盖 / mutation 达标口径已由 `openspec/config.json` 既有配置锚定（AC-6）。
