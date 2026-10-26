# 提案: desktop-build-mode-split

> **变更**: desktop-build-mode-split
> **日期**: 2026-10-26
> **状态**: draft
> **来源**: `openspec/changes/desktop-build-mode-split/explore.md`（requirement 类型；12 项决策 D1–D12 全部落定、三档信号矩阵已实测、桌面端零实现）
> **前置依赖**: 无。数据根注入缝、单实例 / devtools 的 `#[cfg]` 注册、`WorkspaceStores::open(data_root)`、specta 命令面与 `bindings:check` 守卫全部在位（`src-tauri/src/main.rs:18/40`、`crates/infra/store/src/store.rs:2217`、`src/types/generated/bindings.ts`）

---

## 问题

### 现象：「开发 / 生产」不是一个维度，而是三处各自为政的巧合

现状没有「模式」这个概念，只有三个互不相干的信号，且**没有任何信号管数据根**：

```
pnpm dev   ── tauri dev ────► debug profile   ⇒ debug_assertions = true   ┐
                              vp dev          ⇒ import.meta.env.DEV=true  ┤ 数据根 ~/.dev-team
pnpm build ── tauri build ──► release profile ⇒ debug_assertions = false  ┤ （两边完全一样）
                              vp build        ⇒ import.meta.env.DEV=false ┘   ↑ 无任何信号管它
```

| 面 | 位置 | 现状 | 后果 |
|----|------|------|------|
| 构建默认值 | `package.json` `build` = `tauri build` | 本地构建恒 release | 想验 dev 行为只能起 dev server；产物必带安装包 ⇒ 强制要求签名私钥（RELEASE.md 明写） |
| 数据根 | `main.rs:18` `const DATA_DIR = ".dev-team"`，`setup` 里 `home_dir()?.join(DATA_DIR)` | 单一常量、两模式同一个根 | dev 构建写生产库：`~/.dev-team` 被 dev / 已装生产版共用 |
| 单实例 | `tauri-plugin-single-instance`，Windows 以 `identifier`（`dev.wps.dev-team`）派生具名 mutex + 隐藏窗口类 | 无条件注册 | dev 构建与已装生产版**互斥**：后启动者唤起前者后自杀，agent 验收被运行中的生产版阻塞 |
| 更新检查 | `src/hooks/use-updater.ts:64` `if (!import.meta.env.DEV)` | 判据是 **vite** 信号 | dev 构建（`build --debug`）下该值为 **false** ⇒ dev 产物去查生产 release，点一下把生产版装上 |
| 验收入口 | 无 | 只能起 GUI / 等窗口 / 关进程 | 验收链路长、环境事故率高，agent 无从自证「我跑在哪种模式、用的是哪个库」 |

三条原始诉求（explore §2）：① `build` 默认开发模式、CLI 参数才生产；② dev / prod 数据分库，且 dev 构建与 `pnpm run dev` 共库；③ agent 验收用 dev，不被运行中的生产版阻塞。

### 症结：三个候选判别源里有两个是陷阱

实测信号矩阵（cargo 1.95 / tauri-cli 2.12.4 / tauri 2.12.x（仓内 pin 2.12.5）/ Windows）：

| 启动方式 | custom-protocol | cargo profile | `cfg!(debug_assertions)` | `tauri::is_dev()` | `import.meta.env.DEV` | `TAURI_ENV_DEBUG` |
|---|---|---|---|---|---|---|
| `tauri dev` | 否 | dev | **true** | true | true | `"true"` |
| `tauri build --debug` | **是** | dev | **true** | **false** | **false** | `"true"` |
| `tauri build` | 是 | release | false | false | false | **键完全缺失** |

1. **`tauri::is_dev()` 是陷阱**：实现为 `!cfg!(feature="custom-protocol")`，回答「我是不是 build 出来的」，`build --debug` 下为 **false**。
2. **`import.meta.env.DEV` 是陷阱**：回答「前端是否由 dev server 服务」，dev 构建恒 false——正是上面「点一下装上生产版」的成因。
3. **`TAURI_ENV_DEBUG` 在 release 下不存在**（既不是空串也不是 `"false"`）⇒ vite 侧只能写 `=== 'true'`。
4. 唯一在三档下都正确的判据是 **`cfg!(debug_assertions)`**；且 `tauri dev` 与 `build --debug` 同为 true ⇒ 诉求②「dev 构建与 dev server 共库」是**免费推论**，不需要任何额外机制。

### 症结二：单实例与 redb 锁都被数据根单一放大了

「无法访问 db」实为两把锁：① redb 文件锁（单进程写）；② 单实例具名 mutex。锁①由**数据根分裂自动解除**（workspace 库文件名由 canonical root 派生 ⇒ 同一 workspace 在两模式是**同名文件、不同父目录**，互不可见，store 一行不改）；锁②需要一处 `#[cfg]`。反过来，只要数据根不分裂，换任何 identifier 方案都要付编译期烧死的代价。

### 症结三：验收没有 agent 可消费的入口

