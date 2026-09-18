# 测试设计: phase-lifecycle-file-log

> **日期**: 2026-09-18

---

## 验收范围

<!-- 测试框架: vite-plus（vitest 风格 API，`vite-plus/test`），经 test_detect_frameworks 识别。
     单元测试路径经 test_resolve_paths 解析（colocated `*.test.ts`）；
     schemas/*.schema.ts 两文件不在测试配置 scope（解析 errors），其断言经消费方测试覆盖，见「不可测试项」。 -->

| AC ID | 验收条件 | 测试类型 | 被测文件或模块 |
|--------|---------|---------|----------|
| AC-1 | `workflowFileSchema` 接受 `active_phase`（对象或 null）与 `interrupted[]`；eval 条目接受可选 `start_at`；未知键保留行为不变（单测） | 单元测试 | `plugins/dev-team/bin/src/modules/workflow/doc-io.test.ts`、`plugins/dev-team/bin/src/modules/workflow/phase/phase-state.test.ts`、`plugins/dev-team/bin/src/lib/eval-json.test.ts` |
| AC-2 | phase_start 校验 change 与 phase 后 last-wins 写入 active_phase，attempt 由 eval 历史推导且 retry 后递增；非法 phase 报错且 workflow.json 不变（单测） | 单元测试 | `plugins/dev-team/bin/src/commands/phase-start.test.ts`、`plugins/dev-team/bin/src/mcp.test.ts`（phase_start 注册 / inputSchema / 调用落盘面） |
| AC-3 | active_phase 匹配时条目盖章 start_at 并清空 active_phase；无/不匹配时条目无 start_at 且 active_phase 不动；输出 `{written, phase, attempt}` 形状不变（单测） | 单元测试 | `plugins/dev-team/bin/src/commands/phase-log.test.ts` |
| AC-4 | record-files 门控四分支：未绑定丢弃；绑定无 active_phase 丢弃 + stderr 诊断；绑定+active+agent_type 匹配 → phase 记录；不匹配 → workflow 记录（单测） | 单元测试 | `plugins/dev-team/bin/src/commands/record-files.test.ts`（门①②③接线）、`plugins/dev-team/bin/src/lib/workflow.test.ts`（门③ extractAgentName / matchesExecutorAgent 归一）（集成：`plugins/dev-team/bin/__tests__/file-inventory-recording/`） |
| AC-5 | 重绑竞态回归（端到端）：phase_log 最终 pass 之后再发 phase_next 事件（Verdict，done:true）→ 绑定刷新但后续写事件仍被门②丢弃，归账不复活 | 集成测试 | `plugins/dev-team/bin/__tests__/file-inventory-recording/file-inventory-recording.test.ts` |
| AC-6 | file_log dedupe key=(scope 命名空间, attempt, path)：同 key 原位覆盖 op/scope/at（含 scope 翻转），跨 attempt/phase 追加；派生净状态与现行 `foldFileOps` 等价 | 单元测试 | `plugins/dev-team/bin/src/modules/workflow/files/file-inventory.test.ts`、`plugins/dev-team/bin/src/modules/workflow/files/record.test.ts` |
| AC-7 | 四读方（workflow_files / test-execution / test-resolve-paths / c4-cross-ref）行为不变改走派生视图；缺失 `file_log` 硬报错含重建指引，不回退 git | 单元测试 | `plugins/dev-team/bin/src/modules/workflow/files/files-query.test.ts`、`plugins/dev-team/bin/src/lib/c4-cross-ref.test.ts`、`plugins/dev-team/bin/src/commands/test-execution.test.ts`、`plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts`、`plugins/dev-team/bin/src/commands/change-create.test.ts`（`file_log: []` 初始化，闭合读方硬报错契约） |
| AC-8 | change_files：append → workflow scope 按 path upsert（phase 记录保留）；set → 删除涉及 path 全部记录 + 追加 workflow 记录，未涉及 path 不动（单测） | 单元测试 | `plugins/dev-team/bin/src/commands/change-files.test.ts` |
| AC-9 | phase_next 与 phase_start 事件均建立/刷新 `session_id → change` 绑定；注册表其余行为不变（单测） | 单元测试 | `plugins/dev-team/bin/src/commands/record-files.test.ts`（集成：`bin/__tests__/file-inventory-recording/`、`plugins/dev-team/build/__tests__/hooks-user-prompt-submit/`） |
| AC-10 | sweep：绑定 change 有遗留 active_phase → 移入 interrupted[]（end_at=now）并清空，eval 不变；无绑定 no-op；任何失败 exit 0；canonical 增 userPromptSubmit 键，claude 注册、cursor 不产出 | 单元测试 | `plugins/dev-team/bin/src/commands/sweep-phase.test.ts`、`plugins/dev-team/bin/src/modules/workflow/phase/phase-state.test.ts`、`plugins/dev-team/bin/src/hooks.test.ts`、`plugins/dev-team/build/hooks-profile.test.ts`（集成：`bin/__tests__/phase-lifecycle/`、`build/__tests__/hooks-user-prompt-submit/`） |
| AC-11 | 各 phase 技能 SKILL.md 含 phase_next → phase_start 序列，retry 重跑重新 phase_start（grep 技能文本核验） | 单元测试 | `plugins/dev-team/bin/__tests__/skill-phase-start-protocol/skill-phase-start-protocol.test.ts`（新增，SKILL.md 文本断言，参照 `eval-store-skill-copy` 模式） |
| AC-12 | 旧 files-only change 经各消费方硬报错；phase_next/backtrack 响应与 gate 行为零改动；extractFileOps / gitignore / 写保护行为不变（既有测试通过） | 单元测试 | `plugins/dev-team/bin/src/commands/phase-next.test.ts`（本变更唯一修改面：断言面修订，见对应章节） |

<!-- AC-12 其余被测面为既有回归测试、被测源位于 proposal「不要修改」边界内零改动，无需修改这些测试文件，
     故不列入本表、亦不新增 per-file 章节（原样通过即为验收）：
     - backtrack 引擎零改动 → 既有 `plugins/dev-team/bin/src/commands/backtrack.test.ts`
     - extractFileOps 三态提取零改动 → 既有 `plugins/dev-team/bin/src/lib/shell-file-ops.test.ts`
     - gitignore 过滤语义零改动 → 既有 `plugins/dev-team/bin/src/modules/workflow/files/gitignore.test.ts`
     - 写保护（protect-files）零改动 → 既有断言落 `plugins/dev-team/bin/src/hooks.test.ts`（章节已列于 AC-10 行）
     legacy files-only 硬报错文案（AC-12 消费方面）已分布于各读方章节（files-query / test-execution /
     test-resolve-paths / c4-cross-ref / record / change-files）。 -->

---

## 单元测试

<!-- 覆盖所有可在进程内验证的场景。每个源文件对应一个独立的 `### <源文件> -> <测试文件>` 章节。
     文件系统一律不 mock：mkdtempSync 临时目录 + 真盘读写（仓库既有惯例）；
     hook 入口类测试仅 mock 进程边界（stdin fd 0 / os.tmpdir / getProjectDir / process.exit·stderr）。 -->

### plugins/dev-team/bin/src/modules/workflow/doc-io.ts -> plugins/dev-team/bin/src/modules/workflow/doc-io.test.ts

#### 待测功能

<!-- 新增文件（module 根共享读写原语），函数命名以实现为准，职责边界以 design.md「架构组件 / 新增文件」为准 -->

- workflow.json 文档读取原语（read*，命名以实现为准）: 读取并经 `workflowFileSchema` 校验 `<changeDir>/workflow.json`，文件缺失 / JSON 非法 / 根非对象 / 格式非法分别抛对应错误
- workflow.json 文档写回原语（write*，命名以实现为准）: 文件必须已存在，未知键原样保留写回，2 空格缩进 + 末尾换行，绝不创建文件或目录

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 读取原语 | 正向 | 合法 workflow.json（含 `workflow_type`/`created`/`eval`/未知键）返回解析后文档对象 | 新增 |
| 读取原语 | 边界 | 含新字段 `active_phase` / `interrupted` / `file_log` 的文档通过校验（AC-1 schema 扩展经消费方断言）；`active_phase: null` 合法 | 新增 |
| 读取原语 | 异常 | 文件不存在 → 抛「workflow.json 不存在」含 change_create 指引且不创建任何文件 | 新增 |
| 读取原语 | 异常 | JSON 截断 / 含注释 → 抛「解析失败」；根为数组/字符串/null → 抛「根元素必须是对象」 | 新增 |
| 读取原语 | 异常 | `workflow_type` 非法枚举等 schema 违例 → 抛「格式非法」含 issue 明细 | 新增 |
| 写回原语 | 正向 | 写回后未知键 / workflow_type / created / eval / 新运行态字段逐字保留，2 空格缩进 + 尾换行，可再解析 | 新增 |
| 写回原语 | 异常 | 文件不存在 → 抛错且不创建文件与目录（绝不创建契约）；JSON 非法 → 抛错且磁盘逐字节不变 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | mkdtempSync 临时目录 + 真实 fs（同 file-inventory.test.ts 模式） | 全部用例 |

---

### plugins/dev-team/bin/src/modules/workflow/phase/phase-state.ts -> plugins/dev-team/bin/src/modules/workflow/phase/phase-state.test.ts

#### 待测功能

- readActivePhase(changeDir): 读 `active_phase` 运行态，字段缺失或 null 视为无运行态返回 null；文件缺失/非法抛错
- writeActivePhase(changeDir, active): last-wins 覆盖写入 `{phase, attempt, start_at}`，未知键保留
- clearActivePhase(changeDir): 置 null，`interrupted` 不动（`phase_log` 清场用）
- interruptActivePhase(changeDir, endedAt?): 有遗留 active_phase 时以 end_at（缺省 now）追加进 `interrupted[]` 并清空，返回是否发生归档（sweep 用）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| writeActivePhase/readActivePhase | 正向 | 写入后读回 `{phase, attempt, start_at}` 一致；workflow_type/created/eval/未知键保留；2 空格缩进 + 尾换行 | 新增 |
| writeActivePhase | 正向 | 已有 active_phase 时再次写入 → last-wins 覆盖（start_at 刷新、attempt 不变的重入语义由调用方保证） | 新增 |
| writeActivePhase | 异常 | attempt 非正整数 / phase 非法枚举 / start_at 非 ISO → 校验抛错且磁盘不变 | 新增 |
| readActivePhase | 边界 | `active_phase` 字段缺失或为 null → 返回 null（不视为非法） | 新增 |
| readActivePhase | 异常 | 文件缺失 → 抛错不创建；JSON 非法 → 抛错（由调用方吞错策略兜底） | 新增 |
| clearActivePhase | 正向 | 有运行态时清空 → readActivePhase 为 null；`interrupted` 数组原样不动；eval 不动 | 新增 |
| clearActivePhase | 边界 | 本无运行态（null/缺失）→ 置 null 幂等，文件其余内容不变 | 新增 |
| interruptActivePhase | 正向 | 有遗留 → 追加 `{phase, attempt, start_at, end_at}` 进 interrupted[] 并清空 active_phase，返回 true；显式传入 endedAt 时使用传入值 | 新增 |
| interruptActivePhase | 边界 | 无遗留 → 返回 false，文件逐字节不变 | 新增 |
| interruptActivePhase | 边界 | interrupted[] 已有条目 → 追加不覆盖；eval 数组不变（AC-10 不写 eval） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 系统时钟（interruptActivePhase 缺省 end_at=now） | `vi.useFakeTimers` 或 `vi.setSystemTime` 固定时刻断言 end_at | interruptActivePhase 缺省参数用例 |

---

### plugins/dev-team/bin/src/modules/workflow/files/file-inventory.ts -> plugins/dev-team/bin/src/modules/workflow/files/file-inventory.test.ts

#### 待测功能

- readFileLog(changeDir): 读 `file_log[]`；缺失 `file_log` 硬报错含「重建 change」指引，不回退 git
- appendLogEntries(changeDir, entries): (scope 命名空间, attempt?, path) 键控原位覆盖/追加后落盘，返回落盘后 log
- deriveNetState(log): 纯函数，同 path 后条胜；末条 write→written、delete→deleted、revert→双桶均不含；与现行 `foldFileOps` 等价
- appendWorkflowFiles(changeDir, paths): workflow scope 按 path upsert，phase 审计记录保留，不过滤 gitignore
- setWorkflowFiles(changeDir, paths): 删除涉及 path 全部记录 + 末尾追加 workflow 记录；未涉及 path 不动
- （移除）readFileInventory / foldFileOps / writeFileInventory / appendFileOps / setFileBuckets: 桶形态 API 不留 shim

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| readFileLog | 正向 | 含 `file_log` 条目（write/delete/revert × phase/workflow scope）的文档原样读回，条目字段 op/scope/attempt?/path/at 透传 | 新增 |
| readFileLog | 边界 | `file_log: []`（change_create 初始形态）→ 返回空数组 | 新增 |
| readFileLog | 异常 | legacy files-only change（有 `files` 无 `file_log`）→ 硬报错文案含「重建 change」指引，不回退 git（AC-7/AC-12） | 新增 |
| readFileLog | 异常 | 文件缺失 / JSON 非法 / 根非对象 / 格式非法 → 沿用读取纪律四态报错（迁移自 readFileInventory 异常组） | 新增 |
| appendLogEntries | 正向 | phase scope 同 key `(phase-id, attempt, path)` 二次写入 → 原位覆盖 op/scope/at 且保留原数组位置（AC-6） | 新增 |
| appendLogEntries | 正向 | workflow scope 同 key `('workflow', path)` 二次写入 → 原位覆盖；workflow 条目无 attempt 字段 | 新增 |
| appendLogEntries | 正向 | 同 path 跨 attempt / 跨 phase → 追加 log 末尾（跨 key 不覆盖） | 新增 |
| appendLogEntries | 边界 | phase 记录后同 path 写 workflow 记录（scope 翻转）→ 属跨 key：追加新记录，日志保留全部来源轨迹，净状态由派生后条胜生效（AC-6/design 决议） | 新增 |
| appendLogEntries | 边界 | entries=[] → 不写盘，返回现有 log | 新增 |
| appendLogEntries | 异常 | 缺失 file_log → 硬报错且磁盘不变（写前读纪律） | 新增 |
| deriveNetState | 正向 | 末条 write → written 含该 path 且移出 deleted；末条 delete → deleted 含该 path 且移出 written | 新增 |
| deriveNetState | 正向 | 末条 revert → written 与 deleted 均不含该 path（revert 净消除，design 决议） | 新增 |
| deriveNetState | 正向 | 同 path 多条按数组顺序后条胜；phase 后条覆盖前条 workflow（反之亦然） | 新增 |
| deriveNetState | 边界 | 空 log → `{written: [], deleted: []}`；入参 log 不被修改（纯函数不变量） | 新增 |
| deriveNetState | 边界 | 等价回归：现行 `foldFileOps` 用例表全部序列（write→delete、revert、mv 双条目、10+ 步长序列、单桶不变量扫描）改以 deriveNetState 断言，期望值逐一一致（AC-6 等价测试重做） | 新增 |
| appendWorkflowFiles | 正向 | 追加 written/deleted path → 追加 workflow scope 记录，返回派生净状态；同 path 已有 workflow 记录 → 原位覆盖（upsert 去重） | 新增 |
| appendWorkflowFiles | 边界 | 已有 phase scope 审计记录（同 path 不同 key）→ 逐字保留不被 upsert 触碰（AC-8） | 新增 |
| appendWorkflowFiles | 边界 | ignored 路径（项目根 .gitignore 命中）→ 不过滤照常入 log（人工通道契约，迁移既有 AC-5 用例） | 新增 |
| setWorkflowFiles | 正向 | provided = written ∪ deleted → 删除 log 中涉及 path 的全部记录（任意 scope/attempt/op），末尾追加 workflow 记录（written 先、deleted 后） | 新增 |
| setWorkflowFiles | 边界 | 同 path 同时出现在 written 与 deleted → deleted 胜（桶序约定） | 新增 |
| setWorkflowFiles | 边界 | 未涉及 path 的记录（含 phase 审计）逐字保留、顺序不变（AC-8） | 新增 |
| 写盘纪律（append/set 共用） | 正向 | 落盘后 workflow_type/created/eval/active_phase/interrupted/未知键保留、2 空格缩进 + 尾换行 | 新增 |
| 写盘纪律 | 异常 | workflow.json 不存在 → 抛错不创建；非法 → 抛错且磁盘逐字节不变（迁移自 writeFileInventory 异常组） | 新增 |
| （废弃桶 API） | 异常 | `readFileInventory` / `foldFileOps` / `writeFileInventory` / `appendFileOps` / `setFileBuckets` 导出不存在（import 断言 undefined 或编译期失败） | 废弃 |
| （废弃 fold 语义） | 边界 | 旧「foldFileOps 折叠规则 / source 旁挂维护（agentType 盖章、last-writer-wins、清除）」用例组：随桶存储废除，由 deriveNetState 等价断言与 scope 字段断言取代 | 废弃 |
| （废弃桶读写） | 边界 | 旧「appendFileOps / setFileBuckets 读改写（整桶覆写、未提供桶不动、source 保留清理）」用例组：由 appendWorkflowFiles / setWorkflowFiles 新语义用例取代 | 废弃 |

<!-- 另：modules/workflow/index.test.ts（barrel 出口）同步更新——新增 readFileLog / appendLogEntries / deriveNetState /
     appendWorkflowFiles / setWorkflowFiles 及 phase-state 四函数的导出与引用一致性断言，移除旧桶 API 的
     「导出面精确」断言（Object.keys 清单改为新函数名集合）；不新增独立章节。 -->

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | mkdtempSync 临时目录 + 真实 fs + validWorkflowDoc fixture（改写为含 `file_log` 的新 fixture） | 全部用例 |

---

### plugins/dev-team/bin/src/modules/workflow/files/record.ts -> plugins/dev-team/bin/src/modules/workflow/files/record.test.ts

#### 待测功能

- recordFileOps(changeDir, ops, context): 上下文由 `{projectRoot, agentType?}` 改为 `{projectRoot, scope: RecordScope}`；管线顺序不变（规范化 → 自污染排除 → gitignore 过滤 → appendLogEntries 落盘），条目带 scope/attempt/at

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| recordFileOps | 正向 | scope={kind:'phase', phase:'implement', attempt:2} → 落盘条目 scope='implement'、attempt=2、at 为 ISO 时刻（AC-6/AC-4 管线层） | 新增 |
| recordFileOps | 正向 | scope={kind:'workflow'} → 落盘条目 scope='workflow' 且无 attempt 字段（AC-4 主 agent 分支的记录形态） | 新增 |
| recordFileOps | 正向 | 规范化回归：绝对路径 / Windows 反斜杠 → 项目根相对 POSIX 化；越界（../）丢弃 | 新增（迁移保留） |
| recordFileOps | 正向 | 自污染排除与 gitignore 过滤回归：openspec/** 与 workflow.json 不入 log；ignored 路径三态 op 统一过滤；不回溯清洗既有条目；无 .gitignore 时无 stderr 副作用 | 新增（迁移保留） |
| recordFileOps | 边界 | ops=[] / 全部越界 → 不读不写 workflow.json（磁盘逐字节不变） | 新增（迁移保留） |
| recordFileOps | 边界 | `op:'revert'` 事件照常过滤并作为 revert 记录入 log（与 gitignore 过滤叠加） | 新增 |
| recordFileOps | 异常 | legacy change（缺 file_log）→ 抛错向上传播（文案为新「重建 change」指引），文件不被半写 | 新增（文案更新） |
| recordFileOps | 边界 | 落盘后 workflow_type/created/eval/active_phase/interrupted/未知键保留、缩进纪律不变 | 新增（迁移保留） |
| recordFileOps | 边界 | 旧「agentType 盖章 / context.agentType 兜底 / 无 agentType 清除（source 旁挂）」用例组：`FileOp.agentType` 字段与 source 映射废除，由 scope 上下文取代 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 系统时钟（条目 at 字段） | `vi.setSystemTime` 固定时刻断言 at | scope 盖章正向用例 |
| process.stderr.write | vi.spyOn 捕获诊断（.gitignore fail-open 场景沿用） | gitignore 过滤叠加 / fail-open 用例 |

---

### plugins/dev-team/bin/src/modules/workflow/files/files-query.ts -> plugins/dev-team/bin/src/modules/workflow/files/files-query.test.ts

#### 待测功能

- getChangedFiles(changeName, projectRoot): 签名与输出形状不变，实现改为 `deriveNetState(readFileLog(changeDir))`；严格只读，硬报错不回退 git

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| getChangedFiles | 正向 | file_log 混合 phase/workflow 条目（含同 path 后条胜、revert 净消除）→ 返回派生 `{written, deleted}`，输出对象无 scope/attempt 等审计明细键（AC-7） | 新增 |
| getChangedFiles | 边界 | `file_log: []` → 双空数组，不补建字段 | 新增 |
| getChangedFiles | 异常 | 缺失 file_log（legacy files-only）→ 硬报错含「重建 change」指引，错误文案不含 diff 输出（不回退 git，AC-7/AC-12） | 新增 |
| getChangedFiles | 异常 | 四态错误（文件缺失 / JSON 非法 / 根非对象 / 格式非法）语义沿用，错误路径下磁盘逐字节不变 | 新增（迁移保留） |
| getChangedFiles | 边界 | 只读回归：查询前后 workflow.json 逐字节一致、change 目录无新增文件；特殊路径（空串/超长/中文/emoji）透传 | 新增（迁移保留） |
| getChangedFiles | 边界 | 旧「files 净状态投影 / source 审计映射排除」用例组：桶存储与 source 映射废除 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | mkdtempSync 临时项目 + 含 file_log 的 workflow.json fixture | 全部用例 |

---

### plugins/dev-team/bin/src/commands/phase-start.ts -> plugins/dev-team/bin/src/commands/phase-start.test.ts

#### 待测功能

- runPhaseStart(options: PhaseStartOptions): PhaseStartResult — 薄适配：校验 change 存在、workflow.json 合法、phase 属于该 `workflow_type` phase 表；attempt 由 `computeAttempt(readEvalJson(changeDir), phase)` 推导；委托 `writeActivePhase` last-wins 写入；非法 phase 报错且文件不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| runPhaseStart | 正向 | 合法 change + phase 表内 phase → 返回 `{started: true, phase, attempt, start_at}`；workflow.json 落盘 `active_phase{phase, attempt, start_at}`（AC-2） | 新增 |
| runPhaseStart | 正向 | 该 phase 既有 1 条 fail eval 条目 → attempt=2（computeAttempt 推导，retry 后递增）；无条目 → attempt=1 | 新增 |
| runPhaseStart | 正向 | 重入 last-wins：已有 active_phase（他 phase）时再次调用 → 覆盖为新 phase 与新 start_at，不报错 | 新增 |
| runPhaseStart | 正向 | workflow_type=bug-fix / test-only → 按对应 phase 表校验归属（如 test-only 接受 code-analyze） | 新增 |
| runPhaseStart | 异常 | phase 不属于该 workflow_type phase 表（requirement 传 code-analyze）→ 报错且 workflow.json 逐字节不变（AC-2） | 新增 |
| runPhaseStart | 异常 | change 不存在 / workflow.json 缺失 → 报错含 change_create 指引，不创建文件；JSON 非法 → 报错 | 新增 |
| runPhaseStart | 边界 | phase 为 9 值枚举外的值 / 空串 → 输入 schema 校验拒绝 | 新增 |
| runPhaseStart | 边界 | project_root 缺省 → getProjectDir() 兜底解析 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | mkdtempSync 临时项目 + change_create / 手写 workflow.json fixture 真盘 | 全部用例 |

---

### plugins/dev-team/bin/src/commands/sweep-phase.ts -> plugins/dev-team/bin/src/commands/sweep-phase.test.ts

#### 待测功能

- runSweepPhase(): void — UserPromptSubmit hook 子命令：读 stdin `session_id` → `lookupChange` → `interruptActivePhase`；无绑定 no-op；任何失败仅 stderr、exit 0

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| runSweepPhase | 正向 | 绑定 change 有遗留 active_phase → 移入 interrupted[]（end_at=now）并清空；eval 数组不变（AC-10） | 新增 |
| runSweepPhase | 正向 | 绑定 change 无 active_phase（null/缺失）→ no-op，interrupted 不增长 | 新增 |
| runSweepPhase | 边界 | 连续两条消息（两次调用）→ 第二次无遗留可归档，interrupted 仅一条 | 新增 |
| runSweepPhase | 边界 | session 未绑定 → 静默 no-op（stderr 无诊断），exit 0 | 新增 |
| runSweepPhase | 异常 | stdin 非 JSON / 空串 / session_id 缺失 → 静默 exit 0 不抛 | 新增 |
| runSweepPhase | 异常 | workflow.json 缺失或非法（interruptActivePhase 抛错）→ stderr 诊断 + exit 0，不阻塞（fail-open，AC-10） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| stdin（readFileSync fd 0） | vi.mock `node:fs`/`fs` 仅替换 readFileSync，fd 0 调用按队列弹出（参照 file-inventory-recording 注入模式） | 全部用例 |
| session 注册表落盘 | vi.mock `node:os` 将 tmpdir 重定向到每用例隔离目录 | 绑定/未绑定用例 |
| getProjectDir | vi.mock 指向临时项目根（不用 process.chdir） | 全部用例 |
| process.exit / stderr | vi.spyOn 拦截（exit 断言未被调用，stderr 捕获诊断） | fail-open 用例 |

---

### plugins/dev-team/bin/src/commands/phase-log.ts -> plugins/dev-team/bin/src/commands/phase-log.test.ts

#### 待测功能

- runPhaseLog(options: PhaseLogOptions): PhaseLogResult — 落盘前读 active_phase：匹配时 buildEntry 携带 start_at；appendEntry 成功后清空 active_phase；清场失败仅 stderr 吞错；输出 `{written, phase, attempt}` 形状不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| runPhaseLog | 正向 | active_phase.phase === 本次 phase → 落盘条目含 `start_at === active_phase.start_at`，且 active_phase 被清空（AC-3） | 新增 |
| runPhaseLog | 正向 | 无 active_phase → 条目无 start_at 键且 active_phase 保持 null/缺失不动（AC-3） | 新增 |
| runPhaseLog | 异常 | active_phase.phase 不匹配（他 phase 运行态）→ 条目无 start_at 且 active_phase 原样保留（AC-3 防御行为） | 新增 |
| runPhaseLog | 异常 | appendEntry 失败（如缺 workflow.json）→ 不清空 active_phase（盖章在落盘前、清场在成功后的时序防回归），错误照常包装抛出 | 新增 |
| runPhaseLog | 异常 | 清场（clearActivePhase）抛错 → 仅 stderr 诊断，不抛错，仍返回 `{written: true, phase, attempt}`，条目已落盘（吞错收尾无双录，AC-3/design 决议） | 新增 |
| runPhaseLog | 边界 | 输出形状契约：返回对象键恰为 written/phase/attempt（回归） | 新增（迁移保留） |
| runPhaseLog | 边界 | verdict 推导、report 500 字符边界、skipped、attempt 显式传参、非法枚举等既有用例全部回归不因盖章集成改变 | 新增（迁移保留） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无（真盘为主） | mkdtempSync + 手写含 active_phase 的 workflow.json fixture；沿用既有 eval-json mock 模式处仅保留既有依赖注入 | 盖章/清场/防御用例；清场抛错用例经 vi.mock `modules/workflow`（phase-state 层）注入失败 |

---

### plugins/dev-team/bin/src/commands/record-files.ts -> plugins/dev-team/bin/src/commands/record-files.test.ts

#### 待测功能

- runRecordFiles(): void — PostToolUse hook 入口：stdin 解析；`phase_next` / `phase_start` MCP 事件识别并绑定刷新；门①（绑定）→ 门②（readActivePhase）→ 门③（getWorkflowType + getPhaseTable + matchesExecutorAgent）按序判定；scope 解析 `{kind:'phase',phase,attempt} | {kind:'workflow'}` 传入 recordFileOps；门②不满足丢弃 + stderr 诊断；任何错误吞并 exit 0

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 绑定刷新 | 正向 | `mcp__…__phase_start` 事件（tool_input.change）→ 建立/刷新 `session_id → change` 绑定且不归账（AC-9） | 新增 |
| 绑定刷新 | 正向 | `phase_next` 事件建绑回归（现状语义，AC-9） | 新增（迁移自 e2e，落单测） |
| 门① | 异常 | 未绑定 session 的写事件 → 丢弃 + stderr「未绑定」，workflow.json 不变（AC-4 分支①） | 新增 |
| 门② | 异常 | 已绑定但无 active_phase → 丢弃 + stderr 诊断（区别于门①文案），workflow.json 不变（AC-4 分支②） | 新增 |
| 门②+门③ | 正向 | 绑定 + active_phase + 事件 `agent_type: dev-team:implementation-generator` 与运行 phase executor token `__CALL_AGENT:implementation-generator__` 归一相等 → scope={kind:'phase',phase,attempt=active_phase.attempt} 记录（AC-4 分支③） | 新增 |
| 门②+门③ | 正向 | 绑定 + active_phase + 无 agent_type（主 agent）→ scope={kind:'workflow'} 记录（AC-4 分支④） | 新增 |
| 门③ | 边界 | cursor-home 产物前缀 `dev-team_<id>` 归一后与 token 相等 → phase scope（build/env 前缀表一致） | 新增 |
| 门③ | 边界 | 运行 phase 为 `executor: null`（code-review/acceptance）→ 门③不匹配 → workflow scope | 新增 |
| 门③ | 边界 | 事件无 agent_type 字段（undefined）→ 不匹配 → workflow scope（不报错） | 新增 |
| 门②重开 | 正向 | phase_log 清场后（active_phase=null）同 session 再写 → 门②丢弃；重新 phase_start 后恢复归账 | 新增 |
| fail-open | 异常 | 非法 stdin / tool_input 结构残缺 / recordFileOps 抛错（legacy change）→ stderr + exit 0，不阻塞观察的工具调用 | 新增（迁移保留） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| stdin（readFileSync fd 0） | vi.mock 仅替换 readFileSync，事件 JSON 队列按序弹出 | 全部用例 |
| os.tmpdir | 重定向隔离目录承载 session 注册表 | 绑定相关用例 |
| getProjectDir | vi.mock 指向临时项目根 | 全部用例 |
| process.exit / stderr | vi.spyOn 拦截与捕获 | 吞错/诊断断言用例 |

---

### plugins/dev-team/bin/src/commands/change-files.ts -> plugins/dev-team/bin/src/commands/change-files.test.ts

#### 待测功能

- runChangeFiles(options: ChangeFilesOptions): ChangeFilesOutput — 签名与输出形状不变；委托改为 `appendWorkflowFiles` / `setWorkflowFiles` 新语义

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| runChangeFiles(append) | 正向 | append written/deleted → 追加 workflow scope 记录，返回派生净状态（AC-8） | 新增 |
| runChangeFiles(append) | 边界 | 已有 phase scope 审计记录同 path → upsert 仅覆盖 workflow 命名空间，phase 记录保留（AC-8） | 新增 |
| runChangeFiles(append) | 边界 | ignored 路径不过滤（人工补录通道契约回归）；同批重复路径去重 | 新增（迁移保留） |
| runChangeFiles(set) | 正向 | set → log 中涉及 provided path 的全部记录（任意 scope/attempt/op）删除 + 末尾追加 workflow 记录；未涉及 path 不动（AC-8） | 新增 |
| runChangeFiles(set) | 边界 | 同 path 出现在 written 与 deleted 两桶 → deleted 胜 | 新增 |
| runChangeFiles(set) | 边界 | phase 审计记录中涉及 provided path → 一并删除（spec：删除涉及 path 全部记录）；不涉及 → 逐字保留 | 新增 |
| runChangeFiles | 异常 | change 不存在 / workflow.json 非法 / 缺 file_log → 硬报错含「重建 change」指引 | 新增（文案更新） |
| runChangeFiles | 边界 | 输出仅含 written/deleted 两字段（形状契约回归）；written 与 deleted 均缺省 → schema 拒绝 | 新增（迁移保留） |
| runChangeFiles(set) | 边界 | 旧「set 整桶覆写、未提供桶保持原样」「source 保留/清理」断言组：set 语义反转为按 path 删记录重写、source 映射废除 | 废弃 |
| runChangeFiles(append) | 边界 | 旧「append 已存在路径既有 source 保留」断言：source 旁挂废除 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | mkdtempSync 临时项目 + 含 file_log（可含 phase 条目）的 workflow.json fixture | 全部用例 |

---

### plugins/dev-team/bin/src/commands/change-create.ts -> plugins/dev-team/bin/src/commands/change-create.test.ts

#### 待测功能

- runChangeCreate(name, projectRoot, workflowType): 创建 change 目录与 workflow.json；初始化字段由 `files: {written: [], deleted: []}` 改为 `file_log: []`，键序契约 `workflow_type → created → file_log`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| runChangeCreate | 正向 | 创建成功 → workflow.json 含 `file_log: []`、不含 `files` 键；键序 workflow_type → created → file_log（AC-7 必需补充） | 新增 |
| runChangeCreate | 边界 | 新建 workflow.json 可被 readFileLog 读取（空 log）——与读方硬报错契约闭环 | 新增 |
| runChangeCreate | 异常/边界 | kebab-case 校验、超长名、已存在 change、四种 workflow_type → 既有用例回归 | 新增（迁移保留） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | mkdtempSync 临时项目真盘 | 全部用例 |

---

### plugins/dev-team/bin/src/commands/phase-next.ts -> plugins/dev-team/bin/src/commands/phase-next.test.ts

#### 待测功能

- runPhaseNext(options): 引擎响应协议与 gate/round 逻辑零改动
- hasPhasePassed(entries, phaseId): 实现自 `../lib/workflow` 导入并 re-export（原样下沉，行为零改动，既有导入路径 `commands/phase-next` 保持可用）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| hasPhasePassed（re-export 后） | 正向/边界 | 既有 describe「hasPhasePassed — 导出函数」全部用例（pass/skipped/stale/空数组/多 phase 混存）经 re-export 路径断言不变（AC-12 零改动回归） | 新增（迁移保留） |
| runPhaseNext | 边界 | workflow.json 含新字段 active_phase/file_log/interrupted（looseObject）时照常解析推进，新字段不参与 gate 计算（AC-12） | 新增 |
| runPhaseNext | 边界 | 引擎行为零改动回归：First Run / Normal Progression / Retry / Backtrack / Round Limit / session window / done 全部既有用例通过 | 新增（迁移保留） |
| runPhaseNext | 异常 | 严格前置条件（缺 workflow.json、缺 workflow_type、非法枚举）回归——不含「缺 files」类清单断言（清单缺失不再是 phase_next 前置） | 新增（断言面修订） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | mkdtempSync 临时项目 + workflow.json fixture（沿用既有模式） | 全部用例 |

---

### plugins/dev-team/bin/src/commands/test-execution.ts -> plugins/dev-team/bin/src/commands/test-execution.test.ts

#### 待测功能

- runTestExecution(options): 突变 scope 文件集解析（内部 `resolveMutationDiffFiles`）改读派生 written，行为不变

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 突变 scope 解析 | 正向 | change 的 file_log 派生 written 非空 → 突变 scope 取该集合（经 runTestExecution 入口或既有测试挂点断言，AC-7） | 新增 |
| 突变 scope 解析 | 边界 | 派生 written 为空（`file_log: []`）→ 现行「无突变文件」行为不变 | 新增 |
| 突变 scope 解析 | 异常 | 缺失 file_log → 硬报错含重建指引（迁移既有 legacy 断言，文案更新） | 新增 |
| 突变 scope 解析 | 边界 | 旧基于 `files` 桶构造 fixture 的用例组：桶存储废除 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | mkdtempSync 临时项目 + workflow.json fixture（沿用既有测试模式） | 全部用例 |

---

### plugins/dev-team/bin/src/commands/test-resolve-paths.ts -> plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts

#### 待测功能

- runTestResolvePaths(args): `modules === 'change'` 模式改读派生 written，错误语义不变（硬报错、不回退 git）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| modules='change' | 正向 | file_log 派生 written 作为模块清单解析测试路径（输出形状不变，AC-7） | 新增 |
| modules='change' | 异常 | 缺失 file_log → 硬报错含重建指引且不回退 git（迁移既有断言，文案更新，AC-7/AC-12） | 新增 |
| modules='change' | 边界 | 派生 written 为空 → 现行空清单行为不变 | 新增 |
| 其他模式 | 边界 | 显式 modules 数组 / 目录模式既有用例回归不动 | 新增（迁移保留） |
| modules='change' | 边界 | 旧基于 `files` 桶 fixture 的用例组：桶存储废除 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | mkdtempSync 临时项目 + workflow.json fixture（沿用既有测试模式） | 全部用例 |

---

### plugins/dev-team/bin/src/lib/workflow.ts -> plugins/dev-team/bin/src/lib/workflow.test.ts

#### 待测功能

- extractAgentName(agentRef: string): string — 门③归一：`__CALL_AGENT:<id>__` 提取 `<id>`；剥离 `dev-team:` / `dev-team_` 前缀；其余原样返回
- matchesExecutorAgent(eventAgentType: string | undefined, executorAgentType: string | null | undefined): boolean — 双方归一后 bare id 相等才 true；任一缺失为 false
- hasPhasePassed(entries, phaseId): 自 `commands/phase-next.ts` 原样下沉（逻辑逐字不变，参数改结构化条目类型）
- getPhaseTable(workflowType) / getDependents(phaseId, workflowType): 既有，不动

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| extractAgentName | 正向 | `__CALL_AGENT:proposal-planner__` → `proposal-planner`；`dev-team:implementation-generator` → `implementation-generator`；`dev-team_implementation-generator` → `implementation-generator` | 新增 |
| extractAgentName | 边界 | 无前缀无 token 的裸 id（`implementation-generator`）→ 原样返回；含冒号的其余形态原样返回（自研提取层契约） | 新增 |
| extractAgentName | 异常 | 空串 → 空串（与 matchesExecutorAgent 的缺失判定衔接） | 新增 |
| matchesExecutorAgent | 正向 | `dev-team:implementation-generator` vs `__CALL_AGENT:implementation-generator__` → true（跨平台产物形态归一相等） | 新增 |
| matchesExecutorAgent | 正向 | 双方同为裸 id 且相等 → true | 新增 |
| matchesExecutorAgent | 异常 | 任一为 undefined / null / 空串 → false（缺失不误判） | 新增 |
| matchesExecutorAgent | 异常 | executor 为 null（code-review/acceptance 无 executor）→ false | 新增 |
| matchesExecutorAgent | 边界 | 归一后不相等（不同 agent）→ false | 新增 |
| hasPhasePassed（下沉后） | 正向/边界 | 既有纯函数用例（pass/skipped/stale/空数组/多 phase）自 lib 路径导入断言等价；`commands/phase-next` re-export 与 lib 导出为同一引用（toBe） | 新增 |
| getPhaseTable | 边界 | 四种 workflow_type 相位表既有断言回归不动（门③数据源） | 新增（迁移保留） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 纯函数直接导入断言（同 lib/workflow.test.ts 既有模式） | 全部用例 |

---

### plugins/dev-team/bin/src/lib/eval-json.ts -> plugins/dev-team/bin/src/lib/eval-json.test.ts

#### 待测功能

- buildEntry(params: BuildEntryParams): EvalEntry — `BuildEntryParams` 增可选 `start_at`，显式传入才写入条目；其余读写逻辑不动

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| buildEntry | 正向 | 显式传入 start_at（ISO 字符串）→ 条目含 start_at，其余字段（phase/attempt/verdict/report/checklist/timestamp）不变（AC-1） | 新增 |
| buildEntry | 正向 | 未传 start_at → 条目不含 start_at 键（可选字段不写入，与 skipped 同策略） | 新增 |
| buildEntry | 异常 | 必填字段缺失或非法（缺 phase、verdict 枚举外等）→ 组装层 schema.parse 收口抛错、不产出条目；新增可选 start_at 后该收口契约不变（自研组装层抛错面，zod 语法细节不逐项验证） | 新增 |
| buildEntry | 异常 | start_at 显式传非字符串值 → schema.parse 抛错、条目不落盘（可选字段一旦显式提供即受既有条目 schema 约束，组装层不自行兜底转换） | 新增 |
| buildEntry | 边界 | start_at 与 timestamp 并存 → 两字段各自独立保留（per-attempt 耗时 = timestamp − start_at 的数据前提） | 新增 |
| buildEntry | 边界 | 既有用例（skipped 条件写入、timestamp 自动生成、schema.parse 收口）回归 | 新增（迁移保留） |

<!-- 注：start_at 的 ISO 语法合法性属 zod 自带语义，按「不逐项验证库自带语义」惯例不单列用例；
     仅断言自研组装层的「显式传入才携带」行为。 -->

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 纯函数断言（沿用既有 eval-json.test.ts 模式） | 全部用例 |

---

### plugins/dev-team/bin/src/lib/c4-cross-ref.ts -> plugins/dev-team/bin/src/lib/c4-cross-ref.test.ts

#### 待测功能

- runCrossRefCheck(options): 被查文件集改读 `deriveNetState(readFileLog(...)).written`；`staged` 保持硬报错别名语义；`files` 显式入参优先级不变；不执行 `git diff --cached`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| 被查文件集 | 正向 | change file_log 派生 written 作为被查文件集参与交叉核对（结果与现行 files 桶形态一致，AC-7） | 新增 |
| 被查文件集 | 异常 | 缺失 file_log → 硬报错含重建指引（迁移既有 legacy 断言，文案更新） | 新增 |
| staged / files 入参 | 边界 | `staged` 别名硬报错语义、显式 `files` 入参优先于清单、无 `git diff --cached` 调用 → 既有断言回归 | 新增（迁移保留） |
| 被查文件集 | 边界 | 旧基于 `files` 桶 fixture 的用例组：桶存储废除 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | mkdtempSync 临时项目 + workflow.json fixture（沿用既有 c4-cross-ref.test.ts 模式） | 全部用例 |

---

### plugins/dev-team/bin/src/hooks.ts -> plugins/dev-team/bin/src/hooks.test.ts

#### 待测功能

- main(): 子命令分发新增 `sweep-phase` 分支
- 导出面：`export { runProtectFiles, runRecordFiles, runSweepPhase, runStaticCheck }`（增 runSweepPhase）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| main 分发 | 正向 | argv[2]='sweep-phase' → runSweepPhase 被调用（AC-10 分发接线） | 新增 |
| main 分发 | 边界 | protect-files / record-files / static-check 分发既有断言回归；未知子命令 → stderr + exit 1 | 新增（迁移保留） |
| 导出面 | 边界 | hooks 模块导出含 runSweepPhase 且为 function（knip entry-export 面回归） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 子命令模块 | vi.mock `./commands/sweep-phase` 等为 spy，断言分发 | main 分发用例 |
| process.argv / process.exit | 赋值注入与 spyOn 拦截（沿用既有 hooks.test.ts 模式） | main 分发用例 |

---

### plugins/dev-team/bin/src/mcp.ts -> plugins/dev-team/bin/src/mcp.test.ts

#### 待测功能

- connectToServer(transport): 注册 `phase_start` 工具（withResolvedProjectRoot 注入，zod 输入校验，委托 runPhaseStart）；刷新 change_files / workflow_files / test_resolve_paths / archi_check 描述措辞（files → file_log 派生净状态）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| phase_start 注册 | 正向 | tools/list 含 phase_start；inputSchema 与 phaseStartInputSchema 形状一致（change 必填、phase 9 值枚举、project_root 可选）（AC-2） | 新增 |
| phase_start 注册 | 正向 | 合法调用经 InMemoryTransport 返回 `{started: true, phase, attempt, start_at}`；workflow.json 落盘 active_phase | 新增 |
| phase_start 注册 | 异常 | 非法 phase / change 不存在 → 结构化错误响应，workflow.json 不变 | 新增 |
| 注册顺序 | 边界 | registerTool spy 断言：调用次数 16 → 17，REGISTER_ORDER 增 phase_start（AC-2） | 新增（修订既有断言） |
| 描述措辞 | 边界 | workflow_files / change_files / test_resolve_paths / archi_check 描述含 file_log 派生净状态语义、不含 git diff 圈定（既有断言面更新，AC-7） | 新增（修订既有断言） |
| 注册顺序 | 边界 | 旧「调用次数恰好 16」与不含 phase_start 的 REGISTER_ORDER 断言 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| project-root 解析 | 沿用既有 mockResolve（setResolvedRoot / withResolvedProjectRoot 注入） | 全部用例 |
| MCP 传输 | InMemoryTransport.createLinkedPair + Client（既有模式） | 注册与调用用例 |
| 文件系统 | mkdtempSync 临时项目真盘（change_create fixture） | phase_start 调用用例 |

---

### plugins/dev-team/build/hooks-profile.ts -> plugins/dev-team/build/hooks-profile.test.ts

#### 待测功能

- buildHooksFile(canonical, env): canonical schema 增 `userPromptSubmit` 键（default []）；claude 包装产出 `UserPromptSubmit`（无 matcher 字段）；cursor 包装不产出该事件（matcher null 降级）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| hooksCanonicalSchema | 边界 | canonical 缺省 userPromptSubmit → default []，两平台包装均不产出 UserPromptSubmit 键（AC-10） | 新增 |
| buildClaudeNested | 正向 | userPromptSubmit 条目（matcher.claude 为存在标志）→ 产出 `UserPromptSubmit: [{hooks: [{type:'command', command: sweep-phase 模板}]}]`，条目无 matcher 字段（AC-10） | 新增 |
| buildCursorNative | 正向 | userPromptSubmit 条目 matcher.cursor=null → 产出不含 userPromptSubmit 键（cursor 降级，AC-10） | 新增 |
| hooksCanonicalSchema | 异常 | userPromptSubmit 条目缺 commandTemplate → canonical schema 校验失败、构建期报错（自研 canonical schema 抛错面，与集成章节「userPromptSubmit 双平台产出差异」的 schema 违例场景同源） | 新增 |
| hooksCanonicalSchema | 异常 | userPromptSubmit 键非数组 / 条目类型非法 → schema 校验拒绝（`Array<{ matchers, commandTemplate }>` 类型契约的拒绝面，default [] 仅对键缺省生效、不吞类型违例） | 新增 |
| postToolUse（matcher 扩展） | 正向 | canonical postToolUse matchers 追加 `__MCP:phase_start__` → claude 与 cursor 包装均包含该 token（AC-9） | 新增 |
| buildHooksFile | 边界 | 既有 preToolUse / postToolUse / subagentStop 包装与 token 展开断言回归不动 | 新增（迁移保留） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 纯函数 + canonical 对象字面量（getEnv 提供双平台 env，既有模式） | 全部用例 |

<!-- 双平台产物级对比（canonical JSON ↔ buildHooksFile 产出逐项断言）落在集成章节
     `hooks.canonical.json → 双平台包装产物`（build/__tests__/hooks-user-prompt-submit/）。 -->

---

### plugins/dev-team/skills/phase-*/SKILL.md、skills/workflow-*/SKILL.md -> plugins/dev-team/bin/__tests__/skill-phase-start-protocol/skill-phase-start-protocol.test.ts

