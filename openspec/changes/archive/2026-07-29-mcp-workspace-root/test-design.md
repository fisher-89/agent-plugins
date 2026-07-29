# 测试设计: mcp-workspace-root

> **日期**: 2026-07-29

---

## 验收范围

| AC ID | 验收条件 | 测试类型 | 测试文件 | 测试对象/测试场景 |
|--------|---------|---------|----------|----------|
| AC-1 | connect 后 candidates = CLAUDE ∪ WORKSPACE ∪ roots 去重集合；`len==1` 时亦不自动锁定为默认根；无 `requireLocked` | 单元测试 | `plugins/dev-team/bin/src/lib/project-root.test.ts` | collectProjectRootCandidates — 多通道合并去重 / 通道失败不阻断 / listRoots 超时 / 边界 / 无默认根 |
| AC-1 | 同上 | 单元测试 | `plugins/dev-team/bin/src/mcp.test.ts` | MCP 注册 — connect 只 collect |
| AC-1 | 同上 | 集成测试 | `plugins/dev-team/bin/__tests__/mcp-workspace-root/mcp-workspace-root.test.ts` | 多候选下显式选型成功 |
| AC-2 | 触达项目树的全部 MCP tool 的 input schema 均含 required `project_root`；省略时校验失败 | 单元测试 | `plugins/dev-team/bin/src/mcp.test.ts` | MCP schema — 必填 project_root |
| AC-2 | 同上 | 集成测试 | `plugins/dev-team/bin/__tests__/mcp-workspace-root/mcp-workspace-root.test.ts` | 全工具省略 project_root 均失败 |
| AC-3 | 传入合法且 ∈ candidates 的 `project_root` 时 tool 成功执行；成功后可清 pending | 单元测试 | `plugins/dev-team/bin/src/lib/project-root.test.ts` | resolveProjectRootForTool — ∈ 候选放行 |
| AC-3 | 同上 | 单元测试 | `plugins/dev-team/bin/src/mcp.test.ts` | MCP 工具调用 — ∈ 候选接线 / call-scoped 根 |
| AC-3 | 同上 | 集成测试 | `plugins/dev-team/bin/__tests__/mcp-workspace-root/mcp-workspace-root.test.ts` | 多候选下显式选型成功 |
| AC-4 | 合法但 ∉ candidates 时返回错误，载荷含 candidates 列表与「同全参再调即 force」说明；不执行业务 I/O | 单元测试 | `plugins/dev-team/bin/src/lib/project-root.test.ts` | resolveProjectRootForTool — ∉ 候选 / ProjectRootResolveError 载荷 |
| AC-4 | 同上 | 单元测试 | `plugins/dev-team/bin/src/mcp.test.ts` | MCP 工具调用 — resolve 错误映射 |
| AC-4 | 同上 | 集成测试 | `plugins/dev-team/bin/__tests__/mcp-workspace-root/mcp-workspace-root.test.ts` | ∉ 候选拒绝且无 cwd 副作用 |
| AC-5 | 同一 tool 名 + 完整 arguments 与 pending 键相同再调时，将 `project_root` 加入 candidates 并放行执行 | 单元测试 | `plugins/dev-team/bin/src/lib/project-root.test.ts` | resolveProjectRootForTool — force |
| AC-5 | 同上 | 集成测试 | `plugins/dev-team/bin/__tests__/mcp-workspace-root/mcp-workspace-root.test.ts` | 候选外路径二次确认放行 |
| AC-6 | candidates 为空时第一次合法调用必拒；同全参再调加入候选并放行 | 单元测试 | `plugins/dev-team/bin/src/lib/project-root.test.ts` | resolveProjectRootForTool — 空候选逃生 |
| AC-6 | 同上 | 集成测试 | `plugins/dev-team/bin/__tests__/mcp-workspace-root/mcp-workspace-root.test.ts` | 候选外路径二次确认放行 / ∉ 候选拒绝且无 cwd 副作用 |
| AC-7 | pending 键 = tool 名 + 稳定序列化完整 arguments；仅保留最近一次；成功或 force 后清除 | 单元测试 | `plugins/dev-team/bin/src/lib/project-root.test.ts` | resolveProjectRootForTool — force / toolName 边界 / ∉ 候选 |
| AC-7 | 同上 | 集成测试 | `plugins/dev-team/bin/__tests__/mcp-workspace-root/mcp-workspace-root.test.ts` | 候选外路径二次确认放行 |
| AC-8 | MCP resolve 路径不得使用 `process.cwd()`；禁用静默 cwd 回退 | 单元测试 | `plugins/dev-team/bin/src/lib/project-root.test.ts` | collectProjectRootCandidates — 无 cwd / resolveProjectRootForTool — 无 cwd |
| AC-8 | 同上 | 集成测试 | `plugins/dev-team/bin/__tests__/mcp-workspace-root/mcp-workspace-root.test.ts` | ∉ 候选拒绝且无 cwd 副作用 |
| AC-9 | 不因存在上层 `.git` / `openspec/` 而改写传入的宿主工作区路径 | 单元测试 | `plugins/dev-team/bin/src/lib/project-root.test.ts` | resolveProjectRootForTool — 根语义原样 / 无 realpath |
| AC-9 | 同上 | 集成测试 | `plugins/dev-team/bin/__tests__/mcp-workspace-root/mcp-workspace-root.test.ts` | ∉ 候选拒绝且无 cwd 副作用 |
| AC-10 | 共享 `.mcp.json` 不含 `${workspaceFolder}`，也不回灌未展开的 `CLAUDE_PROJECT_DIR` | 单元测试 | `plugins/dev-team/bin/src/mcp.test.ts` | MCP 配置 — .mcp.json 约束 |
| AC-11 | CLI/command 仍可用显式 `projectRoot` / `getProjectDir`（可含 cwd）；不得把 MCP 契约绑死到 cwd | 单元测试 | `plugins/dev-team/bin/src/lib/project-root.test.ts` | getProjectDir — CLI 语义 / runWithCallScopedRoot |

---

## 单元测试

