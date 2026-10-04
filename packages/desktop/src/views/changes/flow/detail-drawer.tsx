import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';

import type { AgentEvent, ArtifactEnvelope, ChecklistItem } from '../../../types/dto';
import { ArtifactTabs } from '../renderers/artifact-tabs';
import { ArtifactView } from '../renderers/artifact-view';
import { FileLogTable } from './file-log-table';
import { SessionTranscriptPanel } from './session-transcript-panel';
import type {
  DrawerSelection,
  FileLogEntry,
  FlowGraph,
  FlowMaterials,
  FlowNode,
  FlowRoleLabel,
  RoleSessionRef,
} from './types';

interface DetailDrawerProps {
  selection: DrawerSelection | null;
  graph: FlowGraph;
  materials: FlowMaterials;
  /** detail.fileLog !== null（v1 及更早代际无此字段 → false，文件表节降级） */
  hasFileLog: boolean;
  /** workspace root（会话转录反查） */
  root: string | null;
  /** change 名（sourceRef 定式组装 `<change>/<phase>/<role>/<attempt>`） */
  change: string;
  /** run 实时事件缓存（sessionId 载荷；抽屉按选中节点过滤） */
  liveEvents: Array<{ sessionId: string; event: AgentEvent }>;
  onClose: () => void;
}

/**
 * 选中对象 → 会话转录联动寻址键组
 */
function selectionRoleRefs(
  selection: DrawerSelection,
  node: FlowNode | null,
  change: string,
): RoleSessionRef[] {
  if (selection.scope !== 'node' || node === null) return [];
  if (node.kind === 'runtime') {
    if (node.role === null) return [];
    return [
      {
        role: node.role,
        sessionId: node.sessionId,
        sourceRef: `${change}/${node.phase}/${node.role}/${node.attempt}`,
      },
    ];
  }
  if (node.kind === 'eval') {
    const attempt = node.record.attempt;
    if (attempt === null) return [];
    return [
      {
        role: 'executor',
        sessionId: node.record.executorSessionId ?? null,
        sourceRef: `${change}/${node.phase}/executor/${attempt}`,
      },
      {
        role: 'evaluator',
        sessionId: node.record.evaluatorSessionId ?? null,
        sourceRef: `${change}/${node.phase}/evaluator/${attempt}`,
      },
      {
        role: 'decision',
        sessionId: node.record.decisionSessionId ?? null,
        // decision 槽位缺席即双 null → 空态（不误挂他 attempt 会话）
        sourceRef: null,
      },
    ];
  }
  if (node.kind === 'active') {
    const ref = (role: FlowRoleLabel): RoleSessionRef => ({
      role,
      sessionId: null,
      sourceRef: `${change}/${node.phase}/${role}/${node.attempt}`,
    });
    return [ref('executor'), ref('evaluator'), ref('decision')];
  }
  return [];
}

function InlineChecklist({ items }: { items: ChecklistItem[] }): React.JSX.Element {
  if (items.length === 0) return <div className="text-muted-foreground">（清单为空）</div>;
  return (
    <ul className="m-0 mt-1.5 list-none p-0" data-testid="checklist">
      {items.map((entry, index) => (
        <li key={index} className="flex items-baseline gap-2 py-[3px]">
          <Badge variant={entry.pass ? 'pass' : 'fail'} data-testid="checklist-verdict">
            {entry.pass ? 'pass' : 'fail'}
          </Badge>
          <div>
            <div className="border-b border-dashed border-border py-1">{entry.item}</div>
            <div className="break-words text-xs text-muted-foreground">{entry.evidence}</div>
          </div>
        </li>
      ))}
    </ul>
  );
}

function DrawerDocsSection({ docs }: { docs: ArtifactEnvelope[] }): React.JSX.Element {
  return (
    <section className="mb-4" data-testid="drawer-docs-section">
      <h3 className="m-0 mb-2 text-[13px] text-muted-foreground">本站文档</h3>
      {docs.length === 0 ? (
        <div className="text-muted-foreground">（无本站文档）</div>
      ) : (
        <ArtifactTabs artifacts={docs} />
      )}
    </section>
  );
}

