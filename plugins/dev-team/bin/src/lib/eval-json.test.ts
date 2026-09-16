/**
 * Tests for eval-json.ts — the authoritative evaluation store.
 *
 * `readEvalJson` / `writeEvalJson` / `appendEntry` are exercised against a real
 * temporary filesystem (`fs.mkdtempSync`) — `fs` is deliberately NOT mocked so
 * that "writes then unlinks a leftover `eval.json`", "never creates the
 * directory / file" and "leaves the on-disk content untouched on failure"
 * remain real contracts.
 *
 * The retained pure helpers (`validateVerdict` / `buildEntry` / `markPhaseStale`
 * / `computeAttempt` / `validateReportLength`) keep their in-memory cases.
 */

import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import { describe, it, expect, beforeEach, afterEach } from 'vite-plus/test';

import { phaseLogSchema } from '../schemas';
import {
  readEvalJson,
  writeEvalJson,
  appendEntry,
  validateVerdict,
  validateReportLength,
  buildEntry,
  computeAttempt,
  markPhaseStale,
  type BuildEntryParams,
  type EvalEntry,
} from './eval-json';

// ---------------------------------------------------------------------------
// Fixture helpers — real temp change directories
// ---------------------------------------------------------------------------

const WORKFLOW_JSON = 'workflow.json';
const LEGACY_EVAL_JSON = 'eval.json';

let tempRoot: string;

beforeEach(() => {
  tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'eval-json-test-'));
});

afterEach(() => {
  fs.rmSync(tempRoot, { recursive: true, force: true });
});

/** Create (and return) a change directory under the temp root. */
function makeChangeDir(name: string = 'my-change'): string {
  const dir = path.join(tempRoot, name);
  fs.mkdirSync(dir, { recursive: true });
  return dir;
}

function workflowPath(changeDir: string): string {
  return path.join(changeDir, WORKFLOW_JSON);
}

function legacyPath(changeDir: string): string {
  return path.join(changeDir, LEGACY_EVAL_JSON);
}

function makeEntry(overrides: Partial<EvalEntry> = {}): EvalEntry {
  return {
    phase: 'proposal',
    verdict: 'pass',
    attempt: 1,
    timestamp: '2026-09-11T10:00:00.000Z',
    report: 'ok',
    checklist: [{ item: '检查项', pass: true, evidence: 'ok' }],
    backtrack_to: null,
    ...overrides,
  };
}

/** Write a raw (possibly malformed) `workflow.json`. */
function writeRawWorkflow(changeDir: string, raw: string): void {
  fs.writeFileSync(workflowPath(changeDir), raw, 'utf-8');
}

/** Write an object-shaped `workflow.json`. */
function writeWorkflow(changeDir: string, doc: unknown): void {
  writeRawWorkflow(changeDir, JSON.stringify(doc));
}

/** Write a legacy `eval.json` (object or raw string). */
function writeLegacy(changeDir: string, value: unknown): void {
  fs.writeFileSync(
    legacyPath(changeDir),
    typeof value === 'string' ? value : JSON.stringify(value),
    'utf-8',
  );
}

function readWorkflow(changeDir: string): Record<string, unknown> {
  return JSON.parse(fs.readFileSync(workflowPath(changeDir), 'utf-8')) as Record<string, unknown>;
}

/** `{ workflow_type, created }` — the shape `change_create` produces. */
function metadataOnly(
  workflowType: string = 'requirement',
  extra: Record<string, unknown> = {},
): Record<string, unknown> {
  return { workflow_type: workflowType, created: '2026-09-11', ...extra };
}

function captureError(fn: () => unknown): Error {
  try {
    fn();
  } catch (e: unknown) {
    return e as Error;
  }
  throw new Error('expected the call to throw, but it returned normally');
}

// ===========================================================================
// readEvalJson — 正向
// ===========================================================================

