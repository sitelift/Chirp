import { Component, type ReactNode } from 'react'

interface Props {
  children: ReactNode
  fallback: 'overlay' | 'settings'
}

interface State {
  hasError: boolean
  error: string | null
}

export class ErrorBoundary extends Component<Props, State> {
  state: State = { hasError: false, error: null }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error: error.message }
  }

  componentDidCatch(error: Error) {
    console.error(`ErrorBoundary [${this.props.fallback}]:`, error)
  }

  render() {
    if (!this.state.hasError) {
      return this.props.children
    }

    if (this.props.fallback === 'overlay') {
      return <OverlayFallback error={this.state.error} />
    }

    return (
      <SettingsFallback
        error={this.state.error}
        onReset={() => this.setState({ hasError: false, error: null })}
      />
    )
  }
}

function OverlayFallback(_props: { error: string | null }) {
  return (
    <div className="flex h-screen w-screen items-end justify-center pb-[80px]">
      <div className="flex h-12 items-center gap-3 rounded-full border border-red-300/30 bg-white/90 backdrop-blur-xl px-4">
        <span className="text-sm text-red-600">Something went wrong</span>
      </div>
    </div>
  )
}

function SettingsFallback({
  error,
  onReset,
}: {
  error: string | null
  onReset: () => void
}) {
  return (
    <div className="flex h-screen w-screen items-center justify-center bg-white">
      <div className="flex flex-col items-center gap-4 p-8">
        <p className="text-lg font-semibold text-chirp-stone-900">
          Something went wrong
        </p>
        {error && (
          <p className="text-sm text-chirp-stone-500 max-w-md text-center">
            {error}
          </p>
        )}
        <button
          onClick={onReset}
          className="rounded-lg bg-chirp-amber-400 px-4 py-2 text-sm font-medium text-white hover:bg-chirp-amber-500 transition-colors"
        >
          Try Again
        </button>
      </div>
    </div>
  )
}
