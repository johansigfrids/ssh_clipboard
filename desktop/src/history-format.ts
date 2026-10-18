// Presentation only: local clock labels never change shelf membership or selection.
export function timestamp(value: unknown): number | null {
  if (typeof value !== "number" && (typeof value !== "string" || !/^\d+$/.test(value))) return null;
  const number = Number(value);
  return Number.isSafeInteger(number) && number >= 0 && number <= 8.64e15 ? number : null;
}

function duration(milliseconds: number, round: (value: number) => number) {
  const [unit, scale]: [string, number] = milliseconds >= 86400000 ? ["day", 86400000] : milliseconds >= 3600000 ? ["hour", 3600000] : ["minute", 60000];
  const count = Math.max(1, round(milliseconds / scale));
  return `${count} ${unit}${count === 1 ? "" : "s"}`;
}

export function receivedTime(value: unknown, now: number): string {
  const time = timestamp(value);
  if (time === null || !Number.isFinite(now)) return "Receipt time unknown";
  if (time > now) return "Receipt time ahead of this clock";
  return now - time < 60000 ? "Just now" : `${duration(now - time, Math.floor)} ago`;
}

export function expiryTime(value: unknown, now: number): string {
  const time = timestamp(value);
  if (time === null || !Number.isFinite(now)) return "Expiry unknown";
  return time <= now ? "Past expiry (local clock)" : `Expires in ${duration(time - now, Math.ceil)}`;
}

export function exactTime(value: unknown): string {
  const time = timestamp(value);
  return time === null ? "Unknown time" : new Date(time).toLocaleString();
}

export function isoTime(value: unknown): string | undefined {
  const time = timestamp(value);
  return time === null ? undefined : new Date(time).toISOString();
}

export function formatSize(bytes: number): string {
  if (!Number.isSafeInteger(bytes) || bytes < 0) return "Unknown size";
  const units = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
  let value = bytes, unit = 0;
  while (value >= 1024 && unit < units.length - 1) { value /= 1024; unit++; }
  return unit === 0 ? `${bytes} B` : `${value.toFixed(1)} ${units[unit]}`;
}
