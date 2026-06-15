import { Component, inject } from '@angular/core';
import { RouterOutlet } from '@angular/router';
import { MatIcon } from '@angular/material/icon';
import { MatToolbar } from '@angular/material/toolbar';
import { MatIconButton } from '@angular/material/button';
import { MatTooltip } from '@angular/material/tooltip';
import { TauriService } from './services/tauri.service';
import LogsComponent from './logs/logs.component';

@Component({
  selector: 'app-root',
  imports: [RouterOutlet, LogsComponent, MatIcon, MatToolbar, MatIconButton, MatTooltip],
  templateUrl: './app.component.html',
  styleUrl: './app.component.scss',
})
export class AppComponent {
  private readonly tauriService = inject(TauriService);

  protected restart(): void {
    this.tauriService.restart().subscribe();
  }
}
