# 0002 · 契约资产目录分层镜像 FQN 祖先链

> 记录 `.dev-team/architecture/models/` 的落位规则变更：由「`<Domain>` 平铺一层」改为「镜像元素祖先链的分层目录」。
> 判据只有一条：**目录必须让真实分层一眼可见，且同名末段的不同元素不许混装**。元素身份仍是 FQN，目录只是布局。

## 1. 结论

契约资产目录段 = 元素 FQN 段链**去掉最外层根段与叶段**后的逐段路径：

```
App.Backend.Infra.ArchiRuntime        → models/Backend/Infra/App.Backend.Infra.ArchiRuntime.md
App.Backend.Core.Foundation           → models/Backend/Core/App.Backend.Core.Foundation.md
App.Frontend.Components.Agent         → models/Frontend/Components/App.Frontend.Components.Agent.md
App.Frontend.Components.Agent.AgentInput
                                      → models/Frontend/Components/Agent/App.Frontend.Components.Agent.AgentInput.md
App.Backend / App（根的直接子元素与根自身）→ models/ 根（零段）
```

文件名恒为完整 FQN（`<FQN>.md` / `<FQN>.schema.json`），目录不承担身份。单一派生点仍是 `api::paths`：`dir_segments(fqn)` → `dir_of(root, fqn)`，`contract_path` / `schema_path` 共用它。

## 2. 为什么换

起点是「`<Domain>` = 父元素 FQN 末段」的平铺布局。它在四元素 / 五元素的试点期够用，到 53 元素时暴露两处硬伤：

| 硬伤 | 平铺布局下的表现 | 分层布局后 |
|---|---|---|
| 真实分层不可见 | `Core/`、`Infra/`、`Components/`、`Agent/` 四个同级目录并排：`Agent/` 装的是前端组件子元素，`Core/` 装的是后端模块，`Backend` / `Frontend` 这条主干在目录里完全不存在 | 主干即目录：`Backend/Core/`、`Backend/Infra/`、`Frontend/Components/Agent/` |
| 同名末段混装 | `App.Backend.Core.Agent` 与 `App.Frontend.Components.Agent` 是不同父段的不同元素，却按「父段末段」落到 `Core/` 与 `Components/` 之外的第三个位置；一旦两层都用 `Agent` 作父段（`Components.Agent.X` 与 `Views.Agent.X` 已是本仓真实形态），两组元素就落进同一目录 | 祖先链不同 ⇒ 目录必然不同，同名末段天然分开；`$ref` 前缀由两侧段链逐段求出，不依赖「同目录 = 同域」的近似 |

试点期把这两点记为已知风险（`module-api-contract-format` 的 R-8）；分层的代价是路径变长，收益是「目录结构 = 架构结构」这条对 agent 与人的共同直觉成立。

## 3. 连带规则

- **根段不建目录**：FQN 第一段（本仓恒为 `App`）只表达「同一个应用」，进每条路径只是多一层噪声；单段 FQN（`App`）与根的直接子元素（`App.Backend`）因此都落 `models/` 根。
- **跨文件 `$ref` 前缀逐段求**：`ref_relative(from, to)` 取两侧段链的公共前缀，向上每层一个 `../`、向下每段 `./<段>/`，同目录 `./`。既有 `../Infra/`（同层跨子树）与新的 `../../../Backend/Core/`（跨层）由同一函数给出，MUST NOT 手算层数。
- **`INDEX.toml` 说明段零改动**：索引只写 repo 相对路径（`schema` 列随落位变化），说明段的「契约面：目录」串取的是 `<archi_models_rel_dir>` 本身。
- **孤儿扫描深度上限提到 8**：`MAX_API_WALK_DEPTH` 原为 4（`<Domain>/<Sub>/` 级的容量假设），分层后目录深度随 FQN 深度增长（本仓当前最深 3 层），上限留足余量；该值只防环，不是布局判据。
- **启用判据不受影响**：`api::enabled` 判的是「载体在盘且含 api fence」，载体派生链仍是 `carrier_path` 单点，递归扫面本来就与目录深度无关。

## 4. 迁移与门禁

`git mv` 搬 10 份契约 md + 10 份 `.schema.json`（内容逐字不变，只换目录），随后 `pnpm api:export` 重生成 `INDEX.toml`——本变更对索引的贡献恰为已声明元素的 `schema` 列 8 行（repo 相对路径含分层段），说明段的「契约面：目录」串取目录根、逐字不变；`pnpm api:check` 零 diff 即闭环。判据语料（`tests/corpus/drift/api-mismatch/contracts/`）与金样快照随落位同批更新。

**修改纪律**：目录规则属 `api::paths` 单点，改它必须连带改本条记录与 `desktop-archi-api` 的落位要求；MUST NOT 在生成器、命令层或前端另拼一份目录段（零行内段名字面量由 `directory零行内段名字面量` 用例机械守）。
