/**
 * 漂移标记 → 视觉映射（封闭词汇，纯函数）。
 *
 * 边四值：`none` 实线（本期恒此值）/ `unfulfilled` 灰虚线 / `crossKind` 橙虚线 /
 * `exempt` 灰实线；节点两值：`unfulfilled` 🟡 / 其余 ⚪。词汇**外**的值退化为中性
 * 样式且不抛错（标记扩展走「改能力 spec + 同步对拍语料」两处，不得静默新增）。
 *
 * 标记的**产生**（何时算漂移）不在本模块——本期生产者恒空，渲染契约先立。
 */
export interface EdgeVisual {
  /** 描边色（CSS 变量，随主题） */
  stroke: string;
  /** 虚线模式（null = 实线） */
  dasharray: string | null;
}

/**
 * 边标记 → 描边视觉；词汇外与 `none` 同款中性。
 *
 * 入参按 `string` 收面（`EdgeMark` 是其子集）：词汇封闭由退化分支承载——运行时
 * 拿到的词汇外值不崩溃、不空白，扩展词汇仍须「改能力 spec + 同步对拍语料」两处。
 */
export function edgeVisual(mark: string): EdgeVisual {
  switch (mark) {
    case 'unfulfilled':
      return { stroke: 'var(--muted-foreground)', dasharray: '6 4' };
    case 'crossKind':
      return { stroke: 'var(--warn)', dasharray: '4 3' };
    case 'exempt':
      return { stroke: 'var(--muted-foreground)', dasharray: null };
    default:
      return { stroke: 'var(--chart-2)', dasharray: null };
  }
}

/**
 * 节点标记 → 状态点；词汇外一律 ⚪ 正常（同 `edgeVisual` 的退化口径：`NodeMark`
 * 两值是 `string` 的子集，退化分支即词汇封闭的执行点）。
 */
export function nodeDot(mark: string): string {
  return mark === 'unfulfilled' ? '🟡' : '⚪';
}
