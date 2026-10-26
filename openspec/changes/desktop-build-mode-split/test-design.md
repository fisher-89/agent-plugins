# 测试设计: desktop-build-mode-split

> **变更**: desktop-build-mode-split
> **日期**: 2026-10-26
> **基于**: `proposal.md`（验收标准 AC-1 ~ AC-17；「变更范围 - 实现文件」14 行 / 「变更范围 - 测试文件」10 行 / 「不要修改」只读红线）、`design.md`（「新增文件」4 项 /「修改文件」10 项；「公共函数 / API」子表 26 行；「类型定义」子表 10 行；「配置」子表 7 行；关键设计决策 D1 ~ D17；待决问题 1 ~ 5）、`specs/{desktop-build-modes,desktop-workspace-store,desktop-data-dimensions,desktop-release-pipeline}/spec.md`
> **被测模块来源**: `design.md`「新增文件」4 项（`src-tauri/src/env.rs`、`src-tauri/src/selfcheck.rs`、`src-tauri/src/commands/env/mod.rs`、`src/hooks/use-app-environment.ts`）+「修改文件」10 项（`src-tauri/src/lib.rs`、`src-tauri/src/main.rs`、`src-tauri/src/commands/mod.rs`、`crates/infra/store/src/store.rs`、`crates/infra/store/src/lib.rs`、`packages/desktop/package.json`、`src/hooks/use-updater.ts`、`src/app.tsx`、`src/types/generated/bindings.ts`、`packages/desktop/RELEASE.md`）；design 无「删除文件」子表（`main.rs` 的 `DATA_DIR` / `LOGS_DIR_NAME` 两常量删除属迁移，不作独立模块）。
> **测试框架**: 沿用仓库既有两套模式——Rust 侧 `cargo test --workspace`（`#[test]`；dev-deps 既有 `tempfile`、`tauri` 的 `test` feature（`tauri::test::mock_app()` MockRuntime，`commands/{agents,db,changes}` 先例）；测试模块以**同目录兄弟文件**承载：`src-tauri/src/{env,selfcheck}.rs` 内 `#[cfg(test)] mod {env_test,selfcheck_test};`、`commands/env/mod.rs` 内 `mod mod_test;`（`commands/config/mod.rs` 先例）；断言用标准 `assert!` / `assert_eq!` / `assert!(matches!(..))`，测试函数名为中文标识符、断言消息为中文；无外部断言库、无独立测试运行器）。前端侧 `vp test`（`vite-plus/test` 的 `describe` / `expect` / `it` / `vi`；jsdom + `@testing-library/react` 的 `renderHook` / `render` / `waitFor` / `screen`；进程边界以 `vi.mock` + `vi.hoisted` 桩替换，`*.test.ts` / `*.test.tsx` 与被测文件同目录）。
> **一处显式补列**: `src-tauri/src/commands/mod.rs` 的公共 API 行（`all_commands!` 命令清单）落点取**既有** `src-tauri/src/bindings/mod_test.rs`——该文件已维护 `COMMAND_NAMES` / `COMMAND_WRAPPERS` / `DTO_TYPES` 三张清单并驱动 `export_bindings()`（design「修改文件」表未列该文件，但其自带口径「按现行口径只补录本变更自身新命令」），故映射对为 `commands/mod.rs -> bindings/mod_test.rs`，**不**新建 `commands/mod_test.rs`（design 与 tasks 均未声明该模块）。
> **不可解析测试路径的模块**: `src-tauri/src/main.rs`、`src-tauri/src/lib.rs`、`packages/desktop/package.json`、`packages/desktop/RELEASE.md`、`src/types/generated/bindings.ts` —— 见 `## 不可测试项` 第 1 ~ 5 项。`crates/infra/store/src/lib.rs` 只追加两名字重导出（无行为），保留章节骨架（`#### 待测功能` / `#### 用例` / `#### Mock策略` 均以 HTML 注释说明「design.md 未声明该文件的公共 API 变更」），不新建 `lib_test.rs`。

用例表读法：`路径类型` ∈ {正向, 异常, 边界}；`迭代类型` ∈ {单值, 参数化, 表驱动, 遍历枚举}。跨模块组合用例按「链路发起方 / 最上层调用方」挂靠入口模块的 `#### 用例` 表（本变更的组合锚分别挂靠 `env.rs`——三分优先级全链 / 双写渲染链 / 与 store 常量的根分裂链，`selfcheck.rs`——探库 → 摘要 → 退出码链，`commands/env/mod.rs`——管理态 → 线面平移链，`app.tsx`——mode 穿透 UI 链），不设独立集成测试章节。

边界取值口径（按 design「类型定义」/「公共函数 / API」/「配置」表的载体逐项收敛）：

| 类型（载体） | 边界取值 | 最少条数 |
|---|---|---|
| `&str` / `String`（`DEV_TEAM_DATA_ROOT` 值、`RootError::raw`、日志 message、`AppEnvironment.data_root`） | `""`（空）/ 全空白（`"   "` / `"\t\n "`）/ 超长（> 1000 字符）/ 特殊字符（`\n` `\0` emoji、中文路径、`=`、尾分隔符、`.` / `..` 段）/ 平台非法形态（非 UTF-8 `OsStr`，`cfg(unix)`） | 4 边界 + 1 正常 |
| `&Path` / `PathBuf`（`home` / `DataRoot.path` / 候选根 / `global_db` / `workspaces_dir`） | 空 Path / 相对路径 / 不存在绝对路径 / 已存在普通文件 / 已存在目录 / 盘符根 / 超长含中文与 emoji / Windows 大小写等价变形 | 4 边界 + 1 正常 |
| `Option<&OsStr>`（`resolve_dev_root` 的 `env`） | `None`（未设）与 `Some(合法)` / `Some(空白)` / `Some(相对)` / `Some(等于 prod 根)` / `Some(prod 祖先)` / `Some(文件)` 各档 | 边界合并于五项校验 |
| `Option<u64>` / `Option<Vec<String>>`（`Probe` / `Summary` 未知值） | `None` / `Some(0)` / `Some(vec![])` / `Some(单元素)` / `Some(三元素短名)` / `Some(1000 项大列表)` | 4 边界 + 1 正常 |
| `bool`（`is_dev` / `global_db_open` / `workspaces_dir_exists` / `error.is_none()`） | `true` 与 `false` 双向必断言（`ok` / `err`、`true` / `false`） | 每值 1 |
| enum（`RootSource` 3 值 / `RootError` 5 variant） | 逐值遍历（封闭集，非法值编译期不可表达）；`RootError` 每 variant 单独断言提示三要素 | N+1 |
| `i32`（`exit_code` 返回 / 进程退出码） | `0`（全通）/ `1`（库或数据根失败）/ `2`（env 非法、prod 拒自检——由 `main` 产出，不在 `exit_code` 值域） | 3 |
| 前端线面 object（`app_environment` 载荷） | `null` / 非对象（`[]` / `'dev'` / `5` / `true`）/ 缺 `mode` / 缺 `dataRoot` / 缺 `source` / 多余字段 / 非法枚举名义值（`mode: 'DEV'`、`source: 'dev'`）/ 类型错（`mode: 5`、`dataRoot: null`） | 4 边界 + 1 正常 |
| `string`（前端 `dataRoot` / `mode` / `source` 线值） | `""` / 超长 > 1000 / 特殊字符（中文、emoji、`\n`、`\\`）/ 非法枚举值 | 4 边界 + 1 正常 |
| `UpdateState`（`useUpdater` 返回面） | 初值（`currentVersion: null` / `available: null` / `status: 'idle'` / `progress: null` / `error: null`）与四终态（`downloading` / `installing` / `error` / `idle` 回退） | 5 |

路径省略说明：`验收范围` 表内的 `src-tauri/…` / `src/…` / `crates/…` 为 `packages/desktop/` 下相对路径；下文 `## 单元测试` 各节标题给仓库根相对全路径。

---

## 验收范围

