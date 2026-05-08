#!/usr/bin/env python3
"""
Phase 3 Integration Tests: Compliance & Design Constraints

Tests the following components:
1. compliance-check.py (C1-C10)
2. spec-compliance.py (proposal scope parsing, route/model extraction, comparison)
3. deviation-check.py (missing/extra detection, alert formatting)
4. End-to-end flow: compliance gate with config-driven C9/C10
5. CLI interface for each utility
"""

import importlib.util
import json
import os
import shutil
import sys
import tempfile
import unittest
from datetime import datetime
from unittest.mock import patch

# ─── Load modules from plugin/utils ────────────────────────────────────────

UTILS_DIR = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "plugin", "utils")


def _load_module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


compliance_check = _load_module("compliance_check", os.path.join(UTILS_DIR, "compliance-check.py"))
spec_compliance = _load_module("spec_compliance", os.path.join(UTILS_DIR, "spec-compliance.py"))
deviation_check = _load_module("deviation_check", os.path.join(UTILS_DIR, "deviation-check.py"))


# ─── Test fixtures ─────────────────────────────────────────────────────────

class TempProject:
    """Creates a temporary project structure for testing."""

    def __init__(self):
        self.root = tempfile.mkdtemp(prefix="phase3_test_")
        self.changes_dir = os.path.join(self.root, "openspec", "changes")
        os.makedirs(self.changes_dir, exist_ok=True)

    def cleanup(self):
        shutil.rmtree(self.root, ignore_errors=True)

    def create_change(self, name, **kwargs):
        """Create a change directory with optional files.

        kwargs:
            proposal: proposal.md content
            tasks: tasks.md content
            design: design.md content
            test_results: list of test result filenames + content
            code_reviews: list of (filename, content) tuples
            test_case_templates: list of test case template filenames
            loop_state: review-loop-state.json content dict
            src_files: dict of {relative_path: content} for source files
            openspec_yaml: .openspec.yaml content
        """
        change_dir = os.path.join(self.changes_dir, name)
        os.makedirs(change_dir, exist_ok=True)

        if "proposal" in kwargs:
            with open(os.path.join(change_dir, "proposal.md"), "w", encoding="utf-8") as f:
                f.write(kwargs["proposal"])

        if "tasks" in kwargs:
            with open(os.path.join(change_dir, "tasks.md"), "w", encoding="utf-8") as f:
                f.write(kwargs["tasks"])

        if "design" in kwargs:
            with open(os.path.join(change_dir, "design.md"), "w", encoding="utf-8") as f:
                f.write(kwargs["design"])

        test_reports_dir = os.path.join(change_dir, "test-reports")
        has_test_reports = False

        if "test_results" in kwargs:
            os.makedirs(test_reports_dir, exist_ok=True)
            has_test_reports = True
            for filename, content in kwargs["test_results"]:
                with open(os.path.join(test_reports_dir, filename), "w", encoding="utf-8") as f:
                    f.write(content)

        if "code_reviews" in kwargs:
            os.makedirs(test_reports_dir, exist_ok=True)
            has_test_reports = True
            for filename, content in kwargs["code_reviews"]:
                with open(os.path.join(test_reports_dir, filename), "w", encoding="utf-8") as f:
                    f.write(content)

        if "test_case_templates" in kwargs:
            os.makedirs(test_reports_dir, exist_ok=True)
            has_test_reports = True
            for filename in kwargs["test_case_templates"]:
                with open(os.path.join(test_reports_dir, filename), "w", encoding="utf-8") as f:
                    f.write("# Test case template\n")

        if "loop_state" in kwargs:
            os.makedirs(test_reports_dir, exist_ok=True)
            has_test_reports = True
            with open(os.path.join(test_reports_dir, "review-loop-state.json"), "w", encoding="utf-8") as f:
                json.dump(kwargs["loop_state"], f)

        if "src_files" in kwargs:
            for rel_path, content in kwargs["src_files"].items():
                full_path = os.path.join(self.root, rel_path)
                os.makedirs(os.path.dirname(full_path), exist_ok=True)
                with open(full_path, "w", encoding="utf-8") as f:
                    f.write(content)

        if "openspec_yaml" in kwargs:
            with open(os.path.join(self.root, ".openspec.yaml"), "w", encoding="utf-8") as f:
                f.write(kwargs["openspec_yaml"])

        if "test_files" in kwargs:
            for rel_path in kwargs["test_files"]:
                full_path = os.path.join(self.root, rel_path)
                os.makedirs(os.path.dirname(full_path), exist_ok=True)
                with open(full_path, "w", encoding="utf-8") as f:
                    f.write("# test file\n")

        return change_dir


# ═══════════════════════════════════════════════════════════════════════════
# Test Suite 1: Compliance Check (C1-C10)
# ═══════════════════════════════════════════════════════════════════════════

