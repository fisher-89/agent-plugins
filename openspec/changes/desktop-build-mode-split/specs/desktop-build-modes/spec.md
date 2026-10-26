# desktop-build-modes Specification

## Purpose

定义 desktop 壳层的**构建模式维度**：以单一判别源（`cfg!(debug_assertions)`）沿模式分裂构建脚本默认值、数据根落位、单实例注册、`--self-check` 自检入口与前端 mode 表现，并把启动失败可见性、prod 侧环境变量留痕与启动序列单点钉死。本能力不改变任何业务语义——store / 引擎 / 编排只认识"某个数据根"，不认识"根从哪来"。

## ADDED Requirements

### Requirement: 构建模式判别源单点

desktop 的构建模式（`dev` / `prod`）SHALL 由**唯一判别源**派生：Rust 侧 `cfg!(debug_assertions)`（含等价的 `#[cfg(debug_assertions)]` / `#[cfg(not(debug_assertions))]` 属性形态，判定结果集中在 `env::mode()`），前端侧只经 IPC 取得（见「前端 mode 穿透」）。

MUST NOT 以 `tauri::is_dev()`、`import.meta.env.DEV` 作模式判据；MUST NOT 引入 build.rs 注入的模式常量或第二条推导链。三档启动方式的信号矩阵 SHALL 作为本条判别依据（实测：cargo 1.95 / rustc 1.95 / tauri-cli 2.12.4 / tauri 2.12.x（仓内 pin 2.12.5）/ Windows）：

| 启动方式 | custom-protocol | cargo profile | `cfg!(debug_assertions)` | `tauri::is_dev()` | `import.meta.env.DEV` | `TAURI_ENV_DEBUG` |
|---|---|---|---|---|---|---|
| `tauri dev` | 否 | dev | **true** | true | true | `"true"` |
| `tauri build --debug` | 是 | dev | **true** | **false** | **false** | `"true"` |
| `tauri build` | 是 | release | false | false | false | **键完全缺失** |

- `tauri::is_dev()` 回答"是否为 build 产物"（实现为 `!cfg!(feature="custom-protocol")`），dev 构建下为 false ⇒ MUST NOT 用作判据；
- `import.meta.env.DEV` 回答"前端是否由 dev server 服务"，dev 构建下恒 false ⇒ MUST NOT 用作判据；
- `TAURI_ENV_DEBUG` 在 release 下**键不存在**（非空串、非 `"false"`）⇒ 若前端消费 vite 信号 MUST 以 `=== 'true'` 收敛，MUST NOT 写 `!== 'false'`（本能力不引入该链路）；
- `tauri dev` 与 `tauri build --debug` 的 `debug_assertions` 同为 true ⇒「dev 构建与 `pnpm dev` 共库」是本条的免费推论，无需额外机制。

#### Scenario: dev 构建判为 dev

- **WHEN** 以 `pnpm build`（= `tauri build --debug`）构建并运行
- **THEN** 数据根、单实例注册、`--self-check` 可用性、IPC `mode` 全部取 dev 侧取值；`tauri::is_dev()` 的 false 与 `import.meta.env.DEV` 的 false 均不影响判定

#### Scenario: release 构建判为 prod

- **WHEN** 以 `pnpm build:prod` 或 CI 的 `tauri build` 构建并运行
- **THEN** 数据根、单实例注册、自检可用性、IPC `mode` 全部取 prod 侧取值；进程内不存在任何把该产物判成 dev 的判据

#### Scenario: 第二判别源零存在

- **WHEN** 扫描 `packages/desktop/src-tauri/src/**` 与 `packages/desktop/src/**`（排除 `src/types/generated/**`）
- **THEN** 无 `tauri::is_dev()` 调用、无 `import.meta.env.DEV` 模式判据、无 build.rs 模式常量；模式判定只出现在 `env::mode()`（Rust 单点）与 `app_environment` IPC（前端唯一来源）

