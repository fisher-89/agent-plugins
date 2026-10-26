# 设计: desktop-build-mode-split

> **变更**: desktop-build-mode-split
> **日期**: 2026-10-26
> **基于**: `proposal.md` 与 `specs/{desktop-build-modes,desktop-workspace-store,desktop-data-dimensions,desktop-release-pipeline}/spec.md`

---

## 提案与规格同步状态

proposal 的九面与四份 delta spec 已定稿（12 项决策 D1–D12、三档信号矩阵已实测、桌面端零实现）。本设计不重复其论证，只做四件事：

1. 把启动序列落成**可编译的确定形状**（含 `setup_hook` 命名化、`manage(DataRoot)` 与 `State` 取根、自检的插入点与退出路径）；
2. 把 `env` / `selfcheck` 的**纯函数面与失败面矩阵**逐项钉死（归一 / 五项校验 / 提示三要素 / 摘要十键 / 退出码）；
3. 定案两处 proposal 与 spec 之间的字面张力（prod 侧自检拒绝分支的形态、`models=` 的取值口径）；
4. 给出「隔离 cargo target 目录」的**落地形态与校准判据**（R-2），并逐条对齐 AC-1..AC-17。

起点核实（工作树实况；路径前缀 `packages/desktop/`，锚点为改动前行号）：

- **壳层入口**：`src-tauri/src/main.rs`——`windows_subsystem` 属性 `:1`（release 隐藏控制台 ⇒ dev 构建天然 CONSOLE，`eprintln` 可见）；`use std::path::PathBuf` `:2`；`use tauri::{generate_handler, Manager}` `:5`；`const DATA_DIR` `:18`、`const LOGS_DIR_NAME` `:21`；builder 装配 `:24-41`（updater `:24`、单实例 `#[cfg(desktop)]` `:26-37`、devtools `#[cfg(debug_assertions)]` `:38-41`）；`dialog` / `opener` / 内联 `setup` 闭包 `:42-75`（数据根 `:51`、`WorkspaceStores::open` `:52`、引擎日志 `:55`、五个 `manage` `:57-73`）；`invoke_handler` `:76`；`run(...).expect(...)` `:77-78`。
- **壳层库目标**：`src-tauri/src/lib.rs` 仅两行 `pub mod bindings;` / `pub mod commands;`。
- **命令登记面**：`src-tauri/src/commands/mod.rs` 十轨 `pub mod …`（`agents` / `change_flow` / `changes` / `config` / `db` / `exec` / `explores` / `stats` / `watch` / `workspaces`）+ `all_commands!` 宏（末条 `config::workspace_config`）+ `TEST_PATH_LOCK` 装置。薄包装 + `*_inner` 微形态先例：`commands/config/mod.rs`（`workspace_config` / `workspace_config_inner`，模块末 `#[cfg(test)] mod mod_test;`）；`State<'_, WorkspaceStores>` 用法先例：`commands/db/mod.rs`。
- **bindings 生成**：`src-tauri/src/bindings/mod.rs` 由 `all_commands!(collect_commands)` 组装 specta builder，`ErrorHandlingMode::Throw`、`dangerously_cast_bigints_to_number`；新增命令 + `Serialize + Type` DTO 即自动进入生成物。
- **store 常量现状**：`crates/infra/store/src/store.rs:381` `pub(crate) const GLOBAL_DB_FILE_NAME: &str = "desktop-global.redb"`；`:384` `pub(crate) const WORKSPACES_DIR_NAME: &str = "workspaces"`；crate 根重导出清单 `crates/infra/store/src/lib.rs:14`（当前 `pub use store::{DbDimension, Store, StoreError, WorkspaceStores, EXPLORE_PLACEHOLDER_PREFIX};`）。`WorkspaceStores::open(data_root)` `store.rs:2217-2224`；`Store::open_with` 的「建父目录 + 缺文件即 create」`store.rs:503-522`；`list_models()` `store.rs:2144-2146`（走 `envelope::list_models`，模型名来自 `MODEL_ENTRIES` 注册表 `envelope.rs:77+`）；`list_workspaces()` `store.rs:560`。
- **日志时间戳先例**：`crates/infra/agent/src/sdk/log.rs:17-31`——引擎日志行格式 `[<unix 毫秒>] <line>`，`OpenOptions::new().create(true).append(true)` + `writeln!`，IO 失败静默。
- **前端现状**：`src/hooks/use-updater.ts:28` `useUpdater()` 无参；`:52-77` `useStartupCheck`（`:64` `if (!import.meta.env.DEV)` 为全仓唯一 `import.meta.env` 模式判据）；`src/app.tsx:44-54` `ShellHeader`（`:49` `<strong>Dev Team</strong>` 后即 `<span className="flex-1" />`）、`:58` `useUpdater()` 调用；`src/types/generated/bindings.ts` 由 specta 导出，`src/types/dto.ts` 为纯 `export type *` shim（禁手写类型）。
- **构建脚本**：`package.json:3` `version 0.5.30`；`:8` `build` = `bindings:export && server:build`；`:10` `server:build`；`:11` `bindings:export`（`cargo run -p export-bindings`，独立编译态）；`tauri.conf.json` `build.beforeBuildCommand = pnpm client:build`、`bundle.createUpdaterArtifacts = true`、`identifier = dev.wps.dev-team`；`packages/desktop/.gitignore` 覆盖 `src-tauri/target`。
- **CI 现状**：`.github/workflows/desktop-release.yml` build matrix 经 `tauri-apps/tauri-action@v0` 直驱 tauri CLI，`args` 仅 `--bundles nsis` / `--bundles app,dmg`。
- **tauri-cli 2.12.4 能力边界**（实测 `tauri build --help`）：有 `--debug` / `--no-bundle`，**无** `--target-dir`；CLI 内以 `cargo metadata` 的 `target_directory` 定位产物（二进制内含 `CARGO_TARGET_DIR` 与 `failed to run 'cargo metadata' command to get target directory`）。

**事实校准（不影响 proposal 的九面，但决定实现形态）**：

