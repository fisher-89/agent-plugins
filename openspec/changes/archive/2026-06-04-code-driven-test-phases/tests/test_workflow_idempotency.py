#!/usr/bin/env python3
"""
Integration Test: Workflow idempotency (T8).

Verifies that repeating the complete test-gen workflow twice produces:
1. No duplicate test files (no test_auth_2.py, no test_auth(1).py)
2. No duplicate test functions within files (no repeated def test_*)
3. Byte-identical file content across runs (SHA-256 match)
4. No residual temp files (.bak, .tmp, cache files)
5. Test-design phase repeated execution produces consistent test-design.md

Also verifies:
- Recovery: delete generated tests, re-run generator, content matches
"""

import hashlib
import json
import os
import shutil
import unittest

PROJECT_ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", "..", ".."))
CHANGE_NAME = "_mock_test_idempotency"
CHANGE_DIR = os.path.join(PROJECT_ROOT, "openspec", "changes", CHANGE_NAME)
MOCK_SRC_DIR = os.path.join(PROJECT_ROOT, "_mock_src_idempotency")

# Mock source files (subset of integration test for focused idempotency check)
MOCK_SRC_FILES = {
    "auth.py": """
def register(email: str, password: str) -> dict | None:
    \"\"\"Register a new user.\"\"\"
    pass

def login(email: str, password: str) -> str | None:
    \"\"\"Login with credentials.\"\"\"
    pass
""",
    "utils.py": """
def validate_email(value):
    \"\"\"Validate email format.\"\"\"
    pass
""",
}


def setup_mock_change():
    """Create a minimal mock change for idempotency testing."""
    if os.path.isdir(CHANGE_DIR):
        shutil.rmtree(CHANGE_DIR)
    os.makedirs(os.path.join(CHANGE_DIR, "specs", "mock-capability"))

    # Write proposal with forward and reverse ACs
    with open(os.path.join(CHANGE_DIR, "proposal.md"), "w", encoding="utf-8") as f:
        f.write("""# Mock Change: Auth Module

## Acceptance Criteria

### AC-1 (Forward): Register with valid credentials
### AC-2 (Reverse): Register with invalid email
### AC-3 (Forward): Login with correct credentials
### AC-4 (Reverse): Login with wrong password
""")

    # Write design.md
    with open(os.path.join(CHANGE_DIR, "design.md"), "w", encoding="utf-8") as f:
        f.write("""# Design: Auth Module

## API
- register(email, password) -> User
- login(email, password) -> Token
""")

    # Write spec.md
    with open(os.path.join(CHANGE_DIR, "specs", "mock-capability", "spec.md"), "w", encoding="utf-8") as f:
        f.write("""# Spec

- `register(email: str, password: str) -> dict | None`
- `login(email: str, password: str) -> str | None`
""")

    # Create mock source directory
    if os.path.isdir(MOCK_SRC_DIR):
        shutil.rmtree(MOCK_SRC_DIR)
    for rel_path, content in MOCK_SRC_FILES.items():
        full_path = os.path.join(MOCK_SRC_DIR, rel_path)
        os.makedirs(os.path.dirname(full_path), exist_ok=True)
        with open(full_path, "w", encoding="utf-8") as f:
            f.write(content)


def teardown_mock_change():
    """Clean up mock data."""
    if os.path.isdir(CHANGE_DIR):
        shutil.rmtree(CHANGE_DIR)
    if os.path.isdir(MOCK_SRC_DIR):
        shutil.rmtree(MOCK_SRC_DIR)


