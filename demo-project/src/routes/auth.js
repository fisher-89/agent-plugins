const express = require('express');
const bcrypt = require('bcryptjs');
const jwt = require('jsonwebtoken');
const { getDb } = require('../db/schema');
const config = require('../config');
const { validateEmail, validatePassword, sanitizeEmail } = require('../utils/validators');
const { authenticate, requireRole } = require('../middleware/auth');
const { getRedis } = require('../db/redis');

const crypto = require('crypto');

const router = express.Router();

// --- Registration ---
router.post('/register', (req, res, next) => {
  try {
    const { email, password, displayName } = req.body;
    const cleanEmail = sanitizeEmail(email);

    if (!cleanEmail || !validateEmail(cleanEmail)) {
      return res.status(400).json({ error: 'Valid email is required' });
    }
    if (!validatePassword(password)) {
      return res.status(400).json({ error: 'Password must be at least 8 characters' });
    }

    const db = getDb();

    const existing = db.prepare('SELECT id FROM users WHERE email = ?').get(cleanEmail);
    if (existing) {
      return res.status(409).json({ error: 'Email already registered' });
    }

    const passwordHash = bcrypt.hashSync(password, 12);
    const result = db.prepare(
      'INSERT INTO users (email, password_hash, display_name) VALUES (?, ?, ?)'
    ).run(cleanEmail, passwordHash, displayName || null);

    const user = { id: result.lastInsertRowid, email: cleanEmail, role: 'user', displayName: displayName || null };

    res.status(201).json({ user });
  } catch (err) {
    next(err);
  }
});

// --- Login ---
router.post('/login', (req, res, next) => {
  try {
    const { email, password } = req.body;
    const cleanEmail = sanitizeEmail(email);

    if (!cleanEmail || !password) {
      return res.status(400).json({ error: 'Email and password are required' });
    }

    const db = getDb();
    const user = db.prepare('SELECT id, email, password_hash, role FROM users WHERE email = ?').get(cleanEmail);

    if (!user || !user.password_hash) {
      return res.status(401).json({ error: 'Invalid credentials' });
    }

    const valid = bcrypt.compareSync(password, user.password_hash);
    if (!valid) {
      return res.status(401).json({ error: 'Invalid credentials' });
    }

    const token = generateToken(user);
    const redis = getRedis();
    if (redis) {
      const tokenHash = hashToken(token);
      redis.setex(`session:${tokenHash}`, ttlSeconds(), JSON.stringify({ userId: user.id, email: user.email, role: user.role })).catch(() => {});
    }

    res.json({ token, user: { id: user.id, email: user.email, role: user.role } });
  } catch (err) {
    next(err);
  }
});

// --- Logout ---
router.post('/logout', authenticate, async (req, res, next) => {
  try {
    const token = extractToken(req);
    if (token) {
      const redis = getRedis();
      if (redis) {
        const tokenHash = hashToken(token);
        const ttl = ttlSeconds();
        await redis.setex(`blacklist:${tokenHash}`, ttl, '1').catch(() => {});
      }
    }
    res.json({ message: 'Logged out' });
  } catch (err) {
    next(err);
  }
});

// --- OAuth: Google ---
router.get('/google', (req, res) => {
  if (!config.googleClientId) {
    return res.status(501).json({ error: 'Google OAuth not configured' });
  }
  const redirectUri = `${req.protocol}://${req.get('host')}/auth/google/callback`;
  const url = `https://accounts.google.com/o/oauth2/v2/auth?client_id=${config.googleClientId}&redirect_uri=${encodeURIComponent(redirectUri)}&response_type=code&scope=email+profile`;
  res.redirect(url);
});

router.get('/google/callback', async (req, res, next) => {
  try {
    if (!config.googleClientId || !config.googleClientSecret) {
      return res.status(501).json({ error: 'Google OAuth not configured' });
    }
    const { code } = req.query;
    if (!code) return res.status(401).json({ error: 'Authorization code required' });

    // Exchange code for tokens
    const tokenRes = await fetch('https://oauth2.googleapis.com/token', {
      method: 'POST',
      headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
      body: new URLSearchParams({
        code, client_id: config.googleClientId, client_secret: config.googleClientSecret,
        redirect_uri: `${req.protocol}://${req.get('host')}/auth/google/callback`,
        grant_type: 'authorization_code',
      }),
    });
    if (!tokenRes.ok) return res.status(401).json({ error: 'Failed to exchange code with Google' });
    const tokenData = await tokenRes.json();
    if (!tokenData.access_token) return res.status(401).json({ error: 'Failed to exchange code' });

    // Fetch user profile
    const profileRes = await fetch('https://www.googleapis.com/oauth2/v2/userinfo', {
      headers: { Authorization: `Bearer ${tokenData.access_token}` },
    });
    if (!profileRes.ok) return res.status(401).json({ error: 'Failed to fetch Google profile' });
    const profile = await profileRes.json();

    const user = upsertOAuthUser('google', profile.id, profile.email, profile.name, profile.picture, tokenData.access_token, tokenData.refresh_token);
    const jwtToken = generateToken(user);

    res.json({ token: jwtToken, user: { id: user.id, email: user.email, role: user.role } });
  } catch (err) {
    next(err);
  }
});