1. **`models=` 的取值口径**。`list_models()` 返回的是信封注册表的**短名**（`workspace` / `agent_provider` / `agent_instance`，见 `envelope.rs:77-98`），不是 `WorkspaceRecord,AgentProviderRecord,AgentInstanceRecord` 的 Rust 类型名。spec 明写「`models=` SHALL 取自全局库已注册模型清单（`list_models()`）」，故落地为短名逗号连接，proposal 示例视为示意文本（若将来要类型名形态，需在 store 侧新增 API，本期不做，登记为待决问题 3）。
2. **prod 侧自检拒绝分支的形态**。proposal §4 的代码草稿用 `#[cfg(not(debug_assertions))] if want_self_check { … }`；而 spec「`--self-check` 自检契约」写「MUST NOT 用 `#[cfg]` 把该分支整块删掉」，AC-12 与 R-8 更明确「该分支**不在** `#[cfg]` 内」。本设计取 **`cfg!` 常量短路**（不留任何 `#[cfg]` 属性），见 D3。
3. **时间戳零新依赖**。仓库既有日志约定是 `[unix 毫秒]` 前缀（校准项上条），`boot-error.log` / `ignored-env.log` 沿同一约定 ⇒ proposal 变更范围无需新增 `Cargo.toml` 行、不引 `time` / `dirs`（`time` 虽在 workspace 依赖表内，但 `dev-team` 包未依赖它）。
4. **自检的「探库」是真开库**。`WorkspaceStores::open` 会 `create_dir_all` 数据根并建/开全局库（`store.rs:503-522`），故自检对**不存在**的数据根会顺手建出 `<数据根>/desktop-global.redb`（与 GUI 首次启动同源、同副作用，不做 dry-run 变体）；`<数据根>/workspaces/` 在只开全局库时通常仍不存在 ⇒ `workspaces_dir_exists=false` 是常态而非异常。
5. **`#[cfg(desktop)]` 迁移**：单实例注册当前是 `#[cfg(desktop)]`（`main.rs:26`），改为 `#[cfg(all(desktop, not(debug_assertions)))]` 只动属性一处；`tauri-plugin-single-instance` 的 Cargo 依赖边与 `capabilities/default.json` 均零改动（命令式插件 init 不产生 capability 需求）。
6. **`import.meta.env.DEV` 的两处出处**：代码一处（`use-updater.ts:64`），文档一处（`RELEASE.md:116`「`tauri dev` 不检查更新(`import.meta.env.DEV` 守卫)」）。AC-1 的扫描域只覆盖 `src/**` 与 `src-tauri/src/**`，但文档陈述必须同批改写，否则留下与实现相反的指引。

**只读红线零触点**：`.github/workflows/desktop-release.yml`、`src-tauri/tauri.conf.json`、`src-tauri/capabilities/default.json`、`crates/**`（除 store 两常量 `pub(crate)` → `pub`）、`src/views/**`、`src/components/**`、`src/routes.tsx`、`src/lib/**`、`plugins/**`、`openspec/config.json`、`openspec/changes/desktop-build-mode-split/explore.md`。

---

## 关键设计决策

