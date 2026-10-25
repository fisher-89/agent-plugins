# 提案: desktop-write-protection

> **变更**: desktop-write-protection
> **日期**: 2026-10-23
> **状态**: draft
> **来源**: `openspec/explores/desktop-write-protection.md`（requirement 类型；八项决策 D1–D8 已定稿、插件端行为已实测、桌面端零实现）
> **前置依赖**: 无。桌面上位能力全部在位——`config::load` 恒返回 `WriteProtection`（`crates/core/config/src/lib.rs:814`）、`checks::globmatch::match_glob` 方言已验（`crates/core/checks/src/globmatch.rs:105`）、`loop::deny()` 拒绝形态现成（`crates/infra/agent/src/sdk/loop.rs:447`）、`with_explore_note_store` 提供同构 builder 缝先例（`crates/infra/agent/src/lib.rs:103`）

---

## 问题

### 现象：默认档下 Agent 可任意改写工作区配置，包括它自己的保护名单

桌面端 agent 默认 permission-mode 为 `BypassPermissions`（全放行，含 `bash`）。全部守卫链现状（`sdk/loop.rs` 逐 tool_call）：

```
① 成员校验   dispatch 面外即拒            loop.rs:375
② 档位门     policy::allows(mode, name)   loop.rs:381
③ 沙箱       sandbox_rewrite(cwd, …)      loop.rs:387
★ 写入保护   —— 空白 ——
④ 执行       tools::execute / explore_note loop.rs:399
```

`config::load` 已经把 `write_protection.files[]` 解析成恒合法的 `WorkspaceConfig`（含 4 类诊断、永不失败），前端配置页也逐条呈现 glob + reason——但 **`infra/agent` 全程零消费**（`write_protection` 在 `crates/infra/agent/**` grep 零命中）。于是：

| 现状 | 后果 |
|---|---|
| `write` / `edit` 对 `openspec/config.json` 零拦截 | Agent 可改写自己的 `write_protection`（自解除保护）、改写覆盖阈值 / 测试 suite / 上下文规则 |
| `bash` 无任何写入面判断 | `echo {} > openspec/config.json`、`rm openspec/config.json`、`cp /tmp/x openspec/config.json` 全放行 |
| 无内置锁定 | 用户未配置 `write_protection` 时保护面**完全为空**（插件端即使 config 非法也仍有内置 `**/openspec/config.json`） |
| 无批量还原拦截 | `git stash` / `git clean` / `git restore openspec/changes/**` 一次性抹掉工作区，且对任何按路径判定的护栏不可见 |

插件端（`plugins/dev-team/bin/src/commands/protect-files.ts` + `lib/shell-file-ops.ts`）这套能力已完整落地并经实测；桌面端是**从零复刻**（不是接线），因为两侧「工具调用」形态根本不同：

| 维度 | 插件端（PreToolUse hook） | 桌面端（loop 内进程内分发） |
|---|---|---|
| 拦截时机 | 独立进程，每工具调用一次 | loop 内分发链一段，无 hook 概念 |
| 工具名 | `Write` / `Edit` / `StrReplace` / `Bash` / `PowerShell` / `Shell` | `write` / `edit` / `bash`（封闭三工具） |
| 输入形态 | stdin JSON `{tool_name, tool_input}` | `ToolCall.function.arguments: Value` |
| 输出形态 | stdout `permissionDecision: deny` | `SystemNotice{subtype}` + is_error ToolResult 回灌 |
| 配置源 | `readConfig()` + zod（非法即整体回默认） | `config::load(root) -> ConfigReport`（永不失败，仅 diagnostic） |

### 复刻不等于照抄：两类硬性偏离 + 一处严禁"顺手优化"

实测插件端产物（`dist/.../hooks.cjs protect-files`，worktree 内含 `openspec/`）暴露的漏面与错位（explore §6.4 / §6.5 原始记录）：

| 命令 | 插件端 | 根因 |
|---|---|---|
| `node -e "require('fs').writeFileSync('openspec/config.json','{}')"` | allow | `checkShellCommand:230` 对 `^(python\|python3\|node)\s` 整体 `return null`（连提取器都不进） |
| `python -c "open('openspec/config.json','w').write('{}')"` | allow | 同上 |
| `cp /tmp/x openspec/config.json` | allow | `extractFileOps` 无 `cp` 族 |
| `dd if=/tmp/x of=openspec/config.json` / `sed -i …` / `truncate -s 0 …` / `find openspec -delete` | allow | 无对应族 |
| `echo {} &> openspec/config.json` | deny | 前缀排除集 `[^-=]` 没排 `&`——纯属侥幸成立 |

