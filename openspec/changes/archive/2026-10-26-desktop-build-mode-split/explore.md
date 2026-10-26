# 桌面端构建模式：默认 dev、显式 prod，与数据根分裂
workflow_type: requirement

> 状态：**探索已收敛**，12 项决策全部落定，可直接进 change。
> 实测环境：cargo 1.95 / rustc 1.95 / tauri-cli 2.12.4 / tauri 2.12.2 / Windows。
> 最后更新：2026-10-26。

---

## 1. 结论先行

一句话：**把"开发/生产"变成一个真正的维度**——单一判别源（`cfg!(debug_assertions)`），
沿它分裂构建默认值、数据根、单实例、自检入口与前端表现；CI 因走裸 `tauri build` 而零改动地
恒落生产侧。

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

### 1.1 决策总表（12 项，全部落定）

| # | 决策 | 依据 | 已否决的替代 |
|---|------|------|-------------|
| D1 | 判别源 = `cfg!(debug_assertions)` | 唯一在三档启动方式下都正确的信号（§3.1） | `tauri::is_dev()`（dev 构建下为 false）、`import.meta.env.DEV`（语义是"dev server 服务"，不是模式）、build.rs 注入 env 常量（前端要再推导一次） |
| D2 | `build` 默认 dev，`build:prod` 才生产 | 原始诉求 D1；CI 零改动 | — |
| D3 | dev 构建**不注册**单实例插件 | 一处 cfg 换来 dev 与已装生产版并存，不改 identifier | 改 identifier（编译期烧死，要 `--config` 变体 + 重编译） |
| D4 | dev 数据根 = `<home>/.dev-team/dev`（嵌套） | 与 prod 同屋檐，一望即知归属 | 兄弟根 `.dev-team-dev`（当选过，后改嵌套）；prod 搬家（破坏已装用户数据，直接否） |
| D5 | 存量数据**不迁移** | dev 数据本可弃；拷种子属隐式魔法，与 fail-fast 相冲 | 首启从 prod 根拷种子 |
| D6 | release **不认** `DEV_TEAM_DATA_ROOT` | 生产数据根不可被重定向 | release 也认（多一条可误触的生产开关） |
| D7 | env 校验失败 = **硬失败 + 自然语言提示** | agent 读提示→改参数→重试；与仓库 fail-fast 哲学同口径 | 忽略后记因（agent 无从得知该改什么） |
| D8 | 验收主入口 = `--self-check` | 绕开"起 GUI / 等窗口 / 关进程"整段环境事故 | WebDriver 驱动（能验界面但链路长，留作后续 A 方案） |
| D9 | 前端 mode 经 **IPC** 单源 | 前端不可能与后端分叉；顺带拿到 dataRoot | vite `define` 从 `TAURI_ENV_DEBUG` 推导（两处推导同一事实；且 release 该键根本不存在） |
| D10 | 失败信息双写：stderr + `logs/boot-error.log` | 双击启动时控制台随进程退出立刻消失 | 仅 stderr（人/agent 都看不到） |
| D11 | release 侧检测到 env → 不阻断，仅留痕 | 陈旧变量不该让安装版打不开；但必须防"设了 env 却误跑 build:prod"无声写生产库 | 阻断启动 |
| D12 | 不清理 `~/.dev-team` 存量数据 | 用户裁定 | — |

---

## 2. 现状：三个信号各自为政

```
pnpm dev  ── tauri dev ───► debug profile   ⇒ debug_assertions = true   ┐
                            vp dev          ⇒ import.meta.env.DEV=true  ┤ 数据根 ~/.dev-team
pnpm build ─ tauri build ─► release profile ⇒ debug_assertions = false  ┤ （两边完全一样）
                            vp build        ⇒ import.meta.env.DEV=false ┘   ↑ 无任何信号管它
```

仓库里已有的"模式痕迹"：

