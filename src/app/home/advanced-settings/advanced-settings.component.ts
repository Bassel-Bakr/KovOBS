import { Component, inject, input } from '@angular/core';
import { FieldTree, FormField } from '@angular/forms/signals';
import { MatFormField, MatHint, MatInput, MatLabel } from '@angular/material/input';
import { MatButton } from '@angular/material/button';
import { CacheService } from '../../services/cache.service';
import { Theme } from '../../models/config';

/** Appearance, scan interval, cache -- the things rarely touched. */
@Component({
  selector: 'app-advanced-settings',
  imports: [FormField, MatFormField, MatLabel, MatHint, MatInput, MatButton],
  templateUrl: './advanced-settings.component.html',
  styleUrl: './advanced-settings.component.scss',
})
export default class AdvancedSettingsComponent {
  private readonly cacheService = inject(CacheService);

  readonly theme = input.required<FieldTree<Theme, string>>();
  readonly scanInterval = input.required<FieldTree<number, string>>();
  readonly cacheFile = input.required<FieldTree<string, string>>();

  protected readonly themes: { id: Theme; label: string }[] = [
    { id: 'system', label: 'System' },
    { id: 'light', label: 'Light' },
    { id: 'dark', label: 'Dark' },
  ];

  protected setTheme(theme: Theme): void {
    this.theme()().value.set(theme);
  }

  protected clearCache(): void {
    this.cacheService.clearCache().subscribe();
  }
}