`--self-check` 一类入口不存在 ⇒ 验收只能靠 GUI。若在 `setup` 钩子里实现，还会踩 tauri 的「先建窗、后跑钩子」：`app.rs:1443` 在 `RuntimeRunEvent::Ready` 调内部 `setup`，内部 `setup`（`app.rs:2688`）先按配置建窗再跑用户钩子 ⇒ 钩子里的自检会闪窗抢焦点。

## 提案

把「开发 / 生产」升格为真正的维度：**单一判别源沿它分裂构建默认值、数据根、单实例、自检入口与前端表现**；CI 因走裸 `tauri build` 而零改动地恒落生产侧。改动全部收在 `packages/desktop` 壳层（`main.rs` / 两个新模块 / 一条新命令 / 三个前端文件 / 构建脚本 / RELEASE.md），`core` / `infra` 零行为改动。

```
              ┌──────────────────────────────────────────────┐
              │  判别源：cfg!(debug_assertions)  —— 唯一单点  │
              └──────────────────┬───────────────────────────┘
                                 │
      ┌──────────────────────────┼──────────────────────────┐
      ▼                          ▼                          ▼
  tauri dev              tauri build --debug         tauri build（CI / build:prod）
  ─────────              ───────────────────         ───────────────────────────
  devtools ✔             devtools ✔                  devtools ✘
  控制台窗口 ✔            控制台窗口 ✔                  GUI subsystem
  单实例 ✘                单实例 ✘                      单实例 ✔
  数据根 dev 子根          数据根 dev 子根 / env 覆盖      数据根 prod（env 忽略 + 留痕）
  --self-check ✘          --self-check ✔                --self-check 立即报错退出
```

### 1. 判别源单点

Rust 侧只用 `cfg!(debug_assertions)`（含等价 `#[cfg(debug_assertions)]`），集中在 `env::mode()`；前端不推导，只经 IPC 取。明令 MUST NOT `tauri::is_dev()` / `import.meta.env.DEV`；不引入 build.rs 常量（前端要二次推导 ⇒ 必分叉）。

### 2. 构建脚本默认 dev，生产显式

```diff
- "build": "pnpm run bindings:export && pnpm run server:build",
- "server:build": "cross-env TAURI_SIGNING_PRIVATE_KEY_PASSWORD= tauri build",
+ "build": "pnpm run bindings:export && pnpm run server:build:dev",
+ "server:build:dev": "cross-env CARGO_TARGET_DIR=src-tauri/target/dev-build tauri build --debug --no-bundle",
+ "build:prod": "pnpm run bindings:export && pnpm run server:build",
+ "server:build": "cross-env TAURI_SIGNING_PRIVATE_KEY_PASSWORD= tauri build",
```

- `--no-bundle` ⇒ dev 构建**不再需要** `TAURI_SIGNING_PRIVATE_KEY`，RELEASE.md 那套私钥前置从此只约束 `build:prod`；
- `--debug` 同样注入 `--features tauri/custom-protocol`，产物内嵌前端资源 ⇒ **可脱离 dev server 双击运行**（已实测：`dev-team.exe` 内可逐字找到 `dist/index.html` 引用的 assets 文件名）；
- 隔离 `CARGO_TARGET_DIR`：`tauri dev` 无 `custom-protocol`、`build --debug` 有，共用 `target/debug` 会让 feature 翻转触发 tauri 及下游全量重编译；隔离目录落在已 gitignore 的 `src-tauri/target/` 内；
- `bindings:export` 走 `cargo run -p export-bindings`，编的是无 `custom-protocol` 的那份 ⇒ 与 `tauri dev` 同态、**不动**。

### 3. 数据根三分优先级 + 全项校验

```rust
// src-tauri/src/env.rs（壳层新模块，与 commands/ 平级；配 env_test.rs）
pub const PROD_DIR: &str = ".dev-team";
pub const DEV_SUBDIR: &str = "dev";
pub const DATA_ROOT_ENV: &str = "DEV_TEAM_DATA_ROOT";

pub struct DataRoot { pub path: PathBuf, pub source: RootSource }
pub enum RootSource { Prod, DevDefault, DevEnv }

pub fn prod_root(home: &Path) -> DataRoot;                                    // 不接收 env
pub fn resolve_dev_root(home: &Path, env: Option<&OsStr>) -> Result<DataRoot, RootError>;
```

| 模式 | `DEV_TEAM_DATA_ROOT` | 数据根 | `source` |
|---|---|---|---|
| dev | 未设 | `<home>/.dev-team/dev` | `dev-default` |
| dev | 已设且通过校验 | 该值（逻辑归一后） | `env` |
| dev | 已设但校验失败 | —（硬失败，退出码 2） | — |
| prod | 任意 | `<home>/.dev-team` | `prod` |

prod 侧**签名不接收 env** ⇒「不读」由结构保证，不靠调用点纪律。校验五项（空白 / 相对 / 等于 prod 根 / prod 根的严格祖先 / 已存在但是文件）各给自然语言提示，提示三要素：指名 `DEV_TEAM_DATA_ROOT`、**回显收到的值**、给可照抄的下一步。

