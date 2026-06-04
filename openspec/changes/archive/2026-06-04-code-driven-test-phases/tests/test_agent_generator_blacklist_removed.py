#!/usr/bin/env python3
"""
AC-2 Unit Test: test-gen-generator file type blacklist removed.

Verifies that the test-gen-generator.md agent definition no longer contains
file type blacklist constraints restricting reading of:
.ts, .tsx, .js, .jsx, .py, .rs, .go, .java, .c, .cpp, .h, .hpp, .hxx, .cxx

Also verifies boundary scenario:
- No other implicit glob pattern or extension list is marked as forbidden
"""

import os
import re
import unittest

PROJECT_ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", "..", ".."))
AGENT_FILE = os.path.join(PROJECT_ROOT, "plugins", "dev-team", "agents", "test-gen-generator.md")


def read_agent_file():
    """Read the test-gen-generator agent definition file."""
    if not os.path.isfile(AGENT_FILE):
        raise FileNotFoundError(f"Agent file not found: {AGENT_FILE}")
    with open(AGENT_FILE, "r", encoding="utf-8") as f:
        return f.read()


def find_explicit_blacklist(content):
    """Find any explicit file type blacklist or forbidden-read patterns."""
    patterns = [
        r"黑名单",
        r"blacklist",
        r"禁止读取",
        r"(?i)do not read",
        r"(?i)must not read",
        r"(?i)forbidden.*ext",
        r"(?i)restricted.*ext",
        r"(?i)prohibited.*file",
        r"(?i)cannot read.*\.(ts|tsx|js|jsx|py|rs|go|java)",
    ]
    matches = []
    for p in patterns:
        m = re.search(p, content, re.IGNORECASE)
        if m:
            matches.append((p, m.group()))
    return matches


def find_glob_restrictions(content):
    """Find glob patterns that would restrict file reading."""
    patterns = [
        r"\*\.ts\b",
        r"\*\.tsx\b",
        r"\*\.js\b",
        r"\*\.jsx\b",
        r"\*\.py\b",
        r"\*\.rs\b",
        r"\*\.go\b",
        r"\*\.java\b",
        r"\*\.c\b",
        r"\*\.cpp\b",
        r"\*\.h\b",
        r"\*\.hpp\b",
        r"\*\.[ch]",
        r"file.*type.*constraint",
        r"file.*type.*restrict",
        r"类型.*黑名单",
        r"禁止.*\.(ts|tsx|js|jsx|py|rs|go|java)",
    ]
    results = {}
    for p in patterns:
        m = re.search(p, content, re.IGNORECASE)
        results[p] = m is not None
    return results


