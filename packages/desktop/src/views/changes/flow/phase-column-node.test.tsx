import { fireEvent, render, screen } from '@testing-library/react';
import type { ComponentProps } from 'react';
import { describe, expect, it, vi } from 'vite-plus/test';

import type { ArtifactEnvelope } from '../../../types/dto';
import { type ColumnFlowNode, PhaseColumnNode } from './phase-column-node';

// ---------------------------------------------------------------------------
// PhaseColumnNode 单测：直渲染 + spy 回调（无 Mock 依赖）；与 ReactFlow subflow
// 的挂载关系由 ChangeFlowGraph 与集成测试覆盖。列容器无 Handle，无需 Provider。
// ---------------------------------------------------------------------------

function doc(title: string): ArtifactEnvelope {
  return {
    kind: 'markdown-doc',
    version: 1,
    title,
    payload: { markdown: '# 文档' },
    fallbackText: null,
  };
}

function renderColumn(
  data: Partial<NonNullable<ColumnFlowNode['data']>> & { phase: string },
): ReturnType<typeof render> {
  const props = { data, selected: false } as unknown as ComponentProps<typeof PhaseColumnNode>;
  return render(<PhaseColumnNode {...props} />);
}

describe('PhaseColumnNode：phase 列容器节点', () => {
  it('列头渲染 phase 名，点击触发 onSelect 且载荷为 { scope: column, phase }', () => {
    const onSelect = vi.fn();
    renderColumn({ phase: 'implement', docs: [], onSelect });
    fireEvent.click(screen.getByTestId('flow-column'));
    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(onSelect).toHaveBeenCalledWith({ scope: 'column', phase: 'implement' });
  });

  it('该站有过程文档 → 徽章呈现该站文档计数', () => {
    renderColumn({ phase: 'dev-design', docs: [doc('设计'), doc('任务进度')], onSelect: vi.fn() });
    expect(screen.getByTestId('column-docs').textContent).toBe('2 份文档');
  });

  it('空列（无事件节点、无文档）→ 列头照常渲染、无徽章', () => {
    renderColumn({ phase: 'code-analyze', docs: [], onSelect: vi.fn() });
    expect(screen.getByTestId('flow-column').textContent).toContain('code-analyze');
    expect(screen.queryByTestId('column-docs')).toBeNull();
  });

  it('docs 零态（调用侧对缺失键兜底为空数组的形态）且 onSelect 未传入 → 列头照常渲染，点击为 no-op 不抛错', () => {
    renderColumn({ phase: 'proposal', docs: [] });
    expect(screen.getByTestId('flow-column').textContent).toContain('proposal');
    expect(() => fireEvent.click(screen.getByTestId('flow-column'))).not.toThrow();
  });
});
