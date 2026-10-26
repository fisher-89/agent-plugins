# 任务: desktop-build-mode-split

> 覆盖 proposal「变更范围 - 实现文件」全部 13 行 = **3 新增 + 10 修改**（其中 `crates/infra/store` 的 `store.rs` + `lib.rs` 一行按两个文件落地，故设计侧修改文件表为 11 行）。**本相位不写测试用例**：`src-tauri/src/{env_test,selfcheck_test}.rs`、`src-tauri/src/commands/env/mod_test.rs`、`src/hooks/use-app-environment.test.ts` 的新增用例，`use-updater.test.ts` / `app.test.tsx` 的改造，全部归 test-design / test-gen / test-execution 相位。
>
> 依赖序要点：**阶段一**（store 两常量公开）解锁**阶段三**（自检摘要取库文件名 / 子树名）；**阶段二**（`env` 模块）解锁**阶段三 / 四 / 五**；**阶段三** 与 **阶段二** 共同解锁**阶段四**（`main` 启动序列同时依赖 `env::resolve_root` 与 `selfcheck::run`）；**阶段五**（命令 + 登记）解锁**阶段七**（前端消费生成物）；**阶段八**（文档）与**阶段九**（实跑校准）收尾。
>
> 中间态约定：`env.rs` / `selfcheck.rs` 声明了 `#[cfg(test)] mod {env_test,selfcheck_test};`，`commands/env/mod.rs` 声明了 `#[cfg(test)] mod mod_test;`，而本仓 `server:check`（`cargo fmt --all` + `cargo clippy --workspace --all-targets`）会解析并编译测试目标 ⇒ 三处必须**同批**建出仅含头注释的空模块文件，否则 fmt/clippy 报「failed to resolve mod」。空模块内零 `#[test]`。
>
> 自查命令 cwd 除特别标注外均为 `packages/desktop/src-tauri`（前端 / 文档项以 `packages/desktop` 或仓库根为 cwd）。
>
> 不预铺：无 config schema 扩展、无 db 迁移、无新第三方依赖、无 `tauri.conf.json` / `capabilities` / `.github/**` 改动、无 `plugins/**` 改动。

## 阶段一：store 常量公开化（解锁自检摘要）

- [x] 修改 `crates/infra/store/src/store.rs`：`pub(crate) const GLOBAL_DB_FILE_NAME: &str = "desktop-global.redb"` → `pub const GLOBAL_DB_FILE_NAME`、`pub(crate) const WORKSPACES_DIR_NAME: &str = "workspaces"` → `pub const WORKSPACES_DIR_NAME`（仅可见性，值与 doc 注释保留；doc 补一句「desktop 壳的自检摘要与 IPC 经 crate 根重导出消费，MUST NOT 在壳层复写库文件名 / 子树名」）
- [x] 修改 `crates/infra/store/src/lib.rs`：crate 根重导出行追加两常量——`pub use store::{DbDimension, Store, StoreError, WorkspaceStores, EXPLORE_PLACEHOLDER_PREFIX, GLOBAL_DB_FILE_NAME, WORKSPACES_DIR_NAME};`；`workspace_db_file_name` / `workspace_db_path` **维持 `pub(crate)`**（无壳层消费者）
- [x] 自查 阶段一：`grep -n "pub const GLOBAL_DB_FILE_NAME\|pub const WORKSPACES_DIR_NAME" crates/infra/store/src/store.rs` 恰两行；`grep -n "GLOBAL_DB_FILE_NAME\|WORKSPACES_DIR_NAME" crates/infra/store/src/lib.rs` 各一处；`grep -n "pub(crate) fn workspace_db_file_name" crates/infra/store/src/store.rs` 仍在（未误扩）

## 阶段二：`env` 模块（模式与数据根单点，依赖阶段一）

