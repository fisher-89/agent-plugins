// ---------------------------------------------------------------------------
// Go Test Output Parser
//
// Parses `go test -json` line-delimited JSON output.  Each line is a JSON
// object with an `Action` field.  Only lines with Action "pass", "fail", or
// "skip" are counted as terminal test-case results.  Lines with Action
// "output" or "run" are ignored.
// ---------------------------------------------------------------------------

import { z } from 'zod';

import type { ParsedTestResult, TestCase } from './types';

// ---------------------------------------------------------------------------
// Zod schema
// ---------------------------------------------------------------------------

const goTestEventSchema = z.object({
  Action: z.string(),
  Test: z.string().optional(),
  Elapsed: z.number().optional(),
  Output: z.string().optional(),
  Package: z.string().optional(),
});

type GoTestEvent = z.infer<typeof goTestEventSchema>;

/**
 * Map Go test event Action to our TestCase status.
 * Go uses "pass"/"fail"/"skip" but our type uses "passed"/"failed"/"skipped".
 */
function mapGoStatus(action: string): 'passed' | 'failed' | 'skipped' {
  switch (action) {
    case 'pass':
      return 'passed';
    case 'fail':
      return 'failed';
    case 'skip':
      return 'skipped';
    default:
      return 'passed';
  }
}

/**
 * Parse `go test -json` line-delimited JSON output.
 *
 * Accepts file contents from planDir/results.ndjson (or equivalent NDJSON
 * text). Execute reads the plan directory file via parsePlanArtifacts.
 *
 * @param content - Raw NDJSON string
 * @returns ParsedTestResult
 */
export function parseGoOutput(content: string): ParsedTestResult {
  if (!content || content.trim().length === 0) {
    return emptyGoResult('Empty results content');
  }

  const lines = content.split('\n').filter((l) => l.trim().length > 0);
  const testMap = collectGoTestEvents(lines);

  const testCases = Array.from(testMap.values());
  const testFiles = deriveGoTestFiles(testCases);
  const sourceFiles = deriveGoSourceFiles(testFiles);

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

function emptyGoResult(error: string): ParsedTestResult {
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

function collectGoTestEvents(lines: string[]): Map<string, TestCase> {
  const testMap = new Map<string, TestCase>();
  const seenTests = new Set<string>();

  for (const line of lines) {
    let json: unknown;
    try {
      json = JSON.parse(line);
    } catch {
      continue;
    }

    const parseResult = goTestEventSchema.safeParse(json);
    if (!parseResult.success) continue;

    const event: GoTestEvent = parseResult.data;
    if (!event.Test) continue;

    if (event.Action === 'pass' || event.Action === 'fail' || event.Action === 'skip') {
      const existing = testMap.get(event.Test);
      if (existing && event.Action === 'pass' && existing.status === 'failed') {
        continue;
      }
      testMap.set(event.Test, {
        name: event.Test,
        status: mapGoStatus(event.Action),
        durationMs: event.Elapsed ? Math.round(event.Elapsed * 1000) : undefined,
      });
      seenTests.add(event.Test);
    } else if (event.Action === 'output' && seenTests.has(event.Test)) {
      const existing = testMap.get(event.Test);
      if (existing && existing.status === 'failed' && event.Output) {
        existing.errorMessage = existing.errorMessage
          ? existing.errorMessage + event.Output
          : event.Output;
      }
    }
  }

  return testMap;
}

function deriveGoTestFiles(testCases: TestCase[]): string[] {
  const testFiles: string[] = [];
  for (const tc of testCases) {
    const match = tc.name.match(/^Test([A-Z].*)/);
    if (match) {
      const snakeCase = match[1]
        .replace(/([A-Z])/g, '_$1')
        .toLowerCase()
        .replace(/^_/, '');
      if (snakeCase) {
        testFiles.push(`${snakeCase}_test.go`);
      }
    }
  }
  return testFiles;
}

function deriveGoSourceFiles(testFiles: string[]): string[] {
  const sourceSet = new Set<string>();

  for (const tf of testFiles) {
    const posix = tf.replace(/\\/g, '/');
    if (tf.includes('__tests__/')) {
      continue;
    }
    // Strip .test. or .spec. suffix patterns
    const src = posix.replace(/_test\.go$/, '.go');

    if (src !== posix) {
      sourceSet.add(src);
    }
  }

  return Array.from(sourceSet).sort();
}
