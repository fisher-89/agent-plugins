# 设计: tailwind-migration

> **变更**: tailwind-migration
> **日期**: 2026-09-22

---

## 提案与规格同步状态

- `proposal.md` 已通过评审(workflow.json eval attempt 1 verdict pass),`specs/desktop-app-shell/spec.md`(4 个 ADDED requirement)已同步——两者为提案阶段产物,不列入本设计变更清单,不建任务。
- 本设计承接 proposal「待决问题」5 项并全部定夺(见「关键设计决策」),另记录实施期发现的必要补充(`src/main.tsx` 样式挂载点)。

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| Tailwind 样式入口 | `@import "tailwindcss"` + token(`:root` + `@theme inline`)+ `@layer base` 元素级样式;全 app 唯一样式体系 | `packages/desktop/src/styles/global.css`(新建) | tailwindcss、@tailwindcss/vite、@tailwindcss/typography | Tailwind v4 |
| 样式挂载点 | 入口 CSS 的 import 挂载,替代 `index.html` 内联 `<style>` | `packages/desktop/src/main.tsx` | global.css | Vite CSS import |
| shadcn 基建 | CLI 生成配置、`@/*` 路径别名(tsconfig + vite 双处)、`cn()` 类名合并 | `packages/desktop/components.json`(新建)、`tsconfig.json`、`vite.config.ts`、`src/lib/utils.ts`(新建) | clsx、tailwind-merge | shadcn CLI + TS paths + vite resolve.alias |
| shadcn 控件 | Button / Badge / Table / Progress,vendored 内部化(过 fmt/lint/knip 全管线,无 Next 语境残留) | `packages/desktop/src/components/ui/*.tsx`(新建) | class-variance-authority、@radix-ui/react-slot、@radix-ui/react-progress | shadcn + radix |
| 视图层溶解 | 类名翻 utilities、交互控件换装、动态模板串收敛为显式 variant 映射、data-testid 挂钩标注 | `src/App.tsx`、`src/views/**`、`src/renderers/**` | ui 控件、`@/lib/utils` | React 19 + Tailwind utilities |
| markdown prose 通道 | react-markdown 裸元素的后代样式改由 `prose` 承载,消除自定义类后代选择器 | `src/renderers/MarkdownDocRenderer.tsx` | @tailwindcss/typography | typography 插件 |
| 变异守线 | mutate glob 收敛 `src/components/ui/**`、全量实测记录、config.json excludes 口径校准 | `packages/desktop/stryker.config.json`、`openspec/config.json` | — | Stryker |

---

## 关键设计决策

承接 proposal 决策表(全量迁移、两步走、控件直换、hex 直写、vendored 内部化、prose、data-testid、ts-only 否决),以下为本阶段新增定夺:

