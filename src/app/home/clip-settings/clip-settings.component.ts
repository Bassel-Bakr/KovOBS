import { Component, inject, input, signal } from '@angular/core';
import { FieldTree, FormField } from '@angular/forms/signals';
import { openUrl } from '@tauri-apps/plugin-opener';
import { MatFormField, MatHint, MatInput, MatLabel } from '@angular/material/input';
import { MatButton } from '@angular/material/button';
import { MatIcon } from '@angular/material/icon';
import { MatProgressSpinner } from '@angular/material/progress-spinner';
import { FfmpegService } from '../../services/ffmpeg.service';

/**
 * Trim padding, and the FFmpeg post-processing panel.
 *
 * Owns the FFmpeg download state: nothing outside this section reads it.
 */
@Component({
  selector: 'app-clip-settings',
  imports: [FormField, MatFormField, MatLabel, MatHint, MatInput, MatButton, MatIcon, MatProgressSpinner],
  templateUrl: './clip-settings.component.html',
  styleUrl: './clip-settings.component.scss',
})
export default class ClipSettingsComponent {
  private readonly ffmpegService = inject(FfmpegService);

  readonly paddingStart = input.required<FieldTree<number, string>>();
  readonly paddingEnd = input.required<FieldTree<number, string>>();
  readonly globalArgs = input.required<FieldTree<string, string>>();
  readonly inputArgs = input.required<FieldTree<string, string>>();
  readonly outputArgs = input.required<FieldTree<string, string>>();

  /** Collapsed by default: most people never touch the args. */
  protected readonly ffmpegOpen = signal(false);

  /** Owned by the page, which also gates first-run setup on it. */
  readonly downloadProgress = input.required<{ state: string; progress: number }>();

  protected toggleFfmpeg(): void {
    this.ffmpegOpen.update((open) => !open);
  }

  protected downloadFFmpeg(): void {
    this.ffmpegService.download().subscribe();
  }

  protected deleteFFmpeg(): void {
    this.ffmpegService.remove().subscribe();
  }

  protected openFFmpegHelp(): void {
    void openUrl('https://ffmpeg.org/ffmpeg.html');
  }
}
