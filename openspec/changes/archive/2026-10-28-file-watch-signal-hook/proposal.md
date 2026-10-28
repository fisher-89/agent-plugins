# 提案: file-watch-signal-hook

> **变更**: file-watch-signal-hook
> **日期**: 2026-10-27
> **状态**: draft
> **来源**: `explore.md`（requirement 类型：逐行取证的现状分歧 / 落位与形态 / 三条纯重构判据 / 一处行为微调记账 / 唯一 spec 触点 / 拆分代价与拒绝的备选）
> **前置依赖**: 无。`desktop-file-watch` 的通道侧（`crates/infra/watch` + `watch_subscribe` / `watch_unsubscribe` 薄命令）与前端 `ipc` 代理层均在位；本变更只动前端消费侧，零 Rust / 零 bindings / 零命令面。
> **性质**: 纯重构——「零行为变化」为默认口径，**两处微调显式记账**（见「提案」§5 与「风险」R2/R3），可被独立验证。

---

## 问题

### 现状：同一个「文件信号 → 防抖 → 取数」写了两遍，且口径已经分叉

| # | 分歧 | 位置 | 形态 |
|---|---|---|---|
| P1 | 防抖器 A（文件信号驱动） | `views/explores/hooks/use-explore-doc.ts:43-63` `createDebouncer` | 纯闭包 `push` / `cancel`，`WATCH_DEBOUNCE_MS = 500` |
| P2 | 防抖器 B（run 内存通知驱动） | `views/changes/hooks/use-change-detail.ts:75-90` `useDebouncedBump` | hook + `useRef`，`DETAIL_REFRESH_DEBOUNCE_MS = 300` |
| P3 | watch 消费样板 | `views/explores/hooks/use-explore-doc.ts:66-78` `armWatch` + effect 内 `new Channel(...)` / `onmessage` / 退订 | `Channel` 建 → `explore_doc_path` → `watch_subscribe` → 卸载退订，四步散落在同一 effect |
| P4 | 未来第三份 | change 详情「脚本步骤运行中 stdout」（另一 change）需要 live 跟随 exec 输出记录 | 若照抄 P1+P3 的链，仓库将出现**三份防抖 + 两份 watch 样板** |

P1 与 P2 **驱动源不同**（磁盘文件失效信号 vs `change_flow_watch` 的 kind-only `RunNotice`），因此不是「同一功能的重复实现」，本变更**只抽取 P1+P3**，不动 P2——划界原则见「提案」§3。

### 为什么现在做

「先减产再增产」：把唯一一条「文件信号 → 防抖 → 取数」样板收敛到单点，再去接新的消费点；否则新消费点只会复制既有样板，重复面从 2 份变 3 份。

### 纯重构的三条判据（本变更成立的前提，已逐条核实）

1. **测试全在公开 API 层面**：`views/explores/hooks/use-explore-doc.test.ts` 602 行 / 23 个用例只 `import { useExploreDoc }`；`grep createDebouncer|armWatch|WATCH_DEBOUNCE_MS` 在 `src` 内仅命中实现文件本身，测试侧**零命中** ⇒ 抽件后既有用例**文件零改动**即应全绿。
2. **对外仅两个导出**：`ExploreDocState`（接口）+ `useExploreDoc`（函数）；`WATCH_DEBOUNCE_MS` / `createDebouncer` / `armWatch` / `fetchDoc` 全为文件私有 ⇒ 搬家不外泄新导出，`knip` 零新增。
3. **零 Rust / 零 bindings / 零 IPC 面**：纯前端文件移动，`bindings:check`（`git diff --exit-code -- src/types/generated`）与 `cargo` 侧断言不受影响。

---

## 提案

### 1. 新增共享消费件 `packages/desktop/src/hooks/use-file-watch-signal.ts`

