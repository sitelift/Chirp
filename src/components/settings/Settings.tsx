import { useEffect } from 'react'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { Home, BookOpen, Zap, Settings as SettingsIcon, Check, Minus, Square, X } from 'lucide-react'
import type { LucideIcon } from 'lucide-react'
import { useAppStore } from '../../stores/appStore'
import { BirdMark } from '../shared/BirdMark'
import { KeyBadge } from '../shared/KeyBadge'
import { AboutModal } from '../shared/AboutModal'
import { formatHotkey } from '../../lib/utils'
import { HomePage } from './HomePage'
import { DictionaryPage } from './DictionaryPage'
import { SnippetsPage } from './SnippetsPage'
import { SettingsPage } from './SettingsPage'

const NAV_ITEMS: { id: string; label: string; icon: LucideIcon }[] = [
  { id: 'home', label: 'Home', icon: Home },
  { id: 'dictionary', label: 'Dictionary', icon: BookOpen },
  { id: 'snippets', label: 'Snippets', icon: Zap },
]

const PAGES: Record<string, React.FC> = {
  home: HomePage,
  dictionary: DictionaryPage,
  snippets: SnippetsPage,
  settings: SettingsPage,
}

export function Settings() {
  const settingsPage = useAppStore((s) => s.settingsPage)
  const setSettingsPage = useAppStore((s) => s.setSettingsPage)
  const settingsSaved = useAppStore((s) => s.settingsSaved)
  const setSettingsSaved = useAppStore((s) => s.setSettingsSaved)
  const hotkey = useAppStore((s) => s.hotkey)
  const hotkeyMode = useAppStore((s) => s.hotkeyMode)
  const hotkeyKeyName = useAppStore((s) => s.hotkeyKeyName)
  const aboutModalOpen = useAppStore((s) => s.aboutModalOpen)
  const setAboutModalOpen = useAppStore((s) => s.setAboutModalOpen)

  useEffect(() => {
    if (settingsSaved) {
      const timer = setTimeout(() => setSettingsSaved(false), 1500)
      return () => clearTimeout(timer)
    }
  }, [settingsSaved, setSettingsSaved])

  useEffect(() => {
    const unlisten = listen('check-for-updates', () => {
      setAboutModalOpen(true)
    })
    return () => { unlisten.then((f) => f()) }
  }, [setAboutModalOpen])

  const hotkeyKeys = hotkeyMode === 'dedicated_key'
    ? [hotkeyKeyName]
    : formatHotkey(hotkey)

  const PageComponent = PAGES[settingsPage] ?? HomePage

  return (
    <div className="flex flex-col h-screen overflow-hidden no-select">
      {/* Custom titlebar */}
      <div data-tauri-drag-region className="flex items-center justify-end h-8 shrink-0 bg-sidebar">
        <button
          onClick={() => getCurrentWindow().minimize()}
          className="w-[46px] h-full flex items-center justify-center text-white/40 hover:text-white/60 transition-colors"
        >
          <Minus size={16} />
        </button>
        <button
          onClick={() => getCurrentWindow().toggleMaximize()}
          className="w-[46px] h-full flex items-center justify-center text-white/40 hover:text-white/60 transition-colors"
        >
          <Square size={12} />
        </button>
        <button
          onClick={() => getCurrentWindow().close()}
          className="w-[46px] h-full flex items-center justify-center text-white/40 hover:bg-red-500 hover:text-white transition-colors"
        >
          <X size={16} />
        </button>
      </div>

      <div className="flex flex-1 min-h-0">
      {/* Dark sidebar */}
      <div className="flex w-[220px] shrink-0 flex-col bg-sidebar p-[20px_12px] relative overflow-hidden sidebar-noise sidebar-glow">
        {/* Logo */}
        <div className="flex items-center gap-[10px] px-[10px] mb-8 relative z-10">
          <div className="w-8 h-8 rounded-[9px] bg-chirp-yellow flex items-center justify-center shadow-logo-glow">
            <BirdMark size={18} color="white" />
          </div>
          <span className="font-display font-black text-xl text-white tracking-[-0.5px]">
            chirp
          </span>
        </div>

        {/* Nav items */}
        <nav className="flex flex-col gap-0.5 relative z-10">
          {NAV_ITEMS.map(({ id, label, icon: Icon }) => {
            const active = settingsPage === id
            return (
              <button
                key={id}
                onClick={() => setSettingsPage(id)}
                className={`flex items-center gap-[10px] px-[14px] py-[10px] rounded-lg text-[13px] transition-all duration-200 relative ${
                  active
                    ? 'text-chirp-yellow font-semibold bg-chirp-yellow/[0.08]'
                    : 'text-white/40 hover:text-white/70 hover:bg-white/[0.04]'
                }`}
              >
                {active && (
                  <span className="absolute left-0 top-1/2 -translate-y-1/2 w-[3px] h-4 bg-chirp-yellow rounded-r-sm" />
                )}
                <Icon size={16} strokeWidth={1.5} />
                {label}
              </button>
            )
          })}
        </nav>

        {/* Footer */}
        <div className="mt-auto relative z-10">
          {/* Settings nav item */}
          <button
            onClick={() => setSettingsPage('settings')}
            className={`flex items-center gap-[10px] w-full px-[14px] py-[10px] rounded-lg text-[13px] transition-all duration-200 relative mb-3 ${
              settingsPage === 'settings'
                ? 'text-chirp-yellow font-semibold bg-chirp-yellow/[0.08]'
                : 'text-white/40 hover:text-white/70 hover:bg-white/[0.04]'
            }`}
          >
            {settingsPage === 'settings' && (
              <span className="absolute left-0 top-1/2 -translate-y-1/2 w-[3px] h-4 bg-chirp-yellow rounded-r-sm" />
            )}
            <SettingsIcon size={16} strokeWidth={1.5} />
            Settings
          </button>

          {/* Hotkey card */}
          <div className="mx-1 p-[14px] bg-white/[0.05] rounded-[10px] border border-white/[0.06] backdrop-blur-sm">
            <div className="text-[10px] text-white/30 font-semibold uppercase tracking-[1px] mb-2">
              Hold to dictate
            </div>
            <div className="flex gap-1">
              {hotkeyKeys.map((key) => (
                <KeyBadge key={key} keyLabel={key} variant="glass" />
              ))}
            </div>
          </div>

          {/* Version / About */}
          <button
            onClick={() => setAboutModalOpen(true)}
            className="w-full text-center mt-3 text-[10px] text-white/[0.15] hover:text-white/30 transition-colors"
          >
            v1.0.0
          </button>
        </div>
      </div>

      {/* Content area */}
      <div className="flex-1 overflow-y-auto px-8 py-7 bg-surface">
        <div className="max-w-5xl mx-auto">
          <div key={settingsPage} className="animate-fade-in">
            <PageComponent />
          </div>
        </div>
      </div>
      </div>

      {/* Saved indicator */}
      {settingsSaved && (
        <div className="fixed bottom-5 right-5 flex items-center gap-[6px] px-4 py-2 bg-[#1a1a1a] text-white rounded-lg text-xs font-medium shadow-elevated animate-saved-pop z-50">
          <Check size={14} className="text-chirp-success" /> Saved
        </div>
      )}

      {/* About modal */}
      {aboutModalOpen && <AboutModal />}
    </div>
  )
}
