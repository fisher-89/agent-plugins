import { render, screen, within } from '@testing-library/react';
import { describe, expect, it } from 'vite-plus/test';

import { FileLogTable } from './file-log-table';
import type { FileLogEntry } from './types';

// ---------------------------------------------------------------------------
// FileLogTable 单测：纯展示组件（无 Mock），shadcn/ui Table 直接渲染。
// workflow 独立面板与抽屉文件表节共用（AC-5）。
// ---------------------------------------------------------------------------

function entry(overrides: Partial<FileLogEntry> = {}): FileLogEntry {
  return {
    op: 'write',
    scope: 'workflow',
    attempt: 1,
    path: 'src/a.rs',
    at: '2026-09-03T00:00:00Z',
    ...overrides,
  };
}

describe('FileLogTable：file_log 条目表体', () => {
  it('条目逐行渲染 op / scope / attempt / path / at 五列矩阵', () => {
    render(
      <FileLogTable
        entries={[
          entry(),
          entry({ op: 'delete', scope: 'dev-design', attempt: 3, path: 'src/b.ts', at: null }),
        ]}
      />,
    );
    const table = screen.getByTestId('filelog-table');
    const rows = within(table).getAllByRole('row');
    expect(rows).toHaveLength(3); // 表头 + 2 数据行
    const headerCells = within(rows[0])
      .getAllByRole('columnheader')
      .map((cell) => cell.textContent ?? '');
    expect(headerCells).toEqual(['op', 'scope', 'attempt', 'path', 'at']);
    const matrix = rows.slice(1).map((row) =>
      within(row)
        .getAllByRole('cell')
        .map((cell) => cell.textContent ?? ''),
    );
    expect(matrix[0]).toEqual(['write', 'workflow', '1', 'src/a.rs', '2026-09-03T00:00:00Z']);
    expect(matrix[1]).toEqual(['delete', 'dev-design', '3', 'src/b.ts', '—']);
  });

  it('空数组 → 「（空）」占位，无 filelog-table 表格', () => {
    const { container } = render(<FileLogTable entries={[]} />);
    expect(container.textContent).toContain('（空）');
    expect(screen.queryByTestId('filelog-table')).toBeNull();
  });

  it('attempt=null 与 at=null 的条目 → 「—」占位符（不渲染 "null" 字面量）', () => {
    render(<FileLogTable entries={[entry({ attempt: null, at: null })]} />);
    const table = screen.getByTestId('filelog-table');
    const dataRow = within(table).getAllByRole('row')[1];
    const cells = within(dataRow)
      .getAllByRole('cell')
      .map((cell) => cell.textContent ?? '');
    expect(cells).toEqual(['write', 'workflow', '—', 'src/a.rs', '—']);
    expect(table.textContent).not.toContain('null');
  });
});
