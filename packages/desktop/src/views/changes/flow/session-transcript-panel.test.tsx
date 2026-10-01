import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { AgentEvent, SessionSummary, TurnSummary } from '../../../types/dto';
import { SessionTranscriptPanel } from './session-transcript-panel';
import type { RoleSessionRef } from './types';

// ---------------------------------------------------------------------------
// 进程边界 Mock：invoke 按命令名分发（agent_sessions / agent_session_transcript，
// fixture 化反查 / 转录响应经 use-session-transcript 底层流入真实 hook——
// hook 不 mock）；AgentTimeline 真实实现参与渲染（沿 agent-run-history 先例）。
// 覆盖：role 分页选择 / sourceRef 定式反查 / 时间线复用 / liveEvents 透传 /
// 空态与单角色退化（AC-5 联动半边）。
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
    { role: 'executor', sourceRef: EXECUTOR_REF },
    { role: 'evaluator', sourceRef: EVALUATOR_REF },
  ];
}

// ---------------------------------------------------------------------------
// 可编程 IPC：按 sourceRef 反查、按 sessionId 取转录
// ---------------------------------------------------------------------------

let sessionsByRef: Record<string, SessionSummary[]>;
let transcriptBySession: Record<string, AgentEvent[]>;

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
  invokeMock.mockImplementation((command: string, params?: Record<string, unknown>) => {
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
  return screen.getAllByTestId('block-text').map((node) => node.textContent);
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
    await mounted([{ role: 'decision', sourceRef: DECISION_REF }]);
    await waitFor(() => expect(panelTexts()).toEqual(['决策会话问询']));

    const panel = screen.getByTestId('session-transcript-panel');
    expect(panel.getAttribute('data-role')).toBe('decision');
    expect(screen.queryByTestId('transcript-role-tab')).toBeNull();
  });

  it('roleRefs 空（未选中会话节点）：面板不渲染、零 invoke', async () => {
    const { container } = await mounted([]);

    expect(screen.queryByTestId('session-transcript-panel')).toBeNull();
    expect(container.childElementCount).toBe(0);
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
  it('反查无会话：transcript-empty 空态呈现不炸（节点尚无会话的合法空态）', async () => {
    await mounted([{ role: 'executor', sourceRef: 'add-feature/code-review/executor/1' }]);
    await waitFor(() => expect(screen.getByTestId('transcript-empty') !== null).toBe(true));

    expect(screen.queryByTestId('agent-timeline')).toBeNull();
  });

  it('root null：零 invoke、transcript-empty 空态', async () => {
    await mounted(roleRefs(), [], null);

    expect(invokeMock).not.toHaveBeenCalled();
    expect(screen.getByTestId('transcript-empty') !== null).toBe(true);
  });

  it('反查 reject → transcript-error 呈现（面板可感知）', async () => {
    invokeMock.mockImplementation((command: string) =>
      command === 'agent_sessions' ? Promise.reject('db: 会话查询失败') : Promise.resolve(null),
    );
    await mounted();
    await waitFor(() =>
      expect(screen.getByTestId('transcript-error').textContent).toBe('db: 会话查询失败'),
    );
  });
});
