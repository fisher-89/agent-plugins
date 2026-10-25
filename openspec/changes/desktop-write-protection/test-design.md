# 测试设计: desktop-write-protection

> **变更**: desktop-write-protection
> **日期**: 2026-10-23
> **基于**: `proposal.md`（验收标准 AC-1 ~ AC-13；「变更范围 - 实现文件」14 项 + 「变更范围 - 测试文件」表）、`design.md`（「新增文件」5 项 /「修改文件」9 项；「公共函数 / API」子表 24 行；「类型定义」子表 11 行；「配置」子表 3 行；关键设计决策 D1 ~ D17；待决问题 1 ~ 6）、`specs/{desktop-write-protection,desktop-agent-execution,desktop-crate-layout}/spec.md`
> **被测模块来源**: `design.md`「新增文件」5 项（`crates/infra/agent/src/sdk/protect/{mod,rules,shell,inline,reason}.rs`）+「修改文件」9 项（`crates/infra/agent/src/{sdk/mod,sdk/loop,sdk/runner,lib,compose}.rs`、`src/commands/{exec,change_flow}/mod.rs`、`crates/infra/agent/Cargo.toml`、`packages/desktop/package.json`）；design 无「删除文件」子表。
> **测试框架**: 沿用仓库既有约定——Rust 侧 `#[test]` / `#[tokio::test]`（dev-deps `tokio` 的 `rt` / `macros` / `test-util`、`tempfile`、`rig` 的 `test-utils`）；测试模块以**同目录兄弟文件**承载：`crates/infra/agent/src/sdk/mod.rs` 内 `#[cfg(test)] mod <name>_test;`（`loop_test` / `runner_test` / `sandbox_test` 等 16 个先例）、crate 根 `lib.rs` 内 `#[cfg(test)] mod lib_test;`、`compose.rs` 内 `#[cfg(test)] #[path = "compose_test.rs"] mod compose_test;`、命令层 `src/commands/{exec,change_flow}/mod.rs` 内 `mod mod_test;`；断言用标准 `assert!` / `assert_eq!` / `assert!(matches!(..))`，测试函数名为中文标识符、断言消息为中文；无外部断言库、无独立测试运行器、无 `tests/` 集成目录。
> **一处显式补列**: `protect/mod.rs` 的 `ProtectRules::evaluate` 分派面与全部跨模块组合锚需要落点，而 design / tasks 只列 `{rules,shell,inline,reason}_test` 四条 `#[cfg(test)]` 声明——本设计补列第五条 `mod_test`（沿 crate 既有惯例：`compose.rs` → `compose_test.rs`、`checks` crate 的 `testexec/mod.rs` → `testexec/mod_test.rs`），故映射对为 `sdk/protect/mod.rs -> sdk/protect/mod_test.rs`。若实现侧坚持四条声明，`mod_test` 的全部用例并入 `rules_test.rs`（同模块树，`pub(super)` 可见性不变）。

用例表读法：`路径类型` ∈ {正向, 异常, 边界}；`迭代类型` ∈ {单值, 参数化, 表驱动, 遍历枚举}。跨模块组合用例按「链路发起方 / 最上层调用方」挂靠入口模块的 `#### 用例` 表（本变更的组合锚分别挂靠 `protect/mod.rs`——`evaluate` 三路判定全链、`protect/shell.rs`——提取到子树相交判定链、`sdk/loop.rs`——守卫链第四段拒绝链、`sdk/runner.rs`——泵起播装配链），不设独立集成测试章节。

边界取值口径（按 design「类型定义」/「公共函数 / API」表的 Rust 类型逐项收敛）：

