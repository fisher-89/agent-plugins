# workflow.json 文件清单机制探索

> 2026-09-16 · 探索轮次 2 轮 · 状态：结论成型，可进 phase-proposal

## 背景与痛点

当前"本 change 改了哪些文件"这一事实由 **git 工作区状态**充当，带来一串问题：

- `getGitDiffFiles`（`lib/git.ts`）用 `git add -N` + `git reset` 改写用户 index，与用户自己的 staging 操作互相干扰
- git status 是整个工作区共享的：用户手头的无关改动、并行 change 的改动、中途 commit 过的文件，全部被扫进突变测试/核对范围
- untracked 的 build 产物（如 `pnpm build` 刷出的 `claude-plugins/**`）污染范围
- 每个 change 有独立的 workflow.json，天然隔离——清单化后多 change 并行不再互相污染

## git 依赖现状全景（探索时扫描结果）

| # | 位置 | 依赖方式 | 类型 |
|---|------|---------|------|
| 1 | `bin/src/lib/git.ts:25` `getGitDiffFiles` | `git status` 拿 untracked → `git add -N` → `git diff HEAD` → `git reset` | 代码 |
| 2 | `test-execution.ts:80` 突变测试 scope（`--mutation-diff-only`） | 调 #1 | 代码 |
| 3 | `test-resolve-paths.ts:165` `modules:"git-change"` 模式 | `git diff HEAD --name-only`（无任何 agent 文档引用，孤儿路径） | 代码 |
| 4 | `c4-cross-ref.ts:111` archi_check staged 模式 | `git diff --cached` | 代码 |
| 5 | `implementation-evaluator.md` I1/I2/I6（I6＝变更范围核对本体） | 提示词要求跑 `git diff` 核对 | 提示词 |
| 6 | `test-gen-evaluator.md` / `test-gen-generator.md:102`（"git diff IS the artifact"） | 同上 | 提示词 |
| 7 | `acceptance-evaluator.md` / `code-review-evaluator.md` | `git diff` 看全量产出 | 提示词 |
| 8 | `phase-implement` / `phase-test-gen` SKILL.md 描述 | 文案 | 提示词 |

计划侧清单目前只存在于非结构化 markdown：proposal.md「变更范围」（实现/测试/不要修改，**无删除概念**）、design.md「变更清单」（新增/修改/公共函数/类型/配置，**无"删除文件"子节**）。

## 结论架构：计划声明 + 被动观察，两边对账

```
   计划侧（主动声明）                    实际侧（被动观察，不可遗忘）
─────────────────────────            ─────────────────────────────
design.md 变更清单                    PostToolUse hook（新增）
  ├── 新增文件                          ├── Write/Edit/NotebookEdit
  ├── 修改文件                          │     → tool_input.file_path
  ├── 删除文件 ◄── 模板需补此节          ├── Bash / PowerShell
  └── （函数/类型/配置子表不变）         │     → 泛化现有 detectBashWrite/
                                       │       detectPowerShellWrite 为路径提取
                                       └── 去重 → 过滤 → 追加
                                                    │
                                                    ▼
                                       workflow.json
                                         files: { actual: [...] }
                                                    │
        ┌──────────────────────────────────────────┤
        ▼                                            ▼
  evaluator 核对（提示词重写）              突变测试 / test_resolve_paths
  ┌────────────────────────────┐           / archi_check
  │ 计划有、actual 无  → 硬 fail │           直接读 actual，git 退场
  │ actual 有、计划无  → agent   │
  │   判断（build产物/连锁修改    │
  │   =合理并说明；无关源码=fail）│
  │ delete 声明 → 查文件系统     │
  └────────────────────────────┘
```

方案优点：

1. **不可遗忘**：hook 是基础设施，agent 想漏都漏不掉（对比 agent 主动调 MCP 注册的方案）
2. **天然过滤 build 产物**：`pnpm build` 等 node 进程内部的写操作对 shell 解析不可见，actual 不会被生成文件淹没（git status 恰好相反）
3. **`getGitDiffFiles` 整个删除**，连同 add -N/reset index 改写 hack
4. **漏记可发现**：实现若全用 hook 看不见的方式写文件 → actual 缺计划内文件 → "计划有 actual 无" → 硬 fail 暴露。只有计划外文件漏记不可见，而那本来就走 agent 判断

