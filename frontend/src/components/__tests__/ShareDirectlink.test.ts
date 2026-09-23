import { mount, flushPromises } from '@vue/test-utils'
import { describe, it, expect, vi } from 'vitest'
import ElementPlus from 'element-plus'
import ShareDialog from '../ShareDirectDownloadDialog.vue'
import { directlinkApi } from '@/api/directlink'
import { copyText } from '@/utils/directlinkClipboard'
vi.mock('@/api/directlink', () => ({ directlinkApi: { resolve: vi.fn() }, tokenError: () => '提取失败' }))
vi.mock('@/utils/directlinkClipboard', () => ({ copyText: vi.fn().mockResolvedValue(true) }))
vi.mock('@/api/config', () => ({ getConfig: vi.fn().mockResolvedValue({ download: {} }), updateRecentDirDebounced: vi.fn(), setDefaultDownloadDir: vi.fn() }))
vi.mock('@/api/transfer', () => ({ createTransfer: vi.fn(), previewShareFiles: vi.fn(), TransferErrorCodes: {} }))
function mountDialog() {
  return mount(ShareDialog, { props: { modelValue: true }, global: { plugins: [ElementPlus], stubs: {
    FilePickerModal: true, ShareFileSelector: true,
    'el-dialog': { props: ['modelValue'], template: '<section v-if="modelValue"><slot /><slot name="footer" /></section>' },
  } } })
}
describe('share extraction', () => {
  it('extracts without local download directory and copies returned link', async () => {
    vi.mocked(directlinkApi.resolve).mockResolvedValue({ task_id: 'task', save_path: '/saved', files: [{ filename: 'a.txt', size: 3, url: 'https://d.pcs.baidu.com/a', headers: { 'User-Agent': 'netdisk' }, expires_at: null }] })
    const wrapper = mountDialog()
    await wrapper.find('input').setValue('https://pan.baidu.com/s/1abc')
    const button = wrapper.findAll('button').find(b => b.text() === '提取直链')
    expect(button).toBeDefined()
    await button!.trigger('click'); await flushPromises()
    expect(directlinkApi.resolve).toHaveBeenCalledWith({ share_url: 'https://pan.baidu.com/s/1abc', password: undefined, selected_fs_ids: undefined })
    expect(wrapper.text()).toContain('a.txt')
    expect(wrapper.find('textarea[aria-label="a.txt 直链"]').element).toHaveProperty('value', 'https://d.pcs.baidu.com/a')
    await wrapper.findAll('button').find(b => b.text() === '复制链接')!.trigger('click')
    expect(copyText).toHaveBeenCalledWith('https://d.pcs.baidu.com/a')
    wrapper.unmount()
  })
  it('does not display success results on upstream failure', async () => {
    vi.mocked(directlinkApi.resolve).mockRejectedValue(new Error('failed'))
    const wrapper = mountDialog()
    await wrapper.find('input').setValue('https://pan.baidu.com/s/1abc')
    const button = wrapper.findAll('button').find(b => b.text() === '提取直链')
    expect(button).toBeDefined()
    await button!.trigger('click'); await flushPromises()
    expect(wrapper.text()).toContain('提取失败')
    expect(wrapper.find('textarea').exists()).toBe(false)
    wrapper.unmount()
  })
})
