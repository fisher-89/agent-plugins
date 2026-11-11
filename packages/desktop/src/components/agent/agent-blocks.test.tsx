// @vitest-environment jsdom
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { AgentToolOutput, AgentUIMessage } from '../../lib/agent-adapter';
import type { AgentCitation, WebSearchSource } from '../../types/dto';
import {
  FinishStepCard,
  TextBlock,
  ThinkingBlock,
  ToolPairCard,
  WebSearchCard,
  isToolPart,
} from './agent-blocks';

// ---------------------------------------------------------------------------
// 进程边界 Mock：外壳 IPC（`@tauri-apps/plugin-opener`）以 `openUrl` 替身注入
// （可按用例切 resolve / reject 并断言调用次数与实参）。其余件（TextBlock /
// WebSearchCard / 私有 CitationList / 私有 ExternalLink / react-markdown）真实
// 组合，不为测试新增导出（私有件经公开组件间接驱动）。
// ---------------------------------------------------------------------------

const { openUrlMock } = vi.hoisted(() => ({ openUrlMock: vi.fn() }));

vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: openUrlMock }));

beforeEach(() => {
  openUrlMock.mockReset();
  openUrlMock.mockResolvedValue(undefined);
});

// ---------------------------------------------------------------------------
// fixture：来源 / 引用 / 输出载荷（serde camelCase 线格式）
// ---------------------------------------------------------------------------

function source(
  url: string,
  title: string | null = null,
  pageAge: string | null = null,
): WebSearchSource {
  return { url, title, pageAge };
}

function citation(url: string | null, title: string | null = null): AgentCitation {
  return { url, title, citedText: null, raw: { url } };
}

function searchOutput(results: WebSearchSource[], error: string | null = null): AgentToolOutput {
  return { content: '', isError: error !== null, search: { results, error } };
}

function renderCard(output: AgentToolOutput | null, input: unknown = { query: '今日汇率' }) {
  return render(
    <WebSearchCard toolCallId="srvtoolu_1" toolName="web_search" input={input} output={output} />,
  );
}

/** 卡面 root（`block-web-search`）。 */
function card(): HTMLElement {
  return screen.getByTestId('block-web-search');
}

/** 卡面标题栏文案（查询词所在行）。 */
function summaryText(): string {
  return card().querySelector('button')?.textContent ?? '';
}

/** 展开卡面（TraceCollapsible 默认折叠，body 内容展开后才挂载）。 */
function expandCard(): void {
  fireEvent.click(within(card()).getByRole('button'));
}

// ---------------------------------------------------------------------------
// WebSearchCard：标题两态 / body 三区 / 错误面
// ---------------------------------------------------------------------------

describe('WebSearchCard：标题两态一致（无状态文案）', () => {
  it('output 为 null（未落定）与 output 在场（落定）两态标题文本一致', () => {
    const { unmount } = render(
      <WebSearchCard
        toolCallId="a"
        toolName="web_search"
        input={{ query: '今日汇率' }}
        output={null}
      />,
    );
    const pendingText = summaryText();
    unmount();

    renderCard(searchOutput([source('https://a/1')]));
    const settledText = summaryText();

    expect(pendingText).toBe('今日汇率');
    expect(settledText).toBe('今日汇率');
  });

  it('文本不含进行中 / 加载 / 失败类状态文案（决策 ③：无搜索实时 delta）', () => {
    renderCard(null);
    expandCard();
    const text = card().textContent ?? '';
    for (const word of ['正在搜索', '搜索中', '加载', '失败', '超时']) {
      expect(text).not.toContain(word);
    }
  });
});

describe('WebSearchCard：body 三区（查询词 / 来源列表 / 错误码）', () => {
  it('查询词、三条来源与 root 同时在场，toolCallId / toolName 任意值不影响呈现', () => {
    render(
      <WebSearchCard
        toolCallId="任意-id"
        toolName="web_search"
        input={{ query: '今日汇率' }}
        output={searchOutput([
          source('https://a/1', '甲来源'),
          source('https://b/2', '乙来源'),
          source('https://c/3'),
        ])}
      />,
    );
    expandCard();

    expect(screen.getByTestId('block-web-search-query').textContent).toContain('今日汇率');
    const sources = screen.getByTestId('block-web-search-sources');
    const items = sources.querySelectorAll('[data-slot="trace-line"]');
    expect(items).toHaveLength(3);
    expect(sources.textContent).toContain('甲来源');
    expect(sources.textContent).toContain('c'); // title 缺席回退主机名
    expect(card().getAttribute('data-tool-id')).toBe('任意-id');
  });

  it('来源按 url 去重且 title 为 null 的项回退显示主机名', () => {
    renderCard(
      searchOutput([
        source('https://dup/1', '同名来源'),
        source('https://dup/1', '重复来源'),
        source('https://only/2'),
      ]),
    );
    expandCard();

    const items = screen
      .getByTestId('block-web-search-sources')
      .querySelectorAll('[data-slot="trace-line"]');
    expect(items).toHaveLength(2);
    expect(items[0].textContent).toBe('同名来源');
    expect(items[1].textContent).toBe('only'); // title 缺席回退主机名
  });

  it('错误码在场时错误区呈现，来源列表不与错误区互斥（同数据两区并存）', () => {
    renderCard(searchOutput([source('https://keep/1', '保留来源')], 'max_uses_exceeded'));
    expandCard();

    expect(screen.getByTestId('block-web-search-error').textContent).toContain('max_uses_exceeded');
  });
});

describe('WebSearchCard：空载荷与孤儿形态（边界）', () => {
  it('input 为 null（result 侧孤儿）→ 不显查询词、不 panic', () => {
    renderCard(searchOutput([source('https://orphan/1')]), null);
    expandCard();

    expect(screen.queryByTestId('block-web-search-query')).toBeNull();
    expect(card()).toBeTruthy();
  });

  it('search.results 为空数组 → 不渲空列表容器', () => {
    renderCard(searchOutput([]));
    expandCard();

    expect(screen.queryByTestId('block-web-search-sources')).toBeNull();
  });

  it('output 为 null → 不显来源区与错误区', () => {
    renderCard(null);
    expandCard();

    expect(screen.queryByTestId('block-web-search-sources')).toBeNull();
    expect(screen.queryByTestId('block-web-search-error')).toBeNull();
  });

  it('input.query 为空串 / 非对象 input → 不显空标签', () => {
    const { unmount } = render(
      <WebSearchCard toolCallId="a" toolName="web_search" input={{ query: '' }} output={null} />,
    );
    expandCard();
    expect(screen.queryByTestId('block-web-search-query')).toBeNull();
    unmount();

    render(<WebSearchCard toolCallId="a" toolName="web_search" input="非对象" output={null} />);
    expandCard();
    expect(screen.queryByTestId('block-web-search-query')).toBeNull();
    expect(card()).toBeTruthy();
  });
});

