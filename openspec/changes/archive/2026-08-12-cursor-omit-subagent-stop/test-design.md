# 测试设计: cursor-omit-subagent-stop

> **日期**: 2026-08-12

---

## 验收范围

| AC ID | 验收条件 | 测试类型 | 被测文件或模块 |
|--------|---------|---------|----------|
| AC-1 | 构建后 `cursor-plugins/.../hooks.json` 与 `cursor-home-image/.../hooks.json` 的 `hooks` 下无 `subagentStop` 键（亦非空数组） | 集成测试 | `hooks.canonical.json` → `hooks-profile` → Cursor hooks 产物 |
| AC-2 | `claude-plugins/.../hooks/hooks.json` 仍含 `SubagentStop`，matcher 覆盖 implementation-generator / test-gen-generator，command 含 `static-check`，`loop_limit` 语义不变 | 集成测试 | `hooks.canonical.json` → `hooks-profile` → Claude hooks 产物 |
| AC-3 | `hooks.canonical.json` 中两条 `subagentStop` 的 `matchers.cursor` 为 `null` | 单元测试 / 集成测试 | `plugins/dev-team/build/hooks-profile.ts`；canonical → hooks 产物关系 |
| AC-4 | 存在 `.cursor.md` 时优先于 `.md`；仅空 default 时 Claude 嵌入空串；两边皆无则构建失败 | 单元测试 | `plugins/dev-team/build/expand-includes.ts` |
| AC-5 | assemble：`expandIncludes` → `applyEnvTokens` → `assertNoNameTokens`；环与可选深度上限触发构建错误；残留 `__INCLUDE:` 失败 | 单元测试 / 集成测试 | `expand-includes.ts`、`assert-no-tokens.ts`、`assemble.ts`；管道关系 |
| AC-6 | Cursor/cursorHome 组装后两 generator 正文含 `run_static_analysis` 门禁步骤；Claude 组装后对应位置无该步骤（空 include） | 集成测试 | `_fragments` + generator agents → assemble outDir |
| AC-7 | 任意 outDir 中不存在 `_fragments/` 目录拷贝 | 集成测试 | `_fragments` + generator agents → assemble outDir |
| AC-8 | hooks-profile 与 include 相关单测通过 | 单元测试 | `hooks-profile.test.ts`、`expand-includes.test.ts`、`assert-no-tokens.test.ts`、`assemble.test.ts` |

---

## 单元测试

### plugins/dev-team/build/expand-includes.ts -> plugins/dev-team/build/expand-includes.test.ts

#### 待测功能

