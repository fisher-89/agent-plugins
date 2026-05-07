const express = require('express');
const cors = require('cors');
const { initSchema } = require('./db/schema');
const { initRedis } = require('./db/redis');
const config = require('./config');
const authRoutes = require('./routes/auth');
const { errorHandler } = require('./middleware/error');

const app = express();

// Initialize database schema
initSchema();

// Middleware
app.use(cors());
app.use(express.json());

// Routes
app.use('/auth', authRoutes);

// Health check
app.get('/health', (req, res) => {
  res.json({ status: 'ok' });
});

// Error handler
app.use(errorHandler);

// Start server
if (require.main === module) {
  initRedis().then(() => {
    app.listen(config.port, () => {
      console.log(`Server running on port ${config.port}`);
    });
  });
}

module.exports = app;