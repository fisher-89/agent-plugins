// test_eval_json_unit.test.mjs — Unit tests for eval-json.ts (eval.json read/write/validate)
//
// Tests:
//   - readEvalJson(): file not found returns [], invalid JSON throws, valid file parses
//   - buildEntry(): auto-fills timestamp and schema_version, handles backtrack_to
//   - computeAttempt(): auto-count across entries, explicit override, cross-phase isolation
//   - checkGate(): all prior pass -> pass, some fail -> fail, empty prior -> pass
//   - validateVerdict(): accepts "pass"/"fail", rejects other values
//   - validateReportLength(): accepts <=500, rejects >500
//   - validateItemsJson(): valid JSON array parses, invalid JSON throws
//   - appendEntry(): mkdir -p behavior, append-only, 2-space indent, trailing newline
//
// Usage:
//   node --test test_eval_json_unit.test.mjs

import { describe, it, mock } from 'node:test';
import assert from 'node:assert/strict';
import path from 'node:path';
import fs from 'node:fs';

// ---------------------------------------------------------------------------
// Import the module under test.
// ---------------------------------------------------------------------------

// For testing compiled JS:
// import { readEvalJson, buildEntry, computeAttempt, checkGate,
//          validateVerdict, validateReportLength, validateItemsJson,
//          appendEntry } from '../../../../plugins/dev-team/bin/dist/lib/eval-json.js';

// For inline stubs (use when source does not yet exist):
// ============================================================================
const SCHEMA_VERSION = '1.0';

function validateVerdict(verdict) {
  if (verdict !== 'pass' && verdict !== 'fail') {
    throw new Error(`Invalid verdict: "${verdict}". Must be "pass" or "fail".`);
  }
}

function validateReportLength(report) {
  if (typeof report !== 'string') {
    throw new Error('Report must be a string.');
  }
  if (report.length > 500) {
    throw new Error(`Report exceeds 500 character limit (${report.length} chars). Consider truncating.`);
  }
}

function validateItemsJson(itemsStr) {
  if (typeof itemsStr !== 'string') {
    throw new Error('Items must be a JSON string.');
  }
  try {
    const parsed = JSON.parse(itemsStr);
    if (!Array.isArray(parsed)) {
      throw new Error('Items must be a JSON array, e.g., \'[{"item_id":"R1","pass":true}]\'.');
    }
    return parsed;
  } catch (err) {
    if (err instanceof SyntaxError) {
      throw new Error(`Items is not valid JSON: ${err.message}. Ensure items is a JSON array string, e.g. --items '[{"item_id":"R1","pass":true}]'`);
    }
    throw err;
  }
}

function readEvalJson(phasesDir) {
  const filePath = path.join(phasesDir, 'eval.json');
  try {
    if (!fs.existsSync(filePath)) {
      return [];
    }
    const raw = fs.readFileSync(filePath, 'utf-8');
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) {
      throw new Error(`eval.json is not a JSON array: ${filePath}`);
    }
    return parsed;
  } catch (err) {
    if (err instanceof SyntaxError) {
      throw new Error(`Failed to parse eval.json at ${filePath}: ${err.message}`);
    }
    throw err;
  }
}

function buildEntry({ phase, verdict, report, items, attempt, backtrack_to }) {
  const entry = {
    phase,
    timestamp: new Date().toISOString(),
    attempt,
    verdict,
    report,
    items,
    backtrack_to: backtrack_to !== undefined ? backtrack_to : null,
    schema_version: SCHEMA_VERSION,
  };
  return entry;
}

function computeAttempt(entries, phase, explicitAttempt) {
  if (explicitAttempt !== undefined) {
    return explicitAttempt;
  }
  const phaseEntries = entries.filter(e => e.phase === phase);
  return phaseEntries.length + 1;
}

function checkGate(entries, priorPhases) {
  const missing = [];
  for (const p of priorPhases) {
    const hasPass = entries.some(e => e.phase === p && e.verdict === 'pass');
    if (!hasPass) {
      missing.push(p);
    }
  }
  return { passed: missing.length === 0, missing };
}

// ============================================================================
// readEvalJson()
// ============================================================================

describe('readEvalJson()', () => {
  it('should return empty array when eval.json does not exist', () => {
    // TODO: implement with mock fs or temp dir
    // const result = readEvalJson('/nonexistent/path');
    // assert.deepEqual(result, []);
  });

  it('should throw when eval.json contains invalid JSON', () => {
    // TODO: implement with mock fs or temp dir
    // assert.throws(() => readEvalJson(tempDir), /Failed to parse eval.json/);
  });

  it('should parse valid eval.json and return array of entries', () => {
    // TODO: implement with mock fs or temp dir
  });

  it('should throw when eval.json root is not an array', () => {
    // TODO: implement with mock fs or temp dir
    // assert.throws(() => readEvalJson(tempDir), /not a JSON array/);
  });
});

