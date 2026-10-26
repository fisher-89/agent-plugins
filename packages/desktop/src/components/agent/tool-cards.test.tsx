// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vite-plus/test';

import { ToolPairCard, WebSearchCard } from './agent-blocks';
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
    // 注册表键面数量与顺序随动：web_search 注册项在场，顺序同源码声明序
    expect(Object.keys(DEFAULT_TOOL_CARDS)).toEqual(['AskUserQuestion', 'web_search']);
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

// ---------------------------------------------------------------------------
// DEFAULT_TOOL_CARDS：web_search 注册项（AC-10）
// ---------------------------------------------------------------------------

/** `web_search` 输出载荷 fixture（工具卡加位形态）。 */
const SEARCH_OUTPUT = {
  content: '',
  isError: false,
  search: {
    results: [
      { url: 'https://a/1', title: '甲来源', pageAge: null },
      { url: 'https://b/2', title: null, pageAge: null },
    ],
    error: null,
  },
};

describe('DEFAULT_TOOL_CARDS：web_search 注册表键面', () => {
  it('键面含 web_search 且值为 WebSearchCard 本体（同引用），既有 AskUserQuestion 项仍在', () => {
    const keys = Object.keys(DEFAULT_TOOL_CARDS);
    expect(keys).toContain('web_search');
    expect(keys).toContain('AskUserQuestion');
    expect(DEFAULT_TOOL_CARDS['web_search']).toBe(WebSearchCard);
    expect(keys).toHaveLength(2);
  });

  it('键名精确匹配：大小写 / 分隔符偏差与空串均 miss（键面不做归一化）', () => {
    for (const miss of ['Web_Search', 'web-search', 'WebSearch', ' web_search', '']) {
      expect(DEFAULT_TOOL_CARDS[miss]).toBeUndefined();
    }
    expect(DEFAULT_TOOL_CARDS['web_search']).toBeDefined();
  });
});

describe('DEFAULT_TOOL_CARDS：web_search 命中渲染', () => {
  it('经注册表取卡渲染 → block-web-search 在场、标题「网络搜索」、查询词与来源在场', () => {
    const Card = DEFAULT_TOOL_CARDS['web_search'];
    render(
      <Card
        toolCallId="srvtoolu_1"
        toolName="web_search"
        input={{ query: '今日汇率' }}
        output={SEARCH_OUTPUT}
      />,
    );

    expect(screen.getByTestId('block-web-search')).toBeTruthy();
    expect(screen.getByTestId('block-web-search').textContent).toContain('网络搜索');
    expect(screen.getByTestId('block-web-search-query').textContent).toContain('今日汇率');
    expect(screen.getByTestId('block-web-search-sources').textContent).toContain('甲来源');
  });

  it('载荷形态异常不炸卡：output null / 缺 search 加位 / search 为 null / input 非对象', () => {
    const Card = DEFAULT_TOOL_CARDS['web_search'];
    const malformed: { input: unknown; output: unknown }[] = [
      { input: { query: 'q' }, output: null },
      { input: { query: 'q' }, output: { content: '', isError: false } },
      { input: { query: 'q' }, output: { content: '', isError: false, search: null } },
      { input: '字符串 input', output: SEARCH_OUTPUT },
      { input: ['数组 input'], output: SEARCH_OUTPUT },
    ];

    for (const { input, output } of malformed) {
      const { unmount } = render(
        <Card
          toolCallId="srvtoolu_bad"
          toolName="web_search"
          input={input}
          output={output as never}
        />,
      );
      // 卡片仍渲染（不抛错、不返回 null）
      expect(screen.getByTestId('block-web-search')).toBeTruthy();
      unmount();
    }
  });

  it('未注册工具名仍 miss（回退默认工具对卡由消费方决定）', () => {
    expect(DEFAULT_TOOL_CARDS['read']).toBeUndefined();
  });
});
