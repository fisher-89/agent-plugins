#!/usr/bin/env python3
"""
Spec compliance analysis utility.

Compares proposal.md Scope with actual implementation to detect:
- Missing: features mentioned in proposal but not implemented
- Extra: features implemented but not mentioned in proposal
- Covered: features both mentioned and implemented

Used by C10 compliance check.
"""

import glob
import json
import os
import re
import sys
from datetime import datetime
from typing import List, Dict, Optional, Tuple
from dataclasses import dataclass, field


@dataclass
class ProposalScope:
    """Parsed scope from proposal.md."""
    features: List[str] = field(default_factory=list)
    api_mentions: List[str] = field(default_factory=list)
    data_models: List[str] = field(default_factory=list)


@dataclass
class RouteInfo:
    """Extracted API route information."""
    method: str
    path: str
    file: str


@dataclass
class ModelInfo:
    """Extracted data model information."""
    name: str
    file: str
    fields: List[str] = field(default_factory=list)


@dataclass
class ComplianceResult:
    """Result of spec compliance comparison."""
    covered: List[str] = field(default_factory=list)
    missing: List[str] = field(default_factory=list)
    extra: List[str] = field(default_factory=list)
    covered_details: List[Dict] = field(default_factory=list)
    missing_details: List[Dict] = field(default_factory=list)
    extra_details: List[Dict] = field(default_factory=list)

    @property
    def passed(self) -> bool:
        return len(self.missing) == 0

    @property
    def coverage_rate(self) -> float:
        total = len(self.covered) + len(self.missing)
        if total == 0:
            return 1.0
        return len(self.covered) / total


# ─── Proposal scope parsing ────────────────────────────────────────────

def parse_proposal_scope(proposal_path: str) -> ProposalScope:
    """Parse proposal.md and extract the Scope section.

    Args:
        proposal_path: Path to proposal.md file.

    Returns:
        ProposalScope with extracted features, API mentions, and data models.
    """
    scope = ProposalScope()

    if not os.path.isfile(proposal_path):
        return scope

    try:
        with open(proposal_path, "r", encoding="utf-8") as f:
            content = f.read()
    except OSError:
        return scope

    # Find the Scope section
    scope_content = _extract_section(content, "Scope")

    if not scope_content:
        # Fallback: use the entire content for feature extraction
        scope_content = content

    # Extract bullet-point features
    scope.features = _extract_bullet_features(scope_content)

    # Extract API mentions (HTTP methods + paths)
    scope.api_mentions = _extract_api_mentions(scope_content)

    # Extract data model mentions
    scope.data_models = _extract_model_mentions(scope_content)

    return scope


def _extract_section(content: str, section_title: str) -> str:
    """Extract a markdown section by title.

    Handles both ## Title and ### Title formats.
    Returns the section content up to the next same-level heading.
    """
    lines = content.splitlines()
    section_lines = []
    in_section = False
    section_level = 0

    for line in lines:
        # Check if this is a heading
        heading_match = re.match(r'^(#{1,6})\s+(.+)$', line)
        if heading_match:
            level = len(heading_match.group(1))
            title = heading_match.group(2).strip()

            if not in_section and level >= 2 and title.lower().startswith(section_title.lower()):
                in_section = True
                section_level = level
                continue

            if in_section and level <= section_level:
                # Next same-level or higher heading — end of section
                break
        elif in_section:
            section_lines.append(line)

    return "\n".join(section_lines)


def _extract_bullet_features(text: str) -> List[str]:
    """Extract bullet-point features from text.

    Supports -, *, and + bullet markers, as well as numbered lists.
    """
    features = []

    for line in text.splitlines():
        line = line.strip()

        # Bullet list: - feature or * feature
        bullet_match = re.match(r"^[-*+]\s+(.+)$", line)
        if bullet_match:
            feature = bullet_match.group(1).strip()
            if feature and len(feature) > 2:
                features.append(feature)
            continue

        # Numbered list: 1. feature
        num_match = re.match(r"^\d+\.\s+(.+)$", line)
        if num_match:
            feature = num_match.group(1).strip()
            if feature and len(feature) > 2:
                features.append(feature)

    return features