```ts
/**
 * 文件 watch 失效信号消费件：Channel 建立 + 布局派生路径订阅 + trailing 防抖
 * + 卸载退订 + 订阅失败不阻断数据面。剥除一切内容语义（信号无载荷）与取数语义
 * （取数留消费方）。
 */
export function useFileWatchSignal(params: {
  /** 订阅身份键：变化即退订重建；null = 不订阅（空参 / 非运行态） */
  armKey: string | null;
  /** 布局派生路径（命令层给出，前端零目录知识——`explore_doc_path` 先例） */
  resolvePath: () => Promise<string | null>;
  /** 信号到达（500ms trailing 防抖后）调用一次 */
  onSignal: () => void;
}): void;
```

**落位理由**：`src/hooks/` 已是跨视图共享件所在层（`use-agent-session` / `use-workspaces` / `use-app-environment` / `use-updater`）。放 `views/changes/` 会偏向 exec 消费方，放 `views/explores/` 会偏向 explore 消费方——**中性落位**。该件不是「取数 hook」，与 `desktop-app-shell`「取数收口 / 页面域取数 hooks 就近页面」的落位规则不冲突（取数仍留消费方）。

**职责边界（五件事，逐条来自 P1/P3 的现状语义）**：

| # | 职责 | 来源 |
|---|---|---|
| C1 | `Channel<FileWatchEvent>` 建立并挂派发 | 现状 effect 内 `new Channel` + `onmessage` |
| C2 | `resolvePath()` → null 则不订阅；否则 `watch_subscribe(channel, path)` | 现状 `armWatch` |
| C3 | trailing 防抖 `SIGNAL_DEBOUNCE_MS = 500`（文件私有常量，**MUST NOT** 开参数）合并为一次 `onSignal` | 现状 `createDebouncer(WATCH_DEBOUNCE_MS, readDoc)` |
| C4 | 卸载 / 臂键变化：取消待发计时 + `watch_unsubscribe(subscriptionId)`；订阅迟到就绪时**立即补发退订** | 现状 cleanup + `if (cancelled) off()` 守卫 |
| C5 | 订阅链失败**静默**（不冒泡为消费方错误态、不阻断数据面，页面仍可显式刷新） | 现状 `.catch(() => {})` |

**刻意不搬的两件**：

- **取数**（`fetchDoc`）：explore 读文档 + 失败降级信封是**消费方语义**，留各 hook。
- **装配首拍**（explore 草案里把「挂载即拉一次」列为公共件职责）：本提案**否决搬入**，理由见 §4。

### 2. 消费方改写：`use-explore-doc.ts`

- 删 `WATCH_DEBOUNCE_MS` / `createDebouncer` / `armWatch`（三件私有件退场，文件约 130 → 约 80 行）。
- 取数 effect 的依赖面**保持 `[root, name, tick]` 不变**（`loading` / `error` / `cancelled` 竞态抑制语义逐字不动），只把 effect 内的 watch 装配整段移出，并把该实例的取数闭包挂到 `readRef`（`useRef`）。
- 新增一次 `useFileWatchSignal({ armKey, resolvePath, onSignal })`：`armKey` = `root` 与 `name` 以 NUL 拼接的订阅身份串（`name` 为 kebab stem，不可能含 NUL，无碰撞），空参时给 `null` 即不订阅；`resolvePath` = `() => ipc.exploreDocPath(root, name)`；`onSignal` 经 `readRef.current()` 走**当前**取数闭包——**信号重拉不置 `loading`、不清 `error`**（与今日一致；否则 `ExplorePreview` 的 `preview-loading` 会在每次外部保存时闪一次）。
- 对外导出面（`ExploreDocState` / `useExploreDoc`）与语义**零变化**，`explore-detail-view.tsx` 与全部呈现层零改动。

### 3. 划界原则（写给后来者）

> **公共件按「驱动源」切分，不按「都是防抖」切分。**

- 文件失效信号（磁盘，无路径解析前不成立 → 需要 `resolvePath` + 订阅 + 退订）→ 本件。
- run 内存通知（`change_flow_watch` 的 kind-only `RunNotice`，无路径解析、无订阅、下游是整页重查）→ 留在 `use-change-detail` 的 `useDebouncedBump`，**本变更零改动**，组件级合流留待未来再评估。

### 4. 否决「装配首拍进公共件」（对 explore 草案的一处修订，须记账）

