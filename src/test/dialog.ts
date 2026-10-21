import { afterEach, beforeEach, vi } from 'vitest';

/** jsdomにないdialogの開閉だけを補い、実際のReactイベント接続は維持する。 */
export function installDialogBoundary() {
  let showDescriptor: PropertyDescriptor | undefined;
  let closeDescriptor: PropertyDescriptor | undefined;
  beforeEach(() => {
    showDescriptor = Object.getOwnPropertyDescriptor(HTMLDialogElement.prototype, 'showModal');
    closeDescriptor = Object.getOwnPropertyDescriptor(HTMLDialogElement.prototype, 'close');
    Object.defineProperties(HTMLDialogElement.prototype, {
      showModal: { configurable: true, value(this: HTMLDialogElement) { this.open = true; } },
      close: { configurable: true, value(this: HTMLDialogElement) { this.open = false; this.dispatchEvent(new Event('close')); } },
    });
  });
  afterEach(() => {
    if (showDescriptor) Object.defineProperty(HTMLDialogElement.prototype, 'showModal', showDescriptor);
    else Reflect.deleteProperty(HTMLDialogElement.prototype, 'showModal');
    if (closeDescriptor) Object.defineProperty(HTMLDialogElement.prototype, 'close', closeDescriptor);
    else Reflect.deleteProperty(HTMLDialogElement.prototype, 'close');
    vi.restoreAllMocks();
  });
}
