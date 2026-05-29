import { describe, it, expect } from "vite-plus/test";
import { PHASES, getPhaseIndex, getPriorPhases } from "../../../../plugins/dev-team/bin/src/lib/workflow";

// =============================================================================
// Unit tests for reordered PHASES array and helper functions
// =============================================================================
// Covers:
//   AC-1: PHASES array order is [01-requirements, 02-dev-design, 03-test-design, ...]
//   Boundary: old phase identifiers return empty / -1 (fault-tolerant)
//   Boundary: getPriorPhases edge cases (first, last, unknown)
// =============================================================================

describe("PHASES (reordered)", () => {
  it("AC-1: should have exactly 9 phases", () => {
    expect(PHASES.length).toBe(9);
  });

  it("AC-1: should have correct reordered sequence", () => {
    const expected = [
      "01-requirements",
      "02-dev-design",
      "03-test-design",
      "04-test-gen",
      "05-implement",
      "06-unit-test",
      "07-code-review",
      "08-integration-test",
      "09-acceptance",
    ];
    expect(PHASES).toEqual(expected);
  });

  it("AC-1: dev-design (02) should precede test-design (03)", () => {
    const devDesignIdx = PHASES.indexOf("02-dev-design");
    const testDesignIdx = PHASES.indexOf("03-test-design");
    expect(devDesignIdx).toBeLessThan(testDesignIdx);
  });

  it("should not contain old phase 02-test-design", () => {
    expect(PHASES).not.toContain("02-test-design");
  });

  it("should not contain old phase 03-dev-proposal", () => {
    expect(PHASES).not.toContain("03-dev-proposal");
  });

  it("should contain new phase 02-dev-design", () => {
    expect(PHASES).toContain("02-dev-design");
  });

  it("should contain new phase 03-test-design", () => {
    expect(PHASES).toContain("03-test-design");
  });

  it("should have renamed 05-implement (not 05-implementation)", () => {
    expect(PHASES).toContain("05-implement");
    expect(PHASES).not.toContain("05-implementation");
  });
});

describe("getPhaseIndex (reordered)", () => {
  it("should return 0 for 01-requirements (first phase)", () => {
    expect(getPhaseIndex("01-requirements")).toBe(0);
  });

  it("AC-1: should return 1 for 02-dev-design", () => {
    expect(getPhaseIndex("02-dev-design")).toBe(1);
  });

  it("AC-1: should return 2 for 03-test-design", () => {
    expect(getPhaseIndex("03-test-design")).toBe(2);
  });

  it("should return 3 for 04-test-gen", () => {
    expect(getPhaseIndex("04-test-gen")).toBe(3);
  });

  it("should return 4 for 05-implement", () => {
    expect(getPhaseIndex("05-implement")).toBe(4);
  });

  it("should return 5 for 06-unit-test", () => {
    expect(getPhaseIndex("06-unit-test")).toBe(5);
  });

  it("should return 6 for 07-code-review", () => {
    expect(getPhaseIndex("07-code-review")).toBe(6);
  });

  it("should return 7 for 08-integration-test", () => {
    expect(getPhaseIndex("08-integration-test")).toBe(7);
  });

  it("should return 8 for 09-acceptance (last phase)", () => {
    expect(getPhaseIndex("09-acceptance")).toBe(8);
  });

  it('should return -1 for unknown phase', () => {
    expect(getPhaseIndex("99-unknown")).toBe(-1);
  });

  it('should return -1 for old 02-test-design (fault-tolerant)', () => {
    expect(getPhaseIndex("02-test-design")).toBe(-1);
  });

  it('should return -1 for old 03-dev-proposal (fault-tolerant)', () => {
    expect(getPhaseIndex("03-dev-proposal")).toBe(-1);
  });
});

describe("getPriorPhases (reordered)", () => {
  it("should return empty array for first phase (01-requirements)", () => {
    expect(getPriorPhases("01-requirements")).toEqual([]);
  });

  it("AC-9: should return [01-requirements] for 02-dev-design", () => {
    expect(getPriorPhases("02-dev-design")).toEqual(["01-requirements"]);
  });

  it("AC-10: should return [01-requirements, 02-dev-design] for 03-test-design", () => {
    expect(getPriorPhases("03-test-design")).toEqual([
      "01-requirements",
      "02-dev-design",
    ]);
  });

  it("should return 4 prior phases for 05-implement", () => {
    const prior = getPriorPhases("05-implement");
    expect(prior).toEqual([
      "01-requirements",
      "02-dev-design",
      "03-test-design",
      "04-test-gen",
    ]);
  });

  it("should return 5 prior phases for 06-unit-test", () => {
    const prior = getPriorPhases("06-unit-test");
    expect(prior).toEqual([
      "01-requirements",
      "02-dev-design",
      "03-test-design",
      "04-test-gen",
      "05-implement",
    ]);
  });

  it("should return all 8 prior phases for 09-acceptance", () => {
    const prior = getPriorPhases("09-acceptance");
    expect(prior.length).toBe(8);
    expect(prior[prior.length - 1]).toBe("08-integration-test");
  });

  it("should return empty array for unknown phase (fault-tolerant)", () => {
    expect(getPriorPhases("99-unknown")).toEqual([]);
  });

  it("should return empty array for old 02-test-design (fault-tolerant)", () => {
    // Old identifier should not throw, just return empty
    expect(getPriorPhases("02-test-design")).toEqual([]);
  });

  it("should return empty array for old 03-dev-proposal (fault-tolerant)", () => {
    // Old identifier should not throw, just return empty
    expect(getPriorPhases("03-dev-proposal")).toEqual([]);
  });
});
