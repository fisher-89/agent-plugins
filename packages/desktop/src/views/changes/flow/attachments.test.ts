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
import type { FileLogEntry } from './types';

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

function fileEntry(overrides: Partial<FileLogEntry> = {}): FileLogEntry {
  return {
    op: 'write',
    scope: 'workflow',
    attempt: null,
    path: 'workflow.json',
    at: null,
    ...overrides,
  };
}

/** v2 详情底座：dev-design 站 1 条 eval（多挂载定位用例共用），其余站空。 */
function detail(overrides: Partial<ChangeDetail> = {}): ChangeDetail {
  return {
    name: 'add-feature',
    source: 'active',
    inventory: 'v2',
    created: null,
    unparsable: false,
    pipeline: PIPELINE_PHASES.map((phase) => ({
      phase,
      attempts: phase === 'dev-design' ? [attempt({ attempt: 2 })] : [],
    })),
    activePhase: null,
    fileLog: [],
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

describe('mountMaterials：记录挂节点（eval-checklist / file_log）', () => {
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

  it('file_log 条目 scope 为 9 站 phase id 且 attempt 非 null → 挂到对应节点', () => {
    const base = detail({
      fileLog: [fileEntry({ scope: 'dev-design', attempt: 2, path: 'src/design.ts' })],
    });
    const materials = mountMaterials(buildFlowGraph(base), base, []);
    expect(materials.nodeFiles['eval:dev-design:2']).toHaveLength(1);
    expect(materials.nodeFiles['eval:dev-design:2'][0].path).toBe('src/design.ts');
    expect(materials.outsideFiles).toEqual([]);
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

  it("file_log scope='workflow' 条目 → outsideFiles，不出现在任何 nodeFiles", () => {
    const entry = fileEntry({ scope: 'workflow', path: 'workflow.json' });
    const base = detail({ fileLog: [entry] });
    const materials = mountMaterials(buildFlowGraph(base), base, []);
    expect(materials.nodeFiles).toEqual({});
    expect(materials.outsideFiles).toEqual([entry]);
  });

  it('file_log scope 为 phase id 但 attempt=null → outsideFiles', () => {
    const base = detail({
      fileLog: [fileEntry({ scope: 'dev-design', attempt: null, path: 'src/d.ts' })],
    });
    const materials = mountMaterials(buildFlowGraph(base), base, []);
    expect(materials.nodeFiles).toEqual({});
    expect(materials.outsideFiles).toHaveLength(1);
  });

  it('scope 为非 9 站字符串（未知 phase）或该站无任何事件节点 → outsideFiles', () => {
    const base = detail({
      fileLog: [
        fileEntry({ scope: 'unknown-phase', attempt: 1, path: 'a.txt' }),
        fileEntry({ scope: 'code-review', attempt: 1, path: 'b.txt' }),
      ],
    });
    const materials = mountMaterials(buildFlowGraph(base), base, []);
    expect(materials.nodeFiles).toEqual({});
    expect(materials.outsideFiles.map((entry) => entry.path)).toEqual(['a.txt', 'b.txt']);
  });
});

describe('mountMaterials：聚合与空态', () => {
  it('同列多文档、同节点多素材 → 数组聚合保序不覆盖', () => {
    const artifacts = [
      descriptor('markdown-doc', 'proposal.md', '提案'),
      descriptor('markdown-doc', 'specs/x/spec.md', '规格'),
    ];
    const base = detail({
      artifacts,
      fileLog: [
        fileEntry({ scope: 'dev-design', attempt: 2, path: 'src/1.ts' }),
        fileEntry({ scope: 'dev-design', attempt: 2, path: 'src/2.ts' }),
      ],
    });
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
    expect(materials.nodeFiles['eval:dev-design:2'].map((entry) => entry.path)).toEqual([
      'src/1.ts',
      'src/2.ts',
    ]);
    expect(materials.nodeChecklists['eval:dev-design:2']).toEqual([first, second]);
  });

  it('全空入参（envelopes=[]、fileLog=[]）→ FlowMaterials 四键齐全且全空，不抛错', () => {
    const base = detail();
    expect(mountMaterials(buildFlowGraph(base), base, [])).toEqual({
      columnDocs: {},
      nodeChecklists: {},
      nodeFiles: {},
      outsideFiles: [],
    });
  });

  it('fileLog=null（v1 及更早代际字段缺失形态）→ 与空数组同型全空输出，不抛错', () => {
    const base = detail({ fileLog: null });
    expect(mountMaterials(buildFlowGraph(base), base, [])).toEqual({
      columnDocs: {},
      nodeChecklists: {},
      nodeFiles: {},
      outsideFiles: [],
    });
  });
});

// ---------------------------------------------------------------------------
// NODE_PRECEDENCE 两段收敛（desktop-drawer-session-column，AC-5）：checklist
// 定位降级链 eval → active 两段（interrupted 段无构造输入）；挂载规则本体
// （文档挂列 / checklist 挂节点 / file_log 挂节点 / outsideFiles 兜底）零改动
// ---------------------------------------------------------------------------

describe('mountMaterials：NODE_PRECEDENCE 两段收敛（eval → active）', () => {
  it('仅 active 在场（eval 缺席）→ checklist 与 file_log 条目挂 active:<phase>:<attempt> 节点（降级链末段语义保留）', () => {
    const base = detail({
      pipeline: PIPELINE_PHASES.map((phase) => ({ phase, attempts: [] })),
      activePhase: { phase: 'dev-design', attempt: 3, startAt: '2026-09-05T00:00:00Z' },
      fileLog: [fileEntry({ scope: 'dev-design', attempt: 3, path: 'src/active.ts' })],
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
    expect(materials.nodeFiles['active:dev-design:3'].map((entry) => entry.path)).toEqual([
      'src/active.ts',
    ]);
    expect(materials.outsideFiles).toEqual([]);
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

  it('同号 eval / active 均缺席（仅他号节点在场）→ checklist 与 file_log 条目均不误挂他号节点（miss 兜底）', () => {
    const base = detail({
      fileLog: [fileEntry({ scope: 'implement', attempt: 1, path: 'src/miss.ts' })],
    });
    const checklist = envelope('eval-checklist', '评估清单', {
      phase: 'implement',
      attempt: 1,
      verdict: 'pass',
      items: [],
    });
    const materials = mountMaterials(buildFlowGraph(base), base, [checklist]);
    expect(materials.nodeChecklists).toEqual({});
    expect(materials.nodeFiles).toEqual({});
    expect(materials.outsideFiles.map((entry) => entry.path)).toEqual(['src/miss.ts']);
  });
});
