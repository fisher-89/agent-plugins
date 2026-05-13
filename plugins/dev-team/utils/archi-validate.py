#!/usr/bin/env python3
"""
archi-validate.py — Architecture validation engine.

Validates code changes against the C4 architecture model by:
1. Parsing the model for metadata.path → element mappings
2. Extracting import statements from changed files (TS/JS/Python)
3. Cross-referencing imports against declared model relationships
4. Generating a JSON validation report

Usage:
  python archi-validate.py --project-root <path> [--staged] [--files <a,b,c>]
"""

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
from datetime import datetime, timezone


def resolve_project_root():
    """Find project root by locating openspec/ or .git."""
    cwd = os.getcwd()
    for start in [cwd] + list(_walk_up(cwd)):
        if os.path.isdir(os.path.join(start, "openspec")):
            return start
        if os.path.isdir(os.path.join(start, ".git")):
            return start
    return cwd


def _walk_up(path):
    while True:
        parent = os.path.dirname(path)
        if parent == path:
            break
        yield parent
        path = parent


def load_model(project_root):
    """Load and parse the architecture model from model.c4.

    Returns:
        dict with:
        - elements: list of {name, kind, paths}
        - relationships: list of {source, target, description}
        - path_to_element: dict mapping normalized paths to element names
        - errors: list of parse errors
    """
    model_path = os.path.join(project_root, "openspec", "architecture", "model.c4")
    result = {
        "elements": [],
        "relationships": [],
        "path_to_element": {},
        "errors": [],
    }

    if not os.path.isfile(model_path):
        result["errors"].append("Model file not found: " + model_path)
        return result

    with open(model_path, "r", encoding="utf-8") as f:
        dsl = f.read()

    return _parse_model_dsl(dsl, project_root)


def _parse_model_dsl(dsl, project_root):
    """Parse C4 DSL to extract elements, relationships, and path mappings."""
    result = {
        "elements": [],
        "relationships": [],
        "path_to_element": {},
        "errors": [],
    }

    lines = dsl.split("\n")
    current_element = None
    in_model = False
    in_specification = True

    for line in lines:
        stripped = line.strip()

        if not stripped or stripped.startswith("//") or stripped.startswith("#"):
            continue

        if stripped == "model {":
            in_model = True
            in_specification = False
            continue
        if stripped == "}":
            if current_element:
                result["elements"].append(current_element)
                # Build path_to_element mapping
                for p in current_element.get("paths", []):
                    result["path_to_element"][_normalize_path(p)] = current_element["name"]
                current_element = None
            continue

        if not in_model:
            continue

        # Relationship: A -> B "description"
        if "->" in stripped and current_element is None:
            rel = _parse_relationship(stripped)
            if rel:
                result["relationships"].append(rel)
            continue

        # Element definition
        elem = _try_parse_element(stripped)
        if elem:
            if current_element:
                result["elements"].append(current_element)
                for p in current_element.get("paths", []):
                    result["path_to_element"][_normalize_path(p)] = current_element["name"]
            current_element = elem
            continue

        # Metadata path inside element
        if current_element and stripped.startswith("metadata path"):
            paths = _parse_metadata_path(stripped)
            current_element["paths"] = paths
            continue

    # Last element
    if current_element:
        result["elements"].append(current_element)
        for p in current_element.get("paths", []):
            result["path_to_element"][_normalize_path(p)] = current_element["name"]

    return result


def _try_parse_element(line):
    """Try to parse an element definition from a DSL line."""
    for kind in ["softwareSystem", "component", "container", "system"]:
        prefix = kind + " "
        if line.startswith(prefix):
            name = line[len(prefix):].rstrip("{").strip().rstrip()
            return {"kind": kind, "name": name, "paths": []}
    return None


def _parse_metadata_path(line):
    """Parse metadata path value from a DSL line."""
    path_str = line[len("metadata path"):].strip()
    return _parse_path_array(path_str)


def _parse_path_array(path_str):
    """Parse a path value (single string or array)."""
    path_str = path_str.strip()
    if path_str.startswith("["):
        path_str = path_str[1:].rstrip("]").strip()
        paths = []
        for p in path_str.split(","):
            p = p.strip().strip('"').strip("'")
            if p:
                paths.append(p)
        return paths
    else:
        p = path_str.strip('"').strip("'")
        return [p] if p else []


def _parse_relationship(line):
    """Parse a relationship: A -> B "description"."""
    arrow_idx = line.find("->")
    if arrow_idx < 0:
        return None

    source = line[:arrow_idx].strip()
    rest = line[arrow_idx + 2:].strip()

    # Extract target (may have description in quotes)
    target = rest.split()[0] if rest else rest
    target = target.rstrip("{").strip()

    description = None
    desc_match = re.search(r'"([^"]*)"', line)
    if desc_match:
        description = desc_match.group(1)

    return {"source": source, "target": target, "description": description}