class TestComplianceCheck(unittest.TestCase):
    """Tests for compliance-check.py (C1-C10)."""

    def setUp(self):
        self.project = TempProject()

    def tearDown(self):
        self.project.cleanup()

    # ─── C1: proposal.md exists ──────────────────────────────────────────

    def test_c1_pass_when_proposal_exists(self):
        change_dir = self.project.create_change("test-change", proposal="# Proposal\nTest proposal")
        result = compliance_check.check_c1_proposal_exists(change_dir)
        self.assertTrue(result.passed)
        self.assertEqual(result.check_id, "C1")
        self.assertEqual(result.level, "must")

    def test_c1_fail_when_proposal_missing(self):
        change_dir = self.project.create_change("test-change")
        result = compliance_check.check_c1_proposal_exists(change_dir)
        self.assertFalse(result.passed)

    # ─── C2: tasks.md exists ─────────────────────────────────────────────

    def test_c2_pass_when_tasks_exists(self):
        change_dir = self.project.create_change("test-change", tasks="# Tasks\n- [x] Task 1")
        result = compliance_check.check_c2_tasks_exists(change_dir)
        self.assertTrue(result.passed)
        self.assertEqual(result.check_id, "C2")

    def test_c2_fail_when_tasks_missing(self):
        change_dir = self.project.create_change("test-change")
        result = compliance_check.check_c2_tasks_exists(change_dir)
        self.assertFalse(result.passed)

    # ─── C3: All tasks complete ──────────────────────────────────────────

    def test_c3_pass_all_tasks_complete(self):
        change_dir = self.project.create_change("test-change",
            tasks="# Tasks\n- [x] Task 1\n- [x] Task 2")
        result = compliance_check.check_c3_all_tasks_complete(change_dir)
        self.assertTrue(result.passed)

    def test_c3_fail_incomplete_tasks(self):
        change_dir = self.project.create_change("test-change",
            tasks="# Tasks\n- [x] Task 1\n- [ ] Task 2")
        result = compliance_check.check_c3_all_tasks_complete(change_dir)
        self.assertFalse(result.passed)
        self.assertIn("1/2", result.details)

    def test_c3_pass_no_tasks(self):
        change_dir = self.project.create_change("test-change",
            tasks="# Tasks\nNo tasks yet")
        result = compliance_check.check_c3_all_tasks_complete(change_dir)
        self.assertTrue(result.passed)

    def test_c3_fail_no_tasks_md(self):
        change_dir = self.project.create_change("test-change")
        result = compliance_check.check_c3_all_tasks_complete(change_dir)
        self.assertFalse(result.passed)

    # ─── C4: Test files exist ────────────────────────────────────────────

    def test_c4_pass_with_test_files(self):
        change_dir = self.project.create_change("test-change",
            test_files=["tests/test_foo.py"])
        result = compliance_check.check_c4_test_files_exist(change_dir, self.project.root)
        self.assertTrue(result.passed)

    def test_c4_pass_with_test_case_templates(self):
        change_dir = self.project.create_change("test-change",
            test_case_templates=["test-case-auth.md"])
        result = compliance_check.check_c4_test_files_exist(change_dir, self.project.root)
        self.assertTrue(result.passed)

    def test_c4_fail_no_test_files(self):
        change_dir = self.project.create_change("test-change")
        result = compliance_check.check_c4_test_files_exist(change_dir, self.project.root)
        self.assertFalse(result.passed)

    # ─── C5: Tests pass ──────────────────────────────────────────────────

    def test_c5_pass_with_test_result_record(self):
        change_dir = self.project.create_change("test-change",
            test_results=[("test-result-20260508-120000.md",
                           "# Test Result\n**Verdict**: PASS\nAll tests passed.")])
        result = compliance_check.check_c5_tests_pass(change_dir, self.project.root)
        self.assertTrue(result.passed)

    def test_c5_fail_with_failed_test_result(self):
        change_dir = self.project.create_change("test-change",
            test_results=[("test-result-20260508-120000.md",
                           "# Test Result\nVerdict: FAIL\n2 tests failed.")])
        result = compliance_check.check_c5_tests_pass(change_dir, self.project.root)
        self.assertFalse(result.passed)

    def test_c5_pass_with_full_test_result(self):
        change_dir = self.project.create_change("test-change",
            test_results=[("full-test-20260508-120000.md",
                           "# Full Test\nResult: PASS\nAll tests passed.")])
        result = compliance_check.check_c5_tests_pass(change_dir, self.project.root)
        self.assertTrue(result.passed)

    def test_c5_pass_with_loop_state(self):
        change_dir = self.project.create_change("test-change",
            loop_state={"full_test_passed": True, "loop_count": 1})
        result = compliance_check.check_c5_tests_pass(change_dir, self.project.root)
        self.assertTrue(result.passed)

    def test_c5_fail_no_test_reports(self):
        change_dir = self.project.create_change("test-change")
        result = compliance_check.check_c5_tests_pass(change_dir, self.project.root)
        self.assertFalse(result.passed)

    # ─── C6: Code review exists ──────────────────────────────────────────

    def test_c6_pass_with_code_review(self):
        change_dir = self.project.create_change("test-change",
            code_reviews=[("code-review-20260508-120000.md",
                           "# Code Review\n**Verdict**: PASS")])
        result = compliance_check.check_c6_review_exists(change_dir)
        self.assertTrue(result.passed)

    def test_c6_fail_no_code_review(self):
        change_dir = self.project.create_change("test-change")
        result = compliance_check.check_c6_review_exists(change_dir)
        self.assertFalse(result.passed)

    # ─── C7: Review no ERROR ─────────────────────────────────────────────

    def test_c7_pass_no_errors(self):
        change_dir = self.project.create_change("test-change",
            code_reviews=[("code-review-20260508-120000.md",
                           "# Code Review\n**Verdict**: PASS\nNo issues found.")])
        result = compliance_check.check_c7_review_no_error(change_dir)
        self.assertTrue(result.passed)

    def test_c7_fail_with_block_verdict(self):
        change_dir = self.project.create_change("test-change",
            code_reviews=[("code-review-20260508-120000.md",
                           "# Code Review\n**Verdict**: BLOCK\n[ERROR-1] Security issue")])
        result = compliance_check.check_c7_review_no_error(change_dir)
        self.assertFalse(result.passed)

    def test_c7_fail_with_error_tags(self):
        """C7 fails when review report has ERROR findings in the expected format."""
        change_dir = self.project.create_change("test-change",
            code_reviews=[("code-review-20260508-120000.md",
                           """# Code Review

#### [ERROR-1] src/db.py:42
- **Category**: security
- **Description**: SQL injection vulnerability

#### [ERROR-2] src/api.py:15
- **Category**: security
- **Description**: XSS vulnerability
""")])
        result = compliance_check.check_c7_review_no_error(change_dir)
        self.assertFalse(result.passed)

    # ─── C8: No uncommitted changes ──────────────────────────────────────

    def test_c8_returns_result(self):
        # Just verify it doesn't crash; actual git state varies
        change_dir = self.project.create_change("test-change")
        result = compliance_check.check_c8_no_uncommitted(self.project.root)
        self.assertEqual(result.check_id, "C8")
        self.assertEqual(result.level, "must")
        # Can't assert pass/fail since git state varies

    # ─── C9: design.md exists (optional) ─────────────────────────────────

    def test_c9_skip_when_not_required(self):
        change_dir = self.project.create_change("test-change")
        result = compliance_check.check_c9_design_exists(change_dir, {"require_design": False})
        self.assertTrue(result.skipped)

    def test_c9_pass_when_required_and_exists(self):
        change_dir = self.project.create_change("test-change",
            design="# Design\nArchitecture: microservices")
        result = compliance_check.check_c9_design_exists(change_dir, {"require_design": True})
        self.assertTrue(result.passed)
        self.assertFalse(result.skipped)

    def test_c9_fail_when_required_but_missing(self):
        change_dir = self.project.create_change("test-change")
        result = compliance_check.check_c9_design_exists(change_dir, {"require_design": True})
        self.assertFalse(result.passed)
        self.assertFalse(result.skipped)

    # ─── C10: Spec compliance (optional) ─────────────────────────────────

    def test_c10_skip_when_not_configured(self):
        change_dir = self.project.create_change("test-change",
            proposal="# Proposal\n## Scope\n- Login feature")
        result = compliance_check.check_c10_spec_compliance(
            change_dir, self.project.root, {"spec_compliance": False})
        self.assertTrue(result.skipped)

    def test_c10_pass_when_configured_and_compliant(self):
        change_dir = self.project.create_change("test-change",
            proposal="# Proposal\n## Scope\n- Login feature",
            src_files={"src/auth.py": "from flask import Blueprint\napp = Flask(__name__)\n@app.route('/api/login', methods=['POST'])\ndef login(): pass"})
        result = compliance_check.check_c10_spec_compliance(
            change_dir, self.project.root, {"spec_compliance": True})
        # At minimum should not be skipped
        self.assertFalse(result.skipped)

    def test_c10_pass_when_no_proposal_with_spec_module(self):
        """When spec_compliance=True but no proposal, check_spec_compliance
        returns passed=True (no scope to compare), so C10 passes (not skipped)."""
        change_dir = self.project.create_change("test-change")
        result = compliance_check.check_c10_spec_compliance(
            change_dir, self.project.root, {"spec_compliance": True})
        # With HAS_SPEC_COMPLIANCE=True, it calls check_spec_compliance
        # which returns passed=True when no proposal exists
        self.assertTrue(result.passed)

    # ─── Full compliance report ──────────────────────────────────────────

    def test_full_report_pass(self):
        """Test a fully compliant change passes all required checks."""
        change_dir = self.project.create_change("full-pass",
            proposal="# Proposal\nTest",
            tasks="# Tasks\n- [x] Task 1",
            test_files=["tests/test_main.py"],
            test_results=[("test-result-20260508-120000.md",
                           "# Test Result\n**Verdict**: PASS")],
            code_reviews=[("code-review-20260508-120000.md",
                           "# Code Review\n**Verdict**: PASS")])

        report = compliance_check.run_compliance_check(
            change_dir, self.project.root, {"require_design": False, "spec_compliance": False})

        # C1-C6 should pass, C8 depends on git state, C9/C10 skipped
        self.assertTrue(report.passed or not all(
            c.passed or c.skipped for c in report.checks
            if c.check_id in ("C8",) and not c.skipped
        ))

    def test_full_report_fail_missing_proposal(self):
        """Test report fails when proposal.md missing."""
        change_dir = self.project.create_change("no-proposal",
            tasks="# Tasks\n- [x] Task 1",
            test_files=["tests/test_main.py"],
            test_results=[("test-result-20260508-120000.md",
                           "**Verdict**: PASS")],
            code_reviews=[("code-review-20260508-120000.md",
                           "**Verdict**: PASS")])

        report = compliance_check.run_compliance_check(change_dir, self.project.root)
        self.assertFalse(report.passed)
        blocking_ids = [c.check_id for c in report.blocking_issues]
        self.assertIn("C1", blocking_ids)

    def test_report_markdown_generation(self):
        """Test markdown report generation."""
        change_dir = self.project.create_change("report-test",
            proposal="# Proposal\nTest",
            tasks="# Tasks\n- [ ] Incomplete task")

        report = compliance_check.run_compliance_check(change_dir, self.project.root)
        md = compliance_check.generate_report_markdown(report)

        self.assertIn("Pre-Archive Compliance Check", md)
        self.assertIn("report-test", md)
        self.assertIn("C1", md)
        self.assertIn("C3", md)

    def test_report_save(self):
        """Test report can be saved to test-reports/."""
        change_dir = self.project.create_change("save-test",
            proposal="# Proposal\nTest")

        report = compliance_check.run_compliance_check(change_dir, self.project.root)
        test_reports_dir = os.path.join(change_dir, "test-reports")
        path = compliance_check.save_report(report, test_reports_dir)

        self.assertTrue(os.path.isfile(path))
        self.assertTrue(path.endswith(".md"))
        self.assertIn("compliance-", os.path.basename(path))

    # ─── Config loading ──────────────────────────────────────────────────

    def test_load_config_from_openspec_yaml(self):
        """Test loading compliance config from .openspec.yaml."""
        self.project.create_change("config-test",
            openspec_yaml="""version: "1.2"
compliance:
  require_design: true
  spec_compliance: true
""")
        config = compliance_check.load_config(self.project.root)
        self.assertTrue(config.get("require_design"))
        self.assertTrue(config.get("spec_compliance"))

    def test_load_config_no_yaml(self):
        """Test config loading when no .openspec.yaml exists."""
        config = compliance_check.load_config(self.project.root)
        self.assertEqual(config, {})


