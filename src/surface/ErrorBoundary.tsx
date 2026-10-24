import { Component, type ReactNode } from 'react'

/** A crash shows a quiet ink pill instead of a blank window. */
export class ErrorBoundary extends Component<{ children: ReactNode }, { error: string | null }> {
  state = { error: null as string | null }

  static getDerivedStateFromError(error: Error) {
    return { error: error.message }
  }

  componentDidCatch(error: Error) {
    console.error('Chirp surface crashed:', error)
  }

  render() {
    if (this.state.error === null) return this.props.children
    return (
      <div className="fixed inset-0 flex items-end justify-center pb-[12px]">
        <button
          type="button"
          onClick={() => location.reload()}
          className="flex h-[36px] items-center rounded-[18px] bg-ink px-[16px] font-display text-row font-semibold text-white shadow-pill"
        >
          Something went wrong · Reload
        </button>
      </div>
    )
  }
}