## 已拍板决策

1. **清单记录方式**：hook 被动记录（非 agent 主动注册）。检查时对比 design 与 actual，额外修改是否合理交由 agent 判断。
2. **current-change 标记**：按 session 隔离，与 phase-next 现有 `run_id` 机制（进程内 `(change, run_id)` anchor）合并设计，持久化为 session 注册表。
3. **git diff 去留**：范围判定全走清单；code-review / acceptance 等 evaluator 可把 `git diff` 留作**纯观察辅助**（看修改内容以 review 质量），不参与范围判定。
4. **不兼容旧 change**：`change_create` 初始化 `files.actual: []`；消费方读到缺失字段 → 硬报错（"该 change 创建于文件清单机制之前，请重建"），不做 git 回退。

## 实现点与初步倾向

| # | 问题 | 倾向 |
|---|------|------|
| 1 | hook 如何定位目标 change | session 隔离注册表（见开放问题 1） |
| 2 | 自污染排除 | `openspec/` 整体排除（design/proposal/reports 是工作流产物）＋ workflow.json 自身；既有 PreToolUse 写保护不变 |
| 3 | delete 捕获 | Write/Edit 删不了文件；shell `rm`/`Remove-Item`/`Move-Item` 从泛化解析抓；漏网由 evaluator 查 design delete 声明 vs 文件系统存在性兜底 |
| 4 | backtrack 清空 | backtrack 到 implement 或更早 → 清空 actual 重记，避免废弃方案残留污染突变范围 |
| 5 | Cursor 环境兜底 | hooks 在 Cursor 侧支持不全（canonical 描述：static-check "主要作用于 Claude"）→ 配手动补录 MCP 工具 `change_files`（append/set），兼作 `archi_write` 等 MCP 工具写 `models/` 之类漏记的补口 |
| 6 | 并发读-改-写 | 多 session 同写一个 workflow.json 有竞争窗口，先标注不解决 |

## 改动面

```
代码层                                   提示词/模板层
──────────────                          ──────────────
hooks.ts（或新 entry）                    design.md.template  补"删除文件"子节
  + PostToolUse 记录器                    proposal.template   变更范围支持删除
  + detectXxxWrite 泛化为路径提取         implementation-evaluator  I1/I2/I6 重写
hooks.canonical.json + hooks-profile     test-gen-evaluator        同理
  + PostToolUse 注册                      acceptance/code-review    改读 actual+文件
lib/git.ts            整个删除            test-gen-generator        "artifact"语义
test-execution.ts     读 actual           phase-implement/test-gen  SKILL.md 文案
  （--mutation-diff-only 改名，如
   --mutation-scope=change）
test-resolve-paths    git-change→清单模式
c4-cross-ref.ts       archi 支持传清单
mcp.ts + schemas      change_files 补录工具 + workflow schema 扩展
change-create.ts      初始化 files.actual=[]
backtrack.ts          清空规则
session 注册表         新增（与 run_id 合并）
```

## 留给 design 的开放问题

1. **session 键桥接**：hook stdin 自带 Claude `session_id`；phase-next 的 `run_id` 是 caller 每 turn 生成的 opaque id，粒度不同。注册表键取哪个、如何桥接（候选：注册表按 session_id 键控，phase_next 增收可选 session_id 参数；或 hook 只做 per-session 暂存事件流，由带 `--change` 的命令认领合并，run_id 作认领凭证）——design 阶段定。
2. **shell 写命令解析精度边界**：泛化正则覆盖 `>` `>>` `tee` heredoc / `Set-Content` `Out-File` `Add-Content` / `rm` `Remove-Item` `Move-Item` `mv` `cp` 等常见形态；长尾漏记按"漏记可发现"性质接受。
3. **MCP 工具写文件的捕获**：`archi_write` 写 `models/` 不经 Write/Bash，进不了 actual——由 `change_files` 手动补录兜底，或 archi_write 内部自记。
4. **actual 是否需要 add/modify 分类**：倾向 flat 列表（突变与对账均不需要分类，分类语义由 design 侧承载）。
