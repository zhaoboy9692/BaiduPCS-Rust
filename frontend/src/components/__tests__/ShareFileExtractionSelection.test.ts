import { mount, flushPromises } from '@vue/test-utils'
import { describe, it, expect, vi, beforeEach } from 'vitest'
import ElementPlus from 'element-plus'
import Selector from '../ShareFileSelector.vue'
import { previewShareDir } from '@/api/transfer'
vi.mock('@/api/transfer', () => ({ previewShareDir: vi.fn(), previewShareFiles: vi.fn() }))
const file = { fs_id: 1, name: 'root.txt', path: '/root.txt', size: 3, is_dir: false }
const folder = { fs_id: 2, name: 'folder', path: '/folder', size: 0, is_dir: true }
const child = { fs_id: 3, name: 'child.txt', path: '/sharelink1-2/folder/child.txt', size: 5, is_dir: false }
const child2 = { ...child, fs_id: 4, name: 'second.txt', path: '/sharelink1-2/folder/second.txt' }
const nested = { ...folder, fs_id: 5, path: '/sharelink1-2/folder/nested', name: 'nested' }
function mountSelector() {
  return mount(Selector, { props: { files: [file, folder], loading: false, extraction: true, shareInfo: { uk: 1, shareid: 2, short_key: 'abc', bdstoken: 'private' } as any }, global: { plugins: [ElementPlus] } })
}
function latest(wrapper: ReturnType<typeof mountSelector>, event: string) { return wrapper.emitted(event)?.at(-1)?.[0] }
beforeEach(() => { vi.clearAllMocks(); vi.mocked(previewShareDir).mockResolvedValue({ files: [child, child2, nested] } as any) })
describe('web file-only selection', () => {
  it('preserves original all-select UI for transfer and sync selectors', async () => {
    const wrapper = mountSelector()
    await wrapper.setProps({ extraction: false })
    expect(wrapper.text()).not.toContain('下载全选')
    const all = wrapper.findAllComponents({ name: 'ElCheckbox' }).find(x => x.text() === '全选')!
    all.vm.$emit('change', false); await flushPromises()
    expect(latest(wrapper, 'update:selectedFsIds')).toEqual([])
    all.vm.$emit('change', true); await flushPromises()
    expect(latest(wrapper, 'update:selectedFsIds')).toEqual([1, 2])
    wrapper.unmount()
  })
  it('never includes folders but keeps original download folder selection', () => {
    const wrapper = mountSelector()
    expect(latest(wrapper, 'update:extractFiles')).toEqual([file])
    expect(latest(wrapper, 'update:selectedFsIds')).toEqual([1, 2])
    expect(previewShareDir).not.toHaveBeenCalled()
    wrapper.unmount()
  })
  it('clears extraction while navigating and supports a single nested file', async () => {
    let finish!: (value: any) => void
    vi.mocked(previewShareDir).mockReturnValue(new Promise(resolve => { finish = resolve }))
    const wrapper = mountSelector()
    await wrapper.findAll('.file-item').find(x => x.text().includes('folder'))!.trigger('click')
    expect(latest(wrapper, 'update:extractFiles')).toEqual([])
    finish({ files: [child, child2, nested] }); await flushPromises()
    await wrapper.findAll('.file-item').find(x => x.text().includes('second.txt'))!.trigger('click')
    expect(latest(wrapper, 'update:extractFiles')).toEqual([child])
    await wrapper.setProps({ loading: true })
    expect(latest(wrapper, 'update:extractFiles')).toEqual([])
    wrapper.unmount()
  })
  it('all-select is current ordinary files, without folding or traversing nested folders', async () => {
    const wrapper = mountSelector()
    await wrapper.findAll('.file-item').find(x => x.text().includes('folder'))!.trigger('click')
    await flushPromises()
    expect(latest(wrapper, 'update:extractFiles')).toEqual([child, child2])
    // Original download may fold all children into the folder, extraction must not.
    expect(latest(wrapper, 'update:selectedFsIds')).toContain(2)
    const all = wrapper.findAllComponents({ name: 'ElCheckbox' }).find(x => x.text() === '全选')!
    all.vm.$emit('change', false); await flushPromises()
    expect(latest(wrapper, 'update:extractFiles')).toEqual([])
    expect(latest(wrapper, 'update:selectedFsIds')).toContain(5) // ordinary all-select never changes nested folder download selection
    all.vm.$emit('change', true); await flushPromises()
    expect(latest(wrapper, 'update:extractFiles')).toEqual([child, child2])
    expect(previewShareDir).toHaveBeenCalledTimes(1)
    await wrapper.findAll('button').find(x => x.text() === '根目录')!.trigger('click')
    expect(latest(wrapper, 'update:extractFiles')).toEqual([file])
    wrapper.unmount()
  })
})
