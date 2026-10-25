# 任务: desktop-write-protection

> 覆盖 proposal「变更范围 - 实现文件」14 项 = **5 新增 + 9 修改**（`Cargo.toml` / `package.json` 各计一项）。**本相位不改测试文件**：`sdk/protect/{rules,shell,inline,reason}_test.rs` 的新建、`loop_test` / `runner_test` 的新增用例、`LoopTurn` 字面量补 `protect`、`compose_turn` 调用点补尾参（40 处测试调用）全部归 test-design / test-gen / test-execution 相位。
>
> 依赖序要点：**阶段一**（依赖边 + 模块骨架）解锁全部后续阶段；**阶段二**（rules）与**阶段三**（reason）互不依赖，共同解锁**阶段四/五**（shell）与**阶段六**（inline）；**阶段七**（判定主体）依赖二～六；**阶段八**（loop 闸门）依赖阶段七；**阶段九**（root 通道）依赖阶段一/七，且**阶段十**（命令层调用点）必须与阶段九同批改齐——`compose_turn` 是公开函数，漏改任一生产调用点即不通过编译。
>
> 中间态约定：阶段八让 `LoopTurn` 长出必填字段 `protect`，此后 `cargo test` 不通过编译（`loop_test.rs` 的 `LoopTurn {}` 字面量缺字段）；阶段九让 `compose_turn` 长出尾参，`compose_test` 32 处 + `worker_test` 7 处测试调用同缺。
>
> **实际处置（静态检查门禁驱动）**：`server:check` = `cargo fmt --all` + `cargo clippy --workspace --all-targets`，两命令都要解析并编译测试目标，故上述中间态在本仓库不可能「只过生产面」。本相位因此只做**纯机械随动、零新用例**：`loop_test.rs` 3 处 `LoopTurn` 字面量补 `protect`（两处助手函数经 `..` 展开已自动承接）、`compose_test` 32 处 + `worker_test` 7 处调用补尾参 `Path::new(&env.root())`、`lib_test.rs` 的 `compose_turn` 签名锚定补第六参；`sdk/protect/{mod,rules,shell,inline,reason}_test.rs` 建立为仅含头注释的空模块（承接 `protect/mod.rs` 的四条 `#[cfg(test)]` 声明 + test-design 补列的 `mod_test`，否则 `cargo fmt` 报「failed to resolve mod」）。用例编写仍归 test-gen 相位——本相位新增 0 条 `#[test]`。
>
> 自查命令的 cwd 除特别标注外均为 `packages/desktop/src-tauri`（`grep -rn … packages/desktop/src/…` 一项以仓库根为 cwd）。
>
> 不预铺：无 config schema 扩展、无 bindings 重生成、无前端改动、无 db 迁移、无新 IPC 命令、无插件侧改动。

## 阶段一：依赖边与模块骨架

- [x] 修改 `packages/desktop/src-tauri/crates/infra/agent/Cargo.toml`：`[dependencies]` 段 `foundation` 行之后插入 `checks = { workspace = true }`，附一行注释「写入保护规则匹配：复用 `checks::globmatch::match_glob`（glob 方言单点，禁第二份实现）」；`[dev-dependencies]` 零改
- [x] 新建 `packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/mod.rs` 骨架：模块 doc（装配 / 判定分工、四段守卫链位置、射程边界、知情边界「`xargs` / `sh -c` 递归拆解 / 裸脚本 / 变量中转 / base64 / `printf` 喂 stdin 不可判别」）+ 四个兄弟模块声明 `mod rules; mod shell; mod inline; mod reason;` + 四条 `#[cfg(test)] mod {rules,shell,inline,reason}_test;` 声明（**测试模块声明在此，不在 `sdk/mod.rs`**）
- [x] 修改 `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mod.rs`：`pub(crate) mod preamble;` 与 `pub(crate) mod resume;` 之间加 `pub(crate) mod protect;`（本文件唯一改动）
- [x] 自查 阶段一：`grep -n "protect" crates/infra/agent/src/sdk/mod.rs` 恰一行；`grep -n "_test" crates/infra/agent/src/sdk/protect/mod.rs` 命中四条声明；`cargo tree -p agent-runtime -i checks` 显示 `agent-runtime → checks` 单边，且 `cargo tree -p checks | grep -E "infra|agent-runtime"` 零命中（无环）

