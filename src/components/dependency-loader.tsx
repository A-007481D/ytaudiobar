interface DependencyLoaderProps {
    status: 'checking' | 'downloading-ytdlp' | 'downloading-ffmpeg' | 'complete'
    progress: number // 0-100 real percentage
}

export function DependencyLoader({ status, progress }: DependencyLoaderProps) {
    const getMessage = () => {
        switch (status) {
            case 'checking':
                return 'Checking dependencies...'
            case 'downloading-ytdlp':
                return 'Downloading yt-dlp...'
            case 'downloading-ffmpeg':
                return 'Downloading ffmpeg...'
            case 'complete':
                return 'Ready!'
            default:
                return 'Initializing...'
        }
    }

    return (
        <div className="flex h-screen items-center justify-center bg-background select-none rounded-[12px] overflow-hidden border border-white/10">
            <div className="w-72 space-y-5">
                {/* App Icon + Name */}
                <div className="flex items-center gap-2">
                    <img src="/icon.png" alt="YTAudioBar" className="w-5 h-5" />
                    <span className="text-[15px] font-semibold text-foreground">
                        YTAudioBar
                    </span>
                </div>

                {/* Status */}
                <p className="text-[13px] text-muted-foreground">
                    {getMessage()}
                </p>

                {/* Progress Bar */}
                <div className="space-y-1.5">
                    <div className="h-1.5 w-full bg-secondary rounded-full overflow-hidden">
                        <div
                            className="h-full bg-[var(--macos-blue)] rounded-full transition-all duration-300 ease-out"
                            style={{ width: `${Math.round(progress)}%` }}
                        />
                    </div>
                    <p className="text-[11px] text-muted-foreground text-right">
                        {Math.round(progress)}%
                    </p>
                </div>
            </div>
        </div>
    )
}
