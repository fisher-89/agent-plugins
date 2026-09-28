// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vite-plus/test';

import { ToolPairCard } from './agent-blocks';
import { DEFAULT_TOOL_CARDS } from './tool-cards';

// ---------------------------------------------------------------------------
// 纯渲染注册表，无进程边界，不需要 Mock。
// ---------------------------------------------------------------------------

const ASK_INPUT = {
  questions: [
    {
      question: '选择重试策略？',
      options: [
        { label: '指数退避', description: '推荐' },
        { label: '固定间隔', description: null },
      ],
    },
    {
      question: '是否同时清理缓存？',
      options: [{ label: '是', description: null }],
    },
  ],
};

// ---------------------------------------------------------------------------
// AskUserQuestion 特化卡
// ---------------------------------------------------------------------------

describe('DEFAULT_TOOL_CARDS：AskUserQuestion 特化', () => {
  it('toolName 为 AskUserQuestion 时命中注册表，问题与选项结构化呈现', () => {
    const Card = DEFAULT_TOOL_CARDS['AskUserQuestion'];
    expect(Card).toBeDefined();

    render(<Card toolCallId="tu_ask" toolName="AskUserQuestion" input={ASK_INPUT} output={null} />);

    expect(screen.getByTestId('ask-question-card').textContent).toContain('请在下方输入框作答');
    const questions = screen.getAllByTestId('ask-question');
    expect(questions).toHaveLength(2);
    expect(questions[0]?.textContent).toBe('选择重试策略？');
    const options = screen.getAllByTestId('ask-option');
    expect(options).toHaveLength(3);
    expect(options[0]?.textContent).toContain('指数退避');
    expect(options[0]?.textContent).toContain('推荐');
    expect(options[1]?.textContent).not.toContain('null');
  });

  it('input 形态不可解析（questions 非数组）→ 回退 JSON 原样呈现，不抛错', () => {
    const Card = DEFAULT_TOOL_CARDS['AskUserQuestion'];
    render(
      <Card
        toolCallId="tu_ask"
        toolName="AskUserQuestion"
        input={{ note: '未知形态' }}
        output={null}
      />,
    );

    const fallback = screen.getByTestId('ask-question-card').querySelector('pre');
    expect(fallback?.textContent).toContain('未知形态');
    expect(screen.queryByTestId('ask-question')).toBeNull();
  });

  it('选项形态残缺（label 缺失）→ 宽容解析不崩', () => {
    const Card = DEFAULT_TOOL_CARDS['AskUserQuestion'];
    render(
      <Card
        toolCallId="tu_ask"
        toolName="AskUserQuestion"
        input={{ questions: [{ question: '残缺选项', options: [{ label: 42 }, '非法项'] }] }}
        output={null}
      />,
    );

    expect(screen.getByTestId('ask-question').textContent).toBe('残缺选项');
    expect(screen.getAllByTestId('ask-option')).toHaveLength(2);
  });
});

// ---------------------------------------------------------------------------
// 未注册回退与错误结果形态
// ---------------------------------------------------------------------------

describe('DEFAULT_TOOL_CARDS：回退与错误结果', () => {
  it('未注册的 toolName 查找返回回退（undefined，由消费方落默认工具对卡）', () => {
    expect(DEFAULT_TOOL_CARDS['NoSuchTool']).toBeUndefined();
    expect(Object.keys(DEFAULT_TOOL_CARDS)).toEqual(['AskUserQuestion']);
  });

  it('isError 的工具结果以错误形态呈现（默认对卡：出错标记 + 失败底色）', () => {
    render(
      <ToolPairCard
        name="Bash"
        toolCallId="tu_err"
        input={{ command: 'exit 1' }}
        output={{ content: '命令失败', isError: true }}
      />,
    );

    expect(screen.getByTestId('block-tool-use').textContent).toContain('出错');
    const result = screen.getByTestId('block-tool-result');
    expect(result.textContent).toBe('命令失败');
    expect(result.className).toContain('text-fail');
  });

  it('正常结果不以错误形态呈现（对照）', () => {
    render(
      <ToolPairCard
        name="Bash"
        toolCallId="tu_ok"
        input={{}}
        output={{ content: 'ok', isError: false }}
      />,
    );

    expect(screen.getByTestId('block-tool-use').textContent).not.toContain('出错');
    expect(screen.getByTestId('block-tool-result').className).not.toContain('text-fail');
  });
});
