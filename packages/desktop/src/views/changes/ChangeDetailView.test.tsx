import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vite-plus/test';

import type { ArtifactEnvelope, AttemptRecord, ChangeDetail } from '../../types/dto';
import { ChangeDetailView } from './ChangeDetailView';
import type { ChangeDetailState } from './hooks/useChangeDetail';

/** 9 站流水线的固定站名（与 Rust PIPELINE_PHASES 一致）。 */
const PIPELINE = [
  'proposal',
  'dev-design',
  'test-design',
  'implement',
  'test-gen',
  'test-execution',
  'code-review',
  'acceptance',
  'code-analyze',
];

/** 以宽松默认值构造单条 AttemptRecord，便于逐字段控制分支形态。 */
function attempt(overrides: Partial<AttemptRecord> = {}): AttemptRecord {
  return {
    attempt: 1,
    verdict: 'pass',
    report: '评估记录',
    checklist: [],
    skipped: false,
    stale: false,
    startAt: null,
    timestamp: null,
    backtrackTo: null,
    backtrackReason: null,
    ...overrides,
  };
}

/** 仅 proposal 站带给定 attempts 的详情（隔离单站分支，避免多站噪声）。 */
function singleStationDetail(
  attempts: AttemptRecord[],
  overrides: Partial<ChangeDetail> = {},
): ChangeDetail {
  return detail({ pipeline: [{ phase: 'proposal', attempts }], ...overrides });
}

function detail(overrides: Partial<ChangeDetail> = {}): ChangeDetail {
  return {
    name: 'add-feature',
    source: 'active',
    inventory: 'v2',
    created: '2026-09-01',
    unparsable: false,
    pipeline: PIPELINE.map((phase) => ({
      phase,
      attempts:
        phase === 'proposal'
          ? [
              {
                attempt: 1,
                verdict: 'pass',
                report: '提案评估通过',
                checklist: [{ item: '问题清晰', pass: true, evidence: 'L1-10' }],
                skipped: false,
                stale: false,
                startAt: null,
                timestamp: '2026-09-01T02:00:00Z',
                backtrackTo: null,
                backtrackReason: null,
              },
            ]
          : phase === 'dev-design'
            ? [
                {
                  attempt: 2,
                  verdict: 'fail',
                  report: '设计未过',
                  checklist: [],
                  skipped: false,
                  stale: true,
                  startAt: null,
                  timestamp: null,
                  backtrackTo: 'test-design',
                  backtrackReason: '测试设计缺失',
                },
              ]
            : [],
    })),
    activePhase: null,
    interrupted: [],
    fileLog: [],
    artifacts: [],
    ...overrides,
  };
}

function envelope(overrides: Partial<ArtifactEnvelope> = {}): ArtifactEnvelope {
  return {
    kind: 'tasks-progress',
    version: 1,
    title: '任务进度',
    payload: { total: 4, done: 4, pending: 0 },
    fallbackText: null,
    ...overrides,
  };
}

function state(
  overrides: Partial<ChangeDetailState> & {
    detail?: ChangeDetail | null;
    artifacts?: ArtifactEnvelope[];
  },
): ChangeDetailState {
  return {
    detail: null,
    artifacts: [],
    loading: false,
    error: null,
    refresh: () => {},
    ...overrides,
  };
}

