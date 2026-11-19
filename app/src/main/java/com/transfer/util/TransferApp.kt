package com.transfer.util

import android.app.Application
import com.google.android.material.color.DynamicColors
import timber.log.Timber

class TransferApp : Application() {
    companion object {
        lateinit var memoryTree: InMemoryLogTree
            private set
    }

    override fun onCreate() {
        super.onCreate()

        // API 31+ only: let Material components pick wallpaper ("Material You") colors.
        // No-op below Android 12, where the values/themes.xml look is kept.
        DynamicColors.applyToActivitiesIfAvailable(this)

        // 1) Plant the debug tree
        Timber.plant(Timber.DebugTree())
        // 2) Plant custom in‑memory tree
        memoryTree = InMemoryLogTree()
        Timber.plant(memoryTree)

        Thread.setDefaultUncaughtExceptionHandler { thread, throwable ->
            Timber.e(throwable, "Uncaught exception in thread ${thread.name}")
        }
    }
}