describe('readEvalJson — 正向（权威数组优先）', () => {
  it('workflow.json.eval 含一条合法条目时返回该数组，且不读取同目录 eval.json（AC-1）', () => {
    const dir = makeChangeDir();
    const authoritative = makeEntry({ report: '权威条目' });
    writeWorkflow(dir, metadataOnly('requirement', { eval: [authoritative] }));
    writeLegacy(dir, [makeEntry({ report: '遗留条目' })]);

    const entries = readEvalJson(dir);

    expect(entries).toHaveLength(1);
    expect(entries[0].report).toBe('权威条目');
  });

  it('workflow.json 无 eval 键、遗留 eval.json 为合法数组时返回遗留数组（AC-3）', () => {
    const dir = makeChangeDir();
    writeWorkflow(dir, metadataOnly());
    writeLegacy(dir, [makeEntry({ report: '遗留' })]);

    const entries = readEvalJson(dir);

    expect(entries).toHaveLength(1);
    expect(entries[0].report).toBe('遗留');
  });

  it('workflow.json 不存在、仅有合法 eval.json 时返回遗留数组（AC-3）', () => {
    const dir = makeChangeDir();
    writeLegacy(dir, [makeEntry(), makeEntry({ phase: 'dev-design', attempt: 1 })]);

    expect(readEvalJson(dir)).toHaveLength(2);
  });

  it('workflow.json 仅有 { workflow_type, created } 且无遗留文件时返回 []', () => {
    const dir = makeChangeDir();
    writeWorkflow(dir, metadataOnly());

    expect(readEvalJson(dir)).toEqual([]);
  });

  it('workflow.json.eval 为 [] 且遗留 eval.json 非空时返回 []，不合并遗留条目（AC-5）', () => {
    const dir = makeChangeDir();
    writeWorkflow(dir, metadataOnly('requirement', { eval: [] }));
    writeLegacy(dir, [makeEntry(), makeEntry()]);

    expect(readEvalJson(dir)).toEqual([]);
  });

  it('两文件皆不存在（change 目录存在）时返回 []，不抛错', () => {
    expect(readEvalJson(makeChangeDir())).toEqual([]);
  });

  it('eval 键缺失（Optional None）且无遗留文件时返回 []', () => {
    const dir = makeChangeDir();
    writeWorkflow(dir, metadataOnly('test-only'));

    expect(readEvalJson(dir)).toEqual([]);
  });

  it('含 eval 数组且含未知键（如 note）时返回条目数组，未知键不影响', () => {
    const dir = makeChangeDir();
    writeWorkflow(dir, metadataOnly('requirement', { note: 'x', eval: [makeEntry()] }));

    expect(readEvalJson(dir)).toHaveLength(1);
  });
});

// ===========================================================================
// readEvalJson — 异常
// ===========================================================================

describe('readEvalJson — 异常（权威数组非法即抛错，不回退）', () => {
  it('workflow.json.eval 为对象 {} 时抛错，message 含 workflow.json.eval 必须是数组，不回退 eval.json（AC-12）', () => {
    const dir = makeChangeDir();
    writeWorkflow(dir, { ...metadataOnly(), eval: {} });
    writeLegacy(dir, [makeEntry()]);

    const error = captureError(() => readEvalJson(dir));

    expect(error.message).toContain('workflow.json.eval 必须是数组');
  });

  it('workflow.json.eval 为 null / 字符串 / 数字时抛错且不回退', () => {
    for (const invalid of [null, 'entries', 42]) {
      const dir = makeChangeDir();
      writeWorkflow(dir, { ...metadataOnly(), eval: invalid });
      writeLegacy(dir, [makeEntry()]);

      expect(() => readEvalJson(dir)).toThrow(/workflow\.json\.eval 必须是数组/);
    }
  });

  it('workflow.json 根为数组时抛错（根必须是对象），不回退', () => {
    const dir = makeChangeDir();
    writeWorkflow(dir, [makeEntry()]);
    writeLegacy(dir, [makeEntry()]);

    expect(() => readEvalJson(dir)).toThrow(/根元素必须是对象/);
  });

  it('workflow.json 非法 JSON 时抛错，message 含解析失败语义，不回退', () => {
    const dir = makeChangeDir();
    writeRawWorkflow(dir, '{ not json');
    writeLegacy(dir, [makeEntry()]);

    expect(() => readEvalJson(dir)).toThrow(/workflow\.json 解析失败/);
  });

  it('eval 数组元素不符合 phaseLogSchema（非法 phase 枚举 / 缺必填）时抛 Zod 校验错误', () => {
    const dir = makeChangeDir();
    writeWorkflow(dir, {
      ...metadataOnly(),
      eval: [{ phase: 'integration-test', verdict: 'pass', report: '', checklist: [] }],
    });
    expect(() => readEvalJson(dir)).toThrow();

    const dir2 = makeChangeDir('my-change-2');
    writeWorkflow(dir2, { ...metadataOnly(), eval: [{ verdict: 'pass' }] });
    expect(() => readEvalJson(dir2)).toThrow();
  });

  it('走遗留路径且 eval.json 根为对象时抛错，message 含 eval.json 根元素必须是数组', () => {
    const dir = makeChangeDir();
    writeWorkflow(dir, metadataOnly());
    writeLegacy(dir, { phase: 'proposal', verdict: 'pass' });

    expect(() => readEvalJson(dir)).toThrow(/eval\.json 根元素必须是数组/);
  });

  it('遗留 eval.json 非法 JSON 时抛错', () => {
    const dir = makeChangeDir();
    writeWorkflow(dir, metadataOnly());
    writeLegacy(dir, 'not json [');

    expect(() => readEvalJson(dir)).toThrow(/eval\.json 解析失败/);
  });

  it('changeDir 为 undefined / null 时抛 TypeError 或路径错误，不静默返回遗留数据', () => {
    expect(() => readEvalJson(undefined as unknown as string)).toThrow();
    expect(() => readEvalJson(null as unknown as string)).toThrow();
  });
});

