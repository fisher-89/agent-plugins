# 设计: add-dev-team-cli-eval-log

> **变更**: add-dev-team-cli-eval-log
> **日期**: 2026-05-21
> **基于**: proposal.md, test-design.md

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| dev-team bash wrapper | 检测 dev-team-bundle.js 是否存在，调用 Node.js 执行打包后的 CLI | `plugins/dev-team/bin/dev-team` | Node.js, dev-team-bundle.js | Bash 脚本 |
| dev-team.cmd wrapper | Windows 环境下检测 dev-team-bundle.js 是否存在，调用 Node.js 执行打包后的 CLI | `plugins/dev-team/bin/dev-team.cmd` | Node.js, dev-team-bundle.js | Windows CMD 批处理 |
| CLI 入口 / 路由 | 创建 cac 实例，注册子命令，调用 cli.parse() 路由到对应命令处理器 | `plugins/dev-team/bin/src/index.ts` | cac (CLI 框架), Node.js 内置模块 | TypeScript, esbuild, cac |
| eval-log 命令 | 通过 cac 声明式 API 定义 eval-log 子命令（选项定义、帮助文本、action 回调），实现完整的 eval-log 流程：参数校验、门控检查、attempt 自动计算、eval.json 追加写入 | `plugins/dev-team/bin/src/commands/eval-log.ts` | eval-json-lib, workflow-lib, change-lib, cac | TypeScript, cac |
| eval-json 读写库 | eval.json 的读取 (`readEvalJson`)、校验 (`validateVerdict`, `validateReportLength`, `validateItemsJson`)、构建条目 (`buildEntry`)、attempt 计算 (`computeAttempt`)、门控检查 (`checkGate`)、追加写入 (`appendEntry`)。支持目录自动创建 (mkdir -p)、append-only 模式、2 空格缩进输出 | `plugins/dev-team/bin/src/lib/eval-json.ts` | Node.js fs/path 内置模块 | TypeScript |
| 工作流阶段库 | 定义 PGE 工作流 7 个阶段的排序常量 (`PHASES`)、`getPriorPhases()` 函数、`getPhaseIndex()` 查找 | `plugins/dev-team/bin/src/lib/workflow.ts` | 无 | TypeScript |
| 变更目录解析库 | 根据变更名称解析变更根目录路径（`openspec/changes/<change-name>`），返回 phases 子目录路径 (`getPhasesDir`) | `plugins/dev-team/bin/src/lib/change.ts` | Node.js path 内置模块 | TypeScript |
| esbuild 构建配置 | TypeScript 源码编译打包为单一 CommonJS bundle 文件，设 `#!/usr/bin/env node` shebang，内联 cac 依赖 | `plugins/dev-team/bin/package.json`, `plugins/dev-team/bin/tsconfig.json` | esbuild ^0.20, cac ^6.x | esbuild |

### 组件图

```
[dev-team bash wrapper] ──→ [dev-team-bundle.js] ──→ [index.ts: cac 实例]
       /                                                        \
[dev-team.cmd wrapper]                                           \
                                                                  \
                                                     [registerEvalLogCommand(cli)]
                                                             /      |       \
                                                   [eval-json]  [workflow]  [change]
                                                       │
                                                   [eval.json (文件系统)]
```

所有 TypeScript 源码在 `plugins/dev-team/bin/src/` 下，经 esbuild 打包为 `plugins/dev-team/bin/dev-team-bundle.js`。包装脚本调用 `node dev-team-bundle.js "$@"` 转发参数。

关键变化：`index.ts` 不再包含手动参数解析或帮助文本输出 —— `cac` 负责命令路由、选项解析和 `--help` 文本自动生成。`commands/eval-log.ts` 通过 `cac` 的声明式 API（`.command()`, `.option()`, `.action()`）定义命令及其选项，帮助文本从选项描述自动生成，与命令定义同处一个文件。

---

## 数据流

### 流程描述