| AC ID | 验收条件（摘要，原文见 `proposal.md`） | 被测文件或模块 |
|-------|---------|----------------|
| AC-1 | 判别源单点：无 `tauri::is_dev()` / 无 `import.meta.env.DEV` 模式判据 / 无 build.rs 常量；Rust 只经 `env::mode()`、前端只经 `app_environment` IPC | `packages/desktop/src-tauri/src/env.rs` -> `env_test.rs`（`mode()` 与 `cfg!(debug_assertions)` 同源）、`commands/env/mod.rs` -> `mod_test.rs`（IPC 单源平移）、`src/hooks/use-updater.ts` -> `use-updater.test.ts`（判据换 `mode === 'prod'`）、`src/app.tsx` -> `app.test.tsx`（mode 只从 IPC 取）；源码零残留扫描见 `## 不可测试项` 第 4 项 |
| AC-2 | 默认构建落 dev：`pnpm build` 成功、不产安装包、产物可双击且运行期 `mode=dev` | `env.rs` -> `env_test.rs`（`mode()` 的 dev 档）、`selfcheck.rs` -> `selfcheck_test.rs`（`mode=dev` 行由 `env::mode()` 透传）；构建实跑与产物形态见 `## 不可测试项` 第 1、9 项 |
| AC-3 | `build:prod` 语义不削弱：缺私钥仍失败、设齐产出同形态安装包与 updater 工件 | `package.json` 脚本形态无单测路径 → `## 不可测试项` 第 1 项；运行期语义由 `env.rs` -> `env_test.rs`（`prod_root` / `resolve_root(false)` 恒 `source=prod`）承接 |
| AC-4 | target 目录隔离：`tauri dev` 落 `target/debug`、dev 构建落隔离目录、均被 gitignore 覆盖、`bindings:export` 不落隔离目录 | `package.json` 配置面无单测路径 → `## 不可测试项` 第 1 项（磁盘检查属实跑） |
| AC-5 | 数据根三分：无 env → `<home>/.dev-team/dev`（`dev-default`）；合法 env → 该值（`env`）且与默认根互不可见；prod → `<home>/.dev-team`（`prod`） | `env.rs` -> `env_test.rs`（三分 + 路径层互不可见 + 与 store 常量组合的「同名文件不同父目录」）；`crates/infra/store/src/store.rs` -> `store_test.rs`（库文件名 / 子树名单点） |
| AC-6 | 五项校验全拒（空白 / 相对 / 等于 prod 根 / prod 根严格祖先含 `..` 与大小写变形 / 目标是文件）+ 提示三要素齐备 + 硬失败退出码 2 | `env.rs` -> `env_test.rs`（五项逐 variant + `render_root_error` 三要素）；退出码 2 与 stderr 面见 `## 不可测试项` 第 9 项 |
| AC-7 | 归一口径：不存在路径不报 `NotFound`；`a/./b/../c/` 与 `a/c` 同判；Windows 大小写等价不绕过红线；源码无 `canonicalize` | `env.rs` -> `env_test.rs`（归一等价 + 大小写 + 首段 `..` 保留）；`canonicalize` 零命中属源码扫描 → `## 不可测试项` 第 4 项 |
| AC-8 | 双写落地：stderr 与 `<home>/.dev-team/logs/boot-error.log` 同一段文字；`logs/` 缺席自建；多次失败 append 不覆盖 | `env.rs` -> `env_test.rs`（`render_root_error` 与 `append_boot_error` 同函数产文 + 自建父目录 + append 不覆盖 + IO 静默）；stderr 通道见 `## 不可测试项` 第 9 项 |
| AC-9 | prod 侧 env 留痕：残留 `DEV_TEAM_DATA_ROOT` 时启动成功、根不变、`logs/ignored-env.log` 追加一行；无 env 不产生记录 | `env.rs` -> `env_test.rs`（`append_ignored_env` 行文本逐字 + append 幂等 + IO 静默 + 锚 prod 根）；prod 产物实跑见 `## 不可测试项` 第 9 项 |
| AC-10 | 单实例并存：prod 版运行中启动 dev 构建两者并存、无 mutex / redb 锁冲突；prod 双击语义不变；`identifier` 仍 `dev.wps.dev-team` | `env.rs` -> `env_test.rs`（数据根分裂是 redb 锁自动解除的前提：同一 workspace 在两模式下同名文件不同父目录）；装配期 `#[cfg(all(desktop, not(debug_assertions)))]` 与 `tauri.conf.json` 见 `## 不可测试项` 第 4、6 项 |
| AC-11 | `--self-check` 契约：dev 十行 `key=value`（键序固定）+ 退出码 0；库坏 → 1 且含 `error=`；env 非法 → 2 且与 GUI 同文案；零窗口；stdout 管道可读（flush） | `selfcheck.rs` -> `selfcheck_test.rs`（十键键序 + `error=` 行 + `exit_code` 0/1 映射，`run` 的返回码语义由其委托面覆盖）；`env.rs` -> `env_test.rs`（env 非法同一渲染函数）；进程级（`&App<Wry>` 入参 / 零窗口 / flush / 退出码 2）见 `## 不可测试项` 第 9 项 |
| AC-12 | prod 拒自检：stderr 文案 + 退出码 2 + 立即可返回；该分支不在 `#[cfg]` 内 | `selfcheck.rs` -> `selfcheck_test.rs`（`PROD_REJECT_MESSAGE` 逐字 + `SELF_CHECK_ARG` 参数名单点）；分支形态审查与实跑见 `## 不可测试项` 第 4、9 项 |
| AC-13 | 验收红线在案：验收 / 示例命令显式设独立 `DEV_TEAM_DATA_ROOT`；dev 实例运行中 self-check 不互相阻塞 | `RELEASE.md` 文档面无单测路径 → `## 不可测试项` 第 2 项（逐字核对属验收）；「独立根」机制面由 `env.rs` -> `env_test.rs` 的 `source=env` 用例承接 |
| AC-14 | 前端穿透：`mode=dev` → `check` 零调用 + `DEV` 徽标（`title` = dev 数据根）；`mode=prod` → 启动检查一次 + 无徽标；`null` → 不判定、无错误态 | `src/hooks/use-app-environment.ts` -> `use-app-environment.test.ts`、`src/hooks/use-updater.ts` -> `use-updater.test.ts`、`src/app.tsx` -> `app.test.tsx` |
| AC-15 | 启动序列单点：`.dev-team` 字面量只在 `env` 常量一处；`setup_hook` 经 `State<DataRoot>` 取根、无二次解析；数据根失败经 `main` 处置（无 `Failed to setup app`） | `env.rs` -> `env_test.rs`（常量单点 + 提示文案由 `home` 派生、零字面量）、`commands/env/mod.rs` -> `mod_test.rs`（管理态 → 线面平移，无二次解析）；`main.rs` 装配序列见 `## 不可测试项` 第 4 项 |
| AC-16 | 零回归与门禁：`cargo test --workspace` 与 `vp test` 全绿；覆盖率达标；`bindings:check` 零 diff；`vp check` / `clippy` 零告警 | 本文件全部映射对（逐文件 `#### 用例` 表）+ `commands/mod.rs` -> `bindings/mod_test.rs`（重导出幂等零 diff）；门禁统计面见 `## 不可测试项` 第 7 项 |
| AC-17 | 交付面收口：`version` 0.5.30 → 0.5.31；`plugins/**` 零改动零 bump；workflow 与 `tauri.conf.json` 零 diff | 三项均无单测路径 → `## 不可测试项` 第 1、6 项（diff 审查） |

> 被测范围边界：`src-tauri/tauri.conf.json`、`src-tauri/capabilities/default.json`、`crates/**`（除 store 两常量可见性）、`src-tauri/crates/core/{agent,checks,config,foundation,orchestration,workflow}/**`、`src-tauri/crates/infra/{agent,checks,exec,vcs,watch}/**`、`src/views/**`、`src/components/**`、`src/routes.tsx`、`src/lib/**`、`plugins/**`、`openspec/config.json`、`.github/workflows/desktop-release.yml` 均为只读红线（design「只读红线零触点」），不纳入被测范围；其行为不变性由既有用例全绿承接（AC-16）。
>
> 机械随动（`src/bindings/mod_test.rs` 的 `COMMAND_NAMES` / `COMMAND_WRAPPERS` 各补 `app_environment` / `appEnvironment`、`DTO_TYPES` 补 `AppEnvironment`；`use-updater.test.ts` 删 `vi.stubEnv('DEV', …)` / `vi.unstubAllEnvs()` 并补 `mode` 入参；`app.test.tsx` 三处序列断言改 `filter` 口径）不构成源模块变更，随对应映射对同批落地。

---

## 单元测试

### packages/desktop/src-tauri/src/env.rs -> packages/desktop/src-tauri/src/env_test.rs

<!-- design.md「公共函数 / API」子表 14 行（6 常量 + `mode` / `prod_root` / `resolve_dev_root` / `resolve_root` / `render_root_error` / `append_boot_error` / `append_ignored_env` / `RootSource::wire`）与「类型定义」子表 3 行（`DataRoot` / `RootSource` / `RootError`）落本文件。测试模块以 `#[cfg(test)] mod env_test;` 在 `env.rs` 末行声明（tasks 阶段二已建空骨架）。私有项（`normalize` / `compare_key` / `prod_dir` / `dev_default_dir` / `append_log` / `now_millis`）不入 `#### 待测功能`，其行为一律经公共入口（`resolve_dev_root` / `append_boot_error` / `append_ignored_env`）观察——沿仓库「不为测试增加导出」规则。`std::env::var_os` 读取只在 `resolve_root` 的 dev 分支，故全局环境变量读写仅允许出现在「`resolve_root` — prod 侧不读 env」与「dev 分支透传」两组用例，并经进程级 `Mutex` 串行 + `Drop` 恢复（`commands` 组 `TEST_PATH_LOCK` 同式先例）。 -->

#### 待测功能

