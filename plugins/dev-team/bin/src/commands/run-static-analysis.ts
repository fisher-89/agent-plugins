import { ensureConfigFile, getValue } from '../lib/config';
import { execCommand } from '../lib/exec-command';
import { getProjectDir } from '../utils';

export interface RunStaticAnalysisOptions {
  projectRoot?: string;
}

function resolveProjectRoot(options?: RunStaticAnalysisOptions): string {
  return options?.projectRoot || getProjectDir();
}

/**
 * Read `static_analysis` from openspec/config.json and execute the configured command.
 * Returns exit code 0 when unconfigured or when the command succeeds.
 */
export function runStaticAnalysis(options?: RunStaticAnalysisOptions): number {
  const projectRoot = resolveProjectRoot(options);
  const config = ensureConfigFile(projectRoot);
  const { value, exists } = getValue(config, 'static_analysis');

  if (!exists || typeof value !== 'string' || value.trim() === '') {
    return 0;
  }

  const command = value.trim();
  const result = execCommand(command, { cwd: projectRoot });

  const stdout = result.stdout ?? '';
  const stderr = result.stderr ?? '';
  const output = `${stdout}${stderr}`.trimEnd();

  if (output.length > 0) {
    process.stderr.write(output.endsWith('\n') ? output : `${output}\n`);
  }

  if (result.error) {
    process.stderr.write(`${result.error.message}\n`);
    return 1;
  }

  if (result.status === null) {
    return 1;
  }

  return result.status;
}
