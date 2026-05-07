const jwt = require('jsonwebtoken');
const config = require('../config');
const { getRedis } = require('../db/redis');
const crypto = require('crypto');

function authenticate(req, res, next) {
  const auth = req.headers.authorization;
  if (!auth || !auth.startsWith('Bearer ')) {
    return res.status(401).json({ error: 'Authentication required' });
  }

  const token = auth.slice(7);

  try {
    const decoded = jwt.verify(token, config.jwtSecret);

    // Check Redis blacklist asynchronously
    const redis = getRedis();
    if (redis) {
      const tokenHash = crypto.createHash('sha256').update(token).digest('hex');
      redis.get(`blacklist:${tokenHash}`).then(blacklisted => {
        if (blacklisted) {
          return res.status(401).json({ error: 'Token has been revoked' });
        }
        req.user = { userId: decoded.userId, email: decoded.email, role: decoded.role || 'user' };
        next();
      }).catch(() => {
        // Redis error — still allow token (graceful degradation)
        req.user = { userId: decoded.userId, email: decoded.email, role: decoded.role || 'user' };
        next();
      });
    } else {
      req.user = { userId: decoded.userId, email: decoded.email, role: decoded.role || 'user' };
      next();
    }
  } catch (err) {
    return res.status(401).json({ error: 'Invalid or expired token' });
  }
}

function requireRole(...roles) {
  return (req, res, next) => {
    if (!req.user) {
      return res.status(401).json({ error: 'Authentication required' });
    }
    if (!roles.includes(req.user.role)) {
      return res.status(403).json({ error: 'Insufficient permissions' });
    }
    next();
  };
}

module.exports = { authenticate, requireRole };