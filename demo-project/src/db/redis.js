const config = require('../config');

let redisClient = null;
let connectionAttempted = false;

function getRedis() {
  return redisClient;
}

async function initRedis() {
  if (connectionAttempted) return redisClient;
  connectionAttempted = true;

  try {
    const Redis = require('ioredis');
    redisClient = new Redis(config.redisUrl, { lazyConnect: true, retryStrategy: () => null });

    redisClient.on('error', (err) => {
      console.warn('Redis connection error:', err.message);
      redisClient = null;
    });

    await redisClient.connect();
    console.log('Redis connected');
    return redisClient;
  } catch (err) {
    console.warn('Redis unavailable, running without cache:', err.message);
    redisClient = null;
    return null;
  }
}

module.exports = { getRedis, initRedis };