#### 待测功能

<!-- 文本断言测试（无导出函数）：读取 10 个 SKILL.md 源文本，核验技能协议插入点。 -->

- 8 个 `skills/phase-*/SKILL.md`: gate `phase_next` 返回本 phase 后、executor/evaluator 执行前含 `__MCP:phase_start__` 调用协议；retry 重跑与 backtrack recall 重进执行前重新调用；Verdict `phase_next` 不触发
- `skills/workflow-requirement/SKILL.md`、`skills/workflow-test-only/SKILL.md`: LOOP 内 gate 通过后、executor 执行前含 phase_start（每轮迭代一次；done/error 不调用）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| phase 技能协议 | 正向 | 每个 phase 技能文本匹配 `phase_next` → `__MCP:phase_start__({change, phase: …})` 序列（AC-11） | 新增 |
| phase 技能协议 | 边界 | retry / backtrack recall 重跑语境的「重新调用」文本存在；Verdict 段落不新增 phase_start 调用 | 新增 |
| workflow 编排技能 | 正向 | workflow-requirement / workflow-test-only 的 LOOP 内含 phase_start 且注明 done/error 不调用（AC-11） | 新增 |
| 技能协议 | 异常 | 文本缺失（技能漏改）→ 断言失败并列出缺失文件名 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 无 | 真实 fs 读取仓库内 SKILL.md 源文本（参照 eval-store-skill-copy.test.ts 的 readFileSync + 断言模式） | 全部用例 |

