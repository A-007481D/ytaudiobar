import { useState, useEffect, useRef } from 'react'
import { AppHeader } from '@/components/app-header'
import { DependencyLoader } from '@/components/dependency-loader'
import { MiniPlayer } from '@/features/player/mini-player'
import { ExpandedPlayer } from '@/features/player/expanded-player'
import { SearchTab } from '@/features/search/search-tab'
import { QueueTab } from '@/features/queue/queue-tab'
import { PlaylistsTab } from '@/features/playlists/playlists-tab'
import { DownloadsTab } from '@/features/downloads/downloads-tab'
import { SettingsTab } from '@/features/settings/settings-tab'
import { usePlayerStore } from '@/stores/player-store'
import {
    checkYtdlpInstalled,
    installYtdlp,
    checkFfmpegAvailable,
    installFfmpeg,
    listenToDepProgress,
    listenToPlaybackState,
    searchYoutube,
    cancelSearch,
    togglePlayPause,
    playNext as playNextTrack,
    playPrevious as playPreviousTrack,
    seekTo,
    updateMediaPlaybackState,
    clearMediaInfo,
    listenToMediaKeyToggle,
    listenToMediaKeyNext,
    listenToMediaKeyPrevious,
    listenToMediaKeyPlay,
    listenToMediaKeyPause,
    listenToMediaKeySeek,
    listenToMediaKeySeekTo,
    type AudioState,
    type YTVideoInfo
} from '@/lib/tauri'

type TabName = 'search' | 'queue' | 'playlists' | 'downloads' | 'settings'

