# 变更 · 前端组件元素覆盖补齐（模型追上判据）
workflow_type: requirement

> 来源：`openspec/explores/ts-export-closure-extraction.md` C.6.7–C.6.9（变更 C 的启动前置对账中发现）。
> 一句话 scope：**两个前端组件文件已有粒度判据、其中一个连存活 spec 都已在案，但架构模型没给它们建元素**——补元素与关系边，让模型追上判据。零源码、零 IPC、零生成物。

## 1 发现路径（为什么现在才暴露）

```text
变更 A（entry 字段 + 落点）       → 入口从探测变声明
变更 A-遗留（数据回填）          → 基线 12 / 27（总 39）落定
变更 C（TS 导出闭包）启动对账    → 判据 1：components/agent/index.tsx 的导出集「恰 7 个名字」
                                  → 按 C 的同元素约束（owner 子树）实算只得 5 个
                                  → 差的两个名字都来自 ./session-panel
                                  → 查模型：components/agent/session-panel.tsx 没有任何元素
```

**静默机制**：`unmatchedFiles` 只在**全量**扫描时列出；现有 archi 报告是 `incremental`（`scannedFiles 0`），所以这条漂移一直没显形。C 的「同元素约束」是第一次把这个缺口变成机械可见的差数。

## 2 判据真值

`.dev-team/architecture/decisions/0001-model-granularity.md` §2「三档粒度」：

> 前端到组件：`components/agent/*.{ts,tsx}` 与 `components/standard/*.tsx` **每文件一元素**（排除测试与 barrel `index.tsx`）；`components/ui/` **收敛为 1 元素**

## 3 实测对拍（前端全量，排除 `ui/` 收敛层、`hooks/`、`lib/`、`types/`、测试）

对拍口径：`models/*.toml` 的 `path` 集合 ↔ `packages/desktop/src/{components,views}` 下全部非测试源文件。