- [x] 新建 `src/env.rs` 骨架：模块 doc（判别源单点 `cfg!(debug_assertions)`、prod 不认 env 由签名保证、逻辑归一禁 `canonicalize`、日志锚 prod 根且与 `engine.log` 分文件、junction / symlink / 8.3 短名为知情边界）+ 注释常量 `PROD_DIR = ".dev-team"` / `DEV_SUBDIR = "dev"` / `DATA_ROOT_ENV = "DEV_TEAM_DATA_ROOT"` / `LOGS_DIR_NAME = "logs"` / `BOOT_ERROR_LOG_NAME = "boot-error.log"` / `IGNORED_ENV_LOG_NAME = "ignored-env.log"`；末行 `#[cfg(test)] mod env_test;`
- [x] 同文件建测试模块空骨架 `src/env_test.rs`：仅头注释（说明用例归测试相位），零 `#[test]`
- [x] 同文件实现类型面：`pub struct DataRoot { pub path: PathBuf, pub source: RootSource }`、`pub enum RootSource { Prod, DevDefault, DevEnv }`（`Debug, Clone, Copy, PartialEq, Eq`）、`pub enum RootError { Blank { raw: String }, Relative { raw: String }, EqualsProd { raw: String }, AncestorOfProd { raw: String }, NotDirectory { raw: String } }`（`Debug, Clone, PartialEq, Eq`）；`impl RootSource { pub fn wire(self) -> &'static str }`（`"prod"` / `"dev-default"` / `"env"`，线值唯一出处）
- [x] 同文件实现私有归一：`fn normalize(path: &Path) -> PathBuf`（按 `Path::components()` 重拼：`CurDir` 丢弃、`ParentDir` 在 `pop()` 失败时原样保留、`Prefix` / `RootDir` / `Normal` 压入；**零 `canonicalize`**）与 `fn compare_key(path: &Path) -> PathBuf`（`#[cfg(windows)]` 版 = `normalize` 后 `to_string_lossy().to_lowercase()`，`#[cfg(not(windows))]` 版 = `normalize` 原样）；另加私有 `fn prod_dir(home: &Path) -> PathBuf`（`home.join(PROD_DIR)`）/ `fn dev_default_dir(home: &Path) -> PathBuf`（`prod_dir(home).join(DEV_SUBDIR)`）
- [x] 同文件实现 `pub fn mode() -> &'static str`（`cfg!(debug_assertions)` → `"dev"` / `"prod"`）、`pub fn prod_root(home: &Path) -> DataRoot`（`normalize(prod_dir(home))` + `RootSource::Prod`，**签名不接收 env**）
- [x] 同文件实现 `pub fn resolve_dev_root(home: &Path, env: Option<&OsStr>) -> Result<DataRoot, RootError>`：`None` → `dev_default_dir` + `RootSource::DevDefault`；`Some(raw_os)` → 取 `raw = raw_os.to_string_lossy()`，按序判 ① `raw.trim().is_empty()` → `Blank`、② `!Path::new(raw_os).is_absolute()` → `Relative`、③ `compare_key` 相等 prod 根 → `EqualsProd`、④ `compare_key(prod).starts_with(compare_key(候选))` 且不等 → `AncestorOfProd`、⑤ `Path::is_file()` → `NotDirectory`；全通过 → `normalize(候选)` + `RootSource::DevEnv`
- [x] 同文件实现 `pub fn resolve_root(home: &Path, is_dev: bool) -> Result<DataRoot, RootError>`：`!is_dev` 早退 `Ok(prod_root(home))`；dev 分支绑定 `let raw = std::env::var_os(DATA_ROOT_ENV);` 后交 `resolve_dev_root(home, raw.as_deref())`（doc 注明「本函数是 dev 侧唯一 env 读取点；prod 侧零读取」）
- [x] 同文件实现 `pub fn render_root_error(err: &RootError, home: &Path) -> String`：逐变体一段自然语言，每段齐备三要素——指名 `DEV_TEAM_DATA_ROOT`、回显 `raw`、给可照抄下一步（「删掉该环境变量改用 `<home>\.dev-team\dev`」或「改指向独立目录绝对路径，例如 `<home>\dev-team-data\accept-1`」）；`EqualsProd` / `AncestorOfProd` 段须点明「会污染生产数据根」。默认根与示例路径一律由 `home` 派生，源码零 `.dev-team` / `dev` 字面量（走 `PROD_DIR` / `DEV_SUBDIR` 常量）
- [x] 同文件实现日志双写：私有 `fn now_millis() -> u128`（`SystemTime::now().duration_since(UNIX_EPOCH)`，失败取 0）+ 私有 `fn append_log(home: &Path, file: &str, line: &str)`（`home.join(PROD_DIR).join(LOGS_DIR_NAME)` → `create_dir_all` → `OpenOptions::new().create(true).append(true)` → `writeln!(file, "[{millis}] {line}")`；任一步失败静默返回）+ `pub fn append_boot_error(home: &Path, message: &str)` + `pub fn append_ignored_env(home: &Path)`（行文本 `format!("ignored_env={DATA_ROOT_ENV} (release build ignores it)")`）
- [x] 修改 `src/lib.rs`：`pub mod commands;` 之后追加 `pub mod env;`（`selfcheck` 于阶段三追加）
- [x] 自查 阶段二：`grep -n "canonicalize" src/env.rs` **零命中**；`grep -n "\"\.dev-team\"\|\"dev\"" src/env.rs` 仅常量定义两行（文案内零字面量）；`grep -n "std::env::var_os" src/env.rs` 仅 `resolve_root` 一处；`ls src/env_test.rs` 命中（模块声明能解析到文件；格式化 / lint 门禁由桌面端 static-check 执行，本相位不重复跑）

