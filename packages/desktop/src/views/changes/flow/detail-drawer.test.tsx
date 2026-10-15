import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type {
  AgentEvent,
  ArtifactEnvelope,
  ChangeDetail,
  SessionSummary,
} from '../../../types/dto';
import { mountMaterials } from './attachments';
import { DetailDrawer } from './detail-drawer';
import { buildFlowGraph } from './graph';
import type { DrawerSelection, FlowGraph, FlowMaterials, RuntimeFlowNode } from './types';

/**
 * requirement 相位序夹具（D12：前端 `PIPELINE_PHASES` 常量退役，列序改由
 * `detail.pipeline` 出线承载，测试各自内联 fixture）：9 列，archive 为第 9 列。
 */
const REQUIREMENT_PHASES = [
  'proposal',
  'dev-design',
  'test-design',
  'implement',
  'test-gen',
  'test-execution',
  'code-review',
  'acceptance',
  'archive',
];

// ---------------------------------------------------------------------------
// DetailDrawer 单测（change-flow-show-step-only，AC-2）：单列分派——列头 →
// 该站过程素材（ArtifactTabs）、WorkerAgent 步（role !== null）→ 会话转录、
// 带输出引用的步（outputRef !== null）→ stdout 面板；双列结构（drawer-docs-
// section / drawer-eval-section / DrawerEvalSection / InlineChecklist）整体
// 退役。进程边界 mock：invoke 按命令名分发（agent_sessions /
// agent_session_transcript / session_detail / read_exec_output）；内部模块
//（SessionTranscriptPanel / ArtifactTabs / StdoutPanel）真实组合不 mock。
// ---------------------------------------------------------------------------

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

/** change id（身份寻址键——sourceRef 定式身份段）与展示名（stdout 磁盘面键）。 */
const CHANGE_ID = '0198f7a0-0000-7000-8000-000000000000';
const NAME = 'add-feature';
const ROOT = 'C:\\demo\\flow';

/** 会话单查 / 反查 / 转录 / 输出记录的可编程注册表。 */
let sessionsFixture: Record<string, SessionSummary[]> = {};
let transcriptFixture: Record<string, AgentEvent[]> = {};
let detailFixture: Record<string, SessionSummary | null> = {};
/** read_exec_output 应答（string = 命中、null = miss、Error = reject）。 */
let outputResult: string | null | Error = null;

function defaultInvoke(
  command: string,
  args: { sessionId?: string; sourceRef?: string; execId?: string } = {},
): Promise<unknown> {
  if (command === 'session_detail') {
    return Promise.resolve(detailFixture[args.sessionId ?? ''] ?? null);
  }
  if (command === 'agent_sessions') {
    return Promise.resolve(sessionsFixture[args.sourceRef ?? ''] ?? []);
  }
  if (command === 'agent_session_transcript') {
    return Promise.resolve(transcriptFixture[args.sessionId ?? ''] ?? []);
  }
  if (command === 'read_exec_output') {
    void args;
    return outputResult instanceof Error
      ? Promise.reject(outputResult.message)
      : Promise.resolve(outputResult);
  }
  return Promise.resolve(null);
}

beforeEach(() => {
  sessionsFixture = {};
  transcriptFixture = {};
  detailFixture = {};
  outputResult = null;
  invokeMock.mockReset();
  invokeMock.mockImplementation(defaultInvoke);
});

afterEach(() => {
  invokeMock.mockReset();
  invokeMock.mockImplementation(defaultInvoke);
});

/** 建档详情底座（id ≠ name 形态：磁盘面寻址与展示面逐点可辨）。 */
function detail(overrides: Partial<ChangeDetail> = {}): ChangeDetail {
  return {
    id: CHANGE_ID,
    name: NAME,
    title: NAME,
    source: 'active',
    status: 'active',
    created: null,
    pipeline: REQUIREMENT_PHASES.map((phase) => ({ phase, attempts: [] })),
    activePhase: null,
    runs: [],
    worktree: null,
    artifacts: [],
    ...overrides,
  };
}

