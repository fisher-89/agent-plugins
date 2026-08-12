# 设计: cursor-omit-subagent-stop

> **变更**: cursor-omit-subagent-stop
> **日期**: 2026-08-12

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| Canonical hooks | 声明逻辑事件与平台 matcher；Cursor `subagentStop` matcher 置 `null` | `plugins/dev-team/hooks/hooks.canonical.json` | `buildHooksFile` | JSON |
| Hooks 组装 | Claude nested / Cursor native 分形发射；过滤 null matcher；空事件键省略 | `plugins/dev-team/build/hooks-profile.ts` | `applyEnvTokens`、`ProductEnv`、zod | TypeScript |
| Include 展开 | 从唯一 `_fragments/` 根 resolve / 递归展开 / 环与深度防护 / 嵌入 trim | `plugins/dev-team/build/expand-includes.ts`（新增） | `ProductEnv`、`node:fs`/`path` | TypeScript |
| Assemble 管道 | copy → hooks/mcp/manifest → `expandIncludes` → `applyEnvTokens` → `assertNoNameTokens`；不拷贝 `_fragments/` | `plugins/dev-team/build/assemble.ts` | expand-includes、apply-env-tokens、assert-no-tokens、hooks-profile | TypeScript |
| Token 残留断言 | 名称类 token + 残留 `__INCLUDE:` 视为失败 | `plugins/dev-team/build/assert-no-tokens.ts` | `scanTextFiles`、`ProductEnv` | TypeScript |
| Fragment 源 | Claude 空 default + Cursor 软门禁正文 | `plugins/dev-team/_fragments/`（新增） | 仅构建期读取 | Markdown |
| Generator agents | Process 末尾引用 `__INCLUDE:static-analysis-gate__` | `plugins/dev-team/agents/implementation-generator.md`、`test-gen-generator.md` | assemble include 管道 | Markdown |
| 产物树 | 重建后 Cursor 无 `subagentStop`；Claude 保留 `SubagentStop`；generator 按平台差异展开 | `claude-plugins/dev-team/`、`cursor-plugins/dev-team/`、`cursor-home-image/dev-team/` | `vp pack` / assemble | 文件系统产物 |

### 构建数据流（本变更相关）

```
hooks.canonical.json
  subagentStop[].matchers.cursor = null
        │
        ├─ buildClaudeNested → SubagentStop（保留，含 static-check / loop_limit）
        └─ buildCursorNative → 过滤后空 → 省略 subagentStop 整键

agents/*-generator.md
  …Process steps…
  __INCLUDE:static-analysis-gate__
        │
        ▼
_fragments/  (源码根，不进 outDir)
  static-analysis-gate.md          → Claude 空串
  static-analysis-gate.cursor.md   → Cursor/cursorHome 软门禁
        │
assemble 文本管道（outDir 已 copy 的可扫描文本）:
  expandIncludes → applyEnvTokens → assertNoNameTokens
```

---

## 变更清单

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `plugins/dev-team/build/expand-includes.ts` | `__INCLUDE:<id>__` resolve / 递归展开 / 环检测 / 深度上限 / 嵌入 trim；导出 `expandIncludes`；`resolveFragment` 与深度上限常量为模块内部 |
| `plugins/dev-team/_fragments/static-analysis-gate.md` | Claude default：空文件（或仅空白），展开为空串 |
| `plugins/dev-team/_fragments/static-analysis-gate.cursor.md` | Cursor / cursorHome 软门禁正文（`run_static_analysis`） |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `plugins/dev-team/hooks/hooks.canonical.json` | 两条 `subagentStop` 的 `matchers.cursor` → `null`；可选微调顶层 `description` 注明 SubagentStop 主要作用于 Claude | AC-3；Cursor 不再经 hook 硬门禁 |
| `plugins/dev-team/build/hooks-profile.ts` | `buildCursorNative`：对 `subagentStop`（及任一事件数组）filter null `matchers.cursor`；过滤后空则**省略该事件键**（不写 `[]`）；`buildClaudeNested` 的 `SubagentStop` 与 `PreToolUse` 对齐 filter null `matchers.claude`（本变更 matcher 仍非 null，行为不变） | AC-1；纠正空键发射 |
| `plugins/dev-team/build/assemble.ts` | 在 `applyTokensInTree`（或等价）中先 `expandIncludes` 再 `applyEnvTokens`；确认 `copyAgents` / `copySkills` / 静态 copy **不**纳入 `_fragments/` | AC-5、AC-7 |
| `plugins/dev-team/build/assert-no-tokens.ts` | 残留 `__INCLUDE:`（建议检测子串或等价正则）视为未解析失败，与名称类 token 一并报错 | AC-5 |
| `plugins/dev-team/agents/implementation-generator.md` | `## Process` 全部既有步骤之后追加字面量 `__INCLUDE:static-analysis-gate__` | AC-6；源侧不手写双端静态检查长文 |
| `plugins/dev-team/agents/test-gen-generator.md` | 同上（Process 结束前） | AC-6 |
| `plugins/dev-team/package.json` | `version` patch bump（当前 `2.10.27` → `2.10.28`） | 源变更后重建产物惯例 |
| `claude-plugins/dev-team/**`、`cursor-plugins/dev-team/**`、`cursor-home-image/dev-team/**` | 运行插件构建刷新产物 | AC-1/2/6/7 |
| `openspec/todo.md` | 第 60 行在实现完成后勾选（实现前保持 `[ ]`） | 与进度一致 |

