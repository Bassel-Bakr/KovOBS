import { Component, inject, input } from '@angular/core';
import { FieldTree, FormField } from '@angular/forms/signals';
import { MatDialog } from '@angular/material/dialog';
import { browseFolder, browseFile, browseRunning } from '../../browse';
import { MatFormField, MatHint, MatInput, MatLabel, MatSuffix } from '@angular/material/input';
import { MatIconButton } from '@angular/material/button';
import { MatIcon } from '@angular/material/icon';
import { MatOption, MatSelect } from '@angular/material/select';

/**
 * The per-game settings pane. KovaaK's and Aimbeast differ only in which fields
 * they point at, so both sections render this.
 */
@Component({
  selector: 'app-game-settings',
  imports: [
    FormField,
    MatFormField,
    MatLabel,
    MatHint,
    MatInput,
    MatSuffix,
    MatIconButton,
    MatIcon,
    MatSelect,
    MatOption,
  ],
  templateUrl: './game-settings.component.html',
  styleUrl: './game-settings.component.scss',
})
export default class GameSettingsComponent {
  readonly statsFolder = input.required<FieldTree<string, string>>();
  readonly clipsFolder = input.required<FieldTree<string, string>>();
  readonly sourceName = input.required<FieldTree<string, string>>();
  readonly executable = input.required<FieldTree<string, string>>();

  readonly sources = input<string[] | null>(null);
  readonly missing = input<ReadonlySet<string>>(new Set());

  private readonly dialog = inject(MatDialog);

  protected readonly browseFolder = browseFolder;
  protected readonly browseFile = browseFile;

  protected browseRunning(field: FieldTree<string, string>): Promise<void> {
    return browseRunning(this.dialog, field);
  }

  protected isMissing(path: string): boolean {
    return this.missing().has(path);
  }
}