describe('ChangeDetailView：9 站流水线、运行标示、降级区块与产物区', () => {
  it('9 站流水线渲染 attempt 序列 / verdict / checklist 展开内容', () => {
    const { container } = render(
      <ChangeDetailView state={state({ detail: detail() })} onBack={() => {}} />,
    );
    for (const phase of PIPELINE) {
      expect(screen.getByText(phase) !== null).toBe(true);
    }
    expect(screen.getByText('提案评估通过') !== null).toBe(true);
    expect(container.textContent).toContain('1 次尝试');
    expect(screen.getAllByText('（无记录）').length).toBe(7);
    // checklist 展开
    expect(screen.getByText('问题清晰') !== null).toBe(true);
    expect(screen.getByText('L1-10') !== null).toBe(true);
    // checklist 条目徽标样式与文案一致
    expect(container.querySelector('.checklist .badge-pass')?.textContent).toBe('pass');
  });

  it('active_phase 存在时渲染运行中标示；backtrack_to / backtrack_reason 随条目展示', () => {
    render(
      <ChangeDetailView
        state={state({
          detail: detail({
            activePhase: { phase: 'implement', attempt: 2, startAt: '2026-09-02T01:00:00Z' },
          }),
        })}
        onBack={() => {}}
      />,
    );
    // 精确断言完整文本（含 startAt 后缀），防止后缀拼接分支被静默跳过
    const active = screen.getByText('运行中 · implement · attempt 2 · 2026-09-02T01:00:00Z');
    expect(active !== null).toBe(true);
    expect(screen.getByText(/回跳至 test-design/) !== null).toBe(true);
    expect(screen.getByText(/测试设计缺失/) !== null).toBe(true);
    expect(screen.getByText('stale') !== null).toBe(true);
  });

  it('v0 change（空流水线 + 纯文档产物清单）以纯文档形态呈现', () => {
    render(
      <ChangeDetailView
        state={state({
          detail: detail({
            inventory: 'v0',
            pipeline: [],
            fileLog: null,
            activePhase: null,
            artifacts: [],
          }),
          artifacts: [
            envelope({
              kind: 'markdown-doc',
              title: '提案',
              payload: { markdown: '# v0 提案文档' },
              fallbackText: '# v0 提案文档',
            }),
          ],
        })}
        onBack={() => {}}
      />,
    );
    expect(screen.getByText(/v0 早期代际：无 workflow.json，仅文档形态/) !== null).toBe(true);
    expect(screen.getByText('v0 提案文档') !== null).toBe(true);
  });

  it('v1 change file_log 区块为 null 时对应区块留空降级，不报错不白屏', () => {
    const { container } = render(
      <ChangeDetailView
        state={state({
          detail: detail({ inventory: 'v1', fileLog: null }),
        })}
        onBack={() => {}}
      />,
    );
    expect(container.textContent.includes('（无 file_log 数据：v1 及更早代际无此字段）')).toBe(
      true,
    );
  });

  it('产物区按信封顺序渲染 ArtifactView 列表', () => {
    const { container } = render(
      <ChangeDetailView
        state={state({
          detail: detail(),
          artifacts: [
            envelope({
              kind: 'markdown-doc',
              title: '提案',
              payload: { markdown: '# 提案正文' },
              fallbackText: '# 提案正文',
            }),
            envelope({
              kind: 'tasks-progress',
              title: '任务进度',
              payload: { total: 4, done: 4, pending: 0 },
            }),
            envelope({
              kind: 'file-log',
              title: '文件清单',
              payload: null,
              fallbackText: 'file-log 保底',
            }),
          ],
        })}
        onBack={() => {}}
      />,
    );
    const cards = container.querySelectorAll('.artifact-card');
    expect(cards).toHaveLength(3);
    expect(cards[0].textContent).toContain('提案');
    expect(cards[1].textContent).toContain('100%');
    expect(cards[2].textContent).toContain('file-log 保底');
  });

  it('detail 为 null / error 态渲染空态与错误提示，不白屏', () => {
    const { container, rerender } = render(
      <ChangeDetailView state={state({})} onBack={() => {}} />,
    );
    expect(container.textContent).toContain('未找到该 change。');
    expect(container.querySelector('.muted') !== null).toBe(true);
    expect(container.querySelector('.error-note')).toBeNull();

    rerender(<ChangeDetailView state={state({ error: 'IPC 断开' })} onBack={() => {}} />);
    expect(container.textContent).toContain('详情加载失败：IPC 断开');
    const note = container.querySelector('.error-note');
    expect(note !== null).toBe(true);
    expect(note?.textContent).toContain('详情加载失败：IPC 断开');
  });

  it('unparsable 详情渲染警示条', () => {
    const { container } = render(
      <ChangeDetailView
        state={state({ detail: detail({ unparsable: true }) })}
        onBack={() => {}}
      />,
    );
    expect(container.textContent).toContain('workflow.json 无法解析');
  });

  it('点击返回列表回调触发', () => {
    const onBack = vi.fn();
    render(<ChangeDetailView state={state({ detail: detail() })} onBack={onBack} />);
    fireEvent.click(screen.getByText('← 返回列表'));
    expect(onBack).toHaveBeenCalled();
  });
});

