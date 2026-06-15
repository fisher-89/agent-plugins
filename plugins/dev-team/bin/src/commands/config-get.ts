import { getValue, ensureConfigFile } from '../lib/config';

export interface ConfigGetOptions {
  key: string;
  projectRoot?: string;
}

export interface ConfigGetResult {
  key: string;
  value: unknown;
  exists: boolean;
}

/**
 * Run config_get: read a value from openspec/config.json by dot-separated key path.
 * Ensures the config file exists before reading.
 */
export function runConfigGet(options: ConfigGetOptions): ConfigGetResult {
  const projectRoot = options.projectRoot || process.env.PROJECT_DIR || process.cwd();

  // Ensure the config file exists (creates skeleton if missing)
  const config = ensureConfigFile(projectRoot);

  // Get the value at the key path
  const { value, exists } = getValue(config, options.key);

  return {
    key: options.key,
    value,
    exists,
  };
}