| 类型（载体） | 边界取值 | 最少条数 |
|---|---|---|
| `String` / `&str`（`command` / `path` / `glob` / `reason` / 命令文本 / 拒绝串） | `""`（空）/ 全空白 / 超长（100 KB 级命令、>1000 字符 glob 与 reason）/ 特殊字符（`\n` `\0` emoji、中文路径与中文命令文本、`\\?\` 前缀、反斜杠、包裹引号） | 4 边界 + 1 正常 |
| `serde_json::Value`（`evaluate` 的 `input`） | `{}`（缺 `path` / `command`）/ 字段为 `null` / `number` / `array` / `object` / `bool`（非字符串）/ 多余字段在场（`content` / `pattern`） | 4 边界 + 1 正常 |
| `Vec<FileOp>` / `Vec<String>`（`extract_file_ops` / `extract_git_ops` / `inline::write_targets` 产物） | `[]`（空）/ 单元素 / 多元素保序 / 重复元素（不折叠不去重）/ 三态混合（write + delete + revert） | 4 边界 + 1 正常 |
| `Option<&Path>`（`ProtectRules::load` / `rules::load` 入参） | `None`（仅内置）/ `Some(空 Path)` / `Some(不存在目录)` / `Some(无配置文件目录)` / `Some(config 合法目录)` | 4 边界 + 1 正常 |
| `Option<String>`（`evaluate` 返回值 / `Rule.reason` / `bulk_revert_detail` 返回值） | `None`（放行 / 无 reason）/ `Some(内置串)` / `Some(用户模板替换后串)` / `Some(回落串)` / `Some(批量还原串)` | 4 边界 + 1 正常 |
| enum（`OpKind` / `OpTarget` / `ReasonKind`） | `OpKind` 三值（`Write` / `Delete` / `Revert`）、`OpTarget` 两值（`Path` / `Tree`）、`ReasonKind` 三形态（`BuiltinConfig` / `User(Some)` / `User(None)`）逐个遍历（封闭集，非法值编译期不可表达） | N+1 |
| `Path` / `PathBuf`（`cwd` / `workspace_root` / `anchor`） | 空 Path / 不存在目录 / `tempdir` 绝对根 / worktree 与主仓双根 / 含中文与 emoji 的路径 | 4 边界 + 1 正常 |
| `bool`（`glob_dir_hit` / `is_flag` / `match_glob` 结论） | 两向必断言（`true` 与 `false` 各一组——AC-5 误拦-漏拦双向纪律） | 2 |

路径省略说明：`验收范围` 表内 `.../` 前缀补齐为 `packages/desktop/src-tauri/crates/`；`src/commands/**` 为 `packages/desktop/src-tauri/src/commands/**` 全路径。下文 `## 单元测试` 各节标题给全路径。

---

## 验收范围

| AC ID | 验收条件（摘要，原文见 `proposal.md`） | 被测文件或模块 |
|-------|---------|----------------|
| AC-1 | 内置锁定在任意档位（含默认 `BypassPermissions`）下生效：`write` / `edit` 指向任一 `<root>/openspec/config.json`（含 worktree 内副本）被拒，事件面 `SystemNotice{subtype:"permission_denied", payload:{tool, reason}}` + 同 id is_error ToolResult，run 不中断；同文件 `read` / `grep` 放行 | `.../infra/agent/src/sdk/protect/mod.rs` -> `.../sdk/protect/mod_test.rs`（内置命中 + 射程遍历 + 档位无关）、`.../sdk/protect/rules.rs` -> `.../sdk/protect/rules_test.rs`（`builtin_rules` 派生 + 命中/不命中双向）、`.../sdk/loop.rs` -> `.../sdk/loop_test.rs`（闸门四臂 + notice/ToolResult 事件面）、`.../sdk/protect/reason.rs` -> `.../sdk/protect/reason_test.rs`（内置串五要素） |
| AC-2 | `write_protection.files[]` 语义双端同解：相对 glob 前缀 `**/` 后命中、绝对 glob 原样命中、`reason` 原样透传且 `%s` / `%t` 全量替换、无 reason 落回落串（本仓现存 `plugins/dev-team/bin/knip.json` 即回落串用例）；config 缺失 / 坏 JSON 时内置面仍生效（用户面为空） | `.../sdk/protect/rules.rs` -> `.../sdk/protect/rules_test.rs`（三态 glob + 规则序 + 配置失败面）、`.../sdk/protect/reason.rs` -> `.../sdk/protect/reason_test.rs`（三态选串 + 替换）、`.../sdk/protect/mod_test.rs`（`load` 合并面组合锚） |
| AC-3 | bash 写入面覆盖：重定向族（`>` / `>>` / `>\|` / `tee` / `>&`）、PS 方言（重定向 / 六 cmdlet / `[System.IO.File]::WriteAll*`）、删除族（`rm` / `rmdir` / `unlink` / `del` / `rd` / `Remove-Item` / `git rm`）、双参移动族（旧删 + 新写）、`git restore --source=` / `git checkout <ref> --` 命中保护即拒 | `.../sdk/protect/shell.rs` -> `.../sdk/protect/shell_test.rs`（`extract_file_ops` / `extract_git_ops` 族矩阵）、`.../sdk/protect/mod_test.rs`（`evaluate` bash 臂组合锚） |
| AC-4 | 档 B 五项扩面：`cp` / `dd of=` / `sed -i` / `truncate` / `find … -delete` 命中即拒；`dd if=` 与无 `-i` 的 `sed` 不误拦 | `.../sdk/protect/shell.rs` -> `.../sdk/protect/shell_test.rs`（档 B 五项取位口径 + 两向不误拦） |
| AC-5 | 精度双向断言：`echo foo > notes.txt`、`cat <<EOF > notes.txt`、`echo "rm -rf /" > notes.txt`、`echo a->b`、`2>&1`、heredoc 体里的受保护路径均**不拦**；同一受保护文件的五条等价写法（重定向 / `rm` / `cp` / `node -e` / `sed -i`）**全拦** | `.../sdk/protect/shell.rs` -> `.../sdk/protect/shell_test.rs`（掩码等长 + 排除集 + `$` 丢弃 + fd 复制跳过）、`.../sdk/protect/inline.rs` -> `.../sdk/protect/inline_test.rs`（inline 臂）、`.../sdk/protect/mod_test.rs`（五写法全拦组合锚） |
| AC-6 | 批量还原拦截：`git stash`（除 `list` / `show`）拒；`git clean`（无路径参数或参数命中 `openspec/**`）拒，`-n` / `--dry-run` 放行；`git restore` / `git checkout --` 命中 `openspec/changes/**` 拒，源码目标放行 | `.../sdk/protect/shell.rs` -> `.../sdk/protect/shell_test.rs`（`bulk_revert_detail` 三规则 + `rules::glob_dir_hit` 子树判定）、`.../sdk/protect/rules_test.rs`（`bulk_globs` 派生）、`.../sdk/protect/mod_test.rs`（批量还原组合锚） |
| AC-7 | node / python：`node -e "…writeFileSync('openspec/config.json',…)"` 与 `python -c "open('openspec/config.json','w')…"` 被拒；嵌套 `bash -c 'node -e "…"'` 同拦；裸脚本 `node scripts/setup.js` 放行并留痕为已知漏面 | `.../sdk/protect/inline.rs` -> `.../sdk/protect/inline_test.rs`（JS / PY 表逐项 + 共现不误拦 + 裸脚本放行）、`.../sdk/protect/mod_test.rs`（bash 臂 inline 组合锚）、`.../sdk/loop_test.rs`（`node -e` 拒绝臂） |
| AC-8 | 拒绝串承担掐断换路链：三条串均含「不要改用其他命令或工具尝试绕过」与「无法规避请立即停止并通知用户」两句；`reason` 同时进 notice 载荷与 ToolResult 内容（模型可见） | `.../sdk/protect/reason.rs` -> `.../sdk/protect/reason_test.rs`（逐字断言 + 两句必含）、`.../sdk/loop_test.rs`（notice 载荷与 ToolResult 同串断言） |
| AC-9 | 装配单点：每 run 恰一次 `config::load`（泵起播时）；loop 内零 IO；run 内改 config 不改本轮行为 | `.../sdk/protect/mod_test.rs`（快照语义：旧 `ProtectRules` 判定不随 config 改动变化）、`.../sdk/runner.rs` -> `.../sdk/runner_test.rs`（泵装配面不 panic、未注入仅内置）、源码扫描见 `## 不可测试项` 第 4 项 |
| AC-10 | 主仓口径：change 流程（执行根 = worktree）下规则取自主仓 `config.json`；worktree 内 `openspec/config.json` 仍被内置 glob 拦 | `.../sdk/protect/mod_test.rs` + `.../sdk/protect/rules_test.rs`（主仓 root 装配 + worktree 路径命中双根断言）、`.../sdk/runner.rs` -> `.../sdk/runner_test.rs`（主仓 root 与执行根两值并存在场不 panic）、命令层调用点见 `## 不可测试项` 第 3 项 |
| AC-11 | 既有语义零变：三档 policy 矩阵、路径沙箱逃逸拦截、成员校验、`create_explore_note` 执行面行为逐条不变；未配置保护时普通写操作零拒绝（内置面除外） | `.../sdk/loop.rs` -> `.../sdk/loop_test.rs`（顺序锚 + 放行回归 + 既有 2339 行用例全绿）、`.../infra/agent/src/lib.rs` -> `.../infra/agent/src/lib_test.rs`、`.../infra/agent/src/compose.rs` -> `.../infra/agent/src/compose_test.rs`（尾参补齐后既有断言全绿）、`src/commands/{exec,change_flow}/mod.rs` -> 同名 `mod_test.rs`（命令面回归全绿） |
| AC-12 | 全管线通过：`cargo test --workspace` 全绿；覆盖率不低于阈值（lines ≥ 90 / branches ≥ 85 / functions ≥ 90）；`bindings:check` 零 diff | 本文件全部映射对（逐文件 `#### 用例` 表）；门禁统计面见 `## 不可测试项` 第 9 项 |
| AC-13 | `packages/desktop/package.json` 0.5.28 → 0.5.29；`plugins/**` 零改动零 bump；`openspec/config.json` 零 schema 扩展 | 三项均无单元测试路径 → `## 不可测试项` 第 1、10 项 |

> 被测范围边界：`crates/core/{config,checks,foundation}/**`、`crates/infra/{exec,vcs,checks,store}/**`、`crates/core/{agent,workflow,orchestration}/**`、`sdk/{bash,tools,policy,sandbox,explore_note,context,compact,resume,model_spec,normalize,preamble,log,config}.rs`、`packages/desktop/src/**`、`src/types/generated/bindings.ts`、`openspec/config.json`、`plugins/**` 均为只读红线（design「只读红线零触点」），不纳入被测范围；其行为不变性由既有用例全绿承接（AC-11）。
>
> 机械随动（`loop_test.rs` 8 处 `LoopTurn {}` 字面量补 `protect`、`compose_test.rs` 33 处 + `worker_test.rs` 7 处 `compose_turn` 调用补尾参）不构成源模块变更，见 `## 不可测试项` 第 5 项。

---

## 单元测试

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/mod.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/mod_test.rs

<!-- design.md「公共函数 / API」子表两行（`ProtectRules::load` / `ProtectRules::evaluate`）与「类型定义」子表一行（`ProtectRules`）落本文件；bash 三路判定顺序取「数据模型」段（`extract_file_ops` → `inline::write_targets` → `bulk_revert_detail`）。测试模块以 `#[cfg(test)] mod mod_test;` 在 `protect/mod.rs` 内声明（crate 惯例：含逻辑的 `mod.rs` 配同级 `mod_test.rs`，见 `compose.rs` / `testexec/mod.rs` 先例；与 design「四条 test 声明」的差异与退路见文件头「一处显式补列」）。`ProtectRules` 字段私有但子模块可见，判定用例直构规则集（不走 IO）。 -->

#### 待测功能

- `ProtectRules::load(workspace_root: Option<&Path>) -> ProtectRules`（公共函数表，AC-1/AC-2/AC-9/AC-10/D1/D14/D15）：唯一 IO 点，委托 `rules::load`（内置 + `config::load` 用户规则合并）；`None` → 仅内置；任何配置状态（缺失 / 坏 JSON / 违例）不失败、不产出空规则集
- `ProtectRules::evaluate(&self, tool: &str, input: &Value, cwd: &Path) -> Option<String>`（公共函数表，AC-1/AC-3/AC-7/AC-8/D4/D5/D13）：工具名分派（`write` / `edit` 取 `path`；`bash` 取 `command` + `cwd`；其余臂恒 `None`）；零 IO 零副作用；命中返回**单条**拒绝串（首个命中即止，MUST NOT 拼接多条 reason）
- `ProtectRules`（类型定义表）：`#[derive(Debug, Clone)]`，`rules: Vec<Rule>` 私有、只由 `load` 构造；每 run 一份的纯数据快照（`clone` 后判定等价）
- bash 三路判定次序（「数据模型」段）：`shell::extract_file_ops`（`Path` → 路径判定 / `Tree` → 子树判定）→ `inline::write_targets` → `shell::bulk_revert_detail`；首个命中即返回

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `load` — 未注入 root 仅内置（AC-1/AC-9/D14） | 正向 | `load(None)` → 规则集非空（MUST NOT 为「保护关闭」空集）、条数与 `rules::builtin_rules()` 相等；以 `<tempdir>/openspec/config.json` 绝对路径驱动 `evaluate("write", …)` → `Some`（内置串） | 单值 |
| `load` — 注入 root 合并用户规则（AC-2/D1） | 正向 | tempdir 内按 `foundation::layout::config_path(root)` 落盘两条用户规则（一条相对 + reason、一条绝对 + 无 reason）→ `load(Some(root))` 规则数 == 内置数 + 2；`evaluate("edit", {path: 相对规则解析后的绝对路径})` → `Some`（用户模板替换后串） | 单值 |
| `load` — 配置失败面三形态（AC-2/D15） | 异常 | ①root 目录无配置文件 ②配置文件为坏 JSON（`{`） ③顶层为数组（非对象）→ 三次 `load(Some(root))` 均不 panic、均只含内置规则、内置命中照常 `Some` | 表驱动（3 形态） |
| `load` — root 边界（D15） | 边界 | `Some(Path::new(""))` / `Some(不存在目录)` / `Some(超长含中文与 emoji 的路径)` → 均不 panic；内置规则恒在（`Some` 命中 `<任意前缀>/openspec/config.json`） | 表驱动（3 形态） |
| `load` — 快照语义：run 内改 config 不改本轮判定（AC-9） | 边界 | `load(Some(root))` 得快照 A（config 仅 1 条规则）→ 改写配置文件新增第 2 条规则 → 快照 A 对第 2 条目标判 `None`（不变）；再次 `load(Some(root))` 得快照 B → 同目标 `Some` | 单值 |
| `load` — 主仓口径双根（AC-10/D1/D2） | 边界 | 主仓 root A（配置文件含用户规则 `docs/**` + reason）与 worktree root B（目录内含 `<B>/openspec/config.json` 副本）→ `load(Some(A))` 后：`evaluate("write", {path: "<B>/openspec/config.json"})` → `Some`（内置 glob 不锚定 root，worktree 副本照拦）；`evaluate("write", {path: "<A>/docs/a.md"})` → `Some`（用户规则取自主仓）；`evaluate("write", {path: "<B>/docs/a.md"})` → `Some`（相对 glob 前缀 `**/` 后跨前缀命中）；`evaluate("write", {path: "<B>/src/x.rs"})` → `None` | 表驱动（4 形态） |
| `evaluate` — `write` 命中内置（AC-1） | 正向 | 规则集 = `load(None)`；`input = {path: "<tempdir>/openspec/config.json"}`、`tool = "write"` → `Some(reason)`，reason 为内置串（含路径原文） | 单值 |
| `evaluate` — `edit` 命中用户规则（AC-2） | 正向 | 规则集含用户规则 `**/docs/**` + reason 模板 → `tool = "edit"`、`input = {path: "<root>/docs/a.md"}` → `Some`，`%s` 已替换为该路径、`%t` 为 `edit` | 单值 |
| `evaluate` — 零 IO / 零副作用（AC-9/D1） | 正向 | 判定前后 tempdir 递归快照（路径 + 字节数）完全一致；以「磁盘上不存在的路径」驱动判定同样返回 `Some`（判定不查盘、不 canonicalize） | 单值 |
| `evaluate` — 射程遍历：仅三工具可判（AC-1/D4） | 正向 | 逐名驱动 `read` / `grep` / `glob` / `ls` / `create_explore_note` + 未知名 `write_many`，入参指向受保护路径 → 全部 `None`；同名入参换成 `write` / `edit` / `bash` → 命中臂 `Some` | 遍历枚举（9 工具名） |
| `evaluate` — write / edit 入参异常 fail-open（D15） | 异常 | `input` 为 `{}`（缺 `path`）/ `{path: null}` / `{path: 5}` / `{path: []}` / `{path: {}}` / `{path: true}` → 全部 `None`；`{path: 受保护路径, content: "x", extra: 1}`（多余字段）→ `Some` | 表驱动（6 形态） |
| `evaluate` — bash 入参异常 fail-open（D15） | 异常 | `input` 为 `{}`（缺 `command`）/ `{command: null}` / `{command: 5}` / `{command: ""}` / `{command: "   \n\t"}` → 全部 `None` | 表驱动（5 形态） |
| `evaluate` — Windows 前缀与反斜杠归一（AC-1/D5） | 边界 | `path = "\\\\?\\D:\\ws\\openspec\\config.json"` → `Some`（剥 `\\?\`、`\`→`/` 后命中）；`path = "D:\\ws\\openspec\\config.json"` → `Some`；`path = "D:\\ws\\openspec\\config.jsonx"` → `None` | 表驱动（3 形态） |
| `evaluate` — `..` 段保留仍命中（D5） | 边界 | `path = "<root>/a/../openspec/config.json"`（不消解 `..`）→ `Some`（`**/` 吸收 `..` 段）；`path = "<root>/openspec/./config.json"`（`.` 段消解）→ `Some` | 表驱动（2 形态） |
| `evaluate` — bash 相对路径按 `cwd` 解析（AC-3/D5） | 正向 | `tool = "bash"`、`command = "echo {} > openspec/config.json"`：`cwd = <root>` → `Some`；`cwd = <另一 tempdir>`（该目录下无受保护文件）→ `None` | 表驱动（2 形态） |
| `evaluate` — bash 反斜杠命令归一（D5） | 边界 | `command = "echo {} > openspec\\config.json"` + `cwd = <root>` → `Some`（整串命令先归一） | 单值 |
| `evaluate` — bash 三路组合锚：重定向 / inline / 批量还原（AC-3/AC-6/AC-7） | 正向 | 同一受保护文件 `openspec/config.json` 五条等价写法（`echo {} > …` / `rm …` / `cp /tmp/x …` / `node -e "…writeFileSync('…')"` / `sed -i s/a/b/ …`）+ `git stash` → 每条形各自 `Some`（分别由重定向族 / 删除族 / 档 B `cp` / inline / 档 B `sed -i` / bulk revert 命中） | 表驱动（6 路径） |
| `evaluate` — bash 三路判定次序（数据模型顺序） | 边界 | `node -e "…writeFileSync('docs/a.md','')" && git stash`（inline 与 bulk 同时命中）→ `Some`，串为 inline 路（用户模板替换后）而非批量还原串；`echo {} > docs/a.md && git stash`（files op 与 bulk 同时命中）→ `Some`，串为文件操作路 | 表驱动（2 形态） |
| `evaluate` — Tree 目标（AC-3/D9） | 边界 | `command = "find . -delete"`、`cwd = <root>` → `Some`；`command = "find src -delete"`、`cwd = <root>`（仅有 `src/` 子树）→ `None`；`command = "find . -print"` → `None` | 表驱动（3 形态） |
| `evaluate` — 精度双向：不误拦（AC-5） | 异常 | `echo foo > notes.txt`（`cwd` 任意）/ `cat <<EOF > notes.txt` / `echo "rm -rf /" > notes.txt` / `echo a->b` / `ls 2>&1` → 全部 `None`；同一规则集下把 `notes.txt` 换成 `openspec/config.json` → 前三条 `Some`（同装置对照） | 表驱动（5 形态） |
| `evaluate` — 首个命中即止、MUST NOT 拼接（AC-8/D3/D13） | 边界 | 两条用户规则（各自 reason 模板）同时命中同一路径（内置不命中）→ 返回串恰等于首条模板全文（不包含第二条模板片段）；内置与用户规则同时命中 → 返回内置串（内置在首位） | 表驱动（2 形态） |
| `evaluate` — 畸形/超长输入不 panic（D15） | 边界 | `command` 为 100 KB 重复片段（内含 1 处受保护写入）→ 返回 `Some` 且耗时不挂起；`command` 含 emoji / 中文 / `\0` → 不 panic；`path` 为 1000+ 字符 → 不 panic | 参数化（3 形态） |
| `evaluate` — 入参无档位量（AC-1/D13） | 边界 | 签名 `evaluate(&self, tool, input, cwd)` 三参不含 permission-mode，故同一规则集与入参下判定结果与档位无关联（Bypass / AcceptEdits 两档的闸门命中由 `loop_test` 档位矩阵承接；Default 档由 policy 先行，见 `## 不可测试项` 第 6 项） | 表驱动（2 档） |
| `ProtectRules` — 类型面（类型定义表） | 边界 | `clone()` 后对同一入参判定等价（同 `Option<String>` 逐字节相等）；`Debug` 渲染可读（含规则数）；直构 `ProtectRules { rules: vec![] }`（空规则集）判定恒 `None`（防御性形态，生产不产生） | 表驱动（3 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（判定面） | 零 mock：`evaluate` 用例以 `ProtectRules { rules: vec![…] }` 直构（被测 API 的显式入参）+ `rules::load(None)` 产出的内置快照驱动，纯内存零 IO | 全部 `evaluate` 用例 |
| 配置文件（进程边界：数据文件） | 不 mock：`tempfile::TempDir` 真实目录 + 按 `foundation::layout::config_path(root)` 派生的真实配置文件（内容为 JSON 字面量），`config::load` 真实读取 | 全部 `load` 用例与「快照语义」用例 |
| 文件系统写面 | 不 mock：受保护文件以真实落盘字节参与（判定前后快照比对，证零写入）；真实目录树由 `tempfile` RAII 清理 | 「零 IO / 零副作用」与「不误拦」对照用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/rules.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/rules_test.rs

<!-- design.md「公共函数 / API」子表八行（`rules::load` / `builtin_rules` / `user_rules` / `bulk_globs` / `glob_dir_hit` / `normalize` / `resolve` / `Rule.concrete_prefix`）与「类型定义」子表三行（`Rule` / `ReasonKind` / `BulkGlobs`）落本文件。断言一律经 `checks::globmatch::match_glob`（D7 方言单点，测试侧不自造第二份匹配），派生断言经 `foundation::layout::{config_path, domain_dir_name, resolve}` 复算，源码内不出现目录名 / 文件名字面量（D2）。 -->