explore 草案把「装配首拍」列为公共件职责（六件事）。本提案**否决**，因为：

1. 今天「挂载即拉一次」是**取数时机**，属消费方数据面；把它搬进公共件会让挂载路径出现**两次** `read_explore`（取数 effect 一次 + 公共件首拍一次），直接撞既有用例「挂载：`read_explore` 拉取一次」（`use-explore-doc.test.ts:122`）与「同步进入 loading 态」（`:274`）。
2. 要避免双击，只能把消费方取数 effect 拆成「挂载不拉、靠首拍拉」，改动面从「移动 70 行」升级为「重写 hook 的状态机」——**失去纯重构的独立可验性**（判据 §1 / §2 均依赖「消费方取数路径不变」）。
3. 第二个消费方（exec 输出跟随）同样自带挂载取数 effect，首拍对它同样冗余。

⇒ 公共件交付 **C1–C5（信号 + 订阅生命周期）**，取数时机归消费方。spec 侧同步按此口径写（见 capabilities）。

### 5. 两处行为微调（显式记账，禁止被后人误认作 bug）

| # | 今日 | 抽取后 | 处置 |
|---|---|---|---|
| M1 | effect 依赖含 `tick`，`refresh()` 增 tick ⇒ **退订 + 重订一对 IPC** | `armKey` 不含 `tick` ⇒ 显式刷新只重取数、**不重订 watch** | **接受**。`desktop-file-watch` 既有语义「订阅生命周期即页面生命周期」「页面生命周期内无中途重挂」本就排除了刷新重订；今日的重订是 effect 依赖造成的 incidental 行为。代价：「手动刷新顺带自愈死亡 watcher」这条旁路消失（现状也只在用户点刷新时才自愈，不是可靠机制）。备选（把 `tick` 并入 `armKey`）否决——每次刷新多一对 IPC，换一条不可靠的自愈 |
| M2 | 刷新（tick bump）会顺带取消挂起的信号防抖 | 防抖计时器归公共件、按臂键存续 ⇒ 显式刷新与**待发的信号**可能各拉一次 `read_explore` | **接受**。极小概率交错（保存后 500ms 内点刷新），两次取数结果幂等、不置 `loading`、无 UI 副作用。为它给公共件开 `cancelPending()` 出口 = 为单次场景造抽象（规则 2），否决 |

两处微调 MUST 在 `use-explore-doc.ts` 与 `use-file-watch-signal.ts` 的注释里留痕，并由新增用例锁定（见「变更范围」测试文件）。

### 6. 契约归属落点修正

`openspec/specs/desktop-file-watch/spec.md` 的 Module Contract 现在把「防抖窗口持有方」挂在 `views/explores/hooks/`——落点随抽取改为 `packages/desktop/src/hooks/use-file-watch-signal.ts`，并补一行 explore 详情页消费方。**这不是新增能力，是契约归属的落点修正**；该 spec 的其余 requirement 逐条不变。

> 落盘口径（沿仓库约定）：spec 改动以 **delta** 形式落在 `openspec/changes/file-watch-signal-hook/specs/desktop-file-watch/spec.md`，归档时合入 `openspec/specs/`。本变更**不直接编辑** `openspec/specs/`。

### 7. 版本纪律

`packages/desktop/package.json` `version` 0.6.2 → **0.6.3**（一次 patch）。`src-tauri/tauri.conf.json` 经 `"version": "../package.json"` 自动跟随；`src-tauri/Cargo.toml` 的 crate 版本行**不跟**（本变更零 Rust 改动）。

### 已否决（存档）

