// ---------------------------------------------------------------------------
// Test Framework Registry
//
// Hardcoded mapping of known test frameworks to their test and coverage
// commands.  Framework commands are implementation details managed by the
// plugin maintainer, NOT by project config.json (see design decision D1).
// ---------------------------------------------------------------------------

import { type TestFramework } from '../schemas';
import { execCommand } from './exec-command';

/** Build a test_execution command template from a detected framework version. */
type TestExecutionBuilder = (version: string) => string;

export interface FrameworkConfig {
  framework: TestFramework;
  /** Command run in the suite cwd to detect the framework version (usually `--version`). */
  version_command: string;
  shell: {
    test_execution: TestExecutionBuilder;
    coverage_cleanup: string[];
    mutation_execution?: string;
  };
  cmd: {
    test_execution: TestExecutionBuilder;
    coverage_cleanup: string[];
    mutation_execution?: string;
  };
  coverage_format: 'istanbul' | 'llvm-cov' | 'node-test' | 'go-cover' | 'coverage-py';
  coverage_output: string;
  default_glob: string;
  mutation_framework: string | null;
  /** CLI flag for injecting a framework config file; null when unsupported. */
  config_flag: string | null;
}

const FRAMEWORK_REGISTRY: Record<TestFramework, FrameworkConfig> = {
  jest: {
    framework: 'jest',
    version_command: 'npx jest --version',
    shell: {
      test_execution: (version) =>
        `npx jest${isVersionAtLeast(version, '29.5.0') ? ' --randomize' : ''} --no-verbose --json --silent --coverage --coverageReporters="json-summary" {config_args} {files}`,
      coverage_cleanup: ['coverage', '.nyc_output'],
      mutation_execution: 'npx stryker run "{config}"',
    },
    cmd: {
      test_execution: (version) =>
        `npx jest${isVersionAtLeast(version, '29.5.0') ? ' --randomize' : ''} --no-verbose --json --silent --coverage --coverageReporters="json-summary" {config_args} {files}`,
      coverage_cleanup: ['coverage', '.nyc_output'],
      mutation_execution: 'npx stryker run "{config}"',
    },
    coverage_format: 'istanbul',
    coverage_output: 'coverage/coverage-summary.json',
    default_glob: '**/*.{test,spec}.{js,ts,jsx,tsx}',
    mutation_framework: 'stryker-js',
    config_flag: '--config',
  },
  vitest: {
    framework: 'vitest',
    version_command: 'npx vitest --version',
    shell: {
      test_execution: () =>
        'npx vitest run --sequence.shuffle --reporter=json --silent --coverage --coverage.reporter=json-summary {config_args} {files}',
      coverage_cleanup: ['coverage', '.nyc_output', 'test-stderr.txt'],
      mutation_execution: 'npx stryker run "{config}"',
    },
    cmd: {
      test_execution: () =>
        'npx vitest run --sequence.shuffle --reporter=json --silent --coverage --coverage.reporter=json-summary {config_args} {files}',
      coverage_cleanup: ['coverage', '.nyc_output', 'test-stderr.txt'],
      mutation_execution: 'npx stryker run "{config}"',
    },
    coverage_format: 'istanbul',
    coverage_output: 'coverage/coverage-summary.json',
    default_glob: '**/*.{test,spec}.{js,ts,jsx,tsx}',
    mutation_framework: 'stryker-js',
    config_flag: '--config',
  },
  'vite-plus': {
    framework: 'vite-plus',
    version_command: 'vp --version',
    shell: {
      test_execution: () =>
        'vp test --sequence.shuffle --reporter=json --silent --coverage --coverage.reporter=json-summary {config_args} {files}',
      coverage_cleanup: ['coverage', '.nyc_output', 'test-stderr.txt'],
      mutation_execution: 'npx stryker run "{config}"',
    },
    cmd: {
      test_execution: () =>
        'vp test --sequence.shuffle --reporter=json --silent --coverage --coverage.reporter=json-summary {config_args} {files}',
      coverage_cleanup: ['coverage', '.nyc_output', 'test-stderr.txt'],
      mutation_execution: 'npx stryker run "{config}"',
    },
    coverage_format: 'istanbul',
    coverage_output: 'coverage/coverage-summary.json',
    default_glob: '**/*.{test,spec}.{js,ts,jsx,tsx}',
    mutation_framework: 'stryker-js',
    config_flag: '--config',
  },
  bun: {
    framework: 'bun',
    version_command: 'bun --version',
    shell: {
      test_execution: () => 'bun test --coverage --coverageReporters=json-summary {files}',
      coverage_cleanup: ['coverage'],
    },
    cmd: {
      test_execution: () => 'bun test --coverage --coverageReporters=json-summary {files}',
      coverage_cleanup: ['coverage'],
    },
    coverage_format: 'istanbul',
    coverage_output: 'coverage/coverage-summary.json',
    default_glob: '**/*.{test,spec}.{js,ts,jsx,tsx}',
    mutation_framework: null,
    config_flag: null,
  },
  rust: {
    framework: 'rust',
    version_command: 'cargo --version',
    shell: {
      test_execution: () =>
        'cargo test; _X=$?; cargo llvm-cov --json --output-path coverage/coverage-summary.json; exit $_X',
      coverage_cleanup: ['coverage', 'target/llvm-cov'],
    },
    cmd: {
      test_execution: () =>
        'cargo test & if errorlevel 1 set _X=%errorlevel% & cargo llvm-cov --json --output-path coverage/coverage-summary.json & exit /b %_X%',
      coverage_cleanup: ['coverage', 'target/llvm-cov'],
    },
    coverage_format: 'llvm-cov',
    coverage_output: 'coverage/coverage-summary.json',
    default_glob: '**/tests/**/*.rs',
    mutation_framework: null,
    config_flag: null,
  },
  'node-test': {
    framework: 'node-test',
    version_command: 'node --version',
    shell: {
      test_execution: () => 'node --test --experimental-test-coverage {files}',
      coverage_cleanup: ['coverage'],
    },
    cmd: {
      test_execution: () => 'node --test --experimental-test-coverage {files}',
      coverage_cleanup: ['coverage'],
    },
    coverage_format: 'node-test',
    coverage_output: 'coverage/node-test-output.txt',
    default_glob: '**/*.test.{mjs,js,cjs}',
    mutation_framework: null,
    config_flag: null,
  },
  go: {
    framework: 'go',
    version_command: 'go version',
    shell: {
      test_execution: () =>
        'go test -json -coverprofile=coverage.out -covermode=atomic {directory}',
      coverage_cleanup: ['coverage', 'coverage.out'],
    },
    cmd: {
      test_execution: () =>
        'go test -json -coverprofile=coverage.out -covermode=atomic {directory}',
      coverage_cleanup: ['coverage', 'coverage.out'],
    },
    coverage_format: 'go-cover',
    coverage_output: 'coverage/func-summary.txt',
    default_glob: '**/*_test.go',
    mutation_framework: null,
    config_flag: null,
  },
  pytest: {
    framework: 'pytest',
    version_command: 'pytest --version',
    shell: {
      test_execution: () => 'pytest -v {files}; pytest --cov=. --cov-report=json --cov-branch -q',
      coverage_cleanup: ['.coverage', 'htmlcov'],
    },
    cmd: {
      test_execution: () => 'pytest -v {files} && pytest --cov=. --cov-report=json --cov-branch -q',
      coverage_cleanup: ['.coverage', 'htmlcov'],
    },
    coverage_format: 'coverage-py',
    coverage_output: 'coverage.json',
    default_glob: '**/test_*.py',
    mutation_framework: null,
    config_flag: null,
  },
};

