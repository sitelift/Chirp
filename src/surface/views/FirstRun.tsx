import { IconOpen } from '../icons'
import { useSurface, useSurfaceContext } from '../store'
import { etaLabel, holdKeyLabel } from '../format'

const title = 'font-display text-title font-semibold text-t1'

/** No wizard: three cards, then the pill teaches itself (frame 10). */
export function FirstRun() {
  const { api } = useSurfaceContext()
  const step = useSurface((s) => s.firstRun)
  const download = useSurface((s) => s.download)
  const settings = useSurface((s) => s.settings)

  if (step === 'downloading') {
    return (
      <div className="flex w-[320px] flex-col gap-[14px] p-[22px]" role="status">
        <div className={title}>Getting ready</div>
        <div className="flex h-[4px] shrink-0 rounded-[2px] bg-track">
          <div className="h-[4px] rounded-[2px] bg-fill transition-[width] duration-300" style={{ width: `${Math.round(Math.min(100, Math.max(0, download.progress)))}%` }} />
        </div>
        <div className="flex justify-between text-meta text-t3">
          <span>Speech model</span>
          <span>{etaLabel(download.etaSec)}</span>
        </div>
      </div>
    )
  }

  if (step === 'mic-blocked') {
    return (
      <div className="flex w-[320px] flex-col gap-[16px] px-[22px] pb-[20px] pt-[22px]">
        <div className={title}>Windows is blocking the mic</div>
        <button
          type="button"
          onClick={() => api.call('open_mic_privacy_settings').catch(() => {})}
          className="flex h-[32px] items-center gap-[6px] self-start rounded-btn bg-btn pl-[14px] pr-[12px] font-display text-row font-semibold text-btn-text"
        >
          Open settings
          <IconOpen size={12} />
        </button>
      </div>
    )
  }

  return (
    <div className="flex w-[320px] flex-col p-[22px]">
      <div className="flex items-center gap-[8px]">
        <span className={title}>Hold</span>
        <span className="flex h-[28px] items-center rounded-btn border border-b-2 px-[10px] font-display text-[14px] font-semibold leading-[18px] text-t1" style={{ background: 'var(--key-bg)', borderColor: 'var(--key-border)' }}>
          {holdKeyLabel(settings.holdBinding, settings.toggleBinding)}
        </span>
        <span className={title}>and talk</span>
      </div>
    </div>
  )
}
