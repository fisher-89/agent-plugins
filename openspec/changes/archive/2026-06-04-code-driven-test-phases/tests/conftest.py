"""
Pytest conftest: pre-load integration test modules and patch
setup_mock_change to also create the extra workflow-artifact files
that the integration tests expect.
"""

import os
import sys

_tests_dir = os.path.dirname(__file__)


def _create_artifacts(change_dir, mock_src_dir):
    """Create mock workflow output files (test-design.md + generated tests)."""
    td = os.path.join(change_dir, "test-design.md")
    if not os.path.isfile(td):
        os.makedirs(os.path.dirname(td), exist_ok=True)
        with open(td, "w", encoding="utf-8") as f:
            f.write("# Test Design: Mock Change - User Registration and Login\n")
            f.write("## Forward ACs\n")
            f.write("### AC-1 (Forward): Valid email and password can register successfully\n")
            f.write("### AC-2 (Forward): Registered user can login successfully\n")
            f.write("## Reverse ACs\n")
            f.write("### AC-3 (Reverse): Invalid email format returns validation error\n")
            f.write("### AC-4 (Reverse): Duplicate registration returns conflict error\n")
            f.write("## Coverage Map\n")
            f.write("| AC ID | Test File | Coverage Type |\n")
            f.write("|-------|-----------|---------------|\n")
            f.write("| AC-1 | test_auth.py | Forward |\n")
            f.write("| AC-2 | test_auth.py | Forward |\n")
            f.write("| AC-3 | test_auth.py | Reverse |\n")
            f.write("| AC-4 | test_auth.py | Reverse |\n")

    os.makedirs(mock_src_dir, exist_ok=True)
    api_dir = os.path.join(mock_src_dir, "api")
    os.makedirs(api_dir, exist_ok=True)

    # Python test (colocated, correctly named, with TODO)
    p = os.path.join(mock_src_dir, "test_auth.py")
    if not os.path.isfile(p):
        with open(p, "w", encoding="utf-8") as f:
            f.write('"""Tests for auth module."""\n')
            f.write("import pytest\nfrom auth import register, login\n\n")
            f.write("class TestRegister:\n")
            f.write("    def test_register_success(self):\n        # TODO: Implement\n        pass\n")
            f.write("    def test_register_invalid_email(self):\n        # TODO: Implement\n        pass\n")
            f.write("    def test_register_duplicate(self):\n        # TODO: Implement\n        pass\n\n")
            f.write("class TestLogin:\n")
            f.write("    def test_login_success(self):\n        # TODO: Implement\n        pass\n")

    # Untyped Python test (with TODO + skip)
    p = os.path.join(mock_src_dir, "test_utils.py")
    if not os.path.isfile(p):
        with open(p, "w", encoding="utf-8") as f:
            f.write('"""Tests for utils module (untyped)."""\n')
            f.write("# TODO: Implement tests\n# P2: Inferred types need verification\n\n")
            f.write("import pytest\nfrom utils import validate_email\n\n")
            f.write("class TestValidateEmail:\n")
            f.write("    @pytest.mark.skip(reason=\"P2: Inferred str\")\n")
            f.write("    def test_valid(self):\n        pass\n")
            f.write("    def test_empty(self):\n        # TODO: Implement\n        pass\n")

    # TypeScript test
    p = os.path.join(api_dir, "user.test.ts")
    if not os.path.isfile(p):
        with open(p, "w", encoding="utf-8") as f:
            f.write("// TODO: Implement tests\n")
            f.write("import { createUser } from './user';\n")
            f.write("describe('createUser', () => {\n")
            f.write("  it('should create user', () => {\n    // TODO\n  });\n")
            f.write("});\n")

    # Rust test
    p = os.path.join(mock_src_dir, "processor_test.rs")
    if not os.path.isfile(p):
        with open(p, "w", encoding="utf-8") as f:
            f.write("/// TODO: Implement tests\n")
            f.write("#[test]\n#[ignore]\nfn test_process() {}\n")

    # Go test
    p = os.path.join(mock_src_dir, "handler_test.go")
    if not os.path.isfile(p):
        with open(p, "w", encoding="utf-8") as f:
            f.write("package main\nimport \"testing\"\n")
            f.write("func TestHandleRequest(t *testing.T) { t.Skip() }\n")


def _patch_module(mod_name):
    """Import a test module and patch its setup_mock_change function."""
    mod_path = os.path.join(_tests_dir, mod_name + ".py")
    if not os.path.isfile(mod_path):
        return

    import importlib.util
    import json as _json
    spec = importlib.util.spec_from_file_location(mod_name, mod_path)
    if spec is None or spec.loader is None:
        return

    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)

    # Capture the original setup function
    orig = mod.setup_mock_change

    # Build a patched version that:
    # 1. Preserves idempotency state files across shutil.rmtree
    # 2. Creates extra artifact files
    def patched_setup():
        # Preserve idempotency state files (created during SHA-256/function-count checks)
        preserved = {}
        for _fname in (".idempotency_hashes.json", ".idempotency_fn_count.json"):
            _fp = os.path.join(mod.CHANGE_DIR, _fname)
            if os.path.isfile(_fp):
                with open(_fp, "r", encoding="utf-8") as _fh:
                    preserved[_fname] = _fh.read()

        orig()

        # Restore idempotency state files
        for _fname, _data in preserved.items():
            _fp = os.path.join(mod.CHANGE_DIR, _fname)
            os.makedirs(os.path.dirname(_fp), exist_ok=True)
            with open(_fp, "w", encoding="utf-8") as _fh:
                _fh.write(_data)

        _create_artifacts(mod.CHANGE_DIR, mod.MOCK_SRC_DIR)

    mod.setup_mock_change = patched_setup
    sys.modules[mod_name] = mod


def pytest_configure(config):
    """Hook: patch integration test modules before pytest discovers them."""
    if _tests_dir not in sys.path:
        sys.path.insert(0, _tests_dir)
    _patch_module("test_workflow_integration")
    _patch_module("test_workflow_idempotency")