// ===========================================================================
// readEvalJson — 边界
// ===========================================================================

describe('readEvalJson — 边界', () => {
  it('changeDir 为空串时按「文件不存在」返回 []，不崩溃', () => {
    expect(readEvalJson('')).toEqual([]);
  });

  it('changeDir 超长（>1000 chars）时按「文件不存在」返回 []，不崩溃', () => {
    expect(readEvalJson('a'.repeat(1001))).toEqual([]);
  });

  it('changeDir 含 \\n / emoji 等特殊字符时不崩溃', () => {
    expect(readEvalJson('a\nb-🧪')).toEqual([]);
  });

  it('eval 为单元素数组时返回长度 1', () => {
    const dir = makeChangeDir();
    writeWorkflow(dir, { ...metadataOnly(), eval: [makeEntry()] });

    expect(readEvalJson(dir)).toHaveLength(1);
  });

  it('eval 为超大列表（>100 条合法条目）时返回等长数组且顺序不变', () => {
    const dir = makeChangeDir();
    const entries = Array.from({ length: 120 }, (_, i) =>
      makeEntry({ attempt: i + 1, report: `entry-${i}` }),
    );
    writeWorkflow(dir, { ...metadataOnly(), eval: entries });

    const read = readEvalJson(dir);

    expect(read).toHaveLength(120);
    expect(read.map((e) => e.report)).toEqual(entries.map((e) => e.report));
  });

  it('workflow.json 键为 "Eval" / "entries"（大小写或别名）时视为无权威数组，回退遗留 eval.json', () => {
    const dir = makeChangeDir();
    writeWorkflow(dir, {
      workflow_type: 'requirement',
      created: '2026-09-11',
      Eval: [makeEntry({ report: '别名条目' })],
      entries: [makeEntry({ report: '别名条目' })],
    });
    writeLegacy(dir, [makeEntry({ report: '遗留条目' })]);

    const entries = readEvalJson(dir);

    expect(entries).toHaveLength(1);
    expect(entries[0].report).toBe('遗留条目');
  });
});

// ===========================================================================
// writeEvalJson — 正向
// ===========================================================================

describe('writeEvalJson — 正向', () => {
  it('已有 { workflow_type, created } 写入一条时两字段值不变、eval 长度 1、不创建 eval.json（AC-1、AC-2）', () => {
    const dir = makeChangeDir();
    writeWorkflow(dir, metadataOnly('test-only'));

    writeEvalJson(dir, [makeEntry()]);

    const doc = readWorkflow(dir);
    expect(doc.workflow_type).toBe('test-only');
    expect(doc.created).toBe('2026-09-11');
    expect(doc.eval).toHaveLength(1);
    expect(fs.existsSync(legacyPath(dir))).toBe(false);
  });

  it('已有未知键 "note": "x" 时写回后仍为 "x"（AC-2）', () => {
    const dir = makeChangeDir();
    writeWorkflow(dir, metadataOnly('requirement', { note: 'x' }));

    writeEvalJson(dir, [makeEntry()]);

    expect(readWorkflow(dir).note).toBe('x');
  });

  it('同目录存在遗留 eval.json 时写成功后 eval.json 不存在，条目仅在 workflow.json.eval（AC-4）', () => {
    const dir = makeChangeDir();
    writeWorkflow(dir, metadataOnly());
    writeLegacy(dir, [makeEntry({ report: '遗留' })]);

    writeEvalJson(dir, [makeEntry({ report: '权威' })]);

    expect(fs.existsSync(legacyPath(dir))).toBe(false);
    const entries = readEvalJson(dir);
    expect(entries).toHaveLength(1);
    expect(entries[0].report).toBe('权威');
  });

  it('传入完整数组覆盖磁盘 eval 字段，不与遗留 eval.json 数组合并', () => {
    const dir = makeChangeDir();
    writeWorkflow(dir, { ...metadataOnly(), eval: [makeEntry({ report: '旧权威' })] });
    writeLegacy(dir, [makeEntry({ report: '遗留' })]);

    writeEvalJson(dir, [makeEntry({ report: '新权威' })]);

    const doc = readWorkflow(dir);
    expect(doc.eval).toHaveLength(1);
    expect((doc.eval as EvalEntry[])[0].report).toBe('新权威');
  });

  it('序列化为 2 空格缩进且文件以换行结尾', () => {
    const dir = makeChangeDir();
    writeWorkflow(dir, metadataOnly());

    writeEvalJson(dir, [makeEntry()]);

    const raw = fs.readFileSync(workflowPath(dir), 'utf-8');
    expect(raw.endsWith('\n')).toBe(true);
    expect(raw).toContain('\n  "eval": [');
    expect(raw).toContain('\n    {\n');
  });

  it('同目录本无 eval.json 时写成功且不因 unlink 抛错', () => {
    const dir = makeChangeDir();
    writeWorkflow(dir, metadataOnly());

    expect(() => writeEvalJson(dir, [makeEntry()])).not.toThrow();
    expect(fs.existsSync(legacyPath(dir))).toBe(false);
  });

  it('已有文件含多余字段（未知键）时全部保留', () => {
    const dir = makeChangeDir();
    writeWorkflow(dir, {
      workflow_type: 'refactor',
      created: '2026-09-11',
      note: 'keep-me',
      nested: { a: 1 },
    });

    writeEvalJson(dir, [makeEntry()]);

    const doc = readWorkflow(dir);
    expect(doc.note).toBe('keep-me');
    expect(doc.nested).toEqual({ a: 1 });
  });
});

