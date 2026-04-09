import { describe, it, expect, beforeEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { server } from '../mocks/server';
import apiClient from '../../services/api/client';

describe('API client error handling', () => {
  beforeEach(() => {
    // Clear localStorage before each test
    localStorage.clear();
  });

  it('attaches Bearer token from localStorage when present', async () => {
    localStorage.setItem('vp_token', 'test-token-123');
    let capturedAuth = '';

    server.use(
      http.get('/api/auth/me', ({ request }) => {
        capturedAuth = request.headers.get('Authorization') ?? '';
        return HttpResponse.json({ user: { id: '1', username: 'test' } });
      }),
    );

    await apiClient.get('/auth/me');
    expect(capturedAuth).toBe('Bearer test-token-123');
  });

  it('does not attach Authorization header when no token stored', async () => {
    let capturedAuth: string | null = 'initial';

    server.use(
      http.get('/api/auth/me', ({ request }) => {
        capturedAuth = request.headers.get('Authorization');
        return HttpResponse.json({ user: { id: '1', username: 'test' } });
      }),
    );

    await apiClient.get('/auth/me');
    expect(capturedAuth).toBeNull();
  });

  it('normalises 404 to a readable error message', async () => {
    server.use(
      http.get('/api/profile/not-found', () =>
        HttpResponse.json({ message: 'Profile not found' }, { status: 404 }),
      ),
    );

    try {
      await apiClient.get('/profile/not-found');
      expect.fail('Should have thrown');
    } catch (err: unknown) {
      expect((err as { status: number }).status).toBe(404);
      expect((err as { message: string }).message).toBe('Profile not found');
    }
  });

  it('normalises 403 with no body to a generic message', async () => {
    server.use(
      http.get('/api/admin-only', () =>
        new HttpResponse(null, { status: 403 }),
      ),
    );

    try {
      await apiClient.get('/admin-only');
      expect.fail('Should have thrown');
    } catch (err: unknown) {
      expect((err as { status: number }).status).toBe(403);
      expect((err as { message: string }).message).toContain('permission');
    }
  });

  it('normalises 500 to a server error message', async () => {
    server.use(
      http.get('/api/broken', () =>
        new HttpResponse(null, { status: 500 }),
      ),
    );

    try {
      await apiClient.get('/broken');
      expect.fail('Should have thrown');
    } catch (err: unknown) {
      expect((err as { status: number }).status).toBe(500);
    }
  });

  it('rejects with status 401 and ApiError shape', async () => {
    // Replace the 401 handler used in the /auth/me handler to return 401
    server.use(
      http.get('/api/session-check', () =>
        new HttpResponse(null, { status: 401 }),
      ),
    );

    try {
      await apiClient.get('/session-check');
      expect.fail('Should have thrown');
    } catch (err: unknown) {
      // The interceptor maps 401 → ApiError
      expect((err as { status: number }).status).toBe(401);
    }
  });
});
