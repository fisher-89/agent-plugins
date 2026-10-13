# desktop-standard-components — fill-test-in-desktop-components delta

## ADDED Requirements

### Requirement: useOpen 开合状态归一

`useOpen(outerOpen?, onOuterOpenChange?)` SHALL 归一受控与非受控开合状态并返回 `[open, setOpen]` 二元组：非受控（不传 `outerOpen`）初始态 SHALL 为 `outerOpen ?? false`；`outerOpen` prop 变化 SHALL 同步内部态（`outerOpen === undefined` 或与当前内部态相同 MUST NOT 干预，其余跟随外部 prop）；返回的变更函数 SHALL 先落内部态再以新值回调 `onOuterOpenChange?.(value)`，回调缺省（undefined）MUST NOT 报错。`StandardDialog` / `StandardDrawer` 的开合态 SHALL 经该 hook 归一，MUST NOT 各自另持第二份开合状态。

#### Scenario: 非受控初始闭合与开合回调

- **WHEN** 不传 `outerOpen` 挂载消费方并经返回的变更函数依次置 `true`、`false`
- **THEN** 初始呈闭合；置 `true` 后呈打开且 `onOuterOpenChange` 收到 `true`，置 `false` 后回闭合且收到 `false`

#### Scenario: 受控 prop 变化驱动开合

- **WHEN** 以 `outerOpen=true` 挂载后由外部重渲染为 `outerOpen=false`
- **THEN** 内部态随 prop 同步（打开 → 闭合）；`outerOpen` 与内部态同值的注入不重设内部态（无重复回调、无状态抖动）

#### Scenario: 回调缺省安全

- **WHEN** 不传 `onOuterOpenChange` 时经变更函数置 `true`
- **THEN** 内部态照常翻转为打开，全程无异常抛出

### Requirement: StandardDialog 标准对话框契约

`StandardDialog` SHALL 以 `title` / `content` 呈现对话框主体，并按入参双态收敛 trigger 与 footer：`trigger` 为 `undefined` 时 MUST NOT 渲染触发器节点（开合由 `open` / `onOpenChange` 受控承载），非 undefined 时以 `render={trigger}` 渲染 `DialogTrigger`；footer SHALL 双态——`onSubmit` 为函数时呈「取消」（DialogClose）与「提交」两按钮，否则仅呈「关闭」（DialogClose）。「提交」SHALL 以 `onSubmit(close)` 回调（`close` 为关窗函数），点击提交 MUST NOT 自动关窗，关窗时机由回调内显式调用 `close` 决定；`close` SHALL 关窗并经 `onOpenChange`（若在场）回报 `false`。开合态 SHALL 经 `useOpen` 归一。

#### Scenario: trigger 可选双态

- **WHEN** 分别以带 `trigger` 与不带 `trigger` 挂载 `StandardDialog`
- **THEN** 带 trigger 时触发器在场且点击后开窗呈现 title / content；不带 trigger 时无触发器节点，以 `open=true` 受控直接呈现

#### Scenario: 提交回调收 close 且回调内显式关窗

- **WHEN** 带 `onSubmit` 打开对话框并点击「提交」，回调内暂不调 `close`，随后调用 `close()`
- **THEN** `onSubmit` 恰被调用一次且入参为函数；未调 `close` 前对话框保持打开，调用 `close()` 后内容退场且 `onOpenChange` 收到 `false`

#### Scenario: 无 onSubmit 仅「关闭」

- **WHEN** 不带 `onSubmit` 打开对话框
- **THEN** footer 仅「关闭」按钮（无「提交」/「取消」），点击「关闭」关窗且 `onSubmit` 零调用

#### Scenario: 取消关窗不触发提交

- **WHEN** 带 `onSubmit` 打开对话框并点击「取消」
- **THEN** 对话框关闭且 `onSubmit` 零调用

### Requirement: StandardDrawer 标准抽屉契约

`StandardDrawer` SHALL 以 `title` / `content` 呈现抽屉主体（`swipeDirection="right"` 右侧滑出），footer SHALL 恒为「关闭」（DrawerClose）单按钮；`trigger` 可选语义 SHALL 与 `StandardDialog` 同式（undefined 不渲染触发器，开合由 `open` / `onOpenChange` 受控承载）；`containerClassName` SHALL 透传至 `DrawerContent` 的 `className`，其余未识别 props SHALL 同步透传内容容器；开合态 SHALL 经 `useOpen` 归一。