| 方案 | 否决理由 |
|---|---|
| 把本变更折成后续功能 change 的 phase 0 | 本变更的核心价值是「零行为变化」可被独立证明（三条判据 + 两处微调记账）；埋进功能 change 后 reviewer 必须反复确认「这是重构还是改行为」，而失去独立可验性 = 失去拆分的全部意义 |
| 合并 `useDebouncedBump`（run 通知防抖）进公共件 | 驱动源不同（见 §3）；合并会让公共件承担两种驱动源，且下游语义（整页重查 vs 显式拉取）也不同 |
| 给公共件开 `debounceMs` 参数 | 此刻只有一个消费者，加可选参数 = 投机；第二个消费方（exec 输出）同样取 500，无需参数 |
| 给公共件开 `enabled` / `retry` / 错误回调 | 无消费者；`armKey: null` 已覆盖「不订阅」，订阅失败已定「静默不阻断」 |
| 顺手修 `use-exec-output.ts:41` 绕过 `ipc` 代理层 | 该文件的 live 跟随与取数形态由后续 change 重写；同一问题只在一处改（本变更零改动，留给后续 change） |
| 顺手把共享件升级成通用「订阅 + 防抖」框架 | 单点 70 行即够；框架化会引入未使用的抽象面（规则 2） |

---

## 能力

### 新增的能力

**无。** 共享消费件是 `desktop-file-watch` 通道契约的**消费侧**，不是新的能力边界：它不新增命令、不新增推送语义、不新增数据面，只是把该能力已声明的消费侧纪律（防抖窗口由前端持有、信号 → 显式取数、卸载退订）落到单一实现。另立能力会让「通道契约」与「通道消费」共用一条 requirement，反而降低可读性。

### 修改的能力

- **desktop-file-watch** — `MODIFIED`「失效信号语义（非数据通道）」（在既有正文后追加消费侧单点持有条款：防抖窗口 + 订阅样板 SHALL 由共享消费件单点持有，消费方 MUST NOT 各自实现；run 通知防抖显式排除在划界之外）；`ADDED`「共享文件 watch 信号消费件」（落位 / 参数面三项 / C1–C5 五件事 / 零取数零状态 / 不开参数）、`ADDED`「订阅臂键与重订边界」（`armKey` = 订阅身份无碰撞编码；刷新维度 MUST NOT 进臂键 ⇒ 显式刷新不重订；`armKey: null` = 不订阅；run 通知防抖零改动）；Module Contract 归属行修正 + 新增消费方行。其余 requirement（单文件订阅通道、生命周期与平台边界）逐条不变。

### 零改动的能力及理由

| 能力 | 理由 |
|---|---|
| **desktop-explore-page** | 详情页行为契约（挂载订阅、信号后防抖再 `read_explore`、卸载退订、`useExploreDoc` 以 `name` 为依赖自动重订阅新路径）逐条不变——`armKey` 含 `name`，名变仍重订。Module Contract 中 `use-explore-session` 行把「watch 信号防抖 → `read_explore` 显式刷新」挂在会话 hook 上属**存量归属漂移**（该语义实际由 `use-explore-doc` 承接），与本次行为无关，不在本变更修（记入 R5） |
| **desktop-explore-queries** | 零命令面 / 零查询语义改动（`read_explore` 调用点、入参、纯读纪律不动） |
| **desktop-app-shell** | 前端 hooks 落位规则（`src/hooks/` = 跨视图共享件；页面域取数 hooks 就近页面）与本件不冲突：本件不取数。零命令注册、零路由改动 |
| **desktop-change-flow-view / desktop-change-orchestration** | `use-exec-output` 与 `use-change-detail` 零改动（划界原则）；stdout live 跟随属后续 change |
| **repo-archi-model** | `src/hooks/**` 本就在建模封闭排除面内，新增文件不产生元素 / 边 / 覆盖变化 |
| **desktop-ipc-runtime-guard / desktop-ipc-type-bindings** | 新文件只经 `ipc.watchSubscribe` / `ipc.watchUnsubscribe` 调用（沿既有一等调用面），零新增裸 `commands.*`、零 bindings 重生成 |
| **desktop-test-execution / desktop-corpus-regression / desktop-release-pipeline** | 门禁命令与发版纪律不变（`vp check` / `vp test` / `knip` / `bindings:check` 形态不动，仅新增用例） |
| **全部 Rust 能力** | 零 Rust 改动 |

---

## 变更范围

### 实现文件

