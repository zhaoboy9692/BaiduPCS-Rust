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
    vi.mocked(directlinkApi.resolve).mockResolvedValue({ total: 1, succeeded: 1, failed: 0, complete: true, list: [{ fs_id: 1, name: 'a.txt', path: '/a.txt', is_dir: false, success: true, size: 3, url: 'https://d.pcs.baidu.com/a', headers: { 'User-Agent': 'netdisk' }, expires_at: null, task_id: 'task', save_path: '/saved', error: null }] })
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

it('shows every success with copy button and individual failure reason', async () => {
  vi.mocked(directlinkApi.resolve).mockResolvedValue({ total:3,succeeded:2,failed:1,complete:true,list:[
    {fs_id:1,name:'first.bin',path:'/folder/first.bin',is_dir:false,size:5_000_000_000,success:true,url:'https://example.test/first',headers:{},expires_at:null,task_id:'t1',save_path:'/s1',error:null},
    {fs_id:2,name:'bad.bin',path:'/folder/bad.bin',is_dir:false,size:10,success:false,url:null,headers:{},expires_at:null,task_id:null,save_path:null,error:{code:'token_quota_exhausted',message:'Token 提取次数已用尽'}},
    {fs_id:3,name:'last.bin',path:'/folder/last.bin',is_dir:false,size:3,success:true,url:'https://example.test/last',headers:{},expires_at:null,task_id:'t3',save_path:'/s3',error:null},
  ] })
  const wrapper=mountDialog()
  await wrapper.find('input').setValue('https://pan.baidu.com/s/1abc')
  await wrapper.findAll('button').find(b=>b.text()==='提取直链')!.trigger('click'); await flushPromises()
  expect(wrapper.text()).toContain('bad.bin');expect(wrapper.text()).toContain('Token 提取次数已用尽')
  expect(wrapper.findAll('button').filter(b=>b.text()==='复制链接')).toHaveLength(2)
  expect(wrapper.find('textarea[aria-label="last.bin 直链"]').element).toHaveProperty('value','https://example.test/last')
  wrapper.unmount()
})