- `expandIncludes(text, env, stack?, depth?)`（导出）：递归替换合法 `__INCLUDE:<id>__`；环或超深度上限抛错；嵌入体 `trim` 后 splice
- 平台 resolve（模块内 `resolveFragment`，不导出）：经 `expandIncludes` 间接覆盖——`<id>.<agent>.md` → `<id>.md`；皆无则抛错；断言嵌入正文或错误 message，不直测路径返回值

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| expandIncludes | 正向 | 文本含单个 `__INCLUDE:static-analysis-gate__`，Cursor env 展开为 `.cursor.md` 正文（含 `run_static_analysis`） | 新增 |
| expandIncludes | 正向 | 同时存在平台档与 default 且正文不同时，Cursor 嵌入平台档正文（非 default） | 新增 |
| expandIncludes | 正向 | 仅存在 `.cursor.md` 时 `env.agent=cursor` 嵌入平台档正文 | 新增 |
| expandIncludes | 正向 | Claude env + 空 default 文件时该 token 位置替换为空串（宿主前后文保留） | 新增 |
| expandIncludes | 正向 | 嵌套 include（A 引用 B）按深度递归展开且最终无残留 `__INCLUDE:` | 新增 |
| expandIncludes | 正向 | 展开结果内含 `__BIN:cli__` 等 env token 时原样保留，供后续 `applyEnvTokens` 处理 | 新增 |
| expandIncludes | 异常 | fragment 正文互相引用形成环时抛错 | 新增 |
| expandIncludes | 异常 | 引用不存在的 id 时抛错且 message 含缺失 fragment id | 新增 |
| expandIncludes | 异常 | `text` 为 `null` / `undefined` 时抛错 | 新增 |
| expandIncludes | 异常 | `env` 为 `null` / `undefined` 时抛错 | 新增 |
| expandIncludes | 异常 | `env={}`（空对象，缺失必填字段）时抛错 | 新增 |
| expandIncludes | 异常 | 嵌套深度超过软上限 `16` 时抛错 | 新增 |
| expandIncludes | 边界 | `text` 为空字符串时返回空字符串 | 新增 |
| expandIncludes | 边界 | `text` 无 include token 时原样返回 | 新增 |
| expandIncludes | 边界 | 超长文本（>1000 chars）含多枚合法 include 时全部展开且不崩溃 | 新增 |
| expandIncludes | 边界 | 嵌入体前后空白经 `trim` 后 splice；宿主全文本身不被 trim | 新增 |
| expandIncludes | 边界 | `stack=[]`（空列表）时从空链正常展开 | 新增 |
| expandIncludes | 边界 | `stack` 为单元素且不含当前 id 时正常展开 | 新增 |
| expandIncludes | 边界 | `stack` 已含当前 id（环）时抛错 | 新增 |
| expandIncludes | 边界 | `stack` 为超大列表（长度 ≫16，如 1000 个互异 id）且不含当前 id 时仍可展开或仅因 depth 规则失败，不因数组体积崩溃 | 新增 |
| expandIncludes | 边界 | `stack=null` / `stack=undefined` 时按实现约定回退默认空栈或抛错（单测固定一种行为） | 新增 |
| expandIncludes | 边界 | `depth=0` 时可展开（与默认起点一致） | 新增 |
| expandIncludes | 边界 | `depth=16`（恰达软上限）时可展开成功 | 新增 |
| expandIncludes | 边界 | `depth=17`（超过软上限）时抛错；message 含数值 `16`（不依赖导出常量） | 新增 |
| expandIncludes | 边界 | `depth=-1` 时按实现约定抛错或视为非法深度（单测固定一种行为） | 新增 |
| expandIncludes | 边界 | `depth=Number.MAX_SAFE_INTEGER`（MAX_INT 等价极端）时立即因超过软上限抛错，不崩溃 | 新增 |
| expandIncludes | 边界 | `depth=undefined` 等同从 0 起算并可展开 | 新增 |
| expandIncludes | 边界 | `depth=null` 时按实现约定回退默认或抛错（单测固定一种行为） | 新增 |
| expandIncludes | 边界 | `env` 含多余字段时仍按 `agent` 优先平台档展开 | 新增 |
| expandIncludes | 边界 | include id 为超长合法串时按文件系统存在性判定（不崩溃） | 新增 |
| expandIncludes | 边界 | 非法字面量如 `__INCLUDE:Foo__` / `__INCLUDE:a/b__` 不被合法正则替换，原文保留（交由 assert 失败） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| `_fragments/` 文件系统 | 使用 `mkdtempSync` 临时目录 + 写入平台档/default/环链文件；通过 stub `FRAGMENTS_ROOT` 或 chdir/注入根路径指向临时目录（与实现暴露方式一致） | expandIncludes 全部用例（含平台 resolve 间接覆盖） |
| `ProductEnv` | 调用 `getEnv('claude'\|'cursor'\|'cursorHome')` 真实 env；不 mock | 平台优先与空 default |

---

### plugins/dev-team/build/hooks-profile.ts -> plugins/dev-team/build/hooks-profile.test.ts

#### 待测功能

