/**
 * Build verification test for config-json-zod-schema.
 *
 * Verify that the `yaml` dependency is retained in package.json for the
 * YAML-to-JSON auto-migration path, and that `zod` is added for schema
 * validation.
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

describe('yaml dependency retained for migration', () => {
  it('should have package.json at the expected location', () => {
    expect(fs.existsSync(PACKAGE_JSON_PATH)).toBe(true);
  });

  it('should declare yaml as a dependency (retained for auto-migration)', () => {
    const pkg = JSON.parse(fs.readFileSync(PACKAGE_JSON_PATH, 'utf-8'));
    expect(pkg.dependencies).toBeDefined();
    expect(pkg.dependencies).toHaveProperty('yaml');
  });

  it('should have zod defined as a dependency (schema validation)', () => {
    const pkg = JSON.parse(fs.readFileSync(PACKAGE_JSON_PATH, 'utf-8'));
    expect(pkg.dependencies).toHaveProperty('zod');
  });
});
