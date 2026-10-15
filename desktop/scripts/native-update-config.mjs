export function nativeOrigin(value, testBuild, debug) {
  if (value === undefined) return undefined;
  if (!testBuild || debug || !/^http:\/\/127\.0\.0\.1:([1-9][0-9]{3,4})\/[0-9a-f]{32}$/.test(value)) {
    throw new Error('Native updater fixtures require an explicit isolated release-mode test build and loopback origin.');
  }
  const port = Number(new URL(value).port);
  if (port < 1024 || port > 65535) throw new Error('Invalid native updater fixture port.');
  return value;
}
