## ADDED Requirements

### Requirement: Tailwind v4 单一样式体系

desktop 前端样式 SHALL 以 Tailwind v4 为唯一样式体系:

- 样式入口 SHALL 为正式 CSS 文件(`@import "tailwindcss"`);`index.html` 内联 `<style>` MUST 移除,MUST NOT 长期并存第二套手写 CSS 体系
- 迁移 SHALL 两步走:步骤① 元素级全局样式(`body` / `button` / `#root`)与设计 token 翻入 `@layer base`,与旧类 CSS 共存且视觉零变;步骤② 单 commit 完成类名全量 utilities 化
- 设计 token SHALL 沿 shadcn 结构命名(`--background` / `--card` / `--border` / `--foreground` / `--muted-foreground` / `--primary` 等),值 SHALL 直写现有 hex(如 `--accent #2563eb` → `--primary`),SHALL NOT 换用 shadcn 默认主题色;`--pass/--fail/--warn` 语义色 SHALL 以自定义 `@theme` token 或语义工具类承载(形态 design 定夺);light/dark 双槽 SHALL 预留但仅填 light 值
- 布局形态 MUST NOT 变化:main 区 max-width 1100px 居中、header 信息架构、欢迎态/壳态 DOM 结构(控件替换除外)保持不变
- markdown 产物样式 SHALL 经 `@tailwindcss/typography`(`prose`)承载:react-markdown 输出裸元素无类名钩子,MUST NOT 依赖自定义类后代选择器(如 `.markdown-doc pre`)

#### Scenario: 步骤① 共存且视觉零变

- **WHEN** 步骤① 落地
- **THEN** CSS 入口以 `@layer base` 承载元素级样式与 token,旧类 CSS 照常生效
- **AND** `vp build` 成功,前后截图对比布局、配色、结构一致

#### Scenario: 步骤② 内联样式清零

- **WHEN** 步骤② 合入
- **THEN** `index.html` 无 `<style>` 块,组件类名为 utilities
- **AND** `.panel` / `.badge-*` / `.attempt` 等旧自定义样式类不复存在

#### Scenario: token 保持现有配色身份

- **WHEN** 审查 CSS 入口的 token 定义
- **THEN** token 值与原 `index.html` 的 hex 一致(如 `--primary: #2563eb`)
- **AND** 未引入 shadcn 默认 zinc 主题色,深色模式槽位留空未填

#### Scenario: markdown 经 prose 承载

- **WHEN** 渲染含表格、代码块的 markdown 产物
- **THEN** 表格边框、代码底色等样式经 `prose` 生效
- **AND** 样式表中无 `.markdown-doc` 后代选择器

### Requirement: shadcn 控件替换与 vendored 内部化

交互控件 SHALL 直接替换为 shadcn 组件并接受观感变化:全局 `button` 样式供养的按钮 → `Button`;`.badge-*` 全部变体(`.badge-inv0` / `.badge-inv1` / `.badge-inv2`(现状经模板串 `badge-in${inventory}` 动态拼出)/ `.badge-pass` / `.badge-fail` / `.badge-kind` / `.badge-active`)→ `Badge`,动态拼接 SHALL 收敛为显式 variant 映射(Tailwind 无法静态识别模板串类名);`.filelog-table` / `.interrupted-list` → `Table`;`.progress-track/fill` → `Progress`。纯布局容器类(`.panel` / `.muted` / `.attempt` / `.station` / `.artifact-card` 等)SHALL 翻为 utilities,MUST NOT 为其另造组件。

`src/components/ui/**` SHALL 视为内部组件而非外来 vendored 物:照常过 fmt / lint / knip 全管线,SHALL NOT 为其新增 ignorePatterns 或管线豁免;knip 报出的未用导出 SHALL 删减;超 `max-lines-per-function` 的生成件 SHALL 拆解消化。不考虑 shadcn 上游升级,MUST NOT 保留 Next 语境残留(如 cookie 持久化)。

#### Scenario: 交互控件组件化

- **WHEN** 审查视图与 renderer 代码
- **THEN** 按钮、徽标、文件日志表、任务进度条分别由 `Button` / `Badge` / `Table` / `Progress` 渲染
- **AND** 原 `button` 全局供养样式与 `.badge-*` / `.filelog-table` / `.progress-track` CSS 定义不再存在