### Requirement: 构建脚本默认 dev 与显式生产入口

`packages/desktop/package.json` 的构建脚本 SHALL 按模式分裂，且默认值 SHALL 倒置为 dev：

| 脚本 | 内容要点 | 模式 |
|---|---|---|
| `build` | `bindings:export` + `server:build:dev` | dev |
| `server:build:dev` | `tauri build --debug --no-bundle`，且 cargo target 目录指向隔离目录 | dev |
| `build:prod` | `bindings:export` + `server:build` | prod |
| `server:build` | `cross-env TAURI_SIGNING_PRIVATE_KEY_PASSWORD= tauri build`（现行语义不变） | prod |

- dev 构建 MUST `--no-bundle`：不打包 ⇒ MUST NOT 需要 `TAURI_SIGNING_PRIVATE_KEY`；签名私钥前置从此**只约束 `build:prod`**；
- dev 构建产物 SHALL 自包含可双击运行（`--debug` 下同样注入 `tauri/custom-protocol` 且前端资源内嵌），MUST NOT 依赖 dev server；
- dev 构建的 cargo target 目录 MUST 与 `tauri dev` 的 `target/debug` 分离（`tauri dev` 不加 `custom-protocol`、`build --debug` 加，共用目录会让 feature 翻转触发 tauri 及下游全量重编译）；隔离目录 MUST 落在 gitignore 覆盖的 `src-tauri/target/` 之内；具体形态（env 变量 / CLI 参数）由实现定夺，**隔离语义不变**；
- `bindings:export` SHALL 保持独立运行且不落隔离目录——它走无 `custom-protocol` 的编译态，与 `tauri dev` 同态；
- `dev` / `client:dev` / `check` / `test` / `mutate` / `tauri` 脚本与 `tauri.conf.json` 全项 MUST NOT 改动。

#### Scenario: 默认构建落 dev

- **WHEN** 在未设任何签名环境变量的机器上执行 `pnpm -C packages/desktop run build`
- **THEN** 构建成功且不产出安装包；产物为可双击运行的 dev 二进制，其运行期 `mode=dev`、数据根为 dev 子根

#### Scenario: 生产构建仍走私钥前置

