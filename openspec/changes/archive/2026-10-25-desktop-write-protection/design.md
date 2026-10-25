# 设计: desktop-write-protection

> **变更**: desktop-write-protection
> **日期**: 2026-10-23
> **基于**: `proposal.md` 与 `specs/{desktop-write-protection,desktop-agent-execution,desktop-crate-layout}/spec.md`

---

## 提案与规格同步状态

proposal 的八面与三份 delta spec 已定稿，本设计不重复其论证，只做四件事：把「拿到一条工具调用怎么判」落成可实现的算法（掩码 / 提取 / 归因的逐族口径）、把规格未给的成形算法缺口（子树相交判定、`>&` 前瞻的 Rust 替代、字符索引纪律）以决策留痕收口、给出 root 通道的逐层签名、按 AC-1..AC-13 逐条对齐。

起点核实（工作树实况；路径前缀 `packages/desktop/src-tauri/`，锚点为改动前行号）：

- **守卫链现状**：`crates/infra/agent/src/sdk/loop.rs`——`LoopTurn` 定义 `:75-99`；成员校验 `:375`、policy 档位门 `:381`、`sandbox_rewrite` `:387`、执行分支 `:399-412`；`deny(sender, subtype, name, reason, id)` `:447`；`sandbox_rewrite` `:561-578`（`glob` 走 `check_pattern`、其余走 `check`，**`bash` 因 `tools::input_path` 取 `path` 字段而原样返回**，故 bash 的 `command` 不被沙箱改写）。
- **沙箱产物形态**：`sdk/sandbox.rs:66` `check(root, raw) -> Result<PathBuf, String>` 已剥 `\\?\`（私有 `strip_verbatim` `:3`）并 canonicalize，故保护闸拿到的 `path` 是 canonical 绝对路径，判定无需二次 IO。
- **泵装配现状**：`sdk/runner.rs`——`SdkRunner` 字段 `:47-58`（`config` / `resume` / `context_window` / `max_tokens` / `explore_store`）；`with_max_tokens` 旁路 builder `:80-82`；`open_session` 泵 spawn `:161-176`；`PumpPayload` `:257-271`；`session_pump` `:274`；`LoopTurn { … }` 初始化 `:316-329`。
- **门面现状**：`crates/infra/agent/src/lib.rs`——`EngineFacade` 字段 `:40-51`；`with_explore_note_store` `:103-110`（同构先例）；`runner_for` sdk 臂 `:117-134`（`SdkRunner::new(...).with_max_tokens(...)`）。
- **组合根现状**：`crates/infra/agent/src/compose.rs:87-93` `compose_turn(stores, registry, streams, store, agent)`（**五参**）；门面链 `:122-128`；测试文件挂载 `:280-282`（`#[path = "compose_test.rs"]`）。
- **命令层调用点**：`src/commands/exec/mod.rs:132`（在手的 `root` 为主仓 root，`exec_root` 已单独解析进 `SessionCtx`）；`src/commands/change_flow/mod.rs:143`（在手的 `root` 为主仓，`exec_root` 取自 `record.worktree`）。
- **配置面现状**：`crates/core/config/src/lib.rs`——`WriteProtection` `:138`、`WriteProtectionFile` `:146`、`assemble_file_rule` `:624`（空串 / 非字符串 glob 整条剔除、`reason` 非字符串落 `None`）、`write_protection` 组装 `:744-781`、`load(root) -> ConfigReport` `:814`（永不失败，四类诊断）。
- **glob 方言现状**：`crates/core/checks/src/globmatch.rs:105` `match_glob(path, pattern) -> bool`；方言选项 `:117-124`（`require_literal_separator: true`、`require_literal_leading_dot: false`、大小写敏感）；无通配模式走「相等或 `pattern/` 起手」。
- **派生源现状**：`crates/core/foundation/src/layout/mod.rs`——`domain_dir_name()` `:51`、`resolve(root) -> Layout` `:56`（`changes_root` = 域根 + changes）、`config_path(root)` `:66`；`CONFIG_FILE_NAME` `:19` 为**私有**常量，故内置 glob 只能经 `config_path` 派生（不得在 protect 内自写文件名）。
- **依赖面现状**：`crates/infra/agent/Cargo.toml` `[dependencies]` 已有 `config` / `foundation` / `regex` / `glob` / `serde_json`，**无 `checks`**；workspace 键 `checks = { path = "crates/core/checks" }`（`src-tauri/Cargo.toml:70`），包名 `checks`；`crates/core/checks` 依赖 `config` / `foundation` / `glob` / `serde` / `serde_json` / `time`（**无任何 infra 依赖 → 无环**）。
- **版本面**：`packages/desktop/package.json:3` = `0.5.28`。
- **插件端精度来源**：`plugins/dev-team/bin/src/lib/shell-file-ops.ts`（403 行：`tokenize` / `isFlag` / `quoteSpans` / `heredocSpans` / `maskSpans` / 双变体 / `scanCommand` / `extractGitOps` / 一族写入正则）+ `plugins/dev-team/bin/src/commands/protect-files.ts`（`loadPatterns` / `isProtected` / `buildDenyReason` / `checkBulkRevert` / `^(python|node)\s` 旁路）。

**四处事实校准（不影响 proposal 的八面，但决定实现形态）**：

1. **测试模块声明位置**：proposal 写「`sdk/mod.rs` 挂 `protect` 模块与四个测试模块」。Rust 模块路径下 `sdk/mod.rs` 内的 `mod rules_test;` 会解析到 `sdk/rules_test.rs` 而非 `sdk/protect/rules_test.rs`。故 `sdk/mod.rs` 只加 `pub(crate) mod protect;` 一行，四个测试模块在 `protect/mod.rs` 内声明（沿 `foundation/src/layout/mod.rs` 声明 `mod_test` 的既有先例）。
2. **`compose_turn` 调用点实量 42 处**（proposal 估「约 40」）：`compose_test.rs` 33 + `worker_test.rs` 7 = 40 处测试调用，加 2 处生产调用；`lib_test.rs` 与 `src/commands/{exec,change_flow}/mod_test.rs` **零调用点**（proposal 测试表列了它们，实量为空）。
3. **命名隔离扫描的实际覆盖**：`crates/core/foundation/src/layout/mod_test.rs:161` 的 `全包产品源码双禁令扫描_openspec与config_json字面量_layout_为唯一例外` 以 `crates/<一级目录>/src` 为扫描根（即 `crates/core/src` 这类不存在的目录），故 `crates/infra/agent/**` 当前**不在**被扫清单内（实际覆盖 `src-tauri/src` + `packages/desktop/src`，`*_test.rs` 排除）。结论：protect 源码的「零字面量」由规格条款 + 设计自查 grep 承载，**MUST NOT** 借机修改 foundation 测试（零改动红线）。
4. **两处「子树相交」判定规格未给算法**：`find … -delete` 的搜索根（AC-3 的 `.` 场景）与 `git clean` 的目录参数（AC-6 的 `openspec/` 场景）都不能用「路径直接 match glob」表达（`.` 永远不比 `**/…/config.json` 命中，而「任何目录都可能含 `**/` 规则的目标」会退化成全禁止）。本设计补一个显式的子树相交判定（D9）。

