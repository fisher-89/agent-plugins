// @vitest-environment jsdom
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { AgentToolOutput } from '../../lib/agent-adapter';
import { FinishStepCard, ToolPairCard, WebSearchCard } from './agent-blocks';
import { BUILD_IN_TOOL_CARDS } from './tool-cards';

// ---------------------------------------------------------------------------
// 进程边界 Mock：`@/lib/highlighter`（shiki 语法包的动态 import 与重型 grammar
// 装载）以替身注入——替身产出可断言行，并记录**语言实参**（`languageOf` 的
// 唯一可观测面）。注册表 / 四张内建卡 / `lib/parse` / `Trace*` / `CodeBlock`
// 真实组合，不为测试新增导出（私有原语经卡渲染间接驱动）。
// ---------------------------------------------------------------------------

const { loadHighlighterMock } = vi.hoisted(() => ({ loadHighlighterMock: vi.fn() }));

vi.mock('@/lib/highlighter', () => ({ loadHighlighter: loadHighlighterMock }));

/** 高亮替身：每行产出可断言文本（`<h|原文>`），state 串接输入。 */
function fakeHighlighter() {
  const highlight = vi.fn((code: string, state?: string) => ({
    lines: code.split('\n').map((line) => `<h|${line}>`),
    state: `${state ?? ''}+${code}`,
  }));
  return { highlight };
}

beforeEach(() => {
  loadHighlighterMock.mockReset();
  loadHighlighterMock.mockResolvedValue(fakeHighlighter());
});

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

