# 任务: rename-mcp-tool-slash-to-underscore

> **变更**: rename-mcp-tool-slash-to-underscore
> **日期**: 2026-06-03

---

## Phase 1: MCP Server 源码重命名

- [x] 修改 `plugins/dev-team/bin/src/mcp.ts` 中 11 处 `server.registerTool` 调用的第一个参数（tool 名称），将 `xx/yy` 替换为 `xx_yy`：
  - `phase/log` -> `phase_log`
  - `phase/check` -> `phase_check`
  - `phase/next` -> `phase_next`
  - `archi/query` -> `archi_query`
  - `archi/validate` -> `archi_validate`
  - `archi/write` -> `archi_write`
  - `archi/check` -> `archi_check`
  - `config/get` -> `config_get`
  - `config/set` -> `config_set`
  - `config/unset` -> `config_unset`
  - `config/context` -> `config_context`

## Phase 2: 重新构建编译产物

- [x] 在 `plugins/dev-team/bin/` 目录下执行 `npm run build`（或 `vp pack`）重新构建 `dev-team-mcp.cjs`
- [x] 验证构建产物：grep 搜索 `dev-team-mcp.cjs` 确认无 `xx/yy` 格式的 MCP tool 名称残留

## Phase 3: Skill 文件引用更新

- [x] 更新 `plugins/dev-team/skills/workflow-requirement/SKILL.md` — 将所有 `mcp__plugin_dev-team_dev-team__phase/next` 和 `mcp__plugin_dev-team_dev-team__phase/check` 引用中的 `/` 替换为 `_`
- [x] 更新 `plugins/dev-team/skills/phase-proposal/SKILL.md` — 将 `mcp__plugin_dev-team_dev-team__phase/check` 中的 `/` 替换为 `_`
- [x] 更新 `plugins/dev-team/skills/phase-dev-design/SKILL.md` — 将 `mcp__plugin_dev-team_dev-team__phase/check` 中的 `/` 替换为 `_`
- [x] 更新 `plugins/dev-team/skills/phase-test-design/SKILL.md` — 将 `mcp__plugin_dev-team_dev-team__phase/check` 中的 `/` 替换为 `_`
- [x] 更新 `plugins/dev-team/skills/phase-test-gen/SKILL.md` — 将 `mcp__plugin_dev-team_dev-team__phase/check` 中的 `/` 替换为 `_`
- [x] 更新 `plugins/dev-team/skills/phase-implement/SKILL.md` — 将 `mcp__plugin_dev-team_dev-team__phase/check` 中的 `/` 替换为 `_`
- [x] 更新 `plugins/dev-team/skills/phase-unit-test/SKILL.md` — 将 `mcp__plugin_dev-team_dev-team__phase/log` 中的 `/` 替换为 `_`
- [x] 更新 `plugins/dev-team/skills/phase-code-review/SKILL.md` — 将 `mcp__plugin_dev-team_dev-team__phase/check` 中的 `/` 替换为 `_`
- [x] 更新 `plugins/dev-team/skills/phase-integration-test/SKILL.md` — 将 `mcp__plugin_dev-team_dev-team__phase/log` 中的 `/` 替换为 `_`
- [x] 更新 `plugins/dev-team/skills/phase-acceptance/SKILL.md` — 将 `mcp__plugin_dev-team_dev-team__phase/check` 中的 `/` 替换为 `_`
- [x] 更新 `plugins/dev-team/skills/openspec-archive-change/SKILL.md` — 将 `mcp__plugin_dev-team_dev-team__phase/check` 中的 `/` 替换为 `_`

## Phase 4: Agent 文件引用更新

