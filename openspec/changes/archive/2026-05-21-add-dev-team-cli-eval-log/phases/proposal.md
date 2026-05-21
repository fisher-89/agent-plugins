# 提案: add-dev-team-cli-eval-log

> **变更**: add-dev-team-cli-eval-log
> **日期**: 2026-05-20
> **状态**: 提案中

---

## 问题

当前 dev-team 插件中，eval.json 的读写操作分散在 7 个 Evaluator 代理的提示词中。每个 Evaluator 代理（requirements-evaluator、test-design-evaluator、dev-proposal-evaluator、test-gen-evaluator、implementation-evaluator、code-review-evaluator、acceptance-evaluator）各自包含以下重复逻辑：

1. 读取 eval.json 文件计算当前阶段的尝试次数（attempt number）
2. 将评估结果直接追加写入 eval.json
3. 处理文件不存在时的边界情况

这种设计带来以下问题：

- **逻辑重复**：7 个 Evaluator 提示词中各自硬编码了相同的 eval.json 读写流程，修改 eval.json 格式需要同步更新所有文件
- **行为不一致**：各 Evaluator 对 eval.json 的边界处理（文件不存在、JSON 格式错误、目录不存在等）可能产生差异
- **缺少门控（gate）**：当前没有机制强制要求前一阶段的评估必须通过后才能进入下一阶段，导致工作流可以跳过关键验证步骤
- **难以演化**：后续需要增加评估回溯（backtrack）、记录 schema_version、变更 JSON 结构等需求时，必须逐一修改所有 Evaluator 提示词
- **耦合度高**：Evaluator 的逻辑中包含文件系统操作，将"评估逻辑"与"持久化逻辑"混合在一起，不利于关注点分离

同时，dev-team 插件目前只有 `openspec` CLI 二进制（bash 包装脚本 + 独立 JS 包），缺少一个专门为 dev-team 工作流设计的新 CLI。作为 dev-team CLI 建设的起点，首先实现 `eval-log` 命令，将 eval.json 的所有操作统一为一个 CLI 接口。

## 解决方案

### 方案一：保持现状，仅优化 Evaluator 提示词

不引入 CLI，仅在 Evaluator 提示词中统一 eval.json 读写的文本描述，使其更加规范和一致。

| 维度 | 评价 |
|------|------|
| 优点 | 改动最小，无需构建新的二进制和 TypeScript 源码；不增加维护成本 |
| 缺点 | 仍然是 7 处重复逻辑的文本描述，无法通过测试保证一致性；无法实现门控逻辑；难以确保所有 Evaluator 行为一致；eval.json 格式变更仍需修改 7 个文件 |

### 方案二：引入 dev-team CLI + eval-log 命令（推荐）

构建 dev-team CLI 框架，实现 `eval-log` 命令统一处理 eval.json 的写入操作。

| 维度 | 评价 |
|------|------|
| 优点 | 消除所有 Evaluator 中的重复逻辑；CLI 可被任何 Agent 调用，包括 Skills 中的流程逻辑；门控逻辑集中实现，行为统一；后续可扩展更多命令（eval-read、eval-list 等）；TypeScript 源码保证类型安全和可测试性；esbuild 打包为独立 JS 包，不增加运行时依赖 |
| 缺点 | 需要额外构建步骤（esbuild 打包）；需要学习 TypeScript 源码结构；引入新二进制文件需要注册到 PATH 或使用完整路径调用 |

### 方案三：使用 Python 脚本替代 TypeScript CLI

用 Python 实现 eval-log 功能，复用插件的 Python 工具链。

| 维度 | 评价 |
|------|------|
| 缺点 | 插件的 Python 工具（utils/eval-check.py）面向确定性脚本任务，不适合构建完整的 CLI；缺少命令行参数解析库的原生支持；需要 Python 运行时，与 Node.js 生态不一致（现有 openspec 已是 JS bundle）；引入第二种语言增加维护成本 |