| # | 议题 | 定稿 | 理由 |
|---|------|------|------|
| D1 | 判别源落点 | Rust 侧唯一判别表达式是 `env::mode()` 内的 `cfg!(debug_assertions)`；装配期分歧用 `#[cfg]` 属性（单实例 / devtools）；`resolve_root(home, is_dev)` 的 `is_dev` 由 `main` 传 `cfg!(debug_assertions)`；前端零推导，只经 IPC | spec「判别源单点」+ 双禁令；`tauri::is_dev()` 与 `import.meta.env.DEV` 在三档矩阵下各有一档错答（proposal 已实测） |
| D2 | 启动序列形状 | `args → prod 侧拒绝 → builder 装配（updater / 单实例[cfg] / devtools[cfg] / dialog / opener / setup_hook） → build() → home → 数据根单点解析（失败即双写 + exit 2） → manage(DataRoot) → [dev] 自检 exit → run`；`setup` 收成命名函数 `setup_hook` | 自检放 `setup` 会闪窗（tauri 内部 `setup` 在 `Ready` 执行且先建窗后跑钩子）；数据根放 `setup` 则失败文本会退化成 `Failed to setup app`，丢掉自然语言提示（AC-15） |
| D3 | prod 拒自检的分支形态 | `if want_self_check && !cfg!(debug_assertions) { eprintln!(…) ; exit(2) }`——**不用** `#[cfg]` 属性 | AC-12 / R-8 / spec 三处均要求「该分支不在 `#[cfg]` 内」且「不许用 `#[cfg]` 整块删掉」；`cfg!` 是常量折叠的编译期宏，两种形态生成代码等价，且同时满足「prod 产物内含该分支」与「dev 构建里零行为」 |
| D4 | 数据根解析 API 三分 | `prod_root(home)` 签名**不接收** env；`resolve_dev_root(home, env: Option<&OsStr>)` 是纯函数；`resolve_root(home, is_dev)` 是薄分派，`is_dev == false` 时**早退**到 `prod_root` 才不读 env；全仓 env 读取点只有该 dev 分支与 prod 侧 `var_os().is_some()` 探测 | 「prod 不认 env」由结构保证而非调用纪律；纯函数侧让三分优先级与五项拒绝全部零 fs 可测 |
| D5 | 归一口径 | 私有 `normalize(path) -> PathBuf`：按 `Path::components()` 逐段重拼——`CurDir` 丢弃、`ParentDir` 在无可弹段时原样保留、`Prefix` / `RootDir` / `Normal` 依次压入；**禁** `canonicalize`。比较专用 `compare_key(path) -> PathBuf`：`normalize` 后 Windows 走 `to_string_lossy().to_lowercase()`（`#[cfg(windows)]` 双定义），其余原样 | 目标目录通常尚不存在 ⇒ `canonicalize` 会 `NotFound` 误报；`components()` 天然吃掉 `.` / 重复分隔符 / 尾分隔符；大小写等价形态必须同判（AC-7） |
| D6 | 五项校验与错误载体 | 顺序：① `trim()` 空 → `Blank`；② `!Path::is_absolute()` → `Relative`（含 Windows 的「仅根」`\foo`，MUST NOT 按 cwd 解析）；③ `compare_key` 相等 prod 根 → `EqualsProd`；④ `compare_key(prod).starts_with(compare_key(候选))` 且不等 → `AncestorOfProd`；⑤ `Path::is_file()` → `NotDirectory`；其余接受。每个变体自带 `raw: String`（收到的原值） | 提示三要素之一「回显收到的值」由类型携带，渲染侧不二次取 env；祖先判定复用 `Path::starts_with` 的段比较，不会把 `C:\Users\w` 误判成 `C:\Users\wps` 的祖先 |
| D7 | 提示文案 | 单一渲染函数 `render_root_error(&RootError, home)`：每变体一段自然语言，统一含三要素（指名 `DEV_TEAM_DATA_ROOT`、回显原值、给可照抄下一步），「下一步」二选一固定暗示——删掉变量改用 `<home>\.dev-team\dev`，或改指独立目录（示例 `<home>\dev-team-data\accept-1`）；默认根与示例由 `home` 派生，不写字面量 | spec「MUST NOT 只输出枚举名 / 错误码 / 裸路径比较结论」；GUI 启动与 `--self-check` 共用同一函数 ⇒ 双面文案逐字一致（AC-11「env 非法时自检与 GUI 同文案」） |
| D8 | 双写与留痕 | `append_boot_error(home, msg)` → `<home>/.dev-team/logs/boot-error.log`；`append_ignored_env(home)` → 同目录 `ignored-env.log`，行文本 `ignored_env=DEV_TEAM_DATA_ROOT (release build ignores it)`（变量名引 `DATA_ROOT_ENV` 常量拼装，不另写字面量）。两者共用私有 `append_log(home, file, line)`：`create_dir_all` 父目录 → `OpenOptions::create(true).append(true)` → `writeln!("[{millis}] {line}")` → 全程 IO 失败静默 | 锚点恒 prod 根（env 已判无效不能当落点，而「日志不是数据」不违反 prod 不认 env）；`[millis]` 沿 `sdk/log.rs` 既有约定（校准 3）；与 `<数据根>/logs/engine.log` 分离（R-11） |
| D9 | 自检的探查与失败面 | `probe(data_root)` 私有：`WorkspaceStores::open` → `list_models()` → `list_workspaces()`，**首个失败即记 `error=` 并停步**（后续字段留空值），`global_db_open` 单独记录库是否打开成功；`Summary` 十键恒在（未知值渲染为空串）。退出前 `print!` + `io::stdout().flush()`，退出码交 `main` 的 `process::exit` | spec 十键键序固定、`error=` 追加、flush 必做（管道捕获下块缓冲会丢输出）；「首错即停」让 `error=` 唯一且可定位，避免拼装多条原因 |
| D10 | `selfcheck::run` 的形状 | `pub fn run(app: &tauri::App, data_root: &DataRoot) -> i32`：`home` 经 `app.path().home_dir()`（与 `main` 同源 API，失败退化为空串、不影响退出码）→ `probe` → `summary` → `render` → flush → 返回 `exit_code` | 保持 spec Module Contract 的签名；`&App` 的唯一用途就是取 `home=` 行，不触碰管理态、不建窗；`home_dir` 失败的臂在 `main` 已成功解析过同 API 后近乎不可达，故退化为空值而非新增错误面 |
| D11 | 单实例与 devtools 分流 | 单实例 `#[cfg(all(desktop, not(debug_assertions)))]`（回调体逐字不动）；devtools 保持 `#[cfg(debug_assertions)]`；`identifier` 零改动 | spec「dev 不注册单实例」；改 identifier 的代价（`--config` 变体 + 产物改名 + endpoint 覆盖 + tauri-build 重编译）已在 proposal D3 否掉 |
| D12 | IPC 线面形态 | `AppEnvironment { mode: String, data_root: String, source: String }`（`serde(rename_all = "camelCase")` + `specta::Type`），两个字段的线值分别来自 `env::mode()` 与 `RootSource::wire()`；**不**把 `RootSource` 做成 serde 重命名枚举 | 「同名同形、单点产出」由 `wire()` 一处表达；若给 `RootSource` 再加一套 serde rename，字面量就有两份映射（serde 属性 + `wire()`），恰是 spec 禁止的两处拼字符串。代价是 TS 侧生成 `string`，前端另以类型别名收窄（编译期，不产生第二份线值） |
| D13 | 前端穿透 | `useAppEnvironment()` 挂载一次 `commands.appEnvironment()`，成功且载荷合法才返回 `AppEnvironment`，reject / 载荷不合法 / 未返回一律 `null`（静默）；`useUpdater(mode)` 拆成「版本号 effect（挂载一次）」+「更新检查 effect（`mode === 'prod'` 才跑）」；`Dev Team` 旁在 `mode === 'dev'` 时渲染 `DEV` 徽标（`data-testid="dev-badge"`、`title = dataRoot`） | 两个 effect 拆分让 mode 从 `null` 变 `prod` 时**只**补一次 `check`，不重复拉版本号；`null` 不判定直接消灭「dev 构建装生产版」的坑；载荷合法性用 `typeof` 守卫收窄（`no-explicit-any` / `no-unsafe-type-assertion` 门禁下不许断言） |
| D14 | store 常量公开化 | `GLOBAL_DB_FILE_NAME` / `WORKSPACES_DIR_NAME` 由 `pub(crate)` 升 `pub` 并在 crate 根重导出；`workspace_db_file_name` / `workspace_db_path` 维持 `pub(crate)`；零行为改动 | 自检摘要与 IPC 需要库文件名 / 子树名，壳层复写字面量会造出第二份定义（R-12 已裁定）。`workspace_db_file_name` 无壳层消费者，不外扩 |
| D15 | target 隔离形态 | `server:build:dev` = `cross-env CARGO_TARGET_DIR=target/dev-build tauri build --debug --no-bundle`；即**相对 cargo 自身 cwd**（tauri CLI 以 `src-tauri` 为 cargo 工作目录 ⇒ 落 `src-tauri/target/dev-build`，在 `.gitignore` 的 `src-tauri/target` 覆盖内）。实现期以磁盘产物校准（AC-4）；若落位偏离（出现 `src-tauri/src-tauri/target/…` 或 `packages/desktop/target/…`），换成相对 pnpm 脚本 cwd 的 `CARGO_TARGET_DIR=src-tauri/target/dev-build` | tauri-cli 无 `--target-dir`，env 是唯一隔离手段；`CARGO_TARGET_DIR` 由 cargo 按自身 cwd 解析，而 tauri CLI 的 `cargo metadata` / `cargo build` 同 cwd ⇒ 两处一致。「隔离语义（≠ `target/debug`）」不随形态变化 |
| D16 | 出线面与版本 | `package.json` `version` `0.5.30` → `0.5.31`；`plugins/**` 零改动零 bump；`.github/workflows/desktop-release.yml`、`tauri.conf.json`、`capabilities/default.json` 零 diff | AC-17；本变更为用户可见变更（默认构建语义倒置 + 数据根分裂） |
| D17 | 明确不做（承认边界） | `--self-check --root <path>`、数据迁移 / 种子拷贝、identifier 变体、`pnpm accept` / WebDriver 验收链、日志轮转、dev 产物用 release 优化（R-1 退路本轮不实现）；junction / symlink / 8.3 短名的逻辑归一漏面按 spec 作为知情边界留痕 | proposal 范围裁决「本期不做」；R-1 的退路只需将来替换 `env::mode()` 单点，本设计的 §启动序列 / 数据根 / 前端穿透全不受影响 |

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 模式与数据根解析 | 常量单点（prod 目录名 / dev 子目录名 / env 变量名 / logs 子树名 / 两个日志文件名）；`mode()`；三分优先级；逻辑归一与大小写归一的比较键；五项校验；提示渲染；双写与留痕 | `src-tauri/src/env.rs`（新） | `std::path` / `std::ffi::OsStr` / `std::fs` / `std::io::Write` | 纯函数为主 + 两处 IO（append 日志、`is_file` 探测） |
| 自检 | 探库（真开 `WorkspaceStores` + `list_models` + `list_workspaces`）、摘要十键渲染、退出码映射、stdout flush、`--self-check` 参数与拒绝文案常量 | `src-tauri/src/selfcheck.rs`（新） | `crate::env`（`DataRoot` / `mode` / `RootSource::wire`）、`store::{WorkspaceStores, GLOBAL_DB_FILE_NAME, WORKSPACES_DIR_NAME}`、`tauri::App`（仅 `path().home_dir()`） | 纯函数渲染 + 一次性 IO |
| 启动装配 | 固定序列、装配期 `#[cfg]` 分流、数据根单点解析与 `manage`、失败双写 + `exit 2`、prod 侧自检拒绝、`setup_hook` 经 `State<DataRoot>` 取根 | `src-tauri/src/main.rs`（修改） | 上述两模块 + 既有五注册表 | Tauri 2 builder |
| IPC 穿透 | `AppEnvironment` DTO + 薄包装命令 + `*_inner` 微形态（`State<DataRoot>` → 线面） | `src-tauri/src/commands/env/mod.rs`（新） | `crate::env::{DataRoot, mode}`、`serde` / `specta` / `tauri::State` | 沿 `commands/config` + `commands/db` 模板 |
| 命令登记 | `pub mod env;` + `all_commands!` 增一行（同时被 `generate_handler` 与 specta builder 消费） | `src-tauri/src/commands/mod.rs`（修改） | — | 既有宏 |
| store 单点公开 | 两个常量升 `pub` 并在 crate 根重导出 | `crates/infra/store/src/store.rs` / `lib.rs`（修改） | — | 零行为 |
| 构建脚本 | `build` 倒置为 dev；`server:build:dev`（`--debug --no-bundle` + 隔离 target）；`build:prod` / `server:build` 保持生产语义 | `packages/desktop/package.json`（修改） | `cross-env` / `@tauri-apps/cli`（既有） | pnpm scripts |
| 前端 mode hook | 挂载取一次 `appEnvironment()`，合法性守卫 + 静默 `null` | `src/hooks/use-app-environment.ts`（新） | `commands`（生成物） | React hook |
| updater 判据 | 版本号 effect / 更新检查 effect 拆分，判据 `mode === 'prod'`，删 `import.meta.env.DEV` | `src/hooks/use-updater.ts`（修改） | `BuildMode` 类型 | React hook |
| 顶栏徽标 | 挂 `useAppEnvironment()`、mode 透传 `useUpdater`、`DEV` 徽标（`data-testid` + `title = dataRoot`） | `src/app.tsx`（修改） | 上述两 hook | React |
| 发版文档 | 本地构建拆两档、dev 数据根与 `--self-check` 用法、更新判据陈述同步 | `packages/desktop/RELEASE.md`（修改） | — | Markdown |