- `buildHooksFile(canonical, env)`: 将 canonical hooks 按平台组装为 Claude nested 或 Cursor native JSON 字符串；Cursor 过滤 null matcher 后省略空事件键；Claude 保留 `SubagentStop`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| buildHooksFile | 正向 | fixture 两条 `subagentStop.matchers.cursor=null` 时，`getEnv('cursor')` 产物 `hooks` 无 `subagentStop` 键（`Object.hasOwn` 为 false，且不是 `[]`） | 新增 |
| buildHooksFile | 正向 | 同上对 `getEnv('cursorHome')` 亦省略 `subagentStop` 整键 | 新增 |
| buildHooksFile | 正向 | fixture 保留非 null claude matcher 时，`getEnv('claude')` 产物含 `hooks.SubagentStop`，长度 2，matcher 覆盖两 generator，command 含 `static-check`，条目含 `loop_limit: 5`（若 schema/输出保留该字段）或嵌套 hooks 命令语义不变 | 新增 |
| buildHooksFile | 正向 | 读取真实 `hooks.canonical.json`：cursor/cursorHome 无 `subagentStop`；claude 仍有 `SubagentStop` | 新增 |
| buildHooksFile | 正向 | `preToolUse` 仍按非 null cursor matcher 发射；仅 `subagentStop` 因全 null 被省略 | 新增 |
| buildHooksFile | 正向 | claude 输出可 JSON.parse，含嵌套 `hooks.PreToolUse` / `SubagentStop` | 新增 |
| buildHooksFile | 异常 | canonical 缺必填字段 / 非法类型时 zod parse 抛错 | 新增 |
| buildHooksFile | 异常 | `canonical` / `env` 为 `null` / `undefined` 时抛错 | 新增 |
| buildHooksFile | 异常 | `canonical={}`（空对象，缺失 `preToolUse` / `subagentStop` 等必填字段）时 zod parse 抛错 | 新增 |
| buildHooksFile | 异常 | `env={}`（空对象，缺失 `agent` 等必填字段）时抛错 | 新增 |
| buildHooksFile | 边界 | `subagentStop=[]` 时 Cursor 省略键；Claude 发射空数组或省略（固定实现行为） | 新增 |
| buildHooksFile | 边界 | `subagentStop` 仅一条 cursor matcher 非 null 时 Cursor 仍写出该键且长度为 1 | 新增 |
| buildHooksFile | 边界 | `preToolUse` 全 null cursor matcher 时 Cursor 亦省略 `preToolUse` 键（通用空键省略） | 新增 |
| buildHooksFile | 边界 | `loop_limit` 为 `0` / `-1` / 省略时 Claude 路径不崩溃且序列化合法 | 新增 |
| buildHooksFile | 边界 | `canonical` 在合法 fixture 基础上含多余顶层字段时 zod 按 schema 剥离或忽略多余字段，仍产出合法 hooks JSON | 新增 |
| buildHooksFile | 边界 | `env` 在合法 `ProductEnv` 基础上含多余字段时仍按 `agent` 分形组装，不崩溃 | 新增 |
| buildHooksFile | 废弃 | Cursor 产物期望 `hooks.subagentStop` 已定义 / matcher 为 `implementation-generator` 的旧断言 | 废弃 |
| buildHooksFile | 废弃 | cursorHome fixture 期望 `subagentStop` 数组存在的旧断言 | 废弃 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| canonical fixture | 内存对象（含 `matchers.cursor: null` 的新 fixture）；可选 `readFileSync` 读真实 `hooks.canonical.json` | 正向 / 边界 / 真实文件用例 |
| `ProductEnv` | `getEnv` 真实返回值 | 全部 buildHooksFile 用例 |

---

### plugins/dev-team/build/assert-no-tokens.ts -> plugins/dev-team/build/assert-no-tokens.test.ts

#### 待测功能

