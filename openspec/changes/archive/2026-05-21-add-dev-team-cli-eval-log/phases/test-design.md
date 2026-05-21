# 测试设计: add-dev-team-cli-eval-log

> **变更**: add-dev-team-cli-eval-log
> **日期**: 2026-05-20
> **基于**: proposal.md

---

## 1. 测试层级

| 层级 | 范围 | 框架 | 目标 |
|------|------|------|------|
| 单元测试 | TypeScript 源码：`workflow.ts`（阶段排序/门控逻辑）、`eval-json.ts`（eval.json 读写/校验）、`change.ts`（变更目录解析）、`index.ts`（命令路由/参数解析） | Node.js 内置 test runner（`node:test`）+ `node:assert` | 达到 90%+ 行覆盖率；验证每个函数在正常路径、边界输入和错误条件下的行为 |
| 集成测试 | CLI 包装脚本（`dev-team` bash wrapper、`dev-team.cmd`）和 `dev-team-bundle.js` 的完整调用链路，包括真实的文件 I/O、eval.json 创建/追加、门控逻辑、`--attempt` 自动计算、`--backtrack-to` 写入 | bats（Bash Automated Testing System）v6+ | 覆盖所有 16 个 AC 中的 15 个（AC-1 至 AC-15），每次测试在临时沙箱中执行，使用真实 bundle 或 mock bundle，验证退出码、stdout/stderr 内容、eval.json 文件内容 |
| 端到端测试 | 完整工作流：esbuild 构建出 dev-team-bundle.js -> 调用 dev-team eval-log 写入多阶段记录 -> 模拟 Evaluator Agent 调用 CLI 的场景 -> 验证 eval.json 符合阶段门控约束 | bats + Node.js（`node:test` 编排） | 验证构建产物可正常使用、Agent 调用 CLI 的全链路正确性、门控逻辑在实际阶段序列中的行为 |

---

## 2. 覆盖映射

| 需求ID | 测试文件 | 测试层级 | 覆盖类型 |
|--------|---------|----------|----------|
| AC-1 | `openspec/changes/add-dev-team-cli-eval-log/tests/test_dev_team_wrapper.bats` | 集成测试 | 功能验证：bash wrapper --help 输出 |
| AC-2 | `openspec/changes/add-dev-team-cli-eval-log/tests/test_dev_team_cmd.bats` | 集成测试 | 功能验证：cmd wrapper --help 输出（Windows） |
| AC-3 | `openspec/changes/add-dev-team-cli-eval-log/tests/test_eval_log_create.bats` | 集成测试 | 功能验证：eval.json 从无到有创建 |
| AC-4 | `openspec/changes/add-dev-team-cli-eval-log/tests/test_eval_log_append.bats` | 集成测试 | 功能验证：追加非覆盖 |
| AC-5 | `openspec/changes/add-dev-team-cli-eval-log/tests/test_eval_log_missing_args.bats` | 集成测试 | 异常路径：--change 缺失 |
| AC-6 | `openspec/changes/add-dev-team-cli-eval-log/tests/test_eval_log_missing_args.bats` | 集成测试 | 异常路径：--phase 缺失 |
| AC-7 | `openspec/changes/add-dev-team-cli-eval-log/tests/test_eval_log_gate.bats` | 集成测试 | 功能验证：门控拒绝缺少前置阶段 |
| AC-8 | `openspec/changes/add-dev-team-cli-eval-log/tests/test_eval_log_gate.bats` | 集成测试 | 功能验证：门控允许前置阶段全 pass |
| AC-9 | `openspec/changes/add-dev-team-cli-eval-log/tests/test_eval_log_gate.bats` | 集成测试 | 功能验证：第一阶段无门控 |
| AC-10 | `openspec/changes/add-dev-team-cli-eval-log/tests/test_eval_log_same_phase.bats` | 集成测试 | 功能验证：同阶段重复写入 |
| AC-11 | `openspec/changes/add-dev-team-cli-eval-log/tests/test_eval_log_attempt.bats` | 集成测试 | 功能验证：attempt 自动计算 |
| AC-12 | `openspec/changes/add-dev-team-cli-eval-log/tests/test_eval_log_backtrack.bats` | 集成测试 | 功能验证：backtrack-to 字段 |
| AC-13 | `openspec/changes/add-dev-team-cli-eval-log/tests/test_eval_log_format.bats` | 集成测试 | 功能验证：JSON 格式和缩进 |
| AC-14 | `openspec/changes/add-dev-team-cli-eval-log/tests/test_dev_team_wrapper.bats` | 集成测试 | 异常路径：bundle 缺失时错误提示 |
| AC-15 | `openspec/changes/add-dev-team-cli-eval-log/tests/test_build.bats` | 集成测试 | 功能验证：esbuild 构建产物 |
| AC-16 | `openspec/changes/add-dev-team-cli-eval-log/tests/check_agent_updates.sh` | 端到端测试 | 人工审查辅助脚本：逐一检查 7 个 agent 提示词中的 eval.json 操作是否已替换为 CLI 调用 |
| (门控基础) | `openspec/changes/add-dev-team-cli-eval-log/tests/test_workflow_unit.test.mjs` | 单元测试 | `getPriorPhases()` 对全部 7 个阶段返回正确前置列表 |
| (参数解析) | `openspec/changes/add-dev-team-cli-eval-log/tests/test_index_unit.test.mjs` | 单元测试 | CLI 参数路由和必填参数校验 |
| (JSON 库) | `openspec/changes/add-dev-team-cli-eval-log/tests/test_eval_json_unit.test.mjs` | 单元测试 | eval.json 读取/校验/追加写入逻辑 |
| (变更路径) | `openspec/changes/add-dev-team-cli-eval-log/tests/test_change_unit.test.mjs` | 单元测试 | 变更名称到目录路径的解析 |

