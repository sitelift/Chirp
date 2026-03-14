import { Settings as SettingsIcon, Mic, Box, BookOpen, Info } from 'lucide-react'
import { useAppStore } from '../../stores/appStore'
import { BirdMark } from '../shared/BirdMark'
import { GeneralPage } from './GeneralPage'
import { AudioPage } from './AudioPage'
import { ModelPage } from './ModelPage'
import { DictionaryPage } from './DictionaryPage'
import { AboutPage } from './AboutPage'

const NAV_ITEMS = [
  { id: 'general', label: 'General', icon: SettingsIcon },
  { id: 'audio', label: 'Audio', icon: Mic },
  { id: 'model', label: 'Model', icon: Box },
  { id: 'dictionary', label: 'Dictionary', icon: BookOpen },
  { id: 'about', label: 'About', icon: Info },
]

const PAGES: Record<string, React.FC> = {
  general: GeneralPage,
  audio: AudioPage,
  model: ModelPage,
  dictionary: DictionaryPage,
  about: AboutPage,
}

export function Settings() {
  const settingsPage = useAppStore((s) => s.settingsPage)
  const setSettingsPage = useAppStore((s) => s.setSettingsPage)

  const PageComponent = PAGES[settingsPage] ?? GeneralPage

  return (
    <div className="flex h-screen no-select">
      {/* Sidebar */}
      <div className="flex w-40 shrink-0 flex-col border-r border-chirp-stone-200 bg-chirp-stone-100 px-2 py-4">
        {/* Logo lockup */}
        <div className="flex items-center gap-2 px-2 pb-4 mb-4 border-b border-chirp-stone-200">
          <BirdMark size={24} />
          <span className="font-display font-extrabold text-[16px] text-chirp-stone-900 tracking-[-0.5px] leading-[1.2]">
            chirp
          </span>
        </div>

        {/* Nav items */}
        <nav className="flex flex-col gap-1">
          {NAV_ITEMS.map(({ id, label, icon: Icon }) => {
            const active = settingsPage === id
            return (
              <button
                key={id}
                onClick={() => setSettingsPage(id)}
                className={`flex h-9 items-center gap-2.5 rounded-lg px-3 text-sm font-body font-medium transition-colors duration-150 ease-out ${
                  active
                    ? 'bg-white text-chirp-stone-900 shadow-subtle'
                    : 'text-chirp-stone-500 hover:bg-chirp-stone-200'
                }`}
              >
                <Icon size={18} strokeWidth={1.5} />
                {label}
              </button>
            )
          })}
        </nav>
      </div>

      {/* Content area */}
      <div className="flex-1 overflow-y-auto p-8">
        <PageComponent />
      </div>
    </div>
  )
}