<!-- 测试文件（hooks-profile.test.ts、新增 expand-includes 单测）属独立测试阶段，本设计不展开实现细节 -->

### 公共函数 / API

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `expandIncludes` | `plugins/dev-team/build/expand-includes.ts` | 新增 | `function expandIncludes(text: string, env: ProductEnv, stack?: readonly string[], depth?: number): string` | 递归替换合法 `__INCLUDE:<id>__`；环（`id ∈ stack`）或超过软深度上限（默认 `16`）抛错；每次嵌入体对展开结果 `trim` 后再 splice；不 trim 宿主全文。平台 resolve 为模块内部实现，经本函数间接覆盖 |
| `buildHooksFile` | `plugins/dev-team/build/hooks-profile.ts` | 修改 | `function buildHooksFile(canonical: object, env: ProductEnv): string` | 签名不变；Cursor 路径在过滤后省略空事件键；Claude 路径仍发射 `SubagentStop` |
| `assertNoNameTokens` | `plugins/dev-team/build/assert-no-tokens.ts` | 修改 | `function assertNoNameTokens(rootDir: string, env: ProductEnv): void` | 签名不变；增加对残留 `__INCLUDE:` 的失败判定 |
| `assembleAll` | `plugins/dev-team/build/assemble.ts` | 修改 | `async function assembleAll(): Promise<void>` | 签名不变；内部文本管道接入 include 展开顺序 |

<!-- 模块私有：resolveFragment、INCLUDE_MAX_DEPTH、buildClaudeNested、buildCursorNative、expandCanonical、applyTokensInTree、copyAgents 等不列入 -->

### 类型定义

<!-- 无新增公共 interface / type alias / enum / class。ProductEnv / AgentType 复用 env.ts；HooksCanonical 等 zod infer 类型保持模块内部 -->

### 配置

| 配置键 | 所在文件 | 类型 | 值类型 | 默认值 | 说明 |
|--------|----------|------|--------|--------|------|
| `subagentStop[].matchers.cursor` | `plugins/dev-team/hooks/hooks.canonical.json` | 修改 | `null` | `null` | 两条均为 null，Cursor 不发射 |
| `subagentStop[].matchers.claude` | `plugins/dev-team/hooks/hooks.canonical.json` | 保留 | `string`（CALL_AGENT token） | 现有 generator 引用 | Claude 继续匹配 |
| `subagentStop[].loop_limit` | `plugins/dev-team/hooks/hooks.canonical.json` | 保留 | `number` | `5` | 仅 Claude hook 路径有效 |
| `subagentStop[].commandTemplate` | `plugins/dev-team/hooks/hooks.canonical.json` | 保留 | `string` | 含 `static-check` | Claude 硬门禁不变 |
| `description` | `plugins/dev-team/hooks/hooks.canonical.json` | 修改（建议） | `string` | 注明静态检查 hook 主要作用于 Claude `SubagentStop` | 避免后人以为 Cursor 仍依赖该事件 |
| `version` | `plugins/dev-team/package.json` | 修改 | `string`（semver） | `2.10.28`（由 `2.10.27` patch） | 重建产物前 bump |
| `INCLUDE_MAX_DEPTH`（模块内常量，不导出） | `plugins/dev-team/build/expand-includes.ts` | 新增 | `number` | `16` | 软深度上限；超限构建失败；单测以数值/`Include depth exceeded` 断言，不依赖 export |

---

