# 桌面端构建模式：默认 dev、显式 prod，与数据库落位分裂

workflow_type: requirement

---

## 0. 一句话现状

今天 desktop 的"开发/生产"根本没有被当成一个维度建模：**数据根是一个常量**（`home_dir()/.dev-team`），
`pnpm dev` 与 `pnpm build` 出来的产物落在同一个目录、同一个 app 标识上，只靠两个互不相干的
编译期信号各自为政。

```
pnpm dev  ── tauri dev ───► cargo debug profile   ⇒ debug_assertions = true   ┐
                            vp dev (dev server)   ⇒ import.meta.env.DEV=true ┤ 数据根：~/.dev-team
pnpm build ─ tauri build ─► cargo release profile ⇒ debug_assertions = false  ┤ （两边完全一样）
                            vp build              ⇒ import.meta.env.DEV=false┘  ↑ 没有第三个信号管它
```

两个信号只在"tauri dev vs tauri build"这对组合下恰好同向；一旦引入"dev 模式的 build"，
它们立刻分叉（见 §6 的 updater 陷阱）。而数据根从来没被任何信号管过。

---

## 1. 请求拆成三件事

| # | 诉求 | 本质 |
|---|------|------|
| D1 | 桌面端 build 默认开发模式，`cli 参数`才构建生产模式 | 构建入口的**默认值倒置** |
| D2 | 分离 dev / prod 数据库文件，且 dev 构建与 `pnpm run dev` 共库 | 数据根的**模式化**（一个常量 → 两个常量） |
| D3 | （隐含）dev 构建的运行时行为要跟 prod 区分 | 模式信号**穿透前后端** |

D1+D2 共享同一个判别源，这是本探索最省事的地方；D3 是唯一真正需要新代码的部分。

---

## 2. 仓库里已经有的模式痕迹

| 面 | 位置 | 现状 |
|----|------|------|
| 数据根常量 | `src-tauri/src/main.rs:16` `const DATA_DIR = ".dev-team"` | 单一常量，`setup` 里 `home_dir()?.join(DATA_DIR)` 注入 `WorkspaceStores::open`，零环境解析（store 侧遵守注入约束，`store.rs:2216`） |
| 数据根下的三样东西 | `desktop-global.redb` / `workspaces/` / `logs/` | 都在数据根下 ⇒ **数据根一分裂，三者全部分裂**，不需要额外工作 |
| devtools 插件 | `main.rs:38` `#[cfg(debug_assertions)]` | 已有 profile 判别 |
| 控制台窗口 | `main.rs:1` `cfg_attr(not(debug_assertions), windows_subsystem="windows")` | debug 构建保留控制台（对开发者是优点） |
| updater 检查 | `src/hooks/use-updater.ts:64` `if (!import.meta.env.DEV)` | 用的是 **vite** 信号，不是 rust 信号 |
| 单实例锁 | `tauri-plugin-single-instance`，Windows 下用 `app.config().identifier` 派生具名 mutex | 标识 `dev.wps.dev-team`，**与已安装的生产版互斥** |
| 打包面 | `src-tauri/tauri.conf.json`：`bundle.targets: "all"`、`createUpdaterArtifacts: true` | 本地 build 强制要求 `TAURI_SIGNING_PRIVATE_KEY`（RELEASE.md 明写） |
| 发版 | `.github/workflows/desktop-release.yml` → `tauri-action` → 内部跑 `tauri build` | CI 走的是"裸 tauri build" |

关键观察：**CI 走裸 `tauri build`**。任何以"debug profile = dev 模式"为判别的方案，
CI 都零改动地落在 prod 侧（天然安全）；反过来，任何以"环境变量显式声明模式"的方案，
CI 需要被显式照顾一次。

---

## 3. D1：判别源三选一

| 判别源 | 谁设置 | `pnpm build`（dev 默认）怎么落 | CI 是否需改 | 误把 dev 面发出去的风险 | 代价 |
|--------|--------|-------------------------------|-------------|--------------------------|------|
| **① profile（`debug_assertions`）** | `tauri build --debug` | `--debug` | **否**（CI 裸 build = release = prod） | 低 | 无新开关；但 dev 构建是未优化二进制 |
| ② cargo feature `dev-mode` | 脚本传 `--features dev-mode` | `--features dev-mode` | 否（不传 = prod，fail-safe） | 低 | 新增 feature + 两处 cfg；`tauri dev` 也要带上 |
| ③ build.rs 读 env → `rustc-env` 常量 | 脚本设 env | `cross-env DEV_TEAM_MODE=dev` | 否（缺省 prod） | 低 | build.rs 逻辑 + vite 侧另抄一份 env（易分叉） |

推荐 ①：它让"`tauri dev` 与 dev 构建共库"成为**免费推论**（都是 debug profile），
也让 CI 零改动，且 devtools / 控制台这些已有 debug 面自动跟着走。
② 的唯一价值：可以构建"release 优化 + dev 数据面"的产物（性能真实、仍不碰生产数据）；
如果觉得 debug 二进制太慢（agent 编排 / tokei / redb 都可能感知到），② 才值得上。

