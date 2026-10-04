import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { AgentEvent, SessionSummary, TurnSummary } from '../../../types/dto';
import { SessionTranscriptPanel } from './session-transcript-panel';
import type { RoleSessionRef } from './types';

// ---------------------------------------------------------------------------
// 进程边界 Mock
// ---------------------------------------------------------------------------

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

// ---------------------------------------------------------------------------
// fixture（serde camelCase 线格式）
// ---------------------------------------------------------------------------

const ROOT = 'C:\\demo\\flow';
const EXECUTOR_REF = 'add-feature/implement/executor/1';
const EVALUATOR_REF = 'add-feature/implement/evaluator/1';
const DECISION_REF = 'add-feature/implement/decision/1';
const EXECUTOR_SESSION = 'ses-exec-1';
const EVALUATOR_SESSION = 'ses-eval-1';
const DECISION_SESSION = 'ses-decision-1';
const TS = 1727000000000;

function turn(turnId: number, sessionId: string, status: TurnSummary['status']): TurnSummary {
  return {
    turnId,
    sessionId,
    status,
    startedAt: TS + turnId,
    finishedAt: status === 'running' ? null : TS + turnId + 999,
    numTurns: 1,
    costUsd: 0.1,
    durationMs: 500,
    error: null,
  };
}

function summary(id: string, sourceRef: string, turns: TurnSummary[]): SessionSummary {
  return {
    row: {
      id,
      remoteSessionId: `engine-${id}`,
      configSnapshot: { engine: 'cli', model: null, permissionMode: 'bypassPermissions' },
      provenance: { source: 'change', sourceRef },
      createdAt: TS,
      updatedAt: TS + 1,
    },
    stats: {
      turnCount: turns.length,
      totalDurationMs: null,
      inputTokens: null,
      outputTokens: null,
    },
    turns,
  };
}

function textMessage(seq: number, text: string): AgentEvent {
  return {
    seq,
    timestampMs: TS,
    kind: 'message',
    role: 'assistant',
    blocks: [{ kind: 'text', text }],
    parentToolUseId: null,
  };
}

function roleRefs(): RoleSessionRef[] {
  return [
    { role: 'executor', sourceRef: EXECUTOR_REF, sessionId: null },
    { role: 'evaluator', sourceRef: EVALUATOR_REF, sessionId: null },
  ];
}

// ---------------------------------------------------------------------------
// 可编程 IPC：按 sessionId 直查（session_detail）、按 sourceRef 反查、按
// sessionId 取转录——三臂注册表
// ---------------------------------------------------------------------------

let sessionsByRef: Record<string, SessionSummary[]>;
let transcriptBySession: Record<string, AgentEvent[]>;
let detailBySession: Record<string, SessionSummary | null | string>;

function mockIpc() {
  sessionsByRef = {
    [EXECUTOR_REF]: [
      summary(EXECUTOR_SESSION, EXECUTOR_REF, [turn(11, EXECUTOR_SESSION, 'completed')]),
    ],
    [EVALUATOR_REF]: [
      summary(EVALUATOR_SESSION, EVALUATOR_REF, [turn(12, EVALUATOR_SESSION, 'completed')]),
    ],
    [DECISION_REF]: [
      summary(DECISION_SESSION, DECISION_REF, [turn(13, DECISION_SESSION, 'completed')]),
    ],
  };
  transcriptBySession = {
    [EXECUTOR_SESSION]: [textMessage(0, '执行会话首轮'), textMessage(1, '执行会话续文')],
    [EVALUATOR_SESSION]: [textMessage(0, '评估会话结论')],
    [DECISION_SESSION]: [textMessage(0, '决策会话问询')],
  };
  detailBySession = {
    [EXECUTOR_SESSION]: summary(EXECUTOR_SESSION, EXECUTOR_REF, [
      turn(11, EXECUTOR_SESSION, 'completed'),
    ]),
    [EVALUATOR_SESSION]: summary(EVALUATOR_SESSION, EVALUATOR_REF, [
      turn(12, EVALUATOR_SESSION, 'completed'),
    ]),
    [DECISION_SESSION]: summary(DECISION_SESSION, DECISION_REF, [
      turn(13, DECISION_SESSION, 'completed'),
    ]),
  };
  invokeMock.mockImplementation((command: string, params?: Record<string, unknown>) => {
    if (command === 'session_detail') {
      const id = (params?.sessionId as string | undefined) ?? '';
      const hit = detailBySession[id];
      return typeof hit === 'string' ? Promise.reject(hit) : Promise.resolve(hit ?? null);
    }
    if (command === 'agent_sessions') {
      const ref = (params?.sourceRef as string | undefined) ?? '';
      return Promise.resolve(sessionsByRef[ref] ?? []);
    }
    if (command === 'agent_session_transcript') {
      const id = (params?.sessionId as string | undefined) ?? '';
      return Promise.resolve(transcriptBySession[id] ?? []);
    }
    return Promise.resolve(null);
  });
}

