// @vitest-environment jsdom
import { fireEvent, render, screen, within } from '@testing-library/react';
import { describe, expect, it } from 'vite-plus/test';

import type { AgentBlock, AgentEvent } from '../../../types/dto';
import { ExploreConversation } from './ExploreConversation';

// ---------------------------------------------------------------------------
// 组件直渲染（jsdom，事件 fixture 直喂，子件全真实实现）：四变体映射
// （Text→markdown、Thinking→折叠、ToolUse→卡片、ToolResult 成对）与
// AskUserQuestion 静态卡是本变更的明确验收面（AC-9）。
// ---------------------------------------------------------------------------

function base(seq: number) {
  return { seq, timestampMs: 1727000000000 };
}

function message(seq: number, blocks: AgentBlock[], role = 'assistant'): AgentEvent {
  return { ...base(seq), kind: 'message', role, blocks, parentToolUseId: null };
}

function runResult(seq: number, isError = false): AgentEvent {
  return {
    ...base(seq),
    kind: 'runResult',
    subtype: isError ? 'error_max_turns' : 'success',
    isError,
    numTurns: 2,
    durationMs: 800,
    costUsd: 0.2,
    usage: {},
    sessionId: 's-1',
  };
}

function renderConversation(
  events: AgentEvent[],
  options?: { loading?: boolean; running?: boolean },
) {
  return render(
    <ExploreConversation
      events={events}
      loading={options?.loading ?? false}
      running={options?.running ?? false}
    />,
  );
}

describe('ExploreConversation：四变体块映射（AC-9）', () => {
  it('Message.blocks 含 Text 块：以 markdown 形态渲染（标题/加粗成元素而非纯文本）', () => {
    renderConversation([message(0, [{ kind: 'text', text: '## 探索结论\n这是**加粗**要点' }])]);

    const block = screen.getByTestId('block-text');
    expect(within(block).getByRole('heading', { level: 2 })?.textContent).toContain('探索结论');
    expect(within(block).getByRole('strong')?.textContent).toBe('加粗');
  });

  it('Thinking 块：呈折叠形态（details/summary），展开后内容可读', () => {
    renderConversation([message(0, [{ kind: 'thinking', thinking: '先查一下调用链' }])]);

    const thinking = screen.getByTestId('block-thinking');
    expect(thinking.tagName).toBe('DETAILS');
    fireEvent.click(within(thinking).getByText('思考'));
    expect(thinking.textContent).toContain('先查一下调用链');
  });

  it('ToolUse（Bash）块：呈卡片形态，工具名与 input 可读', () => {
    renderConversation([
      message(0, [{ kind: 'toolUse', id: 'tu_1', name: 'Bash', input: { command: 'ls' } }]),
      message(1, [{ kind: 'toolResult', id: 'tu_1', content: 'a.txt', isError: false }], 'user'),
    ]);

    const card = screen.getByTestId('block-tool-use');
    expect(card.getAttribute('data-tool-id')).toBe('tu_1');
    expect(within(card).getByText('Bash') !== null).toBe(true);
    expect(within(card).getByTestId('block-tool-result')?.textContent).toContain('a.txt');
  });

  it('AskUserQuestion ToolUse：静态可读卡片（问题与选项原样呈现），无可交互作答控件', () => {
    renderConversation([
      message(0, [
        {
          kind: 'toolUse',
          id: 'tu_ask',
          name: 'AskUserQuestion',
          input: {
            questions: [
              {
                question: '更倾向哪种重试策略？',
                options: [
                  { label: '指数退避', description: '适合限流场景' },
                  { label: '固定间隔', description: null },
                ],
              },
            ],
          },
        },
      ]),
    ]);

    const card = screen.getByTestId('ask-question-card');
    expect(within(card).getByTestId('ask-question')?.textContent).toContain('更倾向哪种重试策略？');
    const options = within(card).getAllByTestId('ask-option');
    expect(options).toHaveLength(2);
    expect(options[0].textContent).toContain('指数退避');
    expect(options[0].textContent).toContain('适合限流场景');
    // MVP 静态卡：卡内无按钮/输入/下拉等作答控件（作答经 composer + resume）
    expect(within(card).queryAllByRole('button')).toHaveLength(0);
    expect(within(card).queryAllByRole('textbox')).toHaveLength(0);
    expect(within(card).queryAllByRole('combobox')).toHaveLength(0);
  });
});

