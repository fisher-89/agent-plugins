import { describe, expect, it } from 'vite-plus/test';

import type {
  ArtifactDescriptor,
  ArtifactEnvelope,
  AttemptRecord,
  ChangeDetail,
} from '../../../types/dto';
import { mountMaterials } from './attachments';
import { buildFlowGraph } from './graph';
import { PIPELINE_PHASES } from './layout';

// ---------------------------------------------------------------------------
// mountMaterials 单测：挂载层为纯函数（无 Mock）；文档 → 列映射表 docColumn 为
// 模块私有，其归属规则全部经 mountMaterials 公共输出覆盖测试（AC-4 / AC-8）。
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

function descriptor(kind: string, source: string, title: string): ArtifactDescriptor {
  return { kind, source, title };
}

/** 建档详情底座：dev-design 站 1 条 eval（多挂载定位用例共用），其余站空。 */
function detail(overrides: Partial<ChangeDetail> = {}): ChangeDetail {
  return {
    name: 'add-feature',
    source: 'active',
    status: 'active',
    created: null,
    pipeline: PIPELINE_PHASES.map((phase) => ({
      phase,
      attempts: phase === 'dev-design' ? [attempt({ attempt: 2 })] : [],
    })),
    activePhase: null,
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

describe('mountMaterials：文档挂列（静态映射 docColumn）', () => {
  it('静态映射全命中：proposal.md 与 specs/ 深路径 → proposal 列；design.md / tasks.md → dev-design 列；test-reports/ 前缀 → test-execution 列', () => {
    const artifacts = [
      descriptor('markdown-doc', 'proposal.md', '提案'),
      descriptor('markdown-doc', 'specs/x/spec.md', '规格'),
      descriptor('markdown-doc', 'design.md', '设计'),
      descriptor('tasks-progress', 'tasks.md', '任务进度'),
      descriptor('markdown-doc', 'test-reports/run-1.md', '测试报告'),
    ];
    const base = detail({ artifacts });
    const materials = mountMaterials(buildFlowGraph(base), base, envelopesFor(artifacts));
    expect(materials.columnDocs['col:proposal']).toHaveLength(2);
    expect(materials.columnDocs['col:dev-design']).toHaveLength(2);
    expect(materials.columnDocs['col:test-execution']).toHaveLength(1);
    // markdown-doc 与 tasks-progress 两 kind 均走映射
    const devDesignKinds = materials.columnDocs['col:dev-design'].map((item) => item.kind);
    expect(devDesignKinds).toEqual(['markdown-doc', 'tasks-progress']);
  });

  it('映射表外文档（explore.md、旧代际 phases/**、未知路径）不入任何 columnDocs', () => {
    const artifacts = [
      descriptor('markdown-doc', 'explore.md', '探索'),
      descriptor('markdown-doc', 'phases/proposal/attempt-1.md', '旧代际'),
      descriptor('markdown-doc', 'random/unknown.txt', '未知'),
    ];
    const base = detail({ artifacts });
    const materials = mountMaterials(buildFlowGraph(base), base, envelopesFor(artifacts));
    expect(materials.columnDocs).toEqual({});
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
    const materials = mountMaterials(
      buildFlowGraph(multiAttempt),
      multiAttempt,
      envelopesFor(artifacts),
    );
    expect(materials.columnDocs['col:dev-design']).toHaveLength(1);
  });
});

describe('mountMaterials：记录挂节点（eval-checklist）', () => {
  it('eval-checklist 信封 payload { phase, attempt } → 按节点 id 方案定位 eval 节点挂载', () => {
    const base = detail();
    const checklist = envelope('eval-checklist', '评估清单', {
      phase: 'dev-design',
      attempt: 2,
      verdict: 'pass',
      items: [],
    });
    const materials = mountMaterials(buildFlowGraph(base), base, [checklist]);
    expect(materials.nodeChecklists['eval:dev-design:2']).toEqual([checklist]);
  });

  it('eval-checklist payload 形状不合规（null / 缺字段 / 类型不符）→ 不挂载任何节点', () => {
    const base = detail();
    const graph = buildFlowGraph(base);
    const broken = [
      envelope('eval-checklist', 'payload null', null),
      envelope('eval-checklist', '缺 phase', { attempt: 2 }),
      envelope('eval-checklist', '缺 attempt', { phase: 'dev-design' }),
      envelope('eval-checklist', '类型不符', { phase: 3, attempt: '2' }),
    ];
    const materials = mountMaterials(graph, base, broken);
    expect(materials.nodeChecklists).toEqual({});
  });
});

describe('mountMaterials：聚合与空态', () => {
  it('同列多文档、同节点多素材 → 数组聚合保序不覆盖', () => {
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
    const materials = mountMaterials(buildFlowGraph(base), base, [
      ...envelopesFor(artifacts),
      first,
      second,
    ]);
    expect(materials.columnDocs['col:proposal'].map((item) => item.title)).toEqual([
      '提案',
      '规格',
    ]);
    expect(materials.nodeChecklists['eval:dev-design:2']).toEqual([first, second]);
  });

  it('全空入参（envelopes=[]）→ FlowMaterials 两键齐全且全空，不抛错', () => {
    const base = detail();
    expect(mountMaterials(buildFlowGraph(base), base, [])).toEqual({
      columnDocs: {},
      nodeChecklists: {},
    });
  });
});

// ---------------------------------------------------------------------------
// NODE_PRECEDENCE 两段收敛（desktop-drawer-session-column，AC-5）：checklist
// 定位降级链 eval → active 两段（interrupted 段无构造输入）；挂载规则本体
// （文档挂列 / checklist 挂节点）零改动
// ---------------------------------------------------------------------------

describe('mountMaterials：NODE_PRECEDENCE 两段收敛（eval → active）', () => {
  it('仅 active 在场（eval 缺席）→ checklist 挂 active:<phase>:<attempt> 节点（降级链末段语义保留）', () => {
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
    const materials = mountMaterials(buildFlowGraph(base), base, [checklist]);
    expect(Object.keys(materials.nodeChecklists)).toEqual(['active:dev-design:3']);
    expect(materials.nodeChecklists['active:dev-design:3']).toEqual([checklist]);
  });

  it('eval 与 active 并存 → checklist 恒挂 eval 节点（优先级首位断言；不重复挂 active）', () => {
    const base = detail({
      activePhase: { phase: 'dev-design', attempt: 2, startAt: '2026-09-05T00:00:00Z' },
    });
    const checklist = envelope('eval-checklist', '评估清单', {
      phase: 'dev-design',
      attempt: 2,
      verdict: null,
      items: [],
    });
    const materials = mountMaterials(buildFlowGraph(base), base, [checklist]);
    expect(Object.keys(materials.nodeChecklists)).toEqual(['eval:dev-design:2']);
    expect(materials.nodeChecklists['active:dev-design:2']).toBeUndefined();
  });

  it('同号 eval / active 均缺席（仅他号节点在场）→ checklist 不误挂他号节点（miss 兜底）', () => {
    const base = detail();
    const checklist = envelope('eval-checklist', '评估清单', {
      phase: 'implement',
      attempt: 1,
      verdict: 'pass',
      items: [],
    });
    const materials = mountMaterials(buildFlowGraph(base), base, [checklist]);
    expect(materials.nodeChecklists).toEqual({});
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

    const materials = mountMaterials(buildFlowGraph(base), base, envelopes);

    // 运行时锚定半边：产物状态面恰两键，退役键零驻留
    expect(Object.keys(materials).sort()).toEqual(['columnDocs', 'nodeChecklists']);
    expect('nodeFiles' in materials).toBe(false);
    expect('outsideFiles' in materials).toBe(false);
    // 零文件挂载节点、零图外素材：仅 eval-checklist 挂 eval 节点，文档列全空
    expect(Object.keys(materials.nodeChecklists)).toEqual(['eval:dev-design:2']);
    expect(materials.nodeChecklists['eval:dev-design:2']).toEqual([envelopes[0]]);
    expect(materials.columnDocs).toEqual({});
  });
});
