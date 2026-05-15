#!/usr/bin/env python3
"""
archi-parser.py — Shared C4 DSL parser for architecture utilities.

Provides a unified parser used by both archi-model.py and archi-validate.py
to eliminate code duplication and ensure consistent parsing behavior.

Exports:
  get_model_files   — list .c4 files from models/ dir (with legacy fallback)
  read_model         — aggregate all model files into a single DSL string
  parse_dsl          — parse DSL text into structured {elements, relationships, path_to_element}
  validate_structure — structural validation of DSL text
"""

import os
import re


ARCHITECTURE_DIR = "openspec/specs/architecture"
MODELS_DIR = os.path.join(ARCHITECTURE_DIR, "models")

ELEMENT_KINDS = [
    "package", "domain", "module", "component",
    "softwareSystem", "container", "system", "person",
]


# ---------------------------------------------------------------------------
# File helpers
# ---------------------------------------------------------------------------


def _walk_up(path):
    while True:
        parent = os.path.dirname(path)
        if parent == path:
            break
        yield parent
        path = parent


def get_model_files(project_root):
    """Return (files) for all .c4 model files.

    files is a list of (filename, full_path) in alphabetical order.
    Returns ([]) when no model files exist.
    """
    models_dir = os.path.join(project_root, MODELS_DIR)
    if os.path.isdir(models_dir):
        c4_files = sorted(
            f for f in os.listdir(models_dir) if f.endswith(".c4")
        )
        if c4_files:
            return [(f, os.path.join(models_dir, f)) for f in c4_files]

    return []


def read_model(project_root):
    """Read all models/*.c4 in alphabetical order and return aggregated DSL text.
    """
    files = get_model_files(project_root)

    if not files:
        return None

    parts = []
    for _filename, filepath in files:
        with open(filepath, "r", encoding="utf-8") as f:
            parts.append(f.read())

    return "\n".join(parts)


# ---------------------------------------------------------------------------
# DSL parser
# ---------------------------------------------------------------------------


def parse_dsl(dsl_text):
    """Parse C4 DSL text into a structured representation.

    Returns:
        dict with:
        - elements: list of {kind, name, paths, metadata}
        - relationships: list of {source, target, description}
        - path_to_element: dict mapping normalized paths to element names
        - errors: list of parse error strings
    """
    result = {
        "elements": [],
        "relationships": [],
        "path_to_element": {},
        "errors": [],
    }

    lines = dsl_text.split("\n")
    in_model = False
    current_element = None
    current_metadata = {}
    in_metadata = False
    metadata_brace_depth = 0
    extend_stack = []

    for line in lines:
        stripped = line.strip()

        if not stripped or stripped.startswith("//") or stripped.startswith("#"):
            continue

        # Handle closing brace inside metadata block first
        if stripped == "}" and in_metadata:
            metadata_brace_depth -= 1
            if metadata_brace_depth == 0:
                in_metadata = False
                if current_element and "path" in current_metadata:
                    current_element["paths"] = current_metadata["path"]
            continue

        # Handle metadata block content
        if in_metadata:
            _parse_metadata_kv(stripped, current_metadata)
            continue

        # Track block boundaries
        if stripped == "model {":
            in_model = True
            continue

        if stripped == "}":
            if current_element:
                if current_metadata:
                    current_element["metadata"] = current_metadata
                    if "path" in current_metadata:
                        current_element["paths"] = current_metadata["path"]
                result["elements"].append(current_element)
                for p in current_element.get("paths", []):
                    result["path_to_element"][_normalize_path(p)] = current_element["name"]
                current_element = None
                current_metadata = {}
                continue
            if extend_stack:
                extend_stack.pop()
                continue
            continue

        if not in_model:
            continue

        # extend <parentRef> {  — scope elements under parent
        if stripped.startswith("extend "):
            parent_ref = stripped[len("extend "):].strip().rstrip("{").strip()
            extend_stack.append(parent_ref)
            current_element = None
            continue

        # Relationship
        if "->" in stripped and current_element is None:
            rel = parse_relationship(stripped)
            if rel:
                result["relationships"].append(rel)
            continue

        # Element definition
        elem = try_parse_element(stripped)
        if elem:
            if current_element:
                if current_metadata:
                    current_element["metadata"] = current_metadata
                    if "path" in current_metadata:
                        current_element["paths"] = current_metadata["path"]
                result["elements"].append(current_element)
                for p in current_element.get("paths", []):
                    result["path_to_element"][_normalize_path(p)] = current_element["name"]
            if extend_stack:
                elem["name"] = extend_stack[-1] + "." + elem["name"]
            current_element = elem
            current_metadata = {}
            continue

        # Metadata block start
        if current_element and stripped.startswith("metadata {"):
            inner = stripped[len("metadata {"):].strip()
            if inner.endswith("}"):
                inner = inner[:-1].strip()
                _parse_metadata_kv(inner, current_metadata)
                if current_element and "path" in current_metadata:
                    current_element["paths"] = current_metadata["path"]
            else:
                in_metadata = True
                metadata_brace_depth = 1
                if inner:
                    _parse_metadata_kv(inner, current_metadata)
            continue

    # Last element (file ended before its closing brace)
    if current_element:
        if current_metadata:
            current_element["metadata"] = current_metadata
            if "path" in current_metadata:
                current_element["paths"] = current_metadata["path"]
        result["elements"].append(current_element)
        for p in current_element.get("paths", []):
            result["path_to_element"][_normalize_path(p)] = current_element["name"]

    return result


