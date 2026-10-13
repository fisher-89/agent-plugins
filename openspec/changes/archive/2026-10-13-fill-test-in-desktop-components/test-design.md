# 测试设计: fill-test-in-desktop-components

> **变更**: fill-test-in-desktop-components
> **日期**: 2026-10-13
> **基于**: design.md（test-only 被测文件测绘 + 公共函数 / API 签名）、proposal.md（验收标准 AC-1…AC-7）
> **测试框架**: `vite-plus/test`（`describe` / `it` / `expect` / `vi` / `beforeEach`）+ `@testing-library/react`（`render` / `renderHook` / `fireEvent` / `screen` / `within` / `waitFor` / `act`）；测试文件扩展名 `.test.ts` / `.test.tsx`；断言风格沿 `change-create-dialog.test.tsx` / `agent-run-form.test.tsx` / `use-workspaces.test.ts` 先例
> **测试落位**: colocated（源文件同目录），查询挂钩 = role / 可及名 / 文本 / `data-testid`（MUST NOT 样式类名查询；类名仅作透传断言的被检属性）

---

## 验收范围

| AC ID | 验收条件 | 被测文件或模块 |
|-------|----------|----------------|
| AC-1 | useOpen 单测：非受控初始 `false`、开合回调收新值、受控 `outerOpen` 变化同步（同值注入不重设）、`onOuterOpenChange` 缺省不报错，四面断言在场（由 `hooks/use-open.test.ts` / `renderHook` 承载） | `packages/desktop/src/components/standard/hooks/use-open.ts` |
| AC-2 | StandardDialog 单测：trigger 有无双态；footer 双态（onSubmit 在场 →「取消」+「提交」，缺席 →「关闭」）；`onSubmit` 恰一次且入参为 `close` 函数；点击「提交」不自动关窗、回调内 `close()` 后内容退场；「取消」/「关闭」关窗且 `onSubmit` 零调用（由 `dailog.test.tsx` 承载） | `packages/desktop/src/components/standard/dailog.tsx` |
| AC-3 | StandardDrawer 单测：trigger 开合 + 「关闭」关窗（`onOpenChange` 随开合回报）；footer 恒「关闭」单按钮；`containerClassName` 透传内容容器；无 trigger 时受控 `open` 驱动呈现（由 `drawer.test.tsx` 承载） | `packages/desktop/src/components/standard/drawer.tsx` |
| AC-4 | StandardSelect 单测：弹层选项 = items labels；选中 `onChange` 收选中 value；`value=null` 呈 placeholder 且 `data-value` 缺席 / 非 null 呈 label 且 `data-value` 在场；默认「请选择」与自定义 placeholder；disabled 禁开零回调（由 `select.test.tsx` 承载） | `packages/desktop/src/components/standard/select.tsx` |
| AC-5 | 测试纪律：四个 colocated 测试文件在场且契约分支直达；零样式类名查询挂钩（`querySelector` / `getElementsByClassName` 零命中，类名仅作被检属性）；不 mock 被测组件与 `ui/**` 底座；Select 弹层以 `findByRole('listbox')` 异步收敛 | `packages/desktop/src/components/standard/`（四个源文件与四个 colocated 测试文件的纪律面） |
| AC-6 | 全管线通过：`client:check` 与 `test` 全绿、覆盖率达阈值（lines 90 / branches 85 / functions 90）、mutation score 达 80、`openspec/config.json` 无新增 excludes / 豁免——静态脚本由桌面端执行，agent 内不重复执行 | `packages/desktop`（全测试面，管线级） |
| AC-7 | 版本交付（归档时执行）：`packages/desktop/package.json` `version` 0.5.0 → 0.5.1；`tauri.conf.json` 维持 `../package.json` 引用、`src-tauri/Cargo.toml` 版本不随动、`plugins/dev-team` 保持 2.10.44 零改动——非单元测试面，见「不可测试项」 | `packages/desktop/package.json` |

---

## 单元测试

### packages/desktop/src/components/standard/hooks/use-open.ts -> packages/desktop/src/components/standard/hooks/use-open.test.ts

#### 待测功能

