# desktop-standard-components Specification（delta: provider-effort-knob）

## Purpose

本 delta 给标准组件层补一枚**可选可填**控件：`StandardCombobox`（输入框可自由键入，下拉给出建议清单；建议是建议、不是取值约束）。它是 provider 表单「三档模型 + 两档 effort」的共同载体——那些字段既要能选目录条目（省去手敲与查文档），又必须保留自由输入（目录只是建议面，自定义值必须能存）。契约要点：值语义用**空串表留空**（与 `StandardSelect` 的 `null` 表未选择分开）、`items` 为空即退化为纯输入框、`data-testid` / `data-value` 落输入框。层内既有组件（Dialog / Drawer / Select / Pagination / Tooltip）零改动；测试纪律随新增文件扩一条分支。

## ADDED Requirements

### Requirement: StandardCombobox 标准组合框契约

`StandardCombobox` SHALL 提供「可选可填」的单值输入控件，落在 `packages/desktop/src/components/standard/combobox.tsx`，底座 SHALL 为 `packages/desktop/src/components/ui/combobox.tsx`（`@base-ui/react/combobox` 的原语层包装），与 `StandardSelect` / `StandardDialog` 同层同风格：

- **取值语义**：`value: string`；`onChange: (value: string) => void`。输入框**任意键入** SHALL 逐键回调输入原文（受控），MUST NOT 因取值不在 `items` 中而拦截或回退——「建议」与「约束」在本组件内必须分开。
- **建议清单**：`items: string[]` SHALL 逐项渲染为可选条目；选中条目 SHALL 把该原文写入输入框并回调该值。`items` 为空数组时 SHALL 退化为**纯输入框**形态（不渲染建议弹层、不报错、不占位提示建议）——这是「目录取数失败即无建议」的降级落点。
- **留空语义**：清空输入 SHALL 以 `onChange('')` 回调（**空串 = 留空**，与 store 侧「`efforts` 空串 = 留空 = 不发」同型）；MUST NOT 用 `null` 承载留空（`null` 在 `StandardSelect` 表「未选择」，两控件的空态语义 MUST NOT 混用）。
- **值透出**：`data-testid` 与 `data-value={value}` SHALL 落在输入框元素上（`value` 为当前输入原文，`''` 时 `data-value` 呈空串而非缺席——留空本身是合法状态）；`id` / `className` / `placeholder` SHALL 透传输入框；`placeholder` 缺省「可输入或选择」。
- **禁用**：`disabled` SHALL 同时禁用输入框与建议弹层（不可键入、不可展开、`onChange` 零调用）。
- **弹层开合**：建议弹层的开合条件（聚焦 / 输入触发）由底座决定，本层 MUST NOT 另持一份开关状态；测试断言 SHALL 以 role / 文本 / `data-testid` 承载，弹层开合 SHALL 以异步收敛断言承载（`findByRole('listbox')` / `waitFor`），MUST NOT 以样式类名为查询条件。

#### Scenario: 自由输入与值透出

- **WHEN** 以 `items = ["deepseek-reasoner", "deepseek-chat"]` 挂载并逐字符键入 `my-custom-model`（不在清单内）
- **THEN** `onChange` 依次收到键入原文、最终为 `my-custom-model`（零拦截零回退）；输入框 `data-value` 等于当前原文，`data-testid` 落在输入框上

#### Scenario: 选中条目回填

- **WHEN** 展开建议清单并选中 `deepseek-reasoner`
- **THEN** 输入框呈该原文且 `onChange` 收到 `deepseek-reasoner`；再次展开时清单项与 `items` 逐项一致

#### Scenario: 清空即留空

- **WHEN** 输入框原有值 `low`，用户清空输入
- **THEN** `onChange` 收到空串（`data-value` 为空串），组件不报错、不自动回退到某项建议

#### Scenario: 空清单退化为纯输入框

- **WHEN** 以 `items = []` 挂载并聚焦 / 键入
- **THEN** 无建议弹层渲染（`listbox` 不在文档中），输入与回调照常工作

#### Scenario: disabled 全量禁用

- **WHEN** 以 `disabled=true` 挂载并尝试键入、聚焦展开建议
- **THEN** 输入框禁用、弹层不展开、`onChange` 零调用