#### Scenario: 全管线无豁免

- **WHEN** 运行 `pnpm -C packages/desktop run client:check`(vp check --fix + knip)
- **THEN** fmt / lint / knip 覆盖 `src/components/ui/**` 且通过
- **AND** vite.config.ts 与 knip.json 无新增 ignorePatterns 或豁免条目

#### Scenario: 超线拆解消化

- **WHEN** 某生成组件超出 `max-lines-per-function: 50`
- **THEN** 按内部组件拆文件消化,而非以 lint 规则豁免

### Requirement: data-testid 测试挂钩

组件测试 MUST NOT 以样式类名作为查询挂钩(Tailwind 为唯一样式体系后类名不再是稳定契约)。测试 SHALL 以 `data-testid`(或语义 data-* 属性)标注挂钩元素;嵌套与计数断言(如 badge 计数、`.attempt-meta .badge-pass` 嵌套查询)SHALL 经 testid 容器组织。类名全量溶解与 testid 改造 SHALL 同 commit 落地,保证任一 commit 状态测试全绿。

#### Scenario: 类名查询清零

- **WHEN** 检索全部 `*.test.tsx` 中的 `querySelector` / `getElementsByClassName`
- **THEN** 无样式类名查询(迁移前 56 处、9 文件清零)

#### Scenario: 迁移原子性

- **WHEN** 步骤② 合入
- **THEN** 同一 commit 内 testid 改造完成,`vp test` 全绿

#### Scenario: testid mutant 不稀释分数

- **WHEN** mutation 报告中存在 `data-testid` 字符串 mutant
- **THEN** 测试断言(按 testid 查询)可将其杀死,不构成分数稀释来源

### Requirement: 变异测试守线与生成件 mutate 处置

迁移完成后 SHALL 全量运行一次 `pnpm -C packages/desktop run mutation-test`,实测分数 SHALL 不低于 `stryker.config.json` break 线 50。`src/components/ui/**` 是否纳入 mutate glob SHALL 由 design 定夺并记录理由:维持纳入则补对应测试;选择排除则以「无对应测试投入的组件代码纳入 mutate 只产噪声」为由(与 `openspec/config.json` desktop 套件已预置的 `excludes: ["src/components/ui/**/*"]` 口径一致)。全局 `mutator.excludedMutations: ["StringLiteral"]` MUST NOT 作为首选手段——仅作漂移不可接受时的第二档兜底,且 MUST 记入 design(代价:连带放过 IPC 契约字符串等健康 mutant)。

#### Scenario: 实测守线

- **WHEN** 迁移完成后运行全量 mutation test
- **THEN** 实测分数 ≥ 50 且记录在案,供 design 定夺 ui/** 处置

#### Scenario: 处置与理由成对

- **WHEN** 审查 `stryker.config.json` 的 mutate glob 与 design 文档
- **THEN** `src/components/ui/**` 的纳入/排除处置均有成对理由,无无理由排除

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src/styles/global.css`(新建,命名 design 定) | Tailwind 样式入口 | `@import "tailwindcss"`;`@theme` token(hex 直写、shadcn 结构命名);`@layer base` 元素级样式 |
| `packages/desktop/src/lib/utils.ts`(新建) | 类名合并 | `cn()` = clsx + tailwind-merge |
| `packages/desktop/src/components/ui/**`(新建) | shadcn 内部化控件 | Button / Badge / Table / Progress;过 fmt/lint/knip 全管线无豁免;无 Next 语境残留 |
| `packages/desktop/components.json`(新建) | shadcn 生成配置 | alias `@/*`;内部化纪律适用 |
| `@/*` 路径别名(tsconfig + vite 双处) | ui/** import 解析 | 双处同步配置 |
| `src/renderers/MarkdownDocRenderer.tsx` | markdown 渲染 | `prose` 接管后代样式;无自定义类后代选择器 |
| `src/renderers/TasksProgressRenderer.tsx` | 任务进度渲染 | Progress 组件 value 承载百分比;无内联 `style={{ width }}` |
| `*.test.tsx`(9 文件) | 测试挂钩 | data-testid;无样式类名查询;与步骤② 同 commit |
| `stryker.config.json` | mutate 范围守线 | ui/** 处置 design 定夺有据;break 50 守线;StringLiteral 全局排除仅兜底 |
