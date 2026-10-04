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
    executorSessionId: null,
    evaluatorSessionId: null,
    decisionSessionId: null,
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

  it('v1 代际（hasFileLog=false）→ 文件表节呈降级文案', () => {
    const state = world({ hasFileLog: false });
    renderDrawer(state, { scope: 'node', nodeId: 'eval:dev-design:2' });
    expect(screen.getByTestId('drawer-files-section').textContent).toContain(
      '（无 file_log 数据：v1 及更早代际无此字段）',
    );
  });
});

// ---------------------------------------------------------------------------
// 会话转录联动（desktop-change-flow 增量 + desktop-change-session-visibility
// 双键化）：role 寻址键双键化（sessionId 直查优先 / sourceRef 定式反查兜底 /
// decision 双 null 空态）。invokeMock 按 sourceRef / sessionId 双寻址注册表扩
// 展 session_detail 臂；SessionTranscriptPanel 与既有分节组件真实组合，fixture
// 经 mock invoke 按命令名分发流入真实 useSessionTranscript。
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

/** 会话清单项 fixture（agent_sessions / session_detail 应答；轮行已收敛 = 重放形态）。 */
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

/** 会话单查 / 反查 / 转录重放 fixture 注册表（按 sessionId / sourceRef 双寻址）。 */
let sessionsFixture: Record<string, SessionSummary[]> = {};
let transcriptFixture: Record<string, AgentEvent[]> = {};
let detailFixture: Record<string, SessionSummary | null> = {};

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

/** 时间线文本块序列（AgentTimeline block-text 依序呈现内容）。 */
function panelTexts(): string[] {
  return screen.queryAllByTestId('block-text').map((node) => node.textContent);
}