因此 `plugins/dev-team/bin/src/lib/shell-file-ops.ts` 的 `extractFileOps` 是**精度来源也是雷区**：它的掩码算法与正则族必须如实照译（照抄 `[^-=]` 排除集，MUST NOT "顺手优化"成 `[^-=&|]`——那会把 `&>` 从 deny 静默变 allow），而 node/python 整体豁免必须**不照搬**、`cp`/`dd`/`sed -i`/`truncate`/`find -delete` 必须**补上**。

### 拒绝形态的隐藏成本：deny 不中断 run，模型会换路重试

`deny()`（`loop.rs:447`）把 reason 同时写进 `SystemNotice.payload.reason`（前端展示）与 is_error `ToolResult.content`（**模型可见**）；注释明写「run 不中断」。模型被拒后的第一反应是换路：

```
write openspec/config.json        → deny
bash: echo {} > openspec/config.json  → deny
bash: cp /tmp/x openspec/config.json  → deny
bash: node -e "fs.writeFileSync(…)"   → deny
```

同一文件连撞四五次、白烧轮次。桌面端 policy 是三档纯函数、**没有审批通道承接**（插件端文案可假设用户会接手），所以拒绝串必须显式承担「掐断这条链」的功能：告知不可改 → 请调整方案 → **不得改用其他命令绕过，无法规避则立即停并通知用户**。

## 提案

只改桌面端 `packages/desktop/src-tauri`（含 `packages/desktop/package.json` 版本号）；插件 / CLI / 前端零改动。形状收敛为八面：

1. **新增 `sdk/protect/` 模块（规则装配 + 判定，各自单点）**：`ProtectRules::load(workspace_root: Option<&Path>)` 是唯一 IO 点（`config::load` + 内置规则合并 + 相对 glob 归一），`ProtectRules::evaluate(tool, input, cwd) -> Option<String>` 是零 IO 纯函数（返回拒绝串）。装配与判定分离：装配每 run 一次、判定每 tool_call 一次——与 `defense` / `retry` / `liveness` 的「装配点算好、loop 纯消费」形态同构，evaluate 可密集单测而不必造 fs。

2. **内置锁定只收一条 `**/openspec/config.json`**（glob 由 `foundation::layout::config_path` + `**/` 前缀派生，MUST NOT 在产品源码写 `openspec` / `config.json` 字面量）。`**/openspec/changes/**/workflow.json` **不复刻**：`core/workflow` 已立双向墙（DB 即状态，MUST NOT 产出 / 零读取，legacy 文件惰性不解析）。内置规则独立于 config 解析结果——`config::load` 永不失败，配置缺失 / 坏 JSON 时内置面照常生效。

3. **用户规则照抄语义（同一份 config 必须双端同解）**：`write_protection.files[]` 逐条取 `glob`，**绝对原样 / 相对前缀 `**/`**（同插件端 `loadPatterns:44`，否则无通配模式只命中 root 相对形态）；`reason` 原样透传并做 `%s`（路径）/ `%t`（工具名）替换；无 reason 落通用回落串。本仓 `openspec/config.json` 现存一条真实用户规则（`plugins/dev-team/bin/knip.json`，**无 reason**）——「相对 glob 加前缀 + 回落串」两条路径落地即有真用例可比对。

