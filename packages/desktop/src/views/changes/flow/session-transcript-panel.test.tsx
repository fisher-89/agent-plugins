// @vitest-environment jsdom
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { AskAnswerActions } from '@/components/agent';
import type * as AgentSessionViewModule from '@/components/agent/agent-session-view';

import type { AgentEvent, SessionSummary, TurnSummary } from '../../../types/dto';
import { SessionTranscriptPanel } from './session-transcript-panel';
import type { RoleSessionRef } from './types';

// ---------------------------------------------------------------------------
// 进程边界 Mock（Mock策略表）：invoke 按命令名分发并可计数（session_detail /
// agent_session_transcript / agent_session_watch），Channel mock 捕获 onmessage
// 供直投事件与 Record；`useAgentSession` / `AgentMessages` 真实组合（SessionMeta
// 由组件提供，本文件不再自定义），`AgentSessionView` 保留真实实现、只在其边界挂
// props 探针（见下）。
// ---------------------------------------------------------------------------

const { ChannelMock, invokeMock } = vi.hoisted(() => {
  class ChannelMock {
    public onmessage: ((message: unknown) => void) | null = null;
    public static instances: ChannelMock[] = [];
    constructor() {
      ChannelMock.instances.push(this);
    }
  }
  return { ChannelMock, invokeMock: vi.fn() };
});

vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock, Channel: ChannelMock }));

// ---------------------------------------------------------------------------
// 探针装置（保留真实实现，仅记录 props）：在 `AgentSessionView` 边界把每次渲染
// 实收的 props push 进 `propsLog` 后原样渲染真实组件——「注入对象是否原样到位」
// 只能由引用相等判定（活卡呈现与调用次数反推不出引用相等）。`importOriginal`
// 保留全部真实实现（活卡 / 判活 / 气泡照旧），探针零行为、只记录。
// ---------------------------------------------------------------------------

const { propsLog } = vi.hoisted(() => ({ propsLog: [] as unknown[] }));

vi.mock('@/components/agent/agent-session-view', async (importOriginal) => {
  const actual = await importOriginal<typeof AgentSessionViewModule>();
  function ProbedAgentSessionView(props: Parameters<typeof actual.AgentSessionView>[0]) {
    propsLog.push(props);
    return <actual.AgentSessionView {...props} />;
  }
  return { ...actual, AgentSessionView: ProbedAgentSessionView };
});

interface RecordedViewProps {
  root?: string | null;
  sessionId?: string | null;
  refreshKey?: number;
  askAnswer?: AskAnswerActions;
}

/** 探针实收的 props 全集（每次渲染一条，按发生序；空 = 下游从未渲染）。 */
function recordedViewProps(): RecordedViewProps[] {
  return propsLog as RecordedViewProps[];
}

/** 最近一次渲染实收的 props。 */
function lastViewProps(): RecordedViewProps {
  const props = recordedViewProps().at(-1);
  if (props === undefined) throw new Error('探针未捕获 AgentSessionView 渲染');
  return props;
}

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
  propsLog.length = 0;
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

// ---------------------------------------------------------------------------
// ask-tool-in-flow：`askAnswer` 透传（本面板唯一例外动作面）
// ---------------------------------------------------------------------------

/** ask 工具调用消息（活卡判据面）。 */
function askMessage(seq: number): AgentEvent {
  return {
    seq,
    timestampMs: TS,
    kind: 'message',
    role: 'assistant',
    blocks: [
      {
        kind: 'toolUse',
        id: 'call_flow_1',
        name: 'AskUserQuestion',
        input: { question: '回溯到哪？', options: ['proposal', 'dev-design'] },
      },
    ],
    parentToolUseId: null,
  };
}

/** 同一会话转录内两条 ask 工具调用（`call_1` / `call_2`，均无结果态）。 */
function twoAskMessage(seq: number): AgentEvent {
  return {
    seq,
    timestampMs: TS,
    kind: 'message',
    role: 'assistant',
    blocks: [
      {
        kind: 'toolUse',
        id: 'call_1',
        name: 'AskUserQuestion',
        input: { question: '退回哪个相位？', options: ['proposal'] },
      },
      {
        kind: 'toolUse',
        id: 'call_2',
        name: 'AskUserQuestion',
        input: { question: '采用哪个实现方案？', options: ['方案甲', '方案乙'] },
      },
    ],
    parentToolUseId: null,
  };
}

/** 传 askAnswer 的面板挂载面。 */
function renderPanelWithAsk(
  askAnswer: { onAnswer: (text: string) => void; pendingToolCallId?: string | null },
  refs: RoleSessionRef | null = roleRef(),
) {
  return render(
    <SessionTranscriptPanel
      root={ROOT}
      roleRef={refs}
      refreshKey={undefined}
      askAnswer={askAnswer}
    />,
  );
}

