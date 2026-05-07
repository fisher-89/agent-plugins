# Add User Authentication

## What
Add user authentication system supporting email/password and OAuth (Google, GitHub).

## Why
The application currently has no authentication. Users cannot have persistent accounts or access control.

## Scope
- Email/password signup and login
- OAuth integration (Google, GitHub)
- Session management with JWT
- Redis-based session/token caching (store active sessions, support token revocation)
- Basic role-based access control (user, admin)