describe('ExploreConversation：成对与空态（AC-9）', () => {
  it('ToolUse 与同 id ToolResult 跨消息成对呈现；重复结果只取首个不重复渲染', () => {
    renderConversation([
      message(0, [{ kind: 'toolUse', id: 'tu_1', name: 'Read', input: { path: 'x' } }]),
      message(
        1,
        [{ kind: 'toolResult', id: 'tu_1', content: '第一次结果', isError: false }],
        'user',
      ),
      message(2, [{ kind: 'toolResult', id: 'tu_1', content: '重复结果', isError: false }], 'user'),
    ]);

    const cards = screen.getAllByTestId('block-tool-use');
    expect(cards).toHaveLength(1);
    expect(within(cards[0]).getAllByTestId('block-tool-result')).toHaveLength(1);
    expect(within(cards[0]).getByTestId('block-tool-result')?.textContent).toContain('第一次结果');
  });

  it('孤立 ToolResult（无同 id ToolUse）：不悬挂不崩，不渲染悬挂卡片', () => {
    renderConversation([
      message(
        0,
        [{ kind: 'toolResult', id: 'tu_ghost', content: '孤立结果', isError: false }],
        'user',
      ),
    ]);

    // 无同 id ToolUse 的结果不成卡（collectResults 全量收集 → 结果分支被跳过）：
    // 气泡仍在、无悬挂节点、无异常
    expect(screen.queryAllByTestId('block-tool-use')).toHaveLength(0);
    expect(screen.queryAllByTestId('block-tool-result')).toHaveLength(0);
    expect(screen.getAllByTestId('chat-bubble')).toHaveLength(1);
  });

  it('错误 ToolResult：工具卡带出错标记、结果区呈失败形态', () => {
    renderConversation([
      message(0, [{ kind: 'toolUse', id: 'tu_err', name: 'Bash', input: {} }]),
      message(1, [{ kind: 'toolResult', id: 'tu_err', content: 'boom', isError: true }], 'user'),
    ]);

    const result = screen.getByTestId('block-tool-result');
    expect(result.textContent).toContain('boom');
    expect(within(screen.getByTestId('block-tool-use')).getByText('出错') !== null).toBe(true);
  });

  it('空事件序列：空态呈现不崩', () => {
    renderConversation([], { loading: false });

    expect(screen.getByTestId('conversation-empty') !== null).toBe(true);
  });

  it('loading 且无事件：会话还原中提示在场（不以空态顶替）', () => {
    renderConversation([], { loading: true });

    expect(screen.getByTestId('conversation-loading') !== null).toBe(true);
    expect(screen.queryByTestId('conversation-empty')).toBeNull();
  });

  it('runResult 汇总卡：轮数/成本/时长/session 可读', () => {
    renderConversation([runResult(0)]);

    const summary = screen.getByTestId('result-summary');
    expect(summary.textContent).toContain('轮数：2');
    expect(summary.textContent).toContain('$0.2');
    expect(summary.textContent).toContain('800ms');
    expect(summary.textContent).toContain('s-1');
  });

  it('raw 事件透传：未识别事件呈现原文形态', () => {
    renderConversation([
      { ...base(0), kind: 'raw', eventType: 'mystery', rawJson: '{"type":"mystery"}' },
    ]);

    expect(screen.getByTestId('event-raw')?.textContent).toContain('{"type":"mystery"}');
  });
});

// ---------------------------------------------------------------------------
// 强化断言（变异补杀）：辅助事件行分发、AskUserQuestion 宽容解析降级、工具卡
// 结果形态、GFM 渲染、汇总卡缺省值、气泡角色形态与容器状态分支。
// ---------------------------------------------------------------------------

function systemNotice(seq: number, subtype: string): AgentEvent {
  return { ...base(seq), kind: 'systemNotice', subtype, payload: { detail: '内部细节' } };
}

function runStarted(seq: number): AgentEvent {
  return {
    ...base(seq),
    kind: 'runStarted',
    model: null,
    sessionId: null,
    tools: [],
    mcpServers: [],
  };
}

describe('ExploreConversation：辅助事件行分发（systemNotice / runStarted）', () => {
  it('systemNotice 事件：呈 info 辅助行（subtype 文案完整），不透传为 raw 原文', () => {
    renderConversation([
      message(0, [{ kind: 'text', text: '正文' }]),
      systemNotice(1, 'quota_watch'),
    ]);

    expect(screen.getByTestId('event-info')?.textContent).toBe('system 通知（quota_watch）');
    expect(screen.queryByTestId('event-raw')).toBeNull();
    expect(screen.queryByTestId('conversation-empty')).toBeNull();
  });

  it('runStarted 事件：呈 run 启动辅助行，同样不透传为 raw 原文', () => {
    renderConversation([runStarted(0)]);

    expect(screen.getByTestId('event-info')?.textContent).toBe('run 启动');
    expect(screen.queryByTestId('event-raw')).toBeNull();
  });
});