def _normalize_path(path):
    """Normalize a path for comparison. Strips ./ prefix and trailing slash."""
    path = path.strip()
    if path.startswith("./"):
        path = path[2:]
    return path.rstrip("/").rstrip("\\")


def get_changed_files(project_root, staged=False, files=None):
    """Get list of changed files.

    Args:
        project_root: Project root directory
        staged: If True, get `git diff --cached` files
        files: Optional list of specific files to check

    Returns:
        List of file paths relative to project_root
    """
    if files:
        return files

    if staged:
        try:
            result = subprocess.run(
                ["git", "diff", "--cached", "--name-only"],
                cwd=project_root,
                capture_output=True,
                text=True,
                timeout=10,
                shell=True,
            )
            if result.returncode == 0:
                return [f.strip() for f in result.stdout.splitlines() if f.strip()]
        except (subprocess.TimeoutExpired, OSError, FileNotFoundError):
            pass

    return []


def map_files_to_elements(files, path_to_element):
    """Map each file to a model element via metadata.path matching.

    A path pointing to a directory matches all files within it recursively.
    A path pointing to a file matches only that exact file.

    Returns:
        dict: {file_path: element_name or None}
    """
    mapping = {}

    for filepath in files:
        norm_file = _normalize_path(filepath)
        matched = None

        # Check all registered paths for matches
        for elem_path, elem_name in path_to_element.items():
            norm_elem = _normalize_path(elem_path)

            # Check if file is under a directory path
            if norm_file.startswith(norm_elem + "/") or norm_file.startswith(norm_elem + "\\"):
                matched = elem_name
                break

            # Check if file exactly matches a file path
            if norm_file == norm_elem:
                matched = elem_name
                break

            # Check if norm_elem is a directory and file is a subpath
            if "/" in norm_elem or "\\" in norm_elem:
                elem_dir = norm_elem
                if norm_file.startswith(elem_dir + "/") or norm_file.startswith(elem_dir + "\\"):
                    matched = elem_name
                    break

        mapping[filepath] = matched

    return mapping


def parse_imports(filepath, project_root):
    """Parse import statements from a source file.

    Supports TypeScript/JavaScript and Python import syntax.

    Returns:
        List of import target strings (module paths)
    """
    full_path = os.path.join(project_root, filepath)
    if not os.path.isfile(full_path):
        return []

    try:
        with open(full_path, "r", encoding="utf-8", errors="ignore") as f:
            content = f.read()
    except OSError:
        return []

    ext = os.path.splitext(filepath)[1].lower()
    imports = []

    if ext in (".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs"):
        imports = _parse_js_imports(content)
    elif ext in (".py", ".pyi"):
        imports = _parse_python_imports(content)

    return imports


def _parse_js_imports(content):
    """Parse JavaScript/TypeScript import statements."""
    imports = []

    # import { x } from './path'
    # import x from './path'
    # import './path'
    # import * as x from './path'
    pattern = re.compile(
        r'import\s+(?:type\s+)?(?:(?:\{[^}]*\}|[\w*\s,]+)\s+from\s+)?[\'"]([^\'"]+)[\'"]',
        re.MULTILINE,
    )
    for match in pattern.finditer(content):
        target = match.group(1)
        if target and (target.startswith("./") or target.startswith("..") or target.startswith("@/")):
            imports.append(target)

    # require('./path')
    require_pattern = re.compile(r'require\s*\(\s*[\'"]([^\'"]+)[\'"]\s*\)')
    for match in require_pattern.finditer(content):
        target = match.group(1)
        if target and (target.startswith("./") or target.startswith("..") or target.startswith("@/")):
            imports.append(target)

    return imports


def _parse_python_imports(content):
    """Parse Python import statements."""
    imports = []

    # from .module import X
    # from ..module import X
    # from package.module import X
    pattern = re.compile(r'^from\s+(\S+)\s+import', re.MULTILINE)
    for match in pattern.finditer(content):
        target = match.group(1)
        if target.startswith("."):
            imports.append(target)
        else:
            # Only track first-party imports (not stdlib)
            top = target.split(".")[0]
            if top not in _PYTHON_STDLIB:
                imports.append(target)

    return imports


# Common Python stdlib top-level modules (non-exhaustive)
_PYTHON_STDLIB = {
    "os", "sys", "re", "json", "math", "time", "datetime", "collections",
    "itertools", "functools", "typing", "io", "pathlib", "shutil", "subprocess",
    "argparse", "logging", "unittest", "abc", "base64", "hashlib", "random",
    "threading", "multiprocessing", "asyncio", "socket", "http", "urllib",
    "xml", "html", "csv", "configparser", "copy", "enum", "gc", "inspect",
    "struct", "tempfile", "textwrap", "traceback", "uuid", "warnings", "zipfile",
}


