// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { eventsToUIMessages, type AgentUIMessage } from '../../lib/agent-adapter';
import type { AgentBlock, AgentCitation, AgentEvent } from '../../types/dto';
import { AgentMessages } from './agent-messages';

// ---------------------------------------------------------------------------
// 流式渲染的解析边界：react-markdown 保留真实实现，仅用 spy 观测调用次数。
// fixture 必须模拟 @ai-sdk/react store 的更新形态：历史消息对象引用不变，
// 当前流式消息及其全部 part 被浅拷贝（part 外层新引用、嵌套 input 引用不变）。
// ---------------------------------------------------------------------------

const { markdownSpy } = vi.hoisted(() => ({ markdownSpy: vi.fn() }));

vi.mock(import('react-markdown'), async (importOriginal) => {
  const actual = await importOriginal();
  markdownSpy.mockImplementation(actual.default);
  return { ...actual, default: markdownSpy };
});

const TS = 1727000000000;
const TOOL_INPUT = { command: 'ls' };

function citation(url: string, title: string | null = null): AgentCitation {
  return { url, title, citedText: null, raw: { url } };
}

function message(seq: number, blocks: AgentBlock[]): AgentEvent {
  return {
    seq,
    timestampMs: TS,
    kind: 'message',
    role: 'assistant',
    blocks,
    parentToolUseId: null,
  };
}

/** 基线：历史文本 + [带引用文本、未落定工具、流式末尾文本] */
function baseEvents(): AgentEvent[] {
  return [
    message(0, [{ kind: 'text', text: '历史结论' }]),
    message(1, [
      { kind: 'text', text: '引用正文', citations: [citation('https://cite/1', '旧来源')] },
      { kind: 'toolUse', id: 'tu_1', name: 'Bash', input: TOOL_INPUT },
      { kind: 'text', text: '甲' },
    ]),
  ];
}

/** store 浅拷贝形态：只重建最后一条消息与全部 part 外层，仅改末尾文本 */
function streamNext(previous: AgentUIMessage[], text: string): AgentUIMessage[] {
  const last = previous.at(-1);
  if (last === undefined) throw new Error('fixture 缺少消息');
  const parts = last.parts.map((part) => ({ ...part }));
  const lastIndex = parts.length - 1;
  const lastPart = parts[lastIndex];
  if (lastPart === undefined || lastPart.type !== 'text') throw new Error('fixture 末件非文本');
  parts[lastIndex] = { ...lastPart, text };
  return [...previous.slice(0, -1), { ...last, parts }];
}

/** 用适配层真实产出（含 providerMetadata.agent.citations）替换首个 text 部件的引用 */
function withNewCitations(previous: AgentUIMessage[], next: AgentCitation): AgentUIMessage[] {
  const last = previous.at(-1);
  if (last === undefined) throw new Error('fixture 缺少消息');
  const source = eventsToUIMessages([
    message(1, [{ kind: 'text', text: '引用正文', citations: [next] }]),
  ])[0];
  const sourcePart = source?.parts[0];
  if (sourcePart === undefined || sourcePart.type !== 'text') throw new Error('fixture 引用缺失');
  const parts = last.parts.map((part) => ({ ...part }));
  const firstPart = parts[0];
  if (firstPart === undefined || firstPart.type !== 'text') throw new Error('fixture 首件非文本');
  parts[0] = { ...firstPart, providerMetadata: sourcePart.providerMetadata };
  return [...previous.slice(0, -1), { ...last, parts }];
}

function markdownCalls(text: string): number {
  return markdownSpy.mock.calls.filter(
    ([props]) => (props as { children?: unknown }).children === text,
  ).length;
}

beforeEach(() => {
  markdownSpy.mockClear();
});

describe('AgentMessages：流式重渲染只解析变化的 markdown', () => {
  it('历史消息、未变文本、未变引用全部跳过；仅流式末尾文本重解析', () => {
    const first = eventsToUIMessages(baseEvents());
    const { rerender } = render(<AgentMessages messages={first} />);

    expect(markdownCalls('历史结论')).toBe(1);
    expect(markdownCalls('引用正文')).toBe(1);
    expect(markdownCalls('甲')).toBe(1);

    const stringifySpy = vi.spyOn(JSON, 'stringify');
    try {
      markdownSpy.mockClear();
      rerender(<AgentMessages messages={streamNext(first, '甲乙')} />);

      expect(markdownCalls('历史结论')).toBe(0);
      expect(markdownCalls('引用正文')).toBe(0);
      expect(markdownCalls('甲')).toBe(0);
      expect(markdownCalls('甲乙')).toBe(1);
      // 流式消息的 part 外层是新引用，但嵌套 input 引用未变 → 工具卡不必重序列化
      expect(stringifySpy.mock.calls.some(([value]) => value === TOOL_INPUT)).toBe(false);
    } finally {
      stringifySpy.mockRestore();
    }
  });

  it('citations 变化照常更新引用列表，但正文 markdown 不重解析', () => {
    const first = eventsToUIMessages(baseEvents());
    const { rerender } = render(<AgentMessages messages={first} />);

    markdownSpy.mockClear();
    rerender(
      <AgentMessages messages={withNewCitations(first, citation('https://cite/2', '新来源'))} />,
    );

    expect(screen.getByTestId('block-citations').textContent).toContain('新来源');
    expect(markdownCalls('引用正文')).toBe(0);
  });
});