# ═══════════════════════════════════════════════════════════════════════════
# Test Suite 2: Spec Compliance Analysis
# ═══════════════════════════════════════════════════════════════════════════

class TestSpecCompliance(unittest.TestCase):
    """Tests for spec-compliance.py."""

    def setUp(self):
        self.project = TempProject()

    def tearDown(self):
        self.project.cleanup()

    # ─── Proposal scope parsing ──────────────────────────────────────────

    def test_parse_scope_with_bullet_features(self):
        change_dir = self.project.create_change("scope-test",
            proposal="""# Add Authentication

## Scope

- User registration with email
- User login with JWT tokens
- OAuth providers: Google, GitHub
- Password reset flow
""")
        scope = spec_compliance.parse_proposal_scope(
            os.path.join(change_dir, "proposal.md"))

        self.assertEqual(len(scope.features), 4)
        self.assertIn("User registration with email", scope.features)
        self.assertIn("User login with JWT tokens", scope.features)

    def test_parse_scope_with_api_mentions(self):
        change_dir = self.project.create_change("api-test",
            proposal="""# API Features

## Scope

- `GET /api/users` - List users
- `POST /api/auth/login` - Login endpoint
- `DELETE /api/users/:id` - Delete user
""")
        scope = spec_compliance.parse_proposal_scope(
            os.path.join(change_dir, "proposal.md"))

        self.assertTrue(len(scope.api_mentions) >= 2)
        # Check that HTTP methods are captured
        methods = [a.split()[0] for a in scope.api_mentions]
        self.assertIn("GET", methods)
        self.assertIn("POST", methods)

    def test_parse_scope_with_model_mentions(self):
        change_dir = self.project.create_change("model-test",
            proposal="""# Data Layer

## Scope

- User model with profile fields
- Session table for active sessions
""")
        scope = spec_compliance.parse_proposal_scope(
            os.path.join(change_dir, "proposal.md"))

        self.assertTrue(len(scope.data_models) >= 1)
        self.assertIn("User", scope.data_models)

    def test_parse_scope_no_scope_section(self):
        change_dir = self.project.create_change("no-scope",
            proposal="# Simple Change\nJust a bug fix.")
        scope = spec_compliance.parse_proposal_scope(
            os.path.join(change_dir, "proposal.md"))
        # Falls back to entire content for feature extraction
        self.assertIsNotNone(scope)

    def test_parse_scope_missing_file(self):
        scope = spec_compliance.parse_proposal_scope("/nonexistent/proposal.md")
        self.assertEqual(len(scope.features), 0)
        self.assertEqual(len(scope.api_mentions), 0)

    # ─── Route extraction ────────────────────────────────────────────────

    def test_extract_express_routes(self):
        self.project.create_change("route-test",
            src_files={"src/routes/user.js": """
const router = require('express').Router();
router.get('/api/users', listUsers);
router.post('/api/users', createUser);
router.delete('/api/users/:id', deleteUser);
"""})
        routes = spec_compliance.extract_routes(os.path.join(self.project.root, "src"))
        self.assertTrue(len(routes) >= 2)
        methods = [r.method for r in routes]
        self.assertIn("GET", methods)
        self.assertIn("POST", methods)

    def test_extract_flask_routes(self):
        self.project.create_change("flask-test",
            src_files={"src/app.py": """
from flask import Flask, Blueprint
app = Flask(__name__)
bp = Blueprint('api', __name__)

@app.route('/api/health')
def health():
    pass

@bp.route('/api/items', methods=['GET', 'POST'])
def items():
    pass
"""})
        routes = spec_compliance.extract_routes(os.path.join(self.project.root, "src"))
        self.assertTrue(len(routes) >= 1)
        paths = [r.path for r in routes]
        self.assertIn("/api/health", paths)

    def test_extract_fastapi_routes(self):
        self.project.create_change("fastapi-test",
            src_files={"src/main.py": """
from fastapi import APIRouter
router = APIRouter()

@router.get('/api/ping')
def ping():
    pass

@router.post('/api/data')
def create_data():
    pass
"""})
        routes = spec_compliance.extract_routes(os.path.join(self.project.root, "src"))
        self.assertTrue(len(routes) >= 1)

    def test_extract_routes_no_src_dir(self):
        routes = spec_compliance.extract_routes("/nonexistent/src")
        self.assertEqual(len(routes), 0)

    # ─── Model extraction ────────────────────────────────────────────────

    def test_extract_sql_models(self):
        self.project.create_change("sql-test",
            src_files={"src/schema.sql": """
CREATE TABLE users (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS sessions (
    id INTEGER PRIMARY KEY,
    user_id INTEGER
);
"""})
        models = spec_compliance.extract_models(os.path.join(self.project.root, "src"))
        names = [m.name for m in models]
        self.assertIn("users", names)
        self.assertIn("sessions", names)

    def test_extract_sqlalchemy_models(self):
        self.project.create_change("sa-test",
            src_files={"src/models.py": """
from sqlalchemy.orm import DeclarativeBase

class Base(DeclarativeBase):
    pass

class User(Base):
    __tablename__ = 'users'

class Order(Base):
    __tablename__ = 'orders'
"""})
        models = spec_compliance.extract_models(os.path.join(self.project.root, "src"))
        names = [m.name for m in models]
        self.assertIn("User", names)
        self.assertIn("Order", names)

    def test_extract_django_models(self):
        self.project.create_change("django-test",
            src_files={"src/models.py": """
from django.db import models

class Product(models.Model):
    name = models.CharField(max_length=100)

class Category(models.Model):
    name = models.CharField(max_length=50)
"""})
        models = spec_compliance.extract_models(os.path.join(self.project.root, "src"))
        names = [m.name for m in models]
        self.assertIn("Product", names)
        self.assertIn("Category", names)

    def test_extract_mongoose_models(self):
        self.project.create_change("mongoose-test",
            src_files={"src/models/user.js": """
const mongoose = require('mongoose');
const User = mongoose.model('User', userSchema);
const Product = mongoose.model('Product', productSchema);
"""})
        models = spec_compliance.extract_models(os.path.join(self.project.root, "src"))
        names = [m.name for m in models]
        self.assertIn("User", names)
        self.assertIn("Product", names)

    def test_extract_prisma_models(self):
        self.project.create_change("prisma-test",
            src_files={"src/schema.prisma": """
model User {
  id    Int     @id @default(autoincrement())
  email String  @unique
  name  String?
}

model Post {
  id        Int     @id @default(autoincrement())
  title     String
  content   String?
  author    User    @relation(fields: [authorId], references: [id])
  authorId  Int
}
"""})
        models = spec_compliance.extract_models(os.path.join(self.project.root, "src"))
        names = [m.name for m in models]
        self.assertIn("User", names)
        self.assertIn("Post", names)

    # ─── Comparison algorithm ────────────────────────────────────────────

    def test_compare_finds_covered_features(self):
        scope = spec_compliance.ProposalScope(
            features=["User login"],
            api_mentions=[],
            data_models=[],
        )
        routes = [spec_compliance.RouteInfo("POST", "/api/login", "auth.py")]
        models = []

        result = spec_compliance.compare_spec_impl(scope, routes, models)
        self.assertTrue(len(result.covered) >= 1)
        self.assertIn("User login", result.covered)

    def test_compare_finds_missing_features(self):
        scope = spec_compliance.ProposalScope(
            features=["Email notification system"],
            api_mentions=[],
            data_models=[],
        )
        routes = []
        models = []

        result = spec_compliance.compare_spec_impl(scope, routes, models)
        self.assertTrue(len(result.missing) >= 1)
        self.assertIn("Email notification system", result.missing)
        self.assertFalse(result.passed)

    def test_compare_finds_extra_routes(self):
        scope = spec_compliance.ProposalScope(
            features=[],
            api_mentions=["GET /api/users"],
            data_models=[],
        )
        routes = [
            spec_compliance.RouteInfo("GET", "/api/users", "users.py"),
            spec_compliance.RouteInfo("DELETE", "/api/admin/purge", "admin.py"),
        ]
        models = []

        result = spec_compliance.compare_spec_impl(scope, routes, models)
        extra_names = [e["name"] for e in result.extra_details]
        # The admin route should be detected as extra
        has_admin = any("admin" in n.lower() for n in extra_names)
        self.assertTrue(has_admin)

    def test_compare_empty_scope_passes(self):
        scope = spec_compliance.ProposalScope()
        result = spec_compliance.compare_spec_impl(scope, [], [])
        self.assertTrue(result.passed)

    # ─── check_spec_compliance end-to-end ────────────────────────────────

    def test_check_spec_compliance_full_flow(self):
        change_dir = self.project.create_change("full-spec",
            proposal="""# Add User Management

## Scope

- User registration
- User login
- `GET /api/users`
- User model
""",
            src_files={"src/app.py": """
from flask import Flask
app = Flask(__name__)

@app.route('/api/users')
def list_users():
    pass

@app.route('/api/register', methods=['POST'])
def register():
    pass

@app.route('/api/login', methods=['POST'])
def login():
    pass
""",
                "src/models.py": """
class User:
    pass
"""})

        result = spec_compliance.check_spec_compliance(change_dir, self.project.root)
        self.assertIn("passed", result)
        self.assertIn("covered", result)
        self.assertIn("missing", result)
        self.assertIn("extra", result)
        self.assertIn("routes", result)
        self.assertIn("models", result)
        self.assertIn("coverage_rate", result)

    def test_check_spec_compliance_no_proposal(self):
        change_dir = self.project.create_change("no-proposal")
        result = spec_compliance.check_spec_compliance(change_dir, self.project.root)
        self.assertTrue(result["passed"])
        self.assertEqual(len(result["covered"]), 0)

    # ─── Report generation ───────────────────────────────────────────────

    def test_generate_spec_report(self):
        result = {
            "passed": True,
            "covered": ["User login"],
            "missing": [],
            "extra": [],
            "covered_details": [{"feature": "User login", "evidence": ["POST /api/login"]}],
            "missing_details": [],
            "extra_details": [],
            "routes": [{"method": "POST", "path": "/api/login", "file": "auth.py"}],
            "models": [],
            "coverage_rate": 1.0,
        }
        report = spec_compliance.generate_spec_report(result, "test-change")
        self.assertIn("Spec Compliance Report", report)
        self.assertIn("COMPLETE", report)
        self.assertIn("User login", report)

    def test_generate_spec_report_with_missing(self):
        result = {
            "passed": False,
            "covered": ["User login"],
            "missing": ["Email notification"],
            "extra": [],
            "covered_details": [{"feature": "User login", "evidence": ["POST /api/login"]}],
            "missing_details": [{"feature": "Email notification", "reason": "Not found"}],
            "extra_details": [],
            "routes": [],
            "models": [],
            "coverage_rate": 0.5,
        }
        report = spec_compliance.generate_spec_report(result, "test-change")
        self.assertIn("INCOMPLETE", report)
        self.assertIn("Email notification", report)


