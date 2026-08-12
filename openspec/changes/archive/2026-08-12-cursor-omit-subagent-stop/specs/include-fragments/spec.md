## ADDED Requirements

### Requirement: `__INCLUDE` token 语法与唯一查找根

构建期文本组装 SHALL 支持占位符 `__INCLUDE:<id>__`，其中 `<id>` MUST 匹配 `[a-z0-9-]+`（与 `__AGENT:` / `__SKILL:` 一致），MUST NOT 含 `.` 或 `/`。

所有 `__INCLUDE:<id>__`（无论出现在 agent、skill 或其它被 assemble 扫描的文本中）SHALL 仅从唯一根 `plugins/dev-team/_fragments/` resolve。MUST NOT 引入 `agents/_fragments`、`skills/_fragments` 或多查找根。

`_fragments/` SHALL 仅在构建期从源码树读取；MUST NOT 拷贝进任何产品 `outDir`。

首版 MUST NOT 支持：`__INCLUDE_IF`、id 含路径/点号、平台档与 default 拼接 overlay、按 `ProductEnvKey` 的第三人称后缀、运行时动态 id。

#### Scenario: 合法 id 从唯一根解析

**WHEN** 源文本含 `__INCLUDE:static-analysis-gate__`
**AND** assemble 对某 `ProductEnv` 展开 include
**THEN** resolve SHALL 仅在 `plugins/dev-team/_fragments/` 下查找
**AND** MUST NOT 按引用文件所在目录相对查找

#### Scenario: 非法 id 字符拒绝

**WHEN** 源文本含 `__INCLUDE:foo/bar__` 或 `__INCLUDE:foo.bar__`
**THEN** 构建 SHALL 失败（或该 token 不被当作合法 include）

#### Scenario: `_fragments` 不进入产物树

**WHEN** assemble 成功写入 `claude` / `cursor` / `cursorHome` 的 `outDir`
**THEN** 各 `outDir` MUST NOT 包含 `_fragments/` 目录

### Requirement: 平台后缀 resolve 优先级

对当前 `ProductEnv`（`agent ∈ {"claude","cursor"}`，且 `cursorHome.agent === "cursor"`），`resolve(id, env)` SHALL 按以下优先级命中即停：

1. `plugins/dev-team/_fragments/<id>.<agent>.md`（平台专用）
2. `plugins/dev-team/_fragments/<id>.md`（default）

文件存在即命中；空文件合法（嵌入空串）。平台档存在时 MUST NOT 读取或合并 default。平台档与 default 皆不存在时 MUST 构建失败（禁止静默丢步骤）。MUST NOT 引入 `.cursorHome.md` 或 layout 第三档（除非将来显式扩表）。

#### Scenario: Cursor 优先平台档

**WHEN** 同时存在 `_fragments/static-analysis-gate.cursor.md` 与 `_fragments/static-analysis-gate.md`
**AND** assemble 的 `env.agent` 为 `cursor`
**THEN** 展开结果 SHALL 来自 `static-analysis-gate.cursor.md`
**AND** MUST NOT 合并 default 文件内容

#### Scenario: Claude 使用空 default

**WHEN** `static-analysis-gate.md` 存在且内容为空（或仅空白）
**AND** 不存在 `static-analysis-gate.claude.md`（或未使用）
**AND** assemble 的 `env.agent` 为 `claude`
**THEN** 该 include 嵌入结果 SHALL 为空串

#### Scenario: 缺文件构建失败

**WHEN** `_fragments/` 下既无 `<id>.<agent>.md` 也无 `<id>.md`
**AND** 源文本引用 `__INCLUDE:<id>__`
**THEN** assemble SHALL 失败并报告缺失 fragment

### Requirement: 递归展开、环检测与嵌入 trim

`expandIncludes(text, env, stack)` SHALL 替换每个合法 `__INCLUDE:id__`：若 `id ∈ stack` 则构建失败；否则读取 `resolve(id, env)` 正文，递归展开后对**该次嵌入体**做首尾空白 `trim` 再 splice。MUST NOT trim 宿主全文。

文件处理顺序 SHALL 为：`expandIncludes` → `applyEnvTokens` → `assertNoNameTokens`（或等价断言）。fragment 内 MAY 含其它 env token 及嵌套 `__INCLUDE:`。实现 SHOULD 提供软深度上限（如 16）作防护。