// ============================================================================
// buildEntry()
// ============================================================================

describe('buildEntry()', () => {
  it('should auto-fill timestamp field with ISO 8601 string', () => {
    const entry = buildEntry({
      phase: '01-requirements',
      verdict: 'pass',
      report: 'All pass.',
      items: [],
      attempt: 1,
    });
    assert.ok(entry.timestamp);
    // ISO 8601 format: YYYY-MM-DDTHH:mm:ss.sssZ
    assert.match(entry.timestamp, /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/);
  });

  it('should auto-fill schema_version as "1.0"', () => {
    const entry = buildEntry({
      phase: '01-requirements',
      verdict: 'pass',
      report: 'All pass.',
      items: [],
      attempt: 1,
    });
    assert.strictEqual(entry.schema_version, '1.0');
  });

  it('should set backtrack_to to null when not provided', () => {
    const entry = buildEntry({
      phase: '01-requirements',
      verdict: 'pass',
      report: 'All pass.',
      items: [],
      attempt: 1,
    });
    assert.strictEqual(entry.backtrack_to, null);
  });

  it('should set backtrack_to to provided value', () => {
    const entry = buildEntry({
      phase: '03-dev-proposal',
      verdict: 'pass',
      report: 'Approved.',
      items: [],
      attempt: 1,
      backtrack_to: '02-test-design',
    });
    assert.strictEqual(entry.backtrack_to, '02-test-design');
  });

  it('should include all provided fields in output', () => {
    const entry = buildEntry({
      phase: '01-requirements',
      verdict: 'pass',
      report: 'All checklist items pass.',
      items: [{ item_id: 'R1', pass: true, evidence: 'done', notes: '' }],
      attempt: 1,
    });
    assert.strictEqual(entry.phase, '01-requirements');
    assert.strictEqual(entry.verdict, 'pass');
    assert.strictEqual(entry.report, 'All checklist items pass.');
    assert.strictEqual(entry.attempt, 1);
    assert.strictEqual(entry.items.length, 1);
    assert.strictEqual(entry.items[0].item_id, 'R1');
  });
});

// ============================================================================
// computeAttempt()
// ============================================================================

describe('computeAttempt()', () => {
  const sampleEntries = [
    { phase: '01-requirements', attempt: 1, verdict: 'fail' },
    { phase: '01-requirements', attempt: 2, verdict: 'pass' },
    { phase: '02-test-design', attempt: 1, verdict: 'pass' },
  ];

  it('should return 1 when entries array is empty', () => {
    assert.strictEqual(computeAttempt([], '01-requirements'), 1);
  });

  it('should return entries.length + 1 when counting same phase', () => {
    assert.strictEqual(computeAttempt(sampleEntries, '01-requirements'), 3);
  });

  it('should return 1 when current phase has no existing entries', () => {
    assert.strictEqual(computeAttempt(sampleEntries, '03-dev-proposal'), 1);
  });

  it('should not count entries from other phases', () => {
    const result = computeAttempt(sampleEntries, '02-test-design');
    assert.strictEqual(result, 2); // 1 existing + 1
  });

  it('should return explicit attempt value when provided', () => {
    assert.strictEqual(computeAttempt(sampleEntries, '01-requirements', 5), 5);
  });

  it('should return explicit attempt even if entries array is empty', () => {
    assert.strictEqual(computeAttempt([], '01-requirements', 99), 99);
  });
});

// ============================================================================
// checkGate()
// ============================================================================

describe('checkGate()', () => {
  const passEntries = [
    { phase: '01-requirements', verdict: 'pass' },
    { phase: '02-test-design', verdict: 'pass' },
  ];

  const mixedEntries = [
    { phase: '01-requirements', verdict: 'pass' },
    { phase: '02-test-design', verdict: 'fail' },
  ];

  const failEntries = [
    { phase: '01-requirements', verdict: 'fail' },
  ];

  it('should return passed=true when empty prior phases (first phase)', () => {
    const result = checkGate(passEntries, []);
    assert.strictEqual(result.passed, true);
    assert.deepEqual(result.missing, []);
  });

  it('should return passed=true when all prior phases have pass verdict', () => {
    const result = checkGate(passEntries, ['01-requirements', '02-test-design']);
    assert.strictEqual(result.passed, true);
    assert.deepEqual(result.missing, []);
  });

  it('should return passed=false and list missing phases when absent', () => {
    const result = checkGate(passEntries, ['01-requirements', '02-test-design', '03-dev-proposal']);
    assert.strictEqual(result.passed, false);
    assert.deepEqual(result.missing, ['03-dev-proposal']);
  });

  it('should return passed=false when prior phase has only fail verdict', () => {
    const result = checkGate(failEntries, ['01-requirements']);
    assert.strictEqual(result.passed, false);
    assert.deepEqual(result.missing, ['01-requirements']);
  });

  it('should return passed=false when one prior phase fails among multiple', () => {
    const result = checkGate(mixedEntries, ['01-requirements', '02-test-design']);
    assert.strictEqual(result.passed, false);
    assert.deepEqual(result.missing, ['02-test-design']);
  });

  it('should return passed=true when prior phase has both fail and pass entries', () => {
    const entries = [
      { phase: '01-requirements', verdict: 'fail' },
      { phase: '01-requirements', verdict: 'pass' },
    ];
    const result = checkGate(entries, ['01-requirements']);
    assert.strictEqual(result.passed, true);
    assert.deepEqual(result.missing, []);
  });

  it('should return passed=false when all prior phases are missing', () => {
    const result = checkGate([], ['01-requirements', '02-test-design']);
    assert.strictEqual(result.passed, false);
    assert.deepEqual(result.missing, ['01-requirements', '02-test-design']);
  });
});

