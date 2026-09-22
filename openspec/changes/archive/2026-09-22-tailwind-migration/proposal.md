# 提案: tailwind-migration

> **变更**: tailwind-migration
> **日期**: 2026-09-22
> **状态**: proposal(待评审)

---

## 问题

desktop 前端(`packages/desktop`)零 UI 基建:无 Tailwind、无 radix、无 cva、无路径别名。全部样式是 `packages/desktop/index.html` 内联 `<style>` 的约 450 行 plain CSS(含 `:root` 变量与 `body` / `button` / `#root` 元素级全局样式),没有正式 CSS 入口。

由此产生四类代价:

1. **无体系**:样式无法组合复用,类名靠手写约定;姊妹 change `workspace-sidebar`(sidebar 壳与 workspace 面板)依赖本 change 先落地 UI 地基,否则新 UI 只能继续堆内联 CSS。
2. **测试与样式耦合**:组件测试大量以样式类名为查询挂钩(grep 实测 `querySelector|getElementsByClassName` 56 处、9 个文件,依赖 `.error-note`、`.badge-pass/.badge-fail`、`.checklist`、`.artifact-card`、`.progress-fill`、`.markdown-doc`、`.filelog-table` 等做结构与计数断言)。样式类名事实上成了测试契约,任何样式重构都会破坏测试。
3. **markdown 渲染断层**:`MarkdownDocRenderer` 只包一层 `.markdown-doc` div,react-markdown 输出裸元素,现有样式全靠 `.markdown-doc pre` / `.markdown-doc table` 后代选择器。一旦类名体系更换,裸元素失去全部样式钩子——而 markdown 是本 app 最重的渲染面。
4. **工程管线撞点潜伏**:`max-lines-per-function: 50`(vite.config.ts lint 节)、knip unused-export、fmt 规整三条管线,对将要引入的 shadcn 生成组件天然会报警,需要事先定下处置纪律而非临时豁免。

此外,Stryker 变异测试 mutate 全部非测试 ts/tsx(break 线 50):Tailwind 化后组件内大量 className 字符串字面量会稀释分数,需要守线策略。

---

## 提案

零 UI 基建的 desktop 前端落 **Tailwind v4 + shadcn token 地基**,`index.html` 内联样式全量溶解为 utilities,现有配色身份经 shadcn 结构 token 保留(`--accent #2563eb` → `--primary`)。**布局形态零变化**——sidebar 壳、header 瘦身、错误 toast 属姊妹 change,不在本 change。

实施按两步走(方案乙),因为元素级全局样式在 Tailwind 接入瞬间即被 preflight 重置,不存在逐文件无痛窗口:

- **步骤① 地基 + 元素级样式共存**:Tailwind v4 + `@tailwindcss/vite` 经 vite-plus 三链路(dev/build/test)spike 验证后接入;建 `components.json`、`@/*` 路径别名(tsconfig + vite 双处)、`cn()`(`clsx` + `tailwind-merge`)、token(`@theme` + `:root`,值直写现有 hex);元素级样式(`body` / `button` / `#root`)翻入 `@layer base`,与旧类 CSS 共存,**视觉零变**,独立可验收(截图对比)。
- **步骤② 单 commit 全量溶解**:类名全量翻 utilities + 交互控件 shadcn 化 + prose 接管 markdown + 类名测试查询改 `data-testid`,同一 commit 落地保证验收原子性(任一 commit 状态测试全绿)。

交互控件直接换 shadcn 组件、接受观感变化:`button` 全局样式供养的按钮 → `Button`、`.badge-*` 全部变体(代际 `inv0/inv1/inv2`、`pass`/`fail`、`kind`、`active`)→ `Badge`、`.filelog-table` / `.interrupted-list` → `Table`、`.progress-track/fill` → `Progress`;纯布局容器类(`.panel` / `.muted` / `.attempt` / `.station` / `.artifact-card` 等)翻 utilities,不另造组件。现状的动态类名模板串(`badge-in${inventory}`)Tailwind 无法静态识别,迁移时 SHALL 收敛为显式 variant 映射。shadcn 生成件 vendored 内部化:视为内部组件,照常过 fmt / lint / knip 全管线,不设 ignorePatterns,未用导出删减、超线拆解。

---

## 能力

### 新增能力

- 无——本 change 不新建 capability,UI 基建约定增补进既有 `desktop-app-shell`(前端视图约定本就归其约束)。

### 修改的能力

- **desktop-app-shell** — 增补 UI 基建约定(ADDED requirements,不改既有 requirement):① Tailwind v4 单一样式体系(含 token 映射与 prose 接管 markdown);② shadcn 控件替换与 vendored 内部化纪律;③ data-testid 测试挂钩;④ 变异测试守线与生成件 mutate 处置。布局与场景语义改写属姊妹 change,本 change 不触碰。
- `desktop-workspace-store` — 零改动(已确认)。

---

## 变更范围

### 实现文件