## 阶段二：规则装配（`protect/rules.rs`，依赖阶段一）

- [x] 新建 `rules.rs` 的类型面：`pub(super) struct Rule { pub(super) glob: String, pub(super) reason: ReasonKind }`、`pub(super) enum ReasonKind { BuiltinConfig, User(Option<String>) }`、`pub(super) struct BulkGlobs { pub(super) artifacts: String, pub(super) changes: String }`（各 `#[derive(Debug, Clone)]`，`Rule` 内字段 `pub(super)` 供 `mod.rs` / `shell.rs` 读）
- [x] 同文件实现 `pub(super) fn builtin_rules() -> Vec<Rule>`：取 `foundation::layout::config_path(Path::new(""))` 的 root 相对形态（`PathBuf` → 正斜杠文本）→ 前缀 `**/` → 一条 `ReasonKind::BuiltinConfig` 规则（`doc` 注明「glob 由 `layout` 派生、本源码禁 `openspec` / `config.json` 字面量；工作流状态文件的插件端内置规则**不复刻**」）
- [x] 同文件实现 `pub(super) fn bulk_globs() -> BulkGlobs`：`artifacts` = `**/` + `foundation::layout::domain_dir_name()` + `/**`；`changes` = `**/` + `foundation::layout::resolve(Path::new("")).changes_root` 的正斜杠文本 + `/**`
- [x] 同文件实现用户规则合并 `pub(super) fn user_rules(root: &Path) -> Vec<Rule>`：`config::load(root)` → `config.write_protection` → `files.unwrap_or_default()` → 逐条 `glob` 为 `None` 跳过、绝对原样、相对加 `**/`（私有 `normalize_user_glob`）、`reason` 落 `ReasonKind::User(_)`；diagnostics **不参与**判定
- [x] 同文件实现 `pub(super) fn load(workspace_root: Option<&Path>) -> Vec<Rule>`：`builtin_rules()` 在前；`Some(root)` 时追加 `user_rules(root)`；`None` 只回内置（不产出空集）
- [x] 同文件实现 `impl Rule { pub(super) fn concrete_prefix(&self, anchor: &Path) -> String }` 与 `pub(super) fn glob_dir_hit(dir: &str, glob: &str, anchor: &Path) -> bool`（D9 三面比较：`prefix == dir` / `prefix` 以 `dir/` 起手 / `dir` 以 `prefix/` 起手）：glob 无 `**/` 起手 → 自身首个通配段之前的静态段；有 `**/` 起手 → `anchor` 归一 + `/` + 尾部静态段；`MatchOptions` 不自建（匹配一律经 `checks::globmatch::match_glob`）
- [x] 同文件实现路径归一与解析单点（`mod.rs` / `shell.rs` / `concrete_prefix` 共用，禁各写一份）：`pub(super) fn normalize(raw: &str) -> String`——剥 Windows 扩展长度前缀 → 反斜杠转 `/` → 按 `/` 切段去空段与 `.` 段、保留 `..` 段 → 重拼并保留前导 `/`；`pub(super) fn resolve(anchor: &Path, raw: &str) -> Option<String>`——空串或含 `$` → `None`，绝对原样归一、相对 `anchor.join` 后归一
- [x] 自查 阶段二：`grep -nE "openspec|config\.json" crates/infra/agent/src/sdk/protect/rules.rs` **零命中**（含注释）；`grep -n "char::from\|\\\\0" crates/infra/agent/src/sdk/protect/rules.rs` 零命中；`grep -n "glob::Pattern\|MatchOptions" crates/infra/agent/src/sdk/protect/rules.rs` 零命中（方言单点）

## 阶段三：拒绝文案（`protect/reason.rs`，依赖阶段一；与阶段二互不依赖）

