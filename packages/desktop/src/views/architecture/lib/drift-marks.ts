import type { ArchiDriftReport, ArchiGraph } from '../../../types/generated/bindings';

/**
 * 漂移报告 → 图标记（纯函数：只读报告、零判定、零 IO、零渲染依赖）。
 *
 * 只消费报告的两段（判定在 `desktop-archi-drift` 的判定纯函数里，本函数 MUST NOT
 * 重算归属或违例）：
 * - `observations[].kind === 'unfulfilledDeclaration'` → 该 `(from, to)` 边
 *   `unfulfilled`，且 `from` 节点 `unfulfilled`（🟡「存在未兑现声明」）；
 * - `suppressed[]` → 该 `(from, to)` 边 `exempt`（`reason` 随抽屉呈现）。
 *
 * 两段命中同一元素对时取 `unfulfilled`（冲突只可能来自拼接了不同时点的报告；
 * 「未兑现」是更值得提示的偏差）。`marks.ts` 的四值边标记 / 两值节点标记样式契约
 * **零改动**——本函数只填标记值。
 */
export function driftMarks(report: ArchiDriftReport | null, graph: ArchiGraph): ArchiGraph {
  if (report === null) return graph;

  const exempt = new Map<string, string>();
  for (const entry of report.suppressed) {
    exempt.set(`${entry.from}->${entry.to}`, entry.reason);
  }

  const unfulfilled = new Set<string>();
  const unfulfilledNodes = new Set<string>();
  for (const observation of report.observations) {
    if (observation.kind !== 'unfulfilledDeclaration') continue;
    unfulfilled.add(`${observation.from}->${observation.to}`);
    unfulfilledNodes.add(observation.from);
  }

  const relations = graph.relations.map((relation) => {
    const key = `${relation.from}->${relation.to}`;
    if (unfulfilled.has(key)) {
      return { ...relation, mark: 'unfulfilled' as const, reason: null };
    }
    const reason = exempt.get(key);
    if (reason !== undefined) {
      return { ...relation, mark: 'exempt' as const, reason };
    }
    return relation;
  });

  const elements = graph.elements.map((element) =>
    unfulfilledNodes.has(element.fqn) ? { ...element, mark: 'unfulfilled' as const } : element,
  );

  return { ...graph, elements, relations };
}
