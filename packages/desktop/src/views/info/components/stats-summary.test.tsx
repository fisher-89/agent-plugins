// @vitest-environment jsdom
import { render, screen, within } from '@testing-library/react';
import { describe, expect, it } from 'vite-plus/test';

import type { CodeTotals } from '../../../types/generated/bindings';
import { StatsSummary } from './stats-summary';

// ---------------------------------------------------------------------------
// fixture：纯 props 驱动组件，jsdom 真实渲染断言 DOM，零 mock
// ---------------------------------------------------------------------------

function totals(overrides: Partial<CodeTotals> = {}): CodeTotals {
  return { files: 1234, code: 340, comments: 40, blanks: 20, ...overrides };
}

/** 汇总项内取数值 span 文本（SummaryItem 结构：label span + value span）。 */
function valueOf(label: string): string {
  const labelNode = screen.getByText(label);
  const item = labelNode.parentElement;
  if (!item || item.children.length !== 2) {
    throw new Error(`汇总项 ${label} 结构不符`);
  }
  return item.children[1].textContent ?? '';
}

describe('StatsSummary：汇总面呈现（AC-3）', () => {
  it('四项数值与入参逐项一致，容器带 info-summary', () => {
    render(<StatsSummary totals={totals()} />);

    expect(screen.getByTestId('info-summary') !== null).toBe(true);
    expect(valueOf('文件数')).toBe('1,234');
    expect(valueOf('代码行')).toBe('340');
    expect(valueOf('注释行')).toBe('40');
    expect(valueOf('空行')).toBe('20');
  });

  it('全 0 totals：四个 0 渲染不崩', () => {
    render(<StatsSummary totals={totals({ files: 0, code: 0, comments: 0, blanks: 0 })} />);

    const container = screen.getByTestId('info-summary');
    expect(within(container).queryAllByText('0')).toHaveLength(4);
  });

  it('大数值（> 2^32，如 5000000000）：完整数值呈现（u64 → number 出线口径）', () => {
    render(<StatsSummary totals={totals({ files: 5000000000 })} />);

    // 全精度呈现：非 2^53 精度丢失、非截断、非科学计数
    expect(valueOf('文件数')).toBe('5,000,000,000');
  });

  it('仅呈现四项、无额外合计 / 派生行（lines 不预建口径）', () => {
    render(<StatsSummary totals={totals()} />);

    const container = screen.getByTestId('info-summary');
    expect(container.children).toHaveLength(4);
    expect(within(container).queryByText(/合计/)).toBeNull();
    expect(within(container).queryByText(/总行/)).toBeNull();
  });
});
