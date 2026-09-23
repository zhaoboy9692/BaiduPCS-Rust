<template>
  <el-dialog :model-value="modelValue" title="提取直链结果" :width="isMobile ? '95%' : '680px'" @update:model-value="$emit('update:modelValue', $event)">
    <el-alert title="链接可能过期，下载时请带下方 User-Agent。转存文件已保留，可在原文件管理页清理；清理可能使链接失效。" type="warning" :closable="false" />
    <p v-if="result">成功 {{ result.succeeded ?? result.list.filter(f => f.success).length }} 项，失败 {{ result.failed ?? result.list.filter(f => !f.success).length }} 项</p>
    <el-alert v-if="result && !result.complete" :title="result.error?.message || '部分目录未能完整读取，请检查失败项后重试。'" type="error" :closable="false" />
    <section v-for="(file, index) in result?.list" :key="index" class="link-file">
      <strong>{{ file.name }} <el-tag :type="file.success ? 'success' : 'danger'" size="small">{{ file.success ? '成功' : '失败' }}</el-tag></strong>
      <p>{{ file.path }} · {{ formatFileSize(file.size) }}</p>
      <template v-if="file.success && file.url">
        <div class="link-row"><el-input type="textarea" :rows="2" :model-value="file.url" readonly :aria-label="`${file.name} 直链`" /><el-button type="primary" @click="copy(file.url)">复制链接</el-button></div>
        <pre>{{ JSON.stringify(file.headers, null, 2) }}</pre>
        <el-button @click="copy(JSON.stringify(file.headers, null, 2))">复制请求头</el-button>
      </template>
      <el-alert v-else :title="file.error?.message || '提取失败，请检查原任务页面'" type="error" :closable="false" show-icon />
      <p v-if="file.save_path">转存目录：{{ file.save_path }}</p>
    </section>
    <template #footer>
      <el-button @click="copy(result?.list.filter(f => f.success && f.url).map(f => f.url).join('\n') || '')">复制全部链接</el-button>
      <el-button @click="$emit('update:modelValue', false)">关闭</el-button>
    </template>
  </el-dialog>
</template>
<script setup lang="ts">
import { formatFileSize } from '@/api/utils'
import { ElMessage } from 'element-plus'
import { useIsMobile } from '@/utils/responsive'
import { copyText } from '@/utils/directlinkClipboard'
import type { DirectlinkResult } from '@/api/directlink'
defineProps<{ modelValue: boolean; result: DirectlinkResult | null }>()
defineEmits<{ 'update:modelValue': [value: boolean] }>()
const isMobile = useIsMobile()
async function copy(value: string) {
  if (value && await copyText(value)) ElMessage.success('已复制')
  else ElMessage.warning('复制失败，请选中文本手动复制')
}
</script>
<style scoped>
.link-row { display: flex; align-items: center; gap: 10px; }
.link-file { margin-top: 18px; }
.link-file strong { display: block; margin-bottom: 8px; overflow-wrap: anywhere; }
pre, p { white-space: pre-wrap; overflow-wrap: anywhere; }
</style>
