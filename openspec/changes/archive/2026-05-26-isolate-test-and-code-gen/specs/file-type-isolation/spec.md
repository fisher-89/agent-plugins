## ADDED Requirements

### Requirement: test-gen-generator 被约束仅读取设计文档文件
test-gen-generator 的 agent prompt SHALL 包含硬编码的文件类型黑名单，禁止读取项目源码文件。
黑名单 SHALL 至少包含以下文件扩展名：`.ts`、`.tsx`、`.js`、`.jsx`、`.py`、`.rs`、`.go`、`.java`、`.c`、`.cpp`、`.h`、`.hpp`、`.cs`、`.rb`、`.php`。
test-gen-generator SHALL 仅被允许读取：`.md`、`.template`、`.yaml`、`.yml`、`.json` 文件。
test-gen-generator 的 prompt SHALL 包含明确指示："你不得 Read、Grep、Glob 任何源码文件（.ts、.py、.js 等），你只能读取设计文档（.md、.template、.yaml、.json）"。

#### Scenario: test-gen-generator 拒绝读取源码文件
- **WHEN** test-gen-generator agent 被调用
- **THEN** 其 system prompt 中包含文件类型黑名单约束
- **AND** 如果 Agent 尝试读取 `.ts` 或 `.py` 文件，skill 层在校验时检测到违规并输出警告

### Requirement: implementation-generator 被约束禁止读取测试文件
implementation-generator 的 agent prompt SHALL 包含硬编码的目录黑名单，禁止读取测试目录下的文件。
黑名单目录 SHALL 至少包含：`tests/`、`__tests__/`、`test/`、`spec/`（以及它们的子目录）。
implementation-generator SHALL 仅被允许读取：`.md`、`.template`、`.yaml`、`.yml`、`.json` 文件以及 `src/`（或对应源码目录）中的非测试文件。
implementation-generator 的 prompt SHALL 包含明确指示："你不得 Read、Grep、Glob 任何 tests/ 目录下的文件"。

#### Scenario: implementation-generator 拒绝读取测试目录
- **WHEN** implementation-generator agent 被调用
- **THEN** 其 system prompt 中包含目录黑名单约束
- **AND** 如果 Agent 尝试读取 `tests/` 目录下的文件，skill 层在校验时检测到违规并输出警告

### Requirement: Generator 执行后的文件访问审计
skill 层在 Generator agent 执行完毕后 SHALL 执行文件访问审计。
审计 SHALL 检查 Generator 在本次执行中所有 Read、Grep、Glob 工具调用的路径参数。
如果发现任何路径匹配黑名单（文件类型或目录），skill SHALL 将该次 Generator 输出标记为违规，并在 eval.json 对应条目中添加违规记录。
如果违规次数达到阈值（默认 3 次），skill SHALL 阻止该次 Generator 输出被使用，并提示用户检查 Generator prompt。

#### Scenario: 文件访问审计无违规
- **WHEN** implementation-generator 执行完毕，skill 检查其所有文件读取路径
- **THEN** 如果所有读取路径均不在黑名单中，审计通过，Generator 输出正常进入后续流程

#### Scenario: 文件访问审计发现违规
- **WHEN** implementation-generator 执行了 Read 操作，路径为 `tests/unit/user.test.ts`
- **THEN** skill 检测到路径匹配 `tests/` 黑名单
- **AND** 在 eval.json 当前条目中添加违规记录 `{violation_type: "blacklisted_directory", path: "tests/unit/user.test.ts", count: 1}`
- **AND** 如果累计违规达到 3 次，输出提示 "Generator 连续违规 3 次，请检查 agent prompt 约束是否足够"

### Requirement: 隔离机制的独立开关
文件类型隔离机制 SHALL 可通过开关配置启用/禁用（用于调试或特殊场景）。
开关默认值为 `enabled: true`。
开关配置 SHALL 位于 `.claude-plugin/plugin.json` 中，路径为 `features.fileTypeIsolation.enabled`。
当开关关闭时，Generator agent prompt 中不注入文件类型黑名单约束，skill 层也不执行文件访问审计。

#### Scenario: 隔离启用
- **WHEN** `features.fileTypeIsolation.enabled` 为 true
- **THEN** test-gen/implement 阶段的 Generator prompt 包含文件类型黑名单，skill 层执行文件访问审计

#### Scenario: 隔离关闭
- **WHEN** `features.fileTypeIsolation.enabled` 为 false
- **THEN** test-gen/implement 阶段的 Generator prompt 不包含文件类型黑名单，skill 层跳过文件访问审计
