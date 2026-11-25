# 架构模型须知

## 职责
模型只写**结构**（元素 / 依赖）与三个**指针**（`api` / `entry` / `doc`）：接口、导出符号、协议正文归契约资产。

只改架构目录内文件；直改主仓随 `git diff` 评审；写盘后画布**不会自动更新**，提示用户刷新；续接历史会话先取当下模型。

## 工具面
字段面、键形、缺省、值域、祖先与计数全由工具承担——MUST NOT 手拼 TOML：
- 读：`archi_search`（发现）/ `archi_module`（定位）/ `archi_drift`（漂移读数）/ `archi_api`（契约两轴 + 草稿）/ `archi_scaffold`（字段词典）
- 写：`archi_element`（元素 upsert / 删除，可内联出边）/ `archi_relation`（单条关系 upsert / 删除）——参数面即字段清单，坏文本零落盘
- 自查：`archi_drift` 非空即逐条修；`archi_api`（`check`）查两轴；生成物用既有命令重生成，MUST NOT 手改

工具表达不了的（重排 / 批量整理）才手改 TOML，改完必须 `archi_module` 自查——坏模型由读取面报错（`file:line`）。

架构域 `config.json` 管 `ignore` / `exclude`，未知键或缺 `reason` 即非法（`archi_drift` 点名）。

## 留给判断
`kind` 选哪个、`path` 覆盖到哪、`api` 是否 `declared`、元素之间是否真有依赖——没有机械答案；工具的 `kind` / `gate` / `api` 参数面给出合法值与语义；纯分组容器用 `composite`。
