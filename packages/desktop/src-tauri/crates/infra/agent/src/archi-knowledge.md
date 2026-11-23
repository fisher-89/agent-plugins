# 架构模型须知

## 位置与键形

模型 `.toml` 按文件名字典序聚合。一元素一表头，键为**带引号的完整 FQN 单键**（`[element."Backend.Core.Agent"]`）；点分嵌套与子表读成**另一元素**（`metadata` 必须内联表），关系写 `[[relation]]`（`from` / `to` / `desc` / `apis`）；字段序与形态用 `archi_scaffold` 取模板。

## 字段纪律

**`[element.*]`**
- `path`：模块 / 包必填，`domain` / `composite` 选填（缺省空），repo 相对、`./` 前缀、目录尾 `/`、文件到扩展名；容器填了即持文件，子元素依赖它会触发 `dependsOnAncestor`。缺 `path` 看 `archi_module` 的 `hasPath`。
- `entry`：单值，仅约定默认指不到时写；成因见 `archi_module` 的 `resolvedEntry`。
- `gate`：off / warn / strict（新元素缺省 off）。
- `api`：declared / none（缺省 declared）。
- `doc`：单值，仅 `api = none` 可写；`declared` 写了即 `apiUnexpected`（在 `apiFindings`）。

**`[[relation]]`**
- `apis`：选填名字清单（只写名字、可留空、生成物面留空）；差额看 `archi_drift`。

## 元素类型（Element.kind，按包含关系从大到小，`domain` 层可省略）

- `package` = 包（最小可编译单元、独立依赖、指定语言）。
- `domain` = 领域（按业务实体边界聚合，例：订单/通知）。
- `module` = 模块（目录、视图或单文件组件）。
- `composite` = 组合（无层级，仅分组过多同级元素；可嵌套在任何类型之上）。

## 层级与读数

层级由 FQN 最长前缀推导；子元素须先声明祖先（违反即 `orphanHierarchy`）。

计数以 `archi_drift` 的 `model.elements` / `model.relations` 为准，改模型同批更新。

架构域 `config.json` 管 `ignore` / `exclude`；未知键或缺 `reason` 即非法，`archi_drift` 报错点名。

## 边界与自检

模型不写接口 / 导出符号：正文归契约资产，指针不扩张。

自查：`archi_drift` 取覆盖 / 归属 / 契约读数（非空即逐条修）；结构诊断经 `archi_module` 带 `file:line`；`archi_api`（`action = "check"`）查两轴；生成物过期用既有生成命令重生成，MUST NOT 手改生成物。

## 写面

直改主仓、随 `git diff` 评审；禁止改架构目录之外的文件（重生成除外）。写盘后画布**不会自动更新**，提示用户「刷新」。续接历史会话先取当下模型。
