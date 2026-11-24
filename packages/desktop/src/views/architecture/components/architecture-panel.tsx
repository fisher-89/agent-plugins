import { useMemo } from 'react';

import { useArchiDrift } from '../hooks/use-archi-drift';
import { useArchiGraph } from '../hooks/use-archi-graph';
import { driftMarks } from '../lib/drift-marks';
import { ArchitectureDashboard } from './dashboard/architecture-dashboard';
import { type CanvasMode } from './model-info';
import { StructureErrorPanel } from './structure-error-panel';

export interface ArchitectureCanvasProps {
  /** 观察根（`null` = 壳态无根：不发起取数） */
  root: string | null;
  /** 能力差异：`full` 全量只读 / `incremental` 增量可改档 */
  mode: CanvasMode;
}

/** 空模型提示（结构性早退分支，画布零节点零边）。 */
function ModelEmptyNotice(): React.JSX.Element {
  return (
    <div className="text-[13px] text-muted-foreground" data-testid="archi-canvas-empty">
      模型为空：先在 change 的 dev-design 阶段生成骨架（新元素缺省 gate = off）。
    </div>
  );
}

export function ArchitecturePanel({ root, mode }: ArchitectureCanvasProps): React.JSX.Element {
  const { graph, loading, error, refresh } = useArchiGraph(root);
  // 漂移取数：与图同一观察根、模式随挂载点分流（`full` / `incremental`）；
  // 标记生产在 drift-marks 纯函数，本组件只做一行合并
  const { report, error: driftError } = useArchiDrift(root, mode);
  const marked = useMemo(
    () => (graph === null ? null : driftMarks(report, graph)),
    [graph, report],
  );
  // 豁免来源索引（关系面板空态③ 的 `via`）：报告已取回、零新 invoke；`drift-marks`
  // 与 `ArchiRelation` 的形状都不动
  const exemptVias = useMemo(() => {
    const vias = new Map<string, 'config' | 'inline'>();
    for (const entry of report?.suppressed ?? []) {
      vias.set(`${entry.from}->${entry.to}`, entry.via);
    }
    return vias;
  }, [report]);
  if (loading && graph === null) {
    return (
      <div className="text-[13px] text-muted-foreground" data-testid="archi-canvas-loading">
        解析中…
      </div>
    );
  }
  if (error !== null) {
    return (
      <div
        className="break-all rounded-md bg-fail-bg px-3 py-2 text-[13px] text-fail"
        data-testid="archi-canvas-error"
      >
        {error}
      </div>
    );
  }
  if (graph === null || marked === null) {
    return (
      <div className="text-[13px] text-muted-foreground" data-testid="archi-canvas-empty">
        无工作区根：无法解析架构模型。
      </div>
    );
  }
  // 全或无停摆：任何结构错 ⇒ 只渲错误面板（零图数据、零降级样式）
  if (graph.diagnostics.length > 0) {
    return (
      <div data-testid="archi-canvas">
        <StructureErrorPanel diagnostics={graph.diagnostics} />
      </div>
    );
  }
  // 模型为空
  if (graph.elements.length === 0) {
    return <ModelEmptyNotice />;
  }
  return (
    <ArchitectureDashboard
      graph={marked}
      root={root}
      mode={mode}
      onChanged={refresh}
      driftError={driftError}
      apiFindings={report?.apiFindings ?? []}
      exemptVias={exemptVias}
    />
  );
}
