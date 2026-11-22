import { toPng } from 'html-to-image'

/** 把模板节点按原始尺寸（不受预览缩放影响）导出 PNG 并下载 */
export async function exportPng(node: HTMLElement, fileName: string) {
  const dataUrl = await toPng(node, {
    pixelRatio: 1,
    cacheBust: true,
    style: { transform: 'none' },
    filter: (n) => !(n instanceof HTMLElement && n.classList.contains('no-export')),
  })
  const a = document.createElement('a')
  a.href = dataUrl
  a.download = fileName
  a.click()
}
