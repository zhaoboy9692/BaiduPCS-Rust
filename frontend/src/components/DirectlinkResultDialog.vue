<template>
  <el-dialog :model-value="modelValue" title="提取直链结果" :width="isMobile ? '95%' : '680px'" @update:model-value="$emit('update:modelValue', $event)">
    <el-alert title="链接可能过期，下载时请带下方 User-Agent。转存文件已保留，可在原文件管理页清理；清理可能使链接失效。" type="warning" :closable="false" />
    <p v-if="result">转存目录：{{ result.save_path }}</p>
    <section v-for="(file, index) in result?.files" :key="index" class="link-file">
      <strong>{{ file.filename }}</strong>
      <el-input type="textarea" :rows="3" :model-value="file.url" readonly :aria-label="`${file.filename} 直链`" />
      <pre>{{ JSON.stringify(file.headers, null, 2) }}</pre>
      <el-button type="primary" @click="copy(file.url)">复制链接</el-button>
      <el-button @click="copy(JSON.stringify(file.headers, null, 2))">复制请求头</el-button>
    </section>
    <template #footer>
      <el-button @click="copy(result?.files.map(f => f.url).join('\n') || '')">复制全部链接</el-button>
      <el-button @click="$emit('update:modelValue', false)">关闭</el-button>
    </template>
  </el-dialog>
</template>
<script setup lang="ts">
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
.link-file { margin-top: 18px; }
.link-file strong { display: block; margin-bottom: 8px; overflow-wrap: anywhere; }
pre, p { white-space: pre-wrap; overflow-wrap: anywhere; }
</style>
