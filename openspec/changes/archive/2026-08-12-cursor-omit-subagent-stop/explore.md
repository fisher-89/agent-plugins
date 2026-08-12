# Explore: Cursor 省略 subagentStop + agent `__INCLUDE` 片段

> 日期：2026-08-12  
> 状态：探索收敛，可开 proposal  
> 来源：`openspec/todo.md` 第 60 行（已误勾 `[x]`，代码未落地）  
> 相关历史：[`static-check-agent-hook`](../changes/archive/2026-06-15-static-check-agent-hook/)（指令→hook）；Cursor 实机：matcher 失效 / followup 不进子 agent（见会话 a3135006 等）

---

## 1. 问题

Cursor 生态上 `subagentStop` → `static-check` 路径不可靠：

- matcher 近似失效（任意 subagent stop 也跑检查）
- 脚本能产出 `decision` / `followup_message`，但门禁不闭环（followup 进不了子 agent）
- home 侧 matcher 前缀 vs `subagent_type` 可能对不上
- 与 bubble/timeout 等问题叠加时更难排

继续在 Cursor 产物里生成 `subagentStop` 成本高、收益低。

Claude 侧 hook 仍可用，应保留硬门禁。

---

## 2. 目标（已锁定）

| 项 | 决定 |
|----|------|
| 产物范围 | `cursor` + `cursorHome` 都不生成 `subagentStop` |
| hooks 形状 | **省略整个 `subagentStop` 键**（不是 `[]`） |
| 替代手段 | 在对应 generator agent 上下文中要求执行 **`static_analysis`**（非口语 static_check） |
| 步骤语义 | 调用 `run_static_analysis`；失败则修复并重跑；**未通过不得结束** |
| Claude | 继续 SubagentStop hook；agent 正文不写该步骤（include 展开为空） |
| 注入载体 | 通用 `__INCLUDE:<id>__` + 平台后缀文件，而非一次性 Cursor 特判 token |

涉及 agent（与现 hook matcher 对齐）：

- `implementation-generator`
- `test-gen-generator`

---

## 3. 现状锚点（代码）

```
hooks.canonical.json
  subagentStop ×2 → implementation-generator / test-gen-generator
       │
       ├─ buildClaudeNested → SubagentStop（保留）
       └─ buildCursorNative → subagentStop（现状仍生成；目标：整键省略）

agents/*.md
  仅 applyEnvTokens；无内容组合
  generator 内已无静态检查步骤（历史迁到 hook）

重复片段（未来复用动机，非本 change 必迁）
  - Parameter Type → Edge Case Mapping（×3）
  - phase_log Output 套话（多 evaluator）
  - ## Language（多 planner）
```

`todo.md:60` 已 `[x]` 但产物仍含 `subagentStop` → 提案前应改回 `[ ]` 或实现后保持勾选。

命名对照：

| 层 | 名称 |
|----|------|
| config / CLI | `static_analysis` / `run_static_analysis` |
| hook 子命令 | `static-check`（仅 Claude 路径继续用） |
| 本 explore 步骤用语 | `static_analysis` |

---

## 4. 机制 A：Cursor hooks 省略 `subagentStop`

与 `preToolUse` 已有「matcher null → filter」对齐：

1. canonical 中两条 `subagentStop` 的 `matchers.cursor` → `null`（或等价「Cursor 不发射」）
2. `buildCursorNative`：对 `subagentStop` **filter 后若为空则不写该键**
3. Claude nested 不变

验收直觉：`cursor-plugins/.../hooks.json` 与 `cursor-home-image/.../hooks.json` 的 `hooks` 下无 `subagentStop`；`claude-plugins` 仍有 `SubagentStop`。

注意：现网 `dual-platform-plugin-build` spec 仍写 marketplace `cursor` 走 nested——实现/文档以 `hooks-profile.ts`（`env.agent === 'cursor'` → native）为准，proposal 时核对 spec 漂移。

---

## 5. 机制 B：`__INCLUDE:<id>__`（敲定匹配规则）

### 5.1 Token

```
__INCLUDE:<id>__
```

- `<id>`：`[a-z0-9-]+`（与 `__AGENT:` / `__SKILL:` 一致）
- **不含** `.` / `/`（避免与平台后缀冲突）
- 只保留这一种 include 语法（无 `__INCLUDE_IF`）

### 5.2 搜索根（唯一）

```
plugins/dev-team/_fragments/
```

与 `agents/`、`skills/` **平级**，全局唯一查找根：

