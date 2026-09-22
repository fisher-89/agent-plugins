# 测试设计: tailwind-migration

> **日期**: 2026-09-22

---

## 验收范围

<!-- 测试框架识别结论(见 test_detect_frameworks):`packages/desktop` 套件为 vite-plus(`vite-plus/test` 运行器 + @testing-library/react + jsdom),单元/集成测试均为组件渲染测试;集成测试文件置于既有测试区域 `src/__tests__/`。
路径解析结论(见 test_resolve_paths):12 个 source -> test_file 对;errors 5 项——`src/components/ui/**` 4 件「Not in test config scope」(与 openspec/config.json desktop 套件 excludes 预置口径一致)、`src/styles/global.css`「Not a testable source file」,均落入「不可测试项」。
本 change 的测试工作主体是**既有 9 个测试文件的挂钩改写**(类名查询 -> data-testid),而非新增覆盖;改写用例在用例表中以「新增」标注并在测试条件内注明改写来源。 -->

| AC ID | 验收条件 | 测试类型 | 被测文件或模块 |
|--------|---------|---------|----------|
| AC-1 | `vite.config.ts` 含 `@tailwindcss/vite`;`vp dev` / `vp build` / `vp test` 三链路可用(build 产物含编译后 CSS) | 不可测试 | — |
| AC-2 | CSS 入口含 `@layer base` 的 body/button/#root 样式与 token;与旧类 CSS 共存;前后截图对比一致 | 不可测试 | — |
| AC-3 | `index.html` 无 `<style>` 块;`className` 中无旧自定义类;全量 utilities | 不可测试 | — |
| AC-4 | `src/components/ui/` 存在 Button / Badge / Table / Progress 且被视图引用;`.badge-*` / `.filelog-table` / `.progress-track` 等 CSS 定义不复存在 | 集成测试 | `packages/desktop/src/App.tsx`、`src/views/WelcomeView.tsx`、`src/views/changes/ChangeListView.tsx`、`src/views/changes/ChangeDetailView.tsx`、`src/renderers/TasksProgressRenderer.tsx`(消费方) × `src/components/ui/{button,badge,table,progress}.tsx`(提供方),见集成测试「视图层/renderer → shadcn 控件换装」 |
| AC-5 | `client:check`(vp check + knip)在含 ui/** 的全 src 上通过;无新增 ignorePatterns 或豁免 | 不可测试 | — |
| AC-6 | 9 个测试文件无类名查询(现 56 处清零);`vp test` 全绿 | 单元测试 | `src/App.test.tsx`、`src/views/changes/ChangeDetailView.test.tsx`、`src/views/changes/ChangeListView.test.tsx`、`src/renderers/EvalChecklistRenderer.test.tsx`、`src/renderers/ArtifactView.test.tsx`、`src/renderers/MarkdownDocRenderer.test.tsx`、`src/renderers/TasksProgressRenderer.test.tsx`、`src/__tests__/ipc_pipeline.test.tsx`、`src/__tests__/workspace_restore.test.tsx`(均相对 `packages/desktop/`;后两个集成层套件的改写另见集成测试两个关系章节) |
| AC-7 | `MarkdownDocRenderer` 以 `prose` 承载;其测试经新挂钩断言表格/代码块结构,无 `.markdown-doc` 后代选择器 | 单元测试 | `packages/desktop/src/renderers/MarkdownDocRenderer.test.tsx` |
| AC-8 | CSS 入口 token 值与原 `index.html` hex 一致(如 `--primary: #2563eb`) | 不可测试 | — |
| AC-9 | main 区 max-width 1100px 居中、DOM 结构不变(控件观感变化除外);步骤② 截图对比一致 | 不可测试 | — |
| AC-10 | 全量 mutation 实测分数 ≥ 50;`stryker.config.json` 的 ui/** 处置与理由成对 | 不可测试 | — |

<!-- AC-6 主路由为单元测试(9 个改写测试文件,7 个为与源文件同址的单测);`src/__tests__/` 下 2 个既有集成套件的改写在集成测试章节以独立关系承载,其「关联AC」同为 AC-6——该 AC 的覆盖同时路由到单元(7 文件)与集成(2 文件)两层,主类型按占多数的单层标注。 -->

---

## 单元测试

<!-- 覆盖所有可在进程内验证的场景(vite-plus/test 组件渲染测试与纯函数测试)。每个源文件对应一个独立的 `### <源文件> -> <测试文件>` 章节。
迭代类型口径:本 change 的测试工作主体是改写既有用例的查询挂钩(类名 -> data-testid/语义查询),行为断言全部保留——改写用例统一标「新增」并在测试条件中注明改写来源与原查询形态;「废弃」仅在用例被整体删除时使用,本 change 无计划删除用例。 -->

### packages/desktop/src/lib/utils.ts -> packages/desktop/src/lib/utils.test.ts

<!-- 源文件和测试文件路径均相对于项目根目录;测试文件为新增(colocated 惯例,resolve 派生路径)。 -->

#### 待测功能

- cn(): `twMerge(clsx(inputs))` 类名合并与 Tailwind 冲突消解,全 app utilities 组合的唯一入口

#### 用例

<!-- cn 是对 clsx + tailwind-merge 的薄组装层:只测组装契约(参数形态、falsy 剔除、冲突取后者),不逐项验证 tailwind-merge 自带的匹配/合并语义(既有测试纪律)。 -->

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| cn | 正向 | `cn('a', cond && 'b', 'c')` 形态:条件表达式为真时保留、为假时剔除,输出仅含真值类名 | 新增 |
| cn | 边界 | 无参调用与全 falsy 输入(`cn()` / `cn(false, undefined, '')`)返回空字符串 `''` | 新增 |
| cn | 边界 | `undefined` / `null` / `false` 混入被忽略,输出不含 `'undefined'` / `'null'` 字样 | 新增 |
| cn | 正向 | 冲突 utilities 后者胜:`cn('px-2', 'px-4')` 输出 `'px-4'`(组装契约冒烟,不深测 tailwind-merge 匹配语义) | 新增 |

#### Mock策略

<!-- 无 Mock:纯函数,无任何进程边界依赖。 -->

---

### packages/desktop/src/main.tsx -> packages/desktop/src/main.test.tsx

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更;本 change 对 main.tsx 的改动仅顶部 `import './styles/global.css'` 一行(D2 样式挂载点),挂载逻辑零变化,无进程内可断言的新行为。 -->

<!-- **不新建 `src/main.test.tsx`**(resolve 派生路径仅登记,不落地):挂载入口无独立断言语义,新建仅产覆盖噪声。样式接入的有效性由 AC-1 三链路构建验证兜底;壳渲染行为由 `src/App.test.tsx` / `src/__tests__/workspace_restore.test.tsx` 既有套件以真实挂载路径覆盖。故本节无用例表与 Mock 策略。 -->

---

### packages/desktop/src/App.tsx -> packages/desktop/src/App.test.tsx

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更(视图组件,无新增导出)。本节为既有套件的挂钩改写清单:App.test.tsx 现存 6 处 `querySelector` 类查询(`.error-note` 4 处、`option` 标签 1 处,另有 `getByRole` 语义查询不受影响),全部改写为 testid / RTL 语义查询,断言行为原样保留。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| App：启动恢复、欢迎屏清单与视图状态 | 正向 | workspace 动作失败后错误呈现:改 `getByTestId('error-note')` 且文本含错误串 `db: 移除失败`(改写:`container.querySelector('.error-note')` 存在性 + textContent 两处断言) | 新增 |
| App：启动恢复、欢迎屏清单与视图状态 | 正向 | 无错误时错误条一个不渲染:改 `queryAllByTestId('error-note')` 长度为 0(改写:`querySelectorAll('.error-note')` 长度断言) | 新增 |
| App：启动恢复、欢迎屏清单与视图状态 | 正向 | 错误呈现停留列表视图:`getByTestId('error-note')` 文本断言 + 下拉 `tagName === 'SELECT'` 断言保留(改写:`.error-note` 文本) | 新增 |
| App：启动恢复、欢迎屏清单与视图状态 | 边界 | 下拉切换后 option 枚举:改 `[...screen.getByRole('combobox').querySelectorAll('option')]` 为 `getAllByRole('option')` 取 value 序列,断言仅剩剩余第一名(改写:`querySelectorAll('option')` 标签查询,清零口径 6 处标签查询之一) | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `@tauri-apps/api/core` invoke | `vi.mock` + `vi.hoisted`,按命令名分发(`list_workspaces` / `touch_workspace` / `remove_workspace` / `add_workspace` / `list_changes` / `get_change_detail`),沿用既有 `mockIpc()` 方案 | 全部用例(挂载恢复、切换、移除、添加流) |
| `@tauri-apps/plugin-dialog` open | `vi.mock`,按用例 `mockResolvedValue` / `mockRejectedValue` 切换 | 添加对话框取消 / reject / 成功用例 |
| `@tauri-apps/api/app` getVersion | `vi.mock` 固定 resolve `'0.1.0'` | 挂载取版本号,与 invoke 调用序列隔离 |

---

### packages/desktop/src/views/WelcomeView.tsx -> packages/desktop/src/views/WelcomeView.test.tsx

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更(视图组件,无新增导出)。 -->

<!-- **无既有测试文件且不新建**(resolve 派生路径仅登记,不落地):欢迎屏行为(空态文案、「添加新文件夹」入口、error-note 呈现、screen-center 布局容器)全部经由 `src/App.tsx` 渲染路径,已被 `App.test.tsx` 与 `__tests__/workspace_restore.test.tsx` 覆盖;本 change 对 WelcomeView 的改动仅为类名翻 utilities + testid 标注,其 `error-note` 挂钩改写随上述两个套件承载。新建专属文件仅产重复覆盖。故本节无用例表与 Mock 策略。 -->

---

### packages/desktop/src/views/changes/ChangeListView.tsx -> packages/desktop/src/views/changes/ChangeListView.test.tsx

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更(视图组件,无新增导出)。本节为既有套件的挂钩改写清单:现存 3 处类名/标签查询(`.badge`、`span.created`、`.error-note`)。`badge-in${inventory}` 动态模板串已按 spec 硬性要求收敛为 `INVENTORY_VARIANT: Record<Inventory, ...>` 显式映射换 Badge,徽标断言面随之迁移。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| ChangeListView：分组列表、代际徽标与进入详情 | 正向 | 代际徽标 v2 / v0 / v1 全枚举:改在 `change-row` testid 作用域内收集 Badge 文本集合,断言 `arrayContaining(['v2', 'v0', 'v1'])`(改写:`querySelectorAll('.badge')` 枚举;映射收敛后徽标宿主从模板串类名 span 变为 Badge 组件) | 新增 |
| ChangeListView：created / 错误条 / 空态提示的分支形态 | 正向 | created 日期精确断言:改 `getAllByTestId('created')` 取文本集合,断言含 `'2026-09-01'` 与 `'2026-05-15'` 且全部非空(改写:`querySelectorAll('span.created')` 标签+类查询;`created` 挂钩按 design「命名约定」补注,分配表未列) | 新增 |
| ChangeListView：created / 错误条 / 空态提示的分支形态 | 正向 | 无错误时不渲染错误条:改 `queryByTestId('error-note')` 为 null(改写:`querySelector('.error-note')`) | 新增 |
| ChangeListView：created / 错误条 / 空态提示的分支形态 | 边界 | created 为 null 的条目不渲染日期节点且整页无 `'null'` 字样(既有 textContent 断言保留,作改写后回归护栏) | 新增 |

#### Mock策略

<!-- 无 Mock:测试以 fixture DTO 直接注入 `ChangeListState`(取数已在 hooks 层被测),不跨进程边界。 -->

---

### packages/desktop/src/views/changes/ChangeDetailView.tsx -> packages/desktop/src/views/changes/ChangeDetailView.test.tsx

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更(视图组件,无新增导出)。本节为既有套件的挂钩改写清单——全库最重的改写面:现存 28 处类名/标签查询(`.checklist`、`.badge-*` 嵌套、`.artifact-card`、`.muted`、`.error-note`、`.backtrack`、`.filelog-table`、`.badge-inv2`、`tbody tr`/`td`/`button` 标签查询),对应 design「主要挂钩分配」表的 `checklist` + `checklist-verdict`、`attempt-meta` + `attempt-verdict`、`artifact-card`、`detail-note`、`error-note`、`backtrack`、`filelog-table` 挂钩。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| ChangeDetailView：9 站流水线、运行标示、降级区块与产物区 | 正向 | checklist 展开徽标:改在 `getByTestId('checklist')` 域内断言 `checklist-verdict` 徽标文本为 `'pass'`(改写:`querySelector('.checklist .badge-pass')` 嵌套类查询) | 新增 |
| ChangeDetailView：9 站流水线、运行标示、降级区块与产物区 | 正向 | 产物区按信封顺序渲染:改 `getAllByTestId('artifact-card')` 长度 3,逐卡 textContent 依次含 `提案` / `100%` / `file-log 保底`(改写:`querySelectorAll('.artifact-card')` 计数与序) | 新增 |
| ChangeDetailView：9 站流水线、运行标示、降级区块与产物区 | 正向 | error 态与未找到态:错误条改 `getByTestId('error-note')` 文本含 `详情加载失败`(改写:`.error-note`);中性降级改 `getByTestId('detail-note')`(改写:`.muted` 分支);detail 为 null 时 `queryByTestId('error-note')` 为 null | 新增 |
| ChangeDetailView：标记位、backtrack 组合、区块分支与头部元信息 | 正向 | verdict 徽标限定 attempt-meta 域:改 `attempt-verdict` testid 断言文本 `'pass'` / `'fail'`,并收集该域内徽标文本集合作 arrayContaining 断言(改写:`.attempt-meta .badge-pass` / `.badge-fail` / `.badge` 三处嵌套查询) | 新增 |
| ChangeDetailView：标记位、backtrack 组合、区块分支与头部元信息 | 正向 | backtrack 四组合形态:to/reason 各形态下文本断言保留,存在态改 `getByTestId('backtrack')`、全空态改 `queryByTestId('backtrack')` 为 null(改写:`.backtrack` 存在性两处) | 新增 |
| ChangeDetailView：标记位、backtrack 组合、区块分支与头部元信息 | 正向 | checklist 分支:空清单改 `queryByTestId('checklist')` 为 null,有清单改 `getByTestId('checklist')` 存在且域内 `checklist-verdict` 文本 `'fail'`(改写:`.checklist` 三处 + `.checklist .badge-fail`) | 新增 |
| ChangeDetailView：标记位、backtrack 组合、区块分支与头部元信息 | 正向 | file_log 表格:改在 `getByTestId('filelog-table')` 域内以 `getAllByRole('row')` / cell 语义查询断言 2 行 5 列文本矩阵含 `—` 占位(改写:`.filelog-table` 存在性 + `tbody tr` / `td` 标签查询,清零口径标签查询之二);空数组时 `queryByTestId('filelog-table')` 为 null 且渲染 `（空）` 占位 | 新增 |
| ChangeDetailView：标记位、backtrack 组合、区块分支与头部元信息 | 正向 | 头部代际徽标:改在头部容器作用域内断言徽标文本 `'v2'`(改写:`.badge-inv2` 类查询;模板串已收敛为 INVENTORY_VARIANT 显式映射换 Badge) | 新增 |
| ChangeDetailView：标记位、backtrack 组合、区块分支与头部元信息 | 边界 | created 为 null 时不产空占位节点:改在 `detail-header` 容器挂钩(按命名约定补注,分配表未列)作用域内断言全部文本节点非空(改写:`querySelectorAll('.muted')` 非空枚举) | 新增 |
| ChangeDetailView：标记位、backtrack 组合、区块分支与头部元信息 | 正向 | 刷新按钮禁用态:改 `getByRole('button', { name: '刷新详情' })` 断言 `disabled`(改写:`querySelectorAll('button')` 遍历查找,清零口径标签查询之三;换装 Button 后 role 查询天然成立) | 新增 |
| ChangeDetailView：标记位、backtrack 组合、区块分支与头部元信息 | 正向 | loading 态与未找到态共用降级页:两态均改 `getByTestId('detail-note')` 存在且 `queryByTestId('error-note')` 为 null(改写:`.muted` 两处 + `.error-note` 两处) | 新增 |

#### Mock策略

<!-- 无 Mock:测试以 `ChangeDetailState` fixture 直接注入视图 props,不跨进程边界;IPC 链路归 `__tests__` 集成套件承载。 -->

---

### packages/desktop/src/views/changes/ChangeView.tsx -> packages/desktop/src/views/changes/ChangeView.test.tsx

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更(视图组件,无新增导出);design 变更清单明确该文件为核对项——现状无 className,预计零改动。 -->

<!-- **不新建 `src/views/changes/ChangeView.test.tsx`**(resolve 派生路径仅登记,不落地):无行为变更即无用例增量;若步骤② 溶解最终触及该文件,其渲染链路已被 App 壳与 Detail 套件经组合渲染覆盖。故本节无用例表与 Mock 策略。 -->

---

### packages/desktop/src/renderers/ArtifactView.tsx -> packages/desktop/src/renderers/ArtifactView.test.tsx

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更(视图组件,无新增导出)。本节为既有套件的挂钩改写清单:现存 4 处类名查询(`.badge-kind` 两处、`.artifact-version` 一处;`.artifact-card` 归 Detail/ipc 套件)。对应挂钩:`artifact-kind`、`artifact-version`、`artifact-card`。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| ArtifactView：信封按 kind 路由到 renderer | 正向 | 信封头渲染断言:改 `getByTestId('artifact-kind')` 文本 `'tasks-progress'`、`getByTestId('artifact-version')` 文本 `'v1'`(改写:`.badge-kind` / `.artifact-version` 类查询;`.badge-kind` 换装 Badge 后 testid 落于 Badge 宿主) | 新增 |
| ArtifactView：信封按 kind 路由到 renderer | 边界 | 未注册 kind 且 fallbackText 为 null:改 `getByTestId('artifact-kind')` 存在且文本 `'mystery-kind'`,占位文案断言保留(改写:`.badge-kind` 存在性;「永不白屏」护栏语义不变) | 新增 |

<!-- 已注册 kind 路由、payload 形状漂移降级两组用例为 getByText / queryByText 语义查询,零改写保留,不入迭代清单。 -->

#### Mock策略

<!-- 无 Mock:内存构造 ArtifactEnvelope 直接注入,不跨进程边界。 -->

---

### packages/desktop/src/renderers/Fallback.tsx -> packages/desktop/src/renderers/Fallback.test.tsx

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更(视图组件,无新增导出)。 -->

<!-- 既有 4 个用例全部为 getByText 文本查询,无任何类名/标签查询;`.fallback-text` 翻 utilities 后断言面零影响——**零改写、不新增/废弃用例**,本节不列用例表与 Mock 策略。(注意区分:`fallback-text` testid 挂钩服务于 EvalChecklistRenderer 与 ChangeDetailView 的降级断言改写,Fallback 自身套件无需。) -->

---

### packages/desktop/src/renderers/EvalChecklistRenderer.tsx -> packages/desktop/src/renderers/EvalChecklistRenderer.test.tsx

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更(视图组件,无新增导出)。本节为既有套件的挂钩改写清单:现存 14 处类名查询(`.badge`、`.badge-pass`、`.badge-fail`、`.checklist` 及嵌套、`.attempt-meta .badge`、`.fallback-text`),对应挂钩:`checklist` + `checklist-verdict`、`attempt-meta` + `attempt-verdict`、`fallback-text`(按命名约定补注,分配表未列)。payload 收窄/守卫逻辑零改动,其既有异常用例全部保留。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| EvalChecklistRenderer：items（item / pass / evidence）清单渲染 | 正向 | pass / fail 条目可区分标识:改在 `getByTestId('checklist')` 域内收集 `checklist-verdict` 徽标文本集合,断言含 `'pass'` 与 `'fail'`(改写:`querySelectorAll('.badge')` + `.badge-pass` / `.badge-fail` 三处) | 新增 |
| EvalChecklistRenderer：payload 收窄、统计与降级分支 | 正向 | verdict 徽标三态:有 verdict 时改 `getByTestId('attempt-verdict')` 文本 `'pass'` / `'fail'`;null 时改 `queryByTestId('attempt-verdict')` 为 null(改写:`.attempt-meta .badge-pass` / `.badge-fail` / `.attempt-meta .badge` 三处) | 新增 |
| EvalChecklistRenderer：payload 收窄、统计与降级分支 | 正向 | 条目徽标按 pass / fail 计数:改 `within(checklist).getAllByTestId('checklist-verdict')` 按文本过滤得 2 pass + 1 fail,且嵌套归属断言落在 checklist 域内(改写:`querySelectorAll('.badge-pass')` 长度 + `.checklist .badge-pass` 嵌套,共 5 处) | 新增 |
| EvalChecklistRenderer：payload 收窄、统计与降级分支 | 异常 | `expectFallback` 助手改写:改 `getByTestId('fallback-text')` 文本断言 + `queryByTestId('checklist')` 为 null;该助手作用于拒收全组用例(payload 非对象、字段类型漂移、缺 phase、items 非数组、非法条目),一次性迁移全部降级断言(改写:`.fallback-text` + `.checklist`,并消除助手内 2 处 querySelector) | 新增 |
| EvalChecklistRenderer：payload 收窄、统计与降级分支 | 边界 | verdict 与 attempt 为 null 属合法 payload:改 `getByTestId('checklist')` 存在,统计/隐藏断言保留(改写:`.checklist` 存在性) | 新增 |

#### Mock策略

<!-- 无 Mock:内存构造 ArtifactEnvelope 直接注入,不跨进程边界。 -->

---

### packages/desktop/src/renderers/TasksProgressRenderer.tsx -> packages/desktop/src/renderers/TasksProgressRenderer.test.tsx

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更(视图组件,无新增导出)。本节为既有套件的挂钩改写清单,且是断言面发生**语义迁移**的唯一文件:`.progress-track` / `.progress-fill` 换装 shadcn Progress(radix 封装)后,全库唯一内联 `style={{ width }}` 消失,原 `fill.style.width === '40%'` 式断言不可复用,迁移至 radix 语义属性(role `progressbar` 的 `aria-valuenow`,辅以 `progress` testid 落点)。百分比计算逻辑零改动,既有数值断言(`40%` / `0%` / `100%` 文本、计数标签)全部保留。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| TasksProgressRenderer：total / done / pending 进度渲染 | 正向 | 比例渲染:改 `getByRole('progressbar')` 断言 `aria-valuenow` 为 40(改写:`.progress-fill` 的 `style.width === '40%'`;内联 width 随控件换装消失,断言面迁移至语义属性) | 新增 |
| TasksProgressRenderer：total / done / pending 进度渲染 | 边界 | total=0 无除零:改 `aria-valuenow` 为 0 且整页无 `NaN`(改写:width `'0%'` 断言;`0%` 文本断言保留) | 新增 |
| TasksProgressRenderer：total / done / pending 进度渲染 | 边界 | done=total 满进度:改 `aria-valuenow` 为 100(改写:width `'100%'` 断言) | 新增 |
| TasksProgressRenderer：total / done / pending 进度渲染 | 异常 | payload 缺字段 / 类型漂移:不抛错且 progressbar `aria-valuenow` 为 0,计数标签按 0 呈现(改写:结构断言迁移至语义属性,`0%` / `共 0` 文本断言保留) | 新增 |

#### Mock策略

<!-- 无 Mock:内存构造 ArtifactEnvelope 直接注入,不跨进程边界。 -->

---

### packages/desktop/src/renderers/MarkdownDocRenderer.tsx -> packages/desktop/src/renderers/MarkdownDocRenderer.test.tsx

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更(视图组件,无新增导出)。本节为既有套件的挂钩改写清单 + AC-7 新增结构断言:现存 2 处查询(`.markdown-doc`、`strong` 标签查询)改写为 `markdown-root` testid 与 RTL 语义查询;payload 守卫与 react-markdown 管线零改动,降级组用例(getByText 文本查询)零改写保留。清零口径下**不得**引入任何新的 `querySelector` / 标签查询——代码块/表格结构断言一律以 testid 域 + `within()` + 文本/role 查询表达。 -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| MarkdownDocRenderer：payload.markdown 的收窄与渲染组装 | 正向 | 加粗结构断言:改 `getByText('加粗')` 断言节点 `tagName === 'STRONG'`(改写:`container.querySelector('strong')` 标签查询,清零口径标签查询之四) | 新增 |
| MarkdownDocRenderer：payload.markdown 的收窄与渲染组装 | 正向 | AC-7 新增——表格结构:注入 GFM 表格 markdown,在 `getByTestId('markdown-root')` 域内以 `getByRole('table')` 断言表格存在且含表头/单元格文本(prose 接管后仍可经语义挂钩断言结构,不再依赖 `.markdown-doc` 后代选择器) | 新增 |
| MarkdownDocRenderer：payload.markdown 的收窄与渲染组装 | 正向 | AC-7 新增——代码块结构:注入围栏代码块 markdown,在 `markdown-root` 域内以 `within()` + 代码文本 getByText 断言代码内容进入 DOM(不引入新标签查询) | 新增 |
| MarkdownDocRenderer：payload.markdown 的收窄与渲染组装 | 边界 | markdown 为空串:改 `getByTestId('markdown-root')` 存在(改写:`.markdown-doc` 存在性;prose 容器承载空内容不崩) | 新增 |

<!-- payload 缺 markdown / 类型漂移 / null 三组降级用例为 getByText 查询,零改写保留,不入迭代清单。 -->

#### Mock策略

<!-- 无 Mock:内存构造 ArtifactEnvelope 直接注入,不跨进程边界。 -->

---

## 集成测试

<!-- 跨模块交互识别结论:本 change 新增的跨模块交互是「视图层/renderer 消费 shadcn 控件」(AC-4);另有两处既有集成套件的挂钩改写(AC-6 集成层)需要独立关系承载。跨模块交互清单之外的关系(如 MarkdownDocRenderer → global.css prose 通道)不设章节:prose 样式效果属 CSS 渲染面,jsdom 无布局引擎无法进程内断言,其可测部分(渲染结构)已由 MarkdownDocRenderer 单测承载(见不可测试项)。 -->

### 视图层/renderer → shadcn 控件换装 → `packages/desktop/src/__tests__/shadcn_controls.test.tsx`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `packages/desktop/src/components/ui/button.tsx` / `badge.tsx` / `table.tsx` / `progress.tsx` | 提供方(shadcn 控件,渲染语义与 variant 面) |
| `packages/desktop/src/views/changes/ChangeListView.tsx` | 消费方(`INVENTORY_VARIANT` → Badge、change-row → Button) |
| `packages/desktop/src/views/changes/ChangeDetailView.tsx` | 消费方(filelog → Table、verdict/inv 徽标 → Badge、动作按钮 → Button) |
| `packages/desktop/src/renderers/TasksProgressRenderer.tsx` | 消费方(progress → Progress 承载百分比) |
| `packages/desktop/src/App.tsx`、`src/views/WelcomeView.tsx` | 消费方(header / 欢迎屏动作 → Button) |

**关联AC**: AC-4

**关系描述**: ui/** 四件生成件被排除出单测与变异口径(`openspec/config.json` excludes 与 stryker mutate glob 同口径,D5),本关系因此是「控件被视图引用且渲染出正确语义」(AC-4 前半)的唯一自动化验证面——不验证观感,只验证 jsdom 可断言的语义属性(role / aria / testid)与交互契约。出错模式集中在四点:variant 传错(`INVENTORY_VARIANT` 映射漏键或键值不配,模板串收敛后类名拼写错误编译期不可再兜底)、Table 组装丢行丢列(filelog 行列矩阵错位)、Progress value 契约错(`value: number | null` 的 0 与 null 混淆)、Button 交互契约丢失(disabled 透传、点击回调)。逐一以渲染断言钉住。

#### 场景: Badge 变体全枚举渲染(含 INVENTORY_VARIANT 显式映射)

验证三个代际变体经 `Record<Inventory, ...>` 显式映射逐一落到 Badge 宿主。前置:注入含 inv0 / inv1 / inv2 条目的 ChangeList fixture 与详情 DTO;输入:三代际同屏渲染;预期:各条目域内徽标文本与代际一一对应,无模板串类名残留。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 列表三代际同屏:各 change-row 作用域内徽标文本分别为 `'v0'` / `'v1'` / `'v2'`(INVENTORY_VARIANT 三键全命中) | 新增 |
| 正向 | 详情头部代际徽标与列表徽标同源映射:inv2 详情头部徽标文本 `'v2'`,两渲染点 variant 一致 | 新增 |
| 异常 | DTO 注入 inventory 漂移值(联合类型外的运行时值):不崩且不以 `badge-in${inventory}` 模板串形态产出类名,回退 Badge 默认变体 | 新增 |

#### 场景: change-row 换装 Button(D7)

验证行级条目从原生 button 元素供养换装为 shadcn Button 后,行点击交互契约不变。前置:注入含可点击条目的列表;输入:fireEvent.click;预期:宿主带 button role 且 onSelect 携带 change 名。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | change-row 宿主为 `role="button"`,点击触发 onSelect 且参数为条目 change 名 | 新增 |
| 边界 | created 为 null 且 unparsable 的条目(标注态)仍可点击进入详情 | 新增 |
| 边界 | 档案分组成员与活跃条目点击行为一致(分组不改变交互契约) | 新增 |

#### 场景: filelog Table 语义结构

验证 `.filelog-table` 换装 shadcn Table 后行列语义完整。前置:注入含 2 条 fileLog(含 attempt/at 空缺)的详情;输入:渲染 ChangeDetailView;预期:filelog-table 挂钩域内呈现 role=table 的完整行列矩阵,空缺字段以 `—` 占位。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | `filelog-table` 域内 `getAllByRole('row')` 共 3 行(表头 + 2 数据行),单元格文本矩阵逐格匹配 | 新增 |
| 异常 | fileLog 为 null:v1 及更早代际降级文案呈现,`queryByTestId('filelog-table')` 为 null,不白屏 | 新增 |
| 边界 | fileLog 为空数组:渲染 `（空）` 占位且无 Table 结构 | 新增 |

#### 场景: Progress 承载百分比

验证 `.progress-track/fill` 换装 radix Progress 后 value 契约正确。前置:注入 tasks-progress 信封;输入:payload 计数三元组;预期:progressbar 语义属性反映百分比,0 与 null 两种「无进度」形态不混淆。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | payload `{ total: 5, done: 2 }`:`getByRole('progressbar')` 的 `aria-valuenow` 为 40 | 新增 |
| 边界 | 两端值:done=total 时 `aria-valuenow` 为 100;total=0 时为 0 且无 NaN | 新增 |
| 异常 | payload 缺字段 / 类型漂移:计算收窄为 0,progressbar value 为数值 0 而非 null(indeterminate 形态不得出现) | 新增 |

#### 场景: Button 换装保持动作契约

验证 header / 欢迎屏 / 详情各动作按钮换装 Button 后点击与禁用契约不变。前置:经 App 壳以 mocked IPC 渲染至各视图态;输入:fireEvent.click;预期:各动作回调经既有 IPC 命令面触发。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | header 移除 / 刷新列表、欢迎屏添加入口、详情返回与刷新详情:点击后对应回调 / IPC 命令照常触发(按钮宿主从全局供养 button 变为 Button 组件,行为面不变) | 新增 |
| 异常 | 详情 loading 中:刷新详情按钮 disabled,点击不触发重复取数 | 新增 |
| 边界 | Button 默认形态(本 app 未用 asChild)渲染原生 button 元素,焦点与禁用属性可透传 | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `@tauri-apps/api/core` invoke | `vi.mock` 按命令名分发返回 fixture(沿用 App.test.tsx 的 mockIpc 分发方案),`read_artifact` 可按 kind 切换 resolve / reject | Button 动作契约场景(经 App 壳渲染);控件直连场景(注入 props 渲染视图)不跨进程,无需 Mock |

---

### workspace壳恢复链路 → error-note 挂钩改写 → `packages/desktop/src/__tests__/workspace_restore.test.tsx`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `packages/desktop/src/App.tsx` | 挂钩落点(error-note 容器标注) |
| `packages/desktop/src/hooks/useWorkspaces.ts` | 数据链路(恢复 touch 与取数时序) |
| `packages/desktop/src/__tests__/workspace_restore.test.tsx` | 验证方(invoke 时序 / 次数 / 参数契约断言) |

**关联AC**: AC-6

**关系描述**: 该套件是启动恢复链(`list_workspaces` → touch fire-and-forget → `list_changes`)的集成验证面,断言焦点是 invoke 时序契约;唯一类名查询是 list_workspaces reject 后的 `document.querySelector('.error-note')`。类名随溶解消失后必须改 `data-testid="error-note"` 查询,且链路时序断言一条不能丢——出错模式正是改写时顺手「简化」掉时序/次数断言,使挂钩改写演变为覆盖缩水。

#### 场景: list_workspaces reject 错误呈现挂钩改写

验证错误呈现断言迁移到 testid 挂钩后,链路护栏语义原样保留。前置:list_workspaces reject 开关打开;输入:渲染 App;预期:error-note testid 命中,且无 touch、无 list_changes 的次数断言与欢迎屏手动添加路径断言全部保留。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | list_workspaces reject:改 `getByTestId('error-note')` 断言错误呈现(改写:`document.querySelector('.error-note')`);无 touch / 无 list_changes 次数断言原样保留 | 新增 |
| 边界 | 恢复失败不阻断手动添加:open resolve 后对话框被调用的链路断言保留(回归护栏,确认改写未伤及链路) | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `@tauri-apps/api/core` invoke | `vi.mock` 按命令分发,list / touch 以模块级 reject 开关切失败 | list_workspaces reject 用例 |
| `@tauri-apps/plugin-dialog` open | `vi.mock` resolve 路径串 | 手动添加路径用例 |

---

### IPC 渲染管线 → artifact-card 挂钩改写 → `packages/desktop/src/__tests__/ipc_pipeline.test.tsx`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `packages/desktop/src/renderers/ArtifactView.tsx` | 挂钩落点(artifact-card 外壳标注) |
| `packages/desktop/src/renderers/registry.ts` | 中间件(kind → renderer 路由,本 change 零改动) |
| `packages/desktop/src/views/changes/ChangeDetailView.tsx` | 聚合方(产物区逐信封渲染) |
| `packages/desktop/src/__tests__/ipc_pipeline.test.tsx` | 验证方(IPC mock → 详情 → 产物全链路断言) |

**关联AC**: AC-6

**关系描述**: 该套件是全库唯一贯通 IPC mock → 详情取数 → 产物信封路由 → renderer 的完整链路验证面;`.artifact-card` 计数查询(单卡 read_artifact reject 时三卡仍齐全)是本套件唯一类名查询,改 `getAllByTestId('artifact-card')`。出错模式:改写时把「降级卡仍计数」的护栏断言弱化为存在性断言,丢失部分失败不阻断主体的语义。

#### 场景: 降级卡计数护栏改写

验证产物卡计数断言迁移到 testid 挂钩后,部分失败降级的护栏语义原样保留。前置:三个产物信封中一个 read_artifact reject;输入:渲染至详情产物区;预期:三张 artifact-card 齐全且失败卡以降级形态渲染。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 单个 read_artifact reject:改 `getAllByTestId('artifact-card')` 长度 3(改写:`querySelectorAll('.artifact-card')`),失败卡以 0% 降级形态渲染且详情主体不阻断 | 新增 |
| 边界 | read 成功路径:保底文本 / kind 徽标可见断言保留(回归护栏) | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `@tauri-apps/api/core` invoke | `vi.mock` 按 `list_workspaces` / `touch_workspace` / `get_change_detail` / `read_artifact` 分发,readArtifact 可按 kind 切 reject | 全部用例 |

---

## 不可测试项

<!-- 含 test_resolve_paths errors 5 项的落账(ui/** 4 件「Not in test config scope」、global.css「Not a testable source file」)。 -->

- AC-1 Tailwind 接入 vite-plus 三链路(`vp dev` / `vp build` / `vp test` 可用、build 产物含编译后 CSS) — **原因**: 构建管线执行面验证,非进程内可断言;由 Phase A 三链路 spike 实测定案(D3),测试套件本身在全绿状态运行即 test 链路可用的间接佐证。
- AC-2 步骤① 元素级样式入 `@layer base` 与旧类 CSS 共存、前后截图对比一致 — **原因**: CSS 文件静态内容断言属同义反复,视觉一致性属截图人工比对口径;jsdom 无布局引擎,进程内无法断言渲染结果。
- AC-3 步骤② 全量溶解(`index.html` 无 `<style>` 块、`className` 无旧自定义类) — **原因**: 文本静态约束,属 grep / lint 口径验收而非行为测试;类名消失后的行为面由 9 个改写测试文件全绿承载(AC-6)。
- AC-5 vendored 全管线无豁免(`client:check` 通过、无新增 ignorePatterns) — **原因**: fmt / lint / knip 静态管线执行面,由 `client:check` 命令验收,与自动化测试分属不同验证层。
- AC-8 token 配色身份保留(hex 直写一致、`--primary: #2563eb`) — **原因**: CSS token 静态值断言属文本比对,不构成行为测试;配色观感归属 AC-9 截图比对口径。
- AC-9 布局零变化(main 区 max-width 1100px 居中、步骤② 截图对比) — **原因**: 布局与视觉属截图人工比对口径;DOM 结构不变部分由既有语义查询测试(heading / combobox / option / 文本矩阵)全绿间接保障,但不构成布局断言。
- AC-10 mutation 实测分数 ≥ 50 与 stryker 处置理由成对 — **原因**: Stryker 变异测试执行面验收,非单元 / 集成测试范畴;由 Phase E 全量实测记录在案。
- `packages/desktop/src/main.tsx`(样式挂载点,单行 import) — **原因**: 入口挂载无独立进程内断言语义;resolve 给出 `src/main.test.tsx` 但不落地(仅产覆盖噪声),由 AC-1 三链路与 App 壳套件兜底。
- `packages/desktop/src/components/ui/{button,badge,table,progress}.tsx` 专属单测 — **原因**: resolve 报「Not in test config scope」(与 `openspec/config.json` desktop 套件 excludes 预置口径一致),且 design D5 定夺 ui/** 排除出 mutate、proposal 明确「选择排除则无新增」;其「被视图引用且渲染正确语义」由集成关系「视图层/renderer → shadcn 控件换装」承载。
- `packages/desktop/src/styles/global.css` — **原因**: resolve 报「Not a testable source file」;纯样式声明无可执行单元,token 与 `@layer base` 内容归 AC-2 / AC-8 静态与截图口径。
- `packages/desktop/components.json`、`vite.config.ts`、`tsconfig.json`、`index.html`、`stryker.config.json`、`openspec/config.json`、`package.json` — **原因**: 配置 / 依赖清单文件,无可执行单元;分别归 AC-1(管线)、AC-3(溶解静态口径)、AC-5(豁免口径)、AC-10(变异处置)的执行面验收。
- prose 对 markdown 的排版观感(typography 生成的表格边框、代码底色等) — **原因**: CSS 渲染结果,jsdom 无布局引擎;可测部分(表格 / 代码块 DOM 结构经 `markdown-root` 挂钩断言)已由 MarkdownDocRenderer 单测承载,观感微调(D9 `prose-*` modifier)归 AC-9 截图比对口径。




