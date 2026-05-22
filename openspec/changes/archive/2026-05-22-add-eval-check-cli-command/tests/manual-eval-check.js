#!/usr/bin/env node

/**
 * Manual Verification Script for eval-check CLI command.
 *
 * Usage:
 *   1. Build the CLI: npm run build (from plugins/dev-team/bin/)
 *   2. Run this script: node manual-eval-check.js [--json]
 *
 * This script creates temporary change directories with various eval.json
 * scenarios and runs the eval-check CLI against each, printing results.
 *
 * --json flag: use JSON output mode for all scenarios.
 *
 * Scenarios:
 *   1. all-pass    - All prior phases pass, timestamps in order
 *   2. gate-fail   - Missing pass record for 02-test-design
 *   3. order-fail  - Timestamps in wrong order
 *   4. backtrack   - 01-requirements has active backtrack marker
 *   5. first-run   - No entries for current phase
 *   6. retry       - Current phase latest entry is fail
 *   7. passed      - Current phase latest entry is pass
 *   8. nonexistent - Non-existent change name
 *   9. bad-phase   - Invalid phase name
 */

const { execSync } = require("child_process");
const fs = require("fs");
const path = require("path");
const os = require("os");

// Determine paths
const scriptDir = __dirname;
const bundlePath = path.resolve(
  scriptDir,
  "../../../plugins/dev-team/bin/dev-team-bundle.js"
);

const useJson = process.argv.includes("--json");

// Helper: create temp dir with eval.json
function createScenario(name, entries) {
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), `eval-check-${name}-`));
  const phasesDir = path.join(tmpDir, "openspec", "changes", "test-change", "phases");
  fs.mkdirSync(phasesDir, { recursive: true });
  fs.writeFileSync(
    path.join(phasesDir, "eval.json"),
    JSON.stringify(entries, null, 2) + "\n"
  );
  return { tmpDir, changeName: "test-change" };
}

function cleanup(tmpDir) {
  if (tmpDir && fs.existsSync(tmpDir)) {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  }
}

function runCheck(changeName, phase, cwd) {
  const args = [
    "--change", changeName,
    "--phase", phase,
  ];
  if (useJson) args.push("--json");

  try {
    const result = execSync(
      `node "${bundlePath}" eval-check ${args.join(" ")}`,
      { encoding: "utf-8", cwd }
    );
    return { status: 0, stdout: result.trim(), stderr: "" };
  } catch (e) {
    return {
      status: e.status,
      stdout: (e.stdout || "").toString().trim(),
      stderr: (e.stderr || "").toString().trim(),
    };
  }
}

function printResult(scenario, result) {
  console.log(`\n${"=".repeat(60)}`);
  console.log(`Scenario: ${scenario}`);
  console.log(`-`.repeat(60));
  console.log(`Exit code: ${result.status}`);
  if (result.stdout) console.log(`stdout: ${result.stdout}`);
  if (result.stderr) console.log(`stderr: ${result.stderr}`);
  if (result.status === 0) {
    console.log("Verdict: PASS");
  } else {
    console.log("Verdict: BLOCKED");
  }
}

console.log(`eval-check Manual Verification`);
console.log(`Bundle: ${bundlePath}`);
console.log(`Mode: ${useJson ? "JSON" : "Human-readable"}`);
console.log(`Bundle exists: ${fs.existsSync(bundlePath)}`);

if (!fs.existsSync(bundlePath)) {
  console.error("\nERROR: Bundle not found. Run 'npm run build' first.");
  process.exit(1);
}

// === Scenario 1: All Pass ===
let s = createScenario("all-pass", [
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
printResult("1: All Pass (03-dev-proposal)", runCheck("test-change", "03-dev-proposal", s.tmpDir));
cleanup(s.tmpDir);

// === Scenario 2: Gate Fail ===
s = createScenario("gate-fail", [
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
    verdict: "fail",
    attempt: 1,
    report: "Test design rejected",
    items: [],
    backtrack_to: null,
    schema_version: "1.0",
  },
]);
printResult("2: Gate Fail (02-test-design missing pass)", runCheck("test-change", "03-dev-proposal", s.tmpDir));
cleanup(s.tmpDir);

// === Scenario 3: Order Fail ===
s = createScenario("order-fail", [
  {
    phase: "01-requirements",
    timestamp: "2026-05-21T12:00:00.000Z",
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
printResult("3: Order Fail (02-test-design earlier than 01-requirements)", runCheck("test-change", "03-dev-proposal", s.tmpDir));
cleanup(s.tmpDir);

// === Scenario 4: Backtrack ===
s = createScenario("backtrack", [
  {
    phase: "01-requirements",
    timestamp: "2026-05-21T10:00:00.000Z",
    verdict: "pass",
    attempt: 1,
    report: "Requirements approved",
    items: [],
    backtrack_to: "01-requirements",
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
printResult("4: Backtrack Active (01-requirements)", runCheck("test-change", "03-dev-proposal", s.tmpDir));
cleanup(s.tmpDir);

// === Scenario 5: First Run ===
s = createScenario("first-run", [
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
printResult("5: First Run (03-dev-proposal has no entries)", runCheck("test-change", "03-dev-proposal", s.tmpDir));
cleanup(s.tmpDir);

// === Scenario 6: Retry ===
s = createScenario("retry", [
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
  // Current phase: first attempt FAIL
  {
    phase: "03-dev-proposal",
    timestamp: "2026-05-21T12:00:00.000Z",
    verdict: "fail",
    attempt: 1,
    report: "Proposal rejected",
    items: [],
    backtrack_to: null,
    schema_version: "1.0",
  },
]);
printResult("6: Retry (03-dev-proposal latest = fail)", runCheck("test-change", "03-dev-proposal", s.tmpDir));
cleanup(s.tmpDir);

// === Scenario 7: Passed ===
s = createScenario("passed", [
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
  // Current phase: already PASSED
  {
    phase: "03-dev-proposal",
    timestamp: "2026-05-21T12:00:00.000Z",
    verdict: "pass",
    attempt: 1,
    report: "Proposal approved",
    items: [],
    backtrack_to: null,
    schema_version: "1.0",
  },
]);
printResult("7: Passed (03-dev-proposal latest = pass)", runCheck("test-change", "03-dev-proposal", s.tmpDir));
cleanup(s.tmpDir);

// === Scenario 8: Non-existent Change ===
printResult("8: Non-existent Change", runCheck("this-does-not-exist", "01-requirements", os.tmpdir()));

// === Scenario 9: Bad Phase ===
s = createScenario("bad-phase", []);
printResult("9: Invalid Phase", runCheck("test-change", "not-a-phase", s.tmpDir));
cleanup(s.tmpDir);

console.log(`\n${"=".repeat(60)}`);
console.log("Manual verification complete.");
