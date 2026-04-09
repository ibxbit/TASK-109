import axios, {
  type AxiosInstance,
  type AxiosError,
  type InternalAxiosRequestConfig,
} from 'axios';
import { TOKEN_KEY } from '../../utils/constants';
import type { ApiError } from '../../types';

// ─────────────────────────────────────────────────────────────────────────────
// Axios instance
// In development the Vite proxy rewrites /api/* → http://localhost:8080/*
// In production set VITE_API_BASE_URL and configure the same proxy or CORS.
// ─────────────────────────────────────────────────────────────────────────────

const BASE_URL = '/api';

export const apiClient: AxiosInstance = axios.create({
  baseURL: BASE_URL,
  timeout: 30_000,
  headers: {
    'Content-Type': 'application/json',
    Accept: 'application/json',
  },
});

// ── Request interceptor — attach JWT Bearer token ─────────────────────────────

apiClient.interceptors.request.use(
  (config: InternalAxiosRequestConfig) => {
    const token = localStorage.getItem(TOKEN_KEY);
    if (token) {
      config.headers.Authorization = `Bearer ${token}`;
    }
    return config;
  },
  (error) => Promise.reject(error),
);

// ── Response interceptor — normalise errors & handle session expiry ───────────

apiClient.interceptors.response.use(
  (response) => response,
  (error: AxiosError) => {
    const status = error.response?.status ?? 0;

    if (status === 401) {
      // Session expired or token invalid — clear credentials and reload to /login
      localStorage.removeItem(TOKEN_KEY);
      // Avoid redirect loop: only redirect if not already on the login page
      if (!window.location.pathname.startsWith('/login')) {
        window.location.href = '/login?reason=session_expired';
      }
    }

    const apiError: ApiError = {
      status,
      message: extractErrorMessage(error),
    };

    return Promise.reject(apiError);
  },
);

function extractErrorMessage(error: AxiosError): string {
  if (!error.response) {
    return 'Network error — please check your connection and try again.';
  }
  const data = error.response.data as Record<string, unknown> | undefined;
  if (data) {
    if (typeof data.message === 'string') return data.message;
    if (typeof data.error   === 'string') return data.error;
    if (typeof data.detail  === 'string') return data.detail;
  }
  switch (error.response.status) {
    case 400: return 'Bad request — please check your input.';
    case 403: return 'You do not have permission to perform this action.';
    case 404: return 'The requested resource was not found.';
    case 409: return 'A conflict occurred — this entry may already exist.';
    case 422: return 'Validation error — please check your input.';
    case 429: return 'Too many requests — please slow down and try again.';
    case 500: return 'Server error — please try again later.';
    default:  return `Unexpected error (HTTP ${error.response.status}).`;
  }
}

export default apiClient;
