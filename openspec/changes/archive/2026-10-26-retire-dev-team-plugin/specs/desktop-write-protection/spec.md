# desktop-write-protection Specification (Delta)

## MODIFIED Requirements

### Requirement: 保护规则装配（内置锁定 + 用户规则合并）

保护规则 SHALL 由两部分合并而成，装配为单点纯数据结构 `ProtectRules`：

- **内置锁定（独立于配置解析结果）**：SHALL 只收一条 `**/openspec/config.json`。该 glob MUST 由 `foundation::layout::config_path` 派生后加 `**/` 前缀，产品源码 MUST NOT 出现 `openspec` 目录名字面量或 `config.json` 文件名字面量（沿 desktop-crate-layout 两条隔离禁令）。内置规则 MUST NOT 依赖 `config::load` 是否成功——工作区配置缺失、坏 JSON、字段违例时内置面 SHALL 照常生效。`**/openspec/changes/**/workflow.json` MUST NOT 纳入内置锁定：工作流状态以库为唯一权威，legacy 文件零读取零产出，锁定它无对象可保护。该规则的存在使**该文件成为保护面的第一道拦截**（`protect/**` 内无 override 或跳过开关）——仓库面若需更新该文件内的指针，SHALL 以值级替换交付并复核（不变量见 `repo-surface-scope`），MUST NOT 为改写指针而改动本条内置锁定语义或给规则加 override。
- **用户规则**：SHALL 取自 `config::load(workspace_root).config.write_protection.files[]`（`config::load` 永不失败，恒返回合法配置 + diagnostics）。逐条规则：`glob` 为 `None`（config 组装层已剔除空串 / 非法值）SHALL 跳过；`glob` 为绝对路径 SHALL 原样使用；`glob` 为相对路径 SHALL 前缀 `**/`（无通配模式在匹配面走「相等或 `pattern/` 起手」语义，不加前缀只命中 root 相对形态）。`reason` SHALL 原样保留（含 `%s` / `%t` 占位符，替换见「拒绝指令文案」）。
- **规则序**：内置规则 SHALL 排在用户规则之前；首个命中规则决定拒绝串（内置 `config.json` 规则恒优先，即使命中的是用户配置里另一条更宽规则）。同一次判定 MUST NOT 拼接多条 reason。
- **方言单点**：规则匹配 SHALL 一律经 `checks::globmatch::match_glob`（`**/` 零段可命中、Windows 盘符归一、无通配模式走目录前缀、花括号交替已覆盖）；`agent-runtime` MUST NOT 自造第二份 glob 匹配实现。非法 / 超限模式 SHALL 按不命中收敛（不 panic、不拒绝启动）。
- **装配时机与唯一 IO 点**：规则装配 SHALL 为每 run 一次（会话泵起播时），是 protect 模块的唯一 IO 点（`config::load` + 规则合并）；loop 内每次判定 MUST NOT 触碰文件系统（run 内改 config 不改变本 run 行为）。
- **工作区根缺席**：装配入参为 `None`（未注入 workspace root）时 SHALL 只产出内置规则（用户规则缺席）——MUST NOT 产出「保护关闭」的空规则集。

#### Scenario: 规则面单源自述

- **WHEN** 审查 `ProtectRules` 装配逻辑的注释与 spec 措辞
- **THEN** 每条规则语义按本 spec 直述（内置面 / 用户面 / 序 / 方言 / 时机 / 根缺席六条）
- **AND** 无「与插件端同款处理 / 插件端此处放行」类对照表述（插件端已不存在，语义不依赖对照读者）

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

#### Scenario: 档 B 五族语义单源

- **WHEN** 审查档 B 五族的判据与代价注记
- **THEN** 每族判据、已知粗粒度代价与本 spec 表逐项一致（不复刻任何外部实现的提取面）
- **AND** 遍历 / 递归拆解仍属承认边界（MUST NOT 实现）

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

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/infra/agent/src/sdk/protect/rules.rs` | 规则装配单点 | 内置一条 `**/openspec/config.json` + 用户规则 `write_protection.files[]`；相对 glob 前缀 `**/`；每 run 一次装配（唯一 IO 点） |
| `crates/infra/agent/src/sdk/protect/shell.rs` | bash 命令文本提取 | 双变体等长掩码 + 三态提取 + 档 B 五族 |
| `crates/infra/agent/src/sdk/protect/inline.rs` | node / python 关联式判别 | API 表逐条正则（路径取第一字符串实参）；对整条命令文本跑、不切脚本段；裸脚本 / `-m` 放行；MUST NOT 采用 `^(python\|node)\s` 整体豁免 |
| `openspec/config.json` 的 `write_protection` | 用户规则实配（写保护内置锁定面内的配置） | `files[0].glob` 改指 `packages/desktop/knip.json`（保护动机：knip 配置决定死代码检出范围，是 `static_analysis` 入口 `client:check` 的唯一配置源）；该行 SHALL 以值级替换回填（逐值清单 + 复核命令，见 `repo-surface-scope`），MUST NOT 为回填改动本条内置锁定语义；内置 `**/openspec/config.json` 恒在兜底 |
