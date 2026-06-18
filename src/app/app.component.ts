import { Component, inject } from '@angular/core';
import { RouterOutlet } from '@angular/router';
import LogsComponent from './logs/logs.component';
import { GlobalService } from './services/global.service';

@Component({
  selector: 'app-root',
  imports: [RouterOutlet, LogsComponent],
  templateUrl: './app.component.html',
  styleUrl: './app.component.scss',
})
export class AppComponent {
  protected globalService = inject(GlobalService);
}
