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
// checkPriorPhases
// ===========================================================================
describe("checkPriorPhases", () => {
  it("returns passed=true when all prior phases have pass records", () => {
    const entries = [
      makeEntry({ phase: "01-requirements", verdict: "pass" }),
      makeEntry({ phase: "02-test-design", verdict: "pass" }),
    ];
    const result = checkPriorPhases(entries, [
      "01-requirements",
      "02-test-design",
    ]);
    expect(result.passed).toBe(true);
    expect(result.missing).toEqual([]);
  });

  it("returns passed=false when a prior phase is missing a pass record", () => {
    const entries = [
      makeEntry({ phase: "01-requirements", verdict: "pass" }),
      // 02-test-design has only a fail record
      makeEntry({ phase: "02-test-design", verdict: "fail" }),
    ];
    const result = checkPriorPhases(entries, [
      "01-requirements",
      "02-test-design",
    ]);
    expect(result.passed).toBe(false);
    expect(result.missing).toEqual(["02-test-design"]);
  });

  it("returns passed=false when all prior phases are missing", () => {
    const entries: any[] = [];
    const result = checkPriorPhases(entries, [
      "01-requirements",
      "02-test-design",
    ]);
    expect(result.passed).toBe(false);
    expect(result.missing).toEqual(["01-requirements", "02-test-design"]);
  });

  it("returns passed=false with empty entries array", () => {
    const result = checkPriorPhases([], ["01-requirements"]);
    expect(result.passed).toBe(false);
    expect(result.missing).toEqual(["01-requirements"]);
  });

  it("returns passed=true when there are no prior phases (first phase)", () => {
    const result = checkPriorPhases([], []);
    expect(result.passed).toBe(true);
    expect(result.missing).toEqual([]);
  });

  it("delegates to checkGate and produces same output", () => {
    const entries = [
      makeEntry({ phase: "01-requirements", verdict: "pass" }),
      makeEntry({ phase: "03-dev-proposal", verdict: "pass" }),
    ];
    const priorPhases = ["01-requirements", "02-test-design"];
    const result = checkPriorPhases(entries, priorPhases);
    const expected = checkGate(entries, priorPhases);
    expect(result).toEqual(expected);
  });
});