function envelope(kind: string, title: string, payload: unknown): ArtifactEnvelope {
  return { kind, version: 1, title, payload, fallbackText: null };
}

interface World {
  graph: FlowGraph;
  materials: FlowMaterials;
}

/** 真实 buildFlowGraph + mountMaterials 组装抽屉输入（按需覆盖素材面）。 */
function world(
  nodes: RuntimeFlowNode[] = [],
  columnDocs?: Record<string, ArtifactEnvelope[]>,
): World {
  const base = detail();
  return {
    graph: buildFlowGraph(base, nodes),
    materials: columnDocs === undefined ? mountMaterials(base, []) : { columnDocs },
  };
}

function renderDrawer(
  worldState: World,
  selection: DrawerSelection | null,
  overrides: { root?: string | null; name?: string; changeId?: string } = {},
) {
  return render(
    <DetailDrawer
      selection={selection}
      graph={worldState.graph}
      materials={worldState.materials}
      root={overrides.root === undefined ? ROOT : overrides.root}
      changeId={overrides.changeId ?? CHANGE_ID}
      name={overrides.name ?? NAME}
      onClose={() => {}}
    />,
  );
}

/** 运行步节点 fixture（run-state.ts runStepNodes 同形投影的直构形态）。 */
function runtimeNode(overrides: Partial<RuntimeFlowNode> = {}): RuntimeFlowNode {
  return {
    id: 'run:implement:2:executor:0',
    kind: 'runtime',
    phase: 'implement',
    attempt: 2,
    colIndex: REQUIREMENT_PHASES.indexOf('implement'),
    order: 0,
    parentId: 'col:implement',
    runId: 'run-1727',
    runStepKind: 'executor',
    group: 'workerAgent',
    role: 'executor',
    instanceNo: 0,
    status: 'running',
    sessionId: 'ses-exec-2',
    detail: null,
    outputRef: null,
    ...overrides,
  };
}

/** ToolStep 步节点 fixture（role null + outputRef 在场——stdout 面板分支）。 */
function outputNode(overrides: Partial<RuntimeFlowNode> = {}): RuntimeFlowNode {
  return runtimeNode({
    id: 'run:implement:2:staticCheck:0',
    runStepKind: 'staticCheck',
    group: 'toolStep',
    role: null,
    sessionId: null,
    outputRef: 'exec-1727000000000-5',
    ...overrides,
  });
}

/** store `SessionConfigSnapshot` 快照线形态（serde camelCase；v2 增
 * `agentInstanceId` 实例外键——fixture 缺键即类型红，防漂移）。 */
interface ConfigSnapshotFixture {
  engine: 'cli' | 'sdk';
  agentInstanceId: number | null;
  model: string | null;
  permissionMode: string;
}

