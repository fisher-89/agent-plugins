import { useEffect, useMemo, useState } from 'react';

import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from '@/components/ui';
import {
  type ArchiApiFinding,
  type ArchiGraph,
  type ArchiRelation,
} from '@/types/generated/bindings';

import { type CanvasMode } from '../model-info';
import { ArchitectureNode } from './architecture-node';
import { TreeColumn, type TreeMode } from './tree-column';
import { useArchiRelation, type ArchiRelationState, type RelationEdge } from './use-archi-relation';

/** 关系态面板的全部入参（选中边 + 图投影 + 取数状态 + 豁免来源）。 */
interface RelationSelection {
  edge: RelationEdge;
  relation: ArchiRelation;
  state: ArchiRelationState;
  exemptVia: 'config' | 'inline' | null;
}

/** 选中边 → 图里的关系投影（元素对已不在图内即 null：派生态失效，按「无选中」渲染）。 */
function relationOf(relations: ArchiRelation[], edge: RelationEdge | null): ArchiRelation | null {
  if (edge === null) return null;
  return (
    relations.find((relation) => relation.from === edge.from && relation.to === edge.to) ?? null
  );
}

/** 缺省聚焦：首个顶层元素，无顶层则首个元素。 */
function defaultFocus(graph: ArchiGraph): string | null {
  const root = graph.elements.find((element) => !element.fqn.includes('.'));
  return root?.fqn ?? graph.elements[0]?.fqn ?? null;
}

/** 聚焦元素：显式覆盖仍在图内即用之，否则回落缺省聚焦。 */
function focusOf(graph: ArchiGraph, override: string | null): string | null {
  if (override !== null && graph.elements.some((element) => element.fqn === override)) {
    return override;
  }
  return defaultFocus(graph);
}

/** 选中边 + 取数状态 + 豁免来源 → 关系态面板入参（无选中 / 派生态失效即 null）。 */
function selectionOf(
  edge: RelationEdge | null,
  relation: ArchiRelation | null,
  state: ArchiRelationState,
  exemptVias: Map<string, 'config' | 'inline'>,
): RelationSelection | null {
  if (edge === null || relation === null) return null;
  return {
    edge,
    relation,
    state,
    exemptVia: exemptVias.get(`${edge.from}->${edge.to}`) ?? null,
  };
}

export function ArchitectureDashboard({
  graph,
  root,
  mode,
  onChanged,
  driftError,
  apiFindings,
  exemptVias,
}: {
  graph: ArchiGraph;
  root: string | null;
  mode: CanvasMode;
  onChanged: () => void;
  driftError: string | null;
  apiFindings: ArchiApiFinding[];
  /** 豁免来源索引（键 = `from->to`；取自漂移报告的 `suppressed[]`，零新 invoke） */
  exemptVias: Map<string, 'config' | 'inline'>;
}): React.JSX.Element {
  const [focusOverride, setFocusOverride] = useState<string | null>(null);
  const [treeMode, setTreeMode] = useState<TreeMode>('element');
  const [selectedEdge, setSelectedEdge] = useState<RelationEdge | null>(null);
  const focus = useMemo(() => focusOf(graph, focusOverride), [graph, focusOverride]);
  const focused =
    focus === null ? null : (graph.elements.find((element) => element.fqn === focus) ?? null);
  const relations: ArchiRelation[] = graph.relations;
  // 选中边是派生态：元素对已不在图内即按「无选中」渲染（不用副作用清，免得抖动）
  const selectedRelation = relationOf(relations, selectedEdge);
  // 关系详情按需取数：`apis` 为空的边不取（没有可比对的清单，取数只是空转）
  const relationState = useArchiRelation(
    root,
    selectedRelation !== null && selectedRelation.apis.length > 0 ? selectedEdge : null,
  );
  const selection = selectionOf(selectedEdge, selectedRelation, relationState, exemptVias);
  // `root` 变化即清选中（回到元素态；显式刷新经 `key` 重挂载自然复位）
  useEffect(() => {
    setSelectedEdge(null);
  }, [root]);
  return (
    <ResizablePanelGroup className="flex h-full min-h-0 gap-2">
      <ResizablePanel defaultSize={300} minSize={200}>
        <TreeColumn
          root={root}
          canvasMode={mode}
          mode={treeMode}
          onModeChange={setTreeMode}
          elements={graph.elements}
          focused={focus}
          onFocus={setFocusOverride}
          findings={apiFindings}
        />
      </ResizablePanel>
      <ResizableHandle />
      <ArchitectureNode
        graph={graph}
        focus={focus}
        setFocusOverride={setFocusOverride}
        driftError={driftError}
        selectedEdge={selectedEdge}
        onSelectEdge={setSelectedEdge}
        inspector={{
          root,
          mode,
          focused,
          relations,
          findings: apiFindings,
          onChanged,
          selection,
          onRetry: relationState.retry,
        }}
      />
    </ResizablePanelGroup>
  );
}
