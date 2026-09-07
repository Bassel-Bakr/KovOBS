import { Component, inject, input } from '@angular/core';
import { FieldTree, FormField } from '@angular/forms/signals';
import { MatSlideToggle } from '@angular/material/slide-toggle';
import { MatButton } from '@angular/material/button';
import { NotificationService } from '../../services/notification.service';

/** Desktop notifications for a saved clip, and for anything that goes wrong. */
@Component({
  selector: 'app-notification-settings',
  imports: [FormField, MatSlideToggle, MatButton],
  templateUrl: './notification-settings.component.html',
  styleUrl: './notification-settings.component.scss',
})
export default class NotificationSettingsComponent {
  private readonly notificationService = inject(NotificationService);

  readonly enabled = input.required<FieldTree<boolean, string>>();
  readonly urgentClips = input.required<FieldTree<boolean, string>>();
  readonly failures = input.required<FieldTree<boolean, string>>();
  readonly sound = input.required<FieldTree<boolean, string>>();

  protected sendTestNotification(): void {
    this.notificationService.sendTest().subscribe();
  }
}
