# 测试设计: rename-mcp-tool-slash-to-underscore

> **变更**: rename-mcp-tool-slash-to-underscore
> **日期**: 2026-06-03
> **基于**: proposal.md, design.md

---

## 1. 测试层级

| 层级 | 范围 | 框架 | 目标 |
|------|------|------|------|
| 单元测试（静态引用验证） | 验证所有源码、技能文件、agent 文件、配置文件和 spec 文件中的 MCP tool 名称均已从 `xx/yy` 更新为 `xx_yy`，无残留旧名称 | Python 3 `unittest` + `subprocess` (grep 封装) | 100% 覆盖 11 个 tool 名称在 6 类文件（`mcp.ts`, `dev-team-mcp.cjs`, skills, agents, settings.local.json, specs）中的引用正确性 |
| 单元测试（构建验证） | 验证 `plugins/dev-team/bin/dev-team-mcp.cjs` 重新构建后产物中仅包含下划线格式的 MCP tool 名称，且排除 description 文本字段中的交叉引用假阳性 | Python 3 `unittest` + `subprocess` (`npm run build`) | 构建产物中 11 个 tool 名称无残留斜杠格式，仅检查实际 tool 注册字符串而非描述文本 |
| 集成测试（MCP Server 冒烟测试） | 验证 MCP Server 启动后 `tools/list` 返回的 11 个 tool 名称均为下划线格式，且每个 tool 均可被 `tools/call` 正确路由；验证 tool-not-found 通过 `result.isError` content 级别判断而非 JSON-RPC error | Node.js `child_process` (或 `@modelcontextprotocol/sdk` client) + Python `subprocess` | 11 个工具全部可注册、可发现、可调用 |

---

## 2. 覆盖映射

