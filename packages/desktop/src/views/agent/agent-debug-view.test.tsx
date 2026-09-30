// @vitest-environment jsdom
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { AgentEvent, AgentInstanceRecord, AgentRunRecord } from '../../types/dto';
import { AgentDebugView } from './agent-debug-view';

// ---------------------------------------------------------------------------
// 驱动方式（最小 mock 原则修订）：use-agent-chat / use-agent-run-history 为
// 进程内内部协作者，不 mock——fixture 改经 mock invoke / Channel 信封流入真实
// useAgentChat（eventsToUIMessages / runRecordToUIMessage 不在套件内直构）。
// 唯一 mock 点为 @tauri-apps/api/core 进程边界：invoke 按命令名分发
// （agent_start / agent_stop / agent_runs / agent_run_events，可切换
// resolve / reject / pending 并记录入参）；Channel 可编程 class 捕获
// onmessage（测试直接投递 Event / Record 信封驱动实时流与终态回流）。
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
// fixture（serde camelCase 线格式，debug 形态：sourceRef / parentRunId 皆 null）
// ---------------------------------------------------------------------------

const ROOT = 'C:\\demo\\beta';

function run(id: number, status: AgentRunRecord['status']): AgentRunRecord {
  return {
    id,
    prompt: '你好',
    cwd: ROOT,
    env: 'default',
    permissionMode: 'bypassPermissions',
    status,
    startedAt: 1727000000000,
    finishedAt: status === 'running' ? null : 1727000001000,
    numTurns: status === 'running' ? null : 3,
    costUsd: status === 'running' ? null : 0.5,
    durationMs: status === 'running' ? null : 1234,
    sessionId: status === 'running' ? null : 's-1',
    error: null,
    source: 'debug',
    sourceRef: null,
    parentRunId: null,
  };
}

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

function runResult(seq: number): AgentEvent {
  return {
    seq,
    timestampMs: 1727000000001,
    kind: 'runResult',
    subtype: 'success',
    isError: false,
    numTurns: 3,
    durationMs: 1234,
    costUsd: 0.5,
    usage: {},
    sessionId: 's-1',
  };
}

let startBehavior: { mode: 'resolve' | 'reject'; value: AgentRunRecord | string } | null = null;

/** agent 实例清单 fixture（list_agent_instances 应答；默认空清单 = 缺省语义）。 */
let instancesFixture: AgentInstanceRecord[] = [];

function mockIpc() {
  ChannelMock.instances.length = 0;
  startBehavior = { mode: 'resolve', value: run(1, 'running') };
  instancesFixture = [];
  invokeMock.mockReset();
  invokeMock.mockImplementation((command: string) => {
    if (command === 'agent_start') {
      const behavior = startBehavior;
      if (behavior?.mode === 'resolve') return Promise.resolve(behavior.value);
      if (behavior?.mode === 'reject') return Promise.reject(behavior.value);
      return new Promise<AgentRunRecord>(() => {});
    }
    if (command === 'agent_stop') return Promise.resolve(null);
    if (command === 'agent_runs') return Promise.resolve([]);
    if (command === 'agent_run_events') return Promise.resolve([]);
    if (command === 'list_agent_instances') return Promise.resolve(instancesFixture);
    return Promise.resolve(null);
  });
}

beforeEach(() => {
  mockIpc();
});

// ---------------------------------------------------------------------------
// 装置
// ---------------------------------------------------------------------------

interface ChannelLike {
  onmessage: ((message: unknown) => void) | null;
}

function lastChannel(): ChannelLike {
  const instance = ChannelMock.instances.at(-1);
  if (!instance) throw new Error('尚未发起运行（无 Channel 实例）');
  return instance;
}

function deliverEvent(event: AgentEvent) {
  act(() => {
    lastChannel().onmessage?.({ ipc: 'event', event });
  });
}

function deliverRecord(record: AgentRunRecord) {
  act(() => {
    lastChannel().onmessage?.({ ipc: 'record', record });
  });
}