| 文件 | 改动 |
|---|---|
| `packages/desktop/src/hooks/use-file-watch-signal.ts`（**新增**，约 70 行） | `SIGNAL_DEBOUNCE_MS = 500`（文件私有）；`useFileWatchSignal({ armKey, resolvePath, onSignal })`：effect 依赖仅 `[armKey]`，`resolvePath` / `onSignal` 经 ref 取最新（沿 `useDebouncedBump` 的 ref 先例）；C1–C5 五件事；零状态、零取数、零 `ipc` 之外的依赖 |
| `packages/desktop/src/views/explores/hooks/use-explore-doc.ts` | 删 `WATCH_DEBOUNCE_MS` / `createDebouncer` / `armWatch`；取数 effect 依赖面 `[root, name, tick]` 与 `loading` / `error` / 取消抑制语义逐字不动，取数闭包挂 `readRef`；挂 `useFileWatchSignal`（`armKey` = 空参时 `null`，否则 `root` 与 `name` 以 NUL 拼接的订阅身份串；`resolvePath` = `() => ipc.exploreDocPath(root, name)`；`onSignal` = `() => readRef.current()`）；两处微调注释留痕；导出面不变 |
| `packages/desktop/package.json` | `version` 0.6.2 → 0.6.3 |

### 测试文件

| 用例 / 装置 | 位置 | 处置 |
|---|---|---|
| 既有 23 例（含挂载恰一次、500ms 合并、卸载退订、迟到订阅补发退订、订阅失败不阻断、参数变更竞态、绑定调用面、`ipc` 校验面） | `src/views/explores/hooks/use-explore-doc.test.ts` | **零改动全绿**——纯重构的判据（判据 §1） |
| 新增：显式 `refresh()` 后 `watch_subscribe` 仍恰 1 次、`watch_unsubscribe` 0 次（锁 M1 臂键语义） | 同上（追加用例，不动既有断言） | 新增 |
| 新增：信号触发的重拉**不置** `loading`、不清 `error`（锁「信号重拉 ≠ 显式刷新」语义） | 同上 | 新增 |
| 可选：先 `refresh()` 再推进 500ms ⇒ `read_explore` 共 2 次（锁 M2 已记账行为，防变异存活） | 同上 | 新增（可选） |
| 公共件自身单测文件 | — | **不新增**：公共件经 `useExploreDoc` 公开面被既有 + 新增用例覆盖（`armKey: null` / `resolvePath` null / 订阅 reject / 迟到退订 / 防抖重置 / 清理六臂全命中）；按仓库规则 5 不为测试新增导出 |

### 不要修改

- **`packages/desktop/src/views/changes/hooks/use-change-detail.ts`**（`useDebouncedBump` / 重试调度）：驱动源不同，划界原则 §3 已定。
- **`packages/desktop/src/views/changes/hooks/use-exec-output.ts`**：其 `readExecOutput` 取数与「绕过 `ipc` 代理」问题归后续 change；同一问题只在一处改。
- **`packages/desktop/src/views/explores/` 其余文件**（`explore-detail-view.tsx` / `components/**` / `use-explore-list.ts` / `use-explore-session.ts`）：导出面不变 ⇒ 零改动。
- **`packages/desktop/src/lib/ipc.ts` / `ipc-drift.ts` / `types/generated/**`**：零新增命令、零 schema 改动；`bindings:check` 必须无 diff。
- **`packages/desktop/knip.json`**：零新增 `ignore` / `ignoreDependencies`（新件是真·生产消费，`useExploreDoc` import 它）。
- **Rust 侧全体**（`crates/infra/watch/`、`src-tauri/src/commands/watch/`、DTO / 命令注册 / `Cargo.toml` crate 版本行）。
- **`openspec/specs/**` 基线**：只在 change 目录出 delta（归档时合入）；`openspec/config.json` 零改动。
- **`openspec/changes/file-watch-signal-hook/explore.md`**：只读。

---

## 验收标准