归一化必须用 `Path::components()` 逻辑归一（吃 `.` / `..` / 尾分隔符），**MUST NOT 用 `canonicalize`**——目标目录通常还不存在，会 `NotFound` 误报；这与 store 的 `canonical::canonical_key`（要求路径存在）是两件事，spec 里写清防误用。祖先 / 相等判定走同一归一（Windows 下大小写不敏感）。

### 4. 启动序列（唯一形状）

```rust
fn main() {
    let want_self_check = std::env::args().skip(1).any(|a| a == "--self-check");

    #[cfg(not(debug_assertions))]                     // T6：prod 侧显式拒绝，不许整块删分支
    if want_self_check {
        eprintln!("--self-check 仅在 dev 构建可用（当前为生产构建）。");
        std::process::exit(2);
    }

    let mut builder = tauri::Builder::default().plugin(tauri_plugin_updater::Builder::new().build());
    #[cfg(all(desktop, not(debug_assertions)))]        // D3：dev 不注册单实例
    { builder = builder.plugin(tauri_plugin_single_instance::init(..)); }
    #[cfg(debug_assertions)]
    { builder = builder.plugin(tauri_plugin_devtools::init()); }
    builder = builder.plugin(tauri_plugin_dialog::init()).plugin(tauri_plugin_opener::init())
                     .setup(setup_hook);

    let app = builder.build(tauri::generate_context!("tauri.conf.json"))
        .expect("desktop 应用构建失败");               // 零窗口

    let home = app.path().home_dir().expect("home 目录解析失败");
    let data_root = match env::resolve_root(&home, cfg!(debug_assertions)) {   // 唯一计算点
        Ok(v) => v,
        Err(err) => { let msg = env::render_root_error(&err, &home);
                      eprintln!("{msg}"); env::append_boot_error(&home, &msg);
                      std::process::exit(2); }
    };
    app.manage(data_root.clone());                     // setup 与 self-check 共用同一份

    #[cfg(debug_assertions)]
    if want_self_check { std::process::exit(selfcheck::run(&app, &data_root)); }

    app.run(|_, _| {});
}
```

- **数据根只算一次**：`app.manage(DataRoot)` 后 `setup` 经 `State<DataRoot>` 取根 ⇒「两处看到同一个根」由类型保证，不靠纪律；
- 单实例 / devtools 全部在 builder 装配期按 `#[cfg]` 分流，运行期零判断；
- `setup` 只做真正需要 runtime 的事（`WorkspaceStores::open` / 引擎日志 / panic 钩子 / 各注册表）；
- `manage` 早于 `run` 是安全的：内部 `setup` 在 `RuntimeRunEvent::Ready` 才执行（`app.rs:1443`）；
- `home_dir` 经 `app.path()` 即可（`dirs::home_dir()` 同源），**无需新增 `dirs` 依赖**，也无需第二份 home 实现。

### 5. `--self-check` 契约（agent 面向）

```
mode=dev
home=C:\Users\wps
data_root=C:\Users\wps\.dev-team\dev
data_root_source=dev-default|env|prod
global_db=C:\Users\wps\.dev-team\dev\desktop-global.redb
global_db_open=ok
workspaces_dir=C:\Users\wps\.dev-team\dev\workspaces
workspaces_dir_exists=false
workspace_count=0
models=WorkspaceRecord,AgentProviderRecord,AgentInstanceRecord
```

逐行 `key=value`、无装饰、退出码即结论：0 全通 / 1 数据根或库打开失败（附 `error=`）/ 2 用法或环境非法。位置 MUST 在 `build()` 与 `run()` 之间（放 `setup` 会闪窗）；`exit` 前 MUST flush stdout（管道捕获时块缓冲会丢输出）；prod 侧必须**识别并拒绝**该参数（整块 `#[cfg]` 删掉会让 prod 传参静默起 GUI，agent 挂到超时）。`--self-check --root <path>` 本期不做（与 env 语义打架）。

`models=` 取自全局库 `list_models()`，免费换来「模型注册齐不齐」的断言面；`global_db=` / `workspaces_dir=` 的路径成分取自 store 公开常量（`GLOBAL_DB_FILE_NAME` / `WORKSPACES_DIR_NAME` 由 `pub(crate)` 升 `pub` 并重导出，**不在壳层复写字面量**）。

### 6. 失败可见性双写

```
env / 数据根校验失败 ─┬─► stderr（命令行启动时 agent 可捕获；dev 构建是 CONSOLE subsystem）
                      └─► <home>/.dev-team/logs/boot-error.log（append-only，自建父目录）
```

锚点选 prod 根：env 已被判无效不能拿它当落点，prod 根是唯一稳定锚（**日志不是数据**，不违反「prod 不认 env」）。GUI 启动与 self-check 共用同一渲染函数，只差「往哪写」。引擎异常日志（`<数据根>/logs/engine.log`）与启动失败日志不混写。

### 7. prod 侧 env 检测留痕（不阻断）

prod 构建只 `var_os().is_some()` 探测（不解析值），命中即向 `<prod 根>/logs/ignored-env.log` 追加 `ignored_env=DEV_TEAM_DATA_ROOT (release build ignores it)`：陈旧变量不该让安装版打不开，但「设了 env 却误跑 `build:prod`」必须留痕，否则数据无声写进真实生产库。