- `PROD_DIR: &str = ".dev-team"`（公共函数表，AC-15/D4）：prod 目录名唯一字面量，壳层其他位置零复写
- `DEV_SUBDIR: &str = "dev"`（公共函数表，AC-5/D4）：dev 子根名，嵌套落位 `<home>/.dev-team/dev`
- `DATA_ROOT_ENV: &str = "DEV_TEAM_DATA_ROOT"`（公共函数表，AC-5/AC-6/AC-9）：变量名单点（提示文案 / 留痕文本 / `var_os` 探测共用）
- `LOGS_DIR_NAME: &str = "logs"`（公共函数表，AC-8）：数据根下日志子树名（引擎日志与启动失败日志共用目录、分文件）
- `BOOT_ERROR_LOG_NAME: &str = "boot-error.log"`（公共函数表，AC-8）：启动失败日志文件名
- `IGNORED_ENV_LOG_NAME: &str = "ignored-env.log"`（公共函数表，AC-9）：prod 侧 env 留痕文件名
- `env::mode() -> &'static str`（公共函数表，AC-1/AC-2）：Rust 侧唯一模式判定，`cfg!(debug_assertions)` → `"dev"` / `"prod"`；线值与 IPC / 自检同名同形
- `env::prod_root(home: &Path) -> DataRoot`（公共函数表，AC-3/AC-5/D4/D6）：`source = Prod`；签名不接收 env ⇒ prod 不认 env 由结构保证
- `env::resolve_dev_root(home: &Path, env: Option<&OsStr>) -> Result<DataRoot, RootError>`（公共函数表，AC-5/AC-6/AC-7/D4/D5/D6）：纯函数——`None` → `<home>/.dev-team/dev`（`DevDefault`）；`Some` → 五项校验 → 逻辑归一路径（`DevEnv`）或 `Err`
- `env::resolve_root(home: &Path, is_dev: bool) -> Result<DataRoot, RootError>`（公共函数表，AC-1/AC-5/AC-10/D4）：薄分派——`!is_dev` 早退 `prod_root`（不读 env）；dev 分支读 `var_os(DATA_ROOT_ENV)` 后交 `resolve_dev_root`
- `env::render_root_error(err: &RootError, home: &Path) -> String`（公共函数表，AC-6/AC-8/AC-11/D7）：自然语言提示三要素（指名变量 / 回显原值 / 可照抄下一步）；GUI 启动与 `--self-check` 共用同一文案
- `env::append_boot_error(home: &Path, message: &str)`（公共函数表，AC-8/D8）：追加 `<home>/.dev-team/logs/boot-error.log`，自建父目录、`[millis]` 前缀、append-only、IO 失败静默
- `env::append_ignored_env(home: &Path)`（公共函数表，AC-9/D8）：追加 `<home>/.dev-team/logs/ignored-env.log`，行文本 `ignored_env=<DATA_ROOT_ENV> (release build ignores it)`
- `RootSource::wire(self) -> &'static str`（公共函数表，AC-5/AC-11/AC-14/D12）：线值单点——`Prod → "prod"` / `DevDefault → "dev-default"` / `DevEnv → "env"`，自检摘要与 IPC 共用
- 类型 `DataRoot` / `RootSource` / `RootError`（类型定义表，AC-5/AC-6/AC-15）：`DataRoot { path, source }`（`Debug, Clone, PartialEq, Eq`）；`RootSource` 三值（`Debug, Clone, Copy, PartialEq, Eq`）；`RootError` 五 variant 各带 `raw: String`（原值回显由类型携带，渲染侧不二次取 env）

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 六个常量 — 字面量单点（AC-15/D4） | 正向 | `PROD_DIR == ".dev-team"`、`DEV_SUBDIR == "dev"`、`DATA_ROOT_ENV == "DEV_TEAM_DATA_ROOT"`、`LOGS_DIR_NAME == "logs"`、`BOOT_ERROR_LOG_NAME == "boot-error.log"`、`IGNORED_ENV_LOG_NAME == "ignored-env.log"` 逐字断言 | 表驱动（6 常量） |
| 六个常量 — 形态守卫（AC-15） | 边界 | 六个值均非空、无前后空白、无路径分隔符（`/` `\\`）、无 `..` 段、无盘符 `:`——防后续误改造成路径逃逸 | 表驱动（6 常量） |
| 六个常量 — 嵌套关系（AC-5/D4） | 正向 | `dev 默认根 == home.join(PROD_DIR).join(DEV_SUBDIR)`；`boot-error.log` / `ignored-env.log` 落 `home.join(PROD_DIR).join(LOGS_DIR_NAME)`；两文件名互不相等且不等于 `engine.log`（分文件约束） | 单值 |
| `mode()` — 与判别源同源（AC-1/AC-2） | 正向 | 返回值 == `if cfg!(debug_assertions) { "dev" } else { "prod" }`；同进程两次调用恒同值；值域闭集 `{"dev", "prod"}`（无第三种可能） | 单值 |
| `mode()` — 与 `RootSource::wire()` 的 `prod` 同形（AC-1/D12） | 边界 | `mode()` 的 prod 字面量与 `RootSource::Prod.wire()` 逐字相等（两处产出的线值同形，不各拼字符串） | 单值 |
| `mode()` — 无错误面（异常类别不适用）（AC-1） | 异常 | 签名零入参、返回 `&'static str`（无 `Result`）⇒ 异常路径为零；进程内设 `DEV_TEAM_DATA_ROOT`（合法与非法各一档）后仍返回同值（判别源不读 env，MUST NOT 被外部变量污染） | 表驱动（2 形态） |
| `prod_root(home)` — 落位与来源（AC-3/AC-5） | 正向 | `path == home.join(PROD_DIR)`、`source == RootSource::Prod`；第二次调用结果相等（无隐藏状态） | 单值 |
| `prod_root(home)` — home 边界（AC-5/D4） | 边界 | `home` 为 `Path::new("")` / 相对路径 `Path::new("rel")` / 超长含中文与 emoji / 带尾分隔符 / 带 `.` 段 → 均不 panic；`path` 恒等于 `home.join(PROD_DIR)`（结构断言，不做归一、不查盘） | 表驱动（5 形态） |
| `prod_root(home)` — 环境变量零影响（AC-3/AC-5/D4） | 异常 | 进程内设 `DEV_TEAM_DATA_ROOT`（合法独立目录 / 非法空白各一档）后调 `prod_root(home)` → 与未设时逐字段相等（签名不接收 env ⇒ 非法 env 也不得产生 `Err`，prod 永不因残留 env 启动失败） | 表驱动（2 形态） |
| `resolve_dev_root` — 无 env 落 dev 默认根（AC-5） | 正向 | `resolve_dev_root(home, None)` → `Ok`，`path == home.join(PROD_DIR).join(DEV_SUBDIR)`、`source == DevDefault` | 单值 |
| `resolve_dev_root` — env 合法覆盖（AC-5） | 正向 | env = `<base>/accept-1`（绝对、非 prod 根及其祖先、不存在）→ `Ok`，`source == DevEnv`，`path` 逻辑归一后与输入等价；目标目录不存在不构成失败（不查盘 → 不 `NotFound`） | 单值 |
| `resolve_dev_root` — ①空白拒绝（AC-6/D6） | 异常 | env = `Some("")` / `Some("   ")` / `Some("\t\n ")` / `Some("\u{3000}")`（全角空白，`trim()` 同判）→ 均 `Err(RootError::Blank { raw })`，且 `raw` 与输入逐字相等（含空格原样） | 表驱动（4 形态） |
| `resolve_dev_root` — ②相对拒绝（AC-6/D6） | 异常 | env = `Some("dev-data")` / `Some("./data")` / `Some("..\\data")` → 均 `Err(RootError::Relative { raw })`；MUST NOT 按 cwd 解析（断言 `path` 不为 cwd 派生值——`Err` 无 `path` 字段，以 `matches!` + `raw` 原值双断言） | 表驱动（3 形态） |
| `resolve_dev_root` — ③等于 prod 根拒绝（AC-6/D6） | 异常 | env = `<home>/.dev-team` / `<home>/.dev-team/`（尾分隔符）/ `<home>/.dev-team/.`（`.` 段）/ `<home>//.dev-team`（重复分隔符）→ 均 `Err(RootError::EqualsProd { raw })` | 表驱动（4 形态） |
| `resolve_dev_root` — ④prod 根严格祖先拒绝（AC-6/D6） | 异常 | env = `<home>`（家目录）/ 盘符根（`<home>` 的 `ancestors()` 顶层）/ `<home>/.dev-team/..`（`..` 归一为 home）/ `<home>` 的任一上层 → 均 `Err(RootError::AncestorOfProd { raw })`；同一断言下 `<home>/.dev-team/dev/accept`（**后代**）→ `Ok`（不误拒） | 表驱动（4 形态） |
| `resolve_dev_root` — ⑤目标是文件拒绝（AC-6/D6） | 异常 | tempdir 内真实落一个普通文件 `<tmp>/data-file`，env = 该路径 → `Err(RootError::NotDirectory { raw })`；对照：同目录下真实子目录 `<tmp>/data-dir` → `Ok` | 表驱动（2 形态） |
| `resolve_dev_root` — 五项顺序（AC-6/D6） | 边界 | 同时命中多项时按 ①→②→③→④→⑤ 取首个：`Some("  ")`（空白且相对）→ `Blank`；`Some(".")`（相对，归一后等于 cwd）→ `Relative`（不落 `EqualsProd`）；`<tmp>/存在的文件` 且为相对形态 → `Relative`（②先于⑤） | 表驱动（3 形态） |
| `resolve_dev_root` — 归一化不依赖存在性（AC-7/D5） | 边界 | env = `<base>/x/./y/../z/`（目标不存在）→ `Ok`；与 `<base>/x/z` 的 `Ok.path` 相等；全程不产生 `NotFound`（不调 `canonicalize`） | 表驱动（2 形态） |
| `resolve_dev_root` — 归一吃 `.` / `..` / 重复 / 尾分隔符（AC-7/D5） | 边界 | `<base>/x/./z`、`<base>/x/y/../z`、`<base>//x///z`、`<base>/x/z/` 四形态 → 四者 `Ok.path` 两两相等；`ParentDir` 无可弹段时原样保留（绝对路径首段即 `..` → 不 panic，红线判定照常生效） | 表驱动（4 形态） |
| `resolve_dev_root` — 大小写等价不绕过红线（AC-7/D5） | 边界 | `#[cfg(windows)]`：`<home>/.DEV-TEAM` → `EqualsProd`；盘符与用户名家目录的大小写变形祖先 → `AncestorOfProd`；`#[cfg(not(windows))]`：同一形态视为不同路径 → `Ok`（大小写敏感） | 表驱动（2 形态） |
| `resolve_dev_root` — `RootError.raw` 逐 variant 回显（AC-6/D6） | 边界 | 五个 variant 各以带前后空白 / 大小写变形 / 尾分隔符 / 中文与 emoji / > 1000 字符的原值触发 → `raw` 与传入值逐字节相等（渲染侧不二次取 env） | 遍历枚举（5 variant） |
| `resolve_dev_root` — 非 UTF-8 路径值（AC-6） | 边界 | `#[cfg(unix)]` 以 `OsStr::from_bytes` 构造非法 UTF-8 值 → 不 panic，`Err` 的 `raw` 为 lossy 字符串（不中断判定） | 单值 |
| `resolve_root(home, false)` — prod 侧不读 env（AC-3/AC-5/D4） | 正向 | 进程内设 `DATA_ROOT_ENV` = 合法独立目录，调 `resolve_root(home, false)` → `Ok` 且 `source == Prod`、`path == prod_root(home).path`（env 零影响；与 `prod_root(home)` 逐字段相等） | 单值 |
| `resolve_root(home, true)` — dev 分支透传 env（AC-5/D4） | 正向 | 清空 `DATA_ROOT_ENV` → `DevDefault`；设为合法独立目录 → `DevEnv` 且路径归一；设为 prod 根 → `Err(EqualsProd)`（三分逐档断言） | 表驱动（3 形态） |
| `resolve_root` — 全局环境变量的存在性双档（AC-5/AC-9） | 边界 | 变量**未设**（`remove_var`）与变量设为**空串**（`set_var("")`）两档：前者 `DevDefault`，后者 `Err(Blank)`——区分「未设」与「设了空值」（`var_os` 的 `Some("")` 语义） | 表驱动（2 形态） |
| `resolve_root` — 组合锚：三分优先级全链（AC-5/AC-11/D4/D12） | 正向 | 三档组合逐档断言 `(is_dev, env) → (path, source.wire())`：`(false, 任意)` → `<home>/.dev-team` / `"prod"`；`(true, None)` → `<home>/.dev-team/dev` / `"dev-default"`；`(true, Some(合法独立目录))` → 归一路径 / `"env"`（`resolve_root` → `prod_root` / `resolve_dev_root` → `wire()` 跨函数组合，挂靠发起方 `env`） | 表驱动（3 档） |
| `resolve_root` — 组合锚：两模式数据面互不可见（AC-5/AC-8/AC-10） | 正向 | 同一 `home` 下：`prod_root(home).path != resolve_dev_root(home, None)?.path` 且后者是前者的直接子目录；`<根>/GLOBAL_DB_FILE_NAME` 与 `<根>/WORKSPACES_DIR_NAME` 在两模式下拼出不同绝对路径（同名文件、不同父目录 ⇒ redb 锁与 `workspaces/` 天然隔离）——与 `store` 两常量组合（挂靠发起方 `env`） | 表驱动（2 模式） |
| `render_root_error` — 三要素齐备（AC-6/D7） | 正向 | 逐 variant：文本含 `DATA_ROOT_ENV` 字面量、含 `raw` 原值（逐字）、含可照抄的绝对路径示例（默认根由 `home` 派生或独立目录示例，含分隔符且为绝对形态） | 遍历枚举（5 variant） |
| `render_root_error` — 不退化、不泄露枚举名（AC-6/D7） | 异常 | 五段文案均不含 variant 标识符（`Blank` / `Relative` / `EqualsProd` / `AncestorOfProd` / `NotDirectory`）、不含裸错误码、含自然语言句子（句读标点在场）；各 variant 文案两两不等（不共用同一段兜底串） | 遍历枚举（5 variant） |
| `render_root_error` — home 边界（AC-6/AC-15） | 边界 | `home` 为空 Path / 超长含中文与 emoji / 带尾分隔符 → 不 panic；默认根示例仍为绝对形态（`home` 为空时退化为不以相对根呈现） | 表驱动（3 形态） |
| `render_root_error` — 同文案复用（AC-8/AC-11） | 正向 | 同一 `(err, home)` 调两次文案逐字相等（纯函数无状态）；`append_boot_error(home, &render_root_error(&err, home))` 落盘内容含该文案全文（GUI 与自检「只差往哪写」由同一函数承接） | 单值 |
| `append_boot_error` — 落点与自建父目录（AC-8/D8） | 正向 | 空 tempdir `home`（`<home>/.dev-team/logs` 不存在）→ 调用后 `home/.dev-team/logs/boot-error.log` 存在，内容含 message 全文，行首匹配 `^\[\d{13}\] ` | 单值 |
| `append_boot_error` — append 不覆盖（AC-8/D8） | 正向 | 连续两次以不同 message 调用 → 文件两行、内容序为调用序、首次内容仍在；第三次仍追加（不清旧） | 单值 |
| `append_boot_error` — 与引擎日志分文件（AC-8/D8） | 异常 | 同 `home` 下预置 `<home>/.dev-team/logs/engine.log` 与 `<home>/.dev-team/dev/logs/engine.log`（真实字节）→ 调用后两文件字节完全不变，仅 `boot-error.log` 变化（MUST NOT 混写） | 单值 |
| `append_boot_error` — IO 失败静默（AC-8/D8） | 异常 | ①`<home>/.dev-team` 位置已被普通文件占据（父目录无法建）②`<home>/.dev-team/logs/boot-error.log` 位置为目录 → 两次调用均不 panic、返回 `()`（MUST NOT 掩盖原始失败原因、不抛错） | 表驱动（2 形态） |
| `append_boot_error` — 文案边界（AC-8） | 边界 | message 为 `""` / 超长 > 1000 / 含 `\n` 与 emoji → 不 panic，文件可读（`\n` 原样落盘，不因文案导致 panic） | 表驱动（3 形态） |
| `append_ignored_env` — 行文本逐字（AC-9/D8） | 正向 | 空 tempdir `home` → `home/.dev-team/logs/ignored-env.log` 存在，末行内容 == `ignored_env=` + `DATA_ROOT_ENV` + ` (release build ignores it)`（由常量复算，测试侧不另写字面量），行首 `^\[\d{13}\] ` | 单值 |
| `append_ignored_env` — append 幂等与目录自建（AC-9/D8） | 正向 | 连续两次调用 → 两行、内容相同、`logs/` 自建；不影响同目录其它文件 | 单值 |
| `append_ignored_env` — 锚 prod 根而非数据根（AC-9/D8） | 边界 | `home` 下同时存在 `<home>/.dev-team/dev/logs/`（dev 数据根）→ 调用只写 `<home>/.dev-team/logs/ignored-env.log`，dev 根零写入（日志不是数据，恒锚 prod 根） | 单值 |
| `append_ignored_env` — IO 失败静默（AC-9/D8） | 异常 | `<home>/.dev-team` 被普通文件占据 → 不 panic、返回 `()`；不阻断调用方（prod 启动不因留痕失败中断） | 单值 |
| `RootSource::wire()` — 三值遍历（AC-5/AC-14/D12） | 正向 | `Prod → "prod"`、`DevDefault → "dev-default"`、`DevEnv → "env"` 逐值断言；三值两两不等、均非空小写 kebab（线面形态） | 遍历枚举（3 值） |
| `RootSource::wire()` — 线值形态守卫（AC-5/D12） | 边界 | 三值均匹配 `^[a-z]+(-[a-z]+)*$`（无非 ASCII、无空格、无下划线、无前后 `-`）；`Prod.wire()` 与 `env::mode()` 的 prod 字面量相等（两处线值同形）；遍历序与枚举声明序一致（`Prod` → `DevDefault` → `DevEnv`） | 遍历枚举（3 值） |
| `RootSource` / `DataRoot` — 类型面（类型定义表） | 边界 | `RootSource` 为 `Copy`：拷贝后与原值相等；`DataRoot` `clone()` 后 `PartialEq` 相等、自反；`Debug` 渲染含 `path` 与 `source`（可诊断，不吞字段） | 表驱动（3 形态） |
| `DataRoot` — 管理态单值承载（AC-15） | 正向 | `DataRoot { path, source }` 字段公开可读；`clone()` 后管理态与 `setup_hook` / `selfcheck` 两处读到同一值（「数据根只算一次」的类型面：同一 `DataRoot` 实例可被多次引用而不产生第二份解析） | 单值 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（判定与渲染面） | 零 mock：`resolve_dev_root` / `resolve_root(home, is_dev)` 以纯函数入参驱动（`home: &Path` + `Option<&OsStr>` 显式传值），不读进程 env、不查盘（仅 ⑤ 校验读 `is_file`） | 三分优先级 / 五项拒绝 / 归一化 / 渲染 / `wire` 全部用例 |
| 进程环境变量（运行环境 / 全局变量） | 不 mock 函数：`std::env::set_var` / `remove_var` + 模块内进程级 `Mutex` 串行（`commands` 组 `TEST_PATH_LOCK` 同式），`Drop` 守卫恢复原值（存旧值 → 复原 / 未设则 `remove_var`） | `resolve_root` 的「prod 侧不读 env」「dev 分支透传 env」「存在性双档」三组用例 + `mode()` / `prod_root()` 的「外部变量零污染」两组用例（五组共用同一 `Mutex`，窗口期最短化：进入即改、断言即复原） |
| 文件系统（进程边界：数据文件 / 目录） | 不 mock：`tempfile::tempdir()` 真实目录承载 `home`、真实普通文件（⑤ 校验与 IO 失败臂）、真实 `engine.log` 字节（分文件断言），RAII 清理；不引入虚拟 FS | 五项校验 ⑤ / 日志双写 / 分文件 / IO 静默全部用例 |
| 系统时钟（`now_millis`） | 不 mock（design 未注入时钟，注入属投机抽象）：只断言行首 `^\[\d{13}\] ` 形态与「两次调用行序 == 调用序」，不断言毫秒具体值 | 全部日志落盘用例 |
| 私有项 `normalize` / `compare_key` | 不直测、不为测试外扩：行为经 `resolve_dev_root` 的归一等价与大小写用例观察（仓库「不为测试增加导出」规则） | 归一化 / 大小写等价用例 |