1. **调用入口**：Agent (Evaluator) 通过 Bash 工具执行 `dev-team eval-log --change <name> --phase <phase> --verdict <verdict> --report "<report>" --items '<items>' [--attempt <n>] [--backtrack-to <phase>]`
2. **包装脚本路由**：`dev-team` bash wrapper 或 `dev-team.cmd` 检测 `dev-team-bundle.js` 是否存在，若不存在则输出错误信息并退出（code 1），否则执行 `node dev-team-bundle.js eval-log ...`
3. **参数解析与命令路由**：`index.ts` 创建 `cac` 实例并调用 `registerEvalLogCommand(cli)` 注册命令，`cli.parse()` 解析 argv，`cac` 自动匹配 `eval-log` 子命令并调用注册的 `action` 回调函数，传入已解析的 options 对象（camelCase 键名，如 `backtrackTo`）
4. **必填参数校验**：`cac` 对声明为 `{ required: true }` 的选项（`--change`, `--phase`, `--verdict`, `--report`, `--items`）自动校验是否缺失，缺失时输出错误并退出（code 1）
5. **值合法性校验**：action 回调中手动校验 `verdict` 为 "pass" 或 "fail"、`report` 不超过 500 字符、`items` 为合法 JSON 数组字符串
6. **变更目录解析**：`change.ts` 根据 `--change` 值拼接出 `openspec/changes/<change-name>/phases/` 目录路径
7. **门控检查**：`workflow.ts` 查询当前阶段的前置阶段列表，`eval-json.ts` 读取已有 eval.json，检查每个前置阶段是否有至少一条 `verdict: "pass"` 的记录。若任一前置阶段缺失或仅有 fail 记录则拒绝写入并退出（code 1）。第一阶段不做门控检查
8. **attempt 自动计算**：若 `--attempt` 未传入，从 eval.json 中统计当前阶段已有条目数并 +1
9. **写入 eval.json**：`eval-json.ts` 构建包含所有必填字段和自动生成字段（`timestamp`、`schema_version`）的条目对象，以 append-only 方式写入 eval.json。若文件不存在，先自动创建目录（mkdir -p）再创建文件
10. **输出确认**：写入成功后 stdout 输出一行确认 JSON，退出码 0

### 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| EvalEntry (写入 eval.json 数组的单个条目) | phase (string, required), timestamp (string, ISO 8601, auto-generated), attempt (integer >=1, required/auto), verdict ("pass"\|"fail", required), report (string, <=500 chars, required), items (array of Item, required), backtrack_to (string\|null, optional), schema_version ("1.0", auto-generated) | 每个条目属于一个 phase；同一 phase 可以有多个条目（多次尝试） | 追加写入 `openspec/changes/<change-name>/phases/eval.json` 的 JSON 数组末尾 |
| Item (单个检查项结果) | item_id (string, required), pass (boolean, required), evidence (string, required), notes (string, required) | 每个 Item 属于一个 EvalEntry 的 items 数组 | 内联在 EvalEntry.items 中 |
| eval.json 整体 | 全局数组，根元素为 `[EvalEntry, ...]` | 数组元素按写入时间顺序排列，每个元素对应一次评估。无主键约束，同一 phase 可有多条 | 文件系统中的 JSON 文件，2 空格缩进 |

**自动生成的字段**：timestamp（当前 ISO 8601 时间戳）和 schema_version（固定值 "1.0"）由 `eval-json.ts` 在写入时自动填充，调用方不应传入。

**数据约束**：
- report 字段最长 500 字符，超出时 CLI 拒绝写入并提示截断建议
- items 必须为合法的 JSON 数组字符串，解析失败则输出明确错误信息并退出 code 1
- eval.json 文件已存在时，追加写入不修改已有条目。文件不存在时创建文件并写入 `[entry]`

---

## 路由 / API 设计

CLI 命令结构（非 HTTP API，基于 `cac` 框架的 CLI 子命令路由）：

