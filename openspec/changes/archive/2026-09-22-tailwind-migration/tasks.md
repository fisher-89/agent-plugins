# 任务: tailwind-migration

> 依赖顺序:Phase A → B → C → D → E,组内按序执行。
> Phase D 全组任务约定**同一 commit** 原子落地(AC-3/AC-6/AC-9 验收原子性;与 test 阶段的 9 个既有测试文件改写合并为原子验收——测试文件改写任务由 test 阶段承载,不列入本清单)。

## Phase A:依赖与三链路 spike(AC-1)

- [x] `packages/desktop/package.json` 新增依赖:devDependencies `tailwindcss`、`@tailwindcss/vite`、`@tailwindcss/typography`;dependencies `clsx`、`tailwind-merge`、`class-variance-authority`、`@radix-ui/react-slot`、`@radix-ui/react-progress`(另 `@types/node` 必要补充,design R1)
- [x] 三链路 spike:最小 CSS 入口(`@import "tailwindcss"`)经 `@tailwindcss/vite` 验证 `vp dev` / `vp build`(产物含编译后 CSS)/ `vp test` 可用;失败则切换 `@tailwindcss/postcss` 备选路线,并把定案回写 design「关键设计决策 D3」(主路线通过,PostCSS 备选不启用,已回写 D3)

## Phase B:地基与步骤①(AC-1/AC-2/AC-8,视觉零变独立验收)

