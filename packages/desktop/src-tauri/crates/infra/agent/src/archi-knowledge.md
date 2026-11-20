# 架构模型须知

## 位置与键形

模型多份 `.toml` 按文件名字典序聚合。一元素一表头，键为**带引号的完整 FQN 单键**（`[element."App.Auth"]`）；点分嵌套与子表都读成**另一元素**（`metadata` 必须内联表），关系写 `[[relation]]`（`from` / `to` / `desc` / `apis`）；字段序与形态用 `archi_scaffold` 取模板。

## 字段纪律

- `path`：叶必填，repo 相对、`./` 前缀，目录尾 `/`、文件精确到扩展名；容器恒空。缺 `path` 由 `archi_module` 的 `hasPath` 判出。
- `entry`：单值，只在约定默认指不到时写；解析结果与成因随 `archi_module` 的 `resolvedEntry` 给出。
- `doc`：单值，只有 `api = none` 元素可写；`declared` 写了即 `apiUnexpected`（在 `apiFindings`）。
- `apis`：选填名字清单（只写名字、可留空、生成物面留空）；差额看 `archi_drift`。

## kind 语义

`package` = 包根（无 `path`）；`domain` = 容器（`path` 恒空）；`module` = crate、目录型模块或视图；`component` = 组件文件。

## 层级、缺省与读数

层级由 FQN 最长前缀推导，子元素须先声明祖先（违反即 `orphanHierarchy`）。新元素 `gate` 缺省 `off`（逐块 ratchet）。

表头计数以 `archi_drift` 的 `model.elements` / `model.relations` 为准，改模型同批更新。

架构域 `config.json` 管 `ignore` / `exclude`，未知键或缺 `reason` 即非法，`archi_drift` 报错点名。

## 边界与自检

模型不写接口 / 导出符号：契约正文归契约资产，指针不扩张出正文。

自查：`archi_drift` 取覆盖 / 归属 / 契约读数（非空即逐条修）；结构诊断经 `archi_module` 带 `file:line`；`archi_api`（`action = "check"`）查两轴；生成物过期用既有命令（`pnpm api:export` / `cargo run -p archi-api-gen -- generate`）重生成，MUST NOT 手改生成物。

## 写面

直改主仓、随 `git diff` 评审；禁止改架构目录之外的文件（重生成除外）。写盘后画布**不会自动更新**，提示用户「刷新」。续接历史会话先取当下模型。