| 面 | 位置 | 现状 |
|----|------|------|
| 数据根常量 | `src-tauri/src/main.rs:16` `const DATA_DIR = ".dev-team"` | 单一常量；`setup` 里 `home_dir()?.join(DATA_DIR)` 注入 `WorkspaceStores::open`，store 侧零环境解析（`store.rs:2216`） |
| 数据根下三样东西 | `desktop-global.redb` / `workspaces/` / `logs/` | 全挂数据根下 ⇒ **根一分裂，三者自动分裂**，store crate 一行不改 |
| devtools 插件 | `main.rs:38` `#[cfg(debug_assertions)]` | 已有 profile 判别 |
| 控制台窗口 | `main.rs:1` `cfg_attr(not(debug_assertions), windows_subsystem="windows")` | debug 保留控制台 |
| updater 检查 | `src/hooks/use-updater.ts:64` `if (!import.meta.env.DEV)` | 用 **vite** 信号 |
| 单实例锁 | `tauri-plugin-single-instance`，Windows 用 `identifier` 派生具名 mutex | 标识 `dev.wps.dev-team`，**与已装生产版互斥** |
| 打包面 | `tauri.conf.json`：`bundle.targets:"all"`、`createUpdaterArtifacts:true` | 本地 `build` 强制要 `TAURI_SIGNING_PRIVATE_KEY`（RELEASE.md 明写） |
| 发版 | `.github/workflows/desktop-release.yml` → `tauri-action` | action 直接调 tauri CLI，**不经过 `pnpm build`** ⇒ D2 不影响 CI |

原始诉求：

| # | 诉求 | 本质 |
|---|------|------|
| 诉求1 | build 默认开发模式，cli 参数才生产 | 构建入口默认值倒置 |
| 诉求2 | 分离 dev / prod 数据库，且 dev 构建与 `pnpm run dev` 共库 | 数据根模式化 |
| 诉求3 | agent 验收用 debug 模式，不受运行中的生产版阻塞 | 第三档运行时（env 覆盖） |

---

## 3. 实测事实基座（决定一切设计的硬数据）

### 3.1 三档信号矩阵 —— 本探索最关键的一张表

| 启动方式 | custom-protocol | cargo profile | `cfg!(debug_assertions)` | `tauri::is_dev()` | `import.meta.env.DEV` | `TAURI_ENV_DEBUG` |
|---|---|---|---|---|---|---|
| `tauri dev` | 否（CLI 不加） | dev | **true** | true | true | `"true"` |
| `tauri build --debug` | **是** | dev | **true** | **false** | **false** | `"true"` |
| `tauri build` | 是 | release | false | false | false | **键完全缺失** |

四条硬结论：

1. **`tauri::is_dev()` 是陷阱**。实现为 `!cfg!(feature="custom-protocol")`（`tauri/src/lib.rs:317`），
   回答"我是不是 build 出来的"，不是"我在哪种模式"——`build --debug` 下返回 **false**。
2. **`import.meta.env.DEV` 是陷阱**。回答"前端是否由 dev server 服务"，dev 构建恒 false。
   现状 `use-updater.ts:64` 据此判定 ⇒ dev 构建会去检查生产 release、点一下就把生产版装上来。
3. **`TAURI_ENV_DEBUG` 在 release hook 里不存在**（不是空串、不是 `"false"`）⇒ vite 侧只能写
   `=== 'true'`，写 `!== 'false'` 会中招。
4. `tauri dev` 与 `build --debug` 的 `debug_assertions` **同为 true** ⇒ "dev 构建与 `pnpm run dev`
   共库"是**免费推论**，诉求2 由此闭合，不需要任何额外机制。

### 3.2 `tauri build --debug` 就是一次真构建

```
Running [tauri_cli] Command `cargo build --bins --features tauri/custom-protocol`
Finished `dev` profile [unoptimized + debuginfo] target(s) in 43.10s
```

- `--features tauri/custom-protocol` 在 `--debug` 下同样注入。
- 产物内嵌前端资源：在 `dev-team.exe` 里逐字找到 `dist/index.html` 引用的
  `assets/index-Cu_NNg4_.js` ⇒ **dev 构建的 exe 可脱离 dev server 直接双击跑**，
  "交付一份能双击的 dev 产物"成立。

### 3.3 dev 构建带控制台（PE subsystem 实测）

