export function downloadBlob(blob: Blob, filename: string) {
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.download = filename;
  document.body.appendChild(anchor);
  try {
    anchor.click();
  } finally {
    anchor.remove();
    // WebKit can start consuming the object URL after the click handler returns.
    // Revoking synchronously races that work and can produce an empty download.
    setTimeout(() => URL.revokeObjectURL(url), 1_000);
  }
}