#### 待测功能

- `rules::load(workspace_root: Option<&Path>) -> Vec<Rule>`（公共函数表，AC-1/AC-2/D1）：规则合并单点——`builtin_rules()` 在前，`Some(root)` 时 `user_rules(root)` 追加在后；`None` → 仅内置
- `rules::builtin_rules() -> Vec<Rule>`（公共函数表，AC-1/D2）：恒一条，glob = `**/` + `config_path(Path::new(""))` 的 root 相对形态（归一后），reason = `ReasonKind::BuiltinConfig`
- `rules::user_rules(root: &Path) -> Vec<Rule>`（公共函数表，AC-2/D2/D15）：逐条取 `write_protection.files[]`——`glob` 为 `None` 跳过、绝对原样、相对前缀 `**/`、`reason` 落 `ReasonKind::User(Option<String>)`
- `rules::bulk_globs() -> BulkGlobs`（公共函数表，AC-6/D2）：两条结构 glob（域目录树 `artifacts` / changes 子树 `changes`）由 `foundation::layout` 派生
- `rules::glob_dir_hit(dir: &str, glob: &str, anchor: &Path) -> bool`（公共函数表，AC-3/AC-6/D9）：子树相交判定——glob 折成 concrete 前缀后判「前缀 == dir」「前缀以 `dir/` 起手」「dir 以 `前缀/` 起手」三面之一
- `rules::normalize(raw: &str) -> String`（公共函数表，D5/D6/D9）：剥 `\\?\` → `\`→`/` → 去空段与 `.` 段、保留 `..` 段 → 去尾分隔符、保留前导 `/`
- `rules::resolve(anchor: &Path, raw: &str) -> Option<String>`（公共函数表，D5/D15）：空串或含 `$` → `None`；绝对原样归一、相对 `anchor.join` 后归一
- `Rule::concrete_prefix(&self, anchor: &Path) -> String`（公共函数表，D9）：glob → concrete 静态前缀（`**/` 起手者锚定到 `anchor`）
- `Rule` / `ReasonKind` / `BulkGlobs`（类型定义表）：`Rule { glob, reason }`；`ReasonKind::{BuiltinConfig, User(Option<String>)}`；`BulkGlobs { artifacts, changes }`；均 `Debug, Clone`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `builtin_rules` — 恰一条且派生正确（AC-1/D2） | 正向 | `len() == 1`；`glob == format!("**/{}", normalize(&config_path(Path::new("")).to_string_lossy()))`；`reason == ReasonKind::BuiltinConfig`（派生复算，源码零字面量） | 单值 |
| `builtin_rules` — 任意前缀命中（AC-1/D7） | 正向 | 内置 glob 对以下路径 `match_glob` 均 `true`：Unix 风格绝对 `/ws/openspec/config.json`、盘符形态 `D:/ws/openspec/config.json`、root 相对 `openspec/config.json`（`**/` 零段命中）、深层 `<a>/<b>/<c>/openspec/config.json` | 表驱动（4 形态） |
| `builtin_rules` — 近似形态不命中（AC-1） | 异常 | 对 `<root>/openspec/config.json.bak`、`<root>/openspec/config.json/x`、`<root>/openspec/explores/config.json`、`<root>/config.json`、`<root>/openspec/config.JSON`（大小写敏感）→ 均 `false` | 表驱动（5 形态） |
| `load` — `None` 仅内置（AC-9/D14） | 正向 | `load(None)` → 条数 == `builtin_rules().len()`；首条 `ReasonKind::BuiltinConfig` | 单值 |
| `load` — `Some(root)` 内置在前（AC-2/D3） | 正向 | tempdir 内 config 含 2 条用户规则 → 条数 == 3；`rules[0].reason == BuiltinConfig`、`rules[1..]` 顺序与配置出现序一致 | 单值 |
| `load` — 配置缺失 / 坏 JSON 仅减用户面（AC-2/D15） | 异常 | ①无配置文件 ②`"{"`（语法非法）③`"[]"`（顶层非对象）④`{"write_protection": "x"}` ⑤`{"write_protection": {"files": {}}}` → 均仅内置、不 panic | 表驱动（5 形态） |
| `load` — root 异常形态（D15） | 边界 | `Some(Path::new(""))`（相对 cwd 读）、`Some(不存在目录)`、`Some(超长路径)` → 不 panic，内置恒在 | 表驱动（3 形态） |
| `user_rules` — 相对 glob 前缀 `**/`（AC-2/D2） | 正向 | 配置写作 `{"glob": "plugins/dev-team/bin/knip.json"}`（本仓现行真实规则原文）→ 产出一条 `glob == "**/plugins/dev-team/bin/knip.json"`；`match_glob` 对 `<root>/plugins/dev-team/bin/knip.json` 与 `<另一前缀>/plugins/dev-team/bin/knip.json` 均 `true`，对 `<root>/plugins/dev-team/bin/knip.json5` `false` | 单值 |
| `user_rules` — 绝对 glob 原样（AC-2） | 正向 | 配置写作 `{"glob": "<tempdir 绝对路径>/secret.md"}` → 产出 glob 与原文逐字符相等（不加 `**/`）；命中该绝对路径，不命中 `<另一 tempdir>/secret.md` | 单值 |
| `user_rules` — 无 reason 落 `User(None)`（AC-2） | 正向 | 配置写作 `{"glob": "plugins/dev-team/bin/knip.json"}`（无 reason，本仓真实形态）→ `reason == ReasonKind::User(None)`；同配置加 `"reason": "金样冻结"` → `User(Some("金样冻结"))` | 表驱动（2 形态） |
| `user_rules` — `glob` 为 `None` 跳过（AC-2/D15） | 边界 | 配置写作 `{"files": [{"reason": "只给 reason"}, {"glob": "docs/**", "reason": "文档冻结"}]}`（首条 glob 缺省 → `WriteProtectionFile.glob == None`）→ 仅 1 条规则产出（首条跳过，不 panic） | 单值 |
| `user_rules` — 空串 / 非法 glob（AC-2/D15） | 异常 | 配置写作 `{"files": [{"glob": ""}, {"glob": 5}, {"glob": "docs/**"}]}`（前两条由 `config` 组装层剔除）→ 仅 `docs/**` 一条；另以直构 `Rule { glob: "[".to_owned(), .. }` 驱动 `match_glob` → 不命中、不 panic（非法模式 fail-open） | 表驱动（2 形态） |
| `user_rules` — glob 顺序与非折叠（AC-2/D3） | 正向 | 配置含 4 条（相对 / 绝对 / 相对重复 / 带 reason）→ 产出 4 条、序与配置一致、重复条不折叠不去重 | 单值 |
| `user_rules` — glob 边界字符（D15） | 边界 | glob 为全空白 `"   "` / 含中文与 emoji 的路径模式 / 1000+ 字符超长模式 / 含 `$` 的模式 → 均不 panic；全空白与超长模式经 `match_glob` 不误命中常规路径 | 参数化（4 形态） |
| `bulk_globs` — 两条派生正确（AC-6/D2） | 正向 | `artifacts == format!("**/{}", domain_dir_name()) + "/**"`（派生复算）；`changes == format!("**/{}", normalize(&resolve(Path::new("")).changes_root.to_string_lossy())) + "/**"` | 单值 |
| `bulk_globs` — 命中面与窄面（AC-6） | 正向 | `changes` glob 命中 `<root>/openspec/changes/x/proposal.md`，不命中 `<root>/openspec/explores/a.md`；`artifacts` 命中两者（子集关系断言：`changes` 命中集 ⊆ `artifacts` 命中集） | 表驱动（3 形态） |
| `glob_dir_hit` — `.` 搜索根命中（AC-3/D9） | 正向 | `dir = "."`、`anchor = <root>`、glob = 内置 config glob → `true`（`find . -delete` 场景；`.` 归一后为空段，落到 anchor 锚定面） | 单值 |
| `glob_dir_hit` — 源码子树不越拦（AC-5/D9） | 异常 | 同上 anchor 与 glob，`dir = "<root>/src"` → `false`；`dir = "<root>/openspecx"` → `false`（段边界：`openspecx` 不以 `openspec/` 起手） | 表驱动（2 形态） |
| `glob_dir_hit` — 祖先 / 相等 / 后代三面（D9） | 正向 | 同 anchor 与 glob：`dir = "<root>"`（祖先面）→ `true`；`dir = "<root>/openspec"`（前缀面）→ `true`；`dir = "<root>/openspec/config.json"`（相等面，搜索根即目标）→ `true` | 表驱动（3 形态） |
| `glob_dir_hit` — 绝对 glob 与 anchor 无关 | 正向 | glob = `D:/ws/docs/**`（无 `**/` 起手）：`dir = "D:/ws"` → `true`；`dir = "D:/other"` → `false`；`dir` 传反斜杠形态 `D:\\ws` 归一后同样 `true` | 表驱动（3 形态） |
| `glob_dir_hit` — 无静态段退化（D9/D15） | 边界 | glob = `**/*`（`**/` 起手但零静态段）：`dir = <root 下任意子目录>` → `true`、`dir = <anchor 外目录>` → `false`；glob = `""` / `"**/"` / `"["` → `false`（不 panic） | 表驱动（5 形态） |
| `glob_dir_hit` — 目录参数边界（D15） | 边界 | `dir = ""` / `"   "` / 1000+ 字符 / 含 emoji → 不 panic；空串与全空白不误判为命中（`false`） | 参数化（4 形态） |
| `normalize` — Windows 前缀与分隔符（D5） | 正向 | `"\\\\?\\D:\\ws\\openspec\\config.json"` → `"D:/ws/openspec/config.json"`；`"a\\b\\c"` → `"a/b/c"`；`"a//b///c"` → `"a/b/c"` | 表驱动（3 形态） |
| `normalize` — `.` 消解 / `..` 保留（D5/D9） | 正向 | `"a/./b"` → `"a/b"`；`"."` → `""`；`"./a"` → `"a"`；`"a/../b"` → `"a/../b"`（不消解）；`"a/.."` → `"a/.."` | 表驱动（5 形态） |
| `normalize` — 前导 / 尾部 / 根（D5） | 边界 | `"/a/b"` → `"/a/b"`（前导 `/` 保留）；`"a/b/"` → `"a/b"`；`"/"` → `"/"`；`"///"` → `"/"`；`"D:/"` → `"D:/"` | 表驱动（5 形态） |
| `normalize` — 空与特殊字符（D6） | 边界 | `""` → `""`；含中文 / emoji / 空格 / `\t` 的路径逐字符保留；1000+ 字符输入不 panic；幂等：`normalize(normalize(x)) == normalize(x)`（3 组输入） | 参数化（5 形态） |
| `resolve` — 绝对原样归一（D5） | 正向 | `resolve(<anchor>, "D:\\ws\\a.md")` → `Some("D:/ws/a.md")`；`resolve(<anchor>, "/x/y.md")` → `Some("/x/y.md")` | 表驱动（2 形态） |
| `resolve` — 相对拼 anchor（D5） | 正向 | `resolve(<root>, "openspec/config.json")` → `Some("<root 归一>/openspec/config.json")`；`resolve(<root>, "./a/./b.txt")` → `Some("<root 归一>/a/b.txt")`；`resolve(<root>, "../x.txt")` → `Some("<root 归一的父>/x.txt")`（`..` 段保留后拼） | 表驱动（3 形态） |
| `resolve` — 放行面（D15） | 异常 | `resolve(<root>, "")` → `None`；`resolve(<root>, "$HOME/x")` → `None`；`resolve(<root>, "${P}/x")` → `None`；`resolve(<root>, "%TEMP%\\x")` → `Some`（非 `$` 形态，按字面解析；命中与否由规则决定） | 表驱动（4 形态） |
| `resolve` — 边界字符（D15） | 边界 | 1000+ 字符相对路径 / 含 emoji / 含包裹引号 `'a b.txt'`（原样参与归一，不 panic） | 参数化（3 形态） |
| `concrete_prefix` — `**/` 起手锚定 anchor（D9） | 正向 | `Rule { glob: "**/" + normalize(config_path("")) }` + `anchor = <root>` → `prefix == "<root 归一>/" + normalize(config_path(""))`（`**/` 段被 anchor 置换） | 单值 |
| `concrete_prefix` — 绝对 glob 与无通配（D9） | 正向 | `glob = "D:/ws/secret.md"`（绝对）→ `prefix` 即自身静态段；`glob = "**/plugins/x/knip.json"` + anchor → `"<root>/plugins/x/knip.json"`（无通配段保留全段） | 表驱动（2 形态） |
| `concrete_prefix` — 通配截断与退化（D9） | 边界 | `glob = "**/a/**/*.md"` + anchor → `"<root>/a"`（首个通配段前截断）；`glob = "**/*"` / `glob = "**/"` → `anchor` 归一形态；`glob = ""` → 不 panic | 表驱动（4 形态） |
| `Rule` / `ReasonKind` / `BulkGlobs` — 类型面（类型定义表） | 边界 | `Rule` 直构 `{ glob, reason }` 与 `Debug` / `clone` 后逐字段相等；`ReasonKind` 三形态（`BuiltinConfig` / `User(Some)` / `User(None)`）逐个构造与相等比较；`BulkGlobs` 两字段可读且 `clone` 等价 | 遍历枚举（3 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 配置文件（进程边界：数据文件） | 不 mock：`tempfile::TempDir` 真实目录 + `foundation::layout::config_path(root)` 派生的真实 JSON 文件；`config::load` 真实读取（含坏 JSON / 缺失 / 违例形态） | `load` / `user_rules` 全部用例 |
| glob 方言 | 不 mock：一律真实调用 `checks::globmatch::match_glob`（D7 单点；测试不引入第二份匹配实现） | 全部命中/不命中断言 |
| 文件系统其余面 | 不 mock：`normalize` / `resolve` / `glob_dir_hit` / `concrete_prefix` 为纯文本函数，零 IO 零时钟 | 全部纯函数用例 |


---

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/shell.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/shell_test.rs

<!-- design.md「公共函数 / API」子表七行（`shell::extract_file_ops` / `extract_git_ops` / `mask_heredocs` / `mask_quoted_and_heredoc` / `tokenize` / `is_flag` / `bulk_revert_detail`）与「类型定义」子表三行（`FileOp` / `OpKind` / `OpTarget`）落本文件；用例矩阵逐条重译自插件端 `plugins/dev-team/bin/src/lib/shell-file-ops.ts`（掩码 / 排除集 / 各族正则）并按档 B 补五项（D8）+ 批量还原三规则（AC-6）。「提取」用例断言 `Vec<FileOp>` 的**形状与顺序**（不判命中），命中判定归 `protect/mod.rs`（关注点分离）；`Vec<char>` 索引纪律（D6）以非 ASCII 命令文本单独覆盖。 -->

#### 待测功能

- `shell::extract_file_ops(command: &str) -> Vec<FileOp>`（公共函数表，AC-3/AC-4/AC-5/D7/D8）：三态提取总入口——重定向族（`>` / `>>` / `>\|` / `tee` / `>&`）、PS 方言（重定向 / 六 cmdlet / `[System.IO.File]::WriteAll*`）、删除族（`rm` / `rmdir` / `unlink` / `del` / `rd` / `Remove-Item` / `git rm`）、双参移动族（旧删 + 新写）、档 B 五族（`cp` / `Copy-Item` / `dd of=` / `sed -i`·`perl -i` / `truncate` / `find -delete`）、git 子命令；保序、不折叠、不去重
- `shell::extract_git_ops(command: &str) -> Vec<FileOp>`（公共函数表，AC-3/D8）：`git restore` / `git checkout` / `git rm` 的 write / revert / delete ops（`--source=` / `-s` / `<ref> --` 决定 write）
- `shell::mask_heredocs(command: &str) -> String`（公共函数表，AC-5/D6）：变体 A——只掩 heredoc 体、保留引号；与原文等长（逐字符替换）
- `shell::mask_quoted_and_heredoc(command: &str) -> String`（公共函数表，AC-5/D6）：变体 B——掩 heredoc + 引号串（命令名定位面）；与变体 A 等长
- `shell::tokenize(args: &str) -> Vec<String>`（公共函数表，AC-6/D8）：空白切分 + 剥包裹引号 + 剔空；**不做** `$` 过滤（`git clean` 零路径判定依赖原始 token 计数）
- `shell::is_flag(token: &str) -> bool`（公共函数表，D8）：`-` 起手判定
- `shell::bulk_revert_detail(command: &str, globs: &BulkGlobs, cwd: &Path) -> Option<String>`（公共函数表，AC-6/D9）：命中即返回**中间事实串**（如「git stash 会批量改写工作区」）；`cwd` 仅作 `git clean` 目录参数与 `git restore` 目标的子树判定锚
- `FileOp` / `OpKind` / `OpTarget`（类型定义表）：`FileOp { kind, target }`；`OpKind::{Write, Delete, Revert}`（`Revert` 不进保护判定）；`OpTarget::{Path(String), Tree(String)}`（`Tree` 走子树相交判定）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `mask_heredocs` — heredoc 体掩码（AC-5/D6） | 正向 | `"cat <<EOF\nrm -rf /\nEOF"` → 掩码后不含 `rm -rf /`（体内逐字符替换），`EOF` 结束行之后内容保留；`"echo \"a b\""` → 引号与内容逐字符保留（变体 A 只掩 heredoc） | 表驱动（2 形态） |
| `mask_heredocs` — 等长与位置 1:1（AC-5/D6） | 正向 | 对无 heredoc / 单 heredoc / 双 heredoc / 含中文与 emoji 的 4 组命令：`chars().count()` 与原文相等，且掩码后第 k 个字符与原文第 k 个字符「同位置」（首尾字符逐一对照） | 表驱动（4 形态） |
| `mask_heredocs` — 多 heredoc 与缩进形态（D6） | 边界 | `<<A … A … <<B … B` 两段均被掩；`<<-EOF`（tab 缩进结束符）同掩；`cat <<EOF`（未闭合）掩到末尾且不 panic | 表驱动（3 形态） |
| `mask_heredocs` — 引号内的 `<<` 不越界（D6） | 边界 | `"echo \"<<EOF\""` → 不 panic、与原文等长（是否被判为 heredoc 起始按插件端 `quoteSpans` 先行口径，用例只锚定等长与不 panic）；空串 → 空串 | 表驱动（2 形态） |
| `mask_quoted_and_heredoc` — 引号掩码（AC-5） | 正向 | `"echo \"rm -rf /\" > x"` → 变体 B 中引号内 `rm -rf /` 被掩（命令名定位不到 `rm`），变体 A 中保留；两变体在引号区之外逐字符一致 | 单值 |
| `mask_quoted_and_heredoc` — 引号族与畸形（D6） | 边界 | 单引号 / 双引号 / 反引号 / PS 单双引号各一形态被掩；未闭合引号掩到末尾；转义引号 `\"` 形态 → 全部与原文等长、不 panic | 表驱动（6 形态） |
| 两变体 — 等长互通（AC-5/D6） | 正向 | 对 6 组命令（含 heredoc、引号、中文、emoji、1000+ 字符）：`mask_heredocs(c).chars().count() == mask_quoted_and_heredoc(c).chars().count() == c.chars().count()`（索引 1:1 互通） | 表驱动（6 形态） |
| `tokenize` — 切分与剥引号（D8） | 正向 | `"a b" c` → `["a b", "c"]`；`"'x y' z"` → `["x y", "z"]`；`"  a\t b\n c  "` → `["a", "b", "c"]`（多空白 / tab / 换行剔空） | 表驱动（3 形态） |
| `tokenize` — 不做 `$` 过滤（AC-6/D8） | 边界 | `"git clean $TARGET"` → `["git", "clean", "$TARGET"]`（原始 token 计数保留——零路径判定依赖此口径）；`""` 与全空白 → `[]` | 表驱动（3 形态） |
| `tokenize` — 边界字符（D15） | 边界 | 引号内中文 / emoji / 空格原样保留；未闭合引号剥到末尾不 panic；单个 10 KB 长 token 不 panic | 参数化（4 形态） |
| `is_flag` — 两向（D8） | 正向 | `-n` / `--dry-run` / `-i.bak` / `-s` / `-` → `true`；`a` / `openspec` / `./a` / `""` / 中文 / emoji → `false` | 表驱动（10 形态） |
| `extract_file_ops` — 重定向族（AC-3） | 正向 | `echo {} > p` → `[Write(Path "p")]`；`echo {} >> p` → `[Write(Path "p")]`；`echo {} >\| p` → `[Write(Path "p")]`；`echo {} \| tee p` → `[Write(Path "p")]`；`echo {} &> p` → `[Write(Path "p")]`（照抄 `[^-=]` 排除集，MUST NOT 反向"修复"） | 表驱动（5 形态） |
| `extract_file_ops` — fd 复制跳过（AC-5/D7） | 异常 | `foo 2>&1` / `foo 1>&2` / `foo 3>&2` / `foo >&2` → 全部 `[]`（目标 token 为纯 ASCII 数字即丢弃该 op） | 表驱动（4 形态） |
| `extract_file_ops` — 箭头与等号不误判（AC-5/D7） | 异常 | `echo a->b` / `node -e "x => p"` / `echo a => p` → 全部 `[]`（`[^-=]` 前缀守卫） | 表驱动（3 形态） |
| `extract_file_ops` — heredoc 与引号内是数据（AC-5） | 异常 | `cat <<EOF > notes.txt\nrm -rf /\n> ignored.txt\nEOF` → 仅 `[Write(Path "notes.txt")]`（体内 `rm` 与 `> ignored.txt` 不算）；`echo "rm -rf /" > notes.txt` → 仅 write，零 delete | 表驱动（2 形态） |
| `extract_file_ops` — `$` 落点丢弃（AC-5/D15） | 异常 | `echo x > $OUT` → `[]`；`rm $TARGET` → `[]`；`echo x > ${OUT}` → `[]`；`cp $SRC dst` → `[Write(Path "dst")]`（末参为字面量，源被丢） | 表驱动（4 形态） |
| `extract_file_ops` — 删除族（AC-3） | 正向 | `rm p` / `rm -rf p` / `rmdir p` / `unlink p` / `del p` / `rd /s /q p` / `Remove-Item -Recurse p` / `git rm p` → 均含 `Delete(Path "p")`（flag 跳过） | 表驱动（8 形态） |
| `extract_file_ops` — 多目标与保序（AC-3） | 边界 | `rm a b` → `[Delete(a), Delete(b)]`（保序）；`rm p p` → 两条（不去重）；`rm -f` → `[]`（只有 flag）；`rm a && rm b` → `[Delete(a), Delete(b)]` 保序 | 表驱动（4 形态） |
| `extract_file_ops` — 移动族双条（AC-3） | 正向 | `mv a b` → `[Delete(a), Write(b)]`（旧删 + 新写）；`ren a b` / `Rename-Item a b` / `Move-Item a b` → 同双条；`mv -f a b` → 同双条（flag 跳过） | 表驱动（4 形态） |
| `extract_file_ops` — 移动族单参不猜（D8） | 边界 | `mv p`（单参）/ `mv`（零参）→ `[]`（不猜目标） | 表驱动（2 形态） |
| `extract_file_ops` — PS 重定向与六 cmdlet（AC-3） | 正向 | PS 重定向 `2> p` / `*> p` → `[Write(Path "p")]`；`Set-Content p` / `Out-File p` / `Add-Content p` / `Export-Csv p` / `Export-CliXml p` / `Tee-Object p` → 各一条 `Write(Path "p")`；`Set-Content -Path p` → `[Write(Path "p")]` | 表驱动（9 形态） |
| `extract_file_ops` — `[System.IO.File]::WriteAll*`（AC-3） | 正向 | `[System.IO.File]::WriteAllText("p", "x")` / `WriteAllLines("p", $x)` / `WriteAllBytes("p", $b)` / `AppendAllText("p", "x")` → 各 `[Write(Path "p")]`；首参为 `$var` → `[]`（`is_literal_path` 丢弃）；`[System.IO.File]::ReadAllText("p")` → `[]` | 表驱动（6 形态） |
| `extract_file_ops` — `cp` 族目标位（AC-4/D8） | 正向 | `cp a p` → `[Write(Path "p")]`；`cp -r a p` → `[Write(Path "p")]`；`cp a b p`（多源）→ `[Write(Path "p")]`（末个非 flag 非 `$` token）；`Copy-Item -Recurse a p` → `[Write(Path "p")]` | 表驱动（4 形态） |
| `extract_file_ops` — `dd` 的 `of=` 与 `if=`（AC-4/D8） | 正向 | `dd if=/tmp/x of=p` → `[Write(Path "p")]`；`dd if=p of=/tmp/x` → `[Write(Path "/tmp/x")]`（`if=` 显式跳过，`p` 零 op） | 表驱动（2 形态） |
| `extract_file_ops` — `sed -i` / `perl -i` 分界（AC-4/D8） | 正向 | `sed -i 's/a/b/' p` → `[Write(Path "p")]`；`sed -i.bak 's/a/b/' p` → `[Write(Path "p")]`；`perl -i -pe 's/a/b/' p` → `[Write(Path "p")]`；`sed 's/a/b/' p`（无 `-i`）→ `[]`；`perl -pe … p` → `[]`（纯读 MUST NOT 拦） | 表驱动（5 形态） |
| `extract_file_ops` — `truncate` 取位（AC-4/D8） | 正向 | `truncate -s 0 p` → `[Write(Path "p")]`；`truncate --size=0 p` → `[Write(Path "p")]`；`truncate p`（无 `-s`）→ `[]` | 表驱动（3 形态） |
| `extract_file_ops` — `find -delete` 搜索根（AC-4/D9） | 正向 | `find . -delete` → `[Delete(Tree ".")]`；`find <root> -name '*.rs' -delete` → `[Delete(Tree "<root>")]`（首个非 flag token 为搜索根）；`find . -print` → `[]` | 表驱动（3 形态） |
| `extract_file_ops` — 畸形输入（D15） | 异常 | `""` / 全空白 / 只有 flag（`-rf`）/ 未闭合引号 / 未闭合 heredoc / 只有分隔符 `;;&&` → `[]` 或有限 op，均不 panic | 表驱动（6 形态） |
| `extract_file_ops` — 非 ASCII 与超长（D6/D15） | 边界 | `"echo 中文路径🚀 > p"` → `[Write(Path "p")]`（char 索引不错位，`>` 之后取对）；`"echo 🚀 > 中文.txt"` → `[Write(Path "中文.txt")]`；100 KB 命令（尾部 1 处重定向）→ 返回该 op 且耗时可接受 | 参数化（3 形态） |
| `extract_git_ops` — `restore` 两态（AC-3/D8） | 正向 | `git restore --source=HEAD~1 p` → `[Write(Path "p")]`；`git restore -s HEAD p` → `[Write(Path "p")]`；`git restore p`（无 source）→ `[Revert(Path "p")]` | 表驱动（3 形态） |
| `extract_git_ops` — `checkout` 两态（AC-3） | 正向 | `git checkout <ref> -- p` → `[Write(Path "p")]`；`git checkout -- p` → `[Revert(Path "p")]`；`git checkout main`（无 `--`）→ `[]` | 表驱动（3 形态） |
| `extract_git_ops` — `rm` 与纯读子命令（AC-3） | 正向 | `git rm p` → `[Delete(Path "p")]`；`git status` / `git diff p` / `git log --oneline` → `[]` | 表驱动（4 形态） |
| `extract_git_ops` — 引号内 git 不算命令（AC-6） | 异常 | `echo "git restore p"` → `[]`；`printf 'git rm p'` → `[]`（命令名在变体 B 定位）；同一命令后接真实 `&& git rm p` → `[Delete(Path "p")]`（变体 B 掩码后仍可定位） | 表驱动（3 形态） |
| `bulk_revert_detail` — `git stash` 族（AC-6） | 正向 | `git stash` / `git stash push` / `git stash pop` / `git stash apply` / `git stash drop` → 各 `Some(detail)` 且 detail 含 `stash` | 表驱动（5 形态） |
| `bulk_revert_detail` — `git stash` 查看放行（AC-6） | 异常 | `git stash list` / `git stash show` / `git stash list -p` → `None` | 表驱动（3 形态） |
| `bulk_revert_detail` — `git clean` 零路径（AC-6） | 正向 | `git clean` / `git clean -fd` / `git clean -fdx` → 各 `Some(detail)`（无路径参数即拒，detail 含 `clean`） | 表驱动（3 形态） |
| `bulk_revert_detail` — `git clean` dry-run 放行（AC-6） | 异常 | `git clean -n` / `git clean --dry-run` / `git clean -nd`（沿插件端 `^-[^-]*n` 口径照译）→ 全部 `None` | 表驱动（3 形态） |
| `bulk_revert_detail` — `git clean` 路径命中面（AC-6/D9） | 正向 | `git clean -fd <cwd>/openspec` → `Some`（目录参数子树命中）；`git clean -fd .`（cwd = 主仓根）→ `Some`（`.` 归一后锚定 cwd）；`git clean -fd src`（cwd 下仅源码子树）→ `None`（源码路径放行） | 表驱动（3 形态） |
| `bulk_revert_detail` — `cwd` 锚定差异（AC-6/D9） | 边界 | 同一 `git clean -fd openspec` 命令：`cwd = <含该子树的 tempdir>` → `Some`；`cwd = <另一无该子树的 tempdir>` → `None`（同一命令两 cwd 对照，内部零 IO） | 表驱动（2 形态） |
| `bulk_revert_detail` — `restore` / `checkout --` changes 面（AC-6/D9） | 正向 | `git restore <cwd>/openspec/changes/x/proposal.md` → `Some`；`git checkout -- <cwd>/openspec/changes/x/a.md` → `Some`；`git checkout main -- openspec/changes/x/a.md`（ref 形态）→ `Some` | 表驱动（3 形态） |
| `bulk_revert_detail` — 源码目标放行（AC-6） | 异常 | `git restore src/a.rs` / `git checkout -- packages/desktop/src/a.ts` → 均 `None`（规范撤销动作） | 表驱动（2 形态） |
| `bulk_revert_detail` — 引号内与畸形（D15） | 异常 | `echo "git stash"` / `echo 'git clean -fd'` → `None`（变体 B 定位不到命令名）；`""` / 全空白 / 100 KB 无命中内容的命令 → `None` 且不 panic | 表驱动（5 形态） |
| `bulk_revert_detail` — 多命中取首个（D13） | 边界 | `git stash && git clean -fd` → `Some`，detail 为 `stash` 一路（首个命中即返回，不拼接两段事实串） | 单值 |
| `FileOp` / `OpKind` / `OpTarget` — 类型面（类型定义表） | 边界 | `FileOp` 直构两态（`Write`+`Path` / `Delete`+`Tree`）与 `PartialEq` 自反、`clone` 等价；`OpKind` 三值 / `OpTarget` 两值逐个构造比较；`Debug` 渲染含变体名 | 遍历枚举（5 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 | 零 mock：全部用例以命令字符串字面量为**被测 API 显式入参**，纯文本函数零 IO / 零子进程 / 零时钟；`cwd` 以 `Path::new("…")` 字面量传入（不建目录、不读盘——`bulk_revert_detail` 契约即零 IO） | 全部用例 |
| glob（`BulkGlobs` 与子树判定） | 不 mock：`bulk_revert_detail` 用例以 `rules::bulk_globs()` 真实派生值传入（`BulkGlobs` 是被测 API 的显式入参，属注入依赖），`glob_dir_hit` 真实组合 | `git clean` / `restore` 命中与放行用例 |
| 文件系统 | 不涉及：本模块不读盘不写盘（design 组件表「`Vec<char>` 索引 + 线性正则」） | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/inline.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/inline_test.rs