- **WHEN** 执行 `pnpm -C packages/desktop run build:prod` 且未设 `TAURI_SIGNING_PRIVATE_KEY`
- **THEN** 构建如现状失败（签名前置未被削弱）；设齐 `TAURI_SIGNING_PRIVATE_KEY` 与空的 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` 后产出现行同形态的安装包与 updater 工件

#### Scenario: target 目录隔离

- **WHEN** 依次执行 `pnpm dev`（`tauri dev`）与 `pnpm build`
- **THEN** 两者分别落 `src-tauri/target/debug` 与隔离目录，互不因 feature 翻转触发对方全量重编译；两目录均在 gitignore 覆盖范围内

### Requirement: 数据根三分优先级与路径校验

数据根 SHALL 由**纯函数**解析，判别与落位如下（唯一计算点见「启动序列与数据根状态单点」）：

| 构建模式 | `DEV_TEAM_DATA_ROOT` | 数据根 | `source` |
|---|---|---|---|
| dev | 未设 | `<home>/.dev-team/dev` | `dev-default` |
| dev | 已设且通过校验 | 该值（逻辑归一后） | `env` |
| dev | 已设但校验失败 | —（硬失败，退出码 2） | — |
| prod | 任意 | `<home>/.dev-team` | `prod` |

- 常量 SHALL 单点：prod 目录名 `.dev-team`、dev 子目录名 `dev`、变量名 `DEV_TEAM_DATA_ROOT` 均由 `env` 模块定义；壳层其他位置 MUST NOT 复写这些字面量；
- 解析入口 SHALL 分为 `prod_root(home)` 与 `resolve_dev_root(home, env)` 两条：prod 侧**签名不接收** env ⇒「prod 不读该变量」由结构保证，MUST NOT 仅靠调用点纪律；
- 校验项 SHALL 全项覆盖，任一项不通过即拒绝：① trim 后为空 / 纯空白；② 非绝对路径（MUST NOT 按 cwd 解析）；③ 归一后**等于** prod 根；④ 归一后是 prod 根的**严格祖先**（家目录、盘符根、任一层上级）；⑤ 已存在但是**普通文件**（需要目录）；其余接受；
- 归一化 MUST 用 `Path::components()` 做**逻辑归一**（吃 `.` / `..` / 重复分隔符 / 尾分隔符，保留根与前缀），MUST NOT 用 `canonicalize`——目标目录通常尚不存在，会 `NotFound` 误报。该归一与 store 的 `canonical::canonical_key`（要求路径存在，用于 workspace 库文件名派生）是**两件事**，MUST NOT 互相替代、MUST NOT 合并为一份实现；
- 「等于」与「祖先」判定 SHALL 走同一份归一结果比较，Windows 下 SHALL 大小写不敏感（盘符 / 用户名大小写差异 MUST NOT 绕过红线）；
- 拒绝提示 SHALL 为自然语言并同时满足三要素：指名变量 `DEV_TEAM_DATA_ROOT`、**回显收到的值**、给出可照抄的下一步（删掉变量改用默认根 / 换一个独立子目录 / 示例绝对路径）；MUST NOT 只输出枚举名、错误码或裸路径比较结论；
- 解析失败 SHALL 硬失败（不起 GUI、MUST NOT 降级到默认根），处置见「启动失败可见性双写」；
- 数据根 SHALL 以单一值 `DataRoot { path, source }` 承载，`source` 值域 `prod` / `dev-default` / `env`，供自检摘要与 IPC 回报来源；
- **已知漏面**：junction / symlink / 8.3 短名指向 prod 根时逻辑归一看不穿 ⇒ 祖先判定可被刻意构造绕过；此为本条知情边界（MUST NOT 被当作安全缺口遗漏），真兜底是「验收用独立目录」红线。

#### Scenario: dev 默认根

- **WHEN** dev 构建启动且未设 `DEV_TEAM_DATA_ROOT`
- **THEN** 数据根为 `<home>/.dev-team/dev`、`source=dev-default`；全局库、`workspaces/`、引擎日志 `logs/` 三者全落该子根，prod 根零新增写入

#### Scenario: env 覆盖成立

- **WHEN** dev 构建启动且 `DEV_TEAM_DATA_ROOT` 设为 `<home>/tmp/accept-1`（绝对路径，非 prod 根及其祖先，且不存在）
- **THEN** 数据根为该目录、`source=env`；其数据与默认 dev 根互不可见；目录不存在不构成失败

#### Scenario: 污染生产库的路径被拒

- **WHEN** `DEV_TEAM_DATA_ROOT` 设为 `<home>/.dev-team`、`<home>`、盘符根，或仅大小写不同的等价形态（如 `<home>\.DEV-TEAM`）
- **THEN** 启动硬失败、退出码 2；提示回显收到值，并说明该路径会污染生产数据根

#### Scenario: 相对路径与空白被拒

- **WHEN** `DEV_TEAM_DATA_ROOT` 设为 `dev-data`（相对）或纯空白
- **THEN** 启动硬失败、退出码 2；提示回显收到值并给出可直接照抄的绝对路径示例；MUST NOT 按 cwd 解析成相对根

#### Scenario: 目标是文件被拒

- **WHEN** `DEV_TEAM_DATA_ROOT` 指向一个已存在的普通文件
- **THEN** 启动硬失败、退出码 2；提示说明需要目录而非文件

#### Scenario: 归一化不依赖路径存在

- **WHEN** 以 `<base>/x/./y/../z/`（目标不存在）作为 env 值，并与 `<base>/x/z` 等价形态比较
- **THEN** 两者归一为同一路径、判定一致；不产生 `NotFound` 报错

### Requirement: 启动失败可见性双写

数据根校验失败 SHALL **双写**后退出：

- **stderr**：命令行启动（含 agent 以管道捕获）可读；dev 构建为 CONSOLE subsystem，`eprintln` 天然可见；
- **`<prod 根>/logs/boot-error.log`**：append-only，写前 SHALL 自建父目录（`logs/` 可能尚不存在）；IO 失败 MUST 静默（MUST NOT 掩盖原始失败原因）；双击启动时控制台随进程退出消失，该文件是唯一留存观测面。

约束：

- 落点 MUST 锚定 **prod 根**（`<home>/.dev-team/logs/`）：env 已被判无效，不能拿它当落点；这是**日志不是数据**，不违反「prod 不认该变量」；
- 两条通道 SHALL 由同一渲染函数产出同一段文字（只差"往哪写"），MUST NOT 各写一份文案；
- 启动失败日志 MUST NOT 与引擎异常日志（`<数据根>/logs/engine.log`，随数据根分裂）混写；
- 退出码 SHALL 为 2（用法或环境非法）。

#### Scenario: 双写落两处

- **WHEN** dev 构建以非法 `DEV_TEAM_DATA_ROOT` 启动
- **THEN** stderr 有完整体检提示；`<home>/.dev-team/logs/boot-error.log` 追加同一段文字（带时间戳前缀）；进程退出码 2；GUI 未起、无窗口闪现

#### Scenario: 日志目录缺席时自建

- **WHEN** 首次运行（`<home>/.dev-team/logs/` 不存在）且此时发现 env 非法
- **THEN** `logs/` 被创建、`boot-error.log` 写入成功；失败不因目录缺席而丢失

#### Scenario: 与引擎日志不混写

- **WHEN** 分别触发一次启动失败与一次引擎异常
- **THEN** 前者只出现在 `boot-error.log`，后者只出现在数据根下 `engine.log`（dev 模式下为 `<home>/.dev-team/dev/logs/engine.log`）

### Requirement: prod 构建的环境变量检测留痕

prod 构建 SHALL 检测 `DEV_TEAM_DATA_ROOT` 是否被设置（只需 `var_os().is_some()`，MUST NOT 解析值、MUST NOT 用它定位任何路径），且 MUST NOT 因此阻断启动、MUST NOT 改变数据根：

- 命中 SHALL 向 `<prod 根>/logs/ignored-env.log` 追加一行 `ignored_env=DEV_TEAM_DATA_ROOT (release build ignores it)`（append-only、尽力而为、IO 失败静默）；
- 该记录 MUST NOT 混入引擎异常日志；
- 动机：陈旧变量不该让安装版打不开（不阻断），但「设了 env 却误跑 `build:prod`」必须留痕，否则数据无声写进真实生产库。

#### Scenario: 设了 env 的安装版照常启动

- **WHEN** prod 构建启动且环境里残留 `DEV_TEAM_DATA_ROOT=<某目录>`
- **THEN** 启动成功、数据根仍为 `<home>/.dev-team`；`logs/ignored-env.log` 追加一行留痕；该环境变量指向的目录零写入

#### Scenario: 未设 env 不留痕

- **WHEN** prod 构建启动且环境里无 `DEV_TEAM_DATA_ROOT`
- **THEN** 不产生 `ignored-env.log` 记录（文件可不存在）

### Requirement: dev 构建不注册单实例插件

`tauri-plugin-single-instance` SHALL 仅在 prod 构建注册（`#[cfg(all(desktop, not(debug_assertions)))]`），dev 构建 MUST NOT 注册：

