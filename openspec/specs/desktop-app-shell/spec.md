# desktop-app-shell Specification

## Purpose

定义 desktop-app Tauri 壳层的组织契约：command 按 queries / exec 双轨组织（MVP 仅实现三个查询命令、exec 为预留空轨道），workspace 经文件夹选择器选定，React 前端以显式刷新取数模型收口在 hooks 内。

## Requirements

### Requirement: Tauri command 查询双轨

desktop-app SHALL 按 queries / exec 双轨组织 Tauri command：

- `commands/queries/`：MVP 实现三个命令——`list_changes`（change 列表）、`get_change_detail`（change 详情）、`read_artifact`（按信封读取单个产物）
- `commands/exec/`：预留空轨道，MUST NOT 实现任何真实命令，MUST NOT 引入空壳 trait（trait 定形等第一条真实执行命令落地）

每个查询 command SHALL 是无状态薄包装：参数 → core 函数 → DTO 返回，MUST NOT 在 command 层持有或缓存 workspace 状态；所有 workspace 状态访问 SHALL 只经 workflow / foundation 的 core 函数。DTO SHALL 区分 Query Result 与 Command Result 形态。

#### Scenario: 查询命令薄包装

- **WHEN** 前端 invoke `list_changes` / `get_change_detail` / `read_artifact`
- **THEN** command 仅做参数转换并调用 core 查询函数，返回 DTO，自身无状态

#### Scenario: exec 轨道为空

- **WHEN** 检查 `commands/exec/`
- **THEN** 无任何已实现命令、无预定义 Executor trait 空壳

#### Scenario: 状态访问收敛 core

- **WHEN** 审查 command 层代码
- **THEN** 无直接文件系统访问，workspace 状态一律经 core 函数获取

### Requirement: workspace 选择

App SHALL 提供 workspace 选择与恢复能力：

- 启动时 SHALL 调用 `list_workspaces`，存在记录时自动恢复最近打开的 workspace（`last_opened_at` 降序第一名）并直接进入列表视图；无任何记录时 SHALL 停留在欢迎屏
- 当前根 SHALL 恒等于清单第一名（`list_workspaces` 按 `last_opened_at` 降序）：清单非空即选中第一名，清单为空（或被移除一空）时停留在欢迎屏；欢迎屏 SHALL 呈现空态与“添加新文件夹”入口；文件夹选择器 SHALL 仍是添加入口，选定后经 `add_workspace` 入库，刷新后新记录（`last_opened_at` 最新）即第一名并打开
- Header SHALL 提供 workspace 下拉切换；清单项 name 仅取目录名最后一段，悬停 SHALL 以 title 展示完整 path；切换 SHALL 清空当前 change 选中并以新根重新取数
- 选中 workspace（含自动恢复与下拉切换）SHALL 触发 `touch_workspace` 刷新 `last_opened_at`
- 移除当前打开的 workspace SHALL 经 Header 入口调用 `remove_workspace`；移除后 SHALL 切换到剩余清单第一名，清单为空时 SHALL 回到欢迎屏

选定 workspace 根后，列表与详情取数均以该根为基准。

#### Scenario: 启动自动恢复

- **WHEN** 应用启动且库中已有 workspace 记录
- **THEN** App 恢复 `last_opened_at` 最新的一条为当前根并进入列表视图，无需用户操作

#### Scenario: 无记录停留欢迎屏

- **WHEN** 应用启动且 `list_workspaces` 返回空清单
- **THEN** App 停留在欢迎屏，呈现“添加新文件夹”入口

#### Scenario: 欢迎屏添加

- **WHEN** 用户经文件夹选择器添加新目录
- **THEN** App 经 `add_workspace` 入库，刷新后以新记录（清单第一名）为根调用 `list_changes` 进入列表视图

#### Scenario: Header 下拉切换

- **WHEN** 用户在 Header 下拉中选择另一 workspace
- **THEN** 当前 change 选中被清空，`touch_workspace` 刷新 `last_opened_at` 后列表以新根（刷新后第一名）重取，悬停清单项可看到完整路径

#### Scenario: 移除当前打开的 workspace

- **WHEN** 用户经 Header 移除当前打开的 workspace
- **THEN** 该项从库中删除，视图切换到剩余清单第一名并以其为根重取列表；无剩余项时回到欢迎屏

### Requirement: React 前端刷新取数模型

