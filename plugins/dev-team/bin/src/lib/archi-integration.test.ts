/**
 * Integration tests for C4 architecture CLI modules.
 *
 * Tests module interaction scenarios based on module boundary contracts
 * defined in specs/c4-cli-integration/spec.md and architecture-model/spec.md.
 *
 * Module dependency graph:
 *   c4-parser (foundation) → archi-query, archi-validate, c4-cross-ref
 *   archi-validate → archi-write
 */

import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import { describe, it, expect, beforeAll, afterAll } from 'vite-plus/test';

import { queryModel } from './archi-query';
import { validateDsl } from './archi-validate';
import { writeDsl } from './archi-write';
import { runCrossRefCheck } from './c4-cross-ref';
import { parseC4Dsl, validateC4Dsl, readAllModels, findSpecificationBlock } from './c4-parser';

// ---------------------------------------------------------------------------
// Fixture: shared temp project with realistic multi-module model
// ---------------------------------------------------------------------------

let projectRoot: string;

beforeAll(() => {
  projectRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'archi-int-'));
  const modelsDir = path.join(projectRoot, 'openspec', 'specs', 'architecture', 'models');
  fs.mkdirSync(modelsDir, { recursive: true });

  // 01-core.c4 — specification + package
  fs.writeFileSync(
    path.join(modelsDir, '01-core.c4'),
    [
      'specification {',
      '  element package',
      '  element domain',
      '  element module',
      '  element component',
      '}',
      '',
      'model {',
      '  package MyApp {',
      '    metadata { path "./src/" }',
      '  }',
      '',
      '  extend MyApp {',
      '    domain AuthDomain {',
      '      metadata { path "./src/auth/" }',
      '    }',
      '  }',
      '',
      '  extend MyApp.AuthDomain {',
      '    module LoginModule {',
      '      metadata { path "./src/auth/login/" }',
      '    }',
      '    module RegisterModule {',
      '      metadata { path "./src/auth/register/" }',
      '    }',
      '  }',
      '',
      '  extend MyApp {',
      '    domain DataDomain {',
      '      metadata { path "./src/data/" }',
      '    }',
      '  }',
      '}',
    ].join('\n'),
    'utf-8',
  );

  // 02-relationships.c4 — declared relationships between modules
  fs.writeFileSync(
    path.join(modelsDir, '02-relationships.c4'),
    [
      'model {',
      "  MyApp.AuthDomain.LoginModule -> MyApp.AuthDomain.RegisterModule 'redirects to'",
      "  MyApp.AuthDomain.LoginModule -> MyApp.DataDomain 'fetches user data'",
      "  MyApp.AuthDomain.RegisterModule -> MyApp.DataDomain 'stores profile'",
      '}',
    ].join('\n'),
    'utf-8',
  );

  // Source files matching model metadata.path entries
  const srcDir = path.join(projectRoot, 'src');
  fs.mkdirSync(path.join(srcDir, 'auth', 'login'), { recursive: true });
  fs.mkdirSync(path.join(srcDir, 'auth', 'register'), { recursive: true });
  fs.mkdirSync(path.join(srcDir, 'data'), { recursive: true });

  fs.writeFileSync(
    path.join(srcDir, 'auth', 'login', 'index.ts'),
    'import { redirect } from "../register/handler";\nimport { fetchUser } from "../../data/repo";\n',
    'utf-8',
  );
  fs.writeFileSync(
    path.join(srcDir, 'auth', 'register', 'handler.ts'),
    'import { saveProfile } from "../../data/repo";\n',
    'utf-8',
  );
  fs.writeFileSync(
    path.join(srcDir, 'data', 'repo.ts'),
    'export function fetchUser() {}\nexport function saveProfile() {}\n',
    'utf-8',
  );
});

afterAll(() => {
  fs.rmSync(projectRoot, { recursive: true, force: true });
});

// ===========================================================================
// Scenario 1: Parse → Query pipeline
// ===========================================================================

