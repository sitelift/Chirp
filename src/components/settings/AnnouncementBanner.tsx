import { useState, useEffect } from 'react'
import { X, Info } from 'lucide-react'
import { invoke } from '@tauri-apps/api/core'

interface Announcement {
  id: string
  title: string
  body: string
}

export function AnnouncementBanner() {
  const [announcements, setAnnouncements] = useState<Announcement[]>([])

  useEffect(() => {
    invoke<Announcement[]>('get_announcements')
      .then(setAnnouncements)
      .catch(() => {})
  }, [])

  const dismiss = async (id: string) => {
    setAnnouncements((prev) => prev.filter((a) => a.id !== id))
    try {
      await invoke('dismiss_announcement', { id })
    } catch { /* fail silently */ }
  }

  if (announcements.length === 0) return null

  const announcement = announcements[0]

  return (
    <div className="rounded-lg border border-chirp-amber-200 bg-chirp-amber-50 p-3 mb-4">
      <div className="flex items-start gap-2">
        <Info size={16} className="text-chirp-amber-500 mt-0.5 shrink-0" />
        <div className="flex-1 min-w-0">
          <div className="text-[13px] font-medium text-chirp-stone-900">{announcement.title}</div>
          <div className="text-[12px] text-chirp-stone-500 mt-0.5">{announcement.body}</div>
        </div>
        <button
          onClick={() => dismiss(announcement.id)}
          className="text-chirp-stone-400 hover:text-chirp-stone-600 transition-colors shrink-0"
        >
          <X size={14} />
        </button>
      </div>
    </div>
  )
}
