<template>
  <section v-for="(file, index) in files" :key="index" class="link-file">
    <strong>{{ file.name }} <el-tag :type="file.success ? 'success' : 'danger'" size="small">{{ file.success ? '成功' : '失败' }}</el-tag></strong>
    <p>{{ file.path }} · {{ formatFileSize(file.size) }}</p>
    <template v-if="file.success && file.url">
      <el-input type="textarea" :rows="2" :model-value="file.url" readonly :aria-label="`${file.name} 直链`" />
      <div class="copy-actions">
        <el-button type="primary" size="small" @click="copy(file.url)">复制链接</el-button>
        <el-button size="small" @click="copy(downloadCommand(file))">复制下载命令</el-button>
        <el-button size="small" @click="copy(JSON.stringify(file.headers, null, 2))">复制请求头</el-button>
      </div>
      <pre>{{ JSON.stringify(file.headers, null, 2) }}</pre>
    </template>
    <el-alert v-else :title="file.error?.message || '提取失败，请重试'" type="error" :closable="false" show-icon />
    <p v-if="file.save_path">转存目录：{{ file.save_path }}</p>
  </section>
</template>
<script setup lang="ts">
import type { DirectlinkFile } from '@/api/directlink'
import { formatFileSize } from '@/api/utils'
import { downloadCommand } from '@/utils/driveDirectlink'
import { copyText } from '@/utils/directlinkClipboard'
import { ElMessage } from 'element-plus'
defineProps<{ files: DirectlinkFile[] }>()
async function copy(value: string) {
  if (value && await copyText(value)) ElMessage.success('已复制')
  else ElMessage.warning('复制失败，请选中文本手动复制')
}
</script>
<style scoped>
.link-file { margin-top: 18px; }
.link-file strong { display: block; overflow-wrap: anywhere; }
.copy-actions { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 8px; }
.copy-actions .el-button { margin-left: 0; }
pre, p { white-space: pre-wrap; overflow-wrap: anywhere; }
</style>