def _extract_api_mentions(text: str) -> List[str]:
    """Extract API mentions from proposal text.

    Looks for HTTP method + path patterns like:
    - GET /api/users
    - POST /auth/login
    - `PUT /users/:id`
    """
    apis = []

    # Match HTTP method + path patterns
    pattern = r"`?(GET|POST|PUT|DELETE|PATCH|HEAD|OPTIONS)\s+(/[^\s`,\)]+)`?"
    matches = re.findall(pattern, text, re.IGNORECASE)

    for method, path in matches:
        api = f"{method.upper()} {path}"
        if api not in apis:
            apis.append(api)

    return apis


def _extract_model_mentions(text: str) -> List[str]:
    """Extract data model/entity mentions from proposal text.

    Looks for patterns like:
    - User model
    - users table
    - `User` entity
    """
    models = []

    # Match "X model", "X entity", "X table", "X schema"
    pattern = r"`?(\b[A-Z][a-zA-Z]+)\s+(?:model|entity|table|schema)\b`?"
    matches = re.findall(pattern, text)

    for m in matches:
        if m not in models and m not in ("The", "This", "Each", "These", "When", "After", "Before"):
            models.append(m)

    # Match "model/table/schema: X" or "model/table/schema named X"
    pattern2 = r"(?:model|table|schema)(?:\s+named|\s*:)\s+`?(\w+)`?"
    matches2 = re.findall(pattern2, text, re.IGNORECASE)

    for m in matches2:
        if m not in models:
            models.append(m)

    return models


# ─── Code scanning: route extraction ───────────────────────────────────

def extract_routes(src_dir: str) -> List[RouteInfo]:
    """Extract API route definitions from source code.

    Supports:
    - Express.js: router.get('/path', ...), app.post('/path', ...)
    - FastAPI/Flask: @app.get('/path'), @router.post('/path')
    - Django: path('route/', view)

    Args:
        src_dir: Path to the source directory.

    Returns:
        List of RouteInfo objects.
    """
    routes = []

    if not os.path.isdir(src_dir):
        return routes

    # Scan JS/TS files
    for filepath in _find_source_files(src_dir, {".js", ".ts", ".jsx", ".tsx"}):
        routes.extend(_extract_express_routes(filepath))

    # Scan Python files
    for filepath in _find_source_files(src_dir, {".py"}):
        routes.extend(_extract_python_routes(filepath))

    return routes


def _find_source_files(src_dir: str, extensions: set) -> List[str]:
    """Find source files with given extensions."""
    files = []
    for ext in extensions:
        pattern = os.path.join(src_dir, "**", f"*{ext}")
        files.extend(glob.glob(pattern, recursive=True))
    return files


def _extract_express_routes(filepath: str) -> List[RouteInfo]:
    """Extract routes from Express/Node.js files."""
    routes = []

    try:
        with open(filepath, "r", encoding="utf-8") as f:
            content = f.read()
    except OSError:
        return routes

    # Express: router.get('/path', ...), app.post('/path', ...)
    pattern = r"(?:router|app|Router\(\))\.(get|post|put|delete|patch)\s*\(\s*['\"]([^'\"]+)['\"]"
    matches = re.findall(pattern, content)

    for method, path in matches:
        routes.append(RouteInfo(
            method=method.upper(),
            path=path,
            file=filepath,
        ))

    # Fastify: fastify.get('/path', ...)
    pattern2 = r"(?:fastify|server)\.(get|post|put|delete|patch)\s*\(\s*['\"]([^'\"]+)['\"]"
    matches2 = re.findall(pattern2, content)

    for method, path in matches2:
        routes.append(RouteInfo(
            method=method.upper(),
            path=path,
            file=filepath,
        ))

    return routes


