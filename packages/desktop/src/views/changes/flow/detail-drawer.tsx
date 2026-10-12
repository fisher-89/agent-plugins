import { StandardDrawer } from '@/components/standard/drawer';

import type { ArtifactEnvelope } from '../../../types/dto';
import { ArtifactTabs } from '../renderers/artifact-tabs';
import { SessionTranscriptPanel } from './session-transcript-panel';
import { StdoutPanel } from './stdout-panel';
import type { DrawerSelection, FlowGraph, FlowMaterials, FlowNode, RoleSessionRef } from './types';

interface DetailDrawerProps {
  selection: DrawerSelection | null;
  graph: FlowGraph;
  materials: FlowMaterials;
  /** workspace root（会话转录反查） */
  root: string | null;
  /** change id（sourceRef 定式组装 `<id>/<phase>/<role>/<attempt>`——页内
   * 单一身份源 = URL id） */
  changeId: string;
  /** change name（`read_exec_output` 磁盘面寻址键——与 `record_exec_output`
   * 同口径取 `detail.name`，非 id） */
  name: string;
  /** 变更通知触发的转录重查键（实时面统一转录库重查——liveEvents 退役） */
  transcriptRefreshKey?: number;
  onClose: () => void;
}

/**
 * 选中对象 → 会话转录联动寻址键组
 */
function selectionRoleRef(
  selection: DrawerSelection,
  node: FlowNode | null,
  changeId: string,
): RoleSessionRef | null {
  if (selection.scope !== 'node' || node === null || node.role === null) return null;
  return {
    role: node.role,
    sessionId: node.sessionId,
    sourceRef: `${changeId}/${node.phase}/${node.role}/${node.attempt}`,
  };
}

/** 选中标题：列头为 phase 名；节点为 phase + attempt + kind（节点缺失时中性占位） */
function selectionTitle(selection: DrawerSelection, node: FlowNode | null): string {
  if (selection.scope === 'column') return selection.phase;
  if (node === null) return '事件节点';
  return `${node.phase} · attempt ${node.attempt} · ${node.kind}`;
}

/** 列头单列：该站过程素材（过程文档 + eval-checklist 信封）经 ArtifactTabs 呈现 */
function ColumnMaterialsSection({
  materials,
}: {
  materials: ArtifactEnvelope[];
}): React.JSX.Element {
  return (
    <section className="min-h-0 flex-1 overflow-y-auto" data-testid="drawer-column-section">
      <h3 className="m-0 mb-2 text-[13px] text-muted-foreground">本站文档</h3>
      {materials.length === 0 ? (
        <div className="text-muted-foreground">（无本站文档）</div>
      ) : (
        <ArtifactTabs artifacts={materials} />
      )}
    </section>
  );
}

/** 无信息源选中（悬空节点 id / 双 null 节点直选）的中性空态占位 */
function DrawerEmptyPlaceholder(): React.JSX.Element {
  return (
    <div className="text-[13px] text-muted-foreground" data-testid="drawer-session-empty">
      （当前选中无关联会话）
    </div>
  );
}

/**
 * 抽屉内容单列分派（信息源判据）：
 * - 列头 → 该站过程素材（文档 + eval-checklist）；
 * - WorkerAgent 步节点（`role !== null`）→ 会话转录（sessionId 直查优先 /
 *   sourceRef 反查兜底）；
 * - 带输出引用的步节点（`outputRef !== null`，static-check / test-execution）→
 *   stdout 面板；
 * - 其余（悬空 / 无信息源）→ 中性空态占位（无 eval 分支、无「评估记录」节）。
 */
function DrawerContent({
  selection,
  node,
  columnId,
  materials,
  root,
  changeId,
  name,
  transcriptRefreshKey,
}: {
  selection: DrawerSelection;
  node: FlowNode | null;
  columnId: string;
  materials: FlowMaterials;
  root: string | null;
  changeId: string;
  name: string;
  transcriptRefreshKey?: number;
}): React.JSX.Element {
  if (selection.scope === 'column') {
    return <ColumnMaterialsSection materials={materials.columnDocs[columnId] ?? []} />;
  }
  if (node !== null && node.role !== null) {
    return (
      <SessionTranscriptPanel
        root={root}
        roleRef={selectionRoleRef(selection, node, changeId)}
        refreshKey={transcriptRefreshKey}
      />
    );
  }
  if (node !== null && node.outputRef !== null) {
    return <StdoutPanel root={root} changeName={name} execId={node.outputRef} />;
  }
  return <DrawerEmptyPlaceholder />;
}

/** 右侧抽屉（单列）：遮罩点击或关闭按钮置 selection 为 null（由页面组装层承载状态） */
export function DetailDrawer({
  selection,
  graph,
  materials,
  root,
  changeId,
  name,
  transcriptRefreshKey,
  onClose,
}: DetailDrawerProps): React.JSX.Element | null {
  if (selection === null) return null;
  const node =
    selection.scope === 'node'
      ? (graph.nodes.find((item) => item.id === selection.nodeId) ?? null)
      : null;
  const phase = selection.scope === 'column' ? selection.phase : (node?.phase ?? '');
  return (
    <StandardDrawer
      data-testid="detail-drawer"
      open={true}
      onOpenChange={(next) => {
        if (!next) onClose();
      }}
      title={selectionTitle(selection, node)}
      containerClassName="w-[960px] max-w-[85vw]"
      content={
        <div className="flex min-h-0 flex-1 flex-col">
          <DrawerContent
            selection={selection}
            node={node}
            columnId={`col:${phase}`}
            materials={materials}
            root={root}
            changeId={changeId}
            name={name}
            transcriptRefreshKey={transcriptRefreshKey}
          />
        </div>
      }
    />
  );
}
