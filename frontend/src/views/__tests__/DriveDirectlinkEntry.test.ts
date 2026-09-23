import { mount, flushPromises } from '@vue/test-utils'
import { ref, markRaw } from 'vue'
import * as Icons from '@element-plus/icons-vue'
const NativeObserver = globalThis.MutationObserver
vi.stubGlobal('MutationObserver', class extends NativeObserver { constructor(callback: MutationCallback) { super(callback); return markRaw(this) } })
import ElementPlus from 'element-plus'
import { beforeEach, expect, it, vi } from 'vitest'
import FilesView from '../FilesView.vue'
import DriveDialog from '@/components/DriveDirectlinkDialog.vue'
const state = vi.hoisted(() => ({ mobile: false }))
vi.mock('@/utils/responsive', () => ({ useIsMobile: () => ref(state.mobile) }))
vi.mock('@/api/file', async importOriginal => ({ ...await importOriginal<any>(), getFileList: vi.fn().mockResolvedValue({ list: [
  { fs_id: 1, path: '/a', server_filename: 'a', size: 1, isdir: 0 },
  { fs_id: 2, path: '/folder', server_filename: 'folder', size: 0, isdir: 1 },
  { fs_id: 3, path: '/encrypted', server_filename: 'encrypted', size: 1, isdir: 0, is_encrypted: true },
], has_more: false, page: 1, total: 3 }) }))
vi.mock('@/api/config', () => ({ getConfig: vi.fn().mockResolvedValue({ download: {}, upload: {}, conflict_strategy: {} }), updateRecentDirDebounced: vi.fn(), setDefaultDownloadDir: vi.fn() }))
vi.mock('@/api/autobackup', () => ({ getEncryptionStatus: vi.fn().mockResolvedValue({ has_key: false }) }))
beforeEach(() => { state.mobile = false })
for (const mobile of [false, true]) it(`opens file/folder dialog and refuses encrypted file (${mobile ? 'mobile' : 'desktop'})`, async () => {
  state.mobile = mobile
  const w = mount(FilesView, { global: { components: Icons, plugins: [ElementPlus], stubs: { FilePickerModal: true, TransferDialog: true, ShareDialog: true, ShareDirectDownloadDialog: true, NetdiskFolderPickerModal: true, DriveDirectlinkDialog: true } } })
  await flushPromises()
  const buttons = w.findAll('button').filter(b => b.text() === '直链')
  expect(buttons).toHaveLength(3)
  expect(buttons[2].attributes('disabled')).toBeDefined()
  await buttons[0].trigger('click')
  expect(w.findComponent(DriveDialog).props()).toMatchObject({ modelValue: true, item: { path: '/a', isdir: 0 } })
  await buttons[1].trigger('click')
  expect(w.findComponent(DriveDialog).props()).toMatchObject({ modelValue: true, item: { path: '/folder', isdir: 1 } })
  w.unmount()
})
