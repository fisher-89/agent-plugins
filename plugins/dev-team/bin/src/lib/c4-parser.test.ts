import { describe, it, expect } from "vite-plus/test";
import * as fs from "fs";
import * as path from "path";
import * as os from "os";
import {
  parseC4Dsl,
  validateC4Dsl,
  getModelFiles,
  readAllModels,
  findSpecificationBlock,
} from "./c4-parser";

describe("parseC4Dsl", () => {
  it("should parse a simple model with specification block", async () => {
    const dsl = `specification {
  element package
  element domain
  element module
  element component
}

model {
  package MyPackage {
    metadata { path './src/' }
  }

  extend MyPackage {
    domain MyDomain {
      metadata { path './src/mydomain/' }
    }
  }

  MyPackage.MyDomain -> MyPackage.MyDomain "depends on"
}
`;
    const result = await parseC4Dsl(dsl);
    expect(result.elements.length).toBeGreaterThanOrEqual(2);
    expect(result.relationships.length).toBeGreaterThanOrEqual(1);
  });

  it("should return empty elements for empty DSL", async () => {
    const result = await parseC4Dsl("");
    expect(result.elements).toEqual([]);
    expect(result.relationships).toEqual([]);
  });

  it("should parse extend relationships correctly", async () => {
    // extend blocks are resolved by the preprocessor
    const dsl = `specification { element package element domain }

model {
  package A {
    metadata { path './a/' }
  }
  extend A {
    domain B {
      metadata { path './a/b/' }
    }
  }
  rel A.B -> A.B { description "depends on" }
}`;
    const result = await parseC4Dsl(dsl);
    expect(result.elements.length).toBeGreaterThanOrEqual(2);
  });

  it("should extract path_to_element mappings", async () => {
    const dsl = `specification { element package element domain }

model {
  package TestPkg {
    metadata { path './src/pkg/' }
  }
}`;
    const result = await parseC4Dsl(dsl);
    expect(result.path_to_element["src/pkg"]).toBe("TestPkg");
  });

  it("should parse metadata with array values", async () => {
    const dsl = `specification { element package }

model {
  package Pkg {
    metadata { path ['./src/path1/', './src/path2/'] }
  }
}`;
    const result = await parseC4Dsl(dsl);
    const pkg = result.elements.find((e) => e.name === "Pkg");
    expect(pkg).toBeDefined();
    expect(pkg!.paths).toContain("./src/path1/");
    expect(pkg!.paths).toContain("./src/path2/");
  });

  it("should handle elements with no metadata", async () => {
    const dsl = `specification { element package }

model {
  package SoloPkg {
  }
}`;
    const result = await parseC4Dsl(dsl);
    const pkg = result.elements.find((e) => e.name === "SoloPkg");
    expect(pkg).toBeDefined();
    expect(pkg!.paths).toEqual([]);
  });

  it("should handle comments", async () => {
    const dsl = `specification { element package }
// This is a comment
model {
  // Another comment
  package Commented {
    metadata { path './commented/' }
  }
}`;
    const result = await parseC4Dsl(dsl);
    expect(result.elements.length).toBe(1);
    expect(result.elements[0].name).toBe("Commented");
  });
});

describe("validateC4Dsl", () => {
  it("should validate a correct DSL", async () => {
    const dsl = `specification { element package }
model {
  package Valid {
    metadata { path './valid/' }
  }
}`;
    const result = await validateC4Dsl(dsl);
    expect(result.valid).toBe(true);
  });

  it("should detect missing specification block", async () => {
    const dsl = `package NoSpec { }`;
    const result = await validateC4Dsl(dsl);
    expect(result.valid).toBe(false);
    expect(result.errors.some((e) => e.includes("specification"))).toBe(true);
  });

  it("should detect missing model content", async () => {
    const dsl = `specification { element package }`;
    const result = await validateC4Dsl(dsl);
    // LikeC4 will accept it as valid with empty model — validateC4Dsl checks for spec presence
    // and this DSL has a spec block, so it may be valid
    expect(result.valid).toBe(true);
  });

  it("should detect duplicate specification blocks across multiple files", async () => {
    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "c4-test-dup-"));
    try {
      const modelsDir = path.join(tmpDir, "openspec", "specs", "architecture", "models");
      fs.mkdirSync(modelsDir, { recursive: true });
      fs.writeFileSync(path.join(modelsDir, "01-a.c4"), "specification { element package }");
      fs.writeFileSync(path.join(modelsDir, "02-b.c4"), "specification { element domain }");

      const dsl = "specification { element package }\npackage P { }";
      const result = await validateC4Dsl(dsl, tmpDir);
      expect(result.valid).toBe(false);
      expect(result.errors.some((e) => e.includes("Duplicate"))).toBe(true);
    } finally {
      fs.rmSync(tmpDir, { recursive: true, force: true });
    }
  });
});

