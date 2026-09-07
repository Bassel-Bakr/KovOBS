import { Component, signal } from '@angular/core';
import { TestBed } from '@angular/core/testing';
import { form } from '@angular/forms/signals';
import { describe, expect, it } from 'vitest';
import ObsSettingsComponent from './obs-settings.component';

@Component({
  imports: [ObsSettingsComponent],
  template: `<app-obs-settings
    [host]="model.host"
    [port]="model.port"
    [password]="model.password"
    [executable]="model.executable"
    [sources]="sources()"
    [missing]="missing()"
  />`,
})
class Host {
  readonly model = form(signal({ host: 'localhost', port: 4455, password: '', executable: 'C:/obs.exe' }));

  readonly sources = signal<string[] | null>(null);
  readonly missing = signal<ReadonlySet<string>>(new Set());
}

describe('ObsSettingsComponent', () => {
  function render() {
    const fixture = TestBed.createComponent(Host);
    fixture.detectChanges();
    return fixture;
  }

  function text(fixture: ReturnType<typeof render>): string {
    return fixture.nativeElement.textContent ?? '';
  }

  it('shows the values it is given', () => {
    const inputs: HTMLInputElement[] = Array.from(render().nativeElement.querySelectorAll('input'));

    expect(inputs.map((i) => i.value)).toContain('localhost');
    expect(inputs.map((i) => i.value)).toContain('4455');
  });

  /** Nothing is shown until OBS has actually answered. */
  it('stays quiet about sources until some are found', async () => {
    const fixture = render();

    expect(text(fixture)).not.toContain('sources found');

    fixture.componentInstance.sources.set(['Desktop', 'Game']);
    fixture.detectChanges();
    await fixture.whenStable();

    expect(text(fixture)).toContain('2 sources found');
  });

  /** A path that is not there should be called out, not silently accepted. */
  it('flags an executable that does not exist', async () => {
    const fixture = render();

    expect(fixture.nativeElement.querySelector('.hint--error')).toBeNull();

    fixture.componentInstance.missing.set(new Set(['C:/obs.exe']));
    fixture.detectChanges();
    await fixture.whenStable();

    expect(fixture.nativeElement.querySelector('.hint--error').textContent).toContain('Cannot find this file');
  });
});