- `assertNoNameTokens(rootDir, env)`: 扫描文本文件；名称类 token、路径 token、残留 `__INCLUDE:` 任一命中则抛错

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| assertNoNameTokens | 正向 | 临时目录仅含已展开文本（无名称 token、无 `__INCLUDE:`）时不抛错 | 新增 |
| assertNoNameTokens | 正向 | 已有：含 `__MCP:` 等名称类 token 时抛错 | 新增 |
| assertNoNameTokens | 异常 | 文件含合法残留 `__INCLUDE:static-analysis-gate__` 时抛错且 message 可定位文件 | 新增 |
| assertNoNameTokens | 异常 | 文件仅含子串 `__INCLUDE:`（非法/残缺 token）时亦抛错 | 新增 |
| assertNoNameTokens | 异常 | `rootDir` / `env` 为 `null` / `undefined` / `""` 时抛错 | 新增 |
| assertNoNameTokens | 异常 | `env={}`（空对象，缺失必填字段）时抛错 | 新增 |
| assertNoNameTokens | 边界 | 空目录不抛错 | 新增 |
| assertNoNameTokens | 边界 | 排除扩展名（`.js` map / 二进制）中含 `__INCLUDE:` 字节时不误报（与现有名称 token 扫描规则一致） | 新增 |
| assertNoNameTokens | 边界 | 超长文件（>1000 chars）末尾残留 `__INCLUDE:` 仍能检出 | 新增 |
| assertNoNameTokens | 边界 | 多文件仅一处残留时抛错列表包含该相对路径 | 新增 |
| assertNoNameTokens | 边界 | `env` 在合法 `ProductEnv` 基础上含多余字段时，无残留 token 的目录仍不抛错 | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 临时 outDir | `mkdtempSync` + `writeFileSync` 写入含/不含 `__INCLUDE:` 的文本 | 全部新增 `__INCLUDE` 相关用例 |
| `ProductEnv` | `getEnv(...)` | 与现有用例相同 |

---

### plugins/dev-team/build/assemble.ts -> plugins/dev-team/build/assemble.test.ts

#### 待测功能

- `assembleAll()`: 对全部 `PRODUCT_ENV_KEYS` 执行 assemble；文本管道顺序为 `expandIncludes` → `applyEnvTokens` → `assertNoNameTokens`；不拷贝 `_fragments/`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---------|----------|----------|----------|
| assembleAll | 正向 | 在可控 fixture 包根（含 agents / `_fragments` / canonical hooks / staging bin）上调用后，三平台 outDir 均生成且无残留 `__INCLUDE:` | 新增 |
| assembleAll | 正向 | Cursor/cursorHome outDir 的 hooks JSON 无 `subagentStop` 键 | 新增 |
| assembleAll | 正向 | Claude outDir hooks 含 `SubagentStop` 且 command 含 `static-check` | 新增 |
| assembleAll | 异常 | generator 引用缺失 fragment 时 assemble 失败（抛错或非零） | 新增 |
| assembleAll | 异常 | fragment 环引用时 assemble 失败 | 新增 |
| assembleAll | 边界 | `_fragments/` 存在于源码树但不出现在任一 `env.outDir` | 新增 |
| assembleAll | 边界 | Claude generator 产物中 include 位置无 `run_static_analysis`；Cursor 产物含该步骤 | 新增 |
| assembleAll | 边界 | `PRODUCT_ENV_KEYS` 单轮全量执行不因某一 env 失败而静默跳过（失败即抛） | 新增 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---------|----------|----------|
| 包根 / outDir | 最小 fixture 目录树或 chdir 到 `plugins/dev-team` 并重定向 `outDir`（若实现允许注入）；避免污染真实 `claude-plugins/` 等产物时可 spy `getEnv` 改写 `outDir` 到临时目录 | assembleAll 全部用例 |
| staging bin | 写入最小 `.pack-staging/bin` 占位文件以满足 copy | 正向 assemble |
| 文件系统 | 真实 `fs` 读写；不对 expandIncludes 内部再 mock | 管道顺序与产物断言 |

---

## 集成测试

### hooks.canonical.json → buildHooksFile → Cursor/Claude hooks 产物 → `plugins/dev-team/build/__tests__/hooks-platform-subagent-stop/hooks-platform-subagent-stop.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/hooks/hooks.canonical.json` | 源配置：Cursor matcher null、Claude matcher / loop_limit / static-check 保留 |
| `plugins/dev-team/build/hooks-profile.ts` | 组装方：按 `env.agent` 分形发射并省略空事件键 |
| `plugins/dev-team/build/env.ts` | 环境方：提供 claude / cursor / cursorHome 的 `ProductEnv` |

**关联AC**: AC-1, AC-2, AC-3

