import { readConfig, writeConfig, setValue } from '../lib/config';

export interface ConfigSetOptions {
  key: string;
  value: unknown;
  projectRoot?: string;
}

export interface ConfigSetResult {
  key: string;
  written: boolean;
}

/**
 * Run config/set: write a value to openspec/config.json by dot-separated key path.
 * Value types are preserved from the JSON input (no heuristic type inference).
 * Creates skeleton config file if missing.
 */
export function runConfigSet(options: ConfigSetOptions): ConfigSetResult {
  const projectRoot = options.projectRoot || process.env.CLAUDE_PROJECT_DIR || process.cwd();

  // Read current config (returns default if no file exists yet)
  const config = readConfig(projectRoot);

  // Set the value at the key path (no type inference — JSON format preserves types)
  setValue(config as Record<string, unknown>, options.key, options.value);

  // Write back to file (validates against Zod schema before writing)
  writeConfig(projectRoot, config);

  return {
    key: options.key,
    written: true,
  };
}
