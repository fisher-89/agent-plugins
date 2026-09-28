/**
 * 右侧抽屉（单一交互入口）：列头与事件节点点击共用的三分节
 * 【本站文档 / eval report+checklist / 文件表】；selection 为 null 时不渲染。
 *
 * - 本站文档节：columnDocs[列 id] 逐个经 ArtifactView（renderers/registry）渲染；
 * - eval 节：record.report 文本 + checklist（有挂载 eval-checklist 信封走
 *   ArtifactView，无挂载时回退内联 record.checklist 条目）；active / interrupted
 *   与列头选中呈空态；
 * - 文件表节：节点选中渲染 nodeFiles[nodeId]；列头选中呈空态（attempt 作用域
 *   未定，不虚构聚合）；v1（hasFileLog = false）呈降级文案。
 *
 * hasFileLog 为 design 抽屉内容规则（v1 降级文案）的最小传参：v1 判定信号
 * （detail.fileLog === null）由页面组装层下探到抽屉文件表节。
 */
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';

import type { ArtifactEnvelope, ChecklistItem } from '../../../types/dto';
import { ArtifactView } from '../renderers/artifact-view';
import { FileLogTable } from './file-log-table';
import type { DrawerSelection, FileLogEntry, FlowGraph, FlowMaterials, FlowNode } from './types';

interface DetailDrawerProps {
  selection: DrawerSelection | null;
  graph: FlowGraph;
  materials: FlowMaterials;
  /** detail.fileLog !== null（v1 及更早代际无此字段 → false，文件表节降级） */
  hasFileLog: boolean;
  onClose: () => void;
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
        docs.map((envelope, index) => (
          <ArtifactView key={`${envelope.kind}-${envelope.title}-${index}`} envelope={envelope} />
        ))
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

/** 右侧抽屉：遮罩点击或关闭按钮置 selection 为 null（由页面组装层承载状态） */
export function DetailDrawer({
  selection,
  graph,
  materials,
  hasFileLog,
  onClose,
}: DetailDrawerProps): React.JSX.Element | null {
  if (selection === null) return null;
  const node =
    selection.scope === 'node'
      ? (graph.nodes.find((item) => item.id === selection.nodeId) ?? null)
      : null;
  const phase = selection.scope === 'column' ? selection.phase : (node?.phase ?? '');
  const columnId = `col:${phase}`;
  return (
    <div className="fixed inset-0 z-50" data-testid="detail-drawer">
      <div aria-hidden className="absolute inset-0 bg-black/50" onClick={onClose} />
      <aside className="absolute inset-y-0 right-0 flex w-[420px] max-w-[85vw] flex-col overflow-y-auto border-l border-border bg-card px-4 py-4">
        <header className="mb-3 flex items-center justify-between gap-2">
          <h2 className="m-0 truncate text-[15px]">{selectionTitle(selection, node)}</h2>
          <Button className="shrink-0" onClick={onClose}>
            关闭
          </Button>
        </header>
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
      </aside>
    </div>
  );
}