### 脚本层长什么样（方案 ①）

```diff
- "dev": "tauri dev",
- "build": "pnpm run bindings:export && pnpm run server:build",
- "server:build": "cross-env TAURI_SIGNING_PRIVATE_KEY_PASSWORD= tauri build",

+ "dev": "tauri dev",
+ "build": "pnpm run bindings:export && pnpm run server:build:dev",
+ "server:build:dev": "tauri build --debug --no-bundle",
+ "build:prod": "pnpm run bindings:export && pnpm run server:build",
+ "server:build": "cross-env TAURI_SIGNING_PRIVATE_KEY_PASSWORD= tauri build",
```

两个附带红利（顺手解决今天两笔摩擦）：

1. `--no-bundle` ⇒ 不打包 ⇒ **不再需要签名私钥**，`pnpm build` 开箱可跑（RELEASE.md 那段"本地构建须知"就只对 `build:prod` 生效）。
2. 产物就是 `src-tauri/target/debug/dev-team.exe`，双击即跑（前提见 §7 的 custom-protocol 验证项）。

---

## 4. D2：数据根分裂的三个落位方案

```
方案 A（推荐，兄弟根）                方案 B（嵌套）                方案 C（prod 搬家）
~/.dev-team/        ← prod          ~/.dev-team/       ← prod    ~/.dev-team/       ← dev（现状不变）
  desktop-global.redb                 desktop-global.redb           desktop-global.redb
  workspaces/                         workspaces/                   workspaces/
~/.dev-team-dev/    ← dev           ~/.dev-team/dev/   ← dev     ~/.dev-team-prod/  ← prod（新）
  desktop-global.redb                 desktop-global.redb
  workspaces/                         workspaces/
```

- **A**：路径常量 `".dev-team"` / `".dev-team-dev"`，`rm -rf` 一次清干净，互不可见。
- **B**：一个屋檐下更像"一个 app 的数据"，但 prod 根里永久躺一个 inert 的 `dev/` 子树，语义含混。
- **C**：会打破**已安装生产用户**的既有数据（workspace 注册表 / provider 配置在 `~/.dev-team`），需要迁移，直接否掉。

存量数据处置（A/B 共同）：今天 `~/.dev-team` 里是**dev 数据**（`tauri dev` 攒的 workspace 与
会话历史），分裂后 dev 模式将看到空库。两条路：
- 接受丢弃（dev 数据本就可弃；重新 add workspace 即可）；
- 文档化一次性手工搬家（`mv ~/.dev-team ~/.dev-team-dev`）——但只有在**生产版没装/没跑**时安全
  （redb 有文件锁，`store.rs:430` 明写"单进程写"，搬家时机错了就是打开失败）。

---

## 5. D3：模式信号穿透前端（唯一必须写代码的地方）

现状 `use-updater.ts` 用 `import.meta.env.DEV` 判断"不检查更新"。这个信号的含义是
**"前端是不是 dev server 服务的"**，不是"这是不是 dev 构建"。所以一个 dev 模式的 build
（`vp build` 产物）会让 `import.meta.env.DEV = false` ⇒ **dev 构建去检查生产 release，
并直接 NSIS 安装生产版覆盖自己**。这是必须一起解决的设计耦合。

两条穿透路线：

```
路线 ①（推荐）IPC 单源           路线 ② vite define 抄一份
Rust: cfg!(debug_assertions)      vite.config.ts 读 process.env.TAURI_ENV_DEBUG
      ↓ #[tauri::command]                 （tauri 为 beforeBuildCommand 注入）
   app_environment()                      ↓ define: __BUILD_DEV__
      ↓ specta 生成 TS 绑定          前端常量
前端 useUpdater / 徽标 / 信息页          前端 useUpdater / 徽标 / 信息页
```

路线 ① 的好处：判据只有一个物理源（rust crate 的 cfg），前端不可能与后端分叉；
`app_environment()` 还能顺带返回 `{ mode, dataRoot }`，把"当前数据根是哪一份"直接显示在 UI 上
（对"两个 app 同时在跑，我看的是哪个库"这类困惑是唯一有效解）。
代价：启动多一次 IPC 往返（updater hook 本来就在 await `getVersion()`，可合并）。

路线 ② 的好处：无 IPC，纯前端；代价：两处独立推导同一事实，`vp test` 环境下还得给默认值。

顺带可挂到同一个 mode 上的 UI 差异（可选，按需取）：
- 顶栏 `Dev Team` 旁一枚 `DEV` 徽标（`app.tsx:44` ShellHeader）；
- `info` 页显示数据根与库文件路径；
- 版本号显示 `v0.5.30-dev`（仅显示层，不动 `package.json`，避免污染 tag/updater 比较）。

