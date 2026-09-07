import { TestBed } from '@angular/core/testing';
import { of, throwError } from 'rxjs';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import AboutSettingsComponent from './about-settings.component';
import { UpdateService } from '../../services/update.service';
import { TauriService } from '../../services/tauri.service';

describe('AboutSettingsComponent', () => {
  const about = vi.fn(() => of({ version: '1.2.3', releases_url: 'https://example/releases' }));
  const check = vi.fn();
  const quit = vi.fn(() => of(undefined));

  beforeEach(() => {
    about.mockClear();
    check.mockReset();
    quit.mockClear();

    TestBed.configureTestingModule({
      providers: [
        { provide: UpdateService, useValue: { about, check } },
        { provide: TauriService, useValue: { quit } },
      ],
    });
  });

  async function render() {
    const fixture = TestBed.createComponent(AboutSettingsComponent);
    fixture.detectChanges();
    await fixture.whenStable();
    fixture.detectChanges();
    return fixture;
  }

  function click(fixture: Awaited<ReturnType<typeof render>>, label: string) {
    const buttons: HTMLButtonElement[] = Array.from(fixture.nativeElement.querySelectorAll('button'));
    buttons.find((b) => b.textContent?.trim().startsWith(label))!.click();
  }

  it('shows the running version', async () => {
    const fixture = await render();

    expect(fixture.nativeElement.querySelector('.about__number').textContent).toContain('1.2.3');
  });

  /** Checking is manual: nothing should reach the network on render. */
  it('does not check for updates on its own', async () => {
    await render();

    expect(check).not.toHaveBeenCalled();
  });

  it('announces a newer release when one is found', async () => {
    check.mockReturnValue(of({ current: '1.2.3', latest: '1.3.0', release_url: 'u', update_available: true }));
    const fixture = await render();

    click(fixture, 'Check for updates');
    fixture.detectChanges();
    await fixture.whenStable();
    fixture.detectChanges();

    expect(fixture.nativeElement.querySelector('.banner').textContent).toContain('1.3.0');
  });

  /** A failed check must say so rather than look like "up to date". */
  it('reports a failed check', async () => {
    check.mockReturnValue(throwError(() => 'network down'));
    const fixture = await render();

    click(fixture, 'Check for updates');
    fixture.detectChanges();
    await fixture.whenStable();
    fixture.detectChanges();

    const note = fixture.nativeElement.querySelector('.about__note--error');
    expect(note.textContent).toContain('network down');
    expect(fixture.nativeElement.querySelector('.banner')).toBeNull();
  });

  it('quits when asked', async () => {
    const fixture = await render();

    click(fixture, 'Quit');

    expect(quit).toHaveBeenCalledOnce();
  });
});