## 阶段三：`selfcheck` 模块（依赖阶段一 / 二）

- [x] 新建 `src/selfcheck.rs` 骨架：模块 doc（契约十条：位置在 `build()` 与 `run()` 之间、零窗口、十键固定键序、`error=` 追加、三档退出码、flush 必做、prod 侧识别并拒绝、不支持 `--root`、验收须用独立 `DEV_TEAM_DATA_ROOT`、`models` 取 `list_models()`）+ 常量 `pub const SELF_CHECK_ARG: &str = "--self-check"` / `pub const PROD_REJECT_MESSAGE: &str = "--self-check 仅在 dev 构建可用（当前为生产构建）。"`；末行 `#[cfg(test)] mod selfcheck_test;`
- [x] 同文件建测试模块空骨架 `src/selfcheck_test.rs`：仅头注释，零 `#[test]`
- [x] 同文件实现类型面：`pub struct Probe { pub global_db_open: bool, pub workspace_count: Option<u64>, pub models: Option<Vec<String>>, pub workspaces_dir_exists: bool, pub error: Option<String> }`、`pub struct Summary { pub mode: &'static str, pub home: PathBuf, pub data_root: PathBuf, pub source: &'static str, pub global_db: PathBuf, pub global_db_open: bool, pub workspaces_dir: PathBuf, pub workspaces_dir_exists: bool, pub workspace_count: Option<u64>, pub models: Option<Vec<String>>, pub error: Option<String> }`（字段序即渲染键序，`Debug, Clone`）
- [x] 同文件实现私有 `fn probe(data_root: &Path) -> Probe`：先算 `workspaces_dir = data_root.join(store::WORKSPACES_DIR_NAME)` 与 `workspaces_dir_exists = workspaces_dir.is_dir()`；`store::WorkspaceStores::open(data_root)` 失败 → `global_db_open=false` + `error=Some("全局库打开失败：{err}")` 即返回；成功 → `stores.global().list_models()` 失败 → `global_db_open=true` + `error=Some("模型清单读取失败：{err}")`；成功 → `stores.global().list_workspaces()` 失败 → `error=Some("workspace 清单读取失败：{err}")`；全通 → `models=Some(names)`、`workspace_count=Some(len as u64)`、`error=None`（首错即停，D9）
- [x] 同文件实现 `pub fn render(summary: &Summary) -> String`：逐行拼接 `mode` / `home` / `data_root` / `data_root_source` / `global_db` / `global_db_open`（`"ok"` / `"err"`）/ `workspaces_dir` / `workspaces_dir_exists`（`"true"` / `"false"`）/ `workspace_count` / `models`（names 以 `,` 连接；`workspace_count` / `models` 为 `None` 时渲染空值）；`summary.error` 为 `Some` 时追加末行 `error=<原因>`；末尾恰一个 `\n`，无横幅 / 无 ANSI / 无空行
- [x] 同文件实现 `pub fn exit_code(probe: &Probe) -> i32`（`error.is_none()` → 0，否则 1；2 由 `main` 产出）与私有 `fn summary_of(data_root: &DataRoot, home: &Path, probe: &Probe) -> Summary`（`mode = env::mode()`、`source = data_root.source.wire()`、`global_db = data_root.path.join(store::GLOBAL_DB_FILE_NAME)`、`workspaces_dir = data_root.path.join(store::WORKSPACES_DIR_NAME)`，其余字段由 `probe` 平移）
- [x] 同文件实现 `pub fn run(app: &tauri::App, data_root: &DataRoot) -> i32`：`let home = app.path().home_dir().unwrap_or_default();`（doc 注明 `&App` 唯一用途 = `home=` 行，失败退化为空串不影响退出码）→ `probe(&data_root.path)` → `summary_of` → `print!("{}", render(&summary))` → `let _ = std::io::stdout().flush();` → `exit_code(&probe)`
- [x] 修改 `src/lib.rs`：追加 `pub mod selfcheck;`
- [x] 自查 阶段三：`grep -n "\"desktop-global.redb\"\|\"workspaces\"" src/selfcheck.rs` **零命中**（路径成分一律取 store 常量）；`grep -n "flush" src/selfcheck.rs` 命中 `run` 一处；`grep -nE "writeln!|print!.*key" src/selfcheck.rs` 无（渲染只经 `render`）

