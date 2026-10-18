// @vitest-environment jsdom
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { AgentEvent, SessionSummary, TurnSummary } from '../../../types/dto';
import type { AgentRunHistoryState } from '../hooks/use-agent-run-history';
import { AgentRunHistory } from './agent-run-history';

// ---------------------------------------------------------------------------
// 进程边界 Mock（Mock策略表）：invoke 按命令名分发（agent_sessions 分页应答、
// session_detail / agent_session_transcript 可编程含 reject 并计数、
// agent_session_watch resolve / 挂起），Channel mock 捕获 onmessage 供重放区
// 自持形态直投信封。清单态由 `state` fixture 直传；重放区（会话级组件自持
// 形态）经真实 IPC 取数与订阅——`useAgentRunHistory` / `AgentSessionView` /
// `AgentMessages` 全部真实组合。
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

const TS = 1727000000000;

function turn(turnId: number, status: TurnSummary['status']): TurnSummary {
  return {
    turnId,
    sessionId: `ses-${turnId}-1727000000000`,
    status,
    startedAt: TS + turnId,
    finishedAt: status === 'running' ? null : TS + turnId + 999,
    numTurns: 4,
    costUsd: 0.2,
    durationMs: 800,
    error: status === 'failed' ? '进程结束但未产出收敛事件' : null,
  };
}

/** store `SessionConfigSnapshot` 快照线形态（serde camelCase；v2 增
 * `agentInstanceId` 实例外键——fixture 缺键即类型红，防漂移）。 */
interface ConfigSnapshotFixture {
  engine: 'cli' | 'sdk';
  agentInstanceId: number | null;
  model: string | null;
  permissionMode: string;
}

function session(
  id: string,
  status: TurnSummary['status'],
  updatedAt: number,
  turns: TurnSummary[] = [turn(1, status)],
): SessionSummary {
  return {
    row: {
      id,
      remoteSessionId: `engine-${id}`,
      configSnapshot: {
        engine: 'sdk',
        agentInstanceId: 7,
        model: 'glm-high',
        permissionMode: 'bypassPermissions',
      } satisfies ConfigSnapshotFixture,
      provenance: { source: 'debug', sourceRef: null },
      createdAt: TS,
      updatedAt,
    },
    stats: { turnCount: turns.length, totalDurationMs: 800, inputTokens: null, outputTokens: null },
    turns,
  };
}

function historyState(overrides: Partial<AgentRunHistoryState> = {}): AgentRunHistoryState {
  const sessions = overrides.sessions ?? [];
  return {
    selectedSessionId: null,
    loading: false,
    error: null,
    // 缺省由 sessions 推导（空态口径看 total，非当前页长度）
    total: sessions.length,
    page: 1,
    totalPages: 1,
    refresh: vi.fn(),
    setPage: vi.fn(),
    nextPage: vi.fn(),
    prevPage: vi.fn(),
    openSession: vi.fn(),
    closeSession: vi.fn(),
    ...overrides,
    sessions,
  };
}

/** workspace root（重放会话取数 / 订阅寻址） */
const ROOT = 'C:demo\beta';

function mount(state: AgentRunHistoryState) {
  return render(
    <AgentRunHistory root={ROOT} state={state} source="debug" onSourceChange={() => {}} />,
  );
}

// ---------------------------------------------------------------------------
// 可编程 IPC（重放区自持形态的取数 / 订阅面）
// ---------------------------------------------------------------------------

/** 重放目标会话 id（点开即重放的全史转录寻址键） */
const REPLAY_ID = 'ses-replay-1-1727000000000';

interface IpcState {
  detail: SessionSummary | null | Error;
  transcript: AgentEvent[] | Error;
  /** 订阅命令应答：pending = 挂起（测试直投信封驱动）/ reject */
  watch: 'pending' | 'reject';
}

let ipc: IpcState;

