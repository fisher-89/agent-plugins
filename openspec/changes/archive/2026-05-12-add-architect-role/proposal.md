## Why

当前项目缺少对软件架构的结构化管理。OpenSpec 管理"做什么"（specs）和"怎么做"（changes），但没有管理"系统长什么样"（architecture）。代码变更可能违反架构约束而不被察觉，架构知识散落在团队成员的脑海中，无法版本化、可验证地演进。

引入 likec4 作为架构建模工具，配合 archi-* 子代理技能和提交时验证钩子，使架构成为可版本化、可查询、可验证的代码资产。

## What Changes

- 新增 `openspec/architecture/` 目录结构，存放 likec4 模型、视图、ADR 记录和验证报告
- 新增 `archi-model` 子代理技能：通过 likec4 API 结构化读取模型，通过 DSL 文本编辑 + `fromSource()` 验证语法后写入
- 新增 `archi-validate` 子代理技能：解析变更文件的 import 依赖，交叉比对模型关系，生成 JSON 验证报告
- 新增 `archi-decide` 子代理技能：创建和维护架构决策记录（ADR），包含影响范围字段引用模型元素
- 新增 PreToolUse 提交钩子：拦截 `git commit`，检查所有变更文件是否有对应验证报告，无报告则拒绝提交

## Capabilities

### New Capabilities

- `architecture-model`: 架构模型的查询与修改。通过 likec4 API 读取模型结构，通过 DSL 文本编辑写入变更，元素通过 `metadata.path` 映射代码目录。
- `architecture-validation`: 提交时的架构一致性验证。解析变更文件的 import 语句，交叉比对模型中的 relationships，检测未建模依赖、过时关系和遗漏元素，输出 JSON 报告。
- `architecture-decisions`: 架构决策记录。创建和维护 ADR，按日期命名，包含背景、决策、后果、备选方案和影响范围字段。
- `architecture-commit-gate`: 提交时架构验证门禁。拦截 git commit，检查所有变更文件是否已验证，无报告则拒绝并引导代理运行 archi-validate。

### Modified Capabilities

<!-- 不修改任何现有 capability -->

## Impact

- 新增插件目录：`plugins/dev-team/skills/archi-model/`, `plugins/dev-team/skills/archi-validate/`, `plugins/dev-team/skills/archi-decide/`
- 新增钩子脚本：`plugins/dev-team/hooks/pre-tool-architecture-gate.py`
- 新增工具脚本：`plugins/dev-team/utils/archi-validate.py`, `plugins/dev-team/utils/archi-model.py`, `plugins/dev-team/utils/archi-decide.py`
- 新增模板：`plugins/dev-team/templates/adr.md`, `plugins/dev-team/templates/validate-report.json`
- 修改 `plugins/dev-team/hooks/hooks.json`：新增 PreToolUse(Bash) git commit 拦截规则
- 新增依赖：`likec4` npm 包（通过 `npx likec4` 使用其 CLI/API）
- 新增子代理定义：`plugins/dev-team/agents/archi-model.md`, `plugins/dev-team/agents/archi-validate.md`, `plugins/dev-team/agents/archi-decide.md`
- 不修改现有钩子和工具
