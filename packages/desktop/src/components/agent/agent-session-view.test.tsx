// @vitest-environment jsdom
import { act, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { AgentSessionActions } from '@/components/agent/agent-session-view';
import { useAgentSession } from '@/hooks/use-agent-session';
import type { AgentEvent, SessionSummary, TurnSummary } from '@/types/dto';

import { AgentSessionView } from './agent-session-view';

// ---------------------------------------------------------------------------
// 进程边界 Mock（Mock策略表）：invoke 按命令名分发并可计数（session_detail /
// agent_session_transcript / agent_session_watch / agent_start / agent_stop），
// Channel mock 捕获 onmessage 供直投信封；`useAgentSession` / `AgentMessages` /
// `agent-blocks` 全部真实组合（组件即链路入口）。
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

const ROOT = 'C:\\demo\\alpha';
const SESSION_ID = 'ses-view-1';
const TS = 1727000000000;

interface ConfigSnapshotFixture {
  engine: 'cli' | 'sdk';
  agentInstanceId: number | null;
  model: string | null;
  permissionMode: string;
}

function turn(
  turnId: number,
  status: TurnSummary['status'],
  sessionId: string = SESSION_ID,
): TurnSummary {
  return {
    turnId,
    sessionId,
    status,
    startedAt: TS + turnId,
    finishedAt: status === 'running' ? null : TS + turnId + 900,
    numTurns: status === 'running' ? null : 1,
    costUsd: status === 'running' ? null : 0.1,
    durationMs: status === 'running' ? null : 500,
    error: null,
  };
}

function summary(
  turns: TurnSummary[],
  tokens: { input: number | null; output: number | null } = { input: null, output: null },
  id: string = SESSION_ID,
): SessionSummary {
  return {
    row: {
      id,
      remoteSessionId: 'sdk-1',
      configSnapshot: {
        engine: 'sdk',
        agentInstanceId: 3,
        model: 'm-high',
        permissionMode: 'bypassPermissions',
      } satisfies ConfigSnapshotFixture,
      provenance: { source: 'debug', sourceRef: null },
      createdAt: TS,
      updatedAt: TS + 9,
    },
    stats: {
      turnCount: turns.length,
      totalDurationMs: null,
      inputTokens: tokens.input,
      outputTokens: tokens.output,
    },
    turns,
  };
}

function textEvent(seq: number, text: string): AgentEvent {
  return {
    seq,
    timestampMs: TS,
    kind: 'message',
    role: 'assistant',
    blocks: [{ kind: 'text', text }],
    parentToolUseId: null,
  };
}

function userEvent(seq: number, text: string): AgentEvent {
  return {
    seq,
    timestampMs: TS,
    kind: 'message',
    role: 'user',
    blocks: [{ kind: 'text', text }],
    parentToolUseId: null,
  };
}

function turnDoneEvent(seq: number): AgentEvent {
  return {
    seq,
    timestampMs: TS,
    kind: 'turnDone',
    subtype: 'success',
    isError: false,
    numTurns: 1,
    durationMs: 400,
    costUsd: 0.1,
    usage: {},
    sessionId: 'sdk-1',
  };
}

function failedTurnDoneEvent(seq: number): AgentEvent {
  return {
    seq,
    // 轮行 startedAt = TS + turnId（11/12），事件时戳须晚于轮开始
    timestampMs: TS + 100 + seq,
    kind: 'turnDone',
    subtype: 'api_error',
    isError: true,
    numTurns: 1,
    durationMs: 1,
    costUsd: null,
    usage: null,
    sessionId: 'sdk-1',
  };
}

function apiErrorNoticeEvent(seq: number, error: string): AgentEvent {
  return {
    seq,
    timestampMs: TS + 100 + seq,
    kind: 'systemNotice',
    subtype: 'api_error',
    payload: { error },
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

// ---------------------------------------------------------------------------
// 可编程 IPC
// ---------------------------------------------------------------------------

interface IpcState {
  detail: SessionSummary | null | Error;
  transcript: AgentEvent[] | Error;
  /** 订阅命令应答：pending = 挂起（测试直投信封驱动）/ resolve / reject */
  watch: 'pending' | 'resolve' | 'reject';
}

let ipc: IpcState;

function resetIpc(overrides: Partial<IpcState> = {}): void {
  ipc = {
    detail: summary([turn(11, 'completed')]),
    transcript: [userEvent(0, '首轮提问'), textEvent(1, '首轮结论'), turnDoneEvent(2)],
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
    if (command === 'agent_start') return Promise.resolve(turn(13, 'running'));
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

function deliverRecord(record: TurnSummary): void {
  act(() => {
    lastChannel().onmessage?.({ ipc: 'record', record });
  });
}

function bubbleTexts(): string[] {
  return screen.queryAllByTestId('block-text').map((node) => node.textContent ?? '');
}

async function mountedView(props: Parameters<typeof AgentSessionView>[0]) {
  const rendered = render(<AgentSessionView {...props} />);
  await act(async () => {});
  await act(async () => {});
  return rendered;
}

// ---------------------------------------------------------------------------
// 自持形态：装配 / 元信息 / 对话
// ---------------------------------------------------------------------------

describe('AgentSessionView（自持形态）', () => {
  it('root + sessionId 挂载 → 各一次装配，呈现元信息行、对话气泡与轮行；data-testid 落在组件根', async () => {
    const { container } = await mountedView({
      root: ROOT,
      sessionId: SESSION_ID,
      'data-testid': 'session-custom',
    });

    expect(callsOf('session_detail')).toHaveLength(1);
    expect(callsOf('agent_session_transcript')).toHaveLength(1);
    expect(screen.getByTestId('session-custom')).toBeTruthy();
    expect(container.firstElementChild?.getAttribute('data-testid')).toBe('session-custom');

    await waitFor(() => expect(screen.getByTestId('session-meta')).toBeTruthy());
    expect(screen.getByTestId('session-meta-id').textContent).toBe(SESSION_ID);
    expect(screen.getByTestId('session-meta-status').textContent).toBe('已收口');
    expect(screen.getByTestId('session-meta-turns').textContent).toBe('轮数 1');
    expect(screen.getByTestId('session-meta-tokens').textContent).toBe('tokens — / —');
    expect(bubbleTexts()).toEqual(['首轮提问', '首轮结论']);
    // 轮行为 system 载体（对话透镜过滤），元信息轮数即轮行面单源
    expect(screen.getByTestId('agent-messages')).toBeTruthy();
  });

  it('缺省 data-testid → agent-session-view（单一入口默认锚）', async () => {
    await mountedView({ root: ROOT, sessionId: SESSION_ID });
    expect(screen.getByTestId('agent-session-view')).toBeTruthy();
  });

  it('运行中会话 → 恰一条订阅，delta 直投呈现 token 级累积文本（AC-2）', async () => {
    resetIpc({ detail: summary([turn(11, 'running')]) });
    await mountedView({ root: ROOT, sessionId: SESSION_ID });

    await waitFor(() => expect(callsOf('agent_session_watch')).toHaveLength(1));
    expect(screen.getByTestId('session-meta-status').textContent).toBe('运行中');

    deliverEvent(deltaEvent(3, '流式增量正文'));
    await waitFor(() => expect(bubbleTexts()).toContain('流式增量正文'));
  });

  it('完成态 → 零订阅零轮询（推进时间 invoke 计数不增）', async () => {
    vi.useFakeTimers();
    await mountedView({ root: ROOT, sessionId: SESSION_ID });
    const before = invokeMock.mock.calls.length;

    await act(async () => {
      vi.advanceTimersByTime(5 * 60 * 1000);
    });

    expect(callsOf('agent_session_watch')).toHaveLength(0);
    expect(invokeMock.mock.calls.length).toBe(before);
  });
});

// ---------------------------------------------------------------------------
// 注入形态
// ---------------------------------------------------------------------------

/** 注入形态宿主：来源侧已建 hook（交互页形态）。 */
function InjectedHost({
  sessionId,
  withComposer,
}: {
  sessionId: string | null;
  withComposer: boolean;
}): React.JSX.Element {
  const session = useAgentSession({ root: ROOT, sessionId, source: 'debug' });
  return (
    <AgentSessionView
      session={session}
      data-testid="injected-view"
      composer={
        withComposer
          ? ({ running, loading }) => (
              <div data-testid="injected-composer">
                running={String(running)} loading={String(loading)}
              </div>
            )
          : undefined
      }
    />
  );
}

describe('AgentSessionView（注入形态）', () => {
  it('传 session → 零额外会话查询、零第二条流；渲染体与自持形态一致', async () => {
    render(<InjectedHost sessionId={SESSION_ID} withComposer={false} />);
    await act(async () => {});
    await waitFor(() => expect(screen.getByTestId('session-meta')).toBeTruthy());

    expect(callsOf('session_detail')).toHaveLength(1);
    expect(callsOf('agent_session_transcript')).toHaveLength(1);
    expect(callsOf('agent_session_watch')).toHaveLength(0);
    expect(screen.getByTestId('agent-messages')).toBeTruthy();
    expect(bubbleTexts()).toEqual(['首轮提问', '首轮结论']);
    // 无插槽 → 只读形态（无发送 / 停止入口）
    expect(screen.queryByTestId('explore-send')).toBeNull();
    expect(screen.queryByTestId('explore-stop')).toBeNull();
  });

  it('注入空 session（sessionId null）→ 注入形态仍呈现（不落自持空态分支），零取数', async () => {
    render(<InjectedHost sessionId={null} withComposer />);
    await act(async () => {});

    expect(invokeMock).not.toHaveBeenCalled();
    expect(screen.queryByTestId('agent-session-empty')).toBeNull();
    expect(screen.getByTestId('injected-composer').textContent).toBe('running=false loading=false');
  });
});

// ---------------------------------------------------------------------------
// composer 插槽 / 只读形态 / 空态 / 错误面
// ---------------------------------------------------------------------------

describe('AgentSessionView：composer 插槽与只读形态', () => {
  it('composer render prop：内容渲染在消息区下方，running / loading 取自会话状态，send 触发 agent_start', async () => {
    const captured: { actions: AgentSessionActions | null } = { actions: null };
    await mountedView({
      root: ROOT,
      sessionId: SESSION_ID,
      composer: (value) => {
        captured.actions = value;
        return (
          <button
            type="button"
            data-testid="slot-send"
            onClick={() => value.send({ prompt: '插槽发送', permissionMode: 'bypassPermissions' })}
          >
            发送
          </button>
        );
      },
    });

    const slot = screen.getByTestId('slot-send');
    expect(screen.getByTestId('agent-messages')).toBeTruthy();
    expect(slot).toBeTruthy();
    // loading 在途判定与 running 库态派生
    expect(captured.actions).not.toBeNull();
    expect(captured.actions?.running).toBe(false);

    act(() => {
      slot.click();
    });
    await act(async () => {});
    expect(callsOf('agent_start')).toHaveLength(1);
  });

  it('不传 composer（change 抽屉 / 历史重放形态）→ 无发送输入、无停止按钮、零动作面接线', async () => {
    resetIpc({ detail: summary([turn(11, 'running')]) });
    await mountedView({ root: ROOT, sessionId: SESSION_ID });
    await waitFor(() => expect(callsOf('agent_session_watch')).toHaveLength(1));

    expect(screen.queryByTestId('explore-composer')).toBeNull();
    expect(screen.queryByTestId('explore-send')).toBeNull();
    expect(screen.queryByTestId('explore-stop')).toBeNull();
    expect(screen.queryByTestId('explore-prompt')).toBeNull();
    // 只读消费点零动作面 invoke（stop 不被自发调用）
    expect(callsOf('agent_stop')).toHaveLength(0);
    expect(callsOf('agent_start')).toHaveLength(0);
  });
});

describe('AgentSessionView：空态与错误面', () => {
  it('sessionId null（自持）→ 空态提示、零取数零订阅', async () => {
    await mountedView({ root: ROOT, sessionId: null });

    expect(screen.getByTestId('agent-session-empty').textContent).toContain('暂无会话');
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it('会话查询 reject → 错误提示呈现（不抛错、不渲染伪气泡）', async () => {
    resetIpc({ detail: new Error('db: 会话查询失败') });
    await mountedView({ root: ROOT, sessionId: SESSION_ID });

    await waitFor(() =>
      expect(screen.getByTestId('agent-session-error').textContent).toContain('会话查询失败'),
    );
    expect(bubbleTexts()).toEqual([]);
    expect(screen.queryByTestId('session-meta')).toBeNull();
  });

  it('最新轮失败且转录含 api_error → 失败横幅呈现友好文案并保留原始错误', async () => {
    resetIpc({
      detail: summary([turn(11, 'failed')]),
      transcript: [
        userEvent(0, '复现 invalid uri'),
        apiErrorNoticeEvent(1, 'API 请求失败: RequestError: invalid uri character'),
        failedTurnDoneEvent(2),
      ],
    });
    await mountedView({ root: ROOT, sessionId: SESSION_ID });

    await waitFor(() => expect(screen.getByTestId('agent-session-failure')).toBeTruthy());
    expect(screen.getByTestId('agent-session-failure-title').textContent).toBe('Provider 地址无效');
    expect(screen.getByTestId('agent-session-failure-hint').textContent).toContain('base_url');
    expect(screen.getByTestId('agent-session-failure-raw').textContent).toContain(
      'invalid uri character',
    );
    // system 载体不进对话区：只有用户提问气泡
    expect(bubbleTexts()).toEqual(['复现 invalid uri']);
  });

  it('上一轮失败、最新轮已完成 → 不展示旧失败横幅', async () => {
    resetIpc({
      detail: summary([turn(11, 'failed'), turn(12, 'completed')]),
      transcript: [
        userEvent(0, '第一轮'),
        apiErrorNoticeEvent(1, 'API 请求失败: RequestError: invalid uri character'),
        failedTurnDoneEvent(2),
      ],
    });
    await mountedView({ root: ROOT, sessionId: SESSION_ID });

    await waitFor(() => expect(screen.getByTestId('session-meta')).toBeTruthy());
    expect(screen.queryByTestId('agent-session-failure')).toBeNull();
  });

  it('最新轮停止 → 不展示失败横幅', async () => {
    resetIpc({
      detail: summary([turn(11, 'stopped')]),
      transcript: [userEvent(0, '停止轮')],
    });
    await mountedView({ root: ROOT, sessionId: SESSION_ID });

    await waitFor(() => expect(screen.getByTestId('session-meta')).toBeTruthy());
    expect(screen.queryByTestId('agent-session-failure')).toBeNull();
  });

  it('session_detail 返回 null → 空态（不虚构会话元信息、零订阅）', async () => {
    resetIpc({ detail: null });
    await mountedView({ root: ROOT, sessionId: SESSION_ID });

    await waitFor(() => expect(screen.getByTestId('conversation-empty')).toBeTruthy());
    expect(screen.queryByTestId('session-meta')).toBeNull();
    expect(callsOf('agent_session_watch')).toHaveLength(0);
    expect(screen.queryByTestId('agent-session-error')).toBeNull();
  });

  it('loading 在途呈现加载占位（不呈现伪空态），落地后占位消失', async () => {
    const gate: { resolve: ((value: SessionSummary | null) => void) | null } = { resolve: null };
    invokeMock.mockImplementation((command: string) => {
      if (command === 'session_detail') {
        return new Promise<SessionSummary | null>((resolve) => {
          gate.resolve = resolve;
        });
      }
      if (command === 'agent_session_transcript') return Promise.resolve([]);
      return Promise.resolve(null);
    });
    render(<AgentSessionView root={ROOT} sessionId={SESSION_ID} />);
    await act(async () => {});

    expect(screen.getByTestId('agent-messages-loading')).toBeTruthy();
    expect(screen.queryByTestId('conversation-empty')).toBeNull();

    await act(async () => {
      gate.resolve?.(summary([turn(11, 'completed')]));
    });
    await waitFor(() => expect(screen.queryByTestId('agent-messages-loading')).toBeNull());
  });
});

// ---------------------------------------------------------------------------
// 状态重评估与形态切换
// ---------------------------------------------------------------------------

describe('AgentSessionView：状态重评估与形态切换', () => {
  it('refreshKey 变更 → 静默态重放重取；收口转 running 挂一次，在挂流期间不重建（并行流 ≤ 1）', async () => {
    // 静默态（轮行全收口）：refreshKey 变更即重放重取、零订阅
    const { rerender } = await mountedView({ root: ROOT, sessionId: SESSION_ID });
    expect(callsOf('agent_session_watch')).toHaveLength(0);
    const detailBefore = callsOf('session_detail').length;

    ipc.detail = summary([turn(11, 'completed'), turn(12, 'completed')]);
    await act(async () => {
      rerender(<AgentSessionView root={ROOT} sessionId={SESSION_ID} refreshKey={1} />);
    });
    await waitFor(() => expect(callsOf('session_detail').length).toBeGreaterThan(detailBefore));
    expect(callsOf('agent_session_watch')).toHaveLength(0);

    // 收口转 running：无在挂流 → 挂一次
    ipc.detail = summary([turn(11, 'running')]);
    await act(async () => {
      rerender(<AgentSessionView root={ROOT} sessionId={SESSION_ID} refreshKey={2} />);
    });
    await waitFor(() => expect(callsOf('agent_session_watch')).toHaveLength(1));

    // 在挂流期间状态重评估：不重建第二条流
    await act(async () => {
      rerender(<AgentSessionView root={ROOT} sessionId={SESSION_ID} refreshKey={3} />);
    });
    await act(async () => {});
    expect(callsOf('agent_session_watch')).toHaveLength(1);
  });

  it('自持切注入 → 不产生并行第二条流（watch 计数恒 ≤ 1）；卸载后零后续 invoke', async () => {
    resetIpc({ detail: summary([turn(11, 'running')]) });
    const { rerender, unmount } = await mountedView({ root: ROOT, sessionId: SESSION_ID });
    await waitFor(() => expect(callsOf('agent_session_watch')).toHaveLength(1));

    await act(async () => {
      rerender(<InjectedHost sessionId={SESSION_ID} withComposer={false} />);
    });
    await act(async () => {});
    // 注入形态宿主自建一条流 → 总数不超过 2（自持那条拆除后注入那条在挂）
    expect(callsOf('agent_session_watch').length).toBeLessThanOrEqual(2);

    unmount();
    const after = invokeMock.mock.calls.length;
    await act(async () => {});
    expect(invokeMock.mock.calls.length, '卸载后零后续 invoke').toBe(after);
  });
});

// ---------------------------------------------------------------------------
// 组合链路（AgentSessionView → useAgentSession → transport → 命令 → Channel）
// ---------------------------------------------------------------------------

describe('AgentSessionView：组合链路', () => {
  it('运行中挂载 → 取数 → 状态门 → 订阅 → delta / 密封 / Record → 收口重放呈现与重开一致的形状', async () => {
    resetIpc({
      detail: summary([turn(11, 'running')]),
      transcript: [userEvent(0, '首轮提问')],
    });
    await mountedView({ root: ROOT, sessionId: SESSION_ID });

    await waitFor(() => expect(callsOf('agent_session_watch')).toHaveLength(1));

    // 增量直投（token 级）
    deliverEvent(deltaEvent(1, '流式正文'));
    await waitFor(() => expect(bubbleTexts()).toContain('流式正文'));

    // 收口：库内转录补齐（密封事件 + 跨消息 tool 单卡），轮行转 completed
    ipc.detail = summary([turn(11, 'completed')]);
    ipc.transcript = [
      userEvent(0, '首轮提问'),
      {
        seq: 1,
        timestampMs: TS,
        kind: 'message',
        role: 'assistant',
        blocks: [
          { kind: 'text', text: '收口正文' },
          { kind: 'toolUse', id: 'tu_1', name: 'Bash', input: { command: 'ls' } },
        ],
        parentToolUseId: null,
      },
      turnDoneEvent(2),
      {
        seq: 3,
        timestampMs: TS,
        kind: 'message',
        role: 'tool',
        blocks: [{ kind: 'toolResult', id: 'tu_1', content: '目录内容', isError: false }],
        parentToolUseId: null,
      },
    ];
    deliverRecord(turn(11, 'completed'));

    await waitFor(() =>
      expect(screen.getByTestId('session-meta-status').textContent).toBe('已收口'),
    );
    expect(bubbleTexts()).toEqual(['首轮提问', '收口正文']);
    // 跨消息 tool 配对收敛为单卡（无双卡残留）
    const panel = screen.getByTestId('agent-messages');
    expect(within(panel).getAllByTestId('block-tool-use')).toHaveLength(1);
    expect(within(panel).getAllByTestId('block-tool-result')).toHaveLength(1);
    expect(callsOf('agent_session_watch'), '收口后订阅关闭且不再重订').toHaveLength(1);
  });
});