- `packages/desktop/package.json` — 新增依赖:`tailwindcss`、`@tailwindcss/vite`、`clsx`、`tailwind-merge`、`class-variance-authority`、`@radix-ui/react-slot`(Button)、`@radix-ui/react-progress`(Progress)、`@tailwindcss/typography`
- `packages/desktop/vite.config.ts` — plugins 增 `@tailwindcss/vite`(插入方式以三链路 spike 结果为准)
- `packages/desktop/tsconfig.json` — `@/*` 路径别名(与 vite 配置双处同步)
- `packages/desktop/index.html` — 步骤① 保留内联样式共存;步骤② 移除 `<style>` 块
- `packages/desktop/src/styles/global.css`(新建,命名 design 定)— `@import "tailwindcss"`、`@theme` token、`@layer base` 元素级样式
- `packages/desktop/components.json`(新建)— shadcn 配置
- `packages/desktop/src/lib/utils.ts`(新建)— `cn()`
- `packages/desktop/src/components/ui/**`(新建)— Button / Badge / Table / Progress 生成件,按内部组件改造(去 Next 语境残留、过全管线)
- `packages/desktop/src/App.tsx` — header / error-note / UpdateIndicator 区类名翻 utilities,按钮换 Button
- `packages/desktop/src/views/WelcomeView.tsx` — screen-center 翻 utilities
- `packages/desktop/src/views/changes/ChangeListView.tsx` — change-row / badge 翻 utilities + Badge
- `packages/desktop/src/views/changes/ChangeDetailView.tsx` — station / attempt / checklist 翻 utilities,filelog / interrupted 列表换 Table
- `packages/desktop/src/views/changes/ChangeView.tsx` — 布局容器翻 utilities
- `packages/desktop/src/renderers/ArtifactView.tsx` / `Fallback.tsx` / `EvalChecklistRenderer.tsx` — 类名翻 utilities
- `packages/desktop/src/renderers/TasksProgressRenderer.tsx` — progress-track/fill 换 Progress(唯一内联 `style={{ width }}` 随之消失)
- `packages/desktop/src/renderers/MarkdownDocRenderer.tsx` — `prose` 接管后代样式
- `packages/desktop/stryker.config.json` — `src/components/ui/**` 的 mutate 处置(design 定夺,见待决问题)
- `openspec/config.json` — desktop 套件 `excludes: ["src/components/ui/**/*"]` 已预置暂存,随本 change 一并校准确认

### 测试文件