<!-- design.md「公共函数 / API」子表一行（`inline::write_targets`）落本文件；用例矩阵对应 AC-7 与 spec「node / python 内联脚本写入判别」四个 Scenario（内联命中 / 嵌套覆盖 / 裸脚本放行 / 共现不误拦）+ JS·PY 双 API 表逐项（D10）。插件端 `^(python|node)\s` 整体豁免为**反向断言对象**（MUST NOT 复刻）。 -->

#### 待测功能

- `inline::write_targets(command: &str) -> Vec<String>`（公共函数表，AC-7/D10）：按 JS / PY 写 API 表逐条正则取**第一个字符串实参**为候选路径（不判命中）；对整条命令文本扫描、不切脚本段（故嵌套 `bash -c 'node -e "…"'` / `pwsh -Command "…"` 一并覆盖）；裸脚本 / `-m` / `--check` 形态零 op

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `write_targets` — JS 表逐项（AC-7/D10） | 正向 | `node -e "require('fs').writeFileSync('p','{}')"` → 含 `"p"`；`writeFile` / `appendFile` / `appendFileSync` / `createWriteStream` / `copyFileSync` / `copyFile` / `rename` / `renameSync` / `unlink` / `unlinkSync` / `rm` / `rmSync` / `fs.promises.writeFile` 逐 API 各一条命令 → 各返回含 `"p"` 的一条 | 表驱动（13 API） |
| `write_targets` — JS `open` 族模式判定（D10） | 正向 | `require('fs').openSync('p','w')` / `'a'` / `'wx'` → 各含 `"p"`；`openSync('p','r')`（只读）→ `[]` | 表驱动（4 形态） |
| `write_targets` — PY 表逐项（AC-7/D10） | 正向 | `python -c "open('p','w').write('{}')"` → 含 `"p"`；`open(p,'a')` / `'x'` / `'wb'` / `'w+'` → 各含 `"p"`；`open(p,'r')` → `[]`；`Path('p').write_text('x')` / `Path('p').write_bytes(b'x')` → 各含 `"p"`；`shutil.copy('src','p')` / `copy2` / `copyfile` / `shutil.move('src','p')` → 各取**首个字符串实参**；`os.remove('p')` / `os.unlink('p')` / `os.rename('src','p')` / `os.replace('src','p')` → 各取首参 | 表驱动（18 API 形态） |
| `write_targets` — copy 族首参为源（D10/待决问题 1） | 边界 | `shutil.copy('<受保护>','/tmp/x')`（从受保护文件复制出去）→ 含 `"<受保护>"`（既定 over-block 行为锚定：规格固定「路径取第一个字符串实参」，方向为安全侧）；`fs.copyFileSync('<受保护>', '/tmp/x')` → 同 | 表驱动（2 形态） |
| `write_targets` — 多路径保序（D10） | 正向 | `node -e "fs.writeFileSync('a',''); fs.unlinkSync('b')"` → 按命令出现序返回 `["a","b"]`（不折叠）；同路径两 API → 两条（不去重） | 表驱动（2 形态） |
| `write_targets` — 引号族（D10） | 正向 | 单引号 / 双引号 / 反引号（模板串）包裹路径各一形态 → 均返回该路径；引号内含空格 / 中文 / emoji 的路径原样返回 | 表驱动（5 形态） |
| `write_targets` — 共现不误拦（AC-7/R-4） | 异常 | `node -e "console.log('writeFileSync','<受保护>')"` → `[]`（API 名与路径实参未绑定）；`node -e "const s='open(p,\"w\")'"` → `[]`（引号内文本不是真实调用） | 表驱动（2 形态） |
| `write_targets` — 裸脚本与模块放行（AC-7/D10） | 异常 | `node scripts/setup.js <受保护>` / `python tools/gen.py` / `node -m pkg` / `python -m mod` / `node -c a.js` / `node --check a.js` → 全部 `[]`（脚本内容不可静态判别，作为已知漏面留痕；与同命令内的重定向 / 删除族不互斥——那部分归 `extract_file_ops`） | 表驱动（6 形态） |
| `write_targets` — 嵌套与 shell 包裹（AC-7） | 正向 | `bash -c 'node -e "require(\"fs\").rmSync(\"p\")"'` → 含 `"p"`；`pwsh -Command "node -e \"fs.writeFileSync('p','')\""` → 含 `"p"`；`bash -c 'python -c "open(\"p\",\"w\")"'` → 含 `"p"`（对整条文本扫描、不切脚本段） | 表驱动（3 形态） |
| `write_targets` — 变量中转漏面（D10/R-3） | 边界 | `node -e "const p='<受保护>'; fs.writeFileSync(p,'')"` → `[]`（无字符串实参，承认漏面）；`python -c "p='<受保护>'; open(p,'w')"` → `[]` | 表驱动（2 形态） |
| `write_targets` — 畸形与边界（D15） | 边界 | `""` / 全空白 / 只有引号 `"''"` / 未闭合引号 → `[]`；100 KB 命令（尾部 1 处写 API）→ 返回 1 条且耗时可接受；含中文路径与 emoji → 正确提取、不 panic | 参数化（6 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 | 零 mock：命令字符串为被测 API 显式入参，纯 `regex` 文本扫描零 IO / 零子进程（**不真跑** `node` / `python`） | 全部用例 |
| 文件系统 / 网络 | 不涉及：判别只看命令行文本（design D10「不切脚本段、不执行脚本」） | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/reason.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/reason_test.rs

