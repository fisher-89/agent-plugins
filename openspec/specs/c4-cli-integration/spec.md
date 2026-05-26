## ADDED Requirements

### Requirement: dev-team CLI 提供 archi 子命令
dev-team CLI SHALL 提供 `archi` 子命令，包含 4 个 action：`query`、`validate`、`write`、`check`。
所有 action SHALL 基于 `@likec4/core` 的 `LikeC4.fromSource()` 和 `LikeC4Model.Computed` API 实现 C4 模型解析，不再依赖 Python archi_parser 的 `parse_dsl()` 自实现。

#### Scenario: archi 命令注册到 CLI
- **WHEN** 用户运行 `dev-team --help`
- **THEN** 输出包含 `archi <action>` 子命令及其 action 列表（query / validate / write / check）

### Requirement: archi query 查询模型元素和关系
`dev-team archi query` SHALL 读取 `openspec/specs/architecture/models/*.c4` 聚合为 DSL 文本，通过 `LikeC4.fromSource()` 解析为 `LikeC4Model.Computed`，输出元素和关系 JSON。
未指定 `--element` 时 SHALL 输出所有元素和所有关系。
指定 `--element <fqn>` 时 SHALL 输出单个元素及其入/出关系。
若 `models/` 目录为空或无 .c4 文件，SHALL 输出 `{"error": "No model files found"}` 并退出码 1。

#### Scenario: 查询所有元素和关系
- **WHEN** 用户运行 `dev-team archi query`
- **THEN** 输出 JSON 包含 `elements` 数组和 `relationships` 数组
- **AND** 每个 element 包含 `name`、`kind`、`paths`、`metadata` 字段

#### Scenario: 查询指定元素的入出关系
- **WHEN** 用户运行 `dev-team archi query --element "App.AuthDomain.LoginModule"`
- **THEN** 输出 JSON 包含 `element` 对象和该元素所有 `incoming` / `outgoing` 关系数组

#### Scenario: 模型文件不存在
- **WHEN** `models/` 目录为空或无 .c4 文件
- **THEN** 输出 JSON `{"error": "No model files found"}` 并退出码 1

### Requirement: archi validate 校验 C4 DSL 语法
`dev-team archi validate` SHALL 通过 `LikeC4.fromSource()` 解析 DSL 并调用 `likec4.getErrors()` 获取诊断。
若 `--source` 参数提供了 DSL 文本，校验该文本。
若未提供 `--source`，读取 `models/*.c4` 聚合文本后校验。
校验通过 SHALL 输出 `{"valid": true}` 并退出码 0。
校验失败 SHALL 输出 errors 列表并退出码 1。

#### Scenario: 校验有效 DSL 文本
- **WHEN** 用户运行 `dev-team archi validate --source "<valid DSL>"`
- **THEN** 输出 `{"valid": true}` 并退出码 0

#### Scenario: 校验无效 DSL 文本
- **WHEN** 用户运行 `dev-team archi validate --source "<invalid DSL>"`
- **THEN** 输出 `{"valid": false, "errors": [...]}` 并退出码 1
- **AND** errors 数组包含具体的语法错误位置和描述

#### Scenario: 校验 models/ 中的现有模型文件
- **WHEN** 用户运行 `dev-team archi validate`（无 --source）
- **THEN** 读取 `models/*.c4` 聚合 DSL，校验并输出结果

### Requirement: archi write 校验并写入模型文件
`dev-team archi write --path <file> --source <dsl>` SHALL 先通过 `dev-team archi validate --source "<dsl>"` 校验 DSL，校验通过后再写入到 `openspec/specs/architecture/models/<file>`。
校验失败 SHALL NOT 写入，并输出错误信息。
`--path` 目标必须在 `models/` 目录内，写入路径超出 models/ 时 SHALL 拒绝。

#### Scenario: 校验通过后写入
- **WHEN** 用户运行 `dev-team archi write --path "models/03-payments.c4" --source "<valid DSL>"`
- **THEN** DSL 通过校验后写入 `openspec/specs/architecture/models/03-payments.c4`
- **AND** 输出 `{"success": true, "path": "<full_path>"}`

#### Scenario: 校验失败不写入
- **WHEN** 用户运行 `dev-team archi write --path "models/03-payments.c4" --source "<invalid DSL>"`
- **THEN** 文件不被写入，输出 `{"success": false, "error": "..."}`

#### Scenario: 路径超出 models/ 目录被拒绝
- **WHEN** `--path` 为 `"../outside.c4"` 或绝对路径指向非 models/ 目录
- **THEN** 输出错误并退出码 1

### Requirement: archi check 执行 import 交叉引用验证
`dev-team archi check` SHALL 实现原 `archi-validate.py` 的 import 交叉引用逻辑：
1. 通过 `@likec4/core` 加载模型，从 `metadata.path` 构建 `path_to_element` 映射
2. 获取变更文件列表（`--staged` 从 git diff --cached 获取，`--files <list>` 使用指定列表）
3. 解析每个变更文件的 import 语句（TS/JS/Python）
4. 将文件映射到模型元素，将 import 解析为元素间依赖
5. 交叉引用：检测 unmodeled_dependency、unmapped_import_target、unused_relationship、path_not_found
6. 输出 violations 和 warnings 数组

#### Scenario: 检测到未建模的依赖
- **WHEN** 变更文件 A（属于 element X）import 了变更文件 B（属于 element Y），但模型中没有 `X -> Y` 关系
- **THEN** violations 数组包含 `type: "unmodeled_dependency"` 条目，detail 指明 X 和 Y

#### Scenario: import 目标不在模型内
- **WHEN** 变更文件的 import 目标解析后的路径不匹配任何元素的 `metadata.path`
- **THEN** warnings 数组包含 `type: "unmapped_import_target"` 条目

#### Scenario: 模型声明了但代码中未使用的依赖
- **WHEN** 模型中有 `A -> B` 关系，但变更集中无对应的 import 证据
- **THEN** warnings 数组包含 `type: "unused_relationship"` 条目

#### Scenario: 无变更文件时跳过
- **WHEN** `--staged` 或 `--files` 返回空文件列表
- **THEN** 输出 `{"status": "no_changes", "changed_files": []}`

#### Scenario: 无模型文件时跳过
- **WHEN** `models/` 目录为空
- **THEN** 输出 `{"status": "skipped", "reason": "no model to validate against"}`

### Requirement: @likec4/core 作为单一解析引擎
dev-team CLI SHALL 使用 `@likec4/core` 的 `LikeC4.fromSource(dslString)` 作为 C4 DSL 的唯一解析引擎。
`archi_parser.py` 的 `parse_dsl()`、`validate_structure()`、`try_parse_element()`、`parse_relationship()`、`_parse_metadata_kv()` 等自实现逻辑 SHALL 不再存在。
C4 语法兼容性由 `@likec4/core` 官方实现保证（100% 兼容 LikeC4 DSL，含 views、deployment、tags、dynamic views、import 等特性）。

#### Scenario: 解析包含 views 块的 C4 DSL
- **WHEN** DSL 文本包含 `views { view View1 { include [...] } }` 块
- **THEN** `@likec4/core` 正确解析，views 信息可通过 `model.views()` 获取

#### Scenario: 解析包含 deployment 的 C4 DSL
- **WHEN** DSL 文本包含 `deployment { node Server { ... } }` 块
- **THEN** `@likec4/core` 正确解析，deployment 信息可通过 `model.deployment` 获取
