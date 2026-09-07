import { FieldTree } from '@angular/forms/signals';
import { open } from '@tauri-apps/plugin-dialog';

/**
 * Native path pickers.
 *
 * Cancelling returns null, and every caller leaves the existing value alone in
 * that case, so a mistaken click cannot blank a configured path.
 */
export function pickFolder(): Promise<string | null> {
  return open({ directory: true, multiple: false });
}

export function pickFile(): Promise<string | null> {
  return open({ multiple: false });
}

/** The same pickers, writing straight into a form field. */
export async function browseFolder(field: FieldTree<string, string>): Promise<void> {
  const folder = await pickFolder();

  if (folder != null) {
    field().value.set(folder);
  }
}

export async function browseFile(field: FieldTree<string, string>): Promise<void> {
  const file = await pickFile();

  if (file != null) {
    field().value.set(file);
  }
}