**关系描述**:

真实 canonical 经 `buildHooksFile` 与三平台 env 组合后，决定 Cursor 生态是否暴露 `subagentStop` 事件面，以及 Claude 硬门禁是否完整。该交互覆盖「源配置 null」与「组装省略整键」两段责任边界：仅改 canonical 而不改组装可能仍写出 `matcher: null`；仅改组装而不改 canonical 则无法从源上声明平台例外。集成测试用真实文件锁定 AC-1/2/3，避免 fixture 与仓库漂移。

#### 场景: 真实 canonical 三平台 hooks 形态

以仓库内 `hooks.canonical.json` 为输入，分别用 `getEnv('claude'|'cursor'|'cursorHome')` 调用 `buildHooksFile`，解析 JSON 后断言：两条 `subagentStop[].matchers.cursor === null`；cursor/cursorHome 的 `hooks` 对象无 `subagentStop` 键；claude 的 `hooks.SubagentStop` 覆盖两 generator matcher，command 含 `static-check`，`loop_limit` 语义保持（canonical 为 5，组装后不丢失硬门禁意图）。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 真实 canonical：`matchers.cursor` 两条均为 `null` | 新增 |
| 正向 | cursor / cursorHome：`hooks` 无 `subagentStop` 键 | 新增 |
| 正向 | claude：`SubagentStop` 含两 generator matcher 且 command 含 `static-check` | 新增 |
| 边界 | cursor 产物不得出现 `subagentStop: []` | 新增 |
| 边界 | claude `loop_limit` 在 canonical 为 5；若输出结构保留该字段则等于 5，否则 command 硬门禁条目数与 matcher 数仍为 2 | 新增 |

##### Mock策略

<!-- 无跨进程边界；读写本地 JSON 与纯函数组装，无需 Mock -->

### expandIncludes → applyEnvTokens → assertNoNameTokens → `plugins/dev-team/build/__tests__/include-assemble-pipeline/include-assemble-pipeline.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/build/expand-includes.ts` | 第一段：展开 `__INCLUDE:` 并 trim 嵌入体 |
| `plugins/dev-team/build/apply-env-tokens.ts` | 第二段：展开 `__BIN:` / 路径等 env token |
| `plugins/dev-team/build/assert-no-tokens.ts` | 第三段：残留 `__INCLUDE:` 与名称类 token 失败 |
| `plugins/dev-team/build/assemble.ts` | 编排方：保证 outDir 文本管道顺序 |

**关联AC**: AC-5

**关系描述**:

软门禁 fragment 同时包含 include 与 env token（如 `__BIN:cli__`）。若先跑 `applyEnvTokens` 再展开 include，平台档内的 token 可能漏替换或顺序颠倒；若展开后不做残留断言，非法 `__INCLUDE:` 会漏进产物。本关系在临时目录模拟 assemble 文本管道（或调用可观测的 assemble 路径），验证三段串联的成功路径与失败路径，捕捉单测隔离时不易发现的顺序错误。

#### 场景: 管道顺序与残留失败

前置：临时 `_fragments` 中 Cursor 平台档含 `run_static_analysis` 与 `__BIN:cli__`；宿主文本含 `__INCLUDE:static-analysis-gate__`。输入依次经 `expandIncludes` → `applyEnvTokens` → `assertNoNameTokens`。预期：最终文本含展开后的 bin 名与 `run_static_analysis`，无 `__INCLUDE:`；人为跳过 expand 或保留残留时 assert 抛错；环/超深在 expand 阶段失败且不进入 assert。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | 完整三段管道后无 `__INCLUDE:` 且 `__BIN:cli__` 已展开 | 新增 |
| 异常 | 跳过 expand 直接 assert 含 `__INCLUDE:` 的文本应抛错 | 新增 |
| 异常 | 环状 fragment 在 expand 阶段抛错 | 新增 |
| 边界 | 空 default include + 后续 env token 文本：Claude 嵌入空串且其余 token 仍被 apply | 新增 |
| 边界 | 深度恰好 16 成功、17 失败 | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| 临时目录树 | `mkdtemp` 写入宿主 md 与 `_fragments`；可选 spy `getEnv().outDir` | 全部场景 |
| 进程 cwd | 若 resolve 依赖包根相对路径，则 `process.chdir` 到 fixture 根并在 afterEach 恢复 | expand 依赖真实相对根时 |

