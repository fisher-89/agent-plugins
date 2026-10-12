import { describe, expect, it } from 'vite-plus/test';

import type {
  ArtifactDescriptor,
  ArtifactEnvelope,
  AttemptRecord,
  ChangeDetail,
} from '../../../types/dto';
import { mountMaterials } from './attachments';
import { PIPELINE_PHASES } from './layout';

// ---------------------------------------------------------------------------
// mountMaterials 单测：挂载层为纯函数（无 Mock）。文档归属 = 后端清单出线
//（`detail.artifacts[].phase`）——前端字符串 → 列静态映射表（docColumn）已退役，
// 归属规则全部经 mountMaterials 公共输出覆盖测试（AC-7）；eval-checklist 按
// payload.phase 归属挂 phase 列头（不接收 graph，挂载与节点无关）。
// ---------------------------------------------------------------------------

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
    executorSessionId: null,
    evaluatorSessionId: null,
    decisionSessionId: null,
    ...overrides,
  };
}

function envelope(kind: string, title: string, payload: unknown): ArtifactEnvelope {
  return { kind, version: 1, title, payload, fallbackText: null };
}

/** 清单归属相位（后端 phase 定义清单口径，与 phase_table artifactManifest 一致）；
 * 清单外路径零候选 → 空归属（不臆测列）。 */
function phaseOf(source: string): string {
  if (source === 'proposal.md' || source.startsWith('specs/')) return 'proposal';
  if (source === 'design.md' || source === 'tasks.md') return 'dev-design';
  if (source.startsWith('test-reports/')) return 'test-execution';
  return '';
}

function descriptor(kind: string, source: string, title: string): ArtifactDescriptor {
  return { kind, source, title, phase: phaseOf(source) };
}

/** 建档详情底座：dev-design 站 1 条 eval（多挂载定位用例共用），其余站空。 */
function detail(overrides: Partial<ChangeDetail> = {}): ChangeDetail {
  return {
    id: 'add-feature',
    name: 'add-feature',
    title: 'add-feature',
    source: 'active',
    status: 'active',
    created: null,
    pipeline: PIPELINE_PHASES.map((phase) => ({
      phase,
      attempts: phase === 'dev-design' ? [attempt({ attempt: 2 })] : [],
    })),
    activePhase: null,
    runs: [],
    worktree: null,
    artifacts: [],
    ...overrides,
  };
}

/** 信封按 detail.artifacts 顺序 1:1 生成（对齐 useChangeDetail 的配对方式）。 */
function envelopesFor(artifacts: ArtifactDescriptor[]): ArtifactEnvelope[] {
  const payloads: Record<string, unknown> = {
    'markdown-doc': { markdown: `# ${'文档'}` },
    'tasks-progress': { total: 1, done: 1, pending: 0 },
    'eval-checklist': null,
  };
  return artifacts.map((item) => envelope(item.kind, item.title, payloads[item.kind] ?? null));
}