| 需求ID | 测试文件 | 测试层级 | 覆盖类型 |
|--------|---------|----------|----------|
| AC-1 | `tests/test_source_references.py` (test_all_11_tools_use_underscore) | 单元（静态引用验证） | 正向覆盖 — 验证 `mcp.ts` 中 11 个 `registerTool` 名称均为 `xx_yy` 格式 |
| AC-1 | `tests/test_source_references.py` (test_no_registerTool_contains_slash) | 单元（静态引用验证） | 逆向覆盖 — 验证 `mcp.ts` 中无 `registerTool` 参数含 `/` |
| AC-2 | `tests/test_spec_references.py` (test_mcp_tool_namespace_spec_uses_underscore) | 单元（静态引用验证） | 正向覆盖 — 验证 `mcp-tool-namespace/spec.md` 中的工具名称引用均为 `xx_yy` 格式 |
| AC-3 | `tests/test_build_artifact.py` (test_cjs_has_underscore_names) | 单元（构建验证） | 正向覆盖 — 验证重建后 `dev-team-mcp.cjs` 包含 11 个下划线 tool 名称；检查对象为 tool 注册字符串（如 `registerTool` / tool 定义中的名称字段），排除 description 等文本字段中的交叉引用 |
| AC-3 | `tests/test_build_artifact.py` (test_cjs_no_slash_tool_names) | 单元（构建验证） | 逆向覆盖 — 验证重建后产物的工具注册路由中无 `xx/yy` 格式；仅检测实际 tool 名称标识符，排除 description 文本字段中可能存在的旧名交叉引用（如 `phase/next` 出现在另一 tool 的说明中） |
| AC-4 | `tests/test_skill_references.py` (test_all_skills_use_underscore_fqn) | 单元（静态引用验证） | 正向覆盖 — 验证 11 个 skill 文件中的 MCP FQN 引用均使用下划线分隔符 |
| AC-4 | `tests/test_skill_references.py` (test_no_skill_contains_slash_fqn) | 单元（静态引用验证） | 逆向覆盖 — 验证 11 个 skill 文件中无 `mcp__plugin_dev-team_dev-team__\w+/\w+` 模式 |
| AC-5 | `tests/test_agent_references.py` (test_all_agents_use_underscore_fqn) | 单元（静态引用验证） | 正向覆盖 — 验证 10 个 agent 文件中的 MCP FQN 引用均使用下划线分隔符 |
| AC-5 | `tests/test_agent_references.py` (test_no_agent_contains_slash_fqn) | 单元（静态引用验证） | 逆向覆盖 — 验证 10 个 agent 文件中无 `mcp__plugin_dev-team_dev-team__\w+/\w+` 模式 |
| AC-6 | `tests/test_settings_config.py` (test_settings_new_names_correct) | 单元（静态引用验证） | 正向覆盖 — 验证 `settings.local.json` 中权限条目使用 `phase_check`、`phase_log` 等新名称 |
| AC-7 | `tests/test_source_references.py` (test_phase_tools_all_underscore) | 单元（静态引用验证） | 分组覆盖 — 验证所有 `phase_` 系列 tool 在 `mcp.ts` 和文档中均为下划线格式 |
| AC-8 | `tests/test_source_references.py` (test_archi_tools_all_underscore) | 单元（静态引用验证） | 分组覆盖 — 验证所有 `archi_` 系列 tool 在 `mcp.ts` 和文档中均为下划线格式 |
| AC-9 | `tests/test_source_references.py` (test_config_tools_all_underscore) | 单元（静态引用验证） | 分组覆盖 — 验证所有 `config_` 系列 tool 在 `mcp.ts` 和文档中均为下划线格式 |
| AC-10 | `tests/test_spec_references.py` (test_all_8_specs_updated) | 单元（静态引用验证） | 文件列表覆盖 — 验证 8 个相关 spec 文件中工具名称引用已更新；需排除 spec 文件中意图性的新旧名称映射对照表段落（如 Markdown 表格中同时列出新旧名称的文档部分） |
| AC-11 | `tests/test_settings_config.py` (test_no_eval_prefix_remaining) | 单元（静态引用验证） | 清理覆盖 — 验证 `settings.local.json` 中无残留 `eval/check`、`eval/log` 等旧条目 |
| AC-1 至 AC-11 | `tests/test_mcp_server_integration.py` | 集成测试 | 端到端冒烟 — 启动 MCP Server，通过 `tools/list` 获取已注册工具列表，验证 11 个工具名称均为 `xx_yy` 格式；tools/call 调用旧名时验证响应通过 `result.isError` content 级别表示工具不存在，而非依赖 JSON-RPC error 层级判断 |

---

## 3. 测试策略

### 3.1 方法

本变更为纯重命名操作，不涉及任何业务逻辑、参数 schema 或数据流变更。因此测试策略以**验证一致性**为核心：

1. **静态引用完整性** — 对所有涉及的文件类型（源码、技能、agent、配置、spec）执行模式匹配验证，确保旧名称 `xx/yy` 已被全面替换为 `xx_yy`，且无遗漏
2. **构建产物一致性** — 重新构建 MCP Server 并验证编译产物中的 tool 名称与源码一致
3. **运行时功能性** — 通过 MCP Client 协议直接与 MCP Server 通信，验证 tools/list 和 tools/call 在新名称下正常工作
4. **逆向验证** — 除正向确认新名称存在外，还通过 grep 确认旧名称模式不再出现，避免假阳性

### 3.2 测试分类

- **单元测试（静态引用验证）**: 不依赖外部服务，仅对文件内容进行 grep 模式匹配。使用 Python 3 的 `unittest` 框架封装 `re` 模块和 `subprocess` 调用，逐文件扫描并断言预期的工具名称模式存在/不存在。
- **单元测试（构建验证）**: 依赖本地 Node.js 和 `npm` 环境执行构建，然后对构建产物进行静态分析。属于 "构建-验证" 模式，无需 mock。
- **集成测试（MCP Server 冒烟测试）**: 依赖本地 `node` 运行时启动 `dev-team-mcp.cjs` 作为子进程，通过 MCP 协议（stdio transport）发送 `tools/list` 和 `tools/call` 请求，验证响应中的工具名称和路由。不使用 mock，使用实际构建产物。

