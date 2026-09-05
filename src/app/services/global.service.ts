import { Service, signal } from '@angular/core';

@Service()
export class GlobalService {
  readonly showLogs = signal(false);

  /** Lives here rather than in the panel, which is destroyed when hidden and
   * would otherwise reset a dragged height on every toggle. */
  readonly logsHeight = signal(226);
}