def try_parse_element(line):
    """Try to parse an element definition from a DSL line.

    Returns dict with kind, name, paths=[] or None.
    """
    for kind in ELEMENT_KINDS:
        prefix = kind + " "
        if line.startswith(prefix):
            name = line[len(prefix):].rstrip("{").strip().rstrip()
            return {"kind": kind, "name": name, "paths": [], "metadata": {}}
    return None


def parse_relationship(line):
    """Parse a relationship: A -> B "description"."""
    arrow_idx = line.find("->")
    if arrow_idx < 0:
        return None

    source = line[:arrow_idx].strip()
    rest = line[arrow_idx + 2:].strip()

    target = rest.split()[0] if rest else rest
    target = target.rstrip("{").strip()

    description = None
    desc_match = re.search(r'"([^"]*)"', line)
    if desc_match:
        description = desc_match.group(1)

    return {"source": source, "target": target, "description": description}


def _parse_metadata_kv(line, metadata_dict):
    """Parse a key-value pair from inside a metadata { } block.

    Examples:
      path './src/services/payment/'
      path ['./src/services/payment/', './src/shared/billing.ts']
      owner 'team-platform'
    """
    line = line.strip()
    if not line:
        return
    parts = line.split(None, 1)
    if len(parts) < 2:
        return
    key = parts[0]
    value = parts[1].strip()

    if value.startswith("["):
        value = value.rstrip(",").rstrip()
        metadata_dict[key] = _parse_path_array(value)
    else:
        v = value.strip('"').strip("'").rstrip(",")
        if v:
            metadata_dict[key] = [v]


def _parse_path_array(path_str):
    """Parse a metadata path value which may be a single string or array."""
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


def _normalize_path(path):
    """Normalize a path for comparison. Strips ./ prefix and trailing slash."""
    path = path.strip()
    if path.startswith("./"):
        path = path[2:]
    return path.rstrip("/").rstrip("\\")


# ---------------------------------------------------------------------------
# Validation
# ---------------------------------------------------------------------------


def validate_structure(dsl_text, project_root=None):
    """Basic structural validation of DSL text.

    Checks: brace balance, required blocks, specification syntax,
    metadata syntax, and duplicate specification blocks across files.
    """
    lines = dsl_text.split("\n")
    brace_depth = 0
    braces = {"{": 1, "}": -1}
    line_number = 0

    for line in lines:
        line_number += 1
        stripped = line.strip()
        if stripped.startswith("//") or stripped.startswith("#"):
            continue
        for char in stripped:
            if char in braces:
                brace_depth += braces[char]
                if brace_depth < 0:
                    return {
                        "valid": False,
                        "error": f"Unexpected '}}' at line {line_number}",
                    }

    if brace_depth != 0:
        suffix = "missing" if brace_depth > 0 else "extra"
        return {
            "valid": False,
            "error": f"Unmatched braces: {suffix} '}}'",
        }

    if "specification" not in dsl_text:
        return {"valid": False, "error": "Missing 'specification' block"}

    if "model" not in dsl_text:
        return {"valid": False, "error": "Missing 'model' block"}

    spec_error = check_specification_syntax(dsl_text)
    if spec_error:
        return {"valid": False, "error": spec_error}

    metadata_error = check_metadata_syntax(dsl_text)
    if metadata_error:
        return {"valid": False, "error": metadata_error}

    if project_root:
        files = get_model_files(project_root)
        if len(files) > 1:
            spec_files = []
            for filename, filepath in files:
                with open(filepath, "r", encoding="utf-8") as f:
                    content = f.read()
                if "specification" in content:
                    spec_files.append(filename)
            if len(spec_files) > 1:
                return {
                    "valid": False,
                    "error": (
                        f"Duplicate 'specification' blocks found in files: "
                        f"{', '.join(spec_files)}. "
                        "Only one file may contain a specification block."
                    ),
                }

    return {"valid": True}


def check_specification_syntax(dsl_text):
    """Check specification block for invalid colon syntax (e.g. 'name: elementKind').

    Returns an error string if invalid syntax is found, None otherwise.
    """
    lines = dsl_text.split("\n")
    in_spec = False
    for line_num, line in enumerate(lines, 1):
        stripped = line.strip()
        if stripped.startswith("//") or stripped.startswith("#"):
            continue
        if stripped == "specification {":
            in_spec = True
            continue
        if in_spec and stripped == "}":
            break
        if in_spec and stripped:
            if stripped.startswith("element "):
                continue
            if stripped.startswith("relationship "):
                continue
            if ": elementKind" in stripped or ": elementkind" in stripped.lower():
                kind = stripped.split(":")[0].strip()
                return (
                    f"Invalid specification syntax at line {line_num}: '{stripped}'. "
                    f"Use 'element {kind}' instead."
                )
            if ":" in stripped and not stripped.startswith("element "):
                kind = stripped.split(":")[0].strip()
                return (
                    f"Invalid specification syntax at line {line_num}: '{stripped}'. "
                    f"Use 'element {kind}' instead."
                )
    return None


def check_metadata_syntax(dsl_text):
    """Check for flat 'metadata path [...]' or 'metadata key value' syntax without braces.

    Returns an error string if invalid syntax is found, None otherwise.
    """
    pattern = re.compile(r'^\s*metadata\s+(\w+)\s+', re.MULTILINE)
    for match in pattern.finditer(dsl_text):
        line_start = match.start()
        line = dsl_text[line_start:].split("\n")[0]
        if "{" in line:
            continue
        return (
            f"Invalid metadata syntax: '{line.strip()}'. "
            "Use 'metadata {{ key value }}' with braces."
        )
    return None