- [x] 新建 `reason.rs` 四条模板常量：内置工作区配置串（五要素：受保护 + 路径；配置权威、Agent 不得改写；说明「改哪个字段、改成什么、为什么」并由用户自行操作；无法规避请立即停止并通知用户；不要改用其他命令或工具尝试绕过）、无 reason 回落串（路径 + 工具名 + 调整方案 + 不绕过 + 停止并通知用户）、批量还原串（事实串 + 「批量还原 / 清理对按路径判定的护栏不可见」+ 引导逐路径 `git restore <path>` + 两句功能句）；模板统一用 `%s`（路径）/ `%t`（工具名）占位
- [x] 同文件实现 `pub(super) fn substitute(template: &str, path: &str, tool: &str) -> String`：`%s` / `%t` **全量**替换（逐次 `replace`，不单次替换）
- [x] 同文件实现 `pub(super) fn deny_reason(kind: &ReasonKind, path: &str, tool: &str) -> String`：`BuiltinConfig` → 内置模板替换；`User(Some(t))` → 原样透传并替换；`User(None)` → 回落模板
- [x] 同文件实现 `pub(super) fn bulk_revert_reason(detail: &str, tool: &str) -> String`：`detail` 落 `%s`、工具名落 `%t`
- [x] 自查 阶段三：`grep -nE "openspec|config\.json" crates/infra/agent/src/sdk/protect/reason.rs` 零命中；四条模板逐条含「不要改用其他命令」与「立即停止并通知用户」（人工核对）；模板无未替换的 `%s` / `%t` 残留路径

## 阶段四：shell 掩码与 token 纪律（`protect/shell.rs`，依赖阶段一）