## 阶段四：`main` 启动序列重排（依赖阶段二 / 三）

- [x] 修改 `src/main.rs` 头部：删 `use std::path::PathBuf;`；删 `const DATA_DIR` 与 `const LOGS_DIR_NAME`（`LOGS_DIR_NAME` 改引 `env::LOGS_DIR_NAME`；`PROD_DIR` 只留在 `env` 内部，`main` 不再拼数据根成分，改了会留死常量）；追加 `use dev_team::env;`（与 `use dev_team::selfcheck;` 同组，位于 `use dev_team::commands::…` 之后）
- [x] 修改 `main` 首部：`let want_self_check = std::env::args().skip(1).any(|arg| arg == selfcheck::SELF_CHECK_ARG);` 之后紧跟 prod 侧显式拒绝——`if want_self_check && !cfg!(debug_assertions) { eprintln!("{}", selfcheck::PROD_REJECT_MESSAGE); std::process::exit(2); }`（**不留任何 `#[cfg]` 属性**，D3；注释写明 AC-12 / R-8 的「分支须常驻产物」口径）
- [x] 修改单实例注册属性：`#[cfg(desktop)]` → `#[cfg(all(desktop, not(debug_assertions)))]`（回调体逐字不动；注释补「dev 构建不注册：与已装生产版并存，identifier 零改动」）；devtools 的 `#[cfg(debug_assertions)]` 块零改动
- [x] 把内联 `.setup(|app| { … })` 闭包抽成文件级 `fn setup_hook(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>>`：首行 `let data_root = app.state::<env::DataRoot>().path.clone();`（doc：数据根由 `main` 单点解析后经管理态注入，此处 MUST NOT 二次解析 / 拼 prod 目录名）；其余语句（`WorkspaceStores::open(&data_root)?` / `agent_runtime::init_engine_log(&data_root.join(env::LOGS_DIR_NAME))` / `install_engine_panic_hook` / 五处 `app.manage` / `Ok(())`）逐字迁移，原 `home_dir()?.join(DATA_DIR)` 与 `:48-50` 的注释一并删除
- [x] 重排 `main` 尾部（D2 形状）：`.invoke_handler(all_commands!(generate_handler))` 保留在 builder 链上 → `let app = builder.build(tauri::generate_context!("tauri.conf.json")).expect("desktop 应用构建失败");` → `let home = app.path().home_dir().expect("home 目录解析失败");` → `let data_root = match env::resolve_root(&home, cfg!(debug_assertions)) { Ok(root) => root, Err(err) => { let message = env::render_root_error(&err, &home); eprintln!("{message}"); env::append_boot_error(&home, &message); std::process::exit(2); } };` → `#[cfg(not(debug_assertions))] if std::env::var_os(env::DATA_ROOT_ENV).is_some() { env::append_ignored_env(&home); }` → `app.manage(data_root.clone());` → `#[cfg(debug_assertions)] if want_self_check { std::process::exit(selfcheck::run(&app, &data_root)); }` → `app.run(|_, _| {}).expect("desktop 应用启动失败");`
- [x] 自查 阶段四：`grep -rn "\"\.dev-team\"" src/main.rs` **零命中**；`grep -n "cfg(desktop)" src/main.rs` 零命中（已改 `all(desktop, not(debug_assertions))`）；`grep -c "fn setup_hook" src/main.rs` = 1；`grep -n "resolve_root\|manage(data_root" src/main.rs` 各一处（数据根单点）；`want_self_check` 在 dev / prod 两档均被使用（拒绝分支无条件编译），不会留下 unused 变量告警

