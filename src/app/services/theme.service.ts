import { Service } from '@angular/core';
import { Theme } from '../models/config';

/**
 * Angular Material emits its tokens as `light-dark(<light>, <dark>)`, which
 * resolves against the element's `color-scheme`. Setting that one property on
 * the root switches every token at once, with no stylesheet swap and no flash.
 */
@Service()
export class ThemeService {
  apply(theme: Theme): void {
    document.documentElement.style.colorScheme = theme === 'system' ? 'light dark' : theme;
  }
}