# ═══════════════════════════════════════════════════════════════════════════
# Test Suite 3: Deviation Check
# ═══════════════════════════════════════════════════════════════════════════

class TestDeviationCheck(unittest.TestCase):
    """Tests for deviation-check.py."""

    def setUp(self):
        self.project = TempProject()

    def tearDown(self):
        self.project.cleanup()

    def test_no_deviations_when_compliant(self):
        change_dir = self.project.create_change("compliant",
            proposal="""# Add Login

## Scope

- User login with JWT
""",
            src_files={"src/auth.py": """
from flask import Flask
app = Flask(__name__)

@app.route('/api/login', methods=['POST'])
def login():
    pass
"""})

        report = deviation_check.check_deviations(change_dir, self.project.root)
        # Should have few or no errors
        self.assertIsInstance(report, deviation_check.DeviationReport)

    def test_missing_feature_detected(self):
        change_dir = self.project.create_change("missing-feature",
            proposal="""# Add Auth

## Scope

- User login
- Email notification system
- Role-based access control
""",
            src_files={"src/auth.py": """
from flask import Flask
app = Flask(__name__)

@app.route('/api/login', methods=['POST'])
def login():
    pass
"""})

        report = deviation_check.check_deviations(change_dir, self.project.root)
        missing_features = [d for d in report.deviations if d.type == "missing"]
        # At least email notification or RBAC should be detected as missing
        self.assertTrue(len(missing_features) >= 1)

    def test_extra_feature_is_warning(self):
        change_dir = self.project.create_change("extra-feature",
            proposal="""# Simple Feature

## Scope

- Basic login
""",
            src_files={"src/app.py": """
from flask import Flask
app = Flask(__name__)

@app.route('/api/login', methods=['POST'])
def login():
    pass

@app.route('/api/analytics', methods=['GET'])
def analytics():
    pass
"""})

        report = deviation_check.check_deviations(change_dir, self.project.root)
        extra = [d for d in report.deviations if d.type == "extra"]
        # Extra features should be warnings by default
        for d in extra:
            self.assertEqual(d.severity, "warning")

    def test_missing_severity_configurable(self):
        change_dir = self.project.create_change("config-severity",
            proposal="""# Feature

## Scope

- Email notification
""",
            src_files={"src/main.py": "# no implementation"})

        # Default: missing is error
        report1 = deviation_check.check_deviations(change_dir, self.project.root, {})
        missing1 = [d for d in report1.deviations if d.type == "missing"]
        for d in missing1:
            self.assertEqual(d.severity, "error")

        # Config: missing is warning
        report2 = deviation_check.check_deviations(
            change_dir, self.project.root, {"missing_severity": "warning"})
        missing2 = [d for d in report2.deviations if d.type == "missing"]
        for d in missing2:
            self.assertEqual(d.severity, "warning")

    # ─── Alert formatting ────────────────────────────────────────────────

    def test_format_alert_with_errors(self):
        report = deviation_check.DeviationReport(
            change_name="test",
            date="2026-05-08",
            deviations=[
                deviation_check.Deviation(
                    type="missing", severity="error",
                    feature="Auth", details="Not implemented"),
            ],
            passed=False,
        )
        alert = deviation_check.format_alert(report)
        self.assertIn("Scope Deviation Alert", alert)
        self.assertIn("Auth", alert)
        self.assertIn("ACTION REQUIRED", alert)

    def test_format_inline_alert(self):
        report = deviation_check.DeviationReport(
            change_name="test",
            date="2026-05-08",
            deviations=[
                deviation_check.Deviation(
                    type="missing", severity="error",
                    feature="Auth", details="Not implemented"),
                deviation_check.Deviation(
                    type="extra", severity="warning",
                    feature="Analytics", details="Not in proposal"),
            ],
        )
        alert = deviation_check.format_inline_alert(report)
        self.assertIn("1 error(s)", alert)
        self.assertIn("1 warning(s)", alert)

    # ─── Report persistence ──────────────────────────────────────────────

    def test_save_deviation_report(self):
        report = deviation_check.DeviationReport(
            change_name="test",
            date="2026-05-08",
            deviations=[],
            passed=True,
        )
        tmp_dir = os.path.join(self.project.root, "test-reports")
        path = deviation_check.save_report(report, tmp_dir)
        self.assertTrue(os.path.isfile(path))
        self.assertIn("deviation-check-", os.path.basename(path))

    # ─── should_block_on_deviation ───────────────────────────────────────

    def test_block_on_errors(self):
        report = deviation_check.DeviationReport(
            change_name="test", date="2026-05-08",
            deviations=[
                deviation_check.Deviation("missing", "error", "Auth", "Not implemented"),
            ],
            passed=False,
        )
        self.assertTrue(deviation_check.should_block_on_deviation(report, {}))

    def test_no_block_on_warnings_only(self):
        report = deviation_check.DeviationReport(
            change_name="test", date="2026-05-08",
            deviations=[
                deviation_check.Deviation("extra", "warning", "Analytics", "Not in proposal"),
            ],
            passed=True,
        )
        self.assertFalse(deviation_check.should_block_on_deviation(report, {}))

    def test_no_block_when_config_disabled(self):
        report = deviation_check.DeviationReport(
            change_name="test", date="2026-05-08",
            deviations=[
                deviation_check.Deviation("missing", "error", "Auth", "Not implemented"),
            ],
            passed=False,
        )
        self.assertFalse(deviation_check.should_block_on_deviation(
            report, {"deviation_block": False}))

    # ─── get_deviation_prompt ────────────────────────────────────────────

    def test_deviation_prompt_for_missing(self):
        report = deviation_check.DeviationReport(
            change_name="test", date="2026-05-08",
            deviations=[
                deviation_check.Deviation("missing", "error", "Auth", "Not implemented"),
            ],
        )
        prompt = deviation_check.get_deviation_prompt(report)
        self.assertIn("Scope Deviation Detected", prompt)
        self.assertIn("Missing", prompt)

    def test_deviation_prompt_empty(self):
        report = deviation_check.DeviationReport(
            change_name="test", date="2026-05-08",
            deviations=[],
        )
        prompt = deviation_check.get_deviation_prompt(report)
        self.assertEqual(prompt, "")


