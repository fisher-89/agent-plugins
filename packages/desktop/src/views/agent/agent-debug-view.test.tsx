// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type * as AgentSessionViewModule from '@/components/agent/agent-session-view';

import type { AgentEvent, AgentInstanceRecord, SessionSummary, TurnSummary } from '../../types/dto';
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
// 探针装置（保留真实实现，仅记录 props）：在 `AgentSessionView` 边界记录下游
// 收到的 `askAnswer` 引用——「引用稳定未击穿 memo」以引用相等（而非渲染次数）
// 为判据。`importOriginal` 保留全部真实实现（活卡 / 判活 / 气泡照旧），探针零
// 行为、只记录。
// ---------------------------------------------------------------------------

const { askAnswerProbe } = vi.hoisted(() => ({ askAnswerProbe: { refs: [] as unknown[] } }));

vi.mock('@/components/agent/agent-session-view', async (importOriginal) => {
  const actual = await importOriginal<typeof AgentSessionViewModule>();
  function ProbedAgentSessionView(props: Parameters<typeof actual.AgentSessionView>[0]) {
    askAnswerProbe.refs.push(props.askAnswer ?? null);
    return <actual.AgentSessionView {...props} />;
  }
  return { ...actual, AgentSessionView: ProbedAgentSessionView };
});

// ---------------------------------------------------------------------------
// fixture（serde camelCase 线格式，debug 形态：sourceRef / parentRunId 皆 null）
// ---------------------------------------------------------------------------

const ROOT = 'C:\\demo\\beta';

const SESSION_ID = 'ses-1-1727000000000';

/** store `SessionConfigSnapshot` 快照线形态（serde camelCase；v2 增
 * `agentInstanceId` 实例外键——fixture 缺键即类型红，防漂移）。 */
interface ConfigSnapshotFixture {
  engine: 'cli' | 'sdk';
  agentInstanceId: number | null;
  model: string | null;
  permissionMode: string;
}

function run(turnId: number, status: TurnSummary['status']): TurnSummary {
  return {
    turnId,
    sessionId: SESSION_ID,
    status,
    startedAt: 1727000000000,
    finishedAt: status === 'running' ? null : 1727000001000,
    numTurns: status === 'running' ? null : 3,
    costUsd: status === 'running' ? null : 0.5,
    durationMs: status === 'running' ? null : 1234,
    error: null,
  };
}

function runStarted(seq: number, turnId: number | null = 1): AgentEvent {
  return {
    seq,
    turnId,
    timestampMs: 1727000000000,
    kind: 'runStarted',
    model: 'claude-opus',
    sessionId: 's-1',
    tools: ['Bash'],
    mcpServers: [],
  };
}

function turnDone(seq: number, turnId: number | null = 1): AgentEvent {
  return {
    seq,
    turnId,
    timestampMs: 1727000000001,
    kind: 'turnDone',
    subtype: 'success',
    isError: false,
    numTurns: 3,
    durationMs: 1234,
    costUsd: 0.5,
    usage: {},
    sessionId: SESSION_ID,
  };
}

function textDeltaEvent(seq: number, text: string, turnId: number | null = 1): AgentEvent {
  return {
    seq,
    turnId,
    timestampMs: 1727000000002,
    kind: 'messageDelta',
    parentToolUseId: null,
    delta: { kind: 'text', text },
  };
}

function sealedTextEvent(seq: number, text: string, turnId: number | null = 1): AgentEvent {
  return {
    seq,
    turnId,
    timestampMs: 1727000000003,
    kind: 'message',
    role: 'assistant',
    blocks: [{ kind: 'text', text }],
    parentToolUseId: null,
  };
}

let startBehavior: { mode: 'resolve' | 'reject'; value: TurnSummary | string } | null = null;

/** agent 实例清单 fixture（list_agent_instances 应答；默认空清单 = 缺省语义）。 */
let instancesFixture: AgentInstanceRecord[] = [];

/** 会话清单 fixture（agent_sessions 应答 `SessionPage` 信封；默认空清单，来源筛选用例按需注入）。 */
let historySessionsFixture: SessionSummary[] = [];

/** 会话单查 fixture（session_detail 应答；默认 null = 库内无该会话行）。 */
let sessionDetailFixture: SessionSummary | null = null;

/** 会话转录 fixture（agent_session_transcript 应答；库内密封事件全史）。 */
let sessionTranscriptFixture: AgentEvent[] = [];