- useOpen(): 受控 / 非受控开合归一 hook——返回 `[open, setOpen]` 二元组；非受控初始 `outerOpen ?? false`；`outerOpen` prop 变化同步内部态（undefined / 同值不干预两分支）；变更函数先落内部态再回调 `onOuterOpenChange?.(value)`，回调缺省安全

<!-- R1（useOpen 回调闭包陈旧）：`handleOpenChange` 以 `useCallback(..., [])` 捕获首渲染回调，「中途替换回调后新回调被调」按现状锁定、MUST NOT 设计为断言；回调断言以稳定 `vi.fn()` 单实例承载。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| useOpen | 正向 | 非受控缺省入参（`outerOpen` 省略）：初始返回 `open=false` | 单次 |
| useOpen | 正向 | 非受控初始 `outerOpen=true`：初始返回 `open=true` | 单次 |
| useOpen | 正向 | 变更函数 `setOpen(true)` → 返回 `open` 同步为 true 且回调收 `true`；`setOpen(false)` 回落 `false`（bool True / False 全档） | 单次 |
| useOpen | 正向 | 受控同步：`rerender` 注入 `outerOpen` false → true，返回 `open` 随 prop 切换 | 迭代 |
| useOpen | 异常 | `onOuterOpenChange` 缺席（undefined，Optional None 档）：`setOpen(true/false)` 不抛错且状态照常切换 | 单次 |
| useOpen | 异常 | `outerOpen` 为 undefined（`rerender` 注入）：不干预内部态（保持 `setOpen` 后的值） | 迭代 |
| useOpen | 异常 | 两可选入参同缺（全缺省调用）：初始 `false`、变更不抛错 | 单次 |
| useOpen | 边界 | `outerOpen` 同值注入（与内部态同为 true 时 `rerender`）：不重设内部态（状态保持 true，无回归） | 迭代 |
| useOpen | 边界 | `outerOpen` 异值注入（内部 true 而 prop=false）：内部态被同步覆盖为 false | 迭代 |
| useOpen | 边界 | `outerOpen` 三态矩阵（true / false / undefined）逐档初始 + 逐档 `rerender` 注入 | 迭代 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `onOuterOpenChange`（被测 API 显式入参） | `vi.fn()` 稳定单实例注入（MUST NOT 中途替换，见 R1） | 回调收值与调用次数断言 |
| React hooks / `useOpen` 内部状态 | 不 mock，`renderHook` + `rerender` + `act` 真实承载 | 全部用例 |
| 进程边界（数据文件 / 网络 / 子进程 / 运行环境） | 无——本 hook 零进程边界，不存在可 mock 主体 | — |

### packages/desktop/src/components/standard/dailog.tsx -> packages/desktop/src/components/standard/dailog.test.tsx

#### 待测功能

- StandardDialog(): 标准对话框——title / content 主体；trigger 可选双态（`trigger !== undefined` 才渲染）；footer 双态（`onSubmit` 为函数 → 「取消」+「提交」，否则仅「关闭」）；「提交」→ `onSubmit(close)` 显式关窗（点击提交本身不关窗）；`open` / `onOpenChange` 经 `useOpen` 归一

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| StandardDialog | 正向 | trigger 在场（JSX 按钮）：点击 trigger 打开弹窗，title 与 content 在场 | 单次 |
| StandardDialog | 正向 | `onSubmit` 在场：footer 呈「取消」+「提交」双按钮 | 单次 |
| StandardDialog | 正向 | 点击「提交」：`onSubmit` 恰收一次且入参为 `close` 函数；点击本身不关窗（content 仍在场）；回调内调 `close()` 后 content 退场 | 单次 |
| StandardDialog | 正向 | 链路（describe 标题注明方向 StandardDialog → useOpen）：非受控 trigger 开、「取消」关，`onOpenChange` 随开合回报 true / false | 迭代 |
| StandardDialog | 异常 | `onSubmit` 缺席（undefined，Optional None 档）：footer 仅「关闭」单按钮，「提交」/「取消」缺席 | 单次 |
| StandardDialog | 异常 | trigger 缺席（undefined，Optional None 档）：不渲染 trigger，受控 `open=true` 驱动弹窗呈现 | 迭代 |
| StandardDialog | 异常 | 「取消」关窗且 `onSubmit` 零调用；「关闭」（`onSubmit` 缺席档）关窗且零调用 | 单次 |
| StandardDialog | 异常 | `onOpenChange` 缺席：开合不抛错（回调缺省安全经 useOpen 透传） | 单次 |
| StandardDialog | 边界 | title 空串 / 特殊字符（`\n` `\0` emoji）/ 超长串（>1000 字符）逐档渲染 title 元素在场 | 单次 |
| StandardDialog | 边界 | 链路（StandardDialog → useOpen）：`rerender` 逐档注入 `open` = false / true / undefined，弹窗呈现随 prop 同步、undefined 不干预 | 迭代 |
| StandardDialog | 边界 | 连续开-关-开（≥3 轮）：footer 双态与 `onSubmit` 计数保持稳定 | 迭代 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `onSubmit` / `onOpenChange`（被测 API 显式入参） | `vi.fn()` 注入（`onSubmit` 以稳定单实例承载 `close` 调用断言） | 提交载荷与开合回报断言 |
| `@/components/ui/{dialog,button}`（vendored 底座） | 不 mock，真实渲染；弹层开合经 `fireEvent` + `waitFor` / `findByRole` 收敛 | 全部用例 |
| `./hooks/use-open`（内部模块） | 不 mock，真实组合（受控链路经 `rerender` 驱动） | 全部用例 |

