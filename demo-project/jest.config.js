module.exports = {
  testEnvironment: 'node',
  testMatch: ['**/tests/**/*.test.js'],
  forceExit: true,
  coveragePathIgnorePatterns: ['/node_modules/', '/tests/'],
};