def _extract_python_routes(filepath: str) -> List[RouteInfo]:
    """Extract routes from Python web framework files."""
    routes = []

    try:
        with open(filepath, "r", encoding="utf-8") as f:
            content = f.read()
    except OSError:
        return routes

    # Flask: @app.route('/path', methods=['GET'])
    flask_pattern = r"@(?:app|bp|blueprint)\.route\s*\(\s*[\'\"]([^\'\"]+)[\'\"]\s*(?:,\s*methods\s*=\s*\[([^\]]*)\])?"
    flask_matches = re.findall(flask_pattern, content)

    for path, methods_str in flask_matches:
        if methods_str:
            methods = re.findall(r'[\'"](\w+)[\'"]', methods_str)
            for method in methods:
                routes.append(RouteInfo(method=method.upper(), path=path, file=filepath))
        else:
            routes.append(RouteInfo(method="GET", path=path, file=filepath))

    # FastAPI: @router.get('/path'), @app.post('/path')
    fastapi_pattern = r"@(?:router|app)\.(get|post|put|delete|patch)\s*\(\s*['\"]([^'\"]+)['\"]"
    fastapi_matches = re.findall(fastapi_pattern, content, re.IGNORECASE)

    for method, path in fastapi_matches:
        routes.append(RouteInfo(method=method.upper(), path=path, file=filepath))

    # Django: path('route/', view)
    django_pattern = r"(?:path|re_path)\s*\(\s*['\"]([^'\"]+)['\"]"
    django_matches = re.findall(django_pattern, content)

    for path in django_matches:
        routes.append(RouteInfo(method="ANY", path=path, file=filepath))

    return routes


# ─── Code scanning: model extraction ───────────────────────────────────

def extract_models(src_dir: str) -> List[ModelInfo]:
    """Extract data model definitions from source code.

    Supports:
    - SQL: CREATE TABLE name
    - SQLAlchemy: class Name(Base)
    - Django: class Name(models.Model)
    - Mongoose/TypeORM: const Name = mongoose.model()
    - Prisma: model Name

    Args:
        src_dir: Path to the source directory.

    Returns:
        List of ModelInfo objects.
    """
    models = []

    if not os.path.isdir(src_dir):
        return models

    # SQL schema files
    for filepath in _find_source_files(src_dir, {".sql"}):
        models.extend(_extract_sql_models(filepath))

    # Python models
    for filepath in _find_source_files(src_dir, {".py"}):
        models.extend(_extract_python_models(filepath))

    # JS/TS models
    for filepath in _find_source_files(src_dir, {".js", ".ts"}):
        models.extend(_extract_js_models(filepath))

    # Prisma schema
    for filepath in _find_source_files(src_dir, {".prisma"}):
        models.extend(_extract_prisma_models(filepath))

    return models


def _extract_sql_models(filepath: str) -> List[ModelInfo]:
    """Extract table names from SQL schema files."""
    models = []

    try:
        with open(filepath, "r", encoding="utf-8") as f:
            content = f.read()
    except OSError:
        return models

    # CREATE TABLE name
    pattern = r"CREATE\s+TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?[`\"\[]?(\w+)[`\"\]]?"
    matches = re.findall(pattern, content, re.IGNORECASE)

    for name in matches:
        models.append(ModelInfo(name=name, file=filepath))

    return models


def _extract_python_models(filepath: str) -> List[ModelInfo]:
    """Extract model classes from Python files."""
    models = []

    try:
        with open(filepath, "r", encoding="utf-8") as f:
            content = f.read()
    except OSError:
        return models

    # SQLAlchemy: class Name(Base)
    pattern1 = r"class\s+(\w+)\s*\(\s*\w*Base\w*\s*\)"
    matches1 = re.findall(pattern1, content)

    # Django: class Name(models.Model)
    pattern2 = r"class\s+(\w+)\s*\(\s*models\.Model\s*\)"
    matches2 = re.findall(pattern2, content)

    # Django: class Name(models.AbstractUser) etc.
    pattern3 = r"class\s+(\w+)\s*\(\s*models\.\w+\s*\)"
    matches3 = re.findall(pattern3, content)

    seen = set()
    for name in matches1 + matches2 + matches3:
        if name not in seen:
            seen.add(name)
            models.append(ModelInfo(name=name, file=filepath))

    return models


