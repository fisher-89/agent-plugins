import { render, screen, within } from '@testing-library/react';
import { ReactFlowProvider } from '@xyflow/react';
import type { ComponentProps } from 'react';
import { describe, expect, it } from 'vite-plus/test';

import type { AttemptRecord } from '../../../types/dto';
import { FlowEventNode } from './flow-event-node';
import type { ActiveFlowNode, EvalFlowNode, FlowNode, InterruptedFlowNode } from './types';

// ---------------------------------------------------------------------------
// FlowEventNode 单测：自定义节点为纯渲染组件，以最小 NodeProps 形态（data 注入
// FlowNode 载荷）直接 render；不挂载 ReactFlow 本体，无 ResizeObserver 依赖。
// Handle 依赖 ReactFlow store context，故外包 ReactFlowProvider（组件头注约定）。
// 三分类视觉迁移旧页面 Attempt 块语义（AC-2）。
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
    executorSessionId: null,
    evaluatorSessionId: null,
    decisionSessionId: null,
    ...overrides,
  };
}

function evalNode(record: AttemptRecord, phase = 'dev-design'): EvalFlowNode {
  return {
    id: `eval:${phase}:${record.attempt ?? 0}`,
    kind: 'eval',
    phase,
    attempt: record.attempt ?? 0,
    colIndex: 1,
    order: 0,
    parentId: `col:${phase}`,
    record,
  };
}

function activeNode(overrides: Partial<ActiveFlowNode> = {}): ActiveFlowNode {
  return {
    id: 'active:implement:2',
    kind: 'active',
    phase: 'implement',
    attempt: 2,
    colIndex: 3,
    order: 0,
    parentId: 'col:implement',
    startAt: '2026-09-04T09:00:00Z',
    ...overrides,
  };
}

function interruptedNode(overrides: Partial<InterruptedFlowNode> = {}): InterruptedFlowNode {
  return {
    id: 'interrupted:implement:2',
    kind: 'interrupted',
    phase: 'implement',
    attempt: 2,
    colIndex: 3,
    order: 0,
    parentId: 'col:implement',
    startAt: '2026-09-03T20:00:00Z',
    endAt: '2026-09-04T08:00:00Z',
    ...overrides,
  };
}

/** 最小 NodeProps 形态：仅注入 data 载荷，其余字段非本组件消费面。 */
function renderNode(node: FlowNode) {
  const props = { data: { node } } as unknown as ComponentProps<typeof FlowEventNode>;
  return render(
    <ReactFlowProvider>
      <FlowEventNode {...props} />
    </ReactFlowProvider>,
  );
}

describe('FlowEventNode：eval 节点三分类视觉', () => {
  it('verdict pass → 实心 pass 底色与 pass 徽标文案；verdict fail → fail 底色与 fail 徽标文案', () => {
    const pass = renderNode(evalNode(attempt({ verdict: 'pass' })));
    const passNode = screen.getByTestId('flow-node');
    expect(passNode.className).toContain('bg-pass-bg');
    expect(within(passNode).getByTestId('attempt-verdict').textContent).toBe('pass');
    pass.unmount();

    const fail = renderNode(evalNode(attempt({ verdict: 'fail' })));
    const failNode = screen.getByTestId('flow-node');
    expect(failNode.className).toContain('bg-fail-bg');
    expect(within(failNode).getByTestId('attempt-verdict').textContent).toBe('fail');
    fail.unmount();
  });

  it('stale=true → 半透明淡化标记存在，节点信息仍完整可读', () => {
    renderNode(evalNode(attempt({ stale: true, report: '废弃支线评估' })));
    const node = screen.getByTestId('flow-node');
    expect(node.className).toContain('opacity-50');
    expect(node.textContent).toContain('stale');
    expect(node.textContent).toContain('attempt 1');
  });

  it('全缺省 eval → 无任何徽标与淡化（不渲染空占位）', () => {
    renderNode(evalNode(attempt()));
    const node = screen.getByTestId('flow-node');
    expect(node.className).not.toContain('opacity-50');
    expect(node.textContent).not.toContain('skipped');
    expect(node.textContent).not.toContain('stale');
  });

  it('attempt 缺号兜底形态：record.attempt=0 → 按兜底值呈现；record.attempt=null → 「attempt —」占位', () => {
    const zero = renderNode(evalNode(attempt({ attempt: 0 })));
    expect(screen.getByTestId('flow-node').textContent).toContain('attempt 0');
    zero.unmount();

    const missing = renderNode(evalNode(attempt({ attempt: null })));
    expect(screen.getByTestId('flow-node').textContent).toContain('attempt —');
    missing.unmount();
  });
});

describe('FlowEventNode：interrupted / active 节点', () => {
  it('interrupted → dashed 灰显标记 + startAt / endAt 文案', () => {
    renderNode(interruptedNode());
    const node = screen.getByTestId('flow-node');
    expect(node.className).toContain('border-dashed');
    expect(node.className).toContain('text-muted-foreground');
    expect(node.textContent).toContain('中断留档 · attempt 2');
    expect(node.textContent).toContain('start: 2026-09-03T20:00:00Z');
    expect(node.textContent).toContain('end: 2026-09-04T08:00:00Z');
  });

  it('active → pulse 运行中标记且无 verdict 徽标', () => {
    renderNode(activeNode());
    const node = screen.getByTestId('flow-node');
    expect(node.className).toContain('animate-pulse');
    expect(node.textContent).toContain('运行中 · attempt 2');
    expect(within(node).queryByTestId('attempt-verdict')).toBeNull();
  });

  it('interrupted startAt 与 endAt 双 null → 两侧均「—」占位', () => {
    renderNode(interruptedNode({ startAt: null, endAt: null }));
    const node = screen.getByTestId('flow-node');
    expect(node.textContent).toContain('start: —');
    expect(node.textContent).toContain('end: —');
    expect(node.textContent).not.toContain('null');
  });

  it('以最小 NodeProps 形态直渲染三种 kind（interrupted / active 无 record 字段）→ 各自取用独有字段不抛错', () => {
    const evalRender = renderNode(evalNode(attempt()));
    expect(evalRender.container.querySelector('[data-testid="flow-node"]') !== null).toBe(true);
    evalRender.unmount();

    const activeRender = renderNode(activeNode());
    expect('record' in activeNode()).toBe(false);
    expect(screen.getByTestId('flow-node').textContent).toContain('运行中');
    activeRender.unmount();

    const interruptedRender = renderNode(interruptedNode());
    expect('record' in interruptedNode()).toBe(false);
    expect(screen.getByTestId('flow-node').textContent).toContain('中断留档');
    interruptedRender.unmount();
  });
});