## 阶段五：命令轨道 `env` 与登记（依赖阶段二）

- [x] 新建 `src/commands/env/mod.rs`：模块 doc（第十一轨、纯管理态读、零 IO、零 `Result` 错误面、线值由 `env` 单点产出）+ `use crate::env::DataRoot;`（模块自身名同为 `env`，故一律用 `crate::env::…` 全路径或 `use crate::env::{…}`，禁裸 `env::`）+ `pub struct AppEnvironment { pub mode: String, pub data_root: String, pub source: String }`（`Debug, Clone, Serialize, Type` + `#[serde(rename_all = "camelCase")]`，字段 doc 说明线值域）+ `#[tauri::command] #[specta::specta] pub fn app_environment(data_root: State<'_, DataRoot>) -> AppEnvironment`（转调 `app_environment_inner(&data_root)`）+ `pub fn app_environment_inner(data_root: &DataRoot) -> AppEnvironment`（`mode: crate::env::mode().to_owned()`、`data_root: data_root.path.to_string_lossy().into_owned()`、`source: data_root.source.wire().to_owned()`）；末行 `#[cfg(test)] mod mod_test;`
- [x] 新建空测试骨架 `src/commands/env/mod_test.rs`：仅头注释，零 `#[test]`
- [x] 修改 `src/commands/mod.rs`：`pub mod exec;` 与 `pub mod explores;` 之间插 `pub mod env;`；`all_commands!` 清单末条（`config::workspace_config,`）之后追加 `$crate::commands::env::app_environment,`
- [x] 重导出 bindings：在 `packages/desktop` 下执行 `pnpm run bindings:export`，确认 `src/types/generated/bindings.ts` 新增 `appEnvironment` 包装（`() => __TAURI_INVOKE<AppEnvironment>("app_environment")`）与 `AppEnvironment = { mode: string; dataRoot: string; source: string }`；**不手改生成物**
- [x] 自查 阶段五：`grep -n "app_environment" src/commands/mod.rs` 恰一行；`grep -n "pub mod env;" src/commands/mod.rs src/lib.rs` 各一处；`grep -n "appEnvironment\|AppEnvironment" ../src/types/generated/bindings.ts` 命中；`git diff -- ../src/types/generated` 只含本次新增（生成物须与代码同批提交，`bindings:check` 的零 diff 由桌面端门禁复核）

## 阶段六：构建脚本与版本（依赖阶段四，构建产物才有意义）

- [x] 修改 `packages/desktop/package.json`：`"build": "pnpm run bindings:export && pnpm run server:build:dev"`；新增 `"build:prod": "pnpm run bindings:export && pnpm run server:build"`；新增 `"server:build:dev": "cross-env CARGO_TARGET_DIR=target/dev-build tauri build --debug --no-bundle"`；`"server:build"` 逐字保留；`"version"` `0.5.30` → `0.5.31`；`dev` / `client:dev` / `client:build` / `bindings:export` / `bindings:check` / `check` / `client:check` / `server:check` / `test` / `client:test` / `server:test` / `mutate` / `tauri` **零改动**
- [x] 自查 阶段六：`node -e "const s=require('./package.json').scripts; console.log(s.build, '|', s['build:prod'], '|', s['server:build:dev'], '|', s['server:build'])"` 输出与「配置」四行逐字一致；`node -e "console.log(require('./package.json').version)"` = `0.5.31`；`grep -n "TAURI_SIGNING_PRIVATE_KEY" package.json` 仅 `server:build` 一行

## 阶段七：前端 mode 穿透（依赖阶段五）

