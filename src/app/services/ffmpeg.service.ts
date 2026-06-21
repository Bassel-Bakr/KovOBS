import { inject, Service } from '@angular/core';
import { Observable } from 'rxjs';
import { TauriService } from './tauri.service';

@Service()
export class FfmpegService {
  private readonly tauriService = inject(TauriService);

  is_downloaded(): Observable<boolean> {
    return this.tauriService.call<boolean>('is_ffmpeg_downloaded');
  }

  download(): Observable<boolean> {
    return this.tauriService.call<boolean>('download_ffmpeg');
  }

  remove(): Observable<void> {
    return this.tauriService.call<void>('remove_ffmpeg');
  }
}
