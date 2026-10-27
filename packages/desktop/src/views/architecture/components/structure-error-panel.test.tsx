import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vite-plus/test';

import type { ArchiDiagnostic } from '../../../types/generated/bindings';
import { StructureErrorPanel } from './structure-error-panel';

/**
 * `StructureErrorPanel` 单测（AC-4 / D5）：诊断逐条呈现（`kind` + `message` +
 * `file:line`）、位置未知只渲文件、「修好后再看」引导文案、清单规模不截断。
 * 诊断数据为 fixture 直构（纯展示组件，零 IPC）。
 */

function diagnostic(
  kind: ArchiDiagnostic['kind'],
  line: number,
  column: number | null,
  message = '结构错详情',
): ArchiDiagnostic {
  return {
    kind,
    message,
    file: 'openspec/architecture/models/01-core.toml',
    line,
    column,
  };
}

/** 行文本（`textContent` 统一收口）。 */
function textOf(element: HTMLElement): string {
  return element.textContent ?? '';
}

afterEach(() => {
  cleanup();
});

describe('停摆面板诊断呈现', () => {
  it('应逐条渲染 kind / message / file:line 三要素', () => {
    render(
      <StructureErrorPanel
        diagnostics={[
          diagnostic('cycle', 12, 1, '依赖成环：App.A -> App.B -> App.A'),
          diagnostic('unknownReference', 20, 3, '未定义引用：App.Auth -> App.Shared'),
          diagnostic('missingField', 4, 1, 'missing field `kind`'),
        ]}
      />,
    );

    const rows = screen.getAllByTestId('archi-error-row');
    expect(rows).toHaveLength(3);
    expect(textOf(rows[0] as HTMLElement)).toContain('cycle');
    expect(textOf(rows[0] as HTMLElement)).toContain(
      'openspec/architecture/models/01-core.toml:12',
    );
    expect(textOf(rows[0] as HTMLElement)).toContain('依赖成环：App.A -> App.B -> App.A');
    expect(textOf(rows[1] as HTMLElement)).toContain('unknownReference');
    expect(textOf(rows[2] as HTMLElement)).toContain('missingField');
    expect(screen.getByText('模型结构错（3 条）')).toBeTruthy();
  });

  it('应在行号未知时只渲文件（不出现 :0）', () => {
    render(
      <StructureErrorPanel
        diagnostics={[
          diagnostic(
            'syntaxError',
            0,
            null,
            '模型文件读取失败：stream did not contain valid UTF-8',
          ),
          diagnostic('missingField', 4, 1),
          diagnostic('invalidValue', 7, 20),
        ]}
      />,
    );
    const rows = screen.getAllByTestId('archi-error-row');
    expect(textOf(rows[0] as HTMLElement)).toContain('openspec/architecture/models/01-core.toml');
    expect(textOf(rows[0] as HTMLElement)).not.toContain(':0');
    expect(textOf(rows[1] as HTMLElement)).toContain(':4');
    expect(textOf(rows[2] as HTMLElement)).toContain(':7');
  });

  it('应呈现「修好后再看」引导文案且不含图数据', () => {
    render(<StructureErrorPanel diagnostics={[diagnostic('orphanHierarchy', 2, null)]} />);
    const panel = screen.getByTestId('archi-error-panel');
    expect(textOf(panel)).toContain('修好后再看');
    expect(textOf(panel)).toContain('零节点');
    expect(screen.queryByTestId('archi-canvas')).toBeNull();
  });

  it('应在九条诊断与超长 message 下不截断、不重复', () => {
    const kinds: ArchiDiagnostic['kind'][] = [
      'syntaxError',
      'missingField',
      'invalidValue',
      'unknownField',
      'duplicateElement',
      'cycle',
      'dependsOnAncestor',
      'unknownReference',
      'orphanHierarchy',
    ];
    const longMessage = '长'.repeat(1200);
    render(
      <StructureErrorPanel
        diagnostics={kinds.map((kind, index) => diagnostic(kind, index + 1, 1, longMessage))}
      />,
    );
    const rows = screen.getAllByTestId('archi-error-row');
    expect(rows).toHaveLength(9);
    expect(new Set(rows.map((row) => textOf(row as HTMLElement))).size).toBe(9);
    expect(textOf(rows[0] as HTMLElement)).toContain('长'.repeat(1200));
  });
});
