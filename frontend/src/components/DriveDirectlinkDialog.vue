<template>
  <el-dialog :model-value="modelValue" title="网盘文件直链" :width="isMobile ? '95%' : '760px'" :close-on-click-modal="false" @update:model-value="close">
    <el-alert title="直接读取网盘已有文件，不转存、不创建目录。链接可能过期或限制下载出口 IP；复制下载命令包含所需 User-Agent（适用于 macOS / Linux）。" type="info" :closable="false" />
    <template v-if="item?.isdir === 1">
      <div class="toolbar">
        <el-button size="small" :disabled="busy || loading || dir === item.path" @click="parent">上一级</el-button>
        <span class="path">{{ dir }}</span>
      </div>
      <el-alert v-if="error" :title="error" type="error" :closable="false" />
      <div v-loading="loading" class="entries">
        <div v-for="entry in entries" :key="entry.path" class="entry">
          <el-checkbox :model-value="selected.has(entry.path)" :disabled="busy || loading || !isPlainFile(entry)" :aria-label="`选择 ${entry.server_filename}`" @change="toggle(entry.path, Boolean($event))" />
          <el-button v-if="entry.isdir === 1" link type="primary" :disabled="busy || loading || entry.is_encrypted_folder" @click="load(entry.path)">📁 {{ entry.server_filename }}</el-button>
          <span v-else>{{ entry.server_filename }}</span>
          <el-tag v-if="entry.is_encrypted || entry.is_encrypted_folder" type="warning" size="small">加密：请用原下载功能</el-tag>
        </div>
        <el-empty v-if="!loading && !error && !entries.length" description="当前目录没有文件" :image-size="45" />
      </div>
      <div class="toolbar">
        <el-button size="small" :disabled="busy || loading || !entries.some(isPlainFile)" @click="selected = new Set(entries.filter(isPlainFile).map(f => f.path))">全选本页文件</el-button>
        <el-button size="small" :disabled="busy || loading || !selected.size" @click="selected.clear()">取消选择</el-button>
        <el-button size="small" :disabled="busy || loading || page === 1" @click="load(dir, page - 1)">上一页</el-button>
        <span>第 {{ page }} 页</span>
        <el-button size="small" :disabled="busy || loading || !hasMore" @click="load(dir, page + 1)">下一页</el-button>
      </div>
      <div class="toolbar">
        <el-button type="primary" :disabled="busy || loading || !selected.size" @click="run(entries.filter(f => selected.has(f.path) && isPlainFile(f)), false)">提取所选文件</el-button>
        <el-button :disabled="busy || loading || !!error" @click="recursive">提取整个目录（含子目录）</el-button>
      </div>
    </template>
    <p role="status">{{ status }} · 已处理 {{ results.length }} 项，成功 {{ succeeded }}，失败 {{ results.length - succeeded }}{{ busy ? '（正在扫描或提取，总数待确定）' : '' }}</p>
    <DirectlinkFileList :files="results" />
    <template #footer>
      <el-button v-if="busy" type="warning" @click="stop">停止提取</el-button>
      <el-button :disabled="!succeeded" @click="copyAll">复制全部链接</el-button>
      <el-button @click="close(false)">关闭</el-button>
    </template>
  </el-dialog>
