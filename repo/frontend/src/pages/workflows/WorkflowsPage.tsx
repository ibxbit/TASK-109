import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { useForm } from 'react-hook-form';
import { zodResolver } from '@hookform/resolvers/zod';
import { GitMerge, Clock } from 'lucide-react';
import { z } from 'zod';
import * as workflowsApi from '../../services/api/workflows';
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
import { formatDateTime, slaColourClass } from '../../utils/formatters';
import { WORKFLOW_STATUS_LABELS } from '../../utils/constants';
import { useAuthStore } from '../../store/authStore';
import type { WorkflowInstance, Approval, WorkflowStatus } from '../../types';
import { ROLE_IDS } from '../../utils/constants';

const workflowActionSchema = z.object({
  action:      z.enum(['approve', 'reject', 'reassign']),
  approval_id: z.string().uuid('Must be a valid approval ID'),
  comment:     z.string().max(500).optional(),
  reassign_to: z.string().uuid().optional().or(z.literal('')),
});

type WorkflowActionFormValues = z.infer<typeof workflowActionSchema>;

const statusVariant: Record<WorkflowStatus, 'default' | 'info' | 'success' | 'danger'> = {
  pending:   'info',
  approved:  'success',
  rejected:  'danger',
  completed: 'success',
};

// ── Approval Row ──────────────────────────────────────────────────────────────

interface ApprovalRowProps {
  approval:     Approval;
  instanceId:   string;
  canAct:       boolean;
  isAdmin:      boolean;
}

function ApprovalRow({ approval, instanceId, canAct, isAdmin }: ApprovalRowProps) {
  const qc = useQueryClient();
  const [actionOpen, setActionOpen] = useState(false);

  const form = useForm<WorkflowActionFormValues>({
    resolver: zodResolver(workflowActionSchema),
    defaultValues: {
      approval_id: approval.id,
      action:      'approve',
    },
  });

  const mutation = useMutation({
    mutationFn: (values: WorkflowActionFormValues) =>
      workflowsApi.takeWorkflowAction(instanceId, {
        action:      values.action,
        approval_id: values.approval_id,
        comment:     values.comment,
        reassign_to: values.reassign_to || undefined,
      }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['workflow-instance', instanceId] });
      toast.success('Action recorded.');
      setActionOpen(false);
    },
    onError: (err: unknown) => {
      toast.error((err as { message?: string }).message ?? 'Action failed.');
    },
  });

  const actionValue = form.watch('action');

  return (
    <div className="flex items-center justify-between gap-4 py-3 border-b border-slate-100 last:border-0">
      <div className="min-w-0 flex-1 space-y-1">
        <div className="flex items-center gap-2 flex-wrap">
          <span className="text-xs font-mono text-slate-500 truncate">
            Node: {approval.node_id.slice(0, 8)}…
          </span>
          {approval.status === 'pending' ? (
            <Badge variant="info">Pending</Badge>
          ) : approval.status === 'approved' ? (
            <Badge variant="success">Approved</Badge>
          ) : (
            <Badge variant="danger">Rejected</Badge>
          )}
          {approval.sla_breached && (
            <Badge variant="danger">SLA Breached</Badge>
          )}
        </div>
        <p className={`text-xs ${slaColourClass(approval.sla_deadline)}`}>
          SLA: {formatDateTime(approval.sla_deadline)}
        </p>
        {approval.decision_at && (
          <p className="text-xs text-slate-400">
            Decision: {formatDateTime(approval.decision_at)}
          </p>
        )}
      </div>

      {canAct && approval.status === 'pending' && (
        <Button size="sm" variant="secondary" onClick={() => setActionOpen(true)}>
          Take action
        </Button>
      )}

      <Modal
        open={actionOpen}
        onClose={() => setActionOpen(false)}
        title="Approval Action"
        footer={
          <>
            <Button variant="secondary" onClick={() => setActionOpen(false)}>Cancel</Button>
            <Button
              loading={mutation.isPending}
              disabled={mutation.isPending}
              onClick={form.handleSubmit((v) => mutation.mutate(v))}
            >
              Submit
            </Button>
          </>
        }
      >
        <form className="space-y-4" noValidate>
          <Select
            label="Action"
            required
            options={[
              { value: 'approve',  label: 'Approve'  },
              { value: 'reject',   label: 'Reject'   },
              ...(isAdmin ? [{ value: 'reassign', label: 'Reassign' }] : []),
            ]}
            error={form.formState.errors.action?.message}
            {...form.register('action')}
          />
          <Textarea
            label="Comment (optional)"
            error={form.formState.errors.comment?.message}
            {...form.register('comment')}
          />
          {actionValue === 'reassign' && (
            <Input
              label="Reassign to (UUID)"
              required
              placeholder="Staff UUID"
              error={form.formState.errors.reassign_to?.message}
              {...form.register('reassign_to')}
            />
          )}
        </form>
      </Modal>
    </div>
  );
}

// ── Instance Card ─────────────────────────────────────────────────────────────

interface InstanceCardProps {
  instance: WorkflowInstance;
  canAct:   boolean;
  isAdmin:  boolean;
}

