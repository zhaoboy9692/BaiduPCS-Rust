<template>
  <section class="token-page">
    <header><h2>API Token</h2><el-button type="primary" size="small" @click="openCreate">创建 Token</el-button></header>
    <el-alert v-if="plainHttp" title="当前使用 HTTP，密码和 Token 会明文传输，请勿在不可信网络使用。" type="warning" :closable="false" show-icon />
    <el-alert title="Token 仅用于直链 API，不能登录管理后台。完整 Token 只在创建时显示一次。" type="info" :closable="false" />
    <el-alert v-if="loadError" :title="loadError" type="error" :closable="false" show-icon />
    <el-table v-loading="loading" :data="records" size="small" stripe>
      <el-table-column label="名称 / 备注" min-width="140"><template #default="{ row }"><strong>{{ row.name }}</strong><div class="note">{{ row.note }}</div></template></el-table-column>
      <el-table-column label="标识" width="130"><template #default="{ row }"><code>{{ row.display_prefix }}…</code></template></el-table-column>
      <el-table-column label="状态" width="85"><template #default="{ row }"><el-tag size="small" :type="tokenStatus(row, clock) === '启用' ? 'success' : 'info'">{{ tokenStatus(row, clock) }}</el-tag></template></el-table-column>
      <el-table-column label="有效期" min-width="150"><template #default="{ row }">{{ date(row.expires_at, '永久') }}</template></el-table-column>
      <el-table-column label="最近使用" min-width="150"><template #default="{ row }">{{ date(row.last_used_at, '未使用') }}</template></el-table-column>
      <el-table-column label="限频 / 分钟" width="100" prop="rate_per_minute" />
      <el-table-column label="次数：已用 / 上限" min-width="150"><template #default="{ row }">{{ row.used_count }} / {{ row.max_uses ?? '不限' }}<div class="note">剩余：{{ row.max_uses === null ? '不限' : Math.max(0, row.max_uses - row.used_count) }}</div></template></el-table-column>
      <el-table-column label="成功 / 失败" width="100"><template #default="{ row }">{{ row.success_count }} / {{ row.failure_count }}</template></el-table-column>
      <el-table-column label="操作" width="310" fixed="right"><template #default="{ row }">
        <el-button link type="primary" size="small" :disabled="row.revoked_at !== null || busy" @click="openEdit(row)">编辑</el-button>
        <el-button link type="warning" size="small" :disabled="row.revoked_at !== null || busy" @click="toggle(row)">{{ row.revoked_at !== null ? '已撤销' : row.enabled ? '禁用' : '恢复' }}</el-button>
        <el-button link type="primary" size="small" :disabled="busy" @click="rotate(row)">重置 Token</el-button>
        <el-button link type="warning" size="small" :disabled="busy" @click="resetUsage(row)">重置次数</el-button>
        <el-button link type="danger" size="small" :disabled="row.revoked_at !== null || busy" @click="revoke(row)">撤销</el-button>
      </template></el-table-column>
    </el-table>
    <footer><el-button size="small" :disabled="page === 0 || loading" @click="page--; load()">上一页</el-button><span>第 {{ page + 1 }} 页</span><el-button size="small" :disabled="records.length < 50 || loading" @click="page++; load()">下一页</el-button><el-button size="small" :loading="loading" @click="load">刷新</el-button></footer>
    <el-dialog v-model="editing" :title="editId ? '编辑 Token' : '创建 Token'" width="min(520px, 95vw)" :close-on-click-modal="false" :close-on-press-escape="!busy" :show-close="!busy">
      <el-form label-width="100px" @submit.prevent="save">
        <el-form-item label="名称"><el-input v-model="form.name" maxlength="80" /></el-form-item>
        <el-form-item label="备注"><el-input v-model="form.note" type="textarea" maxlength="500" /></el-form-item>
        <el-form-item label="有效期"><el-date-picker v-model="expiry" type="datetime" format="YYYY-MM-DD HH:mm" placeholder="留空为永久有效" clearable /></el-form-item>
        <el-form-item label="累计提取次数"><el-input-number v-model="form.max_uses" :min="0" :precision="0" :value-on-clear="null" placeholder="留空不限" /><span class="note">留空不限；每文件尝试计1次，失败也计入，预览不计。</span></el-form-item>
        <el-form-item label="每分钟次数"><el-input-number v-model="form.rate_per_minute" :min="1" :max="60" /></el-form-item>
      </el-form>
      <template #footer><el-button :disabled="busy" @click="editing = false">取消</el-button><el-button type="primary" :loading="busy" @click="save">{{ editId ? '保存' : '创建' }}</el-button></template>
    </el-dialog>
    <el-dialog v-model="showSecret" title="请立即保存 Token" width="min(600px, 95vw)" :close-on-click-modal="false" @closed="secret = ''">
      <el-alert title="完整 Token 仅显示这一次。关闭后无法找回，可创建新 Token 并撤销旧的。" type="warning" :closable="false" />
      <el-input :model-value="secret" type="textarea" readonly :rows="3" aria-label="新建的完整 Token" />
      <template #footer><el-button type="primary" @click="copy">复制 Token</el-button><el-button @click="showSecret = false">已保存，关闭</el-button></template>
    </el-dialog>
  </section>
