# 探索笔记：desktop 前端全量 Tailwind v4 迁移（shadcn 地基）

> 日期：2026-09-22 ｜ 状态：探索完成，待起 proposal
> 拟议 change 名：`tailwind-migration`
> 姊妹篇：`workspace-sidebar.md`（sidebar 壳与 workspace 面板，**依赖本 change 地基先行**）。
> 本文件由 `shadcn-sidebar-workspace.md` 拆分而来（第三轮审计拍板 #16）。

## 目标

零 UI 基建的 desktop 前端落 Tailwind v4 + shadcn token 地基，`index.html` 内 ~450 行
内联 `<style>` 全量溶解为 utilities，类名测试查询全量改 `data-testid`。
**布局形态零变化**——sidebar 壳、header 瘦身、错误 toast 属姊妹 change。

## 已锁定决策

| # | 问题 | 决定 |
|---|------|------|
| 2 | 迁移范围 | **全量迁移**：现有 CSS 全部迁入 Tailwind v4 + shadcn token，单一体系 |
| 12 | 中间态策略 | **方案乙两步走**：① 地基 + 元素级样式（`body` / `button` / `#root`）翻入 `@layer base`，与旧类 CSS 共存（视觉零变）；② 单 commit 类名全量溶解。验收 = 各步前后截图对比 |
| 15 | 「1:1 平移」口径 | **交互控件直接换 shadcn 件，接受视觉变化**：button（全局样式供养）→ Button、`.badge-*` × 5 → Badge、`.filelog-table` → Table、`.progress-track/fill` → Progress；纯布局容器类（`.panel` / `.muted` / `.attempt` / `.station` 等）翻 utilities |
| 18 | token 值格式 | 变量命名沿 shadcn 结构，**值直写现有 hex**（hex → oklch 有精度漂移且无收益）；light/dark 双槽预留（只填 light 值），深色模式不在范围 |
| 13 | 生成件纪律 | **vendored 内部化**：不考虑 shadcn 升级，`src/components/ui/**` 视为内部组件——自由动刀（去 Next 语境残留如 cookie 持久化）、正常过 fmt/lint/knip 管线，不按外来物豁免：超线按项目纪律拆解消化，knip 未用导出直接删减。`vp check --fix` 的 fmt 规整照跑，不设 ignorePatterns |

### 补充默认假设（proposal 阶段可否决）

- 配色保留现有身份（`--accent #2563eb` → `--primary`），不换 shadcn 默认 zinc 主题
- main 区 max-width 1100px 居中维持，仅样式体系切换，不动布局
- 欢迎态 / 壳态的 DOM 结构本 change 不动（控件替换除外，见 #15）

## 现状盘点（关键事实）

| 事实 | 位置 | 影响 |
|------|------|------|
| 零 UI 基建：无 Tailwind、无 radix、无 cva、无路径别名 | `packages/desktop/package.json` / `tsconfig.json` | 本 change 是「第一个组件 = 整套地基」 |
| 全部样式是 index.html 内联 `<style>`（~450 行 plain CSS + `:root` 变量，含 `body` / `button` / `#root` 元素级全局样式） | `packages/desktop/index.html` | 需迁出为正式 CSS 入口；元素级样式与 preflight 无法共存是 #12 的根因 |
| 测试大量依赖类名查询（~50 处、9 文件） | `packages/desktop/src/**/*.test.tsx` | 全量迁移后类名消失（发现 ①） |
| Stryker mutate 全部非测试 ts/tsx，break 线 50 | `packages/desktop/stryker.config.json` | className 字面量稀释（发现 ③） |
| lint（max-lines-per-function 50）/ fmt / knip 三管线 | `packages/desktop/vite.config.ts` / `knip.json` | 撞点见发现 ④ |

## 依赖面

```
Tailwind v4 ── tailwindcss + @tailwindcss/vite（经 vite-plus，需 spike）
 └── tokens：@theme + :root 变量（现有 hex 直写，#18）
cn() ────────── clsx + tailwind-merge
cva ─────────── class-variance-authority
shadcn 控件（#15）── Button(@radix-ui/react-slot) / Badge / Table / Progress(@radix-ui/react-progress)
typography ─── @tailwindcss/typography（prose，发现 ②）
```