| 未覆盖文件 | ADR 判据命中 | 性质 | 处置 |
|---|---|---|---|
| `components/agent/session-panel.tsx` | §2「agent/*.{ts,tsx} 每文件一元素」 | **漂移（数据漏项）** | **本变更补** |
| `components/standard/tooltip.tsx` | §2「standard/*.tsx 每文件一元素」 | **漂移（数据漏项）** | **本变更补** |
| `components/file/file-viewer.tsx` | **ADR 三层表无此档**（只列 agent / standard / ui） | **判据缺口**（补它 = 改能力） | 本变更**不补**，ADR §8「已知缺口」登记一条 |
| `components/standard/hooks/use-open.ts` | §3 反例明写「hooks 目录不入模型」 | 合法排除 | 零动作 |

`views/**` 全覆盖（零缺口）；`components/ui/` 42 文件由 `App.Frontend.Components.Ui` 收敛承载 ✔。

### 3.1 为什么 `session-panel` 是双面漂移

| 证据面 | 内容 |
|---|---|
| ADR §2 粒度判据 | `components/agent/*.{ts,tsx}` 每文件一元素 ✘ 模型缺 |
| 存活 spec | `openspec/specs/desktop-change-chat/spec.md:156`：「change 详情左栏面板的**骨架**（头区动作 / 状态行 / 历史清单 / 折叠窄条）SHALL 抽为共享组件 `SessionPanel`（`components/agent/session-panel.tsx`，经 `components/agent` 桶导出）」 |
| 公共面 | 被 `components/agent/index.tsx` 桶导出（`SessionPanel` / `SessionPanelState`） |

⇒ 模型是三方里唯一落后的一方。补它是**让数据追上判据与 spec**，MUST NOT 被读成新增架构设计。

`tooltip` 是单面漂移：判据在案、spec 未提（15 行的 `TextTooltip` wrapper，被 `Views.Changes` / `Views.Explores` 直接 import）。

## 4 产物清单

### 4.1 元素（`models/06-frontend.toml`）

```toml
[element."App.Frontend.Components.Agent.SessionPanel"]
kind = "component"
gate = "off"
path = ["./packages/desktop/src/components/agent/session-panel.tsx"]
summary = "会话面板骨架（头区动作 / 状态行 / 历史清单 / 折叠窄条，两处会话面共用）"

[element."App.Frontend.Components.Standard.Tooltip"]
kind = "component"
gate = "off"
path = ["./packages/desktop/src/components/standard/tooltip.tsx"]
summary = "文本气泡（原语层 Tooltip 的业务封装）"
```

### 4.2 关系边（+7，按 ADR §2「目标按最具体已声明元素归属」逐条取证）

```text
① SessionPanel → App.Frontend.Components.Agent.AgentSessionView   import { AgentSessionView } from './agent-session-view'
② SessionPanel → App.Frontend.Components.Ui                        import { Button } from '@/components/ui/button' / ResizablePanel（去重 1 条）
③ SessionPanel → App.Backend.Shell                                 import type { SessionSummary } from '@/types/generated/bindings'（ADR §7 映射例外）★
④ App.Frontend.Components.Agent → …Agent.SessionPanel              桶重导出 SessionPanel / SessionPanelState（barrel 端点边，按目标元素去重）
⑤ Standard.Tooltip → App.Frontend.Components.Ui                    import { Tooltip, … } from '@/components/ui/tooltip'
⑥ Views.Changes → …Standard.Tooltip                                views/changes/change-list-view.tsx 的 TextTooltip
⑦ Views.Explores → …Standard.Tooltip                               views/explores/explore-list-view.tsx 的 TextTooltip

★ 待核：ADR §7 说「`types/generated/bindings` 的 import 不丢弃，而是映射为命令面依赖 → `App.Backend.Shell`」，未区分 type-only；
  既有先例 `…Agent.AgentSessionView → App.Backend.Shell`（desc「经生成的命令面类型取数与调用」）同口径。若实测口径为「值 import 才建边」，
  则 ③ 退场，总边数 120 → 119，其余判据按实测重算。
```

### 4.3 计数（三处 SHALL 同值：模型表头 / `repo-archi-model` 计数表 / ADR §9）

```text
06-frontend      28 / 64   →  30 / 71
    元素 = 页壳 1 + 视图 9 + 子域 3 + 共享组件文件 10 → 12 + 原语层 1 = 26 叶，另有 4 容器
    关系 = 叶边 58 → 64（+6）、barrel 端点边 6 → 7（+1）
    恒等式 71 = 64（叶→叶 / 叶→容器）+ 7（barrel 端点边）   ※ 旧式 64 = 58 + 6
全仓             51 / 113  →  53 / 120
apiFindings      12 / 27（总 39）→ 12 / 29（总 41）
    两个新元素都是文件型根 ⇒ 有入口、无契约 ⇒ 各 +1 `apiMissing`；`apiEntryMissing` 仍 12
```

## 5 牵动面（起草时逐行核，别漏）

| 文件 | 要改什么 |
|---|---|
| `.dev-team/architecture/models/06-frontend.toml` | 2 元素 + 7 边 + 表头两处数（`28 元素 / 64 关系`、`共享组件文件 10`） |
| `.dev-team/architecture/decisions/0001-model-granularity.md` | §1 结论（51/113 的构成）、§2 反例行、§3（barrel 端点边「2 出 + 4 入 = 6」→「3 出 + 4 入 = 7」）、§8（`components/file/` 登记一条）、§9 复算表 |
| `openspec/specs/repo-archi-model/spec.md` | `:42`（三处同值纪律）、`:49`（51/113）、`:205`（恒等式 `64 = 58 + 6`）、`:209`（逐条核对 64 条 / 22 源）、`:301`、`:347`（06 构成表：28 元素 = 4 容器 + 24 叶）、`:351` |
| `openspec/specs/desktop-archi-api-check/spec.md` | `:171`（39 条 → 41）、`:187`（基线表 12 / 27 → 12 / 29）、`:188`（三段读数链末段）、`:189`（基线单段）、`:209`（scenario 逐条点名） |
| `openspec/specs/desktop-archi-model/spec.md` | 若含前端元素构成 / 计数描述则同步（`grep` `28` / `64` / `51` / `113`） |
| `openspec/specs/desktop-change-chat/spec.md` | 预期**零改动**（`:156` 早已把 `SessionPanel` 描述为共享组件——本变更是让模型追上它） |
| `packages/desktop/package.json` | 版本纪律：本变更**零源码**，是否 +1 patch 须在 change 内明写判据（AGENT.md 只对「desktop source change」强制 bump），MUST NOT 默认照抄 |

## 6 验收判据（草案，机械可复算）

1. **AC-1 计数三处同值**：`archi_graph(root)` 返回 **53 元素 / 120 关系**、`diagnostics` 空、`warnings` 空；`06` 表头写 `30 元素 / 71 关系`、`repo-archi-model` 计数表与 ADR §9 复算表逐字同值；前端恒等式 `71 = 64 + 7` 成立。
2. **AC-2 归属可复算**：`OwnerIndex::owner_of("packages/desktop/src/components/agent/session-panel.tsx")` = `Owner::Element { fqn: "App.Frontend.Components.Agent.SessionPanel" }`；`…/standard/tooltip.tsx` = `Owner::Element { fqn: "App.Frontend.Components.Standard.Tooltip" }`（两者 MUST NOT 落 `Ambiguous`，否则与既有文件型 `path` 前缀判据冲突）。
3. **AC-3 判据面换桶**：一次全量检查后 `apiFindings` = **41** = `apiEntryMissing` 12 + `apiMissing` 29，逐条点名含两个新元素（各自 1 条「declared 但契约文件缺失」）；`status = clean`、`passed = true`（纳管子集仍为空）；桶数成因（+2 = 两个新元素有入口无契约）须写进 change 报告。
4. **AC-4 扫描面无新增未覆盖**：全量 `unmatchedFiles` 不再含 `session-panel.tsx` / `tooltip.tsx`；仅余 `components/file/file-viewer.tsx`（ADR §8 已登记）与 `components/standard/hooks/use-open.ts`（ADR §3 合法排除）。
5. **AC-5 零代码扩散**：`git diff` 只含 `models/06-frontend.toml`、`decisions/0001-model-granularity.md`、4 份 spec、change 目录（+ 可选的 `package.json` 版本行）；`src-tauri/**` 与 `packages/desktop/src/**` 零 diff；`pnpm api:check` 三列皆空（无契约资产变化）；`bindings:export` 幂等。
6. **AC-6 已登记缺口不越界**：`components/file/file-viewer.tsx` 仍**不在**模型内；ADR §8 出现该条登记（且写明「补它须改本能力：新增 `components/<其它目录>` 粒度档」）。

## 7 风险与待核

| # | 风险 | 处置 |
|---|---|---|
| R-1 | **边数来自判据推演，非实测**：`bindings` type-only 是否建边（③）、`ui` 原语 import 是否去重为 1 条（②⑤） | AC-1 以一次全量 `archi_graph` 实测为准；若 ③ 退场则 120 → 119、`71 = 64 + 7` 变 `70 = 64 + 6`，同批判据重算，MUST NOT 单改模型去凑预估数 |
| R-2 | **与变更 B / C 的基线链**：B 补 12 个 barrel 后 `apiEntryMissing` 归零、`apiMissing` 重排；本变更落的 12 / 29 是 B 之前的一段 | 在 `desktop-archi-api-check` 的三段读数链后追加一段，写明时间序；MUST NOT 把 12 / 29 写成长期值 |
| R-3 | **ADR 是「能力」不是「数据」**：改 §2/§3/§9 的措辞属文档同步（不改判据）；但若有人想顺手加 `components/file/` 一档即为改能力 | AC-6 钉住：本变更只登记缺口、不新增粒度档；新增档须另开 change |
| R-4 | **版本 bump 判据缺失**：本变更是纯数据 + spec 文档 | change 内明写「零 desktop source 变更 ⇒ 不 bump」或「文档同批发布 ⇒ bump」，二选一写死，MUST NOT 留空 |
| R-5 | **`Tooltip` 的 FQN 命名**：同类兄弟为 `Dailog`（既有拼写）/ `Drawer` / `Pagination` / `Select`，均为 CamelCase 文件名派生 | 取 `…Standard.Tooltip`（与 `tooltip.tsx` 对应）；`summary` MUST NOT 提「与原语层重名」以外的设计说明 |

## 8 与主线的关系与排期

```text
A（已归档） → A-遗留 · 数据回填（已归档）
                    │
                    ▼
        本变更（前端组件元素覆盖补齐）   ← 纯数据 + spec 数字，零源码
                    │
                    ▼
        C（TS 导出闭包取数面）—— 判据 1 的「恰 7 个名字」在现态成立
                    │
                    ▼
        B（12 个 barrel 入口化）—— 判据 1 仍为 7，口径不分叉
```

- **对 C 的净效果**：`owner_of('…/session-panel.tsx')` 命中 `…Agent.SessionPanel`，FQN 前缀落在 `Components.Agent` 子树内 ⇒ C 的同元素约束（owner 子树口径）**无需任何 `Owner::None` 放行分支**即可拿到 7 个名字；
- **对 B 的净效果**：B 的容器翻转（`Components.Agent` 的 `path` 改目录）前后，C 的判据 1 都等于 7 ⇒ B 不必为「翻转前后语料不同」写两段期望值；
- **不做的部分**：`components/file/` 粒度档（改能力）、任何前端源码改动、任何契约资产。

## 9 起草时的第一件事

跑一次**全量** archi 检查（`LocalArchiCheck` 或 `archi_drift` 任一面），把「加边之前」的 `unmatchedFiles` 与 `apiFindings` 逐条留痕到 change 的 `reports/`——它是 AC-1 / AC-3 / AC-4 的对照基线，也是「`session-panel` / `tooltip` 确实未被覆盖」的第一手机械证据（本探索的对拍是静态推导，不是扫描输出）。

同批值得顺手记的两条读数：

- **全量 vs 增量的差**：`incremental` 模式 `scannedFiles = 0` 时 `unmatchedFiles` 恒空，这正是这条漂移两年没显形的原因；change 报告里写明「本次用全量模式」，否则下一个人会以为缺口是新出现的。
- **`apiFindings` 逐条点名**：两个新元素各 1 条 `apiMissing`（有入口 `path` 是文件型单值 ⇒ ② 命中、无契约文件 ⇒ 缺失），文案里的契约文件路径要能在报告里读到（它是「补契约」时的落点提示）。
