// ---------------------------------------------------------------------------
// Test Framework Registry
//
// Hardcoded mapping of known test frameworks to their test and coverage
// commands.  Framework commands are implementation details managed by the
// plugin maintainer, NOT by project config.json (see design decision D1).
// ---------------------------------------------------------------------------

import { type TestFrameworks } from '../schemas';

export interface FrameworkConfig {
  framework: TestFrameworks;
  test_cmd: string;
  coverage_format: 'istanbul' | 'llvm-cov' | 'node-test' | 'go-cover' | 'coverage-py';
  coverage_output: string;
  coverage_artifacts: string[];
  coverage_cleanup: string[];
  default_glob: string;
  mutation_framework: string | null;
}

const FRAMEWORK_REGISTRY: Record<TestFrameworks, FrameworkConfig> = {
  jest: {
    framework: 'jest',
    test_cmd:
      'npx jest --randomize --verbose --json --coverage --coverageReporters=json-summary {files}',
    coverage_format: 'istanbul',
    coverage_output: 'coverage/coverage-summary.json',
    coverage_artifacts: ['coverage/coverage-summary.json'],
    coverage_cleanup: ['coverage', '.nyc_output'],
    default_glob: '**/*.{test,spec}.{js,ts,jsx,tsx}',
    mutation_framework: 'stryker-js',
  },
  vitest: {
    framework: 'vitest',
    test_cmd:
      'npx vitest run --sequence.shuffle --reporter=json --coverage --coverage.reporter=json-summary {files}',
    coverage_format: 'istanbul',
    coverage_output: 'coverage/coverage-summary.json',
    coverage_artifacts: ['coverage/coverage-summary.json'],
    coverage_cleanup: ['coverage', '.nyc_output', 'test-stderr.txt'],
    default_glob: '**/*.{test,spec}.{js,ts,jsx,tsx}',
    mutation_framework: 'stryker-js',
  },
  'vite-plus': {
    framework: 'vite-plus',
    test_cmd: 'vp test --sequence.shuffle --coverage --coverage.reporter=json-summary {files}',
    coverage_format: 'istanbul',
    coverage_output: 'coverage/coverage-summary.json',
    coverage_artifacts: ['coverage/coverage-summary.json'],
    coverage_cleanup: ['coverage', '.nyc_output', 'test-stderr.txt'],
    default_glob: '**/*.{test,spec}.{js,ts,jsx,tsx}',
    mutation_framework: 'stryker-js',
  },
  bun: {
    framework: 'bun',
    test_cmd: 'bun test --coverage --coverageReporters=json-summary {files}',
    coverage_format: 'istanbul',
    coverage_output: 'coverage/coverage-summary.json',
    coverage_artifacts: ['coverage/coverage-summary.json'],
    coverage_cleanup: ['coverage'],
    default_glob: '**/*.{test,spec}.{js,ts,jsx,tsx}',
    mutation_framework: null,
  },
  rust: {
    framework: 'rust',
    test_cmd:
      'cargo test; _X=$?; cargo llvm-cov --json --output-path coverage/coverage-summary.json; exit $_X',
    coverage_format: 'llvm-cov',
    coverage_output: 'coverage/coverage-summary.json',
    coverage_artifacts: ['coverage/coverage-summary.json'],
    coverage_cleanup: ['coverage', 'target/llvm-cov'],
    default_glob: '**/tests/**/*.rs',
    mutation_framework: null,
  },
  'node-test': {
    framework: 'node-test',
    test_cmd: 'node --test --experimental-test-coverage {files}',
    coverage_format: 'node-test',
    coverage_output: 'coverage/node-test-output.txt',
    coverage_artifacts: ['coverage/node-test-output.txt'],
    coverage_cleanup: ['coverage'],
    default_glob: '**/*.test.{mjs,js,cjs}',
    mutation_framework: null,
  },
  go: {
    framework: 'go',
    test_cmd: 'go test -json -coverprofile=coverage.out -covermode=atomic {directory}',
    coverage_format: 'go-cover',
    coverage_output: 'coverage/func-summary.txt',
    coverage_artifacts: ['coverage/func-summary.txt'],
    coverage_cleanup: ['coverage', 'coverage.out'],
    default_glob: '**/*_test.go',
    mutation_framework: null,
  },
  pytest: {
    framework: 'pytest',
    test_cmd:
      'pytest -v {files}; _X=$?; pytest --cov=. --cov-report=json --cov-branch -q; exit $_X',
    coverage_format: 'coverage-py',
    coverage_output: 'coverage.json',
    coverage_artifacts: ['coverage.json'],
    coverage_cleanup: ['.coverage', 'htmlcov'],
    default_glob: '**/test_*.py',
    mutation_framework: null,
  },
};

function isTestFramework(framework: string): framework is TestFrameworks {
  return framework in FRAMEWORK_REGISTRY;
}

// ---------------------------------------------------------------------------
// Public interface
// ---------------------------------------------------------------------------

/**
 * Look up a framework's test and coverage command configuration from the
 * hardcoded registry.
 *
 * @param framework - The framework name to look up.
 * @returns The framework configuration object.
 * @throws {Error} If the framework name is not in the registry.
 */
export function getFrameworkConfig(framework: string): FrameworkConfig {
  if (!isTestFramework(framework)) {
    throw new Error(
      `Unknown framework "${framework}". Supported frameworks: ${Object.keys(FRAMEWORK_REGISTRY).join(', ')}`,
    );
  }

  const entry = FRAMEWORK_REGISTRY[framework];
  return { ...entry };
}
