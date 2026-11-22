import { useEffect, useState } from 'react'

export interface Tone { h: number; s: number; l: number }

/** 从封面图取主色（按饱和度加权平均色相），用来给当天的卡片定色调。取不到时返回 null。 */
export function useTone(url: string | null | undefined): Tone | null {
  const [tone, setTone] = useState<Tone | null>(null)
  useEffect(() => {
    if (!url) return
    const img = new Image()
    img.crossOrigin = 'anonymous'
    img.onload = () => {
      const c = document.createElement('canvas')
      c.width = 48; c.height = 36
      const g = c.getContext('2d')
      if (!g) return
      g.drawImage(img, 0, 0, 48, 36)
      const d = g.getImageData(0, 0, 48, 36).data
      let x = 0, y = 0, sw = 0, ls = 0, n = 0
      for (let i = 0; i < d.length; i += 4) {
        const [h, s, l] = rgbToHsl(d[i], d[i + 1], d[i + 2])
        const w = s * (1 - Math.abs(l - 0.5) * 1.6) // 偏好饱和、亮度适中的像素
        if (w > 0) {
          x += Math.cos((h * Math.PI) / 180) * w
          y += Math.sin((h * Math.PI) / 180) * w
          sw += w
        }
        ls += l; n++
      }
      const h = sw > 0 ? ((Math.atan2(y, x) * 180) / Math.PI + 360) % 360 : 220
      setTone({ h, s: Math.min(1, sw / n * 2.2), l: ls / n })
    }
    img.src = url
  }, [url])
  return tone
}

function rgbToHsl(r: number, g: number, b: number): [number, number, number] {
  r /= 255; g /= 255; b /= 255
  const max = Math.max(r, g, b), min = Math.min(r, g, b)
  const l = (max + min) / 2
  if (max === min) return [0, 0, l]
  const d = max - min
  const s = l > 0.5 ? d / (2 - max - min) : d / (max + min)
  const h = max === r ? (g - b) / d + (g < b ? 6 : 0) : max === g ? (b - r) / d + 2 : (r - g) / d + 4
  return [h * 60, s, l]
}