| # | 问题 | 决策 | 理由 / 备选 |
|---|------|------|------------|
| D1 | CSS 入口命名 | `src/styles/global.css`(沿用 proposal 占位命名) | 语义清晰;备选 `src/index.css`(vite 惯例)无额外收益 |
| D2 | CSS 挂载点 | `main.tsx` 顶部 `import './styles/global.css'`(proposal 实现文件清单外的必要补充) | Vite/React 标准路径,dev/build/HMR 行为一致;`index.html` `<link>` 方案弃用 |
| D3 | Tailwind 接入方式 | `@tailwindcss/vite` 以 `plugins: [react(), tailwindcss()]` 追加;若 vite-plus 三链路 spike 失败,回退 `@tailwindcss/postcss` 路线(结论回写本文档) | vp 是包装层,插入方式以 spike 实测定案。**实现期定案:主路线通过**——`vp dev`(页面 200 + CSS 经转换管线)/`vp build`(dist 产物含编译后 CSS)/`vp test`(148 用例)三链路实测定案,PostCSS 备选不启用 |
| D4 | pass/fail/warn 语义色形态 | 自定义 token:`:root` 定义 + `@theme inline` 映射为 `--color-pass` 等,Tailwind 自动生成 `bg-pass-bg` / `text-pass` 类 utilities | 单一来源、可被 Badge variant 直接引用;备选「语义工具类直写」将 hex 散落多处(否决) |
| D5 | Stryker `src/components/ui/**` 处置 | 排除(mutate glob 追加 `!src/components/ui/**`),理由见「变异守线处置」 | 与 `openspec/config.json` desktop 套件已预置 excludes 口径一致 |
| D6 | 生成件超 `max-lines-per-function` 拆解 | lint 按函数计数,shadcn 四件(Button/Badge/Progress/Table)均为小函数组件,预期不超线;若超,按子组件边界拆文件消化 | 内部化纪律,不设 lint 豁免 |
| D7 | `change-row` 归宿 | 换 `Button` + className 行级 utilities(不再保留原生 button 元素直接供养) | AC-4「按钮由 Button 渲染」按全称口径执行;行级观感变化属允许范围 |
| D8 | `.interrupted-list` 处置 | 死 CSS(无任何标记引用),随 `<style>` 块消失;中断留档区维持 div 布局翻 utilities,不换 Table | 该区非表格语义;AC-4 仅列 `.filelog-table` |
| D9 | prose 参数 | `prose prose-sm max-w-none` 起步,截图对比后以 `prose-*` modifier 微调 | prose 默认 65ch 宽度上限会改变布局,`max-w-none` 必须;观感微调在 AC-9 允许口径内 |
| D10 | 徽标变体色落地 | 代际/类别变体(inv0/inv1/inv2/kind)直接用 Tailwind 调色板 utilities——原 hex 本就取自调色板(如 `#dbeafe`=blue-100、`#1d4ed8`=blue-700),等值零漂移;pass/fail/active 经 token utilities | AC-8 只约束 CSS 入口 token;为一次性变体色新增 token 无收益 |
| D11 | 生成件变体集 | Badge/Button 生成件的 shadcn 默认变体集替换为本 app 实际变体集,未用变体与未用导出(`badgeVariants`/`TableCaption`/`TableFooter` 等)按 knip 纪律删减 | vendored 内部化,不留死代码 |

---

## 变更清单

