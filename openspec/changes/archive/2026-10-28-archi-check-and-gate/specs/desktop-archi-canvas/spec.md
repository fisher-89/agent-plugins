# desktop-archi-canvas Specification（delta: archi-check-and-gate）

## MODIFIED Requirements

### Requirement: 漂移标记呈现契约（只认标记，不认判定）

画布 SHALL 接受一组**标记**并按标记渲染，标记的**产生**（何时算漂移、如何判定）MUST NOT 在本能力定义。标记词汇 SHALL 为封闭集：

```
边标记   none         实线（无标记）
         unfulfilled  灰虚线（声明未兑现）
         crossKind    橙虚线（跨 kind 向上，提示价值而非约束）
         exempt       灰实线（豁免；抽屉内呈现 reason）
节点标记 none         ⚪ 正常
         unfulfilled  🟡 存在未兑现声明
```

- **结构错不产生节点标记**：结构错一律以停摆态替代（见上一条），节点级「结构错」标记 MUST NOT 存在——否则等于承认「残缺图 + 红点」这一被否形态。
- **未知标记值** SHALL 退化为中性样式（`none`）并 MUST NOT 崩溃；词汇扩展 SHALL 走「改本 requirement + 同步词汇对拍语料」两处，MUST NOT 静默新增。
- **标记生产源（本变更解锁）**：标记 SHALL 由漂移报告派生——全量检查报告（`archi_drift` 的 `observations` 与 `suppressed` 段，见 desktop-archi-drift「报告结构与 status 五值」）经**取数侧纯函数**映射为图标记：`observations[].kind = unfulfilledDeclaration` 的 `(from, to)` → 该边 `unfulfilled` 且 `from` 节点 `unfulfilled`；`suppressed[]` 的 `(from, to)` → 该边 `exempt`（reason 随抽屉呈现）。映射 MUST 只读报告、MUST NOT 重算归属或判定（判定在 `desktop-archi-drift`）；`marks.ts` 的样式映射与词汇契约零改动（`unfulfilled` / `crossKind` / `exempt` / `none` 四值不变）。两挂载点 SHALL 同源取数、按模式分流：挂载点 A（`/architecture`）取 `full`，挂载点 B（change 详情）取 `incremental`（观察根为 `detail.worktree`，归档 change 回落主仓根）——同一纯函数映射，MUST NOT 出现第二套标记派生。

#### Scenario: 标记驱动边样式

- **WHEN** 图数据中某边标记为 `unfulfilled`、另一条为 `crossKind`、另一条为 `exempt`
- **THEN** 三条分别呈灰虚线 / 橙虚线 / 灰实线；`exempt` 边在抽屉内呈现 reason 文本；无标记（`none`）边呈实线

#### Scenario: 未知标记退化不崩

- **WHEN** 图数据中出现词汇外的标记值
- **THEN** 该边 / 节点按中性样式渲染，画布不崩溃、不空白

#### Scenario: 报告驱动标记

- **WHEN** 全量检查报告含一条 `observations[].kind = unfulfilledDeclaration`（`App.Billing` → `App.Auth`）与一条 `suppressed[]`（`App.Billing` → `App.Legacy`，reason 在位）
- **THEN** 对应两条边分别呈灰虚线（`unfulfilled`）与灰实线（`exempt`，抽屉内呈现 reason）、`App.Billing` 节点呈 🟡；报告未提及的边与节点保持 `none`

#### Scenario: 零报告或零漂移零标记

- **WHEN** 全量检查报告状态为 `clean` / `skipped` / `noChanges`，或取数失败
- **THEN** 全部边为 `none`（实线）、全部节点为 `none`（⚪ 正常），画布不空白、不报错、不重复取数

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `packages/desktop/src/views/architecture/lib/drift-marks.ts`（新） | 报告 → 标记纯函数 | `driftMarks(report, graph) -> ArchiGraph`：只读 `observations` / `suppressed` 两段，按 `(from, to)` 命中边、按 `from` 命中节点；零判定、零 IO、无渲染依赖 |
| `packages/desktop/src/views/architecture/hooks/use-archi-drift.ts`（新） | 漂移取数 | 经 `archi_drift(root, mode)` 显式取数（挂载 / 根切换 / 手动刷新三触发，零 watch 零轮询）；`mode` 由挂载点给定（A=`full` / B=`incremental`）；返回 `report` / `loading` / `error` |
| `packages/desktop/src/views/architecture/lib/marks.ts` | 样式映射（零改动） | 四值边标记与两值节点标记的样式契约不变；未知值仍退化中性 |
| `packages/desktop/src/views/architecture/components/architecture-canvas.tsx` | 接线点（两挂载点共用） | `graph = driftMarks(report, graph)` 后交渲染；漂移取数失败降级为「零标记 + 行内提示」，MUST NOT 阻断图渲染；`mode` 直接决定 `archi_drift` 的调用模式 |
