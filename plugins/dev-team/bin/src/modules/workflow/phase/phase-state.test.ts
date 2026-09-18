/**
 * 单元测试: modules/workflow/phase/phase-state.ts — active_phase / interrupted
 * 运行态操作
 *
 * 覆盖范围（openspec/changes/phase-lifecycle-file-log/test-design.md）:
 * - readActivePhase: 字段缺失或 null 视为无运行态返回 null；文件缺失/非法抛错
 * - writeActivePhase: last-wins 覆盖写入 {phase, attempt, start_at}，未知键与其他
 *   字段保留、2 空格缩进 + 尾换行；非法值（非整数 attempt / 非 ISO start_at）
 *   校验抛错且磁盘不变
 * - clearActivePhase: 置 null，interrupted 不动（phase_log 清场用）；幂等
 * - interruptActivePhase: 有遗留 → 以 end_at（缺省 now）追加进 interrupted[] 并清空，
 *   返回 true；无遗留 → 返回 false 文件逐字节不变；eval 数组不变（AC-10 不写 eval）
 *
 * Mock 策略: 系统时钟（interruptActivePhase 缺省 end_at=now）经 vi.setSystemTime
 * 固定时刻断言；文件系统不 mock（mkdtempSync + 真盘读写）。
 */

import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

import { afterAll, describe, expect, it, vi } from 'vite-plus/test';

import {
  clearActivePhase,
  interruptActivePhase,
  readActivePhase,
  writeActivePhase,
} from './phase-state';

// ---------------------------------------------------------------------------
// Fixture helpers
// ---------------------------------------------------------------------------

interface TempProject {
  root: string;
  cleanup: () => void;
}

function createTempProject(prefix = 'phase-state-test-'): TempProject {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), prefix));
  return {
    root,
    cleanup: () => fs.rmSync(root, { recursive: true, force: true }),
  };
}

function createChangeDir(project: TempProject, name = 'my-change'): string {
  const changeDir = path.join(project.root, 'openspec', 'changes', name);
  fs.mkdirSync(changeDir, { recursive: true });
  return changeDir;
}

function writeWorkflowJson(changeDir: string, doc: unknown): void {
  fs.writeFileSync(path.join(changeDir, 'workflow.json'), JSON.stringify(doc), 'utf-8');
}

function readWorkflowJsonRaw(changeDir: string): string {
  return fs.readFileSync(path.join(changeDir, 'workflow.json'), 'utf-8');
}

function evalEntry(phase: string): Record<string, unknown> {
  return {
    phase,
    verdict: 'pass',
    attempt: 1,
    timestamp: '2026-09-18T00:00:00.000Z',
    report: 'r',
    checklist: [{ item: 'i', pass: true, evidence: 'e' }],
    backtrack_to: null,
  };
}

/** 合法 workflow.json 文档（eval / 未知键 / file_log 必备，运行态字段可注入）。 */
function validDoc(extra: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    workflow_type: 'requirement',
    created: '2026-09-18',
    eval: [evalEntry('proposal')],
    unknown_key: { keep: true },
    file_log: [],
    ...extra,
  };
}

const ACTIVE = { phase: 'implement', attempt: 2, start_at: '2026-09-18T08:00:00.000Z' };
const FIXED_NOW = '2026-09-18T12:00:00.000Z';

afterAll(() => {
  vi.useRealTimers();
});

// ===========================================================================
// writeActivePhase / readActivePhase — 正向
// ===========================================================================

