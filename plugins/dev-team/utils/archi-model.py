#!/usr/bin/env python3
"""
archi-model.py — Architecture model operation utility.

Commands:
  query       Read model structure via Python DSL parser
  validate    Validate model DSL syntax via Python parser
  write       Write DSL to a specific file in models/

Usage:
  python archi-model.py --command query [--element <fqn>]
  python archi-model.py --command validate [--source <dsl_text>]
  python archi-model.py --command write --path models/xx.c4 --source <dsl_text>
"""

import argparse
import json
import os
import sys
from datetime import datetime, timezone

from archi_parser import MODELS_DIR, get_model_files, parse_dsl, read_model, validate_structure


def resolve_project_root():
    """Find project root by locating openspec/specs/architecture/ or .git."""
    cwd = os.getcwd()
    for start in [cwd] + list(_walk_up(cwd)):
        if os.path.isdir(os.path.join(start, "openspec", "specs", "architecture")):
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


def read_model_file(project_root, target_path):
    """Read a specific model file from models/ directory."""
    full_path = os.path.join(project_root, MODELS_DIR, target_path)
    if not os.path.isfile(full_path):
        return None
    with open(full_path, "r", encoding="utf-8") as f:
        return f.read()


def query_model(project_root, element_fqn=None):
    """Query the model via Python DSL parser and return element structure."""
    dsl = read_model(project_root)
    if dsl is None:
        return {"error": "No model files found"}

    result = parse_dsl(dsl)

    if element_fqn:
        return _filter_element(result, element_fqn)

    return result


def validate_dsl(project_root, dsl_text=None):
    """Validate DSL syntax using the Python parser.

    If dsl_text is provided, validate that. Otherwise validate current model.
    When validating a single file that lacks a specification block, the spec
    is automatically prepended from the existing model files.
    """
    if dsl_text is None:
        dsl_text = read_model(project_root)
        if dsl_text is None:
            return {"valid": False, "error": "No model files found"}

    # If source lacks a specification block, prepend it from existing model files
    if "specification" not in dsl_text and project_root:
        spec_block = _find_specification_block(project_root)
        if spec_block:
            dsl_text = spec_block + "\n" + dsl_text

    return validate_structure(dsl_text, project_root)


def _find_specification_block(project_root):
    """Find and return the specification block from existing model files.

    Returns the specification block text (with newline) or None if not found.
    """
    files, _is_legacy = get_model_files(project_root)
    for _filename, filepath in files:
        with open(filepath, "r", encoding="utf-8") as f:
            content = f.read()
        if "specification {" in content:
            # Extract the specification block
            start = content.index("specification {")
            brace_depth = 0
            end = start
            for i, ch in enumerate(content[start:], start):
                if ch == "{":
                    brace_depth += 1
                elif ch == "}":
                    brace_depth -= 1
                    if brace_depth == 0:
                        end = i + 1
                        break
            return content[start:end]
    return None


def write_dsl(project_root, dsl_text, target_path):
    """Write DSL text to a specific file in models/ after validation."""
    models_dir = os.path.join(project_root, MODELS_DIR)
    full_path = os.path.normpath(os.path.join(models_dir, target_path))

    if not full_path.startswith(os.path.normpath(models_dir)):
        return {
            "success": False,
            "error": f"Path '{target_path}' is outside models/ directory",
        }

    validation = validate_dsl(project_root, dsl_text)
    if not validation.get("valid", False):
        return {
            "success": False,
            "error": validation.get("error", "Validation failed"),
            "validation": validation,
        }

    os.makedirs(models_dir, exist_ok=True)
    with open(full_path, "w", encoding="utf-8") as f:
        f.write(dsl_text)

    return {"success": True, "path": full_path}


def _filter_element(result, fqn):
    """Filter model result for a specific element FQN."""
    for elem in result["elements"]:
        if elem["name"] == fqn:
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
    parser.add_argument("--path", default=None, help="Target path within models/ (write only)")
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
        if not args.path:
            print(json.dumps({"success": False, "error": "--path required for write command"}))
            sys.exit(1)
        result = write_dsl(project_root, args.source, args.path)
        print(json.dumps(result, indent=2))
        if not result.get("success", False):
            sys.exit(1)


if __name__ == "__main__":
    main()
