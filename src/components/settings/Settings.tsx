import { useEffect } from 'react'
import { listen } from '@tauri-apps/api/event'
import { LayoutDashboard, Settings as SettingsIcon, Mic, Cpu, BookOpen, Zap, Info, FileAudio } from 'lucide-react'
import { useAppStore } from '../../stores/appStore'
import { useLlmDownloaded } from '../../hooks/useLlmDownloaded'
import { BirdMark } from '../shared/BirdMark'
import { HomePage } from './HomePage'
import { GeneralPage } from './GeneralPage'
import { AudioPage } from './AudioPage'
import { ModelPage } from './ModelPage'
import { DictionaryPage } from './DictionaryPage'
import { SnippetsPage } from './SnippetsPage'
import { AboutPage } from './AboutPage'
import { TranscribePage } from './TranscribePage'

const NAV_SECTIONS = [
  {
    label: 'MAIN',
    items: [
      { id: 'home', label: 'Home', icon: LayoutDashboard },
    ],
  },
  {
    label: 'SETTINGS',
    items: [
      { id: 'general', label: 'General', icon: SettingsIcon },
      { id: 'audio', label: 'Audio', icon: Mic },
      { id: 'model', label: 'Model', icon: Cpu },
    ],
  },
  {
    label: 'TOOLS',
    items: [
      { id: 'dictionary', label: 'Dictionary', icon: BookOpen },
      { id: 'snippets', label: 'Snippets', icon: Zap },
      { id: 'transcribe', label: 'Transcribe File', icon: FileAudio },
    ],
  },
]

const PAGES: Record<string, React.FC> = {
  home: HomePage,
  general: GeneralPage,
  audio: AudioPage,
  model: ModelPage,
  dictionary: DictionaryPage,
  snippets: SnippetsPage,
  about: AboutPage,
  transcribe: TranscribePage,
}

export function Settings() {
  const settingsPage = useAppStore((s) => s.settingsPage)
  const setSettingsPage = useAppStore((s) => s.setSettingsPage)
  const modelDownloaded = useAppStore((s) => s.modelDownloaded)
  const model = useAppStore((s) => s.model)
  const aiCleanup = useAppStore((s) => s.aiCleanup)
  const settingsSaved = useAppStore((s) => s.settingsSaved)
  const setSettingsSaved = useAppStore((s) => s.setSettingsSaved)
  const [llmDownloaded] = useLlmDownloaded()

  const sttReady = modelDownloaded[model]

  useEffect(() => {
    if (settingsSaved) {
      const timer = setTimeout(() => setSettingsSaved(false), 1500)
      return () => clearTimeout(timer)
    }
  }, [settingsSaved, setSettingsSaved])

  useEffect(() => {
    const unlisten = listen('check-for-updates', () => {
      setSettingsPage('about')
    })
    return () => { unlisten.then((f) => f()) }
  }, [setSettingsPage])

  const isReady = sttReady && (!aiCleanup || llmDownloaded)
  const statusLabel = !sttReady
    ? 'Model needed'
    : aiCleanup && !llmDownloaded
      ? 'Setup needed'
      : 'Ready'
  const PageComponent = PAGES[settingsPage] ?? HomePage

  return (
    <div className="flex h-screen no-select">
      {/* Sidebar */}
      <div className="flex w-56 shrink-0 flex-col border-r border-chirp-stone-200 bg-white py-4 px-2.5">
        {/* Logo lockup */}
        <div className="flex items-center gap-2 px-3 pb-2">
          <BirdMark size={24} />
          <span className="font-display font-extrabold text-[16px] text-chirp-stone-900 tracking-[-0.5px] leading-[1.2]">
            chirp
          </span>
        </div>

        {/* Status pill */}
        <div className="flex items-center justify-between gap-2 px-3 pb-4 mb-2 border-b border-chirp-stone-200">
          <div className="flex items-center gap-2">
            <div className={`w-2 h-2 rounded-full ${isReady ? 'bg-chirp-success' : 'bg-chirp-amber-400'}`} />
            <span className="font-body text-xs text-chirp-stone-500">
              {statusLabel}
            </span>
          </div>
          <span
            className={`font-body text-xs text-chirp-success transition-opacity duration-300 ${
              settingsSaved ? 'opacity-100' : 'opacity-0'
            }`}
          >
            Saved
          </span>
        </div>

        {/* Nav sections */}
        <nav className="flex flex-col flex-1">
          {NAV_SECTIONS.map((section) => (
            <div key={section.label}>
              <span className="text-[11px] font-semibold uppercase tracking-[0.8px] text-chirp-stone-400 px-3 mt-5 mb-1.5 block">
                {section.label}
              </span>
              <div className="flex flex-col gap-0.5">
                {section.items.map(({ id, label, icon: Icon }) => {
                  const active = settingsPage === id
                  return (
                    <button
                      key={id}
                      onClick={() => setSettingsPage(id)}
                      className={`flex h-9 items-center gap-2.5 rounded-lg px-3 text-sm font-body font-medium transition-colors duration-150 ease-out ${
                        active
                          ? 'bg-chirp-stone-100 text-chirp-stone-900 shadow-subtle'
                          : 'text-chirp-stone-500 hover:bg-chirp-stone-50'
                      }`}
                    >
                      <Icon size={18} strokeWidth={1.5} />
                      {label}
                    </button>
                  )
                })}
              </div>
            </div>
          ))}

          {/* Footer */}
          <div className="mt-auto border-t border-chirp-stone-200 pt-3">
            <button
              onClick={() => setSettingsPage('about')}
              className={`flex h-9 w-full items-center justify-between rounded-lg px-3 text-sm font-body font-medium transition-colors duration-150 ease-out ${
                settingsPage === 'about'
                  ? 'bg-chirp-stone-100 text-chirp-stone-900 shadow-subtle'
                  : 'text-chirp-stone-500 hover:bg-chirp-stone-50'
              }`}
            >
              <div className="flex items-center gap-2.5">
                <Info size={18} strokeWidth={1.5} />
                About
              </div>
              <span className="font-mono text-[11px] text-chirp-stone-400">v1.0.0</span>
            </button>
          </div>
        </nav>
      </div>

      {/* Content area */}
      <div className="flex-1 overflow-y-auto px-10 py-8 bg-chirp-stone-50">
        <div className="max-w-3xl">
          <PageComponent />
        </div>
      </div>
    </div>
  )
}