def cross_reference(imports_by_file, file_element_map, elements, relationships, path_to_element, project_root):
    """Cross-reference imports against model relationships.

    Args:
        imports_by_file: {filepath: [import_targets]}
        file_element_map: {filepath: element_name}
        elements: list of element dicts
        relationships: list of relationship dicts
        path_to_element: {normalized_path: element_name}

    Returns:
        (violations, warnings) — two lists of findings
    """
    violations = []
    warnings = []

    # Build relationship lookup: (source, target) -> relationship
    rel_lookup = set()
    for rel in relationships:
        if rel.get("source") and rel.get("target"):
            rel_lookup.add((rel["source"], rel["target"]))

    for filepath, import_targets in imports_by_file.items():
        source_elem = file_element_map.get(filepath)
        if not source_elem:
            continue

        for imp in import_targets:
            # Resolve import target to a file path
            resolved = _resolve_import_to_path(filepath, imp)
            if resolved is None:
                continue

            # Map resolved path to element
            target_elem = _match_path_to_element(resolved, path_to_element)

            # Self-import within same element — ignore
            if target_elem == source_elem:
                continue

            if target_elem is None:
                # Import target not mapped to any element
                warnings.append({
                    "type": "unmapped_import_target",
                    "source_file": filepath,
                    "import_target": imp,
                    "detail": f"Import '{imp}' resolves to '{resolved}' which is not mapped to any model element",
                })
                continue

            # Check if relationship exists
            if (source_elem, target_elem) not in rel_lookup:
                violations.append({
                    "type": "unmodeled_dependency",
                    "source_file": filepath,
                    "import_target": imp,
                    "detail": f"Element '{source_elem}' imports '{target_elem}' (via '{imp}') but model has no '{source_elem} -> {target_elem}' relationship",
                })

    # Detect unused relationships
    # Collect all import-derived element pairs
    imported_pairs = set()
    for filepath, import_targets in imports_by_file.items():
        source_elem = file_element_map.get(filepath)
        if not source_elem:
            continue
        for imp in import_targets:
            resolved = _resolve_import_to_path(filepath, imp)
            if resolved is None:
                continue
            target_elem = _match_path_to_element(resolved, path_to_element)
            if target_elem and target_elem != source_elem:
                imported_pairs.add((source_elem, target_elem))

    for rel in relationships:
        pair = (rel.get("source"), rel.get("target"))
        if pair[0] and pair[1] and pair not in imported_pairs:
            warnings.append({
                "type": "unused_relationship",
                "element_a": pair[0],
                "element_b": pair[1],
                "detail": f"Model declares '{pair[0]} -> {pair[1]}' but no import evidence found",
            })

    # Check for path_not_found warnings
    for elem in elements:
        for p in elem.get("paths", []):
            abs_path = os.path.join(project_root, p.lstrip("./"))
            if not os.path.exists(abs_path):
                warnings.append({
                    "type": "path_not_found",
                    "element_a": elem["name"],
                    "detail": f"metadata.path '{p}' for element '{elem['name']}' does not exist",
                    "source_file": "",
                    "import_target": "",
                })

    return violations, warnings


def _resolve_import_to_path(source_file, import_target):
    """Resolve an import target to a relative file path.

    Args:
        source_file: The file containing the import
        import_target: The import target string

    Returns:
        Normalized relative path or None if resolution fails
    """
    source_dir = os.path.dirname(source_file)

    if import_target.startswith("./") or import_target.startswith(".."):
        # Relative import
        resolved = os.path.normpath(os.path.join(source_dir, import_target))
        return resolved.replace("\\", "/")
    elif import_target.startswith("@/"):
        # Path alias
        return import_target[2:]

    return None


def _match_path_to_element(resolved_path, path_to_element):
    """Match a resolved import path to an element name.

    Also tries with common extensions appended.
    """
    norm = _normalize_path(resolved_path)

    # Direct match
    if norm in path_to_element:
        return path_to_element[norm]

    # Try as directory (append /)
    if norm + "/" in path_to_element:
        return path_to_element[norm + "/"]

    # Check if norm is under any registered path
    for elem_path, elem_name in path_to_element.items():
        if norm.startswith(elem_path + "/") or norm == elem_path:
            return elem_name

    # Try with common extensions
    for ext in [".ts", ".tsx", ".js", ".jsx", ".py"]:
        candidate = norm + ext
        if candidate in path_to_element:
            return path_to_element[candidate]

    return None


