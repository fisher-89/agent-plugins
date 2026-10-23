# 桌面端复刻 write_protection：配置读取 + 内置锁定 + bash 写入判断

workflow_type: requirement

> **需求原话**：桌面端读取 config `write_protection`，禁止 Agent 写入；同时复刻插件端的内置锁定文件（桌面端废弃的除外）和 bash 写入判断。
>
> **本篇定位**：调研 + 差距清单 + 待决策问题。不含实现方案定稿（design 阶段的事）。

---

## 1. 需求拆三层

```
「禁止 Agent 写入」
   │
   ├─ ① 配置面  读 openspec/config.json → write_protection.files[]  ← 桌面端 config 模型已有，agent 侧零消费
   ├─ ② 内置面  复刻插件端内置锁定 glob（workflow.json 已废弃 → 只剩 config.json）
   └─ ③ 判断面  write/edit 按路径拦；bash 按「命令里的写入目标」拦（含删除 / 批量还原）
```

插件端这套已完整落地，桌面端是**从零复刻**（不是接线），因为两边的"工具调用"形态根本不同：

| 维度 | 插件端 | 桌面端 |
|---|---|---|
| 拦截时机 | PreToolUse hook，独立进程，每工具调用一次 | loop 内 in-process 分发链，无 hook 概念 |
| 工具名 | `Write` / `Edit` / `Bash` / `PowerShell` / `Shell` / `StrReplace` | `write` / `edit` / `bash`（三个，封闭集） |
| 输入形态 | stdin JSON `{tool_name, tool_input}` | `ToolCall.function.arguments`（Value） |
| 输出形态 | stdout `permissionDecision: deny` | `SystemNotice{subtype}` + is_error ToolResult 回灌 |
| 配置源 | `readConfig()` + zod | `config::load(root)` → `ConfigReport`（**已有** `WriteProtection`） |

---

## 2. 桌面端现状：三块拼图，缺两块半

```
core/config                       infra/agent (sdk loop)               前端
─────────────                     ──────────────────────               ─────
WorkspaceConfig                     ① 成员校验 dispatched ∋ name
  └─ write_protection: Option<...>  ② policy::allows(mode, name)      views/config/
       └─ files: Vec<{glob,reason}> ③ sandbox_rewrite(cwd, ...)         write-protection-section.tsx
                                    ④ tools::execute / explore_note     └─ 只展示 glob + reason
config::load(root) → ConfigReport
  （含 4 类诊断，永不失败）                                              「未配置」占位
```

- **已有**：`core/config` 的 `WriteProtection` / `WriteProtectionFile`（`lib.rs:138`、`:146`），`load()` 完整诊断面（`:814`），前端 config 页展示（`write-protection-section.tsx`）。
- **缺口**：`infra/agent` 全程未消费 `write_protection`（grep 零命中）；内置锁定零；bash 写入判断零。
- **关键上下文**：默认档 = `BypassPermissions`（全放行，含 bash）。所以现状是 **agent 可以任意写 `openspec/config.json`**，包括改自己的 `write_protection`。

### 守卫链现状（`sdk/loop.rs`）

```
for tool_call in tool_calls:
    ① 成员校验   if !dispatched.contains(name)  → deny(tool_unavailable)   :375
    ② 档位门     if !policy::allows(mode, name) → deny(permission_denied)  :381
    ③ 沙箱       sandbox_rewrite(cwd, name, input) → deny(sandbox_denied)  :387
    ★ 写保护     —— 空白 ——
    ④ 执行       tools::execute(cwd, name, sandboxed) / explore_note       :399
```

`deny()`（`:447`）已具备理想形态：`SystemNotice{subtype, payload:{tool, reason}}` + 同 id 的 is_error ToolResult，**run 不中断**（模型换路自愈）。新检查只需接进这条链，拒绝形态零新增机制。

---

## 3. 插件端参考实现（`plugins/dev-team/bin/src`）

### 3.1 规则装配 `commands/protect-files.ts:44`

```
loadPatterns(projectRoot):
  内置  **/openspec/changes/**/workflow.json   reason 提示 phase_log/backtrack/change_create
  内置  **/openspec/config.json                reason 「请提供修改方案，通知用户自行操作」
  用户  write_protection.files[]
          glob 绝对 → 原样；相对 → 前缀 **/
          reason 原样（支持 %s = 路径、%t = 工具名）
```