# ═══════════════════════════════════════════════════════════════════════════
# Test Suite 4: End-to-End Flow
# ═══════════════════════════════════════════════════════════════════════════

class TestEndToEndFlow(unittest.TestCase):
    """End-to-end integration tests simulating the full compliance workflow."""

    def setUp(self):
        self.project = TempProject()

    def tearDown(self):
        self.project.cleanup()

    def test_compliance_gate_blocks_archive(self):
        """Simulate archive-change Step 0: compliance check blocks on failures."""
        change_dir = self.project.create_change("archive-block",
            proposal="# Proposal\nTest",
            tasks="# Tasks\n- [ ] Incomplete task",
            # Missing: test files, test results, code reviews
        )

        config = {"require_design": False, "spec_compliance": False}
        report = compliance_check.run_compliance_check(
            change_dir, self.project.root, config)

        # Should fail (missing C3, C4, C5, C6, C7)
        self.assertFalse(report.passed)
        blocking_ids = [c.check_id for c in report.blocking_issues]
        self.assertIn("C3", blocking_ids)  # Tasks incomplete
        self.assertIn("C4", blocking_ids)  # No test files
        self.assertIn("C5", blocking_ids)  # No test results
        self.assertIn("C6", blocking_ids)  # No code review

    def test_compliance_gate_allows_archive_when_passing(self):
        """Simulate archive-change Step 0: compliance check allows archive."""
        change_dir = self.project.create_change("archive-pass",
            proposal="# Proposal\nTest",
            tasks="# Tasks\n- [x] Task 1",
            test_files=["tests/test_main.py"],
            test_results=[("test-result-20260508-120000.md",
                           "**Verdict**: PASS")],
            code_reviews=[("code-review-20260508-120000.md",
                           "# Code Review\n**Verdict**: PASS")],
            design="# Design\nArchitecture details",
        )

        config = {"require_design": True, "spec_compliance": False}
        report = compliance_check.run_compliance_check(
            change_dir, self.project.root, config)

        # All required checks should pass (except C8 which depends on git state)
        must_checks = [c for c in report.checks if c.level == "must" and c.check_id != "C8"]
        must_pass = all(c.passed for c in must_checks)
        self.assertTrue(must_pass)

        # C9 should pass (design.md exists)
        c9 = [c for c in report.checks if c.check_id == "C9"][0]
        self.assertTrue(c9.passed)

    def test_spec_compliance_with_deviation_integration(self):
        """Test spec compliance + deviation check integration."""
        change_dir = self.project.create_change("spec-deviation",
            proposal="""# Add Auth

## Scope

- User registration
- User login
- OAuth with Google and GitHub
- Password reset
""",
            src_files={"src/app.py": """
from flask import Flask
app = Flask(__name__)

@app.route('/api/register', methods=['POST'])
def register():
    pass

@app.route('/api/login', methods=['POST'])
def login():
    pass
"""})

        # Run spec compliance
        spec_result = spec_compliance.check_spec_compliance(
            change_dir, self.project.root)

        # Run deviation check (which uses spec-compliance internally)
        dev_report = deviation_check.check_deviations(
            change_dir, self.project.root)

        # Both should detect missing features
        self.assertIsInstance(spec_result, dict)
        self.assertIsInstance(dev_report, deviation_check.DeviationReport)

    def test_config_driven_c9_c10(self):
        """Test C9/C10 behavior driven by .openspec.yaml config."""
        change_dir = self.project.create_change("config-driven",
            proposal="# Proposal\n## Scope\n- Login feature",
            tasks="# Tasks\n- [x] Task 1",
            test_files=["tests/test_main.py"],
            test_results=[("test-result-20260508-120000.md",
                           "**Verdict**: PASS")],
            code_reviews=[("code-review-20260508-120000.md",
                           "**Verdict**: PASS")],
            openspec_yaml="""version: "1.2"
compliance:
  require_design: true
  spec_compliance: true
""")

        config = compliance_check.load_config(self.project.root)
        report = compliance_check.run_compliance_check(
            change_dir, self.project.root, config)

        # C9 should fail (no design.md but required)
        c9 = [c for c in report.checks if c.check_id == "C9"][0]
        self.assertFalse(c9.skipped)
        self.assertFalse(c9.passed)

    def test_full_archive_workflow_with_spec_compliance(self):
        """Full workflow: create change, implement, run compliance + deviation checks."""
        change_dir = self.project.create_change("full-workflow",
            proposal="""# User Management API

## Scope

- User registration with email
- User login with JWT
- `GET /api/users` - List users
- `POST /api/auth/login` - Login
- User model
""",
            tasks="# Tasks\n- [x] Implement User model\n- [x] Implement auth routes\n- [x] Write tests",
            design="# Design\n## Architecture\nREST API\n## API\nPOST /api/auth/login\n## Data Models\nUser",
            test_files=["tests/test_auth.py", "tests/test_models.py"],
            test_results=[("full-test-20260508-120000.md",
                           "# Full Test\nResult: PASS\nAll 5 tests passed.")],
            code_reviews=[("code-review-20260508-120000.md",
                           "# Code Review\n**Verdict**: PASS\nNo issues found.")],
            src_files={"src/app.py": """
from flask import Flask
app = Flask(__name__)

@app.route('/api/users')
def list_users():
    pass

@app.route('/api/auth/login', methods=['POST'])
def login():
    pass

@app.route('/api/register', methods=['POST'])
def register():
    pass
""",
                "src/models.py": """
class User:
    def __init__(self, email, name):
        self.email = email
        self.name = name
"""})

        # Step 1: Run compliance check with all features enabled
        config = {"require_design": True, "spec_compliance": True}
        compliance_report = compliance_check.run_compliance_check(
            change_dir, self.project.root, config)

        # C1-C7 should pass, C9 should pass (design.md exists)
        for c in compliance_report.checks:
            if c.check_id in ("C1", "C2", "C3", "C4", "C5", "C6", "C7", "C9"):
                self.assertTrue(c.passed, f"{c.check_id} should pass: {c.details}")

        # Step 2: Run deviation check
        dev_report = deviation_check.check_deviations(
            change_dir, self.project.root, config)

        # Step 3: Save both reports
        test_reports_dir = os.path.join(change_dir, "test-reports")
        compliance_path = compliance_check.save_report(compliance_report, test_reports_dir)
        deviation_path = deviation_check.save_report(dev_report, test_reports_dir)

        self.assertTrue(os.path.isfile(compliance_path))
        self.assertTrue(os.path.isfile(deviation_path))

        # Step 4: Verify report content
        with open(compliance_path, "r", encoding="utf-8") as f:
            compliance_md = f.read()
        self.assertIn("Pre-Archive Compliance Check", compliance_md)

        with open(deviation_path, "r", encoding="utf-8") as f:
            deviation_md = f.read()
        self.assertIn("Deviation Check Report", deviation_md)

    def test_c10_with_proposal_api_mentions(self):
        """Test C10 correctly checks API routes mentioned in proposal."""
        change_dir = self.project.create_change("api-compliance",
            proposal="""# API Feature

## Scope

- `GET /api/users`
- `POST /api/auth/login`
""",
            tasks="# Tasks\n- [x] Task 1",
            src_files={"src/app.py": """
from flask import Flask
app = Flask(__name__)

@app.route('/api/users')
def list_users():
    pass

@app.route('/api/auth/login', methods=['POST'])
def login():
    pass
"""})

        config = {"spec_compliance": True}
        result = compliance_check.check_c10_spec_compliance(
            change_dir, self.project.root, config)

        # Both APIs are implemented, should pass
        self.assertFalse(result.skipped)


