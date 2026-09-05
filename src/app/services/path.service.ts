import { inject, Service } from '@angular/core';
import { map, Observable, of } from 'rxjs';
import { TauriService } from './tauri.service';

@Service()
export class PathService {
  private readonly tauriService = inject(TauriService);

  /**
   * Resolves to the subset of `paths` that do not exist on disk. Empty paths are
   * ignored — an unconfigured field is not a broken one.
   */
  missing(paths: string[]): Observable<Set<string>> {
    const candidates = [...new Set(paths.filter((path) => path.trim().length > 0))];

    if (candidates.length === 0) {
      return of(new Set<string>());
    }

    return this.tauriService
      .call<boolean[]>('paths_exist', { paths: candidates })
      .pipe(map((results) => new Set(candidates.filter((_, index) => !results[index]))));
  }
}