内置规则**独立于 config 解析结果**（config 非法 JSON 时内置照样生效）——这点在桌面端天然成立，因为 `config::load` 永不失败、恒返回 `WriteProtection`（`None` 时只有内置）。

### 3.2 工具路由 `:280 evaluateToolAccess`

| 工具名 | 取字段 | 检查 |
|---|---|---|
| Write / Edit / StrReplace | `file_path` | `isProtected(path)` → deny |
| Bash / Shell | `command` | `checkBashCommand` |
| PowerShell | `command` | `checkBashCommand` + `checkPowerShellCommand`（同一 `checkShellCommand`） |
| 其它 / 缺字段 / 非法 JSON | — | **fail-open → allow** |

### 3.3 shell 命令判定 `:222 checkShellCommand`

```
① python|python3|node 开头  → 直接放行（豁免，历史遗留）
② checkExtractedOps         → 提取 write/delete 路径 ∩ 保护 glob → deny
③ checkBulkRevert           → 批量还原拦截
```

提取器 `lib/shell-file-ops.ts:395 extractFileOps` 一次跑 bash + PowerShell 双方言：

| 类别 | 来源 |
|---|---|
| write | `>` `>>` `>\|` `tee` `>&`（跳过 `->` `=>` 与 `2>&1` 类 fd 复制）；PS `Set-Content`/`Out-File`/`Add-Content`/`Export-Csv`/`Export-CliXml`/`Tee-Object`；`[System.IO.File]::WriteAll*`；`git restore --source=`、`git checkout <ref> --` |
| delete | `rm` / `rmdir` / `unlink` / `del` / `rd` / `Remove-Item` / `git rm` |
| write+delete | 双参 `mv` / `Move-Item` / `ren` / `Rename-Item`（旧删 + 新写） |
| revert | `git restore <paths>`、`git checkout -- <paths>`（**不拦**，是规范撤销动作） |

三类精度护栏（都在 `shell-file-ops.ts` 里，是这套实现最贵的部分）：

```
① 引号 / heredoc 双重掩码
   变体 A（只掩 heredoc，保留引号）→ 跑写入正则 + 切参数（引号里的真路径仍可匹配）
   变体 B（掩 heredoc + 引号）    → 定位命令名（字符串里的 "rm" / "git stash" 不算命令）
② `$var` token 在 op 落点丢弃（未展开变量永远不可能命中真实路径）
③ 长度保持（掩码字符替换，索引 1:1 互通）
```

批量还原拦截 `:174 checkBulkRevert`（保护面之外的独立规则）：

- `git stash`（除 `list` / `show`）→ deny
- `git clean`（无路径参数，或参数命中 `openspec/**`；`-n`/`--dry-run` 放行）→ deny
- `git restore` / `git checkout --` 目标是 `openspec/changes/**` → deny；源码目标放行

---

## 4. 挂载点设计（两张候选图）

### 候选 A：挂在 loop 分发链（推荐基线）

```
sdk/loop.rs  ③ sandbox 之后、④ 执行之前
   write/edit → path 已是 canonical 绝对路径（sandbox 改写后）
   bash       → command 原文（不可 canonicalize，只能文本提取）
   deny → 复用 deny(sender, <subtype>, name, reason, id)
```

### 候选 B：挂到 tools::execute 内部

不推荐：`tools::execute(root, name, input)` 是纯函数且被 explore_note 双轨分流，塞入"读配置"会让签名长出 IO 依赖；而且拒绝形态（SystemNotice）只能在 loop 发。

```
                       ┌─ create_explore_note（专用工具，不经 tools::execute）
loop ④ 执行 ──────────┤      ↑ 若用户把 openspec/explores/** 加进保护，此路不被拦 ← 一致性缺口
                       └─ tools::execute（七通用工具）
```

---

## 5. 复刻差距表

