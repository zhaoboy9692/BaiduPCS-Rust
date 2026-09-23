import { describe, it, expect, vi, beforeEach } from 'vitest'
import { defineComponent, provide, inject, computed, h } from 'vue'
import { mount, flushPromises } from '@vue/test-utils'
import ElementPlus, { ElMessageBox } from 'element-plus'
import { tokenApi } from '@/api/directlink'
import ApiTokensView from '../ApiTokensView.vue'
vi.mock('@/api/directlink', () => ({ tokenApi: { list: vi.fn(), create: vi.fn(), edit: vi.fn(), revoke: vi.fn(), rotate: vi.fn(), resetUsage: vi.fn() }, tokenError: () => 'Token 服务不可用' }))
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
    expect(wrapper.text()).toContain('累计提取次数')
    const name = wrapper.find('.el-dialog input')
    await name.setValue('自动化脚本')
    await wrapper.findAll('button').find(b => b.text() === '创建')!.trigger('click')
    await flushPromises()
    expect(tokenApi.create).toHaveBeenCalledTimes(1)
    expect(tokenApi.create).toHaveBeenCalledWith({ name: '自动化脚本', note: '', expires_at: null, rate_per_minute: 10, max_uses: null })
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

it('rotates revoked token and resets usage via distinct confirmed actions', async () => {
  vi.mocked(tokenApi.list).mockResolvedValue([{id:'revoked-id',name:'已撤销测试',note:'',display_prefix:'dl_old',enabled:false,revoked_at:1,expires_at:null,created_at:0,updated_at:0,last_used_at:null,rate_per_minute:10,success_count:3,failure_count:1,max_uses:10,used_count:4}])
  vi.mocked(tokenApi.rotate).mockResolvedValue({token:'dl_new_secret',record:{} as never})
  vi.mocked(tokenApi.resetUsage).mockResolvedValue({} as never)
  vi.spyOn(ElMessageBox,'confirm').mockResolvedValue('confirm' as never)
  const wrapper=mount(ApiTokensView,{global:{plugins:[ElementPlus],stubs:{'el-table':defineComponent({props:['data'],setup(props,{slots}){provide('test-rows',computed(()=>props.data));return()=>h('div',slots.default?.())}}),'el-table-column':defineComponent({setup(_,{slots}){const rows=inject<any>('test-rows');return()=>h('div',rows.value.map((row:any)=>slots.default?.({row})))}}),'el-dialog':{props:['modelValue'],template:'<section v-if="modelValue"><slot/><slot name="footer"/></section>'}}}})
  await flushPromises()
  const buttons=wrapper.findAll('button')
  const rotate=buttons.find(b=>b.text()==='重置 Token')!
  expect(rotate.attributes('disabled')).toBeUndefined()
  expect(buttons.find(b=>b.text()==='已撤销')!.attributes('disabled')).toBeDefined()
  await rotate.trigger('click');await flushPromises()
  expect(tokenApi.rotate).toHaveBeenCalledWith('revoked-id')
  expect(wrapper.find('textarea[aria-label="新建的完整 Token"]').element).toHaveProperty('value','dl_new_secret')
  await wrapper.findAll('button').find(b=>b.text()==='重置次数')!.trigger('click');await flushPromises()
  expect(tokenApi.resetUsage).toHaveBeenCalledWith('revoked-id')
  wrapper.unmount();vi.restoreAllMocks()
})