// ===========================================================================
// checkTimestampOrder
// ===========================================================================
describe("checkTimestampOrder", () => {
  it("returns passed=true when timestamps are monotonically increasing", () => {
    const entries = [
      makeEntry({
        phase: "01-requirements",
        verdict: "pass",
        timestamp: "2026-05-21T10:00:00.000Z",
      }),
      makeEntry({
        phase: "02-test-design",
        verdict: "pass",
        timestamp: "2026-05-21T11:00:00.000Z",
      }),
    ];
    const result = checkTimestampOrder(entries, [
      "01-requirements",
      "02-test-design",
    ]);
    expect(result.passed).toBe(true);
    expect(result.order_valid).toBe(true);
  });

  it("returns passed=false when timestamps are in reverse order", () => {
    const entries = [
      makeEntry({
        phase: "01-requirements",
        verdict: "pass",
        timestamp: "2026-05-21T12:00:00.000Z",
      }),
      makeEntry({
        phase: "02-test-design",
        verdict: "pass",
        timestamp: "2026-05-21T11:00:00.000Z",
      }),
    ];
    const result = checkTimestampOrder(entries, [
      "01-requirements",
      "02-test-design",
    ]);
    expect(result.passed).toBe(false);
    expect(result.order_valid).toBe(false);
    expect(result.issues).toBeDefined();
    expect(result.issues!.length).toBeGreaterThan(0);
  });

  it("returns passed=true when timestamps are identical (>= comparison)", () => {
    const entries = [
      makeEntry({
        phase: "01-requirements",
        verdict: "pass",
        timestamp: "2026-05-21T10:00:00.000Z",
      }),
      makeEntry({
        phase: "02-test-design",
        verdict: "pass",
        timestamp: "2026-05-21T10:00:00.000Z",
      }),
    ];
    const result = checkTimestampOrder(entries, [
      "01-requirements",
      "02-test-design",
    ]);
    expect(result.passed).toBe(true);
    expect(result.order_valid).toBe(true);
  });

  it("returns passed=true when there is only one prior phase", () => {
    const entries = [
      makeEntry({
        phase: "01-requirements",
        verdict: "pass",
        timestamp: "2026-05-21T10:00:00.000Z",
      }),
    ];
    const result = checkTimestampOrder(entries, ["01-requirements"]);
    expect(result.passed).toBe(true);
    expect(result.order_valid).toBe(true);
  });

  it("returns passed=true when there are no prior phases", () => {
    const result = checkTimestampOrder([], []);
    expect(result.passed).toBe(true);
    expect(result.order_valid).toBe(true);
  });

  it("skips phases without pass entries and still validates remaining", () => {
    const entries = [
      makeEntry({
        phase: "01-requirements",
        verdict: "pass",
        timestamp: "2026-05-21T10:00:00.000Z",
      }),
      // 02-test-design has no pass records
      makeEntry({
        phase: "03-dev-proposal",
        verdict: "pass",
        timestamp: "2026-05-21T09:00:00.000Z",
      }),
    ];
    // Without 02's pass, 03 comes after 01 but has earlier timestamp
    const result = checkTimestampOrder(entries, [
      "01-requirements",
      "02-test-design",
      "03-dev-proposal",
    ]);
    expect(result.passed).toBe(false);
    expect(result.order_valid).toBe(false);
  });

  it("uses the latest pass record for each phase", () => {
    const entries = [
      // 01-requirements: first attempt pass, later retry (should use later ts)
      makeEntry({
        phase: "01-requirements",
        verdict: "pass",
        timestamp: "2026-05-21T10:00:00.000Z",
      }),
      makeEntry({
        phase: "01-requirements",
        verdict: "fail",
        timestamp: "2026-05-21T11:00:00.000Z",
      }),
      makeEntry({
        phase: "01-requirements",
        verdict: "pass",
        timestamp: "2026-05-21T12:00:00.000Z",
      }),
      // 02-test-design passes after 01's last pass
      makeEntry({
        phase: "02-test-design",
        verdict: "pass",
        timestamp: "2026-05-21T13:00:00.000Z",
      }),
    ];
    const result = checkTimestampOrder(entries, [
      "01-requirements",
      "02-test-design",
    ]);
    expect(result.passed).toBe(true);
    expect(result.order_valid).toBe(true);
  });
});

// ===========================================================================
// checkBacktrack
// ===========================================================================
describe("checkBacktrack", () => {
  it("returns passed=true and empty list when no backtrack markers", () => {
    const entries = [
      makeEntry({ phase: "01-requirements", backtrack_to: null }),
      makeEntry({ phase: "02-test-design", backtrack_to: null }),
    ];
    const result = checkBacktrack(entries, [
      "01-requirements",
      "02-test-design",
    ]);
    expect(result.passed).toBe(true);
    expect(result.active_backtrack_phases).toEqual([]);
  });

  it("returns passed=false when a prior phase has active backtrack marker", () => {
    const entries = [
      makeEntry({
        phase: "01-requirements",
        backtrack_to: "01-requirements",
      }),
      makeEntry({ phase: "02-test-design", backtrack_to: null }),
    ];
    const result = checkBacktrack(entries, [
      "01-requirements",
      "02-test-design",
    ]);
    expect(result.passed).toBe(false);
    expect(result.active_backtrack_phases).toEqual(["01-requirements"]);
  });

  it("returns passed=true when backtrack was cleared by a later entry", () => {
    const entries = [
      // Older entry with backtrack
      makeEntry({
        phase: "01-requirements",
        backtrack_to: "01-requirements",
        timestamp: "2026-05-21T10:00:00.000Z",
      }),
      // Newer entry clears backtrack (null)
      makeEntry({
        phase: "01-requirements",
        backtrack_to: null,
        timestamp: "2026-05-21T11:00:00.000Z",
      }),
    ];
    const result = checkBacktrack(entries, ["01-requirements"]);
    expect(result.passed).toBe(true);
    expect(result.active_backtrack_phases).toEqual([]);
  });

  it("handles backtrack_to as empty string (treated as no backtrack)", () => {
    const entries = [
      makeEntry({ phase: "01-requirements", backtrack_to: "" }),
    ];
    const result = checkBacktrack(entries, ["01-requirements"]);
    expect(result.passed).toBe(true);
  });

  it("handles phases with no entries at all", () => {
    const result = checkBacktrack([], ["01-requirements"]);
    expect(result.passed).toBe(true);
    expect(result.active_backtrack_phases).toEqual([]);
  });
});