- [x] 更新 `plugins/dev-team/agents/architecture.md` — 将所有 `mcp__plugin_dev-team_dev-team__archi/query`、`archi/validate`、`archi/write`、`archi/check` 引用中的 `/` 替换为 `_`
- [x] 更新 `plugins/dev-team/agents/proposal-evaluator.md` — 将 `mcp__plugin_dev-team_dev-team__phase/log` 中的 `/` 替换为 `_`
- [x] 更新 `plugins/dev-team/agents/dev-design-evaluator.md` — 将 `mcp__plugin_dev-team_dev-team__phase/log` 中的 `/` 替换为 `_`
- [x] 更新 `plugins/dev-team/agents/test-design-evaluator.md` — 将 `mcp__plugin_dev-team_dev-team__phase/log` 中的 `/` 替换为 `_`
- [x] 更新 `plugins/dev-team/agents/test-gen-evaluator.md` — 将 `mcp__plugin_dev-team_dev-team__phase/log` 中的 `/` 替换为 `_`
- [x] 更新 `plugins/dev-team/agents/implementation-evaluator.md` — 将 `mcp__plugin_dev-team_dev-team__phase/log` 中的 `/` 替换为 `_`
- [x] 更新 `plugins/dev-team/agents/unit-test-evaluator.md` — 将 `mcp__plugin_dev-team_dev-team__phase/log` 中的 `/` 替换为 `_`
- [x] 更新 `plugins/dev-team/agents/code-review-evaluator.md` — 将 `mcp__plugin_dev-team_dev-team__phase/log` 中的 `/` 替换为 `_`
- [x] 更新 `plugins/dev-team/agents/integration-test-evaluator.md` — 将 `mcp__plugin_dev-team_dev-team__phase/log` 中的 `/` 替换为 `_`
- [x] 更新 `plugins/dev-team/agents/acceptance-evaluator.md` — 将 `mcp__plugin_dev-team_dev-team__phase/log` 中的 `/` 替换为 `_`

## Phase 5: settings.local.json 权限更新

- [x] 更新 `.claude/settings.local.json` — 将 `"mcp__plugin_dev-team_dev-team__eval/check"` 替换为 `"mcp__plugin_dev-team_dev-team__phase_check"`
- [x] 更新 `.claude/settings.local.json` — 将 `"mcp__plugin_dev-team_dev-team__eval/log"` 替换为 `"mcp__plugin_dev-team_dev-team__phase_log"`
- [x] 验证 `.claude/settings.local.json` 中无残留 `eval/` 前缀的 MCP tool 权限条目

## Phase 6: Spec 文件引用更新

- [x] 更新 `openspec/specs/mcp-tool-namespace/spec.md` — 将所有 tool 名称引用从 `xx/yy` 更新为 `xx_yy`（包括 Requirement 标题、Scenario、Module Contract 中的工具名称和映射表）
- [x] 更新 `openspec/specs/eval-check-cli/spec.md` — 将 `phase/check` 和 `phase/log` 引用更新为 `phase_check` 和 `phase_log`
- [x] 更新 `openspec/specs/phase-skills/spec.md` — 将所有 `mcp__plugin_dev-team_dev-team__phase/next` 和 `mcp__plugin_dev-team_dev-team__phase/check` 引用更新为下划线格式
- [x] 更新 `openspec/specs/phase-agents/spec.md` — 将所有 `mcp__plugin_dev-team_dev-team__phase/log` 引用更新为 `phase_log`
- [x] 更新 `openspec/specs/config-get/spec.md` — 将所有 `config/get` 引用更新为 `config_get`（包括 Tool name 和 Registration 部分）
- [x] 更新 `openspec/specs/config-set/spec.md` — 将所有 `config/set` 引用更新为 `config_set`
- [x] 更新 `openspec/specs/config-unset/spec.md` — 将所有 `config/unset` 引用更新为 `config_unset`
- [x] 更新 `openspec/specs/config-context/spec.md` — 将所有 `config/context` 引用更新为 `config_context`
- [x] 更新 `openspec/specs/workflow-orchestration/spec.md` — 将 `mcp__plugin_dev-team_dev-team__phase/next` 引用更新为 `phase_next`
- [x] 更新 `openspec/changes/rename-mcp-tool-slash-to-underscore/specs/*/spec.md` 中所有斜杠格式工具名称引用为下划线格式（delta specs 中已使用正确新格式）

## Phase 7: 版本号更新

- [x] 更新 `plugins/dev-team/.claude-plugin/plugin.json` 中的版本号从 `2.5.4` 到 `2.6.0`

## Phase 8: 全局验证

- [x] 全局 grep 搜索 `mcp__plugin_dev-team_dev-team__\w+/\w+` 模式，确认无残留斜杠格式的 FQN 引用（排除 `node_modules` 和归档目录 `openspec/changes/archive/`）
- [x] 全局 grep 搜索 `registerTool\('\w+/\w+'` 模式，确认 `mcp.ts` 中无斜杠格式 tool 名称
- [x] 全局 grep 搜索 `.claude/settings.local.json` 确认无 `eval/` 前缀的 MCP tool 条目
- [x] 运行 `cd plugins/dev-team/bin && npm run check` 确保代码风格和类型检查通过（仅找到预存错误，非本次变更引入）
