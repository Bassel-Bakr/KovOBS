import { Component, signal } from '@angular/core';
import { TestBed } from '@angular/core/testing';
import { form } from '@angular/forms/signals';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import NotificationSettingsComponent from './notification-settings.component';
import { NotificationService } from '../../services/notification.service';
import { of } from 'rxjs';

/** Hosts the component the way the settings page does: fields off a real form. */
@Component({
  imports: [NotificationSettingsComponent],
  template: `<app-notification-settings
    [enabled]="model.enabled"
    [urgentClips]="model.urgent_clips"
    [failures]="model.failures"
    [sound]="model.sound"
  />`,
})
class Host {
  readonly model = form(signal({ enabled: true, urgent_clips: false, failures: true, sound: true }));
}

describe('NotificationSettingsComponent', () => {
  const sendTest = vi.fn(() => of(undefined));

  beforeEach(() => {
    sendTest.mockClear();
    TestBed.configureTestingModule({
      providers: [{ provide: NotificationService, useValue: { sendTest } }],
    });
  });

  function render() {
    const fixture = TestBed.createComponent(Host);
    fixture.detectChanges();
    return fixture;
  }

  function switches(fixture: ReturnType<typeof render>): HTMLButtonElement[] {
    return Array.from(fixture.nativeElement.querySelectorAll('button[role="switch"]'));
  }

  it('renders a toggle per setting', () => {
    expect(switches(render())).toHaveLength(4);
  });

  it('reflects the values it is given', () => {
    const [enabled, urgent] = switches(render());

    expect(enabled.getAttribute('aria-checked')).toBe('true');
    expect(urgent.getAttribute('aria-checked')).toBe('false');
  });

  /** Urgent clips only modifies the saved-clip toast, so it needs that on. */
  it('disables urgent clips when notifications are off', async () => {
    const fixture = render();
    const [enabled, urgent] = switches(fixture);

    expect(urgent.disabled).toBe(false);

    enabled.click();
    fixture.detectChanges();
    await fixture.whenStable();

    expect(switches(fixture)[1].disabled).toBe(true);
  });

  it('sends a test notification when asked', () => {
    const fixture = render();
    const buttons: HTMLButtonElement[] = Array.from(fixture.nativeElement.querySelectorAll('button'));
    const button = buttons.find((b) => b.textContent?.trim() === 'Send test')!;

    button.click();

    expect(sendTest).toHaveBeenCalledOnce();
  });
});
