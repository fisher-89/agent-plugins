# desktop-workspace-config Specification (Delta)

## MODIFIED Requirements

### Requirement: 配置解析 core crate

Rust 侧 SHALL 新增 crate `crates/core/config`（裸名 `config`），承载工作区 `config.json` 的读取、解析、校验与默认值填充：

- 依赖 SHALL 仅 `foundation`（workspace 内，取 config.json 路径）+ `serde` / `serde_json` / `specta`；MUST NOT 依赖 `workflow` / `agent` / 任何 infra crate，MUST NOT 依赖 Tauri；
- 入口 API SHALL 为 `load(root) -> ConfigReport`（函数签名形态 design 定夺），`ConfigReport` SHALL 为 `{ config, diagnostics }` 信封：`config` 为**永远合法**的完整配置（违例字段以默认值填充），`diagnostics` 记录违例细节（哪项非法、哪些字段吃了默认、文件缺失等）；
- 校验规则 SHALL 以 `core/config` **现行实现为权威**（`KNOWN_KEYS` 八项键集（含 `projects`）、`TestFramework` 八值枚举（`jest` / `vitest` / `vite-plus` / `bun` / `rust` / `node-test` / `go` / `pytest`）、数值阈值 0–100 值域（coverage lines / branches / functions、mutation score）、`tests[].root` 非空且 MUST NOT 含 glob 通配符（`*` `?` `{` `[`）、`projects[].root` 非空且 MUST NOT 为绝对路径 / 含 `..` 段 / `:` / glob 通配符（拒绝理由：该路径会成为依赖安装命令的进程 cwd——逃出执行根即逃出沙箱，谓词本身零改动）、`projects[].language` 只校验类型（内容零校验：无白名单 / 无归一 / 无交叉校验）、`projects[].depends_on` 只校验类型（内容零校验：无引用 / 自引用 / 成环 / 去重判定）、`schema` 字段字面量 `spec-driven`、顶层未知字段 passthrough 保留）：原对齐源（CLI zod `config.schema.ts`）已随插件退役删除，MUST NOT 再作为权威或对齐断言的目标；机器可读镜像见「`config.schema.json` 静态镜像与实现一致性」requirement；
- 默认值 SHALL 为：coverage lines 80 / branches 70 / functions 75、mutation score 70、suite `cwd` `.`、`tests` `[]`、`projects` `[]`、`schema` `spec-driven`——数值以本 spec 与 `core/config` 实现为权威，原对齐源（CLI `defaults.ts`）已随插件退役删除，MUST NOT 再出现「对齐 `defaults.ts`」类断言或注释；
- `projects` SHALL 为顶层数组（元素 `{ root: string, language?: string, depends_on?: string[] }`，字段集封闭，承载项目锚点声明 —— 目录锚点 + 编码语言标注 + 项目间依赖声明）：缺失 / 显式 null → 空列表且**零诊断**（「有配置才按项目根扫描」）；非数组 → 空列表 + `InvalidValue`（path `projects`）；元素非对象或 `root` 缺失 / 非法（空串、非字符串、绝对路径、`..` 段、`:`、glob 通配符）→ 该元素剔除 + `InvalidValue`（path `projects[i]` / `projects[i].root`）；合法元素保序保留，`root` 为相对**执行根**的项目目录（git 根 workspace 下执行根即 worktree 根；workspace root 为仓库子目录时执行根为 `worktree/<rel>`，见 desktop-change-worktree「子目录 workspace 的执行根对齐」——`rel` 本身 MUST NOT 出现在配置里）；元素内其余子键静默忽略（沿现状处置，零诊断）；
- `projects[].language` SHALL 为**开放字符串**（编码语言标注，如 `rust` / `typescript`）：缺失 / 显式 null → `None` 且**零诊断**；字符串 → `Some` 原样保留（MUST NOT trim、MUST NOT 大小写 / 别名归一、MUST NOT 白名单校验、MUST NOT 与 `tests[].framework` 交叉校验）；非字符串 → `None` + `InvalidValue`（path `projects[i].language`）；该字段 MUST NOT 影响元素去留（元素身份只由 `root` 决定）；
- `projects[].depends_on` SHALL 为**字符串数组**（依赖的项目根，值宜为某个 `projects[].root`——镜像 description 可作提示文案，非断言）：缺失 / 显式 null → `None` 且**零诊断**；全字符串数组 → `Some`（保序、不去重、**零引用校验**：悬空 / 自引用 / 成环 / 重复项一律原样保留且零诊断，脏数据免疫责任在消费侧，见「项目协议语义边界与消费点纪律」）；非数组或元素混入非字符串 → `None` + `InvalidValue`（path `projects[i].depends_on`）；该字段 MUST NOT 影响元素去留；
- `projects` 元素字段集 SHALL 封闭为 `root` / `language` / `depends_on` 三项：运行策略常量（active 建档数上限、worktree 磁盘门槛等）MUST NOT 经该键承载；
- CLI `readConfig` 的「非法即静默整体回默认」吞错语义 SHALL NOT 复刻：任意输入（缺失文件、坏 JSON、字段非法）MUST NOT 使 `load` 失败收场——产出合法 config + diagnostics；非法输入产出的 config SHALL 能通过同一套校验（自洽不变量）；
- config.json 路径 SHALL 经 `foundation::layout` 的路径解析取得，crate 内 MUST NOT 出现 `config.json` 文件名字面量（隔离执法见 desktop-crate-layout 增量）。

#### Scenario: 合法配置原样呈现

- **WHEN** 对含合法 config.json 的工作区调用 `load`
- **THEN** 返回的 config 逐字段等于文件内容（passthrough 未知字段保留），diagnostics 为空

#### Scenario: 非法字段默认值填充并记 diagnostics