### 8. 前端 mode 穿透（IPC 单源）

`app_environment()` → `{ mode: 'dev'|'prod', dataRoot, source }`（新轨道 `commands/env/`，薄包装 + `*_inner` 微形态，`State<DataRoot>`）。`useUpdater` 判据从 `import.meta.env.DEV` 换成 `mode === 'prod'`，**mode 未就绪（`null`）或 IPC 失败时不判定**。mode === 'dev' 时顶栏呈 `DEV` 徽标（`title` 携 `dataRoot`），解决「两个 app 同跑，我看的是哪个库」。版本号**不**加 `-dev` 后缀（不动 `package.json` 语义，避免污染 tag 与 updater 比较）。

### 范围裁决（explore 决议的落地口径）

- **D1 判别源**：`cfg!(debug_assertions)`；`tauri::is_dev()` / `import.meta.env.DEV` 双禁令，矩阵作 rationale 入 spec。
- **D2 默认值倒置**：`build` = dev、`build:prod` = 现行 release；CI 零改动（tauri-action 直调 tauri CLI，不经 `pnpm build`）。
- **D3 dev 不注册单实例**：一处 `#[cfg]` 换来 dev 与已装生产版并存；改 identifier 被否（编译期烧死 + `--config` 变体 + 产物改名 + endpoint 覆盖 + tauri-build 重编译）。
- **D4 嵌套 dev 子根** `<home>/.dev-team/dev`：与 prod 同屋檐、一望即知归属；兄弟根 `.dev-team-dev` 与 prod 搬家均被否（后者破坏已装用户数据）。
- **D5 存量数据不迁移**：dev 数据本可弃；拷种子属隐式魔法，与 fail-fast 相冲。
- **D6 prod 不认 env**：由签名（`prod_root` 不接收 env）保证。
- **D7 校验失败 = 硬失败 + 自然语言**：agent 读提示 → 改参数 → 重试；「忽略并记因」被否（agent 无从得知该改什么）。
- **D8 验收入口 `--self-check`**：绕开「起 GUI / 等窗口 / 关进程」整段环境事故；WebDriver 方案留后续（与 `--self-check` 共用同一套数据根 / env 约定）。
- **D9 前端经 IPC 单源**：前端不可能与后端分叉；顺带拿到 dataRoot。
- **D10 双写**：双击启动时控制台随进程退出立刻消失，日志是唯一留存观测面。
- **D11 prod 侧 env 不阻断只留痕**（见 7）。
- **D12 不清理 `~/.dev-team` 存量数据**（用户裁定）。
- **本期不做**：`--self-check --root <path>`；数据迁移 / 种子拷贝；identifier 变体；`pnpm accept` + WebDriver 验收链；日志轮转；dev 产物用 release 优化（见 R-1 退路）。
- **已知摩擦（已计入工量）**：`src/app.test.tsx` 有严格 invoke 序列断言（`every(...)` @346/419、`toEqual([...])` @375），挂载期多一次 `app_environment` 会打破，需改 `filter` 口径或前置 mock。

## 能力

### 新增的能力

- **desktop-build-modes** — 九条 requirement：① 构建模式判别源单点（三档矩阵 + 双禁令）；② 构建脚本默认 dev 与显式生产入口（`--no-bundle` / 隔离 target / 私钥只约束 `build:prod`）；③ 数据根三分优先级与路径校验（含「不得落在 prod 根或其祖先」红线、逻辑归一纪律）；④ 启动失败可见性双写（stderr + `boot-error.log`）；⑤ prod 构建的环境变量检测留痕（不阻断）；⑥ dev 构建不注册单实例插件（并存语义 + identifier 零改动）；⑦ `--self-check` 自检契约（零窗口 / 十行 `key=value` / 三档退出码 / prod 显式拒绝 / 验收红线）；⑧ 前端 mode 穿透（IPC 单源 / updater 判据切换 / `DEV` 徽标）；⑨ 启动序列与数据根状态单点。

### 修改的能力

- **desktop-workspace-store** — **MODIFIED**「全局库与 workspace 库双库布局」：全局库 / `workspaces/` 的父目录由「恒 `home_dir()/.dev-team`」改为「**数据根**」——release 为 `<home>/.dev-team`、dev 构建为 `<home>/.dev-team/dev`；store 侧零改动（根一分裂，全局库 / `workspaces/` / `logs/` 三者自动随根分裂），新增「dev 构建根分裂」scenario。既有「单进程约束显式化」requirement 的表述（双开列为已知约束、store 不自建跨进程锁）**原样保留**——它现在由本变更加以解除，而非被推翻。
- **desktop-data-dimensions** — **MODIFIED**「workspace 维度落盘于全局目录 per-workspace db」：`home_dir()/.dev-team/workspaces/` → 「数据根下 `workspaces/` 子树」，并钉住「同一 workspace 在两模式下是不同文件、不同父目录」的隔离语义。
- **desktop-release-pipeline** — **ADDED**「CI 构建恒生产模式（不经 pnpm 脚本）」：build matrix MUST 由 tauri-action 直接调 `tauri build`，MUST NOT 经 `pnpm build`（默认已倒置为 dev，改走脚本会悄悄产 dev 构建）；CI MUST NOT 设 `DEV_TEAM_DATA_ROOT`；`--debug` / `--no-bundle` 参数 MUST NOT 出现在 workflow。