```
dev-team.exe (debug profile) → PE subsystem = 3 (CONSOLE)
```

两面看：好的一面是 panic / `eprintln` / `--self-check` 的 stdout 天然可读（正是 D8 要的）；
不好的一面是双击多一个黑窗。**建议保持现状**——为外观改 GUI subsystem 会掐掉 stdout，代价大于收益。

### 3.4 `setup()` 是"先建窗、后跑钩子"

`tauri-2.12.2/src/app.rs:2688`：

```rust
fn setup<R: Runtime>(app: &mut App<R>) -> crate::Result<()> {
  for window_config in app.config().app.windows.iter().filter(|w| w.create) {
    WebviewWindowBuilder::from_config(app.handle(), window_config)?.build()?;  // ← 先建窗
  }
  app.manager.assets.setup(app);
  if let Some(setup) = app.setup.take() { (setup)(app)?; }                    // ← 后跑用户钩子
}
```

⇒ **任何"零窗口自检"都不能放 `setup` 里**（窗口已在钩子前建好，一闪即退还抢焦点）。
唯一干净的位置是 `Builder::build()` 与 `App::run()` 之间：`build()` 初始化运行时但不建窗口。

### 3.5 `home_dir` 不需要 Tauri 上下文之外的实现

`tauri/src/path/desktop.rs:153`：`home_dir()` 就是 `dirs::home_dir()`。`App` 未 `run` 也实现
`Manager` ⇒ `app.path().home_dir()` 在 `build()` 之后即可用，**无需新增 `dirs` 依赖，也无需
第二份 home 实现**。

### 3.6 "无法访问 db" 其实是两把锁

```
① redb 文件锁    库文件级单进程写（store.rs:430「跨进程并行 run 不可达」）
② 单实例 mutex   Windows: `{identifier}-sic` 具名 mutex + 隐藏窗口类
                 （tauri-plugin-single-instance platform_impl/windows.rs:59-75）
                 后启动者 → FindWindowW 唤起前者 → 自己退出
```

- 锁① 由数据根分裂**自动**解掉：workspace 库文件名由 canonical root 派生（`store.rs:391`），
  同一个 workspace 在两边是**同名文件、不同父目录**，天然互不可见。
- 锁② 由 D3 解掉（dev 不注册插件）。

---

## 4. 设计

### 4.1 启动序列（唯一形状）

```rust
fn main() {
    let want_self_check = std::env::args().skip(1).any(|a| a == "--self-check");

    // release 侧：参数存在即立刻失败退出（关键，见 §5 陷阱 T6）
    #[cfg(not(debug_assertions))]
    if want_self_check {
        eprintln!("--self-check 仅在 dev 构建可用（当前为生产构建）。");
        std::process::exit(2);
    }

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build());
    #[cfg(all(desktop, not(debug_assertions)))]      // ← D3：dev 不注册
    { builder = builder.plugin(tauri_plugin_single_instance::init(...)); }
    #[cfg(debug_assertions)]
    { builder = builder.plugin(tauri_plugin_devtools::init()); }
    builder = builder.plugin(tauri_plugin_dialog::init()).setup(setup_hook);

    let app = builder.build(tauri::generate_context!("tauri.conf.json"))
        .expect("desktop 应用构建失败");             // ← 零窗口

    // 数据根解析：此处才有 home，且 boot-error.log 可写（§4.2 / §4.5）
    let home = app.path().home_dir().expect("home 目录解析失败");
    let data_root = match env::resolve_data_root(
        &home,
        std::env::var_os(env::DATA_ROOT_ENV),
        cfg!(debug_assertions),
    ) {
        Ok(v) => v,
        Err(err) => {
            let msg = env::render_root_error(&err, &home);
            eprintln!("{msg}");
            env::append_boot_error(&home, &msg);
            std::process::exit(2);
        }
    };
    app.manage(data_root.clone());                   // ← 单点计算，setup 与 self-check 共用

    #[cfg(debug_assertions)]
    if want_self_check {
        std::process::exit(selfcheck::run(&app, &data_root.0));   // ← 零窗口
    }

    app.run(|_, _| {});
}
```

