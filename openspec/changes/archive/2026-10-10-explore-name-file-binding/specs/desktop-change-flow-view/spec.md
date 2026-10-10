# desktop-change-flow-view Specification (Delta)

## ADDED Requirements

### Requirement: change 标题渲染

change 清单条目 SHALL 渲染 `summary.title`（人类可读标题，恒非空——自 `ChangeSummary` 直读）而非 `summary.name`；change 详情头 SHALL 渲染 `detail.title`（自 `ChangeDetail` 直读）而非 `detail.name`。`name` 保持裸名语义（kebab-case 身份面），SHALL 仍可经既有信息面 / 归档面板等以 `name` 参数使用；渲染标题时不引入 fallback（title 恒非空不变量由后端保证）。测试挂钩 SHALL 使用 data-testid，MUST NOT 以样式类名作查询挂钩。

#### Scenario: 清单渲染 title

- **WHEN** 某 change 的 `title` 为「API 重试调研」而 `name` 为 `api-retry`
- **THEN** 清单条目显示「API 重试调研」为标题，`name` 不承担标题展示

#### Scenario: 详情头渲染 title

- **WHEN** 打开该 change 详情
- **THEN** 详情头显示「API 重试调研」，归档 / 运行中徽标与 worktree 信息行照常呈现

#### Scenario: 无 fallback 分支

- **WHEN** 扫描清单 / 详情视图对 title 的渲染逻辑
- **THEN** 无「title 为空回退 name」的前端分支（后端 title 恒非空）；data-testid 挂钩在案，无样式类名查询挂钩

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src/views/changes/change-list-view.tsx` | 清单条目 | 渲染 `summary.title` 为标题；行键 / 导航恒 id；`summary.name` 不再作标题展示 |
| `packages/desktop/src/views/changes/change-detail-view.tsx` | 详情头 | `DetailHeader` 渲染 `detail.title`；`detail.name` 保留供归档面板 / run 控制面板使用；worktree 信息行不变 |
