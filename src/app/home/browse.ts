import { FieldTree } from '@angular/forms/signals';
import { open } from '@tauri-apps/plugin-dialog';

/**
 * Native pickers for the path fields, shared by every section that has one.
 *
 * Cancelling returns null and leaves the field alone, so a mistaken click
 * cannot blank a configured path.
 */
async function pick(field: FieldTree<string, string>, directory: boolean): Promise<void> {
  const chosen = await open({ directory, multiple: false });

  if (chosen != null) {
    field().value.set(chosen);
  }
}

export function browseFolder(field: FieldTree<string, string>): Promise<void> {
  return pick(field, true);
}

export function browseFile(field: FieldTree<string, string>): Promise<void> {
  return pick(field, false);
}