---

## 变更清单

> 口径：新增文件用仓库根相对全路径；修改文件与 proposal「变更范围 - 实现文件」同口径（`src-tauri/…` / `src/…` / `crates/…` 省略 `packages/desktop/` 前缀）。

### 新增文件

| 文件路径 | 说明 | 关联条目 |
|----------|------|----------|
| `packages/desktop/src-tauri/src/env.rs` | 模式与数据根单点：四个既有约定常量（`PROD_DIR` / `DEV_SUBDIR` / `DATA_ROOT_ENV` / `LOGS_DIR_NAME`）+ 两个日志文件名常量；`DataRoot` / `RootSource` / `RootError`；`mode()` / `prod_root()` / `resolve_dev_root()` / `resolve_root()`；私有 `normalize()` / `compare_key()` / `prod_dir()` / `dev_default_dir()` / `append_log()` / `now_millis()`；`render_root_error()` / `append_boot_error()` / `append_ignored_env()`；末行 `#[cfg(test)] mod env_test;` | 公共函数表 `env::*` 九行；类型表 `DataRoot` / `RootSource` / `RootError` 三行；D1 / D4 / D5 / D6 / D7 / D8 |
| `packages/desktop/src-tauri/src/selfcheck.rs` | 自检契约：`SELF_CHECK_ARG` / `PROD_REJECT_MESSAGE` 常量；`Probe` / `Summary`；`render()` / `exit_code()` / `run()`；私有 `probe()` / `summary_of()`；末行 `#[cfg(test)] mod selfcheck_test;` | 公共函数表 `selfcheck::*` 三行 + 两常量行；类型表 `Probe` / `Summary` 两行；D9 / D10 |
| `packages/desktop/src-tauri/src/commands/env/mod.rs` | 第十一轨命令：`AppEnvironment` DTO（camelCase 线面）+ `app_environment`（`State<'_, DataRoot>` 薄包装）+ `app_environment_inner(&DataRoot)` 微形态；末行 `#[cfg(test)] mod mod_test;` | 公共函数表 `app_environment` / `app_environment_inner` 两行；类型表 `AppEnvironment` 行；路由表 IPC 行；D12 |
| `packages/desktop/src/hooks/use-app-environment.ts` | 前端 mode 单源：`BuildMode` / `DataRootSource` / `AppEnvironment` 类型别名与接口、`useAppEnvironment()`、私有守卫 `isRecord` / `toEnvironment` | 公共函数表 `useAppEnvironment` 行；类型表三个前端类型行；D13 |

