# 模型同步核对留痕（AC-21 / `repo-archi-model` 漂移纪律）

> **性质**：核对证据文件，**不是**漂移报告。不命名为 `<timestamp>.json`、不被任何消费面（门禁
> runner / `archi_drift` 命令面 / `/architecture` 画布）读取——它只记录「新增 crate `archi-runtime`
> 与模型在同一次变更内同步」这一步的三组机械核对结果。
>
> 本变更只**追加、不收紧**：`gate` 档位全 `off` 不动，`warn` / `strict` 零命中；
> `models/{01-app,02-backend-core,05-backend-tools,06-frontend}.toml` 与
> `.dev-team/architecture/config.json` 零改动；`crates/tools/specta-zod-patched` 为**既有已知
> 缺口**，本变更 MUST NOT 顺手补齐（补它属改本能力的排除面，须另立变更）。

## 一、改动清单（本次唯一模型面 diff）

| 文件 | 变更 |
|------|------|
| `models/03-backend-infra.toml` | 表头 `7 元素 / 21 关系` → `8 元素 / 24 关系`；在 `AgentRuntime` 块后追加元素 `App.Backend.Infra.ArchiRuntime`（`kind = "module"` / `gate = "off"` / `path = ["./packages/desktop/src-tauri/crates/infra/archi/"]`）；在 `AgentRuntime` 的 8 条边后追加 3 条出边 → `Core.Archi` / `Core.Foundation` / `Core.Orchestration`（顺序与 `crates/infra/archi/Cargo.toml` 的 `[dependencies]` 字母序一致） |
| `models/04-backend-shell.toml` | 表头 `1 元素 / 12 关系` → `1 元素 / 13 关系`；在 `Shell → Infra.AgentRuntime` 之后追加 `Shell → Infra.ArchiRuntime`（壳构造与注入 runner）；其余 12 条逐字不变 |
| `decisions/0001-model-granularity.md` | 计数复述五处同步：`§1`（`49 元素 / 107 关系` → `50 / 111`；后端 `43 条（9 + 21 + 12 + 1…）` → `47 条（9 + 24 + 13 + 1…）`）、`§4`（「后端仍 43 条」→ 47）、`§5`（「`App.Backend.Shell` 出边 12 条」→ 13）、`§6`（「基线 49 个元素」→ 50）、`§9`（复算表 `03(7)`→`03(8)`、`03(21)`→`03(24)`、`04(12)`→`04(13)`、元素 `49`→`50（容器 10 + 叶 40）`、关系 `107`→`111`、后端 `43`→`47`）。六条判据与前端拆分式 `64 = 58 + 6` 零改动 |

代码侧证据（四条边逐条可取证，先于模型侧存在）：

```
crates/infra/archi/Cargo.toml   [dependencies] archi / foundation / orchestration（+ serde_json 非 workspace 内）
                                [dev-dependencies] tempfile —— 按段落整体排除，不入模
src-tauri/Cargo.toml            [dependencies] archi-runtime = { workspace = true }
```

## 二、核对组一：四段计数（cwd = 仓库根）

```
grep -c '^\[\[relation\]\]' .dev-team/architecture/models/*.toml
  01-app 0 / 02-backend-core 9 / 03-backend-infra 24 / 04-backend-shell 13 / 05-backend-tools 1 / 06-frontend 64
grep -c '^\[element\.' .dev-team/architecture/models/*.toml
  01-app 3 / 02-backend-core 8 / 03-backend-infra 8 / 04-backend-shell 1 / 05-backend-tools 2 / 06-frontend 28
grep -n "ArchiRuntime" .dev-team/architecture/models/
  03:15（元素）、03:92 / 03:97 / 03:102（三条出边）、04:47（壳入边）  → 03 命中 4 处、04 命中 1 处
grep -rn "warn\|strict" .dev-team/architecture/models/
  零命中（`gate` 全 `off` 不动）
```