其余能力零改动，理由：

- **desktop-app-shell**：新命令轨道 `commands/env/` 是「轨道枚举」的又一成员——该 requirement 现有文本本就只列 `changes` / `exec` / `db` 三轨（`agents` / `stats` / `config` / `explores` / `watch` 历来以零 delta 追加），枚举非穷尽且非规范性；`app_environment` 的契约由新能力「前端 mode 穿透」承载。builder 链仍与 `dialog` / `opener` 同位，`ExternalLink` 唯一调用点不动。
- **desktop-ipc-type-bindings**：「生成物入库与一致性守卫」已覆盖任意新命令（`bindings:check` 重导出后零 diff 属既有门禁，其「新命令绑定同步」scenario 即本类情形），新增 `app_environment` 不需要新条款。
- **desktop-crate-layout**：`env` / `selfcheck` 落 `dev-team` 壳（`src-tauri/src/`，无新 crate、无新依赖边）；判别源在壳层的事实不触碰任何分层结论（core / infra 仍禁 Tauri）；新模块名与 DTO 字段不含 `openspec` 字样。
- **desktop-workspace-config / desktop-agent-execution / desktop-explore-* / desktop-change-\*** 等：数据根以管理态整体注入 `WorkspaceStores::open`，下游（workspace 库寻址、change 流程、explore 清单）语义全不变——它们只认识"某个数据根"，不认识"根从哪来"。

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `src-tauri/src/env.rs`（新） | `PROD_DIR` / `DEV_SUBDIR` / `DATA_ROOT_ENV` / `LOGS_DIR_NAME` 常量；`DataRoot` / `RootSource` / `RootError`；`prod_root(home)`（不接收 env）、`resolve_dev_root(home, env)`、`resolve_root(home, is_dev)` 薄分派；`normalize()` 逻辑归一（components，拒绝 `canonicalize`）；`mode() -> "dev"\|"prod"`；`render_root_error(&RootError, home)`（自然语言三要素）；`append_boot_error(home, msg)` / `append_ignored_env(home)`（自建父目录、append-only、IO 失败静默） |
| `src-tauri/src/selfcheck.rs`（新） | `Summary`（十键）+ `render(&Summary) -> String`（纯函数）+ `exit_code(&Probe) -> i32` + `run(&App, &DataRoot) -> i32`（探库 → 渲染 → flush）；路径成分取 store 公开常量 |
| `src-tauri/src/lib.rs` | 挂 `pub mod env;` / `pub mod selfcheck;`（测试模块 `env_test` / `selfcheck_test` 同处登记） |
| `src-tauri/src/main.rs` | 启动序列重排（build → 解析根 → `manage` → self-check → run）；删除 `DATA_DIR` 常量（迁 `env::PROD_DIR`）、`LOGS_DIR_NAME` 改引 `env::`；单实例 `#[cfg(all(desktop, not(debug_assertions)))]`；`setup` 收成命名函数 `setup_hook` 并改经 `State<DataRoot>` 取根；prod 侧 `--self-check` 显式拒绝 |
| `src-tauri/src/commands/env/mod.rs`（新） | `AppEnvironment { mode, data_root, source }`（camelCase 线面）+ `app_environment`（薄包装，`State<DataRoot>`）+ `app_environment_inner(&DataRoot)` 微形态 |
| `src-tauri/src/commands/mod.rs` | `pub mod env;` + `all_commands!` 增一行 |
| `src-tauri/crates/infra/store/src/store.rs` + `lib.rs` | `GLOBAL_DB_FILE_NAME` / `WORKSPACES_DIR_NAME` 由 `pub(crate)` 升 `pub` 并重导出（零行为改动：为 self-check 摘要提供单点，MUST NOT 在壳层复写字面量） |
| `packages/desktop/package.json` | `build` → dev；新增 `build:prod` / `server:build:dev`（`--debug --no-bundle` + 隔离 `CARGO_TARGET_DIR`）；`version` 0.5.30 → 0.5.31 |
| `src/hooks/use-app-environment.ts`（新） | 挂载一次 `commands.appEnvironment()`；成功返回 `{ mode, dataRoot, source }`，未就绪 / 失败为 `null`（静默） |
| `src/hooks/use-updater.ts` | `useUpdater(mode: 'dev' \| 'prod' \| null)`：启动检查判据换 `mode === 'prod'`，`import.meta.env.DEV` 判据删除 |
| `src/app.tsx` | 挂 `useAppEnvironment()` → 传 mode 给 `useUpdater`；`ShellHeader` 增 `DEV` 徽标（`data-testid`，`title` = dataRoot） |
| `src/types/generated/bindings.ts` | `bindings:export` 重导出（新增 `appEnvironment` 与 `AppEnvironment` 类型） |
| `packages/desktop/RELEASE.md` | 「本地构建须知」拆两档：`build` 零配置产 dev 二进制；私钥前置只约束 `build:prod`；补一句 dev 数据根落 `<home>/.dev-team/dev` 与 `--self-check` 用法 |

