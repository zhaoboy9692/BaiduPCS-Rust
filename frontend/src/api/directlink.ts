import axios from 'axios'
// Deliberately isolated from the upstream WebAuth / Baidu bearer interceptors.
const client = axios.create({ baseURL: '/direct-admin/v1', timeout: 15000 })
export interface ApiToken {
  id: string; name: string; note: string; display_prefix: string
  enabled: boolean; revoked_at: number | null; expires_at: number | null
  created_at: number; updated_at: number; last_used_at: number | null
  rate_per_minute: number; success_count: number; failure_count: number
}
export interface TokenInput { name: string; note: string; expires_at: number | null; rate_per_minute: number }
export const tokenApi = {
  async list(offset = 0, limit = 50): Promise<ApiToken[]> { return (await client.get('/tokens', { params: { offset, limit } })).data.data },
  async create(input: TokenInput): Promise<{ token: string; record: ApiToken }> { return (await client.post('/tokens', input)).data.data },
  async edit(id: string, input: Partial<TokenInput> & { enabled?: boolean }): Promise<ApiToken> { return (await client.patch(`/tokens/${encodeURIComponent(id)}`, input)).data.data },
  async revoke(id: string): Promise<ApiToken> { return (await client.post(`/tokens/${encodeURIComponent(id)}/revoke`)).data.data },
}
export function tokenError(error: unknown): string {
  if (axios.isAxiosError(error)) {
    if (typeof error.response?.data?.message === 'string') return error.response.data.message
    if (error.response?.status === 401) return '请先通过管理员认证，API Token 不能用于后台登录'
    if (error.response?.status === 404 || error.response?.status === 502) return 'Token 服务尚未部署或暂不可用'
  }
  return '操作失败，请检查服务后重试'
}

export interface DirectlinkInput { share_url: string; password?: string; selected_fs_ids?: number[] }
export interface DirectlinkFile { filename: string; size: number; url: string; headers: Record<string, string>; expires_at: number | null }
export interface DirectlinkResult { task_id: string; save_path: string; files: DirectlinkFile[] }
export const directlinkApi = {
  async resolve(input: DirectlinkInput): Promise<DirectlinkResult> {
    return (await client.post('/resolve', input, { timeout: 190000 })).data.data
  },
}