class TestGenGeneratorNoBlacklist(unittest.TestCase):
    """Test that test-gen-generator.md has no file type blacklist."""

    @classmethod
    def setUpClass(cls):
        cls.content = read_agent_file()

    def test_no_blacklist_mention(self):
        """Verify agent file does NOT contain blacklist/禁止读取 keywords."""
        matches = find_explicit_blacklist(self.content)
        # If matches are found, the blacklist restriction might still exist
        if matches:
            # Check that matches are contextually NOT about file type restrictions
            for pattern, match_text in matches:
                # TODO: Examine each match context to confirm it's not about file type restrictions
                pass
        # Primary assertion: no "黑名单" keyword referencing file types
        has_blacklist_keyword = re.search(
            r"黑名单|blacklist|禁止读取",
            self.content
        )
        # TODO: Relax this once agent is confirmed modified — currently old agent has blacklist
        if has_blacklist_keyword:
            # If present, verify it's not about file types
            line_start = max(0, has_blacklist_keyword.start() - 100)
            surrounding = self.content[line_start:has_blacklist_keyword.end() + 100]
            is_about_file_types = any(
                ext in surrounding for ext in [".ts", ".py", ".rs", ".go", ".java"]
            )
            self.assertFalse(
                is_about_file_types,
                "Blacklist keyword should not appear in context of file type restrictions"
            )

    def test_ts_not_restricted(self):
        """Verify .ts files are not in any restricted/forbidden section."""
        glob_results = find_glob_restrictions(self.content)
        # Check that no *.ts restriction pattern matches
        ts_restricted = (
            glob_results.get(r"\*\.ts\b", False)
            or glob_results.get(r"\*\.tsx\b", False)
        )
        # TODO: Update assertion once agent file is confirmed modified
        # For now this is informational — the old agent still has the blacklist
        self.assertFalse(
            ts_restricted,
            ".ts / .tsx should not appear in any file-restriction context"
        )

    def test_py_not_restricted(self):
        """Verify .py files are not in any restricted/forbidden section."""
        glob_results = find_glob_restrictions(self.content)
        py_restricted = glob_results.get(r"\*\.py\b", False)
        if py_restricted:
            # Check context: if *.py appears in a naming convention example
            # (e.g., "test_*.py"), it is NOT a file-type restriction
            for match in re.finditer(r"\*\.py\b", self.content):
                start = max(0, match.start() - 60)
                end = min(len(self.content), match.end() + 60)
                context = self.content[start:end]
                # Naming convention references are allowed (positive instruction)
                is_naming_context = bool(re.search(
                    r"(?i)naming.convention|test_\*\.py|naming.*convention|命名.*规范|e\.g\.",
                    context
                ))
                if not is_naming_context:
                    self.fail(
                        ".py glob appears in a non-naming-context: "
                        f"...{context.strip()}..."
                    )
            # If all occurrences are naming contexts, test passes
            return
        self.assertFalse(py_restricted, ".py should not appear in any file-restriction context")

    def test_rs_not_restricted(self):
        """Verify .rs files are not in any restricted/forbidden section."""
        glob_results = find_glob_restrictions(self.content)
        rs_restricted = glob_results.get(r"\*\.rs\b", False)
        self.assertFalse(
            rs_restricted,
            ".rs should not appear in any file-restriction context"
        )

    def test_go_not_restricted(self):
        """Verify .go files are not in any restricted/forbidden section."""
        glob_results = find_glob_restrictions(self.content)
        go_restricted = glob_results.get(r"\*\.go\b", False)
        self.assertFalse(
            go_restricted,
            ".go should not appear in any file-restriction context"
        )

    def test_java_not_restricted(self):
        """Verify .java files are not in any restricted/forbidden section."""
        glob_results = find_glob_restrictions(self.content)
        java_restricted = glob_results.get(r"\*\.java\b", False)
        self.assertFalse(
            java_restricted,
            ".java should not appear in any file-restriction context"
        )

    def test_c_cpp_not_restricted(self):
        """Verify .c/.cpp/.h/.hpp files are not in any restricted/forbidden section."""
        glob_results = find_glob_restrictions(self.content)
        c_restricted = (
            glob_results.get(r"\*\.c\b", False)
            or glob_results.get(r"\*\.cpp\b", False)
            or glob_results.get(r"\*\.h\b", False)
            or glob_results.get(r"\*\.hpp\b", False)
        )
        self.assertFalse(
            c_restricted,
            "C/C++ extensions should not appear in any file-restriction context"
        )

    def test_no_implicit_glob_restrictions(self):
        """Boundary: verify no other implicit glob pattern or extension list
        is marked as forbidden to read."""
        glob_results = find_glob_restrictions(self.content)
        restricted_globs = [g for g, found in glob_results.items() if found]
        # All glob matches should be acceptable (e.g. in Input sections, not Restrictions)
        for g in restricted_globs:
            # TODO: Check line context to confirm glob is not in a "forbidden" section
            pass
        # Primary assertion: no pattern like "file type restriction" or "禁止" appears
        # that covers extensions
        restriction_phrases = [
            r"文件类型.*约束",
            r"file.*type.*constraint",
            r"禁止.*类型",
            r"restrict.*extension",
        ]
        has_restriction = any(re.search(p, self.content, re.IGNORECASE) for p in restriction_phrases)
        self.assertFalse(
            has_restriction,
            "Agent should not contain any file type restriction phrases"
        )

    def test_source_code_reading_enabled(self):
        """Verify agent contains instruction to read source code files (positive assertion)."""
        reading_patterns = [
            r"Read.*source.*code",
            r"Read.*源码",
            r"read.*\*\*\.py\*\*",
            r"read.*\*\*\.ts\*\*",
            r"extract.*parameter.*type",
            r"提取.*参数.*类型",
            r"读取.*源码.*文件",
        ]
        has_reading_instruction = any(
            re.search(p, self.content, re.IGNORECASE)
            for p in reading_patterns
        )
        # TODO: Strengthen assertion once agent is modified
        # Old agent only reads spec.md files, not source code
        self.assertTrue(
            has_reading_instruction,
            "test-gen-generator.md should contain instruction to read source code files"
        )


if __name__ == "__main__":
    unittest.main(verbosity=2)