---

## 3. 测试策略

### 3.1 方法

采用自底向上的测试策略，分为三个层次：

1. **单元测试**：对 TypeScript 源码中每个导出函数进行隔离测试。使用 Node.js 内置 test runner（`node:test`）直接 import 编译后的源码或通过 ts-node/esm 加载 TypeScript 源码。所有文件系统操作（eval.json 读写、目录创建）在单元测试层被 mock 或使用 memfs，避免产生真实文件副作用。

2. **集成测试**：以 bats 为框架，在临时沙箱目录中执行完整的 CLI 调用。每个测试用例创建独立的临时变更目录，通过调用 `dev-team` 包装脚本（或直接调用 `node dev-team-bundle.js`）验证实际文件输出、退出码和标准错误输出。沙箱在 teardown 阶段自动清理。

3. **端到端测试**：模拟从 esbuild 构建到多个阶段连续评估的完整流程，验证多阶段写入后的 eval.json 符合阶段门控约束，以及 Evaluator Agent 替换后的调用模式可正常工作。

所有测试脚本和辅助工具统一存放于 `openspec/changes/add-dev-team-cli-eval-log/tests/` 目录。

### 3.2 测试分类

- **单元测试**（`*.test.mjs`）：覆盖 `workflow.ts`（`getPriorPhases()`、阶段排序常量）、`eval-json.ts`（`readEvalJson()`、`appendEvalEntry()`、`validateEntry()`）、`change.ts`（`resolveChangeDir()`）、`index.ts`（`parseArgs()`、`showHelp()`）。使用 Node.js native test runner（`node:test`）和 `node:assert`。

- **集成测试**（`*.bats`）：覆盖 CLI 包装脚本调用、eval.json 创建/追加、必填参数校验、阶段门控逻辑、自动 attempt 计算、backtrack-to 字段写入、JSON 格式验证、bundle 缺失错误、esbuild 构建验证。使用 bats v6+，依赖 `python3` 进行 JSON 解析断言。
  - 测试在临时沙箱（`mktemp -d`）中运行，使用项目的真实 `dev-team-bundle.js` 或为测试专门构建的 bundle
  - 共享的 `helpers/setup_test_env.sh` 提供 `setup_sandbox()`、`teardown_sandbox()`、`assert_json_field()`、`assert_file_exists()` 等辅助函数
  - 共享的 `fixtures/` 目录存放预置测试数据（如有多阶段记录的 eval.json）

- **端到端测试**（`test_e2e_full_flow.sh` 和 `check_agent_updates.sh`）：
  - `test_e2e_full_flow.sh`：完整构建 -> 阶段写入 -> 门控验证的全流程脚本
  - `check_agent_updates.sh`：辅助人工审查的脚本，遍历 7 个 Evaluator Agent 文件，报告每文件中是否仍包含 eval.json 直接读写的关键词（"Compute attempt"、"Append to eval.json"、"fs.readFileSync"、"writeFileSync"）

### 3.3 模拟策略