// ===========================================================================
// writeEvalJson — 异常
// ===========================================================================

describe('writeEvalJson — 异常（严格前置条件）', () => {
  it('workflow.json 不存在时抛错，message 含绝对路径与 change_create 指引；目录与文件均未被创建（AC-13）', () => {
    const missingDir = path.join(tempRoot, 'not-yet-created');

    const error = captureError(() => writeEvalJson(missingDir, [makeEntry()]));

    expect(error.message).toContain('workflow.json 不存在');
    expect(error.message).toContain(path.join(missingDir, WORKFLOW_JSON));
    expect(error.message).toContain('change_create');
    expect(fs.existsSync(missingDir)).toBe(false);
    expect(fs.existsSync(path.join(missingDir, WORKFLOW_JSON))).toBe(false);
  });

  it('已有 workflow.json 根为数组时抛错，文件内容不被改写，遗留 eval.json 不被删除（AC-14）', () => {
    const dir = makeChangeDir();
    const raw = JSON.stringify([makeEntry()]);
    writeRawWorkflow(dir, raw);
    writeLegacy(dir, [makeEntry()]);

    expect(() => writeEvalJson(dir, [makeEntry()])).toThrow(/根元素必须是对象/);
    expect(fs.readFileSync(workflowPath(dir), 'utf-8')).toBe(raw);
    expect(fs.existsSync(legacyPath(dir))).toBe(true);
  });

  it('已有 workflow.json 非法 JSON 时抛错，不删除遗留 eval.json（AC-14）', () => {
    const dir = makeChangeDir();
    writeRawWorkflow(dir, '{ broken');
    writeLegacy(dir, [makeEntry()]);

    expect(() => writeEvalJson(dir, [makeEntry()])).toThrow(/解析失败/);
    expect(fs.existsSync(legacyPath(dir))).toBe(true);
  });

  it('workflow_type 缺失 / 非枚举 / 非字符串时抛错且不写入（AC-14）', () => {
    for (const doc of [{}, { workflow_type: 'unknown' }, { workflow_type: 123 }]) {
      const dir = makeChangeDir();
      writeRawWorkflow(dir, JSON.stringify({ created: '2026-09-11', ...doc }));
      const before = fs.readFileSync(workflowPath(dir), 'utf-8');

      expect(() => writeEvalJson(dir, [makeEntry()])).toThrow(/格式非法/);
      expect(fs.readFileSync(workflowPath(dir), 'utf-8')).toBe(before);
    }
  });

  it('created 为 2026/09/11（非 YYYY-MM-DD）时抛错且不写入（AC-14）', () => {
    const dir = makeChangeDir();
    writeWorkflow(dir, { workflow_type: 'requirement', created: '2026/09/11' });

    expect(() => writeEvalJson(dir, [makeEntry()])).toThrow(/格式非法/);
    expect(readWorkflow(dir)).not.toHaveProperty('eval');
  });

  it('磁盘 eval 为对象 {} 时抛错且不写入', () => {
    const dir = makeChangeDir();
    writeWorkflow(dir, { ...metadataOnly(), eval: {} });

    expect(() => writeEvalJson(dir, [makeEntry()])).toThrow(/格式非法/);
    expect(readWorkflow(dir).eval).toEqual({});
  });

  it('entries 为 undefined / null（None）时不得写成非数组 eval', () => {
    for (const entries of [undefined, null]) {
      const dir = makeChangeDir();
      writeWorkflow(dir, metadataOnly());

      expect(() => writeEvalJson(dir, entries as unknown as EvalEntry[])).toThrow();

      const doc = readWorkflow(dir);
      // 设计仅要求「抛错或写入失败，不得写成非数组 eval」：
      // 抛错前未写盘则 eval 缺失，写入失败则可能是 []——两者皆可，唯独不能是非数组。
      expect(doc.eval ?? []).toEqual([]);
    }
  });

  it('changeDir 为 undefined / null（None）时抛 TypeError 或路径错误，不得写出 eval.json', () => {
    expect(() => writeEvalJson(undefined as unknown as string, [makeEntry()])).toThrow();
    expect(() => writeEvalJson(null as unknown as string, [makeEntry()])).toThrow();
  });
});

// ===========================================================================
// writeEvalJson — 边界
// ===========================================================================