### 测试文件

| 用例 / 装置 | 位置 | 处置 |
|---|---|---|
| 归一化：`.` / `..` / 尾分隔符 / 重复分隔符归一到同一路径；不存在路径不报错（证不依赖 `canonicalize`）；Windows 大小写等价形态归一后相等 | `src-tauri/src/env_test.rs`（新） | 新增（纯函数，零 fs） |
| 三分优先级：无 env → `dev-default` 且路径为 `<home>/.dev-team/dev`；env 合法 → `env` 且为归一路径；`prod_root` 恒 `<home>/.dev-team` 且 `source=prod` | `env_test.rs` | 新增 |
| 五项拒绝：空白 / 相对 / 等于 prod 根 / prod 祖先（家目录、盘符根、`..` 变形、大小写变形）/ 目标是文件 | `env_test.rs` | 新增（文件项用 tempdir） |
| 提示三要素：逐字含 `DEV_TEAM_DATA_ROOT`、含收到的原值、含可照抄下一步 | `env_test.rs` | 新增（逐 variant 断言） |
| 双写与留痕：`append_boot_error` 落 `<home>/.dev-team/logs/boot-error.log` 且目录自建、append 不清旧；`append_ignored_env` 落 `<home>/.dev-team/logs/ignored-env.log` 且行文本逐字 | `env_test.rs` | 新增（tempdir） |
| 摘要渲染：十键键序固定、`source` 三值透传、失败附 `error=` 行、退出码映射（0/1/2） | `src-tauri/src/selfcheck_test.rs`（新） | 新增（`render` / `exit_code` 纯函数；`run` 的 IO 部分不在射程） |
| 命令薄包装：`app_environment_inner` 从 `DataRoot` 平移出 `{ mode, dataRoot, source }`（mode 与 `env::mode()` 同源、source 三值逐项） | `src-tauri/src/commands/env/mod_test.rs`（新） | 新增（不起 runtime，沿 `commands/config/mod_test.rs` 直调先例） |
| 前端 hook：`appEnvironment` 成功 → 三字段；reject → `null` 且无错误态；未返回 → `null` | `src/hooks/use-app-environment.test.ts`（新） | 新增（mock 生成物 `commands`） |
| updater 判据：`mode='dev'` → `check` 零调用但版本号照拉；`mode='prod'` → 照旧检查；`mode=null` → 不判定；既有下载 / 安装用例改传 mode | `src/hooks/use-updater.test.ts` | 改造（删 `vi.stubEnv('DEV', …)`，改 mode 入参） |
| 壳层序列：挂载期新增 `app_environment` 一次（`countOf === 1`）；`every(...)` / `toEqual([...])` 断言改 `filter` 口径；`DEV` 徽标用例（dev 有 / prod 无，`title` = dataRoot） | `src/app.test.tsx` | 改造（+2 用例） |

### 不要修改

- `.github/workflows/desktop-release.yml`（零改动**即红线**：tauri-action 直调 tauri CLI ⇒ CI 恒定生产侧；若本文件出现 `pnpm build` / `--debug` / `--no-bundle` 即为违约）。
- `src-tauri/tauri.conf.json`（`identifier` / `version` / `bundle` / `plugins.updater` / `app.windows` 全项不变）。
- `src-tauri/capabilities/default.json`（`tauri-plugin-single-instance` 不需要 capability；权限面零扩张）。
- `src-tauri/crates/**`（除 store 两常量 `pub(crate)` → `pub`）：core 各 crate、`infra/{agent,exec,vcs,checks,watch}` 零行为改动；store 的打开 / 寻址 / 模型语义零改动。
- 前端 `src/views/**` / `src/components/**` / `src/routes.tsx`（除 `app.tsx` 顶栏徽标）；`src/lib/**`。
- `plugins/**`（插件源已弃用，与本次桌面端构建模式无关，零改动零 bump）。
- `openspec/config.json`（无新增配置项）；`openspec/changes/desktop-build-mode-split/explore.md`（只读）。

## 验收标准