> 测试模块文件（`src-tauri/src/env_test.rs` / `selfcheck_test.rs` / `commands/env/mod_test.rs`、`src/hooks/use-app-environment.test.ts`）的**用例**归测试相位；实现相位只需建 `src-tauri` 三个仅含头注释的空模块（否则 `cargo fmt` / `clippy --all-targets` 因 `mod` 解析失败而报错），零 `#[test]`。

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `src-tauri/src/lib.rs` | 末尾追加 `pub mod env;` / `pub mod selfcheck;`（两行，按字母序置于 `pub mod commands;` 之后） | 模块挂载点；测试模块声明在各模块文件内 |
| `src-tauri/src/main.rs` | ① 删 `use std::path::PathBuf;`（改由 `DataRoot` 承载）；② 删 `const DATA_DIR` 与 `const LOGS_DIR_NAME`，改 `use dev_team::env;`（`main` 只用到 `env::{LOGS_DIR_NAME, DATA_ROOT_ENV, DataRoot}`；`env::PROD_DIR` 留在 `env` 内部，`main` 不再拼任何数据根成分）；③ `main` 顶部加 `let want_self_check = std::env::args().skip(1).any(\|arg\| arg == selfcheck::SELF_CHECK_ARG);` 与 prod 侧拒绝（D3 形态）；④ 单实例属性 `#[cfg(desktop)]` → `#[cfg(all(desktop, not(debug_assertions)))]`（回调体逐字不动）；⑤ 内联 `setup` 闭包抽成 `fn setup_hook(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>>`：`let data_root = app.state::<env::DataRoot>().path.clone();`，其余（`WorkspaceStores::open` / `init_engine_log(&data_root.join(env::LOGS_DIR_NAME))` / panic 钩子 / 五 `manage`）逐字迁移，`home_dir` 与 `DATA_DIR` 拼接删除；⑥ `.setup(setup_hook)` 之后改为 `builder.build(tauri::generate_context!("tauri.conf.json")).expect("desktop 应用构建失败")` → `let home = app.path().home_dir().expect("home 目录解析失败");` → 数据根解析（失败即 `eprintln!` + `env::append_boot_error` + `exit(2)`）→ prod 侧 `var_os(env::DATA_ROOT_ENV).is_some()` 则 `env::append_ignored_env(&home)` → `app.manage(data_root.clone())` → dev 侧自检分支 → `app.run(\|_, _\| {}).expect("desktop 应用启动失败")` | AC-15 单点；顺序与失败处置见 D2 / D9；builder 链尾由 `.run(...)` 换成 `build()`，`invoke_handler(all_commands!(generate_handler))` 仍在链上（`generate_handler` 与 specta 同源登记不变） |
| `src-tauri/src/commands/mod.rs` | `pub mod exec;` 与 `pub mod explores;` 之间插 `pub mod env;`；`all_commands!` 末尾（`config::workspace_config` 之后）追加 `$crate::commands::env::app_environment,` | 单点登记面；`generate_handler` 与 specta 同时生效 |
| `crates/infra/store/src/store.rs` | `pub(crate) const GLOBAL_DB_FILE_NAME` → `pub const`；`pub(crate) const WORKSPACES_DIR_NAME` → `pub const`（两行，doc 注释保留并补一句「壳层自检 / IPC 经 crate 根重导出消费，MUST NOT 复写字面量」） | D14；零行为改动 |
| `crates/infra/store/src/lib.rs` | `pub use store::{…, EXPLORE_PLACEHOLDER_PREFIX, GLOBAL_DB_FILE_NAME, WORKSPACES_DIR_NAME};`（追加两常量） | D14 |
| `packages/desktop/package.json` | `version` `0.5.30` → `0.5.31`；`build` → `pnpm run bindings:export && pnpm run server:build:dev`；新增 `build:prod` = `pnpm run bindings:export && pnpm run server:build`；新增 `server:build:dev` = `cross-env CARGO_TARGET_DIR=target/dev-build tauri build --debug --no-bundle`；`server:build` 逐字保留 | D15 / D16；`dev` / `client:dev` / `client:build` / `bindings:*` / `check` / `test` / `mutate` / `tauri` 零改动（`bindings:export` 不受 `CARGO_TARGET_DIR` 影响，因其 env 作用域只在 `server:build:dev` 进程内） |
| `src/hooks/use-updater.ts` | `useUpdater(mode: BuildMode \| null)`；`useStartupCheck` 拆成 `useStartupVersion`（挂载一次拉版本号）与 `useStartupCheck(setAvailable, mode)`（`if (mode !== 'prod') return;` 才 `check()`，失败仍静默）；删 `if (!import.meta.env.DEV)`；`start` / `performUpdate` / `briefOf` 零改动 | D13；AC-14 |
| `src/app.tsx` | `const environment = useAppEnvironment(); const update = useUpdater(environment?.mode ?? null);`；`ShellHeader` 增 `environment` 入参并在 `<strong>Dev Team</strong>` 之后渲染 `mode === 'dev'` 时的 `DEV` 徽标（`data-testid="dev-badge"`、`title={environment.dataRoot}`）；版本号显示链路零改动（不加 `-dev`） | D13；AC-14 |
| `src/types/generated/bindings.ts` | `bindings:export` 重导出：新增 `appEnvironment` 包装与 `AppEnvironment` 类型（`{ mode: string; dataRoot: string; source: string }`） | 生成物，禁手改；`bindings:check` 必须零 diff |
| `packages/desktop/RELEASE.md` | 「本地构建须知」（`:46-59`）拆两档：`build` 零配置产 dev 二进制（`--debug --no-bundle`，免私钥，产物 `src-tauri/target/dev-build/debug/dev-team.exe` 可双击）；私钥前置只约束 `build:prod`；补 dev 数据根落 `<home>/.dev-team/dev`、`--self-check` 用法与「验收须用独立 `DEV_TEAM_DATA_ROOT`」红线；「约束与已知行为」`:116` 的 `import.meta.env.DEV` 陈述改为「dev 构建按 mode 判据不检查更新」 | AC-13；与 D13 同批改，避免文档与实现相反 |

### 公共函数 / API