要点：

- **数据根只算一次**：`app.manage()` 后 `setup` 与 `self-check` 共用同一份 ⇒ "两处看到同一个根"
  由类型保证，不靠纪律。
- 单实例 / devtools 全部在 `builder` 装配期按 cfg 分流，运行期零判断。
- `setup` 内只做真正需要 runtime 的事：`WorkspaceStores::open(data_root)`、engine log、
  panic hook、各注册表。

### 4.2 数据根解析：纯函数 + 三分优先级

```rust
// src-tauri/src/env.rs（shell 新模块，与 commands/ 平级；配 env_test.rs）
pub const PROD_DIR: &str = ".dev-team";
pub const DEV_SUBDIR: &str = "dev";
pub const DATA_ROOT_ENV: &str = "DEV_TEAM_DATA_ROOT";

pub enum RootSource { Prod, DevDefault, DevEnv }

/// debug  && env 有值 → env（校验失败即 Err）
/// debug  && env 无值 → <home>/.dev-team/dev
/// release           → <home>/.dev-team（调用点必传 None，结构上无法被重定向）
pub fn resolve_data_root(
    home: &Path,
    env: Option<&OsStr>,
    is_debug: bool,
) -> Result<(PathBuf, RootSource), RootError>;
```

- release 分支**不接收** env（签名强制）⇒ "不读"由类型保证，不靠纪律。
- `RootSource` 让 self-check 摘要与 IPC 都能回报"根从哪来"。

### 4.3 env 校验项与提示语（逐条可测）

| 场景 | 判据 | 提示语要点 |
|------|------|-----------|
| 空 / 纯空白 | `trim().is_empty()` | 删掉变量用默认根，或传绝对路径 |
| 相对路径 | `!is_absolute()` | 回显收到的值 + 给可照抄的示例（**不做相对解析**，cwd 不可靠） |
| 等于 prod 根 | 归一后 `== <home>/.dev-team` | 拒绝——正是防验收污染生产库 |
| 是 prod 根的祖先（家目录、盘符根） | 归一后 `<home>/.dev-team` 以 `<env>/` 为前缀 | 请用独立子目录 |
| 已存在但是文件 | `is_file()` | 说明要目录不要文件 |

归一化要点（易错）：**不能用 `canonicalize`**——目标目录通常还不存在，会 `NotFound`。
用 `Path::components()` 做逻辑归一（吃 `.` / `..` / 尾分隔符），再与同样归一的 prod 根比较。
这与 store 侧 `canonical::canonical_key`（要求路径存在）是**两件事**，spec 需写清避免误用。

提示语三条硬要求（否则 agent 修不回来）：

1. **指名变量**（`DEV_TEAM_DATA_ROOT`），不要只说"环境变量"；
2. **回显收到的值**，让 agent 看见自己错在哪；
3. **给出可照抄的下一步**（删变量 / 换独立目录 / 示例路径）。

### 4.4 `--self-check` 契约

面向 agent 消费：**逐行 `key=value`，无装饰**，退出码即结论。

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

| 退出码 | 含义 |
|--------|------|
| 0 | 全通 |
| 1 | 数据根 / 库打开失败（附 `error=` 行） |
| 2 | 用法或环境非法：env 校验失败，或 release 构建被传 `--self-check` |

实现要点：

- `models=` 一行来自 `Store::list_models()`（`store.rs:2144`）⇒ 免费获得"模型注册齐不齐"断言面。
- **别在 `std::process::exit` 前忘了 flush**：agent 以管道捕获 stdout 时 `println!` 是块缓冲，
  直接 exit 会丢输出。要么走"返回码 → main 正常返回"，要么显式 flush。
- `--self-check --root <path>`（顺带探测某 workspace 库）**本期不做**，避免与 env 语义打架。

### 4.5 失败可见性：双写

```
env 校验失败 ─┬─► stderr（命令行启动时 agent 可捕获；dev 构建是 CONSOLE subsystem）
              └─► <home>/.dev-team/logs/boot-error.log（append-only）
```