**只读红线零触点**：`crates/core/config/**`、`crates/core/checks/**`（含 `globmatch.rs` 方言与 `match_glob` 签名）、`crates/core/foundation/**`、`crates/infra/{exec,vcs,checks,store}/**`、`crates/core/{agent,workflow,orchestration}/**`、`sdk/{bash,tools,policy,sandbox,explore_note,context,compact,resume,model_spec,normalize,preamble,log,config}.rs`、`packages/desktop/src/**`、`src/types/generated/bindings.ts`、`openspec/config.json`、`plugins/**`。

---

## 关键设计决策

| # | 议题 | 定稿 | 理由 |
|---|------|------|------|
| D1 | 装配与判定的分工 | `protect/rules.rs::load` 是唯一 IO 点（`config::load` + 内置合并 + glob 归一）；`ProtectRules::load` 只在泵起播时被调用一次，`evaluate` 零 IO 零副作用（无 `fs` / 无 `canonicalize` / 无事件通道） | 与 `defense` / `retry` / `liveness` 的「装配点算好、loop 纯消费」同构；`evaluate` 可密集单测而不造 fs（AC-9 / AC-1 的「run 内改 config 不改本轮行为」） |
| D2 | 内置规则与结构 glob 的派生源 | 内置 glob = `**/` + `foundation::layout::config_path(Path::new(""))` 的 root 相对形态；批量还原的两条结构 glob 一律由 `foundation::layout` 派生（`**/` + `domain_dir_name()` + `/**`、`**/` + `resolve(Path::new("")).changes_root` + `/**`）。protect 全部源码（含注释与 doc）**零** `openspec` / `config.json` 字面量 | 规格「MUST 由 `config_path` 派生 + 零字面量」；`CONFIG_FILE_NAME` 为 `foundation` 私有，故派生函数是唯一出路（设计不自造第二触点） |
| D3 | 规则序与 reason 三态 | `Rule { glob: String, reason: ReasonKind }`；`ReasonKind = BuiltinConfig \| User(Option<String>)`；内置恒在首位，判定**首个命中即返回**，MUST NOT 拼接多条 reason | 规格「内置优先于用户规则」「同一次判定 MUST NOT 拼接」；三态载体让「内置串 / 用户串 / 回落串」在一个 match 里收口，不靠 `glob` 字面量比对认内置 |
| D4 | 判定射程的表达 | 射程名单**只存在于** `ProtectRules::evaluate` 的工具名分派（`write` / `edit` / `bash` 三臂，其余 `_ => None`）；loop 侧无条件调用一次、不做工具名特判 | 规格「`create_explore_note` MUST NOT 经保护闸」「只读四工具不在射程」：以分派表表达射程，避免 loop 里长出第二份射程名单（两份名单必然漂移） |
| D5 | 路径归一与解析口径 | 单点文本归一 `rules::normalize`（`mod.rs` / `shell.rs` / `concrete_prefix` 共用，不各写一份）：剥前缀 `\\?\` → `\` → `/` → 去空段与 `.` 段、保留 `..` 段、去尾分隔符；`write` / `edit` 的 `path` 已是 canonical（沙箱产物）只做归一；`bash` 提取出的相对路径 `<cwd>/<path>` 后归一，绝对路径原样归一；解析失败（空串 / 含 `$`）跳过该操作 | 规格「MUST NOT 再次 canonicalize（判定零 IO）」；保留 `..` 而非文本消解是刻意的——`**/` 能吸收 `..` 段，消解反而要在无 IO 前提下猜语义；`.` 段必须消解，否则 `find . -delete` 的搜索根与规则前缀无法比较（D9） |
| D6 | 字符索引纪律 | 掩码 / 切片 / 正则匹配全程在 **`Vec<char>` 空间**：两个掩码变体由同一 `Vec<char>` 派生、逐字符替换、长度相等；参数切片按 char 区间取变体 A 的字符再 `collect()` 成 `String` | JS 版索引单位是 UTF-16 code unit、Rust 以字节为界；两面直接照译会因非 ASCII 命令文本（中文路径、中文注释）错位。取 char 单位后「等长」与「索引 1:1」同时成立，且 `String` 重建不破 UTF-8 边界 |
| D7 | `>&` 的 fd 复制跳过 + `[^-=]` 排除集 | `>` 前缀排除集如实照抄 `[^-=]`（**MUST NOT** 补成 `[^-=&\|]`）；`>&` 的「裸整数目标跳过」由 JS 的 `(?!\d+…)` 前瞻改写成**捕获后过滤**：目标 token 全为 ASCII 数字即丢弃该 op | `regex` crate 无 lookaround（前瞻不可用）；捕获后过滤与原前瞻在「`2>&1` / `1>&2` / `>&2`」上等价，且不改任何匹配边界。排除集一字不改是规格红线（补全会让 `echo {} &> <protected>` 静默变放行） |
| D8 | 档 B 五族的取位口径 | `cp` / `Copy-Item`：末个非 flag 非 `$` token → write；`dd`：`of=` 前缀 token 的等号右侧 → write（显式跳过 `if=`）；`sed` / `perl`：命令行含 `-i`（含 `-i.bak` / `-i''` 紧贴形态）时取**末个**非 flag 非 `$` token → write，不含 `-i` 恒零 op；`truncate`：`-s` / `--size=` 之后的第一个非 flag token → write；`find`：命令行含 `-delete` 时取首个非 flag token 为搜索根 → `OpTarget::Tree` | 五族判据逐条来自规格表；`sed` 取「末参」而非「脚本后的第一个」是为了不把 `s/a/b/` 误当路径；`perl -i -pe … file` 因「末参」口径自动成立 |
| D9 | 子树相交判定 `glob_dir_hit` | 新增 `rules::glob_dir_hit(dir: &str, glob: &str, anchor: &Path) -> bool`：把 glob 折算成**concrete 前缀**（无 `**/` 起手 → glob 自身的首个通配段之前的静态段；有 `**/` 起手 → `normalize(anchor)` + `/` + 尾部的静态段），再判三面之一：`前缀 == dir`、`前缀` 以 `dir/` 起手、`dir` 以 `前缀/` 起手。`find` 的 `Tree` 目标与 `git clean` 的目录参数走此判定，拒绝串里的 `%s` 报**搜索根 / 目录参数本身** | 规格两处场景（`find . -delete`、`git clean openspec/`）无法用「路径 match glob」表达：`.` 不比 `…/config.json` 命中；而「把 `**/` 规则当任意目录皆可命中」会让 `find src -delete` 与 `git clean src/` 全部误拦（AC-5 / AC-6 双向断言不允许）。concrete 前缀把规则锚定到执行根，既覆盖 `.` 又不越拦 `src` |
| D10 | inline 关联式 | 每 API 一条正则，**路径取第一个字符串实参**（引号字面量）；对整条（已归一的）命令文本跑，不切脚本段；裸脚本 / `-m` 形态不产生 op（承认漏面）；变量中转因取不到引号字面量而自然放行 | 规格 API 表 + 「不切脚本段」；关联式（API 名紧跟其路径实参）而非全文共现，使 `console.log('writeFileSync','<protected>')` 不误拦（AC-7 / R-4）。副作用已知：`copyFile*` / `shutil.copy*` / `os.rename*` 的**首参是源**，故「从受保护文件复制出去」也会被拒（见待决问题 1） |
| D11 | 拒绝文案四条 | `reason.rs` 持四条模板：内置配置串 / 用户 reason（`%s` / `%t` 全量替换）/ 无 reason 回落串 / 批量还原串。前三条与批量还原串均含「不要改用其他命令或工具尝试绕过」与「立即停止并通知用户」；模板文本内**禁** `openspec` / `config.json` 字面量（D2） | 规格第 7 面 + AC-8：`deny()` 不中断 run，两句是掐断换路链的唯一手段；批量还原串多带这两句是「MUST 含」的下界之上的强化（规格对它只点名「批量动作不可见 + 引导逐路径 `git restore <path>`」） |
| D12 | 正则预编译与性能纪律 | 全部正则集中在各自模块的 `static …: OnceLock<Vec<Regex>>`（沿 `sdk/log.rs:9` 的 `OnceLock` 先例，crate 中无 `LazyLock` 用法）；模式只用单量词线性形态，禁嵌套量词 / 回溯爆炸；**不设命令长度上限** | R-10：每条 bash 命令跑多层正则，预编译把常数项从「每次编译」降到一次；长度上限等于送一条绕过路径（规格明禁） |
| D13 | 拒绝的合成点 | protect 只返回 `Option<String>`；loop 侧命中即 `deny(&sender, "permission_denied", &name, &reason, &id)` + `outcome_result(..)` + `continue`。零新 subtype、零新事件词汇、零退出码 | 规格「拒绝形态复用既有 `deny()`」「保护模块 MUST NOT 自行发事件」；`bindings.ts` 零 diff（零新载荷） |
| D14 | 工作区根通道 | `compose_turn(…, workspace_root: &Path)`（新增尾参）→ `EngineFacade::with_workspace_root(PathBuf)` → `SdkRunner.workspace_root: Option<PathBuf>` → `PumpPayload.workspace_root: Option<PathBuf>` → 泵起播 `ProtectRules::load(payload.workspace_root.as_deref())` → `LoopTurn.protect`。命令层传**主仓 `root`**（`exec_root` / worktree 仍只进 `SessionCtx.workspace_root`） | 规格「root 只在命令层显式在手」+ D1（主仓口径）；`with_explore_note_store` 为同构先例；五个既有构造点（`new` / `with_resume_transcript` / `with_context_window` / `with_max_tokens` / `with_explore_note_store`）各补一个 `workspace_root: None`（不动签名） |
| D15 | 失败面矩阵 | 配置读取失败 → 仅内置（用户面空）；用户 glob 非法 / 展开超限 → 该条不命中（`match_glob` 已收敛）；`path` / `command` 缺失或非字符串 → `None`；`$` token / 空串 / 解析失败 → 跳过该 op；内置配置规则 fail-closed 恒在 | 规格「装配与判定的分工与失败面」表逐行；`evaluate` 任何输入不 panic（空命令、100 KB 命令、类型不符均收敛为 `None` / `Some`） |
| D16 | 出线与版本 | `packages/desktop/package.json` `version` `0.5.28` → `0.5.29`；零新 IPC 命令、零新 DTO、零 `bindings.ts` 变动；`plugins/**` 零改动零 bump | AC-13 / AC-12（`bindings:check` 零 diff）；用户可见变更（受保护文件拒写 + 拒绝处置指令）沿「仅用户可见变更才提升」纪律 |
| D17 | 明确不做（承认边界） | `xargs` / `sh -c` 递归拆解、`node -m` / 裸脚本内容、变量中转路径、base64 编码脚本、`printf` 喂 stdin、`workflow.json` 内置规则、审批通道、policy 三档改写；`policy.rs` / `bash.rs` 内部 doc 的「安全护栏为零」措辞**不改** | 规格「知情边界」+ proposal 范围裁决 D4/D5/D8；三处 spec 文本已在同一变更内 MODIFIED 为精确分界，代码注释不是契约面，动它属越界（登记于待决问题 3） |

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 规则装配 | 内置 glob 派生 + 用户规则合并 + 相对 glob 归一 + 规则序 + 结构 glob（批量还原用）+ 路径归一与解析单点（`normalize` / `resolve`）+ 子树相交判定 | `crates/infra/agent/src/sdk/protect/rules.rs`（新） | `config::{load, WriteProtection}`、`foundation::layout::{config_path, domain_dir_name, resolve}`、`checks::globmatch::match_glob`、`std::path` | 纯函数 + 一次 fs 读（间接经 `config::load`） |
| 判定入口 | `ProtectRules` 载体内核：工具名分派、`write` / `edit` 路径判定、`bash` 三路（提取 op → inline 关联式 → 批量还原）、路径归一与相对解析、reason 选择 | `crates/infra/agent/src/sdk/protect/mod.rs`（新） | 上述四个兄弟模块、`serde_json::Value` | 零 IO 纯函数 |
| shell 写入面提取 | 双变体等长掩码（heredoc / 引号）+ token 纪律 + 三态提取（重定向族 / PS 方言 / 删除族 / 移动族 / git 子命令）+ 档 B 五族 + 批量还原三规则 | `crates/infra/agent/src/sdk/protect/shell.rs`（新） | `regex`（预编译单点）、`rules::{glob_dir_hit, normalize, resolve, BulkGlobs}` | `Vec<char>` 索引 + 线性正则 |
| inline 关联式判别 | JS / PY 写 API 表逐条正则、首字符串实参为路径；对整条命令跑、不切脚本段 | `crates/infra/agent/src/sdk/protect/inline.rs`（新） | `regex` | 纯函数 |
| 拒绝文案 | 四条模板 + `%s` / `%t` 全量替换 + 按 `ReasonKind` 选串 | `crates/infra/agent/src/sdk/protect/reason.rs`（新） | 零依赖（纯字符串） | 纯函数 |
| loop 写入保护闸 | 守卫链第四段：沙箱之后、执行之前；`protect` 携带于 `LoopTurn`；命中走既有 `deny()` 后 `continue` | `crates/infra/agent/src/sdk/loop.rs`（修改） | `protect::ProtectRules` | 既有 loop 形态 |
| 泵装配点 | `SdkRunner.workspace_root` 持有 + `with_workspace_root` 旁路缝 + `PumpPayload.workspace_root` + 起播时 `ProtectRules::load` 装入 `LoopTurn` | `crates/infra/agent/src/sdk/runner.rs`（修改） | `protect::ProtectRules`、`std::path::PathBuf` | 既有 builder 缝形态 |
| 引擎门面缝 | `EngineFacade.workspace_root: Option<PathBuf>` + `with_workspace_root` + `runner_for` sdk 臂透传 | `crates/infra/agent/src/lib.rs`（修改） | 同 `with_explore_note_store` | 既有 builder 缝 |
| 组合根 | `compose_turn` 新增尾参并只做透传 | `crates/infra/agent/src/compose.rs`（修改） | `std::path::Path` | 既有 |
| 命令层注入点 | exec / change 双轨补 `root`（主仓口径） | `packages/desktop/src-tauri/src/commands/{exec,change_flow}/mod.rs`（修改） | — | 既有 |
| 依赖边 | `agent-runtime → checks`（唯一用途：glob 方言单点） | `crates/infra/agent/Cargo.toml`（修改） | — | Cargo workspace 依赖 |

---

## 变更清单

> 路径口径：新增文件表用仓库根相对全路径；修改文件表与 proposal「变更范围 - 实现文件」同口径（`crates/...` 与 `src/...` 省略 `packages/desktop/src-tauri/` 前缀）。

### 新增文件

| 文件路径 | 说明 | 关联条目 |
|----------|------|----------|
| `packages/desktop/src-tauri/crates/infra/agent/src/sdk/protect/mod.rs` | 模块根与判定入口：`ProtectRules`（`Vec<Rule>`）+ `ProtectRules::load` / `ProtectRules::evaluate`；私有 `evaluate_path` / `evaluate_shell` / `match_path` / `dir_rule_hit` / `deny_for`（路径归一与解析取 `rules::{normalize, resolve}`）；模块 doc 写明装配 / 判定分工、四段守卫链位置、射程边界与知情边界；末尾 `#[cfg(test)] mod {rules,shell,inline,reason}_test;` 四条声明 | 公共函数表 `ProtectRules::load` / `ProtectRules::evaluate` 行；类型表 `ProtectRules` 行；D1 / D4 / D5 / D15 |
| `…/sdk/protect/rules.rs` | 内置规则派生（`**/` + `config_path` root 相对形态）+ 用户规则合并（绝对原样 / 相对加 `**/` / 空 glob 跳过）+ 规则序（内置在前）+ 结构 glob（批量还原两条）+ 路径归一与解析单点（`normalize` / `resolve`）+ `glob_dir_hit`（D9）+ `Rule::concrete_prefix` | 公共函数表 `rules::*` 七行与 `Rule.concrete_prefix` 行；类型表 `Rule` / `ReasonKind` / `BulkGlobs` 行；D2 / D3 / D9 |
| `…/sdk/protect/shell.rs` | `quote_spans` / `heredoc_spans` / `mask_spans` 与双变体等长掩码；`tokenize` / `is_flag` / `is_literal_path` / `scan_command`；`extract_file_ops`（重定向族 / PS 方言 / 删除族 / 移动族 / 档 B 五族）；`extract_git_ops`（`restore` / `checkout` / `rm`）；`bulk_revert_detail(command, globs, cwd)`（`stash` / `clean` / `restore`·`checkout --` 命中 changes 面）；正则全部 `OnceLock` 预编译 | 公共函数表 `shell::*` 七行；类型表 `FileOp` / `OpKind` / `OpTarget` 行；D6 / D7 / D8 / D9 / D12 |
| `…/sdk/protect/inline.rs` | JS / PY API 表（每 API 一条正则，首字符串实参为路径）+ `write_targets`；doc 记「不切脚本段 / 裸脚本放行 / 变量中转与生成脚本为已知漏面 / copy 系首参为源」 | 公共函数表 `inline::write_targets` 行；D10 |
| `…/sdk/protect/reason.rs` | 四条模板常量 + `deny_reason`（按 `ReasonKind` 选串）+ `substitute`（`%s` / `%t` 全量）+ `bulk_revert_reason`；doc 记「两句功能句是掐断换路链的唯一手段」与「模板禁字面量」 | 公共函数表 `reason::*` 三行；D11 |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `crates/infra/agent/Cargo.toml` | `[dependencies]` 段 `foundation` 行之后插入 `checks = { workspace = true }`，附一行注释「写入保护规则匹配：复用 `checks::globmatch::match_glob`（glob 方言单点，禁第二份实现）」 | 唯一新增依赖边（infra → core 纯层）；`glob` / `regex` / `serde_json` 等既有依赖零改 |
| `sdk/mod.rs` | `pub(crate) mod preamble;` 与 `pub(crate) mod resume;` 之间加 `pub(crate) mod protect;`（**唯一改动**；四个测试模块声明在 `protect/mod.rs` 内，见「事实校准 1」） | 模块挂载点 |
| `sdk/loop.rs` | ① `use` 增 `crate::sdk::protect::ProtectRules`；② `LoopTurn` 增 `pub protect: ProtectRules`（doc：装配点算好、loop 纯消费，与 `defense` / `retry` / `liveness` 同列）；③ `:387` `sandbox_rewrite` 成功之后、`:399` 执行臂路由之前插入第四段：`if let Some(reason) = turn.protect.evaluate(&name, &sandboxed, &turn.cwd) { deny(&sender, "permission_denied", &name, &reason, &id).await; results.push(outcome_result(tool_call, reason)); continue; }`；④ 守卫链注释 `:364` 由三段改为四段（成员校验 → policy → sandbox → **写入保护** → 执行），并注明「沙箱先于保护（错误归属不抢报）」「`create_explore_note` 不在射程（分派表表达，非 loop 特判）」 | `deny()` `:447`、`outcome_result` `:475`、`finish_*`、`defend`、`sandbox_rewrite` 逐字零改；零新事件词汇（AC-1 / AC-8） |
| `sdk/runner.rs` | ① `SdkRunner` 增字段 `workspace_root: Option<PathBuf>`（doc：主仓 root，`None` = 仅内置规则）+ `new()` 内 `workspace_root: None`；② 新增 `pub fn with_workspace_root(self, root: PathBuf) -> Self`（与 `with_max_tokens` 同型旁路缝）；③ `PumpPayload` 增 `workspace_root: Option<PathBuf>`，`open_session` 的 `:165` 构造处传 `self.workspace_root.clone()`；④ `session_pump` 内 `LoopTurn` 之前加 `let protect = ProtectRules::load(payload.workspace_root.as_deref());`（每 run 一次、唯一 IO 点），`LoopTurn` 初始化补 `protect` | `new()` 四参签名不动（既有 51 处测试调用零改）；`build_model` / `resolve_output_tokens` / `anthropic_provider` / 泵 select 结构零改（AC-9 / AC-10） |
| `crates/infra/agent/src/lib.rs` | ① `EngineFacade` 增 `workspace_root: Option<PathBuf>`（doc：主仓 root，`None` = 仅内置规则）；② `new()` / `with_resume_transcript()` / `with_context_window()` / `with_max_tokens()` / `with_explore_note_store()` 五处构造补 `workspace_root: None`；③ 新增 `pub fn with_workspace_root(self, root: PathBuf) -> Self`（并列 builder 缝）；④ `runner_for` sdk 臂：`let mut runner = SdkRunner::new(...).with_max_tokens(self.max_tokens); if let Some(root) = &self.workspace_root { runner = runner.with_workspace_root(root.clone()); } Ok(Box::new(runner))` | `EngineKind::Cli` 臂与唯一 match 点结构零改；`ResumeTranscript` 类型零改 |
| `crates/infra/agent/src/compose.rs` | ① `use std::path::Path;`；② `compose_turn` 新增尾参 `workspace_root: &Path`（doc：命令行在手的主仓 root，与 `store` 半边同为「root 身份锚」，`exec_root` 不经此通道）；③ 门面链在 `.with_explore_note_store(..)` 之后追加 `.with_workspace_root(workspace_root.to_path_buf())` | 只做透传、零解析残留；`resolve_agent_engine` / `assemble_injections` / `begin` 零改 |
| `src/commands/exec/mod.rs` | `:132` 的 `compose_turn(…)` 调用补尾参 `Path::new(&root)`（**主仓 `root`**；`exec_root` 仍只进 `SessionCtx.workspace_root`） | 双 root 不变量不变（`for_root` 仍受 `root` 支配） |
| `src/commands/change_flow/mod.rs` | `:143` 同上（**主仓 `root`**，MUST NOT 传 `exec_root` / worktree 路径） | change 轨执行根为 worktree 时规则仍取主仓（AC-10） |
| `packages/desktop/package.json` | `version` `"0.5.28"` → `"0.5.29"`（单行） | 用户可见变更（受保护文件拒写 + 拒绝处置指令）；`plugins/**` 零 bump |

### 公共函数 / API

> 口径：均为 crate 内可见性（`protect` 为 `pub(crate) mod`），无跨 crate 导出、无 IPC 命令、无 CLI 子命令。私有函数（`quote_spans` / `heredoc_spans` / `mask_spans` / `is_literal_path` / `scan_command` / 五族提取子函数 / `restore_ops` / `checkout_ops` / `static_prefix` / `normalize_user_glob` / `evaluate_path` / `evaluate_shell` / `match_path` / `dir_rule_hit` / `deny_for` / 正则表构造器）不入表。

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|-----------|------|------|
| `ProtectRules::load` | `sdk/protect/mod.rs` | 新增 | `pub(crate) fn load(workspace_root: Option<&Path>) -> ProtectRules` | 唯一 IO 点：内置规则 + （root 在场时）`config::load(root)` 的用户规则合并；`None` → 仅内置（不产出空规则集） |
| `ProtectRules::evaluate` | `sdk/protect/mod.rs` | 新增 | `pub(crate) fn evaluate(&self, tool: &str, input: &Value, cwd: &Path) -> Option<String>` | 零 IO 判定：`write` / `edit` 按 `path`；`bash` 按 `command` + `cwd`；其余工具恒 `None`；返回完整拒绝串 |
| `rules::load` | `sdk/protect/rules.rs` | 新增 | `pub(super) fn load(workspace_root: Option<&Path>) -> Vec<Rule>` | 规则合并单点：`builtin_rules()` 在前 + root 在场时 `user_rules(root)` 追加 |
| `rules::builtin_rules` | `sdk/protect/rules.rs` | 新增 | `pub(super) fn builtin_rules() -> Vec<Rule>` | 恒一条 `**/` + `config_path(Path::new(""))` 相对形态，`ReasonKind::BuiltinConfig` |
| `rules::user_rules` | `sdk/protect/rules.rs` | 新增 | `pub(super) fn user_rules(root: &Path) -> Vec<Rule>` | 逐条取 `write_protection.files[]`：`glob` 为 `None` 跳过；绝对原样；相对加 `**/`；`reason` 落 `ReasonKind::User` |
| `rules::bulk_globs` | `sdk/protect/rules.rs` | 新增 | `pub(super) fn bulk_globs() -> BulkGlobs` | 批量还原两条结构 glob 的派生单点（域目录树 / changes 子树） |
| `rules::glob_dir_hit` | `sdk/protect/rules.rs` | 新增 | `pub(super) fn glob_dir_hit(dir: &str, glob: &str, anchor: &Path) -> bool` | 子树相交判定（D9）：`find` 搜索根与 `git clean` 目录参数共用 |
| `rules::normalize` | `sdk/protect/rules.rs` | 新增 | `pub(super) fn normalize(raw: &str) -> String` | 文本归一单点（零 IO）：剥 Windows 扩展长度前缀 → `\` → `/` → 按 `/` 切段去空段与 `.` 段、保留 `..` 段 → 重拼并保留前导 `/`；`write` / `edit` 的 canonical 路径、`bash` 的整串命令与解析后路径共用 |
| `rules::resolve` | `sdk/protect/rules.rs` | 新增 | `pub(super) fn resolve(anchor: &Path, raw: &str) -> Option<String>` | 提取路径解析单点（零 IO）：空串或含 `$` → `None`（放行该 op）；绝对原样归一、相对 `anchor.join` 后归一 |
| `Rule.concrete_prefix` | `sdk/protect/rules.rs` | 新增 | `pub(super) fn concrete_prefix(&self, anchor: &Path) -> String` | glob → concrete 静态前缀（`**/` 起手者锚定到 `anchor`） |
| `shell::extract_file_ops` | `sdk/protect/shell.rs` | 新增 | `pub(super) fn extract_file_ops(command: &str) -> Vec<FileOp>` | 三态提取总入口（重定向族 + PS 方言 + 删除族 + 移动族 + 档 B 五族 + git 子命令），保序、不折叠、不去重（`git rm` 与 `rm` 双记由判定侧「首个命中即止」消化） |
| `shell::extract_git_ops` | `sdk/protect/shell.rs` | 新增 | `pub(super) fn extract_git_ops(command: &str) -> Vec<FileOp>` | `git restore` / `git checkout` / `git rm` 的 write / revert / delete ops（`--source=` / `-s` / `<ref> --` 决定 write） |
| `shell::mask_heredocs` | `sdk/protect/shell.rs` | 新增 | `pub(super) fn mask_heredocs(command: &str) -> String` | 变体 A：只掩 heredoc 体、保留引号（写入正则与参数切片的运行面） |
| `shell::mask_quoted_and_heredoc` | `sdk/protect/shell.rs` | 新增 | `pub(super) fn mask_quoted_and_heredoc(command: &str) -> String` | 变体 B：掩 heredoc + 引号（命令名定位面）；与 A 等长 |
| `shell::tokenize` | `sdk/protect/shell.rs` | 新增 | `pub(super) fn tokenize(args: &str) -> Vec<String>` | 空白切分 + 剥包裹引号 + 剔空；**不做** `$` 过滤（`git clean` 零路径判定依赖原始 token 计数） |
| `shell::is_flag` | `sdk/protect/shell.rs` | 新增 | `pub(super) fn is_flag(token: &str) -> bool` | `-` 起手判定 |
| `shell::bulk_revert_detail` | `sdk/protect/shell.rs` | 新增 | `pub(super) fn bulk_revert_detail(command: &str, globs: &BulkGlobs, cwd: &Path) -> Option<String>` | 命中即返回**中间事实串**（如「git stash 会批量改写工作区」），由 `reason::bulk_revert_reason` 包成完整文案；`cwd` 仅供 `git clean` 目录参数与 `git restore` 目标的子树判定锚定（内部不做 IO） |
| `inline::write_targets` | `sdk/protect/inline.rs` | 新增 | `pub(super) fn write_targets(command: &str) -> Vec<String>` | 按 API 表逐个正则取首字符串实参，聚成候选路径（不判命中） |
| `reason::deny_reason` | `sdk/protect/reason.rs` | 新增 | `pub(super) fn deny_reason(kind: &ReasonKind, path: &str, tool: &str) -> String` | 三态选串：内置 / 用户 reason（占位符替换）/ 回落串 |
| `reason::substitute` | `sdk/protect/reason.rs` | 新增 | `pub(super) fn substitute(template: &str, path: &str, tool: &str) -> String` | `%s` → 路径、`%t` → 工具名，**全量**替换所有出现 |
| `reason::bulk_revert_reason` | `sdk/protect/reason.rs` | 新增 | `pub(super) fn bulk_revert_reason(detail: &str, tool: &str) -> String` | 批量还原专用文案（含「逐路径 `git restore <path>`」引导 + 两句功能句） |
| `EngineFacade::with_workspace_root` | `crates/infra/agent/src/lib.rs` | 新增 | `pub fn with_workspace_root(self, root: PathBuf) -> Self` | 主仓 root 注入缝（与 `with_explore_note_store` 并列） |
| `SdkRunner::with_workspace_root` | `sdk/runner.rs` | 新增 | `pub fn with_workspace_root(self, root: PathBuf) -> Self` | runner 侧旁路缝（与 `with_max_tokens` 同型） |
| `compose_turn` | `crates/infra/agent/src/compose.rs` | 修改 | `pub fn compose_turn(stores: &WorkspaceStores, registry: Arc<StopRegistry>, streams: Arc<SessionStreamRegistry>, store: Arc<Store>, agent: Option<i64>, workspace_root: &Path) -> Result<ComposedTurn, String>` | 新增尾参 `workspace_root`（主仓 root），只做门面透传 |

### 类型定义

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|--------|----------|-----------|------|
| `ProtectRules` | `sdk/protect/mod.rs` | 新增 | `pub(crate) struct ProtectRules { rules: Vec<Rule> }`（`#[derive(Debug, Clone)]`）；规则集纯数据，字段私有、只由 `load` 构造 |
| `Rule` | `sdk/protect/rules.rs` | 新增 | `pub(super) struct Rule { pub(super) glob: String, pub(super) reason: ReasonKind }`（`Debug, Clone`）；内置 / 用户同型，差别只在 `reason` 变体与位次 |
| `ReasonKind` | `sdk/protect/rules.rs` | 新增 | `pub(super) enum ReasonKind { BuiltinConfig, User(Option<String>) }`（`Debug, Clone`）；三态选串的唯一依据（不做 glob 字面量比对） |
| `BulkGlobs` | `sdk/protect/rules.rs` | 新增 | `pub(super) struct BulkGlobs { pub(super) artifacts: String, pub(super) changes: String }`（`Debug, Clone`）；两条结构 glob（域目录树 / changes 子树），由 `foundation::layout` 派生 |
| `FileOp` | `sdk/protect/shell.rs` | 新增 | `pub(super) struct FileOp { pub(super) kind: OpKind, pub(super) target: OpTarget }`（`Debug, Clone, PartialEq, Eq`） |
| `OpKind` | `sdk/protect/shell.rs` | 新增 | `pub(super) enum OpKind { Write, Delete, Revert }`（`Debug, Clone, Copy, PartialEq, Eq`）；`Revert` 不进保护判定（仅批量还原消费） |
| `OpTarget` | `sdk/protect/shell.rs` | 新增 | `pub(super) enum OpTarget { Path(String), Tree(String) }`（`Debug, Clone, PartialEq, Eq`）；`Tree` = `find` 搜索根（走 D9 子树判定），`Path` = 普通路径 |
| `LoopTurn` | `sdk/loop.rs` | 修改 | 增字段 `pub protect: ProtectRules`（doc：装配点算好、loop 纯消费；每 run 一份） |
| `SdkRunner` | `sdk/runner.rs` | 修改 | 增字段 `workspace_root: Option<PathBuf>`（`None` = 未注入 → 仅内置规则） |
| `PumpPayload` | `sdk/runner.rs` | 修改 | 增字段 `workspace_root: Option<PathBuf>`（泵起播装配的原料） |
| `EngineFacade` | `crates/infra/agent/src/lib.rs` | 修改 | 增字段 `workspace_root: Option<PathBuf>`（builder 缝载荷） |

