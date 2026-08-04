// ---------------------------------------------------------------------------
// Test Framework Registry
//
// Hardcoded mapping of known test frameworks to their test and coverage
// commands.  Framework commands are implementation details managed by the
// plugin maintainer, NOT by project config.json (see design decision D1).
//
// Templates embed file-channel placeholders ({results_file}, {coverage_file},
// {report_dir}, {config_args}, …). Detect leaves them unexpanded; execute
// resolves them against the plan report directory.
// ---------------------------------------------------------------------------

import { type TestFramework } from '../schemas';
import { execCommand } from './exec-command';

/** Build a test_execution command template from a detected framework version. */
type TestExecutionBuilder = (version: string) => string;

/** Presence of mutation_execution is the capability gate; omit for unsupported frameworks. */
const MUTATION_EXECUTION_JEST =
  'npx -y -p @stryker-mutator/core@7.2.0 -p @stryker-mutator/jest-runner@7.2.0 stryker run "{config}"';
const MUTATION_EXECUTION_VITEST =
  'npx -y -p @stryker-mutator/core@8 -p @stryker-mutator/vitest-runner@8 stryker run "{config}"';

export interface FrameworkConfig {
  framework: TestFramework;
  /** Command run in the suite cwd to detect the framework version (usually `--version`). */
  version_command: string;
  shell: {
    test_execution: TestExecutionBuilder;
    mutation_execution?: string;
  };
  cmd: {
    test_execution: TestExecutionBuilder;
    mutation_execution?: string;
  };
  coverage_format: 'istanbul' | 'llvm-cov' | 'node-test' | 'go-cover' | 'coverage-py' | 'lcov';
  /** Coverage artifact file name relative to the plan report directory (reportDir). */
  coverage_output: string;
  default_glob: string;
  /** CLI flag for injecting a framework config file; null when unsupported. */
  config_flag: string | null;
}

const FRAMEWORK_REGISTRY: Record<TestFramework, FrameworkConfig> = {
  jest: {
    framework: 'jest',
    version_command: 'npx jest --version',
    shell: {
      test_execution: (version) =>
        `npx jest${isVersionAtLeast(version, '29.5.0') ? ' --randomize' : ''} --no-verbose --json --outputFile="{results_file}" --silent --coverage --coverageDirectory="{report_dir}" --coverageReporters=json-summary {config_args} {files}`,
      mutation_execution: MUTATION_EXECUTION_JEST,
    },
    cmd: {
      test_execution: (version) =>
        `npx jest${isVersionAtLeast(version, '29.5.0') ? ' --randomize' : ''} --no-verbose --json --outputFile="{results_file}" --silent --coverage --coverageDirectory="{report_dir}" --coverageReporters=json-summary {config_args} {files}`,
      mutation_execution: MUTATION_EXECUTION_JEST,
    },
    coverage_format: 'istanbul',
    coverage_output: 'coverage-summary.json',
    default_glob: '**/*.{test,spec}.{js,ts,jsx,tsx}',
    config_flag: '--config',
  },
  vitest: {
    framework: 'vitest',
    version_command: 'npx vitest --version',
    shell: {
      test_execution: () =>
        'npx vitest run --sequence.shuffle --reporter=json --outputFile="{results_file}" --silent --coverage --coverage.reportsDirectory="{report_dir}" --coverage.reporter=json-summary {config_args} {files}',
      mutation_execution: MUTATION_EXECUTION_VITEST,
    },
    cmd: {
      test_execution: () =>
        'npx vitest run --sequence.shuffle --reporter=json --outputFile="{results_file}" --silent --coverage --coverage.reportsDirectory="{report_dir}" --coverage.reporter=json-summary {config_args} {files}',
      mutation_execution: MUTATION_EXECUTION_VITEST,
    },
    coverage_format: 'istanbul',
    coverage_output: 'coverage-summary.json',
    default_glob: '**/*.{test,spec}.{js,ts,jsx,tsx}',
    config_flag: '--config',
  },
  'vite-plus': {
    framework: 'vite-plus',
    version_command: 'vp --version',
    shell: {
      test_execution: () =>
        'vp test --sequence.shuffle --reporter=json --outputFile="{results_file}" --silent --coverage --coverage.reportsDirectory="{report_dir}" --coverage.reporter=json-summary {config_args} {files}',
      mutation_execution: MUTATION_EXECUTION_VITEST,
    },
    cmd: {
      test_execution: () =>
        'vp test --sequence.shuffle --reporter=json --outputFile="{results_file}" --silent --coverage --coverage.reportsDirectory="{report_dir}" --coverage.reporter=json-summary {config_args} {files}',
      mutation_execution: MUTATION_EXECUTION_VITEST,
    },
    coverage_format: 'istanbul',
    coverage_output: 'coverage-summary.json',
    default_glob: '**/*.{test,spec}.{js,ts,jsx,tsx}',
    config_flag: '--config',
  },
  bun: {
    framework: 'bun',
    version_command: 'bun --version',
    shell: {
      test_execution: () => 'bun {config_args} test --coverage {files}',
    },
    cmd: {
      test_execution: () => 'bun {config_args} test --coverage {files}',
    },
    coverage_format: 'lcov',
    coverage_output: 'lcov.info',
    default_glob: '**/*.{test,spec}.{js,ts,jsx,tsx}',
    config_flag: '--config',
  },
  rust: {
    framework: 'rust',
    version_command: 'cargo --version',
    shell: {
      test_execution: () =>
        'cargo test; _X=$?; cargo llvm-cov --json --output-path "{coverage_file}"; exit $_X',
    },
    cmd: {
      test_execution: () =>
        'cargo test & if errorlevel 1 set _X=%errorlevel% & cargo llvm-cov --json --output-path "{coverage_file}" & exit /b %_X%',
    },
    coverage_format: 'llvm-cov',
    coverage_output: 'coverage-summary.json',
    default_glob: '**/tests/**/*.rs',
    config_flag: null,
  },
  'node-test': {
    framework: 'node-test',
    version_command: 'node --version',
    shell: {
      test_execution: () => 'node --test --experimental-test-coverage {files}',
    },
    cmd: {
      test_execution: () => 'node --test --experimental-test-coverage {files}',
    },
    coverage_format: 'node-test',
    coverage_output: 'results.txt',
    default_glob: '**/*.test.{mjs,js,cjs}',
    config_flag: null,
  },
  go: {
    framework: 'go',
    version_command: 'go version',
    shell: {
      test_execution: () =>
        'go test -json -coverprofile="{coverprofile_file}" -covermode=atomic {directory}; _X=$?; go tool cover -func="{coverprofile_file}" > "{coverage_file}"; exit $_X',
    },
    cmd: {
      test_execution: () =>
        'go test -json -coverprofile="{coverprofile_file}" -covermode=atomic {directory} & if errorlevel 1 set _X=%errorlevel% & go tool cover -func="{coverprofile_file}" > "{coverage_file}" & exit /b %_X%',
    },
    coverage_format: 'go-cover',
    coverage_output: 'func-summary.txt',
    default_glob: '**/*_test.go',
    config_flag: null,
  },
  pytest: {
    framework: 'pytest',
    version_command: 'pytest --version',
    shell: {
      test_execution: () =>
        'pytest -v {files}; pytest --cov=. --cov-report="json:{coverage_file}" --cov-branch -q',
    },
    cmd: {
      test_execution: () =>
        'pytest -v {files} && pytest --cov=. --cov-report="json:{coverage_file}" --cov-branch -q',
    },
    coverage_format: 'coverage-py',
    coverage_output: 'coverage.json',
    default_glob: '**/test_*.py',
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