- 目的：Windows 下该插件以 `identifier` 派生具名 mutex（`dev.wps.dev-team-sic`）+ 隐藏窗口类，dev 构建与已装生产版会互相唤起 / 退出，agent 验收被运行中的生产版阻塞；
- MUST NOT 通过改 `identifier` 实现并存（编译期烧死：需 `--config` 变体 + 产物改名 + updater endpoint 覆盖 + tauri-build 重编译）；
- prod 侧行为 MUST 逐字不变：后启动者唤起前者主窗口（`unminimize` / `show` / `set_focus`）后自身退出；
- 数据面互斥的另一把锁（redb 文件锁）由数据根分裂**自动**解除：同一 workspace 在两模式下是**同名文件、不同父目录**，互不可见；store MUST NOT 为此新增任何跨进程锁或探测（沿 desktop-workspace-store「单进程约束显式化」）；
- `tauri_plugin_devtools` 的 `#[cfg(debug_assertions)]` 注册语义不变（dev 构建与控制台窗口同侧）。

#### Scenario: dev 与已装生产版并存

- **WHEN** 已安装的 prod 版正在运行，再启动 dev 构建
- **THEN** dev 进程照常起窗、不唤起前者也不退出；两进程各用各自数据根，无 redb 锁冲突、无 mutex 冲突

