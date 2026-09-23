// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vite-plus/test';

import type { AgentEvent } from '../../../types/dto';
import { AgentRawStream } from './AgentRawStream';

// 纯 props 渲染，无进程边界，不需要 Mock。

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

function raw(seq: number, rawJson: string): AgentEvent {
  return {
    seq,
    timestampMs: 1727000000001,
    kind: 'raw',
    eventType: 'unparsable',
    rawJson,
  };
}

function textMessage(seq: number, text: string): AgentEvent {
  return {
    seq,
    timestampMs: 1727000000002,
    kind: 'message',
    role: 'assistant',
    blocks: [{ kind: 'text', text }],
    parentToolUseId: null,
  };
}

/** 读取某 seq 行渲染的 JSON dump（事件落库形态）。 */
function rawLineJson(seq: number): Record<string, unknown> {
  const node = screen
    .getAllByTestId('raw-line')
    .find((line) => line.getAttribute('data-seq') === seq.toString());
  if (!node) throw new Error(`seq=${seq} 的 raw-line 不存在`);
  return JSON.parse(node.textContent ?? '{}') as Record<string, unknown>;
}

describe('AgentRawStream：逐事件 JSON dump（D12 落库形态）', () => {
  it('每事件渲染一行 JSON，键形态与线格式一致（camelCase）', () => {
    render(<AgentRawStream events={[runStarted(0), textMessage(1, '正文')]} />);

    const dumped = rawLineJson(0);
    expect(dumped['kind']).toBe('runStarted');
    expect(dumped).toHaveProperty('timestampMs');
    expect(dumped).toHaveProperty('sessionId');
    expect(dumped).toHaveProperty('mcpServers');
    expect(rawLineJson(1)).toHaveProperty('parentToolUseId');
  });

  it('raw 变体呈现 rawJson 原文（dump 解析回读不二次转义失真）', () => {
    const original = '{"type":"mystery","text":"第一行\\n\\"quoted\\" 🎉"}';
    render(<AgentRawStream events={[raw(3, original)]} />);

    const dumped = rawLineJson(3);
    expect(dumped['eventType']).toBe('unparsable');
    expect(dumped['rawJson']).toBe(original);
  });

  it('events 为空时呈现空面板不崩', () => {
    render(<AgentRawStream events={[]} />);

    expect(screen.getByTestId('raw-stream-empty') !== null).toBe(true);
    expect(screen.queryAllByTestId('raw-line')).toHaveLength(0);
  });

  it('含中文 / emoji / 换行的 payload 完整呈现', () => {
    render(<AgentRawStream events={[textMessage(4, '多行\n中文 🎉 "quoted"')]} />);

    const line = screen.getByTestId('raw-line');
    expect(line.getAttribute('data-seq')).toBe('4');
    const dumped = JSON.parse(line.textContent ?? '{}') as {
      blocks: { text: string }[];
    };
    expect(dumped['blocks'][0]?.['text']).toBe('多行\n中文 🎉 "quoted"');
  });

  it('大事件列表（300 条）渲染不崩且全量在场', () => {
    const events = Array.from({ length: 300 }, (_, index) => textMessage(index, `事件 ${index}`));
    render(<AgentRawStream events={events} />);

    expect(screen.getAllByTestId('raw-line')).toHaveLength(300);
  });
});