describe('writeEvalJson — 边界', () => {
  it('changeDir 为空串时抛缺文件错误，不得把权威数组写到无关的 eval.json', () => {
    expect(() => writeEvalJson('', [makeEntry()])).toThrow(/workflow\.json 不存在/);
    expect(fs.existsSync(path.resolve('eval.json'))).toBe(false);
  });

  it('changeDir 超长（>1000 chars）时抛路径错误而非 mkdirSync 超长目录', () => {
    const longDir = path.join(tempRoot, 'a'.repeat(1001));
    expect(() => writeEvalJson(longDir, [makeEntry()])).toThrow();
    expect(fs.existsSync(longDir)).toBe(false);
  });

  it('changeDir 含 emoji 时只写该目录下的 workflow.json.eval', () => {
    const dir = makeChangeDir('chg-🧪');
    writeWorkflow(dir, metadataOnly());

    writeEvalJson(dir, [makeEntry({ report: 'emoji-dir' })]);

    expect(fs.readFileSync(workflowPath(dir), 'utf-8')).toContain('emoji-dir');
  });

  it('entries=[] 时 workflow.json.eval 为 []，遗留 eval.json 仍被删除（AC-5）', () => {
    const dir = makeChangeDir();
    writeWorkflow(dir, { ...metadataOnly(), eval: [makeEntry()] });
    writeLegacy(dir, [makeEntry(), makeEntry()]);

    writeEvalJson(dir, []);

    expect(readWorkflow(dir).eval).toEqual([]);
    expect(fs.existsSync(legacyPath(dir))).toBe(false);
  });

  it('entries 为单元素时 eval.length===1', () => {
    const dir = makeChangeDir();
    writeWorkflow(dir, metadataOnly());

    writeEvalJson(dir, [makeEntry()]);

    expect(readWorkflow(dir).eval).toHaveLength(1);
  });

  it('entries 为超大列表（>100 条）时写回长度与顺序一致', () => {
    const dir = makeChangeDir();
    writeWorkflow(dir, metadataOnly());
    const entries = Array.from({ length: 130 }, (_, i) => makeEntry({ attempt: i + 1 }));

    writeEvalJson(dir, entries);

    const read = readEvalJson(dir);
    expect(read).toHaveLength(130);
    expect(read.map((e) => e.attempt)).toEqual(entries.map((e) => e.attempt));
  });

  it('已有文件缺少 created（Optional None）时写成功且不补写该键', () => {
    const dir = makeChangeDir();
    writeWorkflow(dir, { workflow_type: 'requirement' });

    writeEvalJson(dir, [makeEntry()]);

    const doc = readWorkflow(dir);
    expect(Object.prototype.hasOwnProperty.call(doc, 'created')).toBe(false);
    expect(doc.workflow_type).toBe('requirement');
  });
});

// ===========================================================================
// appendEntry
// ===========================================================================

describe('appendEntry — 正向', () => {
  it('仅有 { workflow_type, created } 时追加一条 → eval.length===1 且不产生 eval.json（AC-1）', () => {
    const dir = makeChangeDir();
    writeWorkflow(dir, metadataOnly());

    appendEntry(dir, makeEntry());

    expect(readWorkflow(dir).eval).toHaveLength(1);
    expect(fs.existsSync(legacyPath(dir))).toBe(false);
  });

  it('仅有遗留 eval.json 时追加 → 新数组含旧条目+新条目，落在 workflow.json.eval，eval.json 被删除（AC-4）', () => {
    const dir = makeChangeDir();
    writeWorkflow(dir, metadataOnly());
    writeLegacy(dir, [makeEntry({ report: '旧条目' })]);

    appendEntry(dir, makeEntry({ report: '新条目' }));

    const entries = readEvalJson(dir);
    expect(entries.map((e) => e.report)).toEqual(['旧条目', '新条目']);
    expect(fs.existsSync(legacyPath(dir))).toBe(false);
  });

  it('权威 eval 已有条目时再追加 → 旧条目保留，长度 +1', () => {
    const dir = makeChangeDir();
    writeWorkflow(dir, { ...metadataOnly(), eval: [makeEntry({ report: '旧条目' })] });

    appendEntry(dir, makeEntry({ report: '新条目' }));

    const entries = readEvalJson(dir);
    expect(entries.map((e) => e.report)).toEqual(['旧条目', '新条目']);
  });

  it('连续两次追加同一结构 → 长度为 2（非覆盖）', () => {
    const dir = makeChangeDir();
    writeWorkflow(dir, metadataOnly());

    appendEntry(dir, makeEntry());
    appendEntry(dir, makeEntry());

    expect(readEvalJson(dir)).toHaveLength(2);
  });

  it('entry 含多余未知字段时随数组写入', () => {
    const dir = makeChangeDir();
    writeWorkflow(dir, metadataOnly());

    appendEntry(dir, { ...makeEntry(), note: 'extra' } as EvalEntry);

    expect(readWorkflow(dir).eval).toHaveLength(1);
    expect((readWorkflow(dir).eval as Record<string, unknown>[])[0].note).toBe('extra');
  });
});