| 插件端要素 | 桌面端处置 | 依据 |
|---|---|---|
| 内置 `**/openspec/config.json` | **照搬** | 唯一还活着的内置锁定 |
| 内置 `**/openspec/changes/**/workflow.json` | **不复刻**（已定稿） | `core/workflow` 双向墙：DB 即状态，「MUST NOT 产出 / 零读取」，legacy 文件惰性不解析（`list_test.rs:479`「workflow.json 惰性字节零进投影」）|
| 内置 `eval.json` | 不涉及 | 已在插件端退役（`eval-json-protection`） |
| 用户 `write_protection.files[]` | **照抄语义**（相对 glob 加 `**/`、绝对原样、reason 占位符 %s/%t） | 同一份 config，两边必须同解 |
| Write/Edit → file_path | → `write`/`edit` 的 `path` | 工具名不同，语义一致 |
| Bash/PowerShell/Shell 三入口 | → `bash` 单入口 | 桌面端只有 bash 工具；Windows 底座 git-bash / `cmd /C` 兜底（`bash.rs`）|
| `extractFileOps` 三态提取器 | **需 Rust 重译**（约 400 行 TS → 引号/heredoc 掩码 + 4 类正则族 + git 子命令） | 核心工作量所在 |
| `checkBulkRevert` | 需重译（`git stash`/`git clean`/`git restore openspec/changes/**`） | 面向"hook 不可见的批量改写"，桌面端同样不可见 |
| python/node 豁免 | **不照搬**：裸脚本放行，`-e`/`-c` 明文改判别（已定稿，见 §6.4） | 插件端整体放行是已知旁路，已实测 |
| fail-open（非法 stdin → allow） | 对应语义 = config 解析失败只留内置规则 | `config::load` 已保证 |
| picomatch `matchGlob` | 复用 `checks::globmatch::match_glob`（glob crate + 花括号展开，已覆盖 picomatch 主方言） | 见 D7 |

---

## 6. 决议（已定稿）

| # | 问题 | 决议 | 落点 |
|---|---|---|---|
| D1 | 配置读哪个 root | **恒读主仓**（workspace root），非 worktree | 通道见 §6.1 |
| D2 | 何时读 config | 按「config 运行期不变」处理，**取维护性最优**：每 run 装配一次，规则随 `LoopTurn` 下发 | §6.2 |
| D3 | 检查点在 sandbox 前/后 | **后**（canonical 绝对路径） | `loop.rs:387` 之后 |
| D4 | 内置集合收哪些 | **只 `**/openspec/config.json`**；`workflow.json` 不复刻 | 用户确认 |
| D5 | bash 提取器颗粒度 | 复刻三态提取 + bulk revert；**node/python 不做整体豁免，改明文脚本判别** | §6.3 |
| D6 | 拒绝文案 | **面向 Agent 的处置指令**（非人类说明）：告知文件不可改 → 调整方案 → 无法规避则中断并通知用户；subtype 复用 `permission_denied` | §6.6 |
| D7 | glob 函数位置 | 复用 `checks::globmatch::match_glob`（方言已验，见 §7） | 依赖向待选 |
| D8 | 复刻漏面边界 | **档 B**：照译 + 补 `cp`/`dd of=`/`sed -i`/`truncate`/`find -delete` | §6.5 |

专属写入工具（`create_explore_note`）**明确不拦**——用户决策。代价：用户若把 `openspec/explores/**` 写进 `write_protection`，`edit` 被拦而 `create_explore_note` 直通，一致性缺口保留（记录在案，不再视为待办）。

### 6.1 D1 落地：多一条 builder 缝（有现成先例）

`LoopTurn`（`loop.rs:75`）只有 `cwd`。但 `SdkRunner` 已有**同构先例**：`with_explore_note_store(store)` → `SdkRunner.explore_store` → `PumpPayload` → pump 内 per-turn 铸造 `ExploreNoteContext`。protect 沿同一条缝：

```
命令层（已显式持 root）
  exec/mod.rs:132          compose_turn(stores, registry, streams, store, agent)
  change_flow/mod.rs:143   compose_turn(stores, registry, streams, store, None)
        │                                    ↑ 两处都向上有 `root` 在手
        ▼
  compose_turn(..., root)                     ← 新增一参（2 调用点 + 测试）
        ▼
  EngineFacade::with_workspace_root(root)     ← 与 with_explore_note_store 并列
        ▼
  SdkRunner.workspace_root: Option<PathBuf>
        ▼
  PumpPayload.workspace_root → LoopTurn.protect: ProtectRules
```

顺带说明为何 store 句柄本身不能替代：`Store`（`store.rs:409`）只有 `db` + `dimension`，**没有 root 字段**；root 是 `workspace_db_path(canonical_root)` 的输入而非结构成员。所以必须走命令层显式传参。

### 6.2 D2 落地：装配点读一次（loop 内纯函数）

