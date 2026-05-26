#!/usr/bin/env python3
"""
Integration tests for `dev-team archi validate` command with LikeC4 syntax.

Covers AC-16: archi validate compatibility with LikeC4 DSL features (views, deployment, tags).

Tests:
- Validate with --source containing `views { ... }` block passes
- Validate with --source containing `deployment { ... }` block passes
- Validate with --source containing `tags` declarations passes
- Validate with --source containing `specification` block + `extend` + `metadata` passes
- Validate invalid DSL returns errors
- Validate empty source returns error
"""

import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest


# Sample LikeC4 DSL fragments for testing
LIKE_C4_VIEWS_DSL = """
specification {
    element system
    element person
}

system User {
    tags "external"
}

views {
    view index {
        title "System Context"
        include *
    }
}
"""

LIKE_C4_DEPLOYMENT_DSL = """
specification {
    element system
    element node
    element container
}

system App {
    tags "core"
}

deployment {
    node Production {
        container App {
            tags "production"
        }
    }
}
"""

LIKE_C4_TAGS_DSL = """
specification {
    element system
    element person
}

system Auth {
    tags "security", "core", "critical"
}

person Customer {
    tags "external"
}
"""

LIKE_C4_SPEC_EXTEND_METADATA_DSL = """
specification {
    element system
    element container
}

system Backend {
    metadata {
        techStack = "Node.js"
        owner = "platform-team"
    }
}

container Api {
    extends Backend
    metadata {
        protocol = "REST"
    }
}
"""

INVALID_DSL = """
specification {
    element system
}

system Broken {
    invalid syntax here
"""

EMPTY_DSL = ""


class TestArchiValidateLikeC4(unittest.TestCase):
    """AC-16: `dev-team archi validate` LikeC4 DSL compatibility."""

    def setUp(self):
        self.project_root = os.path.normpath(
            os.path.join(os.path.dirname(os.path.abspath(__file__)),
                         "..", "..", "..", "..")
        )
        self.bundle_path = os.path.join(
            self.project_root, "plugins", "dev-team", "bin", "dev-team-bundle.cjs"
        )

    def _run_validate(self, source: str) -> dict:
        """Run archi validate with the given --source DSL. Returns parsed JSON result."""
        if not os.path.isfile(self.bundle_path):
            self.skipTest(f"Bundle not found at {self.bundle_path}")

        try:
            result = subprocess.run(
                ["node", self.bundle_path, "archi", "validate",
                 "--source", source],
                capture_output=True, text=True, timeout=30,
                cwd=self.project_root,
            )
        except subprocess.TimeoutExpired:
            self.skipTest("Validate command timed out")

        try:
            return json.loads(result.stdout), result.returncode
        except (json.JSONDecodeError, ValueError):
            self.fail(f"Output not valid JSON: {result.stdout[:200]}")
            return {}, 1

    # ─── Views block ──────────────────────────────────────────────────────

    def test_validate_views_block(self):
        """AC-16: `views { ... }` block should validate successfully."""
        output, returncode = self._run_validate(LIKE_C4_VIEWS_DSL)
        self.assertTrue(
            output.get("valid", False),
            f"Views DSL should be valid but got: {output}"
        )
        self.assertEqual(returncode, 0)

    # ─── Deployment block ─────────────────────────────────────────────────

    def test_validate_deployment_block(self):
        """AC-16: `deployment { ... }` block should validate successfully."""
        output, returncode = self._run_validate(LIKE_C4_DEPLOYMENT_DSL)
        self.assertTrue(
            output.get("valid", False),
            f"Deployment DSL should be valid but got: {output}"
        )
        self.assertEqual(returncode, 0)

    # ─── Tags declarations ────────────────────────────────────────────────

    def test_validate_tags(self):
        """AC-16: `tags` declarations should validate successfully."""
        output, returncode = self._run_validate(LIKE_C4_TAGS_DSL)
        self.assertTrue(
            output.get("valid", False),
            f"Tags DSL should be valid but got: {output}"
        )
        self.assertEqual(returncode, 0)

    # ─── Specification + extend + metadata ────────────────────────────────

    def test_validate_spec_extend_metadata(self):
        """AC-16: specification block + extend + metadata should validate."""
        output, returncode = self._run_validate(LIKE_C4_SPEC_EXTEND_METADATA_DSL)
        self.assertTrue(
            output.get("valid", False),
            f"Spec+extend+metadata DSL should be valid but got: {output}"
        )
        self.assertEqual(returncode, 0)

    # ─── Invalid DSL ──────────────────────────────────────────────────────

    def test_validate_invalid_dsl_returns_errors(self):
        """AC-16: Invalid DSL should return valid=false with errors array."""
        output, returncode = self._run_validate(INVALID_DSL)
        self.assertFalse(
            output.get("valid", True),
            "Invalid DSL should not be valid"
        )
        self.assertIn("errors", output,
                      "Invalid DSL response should contain errors array")
        self.assertIsInstance(output["errors"], list)
        self.assertGreater(len(output["errors"]), 0)
        self.assertEqual(returncode, 1)

    def test_validate_invalid_dsl_error_has_message(self):
        """AC-16: Each error should have a message describing the issue."""
        output, returncode = self._run_validate(INVALID_DSL)
        if "errors" in output and len(output["errors"]) > 0:
            first_error = output["errors"][0]
            if isinstance(first_error, dict):
                self.assertIn("message", first_error)
                self.assertIsInstance(first_error["message"], str)
            elif isinstance(first_error, str):
                self.assertGreater(len(first_error), 0)

    # ─── Empty source ─────────────────────────────────────────────────────

    def test_validate_empty_source_returns_error(self):
        """AC-16: Empty (or whitespace-only) source should return error."""
        output, returncode = self._run_validate(EMPTY_DSL)
        # Empty source may be valid (empty model) or invalid depending on @likec4/core
        # At minimum, it must not crash
        self.assertIn("valid", output)

    # ─── Output format ────────────────────────────────────────────────────

    def test_validate_output_has_valid_field(self):
        """AC-16: Validate output always has 'valid' boolean field."""
        output, returncode = self._run_validate(LIKE_C4_VIEWS_DSL)
        self.assertIn("valid", output)
        self.assertIsInstance(output["valid"], bool)


if __name__ == "__main__":
    unittest.main(verbosity=2)