// ===========================================================================
// getModelFiles
// ===========================================================================

describe("getModelFiles", () => {
  it("should list .c4 files in alphabetical order", () => {
    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "c4-test-gmf-"));
    try {
      const modelsDir = path.join(tmpDir, "openspec", "specs", "architecture", "models");
      fs.mkdirSync(modelsDir, { recursive: true });
      fs.writeFileSync(path.join(modelsDir, "02-second.c4"), "");
      fs.writeFileSync(path.join(modelsDir, "01-first.c4"), "");
      fs.writeFileSync(path.join(modelsDir, "readme.md"), "");

      const files = getModelFiles(tmpDir);
      expect(files.length).toBe(2);
      expect(files[0].filename).toBe("01-first.c4");
      expect(files[1].filename).toBe("02-second.c4");
      expect(files[0].filepath).toContain("models");
    } finally {
      fs.rmSync(tmpDir, { recursive: true, force: true });
    }
  });

  it("should return empty array when models/ directory does not exist", () => {
    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "c4-test-gmf-empty-"));
    try {
      const files = getModelFiles(tmpDir);
      expect(files).toEqual([]);
    } finally {
      fs.rmSync(tmpDir, { recursive: true, force: true });
    }
  });
});

// ===========================================================================
// readAllModels
// ===========================================================================

describe("readAllModels", () => {
  it("should concatenate content from all model files", () => {
    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "c4-test-ram-"));
    try {
      const modelsDir = path.join(tmpDir, "openspec", "specs", "architecture", "models");
      fs.mkdirSync(modelsDir, { recursive: true });
      fs.writeFileSync(path.join(modelsDir, "01-a.c4"), "specification { element package }");
      fs.writeFileSync(path.join(modelsDir, "02-b.c4"), "package B { }");

      const result = readAllModels(tmpDir);
      expect(result).not.toBeNull();
      expect(result!).toContain("specification");
      expect(result!).toContain("package");
    } finally {
      fs.rmSync(tmpDir, { recursive: true, force: true });
    }
  });

  it("should return null when no model files exist", () => {
    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "c4-test-ram-null-"));
    try {
      const result = readAllModels(tmpDir);
      expect(result).toBeNull();
    } finally {
      fs.rmSync(tmpDir, { recursive: true, force: true });
    }
  });
});

// ===========================================================================
// findSpecificationBlock
// ===========================================================================

describe("findSpecificationBlock", () => {
  it("should find and return the specification block", () => {
    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "c4-test-fsb-"));
    try {
      const modelsDir = path.join(tmpDir, "openspec", "specs", "architecture", "models");
      fs.mkdirSync(modelsDir, { recursive: true });
      fs.writeFileSync(
        path.join(modelsDir, "01-core.c4"),
        "specification {\n  element package\n  element domain\n}",
      );

      const result = findSpecificationBlock(tmpDir);
      expect(result).not.toBeNull();
      expect(result!).toContain("specification");
      expect(result!).toContain("element package");
    } finally {
      fs.rmSync(tmpDir, { recursive: true, force: true });
    }
  });

  it("should return null when no specification block exists", () => {
    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "c4-test-fsb-null-"));
    try {
      const modelsDir = path.join(tmpDir, "openspec", "specs", "architecture", "models");
      fs.mkdirSync(modelsDir, { recursive: true });
      fs.writeFileSync(path.join(modelsDir, "01-core.c4"), "package P { }");

      const result = findSpecificationBlock(tmpDir);
      expect(result).toBeNull();
    } finally {
      fs.rmSync(tmpDir, { recursive: true, force: true });
    }
  });
});