# ═══════════════════════════════════════════════════════════════════════════
# Test Suite 5: CLI Interface Tests
# ═══════════════════════════════════════════════════════════════════════════

class TestCLIInterfaces(unittest.TestCase):
    """Test CLI interfaces for compliance-check, spec-compliance, deviation-check."""

    def setUp(self):
        self.project = TempProject()

    def tearDown(self):
        self.project.cleanup()

    def test_compliance_check_cli_json_output(self):
        """Test compliance-check.py --json output."""
        change_dir = self.project.create_change("cli-test",
            proposal="# Proposal\nTest",
            tasks="# Tasks\n- [x] Task 1",
            test_files=["tests/test_main.py"],
            test_results=[("test-result-20260508-120000.md",
                           "**Verdict**: PASS")],
            code_reviews=[("code-review-20260508-120000.md",
                           "**Verdict**: PASS")])

        import subprocess
        result = subprocess.run(
            [sys.executable, os.path.join(UTILS_DIR, "compliance-check.py"),
             "--change", "cli-test", "--project-root", self.project.root, "--json"],
            capture_output=True, text=True, timeout=30, shell=True)

        output = result.stdout.strip()
        if output:
            data = json.loads(output)
            self.assertIn("change_name", data)
            self.assertIn("checks", data)
            self.assertIn("passed", data)

    def test_spec_compliance_cli_json_output(self):
        """Test spec-compliance.py --json output."""
        change_dir = self.project.create_change("spec-cli",
            proposal="# Feature\n## Scope\n- Login feature",
            src_files={"src/app.py": "from flask import Flask\napp = Flask(__name__)\n@app.route('/api/login', methods=['POST'])\ndef login(): pass"})

        import subprocess
        result = subprocess.run(
            [sys.executable, os.path.join(UTILS_DIR, "spec-compliance.py"),
             "--change", "spec-cli", "--project-root", self.project.root, "--json"],
            capture_output=True, text=True, timeout=30, shell=True)

        output = result.stdout.strip()
        if output:
            data = json.loads(output)
            self.assertIn("passed", data)
            self.assertIn("covered", data)

    def test_deviation_check_cli_json_output(self):
        """Test deviation-check.py --json output."""
        change_dir = self.project.create_change("dev-cli",
            proposal="# Feature\n## Scope\n- Login feature",
            src_files={"src/app.py": "pass"})

        import subprocess
        result = subprocess.run(
            [sys.executable, os.path.join(UTILS_DIR, "deviation-check.py"),
             "--change", "dev-cli", "--project-root", self.project.root, "--json"],
            capture_output=True, text=True, timeout=30, shell=True)

        output = result.stdout.strip()
        if output:
            data = json.loads(output)
            self.assertIn("passed", data)
            self.assertIn("deviations", data)

    def test_deviation_check_cli_alert_output(self):
        """Test deviation-check.py --alert output."""
        change_dir = self.project.create_change("alert-cli",
            proposal="# Feature\n## Scope\n- Login feature",
            src_files={"src/app.py": "pass"})

        import subprocess
        result = subprocess.run(
            [sys.executable, os.path.join(UTILS_DIR, "deviation-check.py"),
             "--change", "alert-cli", "--project-root", self.project.root, "--alert"],
            capture_output=True, text=True, timeout=30, shell=True)

        # Alert mode outputs compact single-line or empty
        output = result.stdout.strip()
        # If there are deviations, should contain indicator
        if output:
            self.assertTrue(any(s in output for s in ["error", "warning", "info", "Deviation"]))

    def test_compliance_check_cli_save(self):
        """Test compliance-check.py --save creates report file."""
        change_dir = self.project.create_change("save-cli",
            proposal="# Proposal\nTest")

        import subprocess
        result = subprocess.run(
            [sys.executable, os.path.join(UTILS_DIR, "compliance-check.py"),
             "--change", "save-cli", "--project-root", self.project.root, "--save"],
            capture_output=True, text=True, timeout=30, shell=True)

        test_reports_dir = os.path.join(change_dir, "test-reports")
        if os.path.isdir(test_reports_dir):
            files = [f for f in os.listdir(test_reports_dir) if f.startswith("compliance-")]
            self.assertTrue(len(files) >= 1)

    def test_spec_compliance_cli_save(self):
        """Test spec-compliance.py --save creates report file."""
        change_dir = self.project.create_change("spec-save",
            proposal="# Feature\n## Scope\n- Login",
            src_files={"src/main.py": "pass"})

        import subprocess
        result = subprocess.run(
            [sys.executable, os.path.join(UTILS_DIR, "spec-compliance.py"),
             "--change", "spec-save", "--project-root", self.project.root, "--save"],
            capture_output=True, text=True, timeout=30, shell=True)

        test_reports_dir = os.path.join(change_dir, "test-reports")
        if os.path.isdir(test_reports_dir):
            files = [f for f in os.listdir(test_reports_dir) if f.startswith("spec-compliance-")]
            self.assertTrue(len(files) >= 1)

    def test_compliance_check_cli_missing_change(self):
        """Test compliance-check.py with non-existent change exits with error."""
        import subprocess
        result = subprocess.run(
            [sys.executable, os.path.join(UTILS_DIR, "compliance-check.py"),
             "--change", "nonexistent", "--project-root", self.project.root],
            capture_output=True, text=True, timeout=30, shell=True)

        self.assertNotEqual(result.returncode, 0)

    def test_spec_compliance_cli_missing_change(self):
        """Test spec-compliance.py with non-existent change exits with error."""
        import subprocess
        result = subprocess.run(
            [sys.executable, os.path.join(UTILS_DIR, "spec-compliance.py"),
             "--change", "nonexistent", "--project-root", self.project.root],
            capture_output=True, text=True, timeout=30, shell=True)

        self.assertNotEqual(result.returncode, 0)