| ID | 验收条件 | 验证方法 |
|---|---|---|
| AC-1 | 既有 23 个用例**文件零改动**全绿（纯重构判据） | `pnpm -C packages/desktop run client:test`；`git diff -- src/views/explores/hooks/use-explore-doc.test.ts` 内无既有断言行被删改（只有新增行） |
| AC-2 | 文件 watch 消费样板单点化（**生产代码范围**）：在 `packages/desktop/src` 的**非测试生产文件**（排除 `*.test.ts` / `*.test.tsx`）内，`new Channel<FileWatchEvent>` 与 `ipc.watchSubscribe` / `ipc.watchUnsubscribe` 的命中行**文件路径唯一**，且为 `src/hooks/use-file-watch-signal.ts`（该件内三处调用同属一个消费点，**不按行数计**）；`use-explore-doc.ts` 零 `FileWatchEvent` Channel / 零 watch 命令 / 零 `createDebouncer` / 零 `armWatch`。测试文件（`src/lib/ipc.test.ts:1018` 的订阅探针、`src/lib/ipc-contract.test.ts:198` 的 `Parameters<typeof ipc.watchSubscribe>` 类型断言）按 AC-1 保持零改动，**不在本断言管辖内**；`agent-transport.ts` / `use-change-flow-run.ts` 的 `Channel<AgentRunMessage>` / `Channel<RunNotice>` 属其它推送语义，亦不受管辖 | 三条 grep（前两条均排除测试文件）：`grep -rn --exclude='*.test.ts' --exclude='*.test.tsx' "new Channel<FileWatchEvent>" packages/desktop/src`（恰 1 行，位于新件）+ `grep -rn --exclude='*.test.ts' --exclude='*.test.tsx' "ipc\.watchSubscribe\|ipc\.watchUnsubscribe" packages/desktop/src \| cut -d: -f1 \| sort -u`（恰 1 行路径，`packages/desktop/src/hooks/use-file-watch-signal.ts`）+ `grep -n "createDebouncer\|armWatch\|WATCH_DEBOUNCE_MS" packages/desktop/src/views/explores/hooks/use-explore-doc.ts`（零命中） |
| AC-3 | 臂键语义（M1）：显式 `refresh()` 后 `watch_subscribe` 仍恰 1 次、`watch_unsubscribe` 0 次；`root` / `name` 变更仍退订 + 重订 | 新增用例（AC-3 两臂）+ 既有参数变更用例 |
| AC-4 | 信号重拉不置 `loading`、不清 `error`（`preview-loading` 不因外部保存闪动）；500ms trailing 合并语义与既有 3 例一致 | 新增用例 + 既有「信号 500ms trailing 防抖」用例 |
| AC-5 | 订阅失败不阻断数据面、订阅迟到就绪补发退订、`resolvePath` 为 null 零退订：三者语义逐字不变 | 既有用例（`:231` / `:253` / `:294` / `:581`）零改动通过 |
| AC-6 | 导出面不变：`use-explore-doc.ts` 仍只 `export` `ExploreDocState` / `useExploreDoc`；公共件只 `export` `useFileWatchSignal`；`knip` 零新增未使用导出 | `grep -n "^export"` 审查 + `client:check`（含 knip） |
| AC-7 | 门禁全绿：`vp check`（fmt / lint / typeCheck，含 `max-lines-per-function: 50`）、`vp test`（覆盖率 ≥ lines 90 / branches 85 / functions 90）、`bindings:check` 无 diff | `pnpm -C packages/desktop run check` |
| AC-8 | 零后端面：`git diff --stat -- packages/desktop/src-tauri` 为空；`src-tauri/Cargo.toml` crate 版本行未跟 `package.json` | diff 审查 |
| AC-9 | 边界纪律：`use-change-detail.ts` / `use-exec-output.ts` 零 diff；`knip.json` 零 diff | `git diff --stat` 审查 |
| AC-10 | 版本纪律：`package.json` `version` 0.6.2 → 0.6.3（一次 patch） | `package.json` 审查 |
| AC-11 | 契约归属：change 目录 delta 中 `desktop-file-watch` 的 Module Contract 指向 `src/hooks/use-file-watch-signal.ts` 并含 explore 详情页消费方行；既有 requirement 未被删改（`单文件 watch 订阅通道` / `生命周期与平台边界` 原样保留） | delta 审查 + 归档合入演练 |
| AC-12 | 手工冒烟：explore 详情页编辑笔记并保存 → 500ms 内预览自动刷新且**无 loading 闪动**；点刷新后预览更新且 watch 不重建；切换 workspace 无残留订阅（后端零订阅压力） | 手工（Windows 桌面端） |