| ID | 验收条件 | 验证方法 |
|----|---------|----------|
| AC-1 | 判别源单点：`src-tauri/src/**` 与 `src/**`（排除生成物）无 `tauri::is_dev()` 调用、无 `import.meta.env.DEV` 模式判据、无 build.rs 模式常量；模式判定只经 `env::mode()`（Rust）与 `app_environment` IPC（前端） | 源码扫描断言 + 既有 `vite.config.ts` 零改动复核 |
| AC-2 | 默认构建落 dev：未设任何签名环境变量时 `pnpm -C packages/desktop run build` 成功、不产安装包、产物可双击运行且运行期 `mode=dev` | 实跑构建 + `--self-check` 输出 `mode=dev` |
| AC-3 | 生产构建语义不削弱：`build:prod` 缺 `TAURI_SIGNING_PRIVATE_KEY` 仍失败；设齐后产出现行同形态安装包与 updater 工件 | 实跑 + 产物比对（`*_x64-setup.exe` + `.sig`） |
| AC-4 | target 目录隔离：`tauri dev` 落 `src-tauri/target/debug`、dev 构建落隔离目录；两者均被 gitignore 覆盖；`bindings:export` 不落隔离目录 | 磁盘检查 + `git status --porcelain` 干净 |
| AC-5 | 数据根三分：无 env → `<home>/.dev-team/dev`（`source=dev-default`）；合法 env → 该值（`source=env`）且与默认根数据互不可见；prod 构建 → `<home>/.dev-team`（`source=prod`） | `env_test` 三分用例 + 两次 self-check 输出对比 |
| AC-6 | 五项校验全拒且提示三要素齐备：空白 / 相对 / 等于 prod 根 / prod 根严格祖先（含 `..` 与大小写变形）/ 目标是文件 → 启动硬失败、退出码 2、提示逐字含变量名 + 原值 + 可直接照抄的下一步 | `env_test` 逐 variant 断言 + 实跑一次 GUI 启动 |
| AC-7 | 归一口径正确：不存在的目标路径校验不报 `NotFound`；`a/./b/../c/` 与 `a/c` 同判；Windows 大小写等价形态不绕过红线；源码无 `canonicalize` 调用 | `env_test` + 源码扫描 |
| AC-8 | 双写落地：stderr 与 `<home>/.dev-team/logs/boot-error.log` 同一段文字；`logs/` 缺席时自建；多次失败 append 不覆盖 | `env_test`（tempdir）+ 实跑 |
| AC-9 | prod 侧 env 留痕：残留 `DEV_TEAM_DATA_ROOT` 时启动成功、根不变、`logs/ignored-env.log` 追加一行；无 env 时不产生记录 | 实跑 prod 产物 + 文件比对 |
| AC-10 | 单实例并存：prod 版运行中启动 dev 构建 → 两者并存、无 mutex 互斥、无 redb 锁冲突（各自数据根）；prod 双击语义逐字不变；`identifier` 仍 `dev.wps.dev-team` | 实跑双实例 + `tauri.conf.json` 审阅 |
| AC-11 | `--self-check` 契约：dev 构建十行 `key=value`（键序固定）+ 退出码 0；库坏 → 1 且含 `error=`；env 非法 → 2 且与 GUI 同文案；全程零窗口；stdout 经管道可完整读到（flush 生效） | 实跑四条臂 + 管道捕获 |
| AC-12 | prod 拒自检：prod 产物带 `--self-check` → stderr 说明 + 退出码 2 + 立即可返回（不挂住）；该分支不在 `#[cfg]` 内（源码审查） | 实跑 prod 产物（超时护栏 10s） |
| AC-13 | 验收红线在案：验收 / 示例命令显式设独立 `DEV_TEAM_DATA_ROOT`；人工 dev 实例运行中做 self-check 不互相阻塞 | RELEASE.md 与 spec 文本核对 + 实跑 |
| AC-14 | 前端穿透：`mode=dev` → `check` 零调用 + `DEV` 徽标（`title` = dev 数据根）；`mode=prod` → 启动检查一次 + 无徽标；`null` → 不判定、无错误态 | `use-updater.test.ts` / `use-app-environment.test.ts` / `app.test.tsx` 用例 |
| AC-15 | 启动序列单点：`.dev-team` 字面量只在 `env` 模块常量一处；`setup_hook` 经 `State<DataRoot>` 取根、无二次解析；数据根失败经 `main` 处置（stderr 不含 `Failed to setup app`） | 源码审查 + `main.rs` 用例覆盖（失败臂） |
| AC-16 | 零回归与门禁：`cargo test --workspace` 与 `vp test` 全绿（基线 2866 用例）；覆盖率不低于阈值（lines ≥ 90 / branches ≥ 85 / functions ≥ 90，基线 97.23 / 93.39 / 93.84）；`bindings:check` 零 diff（重导出后）；`vp check` 与 `cargo clippy` 零告警 | `pnpm -C packages/desktop run check` / `run test` + test-execution 门禁 |
| AC-17 | 交付面收口：`package.json` version 0.5.30 → 0.5.31；`plugins/**` 零改动零 bump；`.github/workflows/desktop-release.yml` 与 `tauri.conf.json` 零 diff | diff 审查 |

## 风险