<!-- design.md「公共函数 / API」子表三行（`reason::deny_reason` / `substitute` / `bulk_revert_reason`）落本文件；文案规格取 spec「拒绝指令文案」五要素与 AC-8 两句功能句（D11）。四条模板内**禁**目录名 / 文件名字面量（D2），故「模板零字面量」以传入假路径时产物不含目录名的断言承载。 -->

#### 待测功能

- `reason::deny_reason(kind: &ReasonKind, path: &str, tool: &str) -> String`（公共函数表，AC-2/AC-8/D3/D11）：三态选串——`BuiltinConfig` → 内置配置串；`User(Some(t))` → 模板（`%s` / `%t` 全量替换）；`User(None)` → 通用回落串
- `reason::substitute(template: &str, path: &str, tool: &str) -> String`（公共函数表，AC-8/D11）：`%s` → 路径、`%t` → 工具名，**全量**替换所有出现
- `reason::bulk_revert_reason(detail: &str, tool: &str) -> String`（公共函数表，AC-6/AC-8/D11）：批量还原专用串（含 detail 事实 + 逐路径 `git restore <path>` 引导 + 两句功能句）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `deny_reason` — 内置串五要素（AC-8） | 正向 | `deny_reason(&ReasonKind::BuiltinConfig, "<root>/openspec/config.json", "write")` → 串含：① 路径原文 ② 「工作区配置权威 / 不得改写」语义 ③ 「改哪个字段、改成什么、为什么」指引 ④ 「立即停止并通知用户」 ⑤ 「不要改用其他命令或工具尝试绕过」 | 单值 |
| `deny_reason` — 用户模板替换（AC-2/AC-8） | 正向 | `User(Some("受保护：%s（%t）不可写"))` + path=`<root>/docs/a.md` + tool=`bash` → 逐字等于 `"受保护：<root>/docs/a.md（bash）不可写"`；模板含 3 处 `%s` 与 2 处 `%t` → 全部替换（零残留占位符） | 表驱动（2 形态） |
| `deny_reason` — 无 reason 回落串（AC-2） | 正向 | `User(None)` + path=`<root>/plugins/dev-team/bin/knip.json`（本仓现行无 reason 规则形态）+ tool=`edit` → 串含路径与工具名 + 调整方案 + 「立即停止并通知用户」+ 「不要改用其他命令或工具尝试绕过」 | 单值 |
| `deny_reason` — 三态互不相同（AC-8/D11） | 边界 | 同一 path / tool 下三态产物两两不等（模板不共用）；三态均含「不要改用其他命令或工具尝试绕过」与「立即停止并通知用户」（两句为功能句，MUST NOT 省略） | 遍历枚举（3 形态） |
| `deny_reason` — 无占位符与空路径（D15） | 边界 | `User(Some("原文文案"))` → 逐字等于 `"原文文案"`；path 为空串 / 全空白 → 不 panic，占位符替换为空值（产物零 `%s` / `%t` 残留） | 表驱动（3 形态） |
| `deny_reason` — 产物不含目录名字面量（D2/D11） | 边界 | 以 path=`/tmp/ws/x.md`、tool=`write` 调内置态 → 产物不含 `openspec` 与 `config.json` 字面量（模板零字面量纪律的行为面锚；路径原文除外） | 单值 |
| `deny_reason` — 超长与特殊字符（D15） | 边界 | path 为 1000+ 字符 / 含 emoji / 含换行与 `\0` → 原样注入、不 panic、不截断；超长模板（1000+ 字符）替换后长度 ≈ 模板 + 路径 | 参数化（4 形态） |
| `deny_reason` — 确定性（D1） | 边界 | 同入参两次调用返回逐字节相等（纯函数无隐藏状态） | 单值 |
| `substitute` — 全量替换（AC-8/D11） | 正向 | `substitute("%s %s %s", "p", "t")` → `"p p p"`；`substitute("%t\|%t", "p", "bash")` → `"bash\|bash"`；`substitute("%s", "/a/b", "edit")` → `"/a/b"` | 表驱动（3 形态） |
| `substitute` — 无占位符与空串（D15） | 边界 | `substitute("no placeholder", "p", "t")` → 原样；`substitute("", "p", "t")` → 空串；`substitute("%s", "", "")` → 空串（替换为空值，不 panic） | 表驱动（3 形态） |
| `substitute` — 非占位符百分号（D15） | 边界 | `substitute("100% 完成", "p", "t")` → 逐字保留（不 panic、不吞字符）；`substitute("%S", "p", "t")` → 原样保留（占位符大小写敏感） | 表驱动（2 形态） |
| `bulk_revert_reason` — 引导与两句（AC-6/AC-8） | 正向 | `bulk_revert_reason("git stash 会批量改写工作区", "bash")` → 串含 detail 原文、含 `git restore <path>` 逐路径引导、含工具名 `bash`、含「不要改用其他命令或工具尝试绕过」与「立即停止并通知用户」 | 单值 |
| `bulk_revert_reason` — detail 边界（D15） | 边界 | detail 为空串 / 超长 / 含中文与 emoji → 不 panic、原样嵌入；同入参两次调用相等 | 参数化（4 形态） |
| 三条串 — 两两不等（D11） | 边界 | 内置串 / 回落串 / 批量还原串（同 path / tool / detail 底座）两两不等（三条模板独立，无粘连段落） | 遍历枚举（3 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无 | 零 mock：`ReasonKind` / 路径 / 工具名 / detail 均为被测 API 显式入参，纯字符串函数零依赖零 IO | 全部用例 |
| 配置文件 / 环境变量 | 不涉及：文案模板为编译期常量，本模块不读配置（`ReasonKind` 由装配面提供） | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/loop_test.rs

