import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vite-plus/test';

import type { ArtifactEnvelope } from '../types/dto';
import { MarkdownDocRenderer } from './MarkdownDocRenderer';

function envelope(payload: unknown, fallbackText: string | null = null): ArtifactEnvelope {
  return { kind: 'markdown-doc', version: 1, title: '提案', payload, fallbackText };
}

describe('MarkdownDocRenderer：payload.markdown 的收窄与渲染组装', () => {
  it('payload.markdown 为字符串时渲染出内容节点', () => {
    const { container } = render(
      <MarkdownDocRenderer envelope={envelope({ markdown: '# 提案标题\n\n正文**加粗**段落。' })} />,
    );
    // react-markdown 解析语义不属被测对象，此处只断言内容进入 DOM
    expect(screen.getByText('提案标题') !== null).toBe(true);
    expect(container.textContent).toContain('正文加粗段落。');
    expect(container.querySelector('strong')?.textContent).toBe('加粗');
  });

  it('payload 缺 markdown 字段时降级为 fallback_text，不抛错', () => {
    render(<MarkdownDocRenderer envelope={envelope({ other: 1 }, '保底：原文内容。')} />);
    expect(screen.getByText('保底：原文内容。') !== null).toBe(true);
  });

  it('payload.markdown 类型漂移（非字符串）时不抛错并降级', () => {
    render(<MarkdownDocRenderer envelope={envelope({ markdown: 42 })} />);
    expect(screen.getByText('（无 markdown 内容）') !== null).toBe(true);
  });

  it('payload 为 null 时降级到默认空态', () => {
    render(<MarkdownDocRenderer envelope={envelope(null)} />);
    expect(screen.getByText('（无 markdown 内容）') !== null).toBe(true);
  });

  it('markdown 为空串时渲染空容器不崩', () => {
    const { container } = render(<MarkdownDocRenderer envelope={envelope({ markdown: '' })} />);
    expect(container.querySelector('.markdown-doc') !== null).toBe(true);
  });
});