---

## 集成测试

<!-- 集成测试验证跨模块交互。单元测试通过 Mock 已覆盖模块依赖，此处聚焦模块组合时才暴露的行为。
     集成测试文件放置于测试区域 `bin/__tests__/`（及 build 的 `build/__tests__/`）目录下。 -->

### MCP门控事件序列 → session绑定/门②③ → file_log归账与派生净状态 → `plugins/dev-team/bin/__tests__/file-inventory-recording/file-inventory-recording.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/bin/src/commands/change-create.ts` | fixture 创建方（workflow.json） |
| `plugins/dev-team/bin/src/commands/record-files.ts` | 触发方/门控（事件解析、绑定刷新、门①②③判定、scope 解析） |
| `plugins/dev-team/bin/src/lib/session-registry.ts` | 中间件（`session_id → change` 绑定注册表，落盘隔离 tmpdir） |
| `plugins/dev-team/bin/src/modules/workflow/phase/phase-state.ts` | 门②状态源（active_phase 读写清） |
| `plugins/dev-team/bin/src/modules/workflow/files/record.ts` + `file-inventory.ts` | 写入方（file_log 覆盖/追加落盘） |
| `plugins/dev-team/bin/src/modules/workflow/files/files-query.ts` | 读取方（派生净状态投影） |
| `plugins/dev-team/bin/src/hooks.ts` | 入口分发（runRecordFiles） |

