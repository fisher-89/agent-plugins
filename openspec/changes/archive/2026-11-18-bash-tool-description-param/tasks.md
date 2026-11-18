# 任务: bash-tool-description-param

> 覆盖 `proposal.md`「变更范围 - 实现文件」全部 4 项（3 处源码 + 1 行版本号）+ design 的 DD1–DD7 与「文案台账」逐字文案。
>
> **本相位不改测试文件的用例与断言，也不执行测试回归**：改造 / 新增用例（proposal「测试文件」表逐行）归 test-design / test-gen / test-execution 相位；`cargo test --manifest-path packages/desktop/src-tauri/Cargo.toml -p agent-runtime` 与 `pnpm -C packages/desktop exec vp test run src/components/agent/message-cards/tool-cards.test.tsx` 由桌面端 test-execution 相位承接。本相位对测试文件**零动作**——本变更不产生编译期破坏（`intent` 是 JSON 字面量字段、前端是可选呈现位），只是「必填 + 硬闸」使若干既有夹具在运行期不再合法，点位已登记在 design「影响传导」，由测试相位同步。
>
> **依赖序（四阶段）**：
>
> - **阶段一**（定义面 `mount.rs`）先做——它是模型可见契约的成形（`intent` 进 `required` + 取值纪律），阶段二的执行体闸才有「已声明」的字段可守。
> - **阶段二**（执行体闸 `bash.rs`）在阶段一之后——闸的插入点以阶段一后的 schema 形态为参照，且「schema 说必填、执行体真拒」是同批语义（分先后会出现「已要求但缺了不报」的空窗）。
> - **阶段三**（前端折叠态 `tool-cards.tsx`）与阶段一、二**零依赖**（前端只消费载荷原文，不读 schema、不读闸），但放其后以免在字段名未定稿时对着草稿写码。
> - **阶段四**（版本 + 边界自查 + 交接）依赖全部。
>
> **关键约束（贯穿全部任务）**：
> ① **文案单点**：`intent` 的四个取值要素（一句话说明这条命令要干什么 / 折叠态用户只读这一行 / 建议不超过 30 字 / 写目的而非复述命令文本）**只在** `intent.description` 出现一次；工具级 `description` 只追加「每次调用必须给 intent（缺失或空白即拒绝执行）」一句强制与后果；`bash.rs` 的两条 `Err` 只陈述缺什么、不含任何取值纪律措辞。
> ② **零第二真相**：`intent` 只是 `bash` 工具载荷内的一层 JSON 键——`crates/core/agent/**` 的事件词汇、`crates/infra/store/**` 的落库形状、`packages/desktop/src/types/generated/**` 的 IPC 类型 **MUST 零 diff**；出现 diff 即说明另起了结构化字段。
> ③ **判定输入不变**：`intent` MUST NOT 进执行（不进子进程命令行 / 不进 ToolResult / 不进日志）、MUST NOT 进写保护或路径沙箱判定链；`protect/**`、`sandbox.rs`、`tools.rs::input_path`、`context.rs` / `compact.rs` 出现 diff 即越界（design DD6）。
> ④ **前端零导出 / 零新变体**：`BashCard` 仍模块私有、零新 prop、零新导出；`trace.tsx` 零改动（不新增变体 / prop / 类名分支，不启用 `TraceDescription`）；截断只由既有 `TraceTitle` 的 `min-w-0 truncate` 承载（CSS 面），**MUST NOT** 在组件内 JS 侧截断。
> ⑤ **门禁分工**：静态脚本链（`pnpm run check` = `server:check` → `bindings:export` → `api:check` → `client:check`）与测试回归由桌面端 static-check / test-execution 相位执行，本相位不运行；自查项只收源码级静态判据（`grep` 命中数与零命中、读码复核、`git diff` 边界、版本行相对 `HEAD` +1 patch）。

## 阶段一：定义面（`mount.rs` 的 `bash` 定义）