describe('ExploreConversation：AskUserQuestion 宽容解析（未知形态降级）', () => {
  it('input 为 null：降级呈现 JSON 原文（null），不崩、不渲染问题列表', () => {
    renderConversation([
      message(0, [{ kind: 'toolUse', id: 'tu_null', name: 'AskUserQuestion', input: null }]),
    ]);

    const card = screen.getByTestId('ask-question-card');
    expect(within(card).queryByTestId('ask-question')).toBeNull();
    expect(card.querySelector('pre')?.textContent).toBe('null');
  });

  it('input 无 questions 字段：降级呈现 JSON 原文（{}），不崩', () => {
    renderConversation([
      message(0, [{ kind: 'toolUse', id: 'tu_bare', name: 'AskUserQuestion', input: {} }]),
    ]);

    const card = screen.getByTestId('ask-question-card');
    expect(within(card).queryByTestId('ask-question')).toBeNull();
    expect(card.querySelector('pre')?.textContent).toBe('{}');
  });

  it('questions 元素 options 非数组：问题照常呈现，选项列表为空不崩', () => {
    renderConversation([
      message(0, [
        {
          kind: 'toolUse',
          id: 'tu_noopt',
          name: 'AskUserQuestion',
          input: { questions: [{ question: '没有选项的问题？', options: 'not-an-array' }] },
        },
      ]),
    ]);

    const card = screen.getByTestId('ask-question-card');
    expect(within(card).getByTestId('ask-question')?.textContent).toBe('没有选项的问题？');
    expect(within(card).queryAllByTestId('ask-option')).toHaveLength(0);
  });

  it('question / label 非字符串：降级为空串呈现（不透传原值）', () => {
    renderConversation([
      message(0, [
        {
          kind: 'toolUse',
          id: 'tu_loose',
          name: 'AskUserQuestion',
          input: { questions: [{ question: 42, options: [{ label: 42, description: null }] }] },
        },
      ]),
    ]);

    const card = screen.getByTestId('ask-question-card');
    expect(within(card).getByTestId('ask-question')?.textContent).toBe('');
    expect(within(card).getAllByTestId('ask-option')[0]?.textContent).toBe('');
  });

  it('description 非字符串或 null：不渲染描述 span（选项文本不被污染）', () => {
    renderConversation([
      message(0, [
        {
          kind: 'toolUse',
          id: 'tu_desc',
          name: 'AskUserQuestion',
          input: {
            questions: [
              {
                question: '带描述问题？',
                options: [
                  { label: '甲', description: 42 },
                  { label: '乙', description: null },
                ],
              },
            ],
          },
        },
      ]),
    ]);

    const options = screen.getAllByTestId('ask-option');
    expect(options).toHaveLength(2);
    expect(options[0]?.textContent).toBe('甲');
    expect(options[0]?.querySelector('span')).toBeNull();
    expect(options[1]?.textContent).toBe('乙');
    expect(options[1]?.querySelector('span')).toBeNull();
  });
});

describe('ExploreConversation：工具对卡孤立与形态', () => {
  it('孤立 ToolUse（无同 id 结果）：卡片照常呈现，无出错标记、无结果区、不崩', () => {
    renderConversation([
      message(0, [{ kind: 'toolUse', id: 'tu_solo', name: 'Grep', input: { pattern: 'explore' } }]),
    ]);

    const card = screen.getByTestId('block-tool-use');
    expect(card.getAttribute('data-tool-id')).toBe('tu_solo');
    expect(within(card).getByText('Grep') !== null).toBe(true);
    expect(within(card).queryByText('出错')).toBeNull();
    expect(within(card).queryByTestId('block-tool-result')).toBeNull();
  });

  it('成对非错误结果：无出错标记，结果区呈常规形态（bg-muted）', () => {
    renderConversation([
      message(0, [{ kind: 'toolUse', id: 'tu_ok', name: 'Read', input: { path: 'a.md' } }]),
      message(
        1,
        [{ kind: 'toolResult', id: 'tu_ok', content: '文件内容', isError: false }],
        'user',
      ),
    ]);

    const card = screen.getByTestId('block-tool-use');
    expect(within(card).queryByText('出错')).toBeNull();
    const result = within(card).getByTestId('block-tool-result');
    expect(result.textContent).toBe('文件内容');
    expect(result.className).toContain('bg-muted');
    expect(result.className).not.toContain('bg-fail-bg');
  });

  it('成对错误结果：结果区呈失败形态（bg-fail-bg / text-fail）', () => {
    renderConversation([
      message(0, [{ kind: 'toolUse', id: 'tu_bad', name: 'Bash', input: {} }]),
      message(1, [{ kind: 'toolResult', id: 'tu_bad', content: 'boom', isError: true }], 'user'),
    ]);

    const result = screen.getByTestId('block-tool-result');
    expect(result.textContent).toContain('boom');
    expect(result.className).toContain('bg-fail-bg');
    expect(result.className).toContain('text-fail');
  });
});

