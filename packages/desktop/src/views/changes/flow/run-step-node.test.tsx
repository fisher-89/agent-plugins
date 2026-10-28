import { act, render, screen, within } from '@testing-library/react';
import { ReactFlowProvider } from '@xyflow/react';
import type { ComponentProps } from 'react';
import { afterEach, describe, expect, it, vi } from 'vite-plus/test';

import type { ChangeStepKind } from '../../../types/dto';
import { RunStepNode, type RunStepFlowNode } from './run-step-node';
import type { RuntimeFlowNode } from './types';

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
    runId: 'run-1727',
    runStepKind: 'executor',
    group: 'workerAgent',
    role: 'executor',
    instanceNo: 0,
    status: 'running',
    sessionId: 'ses-exec-1',
    detail: null,
    outputRef: null,
    startedAt: 1_727_000_000_000,
    endedAt: null,
    tokenInput: null,
    tokenOutput: null,
    live: true,
    ...overrides,
  };
}

/** 二十一步词汇 → 分组徽章 / 步文案 / role 三面期望表（与 run-state.ts 分组同源；
   含归档 9 步词、git 六步（cleanup 尽力步与 clean 基线恢复步）的 Git 徽章面与拆步新词 mutationTest、
   archi-check 门禁步 archiCheck）。 */
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
    badge: 'Agent',
    label: '执行',
    role: 'executor',
  },
  {
    runStepKind: 'evaluator',
    group: 'workerAgent',
    badge: 'Agent',
    label: '评估',
    role: 'evaluator',
  },
  {
    runStepKind: 'decision',
    group: 'workerAgent',
    badge: 'Agent',
    label: '决策',
    role: 'decision',
  },
  {
    runStepKind: 'phaseStart',
    group: 'toolStep',
    badge: 'Tool',
    label: '开启阶段',
    role: null,
  },
  {
    runStepKind: 'staticCheck',
    group: 'toolStep',
    badge: 'Tool',
    label: '静态检查',
    role: null,
  },
  {
    runStepKind: 'testExecution',
    group: 'toolStep',
    badge: 'Tool',
    label: '测试执行',
    role: null,
  },
  {
    runStepKind: 'mutationTest',
    group: 'toolStep',
    badge: 'Tool',
    label: '突变执行',
    role: null,
  },
  {
    runStepKind: 'archiCheck',
    group: 'toolStep',
    badge: 'Tool',
    label: '架构检查',
    role: null,
  },
  { runStepKind: 'phaseLog', group: 'toolStep', badge: 'Tool', label: '评估落账', role: null },
  { runStepKind: 'verdictGate', group: 'gate', badge: 'Gate', label: 'verdict 门', role: null },
  { runStepKind: 'retryGate', group: 'gate', badge: 'Gate', label: '重试门', role: null },
  { runStepKind: 'whitelistGate', group: 'gate', badge: 'Gate', label: '白名单门', role: null },
  // 归档 9 步词（AC-2 / AC-9）：git 五步走 Git 徽章、内联两步 Tool、两会话步
  // Agent（group 与 run-state 分组同源，非 git）；clean（基线恢复）单列其后
  { runStepKind: 'preflight', group: 'toolStep', badge: 'Tool', label: '归档前置', role: null },
  { runStepKind: 'commit', group: 'git', badge: 'Git', label: 'worktree 提交', role: null },
  { runStepKind: 'rebase', group: 'git', badge: 'Git', label: '分支重放', role: null },
  {
    runStepKind: 'resolveConflicts',
    group: 'workerAgent',
    badge: 'Agent',
    label: '解冲突',
    role: 'resolve_conflicts',
  },
  { runStepKind: 'merge', group: 'git', badge: 'Git', label: '主仓合入', role: null },
  {
    runStepKind: 'specSync',
    group: 'workerAgent',
    badge: 'Agent',
    label: 'spec 同步',
    role: 'spec_sync',
  },
  { runStepKind: 'cleanup', group: 'git', badge: 'Git', label: 'worktree 清理', role: null },
  { runStepKind: 'clean', group: 'git', badge: 'Git', label: '基线恢复', role: null },
  { runStepKind: 'seal', group: 'toolStep', badge: 'Tool', label: '归档收口', role: null },
  { runStepKind: 'finalize', group: 'git', badge: 'Git', label: '归档落盘', role: null },
];

/** 最小 NodeProps 形态 */
function renderNode(node: RuntimeFlowNode) {
  const props = { data: { node } } as unknown as ComponentProps<typeof RunStepNode>;
  return render(
    <ReactFlowProvider>
      <RunStepNode {...props} />
    </ReactFlowProvider>,
  );
}