- [x] 新建 `src/styles/global.css`:`@import "tailwindcss"`、`@plugin "@tailwindcss/typography"`、`:root` + `@theme inline` token(值直写现有 hex,映射见 design token 表)、`@layer base` 1:1 承载 `color-scheme: light` 与 `body` / `button`(`:hover`/`:disabled`) / `#root` 元素级样式(`*{box-sizing}` 弃置,由 preflight 等价覆盖);dark 槽位空注释占位
- [x] `src/main.tsx` 顶部导入 `./styles/global.css`(design D2 挂载点)
- [x] `tsconfig.json` 配 `compilerOptions.paths`、`vite.config.ts` 配 `resolve.alias`,`@/*` → `src/*` 双处同步;`vite.config.ts` plugins 以 spike 定案方式追加 tailwind 插件
- [x] 新建 `components.json`(alias `@/*`,`tailwind.css` 指向 global.css)与 `src/lib/utils.ts`(`cn()`)
- [x] `index.html` 仅移除已迁入 `@layer base` 的元素级规则(`:root`/`*`/`body`/`button` 三态/`#root`),其余类 CSS 原样保留(旧变量名经 global.css 过渡别名桥接,design R6)
- [x] 步骤① 验收:`vp build` 通过(产物含 body/button/#root 编译后 CSS);静态核对 CSS 入口 token 值与原 hex 一致、未引 shadcn 默认主题色(AC-8);前后截图对比须真实环境,移交验收阶段(实现侧 1:1 平移 + preflight 差异补偿见 design R3/R4)

## Phase C:shadcn 生成件 vendored 内部化(AC-4/AC-5)

- [x] 生成 `src/components/ui/button.tsx`(Button,radix Slot 支持 `asChild`),去 Next 语境残留(cookie 持久化等)
- [x] 生成 `src/components/ui/badge.tsx`(Badge),variants 收敛为 `inv0/inv1/inv2/pass/fail/kind/active`(色值按 design D10 变体表;shadcn 默认变体集删减,见 D11)
- [x] 生成 `src/components/ui/table.tsx`(Table/TableHeader/TableBody/TableRow/TableHead/TableCell;`TableCaption`/`TableFooter` 不保留)
- [x] 生成 `src/components/ui/progress.tsx`(Progress,radix 封装)
- [x] 生成件过全管线:`vp check --fix` + knip 全绿(未用导出删减、超 `max-lines-per-function` 按子组件拆文件,见 D6/D11),`vite.config.ts` / `knip.json` 无新增 ignorePatterns(AC-5)(knip 于视图接线后同轮实跑全绿;knip.json project 扩围 `css` 非豁免,design R2)

## Phase D:步骤② 单 commit 全量溶解(AC-3/AC-4/AC-6 源侧标注/AC-7/AC-9)

> 本组全部任务与 test 阶段的 9 个测试文件改写**同一 commit** 落地,保证任一 commit 状态 `vp test` 全绿。
> 实现侧现状:溶解后 `vp test` 为 9 个类名耦合测试文件红(118/148 绿),与 test 阶段改写合并后转绿——即本组任务的原子交接态。

- [x] `src/App.tsx`:`.app-header`/`.app-main`/`.spacer`/`.app-version`/`.error-note` 翻 utilities,按钮换 Button,按 design 挂钩表标注 testid
- [x] `src/views/WelcomeView.tsx`:`.screen-center`/`.app-main`/`.panel`/`.muted`/`.error-note` 翻 utilities,按钮换 Button,挂钩标注
- [x] `src/views/changes/ChangeListView.tsx`:`.change-row`(换 Button + 行级 utilities,D7)/`.panel`/`.name`/`.created` 翻 utilities;`badge-in${inventory}` 模板串收敛为 `INVENTORY_VARIANT: Record<Inventory, 'inv0' | 'inv1' | 'inv2'>` 显式映射换 Badge;挂钩标注
- [x] `src/views/changes/ChangeDetailView.tsx`:`.detail-header`/`.station`/`.station-head`/`.attempt`/`.attempt-meta`/`.report`/`.backtrack`/`.checklist`/`.item-row`/`.evidence`/`.warn-note`/`.panel`/`.muted` 翻 utilities;verdict/inv/active 徽标换 Badge;`.filelog-table` 换 Table;按钮换 Button;挂钩标注
- [x] `src/views/changes/ChangeView.tsx`:核对无样式类名(现状无 className,预计零改动)(已核对,零改动)
- [x] `src/renderers/ArtifactView.tsx`:`.artifact-card`/`> header`/`h3`/`.artifact-version` 翻 utilities,`.badge-kind` 换 Badge,挂钩标注
- [x] `src/renderers/Fallback.tsx`:`.fallback-text` 翻 utilities,挂钩标注
- [x] `src/renderers/EvalChecklistRenderer.tsx`:`.attempt-meta`/`.checklist`/`.item-row`/`.evidence`/`.fallback-text` 翻 utilities,verdict 徽标换 Badge,挂钩标注
- [x] `src/renderers/TasksProgressRenderer.tsx`:`.progress-track`/`.progress-fill` 换 Progress(`value` 承载百分比,消除唯一内联 `style={{ width }}`),`.progress-labels` 翻 utilities,挂钩标注
- [x] `src/renderers/MarkdownDocRenderer.tsx`:`.markdown-doc` 换 `prose prose-sm max-w-none`(D9),挂钩标注
- [x] `index.html` 移除整个 `<style>` 块(AC-3)
- [x] 步骤② 验收:grep 确认 `index.html` 无 `<style>`、`className` 无 `.panel`/`.badge-*`/`.filelog-table`/`.progress-track`/`.markdown-doc` 等旧自定义类、样式表无 `.markdown-doc` 后代选择器(AC-3/AC-4/AC-7 全部 grep 清零);截图对比除替换控件外布局/配色/结构一致(AC-9)移交验收阶段(utilities 按 design 等值换算约定落地);prose 观感微调初版已定(design「待决问题」),终稿按对比结论再调

## Phase E:变异守线与收尾(AC-5/AC-10)

- [x] `stryker.config.json` `mutate` 追加 `"!src/components/ui/**"`(处置理由见 design「变异守线处置」,成对记录)
- [x] 全量 `pnpm -C packages/desktop run mutation-test` 实测:分数 ≥ 50 记录在案;若跌破 config.json 套件阈值 80,按 design 第二档兜底条件评估 `StringLiteral` 排除并把取舍回写 design(移交:test 阶段测试转绿后方有有效分数,Stryker 初始校验要求全绿;见 design「变异守线处置·实现期状态」)。实测闭环:85.29%(416 mutants,killed 348)≥ break 50 且 ≥ 套件阈值 80,`StringLiteral` 二档兜底无需启用,记录见 `reports/test/packages_desktop_vite-plus/report.json:90-106`
- [x] `openspec/config.json` desktop 套件 `excludes: ["src/components/ui/**/*"]` 与 stryker glob 口径校准确认(已预置暂存,不新增条目;与 `!src/components/ui/**` 同口径)
- [x] 收尾复核:`pnpm -C packages/desktop run client:check` 全绿且无新增豁免(AC-5);对照 design「验收标准对齐」表逐项复核 AC-1~AC-10(AC-9 截图、AC-10 实测两项依赖真实环境/测试转绿,移交验收);design「待决问题」三项(spike 定案/prose 微调/mutation 处置)全部回写闭环