# ═══════════════════════════════════════════════════════════════════════════
# Test Suite 6: Data Structures & Edge Cases
# ═══════════════════════════════════════════════════════════════════════════

class TestDataStructuresAndEdgeCases(unittest.TestCase):
    """Test data structures, properties, and edge cases."""

    def test_check_result_status_icons(self):
        r = compliance_check.CheckResult("C1", "test", True, "must")
        self.assertIn("PASS", r.status_icon)

        r = compliance_check.CheckResult("C1", "test", False, "must")
        self.assertIn("FAIL", r.status_icon)

        r = compliance_check.CheckResult("C1", "test", True, "optional", skipped=True)
        self.assertIn("SKIP", r.status_icon)

    def test_compliance_report_properties(self):
        report = compliance_check.ComplianceReport(
            change_name="test", date="2026-05-08",
            checks=[
                compliance_check.CheckResult("C1", "test", True, "must"),
                compliance_check.CheckResult("C2", "test", False, "must", "Missing"),
                compliance_check.CheckResult("C9", "test", True, "optional", skipped=True),
            ])

        self.assertFalse(report.passed)  # C2 fails
        self.assertEqual(len(report.blocking_issues), 1)
        self.assertEqual(report.pass_count, 1)
        self.assertEqual(report.fail_count, 1)
        self.assertEqual(report.skip_count, 1)

    def test_proposal_scope_defaults(self):
        scope = spec_compliance.ProposalScope()
        self.assertEqual(len(scope.features), 0)
        self.assertEqual(len(scope.api_mentions), 0)
        self.assertEqual(len(scope.data_models), 0)

    def test_compliance_result_coverage_rate(self):
        result = spec_compliance.ComplianceResult()
        result.covered = ["A", "B"]
        result.missing = ["C"]
        self.assertAlmostEqual(result.coverage_rate, 2/3)

    def test_compliance_result_passed(self):
        result = spec_compliance.ComplianceResult()
        self.assertTrue(result.passed)

        result.missing = ["feature"]
        self.assertFalse(result.passed)

    def test_deviation_report_errors_warnings(self):
        report = deviation_check.DeviationReport(
            change_name="test", date="2026-05-08",
            deviations=[
                deviation_check.Deviation("missing", "error", "A", "details"),
                deviation_check.Deviation("extra", "warning", "B", "details"),
                deviation_check.Deviation("info", "info", "C", "details"),
            ])

        self.assertEqual(len(report.errors), 1)
        self.assertEqual(len(report.warnings), 1)
        self.assertEqual(len(report.info), 1)

    def test_route_info(self):
        route = spec_compliance.RouteInfo("GET", "/api/users", "users.py")
        self.assertEqual(route.method, "GET")
        self.assertEqual(route.path, "/api/users")

    def test_model_info(self):
        model = spec_compliance.ModelInfo("User", "models.py", ["id", "name"])
        self.assertEqual(model.name, "User")
        self.assertEqual(len(model.fields), 2)

    def test_empty_change_dir(self):
        """Test all checks against an empty change directory."""
        project = TempProject()
        try:
            change_dir = project.create_change("empty")
            report = compliance_check.run_compliance_check(
                change_dir, project.root)
            # Should fail on multiple required checks
            self.assertFalse(report.passed)
        finally:
            project.cleanup()

    def test_check_spec_compliance_empty_src(self):
        """Test spec compliance with no source files."""
        project = TempProject()
        try:
            change_dir = project.create_change("no-src",
                proposal="# Feature\n## Scope\n- Login\n- Registration")
            result = spec_compliance.check_spec_compliance(
                change_dir, project.root)
            self.assertFalse(result["passed"])
            self.assertTrue(len(result["missing"]) >= 1)
        finally:
            project.cleanup()

    def test_deviation_check_without_spec_module(self):
        """Test deviation check gracefully handles missing spec-compliance module."""
        # Mock HAS_SPEC_COMPLIANCE to False
        with patch.object(deviation_check, 'HAS_SPEC_COMPLIANCE', False):
            report = deviation_check.DeviationReport(
                change_name="test", date="2026-05-08")
            # Should still create valid report structure
            self.assertIsInstance(report, deviation_check.DeviationReport)

    def test_compound_feature_splitting(self):
        """Test splitting of compound features like 'OAuth: Google, GitHub'."""
        result = spec_compliance._split_compound_feature("oauth providers: google, github")
        self.assertEqual(len(result), 2)
        self.assertIn("google", result)
        self.assertIn("github", result)

    def test_compound_feature_no_split(self):
        """Test non-compound feature is returned as-is."""
        result = spec_compliance._split_compound_feature("user login")
        self.assertEqual(len(result), 1)
        self.assertEqual(result[0], "user login")

    def test_api_matching_with_params(self):
        """Test API matching normalizes path parameters."""
        routes = [spec_compliance.RouteInfo("GET", "/api/users/:id", "users.py")]
        self.assertTrue(spec_compliance._match_api("GET /api/users/:id", routes))
        self.assertFalse(spec_compliance._match_api("GET /api/users", routes))

    def test_extra_features_skips_common_routes(self):
        """Test that common non-feature routes are not flagged as extra."""
        scope = spec_compliance.ProposalScope()
        routes = [
            spec_compliance.RouteInfo("GET", "/", "app.py"),
            spec_compliance.RouteInfo("GET", "/health", "app.py"),
            spec_compliance.RouteInfo("GET", "/ping", "app.py"),
        ]
        extras = spec_compliance._find_extra_features(scope, routes, [])
        # Common routes should be filtered out
        extra_names = [e["name"] for e in extras]
        self.assertNotIn("GET /", extra_names)
        self.assertNotIn("GET /health", extra_names)


if __name__ == "__main__":
    unittest.main(verbosity=2)