| 段 | 元素 | 关系 | 表头注释 | 结论 |
|----|------|------|----------|------|
| `01-app` | 3 | 0 | 一致 | ✓ |
| `02-backend-core` | 8 | 9 | 一致 | ✓ |
| `03-backend-infra` | **8** | **24** | `# 03 · infra 适配层（8 元素 / 24 关系）` | ✓ |
| `04-backend-shell` | 1 | **13** | `# 04 · Tauri 壳（1 元素 / 13 关系）` | ✓ |
| `05-backend-tools` | 2 | 1 | 一致 | ✓ |
| `06-frontend` | 28 | 64 | 一致 | ✓ |
| **合计** | `3+8+8+1+2+28 = 50`（容器 10 + 叶 40） | `0+9+24+13+1+64 = 111`（后端 47 + 前端 64） | — | **三处同值**：模型表头 / `specs/repo-archi-model` 逐文件计数表 / ADR `§9` 复算表 |

## 三、核对组二：图与工具面（`archi_graph` / `archi_module` / `archi_search`）

| 核对项 | 结果 |
|--------|------|
| `archi_graph(root)` | 正常态：`elements` 50 / `relations` 111、`diagnostics` 空、`warnings` 空，不停摆 |
| `archi_module("App.Backend.Infra.ArchiRuntime")` | `dependsOn` 恰 3 条（`Core.Archi` / `Core.Foundation` / `Core.Orchestration`）；`dependedBy` 含 `App.Backend.Shell`（「架构门禁 runner 的构造与注入」）；`modelFile` = `.dev-team/architecture/models/03-backend-infra.toml`、`modelLine` = 15；`gate = "off"`、`kind = "module"`；`children` 空、`warnings` 空 |
| `archi_module("App.Backend.Shell")` | 出边 13 条（新增 `Infra.ArchiRuntime` 一条，其余 12 条逐条在案）；`modelFile` = `.dev-team/architecture/models/04-backend-shell.toml`、`modelLine` = 4；`docs` 含 ADR `0001 · 架构模型的粒度与取数口径` |
| `archi_search("archi")` | 命中 `App.Backend.Infra.ArchiRuntime`（元素在案）与 ADR `0001` 文档条目（`App` 的 `docs` 指向该文件，`source = "text"`）——等价取证为 grep + 模型面 `docs` 声明 |

## 四、核对组三：双向覆盖差集（crate 目录 ⇄ 模型 crate 元素）

crate 目录枚举 `packages/desktop/src-tauri/crates/{core,infra,tools}/*` = **16 个目录**
（core 7 / infra 7 / tools 2）；模型内 crate 元素（`kind = "module"` 且 `path` 指向 crate 目录）
= **15 个**（core 7 / infra 7 / tools 1）。两侧差集只有一个目录，且是**既有已知缺口**：

| crate 目录 | 模型元素 | 结论 |
|------------|----------|------|
| `core/foundation` `core/config` `core/workflow` `core/archi` `core/orchestration` `core/agent` `core/checks` | `App.Backend.Core.*` 7 条 | 相等 |
| `infra/agent` `infra/archi`（**本变更新增**）`infra/checks` `infra/exec` `infra/store` `infra/vcs` `infra/watch` | `App.Backend.Infra.*` 7 条 | 相等 |
| `tools/export-bindings` | `App.Backend.Tools.ExportBindings` | 相等 |
| `tools/specta-zod-patched` | — | **既有已知缺口**（单列，不在本变更补齐；补它须改本能力的排除面判据，另立变更） |

覆盖补充：`App.Backend.Shell` 覆盖 `packages/desktop/src-tauri/src/`（壳不在 crate 目录下，故不参与上表枚举）。

## 五、结论

- 元素 / 关系两侧计数在**三处**（模型表头 / spec 逐文件计数表 / ADR `§9`）同值 = 50 / 111，
  后端 47、`03` 段 8 / 24、`04` 段 13；`Cargo.toml` 四条边与模型四条 `[[relation]]` 互为证据。
- 结构诊断零命中，`gate` 全 `off`、`warn` / `strict` 零命中；`01` / `02` / `05` / `06` 与
  `config.json` 零改动。
- 双向覆盖差集只剩既有的 `crates/tools/specta-zod-patched` 一条，按 R-15 单列、不顺手补齐。