def _extract_js_models(filepath: str) -> List[ModelInfo]:
    """Extract model definitions from JS/TS files."""
    models = []

    try:
        with open(filepath, "r", encoding="utf-8") as f:
            content = f.read()
    except OSError:
        return models

    # Mongoose: mongoose.model('Name', schema) or mongoose.model("Name", schema)
    pattern1 = r"(?:mongoose\.model|model)\s*\(\s*['\"](\w+)['\"]\s*[,)]"
    matches1 = re.findall(pattern1, content)

    # TypeORM: @Entity() class Name
    pattern2 = r"@Entity\s*\(\s*(?:['\"](\w+)['\"]\s*)?\)\s*\n\s*export\s+class\s+(\w+)"
    matches2 = re.findall(pattern2, content)

    # Sequelize: sequelize.define('Name', ...)
    pattern3 = r"(?:sequelize|Sequelize)\.define\s*\(\s*['\"](\w+)['\"]\s*[,)]"
    matches3 = re.findall(pattern3, content)

    seen = set()
    for name in matches1 + matches3:
        if name not in seen:
            seen.add(name)
            models.append(ModelInfo(name=name, file=filepath))

    for custom_name, class_name in matches2:
        display_name = custom_name if custom_name else class_name
        if display_name not in seen:
            seen.add(display_name)
            models.append(ModelInfo(name=display_name, file=filepath))

    return models


def _extract_prisma_models(filepath: str) -> List[ModelInfo]:
    """Extract model definitions from Prisma schema files."""
    models = []

    try:
        with open(filepath, "r", encoding="utf-8") as f:
            content = f.read()
    except OSError:
        return models

    # model Name {
    pattern = r"^model\s+(\w+)\s*\{"
    matches = re.findall(pattern, content, re.MULTILINE)

    for name in matches:
        models.append(ModelInfo(name=name, file=filepath))

    return models


# ─── Comparison algorithm ───────────────────────────────────────────────

def compare_spec_impl(
    proposal_scope: ProposalScope,
    impl_routes: List[RouteInfo],
    impl_models: List[ModelInfo],
) -> ComplianceResult:
    """Compare proposal scope with implementation.

    Uses keyword-based matching to map proposal features to implementation artifacts.

    Args:
        proposal_scope: Parsed scope from proposal.md.
        impl_routes: Extracted routes from source code.
        impl_models: Extracted models from source code.

    Returns:
        ComplianceResult with covered, missing, and extra features.
    """
    result = ComplianceResult()

    if not proposal_scope.features and not proposal_scope.api_mentions:
        # No scope to compare against — pass by default
        return result

    # Build implementation keyword inventory
    impl_keywords = _build_impl_keywords(impl_routes, impl_models)

    # Check each proposal feature against implementation
    for feature in proposal_scope.features:
        matched = _match_feature(feature, impl_keywords, impl_routes, impl_models)

        if matched["matched"]:
            result.covered.append(feature)
            result.covered_details.append({
                "feature": feature,
                "evidence": matched["evidence"],
            })
        else:
            result.missing.append(feature)
            result.missing_details.append({
                "feature": feature,
                "reason": matched.get("reason", "No matching implementation found"),
            })

    # Check proposal API mentions
    for api in proposal_scope.api_mentions:
        if not _match_api(api, impl_routes):
            # API mentioned in proposal but not found in implementation
            if api not in result.missing:
                result.missing.append(api)
                result.missing_details.append({
                    "feature": api,
                    "reason": "API route not found in implementation",
                })

    # Find extra features (implemented but not in proposal)
    extra_items = _find_extra_features(proposal_scope, impl_routes, impl_models)
    for item in extra_items:
        result.extra.append(item["name"])
        result.extra_details.append(item)

    return result