### packages/desktop/src/components/standard/drawer.tsx -> packages/desktop/src/components/standard/drawer.test.tsx

#### 待测功能

- StandardDrawer(): 标准抽屉——title / content 主体（右侧滑出）；trigger 可选同 Dialog 式；footer 恒「关闭」单按钮；`containerClassName` 透传 `DrawerContent.className`（类名仅作被检属性）；开合经 `useOpen` 归一并随 `onOpenChange` 回报

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| StandardDrawer | 正向 | trigger 在场：点击打开抽屉，title 与 content 在场 | 单次 |
| StandardDrawer | 正向 | footer 恒「关闭」单按钮：无「提交」/「取消」（不论 `onOpenChange` 在场与否） | 单次 |
| StandardDrawer | 正向 | `containerClassName` 在场：内容容器 className 含该类名（对 role / testid 定位元素断言 className，不作查询挂钩） | 单次 |
| StandardDrawer | 正向 | 链路（describe 标题注明方向 StandardDrawer → useOpen）：trigger 开、「关闭」关，`onOpenChange` 随开合回报 true / false | 迭代 |
| StandardDrawer | 异常 | trigger 缺席（undefined，Optional None 档）：不渲染 trigger，受控 `open=true` 驱动呈现 | 迭代 |
| StandardDrawer | 异常 | `onOpenChange` 缺席：开合不抛错 | 单次 |
| StandardDrawer | 异常 | `containerClassName` 缺席（undefined，Optional None 档）：内容容器渲染不抛错、不含自定义类名 | 单次 |
| StandardDrawer | 边界 | `containerClassName` 空串 / 特殊字符 / 超长串（>1000 字符）逐档透传（空串档断言类名属性为空态） | 单次 |
| StandardDrawer | 边界 | title 空串 / 特殊字符（`\n` emoji）/ 超长串逐档渲染 | 单次 |
| StandardDrawer | 边界 | 链路（StandardDrawer → useOpen）：`rerender` 注入 `open` 三态（true / false / undefined）+ 连续开合 ≥3 轮稳定 | 迭代 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `onOpenChange`（被测 API 显式入参） | `vi.fn()` 注入 | 开合回报断言 |
| `@/components/ui/{drawer,button}`（vendored 底座） | 不 mock，真实渲染；弹层开合经 `fireEvent` + `waitFor` 收敛 | 全部用例 |
| `./hooks/use-open`（内部模块） | 不 mock，真实组合（受控链路经 `rerender` 驱动） | 全部用例 |

### packages/desktop/src/components/standard/select.tsx -> packages/desktop/src/components/standard/select.test.tsx

#### 待测功能