### 配置

| 配置键 | 所在文件 | 新增/修改 | 值类型 | 默认值 | 说明 |
|--------|----------|-----------|--------|--------|------|
| `write_protection.files[].glob` | `openspec/config.json`（零 schema 扩展；消费侧新增读取） | 既有键（读取方新增） | `string` | 未配置（`files` 缺席 → 用户规则为空） | 绝对形态原样匹配；相对形态自动前缀 `**/`；空串 / 非法值已被 `config` 组装层剔除，本侧不再校验 |
| `write_protection.files[].reason` | 同上 | 既有键（读取方新增） | `string`（可选） | 缺省 → 回落串 | 原样透传，`%s` → 命中路径、`%t` → 工具名（全量替换） |
| `version` | `packages/desktop/package.json` | 修改 | `string` | `0.5.28` | → `0.5.29`（用户可见：受保护文件拒写 + 拒绝处置指令） |

> 无新增配置键：内置规则不可配置、不可移除（fail-closed 面）；不设「关闭保护」开关。

---

## 数据模型

持久化面**零结构改动**（无表、无索引、无迁移、无新配置键）。本变更只涉及**进程内载体**与一处「配置的只读消费」：

```
rules::load(Option<&Path>)            ← 唯一 IO 点（每 run 一次，泵起播）
├── 内置：Rule { glob: "**/" + config_path("") 相对形态, reason: BuiltinConfig }
└── 用户：Rule { glob: 绝对原样 | "**/" + 相对,        reason: User(reason | None) }

ProtectRules { rules: Vec<Rule> }     ← 位次即优先级（内置在前）
└── LoopTurn.protect

evaluate(tool, input, cwd) -> Option<String>   ← 零 IO，每 tool_call
├── write / edit : path → normalize → match_path
└── bash : command → normalize
    ├── extract_file_ops  → Path(path)  → match_path
    │                     → Tree(root)  → rules::glob_dir_hit
    ├── inline::write_targets → Path(path) → match_path
    └── bulk_revert_detail(globs, cwd) → 事实串 → 完整文案
```