**关联AC**: AC-4, AC-5, AC-6, AC-9

**关系描述**:

这条链路是本变更的核心行为闭环：技能 turn 产生的 `phase_next` / `phase_start` MCP 调用事件经 PostToolUse hook 建立/刷新 session 绑定（门①数据源），`phase_start` 同时落盘 `active_phase`（门②开关）；随后同 session 的文件写事件按门①→门②→门③顺序判定归属，解析为 phase 或 workflow scope 写入 `file_log`；`phase_log` 清场关闭门②后，完成 turn 的 Verdict `phase_next` 仍会刷新绑定——若门控实现有漏洞（如 A2 方案的绑定复活），重绑会系统性击穿清除，这正是 AC-5 回归的存在理由。模块组合才暴露的出错模式包括：绑定已建立但 active_phase 尚未写入（门②误关/误开）、agent_type 平台前缀与 phase 表 token 形态不一致（门③误降级 workflow）、清场与写事件竞态（归账复活）、以及写方 log 与读方派生视图的语义漂移（AC-6 等价性）。三个模块全为真实实现，仅 mock 进程边界（stdin、tmpdir、getProjectDir），事件以 JSON 队列按序注入。

#### 场景: 门控事件序列回放（phase_next 绑定 → phase_start 开门 → 归账 → phase_log 关门）

