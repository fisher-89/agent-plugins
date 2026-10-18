// @vitest-environment jsdom
import { act, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { AgentEvent, SessionSummary, TurnSummary } from '../../../types/dto';
import { SessionTranscriptPanel } from './session-transcript-panel';
import type { RoleSessionRef } from './types';

// ---------------------------------------------------------------------------
// 进程边界 Mock（Mock策略表）：invoke 按命令名分发并可计数（session_detail /
// agent_session_transcript / agent_session_watch），Channel mock 捕获 onmessage
// 供直投事件与 Record；`AgentSessionView` / `useAgentSession` / `AgentMessages`
// 真实组合（SessionMeta 由组件提供，本文件不再自定义）。
// ---------------------------------------------------------------------------

const { ChannelMock, invokeMock } = vi.hoisted(() => {
  class ChannelMock {
    onmessage: ((message: unknown) => void) | null = null;
    static instances: ChannelMock[] = [];
    constructor() {
      ChannelMock.instances.push(this);
    }
  }
  return { ChannelMock, invokeMock: vi.fn() };
});

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock, Channel: ChannelMock }));

// ---------------------------------------------------------------------------
// fixture
// ---------------------------------------------------------------------------

const ROOT = 'C:\\demo\\flow';
const EXECUTOR_SESSION = 'ses-exec-1';
const TS = 1727000000000;

interface ConfigSnapshotFixture {
  engine: 'cli' | 'sdk';
  agentInstanceId: number | null;
  model: string | null;
  permissionMode: string;
}

function turn(turnId: number, status: TurnSummary['status']): TurnSummary {
  return {
    turnId,
    sessionId: EXECUTOR_SESSION,
    status,
    startedAt: TS + turnId,
    finishedAt: status === 'running' ? null : TS + turnId + 999,
    numTurns: 1,
    costUsd: 0.1,
    durationMs: 500,
    error: null,
  };
}