def get_model_changes(project_root):
    """Detect if architecture/ files are in staged changes.

    Returns summary of model changes (added/removed/modified).
    """
    arch_dir = os.path.join(project_root, "openspec", "architecture")
    if not os.path.isdir(arch_dir):
        return {"added": [], "removed": [], "modified": []}

    try:
        # Check staged changes for architecture files
        result = subprocess.run(
            ["git", "diff", "--cached", "--name-status"],
            cwd=project_root,
            capture_output=True,
            text=True,
            timeout=10,
            shell=True,
        )
        if result.returncode != 0:
            return {"added": [], "removed": [], "modified": []}

        changes = {"added": [], "removed": [], "modified": []}
        for line in result.stdout.splitlines():
            if not line.strip():
                continue
            parts = line.split("\t")
            if len(parts) < 2:
                continue
            status, filename = parts[0], parts[1]

            if filename.startswith("openspec/architecture/"):
                if status == "A":
                    changes["added"].append(filename)
                elif status == "D":
                    changes["removed"].append(filename)
                elif status == "M":
                    changes["modified"].append(filename)

        return changes
    except (subprocess.TimeoutExpired, OSError, FileNotFoundError):
        return {"added": [], "removed": [], "modified": []}


def compute_diff_hash(files, project_root):
    """Compute a hash of the staged diff for deduplication."""
    try:
        result = subprocess.run(
            ["git", "diff", "--cached"],
            cwd=project_root,
            capture_output=True,
            text=True,
            timeout=10,
            shell=True,
        )
        if result.returncode == 0 and result.stdout:
            return hashlib.sha256(result.stdout.encode()).hexdigest()[:16]
    except (subprocess.TimeoutExpired, OSError, FileNotFoundError):
        pass
    return hashlib.sha256(",".join(sorted(files)).encode()).hexdigest()[:16]


def generate_report(project_root, changed_files, model, violations, warnings, file_element_map, model_changes):
    """Generate the validation report dict."""
    matched = []
    seen_elements = {}

    for filepath, elem_name in file_element_map.items():
        if elem_name:
            if elem_name not in seen_elements:
                seen_elements[elem_name] = []
            seen_elements[elem_name].append(filepath)

    for elem_name, files in seen_elements.items():
        matched.append({"element_id": elem_name, "files": files})

    unmatched_files = [f for f in changed_files if file_element_map.get(f) is None]

    diff_hash = compute_diff_hash(changed_files, project_root)
    timestamp = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")

    report = {
        "timestamp": timestamp,
        "commit_diff_hash": diff_hash,
        "changed_files": changed_files,
        "matched": matched,
        "violations": violations,
        "warnings": warnings,
        "unmatched_files": unmatched_files,
        "model_changes": model_changes,
    }

    return report


def main():
    parser = argparse.ArgumentParser(description="Architecture validation engine")
    parser.add_argument("--project-root", default=None, help="Project root directory")
    parser.add_argument("--staged", action="store_true", help="Validate staged files")
    parser.add_argument("--files", default=None, help="Comma-separated file list to validate")
    parser.add_argument("--output", default=None, help="Report output path")
    parser.add_argument("--no-save", action="store_true", help="Skip saving report (print only)")
    args = parser.parse_args()

    project_root = args.project_root or resolve_project_root()

    # Load model
    model = load_model(project_root)

    if "Model file not found" in str(model.get("errors", [])):
        print(json.dumps({
            "status": "skipped",
            "reason": "no model to validate against",
        }, indent=2))
        return

    # Get changed files
    files_list = None
    if args.files:
        files_list = [f.strip() for f in args.files.split(",") if f.strip()]
    changed_files = get_changed_files(project_root, staged=args.staged, files=files_list)

    if not changed_files:
        print(json.dumps({
            "status": "no_changes",
            "changed_files": [],
        }, indent=2))
        return

    # Map files to elements
    file_element_map = map_files_to_elements(changed_files, model["path_to_element"])

    # Parse imports
    imports_by_file = {}
    for filepath in changed_files:
        imports_by_file[filepath] = parse_imports(filepath, project_root)

    # Cross-reference
    violations, warnings = cross_reference(
        imports_by_file,
        file_element_map,
        model["elements"],
        model["relationships"],
        model["path_to_element"],
        project_root,
    )

    # Model changes
    model_changes = get_model_changes(project_root)

    # Generate report
    report = generate_report(
        project_root,
        changed_files,
        model,
        violations,
        warnings,
        file_element_map,
        model_changes,
    )

    # Add status summary
    report["status"] = "violations_found" if violations else "clean"

    if not args.no_save:
        # Save report
        reports_dir = os.path.join(project_root, "openspec", "architecture", "reports")
        os.makedirs(reports_dir, exist_ok=True)
        timestamp = datetime.now().strftime("%Y%m%d-%H%M%S")
        report_path = os.path.join(reports_dir, f"validate-{timestamp}.json")
        with open(report_path, "w", encoding="utf-8") as f:
            json.dump(report, f, indent=2, ensure_ascii=False)
        report["_report_path"] = report_path

    print(json.dumps(report, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    main()
