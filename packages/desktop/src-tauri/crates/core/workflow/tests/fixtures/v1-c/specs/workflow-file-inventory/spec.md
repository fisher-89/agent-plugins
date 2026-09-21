# workflow-file-inventory Delta

## ADDED Requirements

### Requirement: 记录器同步 gitignore 过滤

PostToolUse 记录器 SHALL 在归账时同步应用 gitignore 过滤：在同一 hook 调用内、折叠落盘之前，以**层级 `.gitignore`** 规则——项目根 `.gitignore` 与目标路径祖先链上各目录的 `.gitignore`——判定规范化后的目标路径，被忽略的路径 MUST NOT 进入清单净状态。判定 SHALL 遵循 gitignore 语义：注释（`#`）与空行忽略；`*` / `**` 通配；尾随 `/` 的模式仅匹配目录；`!` 负模式取消忽略；后匹配覆盖先匹配；被忽略目录下的子路径一并忽略。层级间：深层文件的规则 SHALL 优先于浅层文件；目录在任一层被排除后，其下路径一律忽略，更深层 `.gitignore` SHALL NOT 参与判定（git 对被排除目录不下降）。

过滤器不可判定时 SHALL fail-open（保留路径入清单）：无 `.gitignore` 的层视为该层无规则；任一层读取失败或包含无法判定的构造时，记录器 SHALL 按未过滤继续归账并向 stderr 留诊断，MUST NOT 抛错阻塞被观察的工具调用（与记录器既有的 exit-0 吞错策略一致）。

gitignore 过滤 SHALL 与既有自污染排除（`openspec/` 整体、`workflow.json` 自身）叠加而非替代。`change_files` 的 append / set 为人工补录与显式修正通道，MUST NOT 应用 gitignore 过滤。过滤仅在归账时生效：MUST NOT 回溯清洗既有净状态，MUST NOT 改变 `workflow_files` 查询忠实返回净状态的语义。

#### Scenario: gitignore 命中的写事件不入清单

- **WHEN** 项目根 `.gitignore` 含 `.*/`，session 已绑定 change，记录器收到对 `<root>/.claude/agent-memory/x.md` 的 Write 事件
- **THEN** `files.written` 不含 `.claude/agent-memory/x.md`
- **AND** `workflow.json` 其余字段不变

#### Scenario: 负模式取消忽略后照常归账

- **WHEN** 项目根 `.gitignore` 含 `.*/` 与 `!.claude-plugin`，记录器收到对 `<root>/.claude-plugin/marketplace.json` 的 Write 事件
- **THEN** `files.written` 含 `.claude-plugin/marketplace.json`

#### Scenario: 子目录 .gitignore 规则覆盖根规则

- **WHEN** 项目根 `.gitignore` 含 `*.log`，`apps/x/.gitignore` 含 `!keep.log`，记录器收到对 `apps/x/keep.log` 的 Write 事件
- **THEN** `files.written` 含 `apps/x/keep.log`（深层负模式覆盖根忽略）

#### Scenario: 被排除目录内的 .gitignore 不生效

- **WHEN** 项目根 `.gitignore` 含 `build/`，`build/.gitignore` 含 `!out.js`，记录器收到对 `build/out.js` 的 Write 事件
- **THEN** `files.written` 不含 `build/out.js`（目录短路，`build/` 内规则不参与判定）

#### Scenario: 未忽略路径不受影响

- **WHEN** 记录器收到对 `src/a.ts` 的 Write 事件
- **THEN** 归账行为与引入过滤前一致（`files.written` 含 `src/a.ts`）

#### Scenario: 过滤器缺失或失败 fail-open

- **WHEN** 项目根与目标路径祖先链均不存在 `.gitignore`（或任一层存在但内容无法解析）
- **THEN** 记录行为与无过滤一致，路径照常入清单
- **AND** hook 进程以 0 退出，MUST NOT 阻塞该工具调用

#### Scenario: change_files 补录不受过滤

- **WHEN** 经 `change_files` append 一条 gitignore 命中的路径（如 `node_modules/pkg/index.js`）
- **THEN** 该路径进入对应桶，append/set 语义不变

## Module Contract

### Module: `plugins/dev-team/bin/src/modules/workflow/files/`（gitignore 过滤器）

| 函数                  | 参数                                       | 返回              | 描述                                                                                                                           |
| --------------------- | ------------------------------------------ | ----------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| `loadGitignoreFilter` | `projectRoot: string`                      | `GitignoreFilter` | 构建层级过滤器：根 `.gitignore` 即刻解析，祖先链各层按需惰性解析并缓存于过滤器实例内；某层文件缺失即该层无规则                 |
| `isGitIgnored`        | `filter: GitignoreFilter, relPath: string` | `boolean`         | 按根相对 POSIX 路径做层级判定（深层文件优先、目录短路、单文件内负模式/目录递归/后匹配覆盖）；不可判定返回 `false`（fail-open） |
| 类型                  | `GitignoreFilter`                          | —                 | 过滤器句柄（不透明）                                                                                                           |

匹配实现基于 `ignore` npm 包（^7，零传递依赖）作单文件规则引擎：每层 `.gitignore` 对应一个 `ignore()` 实例（per filter 缓存），层级应用（祖先链、深层优先、目录短路）自实现——`ignore` 设计上不处理嵌套。任一层解析或判定抛错一律按 fail-open 处理（视为不忽略，路径照常入清单）。

### 组件: `plugins/dev-team/bin/src/commands/record-files.ts`（消费方）

| 项         | 值                                                                 |
| ---------- | ------------------------------------------------------------------ |
| 过滤时机   | 归账管线内、折叠落盘前同步应用（同一 hook 调用内）                 |
| 叠加规则   | 与 `openspec/**` / `workflow.json` 自污染排除按序叠加              |
| 失败语义   | `.gitignore` 缺失/不可解析 → 未过滤继续归账 + stderr 诊断 + exit 0 |
| 不适用通道 | `change_files` append/set、`workflow_files` 查询                   |
