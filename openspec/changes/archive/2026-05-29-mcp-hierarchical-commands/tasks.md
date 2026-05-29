# 实施任务: mcp-hierarchical-commands

> **变更**: mcp-hierarchical-commands
> **日期**: 2026-05-29

---

## 阶段一: MCP Server 核心变更

这些任务是所有其他任务的基础——`mcp.ts` 中的 tool name 定义是引用文件中的完全限定名的来源。

- [x] **T1.1**: 修改 `plugins/dev-team/bin/src/mcp.ts` 中 TOOLS 数组的 6 个 tool `name` 字段：
  - `"eval_log"` → `"eval/log"`
  - `"eval_check"` → `"eval/check"`
  - `"archi_query"` → `"archi/query"`
  - `"archi_validate"` → `"archi/validate"`
  - `"archi_write"` → `"archi/write"`
  - `"archi_check"` → `"archi/check"`
- [x] **T1.2**: 更新 `plugins/dev-team/bin/src/mcp.ts` 中 `handleToolCall` switch 的 6 个 case 标签，匹配新 tool 名称：
  - `case "eval_log":` → `case "eval/log":`
  - `case "eval_check":` → `case "eval/check":`
  - `case "archi_query":` → `case "archi/query":`
  - `case "archi_validate":` → `case "archi/validate":`
  - `case "archi_write":` → `case "archi/write":`
  - `case "archi_check":` → `case "archi/check":`

## 阶段二: 编译产物重建

- [x] **T2.1**: 在 `plugins/dev-team/bin/` 目录执行构建命令（`npm run build` 或等效命令），重新生成 `dev-team-mcp.cjs`
- [x] **T2.2**: 验证 `plugins/dev-team/bin/dev-team-mcp.cjs` 中：
  - TOOLS 数组的 `name` 字段使用 `xx/yy` 格式（无 `xx_yy` 残留）
  - switch case 标签使用 `xx/yy` 格式

## 阶段三: Skill 文件 MCP tool 引用更新

- [x] **T3.1**: 更新 `plugins/dev-team/skills/phase-requirements/SKILL.md`：
  - `mcp__plugin_dev-team_dev-team__eval_check` → `mcp__plugin_dev-team_dev-team__eval/check`
- [x] **T3.2**: 更新 `plugins/dev-team/skills/phase-dev-design/SKILL.md`：
  - `mcp__plugin_dev-team_dev-team__eval_check` → `mcp__plugin_dev-team_dev-team__eval/check`
- [x] **T3.3**: 更新 `plugins/dev-team/skills/phase-test-design/SKILL.md`：
  - `mcp__plugin_dev-team_dev-team__eval_check` → `mcp__plugin_dev-team_dev-team__eval/check`
- [x] **T3.4**: 更新 `plugins/dev-team/skills/phase-test-gen/SKILL.md`：
  - `mcp__plugin_dev-team_dev-team__eval_check` → `mcp__plugin_dev-team_dev-team__eval/check`
- [x] **T3.5**: 更新 `plugins/dev-team/skills/phase-implement/SKILL.md`：
  - `mcp__plugin_dev-team_dev-team__eval_check` → `mcp__plugin_dev-team_dev-team__eval/check`
- [x] **T3.6**: 更新 `plugins/dev-team/skills/phase-unit-test/SKILL.md`：
  - `mcp__plugin_dev-team_dev-team__eval_log` → `mcp__plugin_dev-team_dev-team__eval/log`
- [x] **T3.7**: 更新 `plugins/dev-team/skills/phase-code-review/SKILL.md`：
  - `mcp__plugin_dev-team_dev-team__eval_check` → `mcp__plugin_dev-team_dev-team__eval/check`
- [x] **T3.8**: 更新 `plugins/dev-team/skills/phase-integration-test/SKILL.md`：
  - `mcp__plugin_dev-team_dev-team__eval_log` → `mcp__plugin_dev-team_dev-team__eval/log`
- [x] **T3.9**: 更新 `plugins/dev-team/skills/phase-acceptance/SKILL.md`：
  - `mcp__plugin_dev-team_dev-team__eval_check` → `mcp__plugin_dev-team_dev-team__eval/check`
- [x] **T3.10**: 更新 `plugins/dev-team/skills/openspec-archive-change/SKILL.md`：
  - `mcp__plugin_dev-team_dev-team__eval_check` → `mcp__plugin_dev-team_dev-team__eval/check`

