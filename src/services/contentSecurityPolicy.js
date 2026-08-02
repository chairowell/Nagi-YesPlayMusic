export const CONTENT_SECURITY_POLICY = [
  "default-src 'self'",
  "base-uri 'self'",
  "script-src 'self'",
  "style-src 'self' 'unsafe-inline'",
  "img-src 'self' data: blob: http: https:",
  "media-src 'self' data: blob: http: https:",
  "font-src 'self' data:",
  "connect-src 'self' ipc: http://ipc.localhost http: https:",
  "worker-src 'self' blob:",
  "object-src 'none'",
  "frame-ancestors 'none'",
  "form-action 'self'",
].join('; ');

export function applyRendererSecurityHeaders(_request, response, next) {
  response.setHeader('Content-Security-Policy', CONTENT_SECURITY_POLICY);
  response.setHeader('X-Content-Type-Options', 'nosniff');
  response.setHeader('Referrer-Policy', 'no-referrer');
  next();
}
