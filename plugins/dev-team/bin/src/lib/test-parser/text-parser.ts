// ---------------------------------------------------------------------------
// Text Test Output Parser
//
// Multi-layer fallback parser for free-text test output:
//
//   Layer 1 — Framework-specific regex:
//     - bun:     [PASS] / [FAIL] / [SKIP] markers
//     - cargo:   "test result: ok" / "test result: FAILED" summary line
//     - node:    "# pass" / "# fail" / "# skip" lines
//     - pytest:  "PASSED" / "FAILED" / "SKIPPED" per-line markers
//
//   Layer 2 — Generic regex:
//     - "Tests: N passed, M failed, S skipped"
//     - "N passed, M failed"
//     - "N tests passed"
//
//   Layer 3 — Line heuristic:
//     - Count lines containing PASS/FAIL/SKIP keywords
// ---------------------------------------------------------------------------

import type { ParsedTestResult, TestCase } from './index';

// ---------------------------------------------------------------------------
// Layer 1: Framework-specific parsers
// ---------------------------------------------------------------------------

function parseBunOutput(lines: string[]): ParsedTestResult | null {
  const testCases: TestCase[] = [];
  let hasBunMarker = false;

  for (const line of lines) {
    const passMatch = line.match(/^(\d+)\s+\[PASS\]\s+(.+)/);
    const failMatch = line.match(/^(\d+)\s+\[FAIL\]\s+(.+)/);
    const skipMatch = line.match(/^(\d+)\s+\[SKIP\]\s+(.+)/);

    if (passMatch) {
      hasBunMarker = true;
      testCases.push({ name: passMatch[2].trim(), status: 'passed' });
    } else if (failMatch) {
      hasBunMarker = true;
      testCases.push({ name: failMatch[2].trim(), status: 'failed' });
    } else if (skipMatch) {
      hasBunMarker = true;
      testCases.push({ name: skipMatch[2].trim(), status: 'skipped' });
    }
  }

  if (!hasBunMarker) return null;

  return buildResult(testCases);
}

function parseCargoOutput(lines: string[]): ParsedTestResult | null {
  // Look for: "test result: ok. N passed; M failed; S ignored; T measured; U filtered out"
  // or:        "test result: FAILED. N passed; M failed; ..."
  for (const line of lines) {
    const match = line.match(
      /^test result:\s+(ok|FAILED)\.\s+(\d+)\s+passed;\s+(\d+)\s+failed;\s+(\d+)\s+ignored/i,
    );
    if (match) {
      const passed = parseInt(match[2], 10);
      const failed = parseInt(match[3], 10);
      const skipped = parseInt(match[4], 10);

      // Extract individual test names from preceding lines
      const testCases: TestCase[] = [];
      for (const l of lines) {
        if (l.includes('test result:')) break; // stop at summary
        const testMatch = l.match(/^test\s+(\S+)\s+\.\.\.\s+(ok|FAILED)/i);
        if (testMatch) {
          testCases.push({
            name: testMatch[1],
            status: testMatch[2].toLowerCase() === 'ok' ? 'passed' : 'failed',
          });
        }
      }

      return {
        total: passed + failed + skipped,
        passed,
        failed,
        skipped,
        testCases,
        testFiles: [],
        sourceFiles: [],
      };
    }
  }

  return null;
}