组装完成后产物中 MUST NOT 残留未展开的 `__INCLUDE:`；断言阶段 SHALL 将残留 `__INCLUDE:` 视为失败。

#### Scenario: 嵌套 include 先于 token 替换

**WHEN** fragment 正文含 `__BIN:cli__` 或嵌套 `__INCLUDE:other__`
**AND** assemble 处理含外层 `__INCLUDE:` 的宿主文件
**THEN** 内层 include SHALL 先递归展开
**AND** 随后统一 `applyEnvTokens` 替换名称/路径 token

#### Scenario: 环检测失败

**WHEN** fragment A include B 且 B include A
**THEN** assemble SHALL 失败并报告环依赖

#### Scenario: 嵌入体 trim

**WHEN** fragment 文件仅含空白或带尾空行的正文
**THEN** splice 进宿主前 SHALL 对嵌入展开结果 `trim`
**AND** 仅空白的 default SHALL 嵌入为空串
**AND** 宿主文件其余空白 MUST NOT 因 include 而被整体 trim

#### Scenario: 残留 INCLUDE 断言失败

**WHEN** 展开阶段结束后产物文本仍含 `__INCLUDE:`
**THEN** 断言步骤 SHALL 失败

### Requirement: static-analysis-gate 片段内容

仓库 SHALL 提供：

- `plugins/dev-team/_fragments/static-analysis-gate.md` — 空（或仅空白），供 Claude 展开为空
- `plugins/dev-team/_fragments/static-analysis-gate.cursor.md` — Cursor / cursorHome 软门禁正文

`.cursor.md` 正文 SHALL 要求 agent 在结束前通过 Shell 调用：

`node "__DEV_TEAM_ROOT__/bin/__BIN:cli__" run_static_analysis`

（经 token 展开后为各 env 实际路径/二进制名。）失败时 MUST 修复并重跑直至 exit 0；未通过 MUST NOT 结束本 agent。步骤表述 SHOULD 使用「结束前必须」类措辞，避免绑定两 generator 不一致的序号体系。

#### Scenario: Cursor 产物含 run_static_analysis 门禁

**WHEN** assemble 为 `cursor` 或 `cursorHome`
**AND** 宿主引用 `__INCLUDE:static-analysis-gate__`
**THEN** 写出文本 SHALL 含 `run_static_analysis` 调用说明
**AND** SHALL 含未通过不得结束的约束

#### Scenario: Claude 产物不含该软门禁步骤

**WHEN** assemble 为 `claude`
**AND** 宿主引用 `__INCLUDE:static-analysis-gate__`
**THEN** 写出文本 SHALL NOT 因该 include 插入 `run_static_analysis` 步骤（空展开）

## Module Contract

### Function: `expandIncludes(text, env, stack?)`

| 方面 | 描述 |
|------|------|
| **Export** | `build/expand-includes.ts` 命名导出（assemble / 测试入口） |
| **用途** | 递归展开 `__INCLUDE:<id>__` |
| **输入** | 宿主文本、`ProductEnv`、可选环检测 stack |
| **输出** | 展开后文本（各嵌入体已 trim） |
| **查找根** | `plugins/dev-team/_fragments/`（相对 package 根，唯一） |
| **失败** | 缺文件、环、非法嵌套深度 → 抛错 / 构建失败 |
| **深度上限** | 模块内私有常量（默认 `16`）；MUST NOT 作为公共 export |

### Function: `resolveFragment(id, env)`（名称可实现等价）

| 方面 | 描述 |
|------|------|
| **Export** | 模块内部；MUST NOT 导出；平台 resolve 经 `expandIncludes` 间接覆盖 |
| **用途** | 按平台优先级解析 fragment 路径 |
| **优先级** | `<id>.<agent>.md` → `<id>.md` |
| **空文件** | 合法，读作空串 |
| **皆无** | 错误 |

### Directory: `plugins/dev-team/_fragments/`

| 方面 | 描述 |
|------|------|
| **角色** | 构建期唯一片段源 |
| **首批文件** | `static-analysis-gate.md`（空）、`static-analysis-gate.cursor.md`（门禁） |
| **产物** | 不拷贝 |