> **框架**: `vite-plus`（`vp test`，断言库为 vite-plus/test 的 `describe`/`it`/`expect`/`vi`）。
> **路径解析**: `test_resolve_paths` 仅映射 `project-root.ts` → `project-root.test.ts`、`mcp.ts` → `mcp.test.ts`；各 `schemas/*.schema.ts` 不在 test config scope（schema 契约经 `mcp.test.ts` 与集成测试覆盖）。

### `plugins/dev-team/bin/src/lib/project-root.ts` -> `plugins/dev-team/bin/src/lib/project-root.test.ts`

#### 待测功能

- `collectProjectRootCandidates(server: McpServerLike): Promise<void>`: connect 后合并 CLAUDE ∪ WORKSPACE ∪ roots 全部可用路径并去重填入 candidates；不锁定默认根；不 exit；不注册 `list_changed`；禁用 cwd；roots 保留 2000ms 超时
- `initProjectRootFromMcp(server: McpServerLike): Promise<void>`: 薄别名，委托 `collectProjectRootCandidates`
- `resolveProjectRootForTool(toolName: string, args: Record<string, unknown>): string`: 从 `args.project_root` 按合法 → ∈ 候选 → pending force 解析；放行返回绝对路径；否则抛 `ProjectRootResolveError`；禁止 cwd
- `runWithCallScopedRoot<T>(projectRoot: string, fn: () => T): T`: 执行期间 `getProjectDir()` 优先返回该根；`finally` 清除
- `getProjectRootCandidates(): readonly string[]`: 只读候选快照
- `getProjectDir(): string`: CLI/command：call-scoped → 可用 `CLAUDE_PROJECT_DIR` → 恰好 1 个可用 `WORKSPACE_FOLDER_PATHS` → `process.cwd()`
- `isProjectRootResolveError(err: unknown): err is ProjectRootResolveError`: 跨模块副本鸭类型守卫
- `ProjectRootResolveError`: 含 `code: 'invalid_path' | 'not_in_candidates'`，以及可选 `candidates` / `force_hint` / `project_root`
- `requireLockedProjectRoot` / `getMcpCachedProjectRoot` / `ProjectRootLockError` / `isProjectRootLockError`: **删除**（既有用例标记废弃）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| collectProjectRootCandidates — 多通道合并去重 | 正向 | CLAUDE + WORKSPACE 双可用 + roots 双可用时 candidates 含全部去重路径；`len>1` 不抛、不锁定默认根；`getProjectRootCandidates()` 长度 ≥2 (AC-1) | 新增 |
| collectProjectRootCandidates — 多通道合并去重 | 正向 | 仅 CLAUDE 单可用路径时 candidates 恰好含该路径；`len==1` 亦不自动锁定；未调用 `resolve` 前不得因「单根」省略校验 (AC-1) | 新增 |
| collectProjectRootCandidates — 多通道合并去重 | 正向 | 无 CLAUDE 时 WORKSPACE 按 `,`/`;` 分割后**每一个**可用路径入候选（双可用均在集合中，非仅 `[0]`）(AC-1) | 新增 |
| collectProjectRootCandidates — 多通道合并去重 | 正向 | roots 返回多个可用 `file://` / 裸绝对路径时全部入候选；与 WORKSPACE 重叠路径去重后仅保留一份展示路径 (AC-1) | 新增 |
| collectProjectRootCandidates — 多通道合并去重 | 正向 | `initProjectRootFromMcp` 委托 collect：调用后 candidates 与直接 collect 一致 (AC-1) | 新增 |
| collectProjectRootCandidates — 多通道合并去重 | 异常 | 合并过程中单通道不可用（CLAUDE 非法）时其余通道结果仍完整入集，不得因单通道失败清空集合 (AC-1) | 新增 |
| collectProjectRootCandidates — 多通道合并去重 | 边界 | Windows 盘符大小写 / 尾斜杠不同的同一路径经 normalize 比较键去重后仅一份 (AC-1) | 新增 |
| collectProjectRootCandidates — 通道失败不阻断 | 异常 | `CLAUDE_PROJECT_DIR='${workspaceFolder}'` 被跳过；WORKSPACE 单可用仍入候选；candidates 不含字面量 (AC-1) | 新增 |
| collectProjectRootCandidates — 通道失败不阻断 | 异常 | CLAUDE 为相对路径 / 绝对不存在时跳过该通道，继续合并 WORKSPACE/roots (AC-1) | 新增 |
| collectProjectRootCandidates — 通道失败不阻断 | 异常 | `listRoots` 抛错 / `-32601` 时 stderr 含 `MCP roots/list failed:`；Promise resolve；其它通道结果保留 (AC-1) | 新增 |
| collectProjectRootCandidates — listRoots 超时 | 异常 | `listRoots` hang 时 advance ≥2000ms 后 collect resolve；stderr 含 `timed out` 与 `2000`；`clearTimeout` ≥1；WORKSPACE 仍入候选 (AC-1) | 新增 |
| collectProjectRootCandidates — listRoots 超时 | 边界 | advance 仅 1999ms 时仍 pending；再 +1ms 完成（证明 2000ms 常量）(AC-1) | 新增 |
| collectProjectRootCandidates — 边界 | 边界 | CLAUDE 为 `undefined` / `""` 时跳过 env 通道 | 新增 |
| collectProjectRootCandidates — 边界 | 边界 | CLAUDE 超长不存在路径（>1000 chars）不入候选 | 新增 |
| collectProjectRootCandidates — 边界 | 边界 | CLAUDE 含空格 / emoji 且存在时入候选且展示路径严格等于该路径 | 新增 |
| collectProjectRootCandidates — 边界 | 边界 | WORKSPACE 为 `undefined` / `,,;;` / 仅相对段时该通道无贡献；空候选时 `getProjectRootCandidates()` 为 `[]` 且不含 `process.cwd()` (AC-1/AC-8) | 新增 |
| collectProjectRootCandidates — 边界 | 边界 | WORKSPACE 含空白 trim 后可用路径正确纳入 | 新增 |
| collectProjectRootCandidates — 边界 | 边界 | WORKSPACE 超大列表（`,`/`;` 分割后 ≥100 个可用绝对段）：全部入候选、去重正确、不崩溃、耗时可接受 (AC-1) | 新增 |
| collectProjectRootCandidates — 边界 | 边界 | roots 超大列表（`listRoots` 返回 ≥100 个可用 `file://`/裸绝对根）：全部可用项入候选、与其它通道重叠去重、不崩溃 (AC-1) | 新增 |
| collectProjectRootCandidates — 边界 | 边界 | 客户端无 roots capability 时 `listRootsCalls === 0` | 新增 |
| collectProjectRootCandidates — 边界 | 边界 | roots 含 `http://` / `""` / 相对 / 不存在条目时跳过坏项，可用项仍入候选 | 新增 |
| collectProjectRootCandidates — 边界 | 边界 | URI 含 `%20` 时解码路径与本地 `path.resolve` 一致后入候选 | 新增 |
| collectProjectRootCandidates — 边界 | 异常 | WORKSPACE 超大列表中夹杂大量相对 / 不存在 / `${...}` 坏段时：坏段全部跳过、可用段仍完整入集；不得因坏段占比高而清空或抛未捕获异常 (AC-1) | 新增 |
| collectProjectRootCandidates — 边界 | 异常 | roots 超大列表中夹杂 `http://` / `""` / 相对 / 不存在条目时：坏项跳过、可用项仍入候选；`listRoots` 自身若中途 reject 则 stderr 记录且其它通道结果保留 (AC-1) | 新增 |
| collectProjectRootCandidates — 无 cwd | 异常 | 三通道均失败后 candidates 为空数组；不得含 `process.cwd()` (AC-8) | 新增 |
| collectProjectRootCandidates — 无默认根 | 正向 | 单候选时二次 collect 可合并新通道路径（无「不可变 lock」短路阻止再采集）；或按设计约定幂等合并——断言无 `requireLocked` 导出且不因 `len==1` 抛 multi_root (AC-1) | 新增 |
| collectProjectRootCandidates — 无默认根 | 异常 | 二次 collect 时某通道抛错（如 `listRoots` reject）不得擦除首次已采候选；Promise 仍 resolve (AC-1) | 新增 |
| collectProjectRootCandidates — 无默认根 | 边界 | 二次 collect 与首次完全相同通道输入时 candidates 幂等（长度与比较键集合不变）(AC-1) | 新增 |
| resolveProjectRootForTool — ∈ 候选放行 | 正向 | collect 后传入 ∈ candidates 的合法绝对路径：返回 normalize 后绝对路径；pending 被清除 (AC-3) | 新增 |
| resolveProjectRootForTool — ∈ 候选放行 | 正向 | Windows 下盘符大小写不同但比较键相同的路径视为 ∈ 候选并放行（`win32` 平台或等价比较键）(AC-3) | 新增 |
| resolveProjectRootForTool — ∈ 候选放行 | 正向 | 尾斜杠差异经 normalize 后视为 ∈ 候选（保留盘符根/`/`）(AC-3) | 新增 |
| resolveProjectRootForTool — ∈ 候选放行 | 异常 | ∈ 候选放行前若 `args.project_root` 合法但随后磁盘被删（竞态）：确定性 `invalid_path` 或按 `existsSync` 再检拒绝，不得静默返回已删路径 | 新增 |
| resolveProjectRootForTool — ∈ 候选放行 | 边界 | 多候选时传入比较键等于 candidates[i] 但展示路径尾斜杠不同：放行且返回 normalize 展示形式 (AC-3) | 新增 |
| resolveProjectRootForTool — invalid_path | 异常 | `project_root` 相对路径：抛 `code==='invalid_path'`；**不**更新 pending；无 `candidates` 强制要求或 `force_hint` 可无 (AC-4 前置) | 新增 |
| resolveProjectRootForTool — invalid_path | 异常 | `project_root` 绝对但不存在：`invalid_path`；pending 不变 | 新增 |
| resolveProjectRootForTool — invalid_path | 异常 | `project_root` 含 `${...}`：`invalid_path`；`project_root` 字段回显原始入参 | 新增 |
| resolveProjectRootForTool — invalid_path | 异常 | `project_root` 为 `undefined` / 缺失 / 非 string（`null` / number / object）：`invalid_path` 或确定性拒绝 | 新增 |
| resolveProjectRootForTool — invalid_path | 边界 | `project_root` 为 `""`：`invalid_path` | 新增 |
| resolveProjectRootForTool — invalid_path | 边界 | `project_root` 超长字符串（>1000 chars）不存在：`invalid_path` 且不崩溃 | 新增 |
| resolveProjectRootForTool — invalid_path | 边界 | `project_root` 含 `\n` / `\0` / emoji 且不存在：确定性 `invalid_path` | 新增 |
| resolveProjectRootForTool — toolName 边界 | 正向 | 常规 `toolName`（如 `config_get`）参与 pending 键：同名 + 全参再调 force 成功 (AC-7) | 新增 |
| resolveProjectRootForTool — toolName 边界 | 边界 | `toolName` 为 `""`：合法 ∉ 候选时仍写 pending（键含空 tool 名）；同 `""` + 全参再调可 force；不得因空名崩溃 (AC-7) | 新增 |
| resolveProjectRootForTool — toolName 边界 | 边界 | `toolName` 超长字符串（>1000 chars）：pending 键可稳定建立与匹配；force 成功 (AC-7) | 新增 |
| resolveProjectRootForTool — toolName 边界 | 边界 | `toolName` 含 `\n` / emoji / 特殊字符：pending 键按字面拼接，同字面再调 force，改一字面则不 force (AC-7) | 新增 |
| resolveProjectRootForTool — toolName 边界 | 异常 | `toolName` 为 `undefined` / `null`（运行时非 string）：确定性拒绝或规范化为字符串键，不得抛未捕获 TypeError 导致进程中断 | 新增 |
| resolveProjectRootForTool — ∉ 候选 | 异常 | 合法绝对存在但 ∉ candidates：抛 `not_in_candidates`；`candidates` 为当前快照；`force_hint` 含再提交相同完整 arguments 即 force 的说明；不返回路径 (AC-4) | 新增 |
| resolveProjectRootForTool — ∉ 候选 | 边界 | ∉ 候选后紧接一次 `invalid_path`：pending **不被**非法调用清除或改写；再同全参合法调用仍可 force (AC-7) | 新增 |
| resolveProjectRootForTool — force | 正向 | 第一次 ∉ 候选写入 pending 后，同一 `toolName` + 完整相同 `args` 再调：将 P 加入 candidates、清 pending、返回 P (AC-5) | 新增 |
| resolveProjectRootForTool — force | 异常 | 第二次调用仅改 arguments 其它字段（如多一个 key）：pending 键不匹配，仍 `not_in_candidates`，覆盖为新 pending (AC-7) | 新增 |
| resolveProjectRootForTool — force | 异常 | 第二次调用仅改 `toolName`：不 force，写新 pending (AC-7) | 新增 |
| resolveProjectRootForTool — force | 边界 | arguments 键顺序不同但稳定序列化后相同：应匹配 pending 并 force (AC-7) | 新增 |
| resolveProjectRootForTool — force | 边界 | 成功 ∈ 放行后 pending 为清除状态；随后另一次 ∉ 可重新建立 pending (AC-7) | 新增 |
| resolveProjectRootForTool — 空候选逃生 | 异常 | candidates 为空时第一次合法绝对路径调用必拒 `not_in_candidates` 且 `candidates: []` (AC-6) | 新增 |
| resolveProjectRootForTool — 空候选逃生 | 正向 | 同全参再调：P 加入 candidates 并放行；`getProjectRootCandidates()` 含 P (AC-6) | 新增 |
| resolveProjectRootForTool — 空候选逃生 | 边界 | 空候选下第一次非法路径（`""` / 相对）：`invalid_path` 且 **不**写入 pending；随后合法全参两次仍需完整 force 流程 (AC-6/AC-7) | 新增 |
| resolveProjectRootForTool — 无 cwd | 异常 | 空候选 + 未传/非法路径时不得回退 `process.cwd()` 作为返回值 (AC-8) | 新增 |
| resolveProjectRootForTool — 无 cwd | 边界 | spy `process.cwd` 返回固定路径；任意 resolve 成功路径均不得等于该 spy 返回值（除非该路径本就经由候选/force 显式传入）(AC-8) | 新增 |
| resolveProjectRootForTool — 根语义原样 | 正向 | temp 子目录为工作区，其上层存在 `.git` 与 `openspec/`：传入子目录路径时返回值严格等于该子目录（normalize 后），不得改写为上层 git/openspec 根 (AC-9) | 新增 |
| resolveProjectRootForTool — 根语义原样 | 正向 | force 加入候选的路径同样不向上 walk (AC-9) | 新增 |
| resolveProjectRootForTool — 根语义原样 | 异常 | 传入上层 git 根而候选仅为子目录：应 `not_in_candidates`（不得因「发现 .git」自动改写或放行）(AC-9/AC-4) | 新增 |
| resolveProjectRootForTool — 根语义原样 | 边界 | 子目录路径含尾斜杠 / `.` 段经 `path.resolve` 折叠后仍等于子目录比较键，不升为父级 (AC-9) | 新增 |
| resolveProjectRootForTool — 无 realpath | 边界 | 不跟随符号链接：若平台可建 symlink，候选与入参比较不因 realpath 目标不同而误拒/误改写（无 symlink 能力则 skip）(AC-9) | 新增 |
| resolveProjectRootForTool — 无 realpath | 异常 | 候选存 symlink 路径、入参为其 realpath 目标（或反之）且比较键不同：应 `not_in_candidates`，不得因 `fs.realpath`/跟随链接而误放行或改写返回路径（无 symlink 能力则 skip）(AC-9/AC-4) | 新增 |
| runWithCallScopedRoot | 正向 | `projectRoot` 为存在的绝对路径：`fn` 内 `getProjectDir()` === 传入根；结束后 call-scoped 清除 | 新增 |
| runWithCallScopedRoot | 异常 | `fn` 抛错时仍清除 call-scoped（`finally`）；再次 `getProjectDir()` 不返回已清除的 call-scoped 根 | 新增 |
| runWithCallScopedRoot | 异常 | `fn` 为抛同步异常的函数时 call-scoped 仍清除；异步若 API 仅同步则不测 Promise reject（按签名 `() => T`） | 新增 |
| runWithCallScopedRoot | 边界 | `projectRoot` 为 `""`：call-scoped 期间 `getProjectDir()` 行为确定性（返回 `""` 或按实现拒绝）；结束后不得残留 (string 边界) | 新增 |
| runWithCallScopedRoot | 边界 | `projectRoot` 超长字符串（>1000 chars）：设置/读取/清除不崩溃；`getProjectDir()` 在期间等于该串 | 新增 |
| runWithCallScopedRoot | 边界 | `projectRoot` 含 `\n` / `\0` / emoji：字面透传给 `getProjectDir()`，结束后清除 | 新增 |
| runWithCallScopedRoot | 边界 | `projectRoot` 为 `undefined` / `null`（运行时）：确定性行为（抛 TypeError 或按空处理），不得污染后续 `getProjectDir()` 回退链 | 新增 |
| runWithCallScopedRoot | 边界 | 嵌套调用时内层结束后外层根仍有效（若实现不支持嵌套则文档化单层并测单层） | 新增 |
| getProjectDir — CLI 语义 | 正向 | call-scoped 优先于 CLAUDE / WORKSPACE / cwd (AC-11) | 新增 |
| getProjectDir — CLI 语义 | 正向 | 无 call-scoped、可用 CLAUDE 时返回 CLAUDE (AC-11) | 新增 |
| getProjectDir — CLI 语义 | 正向 | 无 call-scoped/CLAUDE、WORKSPACE 恰 1 可用时返回该路径 (AC-11) | 新增 |
| getProjectDir — CLI 语义 | 异常 | 无 call-scoped 且 CLAUDE 为相对路径：跳过该值，不得返回相对串；继续 WORKSPACE/cwd (AC-11) | 新增 |
| getProjectDir — CLI 语义 | 异常 | 无 call-scoped 且 CLAUDE 为绝对但不存在：跳过，继续 WORKSPACE/cwd；返回值不得等于该不存在路径 (AC-11) | 新增 |
| getProjectDir — CLI 语义 | 异常 | 无 call-scoped/CLAUDE 且 WORKSPACE 段全为相对 / 不存在 / `${...}`：回退 `process.cwd()`，不得取不可用段 (AC-11) | 新增 |
| getProjectDir — CLI 语义 | 异常 | call-scoped 经 `runWithCallScopedRoot` 正常结束后，CLAUDE 不可用且 WORKSPACE 多可用：回退 cwd（证明 scoped 已清，不再短路）(AC-11) | 新增 |
| getProjectDir — CLI 语义 | 边界 | 均不可用时返回 `process.cwd()`（仅 CLI）(AC-11) | 新增 |
| getProjectDir — CLI 语义 | 边界 | CLAUDE 为 `undefined` / `""` / `${...}` 时跳过并继续 WORKSPACE/cwd (AC-11) | 新增 |
| getProjectDir — CLI 语义 | 边界 | WORKSPACE 双可用时回退 cwd，不得取 `[0]` (AC-11) | 新增 |
| getProjectDir — CLI 语义 | 边界 | CLAUDE 超长不存在路径（>1000 chars）跳过；含 emoji 且存在时返回该路径 (AC-11) | 新增 |
| getProjectRootCandidates | 正向 | collect 后返回只读数组，内容与合并结果一致 | 新增 |
| getProjectRootCandidates | 边界 | 未 collect 时返回 `[]` | 新增 |
| getProjectRootCandidates | 异常 | 返回值被调用方 `push` 不得污染内部集合（只读或副本） | 新增 |
| isProjectRootResolveError | 正向 | 真实 `ProjectRootResolveError` 实例 → `true` | 新增 |
| isProjectRootResolveError | 正向 | 同名 Error + 字符串 `code`（模块副本）→ `true` | 新增 |
| isProjectRootResolveError | 异常 | 普通 Error / 仅 name 无 code / `null` / 字符串 / 非 Error 对象 → `false` | 新增 |
| isProjectRootResolveError | 边界 | `name` 匹配但 `code` 为 number → `false` | 新增 |
| ProjectRootResolveError 载荷 | 正向 | `not_in_candidates` 含 `force_hint` 推荐语义文案（含 force-add / identical complete arguments）(AC-4) | 新增 |
| ProjectRootResolveError 载荷 | 异常 | `invalid_path` 实例：`code` 正确；`force_hint` / `candidates` 缺省或未强制要求；`project_root` 回显入参 | 新增 |
| ProjectRootResolveError 载荷 | 边界 | `candidates: []` 时空数组仍可序列化；`force_hint` 为非空 string | 新增 |
| 废弃 — requireLockedProjectRoot | 正向 | 既有锁定/code 映射/multi_root/literal_env 用例 | 废弃 |
| 废弃 — getMcpCachedProjectRoot | 正向 | 既有缓存只读用例 | 废弃 |
| 废弃 — init 不可变 lock / 多根拒绝猜测 | 正向 | 「2+ roots 缓存 null」「WORKSPACE >1 失败」「refusing to guess」等硬锁语义 | 废弃 |
| 废弃 — isProjectRootLockError | 正向 | 锁错误鸭类型用例 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| MCP `McpServerLike` | 最小 mock：`getClientCapabilities` / `listRoots`（resolve、reject、hang、计数）；URI 覆盖 `file://` / 裸绝对 / `http://` / `""` | collect / 超时 / fileUri |
| `process.env` | `beforeEach`/`afterEach` 保存恢复 `CLAUDE_PROJECT_DIR`、`WORKSPACE_FOLDER_PATHS`；覆盖相对 / 不存在 / `${...}` / 超长 / emoji | collect / getProjectDir 异常与边界 |
| 真实 temp 目录 | `fs.mkdtempSync`；可选上层 `.git`/`openspec/` 结构验证 AC-9；可选删除目录模拟竞态 | 路径存在性与根语义 |
| 模块级状态 | `vi.resetModules()` + 动态 `import('./project-root')` | 全部用例隔离 |
| `process.stderr` | spy `write`，断言失败通道子串 | listRoots 失败/超时 |
| `vi.useFakeTimers` + `clearTimeout` spy | 精确 1999/2000ms | roots 超时 |
| `process.cwd` | spy 返回固定路径，断言 MCP resolve 路径永不等于该值；CLI `getProjectDir` 末位回退可等于该值 | AC-8 / AC-11 |
| `toolName` 入参 | 直接传入 `""` / 超长 / `\n`+emoji / `undefined`/`null`，不经 MCP schema | resolveProjectRootForTool — toolName 边界 |
| `runWithCallScopedRoot` 入参 | 直接传入 `""` / 超长 / 特殊字符 / `null`，观察 `getProjectDir` 与清除 | runWithCallScopedRoot 字符串边界 |