---

### packages/desktop/src-tauri/src/selfcheck.rs -> packages/desktop/src-tauri/src/selfcheck_test.rs

<!-- design.md「公共函数 / API」子表 5 行（`SELF_CHECK_ARG` / `PROD_REJECT_MESSAGE` / `render` / `exit_code` / `run`）与「类型定义」子表 2 行（`Probe` / `Summary`）落本文件。测试模块以 `#[cfg(test)] mod selfcheck_test;` 在 `selfcheck.rs` 末行声明（tasks 阶段三已建空骨架）。proposal「变更范围 - 测试文件」明示「`render` / `exit_code` 纯函数；`run` 的 IO 部分不在射程」。**`run` 的签名是具体类型 `&tauri::App`（= `App<Wry>`）**，而仓库既有 `tauri::test::mock_app()` 产出 `App<MockRuntime>`，二者不可互相代入（把签名泛化为 `&App<R: Runtime>` 属改 design 契约，本相位不做）⇒ `run` 无单测覆盖，其可断言语义全部下沉到委托面：十键渲染与 `error=` 行由 `render` 逐字钉住、退出码 0/1 由 `exit_code` 钉住，进程级效果（零窗口 / flush / 退出码 2）见 `## 不可测试项` 第 9 项。私有项（`probe` / `summary_of`）不入 `#### 待测功能`。 -->

#### 待测功能

