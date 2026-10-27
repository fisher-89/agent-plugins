# 架构模型维护须知（架构页会话）

## 模型位置与聚合

架构模型由若干 `.toml` 文件构成，全部位于架构域的 models 目录内（本文件末「派生事实」段给出 repo 相对路径）。目录内所有模型文件按**文件名字典序**聚合为一张图；一份文件可声明多个元素与多条关系，跨文件引用只看 FQN，不看文件名。

## 格式约定

- 一个元素一个表头，键是**带引号的完整 FQN 单键**：`[element."App.Auth"]`。禁点分嵌套写法（`[element.App.Auth]` 会被读成另一层结构）。
- 元素字段面：`kind`（必填）、`gate`、`path`、`docs`、`summary`、`metadata`。
- 关系写成 `[[relation]]` 条目，字段为 `from` / `to` / `desc`。
- `metadata` 必须用**内联表**：`metadata = { owner = "..." }`。禁写 `[element."App.Auth".metadata]` 子表——那是合法 TOML，但会被读成另一个元素 `App.Auth.metadata`。

## 层级与缺省

- 父子层级由 FQN 最长前缀推导（`App.Auth.Token` 属于 `App.Auth`），不靠额外声明父字段。
- 子元素必须**先声明祖先**：写 `App.Auth` 前先有 `[element."App"]`；隐式祖先不会被自动创建。
- 新元素的 `gate` 缺省不收紧（逐块 ratchet，按需再抬档）。

## 边界

- 模型里不写接口 / 导出符号（函数签名、类型定义、导出清单都不进模型）。
- 契约权威在对应能力的 spec 文本，只经元素的 `docs` 关联文档引用，MUST NOT 抄进模型正文。

## 自检纪律

- 每次写完模型立即复查：调用 `archi_module`（不带 `element` 参数）取薄概览；模型有问题时返回结构诊断清单，按其中的 `file:line` 逐条修，直到概览可读。
- 写入面不做校验，坏模型的唯一信号就是读取面诊断，所以「写完必复查」不是可选项。

## 写面与可见性

- 直改主仓：无 worktree、无归档确认回合，改动随 `git diff` 评审。
- 只改架构域模型与 `docs` 关联文档；改源码须用户明确要求。
- 写盘后画布**不会自动更新**（零 watch 零轮询），收口时提示用户点「刷新」查看新图与诊断。

## 历史会话续接

续接历史会话时，先调用 `archi_module` 取当下模型现状，再回答「现在是什么样」一类问题；不要依赖会话早前的描述（工具读取永远新鲜，记忆会旧）。

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
docs = ["docs/auth.md"]
summary = "认证域"
metadata = { owner = "auth-team" }

[[relation]]
from = "App"
to = "App.Auth"
desc = "包含认证域"
```
