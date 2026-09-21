# workflow.json 文件清单机制探索

> 2026-09-16 · 探索轮次 3 轮 · 状态：结论成型，可进 phase-proposal

## 背景与痛点

当前"本 change 改了哪些文件"这一事实由 **git 工作区状态**充当，带来一串问题：

- `getGitDiffFiles`（`lib/git.ts`）用 `git add -N` + `git reset` 改写用户 index，与用户自己的 staging 操作互相干扰
- git status 是整个工作区共享的：用户手头的无关改动、并行 change 的改动、中途 commit 过的文件，全部被扫进突变测试/核对范围
- untracked 的 build 产物（如 `pnpm build` 刷出的 `claude-plugins/**`）污染范围
- 每个 change 有独立的 workflow.json，天然隔离——清单化后多 change 并行不再互相污染

## git 依赖现状全景（探索时扫描结果）

| #   | 位置                                                                                | 依赖方式                                                                 | 类型   |
| --- | ----------------------------------------------------------------------------------- | ------------------------------------------------------------------------ | ------ |
| 1   | `bin/src/lib/git.ts:25` `getGitDiffFiles`                                           | `git status` 拿 untracked → `git add -N` → `git diff HEAD` → `git reset` | 代码   |
| 2   | `test-execution.ts:80` 突变测试 scope（`--mutation-diff-only`）                     | 调 #1                                                                    | 代码   |
| 3   | `test-resolve-paths.ts:165` `modules:"git-change"` 模式                             | `git diff HEAD --name-only`（无任何 agent 文档引用，孤儿路径）           | 代码   |
| 4   | `c4-cross-ref.ts:111` archi_check staged 模式                                       | `git diff --cached`                                                      | 代码   |
| 5   | `implementation-evaluator.md` I1/I2/I6（I6＝变更范围核对本体）                      | 提示词要求跑 `git diff` 核对                                             | 提示词 |
| 6   | `test-gen-evaluator.md` / `test-gen-generator.md:102`（"git diff IS the artifact"） | 同上                                                                     | 提示词 |
| 7   | `acceptance-evaluator.md` / `code-review-evaluator.md`                              | `git diff` 看全量产出                                                    | 提示词 |
| 8   | `phase-implement` / `phase-test-gen` SKILL.md 描述                                  | 文案                                                                     | 提示词 |

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
                                       │       detectPowerShellWrite 为 extractFileOps
                                       └── 折叠净状态 → 过滤 → 记录
                                                    │
                                                    ▼
                                       workflow.json
                                         files: { written: [...],
                                                  deleted: [...] }
                                                    │
        ┌──────────────────────────────────────────┤
        ▼                                            ▼
  evaluator 核对（提示词重写）              突变测试 / test_resolve_paths
  ┌──────────────────────────────────┐     / archi_check
  │ 计划有、actual 无、文件不存在     │     直接读 actual，git 退场
  │   → 硬 fail（write/delete 对称三态，│
  │     见追记 Q3/Q4）                │
  │ 计划有、actual 无、文件存在       │
  │   → 良性漏记，内容照常核对        │
  │ actual 有、计划无 → agent 判断     │
  │   （额外删除从严）                │
  └──────────────────────────────────┘
```

方案优点：

1. **不可遗忘**：hook 是基础设施，agent 想漏都漏不掉（对比 agent 主动调 MCP 注册的方案）
2. **天然过滤 build 产物**：`pnpm build` 等 node 进程内部的写操作对 shell 解析不可见，actual 不会被生成文件淹没（git status 恰好相反）
3. **`getGitDiffFiles` 整个删除**，连同 add -N/reset index 改写 hack
4. **漏记可发现**：实现若全用 hook 看不见的方式写文件 → actual 缺计划内文件 → "计划有 actual 无" → 硬 fail 暴露。只有计划外文件漏记不可见，而那本来就走 agent 判断

## 净状态折叠：还原与净归零怎么记

还原是第一个打破清单单调累积假设的操作，而"还原 = 回到基准"，机制却刻意不持有基准（hook 看到 `rm P` 时不知 P 是 change 前既有还是 agent 刚建）。**不引入 baseline 快照**：路径级快照只能区分"删自己建的 vs 删原有的"，解决不了内容级还原；内容快照＝checkpoint 机制，超出清单职责。改为三条对称折叠规则 + 外部裁判：

```
write(P)   →  deleted -= P ;  written += P
delete(P)  →  written -= P ;  deleted  += P
revert(P)  →  written -= P ;  deleted -= P    ← 仅命令识别触发
              （git restore / git checkout -- ；带 --source= 的归 write）
