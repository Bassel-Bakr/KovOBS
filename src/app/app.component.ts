import { Component, inject } from '@angular/core';
import { RouterOutlet } from '@angular/router';
import LogsComponent from './logs/logs.component';
import { GlobalService } from './services/global.service';
import { MatToolbar } from '@angular/material/toolbar';
import { MatIcon } from '@angular/material/icon';
import { MatIconButton } from '@angular/material/button';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { toSignal } from '@angular/core/rxjs-interop';
import { from } from 'rxjs';

@Component({
  selector: 'app-root',
  imports: [RouterOutlet, LogsComponent, MatToolbar, MatIcon, MatIconButton],
  templateUrl: './app.component.html',
  styleUrl: './app.component.scss',
})
export class AppComponent {
  protected globalService = inject(GlobalService);

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
}