<!-- design.md「公共函数 / API」子表无本文件行（`deny` / `outcome_result` / `sandbox_rewrite` / `finish_*` 逐字零改，MUST NOT 新增导出）；被测面取「类型定义」子表 `LoopTurn` 行（增 `pub protect: ProtectRules`）与「修改文件」表本文件 ③（第四段闸门：沙箱之后、执行之前）+ ④（守卫链注释三段改四段）。用例复用既有装置（`FakeModel` + `loop_turn` 助手 + `notices_of` / `tool_results_of` / `sealed_messages`），不新建装置；`loop_turn` 助手随动补 `protect` 字段（默认 `ProtectRules::load(None)`——仅内置，既有 2339 行用例不写内置保护面，行为零变），新增用例以显式规则集注入。 -->

#### 待测功能

- `LoopTurn.protect: ProtectRules`（类型定义表，AC-1/AC-9/D1/D14）：装配点算好、loop 纯消费；每 run 一份；与 `defense` / `retry` / `liveness` 同列
- 守卫链第四段（修改文件表 loop.rs ③，AC-1/AC-8/AC-11/D13）：`sandbox_rewrite` 成功之后、执行臂路由之前，`turn.protect.evaluate(&name, &sandboxed, &turn.cwd)` 命中 → `deny(&sender, "permission_denied", &name, &reason, &id)` + `outcome_result(tool_call, reason)` + `continue`（run 不中断、零新 subtype、保护模块零事件流出）
- 守卫链顺序与射程（修改文件表 ④ + D4，AC-11）：成员校验 → policy → sandbox → **写入保护** → 执行；`create_explore_note` 走分派表表达（loop 无工具名特判）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 闸门 — `write` 命中内置（AC-1） | 正向 | 模型首轮发起 `write`，`path = <cwd>/openspec/config.json`（`cwd` = tempdir，内置规则命中）→ 事件流恰 1 条 `SystemNotice{subtype:"permission_denied"}`、`payload.tool == "write"`、`payload.reason` 为内置串；同 id is_error ToolResult 内容与 `payload.reason` **逐字节相同**；受保护文件零落盘（不存在）；`TurnDone.is_error == false` 且第二轮请求在场（run 不中断） | 单值 |
| 闸门 — `edit` 命中用户规则（AC-2） | 正向 | 规则集 = `load(Some(tempdir))`（config 含 `docs/**` + reason 模板）→ `edit` 指向 `<cwd>/docs/a.md` → notice 的 reason 为替换后模板（含路径与 `edit`）；ToolResult 同串；文件字节不变 | 单值 |
| 闸门 — bash 三路命中（AC-3/AC-7） | 正向 | 三轮分别发起 `bash`：`echo {} > openspec/config.json` / `rm openspec/config.json` / `node -e "require('fs').writeFileSync('openspec/config.json','{}')"` → 每轮各 1 条 `permission_denied` notice + 同 id is_error ToolResult；受保护文件字节/存在性不变 | 表驱动（3 路径） |
| 闸门 — bash Tree 命中与放行（AC-3/D9） | 边界 | `find . -delete`（`cwd` = 含受保护文件的 tempdir）→ 拒绝（notice 的 `%s` 报搜索根）；同一 `cwd` 下 `find src -delete`（仅 `src/` 子树）→ **零** `permission_denied`，执行照常（结果非 error 或执行体自身错误） | 表驱动（2 形态） |
| 闸门 — 未命中照常执行（AC-11） | 正向 | `write` 指向 `<cwd>/notes.txt` / `bash` 为 `echo foo > notes.txt` → 零 `permission_denied`、结果 `is_error == false`、文件真实生成且内容正确（保护闸不抢执行） | 表驱动（2 形态） |
| 闸门 — 只读四工具直通（AC-1/D4） | 正向 | `read` / `grep` / `glob` / `ls` 指向受保护文件所在目录 → 零 `permission_denied`（读面全开放）；notice 序列为空 | 表驱动（4 工具） |
| 闸门 — `create_explore_note` 直通（AC-11/D4） | 边界 | explore 注入轮（`toolset = Explore`）发起 `create_explore_note` 且规则集含覆盖探索笔记目录的规则 → 零 `permission_denied`（分派表 `_ => None`），专用工具走原执行面（结果形态与既有用例一致） | 单值 |
| 闸门 — 档位矩阵（AC-1） | 正向 | Bypass / AcceptEdits 两档各驱动 `write` 命中内置 → 均 1 条 `permission_denied`（保护串）；Default 档同调用 → 拒绝串为 policy 的「权限档位不允许该工具」（既有语义先行，保护闸未触；两档对照证明闸门与档位无关） | 表驱动（3 档） |
| 闸门 — 顺序锚：沙箱先于保护（AC-11/D3） | 异常 | `write` 指向 workspace 外逃逸路径（`../..` 或盘符外）且该路径形态同时命中规则 → notice subtype 为 `sandbox_denied`（非 `permission_denied`），reason 为沙箱文案（错误归属不抢报） | 单值 |
| 闸门 — 顺序锚：成员校验 / policy 先于保护（AC-11） | 异常 | explore 轮发起不在下发面的 `write` → `tool_unavailable`（非 `permission_denied`）；未知名 `rm_rf` → `tool_unavailable`；两者均不触 protect（零保护文案） | 表驱动（2 形态） |
| 闸门 — 拒绝后 history 收对（AC-8） | 正向 | 拒绝后第二轮请求的 `chat_history` 含该 tool_use 的 tool_result 且文本等于拒绝串（模型可见完整指令）；`sealed_messages` 两轮齐全 | 单值 |
| 闸门 — 同文件连撞不中断（R-2） | 边界 | 连续三轮对同一受保护文件发起 `write` / `bash` 重定向 / `bash cp` → 3 条 `permission_denied` notice、3 条同 id is_error ToolResult、`TurnDone.is_error == false`（deny 不中断也不硬中断；引擎不加「二次拒绝即硬中断」机制） | 表驱动（3 轮） |
| 闸门 — 未配置用户规则时普通写零拒绝（AC-11） | 边界 | `protect = ProtectRules::load(None)`（仅内置，`loop_test` 侧不可直构空规则集——字段私有于 `protect`）→ `write` 指向 `<cwd>/notes.txt` 与 `bash` 为 `echo foo > notes.txt` 各一轮 → 零 `permission_denied`、文件真实生成（内置面之外的普通写零拒绝的装置级对照） | 表驱动（2 形态） |
| 装置随动 — `loop_turn` 助手补 `protect`（类型定义表） | 边界 | 既有 `loop_turn` / `loop_turn_with_defense` / `loop_turn_with_fast_io` 助手与 8 处 `LoopTurn {}` 字面量补 `protect` 字段后编译通过；既有 2339 行用例全绿（默认仅内置规则且既有用例不触内置保护面） | 单值 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 模型 IO（进程边界：provider 网络 / 流） | 不 mock 真实网络：复用既有 `FakeModel`（`rig` 的 `test-utils` `MockScript` wire + 脚本化 transport）预录流事件，`drive_loop` / `drive_loop_turn` 直驱 `loop::run`——无网络、无真实 provider | 全部闸门用例 |
| 事件通道 | 不 mock：真实 `tokio::mpsc` 通道（容量与真实泵同口径）+ `notices_of` / `tool_results_of` 断言真实事件流 | 全部断言 |
| 文件系统 | 不 mock：`tempfile::TempDir` 真实 `cwd`，受保护文件以真实字节落盘（判定前后字节 / 存在性比对，证零写入） | 命中与放行用例 |
| 规则集 | 不 mock：`ProtectRules::load(None)`（仅内置）/ `load(Some(tempdir))`（内置 + 真实配置文件用户规则）真实装配；`loop_test` 位于 `protect` 之外的兄弟模块，故只经 `load` 构造（不直构字段） | 全部闸门用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner.rs -> packages/desktop/src-tauri/crates/infra/agent/src/sdk/runner_test.rs

<!-- design.md「公共函数 / API」子表一行（`SdkRunner::with_workspace_root`）与「类型定义」子表两行（`SdkRunner` / `PumpPayload` 增 `workspace_root`）落本文件；装配面取「修改文件」表本文件 ④（`session_pump` 起播时 `ProtectRules::load(payload.workspace_root.as_deref())`，每 run 一次、唯一 IO 点）。观测边界：`session_pump` / `PumpPayload` 为私有面，端到端「真模型工具调用被拒」不可构造（`runner_test` 的 provider 指向 discard 端点，见 `## 不可测试项` 第 7 项）；本节只断言 runner 侧可观测面（构造 / 链式 / 失败面 / 事件序零变），判定行为由 `protect/mod_test` 与 `loop_test` 承接。 -->

#### 待测功能