### 3.3 模拟策略

| 测试类别 | 模拟策略 | 理由 |
|----------|----------|------|
| 静态引用验证 | 无需模拟 | 仅做文件内容正则匹配，不涉及任何运行时依赖 |
| 构建验证 | 无需模拟 | 构建是确定性操作，本地 npm 环境即可执行 |
| 集成冒烟测试 | 无需模拟外部服务；MCP Server 自身命令以空参数调用（预期返回参数校验错误而非 "tool not found"） | 仅验证工具名称的路由正确性，而非验证工具的业务逻辑。对于需要外部文件的 tool（如 `archi/query` 需指定 `element` 参数），使用最小的有效参数或预期会出错的参数来验证路由可达性 |

### 3.4 已知假阳性预防

基于回溯（backtrack）阶段发现的 4 个假阳性（false positive）测试失败，测试实现需遵循以下预防策略：

| # | 假阳性场景 | 根因 | 预防策略 |
|---|-----------|------|----------|
| FP-1 | spec 文件中包含新旧名称映射表（如 `proposal.md` 中的 `| phase/log | phase_log |` 表格） | 映射表是意图性文档内容，非残留旧名 | 测试 grep 时跳过映射表段落：识别 Markdown 表格行中包含并列新旧名称的行（如 `| xx/yy | xx_yy |` 模式）或文档中明确标注为 "旧名称 -> 新名称" 的段落 |
| FP-2 | CJS 构建产物的 description 文本字段中包含旧名交叉引用（如 A tool 的 description 中写 "See also phase/next"） | description 文本是文档性内容，非实际 tool 注册 | 仅检查 tool 注册/定义区域的字符串（如 `registerTool` 的 name 参数、tool 定义对象的 name 字段、路由 switch/case 的 key），跳过 description/description 等文本字段 |
| FP-3 | 构建成功但 `setUpClass` 中 `_build_error` 被赋值为空字符串 `""` 导致 `assertIsNone` 失败 | `assertIsNone("")` 失败因为空字符串不是 `None` | 使用 `assertEqual(result.returncode, 0)` 验证构建成功，而非对 `_build_error` 做 `assertIsNone`；构建状态应以 `returncode` 为准 |
| FP-4 | MCP SDK 对不存在的 tool 返回 `result.isError` 而非 JSON-RPC 级别 error | MCP SDK 将 tool-not-found 放在 content 级别，而非 JSON-RPC 协议级别 | 检查 `response.get("result", {}).get("isError")` 为 `true` 且 `result.content` 含错误消息，而非依赖 `response.get("error")` 的 JSON-RPC error 结构 |

---

## 4. 边界场景