```
session_pump（每 run 一次，天然有 root + 每轮一命）
   let protect = protect::load(workspace_root);   ← 唯一 IO 点：config::load + 规则合并 + glob 预编译
        ▼ 随 LoopTurn 下发
loop 内每 tool_call：protect::evaluate(name, input) → Option<reason>   ← 纯函数，零 IO
```

三点理由：
1. **可测性**：规则装配与判定分离，`evaluate` 可密集单测，不必每次造 fs
2. **与既有结构同构**：`defense`（上下文防线）、`retry`、`liveness` 全是"装配点算好、loop 纯消费"的形态
3. **新鲜度足够**：泵一轮一命（`runner.rs:335` 注释），change 流程每相位一轮 run ⇒ 等价"每相位重读"

代价：run 内用户改 config 不生效。与用户「config 不会改变」的判定一致。

### 6.3 D5 落地：bash 判断的两层

```
bash 命令文本（原文，不可 canonicalize）
  │
  ├─ 第一层：三态提取（照译插件端 extractFileOps）
  │    write / delete / move / git 子命令 → 路径 → ∩ 保护 glob → deny
  │    ※ 引号+heredoc 双变体掩码是这层精度的全部来源
  │
  └─ 第二层：node/python 明文脚本（插件端没有，桌面端新增）
       裸脚本文件（node x.js / python x.py）→ 放行（承认不可静态判别）
       -e / --eval / -c / --command 明文 → 脚本内容内「写 API + 路径」关联 → deny
       详见 §6.4
```

批量还原拦截（`git stash` / `git clean` / `git restore openspec/changes/**`）随第一层一起复刻。

### 6.4 D5 展开：node/python 明文脚本判别（新增能力）

**先确认旁路真实存在**——用插件端产物实测（`dist/.../hooks.cjs protect-files`，worktree 内含 `openspec/changes/x`）：

| 命令 | 插件端判定 | 说明 |
|---|---|---|
| `node -e "require('fs').writeFileSync('openspec/config.json','{}')"` | **allow** | 直穿内置保护 |
| `node -e "require('fs').unlinkSync('openspec/changes/x/workflow.json')"` | **allow** | 直穿 |
| `python -c "open('openspec/config.json','w').write('{}')"` | **allow** | 直穿 |
| `node scripts/setup.js openspec/config.json` | **allow** | 裸脚本（不可判别） |
| `echo {} > openspec/config.json` | deny | 重定向族 |
| `rm openspec/config.json` | deny | 删除族 |

原因很直接：`checkShellCommand:230` 对 `^(python|python3|node)\s` **整体 return null**，连提取器都不进。而即便去掉豁免，第一层提取器也认不出 `writeFileSync(...)`——它的 op 只来自重定向 / 裸命令名。

**判别设计：关联式优先，不做全文共现**

| 档 | 规则 | 精度 | 漏面 |
|---|---|---|---|
| A 关联式（建议） | 写 API 名 + 其路径实参绑定匹配 | 高 | 变量中转 `const p='…'; writeFileSync(p,…)` |
| B 共现式 | 脚本段内同时出现写 API 名与受保护路径字符串 | 低（`console.log('.../config.json','writeFileSync')` 误拦） | 少 |
| C 整体放行（现状） | — | — | 全部 |

A 档的 API 表（每项一条正则，路径取第一个字符串实参）：

```
JS   writeFileSync|writeFile|appendFileSync|appendFile|createWriteStream
     copyFileSync|copyFile|renameSync|rename|unlinkSync|unlink|rmSync|rm
     fs.promises.writeFile / openSync(p,'w'|'a'|'wx')
PY   open(p,'w'|'a'|'x'|'wb'|'w+')   Path(p).write_text|write_bytes
     shutil.copy|copy2|copyfile|move   os.remove|unlink|rename|replace
```

**嵌套形态同样覆盖**（`bash -c 'node -e "…"'`、`pwsh -Command "…"`）——因为关联正则是对整条命令文本跑的，不依赖"行首是 node"。这点比重定向族更宽松（`>` 正则本来就全文扫描）。

**仍承认的漏面**（与用户判定一致，不做）：生成脚本文件再执行、`node -m` / `python -m` 模块、变量中转路径、base64 编码脚本、`printf` 拼字符串再喂 stdin。