- StandardSelect(): 泛型单选（`T extends string | number`，值语义 `T | null`）——items 逐项渲染 `SelectItem`；选中经 `onChange(value)` 抛出；`value` null / 非 null 呈现与 `data-value` 双态；placeholder 双态（缺省「请选择」）；`disabled` 透传禁开；`data-testid` / `id` / `className` 经 `...rest` 透出 `SelectValue`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|----------|
| StandardSelect | 正向 | `fireEvent.mouseDown` 打开弹层（`findByRole('listbox')` 异步收敛）：option 清单 = items labels | 单次 |
| StandardSelect | 正向 | 选中 option（`pointerDown` + `click`）：`onChange` 恰收选中 value | 单次 |
| StandardSelect | 正向 | `value` 非 null（string 档）：触发器呈对应 label，`data-value` 在场且 = 选中值 | 单次 |
| StandardSelect | 正向 | `data-testid` 在场：透出至 `SelectValue` 元素 | 单次 |
| StandardSelect | 异常 | `value=null`（T \| null 的 null 档）：触发器呈 placeholder，`data-value` 属性缺席 | 单次 |
| StandardSelect | 异常 | `disabled=true`：trigger 禁开（mouseDown 后 listbox 不出现），`onChange` 零调用 | 单次 |
| StandardSelect | 异常 | `items=[]`（空清单）：打开弹层零 option，不抛错 | 单次 |
| StandardSelect | 边界 | placeholder 边界：缺省「请选择」/ 自定义 / 空串 / 特殊字符（`\n` `\0` emoji）/ 超长串（>1000 字符）逐档呈现（`value=null` 档承载） | 单次 |
| StandardSelect | 边界 | `value` str 档（T = string）：正常字符串（正常档）+ 空串 `''` / 特殊字符（`\n` `\0` emoji）/ 超长串（>1000 字符）—— `data-value` 字符串化在场且 label 呈现正确；None 档（`null`）由上方 `value=null` 行承载 | 单次 |
| StandardSelect | 边界 | `value` int 档（T = number）：正数（正常档）+ `0` / `-1` / `Number.MAX_SAFE_INTEGER`（MAX_INT）—— `data-value` 字符串化在场且 label 呈现正确 | 单次 |
| StandardSelect | 边界 | items 边界：单元素 / 多元素（含 value 为 number 的项）/ 超大列表（1000 项）逐档 option 数量与 label 断言 | 单次 |
| StandardSelect | 边界 | label 边界：空串 / 特殊字符（emoji、`\n`）/ 超长串（>1000 字符）option 呈现 | 单次 |
| StandardSelect | 边界 | 迭代选中（A → B 连续改选）：`onChange` 逐次收新值、`data-value` 随选中更新 | 迭代 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `onChange`（被测 API 显式入参） | `vi.fn()` 注入 | 选中值与调用次数断言 |
| `@/components/ui/select`（vendored 底座） | 不 mock，真实渲染；弹层以 `fireEvent.mouseDown` 打开、option 经 `pointerDown` + `click` 选中、`findByRole('listbox')` / `waitFor` 异步收敛 | 全部用例 |
| 进程边界（数据文件 / 网络 / 子进程 / 运行环境） | 无——本组件零进程边界，不存在可 mock 主体 | — |

---

## 不可测试项

| 模块 / 项 | 原因 |
|-----------|------|
| `packages/desktop/package.json`（`version`，AC-7） | 版本号写入是归档时执行的配置变更动作，无运行时行为可断言、无单元测试路径；由归档相位核对 0.5.0 → 0.5.1 与 `tauri.conf.json` / `src-tauri/Cargo.toml` 不随动、`plugins/dev-team` 零改动 |
| AC-6 覆盖率 / mutation 达标口径（`openspec/config.json` / `packages/desktop/stryker.config.json` 既有配置，零改动） | 管线级度量（覆盖阈值、mutation score、全绿口径）由桌面端静态脚本（static-check / test-execution）产出，非单元测试面；agent 内不重复执行 |
| `useOpen`「中途替换回调后新回调被调」语义（R1 闭包陈旧） | `useCallback(..., [])` 现状下该断言必然失败于既有行为；test-only 纪律按现状锁定、不设计断言（MUST NOT 顺手改源码），是否为缺陷留待独立 change 判定 |
