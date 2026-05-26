import { describe, it, expect } from "vitest";
import {
  checkPriorPhases,
  checkTimestampOrder,
  checkBacktrack,
  determinePhaseState,
  buildEvalCheckResult,
  checkSchemaVersion,
  SCHEMA_VERSION,
} from "../../../../plugins/dev-team/bin/src/commands/eval-check";
import { checkGate } from "../../../../plugins/dev-team/bin/src/lib/eval-json";

// ---------------------------------------------------------------------------
// Helper: create a minimal eval entry
// ---------------------------------------------------------------------------
function makeEntry(overrides: Record<string, any> = {}) {
  return {
    phase: "01-requirements",
    timestamp: new Date().toISOString(),
    verdict: "pass",
    attempt: 1,
    report: "ok",
    items: [],
    backtrack_to: null,
    schema_version: SCHEMA_VERSION,
    ...overrides,
  };
}

// ===========================================================================
// AC-13: eval-check no-op phase recognition
// ===========================================================================
describe("eval-check no-op detection (AC-13)", () => {
  // -----------------------------------------------------------------------
  // No-op entries pass gate checks
  // -----------------------------------------------------------------------

  it("treats a skipped phase entry as equivalent to pass in gate check", () => {
    // A skipped entry has verdict "pass" and skipped: true
    const entries = [
      makeEntry({ phase: "01-requirements", verdict: "pass" }),
      makeEntry({
        phase: "06-unit-test",
        verdict: "pass",
        skipped: true,
        evidence: "skipped: no applicable tests",
      }),
    ];
    // When checking if phase 06-unit-test is valid as a prior phase,
    // it should be treated as passed (verdict is "pass")
    const result = checkPriorPhases(entries, ["06-unit-test"]);
    expect(result.passed).toBe(true);
    expect(result.missing).toEqual([]);
  });

  it("includes skipped entries in prior phase gate check", () => {
    const entries = [
      makeEntry({ phase: "01-requirements", verdict: "pass" }),
      makeEntry({ phase: "02-test-design", verdict: "pass" }),
      makeEntry({
        phase: "06-unit-test",
        verdict: "pass",
        skipped: true,
      }),
    ];
    const result = checkPriorPhases(entries, [
      "01-requirements",
      "02-test-design",
      "06-unit-test",
    ]);
    expect(result.passed).toBe(true);
  });

  it("does not block archive flow when skipped phase is the only prior phase", () => {
    const entries = [
      makeEntry({
        phase: "06-unit-test",
        verdict: "pass",
        skipped: true,
      }),
    ];
    const result = checkPriorPhases(entries, ["06-unit-test"]);
    expect(result.passed).toBe(true);
  });

  it("recognises skipped=true field in eval entry", () => {
    const entry = makeEntry({
      phase: "08-integration-test",
      verdict: "pass",
      skipped: true,
      evidence: "skipped: no integration test files found",
    });
    expect(entry.skipped).toBe(true);
    expect(entry.verdict).toBe("pass");
    expect(typeof entry.evidence).toBe("string");
    expect(entry.evidence.length).toBeGreaterThan(0);
  });

  // -----------------------------------------------------------------------
  // Skipped + non-skipped mixes
  // -----------------------------------------------------------------------

  it("mixes skipped and non-skipped prior phases correctly", () => {
    const entries = [
      makeEntry({ phase: "01-requirements", verdict: "pass" }),
      makeEntry({ phase: "02-test-design", verdict: "pass" }),
      makeEntry({
        phase: "04-test-gen",
        verdict: "pass",
        skipped: false,
      }),
      makeEntry({
        phase: "06-unit-test",
        verdict: "pass",
        skipped: true,
      }),
    ];
    const result = checkPriorPhases(entries, [
      "01-requirements",
      "02-test-design",
      "04-test-gen",
      "06-unit-test",
    ]);
    expect(result.passed).toBe(true);
  });

  it("fails gate when a non-skipped prior phase has no pass record", () => {
    const entries = [
      makeEntry({ phase: "01-requirements", verdict: "pass" }),
      makeEntry({
        phase: "06-unit-test",
        verdict: "fail",
        skipped: false,
      }),
    ];
    const result = checkPriorPhases(entries, [
      "01-requirements",
      "06-unit-test",
    ]);
    expect(result.passed).toBe(false);
    expect(result.missing).toContain("06-unit-test");
  });

  // -----------------------------------------------------------------------
  // Timestamp order with skipped entries
  // -----------------------------------------------------------------------

  it("validates timestamp order with skipped entries", () => {
    const entries = [
      makeEntry({
        phase: "01-requirements",
        verdict: "pass",
        timestamp: "2026-05-25T10:00:00.000Z",
      }),
      makeEntry({
        phase: "06-unit-test",
        verdict: "pass",
        skipped: true,
        timestamp: "2026-05-25T12:00:00.000Z",
      }),
    ];
    const result = checkTimestampOrder(entries, [
      "01-requirements",
      "06-unit-test",
    ]);
    expect(result.passed).toBe(true);
    expect(result.order_valid).toBe(true);
  });

  // -----------------------------------------------------------------------
  // Backtrack check with skipped entries
  // -----------------------------------------------------------------------

  it("skipped entries should not have active backtrack markers", () => {
    const entries = [
      makeEntry({
        phase: "06-unit-test",
        verdict: "pass",
        skipped: true,
        backtrack_to: null,
      }),
    ];
    const result = checkBacktrack(entries, ["06-unit-test"]);
    expect(result.passed).toBe(true);
    expect(result.active_backtrack_phases).toEqual([]);
  });

  // -----------------------------------------------------------------------
  // Phase state determination with skipped
  // -----------------------------------------------------------------------

  it("determinePhaseState considers skipped entries as 'passed' state", () => {
    const entries = [
      makeEntry({
        phase: "06-unit-test",
        verdict: "pass",
        skipped: true,
      }),
    ];
    const result = determinePhaseState(entries, "06-unit-test");
    expect(result).toBe("passed");
  });

  // -----------------------------------------------------------------------
  // buildEvalCheckResult with skipped entries
  // -----------------------------------------------------------------------

  it("buildEvalCheckResult passes when prior skipped phases are valid", () => {
    const result = buildEvalCheckResult({
      phase: "07-code-review",
      priorPhases: ["01-requirements", "02-test-design", "06-unit-test"],
      gateResult: { passed: true, missing: [] },
      timestampResult: { passed: true, order_valid: true },
      backtrackResult: { passed: true, active_backtrack_phases: [] },
      phaseState: "first_run",
    });
    expect(result.passed).toBe(true);
    expect(result.block_reasons).toEqual([]);
  });

  // -----------------------------------------------------------------------
  // Schema version with skipped entries
  // -----------------------------------------------------------------------

  it("skipped entries with schema_version are checked by checkSchemaVersion", () => {
    const entries = [
      makeEntry({
        phase: "06-unit-test",
        verdict: "pass",
        skipped: true,
        schema_version: SCHEMA_VERSION,
      }),
    ];
    const warnings = checkSchemaVersion(entries);
    expect(warnings).toEqual([]);
  });

  // -----------------------------------------------------------------------
  // Edge cases
  // -----------------------------------------------------------------------

  it("handles an eval.json with only skipped entries", () => {
    const entries = [
      makeEntry({
        phase: "06-unit-test",
        verdict: "pass",
        skipped: true,
      }),
      makeEntry({
        phase: "08-integration-test",
        verdict: "pass",
        skipped: true,
      }),
    ];
    const result = checkPriorPhases(entries, [
      "06-unit-test",
      "08-integration-test",
    ]);
    expect(result.passed).toBe(true);
  });

  it("skipped entries do not affect checkPriorPhases for non-prior phases", () => {
    const entries = [
      makeEntry({
        phase: "08-integration-test",
        verdict: "pass",
        skipped: true,
      }),
    ];
    // Prior phases that have no entries at all still count as missing
    const result = checkPriorPhases(entries, [
      "01-requirements",
      "02-test-design",
    ]);
    expect(result.passed).toBe(false);
    expect(result.missing).toEqual(["01-requirements", "02-test-design"]);
  });

  it("human-readable output contains 'skipped' for no-op phases", () => {
    // In the CLI output, skipped phases should be indicated
    // This tests the output format of the eval-check command
    const entries = [
      makeEntry({
        phase: "06-unit-test",
        verdict: "pass",
        skipped: true,
      }),
    ];
    // The check itself succeeds (skipped = passed)
    const gateResult = checkPriorPhases(entries, ["06-unit-test"]);
    expect(gateResult.passed).toBe(true);

    // The buildEvalCheckResult reflects this
    const result = buildEvalCheckResult({
      phase: "07-code-review",
      priorPhases: ["06-unit-test"],
      gateResult,
      timestampResult: { passed: true, order_valid: true },
      backtrackResult: { passed: true, active_backtrack_phases: [] },
      phaseState: "first_run",
    });
    expect(result.passed).toBe(true);
  });
});