**一处必须先定的坑**：`-c` 在 node 与 python 里语义不同——`node -c` 是 `--check`（语法检查，不写文件），`python -c` 才是执行命令。若按"flag 白名单"切脚本段，node 侧只能认 `-e` / `--eval`。而 A 档关联式不切脚本段（对全文跑），这个坑自动消失——这也是倾向 A 档的一个理由。

### 6.5 D8 落地：复刻边界取档 B

"复刻"有个绕不开的歧义：**连漏面一起复刻，还是顺手补一档？** 实测插件端产物（`dist/.../hooks.cjs protect-files`，worktree 含 `openspec/`）：

| 命令 | 插件端 | 为何漏 |
|---|---|---|
| `cp /tmp/x openspec/config.json` | **allow** | `extractFileOps` 无 `cp` 族 |
| `dd if=/tmp/x of=openspec/config.json` | **allow** | 无 `dd`（`of=` 是明确写入方） |
| `sed -i s/a/b/ openspec/config.json` | **allow** | 无 `sed -i`（原地改写） |
| `truncate -s 0 openspec/config.json` | **allow** | 无 `truncate`（清空即毁） |
| `find openspec -delete` | **allow** | 无 `find -delete` |
| `xargs rm < list.txt` | **allow** | `rm` 参数来自 stdin，文本不可知 |

**决议：档 B**——照译插件端三态提取 + 补下面五项。`xargs` / `sh -c` 递归拆解**不做**（另一个量级，且与"生成脚本再执行不可防"同属承认边界）。

| 补充项 | 判据 | 已知粗粒度代价 |
|---|---|---|
| `cp` / `Copy-Item` | 目标位（末参或多源的最后一段）→ write | 需区分 src/dst 方向；单参形态不可判 |
| `dd` | `of=<path>` → write | `if=` 是源，须显式排除 |
| `sed -i` / `perl -i` | 末参路径 → write | 无 `-i` 是纯读，不得误拦 |
| `truncate` | `-s` 之后的路径 → write | `-r` 引用形态不覆盖 |
| `find … -delete` | 搜索根的**第一个非 flag 参数** → delete | 匹配项不可静态预知，等价"保护整个搜索根"——删除场景该保守 |

**复刻时顺手要避的一个"优化陷阱"**：插件端 `>` 正则的前缀排除集是 `[^-=]`（只排 `-` 与 `=`），`echo {} &> openspec/config.json` 能 deny **纯属这个集合没排 `&`**：

```
实测：echo {} >| openspec/config.json   → deny
      echo {} >> openspec/config.json   → deny
      echo {} &> openspec/config.json   → deny   ← 靠排除集不完整才成立
      cat <<EOF > openspec/config.json  → deny   ← heredoc 掩码正确，redirect 在头部
      echo "rm -rf /" > notes.txt       → allow  ← 引号内 rm 不算命令
```

若复刻时"顺手把排除集补全成 `[^-=&|]`"，`&>` 会静默从 deny 变 allow 而无人察觉。**如实照抄，别优化。**

### 6.6 D6 落地：拒绝串是给 Agent 的指令，不是给人看的说明

**机制前提已确认**：`deny()`（`loop.rs:447`）把 `reason` **同时**写进两处——`SystemNotice.payload.reason`（前端 notice 展示）与 `ToolResult.content`（`is_error=true`，**模型可见**）。所以这条串的首要读者是模型，次要读者是用户，措辞必须同时可读。

**关键设计压力**：`deny` **不中断 run**（注释明写「run 不中断」）。模型被拒后的第一反应是**换路重试**：

```
write openspec/config.json        → deny
  ↓ 模型换工具
bash: echo {} > openspec/config.json  → deny
  ↓ 再换
bash: cp /tmp/x openspec/config.json  → deny
  ↓ 再换
bash: node -e "fs.writeFileSync(...)" → deny
  ↓ ...
```

同一文件上可能连撞四五个拒绝，白烧轮次。**文案必须显式掐掉这条链**——这正是「无法规避就中断并通知用户」这句要承担的功能，不是客套话。

**拟定文案（内置 `config.json` 规则）**：

```
该文件受写入保护：{path}（detected via {tool}）。
openspec/config.json 是工作区配置权威，Agent 不得改写：
如需调整配置，请在回复中说明「要改哪个字段、改成什么、为什么」，由用户自行操作。
若任务无法在不改该文件的前提下完成，请立即停止并通知用户。
不要改用其他命令或工具尝试绕过——所有写入路径均被拦截。
```