- [x] 新建 `shell.rs` 的字符空间基础：以 `Vec<char>` 为载体，私有 `struct Span { start: usize, end: usize }`（char 索引）、`const MASK_CHAR: char = '\u{0}'`（doc：非空白、不属任一排除类，与插件端同值）、私有 `fn mask_spans(chars: &[char], spans: &[Span]) -> String`（合并重叠区间后逐字符替换，输出长度 == 输入长度）
- [x] 同文件实现 `quote_spans` / `heredoc_spans`：单 / 双引号区间（单引号无转义、双引号认 `\` 转义、不嵌套、未闭合则**放弃该候选并停止掩码**＝fail-open）；heredoc 区间（marker 必须紧贴 `<<` / `<<-`、`<<` 必须在引号外、必须同时找到换行与终止 marker 行，否则放弃候选）——逐行照译插件端语义
- [x] 同文件实现 `pub(super) fn mask_heredocs(command: &str) -> String`（变体 A：只掩 heredoc、保留引号）与 `pub(super) fn mask_quoted_and_heredoc(command: &str) -> String`（变体 B：掩 heredoc + 引号）；doc 写明「两变体等长，char 索引 1:1 互通；A 跑写入正则与参数切片、B 定位命令名」
- [x] 同文件实现 `pub(super) fn tokenize(args: &str) -> Vec<String>`（空白切分 → 剥首尾引号 → 剔空；**不**做 `$` 过滤）、`pub(super) fn is_flag(token: &str) -> bool`（`-` 起手）、私有 `fn is_literal_path(token: &str) -> bool`（不含 `$`；doc：只在 op 落点用，绝不进 `tokenize`——`git clean` 零路径判定依赖原始 token 计数）
- [x] 同文件实现私有 `fn scan_command(command: &str, names: &str) -> Vec<String>`：命令名在变体 B 上以 `(?:^|[\s;&|(])(?:{names})\s+([^;&|\n]*)` 定位，参数在变体 A 上按同索引切片（`(?i)`）
- [x] 自查 阶段四：两变体对同一命令的 `chars().count()` 相等（三例：中文路径、heredoc、未闭合引号）；`grep -n "MASK_CHAR" crates/infra/agent/src/sdk/protect/shell.rs` 仅在定义与掩码处；被掩码文本与参数切片一律走 char 向量（`grep -n "&command\[" crates/infra/agent/src/sdk/protect/shell.rs` 零命中）

## 阶段五：三态提取 + 档 B + 批量还原（`protect/shell.rs`，依赖阶段二/四）

- [x] 同文件实现类型面：`pub(super) struct FileOp { pub(super) kind: OpKind, pub(super) target: OpTarget }`、`pub(super) enum OpKind { Write, Delete, Revert }`、`pub(super) enum OpTarget { Path(String), Tree(String) }`（`Debug, Clone, PartialEq, Eq`）
- [x] 同文件实现重定向写入族（变体 A 上跑）：`>` / `>>`（前缀排除集**如实** `[^-=]`）、`>|`、`tee`（含 `-a` 等 flag）、`>&`（捕获后过滤：目标全为 ASCII 数字即丢弃，替代 JS 前瞻）；PowerShell 族：`*>` / `N>` 重定向、`Set-Content` / `Out-File` / `Add-Content` / `Export-Csv` / `Export-CliXml` / `Tee-Object`（可带 `-Path` / `-FilePath`）、`[System.IO.File]::WriteAllText|WriteAllLines|WriteAllBytes|AppendAllText`（首参含 `$` 丢弃）
- [x] 同文件实现删除族（`scan_command` + `tokenize` + 跳 flag 与 `$` token）：`Remove-Item|rmdir|unlink|rm|del|rd`；移动族：`Move-Item|Rename-Item|ren|mv` 且恰两个路径 token → `Delete(旧)` + `Write(新)`
- [x] 同文件实现 `pub(super) fn extract_git_ops(command: &str) -> Vec<FileOp>`：`git rm` 逐参数 → `Delete`；`git restore`（`--source=` / `-s <ref>` → `Write`，裸形态 → `Revert`）；`git checkout <ref> -- <paths>` → `Write`、`git checkout -- <paths>` → `Revert`（`--` 分隔符之前的非 flag token 决定 write / revert 判定）
- [x] 同文件实现档 B 五族（D8）：`cp` / `Copy-Item` 末个非 flag 非 `$` token → `Write`；`dd` 的 `of=` 右值 → `Write`（跳过 `if=`）；`sed` / `perl` 含 `-i`（含 `-i.bak` / `-i''` 紧贴形态）时末个非 flag 非 `$` token → `Write`（无 `-i` 恒零 op）；`truncate` 的 `-s` / `--size=` 之后首个非 flag token → `Write`；`find` 含 `-delete` 时首个非 flag token → `Tree`
- [x] 同文件实现 `pub(super) fn extract_file_ops(command: &str) -> Vec<FileOp>`：按「重定向族 → PS 方言 → 删除族 → 移动族 → git 子命令 → 档 B 五族」顺序拼接，保序不折叠（`git rm` 与 `rm` 双记由判定侧首个命中即止消化）
- [x] 同文件实现 `pub(super) fn bulk_revert_detail(command: &str, globs: &BulkGlobs, cwd: &Path) -> Option<String>`：`git stash` 参数不以 `list` / `show` 起手 → 命中；`git clean` 先判 dry-run（`--dry-run` 或 `-[^-]*n`）放行，再判「无路径参数 → 命中」/「任一非 flag 参数经 `rules::resolve(cwd, 参数)` 归一后由 `rules::glob_dir_hit(dir, globs.artifacts, cwd)` 命中」；最后判 `extract_git_ops` 中 `OpKind::Revert` 的目标经 `glob_dir_hit(resolved, globs.changes, cwd)` 命中（事实串点名目标路径）。`cwd` 只作路径解析与锚定基座，内部零 IO
- [x] 同文件把全部正则收进 `static …: OnceLock<Vec<Regex>>`（沿 `sdk/log.rs:9` 先例），doc 写明「单量词线性形态、禁嵌套量词、**不设命令文本长度上限**」
- [x] 自查 阶段五：`grep -n "\[^-\=&" crates/infra/agent/src/sdk/protect/shell.rs` **零命中**（排除集不得补 `&` / `|`）；`grep -n "(?!" crates/infra/agent/src/sdk/protect/shell.rs` 零命中（`regex` crate 无 lookaround）；`grep -c "OnceLock" …` ≥ 各模块一处；`grep -n "is_flag" …` 覆盖删除 / 移动 / 档 B 全部参数面

## 阶段六：inline 关联式（`protect/inline.rs`，依赖阶段一）

- [x] 新建 `inline.rs` 的 API 表（每项一条正则，路径取**第一个字符串实参**）：JS —— `writeFileSync` / `writeFile` / `appendFileSync` / `appendFile` / `createWriteStream` / `copyFileSync` / `copyFile` / `renameSync` / `rename` / `unlinkSync` / `unlink` / `rmSync` / `rm` / `fs.promises.writeFile` / `openSync(p, 'w'|'a'|'wx')`（末项需**模式实参**校验）；PY —— `open(p, 'w'|'a'|'x'|'wb'|'w+')`（首模式字符 ∈ `wax`）/ `Path(p).write_text` / `Path(p).write_bytes` / `shutil.copy` · `copy2` · `copyfile` / `shutil.move` / `os.remove` · `unlink` · `rename` · `replace`
- [x] 同文件实现 `pub(super) fn write_targets(command: &str) -> Vec<String>`：逐个预编译正则 `captures_iter`，取第一捕获组（引号内字面量）；变量实参（无引号）不捕获 → 自然放行；不识别 `-e` / `-c` / `--eval` 脚本边界
- [x] 同文件 doc 写明：不照抄插件端 `^(python|node)\s` 整体豁免；`node -c`（`--check`）不构成误判来源；裸脚本 / `-m` 放行且**不影响**同命令内重定向 / `cp` / 删除族的判定；已知漏面（生成脚本再执行 / 变量中转 / base64 / `printf` 喂 stdin）；`copyFile*` / `shutil.copy*` / `os.rename*` 首参为源 → 保守拦截（over-block 方向）
- [x] 自查 阶段六：`grep -n "python|node\\s" crates/infra/agent/src/sdk/protect/inline.rs` 零命中（无整体豁免）；无 `(?i)` 之外的全文共现式（每条正则必须含捕获组且捕获组紧跟 API 名）

## 阶段七：判定主体（`protect/mod.rs`，依赖阶段二～六）

- [x] 同文件实现 `pub(crate) struct ProtectRules { rules: Vec<Rule> }`（`#[derive(Debug, Clone)]`，字段私有）与 `pub(crate) fn load(workspace_root: Option<&Path>) -> ProtectRules`（只调 `rules::load`；doc 写明「唯一 IO 点、每 run 一次」）
- [x] 同文件实现私有 `fn match_path(&self, path: &str) -> Option<&Rule>`（逐条 `checks::globmatch::match_glob`，首个命中即返）、`fn dir_rule_hit(&self, dir: &str, anchor: &Path) -> Option<&Rule>`（逐条 `rules::glob_dir_hit`）、`fn deny_for(rule: &Rule, path: &str, tool: &str) -> String`（转 `reason::deny_reason`）
- [x] 同文件实现 `pub(crate) fn evaluate(&self, tool: &str, input: &Value, cwd: &Path) -> Option<String>` 的工具名分派：`"write" | "edit"` → 私有 `evaluate_path`（取 `path`，缺失 / 非字符串 → `None`；`rules::normalize` 后 `match_path`）；`"bash"` → 私有 `evaluate_shell`；`_ => None`（含只读四工具与专用工具 `create_explore_note`——射程由分派表表达）
- [x] 同文件实现私有 `evaluate_shell`：`command` 缺失 / 非字符串 / 空 → `None`；整串先 `\` → `/` 归一；按序判 ① `shell::extract_file_ops`（`OpKind::Revert` 跳过；`OpTarget::Path` → `rules::resolve(cwd, …)` + `match_path`；`OpTarget::Tree` → `dir_rule_hit(tree, cwd)`；首个命中即返回，`%s` 报 resolved 路径或搜索根）② `inline::write_targets`（逐个 `rules::resolve` + `match_path`）③ `shell::bulk_revert_detail(command, &rules::bulk_globs(), cwd)`（命中 → `reason::bulk_revert_reason`）
- [x] 同文件 doc 补：装配 / 判定分工与失败面矩阵（配置失败 → 仅内置；glob 非法 → 不命中；入参非法 → 放行；`$` / 空串 → 跳过该 op；内置 fail-closed）；「判定零 IO、零副作用、不 panic、不发事件」；「`create_explore_note` 与用户规则的一致性缺口为既定知情边界」
- [x] 自查 阶段七：`grep -nE "fs::|canonicalize|config::load" crates/infra/agent/src/sdk/protect/*.rs` **仅** `rules.rs` 的 `config::load` 一处（`protect` 其余文件零 fs / 零 canonicalize）；`grep -n "regex" crates/infra/agent/src/sdk/protect/mod.rs` 零命中（模式不越模块）

## 阶段八：loop 写入保护闸（`sdk/loop.rs`，依赖阶段七）

- [x] 修改 `loop.rs`：`use crate::sdk::protect::ProtectRules;`；`LoopTurn` 增 `pub protect: ProtectRules`（doc：装配点算好、loop 纯消费，与 `defense` / `retry` / `liveness` 同列）
- [x] 同文件 `:387` `sandbox_rewrite` 成功之后、`:399` 执行臂路由之前插入第四段闸门：`if let Some(reason) = turn.protect.evaluate(&name, &sandboxed, &turn.cwd) { deny(&sender, "permission_denied", &name, &reason, &id).await; results.push(outcome_result(tool_call, reason)); continue; }`（与 policy / sandbox 两段同形；`reason` 单变量同时进 notice 与 ToolResult）
- [x] 同文件把守卫链注释（`:364` 附近）由三段改为四段：`成员校验 → policy 档位门 → 路径沙箱 → 写入保护闸 → 执行`，并写明「保护闸在沙箱之后（canonical 路径已在手、错误归属不抢报）」「任何档位下均生效」「`create_explore_note` 不在射程（由 `evaluate` 分派表表达，不做 loop 侧特判）」
- [x] 自查 阶段八：`grep -n "permission_denied" crates/infra/agent/src/sdk/loop.rs` 命中 policy 臂与保护臂各一处（无新 subtype）；`grep -nE "fs::|config::load|canonicalize" crates/infra/agent/src/sdk/loop.rs` 零命中；`deny` / `outcome_result` / `sandbox_rewrite` / `finish_*` / `defend` 零字节改动；`cargo check -p agent-runtime` 通过（`cargo test` 此时**预期**不通过，见中间态约定）

## 阶段九：root 通道（`sdk/runner.rs` → `lib.rs` → `compose.rs`，依赖阶段七）

- [x] 修改 `sdk/runner.rs`：`SdkRunner` 增 `workspace_root: Option<PathBuf>`（`new()` 内 `workspace_root: None`，四参签名不动）+ `pub fn with_workspace_root(self, root: PathBuf) -> Self`；`PumpPayload` 增 `workspace_root: Option<PathBuf>`，`open_session` 的 `PumpPayload { … }` 构造补 `workspace_root: self.workspace_root.clone()`
- [x] 同文件 `session_pump` 内、`LoopTurn` 构造之前加 `let protect = ProtectRules::load(payload.workspace_root.as_deref());`（每 run 一次、唯一装配点），`LoopTurn { … }` 初始化补 `protect`；`use crate::sdk::protect::ProtectRules;`
- [x] 修改 `crates/infra/agent/src/lib.rs`：`EngineFacade` 增 `workspace_root: Option<PathBuf>`；`new()` / `with_resume_transcript()` / `with_context_window()` / `with_max_tokens()` / `with_explore_note_store()` 五处构造补 `workspace_root: None`（后者为 `Some(store)` 行相邻字段，勿漏）；新增 `pub fn with_workspace_root(self, root: PathBuf) -> Self`
- [x] 同文件 `runner_for` sdk 臂改为先构造后条件注入：`let mut runner = SdkRunner::new(...).with_max_tokens(self.max_tokens); if let Some(root) = &self.workspace_root { runner = runner.with_workspace_root(root.clone()); } Ok(Box::new(runner))`；`EngineKind::Cli` 臂与唯一 match 点结构零改
- [x] 修改 `crates/infra/agent/src/compose.rs`：`use std::path::Path;`；`compose_turn` 新增尾参 `workspace_root: &Path`（doc：主仓 root，会话执行根不经此通道）+ 门面链 `.with_workspace_root(workspace_root.to_path_buf())`（置于 `.with_explore_note_store(..)` 之后）；`resolve_agent_engine` / `assemble_injections` / `ComposedTurn::begin` 零改
- [x] 自查 阶段九：`grep -n "workspace_root" crates/infra/agent/src/lib.rs crates/infra/agent/src/sdk/runner.rs crates/infra/agent/src/compose.rs` 覆盖「字段 + 构造 + builder + 透传 + 装配」五处；`grep -n "ProtectRules::load" crates/infra/agent/src` 恰一处（`runner.rs` 泵起播）；`cargo check -p agent-runtime` 通过

## 阶段十：命令层注入点（依赖阶段九，与阶段九同批改齐）

- [x] 修改 `packages/desktop/src-tauri/src/commands/exec/mod.rs:132`：`compose_turn(…)` 调用补第六参 `std::path::Path::new(&root)`——**主仓 `root`**，`exec_root` 仍只进 `SessionCtx.workspace_root`
- [x] 修改 `packages/desktop/src-tauri/src/commands/change_flow/mod.rs:143`：同上补 `Path::new(&root)`——MUST NOT 传 `exec_root` / worktree 路径（主仓口径，AC-10）
- [x] 自查 阶段十：`grep -rn "compose_turn(" src/` 命中两处且尾参均为 `&root`（`workspace_root` 拼接里不出现 `exec_root` / `worktree`）；`cargo check` （`src-tauri` 全包，不含 `--tests`）通过

## 阶段十一：版本面

- [x] 修改 `packages/desktop/package.json`：`version` `"0.5.28"` → `"0.5.29"`（单行；`tauri.conf.json` 经 `../package.json` 跟随；`src-tauri/Cargo.toml` 与 `plugins/**` **不**随动）

## 阶段十二：边界与语义自查

- [x] 核对 `git status` 仅含 14 项实现文件（5 新增 + 9 修改）：`crates/core/{config,checks,foundation,agent,workflow,orchestration}/**`、`crates/infra/{exec,vcs,checks,store}/**`、`sdk/{bash,tools,policy,sandbox,explore_note,context,compact,resume,model_spec,normalize,preamble,log,config}.rs`、`packages/desktop/src/**`、`src/types/generated/bindings.ts`、`openspec/config.json`、`plugins/**` 全部零改动
- [x] 核对字面量禁令：`grep -rnE "openspec|config\.json" crates/infra/agent/src/sdk/protect/ --include=*.rs | grep -v _test.rs` **零命中**（含注释与 doc）；内置 glob 与两条结构 glob 均来自 `foundation::layout`
- [x] 核对方言单点：`grep -rn "glob::Pattern\|MatchOptions\|try_expand_braces" crates/infra/agent/src/sdk/protect/` 零命中；匹配只经 `checks::globmatch::match_glob`（`grep -c "match_glob"` 全部落在 `rules.rs` / `mod.rs`）
- [x] 核对判定零 IO：`grep -rnE "fs::|canonicalize|std::fs" crates/infra/agent/src/sdk/protect/{mod,shell,inline,reason}.rs` 零命中；`grep -n "config::load" crates/infra/agent/src/sdk/protect/*.rs` 仅 `rules.rs` 一处
- [x] 核对「保护闸不越界」：`grep -n "AgentEventKind\|SystemNotice" crates/infra/agent/src/sdk/protect/*.rs` 零命中（零事件流出）；`grep -rn "permission_mode" crates/infra/agent/src/sdk/protect/*.rs` 零命中（不看档位，任何档位均生效）
- [x] 核对 policy / sandbox / tools 语义零改：三文件 `git diff` 为空；`sdk/mod.rs` 仅 `protect` 一行、`sdk/loop.rs` 仅字段 + 第四段 + 注释、`sdk/runner.rs` 仅字段 / builder / payload / 装配四处
- [x] 核对出线面零变：`grep -rn "ProtectRules\|OpKind\|ReasonKind\|BulkGlobs" packages/desktop/src/types/generated/bindings.ts` 零命中；`grep -rn "ProtectRules\|workspaceRoot" packages/desktop/src/` 零命中（`writeProtection` 为配置页既有字样，不属本项）；前端 `git status` 为空
- [x] 登记测试相位承接清单（不在本相位执行）：`sdk/protect/{rules,shell,inline,reason}_test.rs` 新建（含 `mod_test` 命名隔离扫描对 `*_test.rs` 的既有排除先例、真实配置 tempdir 驱动、双向断言矩阵）；`loop_test.rs` 8 处 `LoopTurn {}` 字面量补 `protect` + 新增闸门四臂用例；`runner_test.rs` 双根 / 未注入两用例；`compose_test.rs` 33 处 + `worker_test.rs` 7 处 `compose_turn` 调用补尾参；100 KB 命令耗时冒烟
- [x] 把待决问题 1（copy 族首参为源的保守拦截）、2（盘符大小写与绝对 glob）、3（`policy.rs` / `bash.rs` 注释措辞不改）、4（`git clean` 通配参数）、6（双禁令扫描未覆盖 `crates/**`）登记给评审，不阻塞实现