| 面 | 载体 | 语义 / 生命周期 |
|----|------|-----------------|
| 规则集 | `ProtectRules.rules` | 每 run 铸造一次，随 `LoopTurn` 存活到本轮结束；不落盘、不缓存到会话之外（run 内改 config 不生效，下一 run 重读） |
| 提取结果 | `Vec<FileOp>` / `Vec<String>`（inline） | 单次 `evaluate` 的栈上产出，随判定返回即释放；无状态、无跨调用记忆（同文件二次拒绝不做特殊处理，D17） |
| 拒绝串 | `Option<String>` | 单条字符串，同时成为 `SystemNotice.payload.reason` 与同 id is_error `ToolResult.content`（loop 侧复用同一 `reason` 变量；protect 零事件流出） |
| 配置（只读） | `config::load(root) -> ConfigReport` | 只取 `config.write_protection.files[]`；`diagnostics` **不参与**保护面判定（内置面独立于配置解析结果） |
| 工作区根 | `EngineFacade.workspace_root` / `SdkRunner.workspace_root` / `PumpPayload.workspace_root` | 主仓 root 的逐层下行副本（`Option<PathBuf>`）；`None` = 未注入 → 仅内置规则，非「保护关闭」 |

关系面：`Rule` 与 `WriteProtectionFile` 是**投影关系**（`glob` / `reason` 字段级一一对应，`ReasonKind` 承载「有无 reason」），不是共享类型——`config` 是 core 层配置模型，`Rule` 是引擎内匹配载体，二者不合并（避免把匹配语义压进 config 契约）。无外键、无跨表引用、无 `bindings.ts` 出线。