export function HomePage() {
    const [activeTab, setActiveTab] = useState<TabName>('search')
    const [isExpanded, setIsExpanded] = useState(false)
    const [currentTrack, setCurrentTrack] = useState<YTVideoInfo | null>(null)
    const [isPlaying, setIsPlaying] = useState(false)
    const [audioState, setAudioState] = useState<AudioState | null>(null)
    const [isInitializing, setIsInitializing] = useState(true)
    const [loadingStatus, setLoadingStatus] = useState<
        'checking' | 'downloading-ytdlp' | 'downloading-ffmpeg' | 'complete'
    >('checking')
    const [loadingProgress, setLoadingProgress] = useState(0)
    const needsYtdlpRef = useRef(false)
    const needsFfmpegRef = useRef(false)

    // Get Zustand store actions
    const {
        setCurrentTrack: setStoreTrack,
        setIsPlaying: setStorePlaying,
        setLoadingTrack
    } = usePlayerStore()

    // Search state (lifted from SearchTab to be accessible from Header)
    const [searchQuery, setSearchQuery] = useState('')
    const [isMusicMode, setIsMusicMode] = useState(false)
    const [searchResults, setSearchResults] = useState<YTVideoInfo[]>([])
    const [isSearching, setIsSearching] = useState(false)
    const [searchTimeout, setSearchTimeout] = useState<NodeJS.Timeout | null>(
        null
    )
    const searchRequestIdRef = useRef(0) // Track current search request to cancel stale requests

    // Initialize dependencies (yt-dlp + ffmpeg)
    useEffect(() => {
        const initDependencies = async () => {
            try {
                // Check what needs installing
                const ytdlpInstalled = await checkYtdlpInstalled()
                const ffmpegAvailable = await checkFfmpegAvailable()

                needsYtdlpRef.current = !ytdlpInstalled
                needsFfmpegRef.current = !ffmpegAvailable

                // If everything is already installed, skip immediately
                if (ytdlpInstalled && ffmpegAvailable) {
                    setIsInitializing(false)
                    return
                }

                // Listen for real download progress events
                const unlisten = await listenToDepProgress((progress) => {
                    if (progress.total === 0) return

                    const depPercent =
                        (progress.downloaded / progress.total) * 100
                    const bothNeeded =
                        needsYtdlpRef.current && needsFfmpegRef.current

                    if (progress.dependency === 'ytdlp') {
                        // yt-dlp: 0-50% if both needed, 0-100% if only ytdlp
                        const overall = bothNeeded
                            ? depPercent * 0.5
                            : depPercent
                        setLoadingProgress(overall)
                    } else if (progress.dependency === 'ffmpeg') {
                        // ffmpeg: 50-100% if both needed, 0-100% if only ffmpeg
                        const overall = bothNeeded
                            ? 50 + depPercent * 0.5
                            : depPercent
                        setLoadingProgress(overall)
                    }
                })

                // Install yt-dlp if needed
                if (!ytdlpInstalled) {
                    setLoadingStatus('downloading-ytdlp')
                    await installYtdlp()
                }

                // Install ffmpeg if needed
                if (!ffmpegAvailable) {
                    setLoadingStatus('downloading-ffmpeg')
                    await installFfmpeg()
                }

                unlisten()
                setIsInitializing(false)
            } catch (error) {
                console.error('Failed to initialize dependencies:', error)
                setIsInitializing(false)
            }
        }
        initDependencies()
    }, [])

    // Listen to playback state changes
    useEffect(() => {
        const unlisten = listenToPlaybackState((state) => {
            setAudioState(state)
            setIsPlaying(state.is_playing)
            setStorePlaying(state.is_playing)
            if (state.current_track) {
                setCurrentTrack(state.current_track)
                setStoreTrack(state.current_track)
                // Update loading state based on backend
                if (state.is_loading) {
                    setLoadingTrack(state.current_track.id)
                } else {
                    setLoadingTrack(null)
                }
            }
        })

        return () => {
            unlisten.then((fn) => fn())
        }
    }, [setStoreTrack, setStorePlaying, setLoadingTrack])

    // Update media info when track or playback state changes
    useEffect(() => {
        if (audioState && audioState.current_track) {
            // Note: updateMediaMetadata is now called from backend directly to avoid race conditions
            // Only update playback state from frontend
            updateMediaPlaybackState(
                audioState.is_playing,
                audioState.current_position,
                audioState.duration
            ).catch(console.error)
        } else {
            clearMediaInfo().catch(console.error)
        }
    }, [audioState])

    // Listen to media key events
    useEffect(() => {
        const unlisteners: Promise<() => void>[] = []

        // Play/Pause/Toggle
        unlisteners.push(
            listenToMediaKeyToggle(() => {
                togglePlayPause().catch(console.error)
            })
        )

        unlisteners.push(
            listenToMediaKeyPlay(() => {
                if (!isPlaying) {
                    togglePlayPause().catch(console.error)
                }
            })
        )

        unlisteners.push(
            listenToMediaKeyPause(() => {
                if (isPlaying) {
                    togglePlayPause().catch(console.error)
                }
            })
        )

        // Next/Previous
        unlisteners.push(
            listenToMediaKeyNext(() => {
                playNextTrack().catch(console.error)
            })
        )

        unlisteners.push(
            listenToMediaKeyPrevious(() => {
                playPreviousTrack().catch(console.error)
            })
        )

        // Seeking
        unlisteners.push(
            listenToMediaKeySeek((offset) => {
                if (audioState) {
                    const newPosition = Math.max(
                        0,
                        Math.min(
                            audioState.current_position + offset,
                            audioState.duration
                        )
                    )
                    seekTo(newPosition).catch(console.error)
                }
            })
        )

        unlisteners.push(
            listenToMediaKeySeekTo((position) => {
                seekTo(position).catch(console.error)
            })
        )

        return () => {
            Promise.all(unlisteners).then((fns) => fns.forEach((fn) => fn()))
        }
    }, [isPlaying, audioState])

    // Handle search with debounce
    useEffect(() => {
        if (searchQuery.trim()) {
            // Debounce search
            if (searchTimeout) clearTimeout(searchTimeout)
            const timeout = setTimeout(() => {
                performSearch(searchQuery)
            }, 500)
            setSearchTimeout(timeout)

            return () => clearTimeout(timeout)
        } else {
            // Clear results and cancel any pending searches when query is empty
            cancelSearch().catch(console.error)
            searchRequestIdRef.current += 1
            setSearchResults([])
            setIsSearching(false)
        }
    }, [searchQuery, isMusicMode])

    const performSearch = async (query: string) => {
        if (!query.trim()) return

        // Cancel any ongoing search on the backend
        await cancelSearch().catch(console.error)

        // Increment request ID to invalidate previous searches
        searchRequestIdRef.current += 1
        const currentRequestId = searchRequestIdRef.current

        console.log(
            `🔍 Starting search request #${currentRequestId} for: "${query}"`
        )

        setIsSearching(true)
        // Auto-switch to search tab when user starts searching
        setActiveTab('search')
        try {
            const results = await searchYoutube(query, isMusicMode)

            // Only use results if this is still the current request
            if (searchRequestIdRef.current === currentRequestId) {
                console.log(
                    `⚡ Fast search completed in request #${currentRequestId} with ${results.length} results (durations loading...)`
                )

                // Show results immediately (duration will be 0 initially)
                setSearchResults(results)
                setIsSearching(false)

                // Durations will be fetched on-demand when items become visible
            } else {
                console.log(
                    `🚫 Ignoring stale search request #${currentRequestId} (current: #${searchRequestIdRef.current})`
                )
            }
        } catch (error) {
            // Only handle error if this is still the current request
            if (searchRequestIdRef.current === currentRequestId) {
                console.error('Search failed:', error)
                setSearchResults([])
                setIsSearching(false)
            } else {
                console.log(
                    `🚫 Ignoring error from stale search request #${currentRequestId}`
                )
            }
        }
    }

    if (isInitializing) {
        return (
            <DependencyLoader
                status={loadingStatus}
                progress={loadingProgress}
            />
        )
    }

    return (
        <div className="flex flex-col h-screen bg-background select-none rounded-[12px] overflow-hidden border border-white/10">
            {/* Header - App Title + Search Bar */}
            <AppHeader
                query={searchQuery}
                onQueryChange={setSearchQuery}
                isMusicMode={isMusicMode}
                onMusicModeToggle={() => setIsMusicMode(!isMusicMode)}
            />

            {/* Player - appears below header when track is loaded */}
            {currentTrack && (
                <>
                    {!isExpanded ? (
                        <MiniPlayer
                            track={currentTrack}
                            isPlaying={isPlaying}
                            isLoading={audioState?.is_loading || false}
                            onExpand={() => setIsExpanded(true)}
                        />
                    ) : (
                        audioState && (
                            <ExpandedPlayer
                                audioState={audioState}
                                onCollapse={() => setIsExpanded(false)}
                            />
                        )
                    )}
                </>
            )}

            {/* Tab Navigation */}
            <div className="flex border-b border-macos-separator bg-card flex-shrink-0">
                <button
                    onClick={() => setActiveTab('search')}
                    className={`flex-1 py-2 text-[13px] font-medium transition-colors ${
                        activeTab === 'search'
                            ? 'text-[var(--macos-blue)] border-b-2 border-[var(--macos-blue)]'
                            : 'text-muted-foreground hover:text-foreground'
                    }`}
                >
                    <span>Search</span>
                </button>
                <button
                    onClick={() => setActiveTab('queue')}
                    className={`flex-1 py-2 text-[13px] font-medium transition-colors ${
                        activeTab === 'queue'
                            ? 'text-[var(--macos-blue)] border-b-2 border-[var(--macos-blue)]'
                            : 'text-muted-foreground hover:text-foreground'
                    }`}
                >
                    <span>Queue</span>
                </button>
                <button
                    onClick={() => setActiveTab('playlists')}
                    className={`flex-1 py-2 text-[13px] font-medium transition-colors ${
                        activeTab === 'playlists'
                            ? 'text-[var(--macos-blue)] border-b-2 border-[var(--macos-blue)]'
                            : 'text-muted-foreground hover:text-foreground'
                    }`}
                >
                    <span>Playlists</span>
                </button>
                <button
                    onClick={() => setActiveTab('downloads')}
                    className={`flex-1 py-2 text-[13px] font-medium transition-colors ${
                        activeTab === 'downloads'
                            ? 'text-[var(--macos-blue)] border-b-2 border-[var(--macos-blue)]'
                            : 'text-muted-foreground hover:text-foreground'
                    }`}
                >
                    <span>Downloads</span>
                </button>
                <button
                    onClick={() => setActiveTab('settings')}
                    className={`flex-1 py-2 text-[13px] font-medium transition-colors ${
                        activeTab === 'settings'
                            ? 'text-[var(--macos-blue)] border-b-2 border-[var(--macos-blue)]'
                            : 'text-muted-foreground hover:text-foreground'
                    }`}
                >
                    <span>Settings</span>
                </button>
            </div>

            {/* Tab Content */}
            <div className="flex-1 overflow-hidden">
                {activeTab === 'search' && (
                    <SearchTab
                        query={searchQuery}
                        isMusicMode={isMusicMode}
                        results={searchResults}
                        isSearching={isSearching}
                    />
                )}
                {activeTab === 'queue' && <QueueTab />}
                {activeTab === 'playlists' && <PlaylistsTab />}
                {activeTab === 'downloads' && <DownloadsTab />}
                {activeTab === 'settings' && <SettingsTab />}
            </div>
        </div>
    )
}

export const Component = HomePage
