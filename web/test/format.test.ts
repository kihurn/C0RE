import { describe, expect, it } from 'vitest'
import { formatBytes, formatCount, formatDuration, relativeTime } from '../src/lib/format'

describe('formatDuration', () => {
  it.each([
    [0, '0 ms'],
    [850, '850 ms'],
    [1800, '1.8 s'],
    [12_400, '12 s'],
    [125_000, '2 min']
  ])('%d ms → %s', (ms, s) => expect(formatDuration(ms)).toBe(s))
})

describe('formatBytes', () => {
  it.each([
    [512, '512 B'],
    [2048, '2 KB'],
    [6_100_000_000, '6.1 GB'],
    [3_300_000_000, '3.3 GB'],
    [150_000_000, '150 MB']
  ])('%d → %s', (n, s) => expect(formatBytes(n)).toBe(s))
})

describe('formatCount', () => {
  it.each([
    [0, '0'],
    [999, '999'],
    [1234, '1,234'],
    [12_345, '12.3k'],
    [1_200_000, '1.2M']
  ])('%d → %s', (n, s) => expect(formatCount(n)).toBe(s))
})

describe('relativeTime', () => {
  const now = 1_000_000
  it.each([
    [now - 5, 'just now'],
    [now - 90, '1 min ago'],
    [now - 3 * 3600, '3 h ago'],
    [now - 2 * 86400, '2 d ago'],
    [null, 'never']
  ])('%s → %s', (ts, s) => expect(relativeTime(ts, now)).toBe(s))
})
