import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { AgentEvent, AttemptRecord, ChangeDetail, SessionSummary } from '../../../types/dto';
import { mountMaterials } from './attachments';
import { DetailDrawer } from './detail-drawer';
import { buildFlowGraph } from './graph';
import { PIPELINE_PHASES } from './layout';
import type { DrawerSelection, FlowGraph, FlowMaterials, RuntimeFlowNode } from './types';

// ---------------------------------------------------------------------------
// 会话转录联动引入进程边界 mock（desktop-change-flow 增量）：invoke 沿
// use-change-list.test 同款 vi.hoisted + vi.mock('@tauri-apps/api/core') 装置。
// 缺省实现恒返回 Promise（反查空清单 = 合法空态）——eval 选中本就渲染转录
// 面板，既有用例在空态下面板呈「（暂无该会话转录）」，断言面不受影响。
// ---------------------------------------------------------------------------

const { defaultInvoke, invokeMock } = vi.hoisted(() => {
  const defaultInvoke = (command: string): Promise<unknown> => {
    if (command === 'agent_sessions') return Promise.resolve([]);
    if (command === 'agent_session_transcript') return Promise.resolve([]);
    return Promise.resolve(null);
  };
  return { defaultInvoke, invokeMock: vi.fn(defaultInvoke) };
});

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

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
      root="root-a"
      change="test-change"
      liveEvents={[]}
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

  it('本站文档多文档 → ArtifactTabs 切换：tab 按序、默认首项、切换仅渲染当前卡片', () => {
    const state = world({
      materials: {
        columnDocs: {
          'col:dev-design': [
            envelope('markdown-doc', '设计文档', { markdown: '# 设计正文' }),
            envelope('tasks-progress', '任务进度', { total: 4, done: 1, pending: 3 }),
          ],
        },
      },
    });
    const { container } = renderDrawer(state, { scope: 'column', phase: 'dev-design' });

    const docsSection = screen.getByTestId('drawer-docs-section');
    const tabs = within(docsSection).getAllByTestId('artifact-tab');
    expect(tabs.map((tab) => tab.textContent)).toEqual(['设计文档', '任务进度']);
    expect(tabs[0].getAttribute('aria-selected')).toBe('true');
    expect(within(docsSection).getAllByTestId('artifact-card')).toHaveLength(1);
    expect(docsSection.textContent).toContain('设计正文');
    expect(docsSection.textContent).not.toContain('25%');

    // TabsTrigger 激活绑在 mousedown（Radix 1.1 行为），点击事件用 mouseDown 模拟
    fireEvent.mouseDown(tabs[1]);
    expect(within(docsSection).getAllByTestId('artifact-card')).toHaveLength(1);
    expect(docsSection.textContent).toContain('25%');
    expect(docsSection.textContent).not.toContain('设计正文');
    expect(container.textContent).not.toContain('（无本站文档）');
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
        root="root-a"
        change="test-change"
        liveEvents={[]}
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

// ---------------------------------------------------------------------------
// 会话转录联动（desktop-change-flow 增量）：WorkerAgent 运行节点与历史 eval
// 节点增会话转录联动区——role × attempt → sourceRef 定式
// `<change>/<phase>/<role>/<attempt>` 反查 agentSessions（source='change'），
// liveEvents 按选中节点的 sessionId 过滤下传；非会话选中无转录区（既有三分
// 节零回归）。SessionTranscriptPanel 与既有分节组件真实组合，fixture 经
// mock invoke 按命令名分发流入真实 useSessionTranscript。
// ---------------------------------------------------------------------------

/** 运行步节点 fixture（run-state.ts runStepNodes 同形投影的直构形态）。 */
function runtimeNode(overrides: Partial<RuntimeFlowNode> = {}): RuntimeFlowNode {
  return {
    id: 'run:implement:2:executor',
    kind: 'runtime',
    phase: 'implement',
    attempt: 2,
    colIndex: 3,
    order: 0,
    parentId: 'col:implement',
    runStepKind: 'executor',
    group: 'workerAgent',
    role: 'executor',
    status: 'running',
    sessionId: 'ses-exec-2',
    detail: null,
    ...overrides,
  };
}

/** 会话清单项 fixture（agent_sessions 应答；轮行已收敛 = 重放形态）。 */
function transcriptSession(sessionId: string, sourceRef: string): SessionSummary {
  return {
    row: {
      id: sessionId,
      remoteSessionId: null,
      configSnapshot: null,
      provenance: { source: 'change', sourceRef },
      createdAt: 1727000000000,
      updatedAt: 1727000001000,
    },
    stats: { turnCount: 1, totalDurationMs: 1234, inputTokens: null, outputTokens: null },
    turns: [
      {
        turnId: 1,
        sessionId,
        status: 'completed',
        startedAt: 1727000000000,
        finishedAt: 1727000001000,
        numTurns: 1,
        costUsd: 0.5,
        durationMs: 1234,
        error: null,
      },
    ],
  };
}

/** 密封文本消息事件 fixture（message 块 → AgentTimeline block-text 呈现）。 */
function textEvent(seq: number, role: 'user' | 'assistant', text: string): AgentEvent {
  return {
    seq,
    timestampMs: 1727000000000 + seq,
    kind: 'message',
    role,
    blocks: [{ kind: 'text', text }],
    parentToolUseId: null,
  };
}

/** 会话反查 / 转录重放 fixture 注册表（按 sourceRef / sessionId 寻址）。 */
let sessionsFixture: Record<string, SessionSummary[]> = {};
let transcriptFixture: Record<string, AgentEvent[]> = {};

/** 含运行步节点的抽屉输入：真实 buildFlowGraph(detail, runNodes) + mountMaterials。 */
function runtimeWorld(node: RuntimeFlowNode): World {
  const base = detail();
  const graph = buildFlowGraph(base, [node]);
  return { graph, materials: mountMaterials(graph, base, []), hasFileLog: true };
}

/** 直挂 DetailDrawer（自定义 liveEvents；change 名沿用 renderDrawer 的 test-change）。 */
function renderTranscriptDrawer(
  worldState: World,
  selection: DrawerSelection,
  liveEvents: Array<{ sessionId: string; event: AgentEvent }> = [],
) {
  return render(
    <DetailDrawer
      selection={selection}
      graph={worldState.graph}
      materials={worldState.materials}
      hasFileLog={worldState.hasFileLog}
      root="root-a"
      change="test-change"
      liveEvents={liveEvents}
      onClose={() => {}}
    />,
  );
}

describe('DetailDrawer：会话转录联动（role × attempt → sourceRef 反查）', () => {
  beforeEach(() => {
    sessionsFixture = {};
    transcriptFixture = {};
    invokeMock.mockReset();
    invokeMock.mockImplementation(
      (command: string, args: { sourceRef?: string; sessionId?: string } = {}) => {
        if (command === 'agent_sessions') {
          return Promise.resolve(sessionsFixture[args.sourceRef ?? ''] ?? []);
        }
        if (command === 'agent_session_transcript') {
          return Promise.resolve(transcriptFixture[args.sessionId ?? ''] ?? []);
        }
        return Promise.resolve(null);
      },
    );
  });

  afterEach(() => {
    // 恢复缺省安全应答：用例乱序（sequence.shuffle）时既有用例不落 undefined 应答
    invokeMock.mockReset();
    invokeMock.mockImplementation(defaultInvoke);
  });

  it('选中运行 executor 节点 → SessionTranscriptPanel 渲染且反查 sourceRef 按 <change>/<phase>/<role>/<attempt> 定式组装', async () => {
    sessionsFixture = {
      'test-change/implement/executor/2': [
        transcriptSession('ses-exec-2', 'test-change/implement/executor/2'),
      ],
    };
    transcriptFixture = {
      'ses-exec-2': [textEvent(0, 'user', '实现该功能'), textEvent(1, 'assistant', '开始实现')],
    };
    renderTranscriptDrawer(runtimeWorld(runtimeNode()), {
      scope: 'node',
      nodeId: 'run:implement:2:executor',
    });

    const panel = await screen.findByTestId('session-transcript-panel');
    expect(panel.getAttribute('data-role')).toBe('executor');
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('agent_sessions', {
        root: 'root-a',
        source: 'change',
        sourceRef: 'test-change/implement/executor/2',
      }),
    );
    // 反查命中的重放转录经 AgentTimeline 呈现
    await waitFor(() => expect(panel.textContent).toContain('开始实现'));
  });

  it('选中历史 eval 节点 → executor + evaluator 双会话联动：默认 executor 反查、切换 evaluator 重放对应 attempt', async () => {
    sessionsFixture = {
      'test-change/dev-design/executor/2': [
        transcriptSession('ses-exec-2', 'test-change/dev-design/executor/2'),
      ],
      'test-change/dev-design/evaluator/2': [
        transcriptSession('ses-eval-2', 'test-change/dev-design/evaluator/2'),
      ],
    };
    transcriptFixture = {
      'ses-exec-2': [textEvent(0, 'user', '执行会话正文')],
      'ses-eval-2': [textEvent(0, 'assistant', '评估会话正文')],
    };
    renderDrawer(world(), { scope: 'node', nodeId: 'eval:dev-design:2' });

    const panel = await screen.findByTestId('session-transcript-panel');
    expect(panel.getAttribute('data-role')).toBe('executor');
    const tabs = within(panel).getAllByTestId('transcript-role-tab');
    expect(tabs.map((tab) => tab.textContent)).toEqual(['执行会话', '评估会话']);
    await waitFor(() => expect(panel.textContent).toContain('执行会话正文'));

    fireEvent.click(tabs[1]);
    await waitFor(() =>
      expect(screen.getByTestId('session-transcript-panel').getAttribute('data-role')).toBe(
        'evaluator',
      ),
    );
    await waitFor(() =>
      expect(screen.getByTestId('session-transcript-panel').textContent).toContain('评估会话正文'),
    );
    expect(invokeMock).toHaveBeenCalledWith('agent_sessions', {
      root: 'root-a',
      source: 'change',
      sourceRef: 'test-change/dev-design/evaluator/2',
    });
  });

  it('非会话选中（active / interrupted / role=null 运行步 / 列头）→ 无转录联动区，既有三分节保持', () => {
    const base = detail({
      activePhase: { phase: 'implement', attempt: 1, startAt: null },
      interrupted: [{ phase: 'test-gen', attempt: 1, startAt: null, endAt: null }],
    });
    const toolNode = runtimeNode({
      id: 'run:implement:1:staticCheck',
      attempt: 1,
      runStepKind: 'staticCheck',
      group: 'toolStep',
      role: null,
      sessionId: null,
    });
    const graph = buildFlowGraph(base, [toolNode]);
    const state: World = { graph, materials: mountMaterials(graph, base, []), hasFileLog: true };

    const selections: DrawerSelection[] = [
      { scope: 'node', nodeId: 'active:implement:1' },
      { scope: 'node', nodeId: 'interrupted:test-gen:1' },
      { scope: 'node', nodeId: 'run:implement:1:staticCheck' },
      { scope: 'column', phase: 'implement' },
    ];
    for (const selection of selections) {
      const view = renderTranscriptDrawer(state, selection);
      expect(screen.getByTestId('detail-drawer') !== null).toBe(true);
      expect(screen.getByTestId('drawer-docs-section') !== null).toBe(true);
      expect(screen.getByTestId('drawer-eval-section') !== null).toBe(true);
      expect(screen.getByTestId('drawer-files-section') !== null).toBe(true);
      expect(screen.queryByTestId('session-transcript-panel')).toBeNull();
      view.unmount();
    }
  });

  it('liveEvents 按选中节点的 sessionId 过滤下传：重放为底、实时事件按 seq 并入增长，异会话事件不混入', async () => {
    sessionsFixture = {
      'test-change/implement/executor/2': [
        transcriptSession('ses-exec-2', 'test-change/implement/executor/2'),
      ],
    };
    transcriptFixture = { 'ses-exec-2': [textEvent(0, 'user', '重放正文')] };
    const state = runtimeWorld(runtimeNode());
    const selection: DrawerSelection = { scope: 'node', nodeId: 'run:implement:2:executor' };
    const { rerender } = renderTranscriptDrawer(state, selection);

    // 重放先行落底
    await waitFor(() =>
      expect(screen.getByTestId('session-transcript-panel').textContent).toContain('重放正文'),
    );
    // 运行中实时事件随后到站：面板收到选中节点的会话事件（seq 去重并入），异会话事件被过滤
    rerender(
      <DetailDrawer
        selection={selection}
        graph={state.graph}
        materials={state.materials}
        hasFileLog
        root="root-a"
        change="test-change"
        liveEvents={[
          { sessionId: 'ses-exec-2', event: textEvent(1, 'assistant', '实时增量正文') },
          { sessionId: 'ses-other', event: textEvent(9, 'assistant', '别的会话不混入') },
        ]}
        onClose={() => {}}
      />,
    );
    await waitFor(() =>
      expect(screen.getByTestId('session-transcript-panel').textContent).toContain('实时增量正文'),
    );
    const panel = screen.getByTestId('session-transcript-panel');
    expect(panel.textContent).toContain('重放正文');
    expect(panel.textContent).not.toContain('别的会话不混入');
  });
});
