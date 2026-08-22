import { create } from 'zustand';

export type TaskKind =
  | 'download'
  | 'organize'
  | 'search'
  | 'search_archive'
  | 'audit'
  | 'audit_fix'
  | 'version_audit'
  | 'mismatch_audit'
  | 'fix_mismatch';

export type TaskStatus = 'running' | 'done' | 'cancelled' | 'error';

export type TaskCandidate = {
  id: string;
  name: string;
  price: number;
};

export type TaskItem = {
  id: string;
  message: string;
  status: string;
  path?: string;
  price?: number;
  source?: string;
  candidates?: TaskCandidate[];
  picked?: string;
  ambiguous?: boolean;
};

export type TaskRecord = {
  kind: TaskKind;
  label: string;
  status: TaskStatus;
  total: number;
  done: number;
  failed: number;
  updateable: number;
  items: TaskItem[];
  logs: string[];
  startedAt: number;
  cmd: string;
  args: Record<string, unknown>;
};

export type ProgressEvt = {
  type: string;
  total?: number;
  done?: number;
  failed?: number;
  updateable?: number;
  id?: string;
  message?: string;
  status?: string;
  line?: string;
  path?: string;
  price?: number;
  source?: string;
  candidates?: TaskCandidate[];
  picked?: string | null;
  ambiguous?: boolean;
};

type TaskState = {
  tasks: Record<string, TaskRecord>;
  latestByKind: Partial<Record<TaskKind, string>>;
  begin: (taskId: string, init: Pick<TaskRecord, 'kind' | 'label' | 'cmd' | 'args'>) => void;
  applyEvent: (taskId: string, evt: ProgressEvt) => void;
};

export const useTaskStore = create<TaskState>((set) => ({
  tasks: {},
  latestByKind: {},

  begin: (taskId, init) =>
    set((s) => {
      const latestByKind = { ...s.latestByKind, [init.kind]: taskId };
      return {
        tasks: {
          ...pruneTasks(s.tasks, latestByKind, taskId),
          [taskId]: {
            kind: init.kind,
            label: init.label,
            status: 'running',
            total: 0,
            done: 0,
            failed: 0,
            updateable: 0,
            items: [],
            logs: [],
            startedAt: Date.now(),
            cmd: init.cmd,
            args: stripCookie(init.args),
          },
        },
        latestByKind,
      };
    }),

  applyEvent: (taskId, evt) =>
    set((s) => {
      const t = s.tasks[taskId];
      if (!t) return s;
      const next: TaskRecord = { ...t, items: t.items, logs: t.logs };
      switch (evt.type) {
        case 'taskStarted':
          next.total = evt.total ?? next.total;
          break;
        case 'progress':
          next.done = evt.done ?? next.done;
          next.total = evt.total ?? next.total;
          break;
        case 'itemDone':
          next.items = capItems([
            ...next.items,
            {
              id: String(evt.id ?? ''),
              message: String(evt.message ?? ''),
              status: String(evt.status ?? 'ok'),
              path: evt.path,
              price: evt.price,
            },
          ]);
          break;
        case 'candidates':
          next.items = capItems([
            ...next.items,
            {
              id: String(evt.picked ?? ''),
              message: '',
              status: evt.ambiguous ? 'ambiguous' : evt.picked ? 'ok' : 'err',
              source: evt.source,
              path: evt.source,
              candidates: evt.candidates,
              picked: evt.picked ?? undefined,
              ambiguous: evt.ambiguous,
            },
          ]);
          break;
        case 'itemError':
          next.items = capItems([
            ...next.items,
            {
              id: String(evt.id ?? ''),
              message: String(evt.message ?? ''),
              status: 'err',
            },
          ]);
          break;
        case 'finished':
          next.status = 'done';
          next.done = Math.max(next.done, evt.done ?? 0);
          next.failed = Math.max(
            next.failed,
            evt.failed ?? 0,
            next.items.filter((i) => i.status === 'err').length,
          );
          next.updateable = evt.updateable ?? next.updateable;
          break;
        case 'cancelled':
          next.status = 'cancelled';
          break;
        case 'log':
          next.logs = capped([...next.logs, String(evt.line ?? '')], LOG_CAP);
          break;
        default:
          break;
      }
      return { tasks: { ...s.tasks, [taskId]: next } };
    }),
}));

export function useLatestTask(kind: TaskKind): { id: string; task: TaskRecord } | undefined {
  const id = useTaskStore((s) => s.latestByKind[kind]);
  const task = useTaskStore((s) => (id ? s.tasks[id] : undefined));
  if (!id || !task) return undefined;
  return { id, task };
}

export function failedItems(task: TaskRecord): TaskItem[] {
  return task.items.filter((i) => i.status === 'err');
}

function keepTask(
  id: string,
  t: TaskRecord,
  latestByKind: Partial<Record<TaskKind, string>>,
): boolean {
  if (t.status === 'running') return true;
  if (t.status === 'done' && t.failed > 0) return true;
  return latestByKind[t.kind] === id;
}

function pruneTasks(
  tasks: Record<string, TaskRecord>,
  latestByKind: Partial<Record<TaskKind, string>>,
  keepId: string,
): Record<string, TaskRecord> {
  const next: Record<string, TaskRecord> = {};
  for (const [id, t] of Object.entries(tasks)) {
    if (id === keepId || keepTask(id, t, latestByKind)) next[id] = t;
  }
  return next;
}

const ITEM_CAP = 500;
const LOG_CAP = 200;

function capped<T>(xs: T[], cap: number): T[] {
  return xs.length > cap ? xs.slice(-cap) : xs;
}

function capItems(xs: TaskItem[]): TaskItem[] {
  if (xs.length <= ITEM_CAP) return xs;
  const errs = xs.filter((i) => i.status === 'err');
  const keepErr = errs.length > ITEM_CAP ? errs.slice(-ITEM_CAP) : errs;
  const keepErrSet = new Set(keepErr);
  const room = ITEM_CAP - keepErr.length;
  const others = xs.filter((i) => !keepErrSet.has(i)).slice(-room);
  const keep = new Set([...keepErr, ...others]);
  return xs.filter((i) => keep.has(i));
}

function stripCookie(args: Record<string, unknown>): Record<string, unknown> {
  if (!('cookie' in args)) return args;
  const next = { ...args };
  delete next.cookie;
  return next;
}
