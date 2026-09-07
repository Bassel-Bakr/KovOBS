import { Component, signal } from '@angular/core';
import { TestBed } from '@angular/core/testing';
import { form } from '@angular/forms/signals';
import { of } from 'rxjs';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import AdvancedSettingsComponent from './advanced-settings.component';
import { CacheService } from '../../services/cache.service';
import { Theme } from '../../models/config';

@Component({
  imports: [AdvancedSettingsComponent],
  template: `<app-advanced-settings
    [theme]="model.theme"
    [scanInterval]="model.scan_interval_secs"
    [cacheFile]="model.cache_file"
  />`,
})
class Host {
  readonly model = form(signal({ theme: 'system' as Theme, scan_interval_secs: 3, cache_file: 'cache.json' }));
}

describe('AdvancedSettingsComponent', () => {
  const clearCache = vi.fn(() => of(undefined));

  beforeEach(() => {
    clearCache.mockClear();
    TestBed.configureTestingModule({
      providers: [{ provide: CacheService, useValue: { clearCache } }],
    });
  });

  function render() {
    const fixture = TestBed.createComponent(Host);
    fixture.detectChanges();
    return fixture;
  }

  function themeButtons(fixture: ReturnType<typeof render>): HTMLButtonElement[] {
    return Array.from(fixture.nativeElement.querySelectorAll('.segmented__item'));
  }

  it('offers every theme, with the current one marked', () => {
    const buttons = themeButtons(render());

    expect(buttons.map((b) => b.textContent?.trim())).toEqual(['System', 'Light', 'Dark']);
    expect(buttons[0].classList).toContain('segmented__item--active');
  });

  /** The picker writes back through an input, which is the part that can break. */
  it('writes the chosen theme into the form', async () => {
    const fixture = render();

    themeButtons(fixture)[2].click();
    fixture.detectChanges();
    await fixture.whenStable();

    expect(fixture.componentInstance.model().value().theme).toBe('dark');
    expect(themeButtons(fixture)[2].classList).toContain('segmented__item--active');
  });

  it('clears the cache when asked', () => {
    const fixture = render();
    const buttons: HTMLButtonElement[] = Array.from(fixture.nativeElement.querySelectorAll('button'));

    buttons.find((b) => b.textContent?.trim() === 'Clear cache')!.click();

    expect(clearCache).toHaveBeenCalledOnce();
  });
});