- [x] `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs`：`generic_definition` 的 `"bash"` 臂（`:473-487`）在 `properties` 的 `command` 与 `timeout_ms` **之间**插入 `intent`——`"intent": { "type": "string", "description": "一句话说明这条命令要干什么——折叠状态下用户只读这一行（与 Bash 同排展示，超长自动省略）。建议不超过 30 字，写目的而非复述命令文本" }`（逐字照抄 design「文案台账」第 1 行；**MUST NOT** 加 `maxLength`、`pattern`、`minLength` 等任何长度约束键）
- [x] 同文件同臂：`required` 由 `json!(["command"])` 改为 `json!(["command", "intent"])`——顺序为 `command` 在前、`intent` 在后（与 spec「逐值含 `command` 与 `intent`」一致，`timeout_ms` 仍不入）
- [x] 同文件同臂：工具级 `description` 在**既有串末尾**追加 `；每次调用必须给 intent（缺失或空白即拒绝执行）`——既有三段（执行与合并口径 / cwd 与非零退出码 / 超时活性护栏）逐字不动，**MUST NOT** 把追加句插到前面（`tools_test.rs:1290` 以 `starts_with` 锚前缀 `执行 shell 命令并返回合并输出`）
- [x] 同文件逐字不动复核点：`read` / `grep` / `glob` / `ls` / `write` / `edit` 六臂的 `description`、`properties`、`required`；`TOOL_NAMES`（`:20`）七件与注册序；`definitions(...)`（`:166`）的注入面与 `None` 分支；`registry()` / `ToolTier` 表（`"bash" => ToolTier::Execute`）；`injected` / `injected_finish` 测试夹具；`unreachable!` 兜底臂——`git diff -- packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs` 的 hunk **只允许**出现在 `"bash"` 臂的 `description` / `properties` / `required` 三处
- [x] 自查 阶段一（静态判据）：`grep -c "折叠态" packages/desktop/src-tauri/crates/infra/agent/src/sdk/mount.rs` 为 **1**（唯一命中行 = `intent` 的 `description`，证明工具级描述未重复同一句）；`grep -c "30 字" …/mount.rs` 与 `grep -c "复述命令" …/mount.rs` 各为 **1**（同一行）；`grep -c "maxLength" …/mount.rs` 与改动前一致（零新增）；`grep -n "required" …/mount.rs` 复核 bash 臂为 `["command","intent"]`、其余六臂数组逐字不变；`cargo clippy --manifest-path packages/desktop/src-tauri/Cargo.toml -p agent-runtime --all-targets` 无新增生产面告警
- [ ] 交接 阶段一（测试相位）：`mount_test.rs` 新增「bash `required` 恰为 `["command","intent"]`、`properties.intent.type == "string"`、`properties.intent` 无 `maxLength`」断言（既有断言零改）；`tools_test.rs:1281` 快照行的 bash 项 `required` 由 `&["command"]` 改为 `&["command","intent"]`（`description_head` 前缀锚**零改**即可继续成立）——**由桌面端 test-design / test-gen / test-execution 相位承接**

## 阶段二：执行体闸（`bash.rs` 的 `execute`）