| 命令 | 子命令 | 描述 | 参数 | 输出 | 退出码 |
|------|--------|------|------|------|--------|
| `dev-team` | (无，顶级) | 显示帮助信息或路由到子命令 | `[--help]` 或 `[subcommand] [args]` | stdout: cac 自动生成的帮助文本（包含用法、子命令列表） | 0 (帮助), 1 (错误) |
| `dev-team` | `eval-log` | 执行评估日志追加写入 | `--change <name>` (required), `--phase <phase>` (required), `--verdict pass\|fail` (required), `--report <string>` (required), `--items <json-string>` (required), `--attempt <n>` (optional), `--backtrack-to <phase>` (optional) | stdout: `{"written":true,"phase":"<phase>","attempt":<n>}` | 0 (成功), 1 (参数错误/门控拒绝/文件错误) |

**参数规则**（通过 `cac` 声明式 API 定义）：
- 所有参数在 `commands/eval-log.ts` 的 `registerEvalLogCommand()` 中通过 `.option()` 声明
- `--change`：变更名称，`{ required: true }`，`cac` 自动校验缺失
- `--phase`：阶段标识符，`{ required: true }`，`cac` 自动校验缺失
- `--verdict`：`{ required: true }`，`cac` 自动校验缺失；action 回调中额外校验值为 "pass" 或 "fail"
- `--report`：`{ required: true }`，`cac` 自动校验缺失；action 回调中额外校验长度 <= 500
- `--items`：`{ required: true }`，`cac` 自动校验缺失；action 回调中额外校验 JSON 格式
- `--attempt`：可选参数，缺省时 action 回调中自动计算
- `--backtrack-to`：可选参数，`cac` 将其转换为 camelCase 键名 `backtrackTo`；action 回调不做语义校验，直接写入

**帮助文本生成**：
- 顶级 `dev-team --help`：`cac` 根据注册的命令列表自动生成，列出可用子命令
- `dev-team eval-log --help`：`cac` 根据 `.option()` 声明自动生成，列出所有参数的描述和必填标记
- 不再需要手动编写 `showHelp()` 和 `showHelpEvalLog()` 函数

**Shell 调用约定**（与之前保持一致）：
- `--report` 使用双引号包裹（`--report "summary text"`），内容中避免使用双引号
- `--items` 使用单引号包裹（`--items '[{"item_id":"R1","pass":true}]'`），避免 shell 展开

---

## 决策