phase_next 事件建立绑定（此刻无 active_phase，写事件应被门②丢弃并出诊断）；phase_start 事件刷新绑定并开门；executor 写事件（agent_type 归一匹配）以 phase scope 落入 file_log（scope=phase id、attempt=active_phase.attempt）；主 agent 写事件（无 agent_type）以 workflow scope 落入；phase_log 清场后同 session 再写被门②丢弃。前置条件：change_create 建立 change 且已 phase_start。预期：file_log 条目 scope/attempt/at 正确，门②关闭后的写事件不落 log 且 stderr 含诊断、exit 0。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | phase_next(绑定) → phase_start(开门) → executor 写 → 主 agent 写 → log 含 phase 条目与 workflow 条目各一条，workflow_type/created/未知键保留 | 新增 |
| 正向 | phase_log 清场后同 session 再写 → log 不增长，stderr 含门②诊断，exit 0 | 新增 |
| 异常 | 绑定后、phase_start 前的写事件 → 门②丢弃 + 诊断（前置顺序敏感） | 新增 |
| 边界 | executor: null 的 phase（如 code-review）开门后写事件 → workflow scope（门③降级不丢事件） | 新增 |
| 边界 | agent_type 为 `dev-team_<id>`（cursor-home 前缀）与 `dev-team:<id>`（claude 前缀）分别匹配同一 token → 均 phase scope | 新增 |
| 废弃 | 旧「phase_next 事件 → 直接归账（无门控）」序列回放用例与「source 条目来源审计（agent_type）」场景组：门控与 scope 取代 | 废弃 |

