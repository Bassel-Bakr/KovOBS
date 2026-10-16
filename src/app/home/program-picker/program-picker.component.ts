import { Component, computed, inject, signal } from '@angular/core';
import { toSignal } from '@angular/core/rxjs-interop';
import { catchError, map, of } from 'rxjs';
import { MatButton } from '@angular/material/button';
import { MatDialogActions, MatDialogClose, MatDialogContent, MatDialogTitle } from '@angular/material/dialog';
import { MatFormField, MatInput, MatLabel } from '@angular/material/input';
import { ProcessService, RunningProgram } from '../../services/process.service';

/** The programs whose name or path contains every word of `query`, in order given. */
export function matchPrograms(programs: readonly RunningProgram[], query: string): RunningProgram[] {
  const words = query.toLowerCase().split(/\s+/).filter((word) => word.length > 0);

  return programs.filter((program) => {
    const text = `${program.name} ${program.path}`.toLowerCase();
    return words.every((word) => text.includes(word));
  });
}

/**
 * Lists what is running so a program can be picked instead of browsed for.
 * Closes with the picked path, or with nothing when cancelled.
 */
@Component({
  selector: 'app-program-picker',
  imports: [MatDialogTitle, MatDialogContent, MatDialogActions, MatDialogClose, MatButton, MatFormField, MatLabel, MatInput],
  templateUrl: './program-picker.component.html',
  styleUrl: './program-picker.component.scss',
})
export default class ProgramPickerComponent {
  private readonly programs = toSignal(
    inject(ProcessService)
      .running()
      .pipe(
        map((programs): RunningProgram[] | 'failed' => programs),
        catchError(() => of('failed' as const)),
      ),
  );

  protected readonly query = signal('');
  protected readonly loading = computed(() => this.programs() === undefined);
  protected readonly failed = computed(() => this.programs() === 'failed');
  protected readonly shown = computed(() => {
    const programs = this.programs();
    return Array.isArray(programs) ? matchPrograms(programs, this.query()) : [];
  });
}
