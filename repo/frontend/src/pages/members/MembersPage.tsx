import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { useForm } from 'react-hook-form';
import { zodResolver } from '@hookform/resolvers/zod';
import { Plus } from 'lucide-react';
import { z } from 'zod';
import * as metricsApi from '../../services/api/metrics';
import * as profilesApi from '../../services/api/profiles';
import { MetricChart } from '../../components/charts/MetricChart';
import { Button } from '../../components/ui/Button';
import { Input } from '../../components/ui/Input';
import { Select } from '../../components/ui/Select';
import { Textarea } from '../../components/ui/Textarea';
import { Modal } from '../../components/ui/Modal';
import { Card } from '../../components/ui/Card';
import { Spinner } from '../../components/ui/Spinner';
import { toast } from '../../components/ui/Toast';
import {
  healthProfileSchema,
  type HealthProfileFormValues,
} from '../../utils/validators';
import { formatDate } from '../../utils/formatters';
import {
  METRIC_TYPE_LABELS,
  NOTES_MAX_LENGTH,
} from '../../utils/constants';
import type { MetricType, HealthProfile, CreateMetricEntryRequest } from '../../types';
import { METRIC_UNITS } from '../../types';
import { useAuthStore } from '../../store/authStore';

const METRIC_TYPES: MetricType[] = [
  'weight',
  'body_fat_percentage',
  'waist',
  'hip',
  'chest',
  'blood_glucose',
];

const SEX_OPTIONS = [
  { value: 'male',              label: 'Male'             },
  { value: 'female',            label: 'Female'           },
  { value: 'other',             label: 'Other'            },
  { value: 'prefer_not_to_say', label: 'Prefer not to say'},
];

const ACTIVITY_OPTIONS = [
  { value: 'sedentary',          label: 'Sedentary'         },
  { value: 'lightly_active',     label: 'Lightly Active'    },
  { value: 'moderately_active',  label: 'Moderately Active' },
  { value: 'very_active',        label: 'Very Active'       },
  { value: 'extra_active',       label: 'Extra Active'      },
];

const RANGE_OPTIONS = [
  { value: '7d',  label: 'Last 7 days'  },
  { value: '30d', label: 'Last 30 days' },
  { value: '90d', label: 'Last 90 days' },
  { value: 'all', label: 'All time'     },
];

// Static metric entry form schema (range validation happens server-side)
const metricFormSchema = z.object({
  member_id:   z.string().uuid('Must be a valid member ID'),
  metric_type: z.enum([
    'weight', 'body_fat_percentage', 'waist', 'hip', 'chest', 'blood_glucose',
  ] as const),
  value:       z.coerce.number({ invalid_type_error: 'Value must be a number' }).positive('Must be positive'),
  entry_date:  z.string().regex(/^\d{4}-\d{2}-\d{2}$/, 'Must be YYYY-MM-DD'),
  notes:       z.string().max(500).optional(),
});

type MetricFormValues = z.infer<typeof metricFormSchema>;

// ── Member Detail Panel ───────────────────────────────────────────────────────

interface MemberDetailProps {
  memberId: string;
}