- `SdkRunner::with_workspace_root(self, root: PathBuf) -> Self`（公共函数表，AC-9/AC-10/D14）：runner 侧旁路缝（与 `with_max_tokens` 同型）；不改 `new()` 四参签名
- `SdkRunner.workspace_root: Option<PathBuf>`（类型定义表，AC-10/D14）：主仓 root（`None` = 未注入 → 仅内置规则）
- `PumpPayload.workspace_root: Option<PathBuf>`（类型定义表，AC-9/D14）：`open_session` 构造处传 `self.workspace_root.clone()`，泵起播装配的原料
- 泵起播装配（修改文件表 ④，AC-9/AC-10）：`session_pump` 内 `let protect = ProtectRules::load(payload.workspace_root.as_deref());` 每 run 一次，装入 `LoopTurn.protect`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `with_workspace_root` — 链式构造（D14） | 正向 | `SdkRunner::new(complete_config(), None, None, None).with_workspace_root(<tempdir>)` 构造成功；与 `with_max_tokens(Some(1024))` 两序续链（`with_max_tokens` → `with_workspace_root` 与反向）均编译可达且 `open_session(open_new())` 成功 | 表驱动（2 序） |
| `with_workspace_root` — 未注入零变化（AC-10/AC-11） | 正向 | 不调 `with_workspace_root` 的 runner `open_session(open_new())` → 首事件 `RunStarted`、`tools` 与 `TOOL_NAMES` 同清单同序（既有探索面用例同口径）；`open_continue` 无装载缝仍 `ConfigMissing`（失败面逐字不变） | 表驱动（2 形态） |
| `with_workspace_root` — root 边界（D15） | 边界 | `PathBuf::new()`（空）/ 不存在目录 / 含中文与 emoji 路径 / 1000+ 字符路径 → `open_session` 仍成功、首事件 `RunStarted` 照常（`config::load` 永不失败、不校验 root 有效性） | 表驱动（4 形态） |
| `with_workspace_root` — 与 `SessionCtx.workspace_root` 双根并存（AC-10） | 边界 | runner 注入主仓 root（`<tempdir A>`）且 `SessionCtx.workspace_root = <tempdir B>`（worktree 形态）→ `open_session` 成功、`RunStarted` 与 `TurnDone` 事件序与单根用例一致（两值不互相污染、不 panic；主仓口径的判定面见 `## 不可测试项` 第 3 项） | 单值 |
| `with_workspace_root` — 空配置失败面不漂移（AC-11） | 异常 | `EngineConfig::empty()` + 已注入 root → `open_session` 仍 `Err(ConfigMissing)` 且文案含 `api_key` / `base_url` / `model`（保护装配不抢配置校验的错误归属） | 单值 |
| `with_workspace_root` — 未注入等价性（AC-11） | 正向 | 同一配置下「未注入」与「注入 root」两 runner 的 `open_session` 首事件字段（`tools` / `model` 档位来源）逐字段相等（唯一差异在不可见的 `workspace_root` 载荷） | 单值 |
| 泵装配 — 每 run 一次（AC-9/D2） | 边界 | 同一 runner 连续两次 `open_session` + 各发一轮问题 → 两次 `RunStarted` 事件面一致（装配在起播时发生、每 run 一份；「恰一次 `config::load`」的机械断言由源码扫描承接，见 `## 不可测试项` 第 4 项） | 单值 |
| 既有用例零变（AC-11） | 边界 | 既有 1554 行用例（`new()` 51 处调用 / 失败面 / 转录装载 / explore 注入）在新增字段后全绿（`new()` 四参签名不动、`workspace_root: None` 缺省） | 单值 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 模型 IO / 网络 | 不 mock 出站网络：`complete_config()` 的 `base_url` 指向本机 discard 端口（`http://127.0.0.1:9/v1`，恒拒连）——用例只观测 `RunStarted` 前的装配面，不发起真实 provider 调用（沿既有 `runner_test` 装置口径） | 全部用例 |
| 转录装载缝 | 不 mock 内部模块：`loader(...)` 以内存闭包替身注入（`ResumeTranscript` 是 `AgentRunner` 层面的显式注入依赖，既有装置） | 续会话 / 失败面用例 |
| 文件系统 | 不 mock：`tempfile::TempDir` 真实主仓 root 与 worktree 目录（含真实配置文件），`config::load` 真实读取 | 全部注入类用例 |
| `Store`（DB） | 不 mock：既有 `temp_store` 真实 `Store::open_workspace`（redb 临时库），仅在 explore 注入臂需要 | 涉 explore 注入的回归用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/lib.rs -> packages/desktop/src-tauri/crates/infra/agent/src/lib_test.rs

<!-- design.md「公共函数 / API」子表一行（`EngineFacade::with_workspace_root`）与「类型定义」子表一行（`EngineFacade` 增 `workspace_root`）落本文件；「修改文件」表列明五个既有构造点（`new` / `with_resume_transcript` / `with_context_window` / `with_max_tokens` / `with_explore_note_store`）各补 `workspace_root: None`，`runner_for` sdk 臂条件透传。用例沿既有 `with_explore_note_store` / `with_context_window` 建缝用例同形（构造链 + 分发等价 + 缺省零变），不新建装置。 -->

#### 待测功能

- `EngineFacade::with_workspace_root(self, root: PathBuf) -> Self`（公共函数表，AC-10/D14）：主仓 root 注入缝（与 `with_explore_note_store` 并列）；`runner_for` 的 sdk 臂在 `Some` 时以 `with_workspace_root` 透传给 `SdkRunner`
- `EngineFacade.workspace_root: Option<PathBuf>`（类型定义表，AC-10/D14）：`None` = 未注入 → 仅内置规则；五个既有构造点各补 `None`（签名不动）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `with_workspace_root` — 链式构造（D14） | 正向 | `EngineFacade::new().with_workspace_root(<tempdir>)` → `runner_for(EngineKind::Sdk, complete_cfg())` 为 `Ok`；与既有四 builder 双向续链（`with_context_window` / `with_max_tokens` / `with_explore_note_store` / `with_resume_transcript` 各一序）均 `Ok` | 表驱动（5 序） |
| `with_workspace_root` — 分发等价（AC-11） | 正向 | 注入 root 后 `runner_for(EngineKind::Sdk, cfg)` 的 `open_session` 首事件 / 失败面与直构 `SdkRunner::new(...).with_workspace_root(root)` 逐字段一致（门面只做透传，零额外语义） | 单值 |
| `with_workspace_root` — 缺省零变（AC-11） | 正向 | 不调该 builder 的 `EngineFacade::new()` / `with_resume_transcript(...)` / `with_context_window(...)` / `with_max_tokens(...)` / `with_explore_note_store(...)` 五构造点 → `runner_for(EngineKind::Sdk, cfg)` 行为与变更前一致（既有用例全绿；`workspace_root == None` 语义不产出「保护关闭」） | 遍历枚举（5 构造点） |
| `with_workspace_root` — root 边界（D15） | 边界 | `PathBuf::new()` / 不存在目录 / 含 emoji / 1000+ 字符 → `runner_for` 仍 `Ok`（门面不校验 root，命令层职责） | 表驱动（4 形态） |
| `with_workspace_root` — Cli 臂零变（AC-11） | 异常 | 注入 root 后 `runner_for(EngineKind::Cli, cfg)` 仍为 `Err(CLI_TENANT_RETIRED)` 逐字文案；装载缝调用计数恒零（root 载荷不进退役臂） | 单值 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 模型 IO / 网络 | 不 mock：`complete_cfg()` 指向本机 discard 端口（既有装置）；用例只断言 `runner_for` 构造面与 `open_session` 早期事件 | 全部用例 |
| 转录装载缝 | 不 mock 内部模块：`counting_loader` 闭包替身（`ResumeTranscript` 为显式注入依赖，既有装置）——用于断言退役臂零调用 | Cli 臂零变用例 |
| 文件系统 | 不 mock：`tempfile::TempDir` 真实 root（可含真实配置文件） | 全部注入类用例 |

---

### packages/desktop/src-tauri/crates/infra/agent/src/compose.rs -> packages/desktop/src-tauri/crates/infra/agent/src/compose_test.rs

<!-- design.md「公共函数 / API」子表一行（`compose_turn` 新增尾参 `workspace_root: &Path`）落本文件；「修改文件」表列明只做门面透传、零解析残留。观测边界：`with_workspace_root` 后的载荷在门面内不可读回，故 compose 层只断言「尾参在场不改既有装配产物与失败面」+「root 取值不校验」；主仓口径的行为面由 `protect/mod_test`（双根判定）与命令层源码审查（`## 不可测试项` 第 3 项）承接。既有 33 处本文件测试调用 + `worker_test` 7 处调用补尾参属机械随动（`## 不可测试项` 第 5 项）。 -->

#### 待测功能

- `compose_turn(stores, registry, streams, store, agent, workspace_root: &Path) -> Result<ComposedTurn, String>`（公共函数表，AC-10/D14）：新增尾参（主仓 root），只做 `EngineFacade::with_workspace_root(workspace_root.to_path_buf())` 透传；不解析不校验

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `compose_turn` — 尾参在场装配照常（AC-11） | 正向 | 既有 `DualEnv` 底座 + 默认 sdk 实例 + 尾参 `<env root>` → `Ok(composed)`，`begin(...)` 成功、轮行 `running`、引擎选型与快照与既有用例一致（尾参不改装配产物） | 单值 |
| `compose_turn` — root 不校验（D15） | 边界 | 尾参传不存在目录 / `Path::new("")` / 含 emoji 路径 → 仍 `Ok`（只透传；不因 root 形态失败） | 表驱动（3 形态） |
| `compose_turn` — 与 store 的 root 不同源（AC-10） | 边界 | 尾参 `<tempdir A>`（主仓 root）而 `stores.for_root(<tempdir B>)` 的 store 传入（worktree 形态）→ `Ok`、`begin` 成功、会话行落 `<tempdir B>` 库（store 半边不受尾参影响，双 root 不变量成立） | 单值 |
| `compose_turn` — 既有失败面零变（AC-11） | 异常 | 空库缺省解析（无 agent 记录）/ 悬空 provider 引用 / 显式不存在 agent id 三形态 → 错误文案与既有用例逐字一致（尾参不引入新失败面） | 表驱动（3 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| `Store` / `WorkspaceStores`（DB） | 不 mock：既有 `DualEnv` 真实 redb 临时库（`stores.for_root` 真实寻址）+ 真实 fixture 落库 | 全部用例 |
| 引擎 runner | 不 mock 内部模块：sdk 臂真实 `SdkRunner`（`base_url` 指向 discard 端点，仅观测装配与 `"running"` 轮行）；`SpyRunner` 替身仅用于既有「注入实例」用例的会话入参捕获 | 「尾参在场装配照常」与「双 root」用例 |
| 文件系统 | 不 mock：`tempfile` 真实 workspace 目录与库文件 | 全部用例 |

---

### packages/desktop/src-tauri/src/commands/exec/mod.rs -> packages/desktop/src-tauri/src/commands/exec/mod_test.rs

<!-- design.md 未声明该文件的公共 API 变更（「公共函数 / API」子表无本文件行）：改动仅为 `:132` 的 `compose_turn(…)` 调用补尾参 `Path::new(&root)`（**主仓 root**，非 `exec_root`），属命令层机械透传，design「修改文件」表本行备注「双 root 不变量不变（`for_root` 仍受 `root` 支配）」。既有 `mod_test.rs` 的 56 个用例（`agent_start_with` 成功链路 / 双 root 拆分 / 失败面）在补参后全绿即承载回归（AC-11）；「尾参取值恒为主仓 root」的断言面为源码审查，见 `## 不可测试项` 第 3 项。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（「公共函数 / API」子表与「类型定义」子表均无本文件行），故本节不设函数级条目。 -->

无新增公共 API 条目：本文件仅 `compose_turn(…)` 调用点补尾参（机械透传），被测面收敛为命令面既有 56 条用例在补参后的行为零变（AC-11），见下 `#### 用例`。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `agent_start` 成功链路（补参后，AC-11） | 正向 | 既有 `agent_start成功链路提前resolve返回running轮行且会话行轮行已落库`、`agent_start参数转换_session引用与来源与agent透传`、`agent_start_kernelscoped_stopregistry托管的命中链经channel流出终态部件` 三条全绿：`running` 轮行、会话行已落库、channel 首事件到场，且 `SessionCtx.workspace_root` 仍取 `exec_root`（尾参不改装配产物、不改双 root 归属） | 表驱动（3 用例） |
| 既有失败面与守卫零变（AC-11） | 异常 | `agent_start_blank_root守卫err且零落库`、`agent_start无默认agent时err引导管理页`、`agent_start_sdk空配置时config_missing映射err直抵前端`、`root不可寻址_三查询命令err透传不panic`、`agent_stop对未运行会话与不存在session_id幂等ok` 五条全绿且错误文案逐字不变（尾参不引入新失败分支、不抢错误归属） | 表驱动（5 用例） |
| 查询面与 watch 面零变（AC-11） | 边界 | `agent_sessions携root直查dto且source过滤透传`、`agent_sessions分页offset_limit透传与total为过滤集全量`、`agent_session_transcript落库转录后按seq序全史重放`、`session_detail直查返回三件套与清单面对应条目全等`、`session_detail跨root隔离_会话落a库经b库单查err`、`watch前向事件与末拍record后返回ok`、`watch_since_seq零时全量实时事件直投`、`watch结束即订阅释放_同会话再invoke立即ok零事件` 等 12 条全绿（无 `compose_turn` 调用点，仅证尾参补齐未波及兄弟命令） | 参数化（12 用例） |
| 56 用例零改动全绿即回归（AC-11） | 边界 | 本文件 `mod_test.rs` 的 56 条 `#[tokio::test]` / `#[test]` 在 `compose_turn` 补尾参后零改动、零新增即通过（命令层机械透传的可观测回归面；「尾参恒为主仓 `root`、MUST NOT 传 `exec_root`」的行为断言见 `## 不可测试项` 第 3 项） | 遍历枚举（56 用例） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 模型 IO / 网络（进程边界） | 不 mock 出站网络：既有装置把 provider `base_url` 置为不可解析地址（`http://bad host/v1`）或空串，executor 轮以 `ConfigMissing` 显式收敛，用例只观测命令层受理 / 落库 / 事件面，不发真实 provider 请求 | 全部用例 |
| `Store` / `WorkspaceStores`（DB） | 不 mock：沿用既有 `Env` 装置的真实 redb 临时库（`stores.for_root` 真实寻址）+ `seed_provider` / `seed_sdk_default` / `seed_sdk_session` 真实落库 fixture | 全部用例 |
| Tauri 宿主面（App / Channel） | 不 mock 内部模块：`tauri::test::MockRuntime` 与真实 `Channel`（`discarding_channel` / `capturing_channel`）属进程边界宿主替身，沿既有装置口径；`agent_start_with` 等被测命令真实调用 | 事件面与失败面用例 |
| 文件系统 | 不 mock：`Env` 的真实 `tempfile` workspace 目录与库文件（判定前后落库 / 零落库断言均以真实字节比对） | 全部用例 |