外加基建件：`components.json`、`@/*` 路径别名（tsconfig + vite 双处）、`lib/utils.ts`、
正式 CSS 入口文件（`@import "tailwindcss"`，index.html 内联 style 承载不了）。

### Token 映射草案（#18：值直写 hex）

```
--bg        → --background
--panel     → --card
--border    → --border
--text      → --foreground
--muted     → --muted-foreground
--accent    → --primary
--pass/--pass-bg、--fail/--fail-bg、--warn/--warn-bg
           → shadcn 无直接对应：自定义 @theme token 或语义工具类
```

## 关键发现（探索阶段实证）

### ① 测试类名查询波及面：~50 处、9 个文件

grep 实测（`querySelector|getElementsByClassName` in `*.test.tsx`）：测试大量依赖
`.error-note`、`.badge-pass/.badge-fail/.badge-kind`、`.checklist`、`.artifact-card`、
`.progress-fill`、`.muted`、`.markdown-doc`、`.attempt-meta`、`.filelog-table` 等类名做
结构与计数断言（含 `.attempt-meta .badge-pass` 嵌套查询）。全量迁移后类名消失。

**结论：改用 `data-testid`（或语义 data-* 属性）做测试挂钩**——Tailwind 保持唯一样式
体系，测试断言与样式解耦；部分计数/嵌套断言（如 badge-pass 计数）无法改写为
role/text 查询（文本 'pass' 会与 checklist 文本撞）。

主要分布：`App.test.tsx`、`ChangeDetailView.test.tsx`、`EvalChecklistRenderer.test.tsx`、
`ArtifactView.test.tsx`、`MarkdownDocRenderer.test.tsx`、`TasksProgressRenderer.test.tsx`、
`ChangeListView.test.tsx`、`__tests__/ipc_pipeline.test.tsx`、`__tests__/workspace_restore.test.tsx`。

### ② markdown 渲染的样式断层

`packages/desktop/src/renderers/MarkdownDocRenderer.tsx` 只包一层 `.markdown-doc` div，
react-markdown 输出**裸元素**（无 className），现有样式全靠 `.markdown-doc pre` /
`.markdown-doc table` 后代选择器。迁 Tailwind 后裸元素没有钩子。

**结论：引入 `@tailwindcss/typography`（`prose`）**，标准场景，且 markdown 是本 app
最重的渲染面（产物几乎全是 markdown）。备选：给 react-markdown 配 components 映射（更繁琐）。

### ③ Stryker 覆盖 src/**

`packages/desktop/stryker.config.json` mutate 全部非测试 ts/tsx（break 线 50）。Tailwind
化后组件内大量 className 字符串字面量，string mutator 会造出海量测试杀不死的等价
mutant，分数被稀释。

**ts-only 方案已否决**（空心化）：desktop 的测试形态几乎纯组件测试，hooks（.ts）的
mutant 也靠组件渲染间接杀死。.tsx 里承载真实逻辑且测试投入最重：App.tsx 的
UpdateIndicator 状态机、ChangeView 的 root 变更重置、ChangeDetailView 的
pipeline/checklist/fileLog 条件渲染（~560 行测试，全库最大）、TasksProgressRenderer
的百分比计算、各 renderer 的 payload 类型守卫。砍成 ts-only 后分数好看，但视图逻辑
零 mutation 覆盖——coverage theater。且与项目既定哲学相悖：归档 change
`2026-08-03-mutation-score-below-60` 对低分的处置是「survived → 补尖锐断言」，
不是排除文件刷分。

### 稀释来源拆解（估算）

