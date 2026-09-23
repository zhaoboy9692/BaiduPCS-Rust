import { mount, flushPromises } from '@vue/test-utils'
import { describe, it, expect, vi, beforeEach } from 'vitest'
import ElementPlus from 'element-plus'
import ShareDialog from '../ShareDirectDownloadDialog.vue'
import { directlinkApi } from '@/api/directlink'
import { previewShareFiles } from '@/api/transfer'
import ShareFileSelector from '../ShareFileSelector.vue'
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
const selected = { fs_id: 1, name: 'a.txt', path: '/a.txt', is_dir: false, size: 3 }
beforeEach(() => {
  vi.clearAllMocks()
  vi.mocked(previewShareFiles).mockResolvedValue({ files: [selected] } as any)
})
async function selectFiles(wrapper: ReturnType<typeof mountDialog>, files = [selected]) {
  await wrapper.find('input').setValue('https://pan.baidu.com/s/1abc')
  await wrapper.findAll('button').find(b => b.text() === '选择分享/提取文件')!.trigger('click')
  await flushPromises()
  wrapper.findComponent(ShareFileSelector).vm.$emit('update:extractFiles', files)
  await flushPromises()
}
describe('share extraction', () => {
  it('has no extraction entry on input and disables extraction for empty selection', async () => {
    const wrapper = mountDialog()
    expect(wrapper.findAll('button').some(b => b.text() === '提取直链')).toBe(false)
    await selectFiles(wrapper, [])
    const button = wrapper.findAll('button').find(b => b.text() === '提取直链')!
    expect(button.attributes('disabled')).toBeDefined()
    await button.trigger('click')
    expect(directlinkApi.resolve).not.toHaveBeenCalled()
    wrapper.unmount()
  })
  it('rejects folder-only extraction selections and keeps the original download controls', async () => {
    const wrapper = mountDialog()
    expect(wrapper.findAll('button').some(b => b.text() === '直下全部')).toBe(true)
    await selectFiles(wrapper, [{ ...selected, is_dir: true }])
    const selector = wrapper.findComponent(ShareFileSelector)
    selector.vm.$emit('update:selectedFsIds', [1]); await flushPromises()
    expect(wrapper.findAll('button').find(b => b.text() === '提取直链')!.attributes('disabled')).toBeDefined()
    expect(wrapper.findAll('button').find(b => b.text() === '开始下载')!.attributes('disabled')).toBeUndefined()
    expect(directlinkApi.resolve).not.toHaveBeenCalled()
    wrapper.unmount()
  })
  it('labels the file selection entry for both sharing and extraction', () => {
    const wrapper = mountDialog()
    const labels = wrapper.findAll('button').map(button => button.text())
    expect(labels).toContain('选择分享/提取文件')
    expect(labels).not.toContain('选择分享文件')
    wrapper.unmount()
  })
  it('extracts without local download directory and copies returned link', async () => {
    vi.mocked(directlinkApi.resolve).mockResolvedValue({ total: 1, succeeded: 1, failed: 0, complete: true, list: [{ fs_id: 1, name: 'a.txt', path: '/a.txt', is_dir: false, success: true, size: 3, url: 'https://d.pcs.baidu.com/a', headers: { 'User-Agent': 'netdisk' }, expires_at: null, task_id: 'task', save_path: '/saved', error: null }] })
    const wrapper = mountDialog()
    await selectFiles(wrapper)
    const button = wrapper.findAll('button').find(b => b.text() === '提取直链')
    expect(button).toBeDefined()
    await button!.trigger('click'); await flushPromises()
    expect(directlinkApi.resolve).toHaveBeenCalledWith({ share_url: 'https://pan.baidu.com/s/1abc', password: undefined, selected_fs_ids: [1], selected_paths: ['/a.txt'] })
    expect(wrapper.text()).toContain('a.txt')
    expect(wrapper.find('textarea[aria-label="a.txt 直链"]').element).toHaveProperty('value', 'https://d.pcs.baidu.com/a')
    await wrapper.findAll('button').find(b => b.text() === '复制链接')!.trigger('click')
    expect(copyText).toHaveBeenCalledWith('https://d.pcs.baidu.com/a')
    wrapper.unmount()
  })
  it('does not display success results on upstream failure', async () => {
    vi.mocked(directlinkApi.resolve).mockRejectedValue(new Error('failed'))
    const wrapper = mountDialog()
    await selectFiles(wrapper)
    const button = wrapper.findAll('button').find(b => b.text() === '提取直链')
    expect(button).toBeDefined()
    await button!.trigger('click'); await flushPromises()
    expect(wrapper.text()).toContain('提取失败')
    expect(wrapper.find('textarea').exists()).toBe(false)
    wrapper.unmount()
  })
})

it('shows every success with copy button and individual failure reason', async () => {
  vi.mocked(directlinkApi.resolve).mockResolvedValue({ total:3,succeeded:2,failed:1,complete:true,list:[
    {fs_id:1,name:'first.bin',path:'/folder/first.bin',is_dir:false,size:5_000_000_000,success:true,url:'https://example.test/first',headers:{},expires_at:null,task_id:'t1',save_path:'/s1',error:null},
    {fs_id:2,name:'bad.bin',path:'/folder/bad.bin',is_dir:false,size:10,success:false,url:null,headers:{},expires_at:null,task_id:null,save_path:null,error:{code:'token_quota_exhausted',message:'Token 提取次数已用尽'}},
    {fs_id:3,name:'last.bin',path:'/folder/last.bin',is_dir:false,size:3,success:true,url:'https://example.test/last',headers:{},expires_at:null,task_id:'t3',save_path:'/s3',error:null},
  ] })
  const wrapper=mountDialog()
  const files = Array.from({ length: 21 }, (_, i) => ({ ...selected, fs_id: i + 1, name: `file${i}.bin`, path: `/folder/file${i}.bin`, size: 5_000_000_000 }))
  await selectFiles(wrapper, files)
  await wrapper.findAll('button').find(b=>b.text()==='提取直链')!.trigger('click'); await flushPromises()
  expect(directlinkApi.resolve).toHaveBeenCalledWith({ share_url: 'https://pan.baidu.com/s/1abc', password: undefined, selected_fs_ids: files.map(file => file.fs_id), selected_paths: files.map(file => file.path) })
  expect(wrapper.text()).toContain('bad.bin');expect(wrapper.text()).toContain('Token 提取次数已用尽')
  expect(wrapper.findAll('button').filter(b=>b.text()==='复制链接')).toHaveLength(2)
  expect(wrapper.find('textarea[aria-label="last.bin 直链"]').element).toHaveProperty('value','https://example.test/last')
  wrapper.unmount()
})