/** 发起一次运行：填 prompt 后点击启动（页面真实表单路径）。 */
async function startRun(prompt = '你好') {
  fireEvent.change(screen.getByTestId('agent-prompt'), { target: { value: prompt } });
  fireEvent.click(screen.getByTestId('agent-start'));
  await act(async () => {});
}

// ---------------------------------------------------------------------------
// 页面接线（真实组合）
// ---------------------------------------------------------------------------

describe('AgentDebugView → AgentTimeline 接线', () => {
  it('渲染四区块：参数面、事件时间线（保真透镜）、原始 JSONL 切换入口、历史运行区', () => {
    render(<AgentDebugView root={ROOT} />);

    expect(screen.getByTestId('agent-run-form') !== null).toBe(true);
    expect(screen.getByTestId('agent-timeline') !== null).toBe(true);
    expect(screen.getByTestId('stream-toggle') !== null).toBe(true);
    expect(screen.getByTestId('agent-run-history') !== null).toBe(true);
  });

  it('root=null 时启动入口缺席：页面自呈现提示、不崩', () => {
    render(<AgentDebugView root={null} />);

    expect(screen.getByTestId('agent-no-root') !== null).toBe(true);
    expect(screen.queryByTestId('agent-run-form')).toBeNull();
    expect(screen.queryByTestId('agent-start')).toBeNull();
  });

  it('运行后事件与终态 record 经 AgentTimeline 全量呈现（event-result / event-run-record 可见）', async () => {
    render(<AgentDebugView root={ROOT} />);
    await startRun();

    deliverEvent(runStarted(0));
    deliverEvent(runResult(1));
    deliverRecord(run(1, 'completed'));
    await waitFor(() => expect(screen.getByTestId('event-run-record') !== null).toBe(true));

    expect(screen.getByTestId('event-run-started') !== null).toBe(true);
    expect(screen.getByTestId('event-result') !== null).toBe(true);
    expect(screen.getByTestId('result-num-turns').textContent).toBe('3');
    expect(screen.getByTestId('event-run-record').getAttribute('data-status')).toBe('completed');
  });

  it('原始 JSONL 切换二选一：AgentRawStream 消费 chat 的 events 镜像逐事件 dump，切回时间线', async () => {
    render(<AgentDebugView root={ROOT} />);
    await startRun();

    deliverEvent(runStarted(0));
    deliverEvent(runResult(1));
    await act(async () => {});

    fireEvent.click(screen.getByTestId('toggle-raw'));
    expect(screen.getByTestId('agent-raw-stream') !== null).toBe(true);
    expect(screen.queryByTestId('agent-timeline')).toBeNull();
    // events 镜像逐事件 dump（camelCase 线格式）
    expect(screen.getAllByTestId('raw-line')).toHaveLength(2);
    expect(screen.getAllByTestId('raw-line')[0]?.textContent).toContain('runStarted');

    fireEvent.click(screen.getByTestId('toggle-timeline'));
    expect(screen.getByTestId('agent-timeline') !== null).toBe(true);
    expect(screen.queryByTestId('agent-raw-stream')).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// 停止入口与停止后收敛（AC-6 触发后事件流与状态收敛的呈现半边）
// ---------------------------------------------------------------------------

describe('AgentDebugView：停止入口与收敛呈现', () => {
  it('非运行态无停止入口', () => {
    render(<AgentDebugView root={ROOT} />);

    expect(screen.queryByTestId('run-stop')).toBeNull();
    expect(screen.queryAllByText(/(停止|取消|终止|中断|kill)/i)).toHaveLength(0);
  });

  it('running 中表单禁用且 chrome 停止按钮在场，点击调 stop 恰一次', async () => {
    render(<AgentDebugView root={ROOT} />);
    // agent_start 提前 resolve running 记录（currentRunId 确立）；流保持打开直至
    // 终态 record 信封——stop 寻址的稳定 running 场景
    await startRun();
    await waitFor(() => expect(screen.getByTestId('run-stop') !== null).toBe(true));
    expect(screen.getByTestId('agent-start').hasAttribute('disabled')).toBe(true);

    fireEvent.click(screen.getByTestId('run-stop'));
    await act(async () => {});

    const stopCalls = invokeMock.mock.calls.filter(([name]) => name === 'agent_stop');
    expect(stopCalls).toHaveLength(1);
    expect(stopCalls[0]).toEqual(['agent_stop', { root: ROOT, runId: 1 }]);
  });

  it('停止后终态 record 回流：event-run-record 呈现、running 复位、表单恢复可用', async () => {
    render(<AgentDebugView root={ROOT} />);
    // 提前 resolve running 记录 + 流保持打开：stop 寻址可用、running 态稳定
    await startRun();
    await waitFor(() => expect(screen.getByTestId('run-stop') !== null).toBe(true));

    fireEvent.click(screen.getByTestId('run-stop'));
    await act(async () => {});
    deliverEvent(runStarted(0));
    deliverRecord(run(1, 'stopped'));

    await waitFor(() => expect(screen.queryByTestId('run-stop')).toBeNull());
    expect(screen.getByTestId('event-run-record').getAttribute('data-status')).toBe('stopped');
    expect(screen.getByTestId('agent-start').hasAttribute('disabled')).toBe(false);
  });

  it('运行发起失败（agent_start reject）呈现错误横幅', async () => {
    startBehavior = { mode: 'reject', value: 'CLI 未找到' };
    render(<AgentDebugView root={ROOT} />);
    await startRun();

    await waitFor(() => expect(screen.getByTestId('run-error') !== null).toBe(true));
    expect(screen.getByTestId('run-error').textContent).toContain('CLI 未找到');
  });
});

// ---------------------------------------------------------------------------
// 区域滚动框架（jsdom className 结构代理）
// ---------------------------------------------------------------------------

describe('AgentDebugView：区域滚动框架', () => {
  it('根节点呈 flex 填充、流视图区（时间线 / 原始 JSONL 二选一）flex-1 填充', async () => {
    const { container } = render(<AgentDebugView root={ROOT} />);
    await startRun();
    deliverEvent(runStarted(0));
    await act(async () => {});

    // 根节点接入壳层 flex 链
    const rootDiv = container.firstElementChild;
    for (const className of ['flex', 'min-h-0', 'flex-1', 'flex-col']) {
      expect(rootDiv?.className).toContain(className);
    }
    // 流视图区（时间线态）flex-1 填充
    expect(screen.getByTestId('agent-timeline').className).toContain('flex-1');

    // 原始 JSONL 态同样 flex-1 填充
    fireEvent.click(screen.getByTestId('toggle-raw'));
    const rawSection = screen.getByTestId('agent-raw-stream');
    for (const className of ['flex', 'min-h-0', 'flex-1', 'flex-col']) {
      expect(rawSection.className).toContain(className);
    }
  });
});

// ---------------------------------------------------------------------------
// 调试页 agent 选择（发起面演进）：agent 选择器选项 = 实例清单（useAgentOptions
// 挂载取数），默认选中默认 agent，缺省项透传 null 由后端解析默认 agent；
// start 透传 input.agent 至 sendMessage；sdk 运行复用既有时间线 / 落库 /
// 重放呈现面
// ---------------------------------------------------------------------------

/** sdk 形态事件 fixture：ToolUse / ToolResult 成对 + 思考块。 */
function toolUseEvent(seq: number): AgentEvent {
  return {
    seq,
    timestampMs: 1727000000000,
    kind: 'message',
    role: 'assistant',
    blocks: [
      { kind: 'thinking', thinking: '先读文件' },
      { kind: 'toolUse', id: 'tu_sdk_1', name: 'read', input: { path: 'a.txt' } },
    ],
    parentToolUseId: null,
  };
}

function toolResultEvent(seq: number): AgentEvent {
  return {
    seq,
    timestampMs: 1727000000001,
    kind: 'message',
    role: 'user',
    blocks: [{ kind: 'toolResult', id: 'tu_sdk_1', content: '文件内容 🎉', isError: false }],
    parentToolUseId: null,
  };
}

/** agent 实例 fixture（serde camelCase 线格式；providerId 对选择器面无关可空）。 */
function agentInstance(
  id: number,
  name: string,
  engine: AgentInstanceRecord['engine'],
  isDefault = false,
): AgentInstanceRecord {
  return { id, name, engine, providerId: null, isDefault };
}

function selectOf(testId: string): HTMLSelectElement {
  return screen.getByTestId(testId);
}

function startCallAgent(): unknown {
  const call = invokeMock.mock.calls.filter(([name]) => name === 'agent_start').at(-1);
  if (!call) throw new Error('agent_start 未被调用');
  return (call[1] as Record<string, unknown>)['agent'];
}

describe('AgentDebugView：调试页 agent 选择（缺省 / 显式）', () => {
  it('空清单：agent-select 仅存缺省项；不动发起 → invoke 入参 agent 为 null（后端解析默认 agent）', async () => {
    render(<AgentDebugView root={ROOT} />);

    expect(selectOf('agent-select').value).toBe('');
    const values = Array.from(selectOf('agent-select').options).map((option) => option.value);
    expect(values).toEqual(['']);

    await startRun();
    expect(startCallAgent()).toBeNull();
  });

  it('清单含默认 agent：初始即选中默认 agent；不动发起 → invoke 入参 agent 为默认 id', async () => {
    instancesFixture = [agentInstance(3, 'cli-a', 'cli'), agentInstance(7, 'sdk-b', 'sdk', true)];
    render(<AgentDebugView root={ROOT} />);

    await waitFor(() => expect(selectOf('agent-select').value).toBe('7'));
    const values = Array.from(selectOf('agent-select').options).map((option) => option.value);
    expect(values).toEqual(['', '3', '7']);

    await startRun();
    expect(startCallAgent()).toBe(7);
  });

  it('切至显式 agent 后发起 → start 透传 input.agent 至 sendMessage，invoke 入参 agent 为显式 id', async () => {
    instancesFixture = [agentInstance(3, 'cli-a', 'cli'), agentInstance(7, 'sdk-b', 'sdk', true)];
    render(<AgentDebugView root={ROOT} />);
    await waitFor(() => expect(selectOf('agent-select').value).toBe('7'));

    fireEvent.change(selectOf('agent-select'), { target: { value: '3' } });
    await startRun('显式 agent 调试轮');

    expect(startCallAgent()).toBe(3);
  });

  it('sdk 运行的 ToolUse / ToolResult 成对事件经既有时间线组件呈现，终态 record 回流（呈现面零改动复用）', async () => {
    render(<AgentDebugView root={ROOT} />);
    await startRun();

    deliverEvent(toolUseEvent(0));
    deliverEvent(toolResultEvent(1));
    deliverEvent(runResult(2));
    deliverRecord(run(1, 'completed'));
    await waitFor(() => expect(screen.getByTestId('event-run-record') !== null).toBe(true));

    // 成对块经既有时间线保真透镜呈现（block 级 testid 复用）
    expect(screen.getByTestId('block-tool-use').getAttribute('data-tool-id')).toBe('tu_sdk_1');
    expect(screen.getByTestId('block-tool-result').textContent).toContain('文件内容 🎉');
    expect(screen.getByTestId('event-run-record').getAttribute('data-status')).toBe('completed');
  });

  it('SDK 启动失败（ConfigMissing 错误串）→ run-error 横幅呈现错误串（Err 抵达前端）', async () => {
    startBehavior = {
      mode: 'reject',
      value: '配置缺失: 配置项未填: api_key / base_url / model（引擎配置硬编码位未手填）',
    };
    render(<AgentDebugView root={ROOT} />);
    await startRun();

    await waitFor(() => expect(screen.getByTestId('run-error') !== null).toBe(true));
    expect(screen.getByTestId('run-error').textContent).toContain('配置缺失');
    expect(screen.getByTestId('run-error').textContent).toContain('api_key');
  });
});
