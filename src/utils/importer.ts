/**
 * 公共书籍导入：解析元数据 → 去重入架 → 缓存封面。
 * 书架本地导入与"下载完成后自动加入书架"共用此实现，保证行为一致。
 */
import { useConfigStore } from '../stores/config'
import { useBookStore } from '../stores/books'
import { parseTxt } from './txtParser'
import { parseEpub } from './epubParser'
import { parseMobi } from './mobiParser'
import { platform } from '../platform'
import { saveCover } from './db'

export interface ImportResult {
  ok: boolean
  title?: string
  /** 失败原因或"已在书架中"等提示。 */
  message?: string
}

function randomCoverColor(): string {
  const colors = [
    '#4a7fa5', '#5c7a6e', '#7a5c6e', '#7a6e5c',
    '#5c6e7a', '#6e7a5c', '#7a5c5c', '#5c5c7a'
  ]
  return colors[Math.floor(Math.random() * colors.length)]
}

function makeFile(name: string, content: Uint8Array<ArrayBuffer>, mime: string): File {
  const blob = new Blob([content], { type: mime })
  return new File([blob], name)
}

/**
 * 从本地文件路径导入一本书到书架。
 * 支持的格式：EPUB / TXT / MOBI（与本地导入一致）。
 * 不抛异常：成功返回 { ok: true }；失败返回 { ok: false, message }（重复返回 ok:false 的提示）。
 */
export async function importBookFromFile(filePath: string): Promise<ImportResult> {
  if (!filePath) return { ok: false, message: '文件路径为空' }
  const name = filePath.split(/[\\/]/).pop() ?? ''
  const isEpub = /\.epub$/i.test(name)
  const isTxt = /\.txt$/i.test(name)
  const isMobi = /\.mobi$/i.test(name)
  if (!isEpub && !isTxt && !isMobi) {
    return { ok: false, message: '仅支持 EPUB、TXT 和 MOBI 格式' }
  }

  const configStore = useConfigStore()
  const bookStore = useBookStore()

  let title = name.replace(/\.(epub|txt|mobi)$/i, '')
  let author = ''
  let description = ''
  let coverColor = randomCoverColor()
  let coverImage: string | undefined

  if (isEpub) {
    try {
      const content = await platform.readFileBinary(filePath).catch(() => null)
      if (content) {
        const file = makeFile(name, content, 'application/epub+zip')
        const result = await parseEpub(file)
        title = result.title || title
        author = result.author || ''
        description = result.description || ''
        if (result.coverUrl && !configStore.config.other.plainTextCover) coverImage = result.coverUrl
      }
    } catch { }
  }

  if (isMobi) {
    try {
      const content = await platform.readFileBinary(filePath).catch(() => null)
      if (content) {
        const file = makeFile(name, content, 'application/x-mobipocket-ebook')
        const result = await parseMobi(file)
        if (result.error) return { ok: false, message: `MOBI解析失败：${result.error}` }
        title = result.title || title
        author = result.author || ''
        description = result.description || ''
        if (result.coverUrl && !configStore.config.other.plainTextCover) coverImage = result.coverUrl
      }
    } catch (e: any) {
      return { ok: false, message: `MOBI导入失败：${e.message}` }
    }
  }

  const fileModifiedAt = await platform.getFileModifiedTime(filePath).catch(() => null)

  const book = bookStore.addBook({
    title,
    author,
    description: description || undefined,
    format: isEpub ? 'epub' : isMobi ? 'mobi' : 'txt',
    filePath,
    coverColor,
    coverImage,
    fileModifiedAt
  })

  if (book) {
    if (coverImage) saveCover(book.id, coverImage).catch(() => { })
    return { ok: true, title }
  }
  return { ok: false, message: '该书籍已在书架中' }
}
