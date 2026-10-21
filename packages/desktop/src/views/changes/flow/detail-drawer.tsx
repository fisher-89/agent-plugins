import { useEffect } from 'react';

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
  /** 会话状态重评估键（组件据其重评估是否挂流；会话事件通知并入统一重查） */
  sessionRefreshKey?: number;
  onClose: () => void;
}

/**
 * 选中对象 → 会话转录联动寻址键组
 */
function selectionRoleRef(
  selection: DrawerSelection,
  node: FlowNode | null,
): RoleSessionRef | null {
  if (selection.scope !== 'node' || node === null || node.role === null) return null;
  return {
    role: node.role,
    sessionId: node.sessionId,
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

/** WorkerAgent 步无会话槽位（role 在场而 `sessionId` 缺席）的步面空态占位——
 * sourceRef 定式反查退役后不再按定式回退寻址，如实呈「此步无会话记录」。 */
function StepSessionEmptyPlaceholder(): React.JSX.Element {
  return (
    <div className="text-[13px] text-muted-foreground" data-testid="drawer-session-empty">
      此步无会话记录
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
  name,
  sessionRefreshKey,
}: {
  selection: DrawerSelection;
  node: FlowNode | null;
  columnId: string;
  materials: FlowMaterials;
  root: string | null;
  changeId: string;
  name: string;
  sessionRefreshKey?: number;
}): React.JSX.Element {
  if (selection.scope === 'column') {
    return <ColumnMaterialsSection materials={materials.columnDocs[columnId] ?? []} />;
  }
  if (node !== null && node.role !== null) {
    // 会话槽位缺席（旧条目 / 未回写）：反查定式退役后不再回退寻址，步面空态兜底
    if (node.sessionId === null) return <StepSessionEmptyPlaceholder />;
    return (
      <SessionTranscriptPanel
        root={root}
        roleRef={selectionRoleRef(selection, node)}
        refreshKey={sessionRefreshKey}
      />
    );
  }
  if (node !== null && node.outputRef !== null) {
    return <StdoutPanel root={root} changeName={name} execId={node.outputRef} />;
  }
  return <DrawerEmptyPlaceholder />;
}

/** 右侧抽屉（单列，非模态）：关闭经页脚「关闭」按钮与 Esc 置 selection 为
 * null（由页面组装层承载状态）；非模态下无「点遮罩关闭」——抽屉不遮蔽页面
 * 其余区域（左栏通用 Agent 面板保持可交互）。 */
export function DetailDrawer({
  selection,
  graph,
  materials,
  root,
  changeId,
  name,
  sessionRefreshKey,
  onClose,
}: DetailDrawerProps): React.JSX.Element | null {
  // Esc 关闭兜底（非模态下 base-ui 的 dismiss 不保证触发）：选中缺席时不挂监
  // 听；与库自身 dismiss 并存无双触发副作用（`onClose` 置 null 幂等）
  useEffect(() => {
    if (selection === null) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [selection, onClose]);
  if (selection === null) return null;
  const node =
    selection.scope === 'node'
      ? (graph.nodes.find((item) => item.id === selection.nodeId) ?? null)
      : null;
  const phase = selection.scope === 'column' ? selection.phase : (node?.phase ?? '');
  return (
    <StandardDrawer
      data-testid="detail-drawer"
      modal={false}
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
            sessionRefreshKey={sessionRefreshKey}
          />
        </div>
      }
    />
  );
}