- 任意源文件（agent / skill / 其他被 assemble 扫描的文本）中的 `__INCLUDE:id__` **一律**从该目录 `resolve`
- **禁止** `agents/_fragments`、`skills/_fragments` 等多根——避免同 id 多来源歧义
- `_fragments/` **不**拷贝进任何产品 outDir（只在构建期读源码树）
- `copyAgents` / `copySkills` 本就不会碰到该目录；assemble 其它 copy 清单也勿纳入

### 5.3 resolve（优先级，命中即停）

对当前 `ProductEnv`，`agent ∈ {"claude","cursor"}`（`cursorHome.agent === "cursor"`）：

| 优先级 | 候选 | 含义 |
|--------|------|------|
| 1 | `_fragments/<id>.<agent>.md` | 平台专用 |
| 2 | `_fragments/<id>.md` | 默认 |

```
FRAGMENTS_ROOT = plugins/dev-team/_fragments   # 相对 package 根；唯一根

resolve(id, env):
  p1 = ${FRAGMENTS_ROOT}/${id}.${env.agent}.md
  p2 = ${FRAGMENTS_ROOT}/${id}.md
  if exists(p1) → p1
  else if exists(p2) → p2
  else → BUILD ERROR
```

补充：

- 文件存在即命中；**空文件合法**（嵌入空串）
- 平台档存在时 **不读、不合并** default
- **不**引入 `.cursorHome.md` / layout 第三档（除非将来显式扩表）
- 平台档与 default 皆无 → **构建失败**（禁止静默丢步骤）

本需求文件布局：

```
plugins/dev-team/
├── agents/
├── skills/
├── _fragments/
│   ├── static-analysis-gate.md           # 空 → Claude 无步骤
│   └── static-analysis-gate.cursor.md    # 门禁正文 → cursor / cursorHome
└── ...
```

引用点（两 generator Process 结束前）：

```
__INCLUDE:static-analysis-gate__
```

### 5.4 展开顺序与嵌套

```
expandIncludes(text, env, stack):
  替换每个 __INCLUDE:id__：
    id ∈ stack → BUILD ERROR（环）
    body = read(resolve(id, env))
    inner = expandIncludes(body, env, stack∪{id})
    splice trim(inner)          # 去掉首尾空白（含格式化误加的尾空行）
  return text

文件处理:
  expandIncludes → applyEnvTokens → assertNoNameTokens
```

- fragment 内可含其他 token（`__MCP:` / `__BIN:` / `__DEV_TEAM_ROOT__` 等）及嵌套 `__INCLUDE:`
- **先递归收完 include，再统一 `applyEnvTokens`**
- 环检测必做；可选软深度上限（如 16）作防护
- **嵌入前对展开结果 `trim`（首尾空白）**：格式化易误加尾空行；纯空白 default 自然成空串；**只 trim 嵌入体，不 trim 宿主全文**

### 5.5 刻意不支持（首版）

- id 含路径或点号
- 平台档 + default 拼接 overlay
- 按 `ProductEnvKey` 的第三人称后缀
- 运行时动态 id

### 5.6 门禁 fragment 内容要点（`.cursor.md`）

- Shell：`node "__DEV_TEAM_ROOT__/bin/__BIN:cli__" run_static_analysis`
- 失败：修问题并重跑直至 exit 0
- **未通过不得结束本 agent**
- 步骤编号用「结束前必须」类表述，避免两 generator 序号体系不一致

---

## 6. 目标形态总览

```
              plugins/dev-team/_fragments/
              static-analysis-gate(.cursor).md
                              │
              ┌───────────────┴───────────────┐
              ▼                               ▼
        Claude 组装                      Cursor* 组装
  INCLUDE → 空 default              INCLUDE → .cursor.md 正文
  hooks: SubagentStop 保留          hooks: 无 subagentStop 键
  硬门禁 + loop_limit:5             软门禁（文案强制不得结束）
```

与 hooks 心智对称：

| 层 | 逻辑 id | 平台实体 |
|----|---------|----------|
| hooks | canonical 事件 | `matchers.claude` / `matchers.cursor`（null=不发射） |
| 文本源（agent/skill/…） | `__INCLUDE:id__` → 一律查 `_fragments/` | `<id>.<agent>.md` → fallback `<id>.md` |

---

## 7. 建议 change 范围

**P0（本 change）**

1. Cursor 产物 hooks 省略 `subagentStop` 整键；Claude 不变
2. 实现 `__INCLUDE` resolve + 递归展开 + 环检测；接入 assemble（在 `applyEnvTokens` 之前）
3. 新增 `static-analysis-gate` 默认空 + `.cursor.md` 门禁；两 generator 引用
4. 单测：resolve 优先级、空 default、缺文件失败、环失败、嵌套 + token 替换、Cursor hooks 无键 / Claude 有 SubagentStop
5. 更新相关 specs：`static-check-hook`、`dual-platform-plugin-build`、`phase-agents`（或等价）
6. 纠正 `todo.md:60` 状态与实现一致