// ---------------------------------------------------------------------------
// 外链唯一调用点（AC-12）：仅 http(s) 渲 <a>，点击走 openUrl
// ---------------------------------------------------------------------------

describe('ExternalLink（经 WebSearchCard 间接驱动）：非 http(s) 降级纯文本', () => {
  it.each(['javascript:alert(1)', 'file:///etc/passwd', 'mailto:a@b.c'])(
    '来源 url 为 %s → 纯文本条目（无 <a>、无 role=link），点击不触达 openUrl',
    (url) => {
      renderCard(searchOutput([source(url, '不可点来源')]));
      expandCard();

      const item = screen
        .getByTestId('block-web-search-sources')
        .querySelector('[data-slot="trace-line"]');
      expect(item?.querySelector('a')).toBeNull();
      expect(item?.querySelector('[role="link"]')).toBeNull();
      expect(item?.textContent).toBe('不可点来源');

      fireEvent.click(item as HTMLElement);
      expect(openUrlMock).not.toHaveBeenCalled();
    },
  );
});

describe('ExternalLink：http(s) 渲链接并经壳侧 opener 打开', () => {
  it('https 来源为 <a href>，点击以该 url 恰调用一次 openUrl 且阻止默认导航', () => {
    renderCard(searchOutput([source('https://example.com/a', '示例来源')]));
    expandCard();

    const anchor = screen.getByTestId('block-web-search-sources').querySelector('a');
    expect(anchor).not.toBeNull();
    expect(anchor?.getAttribute('href')).toBe('https://example.com/a');

    const event = new MouseEvent('click', { bubbles: true, cancelable: true });
    anchor?.dispatchEvent(event);

    expect(openUrlMock).toHaveBeenCalledTimes(1);
    expect(openUrlMock).toHaveBeenCalledWith('https://example.com/a');
    expect(event.defaultPrevented).toBe(true);
  });

  it('http（非 https）同样渲链接', () => {
    renderCard(searchOutput([source('http://example.com/plain')]));
    expandCard();

    expect(
      screen.getByTestId('block-web-search-sources').querySelector('a')?.getAttribute('href'),
    ).toBe('http://example.com/plain');
  });

  it('openUrl reject → 无未捕获拒绝、卡面与来源列表照常渲染', async () => {
    openUrlMock.mockRejectedValue(new Error('scope 拒绝'));
    renderCard(searchOutput([source('https://example.com/fail', '失败来源')]));
    expandCard();

    const anchor = screen.getByTestId('block-web-search-sources').querySelector('a');
    fireEvent.click(anchor as HTMLElement);

    await waitFor(() => expect(openUrlMock).toHaveBeenCalledTimes(1));
    // 呈现不被错误态打断（.catch 吞掉失败）
    expect(card()).toBeTruthy();
    expect(screen.getByTestId('block-web-search-sources').textContent).toContain('失败来源');
  });
});

// ---------------------------------------------------------------------------
// TextBlock：正文 + 底部引用列表
// ---------------------------------------------------------------------------

describe('TextBlock：引用列表（AC-10）', () => {
  it('两条同 url 引用 → 列表一条去重项、文本为 title ?? url，正文 markdown 照常渲染', () => {
    render(
      <TextBlock
        text={'结论 **加粗**'}
        citations={[citation('https://dup/1', '引用标题'), citation('https://dup/1', '重复标题')]}
      />,
    );

    expect(screen.getByTestId('block-text').textContent).toContain('结论');
    const list = screen.getByTestId('block-citations');
    const items = list.querySelectorAll('li');
    expect(items).toHaveLength(1);
    expect(items[0].textContent).toBe('引用标题');
  });

  it('title 缺席 → 回退 url；url 亦缺席 → 回退 citedText', () => {
    render(
      <TextBlock
        text="正文"
        citations={[
          citation('https://only/1', null),
          { url: null, title: null, citedText: '仅片段', raw: null },
        ]}
      />,
    );

    const items = screen.getByTestId('block-citations').querySelectorAll('li');
    expect(items).toHaveLength(2);
    expect(items[0].textContent).toBe('https://only/1');
    expect(items[1].textContent).toBe('仅片段');
  });

  it('引用条目共用外链调用点：mailto 纯文本、https 链接且点击走同一 openUrl', () => {
    render(
      <TextBlock
        text="正文"
        citations={[citation('mailto:a@b.c', '邮件'), citation('https://cite/2', '网页')]}
      />,
    );

    const items = screen.getByTestId('block-citations').querySelectorAll('li');
    expect(items[0].querySelector('a')).toBeNull();
    const anchor = items[1].querySelector('a');
    expect(anchor).not.toBeNull();

    fireEvent.click(anchor as HTMLElement);
    expect(openUrlMock).toHaveBeenCalledTimes(1);
    expect(openUrlMock).toHaveBeenCalledWith('https://cite/2');
  });
});

