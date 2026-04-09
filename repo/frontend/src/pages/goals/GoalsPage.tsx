import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { useForm } from 'react-hook-form';
import { zodResolver } from '@hookform/resolvers/zod';
import { Plus, Target } from 'lucide-react';
import * as goalsApi from '../../services/api/goals';
import { Button } from '../../components/ui/Button';
import { Input } from '../../components/ui/Input';
import { Select } from '../../components/ui/Select';
import { Textarea } from '../../components/ui/Textarea';
import { Modal } from '../../components/ui/Modal';
import { Card } from '../../components/ui/Card';
import { Badge } from '../../components/ui/Badge';
import { EmptyState } from '../../components/ui/EmptyState';
import { Spinner } from '../../components/ui/Spinner';
import { toast } from '../../components/ui/Toast';
import {
  createGoalSchema,
  type CreateGoalFormValues,
} from '../../utils/validators';
import { formatDate } from '../../utils/formatters';
import { GOAL_TYPE_LABELS } from '../../utils/constants';
import { useAuthStore } from '../../store/authStore';
import type { Goal, GoalStatus } from '../../types';
import { ROLE_IDS } from '../../utils/constants';

const STATUS_OPTIONS: { value: GoalStatus | ''; label: string }[] = [
  { value: '',          label: 'All statuses' },
  { value: 'active',    label: 'Active'       },
  { value: 'paused',    label: 'Paused'       },
  { value: 'completed', label: 'Completed'    },
  { value: 'cancelled', label: 'Cancelled'    },
];

const GOAL_TYPE_OPTIONS = [
  { value: 'fat_loss',        label: 'Fat Loss'        },
  { value: 'muscle_gain',     label: 'Muscle Gain'     },
  { value: 'glucose_control', label: 'Glucose Control' },
];

const statusVariant: Record<GoalStatus, 'success' | 'warning' | 'info' | 'default'> = {
  active:    'success',
  paused:    'warning',
  completed: 'info',
  cancelled: 'default',
};