#### Scenario: trigger 开合与关闭

- **WHEN** 带 `trigger` 挂载 `StandardDrawer` 并点击触发器，再点击「关闭」
- **THEN** 点击触发器开窗呈现 title / content（`onOpenChange` 收到 `true`），点击「关闭」关窗（`onOpenChange` 收到 `false`）

#### Scenario: containerClassName 透传

- **WHEN** 以 `containerClassName="custom-x"` 挂载并打开抽屉
- **THEN** 抽屉内容容器携 `custom-x` 类名（类名为被检属性），title / footer 结构不变

#### Scenario: 无 trigger 受控开合

- **WHEN** 不带 `trigger` 以 `open=true` 受控挂载 `StandardDrawer`，再经外部置 `open=false`
- **THEN** 无触发器节点；`open=true` 时 title / content 在场，置 `false` 后退场，`onOpenChange` 随开合回报

### Requirement: StandardSelect 标准选择器契约

`StandardSelect<T extends string | number>` SHALL 以泛型值语义承载单选：`value: T | null`（null 呈 `placeholder` 占位，缺省「请选择」）；`items`（`{ value: string | number; label: string }[]`）逐项渲染 `SelectItem`；选中 SHALL 经 `onChange(value: T | null)` 回调选中值。`SelectValue` SHALL 透出 `data-testid`（入参 `data-testid`）与 `data-value={value}`（value 为 null 时该属性缺席），`id` / `className` 等未识别 props SHALL 透传 `SelectValue`；`disabled` SHALL 透传底层 `Select`。测试断言 SHALL 以 role / 可及名 / 文本 / data-* 挂钩承载。

#### Scenario: 选项渲染与选择回调

- **WHEN** 以 items 清单挂载并打开选择器、选中一项
- **THEN** 弹层逐项呈现各 item 的 label；选中后 `onChange` 收到该项 `value`（与入参同值同型）

#### Scenario: 值透出与 placeholder 双态

- **WHEN** 分别以 `value=null` 与 `value=<某项>` 挂载
- **THEN** null 时触发器呈占位文本（自定义 `placeholder` 优先、缺省「请选择」）且 `data-value` 属性缺席；非 null 时触发器呈该项 label 且 `data-value` = 选中值

#### Scenario: disabled 透传

- **WHEN** 以 `disabled=true` 挂载并尝试打开选择器
- **THEN** 底层 Select 处于禁用态，弹层不打开且 `onChange` 零调用

### Requirement: standard 组件单元测试纪律

`packages/desktop/src/components/standard/` SHALL 以 colocated 单元测试直接锁定本层契约（`dailog.test.tsx` / `drawer.test.tsx` / `select.test.tsx` / `hooks/use-open.test.ts` 与源文件同目录落位），MUST NOT 仅依赖消费方测试间接覆盖。测试 SHALL 覆盖各契约分支（useOpen 受控 / 非受控 / 回调缺省；StandardDialog trigger 双态 / footer 双态 / 显式关窗；StandardDrawer trigger 双态 / 透传 / 受控开合；StandardSelect 值双态 / 选择回调 / disabled）。查询挂钩 SHALL 为 role / 可及名 / 文本 / `data-testid`（及语义 data-* 属性），MUST NOT 以样式类名作为查询条件（类名仅可作透传断言的被检属性）；base-ui 弹层开合 SHALL 以异步收敛断言（如 `findByRole('listbox')` / `waitFor`）承载。本层零进程边界，测试 MUST NOT mock 被测组件或 `src/components/ui/**` 底座；产品源码（四个被测文件）MUST NOT 因测试可测性改动。

#### Scenario: colocated 测试直达契约分支

- **WHEN** 检索 `packages/desktop/src/components/standard/` 目录测试文件
- **THEN** 四个测试文件在场（dailog / drawer / select / hooks/use-open 各一），受控与非受控开合、trigger 有无、footer 双态、value null 与非 null 分支均有直接断言

#### Scenario: 无样式类名查询挂钩