| 场景 | 输入/条件 | 预期行为 | 测试文件 |
|------|----------|----------|----------|
| 旧名称是新名称的子串（如 `config/get` vs `config_get`） | 在旧名称 `config/get` 已被替换为 `config_get` 的上下文中进行全局 grep `config/` | `config/` 的匹配结果应为零，确认无将 `config_` 误判为 `config/` 的假阳性 | `tests/test_source_references.py` — `test_config_tools_all_underscore` |
| `eval/` 前缀的特殊迁移（`eval/check` -> `phase_check`，`eval/log` -> `phase_log`） | settings.local.json 中包含 `eval/check` 和 `eval/log` 旧条目的场景 | 旧 `eval/` 条目已全部清理，仅保留 `phase_check` 和 `phase_log` 新条目 | `tests/test_settings_config.py` — `test_no_eval_prefix_remaining` |
| FQN 中的分段斜杠未被误替换（如插件命名空间分隔符 `__`） | 在所有文件中搜索 `__` 分隔符是否被错误修改 | 双下划线 `__` 应保持不变，仅单斜杠 `/` 被替换为单下划线 `_` | `tests/test_skill_references.py` — `test_fqn_double_underscore_preserved` |
| 构建产物中字符串常量 vs 变量引用 | `dev-team-mcp.cjs` 中 tool 名称可能以字符串字面量或变量形式存在 | 无论以何种形式存在，构建产物中不应出现 `"phase/log"`、`"archi/query"` 等斜杠格式工具名称字符串 | `tests/test_build_artifact.py` — `test_cjs_no_slash_tool_names` |
| skill/agent 文件中的 FQN 注释引用 | skill/agent 文件可能同时包含 MCP FQN 的代码引用和 Markdown 说明文本 | 即使是 Markdown 说明文本中的工具 FQN 引用也应使用下划线格式 | `tests/test_skill_references.py` — `test_no_slash_in_fqn_any_context` |
| `registerTool` 调用的字符串参数中包含非工具名称的斜杠 | `mcp.ts` 中 `description` 字段可能包含斜杠字符（如 URLs） | 仅 `registerTool` 的第一个参数（工具名称）被重命名，description 中的斜杠不应被修改 | `tests/test_source_references.py` — `test_registerTool_description_slash_preserved` |
| 工具名称为空或仅下划线（不合法值，防御性验证） | 假设有人错误地将一个工具注册为空字符串或 `_` | 所有 11 个工具应具有非空、非纯下划线的有意义的名称（`phase_log`、`phase_check` 等） | `tests/test_source_references.py` — `test_all_tool_names_valid_identifier` |
| 大小写敏感性 | 在 grep 中忽略大小写搜索可能误匹配 | 所有工具名称均为小写 `snake_case`，验证大小写敏感模式下匹配正确 | `tests/test_source_references.py` — `test_all_tool_names_lowercase` |
| 同一文件同时包含新旧名称的混合状态 | 模拟迁移未完成的状态（部分文件已更新，部分未更新） | 验证工具应报告所有文件中的名称格式一致，无混合状态 | `tests/test_skill_references.py` — `test_all_skill_fqns_consistent` |
| **FP-1: spec 文件中的新旧名称映射表** | spec 文件中包含 Markdown 表格形式的映射表，如 `\| phase/log \| phase_log \|` 或 `xx/yy → xx_yy` 的文档说明 | 测试应识别并跳过这些意图性映射段落（通过检测行中是否同时包含斜杠和下划线格式的同一 tool 名称，或通过注释标记/代码块隔离），仅检查实际规范定义中的名称引用 | `tests/test_spec_references.py` — `test_all_8_specs_updated` |
| **FP-2: CJS 产物 description 文本中的旧名交叉引用** | CJS 产物中某 tool 的 description 字段包含 `phase/next` 作为对其他工具的引用（`"See phase/next for more details"`） | 测试应仅检查 tool 注册的定义名称区域（`name` 字段、`registerTool` 第一个参数），跳过 description 等文本说明字段 | `tests/test_build_artifact.py` — `test_cjs_no_slash_tool_names` |
| **FP-3: 构建成功但 `_build_error` 为空字符串** | `npm run build` 成功退出（exit code 0），但 `result.stderr` 返回空字符串 `""`，导致 `self._build_error` 为 `""` | 测试应检查 `result.returncode == 0` 而非 `assertIsNone(self._build_error)`。空字符串 `""` 表示 stderr 无输出，不等于构建失败 | `tests/test_build_artifact.py` — `test_cjs_rebuild_succeeds` |
| **FP-4: MCP SDK tool-not-found 返回 `result.isError`** | 调用不存在的 tool（如旧名 `phase/log`），MCP SDK 响应中无 JSON-RPC error，但 `result.isError` 为 `true` 且 `result.content` 包含错误消息 | 测试应检查 `response.get("result", {}).get("isError", False)` 为 `True`，或 `result.content` 包含 `"tool not found"` / `"unknown tool"` 等消息，而非依赖 JSON-RPC 级别 `response.get("error")` | `tests/test_mcp_server_integration.py` — `test_tools_call_slash_name_returns_error` |

