import { describe, expect, it } from 'vite-plus/test';

import type {
  ArchiDriftObservation,
  ArchiDriftReport,
  ArchiDriftSuppressedEntry,
  ArchiGraph,
} from '../../../types/generated/bindings';
import { driftMarks } from './drift-marks';

/**
 * `driftMarks` 单测（AC-19 / D20 / D21）：只读 `observations` / `suppressed` 两段——
 * `unfulfilledDeclaration` → 边 `unfulfilled`、`suppressed` → 边 `exempt`（reason）、
 * 其余保持 `none`；只产边标记（节点标记不呈现，元素 `mark` 原样透传）；畸形报告不抛错。
 *
 * 零 mock：生成绑定的报告 / 图类型字面量直构，纯函数直调。
 */

/** 两元素一关系的健康图（标记全 `none`）。 */
function graph(): ArchiGraph {
  return {
    elements: [
      {
        fqn: 'App.Auth',
        kind: 'domain',
        gate: 'strict',
        path: ['./src/auth/'],
        summary: '认证域',
        metadata: {},
        modelFile: '.dev-team/architecture/models/03-auth.toml',
        modelLine: 3,
        mark: 'none',
      },
      {
        fqn: 'App.Billing',
        kind: 'domain',
        gate: 'warn',
        path: ['./src/billing/'],
        summary: '计费域',
        metadata: {},
        modelFile: '.dev-team/architecture/models/03-auth.toml',
        modelLine: 9,
        mark: 'none',
      },
    ],
    relations: [
      {
        from: 'App.Auth',
        to: 'App.Billing',
        description: '认证读取计费配置',
        mark: 'none',
        reason: null,
      },
    ],
    diagnostics: [],
  };
}

/** 单条观察项。 */
function observation(from: string, to: string): ArchiDriftObservation {
  return {
    kind: 'unfulfilledDeclaration',
    from,
    to,
    message: `本次扫描面内未见 ${from} → ${to} 的 import`,
  };
}

/** 单条豁免留痕。 */
function suppressed(from: string, to: string, reason: string): ArchiDriftSuppressedEntry {
  return {
    from,
    to,
    via: 'inline',
    reason,
    file: 'src/auth/x.ts',
    line: 4,
  };
}

/** 报告（只填两段；其余字段取稳定默认值）。 */
function report(
  overrides: Partial<
    Pick<ArchiDriftReport, 'observations' | 'suppressed' | 'errors' | 'status'>
  > = {},
): ArchiDriftReport {
  return {
    version: 1,
    timestamp: '2026-10-20T08:31:12Z',
    mode: 'full',
    root: '/ws',
    change: null,
    base: null,
    modelHash: 'sha256:00ff',
    model: {
      elements: 2,
      relations: 1,
      byGate: { strict: 1, warn: 1, off: 0 },
      structuralErrors: [],
      warnings: [],
    },
    scan: {
      mode: 'full',
      changedFiles: 0,
      scannedFiles: 2,
      excluded: [],
      unmatchedFiles: [],
      matched: [],
      unresolved: [],
    },
    errors: [],
    warnings: [],
    suppressed: [],
    observations: [],
    status: 'clean',
    ...overrides,
  };
}

/** 边标记（按 `(from, to)` 查）。 */
function edgeMark(result: ArchiGraph, from: string, to: string) {
  return result.relations.find((relation) => relation.from === from && relation.to === to);
}

describe('driftMarks 空报告面', () => {
  it('应在报告为 null 时原样返回入参图', () => {
    const input = graph();
    const result = driftMarks(null, input);
    expect(result).toEqual(input);
    expect(result.relations.every((relation) => relation.mark === 'none')).toBe(true);
    expect(result.elements.every((element) => element.mark === 'none')).toBe(true);
  });

  it('应在 clean / skipped / noChanges 三态报告下保持全 none', () => {
    for (const status of ['clean', 'skipped', 'noChanges'] as const) {
      const result = driftMarks(report({ status }), graph());
      expect(result.relations[0]?.mark, `${status} 态边零标记`).toBe('none');
      expect(result.elements.every((element) => element.mark === 'none')).toBe(true);
    }
  });

  it('应只在两段非空时标记——errors / warnings 非空但两段空 => 全 none', () => {
    const result = driftMarks(
      report({
        status: 'errors',
        errors: [
          {
            kind: 'undeclaredDependency',
            from: 'App.Auth',
            to: 'App.Billing',
            gate: 'strict',
            message: '未声明依赖',
            occurrences: [],
          },
        ],
      }),
      graph(),
    );
    expect(result.relations[0]?.mark).toBe('none');
    expect(result.elements.every((element) => element.mark === 'none')).toBe(true);
  });
});