| ID | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| D1 | 使用 TypeScript + esbuild + cac 构建 CLI，与现有 openspec CLI 技术栈一致 | 团队已熟悉此技术栈；esbuild tree-shaking 可生成紧凑的独立 bundle；cac 提供声明式命令路由和自动 --help 生成 | 备选: 使用 Python 脚本。拒绝理由: 引入第二种语言增加维护成本，且插件的 Python 工具面向确定性脚本任务，不适合构建完整 CLI |
| D2 | 包装脚本检测 bundle 文件是否存在，缺失时给出明确错误 | 提供快速反馈，避免用户因缺少构建产物而困惑；bash wrapper 和 .cmd wrapper 逻辑对称 | 备选: 直接从 src/ 通过 ts-node 运行。拒绝理由: 需要 ts-node 运行时依赖，增加启动延迟；不符合"最小运行时依赖"的设计目标 |
| D3 | 门控逻辑只要求前置阶段至少有一条 "pass" 记录，不限制 fail 记录数 | 允许在同一阶段多次尝试后最终通过；fail 记录用于审计历史，不应阻止后续阶段的合法执行 | 备选: 要求前置阶段 "最近一条记录" 为 pass。拒绝理由: 实现复杂度增加，且语义不清晰（什么是"最近"？基于什么排序？）；PGE 工作流的循环机制确保最终 pass 之前可能有多条 fail 记录 |
| D4 | `--items` 要求调用方使用单引号包裹 JSON 字符串，CLI 内部使用 `JSON.parse()` 解析 | 单引号包裹是 shell 中最可靠的 JSON 转义方式；`JSON.parse()` 是 Node.js 内置方法，无需第三方依赖 | 备选: 将 items 作为文件路径传入，从文件读取 JSON。拒绝理由: 增加了文件 I/O 复杂度，Evaluator Agent 需要先写临时文件再调用 CLI，不够直接 |
| D5 | 阶段顺序定义在一个源文件 (`workflow.ts`) 中，所有门控逻辑基于此 | 单一事实来源，避免阶段排序在不同地方硬编码导致的 drift；新增阶段时只需修改一个文件 | 备选: 从外部配置或 eval.schema.json 中读取阶段列表。拒绝理由: 引入外部依赖和 I/O，增加复杂性；阶段顺序是工作流的固有逻辑，适合在源码中硬编码 |
| D6 | eval-log 不校验 `--backtrack-to` 和 `--phase` 值的语义合法性，原样写入 | CLI 聚焦在"写入"操作上，语义校验由调用方 (Evaluator Agent) 负责；保持 CLI 职责单一 | 备选: 校验 backlog_to 值必须是合法的阶段 ID。拒绝理由: 限制了未来可能扩展的回溯语义（如回溯到任意阶段），过度约束 CLI 行为 |
| D7 | esbuild 构建输出为 CommonJS bundle + shebang，目录结构与 openspec 一致 | 与现有 `openspec-bundled.js` 在同一目录，包装脚本调用方式一致；CommonJS 格式兼容性最广 | 备选: 输出为 ESM bundle。拒绝理由: ESM 需要 Node.js 18+ 且需要使用 `node --loader` 等额外配置，不支持直接 shebang 执行 |
| D8 | 7 个 Evaluator Agent 的提示词更新为调用 CLI 而非直接读写 eval.json | 消除重复逻辑，所有 Evaluator 通过统一接口写入评估结果；CLI 中实现的门控逻辑自动对所有调用方生效 | 备选: 保持现有模式不变。拒绝理由: 7 处重复逻辑的文本描述无法通过测试保证一致性，修改 eval.json 格式需要同步更新所有文件 |
| D9 | 使用 cac 作为 CLI 框架，取代手动参数解析和帮助文本输出 | cac 是轻量级 CLI 框架（~10KB，零依赖），被 Vite 等广泛使用；声明式 API 自动生成 --help 文本、处理 `--` 分隔符、校验必填参数；减少约 70% 的 CLI 样板代码；与 esbuild 打包兼容良好 | 备选1: 手动解析（parseArgs + showHelp + showHelpEvalLog）。拒绝理由: 需要维护大量样板代码（参数解析循环、帮助文本字符串拼接），容易出错，缺少自动化 --help 生成，`--` 分隔符需额外处理，且帮助文本与命令定义分离不利于维护。备选2: commander。拒绝理由: 包体积 25KB+，是 cac 的 2.5 倍，API 更复杂，不适合小型 CLI |

---

## 依赖

### 运行时依赖

- Node.js (>=18) — 执行 dev-team-bundle.js 所需运行时
- cac (^6.x) — CLI 命令路由和参数解析框架（~10KB，零依赖，经 esbuild 内联到 bundle 中）

### 构建 / 测试依赖

- esbuild (^0.20) — TypeScript 源码打包为 CommonJS bundle，内联 cac 依赖
- TypeScript (^5.x) — 类型检查和 `.d.ts` 类型声明
- @types/node (^25.x) — Node.js 类型声明
- bats (v6+) — 集成测试框架
- Node.js test runner (node:test) — 单元测试框架
- python3 — 集成测试中 JSON 断言辅助

### 依赖树

```
dev-team-bundle.js
├── cac (内联)
└── Node.js 内置模块
    ├── fs (eval-json)
    ├── path (change, eval-json)
    └── process (index, eval-log 命令)
```

