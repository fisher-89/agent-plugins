// @vitest-environment jsdom
import { render } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { ExploreDoc } from '../../../types/dto';
import { ExplorePreview } from './explore-preview';

// ---------------------------------------------------------------------------
// 右栏预览：流式 delta 期间 doc/loading/onRefresh 稳定，不应重解析笔记；
// 只有内容字符串变化才重新触发 react-markdown（保留真实实现，仅观测调用）。
// ---------------------------------------------------------------------------

const { markdownSpy } = vi.hoisted(() => ({ markdownSpy: vi.fn() }));

vi.mock(import('react-markdown'), async (importOriginal) => {
  const actual = await importOriginal();
  markdownSpy.mockImplementation(actual.default);
  return { ...actual, default: markdownSpy };
});

const DOC: ExploreDoc = { name: 'ipc-zod-runtime-schema', content: '# 笔记标题\n\n正文' };
const refresh = vi.fn();

beforeEach(() => {
  markdownSpy.mockClear();
});

describe('ExplorePreview：流式期间不重解析笔记', () => {
  it('同 doc / 仅 loading 变化 → 零解析；内容变化 → 恰一次解析', () => {
    const { rerender } = render(<ExplorePreview doc={DOC} loading={false} onRefresh={refresh} />);
    expect(markdownSpy).toHaveBeenCalledTimes(1);

    markdownSpy.mockClear();
    rerender(<ExplorePreview doc={DOC} loading={false} onRefresh={refresh} />);
    expect(markdownSpy).not.toHaveBeenCalled();

    rerender(<ExplorePreview doc={DOC} loading onRefresh={refresh} />);
    expect(markdownSpy).not.toHaveBeenCalled();

    markdownSpy.mockClear();
    rerender(
      <ExplorePreview
        doc={{ ...DOC, content: '# 新标题\n\n新正文' }}
        loading={false}
        onRefresh={refresh}
      />,
    );
    expect(markdownSpy).toHaveBeenCalledTimes(1);
  });
});
