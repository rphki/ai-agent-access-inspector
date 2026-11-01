import "@testing-library/jest-dom/vitest";

// jsdom does not implement modal dialogs; opening sets `open`, which makes
// the dialog visible to queries.
if (!HTMLDialogElement.prototype.showModal) {
  HTMLDialogElement.prototype.showModal = function showModal() {
    this.setAttribute("open", "");
  };
  HTMLDialogElement.prototype.close = function close() {
    this.removeAttribute("open");
  };
}