> 口径：仅列模块级导出项。`env` / `selfcheck` 为 `pub mod`（壳层 crate 内可见即可满足测试直调）；两个 store 常量列为常量行（非函数，但属跨 crate API 面，单列以免遗漏）。私有函数（`normalize` / `compare_key` / `prod_dir` / `dev_default_dir` / `append_log` / `now_millis` / `probe` / `summary_of` / `isRecord` / `toEnvironment`）不入表。

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|-----------|------|------|
| `PROD_DIR` | `src-tauri/src/env.rs` | 新增（迁自 `main.rs` 的 `DATA_DIR`） | `pub const PROD_DIR: &str = ".dev-team"` | prod 目录名唯一字面量；壳层其他位置零复写（AC-15） |
| `DEV_SUBDIR` | 同上 | 新增 | `pub const DEV_SUBDIR: &str = "dev"` | dev 子根名（嵌套落位 `<home>/.dev-team/dev`） |
| `DATA_ROOT_ENV` | 同上 | 新增 | `pub const DATA_ROOT_ENV: &str = "DEV_TEAM_DATA_ROOT"` | 变量名单点（提示文案 / 留痕文本 / `var_os` 探测共用） |
| `LOGS_DIR_NAME` | 同上 | 新增（迁自 `main.rs`） | `pub const LOGS_DIR_NAME: &str = "logs"` | 数据根下日志子树名（引擎日志与启动失败日志共用目录、分文件） |
| `BOOT_ERROR_LOG_NAME` | 同上 | 新增 | `pub const BOOT_ERROR_LOG_NAME: &str = "boot-error.log"` | 启动失败日志文件名 |
| `IGNORED_ENV_LOG_NAME` | 同上 | 新增 | `pub const IGNORED_ENV_LOG_NAME: &str = "ignored-env.log"` | prod 侧 env 留痕文件名 |
| `env::mode` | 同上 | 新增 | `pub fn mode() -> &'static str` | Rust 侧唯一模式判定：`cfg!(debug_assertions)` → `"dev"` / `"prod"`；线值与 IPC / 自检同名同形 |
| `env::prod_root` | 同上 | 新增 | `pub fn prod_root(home: &Path) -> DataRoot` | `source = Prod`；**签名不接收 env** ⇒ prod 不认 env 由结构保证 |
| `env::resolve_dev_root` | 同上 | 新增 | `pub fn resolve_dev_root(home: &Path, env: Option<&OsStr>) -> Result<DataRoot, RootError>` | 纯函数：`None` → `<home>/.dev-team/dev`（`DevDefault`）；`Some` → 五项校验 → 归一路径（`DevEnv`）或 `Err` |
| `env::resolve_root` | 同上 | 新增 | `pub fn resolve_root(home: &Path, is_dev: bool) -> Result<DataRoot, RootError>` | 薄分派：`!is_dev` 早退 `prod_root`；dev 分支读 `std::env::var_os(DATA_ROOT_ENV)` 后交 `resolve_dev_root` |
| `env::render_root_error` | 同上 | 新增 | `pub fn render_root_error(err: &RootError, home: &Path) -> String` | 自然语言提示（三要素齐备）；GUI 启动与 `--self-check` 共用同一文案 |
| `env::append_boot_error` | 同上 | 新增 | `pub fn append_boot_error(home: &Path, message: &str)` | 追加到 `<home>/.dev-team/logs/boot-error.log`；自建父目录、`[millis]` 前缀、IO 失败静默 |
| `env::append_ignored_env` | 同上 | 新增 | `pub fn append_ignored_env(home: &Path)` | 追加 `<home>/.dev-team/logs/ignored-env.log`，行文本 `ignored_env=<DATA_ROOT_ENV> (release build ignores it)` |
| `RootSource::wire` | 同上 | 新增 | `pub fn wire(self) -> &'static str` | 线值单点：`Prod → "prod"` / `DevDefault → "dev-default"` / `DevEnv → "env"`；自检摘要与 IPC 共用 |
| `selfcheck::SELF_CHECK_ARG` | `src-tauri/src/selfcheck.rs` | 新增 | `pub const SELF_CHECK_ARG: &str = "--self-check"` | 参数名单点（`main` 的 args 判定引用） |
| `selfcheck::PROD_REJECT_MESSAGE` | 同上 | 新增 | `pub const PROD_REJECT_MESSAGE: &str = "--self-check 仅在 dev 构建可用（当前为生产构建）。"` | prod 侧拒绝文案单点（spec 逐字口径） |
| `selfcheck::render` | 同上 | 新增 | `pub fn render(summary: &Summary) -> String` | 纯函数：十键固定键序 + 可选 `error=` 行，末尾单换行；未知值渲染为空串 |
| `selfcheck::exit_code` | 同上 | 新增 | `pub fn exit_code(probe: &Probe) -> i32` | `error.is_none()` → 0，否则 1（2 由 `main` 产出：env 非法 / prod 拒自检） |
| `selfcheck::run` | 同上 | 新增 | `pub fn run(app: &tauri::App, data_root: &DataRoot) -> i32` | 探库 → 摘要 → `print!` → `stdout().flush()` → 返回退出码（`&App` 仅用于 `home=` 行） |
| `app_environment` | `src-tauri/src/commands/env/mod.rs` | 新增 | `pub fn app_environment(data_root: State<'_, DataRoot>) -> AppEnvironment` | `#[tauri::command]` + `#[specta::specta]`；无 IO、无 `Result` 错误面（管理态恒在） |
| `app_environment_inner` | 同上 | 新增 | `pub fn app_environment_inner(data_root: &DataRoot) -> AppEnvironment` | 领域平移纯函数（离运行时可测，沿 `workspace_config_inner` 先例） |
| `GLOBAL_DB_FILE_NAME` | `crates/infra/store/src/store.rs`（+ `lib.rs` 重导出） | 修改（`pub(crate)` → `pub`） | `pub const GLOBAL_DB_FILE_NAME: &str = "desktop-global.redb"` | 自检摘要 `global_db=` 的路径成分（壳层零字面量） |
| `WORKSPACES_DIR_NAME` | 同上 | 修改（`pub(crate)` → `pub`） | `pub const WORKSPACES_DIR_NAME: &str = "workspaces"` | 自检摘要 `workspaces_dir=` 的路径成分 |
| `useAppEnvironment` | `src/hooks/use-app-environment.ts` | 新增 | `export function useAppEnvironment(): AppEnvironment \| null` | 挂载一次 `commands.appEnvironment()`；成功且载荷合法 → `AppEnvironment`；reject / 载荷不合法 / 未返回 → `null`（静默，无错误态） |
| `useUpdater` | `src/hooks/use-updater.ts` | 修改 | `export function useUpdater(mode: BuildMode \| null): UpdateState` | 启动检查判据 `mode === 'prod'`；`null` 不判定；下载 / 安装链路零变化 |
| `all_commands!` 命令清单 | `src-tauri/src/commands/mod.rs` | 修改 | `macro_rules! all_commands`（末尾追加 `commands::env::app_environment`） | `generate_handler` 与 specta builder 同源登记 |

### 类型定义

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|--------|----------|-----------|------|
| `DataRoot` | `src-tauri/src/env.rs` | 新增 | `pub struct DataRoot { pub path: PathBuf, pub source: RootSource }`（`Debug, Clone, PartialEq, Eq`）；作 Tauri 管理态单值承载，`setup_hook` 与 `selfcheck::run` 共用（AC-15） |
| `RootSource` | 同上 | 新增 | `pub enum RootSource { Prod, DevDefault, DevEnv }`（`Debug, Clone, Copy, PartialEq, Eq`）；线值经 `wire()` 单点产出 |
| `RootError` | 同上 | 新增 | `pub enum RootError { Blank { raw: String }, Relative { raw: String }, EqualsProd { raw: String }, AncestorOfProd { raw: String }, NotDirectory { raw: String } }`（`Debug, Clone, PartialEq, Eq`）；`raw` = 收到的原值（提示三要素之二由类型携带） |
| `Probe` | `src-tauri/src/selfcheck.rs` | 新增 | `pub struct Probe { pub global_db_open: bool, pub workspace_count: Option<u64>, pub models: Option<Vec<String>>, pub workspaces_dir_exists: bool, pub error: Option<String> }`（`Debug, Clone`）；`exit_code` 的入参，未知字段以 `None` 表达 |
| `Summary` | 同上 | 新增 | `pub struct Summary { pub mode: &'static str, pub home: PathBuf, pub data_root: PathBuf, pub source: &'static str, pub global_db: PathBuf, pub global_db_open: bool, pub workspaces_dir: PathBuf, pub workspaces_dir_exists: bool, pub workspace_count: Option<u64>, pub models: Option<Vec<String>>, pub error: Option<String> }`（`Debug, Clone`）；字段序即渲染键序 |
| `AppEnvironment` | `src-tauri/src/commands/env/mod.rs` | 新增 | `pub struct AppEnvironment { pub mode: String, pub data_root: String, pub source: String }`（`Debug, Clone, Serialize, Type`，`#[serde(rename_all = "camelCase")]`）；线形 `{ mode, dataRoot, source }` |
| `BuildMode` | `src/hooks/use-app-environment.ts` | 新增 | `export type BuildMode = 'dev' \| 'prod'`；编译期收窄用（线值由后端 `mode()` 单点产出） |
| `DataRootSource` | 同上 | 新增 | `export type DataRootSource = 'prod' \| 'dev-default' \| 'env'`；同 `RootSource::wire()` 值域 |
| `AppEnvironment`（前端） | 同上 | 新增 | `export interface AppEnvironment { mode: BuildMode; dataRoot: string; source: DataRootSource }`；`useAppEnvironment` 的返回值形态（与生成物同名，导入生成物时以 `AppEnvironmentPayload` 别名避让） |
| `AppEnvironment`（生成物） | `src/types/generated/bindings.ts` | 修改（重导出） | 生成 `{ mode: string; dataRoot: string; source: string }` + `commands.appEnvironment()` 包装（`bindings:check` 零 diff） |

### 配置

