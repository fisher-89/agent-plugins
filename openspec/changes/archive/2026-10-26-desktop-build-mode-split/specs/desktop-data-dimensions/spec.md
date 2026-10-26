# desktop-data-dimensions Specification（delta: desktop-build-mode-split）

## MODIFIED Requirements

### Requirement: workspace 维度落盘于全局目录 per-workspace db

workspace 维度数据 SHALL 落盘于**数据根下按 workspace 分立的 db 文件**（`<数据根>/workspaces/` 子树，每 workspace 恰一个独立 db 文件；数据根为 prod 构建的 `<home>/.dev-team` 或 dev 构建的 `<home>/.dev-team/dev`，判别与解析见 desktop-build-modes「数据根三分优先级与路径校验」），MUST NOT 落 workspace repo（数据不进 repo，gitignore 分型与克隆可重建两笔约束随之消灭），MUST NOT 落全局库与 workspace 维度库混居。文件路径 SHALL 由 canonical root 经单点纯函数确定性派生（同根恒同名、异根不同名，见 desktop-workspace-store「workspace 库文件确定性寻址」）；派生收口约束替代原「foundation layout 解析器收口」在本维度的适用（layout 解析器约束 openspec 域目录树，与 db 落位分属两域，互不越界）。`workspaces/` 子树 SHALL 是数据根的唯一子路径成分：本维度 MUST NOT 出现第二处根拼接或模式分支（根由 desktop-app 单点解析后整体注入）。workspace 库文件的生命周期 SHALL 跟随 workspace 注册：`remove_workspace` MUST NOT 删除库文件（重加同 root 历史完整恢复），孤儿文件治理留待二期。存量数据 SHALL 零迁移处置（用户裁定，2026-09-29：无需考虑兼容历史数据），旧单库惰性废弃（见 desktop-workspace-store「全新文件组冷启动与旧库惰性废弃」）；dev / prod 两模式之间的数据亦 MUST NOT 互相迁移或拷贝种子（模式切换后 dev 侧冷启动为空库为既定接受面，见 desktop-build-modes）。

#### Scenario: 数据不进 repo

- **WHEN** 审查任一 workspace 维度模型的落盘位置
- **THEN** 位于数据根 `workspaces/` 子树对应 workspace 的 db 文件，workspace repo（含 `.openspec/` 树）内无任何 db 落盘数据

#### Scenario: 每 workspace 物理隔离

- **WHEN** 应用注册并使用两个 workspace 后审查磁盘
- **THEN** `workspaces/` 子树下存在两个独立 db 文件，各自仅含本 workspace 的记录；任一文件损坏不波及另一 workspace 与全局库

#### Scenario: 移除注册文件保留

- **WHEN** `remove_workspace` 移除某注册项
- **THEN** 对应 workspace db 文件保留原位，重新添加同根目录后历史完整恢复

#### Scenario: 模式间零迁移

- **WHEN** 从 prod 构建切到 dev 构建（或反向），并注册同一 workspace 根
- **THEN** 各自模式下 `workspaces/` 内的库文件独立存在、互不读取；无拷贝种子、无搬移、无模式感知的迁移代码路径

## Module Contract

| 模块 | 面 | 关键契约 |
|---|---|---|
| workspace 维度落盘面（`crates/infra/store` 的 `workspaces/` 子树寻址） | Function | 父目录 = 注入的数据根，子树名 `workspaces` 为唯一路径成分；MUST NOT 出现第二处根拼接、MUST NOT 感知构建模式 |
| 数据根来源（desktop 壳 `env` 模块 → `WorkspaceStores::open`） | Function | 根的三种取值（`prod` / `dev-default` / `env`）由 desktop-build-modes 单点产出，本维度只消费注入值 |