#### Scenario: prod 单实例语义不变

- **WHEN** prod 版已运行，再次双击 prod 产物
- **THEN** 后启动者唤起前者主窗口后自身退出（与现状逐字一致）

#### Scenario: identifier 零改动

- **WHEN** 审查 `tauri.conf.json` 与构建脚本
- **THEN** `identifier` 仍为 `dev.wps.dev-team`；无 `--config` 变体、无产物改名逻辑、无第二份 identifier 派生

### Requirement: `--self-check` 自检契约

dev 构建 SHALL 支持命令行自检入口 `--self-check`：零窗口启动、逐行 `key=value` 输出、退出码即结论。

- **位置**：MUST 在 `Builder::build()` 之后、`App::run()` 之前。tauri 的内部 `setup` 在 `RuntimeRunEvent::Ready` 执行且**先建窗、后跑用户钩子**（`app.rs:1443` / `app.rs:2688`），故 `setup` 钩子 MUST NOT 承载自检；自检路径 MUST 在 `run()` 之前退出进程；
- **输出**：键序 SHALL 固定为 `mode` / `home` / `data_root` / `data_root_source` / `global_db` / `global_db_open` / `workspaces_dir` / `workspaces_dir_exists` / `workspace_count` / `models`，失败追加 `error=<原因>`；MUST NOT 加横幅、ANSI 色、进度行或空行；
- `mode` / `data_root_source` 的线值 SHALL 与 IPC `app_environment` 同名同形（`dev` / `prod`；`prod` / `dev-default` / `env`），单点产出；
- `models=` SHALL 取自全局库已注册模型清单（`list_models()`）；`global_db=` / `workspaces_dir=` 的路径成分 MUST 取自 store 公开常量（MUST NOT 在壳层复写库文件名或子树名）；
- **退出码**：0 全通；1 数据根 / 库打开失败（附 `error=`）；2 用法或环境非法（env 校验失败，或 prod 构建被传 `--self-check`）；
- 进程退出前 SHALL 显式 flush stdout（以管道捕获时 `println!` 为块缓冲，直接 `exit` 会丢输出）；
- **prod 侧拒绝**：prod 构建 MUST 识别该参数并立即在 stderr 说明 + `exit 2`；MUST NOT 用 `#[cfg]` 把该分支整块删掉（否则 prod 传参会静默起 GUI，agent 挂到超时）；
- 本期 MUST NOT 支持 `--self-check --root <path>`（与 env 语义冲突，留后续）；
- **验收红线**：凡脚本 / agent 启动 desktop 做验收，MUST 显式设置 `DEV_TEAM_DATA_ROOT` 指向**独立**目录；MUST NOT 复用人工 dev 根（第二个进程会在 `WorkspaceStores::open` 的 redb 锁上 fail fast，形似应用故障的假红）。