#### 场景: 重绑竞态回归（AC-5）

验证最终 pass 的 phase_log 清场之后，Verdict 的 `phase_next`（done:true）事件仅刷新绑定，不复活归账。前置条件：完整走完绑定 → 开门 → 写入 → phase_log(pass) 落盘清场。输入：再注入一条 phase_next 事件（模拟 Verdict 调用），随后注入写事件。预期：绑定注册表确已刷新（可经 lookupChange 路径验证后续事件不再走「未绑定」分支），但写事件仍被门②丢弃、file_log 逐字节不变、无异常退出。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 最终 pass 后 phase_next 事件 → 绑定刷新；后续写事件被门②丢弃，归账不复活（AC-5） | 新增 |
| 边界 | done 后写事件 + 无诊断噪音（门②丢弃走 stderr 诊断路径，非未绑定路径）——文案区分断言 | 新增 |

#### 场景: done 后 backtrack → phase_start 重开自愈

验证错误终态与回溯后的门控自愈。前置条件：工作流 done（全 pass）且门②关闭。输入：backtrack 目标 phase 后，新 turn 重新 phase_start。预期：active_phase 重新写入，同 session 写事件恢复归账为该 phase scope。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | done 后 backtrack → 重新 phase_start → 写事件恢复归账（scope=目标 phase） | 新增 |
| 边界 | 错误终态 turn（未 phase_start，如 done/error 间隙）写事件 → 门②丢弃不开门 | 新增 |