---

## 风险与缓解

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| esbuild 配置与 TypeScript 源码不兼容导致构建失败 | 开发阻塞，无法生成 bundle | 中 | 锁定 esbuild ^0.20；参考现有 openspec-bundled.js 使用的构建配置；构建脚本中使用 `--platform=node --bundle` 参数 |
| eval-log 输出格式与 Evaluator Agent 期望不一致 | 评估流程断裂，工作流卡住 | 中 | 严格遵循 eval.schema.json 定义；构建完成后运行集成测试验证每个字段写入正确性 |
| --items JSON 字符串 shell 转义问题 | 引号被 shell 解释器移除，JSON 解析失败 | 中 | 要求使用单引号包裹；CLI 解析失败时输出明确错误指导；在 Evaluator Agent 提示词中给出标准调用示例 |
| Evaluator Agent 更新后部分 Agent 仍使用旧模式 | 部分阶段直接写入 eval.json，部分使用 CLI | 低 | check_agent_updates.sh 辅助脚本扫描旧模式关键词；PR 审查中逐一核对 7 个 Agent 文件 |
| 门控逻辑中阶段排序与实际工作流不同步 | 允许跳过阶段或错误阻塞正常流程 | 低 | 阶段排序在单一源文件 workflow.ts 中定义；添加单元测试确保 getPriorPhases() 对全部 7 个阶段返回正确列表 |
| cac 版本更新引入 breaking changes | 构建或运行时失败 | 低 | 锁定 cac ^6.x 主版本；esbuild 将 cac 内联到 bundle 中，运行时不受 npm 安装环境影响 |

---

## 迁移步骤

1. **创建 TypeScript 源码目录结构**：创建 `plugins/dev-team/bin/src/` 及其子目录 `commands/` 和 `lib/`
2. **添加 cac 依赖**：在 `package.json` 中添加 `cac` 到 `dependencies`，执行 `npm install`
3. **实现底层库**：按依赖顺序实现 `change.ts` (无依赖) → `workflow.ts` (无依赖) → `eval-json.ts` (依赖 Node.js fs/path)
4. **实现 CLI 入口和 eval-log 命令**：`index.ts` 创建 cac 实例并注册命令；`commands/eval-log.ts` 实现 `registerEvalLogCommand(cli)`，通过 cac 声明式 API 定义命令选项和 action 回调
5. **创建 esbuild 构建配置**：`package.json` (含 build script、esbuild + cac dependencies)、`tsconfig.json`
6. **创建包装脚本**：`dev-team` (bash) 和 `dev-team.cmd` (Windows)，参照现有 openspec wrapper 模式
7. **执行首次构建**：运行 `npm run build` 生成 `dev-team-bundle.js`
8. **验证 CLI 可正常工作**：运行 `bash plugins/dev-team/bin/dev-team --help` 验证 cac 自动生成的帮助输出
9. **更新 7 个 Evaluator Agent 提示词**：逐一修改 `agents/*-evaluator.md`，将直接读写 eval.json 的逻辑替换为调用 `dev-team eval-log ...`
10. **创建测试目录和测试脚本**：在 `openspec/changes/add-dev-team-cli-eval-log/tests/` 下创建单元测试、集成测试和辅助脚本

---

## 待决问题

- dev-team CLI 在插件中的二进制路径注册：当前 `plugin.json` 中 `"bin"` 指向 `./bin/openspec`，dev-team CLI 是否也需要注册为插件的 bin？不注册（保持当前设计），仅通过完整路径 `plugins/dev-team/bin/dev-team` 调用
- 是否需要为 dev-team CLI 添加 smoke test 脚本作为 CI 门控的一部分？待项目实施时决定
- cac 的 camelCase 转换规则：`--backtrack-to` 在 action 回调中变为 `options.backtrackTo`，需在代码注释和 Evaluator Agent 更新时明确说明此行为