---

### `plugins/dev-team/bin/src/mcp.ts` -> `plugins/dev-team/bin/src/mcp.test.ts`

#### 待测功能

- `connectToServer(transport: Transport): Promise<McpServer>`: 注册全部触达项目树的 tools（input 均必填 `project_root`）；connect 后调用 `collectProjectRootCandidates`（或 `initProjectRootFromMcp` 别名）；handlers 经 `withResolvedProjectRoot(toolName, args, run)` → `resolveProjectRootForTool` + `runWithCallScopedRoot`；resolve 错误映射为结构化 JSON `isError`；启动日志打印 candidates；不注册 `list_changed`
- Zod/MCP input schema（经 listTools / 直接 import schema）：全部相关 tool 的 `project_root: z.string()` 为 required
- `.mcp.json` 配置约束（同文件内静态读取断言）：`mcpServers.dev-team` 无 `${workspaceFolder}`、无向其 `env` 回灌未展开 `CLAUDE_PROJECT_DIR`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| MCP 注册 — connect 只 collect | 正向 | connect 后 `collectProjectRootCandidates`（或 `initProjectRootFromMcp`）被调用恰好 1 次；不得调用已删除的 `requireLockedProjectRoot` (AC-1) | 新增 |
| MCP 注册 — connect 只 collect | 异常 | mock collect reject 时 connect 失败行为确定（上抛或记录）；不得假称已锁定默认根 | 新增 |
| MCP 注册 — connect 只 collect | 边界 | connect 成功后 candidates 日志/快照可为空数组（空候选合法），不因 `len==0`/`len==1` 分支差异退出 (AC-1) | 新增 |
| MCP 注册 — 工具 name 精确断言 | 正向 | `listTools()` name 集合经 sort 后严格等于 11 个预期 name | 新增 |
| MCP 注册 — 工具 name 精确断言 | 异常 | 不得包含 `list_changed` / camelCase 别名 | 新增 |
| MCP 注册 — 工具 name 精确断言 | 边界 | name 集合长度恰好 11；无重复 name | 新增 |
| MCP 注册 — 无 list_changed handler | 异常 | 未注册 `notifications/roots/list_changed` | 新增 |
| MCP schema — 必填 project_root | 正向 | `listTools` 中 `phase_log`/`phase_next`/`backtrack`/`change_list`/`config_get`/`archi_*`/`test_*` 的 `inputSchema.required`（或等价）均含 `project_root`；`properties.project_root` 存在 (AC-2) | 新增 |
| MCP schema — 必填 project_root | 正向 | 直接 import 各 `*InputSchema`：`shape` 含 `project_root`；`safeParse` 省略该字段时 `success === false` (AC-2) | 新增 |
| MCP schema — 必填 project_root | 异常 | `callTool` 省略 `project_root`（仅业务字段）时校验失败 / `isError`，且 command spy 次数为 0 (AC-2) | 新增 |
| MCP schema — 必填 project_root | 异常 | `project_root` 为非 string（number / null / object）时 schema `safeParse` 失败 (AC-2) | 新增 |
| MCP schema — 必填 project_root | 边界 | `project_root: ""` 经 schema 或 resolve 失败（不得静默当 cwd）(AC-2/AC-8) | 新增 |
| MCP schema — 必填 project_root | 边界 | `project_root` 超长字符串（>1000 chars）/ 含 `\n` / emoji：schema 若仅 `z.string()` 则通过校验，随后由 resolve 判合法/非法；不得崩溃 (AC-2) | 新增 |
| MCP 工具调用 — ∈ 候选接线 | 正向 | mock `resolveProjectRootForTool` 返回 fixture 根：`change_list`/`config_get`/`test_*`/`archi_*`/`phase_*`/`backtrack` 成功并将该根传入 command/lib；output 可回显 `project_root` (AC-3) | 新增 |
| MCP 工具调用 — ∈ 候选接线 | 异常 | resolve 成功但 command 抛业务错：不得吞为 resolve 成功假象；错误路径与 resolve 短路可区分 | 新增 |
| MCP 工具调用 — ∈ 候选接线 | 边界 | resolve 返回带尾斜杠 normalize 后的根时，传入 command 的路径与回显一致 (AC-3) | 新增 |
| MCP 工具调用 — resolve 错误映射 | 异常 | mock 抛 `ProjectRootResolveError{code:'not_in_candidates', candidates, force_hint, project_root}`：`isError===true`；`content[0].text` 为 JSON，字段 `code`/`candidates`/`force_hint`/`project_root`/`message` 齐全；command spy 次数 0 (AC-4) | 新增 |
| MCP 工具调用 — resolve 错误映射 | 异常 | mock 抛 `invalid_path`：JSON 含 `code` 与 `project_root`；无业务 I/O (AC-4) | 新增 |
| MCP 工具调用 — resolve 错误映射 | 异常 | 鸭类型 `name==='ProjectRootResolveError'` + string `code` 仍映射为 `isError` JSON (AC-4) | 新增 |
| MCP 工具调用 — resolve 错误映射 | 异常 | 普通 `Error('boom')` 不得被包装成成功 JSON；守卫不识别则上抛或非业务成功 (AC-4) | 新增 |
| MCP 工具调用 — resolve 错误映射 | 边界 | `candidates: []` 与超长 `force_hint` / `project_root` 仍可 JSON 序列化进 `content[0].text` | 新增 |
| MCP 工具调用 — call-scoped 根 | 正向 | spy `runWithCallScopedRoot`：resolve 成功后以 resolve 根调用；`phase_log`/`phase_next`/`backtrack` 业务在 scoped 内执行 (AC-3) | 新增 |
| MCP 工具调用 — call-scoped 根 | 异常 | resolve 抛错时 `runWithCallScopedRoot` 调用次数为 0（未进入 scoped）(AC-4) | 新增 |
| MCP 工具调用 — call-scoped 根 | 边界 | `runWithCallScopedRoot` 的第一参严格等于 resolve 返回值（含特殊字符路径 fixture） | 新增 |
| MCP 工具调用 — 全 11 handler | 正向 | resolve mock 成功时 11 个 tool 各 `callTool` 一次均非空成功（或 archi mock 成功）；专杀空箭头 handler | 新增 |
| MCP 工具调用 — 全 11 handler | 异常 | resolve 抛 `not_in_candidates` 时 11 个 tool 均 `isError` 且 spy 次数 0 | 新增 |
| MCP 工具调用 — 全 11 handler | 边界 | 11 个 tool 均传入相同合法 `project_root` 字符串边界值（含尾斜杠 normalize 后路径）时接线一致 | 新增 |
| MCP 配置 — .mcp.json 约束 | 正向 | 读取 `plugins/dev-team/.mcp.json`：`mcpServers.dev-team` 无 `env` 回灌 `CLAUDE_PROJECT_DIR`；整段 JSON 字符串（或 dev-team 节点序列化）不含 `${workspaceFolder}` (AC-10) | 新增 |
| MCP 配置 — .mcp.json 约束 | 异常 | `dev-team` 节点若出现 `env.CLAUDE_PROJECT_DIR` 字面量 `${...}` 则本用例失败 (AC-10) | 新增 |
| MCP 配置 — .mcp.json 约束 | 边界 | `likec4` 服务可继续使用 `${CLAUDE_PROJECT_DIR}`（非本变更约束对象）；断言仅约束 `dev-team` 服务 (AC-10) | 新增 |
| 废弃 — MCP schema 无 input project_root | 正向 | 「properties 不含 project_root」「无参 change_list 成功」等旧锁模型用例 | 废弃 |
| 废弃 — withLockedProjectRoot / requireLocked | 正向 | 未锁定入口 / 锁错误纯 message 映射 / multi_root literal_env 锁 code | 废弃 |
| 废弃 — 有参工具改用锁定根（忽略 args.project_root） | 正向 | 传入 otherDir 仍回显锁定根 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `./lib/project-root` | `vi.mock`：`collectProjectRootCandidates`/`initProjectRootFromMcp` resolve；`resolveProjectRootForTool` 可控返回或抛 `ProjectRootResolveError`；`runWithCallScopedRoot` 默认透传 `fn()`；`isProjectRootResolveError` 用真实或 mock 守卫 | connect / handler / 错误映射 |
| commands / archi lib | 既有 spy：`runChangeList`/`runConfigGet`/`runPhaseLog`/… | 接线与短路 |
| `InMemoryTransport` + MCP Client | 真实协议握手与 `callTool`/`listTools` | schema 与 handler |
| 文件系统 | 读取仓库内 `plugins/dev-team/.mcp.json` | AC-10 |

