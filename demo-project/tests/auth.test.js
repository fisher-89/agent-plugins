const request = require('supertest');
const path = require('path');
let app;

// Set test environment
process.env.DB_PATH = path.join(__dirname, '../data/test.db');
process.env.JWT_SECRET = 'test-secret';
process.env.JWT_EXPIRES_IN = '1h';
process.env.REDIS_URL = 'redis://invalid:9999'; // Will fail gracefully

beforeAll(() => {
  // Clear require cache and reinit app
  delete require.cache[require.resolve('../src/db/schema')];
  delete require.cache[require.resolve('../src/index')];
  const { initSchema } = require('../src/db/schema');
  initSchema();
  app = require('../src/index');
});

afterAll(() => {
  const { getDb } = require('../src/db/schema');
  const db = getDb();
  if (db) db.close();
});

describe('Auth System', () => {
  let authToken;
  let userId;

  describe('Registration', () => {
    it('should register a new user with valid email and password', async () => {
      const res = await request(app)
        .post('/auth/register')
        .send({ email: 'test@example.com', password: 'password123' });

      expect(res.status).toBe(201);
      expect(res.body.user).toHaveProperty('id');
      expect(res.body.user.email).toBe('test@example.com');
      expect(res.body.user.role).toBe('user');
      expect(res.body.user).not.toHaveProperty('password_hash');
      userId = res.body.user.id;
    });

    it('should reject duplicate email registration', async () => {
      const res = await request(app)
        .post('/auth/register')
        .send({ email: 'test@example.com', password: 'password123' });

      expect(res.status).toBe(409);
      expect(res.body.error).toMatch(/already registered/i);
    });

    it('should reject invalid email format', async () => {
      const res = await request(app)
        .post('/auth/register')
        .send({ email: 'invalid-email', password: 'password123' });

      expect(res.status).toBe(400);
    });

    it('should reject password shorter than 8 chars', async () => {
      const res = await request(app)
        .post('/auth/register')
        .send({ email: 'short@test.com', password: 'short' });

      expect(res.status).toBe(400);
    });
  });

  describe('Login', () => {
    it('should login with correct credentials', async () => {
      const res = await request(app)
        .post('/auth/login')
        .send({ email: 'test@example.com', password: 'password123' });

      expect(res.status).toBe(200);
      expect(res.body).toHaveProperty('token');
      expect(res.body.user).toHaveProperty('id');
      expect(res.body.user.email).toBe('test@example.com');
      authToken = res.body.token;
    });

    it('should reject wrong password', async () => {
      const res = await request(app)
        .post('/auth/login')
        .send({ email: 'test@example.com', password: 'wrongpassword' });

      expect(res.status).toBe(401);
    });

    it('should reject non-existent user', async () => {
      const res = await request(app)
        .post('/auth/login')
        .send({ email: 'nonexistent@example.com', password: 'password123' });

      expect(res.status).toBe(401);
    });

    it('should reject missing email or password', async () => {
      const res1 = await request(app).post('/auth/login').send({ email: 'test@example.com' });
      expect(res1.status).toBe(400);

      const res2 = await request(app).post('/auth/login').send({ password: 'password123' });
      expect(res2.status).toBe(400);
    });
  });

  describe('Session Middleware', () => {
    it('should access protected route with valid token', async () => {
      const res = await request(app)
        .get('/auth/me')
        .set('Authorization', `Bearer ${authToken}`);

      expect(res.status).toBe(200);
      expect(res.body.user.email).toBe('test@example.com');
    });

    it('should reject request without token', async () => {
      const res = await request(app).get('/auth/me');
      expect(res.status).toBe(401);
    });

    it('should reject invalid token', async () => {
      const res = await request(app)
        .get('/auth/me')
        .set('Authorization', 'Bearer invalid-token');

      expect(res.status).toBe(401);
    });
  });

  describe('Role-Based Access Control', () => {
    it('should deny regular user from admin route', async () => {
      const res = await request(app)
        .get('/auth/admin')
        .set('Authorization', `Bearer ${authToken}`);

      expect(res.status).toBe(403);
    });

    it('should allow admin user to admin route', async () => {
      // Create admin user directly in DB
      const bcrypt = require('bcryptjs');
      const { getDb } = require('../src/db/schema');
      const db = getDb();
      const passwordHash = bcrypt.hashSync('adminpass', 12);
      db.prepare('INSERT INTO users (email, password_hash, role) VALUES (?, ?, ?)').run('admin@test.com', passwordHash, 'admin');

      const res = await request(app)
        .post('/auth/login')
        .send({ email: 'admin@test.com', password: 'adminpass' });

      const adminToken = res.body.token;

      const adminRes = await request(app)
        .get('/auth/admin')
        .set('Authorization', `Bearer ${adminToken}`);

      expect(adminRes.status).toBe(200);
    });
  });

  describe('Logout', () => {
    it('should logout successfully', async () => {
      const res = await request(app)
        .post('/auth/logout')
        .set('Authorization', `Bearer ${authToken}`);

      expect(res.status).toBe(200);
      expect(res.body.message).toMatch(/logged out/i);
    });
  });

  describe('Health Check', () => {
    it('should return ok status', async () => {
      const res = await request(app).get('/health');
      expect(res.status).toBe(200);
      expect(res.body.status).toBe('ok');
    });
  });
});