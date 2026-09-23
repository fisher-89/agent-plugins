// @vitest-environment jsdom
import { fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vite-plus/test';

import type { AgentEvent } from '../../../types/dto';
import { AgentEventTimeline } from './AgentEventTimeline';

// ---------------------------------------------------------------------------
// fixture：五变体事件构造器（与 Rust serde camelCase 线格式同形）
// ---------------------------------------------------------------------------

function base(seq: number, timestampMs = 1727000000000) {
  return { seq, timestampMs };
}

function runStarted(seq: number): AgentEvent {
  return {
    ...base(seq),
    kind: 'runStarted',
    model: 'claude-opus',
    sessionId: 's-1',
    tools: ['Bash', 'Read'],
    mcpServers: ['mcp-a'],
  };
}

function message(
  seq: number,
  blocks: Extract<AgentEvent, { kind: 'message' }>['blocks'],
  parentToolUseId: string | null = null,
): AgentEvent {
  return { ...base(seq), kind: 'message', role: 'assistant', blocks, parentToolUseId };
}

function systemNotice(seq: number): AgentEvent {
  return { ...base(seq), kind: 'systemNotice', subtype: 'permission_denial', payload: {} };
}

function runResult(
  seq: number,
  fields: Partial<Extract<AgentEvent, { kind: 'runResult' }>> = {},
): AgentEvent {
  return {
    ...base(seq),
    kind: 'runResult',
    subtype: 'success',
    isError: false,
    numTurns: 3,
    durationMs: 1234,
    costUsd: 0.42,
    usage: {},
    sessionId: 's-1',
    ...fields,
  };
}

function raw(seq: number): AgentEvent {
  return {
    ...base(seq),
    kind: 'raw',
    eventType: 'mystery',
    rawJson: '{"type":"mystery","note":"保留原文"}',
  };
}

function mount(events: AgentEvent[], running = false) {
  render(<AgentEventTimeline events={events} running={running} />);
}

// ---------------------------------------------------------------------------
// 剪贴板 stub（jsdom 无实现；若复制走 navigator.clipboard 则断言调用）
// ---------------------------------------------------------------------------

function stubClipboard() {
  const writeText = vi.fn().mockResolvedValue(undefined);
  Object.defineProperty(window.navigator, 'clipboard', {
    configurable: true,
    value: { writeText },
  });
  return writeText;
}

afterEach(() => {
  const nav = window.navigator as unknown as { clipboard?: unknown };
  delete nav.clipboard;
});

describe('AgentEventTimeline：对话流与折叠块（AC-5 loop 可见性）', () => {
  it('assistant message 的 text / thinking 块按序呈现且 role 标识在场', () => {
    mount([
      message(1, [
        { kind: 'thinking', thinking: '先想想' },
        { kind: 'text', text: '正文内容' },
      ]),
    ]);

    const messageNode = screen.getByTestId('event-message');
    expect(messageNode.getAttribute('data-role')).toBe('assistant');
    const blocks = within(messageNode);
    expect(blocks.getByTestId('block-thinking').textContent).toContain('先想想');
    expect(blocks.getByTestId('block-text').textContent).toBe('正文内容');
    // DOM 序：thinking 在 text 之前（按块序呈现）
    expect(
      blocks
        .getByTestId('block-thinking')
        .compareDocumentPosition(blocks.getByTestId('block-text')) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
  });

  it('tool_use 与紧随的 tool_result 折叠块成对呈现（同 id 关联）', () => {
    mount([
      message(1, [{ kind: 'toolUse', id: 'tu_1', name: 'Bash', input: { command: 'ls' } }]),
      {
        ...base(2),
        kind: 'message',
        role: 'user',
        blocks: [{ kind: 'toolResult', id: 'tu_1', content: '目录内容', isError: false }],
        parentToolUseId: null,
      },
    ]);

    const toolUse = screen.getByTestId('block-tool-use');
    const toolResult = screen.getByTestId('block-tool-result');
    expect(toolUse.getAttribute('data-tool-id')).toBe('tu_1');
    expect(toolResult.getAttribute('data-tool-id')).toBe('tu_1');
    expect(toolUse.textContent).toContain('Bash');
    expect(toolResult.textContent).toContain('目录内容');
  });

  it('子代理归因：parentToolUseId 有值的消息归入对应父工具调用的分组内', () => {
    mount([
      message(1, [{ kind: 'toolUse', id: 'tu_1', name: 'Task', input: {} }]),
      message(2, [{ kind: 'text', text: '子代理产出' }], 'tu_1'),
    ]);

    const group = screen.getByTestId('subagent-group');
    expect(group.getAttribute('data-parent-id')).toBe('tu_1');
    const nested = within(group).getByTestId('event-message');
    expect(nested.getAttribute('data-parent-tool-use-id')).toBe('tu_1');
    expect(group.textContent).toContain('子代理产出');
    // 子代理消息不出现在顶层（归因分组而非平铺）
    expect(within(group).getByTestId('block-text')).toBeTruthy();
  });

  it('raw 事件以透传占位呈现（eventType + 原文入口）不崩', () => {
    mount([runStarted(0), raw(1)]);

    const rawNode = screen.getByTestId('event-raw');
    expect(rawNode.textContent).toContain('mystery');
    expect(rawNode.textContent).toContain('保留原文');
  });

  it('runStarted 与 systemNotice 分别呈现启动信息与通知行', () => {
    mount([runStarted(0), systemNotice(1)]);

    expect(screen.getByTestId('event-run-started').textContent).toContain('模型 claude-opus');
    expect(screen.getByTestId('event-system').textContent).toContain('permission_denial');
  });
});

describe('AgentEventTimeline：result 汇总卡与终态', () => {
  it('runResult 渲染轮数 / 成本 / 时长且 session 可复制（可复制断言经剪贴板 stub）', () => {
    const writeText = stubClipboard();
    mount([runResult(2)]);

    expect(screen.getByTestId('result-num-turns').textContent).toBe('3');
    expect(screen.getByTestId('result-cost').textContent).toBe('$0.42');
    expect(screen.getByTestId('result-duration').textContent).toBe('1234ms');

    const copyButton = screen.getByTestId('copy-value');
    // 值在复制按钮旁的 code 节点（CopyValue：code + 复制按钮同组）
    const copyGroup = copyButton.closest('span');
    expect(copyGroup?.textContent).toContain('s-1');
    fireEvent.click(copyButton);
    expect(writeText).toHaveBeenCalledWith('s-1');
  });

  it('汇总字段为 null（缺失）时呈现占位不崩', () => {
    mount([runResult(1, { numTurns: null, costUsd: null, durationMs: null, sessionId: null })]);

    expect(screen.getByTestId('result-num-turns').textContent).toBe('—');
    expect(screen.getByTestId('result-cost').textContent).toBe('—');
    expect(screen.getByTestId('result-duration').textContent).toBe('—');
    expect(screen.getByTestId('result-session').textContent).toBe('—');
    expect(screen.queryByTestId('copy-value')).toBeNull();
  });

  it('is_error 呈现：toolResult 出错标记 + failed result 终态失败标识', () => {
    mount([
      {
        ...base(1),
        kind: 'message',
        role: 'user',
        blocks: [{ kind: 'toolResult', id: 'tu_9', content: '炸了', isError: true }],
        parentToolUseId: null,
      },
      runResult(2, { isError: true, subtype: 'error_max_turns' }),
    ]);

    const toolResult = screen.getByTestId('block-tool-result');
    expect(toolResult.getAttribute('data-is-error')).toBe('true');
    expect(toolResult.textContent).toContain('出错');

    const card = screen.getByTestId('event-result');
    expect(card.getAttribute('data-is-error')).toBe('true');
    expect(card.textContent).toContain('失败');
  });
});

describe('AgentEventTimeline：空态 / running / 容量（边界）', () => {
  it('events 为空时呈现空态占位不崩', () => {
    mount([]);

    expect(screen.getByTestId('timeline-empty') !== null).toBe(true);
  });

  it('running=true 呈现进行中标记；false 且无 result 时呈静止态', () => {
    const { unmount } = render(<AgentEventTimeline events={[message(0, [])]} running={true} />);
    expect(screen.getByTestId('timeline-running') !== null).toBe(true);
    unmount();

    mount([message(0, [])], false);
    expect(screen.queryByTestId('timeline-running')).toBeNull();
  });

  it('200 事件长列表全量渲染无丢失、超长 content 折叠不撑爆布局', () => {
    const longText = '长'.repeat(1200);
    const events = Array.from({ length: 200 }, (_, index) =>
      message(index, [{ kind: 'text', text: index === 100 ? longText : `正文 ${index}` }]),
    );
    mount(events);

    expect(screen.getAllByTestId('event-message')).toHaveLength(200);
    expect(screen.getByTestId('agent-timeline').textContent).toContain(longText);
  });
});