### 推荐方案：方案二

理由：
- TypeScript + esbuild 与现有 `openspec-bundled.js` 的技术栈一致，团队无需学习新技术
- CLI 接口可以被任意 Agent（Evaluators、Skills、甚至手动调试）以相同方式调用
- 门控逻辑在 CLI 中集中实现一次，所有调用方自动受益
- 为后续将 dev-team CLI 发展为 openspec CLI 的替代品奠定基础

---

## 变更范围

### 实现以下特性

- **dev-team 二进制包装脚本**：创建 `plugins/dev-team/bin/dev-team`（bash wrapper）和 `plugins/dev-team/bin/dev-team.cmd`（Windows batch wrapper），遵循现有 `openspec` wrapper 的样式
- **TypeScript CLI 入口**：创建 `plugins/dev-team/bin/src/index.ts`，实现命令路由和 `--help` 输出
- **eval-log 命令**：创建 `plugins/dev-team/bin/src/commands/eval-log.ts`，实现完整的 eval-log 子命令，支持 `--change`、`--phase`、`--verdict`、`--report`、`--items`、`--attempt`、`--backtrack-to` 参数
- **eval.json 读写库**：创建 `plugins/dev-team/bin/src/lib/eval-json.ts`，实现 eval.json 的读取、校验和追加写入，支持目录自动创建（mkdir -p）
- **阶段排序库**：创建 `plugins/dev-team/bin/src/lib/workflow.ts`，定义 PGE 工作流阶段顺序常量（01-requirements 到 07-acceptance），实现 `getPriorPhases()` 函数
- **变更目录解析库**：创建 `plugins/dev-team/bin/src/lib/change.ts`，根据变更名称解析对应的目录路径
- **阶段门控逻辑**：在执行 eval-log 写入时，自动检查指定阶段之前的所有阶段是否已有至少一条 verdict 为 "pass" 的评估记录；若任一前置阶段缺失或仅有 "fail" 记录则退出并报错（code 1），第一阶段（01-requirements）不做门控校验
- **attempt 自动计算**：`--attempt` 为可选参数，缺省时自动从 eval.json 中统计当前阶段已有条目数 + 1
- **esbuild 构建配置**：添加 `plugins/dev-team/bin/package.json` 和 esbuild 构建脚本，将 TypeScript 源码打包为 `plugins/dev-team/bin/dev-team-bundle.js`
- **Evaluator 代理更新**：更新 7 个 Evaluator 代理提示词（agents/*-evaluator.md），将直接读写 eval.json 的逻辑替换为通过 `dev-team eval-log` CLI 调用

### 不要修改

- 不要实现 `--version` 命令（只实现 `--help`）
- 不要实现 eval-log 之外的任何子命令（`eval-check`、`eval-list`、`eval-read` 等留作后续扩展）
- 不要自动检测下一个阶段——调用方必须通过 `--phase` 明确指定
- 不要在 CLI 中校验 eval.json 的 JSON Schema——调用方负责提供正确的数据
- 不要修改现有 `openspec` CLI（`bin/openspec`、`bin/openspec.cmd`、`bin/openspec-bundled.js`）——两者共存，dev-team CLI 逐步替代 openspec CLI 是后续工作
- 不要修改 Skills 中直接读取 eval.json 的逻辑（`SKILL.md` 中检查 verdict 的部分）——这是后续 `eval-check` 或 `eval-status` 命令的范围

---

## 能力

### 新增能力

- **dev-team-cli** — dev-team 二进制包装脚本（bash + .cmd）和 TypeScript CLI 入口点，支持命令路由和 --help 输出。构建产物为 dev-team-bundle.js，由 esbuild 从 TypeScript 源码打包生成
- **eval-log-command** — eval-log 子命令的核心实现，接受 --change（必需）、--phase（必需）、--verdict（pass/fail，必需）、--report（必需，最长 500 字符）、--items（必需，JSON 数组字符串）、--attempt（可选，缺省自动计算）、--backtrack-to（可选）参数。内置阶段门控逻辑，确保前置阶段已通过评估
- **eval-json-lib** — 负责 eval.json 的读取、校验和追加写入的底层库。支持目录自动创建（mkdir -p）、append-only 模式（新条目追加到现有数组末尾，不覆盖已有数据）、正确缩进的 JSON 输出。在文件不存在时创建空数组后写入

### 修改的能力

- **phase-agents** — 更新 7 个 Evaluator 代理（requirements-evaluator、test-design-evaluator、dev-proposal-evaluator、test-gen-evaluator、implementation-evaluator、code-review-evaluator、acceptance-evaluator）的提示词，将原有的直接读取 eval.json 计算 attempt + 写入 eval.json 的模式，替换为通过 Bash 工具调用 `dev-team eval-log --change <name> --phase <phase> --verdict <verdict> --report "<report>" --items '<items>'` 的统一接口。Evaluator 不再直接操作文件系统，改为数据准备+CLI 调用的模式

---

## 验收标准

| ID | 验收条件 | 验证方法 | 优先级 |
|----|---------|----------|--------|
| AC-1 | `dev-team` 二进制可被 Bash 调用并显示帮助信息 | 运行 `bash plugins/dev-team/bin/dev-team --help`，输出包含 "Usage:" 和可用子命令列表 | P0 |
| AC-2 | `dev-team.cmd` 在 Windows 环境下可被调用并显示帮助信息 | 在 Windows CMD 中运行 `plugins/dev-team/bin/dev-team.cmd --help`，输出与 AC-1 一致 | P0 |
| AC-3 | eval-log 在 eval.json 不存在时创建文件并写入首个条目 | 在干净的变更目录中执行 `dev-team eval-log --change <test> --phase 01-requirements --verdict pass --report "test" --items '[{"item_id":"T1","pass":true,"evidence":"test","notes":""}]'`，验证 eval.json 被创建且包含一个有效的 JSON 数组 | P0 |
| AC-4 | eval-log 在 eval.json 已存在时追加新条目而非覆盖 | 对同一变更连续执行两次 eval-log，验证 eval.json 中包含 2 个条目，原有条目未被修改 | P0 |
| AC-5 | eval-log 在 `--change` 缺失时退出并返回错误码 1 | 执行 `dev-team eval-log` 不含 `--change`，验证退出码为 1 且错误信息中包含 "--change" | P0 |
| AC-6 | eval-log 在 `--phase` 缺失时退出并返回错误码 1 | 执行 `dev-team eval-log --change test` 不含 `--phase`，验证退出码为 1 且错误信息中包含 "--phase" | P0 |
| AC-7 | 门控逻辑拒绝当前阶段写入，如果前置阶段缺少 pass 记录 | 对于 03-dev-proposal 阶段，在 01-requirements 和 02-test-design 均无 pass 记录时执行 eval-log，验证退出码为 1 且错误信息指示缺少前置阶段 | P0 |
| AC-8 | 门控逻辑允许当前阶段写入，如果所有前置阶段有 pass 记录 | 对于 03-dev-proposal 阶段，在 01-requirements 和 02-test-design 均有 pass 记录时执行 eval-log，验证退出码为 0 且条目成功写入 | P0 |
| AC-9 | 门控逻辑对第一阶段（01-requirements）始终放行 | 直接为 01-requirements 执行 eval-log（无需任何前置记录），验证退出码为 0 | P0 |
| AC-10 | 同一阶段的重复写入始终允许，无论之前该阶段的条目 verdict 是 pass 还是 fail | 先为 01-requirements 写入一条 fail 记录，再写入一条 pass 记录，验证两条记录都存在，退出码均为 0 | P0 |
| AC-11 | `--attempt` 缺省时自动计算为当前阶段已有条目数 + 1 | 对某阶段执行 eval-log 不传 `--attempt`，验证写入条目中的 attempt 值等于现有条目数 + 1 | P0 |
| AC-12 | `--backtrack-to` 参数的值被正确写入条目 | 执行 eval-log 包含 `--backtrack-to 01-requirements`，验证 eval.json 中对应条目的 backtrack_to 字段为 "01-requirements" | P1 |
| AC-13 | 输出 JSON 格式有效且缩进正确 | 使用 `node -e "JSON.parse(fs.readFileSync('eval.json'))"` 验证 JSON 语法有效，手动检查缩进为 2 空格 | P1 |
| AC-14 | `dev-team` 二进制在 `dev-team-bundle.js` 不存在时给出明确错误 | 删除 `dev-team-bundle.js` 后运行 `dev-team --help`，验证输出错误信息提示"文件未找到"或"请先执行构建" | P1 |
| AC-15 | esbuild 构建脚本可正常打包所有 TypeScript 源码 | 运行构建命令（npm run build 或类似），验证输出 `dev-team-bundle.js` 文件且文件首行为 `#!/usr/bin/env node` | P0 |
| AC-16 | 7 个 Evaluator 代理的提示词中不再包含直接读写 eval.json 的逻辑 | 逐一检查 `agents/*-evaluator.md`，确认"Compute attempt number"和"Append to eval.json"的操作已被替换为调用 `dev-team eval-log` 的步骤 | P1 |

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| TypeScript 源码与 esbuild 配置不兼容，导致构建失败 | 开发阻塞，无法生成 `dev-team-bundle.js` | 中 | 在 `package.json` 中锁定 esbuild 版本（^0.20），构建脚本中启用 `--platform=node` 和 `--bundle` 参数；在 CI 中增加构建验证步骤；参考现有 `openspec-bundled.js` 的构建配置 |
| eval-log 命令输出与 Evaluator 代理期望的格式不一致 | 评估流程断裂，工作流卡住 | 中 | 严格遵循 eval.schema.json 定义输出格式；在 dev-team-bundle.js 构建完成后编写集成测试，用已知输入验证输出 JSON 的每个字段；参考现有 eval.json 的真实数据进行测试 |
| 门控逻辑中阶段排序与实际工作流不同步 | 允许跳过的阶段或错误地阻塞正常流程 | 低 | 阶段排序定义在单一源文件 `workflow.ts` 中，添加单元测试确保 `getPriorPhases()` 对所有 7 个阶段返回正确的前置阶段列表；后续新增阶段时强制要求更新此文件 |
| Windows .cmd 包装脚本存在语法错误 | Windows 用户无法使用 dev-team CLI | 低 | 在 Windows CI runner 上执行 `dev-team.cmd --help` 验证；脚本逻辑保持简单（仅设置目录并调用 node），不包含复杂的 shell 特性 |
| Evaluator 代理更新后 Agent 行为不一致（部分 Agent 仍使用旧模式） | 部分阶段使用 CLI，部分直接写入 eval.json | 低 | 在代码审查中逐一核对所有 7 个 Evaluator 代理的提示词改动；在 PR 描述中列出需要检查的全部文件；添加 checklist 确保无遗漏 |
| --items 参数传递复杂 JSON 数组时 shell 转义问题 | --items 中的引号被 shell 解释器移除或转义，导致 JSON 解析失败 | 中 | 要求调用方使用单引号包裹 JSON 字符串（`--items '[...]'`）；在 CLI 内部使用 `JSON.parse()` 解析，解析失败时输出明确的错误信息指导用户修正格式；在 Evaluator 代理提示词中给出正确的调用示例 |
| dev-team-bundle.js 体积过大影响加载速度 | 每次调用 eval-log 增加不必要的启动延迟 | 低 | esbuild 默认 tree-shaking 移除未使用代码；保持 TypeScript 源码的依赖最小化（仅使用 Node.js 内置模块，不引入第三方包）；`dev-team --help` 和路由逻辑应为惰性加载 |