- `SELF_CHECK_ARG: &str = "--self-check"`（公共函数表，AC-11/AC-12）：参数名单点（`main` 的 args 判定引用）
- `PROD_REJECT_MESSAGE: &str`（公共函数表，AC-12/D3）：prod 侧拒绝文案单点，逐字 `--self-check 仅在 dev 构建可用（当前为生产构建）。`
- `selfcheck::render(summary: &Summary) -> String`（公共函数表，AC-11/D9）：纯函数——十键固定键序 + 可选 `error=` 行、末尾单换行；未知值（`None`）渲染为空串
- `selfcheck::exit_code(probe: &Probe) -> i32`（公共函数表，AC-11/D9）：`error.is_none()` → `0`，否则 `1`（`2` 由 `main` 产出：env 非法 / prod 拒自检）
- `selfcheck::run(app: &tauri::App, data_root: &DataRoot) -> i32`（公共函数表，AC-11/D10）：探库 → 摘要 → `print!` → `stdout().flush()` → 返回退出码（`&App` 仅用于 `home=` 行）
- 类型 `Probe` / `Summary`（类型定义表，AC-11/D9）：`Probe { global_db_open, workspace_count: Option<u64>, models: Option<Vec<String>>, workspaces_dir_exists, error: Option<String> }`；`Summary { mode, home, data_root, source, global_db, global_db_open, workspaces_dir, workspaces_dir_exists, workspace_count, models, error }`——字段序即渲染键序

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `SELF_CHECK_ARG` — 字面量单点（AC-11/AC-12） | 正向 | 逐字等于 `"--self-check"`（双横线、小写 kebab、无前后空白）；以 `args().skip(1)` 语义比较时与传入参数串相等 | 单值 |
| `SELF_CHECK_ARG` — 参数匹配形态（AC-11） | 边界 | 以 `--` 起手、不含 `=`、不含空白 ⇒ 与 `main` 的「`args().skip(1)` 逐项与常量精确相等」比较同形；`--self-check --root <path>` 组合时只匹配首参数（`--root` 被当未知参数忽略，本期不支持）；`--self-checkx` / `--SELF-CHECK` 不匹配（非前缀匹配） | 表驱动（3 形态） |
| `PROD_REJECT_MESSAGE` — 逐字口径（AC-12/D3） | 正向 | 全文等于 `--self-check 仅在 dev 构建可用（当前为生产构建）。`（含全角括号与句末句号）；含 `--self-check` 与「生产构建」两要素；非空且无换行（单行 stderr） | 单值 |
| `PROD_REJECT_MESSAGE` — 单行与可读性（AC-12） | 边界 | 无 ANSI 转义（`[` 零命中）、无首尾空白、长度 < 200 字符（单行 stderr 不被截断）；`eprintln!` 单次输出即完整说明（不拼接第二句） | 单值 |
| `render` — 十键键序固定（AC-11/D9） | 正向 | 全字段已知的 `Summary` → 行序逐字为 `mode` / `home` / `data_root` / `data_root_source` / `global_db` / `global_db_open` / `workspaces_dir` / `workspaces_dir_exists` / `workspace_count` / `models`；每行 `key=value` 单行、无横幅 / 无 ANSI 转义（`\x1b[` 零命中）/ 无空行；文本以**单个** `\n` 结尾 | 单值 |
| `render` — `mode` 两值透传（AC-2/AC-11） | 正向 | `summary.mode = "dev"` → `mode=dev`；`"prod"` → `mode=prod`（与 `env::mode()` 同形，单点产出） | 表驱动（2 形态） |
| `render` — `source` 三值透传（AC-5/AC-11/D12） | 正向 | `summary.source` 取 `"prod"` / `"dev-default"` / `"env"` → `data_root_source=` 行逐字；三行值两两不等 | 遍历枚举（3 值） |
| `render` — `global_db_open` / `workspaces_dir_exists` 双向（AC-11） | 正向 | `global_db_open: true` → `global_db_open=ok`、`false` → `=err`；`workspaces_dir_exists: true` → `=true`、`false` → `=false`（两 bool 各两向必断言） | 表驱动（4 形态） |
| `render` — 未知值与空清单（AC-11/D9） | 边界 | `workspace_count: None` → `workspace_count=`（空值，行仍在）；`models: None` → `models=`；`Some(0)` → `workspace_count=0`；`Some(vec![])` → `models=`（空清单，非 `None` 同形）；`Some(vec!["workspace"])` → `models=workspace` | 表驱动（5 形态） |
| `render` — `models` 逗号连接（AC-11，校准 1） | 正向 | `Some(vec!["workspace", "agent_provider", "agent_instance"])` → `models=workspace,agent_provider,agent_instance`（无空格、序不变、与 `list_models()` 注册表短名同形）；1000 项大列表 → 不 panic 且键序不变 | 表驱动（2 形态） |
| `render` — `error` 行追加在末（AC-11/D9） | 异常 | `error: Some("数据根不可打开")` → 第 11 行 `error=数据根不可打开`，仍在末尾、仍无装饰行；`error: None` → 恰 10 行（`error=` 零命中） | 表驱动（2 形态） |
| `render` — 路径成分取自 store 常量（AC-11） | 正向 | `global_db` 行值 == `data_root.join(GLOBAL_DB_FILE_NAME)` 的 lossy 串；`workspaces_dir` 行值 == `data_root.join(WORKSPACES_DIR_NAME)` 的 lossy 串（测试侧以 store 常量复算，壳层零字面量）；`home` / `data_root` 行为入参原样 | 单值 |
| `render` — 特殊字符与超长值（AC-11） | 边界 | `error` / `models` 元素含中文、emoji、`\0`、`=`、`\n`、> 1000 字符 → 不 panic；首键仍为 `mode`、末行为 `models`（`error=None`）或 `error=`（`error=Some`），不因值内 `\n` 抛错 | 参数化（5 形态） |
| `exit_code` — 全通 0（AC-11/D9） | 正向 | `Probe { error: None, global_db_open: true, workspaces_dir_exists: false, workspace_count: Some(0), models: Some(vec![…]) }` → `0`（`workspaces_dir_exists=false` 是常态，不参与退出码） | 单值 |
| `exit_code` — 失败 1（AC-11/D9） | 异常 | `error: Some("redb: 文件损坏")` → `1`；与其余字段取值无关（`global_db_open` 为 `true` / `false` 两档均 `1`） | 表驱动（2 形态） |
| `exit_code` — 值域与错误串形态（AC-11/AC-12） | 边界 | 恒不返回 `2`（`2` 由 `main` 产出，不在本函数值域）；`error: Some("")`（空串）→ `1`（`Some` 即失败，不看内容）；超长 / 含 emoji / 含 `\n` → 均 `1` 且不 panic | 表驱动（4 形态） |
| `Probe` / `Summary` — 字段序即键序（类型定义表，AC-11） | 边界 | 以 struct 字面量按声明序构造 → `render` 行序与字段序一一对应（键序由类型形状保证，非手写常量表）；`Probe` 全 `None` / 全 `Some` 两档均可构造且参与 `exit_code`（未知字段不必填） | 表驱动（2 形态） |
| `run` — 返回码语义经委托面覆盖（AC-11/D9/D10） | 正向 | `run` 的返回码恒由 `exit_code(&probe)` 产出（探库成功 → `0`、首错即停 → `1`），故两向语义由本表 `exit_code` 两条用例钉住；`run` 自身的 `&App<Wry>` 入参无法在单测内构造（见上方章节注释与 `## 不可测试项` 第 9 项） | 单值 |
| `run` — 首错即停的 `error=` 面经 `render` 覆盖（AC-11/D9） | 异常 | 库不可打开时 `error: Some(…)` → 第 11 行 `error=` 且在末位（`render` 用例）；`global_db_open` 与 `error` 的并存形态由 `exit_code` 的「与其余字段取值无关」用例钉住 | 单值 |
| `run` — 真开库副作用与 `workspaces/` 缺席（AC-11，校准 4） | 边界 | `WorkspaceStores::open` 对不存在的数据根会建出 `<数据根>/GLOBAL_DB_FILE_NAME`（与 GUI 首次启动同源、不做 dry-run），`<数据根>/WORKSPACES_DIR_NAME` 在只开全局库时通常仍不存在（`workspaces_dir_exists=false` 常态）——两事实由 `store_test.rs` 的打开落位用例与 `render` 的 `workspaces_dir_exists` 双向用例承接，`run` 不重复断言 | 单值 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 无（渲染与退出码面） | 零 mock：`render` / `exit_code` 以 `Summary` / `Probe` 字面量直构驱动（被测 API 显式入参），纯内存、零 IO、零时钟 | 键序 / 透传 / 未知值 / error 行 / 退出码全部用例 |
| Tauri 运行环境 | **不引入**（`run` 不在单测射程）：`mock_app()` 产出 `App<MockRuntime>`、`run` 要 `&App<Wry>`，不可代入；为测试泛化签名属改 design 契约。进程级行为属实跑面（`## 不可测试项` 第 9 项） | —（无应用例外） |
| 全局库（数据文件 / DB） | 不 mock、也不在 `run` 用例内触碰：真开库落位由 `store_test.rs` 承接，自检侧只经 `render` 的 `global_db` / `global_db_open` 值面断言 | `render` 路径成分与 bool 双向用例 |
| stdout（标准输出） | 不捕获、不 mock：`print!` / `flush` 的进程级效果属管道实跑面（`## 不可测试项` 第 9 项）；输出内容全由 `render` 纯函数逐字钉住 | 全部 `render` 用例 |

---

### packages/desktop/src-tauri/src/commands/env/mod.rs -> packages/desktop/src-tauri/src/commands/env/mod_test.rs

<!-- design.md「公共函数 / API」子表 2 行（`app_environment` / `app_environment_inner`）与「类型定义」子表 1 行（`AppEnvironment`）落本文件；「路由 / API 设计」IPC 行给出线面 `{ mode, dataRoot, source }`。测试模块以 `#[cfg(test)] mod mod_test;` 在模块末行声明（tasks 阶段四已建空骨架）。沿 `commands/config/mod_test.rs` 先例：**不启动 Tauri runtime** 直调 `*_inner` 纯函数；薄包装的 State 面以 `tauri::test::mock_app()` + `manage(DataRoot)` 装置取值（`commands/{agents,db}` 先例），断言焦点是「命令层零加工」。 -->

#### 待测功能

- `app_environment(data_root: State<'_, DataRoot>) -> AppEnvironment`（公共函数表，AC-1/AC-14/D12）：第十一轨薄包装（`#[tauri::command]` + `#[specta::specta]`）——无 IO、无 `Result` 错误面（管理态恒在），转调 `app_environment_inner(&data_root)`
- `app_environment_inner(data_root: &DataRoot) -> AppEnvironment`（公共函数表，AC-1/AC-14/D12）：领域平移纯函数（离运行时可测）——`mode` 取 `crate::env::mode()`、`data_root` 取 `path` 的 lossy 串、`source` 取 `RootSource::wire()`，命令层零二次加工
- 类型 `AppEnvironment { mode: String, data_root: String, source: String }`（类型定义表，AC-14/D12）：`Debug, Clone, Serialize, Type` + `#[serde(rename_all = "camelCase")]` ⇒ 线面 `{ mode, dataRoot, source }`

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `app_environment_inner` — 三字段平移（AC-1/AC-14） | 正向 | `DataRoot { path: <tempdir>, source: DevEnv }` → `data_root == path` 的 lossy 串逐字、`source == "env"`、`mode == crate::env::mode()`；三字段无空值 | 单值 |
| `app_environment_inner` — `source` 三值遍历（AC-5/AC-14/D12） | 正向 | `Prod` / `DevDefault` / `DevEnv` → `"prod"` / `"dev-default"` / `"env"`，与 `RootSource::wire()` 逐字相等（单点，非命令层另拼） | 遍历枚举（3 值） |
| `app_environment_inner` — `mode` 与 `env::mode()` 同源（AC-1/D12） | 正向 | 返回值 `mode` == `crate::env::mode()`；`mode` 值域闭集 `{"dev", "prod"}`（与自检摘要 `mode=` 同形） | 单值 |
| `app_environment_inner` — 无 `Err` 通道（异常类别不适用）（AC-1/D12） | 异常 | 签名为 `-> AppEnvironment`（非 `Result`，管理态恒在）：以任意 `DataRoot`（空 path / 三值 source）直调均返回三字段完整的结构、不 panic、不返回错误态 | 表驱动（2 形态） |
| `app_environment_inner` — 路径边界（AC-14） | 边界 | `path` 为 `PathBuf::new()`（空）→ `data_root == ""`（不 panic）；超长含中文与 emoji → 逐字透传；含尾分隔符 / 空格 / `..` 段 → 原样透传（平移语义，不归一、不查盘）；`#[cfg(unix)]` 非 UTF-8 路径 → lossy 不 panic | 表驱动（4 形态） |
| `app_environment` — 薄包装与 inner 逐字段一致（AC-1/D12） | 正向 | `mock_app()` + `app.manage(data_root.clone())` → 直调 `app_environment(app.state::<DataRoot>())` 与 `app_environment_inner(&data_root)` 三字段全等（命令层零加工） | 单值 |
| `app_environment` — 不依赖其它管理态（AC-1/AC-14） | 异常 | `mock_app()` 未 `manage` 任何其它注册表（无 `WorkspaceStores` / `WatchRegistry`）→ 仍可取值成功（纯管理态读、无 IO、无 `Result` 错误面）；`State` 缺 `DataRoot` 的形态不构成生产可达路径，不作断言 | 单值 |
| `AppEnvironment` — 线面 camelCase 形态（AC-14/D12） | 正向 | `serde_json::to_value` → 恰三个键 `mode` / `dataRoot` / `source`（`data_root` 蛇形键零命中、无多余字段、无嵌套对象） | 单值 |
| `AppEnvironment` — 类型面（类型定义表） | 边界 | `clone()` 后三字段逐字段相等；`Debug` 渲染含三字段名；三字段类型均为 `String`（线值域由 `env` 单点产出，命令层不复算 / 不收窄） | 表驱动（2 形态） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| Tauri 运行环境（运行环境边界） | `tauri::test::mock_app()`（MockRuntime，零窗口零事件循环）+ `app.manage(DataRoot)`；`State` 经 `app.state::<DataRoot>()` 直取（`commands/{agents,db}` 先例装置，非被测对象 mock） | `app_environment` 薄包装两组用例 |
| 无（平移面） | 零 mock：`app_environment_inner` 以 `DataRoot` 字面量直构入参，纯内存、零 IO、零时钟 | 全部 `*_inner` 用例 |
| 无（序列化面） | 不 mock `serde_json`：真实序列化后按键集合断言（camelCase 由 derive 属性保证，不另造序列化器） | 线面形态用例 |