function MemberDetail({ memberId }: MemberDetailProps) {
  const qc = useQueryClient();
  const user = useAuthStore((s) => s.user);

  const [selectedMetric, setSelectedMetric] = useState<MetricType>('weight');
  const [range, setRange]                   = useState('30d');
  const [addMetricOpen, setAddMetricOpen]   = useState(false);
  const [editProfileOpen, setEditProfileOpen] = useState(false);

  // ── Health Profile ──────────────────────────────────────────────────────────
  const profileQuery = useQuery({
    queryKey: ['profile', memberId],
    queryFn:  () => profilesApi.getProfile(memberId),
    retry: (count, err: unknown) => {
      if ((err as { status?: number }).status === 404) return false;
      return count < 2;
    },
  });

  // ── Metrics ─────────────────────────────────────────────────────────────────
  const metricsQuery = useQuery({
    queryKey: ['metrics', memberId, selectedMetric, range],
    queryFn:  () =>
      metricsApi.getMetrics({
        member_id:   memberId,
        metric_type: selectedMetric,
        range:       range as '7d' | '30d' | '90d' | 'all',
      }),
  });

  // ── Add Metric Entry ────────────────────────────────────────────────────────
  const metricForm = useForm<MetricFormValues>({
    resolver: zodResolver(metricFormSchema),
    defaultValues: {
      member_id:   memberId,
      metric_type: selectedMetric,
      entry_date:  new Date().toISOString().slice(0, 10),
    },
  });

  const addMetricMutation = useMutation({
    mutationFn: (values: MetricFormValues) =>
      metricsApi.createMetricEntry(values as CreateMetricEntryRequest),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['metrics', memberId] });
      toast.success('Metric entry recorded.');
      setAddMetricOpen(false);
      metricForm.reset({
        member_id:   memberId,
        metric_type: selectedMetric,
        entry_date:  new Date().toISOString().slice(0, 10),
      });
    },
    onError: (err: unknown) => {
      toast.error((err as { message?: string }).message ?? 'Failed to save.');
    },
  });

  // ── Edit Profile ────────────────────────────────────────────────────────────
  const profileForm = useForm<HealthProfileFormValues>({
    resolver: zodResolver(healthProfileSchema),
    defaultValues: {
      dietary_notes: profileQuery.data?.dietary_notes ?? '',
      medical_notes: profileQuery.data?.medical_notes ?? '',
    },
  });

  const updateProfileMutation = useMutation({
    mutationFn: (values: HealthProfileFormValues) =>
      profilesApi.updateProfile(memberId, values),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['profile', memberId] });
      toast.success('Profile updated.');
      setEditProfileOpen(false);
    },
    onError: (err: unknown) => {
      toast.error((err as { message?: string }).message ?? 'Failed to update.');
    },
  });

  const profile: HealthProfile | undefined = profileQuery.data;
  const canEdit =
    user?.id === memberId ||
    user?.role_id === '00000000-0000-0000-0000-000000000001' ||
    user?.role_id === '00000000-0000-0000-0000-000000000002';

  const dietaryNotesValue = profileForm.watch('dietary_notes') ?? '';
  const medicalNotesValue = profileForm.watch('medical_notes') ?? '';
  const metricTypeValue   = metricForm.watch('metric_type') ?? selectedMetric;

  return (
    <div className="space-y-5">
      {/* Profile Card */}
      <Card
        title="Health Profile"
        actions={
          canEdit && (
            <Button
              size="sm"
              variant="secondary"
              onClick={() => {
                profileForm.reset({
                  date_of_birth:  profile?.date_of_birth ?? '',
                  sex:            profile?.sex ?? undefined,
                  height_in:      profile?.height_in ?? undefined,
                  weight_lbs:     profile?.weight_lbs ?? undefined,
                  activity_level: profile?.activity_level ?? undefined,
                  dietary_notes:  profile?.dietary_notes ?? '',
                  medical_notes:  profile?.medical_notes ?? '',
                });
                setEditProfileOpen(true);
              }}
            >
              Edit
            </Button>
          )
        }
      >
        {profileQuery.isLoading ? (
          <Spinner />
        ) : profileQuery.isError ? (
          <p className="text-sm text-slate-500">
            {(profileQuery.error as { status?: number }).status === 404
              ? 'No health profile found for this member.'
              : 'Failed to load profile.'}
          </p>
        ) : profile ? (
          <dl className="grid grid-cols-2 sm:grid-cols-3 gap-4 text-sm">
            {[
              { label: 'Date of Birth', value: formatDate(profile.date_of_birth) },
              { label: 'Sex',           value: profile.sex ?? 'N/A' },
              { label: 'Height',        value: profile.height_in ? `${profile.height_in} in` : 'N/A' },
              { label: 'Weight',        value: profile.weight_lbs ? `${profile.weight_lbs} lbs` : 'N/A' },
              { label: 'Activity',      value: profile.activity_level ?? 'N/A' },
            ].map(({ label, value }) => (
              <div key={label}>
                <dt className="text-xs text-slate-500">{label}</dt>
                <dd className="font-medium text-slate-800 capitalize">{value}</dd>
              </div>
            ))}
            {profile.dietary_notes && (
              <div className="col-span-full">
                <dt className="text-xs text-slate-500">Dietary Notes</dt>
                <dd className="text-slate-700 mt-0.5 whitespace-pre-wrap">
                  {profile.dietary_notes}
                </dd>
              </div>
            )}
            {profile.medical_notes && (
              <div className="col-span-full">
                <dt className="text-xs text-slate-500">Medical Notes</dt>
                <dd className="text-slate-700 mt-0.5 whitespace-pre-wrap">
                  {profile.medical_notes}
                </dd>
              </div>
            )}
          </dl>
        ) : null}
      </Card>

      {/* Metrics Chart */}
      <Card
        title="Metric Entries"
        actions={
          <Button
            size="sm"
            leftIcon={<Plus size={14} />}
            onClick={() => {
              metricForm.setValue('member_id', memberId);
              metricForm.setValue('metric_type', selectedMetric);
              setAddMetricOpen(true);
            }}
          >
            Add entry
          </Button>
        }
      >
        <div className="flex gap-3 mb-4 flex-wrap">
          <Select
            options={METRIC_TYPES.map((m) => ({
              value: m,
              label: METRIC_TYPE_LABELS[m] ?? m,
            }))}
            value={selectedMetric}
            onChange={(e) => setSelectedMetric(e.target.value as MetricType)}
            className="w-44"
          />
          <Select
            options={RANGE_OPTIONS}
            value={range}
            onChange={(e) => setRange(e.target.value)}
            className="w-36"
          />
        </div>

        {metricsQuery.isLoading ? (
          <div className="flex justify-center py-10">
            <Spinner />
          </div>
        ) : (
          <MetricChart
            entries={metricsQuery.data ?? []}
            metricType={selectedMetric}
          />
        )}
      </Card>

      {/* Add Metric Modal */}
      <Modal
        open={addMetricOpen}
        onClose={() => setAddMetricOpen(false)}
        title="Add Metric Entry"
        footer={
          <>
            <Button variant="secondary" onClick={() => setAddMetricOpen(false)}>
              Cancel
            </Button>
            <Button
              loading={addMetricMutation.isPending}
              disabled={addMetricMutation.isPending}
              onClick={metricForm.handleSubmit((v: MetricFormValues) =>
                addMetricMutation.mutate(v),
              )}
            >
              Save entry
            </Button>
          </>
        }
      >
        <form className="space-y-4" noValidate>
          <Select
            label="Metric type"
            required
            options={METRIC_TYPES.map((m) => ({
              value: m,
              label: `${METRIC_TYPE_LABELS[m]} (${METRIC_UNITS[m]})`,
            }))}
            error={metricForm.formState.errors.metric_type?.message}
            {...metricForm.register('metric_type')}
          />
          <Input
            label={`Value (${METRIC_UNITS[metricTypeValue as MetricType] ?? ''})`}
            type="number"
            step="0.1"
            required
            error={metricForm.formState.errors.value?.message}
            {...metricForm.register('value', { valueAsNumber: true })}
          />
          <Input
            label="Entry date"
            type="date"
            required
            error={metricForm.formState.errors.entry_date?.message}
            {...metricForm.register('entry_date')}
          />
          <Input
            label="Notes (optional)"
            type="text"
            error={metricForm.formState.errors.notes?.message}
            {...metricForm.register('notes')}
          />
        </form>
      </Modal>

      {/* Edit Profile Modal */}
      <Modal
        open={editProfileOpen}
        onClose={() => setEditProfileOpen(false)}
        title="Edit Health Profile"
        size="lg"
        footer={
          <>
            <Button variant="secondary" onClick={() => setEditProfileOpen(false)}>
              Cancel
            </Button>
            <Button
              loading={updateProfileMutation.isPending}
              disabled={updateProfileMutation.isPending}
              onClick={profileForm.handleSubmit((v: HealthProfileFormValues) =>
                updateProfileMutation.mutate(v),
              )}
            >
              Save changes
            </Button>
          </>
        }
      >
        <form className="space-y-4" noValidate>
          <div className="grid grid-cols-2 gap-4">
            <Input
              label="Date of birth"
              type="date"
              error={profileForm.formState.errors.date_of_birth?.message}
              {...profileForm.register('date_of_birth')}
            />
            <Select
              label="Sex"
              options={SEX_OPTIONS}
              placeholder="Select…"
              error={profileForm.formState.errors.sex?.message}
              {...profileForm.register('sex')}
            />
            <Input
              label="Height (inches)"
              type="number"
              step="0.5"
              error={profileForm.formState.errors.height_in?.message}
              {...profileForm.register('height_in', { valueAsNumber: true })}
            />
            <Input
              label="Weight (lbs)"
              type="number"
              step="0.1"
              error={profileForm.formState.errors.weight_lbs?.message}
              {...profileForm.register('weight_lbs', { valueAsNumber: true })}
            />
          </div>
          <Select
            label="Activity level"
            options={ACTIVITY_OPTIONS}
            placeholder="Select…"
            error={profileForm.formState.errors.activity_level?.message}
            {...profileForm.register('activity_level')}
          />
          <Textarea
            label="Dietary notes"
            maxChars={NOTES_MAX_LENGTH}
            currentLength={dietaryNotesValue.length}
            hint="Allergies, preferences, restrictions"
            error={profileForm.formState.errors.dietary_notes?.message}
            {...profileForm.register('dietary_notes')}
          />
          <Textarea
            label="Medical notes"
            maxChars={NOTES_MAX_LENGTH}
            currentLength={medicalNotesValue.length}
            hint="Conditions, medications, physician notes"
            error={profileForm.formState.errors.medical_notes?.message}
            {...profileForm.register('medical_notes')}
          />
        </form>
      </Modal>
    </div>
  );
}