- 锚点选 `<home>/.dev-team/logs/`：env 已被判无效，不能拿它当落点，prod 根是唯一稳定锚。
  这是**日志不是数据**，不违反 D6。
- GUI 启动与 self-check 共用 `render_root_error(err, home)` 渲染，只差"往哪写"。
- D11 留痕建议写同目录下独立文件 `logs/ignored-env.log`，行形如
  `ignored_env=DEV_TEAM_DATA_ROOT (release build ignores it)`——**不混进引擎异常日志**。
  检测只需 `var_os().is_some()`，不解析值，不违背"不读"的实质。

### 4.6 前端 mode 穿透

```
Rust: cfg!(debug_assertions) ──► app_environment() ──► specta ──► TS 绑定
                                       { mode: 'dev'|'prod', dataRoot, source }
前端: useUpdater（判据从 import.meta.env.DEV 换成 mode === 'prod'）
      app.tsx ShellHeader（可选 DEV 徽标）/ info 页显示数据根
```

- mode 未就绪前 updater **不判定**（IPC 异步，首帧拿不到）。
- 可选增强（按需取）：`DEV` 徽标；info 页展示数据根与库路径（解决"两个 app 同跑我看的是哪个库"）；
  版本号显示 `v0.5.30-dev`（仅显示层，不动 `package.json`，避免污染 tag / updater 比较）。
- 已知摩擦：`src/app.test.tsx:346/375` 有严格的 invoke 调用序列断言（`every(...)` / `toEqual([...])`），
  挂载期多一次 `app_environment` 会打破，需改 `filter` 口径或前置 mock。规模不大但要算工量。

### 4.7 构建脚本与 target 目录

```diff
- "build": "pnpm run bindings:export && pnpm run server:build",
- "server:build": "cross-env TAURI_SIGNING_PRIVATE_KEY_PASSWORD= tauri build",
+ "build": "pnpm run bindings:export && pnpm run server:build:dev",
+ "server:build:dev": "cross-env CARGO_TARGET_DIR=src-tauri/target/dev-build tauri build --debug --no-bundle",
+ "build:prod": "pnpm run bindings:export && pnpm run server:build",
+ "server:build": "cross-env TAURI_SIGNING_PRIVATE_KEY_PASSWORD= tauri build",
```

- `--no-bundle` ⇒ 不打包 ⇒ **dev 构建不再需要 `TAURI_SIGNING_PRIVATE_KEY`**，
  RELEASE.md 那套私钥前置流程从此只约束 `build:prod`。
- 独立 `CARGO_TARGET_DIR` 的理由：`tauri dev` 不加 `custom-protocol`、`build --debug` 加，
  两者若共用 `target/debug`，feature 翻转会让 tauri 及下游全量重编译。

```
target/debug/       ← tauri dev（无 custom-protocol）+ bindings:export（同 feature 态）
target/dev-build/   ← tauri build --debug（有 custom-protocol）
```

- `bindings:export` 走 `cargo run -p export-bindings`，该 crate 依赖 `dev_team`（含 tauri），
  编译的是 `target/debug` 里无 custom-protocol 的那份 ⇒ **与 `tauri dev` 同态、稳定，无需改动**。
- 代价：同一依赖树编两份。`target/` 已在 `packages/desktop/.gitignore:9`，无仓库影响。
  嫌重就删掉那个 env，退回"忍抖动"。

---

## 5. 陷阱清单