describe('mountMaterials：文档挂列（清单归属 detail.artifacts[].phase）', () => {
  it('清单归属驱动：proposal.md 与 specs/ 深路径 → proposal 列；design.md / tasks.md → dev-design 列；test-reports/ 前缀 → test-execution 列', () => {
    const artifacts = [
      descriptor('markdown-doc', 'proposal.md', '提案'),
      descriptor('markdown-doc', 'specs/x/spec.md', '规格'),
      descriptor('markdown-doc', 'design.md', '设计'),
      descriptor('tasks-progress', 'tasks.md', '任务进度'),
      descriptor('markdown-doc', 'test-reports/run-1.md', '测试报告'),
    ];
    const base = detail({ artifacts });
    const materials = mountMaterials(base, envelopesFor(artifacts));
    expect(materials.columnDocs['col:proposal']).toHaveLength(2);
    expect(materials.columnDocs['col:dev-design']).toHaveLength(2);
    expect(materials.columnDocs['col:test-execution']).toHaveLength(1);
    // markdown-doc 与 tasks-progress 两 kind 均走映射
    const devDesignKinds = materials.columnDocs['col:dev-design'].map((item) => item.kind);
    expect(devDesignKinds).toEqual(['markdown-doc', 'tasks-progress']);
  });

  it('空归属 / 未知 phase / pipeline 外相位 → 跳过该 artifact（稳健面，不误挂错站）', () => {
    const artifacts = [
      descriptor('markdown-doc', 'explore.md', '探索'),
      descriptor('markdown-doc', 'phases/proposal/attempt-1.md', '旧代际'),
      descriptor('markdown-doc', 'random/unknown.txt', '未知'),
      { ...descriptor('markdown-doc', 'design.md', '设计'), phase: 'ghost-phase' },
      { ...descriptor('markdown-doc', 'design.md', '设计'), phase: '' },
    ];
    const base = detail({ artifacts });
    const materials = mountMaterials(base, envelopesFor(artifacts));
    expect(materials.columnDocs).toEqual({});
  });

  it('缺 phase 字段（undefined）→ 不 panic、跳过该 artifact', () => {
    const broken = {
      kind: 'markdown-doc',
      source: 'design.md',
      title: '设计',
    } as unknown as ArtifactDescriptor;
    const artifacts = [broken, descriptor('markdown-doc', 'proposal.md', '提案')];
    const base = detail({ artifacts });
    const materials = mountMaterials(base, envelopesFor(artifacts));
    expect(materials.columnDocs['col:dev-design']).toBeUndefined();
    expect(materials.columnDocs['col:proposal']).toHaveLength(1);
  });

  it('每站一份不随 attempt 重复：同源文档单信封 → 该列 columnDocs 恰 1 条', () => {
    const artifacts = [descriptor('markdown-doc', 'design.md', '设计')];
    const multiAttempt = detail({
      artifacts,
      pipeline: PIPELINE_PHASES.map((phase) => ({
        phase,
        attempts:
          phase === 'dev-design'
            ? [attempt({ attempt: 1 }), attempt({ attempt: 2 }), attempt({ attempt: 3 })]
            : [],
      })),
    });
    const materials = mountMaterials(multiAttempt, envelopesFor(artifacts));
    expect(materials.columnDocs['col:dev-design']).toHaveLength(1);
  });
});

describe('mountMaterials：记录挂列头（eval-checklist）', () => {
  it('eval-checklist 信封 payload { phase, attempt } → 按 payload.phase 归属挂对应 phase 列头', () => {
    const base = detail();
    const checklist = envelope('eval-checklist', '评估清单', {
      phase: 'dev-design',
      attempt: 2,
      verdict: 'pass',
      items: [],
    });
    const materials = mountMaterials(base, [checklist]);
    expect(materials.columnDocs['col:dev-design']).toEqual([checklist]);
  });

  it('eval-checklist payload 形状不合规（null / 缺字段 / 类型不符）→ 不挂载任何列', () => {
    const base = detail();
    const broken = [
      envelope('eval-checklist', 'payload null', null),
      envelope('eval-checklist', '缺 phase', { attempt: 2 }),
      envelope('eval-checklist', '缺 attempt', { phase: 'dev-design' }),
      envelope('eval-checklist', '类型不符', { phase: 3, attempt: '2' }),
    ];
    const materials = mountMaterials(base, broken);
    expect(materials.columnDocs).toEqual({});
  });
});

describe('mountMaterials：聚合与空态', () => {
  it('同列多文档、同列多素材 → 数组聚合保序不覆盖', () => {
    const artifacts = [
      descriptor('markdown-doc', 'proposal.md', '提案'),
      descriptor('markdown-doc', 'specs/x/spec.md', '规格'),
    ];
    const base = detail({ artifacts });
    const first = envelope('eval-checklist', '清单一', {
      phase: 'dev-design',
      attempt: 2,
      verdict: null,
      items: [],
    });
    const second = envelope('eval-checklist', '清单二', {
      phase: 'dev-design',
      attempt: 2,
      verdict: null,
      items: [],
    });
    const materials = mountMaterials(base, [...envelopesFor(artifacts), first, second]);
    expect(materials.columnDocs['col:proposal'].map((item) => item.title)).toEqual([
      '提案',
      '规格',
    ]);
    expect(materials.columnDocs['col:dev-design']).toEqual([first, second]);
  });

  it('全空入参（envelopes=[]）→ FlowMaterials 单键齐全且全空，不抛错', () => {
    const base = detail();
    expect(mountMaterials(base, [])).toEqual({ columnDocs: {} });
  });
});

