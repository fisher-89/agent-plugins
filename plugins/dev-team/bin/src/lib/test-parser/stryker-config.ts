// ---------------------------------------------------------------------------
// StrykerJS Configuration Resolver
//
// Detects whether a project already has a stryker.config.* file.  If one
// exists, returns its path with cleanup=false.  Otherwise generates a
// temporary configuration file based on built-in template parameters.
// ---------------------------------------------------------------------------

import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

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
 * Build jest/vitest runner overlay with stryker-cwd-relative configFile when
 * suite.config is set.
 *
 * Vitest runner schema only allows `configFile` (no `config` / `testMatch`).
 */
function buildRunnerConfigOverlay(
  strykerRoot: string,
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
      : path.resolve(strykerRoot, frameworkConfigPath);
  const configFile = path.relative(strykerRoot, absConfig).replace(/\\/g, '/');

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
        config: {
          testMatch: [testMatchGlob],
          reporters: [],
        },
      },
    };
  }

  return { [testRunner]: { configFile } };
}

function generateTempConfig(
  strykerRoot: string,
  planRoot: string,
  sourceFiles: string[],
  testRunner: string,
  reportDir: string,
  frameworkConfigPath?: string | null,
): { configPath: string; tempDirPath: string } {
  const randomSuffix = crypto.randomBytes(4).toString('hex');
  const configPath = path.resolve(strykerRoot, `stryker.config.${randomSuffix}.json`);
  const tempDirPath = path.resolve(reportDir, '_stryker-tmp');
  const tempDirName = tempDirPath.replace(/\\/g, '/');
  const normalizedSources = normalizeSourceFilesForStryker(strykerRoot, sourceFiles);
  const mutationFileAbs = path.resolve(reportDir, 'mutation.json').replace(/\\/g, '/');
  const mutationHtmlAbs = path.resolve(reportDir, 'mutation.html').replace(/\\/g, '/');

  const config = {
    $schema: 'node_modules/@stryker-mutator/core/schema/stryker-schema.json',
    ignorePatterns: ['openspec/**/*'],
    mutate: normalizedSources,
    testRunner,
    plugins: [resolvePluginPackage(testRunner)],
    ...buildRunnerConfigOverlay(strykerRoot, planRoot, testRunner, frameworkConfigPath),
    disableTypeChecks: true,
    ignoreStatic: true,
    reporters: ['json', 'html'],
    jsonReporter: { fileName: mutationFileAbs },
    htmlReporter: { fileName: mutationHtmlAbs },
    tempDirName,
    concurrency: os.cpus().length / 2, // 本地突变仅占用一半内核，保障用户其他操作
    timeoutFactor: 1.5,
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
 * @param strykerRoot - Stryker executing cwd
 * @param planRoot - Suite root relative to projectRoot (drives jest testMatch)
 * @param absoluteSourceFiles - Absolute source file paths
 * @param framework  - The test framework name ("jest", "vitest", or "vite-plus")
 * @param reportDir  - Absolute plan report directory (mutation.json / mutation.html land here)
 * @param frameworkConfigPath - Absolute (or resolvable) path to suite `tests[].config`, if any
 * @throws {Error} If the framework is not supported by StrykerJS
 */
export function resolveStrykerConfig(
  strykerRoot: string,
  planRoot: string,
  absoluteSourceFiles: string[],
  framework: string,
  reportDir: string,
  frameworkConfigPath?: string | null,
): { configPath: string; tempDirPath: string } {
  const testRunner = resolveTestRunner(framework);
  const { configPath, tempDirPath } = generateTempConfig(
    strykerRoot,
    planRoot,
    absoluteSourceFiles,
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
