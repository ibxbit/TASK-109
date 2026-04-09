import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { useForm } from 'react-hook-form';
import { zodResolver } from '@hookform/resolvers/zod';
import { Plus, ClipboardList, ArrowRight } from 'lucide-react';
import * as workOrdersApi from '../../services/api/workOrders';
import { Button } from '../../components/ui/Button';
import { Input } from '../../components/ui/Input';
import { Select } from '../../components/ui/Select';
import { Textarea } from '../../components/ui/Textarea';
import { Modal } from '../../components/ui/Modal';
import { Badge } from '../../components/ui/Badge';
import { EmptyState } from '../../components/ui/EmptyState';
import { Spinner } from '../../components/ui/Spinner';
import { toast } from '../../components/ui/Toast';
import {
  createWorkOrderSchema,
  transitionWorkOrderSchema,
  type CreateWorkOrderFormValues,
  type TransitionWorkOrderFormValues,
} from '../../utils/validators';
import { formatDate } from '../../utils/formatters';
import {
  WORK_ORDER_STATUS_LABELS,
  PRIORITY_CLASSES,
  ROLE_IDS,
} from '../../utils/constants';
import { useAuthStore } from '../../store/authStore';
import type {
  WorkOrder,
  WorkOrderStatus,
  WorkOrderPriority,
} from '../../types';
import { WORK_ORDER_STATUS_FLOW } from '../../types';

const STATUS_COLUMNS: WorkOrderStatus[] = [
  'intake',
  'triage',
  'in_progress',
  'waiting_on_member',
  'resolved',
  'closed',
];

const TICKET_TYPE_OPTIONS = [
  { value: 'health_query', label: 'Health Query' },
  { value: 'equipment',    label: 'Equipment'    },
  { value: 'scheduling',   label: 'Scheduling'   },
  { value: 'nutrition',    label: 'Nutrition'    },
  { value: 'emergency',    label: 'Emergency'    },
];

const PRIORITY_OPTIONS = [
  { value: 'low',    label: 'Low'    },
  { value: 'medium', label: 'Medium' },
  { value: 'high',   label: 'High'   },
  { value: 'urgent', label: 'Urgent' },
];

const statusVariant: Record<WorkOrderStatus, 'default' | 'info' | 'warning' | 'success' | 'danger'> = {
  intake:            'default',
  triage:            'info',
  in_progress:       'info',
  waiting_on_member: 'warning',
  resolved:          'success',
  closed:            'default',
};

// ── Transition Modal ──────────────────────────────────────────────────────────

interface TransitionModalProps {
  workOrder: WorkOrder | null;
  onClose:   () => void;
}

function TransitionModal({ workOrder, onClose }: TransitionModalProps) {
  const qc = useQueryClient();
  const form = useForm<TransitionWorkOrderFormValues>({
    resolver: zodResolver(transitionWorkOrderSchema),
  });

  const mutation = useMutation({
    mutationFn: (values: TransitionWorkOrderFormValues) =>
      workOrdersApi.transitionWorkOrder(workOrder!.id, {
        new_status:       values.new_status,
        processing_notes: values.processing_notes,
        assigned_to:      values.assigned_to || undefined,
      }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['work-orders'] });
      toast.success('Work order updated.');
      onClose();
    },
    onError: (err: unknown) => {
      toast.error((err as { message?: string }).message ?? 'Transition failed.');
    },
  });

  if (!workOrder) return null;

  const currentIdx = WORK_ORDER_STATUS_FLOW.indexOf(workOrder.status);
  const nextStatuses = WORK_ORDER_STATUS_FLOW.slice(currentIdx + 1);

  const nextOptions = nextStatuses.map((s) => ({
    value: s,
    label: WORK_ORDER_STATUS_LABELS[s] ?? s,
  }));

  return (
    <Modal
      open={!!workOrder}
      onClose={onClose}
      title={`Transition: ${workOrder.title}`}
      footer={
        <>
          <Button variant="secondary" onClick={onClose}>Cancel</Button>
          <Button
            loading={mutation.isPending}
            disabled={mutation.isPending}
            onClick={form.handleSubmit((v) => mutation.mutate(v))}
          >
            Apply transition
          </Button>
        </>
      }
    >
      <form className="space-y-4" noValidate>
        <div className="flex items-center gap-2 text-sm text-slate-600">
          <Badge variant={statusVariant[workOrder.status]}>
            {WORK_ORDER_STATUS_LABELS[workOrder.status]}
          </Badge>
          <ArrowRight size={14} />
          <span className="text-slate-400">select new status</span>
        </div>

        <Select
          label="New status"
          required
          options={nextOptions}
          placeholder="Select next status…"
          error={form.formState.errors.new_status?.message}
          {...form.register('new_status')}
        />
        <Textarea
          label="Processing notes"
          hint="Notes are appended to the existing history"
          error={form.formState.errors.processing_notes?.message}
          {...form.register('processing_notes')}
        />
        <Input
          label="Reassign to (UUID, optional)"
          placeholder="Staff member UUID"
          error={form.formState.errors.assigned_to?.message}
          {...form.register('assigned_to')}
        />
      </form>
    </Modal>
  );
}

