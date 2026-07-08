// ---------------------------------------------------------------------------
// JSON Test Output Parser
//
// Parses vitest/jest JSON reporter output.  Both frameworks produce a JSON
// object with a `testResults` array.  Each element contains `assertionResults`
// with individual test-case details.
// ---------------------------------------------------------------------------

/* eslint-disable @typescript-eslint/no-unsafe-type-assertion, @typescript-eslint/no-explicit-any */

import type { ParsedTestResult, TestCase } from './index';

interface VitestAssertionResult {
  title: string;
  fullName: string;
  status: 'passed' | 'failed' | 'skipped';
  duration?: number;
  failureMessages?: string[];
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  meta?: any;
}

interface VitestTestResult {
  assertionResults: VitestAssertionResult[];
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  [key: string]: any;
}

interface VitestJsonOutput {
  testResults: VitestTestResult[];
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  [key: string]: any;
}

/**
 * Attempt to parse a vitest/jest JSON reporter output.
 *
 * @param stdout - The raw stdout from the test command
 * @returns ParsedTestResult
 */
export function parseJsonOutput(stdout: string): ParsedTestResult {
  if (!stdout || stdout.trim().length === 0) {
    return emptyJsonResult('Empty stdout');
  }

  let parsed: VitestJsonOutput;
  try {
    parsed = JSON.parse(stdout) as VitestJsonOutput;
  } catch {
    return emptyJsonResult('Failed to parse JSON output');
  }

  if (!parsed || typeof parsed !== 'object') {
    return emptyJsonResult('Parsed JSON is not an object');
  }

  const testResults = parsed.testResults;
  if (!Array.isArray(testResults)) {
    return emptyJsonResult('Missing testResults array');
  }

  const { testCases, testFiles } = collectJsonTestCases(testResults);
  const sourceFiles = deriveSourceFiles(testFiles);

  const total = testCases.length;
  const passed = testCases.filter((t) => t.status === 'passed').length;
  const failed = testCases.filter((t) => t.status === 'failed').length;
  const skipped = testCases.filter((t) => t.status === 'skipped').length;

  return {
    total,
    passed,
    failed,
    skipped,
    testCases,
    testFiles,
    sourceFiles,
  };
}

function emptyJsonResult(error: string): ParsedTestResult {
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

function collectJsonTestCases(testResults: VitestTestResult[]): {
  testCases: TestCase[];
  testFiles: string[];
} {
  const testCases: TestCase[] = [];
  const testFiles: string[] = [];

  for (const result of testResults) {
    const assertionResults = result.assertionResults;
    if (!Array.isArray(assertionResults)) continue;

    for (const assertion of assertionResults) {
      testCases.push(mapAssertionToTestCase(assertion));
    }

    if (typeof result.name === 'string' && result.name.length > 0) {
      testFiles.push(result.name);
    }
  }

  return { testCases, testFiles };
}

/**
 * Map a JSON assertion result to our TestCase interface.
 */
function mapAssertionToTestCase(assertion: VitestAssertionResult): TestCase {
  const testCase: TestCase = {
    name: assertion.fullName || assertion.title || 'unknown',
    status: assertion.status || 'passed',
  };

  if (typeof assertion.duration === 'number') {
    testCase.durationMs = assertion.duration;
  }

  if (assertion.failureMessages && assertion.failureMessages.length > 0) {
    const firstMsg = assertion.failureMessages[0];
    if (typeof firstMsg === 'string') {
      // Extract error type (first line before colon/newline)
      const newlineIdx = firstMsg.indexOf('\n');
      const firstLine = newlineIdx >= 0 ? firstMsg.slice(0, newlineIdx) : firstMsg;
      const colonIdx = firstLine.indexOf(':');
      if (colonIdx >= 0) {
        testCase.errorType = firstLine.slice(0, colonIdx).trim();
        testCase.errorMessage = firstLine.slice(colonIdx + 1).trim();
      } else {
        testCase.errorType = 'Error';
        testCase.errorMessage = firstLine;
      }
      testCase.stackTrace = firstMsg;
    }
  }

  return testCase;
}

/**
 * Derive source file paths from test file paths.
 *
 * Convention: remove `.test.` / `.spec.` / `_test.` / `test_` markers to
 * get the corresponding source file.
 */
function deriveSourceFiles(testFiles: string[]): string[] {
  const sourceSet = new Set<string>();

  for (const tf of testFiles) {
    const posix = tf.replace(/\\/g, '/');
    if (tf.includes('__tests__/')) {
      continue;
    }
    // Strip .test. or .spec. suffix patterns
    const src = posix
      .replace(/\.test\./g, '.')
      .replace(/\.spec\./g, '.')
      .replace(/_test\.go$/, '.go')
      .replace(/^test_(.+)\.py$/, '$1.py');

    if (src !== posix) {
      sourceSet.add(src);
    }
  }

  return Array.from(sourceSet).sort();
}
