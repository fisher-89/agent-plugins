# 原型文件暂存（dev-design 阶段产物）

这三个文件原位于 `packages/desktop/src/components/agent/`，是本变更 dev-design 阶段的组件原型。2026-09-28 因其导致仓库级静态检查失败（oxlint 空文件警告 + knip 未使用文件）且当时本变更尚处设计阶段、无法接线，被整体移入此处暂存——内容零改动、零丢失。

实现阶段开始时，将三个文件移回 `packages/desktop/src/components/agent/` 并完成接线（`agent-messages.tsx` 中被注释的 `EntryView` / `paired` 引用为待实现点）：

```
mkdir -p packages/desktop/src/components/agent
mv openspec/changes/agent-component-ai-sdk/prototypes/agent-*.tsx openspec/changes/agent-component-ai-sdk/prototypes/index.tsx packages/desktop/src/components/agent/
```