## MODIFIED Requirements

### Requirement: standard 组件单元测试纪律

`packages/desktop/src/components/standard/` SHALL 以 colocated 单元测试直接锁定本层契约（`dailog.test.tsx` / `drawer.test.tsx` / `select.test.tsx` / `pagination.test.tsx` / `combobox.test.tsx` / `hooks/use-open.test.ts` 与源文件同目录落位），MUST NOT 仅依赖消费方测试间接覆盖。测试 SHALL 覆盖各契约分支（useOpen 受控 / 非受控 / 回调缺省；StandardDialog trigger 双态 / footer 双态 / 显式关窗；StandardDrawer trigger 双态 / 透传 / 受控开合；StandardSelect 值双态 / 选择回调 / disabled；StandardCombobox 自由输入 / 选中条目回填 / 清空即留空 / 空清单退化 / disabled；StandardPagination 页码渲染 / 翻页回调 / 越界与 disabled 边界 / 摘要行双态）。查询挂钩 SHALL 为 role / 可及名 / 文本 / `data-testid`（及语义 data-* 属性），MUST NOT 以样式类名作为查询条件（类名仅可作透传断言的被检属性）；base-ui 弹层开合 SHALL 以异步收敛断言（如 `findByRole('listbox')` / `waitFor`）承载。本层零进程边界，测试 MUST NOT mock 被测组件或 `src/components/ui/**` 底座；产品源码（六个被测文件）MUST NOT 因测试可测性改动。

#### Scenario: colocated 测试直达契约分支

- **WHEN** 检索 `packages/desktop/src/components/standard/` 目录测试文件
- **THEN** 六个测试文件在场（dailog / drawer / select / pagination / combobox / hooks/use-open 各一），受控与非受控开合、trigger 有无、footer 双态、value null 与非 null 分支、组合框的自由输入 / 选中条目 / 清空 / 空清单 / disabled 分支均有直接断言

#### Scenario: 无样式类名查询挂钩

- **WHEN** 检索该目录 `*.test.tsx` / `*.test.ts` 中的 `querySelector` / `getElementsByClassName` 与类名查询
- **THEN** 零类名查询挂钩（role / 可及名 / 文本 / data-testid 承载；类名仅出现在对已定位元素的属性断言中）

#### Scenario: 弹层异步收敛且产品源码零改动

- **WHEN** 运行该目录测试（`vp test`，含 `--sequence.shuffle`）并审查本变更 diff
- **THEN** 测试全绿且无重试依赖（Select / Combobox 弹层以 `findByRole('listbox')` 收敛、Dialog / Drawer 关窗以 `waitFor` 收敛）；`dailog.tsx` / `drawer.tsx` / `select.tsx` / `pagination.tsx` / `hooks/use-open.ts` 五个既有被测文件零改动（新增文件只有 `combobox.tsx` 与 `combobox.test.tsx`）

## Module Contract

### Component: `packages/desktop/src/components/standard/combobox.tsx`（新）

| 项 | Props | 描述 |
|------|------|------|
| `StandardCombobox` | `value: string`；`onChange: (value: string) => void`；`items: string[]`；`placeholder?: string`（缺省「可输入或选择」）；`disabled?: boolean`；`data-testid?` / `id?` / `className?` | 「可选可填」单值控件：任意键入逐键回调原文（建议不构成约束）；选中条目把原文写入并回调；清空回调空串（**空串 = 留空**，不用 `null`）；`items` 空 ⇒ 退化纯输入框；`data-testid` / `data-value` 落输入框，`id` / `className` / `placeholder` 透传；`disabled` 同时禁输入与弹层；弹层开合由底座管理，本层零第二份开关状态 |

### Component: `packages/desktop/src/components/ui/combobox.tsx`（新）

| 项 | 形态 | 描述 |
|------|------|------|
| 原语层包装 | `@base-ui/react/combobox`（`Root` / `Trigger` / `Input` / `Popup` / `Item` 等）薄包装 | 与 `ui/select.tsx` 同层同风格：只组装原语与样式，承载零业务语义（无建议来源、无留空策略）；供标准层 `StandardCombobox` 单独消费，产品视图 MUST NOT 直引本文件 |
