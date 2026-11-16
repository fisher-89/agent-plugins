# 架构模型维护须知（架构页会话）

## 模型位置与聚合

架构模型由架构域 models 目录内的多份 `.toml` 构成（repo 相对路径见末段），按**文件名字典序**聚合为一张图；一份文件可声明多个元素与多条关系，跨文件引用只看 FQN。

## 格式约定

- 一个元素一个表头，键是**带引号的完整 FQN 单键**：`[element."App.Auth"]`；禁点分嵌套写法（会被读成另一层结构）。
- 元素字段面：`kind`（必填）、`gate`、`api`、`entry`、`path`、`docs`、`summary`、`metadata`。
- `api` 是状态指针，二值：`declared`（缺省）/ `none`（显式无 API）。
- `entry` 是实现入口指针（**单值**、repo 相对、与 `path` 同口径）：只在约定默认指不到时写（目录型壳、二进制包），单文件根不写（根文件即入口）。
- 关系写 `[[relation]]` 条目（`from` / `to` / `desc`）。
- `metadata` 必须用**内联表**：`metadata = { owner = "..." }`；禁写 `[element."App.Auth".metadata]` 子表——会被读成另一个元素。

## 层级与缺省

- 父子层级由 FQN 最长前缀推导；子元素必须**先声明祖先**（写 `App.Auth` 前先有 `[element."App"]`）。
- 新元素的 `gate` 缺省不收紧（逐块 ratchet）。

## 边界

- 模型里不写接口 / 导出符号（函数签名、类型定义、导出清单都不进模型正文）。
- 契约权威在契约资产（末段给出路径）：模型只存状态指针 `api`，MUST NOT 抄契约正文。
- `api` 与 `entry` 都是**指针**（有无公共 API / 入口在哪），MUST NOT 由它们扩张出协议正文。

## 自检纪律

- 每次写完模型立即复查：调用 `archi_module`（不带 `element` 参数）取薄概览；模型有问题时返回结构诊断清单，按其中的 `file:line` 逐条修，直到概览可读。
- 写入面不做校验，坏模型的唯一信号就是读取面诊断，所以「写完必复查」不是可选项。
- 契约面调 `archi_api`（`action = "check"`）查生成物三类差异与契约判据；过期用 `pnpm api:export` 重生成，MUST NOT 手改生成物。

## 写面与可见性

- 直改主仓：无 worktree、无归档确认回合，改动随 `git diff` 评审。
- 只改架构域模型与 `docs` 关联文档；改源码须用户明确要求。
- 写盘后画布**不会自动更新**（零 watch 零轮询），收口时提示用户点「刷新」查看新图与诊断。

## 历史会话续接

续接历史会话时，先调用 `archi_module` 取当下模型现状，再回答「现在是什么样」；不要依赖会话早前的描述（读取永远新鲜）。

## 示例

```toml
[element."App"]
kind = "package"
gate = "off"
summary = "示例应用"

[element."App.Auth"]
kind = "domain"
gate = "off"
path = ["packages/desktop/src/auth"]
entry = "packages/desktop/src/auth/token.ts"
docs = ["docs/auth.md"]
summary = "认证域"
metadata = { owner = "auth-team" }

[[relation]]
from = "App"
to = "App.Auth"
desc = "包含认证域"
```
