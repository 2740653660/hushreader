/// <reference types="vite/client" />

declare module '*.vue' {
  import type { DefineComponent } from 'vue'
  const component: DefineComponent<Record<string, never>, Record<string, never>, unknown>
  export default component
}

declare global {
  /** 为兼容旧解析代码提供的最小 Buffer 形状（Blob 构造接受 Uint8Array）。 */
  interface Buffer extends Uint8Array {
    toString(encoding?: string): string
  }
  var Buffer: {
    from(data: ArrayLike<number>): Buffer
  }
}

export { }