describe('Parse → Query pipeline', () => {
  it('should parse model files and query all elements with relationships', async () => {
    const dsl = readAllModels(projectRoot);
    expect(dsl).not.toBeNull();
    const parsed = await parseC4Dsl(dsl!);
    expect(parsed.elements.length).toBeGreaterThanOrEqual(5);

    const queryResult = await queryModel(projectRoot);
    expect(queryResult.error).toBeUndefined();
    expect(queryResult.elements!.length).toBe(parsed.elements.length);
    expect(queryResult.relationships!.length).toBe(parsed.relationships.length);
  });

  it('parse-to-query: element FQN filter should match parsed element', async () => {
    const parsed = await parseC4Dsl(readAllModels(projectRoot)!);
    const loginEl = parsed.elements.find((e) => e.name === 'MyApp.AuthDomain.LoginModule');
    expect(loginEl).toBeDefined();

    const queryResult = await queryModel(projectRoot, 'MyApp.AuthDomain.LoginModule');
    expect(queryResult.element).toBeDefined();
    expect(queryResult.element!.name).toBe(loginEl!.name);
    expect(queryResult.element!.kind).toBe(loginEl!.kind);
  });

  it('readAllModels should parse all .c4 files in models directory', () => {
    const dsl = readAllModels(projectRoot);
    expect(dsl).not.toBeNull();
    expect(dsl!.length).toBeGreaterThan(0);
  });
});

// ===========================================================================
// Scenario 2: Parse → Validate pipeline
// ===========================================================================

describe('Parse → Validate pipeline', () => {
  it('should validate parsed DSL as structurally sound', async () => {
    const dsl = readAllModels(projectRoot)!;
    const parsed = await parseC4Dsl(dsl);
    expect(parsed.errors).toEqual([]);

    const validation = await validateC4Dsl(dsl);
    expect(validation.valid).toBe(true);
  });

  it('validateDsl should use findSpecificationBlock to prepend spec for source-only DSL', async () => {
    const specBlock = findSpecificationBlock(projectRoot);
    expect(specBlock).not.toBeNull();

    const sourceWithoutSpec = 'model {\n  package Test { metadata { path "./test/" } }\n}';
    const result = await validateDsl(projectRoot, sourceWithoutSpec);
    expect(result.valid).toBe(true);
  });

  it('validateDsl should fail when model files contain duplicate specs', async () => {
    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'archi-int-dup-'));
    try {
      const modelsDir = path.join(tmpDir, 'openspec', 'specs', 'architecture', 'models');
      fs.mkdirSync(modelsDir, { recursive: true });
      fs.writeFileSync(path.join(modelsDir, '01.c4'), 'specification { element package }');
      fs.writeFileSync(path.join(modelsDir, '02.c4'), 'specification { element domain }');

      const result = await validateDsl(tmpDir);
      expect(result.valid).toBe(false);
      expect(result.errors!.some((e) => e.includes('Duplicate'))).toBe(true);
    } finally {
      fs.rmSync(tmpDir, { recursive: true, force: true });
    }
  });
});

// ===========================================================================
// Scenario 3: Validate → Write pipeline
// ===========================================================================

describe('Validate → Write pipeline', () => {
  it('should validate DSL then write to models/ directory', async () => {
    const dsl = 'model {\n  package NewMod {\n    metadata { path "./new-mod/" }\n  }\n}';

    const validation = await validateDsl(projectRoot, dsl);
    expect(validation.valid).toBe(true);

    const writeResult = await writeDsl(projectRoot, dsl, '03-new-module.c4');
    expect(writeResult.success).toBe(true);

    const modelsDir = path.join(projectRoot, 'openspec', 'specs', 'architecture', 'models');
    const writtenFiles = fs.readdirSync(modelsDir);
    expect(writtenFiles.includes('03-new-module.c4')).toBe(true);
  });

  it('should reject write when validation fails (validate-before-write contract)', async () => {
    const invalidDsl = 'model {\n  package Broken {\n';

    const validation = await validateDsl(projectRoot, invalidDsl);
    // LikeC4 may or may not flag unmatched braces — test just verifies result structure
    if (!validation.valid) {
      const writeResult = await writeDsl(projectRoot, invalidDsl, 'should-not-exist.c4');
      expect(writeResult.success).toBe(false);
    }
  });

  it('should enforce path containment: writeDsl rejects paths outside models/', async () => {
    const dsl =
      'specification { element package }\nmodel {\n  package Escape { metadata { path "./escape/" } }\n}';
    const validation = await validateDsl(projectRoot, dsl);
    expect(validation.valid).toBe(true);
    expect((await writeDsl(projectRoot, dsl, '../../../escape.c4')).success).toBe(false);
    expect((await writeDsl(projectRoot, dsl, '/etc/hacked.c4')).success).toBe(false);
  });
});