- **WHEN** config.json 含非法值（如 framework 为枚举外值、mutation score 为 200、suite root 含 `*`、`schema` 为非 `spec-driven` 字面量）
- **THEN** `load` 不失败：对应字段以默认值填充，diagnostics 逐项记录违例位置与所落默认值，其余合法字段原样保留

#### Scenario: 缺省字段默认值填充

- **WHEN** config.json 未设 coverage 阈值、未设 `tests`、未设 suite `cwd`
- **THEN** config 中 coverage 为 80 / 70 / 75、`tests` 为 `[]`、suite `cwd` 为 `.`，diagnostics 注明这些字段来自默认值而非文件
- **AND** 未设 `projects` 时 config 的 `projects` 为 `[]` 且不产生诊断

#### Scenario: 缺失与坏 JSON 不失败

- **WHEN** 工作区无 config.json，或 config.json 为非法 JSON 语法
- **THEN** `load` 均返回合法报告：config 为默认值填充，diagnostics 区分「文件缺失」与「JSON 语法非法」两种情形供上层呈现

#### Scenario: projects 缺省与 null 静默空列表

- **WHEN** config.json 未设 `projects` 或显式设为 `null`
- **THEN** config 的 `projects` 为空列表，diagnostics **不新增任何条目**（区别于 `tests` 的 DefaultApplied 口径）

#### Scenario: projects 非法元素逐项剔除

- **WHEN** `projects` 数组含非对象元素，或元素 `root` 为空串 / 非字符串 / 绝对路径 / 含 `..` 段 / `:` / glob 通配符
- **THEN** 该元素被剔除并在 `projects[i]` / `projects[i].root` 记 `InvalidValue`，合法元素按原序保留，`load` 不失败

#### Scenario: projects 非数组落空列表并记 InvalidValue

- **WHEN** `projects` 为对象 / 字符串 / 数字 / 布尔等非数组值
- **THEN** config 的 `projects` 为空列表，diagnostics 在 path `projects` 记 `InvalidValue`

#### Scenario: projects 元素 language 与 depends_on 原样保留

- **WHEN** `projects` 为 `[{"root": "a", "language": " TypeScript ", "depends_on": ["b", "b", "ghost"]}, {"root": "b"}]`
- **THEN** 首元素 `language` 为 `" TypeScript "`（不 trim、不归一、不校验）、`depends_on` 为 `["b", "b", "ghost"]`（保序、不去重、悬空项原样在案）；次元素两字段为 `None`；diagnostics **不新增任何条目**

#### Scenario: projects 元素软规则字段类型违例

- **WHEN** 某元素 `language` 为非字符串（数字 / 对象 / 数组），或 `depends_on` 为非数组 / 混入非字符串元素
- **THEN** 该字段落 `None` 并在 `projects[i].language` / `projects[i].depends_on` 记 `InvalidValue`；该元素本身**仍在案**（仅 `root` 决定去留），同元素其余合法字段原样保留

#### Scenario: 软规则字段不影响自洽不变量

- **WHEN** 含 `language` / `depends_on` 的合法 config 重序列化回 `Value` 再经同一组装
- **THEN** 二次组装零诊断（两字段回写形态可被判为合法），`config` 输出仍是同一套校验可接受的合法配置

#### Scenario: 项目根对位执行根

- **WHEN** 子目录 workspace（`rel = packages/desktop`）配置 `projects: [{ root: "." }]` 并创建 change
- **THEN** bootstrap 的执行根为 `host/packages/desktop`，项目 `root` 解析为 `<执行根>/.`（即执行根自身），安装命令 cwd 落在该子树内；配置中不出现 `rel` 段；`..` 与绝对路径仍按既有谓词拒绝（零放宽）

#### Scenario: 路径经 foundation 取得

- **WHEN** 审查 `crates/core/config` 源码
- **THEN** config.json 路径来自 `foundation::layout` 的解析函数，crate 内无 `config.json` 字面量、无第二处路径拼接

#### Scenario: 校验权威单源

- **WHEN** 审查 `core/config` 的校验规则来源（实现常量、注释、测试断言）
- **THEN** 权威只指向本 spec 与 `core/config` 实现（含 `config.schema.json` 镜像的双断言）
- **AND** 无「逐条复刻 CLI zod schema」「默认值对齐 CLI `defaults.ts`」类断言或注释残留（参照物已删除）

## Module Contract

### 模块增量：项目根相对基准改执行根（worktree-subdir-exec-root）

主基线既有行逐字沿用；本变更受影响模块如下：

| 模块 | 职责 | 关键契约（Function / API / CLI / Component） |
|------|------|------|
| `crates/core/config/config.schema.json`（`projects.description` / `projects.items.properties.root.description`） | 编辑器直读镜像 | 描述文案「相对 worktree 根」→「相对执行根」；键集、类型、值域、`default` 与两处断言锚点零改动（须与实现同 commit） |
| `crates/core/config/src/lib.rs`（`is_valid_project_root` 注释） | 拒绝理由留痕 | 注释理由「逃出 worktree」→「逃出执行根」；谓词（非空白 / 非绝对 / 无 `..` / 无 `:` / 无 glob）逐字不变，`KNOWN_KEYS` / `TestFramework` / `validate` 零改动 |
| `crates/core/workflow/src/write/create.rs`（`BootstrapProject.root` 文档 / `bootstrap_project` 警告文案） | 消费侧口径 | 文档与警告文案「相对 worktree 根」「在 worktree 内不存在」→「相对执行根」「在执行根内不存在」；遍历与安装语义零改动（见 desktop-change-worktree「确定性依赖 bootstrap」） |
| `crates/core/config/src/lib_test.rs` | 测试 | 描述文案不参与一致性断言（零断言改动）；`projects` 字段矩阵零改动 |