- **WHEN** 检索该目录 `*.test.tsx` / `*.test.ts` 中的 `querySelector` / `getElementsByClassName` 与类名查询
- **THEN** 零类名查询挂钩（role / 可及名 / 文本 / data-testid 承载；类名仅出现在对已定位元素的属性断言中）

#### Scenario: 弹层异步收敛且产品源码零改动

- **WHEN** 运行该目录测试（`vp test`，含 `--sequence.shuffle`）并审查本变更 diff
- **THEN** 测试全绿且无重试依赖（Select 弹层以 `findByRole('listbox')` 收敛、Dialog / Drawer 关窗以 `waitFor` 收敛）；`dailog.tsx` / `drawer.tsx` / `select.tsx` / `hooks/use-open.ts` 四文件零改动

## Module Contract

### Function: `packages/desktop/src/components/standard/hooks/use-open.ts`

| 函数 | 签名 | 描述 |
|------|------|------|
| `useOpen` | `(outerOpen?: boolean, onOuterOpenChange?: (value: boolean) => void) => [boolean, (value: boolean) => void]` | 受控 / 非受控开合归一：非受控初始 `outerOpen ?? false`；`outerOpen` 变化同步内部态（undefined / 同值不干预）；变更函数先落内部态再回调 `onOuterOpenChange?.(value)`，回调缺省安全 |

### Component: `packages/desktop/src/components/standard/dailog.tsx`

| 组件 | Props | 描述 |
|------|-------|------|
| `StandardDialog` | `title: string`；`content: React.JSX.Element`；`trigger?: React.JSX.Element`；`onSubmit?: (close: () => void) => void`；`open?: boolean`；`onOpenChange?: (open: boolean) => void` | trigger 可选（`trigger !== undefined` 才渲染 `DialogTrigger`，`render={trigger}` 透传）；footer 双态（onSubmit 为函数 →「取消」+「提交」，否则「关闭」）；「提交」→ `onSubmit(close)` 显式关窗（点击本身不关窗）；开合经 `useOpen` |

### Component: `packages/desktop/src/components/standard/drawer.tsx`

| 组件 | Props | 描述 |
|------|-------|------|
| `StandardDrawer` | `title: string`；`content: React.JSX.Element`；`containerClassName?: string`；`trigger?: React.JSX.Element`；`open?: boolean`；`onOpenChange?: (open: boolean) => void` | 右侧滑出（`swipeDirection="right"`）；trigger 可选同 Dialog 式；footer 恒「关闭」单按钮；`containerClassName` 与未识别 props 透传 `DrawerContent`；开合经 `useOpen` |

### Component: `packages/desktop/src/components/standard/select.tsx`

| 组件 | Props | 描述 |
|------|-------|------|
| `StandardSelect<T extends string \| number>` | `value: T \| null`；`onChange: (value: T \| null) => void`；`items: { value: string \| number; label: string }[]`；`placeholder?: string`（缺省「请选择」）；`disabled?: boolean`；`data-testid?` / `id?` / `className?` | items 逐项渲染 `SelectItem`；选中经 `onChange` 收值；`SelectValue` 透出 `data-testid` 与 `data-value={value}`（null 缺席），`id` / `className` 等透传 `SelectValue`；`disabled` 透传底层 `Select` |

### Component: `packages/desktop/src/components/standard/` 测试面

| 文件 | 职责 | 关键契约 |
|------|------|----------|
| `dailog.test.tsx` | StandardDialog 契约单测 | trigger 双态 / footer 双态 / `onSubmit(close)` 显式关窗 / 取消与关闭零 onSubmit；role + 文本挂钩 |
| `drawer.test.tsx` | StandardDrawer 契约单测 | trigger 开合 / 恒「关闭」footer / `containerClassName` 透传（类名作被检属性）/ 受控 `open` / `onOpenChange` |
| `select.test.tsx` | StandardSelect 契约单测 | 选项渲染 / 选择回调 / `data-value` 与 placeholder 双态 / disabled 禁开；`findByRole('listbox')` 异步收敛 |
| `hooks/use-open.test.ts` | useOpen 契约单测 | 非受控初始态 / 开合回调 / 受控 prop 同步 / 回调缺省安全（`renderHook` 承载） |
