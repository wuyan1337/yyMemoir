import { toBlob, toPng } from 'html-to-image'

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

/** 复制成图片到剪贴板（可以直接粘贴到 QQ / 微信） */
export async function copyPng(node: HTMLElement) {
  const blob = await toBlob(node, {
    pixelRatio: 1,
    cacheBust: true,
    style: { transform: 'none' },
    filter: (n) => !(n instanceof HTMLElement && n.classList.contains('no-export')),
  })
  if (!blob) throw new Error('生成图片失败')
  await navigator.clipboard.write([new ClipboardItem({ 'image/png': blob })])
}