- [x] `packages/desktop/src-tauri/crates/infra/agent/src/sdk/bash.rs`：`execute` 在 `command.trim().is_empty()` 判空块（`:22-24`）**之后**、`let timeout = Duration::from_millis(resolve_timeout_ms(input)?)`（`:26`）**之前**插入意图闸——`let intent = input.get("intent").and_then(Value::as_str).ok_or_else(|| "入参缺失或非字符串字段: intent".to_owned())?;` 紧接 `if intent.trim().is_empty() { return Err("intent 不得为空".to_owned()); }`（两条文案逐字照抄 design「文案台账」第 4 / 5 行，与既有 `command` 闸同构成句）
- [x] 同文件：**MUST NOT** 新增长度分支（不看 `intent.len()`、不加 `maxLength` 对应校验）；**MUST NOT** 把 `intent` 写进 `build_command` 的参数、`Err` 文案的其它位置、`append_engine_log` 或任何 `format!` 输出——`intent` 除这两处校验外零消费（design DD4 ⑤）
- [x] 同文件逐字不动复核点：`command` 的两条既有 `Err`（`入参缺失或非字符串字段: command` / `command 不得为空`）、`DEFAULT_TIMEOUT_MS` / `MIN_TIMEOUT_MS` / `MAX_TIMEOUT_MS`、`resolve_timeout_ms`（`:81`）、`build_command`（`:100`）、`drain`（`:110`）、`merge_output`（`:117`）、双管道并发排水 + `child.wait()`、`tokio::time::timeout` 罩全段、`exec::kill_tree(pid)` 与超时文案（含 `{command}`）、非零退出码臂与合并输出口径——`git diff` 的 hunk **只允许**出现在 `execute` 前段新增的两条校验
- [x] 自查 阶段二（静态判据）：`grep -c "trim().is_empty()" …/bash.rs` 为 **2**（既有 `command` 一处 + 新增 `intent` 一处）；`grep -n "intent" …/bash.rs` 的命中**全部落在 `execute` 函数体内**（`resolve_timeout_ms` / `build_command` / `drain` / `merge_output` 零命中）；`grep -n "const intent\|let intent\|intent 不得为空\|字段: intent" …/bash.rs` 复核位点行号满足「`command` 判空行 < `intent` 闸行 < `resolve_timeout_ms(input)?` 行」（校验序：command 先报）；`grep -c "len()" …/bash.rs` 为 **0**（零新增长度分支，与改动前一致）；`cargo clippy --manifest-path packages/desktop/src-tauri/Cargo.toml -p agent-runtime --all-targets` 无新增告警
- [ ] 交接 阶段二（测试相位）：`bash_test.rs` 10 处既有 `execute` 夹具补 `intent`（断言逐条不变；`:118` / `:126` 的两条 `command` 断言零改即可继续命中）+ 新增意图闸三形态反向用例与 200 字软约束正向用例；`tools_test.rs` 两处执行夹具（`:973` 的 60KB 超限用例 / `:998` 的分发正向）补 `intent`、`:1003` 的 `{}` 缺参用例断言零改；`loop_test.rs` 五处走真实执行的 bash 调用（`:2739` / `:2942` / `:2995` / `:3033` / `:5651`）补 `intent`（拒绝臂 `:2974` / `:3345` / `:3350` 可选补齐）；`protect/**_test.rs` / `policy_test.rs` **零动作**（若必须改动即说明闸插错了面）——**由桌面端 test-design / test-gen / test-execution 相位承接**

## 阶段三：前端折叠态（`tool-cards.tsx` 的 `BashCard`）

- [x] `packages/desktop/src/components/agent/message-cards/tool-cards.tsx`：`BashCard` 内 `const command = textOf(asRecord(input).command);` 一行扩为两行——`const record = asRecord(input);` + `const command = textOf(record.command);`（`command` 读取语义与展开态使用点逐字不变）
- [x] 同组件：增 `const intent = textOf(record.intent)?.trim() ?? '';`——`''` 为**缺席哨兵**（`textOf` 已把非字符串归 `null`，`?.trim()` 吃掉纯空白串）；只 `trim` 首尾，**MUST NOT** 折叠内部空白 / 截断 / 合写大小写（design DD2）
- [x] 同组件：`<TraceTitle>Bash</TraceTitle>` 改为 `<TraceTitle>{intent === '' ? 'Bash' : `Bash · ${intent}`}</TraceTitle>`——分隔符为「空格 + `·` + 空格」三字符；**MUST NOT** 引入内层 `span`、分色类（`text-muted-foreground/70`）、`TraceDescription` 次级位、第二个文本承载节点（design DD1 单文本节点裁定）
- [x] 同组件：组件 doc 注释更新为「Bash 特化卡：命令作展开态标题栏、`intent` 作折叠态标题；输出折叠在下方（`intent` 缺席时折叠态仅工具名）」——只改这一条既有注释，**MUST NOT** 新增其它注释
- [x] 同文件逐字不动复核点：`TraceLine status` 三态判据（`output.isError` → `error` / `unpairedRunning(output, turnRunning)` → `running` / 否则省略）、`TraceIcon` 内 Spinner ↔ TerminalIcon 互斥、`TraceCollapsible {...cardAttrs(toolCallId)}` / `TraceCollapsibleTrigger` / `TraceCollapsibleContent` 结构、`CodeBlock` / `CodeBlockHeader>{command}` / `CodeBlockContent`（`className="max-h-60"` / `wrap` / `data-testid="block-tool-result"` / `{output?.content}`）、`cardAttrs` 的 `data-testid="block-tool-use"` + `data-tool-id`、`unpairedRunning` 原语、`BUILD_IN_TOOL_CARDS` 键面与声明序（`AskUserQuestion` / `finish_step` / `web_search` / `bash` / `read` / `write` / `edit`）、其余五卡渲染件——`git diff` 的 hunk **只允许**出现在 `BashCard` 的常量区、`TraceTitle` 与 doc 注释三处
- [x] 自查 阶段三（静态判据）：`grep -c "intent" packages/desktop/src/components/agent/message-cards/tool-cards.tsx` 为 **3**（读取常量一处、标题表达式一处、doc 注释一处）；`grep -c "TraceDescription\|trace-description" …/tool-cards.tsx` 为 **0**；`grep -c "truncate\|line-clamp\|text-ellipsis\|title=" …/tool-cards.tsx` 与改动前一致（前端零新增截断 / tooltip 面）；`grep -n "^export" …/tool-cards.tsx` 面与改动前逐行一致（零新导出——`BashCard` 仍模块私有，AGENT.md「no test-only exports」）；`grep -c "text-muted-foreground/70" …/tool-cards.tsx` 为 **0**（未采纳分色）；`pnpm -C packages/desktop exec vp check`（源码级 lint / 格式，不含测试运行）通过
- [ ] 交接 阶段三（测试相位）：`tool-cards.test.tsx` 新增 AC-5（`trace-title` 文本为 `Bash · 跑单测`、折叠态不含命令原文、展开后命令栏仍为命令）、AC-6（缺席 / 空串 / 非字符串三形态逐字 `Bash`）、AC-7（`trace-title` 唯一承载、超长 `intent` 原文完整保留）三组用例；既有 `:459` 的 `getByText('Bash')` 按 design「待决问题 4」处置——`:453` 夹具保持不带 `intent` 则断言**零改**，若测试相位为其补 `intent` 则同一提交内改为 `/^Bash/` 或改锚 `trace-title` 的 `textContent`——**由桌面端 test-design / test-gen / test-execution 相位承接**

