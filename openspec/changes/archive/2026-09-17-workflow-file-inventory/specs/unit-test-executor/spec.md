# unit-test-executor Specification（workflow-file-inventory 增量）

## ADDED Requirements

### Requirement: 突变测试范围改读 change 文件清单

`plugins/dev-team/bin/src/commands/test-execution.ts` 的突变 scope 解析 SHALL 从读取 git 工作区 diff 改为读取目标 change 的 `workflow.json` 的 `files.written`，且 SHALL 由既有 CLI 选项 `--change=<change-name>` 直接触发：

- `--change=<change-name>` SHALL 兼作清单突变 scope 的唯一入口（该参数现仅用于定位 reports 目录，本变更后兼作突变 scope 来源）：传入该参数时，突变 scope SHALL 自动取自 `openspec/changes/<change>/workflow.json` 的 `files.written`（叠加净归零去噪过滤，见下条要求），调用方 MUST NOT 被要求另行传入任何 scope 选项
- CLI SHALL NOT 引入 `--mutation-scope` 或任何新的清单语义选项；既有 `--mutation-diff-only` 选项 SHALL 整体删除（含其 git diff 数据路径与 `--mutation-diff-only:` stdout 诊断前缀），突变 scope 解析 MUST NOT 再依赖 git
- `getGitDiffFiles`（`lib/git.ts`）SHALL 停止被引用；测试文件 diff 反推同位源文件的 `expandMutationDiffWithInferredSources` 行为 SHALL 保留（对 `written` 中的测试文件同样反推）
- 目标 change 的 `workflow.json` 缺失 `files` 字段时 SHALL 硬报错（"该 change 创建于文件清单机制之前，请重建"），MUST NOT 回退 git diff
- `--skip-mutation` SHALL 保留：显式跳过突变阶段的逃生口不变；突变 scope 解析 SHALL 惰性执行（仅在突变阶段实际运行时读取清单），`--skip-mutation` 时 MUST NOT 因清单缺失或非法而失败

#### Scenario: 传 --change 自动读清单

- **WHEN** 执行 `dev-team test-execution --change=my-change`，该 change 的 `files.written` 为 `["src/a.ts", "src/a.test.ts"]`
- **THEN** 突变 scope 含 `src/a.ts` 及由 `src/a.test.ts` 反推的同位源文件
- **AND** 调用方未传任何 scope 类选项
- **AND** 进程不再调用 `git add -N` / `git reset` / `git diff HEAD`

#### Scenario: scope 选项不存在且 --skip-mutation 保留

- **WHEN** 检查 `dev-team test-execution` 的 CLI 选项定义
- **THEN** 不存在 `--mutation-scope` 与 `--mutation-diff-only`
- **AND** `--skip-mutation` 仍存在且语义不变（跳过突变阶段）

#### Scenario: skip-mutation 时不受清单缺失影响

- **WHEN** 以 `--change=<机制前旧 change> --skip-mutation` 执行，该 change 的 `workflow.json` 无 `files` 字段
- **THEN** 突变阶段被跳过，命令不因清单缺失而失败

#### Scenario: 旧 change 硬报错

- **WHEN** 以 `--change=<机制前旧 change>` 执行且未跳过突变，其 `workflow.json` 无 `files` 字段
- **THEN** 命令以错误退出，文案含重建指引
- **AND** MUST NOT 回退 git diff

### Requirement: 突变范围叠加净归零去噪过滤

清单模式 SHALL 在 `files.written` 基础上叠加薄过滤：内容与 `HEAD` 版本相同的文件（write 后又 revert 回原内容的净归零文件）SHALL 被排除出突变 scope——这类文件的突变体纯属噪声。过滤 SHALL 只作优化、不作范围权威：被过滤文件的实现正确性仍由 evaluator 内容核对承担。

#### Scenario: 净归零文件被过滤

- **WHEN** `files.written` 含 `src/b.ts` 且其工作区内容与 `HEAD` 版本一致
- **THEN** `src/b.ts` 不进入突变 scope

#### Scenario: 内容有差异的文件保留

- **WHEN** `files.written` 含 `src/a.ts` 且内容与 `HEAD` 不同
- **THEN** `src/a.ts` 进入突变 scope

## Module Contract

### Module: `plugins/dev-team/bin/src/cli.ts`（增量）

| 符号 | 变更 |
|------|------|
| `--mutation-diff-only` 选项 | **REMOVED**（连同 `mutationDiffOnly` 透传） |
| `--mutation-scope` | **MUST NOT 引入** |
| `--change` 选项 | **MODIFIED** 兼作突变 scope 入口（既有 reports 目录定位职责不变） |
| `--skip-mutation` 选项 | **不变**（继续映射 `noMutation`） |

### Module: `plugins/dev-team/bin/src/commands/test-execution.ts`（增量）

| 符号 | 变更 |
|------|------|
| `resolveMutationDiffFiles` | **MODIFIED** 数据源 `getGitDiffFiles` → `workflow.json.files.written` + 净归零过滤；触发条件 `--mutation-diff-only` → 既有 `--change` 参数；git 依赖移除；惰性解析（跳过突变时不读取） |
| stdout 诊断行 | **REMOVED** `--mutation-diff-only:` 前缀（可由清单来源诊断行替代） |
| `lib/git.ts` import | **REMOVED** |

### Module: `plugins/dev-team/bin/src/lib/git.ts`

| 符号 | 变更 |
|------|------|
| `getGitDiffFiles` | **REMOVED**（连同 `git.test.ts`；`simple-git` 依赖一并评估移除） |