| # | 陷阱 | 症状 | 处置 |
|---|------|------|------|
| T1 | 用 `tauri::is_dev()` 当判据 | dev 构建被判成 prod | 只用 `cfg!(debug_assertions)`（§3.1） |
| T2 | 用 `import.meta.env.DEV` 当判据 | dev 构建去检查生产 release，点一下装上生产版 | 走 IPC mode（§4.6） |
| T3 | vite 侧写 `TAURI_ENV_DEBUG !== 'false'` | release 下该键不存在 ⇒ 判成 dev | 写 `=== 'true'`（或干脆不用 vite 信号） |
| T4 | 把 self-check 放 `setup` 钩子 | 窗口已建、一闪即退、抢焦点 | 放 `build()` 与 `run()` 之间（§3.4） |
| T5 | 用 `canonicalize` 归一 env 路径 | 目标目录不存在 ⇒ `NotFound` 误报 | `Path::components()` 逻辑归一（§4.3） |
| T6 | 用 `#[cfg]` 把 self-check 分支整块删掉 | **release 传 `--self-check` 会静默启动 GUI**，agent 挂到超时 | release 侧保留"参数存在即 exit 2"显式拒绝（§4.1） |
| T7 | `exit` 前不 flush stdout | agent 捕获到空输出，只看到退出码 | 显式 flush 或正常返回（§4.4） |
| T8 | 人与 agent 共用同一个 dev 根 | 第二个进程在 `WorkspaceStores::open` redb 锁 fail fast，像应用 bug ⇒ 验收假红 | 验收恒设 `DEV_TEAM_DATA_ROOT` 到独立目录（D7 验收红线） |
| T9 | 设了 env 却误跑 `build:prod` | env 被忽略，数据写进真实生产库且无声 | release 侧留痕（D11） |
| T10 | 让 CI 走 `pnpm build` | 默认已改 dev ⇒ CI 悄悄产 dev 构建 | spec 红线：CI MUST 直接调 `tauri build`（现状 tauri-action 正是如此） |
| T11 | `Cargo.toml` 的 `[features] custom-protocol` 注释 | 注释称"生产构建经 tauri build 启用"，诱导拿它当判据 | 可选清理；注释已与事实不符（§3.2） |
| T12 | 嵌套根的语义错位 | 切模式后 dev 看到空库，而 `~/.dev-team` 里那套旧 dev 数据被 release 侧继承当作**生产数据** | 已在 D5 接受；实现期提示一次即可 |

---

## 6. 影响面（唯一权威清单）

| 文件 | 变动 |
|------|------|
| `packages/desktop/package.json` | `build`→dev（`--debug --no-bundle` + 独立 `CARGO_TARGET_DIR`）；新增 `build:prod` |
| `src-tauri/src/main.rs` | 启动序列重排（§4.1）；单实例 `cfg(all(desktop, not(debug_assertions)))`；release 侧 self-check 拒绝 |
| `src-tauri/src/env.rs` + `env_test.rs`（新） | `resolve_data_root` / `RootSource` / `RootError`（自然语言 `Display`）/ `render_root_error` / `append_boot_error` / 逻辑归一 |
| `src-tauri/src/selfcheck.rs` + `selfcheck_test.rs`（新） | 摘要渲染（渲染纯函数可测，IO 部分不可） |
| `src-tauri/src/commands/app_env.rs`（新轨道或并入 config 轨）+ `commands/mod.rs` | `app_environment()` → `{ mode, dataRoot, source }`；`all_commands!` 增一行 |
| `src/hooks/use-updater.ts` | 判据换成 IPC mode，mode 就绪前不判定 |
| `src/app.test.tsx` | invoke 序列断言适配（§4.6） |
| `src/app.tsx`（可选） | `DEV` 徽标 |
| `src/types/generated/**` | 重新 `bindings:export` |
| `RELEASE.md` | 私钥前置仅约束 `build:prod`；dev 构建零配置 |
| `openspec/specs/desktop-workspace-store/spec.md` | 数据根从"恒 `home_dir()/.dev-team/`"改为"prod 根 + debug 子根 `dev/`" |
| `openspec/specs/desktop-release-pipeline/spec.md` | 红线：CI 恒裸 `tauri build`（MUST NOT 走 `pnpm build`）⇒ 恒 prod 模式，env 不生效 |
| 新 capability `desktop-build-modes` | 判别源单点、数据根三分优先级、dev 不注册单实例、self-check 契约、前端 mode 穿透、失败双写、release 留痕 |

---

## 7. 待确认（仅剩实现期验证，均不阻塞设计）

- [ ] debug 二进制下 agent 编排 / tokei / 大库扫描的可忍度。不忍 ⇒ 判据换 `cfg!(feature="dev-mode")`，
      §4 全部结构不变，只把 `debug_assertions` 换成 feature 名（多一层开关，换 release 优化的 dev 产物）。
