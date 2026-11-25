type BrowserSentry = { captureException(error: unknown): unknown };

/** Sends an error the interface caught to the crash reporter, which the desktop app injects only
 * when crash reports are built in; it drops the report while the creator has them turned off. */
export function reportError(error: unknown) {
  const sentry = (globalThis as { Sentry?: BrowserSentry }).Sentry;
  try {
    sentry?.captureException(error);
  } catch {
    // Reporting must never break the page that shows the error.
  }
}
