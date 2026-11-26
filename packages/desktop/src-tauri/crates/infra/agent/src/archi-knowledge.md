# 架构模型须知

## 三资产分工
TOML 答拓扑：谁在谁的里面、谁依赖谁。schema（唯一凭据）答接触面：跨过这条边时它长什么样。md 答理由（无结构，MUST 整读）：为什么这样切、有什么坑。过度设计 = 回答自己不该回答的那个。文档是产品面：架构页直接渲染给用户读。

## 设计顺序
① `archi_search` 发现 → `archi_module` 取邻居与资产。② `archi_element` / `archi_relation` 画拓扑——先边界、后接触面、最后理由。③ `archi_api` 的 `bootstrap` 取双草稿 → 接口用 `write` / `edit` 写 `.schema.json`（坏文本零落盘）→ 理由写 `.md`。

## 三条不可越界
- 文档不列接口：导出名的唯一持有者是 schema；历史 fence 读面不读、MUST NOT 新增（清理中）。
- schema 不写理由：`x-function.description` 只写调用方要知道的语义，MUST NOT 复述签名；`x-` 面封闭为 `x-function` / `x-class` / `x-common` / `x-constructor` / `x-static`。
- TOML 不造空元素：每个元素至少一条接触面。

## 工具面
读五件：`archi_search` 发现 / `archi_module` 定位 / `archi_drift` 漂移 / `archi_scaffold` 词典；写两件：`archi_element` / `archi_relation`；`archi_api` 三动作：`bootstrap` 草稿 / `link` 算 `$ref` / `check` 两轴；MUST NOT 手拼 TOML。工具表达不了的才手改，改完 `archi_module` 自查（坏模型由读取面报 `file:line`）；生成物过期用既有命令重生成、MUST NOT 手改；`archi_drift` 非空即逐条修；`config.json` 管 `ignore` / `exclude`。

## 留给判断
`kind` / `path` / `api` / 依赖的选择没有机械答案；纯分组容器用 `composite`。接口面留白就写空 schema（`$defs {}` + `properties {}`），不要编造导出。