4. **write / edit 按路径拦**：检查点在 ③ 沙箱**之后**、④ 执行之前——此时 `path` 已是 canonical 绝对路径（`sandbox::check` 产物，剥 `\\?\`、`\`→`/` 归一），MUST NOT 再次 canonicalize（零 IO）；`read` / `grep` / `glob` / `ls` 与 `create_explore_note` 不在射程（**专用工具直通为已裁边界**：用户若把 `openspec/explores/**` 写进保护，`edit` 被拦而 `create_explore_note` 直通，一致性缺口记录在案，不再视为待办）。

5. **bash 按「命令里的写入目标」拦（两层 + bulk revert）**：
   - **第一层：三态提取器照译**（write / delete / move / revert）。精度全部来自双变体等长掩码——变体 A（只掩 heredoc，保留引号）跑写入正则与参数切片，变体 B（掩 heredoc + 引号）定位命令名；`$var` token 在 op 落点丢弃；`2>&1` 类 fd 复制跳过；`->` / `=>` 由 `[^-=]` 前缀守卫排除。
   - **档 B 五项补充**：`cp` / `Copy-Item`（目标位 = 末参）、`dd`（`of=`，显式排除 `if=`）、`sed -i` / `perl -i`（无 `-i` 是纯读，MUST NOT 拦）、`truncate`（`-s` 之后的路径）、`find … -delete`（搜索根首个非 flag 参数，等价"保护整个搜索根"——删除场景该保守）。
   - **批量还原拦截**：`git stash`（除 `list` / `show`）、`git clean`（无路径参数或参数命中 `openspec/**`；`-n` / `--dry-run` 放行）、`git restore` / `git checkout --` 目标是 `openspec/changes/**` → deny；源码目标放行（规范撤销动作）。
   - **路径解析**：绝对 → 归一原样；相对 → `<cwd>/path` 归一（cwd = 会话执行根 `LoopTurn.cwd`）；含 `$` / 无法解析 → 跳过（放行）。

6. **node / python 明文脚本判别（桌面端新增，插件端零参考）**：**不做整体豁免**（插件端 `^(python|node)\s` return null 的旁路 MUST NOT 复刻），改**关联式**——写 API 名与其**路径实参**绑定的正则逐条对整条命令文本跑（JS：`writeFileSync` / `writeFile` / `appendFile*` / `createWriteStream` / `copyFile*` / `rename*` / `unlink*` / `rm*` / `fs.promises.writeFile` / `openSync(p,'w'|'a'|'wx')`；PY：`open(p,'w'|'a'|'x'|'wb'|'w+')` / `Path(p).write_text|write_bytes` / `shutil.copy*|move` / `os.remove|unlink|rename|replace`）。关联式**不切脚本段**，故 `node -c`（`--check`，不写文件）与 `python -c`（执行命令）的语义差自动消失，嵌套形态 `bash -c 'node -e "…"'` / `pwsh -Command "…"` 一并覆盖。裸脚本文件（`node x.js` / `python x.py` / `-m` 模块）放行——承认不可静态判别。

7. **拒绝文案三条串，首要读者是模型**：内置 `config.json` 串 / 用户 reason 透传（`%s`、`%t`）/ 用户无 reason 回落串。三条均含「不得改用其他命令绕过」与「无法规避则立即停止并通知用户」两句（这是**功能句**，不是客套：机制侧 `deny` 不中断 run，掐断换路重试链只能靠它）。subtype 复用 `permission_denied`（前端不复用该串做分支，零协议改动）。

8. **工作区根通道（builder 缝，与 `with_explore_note_store` 同构）**：保护规则 MUST 读**主仓** root（`openspec/config.json` 的权威位置），MUST NOT 读会话执行根（change 流程下 = worktree）。root 只在命令层显式在手（`Store` 只有 `db` + `dimension`，无 root 字段）：

```
commands/exec/mod.rs:132 · commands/change_flow/mod.rs:143   （两处均持 root）
  → compose_turn(…, workspace_root: &Path)                    （新增尾参）
  → EngineFacade::with_workspace_root(root)                   （与 with_explore_note_store 并列）
  → SdkRunner.workspace_root: Option<PathBuf>
  → PumpPayload.workspace_root → 泵起播时 protect::load(…)     （每 run 一次）
  → LoopTurn.protect: ProtectRules → loop 内 evaluate（零 IO）
```

装配缺席（`None`）时**仅内置规则生效**（用户规则缺席）——不产出「保护关闭」的空档。

### 范围裁决（explore 决议的落地口径）

- **D1 恒读主仓**：change 流程执行根是 worktree，但配置权威在主仓；worktree 内 `openspec/config.json` 仍被内置 glob（`**/openspec/config.json` 对任一前缀命中）保护，故「读主仓 + 拦两份」成立。代价：worktree 内本地改 config 不影响规则（写入本就被禁，构不成活路）。
- **D2 每 run 装配一次**：泵一轮一命（`runner.rs` 一轮一命纪律）⇒ 等价「每相位重读」；run 内改 config 不生效（与「config 运行期不变」判定一致）。
- **D3 检查点在沙箱之后**：拿到 canonical 绝对路径，规则无需关心 `..` / 符号链接逃逸（沙箱已先行拒绝逃逸）。
- **D4 内置集合只留 config.json**：`workflow.json` / `eval.json` 均已退役，不复刻。
- **D5/D8 复刻边界取档 B**：照译三态提取 + bulk revert，补 `cp` / `dd of=` / `sed -i` / `truncate` / `find -delete`；`xargs` / `sh -c` 递归拆解**不做**（与"生成脚本再执行不可防"同属承认边界，写进 spec 知情边界）。
- **D6 拒绝串面向 Agent**：见提案 7；「中断」= 模型结束本轮（`finish_success`），无引擎新机制，相位会话停手后如何收敛归编排层，protect 不越界。
- **D7 glob 函数复用**：一律经 `checks::globmatch::match_glob`（方言已验：`**/` 零段可命中、Windows 盘符归一、无通配符走目录前缀），**MUST NOT 自造第二份方言**；因此 `agent-runtime` 增 `checks`（core 纯层）依赖边——取「登记新增边 + 方言单点」而非「把 `match_glob` 下沉 `foundation`」（foundation 的零外部依赖不变量是刻意设计，`glob` crate 会破它）。
- **over-block 方向不安全**：裁剪提取器时宁可漏 stdout 形态，也不能把 `echo foo > notes.txt` 判成受保护——双向断言进验收。
- **拒绝串 subtype 复用 `permission_denied`**：payload 词汇 `{tool, reason}` 零扩展，`bindings.ts` 零 diff。

## 能力

### 新增的能力

- **desktop-write-protection** — 八条 requirement：保护规则装配（内置 + 用户合并 / glob 归一 / 顺序）；write·edit 路径判定与解析口径；shell 三态提取与档 B 补充（含掩码精度纪律）；批量还原拦截；node/python 内联脚本关联式判别；拒绝指令文案三条串；装配与判定的分工与失败面（唯一 IO 点、判定零 IO、fail-open 边界）。

### 修改的能力

- **desktop-agent-execution** — **ADDED**「loop 守卫链写入保护闸与工作区根注入缝」（四段守卫链顺序 / 检查点位置 / 拒绝形态复用 `deny()` / 每 run 装配一次 / `compose_turn → EngineFacade → SdkRunner → PumpPayload` 根通道 / `create_explore_note` 不在射程 / 未注入时仅内置）；**MODIFIED**「SDK 引擎 bash 工具」（知情边界由"护栏为零"改写为"写入面护栏到位、真沙箱仍缺"）；**MODIFIED**「SDK 引擎静态权限与路径沙箱」（路径沙箱不再是唯一护栏、bash 的"护栏为零"作废）；**MODIFIED**「SDK 租户 MVP 边界与已知限制」（第 4 条偿还一半：写入保护落地，命令级任意执行 / 真沙箱 / 交互审批仍为边界）。
- **desktop-crate-layout** — **ADDED**「glob 方言单点复用与 agent-runtime → checks 依赖边」（唯一方言实现禁令 / 边方向合法性 / `checks` 反向零依赖 / 该条款为 `agent-runtime` workspace 内依赖集合的现行口径）。

其余能力零改动，理由：

- **desktop-workspace-config**：`WriteProtection` / `WriteProtectionFile` 模型、`load` 诊断面、配置页只读呈现全零改——新增的只是**消费侧**，语义落新能力。配置页「未配置」占位在保护面非空后略显误导，但补齐需前端拿内置规则（新 DTO 字段或前端常量），不属本篇（见遗留观察项）。
- **desktop-crate-layout 的其余条款**：`checks` 反向不依赖 agent，core 零 Tauri 结论不变；既有「agent-runtime 仅依赖 agent」表述已随多轮演进陈旧，本变更只登记新增边、MUST NOT 借机重写依赖结论。
- **desktop-change-orchestration / desktop-phase-prompts / desktop-change-flow-\***：保护闸不改计划图、不改步声明、不改相位 prompt、不改仲裁句式（拒绝串是工具结果，不是相位文案）。
- **crates/core/config、crates/core/checks、crates/infra/{exec,vcs,checks}、crates/core/{agent,workflow,orchestration}**：零改动。

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `crates/infra/agent/Cargo.toml` | 新增 `checks = { workspace = true }`（复用 glob 方言单点） |
| `crates/infra/agent/src/sdk/protect/mod.rs`（新） | `ProtectRules`（`Vec<Rule>`）+ `load(Option<&Path>)`（唯一 IO 点）+ `evaluate(tool, input, cwd) -> Option<String>` + 目标路径解析（绝对原样 / 相对拼 cwd / 剥 `\\?\` / `\`→`/`） |
| `crates/infra/agent/src/sdk/protect/rules.rs`（新） | 内置规则派生（`foundation::layout::config_path` + `**/` 前缀）+ 用户规则合并（绝对原样 / 相对加 `**/` / 空 glob 跳过）+ 规则序（内置在前）+ reason 选择 |
| `crates/infra/agent/src/sdk/protect/shell.rs`（新） | 双变体等长掩码（`mask_heredocs` / `mask_quoted_and_heredoc`）+ `tokenize` / `is_flag` / `is_literal_path` + 三态提取（重定向族 / PS 方言 / 删除族 / 移动族 / git 子命令）+ 档 B 五项 + bulk revert 判定 |
| `crates/infra/agent/src/sdk/protect/inline.rs`（新） | node/python 关联式判别（JS / PY 写 API 表逐条正则，路径取第一个字符串实参） |
| `crates/infra/agent/src/sdk/protect/reason.rs`（新） | 三条文案常量 + `%s` / `%t` 替换 + 命中规则 reason 选择 |
| `crates/infra/agent/src/sdk/mod.rs` | 挂 `protect` 模块与四个测试模块（另有模块内 `mod_test` 命名隔离扫描） |
| `crates/infra/agent/src/sdk/loop.rs` | `LoopTurn.protect: ProtectRules` 字段；守卫链在 ③ 沙箱之后、④ 执行之前插入写入保护段（拒绝走既有 `deny()`，`continue`） |
| `crates/infra/agent/src/sdk/runner.rs` | `SdkRunner.workspace_root: Option<PathBuf>` + `with_workspace_root(self, PathBuf) -> Self`；`PumpPayload.workspace_root`；泵起播时 `ProtectRules::load(payload.workspace_root.as_deref())` 并装入 `LoopTurn` |
| `crates/infra/agent/src/lib.rs` | `EngineFacade.workspace_root: Option<PathBuf>` + `with_workspace_root` + `runner_for` sdk 臂透传 |
| `crates/infra/agent/src/compose.rs` | `compose_turn(…, workspace_root: &Path)` 新增尾参 → `EngineFacade::with_workspace_root` |
| `src/commands/exec/mod.rs` | `compose_turn` 调用补 `root`（主仓 root，非 `exec_root`） |
| `src/commands/change_flow/mod.rs` | 同上（主仓 `root`，非 worktree `exec_root`） |
| `packages/desktop/package.json` | `version` 0.5.28 → 0.5.29（用户可见：受保护文件拒写 + 拒绝处置指令） |

### 测试文件

| 用例 / 装置 | 位置 | 处置 |
|---|---|---|
| 规则装配：内置一条派生正确 / 相对 glob 加 `**/` / 绝对原样 / 空 glob 跳过 / config 缺失与坏 JSON 只用内置 / 内置先于用户（同命中取内置 reason）/ 非法 glob 不命中 | `sdk/protect/rules_test.rs`（新） | 新增（tempdir + 真实 config 文件驱动；`mod_test` 命名隔离扫描照常覆盖新文件） |
| 掩码与提取：heredoc 体掩码后 `>` 不算命令 / 引号内 `rm -rf /` 不算命令 / 两变体等长 / `2>&1`·`3>&2` 跳过 / `echo a->b`·`=> p` 不误判 / `echo {} &> p` 仍 deny（照抄排除集）/ `$var` 落点丢弃 / `>`·`>>`·`>\|`·`tee`·`>&`·PS 重定向·PS cmdlet·`[System.IO.File]::WriteAll*`·`rm`·`rmdir`·`unlink`·`del`·`rd`·`Remove-Item`·`git rm`·双参 `mv`·`ren`·`git restore --source=`·`git checkout <ref> --` 全命中 | `sdk/protect/shell_test.rs`（新） | 新增（源出插件端 `shell-file-ops.test.ts` 用例矩阵，逐条重译 + 档 B 五项） |
| 档 B 五项：`cp` 目标位命中 / `dd of=` 命中而 `if=` 不命中 / `sed -i` 命中而 `sed`（无 `-i`）不命中 / `truncate -s 0` 命中 / `find <root> -delete` 命中根 | `sdk/protect/shell_test.rs` | 新增 |
| bulk revert：`git stash` / `git stash push` / `git stash pop` 拒、`git stash list`·`show` 放；`git clean` 无路径拒、`-n`·`--dry-run` 放、参数命中 `openspec/**` 拒、源码路径放；`git restore openspec/changes/x/proposal.md` 拒、`git restore src/a.rs` 放 | `sdk/protect/shell_test.rs` | 新增 |
| inline：JS / PY 表逐项命中（`writeFileSync` / `createWriteStream` / `fs.promises.writeFile` / `open(p,'w')` / `Path(p).write_text` / `shutil.copy` / `os.remove` …）；裸脚本 `node scripts/setup.js` 放行；`node -c`（`--check`）不误拦；嵌套 `bash -c 'node -e "…"'` / `pwsh -Command` 命中；变量中转漏面（`const p='…'; writeFileSync(p,…)`）留痕为已知漏面 | `sdk/protect/inline_test.rs`（新） | 新增 |
| 文案：三条串逐字 / `%s`·`%t` 替换 / 用户无 reason 回落 / 内置串必含「不要改用其他命令」与「立即停止并通知用户」两句 | `sdk/protect/reason_test.rs`（新） | 新增 |
| loop 闸门：write / edit 命中 → `permission_denied` notice + 同 id is_error ToolResult、run 不中断（后续轮继续）；bash 三路（重定向 / `rm` / `node -e`）命中 → 同形态；未命中 → 执行照常；`create_explore_note` 直通；Bypass 档下闸门仍生效 | `sdk/loop_test.rs` | 新增用例 + 既有 `loop_turn` 装置补 `protect` 字段 |
| 泵装配：注入 root → 用户规则生效；未注入 → 仅内置生效（`openspec/config.json` 仍被拒）；执行根为 worktree 而规则来自主仓时仍生效（主仓口径钉住） | `sdk/runner_test.rs` | 新增 |
| `compose_turn` 新尾参机械补参（约 40 处调用） | `crates/infra/agent/src/{compose_test,worker_test,lib_test}.rs`、`src/commands/{exec,change_flow}/mod_test.rs` | 改写（签名不跨 crate，仅参数面） |

### 不要修改

- `crates/core/config/**`（`WriteProtection` 模型与 `load` 诊断面零改）；`crates/core/checks/**`（尤其 `globmatch.rs` 方言与 `match_glob` 签名零改）。
- `crates/infra/{exec,vcs,checks}/**`、`crates/core/{agent,workflow,orchestration}/**`、`crates/infra/store/**`。
- `crates/infra/agent/src/sdk/{bash,tools,policy,sandbox,explore_note,context,compact,resume,model_spec}.rs` 语义零改（仅 `mod.rs` 挂模块、`loop.rs`/`runner.rs` 两处接线）。
- 前端 `packages/desktop/src/**` 与 `src/types/generated/bindings.ts`（零新载荷、零新 IPC 命令，`bindings:check` 应零 diff）。
- `plugins/**`（插件源已弃用，零改动零 bump）；`openspec/config.json`（保护规则字段沿用既有 `write_protection.files[]`，零 schema 扩展）。
- `openspec/changes/desktop-write-protection/explore.md`（只读）。

## 验收标准

| ID | 验收条件 | 验证方法 |
|----|---------|----------|
| AC-1 | 内置锁定在**任意档位**（含默认 `BypassPermissions`）下生效：`write` / `edit` 指向任一 `<root>/openspec/config.json`（含 worktree 内副本）被拒，事件面为 `SystemNotice{subtype:"permission_denied", payload:{tool, reason}}` + 同 id is_error ToolResult，run 不中断；同文件 `read` / `grep`（只读面）放行 | `protect::rules_test` + `loop_test` 四臂用例 |
| AC-2 | `write_protection.files[]` 语义双端同解：相对 glob 自动前缀 `**/` 后命中、绝对 glob 原样命中、`reason` 原样透传且 `%s` / `%t` 完成替换、无 reason 落回落串（本仓现存 `plugins/dev-team/bin/knip.json` 这条真实规则即回落串用例）；config 缺失 / 坏 JSON 时内置面仍生效（用户面为空） | `protect::rules_test` + `reason_test` |
| AC-3 | bash 写入面覆盖：重定向族（`>` / `>>` / `>\|` / `tee` / `>&`）、PS 方言（重定向 / 六 cmdlet / `[System.IO.File]::WriteAll*`）、删除族（`rm` / `rmdir` / `unlink` / `del` / `rd` / `Remove-Item` / `git rm`）、双参移动族（旧删 + 新写）、`git restore --source=` / `git checkout <ref> --` 命中保护即拒 | `protect::shell_test` 族矩阵 |
| AC-4 | 档 B 五项在插件端基础上**扩面**：`cp` / `dd of=` / `sed -i` / `truncate` / `find … -delete` 命中即拒；`dd if=` 与无 `-i` 的 `sed` 不误拦 | `protect::shell_test` 档 B 用例 |
| AC-5 | 精度双向断言：`echo foo > notes.txt`、`cat <<EOF > notes.txt`、`echo "rm -rf /" > notes.txt`、`echo a->b`、`2>&1`、heredoc 体里的受保护路径均**不拦**；同一受保护文件的五条等价写法（重定向 / `rm` / `cp` / `node -e` / `sed -i`）**全拦** | `protect::shell_test` 误拦-漏拦双向用例 |
| AC-6 | 批量还原拦截：`git stash`（除 `list` / `show`）拒；`git clean`（无路径参数或参数命中 `openspec/**`）拒，`-n` / `--dry-run` 放行；`git restore` / `git checkout --` 命中 `openspec/changes/**` 拒，源码目标放行 | `protect::shell_test` bulk revert 用例 |
| AC-7 | node / python：`node -e "…writeFileSync('openspec/config.json',…)"` 与 `python -c "open('openspec/config.json','w')…"` 被拒（插件端旁路不复刻）；嵌套 `bash -c 'node -e "…"'` 同拦；裸脚本 `node scripts/setup.js` 放行并留痕为已知漏面 | `protect::inline_test` + `loop_test` bash 臂 |
| AC-8 | 拒绝串承担掐断换路链的功能：三条串均含「不要改用其他命令或工具尝试绕过」与「无法规避请立即停止并通知用户」两句；`reason` 同时进 notice 载荷与 ToolResult 内容（模型可见） | `reason_test` 逐字断言 + `loop_test` notice 载荷断言 |
| AC-9 | 装配单点：每 run 恰一次 `config::load`（泵起播时）；loop 内零 IO（`sdk/loop.rs` 无 `fs` / `config::load` / `canonicalize` 调用，源码审查 + 用例）；run 内改 config 不改本轮行为 | `runner_test` + 源码扫描断言 |
| AC-10 | 主仓口径：change 流程（执行根 = worktree）下规则取自主仓 `config.json`；worktree 内 `openspec/config.json` 仍被内置 glob 拦 | `runner_test` 双根用例 |
| AC-11 | 既有语义零变：三档 policy 矩阵、路径沙箱逃逸拦截、成员校验、`create_explore_note` 执行面行为逐条不变；未配置保护时普通写操作零拒绝（内置面除外） | 既有 `policy_test` / `sandbox_test` / `explore_note_test` 全绿 + 新增放行用例 |
| AC-12 | 全管线通过：`cargo test --workspace` 全绿；覆盖率不低于阈值（lines ≥ 90 / branches ≥ 85 / functions ≥ 90，基线 97.12 / 93.15 / 93.68）；`bindings:check` 零 diff（零新载荷出线） | `pnpm -C packages/desktop run check` / `run test` + test-execution 门禁 |
| AC-13 | `packages/desktop/package.json` 0.5.28 → 0.5.29；`plugins/**` 零改动零 bump；`openspec/config.json` 零 schema 扩展 | 版本 diff + `Cargo.toml` / config 审查 |

## 风险

| ID | 风险 | 影响 | 概率 | 缓解措施 |
|----|------|------|------|----------|
| R-1 | **过拦误伤**（over-block 方向不安全）：提取器把普通命令判成受保护写入，正常开发被拒 | 可用性受损，用户无法完成本来合法的写 | 中 | 沿插件端精度护栏（双变体掩码 / `$var` 丢弃 / `2>&1` 跳过 / `[^-=]` 前缀守卫）；AC-5 双向断言钉住「`> notes.txt` 必须放行」；残留误伤可经 config 移除该条用户规则（内置 `config.json` 不可移除——这正是保护的目的） |
| R-2 | **换路重试链**：`deny` 不中断 run，模型可能在同一文件上连撞多次、烧轮次 | 时间与 token 浪费 | 中（模型行为面） | 拒绝串显式两句指令（不得绕过 / 无法规避即停）；AC-8 逐字断言；不引入引擎侧「同文件二次拒绝即硬中断」机制（超出 protect 职责，且会误伤正当重试用例） |
| R-3 | **漏面绕过**：`xargs` / `sh -c` 递归拆解 / 生成脚本再执行 / `-m` 模块 / 变量中转路径 / base64 / `printf` 喂 stdin 均不可静态判别 | 保护可被有意绕过 | 高（既定） | 作为**知情边界**写进 spec（MUST NOT 被当作安全缺口遗漏）；档 B 已比插件端多拦五项；真正的兜底是真沙箱 / 交互审批（仍留后续 change） |
| R-4 | **inline 判别误拦**：关联正则可能命中日志 / 文档字符串里的 API 名 + 路径 | 拒掉无害命令 | 低 | 采关联式（API 名 + 其路径实参绑定）而非全文共现式；用例钉住「`console.log('…config.json','writeFileSync')` 类文本不误拦」 |
| R-5 | **glob 方言分叉**：`glob` crate 的 `**` 只能整段、无通配模式走目录前缀语义，用户写怪 glob 时与插件端 picomatch 可能不同解 | 两侧行为不一致 | 低 | 强制经 `checks::globmatch::match_glob` 单点（MUST NOT 自造）；用户相对 glob 一律加 `**/`（与插件端同款处理）；非法模式按不命中收敛（fail-open 于防护、fail-closed 于匹配） |
| R-6 | **主仓 vs worktree 口径**：worktree 内对 `config.json` 的本地改动不参与规则装配 | 用户在 worktree 里调规则无效 | 低 | D1 裁定 + 写进 spec；且 `config.json` 写入本就被禁，构不成活路；worktree 起点由主仓派生故初始一致 |
| R-7 | **依赖边扩张**：`agent-runtime → checks` 被质疑边界混淆（agent 边界依赖检查域纯层） | crate 图审查受阻 | 中 | 只取纯层 `globmatch`（零 spawn / 零 Tauri / 反向零依赖）；`desktop-crate-layout` 新增条款留痕并写明「禁第二份方言」的理由；替代方案（下沉 `foundation`）被否：foundation 的零外部依赖是刻意不变量 |
| R-8 | **装配一次的新鲜度**：run 内用户改 config 不生效 | 配置改了要等下一 run | 低 | 一轮一命 ⇒ 等价「每相位重读」；写进 spec；相位间本就重启会话 |
| R-9 | **配置页误导**：保护面非空后配置页仍显示「未配置」，用户不知道内置锁定存在 | 认知落差（被拒时不知道规则来源） | 中 | 拒绝串直接说明「该文件受写入保护」+ 由用户自行操作；面板呈现内置规则列入遗留观察项（需前端拿内置规则，另行小项） |
| R-10 | **提取器性能**：每条 bash 命令跑多层正则 | 长命令下延迟放大 | 低 | 全部正则为单量词线性模式（MUST NOT 引入嵌套量词 / 回溯爆炸形态）；不设命令长度上限（设上限等于送一条绕过路径）；加 100 KB 级命令耗时冒烟断言 |
| R-11 | **与既有"护栏为零"表述冲突**：三处 spec 文本（bash 工具 / 沙箱 / MVP 边界）声明 bash 保护为零 | 两套口径并存、审查自相矛盾 | 高（本篇既定） | 三处同一变更内 MODIFIED（拷贝原段再改），MUST NOT 留两套口径；新增条款写明「写入面到位、真沙箱仍缺」的精确分界 |

## 遗留观察项（不在本篇范围）

1. 配置页 write_protection 分区呈现**内置锁定规则**（当前「未配置」占位在保护面非空后误导）——需前端取内置规则，二选一路径（新 DTO 字段 / 前端常量），另立小项。
2. `create_explore_note` 与用户规则的一致性缺口（用户把 `openspec/explores/**` 写进保护时专用工具直通）——已裁为不做，后续若出现真实诉求再议。
3. `xargs` / `sh -c` 递归拆解 / 生成脚本再执行等漏面的收敛路径只有「真沙箱（job object / 容器）或交互审批」，二者仍属后续 change。
4. 拒绝串的「二次拒绝即硬中断」形态（自动掐断换路链而非依赖模型守指令）——需引擎侧机制，与 protect 职责边界冲突，观察实际撞链频率后再议。
5. 插件端 `extractFileOps` 与桌面端 `protect::shell` 的双实现漂移风险：插件源已弃用（AGENT.md 声明不再开发），故不设对账机制；若插件侧将来复活，需重新评估单点化。