---

### packages/desktop/src-tauri/src/commands/mod.rs -> packages/desktop/src-tauri/src/bindings/mod_test.rs

<!-- design.md「公共函数 / API」子表 1 行（`all_commands!` 命令清单，所在文件 `src-tauri/src/commands/mod.rs`）+「修改文件」表本行（`pub mod env;` + `all_commands!` 末尾追加 `$crate::commands::env::app_environment,`）。落点取**既有** `src-tauri/src/bindings/mod_test.rs`：该文件已维护 `COMMAND_NAMES`（37 条 snake_case）/ `COMMAND_WRAPPERS`（37 条 camelCase）/ `DTO_TYPES` 三张清单并驱动 `export_bindings()` 幂等导出与产物字节比对（`authoritative_snapshot`），是「登记面」的唯一自动化断言点；不新建 `commands/mod_test.rs`（design / tasks 均未声明，避免为测试新增模块）。 -->

#### 待测功能

- `all_commands!` 命令清单（公共函数表，AC-14/AC-16）：`macro_rules!` 末尾追加 `$crate::commands::env::app_environment,`——同一清单同时被 `main` 的 `generate_handler` 与 specta builder（`bindings::builder`）消费

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 登记面 — 新命令进两份清单（AC-14/AC-16） | 正向 | `COMMAND_NAMES` 含 `"app_environment"`、`COMMAND_WRAPPERS` 含 `"appEnvironment"`；既有 37 条断言逐条仍在（contains 语义不红）；`DTO_TYPES` 含 `"AppEnvironment"` | 单值 |
| 登记面 — 生成物含新包装与 DTO（AC-14/AC-16） | 正向 | `export_bindings()` 产物文本含 `appEnvironment` 包装与 `AppEnvironment` 类型；`mode` / `dataRoot` / `source` 三字段在场（线面 camelCase） | 单值 |
| 登记面 — 幂等与零 diff（AC-16） | 异常 | 连续两次 `export_bindings()` → 磁盘产物字节完全相等（`authoritative_snapshot` 同式比对）；缺登记 / 误改时该字节比对必红（`bindings:check` 的单测等价面） | 单值 |
| 登记面 — 清单与 builder 同源（AC-16） | 边界 | `all_commands!(collect_commands)` 组装的 builder 命令数 == 清单条数、新命令仅在末尾出现一次（无重复登记、无遗漏）；`generate_handler` 与 specta 消费同一宏 ⇒ 名字两处同名同形 | 遍历枚举 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 生成物文件（数据文件边界） | 不 mock：`export_bindings()` 直接写 `src/types/generated/bindings.ts`（`CARGO_MANIFEST_DIR` 定位，既有装置），文件互斥沿用 `bindings/mod_test.rs` 既有 `file_lock()`；变更类用例挂 `RestoreOnDrop` 恢复权威产物 | 全部用例 |
| specta / tauri-specta | 不 mock：真实 builder 组装与真实 TypeScript 导出（被测对象本身，mock 即失去意义） | 全部用例 |

---

### packages/desktop/src-tauri/crates/infra/store/src/store.rs -> packages/desktop/src-tauri/crates/infra/store/src/store_test.rs

<!-- design.md「公共函数 / API」子表 2 行（`GLOBAL_DB_FILE_NAME` / `WORKSPACES_DIR_NAME`，`pub(crate)` → `pub`）落本文件。测试模块既有（`crates/infra/store/src/lib.rs` 已登记 `#[cfg(test)] mod store_test;`），不新建文件；断言沿用既有 `use crate::store::{workspace_db_file_name, GLOBAL_DB_FILE_NAME};` 路径与本变更新增的 crate 根重导出路径。零行为改动 ⇒ 用例只钉「值逐字 + 形态守卫 + 既有打开 / 寻址回归」，不重复 store 既有语义断言。 -->

#### 待测功能

- `GLOBAL_DB_FILE_NAME: &str = "desktop-global.redb"`（公共函数表，AC-5/AC-11/D14）：`pub(crate)` → `pub`；自检摘要 `global_db=` 的路径成分（壳层零字面量）
- `WORKSPACES_DIR_NAME: &str = "workspaces"`（公共函数表，AC-5/AC-11/D14）：`pub(crate)` → `pub`；自检摘要 `workspaces_dir=` 与 `WorkspaceStores.workspaces_dir` 的路径成分

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| 两常量 — 值逐字（AC-5/AC-11/D14） | 正向 | `GLOBAL_DB_FILE_NAME == "desktop-global.redb"`、`WORKSPACES_DIR_NAME == "workspaces"`；`crate::GLOBAL_DB_FILE_NAME`（crate 根重导出）与 `crate::store::GLOBAL_DB_FILE_NAME` 逐字相等（同名同值，无第二份定义） | 表驱动（2 常量） |
| 两常量 — 与真实打开落位一致（AC-5/D14） | 正向 | 空 tempdir 作 `data_root` → `WorkspaceStores::open(&data_root)` 后 `data_root.join(GLOBAL_DB_FILE_NAME)` 存在且为文件；再以真实 tempdir 子目录作 workspace 根调 `for_root` → 库文件落在 `data_root.join(WORKSPACES_DIR_NAME)` 子树内（`WorkspaceStores.workspaces_dir` 字段私有，测试侧以常量复算落位，不破坏封装） | 单值 |
| 两常量 — 形态守卫（AC-5） | 边界 | 两值均非空、无前后空白、不含路径分隔符（`/` `\\`）、不含 `..` 段、不含盘符 `:`、不含中文 / emoji——防后续误改造成路径逃逸或跨平台文件名非法 | 表驱动（6 断言） |
| 寻址语义零回归（AC-5/AC-16） | 异常 | `workspace_db_path(&data_root.join(WORKSPACES_DIR_NAME), "<canonical root>")`（`pub(crate)` 直调）仍产出 `<workspaces_dir>/<身份段>.redb`（文件名派生未受可见性变更影响）；同一 canonical root 在「prod 根」与「dev 子根」两 `data_root` 下产出**同名文件、不同父目录**（AC-5 隔离语义的 store 侧锚） | 表驱动（2 根） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| 文件系统 / redb 文件（数据文件边界） | 不 mock：`tempfile` 真实 tempdir + 真实 redb 文件（`WorkspaceStores::open` 真开库），RAII 清理（store_test 既有装置） | 落位与寻址回归用例 |
| 无 | 常量断言为纯值比较，零 mock、零 IO | 值逐字与形态守卫用例 |

---

### packages/desktop/src-tauri/crates/infra/store/src/lib.rs -> packages/desktop/src-tauri/crates/infra/store/src/lib_test.rs

<!-- design.md 未声明该文件的公共 API 变更（「公共函数 / API」子表两常量的「所在文件」记为 `store.rs`，本文件仅追加两名字进既有 `pub use store::{…}` 重导出）：无独立运行时行为，导出可用性由 `selfcheck_test.rs` / `env_test.rs` 经 `store::GLOBAL_DB_FILE_NAME` / `store::WORKSPACES_DIR_NAME` 的 crate 根引用在**编译期**覆盖（不能解析即编译失败）。本章节不设待测功能与用例，且不新建 `lib_test.rs`（crate 既有测试模块声明集中在 `lib.rs` 的 `mod {change_port,envelope,model,store}_test;`，无 `lib_test` 先例）。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更 -->

#### 用例

<!-- 无独立用例：见章节注释（重导出可用性是编译期属性，由下游 crate 根引用覆盖） -->

#### Mock策略

<!-- 无 -->

---

### packages/desktop/src/hooks/use-app-environment.ts -> packages/desktop/src/hooks/use-app-environment.test.ts

<!-- design.md「公共函数 / API」子表 1 行（`useAppEnvironment`）+「类型定义」子表 3 行（`BuildMode` / `DataRootSource` / 前端 `AppEnvironment`）落本文件。沿既有 hook 测试装置：`vi.hoisted` + `vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }))`（`use-workspaces.test.ts` / `views/agent/hooks/use-agent-options.test.ts` 先例——生成物 `commands.appEnvironment()` 内部走 `invoke`，mock 该进程边界即覆盖生成包装，不必 mock 生成物模块本身）+ `renderHook` 真实渲染。类型别名（`BuildMode` / `DataRootSource`）为编译期收窄，运行时无实体，只在载荷合法性与线值遍历用例中体现。 -->

#### 待测功能

