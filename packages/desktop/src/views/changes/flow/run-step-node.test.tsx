import { render, screen, within } from '@testing-library/react';
import { ReactFlowProvider } from '@xyflow/react';
import type { ComponentProps } from 'react';
import { describe, expect, it } from 'vite-plus/test';

import type { ChangeStepKind } from '../../../types/dto';
import { RunStepNode, type RunStepFlowNode } from './run-step-node';
import type { RuntimeFlowNode } from './types';

// ---------------------------------------------------------------------------
// RunStepNode 单测：自定义节点为纯渲染组件，以最小 NodeProps 形态（data 注入
// RuntimeFlowNode 载荷）直接 render；不挂载 ReactFlow 本体。Handle 依赖
// ReactFlow store context，故外包 ReactFlowProvider（沿 flow-event-node 先例）。
// 三分类徽章 / pulse 运行态 / 失败红态 / 静态终态 / 载荷缺省形态（AC-5 可辨半边）。
// ---------------------------------------------------------------------------

/** 宽松默认值构造运行步节点载荷（run-state.ts makeNode 投影同形）。 */
function runtimeNode(overrides: Partial<RuntimeFlowNode> = {}): RuntimeFlowNode {
  return {
    id: 'run:implement:1:executor',
    kind: 'runtime',
    phase: 'implement',
    attempt: 1,
    colIndex: 3,
    order: 0,
    parentId: 'col:implement',
    runStepKind: 'executor',
    group: 'workerAgent',
    role: 'executor',
    status: 'running',
    sessionId: 'ses-exec-1',
    detail: null,
    ...overrides,
  };
}

/** 九步词汇 → 分组徽章 / 步文案 / role 三面期望表（与 run-state.ts 分组同源）。 */
const FAMILIES: Array<{
  runStepKind: ChangeStepKind;
  group: RuntimeFlowNode['group'];
  badge: string;
  label: string;
  role: RuntimeFlowNode['role'];
}> = [
  {
    runStepKind: 'executor',
    group: 'workerAgent',
    badge: 'WorkerAgent',
    label: '执行',
    role: 'executor',
  },
  {
    runStepKind: 'evaluator',
    group: 'workerAgent',
    badge: 'WorkerAgent',
    label: '评估',
    role: 'evaluator',
  },
  {
    runStepKind: 'decision',
    group: 'workerAgent',
    badge: 'WorkerAgent',
    label: '决策',
    role: 'decision',
  },
  { runStepKind: 'phaseStart', group: 'toolStep', badge: 'ToolStep', label: '开相位', role: null },
  {
    runStepKind: 'staticCheck',
    group: 'toolStep',
    badge: 'ToolStep',
    label: '静态检查',
    role: null,
  },
  { runStepKind: 'phaseLog', group: 'toolStep', badge: 'ToolStep', label: '评估落账', role: null },
  { runStepKind: 'verdictGate', group: 'gate', badge: 'Gate', label: 'verdict 门', role: null },
  { runStepKind: 'retryGate', group: 'gate', badge: 'Gate', label: '重试门', role: null },
  { runStepKind: 'whitelistGate', group: 'gate', badge: 'Gate', label: '白名单门', role: null },
];

/** 最小 NodeProps 形态：仅注入 data 载荷，其余字段非本组件消费面。 */
function renderNode(node: RuntimeFlowNode) {
  const props = { data: { node } } as unknown as ComponentProps<typeof RunStepNode>;
  return render(
    <ReactFlowProvider>
      <RunStepNode {...props} />
    </ReactFlowProvider>,
  );
}