function isTestFramework(framework: string): framework is TestFramework {
  return framework in FRAMEWORK_REGISTRY;
}

/**
 * Extract the first `major.minor.patch` semver found in command output.
 */
function extractSemver(text: string): string | null {
  const match = text.match(/(\d+)\.(\d+)\.(\d+)/);
  return match ? `${match[1]}.${match[2]}.${match[3]}` : null;
}

/**
 * Compare two semver strings (major.minor.patch). Returns false when either
 * side cannot be parsed — callers treat unknown versions as "feature unsupported".
 */
function isVersionAtLeast(version: string, minimum: string): boolean {
  const a = extractSemver(version);
  const b = extractSemver(minimum);
  if (!a || !b) return false;

  const [aMaj, aMin, aPat] = a.split('.').map(Number);
  const [bMaj, bMin, bPat] = b.split('.').map(Number);

  if (aMaj !== bMaj) return aMaj > bMaj;
  if (aMin !== bMin) return aMin > bMin;
  return aPat >= bPat;
}

/**
 * Detect a framework's installed version by running its `version_command` in `cwd`.
 * Returns an empty string when the command fails or no semver can be parsed.
 */
export function detectFrameworkVersion(framework: string, cwd: string): string {
  const config = getFrameworkConfig(framework);
  const result = execCommand(config.version_command, { cwd, timeout: 30_000 });
  if (result.status !== 0) {
    return '';
  }
  const output = `${result.stdout ?? ''}\n${result.stderr ?? ''}`;
  return extractSemver(output) ?? '';
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