| 来源 | 规模（估） | 测试能否杀死 | 稀释贡献 |
|------|-----------|-------------|---------|
| ① shadcn vendored 组件（`src/components/ui/**`） | ~300-500 mutant（sidebar 块落地后为大头，本 change 仅小控件） | 几乎不能 | ★★★ 大头 |
| ② 自研视图迁移后的 className | ~200-300 mutant | 多数不能 | ★☆ 有界 |
| ③ data-testid 字符串 | ~50 mutant | 能（测试就查它） | 零 |

### 定论与遗留张力

- **必做候选**：mutate 排除 `!src/components/ui/**`——但原理由（vendored 与
  node_modules 同待遇的「外来物」定位）已被 #13 内部化推翻，而分数稀释的客观事实
  不变（测试仍杀不死生成件 mutant）
- **design 阶段定夺**：维持排除（理由改写为「无对应测试投入的组件代码纳入 mutate
  只产噪声」）或纳入并补测试
- **观察**：②的漂移有界（粗算 85% → ~75%，不触 break 线 50），且部分被结构断言
  顺带杀死；Phase 1 落地后跑一次全量 `pnpm mutation-test` 实测
- **兜底（第二档）**：漂移仍不可接受才全局 `mutator.excludedMutations:
  ["StringLiteral"]`——代价是连带放过 hooks 里 `'list_workspaces'` 等 IPC 契约
  字符串的 mutant（`toHaveBeenCalledWith` 杀死的是真覆盖）与 ③的健康 mutant。
  Stryker 无按文件的 mutator 选择，配置空间只有「文件级 mutate glob」+
  「全局 mutator 排除」两档，做此取舍需记入 design
- **红利确认**：data-testid 方案在此兑现——testid 字符串 mutant 会被测试立刻
  杀死，不产生稀释

### ④ 工程约束撞点

- `max-lines-per-function: 50`（`packages/desktop/vite.config.ts` lint 节）——shadcn
  生成组件可能超线。按 #13 内部化：拆解消化，不豁免
- **knip**——shadcn 生成组件若只 import 部分 sub-component，unused-export 检查会
  报警。按 #13：删减生成件未用导出，不豁免
- **fmt**——`vp check --fix` 的 fmt 管线会规整生成件（引号/排序）。按 #13：照跑
  不豁免，不考虑与上游 diff
- **vite-plus 兼容性**——`plugins: [react()]` 已在用，`@tailwindcss/vite` 理论上同路
  插入；但 vp 是 VoidZero 包装层，需先行 spike 验证 `vp dev/build/test` 三链路

## 实施形态（#12 方案乙）

```
Phase 1  地基 + 1:1 视觉平移（无布局变化）
 ├─ Tailwind v4 + @tailwindcss/vite 经 vite-plus 三链路 spike（dev/build/test）
 ├─ components.json、@/ 别名、cn()、tokens（现有配色 hex → shadcn 变量映射）
 ├─ 步骤①：地基 + 元素级样式（body/button/#root）翻入 @layer base
 │   ——与旧类 CSS 共存，视觉零变，独立可验收（截图对比）
 ├─ 步骤②：单 commit 类名全量溶解 + 控件 shadcn 化（#15）+ prose 接管 markdown
 └─ 类名测试查询 → data-testid（与步骤②同 commit，保验收原子性）
```

步骤②先行「视觉平移可逐步验收」路线已否决：元素级全局样式在 Tailwind 接入瞬间
即被 preflight 重置，不存在逐文件无痛窗口。

## Spec 面预览

- `desktop-app-shell`：delta 增补 **UI 基建约定**——Tailwind 单一体系、shadcn
  vendored 组件按内部组件对待（#13）、data-testid 测试挂钩约定。布局与场景语义
  改写在姊妹 change。
- `desktop-workspace-store`：零改动

## 待 design 阶段解决

- vite-plus 三链路 spike 结果与 `@tailwindcss/vite` 插入方式
- Stryker：维持排除 `src/components/ui/**`（改理由）vs 纳入并补测试（#13 张力）
- 生成件超线的拆解方式（按内部组件拆文件 vs 其他）
- pass/fail/warn 语义色的落地形态：自定义 `@theme` token vs 工具类直写