### _fragments + generator agents → assemble outDir → `plugins/dev-team/build/__tests__/static-analysis-gate-assemble/static-analysis-gate-assemble.test.ts`

**涉及模块**:

| 模块 | 角色 |
|------|------|
| `plugins/dev-team/_fragments/static-analysis-gate.md` | Claude default：空内容 → 嵌入空串 |
| `plugins/dev-team/_fragments/static-analysis-gate.cursor.md` | Cursor 软门禁正文源 |
| `plugins/dev-team/agents/implementation-generator.md` | 引用方：Process 末尾 `__INCLUDE:static-analysis-gate__` |
| `plugins/dev-team/agents/test-gen-generator.md` | 引用方：同上 |
| `plugins/dev-team/build/assemble.ts` | 构建编排：copy agents、展开 include、写 outDir；排除 `_fragments` 拷贝 |

**关联AC**: AC-4, AC-6, AC-7

**关系描述**:

软门禁是否生效取决于 fragment 平台档、agent 源引用与 assemble 拷贝/展开策略三者同时正确。单元测试可分别验证 resolve 与 omit 拷贝逻辑，但只有集成装配才能证明 Cursor 产物 generator 正文出现 `run_static_analysis`、Claude 对应位置不出现该步骤，且 outDir 永不出现 `_fragments/`。出错模式包括：漏写 include 引用、误把 `_fragments` 加入 copy 清单、平台后缀解析反了导致 Claude 吃到 Cursor 正文。

#### 场景: 三平台 generator 软门禁与 _fragments 排除

前置：源码已含空 default、`.cursor.md` 软门禁与两 generator 的 include 引用。执行面向临时 outDir 的 assemble（或等价最小装配）。预期：cursor/cursorHome 的两 generator 产物含 `run_static_analysis` 与「未通过不得结束」类约束；claude 对应文件在 include 锚点处无该步骤；三个 outDir 均不存在 `_fragments` 目录。

##### 用例

| 路径类型 | 测试条件 | 迭代类型 |
|----------|----------|----------|
| 正向 | cursor / cursorHome：`implementation-generator` 与 `test-gen-generator` 产物含 `run_static_analysis` | 新增 |
| 正向 | claude：两 generator 产物不含 Cursor 软门禁步骤（空 include） | 新增 |
| 正向 | 任一 outDir 不存在 `_fragments/` 目录 | 新增 |
| 异常 | 删除平台档与 default 后 assemble 失败 | 新增 |
| 边界 | cursorHome 产物中 `__BIN:cli__` / `__DEV_TEAM_ROOT__` 已按 home env 展开 | 新增 |
| 边界 | 源码 `_fragments` 仍存在于包根，仅 outDir 排除 | 新增 |

##### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|----------|----------|----------|
| outDir | spy / 改写 `ProductEnv.outDir` 到临时目录，避免覆盖已发布产物树 | 全部场景 |
| staging 依赖 | 最小 stub `.pack-staging` 以满足 assemble 前置 | 正向装配 |

---

## 不可测试项

- Cursor 上游 `subagentStop` / `followup_message` 平台 bug 是否修复 — **原因**: 提案明确不修复上游；本变更仅省略事件键，无法在本仓库自动化验证 Cursor IDE 运行时行为
- 软门禁文案对模型的实际约束力（无 `loop_limit` 时模型是否遵守「不得结束」） — **原因**: 属模型行为/产品风险，需人工或另议 evaluator 抽查，非构建期可判定
- `openspec/todo.md` 勾选与 `package.json` version bump 本身 — **原因**: 属发布/流程产物，由实现任务与评审确认，不纳入自动化测试套件
- marketplace「cursor 走 nested」类 spec 文档漂移修正 — **原因**: 文档/spec 叙事变更，无运行时断言对象