describe('RunStepNode：三分类徽章可辨（AC-5 可辨半边）', () => {
  it('九步词汇逐词渲染：WorkerAgent / ToolStep / Gate 徽章文案、步文案、data-run-group 标记逐族对应', () => {
    for (const family of FAMILIES) {
      const rendered = renderNode(
        runtimeNode({
          id: `run:implement:1:${family.runStepKind}`,
          runStepKind: family.runStepKind,
          group: family.group,
          role: family.role,
        }),
      );
      const node = screen.getByTestId('run-step-node');
      expect(node.getAttribute('data-run-group')).toBe(family.group);
      expect(within(node).getByTestId('run-step-group').textContent).toBe(family.badge);
      expect(within(node).getByTestId('run-step-kind').textContent).toBe(family.label);
      rendered.unmount();
    }
  });

  it('WorkerAgent 徽章走 active 变体（bg-primary），ToolStep / Gate 徽章走 inv2 变体（blue 调）', () => {
    const worker = renderNode(runtimeNode());
    expect(
      within(screen.getByTestId('run-step-node')).getByTestId('run-step-group').className,
    ).toContain('bg-primary');
    worker.unmount();

    const tool = renderNode(
      runtimeNode({
        id: 'run:implement:1:staticCheck',
        runStepKind: 'staticCheck',
        group: 'toolStep',
        role: null,
      }),
    );
    expect(
      within(screen.getByTestId('run-step-node')).getByTestId('run-step-group').className,
    ).toContain('bg-blue-500/15');
    tool.unmount();

    const gate = renderNode(
      runtimeNode({
        id: 'run:implement:1:verdictGate',
        runStepKind: 'verdictGate',
        group: 'gate',
        role: null,
      }),
    );
    expect(
      within(screen.getByTestId('run-step-node')).getByTestId('run-step-group').className,
    ).toContain('bg-blue-500/15');
    gate.unmount();
  });

  it('RunStepFlowNode 类型映射锚定：Node<RunStepNodeData, runStep> 形态（type=runStep + data.node 载荷）', () => {
    const flowNode: RunStepFlowNode = {
      id: 'run:implement:1:executor',
      type: 'runStep',
      position: { x: 0, y: 0 },
      parentId: 'col:implement',
      data: { node: runtimeNode() },
    };
    expect(flowNode.type).toBe('runStep');
    expect(flowNode.data.node.id).toBe('run:implement:1:executor');
  });
});

describe('RunStepNode：状态视觉（pulse 运行态 / 失败红态 / 静态终态）', () => {
  it('status running → pulse 运行态呈现（animate-pulse + 运行中缀语 + data-run-status 标记）', () => {
    renderNode(runtimeNode({ status: 'running' }));
    const node = screen.getByTestId('run-step-node');
    expect(node.getAttribute('data-run-status')).toBe('running');
    expect(node.className).toContain('animate-pulse');
    expect(node.textContent).toContain('运行中');
  });

  it('status failed → 失败红态呈现（fail 边底 + 失败缀语；static-check 挂了与 executor 挂同等可见）', () => {
    renderNode(
      runtimeNode({
        id: 'run:implement:1:staticCheck',
        runStepKind: 'staticCheck',
        group: 'toolStep',
        role: null,
        status: 'failed',
        detail: '诊断：unused export',
      }),
    );
    const node = screen.getByTestId('run-step-node');
    expect(node.getAttribute('data-run-status')).toBe('failed');
    expect(node.className).toContain('border-fail');
    expect(node.className).toContain('bg-fail-bg');
    expect(node.textContent).toContain('失败');
    expect(screen.getByTestId('run-step-detail').textContent).toContain('诊断：unused export');
  });

  it('status passed → 常态样式收敛：无 pulse、无红态、无状态缀语（运行态不残留）', () => {
    renderNode(runtimeNode({ status: 'passed' }));
    const node = screen.getByTestId('run-step-node');
    expect(node.getAttribute('data-run-status')).toBe('passed');
    expect(node.className).not.toContain('animate-pulse');
    expect(node.className).not.toContain('border-fail');
    expect(node.className).not.toContain('border-dashed');
    expect(node.textContent).not.toContain('运行中');
    expect(node.textContent).not.toContain('失败');
    expect(node.textContent).not.toContain('已停');
  });

  it('status stopped → 灰态虚线收敛 + 已停缀语，无 pulse 无红态', () => {
    renderNode(runtimeNode({ status: 'stopped' }));
    const node = screen.getByTestId('run-step-node');
    expect(node.getAttribute('data-run-status')).toBe('stopped');
    expect(node.className).toContain('border-dashed');
    expect(node.className).toContain('text-muted-foreground');
    expect(node.textContent).toContain('已停');
    expect(node.className).not.toContain('animate-pulse');
    expect(node.className).not.toContain('border-fail');
  });
});

describe('RunStepNode：载荷缺省形态（边界）', () => {
  it('sessionId / role 缺省（null）：渲染不炸、徽章仍可辨、attempt 照常呈现', () => {
    renderNode(
      runtimeNode({ runStepKind: 'phaseStart', group: 'toolStep', role: null, sessionId: null }),
    );
    const node = screen.getByTestId('run-step-node');
    expect(within(node).getByTestId('run-step-group').textContent).toBe('ToolStep');
    expect(node.textContent).toContain('attempt 1');
  });

  it('detail 缺省（null）不渲染详情行；detail 有值随行可读', () => {
    const withoutDetail = renderNode(runtimeNode({ detail: null }));
    expect(screen.queryByTestId('run-step-detail')).toBeNull();
    withoutDetail.unmount();

    renderNode(runtimeNode({ detail: '评估通过：verdict pass' }));
    expect(screen.getByTestId('run-step-detail').textContent).toBe('评估通过：verdict pass');
  });
});
