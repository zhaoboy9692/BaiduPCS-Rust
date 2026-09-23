import { expect, it, vi } from 'vitest'
const { get } = vi.hoisted(() => ({ get: vi.fn().mockResolvedValue({ data: { code: 0, data: { url: 'https://example.test/a' } } }) }))
vi.mock('axios', () => ({ default: { create: () => ({ get, interceptors: { request: { use: vi.fn() }, response: { use: vi.fn() } } }) } }))
import { getDownloadUrl } from '../file'
it('includes required path along with fs_id', async () => {
  await (getDownloadUrl as any)(5, '/folder/a')
  expect(get).toHaveBeenCalledWith('/files/download', { params: { fs_id: 5, path: '/folder/a' } })
})