<!-- 以文件为入口逐层展开,实现阶段以此清单为边界;测试文件改写(9 个 *.test.tsx)由 test 阶段承载,不列入本清单,挂钩契约见「测试挂钩约定」。 -->

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/src/styles/global.css` | Tailwind 样式入口:`@import "tailwindcss"`、`@plugin "@tailwindcss/typography"`、`:root` + `@theme inline` token(hex 直写)、`@layer base` 元素级样式(body/button 三态/#root/`color-scheme: light`) |
| `packages/desktop/components.json` | shadcn CLI 配置:style、`rsc: false`、`tsx: true`、`tailwind.css` 指向 global.css、`cssVariables: true`、aliases 全套指向 `@/*` |
| `packages/desktop/src/lib/utils.ts` | `cn()` = clsx + tailwind-merge |
| `packages/desktop/src/components/ui/button.tsx` | shadcn Button(`asChild` 经 radix Slot),去 Next 语境残留 |
| `packages/desktop/src/components/ui/badge.tsx` | shadcn Badge,variants 收敛为 `inv0/inv1/inv2/pass/fail/kind/active` |
| `packages/desktop/src/components/ui/table.tsx` | shadcn Table 组件族(Table/TableHeader/TableBody/TableRow/TableHead/TableCell) |
| `packages/desktop/src/components/ui/progress.tsx` | shadcn Progress(radix 封装,value 承载百分比) |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/package.json` | 新增依赖(见「依赖」节) | Tailwind 系进 devDependencies(编译期),clsx/radix 系进 dependencies(进 bundle) |
| `packages/desktop/vite.config.ts` | plugins 追加 tailwind 插件(D3 定案方式);`resolve.alias` 配 `@/*` → `src/*` | lint/fmt 的 ignorePatterns 不动(AC-5 无豁免) |
| `packages/desktop/tsconfig.json` | `compilerOptions.paths` 配 `"@/*": ["./src/*"]` | 与 vite alias 双处同步 |
| `packages/desktop/src/main.tsx` | 顶部导入 `./styles/global.css` | proposal 清单外必要补充(D2):入口 CSS 必须有 JS 挂载点 |
| `packages/desktop/index.html` | 步骤①:仅移除已迁入 `@layer base` 的元素级规则(`:root`/`*`/`body`/`button` 三态/`#root`),类 CSS 原样保留;步骤②:移除整个 `<style>` 块 | `*{box-sizing}` 与 preflight 重复,随迁弃置(等价覆盖) |
| `packages/desktop/src/App.tsx` | `.app-header`/`.app-main`/`.spacer`/`.app-version`/`.error-note` 翻 utilities;按钮换 Button;testid 标注 | header 信息架构与 DOM 结构不变(AC-9) |
| `packages/desktop/src/views/WelcomeView.tsx` | `.screen-center`/`.app-main`/`.panel`/`.muted`/`.error-note` 翻 utilities;按钮换 Button;testid 标注 | 欢迎态 DOM 结构不变 |
| `packages/desktop/src/views/changes/ChangeListView.tsx` | `.change-row`(换 Button + 行级 utilities,D7)/`.panel`/`.name`/`.created` 翻 utilities;`badge-in${inventory}` 模板串收敛为 `INVENTORY_VARIANT: Record<Inventory, ...>` 显式映射换 Badge;testid 标注 | Tailwind 无法静态识别模板串类名(spec 硬性要求) |
| `packages/desktop/src/views/changes/ChangeDetailView.tsx` | `.detail-header`/`.station`/`.station-head`/`.attempt`/`.attempt-meta`/`.report`/`.backtrack`/`.checklist`/`.item-row`/`.evidence`/`.warn-note`/`.panel`/`.muted` 翻 utilities;verdict/inv/active 徽标换 Badge;`.filelog-table` 换 Table;按钮换 Button;testid 标注 | 全库最大视图(~250 行),溶解主战场 |
| `packages/desktop/src/views/changes/ChangeView.tsx` | 核对项:现状无 className,预计零改动 | proposal 列入实现文件,保留核对条目以满足清单覆盖 |
| `packages/desktop/src/renderers/ArtifactView.tsx` | `.artifact-card`/`> header`/`h3`/`.artifact-version` 翻 utilities;`.badge-kind` 换 Badge;testid 标注 | 信封路由组件外壳 |
| `packages/desktop/src/renderers/Fallback.tsx` | `.fallback-text` 翻 utilities;testid 标注 | 兜底渲染,永不白屏硬要求不变 |
| `packages/desktop/src/renderers/EvalChecklistRenderer.tsx` | `.attempt-meta`/`.checklist`/`.item-row`/`.evidence`/`.fallback-text` 翻 utilities;verdict 徽标换 Badge;testid 标注 | payload 守卫逻辑零改动 |
| `packages/desktop/src/renderers/TasksProgressRenderer.tsx` | `.progress-track`/`.progress-fill` 换 Progress(`value` 承载百分比,消除全库唯一内联 `style={{ width }}`);`.progress-labels` 翻 utilities;testid 标注 | 百分比计算逻辑零改动 |
| `packages/desktop/src/renderers/MarkdownDocRenderer.tsx` | `.markdown-doc` 换 `prose` 承载(D9);testid 标注 | payload 守卫与 react-markdown 管线零改动 |
| `packages/desktop/stryker.config.json` | `mutate` 追加 `"!src/components/ui/**"` | 处置与理由成对(见「变异守线处置」,AC-10) |
| `openspec/config.json` | desktop 套件 `excludes: ["src/components/ui/**/*"]` 已预置暂存,随本 change 与 stryker glob 口径校准确认 | 不新增条目,仅校准确认 |

### 删除文件

<!-- 无整文件删除;index.html 内联 <style> 块(约 450 行)属文件内修改,见「修改文件」表。 -->

