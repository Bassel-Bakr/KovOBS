import { Component, inject, signal, viewChild } from '@angular/core';
import { RouterOutlet } from '@angular/router';
import LogsComponent from './logs/logs.component';
import { GlobalService } from './services/global.service';
import { MatToolbar } from '@angular/material/toolbar';
import { MatIcon } from '@angular/material/icon';
import { MatIconButton } from '@angular/material/button';
import { MatMenu, MatMenuItem, MatMenuTrigger } from '@angular/material/menu';
import { TauriService } from './services/tauri.service';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { toSignal } from '@angular/core/rxjs-interop';
import { from } from 'rxjs';

@Component({
  selector: 'app-root',
  imports: [RouterOutlet, LogsComponent, MatToolbar, MatIcon, MatIconButton, MatMenu, MatMenuItem, MatMenuTrigger],
  templateUrl: './app.component.html',
  styleUrl: './app.component.scss',
})
export class AppComponent {
  protected globalService = inject(GlobalService);
  private readonly tauriService = inject(TauriService);

  private readonly quitMenuTrigger = viewChild.required(MatMenuTrigger);

  protected readonly quitMenuAt = signal({ x: 0, y: 0 });

  protected currentWindow = getCurrentWindow();

  protected title = toSignal(from(this.currentWindow.title()));

  minimize() {
    void this.currentWindow.minimize();
  }

  toggleMaximize() {
    void this.currentWindow.toggleMaximize();
  }

  close() {
    void this.currentWindow.close();
  }

  /** Closing only hides to the tray, so quitting needs its own way in. */
  openQuitMenu(event: MouseEvent) {
    event.preventDefault();

    this.quitMenuAt.set({ x: event.clientX, y: event.clientY });
    this.quitMenuTrigger().openMenu();
  }

  quit() {
    // Confirmation is the command's job, so the tray's Quit gets it too.
    this.tauriService.quit().subscribe();
  }
}