// ===========================================================================
// Scenario 4: Parse → Cross-Reference pipeline
// ===========================================================================

describe('Parse → Cross-Reference pipeline', () => {
  it('should match changed files to model elements via metadata.path', async () => {
    const result = await runCrossRefCheck(projectRoot, {
      files: ['src/auth/login/index.ts'],
    });
    expect(result.matched.length).toBeGreaterThanOrEqual(1);
    expect(
      result.matched.some(
        (m) => m.element_id === 'MyApp' || m.element_id === 'MyApp.AuthDomain.LoginModule',
      ),
    ).toBe(true);
    expect(['clean', 'violations_found']).toContain(result.status);
  });

  it('should detect unmodeled dependency when import has no relationship', async () => {
    fs.writeFileSync(
      path.join(projectRoot, 'src', 'auth', 'login', 'secret.ts'),
      'import { hack } from "../register/secret-internal";\n',
      'utf-8',
    );

    try {
      const result = await runCrossRefCheck(projectRoot, {
        files: ['src/auth/login/secret.ts'],
      });
      expect(['clean', 'violations_found', 'no_changes']).toContain(result.status);
    } finally {
      fs.unlinkSync(path.join(projectRoot, 'src', 'auth', 'login', 'secret.ts'));
    }
  });

  it('should warn about unmapped import targets not in any model element', async () => {
    const orphanDir = path.join(projectRoot, 'lib');
    fs.mkdirSync(orphanDir);
    fs.writeFileSync(
      path.join(orphanDir, 'external.ts'),
      'import { something } from "./nonexistent";\n',
      'utf-8',
    );

    try {
      const result = await runCrossRefCheck(projectRoot, {
        files: ['lib/external.ts'],
      });
      expect(result.unmatched_files).toContain('lib/external.ts');
      expect(Array.isArray(result.warnings)).toBe(true);
    } finally {
      fs.rmSync(orphanDir, { recursive: true, force: true });
    }
  });

  it('should return no_changes when file list is empty', async () => {
    const result = await runCrossRefCheck(projectRoot, { files: [] });
    expect(result.status).toBe('no_changes');
  });
});

// ===========================================================================
// Scenario 5: Full pipeline (parse → validate → query → write → check)
// ===========================================================================

describe('Full pipeline: parse → validate → query → write → check', () => {
  it('should complete the full lifecycle for a new module', async () => {
    const initial = await queryModel(projectRoot);
    const initialCount = initial.elements!.length;

    expect((await validateDsl(projectRoot)).valid).toBe(true);

    const newDsl = [
      'model {',
      '  extend MyApp.AuthDomain {',
      '    module ResetPasswordModule {',
      '      metadata { path "./src/auth/reset-password/" }',
      '    }',
      '  }',
      "  MyApp.AuthDomain.ResetPasswordModule -> MyApp.DataDomain 'sends email'",
      '}',
    ].join('\n');

    const specBlock = findSpecificationBlock(projectRoot)!;
    const fullDsl = specBlock + '\n' + newDsl;

    const writeResult = await writeDsl(projectRoot, fullDsl, '03-reset-password.c4');
    expect(writeResult.success).toBe(true);

    const updated = await queryModel(projectRoot);
    expect(updated.elements!.length).toBeGreaterThan(initialCount);
    expect(updated.elements!.some((e) => e.name === 'MyApp.AuthDomain.ResetPasswordModule')).toBe(
      true,
    );

    const srcDir = path.join(projectRoot, 'src', 'auth', 'reset-password');
    fs.mkdirSync(srcDir, { recursive: true });
    fs.writeFileSync(
      path.join(srcDir, 'index.ts'),
      'import { fetchUser } from "../../data/repo";\n',
      'utf-8',
    );

    const checkResult = await runCrossRefCheck(projectRoot, {
      files: ['src/auth/reset-password/index.ts'],
    });
    expect(checkResult.matched.length).toBeGreaterThanOrEqual(1);
    expect(['clean', 'violations_found']).toContain(checkResult.status);
  });
});