- [ ] "dev 不注册单实例 ⇒ Windows 零残留 mutex / 窗口类" —— 读码结论，实操验一次。
- [ ] macOS 上 `build()` 后直接 `exit` 是否触发 `NSApplication` 告警（`EventLoop::new()` 在主线程，
      预期无风险，顺手一眼）。
- [ ] `app.test.tsx` 序列断言改造的实际规模。

---

## 8. 交付边界（进 change 建议）

一次 change 覆盖 8 条 requirement，新 capability `desktop-build-modes`：

1. **判别源单点**：`cfg!(debug_assertions)`；MUST NOT 用 `tauri::is_dev()` / `import.meta.env.DEV`
   （§3.1 矩阵作 rationale 入 spec）。
2. **构建脚本默认 dev**：`build` = `--debug --no-bundle`，`build:prod` = 现行 release + bundle；
   CI MUST 直接调 `tauri build`。
3. **数据根三分优先级 + 全项校验 + 自然语言提示**（含"不得落在 prod 根或其祖先"红线）。
4. **dev 构建不注册单实例**（换来 dev 与已装生产版并存）。
5. **`--self-check`**：仅 debug 有实现、零窗口、`key=value` 输出、退出码契约；release 传参立即 exit 2。
6. **前端 mode 穿透**：`app_environment()` IPC（含 dataRoot / source），updater 判据随之切换。
7. **失败信息双写**：stderr + `logs/boot-error.log`。
8. **release 侧 env 检测留痕**（不阻断）。

验收红线（建议写进 spec）：**自动化验收启动 desktop 必须显式设置独立 `DEV_TEAM_DATA_ROOT`**，
MUST NOT 复用人工 dev 根。

配套 spec 增量见 §6 末三行；若偏好"只加新 capability 不改旧 spec"，则三条增量并入
`desktop-build-modes` 的 Module Contract 说明。

---

## 附A 被否方案（保留决策依据）

**A1 判别源三选一**：①profile / ②cargo feature `dev-mode` / ③build.rs 注入 env 常量。选 ①。
② 的唯一价值是"release 优化 + dev 数据面"，留作 §7 第一条的退路；③ 前端要二次推导，必分叉。

**A2 数据根落位**：兄弟根 `.dev-team-dev`（曾选，后改）/ 嵌套 `.dev-team/dev`（**最终**）/
prod 搬家 `.dev-team-prod`（破坏已装用户数据，否）。

**A3 前端穿透**：①IPC（**最终**）/ ②vite `define`（两处推导同一事实 + release 下该键不存在）。

**A4 验收入口**：A `pnpm accept` + WebDriver / B `--self-check`（**最终**）。A 能验界面但链路长，
留作后续；两者共用同一套数据根 / env 约定。

**A5 单实例解法**：dev 不注册（**最终**）/ 改 identifier（重手段：编译期烧进 binary，
要 `--config` 变体 + 产物改名 + updater endpoint 覆盖 + tauri-build 重编译）。

---

## 附B 探索期间的四处自我纠正（留痕，防后人重踩）

1. **数据根从"兄弟根"改为"嵌套"**：初判兄弟根更清晰；用户裁定嵌套（同屋檐）。
   副作用已在 T12 记录。
2. **self-check 位置从 `setup` 钩子改为 `build()` 与 `run()` 之间**：读 `app.rs:2688` 才发现
   窗口在用户钩子**之前**创建，原方案会闪窗。
3. **env 校验从"忽略并记因"改为硬失败 + 自然语言**：agent 无从得知"被忽略了"，
   必须让它能读原因、改参数、重试。
4. **release 侧 self-check 不能只靠 cfg 删分支**（整理本笔记时发现）：若整块 `#[cfg]` 掉，
   release 传 `--self-check` 会**静默启动 GUI**，对 agent 是最坏结果（挂到超时）。
   正解是 release 保留"参数存在即 exit 2"的显式拒绝——自检**实现**不存在，但参数**被识别并拒绝**。