#### Scenario: dev 构建自检全通

- **WHEN** 在 `DEV_TEAM_DATA_ROOT=<独立目录>` 下执行 `<dev 产物> --self-check`
- **THEN** stdout 为十行 `key=value`（含 `mode=dev`、`data_root_source=env`、`global_db_open=ok`、`workspace_count=<n>`、`models=<清单>`），退出码 0；全程无窗口出现

#### Scenario: 库打开失败

- **WHEN** 全局库文件不可打开（损坏 / 无权限）
- **THEN** 输出含 `global_db_open=err` 与 `error=<原因>` 行，退出码 1；输出经管道可完整读到（已 flush）

#### Scenario: env 非法时自检

- **WHEN** 在 `DEV_TEAM_DATA_ROOT=<home>/.dev-team` 下执行 `--self-check`
- **THEN** 走与 GUI 启动同一渲染的提示（stderr + `boot-error.log`），退出码 2，无 `key=value` 自检摘要

#### Scenario: prod 构建拒绝自检

- **WHEN** prod 产物被执行 `--self-check`
- **THEN** stderr 说明「`--self-check` 仅在 dev 构建可用（当前为生产构建）」并退出码 2；MUST NOT 起 GUI、MUST NOT 挂住等待

#### Scenario: 双进程验收不互相阻塞

- **WHEN** 人工 dev 实例运行中，验收脚本以独立 `DEV_TEAM_DATA_ROOT` 对 dev 产物做 self-check
- **THEN** 两进程同在、互不阻塞，自检退出码 0（红线生效）

### Requirement: 前端 mode 穿透（IPC 单源）

前端 SHALL 经单一 IPC 命令取得构建模式，MUST NOT 自行推导：

- 命令 `app_environment()` SHALL 返回 `{ mode: 'dev' | 'prod', dataRoot: string, source: 'prod' | 'dev-default' | 'env' }`；实现落 `commands/env/`（薄包装 + `*_inner` 微形态，经 Tauri 管理态取 `DataRoot`），并登记进 `all_commands!` 与 specta 生成物；
- `mode` / `source` 线值 MUST 与 `--self-check` 摘要同名同形（单点产出，MUST NOT 两处各拼字符串）；
- `useUpdater` 的启动更新检查判据 SHALL 由 `import.meta.env.DEV` 换成 `mode === 'prod'`；mode 未就绪（`null`）或 IPC 失败时 MUST NOT 判定、MUST NOT 回落成"照常检查"（现状判据会让 dev 构建去检查生产 release，点一下把生产版装上）；
- mode === `dev` 时壳层顶栏 SHALL 呈 `DEV` 徽标（带 `data-testid` 挂钩），其 `title` SHALL 携 `dataRoot`（解决"两个 app 同跑，我看的是哪个库"）；
- IPC 失败或载荷不合法 SHALL 静默（无徽标、不判定更新），MUST NOT 升级为壳态错误态、MUST NOT 阻塞其余取数；
- 版本号显示 MUST NOT 追加 `-dev` 后缀（不动 `package.json` 版本语义，避免污染 tag 与 updater 版本比较）。

#### Scenario: dev 构建不查更新

- **WHEN** dev 构建启动，`app_environment` 返回 `mode: 'dev'`
- **THEN** updater 的启动检查不发起（`check` 零调用），版本号照常拉取显示；顶栏出现 `DEV` 徽标，`title` 为 dev 数据根

#### Scenario: prod 构建照常检查

- **WHEN** prod 构建启动，`app_environment` 返回 `mode: 'prod'`
- **THEN** 启动检查照现状发起一次（静默失败语义不变）；无 `DEV` 徽标

#### Scenario: mode 未就绪不判定

- **WHEN** `app_environment` 尚未返回或 reject
- **THEN** 不发起 `check`、无徽标、无错误态；就绪后按实际 mode 行为一次

