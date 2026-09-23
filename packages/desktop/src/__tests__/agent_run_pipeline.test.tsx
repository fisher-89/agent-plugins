// @vitest-environment jsdom
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { AgentEvent, AgentRunRecord } from '../types/dto';
import { AgentDebugView } from '../views/agent/AgentDebugView';

// ---------------------------------------------------------------------------
// 集成关系三/四：AgentRunForm 启动 → useAgentRun → agent_start 实时流 →
// AgentEventTimeline 呈现；历史重放链 agent_runs / agent_run_events →
// AgentRunHistory → 时间线还原（D11/D12/D13）。
//
// 进程边界 Mock：invoke 按命令名分发；Channel mock 为可编程 class（捕获
// onmessage、测试内手动喂事件）——链路内组件/hooks/DTO 全部真实实现。
// ---------------------------------------------------------------------------

const { ChannelMock, invokeMock } = vi.hoisted(() => {
  class ChannelMock {
    onmessage: ((event: unknown) => void) | null = null;
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

const ROOT = 'C:\\demo\\beta';

function runStarted(seq: number): AgentEvent {
  return {
    seq,
    timestampMs: 1727000000000,
    kind: 'runStarted',
    model: 'claude-opus',
    sessionId: 's-1',
    tools: ['Bash'],
    mcpServers: [],
  };
}

function assistantToolUse(seq: number): AgentEvent {
  return {
    seq,
    timestampMs: 1727000000001,
    kind: 'message',
    role: 'assistant',
    blocks: [{ kind: 'toolUse', id: 'tu_1', name: 'Bash', input: { command: 'ls' } }],
    parentToolUseId: null,
  };
}

function subagentMessage(seq: number): AgentEvent {
  return {
    seq,
    timestampMs: 1727000000002,
    kind: 'message',
    role: 'assistant',
    blocks: [{ kind: 'text', text: '子代理产出' }],
    parentToolUseId: 'tu_1',
  };
}

function toolResult(seq: number): AgentEvent {
  return {
    seq,
    timestampMs: 1727000000003,
    kind: 'message',
    role: 'user',
    blocks: [{ kind: 'toolResult', id: 'tu_1', content: '目录内容', isError: false }],
    parentToolUseId: null,
  };
}

function runResult(seq: number, isError = false): AgentEvent {
  return {
    seq,
    timestampMs: 1727000000004,
    kind: 'runResult',
    subtype: isError ? 'error_max_turns' : 'success',
    isError,
    numTurns: 3,
    durationMs: 1234,
    costUsd: 0.5,
    usage: {},
    sessionId: 's-1',
  };
}

function rawEvent(seq: number): AgentEvent {
  return {
    seq,
    timestampMs: 1727000000005,
    kind: 'raw',
    eventType: 'mystery',
    rawJson: '{"type":"mystery"}',
  };
}

function record(status: AgentRunRecord['status'], id = 1): AgentRunRecord {
  return {
    id,
    prompt: '帮我跑一轮',
    cwd: ROOT,
    env: 'default',
    permissionMode: 'bypassPermissions',
    status,
    startedAt: 1727000000000,
    finishedAt: 1727000001000,
    numTurns: 3,
    costUsd: 0.5,
    durationMs: 1234,
    sessionId: 's-1',
    error: null,
  };
}

/** agent_start 悬挂控制：测试内决定 resolve / reject 时机。 */
let resolveStart: ((record: AgentRunRecord) => void) | null = null;
let rejectStart: ((reason: string) => void) | null = null;

let historyRuns: AgentRunRecord[] | string;
let replayEvents: AgentEvent[] | string;

function mockIpc() {
  resolveStart = null;
  rejectStart = null;
  historyRuns = [record('completed', 3), record('failed', 2)];
  replayEvents = [runStarted(0), assistantToolUse(1), rawEvent(2), runResult(3)];
  invokeMock.mockImplementation((command: string) => {
    if (command === 'agent_start') {
      return new Promise<AgentRunRecord>((resolve, reject) => {
        resolveStart = resolve;
        rejectStart = reject;
      });
    }
    if (command === 'agent_runs') {
      return typeof historyRuns === 'string'
        ? Promise.reject(historyRuns)
        : Promise.resolve(historyRuns);
    }
    if (command === 'agent_run_events') {
      return typeof replayEvents === 'string'
        ? Promise.reject(replayEvents)
        : Promise.resolve(replayEvents);
    }
    return Promise.resolve(null);
  });
}

/** Channel mock 实例形状（vi.hoisted 内 class 不外泄类型，取其结构）。 */
interface ChannelLike {
  onmessage: ((event: unknown) => void) | null;
}

function lastChannel(): ChannelLike {
  const instance = ChannelMock.instances.at(-1);
  if (!instance) throw new Error('start 未创建 Channel');
  return instance;
}

function startCallArgs(): {
  onEvent: unknown;
  root: unknown;
  prompt: unknown;
  env: unknown;
  permissionMode: unknown;
} {
  const call = invokeMock.mock.calls.find(([name]) => name === 'agent_start');
  if (!call) throw new Error('agent_start 未被调用');
  const args = call[1];
  if (typeof args !== 'object' || args === null) throw new Error('agent_start 无参数');
  return args as {
    onEvent: unknown;
    root: unknown;
    prompt: unknown;
    env: unknown;
    permissionMode: unknown;
  };
}

function countOf(command: string): number {
  return invokeMock.mock.calls.filter(([name]) => name === command).length;
}

async function launch(prompt = '帮我跑一轮') {
  fireEvent.change(screen.getByTestId('agent-prompt'), { target: { value: prompt } });
  await act(async () => {
    fireEvent.click(screen.getByTestId('agent-start'));
  });
}

beforeEach(() => {
  invokeMock.mockReset();
  ChannelMock.instances.length = 0;
  mockIpc();
});

// ---------------------------------------------------------------------------
// 场景一：启动参数契约与实时 loop 呈现（表单 → hook → Channel → 时间线）
// ---------------------------------------------------------------------------

describe('agent_run_pipeline：表单启动 → agent_start → 实时时间线', () => {
  it('表单启动 → invoke 参数逐字符合契约（默认 env/permissionMode 随链路上抛，IPC 键 camelCase）', async () => {
    render(<AgentDebugView root={ROOT} />);

    await launch();

    expect(startCallArgs()).toEqual({
      onEvent: expect.any(ChannelMock),
      root: ROOT,
      prompt: '帮我跑一轮',
      env: 'default',
      permissionMode: 'bypassPermissions',
    });
  });

  it('Channel 依次喂 runStarted → message(toolUse) → message(toolResult) → runResult：时间线实时呈现对话流、成对折叠块与汇总卡', async () => {
    render(<AgentDebugView root={ROOT} />);
    await launch();

    act(() => {
      lastChannel().onmessage?.(runStarted(0));
    });
    expect(screen.getByTestId('event-run-started') !== null).toBe(true);

    act(() => {
      lastChannel().onmessage?.(assistantToolUse(1));
    });
    act(() => {
      lastChannel().onmessage?.(toolResult(2));
    });
    expect(screen.getByTestId('block-tool-use').getAttribute('data-tool-id')).toBe('tu_1');
    expect(screen.getByTestId('block-tool-result').getAttribute('data-tool-id')).toBe('tu_1');

    // resolve 记录：running=false、表单解禁、汇总卡在等公民位
    await act(async () => {
      resolveStart?.(record('completed'));
    });
    act(() => {
      lastChannel().onmessage?.(runResult(3));
    });

    const card = screen.getByTestId('event-result');
    expect(card.getAttribute('data-is-error')).toBe('false');
    expect(screen.getByTestId('result-num-turns').textContent).toBe('3');
    // 表单解禁：启动按钮不再带 disabled 属性
    expect(screen.getByTestId('agent-start').hasAttribute('disabled')).toBe(false);
    expect(within(card).getByTestId('copy-value') !== null).toBe(true);
  });

  it('invoke reject(string)：error 呈现、running 复位、表单可重试', async () => {
    render(<AgentDebugView root={ROOT} />);
    await launch();

    await act(async () => {
      rejectStart?.('CLI 未找到');
    });
    await waitFor(() =>
      expect(screen.getByTestId('run-error').textContent).toContain('CLI 未找到'),
    );

    expect(screen.getByTestId('agent-start').hasAttribute('disabled')).toBe(false);
    // 可重试：再次点击发起第二次 invoke
    await act(async () => {
      fireEvent.click(screen.getByTestId('agent-start'));
    });
    await waitFor(() => expect(countOf('agent_start')).toBe(2));
  });

  it('runResult is_error=true（in-band 失败）→ 呈现 failed 终态而非成功汇总', async () => {
    render(<AgentDebugView root={ROOT} />);
    await launch();

    act(() => {
      lastChannel().onmessage?.(runResult(0, true));
    });
    await act(async () => {
      resolveStart?.(record('failed'));
    });

    const card = screen.getByTestId('event-result');
    expect(card.getAttribute('data-is-error')).toBe('true');
    expect(card.textContent).toContain('失败');
  });

  it('子代理事件（parentToolUseId 有值）喂入 → 时间线按子代理归因分组呈现', async () => {
    render(<AgentDebugView root={ROOT} />);
    await launch();

    act(() => {
      lastChannel().onmessage?.(runStarted(0));
      lastChannel().onmessage?.(assistantToolUse(1));
      lastChannel().onmessage?.(subagentMessage(2));
    });

    const group = screen.getByTestId('subagent-group');
    expect(group.getAttribute('data-parent-id')).toBe('tu_1');
    expect(group.textContent).toContain('子代理产出');
  });

  it('root=null 时页面自呈现「请先选择 workspace」且点击链路无从发起（不 invoke）', () => {
    render(<AgentDebugView root={null} />);

    expect(screen.getByTestId('agent-no-root') !== null).toBe(true);
    expect(screen.queryByTestId('agent-start')).toBeNull();
    expect(countOf('agent_start')).toBe(0);
  });
});

// ---------------------------------------------------------------------------
// 场景二：两段式取数还原时间线（agent_runs → AgentRunHistory → 点开重放）
// ---------------------------------------------------------------------------

describe('agent_run_pipeline：历史重放链 → 共用时间线还原', () => {
  it('挂载不自动取数（D13：run 结束不自动刷新，刷新仅显式动作）', () => {
    render(<AgentDebugView root={ROOT} />);

    expect(countOf('agent_runs')).toBe(0);
    expect(screen.getByTestId('history-empty') !== null).toBe(true);
  });

  it('refresh → agent_runs 列表呈现状态/时间/摘要；点开 → agent_run_events { runId } → 共用时间线还原（含 Raw 与实时路径同形态）', async () => {
    render(<AgentDebugView root={ROOT} />);

    await act(async () => {
      fireEvent.click(screen.getByTestId('history-refresh'));
    });
    await waitFor(() => expect(screen.getAllByTestId('agent-run-row')).toHaveLength(2));
    const statuses = screen.getAllByTestId('run-status').map((node) => node.textContent);
    expect(statuses).toEqual(['已完成', '失败']);

    await act(async () => {
      fireEvent.click(screen.getAllByTestId('agent-run-row')[0]);
    });
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith('agent_run_events', { runId: 3 }));
    const replay = screen.getByTestId('replay-area');
    expect(replay.getAttribute('data-selected-run-id')).toBe('3');
    // 重放与实时走同一渲染面：runStarted 行 + Raw 透传占位同形态
    expect(within(replay).getByTestId('event-run-started') !== null).toBe(true);
    expect(within(replay).getByTestId('event-raw').textContent).toContain('mystery');
  });

  it('agent_runs 返回 [] → 空态、无 agent_run_events 发生', async () => {
    historyRuns = [];
    render(<AgentDebugView root={ROOT} />);

    await act(async () => {
      fireEvent.click(screen.getByTestId('history-refresh'));
    });
    await waitFor(() => expect(screen.getByTestId('history-empty') !== null).toBe(true));

    expect(countOf('agent_run_events')).toBe(0);
  });

  it('agent_run_events reject → error 呈现、列表仍可用', async () => {
    render(<AgentDebugView root={ROOT} />);
    await act(async () => {
      fireEvent.click(screen.getByTestId('history-refresh'));
    });
    await waitFor(() => expect(screen.getAllByTestId('agent-run-row')).toHaveLength(2));

    replayEvents = 'db: 事件读取失败';
    await act(async () => {
      fireEvent.click(screen.getAllByTestId('agent-run-row')[0]);
    });
    await waitFor(() =>
      expect(screen.getByTestId('history-error').textContent).toContain('db: 事件读取失败'),
    );
    expect(screen.getAllByTestId('agent-run-row')).toHaveLength(2);
  });

  it('一次完整 run 结束（result 到达）后不自动触发 agent_runs（D13），显式刷新才取数', async () => {
    render(<AgentDebugView root={ROOT} />);
    await launch();

    act(() => {
      lastChannel().onmessage?.(runResult(0));
    });
    await act(async () => {
      resolveStart?.(record('completed'));
    });
    await waitFor(() =>
      expect(screen.getByTestId('agent-start').hasAttribute('disabled')).toBe(false),
    );
    expect(countOf('agent_runs')).toBe(0);

    await act(async () => {
      fireEvent.click(screen.getByTestId('history-refresh'));
    });
    await waitFor(() => expect(countOf('agent_runs')).toBe(1));
  });
});