describe('appendEntry — 异常', () => {
  it('readEvalJson 因 eval 非数组抛错时 appendEntry 向上抛出，不写盘', () => {
    const dir = makeChangeDir();
    writeWorkflow(dir, { ...metadataOnly(), eval: {} });

    expect(() => appendEntry(dir, makeEntry())).toThrow(/workflow\.json\.eval 必须是数组/);
    expect(readWorkflow(dir).eval).toEqual({});
  });

  it('workflow.json 缺失时抛错且不创建任何文件 / 目录（AC-13）', () => {
    const missingDir = path.join(tempRoot, 'absent');

    expect(() => appendEntry(missingDir, makeEntry())).toThrow(/workflow\.json 不存在/);
    expect(fs.existsSync(missingDir)).toBe(false);
    expect(fs.existsSync(path.join(missingDir, LEGACY_EVAL_JSON))).toBe(false);
  });

  it('entry 为 undefined / null（None）时抛错', () => {
    const dir = makeChangeDir();
    writeWorkflow(dir, metadataOnly());

    expect(() => appendEntry(dir, undefined as unknown as EvalEntry)).toThrow();
    expect(() => appendEntry(dir, null as unknown as EvalEntry)).toThrow();
  });

  it('changeDir 为 undefined / null（None）时抛 TypeError 或路径错误，不得创建 eval.json', () => {
    expect(() => appendEntry(undefined as unknown as string, makeEntry())).toThrow();
    expect(() => appendEntry(null as unknown as string, makeEntry())).toThrow();
  });
});

describe('appendEntry — 边界', () => {
  it('changeDir 为空串时抛缺文件错误，不得写无关 eval.json', () => {
    expect(() => appendEntry('', makeEntry())).toThrow(/workflow\.json 不存在/);
    expect(fs.existsSync(path.resolve(LEGACY_EVAL_JSON))).toBe(false);
  });

  it('changeDir 超长（>1000 chars）时不崩溃且不 mkdir 超长路径', () => {
    const longDir = path.join(tempRoot, 'a'.repeat(1001));
    expect(() => appendEntry(longDir, makeEntry())).toThrow();
    expect(fs.existsSync(longDir)).toBe(false);
  });

  it('changeDir 含 emoji 时只在该目录追加 workflow.json.eval', () => {
    const dir = makeChangeDir('chg-🧪');
    writeWorkflow(dir, metadataOnly());

    appendEntry(dir, makeEntry({ attempt: 7 }));

    expect(readWorkflow(dir).eval).toHaveLength(1);
  });
});

// ===========================================================================
// 保留的纯函数用例（本变更不触碰其算法）
// ===========================================================================

describe('validateVerdict', () => {
  it("should accept 'pass'", () => {
    expect(() => validateVerdict('pass')).not.toThrow();
  });

  it("should accept 'fail'", () => {
    expect(() => validateVerdict('fail')).not.toThrow();
  });

  it('should reject invalid verdict', () => {
    expect(() => validateVerdict('invalid')).toThrow();
  });

  it('should reject empty string', () => {
    expect(() => validateVerdict('')).toThrow();
  });

  it("should accept 'pass' when skipped is true", () => {
    expect(() => validateVerdict('pass', true)).not.toThrow();
  });

  it("should reject 'fail' when skipped is true", () => {
    expect(() => validateVerdict('fail', true)).toThrow(/skipped=true 时 verdict 必须为 "pass"/);
  });

  it("should reject 'pass' when skipped is false (same behavior as undefined)", () => {
    expect(() => validateVerdict('pass', false)).not.toThrow();
  });
});

describe('validateReportLength', () => {
  it('500 字符（含）以内通过', () => {
    expect(() => validateReportLength('x'.repeat(500))).not.toThrow();
    expect(() => validateReportLength('')).not.toThrow();
  });

  it('501 字符抛错', () => {
    expect(() => validateReportLength('x'.repeat(501))).toThrow(/500 字符/);
  });
});

describe('computeAttempt', () => {
  it('未传 explicitAttempt 时返回该 phase 已有条目数 + 1', () => {
    const entries = [makeEntry({ phase: 'proposal' }), makeEntry({ phase: 'proposal' })];
    expect(computeAttempt(entries, 'proposal')).toBe(3);
    expect(computeAttempt(entries, 'dev-design')).toBe(1);
    expect(computeAttempt([], 'proposal')).toBe(1);
  });

  it('显式传正整数时原样返回', () => {
    expect(computeAttempt([], 'proposal', 4)).toBe(4);
  });

  it('显式传 0 / -1 时抛「attempt 必须为正整数」', () => {
    expect(() => computeAttempt([], 'proposal', 0)).toThrow(/attempt 必须为正整数/);
    expect(() => computeAttempt([], 'proposal', -1)).toThrow(/attempt 必须为正整数/);
  });
});

