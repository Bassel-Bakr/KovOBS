import { Component, input } from '@angular/core';
import { FieldTree, FormField } from '@angular/forms/signals';
import { open } from '@tauri-apps/plugin-dialog';
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

  protected isMissing(path: string): boolean {
    return this.missing().has(path);
  }

  protected async browseFolder(field: FieldTree<string, string>): Promise<void> {
    const folder = await open({ directory: true, multiple: false });

    if (folder != null) {
      field().value.set(folder);
    }
  }

  protected async browseFile(field: FieldTree<string, string>): Promise<void> {
    const file = await open({ multiple: false });

    if (file != null) {
      field().value.set(file);
    }
  }
}