前端 SHALL 以 React + TS 实现，取数收在 hooks（`useChangeList` / `useChangeDetail` / `useWorkspaces`）内：由用户显式刷新动作触发 invoke；`useWorkspaces` 为唯一例外——启动时自动触发一次以支撑自动恢复，此后仍由用户动作触发。MUST NOT 实现文件 watch、后台轮询或事件订阅。刷新 SHALL 覆盖两个层级：workspace 级（重取列表）与 change 级（重取当前详情）。未来替换为推送时 SHALL 仅改动 hooks 内部实现，视图层不感知取数方式。

视图 SHALL 至少呈现：change 列表（代际标注、按月分组）、phase 流水线（attempt / verdict / checklist 展开）、经 renderer 注册表渲染的产物区（含 markdown 文档与 tasks 进度）、workspace 清单（Header 下拉）。

#### Scenario: 刷新按钮触发重取

- **WHEN** 用户点击刷新
- **THEN** hooks 重新 invoke 对应查询命令并更新视图，期间无自动轮询发生

#### Scenario: 取数收口 hooks

- **WHEN** 审查视图组件代码
- **THEN** 组件不直接 invoke，取数统一经 `useChangeList` / `useChangeDetail` / `useWorkspaces`

#### Scenario: 无 watch 依赖

- **WHEN** 检查 desktop 依赖与前端代码
- **THEN** 无文件系统监听（notify 等）依赖、无定时器轮询逻辑

### Requirement: workspace 注册命令轨道

desktop-app SHALL 新增 `commands/workspaces` 命令轨道，承载 workspace 注册表（shell 记忆，非 change 域查询）四命令：`list_workspaces`（`last_opened_at` 降序清单）、`add_workspace`（canonicalize + upsert + touch）、`remove_workspace`、`touch_workspace`。命令 SHALL 经 Tauri `State<Store>` 访问 store（`main.rs` 启动时打开并 `.manage()`），自身 MUST NOT 直接操作 redb 或 db 文件。既有 `commands/queries/` 三命令的无状态语义与 `commands/exec/` 空轨道 SHALL 保持不变。

本轨道 SHALL 确立后续可失败命令的错误约定模板：命令返回 `Result<T, String>`，`Err` 由 Tauri 转为前端 reject，前端 hook 以 error 态接住呈现；MUST NOT 静默吞掉 db 打开或读写失败。db 打开失败 SHALL 使应用启动失败并报错，MUST NOT 静默降级为空清单。

#### Scenario: 命令经 State 访问 store

- **WHEN** 审查 `commands/workspaces` 实现
- **THEN** 四命令均以 `State<Store>` 取得 store，命令体为参数转换 + store 调用 + DTO 返回，签名中无 redb 类型

#### Scenario: 错误以 reject 传达前端

- **WHEN** store 操作失败（如 db 文件损坏、磁盘写失败）
- **THEN** 命令返回 `Err(String)`，前端 hook 的 error 态收到错误字符串并呈现，无静默成功

#### Scenario: db 打开失败快速失败

- **WHEN** 启动时 db 文件无法打开
- **THEN** 应用启动失败并给出错误信息，不进入空清单的静默降级

#### Scenario: 既有轨道不受影响

- **WHEN** 检查 `commands/queries` 与 `commands/exec`
- **THEN** 三个查询命令语义不变，exec 轨道仍无实现、无空壳 trait

### Requirement: command body 纪律与 app 层微形态

desktop-app SHALL NOT 抽独立 app 层 crate：command 即应用服务，维持既有双轨（queries / workspaces）加 exec 空轨道的组织不变。作为补偿纪律，任何 Tauri command body SHALL 只允许三件事：

1. **参数转换**（IPC 入参 → 领域/store 入参）；
2. **调用**（core 函数或 store 操作）；
3. **错误映射**（领域/Store 错误 → `Err(String)`）。

command 层 MUST NOT 实现领域解释（属领域解释的编排 SHALL 下推 core）或跨边界协调（属跨边界协调的编排 SHALL 触发 app crate 决策，见下一 requirement），MUST NOT 以"先塞进命令里"的方式消化编排增长。

`commands/workspaces` 的 `*_inner(&Store)` 纯函数模式 SHALL 视为 app 层微形态（IPC 适配与纯逻辑的函数级分层）予以保留：将来抽 app crate 时 SHALL 将 inner 函数平移复用（函数边界升 crate 边界），MUST NOT 重写。

#### Scenario: 新命令符合三件事

- **WHEN** 新增任一 Tauri command
- **THEN** body 为参数转换 + 调用 + 错误映射三段，无编排逻辑、无领域解释、无跨 store / fs 协调

#### Scenario: 编排增长被下推而非上塞

- **WHEN** 某命令需要新增一段编排逻辑（解析、校验、多步写等）
- **THEN** 领域解释部分落入 core crate，命令层仅保留三件事；跨边界协调则触发 app crate 决策，而非继续膨胀命令体