// ===========================================================================
// determinePhaseState
// ===========================================================================
describe("determinePhaseState", () => {
  it("returns first_run when there are no entries for the phase", () => {
    const result = determinePhaseState([], "03-dev-proposal");
    expect(result).toBe("first_run");
  });

  it("returns first_run when entries exist for other phases but not current", () => {
    const entries = [
      makeEntry({ phase: "01-requirements", verdict: "pass" }),
    ];
    const result = determinePhaseState(entries, "03-dev-proposal");
    expect(result).toBe("first_run");
  });

  it("returns retry when the latest entry verdict is fail", () => {
    const entries = [
      makeEntry({
        phase: "03-dev-proposal",
        verdict: "pass",
        timestamp: "2026-05-21T10:00:00.000Z",
      }),
      makeEntry({
        phase: "03-dev-proposal",
        verdict: "fail",
        timestamp: "2026-05-21T11:00:00.000Z",
      }),
    ];
    const result = determinePhaseState(entries, "03-dev-proposal");
    expect(result).toBe("retry");
  });

  it("returns passed when the latest entry verdict is pass", () => {
    const entries = [
      makeEntry({
        phase: "03-dev-proposal",
        verdict: "fail",
        timestamp: "2026-05-21T10:00:00.000Z",
      }),
      makeEntry({
        phase: "03-dev-proposal",
        verdict: "pass",
        timestamp: "2026-05-21T11:00:00.000Z",
      }),
    ];
    const result = determinePhaseState(entries, "03-dev-proposal");
    expect(result).toBe("passed");
  });

  it("uses the most recent entry when multiple entries exist", () => {
    const entries = [
      makeEntry({
        phase: "03-dev-proposal",
        verdict: "pass",
        timestamp: "2026-05-21T10:00:00.000Z",
      }),
      makeEntry({
        phase: "03-dev-proposal",
        verdict: "pass",
        timestamp: "2026-05-21T11:00:00.000Z",
      }),
      makeEntry({
        phase: "03-dev-proposal",
        verdict: "fail",
        timestamp: "2026-05-21T12:00:00.000Z",
      }),
    ];
    const result = determinePhaseState(entries, "03-dev-proposal");
    expect(result).toBe("retry");
  });
});

