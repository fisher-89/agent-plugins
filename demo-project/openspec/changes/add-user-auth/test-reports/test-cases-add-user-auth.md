# Test Cases: add-user-auth

## Proposal Summary

Add user authentication system supporting email/password and OAuth (Google, GitHub).

## Test Cases by Task

### Task 1: Set up auth database schema (users, sessions, oauth_accounts tables)

#### Unit Tests
- [ ] Should create all three tables (users, sessions, oauth_accounts) on initSchema
- [ ] Should enforce UNIQUE on users.email — inserting duplicate email throws error
- [ ] Should enforce UNIQUE on oauth_accounts(provider, provider_account_id)
- [ ] Should enforce CHECK constraint on users.role — only 'user' or 'admin' allowed
- [ ] Should enforce CHECK constraint on oauth_accounts.provider — only 'google' or 'github'
- [ ] Should enforce foreign key: sessions.user_id references users.id
- [ ] Should enforce foreign key: oauth_accounts.user_id references users.id
- [ ] Should cascade delete sessions and oauth_accounts when user is deleted
- [ ] getDb() should return the same instance on repeated calls (singleton)

#### Edge Cases
- [ ] Inserting user with invalid role (e.g. 'superadmin') should fail
- [ ] Inserting oauth_account with invalid provider (e.g. 'facebook') should fail

### Task 2: Implement email/password registration endpoint

#### Unit Tests
- [ ] POST /auth/register with valid email+password returns 201 and user object (no password_hash)
- [ ] Password is hashed with bcrypt before storage
- [ ] Duplicate email registration returns 409 Conflict
- [ ] Missing email returns 400 with validation error
- [ ] Missing password returns 400 with validation error
- [ ] Invalid email format returns 400 with validation error
- [ ] Password shorter than 8 chars returns 400 with validation error

#### Integration Tests
- [ ] Register → login flow: newly registered user can log in immediately

#### Edge Cases
- [ ] Empty string email returns 400
- [ ] Email with extra whitespace is trimmed before validation
- [ ] Very long password (> 128 chars) is handled gracefully

### Task 3: Implement login endpoint with JWT generation

#### Unit Tests
- [ ] POST /auth/login with correct credentials returns 200 with JWT token
- [ ] Response includes token and user info (id, email, role)
- [ ] Wrong password returns 401 Unauthorized
- [ ] Non-existent email returns 401 Unauthorized
- [ ] JWT payload contains userId, email, and role
- [ ] JWT expires according to configured expiry time

#### Integration Tests
- [ ] Login → access protected route: token from login grants access

#### Edge Cases
- [ ] Empty email/password returns 400, not 401
- [ ] Account created via OAuth (no password_hash) cannot log in via email/password — returns appropriate error

### Task 4: Add OAuth provider configuration (Google, GitHub)

#### Unit Tests
- [ ] Config exposes Google OAuth redirect URL with correct client_id
- [ ] Config exposes GitHub OAuth redirect URL with correct client_id
- [ ] Missing OAuth env vars result in graceful degradation (endpoint returns 501)

#### Edge Cases
- [ ] Neither Google nor GitHub configured — both endpoints return 501

### Task 5: Implement OAuth callback handlers

#### Unit Tests
- [ ] GET /auth/google/callback with valid code creates/finds user and returns JWT
- [ ] GET /auth/github/callback with valid code creates/finds user and returns JWT
- [ ] Existing user with same OAuth provider links to existing account (no duplicate)
- [ ] Invalid authorization code returns 401
- [ ] OAuth user gets 'user' role by default

#### Integration Tests
- [ ] OAuth callback creates oauth_accounts record with provider and provider_account_id

#### Edge Cases
- [ ] User registers with email, then logs in via same-email OAuth — accounts link correctly
- [ ] OAuth provider returns error — handled gracefully with 401

### Task 6: Add session management middleware

#### Unit Tests
- [ ] Valid Bearer token in Authorization header attaches req.user
- [ ] Missing Authorization header returns 401
- [ ] Invalid/malformed JWT returns 401
- [ ] Expired JWT returns 401
- [ ] req.user contains userId, email, role

#### Integration Tests
- [ ] Protected route accessible with valid token, 401 without

#### Edge Cases
- [ ] Token with incorrect signature returns 401
- [ ] Bearer token with extra spaces handled correctly

### Task 7: Set up Redis connection and session/token caching layer

#### Unit Tests
- [ ] Redis client connects successfully (or graceful fallback when unavailable)
- [ ] Session can be stored and retrieved from Redis cache
- [ ] Token can be cached with TTL matching JWT expiry
- [ ] Cached session is JSON-serializable and round-trips correctly

#### Edge Cases
- [ ] Redis unavailable — session falls back to DB lookup (graceful degradation)
- [ ] TTL expiry removes cached session automatically

### Task 8: Implement token revocation via Redis (logout, blacklist)

#### Unit Tests
- [ ] POST /auth/logout with valid token blacklists it in Redis
- [ ] Blacklisted token returns 401 on subsequent requests
- [ ] Logout without token returns 401
- [ ] Already-blacklisted token returns 401 on use

#### Integration Tests
- [ ] Login → logout → access protected route returns 401

#### Edge Cases
- [ ] Revoking an expired token — no error, idempotent
- [ ] Blacklist TTL matches or exceeds JWT expiry

### Task 9: Implement role-based access control checks

#### Unit Tests
- [ ] requireRole('admin') allows admin user, blocks regular user with 403
- [ ] requireRole('user') allows both user and admin roles
- [ ] User without required role gets 403 Forbidden (not 401)
- [ ] requireRole() works as middleware in route definition

#### Integration Tests
- [ ] Admin-only route: admin token succeeds, user token gets 403

#### Edge Cases
- [ ] Token with no role claim defaults to 'user' — admin route returns 403
- [ ] Multiple roles — requireRole('admin', 'editor') allows either

### Task 10: Write integration tests for auth flows

#### Integration Tests
- [ ] Full flow: register → login → access protected route → logout → denied access
- [ ] Full flow: register → login → admin-only route denied (role='user')
- [ ] Admin flow: create admin user → login → access admin route → success
- [ ] OAuth mock flow: callback → token issued → access protected route
- [ ] Token revocation flow: login → logout → token blacklisted → 401

---

*Reviewed and expanded with specific assertions and expected results.*