// ── Work Orders Page ──────────────────────────────────────────────────────────

export function WorkOrdersPage() {
  const qc = useQueryClient();
  const user = useAuthStore((s) => s.user);
  const isAdminOrCoach =
    user?.role_id === ROLE_IDS.ADMINISTRATOR ||
    user?.role_id === ROLE_IDS.CARE_COACH;

  const [createOpen, setCreateOpen]       = useState(false);
  const [transitioning, setTransitioning] = useState<WorkOrder | null>(null);
  const [statusFilter, setStatusFilter]   = useState<WorkOrderStatus | ''>('');

  const query = useQuery({
    queryKey: ['work-orders', statusFilter],
    queryFn:  () =>
      workOrdersApi.getWorkOrders({
        status: statusFilter || undefined,
      }),
  });

  const createForm = useForm<CreateWorkOrderFormValues>({
    resolver: zodResolver(createWorkOrderSchema),
    defaultValues: { priority: 'medium' },
  });

  const createMutation = useMutation({
    mutationFn: (values: CreateWorkOrderFormValues) =>
      workOrdersApi.createWorkOrder({
        ...values,
        due_date: values.due_date || undefined,
      }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['work-orders'] });
      toast.success('Work order created.');
      setCreateOpen(false);
      createForm.reset({ priority: 'medium' });
    },
    onError: (err: unknown) => {
      toast.error((err as { message?: string }).message ?? 'Failed to create.');
    },
  });

  const workOrders = query.data ?? [];

  const STATUS_FILTER_OPTIONS = [
    { value: '', label: 'All statuses' },
    ...STATUS_COLUMNS.map((s) => ({
      value: s,
      label: WORK_ORDER_STATUS_LABELS[s],
    })),
  ];

  return (
    <div className="p-6 max-w-7xl mx-auto space-y-5">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-xl font-bold text-slate-900">Work Orders</h1>
          <p className="text-sm text-slate-500 mt-0.5">
            Manage member support tickets and care requests.
          </p>
        </div>
        {isAdminOrCoach && (
          <Button leftIcon={<Plus size={15} />} onClick={() => setCreateOpen(true)}>
            New work order
          </Button>
        )}
      </div>

      {/* Filters */}
      <Select
        options={STATUS_FILTER_OPTIONS}
        value={statusFilter}
        onChange={(e) => setStatusFilter(e.target.value as WorkOrderStatus | '')}
        className="w-52"
      />

      {/* Board */}
      {query.isLoading ? (
        <div className="flex justify-center py-16"><Spinner /></div>
      ) : query.isError ? (
        <p className="text-sm text-red-600">
          {(query.error as { message?: string }).message ?? 'Failed to load work orders.'}
        </p>
      ) : workOrders.length === 0 ? (
        <EmptyState
          icon={<ClipboardList size={48} />}
          title="No work orders found"
          description="Work orders appear here once created."
          action={
            isAdminOrCoach && (
              <Button size="sm" onClick={() => setCreateOpen(true)}>
                Create first work order
              </Button>
            )
          }
        />
      ) : (
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
          {workOrders.map((wo: WorkOrder) => (
            <WorkOrderCard
              key={wo.id}
              workOrder={wo}
              canTransition={isAdminOrCoach}
              onTransition={() => setTransitioning(wo)}
            />
          ))}
        </div>
      )}

      {/* Create Modal */}
      <Modal
        open={createOpen}
        onClose={() => setCreateOpen(false)}
        title="Create Work Order"
        size="lg"
        footer={
          <>
            <Button variant="secondary" onClick={() => setCreateOpen(false)}>Cancel</Button>
            <Button
              loading={createMutation.isPending}
              disabled={createMutation.isPending}
              onClick={createForm.handleSubmit((v) => createMutation.mutate(v))}
            >
              Create
            </Button>
          </>
        }
      >
        <form className="space-y-4" noValidate>
          <Input
            label="Member ID"
            required
            placeholder="UUID"
            error={createForm.formState.errors.member_id?.message}
            {...createForm.register('member_id')}
          />
          <Input
            label="Title"
            required
            error={createForm.formState.errors.title?.message}
            {...createForm.register('title')}
          />
          <div className="grid grid-cols-2 gap-4">
            <Select
              label="Ticket type"
              required
              options={TICKET_TYPE_OPTIONS}
              placeholder="Select…"
              error={createForm.formState.errors.ticket_type?.message}
              {...createForm.register('ticket_type')}
            />
            <Select
              label="Priority"
              options={PRIORITY_OPTIONS}
              error={createForm.formState.errors.priority?.message}
              {...createForm.register('priority')}
            />
            <Input
              label="Due date"
              type="date"
              error={createForm.formState.errors.due_date?.message}
              {...createForm.register('due_date')}
            />
          </div>
          <Textarea
            label="Description"
            error={createForm.formState.errors.description?.message}
            {...createForm.register('description')}
          />
        </form>
      </Modal>

      {/* Transition Modal */}
      <TransitionModal
        workOrder={transitioning}
        onClose={() => setTransitioning(null)}
      />
    </div>
  );
}

