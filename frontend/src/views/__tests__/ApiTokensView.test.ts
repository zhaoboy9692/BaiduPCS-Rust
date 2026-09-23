import { describe, it, expect, vi, beforeEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import ElementPlus from 'element-plus'
import { tokenApi } from '@/api/directlink'
import ApiTokensView from '../ApiTokensView.vue'
vi.mock('@/api/directlink', () => ({ tokenApi: { list: vi.fn(), create: vi.fn(), edit: vi.fn(), revoke: vi.fn() }, tokenError: () => 'Token 服务不可用' }))
beforeEach(() => vi.clearAllMocks())
describe('API Token management', () => {
  it('loads records and has create action', async () => {
    vi.mocked(tokenApi.list).mockResolvedValue([])
    const wrapper = mount(ApiTokensView, { global: { plugins: [ElementPlus], stubs: { 'el-table': true, 'el-table-column': true } } })
    await flushPromises()
    expect(tokenApi.list).toHaveBeenCalledWith(0, 50)
    expect(wrapper.text()).toContain('创建 Token')
    expect(wrapper.text()).toContain('API Token')
    wrapper.unmount()
  })
  it('shows load failure instead of claiming empty successful list', async () => {
    vi.mocked(tokenApi.list).mockRejectedValue(new Error('unavailable'))
    const wrapper = mount(ApiTokensView, { global: { plugins: [ElementPlus], stubs: { 'el-table': true, 'el-table-column': true } } })
    await flushPromises()
    expect(wrapper.text()).toContain('Token 服务不可用')
    wrapper.unmount()
  })
})

describe('create token dialog', () => {
  it('creates once and displays the returned secret', async () => {
    vi.mocked(tokenApi.list).mockResolvedValue([])
    vi.mocked(tokenApi.create).mockResolvedValue({ token: 'dl_one_time_secret', record: {} as never })
    const wrapper = mount(ApiTokensView, { global: { plugins: [ElementPlus], stubs: { 'el-table': true, 'el-table-column': true, teleport: true, 'el-dialog': { props: ['modelValue'], template: '<div v-if="modelValue" class="el-dialog"><slot /><slot name="footer" /></div>' } } } })
    await flushPromises()
    await wrapper.findAll('button').find(b => b.text() === '创建 Token')!.trigger('click')
    await flushPromises()
    const name = wrapper.find('.el-dialog input')
    await name.setValue('自动化脚本')
    await wrapper.findAll('button').find(b => b.text() === '创建')!.trigger('click')
    await flushPromises()
    expect(tokenApi.create).toHaveBeenCalledTimes(1)
    expect(tokenApi.create).toHaveBeenCalledWith({ name: '自动化脚本', note: '', expires_at: null, rate_per_minute: 10 })
    expect(wrapper.find('textarea[aria-label="新建的完整 Token"]').element).toHaveProperty('value', 'dl_one_time_secret')
    wrapper.unmount()
  })
  it('shows no secret dialog when creation fails', async () => {
    vi.mocked(tokenApi.list).mockResolvedValue([])
    vi.mocked(tokenApi.create).mockRejectedValue(new Error('failed'))
    const wrapper = mount(ApiTokensView, { global: { plugins: [ElementPlus], stubs: { 'el-table': true, 'el-table-column': true, teleport: true, 'el-dialog': { props: ['modelValue'], template: '<div v-if="modelValue" class="el-dialog"><slot /><slot name="footer" /></div>' } } } })
    await flushPromises()
    await wrapper.findAll('button').find(b => b.text() === '创建 Token')!.trigger('click')
    await flushPromises()
    await wrapper.find('.el-dialog input').setValue('失败测试')
    await wrapper.findAll('button').find(b => b.text() === '创建')!.trigger('click')
    await flushPromises()
    expect(wrapper.find('textarea[aria-label="新建的完整 Token"]').exists()).toBe(false)
    wrapper.unmount()
  })
})