---

## 集成测试

### connect 候选采集 → resolve 放行/拒绝 → `plugins/dev-team/bin/__tests__/mcp-workspace-root/mcp-workspace-root.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/bin/src/mcp.ts` | 触发方：connect、注册 tool、handler 包装 |
| `plugins/dev-team/bin/src/lib/project-root.ts` | 中间件：候选采集与 per-call resolve |
| `plugins/dev-team/bin/src/commands/change-list.ts`（及 config/phase 等） | 读取方：业务 I/O 使用 resolve 后的根 |

**关联AC**: AC-1, AC-2, AC-3, AC-4, AC-8, AC-9

**关系描述**:

真实 `connectToServer` + `InMemoryTransport` 不 mock `project-root`，验证宿主通道合并为 candidates 后，tool 必须携带合法 `project_root` 才能执行；∈ 候选时业务读到正确根，∉ 候选时返回结构化错误且无 cwd 副作用。该链路覆盖「只采集不锁定」与「必填参 + resolve」的组合行为，单元分层 mock 无法单独证明。

#### 场景: 多候选下显式选型成功

connect 后 WORKSPACE 含两个 temp 项目根（或 CLAUDE+roots 多路径）。调用 `change_list` 时 `arguments.project_root` 取其中之一；预期 `isError===false`，回显 `project_root` 等于所选路径，且能列出该项目下 changes。省略 `project_root` 时校验失败。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 双 WORKSPACE 可用根时传 ∈ 列表的 A：`change_list` 成功且 `project_root===A` (AC-1/AC-3) | 新增 |
| 正向 | 同进程再传 ∈ 列表的 B：成功且 `project_root===B`（证明无不可变单根 lock）(AC-1/AC-3) | 新增 |
| 异常 | 省略 `project_root`：`change_list`/`config_get`/`phase_next` 失败，command 无副作用 (AC-2) | 新增 |
| 边界 | `len==1` 候选时仍必须传 `project_root`；传该唯一候选则成功 (AC-1/AC-2) | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| MCP Client roots | `setRequestHandler(ListRootsRequestSchema)` 返回可控 roots；可选 `reinitAfterConnect` 规避握手竞态 | roots 通道 |
| `process.env` | 注入/清空 `CLAUDE_PROJECT_DIR`、`WORKSPACE_FOLDER_PATHS` | 多候选与空候选 |
| 临时项目目录 | `mkdtemp` + `openspec/config.json` + changes | 业务成功断言 |