### 公共函数 / API

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `cn` | `src/lib/utils.ts` | 新增 | `export function cn(...inputs: ClassValue[]): string` | `twMerge(clsx(inputs))`,Tailwind 类冲突消解;`ClassValue` 经 inline type import 自 clsx |
| `Button` | `src/components/ui/button.tsx` | 新增 | `function Button({ className, variant, size, asChild = false, ...props }: React.ComponentProps<'button'> & VariantProps<typeof buttonVariants> & { asChild?: boolean }): React.JSX.Element` | shadcn 按钮;`asChild` 经 `@radix-ui/react-slot` 渲染子元素 |
| `Badge` | `src/components/ui/badge.tsx` | 新增 | `function Badge({ className, variant, ...props }: React.ComponentProps<'span'> & VariantProps<typeof badgeVariants>): React.JSX.Element` | variants 见 D10/D11;视图经 `INVENTORY_VARIANT` 映射传 variant |
| `Table` | `src/components/ui/table.tsx` | 新增 | `function Table({ className, ...props }: React.ComponentProps<'table'>): React.JSX.Element` | 表容器,自带横向滚动包裹(`overflow-x-auto`) |
| `TableHeader` / `TableBody` / `TableRow` / `TableHead` / `TableCell` | `src/components/ui/table.tsx` | 新增 | 各自 `function X({ className, ...props }: React.ComponentProps<'thead' | 'tbody' | 'tr' | 'th' | 'td'>): React.JSX.Element` | 行/单元格族,边框与内边距 utilities 内置 |
| `Progress` | `src/components/ui/progress.tsx` | 新增 | `function Progress({ className, value, ...props }: React.ComponentProps<typeof ProgressPrimitive.Root>): React.JSX.Element` | radix Root/Track/Indicator 封装,`value: number | null` 0-100 |

<!-- 未用导出(buttonVariants/badgeVariants 若无外部引用、TableCaption/TableFooter)按 D11/knip 纪律删减,不进入导出面。 -->

### 类型定义

<!-- 无新增导出类型:variant 联合以字面量内联于视图映射常量(如 Record<Inventory, 'inv0' | 'inv1' | 'inv2'>);Inventory 沿用 src/types/dto.ts 既有定义,该文件零改动。 -->

### 配置

| 配置键 | 所在文件 | 类型 | 默认值 | 说明 |
|--------|----------|------|--------|------|
| `compilerOptions.paths["@/*"]` | `tsconfig.json` | 新增 | `["./src/*"]` | `@/*` 别名,与 vite 双处同步 |
| `resolve.alias["@"]` | `vite.config.ts` | 新增 | 指向 `src/` 绝对路径(`fileURLToPath(new URL('./src', import.meta.url))`) | 同上,ui/** import 解析 |
| `plugins` | `vite.config.ts` | 修改 | `[react(), tailwindcss()]` | 插入方式以 spike 定案(D3) |
| `tailwind.css` / `tailwind.cssVariables` / `aliases.*` | `components.json` | 新增 | `src/styles/global.css` / `true` / 全套 `@/*` | shadcn CLI 生成配置 |
| `mutate` | `stryker.config.json` | 修改 | 追加 `"!src/components/ui/**"` | D5 排除处置 |
| `tests[].excludes`(desktop 套件) | `openspec/config.json` | 修改(校准确认) | `["src/components/ui/**/*"]` | 已预置暂存,与 stryker glob 同一口径确认 |

---

## 样式溶解映射

### token 映射(`:root` + `@theme inline`,值直写现有 hex,AC-8)

| 原变量 | 原值(hex) | 新 token | 主要用途 |
|--------|------------|----------|----------|
| `--bg` | `#f5f6f8` | `--background` | body/progress 轨道/pre 底/th 底/fallback 底 |
| `--panel` | `#ffffff` | `--card` | 面板/按钮/header 底色 |
| `--border` | `#d9dee5` | `--border` | 边框 |
| `--text` | `#1f2430` | `--foreground` | 正文 |
| `--muted` | `#6b7280` | `--muted-foreground` | 次要文字 |
| `--accent` | `#2563eb` | `--primary` | 主色(hover/进度/active 徽标) |
| (新增) | `#ffffff` | `--primary-foreground` | primary 上的前景(badge-active 白字) |
| `--pass` / `--pass-bg` | `#15803d` / `#dcfce7` | `--color-pass` / `--color-pass-bg` | 语义色(D4) |
| `--fail` / `--fail-bg` | `#b91c1c` / `#fee2e2` | `--color-fail` / `--color-fail-bg` | 语义色(D4) |
| `--warn` / `--warn-bg` | `#92400e` / `#fef3c7` | `--color-warn` / `--color-warn-bg` | 语义色(D4) |

