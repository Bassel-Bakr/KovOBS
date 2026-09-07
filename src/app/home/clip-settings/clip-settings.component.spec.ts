import { Component, signal } from '@angular/core';
import { TestBed } from '@angular/core/testing';
import { form } from '@angular/forms/signals';
import { of } from 'rxjs';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import ClipSettingsComponent from './clip-settings.component';
import { FfmpegService } from '../../services/ffmpeg.service';

@Component({
  imports: [ClipSettingsComponent],
  template: `<app-clip-settings
    [paddingStart]="model.trim_padding_start"
    [paddingEnd]="model.trim_padding_end"
    [globalArgs]="model.global_args"
    [inputArgs]="model.input_args"
    [outputArgs]="model.output_args"
    [downloadProgress]="progress()"
  />`,
})
class Host {
  readonly model = form(
    signal({
      trim_padding_start: 0,
      trim_padding_end: 5,
      global_args: '',
      input_args: '',
      output_args: '',
    })
  );

  readonly progress = signal({ state: 'Done', progress: 100 });
}

describe('ClipSettingsComponent', () => {
  const download = vi.fn(() => of(undefined));
  const remove = vi.fn(() => of(undefined));

  beforeEach(() => {
    download.mockClear();
    remove.mockClear();
    TestBed.configureTestingModule({
      providers: [{ provide: FfmpegService, useValue: { download, remove } }],
    });
  });

  function render() {
    const fixture = TestBed.createComponent(Host);
    fixture.detectChanges();
    return fixture;
  }

  function labels(fixture: ReturnType<typeof render>): string[] {
    return Array.from<HTMLButtonElement>(fixture.nativeElement.querySelectorAll('button')).map(
      (b) => b.textContent?.trim() ?? ''
    );
  }

  /** The args are the advanced case; the panel stays shut until asked for. */
  it('keeps the post-processing panel collapsed', () => {
    const fixture = render();

    expect(fixture.nativeElement.querySelector('.panel__body')).toBeNull();

    fixture.nativeElement.querySelector('.panel__head').click();
    fixture.detectChanges();

    expect(fixture.nativeElement.querySelector('.panel__body')).not.toBeNull();
  });

  it('offers to delete FFmpeg once it is present', () => {
    const fixture = render();
    fixture.nativeElement.querySelector('.panel__head').click();
    fixture.detectChanges();

    expect(labels(fixture).some((l) => l.includes('Delete FFmpeg'))).toBe(true);
    expect(labels(fixture).some((l) => l.includes('Download FFmpeg'))).toBe(false);
  });

  /** The state arrives as an input from the page, so it must drive the button. */
  it('offers to download it when it is missing', async () => {
    const fixture = render();
    fixture.nativeElement.querySelector('.panel__head').click();
    fixture.componentInstance.progress.set({ state: 'NotDone', progress: 0 });
    fixture.detectChanges();
    await fixture.whenStable();

    expect(labels(fixture).some((l) => l.includes('Download FFmpeg'))).toBe(true);

    const buttons: HTMLButtonElement[] = Array.from(fixture.nativeElement.querySelectorAll('button'));
    buttons.find((b) => b.textContent?.includes('Download FFmpeg'))!.click();

    expect(download).toHaveBeenCalledOnce();
  });

  it('shows the badge for the state it is given', () => {
    const fixture = render();

    expect(fixture.nativeElement.querySelector('.panel__badge').textContent).toContain('FFmpeg ready');
  });
});
