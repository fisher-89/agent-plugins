import { act, render, screen, waitFor, within } from '@testing-library/react';
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

function roleRef(): RoleSessionRef {
  return { role: 'executor', sourceRef: EXECUTOR_REF, sessionId: null };
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

function renderPanel(refs: RoleSessionRef | null, root: string | null = ROOT) {
  return render(<SessionTranscriptPanel root={root} roleRef={refs} />);
}

async function mounted(refs: RoleSessionRef | null = roleRef(), root: string | null = ROOT) {
  const rendered = renderPanel(refs, root);
  await act(async () => {});
  return rendered;
}

function panelTexts(): string[] {
  return screen.queryAllByTestId('block-text').map((node) => node.textContent);
}

describe('SessionTranscriptPanel：AgentMessages 渲染与重放单源', () => {
  it('密封重放消息经 AgentMessages 呈现（组件复用锚：同一条时间线，无第二套渲染）', async () => {
    await mounted();
    const panel = screen.getByTestId('session-transcript-panel');
    await waitFor(() => expect(within(panel).getAllByTestId('block-text')).toHaveLength(2));

    expect(within(panel).getByTestId('agent-messages') !== null).toBe(true);
    expect(
      within(panel)
        .getAllByTestId('block-text')
        .map((node) => node.textContent),
    ).toEqual(['执行会话首轮', '执行会话续文']);
  });

  it('refreshKey 自增触发转录库重查（实时面统一转录库——liveEvents 退役，重放单源）', async () => {
    const { rerender } = renderPanel(roleRef());
    await waitFor(() => expect(panelTexts()).toHaveLength(2));

    // 通知到达：转录库追加一条新密封事件（落库先于通知），refreshKey 自增
    sessionsByRef[EXECUTOR_REF] = [
      summary(EXECUTOR_SESSION, EXECUTOR_REF, [
        turn(11, EXECUTOR_SESSION, 'completed'),
        turn(12, EXECUTOR_SESSION, 'completed'),
      ]),
    ];
    transcriptBySession[EXECUTOR_SESSION] = [
      textMessage(0, '执行会话首轮'),
      textMessage(1, '执行会话续文'),
      textMessage(2, '实时片段'),
    ];
    rerender(<SessionTranscriptPanel root={ROOT} roleRef={roleRef()} refreshKey={1} />);
    await waitFor(() => expect(panelTexts()).toHaveLength(3));

    expect(panelTexts()).toEqual(['执行会话首轮', '执行会话续文', '实时片段']);
  });

  it('会话运行中 → agent-messages-running 呈现；全部终态 → 无该标记（运行中实时与收口重放同组件）', async () => {
    sessionsByRef[EXECUTOR_REF] = [
      summary(EXECUTOR_SESSION, EXECUTOR_REF, [turn(11, EXECUTOR_SESSION, 'running')]),
    ];
    const running = renderPanel(roleRef());
    await waitFor(() => expect(screen.getByTestId('agent-messages-running') !== null).toBe(true));
    running.unmount();

    // 恢复终态轮行 fixture（同测试前半段已改为 running）
    sessionsByRef[EXECUTOR_REF] = [
      summary(EXECUTOR_SESSION, EXECUTOR_REF, [turn(11, EXECUTOR_SESSION, 'completed')]),
    ];
    const done = renderPanel(roleRef());
    await waitFor(() => expect(panelTexts()).toHaveLength(2));
    expect(screen.queryByTestId('agent-messages-running')).toBeNull();
    done.unmount();
  });
});

describe('SessionTranscriptPanel：空态与错误（边界 / 异常）', () => {
  it('反查无会话：transcript-empty 空态呈现不炸、session-meta 整体不渲染（元信息不虚构会话——D4）', async () => {
    await mounted({
      role: 'executor',
      sourceRef: 'add-feature/code-review/executor/1',
      sessionId: null,
    });
    await waitFor(() => expect(screen.getByTestId('transcript-empty') !== null).toBe(true));

    expect(screen.queryByTestId('agent-messages')).toBeNull();
    expect(screen.queryByTestId('session-meta')).toBeNull();
    // 无错不出错误占位（error 与 empty 占位互斥呈现面）
    expect(screen.queryByTestId('transcript-error')).toBeNull();
  });

  it('root null：零 invoke、transcript-empty 空态', async () => {
    await mounted(roleRef(), null);

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
// 会话元信息区与恒渲染
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

    await mounted({ role: 'executor', sourceRef: EXECUTOR_REF, sessionId: null });
    await waitFor(() => expect(screen.getByTestId('session-meta') !== null).toBe(true));

    expect(metaText('session-meta-id')).toBe(EXECUTOR_SESSION);
    expect(metaText('session-meta-status')).toBe('运行中');
    expect(metaText('session-meta-turns')).toBe('轮数 1');
    expect(metaText('session-meta-tokens')).toBe('tokens 120 / 88');
    // 同源推导：running 徽章与对话区运行标记同时呈现
    expect(screen.getByTestId('agent-messages-running') !== null).toBe(true);
  });

  it('summary 在场（全部终态轮行）：徽章「已收口」（与 agent-messages-running 消失同源推导——状态单一事实源为轮行清单）', async () => {
    await mounted({ role: 'executor', sourceRef: EXECUTOR_REF, sessionId: null });
    await waitFor(() => expect(screen.getByTestId('session-meta') !== null).toBe(true));

    expect(metaText('session-meta-status')).toBe('已收口');
    expect(screen.queryByTestId('agent-messages-running')).toBeNull();
  });

  it('stats.inputTokens / outputTokens null → 「—」占位（不渲染字符串 "null"）；id 截断为 CSS 面、title 属性携全量', async () => {
    const first = await mounted({ role: 'executor', sourceRef: EXECUTOR_REF, sessionId: null });
    await waitFor(() => expect(screen.getByTestId('session-meta') !== null).toBe(true));

    expect(metaText('session-meta-tokens')).toBe('tokens — / —');
    expect(metaText('session-meta-tokens')).not.toContain('null');
    first.unmount();

    const longId = 'ses-' + 'x'.repeat(200);
    const longSummary = summary(longId, EXECUTOR_REF, [turn(11, longId, 'completed')]);
    sessionsByRef[EXECUTOR_REF] = [longSummary];
    const rerendered = renderPanel({ role: 'executor', sourceRef: EXECUTOR_REF, sessionId: null });
    await waitFor(() =>
      expect(screen.getByTestId('session-meta-id').getAttribute('title')).toBe(longId),
    );

    const idSpan = screen.getByTestId('session-meta-id');
    expect(idSpan.className).toContain('truncate');
    expect(idSpan.textContent).toBe(longId);
    rerendered.unmount();
  });

  it('summary null（反查无会话 / 未开跑角色）：session-meta 整体不渲染、transcript-empty 承担（D4）', async () => {
    await mounted({
      role: 'executor',
      sourceRef: 'add-feature/code-review/executor/1',
      sessionId: null,
    });
    await waitFor(() => expect(screen.getByTestId('transcript-empty') !== null).toBe(true));
    expect(screen.queryByTestId('session-meta')).toBeNull();
    expect(screen.queryByTestId('session-meta-id')).toBeNull();
    expect(screen.queryByTestId('session-meta-status')).toBeNull();
  });

  it('高度语义改填充宿主列：对话区 min-h-0 flex-1、滚动收在视口 overflow-y-auto、不含 max-h-[480px]；根节含 h-full（AC-1「转录拉满左列」）', async () => {
    await mounted();
    const panel = screen.getByTestId('session-transcript-panel');
    await waitFor(() => expect(within(panel).getAllByTestId('block-text')).toHaveLength(2));

    expect(panel.className).toContain('h-full');
    expect(panel.className).toContain('min-h-0');
    const conversation = within(panel).getByTestId('agent-messages');
    expect(conversation.className).toContain('min-h-0');
    expect(conversation.className).toContain('flex-1');
    const scroller = conversation.querySelector('[data-slot="message-scroller-viewport"]');
    expect(scroller?.className).toContain('overflow-y-auto');
    expect(`${panel.className} ${conversation.className}`).not.toContain('max-h-[480px]');
  });
});
