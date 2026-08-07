// ---------------------------------------------------------------------------
// StrykerJS Configuration Resolver
//
// Detects whether a project already has a stryker.config.* file.  If one
// exists, returns its path with cleanup=false.  Otherwise generates a
// temporary configuration file based on built-in template parameters.
// ---------------------------------------------------------------------------

import * as crypto from 'crypto';
import * as fs from 'fs';
import * as path from 'path';

type RunnerConfigOverlay =
  | {
      vitest: {
        configFile: string;
        dir?: string;
      };
    }
  | {
      jest: {
        configFile: string;
        enableFindRelatedTests?: boolean;
        config?: object;
      };
    }
  | {
      [key: string]: { configFile: string };
    };

// ---------------------------------------------------------------------------
// Temporary config generation
// ---------------------------------------------------------------------------

/** Normalize source paths to forward-slash paths relative to projectRoot. */
function normalizeSourceFilesForStryker(rootPath: string, sourceFiles: string[]): string[] {
  const absRoot = path.resolve(rootPath);
  return sourceFiles.map((f) => {
    if (f === '') {
      return '';
    }
    const abs =
      path.isAbsolute(f) || /^[A-Za-z]:/.test(f) ? path.resolve(f) : path.resolve(absRoot, f);
    return path.relative(absRoot, abs).replace(/\\/g, '/');
  });
}

/**
 * Build jest/vitest runner overlay with projectRoot-relative configFile when
 * suite.config is set. Relative paths resolve against projectRoot.
 *
 * Vitest runner schema only allows `configFile` (no `config` / `testMatch`).
 * Jest keeps `config.testMatch` + `enableFindRelatedTests: false`.
 */
function buildRunnerConfigOverlay(
  projectRoot: string,
  planRoot: string,
  testRunner: string,
  frameworkConfigPath: string | null | undefined,
): RunnerConfigOverlay {
  if (!frameworkConfigPath) {
    return {};
  }
  const absConfig =
    path.isAbsolute(frameworkConfigPath) || /^[A-Za-z]:/.test(frameworkConfigPath)
      ? path.resolve(frameworkConfigPath)
      : path.resolve(projectRoot, frameworkConfigPath);
  const configFile = path.relative(projectRoot, absConfig).replace(/\\/g, '/');

  if (testRunner === 'vitest') {
    return {
      vitest: {
        configFile,
        dir: path.dirname(configFile),
      },
    };
  }

  if (testRunner === 'jest') {
    const normalizedPlan = planRoot.replace(/^\.\//, '').replace(/\\/g, '/');
    const testMatchGlob =
      !normalizedPlan || normalizedPlan === '.'
        ? '**/*.test.ts?(x)'
        : `**/${normalizedPlan}/**/*.test.ts?(x)`;
    return {
      jest: {
        configFile,
        enableFindRelatedTests: false,
        config: {
          testMatch: [testMatchGlob],
        },
      },
    };
  }

  return { [testRunner]: { configFile } };
}

function generateTempConfig(
  projectRoot: string,
  planRoot: string,
  sourceFiles: string[],
  testRunner: string,
  reportDir: string,
  frameworkConfigPath?: string | null,
): { configPath: string; tempDirPath: string } {
  const randomSuffix = crypto.randomBytes(4).toString('hex');
  const configPath = path.resolve(projectRoot, `stryker.config.${randomSuffix}.json`);
  const tempDirPath = path.resolve(projectRoot, '.stryker-tmp');
  const normalizedSources = normalizeSourceFilesForStryker(projectRoot, sourceFiles);
  const mutationFileAbs = path.resolve(reportDir, 'mutation.json').replace(/\\/g, '/');

  const config = {
    $schema: 'node_modules/@stryker-mutator/core/schema/stryker-schema.json',
    mutate: normalizedSources,
    testRunner,
    plugins: [resolvePluginPackage(testRunner)],
    ...buildRunnerConfigOverlay(projectRoot, planRoot, testRunner, frameworkConfigPath),
    ignoreStatic: true,
    reporters: ['json', 'html'],
    jsonReporter: { fileName: mutationFileAbs },
    timeoutMS: 10000,
  };

  fs.writeFileSync(configPath, JSON.stringify(config, null, 2), 'utf-8');
  return { configPath, tempDirPath };
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/**
 * Resolve the StrykerJS configuration for a project.
 *
 * Always generates a temporary configuration in `projectRoot` with
 * `jsonReporter.fileName` as an absolute path to `{reportDir}/mutation.json`.
 * The temp config is deleted by the caller after the run; user long-lived
 * Stryker configs are never modified.
 *
 * @param projectRoot - Absolute project root
 * @param planRoot - Suite root relative to projectRoot (drives jest testMatch)
 * @param sourceFiles - Source file paths (absolute or projectRoot-relative; normalized to projectRoot-relative POSIX)
 * @param framework  - The test framework name ("jest", "vitest", or "vite-plus")
 * @param reportDir  - Absolute plan report directory (mutation.json lands here)
 * @param frameworkConfigPath - Absolute (or resolvable) path to suite `tests[].config`, if any
 * @throws {Error} If the framework is not supported by StrykerJS
 */
export function resolveStrykerConfig(
  projectRoot: string,
  planRoot: string,
  sourceFiles: string[],
  framework: string,
  reportDir: string,
  frameworkConfigPath?: string | null,
): { configPath: string; tempDirPath: string } {
  const testRunner = resolveTestRunner(framework);
  const { configPath, tempDirPath } = generateTempConfig(
    projectRoot,
    planRoot,
    sourceFiles,
    testRunner,
    reportDir,
    frameworkConfigPath,
  );
  return { configPath, tempDirPath };
}

/**
 * Resolve the StrykerJS test runner plugin name from a framework identifier.
 *
 * @param framework - The test framework name
 * @returns The StrykerJS runner plugin name
 * @throws {Error} If the framework is not supported
 */
function resolveTestRunner(framework: string): string {
  switch (framework) {
    case 'jest':
      return 'jest';
    case 'vitest':
    case 'vite-plus':
      return 'vitest';
    default:
      throw new Error(
        `Unsupported mutation testing framework "${framework}". Supported: jest, vitest, vite-plus`,
      );
  }
}

/**
 * Resolve the npm plugin package name from the test runner name.
 *
 * StrykerJS v9 renamed the vitest runner plugin from "vitest-runner" to
 * "vitest" at the config level, but the npm package remains
 * `@stryker-mutator/vitest-runner`.  This function maps the simplified
 * runner name back to the correct package name.
 */
function resolvePluginPackage(testRunner: string): string {
  // In StrykerJS v9, the vitest runner registers as "vitest" but the
  // npm package is still "@stryker-mutator/vitest-runner".
  const pluginMap: Record<string, string> = {
    jest: '@stryker-mutator/jest-runner',
    vitest: '@stryker-mutator/vitest-runner',
  };
  return pluginMap[testRunner] ?? `@stryker-mutator/${testRunner}`;
}
