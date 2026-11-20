# 工作区配置协议须知

## 文件与写权

- 工作区配置是仓库根的 `config.json`。它是**内置写保护规则的第一条**（agent 永远不可写，不可配置、不可移除）——你用 `write` / `edit` / `bash` 重定向去写它都会被写保护拒绝，不要试。
- 你的产出方式是提案：把整份候选交给 `propose_config`，由用户在配置页的提案卡上点「应用」才会真正落盘。
- 仓库里可能没有 `config.json`（多数 workspace 常态）：此时盘面为空对象，全部字段吃默认值，这不算错误。

## 顶层字段

- `$schema`：配置 schema 的引用 URL（字符串）。
- `schema`：模式标识，唯一合法值为字面量 `spec-driven`。
- `context`：项目上下文描述（字符串）。**当前零工程消费**——只在用户明确要求时写。
- `rules`：按工作流阶段分组的自定义规则（对象，子字段 `proposal` / `tasks` 各为字符串数组）。**当前零工程消费**——只在用户明确要求时写。
- `static_analysis`：静态分析工具标识（字符串）。
- `projects`：依赖引导项目锚点列表（数组，元素含 `root` / `language` / `depends_on`）。
- `tests`：测试 suite 列表（数组，元素含 `root` / `framework` / `cwd` / `config` / `includes` / `excludes` / `coverage` / `mutation`）。
- `write_protection`：写入保护配置（对象，子字段 `files` 为规则数组，规则含 `glob` / `reason`）。

顶层出现的未知键会原样保留并回写（passthrough），但这不是配置字段的扩展方式——未知键是用户手写的自由地带，你不该主动造键。

## 值域与阈值

- 覆盖率阈值 `coverage.lines` / `coverage.branches` / `coverage.functions` 与变异测试得分 `mutation.score` 都是 0–100 的数值（百分点）。越界或非数值会吃默认值（80 / 70 / 75 / 70）并记一条 `invalidValue`。
- 数值写整数就写整数（`80` 而不是 `80.0`），写小数按真实意图写。
- `tests[].cwd` 缺省为 `.`（相对 suite root）。

## 路径规则

- `tests[].root` / `projects[].root`：**非空字符串**、**不得为绝对路径**、**不得含 `..` 段**、**不得含 glob 通配符**（`*` `?` `{` `[`）。违例的 suite / 项目会被整条剔除。
- `tests[].config` / `tests[].includes` / `tests[].excludes` / `projects[].depends_on` / `write_protection.files[].glob` 均为相对 root 的路径或 glob 表达。
- 一个 suite 必须给 `root` 与 `framework`：缺任一即整条剔除。

## 软规则字段

- `projects[].language` 是开放字符串（零白名单、零归一、不与 `tests[].framework` 交叉校验）——写用户实际使用的语言名即可，如 `typescript` / `rust` / `go`。
- `projects[].depends_on` 是**平级边**，表达「安装顺序」而非目录层级：被依赖项目的 root 逐个列出，保序、不去重、不做引用校验。
- `projects[].language` / `depends_on` 在协议层零校验，规整只发生在消费侧——所以错了不会报 `invalidValue`，但会误导消费者，务必问清再写。

## 默认值的含义

- 诊断里的 `defaultApplied` 是**正常形态**，不是错误：它表示「这个有默认值的字段文件里没写，用了默认值」。
- 「清零噪音」的准确含义是给**有默认值的**字段写显式真值（例如 `coverage.lines: 90`），MUST NOT 给 `Optional` 字段写 `null`——显式 `null` 与缺键同义，噪音不会消失。

## 提案工具 propose_config

- `propose_config({})`：只读模式。返回当前盘面的文件面投影（`candidate` 为 `null`、`changes` 为空）——先看盘，再动手。
- `propose_config({ candidate })`：校验模式。候选必须是**整份文件面**（一次全量，不接受片段）：多词键用 snake_case（`static_analysis` / `write_protection` / `depends_on`），未知顶层键直接写在顶层（不要包进 `extra`），`extra` 那种 `[{key, value}]` 包装形态只出现在返回体之外的旧数据里。
- 校验失败返回 `is_error`，文案逐条列 `path: message`；按诊断逐条修正后**重交完整候选**（只交改动片段会被当成整份文件）。
- 校验通过返回归一化后的 `candidate` 与字段级 `changes`。落盘只发生在用户点「应用」之后——你永远不写文件。
- 建议节奏：先只读模式看盘，再用 `read` / `grep` / `glob` / `bash` 探查项目结构（lockfile、目录、测试配置），最后一次性提交完整候选。