describe('driftMarks 未兑现声明', () => {
  it('应只把未兑现声明的边标为 unfulfilled（节点标记不派生）', () => {
    const result = driftMarks(
      report({ observations: [observation('App.Auth', 'App.Billing')] }),
      graph(),
    );
    expect(edgeMark(result, 'App.Auth', 'App.Billing')?.mark).toBe('unfulfilled');
    expect(result.elements.every((element) => element.mark === 'none')).toBe(true);
  });

  it('应在同一 from 对 3 个 to 时标 3 条边且元素标记不变', () => {
    const input = graph();
    input.elements.push(
      {
        fqn: 'App.Search',
        kind: 'domain',
        gate: 'off',
        path: ['./src/search/'],
        summary: null,
        metadata: {},
        modelFile: '.dev-team/architecture/models/03-auth.toml',
        modelLine: 15,
        mark: 'none',
      },
      {
        fqn: 'App.Docs',
        kind: 'domain',
        gate: 'off',
        path: ['./src/docs/'],
        summary: null,
        metadata: {},
        modelFile: '.dev-team/architecture/models/03-auth.toml',
        modelLine: 21,
        mark: 'none',
      },
    );
    input.relations.push(
      {
        from: 'App.Auth',
        to: 'App.Search',
        description: null,
        mark: 'none',
        reason: null,
      },
      {
        from: 'App.Auth',
        to: 'App.Docs',
        description: null,
        mark: 'none',
        reason: null,
      },
    );
    const result = driftMarks(
      report({
        observations: [
          observation('App.Auth', 'App.Billing'),
          observation('App.Auth', 'App.Search'),
          observation('App.Auth', 'App.Docs'),
        ],
      }),
      input,
    );
    const marked = result.relations.filter((relation) => relation.mark === 'unfulfilled');
    expect(marked).toHaveLength(3);
    expect(result.elements.every((element) => element.mark === 'none')).toBe(true);
  });
});

describe('driftMarks 豁免留痕', () => {
  it.each(['inline', 'config'] as const)('应在 via = %s 时把边标为 exempt 并带 reason', (via) => {
    const entry: ArchiDriftSuppressedEntry = { ...suppressed('App.Auth', 'App.Billing', '迁移期') };
    entry.via = via;
    if (via === 'config') {
      entry.file = null;
      entry.line = null;
    }
    const result = driftMarks(report({ suppressed: [entry] }), graph());
    const edge = edgeMark(result, 'App.Auth', 'App.Billing');
    expect(edge?.mark).toBe('exempt');
    expect(edge?.reason).toBe('迁移期');
    expect(result.elements.every((element) => element.mark === 'none')).toBe(true);
  });

  it('应在 reason 为空串时如实透传空串（不擅自补文案）', () => {
    const result = driftMarks(
      report({ suppressed: [suppressed('App.Auth', 'App.Billing', '')] }),
      graph(),
    );
    const edge = edgeMark(result, 'App.Auth', 'App.Billing');
    expect(edge?.mark).toBe('exempt');
    expect(edge?.reason).toBe('');
  });

  it('应在同一元素对同时命中两段时取 unfulfilled', () => {
    const result = driftMarks(
      report({
        observations: [observation('App.Auth', 'App.Billing')],
        suppressed: [suppressed('App.Auth', 'App.Billing', '迁移期')],
      }),
      graph(),
    );
    const edge = edgeMark(result, 'App.Auth', 'App.Billing');
    expect(edge?.mark).toBe('unfulfilled');
    expect(edge?.reason).toBeNull();
  });
});