| 层级 | 模拟点 | 策略 |
|------|--------|------|
| 单元测试 | 文件系统（fs 模块） | 使用 `node:mock`（`mock.fn()`）拦截 `fs.readFileSync`、`fs.writeFileSync`、`fs.mkdirSync`、`fs.existsSync`。不产生真实文件写入 |
| 单元测试 | 进程退出（process.exit） | 使用 `node:mock` 拦截 `process.exit`，捕获退出码而非真正退出进程 |
| 单元测试 | `eval-json.ts` -> `change.ts` | 对 `resolveChangeDir()` 进行 mock，返回固定路径而非解析真实目录 |
| 集成测试 | 变更目录 | 在 bats `setup()` 中通过 `mktemp -d` 创建隔离的沙箱目录，测试中通过 `FAKE_CHANGES_DIR` 环境变量将变更根目录指向沙箱路径 |
| 集成测试 | `dev-team-bundle.js` | 优先使用已构建的 bundle 文件；若不存在则在 `setup()` 中通过 esbuild 临时构建测试 bundle |
| 集成测试 | 当前工作目录 | 每个测试用例通过 `cd` 进入沙箱目录执行，避免对项目目录的副作用 |
| 端到端测试 | 无（完全真实调用） | 使用真实 dev-team-bundle.js 和真实/临时变更目录，模拟完整工作流 |

---

## 4. 边界场景

| 场景 | 输入/条件 | 预期行为 | 测试文件 |
|------|----------|----------|----------|
| 空变更目录下首次写入 eval-log | 变更目录存在但无 phases 子目录 | CLI 自动创建 phases 目录和 eval.json 文件，退出码 0 | `test_eval_log_create.bats` |
| eval.json 包含无效 JSON 时写入 | eval.json 内容为 `{invalid-json` | CLI 退出码 1，stderr 提示 JSON 解析错误 | `test_eval_log_format.bats` |
| 门控检查时前置阶段存在但全部为 fail | 01-requirements 有一条 verdict "fail" 的记录，无 pass 记录 | CLI 退出码 1，stderr 明确指出 "01-requirements" 缺少 pass 记录 | `test_eval_log_gate.bats` |
| 门控检查时前置阶段完全不存在 | eval.json 为空数组 `[]`，执行 03-dev-proposal 的 eval-log | CLI 退出码 1，stderr 列出缺失的 01-requirements 和 02-test-design | `test_eval_log_gate.bats` |
| 门控检查时多个前置阶段混合 pass/fail | 01-requirements 有一条 pass，02-test-design 有一条 fail | CLI 退出码 1，仅指出 02-test-design 缺少 pass 记录 | `test_eval_log_gate.bats` |
| 同阶段先 fail 后 pass 连续写入 | 对 01-requirements 先后写入 fail 和 pass 各一条 | 两条记录均存在，eval.json 数组长度为 2，各自 verdict 正确 | `test_eval_log_same_phase.bats` |
| `--attempt` 缺省且 eval.json 为空 | eval.json 不存在或为空数组 | 写入的条目中 `attempt` 字段为 1 | `test_eval_log_attempt.bats` |
| `--attempt` 缺省且已有 3 条同阶段记录 | eval.json 中已有 3 条 01-requirements 记录 | 第 4 条条目的 `attempt` 字段为 4 | `test_eval_log_attempt.bats` |
| `--attempt` 缺省但其他阶段有记录，当前阶段无记录 | eval.json 中有 3 条 02-test-design，当前阶段为 01-requirements | 当前阶段 attempt 为 1（不统计其他阶段） | `test_eval_log_attempt.bats` |
| `--backtrack-to` 设为和当前阶段相同的值 | `--phase 04-test-gen --backtrack-to 04-test-gen` | `backtrack_to` 字段按传入值写入（CLI 不校验值语义合法性） | `test_eval_log_backtrack.bats` |
| `--backtrack-to` 值中包含无效阶段名 | `--backtrack-to 99-nonexistent` | `backtrack_to` 字段按传入值写入（CLI 不做阶段名校验） | `test_eval_log_backtrack.bats` |
| `--report` 参数恰好 500 字符 | `--report` 后跟一个 500 字符的字符串 | 写入成功，退出码 0 | `test_eval_log_create.bats` |
| `--items` 传入格式错误的 JSON | `--items '{bad json}'` | CLI 退出码 1，stderr 提示 JSON 解析失败并提供指导信息 | `test_eval_log_missing_args.bats` |
| `dev-team-bundle.js` 被删除/不存在 | `dev-team-bundle.js` 不存在时运行 `dev-team --help` | stderr 输出提示"文件未找到"或"请先执行构建" | `test_dev_team_wrapper.bats` |
| 变更名称含空格或特殊字符 | `--change "my change with spaces"` | CLI 使用 shell 展开后的实际值作为变更名，路径解析时使用该值 | `test_change_unit.test.mjs` |
| 阶段名不符合规范（非 01-requirements 等） | `--phase invalid-phase` | CLI 使用传入值直接写入，不做阶段名格式校验 | `test_index_unit.test.mjs` |
| `.cmd` 包装脚本在 bash 中被调用 | 在 bash 中执行 `sh plugins/dev-team/bin/dev-team.cmd` | `.cmd` 语法与 bash 不兼容，预期失败。此场景仅文档记录，不纳入自动化测试 | 不测试（平台边界） |