**P1（另 change，证明复用）**

- 抽出 edge-case 表、phase_log Output、Language 等重复块 → `_fragments/` + `__INCLUDE:`
- 非本单验收门槛

**非目标**

- 修复 Cursor 上游 subagentStop/followup bug
- 改 Claude 硬门禁语义
- workflow 入口「开始前 static_check」（`todo.md:38`，另一条线）

---

## 8. 整体审视（风险 / 缺口 / 一致性）

### 8.1 刻意退步

Cursor 从硬门禁退到软门禁：无 `loop_limit`，依赖模型遵守「不得结束」。这是规避平台 bug 的取舍，proposal 须写明，避免后人以为双端同强度。

### 8.2 Spec / 文档漂移

- `openspec/specs/static-check-hook/spec.md` 仍假定通用 `hooks.json` 含 `subagentStop` → 需改为「Claude 产物 / canonical 发射」视角
- `dual-platform-plugin-build` 对 marketplace `cursor` 格式描述可能与实现不一致 → proposal 时一并核对
- `assertNoNameTokens` 须把未展开的 `__INCLUDE:` 也视为失败（或 expand 阶段已保证无残留）

### 8.3 构建顺序

`copyAgents` / `copySkills` 当前先 copy 再全树 token。include 必须在 token 前、且从 **源码树** `plugins/dev-team/_fragments/` resolve（不进 outDir）。对 outDir 内已 copy 的 agent/skill 文本做 expand 时，读 fragment 仍指回源根——避免「按调用文件相对路径找 fragment」造成多根歧义。

### 8.4 空 default vs 缺 default

「Claude 无步骤」推荐 **保留空 `static-analysis-gate.md`**，这样缺平台档时不会误用到「无文件」；两边都不存在才 fail。审视结论：空文件与 missing 语义不同，规则已区分。

### 8.5 嵌套与安全

嵌套有价值（共享子片段）；环必须 fail。不在 fragment 内执行任意逻辑，只做文本拼接 → 构建期风险可控。

### 8.6 与历史提案的张力

`static-check-agent-hook` 明确反对「再放回 agent 指令」。本方案是 **仅 Cursor** 回退，Claude 仍走 hook——双轨，不是全面倒退。explore / proposal 应显式引用该历史，说明平台例外。

### 8.7 开放小项（不阻塞开提案）

| 项 | 倾向 |
|----|------|
| `_fragments` 位置 | **已定**：`plugins/dev-team/_fragments/`（与 agents/skills 平级，唯一根） |
| include 展开作用面 | agents + skills（凡 assemble 扫到且含 `__INCLUDE:` 的文本）；resolve 始终同一根 |
| 软深度上限 | 16，可选 |
| Cursor 软门禁是否未来再挂 evaluator 抽查 | 另议；本单不强制 |

### 8.8 收敛判断

问题、双轨策略、hooks 省略键、`static_analysis` 命名、`__INCLUDE` 匹配表、首个 fragment 布局均已钉死，**足够开 `dev-team_phase-proposal`**。

建议 change 名（备选）：`cursor-omit-subagent-stop` 或 `agent-include-static-analysis-gate`（前者偏产品动机，后者偏机制；可二选一或提案时合并表述）。

---

## 9. 下一步

- [ ] 开 change + `dev-team_phase-proposal`（本文件可 promote 为 `openspec/changes/<name>/explore.md`）
- [ ] 实现前把 `todo.md:60` 与真实进度对齐
- [ ] P1 另开：重复 md 迁入 `plugins/dev-team/_fragments/`

---

## 10. 修订：唯一 `_fragments` 根（2026-08-12）

否决 `agents/_fragments/`（及任何挂在 agent/skill 下的片段目录）。

**理由**：agent、skill 将来都会 `__INCLUDE:`；若各树自带 fragments，同 id 会有多来源歧义，嵌套时更难推理「这段来自哪」。

**规则**：`__INCLUDE:id__` 的文件匹配 **只** 在 `plugins/dev-team/_fragments/` 下按 §5.3 两档优先级解析；与引用点所在目录无关。

---

## 11. 修订：include 嵌入 `trim`（2026-08-12）

格式化工具容易给 fragment 文件留下尾空行。约定：每次 `__INCLUDE__` 替换时，对**该次嵌入的展开结果**做首尾空白 `trim` 再 splice。

- 空 default / 仅空白 → 嵌入空串  
- 不 trim 宿主文件全文  
- 嵌套：内层先 trim 再拼进外层，外层再 trim 后返回