// ── Work Order Card ───────────────────────────────────────────────────────────

interface WOCardProps {
  workOrder:     WorkOrder;
  canTransition: boolean;
  onTransition:  () => void;
}

function WorkOrderCard({ workOrder: wo, canTransition, onTransition }: WOCardProps) {
  return (
    <div className="bg-white rounded-lg border border-slate-200 shadow-sm p-4 space-y-3 hover:border-slate-300 transition-colors">
      <div className="flex items-start justify-between gap-2">
        <h3 className="text-sm font-semibold text-slate-800 leading-snug line-clamp-2">
          {wo.title}
        </h3>
        <span
          className={`shrink-0 text-xs font-medium px-2 py-0.5 rounded-full ${PRIORITY_CLASSES[wo.priority as WorkOrderPriority]}`}
        >
          {wo.priority}
        </span>
      </div>

      <div className="flex items-center gap-2 flex-wrap">
        <Badge variant={statusVariant[wo.status]}>
          {WORK_ORDER_STATUS_LABELS[wo.status]}
        </Badge>
        <Badge variant="default">{wo.ticket_type}</Badge>
      </div>

      {wo.description && (
        <p className="text-xs text-slate-500 line-clamp-2">{wo.description}</p>
      )}

      <div className="text-xs text-slate-400 space-y-0.5">
        {wo.due_date && <p>Due: {formatDate(wo.due_date)}</p>}
        <p>Opened: {formatDate(wo.created_at)}</p>
      </div>

      {canTransition && wo.status !== 'closed' && (
        <Button size="sm" variant="secondary" className="w-full" onClick={onTransition}>
          Transition status
        </Button>
      )}
    </div>
  );
}