</template>
<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { getFileList, type FileItem } from '@/api/file'
import type { DirectlinkFile } from '@/api/directlink'
import { extractDriveLinks, isPlainFile, validateDrivePage } from '@/utils/driveDirectlink'
import { copyText } from '@/utils/directlinkClipboard'
import { useIsMobile } from '@/utils/responsive'
import DirectlinkFileList from './DirectlinkFileList.vue'
const props = defineProps<{ modelValue: boolean; item: FileItem | null }>()
const emit = defineEmits<{ 'update:modelValue': [value: boolean] }>()
const isMobile = useIsMobile()
const dir = ref(''), page = ref(1), hasMore = ref(false), loading = ref(false), busy = ref(false), error = ref(''), status = ref('等待选择文件')
const entries = ref<FileItem[]>([]), selected = ref(new Set<string>()), results = ref<DirectlinkFile[]>([])
const succeeded = computed(() => results.value.filter(f => f.success).length)
let controller: AbortController | null = null
let generation = 0
let listGeneration = 0
function stop() { controller?.abort(); busy.value = false; status.value = '已停止，已提取的链接保留' }
function close(value: boolean) { if (!value) { stop(); generation++; listGeneration++ }; emit('update:modelValue', value) }
async function load(path: string, nextPage = 1) {
  if (!props.item || (path !== props.item.path && !path.startsWith(props.item.path + '/'))) return
  const current = ++listGeneration
  selected.value.clear(); entries.value = []; error.value = ''; hasMore.value = false; loading.value = true
  dir.value = path; page.value = nextPage
  try {
    const data = await getFileList(path, nextPage, 100)
    if (current !== listGeneration) return
    validateDrivePage(data.list, path, new Set())
    if (data.has_more && !data.list.length) throw new Error('目录分页异常，请刷新后重试')
    entries.value = data.list; hasMore.value = data.has_more
  } catch (e) { if (current === listGeneration) error.value = e instanceof Error ? e.message : '目录读取失败' }
  finally { if (current === listGeneration) loading.value = false }
}
function parent() { void load(dir.value.slice(0, dir.value.lastIndexOf('/')) || '/') }
function toggle(path: string, checked: boolean) { if (checked) selected.value.add(path); else selected.value.delete(path) }
async function run(files: FileItem[], recurse: boolean) {
  if (busy.value || !files.length) return
  controller?.abort()
  const control = new AbortController(); controller = control
  const current = generation
  results.value = []; busy.value = true; status.value = '正在提取'
  try {
    for await (const result of extractDriveLinks(files, recurse, control.signal)) {
      if (current !== generation || control.signal.aborted) return
      results.value.push(result)
    }
    if (current === generation && !control.signal.aborted) status.value = results.value.length ? '提取完成' : '目录内没有可提取的文件'
  } finally { if (controller === control) busy.value = false }
}
async function recursive() {
  const current = generation, path = dir.value
  try { await ElMessageBox.confirm('将遍历当前目录及所有子目录，逐个提取文件链接；不会创建目录或下载文件。是否继续？', '确认整目录提取', { type: 'warning', confirmButtonText: '开始提取', cancelButtonText: '取消' }) } catch { return }
  if (current !== generation || !props.modelValue || path !== dir.value || !props.item) return
  await run([{ ...props.item, path, server_filename: path.split('/').pop()!, isdir: 1 }], true)
}
async function copyAll() {
  if (await copyText(results.value.filter(f => f.success && f.url).map(f => f.url).join('\n'))) ElMessage.success('已复制')
  else ElMessage.warning('复制失败，请手动复制')
}
watch(() => [props.modelValue, props.item] as const, ([open, item]) => {
  controller?.abort(); generation++; listGeneration++; busy.value = false; loading.value = false
  entries.value = []; selected.value.clear(); results.value = []; error.value = ''; status.value = '等待选择文件'
  if (open && item) { if (item.isdir === 1) void load(item.path); else void run([item], false) }
}, { immediate: true })
function accountChanged() { close(false) }
onMounted(() => window.addEventListener('multi-account:active-changed', accountChanged))
onBeforeUnmount(() => { controller?.abort(); generation++; listGeneration++; window.removeEventListener('multi-account:active-changed', accountChanged) })
</script>
<style scoped>
.toolbar { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; margin: 12px 0; }
.toolbar .el-button { margin-left: 0; }
.path { overflow-wrap: anywhere; }
.entries { max-height: 260px; overflow: auto; }
.entry { display: flex; align-items: center; gap: 10px; min-height: 36px; }
.entry span, .entry .el-button { overflow-wrap: anywhere; white-space: normal; text-align: left; }
</style>
