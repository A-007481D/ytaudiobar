import { useState, useEffect, useRef } from 'react'
import { Shuffle, Repeat, Repeat1, ListMusic, GripVertical } from 'lucide-react'
import {
    getQueue,
    toggleShuffle,
    cycleRepeatMode,
    reorderQueue,
    removeFromQueue,
    getShuffleMode,
    getRepeatMode,
    type YTVideoInfo,
    type RepeatMode
} from '@/lib/tauri'
import { TrackItem } from '@/components/track-item'
import { TabHeader } from '@/components/tab-header'

export function QueueTab() {
    const [queue, setQueue] = useState<YTVideoInfo[]>([])
    const [shuffleMode, setShuffleMode] = useState(false)
    const [repeatMode, setRepeatMode] = useState<RepeatMode>('Off')
    const [draggedIndex, setDraggedIndex] = useState<number | null>(null)
    const [dragOverIndex, setDragOverIndex] = useState<number | null>(null)
    const [isLoading, setIsLoading] = useState(true)
    const isMountedRef = useRef(true)
    const latestLoadRequestRef = useRef(0)

    const loadQueue = async () => {
        const requestId = ++latestLoadRequestRef.current

        try {
            const [queueData, shuffle, repeat] = await Promise.all([
                getQueue(),
                getShuffleMode(),
                getRepeatMode()
            ])

            // Ignore stale responses from older polling requests.
            if (
                !isMountedRef.current ||
                requestId !== latestLoadRequestRef.current
            ) {
                return
            }

            setQueue(queueData)
            setShuffleMode(shuffle)
            setRepeatMode(repeat)
        } catch (error) {
            console.error('Failed to load queue:', error)
        } finally {
            if (
                isMountedRef.current &&
                requestId === latestLoadRequestRef.current
            ) {
                setIsLoading(false)
            }
        }
    }

    useEffect(() => {
        isMountedRef.current = true
        void loadQueue()

        // Poll queue/modes because multiple views can mutate queue state.
        const interval = setInterval(() => {
            void loadQueue()
        }, 2000)

        return () => {
            isMountedRef.current = false
            clearInterval(interval)
        }
    }, [])

    const handleToggleShuffle = async () => {
        try {
            const enabled = await toggleShuffle()
            setShuffleMode(enabled)
            await loadQueue()
        } catch (error) {
            console.error('Failed to toggle shuffle:', error)
        }
    }

    const handleCycleRepeat = async () => {
        try {
            const mode = await cycleRepeatMode()
            setRepeatMode(mode)
        } catch (error) {
            console.error('Failed to cycle repeat mode:', error)
        }
    }

    const handleRemoveFromQueue = async (index: number) => {
        try {
            await removeFromQueue(index)
            await loadQueue()
        } catch (error) {
            console.error('Failed to remove from queue:', error)
        }
    }

    // Drag and drop handlers
    const handleDragStart = (event: React.DragEvent, index: number) => {
        event.dataTransfer.effectAllowed = 'move'
        event.dataTransfer.setData('text/plain', String(index))
        setDraggedIndex(index)
    }

    const handleDragEnter = (e: React.DragEvent, index: number) => {
        e.preventDefault()
        if (draggedIndex === null || draggedIndex === index) return
        setDragOverIndex(index)
    }

    const handleDragOver = (e: React.DragEvent) => {
        e.preventDefault()
    }

    const handleDragLeave = () => {
        setDragOverIndex(null)
    }

    const handleDrop = (e: React.DragEvent, dropIndex: number) => {
        e.preventDefault()
        if (draggedIndex === null || draggedIndex === dropIndex) {
            setDraggedIndex(null)
            setDragOverIndex(null)
            return
        }

        const newQueue = [...queue]
        const draggedItem = newQueue[draggedIndex]
        if (!draggedItem) {
            setDraggedIndex(null)
            setDragOverIndex(null)
            return
        }

        // Remove from old position
        newQueue.splice(draggedIndex, 1)

        // Insert at new position
        newQueue.splice(dropIndex, 0, draggedItem)

        setQueue(newQueue)
        setDraggedIndex(null)
        setDragOverIndex(null)

        // Update queue order in backend
        reorderQueue(newQueue).catch((error) => {
            console.error('Failed to reorder queue:', error)
            void loadQueue()
        })
    }

    const handleDragEnd = () => {
        setDraggedIndex(null)
        setDragOverIndex(null)
    }

    return (
        <div className="flex flex-col h-full bg-background">
            <TabHeader
                title="Queue"
                actions={
                    <>
                        <button
                            onClick={handleToggleShuffle}
                            className={`w-8 h-8 flex items-center justify-center rounded-full hover-macos-button transition-colors ${
                                shuffleMode
                                    ? 'text-[var(--macos-blue)]'
                                    : 'text-muted-foreground'
                            }`}
                            title={shuffleMode ? 'Shuffle On' : 'Shuffle Off'}
                        >
                            <Shuffle className="w-4 h-4" />
                        </button>
                        <button
                            onClick={handleCycleRepeat}
                            className={`w-8 h-8 flex items-center justify-center rounded-full hover-macos-button transition-colors ${
                                repeatMode !== 'Off'
                                    ? 'text-[var(--macos-blue)]'
                                    : 'text-muted-foreground'
                            }`}
                            title={`Repeat ${repeatMode}`}
                        >
                            {repeatMode === 'One' ? (
                                <Repeat1 className="w-4 h-4" />
                            ) : (
                                <Repeat className="w-4 h-4" />
                            )}
                        </button>
                    </>
                }
            />

            {/* Queue Content */}
            <div className="flex-1 overflow-y-auto">
                {isLoading ? null : queue.length === 0 ? (
                    <div className="flex flex-col items-center justify-center h-full text-center px-6">
                        <ListMusic className="w-12 h-12 text-muted-foreground mb-4 opacity-60" />
                        <h3 className="text-[15px] font-semibold text-foreground mb-2">
                            Queue is Empty
                        </h3>
                        <p className="text-[13px] text-muted-foreground max-w-[250px]">
                            Use Play All on a playlist to add tracks to your
                            queue
                        </p>
                    </div>
                ) : (
                    <div className="py-2">
                        {queue.map((track, index) => (
                            <div
                                key={track.id}
                                draggable
                                onDragStart={(e) => handleDragStart(e, index)}
                                onDragEnter={(e) => handleDragEnter(e, index)}
                                onDragOver={handleDragOver}
                                onDragLeave={handleDragLeave}
                                onDrop={(e) => handleDrop(e, index)}
                                onDragEnd={handleDragEnd}
                                className={`group flex items-center gap-1 transition-all duration-200 ${
                                    draggedIndex === index
                                        ? 'opacity-30 scale-95'
                                        : 'opacity-100'
                                } ${
                                    dragOverIndex === index &&
                                    draggedIndex !== index
                                        ? 'border-t-2 border-[var(--macos-blue)] pt-2'
                                        : ''
                                }`}
                            >
                                {/* Drag Handle */}
                                <div className="pl-1 cursor-grab active:cursor-grabbing">
                                    <GripVertical className="w-4 h-4 text-muted-foreground" />
                                </div>

                                <div className="flex-1 min-w-0 overflow-hidden">
                                    <TrackItem
                                        track={track}
                                        context="queue"
                                        queueIndex={index}
                                        onRemove={() =>
                                            handleRemoveFromQueue(index)
                                        }
                                    />
                                </div>
                            </div>
                        ))}
                    </div>
                )}
            </div>
        </div>
    )
}