// --- OAuth: GitHub ---
router.get('/github', (req, res) => {
  if (!config.githubClientId) {
    return res.status(501).json({ error: 'GitHub OAuth not configured' });
  }
  const url = `https://github.com/login/oauth/authorize?client_id=${config.githubClientId}&scope=user:email`;
  res.redirect(url);
});

router.get('/github/callback', async (req, res, next) => {
  try {
    if (!config.githubClientId || !config.githubClientSecret) {
      return res.status(501).json({ error: 'GitHub OAuth not configured' });
    }
    const { code } = req.query;
    if (!code) return res.status(401).json({ error: 'Authorization code required' });

    const tokenRes = await fetch('https://github.com/login/oauth/access_token', {
      method: 'POST',
      headers: { 'Accept': 'application/json', 'Content-Type': 'application/json' },
      body: JSON.stringify({ code, client_id: config.githubClientId, client_secret: config.githubClientSecret }),
    });
    if (!tokenRes.ok) return res.status(401).json({ error: 'Failed to exchange code with GitHub' });
    const tokenData = await tokenRes.json();
    if (!tokenData.access_token) return res.status(401).json({ error: 'Failed to exchange code' });

    const profileRes = await fetch('https://api.github.com/user', {
      headers: { Authorization: `Bearer ${tokenData.access_token}`, 'User-Agent': 'demo-project' },
    });
    if (!profileRes.ok) return res.status(401).json({ error: 'Failed to fetch GitHub profile' });
    const profile = await profileRes.json();

    const user = upsertOAuthUser('github', String(profile.id), profile.email, profile.name, profile.avatar_url, tokenData.access_token, null);
    const jwtToken = generateToken(user);

    res.json({ token: jwtToken, user: { id: user.id, email: user.email, role: user.role } });
  } catch (err) {
    next(err);
  }
});

// --- Protected route example ---
router.get('/me', authenticate, (req, res) => {
  res.json({ user: req.user });
});

// --- Admin-only route example ---
router.get('/admin', authenticate, requireRole('admin'), (req, res) => {
  res.json({ message: 'Admin access granted' });
});

// --- Helpers ---
function generateToken(user) {
  return jwt.sign({ userId: user.id, email: user.email, role: user.role }, config.jwtSecret, { expiresIn: config.jwtExpiresIn });
}

function extractToken(req) {
  const auth = req.headers.authorization;
  if (!auth || !auth.startsWith('Bearer ')) return null;
  return auth.slice(7);
}

function hashToken(token) {
  return crypto.createHash('sha256').update(token).digest('hex');
}

function ttlSeconds() {
  const expiry = config.jwtExpiresIn;
  const match = expiry.match(/^(\d+)(h|d|s|m)$/);
  if (!match) return 86400;
  const val = parseInt(match[1]);
  const unit = match[2];
  if (unit === 's') return val;
  if (unit === 'm') return val * 60;
  if (unit === 'h') return val * 3600;
  if (unit === 'd') return val * 86400;
  return 86400;
}

function upsertOAuthUser(provider, providerAccountId, email, displayName, avatarUrl, accessToken, refreshToken) {
  const db = getDb();
  const cleanEmail = email ? email.trim().toLowerCase() : `${provider}_${providerAccountId}@oauth.local`;

  // Check if OAuth account exists
  let user = db.prepare(`
    SELECT u.id, u.email, u.role FROM users u
    JOIN oauth_accounts o ON o.user_id = u.id
    WHERE o.provider = ? AND o.provider_account_id = ?
  `).get(provider, providerAccountId);

  if (!user) {
    // Check if user exists with same email (link accounts)
    user = db.prepare('SELECT id, email, role FROM users WHERE email = ?').get(cleanEmail);

    if (!user) {
      // Create new user
      const result = db.prepare(
        'INSERT INTO users (email, display_name, avatar_url) VALUES (?, ?, ?)'
      ).run(cleanEmail, displayName || null, avatarUrl || null);
      user = { id: result.lastInsertRowid, email: cleanEmail, role: 'user' };
    }

    // Create OAuth account link
    db.prepare(
      'INSERT INTO oauth_accounts (user_id, provider, provider_account_id, access_token, refresh_token) VALUES (?, ?, ?, ?, ?)'
    ).run(user.id, provider, providerAccountId, accessToken || null, refreshToken || null);
  } else {
    // Update tokens
    db.prepare(
      'UPDATE oauth_accounts SET access_token = ?, refresh_token = ?, updated_at = datetime(\'now\') WHERE provider = ? AND provider_account_id = ?'
    ).run(accessToken || null, refreshToken || null, provider, providerAccountId);
  }

  return user;
}

module.exports = router;