describe('DetailDrawer：会话转录联动（双键寻址：直查优先 / 反查兜底）', () => {
  beforeEach(() => {
    sessionsFixture = {};
    transcriptFixture = {};
    detailFixture = {};
    invokeMock.mockReset();
    invokeMock.mockImplementation(
      (command: string, args: { sourceRef?: string; sessionId?: string } = {}) => {
        if (command === 'session_detail') {
          const hit = detailFixture[args.sessionId ?? ''];
          return Promise.resolve(hit ?? null);
        }
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

  it('选中运行 executor 节点（sessionId 在场）→ 槽位 id 直查 session_detail（运行步实时 id 寻址）', async () => {
    detailFixture = {
      'ses-exec-2': transcriptSession('ses-exec-2', 'test-change/implement/executor/2'),
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
      expect(invokeMock).toHaveBeenCalledWith('session_detail', {
        root: 'root-a',
        sessionId: 'ses-exec-2',
      }),
    );
    // 直查命中的重放转录经 AgentTimeline 呈现；反查不被发起（直查优先）
    await waitFor(() => expect(panel.textContent).toContain('开始实现'));
    expect(invokeMock.mock.calls.filter(([name]) => name === 'agent_sessions')).toHaveLength(0);
    // 运行步节点抽屉标题：phase · attempt · runtime（attempt 取节点自身字段）
    expect(screen.getByTestId('detail-drawer').textContent).toContain(
      'implement · attempt 2 · runtime',
    );
  });

  it('选中运行 executor 节点（sessionId null 旧条目）→ 回退 sourceRef 定式反查（行为与升级前一致）', async () => {
    sessionsFixture = {
      'test-change/implement/executor/2': [
        transcriptSession('ses-exec-2', 'test-change/implement/executor/2'),
      ],
    };
    transcriptFixture = {
      'ses-exec-2': [textEvent(0, 'user', '实现该功能'), textEvent(1, 'assistant', '开始实现')],
    };
    renderTranscriptDrawer(runtimeWorld(runtimeNode({ sessionId: null })), {
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
    await waitFor(() => expect(panel.textContent).toContain('开始实现'));
  });

  it('选中历史 eval 节点（槽位缺席）→ executor + evaluator 双会话反查联动：默认 executor、切换 evaluator 重放对应 attempt', async () => {
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
    expect(tabs.map((tab) => tab.textContent)).toEqual(['执行会话', '评估会话', '决策会话']);
    await waitFor(() => expect(panel.textContent).toContain('执行会话正文'));
    // 空实时流下转录恰为重放内容（无 liveEvents 不得混入杂音行）
    await waitFor(() => expect(panelTexts()).toEqual(['执行会话正文']));

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

  it('选中历史 eval 节点（三槽位在场）→ 三 tab（执行 / 评估 / 决策），executor / decision 两路 session_detail 直查（槽位 id 透传）', async () => {
    const state = world({
      detail: {
        pipeline: PIPELINE_PHASES.map((phase) => ({
          phase,
          attempts:
            phase === 'dev-design'
              ? [
                  attempt({
                    attempt: 2,
                    executorSessionId: 'ses-exec-9',
                    evaluatorSessionId: 'ses-eval-9',
                    decisionSessionId: 'ses-decision-9',
                  }),
                ]
              : [],
        })),
      },
    });
    detailFixture = {
      'ses-exec-9': transcriptSession('ses-exec-9', 'test-change/dev-design/executor/2'),
      'ses-decision-9': transcriptSession('ses-decision-9', ''),
    };
    transcriptFixture = {
      'ses-exec-9': [textEvent(0, 'user', '执行槽位正文')],
      'ses-eval-9': [textEvent(0, 'assistant', '评估槽位正文')],
      'ses-decision-9': [textEvent(0, 'assistant', '决策槽位正文')],
    };
    renderDrawer(state, { scope: 'node', nodeId: 'eval:dev-design:2' });

    const panel = await screen.findByTestId('session-transcript-panel');
    const tabs = within(panel).getAllByTestId('transcript-role-tab');
    expect(tabs.map((tab) => tab.textContent)).toEqual(['执行会话', '评估会话', '决策会话']);
    // executor 槽位 id 直查（槽位 id 透传——AC-7 / AC-8）
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('session_detail', {
        root: 'root-a',
        sessionId: 'ses-exec-9',
      }),
    );
    await waitFor(() => expect(panel.textContent).toContain('执行槽位正文'));

    // 切决策 tab：decision 槽位 id 直查
    fireEvent.click(tabs[2]);
    await waitFor(() =>
      expect(screen.getByTestId('session-transcript-panel').getAttribute('data-role')).toBe(
        'decision',
      ),
    );
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('session_detail', {
        root: 'root-a',
        sessionId: 'ses-decision-9',
      }),
    );
    await waitFor(() =>
      expect(screen.getByTestId('session-transcript-panel').textContent).toContain('决策槽位正文'),
    );
  });

  it('evaluator 槽位在场 → 切评估 tab 走 session_detail 直查其槽位 id（非反查、非串位他槽 id）', async () => {
    const state = world({
      detail: {
        pipeline: PIPELINE_PHASES.map((phase) => ({
          phase,
          attempts:
            phase === 'dev-design'
              ? [
                  attempt({
                    attempt: 2,
                    executorSessionId: 'ses-exec-9',
                    evaluatorSessionId: 'ses-eval-9',
                    decisionSessionId: 'ses-decision-9',
                  }),
                ]
              : [],
        })),
      },
    });
    detailFixture = {
      'ses-exec-9': transcriptSession('ses-exec-9', 'test-change/dev-design/executor/2'),
      'ses-eval-9': transcriptSession('ses-eval-9', 'test-change/dev-design/evaluator/2'),
    };
    transcriptFixture = {
      'ses-exec-9': [textEvent(0, 'user', '执行槽位正文')],
      'ses-eval-9': [textEvent(0, 'assistant', '评估槽位正文')],
    };
    renderDrawer(state, { scope: 'node', nodeId: 'eval:dev-design:2' });

    const panel = await screen.findByTestId('session-transcript-panel');
    await waitFor(() => expect(panel.textContent).toContain('执行槽位正文'));

    fireEvent.click(within(panel).getAllByTestId('transcript-role-tab')[1]);
    await waitFor(() =>
      expect(screen.getByTestId('session-transcript-panel').getAttribute('data-role')).toBe(
        'evaluator',
      ),
    );
    // evaluator 槽位 id 直查（槽位 id 透传，不串 executor / decision 槽位）
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('session_detail', {
        root: 'root-a',
        sessionId: 'ses-eval-9',
      }),
    );
    await waitFor(() =>
      expect(screen.getByTestId('session-transcript-panel').textContent).toContain('评估槽位正文'),
    );
    // 直查路径全程不发起反查
    expect(invokeMock.mock.calls.filter(([name]) => name === 'agent_sessions')).toHaveLength(0);
  });

  it('旧条目（record 三槽位均 null）→ decision ref 双 null：决策 tab 空态且零查询（不误挂他 attempt 会话）', async () => {
    sessionsFixture = {
      'test-change/dev-design/executor/2': [
        transcriptSession('ses-exec-2', 'test-change/dev-design/executor/2'),
      ],
    };
    transcriptFixture = {
      'ses-exec-2': [textEvent(0, 'user', '执行反查正文')],
    };
    renderDrawer(world(), { scope: 'node', nodeId: 'eval:dev-design:2' });

    const panel = await screen.findByTestId('session-transcript-panel');
    const tabs = within(panel).getAllByTestId('transcript-role-tab');
    expect(tabs.map((tab) => tab.textContent)).toEqual(['执行会话', '评估会话', '决策会话']);

    // 切决策 tab：双 null → transcript-empty 空态、零查询
    fireEvent.click(tabs[2]);
    await waitFor(() =>
      expect(screen.getByTestId('session-transcript-panel').getAttribute('data-role')).toBe(
        'decision',
      ),
    );
    await waitFor(() => expect(screen.getByTestId('transcript-empty') !== null).toBe(true));
    // session_detail 零调用（decision 槽位缺席不虚构会话）；反查仅默认 executor
    // 一路（evaluator tab 未激活不取数）
    const detailCalls = invokeMock.mock.calls.filter(([name]) => name === 'session_detail');
    expect(detailCalls).toHaveLength(0);
    const sessionRefs = (invokeMock.mock.calls as unknown[][])
      .filter((call) => call[0] === 'agent_sessions')
      .map((call) => (call[1] as { sourceRef?: string } | undefined)?.sourceRef);
    expect(sessionRefs).toEqual(['test-change/dev-design/executor/2']);
  });

  it('eval 节点 attempt 为 null → ref 组为空、左列呈 drawer-session-empty 空态占位（零 invoke；attempt — 标题占位保留）', () => {
    const state = world({
      detail: {
        pipeline: PIPELINE_PHASES.map((phase) => ({
          phase,
          attempts:
            phase === 'dev-design'
              ? [attempt({ attempt: null, executorSessionId: 'ses-exec-9' })]
              : [],
        })),
      },
    });
    // attempt 缺号按 0 兜底成节点 id（graph.ts 装置）
    renderDrawer(state, { scope: 'node', nodeId: 'eval:dev-design:0' });

    // D2 恒渲染：ref 组为空 → 面板呈空态占位（非整块消失，双列结构不跳变）
    expect(screen.queryByTestId('session-transcript-panel')).toBeNull();
    expect(screen.getByTestId('drawer-session-empty').textContent).toBe('（当前选中无关联会话）');
    expect(invokeMock).not.toHaveBeenCalled();
    // 标题占位：attempt 缺号呈 —（非空串占位）；无文档列呈空态文案（非渲染产物条目）
    expect(screen.getByTestId('detail-drawer').textContent).toContain(
      'dev-design · attempt — · eval',
    );
    expect(screen.getByTestId('drawer-docs-section').textContent).toContain('（无本站文档）');
    // 无挂载信封 + 空清单 → 清单为空占位
    expect(screen.getByTestId('drawer-eval-section').textContent).toContain('（清单为空）');
  });

  it('节点 id 未命中 graph（悬空 selection）→ 标题中性占位「事件节点」不崩、左列空态占位承接', () => {
    renderDrawer(world(), { scope: 'node', nodeId: 'eval:nowhere:9' });

    expect(screen.getByTestId('detail-drawer') !== null).toBe(true);
    expect(screen.getByTestId('detail-drawer').textContent).toContain('事件节点');
    // 悬空节点无 ref 组 → 左列空态占位承接（D2 恒渲染），双列结构保持
    expect(screen.getByTestId('drawer-session-empty') !== null).toBe(true);
    // 未命中节点无素材面内容：文档节空态、eval 节空态
    expect(screen.getByTestId('drawer-docs-section').textContent).toContain('（无本站文档）');
    expect(screen.getByTestId('drawer-eval-section').textContent).toContain('（无评估记录）');
  });

  it('liveEvents 按选中节点的 sessionId 过滤下传：重放为底、实时事件按 seq 并入增长，异会话事件不混入', async () => {
    detailFixture = {
      'ses-exec-2': transcriptSession('ses-exec-2', 'test-change/implement/executor/2'),
    };
    transcriptFixture = { 'ses-exec-2': [textEvent(0, 'user', '重放正文')] };
    const state = runtimeWorld(runtimeNode());
    const selection: DrawerSelection = { scope: 'node', nodeId: 'run:implement:2:executor' };
    const { rerender } = renderTranscriptDrawer(state, selection);

    // 直查重放先行落底
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

// ---------------------------------------------------------------------------
// 双列壳（desktop-drawer-session-column，AC-1 / D1）：aside w-[960px] +
// max-w-[85vw]、去整列单滚、内容行 flex min-h-0 flex-1、左列 w-[60%] min-w-0
// 会话区恒渲染 / 右列 min-w-0 flex-1 overflow-y-auto 三分节——jsdom 无布局
// 引擎，以类契约断言列结构
// ---------------------------------------------------------------------------

describe('DetailDrawer：双列壳布局（AC-1 / D1）', () => {
  it('打开抽屉（eval 节点选中）→ 左列会话区与右列三分节同时渲染；壳类含 w-[960px] 与 max-w-[85vw]、右列容器列内滚动', () => {
    const { container } = renderDrawer(world(), { scope: 'node', nodeId: 'eval:dev-design:2' });

    // 抽屉壳：定宽 + 视口钳制
    const aside = container.querySelector('aside');
    expect(aside?.className).toContain('w-[960px]');
    expect(aside?.className).toContain('max-w-[85vw]');
    // 左列：会话面板宿主列 w-[60%] min-w-0（元信息 + 转录拉满滚动）
    const leftColumn = screen.getByTestId('session-transcript-panel').parentElement;
    expect(leftColumn?.className).toContain('w-[60%]');
    expect(leftColumn?.className).toContain('min-w-0');
    // 内容行：flex min-h-0 flex-1（双列并排、整列单滚移除）
    const contentRow = leftColumn?.parentElement;
    expect(contentRow?.className).toContain('flex');
    expect(contentRow?.className).toContain('min-h-0');
    expect(contentRow?.className).toContain('flex-1');
    // 右列三分节同时渲染，容器 min-w-0 flex-1 overflow-y-auto（列内滚动）
    expect(screen.getByTestId('drawer-docs-section') !== null).toBe(true);
    expect(screen.getByTestId('drawer-eval-section') !== null).toBe(true);
    expect(screen.getByTestId('drawer-files-section') !== null).toBe(true);
    const rightColumn = screen.getByTestId('drawer-docs-section').parentElement;
    expect(rightColumn?.className).toContain('min-w-0');
    expect(rightColumn?.className).toContain('flex-1');
    expect(rightColumn?.className).toContain('overflow-y-auto');
  });

  it('列头选中（非会话选中）→ 左列 drawer-session-empty 空态占位、右列三分节保持、双列结构不跳变', () => {
    renderDrawer(world(), { scope: 'column', phase: 'dev-design' });

    expect(screen.getByTestId('drawer-session-empty') !== null).toBe(true);
    expect(screen.queryByTestId('session-transcript-panel')).toBeNull();
    expect(screen.getByTestId('drawer-docs-section') !== null).toBe(true);
    expect(screen.getByTestId('drawer-eval-section') !== null).toBe(true);
    expect(screen.getByTestId('drawer-files-section') !== null).toBe(true);
  });

  it('role=null 运行步选中（ToolStep）→ 左列 drawer-session-empty、右列三分节保持（ToolStep / Gate 呈现不变）', () => {
    const toolStep = runtimeNode({
      id: 'run:implement:2:staticCheck',
      runStepKind: 'staticCheck',
      group: 'toolStep',
      role: null,
      sessionId: null,
    });
    renderTranscriptDrawer(runtimeWorld(toolStep), {
      scope: 'node',
      nodeId: 'run:implement:2:staticCheck',
    });

    expect(screen.getByTestId('drawer-session-empty') !== null).toBe(true);
    expect(screen.getByTestId('drawer-eval-section').textContent).toContain('（无评估记录）');
    expect(screen.getByTestId('drawer-files-section') !== null).toBe(true);
    // 抽屉标题：phase · attempt · runtime（ToolStep 词汇不变）
    expect(screen.getByTestId('detail-drawer').textContent).toContain(
      'implement · attempt 2 · runtime',
    );
  });
});

// ---------------------------------------------------------------------------
// active 节点三会话反查（desktop-drawer-session-column，AC-2 / D3）：
// selectionRoleRefs 补 `kind === 'active'` 分支——executor / evaluator /
// decision 三 ref，sessionId 恒 null、sourceRef 定式
// `<change>/<phase>/<role>/<attempt>`；全程不发起实时事件流订阅（liveEvents
// 按 sessionId 过滤恒空的结构保证）
// ---------------------------------------------------------------------------

/** active 事件节点世界：activePhase implement attempt 2（buildFlowGraph 真实产出 active 节点）。 */
function activeWorld(): World {
  const base = detail({
    activePhase: { phase: 'implement', attempt: 2, startAt: '2026-09-05T00:00:00Z' },
  });
  const graph = buildFlowGraph(base);
  return { graph, materials: mountMaterials(graph, base, []), hasFileLog: true };
}

describe('DetailDrawer：active 节点三会话反查（AC-2 / D3）', () => {
  beforeEach(() => {
    sessionsFixture = {};
    transcriptFixture = {};
    detailFixture = {};
    invokeMock.mockReset();
    invokeMock.mockImplementation(
      (command: string, args: { sourceRef?: string; sessionId?: string } = {}) => {
        if (command === 'session_detail') {
          const hit = detailFixture[args.sessionId ?? ''];
          return Promise.resolve(hit ?? null);
        }
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
    invokeMock.mockReset();
    invokeMock.mockImplementation(defaultInvoke);
  });

  it('active 节点选中 → 三转录 tab（执行 / 评估 / 决策）；executor 反查 agent_sessions 携定式 sourceRef、session_detail 恒零调用', async () => {
    sessionsFixture = {
      'test-change/implement/executor/2': [
        transcriptSession('ses-exec-2', 'test-change/implement/executor/2'),
      ],
    };
    transcriptFixture = { 'ses-exec-2': [textEvent(0, 'user', '已流出正文')] };
    renderTranscriptDrawer(activeWorld(), { scope: 'node', nodeId: 'active:implement:2' });

    const panel = await screen.findByTestId('session-transcript-panel');
    expect(panel.getAttribute('data-role')).toBe('executor');
    const tabs = within(panel).getAllByTestId('transcript-role-tab');
    expect(tabs.map((tab) => tab.textContent)).toEqual(['执行会话', '评估会话', '决策会话']);
    // 反查定式：source 恒 change + sourceRef `<change>/<phase>/<role>/<attempt>`
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('agent_sessions', {
        root: 'root-a',
        source: 'change',
        sourceRef: 'test-change/implement/executor/2',
      }),
    );
    await waitFor(() => expect(panel.textContent).toContain('已流出正文'));
    // sessionId 恒 null → session_detail 恒零调用（直查不发起）
    expect(invokeMock.mock.calls.filter(([name]) => name === 'session_detail')).toHaveLength(0);
  });

  it('已开跑角色（反查命中、轮行含 running 的进行中形态）→ 已流出转录重放呈现在左列（建档即落库的查询时快照面）', async () => {
    const runningSession = transcriptSession('ses-exec-2', 'test-change/implement/executor/2');
    runningSession.turns = [{ ...runningSession.turns[0], status: 'running', finishedAt: null }];
    sessionsFixture = {
      'test-change/implement/executor/2': [runningSession],
    };
    transcriptFixture = { 'ses-exec-2': [textEvent(0, 'user', '进行中已流出正文')] };
    renderTranscriptDrawer(activeWorld(), { scope: 'node', nodeId: 'active:implement:2' });

    await waitFor(() => expect(panelTexts()).toEqual(['进行中已流出正文']));
    // 进行中会话同源推导：timeline-running 标记随轮行呈现
    expect(screen.getByTestId('timeline-running') !== null).toBe(true);
  });

  it('切 decision tab → decision 走反查（active 无槽位，反查是该 phase / attempt 下 decision 会话唯一寻址）', async () => {
    sessionsFixture = {
      'test-change/implement/decision/2': [
        transcriptSession('ses-decision-2', 'test-change/implement/decision/2'),
      ],
    };
    transcriptFixture = { 'ses-decision-2': [textEvent(0, 'assistant', '决策已落库正文')] };
    renderTranscriptDrawer(activeWorld(), { scope: 'node', nodeId: 'active:implement:2' });

    await screen.findByTestId('session-transcript-panel');
    fireEvent.click(
      within(screen.getByTestId('session-transcript-panel')).getAllByTestId(
        'transcript-role-tab',
      )[2],
    );
    await waitFor(() =>
      expect(screen.getByTestId('session-transcript-panel').getAttribute('data-role')).toBe(
        'decision',
      ),
    );
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('agent_sessions', {
        root: 'root-a',
        source: 'change',
        sourceRef: 'test-change/implement/decision/2',
      }),
    );
    await waitFor(() => expect(panelTexts()).toEqual(['决策已落库正文']));
  });

  it('未开跑角色（反查空清单）→ 切该 tab 呈 transcript-empty「（暂无该会话转录）」、零进一步查询', async () => {
    sessionsFixture = {
      'test-change/implement/executor/2': [
        transcriptSession('ses-exec-2', 'test-change/implement/executor/2'),
      ],
    };
    transcriptFixture = { 'ses-exec-2': [textEvent(0, 'user', '执行正文')] };
    renderTranscriptDrawer(activeWorld(), { scope: 'node', nodeId: 'active:implement:2' });

    await waitFor(() => expect(panelTexts()).toEqual(['执行正文']));
    fireEvent.click(
      within(screen.getByTestId('session-transcript-panel')).getAllByTestId(
        'transcript-role-tab',
      )[1],
    );
    await waitFor(() => expect(screen.getByTestId('transcript-empty') !== null).toBe(true));
    expect(screen.getByTestId('transcript-empty').textContent).toBe('（暂无该会话转录）');
    // 反查恰 executor + evaluator 两路（decision tab 未激活不取数）；转录重放仅命中会话一路
    const sessionRefs = (invokeMock.mock.calls as unknown[][])
      .filter((call) => call[0] === 'agent_sessions')
      .map((call) => (call[1] as { sourceRef?: string } | undefined)?.sourceRef);
    expect(sessionRefs).toEqual([
      'test-change/implement/executor/2',
      'test-change/implement/evaluator/2',
    ]);
    expect(
      (invokeMock.mock.calls as unknown[][]).filter(
        (call) => call[0] === 'agent_session_transcript',
      ),
    ).toHaveLength(1);
  });

  it('liveEvents 非空传入 → 面板转录不含任何实时事件（active 无 sessionId → 过滤恒空，结构上不订阅实时流）', async () => {
    sessionsFixture = {
      'test-change/implement/executor/2': [
        transcriptSession('ses-exec-2', 'test-change/implement/executor/2'),
      ],
    };
    transcriptFixture = { 'ses-exec-2': [textEvent(0, 'user', '仅重放正文')] };
    renderTranscriptDrawer(activeWorld(), { scope: 'node', nodeId: 'active:implement:2' }, [
      { sessionId: 'ses-exec-2', event: textEvent(1, 'assistant', '实时增量不出现') },
    ]);

    await waitFor(() => expect(panelTexts()).toEqual(['仅重放正文']));
    expect(screen.getByTestId('session-transcript-panel').textContent).not.toContain(
      '实时增量不出现',
    );
  });

  it('sourceRef 定式与 active 节点 id 同参：active:<phase>:<attempt> 与 <change>/<phase>/<role>/<attempt> 同 attempt 组装', async () => {
    sessionsFixture = {
      'test-change/implement/executor/2': [
        transcriptSession('ses-exec-2', 'test-change/implement/executor/2'),
      ],
    };
    transcriptFixture = { 'ses-exec-2': [textEvent(0, 'user', '同参正文')] };
    renderTranscriptDrawer(activeWorld(), { scope: 'node', nodeId: 'active:implement:2' });

    const panel = await screen.findByTestId('session-transcript-panel');
    await waitFor(() => expect(panelTexts()).toEqual(['同参正文']));
    // 四段定式第二三段与节点 id `active:implement:2` 的 phase / attempt 同参
    const call = (invokeMock.mock.calls as unknown[][]).find(
      (entry) => entry[0] === 'agent_sessions',
    );
    const params = call?.[1] as { sourceRef?: string } | undefined;
    expect(params?.sourceRef).toBe('test-change/implement/executor/2');
    expect(panel.getAttribute('data-role')).toBe('executor');
  });
});
