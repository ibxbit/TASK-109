import { describe, it, expect, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { http, HttpResponse } from 'msw';
import { server } from '../mocks/server';
import { LoginPage } from '../../pages/auth/LoginPage';
import { useAuthStore } from '../../store/authStore';

function renderLogin() {
  const qc = new QueryClient({ defaultOptions: { mutations: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>
        <LoginPage />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe('LoginPage', () => {
  beforeEach(() => {
    localStorage.clear();
    useAuthStore.getState().clearCredentials();
  });

  it('renders the sign-in form', () => {
    renderLogin();
    expect(screen.getByLabelText(/username/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/password/i)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /sign in/i })).toBeInTheDocument();
  });

  it('shows required errors when submitted empty', async () => {
    renderLogin();
    await userEvent.click(screen.getByRole('button', { name: /sign in/i }));
    await waitFor(() => {
      expect(screen.getByText(/username is required/i)).toBeInTheDocument();
      expect(screen.getByText(/password is required/i)).toBeInTheDocument();
    });
  });

  it('shows API error on invalid credentials', async () => {
    renderLogin();
    await userEvent.type(screen.getByLabelText(/username/i), 'baduser');
    await userEvent.type(screen.getByLabelText(/password/i), 'badpass');
    await userEvent.click(screen.getByRole('button', { name: /sign in/i }));
    await waitFor(() => {
      expect(screen.getByRole('alert')).toBeInTheDocument();
    });
  });

  it('shows CAPTCHA challenge when server requests it', async () => {
    renderLogin();
    await userEvent.type(screen.getByLabelText(/username/i), 'captcha');
    await userEvent.type(screen.getByLabelText(/password/i), 'any');
    await userEvent.click(screen.getByRole('button', { name: /sign in/i }));
    await waitFor(() => {
      expect(screen.getByText(/security check required/i)).toBeInTheDocument();
      expect(screen.getByText(/what is 3 \+ 4\?/i)).toBeInTheDocument();
    });
  });

  it('shows account locked banner when server returns locked', async () => {
    renderLogin();
    await userEvent.type(screen.getByLabelText(/username/i), 'locked');
    await userEvent.type(screen.getByLabelText(/password/i), 'any');
    await userEvent.click(screen.getByRole('button', { name: /sign in/i }));
    await waitFor(() => {
      // The banner heading "Account locked" is in the p.text-sm.font-medium
      expect(screen.getByText('Account locked')).toBeInTheDocument();
    });
  });

  it('stores token and user on successful login', async () => {
    renderLogin();
    await userEvent.type(screen.getByLabelText(/username/i), 'admin');
    await userEvent.type(screen.getByLabelText(/password/i), 'password');
    await userEvent.click(screen.getByRole('button', { name: /sign in/i }));
    await waitFor(() => {
      expect(localStorage.getItem('vp_token')).toBe('mock-jwt-token');
    });
  });

  it('disables the submit button while loading', async () => {
    // Delay the response so we can inspect the button state mid-flight
    server.use(
      http.post('/api/auth/login', async () => {
        await new Promise((r) => setTimeout(r, 100));
        return HttpResponse.json({
          token:      'mock-jwt-token',
          user:       { id: '1', username: 'admin', role_id: '00000000-0000-0000-0000-000000000001' },
          expires_at: new Date().toISOString(),
        });
      }),
    );

    renderLogin();
    await userEvent.type(screen.getByLabelText(/username/i), 'admin');
    await userEvent.type(screen.getByLabelText(/password/i), 'password');

    const btn = screen.getByRole('button', { name: /sign in/i });
    await userEvent.click(btn);

    expect(btn).toBeDisabled();
    await waitFor(() => expect(btn).not.toBeDisabled(), { timeout: 2000 });
  });
});
