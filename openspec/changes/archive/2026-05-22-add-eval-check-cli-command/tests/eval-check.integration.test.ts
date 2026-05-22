import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { execSync } from "child_process";
import * as fs from "fs";
import * as path from "path";
import * as os from "os";

// ---------------------------------------------------------------------------
// Paths
// ---------------------------------------------------------------------------
const PROJECT_ROOT = path.resolve(
  __dirname,
  "../../../../"
);
const CHANGES_DIR = path.resolve(PROJECT_ROOT, "openspec", "changes");
const BUNDLE_PATH = path.resolve(
  PROJECT_ROOT,
  "plugins",
  "dev-team",
  "bin",
  "dev-team-bundle.js"
);

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
interface TempChange {
  name: string;
  cleanup: () => void;
}

const createdChanges: string[] = [];

/**
 * Create a temporary change directory inside the project's openspec/changes/
 * so that the CLI's built-in path resolution finds it correctly.
 * Returns the change name and a cleanup function.
 */
function createTempChange(): TempChange {
  const randomSuffix = Math.random().toString(36).substring(2, 8);
  const changeName = `__eval_check_test_${randomSuffix}`;
  const phasesDir = path.resolve(CHANGES_DIR, changeName, "phases");
  fs.mkdirSync(phasesDir, { recursive: true });
  createdChanges.push(changeName);

  return {
    name: changeName,
    cleanup: () => {
      const changeDir = path.resolve(CHANGES_DIR, changeName);
      if (fs.existsSync(changeDir)) {
        fs.rmSync(changeDir, { recursive: true, force: true });
      }
    },
  };
}

function writeEvalJson(changeName: string, entries: any[]): void {
  const filePath = path.resolve(
    CHANGES_DIR,
    changeName,
    "phases",
    "eval.json"
  );
  fs.writeFileSync(filePath, JSON.stringify(entries, null, 2) + "\n", "utf-8");
}

function runEvalCheck(
  args: string[]
): { stdout: string; stderr: string; status: number } {
  try {
    const result = execSync(
      `node "${BUNDLE_PATH}" eval-check ${args.join(" ")}`,
      { encoding: "utf-8", cwd: PROJECT_ROOT }
    );
    return { stdout: result.trim(), stderr: "", status: 0 };
  } catch (e: any) {
    return {
      stdout: e.stdout ? e.stdout.toString().trim() : "",
      stderr: e.stderr ? e.stderr.toString().trim() : "",
      status: e.status !== undefined ? e.status : 1,
    };
  }
}

beforeAll(() => {
  if (!fs.existsSync(BUNDLE_PATH)) {
    console.warn(
      `Skipping integration tests: bundle not found at ${BUNDLE_PATH}`
    );
  }
});

afterAll(() => {
  // Cleanup all temporary changes
  for (const changeName of createdChanges) {
    const changeDir = path.resolve(CHANGES_DIR, changeName);
    if (fs.existsSync(changeDir)) {
      fs.rmSync(changeDir, { recursive: true, force: true });
    }
  }
});