def _build_impl_keywords(routes: List[RouteInfo], models: List[ModelInfo]) -> Dict[str, List[str]]:
    """Build keyword map from implementation artifacts.

    Returns:
        Dict mapping normalized keywords to source artifacts.
    """
    keywords: Dict[str, List[str]] = {}

    # Keyword normalization map
    keyword_map = {
        "register": "registration",
        "signup": "registration",
        "sign-up": "registration",
        "signin": "login",
        "sign-in": "login",
        "log-in": "login",
        "log-in": "login",
        "auth": "authentication",
        "oauth": "oauth",
        "google": "google",
        "github": "github",
        "facebook": "facebook",
        "jwt": "jwt",
        "token": "token",
        "session": "session",
        "cache": "caching",
        "redis": "redis",
        "rbac": "rbac",
        "role": "role",
        "permission": "permission",
        "crud": "crud",
        "create": "create",
        "read": "read",
        "update": "update",
        "delete": "delete",
        "list": "list",
        "search": "search",
        "filter": "filter",
        "sort": "sort",
        "paginate": "pagination",
        "pagination": "pagination",
        "upload": "upload",
        "download": "download",
        "export": "export",
        "import": "import",
        "notify": "notification",
        "notification": "notification",
        "email": "email",
        "webhook": "webhook",
        "cron": "cron",
        "schedule": "schedule",
        "queue": "queue",
        "worker": "worker",
    }

    for route in routes:
        path_lower = route.path.lower()
        method_lower = route.method.lower()

        # Extract keywords from path segments
        segments = [s for s in path_lower.split("/") if s and not s.startswith(":") and not s.startswith("{")]

        for segment in segments:
            normalized = keyword_map.get(segment, segment)
            if normalized not in keywords:
                keywords[normalized] = []
            keywords[normalized].append(f"{route.method} {route.path} ({route.file})")

        # Also match on method + path combination
        combined = f"{method_lower} {path_lower}"
        for key, norm in keyword_map.items():
            if key in combined and norm not in keywords:
                keywords[norm] = []
                keywords[norm].append(f"{route.method} {route.path}")

    for model in models:
        name_lower = model.name.lower()
        normalized = keyword_map.get(name_lower, name_lower)
        if normalized not in keywords:
            keywords[normalized] = []
        keywords[normalized].append(f"Model: {model.name} ({model.file})")

    return keywords