describe('writeActivePhase / readActivePhase — 正向', () => {
  it('写入后读回 {phase, attempt, start_at} 一致；workflow_type/created/eval/未知键保留；2 空格缩进 + 尾换行', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validDoc());

      writeActivePhase(changeDir, ACTIVE);

      expect(readActivePhase(changeDir)).toEqual(ACTIVE);
      const raw = readWorkflowJsonRaw(changeDir);
      const doc = JSON.parse(raw) as Record<string, unknown>;
      expect(doc.workflow_type).toBe('requirement');
      expect(doc.created).toBe('2026-09-18');
      expect(doc.eval).toEqual([evalEntry('proposal')]);
      expect(doc.unknown_key).toEqual({ keep: true });
      expect(doc.file_log).toEqual([]);
      expect(raw.endsWith('\n')).toBe(true);
      expect(raw).toContain('\n  "workflow_type"');
      expect(raw).toContain('\n    "start_at"');
    } finally {
      project.cleanup();
    }
  });

  it('已有 active_phase 时再次写入 → last-wins 覆盖（start_at 刷新；attempt 不变的重入语义由调用方保证）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validDoc({ active_phase: ACTIVE }));

      writeActivePhase(changeDir, {
        phase: 'implement',
        attempt: 2,
        start_at: '2026-09-18T09:30:00.000Z',
      });

      expect(readActivePhase(changeDir)).toEqual({
        phase: 'implement',
        attempt: 2,
        start_at: '2026-09-18T09:30:00.000Z',
      });
    } finally {
      project.cleanup();
    }
  });

  it('重入他 phase → 覆盖为新 phase（last-wins 不挑 phase）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validDoc({ active_phase: ACTIVE }));

      writeActivePhase(changeDir, {
        phase: 'test-gen',
        attempt: 1,
        start_at: '2026-09-18T10:00:00.000Z',
      });

      expect(readActivePhase(changeDir)?.phase).toBe('test-gen');
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// writeActivePhase — 校验收口在读路径
// ===========================================================================

describe('writeActivePhase — 校验收口（写不校验、读时 activePhaseSchema 抛错）', () => {
  it('attempt 非整数 → 写入透传不校验；readActivePhase 读取时 activePhaseSchema 抛错', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validDoc());

      expect(() =>
        writeActivePhase(changeDir, {
          phase: 'implement',
          attempt: 1.5,
          start_at: '2026-09-18T08:00:00.000Z',
        }),
      ).not.toThrow();

      expect(() => readActivePhase(changeDir)).toThrow();
    } finally {
      project.cleanup();
    }
  });

  it('start_at 非 ISO → 写入透传；readActivePhase 读取时抛错（phase 字段在本层为自由字符串，归属校验由 phase_start 命令层保证）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validDoc());

      expect(() =>
        writeActivePhase(changeDir, {
          phase: 'implement',
          attempt: 1,
          start_at: '2026-09-18 08:00:00',
        }),
      ).not.toThrow();

      expect(() => readActivePhase(changeDir)).toThrow();
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// readActivePhase — 边界与异常
// ===========================================================================

describe('readActivePhase — 边界与异常', () => {
  it('active_phase 字段缺失 → 返回 null（不视为非法）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validDoc());

      expect(readActivePhase(changeDir)).toBeNull();
    } finally {
      project.cleanup();
    }
  });

  it('active_phase: null → 返回 null（不视为非法）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validDoc({ active_phase: null }));

      expect(readActivePhase(changeDir)).toBeNull();
    } finally {
      project.cleanup();
    }
  });

  it('文件缺失 → 抛错不创建（调用方吞错策略兜底）', () => {
    const project = createTempProject();
    try {
      const changeDir = path.join(project.root, 'openspec', 'changes', 'missing');

      expect(() => readActivePhase(changeDir)).toThrow(/workflow\.json 不存在/);
      expect(fs.existsSync(changeDir)).toBe(false);
    } finally {
      project.cleanup();
    }
  });

  it('JSON 非法 → 抛「解析失败」', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validDoc());
      fs.writeFileSync(path.join(changeDir, 'workflow.json'), '{broken', 'utf-8');

      expect(() => readActivePhase(changeDir)).toThrow(/解析失败/);
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// clearActivePhase — 清场（phase_log 用）
// ===========================================================================

