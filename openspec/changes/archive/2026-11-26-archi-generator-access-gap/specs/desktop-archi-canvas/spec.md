# desktop-archi-canvas Specification（delta: archi-generator-access-gap）

## Purpose

本 delta 只让一处既有裁定**兑现**，不改画布能力：`full` 模式的只读约束本就写着「想清理过期生成物，合法路径是让页内会话执行生成」——那句在过去是空头支票（会话没有生成工具、通用工作区也没有生成命令）。`archi_generate` 下发到 archi 臂之后它成立了。本 delta 把文案里的「本仓的生成命令」换掉，并显式写明**画布仍零生成入口**。

## MODIFIED Requirements

### Requirement: 能力差异（全量只读 / 增量可改档）

`mode = full` 与 `mode = incremental` 的能力差异 SHALL 是**数据语义差异**而非 UI 差异：

| 能力 | `full`（workspace 级） | `incremental`（change 级） |
|---|---|---|
| 结构校验 / 图渲染 / 入口文档段 | ✅ | ✅ |
| 改 `gate` 档 | ❌ 只读视图，呈现「改档需要开一个 change」提示 | ✅ 写 `detail.worktree` 内模型，随该 change 归档合回 |
| 生成契约生成物（写盘） | ❌ 只显示三类差异 + 呈现「生成需开 change，或让左侧会话生成」提示 | ✅ 二次确认后写 `detail.worktree` 内的生成物，随该 change 归档合回 |

- `full` 模式 MUST NOT 呈现任何可提交的 `gate` 编辑入口（按钮 / 下拉 / 行内编辑），MUST NOT 发起写命令；同理 MUST NOT 呈现任何可提交的**生成物生成**入口，MUST NOT 发起 `archi_api_generate`。
- `incremental` 模式的改档 SHALL 经**一个显式写命令**（服务端写前校验 + 渲染自证 + 原子写回）完成，MUST NOT 由前端直接拼写 / 重写 TOML 文本。
- 改档成功 SHALL 触发一次显式重取（图与元素信息面板同步刷新）；失败 SHALL 以行内错误呈现且磁盘零变化。
- `incremental` 模式的生成 SHALL 经**一个显式写命令**（`archi_api_generate`，只增只改、从不删文件）并在二次确认后发起（交互细节归 `desktop-archi-contract-panel`）；成功后 SHALL 重跑一次检查，失败 SHALL 以行内错误如实呈现（写盘非原子）。
- `incremental` 模式 MUST NOT 提供「新增 / 删除元素」「连边」一类结构编辑入口（本期只开放 `gate` 单字段写入与生成物写盘）。
- **只读约束的对象 = 画布的人类可提交入口**：`full` 模式的只读裁定 MUST NOT 被解读为「workspace 级模型不可被 agent 修改」。`/architecture` 页可同时挂载通用 agent 会话（`desktop-archi-chat`），该会话在主仓根直改模型属**会话能力**，不是画布能力；两者并存：MUST NOT 以「会话能写」为理由为画布新增任何写入口或写命令，亦 MUST NOT 以「画布只读」为理由禁止该会话写。会话写盘后画布不自动更新（零 watch 零轮询的必然推论），用户点刷新即可见新图或停摆态诊断。该裁定 SHALL 同样覆盖契约视图的生成按钮——它也是「画布的人类可提交入口」，故 `full` 模式无生成入口；`full` 模式下想清理过期生成物，其合法路径是让页内会话**生成**（会话经 `archi_generate` 按当前契约渲染并写主仓；见 `desktop-archi-agent`「archi_generate 生成物工具」），或开一个 change。
- **「页内会话能生成」不构成新入口（本变更澄清）**：上述合法路径 SHALL 由 agent 工具承载（`archi_generate` 在 archi 来源臂下发），MUST NOT 由画布或面板发起命令；画布侧零写命令调用这一断言逐字不变。

#### Scenario: 全量模式零写入口

- **WHEN** 在 `/architecture`（`mode = full`）审查元素信息面板与契约视图
- **THEN** 无任何可提交的 `gate` 编辑控件、无任何可提交的生成按钮，分别呈现「改档需要开一个 change」与「生成需开 change，或让左侧会话生成」提示；审查源码零写命令调用（`archi_set_gate` / `archi_api_generate` 均不在 `full` 分支）

#### Scenario: 增量模式改档落 worktree

- **WHEN** 在 change 详情「架构」页签把 `App.Billing` 的 `gate` 由 `off` 改为 `strict`
- **THEN** 写命令只改该 worktree 内模型文件的目标行（注释与其余内容保留），成功后图与元素信息面板显示新档位；主仓模型零变化

#### Scenario: 改档失败零写入

- **WHEN** 改档命令因元素缺席 / 值非法 / 渲染自证失败返回 `Err`
- **THEN** 元素信息面板呈现行内错误，磁盘模型字节零变化

#### Scenario: 增量模式可生成生成物

- **WHEN** 在 change 详情「架构」页签、契约视图存在三类差异时点生成并确认
- **THEN** `archi_api_generate` 写该 worktree 内生成物（主仓零变化），随后检查差异清空、面板刷新；孤儿生成物仍在盘（生成不删文件）

#### Scenario: 画布只读与页内会话写权并存

- **WHEN** 在 `/architecture` 页展开左栏通用 agent 会话并让 agent 改主仓模型（或调用 `archi_generate` 写主仓生成物）
- **THEN** 元素信息面板仍无任何可提交 `gate` 入口、契约视图无生成按钮、页面零画布写命令；模型或生成物改动由会话写入，画布与面板在用户点刷新前保持旧数据，刷新后呈现新图 / 新差异（两条能力互不代理）

## Module Contract

主基线既有行逐字沿用；本变更受影响模块如下：

### Component: `packages/desktop/src/views/architecture/**`（画布）

| 项 | 变化 | 描述 |
|---|---|---|
| 能力差异提示文案 | 改 | 「生成需开 change（或让左侧会话执行本仓的生成命令）」→「生成需开 change，或让左侧会话生成」 |
| 结构编辑入口 / 生成按钮 / 写命令调用 | 零改动 | `full` 分支零 `archi_set_gate` / 零 `archiApiGenerate` 的断言逐字不变 |
