# 抽出共享文件 watch 消费件（file-watch-signal-hook）

workflow_type: requirement

> 本探索服务 **Change A：`extract-file-watch-signal-hook`**（纯重构，零行为变化）。
> 主功能 Change B（change 详情脚本步骤运行中 stdout）的探索见
> `openspec/explores/change-detail-live-stdout.md`；两文档互为交叉引用。
> 落盘说明：本次探索会话的绑定笔记是 B 那份，本文档由 bash 落盘，不与探索记录绑定。

## 1. 问题：同一个「文件信号 → 防抖 → 取数」写了两遍，且口径已经分叉

### 1.1 分歧现状（逐行核实）

| 分歧 | 位置 | 形态 |
|---|---|---|
| 防抖器 A | `views/explores/hooks/use-explore-doc.ts:43-63` `createDebouncer` | 纯闭包，`push` / `cancel`，500 ms |
| 防抖器 B | `views/changes/hooks/use-change-detail.ts:75-90` `useDebouncedBump` | hook + `useRef`，300 ms |
| watch 消费样板 | `views/explores/hooks/use-explore-doc.ts:66-78` `armWatch` | `Channel` 建 → `exploreDocPath` → `watchSubscribe` → 卸载退订 |
| IPC 层不一致 | `views/changes/hooks/use-exec-output.ts:41` 用裸 `commands.readExecOutput` | 绕过 `ipc` 代理的 zod 校验层 |

两份防抖**驱动源不同**（文件信号 vs run 内存通知），所以不是「同一功能的重复实现」，
不能简单合并——见 §3.2 的划界原则。

### 1.2 为什么现在做

Change B 要新增第三个「文件 watch 消费点」（exec 输出记录 live 跟随）。若照抄 explore 那一条链，
仓库里将出现**三份防抖 + 两份 watch 样板**。趁 Change B 之前把消费样板抽单点，
是「先减产再增产」的顺序。

## 2. 抽取目标：`src/hooks/use-file-watch-signal.ts`

### 2.1 落位理由

`src/hooks/` 已住着跨视图件（`use-agent-session` / `use-workspaces` / `use-updater` 等）。
放 `views/changes/` 偏向 exec 消费方，放 `views/explores/` 偏向 explore 消费方——**中性落位**。

### 2.2 形态

```ts
/** 文件 watch 失效信号消费件：Channel 建立 + 订阅 + trailing 防抖 + 装配首拍 +
 *  卸载退订。剥除一切内容语义（信号无载荷）。 */
export function useFileWatchSignal(params: {
  /** 订阅身份键：变化即退订重建；null = 不订阅（空参 / 非运行态） */
  armKey: string | null;
  /** 布局派生路径（命令层给出，前端零目录知识——explore_doc_path 先例 D6） */
  resolvePath: () => Promise<string | null>;
  /** 信号到达（防抖后）与装配首拍均调用 */
  onSignal: () => void;
}): void;
```

收拢的六件事：`Channel` 建立、`watchSubscribe`、trailing 防抖合并、装配首拍、
卸载退订、**订阅失败不阻断数据面**（explore 现状语义，见 §5 用例清单）。

### 2.3 刻意不开的参数

- **`debounceMs`**：此刻只有 explore 一个消费者。加可选参数 = 投机（AGENTS 规则 2「不为单次
  使用造抽象」）。`SIGNAL_DEBOUNCE_MS = 500` 取常量；Change B 接入时也**不**开——两消费方同取
  500（理由见 B 文档 §14.3）。
- **`enabled` / `retry` / 错误回调**：无消费者。

## 3. 刻意不搬迁的两件

### 3.1 `fetchDoc`（取数）

取数是**消费方语义**（explore 读文档 + 失败降级信封），不属于「信号消费」。留在各 hook。

### 3.2 `useDebouncedBump`（run 内存通知防抖）

它由 `notifyRefresh`（`change_flow_watch` 的 kind-only `RunNotice`）驱动：无路径解析、无订阅、
无装配首拍语义，且下游是整页 `get_change_detail` 重查。合并会让公共件承担两种驱动源。

> **划界原则：公共件按「驱动源」切分，不按「都是防抖」切分。**

留给未来「组件合一」时再评估。

## 4. 为什么这次能算「纯重构」（三条硬判据）

1. **测试全在公开 API 层面**：`views/explores/hooks/use-explore-doc.test.ts` 602 行 / 28 个用例，
   只 `import { useExploreDoc }`；`grep createDebouncer|armWatch` → **零命中**。
   ⇒ 抽件后**测试文件零改动**即应全绿。
2. **对外仅两个导出**：`ExploreDocState`（接口）+ `useExploreDoc`（函数）。
   `WATCH_DEBOUNCE_MS` / `createDebouncer` / `armWatch` / `fetchDoc` 全是文件私有
   ⇒ 搬家不外泄新导出，knip 零新增。
3. **零 Rust / 零 bindings / 零 IPC 面**：纯前端文件移动，`bindings:check` 与
   `cargo` 侧断言不受影响。

## 5. 唯一的行为微调（必须显式记账）

**现状**：`useExploreDoc` 的 effect 依赖 `[root, name, tick]`；`refresh()` 增 `tick`
⇒ 触发 effect 重跑 ⇒ **退订 + 重订一对 IPC**。

**抽取后**：`armKey = \`${root}\0${name}\`` 不变 ⇒ `tick` 只驱动重读，**不再重订 watch**。

### 5.1 测试面零影响（逐个用例核实）