beforeEach(() => {
  invokeMock.mockReset();
  mockIpc();
});

function renderPanel(
  refs: RoleSessionRef[],
  liveEvents: AgentEvent[] = [],
  root: string | null = ROOT,
) {
  return render(<SessionTranscriptPanel root={root} roleRefs={refs} liveEvents={liveEvents} />);
}

async function mounted(
  refs: RoleSessionRef[] = roleRefs(),
  liveEvents: AgentEvent[] = [],
  root: string | null = ROOT,
) {
  const rendered = renderPanel(refs, liveEvents, root);
  await act(async () => {});
  return rendered;
}

function sessionsCalls(): unknown[][] {
  return invokeMock.mock.calls.filter(([name]) => name === 'agent_sessions');
}

function panelTexts(): string[] {
  return screen.queryAllByTestId('block-text').map((node) => node.textContent);
}

describe('SessionTranscriptPanel：role 分页与反查联动（AC-5 联动半边）', () => {
  it('多 role：分页 tab 按序渲染、默认选中首个 role、sourceRef 定式透传反查（source 恒 change）', async () => {
    await mounted();
    await waitFor(() => expect(panelTexts()).toContain('执行会话首轮'));

    const tabs = screen.getAllByTestId('transcript-role-tab');
    expect(tabs.map((tab) => tab.textContent)).toEqual(['执行会话', '评估会话']);
    const panel = screen.getByTestId('session-transcript-panel');
    expect(panel.getAttribute('data-role')).toBe('executor');

    const calls = sessionsCalls();
    expect(calls).toHaveLength(1);
    expect(calls[0]).toEqual([
      'agent_sessions',
      { root: ROOT, source: 'change', sourceRef: EXECUTOR_REF },
    ]);
  });

  it('点击分页切换：反查改用选中 role 的 sourceRef（role×attempt → sourceRef 组装）、转录随选切换', async () => {
    await mounted();
    await waitFor(() => expect(panelTexts()).toContain('执行会话首轮'));

    fireEvent.click(screen.getAllByTestId('transcript-role-tab')[1]);
    await waitFor(() =>
      expect(screen.getByTestId('session-transcript-panel').getAttribute('data-role')).toBe(
        'evaluator',
      ),
    );
    await waitFor(() => expect(panelTexts()).toEqual(['评估会话结论']));

    const calls = sessionsCalls();
    expect(calls).toHaveLength(2);
    expect(calls[1]).toEqual([
      'agent_sessions',
      { root: ROOT, source: 'change', sourceRef: EVALUATOR_REF },
    ]);
  });

  it('单 role 退化：无分页 tab、直接呈现该 role 转录（无分页歧义）', async () => {
    await mounted([{ role: 'decision', sourceRef: DECISION_REF, sessionId: null }]);
    await waitFor(() => expect(panelTexts()).toEqual(['决策会话问询']));

    const panel = screen.getByTestId('session-transcript-panel');
    expect(panel.getAttribute('data-role')).toBe('decision');
    expect(screen.queryByTestId('transcript-role-tab')).toBeNull();
  });

  it('roleRefs 空（列头 / ToolStep 选中）：面板渲染 drawer-session-empty 空态占位、零 invoke（恒渲染结构，AC-1 / D2）', async () => {
    const { container } = await mounted([]);

    expect(screen.queryByTestId('session-transcript-panel')).toBeNull();
    expect(screen.getByTestId('drawer-session-empty').textContent).toBe('（当前选中无关联会话）');
    expect(container.childElementCount).toBe(1);
    expect(invokeMock).not.toHaveBeenCalled();
  });
});