def _match_feature(
    feature: str,
    impl_keywords: Dict[str, List[str]],
    impl_routes: List[RouteInfo],
    impl_models: List[ModelInfo],
) -> Dict:
    """Match a proposal feature against implementation.

    Returns:
        Dict with "matched" bool and "evidence" or "reason".
    """
    feature_lower = feature.lower()
    evidence = []

    # Strategy 1: Direct keyword matching
    keyword_map = {
        "register": "registration",
        "signup": "registration",
        "sign-up": "registration",
        "login": "login",
        "signin": "login",
        "sign-in": "login",
        "logout": "logout",
        "auth": "authentication",
        "oauth": "oauth",
        "google": "google",
        "github": "github",
        "jwt": "jwt",
        "token": "token",
        "session": "session",
        "cache": "caching",
        "redis": "redis",
        "rbac": "rbac",
        "role": "role",
        "permission": "permission",
        "crud": "crud",
        "notification": "notification",
        "email": "email",
        "webhook": "webhook",
        "queue": "queue",
        "worker": "worker",
        "upload": "upload",
        "download": "download",
        "export": "export",
        "import": "import",
        "search": "search",
        "filter": "filter",
        "pagination": "pagination",
        "schedule": "schedule",
        "cron": "cron",
    }

    # Check each keyword against feature text
    matched_keywords = set()
    for key, normalized in keyword_map.items():
        if key in feature_lower:
            matched_keywords.add(normalized)
            if normalized in impl_keywords:
                evidence.extend(impl_keywords[normalized])

    # Strategy 2: Route path matching
    for route in impl_routes:
        path_lower = route.path.lower()
        # Check if route path relates to feature keywords
        feature_words = re.findall(r"[a-z]+", feature_lower)
        for word in feature_words:
            if len(word) >= 3 and word in path_lower:
                evidence.append(f"{route.method} {route.path} ({route.file})")
                break

    # Strategy 3: Model name matching
    for model in impl_models:
        model_lower = model.name.lower()
        if model_lower in feature_lower or any(
            w in model_lower for w in re.findall(r"[a-z]{3,}", feature_lower)
        ):
            evidence.append(f"Model: {model.name} ({model.file})")

    # Deduplicate evidence
    seen = set()
    unique_evidence = []
    for e in evidence:
        if e not in seen:
            seen.add(e)
            unique_evidence.append(e)

    # Determine match: feature is covered if we found evidence
    # For compound features (e.g., "OAuth providers: Google, GitHub"),
    # partial coverage is acceptable
    if unique_evidence:
        # Check if ALL sub-features are covered for compound features
        sub_features = _split_compound_feature(feature_lower)
        if len(sub_features) > 1:
            all_covered = True
            for sub in sub_features:
                sub_found = False
                sub_norm = keyword_map.get(sub.strip(), sub.strip())
                if sub_norm in impl_keywords:
                    sub_found = True
                else:
                    # Check route/model evidence for sub-feature
                    for e in unique_evidence:
                        if sub in e.lower():
                            sub_found = True
                            break
                if not sub_found:
                    all_covered = False
                    break

            if all_covered:
                return {"matched": True, "evidence": unique_evidence}
            else:
                return {
                    "matched": False,
                    "evidence": unique_evidence,
                    "reason": "Partially covered — some sub-features missing",
                }

        return {"matched": True, "evidence": unique_evidence}

    return {"matched": False, "reason": "No matching implementation found"}


def _split_compound_feature(feature_lower: str) -> List[str]:
    """Split compound features like 'OAuth providers: Google, GitHub'.

    Returns:
        List of individual sub-feature keywords.
    """
    # Pattern: "X: A, B, C" or "X - A, B, C"
    match = re.match(r"^(.+?)\s*[:\-]\s*(.+)$", feature_lower)
    if match:
        items = re.split(r"[,;/]|\band\b", match.group(2))
        return [item.strip() for item in items if item.strip()]

    return [feature_lower]


def _match_api(api_mention: str, impl_routes: List[RouteInfo]) -> bool:
    """Check if a specific API mention exists in implementation."""
    # Parse the API mention: "GET /path"
    match = re.match(r"(GET|POST|PUT|DELETE|PATCH|HEAD|OPTIONS)\s+(.+)", api_mention, re.IGNORECASE)
    if not match:
        return False

    method = match.group(1).upper()
    path = match.group(2).rstrip("/")

    for route in impl_routes:
        route_path = route.path.rstrip("/")
        if route.method == method and route_path == path:
            return True
        # Also check with parameter normalization
        normalized_route = re.sub(r"/:[\w]+", "/{param}", route_path)
        normalized_proposal = re.sub(r"/:[\w]+", "/{param}", path)
        if route.method == method and normalized_route == normalized_proposal:
            return True

    return False