describe('RunStepNode：三分类徽章可辨（AC-5 可辨半边）', () => {
  it('二十二步词汇逐词渲染：Agent / Tool / Git / Gate 徽章文案、步文案、data-run-group 标记逐族对应', () => {
    expect(FAMILIES).toHaveLength(22);
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

  it('Agent 徽章走 default 变体（bg-primary），Tool / Gate 徽章走 secondary 变体', () => {
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
    ).toContain('bg-secondary');
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
    ).toContain('bg-secondary');
    gate.unmount();
  });

  it('git 徽章变体：archive 相位 git 步（commit / rebase / merge / cleanup / finalize）与 initialize 基线恢复步（clean）渲染 Git 徽章与 secondary 变体', () => {
    for (const kind of [
      'commit',
      'rebase',
      'merge',
      'cleanup',
      'finalize',
      'clean',
    ] as ChangeStepKind[]) {
      const rendered = renderNode(
        runtimeNode({
          id: `run:archive:1:${kind}`,
          phase: 'archive',
          runStepKind: kind,
          group: 'git',
          role: null,
        }),
      );
      const node = screen.getByTestId('run-step-node');
      expect(node.getAttribute('data-run-group')).toBe('git');
      const badge = within(node).getByTestId('run-step-group');
      expect(badge.textContent).toBe('Git');
      expect(badge.className).toContain('bg-secondary');
      rendered.unmount();
    }
  });

  it('STEP_LABEL 封闭联合穷举：Record<ChangeStepKind, string> 键集恰等二十二词零缺键（21 → 22 扩位——运行时穷举兜底断言）', () => {
    // 二十二值全词汇键集（bindings RunStepKind 联合——缺键即编译期 Record 报错、
    // 运行期穷举兜底防词面漂移）
    const allKinds: ChangeStepKind[] = [
      'executor',
      'evaluator',
      'decision',
      'phaseStart',
      'staticCheck',
      'testExecution',
      'mutationTest',
      'archiCheck',
      'clean',
      'phaseLog',
      'verdictGate',
      'retryGate',
      'whitelistGate',
      'preflight',
      'commit',
      'rebase',
      'resolveConflicts',
      'merge',
      'specSync',
      'cleanup',
      'seal',
      'finalize',
    ];
    for (const kind of allKinds) {
      const rendered = renderNode(
        runtimeNode({
          id: `run:p:1:${kind}`,
          phase: 'initialize',
          runStepKind: kind,
          group: 'toolStep',
          role: null,
        }),
      );
      const label = screen.getByTestId('run-step-kind').textContent;
      expect(label?.length ?? 0).toBeGreaterThan(0);
      rendered.unmount();
    }
  });

  it('clean 基线恢复词条出线（正向）：STEP_LABEL 新键值对拍——「基线恢复」渲染（run-step-kind 锚）', () => {
    const rendered = renderNode(
      runtimeNode({
        id: 'run:init:1:clean',
        phase: 'initialize',
        runStepKind: 'clean',
        group: 'git',
        role: null,
      }),
    );
    expect(screen.getByTestId('run-step-kind').textContent).toBe('基线恢复');
    rendered.unmount();
  });

  it('归档步词文案逐词在案：STEP_LABEL 二十一值穷举可查（9 归档步词各携人读文案）', () => {
    const expected: Array<[ChangeStepKind, string, RuntimeFlowNode['group']]> = [
      ['preflight', '归档前置', 'toolStep'],
      ['commit', 'worktree 提交', 'git'],
      ['rebase', '分支重放', 'git'],
      ['resolveConflicts', '解冲突', 'workerAgent'],
      ['merge', '主仓合入', 'git'],
      ['specSync', 'spec 同步', 'workerAgent'],
      ['cleanup', 'worktree 清理', 'git'],
      ['seal', '归档收口', 'toolStep'],
      ['finalize', '归档落盘', 'git'],
    ];

    for (const [kind, label, group] of expected) {
      const rendered = renderNode(
        runtimeNode({
          id: `run:archive:1:${kind}`,
          phase: 'archive',
          runStepKind: kind,
          group,
          role: group === 'workerAgent' ? (kind as RuntimeFlowNode['role']) : null,
        }),
      );
      expect(
        within(screen.getByTestId('run-step-node')).getByTestId('run-step-kind').textContent,
      ).toBe(label);
      rendered.unmount();
    }
  });

  it('既有三分类徽章回归：workerAgent / toolStep / gate 分组标记不变，徽章文案呈 Agent / Tool / Gate', () => {
    const cases: Array<
      [ChangeStepKind, ChangeStepKind extends never ? never : RuntimeFlowNode['group'], string]
    > = [
      ['executor', 'workerAgent', 'Agent'],
      ['staticCheck', 'toolStep', 'Tool'],
      ['testExecution', 'toolStep', 'Tool'],
      ['verdictGate', 'gate', 'Gate'],
    ];

    for (const [kind, group, badge] of cases) {
      const rendered = renderNode(
        runtimeNode({ id: `run:implement:1:${kind}`, runStepKind: kind, group, role: null }),
      );
      const node = screen.getByTestId('run-step-node');
      expect(within(node).getByTestId('run-step-group').textContent).toBe(badge);
      expect(node.getAttribute('data-run-group')).toBe(group);
      rendered.unmount();
    }
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
    // 常态底样式在位（default 臂正向可辨，非空样式退化）
    expect(node.className).toContain('border-border');
    expect(node.className).toContain('bg-card');
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
    expect(within(node).getByTestId('run-step-group').textContent).toBe('Tool');
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

// ---------------------------------------------------------------------------
// RunStepNodeData 收敛 `{ node }`（desktop-drawer-session-column，AC-4）：
// `onOpenSession` prop 与「查看会话」按钮块删除——data 无多余键，渲染输出
// 不含 button 元素（workerAgent 在场亦然）；点击节点本体打开抽屉入口回归
// change-flow-graph 的 onNodeClick 承接
// ---------------------------------------------------------------------------

describe('RunStepNode：RunStepNodeData 收敛 { node }（AC-4）', () => {
  it('workerAgent 节点渲染不炸且输出不含 button 元素（「查看会话」按钮块删除的组件级锚）', () => {
    const { container } = renderNode(runtimeNode());

    const node = screen.getByTestId('run-step-node');
    expect(node.querySelector('button')).toBeNull();
    expect(container.querySelector('button')).toBeNull();
    expect(screen.queryByTestId('view-session')).toBeNull();
    // 收敛后渲染面零回归：分组徽章 / 步词汇照常可辨
    expect(within(node).getByTestId('run-step-group').textContent).toBe('Agent');
    expect(within(node).getByTestId('run-step-kind').textContent).toBe('执行');
  });

  it('三组各抽一步穷举：data 收敛后 toolStep / gate 输出同样不含 button 元素与 view-session 入口', () => {
    for (const overrides of [
      {
        id: 'run:implement:1:staticCheck',
        runStepKind: 'staticCheck' as const,
        group: 'toolStep' as const,
        role: null,
      },
      {
        id: 'run:implement:1:verdictGate',
        runStepKind: 'verdictGate' as const,
        group: 'gate' as const,
        role: null,
      },
    ]) {
      const rendered = renderNode(runtimeNode(overrides));
      const node = screen.getByTestId('run-step-node');
      expect(node.querySelector('button')).toBeNull();
      expect(screen.queryByTestId('view-session')).toBeNull();
      rendered.unmount();
    }
  });
});

// ---------------------------------------------------------------------------
// mutationTest 步节点（desktop-fix-mutation-test / AC-2）：STEP_LABEL 新键 +
// 渲染零改动语义（现有步骤渲染直接吃）+ 全词汇键集恰等
// ---------------------------------------------------------------------------

describe('RunStepNode：archiCheck 门禁步节点（AC-16）', () => {
  it('应把 archiCheck 渲染为「架构检查」并归 Tool 徽章（分组零新增）', () => {
    const rendered = renderNode(
      runtimeNode({
        id: 'run:implement:1:archiCheck:0',
        runStepKind: 'archiCheck',
        group: 'toolStep',
        role: null,
        sessionId: null,
        detail: '未检查（无模型）',
      }),
    );
    const node = screen.getByTestId('run-step-node');
    expect(node.getAttribute('data-run-group')).toBe('toolStep');
    expect(within(node).getByTestId('run-step-group').textContent).toBe('Tool');
    expect(within(node).getByTestId('run-step-kind').textContent).toBe('架构检查');
    rendered.unmount();
  });

  it.each([
    ['未检查（无模型）', '未检查（无模型）'],
    ['未检查（无可判文件）', '未检查（无可判文件）'],
  ])('应逐字上屏 detail = %s（不追加「通过」、不截断）', (detail, expected) => {
    const rendered = renderNode(
      runtimeNode({
        id: 'run:implement:1:archiCheck:0',
        runStepKind: 'archiCheck',
        group: 'toolStep',
        role: null,
        sessionId: null,
        detail,
        status: 'passed',
        endedAt: 1_727_000_000_500,
      }),
    );
    expect(screen.getByTestId('run-step-detail').textContent).toBe(expected);
    rendered.unmount();
  });

  it('应在超长 detail（>1000 chars）时渲染不崩且文案不改写', () => {
    const longDetail = `架构漂移：${'长'.repeat(1200)}`;
    const rendered = renderNode(
      runtimeNode({
        id: 'run:implement:1:archiCheck:0',
        runStepKind: 'archiCheck',
        group: 'toolStep',
        role: null,
        sessionId: null,
        detail: longDetail,
      }),
    );
    expect(screen.getByTestId('run-step-detail').textContent).toBe(longDetail);
    rendered.unmount();
  });

  it('应在 detail 为 null 时无详情区且不崩', () => {
    const rendered = renderNode(
      runtimeNode({
        id: 'run:implement:1:archiCheck:0',
        runStepKind: 'archiCheck',
        group: 'toolStep',
        role: null,
        sessionId: null,
        detail: null,
      }),
    );
    expect(screen.queryByTestId('run-step-detail')).toBeNull();
    rendered.unmount();
  });

  it.each(['archi_check', '', 'unknownKind'])(
    '应在越集 runStepKind = %s 时不崩且不上屏 undefined',
    (kind) => {
      const rendered = renderNode(
        runtimeNode({
          id: 'run:implement:1:archiCheck:0',
          runStepKind: kind as ChangeStepKind,
          group: 'toolStep',
          role: null,
        }),
      );
      const label = screen.getByTestId('run-step-kind').textContent ?? '';
      expect(label).not.toBe('undefined');
      rendered.unmount();
    },
  );
});

describe('RunStepNode：mutationTest 步节点（AC-2）', () => {
  it('STEP_LABEL 键面：mutationTest 渲染「突变执行」文案（新键值断言）', () => {
    const rendered = renderNode(
      runtimeNode({
        id: 'run:test-execution:1:mutationTest',
        phase: 'test-execution',
        runStepKind: 'mutationTest',
        group: 'toolStep',
        role: null,
      }),
    );
    const node = screen.getByTestId('run-step-node');
    expect(within(node).getByTestId('run-step-kind').textContent).toBe('突变执行');
    // 归 exec 组列（Tool 徽章——零渲染改动语义：经 groupOf default 落 toolStep）
    expect(node.getAttribute('data-run-group')).toBe('toolStep');
    expect(within(node).getByTestId('run-step-group').textContent).toBe('Tool');
    rendered.unmount();
  });

  it('mutationTest 全状态渲染不炸（running / passed / failed / stopped 四态视觉面持衡）', () => {
    for (const status of ['running', 'passed', 'failed', 'stopped'] as const) {
      const rendered = renderNode(
        runtimeNode({
          id: `run:test-execution:1:mutationTest:${status}`,
          phase: 'test-execution',
          runStepKind: 'mutationTest',
          group: 'toolStep',
          role: null,
          status,
          detail: status === 'failed' ? '突变测试未达标：score 55 < threshold 80' : null,
        }),
      );
      const node = screen.getByTestId('run-step-node');
      expect(node.getAttribute('data-run-status')).toBe(status);
      expect(within(node).getByTestId('run-step-kind').textContent).toBe('突变执行');
      if (status === 'failed') {
        // 失败红态视觉面持衡
        expect(node.className).toContain('border-fail');
        expect(screen.getByTestId('run-step-detail').textContent).toContain('score 55');
      }
      rendered.unmount();
    }
  });

  it('STEP_LABEL 键集恰等：二十二词全词汇逐词渲染零缺键（编译期穷尽表的运行时兜底）', () => {
    const ALL_KINDS: ChangeStepKind[] = [
      'executor',
      'evaluator',
      'decision',
      'phaseStart',
      'staticCheck',
      'testExecution',
      'mutationTest',
      'archiCheck',
      'clean',
      'phaseLog',
      'verdictGate',
      'retryGate',
      'whitelistGate',
      'preflight',
      'commit',
      'rebase',
      'resolveConflicts',
      'merge',
      'specSync',
      'cleanup',
      'seal',
      'finalize',
    ];
    expect(ALL_KINDS).toHaveLength(22);
    for (const kind of ALL_KINDS) {
      const rendered = renderNode(
        runtimeNode({
          id: `run:implement:1:${kind}:keys`,
          runStepKind: kind,
          role: null,
        }),
      );
      const label = within(screen.getByTestId('run-step-node')).getByTestId(
        'run-step-kind',
      ).textContent;
      // STEP_LABEL[kind] 键应在案（非空文案）
      expect(label).toBeTruthy();
      expect(label.length).toBeGreaterThan(0);
      rendered.unmount();
    }
  });
});

describe('RunStepNode：mutationTest 渲染回归（异常载荷）', () => {
  it('非法 / 缺失 detail 载荷下 mutationTest 节点渲染不炸图（既有渲染边界持衡）', () => {
    // detail 缺省（null）→ 零详情行
    const withoutDetail = renderNode(
      runtimeNode({
        id: 'run:test-execution:1:mutationTest:no-detail',
        phase: 'test-execution',
        runStepKind: 'mutationTest',
        group: 'toolStep',
        role: null,
        detail: null,
      }),
    );
    expect(screen.queryByTestId('run-step-detail')).toBeNull();
    withoutDetail.unmount();

    // 超长 detail（feedback_text 摘要面直挂）→ 渲染不炸、原文可读
    const longDetail = `突变测试未达标：score 55 < threshold 80 ${'报告路径 reports/test/app_rust/mutation/mutation-seg-0.json '.repeat(50)}`;
    const withDetail = renderNode(
      runtimeNode({
        id: 'run:test-execution:1:mutationTest:long-detail',
        phase: 'test-execution',
        runStepKind: 'mutationTest',
        group: 'toolStep',
        role: null,
        status: 'failed',
        detail: longDetail,
      }),
    );
    expect(screen.getByTestId('run-step-detail').textContent).toBe(longDetail);
    withDetail.unmount();

    // sessionId 缺省（null——确定性工具步恒无会话）+ 非法 status cast → 不炸
    const broken = runtimeNode({
      id: 'run:test-execution:1:mutationTest:broken',
      phase: 'test-execution',
      runStepKind: 'mutationTest',
      group: 'toolStep',
      role: null,
      sessionId: null,
      status: 'weirdStatus' as RuntimeFlowNode['status'],
    });
    const rendered = renderNode(broken);
    expect(screen.getByTestId('run-step-node')).toBeTruthy();
    rendered.unmount();
  });
});

// ---------------------------------------------------------------------------
// cleanup 步节点（worktree-cleanup-and-active-cap / AC-9）：STEP_LABEL 新键 +
// git 组徽章 + detail 载荷降级面持衡
// ---------------------------------------------------------------------------

describe('RunStepNode：cleanup 步节点（worktree-cleanup-and-active-cap）', () => {
  it('STEP_LABEL 新键：cleanup 渲染「worktree 清理」词条（data-testid="run-step-kind" 出线断言）', () => {
    const rendered = renderNode(
      runtimeNode({
        id: 'run:archive:1:cleanup',
        phase: 'archive',
        runStepKind: 'cleanup',
        group: 'git',
        role: null,
      }),
    );
    const node = screen.getByTestId('run-step-node');
    expect(within(node).getByTestId('run-step-kind').textContent).toBe('worktree 清理');
    // git 组徽章与 commit / rebase / merge / finalize 同列可辨（AC-9 观测半边）
    expect(node.getAttribute('data-run-group')).toBe('git');
    const badge = within(node).getByTestId('run-step-group');
    expect(badge.textContent).toBe('Git');
    expect(badge.className).toContain('bg-secondary');
    rendered.unmount();
  });

  it('cleanup 节点 detail 载荷两态：警告摘要随行可读、载荷缺省（detail=null / sessionId=null）降级渲染不炸', () => {
    // detail 有值（清理失败警告形态——尽力步 detail 面）
    const withDetail = renderNode(
      runtimeNode({
        id: 'run:archive:1:cleanup:warn',
        phase: 'archive',
        runStepKind: 'cleanup',
        group: 'git',
        role: null,
        status: 'passed',
        detail: 'worktree 清理失败（尽力步，不影响归档收口）: git worktree remove --force 清理',
      }),
    );
    expect(screen.getByTestId('run-step-detail').textContent).toContain('worktree 清理失败');
    withDetail.unmount();

    // 载荷缺省形态（detail=null / sessionId=null）——扩键不改变缺省面
    const withoutDetail = renderNode(
      runtimeNode({
        id: 'run:archive:1:cleanup:bare',
        phase: 'archive',
        runStepKind: 'cleanup',
        group: 'git',
        role: null,
        status: 'passed',
        detail: null,
        sessionId: null,
      }),
    );
    expect(screen.queryByTestId('run-step-detail')).toBeNull();
    expect(
      within(screen.getByTestId('run-step-node')).getByTestId('run-step-kind').textContent,
    ).toBe('worktree 清理');
    withoutDetail.unmount();
  });

  it('cleanup 四态视觉面持衡：running / passed / failed / stopped 渲染零崩溃（状态机照常吃）', () => {
    for (const status of ['running', 'passed', 'failed', 'stopped'] as const) {
      const rendered = renderNode(
        runtimeNode({
          id: `run:archive:1:cleanup:${status}`,
          phase: 'archive',
          runStepKind: 'cleanup',
          group: 'git',
          role: null,
          status,
        }),
      );
      expect(screen.getByTestId('run-step-node').getAttribute('data-run-status')).toBe(status);
      rendered.unmount();
    }
  });
});

// ---------------------------------------------------------------------------
// desktop-step-running-snapshot（AC-7 / AC-8 / D10）：标题下常驻度量行
//（静态耗时 + workerAgent token 段）+ 活面 running 走秒（组件内 interval
// 仅 live && running 挂载，翻态 / 卸载即清）。metrics.ts 真件组合零 mock；
// 时钟 / interval 以 vi.useFakeTimers() 伪造（含 Date，沿 use-change-detail
// 既有先例）。
// ---------------------------------------------------------------------------

afterEach(() => {
  vi.useRealTimers();
});

/** 已收口 executor 步载荷（起止与 token 定值——D10 词面断言的事实源）。 */
function settledWorkerNode(overrides: Partial<RuntimeFlowNode> = {}): RuntimeFlowNode {
  return runtimeNode({
    status: 'passed',
    startedAt: 1_727_000_000_000,
    endedAt: 1_727_000_200_000,
    tokenInput: 12_000,
    tokenOutput: 3_400,
    live: false,
    ...overrides,
  });
}

describe('RunStepNode：常驻度量行静态双段（AC-7 / D10 词面）', () => {
  it('已收口 workerAgent 步 → run-step-metrics 行在场：⏱ 前缀 + 分级串、tokens 12k/3.4k、两段以 " ▸ " 分隔', () => {
    renderNode(settledWorkerNode());
    const metrics = within(screen.getByTestId('run-step-node')).getByTestId('run-step-metrics');
    const duration = within(metrics).getByTestId('run-step-duration');
    const tokens = within(metrics).getByTestId('run-step-tokens');
    // 耗时段：⏱ 前缀 + 分级串（200_000ms → 3m20s）
    expect(duration.textContent).toBe('⏱ 3m20s');
    // token 段：k / 一位小数缩写
    expect(tokens.textContent).toBe('tokens 12k/3.4k');
    // 两段以 " ▸ " 分隔（D10 词面，整行文本逐字）
    expect(metrics.textContent).toBe('⏱ 3m20s ▸ tokens 12k/3.4k');
  });

  it('已收口非 workerAgent 步（toolStep / gate / git）→ token 段不渲染、仅耗时段（无 " ▸ " 分隔符）', () => {
    for (const overrides of [
      {
        id: 'run:implement:1:staticCheck',
        runStepKind: 'staticCheck' as const,
        group: 'toolStep' as const,
        role: null,
      },
      {
        id: 'run:implement:1:verdictGate',
        runStepKind: 'verdictGate' as const,
        group: 'gate' as const,
        role: null,
      },
      {
        id: 'run:archive:1:commit',
        phase: 'archive',
        runStepKind: 'commit' as const,
        group: 'git' as const,
        role: null,
      },
    ]) {
      const rendered = renderNode(settledWorkerNode(overrides));
      const metrics = within(screen.getByTestId('run-step-node')).getByTestId('run-step-metrics');
      expect(within(metrics).getByTestId('run-step-duration').textContent).toBe('⏱ 3m20s');
      expect(screen.queryByTestId('run-step-tokens')).toBeNull();
      expect(metrics.textContent).not.toContain('▸');
      rendered.unmount();
    }
  });

  it('workerAgent 步 token 缺席（null）→ token 段 "tokens —/—" 渲染在场（em dash）', () => {
    renderNode(settledWorkerNode({ tokenInput: null, tokenOutput: null }));
    const tokens = within(screen.getByTestId('run-step-node')).getByTestId('run-step-tokens');
    expect(tokens.textContent).toBe('tokens —/—');
  });

  it('workerAgent 步 token 单侧缺席 → 在场侧缩写、缺席侧 em dash（in/out 各自独立缩写）', () => {
    const rendered = renderNode(settledWorkerNode({ tokenInput: 500, tokenOutput: null }));
    expect(
      within(screen.getByTestId('run-step-node')).getByTestId('run-step-tokens').textContent,
    ).toBe('tokens 500/—');
    rendered.unmount();
    renderNode(settledWorkerNode({ tokenInput: null, tokenOutput: 1_500_000 }));
    expect(
      within(screen.getByTestId('run-step-node')).getByTestId('run-step-tokens').textContent,
    ).toBe('tokens —/1.5M');
  });
});

describe('RunStepNode：常驻度量行边界（0ms 瞬步 / 旧数据）', () => {
  it('0ms 瞬步且非 workerAgent（startedAt==endedAt、无 token 段）→ 整行 run-step-metrics 省显（行条件 ≥ 1 段在场）', () => {
    renderNode(
      runtimeNode({
        runStepKind: 'phaseStart',
        group: 'toolStep',
        role: null,
        status: 'passed',
        startedAt: 1_727_000_000_000,
        endedAt: 1_727_000_000_000,
        live: false,
      }),
    );
    expect(screen.queryByTestId('run-step-metrics')).toBeNull();
    expect(screen.queryByTestId('run-step-duration')).toBeNull();
    expect(screen.queryByTestId('run-step-tokens')).toBeNull();
    // 节点本体照常可辨（仅度量行省显，头行不缺）
    expect(
      within(screen.getByTestId('run-step-node')).getByTestId('run-step-kind').textContent,
    ).toBe('开启阶段');
  });

  it('0ms 瞬步 workerAgent → 行在场、仅 token 段（耗时段 null 省显、无 " ▸ "）', () => {
    renderNode(
      settledWorkerNode({
        startedAt: 1_727_000_000_000,
        endedAt: 1_727_000_000_000,
      }),
    );
    const metrics = within(screen.getByTestId('run-step-node')).getByTestId('run-step-metrics');
    expect(screen.queryByTestId('run-step-duration')).toBeNull();
    expect(within(metrics).getByTestId('run-step-tokens').textContent).toBe('tokens 12k/3.4k');
    expect(metrics.textContent).toBe('tokens 12k/3.4k');
  });

  it('旧数据（startedAt=0 / endedAt=null、非活面 running）→ 耗时段 "-"（hyphen，AC-7 旧库读史形态）', () => {
    renderNode(
      settledWorkerNode({ startedAt: 0, endedAt: null, tokenInput: null, tokenOutput: null }),
    );
    const metrics = within(screen.getByTestId('run-step-node')).getByTestId('run-step-metrics');
    expect(within(metrics).getByTestId('run-step-duration').textContent).toBe('⏱ -');
    // token 缺席形态照常渲染（em dash——与耗时 "-" 词面分立）
    expect(within(metrics).getByTestId('run-step-tokens').textContent).toBe('tokens —/—');
  });
});

describe('RunStepNode：走秒仅活面 running 挂载（AC-8）', () => {
  it('库读史 status=running 残留（live=false）→ 不挂 timer、耗时段静态 "-"', () => {
    vi.useFakeTimers();
    vi.setSystemTime(1_727_000_000_000);
    renderNode(
      runtimeNode({ status: 'running', startedAt: 1_727_000_000_000, endedAt: null, live: false }),
    );
    expect(
      within(screen.getByTestId('run-step-node')).getByTestId('run-step-duration').textContent,
    ).toBe('⏱ -');
  });

  it('live=false && status=running → 不挂 interval（推进 fake timers 文本不变）', () => {
    vi.useFakeTimers();
    vi.setSystemTime(1_727_000_000_000);
    const rendered = renderNode(
      runtimeNode({ status: 'running', startedAt: 1_727_000_000_000, endedAt: null, live: false }),
    );
    act(() => {
      vi.advanceTimersByTime(5_000);
    });
    expect(
      within(screen.getByTestId('run-step-node')).getByTestId('run-step-duration').textContent,
    ).toBe('⏱ -');
    rendered.unmount();
  });

  it('活面走秒：live=true && running，fake timers 逐秒推进 → duration 文本随 now-startedAt 更新', () => {
    vi.useFakeTimers();
    vi.setSystemTime(1_727_000_000_000);
    renderNode(
      runtimeNode({ status: 'running', startedAt: 1_727_000_000_000, endedAt: null, live: true }),
    );
    const duration = () =>
      within(screen.getByTestId('run-step-node')).getByTestId('run-step-duration').textContent;
    // 挂载即对齐当前 tick（0ms → "0.0s"——running 恒显无省显路径）
    expect(duration()).toBe('⏱ 0.0s');
    // 每秒 tick 局部重渲染
    act(() => {
      vi.advanceTimersByTime(1_000);
    });
    expect(duration()).toBe('⏱ 1.0s');
    act(() => {
      vi.advanceTimersByTime(59_000);
    });
    expect(duration()).toBe('⏱ 1m0s');
    act(() => {
      vi.advanceTimersByTime(3_600_000);
    });
    expect(duration()).toBe('⏱ 1h01m');
  });

  it('走秒收口回落：running → passed 翻态 rerender 后 timer 卸载、耗时段回落静态分级串', () => {
    vi.useFakeTimers();
    const started = 1_727_000_000_000;
    vi.setSystemTime(started);
    const rendered = renderNode(
      runtimeNode({ status: 'running', startedAt: started, endedAt: null, live: true }),
    );
    act(() => {
      vi.advanceTimersByTime(2_000);
    });
    expect(
      within(screen.getByTestId('run-step-node')).getByTestId('run-step-duration').textContent,
    ).toBe('⏱ 2.0s');

    // 翻态：passed 收口（endedAt = started + 200_000 定值）→ ticking 条件失效
    const settledProps = {
      data: { node: settledWorkerNode({ startedAt: started, endedAt: started + 200_000 }) },
    } as unknown as ComponentProps<typeof RunStepNode>;
    rendered.rerender(
      <ReactFlowProvider>
        <RunStepNode {...settledProps} />
      </ReactFlowProvider>,
    );
    const duration = () =>
      within(screen.getByTestId('run-step-node')).getByTestId('run-step-duration').textContent;
    expect(duration()).toBe('⏱ 3m20s');
    // timer 已卸载：再推进 fake timers 文本钉死在静态分级串（R1 无泄漏）
    act(() => {
      vi.advanceTimersByTime(10_000);
    });
    expect(duration()).toBe('⏱ 3m20s');
  });

  it('走秒卸载清理：unmount 后推进 fake timers 不炸不泄漏（timer 生命周期绑定组件，R1）', () => {
    vi.useFakeTimers();
    vi.setSystemTime(1_727_000_000_000);
    const rendered = renderNode(
      runtimeNode({ status: 'running', startedAt: 1_727_000_000_000, endedAt: null, live: true }),
    );
    act(() => {
      vi.advanceTimersByTime(1_000);
    });
    rendered.unmount();
    // 卸载后 interval 已清：推进不触发任何已卸载组件状态更新（不炸即锚）
    expect(() => {
      act(() => {
        vi.advanceTimersByTime(10_000);
      });
    }).not.toThrow();
    expect(screen.queryByTestId('run-step-node')).toBeNull();
  });
});

describe('RunStepNode：常驻行与既有锚并存（异常载荷 / D10 排布）', () => {
  it('异常载荷与常驻行并存（超长 detail / 非法 status cast）→ 渲染不炸、既有锚与常驻行同屏可辨', () => {
    const longDetail = `诊断：${'评估未过，反馈明细'.repeat(80)}`;
    renderNode(
      runtimeNode({
        status: 'weirdStatus' as RuntimeFlowNode['status'],
        detail: longDetail,
        startedAt: 1_727_000_000_000,
        endedAt: 1_727_000_200_000,
        live: false,
      }),
    );
    const node = screen.getByTestId('run-step-node');
    // 既有锚四面在案
    expect(within(node).getByTestId('run-step-group').textContent).toBe('Agent');
    expect(within(node).getByTestId('run-step-kind').textContent).toBe('执行');
    expect(within(node).getByTestId('run-step-detail').textContent).toBe(longDetail);
    // 常驻行同屏（非法 status 走 settled 臂静态分级）
    expect(within(node).getByTestId('run-step-metrics').textContent).toContain('⏱ 3m20s');
  });

  it('排布回归：常驻行插于标题头行与 detail 行之间（D10——头行 / 常驻行 / detail 依次相邻）', () => {
    renderNode(settledWorkerNode({ detail: '评估通过：verdict pass' }));
    const node = screen.getByTestId('run-step-node');
    const header = within(node).getByTestId('run-step-kind').parentElement!;
    const metrics = within(node).getByTestId('run-step-metrics');
    const detail = within(node).getByTestId('run-step-detail');
    // DOM 先后序：头行 < 常驻行 < detail（FOLLOWING 位恒成立即序稳定）
    expect(header.compareDocumentPosition(metrics) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(metrics.compareDocumentPosition(detail) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });
});