// ============================================================================
// validateVerdict()
// ============================================================================

describe('validateVerdict()', () => {
  it('should accept "pass"', () => {
    assert.doesNotThrow(() => validateVerdict('pass'));
  });

  it('should accept "fail"', () => {
    assert.doesNotThrow(() => validateVerdict('fail'));
  });

  it('should reject empty string', () => {
    assert.throws(() => validateVerdict(''), /Invalid verdict/);
  });

  it('should reject unknown verdict value', () => {
    assert.throws(() => validateVerdict('unknown'), /Invalid verdict/);
  });

  it('should reject boolean', () => {
    assert.throws(() => validateVerdict(true), /Invalid verdict/);
  });

  it('should reject null', () => {
    assert.throws(() => validateVerdict(null), /Invalid verdict/);
  });

  it('should reject uppercase "PASS"', () => {
    assert.throws(() => validateVerdict('PASS'), /Invalid verdict/);
  });
});

// ============================================================================
// validateReportLength()
// ============================================================================

describe('validateReportLength()', () => {
  it('should accept empty report string', () => {
    assert.doesNotThrow(() => validateReportLength(''));
  });

  it('should accept 500 character report', () => {
    const report = 'a'.repeat(500);
    assert.doesNotThrow(() => validateReportLength(report));
  });

  it('should reject 501 character report', () => {
    const report = 'a'.repeat(501);
    assert.throws(() => validateReportLength(report), /exceeds 500 character limit/);
  });

  it('should accept short report (1 char)', () => {
    assert.doesNotThrow(() => validateReportLength('x'));
  });

  it('should reject non-string input (number)', () => {
    assert.throws(() => validateReportLength(123), /Report must be a string/);
  });
});

// ============================================================================
// validateItemsJson()
// ============================================================================

describe('validateItemsJson()', () => {
  it('should parse valid JSON array string', () => {
    const result = validateItemsJson('[{"item_id":"R1","pass":true}]');
    assert.ok(Array.isArray(result));
    assert.strictEqual(result.length, 1);
    assert.strictEqual(result[0].item_id, 'R1');
  });

  it('should parse empty JSON array', () => {
    const result = validateItemsJson('[]');
    assert.ok(Array.isArray(result));
    assert.strictEqual(result.length, 0);
  });

  it('should reject JSON object (not array)', () => {
    assert.throws(() => validateItemsJson('{"key":"value"}'), /Items must be a JSON array/);
  });

  it('should reject malformed JSON string', () => {
    assert.throws(() => validateItemsJson('{bad json}'), /not valid JSON/);
  });

  it('should reject non-JSON plain string', () => {
    assert.throws(() => validateItemsJson('not-json-at-all'), /not valid JSON/);
  });

  it('should reject null input', () => {
    assert.throws(() => validateItemsJson(null), /must be a JSON string/);
  });
});

// ============================================================================
// appendEntry() — requires real filesystem or mock
// ============================================================================

describe('appendEntry()', () => {
  it('should create phases directory if it does not exist', () => {
    // TODO: implement with temp directory or mock fs
  });

  it('should create eval.json file if it does not exist', () => {
    // TODO: implement with temp directory or mock fs
  });

  it('should append entry to existing eval.json array', () => {
    // TODO: implement with temp directory or mock fs
  });

  it('should preserve existing entries when appending', () => {
    // TODO: implement with temp directory or mock fs
  });

  it('should write with 2-space indentation', () => {
    // TODO: implement with temp directory or mock fs
  });

  it('should write with trailing newline', () => {
    // TODO: implement with temp directory or mock fs
  });

  it('should throw when eval.json contains non-array data', () => {
    // TODO: implement with temp directory or mock fs
  });
});
