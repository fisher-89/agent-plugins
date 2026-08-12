# 提案: cursor-omit-subagent-stop

> **变更**: cursor-omit-subagent-stop
> **日期**: 2026-08-12
> **状态**: 草案

---

## 问题

Cursor 生态上 `subagentStop` → `static-check` 路径不可靠：matcher 近似失效、脚本产出的 `decision` / `followup_message` 无法闭环进子 agent，且与 home 侧 matcher 前缀等问题叠加后排障成本高。继续在 `cursor` / `cursorHome` 产物中生成 `subagentStop` 收益低、成本高。

Claude 侧 `SubagentStop` hook 仍可用，应保留硬门禁（含 `loop_limit: 5`）。历史变更 `static-check-agent-hook` 曾把静态检查从 agent 指令迁到 hook；本变更是 **仅 Cursor 的平台例外回退**，不是双端全面倒退。

另：agent/skill 正文缺少可复用的构建期片段机制，若仅为 Cursor 硬编码特判 token 会难以扩展。

---

## 提案

双轨策略：

1. **Cursor hooks 省略 `subagentStop`**：canonical 中两条 `subagentStop` 的 `matchers.cursor` 置 `null`；`buildCursorNative` 过滤后若为空则 **不写整个 `subagentStop` 键**（不是 `[]`）。`cursor` 与 `cursorHome` 均适用。Claude nested 的 `SubagentStop` 不变。
2. **通用 `__INCLUDE:<id>__` 片段**：在 `plugins/dev-team/_fragments/`（与 `agents/`、`skills/` 平级、唯一查找根）按 `<id>.<agent>.md` → `<id>.md` 解析；assemble 在 `applyEnvTokens` 之前递归展开（环检测、嵌入体 `trim`）；`_fragments/` 不拷贝进任何 outDir。
3. **Cursor 软门禁**：新增 `static-analysis-gate` fragment（default 空文件 → Claude 展开为空；`.cursor.md` 要求调用 `run_static_analysis`，失败则修复重跑，未通过不得结束）。`implementation-generator` 与 `test-gen-generator` Process 结束前引用 `__INCLUDE:static-analysis-gate__`。

命名对照：agent 步骤用语与 CLI 为 `static_analysis` / `run_static_analysis`；Claude hook 子命令仍为 `static-check`。

同步纠正 `dual-platform-plugin-build` 中「marketplace `cursor` 走 nested」的漂移描述：实现以 `env.agent === 'cursor'` → `cursorNative` 为准。

---

## 能力

### 新增能力

- **include-fragments** — 构建期 `__INCLUDE:<id>__` 解析/递归展开；`_fragments` 唯一根与平台后缀；`static-analysis-gate` 片段内容

### 修改的能力

- **static-check-hook** — Claude 产物继续声明 `SubagentStop` + `static-check`；Cursor 产物不再发射 `subagentStop` 键
- **dual-platform-plugin-build** — assemble 接入 include 展开顺序；`buildCursorNative` 省略空事件键；修正 marketplace `cursor` hooks 形态描述
- **phase-agents** — 两 generator 在 Process 结束前引用 `static-analysis-gate` include

---

## 变更范围

### 实现文件

- `plugins/dev-team/hooks/hooks.canonical.json` — `subagentStop[].matchers.cursor` → `null`
- `plugins/dev-team/build/hooks-profile.ts` — filter null cursor matcher；`subagentStop` 空则省略整键
- `plugins/dev-team/build/` — 新增 include 展开（resolve / 递归 / 环检测 / trim）；`assemble.ts` 在 `applyEnvTokens` 前调用；`assert-no-tokens`（或等价）将残留 `__INCLUDE:` 视为失败
- `plugins/dev-team/_fragments/static-analysis-gate.md` — 空 default（Claude）
- `plugins/dev-team/_fragments/static-analysis-gate.cursor.md` — Cursor 软门禁正文（`run_static_analysis`）
- `plugins/dev-team/agents/implementation-generator.md` — Process 结束前 `__INCLUDE:static-analysis-gate__`
- `plugins/dev-team/agents/test-gen-generator.md` — 同上
- `plugins/dev-team/package.json` — 插件版本 patch bump（源变更后重建产物）
- 重建产物：`claude-plugins/dev-team/`、`cursor-plugins/dev-team/`、`cursor-home-image/dev-team/`
- `openspec/todo.md` 第 60 行 — 与实现进度保持一致（完成后勾选）

### 测试文件