// ===========================================================================
// buildEvalCheckResult
// ===========================================================================
describe("buildEvalCheckResult", () => {
  const baseOptions = {
    phase: "03-dev-proposal",
    priorPhases: ["01-requirements", "02-test-design"],
    gateResult: { passed: true, missing: [] },
    timestampResult: { passed: true, order_valid: true },
    backtrackResult: { passed: true, active_backtrack_phases: [] },
    phaseState: "first_run" as const,
  };

  it("returns passed=true with empty block_reasons when all checks pass", () => {
    const result = buildEvalCheckResult(baseOptions);
    expect(result.passed).toBe(true);
    expect(result.phase).toBe("03-dev-proposal");
    expect(result.prior_phases).toEqual(["01-requirements", "02-test-design"]);
    expect(result.block_reasons).toEqual([]);
    expect(result.phase_state).toBe("first_run");
    expect(result.details.prior_phase_gate.passed).toBe(true);
    expect(result.details.timestamp_order.passed).toBe(true);
    expect(result.details.backtrack.passed).toBe(true);
  });

  it("includes gate failure in block_reasons", () => {
    const result = buildEvalCheckResult({
      ...baseOptions,
      gateResult: { passed: false, missing: ["02-test-design"] },
    });
    expect(result.passed).toBe(false);
    expect(result.block_reasons.length).toBeGreaterThanOrEqual(1);
    expect(result.block_reasons[0]).toContain("门控");
    expect(result.block_reasons[0]).toContain("02-test-design");
  });

  it("includes timestamp failure in block_reasons", () => {
    const result = buildEvalCheckResult({
      ...baseOptions,
      timestampResult: {
        passed: false,
        order_valid: false,
        issues: ["02-test-design (T2) timestamp is earlier than 01-requirements (T1)"],
      },
    });
    expect(result.passed).toBe(false);
    expect(result.block_reasons.length).toBeGreaterThanOrEqual(1);
    expect(result.block_reasons[0]).toContain("时间戳");
  });

  it("includes backtrack failure in block_reasons", () => {
    const result = buildEvalCheckResult({
      ...baseOptions,
      backtrackResult: {
        passed: false,
        active_backtrack_phases: ["01-requirements"],
      },
    });
    expect(result.passed).toBe(false);
    expect(result.block_reasons.length).toBeGreaterThanOrEqual(1);
    expect(result.block_reasons[0]).toContain("backtrack");
    expect(result.block_reasons[0]).toContain("01-requirements");
  });

  it("aggregates multiple failure reasons", () => {
    const result = buildEvalCheckResult({
      ...baseOptions,
      gateResult: { passed: false, missing: ["02-test-design"] },
      timestampResult: {
        passed: false,
        order_valid: false,
        issues: ["02-test-design timestamp is earlier than 01-requirements"],
      },
      backtrackResult: {
        passed: false,
        active_backtrack_phases: ["01-requirements"],
      },
    });
    expect(result.passed).toBe(false);
    expect(result.block_reasons.length).toBe(3);
  });

  it("passes through phase_state correctly", () => {
    const retryResult = buildEvalCheckResult({
      ...baseOptions,
      phaseState: "retry",
    });
    expect(retryResult.phase_state).toBe("retry");

    const passedResult = buildEvalCheckResult({
      ...baseOptions,
      phaseState: "passed",
    });
    expect(passedResult.phase_state).toBe("passed");
  });

  it("contains all required top-level fields", () => {
    const result = buildEvalCheckResult(baseOptions);
    const keys = Object.keys(result);
    expect(keys).toContain("passed");
    expect(keys).toContain("phase");
    expect(keys).toContain("prior_phases");
    expect(keys).toContain("block_reasons");
    expect(keys).toContain("phase_state");
    expect(keys).toContain("details");
  });
});

// ===========================================================================
// checkSchemaVersion
// ===========================================================================
describe("checkSchemaVersion", () => {
  it("returns empty warnings when all entries match SCHEMA_VERSION", () => {
    const entries = [
      makeEntry({ schema_version: SCHEMA_VERSION }),
      makeEntry({ schema_version: SCHEMA_VERSION }),
    ];
    const warnings = checkSchemaVersion(entries);
    expect(warnings).toEqual([]);
  });

  it("returns warnings when entries have mismatched schema_version", () => {
    const entries = [
      makeEntry({ schema_version: "2.0" }),
    ];
    const warnings = checkSchemaVersion(entries);
    expect(warnings.length).toBeGreaterThan(0);
    expect(warnings[0]).toContain("2.0");
    expect(warnings[0]).toContain(SCHEMA_VERSION);
  });

  it("does not warn for entries without schema_version field", () => {
    const entries = [
      { phase: "01-requirements", verdict: "pass" },
    ];
    const warnings = checkSchemaVersion(entries);
    expect(warnings).toEqual([]);
  });

  it("deduplicates warnings for multiple entries with same mismatched version", () => {
    const entries = [
      makeEntry({ schema_version: "2.0" }),
      makeEntry({ schema_version: "2.0" }),
    ];
    const warnings = checkSchemaVersion(entries);
    expect(warnings.length).toBe(1);
  });

  it("uses custom expected version when provided", () => {
    const entries = [
      makeEntry({ schema_version: "2.0" }),
    ];
    const warnings = checkSchemaVersion(entries, "2.0");
    expect(warnings).toEqual([]);
  });
});
