function validateEmail(email) {
  if (!email || typeof email !== 'string') return false;
  const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
  return emailRegex.test(email.trim());
}

function validatePassword(password) {
  if (!password || typeof password !== 'string') return false;
  return password.length >= 8;
}

function sanitizeEmail(email) {
  if (!email || typeof email !== 'string') return null;
  return email.trim().toLowerCase();
}

module.exports = { validateEmail, validatePassword, sanitizeEmail };