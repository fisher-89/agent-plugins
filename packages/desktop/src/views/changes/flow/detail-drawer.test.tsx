import { fireEvent, render, screen, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vite-plus/test';

import type { AttemptRecord, ChangeDetail } from '../../../types/dto';
import { mountMaterials } from './attachments';
import { DetailDrawer } from './detail-drawer';
import { buildFlowGraph } from './graph';
import { PIPELINE_PHASES } from './layout';
import type { DrawerSelection, FlowGraph, FlowMaterials } from './types';

// ---------------------------------------------------------------------------
// DetailDrawer 单测：右侧抽屉三分节（无跨进程 Mock —— ArtifactView 与
// FileLogTable 以真实实现消费；信封与 file_log 条目以 fixture 构造）。
// 列头与节点点击共用同一入口；selection 为 null 时不渲染（AC-5）。
// ---------------------------------------------------------------------------

function attempt(overrides: Partial<AttemptRecord> = {}): AttemptRecord {
  return {
    attempt: 1,
    verdict: 'pass',
    report: '评估记录',
    checklist: [],
    skipped: false,
    stale: false,
    startAt: null,
    timestamp: null,
    backtrackTo: null,
    backtrackReason: null,
    ...overrides,
  };
}

function detail(overrides: Partial<ChangeDetail> = {}): ChangeDetail {
  return {
    name: 'add-feature',
    source: 'active',
    inventory: 'v2',
    created: null,
    unparsable: false,
    pipeline: PIPELINE_PHASES.map((phase) => ({
      phase,
      attempts: phase === 'dev-design' ? [attempt({ attempt: 2 })] : [],
    })),
    activePhase: null,
    interrupted: [],
    fileLog: [],
    artifacts: [],
    ...overrides,
  };
}

function envelope(kind: string, title: string, payload: unknown) {
  return { kind, version: 1, title, payload, fallbackText: null };
}

interface World {
  graph: FlowGraph;
  materials: FlowMaterials;
  hasFileLog: boolean;
}

/** 真实 buildFlowGraph + mountMaterials 组装抽屉输入；按需覆盖素材面。 */
function world(
  overrides: {
    detail?: Partial<ChangeDetail>;
    materials?: Partial<FlowMaterials>;
    hasFileLog?: boolean;
  } = {},
): World {
  const base = detail(overrides.detail);
  const graph = buildFlowGraph(base);
  const materials = {
    ...mountMaterials(graph, base, []),
    ...overrides.materials,
  };
  return { graph, materials, hasFileLog: overrides.hasFileLog ?? base.fileLog !== null };
}

function renderDrawer(worldState: World, selection: DrawerSelection | null, onClose = vi.fn()) {
  return render(
    <DetailDrawer
      selection={selection}
      graph={worldState.graph}
      materials={worldState.materials}
      hasFileLog={worldState.hasFileLog}
      onClose={onClose}
    />,
  );
}

describe('DetailDrawer：三分节内容组装', () => {
  it('selection 为 eval 节点 → 三分节齐备：文档节经 ArtifactView 渲染、eval 节渲染 report 与挂载 checklist 信封、文件表节渲染 nodeFiles', () => {
    const state = world({
      materials: {
        columnDocs: {
          'col:dev-design': [envelope('markdown-doc', '设计文档', { markdown: '# 设计' })],
        },
        nodeChecklists: {
          'eval:dev-design:2': [
            envelope('eval-checklist', '评估清单', {
              phase: 'dev-design',
              attempt: 2,
              verdict: 'pass',
              items: [{ item: '接口已定', pass: true, evidence: 'L1-10' }],
            }),
          ],
        },
        nodeFiles: {
          'eval:dev-design:2': [
            { op: 'write', scope: 'dev-design', attempt: 2, path: 'src/design.ts', at: null },
          ],
        },
      },
    });
    renderDrawer(state, { scope: 'node', nodeId: 'eval:dev-design:2' });

    expect(screen.getByTestId('drawer-docs-section') !== null).toBe(true);
    expect(screen.getByTestId('drawer-eval-section') !== null).toBe(true);
    expect(screen.getByTestId('drawer-files-section') !== null).toBe(true);
    // 文档节：该列文档经 registry 渲染
    const docsSection = screen.getByTestId('drawer-docs-section');
    expect(within(docsSection).getByTestId('artifact-card').textContent).toContain('设计文档');
    // eval 节：record.report + 挂载的信封（而非内联清单）
    const evalSection = screen.getByTestId('drawer-eval-section');
    expect(evalSection.textContent).toContain('评估记录');
    expect(within(evalSection).getAllByTestId('artifact-card').length).toBe(1);
    expect(within(evalSection).getByTestId('checklist') !== null).toBe(true);
    // 文件表节：该节点 file_log 条目
    expect(
      within(screen.getByTestId('drawer-files-section')).getByTestId('filelog-table') !== null,
    ).toBe(true);
    expect(screen.getByTestId('detail-drawer').textContent).toContain(
      'dev-design · attempt 2 · eval',
    );
  });

  it('selection 为列头 → 本站文档节渲染该列文档，eval 节与文件表节呈空态', () => {
    const state = world({
      materials: {
        columnDocs: {
          'col:dev-design': [envelope('markdown-doc', '设计文档', { markdown: '# 设计' })],
        },
      },
    });
    renderDrawer(state, { scope: 'column', phase: 'dev-design' });

    expect(
      within(screen.getByTestId('drawer-docs-section')).getAllByTestId('artifact-card'),
    ).toHaveLength(1);
    expect(screen.getByTestId('drawer-eval-section').textContent).toContain('（无评估记录）');
    expect(screen.getByTestId('drawer-files-section').textContent).toContain(
      '（列头未对应单一 attempt，不聚合文件表）',
    );
    expect(screen.getByTestId('detail-drawer').textContent).toContain('dev-design');
  });

  it('遮罩点击或关闭按钮 → onClose 回调触发', () => {
    const onClose = vi.fn();
    const state = world();
    const { container } = renderDrawer(state, { scope: 'column', phase: 'dev-design' }, onClose);
    fireEvent.click(container.querySelector('div[aria-hidden="true"]')!);
    expect(onClose).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole('button', { name: '关闭' }));
    expect(onClose).toHaveBeenCalledTimes(2);
  });

  it('selection=null → 组件返回 null 不渲染', () => {
    const { container } = renderDrawer(world(), null);
    expect(container.childElementCount).toBe(0);
    expect(screen.queryByTestId('detail-drawer')).toBeNull();
  });

  it('eval 节点无挂载信封 → 回退内联渲染 record.checklist 条目（item / evidence / pass 徽标）', () => {
    const state = world({
      detail: {
        pipeline: PIPELINE_PHASES.map((phase) => ({
          phase,
          attempts: [
            attempt({
              checklist: [{ item: '含验收清单', pass: false, evidence: 'proposal 缺 AC 段' }],
            }),
          ],
        })),
      },
    });
    renderDrawer(state, { scope: 'node', nodeId: 'eval:proposal:1' });

    const evalSection = screen.getByTestId('drawer-eval-section');
    expect(within(evalSection).queryByTestId('artifact-card')).toBeNull();
    const checklist = within(evalSection).getByTestId('checklist');
    expect(checklist.textContent).toContain('含验收清单');
    expect(checklist.textContent).toContain('proposal 缺 AC 段');
    expect(within(checklist).getByTestId('checklist-verdict').textContent).toBe('fail');
    // 无挂载文件 → 文件表节空态占位
    expect(screen.getByTestId('drawer-files-section').textContent).toContain('（空）');
  });

  it('active / interrupted 节点选中 → eval 节空态（无 report / checklist 内容）', () => {
    const base = detail({
      activePhase: { phase: 'implement', attempt: 1, startAt: null },
      interrupted: [{ phase: 'test-gen', attempt: 1, startAt: null, endAt: null }],
    });
    const graph = buildFlowGraph(base);
    const state: World = {
      graph,
      materials: mountMaterials(graph, base, []),
      hasFileLog: true,
    };
    const { rerender } = renderDrawer(state, { scope: 'node', nodeId: 'active:implement:1' });
    expect(screen.getByTestId('drawer-eval-section').textContent).toContain('（无评估记录）');
    expect(screen.queryByTestId('checklist')).toBeNull();

    rerender(
      <DetailDrawer
        selection={{ scope: 'node', nodeId: 'interrupted:test-gen:1' }}
        graph={graph}
        materials={state.materials}
        hasFileLog
        onClose={() => {}}
      />,
    );
    expect(screen.getByTestId('drawer-eval-section').textContent).toContain('（无评估记录）');
  });

  it('v1 代际（hasFileLog=false）→ 文件表节呈降级文案', () => {
    const state = world({ hasFileLog: false });
    renderDrawer(state, { scope: 'node', nodeId: 'eval:dev-design:2' });
    expect(screen.getByTestId('drawer-files-section').textContent).toContain(
      '（无 file_log 数据：v1 及更早代际无此字段）',
    );
  });
});