#### Scenario: inner 函数缝保留

- **WHEN** 审查 `commands/workspaces` 实现
- **THEN** IPC 命令为薄包装并经 `*_inner(&Store)` 纯函数调用 store，错误映射在命令层完成；该模式未被"内联回命令体"的重构破坏

### Requirement: app crate 翻转信号

出现下列任一信号时 SHALL 重新决策是否抽独立 app 层 crate（决策触发，不等架构回顾）：

1. Rust 侧引入 phase lifecycle 类命令（parse + validate + mutate + write 的真编排；TS 端 phase_start / phase_log 已落地，Rust 对齐时即触发）；
2. exec 轨道第一条真实命令落地（写 repo：保护检查 + 审计 + 执行 + touch 协调）；
3. 命令出现跨 store + fs 协调（如 add workspace 后自动 rescan）；
4. CLI / headless 复用桌面后端逻辑的需求出现；
5. 机械判据兜底：单命令非 IPC 样板逻辑超过一屏（约 30 行），或 `commands/` 非样板逻辑合计持续增长。

翻转信号清单 SHALL 保持可机械判定（命令类型可枚举、行数可数、复用需求可证），SHALL 随本能力 spec 存档，供代码评审与后续变更 proposal 引用。

#### Scenario: 信号可机械判定

- **WHEN** 审查翻转信号清单
- **THEN** 每条信号可由代码状态直接判定（特定命令是否出现 / 命令体行数 / 复用需求是否提出），无需主观架构评价

#### Scenario: 信号触发即重议

- **WHEN** 任一翻转信号出现（如 exec 第一条真实命令进入 design）
- **THEN** 对应变更的 proposal 显式回应"是否抽 app crate"的重新决策，而不是默认沿袭"不建"

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
| `desktop-app::commands::queries` | 三个查询命令 | list_changes / get_change_detail / read_artifact；无状态薄包装；返回 DTO |
| `desktop-app::commands::workspaces` | workspace 注册命令轨道 | list_workspaces / add_workspace / remove_workspace / touch_workspace；`State<Store>`；`Result<T, String>` |
| `desktop-app::commands::exec` | 预留空轨道 | 无实现、无空壳 trait |
| 前端 `hooks/` | 取数收口 | useChangeList / useChangeDetail / useWorkspaces；显式刷新触发（useWorkspaces 启动自动一次） |
| 前端视图 | 列表 / 流水线 / 产物区渲染 + 欢迎屏空态 / Header 下拉 | 消费 DTO 与 ArtifactEnvelope；未注册 kind 由 Fallback 兜底 |
| `desktop-app::commands::*`（全体命令） | 应用服务（command 即应用服务） | body 三件事：参数转换 / 调用 / 错误映射；无独立 app crate；编排下推 core 或触发翻转 |
| `commands::workspaces::{list,add,remove,touch}_workspace_inner` | app 层微形态 | 纯函数 + `&Store` 入参；将来抽 crate 时平移复用，不重写 |
| app crate 翻转信号 | 决策触发器 | 五条信号（phase lifecycle 命令 / exec 首命令 / 跨 store+fs 协调 / CLI 复用 / 行数机械判据），任一出现即重议 |
| `packages/desktop/src/styles/global.css` | Tailwind 样式入口 | `@import "tailwindcss"`;`@theme` token(hex 直写、shadcn 结构命名);`@layer base` 元素级样式 |
| `packages/desktop/src/lib/utils.ts` | 类名合并 | `cn()` = clsx + tailwind-merge |
| `packages/desktop/src/components/ui/**` | shadcn 内部化控件 | Button / Badge / Table / Progress;过 fmt/lint/knip 全管线无豁免;无 Next 语境残留 |
| `packages/desktop/components.json` | shadcn 生成配置 | alias `@/*`;内部化纪律适用 |
| `@/*` 路径别名(tsconfig + vite 双处) | ui/** import 解析 | 双处同步配置 |
| `src/renderers/MarkdownDocRenderer.tsx` | markdown 渲染 | `prose` 接管后代样式;无自定义类后代选择器 |
| `src/renderers/TasksProgressRenderer.tsx` | 任务进度渲染 | Progress 组件 value 承载百分比;无内联 `style={{ width }}` |
| `*.test.tsx`(9 文件) | 测试挂钩 | data-testid;无样式类名查询;与步骤② 同 commit |
| `stryker.config.json` | mutate 范围守线 | ui/** 处置 design 定夺有据;break 50 守线;StringLiteral 全局排除仅兜底 |
