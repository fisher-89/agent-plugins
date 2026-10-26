// @vitest-environment jsdom
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { AgentToolOutput, AgentUIMessage } from '../../lib/agent-adapter';
import type { AgentCitation, WebSearchSource } from '../../types/dto';
import { TextBlock, ThinkingBlock, ToolPairCard, WebSearchCard, isToolPart } from './agent-blocks';

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
  render(
    <WebSearchCard toolCallId="srvtoolu_1" toolName="web_search" input={input} output={output} />,
  );
}

/** 卡面 root（`block-web-search`）。 */
function card(): HTMLElement {
  return screen.getByTestId('block-web-search');
}

/** 卡面 summary 文案（恒「网络搜索」）。 */
function summaryText(): string {
  return card().querySelector('summary')?.textContent ?? '';
}

// ---------------------------------------------------------------------------
// WebSearchCard：恒标题两态 / body 三区 / 错误面
// ---------------------------------------------------------------------------

describe('WebSearchCard：summary 恒「网络搜索」（无状态文案）', () => {
  it('output 为 null（未落定）与 output 在场（落定）两态 summary 文本一致', () => {
    const { unmount } = render(
      <WebSearchCard toolCallId="a" toolName="web_search" input={{ query: '甲' }} output={null} />,
    );
    const pendingText = summaryText();
    const pendingHtml = card().outerHTML;
    unmount();

    renderCard(searchOutput([source('https://a/1')]));
    const settledText = summaryText();
    const settledHtml = card().outerHTML;

    expect(pendingText).toBe('网络搜索');
    expect(settledText).toBe('网络搜索');
    // 两态结构一致：仅 body 内容差异（output 在场才渲来源区）
    expect(pendingHtml.includes('block-web-search-sources')).toBe(false);
    expect(settledHtml.includes('block-web-search-sources')).toBe(true);
  });

  it('文本不含进行中 / 加载 / 失败类状态文案（决策 ③：无搜索实时 delta）', () => {
    renderCard(null);
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

    expect(screen.getByTestId('block-web-search-query').textContent).toContain('今日汇率');
    const sources = screen.getByTestId('block-web-search-sources');
    expect(sources.querySelectorAll('li')).toHaveLength(3);
    expect(sources.textContent).toContain('甲来源');
    expect(sources.textContent).toContain('https://c/3'); // title 缺席回退 url
    expect(card().getAttribute('data-tool-id')).toBe('任意-id');
  });

  it('来源按 url 去重且 title 为 null 的项回退显示 url', () => {
    renderCard(
      searchOutput([
        source('https://dup/1', '同名来源'),
        source('https://dup/1', '重复来源'),
        source('https://only/2'),
      ]),
    );

    const items = screen.getByTestId('block-web-search-sources').querySelectorAll('li');
    expect(items).toHaveLength(2);
    expect(items[0].textContent).toBe('同名来源');
    expect(items[1].textContent).toBe('https://only/2');
  });

  it('错误码在场时错误区呈现，且来源列表不被吞（同数据两区并存）', () => {
    renderCard(searchOutput([source('https://keep/1', '保留来源')], 'max_uses_exceeded'));

    expect(screen.getByTestId('block-web-search-error').textContent).toContain('max_uses_exceeded');
    expect(screen.getByTestId('block-web-search-sources').textContent).toContain('保留来源');
    expect(card().textContent).not.toContain('出错'); // 卡面无独立错误标记位（错误码即标记）
  });
});

describe('WebSearchCard：空载荷与孤儿形态（边界）', () => {
  it('input 为 null（result 侧孤儿）→ 不显查询词、不 panic', () => {
    renderCard(searchOutput([source('https://orphan/1')]), null);

    expect(screen.queryByTestId('block-web-search-query')).toBeNull();
    expect(card()).toBeTruthy();
  });

  it('search.results 为空数组 → 不渲空列表容器', () => {
    renderCard(searchOutput([]));

    expect(screen.queryByTestId('block-web-search-sources')).toBeNull();
  });

  it('output 为 null → 不显来源区与错误区', () => {
    renderCard(null);

    expect(screen.queryByTestId('block-web-search-sources')).toBeNull();
    expect(screen.queryByTestId('block-web-search-error')).toBeNull();
  });

  it('input.query 为空串 / 非对象 input → 不显空标签', () => {
    const { unmount } = render(
      <WebSearchCard toolCallId="a" toolName="web_search" input={{ query: '' }} output={null} />,
    );
    expect(screen.queryByTestId('block-web-search-query')).toBeNull();
    unmount();

    render(<WebSearchCard toolCallId="a" toolName="web_search" input="非对象" output={null} />);
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

      const item = screen.getByTestId('block-web-search-sources').querySelector('li');
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

    expect(
      screen.getByTestId('block-web-search-sources').querySelector('a')?.getAttribute('href'),
    ).toBe('http://example.com/plain');
  });

  it('openUrl reject → 无未捕获拒绝、卡面与来源列表照常渲染', async () => {
    openUrlMock.mockRejectedValue(new Error('scope 拒绝'));
    renderCard(searchOutput([source('https://example.com/fail', '失败来源')]));

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
  it('ThinkingBlock 折叠呈现（details + summary 文案「思考」）', () => {
    render(<ThinkingBlock text="先想一想" />);

    const block = screen.getByTestId('block-thinking');
    expect(block.tagName).toBe('DETAILS');
    expect(block.querySelector('summary')?.textContent).toBe('思考');
    expect(block.textContent).toContain('先想一想');
  });

  it('ToolPairCard：input / output 成对呈现，isError 高亮标记在场', () => {
    render(
      <ToolPairCard
        name="Bash"
        toolCallId="tu_1"
        input={{ command: 'ls' }}
        output={{ content: '目录内容', isError: true }}
      />,
    );

    const cardNode = screen.getByTestId('block-tool-use');
    expect(cardNode.textContent).toContain('Bash');
    expect(cardNode.textContent).toContain('出错');
    expect(screen.getByTestId('block-tool-result').textContent).toBe('目录内容');
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
