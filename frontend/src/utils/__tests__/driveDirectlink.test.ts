import { describe, it, expect } from 'vitest'
import { extractDriveLinks, downloadCommand } from '../driveDirectlink'
import type { FileItem } from '@/api/file'
const file = (path: string, isdir = 0): FileItem => ({ path, fs_id: 1, server_filename: path.split('/').pop()!, isdir, size: 4, category: 1, server_ctime: 0, server_mtime: 0, local_ctime: 0, local_mtime: 0, is_encrypted: false, is_encrypted_folder: false })
const listing = (list: FileItem[], has_more = false) => ({ list, has_more, dir: '/', page: 1, total: list.length })
async function collect(items: FileItem[], recursive = false, overrides: any = {}, signal = new AbortController().signal) {
  const reads: string[] = []
  const api = { list: async () => listing([]), link: async (_id: number, path: string) => { reads.push(path); return 'https://example.test/file' }, ...overrides }
  const results = []
  for await (const r of extractDriveLinks(items, recursive, signal, api)) results.push(r)
  return { results, reads }
}
describe('existing drive extraction', () => {
  it('extracts existing file by path without a transfer or directory', async () => {
    const { results, reads } = await collect([file('/a.bin')])
    expect(reads).toEqual(['/a.bin']); expect(results[0]).toMatchObject({ success: true, save_path: null, task_id: null })
    expect(results[0].headers['User-Agent']).toContain('netdisk')
  })
  it('continues after per-file failure', async () => {
    const { results } = await collect([file('/bad'), file('/good')], false, { link: async (_: number, p: string) => { if(p === '/bad') throw new Error('凭证已过期'); return 'https://example.test/good' } })
    expect(results.map(x => x.success)).toEqual([false, true]); expect(results[0].error?.message).toBe('凭证已过期')
  })
  it('does not recurse implicitly', async () => {
    let calls = 0
    const { results } = await collect([file('/folder', 1)], false, { list: async () => { calls++; return listing([]) } })
    expect(calls).toBe(0); expect(results[0].success).toBe(false)
  })
  it('reads pagination and descendants', async () => {
    const pages: string[] = []
    const { reads } = await collect([file('/f', 1)], true, { list: async (p: string, n: number) => {
      pages.push(`${p}:${n}`)
      return p === '/f/sub' ? listing([file('/f/sub/c')]) : n === 1 ? listing([file('/f/a'), file('/f/sub', 1)], true) : listing([file('/f/b')])
    } })
    expect(pages).toEqual(['/f:1', '/f:2', '/f/sub:1']); expect(reads.sort()).toEqual(['/f/a', '/f/b', '/f/sub/c'])
  })
  it('reports unreadable folders and continues others', async () => {
    const { results } = await collect([file('/bad', 1), file('/ok')], true, { list: async () => { throw new Error('读取目录失败') } })
    expect(results.map(x => x.success)).toEqual([false, true]); expect(results[0].is_dir).toBe(true)
  })
  it('cancels additional requests and discards pending result', async () => {
    const control = new AbortController(); let calls = 0
    const { results } = await collect([file('/a'), file('/b')], false, { link: async () => { calls++; control.abort(); return 'https://example.test/a' } }, control.signal)
    expect(calls).toBe(1); expect(results).toEqual([])
  })
  it('refuses encrypted files and directories', async () => {
    const { reads, results } = await collect([{ ...file('/a'), is_encrypted: true }, { ...file('/dir', 1), is_encrypted_folder: true }], true)
    expect(reads).toEqual([]); expect(results).toHaveLength(2); expect(results.every(x => x.error?.message.includes('加密'))).toBe(true)
  })
  it('rejects invalid or repeated entries instead of escaping or looping', async () => {
    for (const entries of [[file('/outside')], [file('/f/a'), file('/f/a')], [file('/f/../a')]]) {
      const { results, reads } = await collect([file('/f', 1)], true, { list: async () => listing(entries, true) })
      expect(results.some(x => !x.success)).toBe(true); expect(reads).toEqual([])
    }
  })
  it('rejects non-download schemes and handles empty folders', async () => {
    expect((await collect([file('/a')], false, { link: async () => 'javascript:alert(1)' })).results[0].success).toBe(false)
    expect((await collect([file('/f', 1)], true)).results).toEqual([])
  })
  it('quotes shell metacharacters', () => {
    const cmd = downloadCommand({ name: "a'$(touch nope).bin", url: "https://example.test/?a='&b=2", headers: { 'User-Agent': 'netdisk' } })
    expect(cmd).toContain("'a'\"'\"'$(touch nope).bin'"); expect(cmd).toContain('--user-agent'); expect(cmd).toContain('--output')
  })
})