#### 场景: 写方与读方派生等价（跨模块净状态一致性）

验证记录器写入的 file_log 经 files-query 派生读取与写入意图一致。前置条件：同 change 已注入含 scope 翻转的事件序列（phase 记录后同 path 主 agent 重写）。预期：getChangedFiles 返回派生净状态，该 path 归 workflow 后条胜，无审计明细键泄漏。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | phase 写 + workflow 重写同 path → 派生 written 含该 path 且无 scope/attempt 泄漏（AC-6 跨模块面） | 新增 |
| 边界 | revert 事件（git restore）→ 派生双桶均不含该 path | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| stdin（readFileSync fd 0） | vi.mock `node:fs`/`fs` 仅替换 readFileSync，fd 0 按队列弹出；真盘读写不受影响 | 全部场景 |
| os.tmpdir | 重定向隔离目录承载 session 注册表 | 全部场景 |
| getProjectDir | vi.mock 指向临时项目根 | 全部场景 |
| process.exit / stderr / stdout | vi.spyOn 拦截与捕获（hooks.ts 顶层 main 副作用防护沿用既有动态 import 模式） | 诊断与 exit 断言 |

---

### phase_start → active_phase 运行态 → phase_log 盖章清场 / sweep 中断归档 → `plugins/dev-team/bin/__tests__/phase-lifecycle/phase-lifecycle.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/bin/src/commands/phase-start.ts` | 触发方（校验 + attempt 推导 + 写运行态） |
| `plugins/dev-team/bin/src/modules/workflow/phase/phase-state.ts` | 状态层（active_phase / interrupted 读写清归档） |
| `plugins/dev-team/bin/src/lib/eval-json.ts` | 条目组装（buildEntry 携带 start_at、appendEntry 落盘、computeAttempt） |
| `plugins/dev-team/bin/src/commands/phase-log.ts` | 盖章清场方（落盘前盖章、成功后清场） |
| `plugins/dev-team/bin/src/commands/sweep-phase.ts` | 中断收口方（UserPromptSubmit sweep） |
| `plugins/dev-team/bin/src/lib/session-registry.ts` | sweep 绑定查询（lookupChange） |

**关联AC**: AC-2, AC-3, AC-10（并覆盖 AC-1 的 start_at 落盘面）

**关系描述**:

phase 生命周期横跨三个命令与一个状态层：`phase_start` 写运行态、`phase_log` 消费运行态（盖章 start_at 使 per-attempt 耗时 = timestamp − start_at 可计算）并清场、`sweep-phase` 在用户中断后把遗留运行态归档进 `interrupted[]`。组合才暴露的出错模式：attempt 推导与 active_phase.attempt 漂移（retry 计时串档）、清场时序错误（先清后写会在 appendEntry 失败时白丢运行态；先写后清失败会双录）、sweep 与 phase_log 对同一遗留态的归档竞争（重复 interrupted 条目或误写 eval）。全链路真盘读写，仅 mock stdin 与进程边界。

#### 场景: 完整 turn 生命周期（开启 → 盖章 → 清场）