---

## 5. 测试数据

### 5.1 预制 eval.json 夹具

以下夹具文件存放于 `tests/fixtures/` 目录，用于集成测试的门控逻辑验证：

- **`eval_empty.json`**: 空数组 `[]`，用于测试首次写入和空文件场景
- **`eval_single_phase_pass.json`**: 仅包含 01-requirements 的一条 pass 记录，用于测试门控放行
- **`eval_single_phase_fail.json`**: 仅包含 01-requirements 的一条 fail 记录，用于测试门控拒绝
- **`eval_multi_phase_pass.json`**: 包含 01-requirements（pass）和 02-test-design（pass）的记录，用于测试 03-dev-proposal 的门控放行
- **`eval_multi_phase_mixed.json`**: 包含 01-requirements（pass）和 02-test-design（fail）的记录，用于测试门控拒绝
- **`eval_with_backtrack.json`**: 包含一条带有 `backtrack_to` 字段的记录，用于测试回溯字段读取

### 5.2 预置阶段列表

测试中使用的阶段列表与 `workflow.ts` 定义一致：

| 阶段编号 | 阶段 ID | 前置阶段 |
|----------|---------|----------|
| 1 | 01-requirements | (无) |
| 2 | 02-test-design | 01-requirements |
| 3 | 03-dev-proposal | 01-requirements, 02-test-design |
| 4 | 04-test-gen | 01-requirements, 02-test-design, 03-dev-proposal |
| 5 | 05-implementation | 01-requirements, 02-test-design, 03-dev-proposal, 04-test-gen |
| 6 | 06-code-review | 01-requirements ~ 05-implementation |
| 7 | 07-acceptance | 01-requirements ~ 06-code-review |

### 5.3 标准 eval-log 调用参数

集成测试中使用的标准 eval-log 调用：

```bash
# 标准 pass 条目
dev-team eval-log \
  --change test-change \
  --phase 01-requirements \
  --verdict pass \
  --report "All 6 required items pass. Verdict: pass." \
  --items '[{"item_id":"R1","pass":true,"evidence":"test","notes":""}]'

# 标准 fail 条目
dev-team eval-log \
  --change test-change \
  --phase 01-requirements \
  --verdict fail \
  --report "Item R3 failed: no risk mitigation found." \
  --items '[{"item_id":"R1","pass":true,"evidence":"test","notes":""},{"item_id":"R3","pass":false,"evidence":"missing","notes":"no mitigation"}]'
```

---

## 6. 不可测试项

- **AC-16（Evaluator Agent 更新审查）** — **原因**: Evaluator Agent 是 LLM 提示词（`.md` 文件），无法通过自动化测试验证其行为。提供 `check_agent_updates.sh` 辅助脚本作为人工审查工具，该脚本扫描 7 个 Agent 文件中是否仍包含"Compute attempt"和"Append to eval.json"等旧模式关键词，但最终确认需人工逐文件审查提示词中的具体步骤描述。代理在运行时的实际行为依赖于 LLM 对提示词的理解，不在自动化测试范围内。

- **Windows `.cmd` 包装脚本在非 Windows 环境下的行为** — **原因**: `.cmd` 文件是 Windows CMD 批处理脚本。在 Linux/macOS CI 环境中无法可靠执行。AC-2 的验证应在 Windows 开发机上手动执行，或通过专用的 Windows CI runner 验证。本测试设计中的 `test_dev_team_cmd.bats` 仅在 Windows 环境下运行，在非 Windows 环境下自动跳过。

- **`dev-team` CLI 在 PATH 未注册时的行为** — **原因**: CLI 通过包装脚本的完整路径调用，不依赖 PATH 注册。确保包装脚本在已知路径下可调用即可。PATH 注册属于开发者环境配置，非 CLI 自身行为。

- **esbuild 构建过程与 npm 依赖的兼容性** — **原因**: 构建过程依赖 `package.json` 中声明的 esbuild 版本。`test_build.bats` 验证构建脚本可正常执行且输出文件格式正确，但无法覆盖所有可能的 npm 安装/版本冲突场景。若 esbuild 版本与 Node.js 运行时存在 ABI 不兼容，应从项目的 `package-lock.json` 或 CI 构建日志中排查。
