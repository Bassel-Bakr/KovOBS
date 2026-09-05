import { inject, Service, signal } from '@angular/core';
import { EventService } from './event.service';
import { takeUntilDestroyed } from '@angular/core/rxjs-interop';
import { LogEntry, LogLevel } from '../models/log-entry';

const MAX_ENTRIES = 1000;

/**
 * The backend sends log lines as plain strings, so the level is read back off
 * the emoji `ui_println!` already prefixes them with. Heuristic by nature: a
 * line with no known prefix is treated as info.
 */
const ERROR_PREFIXES = ['❌', '🛑', '💥', '👎', '⚠️'];
const CLIP_PREFIXES = ['🗃️', '✂️', '📸', '🆕'];

function levelOf(message: string): LogLevel {
  if (ERROR_PREFIXES.some((prefix) => message.startsWith(prefix))) {
    return 'error';
  }

  if (CLIP_PREFIXES.some((prefix) => message.startsWith(prefix))) {
    return 'clip';
  }

  return 'info';
}

function timestamp(date: Date): string {
  return date.toTimeString().slice(0, 8);
}

@Service()
export class LogService {
  private readonly eventsService = inject(EventService);
  private readonly internalLogs = signal<LogEntry[]>([]);

  private nextId = 0;

  readonly logs = this.internalLogs.asReadonly();

  constructor() {
    this.eventsService
      .messages()
      .pipe(takeUntilDestroyed())
      .subscribe((message) => {
        const entry: LogEntry = {
          id: this.nextId++,
          time: timestamp(new Date()),
          text: message,
          level: levelOf(message),
        };

        // Replace the array rather than pushing into it: the previous value is
        // still held by anything that read the signal before this update.
        this.internalLogs.update((logs) => [...logs, entry].slice(-MAX_ENTRIES));
      });
  }

  clear(): void {
    this.internalLogs.set([]);
  }
}