function DrawerEvalSection({
  node,
  checklists,
}: {
  node: FlowNode | null;
  checklists: ArtifactEnvelope[];
}): React.JSX.Element {
  return (
    <section className="mb-4" data-testid="drawer-eval-section">
      <h3 className="m-0 mb-2 text-[13px] text-muted-foreground">评估记录</h3>
      {node?.kind !== 'eval' ? (
        <div className="text-muted-foreground">（无评估记录）</div>
      ) : (
        <div>
          <div className="my-1 whitespace-pre-wrap break-words">{node.record.report}</div>
          {checklists.length > 0 ? (
            checklists.map((envelope, index) => (
              <ArtifactView
                key={`${envelope.kind}-${envelope.title}-${index}`}
                envelope={envelope}
              />
            ))
          ) : (
            <InlineChecklist items={node.record.checklist} />
          )}
        </div>
      )}
    </section>
  );
}

function DrawerFilesSection({
  nodeId,
  files,
  hasFileLog,
}: {
  nodeId: string | null;
  files: FileLogEntry[];
  hasFileLog: boolean;
}): React.JSX.Element {
  return (
    <section className="mb-4" data-testid="drawer-files-section">
      <h3 className="m-0 mb-2 text-[13px] text-muted-foreground">文件清单</h3>
      {!hasFileLog ? (
        <div className="text-muted-foreground">（无 file_log 数据：v1 及更早代际无此字段）</div>
      ) : nodeId === null ? (
        <div className="text-muted-foreground">（列头未对应单一 attempt，不聚合文件表）</div>
      ) : (
        <FileLogTable entries={files} />
      )}
    </section>
  );
}

/** 选中标题：列头为 phase 名；节点为 phase + attempt + kind（节点缺失时中性占位） */
function selectionTitle(selection: DrawerSelection, node: FlowNode | null): string {
  if (selection.scope === 'column') return selection.phase;
  if (node === null) return '事件节点';
  const attempt = node.kind === 'eval' ? node.record.attempt : node.attempt;
  return `${node.phase} · attempt ${attempt ?? '—'} · ${node.kind}`;
}

/** 右列三分节（本站文档 / 评估记录 / 文件表）：列内滚动，节结构与 data-testid
 * 锚点零变化。 */
function RightSections({
  node,
  materials,
  columnId,
  hasFileLog,
}: {
  node: FlowNode | null;
  materials: FlowMaterials;
  columnId: string;
  hasFileLog: boolean;
}): React.JSX.Element {
  return (
    <div className="min-w-0 flex-1 overflow-y-auto">
      <DrawerDocsSection docs={materials.columnDocs[columnId] ?? []} />
      <DrawerEvalSection
        node={node}
        checklists={node === null ? [] : (materials.nodeChecklists[node.id] ?? [])}
      />
      <DrawerFilesSection
        nodeId={node?.id ?? null}
        files={node === null ? [] : (materials.nodeFiles[node.id] ?? [])}
        hasFileLog={hasFileLog}
      />
    </div>
  );
}

/** 右侧抽屉：遮罩点击或关闭按钮置 selection 为 null（由页面组装层承载状态） */
export function DetailDrawer({
  selection,
  graph,
  materials,
  hasFileLog,
  root,
  change,
  liveEvents,
  onClose,
}: DetailDrawerProps): React.JSX.Element | null {
  if (selection === null) return null;
  const node =
    selection.scope === 'node'
      ? (graph.nodes.find((item) => item.id === selection.nodeId) ?? null)
      : null;
  const phase = selection.scope === 'column' ? selection.phase : (node?.phase ?? '');
  const columnId = `col:${phase}`;
  const roleRefs = selectionRoleRefs(selection, node, change);
  // 实时事件按选中节点的会话过滤（运行步节点携带 sessionId；历史节点无实时流）
  const sessionId = node?.kind === 'runtime' ? node.sessionId : null;
  const live =
    sessionId === null
      ? []
      : liveEvents.filter((entry) => entry.sessionId === sessionId).map((entry) => entry.event);
  return (
    <div className="fixed inset-0 z-50" data-testid="detail-drawer">
      <div aria-hidden className="absolute inset-0 bg-black/50" onClick={onClose} />
      <aside className="absolute inset-y-0 right-0 flex w-[960px] max-w-[85vw] flex-col border-l border-border bg-card px-4 py-4">
        <header className="mb-3 flex items-center justify-between gap-2">
          <h2 className="m-0 truncate text-[15px]">{selectionTitle(selection, node)}</h2>
          <Button className="shrink-0" onClick={onClose}>
            关闭
          </Button>
        </header>
        <div className="flex min-h-0 flex-1">
          <div className="flex w-[60%] min-w-0 flex-col">
            <SessionTranscriptPanel root={root} roleRefs={roleRefs} liveEvents={live} />
          </div>
          <RightSections
            node={node}
            materials={materials}
            columnId={columnId}
            hasFileLog={hasFileLog}
          />
        </div>
      </aside>
    </div>
  );
}