function InstanceCard({ instance, canAct, isAdmin }: InstanceCardProps) {
  const [expanded, setExpanded] = useState(false);

  const pendingCount = instance.approvals.filter((a) => a.status === 'pending').length;
  const slaBreached  = instance.approvals.some((a) => a.sla_breached);

  return (
    <div className="bg-white rounded-lg border border-slate-200 shadow-sm p-5 space-y-3 hover:border-slate-300 transition-colors">
      <div className="flex items-start justify-between gap-2">
        <div>
          <div className="flex items-center gap-2">
            <Badge variant={statusVariant[instance.status]}>
              {WORKFLOW_STATUS_LABELS[instance.status]}
            </Badge>
            {slaBreached && (
              <Badge variant="danger">SLA Breached</Badge>
            )}
          </div>
          <p className="text-xs font-mono text-slate-400 mt-1">
            {instance.id.slice(0, 8)}…
          </p>
        </div>
        <div className="text-right text-xs text-slate-400">
          <p>Stage {instance.current_stage}</p>
          <p>{formatDateTime(instance.created_at)}</p>
        </div>
      </div>

      {pendingCount > 0 && (
        <div className="flex items-center gap-1.5 text-xs text-amber-600">
          <Clock size={13} />
          {pendingCount} pending approval{pendingCount > 1 ? 's' : ''}
        </div>
      )}

      <Button
        size="sm"
        variant="ghost"
        className="w-full"
        onClick={() => setExpanded((v) => !v)}
      >
        {expanded ? 'Hide' : 'Show'} approvals ({instance.approvals.length})
      </Button>

      {expanded && instance.approvals.length > 0 && (
        <div>
          {instance.approvals.map((approval) => (
            <ApprovalRow
              key={approval.id}
              approval={approval}
              instanceId={instance.id}
              canAct={canAct}
              isAdmin={isAdmin}
            />
          ))}
        </div>
      )}

      {expanded && instance.approvals.length === 0 && (
        <p className="text-xs text-slate-400 text-center py-3">
          No approval tasks yet.
        </p>
      )}
    </div>
  );
}

// ── Workflows Page ────────────────────────────────────────────────────────────

export function WorkflowsPage() {
  const user    = useAuthStore((s) => s.user);
  const isAdmin = user?.role_id === ROLE_IDS.ADMINISTRATOR;
  const canAct  =
    user?.role_id === ROLE_IDS.ADMINISTRATOR ||
    user?.role_id === ROLE_IDS.APPROVER;

  const [lookupId, setLookupId]     = useState('');
  const [instanceId, setInstanceId] = useState<string | null>(null);

  const instanceQuery = useQuery({
    queryKey: ['workflow-instance', instanceId],
    queryFn:  () => workflowsApi.getWorkflowInstance(instanceId!),
    enabled:  !!instanceId,
    retry: false,
  });

  const [startOpen, setStartOpen] = useState(false);

  const startForm = useForm({
    defaultValues: { template_id: '' },
    resolver: zodResolver(z.object({ template_id: z.string().uuid('Must be a valid template UUID') })),
  });

  const startMutation = useMutation({
    mutationFn: (values: { template_id: string }) =>
      workflowsApi.startWorkflowInstance(values),
    onSuccess: (data) => {
      toast.success('Workflow instance started.');
      setInstanceId(data.id);
      setStartOpen(false);
      startForm.reset();
    },
    onError: (err: unknown) => {
      toast.error((err as { message?: string }).message ?? 'Failed to start workflow.');
    },
  });

  return (
    <div className="p-6 max-w-4xl mx-auto space-y-5">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-xl font-bold text-slate-900">Workflow Approvals</h1>
          <p className="text-sm text-slate-500 mt-0.5">
            Manage approval workflows with SLA tracking.
          </p>
        </div>
        {isAdmin && (
          <Button onClick={() => setStartOpen(true)}>Start workflow</Button>
        )}
      </div>

      {/* Instance lookup */}
      <Card title="Look up workflow instance">
        <div className="flex gap-3">
          <Input
            placeholder="Workflow instance UUID…"
            value={lookupId}
            onChange={(e) => setLookupId(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && setInstanceId(lookupId.trim())}
            className="flex-1"
          />
          <Button onClick={() => setInstanceId(lookupId.trim())}>
            Load
          </Button>
        </div>
      </Card>

      {/* Instance detail */}
      {instanceId && (
        <div>
          {instanceQuery.isLoading && (
            <div className="flex justify-center py-10"><Spinner /></div>
          )}
          {instanceQuery.isError && (
            <p className="text-sm text-red-600">
              {(instanceQuery.error as { message?: string }).message ?? 'Failed to load instance.'}
            </p>
          )}
          {instanceQuery.data && (
            <InstanceCard
              instance={instanceQuery.data}
              canAct={canAct}
              isAdmin={isAdmin}
            />
          )}
        </div>
      )}

      {!instanceId && (
        <EmptyState
          icon={<GitMerge size={48} />}
          title="No instance selected"
          description="Enter a workflow instance ID above to view its approval tasks."
        />
      )}

      {/* Start Workflow Modal */}
      <Modal
        open={startOpen}
        onClose={() => setStartOpen(false)}
        title="Start Workflow Instance"
        footer={
          <>
            <Button variant="secondary" onClick={() => setStartOpen(false)}>Cancel</Button>
            <Button
              loading={startMutation.isPending}
              disabled={startMutation.isPending}
              onClick={startForm.handleSubmit((v) => startMutation.mutate(v))}
            >
              Start
            </Button>
          </>
        }
      >
        <form noValidate>
          <Input
            label="Template ID"
            required
            placeholder="UUID of the workflow template"
            error={startForm.formState.errors.template_id?.message}
            {...startForm.register('template_id')}
          />
        </form>
      </Modal>
    </div>
  );
}
