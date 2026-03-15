import { X, Music, Minus, Move, Shrink, Expand } from 'lucide-react'
import { getCurrentWindow } from '@tauri-apps/api/window'

interface AppHeaderProps {
    query: string
    onQueryChange: (query: string) => void
    isMusicMode: boolean
    isShrinked: boolean
    onMusicModeToggle: () => void
    onIsShrinkedToggle: () => void
    onResetWindow: () => void
}

export function AppHeader({
    query,
    onQueryChange,
    isMusicMode,
    isShrinked,
    onMusicModeToggle,
    onIsShrinkedToggle,
    onResetWindow
}: AppHeaderProps) {
    return (
        <div className="flex-shrink-0 bg-background">
            {/* App Title Section - Draggable area */}
            <div
                className="px-4 pt-4 pb-3 flex items-center gap-2 cursor-grab active:cursor-grabbing select-none"
                onMouseDown={(e) => {
                    if (e.button === 0) getCurrentWindow().startDragging()
                }}
            >
                <img src="/icon.png" alt="YTAudioBar" className="w-5 h-5" />
                <h1 className="text-[15px] font-semibold text-foreground">
                    YTAudioBar
                </h1>
                <div
                    className="ml-auto flex items-center gap-1"
                    onMouseDown={(e) => e.stopPropagation()}
                >
                    <button
                        onClick={onIsShrinkedToggle}
                        className="w-6 h-6 flex items-center justify-center rounded hover:bg-secondary text-muted-foreground hover:text-foreground transition-colors"
                        title={isShrinked ? 'Expand' : 'Shrink'}
                    >
                        {isShrinked ? (
                            <Expand className="w-4 h-4" />
                        ) : (
                            <Shrink className="w-4 h-4" />
                        )}
                    </button>
                    <button
                        onClick={onResetWindow}
                        className="w-6 h-6 flex items-center justify-center rounded hover:bg-secondary text-muted-foreground hover:text-foreground transition-colors"
                        title="Reset position & size"
                    >
                        <Move className="w-4 h-4" />
                    </button>
                    <button
                        onClick={() => getCurrentWindow().minimize()}
                        className="w-6 h-6 flex items-center justify-center rounded hover:bg-secondary text-muted-foreground hover:text-foreground transition-colors"
                        title="Minimize"
                    >
                        <Minus className="w-4 h-4" />
                    </button>
                </div>
            </div>

            {!isShrinked && (
                /* Search Bar Section */
                <div className="px-4 pb-3">
                    <div className="relative">
                        <input
                            type="text"
                            value={query}
                            onChange={(e) => onQueryChange(e.target.value)}
                            placeholder={
                                isMusicMode
                                    ? 'Search YouTube Music...'
                                    : 'Search YouTube...'
                            }
                            className="w-full px-3 py-2 pr-24 bg-secondary border-none rounded-lg text-[14px] text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-[var(--macos-blue)]"
                        />

                        {/* Right side buttons */}
                        <div className="absolute right-2 top-1/2 -translate-y-1/2 flex items-center gap-1">
                            {query && (
                                <button
                                    onClick={() => onQueryChange('')}
                                    className="w-5 h-5 flex items-center justify-center rounded-full hover-macos-button"
                                >
                                    <X className="w-3.5 h-3.5 text-muted-foreground" />
                                </button>
                            )}

                            {/* Music Mode Toggle */}
                            <button
                                onClick={onMusicModeToggle}
                                className={`w-6 h-6 flex items-center justify-center rounded transition-colors ${
                                    isMusicMode
                                        ? 'text-[var(--macos-blue)]'
                                        : 'text-muted-foreground hover:text-foreground'
                                }`}
                                title={
                                    isMusicMode ? 'YouTube Music' : 'YouTube'
                                }
                            >
                                <Music className="w-4 h-4" />
                            </button>
                        </div>
                    </div>
                </div>
            )}
        </div>
    )
}
