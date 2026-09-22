import { render, screen, within } from '@testing-library/react';
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
    // 加粗结构经语义文本查询断言（不再使用 strong 标签查询）
    expect(screen.getByText('加粗').tagName).toBe('STRONG');
  });

  it('AC-7：GFM 表格在 markdown-root 域内呈现完整表格语义结构', () => {
    render(
      <MarkdownDocRenderer
        envelope={envelope({ markdown: '| 阶段 | 结论 |\n| --- | --- |\n| proposal | pass |' })}
      />,
    );
    // prose 承载裸元素样式后，结构断言改经 markdown-root 挂钩 + role 语义查询
    const root = screen.getByTestId('markdown-root');
    const table = within(root).getByRole('table');
    const headers = within(table)
      .getAllByRole('columnheader')
      .map((cell) => cell.textContent ?? '');
    expect(headers).toEqual(['阶段', '结论']);
    const rows = within(table).getAllByRole('row');
    expect(rows).toHaveLength(2); // 表头行 + 1 数据行
    expect(
      within(rows[1])
        .getAllByRole('cell')
        .map((cell) => cell.textContent ?? ''),
    ).toEqual(['proposal', 'pass']);
  });

  it('AC-7：围栏代码块内容在 markdown-root 域内经文本查询可断言进入 DOM', () => {
    render(
      <MarkdownDocRenderer envelope={envelope({ markdown: '```ts\nconst answer = 42;\n```' })} />,
    );
    const root = screen.getByTestId('markdown-root');
    // 不引入新的标签查询：代码内容以文本查询断言
    expect(within(root).getByText('const answer = 42;') !== null).toBe(true);
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

  it('markdown 为空串时 prose 容器承载空内容不崩', () => {
    render(<MarkdownDocRenderer envelope={envelope({ markdown: '' })} />);
    expect(screen.getByTestId('markdown-root') !== null).toBe(true);
  });
});