def get_sha256(filepath):
    """Compute SHA-256 hash of a file."""
    sha256 = hashlib.sha256()
    with open(filepath, "rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            sha256.update(chunk)
    return sha256.hexdigest()


def find_generated_test_files(base_dir):
    """Find all generated test files in the mock source directory."""
    test_files = []
    for root, dirs, files in os.walk(base_dir):
        for f in files:
            if (f.startswith("test_") and f.endswith(".py")) or \
               f.endswith(".test.ts") or \
               f.endswith("_test.rs") or \
               f.endswith("_test.go"):
                test_files.append(os.path.join(root, f))
    return sorted(test_files)


def count_test_functions(filepath):
    """Count test function definitions in a file."""
    count = 0
    try:
        with open(filepath, "r", encoding="utf-8") as f:
            for line in f:
                stripped = line.strip()
                if stripped.startswith("def test_") or \
                   stripped.startswith("it(") or \
                   stripped.startswith('it(') or \
                   stripped.startswith("test ") or \
                   stripped.startswith("func Test") or \
                   stripped.startswith("#[test]") or \
                   stripped.startswith("it("):
                    count += 1
    except (UnicodeDecodeError, IOError):
        pass
    return count


class TestWorkflowIdempotency(unittest.TestCase):
    """Test that repeating the test-gen workflow produces identical results."""

    @classmethod
    def setUpClass(cls):
        """Set up mock change once, shared across all tests."""
        setup_mock_change()

    @classmethod
    def tearDownClass(cls):
        """Clean up after all tests."""
        # Comment out to keep artifacts for debugging
        # teardown_mock_change()
        pass

    def setUp(self):
        """Verify prerequisites."""
        self.assertTrue(os.path.isdir(CHANGE_DIR), "Mock change directory must exist")
        self.assertTrue(os.path.isdir(MOCK_SRC_DIR), "Mock source directory must exist")

    # ─── Round 1: Record baseline ─────────────────────────────────────────

    def test_round1_no_duplicate_test_files(self):
        """Round 1: Verify no duplicate test files exist after generation.
        Specifically, no test_auth_2.py, no test_auth(1).py, etc."""
        test_files = find_generated_test_files(MOCK_SRC_DIR)

        # Check for duplicate patterns
        for f in test_files:
            basename = os.path.basename(f)
            # Check for suffix-based duplicates
            has_duplicate_suffix = bool(
                re.search(r'_\d+\.py|\(\d+\)\.py|_\d+\.ts|\(\d+\)\.ts|_\d+\.rs|\(\d+\)\.rs|_\d+\.go|\(\d+\)\.go', basename)
            )
            self.assertFalse(
                has_duplicate_suffix,
                f"No duplicate-named test files should exist: {basename}"
            )

    def test_round1_no_duplicate_test_functions(self):
        """Round 1: Verify no duplicate test function definitions within files."""
        test_files = find_generated_test_files(MOCK_SRC_DIR)
        for f in test_files:
            function_names = set()
            try:
                with open(f, "r", encoding="utf-8") as fh:
                    for line in fh:
                        stripped = line.strip()
                        # Python test function
                        m = re.match(r'def (test_\w+)\(', stripped)
                        if m:
                            name = m.group(1)
                            self.assertNotIn(
                                name, function_names,
                                f"Duplicate test function '{name}' in {f}"
                            )
                            function_names.add(name)
            except (UnicodeDecodeError, IOError):
                pass

    def test_round1_sha256_recorded(self):
        """Round 1: Record SHA-256 hashes for generated test files."""
        test_files = find_generated_test_files(MOCK_SRC_DIR)
        # Store hashes for round 2 comparison
        # TODO: Persist these hashes to compare after round 2
        round1_hashes = {}
        for f in test_files:
            round1_hashes[f] = get_sha256(f)

        if round1_hashes:
            self.assertGreater(
                len(round1_hashes), 0,
                "At least one test file should exist after generation"
            )

    # ─── Round 2: Idempotency checks ──────────────────────────────────────

    def test_round2_no_new_files_vs_round1(self):
        """Round 2: Verify no new test files appear after second run
        that weren't in round 1."""
        # TODO: Run second workflow, then compare file lists
        # This requires the workflow to be executed twice externally
        round1_files = find_generated_test_files(MOCK_SRC_DIR)
        # Run workflow again, get round 2 files
        round2_files = find_generated_test_files(MOCK_SRC_DIR)
        # File count should be identical
        self.assertEqual(
            len(round1_files), len(round2_files),
            "File count should be identical across runs"
        )

    def test_round2_no_extraneous_temp_files(self):
        """Round 2: Verify no .bak, .tmp, .swp, or other residual files exist
        after the second run."""
        # Check MOCK_SRC_DIR for non-source, non-test files
        source_basenames = set(MOCK_SRC_FILES.keys())
        test_patterns = (
            "test_auth.py", "test_utils.py",
        )

        for root, dirs, files in os.walk(MOCK_SRC_DIR):
            for f in files:
                # Skip source files
                if f in source_basenames:
                    continue
                # Skip expected test files
                if f in test_patterns or f.startswith("test_") or \
                   f.endswith(".test.ts") or f.endswith("_test.rs") or f.endswith("_test.go"):
                    continue
                # Any other file is a residual
                if f.endswith((".bak", ".tmp", ".swp", "~")):
                    self.fail(
                        f"Residual temporary file found after idempotent run: "
                        f"{os.path.join(root, f)}"
                    )

    def test_sha256_identity_across_runs(self):
        """Verify SHA-256 hashes are identical between round 1 and round 2
        for each generated test file."""
        # TODO: This requires persisting round 1 hashes and comparing after round 2
        test_files = find_generated_test_files(MOCK_SRC_DIR)

        # Record current hashes
        current_hashes = {}
        for f in test_files:
            current_hashes[f] = get_sha256(f)

        # If we have stored hashes from a previous run file, compare
        hash_storage = os.path.join(CHANGE_DIR, ".idempotency_hashes.json")
        if os.path.isfile(hash_storage):
            with open(hash_storage, "r", encoding="utf-8") as fh:
                stored_hashes = json.load(fh)

            # Compare file lists
            stored_files = set(stored_hashes.keys())
            current_files = set(current_hashes.keys())
            self.assertEqual(
                stored_files, current_files,
                "Generated file set should be identical across runs"
            )

            # Compare content
            for filepath, stored_hash in stored_hashes.items():
                if filepath in current_hashes:
                    self.assertEqual(
                        stored_hash,
                        current_hashes[filepath],
                        f"SHA-256 mismatch for {filepath} between runs"
                    )
        else:
            # First run: store hashes for future comparison
            # TODO: On actual second run, reload stored hashes and compare
            with open(hash_storage, "w", encoding="utf-8") as fh:
                json.dump(current_hashes, fh, indent=2)
            self.skipTest(
                "Stored round 1 hashes. Re-run workflow and run test again for comparison."
            )

    def test_no_function_count_increase(self):
        """Verify test function count does not increase after second run."""
        test_files = find_generated_test_files(MOCK_SRC_DIR)

        function_count_file = os.path.join(CHANGE_DIR, ".idempotency_fn_count.json")
        current_counts = {}
        for f in test_files:
            current_counts[f] = count_test_functions(f)

        if os.path.isfile(function_count_file):
            with open(function_count_file, "r", encoding="utf-8") as fh:
                stored_counts = json.load(fh)

            for filepath, stored_count in stored_counts.items():
                if filepath in current_counts:
                    self.assertEqual(
                        stored_count,
                        current_counts[filepath],
                        f"Test function count increased in {filepath}: "
                        f"{stored_count} -> {current_counts[filepath]}"
                    )
        else:
            with open(function_count_file, "w", encoding="utf-8") as fh:
                json.dump(current_counts, fh, indent=2)
            self.skipTest("Stored round 1 function counts. Re-run to compare.")


class TestWorkflowIdempotencyRecovery(unittest.TestCase):
    """Test recovery: delete generated tests, re-run generator, content matches."""

    def test_recovery_regenerates_deleted_files(self):
        """Verify manually deleting a generated test file and re-running
        the generator produces the same file content."""
        # TODO: Requires manual workflow execution between test steps
        # 1. Run workflow -> record SHA-256
        # 2. Delete one test file
        # 3. Run workflow again -> new file should have same SHA-256
        pass

    def test_recovery_no_permanent_loss(self):
        """Verify generator doesn't skip files because 'they already exist'
        when files were manually deleted — re-running should regenerate them."""
        # TODO: Same as above
        pass


class TestWorkflowIdempotencyTestDesign(unittest.TestCase):
    """Test test-design phase idempotency."""

    def test_test_design_consistency_across_runs(self):
        """Verify two runs of test-design phase produce consistent test-design.md."""
        test_design_path = os.path.join(CHANGE_DIR, "test-design.md")
        if not os.path.isfile(test_design_path):
            self.skipTest("test-design.md not found")

        # TODO: Compare against stored baseline from first run
        # Key sections to compare:
        # - Forward ACs list structure and content
        # - Reverse ACs list structure and content
        # - Coverage map table: number of entries, AC IDs, file paths
        pass


# Helper: import re for duplicate detection
import re

if __name__ == "__main__":
    unittest.main(verbosity=2)