| 配置键 | 所在文件 | 新增/修改 | 值类型 | 默认值 | 说明 |
|--------|----------|-----------|--------|--------|------|
| `scripts.build` | `packages/desktop/package.json` | 修改 | `string` | `pnpm run bindings:export && pnpm run server:build:dev` | 默认值倒置为 **dev**（AC-2） |
| `scripts.build:prod` | 同上 | 新增 | `string` | `pnpm run bindings:export && pnpm run server:build` | 生产构建显式入口（AC-3）；CI 不消费（走裸 `tauri build`） |
| `scripts.server:build:dev` | 同上 | 新增 | `string` | `cross-env CARGO_TARGET_DIR=target/dev-build tauri build --debug --no-bundle` | dev 构建：免私钥、不打包、隔离 target（D15，AC-2 / AC-4） |
| `scripts.server:build` | 同上 | 保留（零改动） | `string` | `cross-env TAURI_SIGNING_PRIVATE_KEY_PASSWORD= tauri build` | 生产侧签名前置不削弱（AC-3） |
| `version` | 同上 | 修改 | `string` | `0.5.30` → `0.5.31` | AC-17 |
| `DEV_TEAM_DATA_ROOT` | 定义于 `src-tauri/src/env.rs`（`DATA_ROOT_ENV`），无配置文件 | 新增（运行时环境变量） | `string`（绝对路径） | 未设 | 仅 dev 构建消费；五项校验；prod 构建只探测留痕（AC-5 / AC-6 / AC-9） |
| `CARGO_TARGET_DIR` | `packages/desktop/package.json`（`server:build:dev` 内联，进程级 env） | 新增 | `string`（相对 cargo cwd） | `target/dev-build` | 仅作用于 `server:build:dev` 进程；`bindings:export` / `tauri dev` 零影响（AC-4） |

> 无新增 `openspec/config.json` 配置项；`tauri.conf.json` / `capabilities/default.json` 零改动。

---

## 数据模型

持久化面**零结构改动**（不新增 / 修改任何 native_db 模型、无索引变更、无迁移）。本变更改的是**根的位置**与**两个新日志文件**：

```
home ─┬─ .dev-team（= env::PROD_DIR，prod 根）
      │    ├── desktop-global.redb        ← store::GLOBAL_DB_FILE_NAME（prod 模式）
      │    ├── workspaces/                ← store::WORKSPACES_DIR_NAME
      │    ├── logs/
      │    │    ├── engine.log            ← agent_runtime::init_engine_log（随根走）
      │    │    ├── boot-error.log        ← 启动失败双写（恒锚 prod 根，与模式无关）
      │    │    └── ignored-env.log       ← prod 侧 env 留痕（恒锚 prod 根）
      │    └── dev/（= env::DEV_SUBDIR，dev 根）
      │         ├── desktop-global.redb   ← 同名文件、不同父目录
      │         ├── workspaces/
      │         └── logs/engine.log
      └─ （env 覆盖时的数据根可落在任意独立目录，结构与上面 dev 根同形）
```

| 面 | 载体 | 语义 / 生命周期 |
|----|------|-----------------|
| 数据根 | `DataRoot { path, source }`（Tauri 管理态） | `main` 单点解析一次，`manage` 后 `setup_hook`（`app.state::<DataRoot>()`）与 `selfcheck::run` 共用同一个值；进程生命周期，重启重算 |
| 两库 | `<数据根>/desktop-global.redb`、`<数据根>/workspaces/<identity>.redb` | 结构零改动；根分裂使同一 workspace 在两模式下为**同名文件、不同父目录**，redb 文件锁与单实例 mutex 冲突随之解除（store 零感知） |
| 启动失败日志 | `<prod 根>/logs/boot-error.log` | append-only，行格式 `[<unix 毫秒>] <提示全文>`；写前自建 `logs/`；无轮转（与 `engine.log` 现状同口径，R-11 观察项）；恒锚 prod 根（日志不是数据） |
| env 留痕日志 | `<prod 根>/logs/ignored-env.log` | append-only，行格式 `[<unix 毫秒>] ignored_env=DEV_TEAM_DATA_ROOT (release build ignores it)`；只在 prod 构建且 `var_os().is_some()` 时产生；不阻断启动 |
| 引擎日志 | `<数据根>/logs/engine.log` | 既有面零改动（随根分裂）；与上述两个文件**分文件**，互不混写 |
| 自检产出 | stdout 十行 `key=value`（可选第 11 行 `error=`） | 一次性、不落盘；`workspace_count` / `models` 在库不可读时渲染为空值 |

关系面：`RootSource` 与线值 `prod` / `dev-default` / `env` 是**一对一映射**（`wire()` 单点）；`Summary` 与 `Probe` 是「渲染视图 ← 探查事实」的投影（`Summary` 的 `workspace_count` / `models` 直接搬运 `Probe` 的 `Option`）；`AppEnvironment` 与 `DataRoot` 是**字段平移**（`path` → `dataRoot` 字符串、`source` → `wire()`、`mode` → `env::mode()`），无第二处加工。无外键、无跨表引用。

---

## 路由 / API 设计

### IPC 命令

| method | 名称 | 说明 | 输入 | 输出 | auth |
|--------|------|------|------|------|------|
| Tauri command（`invoke`） | `app_environment` | 前端 mode 单源；薄包装 + `*_inner` 微形态，从管理态取 `DataRoot` 平移出线面；无 IO、无 `Result` 错误面 | 无 | `AppEnvironment { mode: "dev" \| "prod", dataRoot: string, source: "prod" \| "dev-default" \| "env" }` | 无（应用内命令不进 capability；`capabilities/default.json` 零改动） |

### CLI 入参（自检契约）

| 形态 | 说明 | 输出 | 退出码 |
|------|------|------|--------|
| `<dev 产物> --self-check` | 零窗口自检；`build()` 之后、`run()` 之前执行并 `exit` | stdout 十行（键序固定）：`mode` / `home` / `data_root` / `data_root_source` / `global_db` / `global_db_open` / `workspaces_dir` / `workspaces_dir_exists` / `workspace_count` / `models`；失败追加 `error=<原因>` | 0 全通；1 数据根 / 库打开失败（附 `error=`）；2 env 非法（与 GUI 同文案双写） |
| `<prod 产物> --self-check` | 显式拒绝（分支常驻产物，D3） | stderr `--self-check 仅在 dev 构建可用（当前为生产构建）。` | 2（立即返回，不挂住） |
| `<dev 产物> --self-check --root <path>` | **本期不支持**（与 env 语义冲突）；`--root` 被当作未知参数忽略，仍按 env / 默认根自检 | 同上 | 同上 |

---

## 依赖

### 运行时依赖

| crate / 特性 | 变更 | 用途 |
|--------------|------|------|
| `dev-team` → `store` | 不变（既有边） | `WorkspaceStores` / `GLOBAL_DB_FILE_NAME` / `WORKSPACES_DIR_NAME`（后者为本次扩为 `pub` 的两常量，零新依赖边） |
| `dev-team` → `tauri` | 不变（既有边） | `App` / `Manager` / `State` / `generate_context!` / `generate_handler` |
| `time` / `dirs` | **不新增** | 时间戳沿 `[unix 毫秒]` 约定（`std::time`）；home 经 `app.path().home_dir()`（校准 3 / D2） |
| `tauri-plugin-single-instance` | 不变（依赖边与 init 调用体均不变） | 仅在 `not(debug_assertions)` 下注册（D11） |
| `tauri-plugin-devtools` / `-dialog` / `-opener` / `-updater` | 不变 | 装配链位置与语义零变化 |
| 新增第三方 crate | **无** | 归一用 `std::path`，日志用 `std::fs` + `std::io::Write`，大小写归一用 `String::to_lowercase` |