def _find_extra_features(
    proposal_scope: ProposalScope,
    impl_routes: List[RouteInfo],
    impl_models: List[ModelInfo],
) -> List[Dict]:
    """Find features in implementation that are not mentioned in proposal."""
    extra = []

    # Build set of proposal keywords for comparison
    proposal_text = " ".join(proposal_scope.features).lower()
    proposal_api_paths = set()
    for api in proposal_scope.api_mentions:
        match = re.match(r"\w+\s+(.+)", api)
        if match:
            proposal_api_paths.add(match.group(1).rstrip("/").lower())

    # Check routes not in proposal
    for route in impl_routes:
        route_path = route.path.rstrip("/").lower()

        # Skip common non-feature routes
        if route_path in ("/", "/health", "/ping", "/status", "/favicon.ico"):
            continue

        # Check if this route was mentioned in proposal
        in_proposal = False

        # Direct path match
        if route_path in proposal_api_paths:
            in_proposal = True

        # Keyword match with proposal text
        path_segments = [s for s in route_path.split("/") if s and not s.startswith(":") and not s.startswith("{")]
        for segment in path_segments:
            if segment in proposal_text:
                in_proposal = True
                break

        if not in_proposal:
            extra.append({
                "name": f"{route.method} {route.path}",
                "type": "route",
                "file": route.file,
            })

    # Check models not in proposal
    for model in impl_models:
        model_lower = model.name.lower()
        if model_lower not in proposal_text and model_lower not in [
            m.lower() for m in proposal_scope.data_models
        ]:
            extra.append({
                "name": f"Model: {model.name}",
                "type": "model",
                "file": model.file,
            })

    return extra


# ─── Main entry point ──────────────────────────────────────────────────

def check_spec_compliance(change_dir: str, project_root: str) -> Dict:
    """Run spec compliance analysis.

    This is the main entry point called by compliance-check.py (C10).

    Args:
        change_dir: Path to the change directory (e.g., openspec/changes/add-auth/).
        project_root: Path to the project root.

    Returns:
        Dict with keys: passed, covered, missing, extra, covered_details,
        missing_details, extra_details, routes, models.
    """
    # Parse proposal scope
    proposal_path = os.path.join(change_dir, "proposal.md")
    proposal_scope = parse_proposal_scope(proposal_path)

    if not proposal_scope.features and not proposal_scope.api_mentions:
        return {
            "passed": True,
            "covered": [],
            "missing": [],
            "extra": [],
            "covered_details": [],
            "missing_details": [],
            "extra_details": [],
            "routes": [],
            "models": [],
            "message": "No scope features found in proposal — passing by default",
        }

    # Find source directory
    src_dir = _find_src_dir(project_root)

    # Scan implementation
    impl_routes = extract_routes(src_dir)
    impl_models = extract_models(src_dir)

    # Compare
    result = compare_spec_impl(proposal_scope, impl_routes, impl_models)

    return {
        "passed": result.passed,
        "covered": result.covered,
        "missing": result.missing,
        "extra": result.extra,
        "covered_details": result.covered_details,
        "missing_details": result.missing_details,
        "extra_details": result.extra_details,
        "routes": [
            {"method": r.method, "path": r.path, "file": r.file}
            for r in impl_routes
        ],
        "models": [
            {"name": m.name, "file": m.file}
            for m in impl_models
        ],
        "coverage_rate": result.coverage_rate,
        "message": f"Coverage: {result.coverage_rate:.0%} ({len(result.covered)}/{len(result.covered) + len(result.missing)} features)",
    }


def _find_src_dir(project_root: str) -> str:
    """Find the source directory in the project."""
    candidates = ["src", "lib", "app", "source", ""]
    for candidate in candidates:
        path = os.path.join(project_root, candidate)
        if os.path.isdir(path):
            return path
    return project_root


# ─── Report generation ─────────────────────────────────────────────────