| ID | 风险 | 影响 | 概率 | 缓解措施 |
|----|------|------|------|----------|
| R-1 | **debug profile 的性能代价**：`cfg!(debug_assertions)` 只给 dev profile（opt-level=0），dev 产物跑 agent 编排 / tokei / 大库扫描可能慢到不可用 | dev 产物不可日常使用，等于只换了个名字 | 中 | 实现后**实测**一次编排 run 与代码统计耗时并与 release 对比；超阈值则退路为判据换 `cfg!(feature="dev-mode")`（release 优化 + dev 数据面），**§4 全部结构不变，只换判别源名字**——该退路写进 spec 的余地保留在 `env::mode()` 单点内 |
| R-2 | **`CARGO_TARGET_DIR` 相对路径基座**：cargo 以 `src-tauri` 为 cwd 运行，`src-tauri/target/dev-build` 可能被解析成 `src-tauri/src-tauri/target/dev-build` | 隔离目录落错位（仍在 gitignore 内，但语义噪音） | 中 | 实现期以磁盘实际产物校准；必要时改绝对路径或改用 tauri CLI 自身的 target 目录形态——**隔离语义（与 `target/debug` 不同）不变**，落地形态 design 定夺 |
| R-3 | **`build()` 后直接 `exit` 的平台副作用**：macOS 上事件循环已创建后立即退出可能触发 `NSApplication` 告警或清理噪音（explore §7 未验） | self-check 在 mac 上出现噪音输出，污染 `key=value` 面 | 低 | 显式 flush + 实测一次；若告警落在 stdout/stderr，改「返回退出码 → `main` 正常返回」形态（退出码语义不变）；本期只在 Windows 出验收结论，mac 侧留观测项 |
| R-4 | **env 覆盖踩到既有目录**：agent / 人把 `DEV_TEAM_DATA_ROOT` 设到已有数据的目录，与既有数据混合 | 数据面混淆，验收结论失真 | 中 | self-check 摘要与 IPC 都回报 `data_root` 与 `source`；RELEASE.md 与 spec 写明「验收须用独立目录」；`WORKSPACES_DIR_NAME` 下的内容按 canonical root 寻址，混用只会多出孤儿库文件（可识别、可清理），不串库 |
| R-5 | **归一的已知漏面**：junction / symlink / 8.3 短名指向 prod 根时，逻辑归一看不穿 ⇒「不得落在 prod 根或其祖先」可被绕过 | 验收脚本可能写进生产库 | 低（需刻意构造） | 逻辑归一 + Windows 大小写不敏感比较覆盖常规误设；**不引入 `canonicalize`**（会把"不存在路径"变成硬错误）；junction 漏面作为知情边界写进 spec（MUST NOT 被当作安全缺口遗漏）；真兜底是「验收用独立目录」红线 |
| R-6 | **存量数据语义错位**：切模式后 dev 看到空库，而 `~/.dev-team` 里那套旧 dev 数据被 release 侧继承当作**生产数据** | 用户认知落差（"我的 workspace 清单没了"） | 中（既定，D5 接受） | 不迁移、不拷贝种子（D5/D12）；spec 与 RELEASE.md 各提示一次；dev 侧重新添加目录即可重建（注册在全局库，冷启动从空开始） |
| R-7 | **`app.test.tsx` 序列断言的改造规模**：新增挂载期 `app_environment` 会打破 `every(...)` @346/419 与 `toEqual([...])` @375 | 用例改造被低估，拖累工期 | 中 | 已列为显式测试文件处置（改 `filter` 口径 + `countOf('app_environment') === 1`）；mock 的兜底分支返回 `null` ⇒ 未显式打桩的用例自动落入「mode 未知」路径，改造面收敛为三处断言 |
| R-8 | **prod 侧拒绝分支被 `#[cfg]` 误删**（T6）：整块 `#[cfg(not(debug_assertions))]` 掉 `--self-check` 的拒绝，prod 传参会静默起 GUI | agent 挂到超时（最坏结果：无输出无报错） | 中 | AC-12 用 prod 产物实跑（10s 超时护栏）+ 源码审查该分支不置于 `#[cfg]` 内 |
| R-9 | **updater 判据切换后的空窗**：mode 未就绪 / IPC 永久失败时安装版永不检查更新（静默退化） | 更新提示不出现，用户以为已是最新 | 低 | 该命令是无 IO 的管理态纯读，失败面近乎为零；徽标缺席可作线索；`use-updater.test.ts` 三态（dev / prod / null）用例钉住行为 |
| R-10 | **CI 被改成 `pnpm build`**（T10）：默认已倒置为 dev，改走脚本会悄悄产 dev 构建 | 发版产物是 dev 版（无 devtools 但数据根 / 单实例全落 dev 侧） | 低 | desktop-release-pipeline 新增红线 requirement；AC-17 断言 workflow 零 diff；`.github/**` 不进本变更实现范围 |
| R-11 | **prod 根下新增两个日志文件的语义边界**：`boot-error.log` / `ignored-env.log` 与 `engine.log` 若混写会破坏引擎日志的可解析性 | 观测面互相污染 | 低 | spec 明写文件归属（启动失败 / env 留痕恒锚 prod 根，引擎日志随数据根）；Module Contract 登记；无轮转（与 `engine.log` 现状同口径）留观察项 |
| R-12 | **`store` 常量公开化扩大 API 面**：`pub(crate)` → `pub` 被质疑越界 | crate 边界面审查受阻 | 低 | 仅两常量、零行为、零新依赖；替代方案（壳层复写 `"desktop-global.redb"` / `"workspaces"`）被否——那会造出第二份定义，与本仓「路径 / 名字单点」纪律冲突；新能力 Module Contract 登记该动机 |
| R-13 | **`--no-bundle` 的隐含变化**：dev 构建不再产安装包 ⇒ 有人仍需"装一份 dev 版"时无产物可用 | 使用方式变化 | 低 | 双击 `src-tauri/target/dev-build/debug/dev-team.exe` 即可运行（内嵌前端）；需要安装包时走 `build:prod`（生产侧本来就不该装 dev 版）；RELEASE.md 写明 |