- `useAppEnvironment(): AppEnvironment | null`（公共函数表，AC-14/D13）：挂载一次 `commands.appEnvironment()`；成功且载荷合法 → `{ mode, dataRoot, source }`；reject / 载荷不合法 / 未返回 → `null`（静默，无错误态、不阻塞其余取数）
- 前端类型 `BuildMode = 'dev' | 'prod'` / `DataRootSource = 'prod' | 'dev-default' | 'env'` / `AppEnvironment { mode, dataRoot, source }`（类型定义表，AC-14/D13）：编译期收窄用，线值由后端 `mode()` / `RootSource::wire()` 单点产出

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `useAppEnvironment` — 成功返回三字段（AC-14） | 正向 | `invoke('app_environment')` resolve `{ mode: 'dev', dataRoot: 'C:\\u\\.dev-team\\dev', source: 'dev-default' }` → 返回值三字段逐字相等（不加工、不改写大小写、不加尾分隔符） | 单值 |
| `useAppEnvironment` — 挂载恰一次（AC-14） | 正向 | 挂载 + 等待收敛后 `countOf('app_environment') === 1`；无第二次轮询 / 无额外命令调用 | 单值 |
| `useAppEnvironment` — `mode` 两值透传（AC-14） | 正向 | `'dev'` / `'prod'` → 原样；返回值 `mode` 值域闭集（非法值不出现） | 表驱动（2 形态） |
| `useAppEnvironment` — `source` 三值遍历（AC-14/D12） | 正向 | `'prod'` / `'dev-default'` / `'env'` → 原样透传（与 `RootSource::wire()` 值域一致） | 遍历枚举（3 值） |
| `useAppEnvironment` — reject 静默（AC-14/D13） | 异常 | `invoke` reject `Error('IPC 不可用')` → 返回 `null`；不 `console.error`、不抛、不产生错误态字段（返回值形态恒 `AppEnvironment` 或 `null`） | 单值 |
| `useAppEnvironment` — 载荷不合法一律 `null`（AC-14/D13） | 异常 | 非对象（`null` / `[]` / `'dev'` / `5` / `true`）/ 缺 `mode` / 缺 `dataRoot` / 缺 `source` / 字段类型错（`mode: 5`、`dataRoot: null`、`source: {}`）/ 非法枚举名义值（`mode: 'DEV'`、`mode: 'dev '`、`source: 'dev'`）→ 全 `null`，不抛 | 表驱动（14 形态） |
| `useAppEnvironment` — 未返回（pending）不判定（AC-14/D13） | 边界 | `invoke` 返回永不 settle 的 Promise → 初值 `null`，且无异常；后续 resolve 后回填三字段（未就绪 ≠ 错误） | 单值 |
| `useAppEnvironment` — 多余字段不破坏合法性（AC-14/D13） | 边界 | 载荷为 `{ mode: 'dev', dataRoot: 'C:\\d', source: 'env', extra: 1, nested: { a: 1 } }` → 返回 `{ mode, dataRoot, source }`（多余字段不透出、不报错） | 单值 |
| `useAppEnvironment` — 值边界字符（AC-14） | 边界 | `dataRoot` 为 `""`（空）/ 超长 > 1000 / 含中文与 emoji / 含 `\n` 与 `\\` → 原样透传、不 panic（空串是否合法由后端线值决定，前端不做路径校验） | 表驱动（4 形态） |
| `useAppEnvironment` — 卸载后 resolve（AC-14） | 边界 | 未收敛即 `unmount()`，随后 Promise resolve → 不产生 React 状态更新告警、不抛（cancelled 守卫） | 单值 |
| 类型面 — 编译期收窄（类型定义表） | 边界 | `mode` 收窄为 `BuildMode`、`source` 收窄为 `DataRootSource` 后 App 侧消费无需断言（`tsc` / `vp check` 门禁覆盖）；运行时不得以 `as` 断言绕过（`no-unsafe-type-assertion` 规则在场） | 单值 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| IPC（进程边界：接口 / 网络） | `vi.hoisted` + `vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }))`，按命令名分发（`app_environment` 可切 resolve / reject / pending / 非法载荷），沿 `use-workspaces.test.ts` / `use-agent-options.test.ts` 先例；生成物 `commands.appEnvironment()` 真实执行到该桩（不 mock 生成模块） | 全部用例 |
| React 运行时 | 不 mock：`renderHook` + `waitFor` 真实渲染与 effect 时序（`@testing-library/react`） | 全部用例 |
| 无 | 无数据文件 / 无 store / 无定时器 mock（hook 无 IO、无轮询） | 全部用例 |

---

### packages/desktop/src/hooks/use-updater.ts -> packages/desktop/src/hooks/use-updater.test.ts

<!-- design.md「公共函数 / API」子表 1 行（`useUpdater(mode: BuildMode | null)`）+「修改文件」表本行（`useStartupCheck` 拆成 `useStartupVersion` / `useStartupCheck(setAvailable, mode)`，`if (mode !== 'prod') return;`，删 `if (!import.meta.env.DEV)`，`start` / `performUpdate` / `briefOf` 零改动）。既有 `use-updater.test.ts` 为**改造**：删 `vi.stubEnv('DEV', …)` / `vi.unstubAllEnvs()` 与 `afterEach` 的 env 清理，全部 `renderHook(() => useUpdater())` 改带 `mode` 入参（`'prod'` 复用既有检查路径断言，`'dev'` / `null` 新增判据断言）。 -->

#### 待测功能

- `useUpdater(mode: BuildMode | null): UpdateState`（公共函数表，AC-14/D13）：启动更新检查判据 `mode === 'prod'`（`null` / `'dev'` 不判定，且不回落成「照常检查」）；版本号 effect 与更新检查 effect 拆分——版本号挂载一次拉取，`mode` 由 `null` 变 `'prod'` 时只补一次 `check`；下载 / 安装链路（`start` / `performUpdate` / `briefOf`）零变化

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| `useUpdater` — `mode='prod'` 照常检查（AC-14） | 正向 | `renderHook(() => useUpdater('prod'))` → `check` 恰一次、`getVersion` 恰一次、`currentVersion` 回填、`available` 按 `check` 结果填充（既有「无更新」「发现新版本」两例改带入参后全绿） | 表驱动（2 既有例） |
| `useUpdater` — `mode='dev'` 零检查但版本号照拉（AC-14/D13） | 正向 | `useUpdater('dev')` → `checkMock` 零调用、`getVersion` 一次、`currentVersion === '0.1.0'`；无错误态（替换既有 `vi.stubEnv('DEV', true)` 用例） | 单值 |
| `useUpdater` — `mode=null` 不判定（AC-14/D13） | 异常 | `useUpdater(null)` → `checkMock` 零调用（MUST NOT 回落成照常检查）、`available` / `error` / `status` 保持初值；`getVersion` 仍一次 | 单值 |
| `useUpdater` — 就绪后补一次检查（AC-14/D13） | 正向 | `useUpdater(null)` 挂载后 `rerender` 为 `'prod'` → `check` 恰一次（补检）、`getVersion` 仍总一次（版本号 effect 不重跑）；`available` 随 `check` 结果回填 | 单值 |
| `useUpdater` — 已检查后模式回退不重复（AC-14/D13） | 边界 | `'prod'` → `rerender` 为 `'dev'` → `check` 调用数不增（不回退撤销、不重复触发）；`'dev'` → `'prod'` → `'prod'`（同值重渲染）→ 仍一次 | 表驱动（2 形态） |
| `useUpdater` — 检查失败静默（AC-14） | 异常 | `mode='prod'` + `checkMock` reject `Error('network down')` → `error` 为 `null`、`available` 为 `null`、`status === 'idle'`（既有「启动检查失败静默」例改入参） | 单值 |
| `useUpdater` — 版本号失败不阻断检查（AC-14） | 异常 | `mode='prod'` + `getVersionMock` reject → `currentVersion` 保持 `null`、`check` 仍被调用、`error` 为 `null`（既有「getVersion 失败」例改入参） | 单值 |
| `useUpdater` — 下载 / 安装链路零变化（AC-14） | 正向 | `mode='prod'` 下既有 5 例（`Started` → `Progress×n` → `Finished` 百分比、`start` 失败置 `error` 并重取、更新被撤下回 `idle`、busy 守卫、`close` 释放）逐条断言不变 | 表驱动（4 既有例） |
| `useUpdater` — 初值面（`UpdateState` 边界） | 边界 | `mode=null` 且 `getVersion` reject → `{ currentVersion: null, available: null, status: 'idle', progress: null, error: null }` 全初值、无异常冒出 | 单值 |
| `useUpdater` — 判据为严格相等（AC-14/D13） | 边界 | 判据实现为 `mode === 'prod'`（大小写敏感、无真值判断）：`BuildMode` 收窄后非法值编译期不可表达；运行时 `'PROD'` / `''` 形态由类型面排除，测试侧不以 `as` 断言构造（`no-unsafe-type-assertion` 门禁） | 单值 |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| IPC：版本号（进程边界：接口） | `vi.mock('@tauri-apps/api/app', () => ({ getVersion: getVersionMock }))`（既有装置原样保留） | 全部用例 |
| IPC：更新检查（进程边界：网络 / 更新端点） | `vi.mock('@tauri-apps/plugin-updater', () => ({ check: checkMock }))` + `fakeUpdate()` 替身（`version` / `body` / `close` / `downloadAndInstall`），既有 `emitEvents` 装置承载进度事件 | 全部用例 |
| 前端构建信号（已删除的判据） | **不再 mock**：`vi.stubEnv('DEV', …)` 与 `vi.unstubAllEnvs()` 整组删除（`import.meta.env.DEV` 判据移除，AC-1 的测试侧配套） | 改造点，不属用例 |
| React 运行时 | 不 mock：`renderHook` / `rerender` / `waitFor` / `act` 真实时序 | 全部用例 |
| 定时器 | 不 mock：hook 无定时器（进度由事件驱动） | — |

---

### packages/desktop/src/app.tsx -> packages/desktop/src/app.test.tsx

<!-- design.md 未声明该文件的公共 API 变更（「公共函数 / API」子表无本文件行）：改动为壳层组合——`useAppEnvironment()` 挂载、`useUpdater(environment?.mode ?? null)` 透传、`ShellHeader` 增 `environment` 入参与 `mode === 'dev'` 时的 `DEV` 徽标（`data-testid="dev-badge"`、`title = dataRoot`），版本号显示链路零改动（不加 `-dev`）。本章节承载 AC-14 的 UI 面与既有装置改造（proposal 明示 `app.test.tsx` 有三处严格 invoke 序列断言需改 `filter` 口径：`every(...)` @346/419、`toEqual([...])` @375）。既有 mock 兜底分支（`return Promise.resolve(null)`）使未显式打桩的用例自动落入「mode 未知」路径（R-7：改造面收敛）。 -->