## 阶段四: Agent 文件 MCP tool 引用更新

- [x] **T4.1**: 更新 `plugins/dev-team/agents/architecture.md` 中所有 MCP tool 引用：
  - `eval_log` → `eval/log`（description 和代码示例中的 FQN）
  - `eval_check` → `eval/check`（如果有，description 中）
  - `archi_query` → `archi/query`（description 和代码示例中的短名）
  - `archi_validate` → `archi/validate`
  - `archi_write` → `archi/write`
  - `archi_check` → `archi/check`
  - 注意更新 `mcp__plugin_dev-team_dev-team__archi_*` → `mcp__plugin_dev-team_dev-team__archi/*`
- [x] **T4.2**: 更新 `plugins/dev-team/agents/requirements-evaluator.md`：
  - `mcp__plugin_dev-team_dev-team__eval_log` → `mcp__plugin_dev-team_dev-team__eval/log`
  - 描述文字中的 `eval_log` → `eval/log`
- [x] **T4.3**: 更新 `plugins/dev-team/agents/dev-design-evaluator.md`：
  - `mcp__plugin_dev-team_dev-team__eval_log` → `mcp__plugin_dev-team_dev-team__eval/log`
- [x] **T4.4**: 更新 `plugins/dev-team/agents/test-design-evaluator.md`：
  - `mcp__plugin_dev-team_dev-team__eval_log` → `mcp__plugin_dev-team_dev-team__eval/log`
- [x] **T4.5**: 更新 `plugins/dev-team/agents/test-gen-evaluator.md`：
  - `mcp__plugin_dev-team_dev-team__eval_log` → `mcp__plugin_dev-team_dev-team__eval/log`
- [x] **T4.6**: 更新 `plugins/dev-team/agents/implementation-evaluator.md`：
  - `mcp__plugin_dev-team_dev-team__eval_log` → `mcp__plugin_dev-team_dev-team__eval/log`
- [x] **T4.7**: 更新 `plugins/dev-team/agents/unit-test-evaluator.md`：
  - `mcp__plugin_dev-team_dev-team__eval_log` → `mcp__plugin_dev-team_dev-team__eval/log`
- [x] **T4.8**: 更新 `plugins/dev-team/agents/code-review-evaluator.md`：
  - `mcp__plugin_dev-team_dev-team__eval_log` → `mcp__plugin_dev-team_dev-team__eval/log`
- [x] **T4.9**: 更新 `plugins/dev-team/agents/integration-test-evaluator.md`：
  - `mcp__plugin_dev-team_dev-team__eval_log` → `mcp__plugin_dev-team_dev-team__eval/log`
- [x] **T4.10**: 更新 `plugins/dev-team/agents/acceptance-evaluator.md`：
  - `mcp__plugin_dev-team_dev-team__eval_log` → `mcp__plugin_dev-team_dev-team__eval/log`

## 阶段五: 权限配置更新

- [x] **T5.1**: 更新 `.claude/settings.local.json` 中 `permissions.allow` 数组：
  - `"mcp__plugin_dev-team_dev-team__eval_check"` → `"mcp__plugin_dev-team_dev-team__eval/check"`
  - `"mcp__plugin_dev-team_dev-team__eval_log"` → `"mcp__plugin_dev-team_dev-team__eval/log"`

## 阶段六: 全面验证

- [x] **T6.1**: 全局搜索确认无旧名称遗留：
  - `grep -rn "eval_log" plugins/dev-team/` — 确认 0 匹配
  - `grep -rn "eval_check" plugins/dev-team/` — 确认 0 匹配
  - `grep -rn "archi_query" plugins/dev-team/` — 确认 0 匹配
  - `grep -rn "archi_validate" plugins/dev-team/` — 确认 0 匹配
  - `grep -rn "archi_write" plugins/dev-team/` — 确认 0 匹配
  - `grep -rn "archi_check" plugins/dev-team/` — 确认 0 匹配
  - `grep "eval_check\|eval_log" .claude/settings.local.json` — 确认仅新名称出现
- [x] **T6.2**: 验证 `plugins/dev-team/bin/dev-team-mcp.cjs` 中 TOOLS 数组的 name 字段全部使用 `xx/yy` 格式，无 `xx_yy` 格式残留