light/dark 双槽:`:root` 仅填 light 值,dark 覆写块以空注释占位不填值(spec 约定)。

### 类名归宿

| 原类名 / 元素 | 处置 | 归宿 |
|---------------|------|------|
| `:root color-scheme`、`body`、`button`(含 `:hover`/`:disabled`)、`#root` | 步骤① 1:1 平移 | global.css `@layer base`(`*{box-sizing}` 随 preflight 免除) |
| button(全局供养)、`.badge-*` 全变体、`.filelog-table`、`.progress-track/fill` | 控件替换 | Button / Badge / Table / Progress(AC-4) |
| `.panel` `.muted` `.error-note` `.warn-note` `.app-header` `.app-main` `.screen-center` `.spacer` `.app-version` `.detail-header` `.station` `.station-head` `.attempt` `.attempt-meta` `.report` `.backtrack` `.checklist`(含 `li`/`.evidence`)`.item-row` `.artifact-card` `.artifact-version` `.fallback-text` `.progress-labels` `.change-row` `.name` `.created` | 翻 utilities | 各落点文件(见「修改文件」表);换算等值约定见 AC-9 行 |
| `` `badge-in${inventory}` ``(ChangeListView / ChangeDetailView 动态模板串) | 显式映射 | `const INVENTORY_VARIANT: Record<Inventory, 'inv0' \| 'inv1' \| 'inv2'>` + Badge variant |
| `.markdown-doc`(含全部后代选择器) | prose 接管 | `prose prose-sm max-w-none`(D9),样式表中不再存在 `.markdown-doc` 后代选择器(AC-7) |
| `.workspace`(`.app-header .workspace`)、`.interrupted-list` | 死 CSS 确认 | 无标记引用,随 `<style>` 块消失(D8) |

### utilities 等值换算约定(AC-9 依据)

- 间距/圆角/字号按 Tailwind 默认刻度等值换算:`padding: 16px` → `px-4 py-4`、`gap: 12px` → `gap-3`、`border-radius: 8px` → `rounded-lg`、`12px` 字号 → `text-xs`;非刻度值用任意值:`text-[13px]` / `text-[15px]` / `text-[17px]` / `max-w-[1100px]` / `border-l-[3px]` / `max-h-[320px]`。
- 布局锚点零变化:`.app-main` → `flex-1 px-4 py-4 max-w-[1100px] w-full mx-auto`(1100px 居中保持);`.app-header`/`#root` flex 结构逐属性对应。
- Badge/Table/Progress 换装件使用 shadcn 自带刻度,观感变化属 AC-9 允许口径(替换控件除外条款)。

---

## 测试挂钩约定(data-testid)

测试文件的改写由 test 阶段承载;本节为源侧挂钩契约(spec「data-testid 测试挂钩」requirement 的设计落点),实现任务在 Phase D 落标注。

- 命名约定:`data-testid` 值用语义角色名,与样式名解耦;嵌套/计数断言经「容器 testid + 子元素 testid」组织(spec 硬性要求)。
- 清零口径(AC-6):`grep -E "querySelector|getElementsByClassName"` 于 9 个 `*.test.tsx` 全零——含 6 处元素标签查询(`option`/`strong`/`tbody tr`/`td`/`button`),一并改写为 RTL 语义查询或 testid 查询,以最严口径消除验收争议。

主要挂钩分配:

| 挂钩 testid | 落点 | 承接的原类名查询 |
|-------------|------|------------------|
| `error-note` | App / WelcomeView / ChangeListView / ChangeDetailView 错误容器 | `.error-note`(存在性与文本) |
| `detail-note` | DetailFallback 中性提示容器(原 `.muted` 分支) | 加载中/未找到态区分 |
| `artifact-card` / `artifact-kind` / `artifact-version` | ArtifactView 外壳/kind 徽标/版本 | `.artifact-card` 计数、`.badge-kind`、`.artifact-version` |
| `checklist` + `checklist-verdict`(逐项) | Attempt 与 EvalChecklistRenderer 清单容器/逐项徽标 | `.checklist .badge-pass/fail` 嵌套计数 |
| `attempt-meta` + `attempt-verdict` | Attempt 元信息容器/verdict 徽标 | `.attempt-meta .badge` 嵌套查询 |
| `filelog-table` | filelog Table 外壳 | `.filelog-table` 存在性、`tbody tr`/`td` 计数 |
| `progress` | TasksProgressRenderer 的 Progress | `.progress-track`/`.progress-fill` 结构断言 |
| `markdown-root` | MarkdownDocRenderer 根 div | `.markdown-doc` 存在性;表格/代码块结构改经 role/text 或新挂钩 |
| `backtrack` / `warn-note` / `change-row` | 对应元素 | 同名类查询 |