### 构建 / 测试依赖

| 项 | 变更 | 用途 |
|----|------|------|
| `@tauri-apps/cli`（2.12.4） | 不变 | `--debug` / `--no-bundle` 为既有 CLI 选项（D15） |
| `cross-env` | 不变（既有 devDependency） | `server:build:dev` 注入 `CARGO_TARGET_DIR`；`server:build` 注入空密码 env |
| `packages/desktop/.gitignore` | 不变 | 已覆盖 `src-tauri/target` ⇒ 隔离目录 `src-tauri/target/dev-build` 天然被忽略（AC-4） |
| `cargo` / `rustc` | 不变 | `CARGO_TARGET_DIR` 为标准环境变量，`cargo metadata` 与 `cargo build` 同 cwd 解析 |
| 新增 dev-dependency | **无** | 环境 / 自检用例走既有 `tempfile` + 纯函数（用例编写归测试相位） |

---

## 验收标准回应

| AC | 设计落点 |
|----|----------|
| AC-1 | 判别源单点：`env::mode()`（D1）+ `cfg!`/`#[cfg]` 的两处装配期分流（D11、D3）；`use-updater.ts` 删除 `import.meta.env.DEV`（修改文件表）；`vite.config.ts` 零改动（不在实现文件表，无 define 注入） |
| AC-2 | `scripts.build` → `server:build:dev`（配置表）+ `--debug --no-bundle`（D15）；运行期 `mode=dev` 由 `env::mode()` 保证（AC-11 的 self-check 实跑即证） |
| AC-3 | `build:prod` → `server:build` 逐字保留（配置表）；签名前置语义零触碰（`tauri.conf.json` 零改动） |
| AC-4 | `CARGO_TARGET_DIR=target/dev-build` + 校准判据（D15）；`bindings:export` 独立编译态不受影响（配置表说明）；`.gitignore` 覆盖 `src-tauri/target`（依赖表） |
| AC-5 | 三分优先级：`prod_root` / `resolve_dev_root` 双入口 + `resolve_root` 薄分派（D4、公共函数表三行）；`source` 值域由 `RootSource::wire()` 单点（D12） |
| AC-6 | 五项校验顺序与 `RootError` 变体（D6）；提示三要素由 `render_root_error` 统一产出（D7）；硬失败 `exit 2` 在 `main`（D2） |
| AC-7 | 逻辑归一 `components()` + 「无可弹段则保留 `..`」（D5）；`compare_key` 的 Windows 小写（D5）；源码零 `canonicalize`（tasks 自查项） |
| AC-8 | `append_boot_error` 锚 prod 根、自建父目录、`[millis]` 前缀、append-only（D8）；与引擎日志分文件（数据模型表） |
| AC-9 | prod 侧只 `var_os().is_some()` 探测 + `append_ignored_env`（D8 + `main` 修改项 ⑥）；调用点带 `#[cfg(not(debug_assertions))]`，dev 构建零留痕 |
| AC-10 | 单实例属性改为 `all(desktop, not(debug_assertions))`（D11 + `main` 修改项 ④）；redb 锁由根分裂解除（数据模型「两库」行）；`identifier` 零改动（只读红线） |
| AC-11 | `selfcheck::run` 的插点（`build()` 后 / `run()` 前，D2）、十键键序 + `error=`（D9）、flush（D9）、退出码映射（`exit_code` + `main` 的 2） |
| AC-12 | `cfg!` 常量短路的 prod 拒绝分支（D3）；文案常量 `PROD_REJECT_MESSAGE`（公共函数表）；`--self-check` 分支不进 `#[cfg]` |
| AC-13 | RELEASE.md 两档改写 + 独立 `DEV_TEAM_DATA_ROOT` 红线（修改文件表 RELEASE.md 行）；不注册单实例使双进程并存（D11） |
| AC-14 | `useAppEnvironment` 三态（`dev` / `prod` / `null`）与 UI 消费（D13）；updater 拆双 effect 保证「就绪后只补一次检查」；徽标 `data-testid="dev-badge"` + `title = dataRoot`（前置类型 `mode: string` 的收窄由守卫完成） |
| AC-15 | `main` 单点解析 + `manage(DataRoot)` + `setup_hook` 经 `State` 取根（D2 + `main` 修改项 ⑤/⑥）；`.dev-team` 字面量只存在于 `env` 模块（`PROD_DIR` 常量及其派生文案）；数据根失败在 `main` 处置（不进 `setup`） |
| AC-16 | 无新增依赖、无 schema 扩展（依赖表）；`bindings:check` 零 diff（生成物由 `bindings:export` 产出）；`server:check` = `cargo fmt` + `clippy --all-targets` 要求三个测试模块文件先存在（tasks 阶段二 / 三 / 五的空骨架项） |
| AC-17 | `version` 0.5.31（配置表）+ `plugins/**` 零改动 + workflow / `tauri.conf.json` / `capabilities` 零 diff（D16 与只读红线） |

---

## 待决问题

1. **隔离 target 目录的实际基座（R-2）**。主形态 `CARGO_TARGET_DIR=target/dev-build` 依赖「tauri CLI 以 `src-tauri` 为 cargo cwd」这一事实（CLI 内 `cargo metadata` 的 `target_directory` 与 `cargo build` 同 cwd）。实现期须以磁盘产物校准；若落位偏离，退路为相对 pnpm 脚本 cwd 的 `src-tauri/target/dev-build`（D15 已给判据）。隔离语义不随形态变化。
2. **macOS 上 `build()` 后 `exit` 的副作用（R-3）**。自检在事件循环创建后立即退出，mac 侧可能出现 `NSApplication` 清理噪音污染 stdout/stderr。本期只在 Windows 出验收结论；若实测有噪音，退路是「`run` 返回退出码 → `main` 正常返回」，退出码语义不变（D10 的形状不用改）。
3. **`models=` 的取值形态**。当前按 `list_models()` 的注册表短名出线（`workspace,agent_provider,agent_instance`，校准 1），与 proposal 示例的 Rust 类型名不同。若产品要求类型名形态，需在 store 侧新增一层「注册表名 → 类型名」映射（扩 store API 面），超出本变更范围。
4. **`DEV` 徽标的可见范围**。徽标落在 `ShellHeader`（`app.tsx`），而该顶栏只在选中 workspace 后渲染（欢迎屏是独立全屏视图）。因此「无 workspace 时看不到徽标」。扩到欢迎屏需改 `views/welcome-view.tsx`（proposal 明列不改），本期不做，作为知情边界。
5. **自检的建库副作用**。自检对不存在的数据根会建出 `<数据根>/desktop-global.redb`（校准 4）。这与 GUI 首次启动同源、可接受；若将来要求「纯只读探查」，需在 store 侧新增不建库的探测入口（本变更不做）。