#### 场景: ∉ 候选拒绝且无 cwd 副作用

候选仅含路径 A；调用传入存在但不在候选的路径 B。预期结构化 `not_in_candidates`（JSON 含 `candidates` 与 `force_hint`），且 `process.cwd()` 下无 openspec 写入；`config_get`/`phase_log` spy 或副作用检查为未执行。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 候选含 A 时传 A：`change_list`/`config_get` 成功且无 cwd 副作用（对照组）(AC-3/AC-8) | 新增 |
| 异常 | 合法 B∉candidates：`isError`，解析 text JSON：`code==='not_in_candidates'`，`candidates` 含 A，含 force 说明 (AC-4) | 新增 |
| 异常 | 三通道失败（空候选）且传合法绝对路径：第一次必拒，`candidates:[]`，cwd 无副作用 (AC-6/AC-8) | 新增 |
| 边界 | 子目录工作区（上层有 `.git`/`openspec`）作为候选并传入：成功回显等于子目录，不等于上层 (AC-9) | 新增 |
| 边界 | `project_root: ""` / 相对路径：结构化 `invalid_path` 或 schema 失败，cwd 无 openspec 写入 (AC-2/AC-8) | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| MCP Client roots | `setRequestHandler(ListRootsRequestSchema)` 返回可控 roots；可选关闭 capability 造空候选 | 候选仅含 A / 三通道失败 |
| `process.env` | 注入候选路径 A；或清空 `CLAUDE_PROJECT_DIR`/`WORKSPACE_FOLDER_PATHS` 造空候选 | 异常 / 边界（AC-4/AC-6） |
| 临时项目目录 | `mkdtemp` 创建 A（候选内）与 B（∉ 候选但存在）；可选上层 `.git`/`openspec/` 子目录工作区 | 拒绝无副作用 / AC-9 原样根 |
| `process.cwd` 副作用检查 | 记录 cwd 下 openspec 路径；断言拒绝后无新建写入；可选 spy command/lib 调用次数为 0 | 异常路径无 cwd 副作用（AC-8） |