describe('clearActivePhase', () => {
  it('有运行态时清空 → readActivePhase 为 null；interrupted 数组原样不动；eval 不动', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      const interrupted = [
        {
          phase: 'implement',
          attempt: 1,
          start_at: '2026-09-18T07:00:00.000Z',
          end_at: '2026-09-18T07:30:00.000Z',
        },
      ];
      writeWorkflowJson(changeDir, validDoc({ active_phase: ACTIVE, interrupted }));

      clearActivePhase(changeDir);

      expect(readActivePhase(changeDir)).toBeNull();
      const doc = JSON.parse(readWorkflowJsonRaw(changeDir)) as Record<string, unknown>;
      expect(doc.interrupted).toEqual(interrupted);
      expect(doc.eval).toEqual([evalEntry('proposal')]);
      expect(doc.unknown_key).toEqual({ keep: true });
    } finally {
      project.cleanup();
    }
  });

  it('本无运行态（active_phase: null）→ 置 null 幂等，文件其余内容不变', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validDoc({ active_phase: null }));

      clearActivePhase(changeDir);

      expect(readActivePhase(changeDir)).toBeNull();
      const doc = JSON.parse(readWorkflowJsonRaw(changeDir)) as Record<string, unknown>;
      expect(doc.active_phase).toBeNull();
      expect(doc.unknown_key).toEqual({ keep: true });
    } finally {
      project.cleanup();
    }
  });

  it('本无运行态（字段缺失）→ 幂等置 null，不报错', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validDoc());

      expect(() => clearActivePhase(changeDir)).not.toThrow();
      expect(readActivePhase(changeDir)).toBeNull();
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// interruptActivePhase — 中断归档（sweep 用，AC-10）
// ===========================================================================

describe('interruptActivePhase', () => {
  it('有遗留 → 追加 {phase, attempt, start_at, end_at} 进 interrupted[] 并清空 active_phase，返回 true；显式 endedAt 时使用传入值', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validDoc({ active_phase: ACTIVE }));

      const archived = interruptActivePhase(changeDir, '2026-09-18T11:45:00.000Z');

      expect(archived).toBe(true);
      expect(readActivePhase(changeDir)).toBeNull();
      const doc = JSON.parse(readWorkflowJsonRaw(changeDir)) as Record<string, unknown>;
      expect(doc.interrupted).toEqual([{ ...ACTIVE, end_at: '2026-09-18T11:45:00.000Z' }]);
    } finally {
      project.cleanup();
    }
  });

  it('缺省 endedAt → 使用系统当前时刻（固定时钟断言 end_at=now）', () => {
    vi.setSystemTime(new Date(FIXED_NOW));
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      writeWorkflowJson(changeDir, validDoc({ active_phase: ACTIVE }));

      expect(interruptActivePhase(changeDir)).toBe(true);

      const doc = JSON.parse(readWorkflowJsonRaw(changeDir)) as {
        interrupted: Array<{ end_at: string }>;
      };
      expect(doc.interrupted).toHaveLength(1);
      expect(doc.interrupted[0].end_at).toBe(FIXED_NOW);
    } finally {
      project.cleanup();
      vi.useRealTimers();
    }
  });

  it('无遗留（active_phase 缺失 / null）→ 返回 false，文件逐字节不变', () => {
    const project = createTempProject();
    try {
      for (const activePhase of [undefined, null]) {
        const changeDir = createChangeDir(project, `no-leftover-${String(activePhase)}`);
        const doc = activePhase === undefined ? validDoc() : validDoc({ active_phase: null });
        writeWorkflowJson(changeDir, doc);
        const before = readWorkflowJsonRaw(changeDir);

        expect(interruptActivePhase(changeDir)).toBe(false);
        expect(readWorkflowJsonRaw(changeDir)).toBe(before);
      }
    } finally {
      project.cleanup();
    }
  });

  it('interrupted[] 已有条目 → 追加不覆盖；eval 数组不变（AC-10 不写 eval）', () => {
    const project = createTempProject();
    try {
      const changeDir = createChangeDir(project);
      const prior = [
        {
          phase: 'implement',
          attempt: 1,
          start_at: '2026-09-18T07:00:00.000Z',
          end_at: '2026-09-18T07:30:00.000Z',
        },
      ];
      writeWorkflowJson(changeDir, validDoc({ active_phase: ACTIVE, interrupted: prior }));

      expect(interruptActivePhase(changeDir, '2026-09-18T11:50:00.000Z')).toBe(true);

      const doc = JSON.parse(readWorkflowJsonRaw(changeDir)) as Record<string, unknown>;
      expect(doc.interrupted).toEqual([
        ...prior,
        { ...ACTIVE, end_at: '2026-09-18T11:50:00.000Z' },
      ]);
      expect(doc.eval).toEqual([evalEntry('proposal')]);
    } finally {
      project.cleanup();
    }
  });
});