```

关键性质：**规则保证存在性事实正确，内容歧义留给裁判**。折叠只会多记（overstate）、不会漏记危险操作，多记方向全落在既有兜底通道：

| 场景                         | 事件序列       | 折叠结果                                 | 裁决                                                                                                |
| ---------------------------- | -------------- | ---------------------------------------- | --------------------------------------------------------------------------------------------------- |
| 撤销修改（git restore）      | write → revert | untouched                                | ✓                                                                                                   |
| 新建后放弃                   | write → delete | deleted                                  | 文件不存在；design 声明过新增 → 硬 fail（确实没交付）；未声明 → agent 判断（自建自删，说明后放行）✓ |
| 改完决定整个删除（原有文件） | write → delete | deleted                                  | 与 design 删除声明匹配 ✓                                                                            |
| 删除后以新内容重建           | delete → write | written                                  | design 声明删、文件存在 → "未删"硬 fail ✓                                                           |
| mv 往返（A→B→A）             | del+write ×2   | A∈written、B∈deleted（真值双 untouched） | A 内容==原样 → 内容核对抓"未修改"；B 从未存在 → agent 判断放行。错但安全                            |
| 重写原内容还原               | write → write  | written（真值 untouched）                | 内容核对 + 突变去噪兜底。错但安全                                                                   |

四层分工，无新增基础设施：

1. **hook 被动识别**：extractFileOps 扩三态（write/delete/revert）。`git stash`/`git clean` 类批量还原不可见，交 PreToolUse 拦——它们同时对用户数据最危险，且 `git restore` 也能还原掉 openspec/ 下工作流产物，保护规则一并覆盖（与 Q2 提取器共用路线一致）。
2. **change_files 显式修正**：既有 append/set 补录工具的 set 语义即净状态覆写，hook 认不出的还原路径走它修正（与 Cursor 兜底同一工具，零新增）。
3. **消费方裁判**：delete 裁判＝文件系统存在性（Q4 已定）；write 净状态裁判＝内容核对。突变 scope 叠加薄过滤 `written ∩ (content ≠ HEAD)`——净归零文件的突变体纯属噪声，旧 `--mutation-diff-only` 天然排除它们、清单模式需补回（与决策 3"git 纯观察辅助"一致，只作优化过滤、不作范围权威）。
4. **提示词规范**：implementation-generator 注明"撤销文件修改用 `git restore <path>`（可被记录），勿重写内容"——把 hook 认识的形态定为规范动作，长尾降级为兜底路径。

与实现点 4 分工：backtrack 清空管**批量**还原（整个 implement 作废重记）；折叠规则管 **phase 内增量**还原（attempt 2 撤 attempt 1 的某文件）。

## 已拍板决策

1. **清单记录方式**：hook 被动记录（非 agent 主动注册）。检查时对比 design 与 actual，额外修改是否合理交由 agent 判断。
2. **current-change 标记**：按 session 隔离，与 phase-next 现有 `run_id` 机制（进程内 `(change, run_id)` anchor）合并设计，持久化为 session 注册表。
3. **git diff 去留**：范围判定全走清单；code-review / acceptance 等 evaluator 可把 `git diff` 留作**纯观察辅助**（看修改内容以 review 质量），不参与范围判定。
4. **不兼容旧 change**：`change_create` 初始化 `files: { written: [], deleted: [] }`；消费方读到缺失字段 → 硬报错（"该 change 创建于文件清单机制之前，请重建"），不做 git 回退。
5. **还原/净归零**：清单语义从"去重追加"升级为折叠维护净状态（write/delete/revert 三条对称规则，见"净状态折叠"节）；不引入 baseline 快照；折叠只多记、不漏记危险操作，多记方向由外部裁判（文件系统/内容核对）与 agent 判断兜底。

## 实现点与初步倾向

| #   | 问题                     | 倾向                                                                                                                                                                                                           |
| --- | ------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | hook 如何定位目标 change | session 隔离注册表（见开放问题 1）                                                                                                                                                                             |
| 2   | 自污染排除               | `openspec/` 整体排除（design/proposal/reports 是工作流产物）＋ workflow.json 自身；既有 PreToolUse 写保护不变                                                                                                  |
| 3   | delete 捕获              | ~~初稿倾向~~ → 已解决，见追记 Q4（extractFileOps 分类 + 前缀语义 + 文件系统裁判）                                                                                                                              |
| 4   | backtrack 清空           | backtrack 到 implement 或更早 → 清空 actual 重记，避免废弃方案残留污染突变范围                                                                                                                                 |
| 5   | Cursor 环境兜底          | hooks 在 Cursor 侧支持不全（canonical 描述：static-check "主要作用于 Claude"）→ 配手动补录 MCP 工具 `change_files`（append/set），作非 Claude 环境或漏记的人工灌入口；兼作还原的显式修正通道（set 覆写净状态） |
| 6   | 并发读-改-写             | 多 session 同写一个 workflow.json 有竞争窗口，先标注不解决                                                                                                                                                     |
| 7   | 还原/净归零              | 三折叠规则 + revert 命令识别（见"净状态折叠"节）；PreToolUse 拦 git stash/clean；提示词把 git restore 定为规范还原动作                                                                                         |

## 改动面

```
代码层                                   提示词/模板层
──────────────                          ──────────────
hooks.ts（或新 entry）                    design.md.template  补"删除文件"子节
  + PostToolUse 记录器（净状态折叠）       proposal.template   变更范围支持删除
  + extractFileOps 泛化                  implementation-evaluator  I1/I2/I6 重写
    （write/delete/revert 三态）          test-gen-evaluator        同理
  + PreToolUse 保护扩拦截                 acceptance/code-review    改读 actual+文件
    git stash/clean/还原保护路径          test-gen-generator        "artifact"语义