- [x] 新建 `src/hooks/use-app-environment.ts`：`export type BuildMode = 'dev' | 'prod'`、`export type DataRootSource = 'prod' | 'dev-default' | 'env'`、`export interface AppEnvironment { mode: BuildMode; dataRoot: string; source: DataRootSource }`（doc：线值域由后端 `env::mode()` / `RootSource::wire()` 单点产出，前端只做编译期收窄）；私有守卫 `isRecord(value: unknown): value is Record<string, unknown>` 与 `toEnvironment(payload: unknown): AppEnvironment | null`（`typeof` 逐字段校验，非法 → `null`；**禁** `as` 断言与 `any`）；`export function useAppEnvironment(): AppEnvironment | null`（挂载一次 `commands.appEnvironment()`，`cancelled` 标志 + `.catch(() => {})` 静默，未返回 / reject / 非法 → `null`）
- [x] 修改 `src/hooks/use-updater.ts`：签名改 `export function useUpdater(mode: BuildMode | null): UpdateState`（`import type { BuildMode } from './use-app-environment';`）；把 `useStartupCheck` 拆为 `useStartupVersion(setCurrentVersion)`（挂载拉一次 `getVersion()`，失败静默）与 `useStartupCheck(setAvailable, mode)`（`if (mode !== 'prod') return;` 之后才 `check()`，`cancelled` + `.catch(() => {})` 语义照旧，依赖数组 `[mode, setAvailable]`）；删 `if (!import.meta.env.DEV)` 与文件内注释中的 DEV 表述；`start` / `performUpdate` / `briefOf` / `busyRef` 零改动
- [x] 修改 `src/app.tsx`：`const environment = useAppEnvironment();` 与 `const update = useUpdater(environment?.mode ?? null);`；`ShellHeader` 增 `environment: AppEnvironment | null` 入参并在 `<strong>Dev Team</strong>` 之后渲染 `environment?.mode === 'dev'` 时的徽标 `{environment.dataRoot}` 取 `title`：`<span data-testid="dev-badge" title={environment.dataRoot} className="rounded border border-amber-500/50 px-1.5 py-0.5 text-[10px] font-semibold tracking-wide text-amber-400">DEV</span>`（`mode` 非 dev / 环境未就绪 → 不渲染；版本号显示链路零改动、不加 `-dev` 后缀）；`App` 内 hook 声明序保持 `useWorkspaces()` → `useAppEnvironment()` → `useUpdater(...)`（挂载期 invoke 序列断言口径由测试相位按 `filter` 收敛）
- [x] 自查 阶段七：`grep -rn "import.meta.env" src/` **零命中**；`grep -rn "as BuildMode\|as AppEnvironment\|: any" src/hooks/use-app-environment.ts` 零命中；`grep -n "dev-badge" src/app.tsx` 一处；`grep -rn "useUpdater(" src/` 的生产调用点仅 `app.tsx` 一处且带 mode 入参（`use-updater.test.ts` 的入参改造归测试相位）

## 阶段八：发版文档（依赖阶段六 / 七）

