## 状态：已归档（决议 C1/C2=B）

本 capability **不再作为实现依据**。自动意图分类已移除；skill→agent 映射与 fail 后路由分别以 `phase-skills`、`pipeline-backtrack` 为准。

历史背景（只读）：曾描述 `phase-requirements` / `requirements-planner` 映射，以及 EVALUATOR-ONLY fail 且设 `backtrack_to` 后 skill 停止。现行为 `phase-proposal` / `proposal-*`，且 fail 后由 skill 三叉决策并继续循环。

## REMOVED Requirements

### Requirement: Intent classification and auto-routing
**Reason**: 自动意图分类不可靠；路由已内联到各 skill / workflow。  
**Migration**: 用户显式调用 `/dev-team:phase-*`、`/dev-team:workflow-*`、`openspec-archive-change`；映射见 `phase-skills`；回溯见 `pipeline-backtrack`。
