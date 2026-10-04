import { render, screen, within } from '@testing-library/react';
import { ReactFlowProvider } from '@xyflow/react';
import type { ComponentProps } from 'react';
import { describe, expect, it } from 'vite-plus/test';

import type { AttemptRecord } from '../../../types/dto';
import { FlowEventNode } from './flow-event-node';
import type { ActiveFlowNode, EvalFlowNode, FlowNode } from './types';

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

describe('FlowEventNode：active 节点', () => {
  it('active → pulse 运行中标记且无 verdict 徽标', () => {
    renderNode(activeNode());
    const node = screen.getByTestId('flow-node');
    expect(node.className).toContain('animate-pulse');
    expect(node.textContent).toContain('运行中 · attempt 2');
    expect(within(node).queryByTestId('attempt-verdict')).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// 两分类收敛（desktop-drawer-session-column，AC-5）：`InterruptedBody` 组件与
// `kind === 'interrupted'` 渲染分支删、`nodeClass` 收敛 eval / active 两分支
// （dashed 回退分支删）——渲染输出恒不含 dashed 类名与「中断留档」词汇
// ---------------------------------------------------------------------------

describe('FlowEventNode：两分类收敛（AC-5）', () => {
  it('eval / active 两 kind 穷举：渲染输出恒不含 dashed 类名与「中断留档」词汇（无路径可产出）', () => {
    const evalRendered = renderNode(evalNode(attempt()));
    let node = screen.getByTestId('flow-node');
    expect(node.className).not.toContain('dashed');
    expect(node.textContent).not.toContain('中断留档');
    evalRendered.unmount();

    const activeRendered = renderNode(activeNode());
    node = screen.getByTestId('flow-node');
    expect(node.className).not.toContain('dashed');
    expect(node.textContent).not.toContain('中断留档');
    activeRendered.unmount();
  });

  it('eval 退化形态（attempt 缺号 + 空 checklist）与 active 无 startAt 形态渲染不炸、无 dashed 回退分支', () => {
    const degraded = renderNode(evalNode(attempt({ attempt: null, checklist: [] })));
    expect(screen.getByTestId('flow-node').textContent).toContain('attempt —');
    degraded.unmount();

    renderNode(activeNode({ startAt: null }));
    const node = screen.getByTestId('flow-node');
    expect(node.className).toContain('animate-pulse');
    expect(node.className).not.toContain('dashed');
    expect(node.textContent).toContain('运行中 · attempt 2');
  });
});
