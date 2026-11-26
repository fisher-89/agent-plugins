# desktop-archi-contract-panel Specification（delta: archi-generator-access-gap）

## Purpose

本 delta 只改 `full` 模式的一行提示文案与其场景断言：把「（或让左侧会话执行**本仓的生成命令**）」这个在通用工作区不成立的死指代，换成「或让左侧会话**生成**」。生成按钮、二次确认、模式权限与写盘单点全部逐字不变——**画布仍不加生成入口**，变的是页内会话从此真的能生成（`archi_generate`，见 `desktop-archi-agent`）。

## MODIFIED Requirements

### Requirement: 生成交互与模式权限

生成是**写工作树**的动作，交互 SHALL 与既有「写要开 change」纪律同构：

- **二次确认**：点击生成 SHALL 弹二次确认（`StandardDialog` 既有原语），文案 SHALL 含三件事——① 「将写入 N 份（不符 X + 缺失 Y）」；② 「保留 M 份孤儿生成物不做处理」（M 为当前 `orphaned` 长度）；③ 写盘非原子的如实表述。MUST NOT 以轻量 toast 代替确认。
- **成功后重跑检查**：生成成功后 SHALL 触发一次检查重取（用取数 hook 自身的刷新，不重挂整页），并 toast 报 `written` / `unchanged` 两份计数；MUST NOT 让面板停留在陈旧差异上。
- **模式权限（U1，文案本变更改写）**：生成入口 SHALL 仅出现在 `mode = incremental`（change 详情「架构」页签）；`mode = full`（`/architecture`）MUST NOT 出现可提交的生成入口，改为一行提示「**生成需开 change，或让左侧会话生成**」，形态沿既有 `archi-gate-readonly` 提示。该文案 MUST NOT 再出现「本仓的生成命令」一类死指代——通用工作区里没有那条命令，页内会话的生成能力由 `archi_generate` 工具承载（见 `desktop-archi-agent`「archi 工具面注入与 (phase, role) 矩阵」的 archi 来源臂）。
- **只读对象的延伸**：画布「只读约束的对象 = 画布的人类可提交入口」这一既有裁定同样覆盖本面板的生成按钮——MUST NOT 以「会话能写」为理由为 `full` 模式新增生成入口，亦 MUST NOT 以「画布只读」为理由禁止页内会话生成。**「页内会话生成」是既有条款的兑现，不是新入口**：它走 `archi_generate`（写主仓、事后 `git diff` 评审），与面板按钮是两条互不代理的路径。
- **生成期间零并发**：生成进行中 SHALL 禁用生成与检查按钮（防并发写同一批文件）；MUST NOT 在生成未完成时重跑检查。

#### Scenario: 二次确认与重跑检查

- **WHEN** 在 `incremental` 模式下点生成并确认
- **THEN** dialog 文案含「写入 N 份」「保留 M 份孤儿不做处理」与非原子表述；确认后 `archi_api_generate` 恰调用一次，toast 报 written / unchanged 计数，随后 `archi_api_check` 恰调用一次；面板差异按新结果刷新

#### Scenario: 取消零写盘

- **WHEN** 点生成后在 dialog 选择取消
- **THEN** `archi_api_generate` 零调用、磁盘字节零变化；面板维持原差异

#### Scenario: full 模式零生成入口且文案无死指代

- **WHEN** 在 `/architecture`（`mode = full`）审查契约视图与源码
- **THEN** 无任何可提交的生成按钮，呈一行「生成需开 change，或让左侧会话生成」提示；源码内 `full` 分支零 `archiApiGenerate` 调用路径；提示文案对「本仓的生成命令」零命中（含测试断言串）

#### Scenario: 生成中禁并发

- **WHEN** 一次生成尚未返回时再次点击生成或检查
- **THEN** 两个按钮均为禁用态、无第二次 invoke；生成返回后按钮恢复

#### Scenario: 页内会话生成不代理面板按钮

- **WHEN** 在 `full` 模式让左侧会话调用 `archi_generate` 写入主仓生成物后回到契约视图
- **THEN** 面板仍无生成按钮（模式权限不变），用户点刷新后三类差异按新结果呈现；两条路径互不调用、互不隐藏

## Module Contract

主基线既有行逐字沿用；本变更受影响模块如下：

### Component: `packages/desktop/src/views/architecture/components/dashboard/contract-panel.tsx`

| 项 | 变化 | 描述 |
|---|---|---|
| `full` 模式提示 | 改 | 文案由「生成需开 change（或让左侧会话执行本仓的生成命令）」改为「生成需开 change，或让左侧会话生成」；形态类名与位次不变 |
| 生成按钮 / 二次确认 / 并发禁用 / 重跑检查 | 零改动 | 逐字不变（`incremental` 分支的 `archiApiGenerate` 调用面零变化） |

### Component: `contract-panel.test.tsx`

| 项 | 变化 | 描述 |
|---|---|---|
| 提示文案断言 | 改 | 断言串逐字改为新文案；`full` 分支零 `archiApiGenerate` 的断言逐字不变 |
