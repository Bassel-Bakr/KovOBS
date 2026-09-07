import { Component, inject, signal } from '@angular/core';
import { rxResource } from '@angular/core/rxjs-interop';
import { openUrl } from '@tauri-apps/plugin-opener';
import { MatButton } from '@angular/material/button';
import { MatIcon } from '@angular/material/icon';
import { AboutInfo, UpdateInfo, UpdateService } from '../../services/update.service';
import { TauriService } from '../../services/tauri.service';

/**
 * Which version is running, whether a newer one exists, and the way out of the
 * app.
 *
 * Owns its own update state rather than taking it as input: nothing else in the
 * settings window reads it.
 */
@Component({
  selector: 'app-about-settings',
  imports: [MatButton, MatIcon],
  templateUrl: './about-settings.component.html',
  styleUrl: './about-settings.component.scss',
})
export default class AboutSettingsComponent {
  private readonly updateService = inject(UpdateService);
  private readonly tauriService = inject(TauriService);

  protected readonly about = rxResource<AboutInfo | null, unknown>({
    stream: () => this.updateService.about(),
    defaultValue: null,
  });

  protected readonly update = signal<UpdateInfo | null>(null);
  protected readonly checking = signal(false);
  protected readonly checkError = signal('');

  protected checkForUpdate(): void {
    this.checking.set(true);
    this.checkError.set('');

    this.updateService.check().subscribe({
      next: (info) => {
        this.update.set(info);
        this.checking.set(false);
      },
      error: (error: unknown) => {
        this.checkError.set(String(error));
        this.checking.set(false);
      },
    });
  }

  /** The releases index, not a specific release. */
  protected openReleases(): void {
    const url = this.about.value()?.releases_url;

    if (url) {
      void openUrl(url);
    }
  }

  /** The page for the release the last check found. */
  protected openLatestRelease(): void {
    const url = this.update()?.release_url;

    if (url) {
      void openUrl(url);
    }
  }

  /** Confirmation is the command's job, so the tray's Quit gets it too. */
  protected quit(): void {
    this.tauriService.quit().subscribe();
  }
}
