import { Component, input } from '@angular/core';
import { FieldTree, FormField } from '@angular/forms/signals';
import { MatFormField, MatHint, MatInput, MatLabel, MatSuffix } from '@angular/material/input';
import { MatIconButton } from '@angular/material/button';
import { MatIcon } from '@angular/material/icon';
import { browseFile } from '../../browse';

/** How KovOBS reaches OBS, and where its executable lives. */
@Component({
  selector: 'app-obs-settings',
  imports: [FormField, MatFormField, MatLabel, MatHint, MatInput, MatSuffix, MatIconButton, MatIcon],
  templateUrl: './obs-settings.component.html',
  styleUrl: './obs-settings.component.scss',
})
export default class ObsSettingsComponent {
  readonly host = input.required<FieldTree<string, string>>();
  readonly port = input.required<FieldTree<number, string>>();
  readonly password = input.required<FieldTree<string, string>>();
  readonly executable = input.required<FieldTree<string, string>>();

  /** Names of sources OBS reported, or null before it has been asked. */
  readonly sources = input<string[] | null>(null);
  readonly missing = input<ReadonlySet<string>>(new Set());

  protected readonly browseFile = browseFile;

  protected isMissing(path: string): boolean {
    return this.missing().has(path);
  }
}