describe('TextBlock：无引用形态（边界）', () => {
  it('citations 缺席 / 空数组 → 无列表节点、无空容器', () => {
    const { unmount } = render(<TextBlock text="无引用正文" />);
    expect(screen.queryByTestId('block-citations')).toBeNull();
    unmount();

    render(<TextBlock text="空引用正文" citations={[]} />);
    expect(screen.queryByTestId('block-citations')).toBeNull();
  });

  it('text 为空串 → 不 panic、不渲列表', () => {
    render(<TextBlock text="" citations={[]} />);

    expect(screen.getByTestId('block-text')).toBeTruthy();
    expect(screen.queryByTestId('block-citations')).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// 既有块回归（ThinkingBlock / ToolPairCard / isToolPart 语义不变）
// ---------------------------------------------------------------------------

describe('既有块回归', () => {
  it('ThinkingBlock 折叠呈现（Collapsible + 触发按钮文案「思考」，点开后现正文）', async () => {
    render(<ThinkingBlock text="先想一想" />);

    const block = screen.getByTestId('block-thinking');
    expect(block.querySelector('[data-slot="collapsible-content"]')).toBeNull();
    // 未展开时正文不可见
    expect(block.textContent).not.toContain('先想一想');
    fireEvent.click(screen.getByRole('button', { name: /思考/ }));
    await waitFor(() => {
      expect(block.textContent).toContain('先想一想');
    });
  });

  it('ToolPairCard：input / output 成对呈现，isError 高亮标记在场', async () => {
    render(
      <ToolPairCard
        name="Bash"
        toolCallId="tu_1"
        state="output-available"
        input={{ command: 'ls' }}
        output={{ content: '目录内容', isError: true }}
      />,
    );

    const cardNode = screen.getByTestId('block-tool-use');
    expect(cardNode.textContent).toContain('Bash');
    // isError 以 trace-line 的 error 状态呈现（文字为工具名，错误标记走 data-status）
    expect(cardNode.querySelector('[data-status="error"]')).toBeTruthy();

    fireEvent.click(within(cardNode).getByRole('button'));
    expect((await screen.findByTestId('block-tool-result')).textContent).toBe('目录内容');
  });

  // 图标选择分支的判据面（同 ThinkingBlock：借 Spinner 的产品锚点判两态互斥，
  // 不许断言 lucide 内部 class）。`output` 恒为 null 使 isError 面不介入。
  it.each([
    ['output-available' as const, false],
    ['input-available' as const, true],
  ])('ToolPairCard state 为 %s → 旋载图标在场 %s', (state, spinner) => {
    const { unmount } = render(
      <ToolPairCard name="Bash" toolCallId="tu_1" state={state} input={{}} output={null} />,
    );
    const icon = screen.getByTestId('block-tool-use').querySelector('[data-slot="trace-icon"]');
    expect(icon?.querySelector('[data-slot="spinner"]') !== null).toBe(spinner);
    expect(icon?.querySelector('svg') !== null).toBe(true);
    unmount();
  });

  it('isToolPart 收窄：tool-* 命中、dynamic-tool 与 text / data-* 不命中', () => {
    const toolPart = {
      type: 'tool-web_search',
      toolCallId: 'srvtoolu_1',
      state: 'input-available',
      input: { query: 'q' },
    } as unknown as AgentUIMessage['parts'][number];
    expect(isToolPart(toolPart)).toBe(true);

    for (const part of [
      { type: 'dynamic-tool', toolCallId: 'x', state: 'input-available', input: null },
      { type: 'text', text: '正文', state: 'done' },
      { type: 'data-raw', data: { seq: 0 } },
    ]) {
      expect(isToolPart(part as unknown as AgentUIMessage['parts'][number])).toBe(false);
    }
  });
});

// ---------------------------------------------------------------------------
// FinishStepCard：`finish_step` 收口卡两形态（AC-13）
// ---------------------------------------------------------------------------

/** evaluator 形态 input fixture（两项，一 pass 一 fail）。 */
const FINISH_EVALUATOR_INPUT = {
  report: '评估报告正文',
  items: [
    { id: 'C1', pass: true, evidence: 'src/a.rs:10' },
    { id: 'C2', pass: false, evidence: '缺测试' },
  ],
};

describe('FinishStepCard：evaluator 形态渲染', () => {
  it('卡根与摘要区在场，逐项 ID / pass 标记 / evidence 可见，report 在折叠区内', () => {
    render(
      <FinishStepCard
        toolCallId="tu_finish"
        toolName="finish_step"
        input={FINISH_EVALUATOR_INPUT}
        output={null}
      />,
    );

    const card = screen.getByTestId('block-tool-use');
    expect(card.getAttribute('data-tool-id')).toBe('tu_finish');
    expect(screen.getByTestId('block-finish-step-summary').textContent).toContain('含未通过项');
    expect(screen.getByTestId('block-finish-step-summary').textContent).not.toContain('清单全通过');

    // 清单与 report 在折叠区内：先展开
    fireEvent.click(within(card).getByRole('button'));
    const items = screen.getAllByTestId('block-finish-step-item');
    expect(items).toHaveLength(2);
    expect(items[0]?.textContent).toContain('C1');
    expect(items[0]?.textContent).toContain('通过');
    expect(items[0]?.textContent).toContain('src/a.rs:10');
    expect(items[1]?.textContent).toContain('C2');
    expect(items[1]?.textContent).toContain('未通过');
    expect(card.textContent).toContain('评估报告正文');
  });

  it('全通过时摘要为「清单全通过」且不出现「含未通过项」（两摘要文案互斥）', () => {
    render(
      <FinishStepCard
        toolCallId="tu_pass"
        toolName="finish_step"
        input={{
          report: '报告',
          items: [
            { id: 'C1', pass: true, evidence: '甲' },
            { id: 'C2', pass: true, evidence: '乙' },
          ],
        }}
        output={null}
      />,
    );
    const summary = screen.getByTestId('block-finish-step-summary').textContent ?? '';
    expect(summary).toContain('清单全通过');
    expect(summary).not.toContain('含未通过项');
  });
});

describe('FinishStepCard：decision 三动作渲染', () => {
  it('backtrack 呈目标相位与 reason', () => {
    render(
      <FinishStepCard
        toolCallId="tu_back"
        toolName="finish_step"
        input={{ action: 'backtrack', to: 'test-gen', reason: '覆盖不足' }}
        output={null}
      />,
    );
    const summary = screen.getByTestId('block-finish-step-summary').textContent ?? '';
    expect(summary).toContain('backtrack');
    expect(summary).toContain('test-gen');
    expect(summary).toContain('覆盖不足');
  });

  it('stop 呈 stop 与 reason；ask 呈 question 与每个选项；未知 action 原样呈现', () => {
    const { unmount } = render(
      <FinishStepCard
        toolCallId="tu_stop"
        toolName="finish_step"
        input={{ action: 'stop', reason: '受控终止' }}
        output={null}
      />,
    );
    const stopSummary = screen.getByTestId('block-finish-step-summary').textContent ?? '';
    expect(stopSummary).toContain('stop');
    expect(stopSummary).toContain('受控终止');
    unmount();

    const { unmount: unmountAsk } = render(
      <FinishStepCard
        toolCallId="tu_ask"
        toolName="finish_step"
        input={{ action: 'ask', question: '选哪个？', options: ['甲方案', '乙方案'] }}
        output={null}
      />,
    );
    const askSummary = screen.getByTestId('block-finish-step-summary').textContent ?? '';
    expect(askSummary).toContain('选哪个？');
    expect(askSummary).toContain('甲方案');
    expect(askSummary).toContain('乙方案');
    unmountAsk();

    // 未知 action：原样呈现且不抛错
    render(
      <FinishStepCard
        toolCallId="tu_retry"
        toolName="finish_step"
        input={{ action: 'retry' }}
        output={null}
      />,
    );
    expect(screen.getByTestId('block-finish-step-summary').textContent).toContain('retry');
  });
});

describe('FinishStepCard：input 非预期形态回退', () => {
  it('null / 裸字符串 / 数组 / 空对象 / 类型错各回退 JSON 原样且不抛错', () => {
    const malformed: unknown[] = [null, '裸字符串', [], {}, { items: 'str' }, { action: 42 }];
    for (const input of malformed) {
      const { unmount } = render(
        <FinishStepCard toolCallId="tu_bad" toolName="finish_step" input={input} output={null} />,
      );
      // 卡片仍渲染（不抛错），摘要区回退为缺省标题
      expect(screen.getByTestId('block-tool-use')).toBeTruthy();
      expect(screen.getByTestId('block-finish-step-summary').textContent).toBe('finish_step');
      unmount();
    }
  });
});

describe('FinishStepCard：items 与文本边界', () => {
  it('空数组 / 单元素 / 100 项 / 超长 evidence / 空 report 均不抛错', () => {
    const shapes = [
      [],
      [{ id: 'C1', pass: true, evidence: '甲' }],
      Array.from({ length: 100 }, (_, index) => ({
        id: `C${index}`,
        pass: index % 2 === 0,
        evidence: `据 ${index}`,
      })),
      [{ id: 'C1', pass: true, evidence: 'x'.repeat(5000) }],
    ];
    for (const items of shapes) {
      const { unmount } = render(
        <FinishStepCard
          toolCallId="tu_items"
          toolName="finish_step"
          input={{ report: '', items }}
          output={null}
        />,
      );
      const card = screen.getByTestId('block-tool-use');
      fireEvent.click(within(card).getByRole('button'));
      // 空数组 → 零清单行（queryAll 容忍空集）；非空逐项一一行
      expect(screen.queryAllByTestId('block-finish-step-item')).toHaveLength(items.length);
      unmount();
    }
  });

  it('特殊字符（换行 / NUL / emoji）渲染不抛错且文本可见（React 转义）', () => {
    render(
      <FinishStepCard
        toolCallId="tu_special"
        toolName="finish_step"
        input={{
          report: '报告\n含\0与🚀',
          items: [{ id: '🚀1', pass: true, evidence: '证\n据\0据' }],
        }}
        output={null}
      />,
    );
    const card = screen.getByTestId('block-tool-use');
    fireEvent.click(within(card).getByRole('button'));
    const items = screen.getAllByTestId('block-finish-step-item');
    expect(items[0]?.textContent).toContain('🚀1');
    // 无 dangerouslySetInnerHTML：标签形态的文本原样呈现
    expect(items[0]?.querySelector('script')).toBeNull();
  });
});

describe('FinishStepCard：结果与错误形态', () => {
  it('正常结果呈「已收口」、错误结果呈 trace-line error、output null 时结果区缺席', async () => {
    const { unmount } = render(
      <FinishStepCard
        toolCallId="tu_ok"
        toolName="finish_step"
        input={FINISH_EVALUATOR_INPUT}
        output={{ content: '已收口', isError: false }}
      />,
    );
    const card = screen.getByTestId('block-tool-use');
    fireEvent.click(within(card).getByRole('button'));
    expect((await screen.findByTestId('block-tool-result')).textContent).toBe('已收口');
    expect(card.querySelector('[data-status="error"]')).toBeNull();
    unmount();

    const { unmount: unmountErr } = render(
      <FinishStepCard
        toolCallId="tu_err"
        toolName="finish_step"
        input={FINISH_EVALUATOR_INPUT}
        output={{ content: 'report 超长', isError: true }}
      />,
    );
    const errCard = screen.getByTestId('block-tool-use');
    expect(errCard.querySelector('[data-status="error"]')).toBeTruthy();
    unmountErr();

    // output null：结果区缺席、卡不抛错
    render(
      <FinishStepCard
        toolCallId="tu_none"
        toolName="finish_step"
        input={FINISH_EVALUATOR_INPUT}
        output={null}
      />,
    );
    expect(screen.queryByTestId('block-tool-result')).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// FinishStepCard：decision 摘要逐字 / 逐项行结构 / report 在场判据（AC-13）
// ---------------------------------------------------------------------------

/** 收口卡摘要行文案（`block-finish-step-summary`）。 */
function finishSummary(): string {
  return screen.getByTestId('block-finish-step-summary').textContent ?? '';
}

/** 收口卡卡根（`block-tool-use`）。 */
function finishCard(): HTMLElement {
  return screen.getByTestId('block-tool-use');
}

/** 展开收口卡（折叠内容默认不挂载）。 */
function expandFinishCard(): void {
  fireEvent.click(within(finishCard()).getByRole('button'));
}

/** 逐项清单行（展开后；空集即空数组）。 */
function finishItemRows(): HTMLElement[] {
  return screen.queryAllByTestId('block-finish-step-item');
}

/** 渲染收口卡（evaluator / decision 两形态共用入口）。 */
function renderFinish(input: unknown, output: AgentToolOutput | null = null) {
  return render(
    <FinishStepCard toolCallId="tu_finish" toolName="finish_step" input={input} output={output} />,
  );
}

describe('FinishStepCard：decision 摘要逐字形态', () => {
  it.each([
    [{ action: 'backtrack', to: 'test-gen', reason: '覆盖不足' }, 'backtrack → test-gen：覆盖不足'],
    [{ action: 'stop', reason: '受控终止' }, 'stop：受控终止'],
    [
      { action: 'ask', question: '选哪个？', options: ['甲方案', '乙方案'] },
      'ask：选哪个？（甲方案 / 乙方案）',
    ],
    [{ action: 'retry' }, 'retry'],
  ])('input 为 %j → 摘要逐字为 %s 且不抛错', (input, expected) => {
    const { unmount } = renderFinish(input);
    expect(finishSummary()).toBe(expected);
    unmount();
  });
});

describe('FinishStepCard：decision 摘要缺字段（dict 边界）', () => {
  it.each([
    [{ action: 'backtrack' }, 'backtrack → ：'],
    [{ action: 'stop' }, 'stop：'],
    [{ action: 'ask', question: 42, options: ['甲'] }, 'ask：（甲）'],
    [{ action: 'ask', question: '选哪个？', options: '非数组' }, 'ask：选哪个？'],
    [{ action: 'ask', question: '选哪个？', options: [] }, 'ask：选哪个？'],
    [{ action: 'ask', question: 'q', options: ['甲', 42, '乙'] }, 'ask：q（甲 /  / 乙）'],
  ])('input 为 %j → 摘要逐字为 %s（空位不落假串）', (input, expected) => {
    const { unmount } = renderFinish(input);
    expect(finishSummary()).toBe(expected);
    unmount();
  });
});

describe('FinishStepCard：input 非预期形态回退 JSON 原样', () => {
  it.each([[null], ['裸字符串'], [[]], [{}], [{ items: 'str' }], [{ action: 42 }]])(
    'input 为 %j → 摘要逐字为 finish_step、折叠区为 JSON 原样、render 不抛错',
    (input) => {
      const { unmount } = renderFinish(input);
      expect(finishSummary()).toBe('finish_step');
      expandFinishCard();
      expect(finishCard().querySelector('pre')?.textContent).toBe(JSON.stringify(input, null, 2));
      unmount();
    },
  );

  it('对照：decision 分支命中时不落 JSON 回退面板', () => {
    renderFinish({ action: 'stop', reason: '受控终止' });
    expect(finishSummary()).toBe('stop：受控终止');
    expandFinishCard();
    expect(finishCard().querySelector('pre')).toBeNull();
  });
});

describe('FinishStepCard：逐项行结构与空位（dict / str / bool 边界）', () => {
  it.each([
    [{ pass: true, evidence: '据甲' }, '', 3],
    [{ id: 'C1', pass: true }, 'C1', 2],
    [{ id: 'C1', pass: true, evidence: '' }, 'C1', 3],
    [{ id: 'C1', pass: 'true', evidence: '据甲' }, 'C1', 3],
    [{ id: 'C1', pass: 1 }, 'C1', 2],
  ])('单元素 items 为 %j → ID 文本为 %j、span 计数 %i', (item, expectedId, expectedSpans) => {
    const { unmount } = renderFinish({ report: '', items: [item] });
    expandFinishCard();
    const rows = finishItemRows();
    expect(rows).toHaveLength(1);
    const spans = rows[0]?.querySelectorAll('span') ?? [];
    expect(spans.length).toBe(expectedSpans);
    expect(spans[0]?.textContent).toBe(expectedId);
    unmount();
  });

  it.each([[{ id: 'C1', pass: 'true' }], [{ id: 'C1', pass: 1 }], [{ id: 'C1' }]])(
    'pass 非严格 true（%j）→ 呈「未通过」',
    (item) => {
      const { unmount } = renderFinish({ report: '', items: [item] });
      expandFinishCard();
      expect(finishItemRows()[0]?.querySelector('.text-fail')?.textContent).toBe('未通过');
      unmount();
    },
  );
});

describe('FinishStepCard：pass 标记着色与 report 在场判据', () => {
  it('pass 两态各命中 text-pass / text-fail（着色钩子是唯一 DOM 可观测面）', () => {
    renderFinish({
      report: '报告',
      items: [
        { id: 'C1', pass: true, evidence: '甲' },
        { id: 'C2', pass: false, evidence: '乙' },
      ],
    });
    expandFinishCard();
    const rows = finishItemRows();
    expect(rows[0]?.querySelector('.text-pass')?.textContent).toBe('通过');
    expect(rows[1]?.querySelector('.text-fail')?.textContent).toBe('未通过');
    expect(rows[0]?.querySelector('.text-fail')).toBeNull();
    expect(rows[1]?.querySelector('.text-pass')).toBeNull();
  });

  it.each([
    [{ report: '报告正文', items: [] }, 1, '报告正文'],
    [{ report: '', items: [] }, 1, ''],
    [{ items: [] }, 0, ''],
    [{ report: 42, items: [] }, 0, ''],
  ])('report 形态 %j → 容器计数 %i、文本 %j（判据是 !== null 而非真值）', (input, count, text) => {
    const { unmount } = renderFinish(input);
    expandFinishCard();
    const containers = finishCard().querySelectorAll('.whitespace-pre-wrap');
    expect(containers.length).toBe(count);
    if (count === 1) expect(containers[0]?.textContent).toBe(text);
    unmount();
  });
});

describe('FinishStepCard：items 边界（list 边界）', () => {
  it('items 为空数组 → 摘要「清单全通过」且零逐项行（every 于空数组为真）', () => {
    renderFinish({ report: '', items: [] });
    expect(finishSummary()).toBe('清单全通过');
    expandFinishCard();
    expect(finishItemRows()).toHaveLength(0);
  });

  it('items 单元素 / 100 项 / 超长 evidence 均逐项成行不截断', () => {
    const { unmount } = renderFinish({ report: '', items: [{ id: 'C1', pass: true }] });
    expandFinishCard();
    expect(finishItemRows()).toHaveLength(1);
    unmount();

    const many = Array.from({ length: 100 }, (_, index) => ({
      id: `C${index}`,
      pass: index % 2 === 0,
      evidence: `据 ${index}`,
    }));
    const { unmount: unmountMany } = renderFinish({ report: '', items: many });
    expandFinishCard();
    expect(finishItemRows()).toHaveLength(100);
    unmountMany();

    const longEvidence = 'x'.repeat(5000);
    renderFinish({ report: '', items: [{ id: 'C1', pass: true, evidence: longEvidence }] });
    expandFinishCard();
    const spans = finishItemRows()[0]?.querySelectorAll('span') ?? [];
    expect(spans[2]?.textContent).toBe(longEvidence);
  });

  it.each([[{ items: null }], [{ items: 'str' }]])(
    'items 非数组（%j）→ 落 JSON 回退且摘要为 finish_step',
    (input) => {
      const { unmount } = renderFinish(input);
      expect(finishSummary()).toBe('finish_step');
      expandFinishCard();
      expect(finishCard().querySelector('pre')).not.toBeNull();
      unmount();
    },
  );
});

describe('FinishStepCard：文本特殊字符（str 边界）', () => {
  it('evidence / report / question 含换行、NUL 与 emoji → 渲染不抛错且原样可见', () => {
    const { unmount } = renderFinish({
      report: '报告\n含\0与🚀',
      items: [{ id: '🚀1', pass: true, evidence: '证\n据\0据' }],
    });
    expandFinishCard();
    expect(finishItemRows()[0]?.textContent).toContain('🚀1');
    expect(finishCard().textContent).toContain('报告');
    unmount();

    renderFinish({ action: 'ask', question: '选\n哪个🚀？', options: ['甲\0方案'] });
    expect(finishSummary()).toBe('ask：选\n哪个🚀？（甲\0方案）');
    // React 转义：文本不进 innerHTML，无 script 节点
    expect(finishCard().querySelector('script')).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// TextBlock：memo 比较器（三字段逐项判等 / 长度先判 / 同引用短路）
// ---------------------------------------------------------------------------

/** 简写引用构造：三展示字段全可控（`raw` 不参与呈现）。 */
function cite(url: string | null, title: string | null, citedText: string | null): AgentCitation {
  return { url, title, citedText, raw: { url } };
}

/** 引用列表条目文本（列表未渲染即空数组）。 */
function citationTexts(): string[] {
  const list = screen.queryByTestId('block-citations');
  if (list === null) return [];
  return Array.from(list.querySelectorAll('li')).map((item) => item.textContent ?? '');
}

describe('TextBlock：比较器三字段单点判据', () => {
  it('仅 url 变化 → 引用列表更新为新 url', () => {
    const view = render(<TextBlock text="正文" citations={[cite('https://a/1', null, null)]} />);
    expect(citationTexts()).toEqual(['https://a/1']);

    view.rerender(<TextBlock text="正文" citations={[cite('https://a/2', null, null)]} />);
    expect(citationTexts()).toEqual(['https://a/2']);
  });

  it('仅 title 变化 → 引用列表更新为新 title', () => {
    const view = render(<TextBlock text="正文" citations={[cite(null, '旧标题', '片段')]} />);
    expect(citationTexts()).toEqual(['旧标题']);

    view.rerender(<TextBlock text="正文" citations={[cite(null, '新标题', '片段')]} />);
    expect(citationTexts()).toEqual(['新标题']);
  });

  it('仅 citedText 变化 → 引用列表更新为新片段', () => {
    const view = render(<TextBlock text="正文" citations={[cite(null, null, '旧片段')]} />);
    expect(citationTexts()).toEqual(['旧片段']);

    view.rerender(<TextBlock text="正文" citations={[cite(null, null, '新片段')]} />);
    expect(citationTexts()).toEqual(['新片段']);
  });
});

describe('TextBlock：比较器部分相等与长度差', () => {
  it('首条逐字相同、次条仅 title 变 → 次条文本更新（every 语义，非 some）', () => {
    const view = render(
      <TextBlock
        text="正文"
        citations={[cite('https://a/1', '甲', null), cite('https://b/2', '旧次条', null)]}
      />,
    );
    expect(citationTexts()).toEqual(['甲', '旧次条']);

    view.rerender(
      <TextBlock
        text="正文"
        citations={[cite('https://a/1', '甲', null), cite('https://b/2', '新次条', null)]}
      />,
    );
    expect(citationTexts()).toEqual(['甲', '新次条']);
  });

  it('长度由 1 变 2（首项逐字相同）→ 列表渲两条（长度先判）', () => {
    const view = render(<TextBlock text="正文" citations={[cite('https://a/1', '甲', null)]} />);
    expect(citationTexts()).toHaveLength(1);

    view.rerender(
      <TextBlock
        text="正文"
        citations={[cite('https://a/1', '甲', null), cite('https://b/2', '乙', null)]}
      />,
    );
    expect(citationTexts()).toEqual(['甲', '乙']);
  });

  it('同一数组引用且 text 不变 → 不抛错、列表保持原样（同引用短路）', () => {
    const citations = [cite('https://a/1', '甲', null)];
    const view = render(<TextBlock text="正文" citations={citations} />);
    expect(citationTexts()).toEqual(['甲']);

    view.rerender(<TextBlock text="正文" citations={citations} />);
    expect(citationTexts()).toEqual(['甲']);
  });

  it('换新数组但三字段逐字全同 → 列表与文案不变（逐项全等分支）', () => {
    const view = render(<TextBlock text="正文" citations={[cite('https://a/1', '甲', '片段')]} />);
    expect(citationTexts()).toEqual(['甲']);

    view.rerender(<TextBlock text="正文" citations={[cite('https://a/1', '甲', '片段')]} />);
    expect(citationTexts()).toEqual(['甲']);
    expect(screen.getByTestId('block-text').textContent).toContain('正文');
  });
});

describe('TextBlock：citations 缺席与三字段全 undefined 引用互换（Optional 边界）', () => {
  it('undefined → [] 与 [] → undefined 两向均不抛错且无列表容器', () => {
    const view = render(<TextBlock text="正文" />);
    expect(citationTexts()).toEqual([]);

    view.rerender(<TextBlock text="正文" citations={[]} />);
    expect(screen.queryByTestId('block-citations')).toBeNull();

    view.rerender(<TextBlock text="正文" />);
    expect(screen.queryByTestId('block-citations')).toBeNull();

    // 对照：有引用即渲染容器（两向兜底未把非空清单吞掉）
    view.rerender(<TextBlock text="正文" citations={[cite('https://a/1', '甲', null)]} />);
    expect(citationTexts()).toEqual(['甲']);
  });

  // `?? []` 两向兜底的可判据面：四个可选字段全 undefined 的引用（类型合法，
  // 序列化后与 `{}` 同形）落 `[]` 会抛 `undefined.length`。判据取「列表容器
  // 与条目数随值变化」——任一侧兜底被改成哨兵数组都会让长度先判错位。
  it('undefined ↔ 三字段全 undefined 引用：两向都重渲染且条目数正确', () => {
    const blank: AgentCitation = {
      url: undefined,
      title: undefined,
      citedText: undefined,
      raw: null,
    };

    const view = render(<TextBlock text="正文" />);
    expect(screen.queryByTestId('block-citations')).toBeNull();

    view.rerender(<TextBlock text="正文" citations={[blank]} />);
    expect(screen.getAllByTestId('block-citations')).toHaveLength(1);
    expect(citationTexts()).toEqual(['']);

    view.rerender(<TextBlock text="正文" />);
    expect(screen.queryByTestId('block-citations')).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// CitationList / dedupeByUrl：四级回退链与无 url 条目面（既有分支）
// ---------------------------------------------------------------------------

describe('CitationList：四级回退链', () => {
  it('title ?? url ?? citedText ?? 空串，空串 title 不继续回退', () => {
    render(
      <TextBlock
        text="正文"
        citations={[
          cite(null, null, null),
          cite('https://only/1', null, '片段'),
          cite(null, null, '仅片段'),
          cite('https://empty-title/1', '', '被遮蔽片段'),
        ]}
      />,
    );
    expect(citationTexts()).toEqual(['', 'https://only/1', '仅片段', '']);
  });
});

describe('dedupeByUrl：无 url 条目逐条保留', () => {
  it('两条无 url 同文引用 → 两条；有 url + 无 url → 两条；两条同 url → 一条', () => {
    const { unmount } = render(
      <TextBlock text="正文" citations={[cite(null, '同文', null), cite(null, '同文', null)]} />,
    );
    expect(citationTexts()).toHaveLength(2);
    unmount();

    const { unmount: unmountMixed } = render(
      <TextBlock
        text="正文"
        citations={[cite('https://dup/1', '甲', null), cite(null, '乙', null)]}
      />,
    );
    expect(citationTexts()).toEqual(['甲', '乙']);
    unmountMixed();

    render(
      <TextBlock
        text="正文"
        citations={[cite('https://dup/1', '甲', null), cite('https://dup/1', '乙', null)]}
      />,
    );
    expect(citationTexts()).toEqual(['甲']);
  });

  it('搜索来源同 url 两条 → 一条（来源面同一去重口径）', () => {
    renderCard(
      searchOutput([source('https://dup/1', '甲来源'), source('https://dup/1', '乙来源')]),
    );
    expandCard();
    expect(
      screen.getByTestId('block-web-search-sources').querySelectorAll('[data-slot="trace-line"]'),
    ).toHaveLength(1);
  });
});

// ---------------------------------------------------------------------------
// ThinkingBlock：state 两态（既有分支）
// ---------------------------------------------------------------------------

describe('ThinkingBlock：state 两态文案互斥', () => {
  it.each([
    ['streaming' as const, '思考中…', 'pending'],
    ['done' as const, '思考', null],
    [undefined, '思考', null],
  ])('state 为 %s → 标题 %s、trace-line 状态 %s', (state, title, status) => {
    const { unmount } = render(<ThinkingBlock text="正文" state={state} />);
    const block = screen.getByTestId('block-thinking');
    expect(block.querySelector('[data-slot="trace-title"]')?.textContent).toBe(title);
    expect(block.querySelector('[data-slot="trace-line"]')?.getAttribute('data-status')).toBe(
      status,
    );
    unmount();
  });

  // 图标选择分支的判据面：Spinner 自带产品锚点（`data-slot` + `aria-label`），
  // 故判据不依赖 lucide 内部 class——两态产出的图标元素不同即可判死四枚突变体
  // （条件整体 → true / false、相等算子取反、字面量 → ""）。
  it.each([
    ['streaming' as const, true],
    ['done' as const, false],
    [undefined, false],
  ])('state 为 %s → 旋载图标在场 %s', (state, spinner) => {
    const { unmount } = render(<ThinkingBlock text="正文" state={state} />);
    const block = screen.getByTestId('block-thinking');
    const icon = block.querySelector('[data-slot="trace-icon"]');
    expect(icon?.querySelector('[data-slot="spinner"]') !== null).toBe(spinner);
    // 非旋载态落 DotIcon（lucide 图标恒一枚）：图标位不空
    expect(icon?.querySelector('svg') !== null).toBe(true);
    unmount();
  });
});

// ---------------------------------------------------------------------------
// WebSearchCard：查询词空值 / 正文与错误空串 / 来源区缺席（既有分支）
// ---------------------------------------------------------------------------

describe('WebSearchCard：查询词空值面（既有分支）', () => {
  it.each([
    [{ query: '' }, false],
    [{ query: '   ' }, true],
    [{ query: 42 }, false],
    [{ query: null }, false],
  ])('input 为 %j → 查询词节点在场 %s', (input, present) => {
    const { unmount } = render(
      <WebSearchCard toolCallId="a" toolName="web_search" input={input} output={null} />,
    );
    expect(screen.queryByTestId('block-web-search-query') !== null).toBe(present);
    // 标题栏文本与查询词一致：省略时不落假串
    expect(summaryText()).toBe(present ? String(input.query) : '');
    unmount();
  });
});

describe('WebSearchCard：正文与错误空串面（既有分支）', () => {
  it('正常结果：展开后卡文本含 content；正文空串不落假串', () => {
    const { unmount } = renderCard({
      content: '来源正文',
      isError: false,
      search: { results: [], error: null },
    });
    expandCard();
    expect(card().textContent).toContain('来源正文');
    unmount();

    // 查询词与正文双双为空 → 卡面零文本（不落假串）
    renderCard({ content: '', isError: false, search: { results: [], error: null } }, null);
    expandCard();
    expect(card().textContent).toBe('');
  });

  // `output?.content ?? ''` 的唯一可观测面：output 缺席时该兜底值**会被挂载**
  // （`{content}` 位在折叠区内恒在场，与来源列表 / 错误区的条件挂载不同）——落哨兵
  // 串即在空卡上现假文本。上例的正文空串走的是 `content: ''` 分支，判不掉它。
  it('output 为 null → 展开后卡面零文本（兜底值不落假串）', () => {
    renderCard(null);
    expandCard();
    const body = card().querySelector('[data-slot="trace-collapsible-content-inner"]');
    expect(body?.textContent).toBe('');
  });

  it('search.error 为空串 → 无错误区且 trace-line 无 data-status；非空串 → 错误区呈错误态', () => {
    const { unmount } = renderCard(searchOutput([], ''));
    expandCard();
    expect(screen.queryByTestId('block-web-search-error')).toBeNull();
    expect(card().querySelector('[data-status]')).toBeNull();
    unmount();

    renderCard(searchOutput([], 'max_uses_exceeded'));
    expandCard();
    expect(screen.getByTestId('block-web-search-error').textContent).toBe('max_uses_exceeded');
    expect(card().querySelector('[data-status="error"]')).not.toBeNull();
  });
});

describe('WebSearchCard：来源区缺席三形态（既有分支）', () => {
  it.each([
    [{ content: '正文', isError: false }],
    [{ content: '正文', isError: false, search: null }],
    [{ content: '正文', isError: false, search: { results: [], error: null } }],
  ])('output 为 %j → 无来源容器且不抛错', (output) => {
    const { unmount } = render(
      <WebSearchCard
        toolCallId="a"
        toolName="web_search"
        input={{ query: 'q' }}
        output={output as unknown as AgentToolOutput}
      />,
    );
    expandCard();
    expect(screen.queryByTestId('block-web-search-sources')).toBeNull();
    expect(card()).toBeTruthy();
    unmount();
  });
});

// ---------------------------------------------------------------------------
// ExternalLink：scheme 行首锚定与非字符串降级（既有分支）
// ---------------------------------------------------------------------------

/** 来源条目（`title` 恒在场以绕开 `new URL` 的主机名回退面）。 */
function titledSource(url: unknown, title = '来源标题'): WebSearchSource {
  return { url: url as string, title };
}

describe('ExternalLink：scheme 行首锚定', () => {
  it.each(['https://a/x', 'HTTPS://A/X', 'http://a/x', 'HTTP://A/X'])(
    '来源 url 为 %s → 渲 <a> 且点击以原样 url 打开一次',
    (url) => {
      const { unmount } = renderCard(searchOutput([titledSource(url)]));
      expandCard();
      const anchor = screen.getByTestId('block-web-search-sources').querySelector('a');
      expect(anchor?.getAttribute('href')).toBe(url);

      fireEvent.click(anchor as HTMLElement);
      expect(openUrlMock).toHaveBeenCalledTimes(1);
      expect(openUrlMock).toHaveBeenCalledWith(url);
      unmount();
    },
  );

  it.each([
    'javascript:alert(1)',
    'file:///etc/passwd',
    'mailto:a@b.c',
    '/relative/path',
    'prefixhttps://evil.com',
    ' https://leading-space.example',
  ])('来源 url 为 %s → 降级纯文本、点击零 openUrl', (url) => {
    const { unmount } = renderCard(searchOutput([titledSource(url)]));
    expandCard();
    const item = screen
      .getByTestId('block-web-search-sources')
      .querySelector('[data-slot="trace-line"]');
    expect(item?.querySelector('a')).toBeNull();
    expect(item?.textContent).toBe('来源标题');

    fireEvent.click(item as HTMLElement);
    expect(openUrlMock).not.toHaveBeenCalled();
    unmount();
  });

  it('引用条目共用同一 scheme 判据（mailto 纯文本 / https 链接）', () => {
    render(
      <TextBlock
        text="正文"
        citations={[cite('mailto:a@b.c', '邮件', null), cite('https://cite/2', '网页', null)]}
      />,
    );
    const items = screen.getByTestId('block-citations').querySelectorAll('li');
    expect(items[0]?.querySelector('a')).toBeNull();
    expect(items[1]?.querySelector('a')?.getAttribute('href')).toBe('https://cite/2');
  });
});

describe('ExternalLink：非字符串与非 http 边界（str 边界）', () => {
  it.each([null, undefined, 42, { href: 'https://x' }, ['https://array/1']])(
    '来源 url 为 %j → 降级纯文本、不抛错、零 openUrl',
    (url) => {
      const { unmount } = renderCard(searchOutput([titledSource(url)]));
      expandCard();
      expect(screen.getByTestId('block-web-search-sources').querySelector('a')).toBeNull();
      expect(openUrlMock).not.toHaveBeenCalled();
      unmount();
    },
  );

  // 数组也是对象：`/^https?:\/\//i.test(url)` 会把它 String 成 "https://array/1"
  // 而判真，`typeof url === 'string'` 是该判据的唯一防线（去掉即渲出 href 为
  // 数组串的假链接）。上面的 `{ href: … }` 形态 String 后不命中正则，杀不掉它。
  it('非字符串但 String 后命中 scheme 的目标（数组）→ 仍降级纯文本', () => {
    renderCard(searchOutput([titledSource(['https://array/1'])]));
    expandCard();
    const item = screen
      .getByTestId('block-web-search-sources')
      .querySelector('[data-slot="trace-line"]');
    expect(item?.querySelector('a')).toBeNull();
    expect(item?.textContent).toBe('来源标题');
  });
});

// ---------------------------------------------------------------------------
// isToolPart：收窄面（既有分支）
// ---------------------------------------------------------------------------

describe('isToolPart：收窄面', () => {
  it.each([
    ['tool-read', true],
    ['tool-finish_step', true],
    ['dynamic-tool', false],
    ['text', false],
    ['data-raw', false],
    ['step-start', false],
    ['', false],
  ])('type 为 %j → %s', (type, expected) => {
    const part = { type, toolCallId: 'x', state: 'input-available', input: null };
    expect(isToolPart(part as unknown as AgentUIMessage['parts'][number])).toBe(expected);
  });
});