def generate_spec_report(result: Dict, change_name: str) -> str:
    """Generate a markdown spec compliance report.

    Args:
        result: Output from check_spec_compliance().
        change_name: Name of the change.

    Returns:
        Markdown report string.
    """
    lines = [
        "# Spec Compliance Report",
        "",
        f"> **Change**: {change_name}",
        f"> **Date**: {datetime.now().strftime('%Y-%m-%d %H:%M')}",
        f"> **Coverage**: {result.get('coverage_rate', 0):.0%}",
        "",
        "---",
        "",
        "## Proposal Scope",
        "",
    ]

    # Feature table
    features = result.get("covered", []) + result.get("missing", [])
    if features:
        lines.append("| # | Feature |")
        lines.append("|---|---------|")
        for i, feature in enumerate(features, 1):
            lines.append(f"| {i} | {feature} |")

    # Implementation inventory
    lines.extend(["", "---", "", "## Implementation Inventory", ""])

    routes = result.get("routes", [])
    if routes:
        lines.append("### API Routes")
        lines.append("| Method | Path | File |")
        lines.append("|--------|------|------|")
        for r in routes:
            # Make path relative
            short_file = r["file"]
            if len(short_file) > 50:
                short_file = "..." + short_file[-47:]
            lines.append(f"| {r['method']} | {r['path']} | {short_file} |")

    models = result.get("models", [])
    if models:
        lines.extend(["", "### Data Models"])
        lines.append("| Model | File |")
        lines.append("|-------|------|")
        for m in models:
            short_file = m["file"]
            if len(short_file) > 50:
                short_file = "..." + short_file[-47:]
            lines.append(f"| {m['name']} | {short_file} |")

    # Coverage analysis
    lines.extend(["", "---", "", "## Coverage Analysis", ""])

    covered = result.get("covered_details", [])
    if covered:
        lines.append("### Covered")
        for c in covered:
            evidence = c.get("evidence", [])
            evidence_str = evidence[0] if evidence else "matched"
            lines.append(f"- [x] {c['feature']} → {evidence_str}")

    missing = result.get("missing_details", [])
    if missing:
        lines.extend(["", "### Missing"])
        for m in missing:
            reason = m.get("reason", "")
            lines.append(f"- [ ] {m['feature']} — *{reason}*")

    extra = result.get("extra_details", [])
    if extra:
        lines.extend(["", "### Extra (Not in Proposal)"])
        for e in extra:
            lines.append(f"- [x] {e.get('name', 'unknown')} ({e.get('type', '')})")

    # Verdict
    lines.extend(["", "---", "", "## Verdict", ""])

    covered_count = len(result.get("covered", []))
    missing_count = len(result.get("missing", []))
    total = covered_count + missing_count

    if missing_count == 0:
        lines.append(f"**COMPLETE** — All {covered_count} proposal features implemented.")
    else:
        lines.append(
            f"**INCOMPLETE** — {covered_count}/{total} proposal features implemented "
            f"({result.get('coverage_rate', 0):.0%})"
        )
        lines.append(f"Missing items: {missing_count}")
        lines.append("")
        lines.append("Consider updating proposal or implementing missing features.")

    return "\n".join(lines)


# ─── CLI interface ──────────────────────────────────────────────────────

def main():
    import argparse

    parser = argparse.ArgumentParser(description="Spec compliance analysis")
    parser.add_argument("--change", required=True, help="Change name")
    parser.add_argument("--project-root", default=".", help="Project root directory")
    parser.add_argument("--json", action="store_true", help="Output as JSON")
    parser.add_argument("--save", action="store_true", help="Save report to test-reports/")

    args = parser.parse_args()

    project_root = os.path.abspath(args.project_root)
    change_dir = os.path.join(project_root, "openspec", "changes", args.change)

    if not os.path.isdir(change_dir):
        print(f"Error: Change directory not found: {change_dir}", file=sys.stderr)
        sys.exit(1)

    # Run analysis
    result = check_spec_compliance(change_dir, project_root)

    if args.json:
        print(json.dumps(result, indent=2, ensure_ascii=False))
    else:
        print(generate_spec_report(result, args.change))

    if args.save:
        test_reports_dir = os.path.join(change_dir, "test-reports")
        os.makedirs(test_reports_dir, exist_ok=True)

        timestamp = datetime.now().strftime("%Y%m%d-%H%M%S")
        filename = f"spec-compliance-{timestamp}.md"
        filepath = os.path.join(test_reports_dir, filename)

        with open(filepath, "w", encoding="utf-8") as f:
            f.write(generate_spec_report(result, args.change))

        if not args.json:
            print(f"\nReport saved to: {filepath}")

    sys.exit(0 if result.get("passed", True) else 1)


if __name__ == "__main__":
    main()