hooks.canonical.json + hooks-profile     phase-implement/test-gen  SKILL.md 文案
  + PostToolUse 注册                      implementation-generator  规范还原动作
lib/git.ts            整个删除            （git restore，见折叠节）
test-execution.ts     读 actual
  （--mutation-diff-only 改名，如
   --mutation-scope=change；＋净归零去噪过滤）
test-resolve-paths    git-change→清单模式
c4-cross-ref.ts       archi 支持传清单
mcp.ts + schemas      change_files 补录工具 + workflow schema 扩展
change-create.ts      初始化 files.actual=[]
backtrack.ts          清空规则
session 注册表         新增（与 run_id 合并）
```

## 留给 design 的开放问题

1. **session 键桥接**：~~design 阶段定~~ → 已解决，见追记。
2. **shell 写命令解析精度边界**：泛化正则覆盖 `>` `>>` `tee` heredoc / `Set-Content` `Out-File` `Add-Content` / `rm` `Remove-Item` `Move-Item` `mv` `cp` 等常见形态；长尾漏记按"漏记可发现"性质接受。→ 解析器形态已定（见追记 Q2），具体覆盖面 design 阶段定。revert 识别精度同归此类（`git restore` 默认还原源是 index 而非 HEAD——恰好更安全，保住用户 staged 内容；`--source=<commit>` 写历史版本归 write），见"净状态折叠"节。
3. **MCP 工具写文件的捕获**：~~`archi_write` 写 `models/` 不经 Write/Bash，进不了 actual~~ → 已解决，见追记 Q3：不做任何记录（outputs 落在 openspec/ 排除范围内，且无消费方需要）。
4. **actual 是否需要 add/modify 分类**：~~倾向 flat~~ → 已解决，见追记 Q4：written/deleted 分桶，桶内 flat。

## 追记（2026-09-16）：开放问题逐项结论（1/2/4/3 全部关闭）

### Q1 已解决：hook 观察 phase_next 调用建立 session 绑定

依据（code.claude.com/docs/en/hooks、/sub-agents，经 claude-code-guide 查证）：

- **subagent 工具调用触发同样的 hooks**（文档明说），且 stdin 额外携带 `agent_id` / `agent_type` 标识来源 subagent——PostToolUse 记录器能看见 implementation-generator / test-gen-generator 的每次 Write/Edit/Bash，`agent_type` 可作来源审计。
- **session_id 是 session 级、非 agent 级**（subagent transcript 存于 `~/.claude/projects/{project}/{sessionId}/subagents/`；SubagentStop 示例中 session_id 为主会话 id）。注意：mid-subagent 工具事件的 session_id 共享是强指标但文档未明文——实现期需一次性实测（临时 log hook 跑一轮工作流）；若实测不共享，退化为"暂存-认领"模式（hook 按 session_id 暂存事件流，消费命令认领合并）。
- **MCP 工具调用同样触发 hooks 且 tool_input 可见**——桥接机制成立。
- 无 `CLAUDE_SESSION_ID` 环境变量，stdin `session_id` 是唯一会话通道。

桥接设计（phase_next 协议零改动）：

```
主 agent 调 phase_next({change, run_id})
        │ 该 MCP 调用本身触发 PostToolUse hook
        ▼
