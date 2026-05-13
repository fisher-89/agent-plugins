#!/usr/bin/env python3
"""
archi-model.py — Architecture model operation utility.

Commands:
  query       Read model structure via Python DSL parser
  validate    Validate model.c4 DSL syntax via Python parser

Usage:
  python archi-model.py --command query [--element <fqn>]
  python archi-model.py --command validate [--source <dsl_text>]
  python archi-model.py --command write --source <dsl_text>
"""

import argparse
import json
import os
import sys
from datetime import datetime, timezone


ARCHITECTURE_DIR = "openspec/architecture"
MODEL_FILE = os.path.join(ARCHITECTURE_DIR, "model.c4")


def resolve_project_root():
    """Find project root by locating openspec/architecture/ or .git."""
    cwd = os.getcwd()
    # Try cwd first, then walk up
    for start in [cwd] + list(_walk_up(cwd)):
        if os.path.isdir(os.path.join(start, "openspec", "architecture")):
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


def read_model(project_root):
    """Read model.c4 and return DSL text."""
    model_path = os.path.join(project_root, MODEL_FILE)
    if not os.path.isfile(model_path):
        return None
    with open(model_path, "r", encoding="utf-8") as f:
        return f.read()


def query_model(project_root, element_fqn=None):
    """Query the model via Python DSL parser and return element structure."""
    model_path = os.path.join(project_root, MODEL_FILE)
    if not os.path.isfile(model_path):
        return {"error": f"Model file not found: {model_path}"}

    dsl = read_model(project_root)
    result = _parse_dsl(dsl)

    if element_fqn:
        return _filter_element(result, element_fqn)

    return result


def validate_dsl(project_root, dsl_text=None):
    """Validate DSL syntax using the Python parser.

    If dsl_text is provided, validate that. Otherwise validate current model.c4.
    """
    if dsl_text is None:
        model_path = os.path.join(project_root, MODEL_FILE)
        if not os.path.isfile(model_path):
            return {"valid": False, "error": "Model file not found"}
        with open(model_path, "r", encoding="utf-8") as f:
            dsl_text = f.read()

    return _validate_structure(dsl_text)


def write_dsl(project_root, dsl_text):
    """Write DSL text to model.c4 after validation."""
    validation = validate_dsl(project_root, dsl_text)
    if not validation.get("valid", False):
        return {
            "success": False,
            "error": validation.get("error", "Validation failed"),
            "validation": validation,
        }

    model_path = os.path.join(project_root, MODEL_FILE)
    os.makedirs(os.path.dirname(model_path), exist_ok=True)
    with open(model_path, "w", encoding="utf-8") as f:
        f.write(dsl_text)

    return {"success": True, "path": model_path}



def _validate_structure(dsl_text):
    """Basic structural validation of DSL text.

    Checks for balanced braces and required blocks.
    """
    lines = dsl_text.split("\n")
    brace_depth = 0
    braces = {"{": 1, "}": -1}
    line_number = 0

    for line in lines:
        line_number += 1
        stripped = line.strip()
        # Skip comments
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
        return {
            "valid": False,
            "error": f"Unmatched braces: {'missing' if brace_depth > 0 else 'extra'} '}}'",
        }

    if "specification" not in dsl_text:
        return {"valid": False, "error": "Missing 'specification' block"}

    if "model" not in dsl_text:
        return {"valid": False, "error": "Missing 'model' block"}

    return {"valid": True}


def _parse_dsl(dsl_text):
    """Parse C4 DSL text into a structured representation.

    Extracts specification elements, model elements (with metadata.path),
    and relationships.
    """
    result = {
        "specification": {},
        "elements": [],
        "relationships": [],
    }

    in_model = False
    current_element = None
    current_metadata = {}
    in_metadata = False
    metadata_key = None

    for line in dsl_text.split("\n"):
        stripped = line.strip()

        # Skip comments and empty lines
        if not stripped or stripped.startswith("//") or stripped.startswith("#"):
            continue

        # Track block boundaries
        if stripped == "specification {":
            in_model = False
            current_element = None
            continue
        if stripped == "model {":
            in_model = True
            continue
        if stripped == "}":
            if current_element:
                if current_metadata:
                    current_element["metadata"] = current_metadata
                result["elements"].append(current_element)
                current_element = None
                current_metadata = {}
                in_metadata = False
            continue

        if not in_model:
            continue

        # Relationship in model block
        if "->" in stripped and current_element is None and not in_metadata:
            parts = stripped.split("->")
            if len(parts) >= 2:
                source_part = parts[0].strip()
                target_part = parts[1].strip().split(" ")[0] if len(parts[1].strip().split(" ")) > 0 else parts[1].strip()
                description = None
                desc_start = stripped.find('"')
                if desc_start > 0:
                    desc_end = stripped.find('"', desc_start + 1)
                    if desc_end > desc_start:
                        description = stripped[desc_start + 1:desc_end]
                result["relationships"].append({
                    "source": source_part,
                    "target": target_part,
                    "description": description,
                })
            continue

        # Model elements (component, softwareSystem, container, etc.)
        elem_match = False
        for kind in ["softwareSystem", "component", "container", "system"]:
            if stripped.startswith(kind + " ") or stripped.startswith(kind + "\t"):
                name = stripped[len(kind):].strip().rstrip("{").strip()
                current_element = {"kind": kind, "name": name}
                current_metadata = {}
                in_metadata = False
                elem_match = True
                break

        if elem_match:
            continue

        # Metadata path
        if current_element and stripped.startswith("metadata path"):
            path_str = stripped[len("metadata path"):].strip()
            paths = _parse_path_array(path_str)
            current_metadata["path"] = paths
            continue

    return result


def _parse_path_array(path_str):
    """Parse a metadata path value which may be a single string or array."""
    path_str = path_str.strip()
    if path_str.startswith("["):
        # Array syntax: ["./a/", "./b.ts"]
        path_str = path_str[1:].rstrip("]").strip()
        paths = []
        for p in path_str.split(","):
            p = p.strip().strip('"').strip("'")
            if p:
                paths.append(p)
        return paths
    else:
        # Single path
        p = path_str.strip('"').strip("'")
        return [p] if p else []


def _filter_element(result, fqn):
    """Filter model result for a specific element FQN."""
    for elem in result["elements"]:
        if elem["name"] == fqn:
            # Find relationships involving this element
            related = []
            for rel in result["relationships"]:
                if rel["source"] == fqn or rel["target"] == fqn:
                    related.append(rel)
            return {
                "element": elem,
                "relationships": related,
            }
    return {"error": f"Element '{fqn}' not found"}


def main():
    parser = argparse.ArgumentParser(description="Architecture model operations")
    parser.add_argument("--command", required=True, choices=["query", "validate", "write"])
    parser.add_argument("--element", default=None, help="Filter by element FQN (query only)")
    parser.add_argument("--source", default=None, help="DSL text to validate or write")
    args = parser.parse_args()

    project_root = resolve_project_root()

    if args.command == "query":
        result = query_model(project_root, args.element)
        print(json.dumps(result, indent=2, ensure_ascii=False))

    elif args.command == "validate":
        dsl_text = args.source if args.source else None
        result = validate_dsl(project_root, dsl_text)
        print(json.dumps(result, indent=2))

    elif args.command == "write":
        if not args.source:
            print(json.dumps({"success": False, "error": "--source required for write command"}))
            sys.exit(1)
        result = write_dsl(project_root, args.source)
        print(json.dumps(result, indent=2))
        if not result.get("success", False):
            sys.exit(1)


if __name__ == "__main__":
    main()