/** agent_sessions 应答信封（total 缺省由清单推导）。 */
function historyPage() {
  return { sessions: historySessionsFixture, total: historySessionsFixture.length };
}

function mockIpc() {
  ChannelMock.instances.length = 0;
  startBehavior = { mode: 'resolve', value: run(1, 'running') };
  instancesFixture = [];
  historySessionsFixture = [];
  sessionDetailFixture = null;
  sessionTranscriptFixture = [];
  invokeMock.mockReset();
  invokeMock.mockImplementation((command: string) => {
    if (command === 'agent_start') {
      const behavior = startBehavior;
      if (behavior?.mode === 'resolve') return Promise.resolve(behavior.value);
      if (behavior?.mode === 'reject') return Promise.reject(behavior.value);
      return new Promise<TurnSummary>(() => {});
    }
    if (command === 'agent_stop') return Promise.resolve(null);
    if (command === 'agent_sessions') return Promise.resolve(historyPage());
    if (command === 'session_detail') return Promise.resolve(sessionDetailFixture);
    if (command === 'agent_session_transcript') return Promise.resolve(sessionTranscriptFixture);
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

function deliverRecord(record: TurnSummary) {
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

describe('AgentDebugView → AgentMessages 接线', () => {
  it('渲染四区块：参数面、对话视图（AgentMessages 透镜）、原始 JSONL 切换入口、历史运行区', () => {
    render(<AgentDebugView root={ROOT} />);

    expect(screen.getByTestId('agent-run-form') !== null).toBe(true);
    expect(screen.getByTestId('agent-messages') !== null).toBe(true);
    expect(screen.getByTestId('stream-toggle') !== null).toBe(true);
    expect(screen.getByTestId('agent-run-history') !== null).toBe(true);
  });

  it('root=null 时启动入口缺席：页面自呈现提示、不崩', () => {
    render(<AgentDebugView root={null} />);

    expect(screen.getByTestId('agent-no-root') !== null).toBe(true);
    expect(screen.queryByTestId('agent-run-form')).toBeNull();
    expect(screen.queryByTestId('agent-start')).toBeNull();
  });

  it('运行后事件与终态 record 经 AgentMessages 呈现：对话气泡可读、running 收口复位（辅助载体不进对话）', async () => {
    render(<AgentDebugView root={ROOT} />);
    await startRun();

    // runStarted 为辅助载体（不进对话呈现），密封消息进对话气泡；提问由本地
    // 推入（会话状态侧）故另有一条 user 气泡
    deliverEvent(runStarted(0));
    deliverEvent(sealedTextEvent(1, '调试运行正文'));
    await waitFor(() =>
      expect(screen.getAllByTestId('block-text').map((node) => node.textContent)).toContain(
        '调试运行正文',
      ),
    );
    // 运行中：对话区头部运行标记在场
    expect(screen.getByTestId('agent-messages-running') !== null).toBe(true);

    // 终态收口：turnDone / record 均为辅助载体，以 running 复位观测回流
    deliverEvent(turnDone(2));
    deliverRecord(run(1, 'completed'));
    await waitFor(() => expect(screen.queryByTestId('agent-messages-running')).toBeNull());
  });

  it('原始 JSONL 切换二选一：AgentRawStream 消费 chat 的 events 镜像逐事件 dump，切回时间线', async () => {
    render(<AgentDebugView root={ROOT} />);
    await startRun();

    deliverEvent(runStarted(0));
    deliverEvent(turnDone(1));
    await act(async () => {});

    fireEvent.click(screen.getByTestId('toggle-raw'));
    expect(screen.getByTestId('agent-raw-stream') !== null).toBe(true);
    expect(screen.queryByTestId('agent-messages')).toBeNull();
    // events 镜像逐事件 dump（camelCase 线格式）
    expect(screen.getAllByTestId('raw-line')).toHaveLength(2);
    expect(screen.getAllByTestId('raw-line')[0]?.textContent).toContain('runStarted');

    fireEvent.click(screen.getByTestId('toggle-timeline'));
    expect(screen.getByTestId('agent-messages') !== null).toBe(true);
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
    expect(stopCalls[0]).toEqual(['agent_stop', { root: ROOT, sessionId: SESSION_ID }]);
  });

  it('停止后终态 record 回流：running 复位、表单恢复可用（record 为辅助载体，以运行复位观测回流）', async () => {
    render(<AgentDebugView root={ROOT} />);
    // 提前 resolve running 记录 + 流保持打开：stop 寻址可用、running 态稳定
    await startRun();
    await waitFor(() => expect(screen.getByTestId('run-stop') !== null).toBe(true));

    fireEvent.click(screen.getByTestId('run-stop'));
    await act(async () => {});
    deliverEvent(runStarted(0));
    deliverRecord(run(1, 'stopped'));

    await waitFor(() => expect(screen.queryByTestId('run-stop')).toBeNull());
    expect(screen.queryByTestId('agent-messages-running')).toBeNull();
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
    expect(screen.getByTestId('agent-messages').className).toContain('flex-1');

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
function toolUseEvent(seq: number, turnId: number | null = 1): AgentEvent {
  return {
    seq,
    turnId,
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

function toolResultEvent(seq: number, turnId: number | null = 1): AgentEvent {
  return {
    seq,
    turnId,
    timestampMs: 1727000000001,
    kind: 'message',
    // 引擎工具结果管道 = `role: 'tool'`（`loop.rs` 生产点同值）；`role: 'user'`
    // 只用于真实用户言语（其事件零 chunk，提问由会话状态侧承载）
    role: 'tool',
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

/** agent-select 为 StandardSelect（base-ui Select）：testid 落在 SelectValue
 * span（data-value 承载选中 id），trigger 为其外层 button（role=combobox）。 */
function agentValue(): HTMLElement {
  return screen.getByTestId('agent-select');
}

function agentTrigger(): HTMLElement {
  const trigger = agentValue().closest('button');
  if (!trigger) throw new Error('agent-select 不在 trigger button 内');
  return trigger;
}

/** 打开 agent 选择器：base-ui Select 以 mousedown 打开、开启动作为 frame
 * 异步执行，以 listbox 出现收敛最终态。 */
async function openAgentSelect(): Promise<void> {
  fireEvent.mouseDown(agentTrigger());
  await screen.findByRole('listbox');
}

/** 打开选择器并点选指定 label 项：pointerDown 先行满足 base-ui 鼠标选择
 * 守卫（allowMouseSelectionRef），click 提交选中。 */
async function pickAgent(label: string): Promise<void> {
  await openAgentSelect();
  const option = within(screen.getByRole('listbox')).getByRole('option', { name: label });
  fireEvent.pointerDown(option);
  fireEvent.click(option);
}

function startCallAgent(): unknown {
  const call = invokeMock.mock.calls.filter(([name]) => name === 'agent_start').at(-1);
  if (!call) throw new Error('agent_start 未被调用');
  return (call[1] as Record<string, unknown>)['agent'];
}

describe('AgentDebugView：调试页 agent 选择（缺省 / 显式）', () => {
  it('空清单：agent-select 呈缺省占位、data-value 缺席、弹层零 option；不动发起 → invoke 入参 agent 为 null（后端解析默认 agent）', async () => {
    render(<AgentDebugView root={ROOT} />);

    expect(agentValue().getAttribute('data-value')).toBeNull();
    expect(agentValue().textContent).toBe('请选择');
    await openAgentSelect();
    expect(within(screen.getByRole('listbox')).queryAllByRole('option')).toHaveLength(0);

    await startRun();
    expect(startCallAgent()).toBeNull();
  });

  it('清单含默认 agent：初始即选中默认 agent；不动发起 → invoke 入参 agent 为默认 id', async () => {
    instancesFixture = [agentInstance(3, 'cli-a', 'cli'), agentInstance(7, 'sdk-b', 'sdk', true)];
    render(<AgentDebugView root={ROOT} />);

    await waitFor(() => expect(agentValue().getAttribute('data-value')).toBe('7'));
    expect(agentValue().textContent).toBe('sdk-b（sdk）');
    await openAgentSelect();
    const options = within(screen.getByRole('listbox')).getAllByRole('option');
    expect(options.map((option) => option.textContent)).toEqual(['cli-a（cli）', 'sdk-b（sdk）']);

    await startRun();
    expect(startCallAgent()).toBe(7);
  });

  it('切至显式 agent 后发起 → start 透传 input.agent 至 sendMessage，invoke 入参 agent 为显式 id', async () => {
    instancesFixture = [agentInstance(3, 'cli-a', 'cli'), agentInstance(7, 'sdk-b', 'sdk', true)];
    render(<AgentDebugView root={ROOT} />);
    await waitFor(() => expect(agentValue().getAttribute('data-value')).toBe('7'));

    await pickAgent('cli-a（cli）');
    expect(agentValue().getAttribute('data-value')).toBe('3');
    await startRun('显式 agent 调试轮');

    expect(startCallAgent()).toBe(3);
  });

  it('sdk 运行的 ToolUse / ToolResult 成对事件经既有对话组件呈现，终态 record 回流（呈现面零改动复用）', async () => {
    render(<AgentDebugView root={ROOT} />);
    await startRun();

    deliverEvent(toolUseEvent(0));
    deliverEvent(toolResultEvent(1));
    deliverEvent(turnDone(2));
    // 收口以重放收敛（库为准）：库内会话行 + 密封转录即本次投递的三枚事件
    sessionDetailFixture = {
      row: {
        id: SESSION_ID,
        remoteSessionId: null,
        configSnapshot: {
          engine: 'sdk',
          agentInstanceId: null,
          model: null,
          permissionMode: 'bypassPermissions',
        },
        provenance: { source: 'debug', sourceRef: null },
        createdAt: 1727000000000,
        updatedAt: 1727000001000,
      },
      stats: { turnCount: 1, totalDurationMs: null, inputTokens: null, outputTokens: null },
      turns: [run(1, 'completed')],
    };
    sessionTranscriptFixture = [toolUseEvent(0), toolResultEvent(1), turnDone(2)];
    deliverRecord(run(1, 'completed'));
    // 终态 record 回流（辅助载体不进对话）：以 running 复位为回流锚点
    await waitFor(() =>
      expect(screen.getByTestId('agent-start').hasAttribute('disabled')).toBe(false),
    );

    // 成对块经同一对话透镜呈现（收口以重放重建 ⇒ 对话内该轮恰一条 assistant 气泡）
    const assistantBubbles = () =>
      screen
        .getAllByTestId('chat-bubble')
        .filter((bubble) => bubble.getAttribute('data-role') === 'assistant');
    await waitFor(() => {
      expect(assistantBubbles()).toHaveLength(1);
      expect(
        within(assistantBubbles()[0] as HTMLElement)
          .getByTestId('block-tool-use')
          .getAttribute('data-tool-id'),
      ).toBe('tu_sdk_1');
    });
    // input 与同 id output 合卡；结果正文在卡面展开后呈现（TraceCollapsible 默认折叠）
    const toolCard = within(assistantBubbles()[0] as HTMLElement).getByTestId('block-tool-use');
    fireEvent.click(within(toolCard).getByRole('button'));
    expect((await screen.findByTestId('block-tool-result')).textContent).toContain('文件内容 🎉');
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

// ---------------------------------------------------------------------------
// 来源筛选接线（desktop-change-session-visibility，AC-3）：筛选状态由页面
// 持有（useState<AgentHistorySource>('debug')）并下传 useAgentRunHistory 与
// AgentRunHistory——点击枚后 onSourceChange 状态提升生效，agent_sessions 按
// 新 source 重查（真实组合：fixture 经 mock invoke 流入真实 hook）。
// ---------------------------------------------------------------------------

describe('AgentDebugView：来源筛选接线（AC-3）', () => {
  function sessionsCalls(): unknown[][] {
    return invokeMock.mock.calls.filter(([name]) => name === 'agent_sessions');
  }

  it('页面挂载：history-source-filter 在场且 debug 枚高亮，agent_sessions 以 source: debug 取数（默认现状不变）', async () => {
    render(<AgentDebugView root={ROOT} />);

    const filter = screen.getByTestId('history-source-filter');
    const buttons = within(filter).getAllByRole('button');
    expect(buttons.map((button) => button.getAttribute('aria-pressed'))).toEqual([
      'true',
      'false',
      'false',
    ]);

    await waitFor(() => expect(sessionsCalls()).toHaveLength(1));
    expect(invokeMock).toHaveBeenCalledWith('agent_sessions', {
      root: ROOT,
      source: 'debug',
      sourceRef: null,
      offset: 0,
      limit: 20,
    });
  });

  it('点击 change 枚：onSourceChange 状态提升生效 → agent_sessions 按新 source 重查（筛选状态 → hook 取数链路）', async () => {
    render(<AgentDebugView root={ROOT} />);
    await waitFor(() => expect(sessionsCalls()).toHaveLength(1));

    const buttons = within(screen.getByTestId('history-source-filter')).getAllByRole('button');
    fireEvent.click(buttons[1]);

    await waitFor(() => expect(sessionsCalls()).toHaveLength(2));
    expect(invokeMock).toHaveBeenLastCalledWith('agent_sessions', {
      root: ROOT,
      source: 'change',
      sourceRef: null,
      offset: 0,
      limit: 20,
    });
    expect(
      within(screen.getByTestId('history-source-filter'))
        .getAllByRole('button')
        .map((button) => button.getAttribute('aria-pressed')),
    ).toEqual(['false', 'true', 'false']);
  });

  it('点击全部枚：agent_sessions 携 source: null，混合来源清单渲染不炸（列表承接零新概念）', async () => {
    historySessionsFixture = [
      {
        row: {
          id: 'ses-debug-1',
          remoteSessionId: null,
          configSnapshot: {
            engine: 'sdk',
            agentInstanceId: 7,
            model: 'glm-high',
            permissionMode: 'bypassPermissions',
          } satisfies ConfigSnapshotFixture,
          provenance: { source: 'debug', sourceRef: null },
          createdAt: 1727000000000,
          updatedAt: 1727000000300,
        },
        stats: { turnCount: 1, totalDurationMs: 100, inputTokens: null, outputTokens: null },
        turns: [],
      },
      {
        row: {
          id: 'ses-change-1',
          remoteSessionId: null,
          configSnapshot: {
            engine: 'cli',
            agentInstanceId: null,
            model: null,
            permissionMode: 'bypassPermissions',
          } satisfies ConfigSnapshotFixture,
          provenance: { source: 'change', sourceRef: 'add-feature/implement/executor/1' },
          createdAt: 1727000000000,
          updatedAt: 1727000000200,
        },
        stats: { turnCount: 2, totalDurationMs: 200, inputTokens: null, outputTokens: null },
        turns: [],
      },
    ];
    render(<AgentDebugView root={ROOT} />);

    const buttons = within(screen.getByTestId('history-source-filter')).getAllByRole('button');
    fireEvent.click(buttons[2]);

    await waitFor(() =>
      expect(invokeMock).toHaveBeenLastCalledWith('agent_sessions', {
        root: ROOT,
        source: null,
        sourceRef: null,
        offset: 0,
        limit: 20,
      }),
    );
    const rows = await screen.findAllByTestId('agent-run-row');
    expect(rows.map((row) => row.getAttribute('data-session-id'))).toEqual([
      'ses-debug-1',
      'ses-change-1',
    ]);
    // 高亮随页面状态切换（all 枚按下）
    expect(
      within(screen.getByTestId('history-source-filter'))
        .getAllByRole('button')
        .map((button) => button.getAttribute('aria-pressed')),
    ).toEqual(['false', 'false', 'true']);
  });
});

// ---------------------------------------------------------------------------
// 补遗：每跑重置 New 会话（AC-9 命令面投影）/ 流式呈现单条连续增长消息
//（AC-1 UI 半边——碎行不复现）
// ---------------------------------------------------------------------------

describe('AgentDebugView：每跑重置', () => {
  it('上一跑终态后再次发起：invoke 不携带旧 sessionId（每跑 New 会话）', async () => {
    render(<AgentDebugView root={ROOT} />);

    // 第一跑：发起 → 终态 record 回流（会话镜像确立）
    await startRun('第一跑');
    deliverRecord(run(1, 'completed'));
    await waitFor(() =>
      expect(screen.getByTestId('agent-start').hasAttribute('disabled')).toBe(false),
    );

    // 第二跑：重置后发起——agent_start 入参 sessionId 为 null（New 语义）
    await startRun('第二跑');

    const startCalls = invokeMock.mock.calls.filter(([name]) => name === 'agent_start');
    expect(startCalls).toHaveLength(2);
    const secondArgs = startCalls[1]?.[1] as Record<string, unknown>;
    expect(secondArgs['sessionId']).toBeNull();
    expect(secondArgs['root']).toBe(ROOT);
  });
});

describe('AgentDebugView：流式呈现（AC-1 UI 半边）', () => {
  it('delta 信封累积为单条连续增长消息（碎行不复现），密封到达同键让位替换仍单条', async () => {
    render(<AgentDebugView root={ROOT} />);
    await startRun();

    // 两枚文本 delta：同一 step 内累积为单条 assistant 气泡；提问由本地推入
    //（会话状态侧）⇒ 对话为「提问 + 一条 assistant」两条
    deliverEvent(textDeltaEvent(0, '你好'));
    deliverEvent(textDeltaEvent(1, '工作区'));
    await waitFor(() => {
      const roles = screen
        .getAllByTestId('chat-bubble')
        .map((bubble) => bubble.getAttribute('data-role'));
      expect(roles).toEqual(['user', 'assistant']);
    });
    // messages 通知有节流窗：累积文本以终态等待
    await waitFor(() => {
      expect(screen.getAllByTestId('block-text').map((node) => node.textContent)).toContain(
        '你好工作区',
      );
    });

    // 密封 Message 到达：整块部件组（start + reset-step + 整块）
    deliverEvent(sealedTextEvent(2, '你好工作区'));
    await act(async () => {});

    // 终态收尾：流结束触发收口归一 + 重放收敛——provisional 同键让位不残留：
    // 对话仍恰一条气泡（record 载体不进对话），文本块恰一处且以库内密封为准
    sessionDetailFixture = {
      row: {
        id: SESSION_ID,
        remoteSessionId: null,
        configSnapshot: {
          engine: 'sdk',
          agentInstanceId: null,
          model: null,
          permissionMode: 'bypassPermissions',
        },
        provenance: { source: 'debug', sourceRef: null },
        createdAt: 1727000000000,
        updatedAt: 1727000001000,
      },
      stats: { turnCount: 1, totalDurationMs: null, inputTokens: null, outputTokens: null },
      turns: [run(1, 'completed')],
    };
    sessionTranscriptFixture = [sealedTextEvent(2, '你好工作区'), turnDone(2)];
    deliverRecord(run(1, 'completed'));
    await waitFor(() =>
      expect(screen.getByTestId('agent-start').hasAttribute('disabled')).toBe(false),
    );
    await waitFor(() => {
      expect(screen.getAllByTestId('chat-bubble')).toHaveLength(1);
    });
    const texts = screen.getAllByTestId('block-text').map((node) => node.textContent);
    expect(texts).toEqual(['你好工作区']);
  });
});

// ---------------------------------------------------------------------------
// configSnapshot fixture 补 agentInstanceId 键（fixture 随动，AC-7）
// ---------------------------------------------------------------------------

describe('AgentDebugView：configSnapshot fixture 携 agentInstanceId 键（AC-7 随动）', () => {
  it('补键后历史清单呈现既有断言保绿（具体 id 与 null 两形态同口径）', async () => {
    const snapshotSdk: ConfigSnapshotFixture = {
      engine: 'sdk',
      agentInstanceId: 7,
      model: 'glm-high',
      permissionMode: 'bypassPermissions',
    };
    const snapshotLegacy: ConfigSnapshotFixture = {
      engine: 'cli',
      agentInstanceId: null,
      model: null,
      permissionMode: 'bypassPermissions',
    };
    // fixture 两形态键在场（缺键旧形态由类型面拦截，运行面再锚一次）
    expect(snapshotSdk).toHaveProperty('agentInstanceId', 7);
    expect(snapshotLegacy).toHaveProperty('agentInstanceId', null);
    historySessionsFixture = [
      {
        row: {
          id: 'ses-a-1727000000000',
          remoteSessionId: null,
          configSnapshot: snapshotSdk,
          provenance: { source: 'debug', sourceRef: null },
          createdAt: 1727000000000,
          updatedAt: 1727000000300,
        },
        stats: { turnCount: 1, totalDurationMs: 100, inputTokens: null, outputTokens: null },
        turns: [],
      },
      {
        row: {
          id: 'ses-b-1727000000000',
          remoteSessionId: null,
          configSnapshot: snapshotLegacy,
          provenance: { source: 'change', sourceRef: 'add-feature/implement/executor/1' },
          createdAt: 1727000000000,
          updatedAt: 1727000000200,
        },
        stats: { turnCount: 2, totalDurationMs: 200, inputTokens: null, outputTokens: null },
        turns: [],
      },
    ];
    render(<AgentDebugView root={ROOT} />);

    const buttons = within(screen.getByTestId('history-source-filter')).getAllByRole('button');
    fireEvent.click(buttons[2]);
    await waitFor(() =>
      expect(invokeMock).toHaveBeenLastCalledWith('agent_sessions', {
        root: ROOT,
        source: null,
        sourceRef: null,
        offset: 0,
        limit: 20,
      }),
    );
    const rows = await screen.findAllByTestId('agent-run-row');
    expect(rows.map((row) => row.getAttribute('data-session-id'))).toEqual([
      'ses-a-1727000000000',
      'ses-b-1727000000000',
    ]);
  });
});
// ---------------------------------------------------------------------------
// 时间线位（注入形态 AgentSessionView）与历史区 root 透传
// ---------------------------------------------------------------------------

describe('AgentDebugView：时间线位与历史区', () => {
  it('时间线位为注入形态 AgentSessionView（data-testid="agent-timeline"），对话由组件承载', async () => {
    render(<AgentDebugView root={ROOT} />);
    await act(async () => {});

    const timeline = screen.getByTestId('agent-timeline');
    expect(timeline).toBeTruthy();
    expect(within(timeline).getByTestId('agent-messages')).toBeTruthy();
    // 注入形态：会话 id 由 early-resolve 轮行回写 → 未发起时零会话查询
    expect(invokeMock.mock.calls.filter(([name]) => name === 'session_detail')).toHaveLength(0);
  });

  it('切到原始 JSONL → 时间线位退场、原始转录面在场（密封事件镜像，delta 不入镜像）', async () => {
    render(<AgentDebugView root={ROOT} />);
    await startRun('raw 一轮');

    deliverEvent(textDeltaEvent(2, '仅实时可见'));
    deliverEvent(sealedTextEvent(3, '密封正文'));
    await act(async () => {});

    fireEvent.click(screen.getByTestId('toggle-raw'));
    expect(screen.queryByTestId('agent-timeline')).toBeNull();
    expect(screen.getByTestId('agent-raw-stream')).toBeTruthy();
    const raw = screen.getByTestId('agent-raw-stream');
    expect(raw.textContent).toContain('密封正文');
    expect(raw.textContent).not.toContain('仅实时可见');
  });

  it('历史区透传当前 root（自持形态可寻址）；root null 时页面整替不渲染时间线', async () => {
    render(<AgentDebugView root={ROOT} />);
    await act(async () => {});

    const listCall = invokeMock.mock.calls.filter(([name]) => name === 'agent_sessions').at(-1);
    expect(listCall?.[1]).toMatchObject({ root: ROOT, source: 'debug' });

    cleanup();
    render(<AgentDebugView root={null} />);
    expect(screen.getByTestId('agent-no-root')).toBeTruthy();
    expect(screen.queryByTestId('agent-timeline')).toBeNull();
    expect(screen.queryByTestId('agent-run-history')).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// ask-tool-in-flow：调试页合成 ask 动作面（session.answer）
// ---------------------------------------------------------------------------

describe('AgentDebugView：ask 动作面（AC-13）', () => {
  it('运行后 ask 工具调用 → 活卡选项可点即 invoke agent_answer（三参逐字）', async () => {
    render(<AgentDebugView root={ROOT} />);
    await startRun('请裁决');

    deliverEvent(runStarted(0));
    deliverEvent({
      seq: 1,
      turnId: 1,
      timestampMs: 1727000000000,
      kind: 'message',
      role: 'assistant',
      blocks: [
        {
          kind: 'toolUse',
          id: 'call_1',
          name: 'AskUserQuestion',
          input: { question: '选哪个重试策略？', options: ['指数退避', '固定间隔'] },
        },
      ],
      parentToolUseId: null,
    });
    await act(async () => {});

    const timeline = screen.getByTestId('agent-timeline');
    const buttons = await within(timeline).findAllByTestId('ask-answer-option');
    expect(buttons.map((node) => node.textContent)).toEqual(['指数退避', '固定间隔']);

    fireEvent.click(buttons[0] as HTMLElement);
    await act(async () => {});

    const answerCall = invokeMock.mock.calls.filter(([name]) => name === 'agent_answer').at(-1);
    expect(answerCall).toBeTruthy();
    expect(answerCall?.[1]).toEqual({
      root: ROOT,
      sessionId: SESSION_ID,
      text: '指数退避',
    });
  });

  it('与 showRaw 切换互不干扰：原始流视图无 ask 卡，切回时间线后活卡在场', async () => {
    render(<AgentDebugView root={ROOT} />);
    await startRun('请裁决');
    deliverEvent(runStarted(0));
    deliverEvent({
      seq: 1,
      turnId: 1,
      timestampMs: 1727000000000,
      kind: 'message',
      role: 'assistant',
      blocks: [
        {
          kind: 'toolUse',
          id: 'call_raw',
          name: 'AskUserQuestion',
          input: { question: 'q', options: ['a'] },
        },
      ],
      parentToolUseId: null,
    });
    await act(async () => {});

    fireEvent.click(screen.getByTestId('toggle-raw'));
    expect(screen.getByTestId('agent-raw-stream')).toBeTruthy();
    expect(screen.queryAllByTestId('ask-answer-option')).toHaveLength(0);

    fireEvent.click(screen.getByTestId('toggle-timeline'));
    // 切回时间线 → AgentMessages 重新挂载（事件镜像回流后活卡在场）
    const timeline = screen.getByTestId('agent-timeline');
    await waitFor(() =>
      expect(within(timeline).getAllByTestId('ask-answer-option')).toHaveLength(1),
    );
  });

  it('会话未建立（无运行轮）→ 全静态且零 agent_answer（answer 内部 null 守卫的上层一半）', async () => {
    render(<AgentDebugView root={ROOT} />);
    await act(async () => {});

    // 未 startRun：时间线骨架在但空态，ask 卡与应答入口均不存在
    const timeline = screen.getByTestId('agent-timeline');
    expect(within(timeline).queryAllByTestId('ask-answer-option')).toHaveLength(0);
    expect(within(timeline).queryAllByTestId('ask-option')).toHaveLength(0);
    expect(invokeMock.mock.calls.filter(([name]) => name === 'agent_answer')).toHaveLength(0);
  });

  it('引用稳定（DD12）：answer 引用未变时重渲染 → 活卡仍单次投递（不击穿 memo）', async () => {
    render(<AgentDebugView root={ROOT} />);
    await startRun('请裁决');
    deliverEvent(runStarted(0));
    deliverEvent({
      seq: 1,
      turnId: 1,
      timestampMs: 1727000000000,
      kind: 'message',
      role: 'assistant',
      blocks: [
        {
          kind: 'toolUse',
          id: 'call_stable',
          name: 'AskUserQuestion',
          input: { question: '答哪个？', options: ['甲', '乙'] },
        },
      ],
      parentToolUseId: null,
    });
    await act(async () => {});

    const timeline = screen.getByTestId('agent-timeline');
    // 父层无关重渲染（视图切换回时间线同一面板）：活卡仍在且选项未变
    await within(timeline).findAllByTestId('ask-answer-option');
    const buttons = within(timeline).getAllByTestId('ask-answer-option');
    fireEvent.click(buttons[0] as HTMLElement);
    await act(async () => {});

    const answers = invokeMock.mock.calls.filter(([name]) => name === 'agent_answer');
    expect(answers).toHaveLength(1);
    expect(answers[0]?.[1]).toEqual({ root: ROOT, sessionId: SESSION_ID, text: '甲' });
  });

  it('引用稳定（DD12）：answer 引用未变时父层重渲染 → 下游 askAnswer 的 onAnswer 同引用', async () => {
    render(<AgentDebugView root={ROOT} />);
    await startRun('请裁决');
    deliverEvent(runStarted(0));
    deliverEvent({
      seq: 1,
      turnId: 1,
      timestampMs: 1727000000000,
      kind: 'message',
      role: 'assistant',
      blocks: [
        {
          kind: 'toolUse',
          id: 'call_stable_ref',
          name: 'AskUserQuestion',
          input: { question: '引用稳定？', options: ['是', '否'] },
        },
      ],
      parentToolUseId: null,
    });
    await act(async () => {});

    const timeline = screen.getByTestId('agent-timeline');
    await within(timeline).findAllByTestId('ask-answer-option');
    const before = askAnswerProbe.refs.filter((ref) => ref !== null).at(-1) as {
      onAnswer: unknown;
    };

    // 与 showRaw 切换无关的父层重渲染（历史区来源筛选变更触发整页重渲染）
    askAnswerProbe.refs.length = 0;
    fireEvent.click(screen.getByTestId('toggle-raw'));
    fireEvent.click(screen.getByTestId('toggle-timeline'));
    await act(async () => {});

    const after = askAnswerProbe.refs.filter((ref) => ref !== null) as { onAnswer: unknown }[];
    expect(after.length).toBeGreaterThan(0);
    // 视图层每渲染新造 `{ onAnswer: session.answer }`，但字段引用稳定（不击穿 memo）
    for (const ref of after) expect(ref.onAnswer).toBe(before.onAnswer);
  });

  it('无 ask 部件时时间线零应答入口（既有形态零漂移）', async () => {
    render(<AgentDebugView root={ROOT} />);
    await startRun();
    deliverEvent(runStarted(0));
    deliverEvent(sealedTextEvent(1, '普通正文'));
    await waitFor(() =>
      expect(screen.getAllByTestId('block-text').map((node) => node.textContent)).toContain(
        '普通正文',
      ),
    );
    expect(screen.queryAllByTestId('ask-answer-option')).toHaveLength(0);
    expect(invokeMock.mock.calls.filter(([name]) => name === 'agent_answer')).toHaveLength(0);
  });
});
