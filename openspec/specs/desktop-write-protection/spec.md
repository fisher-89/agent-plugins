# desktop-write-protection Specification

## Purpose

定义桌面端「Agent 不得写入受保护文件」能力：保护规则的装配（内置锁定 + 工作区 config 的用户规则）、write / edit 的路径判定、bash 命令写入面提取（三态 + 档 B 补充 + 批量还原拦截）、node / python 内联脚本判别，以及面向 Agent 的拒绝指令文案。判定的挂载点（loop 守卫链位置与工作区根注入缝）归 desktop-agent-execution；本能力只管「规则从哪来」与「拿到一条工具调用怎么判」。

## Requirements

### Requirement: 保护规则装配（内置锁定 + 用户规则合并）

保护规则 SHALL 由两部分合并而成，装配为单点纯数据结构 `ProtectRules`：

- **内置锁定（独立于配置解析结果）**：SHALL 只收一条 `**/openspec/config.json`。该 glob MUST 由 `foundation::layout::config_path` 派生后加 `**/` 前缀，产品源码 MUST NOT 出现 `openspec` 目录名字面量或 `config.json` 文件名字面量（沿 desktop-crate-layout 两条隔离禁令）。内置规则 MUST NOT 依赖 `config::load` 是否成功——工作区配置缺失、坏 JSON、字段违例时内置面 SHALL 照常生效。`**/openspec/changes/**/workflow.json` MUST NOT 纳入内置锁定：工作流状态以库为唯一权威，legacy 文件零读取零产出，锁定它无对象可保护。该规则的存在使**该文件成为保护面的第一道拦截**（`protect/**` 内无 override 或跳过开关）——仓库面若需更新该文件内的指针，SHALL 以值级替换交付并复核（不变量见 `repo-surface-scope`），MUST NOT 为改写指针而改动本条内置锁定语义或给规则加 override。
- **用户规则**：SHALL 取自 `config::load(workspace_root).config.write_protection.files[]`（`config::load` 永不失败，恒返回合法配置 + diagnostics）。逐条规则：`glob` 为 `None`（config 组装层已剔除空串 / 非法值）SHALL 跳过；`glob` 为绝对路径 SHALL 原样使用；`glob` 为相对路径 SHALL 前缀 `**/`（无通配模式在匹配面走「相等或 `pattern/` 起手」语义，不加前缀只命中 root 相对形态）。`reason` SHALL 原样保留（含 `%s` / `%t` 占位符，替换见「拒绝指令文案」）。
- **规则序**：内置规则 SHALL 排在用户规则之前；首个命中规则决定拒绝串（内置 `config.json` 规则恒优先，即使命中的是用户配置里另一条更宽规则）。同一次判定 MUST NOT 拼接多条 reason。
- **方言单点**：规则匹配 SHALL 一律经 `checks::globmatch::match_glob`（`**/` 零段可命中、Windows 盘符归一、无通配模式走目录前缀、花括号交替已覆盖）；`agent-runtime` MUST NOT 自造第二份 glob 匹配实现。非法 / 超限模式 SHALL 按不命中收敛（不 panic、不拒绝启动）。
- **装配时机与唯一 IO 点**：规则装配 SHALL 为每 run 一次（会话泵起播时），是 protect 模块的唯一 IO 点（`config::load` + 规则合并）；loop 内每次判定 MUST NOT 触碰文件系统（run 内改 config 不改变本 run 行为）。
- **工作区根缺席**：装配入参为 `None`（未注入 workspace root）时 SHALL 只产出内置规则（用户规则缺席）——MUST NOT 产出「保护关闭」的空规则集。

#### Scenario: 内置规则独立于配置解析

- **WHEN** 工作区无 `openspec/config.json`，或该文件为非法 JSON，或 `write_protection` 字段类型违例
- **THEN** 装配结果含内置 `**/openspec/config.json` 一条，write / edit 指向该文件仍被拒；`config::load` 的 diagnostics 不参与保护面判定