describe('SessionTranscriptPanel：ask 动作面透传（AC-13）', () => {
  it('传 askAnswer（精确 toolCallId）→ 该卡活卡可点即回调 onAnswer', async () => {
    resetIpc({ transcript: [askMessage(0)] });
    const onAnswer = vi.fn();
    const askAnswer = { onAnswer, pendingToolCallId: 'call_flow_1' };
    renderPanelWithAsk(askAnswer);
    await act(async () => {});
    await act(async () => {});

    const panel = screen.getByTestId('session-transcript-panel');
    const buttons = await within(panel).findAllByTestId('ask-answer-option');
    expect(buttons.map((node) => node.textContent)).toEqual(['proposal', 'dev-design']);

    // 探针判据（DD12「同一对象」）：下游收到的就是注入的那一个对象，零包装零重建
    const received = recordedViewProps();
    expect(received.length, '下游应已渲染').toBeGreaterThan(0);
    for (const props of received) {
      expect(props.askAnswer).toBe(askAnswer);
      expect(props.askAnswer?.onAnswer).toBe(onAnswer);
    }

    fireEvent.click(buttons[0] as HTMLElement);
    expect(onAnswer).toHaveBeenCalledWith('proposal');
  });

  it('两 ask 部件精确对账：pendingToolCallId 指向者活、另一张静态，切换后互为镜像', async () => {
    resetIpc({ transcript: [twoAskMessage(0)] });
    const onAnswer = vi.fn();
    const toSecond = { onAnswer, pendingToolCallId: 'call_2' };
    const { rerender } = render(
      <SessionTranscriptPanel
        root={ROOT}
        roleRef={roleRef()}
        refreshKey={undefined}
        askAnswer={toSecond}
      />,
    );
    await act(async () => {});
    await act(async () => {});

    const panel = screen.getByTestId('session-transcript-panel');
    await waitFor(() => expect(within(panel).getAllByTestId('ask-question-card')).toHaveLength(2));
    const [first, second] = within(panel).getAllByTestId('ask-question-card') as [
      HTMLElement,
      HTMLElement,
    ];

    // call_2 活卡：两条选项按钮 + 输入框；call_1 静态只读：单条只读行、零输入
    expect(
      within(second)
        .getAllByTestId('ask-answer-option')
        .map((node) => node.textContent),
    ).toEqual(['方案甲', '方案乙']);
    expect(within(second).queryAllByTestId('ask-answer-input')).toHaveLength(1);
    expect(within(first).queryAllByTestId('ask-answer-option')).toHaveLength(0);
    expect(within(first).queryAllByTestId('ask-option')).toHaveLength(1);

    // 探针判据：判活指针按值直通（对象同一、字段原值，未被重建）
    expect(lastViewProps().askAnswer).toBe(toSecond);
    expect(lastViewProps().askAnswer?.pendingToolCallId).toBe('call_2');

    // 点活卡选项 → onAnswer 恰一次且参数为 call_2 的选项文本（不串卡）
    fireEvent.click(within(second).getAllByTestId('ask-answer-option')[1] as HTMLElement);
    expect(onAnswer).toHaveBeenCalledTimes(1);
    expect(onAnswer).toHaveBeenCalledWith('方案乙');

    // 指针切到 call_1 → 两卡活性互为镜像
    const toFirst = { onAnswer, pendingToolCallId: 'call_1' };
    rerender(
      <SessionTranscriptPanel
        root={ROOT}
        roleRef={roleRef()}
        refreshKey={undefined}
        askAnswer={toFirst}
      />,
    );
    await act(async () => {});

    const [mirroredFirst, mirroredSecond] = within(panel).getAllByTestId('ask-question-card') as [
      HTMLElement,
      HTMLElement,
    ];
    expect(
      within(mirroredFirst)
        .getAllByTestId('ask-answer-option')
        .map((node) => node.textContent),
    ).toEqual(['proposal']);
    expect(within(mirroredSecond).queryAllByTestId('ask-answer-option')).toHaveLength(0);
    expect(within(mirroredSecond).queryAllByTestId('ask-option')).toHaveLength(2);
    expect(lastViewProps().askAnswer).toBe(toFirst);
    expect(lastViewProps().askAnswer?.pendingToolCallId).toBe('call_1');
  });

  it('askAnswer.pendingToolCallId = null → 全静态（历史重放零应答入口）', async () => {
    resetIpc({ transcript: [askMessage(0)] });
    const onAnswer = vi.fn();
    const askAnswer = { onAnswer, pendingToolCallId: null };
    renderPanelWithAsk(askAnswer);
    await act(async () => {});
    await act(async () => {});

    const panel = screen.getByTestId('session-transcript-panel');
    await waitFor(() => expect(within(panel).queryAllByTestId('ask-option')).toHaveLength(2));
    expect(within(panel).queryAllByTestId('ask-answer-option')).toHaveLength(0);
    // 探针判据：`null` 指针按值直通（非缺省键、非重建为新对象）
    expect(lastViewProps().askAnswer).toBe(askAnswer);
    expect(lastViewProps().askAnswer?.pendingToolCallId).toBeNull();
  });

  it('不传 askAnswer → 全静态且其余动作面仍缺席（无发送 / 停止入口）', async () => {
    resetIpc({ transcript: [askMessage(0)] });
    await mounted();

    const panel = screen.getByTestId('session-transcript-panel');
    await waitFor(() => expect(within(panel).queryAllByTestId('ask-option')).toHaveLength(2));
    expect(within(panel).queryAllByTestId('ask-answer-option')).toHaveLength(0);
    expect(screen.queryByTestId('explore-send')).toBeNull();
    expect(screen.queryByTestId('explore-stop')).toBeNull();
    expect(callsOf('agent_start')).toHaveLength(0);
  });

  it('空态（roleRef null）时 askAnswer 在场也不虚构会话（零取数）', async () => {
    resetIpc();
    const onAnswer = vi.fn();
    renderPanelWithAsk({ onAnswer }, null);
    await act(async () => {});

    expect(screen.getByTestId('drawer-session-empty').textContent).toContain('当前选中无关联会话');
    expect(invokeMock).not.toHaveBeenCalled();
    expect(recordedViewProps(), '空态不挂会话面（askAnswer 无处透传）').toHaveLength(0);
  });

  it('未注入即全静态：ask 卡零按钮零输入，双应答命令零 invoke（历史重放零应答入口）', async () => {
    resetIpc({ transcript: [askMessage(0)] });
    await mounted();

    const panel = screen.getByTestId('session-transcript-panel');
    await waitFor(() => expect(within(panel).queryAllByTestId('ask-option')).toHaveLength(2));
    expect(within(panel).queryAllByTestId('ask-answer-option')).toHaveLength(0);
    expect(within(panel).queryAllByTestId('ask-answer-input')).toHaveLength(0);
    expect(callsOf('change_flow_answer')).toHaveLength(0);
    expect(callsOf('agent_answer')).toHaveLength(0);
    // 探针判据：不注入即下游每条渲染都收不到动作面（`viewProps` 不含该键）
    expect(recordedViewProps().length).toBeGreaterThan(0);
    for (const props of recordedViewProps()) expect(props.askAnswer).toBeUndefined();
  });

  it('composer 仍不可注入：渲染面无发送输入框（只读纪律不因 ask 例外而松动）', async () => {
    resetIpc({ transcript: [askMessage(0)] });
    const onAnswer = vi.fn();
    renderPanelWithAsk({ onAnswer, pendingToolCallId: 'call_flow_1' });
    await act(async () => {});
    await act(async () => {});

    await within(screen.getByTestId('session-transcript-panel')).findAllByTestId(
      'ask-answer-option',
    );
    expect(screen.queryByTestId('explore-send')).toBeNull();
    expect(screen.queryByTestId('explore-prompt')).toBeNull();
    expect(callsOf('agent_start')).toHaveLength(0);
  });

  it('既有 viewProps 与 data-testid 不因新增键改变：注入与不注入两形态逐条相等', async () => {
    resetIpc({ transcript: [askMessage(0)] });
    const askAnswer = { onAnswer: vi.fn(), pendingToolCallId: 'call_flow_1' };
    const withAsk = renderPanelWithAsk(askAnswer);
    await act(async () => {});
    await act(async () => {});
    const injectedPanel = screen.getByTestId('session-transcript-panel');
    // 探针判据：新增键只在 `askAnswer` 一位上出现，其余三项按值原样
    expect(lastViewProps().root).toBe(ROOT);
    expect(lastViewProps().sessionId).toBe(EXECUTOR_SESSION);
    expect(lastViewProps().refreshKey).toBeUndefined();
    expect(lastViewProps().askAnswer).toBe(askAnswer);
    const injectedArgs = {
      detail: callsOf('session_detail').length,
      transcript: callsOf('agent_session_transcript').length,
      role: injectedPanel
        .querySelector('[data-transcript-role]')
        ?.getAttribute('data-transcript-role'),
      transcriptTestId: within(injectedPanel)
        .getByTestId('session-transcript')
        .getAttribute('data-testid'),
    };
    withAsk.unmount();

    resetIpc({ transcript: [askMessage(0)] });
    await mounted();
    const plainPanel = screen.getByTestId('session-transcript-panel');
    expect(lastViewProps().root).toBe(ROOT);
    expect(lastViewProps().sessionId).toBe(EXECUTOR_SESSION);
    expect(lastViewProps().refreshKey).toBeUndefined();
    expect({
      detail: callsOf('session_detail').length,
      transcript: callsOf('agent_session_transcript').length,
      role: plainPanel
        .querySelector('[data-transcript-role]')
        ?.getAttribute('data-transcript-role'),
      transcriptTestId: within(plainPanel)
        .getByTestId('session-transcript')
        .getAttribute('data-testid'),
    }).toEqual(injectedArgs);
  });

  it('roleRef.sessionId = null：仍走空态分支，askAnswer 在场也不渲染卡', async () => {
    resetIpc();
    const onAnswer = vi.fn();
    renderPanelWithAsk({ onAnswer }, { role: 'executor', sessionId: null });
    await act(async () => {});

    expect(screen.getByTestId('drawer-session-empty').textContent).toContain('此步无会话记录');
    expect(screen.queryAllByTestId('ask-answer-option')).toHaveLength(0);
    expect(invokeMock).not.toHaveBeenCalled();
    expect(recordedViewProps(), '空态不挂会话面（askAnswer 无处透传）').toHaveLength(0);
  });
});
