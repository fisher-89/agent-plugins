// ---------------------------------------------------------------------------
// JSON Test Output Parser
//
// Parses vitest/jest JSON reporter output.  Both frameworks produce a JSON
// object with a `testResults` array.  Each element contains `assertionResults`
// with individual test-case details.
// ---------------------------------------------------------------------------

import { z } from 'zod';

import type { ParsedTestResult, TestCase } from './index';

// ---------------------------------------------------------------------------
// Zod schemas
// ---------------------------------------------------------------------------

const vitestAssertionResultSchema = z.object({
  title: z.string().nullable(),
  fullName: z.string().nullable(),
  status: z.enum(['passed', 'failed', 'skipped']),
  duration: z.number().optional(),
  failureMessages: z.array(z.string()).optional(),
});

const vitestTestResultSchema = z.object({
  assertionResults: z.array(vitestAssertionResultSchema),
  name: z.string().optional(),
});

const vitestJsonOutputSchema = z.object({
  testResults: z.array(vitestTestResultSchema),
});

type VitestAssertionResult = z.infer<typeof vitestAssertionResultSchema>;
type VitestTestResult = z.infer<typeof vitestTestResultSchema>;

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

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

  let json: unknown;
  try {
    json = JSON.parse(stdout);
  } catch {
    return emptyJsonResult('Failed to parse JSON output');
  }

  if (json === null || typeof json !== 'object') {
    return emptyJsonResult('Parsed JSON is not an object');
  }

  const parseResult = vitestJsonOutputSchema.safeParse(json);
  if (!parseResult.success) {
    return emptyJsonResult('Missing testResults array');
  }

  const { testResults } = parseResult.data;
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
    for (const assertion of result.assertionResults) {
      testCases.push(mapAssertionToTestCase(assertion));
    }

    if (result.name && result.name.length > 0) {
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
    status: assertion.status,
  };

  if (typeof assertion.duration === 'number') {
    testCase.durationMs = assertion.duration;
  }

  if (assertion.failureMessages && assertion.failureMessages.length > 0) {
    const firstMsg = assertion.failureMessages[0];
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