hook: stdin.session_id + tool_input.change
      → 写 session 注册表 {session_id → change}
        │
        ▼ 之后 subagent Write/Edit/Bash 事件带同一 session_id
          → 查表 → 追加对应 change 的 workflow.json files.actual
```

run_id 各司其职：继续做 phase-next 的 turn 内中断 anchor；注册表键控用 session_id；两者在同一 MCP 调用事件中交汇。

实现细节：hook matcher 必须用全名 `mcp__plugin_dev-team_dev-team__phase_next`（裸 server 名匹配不到任何工具，文档点名的坑）。

### Q2 已决定：写命令解析与写保护共用一个提取器

泛化 detectBashWrite / detectPowerShellWrite 为 `extractWriteTargets(command) → string[]`，两个消费者：

```
        extractWriteTargets(command)
                  │
    ┌─────────────┴─────────────┐
    ▼                           ▼
PreToolUse 写保护           PostToolUse 记录器
提取路径 ∩ 保护 glob → deny  提取路径 → 过滤 → actual
```

附带收益：写保护从"命令字符串包含 workflow.json"的包含式判断，升级为按提取路径精确匹配 glob——保护本身变准。

### Q4 已解决：actual 如何记录 delete

**捕获——`extractFileOps` 分类而非只提路径**（Q2 的提取器再前进一步；revert 第三态与折叠规则见正文"净状态折叠"节）：

```
extractFileOps(command) → [{ op: 'write' | 'delete' | 'revert', path }]
  rm / Remove-Item / del / git rm    → delete
  Move-Item / mv / ren 旧路径        → delete(旧) + write(新)   ← 双条目即可表达
  > / >> / tee / Set-Content / …     → write
  git restore / git checkout --      → revert（--source= 的归 write）
