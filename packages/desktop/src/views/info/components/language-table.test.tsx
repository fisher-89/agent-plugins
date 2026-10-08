// @vitest-environment jsdom
import { render, screen, within } from '@testing-library/react';
import { describe, expect, it } from 'vite-plus/test';

import type { LanguageStats } from '../../../types/generated/bindings';
import { LanguageTable } from './language-table';

// ---------------------------------------------------------------------------
// fixture：纯 props 驱动组件（排序职责在 inner，本组件不重排），零 mock
// ---------------------------------------------------------------------------

function row(name: string, overrides: Partial<LanguageStats> = {}): LanguageStats {
  return { name, files: 6, code: 220, comments: 30, blanks: 10, share: 64.7, ...overrides };
}

/** 行内占比单元格的 Progress 指示条宽度（base-ui progress 以内联 width 百分比承载）。 */
function indicatorWidth(row: HTMLElement): string {
  const indicator = row.querySelector('[data-slot="progress-indicator"]');
  if (!indicator) throw new Error('行内缺 progress-indicator');
  return (indicator as HTMLElement).style.width;
}

describe('LanguageTable：语言占比表呈现（AC-3）', () => {
  it('多语言行按入参序渲染（组件不重排），每行 info-language-row 六列内容与入参一致', () => {
    const languages = [
      row('Rust'),
      row('TypeScript', { code: 120, comments: 10, share: 35.3 }),
      row('Markdown', { files: 2, code: 0, comments: 1, blanks: 4, share: 0 }),
    ];
    render(<LanguageTable languages={languages} />);

    const rows = screen.getAllByTestId('info-language-row');
    expect(rows).toHaveLength(3);
    expect(rows.map((item) => item.getAttribute('data-lang'))).toEqual([
      'Rust',
      'TypeScript',
      'Markdown',
    ]);

    const rustCells = within(rows[0]).getAllByRole('cell');
    expect(rustCells).toHaveLength(6);
    expect(rustCells[0].textContent).toBe('Rust');
    expect(rustCells[1].textContent).toBe('6');
    expect(rustCells[2].textContent).toBe('220');
    expect(rustCells[3].textContent).toBe('30');
    expect(rustCells[4].textContent).toBe('10');
    expect(rustCells[5].textContent).toContain('64.7%');
  });

  it('空数组：表头在场、零数据行、不崩（页面级空态由 InfoView 承载）', () => {
    render(<LanguageTable languages={[]} />);

    expect(screen.getByTestId('info-language-table') !== null).toBe(true);
    expect(screen.getByText('语言') !== null).toBe(true);
    expect(screen.queryAllByTestId('info-language-row')).toHaveLength(0);
  });

  it('单语言行：单行渲染正常', () => {
    render(<LanguageTable languages={[row('Rust')]} />);

    const rows = screen.getAllByTestId('info-language-row');
    expect(rows).toHaveLength(1);
    expect(within(rows[0]).getAllByRole('cell')).toHaveLength(6);
  });

  it('share=0.0 与 share=100.0：Progress value 端点呈现；null share 以 0 防御（`share ?? 0`）', () => {
    render(
      <LanguageTable
        languages={[
          row('Zero', { share: 0 }),
          row('Full', { share: 100 }),
          row('Null', { share: null }),
        ]}
      />,
    );

    const rows = screen.getAllByTestId('info-language-row');
    // value 0 → 指示条 width 0%（零填充）；100 → 100%（满填充）
    expect(indicatorWidth(rows[0])).toBe('0%');
    expect(indicatorWidth(rows[1])).toBe('100%');
    expect(indicatorWidth(rows[2])).toBe('0%');
    expect(within(rows[0]).getByText('0.0%') !== null).toBe(true);
    expect(within(rows[1]).getByText('100.0%') !== null).toBe(true);
    expect(within(rows[2]).getByText('0.0%') !== null).toBe(true);
  });

  it('50 行语言全量渲染、行数一致无丢失', () => {
    const many = Array.from({ length: 50 }, (_, index) => row(`lang-${index}`));
    render(<LanguageTable languages={many} />);

    expect(screen.getAllByTestId('info-language-row')).toHaveLength(50);
  });

  it('指示条宽度与 share 一致：行子树内携带 width 的内联节点仅 progress-indicator 且值逐行对应', () => {
    const { container } = render(
      <LanguageTable languages={[row('Rust'), row('TypeScript', { share: 35.3 })]} />,
    );

    const rows = screen.getAllByTestId('info-language-row');
    expect(indicatorWidth(rows[0])).toBe('64.7%');
    expect(indicatorWidth(rows[1])).toBe('35.3%');
    // 行子树内携带百分比 width 的内联节点只有 progress-indicator（base-ui
    // progress 另有 1px 内部测量节点，非占比承载面，不计入）
    const percentBearers = Array.from(
      container.querySelectorAll('[data-testid="info-language-row"] [style]'),
    ).filter((element) => (element as HTMLElement).style.width.endsWith('%'));
    expect(percentBearers).toHaveLength(2);
    for (const element of percentBearers) {
      expect(element.getAttribute('data-slot')).toBe('progress-indicator');
    }
  });
});