#### Scenario: 相对 glob 自动前缀

- **WHEN** 用户配置 `write_protection.files = [{ glob: "openspec/changes/**/proposal.md" }]`，模型在 `<root>/openspec/changes/x/proposal.md` 上调用 write
- **THEN** 判定命中（相对 glob 加 `**/` 后可匹配绝对路径），拒绝串为通用回落串

#### Scenario: 绝对 glob 原样

- **WHEN** 用户配置绝对 glob `D:/ws/secret.md`（或 `/home/u/secret.md`），模型写入该绝对路径
- **THEN** 判定命中（绝对 glob 不加 `**/` 前缀，原样匹配）

#### Scenario: 内置优先于用户规则

- **WHEN** 用户同时配置 `{ glob: "openspec/**", reason: "自定义原因" }`，模型写 `<root>/openspec/config.json`
- **THEN** 拒绝串取内置 `config.json` 规则的原因（内置先行，首个命中即定），自定义 reason 不被拼接

#### Scenario: 工作区根缺席仅内置

- **WHEN** 装配入参为 `None`（root 未注入）
- **THEN** 规则集只含内置一条；写 `openspec/config.json` 仍被拒，用户配置的任何 glob 均不生效

#### Scenario: 规则面单源自述

- **WHEN** 审查 `ProtectRules` 装配逻辑的注释与 spec 措辞
- **THEN** 每条规则语义按本 spec 直述（内置面 / 用户面 / 序 / 方言 / 时机 / 根缺席六条）
- **AND** 无「与插件端同款处理 / 插件端此处放行」类对照表述（插件端已不存在，语义不依赖对照读者）

### Requirement: write / edit 路径保护判定

`ProtectRules::evaluate(tool, input, cwd)` SHALL 对 `write` / `edit` 工具按路径判定，返回 `Option<String>`（命中即拒绝串，未命中即 `None`）：