---

## 5. 测试数据

本变更为纯重命名操作，无需外部测试数据文件。测试所需数据均来自项目文件自身的静态分析：

| 数据来源 | 用途 | 获取方式 |
|----------|------|----------|
| `plugins/dev-team/bin/src/mcp.ts` | 验证 registerTool 名称 | 直接文件读取 + 正则解析 |
| `plugins/dev-team/bin/dev-team-mcp.cjs` | 验证构建产物工具名称（仅检查注册字符串，排除 description 文本） | npm run build 后读取 |
| `plugins/dev-team/skills/*/SKILL.md` (11 个) | 验证 skill FQN 引用 | glob 匹配 + 文件读取 |
| `plugins/dev-team/agents/*.md` (10 个) | 验证 agent FQN 引用 | glob 匹配 + 文件读取 |
| `.claude/settings.local.json` | 验证权限条目 | JSON 解析 |
| `openspec/specs/mcp-tool-namespace/spec.md` (及另外 7 个 spec 文件) | 验证 spec 文档引用（排除意图性新旧名称映射表段落） | glob 匹配 + 文件读取 |

所有工具名称的预期值定义在测试代码的常量和列表中，直接来源于 proposal.md 的映射表：

```python
EXPECTED_TOOL_NAMES = [
    "phase_log", "phase_check", "phase_next",
    "archi_query", "archi_validate", "archi_write", "archi_check",
    "config_get", "config_set", "config_unset", "config_context",
]

EXPECTED_FQN_PREFIX = "mcp__plugin_dev-team_dev-team__"
```

### 假阳性过滤标记

spec 文件中意图性的新旧名称映射表段落应通过以下启发式规则在测试中跳过：

- Markdown 表格行中同时包含 `xx/yy` 和 `xx_yy` 的同一工具名称（如 `| phase/log | phase_log |`）
- 代码块（triple-backtick 包裹）中包含的映射表
- 行首包含 `>`（blockquote）的映射说明
- 明确标注为 "旧名称" / "新名称" / "迁移" / "映射" 的段落

CJS 产物中仅检查以下区域中的工具名称字符串：
- `registerTool` 调用的第一个字符串参数
- tool 定义对象的 `name` 属性值
- switch/case 中匹配 tool 名称的 case 标签

---

## 6. 不可测试项

- **MCP tool 的业务逻辑正确性** — 本变更不修改任何业务逻辑（`commands/*.ts`、`lib/*.ts`），因此不测试各 tool 的输入处理、schema 校验、输出格式等功能。**原因**: 不属于本变更范围，逻辑与重命名正交。
- **MCP Client 端的兼容性** — 不同 MCP Client（如 Claude Desktop、VS Code 扩展、自定义客户端）对工具名称中下划线的处理方式不在本变更可控范围内。**原因**: 属于 Client 端实现，无法在 Server 端测试。
- **重命名对存档变更的影响** — `openspec/changes/archive/` 目录中的旧设计文档和测试产物引用了旧工具名称。根据设计决策 D3，这些归档文件不应被修改。**原因**: 设计决策明确约定不修改归档，以避免破坏审计追踪。
- **CLI 命令 `dev-team eval-log` / `dev-team eval-check`** — 根据 proposal.md 的 "不要修改" 部分，CLI 命令不受本次 MCP tool 重命名影响。**原因**: CLI 命令是独立的入口点，与 `mcp.ts` 中的 tool 注册名称无直接关联。
- **`bin/openspec-bundled.js`** — 打包的 CLI 文件，不引用 MCP tool 名称。**原因**: 不在变更范围内。