---

### packages/desktop/src-tauri/src/commands/change_flow/mod.rs -> packages/desktop/src-tauri/src/commands/change_flow/mod_test.rs

<!-- design.md 未声明该文件的公共 API 变更（「公共函数 / API」子表无本文件行）：改动仅为 `:143` 的 `compose_turn(…)` 调用补尾参（**主仓 `root`**，MUST NOT 传 `exec_root` / worktree 路径），属命令层机械透传。既有 `mod_test.rs` 的 26 个用例（`change_flow_start_with` / 步进与 sink / 失败面）在补参后全绿即承载回归（AC-11）；主仓口径的静态断言面见 `## 不可测试项` 第 3 项。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（「公共函数 / API」子表与「类型定义」子表均无本文件行），故本节不设函数级条目。 -->

无新增公共 API 条目：本文件仅 `compose_turn(…)` 调用点补尾参（机械透传），被测面收敛为命令面既有 26 条用例在补参后的行为零变（AC-11），见下 `#### 用例`。

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `change_flow` 受理链路（补参后，AC-11） | 正向 | 既有 `start提前resolve返回running摘要且channel首事件到达后台驱动不阻塞`、`auto_next_phase_true透传受理零confirmwait照常后台收敛`、`start_test_only记录放行且提前resolve返回running摘要` 三条全绿：`running` 摘要、channel 首事件到场、受理不阻塞（`compose_turn` 尾参不改受理产物） | 表驱动（3 用例） |
| 双 root 拆分与执行根不变（AC-10/AC-11） | 正向 | 既有 `exec_root解析成功_相位落workspace库且无第二库文件`、`并行冲突键id_同名异id各自受理且异root同id互不误拒`、`worktree目录缺失拒绝_显式err且零run登记` 三条全绿：执行根仍为 worktree（相位落 workspace 库、零第二库文件）、并行冲突键不变，尾参取主仓 `root` 不改变双 root 结构 | 表驱动（3 用例） |
| 既有失败面与守卫零变（AC-11） | 异常 | `start前置校验无建档与相位表缺失各自err且成因互不重合`、`start同change并行run冲突err`、`auto_next_phase_true不绕过守卫与前置校验`、`stop运行中置位且幂等忽略不报错`、`answer与confirm无等待方时err透传`、`confirm校验blank与无运行run双面err`、`参数转换守卫blank_root各命令模板保留`、`参数转换守卫blank_id五缝各就位` 八条全绿且错误文案逐字不变（尾参不引入新失败分支） | 表驱动（8 用例） |
| 兄弟命令面零变（AC-11） | 边界 | `watch补订运行中接收后续信封_无run时ok非错误`、`sink事件桥接_session_event记会话锚且首拍挂载在途节点`、`channel面五kind通知各自一拍且零载荷键`、`confirm透传sync_specs四组合至注册表`、`start归档互斥面摘除_无归档注册表照常受理`、`start走完路由相位停在archive前且confirm拒绝收敛stopped`、`组合根装配_staticcheck步默认recorded落单一log并回填引用`、`auto_next_phase_false默认档受理面回归` 等 12 条全绿（无 `compose_turn` 调用点，仅证尾参补齐未波及兄弟命令） | 参数化（12 用例） |
| 26 用例零改动全绿即回归（AC-11） | 边界 | 本文件 `mod_test.rs` 的 26 条 `#[tokio::test]` / `#[test]` 在 `compose_turn` 补尾参后零改动、零新增即通过（命令层机械透传的可观测回归面；「尾参恒为主仓 `root`、MUST NOT 传 `exec_root` / worktree」的行为断言见 `## 不可测试项` 第 3 项） | 遍历枚举（26 用例） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 模型 IO / 网络（进程边界） | 不 mock 出站网络：既有 `seed_sdk_default_instance` 的 provider 三件套留空 → executor 轮在 `begin()` open 段以 `ConfigMissing` 收敛，不 spawn 进程、不发网络请求；用例只观测命令层受理 / 落库 / 通知面 | 全部用例 |
| `Store` / `WorkspaceStores`（DB） | 不 mock：沿用既有 `app_with` 的真实 redb 临时库 + `seed_change_full` / `seed_change_with_worktree` / `seed_initialize_passed` 真实落库 fixture；主仓 root 与执行根（worktree）均为真实 `tempfile` 目录 | 全部用例，尤其双 root 用例 |
| Tauri 宿主面（App / Channel / 注册表） | 不 mock 内部模块：`tauri::test::MockRuntime`、真实 `Channel`（`discarding_channel` / `capturing_channel`）、真实 `SessionStreamRegistry`（sink 桥接面）沿既有装置口径 | 事件面 / watch / sink 用例 |
| 子进程（`PATH` 隔离与静态步 shim） | 不 mock 内部模块：`write_static_shim` 在临时目录真实生成可执行 shim，经 `prefix_path` 隔离 `PATH` 并由 `restore_path` 收尾——子进程属进程边界，按真实调用参与 | `组合根装配_staticcheck步…` 用例 |

---

## 不可测试项

| 项 | 原因 | 承接方式 |
|---|---|---|
| 1. `packages/desktop/package.json` 版本 `0.5.28` → `0.5.29` | 数据面（JSON 单字段），无运行时测试路径 | 版本 diff 复核（AC-13）；用户可见变更口径审查（受保护文件拒写 + 拒绝处置指令） |
| 2. `packages/desktop/src-tauri/crates/infra/agent/Cargo.toml`（新增 `checks = { workspace = true }`） | 构建配置，无单元测试断言面 | `Cargo.toml` 逐项审查 + `cargo tree -p agent-runtime -i checks`（单边）+ `cargo tree -p checks \| grep -E "infra\|agent-runtime"`（零命中，无环）+ `cargo check -p agent-runtime`（唯一用途 `match_glob` 可用） |
| 3. 命令层调用点的 `workspace_root` 取值（`src/commands/{exec,change_flow}/mod.rs` 尾参恒主仓 `root`，MUST NOT 传 `exec_root` / worktree） | 载荷在 `EngineFacade` / `SdkRunner` / `PumpPayload` 内不可读回，且端到端需真模型发起受保护写入（不可构造，见第 7 项） | 源码审查 + `grep -rn "compose_turn(" src/` 命中两处且尾参为 `&root`（不出现在 `exec_root` / `worktree` 拼接里）；主仓口径的行为面由 `protect/mod_test` 的「主仓 root 装配 + worktree 路径命中」双根用例承接（AC-10） |
| 4. AC-9 的「每 run 恰一次 `config::load`」「loop 内零 IO」「protect 源码零 `openspec` / `config.json` 字面量」（D2） | 源码文本面与调用计数（不可在运行期断言；`foundation` 的双禁令扫描当前不覆盖 `crates/infra/agent/**`，design 事实校准 3） | 源码审查 + 机械 grep：`grep -nE "fs::\|config::load\|canonicalize" crates/infra/agent/src/sdk/loop.rs`（零命中）、`grep -nE "config::load" crates/infra/agent/src/sdk/protect/`（仅 `rules.rs` 一处）、`grep -rnE "openspec\|config\.json" crates/infra/agent/src/sdk/protect/ --include=*.rs \| grep -v _test.rs`（零命中，含注释与 doc） |
| 5. 机械随动：`loop_test.rs` 8 处 `LoopTurn {}` 字面量补 `protect`；`compose_test.rs` 33 处 + `worker_test.rs` 7 处 `compose_turn` 调用补尾参 | 编译期一致性，无独立运行时断言 | `cargo test --no-run` / `cargo clippy --all-targets`（编译期即证）+ 两文件既有用例全绿（语义零变）；tasks 已登记为既定中间态，不在实现相位回填 |
| 6. Default 档下「保护闸在任意档位生效」的**直接**观测 | Default 档下 `write` / `edit` / `bash` 由 policy 先行拒绝（`permission_denied` + 「权限档位不允许该工具」），保护闸不可达；改变该顺序即违反 AC-11 | 以 Bypass / AcceptEdits 两档断言闸门生效（`loop_test` 档位矩阵）+ Default 档断言拒绝串为 policy 文案（顺序锚用例）承接；`evaluate` 本身无档位入参（`protect/mod_test` 断言入参面无档位量） |
| 7. 泵层端到端「真模型工具调用被内置规则拒」 | `runner_test` 的 provider 指向 discard 端点（无真实模型脚本注入缝），无法让泵产出 tool_call；`session_pump` / `PumpPayload` 为私有面 | 拆解承接：装配面 = `runner_test`（注入 / 未注入不 panic、事件序零变）+ 判定面 = `protect/mod_test`（`ProtectRules::load` 到 `evaluate` 全链）+ 拒绝链 = `loop_test`（第四段闸门四臂）；链路各段均有用例，仅「同一进程内三段串起来」无断言 |
| 8. Windows 盘符大小写与绝对 glob（design 待决问题 2） | `checks::globmatch` 的 `case_sensitive: true`，沙箱 canonicalize 产出的盘符大小写由系统决定；归一化属 `globmatch` 侧改动（只读红线，本变更 MUST NOT 自造方言） | 用例只锚定「与系统产出一致的大小写」（以 `tempdir` 真实盘符形态构造路径），跨盘符大小写差异登记为方言既有行为；不在本变更断言 |
| 9. AC-12 的覆盖率阈值（lines ≥ 90 / branches ≥ 85 / functions ≥ 90）、`cargo test --workspace` 全绿、`bindings:check` 零 diff | 门禁统计面（由桌面端静态脚本 / test-execution 门禁执行，agent 内不重复执行） | `pnpm -C packages/desktop run check` / `run test` + test-execution 门禁；`bindings:check` 零 diff（零新 IPC 命令、零新 DTO、payload 词汇零扩展——D13/D16）；新模块（`protect/{mod,rules,shell,inline,reason}`）用例先行全覆盖后再计入 |
| 10. AC-13 的 `plugins/**` 零改动零 bump、`openspec/config.json` 零 schema 扩展 | 数据 / 目录面 | `git diff --stat plugins/`（零条目）+ `git diff openspec/config.json`（零条目）+ 版本号 grep 复核（`packages/desktop/package.json` 单行变更） |
| 11. 已知漏面：`xargs` / `sh -c '<cmd>'` 递归拆解 / 生成脚本再执行 / `node -m` / 变量中转路径 / base64 编码脚本 / `printf` 喂 stdin | 静态不可判别（design D17 / R-3 定为知情边界，MUST NOT 当作安全缺口） | 用例以「放行」形态**显式留痕**（`inline_test` 裸脚本 / 变量中转；`shell_test` 无 `-exec` / `xargs` 形态），并由 spec 知情边界条款承接；真兜底（真沙箱 / 交互审批）留给后续 change |
| 12. `git clean` 参数自带通配（`git clean -fd 'openspec/*'`，design 待决问题 4） | `glob_dir_hit` 把参数当**字面目录**处理，通配段被静态前缀截断 → 漏拦（fail-open 方向，非误拦） | 用例只锚定字面目录形态（`shell_test` 三形态）；通配形态登记为待决问题，覆盖需引入第二种展开语义（风险高于收益） |
| 13. `find … -exec rm {} +`（design 待决问题 5） | `{}` 占位不可解析；`-exec` 后的 `rm` 在命令名定位面被当参数 | 用例只锚定 `-delete` 族（`shell_test`）；`-exec` 形态登记为知情边界，与 `xargs` 同类 |
| 14. `copyFile*` / `shutil.copy*` / `os.rename*` 首参为源的 over-block（design 待决问题 1） | 规格固定「路径取第一个字符串实参」，故「从受保护文件复制出去」会被拒 | 用例以**既定行为**断言承接（`inline_test` copy 族首参用例，方向为安全侧）；若产品改为「取第二实参」，该行用例随之改写 |
| 15. 100 KB 命令的绝对耗时上界（D12「不设命令长度上限」） | 性能阈值属门禁 / 环境相关，无法在单元测试中给出稳定断言 | 用例只断言「不 panic、不挂起、返回正确结果」（`shell_test` / `inline_test` / `protect/mod_test` 各一条冒烟）；细粒度基准由性能观测承接 |
| 16. 非 Windows 平台臂（`non-windows` 下的 `\\?\` 形态不可产生、盘符路径不成立） | 当前门禁仅 Windows | 归一用例以字符串形态直接驱动（`normalize` / `evaluate` 的 `path` 为字面量，不依赖平台产物）；平台相关分支由 `#[cfg(windows)]` 隔离，非 Windows 臂不另设用例 |
| 17. `packages/desktop/src-tauri/crates/infra/agent/src/sdk/mod.rs`（唯一改动：`pub(crate) mod protect;` 一行） | 仅私有模块挂载，design「公共函数 / API」子表无该文件行；该文件无同名 `*_test.rs`（测试模块声明在 `protect/mod.rs` 内，design 事实校准 1） | 挂载正确性即编译期（`mod` 缺失则 `sdk/protect/**` 与 `loop.rs` / `runner.rs` 引用无法解析）+ `grep -n "protect" crates/infra/agent/src/sdk/mod.rs` 恰一行自查（tasks 阶段一自查项）；行为面由 `protect/*` 五个映射对与 `loop_test` / `runner_test` 覆盖 |