describe('buildEntry', () => {
  const baseParams: BuildEntryParams = {
    phase: 'test-execution',
    verdict: 'pass',
    report: 'All tests passed',
    checklist: [{ item: '测试覆盖率达到80%', pass: true, evidence: 'ok' }],
    attempt: 1,
  };

  it('should build a basic entry with required fields', () => {
    const entry = buildEntry(baseParams);
    expect(entry.phase).toBe('test-execution');
    expect(entry.verdict).toBe('pass');
    expect(entry.attempt).toBe(1);
    expect(entry.timestamp).toBeDefined();
    // backtrack_to not set by buildEntry (handled by standalone backtrack tool)
    expect(entry.backtrack_to).toBeUndefined();
    // Extended fields not set
    expect(entry.skipped).toBeUndefined();
  });

  it('should include skipped field when set', () => {
    const entry = buildEntry({ ...baseParams, skipped: true });
    expect(entry.skipped).toBe(true);
  });

  it('should include all extended fields simultaneously', () => {
    const params: BuildEntryParams = {
      ...baseParams,
      skipped: true,
      report: 'No tests found, skipping',
    };
    const entry = buildEntry(params);
    expect(entry.skipped).toBe(true);
    expect(entry.report).toBe('No tests found, skipping');
  });

  it('should build entry without backtrack fields (now handled by standalone backtrack tool)', () => {
    const entry = buildEntry(baseParams);
    expect(entry.phase).toBe('test-execution');
    expect(entry.backtrack_to).toBeUndefined();
    expect(entry.backtrack_reason).toBeUndefined();
  });

  it('phase 为 "integration-test" 时 zod parse 应失败（不在枚举中）', () => {
    const result = phaseLogSchema.safeParse({
      phase: 'integration-test',
      attempt: 1,
      verdict: 'pass',
      report: 'test',
      checklist: [],
      timestamp: new Date().toISOString(),
      backtrack_to: null,
    });
    expect(result.success).toBe(false);
  });

  it('phase 为 "unit-test" 时 zod parse 应失败', () => {
    const result = phaseLogSchema.safeParse({
      phase: 'unit-test',
      attempt: 1,
      verdict: 'pass',
      report: 'test',
      checklist: [],
      timestamp: new Date().toISOString(),
      backtrack_to: null,
    });
    expect(result.success).toBe(false);
  });

  it('report 长度 > 500 字符时 zod parse 应失败', () => {
    expect(() => buildEntry({ ...baseParams, report: 'x'.repeat(501) })).toThrow();
  });

  it('checklist 为空数组时应通过', () => {
    const entry = buildEntry({ ...baseParams, checklist: [] });
    expect(entry.checklist).toEqual([]);
  });
});

// ---------------------------------------------------------------------------
// markPhaseStale — AC-8, AC-14
// ---------------------------------------------------------------------------

function makePassEntry(phase: EvalEntry['phase'], attempt: number = 1, ts?: string): EvalEntry {
  return {
    phase,
    verdict: 'pass',
    attempt,
    timestamp: ts || new Date(Date.now() + attempt).toISOString(),
    backtrack_to: null,
    report: '',
    checklist: [],
  };
}

function makeFailEntry(phase: EvalEntry['phase'], attempt: number = 1): EvalEntry {
  return {
    phase,
    verdict: 'fail',
    attempt,
    timestamp: new Date(Date.now() + attempt).toISOString(),
    backtrack_to: null,
    report: '',
    checklist: [],
  };
}

