/**
 * 封面/章节等大对象的本地存储。
 *
 * 旧版依赖 ZTools 提供的 IndexedDB 封装；Tauri 的 WebView2 自带 IndexedDB
 * （数据持久化在应用数据目录），这里改为直接使用原生 IndexedDB，对外接口保持不变。
 */

let dbPromise: Promise<IDBDatabase> | null = null

function openDb(): Promise<IDBDatabase> {
  if (!dbPromise) {
    dbPromise = new Promise((resolve, reject) => {
      const req = indexedDB.open('hushreader', 1)
      req.onupgradeneeded = () => {
        const db = req.result
        if (!db.objectStoreNames.contains('kv')) {
          db.createObjectStore('kv')
        }
      }
      req.onsuccess = () => resolve(req.result)
      req.onerror = () => reject(req.error)
    })
  }
  return dbPromise
}

async function dbGet<T>(key: string): Promise<T | null> {
  try {
    const db = await openDb()
    return await new Promise<T | null>((resolve, reject) => {
      const tx = db.transaction('kv', 'readonly')
      const req = tx.objectStore('kv').get(key)
      req.onsuccess = () => resolve((req.result as T | undefined) ?? null)
      req.onerror = () => reject(req.error)
    })
  } catch {
    return null
  }
}

async function dbPut(key: string, value: unknown): Promise<void> {
  try {
    const db = await openDb()
    await new Promise<void>((resolve, reject) => {
      const tx = db.transaction('kv', 'readwrite')
      tx.objectStore('kv').put(value, key)
      tx.oncomplete = () => resolve()
      tx.onerror = () => reject(tx.error)
    })
  } catch (e) {
    console.warn('[db] put failed', e)
  }
}

async function dbRemove(key: string): Promise<void> {
  try {
    const db = await openDb()
    await new Promise<void>((resolve, reject) => {
      const tx = db.transaction('kv', 'readwrite')
      tx.objectStore('kv').delete(key)
      tx.oncomplete = () => resolve()
      tx.onerror = () => reject(tx.error)
    })
  } catch (e) {
    console.warn('[db] remove failed', e)
  }
}

export async function saveCover(bookId: string, coverData: string) {
  await dbPut(`cover_${bookId}`, { data: coverData })
}

export async function loadCover(bookId: string): Promise<string | null> {
  const doc = await dbGet<{ data: string }>(`cover_${bookId}`)
  return doc?.data ?? null
}

export async function removeCover(bookId: string) {
  await dbRemove(`cover_${bookId}`)
}

export async function saveCustomCover(bookId: string, coverData: string) {
  await dbPut(`custom_cover_${bookId}`, { data: coverData })
}

export async function loadCustomCover(bookId: string): Promise<string | null> {
  const doc = await dbGet<{ data: string }>(`custom_cover_${bookId}`)
  return doc?.data ?? null
}

export async function removeCustomCover(bookId: string) {
  await dbRemove(`custom_cover_${bookId}`)
}

export async function saveChapters(bookId: string, chapters: any[]) {
  await dbPut(`chapters_${bookId}`, { data: chapters })
}

export async function loadChapters(bookId: string): Promise<any[] | null> {
  const doc = await dbGet<{ data: any[] }>(`chapters_${bookId}`)
  return Array.isArray(doc?.data) ? doc.data : null
}

export async function removeChapters(bookId: string) {
  await dbRemove(`chapters_${bookId}`)
}

export async function removeBookData(bookId: string) {
  await Promise.allSettled([
    removeCover(bookId),
    removeCustomCover(bookId),
    removeChapters(bookId)
  ])
}

export async function loadAllCovers(bookIds: string[]): Promise<Record<string, { cover?: string; customCover?: string }>> {
  const result: Record<string, { cover?: string; customCover?: string }> = {}
  await Promise.allSettled(
    bookIds.map(async (id) => {
      const [cover, customCover] = await Promise.all([
        loadCover(id),
        loadCustomCover(id)
      ])
      const entry: { cover?: string; customCover?: string } = {}
      if (cover) entry.cover = cover
      if (customCover) entry.customCover = customCover
      if (cover || customCover) result[id] = entry
    })
  )
  return result
}