- 路径字段 SHALL 取 `path`（与工具入参字段一致）。检查点位于路径沙箱改写之后，故此处 `path` 通常是 canonical 绝对路径；判定前 SHALL 做归一（剥 Windows 扩展长度前缀 `\\?\`、`\` → `/`），MUST NOT 再次 canonicalize（判定 MUST 零 IO）。
- 命中 SHALL 产出一条拒绝串并返回；MUST NOT 在此处发事件、写盘或产生任何副作用（`SystemNotice` / ToolResult 由 loop 侧统一合成）。
- **射程边界**：`read` / `grep` / `glob` / `ls` 四只读工具 MUST NOT 被写入保护拦截（保护面只覆盖写，读面全开放）；专用工具 `create_explore_note` MUST NOT 被拦截（其目标目录属探索笔记，写入语义与 `write` / `edit` 不同源——一致性缺口为既定裁定的知情边界：用户若把 `openspec/explores/**` 写进保护，`edit` 被拒而该专用工具直通）。
- **入参非法 fail-open**：`path` 缺失或非字符串时 SHALL 放行（执行体自会以「入参缺失或非字符串字段」报错，保护面不抢错误归属）。

#### Scenario: write 命中拒绝而读面放行

- **WHEN** 模型对 `<root>/openspec/config.json` 分别调用 `write`、`edit` 与 `read`、`grep`
- **THEN** `write` / `edit` 返回拒绝串（`Some`），`read` / `grep` 返回 `None`（放行）

#### Scenario: Windows 前缀与反斜杠归一

- **WHEN** 沙箱改写后的路径形态为 `\\?\D:\ws\openspec\config.json`（或含 `\` 分隔符）
- **THEN** 归一后与 `**/openspec/config.json` 命中，判定为拒（不得因前缀或分隔符漏判）

#### Scenario: 专用工具不在射程

- **WHEN** 用户在配置中保护 `openspec/explores/**`，模型调用 `create_explore_note`
- **THEN** 判定返回 `None`（该专用工具直通），同目录的 `write` / `edit` 调用仍被拒——缺口作为知情边界留痕

#### Scenario: 判定零副作用

- **WHEN** 在无文件系统访问权限的环境（或以只读 tempdir）驱动 `evaluate`
- **THEN** 判定正常返回结果，无 IO 错误、无事件流出（判定与事件合成解耦）

### Requirement: shell 命令写入面提取（掩码 + 三态）

对 `bash` 工具的 `command` 原文（不可 canonicalize，只能文本提取），`evaluate` SHALL 提取写入 / 删除类文件操作并与保护规则求交：

- **双变体等长掩码（精度唯一来源）**：SHALL 提供两个掩码变体——变体 A（只掩 heredoc 体，保留引号）用于跑写入正则与参数切片（引号内的真实目标仍可匹配）；变体 B（掩 heredoc 体 + 引号串）用于定位命令名（字符串 / heredoc 数据里的 `rm`、`git stash` 不得被当作命令）。两变体 MUST 等长（掩码字符逐字符替换），索引 1:1 互通。
- **三态操作**：SHALL 提取 write / delete / revert 三态（revert 不入保护面，见批量还原拦截）：

| 态 | 来源 |
|---|---|
| write | `>`、`>>`（前缀守卫 `[^-=]`，跳过 `->` 与 `=>`）、`>\|`、`tee`（含 `-a` 等 flag）、`>&`（`(?!\d+)` 前瞻跳过 `2>&1` / `1>&2` 类 fd 复制）；PowerShell 重定向 `*>` / `N>`、cmdlet `Set-Content` / `Out-File` / `Add-Content` / `Export-Csv` / `Export-CliXml` / `Tee-Object`（可带 `-Path` / `-FilePath`）、`[System.IO.File]::WriteAllText|WriteAllLines|WriteAllBytes|AppendAllText`；`git restore --source=<ref>` / `-s <ref>`、`git checkout <ref> -- <paths>` |
| delete | `rm`、`rmdir`、`unlink`、`del`、`rd`、`Remove-Item`、`git rm`（逐参数，跳过 `-` flag） |
| write + delete | 双参数 `mv`、`Move-Item`、`ren`、`Rename-Item`（旧路径记 delete、新路径记 write） |
| revert | `git restore <paths>`（无 `--source`）、`git checkout -- <paths>`——不入保护判定，是规范撤销动作 |

- **token 纪律**：flag（`-` 起手）SHALL 跳过；未展开变量 token（含 `$`）SHALL 在操作落点丢弃（执行前不可知，字面 token 永远命不中真实路径）。
- **路径解析**：提取出的路径为绝对形态 SHALL 归一后直接匹配（剥 `\\?\`、`\` → `/`）；相对形态 SHALL 拼会话执行根 `<cwd>/<path>` 后匹配；解析失败（空串 / 无可用祖先）SHALL 跳过该操作。
- **判定**：任一 write / delete 操作命中规则即 SHALL 返回该规则的拒绝串（首个命中即止，不遍历剩余操作）。
- **精度纪律（MUST NOT"顺手优化"）**：`>` 前缀排除集 SHALL 如实保持 `[^-=]`——补全成 `[^-=&|]` 会让 `echo {} &> <protected>` 从拒静默变放行。over-block 方向不安全：宁可漏掉 stdout 形态，也 MUST NOT 让普通写入（`echo foo > notes.txt`）被误拦；over-record 方向的宽容（多提取）在本能力不适用。
- **性能纪律**：所有正则 SHALL 为单量词线性模式，MUST NOT 引入嵌套量词 / 回溯爆炸形态；MUST NOT 以「命令文本长度上限」换取性能（设上限等于送出一条绕过路径）。

#### Scenario: 重定向族命中

- **WHEN** 命令为 `echo {} > openspec/config.json`、`cat x >> <root>/openspec/config.json`、`echo {} >| openspec/config.json`、`echo {} | tee openspec/config.json`、`echo {} &> openspec/config.json`
- **THEN** 五种写法均被拒（`&>` 是照抄排除集 `[^-=]` 的既有结论，MUST NOT 反向"修复"）

#### Scenario: 删除族与移动族命中

- **WHEN** 命令为 `rm -f openspec/config.json`、`git rm openspec/config.json`、`Remove-Item openspec/config.json`、`mv openspec/config.json /tmp/x`
- **THEN** 前三者按 delete 命中被拒；`mv` 按「旧路径 delete + 新路径 write」双记，旧路径命中即拒

#### Scenario: fd 复制与箭头不误判

- **WHEN** 命令为 `cargo test 2>&1`、`cargo test 1>&2`、`echo a->b`、`awk 'NR=>2' x`、`echo "x => y" > notes.txt`
- **THEN** 前两者不产生 write 操作（fd 复制非文件）、`->` / `=>` 不产生 write 目标、最后一条只提取 `notes.txt` 且不命中保护（放行）

#### Scenario: heredoc 与引号内的"写入"是数据

- **WHEN** 命令为 `cat <<EOF > notes.txt\n> openspec/config.json\nEOF`，或 `echo "rm -rf /" > notes.txt`
- **THEN** heredoc 体里的 `>` 不产生 write 操作、引号内的 `rm` 不被当作命令；两条命令均放行（不因数据文本误拦）

#### Scenario: 未展开变量丢弃

- **WHEN** 命令为 `echo x > $OUT`、`rm $TARGET`、`cp $SRC openspec/config.json`
- **THEN** `$OUT` / `$TARGET` 在操作落点被丢弃（不参与匹配）；`cp` 的目标位命中保护仍被拒

#### Scenario: 相对路径按执行根解析

- **WHEN** 会话执行根为 `<root>`，命令为 `rm openspec/config.json`
- **THEN** 提取路径 `openspec/config.json` 拼为 `<root>/openspec/config.json` 后命中内置规则被拒

### Requirement: shell 提取的档 B 补充族

在「shell 命令写入面提取（掩码 + 三态）」的 write / delete / revert 三态提取之外，SHALL 补充以下五族（档 B 边界）：

| 族 | 判据 | 已知粗粒度代价 |
|---|---|---|
| `cp` / `Copy-Item` | 目标是**末参**（多源时取最后一段）→ write | 需区分 src / dst 方向；单参数形态不可判（跳过） |
| `dd` | `of=<path>` → write | `if=` 是源，MUST 显式排除 |
| `sed -i` / `perl -i` | 末参路径 → write | 无 `-i` 是纯读，MUST NOT 拦 |
| `truncate` | `-s` 之后的路径 → write | `-r` 引用形态不覆盖 |
| `find … -delete` | 搜索根（第一个非 flag 参数）→ delete | 匹配项不可静态预知，等价"保护整个搜索根"；删除场景该保守 |

遍历与递归拆解（`xargs` / `sh -c '<cmd>'`）MUST NOT 实现——与「生成脚本再执行不可静态判别」同属承认边界（写进知情边界条款）。

#### Scenario: cp 目标位命中

- **WHEN** 命令为 `cp /tmp/x openspec/config.json`、`cp /tmp/a /tmp/b openspec/config.json`、`Copy-Item C:\tmp\x openspec\config.json`
- **THEN** 三者均按目标位（末参）命中被拒

#### Scenario: dd 的 if 不误拦而 of 拦截

- **WHEN** 命令为 `dd if=openspec/config.json of=/tmp/backup.json` 与 `dd if=/tmp/x of=openspec/config.json`
- **THEN** 前者放行（`if=` 是读源）、后者被拒（`of=` 是写目标）

#### Scenario: 原地改写与纯读分界

- **WHEN** 命令为 `sed -i s/a/b/ openspec/config.json` 与 `sed -n 1p openspec/config.json`、`perl -i -pe s/a/b/ openspec/config.json`
- **THEN** 带 `-i` 的两条被拒，不带 `-i` 的一条放行（纯读不得误拦）

#### Scenario: 清空即毁与递归删除

- **WHEN** 命令为 `truncate -s 0 openspec/config.json` 与 `find . -name '*.json' -delete`
- **THEN** 前者按 `-s` 后的路径被拒；后者按搜索根 `.` 被拒（`find` 的匹配项不可静态预知，删除场景按"保护整个搜索根"保守处理）

#### Scenario: 档 B 五族语义单源

- **WHEN** 审查档 B 五族的判据与代价注记
- **THEN** 每族判据、已知粗粒度代价与本 spec 表逐项一致（不复刻任何外部实现的提取面）
- **AND** 遍历 / 递归拆解仍属承认边界（MUST NOT 实现）

### Requirement: 批量还原拦截

对「对文件清单记录器不可见、且一次性改写工作区」的 git 批量动作，SHALL 独立于用户配置规则拦截（保护面之外的固定规则）：

- `git stash`（任何子命令，除 `list` / `show`）→ 拒（会批量改写工作区）；
- `git clean`：无路径参数 → 拒；有路径参数且任一命中 `**/openspec/**` → 拒；带 `-n` / `--dry-run` → 放行（预览形态）；
- `git restore` / `git checkout --`：目标命中 `**/openspec/changes/**` → 拒（工作流产物是批量还原的高危面）；源码目标 → 放行（规范撤销动作）。

命令名定位 SHALL 在「引号 + heredoc 已掩码」的变体上做、参数切片 SHALL 在「引号保留」的变体上做（同 shell 提取的双变体纪律）。批量还原拒绝串 SHALL 为独立文案（说明批量动作不可见 + 引导逐路径 `git restore <path>`）。

#### Scenario: git stash 拦截而查看放行

- **WHEN** 命令为 `git stash`、`git stash push -m wip`、`git stash pop` 与 `git stash list`、`git stash show`
- **THEN** 前三者被拒（批量改写），后两者放行（只读查看）

#### Scenario: git clean 三态

- **WHEN** 命令为 `git clean -fd`（无路径）、`git clean -fd openspec/`、`git clean -n`、`git clean -fd src/`
- **THEN** 前两者被拒（无路径 = 全仓；参数命中 `openspec/**`），`-n` 放行（dry-run），`src/` 放行

#### Scenario: 工作流产物还原拦截而源码放行

- **WHEN** 命令为 `git restore openspec/changes/x/proposal.md` 与 `git restore src/a.rs`
- **THEN** 前者被拒（工作流产物），后者放行（源码路径的撤销是规范动作）

#### Scenario: 引号内的 git 不算命令

- **WHEN** 命令为 `echo "git stash" > notes.txt`
- **THEN** 不触发批量还原拦截（命令名在引号变体 B 上定位，字符串数据不构成命令）

### Requirement: node / python 内联脚本写入判别

对以 `node` / `python` / `python3` 起手或嵌套出现的脚本调用，SHALL 以**关联式**判别写入（MUST NOT 采用 `^(python|python3|node)\s` 的整体豁免旁路）：

- **判据**：写 API 名与其**路径实参**绑定的正则逐条对整条命令文本扫描，命中受保护路径即拒。MUST NOT 采用「全文共现式」（脚本段内同时出现写 API 名与受保护路径字符串即拒——会把日志 / 文档文本误拦）。
- **API 表**（每项一条正则，路径取第一个字符串实参）：

| 语言 | API |
|---|---|
| JS / Node | `writeFileSync` / `writeFile` / `appendFileSync` / `appendFile` / `createWriteStream` / `copyFileSync` / `copyFile` / `renameSync` / `rename` / `unlinkSync` / `unlink` / `rmSync` / `rm` / `fs.promises.writeFile` / `openSync(p, 'w' \| 'a' \| 'wx')` |
| Python | `open(p, 'w' \| 'a' \| 'x' \| 'wb' \| 'w+')` / `Path(p).write_text` / `Path(p).write_bytes` / `shutil.copy` · `copy2` · `copyfile` / `shutil.move` / `os.remove` · `unlink` · `rename` · `replace` |

- **不切脚本段**：判别对整条命令文本跑（不识别 `-e` / `--eval` / `-c` / `--command` 的脚本边界）。因此 `node -c`（`--check`，只做语法检查）不构成误判来源，`python -c`（执行命令）亦无需特判；嵌套形态（`bash -c 'node -e "…"'`、`pwsh -Command "…"`）同样被覆盖（关联正则不依赖"行首是 node"）。
- **裸脚本放行**：`node <file>.js` / `python <file>.py` / `node -m` / `python -m` 等以脚本或模块文件为入口的形态 SHALL 放行（脚本内容不在命令行文本内，不可静态判别）——该放行 MUST NOT 扩展到同一条命令里的重定向 / `cp` / 删除族（那些仍按前述族判定）。
- **已知漏面（知情边界）**：生成脚本文件再执行、变量中转路径（`const p='…'; writeFileSync(p,…)`）、base64 编码脚本、`printf` 拼串后喂 stdin——均不覆盖。

#### Scenario: node 内联写命中

- **WHEN** 命令为 `node -e "require('fs').writeFileSync('openspec/config.json','{}')"`
- **THEN** 被拒（整体豁免旁路 MUST NOT 存在——`node` / `python` 起手的脚本调用同样落判定）

#### Scenario: python 内联写命中

- **WHEN** 命令为 `python -c "open('openspec/config.json','w').write('{}')"` / `python3 -c "from pathlib import Path; Path('openspec/config.json').write_text('{}')"`
- **THEN** 两条均被拒

#### Scenario: 嵌套形态覆盖

- **WHEN** 命令为 `bash -c 'node -e "require(\"fs\").rmSync(\"openspec/config.json\")"'`
- **THEN** 被拒（关联式对整条文本扫描，不依赖脚本段定位）

#### Scenario: 裸脚本放行

- **WHEN** 命令为 `node scripts/setup.js openspec/config.json`、`python tools/gen.py`
- **THEN** 两条均放行（脚本内容不可静态判别，作为已知漏面留痕），不因参数里出现受保护路径而拒

#### Scenario: 共现不误拦

- **WHEN** 命令为 `node -e "console.log('writeFileSync','openspec/config.json')"`
- **THEN** 放行（关联式要求 API 名后紧跟其路径实参，纯字符串共现不构成写操作）

### Requirement: 拒绝指令文案

命中保护时 SHALL 产出一条面向 **Agent**（首要读者，模型可见）兼可读于用户（次读者）的拒绝指令串，三条形态：

- **内置 `config.json` 规则串**：SHALL 含五要素——① 该文件受写入保护（携路径）；② 该文件是工作区配置权威、Agent 不得改写；③ 如需调整须在回复中说明「改哪个字段、改成什么、为什么」并由用户自行操作；④ 若任务无法在不改该文件的前提下完成，立即停止并通知用户；⑤ 不要改用其他命令或工具尝试绕过（所有写入路径均被拦截）。④⑤ MUST NOT 省略——`deny` 不中断 run，模型被拒后会自然换路重试（write → 重定向 → `cp` → `node -e`），这两句是掐断该链的唯一手段。
- **用户规则带 `reason`**：SHALL 原样透传配置文案，并完成占位符替换——`%s` → 命中路径，`%t` → 工具名（`write` / `edit` / `bash`），全量替换所有出现。
- **用户规则无 `reason` 回落串**：SHALL 为通用串——该文件受写入保护（携路径与工具名）+ 请调整方案避免修改该文件 + 如无法规避立即停止并通知用户、不要改用其他命令绕过。
- **传递面**：拒绝串 SHALL 以单条字符串返回，由 loop 侧同时写进 `SystemNotice.payload.reason`（前端 notice 展示）与 is_error `ToolResult.content`（模型可见）；保护模块 MUST NOT 自行发事件、MUST NOT 分叉出第二套文案。
- **「中断」语义边界**：文案中的「立即停止」指模型结束本轮（正常收敛），引擎 MUST NOT 为此新增机制；相位会话停手后的收敛归编排层判定，保护模块不越界。

#### Scenario: 内置串含掐断换路链两要素

- **WHEN** 命中内置 `openspec/config.json` 规则
- **THEN** 拒绝串含「不要改用其他命令或工具尝试绕过」与「立即停止并通知用户」两句，且含「改哪个字段、改成什么、为什么」的用户处置指引

#### Scenario: 用户 reason 占位符替换

- **WHEN** 用户配置 `{ glob: "docs/**", reason: "受保护：%s（%t）不可写" }`，模型以 `bash` 写 `docs/a.md`
- **THEN** 拒绝串为该模板全文，`%s` 替换为实际路径、`%t` 替换为 `bash`（多处出现全部替换）

#### Scenario: 无 reason 回落

- **WHEN** 用户配置的规则只有 `glob` 无 `reason`，判定命中
- **THEN** 拒绝串为通用回落串（含路径与工具名 + 调整方案 + 不绕过 + 停止并通知用户）

#### Scenario: 单串双面传递

- **WHEN** 保护闸拒绝一次工具调用
- **THEN** `SystemNotice.payload.reason` 与同 id 的 is_error `ToolResult.content` 为同一字符串，模型与用户看到同一份指令（保护模块自身零事件流出）

### Requirement: 装配与判定的分工与失败面

`protect` 模块 SHALL 严格分离装配与判定，并显式定义各失败面的方向（fail-open 于防护、fail-closed 于内置面）：

| 面 | 方向 | 语义 |
|---|---|---|
| 配置读取（缺失 / 坏 JSON / 字段违例） | fail-open（配置面） | 只用内置规则，用户规则缺席；不拒绝 run、不报错给用户 |
| 用户 glob 非法 / 展开超限 | fail-open（防护面） | 该条规则不命中（`match_glob` 已按不命中收敛），其余规则照常 |
| write / edit 路径字段缺失或非字符串 | fail-open | 放行，错误归属执行体 |
| shell 提取无法解析的 token（含 `$` / 空串） | fail-open | 跳过该操作，其余操作照常判定 |
| 内置 `config.json` 规则 | fail-closed | 恒在，不受任何配置状态影响 |

- `ProtectRules::load(workspace_root: Option<&Path>) -> ProtectRules` SHALL 为唯一 IO 点；`ProtectRules::evaluate(tool, input, cwd) -> Option<String>` SHALL 为纯函数（零 IO、零副作用、可密集单测）。
- 判定 MUST NOT panic：任何输入形态（缺字段 / 类型不符 / 超长命令 / 空命令）SHALL 收敛为 `None`（放行）或 `Some(reason)`（拒绝）。
- 本能力 MUST NOT 引入审批通道、MUST NOT 修改 permission-mode 三档语义（保护闸是档位门之后的独立一段，任何档位下均生效）。

#### Scenario: 装配与判定分离可测

- **WHEN** 以构造好的 `ProtectRules`（不经文件系统）驱动 `evaluate` 覆盖 write / edit / bash 三路
- **THEN** 全部判定可在纯内存用例中完成（无 tempdir、无 IO），装配面另以 tempdir + config 文件单独覆盖

#### Scenario: 畸形输入不 panic

- **WHEN** 以缺 `path` 的 write 入参、缺 `command` 的 bash 入参、空字符串命令、超长（100 KB 级）命令驱动 `evaluate`
- **THEN** 前两者返回 `None`，空命令返回 `None`，超长命令在可接受耗时内返回结果（不 panic、不挂起）

#### Scenario: 档位语义不被改写

- **WHEN** 审查保护闸与 policy 的关系
- **THEN** 三档决策表逐格行为与保护闸引入前一致；保护闸只做「档位已放行之后的追加拒绝」，MUST NOT 把任何档位变宽或变窄

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/agent/src/sdk/protect/mod.rs`（新） | 装配 + 判定入口 + 目标路径解析 | `ProtectRules::load(Option<&Path>) -> ProtectRules`（唯一 IO 点）；`evaluate(tool, input, cwd) -> Option<String>`（纯函数零 IO）；归一剥 `\\?\`、`\`→`/`；相对路径拼 `cwd` |
| `crates/infra/agent/src/sdk/protect/rules.rs`（新） | 内置规则派生 + 用户规则合并 + 规则序 | 内置 glob = `**/` + `foundation::layout::config_path` 的 root 相对形态（源码零 `openspec` / `config.json` 字面量）；相对 glob 加 `**/`、绝对原样、空 glob 跳过；内置在前；`None` root = 仅内置 |
| `crates/infra/agent/src/sdk/protect/shell.rs`（新） | 双变体等长掩码 + token 纪律 + 三态提取 + 档 B 五项 + bulk revert | `mask_heredocs`（变体 A）/ `mask_quoted_and_heredoc`（变体 B）等长；`>` 排除集如实 `[^-=]`；`>&` 跳 `\d+`；`$` token 落点丢弃；`cp`/`dd of=`/`sed -i`/`truncate`/`find -delete` 判据；`git stash`/`clean`/`restore openspec/changes/**` 三规则 |
| `crates/infra/agent/src/sdk/protect/inline.rs`（新） | node / python 关联式写入判别 | JS / PY API 表逐条正则（路径取第一字符串实参）；对整条命令文本跑、不切脚本段；裸脚本 / `-m` 放行；MUST NOT 采用 `^(python\|node)\s` 整体豁免 |
| `openspec/config.json` 的 `write_protection` | 用户规则实配（写保护内置锁定面内的配置） | `files[0].glob` 改指 `packages/desktop/knip.json`（保护动机：knip 配置决定死代码检出范围，是 `static_analysis` 入口 `client:check` 的唯一配置源）；该行 SHALL 以值级替换回填（逐值清单 + 复核命令，见 `repo-surface-scope`），MUST NOT 为回填改动本条内置锁定语义；内置 `**/openspec/config.json` 恒在兜底 |
| `crates/infra/agent/src/sdk/protect/reason.rs`（新） | 三条拒绝指令串 | 内置串五要素（含"不得绕过"+"立即停止并通知用户"）；用户 reason `%s` / `%t` 全量替换；无 reason 回落串；单串返回（事件与 ToolResult 由 loop 合成） |
| `crates/infra/agent/Cargo.toml` | glob 方言单点复用 | 新增 `checks = { workspace = true }`（仅取 `checks::globmatch::match_glob`；MUST NOT 自造第二份方言） |
| `checks::globmatch::match_glob`（既有，零改） | 唯一 glob 方言实现 | `match_glob(path, pattern) -> bool`：`**/` 零段可命中、正斜杠归一、无通配模式走目录前缀；非法 / 超限按不命中收敛 |
| `config::load` / `WriteProtection` / `WriteProtectionFile`（既有，零改） | 用户规则来源 | `load(root) -> ConfigReport` 永不失败；`files[].glob` / `files[].reason` 已由组装层剔空与校验 |
| `foundation::layout::config_path`（既有，零改） | 内置 glob 派生源 | 返回 `<root>/openspec/config.json`；内置 glob = `**/` + 该相对形态（字面量隔离唯一触点） |