/** 会话清单项 fixture（session_detail 应答；轮行已收敛 = 重放形态）。 */
function transcriptSession(sessionId: string, sourceRef: string): SessionSummary {
  return {
    row: {
      id: sessionId,
      remoteSessionId: null,
      configSnapshot: {
        engine: 'sdk',
        agentInstanceId: null,
        model: 'glm-high',
        permissionMode: 'bypassPermissions',
      } satisfies ConfigSnapshotFixture,
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

/** 密封文本消息事件 fixture（message 块 → AgentMessages 呈现）。 */
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

function readExecCalls(): unknown[][] {
  return invokeMock.mock.calls.filter(([name]) => name === 'read_exec_output');
}

describe('DetailDrawer：列头单列（该站过程素材）', () => {
  it('正向：selection.scope === column → 单列 ArtifactTabs 呈现该站文档 + eval-checklist，无会话转录区', async () => {
    const checklist = envelope('eval-checklist', '评估清单', {
      phase: 'dev-design',
      attempt: 2,
      verdict: 'pass',
      items: [{ item: '接口已定', pass: true, evidence: 'L1-10' }],
    });
    const state = world([], {
      'col:dev-design': [
        envelope('markdown-doc', '设计文档', { markdown: '# 设计正文' }),
        checklist,
      ],
    });
    renderDrawer(state, { scope: 'column', phase: 'dev-design' });

    expect(screen.getByTestId('detail-drawer').textContent).toContain('dev-design');
    const tabs = screen.getAllByTestId('artifact-tab');
    expect(tabs.map((tab) => tab.textContent)).toEqual(['设计文档', '评估清单']);
    // base-ui 抽屉经 portal 挂 body：以壳 testid 域内文本为断言面
    expect(screen.getByTestId('drawer-column-section').textContent).toContain('设计正文');
    // 列头分派不落会话区 / stdout 面（单列语义）
    expect(screen.queryByTestId('session-transcript-panel')).toBeNull();
    expect(screen.queryByTestId('stdout-panel')).toBeNull();
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it('正向：切到 eval-checklist tab → 渲染清单内容（挂列头的 checklist 与文档同列聚合）', async () => {
    const checklist = envelope('eval-checklist', '评估清单', {
      phase: 'dev-design',
      attempt: 2,
      verdict: 'fail',
      items: [{ item: '接口已定', pass: false, evidence: 'design.md 缺接口段' }],
    });
    const state = world([], {
      'col:dev-design': [
        envelope('markdown-doc', '设计文档', { markdown: '# 设计正文' }),
        checklist,
      ],
    });
    renderDrawer(state, { scope: 'column', phase: 'dev-design' });

    fireEvent.click(screen.getAllByTestId('artifact-tab')[1]);

    const checklistNode = await screen.findByTestId('checklist');
    expect(checklistNode.textContent).toContain('接口已定');
    expect(checklistNode.textContent).toContain('design.md 缺接口段');
    expect(within(checklistNode).getByTestId('checklist-verdict').textContent).toBe('fail');
  });

  it('边界：该站零素材 → 空态文案（列头照常开抽屉，不炸）', () => {
    renderDrawer(world(), { scope: 'column', phase: 'implement' });

    expect(screen.getByTestId('drawer-column-section').textContent).toContain('（无本站文档）');
    expect(screen.queryByTestId('session-transcript-panel')).toBeNull();
    expect(screen.queryByTestId('stdout-panel')).toBeNull();
  });
});

describe('DetailDrawer：runtime 步节点单列分派', () => {
  it('正向：role !== null → 单列 SessionTranscriptPanel（sessionId 直查优先，零 read_exec_output）', async () => {
    detailFixture = {
      'ses-exec-2': transcriptSession('ses-exec-2', `${CHANGE_ID}/implement/executor/2`),
    };
    transcriptFixture = {
      'ses-exec-2': [textEvent(0, 'user', '实现该功能'), textEvent(1, 'assistant', '开始实现')],
    };
    renderDrawer(world([runtimeNode()]), {
      scope: 'node',
      nodeId: 'run:implement:2:executor:0',
    });

    const panel = await screen.findByTestId('session-transcript-panel');
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('session_detail', {
        root: ROOT,
        sessionId: 'ses-exec-2',
      }),
    );
    await waitFor(() => expect(panel.textContent).toContain('开始实现'));
    // 单列分派：stdout 面板不并行挂载、输出记录零取数
    expect(screen.queryByTestId('stdout-panel')).toBeNull();
    expect(readExecCalls()).toHaveLength(0);
    // 抽屉标题：phase · attempt · runtime（节点载荷面）
    expect(screen.getByTestId('detail-drawer').textContent).toContain(
      'implement · attempt 2 · runtime',
    );
  });

  it('边界：sessionId 缺席（旧条目）→ 无反查兜底，呈「此步无会话记录」空态（react 面零取数）', async () => {
    // sourceRef 定式反查退役：槽位 sessionId 缺席即无会话可寻址，不得按定式回退
    // 反查（杜绝「同 ref 多会话取最近一条」歧义），如实呈步面空态
    sessionsFixture = {
      [`${CHANGE_ID}/implement/executor/2`]: [
        transcriptSession('ses-exec-2', `${CHANGE_ID}/implement/executor/2`),
      ],
    };
    transcriptFixture = { 'ses-exec-2': [textEvent(0, 'assistant', '反查命中正文')] };
    renderDrawer(world([runtimeNode({ sessionId: null })]), {
      scope: 'node',
      nodeId: 'run:implement:2:executor:0',
    });

    const empty = await screen.findByTestId('drawer-session-empty');
    expect(empty.textContent).toBe('此步无会话记录');
    expect(screen.queryByTestId('session-transcript-panel')).toBeNull();
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it('正向：outputRef !== null（role null 的 static-check / test-execution 步）→ 单列 StdoutPanel 取数 { root, changeName, execId }', async () => {
    outputResult = 'clippy: 3 warnings\n';
    renderDrawer(world([outputNode()]), { scope: 'node', nodeId: 'run:implement:2:staticCheck:0' });

    const panel = await screen.findByTestId('stdout-panel');
    await waitFor(() => expect(panel.textContent).toContain('clippy: 3 warnings'));
    expect(readExecCalls()).toEqual([
      ['read_exec_output', { root: ROOT, changeName: NAME, execId: 'exec-1727000000000-5' }],
    ]);
    // 单列分派：会话转录面不并行挂载、会话命令零调用
    expect(screen.queryByTestId('session-transcript-panel')).toBeNull();
    expect(invokeMock.mock.calls.filter(([name]) => name === 'agent_sessions')).toHaveLength(0);
  });

  it('正向：name prop 透传——stdout 取数按 change name 而非 changeId 寻址', async () => {
    outputResult = 'stdout 正文\n';
    renderDrawer(
      world([outputNode()]),
      { scope: 'node', nodeId: 'run:implement:2:staticCheck:0' },
      { changeId: CHANGE_ID, name: 'renamed-change' },
    );

    await waitFor(() => expect(readExecCalls()).toHaveLength(1));
    expect(readExecCalls()[0]).toEqual([
      'read_exec_output',
      { root: ROOT, changeName: 'renamed-change', execId: 'exec-1727000000000-5' },
    ]);
    expect(readExecCalls()[0][1]).not.toMatchObject({ changeName: CHANGE_ID });
  });

  it('异常：selection === null → 组件返回 null 不渲染', () => {
    const { container } = renderDrawer(world([runtimeNode()]), null);

    expect(container.childElementCount).toBe(0);
    expect(screen.queryByTestId('detail-drawer')).toBeNull();
  });

  it('异常：悬空 nodeId（图中无该节点）→ 中性占位标题 + 空态占位，不崩', () => {
    renderDrawer(world([runtimeNode()]), { scope: 'node', nodeId: 'run:ghost:9:executor:0' });

    expect(screen.getByTestId('detail-drawer') !== null).toBe(true);
    expect(screen.getByTestId('detail-drawer').textContent).toContain('事件节点');
    expect(screen.getByTestId('drawer-session-empty').textContent).toBe('（当前选中无关联会话）');
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it('异常：无信息源节点（role 与 outputRef 皆 null——直选形态）→ 中性空态占位，零取数', () => {
    const silent = runtimeNode({
      id: 'run:implement:2:phaseStart:0',
      runStepKind: 'phaseStart',
      group: 'toolStep',
      role: null,
      sessionId: null,
    });
    renderDrawer(world([silent]), { scope: 'node', nodeId: 'run:implement:2:phaseStart:0' });

    expect(screen.getByTestId('drawer-session-empty') !== null).toBe(true);
    expect(screen.queryByTestId('stdout-panel')).toBeNull();
    expect(screen.queryByTestId('session-transcript-panel')).toBeNull();
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it('异常：root === null 且 outputRef 在场 → stdout 面板呈空态（零 invoke，不炸）', async () => {
    renderDrawer(
      world([outputNode()]),
      { scope: 'node', nodeId: 'run:implement:2:staticCheck:0' },
      { root: null },
    );

    await waitFor(() => expect(screen.getByTestId('stdout-empty') !== null).toBe(true));
    expect(readExecCalls()).toHaveLength(0);
  });
});

describe('DetailDrawer：双列结构退役（单列壳）', () => {
  it('边界：列头 / 节点两种选中形态 → 整树无二分节 testid 与旧分节文案残留', async () => {
    const selections: DrawerSelection[] = [
      { scope: 'column', phase: 'dev-design' },
      { scope: 'node', nodeId: 'run:implement:2:executor:0' },
      { scope: 'node', nodeId: 'run:implement:2:staticCheck:0' },
    ];
    const state = world([runtimeNode(), outputNode()], {
      'col:dev-design': [envelope('markdown-doc', '设计文档', { markdown: '# 设计' })],
    });
    for (const selection of selections) {
      const { unmount } = renderDrawer(state, selection);
      const shellText = screen.getByTestId('detail-drawer').textContent ?? '';
      expect(screen.queryByTestId('drawer-docs-section')).toBeNull();
      expect(screen.queryByTestId('drawer-eval-section')).toBeNull();
      expect(shellText).not.toContain('评估记录');
      expect(shellText).not.toContain('（无评估记录）');
      // 内容壳恒单列：分派产物恰一个（二分节并列形态退役）
      const products = screen
        .getByTestId('detail-drawer')
        .querySelectorAll(
          [
            '[data-testid="drawer-column-section"]',
            '[data-testid="session-transcript-panel"]',
            '[data-testid="stdout-panel"]',
            '[data-testid="drawer-session-empty"]',
          ].join(', '),
        );
      expect(products).toHaveLength(1);
      unmount();
    }
  });
});

// ---------------------------------------------------------------------------
// configSnapshot fixture 补 agentInstanceId 键（fixture 随动，AC-7）
// ---------------------------------------------------------------------------

describe('DetailDrawer：configSnapshot fixture 携 agentInstanceId 键（AC-7 随动）', () => {
  it('补键后转录面板既有断言保绿（null 存量缺列语义）；具体 id 形态同口径', async () => {
    const legacy = transcriptSession('ses-exec-2', `${CHANGE_ID}/implement/executor/2`);
    // helper 默认形态：agentInstanceId null（存量会话缺列语义）
    expect((legacy.row.configSnapshot as ConfigSnapshotFixture).agentInstanceId).toBeNull();
    detailFixture = { 'ses-exec-2': legacy };
    transcriptFixture = {
      'ses-exec-2': [textEvent(0, 'user', '实现该功能'), textEvent(1, 'assistant', '开始实现')],
    };
    renderDrawer(world([runtimeNode()]), {
      scope: 'node',
      nodeId: 'run:implement:2:executor:0',
    });

    const panel = await screen.findByTestId('session-transcript-panel');
    await waitFor(() => expect(panel.textContent).toContain('开始实现'));

    // 具体实例 id（新会话语义）形态：fixture 同形、呈现面不受影响
    const fresh = transcriptSession('ses-exec-2', `${CHANGE_ID}/implement/executor/2`);
    fresh.row.configSnapshot = {
      engine: 'sdk',
      agentInstanceId: 7,
      model: 'glm-high',
      permissionMode: 'bypassPermissions',
    } satisfies ConfigSnapshotFixture;
    expect((fresh.row.configSnapshot as ConfigSnapshotFixture).agentInstanceId).toBe(7);
    detailFixture = { 'ses-exec-2': fresh };
  });
});
