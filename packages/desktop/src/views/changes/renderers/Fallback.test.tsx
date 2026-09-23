import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vite-plus/test';

import type { ArtifactEnvelope } from '../../../types/dto';
import { Fallback } from './Fallback';

/** 内存构造信封。 */
function envelope(overrides: Partial<ArtifactEnvelope> = {}): ArtifactEnvelope {
  return {
    kind: 'file-log',
    version: 0,
    title: '未注册产物',
    payload: null,
    fallbackText: '保底文本：这里是一段降级说明。',
    ...overrides,
  };
}

describe('Fallback：未注册 kind 的兜底组件（永不白屏）', () => {
  it('渲染 fallback_text 文本', () => {
    render(<Fallback envelope={envelope()} />);
    expect(screen.getByText('保底文本：这里是一段降级说明。') !== null).toBe(true);
  });

  it('fallback_text 为 null 时渲染占位文案而非空白', () => {
    render(<Fallback envelope={envelope({ fallbackText: null })} />);
    expect(screen.getByText('（该产物类型暂无渲染器，且无保底文本）') !== null).toBe(true);
  });

  it('超长 fallback_text（>1000 字符）完整渲染不崩', () => {
    const long = '长文本片段—'.repeat(300); // 远超 1000 字符
    render(<Fallback envelope={envelope({ fallbackText: long })} />);
    const node = screen.getByText(long);
    expect(node.textContent).toBe(long);
  });

  it('envelope 缺 kind 字段时不抛错并渲染兜底形态', () => {
    const broken: ArtifactEnvelope = {
      kind: '',
      version: 0,
      title: 'x',
      payload: null,
      fallbackText: '仍可保底',
    };
    expect(() => render(<Fallback envelope={broken} />)).not.toThrow();
    expect(screen.getByText('仍可保底') !== null).toBe(true);
  });
});