---

<!-- 路由 / API 设计：本变更不涉及。零新增 / 修改 Tauri IPC 命令、零 HTTP 端点、零 CLI 子命令；拒绝走既有 `SystemNotice{subtype:"permission_denied", payload:{tool, reason}}` + is_error `ToolResult`（payload 词汇零扩展），`src/types/generated/bindings.ts` 零 diff（D13 / D16）。 -->

---

## 依赖

### 运行时依赖

| crate / 特性 | 变更 | 用途 |
|--------------|------|------|
| `agent-runtime` → `checks = { workspace = true }` | **新增** | 唯一用途：`checks::globmatch::match_glob` 作规则匹配（glob 方言单点，禁第二份实现）；`try_expand_braces` 不调用 |
| `agent-runtime` → `config` | 不变（既有边） | `config::load(root)` 取 `write_protection.files[]`（唯一 IO 点的原料） |
| `agent-runtime` → `foundation` | 不变（既有边） | `layout::{config_path, domain_dir_name, resolve}` 派生内置 glob 与两条结构 glob（字面量隔离唯一触点） |
| `agent-runtime` → `regex` | 不变（既有边，`sdk/tools.rs` 已用） | 提取器与 inline 关联式的模式引擎；经 `OnceLock<Vec<Regex>>` 预编译（D12） |
| `agent-runtime` → `serde_json` | 不变（既有边） | `ToolCall.function.arguments: Value` 的字段读取（`path` / `command`） |
| `checks` 的反向依赖 | **零** | `crates/core/checks` 依赖 `config` / `foundation` / `glob` / `serde` / `serde_json` / `time`——无 infra 依赖，crate 图机械无环（`desktop-crate-layout` ADDED 条款） |
| 新增第三方 crate | **无** | 掩码 / 提取 / 关联式 / 文案全为 `regex` + std（`Vec<char>` / `Path` / `OnceLock`） |

