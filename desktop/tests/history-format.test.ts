import { describe, expect, it } from "vitest";
import { exactTime, expiryTime, formatSize, isoTime, receivedTime, timestamp } from "../src/history-format";

const now = Date.UTC(2026, 9, 18, 12);

describe("history presentation", () => {
  it("receipt labels handle minute/hour/day boundaries without inventing a future receipt", () => {
    expect(receivedTime(String(now), now)).toBe("Just now");
    expect(receivedTime(now - 59999, now)).toBe("Just now");
    expect(receivedTime(now - 60000, now)).toBe("1 minute ago");
    expect(receivedTime(now - 120000, now)).toBe("2 minutes ago");
    expect(receivedTime(now - 3600000, now)).toBe("1 hour ago");
    expect(receivedTime(now - 172800000, now)).toBe("2 days ago");
    expect(receivedTime(now + 1, now)).toBe("Receipt time ahead of this clock");
  });

  it("expiry rounds up and treats elapsed time as a local-clock observation", () => {
    expect(expiryTime(now + 1, now)).toBe("Expires in 1 minute");
    expect(expiryTime(now + 60000, now)).toBe("Expires in 1 minute");
    expect(expiryTime(now + 60001, now)).toBe("Expires in 2 minutes");
    expect(expiryTime(now + 3600000, now)).toBe("Expires in 1 hour");
    expect(expiryTime(now + 86400000, now)).toBe("Expires in 1 day");
    expect(expiryTime(now, now)).toBe("Past expiry (local clock)");
    expect(expiryTime(now - 1, now)).toBe("Past expiry (local clock)");
  });

  it("invalid, imprecise and out-of-Date-range values remain unknown; valid exact times stay localized", () => {
    for (const value of [null, undefined, "", " ", "garbage", -1, 1.5, "9007199254740993", "8640000000000001", Infinity, NaN]) {
      expect(timestamp(value)).toBeNull();
      expect(receivedTime(value, now)).toBe("Receipt time unknown");
      expect(expiryTime(value, now)).toBe("Expiry unknown");
      expect(exactTime(value)).toBe("Unknown time");
      expect(isoTime(value)).toBeUndefined();
    }
    expect(timestamp("0")).toBe(0);
    expect(isoTime(String(now))).toBe("2026-10-18T12:00:00.000Z");
    expect(exactTime(now)).toBe(new Date(now).toLocaleString());
  });

  it("byte labels use readable binary units through large values without rounding stored bytes", () => {
    expect(formatSize(0)).toBe("0 B");
    expect(formatSize(1023)).toBe("1023 B");
    expect(formatSize(1024)).toBe("1.0 KiB");
    expect(formatSize(1048576)).toBe("1.0 MiB");
    expect(formatSize(268435456)).toBe("256.0 MiB");
    expect(formatSize(4294967295)).toBe("4.0 GiB");
    expect(formatSize(1099511627776)).toBe("1.0 TiB");
    expect(formatSize(-1)).toBe("Unknown size");
  });
});
