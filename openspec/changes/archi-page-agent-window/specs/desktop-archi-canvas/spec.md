# desktop-archi-canvas Specification（delta: archi-page-agent-window）

## MODIFIED Requirements

### Requirement: 能力差异（全量只读 / 增量可改档）

`mode = full` 与 `mode = incremental` 的能力差异 SHALL 是**数据语义差异**而非 UI 差异：

| 能力 | `full`（workspace 级） | `incremental`（change 级） |
|---|---|---|
| 结构校验 / 图渲染 / 文档关联 | ✅ | ✅ |
| 改 `gate` 档 | ❌ 只读视图，呈现「改档需要开一个 change」提示 | ✅ 写 `detail.worktree` 内模型，随该 change 归档合回 |

- `full` 模式 MUST NOT 呈现任何可提交的 `gate` 编辑入口（按钮 / 下拉 / 行内编辑），MUST NOT 发起写命令。
- `incremental` 模式的改档 SHALL 经**一个显式写命令**（服务端写前校验 + 渲染自证 + 原子写回）完成，MUST NOT 由前端直接拼写 / 重写 TOML 文本。
- 改档成功 SHALL 触发一次显式重取（图与抽屉同步刷新）；失败 SHALL 以行内错误呈现且磁盘零变化。
- `incremental` 模式 MUST NOT 提供「新增 / 删除元素」「连边」一类结构编辑入口（本期只开放 `gate` 单字段写入）。
- **只读约束的对象 = 画布的人类可提交入口**：`full` 模式的只读裁定 MUST NOT 被解读为「workspace 级模型不可被 agent 修改」。`/architecture` 页可同时挂载通用 agent 会话（`desktop-archi-chat`），该会话在主仓根直改模型属**会话能力**，不是画布能力；两者并存：MUST NOT 以「会话能写」为理由为画布新增任何写入口或写命令，亦 MUST NOT 以「画布只读」为理由禁止该会话写。会话写盘后画布不自动更新（零 watch 零轮询的必然推论），用户点刷新即可见新图或停摆态诊断。

#### Scenario: 全量模式零写入口

- **WHEN** 在 `/architecture`（`mode = full`）审查元素抽屉
- **THEN** 无任何可提交的 `gate` 编辑控件，呈现「改档需要开一个 change」提示；审查源码零写命令调用

#### Scenario: 增量模式改档落 worktree

- **WHEN** 在 change 详情「架构」页签把 `App.Billing` 的 `gate` 由 `off` 改为 `strict`
- **THEN** 写命令只改该 worktree 内模型文件的目标行（注释与其余内容保留），成功后图与抽屉显示新档位；主仓模型零变化

#### Scenario: 改档失败零写入

- **WHEN** 改档命令因元素缺席 / 值非法 / 渲染自证失败返回 `Err`
- **THEN** 抽屉呈现行内错误，磁盘模型字节零变化

#### Scenario: 画布只读与页内会话写权并存

- **WHEN** 在 `/architecture` 页展开左栏通用 agent 会话并让 agent 改主仓模型
- **THEN** 抽屉仍无任何可提交 `gate` 入口、页面零画布写命令；模型改动由会话写入，画布在用户点刷新前保持旧图，刷新后呈现新图或停摆态诊断（两条能力互不代理）