describe('ExploreConversation：markdown GFM 渲染', () => {
  it('GFM 扩展语法（删除线）按 del 元素渲染而非字面文本', () => {
    renderConversation([message(0, [{ kind: 'text', text: '~~旧结论~~ 新结论' }])]);

    const block = screen.getByTestId('block-text');
    expect(block.querySelector('del')?.textContent).toBe('旧结论');
  });
});

describe('ExploreConversation：runResult 汇总卡缺省值与失败形态', () => {
  it('全缺省成功结果：轮数/成本/时长/session 均呈 — 占位，头部为成功形态', () => {
    renderConversation([
      {
        ...base(0),
        kind: 'runResult',
        subtype: 'success',
        isError: false,
        numTurns: null,
        durationMs: null,
        costUsd: null,
        usage: {},
        sessionId: null,
      },
    ]);

    const card = screen.getByTestId('event-result');
    expect(card.getAttribute('data-is-error')).toBe('false');
    expect(card.className).toContain('bg-muted');
    expect(card.textContent).toContain('· 成功');
    const summary = screen.getByTestId('result-summary');
    expect(summary.textContent).toContain('轮数：—');
    expect(summary.textContent).toContain('成本：—');
    expect(summary.textContent).not.toContain('$');
    expect(summary.textContent).toContain('时长：—');
    expect(summary.textContent).not.toContain('ms');
    expect(summary.textContent).toContain('session：—');
  });

  it('失败结果：头部为失败形态、卡片呈失败配色（bg-fail-bg）', () => {
    renderConversation([
      {
        ...base(0),
        kind: 'runResult',
        subtype: 'error_max_turns',
        isError: true,
        numTurns: 9,
        durationMs: 1200,
        costUsd: 0.5,
        usage: {},
        sessionId: 's-err',
      },
    ]);

    const card = screen.getByTestId('event-result');
    expect(card.getAttribute('data-is-error')).toBe('true');
    expect(card.className).toContain('bg-fail-bg');
    expect(card.textContent).toContain('· 失败');
    expect(screen.getByTestId('result-summary').textContent).toContain('轮数：9');
  });
});

describe('ExploreConversation：气泡角色形态与容器状态分支', () => {
  it('user 消息：气泡靠右（align end）且呈 bg-muted 内容形态', () => {
    renderConversation([message(0, [{ kind: 'text', text: '用户输入' }], 'user')]);

    const bubble = screen.getByTestId('chat-bubble');
    expect(bubble.getAttribute('data-role')).toBe('user');
    expect(bubble.getAttribute('data-align')).toBe('end');
    const content = bubble.querySelector('[data-slot="message-content"]');
    expect(content?.className).toContain('bg-muted');
    expect(content?.className).not.toContain('bg-card');
  });

  it('assistant 消息：气泡靠左（align start）且呈 bg-card 内容形态', () => {
    renderConversation([message(0, [{ kind: 'text', text: '助手回答' }])]);

    const bubble = screen.getByTestId('chat-bubble');
    expect(bubble.getAttribute('data-role')).toBe('assistant');
    expect(bubble.getAttribute('data-align')).toBe('start');
    const content = bubble.querySelector('[data-slot="message-content"]');
    expect(content?.className).toContain('bg-card');
    expect(content?.className).not.toContain('bg-muted');
  });

  it('有条目时：不呈现空态占位，loading 组合也不误显还原提示', () => {
    renderConversation([message(0, [{ kind: 'text', text: '已有内容' }])], { loading: true });

    expect(screen.queryByTestId('conversation-empty')).toBeNull();
    expect(screen.queryByTestId('conversation-loading')).toBeNull();
    expect(screen.getByTestId('block-text') !== null).toBe(true);
  });
});
