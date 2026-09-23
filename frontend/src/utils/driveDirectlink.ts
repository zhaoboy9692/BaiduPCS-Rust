import { getFileList, getDownloadUrl, type FileItem, type FileListData } from '@/api/file'
import type { DirectlinkFile } from '@/api/directlink'
export const DOWNLOAD_UA = 'netdisk;P2SP;3.0.0.8;netdisk;11.12.3;ANG-AN00;android-android;10.0;JSbridge4.4.0;jointBridge;1.1.0;'
interface DriveApi {
  list: (dir: string, page: number, pageSize: number) => Promise<FileListData>
  link: (id: number, path: string) => Promise<string>
}
export const isPlainFile = (file: FileItem) => file.isdir === 0 && !file.is_encrypted && !file.is_encrypted_folder
function validPath(path: string) {
  return path.startsWith('/') && !path.includes('\\') && !/[\x00-\x1f\x7f]/.test(path) && path.slice(1).split('/').every(p => p && p !== '.' && p !== '..')
}
// Validate a whole page before using any entry. Reject repeated pages instead of looping.
export function validateDrivePage(list: FileItem[], dir: string, seen: Set<string>) {
  const page = new Set<string>()
  for (const item of list) {
    if (!validPath(item.path) || item.path.slice(0, item.path.lastIndexOf('/')) !== dir.replace(/\/$/, '') ||
      item.server_filename !== item.path.split('/').pop() || !item.fs_id || ![0, 1].includes(item.isdir) ||
      seen.has(item.path) || page.has(item.path)) throw new Error('目录返回了重复或不属于当前目录的文件，请刷新后重试')
    page.add(item.path)
  }
  page.forEach(p => seen.add(p))
}
export async function* extractDriveLinks(
  items: FileItem[], recursive: boolean, signal: AbortSignal,
  api: DriveApi = { list: getFileList, link: getDownloadUrl },
): AsyncGenerator<DirectlinkFile> {
  const queue = [...items]
  const visited = new Set<string>()
  for (let index = 0; index < queue.length; index++) {
    if (signal.aborted) return
    const file = queue[index]
    if (visited.has(file.path)) continue
    visited.add(file.path)
    const result: DirectlinkFile = { fs_id: file.fs_id, name: file.server_filename, path: file.path,
      is_dir: file.isdir === 1, size: file.size, success: false, url: null, headers: {}, expires_at: null,
      task_id: null, save_path: null, error: null }
    try {
      if (!validPath(file.path) || !file.fs_id) throw new Error('文件路径或标识无效')
      if (file.is_encrypted || file.is_encrypted_folder) throw new Error('加密文件不能使用普通直链，请使用原下载功能解密')
      if (file.isdir === 1) {
        if (!recursive) throw new Error('请进入目录选择文件，或确认提取整个目录')
        const seen = new Set<string>()
        for (let page = 1; ; page++) {
          if (signal.aborted) return
          const data = await api.list(file.path, page, 100)
          if (signal.aborted) return
          if (data.has_more && !data.list.length) throw new Error('目录分页异常，请刷新后重试')
          validateDrivePage(data.list, file.path, seen)
          queue.push(...data.list)
          if (!data.has_more) break
        }
        continue
      }
      const url = await api.link(file.fs_id, file.path)
      if (signal.aborted) return
      const parsed = new URL(url)
      if (!['https:', 'http:'].includes(parsed.protocol) || !parsed.hostname || parsed.username || parsed.password) throw new Error('服务器返回的下载链接无效')
      result.url = url; result.success = true; result.headers = { 'User-Agent': DOWNLOAD_UA }
    } catch (error) {
      if (signal.aborted) return
      result.error = { code: 'drive_extraction_failed', message: error instanceof Error ? error.message : '提取失败，请重试' }
    }
    yield result
  }
}
const shellQuote = (value: string) => `'${value.replace(/'/g, `'"'"'`)}'`
export function downloadCommand(file: Pick<DirectlinkFile, 'name' | 'url' | 'headers'>): string {
  // A basename only: copied command must not overwrite files outside the working directory.
  const name = file.name.split(/[\\/]/).pop() || 'download.bin'
  const safeName = name === '.' || name === '..' ? 'download.bin' : name
  return `curl --fail --location --show-error --user-agent ${shellQuote(file.headers['User-Agent'] || DOWNLOAD_UA)} --output ${shellQuote(safeName)} -- ${shellQuote(file.url || '')}`
}
