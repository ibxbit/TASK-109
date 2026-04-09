import { useState } from 'react';
import { useForm } from 'react-hook-form';
import { zodResolver } from '@hookform/resolvers/zod';
import { AlertCircle, Lock } from 'lucide-react';
import { useAuth } from '../../hooks/useAuth';
import { Input } from '../../components/ui/Input';
import { Button } from '../../components/ui/Button';
import { loginSchema, type LoginFormValues } from '../../utils/validators';
import type { CaptchaChallenge, LockedResponse } from '../../types';
import { APP_NAME } from '../../utils/constants';
import { formatDateTime } from '../../utils/formatters';

export function LoginPage() {
  const { login, loginPending, isCaptchaChallenge, isLockedResponse } = useAuth();

  const [captcha, setCaptcha]   = useState<CaptchaChallenge | null>(null);
  const [locked,  setLocked]    = useState<LockedResponse | null>(null);
  const [apiError, setApiError] = useState<string | null>(null);

  const {
    register,
    handleSubmit,
    formState: { errors },
    setValue,
  } = useForm<LoginFormValues>({
    resolver: zodResolver(loginSchema),
  });

  async function onSubmit(values: LoginFormValues) {
    setApiError(null);

    try {
      const response = await login({
        username:       values.username,
        password:       values.password,
        captcha_token:  captcha?.captcha_token,
        captcha_answer: values.captcha_answer,
      });

      if (isCaptchaChallenge(response)) {
        setCaptcha(response as CaptchaChallenge);
        setValue('captcha_answer', undefined);
        return;
      }

      if (isLockedResponse(response)) {
        setLocked(response as LockedResponse);
        setCaptcha(null);
        return;
      }

      // LoginSuccess — useAuth.login handles navigation
    } catch (err: unknown) {
      const msg = (err as { message?: string }).message ?? 'Login failed.';
      setApiError(msg);
    }
  }

  return (
    <div className="min-h-screen flex items-center justify-center bg-gradient-to-br from-brand-900 via-brand-800 to-brand-700 px-4">
      <div className="w-full max-w-sm">
        {/* Logo / Brand */}
        <div className="text-center mb-8">
          <div className="inline-flex items-center justify-center w-14 h-14 rounded-full bg-brand-600/30 border border-brand-500 mb-4">
            <Lock size={26} className="text-brand-200" />
          </div>
          <h1 className="text-2xl font-bold text-white">{APP_NAME}</h1>
          <p className="text-brand-300 text-sm mt-1">Sign in to your account</p>
        </div>

        {/* Card */}
        <div className="bg-white rounded-xl shadow-2xl px-8 py-8 space-y-5">

          {/* Account locked banner */}
          {locked && (
            <div className="flex gap-3 p-3 rounded-md bg-red-50 border border-red-200">
              <AlertCircle size={18} className="text-red-500 shrink-0 mt-0.5" />
              <div>
                <p className="text-sm font-medium text-red-700">Account locked</p>
                <p className="text-xs text-red-600 mt-0.5">
                  {locked.message} Locked until{' '}
                  {formatDateTime(locked.locked_until)}.
                </p>
              </div>
            </div>
          )}

          {/* API / server error */}
          {apiError && (
            <div
              role="alert"
              className="flex gap-3 p-3 rounded-md bg-red-50 border border-red-200"
            >
              <AlertCircle size={18} className="text-red-500 shrink-0 mt-0.5" />
              <p className="text-sm text-red-700">{apiError}</p>
            </div>
          )}

          <form onSubmit={handleSubmit(onSubmit)} noValidate className="space-y-4">
            <Input
              label="Username"
              type="text"
              autoComplete="username"
              autoFocus
              required
              error={errors.username?.message}
              {...register('username')}
            />

            <Input
              label="Password"
              type="password"
              autoComplete="current-password"
              required
              error={errors.password?.message}
              {...register('password')}
            />

            {/* CAPTCHA challenge */}
            {captcha && (
              <div className="p-3 rounded-md bg-amber-50 border border-amber-200 space-y-2">
                <p className="text-xs text-amber-700 font-medium">
                  Security check required after multiple failed attempts.
                </p>
                <p className="text-sm font-semibold text-amber-900">
                  {captcha.question}
                </p>
                <Input
                  label="Your answer"
                  type="number"
                  required
                  error={errors.captcha_answer?.message}
                  {...register('captcha_answer', { valueAsNumber: true })}
                />
              </div>
            )}

            <Button
              type="submit"
              className="w-full"
              loading={loginPending}
              disabled={loginPending || !!locked}
            >
              Sign in
            </Button>
          </form>

          <p className="text-xs text-slate-400 text-center">
            Sessions expire after 30 minutes of inactivity.
          </p>
        </div>
      </div>
    </div>
  );
}