```

- **递归删除的前缀语义**：PostToolUse 在 `rm -rf src/old-module` 之后触发，目录已无法枚举——记录目录路径本身，消费方按路径前缀匹配；design 侧声明删除同样允许写目录，两边同构。
- **Schema**：`files: { written: [...], deleted: [...] }`，两个 flat 列表；"flat 不分类"原则存活于 written 内部（add/modify 区分由 design 承载），delete 是另一种事实必须分桶。

**对账——文件系统是 delete 的最终裁判**：

| 对比                                         | 判定                                                                                              |
| -------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| design 声明删 ∩ 文件系统仍存在               | 未删 → 硬 fail                                                                                    |
| design 声明删 ∖ actual.deleted，但文件已消失 | hook 漏记，良性（文件系统兜底已核实）                                                             |
| actual.deleted ∖ design 声明                 | agent 判断但**从严**：删除是最危险的额外操作（build 清理/连锁移动=说明后放行；无关源码被删=fail） |

关键性质：**delete 的 hook 漏记是优雅降级**——write 漏记靠"计划有 actual 无"暴露，delete 漏记不影响判定（design vs 文件系统才是权威核对，actual.deleted 只用于发现声明外的删除）。两套漏记都有兜底，机制闭合。

**顺带收益**：

1. 写保护升级为写/删保护：PreToolUse 保护规则同步拦截 `rm workflow.json` 类操作（现有包含式判断未必覆盖 rm），共用 extractFileOps 自动获得。
2. 被删内容的 review 靠保留的 git diff 纯观察辅助，无需新机制。

### Q3 已解决：archi_write 写文件不做任何记录——可行且是最优解

**事实核实**（`lib/archi-write.ts:12-13`）：`ARCHITECTURE_DIR = 'openspec/architecture'`，故

- `archi_write` → `openspec/architecture/models/*.c4`
- `archi_decide` → `openspec/architecture/decisions/*.md`

两者都落在实现点 #2「openspec/ 整体排除」范围内——**不记录不是新增决策，是既定过滤规则的默认行为**（初稿误以为 models/ 在 openspec/ 之外，此问一半是伪问题）。

**消费方扫描**：即便没有过滤，也无消费方需要 models 文件进清单——突变（本仓测试 root 为 `plugins/dev-team`，`.c4` 不在范围且 Stryker 不认 DSL）、test_resolve_paths（无同位测试）、archi_check（模型是参照系非被查对象）。唯一受影响的是 evaluator 对账。

**对账适配（核心变更）**：若 change 声明模型文件而 actual 永不含它，按"计划有 actual 无 → 硬 fail"会误报死循环。write 侧对账软化为与 delete 侧对称的三态：

```
计划有、actual 无、文件不存在 → 硬 fail（未实现）
计划有、actual 无、文件存在   → 漏记良性，内容照常核对（I1/I2 独立于 actual）
```

此软化**本来就该做**（`node -e` 等 hook 不可见写路径同样存在）；actual 的职责是圈定突变/审查范围 + 抓计划外改动，不是证明实现发生过——后者由内容核对权威承担。代价：write 侧"hook 不可见写必曝"降级为文件系统兜底（可接受，内容核对仍在）；突变对漏记的 hook 不可见源码静默缩圈（既有接受风险，与 archi_write 无关）。

**审查门禁不缺失**：architecture agent propose 模式自带 "present diff, wait for confirmation" 写前人审；in-workflow subagent 直接调 archi_write 的场景在 phase 表中不存在，即便发生由 design 声明 + 三态对账 + reviewer 交叉读 design 兜底。

**反方案否决**：为 archi_write 加记录需 recorder 白名单解析特定 MCP 工具的 tool_input（path 还是 models/ 内相对路径需拼前缀）——每新增写文件的 MCP 工具就多一条特判，而记录结果无任何机器消费方需要。成本实、收益零。

## 追记（2026-09-17）：突变测试 CLI 参数面收窄（用户拍板）

test-execution CLI（`bin/src/cli.ts` / `commands/test-execution.ts`，由 test-execution-executor 调用）的清单消费方式定稿，取代"改动面"表中 `--mutation-diff-only 改名 --mutation-scope=change` 的旧方案：

- **`--change=<change-name>` 即清单入口**：该参数已存在（现仅用于定位 reports 目录），今后兼作突变 scope 来源——CLI 依 change 名读取 `openspec/changes/<name>/workflow.json` 的 `files` 清单，突变范围即清单内容（叠加净归零去噪过滤）。
- **`--mutation-scope` / `--mutation-diff-only` 均不需要**：拟议的 `--mutation-scope` 不再引入；现有 `--mutation-diff-only`（及其 `getGitDiffFiles` git diff 路径）随清单机制退场删除。
- **`--skip-mutation` 保留**：显式跳过突变测试的逃生口不变。

## 追记（2026-09-17）：清单条目记录来源 agent_type（用户拍板）

待决问题"subagent `agent_type` 是否随清单条目记录作来源审计（可选增强）"定为**记录**：

- PostToolUse 记录器归账时，事件 stdin 携带的 `agent_type`（subagent 来源标识，Q1 查证结论：subagent 工具事件天然带此字段）随清单条目一并落盘，作来源审计——排查归账错误/范围超集时可直接看出条目出自哪个 agent，不必翻 transcript。
- 主会话事件与 `change_files` 手动补录的条目无 subagent 来源，省略该字段；append 已存在路径去重时保留既有来源。
- 折叠合并语义：同路径多次写入，来源取最后写入者（last-writer-wins）；条目被折叠移除时来源随之移除。
- 该字段仅作审计，消费方（突变 scope / test_resolve_paths / archi_check / evaluator 对账）不依赖它。
- 具体 JSON 形态（条目内嵌对象 vs 保持路径数组 flat 的旁挂 provenance 映射）由 design 定；消费方契约仅依赖路径。