前置条件：change_create 建立的 change、eval 含 1 条该 phase 的 fail 条目（retry 场景）。输入：phase_start(implement) → phase_log(implement, pass)。预期：第一次调用落盘 active_phase{phase:'implement', attempt:2, start_at}；phase_log 落盘条目含 start_at === active_phase.start_at，且 active_phase 被清空；eval 条目 timestamp 与 start_at 均为 ISO，耗时可得。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | phase_start → phase_log 全链路：条目盖 start_at、active_phase 清空、attempt=2（AC-2/AC-3 跨模块面） | 新增 |
| 正向 | 无 phase_start 直接 phase_log → 条目无 start_at、active_phase 不存在不报错 | 新增 |
| 异常 | phase_start 传非法 phase → 报错且 workflow.json 逐字节不变，后续 phase_log 正常（无残留半态） | 新增 |
| 边界 | retry 序列：fail 落盘 → 重新 phase_start → attempt 递增且 start_at 刷新（独立计时） | 新增 |

#### 场景: sweep 中断归档与竞争防护

前置条件：绑定 session 且 change 有遗留 active_phase（模拟用户中断后残留）。输入：注入 stdin(session_id) 调用 runSweepPhase；随后正常 phase_log。预期：sweep 将遗留态移入 interrupted[]（含 end_at）并清空、eval 数组不变；之后的 phase_log 条目无 start_at（门已关）；连续 sweep 第二次为 no-op。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 遗留 active_phase → interrupted[] 追加一条 {phase, attempt, start_at, end_at}、active_phase 清空、eval 不变（AC-10） | 新增 |
| 正向 | sweep 后 phase_log → 条目无 start_at（中断后不误盖章） | 新增 |
| 边界 | 无遗留 / 未绑定 / 重复 sweep → no-op 不增长 interrupted | 新增 |
| 异常 | workflow.json 缺失 → stderr + exit 0 不阻塞 | 新增 |

#### 场景: 清场失败吞错与无双录

前置条件：注入 clearActivePhase 失败（如以只读方式锁文件不可行，则以 vi.mock phase-state 层注入抛错——跨进程边界外的内部故障注入，仅此场景使用）。输入：active_phase 匹配 + appendEntry 成功 + 清场抛错。预期：进程不抛错、条目已落盘、返回 `{written, phase, attempt}`；遗留 active_phase 由下次 phase_start last-wins 覆盖可回收。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 异常 | 清场抛错 → stderr 诊断、条目落盘、输出形状不变（AC-3 吞错收尾） | 新增 |
| 边界 | 清场失败后重新 phase_start → last-wins 覆盖遗留态，无双录条目 | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| stdin（readFileSync fd 0） | vi.mock 仅替换 readFileSync（sweep 场景注入 session_id） | sweep 场景 |
| os.tmpdir | 重定向隔离注册表目录 | sweep 绑定用例 |
| phase-state 层（仅清场故障注入） | vi.mock `modules/workflow` 的 clearActivePhase 为一次性抛错 | 清场失败吞错场景 |
| 系统时钟 | vi.setSystemTime 固定 start_at/end_at 断言 | 生命周期计时断言 |

---

### hooks.canonical.json → 双平台包装产物 → `plugins/dev-team/build/__tests__/hooks-user-prompt-submit/hooks-user-prompt-submit.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/hooks/hooks.canonical.json` | 配置源（postToolUse matchers 增 `__MCP:phase_start__`、新增 userPromptSubmit 事件项） |
| `plugins/dev-team/build/hooks-profile.ts` | 包装器（canonical schema 校验 + 双平台形状包装） |
| `plugins/dev-team/build/apply-env-tokens.ts` | 中间件（commandTemplate 的 env token 展开） |
| `plugins/dev-team/build/env.ts` | 平台环境（getEnv 提供 claude / cursor 两种 ProductEnv） |

**关联AC**: AC-9, AC-10

**关系描述**:

canonical hooks 清单是单一事实源，双平台产物由 `buildHooksFile` 包装生成。本变更改动两处清单语义：postToolUse matchers 追加 `__MCP:phase_start__` token（phase_start 调用事件必须触发 record-files 刷新绑定，漏配则 AC-9 的绑定自愈失效），以及新增 `userPromptSubmit` 事件（claude 需产出无 matcher 的 `UserPromptSubmit` 条目指向 sweep-phase，cursor 因 matcher 为 null 必须不产出——配反则 cursor 端 hooks.json 非法或 claude 端 sweep 不生效）。组合才暴露的出错模式：schema default 缺失导致旧 canonical 解析失败、claude 包装误带 matcher 字段、cursor 降级判断遗漏。测试读取真实 canonical JSON 与 `buildHooksFile` 产出对比（参照 `hooks-post-tool-use-registry` 模式），不依赖生成产物目录。

#### 场景: userPromptSubmit 双平台产出差异

前置条件：canonical JSON 含 userPromptSubmit 条目（`matchers: {claude: "*", cursor: null}`、commandTemplate 指向 sweep-phase）。输入：分别以 claude 与 cursor env 调用 buildHooksFile。预期：claude 产出 `hooks.UserPromptSubmit` 数组、条目无 matcher 字段、command 经 token 展开指向 `sweep-phase`；cursor 产出不含 `userPromptSubmit` 键。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | claude env → UserPromptSubmit 事件注册且无 matcher 字段、指向 sweep-phase 子命令（AC-10） | 新增 |
| 正向 | cursor env → 产物不含 userPromptSubmit 键（AC-10 降级） | 新增 |
| 边界 | canonical 删除 userPromptSubmit 键 → schema default [] 兜底，两平台均不产出且不抛错 | 新增 |
| 异常 | userPromptSubmit 条目缺 commandTemplate → schema 校验失败（构建期报错） | 新增 |

#### 场景: postToolUse matcher 扩展贯通双平台

前置条件：真实 `hooks.canonical.json`。输入：双平台包装。预期：两平台 postToolUse matchers 均包含 `__MCP:phase_start__` 与既有 `__MCP:phase_next__`，record-files command 不变。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | claude 与 cursor 的 postToolUse matcher 均含 `__MCP:phase_start__`（AC-9） | 新增 |
| 边界 | 既有 Write/Edit/Bash 等 matcher 项逐字保留（回归） | 新增（迁移保留） |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| 无 | 真实 fs 读取 hooks.canonical.json + getEnv 双平台 env 纯函数包装（既有 build/__tests__ 模式） | 全部场景 |

---

## 不可测试项

<!-- 列出 proposal 范围内但无法通过自动化测试验证的条目，每项说明原因。 -->

- `plugins/dev-team/bin/src/schemas/workflow.schema.ts`、`plugins/dev-team/bin/src/schemas/phase-start.schema.ts` 的独立单测 — **原因**: test_resolve_paths 返回 errors「Not in test config scope」，schemas 目录不在测试配置 scope 内；schema 行为（AC-1 的 active_phase/interrupted/start_at 接受性、未知键保留、AC-2 的输入校验）经消费方测试覆盖（doc-io / phase-state / eval-json / phase-start / mcp 的 inputSchema 形状断言）。
- `plugins/dev-team/package.json` 版本 bump 与 `pnpm -C plugins/dev-team run build` 双平台产物重建（claude-plugins/ / cursor-plugins/ / cursor-home-image/ 刷新） — **原因**: 构建流程动作而非可断言代码行为；产物目录为生成物，不在变更清单，仓库惯例不对其落自动化断言。
- Cursor 真实运行时无 UserPromptSubmit 事件的降级窗口（中断后 active_phase 遗留、门常开至下一条消息收口） — **原因**: 平台运行时行为，自动化只能断言「cursor 包装不产出该事件」（已在 build 测试覆盖）；「门常开窗口内的实际归账表现」依赖真实 Cursor 宿主，进程内不可复现（design 已明确接受该降级）。
- 多 session 并发绑同一 change 的真并发 last-wins 互踩 — **原因**: design 记为已知限制并接受（实际单人使用）；单线程 vitest 只能覆盖顺序化 last-wins（已在 phase-start 用例覆盖），真并发时序不可在进程内确定性构造。
- Stop 钩子在中断与 API 错误时不触发等平台查证结论（proposal 选型依据） — **原因**: 平台行为事实，非本仓库代码行为，无法也不需要测试；其推论（必须由 UserPromptSubmit sweep 收口）已由 sweep 集成场景覆盖。
- `phase_start` 输出 start_at 与真实墙钟的偏差、per-attempt 耗时数值正确性 — **原因**: 耗时由 timestamp − start_at 在消费侧计算，测试仅断言两 ISO 字段并存且 start_at 来自 active_phase（数据前提），不验证真实时间流逝。