---

## 变异守线处置

- **定夺(D5)**:`stryker.config.json` 的 `mutate` 排除 `src/components/ui/**`,且仅此一处文件级排除。
- **理由(与处置成对,AC-10)**:mutate 是测试驱动的评分器,无对应测试投入的生成控件代码纳入 mutate 只产等价 mutant 噪声(探索期估 shadcn 件 ~300-500 mutant 几乎杀不死)。这不与 vendored 内部化纪律冲突:fmt/lint/knip 是静态管线,无需测试投入即可合规,故 ui/** 照常过全管线;mutation 分数需要逐 mutant 对应断言,口径与 `openspec/config.json` desktop 套件已预置 excludes 一致。
- **代价与缓解**:生成件内部逻辑缺陷无变异覆盖;控件为标准 shadcn 实现,业务映射逻辑(`INVENTORY_VARIANT`、verdict 三元式)留在视图层,纳入 mutate。
- **实测义务**:迁移完成后全量 `pnpm -C packages/desktop run mutation-test`,分数 ≥ 50(break 线)记录在案。**实现期状态**:config 改动(`!src/components/ui/**`)已落地;实测在实现阶段不可执行——Stryker 初始校验要求全测试套件绿,而 9 个类名耦合测试文件须待 test 阶段改写(与本阶段同一原子 commit)方可转绿,且无 coverage analysis 时逐 mutant 全量跑套件属离线量级。实测移交 test-execution/acceptance 阶段执行,第二档 StringLiteral 兜底以届时数据评估。
- **第二档兜底**:排除 ui/** 后若实测仍跌破 config.json 套件阈值 80(探索期估 85% → ~75%),方评估全局 `mutator.excludedMutations: ["StringLiteral"]`——代价为连带放过 hooks IPC 契约字符串(`'list_workspaces'` 等)与健康 testid mutant 的真覆盖;Stryker 无按文件的 mutator 选择,取舍须回写本文档记录。break 50 为硬守线,不作任何刷分排除。

---

## 数据模型

无新增/修改数据模型。payload 契约不变:`EvalChecklistPayload` / `TasksProgressPayload` / `MarkdownDocPayload` 为 renderer 模块内部接口,形状零改动;`src/types/dto.ts` 零改动(proposal「不要修改」);Tauri Rust 后端与全部持久化零涉及。

---

<!-- 路由/API 设计:本变更无 HTTP API;Tauri IPC 契约不变(proposal「不要修改」明确 hooks 取数与 payload 契约不动),此节省略。 -->

---

## 依赖

### 运行时依赖(dependencies,进 bundle)

- `clsx` — `cn()` 类名组合
- `tailwind-merge` — `cn()` Tailwind 冲突消解
- `class-variance-authority` — Button/Badge variant(cva)
- `@radix-ui/react-slot` — Button `asChild`
- `@radix-ui/react-progress` — Progress primitive

### 构建/测试依赖(devDependencies,编译期)

- `tailwindcss` — CSS 框架本体(v4,`@theme`/`@layer`/preflight)
- `@tailwindcss/vite` — vite 插件接入(D3;备选 `@tailwindcss/postcss`)
- `@tailwindcss/typography` — `prose` 承载 markdown(AC-7)

无其他新增;knip 的 `ignoreDependencies`(`@vitest/coverage-v8`)与 vite-plus/knip/Stryker 既有版本不动。

---

## 验收标准对齐

| AC | 设计落点 |
|----|----------|
| AC-1 | 依赖节(devDependencies)+ D3 接入决策 + Phase A 三链路 spike 任务;`vp build` 产物含编译后 CSS 为 spike 验收项 |
| AC-2 | global.css 设计(`@layer base` 1:1 平移 + token)+ Phase B 步骤① 任务(index.html 移除元素级规则)+ 截图对比验收 |
| AC-3 | 步骤② 单 commit 约定(tasks Phase D)+ index.html `<style>` 全移 + 类名归宿表全量 utilities 化 |
| AC-4 | ui 四件生成件(公共函数表)+ 类名归宿表控件替换行 + D7/D8;原 CSS 定义随 `<style>` 块消失 |
| AC-5 | vendored 内部化纪律(D6/D11)+ Phase C 全管线任务;vite.config.ts lint/fmt 与 knip.json 均不加 ignorePatterns |
| AC-6 | 测试挂钩约定与分配表 + 清零口径(含 6 处标签查询一并清零)+ Phase D 源侧标注任务(测试改写在 test 阶段,同 commit) |
| AC-7 | D9 prose 参数 + markdown prose 通道组件;`.markdown-doc` 后代选择器不复存在 |
| AC-8 | token 映射表(hex 直写,`--primary: #2563eb`,不引 zinc 默认主题,dark 槽留空) |
| AC-9 | utilities 等值换算约定(1100px/间距/字号/圆角)+ Phase D 截图对比口径(替换控件观感变化除外) |
| AC-10 | 变异守线处置节(排除 + 理由成对、实测 ≥ 50 记录、第二档兜底条件)+ Phase E 任务 |

---

## 待决问题

- ~~prose 对原 markdown 观感的最终 modifier 集~~ **实现期已定初版**:`prose prose-sm max-w-none overflow-x-auto` + `prose-th/prose-pre/prose-code` 系 modifier(表格表头底色/代码底色取 token 背景,td 边框经 `[--tw-prose-td-borders:var(--border)]`)。最终微调仍依赖真实环境截图对比,验收阶段可再调。
- ~~实测 mutation 分数与阈值 80 差距处置~~ **移交**:见「变异守线处置·实测义务」实现期状态——9 个测试文件转绿(同原子 commit)后实测,数据落地再评估第二档兜底。
- ~~vite-plus 三链路 spike 定案~~ **已闭环**:D3 主路线(`@tailwindcss/vite`)通过,见 D3 行实现期定案。

## 实现期回写(必要偏差与补充,均不触豁免纪律)

| # | 事项 | 说明 |
|---|------|------|
| R1 | `@types/node` 进 devDependencies | 依赖清单外必要补充:`vite.config.ts` resolve.alias 按 D3/配置表采用 `fileURLToPath(new URL('./src', import.meta.url))`,vp lint(typeCheck)要求 node 类型;类型期依赖,不进 bundle |
| R2 | `knip.json` `project` glob 追加 `css` | 非 ignorePatterns/豁免:knip 本不解析 CSS 入口,`src/**/*.{ts,tsx,css}` 扩围后 knip 跟随 global.css 的 `@import 'tailwindcss'` 与 `@plugin '@tailwindcss/typography'` 识别依赖,unused devDeps 归零(恰为 knip 自身 hint 指引方向) |
| R3 | `body` 补 `line-height: normal` | 原 CSS 未设 line-height(浏览器默认 normal),preflight 在 html 层注入 1.5;为达成步骤①「视觉零变」在 `@layer base` body 还原原始基线,utilities 照常可覆写 |
| R4 | App 顶栏 `select` 补 utilities | 原样式无 select 规则(裸浏览器默认),preflight 重置其边框/底色;以 `rounded-md border border-border bg-card px-2 py-1` 还原原生观感,顺带析出 `WorkspaceSelect` 子组件(max-lines-per-function, D6) |
| R5 | `ChangeListView` 析出 `ArchiveGroups` 子组件 | utilities 化后主函数超 50 行,按 D6 子组件边界拆解,无 lint 豁免 |
| R6 | Step①→② 过渡别名 | 步骤① 期间旧类 CSS 仍引用 `--bg/--panel/--text/--muted/--accent`,global.css `:root` 以 `var()` 别名桥接(单一来源);步骤② 移除 `<style>` 块时随行删除,已不存在的过渡产物 |