function summary(turns: TurnSummary[], tokens: number | null = null): SessionSummary {
  return {
    row: {
      id: EXECUTOR_SESSION,
      remoteSessionId: `engine-${EXECUTOR_SESSION}`,
      configSnapshot: {
        engine: 'sdk',
        agentInstanceId: null,
        model: null,
        permissionMode: 'bypassPermissions',
      } satisfies ConfigSnapshotFixture,
      provenance: { source: 'change', sourceRef: null },
      createdAt: TS,
      updatedAt: TS + 1,
    },
    stats: {
      turnCount: turns.length,
      totalDurationMs: null,
      inputTokens: tokens,
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

function deltaEvent(seq: number, text: string): AgentEvent {
  return {
    seq,
    timestampMs: TS,
    kind: 'messageDelta',
    parentToolUseId: null,
    delta: { kind: 'text', text },
  };
}

function roleRef(): RoleSessionRef {
  return { role: 'executor', sessionId: EXECUTOR_SESSION };
}

// ---------------------------------------------------------------------------
// 可编程 IPC
// ---------------------------------------------------------------------------

interface IpcState {
  detail: SessionSummary | null | Error;
  transcript: AgentEvent[] | Error;
  watch: 'pending' | 'resolve' | 'reject';
}

let ipc: IpcState;

function resetIpc(overrides: Partial<IpcState> = {}): void {
  ipc = {
    detail: summary([turn(11, 'completed')]),
    transcript: [textMessage(0, '执行会话首轮'), textMessage(1, '执行会话续文')],
    watch: 'pending',
    ...overrides,
  };
  ChannelMock.instances.length = 0;
  invokeMock.mockReset();
  invokeMock.mockImplementation((command: string) => {
    if (command === 'session_detail') {
      return ipc.detail instanceof Error ? Promise.reject(ipc.detail) : Promise.resolve(ipc.detail);
    }
    if (command === 'agent_session_transcript') {
      return ipc.transcript instanceof Error
        ? Promise.reject(ipc.transcript)
        : Promise.resolve(ipc.transcript);
    }
    if (command === 'agent_session_watch') {
      if (ipc.watch === 'reject') return Promise.reject(new Error('watch 命令失败'));
      if (ipc.watch === 'pending') return new Promise<null>(() => {});
      return Promise.resolve(null);
    }
    return Promise.resolve(null);
  });
}

beforeEach(() => {
  resetIpc();
});

afterEach(() => {
  vi.useRealTimers();
});

function callsOf(name: string): unknown[][] {
  return invokeMock.mock.calls.filter(([command]) => command === name);
}

interface ChannelLike {
  onmessage: ((message: unknown) => void) | null;
}

function lastChannel(): ChannelLike {
  const instance = ChannelMock.instances.at(-1);
  if (!instance) throw new Error('尚未建立流（无 Channel 实例）');
  return instance;
}

function deliverEvent(event: AgentEvent): void {
  act(() => {
    lastChannel().onmessage?.({ ipc: 'event', event });
  });
}

function renderPanel(refs: RoleSessionRef | null, root: string | null = ROOT, refreshKey?: number) {
  return render(<SessionTranscriptPanel root={root} roleRef={refs} refreshKey={refreshKey} />);
}

async function mounted(refs: RoleSessionRef | null = roleRef(), root: string | null = ROOT) {
  const rendered = renderPanel(refs, root);
  await act(async () => {});
  await act(async () => {});
  return rendered;
}

function panelTexts(): string[] {
  return screen.queryAllByTestId('block-text').map((node) => node.textContent ?? '');
}

// ---------------------------------------------------------------------------
// 面板接线（自持形态）
// ---------------------------------------------------------------------------

describe('SessionTranscriptPanel：面板接线与元信息（D12 上移后断言面不变）', () => {
  it('roleRef 在场 → wrapper + session-transcript 自持挂载，对话 / 元信息由组件呈现', async () => {
    await mounted();
    const panel = screen.getByTestId('session-transcript-panel');
    await waitFor(() => expect(within(panel).getAllByTestId('block-text')).toHaveLength(2));

    expect(
      panel.querySelector('[data-transcript-role]')?.getAttribute('data-transcript-role'),
    ).toBe('executor');
    const view = within(panel).getByTestId('session-transcript');
    expect(view).toBeTruthy();
    expect(within(view).getByTestId('session-meta-id').textContent).toBe(EXECUTOR_SESSION);
    expect(within(view).getByTestId('session-meta-status').textContent).toBe('已收口');
    expect(panelTexts()).toEqual(['执行会话首轮', '执行会话续文']);
    expect(callsOf('session_detail')).toHaveLength(1);
    expect(callsOf('agent_session_transcript')).toHaveLength(1);
  });

  it('运行中会话 → 建立订阅，Channel 直投 delta → 面板文本 token 级增量可见（AC-5 前端半边）', async () => {
    resetIpc({ detail: summary([turn(11, 'running')]) });
    await mounted();

    await waitFor(() => expect(callsOf('agent_session_watch')).toHaveLength(1));
    expect(screen.getByTestId('session-meta-status').textContent).toBe('运行中');

    deliverEvent(deltaEvent(5, '旁观的增量'));
    await waitFor(() => expect(panelTexts()).toContain('旁观的增量'));
    // 不再「通知 → 重查转录库」：订阅期间零额外转录查询
    expect(callsOf('agent_session_transcript')).toHaveLength(1);
  });

  it('已收口会话 → agent_session_watch 零调用、零轮询（推进时间计数不增）', async () => {
    vi.useFakeTimers();
    await mounted();
    const before = invokeMock.mock.calls.length;

    await act(async () => {
      vi.advanceTimersByTime(5 * 60 * 1000);
    });

    expect(callsOf('agent_session_watch')).toHaveLength(0);
    expect(invokeMock.mock.calls.length).toBe(before);
  });
});

// ---------------------------------------------------------------------------
// 空态与错误面
// ---------------------------------------------------------------------------

describe('SessionTranscriptPanel：空态与错误（边界 / 异常）', () => {
  it('roleRef = null → drawer-session-empty 空态、零查询', async () => {
    await mounted(null);

    expect(screen.getByTestId('drawer-session-empty').textContent).toContain('当前选中无关联会话');
    expect(screen.queryByTestId('session-transcript-panel')).toBeNull();
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it('roleRef.sessionId = null → 空态（不虚构会话、零查询）', async () => {
    await mounted({ role: 'executor', sessionId: null });

    expect(screen.getByTestId('drawer-session-empty').textContent).toContain('此步无会话记录');
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it('会话查询 reject → 组件错误提示，面板框架不塌', async () => {
    resetIpc({ detail: new Error('db: 会话查询失败') });
    await mounted();

    await waitFor(() =>
      expect(screen.getByTestId('agent-session-error').textContent).toContain('会话查询失败'),
    );
    // 面板框架仍在（wrapper + 角色锚）
    expect(screen.getByTestId('session-transcript-panel')).toBeTruthy();
    expect(panelTexts()).toEqual([]);
  });

  it('session_detail 返回 null → 空对话态（不虚构会话元信息、零订阅）', async () => {
    resetIpc({ detail: null });
    await mounted();

    await waitFor(() => expect(screen.getByTestId('conversation-empty')).toBeTruthy());
    expect(screen.queryByTestId('session-meta')).toBeNull();
    expect(callsOf('agent_session_watch')).toHaveLength(0);
  });
});

// ---------------------------------------------------------------------------
// 状态重评估（refreshKey = 会话状态重评估信号）
// ---------------------------------------------------------------------------

describe('SessionTranscriptPanel：refreshKey 状态重评估', () => {
  it('refreshKey 变更 → 重放重取一次（新转录呈现在案）', async () => {
    const { rerender } = renderPanel(roleRef());
    await waitFor(() => expect(panelTexts()).toHaveLength(2));

    ipc.transcript = [
      textMessage(0, '执行会话首轮'),
      textMessage(1, '执行会话续文'),
      textMessage(2, '重取后新增'),
    ];
    ipc.detail = summary([turn(11, 'completed'), turn(12, 'completed')]);
    await act(async () => {
      rerender(<SessionTranscriptPanel root={ROOT} roleRef={roleRef()} refreshKey={1} />);
    });

    await waitFor(() => expect(panelTexts()).toContain('重取后新增'));
    expect(callsOf('session_detail')).toHaveLength(2);
    expect(screen.getByTestId('session-meta-turns').textContent).toBe('轮数 2');
  });

  it('running 转收口 → 订阅拆除、不重订（并行流 ≤ 1）', async () => {
    // 空轮订阅立即结束（断流收尾）；状态重评估后按收口库态收敛
    resetIpc({ detail: summary([turn(11, 'running')]), watch: 'resolve' });
    const { rerender } = renderPanel(roleRef());
    await act(async () => {});
    await waitFor(() => expect(callsOf('agent_session_watch')).toHaveLength(1));
    await act(async () => {});

    ipc.detail = summary([turn(11, 'completed')]);
    await act(async () => {
      rerender(<SessionTranscriptPanel root={ROOT} roleRef={roleRef()} refreshKey={1} />);
    });
    await waitFor(() =>
      expect(screen.getByTestId('session-meta-status').textContent).toBe('已收口'),
    );
    await act(async () => {});
    expect(callsOf('agent_session_watch'), '收口后不重订').toHaveLength(1);
  });

  it('会话切换（sessionId 变更）→ 新会话重取、旧会话零残留（watch 并行 ≤ 1）', async () => {
    const { rerender } = renderPanel(roleRef());
    await act(async () => {});
    await waitFor(() => expect(panelTexts()).toHaveLength(2));

    const otherSession = 'ses-review-2';
    ipc.detail = {
      ...summary([turn(21, 'completed')]),
      row: { ...summary([]).row, id: otherSession },
    };
    ipc.transcript = [textMessage(0, '评审会话正文')];
    await act(async () => {
      rerender(
        <SessionTranscriptPanel
          root={ROOT}
          roleRef={{ role: 'evaluator', sessionId: otherSession }}
        />,
      );
    });

    await waitFor(() => expect(panelTexts()).toEqual(['评审会话正文']));
    expect(
      screen
        .getByTestId('session-transcript-panel')
        .querySelector('[data-transcript-role]')
        ?.getAttribute('data-transcript-role'),
    ).toBe('evaluator');
    // 两条会话均无运行中轮行 → 零订阅（并行流 ≤ 1 的上界）
    expect(callsOf('agent_session_watch')).toHaveLength(0);
  });
});