function resetIpc(overrides: Partial<IpcState> = {}): void {
  ipc = {
    detail: session(REPLAY_ID, 'completed', TS + 1),
    transcript: [userEvent(0, '重放提问'), textEvent(1, '重放结论'), turnDoneEvent(2)],
    watch: 'pending',
    ...overrides,
  };
  ChannelMock.instances.length = 0;
  invokeMock.mockReset();
  invokeMock.mockImplementation((command: string) => {
    if (command === 'agent_sessions') return Promise.resolve({ sessions: [], total: 0 });
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
      return new Promise<null>(() => {});
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

function deliverRecord(record: TurnSummary): void {
  act(() => {
    lastChannel().onmessage?.({ ipc: 'record', record });
  });
}

function bubbleTexts(): string[] {
  return screen.queryAllByTestId('block-text').map((node) => node.textContent ?? '');
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

function deltaEvent(seq: number, text: string): AgentEvent {
  return {
    seq,
    timestampMs: TS,
    kind: 'messageDelta',
    parentToolUseId: null,
    delta: { kind: 'text', text },
  };
}

describe('AgentRunHistory：会话列表与触发（AC-5 / D13）', () => {
  it('sessions 渲染终态 / 轮数 / 时间三要素（会话行 = 最新轮终态 + 现算轮数）', () => {
    mount(
      historyState({
        sessions: [
          session('ses-1-1727000000000', 'completed', TS + 1),
          session('ses-2-1727000000000', 'failed', TS + 2),
        ],
        total: 2,
      }),
    );

    const rows = screen.getAllByTestId('agent-run-row');
    expect(rows).toHaveLength(2);
    const statuses = screen.getAllByTestId('run-status').map((node) => node.textContent);
    expect(statuses).toEqual(['已完成', '失败']);
    expect(rows[0]?.textContent).toContain('1 轮');
    // 时间以本地时区字符串呈现（非空即可，不绑定格式）
    expect((rows[0]?.textContent ?? '').length).toBeGreaterThan(0);
    expect(screen.getAllByTestId('run-row-error')).toHaveLength(1);
    expect(screen.getAllByTestId('run-row-error')[0]?.textContent).toContain(
      '进程结束但未产出收敛事件',
    );
    // 头行计数取过滤集全量 total（非当前页长度）
    expect(screen.getByTestId('agent-run-history').textContent).toContain('历史会话 (2)');
  });

  it('最新轮行 status 为 stopped 的会话行状态标签呈「已停止」，其余三态标签不回归', () => {
    mount(
      historyState({
        sessions: [
          session('ses-1-1727000000000', 'running', TS + 1),
          session('ses-2-1727000000000', 'completed', TS + 2),
          session('ses-3-1727000000000', 'stopped', TS + 3),
          session('ses-4-1727000000000', 'failed', TS + 4),
        ],
      }),
    );

    const statuses = screen.getAllByTestId('run-status').map((node) => node.textContent);
    expect(statuses).toEqual(['运行中', '已完成', '已停止', '失败']);
    const stoppedRow = screen
      .getAllByTestId('agent-run-row')
      .find((row) => row.getAttribute('data-status') === 'stopped');
    expect(stoppedRow?.getAttribute('data-session-id')).toBe('ses-3-1727000000000');
    // 徽章配色随终态可辨：completed 绿 / failed 红 / running 灰（三元链逐臂）
    const rows = screen.getAllByTestId('agent-run-row');
    const statusBadges = rows.map((row) => within(row).getByTestId('run-status'));
    expect(statusBadges[0]?.className).toContain('text-muted-foreground');
    expect(statusBadges[1]?.className).toContain('text-pass');
    expect(statusBadges[3]?.className).toContain('text-fail');
    expect(statusBadges[2]?.className).toContain('text-muted-foreground');
    expect(statusBadges[1]?.className).not.toContain('text-fail');
  });

  it('终态取自最新轮行（非首行轮）：completed 后接 failed 轮 → 行徽标随末轮呈失败', () => {
    const id = 'ses-8-1727000000000';
    const multi: SessionSummary = {
      ...session(id, 'failed', TS + 8, [turn(1, 'completed'), turn(2, 'failed')]),
      stats: { turnCount: 5, totalDurationMs: 800, inputTokens: null, outputTokens: null },
    };
    mount(historyState({ sessions: [multi] }));

    const row = screen.getByTestId('agent-run-row');
    expect(row.getAttribute('data-status')).toBe('failed');
    expect(within(row).getByTestId('run-status').textContent).toBe('失败');
    expect(within(row).getByTestId('run-status').className).toContain('text-fail');
    // 轮数取 stats.turnCount 现算值（非 turns 数组长度投影）
    expect(row.textContent).toContain('5 轮');
    // 末轮 error 随行呈现
    expect(within(row).getByTestId('run-row-error').textContent).toContain(
      '进程结束但未产出收敛事件',
    );
  });

  it('无轮行空会话：徽标「空会话」灰显、data-status empty、无错误行（空态词汇不缺省）', () => {
    const id = 'ses-9-1727000000000';
    mount(historyState({ sessions: [session(id, 'completed', TS + 9, [])] }));

    const row = screen.getByTestId('agent-run-row');
    expect(row.getAttribute('data-status')).toBe('empty');
    const badge = within(row).getByTestId('run-status');
    expect(badge.textContent).toBe('空会话');
    expect(badge.className).toContain('text-muted-foreground');
    expect(within(row).queryByTestId('run-row-error')).toBeNull();
  });

  it('点击会话行 → openSession(sessionId) 恰调用一次（重放触发半边）', () => {
    const openSession = vi.fn();
    const sessionId = 'ses-7-1727000000000';
    mount(historyState({ sessions: [session(sessionId, 'completed', TS + 7)], openSession }));

    fireEvent.click(screen.getByTestId('agent-run-row'));

    expect(openSession).toHaveBeenCalledTimes(1);
    expect(openSession).toHaveBeenCalledWith(sessionId);
  });

  it('点击刷新 → refresh() 恰调用一次（D13 显式触发）', () => {
    const refresh = vi.fn();
    mount(historyState({ sessions: [session('ses-1-1727000000000', 'completed', TS)], refresh }));

    fireEvent.click(screen.getByTestId('history-refresh'));

    expect(refresh).toHaveBeenCalledTimes(1);
  });

  it('selectedSessionId 匹配时呈现重放区（会话级组件自持形态，取数与折叠归组件）', async () => {
    const sessionId = 'ses-7-1727000000000';
    resetIpc({
      detail: session(sessionId, 'completed', TS + 7),
      transcript: [userEvent(0, '重放提问'), textEvent(1, '重放结论'), turnDoneEvent(2)],
    });
    mount(
      historyState({
        sessions: [session(sessionId, 'completed', TS + 7)],
        selectedSessionId: sessionId,
      }),
    );

    const replay = screen.getByTestId('replay-area');
    expect(within(replay).getByTestId('replay-area-session') !== null).toBe(true);
    // 自持形态装配：session_detail + agent_session_transcript 各一次
    await waitFor(() => expect(screen.getByTestId('session-meta-id').textContent).toBe(sessionId));
    expect(callsOf('session_detail')).toHaveLength(1);
    expect(callsOf('agent_session_transcript')).toHaveLength(1);
    expect(bubbleTexts()).toEqual(['重放提问', '重放结论']);
    // 完成态零订阅（点开即重放，不带流）
    expect(callsOf('agent_session_watch')).toHaveLength(0);
  });
});

describe('AgentRunHistory：空态 / loading / error（边界与异常）', () => {
  it('sessions 为空时呈现空态文案不崩', () => {
    mount(historyState());

    expect(screen.getByTestId('history-empty') !== null).toBe(true);
    expect(screen.queryAllByTestId('agent-run-row')).toHaveLength(0);
  });

  it('loading=true 呈现加载态', () => {
    const refresh = vi.fn();
    mount(historyState({ loading: true, refresh }));

    expect(screen.getByTestId('history-loading') !== null).toBe(true);
    // 取数进行中刷新入口禁用（disabled 随 loading）
    expect(screen.getByTestId('history-refresh')).toHaveProperty('disabled', true);
    // 无选中会话时不渲染重放区（selectedSessionId null 不越权呈现）
    expect(screen.queryByTestId('replay-area')).toBeNull();
    // 加载中不出空态文案、不出错误条（空态三条件随状态收口）
    expect(screen.queryByTestId('history-empty')).toBeNull();
    expect(screen.queryByTestId('history-error')).toBeNull();
  });

  it('error 非空呈现错误条；列表仍可用（行照常渲染）', () => {
    const refresh = vi.fn();
    mount(
      historyState({
        sessions: [session('ses-1-1727000000000', 'completed', TS)],
        error: 'db: 转录读取失败',
        refresh,
      }),
    );

    expect(screen.getByTestId('history-error').textContent).toContain('db: 转录读取失败');
    expect(screen.getAllByTestId('agent-run-row')).toHaveLength(1);
    // 错误态下刷新入口仍可触发
    fireEvent.click(screen.getByTestId('history-refresh'));
    expect(refresh).toHaveBeenCalledTimes(1);
  });

  it('error 非空且清单为空：错误条呈现、空态文案不并存（空态被 error 门禁收口）', () => {
    mount(historyState({ sessions: [], error: 'db: 清单打开失败' }));

    expect(screen.getByTestId('history-error') !== null).toBe(true);
    expect(screen.queryByTestId('history-empty')).toBeNull();
    expect(screen.queryByTestId('history-loading')).toBeNull();
  });

  it('error 为 null：错误条缺席（无错不渲染错误横幅）', () => {
    mount(historyState({ sessions: [session('ses-1-1727000000000', 'completed', TS)] }));

    expect(screen.queryByTestId('history-error')).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// 来源筛选三态按钮组（desktop-change-session-visibility，AC-3）：头行
// debug / change / 全部三枚按钮（data-testid 挂钩 + data-source + aria-pressed
// 高亮随 props 切换——组件无状态，状态提升断言面）。
// ---------------------------------------------------------------------------

describe('AgentRunHistory：来源筛选三态按钮组（AC-3）', () => {
  it("source='debug' 挂载：history-source-filter 内三枚按钮 data-source 为 debug / change / all，debug 枚 aria-pressed 高亮", () => {
    mount(historyState());

    const filter = screen.getByTestId('history-source-filter');
    const buttons = within(filter).getAllByRole('button');
    expect(buttons.map((button) => button.getAttribute('data-source'))).toEqual([
      'debug',
      'change',
      'all',
    ]);
    expect(buttons.map((button) => button.textContent)).toEqual(['调试', '变更', '全部']);
    expect(buttons.map((button) => button.getAttribute('aria-pressed'))).toEqual([
      'true',
      'false',
      'false',
    ]);
    // 高亮态着色随 aria-pressed：选中项无灰化类，未选中项带灰化类（三元两臂）
    expect(buttons[0]?.className).not.toContain('bg-muted');
    expect(buttons[1]?.className).toContain('bg-muted');
    expect(buttons[2]?.className).toContain('bg-muted');
  });

  it("点击 change 枚 → onSourceChange('change') 恰调用一次（组件无状态——状态提升断言面）", () => {
    const onSourceChange = vi.fn();
    render(
      <AgentRunHistory
        root={ROOT}
        state={historyState()}
        source="debug"
        onSourceChange={onSourceChange}
      />,
    );

    const buttons = within(screen.getByTestId('history-source-filter')).getAllByRole('button');
    fireEvent.click(buttons[1]);

    expect(onSourceChange).toHaveBeenCalledTimes(1);
    expect(onSourceChange).toHaveBeenCalledWith('change');
  });

  it("source='all' 挂载：all 枚 aria-pressed 高亮（高亮随 props 切换，组件不自持状态）", () => {
    render(
      <AgentRunHistory root={ROOT} state={historyState()} source="all" onSourceChange={() => {}} />,
    );

    const pressed = within(screen.getByTestId('history-source-filter'))
      .getAllByRole('button')
      .map((button) => button.getAttribute('aria-pressed'));
    expect(pressed).toEqual(['false', 'false', 'true']);
  });

  it('筛选按钮组与列表 / 刷新 / 重放区并存：挂载头行既含筛选又含刷新（布局零改动回归）', () => {
    mount(
      historyState({
        sessions: [session('ses-1-1727000000000', 'completed', TS)],
      }),
    );

    expect(screen.getByTestId('history-source-filter') !== null).toBe(true);
    expect(screen.getByTestId('history-refresh') !== null).toBe(true);
    expect(screen.getAllByTestId('agent-run-row')).toHaveLength(1);
    // 清单非空时不出空态文案（空态仅限零清单）
    expect(screen.queryByTestId('history-empty')).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// 分页器接入（AC：totalPages > 1 时呈通用分页器，翻页回调 setPage）
// ---------------------------------------------------------------------------

describe('AgentRunHistory：分页器接入', () => {
  it('totalPages > 1 呈分页器，摘要行携全量 total 与真实页界', () => {
    mount(
      historyState({
        sessions: [session('ses-1-1727000000000', 'completed', TS)],
        total: 45,
        page: 2,
        totalPages: 3,
      }),
    );

    const pager = screen.getByTestId('agent-history-pagination');
    expect(pager).toBeTruthy();
    expect(pager.textContent).toContain('第 2 / 3 页（共 45 条）');
    expect(screen.getByRole('button', { name: '下一页' })).toBeTruthy();
  });

  it('点下一页 → nextPage；点页码 → setPage 携目标页', () => {
    const setPage = vi.fn<(page: number) => void>();
    const nextPage = vi.fn();
    mount(
      historyState({
        sessions: [session('ses-1-1727000000000', 'completed', TS)],
        total: 45,
        page: 2,
        totalPages: 3,
        setPage,
        nextPage,
      }),
    );

    // 通用分页器下一页回调 onPageChange(page+1) → 经 hook 的 setPage 落点
    fireEvent.click(screen.getByRole('button', { name: '下一页' }));
    expect(setPage).toHaveBeenCalledWith(3);

    fireEvent.click(screen.getByRole('button', { name: '第 3 页' }));
    expect(setPage).toHaveBeenLastCalledWith(3);
  });

  it('边界：totalPages=1 不渲染分页器（可见性由调用方把关）', () => {
    mount(historyState({ total: 3, page: 1, totalPages: 1 }));
    expect(screen.queryByTestId('agent-history-pagination')).toBeNull();
  });

  it('loading 期分页器 disabled（翻页不与取数竞跑）', () => {
    mount(
      historyState({
        sessions: [session('ses-1-1727000000000', 'completed', TS)],
        total: 45,
        page: 1,
        totalPages: 3,
        loading: true,
      }),
    );
    for (const button of within(screen.getByTestId('agent-history-pagination')).getAllByRole(
      'button',
    )) {
      expect(button.hasAttribute('disabled')).toBe(true);
    }
  });

  it('头行计数取 total 而非当前页长度（多页仅呈本页行数）', () => {
    mount(
      historyState({
        sessions: [session('ses-1-1727000000000', 'completed', TS)],
        total: 45,
        page: 1,
        totalPages: 3,
      }),
    );
    expect(screen.getByTestId('agent-run-history').textContent).toContain('历史会话 (45)');
    expect(screen.getAllByTestId('agent-run-row')).toHaveLength(1);
  });

  it('边界：total=0 空态呈文案且不渲染分页器（越界页不闪空态另由 total 兜住）', () => {
    mount(historyState({ total: 0, totalPages: 1 }));
    expect(screen.getByTestId('history-empty')).toBeTruthy();
    expect(screen.queryByTestId('agent-history-pagination')).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// configSnapshot fixture 补 agentInstanceId 键（fixture 随动，AC-7）
// ---------------------------------------------------------------------------

describe('AgentRunHistory：configSnapshot fixture 携 agentInstanceId 键（AC-7 随动）', () => {
  it('补键后清单呈现既有断言保绿（多元素清单 + 两形态快照同口径）', () => {
    const legacy = session('ses-3-1727000000000', 'completed', TS + 3);
    legacy.row.configSnapshot = {
      engine: 'cli',
      agentInstanceId: null,
      model: null,
      permissionMode: 'bypassPermissions',
    } satisfies ConfigSnapshotFixture;
    mount(
      historyState({
        sessions: [
          session('ses-1-1727000000000', 'completed', TS + 1),
          session('ses-2-1727000000000', 'failed', TS + 2),
          legacy,
        ],
      }),
    );

    // 呈现断言容忍两形态：具体实例 id（新会话语义）与 null（存量缺列语义）；
    // fixture 补键后既有行渲染断言保持
    const rows = screen.getAllByTestId('agent-run-row');
    expect(rows).toHaveLength(3);
    expect(screen.getAllByTestId('run-status').map((node) => node.textContent)).toEqual([
      '已完成',
      '失败',
      '已完成',
    ]);
    expect(screen.getByTestId('agent-run-history').textContent).toContain('历史会话 (3)');
  });
});

// ---------------------------------------------------------------------------
// 只读重放（自持形态）的实时面与异常面（D14）：订阅流 token 级增量 / 查询失败
// 落组件内错误提示 / root 缺席时重放区不越权渲染。
// ---------------------------------------------------------------------------

describe('AgentRunHistory：重放区订阅流与查询失败面（D14）', () => {
  it('运行中会话重放 → 订阅流可见 token 级增量（只读旁观、无动作面）；收口后零重订', async () => {
    resetIpc({
      detail: session(REPLAY_ID, 'running', TS + 1),
      transcript: [userEvent(0, '重放提问')],
    });
    mount(
      historyState({
        sessions: [session(REPLAY_ID, 'running', TS + 1)],
        selectedSessionId: REPLAY_ID,
      }),
    );

    // 运行中库态 → 恰一条订阅在挂
    await waitFor(() => expect(callsOf('agent_session_watch')).toHaveLength(1));
    const replay = screen.getByTestId('replay-area');
    // 只读旁观：重放区没有任何动作面（发送 / 停止 / 输入）
    expect(within(replay).queryByTestId('explore-composer')).toBeNull();
    expect(within(replay).queryByTestId('explore-send')).toBeNull();
    expect(within(replay).queryByTestId('explore-stop')).toBeNull();
    expect(within(replay).queryByTestId('explore-prompt')).toBeNull();
    expect(callsOf('agent_stop')).toHaveLength(0);

    // token 级增量直投 → 只读重放区呈现累积文本
    deliverEvent(deltaEvent(3, '重放流增量正文'));
    await waitFor(() => expect(bubbleTexts()).toContain('重放流增量正文'));

    // 收口：库内转录补齐 + 终态轮行回流 → 元信息转「已收口」，订阅不重订
    ipc.detail = session(REPLAY_ID, 'completed', TS + 2, [turn(1, 'completed')]);
    ipc.transcript = [userEvent(0, '重放提问'), textEvent(1, '重放结论'), turnDoneEvent(2)];
    deliverRecord({ ...turn(1, 'completed'), sessionId: REPLAY_ID });

    await waitFor(() =>
      expect(screen.getByTestId('session-meta-status').textContent).toBe('已收口'),
    );
    expect(callsOf('agent_session_watch'), '收口后零重订').toHaveLength(1);
  });

  it('重放区查询 reject → 重放区内错误提示呈现，清单不塌、不抛错（无伪气泡、零订阅）', async () => {
    resetIpc({ detail: new Error('db: 会话查询失败') });
    mount(
      historyState({
        sessions: [session(REPLAY_ID, 'completed', TS + 1)],
        selectedSessionId: REPLAY_ID,
      }),
    );

    await waitFor(() =>
      expect(screen.getByTestId('agent-session-error').textContent).toContain('会话查询失败'),
    );
    // 错误提示落在重放区内（会话级组件错误面），不虚构会话元信息与气泡
    const replay = screen.getByTestId('replay-area');
    expect(within(replay).getByTestId('agent-session-error')).toBeTruthy();
    expect(screen.queryByTestId('session-meta')).toBeNull();
    expect(bubbleTexts()).toEqual([]);
    // 清单不塌：行与头行计数仍在，且无清单级错误条（错误面不越权上抛）
    expect(screen.getAllByTestId('agent-run-row')).toHaveLength(1);
    expect(screen.getByTestId('agent-run-history').textContent).toContain('历史会话 (1)');
    expect(screen.queryByTestId('history-error')).toBeNull();
    expect(callsOf('agent_session_watch')).toHaveLength(0);
  });

  it('边界：root 为 null → 清单空态 + 重放区缺席（零会话查询，不越权订阅）', async () => {
    render(
      <AgentRunHistory
        root={null}
        state={historyState()}
        source="debug"
        onSourceChange={() => {}}
      />,
    );
    await act(async () => {});

    expect(screen.getByTestId('history-empty')).toBeTruthy();
    expect(screen.queryAllByTestId('agent-run-row')).toHaveLength(0);
    expect(screen.queryByTestId('replay-area')).toBeNull();
    expect(screen.queryByTestId('replay-area-session')).toBeNull();
    expect(invokeMock, 'root 缺席 → 零会话查询').not.toHaveBeenCalled();
  });
});
