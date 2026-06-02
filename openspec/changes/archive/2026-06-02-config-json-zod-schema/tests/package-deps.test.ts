/**
 * Build verification test for config-json-zod-schema.
 *
 * AC-8: Verify that the `yaml` dependency is available for migration.
 * The yaml package is needed for the YAML-to-JSON automatic migration path.
 * After all projects have been migrated (future cleanup), yaml can be removed.
 *
 * @see openspec/changes/config-json-zod-schema/proposal.md
 */

import { describe, it, expect } from 'vite-plus/test';
import * as fs from 'fs';
import * as path from 'path';

const PACKAGE_JSON_PATH = path.resolve(
  __dirname,
  '../../../../plugins/dev-team/bin/package.json',
);

describe('yaml dependency for migration', () => {
  it('should have package.json at the expected location', () => {
    expect(fs.existsSync(PACKAGE_JSON_PATH)).toBe(true);
  });

  it('should declare yaml as a production dependency (needed for migration)', () => {
    const pkg = JSON.parse(fs.readFileSync(PACKAGE_JSON_PATH, 'utf-8'));
    expect(pkg.dependencies).toBeDefined();
    expect(pkg.dependencies).toHaveProperty('yaml');
  });

  it('should have a non-empty version for yaml dependency', () => {
    const pkg = JSON.parse(fs.readFileSync(PACKAGE_JSON_PATH, 'utf-8'));
    const version = pkg.dependencies.yaml;
    expect(version).toBeDefined();
    expect(version).not.toBe('');
    expect(version).not.toBe('*');
  });

  it('should have zod for config validation', () => {
    const pkg = JSON.parse(fs.readFileSync(PACKAGE_JSON_PATH, 'utf-8'));
    expect(pkg.dependencies).toHaveProperty('zod');
  });
});