#### Scenario: 前端零自推导

- **WHEN** 扫描 `packages/desktop/src/**`（排除生成物）
- **THEN** 无 `import.meta.env.DEV` 模式判据、无 `TAURI_ENV_DEBUG` 推导；mode 来源只有 `useAppEnvironment` 经 IPC

### Requirement: 启动序列与数据根状态单点

`main` SHALL 按固定序列装配，模式相关分歧 MUST 在装配期以编译期 `#[cfg]` 完成，运行期零判断：

```
args 判定（--self-check）
  → prod 侧显式拒绝 --self-check（exit 2）
  → builder 装配（updater / 单实例[cfg] / devtools[cfg] / dialog / opener / setup_hook）
  → builder.build(generate_context!)          // 零窗口
  → home_dir（app.path()，无需 run）
  → 数据根解析（唯一计算点；失败即双写 + exit 2）
  → app.manage(DataRoot)                      // 单点，setup 与 self-check 共用
  → [cfg(debug_assertions)] 自检分支（exit）
  → app.run(...)
```

- 数据根 MUST 只解析一次并以管理态承载：`setup` 钩子 SHALL 经该管理态取根，MUST NOT 二次解析、MUST NOT 自行拼接 prod 目录名或 dev 子目录名；
- `setup` 钩子 SHALL 只做真正需要 runtime 的事（打开 `WorkspaceStores` / 引擎日志 / panic 钩子 / 各注册表挂载），MUST NOT 承载模式判定或自检；
- 数据根解析失败 SHALL 在 `main` 内显式处置（提示 + 双写 + 退出码 2），MUST NOT 经 `setup` 返回 `Err`（那会变成 `Failed to setup app` 的 panic 文本，丢掉自然语言提示）；
- `home_dir` SHALL 经 `app.path()`（与 `dirs::home_dir()` 同源），MUST NOT 新增 `dirs` 依赖、MUST NOT 造第二份 home 解析；`build()` 之后管理态与 `app.path()` 均可用（内部 `setup` 在 `run` 内 Ready 时才执行），该先后关系 SHALL 由本条约束固化；
- `WorkspaceStores::open` 失败 SHALL 保持 fail fast（`setup` Err → 启动失败），MUST NOT 降级为空清单或"无库"模式（既有口径不变；该失败面不在本条双写射程内）。

#### Scenario: 数据根单点计算

- **WHEN** 审查启动路径与 `setup` 钩子
- **THEN** 数据根解析恰一处（`main`），`setup` 与自检均取同一管理态值；prod 目录名字面量只出现在 `env` 模块常量一处，壳层其他位置零复写

#### Scenario: 解析失败不 panic

- **WHEN** env 校验失败
- **THEN** 进程输出自然语言提示、写 `boot-error.log`、退出码 2；stderr 不含 `Failed to setup app`、无窗口闪现

#### Scenario: 装配期分流

- **WHEN** 审查 builder 装配链
- **THEN** 单实例注册带 `not(debug_assertions)`、devtools 注册带 `debug_assertions`，两处均为编译期 `#[cfg]`；`run` 之后无 mode 分支

#### Scenario: 库打开失败仍 fail fast

- **WHEN** 数据根合法但全局库文件不可打开
- **THEN** 启动失败（不起空壳窗口、不静默降级），失败原因可从引擎日志 / stderr 追溯

## Module Contract

