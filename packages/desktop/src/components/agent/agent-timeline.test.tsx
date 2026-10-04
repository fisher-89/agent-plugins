// @vitest-environment jsdom
import { fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vite-plus/test';

import { eventsToUIMessages, runRecordToUIMessage } from '../../lib/agent-adapter';
import type { AgentBlock, AgentEvent, TurnSummary } from '../../types/dto';
import { AgentTimeline } from './agent-timeline';

// ---------------------------------------------------------------------------
// 纯渲染组件，无进程边界，不需要 Mock。消息序列经适配层真实输出构造
// （eventsToUIMessages / runRecordToUIMessage），与页面消费形状一致。
// ---------------------------------------------------------------------------

const TS = 1727000000000;

function message(
  seq: number,
  blocks: AgentBlock[],
  parentToolUseId: string | null = null,
): AgentEvent {
  return { seq, timestampMs: TS, kind: 'message', role: 'assistant', blocks, parentToolUseId };
}

function userMessage(seq: number, text: string): AgentEvent {
  return {
    seq,
    timestampMs: TS,
    kind: 'message',
    role: 'user',
    blocks: [{ kind: 'text', text }],
    parentToolUseId: null,
  };
}

function runStarted(seq: number): AgentEvent {
  return {
    seq,
    timestampMs: TS,
    kind: 'runStarted',
    model: 'claude-opus',
    sessionId: 's-1',
    tools: ['Bash', 'Read'],
    mcpServers: ['mcp-a'],
  };
}

function systemNotice(seq: number): AgentEvent {
  return { seq, timestampMs: TS, kind: 'systemNotice', subtype: 'api_retry', payload: {} };
}

function turnDone(seq: number, usage: unknown = {}): AgentEvent {
  return {
    seq,
    timestampMs: TS,
    kind: 'turnDone',
    subtype: 'success',
    isError: false,
    numTurns: 3,
    durationMs: 1234,
    costUsd: 0.42,
    usage,
    sessionId: 'ses-0-1727000000000',
  };
}

function raw(seq: number): AgentEvent {
  return {
    seq,
    timestampMs: TS,
    kind: 'raw',
    eventType: 'mystery',
    rawJson: '{"type":"mystery","note":"保留原文"}',
  };
}

function recordRow(status: TurnSummary['status']): TurnSummary {
  return {
    turnId: 5,
    sessionId: 'ses-0-1727000000000',
    status,
    startedAt: TS,
    finishedAt: TS + 999,
    numTurns: 3,
    costUsd: 0.42,
    durationMs: 1234,
    error: null,
  };
}

/** jsdom 无剪贴板实现：以可配置属性替换后断言调用。 */
function stubClipboard(): ReturnType<typeof vi.fn> {
  const writeText = vi.fn().mockResolvedValue(undefined);
  Object.defineProperty(navigator, 'clipboard', {
    value: { writeText },
    configurable: true,
  });
  return writeText;
}

afterEach(() => {
  // @ts-expect-error 测试替身：还原 clipboard 缺省形态
  delete navigator.clipboard;
});

// ---------------------------------------------------------------------------
// seq 序与块呈现
// ---------------------------------------------------------------------------

describe('AgentTimeline：seq 序', () => {
  it('输入按 seq 升序（适配层产出口径）→ 呈现序与 seq 序一致（system 载体全量在内）', () => {
    const events = [
      runStarted(0),
      userMessage(1, '第一条'),
      message(2, [{ kind: 'text', text: '第二条' }]),
    ];
    render(<AgentTimeline messages={eventsToUIMessages(events)} running={false} />);

    const nodes = screen.getAllByTestId('event-message');
    // 全量呈现：system 载体（run-started）与两条 message 消息共 3 节点
    expect(nodes).toHaveLength(3);
    expect(nodes[0]?.getAttribute('data-role')).toBe('system');
    expect(nodes[0]?.textContent).toContain('run 启动');
    // DOM 序与 seq 序一致：user 消息（seq 1）在 assistant 消息（seq 2）之前
    expect(
      nodes[1]?.compareDocumentPosition(nodes[2]) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    expect(nodes[1]?.textContent).toContain('第一条');
    expect(nodes[2]?.textContent).toContain('第二条');
    // seq 保真在 metadata（DOM 属性以 parentToolUseId 标注归因，无归因为 null）
    expect(nodes[1]?.getAttribute('data-parent-tool-use-id')).toBeNull();
  });

  it('同 seq 保输入序：两条 seq 相同的消息按入参顺序呈现', () => {
    const first = eventsToUIMessages([message(1, [{ kind: 'text', text: '先到' }])])[0];
    const second = eventsToUIMessages([message(1, [{ kind: 'text', text: '后到' }])])[0];
    render(<AgentTimeline messages={[first, second]} running={false} />);

    const nodes = screen.getAllByTestId('event-message');
    expect(
      nodes[0]?.compareDocumentPosition(nodes[1]) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    expect(nodes[0]?.textContent).toContain('先到');
    expect(nodes[1]?.textContent).toContain('后到');
  });

  it('assistant 的 text / thinking 块按序展开呈现且 role 标识在场', () => {
    const events = [
      message(1, [
        { kind: 'thinking', thinking: '先想想' },
        { kind: 'text', text: '正文内容' },
      ]),
    ];
    render(<AgentTimeline messages={eventsToUIMessages(events)} running={false} />);

    const messageNode = screen.getByTestId('event-message');
    expect(messageNode.getAttribute('data-role')).toBe('assistant');
    const blocks = within(messageNode);
    expect(blocks.getByTestId('block-thinking').textContent).toContain('先想想');
    expect(blocks.getByTestId('block-text').textContent).toBe('正文内容');
    expect(
      blocks
        .getByTestId('block-thinking')
        .compareDocumentPosition(blocks.getByTestId('block-text')) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
  });
});

// ---------------------------------------------------------------------------
// 子代理归因分组
// ---------------------------------------------------------------------------

describe('AgentTimeline：子代理归因分组', () => {
  it('携带 parentToolUseId 的消息归入对应父工具卡下，归因头与组内顺序正确', () => {
    const events = [
      userMessage(0, '跑个任务'),
      message(1, [{ kind: 'toolUse', id: 'tu_1', name: 'Task', input: {} }]),
      message(2, [{ kind: 'text', text: '子代理第一段' }], 'tu_1'),
      message(3, [{ kind: 'text', text: '子代理第二段' }], 'tu_1'),
      message(4, [{ kind: 'text', text: '顶层收尾' }]),
    ];
    render(<AgentTimeline messages={eventsToUIMessages(events)} running={false} />);

    const group = screen.getByTestId('subagent-group');
    expect(group.getAttribute('data-parent-id')).toBe('tu_1');
    expect(group.textContent).toContain('子代理（父工具调用 tu_1）');
    const nested = within(group).getAllByTestId('event-message');
    expect(nested).toHaveLength(2);
    expect(nested[0]?.getAttribute('data-parent-tool-use-id')).toBe('tu_1');
    expect(nested[0]?.textContent).toContain('子代理第一段');
    expect(nested[1]?.textContent).toContain('子代理第二段');
    // 归因组嵌在父工具卡内
    const toolCard = screen.getByTestId('block-tool-use');
    expect(toolCard.getAttribute('data-tool-id')).toBe('tu_1');
    expect(toolCard.contains(group)).toBe(true);
    // 全量节点：3 条顶层 + 2 条嵌套（归因分组而非平铺）
    expect(screen.getAllByTestId('event-message')).toHaveLength(5);
  });

  it('MessageScrollerItem 以 top-level 消息为单位，嵌套组随父项滚动', () => {
    const events = [
      message(1, [{ kind: 'toolUse', id: 'tu_1', name: 'Task', input: {} }]),
      message(2, [{ kind: 'text', text: '子代理产出' }], 'tu_1'),
      message(3, [{ kind: 'text', text: '顶层消息' }]),
    ];
    render(<AgentTimeline messages={eventsToUIMessages(events)} running={false} />);

    const items = document.querySelectorAll('[data-slot="message-scroller-item"]');
    expect(items).toHaveLength(2);
  });
});

// ---------------------------------------------------------------------------
// 零部件不变式：配对折叠后的工具结果载体不产生空行
// ---------------------------------------------------------------------------

/** 工具结果消息（role 可选：新协议 tool / 存量转录 user 形态） */
function toolResultMessage(seq: number, id: string, role: 'tool' | 'user' = 'tool'): AgentEvent {
  return {
    seq,
    timestampMs: TS,
    kind: 'message',
    role,
    blocks: [{ kind: 'toolResult', id, content: '工具产物', isError: false }],
    parentToolUseId: null,
  };
}

describe('AgentTimeline：零部件消息不产生行', () => {
  it('tool role 结果消息（已配对折叠）不产生顶层行，工具卡携带 output', () => {
    const events = [
      userMessage(0, '跑一下'),
      message(1, [{ kind: 'toolUse', id: 'tu_1', name: 'read', input: {} }]),
      toolResultMessage(2, 'tu_1'),
      message(3, [{ kind: 'text', text: '结论' }]),
    ];
    render(<AgentTimeline messages={eventsToUIMessages(events)} running={false} />);

    // 仅 user 提问 + assistant 工具卡 + assistant 结论三行，无空 user 行
    const nodes = screen.getAllByTestId('event-message');
    expect(nodes).toHaveLength(3);
    expect(nodes.map((node) => node.getAttribute('data-role'))).toEqual([
      'user',
      'assistant',
      'assistant',
    ]);
    // 结果折叠进工具卡 output（对读呈现）
    const toolCard = screen.getByTestId('block-tool-use');
    expect(within(toolCard).getByTestId('block-tool-result').textContent).toContain('工具产物');
  });

  it('存量转录 user 形态的配对结果同样不产生行（零部件不变式与 role 无关）', () => {
    const events = [
      message(1, [{ kind: 'toolUse', id: 'tu_1', name: 'read', input: {} }]),
      toolResultMessage(2, 'tu_1', 'user'),
    ];
    render(<AgentTimeline messages={eventsToUIMessages(events)} running={false} />);

    expect(screen.getAllByTestId('event-message')).toHaveLength(1);
    expect(screen.queryByText('user')).toBeNull();
  });

  it('无主 toolResult（未配对）仍有部件，照常呈现占位卡', () => {
    const events = [toolResultMessage(2, 'tu_orphan')];
    render(<AgentTimeline messages={eventsToUIMessages(events)} running={false} />);

    const nodes = screen.getAllByTestId('event-message');
    expect(nodes).toHaveLength(1);
    expect(within(nodes[0]).getByTestId('block-tool-result').textContent).toContain('工具产物');
  });
});

// ---------------------------------------------------------------------------
// raw 透传与 result 汇总可复制
// ---------------------------------------------------------------------------

describe('AgentTimeline：raw 透传与可复制', () => {
  it('data-raw 原文透传呈现（eventType + 原文不解析）', () => {
    render(<AgentTimeline messages={eventsToUIMessages([raw(1)])} running={false} />);

    const rawNode = screen.getByTestId('event-raw');
    expect(rawNode.textContent).toContain('mystery');
    expect(rawNode.textContent).toContain('保留原文');
    expect(rawNode.textContent).toContain('未识别事件');
  });

  it('data-run-result 汇总卡呈现轮数 / 成本 / 时长且 session 可复制', () => {
    const writeText = stubClipboard();
    render(<AgentTimeline messages={eventsToUIMessages([turnDone(2)])} running={false} />);

    expect(screen.getByTestId('event-result').getAttribute('data-is-error')).toBe('false');
    expect(screen.getByTestId('result-num-turns').textContent).toBe('3');
    expect(screen.getByTestId('result-cost').textContent).toBe('$0.42');
    expect(screen.getByTestId('result-duration').textContent).toBe('1234ms');
    expect(screen.queryByTestId('result-tokens') === null).toBe(true);
    const copyButton = screen.getByTestId('copy-value');
    expect(copyButton.closest('span')?.textContent).toContain('ses-0-1727000000000');
    fireEvent.click(copyButton);
    expect(writeText).toHaveBeenCalledWith('ses-0-1727000000000');
  });

  it('usage 携带 snake_case token 字段时汇总卡追加 tokens 项（CLI result / SDK rig 同形）', () => {
    const usageBearing = turnDone(2, {
      input_tokens: 2,
      cache_read_input_tokens: 37376,
      output_tokens: 32,
      output_tokens_details: { thinking_tokens: 0 },
    });
    render(<AgentTimeline messages={eventsToUIMessages([usageBearing])} running={false} />);

    expect(screen.getByTestId('result-tokens').textContent).toBe('入 2 / 出 32');
  });
});

// ---------------------------------------------------------------------------
// 全量呈现（含 system 载体消息；与对话透镜过滤口径互补）
// ---------------------------------------------------------------------------

describe('AgentTimeline：全量呈现含 system', () => {
  it('run-started / system-notice / raw / record 部件均可见（AC-6 loop 可见性数据面）', () => {
    const messages = [
      ...eventsToUIMessages([runStarted(0), systemNotice(1), raw(2), turnDone(3)]),
      runRecordToUIMessage(recordRow('stopped')),
    ];
    render(<AgentTimeline messages={messages} running={false} />);

    expect(screen.getByTestId('event-run-started').textContent).toContain('claude-opus');
    expect(screen.getByTestId('event-run-started').textContent).toContain('MCP 1 个');
    expect(screen.getByTestId('event-system').textContent).toContain('api_retry');
    expect(screen.getByTestId('event-raw') !== null).toBe(true);
    expect(screen.getByTestId('event-result') !== null).toBe(true);
    const recordRowNode = screen.getByTestId('event-run-record');
    expect(recordRowNode.getAttribute('data-status')).toBe('stopped');
    expect(recordRowNode.textContent).toContain('已停止');
  });
});

// ---------------------------------------------------------------------------
// 区域滚动组合与空态
// ---------------------------------------------------------------------------

describe('AgentTimeline：区域滚动组合', () => {
  it('section 呈 flex 填充且内部为 message-scroller 组合（Provider autoScroll + 视口 + 到底按钮在场）', () => {
    render(<AgentTimeline messages={eventsToUIMessages([runStarted(0)])} running={false} />);

    const section = screen.getByTestId('agent-timeline');
    for (const className of ['flex', 'min-h-0', 'flex-1', 'flex-col']) {
      expect(section.className).toContain(className);
    }
    expect(document.querySelector('[data-slot="message-scroller"]') !== null).toBe(true);
    const viewport = document.querySelector('[data-slot="message-scroller-viewport"]');
    expect(viewport?.getAttribute('aria-label')).toBe('事件时间线');
    expect(document.querySelector('[data-slot="message-scroller-button"]') !== null).toBe(true);
  });

  it('历史重放嵌入态（普通块容器内）同组件无 props 分叉：className 与流式形态一致', () => {
    // 嵌入限高普通块容器（run-history 重放区形态）：组件不接嵌入 props，
    // 呈现结构（flex 填充类 + scroller 组合）与流式形态完全一致
    render(<AgentTimeline messages={eventsToUIMessages([runStarted(0)])} running={false} />);

    expect(screen.getByTestId('agent-timeline').className).toContain('flex-1');
    expect(document.querySelector('[data-slot="message-scroller-viewport"]') !== null).toBe(true);
  });

  it('running 标记：头部流式状态随 running 呈现与消退', () => {
    const messages = eventsToUIMessages([runStarted(0)]);
    const rendered = render(<AgentTimeline messages={messages} running={true} />);
    expect(screen.getByTestId('timeline-running') !== null).toBe(true);

    rendered.rerender(<AgentTimeline messages={messages} running={false} />);
    expect(screen.queryByTestId('timeline-running')).toBeNull();
  });
});

describe('AgentTimeline：空态', () => {
  it('空输入 → 空态呈现不抛错，无 scroller', () => {
    render(<AgentTimeline messages={[]} running={false} />);

    expect(screen.getByTestId('timeline-empty') !== null).toBe(true);
    expect(document.querySelector('[data-slot="message-scroller"]')).toBeNull();
  });
});