## 阶段四：版本 + 边界自查 + 交接

- [x] `packages/desktop/package.json`：`version` 由落地时 `HEAD` 现值 **+1 patch**（当前 `0.6.52` → 期望 `0.6.53`；**MUST NOT** 写死字面号，以 `git diff HEAD -- packages/desktop/package.json` 的版本行为「恰好 +1」判据）；`packages/desktop/src-tauri/tauri.conf.json` 经 `"version": "../package.json"` 跟随（**零改动**）、`packages/desktop/src-tauri/Cargo.toml` 的 crate 版本行（`version = "0.1.0"`）**不随动**
- [x] 边界自查（越界即回退，逐条 `git diff --name-only` 核对）：`crates/infra/agent/src/sdk/protect/**` 零 diff（写保护判定输入仍只有 `command` 与会话执行根）；`crates/infra/agent/src/sdk/{sandbox.rs,tools.rs,context.rs,compact.rs,policy.rs,loop.rs}` 零 diff；`crates/core/agent/**`、`crates/infra/store/**`、`crates/core/workflow/src/prompts/**`、`AGENT.md` 零 diff；`packages/desktop/src/components/ui/trace.tsx`、`message-cards/blocks.tsx`、`src/lib/{agent-adapter,parse}.ts`、`src/types/generated/**`、`src/views/**` 零 diff；`.dev-team/architecture/models/**` 与 `graph/INDEX.toml` 零 diff
- [x] 读码复核（不改代码，只确认形状）：`mount.rs` 的 bash 臂 `intent` 无任何长度约束键、`required` 两值有序、工具级追加句在末尾；`bash.rs` 的意图闸位于 `resolve_timeout_ms` 之前且 `intent` 只被 `trim` 一次；`tool-cards.tsx` 的 `TraceTitle` 仍是**单文本节点**、`command` 只出现在展开态 `CodeBlockHeader`
- [x] 版本与生成物复核：`git diff HEAD -- packages/desktop/src-tauri/tauri.conf.json packages/desktop/src-tauri/Cargo.toml` 为空；`git diff --stat HEAD` 的实现文件集合恰为 design「修改文件」表的 4 项
- [ ] 交接 阶段四（生成物 + 门禁 + 回归，本相位不执行）：`bindings:export` 再生成后**期望零 diff**（`ToolUseBlock.input` 仍是自由 JSON，不出线新 IPC 类型）由桌面端 **static-check** 承接；整链静态门（`server:check` → `bindings:export` → `api:check` → `client:check`）与两个验证命令（`cargo test --manifest-path packages/desktop/src-tauri/Cargo.toml -p agent-runtime`、`pnpm -C packages/desktop exec vp test run src/components/agent/message-cards/tool-cards.test.tsx`）由桌面端 **static-check / test-execution** 承接。**交付物 = 期望登记与移交**