// ---------------------------------------------------------------------------
// 挂载单层（change-flow-show-step-only，AC-4 / D6）：eval-checklist 挂 phase 列头
// （payload.phase 归属），不再挂节点——NODE_PRECEDENCE / nodeChecklists 退役，
// 挂载与 eval / active 节点在场与否无关
// ---------------------------------------------------------------------------

describe('mountMaterials：NODE_PRECEDENCE 退役（checklist 恒挂列头）', () => {
  it('节点全缺席（pipeline 各站零 attempt + activePhase 在场）→ checklist 仍挂 payload.phase 列头', () => {
    const base = detail({
      pipeline: PIPELINE_PHASES.map((phase) => ({ phase, attempts: [] })),
      activePhase: { phase: 'dev-design', attempt: 3, startAt: '2026-09-05T00:00:00Z' },
    });
    const checklist = envelope('eval-checklist', '评估清单', {
      phase: 'dev-design',
      attempt: 3,
      verdict: 'pass',
      items: [],
    });
    const materials = mountMaterials(base, [checklist]);
    expect(Object.keys(materials.columnDocs)).toEqual(['col:dev-design']);
    expect(materials.columnDocs['col:dev-design']).toEqual([checklist]);
  });

  it('payload.phase 为 pipeline 外相位 → 不误挂任何列（归属校验兜底）', () => {
    const base = detail();
    const checklist = envelope('eval-checklist', '评估清单', {
      phase: 'ghost-phase',
      attempt: 1,
      verdict: 'pass',
      items: [],
    });
    const materials = mountMaterials(base, [checklist]);
    expect(materials.columnDocs).toEqual({});
  });
});

// ---------------------------------------------------------------------------
// file_log 挂载分支退役（desktop-workflow-db-state）：file_log 挂节点分支与
// scope='workflow' 图外素材分支删除，`nodeFiles` / `outsideFiles` 状态字段
// 退役——FlowMaterials / ChangeDetail 类型面已无对应字段（编译期锚定），
// 运行时产物键集经本节锚定。
// ---------------------------------------------------------------------------

describe('mountMaterials：file_log 挂载分支退役', () => {
  it('detail 夹具无 fileLog 字段 → 产物零文件挂载节点、零图外素材；产物状态面无 nodeFiles / outsideFiles 键', () => {
    // 历史上会产文件挂载的多 attempt 形态（fileLog 已不在 ChangeDetail 类型面，
    // 夹具带该字段即编译失败——编译期锚定半边）
    const base = detail({
      pipeline: PIPELINE_PHASES.map((phase) => ({
        phase,
        attempts: phase === 'dev-design' ? [attempt({ attempt: 1 }), attempt({ attempt: 2 })] : [],
      })),
    });
    const envelopes = [
      envelope('eval-checklist', '评估清单', {
        phase: 'dev-design',
        attempt: 2,
        verdict: 'pass',
        items: [],
      }),
      // 历史 file-log 信封 kind：分支删除后不再落入任何挂载产物（直接跳过）
      envelope('file-log', '文件清单', null),
    ];

    const materials = mountMaterials(base, envelopes);

    // 运行时锚定半边：产物状态面恰单键，退役键零驻留
    expect(Object.keys(materials)).toEqual(['columnDocs']);
    expect('nodeFiles' in materials).toBe(false);
    expect('outsideFiles' in materials).toBe(false);
    expect('nodeChecklists' in materials).toBe(false);
    // 零文件挂载节点、零图外素材：仅 eval-checklist 挂 dev-design 列头
    expect(Object.keys(materials.columnDocs)).toEqual(['col:dev-design']);
    expect(materials.columnDocs['col:dev-design']).toEqual([envelopes[0]]);
  });
});