| 用例 | 断言 | 影响 |
|---|---|---|
| `有效 root+name 挂载…`（`:122`） | `watch_subscribe` 恰 1 次 | 挂载路径，不受影响 |
| `watch_subscribe reject…refresh 显式拉取仍可用`（`:253`） | 只断 `readCallCount() === 2` | 不涉及订阅次数 |
| 其余 26 例 | doc / error / loading / 时序 | 均不断订阅次数 |

⇒ 唯一「订阅次数」断言在挂载用例（`toHaveLength(1)`），与 `refresh` 无关。

### 5.2 语义面更一致

`openspec/specs/desktop-file-watch/spec.md` 与 archive change 的 **D5** 原文：
「页面生命周期内**无中途重挂**」「消费侧订阅生命周期即页面生命周期」。
现状的重订是 effect 依赖造成的 incidental 行为，**不是**设计意图；抽取顺带把它归正。

### 5.3 代价诚实记账

少了「手动刷新顺带自愈 watcher」这条旁路：若 watcher 在页面生命周期内意外死亡，手动刷新
不再重建它（需重开页面）。**推荐接受**——现状也只在用户点刷新时才自愈，不是可靠机制。
若要保留：把 `tick` 并入 `armKey`（同样零测试影响，代价是每次刷新多一对 IPC）。

## 6. spec 触点（唯一一处）

`openspec/specs/desktop-file-watch/spec.md` 的 **Module Contract** 末行现为：

> 前端 watch hook（`views/explores/hooks/`）| 消费侧 | 防抖窗口持有方；信号 → 显式 `read_explore`
> 拉取；页面卸载退订

须改为：

> 共享消费件 `packages/desktop/src/hooks/use-file-watch-signal.ts` | 信号消费 | Channel 建立 /
> 订阅 / trailing 防抖（`SIGNAL_DEBOUNCE_MS`）/ 装配首拍 / 卸载退订 / 订阅失败不阻断数据面
> —— explore 详情页与（后续）change 详情脚本输出共用

并补一行消费方条目（explore 详情页：`read_explore`）。

**这不是新增能力，是契约归属的落点修正**——但它是真实改动，别漏进 tasks。
`desktop-file-watch` 的其余 Requirement 逐条不变。

## 7. 文件级改动面

```
新增  packages/desktop/src/hooks/use-file-watch-signal.ts        ~70 行
修改  packages/desktop/src/views/explores/hooks/use-explore-doc.ts  删 3 个内部件 + 改 effect
修改  openspec/specs/desktop-file-watch/spec.md                  Module Contract 一行
修改  packages/desktop/package.json                               patch +1（版本纪律）
```

`views/changes/hooks/use-change-detail.ts` **零改动**（§3.2）。
`use-exec-output.ts` **零改动**（它的问题归 Change B）。

## 8. 验证判据

1. `vp test`：`use-explore-doc.test.ts` 的 28 个用例**文件零改动**全绿。
2. `vp check`：lint / 类型零新增告警。
3. `knip`：零新增未使用导出（公共件有两个消费点前置条件不成立——此刻只有 explore 一个
   消费者，故 `useFileWatchSignal` 是**真·生产消费**，不是 test-only export，符合 AGENTS 规则 5）。
4. `bindings:check`：`git diff --exit-code -- src/types/generated` 无变化。
5. 手工：explore 详情页编辑笔记 → 预览 500 ms 内自动刷新；切换 workspace 无残留订阅。

## 9. 风险

| 风险 | 缓解 |
|---|---|
| 公共件形态只经一个消费者验证 | API 按两消费方（含 Change B 的 `armKey: null` 非运行态）设计，但不开空参数；B 若发现形态不合，返工面是一个 70 行文件 |
| 行为微调被后人误认为 bug | 代码注释 + 本文档 §5 显式记账 |
| Change B 延期导致公共件「为谁而抽」失去依据 | A 本身已是净收益：消除两份防抖之一，且顺手统一 `armKey` 语义 |

## 10. 拆分代价与拒绝的备选

### 10.1 拆成两个 change 的代价（诚实记账）

- 两次 `package.json` patch bump、两套 tasks / verification。
- A 期间公共件只有 **1 个消费者**，形态未经第二消费方验证
  （缓解见 §2.3：API 按两消费方设计，但不开空参数）。
- A 提前改一份共享 spec 行的归属（§6）；若 B 最终延期，A 仍是净收益——
  它本身已消除两份防抖之一。
- B 的前端相位必须等 A 归档后才能 import 公共件（B 的后端相位与之无依赖，可并行）。

### 10.2 拒绝的备选：A 作为 B 的 phase 0

把 A 折成 B 的 tasks 首相位，避免两个 change 的开销。

**判否理由**：A 的核心价值是「零行为变化」可被独立证明（§4 的三条判据 + §5 的微调记账）。
把它埋进一个功能 change 里，reviewer 必须在「这是重构还是改行为」之间反复确认，
而这恰恰是 A 唯一需要被审视的地方。**失去独立可验性 = 失去拆分的全部意义。**

### 10.3 A 与 B 的边界（便于评审时一句话判断归属）

| 问题 | 归属 |
|---|---|
| 「两份防抖 / watch 样板重复」 | A |
| 「`use-exec-output` 绕过 `ipc` 校验层」 | A（对齐既有消费方）或 B（该文件本身要重写）——择一即可，勿两处都改 |
| 「运行中看不到 stdout / 命令没落盘 / `outputRef` 只在终态」 | B |
| 「watch 父目录缺失静默休眠」（R4） | B（决策属 B，落在共享 crate 属实现位置） |