function parseNodeOutput(lines: string[]): ParsedTestResult | null {
  let passed = 0;
  let failed = 0;
  let skipped = 0;
  let hasNodeMarker = false;
  const testCases: TestCase[] = [];

  for (const line of lines) {
    // Match both TAP (# pass N) and Node.js v24+ native reporter (ℹ pass N / ℹ fail N / ℹ skipped N)
    const passMatch = line.match(/^\s*(?:#|ℹ)\s+pass(?:ed)?\s+(\d+)/i);
    const failMatch = line.match(/^\s*(?:#|ℹ)\s+fail(?:ed)?\s+(\d+)/i);
    const skipMatch = line.match(/^\s*(?:#|ℹ)\s+skip(?:ped)?\s+(\d+)/i);

    if (passMatch) {
      hasNodeMarker = true;
      passed += parseInt(passMatch[1], 10);
    } else if (failMatch) {
      hasNodeMarker = true;
      failed += parseInt(failMatch[1], 10);
    } else if (skipMatch) {
      hasNodeMarker = true;
      skipped += parseInt(skipMatch[1], 10);
    } else {
      // Try to extract individual test names
      const testMatch = line.match(/^(?:ok|not ok)\s+\d+\s+(.+)/);
      if (testMatch) {
        testCases.push({
          name: testMatch[1].trim(),
          status: line.startsWith('ok') ? 'passed' : 'failed',
        });
      }
    }
  }

  if (!hasNodeMarker) return null;

  return {
    total: passed + failed + skipped,
    passed,
    failed,
    skipped,
    testCases,
    testFiles: [],
    sourceFiles: [],
  };
}

function parsePytestOutput(lines: string[]): ParsedTestResult | null {
  const testCases: TestCase[] = [];
  let hasPytestMarker = false;

  for (const line of lines) {
    // Match pytest verbose output: tests/test_foo.py::test_bar PASSED
    // Or: tests/test_foo.py::test_bar FAILED
    // Or: tests/test_foo.py::test_bar SKIPPED
    const testMatch = line.match(/^(.+::.+)\s+(PASSED|FAILED|SKIPPED)/);
    if (testMatch) {
      hasPytestMarker = true;
      testCases.push({
        name: testMatch[1].trim(),
        status: mapPytestStatus(testMatch[2]),
      });
    }
  }

  if (!hasPytestMarker) return null;

  return buildResult(testCases);
}

// ---------------------------------------------------------------------------
// Layer 2: Generic regex fallback
// ---------------------------------------------------------------------------

function parseGenericRegex(output: string): ParsedTestResult | null {
  // Pattern: "Tests: N passed, M failed, S total" or "N passed, M failed"
  const patterns = [
    /Tests:\s+(\d+)\s+passed,\s+(\d+)\s+failed,\s+(\d+)\s+total/i,
    /Tests:\s+(\d+)\s+passed,\s+(\d+)\s+failed/i,
    /(\d+)\s+passed,\s+(\d+)\s+failed/i,
    /(\d+)\s+tests?\s+passed/i,
  ];

  for (const pattern of patterns) {
    const match = output.match(pattern);
    if (match) {
      const passed = parseInt(match[1], 10);
      const failed = match[2] ? parseInt(match[2], 10) : 0;
      const skipped = match[3] ? parseInt(match[3], 10) : 0;
      return {
        total: passed + failed + skipped,
        passed,
        failed,
        skipped,
        testCases: [],
        testFiles: [],
        sourceFiles: [],
      };
    }
  }

  return null;
}

// ---------------------------------------------------------------------------
// Layer 3: Line heuristic
// ---------------------------------------------------------------------------

function parseHeuristic(lines: string[]): ParsedTestResult {
  let passed = 0;
  let failed = 0;
  let skipped = 0;
  const testCases: TestCase[] = [];

  for (const line of lines) {
    if (/PASS/i.test(line) && !/FAIL/i.test(line) && !/PASSED/i.test(line)) {
      // Skip lines that contain PASS but are not actual test output
      continue;
    }

    if (/PASSED/.test(line) || /^ok\b/.test(line)) {
      passed++;
      testCases.push({ name: line.trim(), status: 'passed' });
    } else if (/FAILED/.test(line) || /^not ok\b/.test(line)) {
      failed++;
      testCases.push({ name: line.trim(), status: 'failed' });
    } else if (/SKIP/.test(line)) {
      skipped++;
      testCases.push({ name: line.trim(), status: 'skipped' });
    }
  }

  const total = passed + failed + skipped;
  return {
    total,
    passed,
    failed,
    skipped,
    testCases,
    testFiles: [],
    sourceFiles: [],
  };
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function buildResult(testCases: TestCase[]): ParsedTestResult {
  const passed = testCases.filter((t) => t.status === 'passed').length;
  const failed = testCases.filter((t) => t.status === 'failed').length;
  const skipped = testCases.filter((t) => t.status === 'skipped').length;

  return {
    total: testCases.length,
    passed,
    failed,
    skipped,
    testCases,
    testFiles: [],
    sourceFiles: [],
  };
}

function mapPytestStatus(status: string): 'passed' | 'failed' | 'skipped' {
  switch (status) {
    case 'PASSED':
      return 'passed';
    case 'FAILED':
      return 'failed';
    case 'SKIPPED':
      return 'skipped';
    default:
      return 'passed';
  }
}

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

/**
 * Parse free-text test output using a multi-layer fallback strategy.
 *
 * @param output - The raw text output (stdout or stderr)
 * @returns ParsedTestResult
 */
export function parseTextOutput(output: string): ParsedTestResult {
  if (!output || output.trim().length === 0) {
    return emptyTextResult('Empty output');
  }

  const lines = output.split('\n').filter((l) => l.trim().length > 0);

  // Layer 1: Framework-specific
  const layer1Result = tryFrameworkParsers(lines);
  if (layer1Result) return layer1Result;

  // Layer 2: Generic regex
  const genericResult = parseGenericRegex(output);
  if (genericResult) return genericResult;

  // Layer 3: Heuristic line-by-line
  const heuristicResult = parseHeuristic(lines);
  if (heuristicResult.total === 0) {
    return emptyTextResult('Unable to parse test output');
  }

  return heuristicResult;
}

function emptyTextResult(error: string): ParsedTestResult {
  return {
    total: 0,
    passed: 0,
    failed: 0,
    skipped: 0,
    testCases: [],
    testFiles: [],
    sourceFiles: [],
    error,
  };
}

function tryFrameworkParsers(lines: string[]): ParsedTestResult | null {
  const bunResult = parseBunOutput(lines);
  if (bunResult) return bunResult;

  const cargoResult = parseCargoOutput(lines);
  if (cargoResult) return cargoResult;

  const nodeResult = parseNodeOutput(lines);
  if (nodeResult) return nodeResult;

  const pytestResult = parsePytestOutput(lines);
  if (pytestResult) return pytestResult;

  return null;
}