describe('ChangeDetailView：标记位、backtrack 组合、区块分支与头部元信息', () => {
  it('verdict 徽标按 pass / fail 呈现对应样式与文案', () => {
    const { container } = render(
      <ChangeDetailView state={state({ detail: detail() })} onBack={() => {}} />,
    );
    // 断言范围限定在 attempt-meta 内的 verdict 徽标，避免与 checklist 条目徽标混淆
    expect(container.querySelector('.attempt-meta .badge-pass')?.textContent).toBe('pass');
    expect(container.querySelector('.attempt-meta .badge-fail')?.textContent).toBe('fail');
    const metaBadgeTexts = Array.from(container.querySelectorAll('.attempt-meta .badge')).map(
      (badge) => badge.textContent ?? '',
    );
    expect(metaBadgeTexts).toEqual(expect.arrayContaining(['pass', 'fail']));
  });

  it('attempt 编号缺失渲染占位符 attempt —，有编号渲染实际值', () => {
    const { container } = render(
      <ChangeDetailView
        state={state({
          detail: singleStationDetail([attempt({ attempt: null }), attempt({ attempt: 3 })]),
        })}
        onBack={() => {}}
      />,
    );
    expect(container.textContent).toContain('attempt —');
    expect(container.textContent).toContain('attempt 3');
  });

  it('skipped / stale / start / at 标记仅在对应值存在时渲染', () => {
    const full = render(
      <ChangeDetailView
        state={state({
          detail: singleStationDetail([
            attempt({
              skipped: true,
              stale: true,
              startAt: '2026-09-01T00:00:00Z',
              timestamp: '2026-09-01T02:00:00Z',
            }),
          ]),
        })}
        onBack={() => {}}
      />,
    );
    expect(full.container.textContent).toContain('skipped');
    expect(full.container.textContent).toContain('stale');
    expect(full.container.textContent).toContain('start: 2026-09-01T00:00:00Z');
    expect(full.container.textContent).toContain('at: 2026-09-01T02:00:00Z');

    const bare = render(
      <ChangeDetailView
        state={state({ detail: singleStationDetail([attempt()]) })}
        onBack={() => {}}
      />,
    );
    expect(bare.container.textContent).not.toContain('skipped');
    expect(bare.container.textContent).not.toContain('stale');
    expect(bare.container.textContent).not.toContain('start:');
    expect(bare.container.textContent).not.toContain('at:');
  });

  it('backtrack 块按 to / reason 的四种组合形态渲染', () => {
    const both = render(
      <ChangeDetailView
        state={state({
          detail: singleStationDetail([
            attempt({ backtrackTo: 'test-design', backtrackReason: '设计缺失' }),
          ]),
        })}
        onBack={() => {}}
      />,
    );
    expect(both.container.textContent).toContain('回跳至 test-design');
    expect(both.container.textContent).toContain('：设计缺失');
    expect(both.container.querySelector('.backtrack') !== null).toBe(true);

    const toOnly = render(
      <ChangeDetailView
        state={state({
          detail: singleStationDetail([
            attempt({ backtrackTo: 'proposal', backtrackReason: null }),
          ]),
        })}
        onBack={() => {}}
      />,
    );
    expect(toOnly.container.textContent).toContain('回跳至 proposal');
    expect(toOnly.container.textContent).not.toContain('：null');

    const reasonOnly = render(
      <ChangeDetailView
        state={state({
          detail: singleStationDetail([attempt({ backtrackTo: null, backtrackReason: '仅原因' })]),
        })}
        onBack={() => {}}
      />,
    );
    expect(reasonOnly.container.textContent).toContain('回跳至 ?');
    expect(reasonOnly.container.textContent).toContain('：仅原因');

    const none = render(
      <ChangeDetailView
        state={state({
          detail: singleStationDetail([attempt({ backtrackTo: null, backtrackReason: null })]),
        })}
        onBack={() => {}}
      />,
    );
    expect(none.container.querySelector('.backtrack')).toBeNull();
  });

  it('checklist 为空时不渲染清单列表区块', () => {
    const { container } = render(
      <ChangeDetailView
        state={state({ detail: singleStationDetail([attempt({ checklist: [] })]) })}
        onBack={() => {}}
      />,
    );
    expect(container.querySelector('.checklist')).toBeNull();
  });

  it('checklist 条目渲染 item 名与 evidence 文本', () => {
    const { container } = render(
      <ChangeDetailView
        state={state({
          detail: singleStationDetail([
            attempt({
              checklist: [{ item: '含验收清单', pass: false, evidence: 'proposal 缺 AC 段' }],
            }),
          ]),
        })}
        onBack={() => {}}
      />,
    );
    expect(container.querySelector('.checklist')).not.toBeNull();
    expect(container.textContent).toContain('含验收清单');
    expect(container.textContent).toContain('proposal 缺 AC 段');
    expect(container.querySelector('.checklist .badge-fail')?.textContent).toBe('fail');
  });

  it('中断留档区块渲染条目，空缺时间显示占位符', () => {
    const { container } = render(
      <ChangeDetailView
        state={state({
          detail: detail({
            interrupted: [
              { phase: 'implement', attempt: 2, startAt: '2026-09-02T00:00:00Z', endAt: null },
            ],
          }),
        })}
        onBack={() => {}}
      />,
    );
    expect(screen.getByText('中断留档') !== null).toBe(true);
    expect(container.textContent).toContain('implement');
    expect(container.textContent).toContain('attempt 2');
    expect(container.textContent).toContain('start: 2026-09-02T00:00:00Z');
    expect(container.textContent).toContain('end: —');
  });

  it('interrupted 为空数组时不渲染中断留档区块', () => {
    render(<ChangeDetailView state={state({ detail: detail() })} onBack={() => {}} />);
    expect(screen.queryByText('中断留档')).toBeNull();
  });

  it('file_log 为空数组时渲染（空）占位而非留空降级', () => {
    const { container } = render(
      <ChangeDetailView state={state({ detail: detail() })} onBack={() => {}} />,
    );
    expect(container.textContent).toContain('（空）');
    expect(container.querySelector('.filelog-table')).toBeNull();
  });

  it('file_log 条目以表格逐行渲染，attempt 与时间空缺显示占位符', () => {
    const { container } = render(
      <ChangeDetailView
        state={state({
          detail: detail({
            fileLog: [
              {
                op: 'write',
                scope: 'workflow',
                attempt: 3,
                path: 'src/a.json',
                at: '2026-09-03T00:00:00Z',
              },
              { op: 'delete', scope: 'files', attempt: null, path: 'src/b.ts', at: null },
            ],
          }),
        })}
        onBack={() => {}}
      />,
    );
    const rows = Array.from(container.querySelectorAll('tbody tr'));
    expect(rows).toHaveLength(2);
    const cells = rows.map((row) =>
      Array.from(row.querySelectorAll('td')).map((td) => td.textContent ?? ''),
    );
    expect(cells[0]).toEqual(['write', 'workflow', '3', 'src/a.json', '2026-09-03T00:00:00Z']);
    expect(cells[1]).toEqual(['delete', 'files', '—', 'src/b.ts', '—']);
  });

  it('头部按 source / created / inventory 渲染元信息', () => {
    const { container } = render(
      <ChangeDetailView
        state={state({ detail: singleStationDetail([attempt()]) })}
        onBack={() => {}}
      />,
    );
    expect(screen.getByText('进行中') !== null).toBe(true);
    expect(container.textContent).not.toContain('已归档');
    expect(container.textContent).toContain('2026-09-01');
    expect(container.querySelector('.badge-inv2')?.textContent).toBe('v2');

    const archived = render(
      <ChangeDetailView
        state={state({
          detail: singleStationDetail([attempt()], { source: 'archive', created: null }),
        })}
        onBack={() => {}}
      />,
    );
    expect(archived.container.textContent).toContain('已归档');
    expect(archived.container.textContent).not.toContain('进行中');
    expect(archived.container.textContent).not.toContain('2026-09-01');
    // created 为 null 时不得出现空的 muted 占位节点（如无条件渲染 created span）
    const mutedTexts = Array.from(archived.container.querySelectorAll('.muted')).map(
      (el) => el.textContent ?? '',
    );
    expect(mutedTexts.length).toBeGreaterThan(0);
    for (const text of mutedTexts) {
      expect(text.length).toBeGreaterThan(0);
    }
  });

  it('active_phase 的 startAt 为空时不拼接时间与占位符', () => {
    const { container } = render(
      <ChangeDetailView
        state={state({
          detail: singleStationDetail([attempt()], {
            activePhase: { phase: 'implement', attempt: 2, startAt: null },
          }),
        })}
        onBack={() => {}}
      />,
    );
    expect(screen.getByText('运行中 · implement · attempt 2') !== null).toBe(true);
    expect(container.textContent).not.toContain(' · —');
  });

  it('v2 空流水线渲染（无评估记录）而非 v0 纯文档文案', () => {
    const { container } = render(
      <ChangeDetailView state={state({ detail: detail({ pipeline: [] }) })} onBack={() => {}} />,
    );
    expect(container.textContent).toContain('（无评估记录）');
    expect(container.textContent).not.toContain('v0 早期代际');
  });

  it('产物清单为空时渲染（未发现可读产物）占位', () => {
    const { container } = render(
      <ChangeDetailView state={state({ detail: detail() })} onBack={() => {}} />,
    );
    expect(container.textContent).toContain('（未发现可读产物）');
    expect(container.querySelector('.artifact-card')).toBeNull();
  });

  it('loading 态与未找到态共用降级页且采用 muted 样式', () => {
    const { container, rerender } = render(
      <ChangeDetailView state={state({ loading: true })} onBack={() => {}} />,
    );
    expect(container.textContent).toContain('加载中…');
    expect(container.querySelector('.muted') !== null).toBe(true);
    expect(container.querySelector('.error-note')).toBeNull();

    rerender(<ChangeDetailView state={state({})} onBack={() => {}} />);
    expect(container.textContent).toContain('未找到该 change。');
    expect(container.querySelector('.muted') !== null).toBe(true);
    expect(container.querySelector('.error-note')).toBeNull();
  });

  it('loading 中已有详情时不回落到加载占位，刷新按钮禁用', () => {
    const { container } = render(
      <ChangeDetailView state={state({ loading: true, detail: detail() })} onBack={() => {}} />,
    );
    expect(container.textContent).not.toContain('加载中…');
    expect(container.textContent).toContain('add-feature');
    const refreshButton = Array.from(container.querySelectorAll('button')).find(
      (button) => button.textContent === '刷新详情',
    );
    expect(refreshButton?.disabled).toBe(true);
  });

  it('unparsable 为 false 时不渲染警示条', () => {
    const { container } = render(
      <ChangeDetailView state={state({ detail: detail() })} onBack={() => {}} />,
    );
    expect(container.textContent).not.toContain('workflow.json 无法解析');
  });
});