describe('SessionTranscriptPanel：AgentTimeline 渲染与 liveEvents 透传', () => {
  it('密封重放消息经 AgentTimeline 呈现（组件复用锚：同一条时间线，无第二套渲染）', async () => {
    await mounted();
    const panel = screen.getByTestId('session-transcript-panel');
    await waitFor(() => expect(within(panel).getAllByTestId('block-text')).toHaveLength(2));

    expect(within(panel).getByTestId('agent-timeline') !== null).toBe(true);
    expect(
      within(panel)
        .getAllByTestId('block-text')
        .map((node) => node.textContent),
    ).toEqual(['执行会话首轮', '执行会话续文']);
  });

  it('liveEvents 透传：运行中实时事件并入时间线（与重放按 seq 去重、增长不重复）', async () => {
    const { rerender } = renderPanel(roleRefs());
    await waitFor(() => expect(panelTexts()).toHaveLength(2));

    rerender(
      <SessionTranscriptPanel
        root={ROOT}
        roleRefs={roleRefs()}
        liveEvents={[textMessage(1, '重复 seq'), textMessage(2, '实时片段')]}
      />,
    );
    await waitFor(() => expect(panelTexts()).toHaveLength(3));

    expect(panelTexts()).toEqual(['执行会话首轮', '执行会话续文', '实时片段']);
  });

  it('会话运行中 → timeline-running 呈现；全部终态 → 无该标记（运行中实时与收口重放同组件）', async () => {
    sessionsByRef[EXECUTOR_REF] = [
      summary(EXECUTOR_SESSION, EXECUTOR_REF, [turn(11, EXECUTOR_SESSION, 'running')]),
    ];
    const running = renderPanel(roleRefs());
    await waitFor(() => expect(screen.getByTestId('timeline-running') !== null).toBe(true));
    running.unmount();

    // 恢复终态轮行 fixture（同测试前半段已改为 running）
    sessionsByRef[EXECUTOR_REF] = [
      summary(EXECUTOR_SESSION, EXECUTOR_REF, [turn(11, EXECUTOR_SESSION, 'completed')]),
    ];
    const done = renderPanel(roleRefs());
    await waitFor(() => expect(panelTexts()).toHaveLength(2));
    expect(screen.queryByTestId('timeline-running')).toBeNull();
    done.unmount();
  });
});

