import { Component, input } from '@angular/core';
import { FieldTree, FormField } from '@angular/forms/signals';
import { MatSlideToggle } from '@angular/material/slide-toggle';

/** What KovOBS does on its own while it runs. Toggles only, no state to own. */
@Component({
  selector: 'app-automation-settings',
  imports: [FormField, MatSlideToggle],
  templateUrl: './automation-settings.component.html',
  styleUrl: './automation-settings.component.scss',
})
export default class AutomationSettingsComponent {
  readonly trim = input.required<FieldTree<boolean, string>>();
  readonly autoStart = input.required<FieldTree<boolean, string>>();
  readonly onlyPb = input.required<FieldTree<boolean, string>>();
  readonly deleteAfterTrimming = input.required<FieldTree<boolean, string>>();
  readonly screenshot = input.required<FieldTree<boolean, string>>();
}