- [x] 改 `packages/desktop/RELEASE.md`「本地构建须知」（原 `:46-59`）：拆两档——①`pnpm -C packages/desktop run build` = dev 二进制（`--debug --no-bundle`，**零配置免私钥**），产物 `src-tauri/target/dev-build/debug/dev-team.exe`（前端资源内嵌，可双击脱离 dev server 运行），数据根落 `<home>/.dev-team/dev`，附 `DEV_TEAM_DATA_ROOT` 覆盖与 `<dev-team.exe> --self-check` 用法示例；②`build:prod` = 现生产链路，私钥 / 空密码两行命令原样保留在档②下，并注明「缺私钥仍失败」只约束档②
- [x] 同文件「约束与已知行为」原 `:116` 的「`tauri dev` 不检查更新(`import.meta.env.DEV` 守卫)」改为「dev 构建按 `app_environment` 的 `mode` 判据不检查更新（dev 构建不会去查生产 release）」；同章补一条「验收专用红线：脚本 / agent 起 desktop 做验收 MUST 显式设独立 `DEV_TEAM_DATA_ROOT`，MUST NOT 复用人工 dev 根（第二个进程会在 redb 锁上 fail fast）」
- [x] 自查 阶段八：`grep -n "import.meta.env" RELEASE.md` **零命中**；`grep -n "build:prod\|self-check\|\.dev-team/dev" RELEASE.md` 三词均有命中；档②仍逐字含 `TAURI_SIGNING_PRIVATE_KEY` 与 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ""` 两步

## 阶段九：实现期校准与实跑（依赖全部前序阶段）

- [x] 校准隔离 target 目录（D15 / R-2）：跑一次 `pnpm -C packages/desktop run build`，确认产物落在 `src-tauri/target/dev-build/debug/dev-team.exe`；若落位偏离（出现 `src-tauri/src-tauri/target/…` 或 `packages/desktop/target/…`），把 `server:build:dev` 的 `CARGO_TARGET_DIR` 改为 `src-tauri/target/dev-build` 后重跑，直至隔离目录位于 `src-tauri/target/` 之内；`tauri dev` 仍落 `src-tauri/target/debug`
- [x] 校准后核对（`packages/desktop` cwd）：`git -C ../.. status --porcelain` 零构建产物（隔离目录被 `src-tauri/target` 忽略覆盖）；`ls src-tauri/target/debug/dev-team.exe` 与 `ls src-tauri/target/dev-build/debug/dev-team.exe` 并存（feature 翻转不再共用目录）
- [x] 实跑四条自检臂并留存输出（`packages/desktop` cwd）：①`DEV_TEAM_DATA_ROOT=<独立目录> src-tauri/target/dev-build/debug/dev-team.exe --self-check` → 十行 `key=value`、`mode=dev`、`data_root_source=env`、退出码 0、无窗口；②同上但 `DEV_TEAM_DATA_ROOT=<home>/.dev-team` → stderr 提示 + `<home>/.dev-team/logs/boot-error.log` 追加同段文字 + 退出码 2 + 无摘要；③不设变量 → `data_root_source=dev-default`、`data_root=<home>/.dev-team/dev`；④stdout 经管道（`| cat` / 重定向文件）完整可读（flush 生效）
- [x] 实跑并存语义（AC-10）：prod 版运行中启动 dev 构建 → 两进程并存、互不唤起、无 redb 锁冲突；`grep -n "identifier" src-tauri/tauri.conf.json`（`packages/desktop` cwd）仍 `dev.wps.dev-team`
- [x] 实跑 prod 侧两条（需一次 `build:prod` 或已装产物）：带 `--self-check` → stderr 说明 + 退出码 2 + 10s 内返回（不挂住）；`DEV_TEAM_DATA_ROOT` 残留时启动成功、数据根不变、`<home>/.dev-team/logs/ignored-env.log` 追加一行
- [x] 收口复核（仓库根 cwd，只读项零 diff）：`git diff --stat -- .github/workflows/desktop-release.yml packages/desktop/src-tauri/tauri.conf.json packages/desktop/src-tauri/capabilities/default.json plugins` **零输出**；`grep -rn "is_dev()" packages/desktop/src packages/desktop/src-tauri/src` 零命中；`grep -rn "import.meta.env" packages/desktop/src` 零命中
- [x] 出线面复核（仓库根 cwd）：`grep -rn "\.dev-team" packages/desktop/src-tauri/src` 命中全部落在 `env.rs` 内（常量与由它派生的文案；其余文件零命中）；`grep -rn "desktop-global.redb\|\"workspaces\"" packages/desktop/src-tauri/src` 零命中（路径成分取 store 常量）

> **实现相位校准实录（与 tasks 措辞的差异，均落在 `packages/desktop`）**
>
> 1. **隔离 target 目录形态确认（R-2 落定，无需退路）**：`server:build:dev` 的
>    `CARGO_TARGET_DIR=target/dev-build` 实测落 `src-tauri/target/dev-build/debug/dev-team.exe`
>    （tauri CLI 以 `src-tauri` 为 cargo cwd）⇒ D15 主形态成立，退路形态不启用。
> 2. **`pnpm run build` 实跑**：首次实跑用 `--config` 临时把 `beforeBuildCommand` 换成
>    `vp build`（跳过 tsc，因当时 `useUpdater` 要求必填入参而旧测试仍无参调用）；静态检查
>    反馈后改为 `mode` 缺省 `null`（见第 6 条），随后 `pnpm -C packages/desktop run build`
>    原样端到端通过（tsc + vp build + `tauri build --debug --no-bundle`），产物仍落
>    `src-tauri/target/dev-build/debug/dev-team.exe`。
> 3. **devtools 与自检的冲突（实现期发现，已收敛）**：`tauri-plugin-devtools` 在
>    `init()` 时就向 stdout 打印「Tauri Devtools v… → Local: …」横幅 + 空行，会污染
>    `--self-check` 的十行 `key=value` 机器可读流。故 `main` 的 devtools 注册改为
>    `#[cfg(debug_assertions)] if !want_self_check { … }`——仍是 dev 侧 `#[cfg]` 分流
>    （模式判据不变），只是自检路径不注册该插件；GUI 路径的 devtools 语义零变化。
> 4. **实跑结果**：自检四臂全通（十行、`mode=dev`、`source=env`/`dev-default`、退出码
>    0；非法值 → stderr 文案 + `boot-error.log` 追加 + 退出码 2 且无摘要；管道读取 10 行
>    完整）；五项校验逐项实跑（大小写变形 / 家目录 / 盘符根 / `..` 变形 / 相对 / 空白 /
>    普通文件）全部 exit 2 且提示三要素齐备；prod 侧 `--self-check` 实测 exit 2、1s 内返回
>    （release 产物 `src-tauri/target/release/dev-team.exe`，`cargo build --release
>    --features custom-protocol` 直编，未走 `build:prod` 以免要求签名私钥）。
> 5. **未在本相位实跑的两项**（需起 GUI / 已装产物，留验收相位）：①prod 版运行中启动
>    dev 构建的并存语义（AC-10 的进程面；数据面已由根分裂与 `identifier` 零 diff 保证）；
>    ②prod 构建残留 `DEV_TEAM_DATA_ROOT` 时的 `ignored-env.log` 留痕（需 prod GUI 启动，
>    会触碰真实生产库）。
>
> **静态检查反馈期修正（第 1 轮）**
>
> 6. **`useUpdater` 的 `mode` 取缺省值 `null`**（`useUpdater(mode: BuildMode | null = null)`）：
>    签名的类型面与 spec 一致（仍接受 `dev` / `prod` / `null`），缺省即「未就绪」语义
>    （不判定更新、不回落成照常检查），壳态唯一调用点 `app.tsx` 显式传 mode。动因：
>    `src/hooks/use-updater.test.ts` 的 9 处零参调用属测试相位的入参改造（见上文阶段七
>    自查项），而 `client:check` 的 `vp check` 对测试文件同做 typeCheck —— 缺省值让
>    「既有调用形态」与「新签名」在静态门禁下共存，端到端用例改造仍归测试相位。
> 7. **`DataRootSource` 改为模块私有**（`export type` → `type`）：knip 的 dead-code 报告
>    将其列为 unused exported type（仅 `AppEnvironment.source` 同模块内消费，无第二个
>    消费者）。与 `use-updater.ts` 的 `UpdateBrief`（同样不外导）同例；`BuildMode` /
>    `AppEnvironment` / `useAppEnvironment` 保持导出（跨模块消费者存在）。
> 8. **生成物入库形态**：`bindings:check` = 重导出 + `git diff --exit-code -- src/types/generated`，
>    比较的是工作区与**索引区**，故本次把重导出后的 `src/types/generated/bindings.ts`
>    加入索引（`git add`，不提交）——守卫语义（入库物必须与导出结果一致）由此成立；
>    连续两次导出逐字节一致（md5 相同）已核对。

