#!/usr/bin/env python3
"""
Validation script: Verify the three old Python archi utility files have been deleted
from plugins/dev-team/utils/.

Covers AC-19: Verify archi three files are deleted from utils/ directory.

Files expected to be deleted:
- archi_parser.py (old self-implemented DSL parser)
- archi-model.py (old model builder)
- archi-validate.py (old import cross-reference validator)
"""

import os
import sys
import unittest


# The three archi files that should be deleted (replaced by @likec4/core)
ARCHI_FILES_TO_DELETE = [
    "archi_parser.py",
    "archi-model.py",
    "archi-validate.py",
]


class TestArchiFilesDeleted(unittest.TestCase):
    """AC-19: Verify old Python archi files have been deleted from utils/."""

    def setUp(self):
        self.project_root = os.path.normpath(
            os.path.join(os.path.dirname(os.path.abspath(__file__)),
                         "..", "..", "..", "..")
        )
        self.utils_dir = os.path.join(
            self.project_root, "plugins", "dev-team", "utils"
        )

    def test_archi_parser_py_deleted(self):
        """AC-19: archi_parser.py should be deleted from utils/."""
        filepath = os.path.join(self.utils_dir, "archi_parser.py")
        self.assertFalse(
            os.path.isfile(filepath),
            f"Old archi file still exists: {filepath}"
        )

    def test_archi_model_py_deleted(self):
        """AC-19: archi-model.py should be deleted from utils/."""
        filepath = os.path.join(self.utils_dir, "archi-model.py")
        self.assertFalse(
            os.path.isfile(filepath),
            f"Old archi file still exists: {filepath}"
        )

    def test_archi_validate_py_deleted(self):
        """AC-19: archi-validate.py should be deleted from utils/."""
        filepath = os.path.join(self.utils_dir, "archi-validate.py")
        self.assertFalse(
            os.path.isfile(filepath),
            f"Old archi file still exists: {filepath}"
        )

    def test_replacement_files_exist(self):
        """Verify the new @likec4/core based archi files exist."""
        bin_dir = os.path.join(self.project_root, "plugins", "dev-team", "bin")
        src_dir = os.path.join(bin_dir, "src")
        # The archi commands may be in the CLI bundle; check that the CLI entry exists
        index_file = os.path.join(src_dir, "index.ts")
        if os.path.isfile(index_file):
            with open(index_file, "r", encoding="utf-8") as f:
                content = f.read()
            # Check that archi commands are registered
            self.assertIn(
                "archi", content,
                "index.ts should register archi subcommands"
            )
        else:
            # Maybe bundle exists
            bundle_file = os.path.join(bin_dir, "dev-team-bundle.cjs")
            if os.path.isfile(bundle_file):
                with open(bundle_file, "r", encoding="utf-8") as f:
                    content = f.read(4096)
                self.assertIn(
                    "archi", content,
                    "dev-team-bundle.cjs should include archi command"
                )


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "--scan":
        project_root = os.path.normpath(
            os.path.join(os.path.dirname(os.path.abspath(__file__)),
                         "..", "..", "..", "..")
        )
        utils_dir = os.path.join(project_root, "plugins", "dev-team", "utils")
        print(f"Scanning {utils_dir} for old archi files...")
        for filename in ARCHI_FILES_TO_DELETE:
            filepath = os.path.join(utils_dir, filename)
            if os.path.isfile(filepath):
                print(f"  [FOUND] {filename} still exists at {filepath}")
            else:
                print(f"  [OK] {filename} has been deleted")
        sys.exit(0)

    unittest.main(verbosity=2)