**用户规则**：`write_protection.files[].reason` **原样透传**（同一份 config 双端同解），保留 `%s`（路径）/ `%t`（工具名）占位符替换。用户没写 reason 时回落通用串：

```
该文件受写入保护：{path}（detected via {tool}）。
请调整方案避免修改该文件；如无法规避，立即停止并通知用户，不要改用其他命令绕过。
```

**与插件端文案的差异**：插件端 `config.json` 内置串是「请提供修改方案，通知用户自行操作」——**没有"不得绕过""无法规避就停"两句**，因为 Claude Code 有审批通道可承接。桌面端无审批通道（policy 是三档纯函数），所以这两句是必需的。

**边界**：「中断」= 模型结束本轮（`finish_success`），无引擎新机制；change 流程的相位会话停手后如何收敛归编排层（verdict 由 core 判定），protect 模块不越界。

---

## 7. 坑清单（含已实测结论）

- ✅ **`**/` 零段可命中**：picomatch 与 Rust `glob` 均成立（前者实测 `openspec/config.json` ← `**/openspec/config.json` = true；后者源码 `matches_from` 有 "Empty match" 分支）。
- ✅ **Windows 盘符不挡路**：`D:/ws/openspec/config.json` ← `**/openspec/config.json` = true（两方言皆真）。`match_glob` 已做 `to_forward_slash` 归一。
- ⚠️ **`**` 只能占满整段**：`glob` crate 里 `a**/b`、`a/**b` 直接 `PatternError`（`glob-0.3.4/src/lib.rs:655`），picomatch 更宽松。用户 glob 若写 `openspec/**/config.json` 之外的怪形态，两边可能分叉 → `match_glob` 已按"非法即不命中"收敛（fail-closed 于匹配、fail-open 于防护）。
- ⚠️ **无通配符 glob 走目录前缀语义**：`match_glob` 对无通配模式匹配"相等或 `pattern/` 起手"。所以用户写 `openspec/config.json` 时必须先加 `**/`（插件端同款处理），否则只命中 root 相对形态。
- ⚠️ **`2>&1` 必须跳过**：`>&` 正则不加 `(?!\d+)` 前瞻会把 fd 复制当成写文件。
- ⚠️ **`->` / `=>` 干扰**：`echo a->b`、JS 箭头函数都曾误判为写目标，前缀守卫 `[^-=]` 是必要护栏。
- ⚠️ **heredoc 内的 `>` 是数据不是命令**：必须掩码，否则 `cat <<EOF\n> openspec/config.json\nEOF` 误拦。
- ⚠️ **引号内的 `rm` 不是命令**：命令名定位必须在"引号已掩码"变体上做，参数切片要在"保留引号"变体上做（两变体长度相等、索引互通）。
- ⚠️ **保护是硬拒绝，无审批通道**：桌面端 policy 是纯函数三档，没有 interactive approval。拦了就是拦了，模型只能通知用户手动改 → phase agent 自动跑时可能出现"重试—再被拦"的额外轮次（可接受，但不该在同一文件上死循环）。
- ⚠️ **over-record 方向安全、over-block 方向不安全**：插件端 PostToolUse 记录器容忍多记（有对账），但 PreToolUse 保护多拦就是误伤。裁剪提取器时宁可漏 stdout 形态，也别把 `echo foo > notes.txt` 判成受保护。

---

## 8. 一句话结论

决策已定稿，形状收敛为：**一个 `protect` 模块（规则装配纯函数 + 判定纯函数）+ 一条 builder 缝（`with_workspace_root`，与 `with_explore_note_store` 同构）+ 一份插件端 shell 提取器的 Rust 重译**。

真正的工作量分布（非"读配置"）：

```
~10%  配置读入（config::load 已现成，只需并上内置 glob）
~15%  builder 缝 + loop 挂点（有先例，改动面清楚）
~45%  extractFileOps 重译（掩码算法 + 4 类正则族 + git 子命令 + bulk revert）
~20%  node/python 明文脚本判别（桌面端新增，插件端无参考）
~10%  测试面（方言对照 / 掩码边界 / 误拦-漏拦双向断言）
```

两个"看起来是设计问题、其实已不是"的点：D2（读配置时机）与 D3（检查点位置）都已定，落地形态由既有结构直接决定。剩下唯一需要你点头的是 **D6 拒绝文案**（沿用插件端「请提供修改方案，通知用户自行操作」还是重写）。