## 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `IncludeToken` | `id: string` 匹配 `[a-z0-9-]+`；字面量 `__INCLUDE:<id>__` | 出现在 assemble 扫描的任意文本；解析时读 FragmentFile | 源 markdown / 其它文本字面量 |
| `FragmentFile` | `id`；可选平台后缀 `agent ∈ {claude,cursor}`；正文 `body: string`（可空） | 唯一根 `_fragments/`；平台档优先于 default，不 overlay | `plugins/dev-team/_fragments/<id>[.<agent>].md`（不进 outDir） |
| `ExpandStack` | `stack: string[]`（正在展开的 id 链）；`depth: number` | 环：`id ∈ stack`；深度：`depth > 16` | 仅内存（构建期） |
| `HooksCanonical.subagentStop[]` | `matchers.{claude,cursor}`；`loop_limit?`；`commandTemplate` | Claude → nested `SubagentStop`；Cursor matcher null → 不发射 | `hooks.canonical.json` |
| `CursorNativeHooksDoc` | `version: 1`；`hooks: { preToolUse?: … }`（**无** `subagentStop` 键于本变更后默认产物） | 由 `buildCursorNative` 生成 | `cursor-plugins/.../hooks/hooks.json`、`cursor-home-image/.../hooks.json` |
| `ClaudeNestedHooksDoc` | `hooks.PreToolUse` / `hooks.SubagentStop` | SubagentStop 仍含两 generator + `static-check` | `claude-plugins/.../hooks/hooks.json` |
| `StaticAnalysisGateFragment`（Cursor） | 要求 Shell 调用 `node "__DEV_TEAM_ROOT__/bin/__BIN:cli__" run_static_analysis`；失败修复重跑；未通过不得结束 | 经 token 展开为各 env 实际路径/二进制名 | `_fragments/static-analysis-gate.cursor.md` |

### Include resolve 算法

```
FRAGMENTS_ROOT = "_fragments"   # 相对 plugins/dev-team 包根（assemble cwd）

# resolveFragment 为模块内部（不导出）；仅由 expandIncludes 在正则已捕获合法 id 后调用
resolveFragment(id, env):
  p1 = FRAGMENTS_ROOT / `${id}.${env.agent}.md`
  p2 = FRAGMENTS_ROOT / `${id}.md`
  if exists(p1) → p1
  else if exists(p2) → p2
  else → 构建失败（报告缺失 fragment）

# 非法 id：__INCLUDE:([a-z0-9-]+)__ 不匹配 → 原文保留 → assert 残留失败
```

### Cursor 软门禁正文要点（`.cursor.md`）

使用「结束前必须」类措辞（避免绑定两 generator 不同序号），至少包含：

1. 结束前必须通过 Shell 执行：`node "__DEV_TEAM_ROOT__/bin/__BIN:cli__" run_static_analysis`
2. 非零退出时修复问题并重跑，直至 exit 0
3. 未通过静态分析时 **MUST NOT** 结束本 agent

Claude 侧对应 include 为空，硬门禁仍由 `SubagentStop` + `static-check` + `loop_limit: 5` 承担。

### Assemble 约束

1. fragment **始终**从源码树 `_fragments/` 读取，不得要求 outDir 内存在该目录，也不得相对宿主文件路径查找。
2. `copyAgents` / `copySkills` / `copyStaticAssets` 清单不得加入 `_fragments/`（AC-7）。
3. 非法 id（含 `.` / `/`）不被合法 token 匹配时，若原文仍含 `__INCLUDE:`，由 `assertNoNameTokens` 失败。

---

<!-- 本变更不涉及 HTTP API / MCP 新端点 / CLI 新子命令；路由/API 设计整节省略 -->

## 依赖

### 运行时依赖

- 无新增运行时 npm 依赖；复用现有 Node 内置 `fs`/`path` 与构建期已有 `zod`（hooks-profile）
- 软门禁调用既有 CLI 子命令 `run_static_analysis`（经 `__BIN:cli__`）；Claude hook 仍用既有 `static-check`

### 构建/测试依赖

- 现有 `vite-plus` / `vp pack` / `assembleAll` 流水线（`vite.config.ts` closeBundle 触发）
- 单测框架仍为仓库既有 `vite-plus/test`（本阶段不写测试任务）

---

## 待决问题

- 软深度上限默认 `16`：实现直接采用，无需再决策（提案已放行）
- Cursor 软门禁未来是否挂 evaluator 抽查：另议，本单不实现
- 非法 id：由 `__INCLUDE:([a-z0-9-]+)__` 不匹配而原样保留，交由残留 `__INCLUDE:` 断言失败；模块内 `resolveFragment` 不再承担非法 id 校验（仅处理已匹配合法 id）