// ===========================================================================
// Integration Tests
// ===========================================================================
describe("eval-check CLI (integration)", () => {
  // -----------------------------------------------------------------------
  // AC-1: All prior phases have pass records -> exit code 0
  // -----------------------------------------------------------------------
  it("AC-1: returns exit code 0 when all prior phases have pass records", () => {
    if (!fs.existsSync(BUNDLE_PATH)) return;

    const change = createTempChange();
    writeEvalJson(change.name, [
      {
        phase: "01-requirements",
        timestamp: "2026-05-21T10:00:00.000Z",
        verdict: "pass",
        attempt: 1,
        report: "Requirements approved",
        items: [],
        backtrack_to: null,
        schema_version: "1.0",
      },
      {
        phase: "02-test-design",
        timestamp: "2026-05-21T11:00:00.000Z",
        verdict: "pass",
        attempt: 1,
        report: "Test design approved",
        items: [],
        backtrack_to: null,
        schema_version: "1.0",
      },
    ]);

    const { stdout, stderr, status } = runEvalCheck([
      "--change",
      change.name,
      "--phase",
      "03-dev-proposal",
    ]);

    change.cleanup();
    expect(status).toBe(0);
    expect(stdout).toContain("检查通过");
  });

  // -----------------------------------------------------------------------
  // AC-4/AC-5: --json mode outputs valid JSON with required fields
  // -----------------------------------------------------------------------
  it("AC-4/AC-5: --json mode outputs valid JSON with required fields", () => {
    if (!fs.existsSync(BUNDLE_PATH)) return;

    const change = createTempChange();
    writeEvalJson(change.name, [
      {
        phase: "01-requirements",
        timestamp: "2026-05-21T10:00:00.000Z",
        verdict: "pass",
        attempt: 1,
        report: "ok",
        items: [],
        backtrack_to: null,
        schema_version: "1.0",
      },
      {
        phase: "02-test-design",
        timestamp: "2026-05-21T11:00:00.000Z",
        verdict: "pass",
        attempt: 1,
        report: "ok",
        items: [],
        backtrack_to: null,
        schema_version: "1.0",
      },
    ]);

    const { stdout, stderr, status } = runEvalCheck([
      "--change",
      change.name,
      "--phase",
      "03-dev-proposal",
      "--json",
    ]);

    change.cleanup();
    expect(status).toBe(0);

    let parsed: any;
    expect(() => {
      parsed = JSON.parse(stdout);
    }).not.toThrow();

    expect(parsed).toHaveProperty("passed");
    expect(parsed).toHaveProperty("phase");
    expect(parsed).toHaveProperty("prior_phases");
    expect(parsed).toHaveProperty("block_reasons");
    expect(parsed).toHaveProperty("phase_state");
    expect(parsed).toHaveProperty("details");
    expect(typeof parsed.passed).toBe("boolean");
    expect(typeof parsed.phase).toBe("string");
    expect(Array.isArray(parsed.prior_phases)).toBe(true);
    expect(Array.isArray(parsed.block_reasons)).toBe(true);
    expect(["first_run", "retry", "passed"]).toContain(parsed.phase_state);
  });

  // -----------------------------------------------------------------------
  // AC-11: Non-existent change name -> meaningful error, exit code 1
  // -----------------------------------------------------------------------
  it("AC-11: non-existent change name outputs meaningful error and exits 1", () => {
    if (!fs.existsSync(BUNDLE_PATH)) return;

    const { stdout, stderr, status } = runEvalCheck([
      "--change",
      "non-existent-change",
      "--phase",
      "01-requirements",
    ]);

    expect(status).toBe(1);
    expect(stderr).toContain("错误");
    expect(stderr).toContain("目录不存在");
  });

  // -----------------------------------------------------------------------
  // Invalid phase name -> meaningful error, exit code 1
  // -----------------------------------------------------------------------
  it("invalid phase name outputs meaningful error and exits 1", () => {
    if (!fs.existsSync(BUNDLE_PATH)) return;

    const change = createTempChange();
    const { stdout, stderr, status } = runEvalCheck([
      "--change",
      change.name,
      "--phase",
      "invalid-phase",
    ]);

    change.cleanup();
    expect(status).toBe(1);
    expect(stderr).toContain("错误");
    expect(stderr).toContain("无效的阶段标识符");
  });

  // -----------------------------------------------------------------------
  // Missing --change argument -> exit code 1
  // -----------------------------------------------------------------------
  it("missing --change argument exits with code 1", () => {
    if (!fs.existsSync(BUNDLE_PATH)) return;

    const { stdout, stderr, status } = runEvalCheck([
      "--phase",
      "01-requirements",
    ]);

    expect(status).toBe(1);
    expect(stderr).toContain("错误");
  });

  // -----------------------------------------------------------------------
  // Missing --phase argument -> exit code 1
  // -----------------------------------------------------------------------
  it("missing --phase argument exits with code 1", () => {
    if (!fs.existsSync(BUNDLE_PATH)) return;

    const change = createTempChange();
    const { stdout, stderr, status } = runEvalCheck([
      "--change",
      change.name,
    ]);

    change.cleanup();
    expect(status).toBe(1);
    expect(stderr).toContain("错误");
  });

  // -----------------------------------------------------------------------
  // AC-10: first phase (01-requirements) passes with no prior phases
  // -----------------------------------------------------------------------
  it("AC-10: first phase (01-requirements) passes with no prior phases", () => {
    if (!fs.existsSync(BUNDLE_PATH)) return;

    const change = createTempChange();
    writeEvalJson(change.name, [
      {
        phase: "01-requirements",
        timestamp: "2026-05-21T10:00:00.000Z",
        verdict: "pass",
        attempt: 1,
        report: "Requirements approved",
        items: [],
        backtrack_to: null,
        schema_version: "1.0",
      },
    ]);

    const { stdout, stderr, status } = runEvalCheck([
      "--change",
      change.name,
      "--phase",
      "01-requirements",
    ]);

    change.cleanup();
    expect(status).toBe(0);
    expect(stdout).toContain("检查通过");
  });
});