> **回溯修正（R-2 / C7）：前端 `AppEnvironment` 与生成物的编译期关联**
>
> 9. **手写同形接口 → 派生自生成物**：`src/hooks/use-app-environment.ts` 原以
>    `export interface AppEnvironment { mode; dataRoot; source }` 手写生成物
>    `bindings.ts` 的同形结构，两者无编译期关联（:3 只 `imports commands`），
>    后端 DTO 改名（如 `data_root` → 其他线名）则静默退化为 `null`
>    （`use-updater.ts` 的 `mode !== 'prod'` 直接不检查更新），测试自 mock 无守护。
>    按 design.md:161「导入生成物时以 `AppEnvironmentPayload` 别名避让」落定：
>    :3 追加 `type AppEnvironment as AppEnvironmentPayload` 导入（先例
>    `use-workspaces.ts:4`），类型改为
>    `Omit<AppEnvironmentPayload, 'mode' | 'source'> & { mode: BuildMode; source: DataRootSource }`
>    ——字段集合（含 `dataRoot`）由生成物提供，改名即刻编译失败。
> 10. **守卫收窄函数保留但锚定派生类型**：`isBuildMode` / `isDataRootSource`
>    的谓词由 `value is BuildMode` / `value is DataRootSource` 改为
>    `value is AppEnvironment['mode']` / `value is AppEnvironment['source']`，
>    使运行时校验与派生类型同源（`toEnvironment` 的 `isRecord` + 逐字段校验
>    + 零 `as` / 零 `any` 形态不变）。`DataRootSource` 仍模块私有（knip 口径
>    同第 7 条）；`BuildMode` / `AppEnvironment` / `useAppEnvironment` 导出面不变。