### 构建 / 测试依赖

| crate / 特性 | 变更 | 用途 |
|--------------|------|------|
| `agent-runtime` dev-dependencies | 不变（`tempfile` / `native_db` / `rig test-utils` / `tokio test-util`） | 规则装配用例沿用 `tempfile` 造真实配置；判定用例零 fs（纯内存构造 `ProtectRules`，D1） |
| `crates/core/foundation`、`crates/core/{config,checks}` | **零改动** | `foundation` 依赖面零变化（不引 `glob` crate，不引 workspace 内依赖）；`checks::globmatch` 签名与语义零改；`config` 模型与诊断面零改 |

---

## 验收标准回应

| AC | 设计落点 |
|----|----------|
| AC-1 | 内置规则由 `rules::builtin_rules()` 恒产一条（D2），与 `config::load` 结果完全解耦（失败面表「内置 fail-closed」）；闸门在任何 `permission_mode` 之后（D13，`loop.rs` 修改 ③）；`read` / `grep` 由 `evaluate` 分派表恒 `None`（D4） |
| AC-2 | 相对 glob 加 `**/`、绝对原样、空 glob 跳过（`rules::user_rules`）；`reason` 三态与 `%s` / `%t` 全量替换（D3 / D11）；本仓现存的真实无 reason 规则即回落串用例（`rules::user_rules` + `reason::deny_reason`）；配置缺失 / 坏 JSON 只减用户面（D15） |
| AC-3 | 三态提取的重定向族 / PS 方言 / 删除族 / 双参移动族 / `git restore --source=` / `git checkout <ref> --`（`shell::extract_file_ops` + `shell::extract_git_ops`，D7 / D8） |
| AC-4 | 档 B 五族取位口径（D8）；`dd if=` 显式跳过、`sed` 无 `-i` 恒零 op |
| AC-5 | 双变体等长掩码 + `$` 落点丢弃 + `2>&1` 捕获后过滤 + `[^-=]` 排除集照抄（D6 / D7）；`Tree` 判定不采用「任意目录皆命中」（D9）；同一文件的五条等价写法由 5 个族分别覆盖（重定向 / `rm` / `cp` / inline / `sed -i`） |
| AC-6 | `bulk_revert_detail` 三条规则（`stash` 除 `list`·`show`、`clean` 无路径或目录参数命中、`restore`·`checkout --` 命中 changes 子树）；命令名在变体 B 定位、参数在变体 A 切片（D6 / D9） |
| AC-7 | `inline::write_targets` 关联式（D10）：「API 名 + 其路径实参」绑定，无 `^(python\|node)\s` 豁免；对整条命令跑故嵌套 `bash -c 'node -e "…"'` 一并覆盖；裸脚本 / `-m` 零 op |
| AC-8 | 四条模板均含「不要改用其他命令或工具尝试绕过」与「立即停止并通知用户」；`evaluate` 返回单串，loop 以同一变量写 notice 载荷与 ToolResult 内容（D11 / D13） |
| AC-9 | 装配单点：`rules::load` 是唯一 `config::load` 调用点、只在 `session_pump` 起播调用一次（D1 / D14）；`protect` 四模块零 `fs` / 零 `canonicalize`（源码自查项写入 tasks） |
| AC-10 | 命令层传主仓 `root`（D14 修改文件表两行）；worktree 内副本仍被 `**/` 内置 glob 命中（内置 glob 无 root 锚定）；`None` → 仅内置（`SdkRunner.workspace_root: Option`） |
| AC-11 | policy / sandbox / tools / explore_note 逐字零改（修改文件表只碰 `mod.rs` / `loop.rs` / `runner.rs` 三处接线）；闸门只做「档位已放行之后的追加拒绝」（D13），未命中路径 `evaluate → None` → 执行照常 |
| AC-12 | 零新出线载荷、零新 IPC（D16）；新模块的装配面（`rules`）与判定面（`mod` / `shell` / `inline` / `reason`）均为纯函数，覆盖率由新模块用例承接（用例编写归测试相位） |
| AC-13 | `package.json` `0.5.28` → `0.5.29`（配置表）；`plugins/**` 零改动零 bump；`openspec/config.json` 零 schema 扩展（配置表两行均为既有键） |