#### 场景: 旧锁模型集成用例废弃

既有「无参 change_list 成功」「多 WORKSPACE 全部 tool 失败」「未锁定即失败」等硬锁集成用例整体废弃，目录可由 `mcp-project-root-lock` 重命名/替换为 `mcp-workspace-root`。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 无参 `change_list` 依赖锁定根成功 | 废弃 |
| 异常 | WORKSPACE 双路径时全部 tool 失败（不再成立：应可传参选型） | 废弃 |

---

### pending/force 全参再调 → candidates 扩充 → `plugins/dev-team/bin/__tests__/mcp-workspace-root/mcp-workspace-root.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/bin/src/lib/project-root.ts` | 写入方：pending 键、force 加入 candidates |
| `plugins/dev-team/bin/src/mcp.ts` | 触发方：两次 `callTool` 完整 arguments 透传 |
| `plugins/dev-team/bin/src/commands/config-get.ts` | 读取方：force 成功后才执行业务 |

**关联AC**: AC-5, AC-6, AC-7

**关系描述**:

验证跨模块的 force 协议：handler 不做独立 `force` 字段，而是依赖 `resolveProjectRootForTool` 对「相同 tool + 稳定序列化完整 arguments」的二次调用确认。集成层确保 MCP 序列化/透传不会破坏 pending 键匹配，并在 force 后真正执行业务 I/O。