| 模块 | 面 | 关键契约 |
|---|---|---|
| `dev_team::env` | Function | `PROD_DIR` / `DEV_SUBDIR` / `DATA_ROOT_ENV` / `LOGS_DIR_NAME` 常量单点；`prod_root(home) -> DataRoot`（签名不接收 env）；`resolve_dev_root(home, Option<&OsStr>) -> Result<DataRoot, RootError>`；`resolve_root(home, is_dev)` 薄分派；`normalize()` 逻辑归一（components，禁 `canonicalize`）；`mode() -> "dev" \| "prod"`；`render_root_error(&RootError, &Path) -> String`；`append_boot_error(&Path, &str)` / `append_ignored_env(&Path)` |
| `dev_team::env::DataRoot` | Function | `{ path: PathBuf, source: RootSource }`；`RootSource` 值域 `Prod` / `DevDefault` / `DevEnv`，线值 `prod` / `dev-default` / `env`；作 Tauri 管理态单值承载 |
| `dev_team::selfcheck` | Function | `Summary` 十键 + `render(&Summary) -> String`（纯函数、键序固定）+ `exit_code(&Probe) -> i32` + `run(&App, &DataRoot) -> i32`（探库 → 渲染 → flush）；路径成分取 store 公开常量 |
| `dev_team`（`src/main.rs`） | Function | 固定启动序列（args → prod 侧拒绝 → builder 装配 → `build()` → 数据根单点解析 → `manage(DataRoot)` → 自检 exit → `run`）；`setup_hook` 命名函数经管理态取根；单实例 `#[cfg(all(desktop, not(debug_assertions)))]`；prod 侧 `--self-check` 显式拒绝且该分支不在 `#[cfg]` 内 |
| `dev_team::commands::env::app_environment` | API | IPC 命令，返回 `AppEnvironment { mode, dataRoot, source }`（camelCase 线面）；薄包装 + `app_environment_inner(&DataRoot)` 微形态；无 IO、无 `Result` 错误面（管理态恒在）；登记进 `all_commands!` |
| `store`（`crates/infra/store`） | API | `GLOBAL_DB_FILE_NAME` / `WORKSPACES_DIR_NAME` 由 `pub(crate)` 升 `pub` 并重导出：零行为改动，唯一用途是让自检摘要与 IPC 不以壳层字面量复写库文件名 / 子树名 |
| `packages/desktop/package.json` | CLI | `build`（dev）= `bindings:export` + `server:build:dev`；`server:build:dev` = `tauri build --debug --no-bundle` + 隔离 cargo target 目录；`build:prod` = `bindings:export` + `server:build`；`server:build` 保持现行 release + 签名语义；`dev` / `check` / `test` / `mutate` 不变 |
| `packages/desktop/src/hooks/use-app-environment.ts` | Component | 挂载一次 `commands.appEnvironment()`；成功返回 `{ mode, dataRoot, source }`，未就绪 / 失败为 `null`（静默，无错误态） |
| `packages/desktop/src/hooks/use-updater.ts` | Component | `useUpdater(mode: 'dev' \| 'prod' \| null)`；启动检查判据 `mode === 'prod'`；`null` 时不判定；删除 `import.meta.env.DEV` 判据；下载 / 安装链路零变化 |
| `packages/desktop/src/app.tsx` | Component | 挂 `useAppEnvironment()` → mode 传入 `useUpdater`；`ShellHeader` 在 `mode === 'dev'` 时渲染 `DEV` 徽标（`data-testid` + `title = dataRoot`）；版本号显示零 `-dev` 后缀 |
| `packages/desktop/src/types/generated/bindings.ts` | API | `bindings:export` 重导出：含 `appEnvironment` typed 包装与 `AppEnvironment` 类型；`bindings:check` 零 diff |
| `packages/desktop/RELEASE.md` | Component | 本地构建拆两档：`build` 零配置产 dev 二进制；签名私钥前置只约束 `build:prod`；注明 dev 数据根落 `<home>/.dev-team/dev` 与 `--self-check` 用法 |
| `.github/workflows/desktop-release.yml` | CLI | **零改动**（红线）：CI 恒经 tauri-action 直调 `tauri build`，MUST NOT 出现 `pnpm build` / `--debug` / `--no-bundle`；`DEV_TEAM_DATA_ROOT` 不在 CI 设置 |
