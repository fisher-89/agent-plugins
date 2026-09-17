# protect-files-hook Specification（workflow-file-inventory 增量）

## ADDED Requirements

### Requirement: PreToolUse 与 PostToolUse 共用 extractFileOps 提取器

`plugins/dev-team/bin/src/hooks.ts` SHALL 将 bash 侧 `extractBashWriteTargets` 与 PowerShell 侧 `extractPowerShellWriteTargets` 泛化为单一三态提取器 `extractFileOps(command) → Array<{ op: 'write' | 'delete' | 'revert', path }>`（分类规则见 `workflow-file-inventory` 变更），并让 PreToolUse 写保护改为消费该提取器：

- PreToolUse：提取路径 ∩ 保护 glob → deny（路径级精确匹配）
- PostToolUse：提取路径 → 过滤 → 归账文件清单

既有 python/node 命令豁免、fail-open 行为、内置与用户 glob 合并逻辑 SHALL 保持不变；`Write`/`Edit`/`Shell`/`StrReplace` 的既有拦截语义 SHALL 兼容。写保护由此从包含式命令判断升级为按提取路径匹配，对 `workflow.json` 的重定向写入仍 SHALL deny。

#### Scenario: 重定向写受保护路径仍被拒绝

- **WHEN** `tool_name` 为 `Bash` 且 command 为 `echo x > openspec/changes/x/workflow.json`
- **THEN** `permissionDecision: "deny"`

#### Scenario: 提取路径不匹配则放行

- **WHEN** `tool_name` 为 `Bash` 且 command 为 `echo x > src/notes.txt`
- **THEN** `permissionDecision: "allow"`

#### Scenario: 豁免语义不变

- **WHEN** command 为 `node scripts/setup.js openspec/config.json`
- **THEN** `permissionDecision: "allow"`（python/node 豁免保持）

### Requirement: 写保护扩展为写/删保护

PreToolUse 保护 SHALL 依据 `extractFileOps` 的 `delete` 分类同步拦截对受保护路径的删除：`rm <受保护路径>`、`Remove-Item`、`git rm` 等命中保护 glob 时 SHALL 输出 `permissionDecision: "deny"`，denial reason 沿用既有 `%s` / `%t` 占位符规则。

#### Scenario: rm 受保护文件被拒绝

- **WHEN** `tool_name` 为 `Bash` 且 command 为 `rm openspec/changes/x/workflow.json`
- **THEN** `permissionDecision: "deny"`
- **AND** denial reason 含该文件路径

#### Scenario: 删除非保护文件放行

- **WHEN** command 为 `rm src/tmp.txt`
- **THEN** `permissionDecision: "allow"`

### Requirement: 批量还原命令拦截

PreToolUse SHALL 拦截对 hook 不可见且对用户数据最危险的批量还原命令：`git stash`（含 `stash pop`/`stash apply` 对工作区的改写形态按 design 细化）与 `git clean`。拦截范围 SHALL 至少覆盖 `openspec/changes/**` 工作流产物（防止批量还原抹掉 proposal/design/reports 与文件清单）；`git restore` / `git checkout --` 对 `openspec/changes/**` 工作流产物的单路径还原 SHALL 同样被拦（作为清单机制认定的规范还原动作的保护区），对项目源码的 `git restore` SHALL 放行（它是可被记录的规范还原动作）。

#### Scenario: git clean 拦截

- **WHEN** `tool_name` 为 `Bash` 且 command 为 `git clean -fd openspec/changes/my-change`
- **THEN** `permissionDecision: "deny"`

#### Scenario: restore 工作流产物拦截

- **WHEN** command 为 `git restore openspec/changes/my-change/design.md`
- **THEN** `permissionDecision: "deny"`

#### Scenario: restore 源码放行

- **WHEN** command 为 `git restore src/a.ts`
- **THEN** `permissionDecision: "allow"`
- **AND** 该 revert 操作由 PostToolUse 记录器按折叠规则记录

## Module Contract

### Module: `plugins/dev-team/bin/src/hooks.ts`（增量）

| 符号 | 变更 |
|------|------|
| `extractFileOps` | **ADDED** 三态共享提取器（PreToolUse 与 PostToolUse 双消费） |
| `checkBashCommand` / `checkPowerShellCommand` | **MODIFIED** 改按 `extractFileOps` 提取路径匹配保护 glob；write 与 delete 双分类拦截 |
| 批量还原拦截 | **ADDED** `git stash` / `git clean` deny；`git restore` 按目标路径区分（openspec 工作流产物 deny、源码 allow） |
| 既有豁免 / fail-open / glob 合并 | 不变 |
