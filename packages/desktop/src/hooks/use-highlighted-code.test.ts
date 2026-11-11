import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { useHighlightedCode } from './use-highlighted-code';

const { loadHighlighterMock } = vi.hoisted(() => ({ loadHighlighterMock: vi.fn() }));

// 进程边界 Mock：hook 只消费 `loadHighlighter` 的返回形态（`highlight` 产出行
// 与 grammar state 续接），shiki 本体不参与（真实装载在 jsdom 下慢且与判据无关）。
vi.mock('@/lib/highlighter', () => ({ loadHighlighter: loadHighlighterMock }));

/**
 * 高亮替身：每行产出可断言文本，state 串接输入以便校验增量续接。
 * `tag` 区分不同语言的实例（切换语言的判据用）。
 */
function fakeHighlighter(tag = 'h') {
  const highlight = vi.fn((code: string, state?: string) => ({
    lines:
      code === ''
        ? ['<empty>']
        : code.split('\n').map((line) => `<${tag}:${state ?? 'fresh'}|${line}>`),
    state: `${state ?? ''}+${code.replaceAll('\n', '/')}`,
  }));
  return { highlight };
}

/** 可控 promise：手动决定装载何时完成（观测「装载中」态）。 */
function deferred<T>() {
  let resolve: (value: T) => void = () => {};
  let reject: (error: unknown) => void = () => {};
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

/** 让装载 promise 的 then / catch 链跑完（一次微任务冲洗，无轮询）。 */
async function flush(): Promise<void> {
  await act(async () => {
    await Promise.resolve();
  });
}

function renderCode(value: string, language: string) {
  return renderHook(({ v, l }: { v: string; l: string }) => useHighlightedCode(v, l), {
    initialProps: { v: value, l: language },
  });
}

beforeEach(() => {
  loadHighlighterMock.mockReset();
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe('useHighlightedCode：按语言装 highlighter', () => {
  it('装载未就绪：按行回退纯文本，不等装载结果', () => {
    const pending = deferred();
    loadHighlighterMock.mockReturnValue(pending.promise);
    const { result } = renderCode('a\nb\n', 'ts');

    expect(result.current).toEqual(['a', 'b']);
    expect(loadHighlighterMock).toHaveBeenCalledTimes(1);
    expect(loadHighlighterMock).toHaveBeenCalledWith('ts');
  });

  it('语言未登记（loadHighlighter 回 null）：纯文本回退、不抛', async () => {
    loadHighlighterMock.mockResolvedValue(null);
    const { result } = renderCode('const x = 1', 'unknown-lang');

    await flush();
    expect(result.current).toEqual(['const x = 1']);
  });

  it('装载完成：返回高亮行，完整行从初始 grammar state 开始 tokenize', async () => {
    const highlighter = fakeHighlighter();
    loadHighlighterMock.mockResolvedValue(highlighter);
    const { result } = renderCode('a\nb\n', 'ts');

    await waitFor(() => expect(result.current).toEqual(['<h:fresh|a>', '<h:+a|b>']));
    expect(highlighter.highlight).toHaveBeenNthCalledWith(1, 'a', undefined);
  });

  it("尾部换行只切一个：'a\\n\\n' 保留一个空行", async () => {
    loadHighlighterMock.mockResolvedValue(fakeHighlighter());
    const { result } = renderCode('a\n\n', 'ts');

    await waitFor(() => expect(result.current).toEqual(['<h:fresh|a>', '<empty>']));
  });

  it('值变化不触发重新装载：语言未变时 loadHighlighter 恰一次', async () => {
    loadHighlighterMock.mockResolvedValue(fakeHighlighter());
    const { result, rerender } = renderCode('a\nb', 'ts');

    await waitFor(() => expect(result.current).toHaveLength(2));
    rerender({ v: 'a\nb\nc', l: 'ts' });
    await flush();

    expect(loadHighlighterMock).toHaveBeenCalledTimes(1);
  });

  it('装载失败：静默回退纯文本，不 console.error / warn', async () => {
    const errorSpy = vi.spyOn(console, 'error');
    const warnSpy = vi.spyOn(console, 'warn');
    loadHighlighterMock.mockRejectedValue(new Error('shiki 装载失败'));
    const { result } = renderCode('a\nb', 'ts');

    await flush();
    expect(result.current).toEqual(['a', 'b']);
    expect(errorSpy).not.toHaveBeenCalled();
    expect(warnSpy).not.toHaveBeenCalled();
  });

  it('卸载后装载完成：不产生状态更新告警（cancelled 守卫）', async () => {
    const errorSpy = vi.spyOn(console, 'error');
    const pending = deferred();
    loadHighlighterMock.mockReturnValue(pending.promise);
    const { unmount } = renderCode('a', 'ts');

    unmount();
    await act(async () => {
      pending.resolve(fakeHighlighter());
    });

    expect(errorSpy).not.toHaveBeenCalled();
  });
});

describe('useHighlightedCode：增量 tokenize 与缓存键', () => {
  it('流式追加换行：从缓存前缀末端续接，不重跑已缓存行', async () => {
    const highlighter = fakeHighlighter();
    loadHighlighterMock.mockResolvedValue(highlighter);
    const { result, rerender } = renderCode('a\nb\n', 'ts');

    await waitFor(() => expect(result.current).toEqual(['<h:fresh|a>', '<h:+a|b>']));
    highlighter.highlight.mockClear();

    rerender({ v: 'a\nb\nc\nd', l: 'ts' });

    await waitFor(() =>
      expect(result.current).toEqual(['<h:fresh|a>', '<h:+a|b>', '<h:+a|c>', '<h:+a+b/c|d>']),
    );
    expect(highlighter.highlight).toHaveBeenCalledWith('b\nc', '+a');
  });

  it('代码被重写（非前缀）：清空累积行，全量重高亮且 state 从头开始', async () => {
    const highlighter = fakeHighlighter();
    loadHighlighterMock.mockResolvedValue(highlighter);
    const { result, rerender } = renderCode('a\nb\n', 'ts');

    await waitFor(() => expect(result.current).toEqual(['<h:fresh|a>', '<h:+a|b>']));
    highlighter.highlight.mockClear();

    rerender({ v: 'x\ny', l: 'ts' });

    await waitFor(() => expect(result.current).toEqual(['<h:fresh|x>', '<h:+x|y>']));
    expect(highlighter.highlight).toHaveBeenCalledWith('x', undefined);
  });

  it('未完成尾行逐次重新 tokenize：单行增长不产生累积缓存', async () => {
    const highlighter = fakeHighlighter();
    loadHighlighterMock.mockResolvedValue(highlighter);
    const { result, rerender } = renderCode('ab', 'ts');

    await waitFor(() => expect(result.current).toEqual(['<h:fresh|ab>']));
    rerender({ v: 'abc', l: 'ts' });
    await waitFor(() => expect(result.current).toEqual(['<h:fresh|abc>']));

    expect(highlighter.highlight).toHaveBeenCalledWith('ab', undefined);
    expect(highlighter.highlight).toHaveBeenCalledWith('abc', undefined);
  });

  it('语言切换：旧高亮即时失效回退纯文本，新语言就绪后全量重高亮', async () => {
    loadHighlighterMock.mockResolvedValue(fakeHighlighter('ts'));
    const { result, rerender } = renderCode('a\nb', 'ts');

    await waitFor(() => expect(result.current).toEqual(['<ts:fresh|a>', '<ts:+a|b>']));

    const pending = deferred();
    loadHighlighterMock.mockReturnValue(pending.promise);
    rerender({ v: 'a\nb', l: 'py' });

    expect(result.current).toEqual(['a', 'b']);

    await act(async () => {
      pending.resolve(fakeHighlighter('py'));
    });

    await waitFor(() => expect(result.current).toEqual(['<py:fresh|a>', '<py:+a|b>']));
  });
});