#### 场景: 候选外路径二次确认放行

候选为 A；第一次 `config_get` 传 B（存在）失败并带 `force_hint`；第二次以**完全相同** arguments 再调：预期成功，`value`/`exists` 来自 B 项目配置；其后 `getProjectRootCandidates`（若可观测）或第三次传 B 应直接 ∈ 放行。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 同全参再调 force：第二次 `config_get` 成功且读取 B 下 config (AC-5) | 新增 |
| 异常 | 第二次仅改 `key` 字段：仍失败，不 force (AC-7) | 新增 |
| 边界 | 空候选两次相同全参：第二次放行并可读 B (AC-6) | 新增 |
| 边界 | force 成功后第三次传 B 直接成功（已在 candidates）(AC-5/AC-7) | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| 临时双项目 | A 在候选（env），B 仅磁盘存在；B 写入可区分的 `openspec/config.json` | force 前后读配置 |
| `process.env` | 仅注入 A 或清空以造空候选 | AC-5 / AC-6 |

---

### MCP schema 必填 project_root → handler 短路 → `plugins/dev-team/bin/__tests__/mcp-workspace-root/mcp-workspace-root.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/bin/src/schemas/phase-log.schema.ts` 等（全部触达项目树 schema） | 校验方：required `project_root` |
| `plugins/dev-team/bin/src/mcp.ts` | 中间件：未通过校验则不进入 resolve/业务 |