describe('BUILD_IN_TOOL_CARDS：AskUserQuestion 特化', () => {
  it('toolName 为 AskUserQuestion 时命中注册表，问题与选项结构化呈现', () => {
    const Card = BUILD_IN_TOOL_CARDS['AskUserQuestion'];
    expect(Card).toBeDefined();

    render(
      <Card
        toolCallId="tu_ask"
        toolName="AskUserQuestion"
        state="input-available"
        input={ASK_INPUT}
        output={null}
      />,
    );

    // 静态态标题不再承诺输入框（活卡态才有输入面）
    expect(screen.getByTestId('ask-question-card').textContent).toBe(
      '向用户提问（AskUserQuestion）选择重试策略？指数退避推荐固定间隔是否同时清理缓存？是',
    );
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
    const Card = BUILD_IN_TOOL_CARDS['AskUserQuestion'];
    render(
      <Card
        toolCallId="tu_ask"
        toolName="AskUserQuestion"
        state="input-available"
        input={{ note: '未知形态' }}
        output={null}
      />,
    );

    const fallback = screen.getByTestId('ask-question-card').querySelector('pre');
    expect(fallback?.textContent).toContain('未知形态');
    expect(screen.queryByTestId('ask-question')).toBeNull();
  });

  it('选项形态残缺（label 缺失）→ 宽容解析不崩', () => {
    const Card = BUILD_IN_TOOL_CARDS['AskUserQuestion'];
    render(
      <Card
        toolCallId="tu_ask"
        toolName="AskUserQuestion"
        state="input-available"
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

describe('BUILD_IN_TOOL_CARDS：回退与错误结果', () => {
  it('未注册的 toolName 查找返回回退（undefined，由消费方落默认工具对卡）', () => {
    expect(BUILD_IN_TOOL_CARDS['NoSuchTool']).toBeUndefined();
    // 注册表键面数量与顺序随动：内建工具卡按源码声明序注册
    expect(Object.keys(BUILD_IN_TOOL_CARDS)).toEqual([
      'AskUserQuestion',
      'finish_step',
      'web_search',
      'bash',
      'read',
      'write',
      'edit',
    ]);
  });

  it('isError 的工具结果以错误形态呈现（trace-line error 状态 + 失败结果正文）', async () => {
    render(
      <ToolPairCard
        name="Bash"
        toolCallId="tu_err"
        state="output-available"
        input={{ command: 'exit 1' }}
        output={{ content: '命令失败', isError: true }}
      />,
    );

    const cardNode = screen.getByTestId('block-tool-use');
    expect(cardNode.querySelector('[data-status="error"]')).toBeTruthy();

    fireEvent.click(within(cardNode).getByRole('button'));
    expect((await screen.findByTestId('block-tool-result')).textContent).toBe('命令失败');
  });

  it('正常结果不以错误形态呈现（对照）', async () => {
    render(
      <ToolPairCard
        name="Bash"
        toolCallId="tu_ok"
        state="output-available"
        input={{}}
        output={{ content: 'ok', isError: false }}
      />,
    );

    const cardNode = screen.getByTestId('block-tool-use');
    expect(cardNode.querySelector('[data-status="error"]')).toBeNull();

    fireEvent.click(within(cardNode).getByRole('button'));
    expect((await screen.findByTestId('block-tool-result')).textContent).toBe('ok');
  });
});

// ---------------------------------------------------------------------------
// BUILD_IN_TOOL_CARDS：web_search 注册项（AC-10）
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

describe('BUILD_IN_TOOL_CARDS：web_search 注册表键面', () => {
  it('键面含 web_search 且值为 WebSearchCard 本体（同引用），既有 AskUserQuestion 项仍在', () => {
    const keys = Object.keys(BUILD_IN_TOOL_CARDS);
    expect(keys).toContain('web_search');
    expect(keys).toContain('AskUserQuestion');
    expect(BUILD_IN_TOOL_CARDS['web_search']).toBe(WebSearchCard);
  });

  it('键名精确匹配：大小写 / 分隔符偏差与空串均 miss（键面不做归一化）', () => {
    for (const miss of ['Web_Search', 'web-search', 'WebSearch', ' web_search', '']) {
      expect(BUILD_IN_TOOL_CARDS[miss]).toBeUndefined();
    }
    expect(BUILD_IN_TOOL_CARDS['web_search']).toBeDefined();
  });
});

describe('BUILD_IN_TOOL_CARDS：web_search 命中渲染', () => {
  it('经注册表取卡渲染 → block-web-search 在场、标题「网络搜索」、查询词与来源在场', () => {
    const Card = BUILD_IN_TOOL_CARDS['web_search'];
    render(
      <Card
        toolCallId="srvtoolu_1"
        toolName="web_search"
        state="output-available"
        input={{ query: '今日汇率' }}
        output={SEARCH_OUTPUT}
      />,
    );

    const card = screen.getByTestId('block-web-search');
    expect(card).toBeTruthy();
    // 标题栏呈现查询词；来源与错误区在展开后挂载
    fireEvent.click(within(card).getByRole('button'));
    expect(screen.getByTestId('block-web-search-query').textContent).toContain('今日汇率');
    expect(screen.getByTestId('block-web-search-sources').textContent).toContain('甲来源');
  });

  it('载荷形态异常不炸卡：output null / 缺 search 加位 / search 为 null / input 非对象', () => {
    const Card = BUILD_IN_TOOL_CARDS['web_search'];
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
          state="output-available"
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
    expect(BUILD_IN_TOOL_CARDS['NoSuchTool']).toBeUndefined();
  });
});

// ---------------------------------------------------------------------------
// BUILD_IN_TOOL_CARDS：finish_step 注册项（AC-13）
// ---------------------------------------------------------------------------

describe('BUILD_IN_TOOL_CARDS：finish_step 注册表键面', () => {
  it('键面含 finish_step 且值为 FinishStepCard 本体（同引用），既有项仍在', () => {
    const keys = Object.keys(BUILD_IN_TOOL_CARDS);
    expect(keys).toContain('finish_step');
    expect(BUILD_IN_TOOL_CARDS['finish_step']).toBe(FinishStepCard);
    // 既有项零回归
    expect(BUILD_IN_TOOL_CARDS['AskUserQuestion']).toBeDefined();
    expect(BUILD_IN_TOOL_CARDS['web_search']).toBe(WebSearchCard);
    expect(keys).toHaveLength(7);
  });

  it('键名精确匹配：大小写 / 分隔符偏差与空串均 miss（键面不做归一化）', () => {
    for (const miss of ['Finish_Step', 'finish-step', 'FinishStep', ' finish_step', '']) {
      expect(BUILD_IN_TOOL_CARDS[miss]).toBeUndefined();
    }
    expect(BUILD_IN_TOOL_CARDS['finish_step']).toBeDefined();
  });
});

describe('BUILD_IN_TOOL_CARDS：finish_step 命中渲染（经注册表）', () => {
  it('evaluator 形态经注册表取卡渲染 → 摘要行与逐项清单在折叠区内', () => {
    const Card = BUILD_IN_TOOL_CARDS['finish_step'];
    render(
      <Card
        toolCallId="tu_finish"
        toolName="finish_step"
        state="output-available"
        input={{
          report: '评估报告',
          items: [
            { id: 'C1', pass: true, evidence: '据甲' },
            { id: 'C2', pass: false, evidence: '据乙' },
          ],
        }}
        output={{ content: '已收口', isError: false }}
      />,
    );

    const card = screen.getByTestId('block-tool-use');
    expect(screen.getByTestId('block-finish-step-summary').textContent).toContain('含未通过项');
    fireEvent.click(within(card).getByRole('button'));
    expect(screen.getAllByTestId('block-finish-step-item')).toHaveLength(2);
    expect(screen.getByTestId('block-tool-result').textContent).toBe('已收口');
  });

  it('decision 形态经注册表取卡渲染 → 动作摘要行（backtrack 目标 + reason）', () => {
    const Card = BUILD_IN_TOOL_CARDS['finish_step'];
    render(
      <Card
        toolCallId="tu_back"
        toolName="finish_step"
        state="input-available"
        input={{ action: 'backtrack', to: 'dev-design', reason: '设计返工' }}
        output={null}
      />,
    );
    const summary = screen.getByTestId('block-finish-step-summary').textContent ?? '';
    expect(summary).toContain('backtrack → dev-design');
    expect(summary).toContain('设计返工');
  });

  it('载荷形态异常不炸卡（input 非对象 / items 非数组 / action 非字符串）', () => {
    const Card = BUILD_IN_TOOL_CARDS['finish_step'];
    const malformed: unknown[] = [null, 'str', ['arr'], { items: 'not-array' }, { action: 42 }, {}];
    for (const input of malformed) {
      const { unmount } = render(
        <Card
          toolCallId="tu_bad"
          toolName="finish_step"
          state="input-available"
          input={input}
          output={null}
        />,
      );
      expect(screen.getByTestId('block-tool-use')).toBeTruthy();
      unmount();
    }
  });
});

// ---------------------------------------------------------------------------
// 四张既有内建卡与三个私有原语（本 change 整文件突变面的既有分支）
// ---------------------------------------------------------------------------

/** 卡面根（`block-tool-use`，四张折叠卡与 finish_step 卡同形）。 */
function cardRoot(): HTMLElement {
  return screen.getByTestId('block-tool-use');
}

/** 卡面触发按钮（`TraceCollapsibleTrigger`，折叠态仍可点但面板不展开）。 */
function trigger(): HTMLElement {
  return within(cardRoot()).getByRole('button');
}

/** 展开卡面（`defaultOpen` 为假的卡需要显式展开）。 */
function expandCard(): void {
  fireEvent.click(trigger());
}

/** 经注册表取卡渲染（未注册即断言失败，避免静默落空）。 */
function renderToolCard(
  toolName: string,
  options: {
    input?: unknown;
    output?: AgentToolOutput | null;
    state?: 'input-available' | 'output-available';
    toolCallId?: string;
  } = {},
) {
  const Card = BUILD_IN_TOOL_CARDS[toolName];
  expect(Card, `注册表缺 ${toolName}`).toBeDefined();
  return render(
    <Card
      toolCallId={options.toolCallId ?? `tu_${toolName}`}
      toolName={toolName}
      state={options.state ?? 'input-available'}
      input={options.input ?? {}}
      output={options.output ?? null}
    />,
  );
}

/** 卡内折叠区行清单（`data-slot="code-block-line"`）。 */
function codeLines(): HTMLElement[] {
  return Array.from(cardRoot().querySelectorAll('[data-slot="code-block-line"]'));
}

/** `loadHighlighter` 收到的语言实参序列。 */
function highlightLanguages(): string[] {
  return loadHighlighterMock.mock.calls.map(([language]) => String(language));
}

describe('BUILD_IN_TOOL_CARDS：既有六键零回归', () => {
  it.each([
    ['AskUserQuestion', 'ask-question-card'],
    ['web_search', 'block-web-search'],
    ['bash', 'block-tool-use'],
    ['read', 'block-tool-use'],
    ['write', 'block-tool-use'],
    ['edit', 'block-tool-use'],
  ])('键 %s 经注册表取卡渲染 → 卡根 %s 在场且 data-tool-id 等于入参', (toolName, rootTestId) => {
    renderToolCard(toolName, {
      state: 'output-available',
      input: { command: 'ls', path: 'a.ts', content: 'x', questions: [] },
      output: { content: 'ok', isError: false },
      toolCallId: `tu_${toolName}`,
    });

    const root = screen.getByTestId(rootTestId);
    expect(root).toBeTruthy();
    // 四张折叠卡与 finish_step 卡带 data-tool-id；AskUserQuestion 为容器形态（无该钩子）
    if (rootTestId !== 'ask-question-card') {
      expect(root.getAttribute('data-tool-id')).toBe(`tu_${toolName}`);
    }
  });
});

describe('cardAttrs：卡根同形（既有分支）', () => {
  it.each(['bash', 'read', 'write', 'edit'])(
    '卡 %s 的卡根 data-testid 为 block-tool-use 且 block-tool-use 恰 1 个',
    (toolName) => {
      const { unmount } = renderToolCard(toolName, {
        state: 'output-available',
        input: { path: 'a.ts', content: 'x', command: 'ls' },
        output: { content: 'ok', isError: false },
        toolCallId: `id_${toolName}`,
      });
      expect(cardRoot().getAttribute('data-testid')).toBe('block-tool-use');
      expect(cardRoot().getAttribute('data-tool-id')).toBe(`id_${toolName}`);
      expect(screen.queryAllByTestId('block-tool-use')).toHaveLength(1);
      unmount();
    },
  );

  it('对照：AskUserQuestion 为容器形态，不带该钩子', () => {
    renderToolCard('AskUserQuestion', { state: 'input-available', input: { questions: [] } });
    expect(screen.queryByTestId('block-tool-use')).toBeNull();
    expect(screen.getByTestId('ask-question-card')).toBeTruthy();
  });
});

describe('BashCard：output 三态（既有分支）', () => {
  it('output 为 null → aria-disabled 真、trace-line running、点后仍无结果区', () => {
    renderToolCard('bash', { input: { command: 'ls -la' } });

    expect(trigger().getAttribute('aria-disabled')).toBe('true');
    expect(cardRoot().querySelector('[data-slot="trace-line"]')?.getAttribute('data-status')).toBe(
      'running',
    );
    expect(within(cardRoot()).getByText('Bash')).toBeTruthy();

    expandCard();
    expect(screen.queryByTestId('block-tool-result')).toBeNull();
  });

  it('正常结果 → aria-disabled 显式落 false、无 data-status、展开后结果区为 content', () => {
    renderToolCard('bash', {
      state: 'output-available',
      input: { command: 'ls -la' },
      output: { content: '目录内容', isError: false },
    });

    expect(trigger().getAttribute('aria-disabled')).toBe('false');
    expect(
      cardRoot().querySelector('[data-slot="trace-line"]')?.getAttribute('data-status') ?? null,
    ).toBeNull();

    expandCard();
    expect(screen.getByTestId('block-tool-result').textContent).toBe('目录内容');
  });

  it('isError → data-status 为 error 且结果区文本仍为该 content（错误不替换正文）', () => {
    renderToolCard('bash', {
      state: 'output-available',
      input: { command: 'exit 1' },
      output: { content: '命令失败', isError: true },
    });

    expect(trigger().getAttribute('aria-disabled')).toBe('false');
    expect(cardRoot().querySelector('[data-slot="trace-line"]')?.getAttribute('data-status')).toBe(
      'error',
    );

    expandCard();
    expect(screen.getByTestId('block-tool-result').textContent).toBe('命令失败');
  });

  // 图标选择分支的判据面：BashCard 的旋载/终端图标由 `output === null` 选，
  // 借 Spinner 的产品锚点（`data-slot`）判两态互斥——不依赖 lucide 内部 class。
  it.each([
    [null, true],
    [{ content: 'ok', isError: false }, false],
  ])('output 为 %j → 旋载图标在场 %s', (output, spinner) => {
    renderToolCard('bash', { state: 'output-available', input: { command: 'ls' }, output });
    const icon = cardRoot().querySelector('[data-slot="trace-icon"]');
    expect(icon?.querySelector('[data-slot="spinner"]') !== null).toBe(spinner);
    // 两态恒落一枚 lucide 图标（旋载或终端）：图标位不空
    expect(icon?.querySelector('svg') !== null).toBe(true);
  });
});

describe('BashCard：命令栏与异常载荷（既有分支）', () => {
  it.each([
    [{ command: 'ls -la' }, 'ls -la'],
    [{}, ''],
    [null, ''],
    ['裸字符串', ''],
  ])('input 为 %j + output 在场 → 展开后命令栏文本为 %j', (input, expected) => {
    const { unmount } = renderToolCard('bash', {
      state: 'output-available',
      input,
      output: { content: 'ok', isError: false },
    });
    expandCard();
    const headers = cardRoot().querySelectorAll('[data-slot="code-block-header"]');
    expect(headers).toHaveLength(1);
    expect(headers[0]?.textContent).toBe(expected);
    unmount();
  });

  it('output 为 null → 展开被拦：命令栏与结果区均零命中', () => {
    renderToolCard('bash', { input: { command: 'ls -la' } });
    expandCard();
    expect(cardRoot().querySelector('[data-slot="code-block-header"]')).toBeNull();
    expect(screen.queryByTestId('block-tool-result')).toBeNull();
  });
});

describe('ReadCard：标题与行区间（既有分支 / rangeSuffix）', () => {
  it.each([
    [{ path: 'src/a.ts', offset: 10, limit: 5 }, 'Read src/a.ts:10-15'],
    [{ path: 'src/a.ts', offset: 10 }, 'Read src/a.ts:10-最后'],
    [{ path: 'src/a.ts' }, 'Read src/a.ts'],
    [{ path: 'src/a.ts', offset: '10' }, 'Read src/a.ts'],
    [{ path: 'src/a.ts', offset: null }, 'Read src/a.ts'],
    [{ path: 'src/a.ts', offset: 0, limit: 0 }, 'Read src/a.ts:0-0'],
    [{}, 'Read null'],
  ])('input 为 %j → 标题逐字为 %j', (input, expected) => {
    const { unmount } = renderToolCard('read', {
      state: 'output-available',
      input,
      output: { content: 'ok', isError: false },
    });
    expect(cardRoot().querySelector('[data-slot="trace-title"]')?.textContent).toBe(expected);
    unmount();
  });
});

describe('ReadCard：output 三态与路径缺席（既有分支）', () => {
  it('output 为 null → aria-disabled 真、running、展开被拦（零 code-block）', () => {
    renderToolCard('read', { input: { path: 'src/a.ts' } });
    expect(trigger().getAttribute('aria-disabled')).toBe('true');
    expect(cardRoot().querySelector('[data-slot="trace-line"]')?.getAttribute('data-status')).toBe(
      'running',
    );
    expandCard();
    expect(cardRoot().querySelector('[data-slot="code-block"]')).toBeNull();
  });

  it('正常结果 → aria-disabled 假、无 data-status、结果区为 content 且零 text-destructive', () => {
    renderToolCard('read', {
      state: 'output-available',
      input: { path: 'src/a.ts' },
      output: { content: '文件内容', isError: false },
    });
    expect(trigger().getAttribute('aria-disabled')).toBe('false');
    expect(
      cardRoot().querySelector('[data-slot="trace-line"]')?.getAttribute('data-status') ?? null,
    ).toBeNull();

    expandCard();
    expect(screen.getByTestId('block-tool-result').textContent).toBe('文件内容');
    expect(cardRoot().querySelectorAll('.text-destructive')).toHaveLength(0);
  });

  it('isError → 错误提示与正文两处并存（text-destructive 恰 1 个）', () => {
    renderToolCard('read', {
      state: 'output-available',
      input: { path: 'src/a.ts' },
      output: { content: '读取失败', isError: true },
    });
    expect(cardRoot().querySelector('[data-slot="trace-line"]')?.getAttribute('data-status')).toBe(
      'error',
    );

    expandCard();
    const errors = cardRoot().querySelectorAll('.text-destructive');
    expect(errors).toHaveLength(1);
    expect(errors[0]?.textContent).toBe('读取失败');
    expect(screen.getByTestId('block-tool-result').textContent).toBe('读取失败');
  });
});

describe('WriteCard：高亮行 vs 纯文本切行（既有分支 / languageOf）', () => {
  it('有扩展名 → 语言实参 ts、行数 1（高亮路径削尾换行）、未点击即可见行', async () => {
    renderToolCard('write', {
      state: 'output-available',
      input: { path: 'src/a.ts', content: 'x\n' },
      output: { content: 'ok', isError: false },
    });

    await waitFor(() => expect(highlightLanguages()).toEqual(['ts']));
    // defaultOpen 为真：未点击即见行；高亮臂削尾换行 → 1 行
    await waitFor(() => expect(codeLines().length).toBe(1));
    expect(codeLines()[0]?.textContent).toBe('<h|x>');
    expect(trigger().getAttribute('aria-disabled')).toBe('false');
  });

  it.each([
    [{ path: 'README', content: 'x\n' }, '', 2],
    [{ path: 'a.b.c', content: 'x\n' }, 'c', 1],
    // 尾点路径：`languageOf` 落空串扩展名（非 null）→ 仍走高亮臂（削尾换行 → 1 行）
    [{ path: 'a.', content: 'x\n' }, '', 1],
    [{ path: null, content: 'x\n' }, '', 2],
  ])('input 为 %j → 语言实参 %j、行数 %i', async (input, language, lines) => {
    const { unmount } = renderToolCard('write', {
      state: 'output-available',
      input,
      output: { content: 'ok', isError: false },
    });

    await waitFor(() => expect(highlightLanguages()).toEqual([language]));
    await waitFor(() => expect(codeLines().length).toBe(lines));
    unmount();
  });
});

describe('WriteCard：output 三态（既有分支）', () => {
  it('output 为 null → aria-disabled 真、trace-line running、无行可见（defaultOpen 为假）', () => {
    renderToolCard('write', { input: { path: 'src/a.ts', content: 'x' } });

    expect(trigger().getAttribute('aria-disabled')).toBe('true');
    expect(cardRoot().querySelector('[data-slot="trace-line"]')?.getAttribute('data-status')).toBe(
      'running',
    );
    expect(codeLines()).toHaveLength(0);
  });

  it('isError → trace-line error、错误提示取代正文（零 code-block-line）', () => {
    renderToolCard('write', {
      state: 'output-available',
      input: { path: 'src/a.ts', content: 'x' },
      output: { content: '写入失败', isError: true },
    });

    expect(trigger().getAttribute('aria-disabled')).toBe('false');
    expect(cardRoot().querySelector('[data-slot="trace-line"]')?.getAttribute('data-status')).toBe(
      'error',
    );

    const errors = cardRoot().querySelectorAll('.text-destructive');
    expect(errors).toHaveLength(1);
    expect(errors[0]?.textContent).toBe('写入失败');
    expect(codeLines()).toHaveLength(0);
  });

  it('content 缺失 → 行文本为空串（不落假串）', () => {
    renderToolCard('write', {
      state: 'output-available',
      input: { path: 'README' },
      output: { content: 'ok', isError: false },
    });
    // 缺失 content 落空串而非占位串（CodeBlockLine 的空行以换行占位保拷贝语义）
    expect(codeLines()).toHaveLength(1);
    expect(codeLines()[0]?.textContent).toContain('\n');
    expect(codeLines()[0]?.textContent).not.toContain('Stryker');
  });
});

describe('EditCard：旧 / 新两区（既有分支）', () => {
  it('两区 variant 分别为 removed / added，旧区在新区之前', () => {
    renderToolCard('edit', {
      state: 'output-available',
      input: { path: 'src/a.ts', old_string: '旧行', new_string: '新行' },
      output: { content: 'ok', isError: false },
    });

    const lines = codeLines();
    expect(lines).toHaveLength(2);
    expect(lines[0]?.getAttribute('data-variant')).toBe('removed');
    expect(lines[0]?.textContent).toBe('旧行');
    expect(lines[1]?.getAttribute('data-variant')).toBe('added');
    expect(lines[1]?.textContent).toBe('新行');

    const contents = cardRoot().querySelectorAll('[data-slot="code-block-content"]');
    expect(contents).toHaveLength(2);
    expect(contents[0]?.contains(lines[0] as Node)).toBe(true);
    expect(contents[1]?.contains(lines[1] as Node)).toBe(true);
  });

  it.each([
    [
      { path: 'src/a.ts', old_string: '旧', new_string: '新' },
      ['ts', 'ts'],
      ['<h|旧>'],
      ['<h|新>'],
    ],
    [{ path: 'README', old_string: '旧', new_string: '新' }, ['', ''], ['旧'], ['新']],
    // 多行：两区各自成行（高亮臂逐行产替身行，纯文本臂逐行产原文）
    [
      { path: 'src/a.ts', old_string: 'a\nb', new_string: 'c\nd' },
      ['ts', 'ts'],
      ['<h|a>', '<h|b>'],
      ['<h|c>', '<h|d>'],
    ],
  ])(
    'input 为 %j → 语言实参两次为 %j、旧区行 %j、新区行 %j',
    async (input, expected, oldLines, newLines) => {
      const { unmount } = renderToolCard('edit', {
        state: 'output-available',
        input,
        output: { content: 'ok', isError: false },
      });
      await waitFor(() => expect(highlightLanguages()).toEqual(expected));

      // 两枚 code-block-content 依次为旧区 / 新区（旧区先、新区后）
      const regions = cardRoot().querySelectorAll('[data-slot="code-block-content"]');
      expect(regions).toHaveLength(2);
      const textsOf = (region: Element): string[] =>
        Array.from(region.querySelectorAll('[data-slot="code-block-line"]')).map(
          (line) => line.textContent ?? '',
        );
      expect(textsOf(regions[0] as Element)).toEqual(oldLines);
      expect(textsOf(regions[1] as Element)).toEqual(newLines);
      unmount();
    },
  );

  it.each([
    [{ path: 'src/a.ts' }, ['ts', 'ts']],
    [{ path: 'README' }, ['', '']],
  ])('两字段双双缺失（input 为 %j）→ 语言实参 %j 且两区行文本为空串', async (input, expected) => {
    const { unmount } = renderToolCard('edit', {
      state: 'output-available',
      input,
      output: { content: 'ok', isError: false },
    });
    await waitFor(() => expect(highlightLanguages()).toEqual(expected));
    // 缺字段落空串（不落假串）：两区各一行、文本皆空
    await waitFor(() => expect(codeLines()).toHaveLength(2));
    const texts = codeLines().map((line) => line.textContent ?? '');
    expect(texts.some((text) => text.includes('Stryker'))).toBe(false);
    unmount();
  });
});

describe('EditCard：output 三态与 trace-line 无 status（既有分支）', () => {
  it('output 为 null → aria-disabled 真且零 code-block-line', () => {
    renderToolCard('edit', { input: { path: 'src/a.ts', old_string: '旧', new_string: '新' } });
    expect(trigger().getAttribute('aria-disabled')).toBe('true');
    expandCard();
    expect(codeLines()).toHaveLength(0);
  });

  it('正常结果 → aria-disabled 假、两区未点击即可见', () => {
    renderToolCard('edit', {
      state: 'output-available',
      input: { path: 'src/a.ts', old_string: '旧', new_string: '新' },
      output: { content: 'ok', isError: false },
    });
    expect(trigger().getAttribute('aria-disabled')).toBe('false');
    expect(codeLines()).toHaveLength(2);
  });

  it('isError → 错误提示在场且两区均不渲；trace-line 恒无 data-status（对照）', () => {
    renderToolCard('edit', {
      state: 'output-available',
      input: { path: 'src/a.ts', old_string: '旧', new_string: '新' },
      output: { content: '写入失败', isError: true },
    });
    // EditCard 源码未传 status：isError 与正常两态一致（无 data-status 属性）
    expect(cardRoot().querySelector('[data-status]')).toBeNull();

    const errors = cardRoot().querySelectorAll('.text-destructive');
    expect(errors).toHaveLength(1);
    expect(errors[0]?.textContent).toBe('写入失败');
    expect(codeLines()).toHaveLength(0);
  });
});

describe('languageOf：分支表（经 WriteCard 驱动的语言实参）', () => {
  it.each([
    [undefined, ''],
    ['README', ''],
    ['a.ts', 'ts'],
    ['x/a.b.c', 'c'],
    ['a.', ''],
    ['.ts', 'ts'],
    ['', ''],
  ])('path 为 %j → 语言实参 %j', async (path, expected) => {
    const { unmount } = renderToolCard('write', {
      state: 'output-available',
      input: { path, content: 'x' },
      output: { content: 'ok', isError: false },
    });
    await waitFor(() => expect(highlightLanguages()).toEqual([expected]));
    unmount();
  });
});

describe('AskUserQuestionCard：宽容解析存活面（既有分支）', () => {
  it('label / description 非字符串 → 选项文本为空串且无描述 span', () => {
    renderToolCard('AskUserQuestion', {
      input: { questions: [{ question: '问', options: [{ label: 42, description: 42 }] }] },
    });
    const option = screen.getByTestId('ask-option');
    expect(option.textContent).toBe('');
    expect(option.querySelector('span')).toBeNull();
  });

  it('question 非字符串 → 题干文本为空串', () => {
    renderToolCard('AskUserQuestion', {
      input: { questions: [{ question: 42, options: [] }] },
    });
    expect(screen.getByTestId('ask-question').textContent).toBe('');
  });

  it('options 非数组 → 该问题零 ask-option 行（不落假串）', () => {
    renderToolCard('AskUserQuestion', {
      input: { questions: [{ question: '问', options: '非数组' }] },
    });
    expect(screen.queryAllByTestId('ask-option')).toHaveLength(0);
    expect(screen.getByTestId('ask-question').textContent).toBe('问');
  });

  it('description 缺席 → 选项文本恰为 label', () => {
    renderToolCard('AskUserQuestion', {
      input: { questions: [{ question: '问', options: [{ label: '甲' }] }] },
    });
    const option = screen.getByTestId('ask-option');
    expect(option.textContent).toBe('甲');
    expect(option.querySelector('span')).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// ask-tool-in-flow：`AskUserQuestion` 卡双态（静态 / 活卡）与双形态解析
// ---------------------------------------------------------------------------

/** 单问形态（阻塞工具 `AskUserQuestion` 的当前线形态，选项为裸字符串）。 */
const SINGLE_ASK_INPUT = {
  question: '选哪个重试策略？',
  options: ['指数退避', '固定间隔'],
};

describe('BUILD_IN_TOOL_CARDS：AskUserQuestion 双形态归一与双态', () => {
  it('静态态零交互面（AC-11）：未注入动作面 → 零按钮、零 input / textarea、零 onAnswer', () => {
    const Card = BUILD_IN_TOOL_CARDS['AskUserQuestion'];
    render(
      <Card
        toolCallId="tu_static"
        toolName="AskUserQuestion"
        state="input-available"
        input={SINGLE_ASK_INPUT}
        output={null}
      />,
    );

    const card = screen.getByTestId('ask-question-card');
    // 卡在场、题干与选项逐字可读（不是删文案，只是去掉输入承诺）
    expect(card.textContent).toContain('向用户提问（AskUserQuestion）');
    expect(card.textContent).not.toContain('请在下方输入框作答');
    expect(screen.getByTestId('ask-question').textContent).toBe('选哪个重试策略？');
    expect(screen.getAllByTestId('ask-option').map((node) => node.textContent)).toEqual([
      '指数退避',
      '固定间隔',
    ]);
    // 静态态零交互面
    expect(within(card).queryAllByRole('button')).toHaveLength(0);
    expect(card.querySelectorAll('input')).toHaveLength(0);
    expect(card.querySelectorAll('textarea')).toHaveLength(0);
  });

  it('活卡自由文本与选项两路并存（AC-6）：选项按钮与输入框同卡在场，提交走 onAnswer', () => {
    const onAnswer = vi.fn();
    const Card = BUILD_IN_TOOL_CARDS['AskUserQuestion'];
    render(
      <Card
        toolCallId="tu_two_paths"
        toolName="AskUserQuestion"
        state="input-available"
        input={SINGLE_ASK_INPUT}
        output={null}
        askAnswer={{ onAnswer }}
      />,
    );

    // 两路并存
    expect(screen.getAllByTestId('ask-answer-option')).toHaveLength(2);
    expect(screen.getByTestId('ask-answer-input')).toBeTruthy();

    // 自由文本路：输入 → 提交按钮 → onAnswer 收到原文（提交前 trim）
    fireEvent.change(screen.getByTestId('ask-answer-input'), {
      target: { value: ' 自由作答 ' },
    });
    fireEvent.click(screen.getByTestId('ask-answer-submit'));
    expect(onAnswer).toHaveBeenCalledWith('自由作答');
  });

  it('提交后本地禁用（选项与文本两形态）：二次提交零 replay', () => {
    const onAnswer = vi.fn();
    const Card = BUILD_IN_TOOL_CARDS['AskUserQuestion'];
    const first = render(
      <Card
        toolCallId="tu_opt_once"
        toolName="AskUserQuestion"
        state="input-available"
        input={SINGLE_ASK_INPUT}
        output={null}
        askAnswer={{ onAnswer }}
      />,
    );
    const buttons = screen.getAllByTestId('ask-answer-option');
    fireEvent.click(buttons[0] as HTMLElement);
    fireEvent.click(buttons[0] as HTMLElement);
    expect(onAnswer).toHaveBeenCalledTimes(1);
    first.unmount();

    const second = render(
      <Card
        toolCallId="tu_text_once"
        toolName="AskUserQuestion"
        state="input-available"
        input={SINGLE_ASK_INPUT}
        output={null}
        askAnswer={{ onAnswer }}
      />,
    );
    fireEvent.change(screen.getByTestId('ask-answer-input'), { target: { value: '文本作答' } });
    fireEvent.click(screen.getByTestId('ask-answer-submit'));
    // 提交后卡片回落静态（提交按钮 / 输入框退场），零第二次投递
    expect(screen.queryByTestId('ask-answer-submit')).toBeNull();
    expect(screen.queryByTestId('ask-answer-input')).toBeNull();
    expect(onAnswer).toHaveBeenCalledTimes(2);
    second.unmount();
  });

  it('空串与纯空白不提交（AC-11）：提交按钮禁用、click 后 onAnswer 零调用', () => {
    const onAnswer = vi.fn();
    const Card = BUILD_IN_TOOL_CARDS['AskUserQuestion'];
    render(
      <Card
        toolCallId="tu_blank"
        toolName="AskUserQuestion"
        state="input-available"
        input={SINGLE_ASK_INPUT}
        output={null}
        askAnswer={{ onAnswer }}
      />,
    );

    const submit = screen.getByTestId<HTMLButtonElement>('ask-answer-submit');
    // 空串形态
    expect(submit.disabled).toBe(true);
    fireEvent.click(submit);
    expect(onAnswer).not.toHaveBeenCalled();

    // 纯空白形态
    fireEvent.change(screen.getByTestId('ask-answer-input'), { target: { value: '   ' } });
    expect(screen.getByTestId<HTMLButtonElement>('ask-answer-submit').disabled).toBe(true);
    fireEvent.click(screen.getByTestId('ask-answer-submit'));
    expect(onAnswer).not.toHaveBeenCalled();
  });

  it('cardAsk 动作面类型面两形态：省略与给值均可编译，给值时卡内不消费 pendingToolCallId', () => {
    const Card = BUILD_IN_TOOL_CARDS['AskUserQuestion'];
    const props: React.ComponentProps<typeof Card> = {
      toolCallId: 'tu_type',
      toolName: 'AskUserQuestion',
      state: 'input-available',
      input: SINGLE_ASK_INPUT,
      output: null,
    };
    // 省略形态
    const { unmount } = render(<Card {...props} />);
    expect(screen.queryAllByTestId('ask-answer-option')).toHaveLength(0);
    unmount();

    // 给值形态：动作面只有 `onAnswer`（判活在上层 `ToolPartView` 完成）
    const onAnswer = vi.fn();
    const withAction: React.ComponentProps<typeof Card> = { ...props, askAnswer: { onAnswer } };
    render(<Card {...withAction} />);
    const buttons = screen.getAllByTestId('ask-answer-option');
    fireEvent.click(buttons[0] as HTMLElement);
    expect(onAnswer).toHaveBeenCalledWith('指数退避');
  });

  it('单问形态（question + 裸字符串 options）→ 结构化呈现，与历史 questions 形态同构', () => {
    const Card = BUILD_IN_TOOL_CARDS['AskUserQuestion'];
    render(
      <Card
        toolCallId="tu_single"
        toolName="AskUserQuestion"
        state="input-available"
        input={SINGLE_ASK_INPUT}
        output={null}
      />,
    );

    expect(screen.getAllByTestId('ask-question')).toHaveLength(1);
    expect(screen.getByTestId('ask-question').textContent).toBe('选哪个重试策略？');
    const options = screen.getAllByTestId('ask-option');
    expect(options.map((node) => node.textContent)).toEqual(['指数退避', '固定间隔']);
    // 静态态零交互
    expect(screen.queryByTestId('ask-answer-option')).toBeNull();
    expect(screen.queryByTestId('ask-answer-input')).toBeNull();
  });

  it('历史形态（questions 数组 + {label, description}）零改动仍可读', () => {
    const Card = BUILD_IN_TOOL_CARDS['AskUserQuestion'];
    render(
      <Card
        toolCallId="tu_legacy"
        toolName="AskUserQuestion"
        state="input-available"
        input={ASK_INPUT}
        output={null}
      />,
    );
    expect(screen.getAllByTestId('ask-question')).toHaveLength(2);
    expect(screen.getAllByTestId('ask-option')[0]?.textContent).toContain('推荐');
  });

  it('活卡态（注入 askAnswer）→ 选项变可点按钮 + 自由文本输入与提交', () => {
    const onAnswer = vi.fn();
    const Card = BUILD_IN_TOOL_CARDS['AskUserQuestion'];
    render(
      <Card
        toolCallId="tu_live"
        toolName="AskUserQuestion"
        state="input-available"
        input={SINGLE_ASK_INPUT}
        output={null}
        askAnswer={{ onAnswer }}
      />,
    );

    expect(screen.getByTestId('ask-question-card').textContent).toContain('选择一个选项或自行作答');
    expect(screen.queryAllByTestId('ask-option')).toHaveLength(0);
    const buttons = screen.getAllByTestId('ask-answer-option');
    expect(buttons.map((node) => node.textContent)).toEqual(['指数退避', '固定间隔']);
    expect(screen.getByTestId('ask-answer-input')).toBeTruthy();
    expect(screen.getByTestId('ask-answer-submit')).toBeTruthy();

    fireEvent.click(buttons[0] as HTMLElement);
    expect(onAnswer).toHaveBeenCalledWith('指数退避');
  });

  it('活卡提交后本地即时禁用（防连点）：二次点击零 replay', () => {
    const onAnswer = vi.fn();
    const Card = BUILD_IN_TOOL_CARDS['AskUserQuestion'];
    render(
      <Card
        toolCallId="tu_live_once"
        toolName="AskUserQuestion"
        state="input-available"
        input={SINGLE_ASK_INPUT}
        output={null}
        askAnswer={{ onAnswer }}
      />,
    );

    fireEvent.click(screen.getAllByTestId('ask-answer-option')[0] as HTMLElement);
    expect(onAnswer).toHaveBeenCalledTimes(1);
    // 提交后 submitted = true → 卡片回落静态形态（零按钮）
    expect(screen.queryAllByTestId('ask-answer-option')).toHaveLength(0);
    expect(screen.getAllByTestId('ask-option')).toHaveLength(2);
    expect(onAnswer).toHaveBeenCalledTimes(1);
  });

  it('自由文本：Enter 提交与按钮提交同效，且空白输入被拦（提交侧零 trim 到后端）', () => {
    const onAnswer = vi.fn();
    const Card = BUILD_IN_TOOL_CARDS['AskUserQuestion'];
    render(
      <Card
        toolCallId="tu_live_text"
        toolName="AskUserQuestion"
        state="input-available"
        input={SINGLE_ASK_INPUT}
        output={null}
        askAnswer={{ onAnswer }}
      />,
    );

    const input = screen.getByTestId('ask-answer-input');
    // 纯空白：提交按钮禁用且 Enter 零提交
    fireEvent.change(input, { target: { value: '   ' } });
    expect((screen.getByTestId('ask-answer-submit') as HTMLButtonElement).disabled).toBe(true);
    fireEvent.keyDown(input, { key: 'Enter' });
    expect(onAnswer).not.toHaveBeenCalled();

    // 有内容回车提交（提交前 trim）
    fireEvent.change(input, { target: { value: '  自定义方案  ' } });
    fireEvent.keyDown(input, { key: 'Enter' });
    expect(onAnswer).toHaveBeenCalledWith('自定义方案');
  });

  it('两形态皆不匹配（DD9）：{} / question 非串 / 原串 / null / 数组 → 回退 JSON 原样，零 panic', () => {
    const Card = BUILD_IN_TOOL_CARDS['AskUserQuestion'];
    const cases: unknown[] = [{}, { question: 42, options: [] }, 'str', null, []];
    for (const input of cases) {
      const mounted = render(
        <Card
          toolCallId="tu_bad"
          toolName="AskUserQuestion"
          state="input-available"
          input={input}
          output={null}
          askAnswer={{ onAnswer: vi.fn() }}
        />,
      );
      const fallback = screen.getByTestId('ask-question-card').querySelector('pre');
      // JSON 原样回退（宽容解析零回归）：题干节点缺席、pre 在场
      expect(fallback, `${JSON.stringify(input)} 的回退呈现`).toBeTruthy();
      expect(screen.queryByTestId('ask-question')).toBeNull();
      mounted.unmount();
    }
  });

  it('历史 SDK 形态（questions + {label, description}）在活卡态下同样可点（双形态归一）', () => {
    const onAnswer = vi.fn();
    const Card = BUILD_IN_TOOL_CARDS['AskUserQuestion'];
    render(
      <Card
        toolCallId="tu_legacy_live"
        toolName="AskUserQuestion"
        state="input-available"
        input={{
          questions: [
            {
              question: '历史形态问',
              options: [{ label: '标签甲', description: '甲的说明' }],
            },
          ],
        }}
        output={null}
        askAnswer={{ onAnswer }}
      />,
    );

    const buttons = screen.getAllByTestId('ask-answer-option');
    expect(buttons).toHaveLength(1);
    expect(buttons[0]?.textContent).toContain('标签甲');
    expect(buttons[0]?.textContent).toContain('甲的说明');
    fireEvent.click(buttons[0] as HTMLElement);
    expect(onAnswer).toHaveBeenCalledWith('标签甲');
  });

  it('options 缺席（单问形态）→ 空选项清单不崩，输入面仍可用', () => {
    const onAnswer = vi.fn();
    const Card = BUILD_IN_TOOL_CARDS['AskUserQuestion'];
    render(
      <Card
        toolCallId="tu_no_options"
        toolName="AskUserQuestion"
        state="input-available"
        input={{ question: '只有题干' }}
        output={null}
        askAnswer={{ onAnswer }}
      />,
    );
    expect(screen.getByTestId('ask-question').textContent).toBe('只有题干');
    expect(screen.queryAllByTestId('ask-answer-option')).toHaveLength(0);
    expect(screen.getByTestId('ask-answer-input')).toBeTruthy();
  });

  it('混合选项形态：字符串与 {label, description} 并存 → 逐项归一到同一渲染（description 各自保真）', () => {
    const Card = BUILD_IN_TOOL_CARDS['AskUserQuestion'];
    render(
      <Card
        toolCallId="tu_mixed"
        toolName="AskUserQuestion"
        state="input-available"
        input={{
          questions: [
            {
              question: '混合形态',
              options: ['甲', { label: '乙', description: '乙的补充说明' }],
            },
          ],
        }}
        output={null}
      />,
    );

    const options = screen.getAllByTestId('ask-option');
    expect(options).toHaveLength(2);
    expect(options[0]?.textContent).toContain('甲');
    expect(options[0]?.textContent).not.toContain('null');
    expect(options[1]?.textContent).toContain('乙');
    expect(options[1]?.textContent).toContain('乙的补充说明');
  });

  it('零选项（options: []）→ 空列表、题干仍渲染；1000 选项逐项渲染不崩', () => {
    const Card = BUILD_IN_TOOL_CARDS['AskUserQuestion'];
    const empty = render(
      <Card
        toolCallId="tu_empty_options"
        toolName="AskUserQuestion"
        state="input-available"
        input={{ question: '零选项', options: [] }}
        output={null}
      />,
    );
    expect(screen.getByTestId('ask-question').textContent).toBe('零选项');
    expect(screen.queryAllByTestId('ask-option')).toHaveLength(0);
    empty.unmount();

    const many = Array.from({ length: 1000 }, (_, index) => `选项${index}`);
    render(
      <Card
        toolCallId="tu_many_options"
        toolName="AskUserQuestion"
        state="input-available"
        input={{ question: '千选项', options: many }}
        output={null}
      />,
    );
    expect(screen.getAllByTestId('ask-option')).toHaveLength(1000);
  });

  it('题干与选项边界：空串题干 / 特殊字符 / 超长选项逐字呈现', () => {
    const Card = BUILD_IN_TOOL_CARDS['AskUserQuestion'];
    const long = '字'.repeat(1200);
    render(
      <Card
        toolCallId="tu_bounds"
        toolName="AskUserQuestion"
        state="input-available"
        input={{ question: '', options: ['a\nb', 'a\0b🚀', long] }}
        output={null}
      />,
    );
    expect(screen.getByTestId('ask-question').textContent).toBe('');
    const options = screen.getAllByTestId('ask-option');
    expect(options).toHaveLength(3);
    expect(options[2]?.textContent).toBe(long);
  });
});