---

## 6. 你可能没算到的四笔耦合

1. **单实例锁会让"dev 构建 + 已装生产版"根本跑不起来**：`tauri-plugin-single-instance`
   在 Windows 下以 `app.config().identifier` 派生具名 mutex（crate 源码
   `platform_impl/windows.rs:59-75`），两者标识同为 `dev.wps.dev-team` ⇒ 后启动者被折叠进先启动者。
   数据库分离**并不能**让两个 app 并存，标识也得分离（`tauri build --config '{"identifier":"dev.wps.dev-team.dev"}'`
   或 `src-tauri/tauri.dev.conf.json` 让 CLI 自动合并）。这笔要单独拍板。
2. **updater 反过来也会污染 dev**：同一个 `plugins.updater.endpoints` 指向生产 latest.json，
   dev 构建一旦检查更新，弹的是"更新到 v0.5.30"，点下去装的是**生产版**（且它用的是
   `~/.dev-team`）。要么 dev 构建禁 updater（mode 判别 + cfg 掉插件注册也行），要么
   `--config` 覆盖 endpoints 为空。
3. **redb 文件锁**：同库文件天然单进程写（`store.rs:430`），今天 dev + prod 同时开必炸；
   分裂后 dev/prod 两个库互不影响——这是 D2 的第二个真实收益（第一个是数据不互污）。
4. **`--no-bundle` 的取舍**：跳过打包 ⇒ 无安装包、无 updater 工件、无签名需求；但也就
   失去了"验证安装器/更新链路"的能力。那条路只能在 `build:prod` 里走。

---

## 7. 待验证（都很快，但结论会改动方案细节）

- [ ] `tauri build --debug` 是否启用 `tauri/custom-protocol`（即资源是否内嵌、能否**脱离 dev server 直接跑**）。
      CLI 二进制里能看到 `tauri/custom-protocol` 与 `--no-default-features` 字样，趋势是"build 一律内嵌"，
      但没在 `--debug` 下实证。**这是方案 ① 的地基**：若不内嵌，dev 构建必须 `tauri dev` 才能看到界面，
      "打包一份能双击的 dev 产物"这个诉求就要换实现（改判据为 cargo feature + release profile）。
- [ ] `TAURI_ENV_DEBUG` 在 `tauri build --debug` 的 `beforeBuildCommand` 里确实是 `"true"`
      （schema 只说会注入），决定路线 ② 是否可行。
- [ ] debug 二进制的体验底线：agent 编排 / tokei 统计 / 大库扫描在未优化构建下是否可忍。
      不忍 ⇒ 上"方案 ②：cargo feature `dev-mode` + release profile"。

---

## 8. 需要你拍板

1. **判别源**：profile（简单，CI 零改动，dev 产物未优化）还是 cargo feature（可做"release 优化的 dev 产物"，多一层开关）？
2. **数据根命名**：`~/.dev-team-dev`（A）还是 `~/.dev-team/dev`（B）？存量 dev 数据是丢还是搬？
3. **标识分离**：dev 构建要不要换 bundle identifier（换来"两个 app 可同时跑"，代价是多一份 `--config`/配置文件）？
4. **dev 构建的产物形态**：只要 `target/debug/*.exe`（`--no-bundle`，无签名麻烦）还是要一个可安装包？
5. **updater 在 dev 构建里彻底关掉**（连插件注册都 cfg 掉）还是仅 UI 不提示？

---

## 9. 影响面（实现时会被碰到的清单）

| 文件 | 变动 |
|------|------|
| `packages/desktop/package.json` | 脚本：`build` 默认 dev、新增 `build:prod` |
| `packages/desktop/src-tauri/src/main.rs` | `DATA_DIR` → 按模式取常量；可能新增 `app_environment` 命令注册 |
| `packages/desktop/src-tauri/src/commands/`（新轨道或 info 轨） | `app_environment()`：`{ mode, data_root }`，specta 导出 |
| `packages/desktop/src/hooks/use-updater.ts` | 判据从 `import.meta.env.DEV` 换成 mode |
| `packages/desktop/src/app.tsx` | 可选：`DEV` 徽标 |
| `packages/desktop/src/types/generated/**` | 重新 `bindings:export` |
| `src-tauri/tauri.dev.conf.json`（若选标识分离） | `identifier` / `productName` / updater 覆盖 |
| `packages/desktop/RELEASE.md` | 本地构建须知改写：dev 零配置、prod 才要私钥 |
| `openspec/specs/desktop-workspace-store/spec.md` | 「沿用 `home_dir()/.dev-team/` 根」需补「dev 模式为另一根」 |
| `openspec/specs/desktop-release-pipeline/spec.md` | 补一条「CI 构建恒为生产模式（裸 `tauri build`）」红线 |
| 新 capability（建议） | `desktop-build-modes`：判别源单点、数据根两分、前端 mode 穿透、CI 恒 prod |