describe('markPhaseStale', () => {
  it('should mark latest pass entry stale and propagate downstream (AC-8)', () => {
    const entries = [
      makePassEntry('proposal', 1),
      makePassEntry('dev-design', 1),
      makePassEntry('test-design', 1),
      makePassEntry('test-gen', 1),
      makePassEntry('implement', 1),
    ];
    markPhaseStale(entries, 'dev-design', 'requirement');

    // 02 should be stale
    const e2 = entries.find((e) => e.phase === 'dev-design')!;
    expect(e2.stale).toBe(true);

    // Downstream should be propagated: 03 (dep on 02), 05 (dep on 02)
    const e3 = entries.find((e) => e.phase === 'test-design')!;
    expect(e3.stale).toBe(true);
    const e5 = entries.find((e) => e.phase === 'implement')!;
    expect(e5.stale).toBe(true);

    // 01 should NOT be stale (no dependency on 02)
    const e1 = entries.find((e) => e.phase === 'proposal')!;
    expect(e1.stale).toBeUndefined();
  });

  it('should mark latest of multiple pass entries', () => {
    const entries = [
      makePassEntry('dev-design', 1, '2026-01-01T00:00:00.000Z'),
      makePassEntry('dev-design', 2, '2026-01-02T00:00:00.000Z'),
    ];
    markPhaseStale(entries, 'dev-design', 'requirement');

    const attempt2 = entries.find((e) => e.phase === 'dev-design' && e.attempt === 2)!;
    expect(attempt2.stale).toBe(true);
  });

  it('should be no-op when no pass entry exists', () => {
    const entries = [makeFailEntry('dev-design', 1)];
    expect(() => markPhaseStale(entries, 'dev-design', 'requirement')).not.toThrow();
    expect(entries[0].stale).toBeUndefined();
  });

  it('should propagate to full transitive closure', () => {
    const entries = [];
    const phases: EvalEntry['phase'][] = [
      'proposal',
      'dev-design',
      'test-design',
      'test-gen',
      'implement',
      'test-execution',
      'code-review',
      'acceptance',
    ];
    for (const phase of phases) {
      entries.push(makePassEntry(phase, 1));
    }
    markPhaseStale(entries, 'dev-design', 'requirement');

    // 02 stale
    expect(entries.find((e) => e.phase === 'dev-design')!.stale).toBe(true);
    // 03 stale (dep on 02)
    expect(entries.find((e) => e.phase === 'test-design')!.stale).toBe(true);
    // 04 stale (transitive: 03 → 04)
    expect(entries.find((e) => e.phase === 'test-gen')!.stale).toBe(true);
    // 05 stale (dep on 02)
    expect(entries.find((e) => e.phase === 'implement')!.stale).toBe(true);
    // 06 stale (dep on 04+05, both in chain)
    expect(entries.find((e) => e.phase === 'test-execution')!.stale).toBe(true);
    // 07 stale
    expect(entries.find((e) => e.phase === 'code-review')!.stale).toBe(true);
    // 08 stale (dep on 01+02+05)
    expect(entries.find((e) => e.phase === 'acceptance')!.stale).toBe(true);
    // 01 should NOT be stale
    expect(entries.find((e) => e.phase === 'proposal')!.stale).toBeUndefined();
  });

  it('should not mark same-phase new entry as stale', () => {
    // This simulates: old entry marked stale, new entry written after markPhaseStale
    const entries = [makePassEntry('dev-design', 1)];
    markPhaseStale(entries, 'dev-design', 'requirement');
    // Old entry stale
    expect(entries[0].stale).toBe(true);

    // Simulate new entry being added after markPhaseStale
    entries.push(makePassEntry('dev-design', 2));
    const newEntry = entries.find((e) => e.phase === 'dev-design' && e.attempt === 2)!;
    expect(newEntry.stale).toBeUndefined();
  });

  it('should mark test-gen stale when marking implement — test-gen 为直接 downstream（AC-8）', () => {
    const entries = [
      makePassEntry('proposal', 1),
      makePassEntry('dev-design', 1),
      makePassEntry('test-design', 1),
      makePassEntry('test-gen', 1),
      makePassEntry('implement', 1),
    ];
    markPhaseStale(entries, 'implement', 'requirement');

    expect(entries.find((e) => e.phase === 'implement')!.stale).toBe(true);
    expect(entries.find((e) => e.phase === 'test-gen')!.stale).toBe(true);
    expect(entries.find((e) => e.phase === 'test-design')!.stale).toBeUndefined();
  });

  it('should propagate from implement to test-execution/code-review/acceptance', () => {
    const entries = [];
    const phases: EvalEntry['phase'][] = [
      'proposal',
      'dev-design',
      'test-design',
      'test-gen',
      'implement',
      'test-execution',
      'code-review',
      'acceptance',
    ];
    for (const phase of phases) {
      entries.push(makePassEntry(phase, 1));
    }
    markPhaseStale(entries, 'implement', 'requirement');

    for (const phase of ['implement', 'test-gen', 'test-execution', 'code-review', 'acceptance']) {
      expect(entries.find((e) => e.phase === phase)!.stale).toBe(true);
    }
    expect(entries.find((e) => e.phase === 'test-design')!.stale).toBeUndefined();
  });

  it('should mark test-gen and downstream stale when marking test-design', () => {
    const entries = [
      makePassEntry('proposal', 1),
      makePassEntry('dev-design', 1),
      makePassEntry('test-design', 1),
      makePassEntry('test-gen', 1),
      makePassEntry('implement', 1),
      makePassEntry('test-execution', 1),
    ];
    markPhaseStale(entries, 'test-design', 'requirement');

    for (const phase of ['test-design', 'test-gen', 'test-execution']) {
      expect(entries.find((e) => e.phase === phase)!.stale).toBe(true);
    }
  });

  it('should NOT mark implement stale when marking test-design（AC-9）', () => {
    const entries = [
      makePassEntry('proposal', 1),
      makePassEntry('dev-design', 1),
      makePassEntry('test-design', 1),
      makePassEntry('test-gen', 1),
      makePassEntry('implement', 1),
    ];
    markPhaseStale(entries, 'test-design', 'requirement');

    expect(entries.find((e) => e.phase === 'implement')!.stale).toBeUndefined();
    expect(entries.find((e) => e.phase === 'test-gen')!.stale).toBe(true);
  });
});
