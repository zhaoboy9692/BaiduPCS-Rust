import { mount, flushPromises } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import ElementPlus, { ElMessageBox } from 'element-plus'
import Dialog from '../DriveDirectlinkDialog.vue'
import { getFileList, getDownloadUrl } from '@/api/file'
import { copyText } from '@/utils/directlinkClipboard'
vi.mock('@/api/file', () => ({ getFileList: vi.fn(), getDownloadUrl: vi.fn() }))
vi.mock('@/utils/directlinkClipboard', () => ({ copyText: vi.fn().mockResolvedValue(true) }))
const file = (path: string, isdir = 0) => ({ path, fs_id: 1, server_filename: path.split('/').pop()!, isdir, size: 4, is_encrypted: false, is_encrypted_folder: false })
function open(item = file('/a.bin')) {
  return mount(Dialog, { props: { modelValue: true, item: item as any }, global: { plugins: [ElementPlus], stubs: {
    'el-dialog': { props: ['modelValue'], template: '<section v-if="modelValue"><slot /><slot name="footer" /></section>' },
  } } })
}
function button(w: ReturnType<typeof open>, text: string) { return w.findAll('button').find(b => b.text() === text)! }
beforeEach(() => {
  vi.clearAllMocks()
  vi.mocked(getFileList).mockResolvedValue({ list: [file('/folder/a'), file('/folder/sub', 1)] as any, has_more: false, dir: '/folder', page: 1, total: 2 })
  vi.mocked(getDownloadUrl).mockResolvedValue('https://example.test/a')
})
describe('drive directlink UI', () => {
  it('extracts a regular file and copies its link and command without a transfer warning', async () => {
    const w = open(); await flushPromises()
    expect(getDownloadUrl).toHaveBeenCalledWith(1, '/a.bin')
    expect(w.text()).not.toContain('转存文件已保留')
    await button(w, '复制链接').trigger('click')
    expect(copyText).toHaveBeenCalledWith('https://example.test/a')
    await button(w, '复制下载命令').trigger('click')
    expect(copyText).toHaveBeenLastCalledWith(expect.stringContaining('--user-agent'))
    w.unmount()
  })
  it('folder only lists and all-select excludes directories', async () => {
    const w = open(file('/folder', 1)); await flushPromises()
    expect(getDownloadUrl).not.toHaveBeenCalled()
    expect(button(w, '提取所选文件').attributes('disabled')).toBeDefined()
    await button(w, '全选本页文件').trigger('click'); await button(w, '提取所选文件').trigger('click'); await flushPromises()
    expect(getDownloadUrl).toHaveBeenCalledTimes(1); expect(getDownloadUrl).toHaveBeenCalledWith(1, '/folder/a')
    expect(getFileList).toHaveBeenCalledTimes(1)
    w.unmount()
  })
  it('asks before recursion and cancellation does not start it', async () => {
    const confirm = vi.spyOn(ElMessageBox, 'confirm').mockRejectedValue('cancel')
    const w = open(file('/folder', 1)); await flushPromises()
    await button(w, '提取整个目录（含子目录）').trigger('click'); await flushPromises()
    expect(confirm).toHaveBeenCalledOnce(); expect(getDownloadUrl).not.toHaveBeenCalled(); expect(getFileList).toHaveBeenCalledTimes(1)
    confirm.mockRestore(); w.unmount()
  })
  it('stops in-flight extraction without displaying stale links', async () => {
    let finish!: (url: string) => void
    vi.mocked(getDownloadUrl).mockReturnValue(new Promise(resolve => { finish = resolve }))
    const w = open(); await flushPromises()
    await button(w, '停止提取').trigger('click'); finish('https://example.test/stale'); await flushPromises()
    expect(w.text()).toContain('已停止'); expect(w.find('textarea').exists()).toBe(false)
    w.unmount()
  })
  it('shows directory list errors without allowing stale selections', async () => {
    vi.mocked(getFileList).mockRejectedValue(new Error('目录无法读取'))
    const w = open(file('/folder', 1)); await flushPromises()
    expect(w.text()).toContain('目录无法读取'); expect(button(w, '提取所选文件').attributes('disabled')).toBeDefined()
    w.unmount()
  })
})
it('closes and discards links when active account changes', async () => {
  let finish!: (url: string) => void
  vi.mocked(getDownloadUrl).mockReturnValue(new Promise(resolve => { finish = resolve }))
  const w = open(); await flushPromises()
  window.dispatchEvent(new CustomEvent('multi-account:active-changed'))
  finish('https://example.test/stale'); await flushPromises()
  expect(w.emitted('update:modelValue')?.at(-1)).toEqual([false])
  expect(w.find('textarea').exists()).toBe(false)
  w.unmount()
})
it('navigates into folders, clears selection and returns to parent', async () => {
  const w = open(file('/folder', 1)); await flushPromises()
  await button(w, '全选本页文件').trigger('click')
  vi.mocked(getFileList).mockResolvedValue({ list: [] as any, has_more: false, dir: '/folder/sub', page: 1, total: 0 })
  await button(w, '📁 sub').trigger('click'); await flushPromises()
  expect(getFileList).toHaveBeenLastCalledWith('/folder/sub', 1, 100)
  expect(button(w, '提取所选文件').attributes('disabled')).toBeDefined()
  await button(w, '上一级').trigger('click'); await flushPromises()
  expect(getFileList).toHaveBeenLastCalledWith('/folder', 1, 100)
  w.unmount()
})