export function GoalsPage() {
  const qc = useQueryClient();
  const user = useAuthStore((s) => s.user);
  const isAdminOrCoach =
    user?.role_id === ROLE_IDS.ADMINISTRATOR ||
    user?.role_id === ROLE_IDS.CARE_COACH;

  const [statusFilter, setStatusFilter] = useState<GoalStatus | ''>('active');
  const [createOpen, setCreateOpen]     = useState(false);

  const goalsQuery = useQuery({
    queryKey: ['goals', statusFilter, isAdminOrCoach ? 'all' : user?.id],
    queryFn: () =>
      goalsApi.getGoals({
        member_id: isAdminOrCoach ? undefined : (user?.id ?? ''),
        status:    statusFilter || undefined,
      }),
  });

  const form = useForm<CreateGoalFormValues>({
    resolver: zodResolver(createGoalSchema),
    defaultValues: {
      start_date: new Date().toISOString().slice(0, 10),
    },
  });

  const createMutation = useMutation({
    mutationFn: (values: CreateGoalFormValues) =>
      goalsApi.createGoal({
        ...values,
        target_date: values.target_date || undefined,
        start_date:  values.start_date  || undefined,
      }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['goals'] });
      toast.success('Goal created successfully.');
      setCreateOpen(false);
      form.reset();
    },
    onError: (err: unknown) => {
      toast.error((err as { message?: string }).message ?? 'Failed to create goal.');
    },
  });

  const toggleStatusMutation = useMutation({
    mutationFn: ({ id, status }: { id: string; status: GoalStatus }) =>
      goalsApi.updateGoal(id, { status }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['goals'] });
      toast.success('Goal status updated.');
    },
    onError: (err: unknown) => {
      toast.error((err as { message?: string }).message ?? 'Failed to update.');
    },
  });

  const goals = goalsQuery.data ?? [];

  return (
    <div className="p-6 max-w-5xl mx-auto space-y-5">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-xl font-bold text-slate-900">Goals</h1>
          <p className="text-sm text-slate-500 mt-0.5">
            Track active, paused, and completed health goals.
          </p>
        </div>
        {isAdminOrCoach && (
          <Button leftIcon={<Plus size={15} />} onClick={() => setCreateOpen(true)}>
            New goal
          </Button>
        )}
      </div>

      {/* Filters */}
      <div className="flex items-center gap-3">
        <Select
          options={STATUS_OPTIONS as { value: string; label: string }[]}
          value={statusFilter}
          onChange={(e) => setStatusFilter(e.target.value as GoalStatus | '')}
          className="w-44"
        />
      </div>

      {/* Goals list */}
      {goalsQuery.isLoading ? (
        <div className="flex justify-center py-16"><Spinner /></div>
      ) : goalsQuery.isError ? (
        <Card>
          <p className="text-sm text-red-600">
            {(goalsQuery.error as { message?: string }).message ?? 'Failed to load goals.'}
          </p>
        </Card>
      ) : goals.length === 0 ? (
        <EmptyState
          icon={<Target size={48} />}
          title="No goals found"
          description="Goals will appear here once created."
          action={
            isAdminOrCoach && (
              <Button size="sm" onClick={() => setCreateOpen(true)}>
                Create first goal
              </Button>
            )
          }
        />
      ) : (
        <div className="space-y-3">
          {goals.map((goal: Goal) => (
            <GoalCard
              key={goal.id}
              goal={goal}
              canManage={isAdminOrCoach}
              isMember={user?.role_id === ROLE_IDS.MEMBER}
              onToggle={(id, status) => toggleStatusMutation.mutate({ id, status })}
            />
          ))}
        </div>
      )}

      {/* Create Goal Modal */}
      <Modal
        open={createOpen}
        onClose={() => setCreateOpen(false)}
        title="Create Goal"
        size="lg"
        footer={
          <>
            <Button variant="secondary" onClick={() => setCreateOpen(false)}>
              Cancel
            </Button>
            <Button
              loading={createMutation.isPending}
              disabled={createMutation.isPending}
              onClick={form.handleSubmit((v: CreateGoalFormValues) => createMutation.mutate(v))}
            >
              Create goal
            </Button>
          </>
        }
      >
        <form className="space-y-4" noValidate>
          <Input
            label="Member ID"
            required
            placeholder="UUID of the member"
            error={form.formState.errors.member_id?.message}
            {...form.register('member_id')}
          />
          <Input
            label="Title"
            required
            error={form.formState.errors.title?.message}
            {...form.register('title')}
          />
          <div className="grid grid-cols-2 gap-4">
            <Select
              label="Goal type"
              required
              options={GOAL_TYPE_OPTIONS}
              placeholder="Select…"
              error={form.formState.errors.goal_type?.message}
              {...form.register('goal_type')}
            />
            <Input
              label="Target value"
              type="number"
              step="0.1"
              required
              error={form.formState.errors.target_value?.message}
              {...form.register('target_value', { valueAsNumber: true })}
            />
            <Input
              label="Baseline value"
              type="number"
              step="0.1"
              error={form.formState.errors.baseline_value?.message}
              {...form.register('baseline_value', { valueAsNumber: true })}
            />
            <Input
              label="Target date"
              type="date"
              error={form.formState.errors.target_date?.message}
              {...form.register('target_date')}
            />
            <Input
              label="Start date"
              type="date"
              error={form.formState.errors.start_date?.message}
              {...form.register('start_date')}
            />
          </div>
          <Textarea
            label="Description (optional)"
            error={form.formState.errors.description?.message}
            {...form.register('description')}
          />
        </form>
      </Modal>
    </div>
  );
}

// ── Goal Card ─────────────────────────────────────────────────────────────────

interface GoalCardProps {
  goal:       Goal;
  canManage:  boolean;
  isMember:   boolean;
  onToggle:   (id: string, status: GoalStatus) => void;
}

function GoalCard({ goal, canManage, isMember, onToggle }: GoalCardProps) {
  return (
    <div className="bg-white rounded-lg border border-slate-200 shadow-sm px-5 py-4 flex items-start justify-between gap-4 hover:border-slate-300 transition-colors">
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2 flex-wrap">
          <h3 className="text-sm font-semibold text-slate-800 truncate">
            {goal.title}
          </h3>
          <Badge variant={statusVariant[goal.status]}>
            {goal.status}
          </Badge>
          <Badge variant="default">
            {GOAL_TYPE_LABELS[goal.goal_type] ?? goal.goal_type}
          </Badge>
        </div>
        {goal.description && (
          <p className="text-xs text-slate-500 mt-1 line-clamp-2">
            {goal.description}
          </p>
        )}
        <div className="flex items-center gap-4 mt-2 text-xs text-slate-500">
          <span>Target: <strong>{goal.target_value}</strong></span>
          {goal.target_date && (
            <span>Due: <strong>{formatDate(goal.target_date)}</strong></span>
          )}
          <span>Started: {formatDate(goal.start_date)}</span>
        </div>
      </div>

      {/* Actions */}
      <div className="shrink-0">
        {(canManage || isMember) && goal.status === 'active' && (
          <Button
            size="sm"
            variant="secondary"
            onClick={() => onToggle(goal.id, 'paused')}
          >
            Pause
          </Button>
        )}
        {(canManage || isMember) && goal.status === 'paused' && (
          <Button
            size="sm"
            variant="secondary"
            onClick={() => onToggle(goal.id, 'active')}
          >
            Resume
          </Button>
        )}
      </div>
    </div>
  );
}