- 9 个既有测试文件改造(类名查询 → data-testid):`src/App.test.tsx`、`src/views/changes/ChangeDetailView.test.tsx`、`src/views/changes/ChangeListView.test.tsx`、`src/renderers/EvalChecklistRenderer.test.tsx`、`src/renderers/ArtifactView.test.tsx`、`src/renderers/MarkdownDocRenderer.test.tsx`、`src/renderers/TasksProgressRenderer.test.tsx`、`src/__tests__/ipc_pipeline.test.tsx`、`src/__tests__/workspace_restore.test.tsx`
- 若 design 选择「ui/** 纳入 mutate 并补测试」,则为 `src/components/ui/**` 新增测试;选择排除则无新增

### 删除文件

- 无整文件删除;`packages/desktop/index.html` 内联 `<style>` 块(约 450 行)在步骤② 移除

### 不要修改

- `packages/desktop/src-tauri/**` — Rust 后端零涉及
- `packages/desktop/src/hooks/**` 取数逻辑与 `src/types/dto.ts` — 显式刷新取数模型、payload 契约不变
- `packages/desktop/src/renderers/registry.ts` — renderer 注册表机制不变
- 布局形态:main 区 max-width 1100px 居中、header 信息架构、欢迎态/壳态 DOM 结构(控件替换除外)
- 深色模式 — light/dark 双槽只预留,不填 dark 值,不引入主题切换
- 姊妹 change 范围:sidebar 壳、header 瘦身、错误 toast、workspace 面板布局

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | Tailwind 接入 vite-plus | `vite.config.ts` 含 `@tailwindcss/vite`;`vp dev` / `vp build` / `vp test` 三链路可用(build 产物含编译后 CSS) |
| AC-2 | 步骤① 元素级样式入 @layer base | CSS 入口含 `@layer base` 的 body/button/#root 样式与 token;与旧类 CSS 共存;前后截图对比布局/配色/结构一致 |
| AC-3 | 步骤② 全量溶解 | `index.html` 无 `<style>` 块;`className` 中无 `.panel` / `.badge-*` 等旧自定义类;全量 utilities |
| AC-4 | shadcn 控件替换 | `src/components/ui/` 存在 Button / Badge / Table / Progress 且被视图引用;`.badge-*` / `.filelog-table` / `.progress-track` 等 CSS 定义不复存在 |
| AC-5 | vendored 全管线无豁免 | `pnpm -C packages/desktop run client:check`(vp check --fix + knip)在含 ui/** 的全 src 上通过;vite.config.ts / knip.json 无新增 ignorePatterns 或豁免 |
| AC-6 | data-testid 挂钩 | 9 个测试文件中无类名查询(现 56 处清零);`vp test` 全绿 |
| AC-7 | prose 接管 markdown | `MarkdownDocRenderer` 以 `prose` 承载;其测试经新挂钩断言表格/代码块结构,无 `.markdown-doc` 后代选择器 |
| AC-8 | token 配色身份保留 | CSS 入口 token 值与原 `index.html` hex 一致(如 `--primary: #2563eb`),未换 shadcn 默认主题色 |
| AC-9 | 布局零变化 | main 区 max-width 1100px 居中、欢迎态/壳态 DOM 结构不变(控件观感变化除外);步骤② 截图对比除替换控件外一致 |
| AC-10 | mutation 实测守线 | 全量 `pnpm -C packages/desktop run mutation-test` 实测分数 ≥ 50(break 线);`stryker.config.json` 的 ui/** 处置与 design 理由成对出现 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| vite-plus 与 `@tailwindcss/vite` 兼容性未知(vp 是包装层) | 地基接入失败,后续全部阻塞 | 中 | Phase 1 首项做三链路 spike;失败回退 `@tailwindcss/postcss` 路线 |
| Stryker 分数稀释(className 字面量 mutant 杀不死) | 实测分数下滑(估 85% → ~75%):不触本地 break 50,但会跌破 config.json 套件 mutation 阈值 80 | 高 | design 定夺本地 mutate glob 排除 ui/**(config.json 已预置 excludes 口径一致);data-testid 方案保证 testid mutant 可杀;漂移超预期的全局 StringLiteral 排除仅作第二档兜底且须记入 design |
| preflight 接入即重置元素级样式 | 步骤① 视觉回归(button 无边框等) | 中 | 元素级样式翻入 `@layer base`;每步截图对比验收,发现问题即回退该 commit |
| 生成件撞 lint / knip 管线(max-lines-per-function 50、unused-export) | `client:check` 失败 | 高 | 按内部组件纪律拆解文件、删减未用导出,不设豁免(#13) |
| 步骤② 原子 commit 内视觉变化混合(布局平移 vs 控件换肤) | 截图对比口径不清,验收争议 | 中 | 验收口径分层:布局/配色/结构零变化(AC-9),替换控件观感变化明示允许(#15) |
| 嵌套/计数断言无法改写为 role/text 查询(如 badge-pass 计数与 checklist 文本撞) | 部分断言只能靠 testid 容器,语义查询覆盖不全 | 低 | data-testid 为既定挂钩约定(非权宜),容器级计数断言照常表达 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| 迁移范围 | 全量迁移,单一 Tailwind 体系 | 双轨并存长期成本高,且类名测试耦合必须一次解开 | 渐进双轨并存(否决) |
| 中间态策略 | 两步走:①元素级入 @layer base 共存(视觉零变);②单 commit 全量溶解 | 元素级全局样式在 Tailwind 接入瞬间被 preflight 重置,不存在逐文件无痛窗口 | 逐文件平移(已否决) |
| 交互控件 | 直接换 shadcn 件,接受视觉变化 | 手工 CSS 仿制 shadcn 观感无收益 | CSS 1:1 仿制(否决) |
| token 值格式 | shadcn 结构命名,值直写现有 hex | hex → oklch 有精度漂移且无收益 | oklch 全量转换(否决) |
| shadcn 生成件 | vendored 内部化:过 fmt/lint/knip 全管线,不豁免 | 无升级诉求,外来物豁免与项目纪律冲突;超线拆解、未用导出删减 | 按外来物 ignorePatterns 豁免(否决) |
| markdown 样式 | `@tailwindcss/typography` prose | react-markdown 输出裸元素无类名钩子;markdown 是最重渲染面 | components 映射逐元素配 className(更繁琐) |
| 测试挂钩 | data-testid(或语义 data-* 属性) | 类名迁移后消失;部分计数/嵌套断言无法改写为 role/text 查询(文本撞车) | 纯语义查询(部分断言不可表达) |
| Stryker ts-only | 否决 | desktop 测试几乎纯组件测试,hooks mutant 也靠渲染间接杀死;砍 tsx 即视图逻辑零覆盖,coverage theater | ts-only mutate(否决) |

### 待决问题

- vite-plus 三链路 spike 结果与 `@tailwindcss/vite` 插入方式(失败则 PostCSS 备选)
- Stryker:`src/components/ui/**` 维持排除(理由改写为「无对应测试投入的组件代码纳入 mutate 只产噪声」,与 config.json 已预置 excludes 口径一致)vs 纳入并补测试——design 定夺
- 生成件超 `max-lines-per-function` 的拆解方式(按内部组件拆文件 vs 其他)
- `--pass/--fail/--warn` 语义色的落地形态:自定义 `@theme` token vs 语义工具类直写
- CSS 入口文件命名与目录(`src/styles/global.css` 为占位)

---