**关联AC**: AC-2

**关系描述**:

在真实 MCP 协议路径上确认 Zod input schema 的必填约束对全部 11 个工具生效，避免仅单测 mock `listTools` 形状与运行时 SDK 校验不一致。省略 `project_root` 时不得进入 `resolveProjectRootForTool` 或 command。

#### 场景: 全工具省略 project_root 均失败

在已 collect 单候选的连接上，对 11 个 tool 各发起一次缺少 `project_root` 的调用（其它字段给最小合法值）。预期全部失败，且无业务副作用。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 异常 | 11 个 tool 省略 `project_root` 均失败 (AC-2) | 新增 |
| 正向 | 同一连接补传候选内 `project_root` 后 `change_list`/`config_get` 成功 (AC-2/AC-3) | 新增 |
| 边界 | `phase_log`/`phase_next`/`backtrack`（原先无该字段）同样 required (AC-2) | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| `InMemoryTransport` + MCP Client | 真实协议握手与 `callTool`；不 mock schema 模块，走运行时 Zod/SDK 校验 | 11 工具省略参失败 / 补传成功 |
| `process.env` | 注入单可用候选路径，保证 connect 后 `len==1` | 场景前置 collect |
| 临时项目目录 | 单候选根写入最小 `openspec/config.json`，供补传后正向对照 | 正向补传成功 |

---

## 不可测试项

- `plugins/dev-team/bin/src/schemas/*.schema.ts`（各 schema 文件单独作为 `test_resolve_paths` 模块）— **原因**: `test_resolve_paths` 返回 `Not in test config scope`；契约改由 `mcp.test.ts`（直接 import schema + `listTools`）与 `__tests__/mcp-workspace-root` 协议级调用覆盖
- `plugins/dev-team/.mcp.json` 作为独立测试框架目标 — **原因**: `test_detect_frameworks` 返回 `framework: unknown`；AC-10 改为在 `mcp.test.ts` 内静态读取断言
- 插件版本号升级（`2.10.4` → `2.10.5`）与 `node scripts/build-plugins.mjs` 双端产物重建 — **原因**: 属发布/构建步骤，非业务行为自动化验收；人工或既有构建集成测试检查
- Cursor / Claude Code 真实多 workspace 宿主注入（`WORKSPACE_FOLDER_PATHS` / `CLAUDE_PROJECT_DIR` / roots 握手）端到端探针 — **原因**: 依赖真实 IDE 宿主；自动化以 env mock + InMemoryTransport 模拟通道
- `notifications/roots/list_changed` 热刷新 — **原因**: 提案明确不实现
- skill 内嵌根状态机 / 按 schema 填参的 agent 行为 — **原因**: 属 prompt/skill 约定，非本仓库可稳定单测的运行时逻辑
- `hooks.test.ts` 等对旧 `requireLockedProjectRoot` mock 的适应性修改 — **原因**: 测试维护项，不映射新 AC；实现阶段随 API 删除同步调整即可