</template>
<script setup lang="ts">
import { onMounted, onUnmounted, reactive, ref } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { tokenApi, tokenError, type ApiToken, type TokenInput } from '@/api/directlink'
import { copyText, tokenStatus } from '@/utils/directlinkClipboard'
const records = ref<ApiToken[]>([]), loading = ref(false), busy = ref(false), loadError = ref(''), page = ref(0)
const editing = ref(false), editId = ref(''), showSecret = ref(false), secret = ref(''), expiry = ref<Date | null>(null)
const form = reactive<TokenInput>({ name: '', note: '', expires_at: null, rate_per_minute: 10, max_uses: null })
const plainHttp = window.location.protocol === 'http:'
const clock = ref(Date.now() / 1000)
let alive = true
const timer = setInterval(() => { clock.value = Date.now() / 1000 }, 1000)
onUnmounted(() => { alive = false; clearInterval(timer); secret.value = '' })
function date(value: number | null, fallback: string) { return value === null ? fallback : new Date(value * 1000).toLocaleString('zh-CN', { year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit', hour12: false }) }
async function load() {
  if (loading.value) return
  loading.value = true; loadError.value = ''
  try { const result = await tokenApi.list(page.value * 50, 50); if (alive) records.value = result }
  catch (error) { if (alive) loadError.value = tokenError(error) }
  finally { if (alive) loading.value = false }
}
function openCreate() { editId.value = ''; Object.assign(form, { name: '', note: '', expires_at: null, rate_per_minute: 10, max_uses: null }); expiry.value = null; editing.value = true }
function openEdit(row: ApiToken) { editId.value = row.id; Object.assign(form, { name: row.name, note: row.note, rate_per_minute: row.rate_per_minute, expires_at: row.expires_at, max_uses: row.max_uses }); expiry.value = row.expires_at === null ? null : new Date(row.expires_at * 1000); editing.value = true }
async function save() {
  if (busy.value) return
  if (!form.name.trim()) { ElMessage.warning('请填写名称'); return }
  const expires_at = expiry.value ? Math.floor(expiry.value.getTime() / 1000) : null
  if (expires_at !== null && (!Number.isFinite(expires_at) || expires_at <= Date.now() / 1000)) { ElMessage.warning('有效期必须晚于现在'); return }
  busy.value = true
  try {
    const input = { ...form, name: form.name.trim(), expires_at }
    if (editId.value) { await tokenApi.edit(editId.value, input); if (alive) ElMessage.success('已保存') }
    else { const issued = await tokenApi.create(input); if (alive) { secret.value = issued.token; showSecret.value = true } }
    if (alive) { editing.value = false; await load() }
  } catch (error) { if (alive) ElMessage.error(tokenError(error)) }
  finally { busy.value = false }
}
async function toggle(row: ApiToken) {
  if (busy.value) return
  busy.value = true
  try { await tokenApi.edit(row.id, { enabled: !row.enabled }); await load() }
  catch (error) { ElMessage.error(tokenError(error)) }
  finally { busy.value = false }
}
async function revoke(row: ApiToken) {
  if (busy.value) return
  try { await ElMessageBox.confirm(`永久撤销「${row.name}」？撤销后不能恢复。`, '确认撤销', { type: 'warning', confirmButtonText: '永久撤销' }) } catch { return }
  busy.value = true
  try { await tokenApi.revoke(row.id); await load() }
  catch (error) { ElMessage.error(tokenError(error)) }
  finally { busy.value = false }
}
async function rotate(row: ApiToken) {
  if (busy.value) return
  try { await ElMessageBox.confirm(`重置「${row.name}」的 Token？旧密钥永久失效，新密钥仅显示一次；保留有效期、次数上限和已用次数。`, '重置 Token', { type: 'warning' }) } catch { return }
  busy.value = true
  try { const issued = await tokenApi.rotate(row.id); if (alive) { secret.value = issued.token; showSecret.value = true }; await load() }
  catch (error) { ElMessage.error(tokenError(error)) }
  finally { busy.value = false }
}
async function resetUsage(row: ApiToken) {
  if (busy.value) return
  try { await ElMessageBox.confirm(`将「${row.name}」已用次数清零？上限和成功/失败记录保留；正在运行的提取会继续计数。`, '重置次数', { type: 'warning' }) } catch { return }
  busy.value = true
  try { await tokenApi.resetUsage(row.id); await load() }
  catch (error) { ElMessage.error(tokenError(error)) }
  finally { busy.value = false }
}
async function copy() { if (await copyText(secret.value)) ElMessage.success('已复制'); else ElMessage.warning('自动复制失败，请选中文本手动复制') }
onMounted(load)
</script>
<style scoped>
.token-page { padding: 16px; }
header, footer { display: flex; align-items: center; gap: 12px; margin-bottom: 14px; }
header { justify-content: space-between; } h2 { font-size: 20px; margin: 0; }
.el-alert { margin-bottom: 12px; } .note { color: var(--el-text-color-secondary); font-size: 12px; overflow-wrap: anywhere; }
footer { margin-top: 12px; justify-content: flex-end; font-size: 13px; }
</style>