**AC-2 口径说明（回溯修正，勿再收紧）**：该断言的范围是**生产代码**，不是整个 `src`。全 `src` 计数口径与仓库事实冲突——`src/lib/ipc.test.ts:1018`（`ipc.watchSubscribe(...)` 订阅探针）与 `src/lib/ipc-contract.test.ts:198`（`Parameters<typeof ipc.watchSubscribe>` 类型断言）已各有 1 命中，且按 AC-1 这两个测试文件必须零改动；正确实现下新件自身另含 `watchSubscribe` + `watchUnsubscribe` 各 1 行。因此「全 `src` 恰 1 命中」在正确实现下必然为红，MUST NOT 被当作「实现不达标」去改测试或改实现。判据取「排除 `*.test.ts(x)` 后命中行的**文件路径唯一**」，行数不作断言（单点可含多行调用）。

---

## 风险

| ID | 风险 | 影响 | 概率 | 缓解措施 |
|---|---|---|---|---|
| R-1 | 公共件在 A 期只有**一个**消费者，形态未经第二消费方验证 | 后续 change 接入时若形态不合，返工面是本件 | 中 | 参数面按两消费方形态设计（`armKey: null` 即非运行态 / 非定址态），但**不开** `enabled` / `debounceMs` 空参数；返工面固定在约 70 行单文件 + 其消费点改动 |
| R-2 | 微调 M1（刷新不再重订 watch）被后人误认作 bug 而「修回去」 | 语义回退、hidden IPC 往返回归 | 中 | 公共件与消费方注释留痕 + 本 proposal §5 记账 + 新增用例 AC-3 锁定「刷新不重订」 |
| R-3 | 微调 M2（刷新与待发信号各拉一次）被当作重复拉取缺陷 | 误改防抖归属，重新耦合回取数 effect | 低 | §5 表格记账；不给公共件开 `cancelPending()`（否决理由在案） |
| R-4 | 失去「手动刷新顺带自愈死亡 watcher」旁路 | 极端情况下 watcher 意外死亡需重开页面 | 低 | 现状也只在用户点刷新时自愈，非可靠机制；订阅失败已是「静默不阻断」（C5）；若未来确需自愈，另立 change 按信号健康度设计，MUST NOT 靠刷新重订兜 |
| R-5 | `desktop-explore-page` Module Contract 的 `use-explore-session` 行把「watch 信号防抖 → `read_explore`」挂在会话 hook 上（存量归属漂移） | 契约文本与实现归属不一致，读 spec 者误判责任文件 | 中 | 本变更**不修**（改它会与「只做抽取」的范围纪律冲突，且该行描述的页面级结果仍成立）；记入本表，归属修正留给触及该 hook 的后续 change |
| R-6 | 后续 change（exec 输出 live 跟随）的探索档 `openspec/explores/change-detail-live-stdout.md` **不在盘上**，其两消费方假设不可复核 | 本件的第二消费方形态无法在 A 期校验 | 中 | 立项依据不以 B 为前提：A 自身即净收益（消除重复样板之一、统一臂键语义）；B 侧形态不合时返工面同上 R-1 |
| R-7 | `max-lines-per-function: 50`（非测试 `.ts`）对公共件函数体的约束 | lint 报错即需拆内部 helper | 中 | 公共件按 C1–C5 拆成文件私有 helper（`arm` / 退订外壳），保持单函数 ≤ 50 行；拆分保持在文件内，不外泄导出（AC-6） |
| R-8 | 覆盖率 / 突变分数按「新文件被既有套件覆盖」计算，若某分支未被命中会拉低门禁 | 需补用例或调整实现分支形态 | 低 | 六臂（`armKey: null` / `resolvePath` null / 订阅 reject / 迟到退订 / 防抖重置 / 清理）均已被既有或新增用例命中；门禁不过时优先**补用例**而非加豁免 |