describe('driftMarks 未命中与既有标记', () => {
  it('应在报告条目指向不存在的边 / 元素时不新增节点也不抛错', () => {
    const input = graph();
    const result = driftMarks(
      report({
        observations: [observation('App.Ghost', 'App.Nowhere')],
        suppressed: [suppressed('App.Nobody', 'App.Nothing', 'x')],
      }),
      input,
    );
    expect(result.elements).toHaveLength(input.elements.length);
    expect(result.relations).toHaveLength(input.relations.length);
    expect(result.relations.every((relation) => relation.mark === 'none')).toBe(true);
    expect(result.elements.every((element) => element.mark === 'none')).toBe(true);
  });

  it('应保持既有 crossKind 标记不被覆写', () => {
    const input = graph();
    input.relations[0] = { ...input.relations[0]!, mark: 'crossKind' };
    const result = driftMarks(report({ observations: [] }), input);
    expect(result.relations[0]?.mark).toBe('crossKind');
  });

  it('应在空图 + 非空报告时返回空图', () => {
    const empty: ArchiGraph = { elements: [], relations: [], diagnostics: [] };
    const result = driftMarks(
      report({ observations: [observation('App.Auth', 'App.Billing')] }),
      empty,
    );
    expect(result.elements).toEqual([]);
    expect(result.relations).toEqual([]);
  });
});

describe('driftMarks 纯函数性', () => {
  it('应不改写入参且两次调用结果深相等', () => {
    const input = graph();
    const snapshot = structuredClone(input);
    const first = driftMarks(
      report({ observations: [observation('App.Auth', 'App.Billing')] }),
      input,
    );
    const second = driftMarks(
      report({ observations: [observation('App.Auth', 'App.Billing')] }),
      input,
    );
    expect(input).toEqual(snapshot);
    expect(first).toEqual(second);
    expect(first).not.toBe(input);
  });

  it('应在 1000 条观察 + 1000 条豁免 + 1000 条边时正确标记且不爆时', () => {
    const big: ArchiGraph = { elements: [], relations: [], diagnostics: [] };
    for (let index = 0; index < 1000; index += 1) {
      big.elements.push({
        fqn: `App.E${index}`,
        kind: 'module',
        gate: 'off',
        path: [`./src/e${index}/`],
        summary: null,
        metadata: {},
        modelFile: '.dev-team/architecture/models/09-big.toml',
        modelLine: index + 1,
        mark: 'none',
      });
      big.relations.push({
        from: `App.E${index}`,
        to: `App.E${(index + 1) % 1000}`,
        description: null,
        mark: 'none',
        reason: null,
      });
    }
    const observations: ArchiDriftObservation[] = [];
    const suppressedEntries: ArchiDriftSuppressedEntry[] = [];
    for (let index = 0; index < 1000; index += 1) {
      observations.push(observation(`App.E${index}`, `App.E${(index + 1) % 1000}`));
      suppressedEntries.push(suppressed(`App.E${index}`, `App.E${(index + 2) % 1000}`, 'r'));
    }
    const result = driftMarks(report({ observations, suppressed: suppressedEntries }), big);
    // 每个 (from, to) 都命中观察项 ⇒ 全部 unfulfilled（观察项优先）
    expect(result.relations.every((relation) => relation.mark === 'unfulfilled')).toBe(true);
    expect(result.elements.every((element) => element.mark === 'none')).toBe(true);
  });
});

describe('driftMarks 畸形报告降级', () => {
  it('应忽略越集 kind 与空串 FQN 等畸形条目而不抛错', () => {
    const malformed = {
      ...report(),
      observations: [
        { kind: 'unusedDeclaration', from: 'App.Auth', to: 'App.Billing', message: 'x' },
        { kind: 'someFutureKind', from: 'App.Auth', to: 'App.Billing', message: 'x' },
        { kind: 'unfulfilledDeclaration', from: '', to: '', message: 'x' },
      ],
      suppressed: [{ from: '', to: '', via: 'config', reason: '', file: null, line: null }],
    } as unknown as ArchiDriftReport;
    const result = driftMarks(malformed, graph());
    expect(result.relations[0]?.mark).toBe('none');
    expect(result.elements.every((element) => element.mark === 'none')).toBe(true);
  });

  it('应在 observations 为空数组而 suppressed 为畸形值时降级为零标记', () => {
    const malformed = {
      ...report(),
      observations: [],
      suppressed: 'not-an-array',
    } as unknown as ArchiDriftReport;
    expect(() => driftMarks(malformed, graph())).not.toThrow();
    const result = driftMarks(malformed, graph());
    expect(result.relations[0]?.mark).toBe('none');
  });
});