// ── Members Page ──────────────────────────────────────────────────────────────

export function MembersPage() {
  const currentUser = useAuthStore((s) => s.user);
  const [selectedMemberId, setSelectedMemberId] = useState<string | null>(
    currentUser?.id ?? null,
  );
  const [searchId, setSearchId] = useState('');

  const handleSearch = () => {
    const trimmed = searchId.trim();
    if (trimmed) setSelectedMemberId(trimmed);
  };

  return (
    <div className="p-6 max-w-5xl mx-auto space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-xl font-bold text-slate-900">
            Members &amp; Health Profiles
          </h1>
          <p className="text-sm text-slate-500 mt-0.5">
            View and manage member health data, metrics, and profiles.
          </p>
        </div>
      </div>

      {/* Member lookup */}
      <Card title="Look up member">
        <div className="flex gap-3">
          <Input
            placeholder="Paste member UUID…"
            value={searchId}
            onChange={(e) => setSearchId(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && handleSearch()}
            className="flex-1"
          />
          <Button onClick={handleSearch}>View profile</Button>
        </div>
        {currentUser && (
          <button
            className="mt-2 text-xs text-brand-600 hover:underline"
            onClick={() => setSelectedMemberId(currentUser.id)}
          >
            View my profile ({currentUser.username})
          </button>
        )}
      </Card>

      {/* Member detail */}
      {selectedMemberId && (
        <div>
          <div className="flex items-center gap-2 text-xs text-slate-500 mb-3">
            <span>Member ID:</span>
            <code className="bg-slate-100 px-1.5 py-0.5 rounded text-slate-700">
              {selectedMemberId}
            </code>
          </div>
          <MemberDetail memberId={selectedMemberId} />
        </div>
      )}
    </div>
  );
}