- `plugins/dev-team/build/hooks-profile.test.ts` — Cursor 产物无 `subagentStop` 键；Claude 仍有 `SubagentStop`
- 新增 include 单测（与现有 `build/*.test.ts` 并列）：resolve 优先级、空 default、缺文件失败、环失败、嵌套 + 后续 token 替换、嵌入 trim
- 必要时更新 hooks / 产物结构相关集成断言

### 不要修改

- Claude `SubagentStop` 硬门禁语义与 `static-check` 子命令行为（除产物侧声明范围澄清外）
- 修复 Cursor 上游 `subagentStop` / followup bug
- workflow 入口「开始前 static_check」（`todo.md` 另一条线）
- P1：把 edge-case 表、`phase_log` Output、`## Language` 等重复块迁入 `_fragments/`（另 change）
- 引入 `__INCLUDE_IF`、按 `ProductEnvKey` 的第三人称后缀、多 fragments 根

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | Cursor hooks 省略 `subagentStop` | 构建后 `cursor-plugins/.../hooks.json` 与 `cursor-home-image/.../hooks.json` 的 `hooks` 下无 `subagentStop` 键（亦非空数组） |
| AC-2 | Claude 保留 SubagentStop | `claude-plugins/.../hooks/hooks.json` 仍含 `SubagentStop`，matcher 覆盖 implementation-generator / test-gen-generator，command 含 `static-check`，`loop_limit` 语义不变 |
| AC-3 | canonical cursor matcher null | `hooks.canonical.json` 中两条 `subagentStop` 的 `matchers.cursor` 为 `null` |
| AC-4 | `__INCLUDE` resolve | 存在 `.cursor.md` 时优先于 `.md`；仅空 default 时 Claude 嵌入空串；两边皆无则构建失败 |
| AC-5 | 展开顺序与环 | assemble：`expandIncludes` → `applyEnvTokens` → `assertNoNameTokens`；环与可选深度上限触发构建错误；残留 `__INCLUDE:` 失败 |
| AC-6 | generator 软门禁 | Cursor/cursorHome 组装后两 generator 正文含 `run_static_analysis` 门禁步骤；Claude 组装后对应位置无该步骤（空 include） |
| AC-7 | `_fragments` 不进产物 | 任意 outDir 中不存在 `_fragments/` 目录拷贝 |
| AC-8 | 单测通过 | hooks-profile 与 include 相关单测通过 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| Cursor 软门禁无 `loop_limit`，模型可能无视「不得结束」 | 静态错误漏入后续阶段 | 中 | 文案强制；后续可另议 evaluator 抽查；不伪装与 Claude 同强度 |
| include 解析错误导致构建中断 | CI/本地 build 失败 | 低 | 缺文件/环/残留 token 显式失败；单测覆盖 |
| 误把 `_fragments` 拷进产物 | 泄漏构建源或体积膨胀 | 低 | copy 清单明确排除；AC-7 |
| spec 与实现再漂移（cursor nested vs native） | 验收歧义 | 中 | 本变更同步修正 `dual-platform-plugin-build` 描述 |
| 与 `static-check-agent-hook` 历史决议表面冲突 | 后人误以为应全面回退到 agent 指令 | 中 | 提案与风险明确「仅 Cursor 例外」 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| Cursor 是否继续生成 `subagentStop` | 否；省略整键 | 平台 bug 导致硬门禁不可靠 | 空数组 `[]`（仍暴露事件面，拒绝） |
| Claude 是否改 agent 指令 | 否；保留 SubagentStop hook | hook 仍可用；避免全面倒退 | 双端都改 agent（拒绝） |
| 注入载体 | `__INCLUDE:<id>__` + 平台后缀文件 | 可复用，非一次性 Cursor 特判 | Cursor 硬编码 token；`__INCLUDE_IF` |
| fragments 根 | 唯一 `plugins/dev-team/_fragments/` | 避免多根同 id 歧义 | `agents/_fragments` 等（已否决） |
| 步骤命名 | `static_analysis` / `run_static_analysis` | 与 CLI 一致；hook 子命令仍叫 `static-check` | 口语 `static_check` |
| default 空文件 vs 缺省 | 保留空 `static-analysis-gate.md` | 空与 missing 语义不同；缺平台档不误判 | 仅平台档、无 default |

### 待决问题

- 软深度上限默认 16：实现时采用即可，不阻塞本提案
- Cursor 软门禁是否未来挂 evaluator 抽查：另议，本单不强制
