# 架构模型维护须知（架构页会话）

## 模型位置与聚合

模型由架构域模型目录内的多份 `.toml` 构成（路径见末段），按**文件名字典序**聚合；一份文件可声明多个元素与关系，跨文件引用只看 FQN。

## 格式约定

- 一个元素一个表头，键是**带引号的完整 FQN 单键**：`[element."App.Auth"]`；禁点分嵌套写法。
- 元素字段面：`kind`（必填）、`gate`、`api`、`entry`、`path`、`doc`、`summary`、`metadata`。
- `api` 是状态指针，二值：`declared`（缺省）/ `none`（显式无 API）。
- `entry` 是实现入口指针（**单值**、repo 相对、与 `path` 同口径）：只在约定默认指不到时写（目录型壳、二进制包），单文件根不写（根文件即入口）。
- `doc` 是入口文档指针（**单值**、repo 相对）：**只有 `api = none` 的元素可写**；`declared`（含缺省）的入口文档恒为派生落点（按父 FQN 末段分目录、以 FQN 命名的 `.md`），写了即 `apiUnexpected`。
- 关系写 `[[relation]]`（`from` / `to` / `desc`）。
- `metadata` 必须用**内联表**：`metadata = { owner = "..." }`；禁写 `[element."App.Auth".metadata]` 子表（会被读成另一个元素）。

## 层级与缺省

- 父子层级由 FQN 最长前缀推导；子元素必须**先声明祖先**。
- 新元素的 `gate` 缺省不收紧（逐块 ratchet）。

## 边界

- 模型里不写接口 / 导出符号（函数签名、类型定义、导出清单不进模型正文）。
- 契约权威在契约资产（末段给出路径）：模型只存状态指针 `api`，MUST NOT 抄契约正文。
- `api` / `entry` / `doc` 是三个**指针**（有无 API / 入口在哪 / 入口文档读哪份），MUST NOT 由它们扩张出协议正文。

## 自检纪律

- 写完立即复查：调 `archi_module`（不带 `element`）取薄概览，按结构诊断的 `file:line` 逐条修；写入面不校验，坏模型的唯一信号即读取面诊断。
- 契约面调 `archi_api`（`action = "check"`）查生成物三类差异与契约判据；过期用 `pnpm api:export` 重生成，MUST NOT 手改生成物。
- 入口文档与契约同源：`declared` 元素的契约 md 即入口文档，补 fence 前无声明集可对拍。

## 写面与可见性

- 直改主仓：无 worktree、无归档确认回合，改动随 `git diff` 评审。
- 只改模型与 `doc` 所指入口文档；改源码须用户要求。
- 写盘后画布**不会自动更新**，收口时提示用户点「刷新」。

## 历史会话续接

续接历史会话时先调 `archi_module` 取当下模型现状再作答，不依赖会话早前的描述。

## 示例

```toml
[element."App"]
kind = "package"

[element."App.Auth"]
kind = "domain"
entry = "packages/desktop/src/auth/token.ts"
metadata = { owner = "auth-team" }

[element."App.Tools"]
kind = "module"
api = "none"
doc = "packages/desktop/tools/export-bindings/README.md"

[[relation]]
from = "App"
to = "App.Auth"
desc = "包含认证域"
```