---

## 待决问题

1. **`copyFile*` / `shutil.copy*` / `os.rename*` 的首参是源**。规格固定「路径取第一个字符串实参」，故 `shutil.copy('<protected>', '/tmp/x')`（从受保护文件**复制出去**）与 `fs.copyFileSync('<protected>', dst)` 会被拒。方向是 over-block（安全侧），AC-7 未覆盖；若产品认为「只读复制应当放行」，需在规格相位改为「copy 族取第二字符串实参（目标位）」，本设计仅改 inline 表的一行正则分组。
2. **Windows 盘符大小写与绝对 glob**。`checks::globmatch` 的 `MatchOptions` 为 `case_sensitive: true`，而沙箱 canonicalize 产出的盘符大小写由系统决定（通常大写）；用户手写小写盘符的绝对 glob（`d:/ws/secret.md`）可能不命中。插件端 picomatch 同款问题，属方言既有行为（本变更 MUST NOT 自造第二份方言）。若要收敛，需在 `globmatch` 侧加盘符归一——超出本变更范围。
3. **`policy.rs` / `bash.rs` 内部 doc 的「安全护栏为零」措辞**。三处 spec 文本已在同一变更内 MODIFIED 为精确分界，但两个源码文件的模块 doc 仍写「安全护栏为零」。提案文件表为封闭集（`sdk/{bash,tools,policy,…}.rs` 语义零改），故本设计**不改**这两处注释，仅登记：代码注释不是契约面，口径以 spec 为准；若评审要求一致，属注释级一次小改（两行）。
4. **`git clean` 参数为通配形态**（`git clean -fd 'openspec/*'`）。`glob_dir_hit` 把参数当**字面目录**处理，参数自带通配符时静态前缀把 `*` 段截断，可能漏拦。方向是 fail-open（漏拦，非误拦）；AC-6 场景为字面目录，暂不覆盖；若要覆盖需在判定前对参数做 glob 展开（引入第二种展开语义，风险高于收益）。
5. **`find` 的 `Tree` 判定不覆盖 `-exec rm {} +`**。规格只点名 `-delete` 族；`-exec` 形态下 `rm` 出现在 `scanCommand` 的参数位但命令名定位面把它当参数而非命令（`find` 段内 `-exec rm x` 的 `rm` 前是 `-exec` 空格，`(?:^|[\s;&|(])(?:…rm…)` 会命中并把 `x` 记为 delete——即被 `-exec` 覆盖的路径仍可拦，但 `{}` 占位不可解析）。作为知情边界留痕（与 `xargs` 同类），不在本篇扩展。
6. **命名隔离扫描的实际覆盖缺口**（事实校准 3）：`crates/infra/agent/**` 当前不在 foundation 的双禁令扫描清单内，故 protect 源码的「零字面量」无机械门。本设计以 tasks 中的 `grep` 自查 + code review 承载；修 foundation 扫描不在本变更范围（零改动红线），建议另立小项。