describe('SessionTranscriptPanel：空态与错误（边界 / 异常）', () => {
  it('反查无会话：transcript-empty 空态呈现不炸、session-meta 整体不渲染（元信息不虚构会话——D4）', async () => {
    await mounted([
      { role: 'executor', sourceRef: 'add-feature/code-review/executor/1', sessionId: null },
    ]);
    await waitFor(() => expect(screen.getByTestId('transcript-empty') !== null).toBe(true));

    expect(screen.queryByTestId('agent-timeline')).toBeNull();
    expect(screen.queryByTestId('session-meta')).toBeNull();
    // 无错不出错误占位（error 与 empty 占位互斥呈现面）
    expect(screen.queryByTestId('transcript-error')).toBeNull();
  });

  it('root null：零 invoke、transcript-empty 空态', async () => {
    await mounted(roleRefs(), [], null);

    expect(invokeMock).not.toHaveBeenCalled();
    expect(screen.getByTestId('transcript-empty') !== null).toBe(true);
  });

  it('反查 reject → transcript-error 呈现、session-meta 同样不渲染（错误态无元信息可呈）', async () => {
    invokeMock.mockImplementation((command: string) =>
      command === 'agent_sessions' ? Promise.reject('db: 会话查询失败') : Promise.resolve(null),
    );
    await mounted();
    await waitFor(() =>
      expect(screen.getByTestId('transcript-error').textContent).toBe('db: 会话查询失败'),
    );

    // 错误态不出空态占位（error 门禁下 empty 分支收口）
    expect(screen.queryByTestId('transcript-empty')).toBeNull();
    expect(screen.queryByTestId('session-meta')).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// 会话元信息区与恒渲染（desktop-drawer-session-column，AC-1 / AC-3 / D2 / D4 /
// D5）：`SessionMeta` 元信息区（session id 等宽截断 + title 全量、运行徽章、
// 轮数、token 合计 null → 「—」）；summary null 整体不渲染；高度语义改填充
// 宿主列（时间线容器 min-h-0 flex-1，根节 h-full——jsdom 无布局引擎，以类
// 契约断言）
// ---------------------------------------------------------------------------

describe('SessionTranscriptPanel：会话元信息区（SessionMeta）', () => {
  function metaText(testId: string): string {
    return screen.getByTestId(testId).textContent ?? '';
  }

  it('summary 在场（轮行含 running）：session id、「运行中」徽章、轮数、token 合计逐项可辨', async () => {
    const runningSummary = summary(EXECUTOR_SESSION, EXECUTOR_REF, [
      turn(11, EXECUTOR_SESSION, 'running'),
    ]);
    runningSummary.stats = { ...runningSummary.stats, inputTokens: 120, outputTokens: 88 };
    sessionsByRef[EXECUTOR_REF] = [runningSummary];

    await mounted([{ role: 'executor', sourceRef: EXECUTOR_REF, sessionId: null }]);
    await waitFor(() => expect(screen.getByTestId('session-meta') !== null).toBe(true));

    expect(metaText('session-meta-id')).toBe(EXECUTOR_SESSION);
    expect(metaText('session-meta-status')).toBe('运行中');
    expect(metaText('session-meta-turns')).toBe('轮数 1');
    expect(metaText('session-meta-tokens')).toBe('tokens 120 / 88');
    // 同源推导：running 徽章与时间线运行标记同时呈现
    expect(screen.getByTestId('timeline-running') !== null).toBe(true);
  });

  it('summary 在场（全部终态轮行）：徽章「已收口」（与 timeline-running 消失同源推导——状态单一事实源为轮行清单）', async () => {
    await mounted([{ role: 'executor', sourceRef: EXECUTOR_REF, sessionId: null }]);
    await waitFor(() => expect(screen.getByTestId('session-meta') !== null).toBe(true));

    expect(metaText('session-meta-status')).toBe('已收口');
    expect(screen.queryByTestId('timeline-running')).toBeNull();
  });

  it('stats.inputTokens / outputTokens null → 「—」占位（不渲染字符串 "null"）；id 截断为 CSS 面、title 属性携全量', async () => {
    const first = await mounted([{ role: 'executor', sourceRef: EXECUTOR_REF, sessionId: null }]);
    await waitFor(() => expect(screen.getByTestId('session-meta') !== null).toBe(true));

    expect(metaText('session-meta-tokens')).toBe('tokens — / —');
    expect(metaText('session-meta-tokens')).not.toContain('null');
    first.unmount();

    const longId = 'ses-' + 'x'.repeat(200);
    const longSummary = summary(longId, EXECUTOR_REF, [turn(11, longId, 'completed')]);
    sessionsByRef[EXECUTOR_REF] = [longSummary];
    const rerendered = renderPanel([
      { role: 'executor', sourceRef: EXECUTOR_REF, sessionId: null },
    ]);
    await waitFor(() =>
      expect(screen.getByTestId('session-meta-id').getAttribute('title')).toBe(longId),
    );

    const idSpan = screen.getByTestId('session-meta-id');
    expect(idSpan.className).toContain('truncate');
    expect(idSpan.textContent).toBe(longId);
    rerendered.unmount();
  });

  it('summary null（反查无会话 / 未开跑角色）：session-meta 整体不渲染、transcript-empty 承担（D4）', async () => {
    await mounted([
      { role: 'executor', sourceRef: 'add-feature/code-review/executor/1', sessionId: null },
    ]);
    await waitFor(() => expect(screen.getByTestId('transcript-empty') !== null).toBe(true));
    expect(screen.queryByTestId('session-meta')).toBeNull();
    expect(screen.queryByTestId('session-meta-id')).toBeNull();
    expect(screen.queryByTestId('session-meta-status')).toBeNull();
  });

  it('高度语义改填充宿主列：时间线容器 min-h-0 flex-1 overflow-y-auto、不含 max-h-[480px]；根节含 h-full（AC-1「转录拉满左列」）', async () => {
    await mounted();
    const panel = screen.getByTestId('session-transcript-panel');
    await waitFor(() => expect(within(panel).getAllByTestId('block-text')).toHaveLength(2));

    expect(panel.className).toContain('h-full');
    expect(panel.className).toContain('min-h-0');
    const timeline = within(panel).getByTestId('agent-timeline');
    const scroller = timeline.parentElement;
    expect(scroller?.className).toContain('min-h-0');
    expect(scroller?.className).toContain('flex-1');
    expect(scroller?.className).toContain('overflow-y-auto');
    expect(scroller?.className).not.toContain('max-h-[480px]');
  });
});

// ---------------------------------------------------------------------------
// 三会话 tab（desktop-change-session-visibility，AC-7 / D8）：执行 / 评估 /
// 决策三槽位 tab（tab key = sessionId ?? sourceRef ?? role 回退链）；decision
// 双 null 空态零查询（MUST NOT 虚构 / 误挂）；直查 reject 同错误态锚点。
// ---------------------------------------------------------------------------

describe('SessionTranscriptPanel：三会话 tab（AC-7）', () => {
  function detailCalls(): unknown[][] {
    return invokeMock.mock.calls.filter(([name]) => name === 'session_detail');
  }

  it('refs 携三 role 槽位均在场：三枚 transcript-role-tab（执行 / 评估 / 决策），默认选中执行并直查其转录', async () => {
    await mounted([
      { role: 'executor', sourceRef: EXECUTOR_REF, sessionId: EXECUTOR_SESSION },
      { role: 'evaluator', sourceRef: EVALUATOR_REF, sessionId: EVALUATOR_SESSION },
      { role: 'decision', sourceRef: DECISION_REF, sessionId: DECISION_SESSION },
    ]);
    await waitFor(() => expect(panelTexts()).toContain('执行会话首轮'));

    const tabs = screen.getAllByTestId('transcript-role-tab');
    expect(tabs.map((tab) => tab.textContent)).toEqual(['执行会话', '评估会话', '决策会话']);
    expect(screen.getByTestId('session-transcript-panel').getAttribute('data-role')).toBe(
      'executor',
    );
    // 默认执行 tab：session_detail 直查（sessionId 透传）
    expect(detailCalls()).toEqual([
      ['session_detail', { root: ROOT, sessionId: EXECUTOR_SESSION }],
    ]);
  });

  it('点击决策 tab → session_detail 直查其转录（sessionId 透传——AC-7）', async () => {
    await mounted([
      { role: 'executor', sourceRef: EXECUTOR_REF, sessionId: EXECUTOR_SESSION },
      { role: 'evaluator', sourceRef: EVALUATOR_REF, sessionId: EVALUATOR_SESSION },
      { role: 'decision', sourceRef: null, sessionId: DECISION_SESSION },
    ]);
    await waitFor(() => expect(panelTexts()).toContain('执行会话首轮'));

    fireEvent.click(screen.getAllByTestId('transcript-role-tab')[2]);
    await waitFor(() =>
      expect(screen.getByTestId('session-transcript-panel').getAttribute('data-role')).toBe(
        'decision',
      ),
    );
    await waitFor(() => expect(panelTexts()).toEqual(['决策会话问询']));

    expect(detailCalls()[1]).toEqual([
      'session_detail',
      { root: ROOT, sessionId: DECISION_SESSION },
    ]);
  });

  it('decision 双 null（槽位缺席）：决策 tab 呈 transcript-empty 空态、零查询、面板不渲染他 attempt 会话内容', async () => {
    await mounted([
      { role: 'executor', sourceRef: EXECUTOR_REF, sessionId: EXECUTOR_SESSION },
      { role: 'evaluator', sourceRef: EVALUATOR_REF, sessionId: EVALUATOR_SESSION },
      { role: 'decision', sourceRef: null, sessionId: null },
    ]);
    await waitFor(() => expect(panelTexts()).toContain('执行会话首轮'));

    fireEvent.click(screen.getAllByTestId('transcript-role-tab')[2]);
    await waitFor(() =>
      expect(screen.getByTestId('session-transcript-panel').getAttribute('data-role')).toBe(
        'decision',
      ),
    );
    await waitFor(() => expect(screen.getByTestId('transcript-empty') !== null).toBe(true));

    // 零查询：executor 直查恰一次（默认 tab），decision 无任何 invoke
    expect(detailCalls()).toHaveLength(1);
    // 面板不渲染他 attempt 会话内容（MUST NOT 虚构 / 误挂——单 ref 直渲染面）
    expect(panelTexts()).not.toContain('执行会话首轮');
    expect(panelTexts()).not.toContain('评估会话结论');
  });

  it('tab key 回退链：executor 反查 ref（sessionId null）与 decision 直查 ref（sessionId 在场）混合分页切换互不串页', async () => {
    await mounted([
      { role: 'executor', sourceRef: EXECUTOR_REF, sessionId: null },
      { role: 'evaluator', sourceRef: EVALUATOR_REF, sessionId: null },
      { role: 'decision', sourceRef: null, sessionId: DECISION_SESSION },
    ]);
    await waitFor(() => expect(panelTexts()).toContain('执行会话首轮'));

    // 反查 tab（executor）→ 直查 tab（decision）→ 反查 tab 往返，转录各归其页
    fireEvent.click(screen.getAllByTestId('transcript-role-tab')[2]);
    await waitFor(() => expect(panelTexts()).toEqual(['决策会话问询']));

    fireEvent.click(screen.getAllByTestId('transcript-role-tab')[0]);
    await waitFor(() => expect(panelTexts()).toEqual(['执行会话首轮', '执行会话续文']));

    fireEvent.click(screen.getAllByTestId('transcript-role-tab')[2]);
    await waitFor(() => expect(panelTexts()).toEqual(['决策会话问询']));

    // executor 反查、decision 直查，转录互不串页
    expect(panelTexts()).not.toContain('评估会话结论');
  });

  it('refs 变化（换节点选中）→ 分页复位到首个 role（前页选中不跨节点残留）', async () => {
    const initial = [
      { role: 'executor' as const, sourceRef: EXECUTOR_REF, sessionId: EXECUTOR_SESSION },
      { role: 'evaluator' as const, sourceRef: EVALUATOR_REF, sessionId: EVALUATOR_SESSION },
      { role: 'decision' as const, sourceRef: null, sessionId: DECISION_SESSION },
    ];
    const { rerender } = renderPanel(initial);
    await waitFor(() => expect(panelTexts()).toContain('执行会话首轮'));

    // 切到决策 tab（activeIndex = 2）
    fireEvent.click(screen.getAllByTestId('transcript-role-tab')[2]);
    await waitFor(() =>
      expect(screen.getByTestId('session-transcript-panel').getAttribute('data-role')).toBe(
        'decision',
      ),
    );

    // 换节点：roleRefs 换为新数组（两 ref）→ 复位首页 executor
    rerender(
      <SessionTranscriptPanel
        root={ROOT}
        roleRefs={[
          { role: 'executor', sourceRef: 'add-feature/code-review/executor/1', sessionId: null },
          { role: 'decision', sourceRef: null, sessionId: null },
        ]}
        liveEvents={[]}
      />,
    );
    await waitFor(() =>
      expect(screen.getByTestId('session-transcript-panel').getAttribute('data-role')).toBe(
        'executor',
      ),
    );
    await waitFor(() => expect(screen.getByTestId('transcript-empty') !== null).toBe(true));

    // 复位后仅首页 ref 取数（新节点 executor 反查恰一次，决策页不残留取数）
    const refs = sessionsCalls().map(
      (call) => (call[1] as { sourceRef?: string } | undefined)?.sourceRef,
    );
    expect(refs).toEqual(['add-feature/code-review/executor/1']);
  });

  it('refs 收缩越界（activeIndex 2 → refs 仅剩 2 枚）：钳制取位不越界，复位首页不崩', async () => {
    const initial = [
      { role: 'executor' as const, sourceRef: EXECUTOR_REF, sessionId: EXECUTOR_SESSION },
      { role: 'evaluator' as const, sourceRef: EVALUATOR_REF, sessionId: EVALUATOR_SESSION },
      { role: 'decision' as const, sourceRef: null, sessionId: DECISION_SESSION },
    ];
    const { rerender } = renderPanel(initial);
    await waitFor(() => expect(panelTexts()).toContain('执行会话首轮'));

    fireEvent.click(screen.getAllByTestId('transcript-role-tab')[2]);
    await waitFor(() =>
      expect(screen.getByTestId('session-transcript-panel').getAttribute('data-role')).toBe(
        'decision',
      ),
    );

    // 收缩为 2 枚 ref：activeIndex 2 越界 → 钳制到末位（渲染期不崩）→ effect 复位首页
    expect(() =>
      rerender(
        <SessionTranscriptPanel
          root={ROOT}
          roleRefs={[
            { role: 'executor', sourceRef: EXECUTOR_REF, sessionId: EXECUTOR_SESSION },
            { role: 'evaluator', sourceRef: EVALUATOR_REF, sessionId: EVALUATOR_SESSION },
          ]}
          liveEvents={[]}
        />,
      ),
    ).not.toThrow();
    await waitFor(() =>
      expect(screen.getByTestId('session-transcript-panel').getAttribute('data-role')).toBe(
        'executor',
      ),
    );
    await waitFor(() => expect(panelTexts()).toContain('执行会话首轮'));
  });

  it('点击次序页（三 ref 的第 2 枚）→ 恰落 evaluator 页（页序精确——非首末两页）', async () => {
    await mounted([
      { role: 'executor', sourceRef: EXECUTOR_REF, sessionId: EXECUTOR_SESSION },
      { role: 'evaluator', sourceRef: EVALUATOR_REF, sessionId: EVALUATOR_SESSION },
      { role: 'decision', sourceRef: null, sessionId: DECISION_SESSION },
    ]);
    await waitFor(() => expect(panelTexts()).toContain('执行会话首轮'));

    fireEvent.click(screen.getAllByTestId('transcript-role-tab')[1]);
    await waitFor(() =>
      expect(screen.getByTestId('session-transcript-panel').getAttribute('data-role')).toBe(
        'evaluator',
      ),
    );
    await waitFor(() => expect(panelTexts()).toEqual(['评估会话结论']));
  });

  it('直查 reject → transcript-error 呈现（与反查 reject 同错误态锚点）', async () => {
    detailBySession = { [DECISION_SESSION]: '会话不存在: id=ses-gone' };
    await mounted([{ role: 'decision', sourceRef: null, sessionId: DECISION_SESSION }]);
    await waitFor(() =>
      expect(screen.getByTestId('transcript-error').textContent).toBe('会话不存在: id=ses-gone'),
    );

    // 错误态无转录内容（时间线空态，不虚构会话内容）
    expect(screen.queryAllByTestId('block-text')).toHaveLength(0);
  });
});