#### 待测功能

<!-- design.md 未声明该文件的公共 API 变更（壳层组合层：hook 挂载 + 入参透传 + 渲染分支） -->

#### 用例

| 测试对象 | 路径类型 | 测试条件 | 迭代类型 |
|---|---|---|---|
| App 壳 — 挂载期新增一次 `app_environment`（AC-14） | 正向 | `render(<App />)` 并等待收敛 → `countOf('app_environment') === 1`（不随视图切换 / 刷新重发） | 单值 |
| App 壳 — `mode='dev'` 呈 `DEV` 徽标（AC-14/D13） | 正向 | `app_environment` resolve `{ mode: 'dev', dataRoot: 'C:\\u\\.dev-team\\dev', source: 'dev-default' }` → `screen.getByTestId('dev-badge')` 存在、文本为 `DEV`、`title` 逐字等于 `dataRoot` | 单值 |
| App 壳 — `mode='prod'` 无徽标（AC-14/D13） | 正向 | resolve `{ mode: 'prod', dataRoot: 'C:\\u\\.dev-team', source: 'prod' }` → `queryByTestId('dev-badge') === null`；更新指示链路照常（`check` 一次、无新版本时不渲染按钮） | 单值 |
| App 壳 — mode 未就绪 / IPC 失败静默（AC-14/D13） | 异常 | ①`app_environment` reject ②永不 settle（pending）③resolve `null` → 三形态均无徽标、无错误态（无 toast）、其余取数不受阻（列表 / 详情照常渲染） | 表驱动（3 形态） |
| App 壳 — 载荷不合法不污染壳态（AC-14/D13） | 边界 | resolve `{ mode: 'other', dataRoot: 5 }`（非法枚举值 + 类型错）→ 无徽标、`useUpdater(null)` 不判定（`check` 零调用）、无异常冒出 | 单值 |
| App 壳 — mode 透传到 updater（AC-14/D13） | 正向 | `mode='dev'` → `checkMock` 零调用、`getVersion` 仍一次（版本号显示链路零改动，无 `-dev` 后缀）；`mode='prod'` → `check` 一次且发现新版本时渲染「更新到 vX」 | 表驱动（2 形态） |
| App 壳 — 既有序列断言改 `filter` 口径（AC-14/R-7） | 边界 | 三处断言改造后语义不变：`invokeMock.mock.calls.filter(([n]) => n !== 'app_environment')` 后与既有期望序列相等（`every(...)` @346/419 的「只有 list_workspaces」口径同样先 filter）；`app_environment` 次数单列断言（与 @375 的 `toEqual([...])` 解耦） | 表驱动（3 处断言） |
| App 壳 — 欢迎屏无徽标（AC-14/待决问题 4） | 边界 | 空清单启动（`list_workspaces` 回 `[]`）→ 欢迎屏在场、`queryByTestId('dev-badge') === null`（`ShellHeader` 仅在选中 workspace 后渲染，属知情边界，不作缺陷断言） | 单值 |
| App 壳 — 既有 UI 行为零回归（AC-16） | 正向 | 既有全量用例（恢复默认序第一名 / 列表 → 详情路由 / 添加与取消对话框 / 移除 / db 查看 / 配置 / 基础信息 / agent 选择器）在新增 `app_environment` 桩后全绿 | 表驱动（既有全量） |

#### Mock策略

| Mock主体 | Mock方案 | 应用场景 |
|---|---|---|
| IPC（进程边界：接口） | `vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }))` + `mockIpc()` 按命令名分发；**新增** `app_environment` 分支（默认回 dev fixture，可切 prod / reject / pending / 非法载荷），未显式打桩时按既有兜底 `Promise.resolve(null)` 落入「mode 未知」 | 全部用例 |
| 版本号与更新端点（进程边界） | 既有 `vi.mock('@tauri-apps/api/app', …)` / `vi.mock('@tauri-apps/plugin-updater', …)` 原样保留 | 全部用例 |
| 文件对话框（进程边界） | 既有 `vi.mock('@tauri-apps/plugin-dialog', () => ({ open: openMock }))` 原样保留 | 添加 / 取消流用例 |
| 前端构建信号（已删除的判据） | **不再 mock**：`vi.stubEnv('DEV', …)` / `vi.unstubAllEnvs()` 与 `afterEach` env 清理删除（判据换 `mode`，AC-1 测试侧配套） | 改造点，不属用例 |
| sonner（toast） | 不 mock：`<Toaster />` 真实挂载，断言走 DOM 文案 + `waitFor`；`beforeEach` 统一 `toast.dismiss()` 清模块级残留（既有装置） | 失败面用例 |

---

## 不可测试项

1. **`packages/desktop/package.json`（`build` / `build:prod` / `server:build:dev` / version bump，AC-2 / AC-3 / AC-4 / AC-17）** — 原因：JSON 脚本配置无函数面，无单元测试路径。`--debug --no-bundle`、`CARGO_TARGET_DIR=target/dev-build`、私钥只约束 `server:build` 等语义均为**构建实跑 + 磁盘产物 + `git status --porcelain`** 验证（proposal AC-2/3/4 的「验证方法」列已是实跑口径）；`version` 0.5.31 与 `plugins/**` 零 bump 属 diff 审查。单测可覆盖的仅是运行时效果（`mode=dev` 落 `env::mode()`、数据根落 `env::resolve_root`），已在 `env_test.rs` / `selfcheck_test.rs` 承接。

2. **`packages/desktop/RELEASE.md`（本地构建两档 / dev 数据根 / `--self-check` 用法 / 验收红线，AC-13）** — 原因：文档无执行面。文本与实现的一致性属人工核对（AC-13「RELEASE.md 与 spec 文本核对」）；`import.meta.env.DEV` 陈述改成「按 mode 判据」的同步性由 `grep` 复核（桌面 static-check 承接）。

3. **`packages/desktop/src/types/generated/bindings.ts`（重导出生成物）** — 原因：生成物禁手改，其正确性由 `bindings:check`（`bindings:export` 后 `git diff --exit-code -- src/types/generated`）与 `src/bindings/mod_test.rs` 的产物字节 / 清单断言承接（AC-16）；单测不另写对生成物文本的重复断言。

4. **`packages/desktop/src-tauri/src/main.rs`（启动装配序列，AC-10 / AC-12 / AC-15 / AC-1 源码面）** — 原因：二进制入口，无公共函数行（design「公共函数 / API」子表零行），`main` 内的 `build()` → 数据根解析 → `manage(DataRoot)` → 自检 → `run` 序列、装配期 `#[cfg(all(desktop, not(debug_assertions)))]` 单实例分流、prod 侧 `--self-check` 拒绝分支（`cfg!` 常量短路，不在 `#[cfg]` 内）均**无法**在单元测试内构造（需真实 `Builder::build` + 事件循环 + 进程退出）。可自动化部分已下沉：提示渲染与双写 → `env_test.rs`（`render_root_error` + `append_boot_error`），prod 拒绝文案 → `selfcheck_test.rs`（`PROD_REJECT_MESSAGE` 逐字），退出码 2 → 进程级实跑。装配形态属**源码审查 + 实跑**（AC-10 / AC-12 / AC-15 的「验证方法」列已如此规定）。同理，AC-1 / AC-7 的**源码零残留扫描**（`tauri::is_dev()` / `import.meta.env` / `canonicalize` / `.dev-team` 字面量唯一性）是静态检查项，属桌面 static-check，不在单元测试内。

5. **`packages/desktop/src-tauri/src/lib.rs`（两行 `pub mod env;` / `pub mod selfcheck;` 挂载）** — 原因：无公共 API 行、无运行时行为；挂载正确性由 `cargo clippy --all-targets` / `cargo test` 的模块解析在编译期机械保证（未挂载 ⇒ `env_test` / `selfcheck_test` 无法解析即失败）。不为测试新增 `lib_test.rs`。

6. **`.github/workflows/desktop-release.yml` / `src-tauri/tauri.conf.json` / `src-tauri/capabilities/default.json`（只读红线，AC-10 / AC-17）** — 原因：只读红线的验收语义就是**零 diff**（workflow 无 `pnpm build` / `--debug` / `--no-bundle`、`identifier` 仍 `dev.wps.dev-team`、capability 零扩张），由 diff 审查承接（AC-17「diff 审查」）；CI 本身的构建行为不在本仓库单测射程。

7. **覆盖率与门禁统计（AC-16）** — 原因：`cargo test --workspace` / `vp test` 全绿、覆盖率阈值（lines ≥ 90 / branches ≥ 85 / functions ≥ 90）、`bindings:check` 零 diff、`vp check` / `clippy` 零告警属**桌面端机械步骤**（static-check / test-execution），本设计只给用例面，不复制运行脚本与命令细节。

8. **平台与进程级行为（AC-10 双实例并存 / R-3 macOS `build()` 后 exit 噪音 / R-5 junction 与 symlink 漏面）** — 原因：需要两个真实进程、真实安装版、真实符号链接（junction 需管理员或 `mklink`），单测无可构造面。AC-10 的**机制前提**（两根同名文件不同父目录）已在 `env_test.rs` + `store_test.rs` 承接；并存 / 不挂住 / mac 噪音属实跑观测项（proposal R-3 已明示本期只在 Windows 出结论；R-5 作为知情边界写进 spec）。

9. **`--self-check` 的进程级契约（AC-11 / AC-12：`selfcheck::run` 本体、零窗口、退出码 0/1/2、stdout 管道 flush、prod 侧立即返回不挂住）** — 原因：①`run(app: &tauri::App, …)` 的入参是**具体类型** `App<Wry>`，单测装置 `tauri::test::mock_app()` 产出 `App<MockRuntime>`，不可代入；若为可测性把签名泛化为 `&App<R: Runtime>` 属修改 design 契约（本期不做，登记为知情边界）；②零窗口需真实窗口系统观测、退出码需真实进程退出、flush 需管道捕获（Rust 单测内捕获 `stdout` 需重定向文件描述符，与 `--test-threads` 并行冲突）。单测面已到 `render`（逐字十键 + `error=`）与 `exit_code`（0/1 映射），`run` 的返回码恒由其委托面产出；整链四条臂属实跑（proposal AC-11「实跑四条臂 + 管道捕获」、AC-12「实跑 prod 产物（超时护栏 10s）」）。
